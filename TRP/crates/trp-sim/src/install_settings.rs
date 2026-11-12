//! The movement, hands, launcher and empty-hands settings read from the install.

use super::*;

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
    settings.head = Some(trp_data::character::player::HeadCollidersData::load(assets)?);
    Ok(settings)
}

/// Movement settings for another character or another weight, built as
/// `load_movement_settings` builds them but from the root motion table and
/// the head's capsules it read for `read`, the install not read again
/// (the dummies' from the player's: 30 ms at the start); none of `read`'s
/// own changes since (its Strength, its weight's limits) carried over, as
/// `MovementSettings::reweighed` would carry them.
pub fn movement_settings_as_read(read: &MovementSettings, db: &Database, hard: &HardSettings, engine: &EngineSettings, weight: f32) -> Result<MovementSettings> {
    let mut settings = MovementSettings::new(read.table.clone(), hard, &db.config, weight, engine.gravity[1])?;
    settings.steps.read_client(&db.client);
    settings.head = read.head.clone();
    Ok(settings)
}

pub(super) fn number(v: &Json, name: &str) -> Result<f32> {
    v.get(name).and_then(Json::as_f64).map(|v| v as f32).with_context(|| format!("no number {name}"))
}

pub(super) fn pair(v: &Json, name: &str) -> Result<[f32; 2]> {
    let v = v.get(name).with_context(|| format!("no {name}"))?;
    Ok([number(v, "x")?, number(v, "y")?])
}

pub(super) fn triple(v: &Json, name: &str) -> Result<[f32; 3]> {
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
    let stamina = db.config.get("Stamina").ok_or_else(|| anyhow!("globals have no Stamina"))?;
    let template = &db.item(&weapon.template)?.props;
    let recoil = recoil_settings(&data, aiming, template, weapon.recoil_delta)?;
    // With its stock folded the build's recoil is without the stock's share.
    let folded = weapon
        .fold
        .map(|fold| -> Result<weapons::hands::FoldedHold> {
            let folded = recoil_settings(&data, aiming, template, fold.recoil_delta)?;
            Ok(weapons::hands::FoldedHold {
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
        aiming: weapons::hands::AimingConfig {
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
        tactical_stiffness: triple(template, "TacticalReloadStiffnes").unwrap_or([0.0; 3]),
        tactical_fixation: number(template, "TacticalReloadFixation").unwrap_or(0.0),
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
        sights: weapons::sights::Sights::of_weapon(weapon).map(|mut sights| {
            // The zeroing points (`RecalculateOpticCalibrationPoints`): the
            // weapon's default round at the build's speed factor.
            let round = template.get("defAmmo").and_then(Json::as_str).and_then(|id| db.items.get(id)).and_then(combat::ballistics::Round::of);
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
        recoil_force: Some(weapons::hands::RecoilForce {
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
        mount: crate::weapons::mounting::WeaponMount::of_weapon(db, weapon)?,
        mount_settings: crate::weapons::mounting::Settings::from_config(&db.config)?,
        mount_ergonomics: 0.0,
        overweight: 0.0,
        gear_ergonomics: 0.0,
        arms_damaged: [false; 2],
        load_aiming: number(stamina, "AimingSpeedMultiplier")?,
        load_walk: number(stamina, "WalkVisualEffectMultiplier")?,
        data,
    })
}

/// An underbarrel launcher's handling (`hands::WeaponHandling`;
/// underbarrel-launcher spec, "While the launcher is in use"): its ghost
/// weapon template's figures (`Launcher.WeaponTemplate`), with the
/// launcher's own parts' recoil (it has none) and the rifle's stock's
/// (`ShotEffector`: `RecoilDelta` plus `StockRecoilDelta`; a launcher
/// forbids folding, so no folded hold). The camera's snap is the ghost's
/// where it names one, else left as the rifle's (`rifle`).
pub fn launcher_handling(data: &HandsData, db: &Database, ghost: &str, stock_delta: f32, rifle: &weapons::hands::WeaponHandling) -> Result<weapons::hands::WeaponHandling> {
    let aiming = db.config.get("Aiming").ok_or_else(|| anyhow!("globals have no Aiming"))?;
    let template = &db.item(ghost)?.props;
    let recoil = recoil_settings(data, aiming, template, stock_delta)?;
    let or = |name: &str, default: f32| number(template, name).unwrap_or(default);
    let camera_snap = match number(template, "CameraSnap").ok().filter(|snap| *snap > 0.0) {
        Some(snap) => snap * number(aiming, "CameraSnapGlobalMult")?,
        None => rifle.camera_snap,
    };
    Ok(weapons::hands::WeaponHandling {
        recoil,
        recoil_force: Some(weapons::hands::RecoilForce {
            up: number(template, "RecoilForceUp")?,
            back: number(template, "RecoilForceBack")?,
            vert_bonus: number(aiming, "RecoilVertBonus")?,
            back_bonus: number(aiming, "RecoilBackBonus")?,
            multiplier: data.recoil.incoming_strength_multiplier,
            rotation_range: data.recoil.rotation_strength_range,
            position_range: data.recoil.position_strength_range,
            parts: (1.0 + stock_delta).max(0.0),
            folded_parts: None,
        }),
        folded: None,
        camera_snap,
        breath_gain: [or("HipInnaccuracyGain", data.breath.gain_per_shot), or("HipAccuracyRestorationDelay", data.breath.gain_delay), or("HipAccuracyRestorationSpeed", data.breath.gain_speed)],
        tactical_stiffness: triple(template, "TacticalReloadStiffnes").unwrap_or([0.0; 3]),
        tactical_fixation: or("TacticalReloadFixation", 0.0),
        mount: [
            or("MountVerticalRecoilMultiplier", 0.75),
            or("MountHorizontalRecoilMultiplier", 0.6),
            or("MountCameraSnapMultiplier", 1.0),
            or("MountReturnSpeedHandMultiplier", 1.0),
            or("MountingVerticalOutOfBreathMultiplier", 0.8),
            or("MountingHorizontalOutOfBreathMultiplier", 0.4),
        ],
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
    let stamina = db.config.get("Stamina").ok_or_else(|| anyhow!("globals have no Stamina"))?;
    Ok(HandsSettings {
        fixed_timestep: engine.fixed_timestep,
        sway_frequency: hard.vec2("SWAY_FREQ_NORMAL_DAMAGED")?[0],
        sway_damping: hard.vec2("SWAY_DAMPING_NORMAL_DAMAGED")?[0],
        sway_damaged: [hard.vec2("SWAY_FREQ_NORMAL_DAMAGED")?[1], hard.vec2("SWAY_DAMPING_NORMAL_DAMAGED")?[1]],
        sway_strength_per_kg: hard.curve("SWAY_STRENGTH_PER_KG")?.clone(),
        displacement_strength_per_kg: hard.curve("DISPLACEMENT_STRENGTH_PER_KG")?.clone(),
        aiming: weapons::hands::AimingConfig {
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
        // Not a firearm: nothing a tactical reload would stiffen.
        tactical_stiffness: [0.0; 3],
        tactical_fixation: 0.0,
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
        mount: crate::weapons::mounting::WeaponMount::default(),
        mount_settings: crate::weapons::mounting::Settings::from_config(&db.config).unwrap_or_default(),
        mount_ergonomics: 0.0,
        overweight: 0.0,
        gear_ergonomics: 0.0,
        arms_damaged: [false; 2],
        load_aiming: number(stamina, "AimingSpeedMultiplier")?,
        load_walk: number(stamina, "WalkVisualEffectMultiplier")?,
        data,
    })
}
