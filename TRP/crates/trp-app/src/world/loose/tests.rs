//! Tests of what uses the loose models and of the model asked ahead of a throw.

use std::sync::Arc;

use trp_sim::items::inventory::{Item, Kind};

use super::foreseen::RESTS_ON;
use super::*;

/// A thing of the classes named, its template its name.
fn thing(name: &str, classes: &[&str]) -> Item {
    let named: Vec<String> = std::iter::once(name).chain(classes.iter().copied()).chain(["Item"]).map(String::from).collect();
    Item::new(Arc::new(Kind { template: name.into(), name: name.into(), classes: named.clone(), class_names: named, ..Kind::default() }))
}

/// The ground things' keys are strings made from the things (a build's from
/// its parts): made when what is carried or lies about has changed, and
/// while it has not, what lies there is not even looked at. A change of
/// theirs has what uses the models looked through.
#[test]
fn the_ground_things_keys_are_made_only_when_what_is_carried_changed() {
    let ground = [thing("rifle", &["Weapon"]), thing("round", &["Ammo"])];
    let mut uses = Uses::default();
    assert!(uses.keep_keys(7, &ground), "never made: made");
    assert!(uses.stale);
    // A weapon by its build's key (its template, as its stock build), a
    // stack of rounds as an unspent round.
    assert_eq!(uses.lying(|_| true).collect::<Vec<_>>(), ["rifle", "round in a magazine round"]);
    // Only a thing that has a place is drawn where it lies.
    let rounds = ground[1].id;
    assert_eq!(uses.lying(|id| id == rounds).collect::<Vec<_>>(), ["round in a magazine round"]);
    uses.stale = false;
    let untouched = std::iter::from_fn(|| -> Option<&Item> { panic!("the ground is not looked at while nothing carried has changed") });
    assert!(!uses.keep_keys(7, untouched));
    assert!(!uses.stale, "nothing changed: nothing to look through");
    // The rounds taken up: the carried things' change number has moved.
    assert!(uses.keep_keys(8, &ground[..1]));
    assert!(uses.stale);
    assert_eq!(uses.lying(|_| true).collect::<Vec<_>>(), ["rifle"]);
}

/// A model is let go of once it is none of: a flight's, a lying thing's,
/// the grenade in hand's, the one asked ahead, a kept one.
#[test]
fn a_model_nothing_uses_is_let_go_of() {
    let ready = ["flying", "lying", "in hand", "ahead", "kept", "left over"];
    let in_use = ["flying", "lying", "in hand", "ahead", "kept"];
    assert_eq!(uses::unused(ready.into_iter(), in_use.into_iter()), ["left over"]);
    // Its flight over and the thing taken up, and nothing else of its key:
    // it goes; while another thing of the same key lies there it stays.
    assert_eq!(uses::unused(["rifle"].into_iter(), std::iter::empty()), ["rifle"]);
    assert_eq!(uses::unused(["rifle"].into_iter(), ["rifle", "rifle"].into_iter()), Vec::<String>::new());
}

const RIFLE: Id = 41;
const TIN: Id = 42;

/// A weapon's model's key by its number; nothing else is asked ahead.
fn key_of(id: Id) -> Option<String> {
    (id == RIFLE).then(|| "rifle".to_owned())
}

fn never(_: Id) -> Option<String> {
    panic!("the key is not made again while the thing and the carried things are the same")
}

fn open() -> Signs {
    Signs { open: true, ..Signs::default() }
}

/// The mouse on a weapon: its model is asked for once the mouse has rested
/// there, kept while it stays, and let go of as it moves off.
#[test]
fn a_weapon_the_mouse_rests_on_is_asked_ahead_and_let_go_as_it_leaves() {
    let mut foreseen = Foreseen::default();
    let on_rifle = Signs { pointed: Some(RIFLE), ..open() };
    assert!(!foreseen.frame(1.0, on_rifle, 7, key_of));
    assert_eq!(foreseen.key(), None, "the mouse only just on it");
    assert!(!foreseen.frame(1.0 + RESTS_ON * 0.5, on_rifle, 7, key_of));
    assert_eq!(foreseen.key(), None);
    assert!(foreseen.frame(1.01 + RESTS_ON, on_rifle, 7, key_of), "rested: asked for");
    assert_eq!(foreseen.key(), Some("rifle"));
    assert!(!foreseen.frame(5.0, on_rifle, 7, never), "kept, and its key not made again");
    assert_eq!(foreseen.key(), Some("rifle"));
    // Onto a tin: the rifle's is let go of, and a tin's is not asked ahead.
    let on_tin = Signs { pointed: Some(TIN), ..open() };
    assert!(foreseen.frame(5.1, on_tin, 7, key_of), "the key held has changed: looked through");
    assert_eq!(foreseen.key(), None);
    assert!(!foreseen.frame(6.0, on_tin, 7, key_of), "rested on a tin: nothing held, nothing changed");
    assert_eq!(foreseen.key(), None);
    // Back onto the rifle: it rests anew.
    assert!(!foreseen.frame(6.1, on_rifle, 7, key_of));
    assert!(foreseen.frame(6.3, on_rifle, 7, key_of));
    // Off everything.
    assert!(foreseen.frame(6.4, open(), 7, key_of));
    assert_eq!(foreseen.key(), None);
}

/// Taken up by the mouse, or its menu opened: asked for at once, kept while
/// the mouse carries it or the menu is open, the mouse wherever it is.
#[test]
fn a_weapon_taken_up_or_with_its_menu_open_is_asked_ahead_at_once() {
    let mut foreseen = Foreseen::default();
    assert!(foreseen.frame(1.0, Signs { pointed: Some(RIFLE), taken: Some(RIFLE), ..open() }, 7, key_of));
    assert_eq!(foreseen.key(), Some("rifle"));
    // Let go of over a tin, which the mouse is now on.
    assert!(foreseen.frame(1.5, Signs { pointed: Some(TIN), ..open() }, 7, key_of));
    assert_eq!(foreseen.key(), None);
    // The rifle's menu opened, the mouse on the menu and no thing.
    assert!(foreseen.frame(2.0, Signs { menu: Some(RIFLE), ..open() }, 7, key_of));
    assert_eq!(foreseen.key(), Some("rifle"));
    assert!(!foreseen.frame(2.5, Signs { menu: Some(RIFLE), pointed: Some(TIN), ..open() }, 7, never));
    assert_eq!(foreseen.key(), Some("rifle"), "the menu's thing, not the one behind it");
    assert!(foreseen.frame(3.0, open(), 7, key_of), "the menu shut");
    assert_eq!(foreseen.key(), None);
}

/// The screen shut: whatever was asked ahead is let go of, and the reader
/// is no longer kept; opened again, the reader is asked for anew.
#[test]
fn the_screen_shut_lets_go_of_what_was_asked_ahead() {
    let mut foreseen = Foreseen::default();
    assert!(!foreseen.frame(0.5, Signs { pointed: Some(RIFLE), taken: Some(RIFLE), menu: Some(RIFLE), ..Signs::default() }, 7, key_of), "shut: no sign counts");
    assert!(!foreseen.open);
    assert!(foreseen.frame(1.0, Signs { menu: Some(RIFLE), ..open() }, 7, key_of));
    assert!(foreseen.open);
    foreseen.reader_asked = true;
    assert!(foreseen.frame(2.0, Signs { menu: Some(RIFLE), ..Signs::default() }, 7, key_of));
    assert_eq!(foreseen.key(), None);
    assert!(!foreseen.open && !foreseen.reader_asked);
    // The mouse left on the rifle while the screen was shut rests from the
    // opening on.
    assert!(!foreseen.frame(9.0, Signs { pointed: Some(RIFLE), ..open() }, 7, key_of));
    assert!(foreseen.frame(9.2, Signs { pointed: Some(RIFLE), ..open() }, 7, key_of));
}

/// The carried things changed under the sign: the key is made again (a
/// build's changes with its parts); and a thing put down is no longer one
/// the screen has, so its key is let go of (its flight has the model).
#[test]
fn the_key_asked_ahead_follows_the_carried_things() {
    let mut foreseen = Foreseen::default();
    let menu = Signs { menu: Some(RIFLE), ..open() };
    assert!(foreseen.frame(1.0, menu, 7, key_of));
    // A part taken off: another build, another key.
    assert!(foreseen.frame(1.5, menu, 8, |_| Some("rifle[no stock]".to_owned())));
    assert_eq!(foreseen.key(), Some("rifle[no stock]"));
    // A change that leaves its build alone: the same key, nothing to do.
    assert!(!foreseen.frame(1.6, menu, 9, |_| Some("rifle[no stock]".to_owned())));
    // Put down: gone from what the screen can put down.
    assert!(foreseen.frame(1.7, menu, 10, |_| None));
    assert_eq!(foreseen.key(), None);
}
