//! Tests of the blend between two poses of the rig.
use super::blend_rig;

fn about_z(degrees: f32) -> [f32; 4] {
    let (s, c) = (degrees.to_radians() / 2.0).sin_cos();
    [0.0, 0.0, s, c]
}

/// A cylinder snapped a chamber on (60 degrees for six) between two
/// frames is drawn at one frame or the other, never between; a turn
/// under half a chamber, and every other bone, is blended.
#[test]
fn a_cylinder_s_snap_is_not_drawn_as_a_spin() {
    let older = [([0.0; 3], about_z(0.0)), ([0.0; 3], about_z(0.0)), ([0.0; 3], about_z(0.0))];
    let newer = [([0.0; 3], about_z(60.0)), ([0.0; 3], about_z(-60.0)), ([0.0; 3], about_z(20.0))];
    let near_older = super::blend_rig(&older, &newer, 0.4, Some((1, 30.0)));
    let near_newer = blend_rig(&older, &newer, 0.6, Some((1, 30.0)));
    assert_eq!(near_older[1].1, about_z(0.0));
    assert_eq!(near_newer[1].1, about_z(-60.0));
    // The other bones blended: 60 degrees at 0.4 is 24.
    let turned = 2.0 * near_older[0].1[2].asin().to_degrees();
    assert!((turned - 24.0).abs() < 0.5, "{turned}");
    // Under half a chamber on the cylinder's bone itself: blended.
    let slow = blend_rig(&older, &[older[0], ([0.0; 3], about_z(20.0)), older[2]], 0.5, Some((1, 30.0)));
    assert!((2.0 * slow[1].1[2].asin().to_degrees() - 10.0).abs() < 0.5);
    // No cylinder: blended as before.
    let plain = blend_rig(&older, &newer, 0.5, None);
    assert!((2.0 * plain[1].1[2].asin().to_degrees() + 30.0).abs() < 0.5);
}
