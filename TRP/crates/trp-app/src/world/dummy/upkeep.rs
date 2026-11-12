//! Blows, wear kept, weapons cleared, and dummies shown, healed and reweighed.

use super::*;

/// The knife's blows on the dummies since the last frame (`controller::
/// step`'s sweep of the blade: `knife`), each taken (`Dummies::take_blow`)
/// with the knife in hand's poison: its own doses, kept on its thing
/// (`SideEffectComponent.Value`; `Item::resource`, none while full), one
/// spent on each blow that puts it on, the thing then told
/// (`RaiseRefreshEvent`). Before the dummies are stepped and placed.
pub fn blow_dummies(
    mut controller: ResMut<crate::player::controller::Controller>,
    dummies: Option<ResMut<Dummies>>,
    held: Res<crate::screens::inventory_view::HandsHeld>,
    mut carried: ResMut<crate::screens::inventory_view::Carried>,
    game: Res<crate::GameData>,
) {
    if controller.blows.is_empty() {
        return;
    }
    let blows = std::mem::take(&mut controller.blows);
    let Some(mut dummies) = dummies else { return };
    for blow in blows {
        let knife = held.item.and_then(|id| carried.equipment.item.find(id));
        let poison = knife.and_then(|knife| {
            let mut side = trp_sim::condition::health::SideEffect::of(&game.db.items.get(&knife.kind.template)?.props)?;
            side.value = knife.resource.unwrap_or(side.max);
            Some((knife.id, knife.kind.template.clone(), side))
        });
        match poison {
            Some((id, template, mut side)) => {
                let before = side.value;
                dummies.take_blow(&blow, Some((&template, &mut side)));
                if side.value != before {
                    if let Some(knife) = carried.equipment.item.find_mut(id) {
                        knife.resource = Some(side.value);
                    }
                    info!("the {template}'s poison ({}) put on dummy {}: {} of {} doses left", side.buffs, blow.target.which, side.value, side.max);
                }
            }
            None => {
                dummies.take_blow(&blow, None);
            }
        }
    }
}

/// Each dummy's armour's wear written onto its things after the frame's
/// bullets and blasts (body hits spec, "Its armour's wear kept on its
/// things"), wherever its body's gear now is (on the body, or on the screen
/// while searched: `Carried::body_mut`), so that a vest taken off it, or
/// looked at on it, shows its wear; a knife's blows' too (`blow_dummies`).
pub fn keep_dummy_wear(dummies: Option<ResMut<Dummies>>, mut carried: ResMut<crate::screens::inventory_view::Carried>, game: Res<crate::GameData>) {
    let Some(mut dummies) = dummies else { return };
    if !dummies.list.iter().any(|dummy| dummy.worn) {
        return;
    }
    for dummy in &mut dummies.list {
        if !std::mem::take(&mut dummy.worn) || !carried.body(dummy.which).is_some_and(|gear| wear_differs(&dummy.wearer, gear)) {
            continue;
        }
        let Some(gear) = carried.body_mut(dummy.which) else { continue };
        keep_wear(&dummy.wearer, gear);
        info!(
            "dummy {}'s armour's wear kept on its things: {}",
            dummy.which,
            dummy.wearer.pieces.iter().map(|p| format!("{} {:.2}/{:.0}", game.db.items.get(&p.template).map_or(p.template.as_str(), |t| t.name.as_str()), p.durability, p.max_durability)).collect::<Vec<_>>().join(", ")
        );
    }
}

/// A dead dummy's weapon gone from its body (body hits spec, "Its weapon in
/// hand stays in its slot"; `Corpse.RemoveLootItem` → `CorpseRagdoll.
/// ClearWeapon`, `Player.ReleaseHand`): once the thing it held as it died
/// leaves its slot by any move (taken by a search, put elsewhere on the
/// body), or where it held nothing, the model's body and spring are taken
/// out of its engine and the model hidden, until HEAL AND STAND UP. The
/// model is no thing to take by looking at it (`GameWorld.
/// FindInteractable` meets it, its loot switched off). After the falls (a
/// death's model hung first) and the screen's moves.
pub fn clear_dummy_weapons(dummies: Option<ResMut<Dummies>>, carried: Res<crate::screens::inventory_view::Carried>, mut visibility: Query<&mut Visibility>) {
    let Some(mut dummies) = dummies else { return };
    if !dummies.list.iter().any(|dummy| dummy.fall.as_ref().is_some_and(|fall| fall.hung)) {
        return;
    }
    for dummy in &mut dummies.list {
        let Some(fall) = dummy.fall.as_mut().filter(|fall| fall.hung) else { continue };
        if fall.in_hands.is_some_and(|held| still_in_hands(carried.body(dummy.which), held)) {
            continue;
        }
        fall.hung = false;
        if let Some(body) = fall.weapon.take() {
            fall.engine.remove(body);
        }
        if let Some(model) = dummy.model.as_ref() {
            if let Ok(mut shown) = visibility.get_mut(model.player) {
                *shown = Visibility::Hidden;
            }
        }
        info!("dummy {}'s weapon {}: its hung model gone, its body and spring out of the corpse's engine", dummy.which, if fall.in_hands.is_some() { "taken off its body" } else { "none in hand as it died" });
    }
}

/// Each piece of a dummy shown by what its own body has on (`Look`), as the
/// body's pieces are by what the character has on (`body_view::show_worn`,
/// `show_head_parts`, `show_outfit`, `show_torso`): what searching takes off
/// its body or puts on it comes and goes on it so, dead or standing again.
/// A thing put on it that the run had not read is read then and hung on it
/// (`worn_reads`), shown once it is (its copies `Added`).
pub fn show_dummies(
    carried: Res<crate::screens::inventory_view::Carried>,
    parts: Res<crate::player::body_view::BodyParts>,
    made: Query<(), (Added<DummyShows>, crate::platform::park::EvenParked)>,
    mut pieces: Query<(&DummyTwin, &DummyShows, &mut Visibility), crate::platform::park::EvenParked>,
) {
    // (Armour read later may be worn like a vest: `worn_reads`.)
    if !carried.is_changed() && !parts.is_changed() && made.is_empty() {
        return;
    }
    let looks: Vec<Option<Look>> = (0..places().len()).map(|which| carried.body(which).map(|gear| Look::of(&parts, gear))).collect();
    for (twin, shows, mut visibility) in &mut pieces {
        let shown = looks.get(twin.which).and_then(Option::as_ref).is_some_and(|look| look.shows(&shows.0));
        let wanted = if shown { Visibility::Inherited } else { Visibility::Hidden };
        if *visibility != wanted {
            *visibility = wanted;
        }
    }
}

/// The debug panel's HEAL AND STAND UP (each dummy's kit its own, kept as
/// changed): each dummy's health whole, its armour renewed on what its body
/// has on now (`renew_armour`: its things made new, TRP's default), its character made anew where it stands, standing again if
/// it lay dead, its own AKM back in its hands where its body has it in a
/// weapon's slot, else its hands empty. What its body holds
/// stays as it is, and as searched: a thing taken off it or put on it by
/// searching stays so.
#[allow(clippy::too_many_arguments)]
pub fn heal_dummies(
    options: Res<crate::Options>,
    game: Res<crate::GameData>,
    mut carried: ResMut<crate::screens::inventory_view::Carried>,
    source: Option<Res<DummySource>>,
    dummies: Option<ResMut<Dummies>>,
    rigs: Option<Res<DummyRigs>>,
    mut transforms: Query<&mut Transform>,
    mut visibility: Query<&mut Visibility>,
    mut panel: MessageReader<crate::platform::debug_menu::DebugCommand>,
) {
    // The debug panel's button (`debug_menu`; a command of the panel, with no key).
    let pressed = panel.read().any(|command| *command == crate::platform::debug_menu::DebugCommand::HealDummies);
    let (Some(source), Some(mut dummies), Some(rigs)) = (source, dummies, rigs) else { return };
    if !(options.keys_live() && pressed) {
        return;
    }
    // A body open on the screen gets its gear back first.
    carried.put_back_body();
    let dummies = &mut *dummies;
    let bleeding_drops = &mut dummies.bleeding_drops;
    let listened = dummies.listened;
    for dummy in &mut dummies.list {
        let rig = &rigs.0[dummy.which];
        dummy.wearer = match carried.body_mut(dummy.which) {
            Some(gear) => renew_armour(&game.db, source.rules.clone(), gear, VestSkills::of(&source.skills)),
            None => Wearer::dress(&game.db, source.rules.clone(), &[], VestSkills::of(&source.skills)),
        };
        dummy.worn = false;
        let kit = carried.body(dummy.which);
        let seed = dummy.health.random_whole(0, i32::MAX) as u64;
        dummy.health = Health::new(source.health.clone(), seed);
        dummy.health.skills = health_skills(&source.skills);
        bleeding_drops.push((dummy.character(), false));
        // A new character to its voice: what it was told of the old one let
        // go of.
        dummy.heard.clear();
        if listened {
            dummy.heard.push(Heard::Anew);
        }
        dummy.killing = None;
        // Its character made anew where it stands, and standing again if it
        // lay dead, its load read from what its body has on now (searched
        // while it lay dead, say): its own AKM in its hands where its body
        // has it in a weapon's slot, else its hands empty, as the game's
        // character takes its weapon last in hand from the body or, with
        // none there, has empty hands (`Player.SetControllerInsteadRemovedOne`,
        // `TrySetLastEquippedWeapon`). `PROVISIONAL`: another weapon put on
        // its body is not taken in hand (the game's would take the first
        // that can be); its hands are made with its own AKM's model alone.
        if let Some(maker) = source.maker.as_ref() {
            dummy.load = maker.load_of(kit, &source.skills);
            let holds = held_of(kit, dummy.own);
            match maker.make(&game, places()[dummy.which].at, kit, &source.skills, holds.is_some()) {
                Ok(puppet) => (dummy.puppet, dummy.holds) = (Some(puppet), holds),
                // (None then: it stands as the screens stand a character.)
                Err(e) => {
                    warn!("dummy {}'s character could not be made again: {e:#}", dummy.which);
                    (dummy.puppet, dummy.holds) = (None, None);
                }
            }
        }
        dummy.heading = 1.0;
        // Its new character's frames alone drawn and gone by, none of the
        // old one's: nothing drawn between the two (`Puppet::drawn`, a
        // character's own frames), its colliders where its skeleton stands
        // until its first (its old frames' placements let go: the new
        // one's numbers start again).
        dummy.forget_placements();
        let fell = dummy.fall.take().is_some();
        // The AKM's model in its hands while they hold it, else hidden
        // (none drawn in its hands, and its thing hung on the body by its
        // slot: `slung_view`).
        if let Some(model) = dummy.model.as_ref() {
            if let Ok(mut shown) = visibility.get_mut(model.player) {
                *shown = if dummy.holds.is_some() { Visibility::Inherited } else { Visibility::Hidden };
            }
        }
        if fell {
            dummy.locals = rig.rest.clone();
            for (bone, rest) in rig.bones.iter().zip(&rig.rest) {
                if let Ok(mut transform) = transforms.get_mut(*bone) {
                    *transform = *rest;
                }
            }
            if let Ok(mut root) = transforms.get_mut(rig.root) {
                *root = place(dummy.which);
            }
            let world = world_of(&rig.parents, &rig.rest, &[], None, Affine3A::IDENTITY);
            pose_outfits(rig, &world, &mut transforms);
        }
        info!(
            "dummy {} healed and stood up in its own gear, {}: {}; plates on {:#x}; {}",
            dummy.which,
            if dummy.holds.is_some() { "its AKM in its hands" } else { "its hands empty" },
            if dummy.wearer.pieces.is_empty() {
                "no armour".to_owned()
            } else {
                dummy.wearer.pieces.iter().map(|p| format!("{} (class {}, {:.0})", game.db.items.get(&p.template).map_or(p.template.as_str(), |t| t.name.as_str()), p.class, p.durability)).collect::<Vec<_>>().join(", ")
            },
            dummy.wearer.plates_on(),
            dummy.puppet.as_ref().map_or(String::new(), |puppet| load_said(puppet, &dummy.load, &game.db, &source.skills))
        );
    }
}

/// A standing dummy's load read again whenever its body changes (`Carried`:
/// a thing put on it or taken off it), as the game works a character's
/// weight and gear out again on every change of its inventory
/// (`Physical.OnWeightUpdated`, `Player.RecalculateEquipmentParams`): its
/// weight (the overweight's limits staying a bot's), its gear's speed
/// penalty and share of the ergonomics. After its frame's step, as the
/// game's `LateUpdate` after its movement (movement spec, "The weight, as
/// the inventory tells of it"). A dead body's is read when it is
/// stood up again (`heal_dummies`). Its walk floor goes by its own inertia
/// alone, as the player's by the player's: the game keys one curve for
/// every character, TRP each its own (a departure on purpose; movement
/// spec, "The floor's curve is shared").
pub fn reweigh_dummies(carried: Res<crate::screens::inventory_view::Carried>, game: Res<crate::GameData>, source: Option<Res<DummySource>>, dummies: Option<ResMut<Dummies>>) {
    let (Some(source), Some(mut dummies)) = (source, dummies) else { return };
    let Some(maker) = source.maker.as_ref() else { return };
    if !carried.is_changed() {
        return;
    }
    for dummy in dummies.list.iter_mut().filter(|dummy| dummy.dressed && dummy.health.alive && dummy.fall.is_none()) {
        let load = maker.load_of(carried.body(dummy.which), &source.skills);
        if load == dummy.load {
            continue;
        }
        let Some(puppet) = dummy.puppet.as_mut() else { continue };
        // Its weight worked out again only past the game's step of 0.01 kg
        // (`Movement::weigh`); its gear on any change.
        if let Err(e) = puppet.movement.weigh(&game.hard, &game.db.config, load.0) {
            warn!("dummy {}'s weight could not be read again: {e:#}", dummy.which);
            continue;
        }
        let (penalty, ergonomics) = gear_of(&game.db, &load.1, &source.skills);
        puppet.carry_gear(penalty, ergonomics);
        info!("dummy {}'s body changed: {}", dummy.which, load_said(puppet, &load, &game.db, &source.skills));
        dummy.load = load;
    }
}
