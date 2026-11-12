//! Tests of the interface's dump.

use super::*;

#[test]
fn sounds_are_written_as_the_recorder_writes_them() {
    // `courses10`'s `how-many`, frame 238.
    assert_eq!(told(&Told::Item("ammo_singleround".into(), "drop")), r#"["item","ammo_singleround","drop"]"#);
    assert_eq!(told(&Told::Clip("ammo_singleround_drop".into(), true)), r#"["clip","ammo_singleround_drop",false,true,1]"#);
    assert_eq!(told(&Told::Ui("MenuInstallMag")), r#"["ui","MenuInstallMag"]"#);
}

#[test]
fn words_are_written_in_their_own_colour_not_their_noughts() {
    // `courses10`'s `inventory-open-close`, frame 106: hydration's
    // figure, written in its blue, its dim nought in a tag.
    let run = |text: &str, color: [f32; 4]| trp_data::interface::tmp_text::TmpRun { text: text.into(), size: 40.0, color, bold: false };
    let (dim, blue) = ([0.6, 0.6, 0.6, 0.2], [0.3608, 0.6824, 0.8431, 1.0]);
    assert_eq!(words_colour(&[run("0", dim), run("98", blue)]), blue);
    // A figure with no noughts keeps the noughts' run, empty.
    assert_eq!(words_colour(&[run("", dim), run("440", blue)]), blue);
    assert_eq!(words_colour(&[run("", blue)]), blue);
}

#[test]
fn numbers_are_written_as_the_recorder_writes_them() {
    assert_eq!(p(12.0), "12");
    assert_eq!(p(r2(12.345_67)), "12.35");
    assert_eq!(p(r2(-0.001)), "0");
    assert_eq!(f(r4(0.5)), "0.5");
    assert_eq!(f(r4(0.123_456)), "0.1235");
    assert_eq!(text("a\"b\\c\nd"), "\"a\\\"b\\\\c\\nd\"");
}

#[test]
fn a_turned_box_is_the_box_round_its_corners() {
    let flat = UiGlobalTransform::from_translation(Vec2::new(100.0, 50.0));
    assert_eq!(box_of(&flat, Vec2::new(20.0, 10.0)), ([90.0, 45.0, 110.0, 55.0], 0.0));
    // A quarter turn counter-clockwise as seen, down being positive.
    let turned = UiGlobalTransform::from(bevy::math::Affine2::from_angle_translation(-std::f32::consts::FRAC_PI_2, Vec2::new(100.0, 50.0)));
    let (rect, turn) = box_of(&turned, Vec2::new(20.0, 10.0));
    assert_eq!(rect, [95.0, 40.0, 105.0, 60.0]);
    assert_eq!(turn, 90.0);
}
