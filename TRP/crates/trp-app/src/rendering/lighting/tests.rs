//! Tests of the range lit as Customs in the recorded morning.
use super::*;

#[test]
fn the_curve_back_is_the_encodings_inverse() {
    for g in [0.0f32, 0.02, 0.3, 0.5, 1.0, 1.2] {
        let back = linear(g).powf(1.0 / 2.2);
        assert!((back - g).abs() < 1e-5, "{g} {back}");
    }
    // A product of two figures goes back to the product of the two.
    assert!(((linear(0.32) * linear(0.1)).powf(1.0 / 2.2) - 0.032).abs() < 1e-5);
}

#[test]
fn the_surface_ambient_is_the_harmonics_taken_to_gamma() {
    // Factory by day's from above and sideways (lighting spec).
    assert!((surface_ambient(0.1009) - 0.3506).abs() < 1e-3);
    assert!((surface_ambient(0.0117) - 0.110).abs() < 1e-3);
    assert_eq!(surface_ambient(-0.01), 0.0);
}

#[test]
fn halves_are_the_floats_to_nearest() {
    assert_eq!(half(1.0), 0x3c00);
    assert_eq!(half(0.5), 0x3800);
    assert_eq!(half(-2.0), 0xc000);
    assert_eq!(half(0.0), 0);
    // 0.01, as a half 0x211f (0.0099945).
    assert_eq!(half(0.01), 0x211f);
}

#[test]
fn each_face_looks_its_own_way() {
    assert_eq!(face_way(0, 0.0, 0.0), [1.0, 0.0, 0.0]);
    assert_eq!(face_way(2, 0.0, 0.0), [0.0, 1.0, 0.0]);
    assert_eq!(face_way(5, 0.0, 0.0), [0.0, 0.0, -1.0]);
    // The first row of a side face is its top.
    assert!(face_way(4, 0.0, -1.0)[1] > 0.0);
}
