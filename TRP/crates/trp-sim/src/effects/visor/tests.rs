//! Tests of the face shield's mask: its cracks, fade, swing and the shield it goes by.
use super::*;

/// The camera's filter as the install has it (`Cam2`'s
/// `HysteresisFilter`): down after 0.6 s over 0.2 s, up likewise.
fn figures() -> VisorFigures {
    let inf = f32::INFINITY;
    let key = |time, value, in_slope, out_slope| trp_data::formats::curve::Key { time, value, in_slope, out_slope };
    VisorFigures {
        forward: Curve { keys: vec![key(0.0, 0.0, inf, inf), key(0.60171777, 0.0, 0.0, 5.0000005), key(0.80171776, 1.0, 5.0000005, inf)] },
        backward: Curve { keys: vec![key(0.0, 0.0, 3.862559, 3.862559), key(0.19999999, 1.0, 5.0000005, -0.0022513764), key(1.3423328, 0.9974282, inf, inf)] },
        edge_fade: Curve::linear(&[(0.0, 0.1), (1.0, 0.0)]),
        dx: Curve::linear(&[(0.0, 0.0), (0.5, 0.001), (1.0, 0.0)]),
        dy: Curve::linear(&[(0.0, 0.0), (1.0, 0.0)]),
        walk_rotation: Curve::linear(&[(0.0, 0.0), (1.0, 0.0)]),
    }
}

fn shield(id: Id, down: bool) -> Option<Shield> {
    Some(Shield { id, mask: MaskKind::Wide, down, hits: 0, seed: 7 })
}

/// The cracks are drawn anew at every change handed on with a shield
/// (put on, switched, a mark), by its seed and its hits up to 20; a
/// mark on the head's shield while the face cover has one is not heard;
/// taken off, the last drawing stays.
#[test]
fn a_mark_draws_the_cracks_anew() {
    let mut visor = Visor::new(figures());
    assert_eq!(visor.cracks().drawings, 0, "never drawn");
    visor.observe(shield(1, true), None);
    assert_eq!(visor.cracks(), Cracks { seed: 7, count: 0, drawings: 1 });
    visor.observe(shield(1, true), None);
    assert_eq!(visor.cracks().drawings, 1, "nothing changed, nothing drawn");
    let marked = |hits| Some(Shield { hits, ..shield(1, true).unwrap() });
    visor.observe(marked(3), None);
    assert_eq!(visor.cracks(), Cracks { seed: 7, count: 3, drawings: 2 });
    visor.observe(marked(25), None);
    assert_eq!(visor.cracks().count, 20, "at most 20");
    // Raised, drawn again as it was.
    visor.observe(Some(Shield { down: false, ..marked(25).unwrap() }), None);
    assert_eq!(visor.cracks().drawings, 4);
    // The face cover has one: the head's mark is not handed on.
    let welding = Some(Shield { id: 2, mask: MaskKind::Wide, down: true, hits: 1, seed: 9 });
    visor.observe(marked(26), welding);
    assert_eq!(visor.cracks(), Cracks { seed: 7, count: 20, drawings: 4 }, "the face cover's change is not heard while the head has one");
    visor.observe(None, welding);
    visor.observe(None, None);
    assert_eq!(visor.cracks(), Cracks { seed: 7, count: 20, drawings: 4 }, "none keeps the last");
    // The face cover's alone is handed on, by its own seed.
    let mut visor = Visor::new(figures());
    visor.observe(None, welding);
    assert_eq!(visor.cracks(), Cracks { seed: 9, count: 1, drawings: 1 });
}

/// Each crack's quad from Unity's numbers seeded by the shield's seed:
/// x, y, the turn and the size drawn in that order, each `Range` of one
/// number, the quad `x ∓ k/a`, `−y ∓ k`; as many as asked.
#[test]
fn the_cracks_quads_follow_the_seed() {
    let aspect = 16.0 / 9.0;
    let quads = crack_quads(201, 5, aspect);
    assert_eq!(quads.len(), 5);
    let mut random = crate::base::unity_random::UnityRandom::new(201);
    for quad in &quads {
        let t: Vec<f32> = (0..4).map(|_| random.value()).collect();
        let range = |t: f32, min: f32, max: f32| (1.0 - t) * max + t * min;
        let (x, y, turn, k) = (range(t[0], -1.0, 1.0), range(t[1], -1.0, 1.0), range(t[2], 0.0, 360.0), range(t[3], 1.6, 2.6));
        assert_eq!(*quad, CrackQuad { low: [x - k / aspect, -y - k], high: [x + k / aspect, -y + k], rotation: turn });
    }
    for seed in 0..=255u8 {
        for quad in crack_quads(seed, 20, aspect) {
            let (k, middle) = ((quad.high[1] - quad.low[1]) / 2.0, [(quad.low[0] + quad.high[0]) / 2.0, (quad.low[1] + quad.high[1]) / 2.0]);
            assert!((1.6..=2.6).contains(&k) && (0.0..=360.0).contains(&quad.rotation), "{quad:?}");
            assert!(((quad.high[0] - quad.low[0]) / 2.0 * aspect - k).abs() < 1e-5);
            assert!(middle.iter().all(|m| (-1.0..=1.0).contains(m)));
        }
    }
    assert_eq!(crack_quads(201, 3, aspect)[..], quads[..3], "the same seed, the same cracks");
    assert_ne!(crack_quads(202, 1, aspect), quads[..1]);
}

/// Worn down from the start it is on at once; flipped up it holds for
/// 0.6 s and is gone by 0.8 s; down again, nothing for 0.6 s, then on
/// by 0.8 s.
#[test]
fn the_mask_fades_as_the_shield_moves() {
    let mut visor = Visor::new(figures());
    visor.observe(shield(1, true), None);
    assert_eq!(visor.frame(0.0, 0.7, 0.0).intensity, 1.0, "at once as the camera is made");
    visor.observe(shield(1, false), None);
    let at = |visor: &mut Visor, seconds: f32| {
        let mut frame = visor.frame(0.0, 0.7, 0.0);
        for _ in 0..(seconds * 100.0).round() as usize {
            visor.update(0.01);
            frame = visor.frame(0.01, 0.7, 0.0);
        }
        frame.intensity
    };
    assert!(at(&mut visor, 0.5) > 0.99, "held while the hand reaches up");
    assert!(at(&mut visor, 0.35) < 0.01, "up by 0.8 s");
    visor.observe(shield(1, true), None);
    assert!(at(&mut visor, 0.55) < 0.01, "nothing for 0.6 s");
    assert!(at(&mut visor, 0.3) > 0.99, "down by 0.8 s");
    // Taken off, it goes as if flipped up, its mask kept.
    visor.observe(None, None);
    assert!(at(&mut visor, 1.0) < 0.01);
    assert_eq!(visor.frame(0.0, 0.7, 0.0).mask, MaskKind::Wide);
}

/// The face cover's shield counts only while the head has none, and a
/// change of the head's is not heard while the face has one.
#[test]
fn the_head_s_shield_and_the_face_s() {
    let mut visor = Visor::new(figures());
    let welding = Some(Shield { id: 2, mask: MaskKind::Wide, down: true, hits: 0, seed: 1 });
    visor.observe(None, welding);
    assert_eq!(visor.frame(0.0, 0.7, 0.0).intensity, 1.0, "the face's, at once");
    visor.observe(Some(Shield { id: 3, mask: MaskKind::Narrow, down: false, hits: 0, seed: 1 }), welding);
    assert_eq!(visor.frame(0.0, 0.7, 0.0).mask, MaskKind::Wide, "the head's not heard while the face has one");
}

/// Walking, the swing follows the curves on a clock run by the step
/// frequency; standing, it settles back.
#[test]
fn the_mask_swings_with_the_walk() {
    let mut visor = Visor::new(figures());
    visor.observe(shield(1, true), None);
    let mut frame = visor.frame(0.0, 1.0, 1.0);
    for _ in 0..50 {
        frame = visor.frame(0.01, 1.0, 1.0);
    }
    assert!((frame.swing[0] - 0.001).abs() < 1e-5, "{:?}", frame.swing);
    for _ in 0..100 {
        frame = visor.frame(0.01, 1.0, 0.0);
    }
    assert!(frame.swing[0].abs() < 1e-6, "{:?}", frame.swing);
}

/// A switch on the frame the filter has not moved: its clock starts on
/// the next (`courses8`'s visors, the camera's filter and the hinge's
/// alike); turned back a few frames later, the filter has moved on
/// that frame toward the old way first.
#[test]
fn the_filter_moves_before_the_switch_is_heard() {
    let mut visor = Visor::new(figures());
    visor.observe(shield(1, false), None);
    let dt = 1.0 / 120.0;
    visor.update(dt);
    visor.observe(shield(1, true), None);
    assert_eq!(visor.filter().clock(), 0.0, "not moved on the switch's frame");
    for _ in 0..12 {
        visor.update(dt);
        visor.observe(shield(1, true), None);
    }
    visor.update(dt);
    visor.observe(shield(1, false), None);
    assert!((visor.filter().clock() - 13.0 * dt).abs() < 1e-6, "{}", visor.filter().clock());
    assert!(!visor.filter().going_on());
}

/// The hinge's rule: set at once as the thing carrying it is first
/// seen worn, then moved before a switch is heard; let go of while not
/// worn, and set at once again when it is worn anew.
#[test]
fn a_hinge_is_set_at_once_then_moved_before_its_switch() {
    use trp_data::formats::curve::Key;
    let key = |time, value, in_slope, out_slope| Key { time, value, in_slope, out_slope };
    let forward = Curve { keys: vec![key(0.0, 0.0, f32::INFINITY, f32::INFINITY), key(0.6, 0.0, 0.0, 2.5), key(1.0, 1.0, 2.5, f32::INFINITY)] };
    let backward = Curve { keys: vec![key(0.0, 0.0, 2.5, 2.5), key(0.4, 1.0, 2.5, -0.0), key(1.0, 1.0, f32::INFINITY, f32::INFINITY)] };
    let mut hinge = ShieldHinge::new(forward, backward);
    let kind = inventory::Kind { face_shield: true, hinge: true, ..inventory::Kind::default() };
    let mut visor = inventory::Item::new(std::sync::Arc::new(kind));
    assert_eq!(hinge.frame(Some(&visor), 0.01), Some(0.0), "up at once: its switch is off");
    visor.on = true;
    assert_eq!(hinge.frame(Some(&visor), 0.01), None);
    assert_eq!(hinge.filter().clock(), 0.0, "not moved on the switch's frame");
    hinge.frame(Some(&visor), 0.01);
    assert!((hinge.filter().clock() - 0.01).abs() < 1e-7);
    assert_eq!(hinge.frame(None, 0.01), None);
    assert!(!hinge.worn());
    assert_eq!(hinge.frame(Some(&visor), 0.01), Some(1.0), "down at once, worn anew");
}

/// What is heard of the head goes by what is worn there, there and on,
/// as `courses2\6-helmet` showed: a switch is heard, and so is a helmet
/// taken off with its shield down or put on with its night vision on;
/// a shield put on up, or taken off up, is not.
#[test]
fn the_head_is_heard_as_worn() {
    use std::sync::Arc;
    let slot = trp_data::character::inventory::Holder { name: "Headwear".into(), ..Default::default() };
    let slots = vec![trp_data::character::inventory::Holder { name: "mod_equipment".into(), ..Default::default() }];
    let mut equipment = Equipment { item: inventory::Item::new(Arc::new(inventory::Kind { slots: vec![slot], ..inventory::Kind::default() })) };
    let helmet = |part: inventory::Kind| {
        let mut helmet = inventory::Item::new(Arc::new(inventory::Kind { slots: slots.clone(), ..inventory::Kind::default() }));
        helmet.slots[0] = Some(inventory::Item::new(Arc::new(part)));
        helmet
    };
    let shield = helmet(inventory::Kind { face_shield: true, hinge: true, ..inventory::Kind::default() });
    let goggles = helmet(inventory::Kind { class_names: vec!["NightVision".into()], ..inventory::Kind::default() });
    let mut heard = SwitchSounds::default();
    assert!(heard.hear(&equipment).is_empty());
    equipment.item.slots[0] = Some(shield);
    assert!(heard.hear(&equipment).is_empty(), "a shield put on up");
    let worn = equipment.worn("Headwear").unwrap().id;
    toggle_goggles(&mut equipment);
    assert_eq!(heard.hear(&equipment), [0], "FaceShieldOn");
    assert!(heard.hear(&equipment).is_empty(), "heard once");
    assert_eq!(equipment.worn("Headwear").unwrap().id, worn);
    equipment.item.slots[0] = None;
    assert_eq!(heard.hear(&equipment), [1], "FaceShieldOff, taken off down");
    let mut goggles = goggles;
    goggles.slots[0].as_mut().unwrap().on = true;
    equipment.item.slots[0] = Some(goggles);
    assert_eq!(heard.hear(&equipment), [2], "NightVisionOn, put on on");
    toggle_goggles(&mut equipment);
    assert_eq!(heard.hear(&equipment), [3], "NightVisionOff");
    // A recording begun with it on has heard it already.
    toggle_goggles(&mut equipment);
    assert!(SwitchSounds::as_worn(&equipment).hear(&equipment).is_empty());
}
