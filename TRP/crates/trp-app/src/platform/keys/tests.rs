//! Tests of the game's key bindings as the install has them.
use super::*;

#[test]
fn two_notches_in_a_frame_are_none() {
    assert_eq!([notch(1.0), notch(-1.0), notch(0.0), notch(2.0), notch(-3.0), notch(0.5)], [1, -1, 0, 0, 0, 0]);
}

#[test]
fn every_key_named_here_has_the_game_engine_s_number() {
    for (key, name) in NAMED {
        assert!(code(name).is_some(), "{key:?} is named {name}, which the engine has no number for");
    }
    let all = engine_keys();
    assert!(all.windows(2).all(|pair| pair[0].0 < pair[1].0), "each key once, lowest first");
    // What was read of this install is all in keys known here.
    for (name, read, press) in AS_READ {
        assert!(!as_read(read).is_empty() && PressType::of(press).is_some(), "{name}");
        assert!(read.split('+').all(|key| key.starts_with("Wheel") || code(key).is_some()), "{name}: {read}");
    }
}

#[test]
fn a_variant_reads_with_the_keys_held_first() {
    assert_eq!(words(&as_read("T+LeftAlt")), "L ALT + T");
    assert_eq!(words(&as_read("C+WheelUp")), "C + WHEEL UP");
    assert_eq!(words(&as_read("Alpha2")), "2");
    assert_eq!(words(&as_read("Mouse1+LeftControl")), "L CTRL + RMB");
    assert_eq!(words(&Source::default()), "");
}

#[test]
fn a_run_can_tap_the_keys_a_text_field_hears() {
    let board = |word: &str| tapped(word).and_then(|key| if let Key::Board(code) = key { Some(code) } else { None });
    let words = [("left", KeyCode::ArrowLeft), ("right", KeyCode::ArrowRight), ("up", KeyCode::ArrowUp), ("down", KeyCode::ArrowDown), ("home", KeyCode::Home), ("end", KeyCode::End), ("backspace", KeyCode::Backspace), ("delete", KeyCode::Delete), ("pageup", KeyCode::PageUp), ("a", KeyCode::KeyA), ("ctrl", KeyCode::ControlLeft)];
    for (word, key) in words {
        assert_eq!(board(word), Some(key), "{word}");
    }
    assert_eq!(tapped("nothing"), None);
}
