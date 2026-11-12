//! The bodies things hang on, the slung state and the bones wanted.

use super::*;

/// The slots whose things hang on the body (`PlayerBody.SlotNames`' weapons
/// and the holster's own view).
const SLOTS: [&str; 4] = ["FirstPrimaryWeapon", "SecondPrimaryWeapon", "Holster", "Scabbard"];

/// Whose body a model hangs on: every body the game makes a character's
/// slot views for (`PlayerBody.Init`), and the one it never shows them on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Body {
    /// The character's own, seen from outside (`show`).
    Character,
    /// The gear page's figure, and with `OVERALL FIGURE` off the OVERALL
    /// tab's too, one skeleton drawn by two cameras (`figure_view`).
    Figure,
    /// The OVERALL tab's own figure, with `OVERALL FIGURE` on
    /// (`figure_view::Overall`): the thing it was made holding left off.
    Overall,
    /// A dummy on the range, standing or dead, by its number.
    Dummy(usize),
    /// The character's corpse.
    Corpse,
}

impl Body {
    /// Whether anything hangs on it. Not on the character's corpse: the
    /// game's corpse is the living body kept (`Player.CreateCorpse` passes
    /// `reinitBody: false`, so `Corpse.InitBody` never runs for it), whose
    /// slots' renderers were switched off as its own player's
    /// (`CreateAndParent`, `_isYourPlayer`) and nothing switches them on.
    pub fn hangs(self) -> bool {
        !matches!(self, Body::Corpse)
    }

    pub(super) fn said(self) -> String {
        match self {
            Body::Character => String::new(),
            Body::Figure => " on the character model".to_owned(),
            Body::Overall => " on the OVERALL tab's own figure".to_owned(),
            Body::Dummy(which) => format!(" on dummy {which}"),
            Body::Corpse => " on the corpse".to_owned(),
        }
    }
}

/// What the hanging goes by, read as the run begins: the slots' bones, the
/// legs' holster (the legs the character begins in; a lower suit put on
/// later brings its own, `clothing::Clothing::holster`), each dummy's legs'
/// holster, and the templates whose flags pick a bone or a model.
#[derive(Resource)]
pub struct Slung {
    pub(super) bones: SlotBones,
    pub(super) holster: Option<LegsHolster>,
    /// The holster last hung by, to say when a suit changes it.
    pub(super) holster_now: Option<Option<LegsHolster>>,
    /// Each dummy's own legs' holster, by its number.
    pub(super) dummy_holsters: Vec<Option<LegsHolster>>,
    /// The holster the figure's pistol bone was last given.
    pub(super) figure_holster: Option<Option<LegsHolster>>,
    /// Knives shown on the body (`KnifeComponent.DisplayOnModel`).
    shown_knives: HashSet<String>,
    /// Weapons hung by their alternative point (`Weapon.UseAltMountBone`).
    pub(super) alternative_mount: HashSet<String>,
    /// Backpacks the weapons lean against (`LeanWeaponAgainstBody`).
    leaning: HashSet<String>,
    /// Each slot's model as hung, on every body.
    pub(super) hung: Vec<Hung>,
    /// What each body's slots were last found to hang (`keep_hung`).
    pub(super) wants: Vec<Wants>,
}

/// What a body's slots hang, as last worked out, and what it was worked
/// out from: the carried things as they last changed (their change's
/// number), the thing in hand, the legs' holster and whether rounds are
/// shown. While those stay, the builds' keys and the rounds' names are
/// not made again every frame. A thing's rounds are none until its model
/// is read (they go by the model's points).
pub(super) struct Wants {
    pub(super) body: Body,
    pub(super) stamp: u32,
    pub(super) in_hand: Option<Id>,
    pub(super) holster: Option<LegsHolster>,
    pub(super) rounds_shown: bool,
    pub(super) wanted: Vec<Want>,
    pub(super) rounds: Vec<Option<Vec<(usize, String, bool)>>>,
}

/// A slot's model as hung: the body and slot, its root, the item, its
/// model's key and the bone it is on.
pub(super) struct Hung {
    pub(super) body: Body,
    pub(super) template: String,
    pub(super) slot: &'static str,
    pub(super) root: Entity,
    pub(super) item: Id,
    pub(super) key: String,
    pub(super) bone: String,
    /// The rounds it was hung with (`loose::shown_rounds`).
    pub(super) rounds: Vec<(usize, String, bool)>,
    /// Each object's entity, by the model's own numbering.
    pub(super) objects: Vec<Entity>,
    /// Its tactical devices, numbered as `objects` (`loose::Ready::
    /// devices`).
    pub(super) devices: std::sync::Arc<Vec<trp_data::weapons::tactical::Device>>,
}

/// A weapon hung on a body as its tactical devices see it
/// (`tactical_view`): its body, its item, its model's root, its objects and
/// its devices.
pub struct HungDevices<'a> {
    pub body: Body,
    pub item: Id,
    pub root: Entity,
    pub objects: &'a [Entity],
    pub devices: &'a std::sync::Arc<Vec<trp_data::weapons::tactical::Device>>,
}

impl HungDevices<'_> {
    /// Its thing, wherever its body's gear is kept.
    pub fn item_in<'a>(&self, carried: &'a Carried) -> Option<&'a Item> {
        match self.body {
            Body::Dummy(which) => carried.body(which)?.find(self.item),
            _ => carried.equipment.item.find(self.item),
        }
    }
}

/// A hung model's root on the character's body (the one `show` hides in
/// first person).
#[derive(Component)]
pub struct SlungModel;

/// A hung model's root on one of the inventory's figures (`Body::Figure`,
/// `Body::Overall`): shown while a picture of that figure is drawn or
/// about to be, and parked otherwise (`figure_view::show_hung`).
#[derive(Component, Clone, Copy)]
pub struct FigureHung(pub Body);

/// A slot's thing to hang: its slot, its number, its model's key, its
/// template and the bone it goes on.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Want {
    pub(super) slot: &'static str,
    pub(super) item: Id,
    pub(super) key: String,
    pub(super) template: String,
    pub(super) bone: String,
}

impl Slung {
    pub fn new(db: &trp_data::Database, bones: SlotBones, holster: Option<LegsHolster>, dummy_holsters: Vec<Option<LegsHolster>>) -> Self {
        let flagged = |name: &str| -> HashSet<String> { db.items.values().filter(|item| item.props.get(name).and_then(|v| v.as_bool()).unwrap_or(false)).map(|item| item.id.clone()).collect() };
        info!(
            "weapons on the body: the sling's bone {:?}, the back's {:?}, the holster's {:?} ({}), the scabbard's {:?}",
            bones.primary,
            bones.secondary,
            if holster.is_some_and(|h| h.right) { &bones.pistol } else { &bones.pistol_left },
            match holster {
                Some(h) if h.right => "the legs' right",
                Some(_) => "the legs' left",
                None => "the legs carry none",
            },
            bones.scabbard
        );
        let side = |holster: &Option<LegsHolster>| match holster {
            Some(h) if h.right => "right",
            Some(_) => "left",
            None => "none",
        };
        info!("the dummies' legs' holsters: {:?}", dummy_holsters.iter().map(side).collect::<Vec<_>>());
        Self {
            bones,
            holster,
            holster_now: None,
            dummy_holsters,
            figure_holster: None,
            shown_knives: flagged("DisplayOnModel"),
            alternative_mount: flagged("UseAltMountBone"),
            leaning: flagged("LeanWeaponAgainstBody"),
            hung: Vec::new(),
            wants: Vec::new(),
        }
    }

    /// The bone a slot's thing hangs on (`CreateAndParent`): a weapon of
    /// the sling or the back on its slot's alternative bone with no
    /// backpack worn, and with one on the alternative where the pack leans
    /// the weapons against the body, else on its own; the pistol on the
    /// holster's leg's bone, where the legs carry a holster (`HaveHolster`:
    /// on legs without one no pistol is hung); the knife on the scabbard's.
    pub(super) fn bone(&self, slot: &str, backpack: Option<&str>, holster: Option<LegsHolster>) -> Option<&str> {
        let b = &self.bones;
        let weapon = |own: &Option<String>, alternative: &Option<String>| -> Option<String> {
            match (backpack, alternative) {
                (Some(pack), Some(alternative)) => Some(if self.leaning.contains(pack) { alternative.clone() } else { own.clone()? }),
                (None, Some(alternative)) => Some(alternative.clone()),
                (_, None) => own.clone(),
            }
        };
        let path = match slot {
            "FirstPrimaryWeapon" => weapon(&b.primary, &b.primary_alternative),
            "SecondPrimaryWeapon" => weapon(&b.secondary, &b.secondary_alternative),
            "Holster" => holster.and_then(|h| if h.right { b.pistol.clone() } else { b.pistol_left.clone() }),
            "Scabbard" => b.scabbard.clone(),
            _ => None,
        }?;
        [&b.primary, &b.primary_alternative, &b.secondary, &b.secondary_alternative, &b.pistol, &b.pistol_left, &b.scabbard].into_iter().flatten().find(|known| **known == path).map(String::as_str)
    }

    /// The holster bone's path for a holster, where the legs carry one.
    pub(super) fn holster_bone(&self, holster: Option<LegsHolster>) -> Option<(&str, LegsHolster)> {
        let holster = holster?;
        let path = if holster.right { self.bones.pistol.as_deref() } else { self.bones.pistol_left.as_deref() };
        Some((path?, holster))
    }

    /// What a body's slots hang (`SlotView`'s binding: `Binding.Combine` of
    /// the slot's thing and the item in hand): each slot's thing but the
    /// one `in_hand`, a knife only where it is shown on the body, each on
    /// its bone by the backpack `gear` wears and its legs' `holster`.
    pub(super) fn wanted(&self, gear: &Item, in_hand: Option<Id>, holster: Option<LegsHolster>) -> Vec<Want> {
        let backpack = gear.in_slot("Backpack").map(|pack| pack.kind.template.as_str());
        let mut wanted = Vec::new();
        for slot in SLOTS {
            let Some(item) = gear.in_slot(slot).filter(|item| Some(item.id) != in_hand) else { continue };
            if slot == "Scabbard" && !self.shown_knives.contains(&item.kind.template) {
                continue;
            }
            let Some(bone) = self.bone(slot, backpack, holster) else { continue };
            wanted.push(Want { slot, item: item.id, key: trp_sim::items::inventory::rig_key(item), template: item.kind.template.clone(), bone: bone.to_owned() });
        }
        wanted
    }

    /// What a body hangs: nothing on the character's corpse (`Body::
    /// hangs`), else its gear's slots' things but the one in hand
    /// (`wanted`).
    pub(super) fn hanging(&self, body: Body, gear: Option<&Item>, in_hand: Option<Id>, holster: Option<LegsHolster>) -> Vec<Want> {
        match gear {
            Some(gear) if body.hangs() => self.wanted(gear, in_hand, holster),
            _ => Vec::new(),
        }
    }

    /// What a body's slots hang, taken out of `wants` for the frame's work
    /// (`keep_hung` puts it back): as last worked out while the carried
    /// things' change number, the thing in hand, the holster and the
    /// rounds' switch are the same, else worked out afresh, its rounds
    /// yet to find.
    pub(super) fn take_wants(&mut self, body: Body, gear: Option<&Item>, stamp: u32, in_hand: Option<Id>, holster: Option<LegsHolster>, rounds_shown: bool) -> Wants {
        let kept = self.wants.iter().position(|wants| wants.body == body).map(|at| self.wants.swap_remove(at));
        match kept.filter(|wants| wants.stamp == stamp && wants.in_hand == in_hand && wants.holster == holster && wants.rounds_shown == rounds_shown) {
            Some(wants) => wants,
            None => {
                let wanted = self.hanging(body, gear, in_hand, holster);
                Wants { body, stamp, in_hand, holster, rounds_shown, rounds: vec![None; wanted.len()], wanted }
            }
        }
    }

    /// The slot bones the skeleton is to have for the hanging.
    pub fn wanted_bones(&self) -> Vec<String> {
        let b = &self.bones;
        [&b.primary, &b.primary_alternative, &b.secondary, &b.secondary_alternative, &b.pistol, &b.pistol_left, &b.scabbard].into_iter().flatten().cloned().collect()
    }

    /// The models hung on the character and the dummies that carry a
    /// tactical device (the figure's, on a layer of its own, light
    /// nothing in the world).
    pub fn with_devices(&self) -> impl Iterator<Item = HungDevices<'_>> {
        self.hung
            .iter()
            .filter(|hung| !hung.devices.is_empty() && matches!(hung.body, Body::Character | Body::Dummy(_)))
            .map(|hung| HungDevices { body: hung.body, item: hung.item, root: hung.root, objects: &hung.objects, devices: &hung.devices })
    }

    /// The items hung on a body, by slot (for the tests and the log).
    #[cfg(test)]
    pub(super) fn hung_on(&self, body: Body) -> Vec<(&'static str, Id)> {
        self.hung.iter().filter(|hung| hung.body == body).map(|hung| (hung.slot, hung.item)).collect()
    }
}
