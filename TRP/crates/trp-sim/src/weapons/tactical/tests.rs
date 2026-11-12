//! Tests of tactical devices: the keys, the modes, the hip penalty, the lights and the head light's switch.
use super::*;
use crate::items::inventory::Kind;
use std::sync::Arc;

fn device(class: &str, modes: u32) -> Item {
    Item::new(Arc::new(Kind { light_modes: Some(modes), class_names: vec![class.into()], ..Kind::default() }))
}

#[test]
fn hip_penalty_by_ergonomics_and_combo() {
    // The recorded figures (`courses6`, course 7): the M60E6 FDE's E
    // 4.75 and the M4A1's 45.57, off and with the combo unit on.
    assert!((hip_penalty(4.75, false) - 1.0).abs() < 1e-6);
    assert!((hip_penalty(4.75, true) - 0.288125).abs() < 1e-6);
    assert!((hip_penalty(45.57, false) - 0.96772).abs() < 1e-5);
    assert!((hip_penalty(45.57, true) - 0.186075).abs() < 1e-5);
    assert!((hip_penalty(76.26, false) - 0.84496).abs() < 1e-5);
}

#[test]
fn euler_round_trip_as_unity_orders_it() {
    for angles in [[10.0, 91.0, 0.0], [350.0, 270.5, 2.0], [0.0, 0.0, 0.0], [45.0, 30.0, 20.0]] {
        let back = euler_of(quat_of_euler(angles));
        for (a, b) in angles.iter().zip(back) {
            let d = (a - b).rem_euclid(360.0);
            assert!(d < 1e-3 || d > 360.0 - 1e-3, "{angles:?} came back {back:?}");
        }
    }
    // 90° about y turns forward (+z) to the right (+x).
    let f = turn(quat_of_euler([0.0, 90.0, 0.0]), [0.0, 0.0, 1.0]);
    assert!((f[0] - 1.0).abs() < 1e-5 && f[2].abs() < 1e-5);
    // 10° about x pitches forward down (Unity: x positive is down).
    let f = turn(quat_of_euler([10.0, 0.0, 0.0]), [0.0, 0.0, 1.0]);
    assert!(f[1] < 0.0 && (f[1] + 10f32.to_radians().sin()).abs() < 1e-5);
}

/// The recorded head light (`courses6`, course 7, the M60E6 FDE's
/// `04-head-light`, frame 300): the camera at (0.1204, 1.5166, −0.0973)
/// from the player's origin turned 91.08° (yaw) and −0.576° (pitch) at a
/// field of view of 74, the WF-501B's light on the left at (0.3641,
/// 1.6812, 0.0858) pointing (0.9840, −0.1637, −0.0703). Placed twice: the
/// second time the light moves along its turn of the first, as the game's
/// does along the frame before's.
#[test]
fn the_head_light_sits_where_the_recording_has_it() {
    let camera = quat_of_euler([-0.576, 91.08, 0.0]);
    let (_, first) = head_pose([0.1204, 1.5166, -0.0973], camera, 74.0, false, [0.0, 0.0, 0.0, 1.0]);
    let (at, turned) = head_pose([0.1204, 1.5166, -0.0973], camera, 74.0, false, first);
    for (ours, theirs) in at.iter().zip([0.3641, 1.6812, 0.0858]) {
        assert!((ours - theirs).abs() < 0.004, "{at:?}");
    }
    let way = turn(turned, [0.0, 0.0, 1.0]);
    for (ours, theirs) in way.iter().zip([0.9840, -0.1637, -0.0703]) {
        assert!((ours - theirs).abs() < 0.003, "{way:?}");
    }
}

/// The recorded laser (`courses8\2017-tactical-lights`, frame 400, the
/// X400's red laser on concrete 5.768 m along its ray; its figures as the
/// install has them: rays from 0.1 m, 100 m long, the dot 0.017 to 0.2,
/// its light 0.1 m off, 0.2 m reach, 3.1 strong, its cone 15 to 300).
#[test]
fn a_laser_draws_what_the_recording_has_from_its_hit() {
    let laser = trp_data::weapons::tactical::DeviceLaser {
        node: 0,
        mode: None,
        ray_start: 0.1,
        max_distance: 100.0,
        use_point_light: true,
        mask: 0,
        beam_size: 0.0,
        point_size: [0.017, 0.2],
        surface_offset: 0.1,
        light_range: 0.2,
        light_intensity: 3.1,
        intensity_factor: 1.0,
        angle_close_far: [15.0, 300.0],
        cookie: None,
        beam: None,
        point: None,
        infrared: false,
    };
    let f = laser_figures(&laser, Some(5.767_775_5));
    for (ours, theirs) in [(f.length, 5.867_775_4), (f.brightness, 0.942_322_25), (f.dot_size, 0.027_555_03), (f.light_intensity, 2.921_198_8), (f.light_angle, 31.438_16)] {
        assert!((ours - theirs).abs() < 1e-4, "{f:?}");
    }
    let at = laser_light_at([1.518_852_2, 0.873_952_4, 6.200_519_6], [0.0, -1.192_093e-7, -1.0], [0.208_582_88, -0.074_851_33, 0.975_136_16], laser.surface_offset);
    for (ours, theirs) in at.iter().zip([1.508_358, 0.877_718_7, 6.101_151_5]) {
        assert!((ours - theirs).abs() < 1e-4, "{at:?}");
    }
    // No hit: the whole ray at full brightness, its light dark.
    let none = laser_figures(&laser, None);
    assert_eq!((none.length, none.brightness, none.light_intensity), (100.0, 1.0, 0.0));
}

#[test]
fn modes_wrap_by_their_count() {
    assert_eq!((next_mode(2, 3), next_mode(0, 1), next_mode(1, 4)), (0, 0, 2));
}

#[test]
fn head_light_switches_at_0_8_and_unlocks_at_1_4() {
    let mut head = HeadLight::default();
    assert!(head.press() && head.locked() && !head.press());
    let dt = 1.0 / 120.0;
    let (mut t, mut switched, mut unlocked) = (0.0, None, None);
    while unlocked.is_none() {
        t += dt;
        let step = head.step(dt);
        if step.switched {
            switched = Some(t);
        }
        if step.unlocked {
            unlocked = Some(t);
        }
    }
    let (s, u) = (switched.unwrap(), unlocked.unwrap());
    assert!((0.8..0.8 + dt * 1.01).contains(&s) && (1.4..1.4 + dt * 2.01).contains(&u) && !head.locked());
}

/// A weapon with a combo unit in its slot and a flashlight on the unit's.
fn weapon_with_two() -> (Item, Id, Id) {
    use trp_data::character::inventory::Holder;
    let holder = |name: &str| Holder { name: name.into(), ..Holder::default() };
    let mut combo = Item::new(Arc::new(Kind { slots: vec![holder("mod_flashlight")], light_modes: Some(3), class_names: vec!["TacticalCombo".into()], ..Kind::default() }));
    let flashlight = device("Flashlight", 1);
    let (combo_id, flashlight_id) = (combo.id, flashlight.id);
    combo.slots[0] = Some(flashlight);
    let mut weapon = Item::new(Arc::new(Kind { slots: vec![holder("mod_tactical")], ..Kind::default() }));
    weapon.slots[0] = Some(combo);
    (weapon, combo_id, flashlight_id)
}

#[test]
fn keys_ask_of_every_device_nested_ones_too() {
    let (mut weapon, combo, flashlight) = weapon_with_two();
    assert_eq!(devices(&weapon).iter().map(|d| d.id).collect::<Vec<_>>(), vec![combo, flashlight]);
    // Press mode flips each one on its own: on and off swap.
    apply(&mut weapon, &[(flashlight, Light { on: true, mode: 0 })]);
    let toggle = weapon_request(&weapon, WeaponKey::Toggle).unwrap();
    assert!(toggle.animated && !toggle.forced && !toggle.sound);
    assert_eq!(toggle.states, vec![(combo, Light { on: true, mode: 0 }), (flashlight, Light { on: false, mode: 0 })]);
    // Hold mode: down asks only where not all are on, up only where any is.
    let down = weapon_request(&weapon, WeaponKey::Down).unwrap();
    assert!(!down.animated && down.sound && down.states.iter().all(|(_, l)| l.on));
    apply(&mut weapon, &down.states);
    assert!(weapon_request(&weapon, WeaponKey::Down).is_none() && combo_on(&weapon));
    let up = weapon_request(&weapon, WeaponKey::Up).unwrap();
    assert!(up.forced && !up.animated && up.sound);
    apply(&mut weapon, &up.states);
    assert!(weapon_request(&weapon, WeaponKey::Up).is_none() && !combo_on(&weapon));
    // Ctrl+T: each mode up by its own count, an off one's too.
    let next = weapon_request(&weapon, WeaponKey::Next).unwrap();
    assert_eq!(next.states, vec![(combo, Light { on: false, mode: 1 }), (flashlight, Light { on: false, mode: 0 })]);
    // A thing with none asks nothing.
    assert_eq!(weapon_request(&device("Flashlight", 1), WeaponKey::Toggle), None);
}

#[test]
fn a_flashlight_alone_is_no_combo_unit() {
    let (mut weapon, _, flashlight) = weapon_with_two();
    apply(&mut weapon, &[(flashlight, Light { on: true, mode: 0 })]);
    assert!(!combo_on(&weapon));
}
