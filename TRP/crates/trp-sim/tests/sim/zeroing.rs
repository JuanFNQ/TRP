//! Zeroing (`trp_sim::sights`; optics spec, "Zeroing"): the points on the
//! default round's flight, the keys through a scope's distances, the turns
//! toward the point of the optics' cameras, the collimators and the M32A1's
//! part turned by distance, a launcher's line, the shot's squeeze, and what
//! the battle screen is told, on the range's stock builds. Those that need
//! the install pass trivially, with a note, where none is configured.

use trp_sim::ballistics::{Round, Trajectory};
use trp_sim::math::{quat_euler, quat_mul, quat_rotate};
use trp_sim::rig::WeaponRig;
use trp_sim::sights::{zeroing_points, SightHud, Sights};

const SPEAR: &str = "65290f395ae2ae97b80fdf2d";
const G28: &str = "6176aca650224f204c1da3fb";
const G36: &str = "623063e994fc3f7b302a9696";
const M60E6: &str = "661cec09b2c6356b4d0c7a36";
const M32A1: &str = "6275303a9f372d6ea97f9ec7";
const GRAVITY: [f32; 3] = [0.0, -9.81, 0.0];

/// A 5.56 mm round as the item database writes one (M855's figures).
fn m855() -> Round {
    Round {
        initial_speed: 922.0,
        mass_g: 4.0,
        diameter_mm: 5.7,
        ballistic_coefficient: 0.276,
        damage: 49.0,
        penetration_power: 31.0,
        penetration_deviation: 0.0,
        penetration_chance: 0.0,
        ricochet_chance: 0.0,
        fragmentation_chance: 0.0,
        min_fragments: 0,
        max_fragments: 0,
        armor_damage: 0.0,
        life: 2.0,
        accuracy: 0,
        projectiles: 1,
        penetration_damage_mod: 0.0,
        explosive: false,
        stamina_burn: 0.0,
        heavy_bleeding: 0.0,
        light_bleeding: 0.0,
    }
}

fn length(v: [f32; 3]) -> f32 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

/// Each point is a sample of the flight on the millisecond, the first at
/// least its distance off; the list given in the order written, its repeats
/// the same point; the bullet lower and slower the farther.
#[test]
fn zeroing_points_are_the_first_samples_of_the_flight_past_each_distance() {
    let round = m855();
    let factor = 0.98;
    let points = zeroing_points(&[100, 50, 300, 100], &round, factor, GRAVITY);
    assert_eq!(points.len(), 4);
    assert_eq!(points[0], points[3]);
    let mut flight = Trajectory::new([0.0; 3], [0.0, 0.0, round.initial_speed * factor], round.mass_g, round.diameter_mm, round.ballistic_coefficient, GRAVITY);
    for (point, distance) in points.iter().zip([100.0f32, 50.0, 300.0, 100.0]) {
        // The sample's time, found again from the flight: the first
        // millisecond whose place is at least the distance off.
        let mut time = 0.0f32;
        let mut at = [0.0; 3];
        while length(at) < distance {
            at = flight.at(time).0;
            time += 0.001;
        }
        assert_eq!(*point, at, "{distance} m");
        let before = flight.at(time - 0.002).0;
        assert!(length(before) < distance, "{distance} m: the sample before is short of it");
        assert!(length(*point) - distance < 1.0, "{distance} m: within a millisecond's flight");
        assert!(point[1] < 0.0 && point[0] == 0.0, "{distance} m: {point:?}");
    }
    assert!(points[2][1] < points[0][1] && points[0][1] < points[1][1], "the drop grows: {points:?}");
}

/// A weapon's sights with their zeroing points, from the item database and
/// the assembled build; and the build.
fn zeroed(template: &str) -> Option<(Sights, trp_data::weapon::Weapon)> {
    let install = crate::fixture::install()?;
    let db = crate::fixture::database()?;
    let mut assets = trp_data::unity::Assets::new(install).expect("manifest");
    let weapon = trp_data::weapon::assemble(&mut assets, db, template).expect("the weapon");
    let mut sights = Sights::of_weapon(&weapon).expect("its sights");
    let props = &db.items[template].props;
    let round = props.get("defAmmo").and_then(|v| v.as_str()).and_then(|id| db.items.get(id)).and_then(Round::of);
    let launcher = props.get("AdjustCollimatorsToTrajectory").and_then(|v| v.as_bool()).unwrap_or(false);
    sights.set_ballistics(round.as_ref(), weapon.speed_factor, GRAVITY, launcher);
    Some((sights, weapon))
}

/// The weapon's tree at rest with the zeroing's turns on it.
fn turned_rig(sights: &Sights, weapon: &trp_data::weapon::Weapon) -> WeaponRig {
    let mut rig = WeaponRig::new(&weapon.hierarchy);
    for (node, turn) in sights.turns() {
        let place = rig.local(node).0;
        rig.set_local(node, (place, turn));
    }
    rig
}

/// The zeroing point in the tree (`GetWorldCalibrationPoint`), worked out
/// here from the fireport: no cant on these builds.
fn point_in_tree(sights: &Sights, weapon: &trp_data::weapon::Weapon, rig: &WeaponRig) -> [f32; 3] {
    let h = &weapon.hierarchy;
    let fireport = (0..h.nodes.len()).find(|&i| h.nodes[i].name == "fireport").expect("a fireport");
    let (at, turn) = rig.placed(fireport);
    let off = quat_rotate(quat_mul(turn, quat_euler([90.0, 0.0, 0.0])), sights.local_point());
    std::array::from_fn(|k| at[k] + off[k])
}

/// The angle between two ways, in radians, fine near nought (from the
/// cross product, in doubles).
fn angle_between(a: [f32; 3], b: [f32; 3]) -> f32 {
    let (a, b) = (a.map(f64::from), b.map(f64::from));
    let cross = [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    let dot = a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    (cross[0] * cross[0] + cross[1] * cross[1] + cross[2] * cross[2]).sqrt().atan2(dot) as f32
}

/// The keys step the current scope's distances while aiming, held at the
/// ends; each step asks for the calibration, which names the distance.
#[test]
fn the_tango_steps_its_six_distances_and_holds_at_the_ends() {
    let Some((mut sights, _)) = zeroed(SPEAR) else { return };
    assert_eq!(sights.zeroing_distance(), Some(50));
    assert!(!sights.step_zeroing(true, false), "refused unless aiming");
    let mut seen = Vec::new();
    for _ in 0..8 {
        if sights.step_zeroing(true, true) {
            sights.calibrate(true);
            seen.extend(sights.take_hud());
        }
    }
    let named: Vec<SightHud> = [100, 150, 200, 250, 300].into_iter().map(SightHud::Zeroing).collect();
    assert_eq!(seen, named);
    assert_eq!(sights.zeroing_distance(), Some(300));
    for _ in 0..8 {
        sights.step_zeroing(false, true);
    }
    assert_eq!(sights.zeroing_distance(), Some(50));
    assert!(!sights.step_zeroing(false, true), "no wrap below");
}

/// A scope keeps its own index: the G36's carry handle's 3x and its backup
/// (scope 1, one distance); the Specter's 4x and 1x share theirs.
#[test]
fn the_zeroing_is_a_scope_s_not_a_mode_s() {
    let Some((mut g36, _)) = zeroed(G36) else { return };
    g36.step_zeroing(true, true);
    g36.step_zeroing(true, true);
    assert_eq!(g36.zeroing_distance(), Some(150));
    assert!(g36.change_aim_scope(false));
    assert_eq!(g36.zeroing_distance(), Some(25));
    assert!(!g36.step_zeroing(true, true), "one distance");
    assert!(g36.change_aim_scope(false) && g36.change_aim_scope(false));
    assert_eq!(g36.zeroing_distance(), Some(150), "the 3x's again");
    let Some((mut m60, _)) = zeroed(M60E6) else { return };
    m60.step_zeroing(true, true);
    assert_eq!(m60.zeroing_distance(), Some(100));
    assert!(m60.change_magnification());
    assert_eq!(m60.scope_and_mode(), Some((0, 1)));
    assert_eq!(m60.zeroing_distance(), Some(100), "the 1x shares the 4x's");
}

/// The Tango's camera object looks at the point on the round's path, its
/// up toward the fireport's forward; a little down from its prefab's
/// (the scope above the bore); and the shot's squeeze is then off.
#[test]
fn the_optic_camera_looks_at_the_zeroing_point() {
    let Some((mut sights, weapon)) = zeroed(SPEAR) else { return };
    assert!(sights.shot_squeeze());
    sights.aim_changed(true);
    sights.calibrate(true);
    assert!(!sights.shot_squeeze(), "an optic turned");
    assert_eq!(sights.take_hud(), vec![SightHud::Zeroing(50)]);
    let camera = sights.parts()[0].modes[0].optic.as_ref().and_then(|optic| optic.camera).expect("its camera object");
    let rest = WeaponRig::new(&weapon.hierarchy);
    let mut last = None;
    for distance in [50, 100, 300] {
        while sights.zeroing_distance() != Some(distance) {
            assert!(sights.step_zeroing(true, true));
        }
        sights.calibrate(true);
        let rig = turned_rig(&sights, &weapon);
        let point = point_in_tree(&sights, &weapon, &rig);
        let (at, turn) = rig.placed(camera);
        let forward = quat_rotate(turn, [0.0, 0.0, 1.0]);
        let toward: [f32; 3] = std::array::from_fn(|k| point[k] - at[k]);
        assert!(angle_between(forward, toward) < 2e-5, "{distance} m: {}", angle_between(forward, toward));
        assert!((length(toward) - distance as f32).abs() < 2.0, "{distance} m: {}", length(toward));
        let was = quat_rotate(rest.placed(camera).1, [0.0, 0.0, 1.0]);
        let moved = angle_between(forward, was).to_degrees();
        assert!(moved < 0.5, "{distance} m: {moved} degrees from the prefab's");
        eprintln!("the Tango at {distance} m: its camera {moved} degrees from its prefab's");
        assert!(last != Some(turn), "turned again for {distance} m");
        last = Some(turn);
    }
    // Down, the squeeze back on.
    sights.aim_changed(false);
    assert!(sights.shot_squeeze());
}

/// The G28's T1 (first by its item's name): its collimator's axis toward
/// the eye points straight away from the point, the shot's squeeze kept; the
/// S&B after it turns its optic and puts the squeeze off. The game turns
/// them with the weapon squeezed along its root's forward (at the install's
/// 74 degrees, to 0.664): the T1's turn at 25 m differs by so little.
#[test]
fn the_t1_s_dot_lies_on_the_bullet_s_path() {
    let Some((mut sights, weapon)) = zeroed(G28) else { return };
    let t1 = sights.parts().iter().position(|part| part.template == "58d399e486f77442e0016fe7").expect("the T1");
    assert_eq!(sights.current().part, Some(t1));
    assert_eq!(sights.zeroing_distance(), Some(25));
    sights.aim_changed(true);
    sights.calibrate(true);
    assert!(sights.shot_squeeze(), "no optic turned");
    assert_eq!(sights.take_hud(), vec![SightHud::Zeroing(25)]);
    let lens = sights.parts()[t1].modes[0].collimator.expect("its collimator");
    let rig = turned_rig(&sights, &weapon);
    let point = point_in_tree(&sights, &weapon, &rig);
    let (at, turn) = rig.placed(lens);
    let axis = quat_rotate(turn, [0.0, 1.0, 0.0]);
    let away: [f32; 3] = std::array::from_fn(|k| at[k] - point[k]);
    assert!(angle_between(axis, away) < 2e-5, "{}", angle_between(axis, away));
    // The same turn with the tree squeezed along its root's z, the turns not.
    let s = 0.664f32;
    let squeeze = |v: [f32; 3]| [v[0], v[1], v[2] * s];
    let h = &weapon.hierarchy;
    let fireport = (0..h.nodes.len()).find(|&i| h.nodes[i].name == "fireport").expect("a fireport");
    let (port, port_turn) = rig.placed(fireport);
    let barrel = quat_rotate(port_turn, [0.0, -1.0, 0.0]);
    assert!(barrel[2] > 0.99, "the barrel along the root's z: {barrel:?}");
    let off = quat_rotate(quat_mul(port_turn, quat_euler([90.0, 0.0, 0.0])), sights.local_point());
    let squeezed_point: [f32; 3] = std::array::from_fn(|k| squeeze(port)[k] + off[k]);
    let squeezed_away: [f32; 3] = std::array::from_fn(|k| squeeze(at)[k] - squeezed_point[k]);
    let gap = angle_between(away, squeezed_away);
    assert!(gap < 5e-5, "the squeeze's part: {gap} rad");
    eprintln!("the T1 at 25 m: the squeezed world's turn {gap:e} rad from the unsqueezed");
    // The S&B.
    assert!(sights.change_aim_scope(false));
    assert!(sights.optic());
    sights.calibrate(true);
    assert!(!sights.shot_squeeze());
    assert_eq!(sights.take_hud(), vec![SightHud::Zeroing(50)]);
}

/// The M32A1: a launcher, no optic turned; its reflex's part turned by the
/// distance's angle (`weapon_scope_axis`, -87 at 50 m, -68.5 at 400 m) and
/// its line of sight from the collimator to the point, the barrel lifted
/// more the farther.
#[test]
fn the_m32a1_lifts_its_barrel_to_the_distance() {
    let Some((mut sights, weapon)) = zeroed(M32A1) else { return };
    assert!(sights.launcher());
    let part = sights.parts().iter().position(|part| part.angle_by_range.is_some()).expect("its M2A1");
    let axis = sights.parts()[part].angle_by_range.as_ref().expect("turned").axis;
    assert_eq!(weapon.hierarchy.nodes[axis].name, "weapon_scope_axis");
    sights.aim_changed(true);
    sights.calibrate(true);
    let near = sights.launcher_line().expect("a launcher's line").angle;
    let turned: Vec<(usize, [f32; 4])> = sights.turns().collect();
    let (_, turn) = turned.iter().find(|(node, _)| *node == axis).expect("the part turned");
    let x = trp_sim::math::quat_to_euler(*turn)[0];
    assert!((x + 87.0).abs() < 0.01, "{x}");
    while sights.step_zeroing(true, true) {}
    assert_eq!(sights.zeroing_distance(), Some(400));
    sights.calibrate(true);
    let far = sights.launcher_line().expect("a launcher's line").angle;
    let turned: Vec<(usize, [f32; 4])> = sights.turns().collect();
    let (_, turn) = turned.iter().find(|(node, _)| *node == axis).expect("the part turned");
    let x = trp_sim::math::quat_to_euler(*turn)[0];
    assert!((x + 68.5).abs() < 0.01, "{x}");
    assert!(far.abs() > near.abs() + 5.0, "50 m: {near} degrees, 400 m: {far}");
    eprintln!("the M32A1's line of sight: {near} degrees at 50 m, {far} at 400 m");
    assert!(sights.shot_squeeze(), "a launcher turns no optic");
    sights.aim_changed(false);
    assert_eq!(sights.launcher_line().map(|line| line.angle), Some(0.0));
}

/// What the optic's panel is told: a fixed scope's mode with a zoom above
/// 1 in its scope (the Specter's 1x: "x1"), a variable scope at either end
/// of its range ("x6", "x1"), a sight without nothing.
#[test]
fn the_magnification_panel_hears_of_modes_and_ends() {
    let Some((mut m60, _)) = zeroed(M60E6) else { return };
    assert!(m60.change_magnification());
    assert_eq!(m60.take_hud(), vec![SightHud::Magnification(1.0)]);
    assert!(m60.change_magnification());
    assert_eq!(m60.take_hud(), vec![SightHud::Magnification(4.0)]);
    // The backup on top: its item is the Specter, left on its scope 0.
    assert!(m60.change_aim_scope(false));
    assert_eq!(m60.take_hud(), vec![SightHud::Magnification(4.0)]);
    let Some((mut spear, _)) = zeroed(SPEAR) else { return };
    assert!(spear.change_magnification());
    assert_eq!(spear.take_hud(), vec![SightHud::Magnification(6.0)]);
    assert!(spear.change_magnification());
    assert_eq!(spear.take_hud(), vec![SightHud::Magnification(1.0)]);
    // The wheel off the widest hides it.
    spear.zoom_frame(1.0, 1.0 / 120.0, true);
    assert_eq!(spear.take_hud(), vec![SightHud::HideMagnification]);
    let Some((mut g36, _)) = zeroed(G36) else { return };
    assert!(g36.change_aim_scope(false));
    assert_eq!(g36.take_hud(), Vec::new(), "the backup's scope has 1x alone");
    assert!(g36.change_aim_scope(false));
    assert_eq!(g36.take_hud(), Vec::new(), "the red dot");
    assert!(g36.change_aim_scope(false));
    assert_eq!(g36.take_hud(), vec![SightHud::Magnification(3.0)]);
}
