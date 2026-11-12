//! Tests of the tooltips.

use super::*;

/// `courses10`'s `hover-tooltips`: the vest's name, two lines, 400 by
/// 51.15 pixels, the mouse at 946,295 at 2560 by 1440: its top left at
/// 959,231 (13.33 right of the mouse, its top on a whole pixel); on its
/// first frame (214), placed by the one-line size the tooltip had
/// before (29.87 high), at 959,230.72.
#[test]
fn a_tooltip_is_placed_by_the_mouse_and_its_top_left_on_a_pixel() {
    let (screen, s) = (Vec2::new(2560.0, 1440.0), 1440.0 / 1080.0);
    let size = Vec2::new(400.0, 51.15);
    let at = corner(Vec2::new(946.0, 295.0), SimpleTip::OFFSET, size, size, screen, s);
    assert!((at - Vec2::new(959.0, 231.0)).length() < 0.01, "{at}");
    let first = corner(Vec2::new(946.0, 295.0), SimpleTip::OFFSET, Vec2::new(400.0, 29.87), size, screen, s);
    assert!((first - Vec2::new(959.0, 230.72)).length() < 0.01, "{first}");
    // The tooltip of lines, no offset: frame 600, the mouse at
    // 1001,379, 257.28 by 221.68: at 1001,157.
    let lines = Vec2::new(257.28, 221.68);
    let at = corner(Vec2::new(1001.0, 379.0), Vec2::ZERO, lines, lines, screen, s);
    assert!((at - Vec2::new(1001.0, 157.0)).length() < 0.01, "{at}");
}

/// Kept on the screen: pushed back from the right and the top edges it
/// crosses, or the left and the bottom, then its top left put on a
/// whole pixel (at the bottom, its top at 1410, its foot 0.13 above the
/// edge).
#[test]
fn a_tooltip_is_kept_on_the_screen() {
    let (screen, s) = (Vec2::new(2560.0, 1440.0), 1440.0 / 1080.0);
    let size = Vec2::new(400.0, 29.87);
    let at = corner(Vec2::new(2500.0, 10.0), SimpleTip::OFFSET, size, size, screen, s);
    assert!((at.x - 2160.0).abs() < 0.01 && at.y.abs() < 0.01, "{at}");
    let at = corner(Vec2::new(0.0, 1440.0), Vec2::new(-20.0, -20.0), size, size, screen, s);
    assert!(at.x.abs() < 0.01 && (at.y - 1410.0).abs() < 0.01, "{at}");
}

/// One showing of a tooltip by `place_tip`, its words `made`, the new words
/// taken in place where `swaps`: what is shown after, and whether it was
/// made afresh.
fn shown_with(world: &mut World, shown: Shown, made: &'static str, swaps: bool) -> (Shown, bool) {
    use bevy::ecs::system::RunSystemOnce;
    let showing = move |In(mut shown): In<Shown>, mut commands: Commands, mut roots: Roots| {
        let mut afresh = false;
        let mut make = |commands: &mut Commands, _: &dyn Fn(Vec2) -> Vec2| {
            afresh = true;
            (commands.spawn((TipRoot, Node::default(), Visibility::Hidden)).id(), Vec2::new(40.0, 20.0), None)
        };
        let mut swap = |_: &mut Commands, shown: &mut Shown, _: &mut Roots| {
            if swaps {
                shown.size = Vec2::new(44.0, 20.0);
            }
            swaps
        };
        place_tip(&mut commands, &mut shown, &mut roots, made.to_owned(), (Vec2::new(100.0, 100.0), SimpleTip::OFFSET, Vec2::new(1920.0, 1080.0), 1.0), &mut make, &mut swap);
        (shown, afresh)
    };
    world.run_system_once_with(showing, shown).expect("the system ran")
}

/// New words the tooltip can take in place keep its entities (a
/// stimulator's seconds counting down: the game sets its one tooltip's
/// text); words it cannot take are made afresh, the old root gone; the same
/// words change nothing.
#[test]
fn new_words_of_the_same_shape_keep_the_tooltip() {
    let mut world = World::new();
    let (shown, afresh) = shown_with(&mut world, Shown::default(), "Pain killer (58)", true);
    let first = shown.root.expect("made");
    assert!(afresh, "nothing was shown: made");
    let (shown, afresh) = shown_with(&mut world, shown, "Pain killer (57)", true);
    assert!(!afresh && shown.root == Some(first) && shown.made.as_deref() == Some("Pain killer (57)"), "taken in place");
    assert_eq!(shown.size, Vec2::new(44.0, 20.0), "sized to the new words");
    let (shown, afresh) = shown_with(&mut world, shown, "Pain killer (57)", false);
    assert!(!afresh && shown.root == Some(first), "the same words: nothing done");
    let (shown, afresh) = shown_with(&mut world, shown, "<sprite name=\"armor_classes_4\">CHEST", false);
    assert!(afresh && shown.root.is_some_and(|root| root != first), "another shape: made afresh");
    assert!(world.get_entity(first).is_err(), "the old root despawned");
}

/// The shape new words must share to be taken in place: the scale, the
/// mark, the game's face.
#[test]
fn a_tooltip_s_shape_is_its_scale_its_mark_and_the_game_s_face() {
    assert!(tip_shape(1.0, None, true).is_some());
    assert_eq!(tip_shape(1.0, None, true), tip_shape(1.0, None, true));
    assert_ne!(tip_shape(1.0, Some(4), true), tip_shape(1.0, None, true));
    assert_ne!(tip_shape(4.0 / 3.0, None, true), tip_shape(1.0, None, true));
    assert_eq!(tip_shape(1.0, None, false), None, "the app's font: always made afresh");
}
