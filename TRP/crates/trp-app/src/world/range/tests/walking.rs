//! Tests of walking the range: the open start, walls, boxes, the platform and the window.

use super::*;

/// The open start has 40 m of floor clear ahead, for a long sprint.
#[test]
fn the_open_start_has_forty_metres_clear() {
    let range = Range::default();
    let at = range.place("open").unwrap();
    let [yaw, _] = range.start_view("open").unwrap();
    assert!(clear_ahead(&range, at, yaw) >= 300.0, "{}", clear_ahead(&range, at, yaw));
}

/// A box set into the ground is lifted out of it, and one set into the
/// range's wall is moved out of the wall by the shortest way, each by
/// the margin more; a box clear of everything is left alone.
#[test]
fn a_box_is_pushed_out_of_the_ground_and_the_wall() {
    let range = Range::default();
    let (half, level, margin) = ([0.065, 0.115, 0.115], [0.0, 0.0, 0.0, 1.0], 0.001);
    assert_eq!(range.push_out([0.0, 1.5, 0.0], half, level, margin), [0.0; 3]);
    let lifted = range.push_out([0.0, 0.1, 0.0], half, level, margin);
    assert!((lifted[1] - 0.016).abs() < 1e-5 && lifted[0] == 0.0 && lifted[2] == 0.0, "{lifted:?}");
    // Tipped a quarter turn about the view, its width stands upright.
    let tipped = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
    let lifted = range.push_out([0.0, 0.1, 0.0], half, tipped.to_array(), margin);
    assert!(lifted[1].abs() < 1e-5, "{lifted:?}");
    // The door wall's face on the start's side is 5.8 m behind the
    // start (-5.8 in the simulation's axes); the box's back is 3 cm into
    // it, to the window's right.
    let out = range.push_out([3.5, 1.5, -(DOOR_WALL_Z - WALL_THICK / 2.0) + 0.115 - 0.03], half, level, margin);
    assert!((out[2] - 0.031).abs() < 1e-5 && out[0].abs() < 1e-6 && out[1].abs() < 1e-6, "{out:?}");
}

/// The stairs are ridden up at a walk's pace by the game's own rules
/// and the platform's side is a wall; off its edge there is nothing
/// under the feet, and a character that finds itself against the
/// platform's face on its way down is moved out of it a twentieth of
/// a metre a move.
#[test]
fn the_platform_is_climbed_by_its_stairs_and_fallen_from() {
    let range = Range::default();
    let height = Capsule { underside: 0.0, top: 1.6, lift: true };
    // As the simulation moves: the motion, what stands in the way,
    // then the ground under where that leaves the feet. A walk's
    // pace at 120 frames a second.
    let walk = |mut at: [f32; 3], step: [f32; 2], until: &dyn Fn([f32; 3]) -> bool| {
        for frame in 0..4000 {
            if until(at) {
                return (at, frame);
            }
            let to = range.obstructed(at, [at[0] + step[0], at[1], at[2] + step[1]], height);
            at = [to[0], to[1].max(range.ground_height(to[0], to[2])), to[2]];
        }
        (at, 4000)
    };
    let start = range.place("stairs").unwrap();
    assert_eq!(start[1], 0.0);
    // Where the stairs end, along the simulation's z.
    let top = STAIRS as f32 * TREAD - STAIRS_FOOT;
    let (at, frames) = walk(start, [0.0, 0.0235], &|at| at[2] > top + 1.0);
    let level = ((top + 1.0 - start[2]) / 0.0235).ceil();
    println!("the stairs: {frames} frames to the platform, {level} on level ground");
    assert!((at[1] - PLATFORM).abs() < 1e-4 && (frames as f32) < level * 1.25, "{at:?} after {frames} frames");
    // From the ground, the lower platform's right side stops the
    // character its radius away, and it stays on the ground.
    let (stopped, _) = walk([STAIRS_LEFT + 12.0, 0.0, top + 2.5], [-0.0235, 0.0], &|at| at[0] < STAIRS_LEFT + 7.0);
    assert!((stopped[0] - (STAIRS_LEFT + 8.0 + range.radius)).abs() < 1e-4 && stopped[1] == 0.0, "{stopped:?}");
    // Off the far edge, on the half the second flight does not go up:
    // nothing under the feet.
    let (edge, middle) = (top + 5.0, STAIRS_LEFT + 3.75);
    assert_eq!(range.ground_height(middle, edge - 0.01), PLATFORM);
    assert_eq!(range.ground_height(middle, edge + 0.01), 0.0);
    // Falling with its axis a centimetre past the edge, the capsule
    // is in the platform's face: out of it 5 cm a move, and down.
    let falling = range.obstructed([middle, 2.0, edge + 0.01], [middle, 1.99, edge + 0.01], height);
    assert!((falling[2] - (edge + 0.06)).abs() < 2e-3 && falling[1] < 2.0 && falling[0] == middle, "{falling:?}");
    // And it cannot walk back in under the platform's top.
    let (back, _) = walk([middle, 0.0, edge + 1.0], [0.0, -0.0235], &|at| at[2] < edge);
    assert!((back[2] - (edge + range.radius)).abs() < 1e-4, "{back:?}");
    // From the platform, the second flight climbs to the ledge, 6 m up
    // (health spec, "Falls"), on the way the first goes.
    let across = STAIRS_LEFT + 1.25;
    let ledge = top + 5.0 + LEDGE_STEPS as f32 * TREAD;
    let (up, frames) = walk([across, PLATFORM, top + 4.0], [0.0, 0.0235], &|at| at[2] > ledge + 1.0);
    println!("the second flight: {frames} frames to the ledge");
    assert!((up[1] - LEDGE).abs() < 1e-4, "{up:?} after {frames} frames");
    assert_eq!(range.place("ledge").map(|at| at[1]), Some(LEDGE));
}

/// The knee-high wall is not walked onto: the capsule's foot meets
/// its rim too low to be pushed up over it.
#[test]
fn the_low_wall_stops_a_walk() {
    let range = Range::default();
    let height = Capsule { underside: 0.0, top: 1.6, lift: true };
    // Ahead of the start, past the tables of weapons to the right of the
    // workbench (along the rest of the wall, the tables stand).
    let mut at = [8.0, 0.0, 2.0];
    for _ in 0..300 {
        let to = range.obstructed(at, [at[0], at[1], at[2] + 0.0235], height);
        at = [to[0], to[1].max(range.ground_height(to[0], to[2])), to[2]];
    }
    let face = -(KNEE_WALL_Z + WALL_THICK / 2.0);
    assert!((at[2] - (face - range.radius)).abs() < 1e-4 && at[1] == 0.0, "{at:?}");
}

/// The range is open to the sky (TRP's choice): nowhere is
/// indoors, nothing is overhead, and the morning sun's line from high
/// over each start comes down to the ground or what stands there, with
/// no roof or wall of a hall in its way.
#[test]
fn the_range_is_open_to_the_sky() {
    let range = Range::default();
    assert!(indoors().is_none() && hall_lamps().is_empty());
    assert!(!range.solids.iter().any(|solid| solid.overhead || solid.name == "hall wall" || solid.name == "roof"));
    // The way Customs' recorded morning's light goes (Bevy's axes).
    let light = Vec3::new(0.845, -0.525, 0.098).normalize();
    for name in ["rifles", "rigs", "eyewear", "lanes", "dummy"] {
        let at = in_bevy(range.place(name).unwrap());
        assert!(!in_hall(at + Vec3::Y), "{name} is indoors");
        let from = at + Vec3::Y * 0.1 - light * 20.0;
        let hit = range.entered(in_sim(from), in_sim(at + Vec3::Y * 0.1)).map(|share| from + (at + Vec3::Y * 0.1 - from) * share);
        assert!(hit.is_none_or(|hit| hit.y < 2.6), "the sun's line to {name} meets something at {hit:?}");
    }
}

/// The window is vaulted through by the game's own rules (the
/// simulation's measuring and choosing, with the install's settings):
/// from the lane's side, its sill is an obstacle 0.9 m high and as deep
/// as the wall, and the move is a vault; before the wall beside it,
/// nothing is.
#[test]
fn the_window_is_vaulted_through() {
    use trp_sim::locomotion::vaulting::{Move, Standing, Vaulting, VaultingSettings};
    let Ok(install) = trp_data::Install::locate() else {
        eprintln!("skipped: no install configured (set SPT_DIR or local/spt_dir.txt)");
        return;
    };
    let db = trp_data::Database::load(&install).expect("database");
    let settings = VaultingSettings::new(&db.config).expect("vaulting settings");
    let range = Range::default();
    let standing = Standing { vaulting: false, backing: false, grounded: true, in_transition: false, jumping: false, sprinting: false, can_vault: true, can_climb: true };
    // Standing 0.45 m before the wall's face on the lane's side (in the
    // simulation's axes the wall is at -6 m), facing it: half round.
    let try_at = |x: f32| {
        let feet = [x, 0.0, -(DOOR_WALL_Z - WALL_THICK / 2.0) + 0.45];
        let mut vaulting = Vaulting::default();
        vaulting.measure(&settings, &range, feet, 180.0, 0.0, false);
        vaulting.measure(&settings, &range, feet, 180.0, 0.0, false);
        (vaulting.choose(&settings, &standing), vaulting.obstacle.height, vaulting.obstacle.length)
    };
    let (chosen, height, length) = try_at((WINDOW[0] + WINDOW[1]) / 2.0);
    println!("before the window: {chosen:?}, an obstacle {height:.2} m high and {length:.2} m deep");
    assert_eq!(chosen, Some(Move::Vault));
    assert!((height - SILL).abs() < 0.11 && length <= 0.41, "{height} {length}");
    let (beside, ..) = try_at(WINDOW[1] + 0.5);
    assert_eq!(beside, None);
}

/// The window's sill takes a forward mount from the window's start
/// (mounting spec, "The search"): a character 1.1 m before the wall's
/// face, the grid's root at the hands, crouched (the root 0.87 m up, as
/// the app's body has it) or half way up, finds the sill's top on the
/// second frame, 2 cm over it, and its face turned round. Standing (the
/// root 1.41 m up) the search's band, the root's height less half a
/// metre, stops over the 0.9 m sill: refused, by the game's rule.
#[test]
fn the_window_sill_takes_a_mount_crouched() {
    let range = Range::default();
    let s = trp_sim::weapons::mounting::Settings::default();
    let feet = [(WINDOW[0] + WINDOW[1]) / 2.0, 0.0, -(DOOR_WALL_Z - 1.3)];
    let standing = trp_sim::weapons::mounting::Grid { root: [feet[0], 1.41, feet[2] - 0.3], yaw: 180.0, feet: 0.0, column_raise: -0.04, row_raise: -0.04 };
    let mut search = trp_sim::weapons::mounting::Search::standing(&range, &s, standing);
    assert_eq!((search.frame(), search.frame(), search.frame()), (None, None, Some(None)), "refused standing");
    for height in [0.87f32, 1.0, 1.2, 1.3] {
        let grid = trp_sim::weapons::mounting::Grid { root: [feet[0], height, feet[2] - 0.3], yaw: 180.0, feet: 0.0, column_raise: -0.04, row_raise: -0.04 };
        let mut search = trp_sim::weapons::mounting::Search::standing(&range, &s, grid);
        let first = search.frame();
        let point = search.frame().flatten();
        println!("root at {height}: first frame {first:?}, then {point:?}");
        assert_eq!(first, None);
        let point = point.expect("found on the second frame");
        assert!((point.at[1] - (SILL + 0.02)).abs() < 1e-3, "{point:?}");
        assert!((point.direction[2] + 1.0).abs() < 1e-4, "{point:?}");
    }
}
