//! Tests of the asking window's sliders, stops and stepped knob.

use super::*;

/// The slider of the window that asks how many stops where the
/// game's does: a notch every twenty-fourth of the most, rounded up,
/// from one, and the most at the end.
#[test]
fn the_slider_has_the_game_s_stops() {
    assert_eq!(Asking::stops(1), [1]);
    assert_eq!(Asking::stops(10), [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
    assert_eq!(Asking::stops(24).len(), 24);
    let of_59 = Asking::stops(59);
    assert_eq!((of_59.len(), of_59[0], of_59[1], of_59[19], of_59[20]), (21, 1, 3, 57, 59));
    let of_25 = Asking::stops(25);
    assert_eq!((of_25.len(), of_25[0], of_25[1], of_25[12], of_25[13]), (14, 1, 2, 24, 25));
    // The window comes up at half and one of the most.
    assert_eq!((59 / 2 + 1, 29 / 2 + 1), (30, 15));
}

/// The window that asks how much of food or drink to use is the
/// game's `ShowSplitMenuDialog`: a track from 1 to the whole, the knob
/// from `(int)(whole - left + 1)` on, the figure 1 to what is left,
/// all of it to begin with.
#[test]
fn the_window_for_a_share_of_food_or_drink_is_the_game_s() {
    // A bottle of 60 with 40 left.
    let bottle = Asking::of_use(7, 60.0, 40.0, Vec2::ZERO).expect("something is left");
    assert_eq!((bottle.title, bottle.most, bottle.count), ("Use", 40, 40));
    // The knob at the track's end, and a third of it gone before its
    // first place (20 of the 59 steps from 1 to 60).
    let (knob, gone) = bottle.shares();
    assert!((knob - 1.0).abs() < 1e-6 && (gone - 20.0 / 59.0).abs() < 1e-6, "{knob} {gone}");
    // Held at the track's start it goes no lower than its first place:
    // a figure of 1; at its end, all that is left.
    assert_eq!((bottle.slid_to(0.0), bottle.slid_to(1.0)), (1, 40));
    assert_eq!(bottle.slid_to(30.0 / 59.0), 11);
    // Full, nothing is gone; empty, there is no window.
    let full = Asking::of_use(7, 60.0, 60.0, Vec2::ZERO).expect("full");
    assert_eq!((full.most, full.shares().1), (60, 0.0));
    assert!(Asking::of_use(7, 60.0, 0.0, Vec2::ZERO).is_none());
    // Its notches, one for each place but the last: with 14 of 60
    // left (as a capture of the game shows), 46 not
    // available, then 13 that are, and the row's own last.
    let fourteen = Asking::of_use(7, 60.0, 14.0, Vec2::ZERO).expect("something is left");
    let notches = fourteen.notches();
    assert_eq!((notches.len(), notches.iter().filter(|on| !**on).count(), fourteen.field()), (59, 46, "14".to_owned()));
    assert!(notches[46..].iter().all(|on| *on) && !notches[45]);
    // Typed: in place of all selected, kept to what is left; emptied, it
    // says nothing and the figure stands.
    let mut typed = fourteen;
    typed.typed(0);
    assert_eq!(typed.field(), "1");
    typed.typed(9);
    assert_eq!(typed.field(), "14");
    typed.rubbed_out();
    typed.rubbed_out();
    assert_eq!((typed.field(), typed.count), (String::new(), 1));
    typed.typed(5);
    assert_eq!(typed.field(), "5");
}

/// The stepped slider's knob as the game's stands (`StepSlider`): a
/// figure it came up at or typed puts it at the place of the stop
/// nearest that figure, counted from nought, on a slider counted from
/// one, so a stop short of it (a capture of the game, 30 of 59: its
/// knob on the tenth notch, 27's); slid, at the stop slid to; at the
/// most, at the end.
#[test]
fn the_stepped_knob_stands_where_the_game_s_does() {
    let window = Asking::of_stack(7, Target::Item(8), false, 59, Vec2::ZERO);
    assert_eq!((window.count, window.knob, window.title, window.notches().len()), (30, 10, "Split", 20));
    assert!((window.shares().0 - 0.45).abs() < 1e-6);
    // Typed: the most at the end, 1 at the start.
    let mut typed = window;
    typed.typed(5);
    typed.typed(9);
    assert_eq!((typed.count, typed.knob), (59, 21));
    typed.rubbed_out();
    assert_eq!((typed.count, typed.knob, typed.field()), (5, 2, "5".to_owned()));
    // A figure past the most is the most; nought is one.
    typed.typed(9);
    assert_eq!(typed.count, 59);
    let mut nought = window;
    nought.typed(0);
    assert_eq!((nought.count, nought.knob), (1, 1));
    // Emptied, it reads as one; emptied at one, it says nothing.
    nought.rubbed_out();
    assert_eq!((nought.count, nought.field()), (1, String::new()));
    // Slid: the value nearest the share, half to even, and the stop
    // there.
    let mut slid = window;
    slid.slide_to(0.5);
    assert_eq!((slid.count, slid.knob), (30, 11));
    slid.slide_to(1.0);
    assert_eq!((slid.count, slid.knob), (59, 21));
    // Where two stops are as near, the later (`Aggregate` taking the
    // second on a tie): 2 lies between 1 and 3.
    assert_eq!(Asking::knob_of(59, 2), 1);
    assert_eq!(Asking::knob_of(59, 5), 2);
}

/// `courses10`'s `how-many`: the handle of the window at "30" of 59
/// (its knob on 27's notch, slider value 10, the row 869 to 1269) held
/// 2 px left of its middle (1049) at frame 458 and drawn left: "30"
/// until the mouse reached 1038 (value 9.55, still 10), 24 at 1036
/// (9.45, frame 476), as `Slider.Set` passes over the value it has.
#[test]
fn a_stepped_slider_held_at_its_knob_s_value_keeps_its_figure() {
    let window = Asking::of_stack(7, Target::Item(8), false, 59, Vec2::ZERO);
    let held = |mouse: f32| {
        let mut held = window;
        held.slide_to((mouse + 2.0 - 869.0) / 400.0);
        (held.count, held.knob)
    };
    assert_eq!(held(1047.0), (30, 10), "pressed, not moved");
    assert_eq!(held(1038.0), (30, 10));
    assert_eq!(held(1036.0), (24, 9));
}
