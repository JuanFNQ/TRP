//! Tests of the weapon in hand's wear, malfunctions and heat.
use super::*;

/// A reminder's glow shows at the third ask within 2 s of the first,
/// then no more in that window; a forced one shows at once
/// (`BattleUIMalfunctionGlow.ShowGlow`).
#[test]
fn the_glow_shows_at_the_third_ask_in_its_window() {
    let mut feedback = Feedback::default();
    let ask = |feedback: &mut Feedback, at: f64| feedback.ask_glow(Glow::Examined, false, at, 3, 0.5);
    assert!(!ask(&mut feedback, 10.0) && !ask(&mut feedback, 10.5));
    assert!(ask(&mut feedback, 11.0), "the third within 2 s");
    assert!(!ask(&mut feedback, 11.5) && !ask(&mut feedback, 11.9), "none more in the window");
    assert!(!ask(&mut feedback, 12.5), "a new window from here");
    assert!(feedback.ask_glow(Glow::Repaired, true, 12.6, 3, 0.5), "forced");
    assert_eq!(feedback.glow.map(|(glow, ..)| glow), Some(Glow::Repaired));
}

/// A notice's row opens and folds as `courses10`'s `examine-multiple`'s
/// first warning (its box in pixels at 1440 lines, 26.61 units when
/// open and the row's padding of 2 below it): its clock from frame 177
/// (1.600), open 20.97 on 190 (1.715; the take 21.05), 31.16 on 200
/// (1.799; 31.22), 34.10 on 205 (1.843; 34.14); its fold from 499
/// (4.354), 21.34 on 505 (4.416; 21.29), 0 on 518 (4.582; 0).
#[test]
fn a_notice_row_opens_and_folds_as_the_game() {
    let s = 1440.0 / 1080.0;
    let shown = |since: f64, folding: Option<f64>| notice_box(notice_row(26.61 + 2.0, since, folding), 2.0) * s;
    for (since, take) in [(1.715 - 1.6, 21.05), (1.799 - 1.6, 31.22), (1.843 - 1.6, 34.14)] {
        assert!((shown(since, None) - take).abs() < 0.15, "open {since}: {} against {take}", shown(since, None));
    }
    assert!((shown(1.0, None) - 35.48).abs() < 0.01, "open");
    let (fold, take) = (4.416 - 4.354, 21.29);
    assert!((shown(3.0, Some(fold)) - take).abs() < 0.15, "folding: {}", shown(3.0, Some(fold)));
    assert_eq!(shown(3.0, Some(4.582 - 4.354)), 0.0, "shut");
}

/// Every notice asked shows at once, the same alert's words not again
/// while one is up and not folding.
#[test]
fn notices_show_at_once_but_a_singleton_once() {
    let mut feedback = Feedback::default();
    feedback.warning("You can't examine two items at the same time".to_owned());
    feedback.warning("You can't examine two items at the same time".to_owned());
    feedback.message("Item examined: Gas safe (+10 exp)".to_owned());
    assert_eq!(feedback.waiting.len(), 3, "each asked is shown");
    feedback.single_warning("YOU CAN'T EXAMINE TWO ITEMS AT THE SAME TIME".to_owned());
    assert_eq!(feedback.waiting.len(), 3, "a singleton not while the same waits");
    feedback.waiting.clear();
    feedback.shown.push(Notice { words: "Item is being examined".to_owned(), alert: true, began: 0.0, fold: Some(2.75), number: 1 });
    feedback.single_warning("Item is being examined".to_owned());
    assert_eq!(feedback.waiting.len(), 1, "one folding is hiding");
}

/// A notice's alphas: its icon in over a third of a second, all of it
/// out over a quarter after 2.667 s.
#[test]
fn a_notice_comes_and_goes() {
    assert_eq!(notice_alphas(0.0), (1.0, 0.0));
    assert!((notice_alphas(NOTICE_ICON_IN / 2.0).1 - 0.5).abs() < 1e-5);
    assert_eq!(notice_alphas(1.0), (1.0, 1.0));
    let (group, _) = notice_alphas(NOTICE_SHOW + NOTICE_HIDE / 2.0);
    assert!((group - 0.5).abs() < 1e-5);
    assert_eq!(notice_alphas(NOTICE_SHOW + NOTICE_HIDE), (0.0, 0.0));
}
