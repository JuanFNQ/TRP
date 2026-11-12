//! The recoil and firearm settings read from a weapon and the install.

use super::*;

/// Resolves a weapon's recoil: the prefab's tuning, the template's values
/// and what the fitted parts change. Skills are taken as untrained.
pub(super) fn recoil_settings(data: &HandsData, aiming: &Json, template: &Json, recoil_delta: f32) -> Result<RecoilSettings> {
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
    // Whether that magazine comes out (`ReloadMagType`): the same way.
    let magazine_external = weapon
        .magazine_external
        .or_else(|| {
            let magazine = preset.items.iter().find(|i| i.slot.as_deref() == Some("mod_magazine"))?;
            Some(db.items.get(&magazine.template)?.props.get("ReloadMagType")?.as_str()? == "ExternalMagazine")
        })
        .unwrap_or(false);
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
        magazine_external,
        unique_animation: weapon.unique_animation,
        // The character's, not the weapon's: a host that knows it sets it.
        mastering: 0,
        belt_fed: flag("IsBeltMachineGun"),
        stop_aiming_at: hard.float("STOP_AIMING_AT")?,
        cylinder,
        barrels,
        fast_reload: flag("isFastReload"),
        rocket_launcher: db.item(&weapon.template).ok().is_some_and(|item| db.is_a(item, "RocketLauncher")),
        one_off: flag("IsOneoff"),
        can_unload: template.get("CanUnloadAmmoByPlayer").and_then(Json::as_bool).unwrap_or(true),
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
pub(super) const REST_STATE: &str = "Hands.IDLE";

/// The state a shot plays, and the one that plays the switch of fire mode.
const FIRE_STATE: &str = "Hands.FIRE";

const FIRE_MODE_STATE: &str = "Hands.FIREMODE";

/// The state a self-loading shot plays (`FirearmsAnimator.FullSemiFireStateName`).
const SEMI_FIRE_STATE: &str = "Hands.SA FIRE";
