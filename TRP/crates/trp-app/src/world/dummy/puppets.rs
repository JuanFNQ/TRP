//! The dummies' rigs, puppet makers, sources, colliders, falls and grips.

use super::*;

/// The dummies' skeletons, kept for pieces put on later (`clothing`).
#[derive(Resource)]
pub struct DummySkeletons(pub Vec<crate::screens::figure_view::Skeleton>);

/// A dummy as built (`figure_view::Builder::finish_dummy`): its root and
/// bones by path (a parent before what hangs from it), each bone's place
/// in its parent as it stands, and its outfits' skeletons.
pub struct DummyRig {
    pub root: Entity,
    pub paths: Vec<String>,
    pub bones: Vec<Entity>,
    pub parents: Vec<Option<usize>>,
    pub rest: Vec<Transform>,
    pub outfits: Vec<crate::player::death::CorpseOutfit>,
}

impl DummyRig {
    pub fn bone(&self, name: &str) -> Option<usize> {
        self.paths.iter().position(|path| path.rsplit('/').next() == Some(name))
    }
}

/// Every dummy's, by its number.
#[derive(Resource)]
pub struct DummyRigs(pub Vec<DummyRig>);

/// What a dummy's character is made of, read at the start (`main`): the
/// weapon in its hands, assembled, with its animator, its logic's settings
/// and the motion's data; the movement's settings and the body's animator;
/// the arms' solver; the player's frame rate and the engine's step.
pub struct PuppetMaker {
    pub weapon: Arc<trp_data::weapons::weapon::Weapon>,
    /// The weapon's class, whose skill's buffs it takes (`Skill::
    /// of_weapon`: the AKM's Assault).
    pub class: Option<Skill>,
    /// The options' `WEIGHT`: off, what it carries weighs nothing, as the
    /// player's.
    pub weight_on: bool,
    pub animator: trp_sim::weapons::weapon_animator::WeaponAnimatorData,
    pub firearm: trp_sim::FirearmSettings,
    pub hands_data: HandsData,
    pub view: trp_sim::ViewSettings,
    pub movement: trp_sim::MovementSettings,
    pub body: trp_sim::locomotion::body::BodyData,
    pub limbs: [LimbData; 2],
    pub frame_rate: f32,
    pub physics_step: f32,
    pub longest_frame: f32,
    pub smooth: bool,
    /// The empty hands it holds once its AKM is gone, where they were read.
    pub empty: Option<EmptyMaker>,
}

/// The game's empty hands for a dummy's character (`Puppet::empty_handed`;
/// inventory spec, "Empty hands"): their rig, assembled, and its animator.
pub struct EmptyMaker {
    pub rig: Arc<trp_data::weapons::weapon::Weapon>,
    pub animator: trp_sim::weapons::weapon_animator::WeaponAnimatorData,
}

impl PuppetMaker {
    /// A character standing at `at` (Bevy's axes), facing the firing line,
    /// dressed in `kit` (its body's gear), with the skills `skills`: set up
    /// as the game sets a bot up (body hits spec, "The dummy on the
    /// range"): its kit's weight with encumbrance off (`Player.
    /// SetOwnerToAIData`), the skills' buffs on its movement and weapon,
    /// its gear's speed penalty and share of the ergonomics. With its AKM
    /// in its hands where `holding`, else with the empty hands (made anew:
    /// told to forget a weapon never had, as the player's first empty
    /// hands, `weapon::cleared`), whose class is none.
    pub(super) fn make(&self, game: &crate::GameData, at: Vec3, kit: Option<&trp_sim::items::inventory::Item>, skills: &SkillLevels, holding: bool) -> anyhow::Result<Puppet> {
        let (rig, hands) = if holding {
            (&self.weapon, trp_sim::Hands::new(trp_sim::hands_settings(self.hands_data.clone(), &game.db, &game.hard, &game.engine, &self.weapon, self.view)?))
        } else {
            let empty = self.empty.as_ref().ok_or_else(|| anyhow::anyhow!("the empty hands were not read"))?;
            let mut hands = trp_sim::Hands::new(trp_sim::empty_hands_settings(self.hands_data.clone(), &game.db, &game.hard, &game.engine, &empty.rig, self.view)?);
            hands.keep_from(hands.kept());
            (&empty.rig, hands)
        };
        let width = rig.cell_width as f32;
        let (weight, worn) = self.load_of(kit, skills);
        let settings = self.movement.relimited(&game.hard, &game.db.config, weight, WeightLimits { unencumbered: true, ..WeightLimits::BASE })?;
        let mut movement = trp_sim::Movement::new(settings, trp_sim::Body::new(&self.body, width));
        movement.position = [at.x, at.y, -at.z];
        movement.yaw = FACING;
        movement.body_yaw = FACING;
        movement.note_rotation();
        let pacer = trp_sim::Pacer::new(self.frame_rate, self.physics_step, self.longest_frame);
        let mut puppet = match (holding, self.empty.as_ref()) {
            (false, Some(empty)) => Puppet::empty_handed(movement, hands, trp_sim::EmptyHands::new(&empty.animator), &rig.hierarchy, self.limbs, width, pacer, self.smooth)?,
            _ => Puppet::new(movement, hands, trp_sim::Firearm::new(self.firearm.clone(), &self.animator), &rig.hierarchy, self.limbs, width, pacer, self.smooth)?,
        };
        // (No stimulator's weight buff on it: the carrying factor Strength's
        // lift alone.)
        let buffs = skills.buffs();
        puppet.take_skills(&buffs, if holding { self.class } else { None }, 1.0 + buffs.strength_lift, &game.hard, &game.db.config)?;
        let (penalty, ergonomics) = gear_of(&game.db, &worn, skills);
        puppet.carry_gear(penalty, ergonomics);
        Ok(puppet)
    }

    /// What a body's gear weighs as its character goes by it (the
    /// inventory's sum: inventory spec, "Weight"; nothing with `WEIGHT`
    /// off), and what it wears for its armour.
    pub(super) fn load_of(&self, kit: Option<&trp_sim::items::inventory::Item>, skills: &SkillLevels) -> (f32, Vec<Worn>) {
        let elite = skills.buffs().strength_elite;
        let weight = kit.filter(|_| self.weight_on).map_or(0.0, |kit| trp_sim::items::inventory::carried_weight(kit, elite));
        (weight, kit.map(worn_of).unwrap_or_default())
    }
}

/// What a character's gear takes off its speed (in percent, its vest
/// skills' shares off) and off its weapon's ergonomics (`Player.
/// RecalculateEquipmentParams`; movement spec, "Speed limits").
pub(super) fn gear_of(db: &trp_data::Database, worn: &[Worn], skills: &SkillLevels) -> (f32, f32) {
    (armor::speed_penalty(db, worn, &VestSkills::of(skills)), armor::ergonomics_share(db, worn))
}

/// The health's buffs of the dummies' skills.
pub(super) fn health_skills(skills: &SkillLevels) -> HealthSkills {
    HealthSkills::of_levels(|name| skills.level_of(name))
}

/// What the dummies are made with, read at the start (`main`): the player
/// prefab's hit colliders, the armour's and health's figures, the killing
/// hit's force (`HIT_FORCE`), their characters' makings (none where they
/// could not be read: the dummies then stand as the screens stand a
/// character), and each one's own kit, by its number (a Scav's body of a
/// new PMC's starting kit: `SIDES`).
#[derive(Resource)]
pub struct DummySource {
    pub colliders: Vec<HitColliderData>,
    pub rules: Arc<ArmorRules>,
    pub health: Arc<HealthSettings>,
    pub hit_force: f32,
    pub maker: Option<PuppetMaker>,
    pub kits: Vec<trp_sim::items::inventory::Item>,
    /// Their skills: each at `SKILL_LEVEL` with `SKILLS` on, untrained
    /// with it off.
    pub skills: SkillLevels,
}

/// One of a dummy's hit colliders: as the prefab has it, the bone it is on
/// and its place there (Bevy's axes), what a hit on it is, and its figures
/// as the game sets them.
pub(super) struct Collider {
    pub(super) data: HitColliderData,
    pub(super) bone: usize,
    pub(super) local: Affine3A,
    pub(super) body: BodyRef,
    pub(super) figures: [f32; 6],
}

/// A collider where it stands this frame, in Bevy's axes.
#[derive(Debug, Clone, Copy)]
pub(super) enum Placed {
    Sphere { centre: Vec3, radius: f32 },
    Box { centre: Vec3, axes: [Vec3; 3], half: Vec3 },
    Capsule { a: Vec3, b: Vec3, radius: f32 },
}

/// The killing hit, for the push (`ApplyCorpseImpulse`): the collider it
/// was on, the bullet's way, the point, the penetration as the armour left
/// it. All in the game's axes.
#[derive(Clone, Copy)]
pub(super) struct Killing {
    pub(super) collider: usize,
    pub(super) direction: V3,
    pub(super) point: V3,
    pub(super) penetration: f32,
}

/// A dummy fallen: the game's engine with its ragdoll, each bone's body (or
/// none), the push still to come, and the weapon let go.
pub(super) struct Fall {
    pub(super) engine: World,
    pub(super) moved: Vec<Option<Body>>,
    pub(super) push: Option<(Body, V3, V3)>,
    pub(super) weapon: Option<Body>,
    /// The weapon heard as it lands (`landing_sound`), the time since the
    /// death, and each body's surface (its bone's hit collider's
    /// `SurfaceSound`), which a weapon landing on it plays by.
    pub(super) landing: trp_sim::sound::landing::Landing,
    pub(super) since: f32,
    pub(super) surfaces: HashMap<Body, u8>,
    /// The thing in its hands as it died, by its slot on the body and its
    /// number (`Corpse.ItemInHands`: the weapon stays in its slot), none
    /// where its hands were empty; and whether the weapon's model still
    /// hangs, which goes once that thing leaves its slot
    /// (`clear_dummy_weapons`), and never hung where none was in hand.
    pub(super) in_hands: Option<(usize, trp_sim::items::inventory::Id)>,
    pub(super) hung: bool,
}

/// The AKM's model in a dummy's hands: the frame it hangs in (the feet
/// turned with the view, then the ribcage turned by the view's pitch), the
/// model's nodes, the weapon's own tree (`Weapon_root`), which a death lets
/// go, and its grip.
pub(super) struct Model {
    pub(super) player: Entity,
    pub(super) pivot: Entity,
    pub(super) tree: Vec<Entity>,
    pub(super) weapon_root: Option<Entity>,
    pub(super) grip: Grip,
}

/// What a rig in a dummy's hands is drawn by: its root's own place in the
/// frame it hangs in, and its finger bones with the skeleton's of the same
/// name (the AKM's, or the empty hands', whose rig is not drawn).
pub(super) struct Grip {
    pub(super) root_local: Transform,
    pub(super) fingers: Vec<(usize, usize)>,
}

impl Grip {
    /// A rig's: its root at the frame's origin, and the digits under each
    /// palm of the rig (a grip's own finger pose has bones of the same
    /// names) with the skeleton's bones.
    pub(super) fn of(hierarchy: &trp_data::unity::extract::Hierarchy, rig: &DummyRig) -> Self {
        let root = &hierarchy.nodes[0];
        let mut root_local = convert::transform(root.translation, root.rotation, root.scale);
        root_local.translation = Vec3::ZERO;
        let fingers = hierarchy
            .nodes
            .iter()
            .enumerate()
            .filter(|(index, node)| node.name.starts_with("Base Human") && node.name.contains("Digit") && hierarchy.path(*index).starts_with("Base Human"))
            .filter_map(|(index, node)| Some((index, rig.bone(&node.name)?)))
            .collect();
        Self { root_local, fingers }
    }
}

/// The weapons' slots a dummy's own AKM is taken in hand from, in the
/// order the game's character looks (`TrySetLastEquippedWeapon`: the weapon
/// last in hand wherever it is on the body, else the first that can be).
const HOLDING_SLOTS: [&str; 2] = ["FirstPrimaryWeapon", "SecondPrimaryWeapon"];

/// Where a dummy's own AKM (`own`, its kit's) is on its body, by its slot's
/// place and its number: in either weapon's slot, which its hands take it
/// from; none where it is not on the body (taken, or in a pocket).
pub(super) fn held_of(gear: Option<&trp_sim::items::inventory::Item>, own: Option<trp_sim::items::inventory::Id>) -> Option<(usize, trp_sim::items::inventory::Id)> {
    let (gear, own) = (gear?, own?);
    HOLDING_SLOTS.iter().filter_map(|slot| gear.slot_index(slot)).find(|&slot| in_slot(gear, slot) == Some(own)).map(|slot| (slot, own))
}
