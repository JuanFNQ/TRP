//! The character controller as a plain simulation.
//!
//! Nothing here knows about rendering or about any particular engine. A
//! host steps it with input and reads back where the character, the weapon
//! and the camera are.

pub mod ambient;
pub mod animator;
pub mod armor;
pub mod arms;
pub mod ballistics;
pub mod bindings;
pub mod blast;
pub mod blind_fire;
pub mod blood;
pub mod clouds;
pub mod body;
pub mod breath;
pub mod collision;
pub mod decals;
pub mod emitters;
pub mod empty_hands;
pub mod firearm;
pub mod grenade_hands;
pub mod hands;
pub mod hbao;
pub mod health;
pub mod impacts;
pub mod health_replay;
pub mod hysteresis;
pub mod head_look;
pub mod interactions;
pub mod inventory;
pub mod item_menu;
pub mod knife_hands;
pub mod rangefinder;
pub mod landing;
pub mod left_stance;
pub mod magazine_process;
pub mod magazine_view;
pub mod malfunction;
pub mod repair;
pub mod marks;
pub mod meds_hands;
pub mod math;
pub mod mounting;
pub mod movement;
pub mod muzzle;
pub mod pacing;
pub mod popper;
pub mod particles;
pub mod prism;
pub mod quick_draw;
pub mod speaker;
pub mod edit_build;
pub mod item_attributes;
pub mod puppet;
pub mod recoil;
pub mod rig;
pub mod screen_effects;
pub mod script;
pub mod settings;
pub mod shading;
pub mod shells;
pub mod sights;
pub mod skills;
pub mod sparks;
pub mod stamina;
pub mod steps;
pub mod systems;
pub mod tactical;
pub mod things;
pub mod tod;
pub mod unity_random;
pub mod vaulting;
pub mod visor;
pub mod weapon_animator;

pub use body::{ArmPose, Body};
pub use breath::Wind;
pub use empty_hands::EmptyHands;
pub use firearm::{Event as FirearmEvent, FireMode, Firearm, FirearmSettings};
pub use grenade_hands::{GrenadeEvent, GrenadeHands};
pub use knife_hands::{KnifeEvent, KnifeHands, KnifeSettings, Stroke};
pub use rangefinder::{RangefinderEvent, RangefinderHands};
pub use magazine_process::MagazineProcess;
pub use hands::{AimPoint, CameraPose, Carried, Hands, HandsInput, HandsSettings, Held, SightLine};
pub use interactions::Interaction;
pub use meds_hands::{MedsEvent, MedsHands};
pub use movement::{Acted, Flat, Input, Movement, State, Stopwatch, World, WorldInteraction};
pub use pacing::{Pacer, REFERENCE_FRAME_RATE};
pub use settings::MovementSettings;
pub use stamina::{Stamina, StaminaSettings};
pub use systems::Systems;

use anyhow::{anyhow, Context, Result};
use trp_data::curve::{Curve, Key};
use trp_data::engine::EngineSettings;
use trp_data::hard_settings::HardSettings;
use trp_data::player::HandsData;
use trp_data::rootmotion::RootMotionTable;
use trp_data::states::StateTable;
use trp_data::unity::Assets;
use trp_data::weapon::Weapon;
use trp_data::Database;
use serde_json::Value as Json;

use recoil::{CurveRecoil, RecoilSettings, RotationSettings, ShotGroup};

/// Whose eyes a character is seen with: its own, or anyone else's (another
/// player's, a camera's). The game has one class for both and a switch on
/// it (`Player.PointOfView`): a bot, and whoever is not the one playing,
/// is that class seen from outside. What the switch changes is in
/// `docs/spec/third-person.md`; `Body::set_point_of_view` and
/// `Hands::set_point_of_view` are told it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PointOfView {
    #[default]
    FirstPerson,
    ThirdPerson,
}

/// The player's own settings that change how the weapon is shown, with the
/// game's defaults.
#[derive(Debug, Clone, Copy)]
pub struct ViewSettings {
    /// Vertical field of view in degrees, 50 to 75.
    pub field_of_view: f32,
    /// 0.2 to 1.
    pub head_bobbing: f32,
    /// Seed of the recoil's random numbers.
    pub seed: u64,
    /// What the character's skills add to the aiming speed, as a fraction.
    /// Zero for an untrained character.
    pub aim_speed_bonus: f32,
}

impl Default for ViewSettings {
    fn default() -> Self {
        Self { field_of_view: 50.0, head_bobbing: 0.2, seed: 1, aim_speed_bonus: 0.0 }
    }
}

/// Builds the movement settings from an install.
pub fn load_movement_settings(
    assets: &mut Assets,
    db: &Database,
    hard: &HardSettings,
    engine: &EngineSettings,
    weight: f32,
) -> Result<MovementSettings> {
    let table = RootMotionTable::load(assets)?;
    let mut settings = MovementSettings::new(table, hard, &db.config, weight, engine.gravity[1])?;
    // The kinds of moving's multipliers for its sounds are the client's
    // settings', not the globals' (`steps.rs`).
    settings.steps.read_client(&db.client);
    // The head's capsules, from the player prefab (`PlayerOverlapManager`).
    settings.head = Some(trp_data::player::HeadCollidersData::load(assets)?);
    Ok(settings)
}

fn number(v: &Json, name: &str) -> Result<f32> {
    v.get(name).and_then(Json::as_f64).map(|v| v as f32).with_context(|| format!("no number {name}"))
}

fn pair(v: &Json, name: &str) -> Result<[f32; 2]> {
    let v = v.get(name).with_context(|| format!("no {name}"))?;
    Ok([number(v, "x")?, number(v, "y")?])
}

fn triple(v: &Json, name: &str) -> Result<[f32; 3]> {
    let v = v.get(name).with_context(|| format!("no {name}"))?;
    Ok([number(v, "x")?, number(v, "y")?, number(v, "z")?])
}

/// Builds the weapon-motion settings for one weapon.
pub fn hands_settings(
    data: HandsData,
    db: &Database,
    hard: &HardSettings,
    engine: &EngineSettings,
    weapon: &Weapon,
    view: ViewSettings,
) -> Result<HandsSettings> {
    let aiming = db.config.get("Aiming").ok_or_else(|| anyhow!("globals have no Aiming"))?;
    let template = &db.item(&weapon.template)?.props;
    let recoil = recoil_settings(&data, aiming, template, weapon.recoil_delta)?;
    // With its stock folded the build's recoil is without the stock's share.
    let folded = weapon
        .fold
        .map(|fold| -> Result<hands::FoldedHold> {
            let folded = recoil_settings(&data, aiming, template, fold.recoil_delta)?;
            Ok(hands::FoldedHold {
                rotation_strength: folded.rotation_strength,
                position_strength: folded.position_strength,
                held_closer: fold.held_closer,
            })
        })
        .transpose()?;
    let pistol = db.is_a(db.item(&weapon.template)?, "Pistol");
    // A weapon that names its own after-shot camera rate replaces the prefab's.
    let snap = number(template, "CameraSnap").ok().filter(|s| *s > 0.0).unwrap_or(data.camera_smooth_recoil);
    Ok(HandsSettings {
        fixed_timestep: engine.fixed_timestep,
        // The first of each pair is for sound arms and no overweight.
        sway_frequency: hard.vec2("SWAY_FREQ_NORMAL_DAMAGED")?[0],
        sway_damping: hard.vec2("SWAY_DAMPING_NORMAL_DAMAGED")?[0],
        sway_damaged: [hard.vec2("SWAY_FREQ_NORMAL_DAMAGED")?[1], hard.vec2("SWAY_DAMPING_NORMAL_DAMAGED")?[1]],
        sway_strength_per_kg: hard.curve("SWAY_STRENGTH_PER_KG")?.clone(),
        displacement_strength_per_kg: hard.curve("DISPLACEMENT_STRENGTH_PER_KG")?.clone(),
        aiming: hands::AimingConfig {
            light_weight: number(aiming, "LightWeight")?,
            heavy_weight: number(aiming, "HeavyWeight")?,
            max_time_light: number(aiming, "MaxTimeLight")?,
            max_time_heavy: number(aiming, "MaxTimeHeavy")?,
            min_time_light: number(aiming, "MinTimeLight")?,
            min_time_heavy: number(aiming, "MinTimeHeavy")?,
        },
        weapon_weight: weapon.total_weight,
        weapon_ergonomics: weapon.total_ergonomics,
        rotation_center: triple(template, "RotationCenter")?,
        recoil_pivot: triple(template, "RecoilCenter")?,
        hand_damping: number(aiming, "RecoilHandDamping")?,
        aim_intensity: number(aiming, "AimProceduralIntensity")?,
        camera_snap: snap * number(aiming, "CameraSnapGlobalMult")?,
        camera_follow_range: pair(template, "CameraToWeaponAngleSpeedRange")?,
        camera_follow_step: number(template, "CameraToWeaponAngleStep")?,
        recoil,
        intensity_by_pose: triple(aiming, "ProceduralIntensityByPose")?,
        // The weapon's own figures for how hip shots widen the wander.
        breath_gain: [
            number(template, "HipInnaccuracyGain").unwrap_or(data.breath.gain_per_shot),
            number(template, "HipAccuracyRestorationDelay").unwrap_or(data.breath.gain_delay),
            number(template, "HipAccuracyRestorationSpeed").unwrap_or(data.breath.gain_speed),
        ],
        seed: view.seed,
        aim_punch: number(&db.config, "AimPunchMagnitude")?,
        field_of_view: view.field_of_view,
        head_bobbing: view.head_bobbing,
        aim_speed_bonus: view.aim_speed_bonus,
        ergonomics_buff: 0.0,
        aim_plane_depth: aim_plane_depth(weapon)?,
        sights: sights::Sights::of_weapon(weapon).map(|mut sights| {
            // The zeroing points (`RecalculateOpticCalibrationPoints`): the
            // weapon's default round at the build's speed factor.
            let round = template.get("defAmmo").and_then(Json::as_str).and_then(|id| db.items.get(id)).and_then(ballistics::Round::of);
            let launcher = template.get("AdjustCollimatorsToTrajectory").and_then(Json::as_bool).unwrap_or(false);
            sights.set_ballistics(round.as_ref(), weapon.speed_factor, engine.gravity, launcher);
            sights
        }),
        weapon_length: weapon_length(weapon)?,
        weapon_root: weapon_root(weapon)?,
        // What the turn-away from an obstacle goes by (`TurnAway.IsPistol`).
        pistol,
        held_closer: weapon.held_closer,
        rotation_center_no_stock: triple(template, "RotationCenterNoStock").unwrap_or(triple(template, "RotationCenter")?),
        folded,
        recoil_force: Some(hands::RecoilForce {
            up: number(template, "RecoilForceUp")?,
            back: number(template, "RecoilForceBack")?,
            vert_bonus: number(aiming, "RecoilVertBonus")?,
            back_bonus: number(aiming, "RecoilBackBonus")?,
            multiplier: data.recoil.incoming_strength_multiplier,
            rotation_range: data.recoil.rotation_strength_range,
            position_range: data.recoil.position_strength_range,
            parts: (1.0 + weapon.recoil_delta).max(0.0),
            folded_parts: weapon.fold.map(|fold| (1.0 + fold.recoil_delta).max(0.0)),
        }),
        recoil_suppression: 0.0,
        mount: crate::mounting::WeaponMount::of_weapon(db, weapon)?,
        mount_settings: crate::mounting::Settings::from_config(&db.config)?,
        mount_ergonomics: 0.0,
        data,
    })
}

/// Builds the weapon-motion settings for hands that hold no weapon
/// (inventory spec, "Empty hands"). `hands` is the empty hands' own rig.
///
/// The game makes nothing afresh for them: its weapon motion is one object
/// for whatever is in the hands, and taking a weapon out only clears what
/// that weapon was. So what stands here is what a character has that has
/// held no weapon yet: the player prefab's own point for the rig to swing
/// about and its own damping of the position spring, no weight for the
/// sway to go by, no recoil, and no line that feels for obstacles.
/// (After a weapon, the game's empty hands keep that weapon's swing point,
/// damping and ergonomic weight; that carrying over is not built.)
pub fn empty_hands_settings(
    data: HandsData,
    db: &Database,
    hard: &HardSettings,
    engine: &EngineSettings,
    hands: &Weapon,
    view: ViewSettings,
) -> Result<HandsSettings> {
    let aiming = db.config.get("Aiming").ok_or_else(|| anyhow!("globals have no Aiming"))?;
    Ok(HandsSettings {
        fixed_timestep: engine.fixed_timestep,
        sway_frequency: hard.vec2("SWAY_FREQ_NORMAL_DAMAGED")?[0],
        sway_damping: hard.vec2("SWAY_DAMPING_NORMAL_DAMAGED")?[0],
        sway_damaged: [hard.vec2("SWAY_FREQ_NORMAL_DAMAGED")?[1], hard.vec2("SWAY_DAMPING_NORMAL_DAMAGED")?[1]],
        sway_strength_per_kg: hard.curve("SWAY_STRENGTH_PER_KG")?.clone(),
        displacement_strength_per_kg: hard.curve("DISPLACEMENT_STRENGTH_PER_KG")?.clone(),
        aiming: hands::AimingConfig {
            light_weight: number(aiming, "LightWeight")?,
            heavy_weight: number(aiming, "HeavyWeight")?,
            max_time_light: number(aiming, "MaxTimeLight")?,
            max_time_heavy: number(aiming, "MaxTimeHeavy")?,
            min_time_light: number(aiming, "MinTimeLight")?,
            min_time_heavy: number(aiming, "MinTimeHeavy")?,
        },
        weapon_weight: 0.0,
        weapon_ergonomics: 0.0,
        rotation_center: data.rotation_center,
        recoil_pivot: [0.0; 3],
        hand_damping: data.hands_position.damping,
        aim_intensity: number(aiming, "AimProceduralIntensity")?,
        camera_snap: data.camera_smooth_recoil * number(aiming, "CameraSnapGlobalMult")?,
        // Never used: the camera only follows a weapon's recoil.
        camera_follow_range: [1.0; 2],
        camera_follow_step: 0.0,
        recoil: RecoilSettings::of_prefab(&data.recoil),
        intensity_by_pose: triple(aiming, "ProceduralIntensityByPose")?,
        breath_gain: [data.breath.gain_per_shot, data.breath.gain_delay, data.breath.gain_speed],
        seed: view.seed,
        aim_punch: number(&db.config, "AimPunchMagnitude")?,
        field_of_view: view.field_of_view,
        head_bobbing: view.head_bobbing,
        aim_speed_bonus: view.aim_speed_bonus,
        ergonomics_buff: 0.0,
        aim_plane_depth: aim_plane_depth(hands)?,
        sights: None,
        weapon_length: 0.0,
        weapon_root: weapon_root(hands)?,
        pistol: false,
        held_closer: false,
        rotation_center_no_stock: data.rotation_center,
        folded: None,
        recoil_force: None,
        recoil_suppression: 0.0,
        mount: crate::mounting::WeaponMount::default(),
        mount_settings: crate::mounting::Settings::from_config(&db.config).unwrap_or_default(),
        mount_ergonomics: 0.0,
        data,
    })
}

/// The bone the weapon's clips move, which the bone the procedural motion
/// moves hangs from.
const WEAPON_ROOT: &str = "Weapon_root";

/// Where that bone is and how it is turned in the frame the weapon's rig
/// hangs in, at rest.
fn weapon_root(weapon: &Weapon) -> Result<([f32; 3], [f32; 4])> {
    let rig = rig::WeaponRig::new(&weapon.hierarchy);
    let bone = rig.bone(WEAPON_ROOT).context("the weapon has no root bone")?;
    Ok(rig.placed(bone))
}

/// The length of the line the game casts along the weapon to feel for
/// obstacles ("Turn-away" in the weapon-handling spec): from the bone the
/// procedural motion moves to the muzzle, or to the farthest muzzle
/// effect if one lies farther. Of the effects only those deepest in the
/// weapon's tree count, and one named for the extractor's smoke. Measured
/// on the rig at rest; the game measures with the weapon in its resting
/// clip.
pub fn weapon_length(weapon: &Weapon) -> Result<f32> {
    let rig = rig::WeaponRig::new(&weapon.hierarchy);
    weapon_length_on(&rig, &weapon_line_ends(weapon)?).context("the weapon has no moved bone")
}

/// The same length on a rig as its clips have posed it: the game takes it
/// again when the weapon appears in the hands and at the end of every
/// reload, in whatever pose the clip then has the weapon. `ends` are the
/// weapon's points it is measured to (`weapon_line_ends`).
pub fn weapon_length_on(rig: &rig::WeaponRig, ends: &[usize]) -> Option<f32> {
    let from = rig.placed(rig.moved_bone()?).0;
    let reach = |node: usize| {
        let at = rig.placed(node).0;
        (0..3).map(|k| (at[k] - from[k]).powi(2)).sum::<f32>().sqrt()
    };
    ends.iter().map(|&node| reach(node)).reduce(f32::max)
}

/// The length as the game comes by it (weapon-handling spec, "Turn-
/// away", "Measured squeezed"): the greatest distance from the moved
/// bone to an end as the world has them while the wide field of view's
/// squeeze is on the rig, divided by how much the muzzle's own axis is
/// squeezed. Exact only for a weapon lying along the view. `squeeze` is
/// the rig's scale along the view (`Hands::fov_scale`); the muzzle is the
/// last of `ends`.
pub fn weapon_length_squeezed(rig: &rig::WeaponRig, ends: &[usize], squeeze: f32) -> Option<f32> {
    let from = rig.placed(rig.moved_bone()?).0;
    let reach = |node: usize| {
        let at = rig.placed(node).0;
        let across = (at[0] - from[0]).powi(2) + (at[1] - from[1]).powi(2);
        (across + ((at[2] - from[2]) * squeeze).powi(2)).sqrt()
    };
    let length = ends.iter().map(|&node| reach(node)).reduce(f32::max)?;
    // The engine's word for a bone's scale under a squeeze it is turned
    // against: how far its own axis, squeezed, still reaches along itself.
    let axis = math::quat_rotate(rig.placed(*ends.last()?).1, [0.0, 1.0, 0.0]);
    let scale = axis[0] * axis[0] + axis[1] * axis[1] + squeeze * axis[2] * axis[2];
    Some(if scale < 1.0 { length / scale } else { length })
}

/// The points of a weapon its length is measured to: the muzzle, and the
/// muzzle effects that count.
pub fn weapon_line_ends(weapon: &Weapon) -> Result<Vec<usize>> {
    let hierarchy = &weapon.hierarchy;
    let muzzle = (0..hierarchy.nodes.len())
        .find(|&i| hierarchy.nodes[i].name == "fireport")
        .context("the weapon has no muzzle")?;
    // The muzzle effects that count (`UpdateJetsAndFumes`'s choice).
    Ok(trp_data::muzzle::effect_objects(hierarchy).into_iter().chain([muzzle]).collect())
}

/// The clip the weapon's animator plays at rest: the one its resting
/// state's blend tree gives for a weapon with nothing under the barrel.
///
/// That state chooses between two clips by a parameter (`LauncherID`) the
/// game only sets on a weapon that has an underbarrel launcher. Left at
/// the value the controller starts it with, the tree plays the second of
/// them, whose name speaks of a launcher; recorded, the rifle's left-hand
/// marker is that clip's to a tenth of a millimetre and 2.6 cm from the
/// other's.
pub fn rest_clip(assets: &Assets, weapon: &Weapon) -> Result<(String, trp_data::unity::ObjRef)> {
    let table = StateTable::load(assets, weapon)?;
    table.clip_by_default(REST_STATE).cloned().ok_or_else(|| anyhow!("the weapon's animator has no state {REST_STATE}"))
}

/// The second clip of a resting state that blends two by the view's pitch,
/// with the pitch at which the first and the second each play alone. The
/// empty hands rest so (`Aim_angle`, which the game sets from the view's
/// pitch every frame); a weapon whose resting state goes by anything else
/// has none.
pub fn rest_blend(assets: &Assets, weapon: &Weapon) -> Result<Option<(String, trp_data::unity::ObjRef, [f32; 2])>> {
    let table = StateTable::load(assets, weapon)?;
    let Some(state) = table.find(REST_STATE) else { return Ok(None) };
    let (first, _) = table.clip_by_default(REST_STATE).ok_or_else(|| anyhow!("state {REST_STATE} has no clip"))?;
    if state.blend_parameter.as_deref() != Some("Aim_angle") || state.clips.len() != 2 || state.thresholds.len() != 2 {
        return Ok(None);
    }
    let at = |name: &str| state.clips.iter().position(|(clip, _)| clip == name);
    let (Some(first), Some(other)) = (at(first), state.clips.iter().position(|(clip, _)| clip != first)) else { return Ok(None) };
    let (name, object) = state.clips[other].clone();
    Ok(Some((name, object, [state.thresholds[first], state.thresholds[other]])))
}

/// The name of the clip one of the weapon animator's states plays with its
/// parameters left alone, if the animator has the state: for a host that
/// plays a state's clip by name (`Hands.OUT TO IDLE` to draw the weapon,
/// `Hands.IDLE TO OUT` to put it away).
pub fn state_clip(assets: &Assets, weapon: &Weapon, state: &str) -> Result<Option<String>> {
    Ok(StateTable::load(assets, weapon)?.clip_by_default(state).map(|(name, _)| name.clone()))
}

/// The weapon's marker for the right hand, in the axes of the bone the
/// procedural motion moves, as the weapon's resting clip has it: where the
/// weapon is held when the body's hand takes it (`Hands::weapon_transform_carried`).
/// A host that plays the weapon's clips reads the marker off its rig instead.
pub fn right_hand_marker(assets: &Assets, weapon: &Weapon) -> Result<([f32; 3], [f32; 4])> {
    use trp_data::unity::anim::{euler_to_quat, path_hash, Clip, Property};
    const BONE: &str = "Weapon_root/Weapon_root_anim/weapon";
    let hierarchy = &weapon.hierarchy;
    let idle = rest_clip(assets, weapon).ok().map(|(_, clip)| Clip::read(assets, clip)).transpose()?;
    let mut values = Vec::new();
    if let Some(clip) = &idle {
        clip.sample(0.0, &mut values);
    }
    // A bone in its parent: at rest, with whatever the clip moves replaced.
    let local = |path: &str| -> Result<([f32; 3], [f32; 4])> {
        let node = (0..hierarchy.nodes.len())
            .find(|&i| hierarchy.path(i) == path)
            .map(|i| &hierarchy.nodes[i])
            .with_context(|| format!("the weapon has no {path}"))?;
        let (mut position, mut rotation) = (node.translation, node.rotation);
        let hash = path_hash(path);
        for track in idle.iter().flat_map(|c| &c.tracks).filter(|t| t.path == hash) {
            let v = &values[track.curve..];
            match track.property {
                Property::Position => position = [v[0], v[1], v[2]],
                Property::Rotation => rotation = [v[0], v[1], v[2], v[3]],
                Property::Euler => rotation = euler_to_quat([v[0], v[1], v[2]]),
                Property::Scale => {}
            }
        }
        Ok((position, rotation))
    };
    let (bone_position, bone_rotation) = local(BONE)?;
    let (marker_position, marker_rotation) = local(&format!("{BONE}/weapon_R_IK_marker"))?;
    let reach = math::quat_rotate(bone_rotation, marker_position);
    Ok((std::array::from_fn(|i| bone_position[i] + reach[i]), math::quat_mul(bone_rotation, marker_rotation)))
}

/// The nearer of the weapon prefab's two aim planes: its ordinary one,
/// unless the far plane is nearer still.
fn aim_plane_depth(weapon: &Weapon) -> Result<f32> {
    let depth = |name: &str| -> Result<f32> {
        weapon.prefab.req(name)?.req("Depth")?.as_f32().with_context(|| format!("{name} has no depth"))
    };
    Ok(depth("DefaultAimPlane")?.min(depth("FarPlane")?))
}

/// Resolves a weapon's recoil: the prefab's tuning, the template's values
/// and what the fitted parts change. Skills are taken as untrained.
fn recoil_settings(data: &HandsData, aiming: &Json, template: &Json, recoil_delta: f32) -> Result<RecoilSettings> {
    let base = &data.recoil;
    let parts = (1.0 + recoil_delta).max(0.0);
    let strength = |range: [f32; 2], force: &str, bonus: &str| -> Result<[f32; 2]> {
        let scale = (number(template, force)? * parts + number(aiming, bonus)?) * base.incoming_strength_multiplier;
        Ok([range[0] * scale, range[1] * scale])
    };
    let angle = number(template, "RecoilAngle")?;
    let spread = number(template, "RecolDispersion")?;

    let camera = number(template, "RecoilCamera")?;
    let mut shot_vals = base.shot_vals.clone();
    // The fourth and fifth entries are the camera's; the weapon sets them.
    if let Some(v) = shot_vals.get_mut(3) {
        v.intensity = camera;
    }
    if let Some(v) = shot_vals.get_mut(4) {
        v.intensity = -camera;
    }

    let mut groups = Vec::new();
    for g in template.get("ShotsGroupSettings").and_then(Json::as_array).into_iter().flatten() {
        groups.push(ShotGroup {
            start: number(g, "StartShotIndex")? as i32,
            end: number(g, "EndShotIndex")? as i32,
            rotation_strength: pair(g, "ShotRecoilRotationStrength")?,
            position_strength: pair(g, "ShotRecoilPositionStrength")?,
            angle_range: pair(g, "ShotRecoilRadianRange")?,
        });
    }

    let by_pose = |name: &str| triple(aiming, name);
    let (x, y, z) = (by_pose("RecoilXIntensityByPose")?, by_pose("RecoilYIntensityByPose")?, by_pose("RecoilZIntensityByPose")?);
    let z_limit = number(template, "RecoilPosZMult")?;
    let intensity_by_pose = std::array::from_fn(|pose| [x[pose], y[pose], z[pose].min(z_limit)]);

    let mut curves = Vec::new();
    let own = template.get("WeaponRecoilSettings");
    if own.and_then(|w| w.get("Enable")).and_then(Json::as_bool) == Some(true) {
        for value in own.and_then(|w| w.get("Values")).and_then(Json::as_array).into_iter().flatten() {
            if value.get("Enable").and_then(Json::as_bool) != Some(true) {
                continue;
            }
            let process = value.get("Process").context("recoil curve has no Process")?;
            let keys = process
                .get("TransformationCurve")
                .and_then(|c| c.get("Keys"))
                .and_then(Json::as_array)
                .context("recoil curve has no keys")?;
            let keys = keys
                .iter()
                .map(|k| {
                    Ok(Key {
                        time: number(k, "time")?,
                        value: number(k, "value")?,
                        in_slope: number(k, "inTangent")?,
                        out_slope: number(k, "outTangent")?,
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            curves.push(CurveRecoil {
                target: match value.get("Target").and_then(Json::as_str) {
                    Some("CameraRotation") => 0,
                    Some("HandsPosition") => 1,
                    Some("HandsRotation") => 2,
                    other => return Err(anyhow!("recoil curve targets {other:?}")),
                },
                axis: match process.get("ComponentType").and_then(Json::as_str) {
                    Some("X") => 0,
                    Some("Y") => 1,
                    _ => 2,
                },
                time_multiply: number(process, "CurveTimeMultiply")?,
                value_multiply: number(process, "CurveValueMultiply")?,
                aiming_value_multiply: number(process, "CurveAimingValueMultiply")?,
                curve: Curve { keys },
            });
        }
    }

    Ok(RecoilSettings {
        progress_power_on: base.progress_power_on,
        progress_power_shot: base.progress_power_shot,
        stable_shot_index: number(template, "RecoilStableIndexShot")? as i32,
        shot_vals,
        rotation_strength: strength(base.rotation_strength_range, "RecoilForceUp", "RecoilVertBonus")?,
        position_strength: strength(base.position_strength_range, "RecoilForceBack", "RecoilBackBonus")?,
        angle_range: [(angle - spread).to_radians(), (angle + spread).to_radians()],
        groups,
        intensity_by_pose,
        aim_intensity: number(aiming, "AimProceduralIntensity")?,
        rotation: RotationSettings {
            return_speed: number(template, "RecoilReturnSpeedHandRotation")? * number(aiming, "RecoilConvergenceMult")?,
            damping: number(template, "RecoilDampingHandRotation")?,
            category_multiplier: number(template, "RecoilCategoryMultiplierHandRotation")?,
            return_path_offset: number(template, "RecoilReturnPathOffsetHandRotation")?,
            return_path_damping: number(template, "RecoilReturnPathDampingHandRotation")?,
            stable_angle_increase_step: number(template, "RecoilStableAngleIncreaseStep")?,
            progress_angle_on_stable: pair(template, "ProgressRecoilAngleOnStable")?,
            after_recoil_vertical_range: pair(template, "PostRecoilVerticalRangeHandRotation")?,
            after_recoil_horizontal_range: pair(template, "PostRecoilHorizontalRangeHandRotation")?,
            reset_after_shot: template.get("ResetAfterShot").and_then(Json::as_bool).unwrap_or(false),
            to_next_stable_point_distance_limit: base.to_next_stable_point_distance_limit,
            next_stable_point_distance_range: base.next_stable_point_distance_range,
            to_next_current_speed: base.to_next_current_speed,
            return_speed_curve: base.return_speed_curve.clone(),
        },
        hand_position: base.hand_position,
        camera_rotation: base.camera_rotation,
        curves,
    })
}

/// Builds the weapon logic's settings: rates and rounds from the database.
/// What the logic waits on, the states of the weapon's animator and the
/// marks in them, is the animator's own (`weapon_animator::WeaponAnimatorData`).
pub fn firearm_settings(_assets: &Assets, db: &Database, hard: &HardSettings, weapon: &Weapon) -> Result<FirearmSettings> {
    let template = &db.item(&weapon.template)?.props;
    let preset = db.default_preset(&weapon.template)?;
    // Which of a state's clips plays depends on the magazine fitted
    // (`magAnimationIndex`): the build's, else its stock preset's.
    let magazine_variant = weapon
        .magazine_animation
        .or_else(|| {
            let magazine = preset.items.iter().find(|i| i.slot.as_deref() == Some("mod_magazine"))?;
            db.items.get(&magazine.template)?.props.get("magAnimationIndex")?.as_i64().map(|n| n as i32)
        })
        .unwrap_or(0);
    let modes: Vec<FireMode> = template
        .get("weapFireType")
        .and_then(Json::as_array)
        .into_iter()
        .flatten()
        .filter_map(|m| match m.as_str()? {
            "single" => Some(FireMode::Single),
            "fullauto" => Some(FireMode::FullAuto),
            "burst" => Some(FireMode::Burst),
            "semiauto" => Some(FireMode::SemiAuto),
            "doubleaction" => Some(FireMode::DoubleAction),
            "doublet" => Some(FireMode::Doublet),
            _ => None,
        })
        .collect();
    // A cylinder (the game's `Revolver`): its chambers are the slots of
    // the magazine fitted that are named for them, and a spring turns the
    // launcher's. Barrels loaded one by one are the weapon's own chambers.
    let fitted = preset.items.iter().find(|i| i.slot.as_deref() == Some("mod_magazine")).and_then(|i| db.items.get(&i.template));
    let cylinder = db.item(&weapon.template).ok().filter(|item| db.is_a(item, "Revolver")).and_then(|_| {
        let fitted = fitted?;
        let chambers = fitted.props.get("Slots")?.as_array()?.iter().filter(|slot| slot.get("_name").and_then(Json::as_str).is_some_and(|name| name.starts_with("camora_"))).count();
        (chambers > 0).then_some((chambers as u32, db.is_a(fitted, "SpringDrivenCylinder")))
    });
    let barrels = match template.get("ReloadMode").and_then(Json::as_str) {
        Some("OnlyBarrel") => template.get("Chambers").and_then(Json::as_array).map_or(0, |chambers| chambers.len() as u32),
        _ => 0,
    };
    let ammo = template.get("defAmmo").and_then(Json::as_str).and_then(|id| db.items.get(id));
    let ammo_recoil = ammo.and_then(|a| a.prop_f32("ammoRec")).unwrap_or(0.0);
    let flag = |name: &str| template.get(name).and_then(Json::as_bool).unwrap_or(false);
    Ok(FirearmSettings {
        fire_rate: number(template, "bFirerate")?,
        single_fire_rate: number(template, "SingleFireRate")?,
        modes,
        burst_count: number(template, "BurstShotsCount").unwrap_or(3.0) as u32,
        magazine_capacity: cylinder.map(|(chambers, _)| chambers).or((barrels > 0).then_some(barrels)).unwrap_or(weapon.magazine_capacity.unwrap_or(0)),
        bolt_catch: template.get("isBoltCatch").and_then(Json::as_bool).unwrap_or(false),
        can_queue_second_shot: template.get("CanQueueSecondShot").and_then(Json::as_bool).unwrap_or(false),
        shot_strength: 1.0 + ammo_recoil / 100.0,
        rest_state: REST_STATE.to_owned(),
        fire_state: FIRE_STATE.to_owned(),
        fire_mode_state: FIRE_MODE_STATE.to_owned(),
        semi_fire_state: SEMI_FIRE_STATE.to_owned(),
        // Barrels are loaded with loose rounds as a tube is.
        internal_magazine: template.get("ReloadMode").and_then(Json::as_str) == Some("InternalMagazine") || barrels > 0,
        no_fire_mode_on_bolt_catch: flag("NoFiremodeOnBoltcatch"),
        bolt_action: flag("BoltAction"),
        open_bolt_reload: flag("MustBoltBeOpennedForInternalReload"),
        must_open_bolt: flag("MustBoltBeOpennedForInternalReload") || flag("MustBoltBeOpennedForExternalReload"),
        manual_bolt_catch: flag("ManualBoltCatch"),
        chamber_load: template.get("isChamberLoad").and_then(Json::as_bool).unwrap_or(true),
        foldable: weapon.fold.is_some(),
        // The character's, not the weapon's: a host that knows its skill
        // with this class of weapon sets them (`weapon_skill_speeds`).
        reload_speed: 1.0,
        draw_speed: 1.0,
        magazine_variant,
        // The character's, not the weapon's: a host that knows it sets it.
        mastering: 0,
        stop_aiming_at: hard.float("STOP_AIMING_AT")?,
        cylinder,
        barrels,
        fast_reload: flag("isFastReload"),
        rocket_launcher: db.item(&weapon.template).ok().is_some_and(|item| db.is_a(item, "RocketLauncher")),
        // No chamber at all: none listed, a detachable magazine, and not a
        // cylinder's (`Weapon.HasChambers`).
        no_chamber: template.get("Chambers").and_then(Json::as_array).is_some_and(|chambers| chambers.is_empty())
            && template.get("ReloadMode").and_then(Json::as_str) == Some("ExternalMagazine")
            && cylinder.is_none(),
        mountable: flag("IsMountable"),
        bipod: weapon.bipod.as_ref().map(|bipod| bipod.forward),
    })
}

/// The weapon animator's state between actions.
const REST_STATE: &str = "Hands.IDLE";
/// The state a shot plays, and the one that plays the switch of fire mode.
const FIRE_STATE: &str = "Hands.FIRE";
const FIRE_MODE_STATE: &str = "Hands.FIREMODE";
/// The state a self-loading shot plays (`FirearmsAnimator.FullSemiFireStateName`).
const SEMI_FIRE_STATE: &str = "Hands.SA FIRE";
