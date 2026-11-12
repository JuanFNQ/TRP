//! Tests of the knife's blade, its damage and the direction of its hits.
use super::*;

/// The ice pick's blade box (0.0008 by 0.216 by 0.127, its centre at
/// its object) looked along level forward: its box back a blade's
/// length, half its width and depth, a quarter of its length.
#[test]
fn a_blade_is_swept_from_a_blade_length_back() {
    let look = look_way(0.0, 0.0);
    let sweep = BladeSweep::of([0.2, 1.4, 0.5], [0.0, 0.0, 0.0, 1.0], [0.0; 3], [0.0008, 0.216, 0.127], look, 0.6);
    assert!((sweep.from[2] - (0.5 - 0.216)).abs() < 1e-6 && sweep.from[0] == 0.2);
    assert_eq!(sweep.half, [0.0004, 0.054, 0.0635]);
    // A look 90 degrees right and 30 down.
    let way = look_way(90.0, 30.0);
    assert!((way[0] - 30f32.to_radians().cos()).abs() < 1e-6 && (way[1] + 0.5).abs() < 1e-6 && way[2].abs() < 1e-6);
}

/// A blow's damage (`ProcessHit`): the ice pick's slash 29 and stab 35;
/// exhausted, the whole number under 0.33 of it (9 and 11); then times
/// one and Strength's buff.
#[test]
fn a_blows_damage_is_cut_whole_when_exhausted() {
    assert_eq!(blow_damage(29.0, None, 0.0), 29.0);
    assert_eq!(blow_damage(29.0, Some(0.33), 0.0), 9.0);
    assert_eq!(blow_damage(35.0, Some(0.33), 0.0), 11.0);
    assert!((blow_damage(35.0, None, 0.3) - 45.5).abs() < 1e-5);
    assert!((blow_damage(35.0, Some(0.33), 0.1) - 12.1).abs() < 1e-5);
}

/// The swing's way (`GlobalSwingDirection`): eased by 0.9 toward each
/// step of the blade in a stroke, once more at the hit; a quick
/// stroke's from nought, toward the blade from the world's origin.
#[test]
fn a_hits_direction_follows_the_blades_steps() {
    let close = |a: [f32; 3], b: [f32; 3]| a.iter().zip(b).all(|(a, b)| (a - b).abs() < 1e-6);
    let mut swing = Swing::default();
    swing.follow([0.0, 1.0, 0.0]);
    // The first step from nought: 0.9 of the way to (0, 1, 0).
    assert_eq!(swing.way, [0.0, 0.9, 0.0]);
    swing.follow([1.0, 1.0, 0.0]);
    assert!(close(swing.way, [0.9, 0.09, 0.0]), "{swing:?}");
    let hit = swing.at_hit([1.0, 1.0, 1.0]);
    assert!(close(hit, [0.09, 0.009, 0.9]), "{hit:?}");
    assert_eq!((swing.way, swing.before), (hit, [1.0, 1.0, 0.0]), "the way kept, the place not");
    // A quick stroke's: new, never following; toward the blade from the
    // world's origin.
    let hit = Swing::default().at_hit([3.0, 0.0, 4.0]);
    assert!(close(hit, [0.54, 0.0, 0.72]), "{hit:?}");
    // A step under 1e-5 long: no way to ease toward.
    let mut still = Swing { way: [1.0, 0.0, 0.0], before: [2.0; 3] };
    still.follow([2.0; 3]);
    assert!(close(still.way, [0.1, 0.0, 0.0]), "{still:?}");
}
