//! Tests of the edit build screen: a circle's lengths, the spacing, the slots on the ellipse and their classes.
use super::*;

/// A circle's quarter is a quarter of its round, and an angle comes
/// back from its length; in the third quarter a degree on, as the
/// game's table has it (`GetCashLength` reads `CachedLengths[angle −
/// 180]`, a degree further than the others' reading).
#[test]
fn a_circle_s_lengths_and_angles() {
    let circle = Ellipse::new(100.0, 100.0);
    assert!((circle.length - 2.0 * std::f64::consts::PI * 100.0).abs() < 1.0, "{}", circle.length);
    for (angle, back) in [(10.0f32, 10.0), (45.0, 45.0), (100.0, 100.0), (200.0, 201.0), (300.0, 300.0)] {
        let length = circle.length_at(angle);
        assert!((circle.angle_at(length) - back).abs() < 0.2, "{angle}: {length} back to {}", circle.angle_at(length));
    }
}

/// Values nearer than the distance are pushed apart about their mean;
/// those far apart stay.
#[test]
fn values_are_spaced_about_their_mean() {
    let mut values = [100.0, 105.0, 500.0];
    space_out(&mut values, 110.0, 2000.0);
    assert!((values[0] - 47.5).abs() < 1e-3 && (values[1] - 157.5).abs() < 1e-3 && values[2] == 500.0, "{values:?}");
    // Across the line's ends: the later moved round.
    let mut values = [10.0, 1990.0];
    space_out(&mut values, 110.0, 2000.0);
    assert!((values[1] - values[0]).abs() > 109.0, "{values:?}");
}

/// A slot's view lies on the ellipse.
#[test]
fn slots_lie_on_the_ellipse() {
    let placed = place_slots(&[[900.0, 600.0], [1000.0, 300.0]], [960.0, 540.0], 576.0, 325.0);
    for [x, y] in placed {
        let on = ((x - 960.0) / 576.0).powi(2) + ((y - 540.0) / 325.0).powi(2);
        assert!((on - 1.0).abs() < 0.02, "{x}, {y}: {on}");
    }
}

#[test]
fn slot_classes_and_words() {
    assert_eq!(slot_class("mod_magazine", false), ModClass::Gear);
    assert_eq!(slot_class("mod_scope", false), ModClass::Functional);
    assert_eq!(slot_class("mod_mount_000", false), ModClass::Functional);
    assert_eq!(slot_class("mod_scope", true), ModClass::Vital);
    assert_eq!(slot_words("mod_pistol_grip"), "PGrip");
    assert_eq!(slot_words("mod_gas_block"), "mod_gas_block");
    assert_eq!(slot_words("mod_mount_000"), "mod_tactical");
}
