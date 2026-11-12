//! Tests of vaulting and climbing: what is vaulted, what is climbed and what refuses a move.
use super::*;

/// This install's settings.
pub(crate) fn settings() -> VaultingSettings {
    let limits = |distance, height, length| Limits { distance, height, length };
    VaultingSettings {
        active: true,
        input_time: 0.6,
        grid_size: [0.0, 1.8, 2.0],
        grid_step: [0.1, 0.1, 0.1],
        grid_offset: [0.0; 3],
        offset_factor: 0.1,
        vault: MoveSettings {
            active: true,
            max_without_hand: 0.65,
            max_one_hand: f32::INFINITY,
            speed: [1.25, 0.9],
            limits: limits(0.5, [0.25, 1.1], [0.0, 0.41]),
            auto_limits: limits(0.41, [0.25, 0.41], [0.0, 0.41]),
        },
        climb: MoveSettings {
            active: true,
            max_without_hand: 0.65,
            max_one_hand: 1.2,
            speed: [1.1, 0.8],
            limits: limits(0.5, [0.25, 1.31], [0.0, 10.0]),
            auto_limits: limits(0.41, [0.25, 0.41], [0.41, 10.0]),
        },
    }
}

/// Level ground with boxes on it, each from `near` to `far` ahead
/// (along z), endlessly wide, from `low` to `high`.
pub(crate) struct Boxes(pub(crate) Vec<[f32; 4]>);

impl World for Boxes {
    fn ground_height(&self, _x: f32, _z: f32) -> f32 {
        0.0
    }

    fn line(&self, from: [f32; 3], to: [f32; 3]) -> Option<f32> {
        let length = (0..3).map(|k| (to[k] - from[k]).powi(2)).sum::<f32>().sqrt();
        let mut nearest: Option<f32> = (from[1] >= 0.0 && to[1] < 0.0).then(|| from[1] / (from[1] - to[1]));
        for [near, far, low, high] in &self.0 {
            let (mut enter, mut leave) = (0.0f32, 1.0f32);
            let mut misses = false;
            for (k, (a, b)) in [(2, (*near, *far)), (1, (*low, *high))] {
                let (start, travel) = (from[k], to[k] - from[k]);
                if travel.abs() < 1e-9 {
                    // A line along a face does not enter the box.
                    misses |= start <= a || start >= b;
                    continue;
                }
                let (t0, t1) = ((a - start) / travel, (b - start) / travel);
                enter = enter.max(t0.min(t1));
                leave = leave.min(t0.max(t1));
            }
            if !misses && enter <= leave && enter > 0.0 {
                nearest = Some(nearest.map_or(enter, |n| n.min(enter)));
            }
        }
        nearest.map(|share| share * length)
    }

    fn terrain(&self, at: [f32; 3]) -> bool {
        at[1] < 1e-4
    }
}

const READY: Standing =
    Standing { vaulting: false, backing: false, grounded: true, in_transition: false, jumping: false, sprinting: false, can_vault: true, can_climb: true };

fn measured(boxes: &[[f32; 4]], sprinting: bool) -> Vaulting {
    let mut v = Vaulting::default();
    v.measure(&settings(), &Boxes(boxes.to_vec()), [0.0; 3], 0.0, 0.0, sprinting);
    v
}

#[test]
fn a_thin_wall_is_vaulted_and_a_deep_box_climbed() {
    let s = settings();
    // A wall 0.35 m thick and a metre high, 0.42 m ahead.
    let wall = measured(&[[0.42, 0.77, 0.0, 1.0]], false);
    let o = wall.obstacle;
    println!("wall: height {:.3}, length {:.3}, distance {:.3}, behind {:.3}", o.height, o.length, o.distance, o.behind);
    assert!((o.height - 1.0).abs() < 1e-4 && (o.distance - 0.5).abs() < 1e-4, "{o:?}");
    assert!((o.length - 0.2).abs() < 1e-4, "the rays at 0.5, 0.6 and 0.7 m stand on it: {o:?}");
    assert!((o.behind + 1.0).abs() < 1e-4, "level ground beyond: {o:?}");
    assert_eq!(wall.choose(&s, &READY), Some(Move::Vault));

    // A box a metre deep: too deep to vault, so it is climbed.
    let deep = measured(&[[0.42, 1.42, 0.0, 1.0]], false);
    let o = deep.obstacle;
    println!("box: height {:.3}, length {:.3}, distance {:.3}, behind {:.3}", o.height, o.length, o.distance, o.behind);
    assert!((o.length - 0.9).abs() < 1e-4, "{o:?}");
    assert_eq!(deep.choose(&s, &READY), Some(Move::Climb));

    // A ledge: the ground beyond is the top itself.
    let ledge = measured(&[[0.42, 9.0, 0.0, 1.0]], false);
    assert!((ledge.obstacle.behind - 1.0).abs() < 1e-4, "twice the height less one: {:?}", ledge.obstacle);
    assert_eq!(ledge.choose(&s, &READY), Some(Move::Climb));
}

#[test]
fn what_refuses_a_move() {
    let s = settings();
    let wall = [0.42, 0.77, 0.0, 1.0];
    let stands = |change: fn(&mut Standing)| {
        let mut at = READY;
        change(&mut at);
        at
    };
    let v = measured(&[wall], false);
    assert_eq!(v.choose(&s, &stands(|at| at.vaulting = true)), None);
    assert_eq!(v.choose(&s, &stands(|at| at.backing = true)), None);
    assert_eq!(v.choose(&s, &stands(|at| at.grounded = false)), None);
    assert_eq!(v.choose(&s, &stands(|at| at.in_transition = true)), None);
    assert_eq!(v.choose(&s, &stands(|at| at.jumping = true)), None);
    assert_eq!(v.choose(&s, &stands(|at| at.can_vault = false)), Some(Move::Climb), "legs that cannot pay a vault may pay a climb");
    // Too far, too high, too low.
    assert_eq!(measured(&[[0.52, 0.87, 0.0, 1.0]], false).choose(&s, &READY), None, "0.6 m to the first ray on it");
    assert_eq!(measured(&[[0.42, 0.77, 0.0, 1.2]], false).choose(&s, &READY), Some(Move::Climb), "over a vault's height");
    assert_eq!(measured(&[[0.42, 0.77, 0.0, 1.4]], false).choose(&s, &READY), None, "over a climb's");
    assert_eq!(measured(&[[0.42, 0.77, 0.0, 0.2]], false).choose(&s, &READY), None, "a kerb");
    // Open ground has nothing of a height to take.
    assert_eq!(measured(&[], false).choose(&s, &READY), None);
    // And the same wall as part of the terrain is left alone.
    let mut hill = measured(&[wall], false);
    hill.obstacle.terrain = true;
    assert_eq!(hill.choose(&s, &READY), None);
    // A roof a metre and a half over the top lets a vault under and not a climb.
    let roofed = measured(&[[0.42, 1.42, 0.0, 1.0], [0.0, 3.0, 2.5, 2.6]], false);
    assert_eq!(roofed.obstacle.roof.map(|r| (r * 100.0).round()), Some(150.0));
    assert_eq!(roofed.choose(&s, &READY), None, "too deep to vault, too low a roof to climb");
    // Something standing just beyond the wall, above it: no vault.
    let blocked = measured(&[wall, [1.5, 1.7, 0.0, 1.8]], false);
    assert_eq!(blocked.choose(&s, &READY), Some(Move::Climb));
    // Something before the wall, higher than it.
    let behind_a_post = measured(&[wall, [0.2, 0.3, 0.0, 1.5]], false);
    assert_eq!(behind_a_post.choose(&s, &READY), None);
}

#[test]
fn a_sprint_vaults_only_onto_lower_ground_and_never_climbs() {
    let s = settings();
    let sprinting = Standing { sprinting: true, ..READY };
    let wall = measured(&[[0.42, 0.77, 0.0, 1.0]], true);
    assert_eq!(wall.choose(&s, &sprinting), Some(Move::Vault), "level ground beyond is -1: {:?}", wall.obstacle);
    // Ground 0.6 m up beyond the wall, 2.6 m ahead: -0.33, a vault
    // walking and none sprinting.
    let step = [0.77, 9.0, 0.0, 0.6];
    let raised = measured(&[[0.42, 0.77, 0.0, 1.0], step], true);
    println!("behind in a sprint: {:.3}", raised.obstacle.behind);
    assert_eq!(raised.choose(&s, &sprinting), None);
    assert_eq!(measured(&[[0.42, 1.42, 0.0, 1.0]], true).choose(&s, &sprinting), None, "a sprint does not climb");
}

#[test]
fn the_grid_is_put_ahead_by_the_speed() {
    let s = settings();
    let world = Boxes(vec![[0.92, 1.27, 0.0, 1.0]]);
    let mut v = Vaulting::default();
    // The first measurement is made from the feet and finds the wall
    // too far; it leaves the root 0.5 m ahead for a sprint's 5 m/s.
    v.measure(&s, &world, [0.0; 3], 0.0, 5.0, true);
    assert!((v.obstacle.distance - 1.0).abs() < 1e-4, "{:?}", v.obstacle);
    v.measure(&s, &world, [0.0; 3], 0.0, 5.0, true);
    assert!((v.obstacle.distance - 0.5).abs() < 1e-4, "{:?}", v.obstacle);
    assert_eq!(v.choose(&s, &Standing { sprinting: true, ..READY }), Some(Move::Vault));
}

#[test]
fn stairs_are_not_climbed() {
    // Treads 0.2 m deep rising 0.15 m each, from 0.3 m behind the feet.
    let stairs: Vec<[f32; 4]> = (0..12).map(|i| [-0.3 + 0.2 * i as f32, 9.0, 0.0, 0.15 * (i + 1) as f32]).collect();
    let v = measured(&stairs, false);
    assert!(v.on_stairs());
    assert_eq!(v.choose(&settings(), &READY), None);
    assert!(!measured(&[[0.42, 0.77, 0.0, 1.0]], false).on_stairs());
}
