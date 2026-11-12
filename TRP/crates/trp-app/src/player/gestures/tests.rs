//! Tests of the hand signs and the voice's phrases.
use super::*;

/// The prompt's phrase is the highest priority, a tie to the name last
/// in the alphabet; a tap lowers it by 10, so the next tap moves on;
/// a fresh situation restores it, but a hurt's.
#[test]
fn the_prompt_s_phrase_moves_on_with_each_tap() {
    let voiced: HashSet<i32> = [phrase::LOOT_BODY, phrase::CHECK_HIM, phrase::HURT_HEAVY, phrase::BLEEDING].into_iter().collect();
    let mut g = Gestures::default();
    g.activate(phrase::LOOT_BODY, &voiced);
    g.activate(phrase::CHECK_HIM, &voiced);
    assert_eq!(g.top(), Some(phrase::CHECK_HIM), "75 over 70");
    g.activate(phrase::BLEEDING, &voiced);
    assert_eq!(g.top(), Some(phrase::BLEEDING));
    *g.quick.get_mut(&phrase::BLEEDING).unwrap() -= 10;
    assert_eq!(g.top(), Some(phrase::CHECK_HIM), "75 over 75: CheckHim after Bleeding in the alphabet");
    g.activate(phrase::BLEEDING, &voiced);
    assert_eq!(g.top(), Some(phrase::BLEEDING), "restored");
    g.activate(phrase::HURT_HEAVY, &voiced);
    *g.quick.get_mut(&phrase::HURT_HEAVY).unwrap() -= 10;
    g.activate(phrase::HURT_HEAVY, &voiced);
    assert_eq!(g.quick[&phrase::HURT_HEAVY], 80, "a hurt's is not restored");
    g.activate(phrase::LOOT_KEY, &voiced);
    assert!(!g.quick.contains_key(&phrase::LOOT_KEY), "not in the voice");
    assert_eq!(g.rows(), [phrase::BLEEDING, phrase::HURT_HEAVY, phrase::LOOT_BODY, phrase::CHECK_HIM]);
}

/// One key a thing, one thing a key; F1 the mumble by default.
#[test]
fn a_bind_moves_and_replaces() {
    let mut g = Gestures::default();
    assert_eq!(g.binds[0], phrase::MUMBLE);
    g.bind(2, 67);
    g.bind(3, 67);
    assert_eq!((g.binds[2], g.binds[3]), (0, 67));
    g.bind(3, 203);
    assert_eq!(g.binds[3], 203);
    assert_eq!(is_sign(203), Some(Sign::AllRightGesture));
    assert_eq!(is_sign(67), None);
    assert!(g.available(67) && !g.available(phrase::LOOT_BODY) && g.available(phrase::NEED_AMMO));
}

/// The blink: lit, out at a sixth, lit at a third, held lit from two
/// thirds to the second.
#[test]
fn the_prompt_blinks_twice_a_second() {
    assert!((blink_alpha(0.0) - 1.0).abs() < 1e-6);
    assert!(blink_alpha(1.0 / 6.0) < 1e-5);
    assert!((blink_alpha(1.0 / 3.0) - 1.0).abs() < 1e-5);
    assert!(blink_alpha(0.5) < 1e-5);
    assert_eq!(blink_alpha(0.8), 1.0);
    assert!((blink_alpha(1.0) - 1.0).abs() < 1e-6, "looped");
}

/// A weapon's malfunction raises WEAPON BROKEN at the shot (its kind
/// unknown) and again 150 ms on, so it holds 5.15 s from the shot; an
/// elite shooter's slide is JAMMED at once; the look at its kind raises
/// nothing; a weapon drawn with one raises it, one drawn clean nothing
/// (`NeedRepairMalfPhraseSituation` and its callers).
#[test]
fn a_malfunction_raises_the_prompt_at_the_shot_and_the_draw() {
    use trp_sim::weapons::malfunction::{Malf, State};
    use trp_sim::FirearmEvent;
    let jammed = State { malf: Malf::Jam, known: true, ..State::default() };
    let raised = phrase::of_hands_event(&FirearmEvent::Malfunctioned(Malf::Jam), Some(&jammed));
    assert_eq!(raised, Some(phrase::WEAPON_BROKEN));
    let elite = State { malf: Malf::SoftSlide, known: true, type_known: true, ..State::default() };
    assert_eq!(phrase::of_hands_event(&FirearmEvent::Malfunctioned(Malf::SoftSlide), Some(&elite)), Some(phrase::ON_WEAPON_JAMMED));
    let unknown = State { type_known: false, ..elite };
    assert_eq!(phrase::of_hands_event(&FirearmEvent::Malfunctioned(Malf::SoftSlide), Some(&unknown)), Some(phrase::WEAPON_BROKEN), "a slide's kind unknown");
    assert_eq!(phrase::of_hands_event(&FirearmEvent::MalfunctionExamined { malf: Malf::SoftSlide, first: true }, Some(&elite)), None, "the look raises none");
    assert_eq!(phrase::of_hands_event(&FirearmEvent::MalfunctionReminder { malf: Malf::HardSlide, type_known: true }, None), Some(phrase::ON_WEAPON_JAMMED));
    assert_eq!(phrase::of_hands_event(&FirearmEvent::Appeared, Some(&jammed)), Some(phrase::WEAPON_BROKEN), "drawn jammed");
    assert_eq!(phrase::of_hands_event(&FirearmEvent::Appeared, Some(&State::default())), None, "drawn clean");
    assert_eq!(phrase::of_hands_event(&FirearmEvent::Appeared, None), None, "no wear");
    // Raised at the shot and 150 ms on: up until 5.15 s.
    let voiced: HashSet<i32> = [phrase::WEAPON_BROKEN].into_iter().collect();
    let mut g = Gestures::default();
    g.raise(phrase::WEAPON_BROKEN, 10.0, &voiced);
    g.expire(10.0);
    g.raise(phrase::WEAPON_BROKEN, 10.15, &voiced);
    g.expire(15.1);
    assert_eq!(g.top(), Some(phrase::WEAPON_BROKEN), "still up 5.1 s from the shot");
    g.expire(15.15);
    assert_eq!(g.top(), None, "gone 5.15 s from the shot");
}

#[test]
fn every_phrase_of_the_menu_has_its_name() {
    for (_, _, phrases) in GROUPS {
        for p in phrases {
            assert!(!phrase::name(*p).is_empty(), "{p}");
        }
    }
    assert_eq!(phrase::name(phrase::MUMBLE), "MumblePhrase");
    assert!(phrase::situational(phrase::LOOT_BODY) && !phrase::situational(phrase::NEED_AMMO) && !phrase::situational(67));
}
