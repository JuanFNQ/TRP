//! The hands' rig, and the rigs stowed for each weapon.

use super::*;

/// The weapon-motion simulation for the weapon in hand.
#[derive(Resource)]
pub struct HandsRig {
    pub hands: trp_sim::Hands,
    /// The weapon's trigger, rounds and reloading, with its animator,
    /// when that could be read; and for the rig of the empty hands, their
    /// own controller on their own animator.
    pub firearm: Option<trp_sim::Firearm>,
    pub empty: Option<trp_sim::EmptyHands>,
    /// For the rig of an item used (medicine, food or drink, from its
    /// `UsePrefab`): its controller on its own animator (health spec, "The
    /// hands").
    pub meds: Option<trp_sim::MedsHands>,
    /// For a grenade's rig (its item's `GrenadePrefab`): its controller on
    /// its own animator, and what a throw goes by (grenades-and-knife spec).
    pub grenade: Option<trp_sim::GrenadeHands>,
    /// Its item's template with it, for the flying model to be read while
    /// it is in hand (`loose::set_going`).
    pub thrown: Option<(String, std::sync::Arc<trp_data::weapons::grenade::GrenadeData>)>,
    /// The hand's grenade on the rig (`ThrowingParts[0]`, the first object
    /// of that name), which a throw leaves from (`Controller::grenade_release`).
    pub thrown_bone: Option<usize>,
    /// For a melee weapon's rig: its controller on its own animator, and its
    /// blade's box, which is swept while a stroke is live (`knife::sweep`).
    pub knife: Option<trp_sim::KnifeHands>,
    pub blade: Option<crate::weapons::knife::Blade>,
    /// For the handheld rangefinder's rig: its controller on its own
    /// animator (rangefinder spec).
    pub rangefinder: Option<trp_sim::RangefinderHands>,
    /// The rig with no engine, for what the simulation reads off it.
    pub frame: RigFrame,
    pub aiming: bool,
    /// The weapon's own factor on mouse sensitivity while aiming, where no
    /// sight part gives one (`trp_sim::Hands::aim_sensitivity`).
    pub aim_sensitivity: f32,
    /// The weapon's ergonomic weight, which aiming costs the arms by.
    pub aim_mass: f32,
    /// How many cells wide the body's animator is told the thing is.
    pub width: f32,
    /// How many cells wide the weapon is with its stock folded, if it has
    /// one that folds: what the body's animator is then told.
    pub folded_width: Option<f32>,
    /// Its own sound player's sounds (`hands_sound`), where it has one and
    /// the options' `HANDS SOUNDS` is on.
    pub sounds: Option<std::sync::Arc<crate::audio::hands_sound::RigSounds>>,
    /// The weapon class whose skill a firearm goes by (`skills::Skill::
    /// of_weapon`), if any.
    pub class: Option<trp_sim::condition::skills::Skill>,
    /// What that skill adds to the ergonomics' share mounted and forward on
    /// the bipod (`trp_sim::skills::WeaponBuffs::mount_ergonomics`), as last
    /// taken.
    pub mount_ergonomics: [f32; 2],
    /// The share of a double action's penalty that skill takes off a shot
    /// (`trp_sim::skills::WeaponBuffs::double_action_reduce`), as last
    /// taken (`SyncWithCharacterSkills`).
    pub double_action_reduce: f32,
    /// With an underbarrel launcher fitted, the rifle's handling and the
    /// launcher's ghost weapon's (`trp_sim::hands::WeaponHandling`), taken by
    /// the weapon's motion as the switch ends each way.
    pub launcher_handling: Option<std::sync::Arc<[trp_sim::weapons::hands::WeaponHandling; 2]>>,
}

impl HandsRig {
    /// The animator of whatever this rig is: the weapon's, the empty
    /// hands', or an item's.
    pub fn animator(&self) -> Option<&trp_sim::weapons::weapon_animator::WeaponAnimator> {
        self.firearm
            .as_ref()
            .map(|firearm| firearm.animator())
            .or(self.empty.as_ref().map(|empty| empty.animator()))
            .or(self.meds.as_ref().and_then(|meds| meds.animator()))
            .or(self.grenade.as_ref().map(|grenade| grenade.animator()))
            .or(self.knife.as_ref().map(|knife| knife.animator()))
            .or(self.rangefinder.as_ref().map(|rangefinder| rangefinder.animator()))
    }

    /// The live optic is looked through: aiming, or the rangefinder's
    /// optic switched on (its `SetAiming` turns it on even where the aim is
    /// refused: rangefinder spec).
    pub fn optic_seen(&self) -> bool {
        self.aiming || self.rangefinder.as_ref().is_some_and(|rangefinder| rangefinder.optic_on())
    }

    /// The thing used from a quick key (medicine, food, drink, a grenade
    /// or the quick knife) is still in the hands.
    pub fn using(&self) -> bool {
        self.meds.as_ref().is_some_and(|meds| meds.in_hand())
            || self.grenade.as_ref().is_some_and(|grenade| grenade.in_hand())
            || self.knife.as_ref().is_some_and(|knife| knife.in_hand())
            || self.rangefinder.as_ref().is_some_and(|rangefinder| rangefinder.in_hand())
    }

    /// The thing used gives way to what was asked for next: medicine's work
    /// cut short, a grenade or the knife put away when it may be.
    pub fn give_way(&mut self) {
        if let Some(meds) = self.meds.as_mut() {
            meds.drop();
        }
        if let Some(grenade) = self.grenade.as_mut() {
            grenade.hide();
        }
        if let Some(knife) = self.knife.as_mut() {
            knife.hide();
        }
        if let Some(rangefinder) = self.rangefinder.as_mut() {
            rangefinder.hide();
        }
    }

    /// What is held from a slot (a firearm, or the knife from its sheath)
    /// is drawn, put away, or still in the hands.
    pub fn draw_held(&mut self) {
        if let Some(firearm) = self.firearm.as_mut() {
            firearm.draw();
        }
        if let Some(knife) = self.knife.as_mut() {
            knife.draw();
        }
    }

    pub fn hide_held(&mut self) {
        if let Some(firearm) = self.firearm.as_mut() {
            firearm.hide(false);
        }
        if let Some(knife) = self.knife.as_mut() {
            knife.hide();
        }
    }

    pub fn held_in_hand(&self) -> bool {
        self.firearm.as_ref().is_some_and(|firearm| firearm.in_hand()) || self.knife.as_ref().is_some_and(|knife| knife.in_hand())
    }
}

/// What stands by for the hands: every rig that is not in them just now,
/// each with its clips and its weapon motion. **A weapon's rig is made
/// ready on another thread** (`Preparing`: the crate's weapons from the
/// run's start, one at a time, a weapon hung before its turn first) and
/// put into the world when it is ready (`RigBuilder::finish`; the run's
/// first weapon at the start), and the
/// game's empty hands are a rig like a weapon's (inventory spec, "Empty
/// hands"), so the hands always hold one of them. The resources
/// `WeaponRig` and `HandsRig` are those of whichever is in the hands, and
/// everything that draws or steps the hands goes by them; taking a weapon
/// in hand or putting it away changes one set for another.
#[derive(Resource)]
pub struct Stowed {
    /// The rigs standing by, each with the item template of the weapon it
    /// is, or none for the empty hands.
    pub(super) standing: Vec<(Option<String>, WeaponRig, HandsRig)>,
    /// What is in the hands: a weapon's template, or none for the empty
    /// hands.
    pub(super) held: Option<String>,
    /// The build the weapon in hand's next magazine would make, where it is
    /// of another kind (`inventory_view::settle`): its rig is kept as a
    /// carried one is (`free_rigs`), made ready ahead for a reload.
    pub next_build: Option<String>,
    /// Whether it is the empty hands that stand by, and a weapon that is
    /// in hand.
    pub empty: bool,
    /// The templates of the weapons a rig was built for, whose entities
    /// are in the world: those the hands may take.
    pub built: Vec<String>,
    /// Those whose rig has been put into the world and is not yet all on the
    /// graphics card (`Uploads`): its entities are there from the end of
    /// the frame it was put in, and the hands may take it once the renderer
    /// has its meshes and pictures. (Taken in the same frame, the weapon
    /// was shown before its entities were there, and hidden by its own
    /// making after: floating hands until it was put away and drawn again.)
    pub(super) fresh: Vec<String>,
    /// Those a rig could not be built for (a kind of weapon whose logic
    /// is not written, a model that is not there): not tried again.
    pub(super) failed: Vec<String>,
    /// What a rig built later hangs from and shares with the first: the
    /// view's pivot, the first rig's nodes by their paths (the arms' bones
    /// and the camera's are every rig's), and the arms' solver.
    pub(super) pivot: Entity,
    pub(super) shared: HashMap<String, Entity>,
    pub(super) limbs: Option<[trp_data::character::player::LimbData; 2]>,
    /// The field-of-view setting as it stands, for a rig built later.
    pub(super) field_of_view: f32,
    /// Each rig made on the thread, by its weapon's template: what of it
    /// is its own, to be let go of (`free_rigs`).
    pub(super) own: HashMap<String, OwnOfRig>,
    /// The skills' buffs as they stand, and Aim Drills' share of the
    /// aiming speed, which a weapon takes as it is drawn (`skills`).
    pub(super) skills: trp_sim::condition::skills::Buffs,
    pub(super) aim_speed: f32,
    /// The weapon mastering as it stands, which a weapon takes as it is
    /// drawn (`skills::Mastery`; `SyncWithCharacterSkills`).
    pub(super) mastery: crate::player::skills::Mastery,
}

impl Stowed {
    /// Whether a weapon has a rig, in the world already (or put there this
    /// frame).
    pub fn has_rig(&self, template: &str) -> bool {
        self.built.iter().chain(&self.fresh).any(|built| built == template)
    }

    /// Whether a weapon's rig is still to come: not there yet, and not
    /// one that could not be made.
    pub fn awaited(&self, template: &str) -> bool {
        !self.built.iter().any(|built| built == template) && !self.failed.iter().any(|failed| failed == template)
    }

    /// Changes what is in the hands for the rig of a weapon, or for the
    /// empty hands'. Whether there is such a rig; it is in the hands then
    /// (also if it was already). `combo`: whether a combo unit on the
    /// weapon taken is on (with the tactical devices switched on), which its
    /// hip penalty goes by as its controller is made.
    pub fn change_to(&mut self, what: Option<&str>, rig: &mut WeaponRig, hands: &mut HandsRig, combo: bool) -> bool {
        if self.held.as_deref() == what {
            return true;
        }
        let Some(at) = self.standing.iter().position(|(is, ..)| is.as_deref() == what) else { return false };
        let (_, standing_rig, standing_hands) = &mut self.standing[at];
        std::mem::swap(standing_rig, rig);
        std::mem::swap(standing_hands, hands);
        // The game's motion is one object: what runs runs on through the
        // change (`trp_sim::Hands::carry_on_from`), and hands that hold no
        // firearm keep the one put away's figures, told only to forget the
        // weapon (`trp_sim::Hands::keep_from`; inventory spec, "Empty hands").
        // A firearm's controller made works out its own hip penalty
        // (`trp_sim::Hands::firearm_made`; weapon handling spec, "The hip
        // penalty, each weapon's own").
        hands.hands.carry_on_from(&standing_hands.hands);
        hands.hands.tie_to_new_rig();
        if hands.firearm.is_none() {
            hands.hands.keep_from(standing_hands.hands.kept());
        } else {
            hands.hands.firearm_made(combo);
        }
        self.standing[at].0 = std::mem::replace(&mut self.held, what.map(str::to_owned));
        self.empty = self.held.is_some() && self.standing.iter().any(|(is, ..)| is.is_none());
        // The weapon now drawn takes the skills as they stand, and its
        // family's mastering.
        hands.take_skills(&self.skills, self.aim_speed);
        if let (Some(firearm), Some(key)) = (hands.firearm.as_mut(), what) {
            firearm.set_mastering(self.mastery.level_of(trp_sim::items::inventory::key_template(key)));
        }
        true
    }

    /// The weapon mastering as it now stands, for the weapons drawn from
    /// now on (the one in the hands is put right by `skills::
    /// mastery_apply`).
    pub fn set_mastery(&mut self, mastery: crate::player::skills::Mastery) {
        self.mastery = mastery;
    }

    /// The skills' buffs as they now stand, for the weapons drawn from now
    /// on (the one in the hands keeps those it was drawn with, as the
    /// game's does).
    pub fn set_skills(&mut self, buffs: trp_sim::condition::skills::Buffs, aim_speed: f32) {
        (self.skills, self.aim_speed) = (buffs, aim_speed);
    }

    /// The field-of-view setting has been changed: every rig that stands
    /// by takes it, as the one in the hands has.
    pub fn set_field_of_view(&mut self, degrees: f32) {
        self.field_of_view = degrees;
        for (_, _, hands) in &mut self.standing {
            hands.hands.set_field_of_view(degrees);
        }
    }

    /// The weapon in the hands, by its rig's key (its template, or its
    /// build's: `trp_sim::inventory::rig_key`).
    pub fn held(&self) -> Option<&str> {
        self.held.as_deref()
    }

    /// The hands of a rig that stands by, by its key.
    pub fn hands_of(&self, key: &str) -> Option<&HandsRig> {
        self.standing.iter().find(|(is, ..)| is.as_deref() == Some(key)).map(|(_, _, hands)| hands)
    }

    /// A rig that has just left the hands without being put away (the
    /// weapon in hand built anew, the new build's rig taken in its place):
    /// its weapon put away at once, unseen (`Firearm::put_away_unseen`), so
    /// that taken again it is drawn.
    pub fn put_away(&mut self, key: &str) {
        let standing = self.standing.iter_mut().find(|(is, ..)| is.as_deref() == Some(key));
        if let Some(firearm) = standing.and_then(|(_, _, hands)| hands.firearm.as_mut()) {
            firearm.put_away_unseen();
        }
    }

    /// The rig standing by for a weapon's key.
    pub fn rig_of(&self, key: &str) -> Option<&WeaponRig> {
        self.standing.iter().find(|(is, ..)| is.as_deref() == Some(key)).map(|(_, rig, _)| rig)
    }
}

/// Each bone of `new` with the place of the same bone of `old` (by its
/// path from the animated root), where `old` has it.
pub fn bone_map(old: &WeaponRig, new: &WeaponRig) -> Vec<Option<usize>> {
    let paths = |rig: &WeaponRig| -> Vec<Option<u32>> {
        let of: HashMap<Entity, u32> = rig.by_hash.iter().map(|(hash, entity)| (*entity, *hash)).collect();
        rig.bones.iter().map(|entity| of.get(entity).copied()).collect()
    };
    let old_at: HashMap<u32, usize> = paths(old).into_iter().enumerate().filter_map(|(index, hash)| Some((hash?, index))).collect();
    paths(new).into_iter().map(|hash| hash.and_then(|hash| old_at.get(&hash).copied())).collect()
}

impl Stowed {

}
