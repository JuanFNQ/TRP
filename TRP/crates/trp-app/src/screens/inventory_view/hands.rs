//! The quick keys and the hands' state with its chambers.

use super::*;

/// The quick-use keys, 4 to 0 (`ECommand.SelectFastSlot4` and on; the
/// install has them on the key coming up), with the screen open: the key
/// binds the thing under the mouse to itself, if it is the character's
/// and lies where it may be bound (`ItemUiContext.BindItem`). In play they
/// are heard before the controller's step (`quick_use_keys`). And the
/// grenade the grenade's key takes, kept for the quick bar's view.
pub(super) fn quick_keys(
    bindings: Res<Bindings>,
    controller: Res<Controller>,
    game: Res<GameData>,
    features: Res<crate::screens::options_view::Features>,
    mut carried: ResMut<Carried>,
    blinking: Res<Blinking>,
    items: Query<(&ItemView, &RelativeCursorPosition, &ComputedNode)>,
    covers: Query<(&Cover, &RelativeCursorPosition)>,
) {
    // With the options' `INVENTORY SYSTEM` off nothing is bound or used.
    if !features.inventory {
        return;
    }
    // The grenade the key takes and its view shows: the equipment's top
    // priority grenade, which the view sets at the raid's start, and again
    // when one is gone, to the first of the list by kind
    // (`FastAccessGrenadeItemView`; chosen otherwise in the game's grenade
    // selector, which is not built).
    let top = if features.grenades_and_knife { top_grenade(&carried, &game.db) } else { None };
    if carried.top_grenade != top {
        carried.top_grenade = top;
    }
    // Under the edit build screen no key of these is heard, as under any
    // of the game's screens (inventory spec, "What an open screen does to
    // the keys": they are blocked while it is open).
    if controller.edit_build || !controller.inventory_open {
        return;
    }
    let Some(key) = (0..QUICK_KEYS.len()).find(|key| bindings.fired(QUICK_KEYS[*key].0)) else { return };
    let word = match bindings.caption(QUICK_KEYS[key].0) {
        "" => QUICK_KEYS[key].1,
        word => word,
    };
    let carried = &mut *carried;
    if carried.asking.is_some() {
        return;
    }
    let Some((id, _)) = item_under(items.iter(), cover_under(covers.iter()), None, &blinking) else { return };
    // Only what the character has is bound.
    let Some(item) = carried.equipment.item.find(id) else { return };
    let name = carried.name(item);
    carried.note = match carried.bound.bind(&carried.equipment.item, id, key) {
        true => format!("{name}: bound to {word}"),
        false => format!("{name}: not bound (a key takes a weapon, a grenade, a knife, medicine, food, drink or a rangefinder that lies in the vest or the pockets)"),
    };
}

/// The quick-use keys in play, heard before the controller's step: the key
/// takes what it is bound to in hand (`Player.SetQuickSlotItem`): a weapon
/// is drawn, a grenade or the rangefinder taken; medicine is used if it can
/// do something now, food and drink if anything is left. And the grenade's
/// own key and the knife's quick stroke. From the empty hands, what they
/// ask is in hand before the step (`hands_first`); from anything else the
/// hands go by `settle` after it.
pub(super) fn quick_use_keys(
    bindings: Res<Bindings>,
    options: Res<Options>,
    controller: Res<Controller>,
    game: Res<GameData>,
    features: Res<crate::screens::options_view::Features>,
    mut carried: ResMut<Carried>,
    mut asked: ResMut<AskedFor>,
) {
    // Not with the options' `INVENTORY SYSTEM` off, nor under a screen
    // (`quick_keys` binds with the inventory open).
    if !features.inventory || controller.edit_build || controller.inventory_open {
        return;
    }
    // The grenade's own key takes the first grenade within reach (the one
    // `quick_keys` keeps for the quick bar, found afresh); the knife's
    // twice, the quick stroke (grenades-and-knife spec, "Taking them in
    // hand"). With the options' switch for them off, neither.
    if options.keys_live() && features.grenades_and_knife {
        if bindings.fired("ThrowGrenade") {
            match top_grenade(&carried, &game.db) {
                Some(id) => asked.0 = Some(id),
                None => carried.note = "no grenade lies in the vest or the pockets".to_owned(),
            }
        }
        if bindings.fired("QuickKnife") {
            asked.2 = true;
        }
    }
    let Some(key) = (0..QUICK_KEYS.len()).find(|key| bindings.fired(QUICK_KEYS[*key].0)) else { return };
    let word = match bindings.caption(QUICK_KEYS[key].0) {
        "" => QUICK_KEYS[key].1,
        word => word,
    };
    let carried = &mut *carried;
    if !options.keys_live() {
        return;
    }
    let Some(item) = carried.bound.items[key].and_then(|id| carried.equipment.item.find(id)) else { return };
    if is(item, "Weapon") {
        asked.0 = Some(item.id);
    } else if is(item, "ThrowWeap") && features.grenades_and_knife {
        // A grenade is taken in hand to be thrown (`TryProceed`).
        asked.0 = Some(item.id);
    } else if is(item, "PortableRangeFinder") {
        // The rangefinder is taken in hand to be aimed through
        // (`ProceedPortableRangeFinder`; rangefinder spec).
        asked.0 = Some(item.id);
    } else if let Some(medicine) = trp_sim::condition::health::Medicine::of(&game.db, &item.kind.template) {
        // Medicine is used only if it can do something to the body now
        // (`Player.SetQuickSlotItem`: `CanApplyItem(item, Common)`); food
        // and drink if anything is left (health spec, "The hands").
        let (id, name, resource) = (item.id, carried.name(item), item.resource.unwrap_or_else(|| medicine.full()));
        match controller.health.as_ref() {
            Some(health) if health.part_to_apply(&medicine, resource, None).is_some() => asked.0 = Some(id),
            Some(_) => carried.note = format!("{name}: it can do nothing now"),
            None => carried.note = format!("{name}: with health switched off nothing is used"),
        }
    } else {
        // A knife or a grenade: not built yet.
        let name = carried.name(item);
        info!("the key {word}: {name} would be taken in hand; that is not built yet");
        carried.note = format!("{name}: the key {word} takes it in hand in the game; that is not built yet");
    }
}

/// What the hands have been through, for `settle` (and `hands_first`,
/// which takes from the empty hands before the controller's step).
#[derive(Resource, Default)]
pub(super) struct Hands {
    pub(super) settled: bool,
    /// The frame's keys have been heard before the step, with the hands
    /// empty (`hands_first`): `settle` does not hear them again.
    pub(super) heard: bool,
    /// When a weapon was last put away for the quick draw
    /// (`QuickdrawWeaponFast`, `QuickdrawTime`): the pistol drawn within a
    /// second after is drawn quick (stamina spec, "A drain";
    /// `trp_sim::quick_draw`). Whether the last slot was taken by the
    /// holster's key pressed twice is the player's (`Controller::fast_slot`).
    pub(super) quickdraw: Option<f32>,
    /// The weapon in hand, or on its way out of the hands.
    pub(super) item: Option<Id>,
    /// The slot the hands last drew from (the game's `ActiveSlot`); it
    /// stays so when that slot is emptied.
    pub(super) active: Option<&'static str>,
    /// Whether the inventory was open when the thing in hand was last
    /// told of it.
    pub(super) told_open: bool,
    /// The weapon has been told to go and is not out of the hands yet.
    pub(super) going: bool,
    /// What is to be taken in hand when it has gone: a weapon and the
    /// slot it hangs in, or nothing (the game's scheduled process).
    pub(super) next: Option<(Id, &'static str)>,
    /// A weapon asked for by its slot's key while its rig was still being
    /// made (`weapon::Preparing`), and its slot: the key is heard when the
    /// rig comes, unless another is pressed first or the weapon leaves the
    /// slot. (Not the game's: its weapons have no rig to wait for.)
    pub(super) waiting: Option<(Id, &'static str)>,
    /// A weapon put into the slot the hands last drew from and taken in
    /// hand for it, until it has appeared there; and the weapon in hand
    /// taken out of its slot, until it has been put away and the next is
    /// in hand. Their boxes blink (`Blinking`).
    pub(super) moved_in: Option<Id>,
    pub(super) moved_out: Option<Id>,
    /// The item being used in the hands (medicine, food or drink), until
    /// it has been put away.
    pub(super) using: Option<Id>,
    /// The part the item waiting to be used is for (let go on the HEALTH
    /// tab's, or the inventory's "Use"), or the common body (a quick-use
    /// key's); and food's or drink's share asked (none: what is left).
    pub(super) use_part: Option<trp_sim::condition::health::Part>,
    pub(super) use_amount: Option<f32>,
    /// The weapon held, the item in use and whether the hands were putting
    /// something away, as the last frame left them: a change of the hands
    /// begins as they start putting away, or take something into empty
    /// hands.
    pub(super) was: (Option<Id>, Option<Id>, bool),
    /// An item whose use was asked for while its rig was still being made,
    /// as `waiting` is for a weapon.
    pub(super) use_waiting: Option<Id>,
    /// An item to be used when what is in hand has gone.
    pub(super) next_use: Option<Id>,
    /// The item waiting to be used is the sheath's knife for its quick
    /// stroke (grenades-and-knife spec).
    pub(super) use_quick: bool,
    /// The weapon last taken in hand (`LastEquippedWeaponOrKnifeItem`):
    /// what comes back when an item's use is over.
    pub(super) last_weapon: Option<Id>,
    /// The item in use was taken from the empty hands: when its use is over
    /// the hands take the first weapon in the slots' order
    /// (`SetFirstAvailableItem`), not the one last held.
    pub(super) from_empty: bool,
    /// The weapon in hand and the magazine in its slot as the last frame
    /// left them, and a magazine its hands are moving for the inventory
    /// (firing spec, "A magazine taken out or put in from the inventory").
    pub(super) fitted: Option<(Id, Option<Id>)>,
    pub(super) magazine_moving: Option<Id>,
    /// A shot has been made since the weapon's magazine was last set beside
    /// its logic's count (`UncheckOnShot`).
    pub(super) fired: bool,
    /// The weapon whose hands are putting a round into its chamber.
    pub(super) loading_chamber: Option<Id>,
    /// The weapon whose empty magazine the reload key's last resort is
    /// sending to the rig, the round to go into its chamber as the hands
    /// are done (`chamber_by_hand`).
    pub(super) chamber_after_magazine: Option<Id>,
    /// The weapon whose hands are putting a round into one of its barrels,
    /// and the barrel's number (a load given up is undone).
    pub(super) loading_barrel: Option<(Id, usize)>,
    /// The weapon in hand and whether its item had a round in its chamber
    /// when the item and the logic last agreed: a round the inventory has
    /// moved in or out since is a change in the item, where a shot leaves
    /// the item behind the logic until it is at rest again.
    pub(super) chamber_agreed: Option<(Id, bool)>,
    /// The weapon in hand with an underbarrel launcher and whether the
    /// launcher's item had a round in its chamber when they last agreed (none
    /// until a weapon newly in hand has taken its item's).
    pub(super) launcher_agreed: Option<(Id, bool)>,
    /// The weapon in hand with chambers of its own (a cylinder, barrels)
    /// and the round and the case its item held in each when the item and
    /// the logic last agreed: an item changed since is the inventory's
    /// doing, which the logic takes; else the logic's go into the item.
    /// None until the weapon taken in hand has been made from its item.
    pub(super) chambers_agreed: Option<(Id, ChambersHeld)>,
    /// The weapon was put away for the workbench: the empty hands do not
    /// take it again from its slot until asked (`BenchAsk`).
    pub(super) holstered: bool,
    /// The frame the hands were last asked for something on, until it is
    /// in them, for a run that writes each change (`controller::hands_log`).
    pub(super) asked_at: Option<u32>,
}

/// What a weapon's item holds in its chambers of its own, by their numbers:
/// the round's template in each, and the case's (`Weapon.ShellsInChambers`).
pub(super) type ChambersHeld = (Vec<Option<String>>, Vec<Option<String>>);

/// The weapon whose chamber a round carried over a slot shows
/// (`SlotView.HighlightItemViewPosition`): a weapon's chamber
/// (`patron_in_weapon`) whose filter takes the round (`CanAccept`, full or
/// not), or a slot whose name has "Weapon" (the sling's and the back's, not
/// the holster's) holding a weapon whose first chamber takes it.
pub(super) fn round_over_chamber(carried: &Carried, round: Id, holder: Id, slot: usize) -> Option<Id> {
    let round = carried.find(round).filter(|item| is(item, "Ammo"))?;
    let holder = carried.find(holder)?;
    let own = holder.kind.slots.get(slot)?;
    let takes = |chamber: &trp_data::character::inventory::Holder| round.kind.among(&chamber.takes) && !round.kind.among(&chamber.refuses);
    if own.name.starts_with("patron_in_weapon") && is(holder, "Weapon") {
        return takes(own).then_some(holder.id);
    }
    let weapon = holder.slots.get(slot)?.as_ref().filter(|weapon| own.name.contains("Weapon") && is(weapon, "Weapon"))?;
    let chamber = weapon.kind.slots.iter().find(|chamber| chamber.name.starts_with("patron_in_weapon"))?;
    takes(chamber).then_some(weapon.id)
}

/// Whether a weapon's chamber is known after the shots that took its
/// magazine from `rounds` to `count` (`FireOperation.UncheckOnShot`, at
/// each shot that fed it: the chamber known as the magazine was before that
/// shot, the magazine then known by `known_after`, a shot unknowing it
/// where `shot`).
pub(super) fn chamber_after_shots(known: bool, rounds: u32, count: u32, capacity: u32, shot: bool) -> bool {
    let (mut magazine, mut chamber) = (known, known);
    for left in (count..rounds).rev() {
        chamber = magazine;
        magazine = trp_sim::items::inventory::known_after(left, capacity, magazine, shot);
    }
    chamber
}

/// A weapon's chambers of its own as its item holds them, wherever it lies.
pub(super) fn item_chambers(carried: &Carried, gun: Id) -> ChambersHeld {
    let templates = |list: Vec<Option<std::sync::Arc<trp_sim::items::inventory::Kind>>>| list.into_iter().map(|kind| kind.map(|kind| kind.template.clone())).collect();
    carried.find(gun).map(|weapon| (templates(trp_sim::items::inventory::chamber_rounds(weapon)), templates(trp_sim::items::inventory::chamber_cases(weapon)))).unwrap_or_default()
}

/// A weapon's magazine, if one is fitted.
pub(super) fn fitted_magazine(weapon: &Item) -> Option<Id> {
    weapon.slot_index(trp_sim::items::inventory::MAGAZINE_SLOT).and_then(|slot| weapon.slots[slot].as_ref()).map(|magazine| magazine.id)
}

/// A weapon taken in hand comes as the item has it: its magazine's rounds
/// (and those its chamber refuses), and its fold (shotgun spec, "The
/// stock": the animator is told as it is drawn) with its logic, its recoil
/// and hold, and the width the body's animator goes by.
pub(super) fn fold_as_item(weapon: &mut HandsRig, rig: &WeaponRig, controller: &mut Controller, folded: bool, (magazine, chamber, variant): (Option<Option<u32>>, Option<bool>, Option<(i32, bool, u32)>)) {
    let Some(firearm) = weapon.firearm.as_mut() else { return };
    // Its magazine's or tube's rounds as the item holds them, and its
    // chamber of its own (a cylinder's or barrels' rounds lie in chambers
    // of their own: the logic's alone); the fitted magazine's clips' index
    // (`magAnimationIndex`) and whether it comes out (`ReloadMagType`),
    // where the item's magazine is not the build's.
    if let Some(magazine) = magazine.filter(|_| firearm.single_chamber()) {
        firearm.magazine = magazine;
    }
    if let Some((variant, external, unfit)) = variant {
        firearm.set_magazine_variant(variant);
        firearm.set_magazine_external(external);
        firearm.set_magazine_unfit(unfit);
    }
    if let Some(chamber) = chamber.filter(|_| firearm.single_chamber()) {
        firearm.chamber = chamber;
    }
    firearm.start_folded(folded);
    let folded = firearm.stock_folded();
    weapon.hands.set_folded(folded, false);
    controller.movement.body.set_weapon_width(if folded { weapon.folded_width.unwrap_or(rig.width) } else { rig.width });
}

/// The rig that has just been taken in hand is drawn from its animator's
/// first pose on, and the simulation's next frame reads that of it.
pub(super) fn first_pose(weapon: &mut HandsRig, controller: &mut Controller) {
    if let Some(pose) = weapon.animator().map(|animator| animator.pose_now()) {
        let read = weapon.frame.read(&pose);
        controller.rig_changed(pose, read);
    }
}

/// The keys of the three weapon slots by the game's names for them
/// (`PlayerInputTranslator`: the sling's, the back's, the holster's).
/// And the knife's, the sheath's (`SelectKnife`; grenades-and-knife spec).
pub(super) const SLOT_KEYS: [(&str, &str); 4] = [("PrimaryWeaponFirst", "FirstPrimaryWeapon"), ("PrimaryWeaponSecond", "SecondPrimaryWeapon"), ("SecondaryWeapon", "Holster"), ("Knife", "Scabbard")];
