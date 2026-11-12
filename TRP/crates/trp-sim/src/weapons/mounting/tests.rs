//! Tests of the search for a mount, its approach and a forward mount's data.
use super::*;
use crate::locomotion::movement::Flat;

/// A wall ahead with a top at `height`, its face at z = `face`, and the
/// ground at 0.
struct Sill {
    face: f32,
    height: f32,
    depth: f32,
}

impl World for Sill {
    fn ground_height(&self, _x: f32, _z: f32) -> f32 {
        0.0
    }

    fn ray(&self, from: [f32; 3], direction: [f32; 3], length: f32, _layer: Layer) -> Option<Hit> {
        let block = crate::locomotion::collision::Block::square([-5.0, 0.0, self.face], [5.0, self.height, self.face + self.depth]);
        let to = add(from, scale(direction, length));
        let solid = block.entered_face(from, to).map(|(t, normal)| (t * length, normal));
        let ground = Flat.ray(from, direction, length, Layer::Detailed).map(|h| (h.distance, h.normal));
        let (distance, normal) = solid.into_iter().chain(ground).min_by(|a, b| a.0.total_cmp(&b.0))?;
        Some(Hit { distance, point: add(from, scale(direction, distance)), normal })
    }
}

#[test]
fn a_sill_ahead_is_found_on_the_second_frame() {
    let s = Settings::default();
    let world = Sill { face: 0.6, height: 1.0, depth: 0.3 };
    let grid = Grid { root: [0.0, 1.35, 0.0], yaw: 0.0, feet: 0.0, column_raise: 0.03, row_raise: 0.03 };
    let mut search = Search::standing(&world, &s, grid);
    assert_eq!(search.frame(), None);
    let point = search.frame().expect("answers on the second frame").expect("found");
    assert_eq!(point.side, Side::Forward);
    assert!((point.at[1] - 1.02).abs() < 1e-3, "{point:?}");
    assert!((point.at[2] - 0.6).abs() < 0.02, "{point:?}");
    assert!((point.direction[2] - 1.0).abs() < 1e-4, "faces the wall turned round: {point:?}");
}

#[test]
fn open_ground_answers_nothing_on_the_third_frame() {
    let s = Settings::default();
    let grid = Grid { root: [0.0, 1.35, 0.0], yaw: 30.0, feet: 0.0, column_raise: 0.03, row_raise: 0.03 };
    let mut search = Search::standing(&Flat, &s, grid);
    assert_eq!(search.frame(), None);
    assert_eq!(search.frame(), None);
    assert_eq!(search.frame(), Some(None));
}

#[test]
fn the_approach_is_two_tenths_and_more_by_the_distance() {
    // The M60E6's second `05-mount`: 0.243 m off, 0.36195 s.
    let s = Settings::default();
    let d = 0.2429f32;
    let approach = lerp(s.approach[0], s.approach[1], inverse_lerp(0.0, s.edge, d));
    assert!((approach - 0.3619).abs() < 2e-4, "{approach}");
}

#[test]
fn a_forward_mount_s_data_cuts_the_pitch_to_the_stance() {
    let s = Settings::default();
    let world = Sill { face: 1.0, height: 1.0, depth: 0.3 };
    let weapon = WeaponMount { mountable: true, pivot: [0.0, -0.687, 0.03], ..WeaponMount::default() };
    let point = Point { at: [0.0, 1.02, 1.0], direction: [0.0, 0.0, 1.0], side: Side::Forward };
    let asking = Asking { position: [0.0, 0.0, 0.0], yaw: 0.0, pitch: 0.0, pose_level: 1.0, prone: false, grounded: true, weapon, bipod_out: false, capsule: [0.35, 1.8, 0.3, 0.9] };
    let data = data_for(&world, &s, point, &asking).expect("taken");
    // ±(atan(0.25 / 0.687) - 1): the M700's recorded ±18.98.
    assert!((data.yaw_limit[1] - 18.99).abs() < 0.02, "{data:?}");
    let share = inverse_lerp(0.81, 1.4, 1.02 - 0.03);
    assert!((data.pose_limit[0] - (share - 0.3)).abs() < 1e-4 && (data.pose_limit[1] - (share + 0.3)).abs() < 1e-4, "{data:?}");
    assert!((data.target[2] - (1.0 - 0.687 - 0.074)).abs() < 1e-4, "{data:?}");
}
