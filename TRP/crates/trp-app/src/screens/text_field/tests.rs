//! Tests of the text field: every key, the presses and the drag, and the letter under the mouse.

use bevy::prelude::{Color, Entity};

use super::*;

/// The tag field's rules as the install has them: 16 letters, selecting
/// all as it takes the keys; and a check that keeps anything but a tilde.
const RULES: Rules = Rules { limit: 16, keeps: |letter| letter != '~', select_all: true };
const NONE: Mods = Mods { shift: false, ctrl: false, alt: false };
const SHIFT: Mods = Mods { shift: true, ctrl: false, alt: false };
const CTRL: Mods = Mods { shift: false, ctrl: true, alt: false };

/// A field with the keys saying that, its caret at a gap, its moving end
/// at another.
fn field(text: &str, caret: usize, select: usize) -> TextField {
    TextField { caret, select, focused: true, ..TextField::saying(text.to_owned()) }
}

/// The field after the keys, each with what is held.
fn after(mut field: TextField, keys: &[(FieldKey, Mods)]) -> TextField {
    for (key, mods) in keys {
        field.key(key, *mods, &RULES, 1.0);
    }
    field
}

fn typed(letters: &str) -> FieldKey {
    FieldKey::Typed(letters.to_owned())
}

/// What a field says with its caret and moving end, for a test's eye.
fn stands(field: &TextField) -> (&str, usize, usize) {
    (field.text.as_str(), field.caret, field.select)
}

/// The arrows step the caret a letter and stop at the ends; Home, End
/// and their like go to the ends.
#[test]
fn the_arrows_move_the_caret() {
    assert_eq!(stands(&after(field("akm", 3, 3), &[(FieldKey::Left, NONE)])), ("akm", 2, 2));
    assert_eq!(stands(&after(field("akm", 0, 0), &[(FieldKey::Left, NONE)])), ("akm", 0, 0));
    assert_eq!(stands(&after(field("akm", 1, 1), &[(FieldKey::Right, NONE)])), ("akm", 2, 2));
    assert_eq!(stands(&after(field("akm", 3, 3), &[(FieldKey::Right, NONE)])), ("akm", 3, 3));
    assert_eq!(stands(&after(field("akm", 2, 2), &[(FieldKey::Start, NONE)])), ("akm", 0, 0));
    assert_eq!(stands(&after(field("akm", 1, 1), &[(FieldKey::End, NONE)])), ("akm", 3, 3));
}

/// With Shift the moving end goes alone and the caret stays, the
/// selection between them; an arrow without Shift then puts the caret at
/// that end of the selection and moves no further.
#[test]
fn shift_selects_and_an_arrow_alone_ends_the_selection() {
    let left = after(field("akm", 3, 3), &[(FieldKey::Left, SHIFT), (FieldKey::Left, SHIFT)]);
    assert_eq!((stands(&left), left.selection()), (("akm", 3, 1), Some((1, 3))));
    assert_eq!(stands(&after(left.clone(), &[(FieldKey::Left, NONE)])), ("akm", 1, 1));
    assert_eq!(stands(&after(left.clone(), &[(FieldKey::Right, NONE)])), ("akm", 3, 3));
    assert_eq!(stands(&after(left, &[(FieldKey::Right, SHIFT)])), ("akm", 3, 2));
    assert_eq!(stands(&after(field("akm", 2, 2), &[(FieldKey::Start, SHIFT)])), ("akm", 2, 0));
    assert_eq!(stands(&after(field("akm", 1, 1), &[(FieldKey::End, SHIFT)])), ("akm", 1, 3));
    // Home and End without Shift leave nothing selected.
    assert_eq!(stands(&after(field("akm", 3, 1), &[(FieldKey::Start, NONE)])), ("akm", 0, 0));
    assert_eq!(stands(&after(field("akm", 0, 2), &[(FieldKey::End, NONE)])), ("akm", 3, 3));
}

/// Ctrl with an arrow goes to after the next separator a letter on or
/// more, or to after the last one two letters back or more, as the
/// component looks for them: from after the comma the next stop is the
/// end, the space there being no letter on.
#[test]
fn ctrl_moves_by_words() {
    let said = "ak 74, kit";
    let rights: Vec<usize> = (0..4).scan(field(said, 0, 0), |at, _| {
        at.key(&FieldKey::Right, CTRL, &RULES, 1.0);
        Some(at.caret)
    }).collect();
    assert_eq!(rights, [3, 6, 10, 10]);
    let lefts: Vec<usize> = (0..4).scan(field(said, 10, 10), |at, _| {
        at.key(&FieldKey::Left, CTRL, &RULES, 1.0);
        Some(at.caret)
    }).collect();
    assert_eq!(lefts, [7, 6, 3, 0]);
    let both = Mods { shift: true, ctrl: true, alt: false };
    assert_eq!(stands(&after(field(said, 3, 3), &[(FieldKey::Right, both)])), (said, 3, 6));
    assert_eq!(stands(&after(field("a", 1, 1), &[(FieldKey::Left, CTRL)])), ("a", 0, 0));
}

/// Backspace takes the letter before the caret and Delete the one after,
/// each wherever the caret stands; what is selected goes whole to either,
/// the caret left where it began.
#[test]
fn backspace_and_delete_work_at_the_caret() {
    assert_eq!(stands(&after(field("akm", 2, 2), &[(FieldKey::Backspace, NONE)])), ("am", 1, 1));
    assert_eq!(stands(&after(field("akm", 0, 0), &[(FieldKey::Backspace, NONE)])), ("akm", 0, 0));
    assert_eq!(stands(&after(field("akm", 1, 1), &[(FieldKey::Delete, NONE)])), ("am", 1, 1));
    assert_eq!(stands(&after(field("akm", 3, 3), &[(FieldKey::Delete, NONE)])), ("akm", 3, 3));
    assert_eq!(stands(&after(field("akm 74", 4, 1), &[(FieldKey::Backspace, NONE)])), ("a74", 1, 1));
    assert_eq!(stands(&after(field("akm 74", 1, 4), &[(FieldKey::Delete, NONE)])), ("a74", 1, 1));
    // Letters of more than a byte are whole letters.
    assert_eq!(stands(&after(field("ёж№", 2, 2), &[(FieldKey::Backspace, NONE), (FieldKey::Delete, NONE)])), ("ё", 1, 1));
}

/// Ctrl with A selects all, the caret at the end and the moving end at
/// the start; what is typed then takes its place.
#[test]
fn select_all_and_typing_over_a_selection() {
    let all = after(field("akm", 1, 1), &[(FieldKey::SelectAll, NONE)]);
    assert_eq!((stands(&all), all.selection()), (("akm", 3, 0), Some((0, 3))));
    assert_eq!(stands(&after(all, &[(typed("m4"), NONE)])), ("m4", 2, 2));
    assert_eq!(stands(&after(field("akm 74", 4, 1), &[(typed("k"), NONE)])), ("ak74", 2, 2));
}

/// A letter goes in at the caret; a control letter never; a full field
/// takes no more, though what was selected is gone first.
#[test]
fn letters_go_in_at_the_caret_up_to_the_limit() {
    assert_eq!(stands(&after(field("am", 1, 1), &[(typed("k"), NONE)])), ("akm", 2, 2));
    assert_eq!(stands(&after(field("am", 1, 1), &[(typed("\t\r\u{8}"), NONE)])), ("am", 1, 1));
    let full = "0123456789abcdef";
    assert_eq!(stands(&after(field(full, 4, 4), &[(typed("x"), NONE)])), (full, 4, 4));
    assert_eq!(stands(&after(field(full, 16, 14), &[(typed("xyz"), NONE)])), ("0123456789abcdxy", 16, 16));
    // A field of no limit takes any number.
    let mut any = field("", 0, 0);
    any.key(&typed(&"a".repeat(40)), NONE, &Rules { limit: 0, ..RULES }, 1.0);
    assert_eq!(any.letters(), 40);
}

/// A letter the game's check does not keep is taken out after the field
/// put it in: nothing more is said, what was selected is gone, and the
/// caret stands a letter on, the end at most.
#[test]
fn a_letter_the_game_s_check_drops_still_moves_the_caret() {
    assert_eq!(stands(&after(field("akm", 1, 1), &[(typed("~"), NONE)])), ("akm", 2, 2));
    assert_eq!(stands(&after(field("akm", 3, 3), &[(typed("~"), NONE)])), ("akm", 3, 3));
    assert_eq!(stands(&after(field("akm 74", 4, 1), &[(typed("~"), NONE)])), ("a74", 2, 2));
}

/// Enter lets the keys go. Any other key begins the caret's blink again
/// where the field says anything, and not where it says nothing.
#[test]
fn enter_lets_go_and_a_key_begins_the_blink_again() {
    let mut says = field("akm", 3, 3);
    says.key(&FieldKey::Left, NONE, &RULES, 7.0);
    assert_eq!(says.blink_since, Some(7.0));
    says.key(&FieldKey::Enter, NONE, &RULES, 8.0);
    assert!(!says.focused && says.blink_since == Some(7.0));
    let mut emptied = field("a", 1, 1);
    emptied.blink_since = Some(2.0);
    emptied.key(&FieldKey::Backspace, NONE, &RULES, 9.0);
    assert_eq!((emptied.text.as_str(), emptied.blink_since), ("", Some(2.0)));
}

/// The first press gives a field that selects all the keys with all it
/// says selected, wherever the press was; the next puts the caret at the
/// press's gap, or with Shift the moving end alone.
#[test]
fn a_press_takes_the_keys_then_puts_the_caret() {
    let spot = |gap: usize| Spot { gap, letter: gap, word: None };
    let mut tag = TextField::saying("ammo".to_owned());
    tag.press(spot(1), [0.0, 0.0], NONE, &RULES, 10.0);
    assert!(tag.focused && tag.blink_since == Some(10.0));
    assert_eq!(stands(&tag), ("ammo", 4, 0));
    tag.release();
    tag.press(spot(1), [0.0, 0.0], NONE, &RULES, 11.0);
    assert_eq!((stands(&tag), tag.selection(), tag.blink_since), (("ammo", 1, 1), None, Some(11.0)));
    tag.press(spot(3), [0.0, 0.0], SHIFT, &RULES, 12.0);
    assert_eq!(stands(&tag), ("ammo", 1, 3));
    // A field that does not select all takes its caret from the press
    // that gives it the keys; an empty one shows its caret at once.
    let plain = Rules { select_all: false, ..RULES };
    let mut other = TextField::saying("ammo".to_owned());
    other.press(spot(2), [0.0, 0.0], NONE, &plain, 1.0);
    assert_eq!(stands(&other), ("ammo", 2, 2));
    let mut empty = TextField::default();
    empty.press(spot(0), [0.0, 0.0], NONE, &RULES, 3.0);
    assert_eq!((empty.selection(), empty.blink_since), (None, Some(3.0)));
}

/// A second press within half a second selects the word it is on, else
/// the nearest letter; later than that it is a press like any other.
#[test]
fn a_second_press_soon_selects_the_word() {
    let on_word = Spot { gap: 4, letter: 4, word: Some((3, 5)) };
    let mut tag = field("ak 74", 0, 0);
    tag.press(on_word, [0.0, 0.0], NONE, &RULES, 10.0);
    assert_eq!(stands(&tag), ("ak 74", 4, 4));
    tag.press(on_word, [0.0, 0.0], NONE, &RULES, 10.4);
    assert_eq!(stands(&tag), ("ak 74", 3, 5));
    tag.press(on_word, [0.0, 0.0], NONE, &RULES, 11.0);
    assert_eq!(stands(&tag), ("ak 74", 4, 4));
    // Off any word: the nearest letter, and past the end nothing.
    tag.press(Spot { gap: 3, letter: 2, word: None }, [0.0, 0.0], NONE, &RULES, 11.2);
    assert_eq!(stands(&tag), ("ak 74", 2, 3));
    tag.press(Spot { gap: 5, letter: 5, word: None }, [0.0, 0.0], NONE, &RULES, 11.3);
    assert_eq!(stands(&tag), ("ak 74", 5, 5));
    // The press that gave the keys counts as the first.
    let mut fresh = TextField::saying("ak 74".to_owned());
    fresh.press(on_word, [0.0, 0.0], NONE, &RULES, 20.0);
    fresh.press(on_word, [0.0, 0.0], NONE, &RULES, 20.3);
    assert_eq!(stands(&fresh), ("ak 74", 3, 5));
}

/// The mouse held from a press drags the moving end once it has gone the
/// threshold, and from then on however near it comes back; let go, or
/// without the keys, it drags nothing.
#[test]
fn a_drag_moves_the_selection_s_end_past_the_threshold() {
    let spot = |gap: usize| Spot { gap, letter: gap, word: None };
    let mut tag = field("ammo", 0, 0);
    tag.press(spot(1), [100.0, 50.0], NONE, &RULES, 1.0);
    tag.drag(spot(2), [104.0, 50.0], 5.0);
    assert_eq!(stands(&tag), ("ammo", 1, 1));
    tag.drag(spot(3), [105.0, 50.0], 5.0);
    assert_eq!(stands(&tag), ("ammo", 1, 3));
    tag.drag(spot(2), [101.0, 50.0], 5.0);
    assert_eq!(stands(&tag), ("ammo", 1, 2));
    tag.release();
    tag.drag(spot(4), [140.0, 50.0], 5.0);
    assert_eq!(stands(&tag), ("ammo", 1, 2));
    // From the press that gave the keys, the selection runs from the
    // end (all was selected) to the mouse.
    let mut fresh = TextField::saying("ammo".to_owned());
    fresh.press(spot(1), [100.0, 50.0], NONE, &RULES, 1.0);
    fresh.drag(spot(1), [110.0, 50.0], 5.0);
    assert_eq!((stands(&fresh), fresh.selection()), (("ammo", 4, 1), Some((1, 4))));
    fresh.blur();
    fresh.drag(spot(3), [130.0, 50.0], 5.0);
    assert_eq!(stands(&fresh), ("ammo", 4, 1));
}

/// TextMeshPro's words: letters, figures and hyphens together, an
/// apostrophe between two of them with them.
#[test]
fn words_are_counted_as_the_component_counts_them() {
    assert_eq!(words("ak-74 don't 'x' №5"), [(0, 4), (6, 10), (13, 13), (17, 17)]);
    assert!(words(" . ").is_empty());
}

/// Letters ten wide whose drawn boxes are two in from each side, a space
/// (a box of no width) after the second, the line from 100 to 120 down.
fn place() -> FieldPlace {
    let gaps = vec![50.0, 60.0, 70.0, 76.0, 86.0, 96.0];
    let boxes = vec![[52.0, 58.0], [62.0, 68.0], [70.0, 70.0], [78.0, 84.0], [88.0, 94.0]];
    FieldPlace { gaps, boxes, top: 100.0, height: 20.0, color: Color::WHITE }
}

/// The caret goes before a letter left of its box's middle and after it
/// from the middle on; off the ends, to the ends; past a space's origin,
/// to after the space, as the component finds the nearest box.
#[test]
fn a_press_finds_the_gap_and_the_word() {
    let (place, said) = (place(), "ak 74");
    let gap = |x: f32| place.spot(said, [x, 110.0]).gap;
    assert_eq!([gap(0.0), gap(54.9), gap(55.0), gap(60.0), gap(64.0), gap(66.0)], [0, 0, 1, 1, 1, 2]);
    assert_eq!([gap(69.5), gap(71.0), gap(75.0), gap(80.0), gap(91.0), gap(300.0)], [2, 3, 3, 3, 5, 5]);
    // The word under the mouse, within the line's height alone.
    assert_eq!(place.spot(said, [63.0, 110.0]), Spot { gap: 1, letter: 1, word: Some((0, 2)) });
    assert_eq!(place.spot(said, [90.0, 119.0]).word, Some((3, 5)));
    assert_eq!(place.spot(said, [90.0, 121.0]).word, None);
    assert_eq!(place.spot(said, [73.0, 110.0]), Spot { gap: 3, letter: 2, word: None });
    // Past the end, the letter is the field's own last one.
    assert_eq!(place.spot(said, [300.0, 110.0]).letter, 5);
    // A field that says nothing has the one gap.
    let empty = FieldPlace { gaps: vec![50.0], boxes: vec![], top: 100.0, height: 20.0, color: Color::WHITE };
    assert_eq!(empty.spot("", [10.0, 110.0]), Spot::default());
    assert_eq!(empty.spot("", [90.0, 110.0]), Spot::default());
}

/// A key as the engine tells it becomes the field's: the arrows and
/// their like, Ctrl with A alone, and what a key types unless that is a
/// control letter.
#[test]
fn a_key_of_the_engine_s_becomes_the_field_s() {
    use bevy::input::keyboard::{Key, KeyCode, KeyboardInput};
    let pressed = |key_code: KeyCode, logical_key: Key, text: Option<&str>| KeyboardInput { key_code, logical_key, state: bevy::input::ButtonState::Pressed, text: text.map(Into::into), repeat: false, window: Entity::PLACEHOLDER };
    let plain = |key_code: KeyCode, logical_key: Key| field_key(&pressed(key_code, logical_key, None), NONE);
    assert_eq!(plain(KeyCode::ArrowLeft, Key::ArrowLeft), Some(FieldKey::Left));
    assert_eq!(plain(KeyCode::ArrowRight, Key::ArrowRight), Some(FieldKey::Right));
    for (code, key) in [(KeyCode::Home, Key::Home), (KeyCode::ArrowUp, Key::ArrowUp), (KeyCode::PageUp, Key::PageUp)] {
        assert_eq!(plain(code, key), Some(FieldKey::Start));
    }
    for (code, key) in [(KeyCode::End, Key::End), (KeyCode::ArrowDown, Key::ArrowDown), (KeyCode::PageDown, Key::PageDown)] {
        assert_eq!(plain(code, key), Some(FieldKey::End));
    }
    assert_eq!(plain(KeyCode::Backspace, Key::Backspace), Some(FieldKey::Backspace));
    assert_eq!(plain(KeyCode::Delete, Key::Delete), Some(FieldKey::Delete));
    assert_eq!(plain(KeyCode::NumpadEnter, Key::Enter), Some(FieldKey::Enter));
    assert_eq!(plain(KeyCode::Escape, Key::Escape), None);
    let a = pressed(KeyCode::KeyA, Key::Character("a".into()), Some("a"));
    assert_eq!(field_key(&a, NONE), Some(typed("a")));
    assert_eq!(field_key(&pressed(KeyCode::KeyA, Key::Character("a".into()), Some("\u{1}")), CTRL), Some(FieldKey::SelectAll));
    assert_eq!(field_key(&a, Mods { shift: true, ctrl: true, alt: false }), Some(typed("a")));
    assert_eq!(field_key(&pressed(KeyCode::KeyC, Key::Character("c".into()), Some("\u{3}")), CTRL), None);
    assert_eq!(field_key(&pressed(KeyCode::Space, Key::Space, Some(" ")), NONE), Some(typed(" ")));
    assert_eq!(field_key(&pressed(KeyCode::Tab, Key::Tab, Some("\t")), NONE), None);
}
