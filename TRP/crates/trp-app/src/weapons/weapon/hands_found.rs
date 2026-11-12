//! The hands for a weapon, an item or nothing, and their clips.

use super::*;

/// A weapon's motion and logic, from the install: its springs, its
/// recoil, its animator and the operations run on it.
pub(super) fn hands_of(
    unity: &mut trp_data::unity::Assets,
    db: &trp_data::Database,
    hard: &trp_data::setup::hard_settings::HardSettings,
    engine: &trp_data::setup::engine::EngineSettings,
    hands_data: Option<trp_data::character::player::HandsData>,
    weapon: &trp_data::weapons::weapon::Weapon,
    view: trp_sim::ViewSettings,
    mastering: u32,
    launcher: bool,
) -> Option<HandsRig> {
    let Some(data) = hands_data else {
        warn!("no weapon motion data");
        return None;
    };
    let settings = match trp_sim::hands_settings(data, db, hard, engine, weapon, view) {
        Ok(settings) => settings,
        Err(e) => {
            warn!("weapon motion settings of {}: {e:#}", weapon.name);
            return None;
        }
    };
    // Aiming costs the arms by the weapon's ergonomic weight.
    let aim_mass = settings.ergonomic_weight();
    let hands = trp_sim::Hands::new(settings);
    info!("{}: {:.2} kg, ergonomics {}: time to aim {:.2} s", weapon.name, weapon.total_weight, weapon.total_ergonomics, 1.0 / hands.aiming_speed);
    // The weapon's logic goes by the weapon's own animator.
    let animator = trp_sim::weapons::weapon_animator::WeaponAnimatorData::load(unity, weapon);
    let firearm = match (trp_sim::firearm_settings(unity, db, hard, weapon), animator) {
        (Ok(settings), Ok(animator)) => {
            info!(
                "  {} rounds a minute, {} for single shots, magazine of {}; its animator has {} clips",
                settings.fire_rate,
                settings.single_fire_rate,
                settings.magazine_capacity,
                animator.clip_count()
            );
            // The class skill's speeds are an untrained character's until
            // the weapon is drawn, when it takes the skills as they stand
            // (`HandsRig::take_skills`).
            let (reload_speed, draw_speed) = trp_sim::weapons::firearm::weapon_skill_speeds(0);
            let mut firearm = trp_sim::Firearm::new(trp_sim::FirearmSettings { mastering, reload_speed, draw_speed, ..settings }, &animator);
            // Its underbarrel launcher, where the build has one and the
            // options' switch is on (underbarrel-launcher spec); its round is
            // the item's, told at each draw (`HandsRig::launcher_round`).
            match launcher.then(|| trp_sim::weapons::firearm::LauncherData::load(unity, weapon)).transpose() {
                Ok(Some(Some(data))) => {
                    info!("  an underbarrel launcher: {:?}", data.bones());
                    firearm.fit_launcher(&data, false);
                }
                Ok(_) => {}
                Err(e) => warn!("the underbarrel launcher of {}: {e:#}", weapon.name),
            }
            Some(firearm)
        }
        (Err(e), _) | (_, Err(e)) => {
            warn!("weapon logic of {}: {e:#}", weapon.name);
            None
        }
    };
    let aim_sensitivity = db.items.get(&weapon.template).and_then(|t| t.prop_f32("AimSensitivity")).unwrap_or(1.0);
    // With a launcher fitted, the rifle's handling and its ghost weapon's,
    // which the weapon's motion takes at the ends of the switch.
    let launcher_handling = weapon.launcher.as_ref().filter(|_| firearm.as_ref().is_some_and(trp_sim::Firearm::has_launcher)).and_then(|part| {
        let rifle = trp_sim::weapons::hands::WeaponHandling::of(&hands.settings);
        let ghost = part.linked.as_deref()?;
        match trp_sim::launcher_handling(&hands.settings.data, db, ghost, part.stock_recoil_delta, &rifle) {
            Ok(launcher) => Some(std::sync::Arc::new([rifle, launcher])),
            Err(e) => {
                warn!("the launcher's handling of {}: {e:#}", weapon.name);
                None
            }
        }
    });
    Some(HandsRig {
        hands,
        firearm,
        launcher_handling,
        empty: None,
        meds: None,
        grenade: None,
        thrown: None,
        thrown_bone: None,
        knife: None,
        blade: None,
        rangefinder: None,
        frame: RigFrame::new(weapon),
        aiming: false,
        aim_sensitivity,
        aim_mass,
        width: weapon.cell_width as f32,
        folded_width: weapon.fold.map(|fold| fold.cell_width as f32),
        sounds: None,
        class: trp_sim::condition::skills::Skill::of_weapon(&db.class_names(&weapon.template)),
        mount_ergonomics: [0.0; 2],
        double_action_reduce: 0.0,
    })
}

/// The game's empty hands as a rig of the hands' (inventory spec, "Empty
/// hands"), its entities spawned already: their controller on their own
/// animator, and their motion.
#[allow(clippy::too_many_arguments)]
pub(super) fn empty_hands_of(
    unity: &trp_data::unity::Assets,
    db: &trp_data::Database,
    hard: &trp_data::setup::hard_settings::HardSettings,
    engine: &trp_data::setup::engine::EngineSettings,
    hands_data: Option<trp_data::character::player::HandsData>,
    empty: &trp_data::weapons::weapon::Weapon,
    entities: &[Entity],
    by_path: &HashMap<String, Entity>,
    pivot: Entity,
    limbs: Option<[trp_data::character::player::LimbData; 2]>,
    view: trp_sim::ViewSettings,
) -> Option<(WeaponRig, HandsRig)> {
    let animator = match trp_sim::weapons::weapon_animator::WeaponAnimatorData::load(unity, empty) {
        Ok(animator) => animator,
        Err(e) => {
            warn!("the empty hands' animator: {e:#}");
            return None;
        }
    };
    let settings = match trp_sim::empty_hands_settings(hands_data?, db, hard, engine, empty, view) {
        Ok(settings) => settings,
        Err(e) => {
            warn!("the empty hands' motion settings: {e:#}");
            return None;
        }
    };
    info!("empty hands: {} bones, {} clips", empty.hierarchy.nodes.len(), animator.clip_count());
    let rig = rig_of(&empty.hierarchy, entities, by_path, pivot, limbs, empty.cell_width as f32, trp_sim::locomotion::body::hands_kind::EMPTY_HANDS);
    let hands = HandsRig {
        hands: cleared(trp_sim::Hands::new(settings)),
        firearm: None,
        empty: Some(trp_sim::EmptyHands::new(&animator)),
        meds: None,
        grenade: None,
        thrown: None,
        thrown_bone: None,
        knife: None,
        blade: None,
        rangefinder: None,
        frame: RigFrame::new(empty),
        aiming: false,
        aim_sensitivity: 1.0,
        aim_mass: 0.0,
        width: empty.cell_width as f32,
        folded_width: None,
        sounds: None,
        class: None,
        mount_ergonomics: [0.0; 2],
        double_action_reduce: 0.0,
        launcher_handling: None,
    };
    Some((rig, hands))
}

/// The motion of hands that hold no firearm, as a character has it that has
/// held none yet: told to forget the weapon it never had (`ClearPreviousWeapon`:
/// the displacement's strength at 0.5), the prefab's own figures kept. A rig
/// taken in hand after another takes that one's (`Stowed::change_to`).
fn cleared(mut hands: trp_sim::Hands) -> trp_sim::Hands {
    hands.keep_from(hands.kept());
    hands
}

/// The hands that use an item (health spec, "The hands"): its rig from its
/// `UsePrefab`, its controller on the rig's own animator, and its motion,
/// which is the empty hands' (no weight and no recoil of its own:
/// PROVISIONAL).
#[allow(clippy::too_many_arguments)]
pub(super) fn item_hands_of(
    unity: &trp_data::unity::Assets,
    db: &trp_data::Database,
    hard: &trp_data::setup::hard_settings::HardSettings,
    engine: &trp_data::setup::engine::EngineSettings,
    hands_data: Option<trp_data::character::player::HandsData>,
    rig: &trp_data::weapons::weapon::Weapon,
    view: trp_sim::ViewSettings,
    medicine: trp_sim::condition::health::Medicine,
    variants: u32,
) -> Option<HandsRig> {
    let (mut hands, animator) = plain_hands_of(unity, db, hard, engine, hands_data, rig, view)?;
    info!("{}: {variants} variants", rig.name);
    hands.meds = Some(trp_sim::MedsHands::new(Some(&animator), std::sync::Arc::new(medicine), variants));
    Some(hands)
}

/// The hands for a rig that is not a firearm's (an item used, a grenade, a
/// melee weapon): its own animator and the empty hands' motion (no weight
/// and no recoil of its own: PROVISIONAL), with no
/// controller yet; and the animator's data for the controller.
#[allow(clippy::too_many_arguments)]
pub(super) fn plain_hands_of(
    unity: &trp_data::unity::Assets,
    db: &trp_data::Database,
    hard: &trp_data::setup::hard_settings::HardSettings,
    engine: &trp_data::setup::engine::EngineSettings,
    hands_data: Option<trp_data::character::player::HandsData>,
    rig: &trp_data::weapons::weapon::Weapon,
    view: trp_sim::ViewSettings,
) -> Option<(HandsRig, trp_sim::weapons::weapon_animator::WeaponAnimatorData)> {
    let animator = match trp_sim::weapons::weapon_animator::WeaponAnimatorData::load_scaled(unity, rig) {
        Ok(animator) => animator,
        Err(e) => {
            warn!("the animator of {}: {e:#}", rig.name);
            return None;
        }
    };
    let settings = match trp_sim::empty_hands_settings(hands_data?, db, hard, engine, rig, view) {
        Ok(settings) => settings,
        Err(e) => {
            warn!("the motion of {}: {e:#}", rig.name);
            return None;
        }
    };
    info!("{}: {} bones, {} clips; its clips scale {}", rig.name, rig.hierarchy.nodes.len(), animator.clip_count(), animator.scaled_bones().len());
    let hands = HandsRig {
        hands: cleared(trp_sim::Hands::new(settings)),
        firearm: None,
        empty: None,
        meds: None,
        grenade: None,
        thrown: None,
        thrown_bone: None,
        knife: None,
        blade: None,
        rangefinder: None,
        frame: RigFrame::new(rig),
        aiming: false,
        aim_sensitivity: 1.0,
        aim_mass: 0.0,
        width: rig.cell_width as f32,
        folded_width: None,
        sounds: None,
        class: None,
        mount_ergonomics: [0.0; 2],
        double_action_reduce: 0.0,
        launcher_handling: None,
    };
    Some((hands, animator))
}

/// The clips of a weapon or of a rig like one, by the names its animator
/// uses, with the one it rests by: for looking at one clip by its name
/// (`--play`). In play the rig is posed by its animator, not from here.
pub(super) fn clips_of(unity: &trp_data::unity::Assets, weapon: &trp_data::weapons::weapon::Weapon) -> Option<(ClipLibrary, String, Option<(String, [f32; 2])>)> {
    let mut clips: HashMap<String, _> = match weapon.clips(unity) {
        Ok(clips) => clips.into_iter().collect(),
        Err(e) => {
            warn!("animation clips of {}: {e:#}", weapon.name);
            return None;
        }
    };
    let mut idle = String::new();
    match trp_sim::rest_clip(unity, weapon) {
        Ok((name, clip)) => {
            clips.insert(name.clone(), clip);
            idle = name;
        }
        Err(e) => warn!("resting clip of {}: {e:#}", weapon.name),
    }
    let other = trp_sim::rest_blend(unity, weapon).ok().flatten().map(|(name, clip, values)| {
        clips.insert(name.clone(), clip);
        (name, values)
    });
    Some((ClipLibrary { clips }, idle, other))
}
