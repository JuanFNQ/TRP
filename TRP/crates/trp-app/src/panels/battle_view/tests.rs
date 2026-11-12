//! Tests of the battle screen's panels.

use super::*;

/// The pistols' glow after a malfunction (`ShowQuickdrawGlow`): 28 legs
/// for the install's 4.85 s; up to 0.3 by out quad over the first 0.175 s
/// (0.225 halfway), down the same way back, 0 at 0.35, and over at 4.9 s.
#[test]
fn the_pistols_glow_rises_and_falls_for_4_9_s() {
    let legs = glow_legs(5.0 - 0.15);
    assert_eq!(legs, 28);
    let at = |t: f32| glow_alpha(t, legs);
    assert_eq!(at(0.0), Some(0.0));
    assert!((at(0.0875).unwrap() - 0.225).abs() < 1e-5, "{:?}", at(0.0875));
    assert!((at(0.175).unwrap() - 0.3).abs() < 1e-5, "{:?}", at(0.175));
    assert!((at(0.175 + 0.0875).unwrap() - 0.3 * 0.75).abs() < 1e-5, "the way down mirrored in time: {:?}", at(0.2625));
    assert!(at(0.35).unwrap().abs() < 1e-5, "{:?}", at(0.35));
    assert!(at(4.85).is_some(), "the last leg still running");
    assert_eq!(at(4.9), None, "over: back at rest");
}

/// A malfunction raises the quick bar as a change of the hands does:
/// a while with Autohide (whole from half a second, out 3.5 s on), held
/// with Always, never with Never.
#[test]
fn a_malfunction_raises_the_quick_bar_by_its_setting() {
    let alpha_after = |quick: Shows, seconds: f32| {
        let mut state = BattleState::default();
        state.raise_quick(BattleModes { quick, ..BattleModes::default() });
        let mut t = 0.0;
        while t < seconds {
            state.quick.advance(1.0 / 120.0);
            t += 1.0 / 120.0;
        }
        state.quick.alpha
    };
    assert_eq!(alpha_after(Shows::Autohide, 1.0), 1.0);
    assert_eq!(alpha_after(Shows::Autohide, 4.1), 0.0, "out after its three seconds' stay");
    assert_eq!(alpha_after(Shows::Always, 4.1), 1.0);
    assert_eq!(alpha_after(Shows::Never, 1.0), 0.0);
}

/// The pistols' glow runs on the slots' pistols alone: not a rifle, not a
/// pistol bound to a key, not a pistol with a malfunction known.
#[test]
fn only_a_slot_s_pistol_with_no_known_malfunction_glows() {
    use std::sync::Arc;
    use trp_sim::items::inventory::Kind;
    let weapon = |template: &str| Item::new(Arc::new(Kind { template: template.into(), class_names: vec![template.into(), "Weapon".into(), "Item".into()], ..Kind::default() }));
    let pistol = weapon("pm");
    let rifle = weapon("akm");
    let mut jammed = weapon("pm");
    jammed.malfunction.malf = trp_sim::weapons::malfunction::Malf::Jam;
    jammed.malfunction.known = true;
    let bound = weapon("pm");
    let things: HashMap<&str, &Item> = [("Item1", &pistol), ("Item2", &rifle), ("Item3", &jammed), ("Item4", &bound)].into_iter().collect();
    let keys = ["Grenade", "ItemV", "Item1", "Item2", "Item3", "Item4"];
    let glowing = glowing_slots(keys.into_iter(), |key| things.get(key).copied(), |item| item.kind.template == "pm");
    assert_eq!(glowing, ["Item1"]);
}

/// The fire mode's panel in `courses8`'s check (`5-check`, numpad 5 of
/// recorder 0.14.0): shown on the key's frame and stepped on it, whole a
/// frame past half a second, held three seconds from that frame, and
/// faded out from the frame after the one the wait ends on. Each figure
/// is the game's `CanvasGroup.alpha` on the frame at that time.
#[test]
fn a_panel_fades_as_the_check_recorded_it() {
    let frames: [(f64, Option<f32>); 9] = [
        (0.491674364, None),
        (0.5000086, Some(0.0330589637)),
        (0.5083419, Some(0.0655587763)),
        (0.983351767, Some(0.9997229)),
        (0.9916853, Some(1.0)),
        (3.98812747, Some(1.0)),
        (3.99646068, Some(1.0)),
        (4.004794, Some(0.9669438)),
        (4.013128, Some(0.9344416)),
    ];
    let mut fader = Fader::default();
    for pair in frames.windows(2) {
        let ((before, _), (now, alpha)) = (pair[0], pair[1]);
        if before == 0.491674364 {
            fader.show(true);
        }
        fader.advance((now - before) as f32);
        let alpha = alpha.unwrap();
        assert!((fader.alpha - alpha).abs() < 2e-5, "at {now} s: {} where the game had {alpha}", fader.alpha);
    }
}

/// The health panel raised by a fall in `courses10`'s
/// `battle-health-autohide` (Autohide): the landing's damage on frame
/// 305 and the jump's end on a broken leg on 306, each heard after the
/// frame's step. Each figure is the game's `CanvasGroup.alpha` as the
/// interface file keeps it, to four places.
#[test]
fn a_raise_by_damage_is_stepped_from_the_next_frame() {
    let frames: [(f64, f32); 13] = [
        (2.76254129, 0.0),
        (2.7708745, 0.0),
        (2.792783, 0.0857),
        (2.80147672, 0.1172),
        (3.28258681, 0.9996),
        (3.29091525, 1.0),
        (3.2992487, 1.0),
        (6.293102, 1.0),
        (6.30142832, 1.0),
        (6.30976248, 0.9669),
        (6.318096, 0.9344),
        (6.79367, 0.0002),
        (6.802005, 0.0),
    ];
    let mut fader = Fader::default();
    for pair in frames.windows(2) {
        let ((before, _), (now, alpha)) = (pair[0], pair[1]);
        fader.advance((now - before) as f32);
        assert!((fader.alpha - alpha).abs() < 6e-5, "at {now} s: {} where the game had {alpha}", fader.alpha);
        if now == 2.7708745 || now == 2.792783 {
            fader.show(true);
        }
    }
}

/// The stance panel raised by the aim's key in `courses10`'s
/// `battle-panels` (Autohide; the sights put down on frame 1168, the
/// panel out): raised before the frame's step (the arms' drain ended)
/// and again after it (the aim's limit gone, heard in the movement's
/// update). Each figure is the game's `CanvasGroup.alpha` (the
/// recorder's `hud`); one raise would have 0.0656 on the second frame.
#[test]
fn the_aims_key_raises_the_stance_panel_twice() {
    let frames: [(f64, f32); 5] = [(9.862347, 0.0), (9.870681, 0.0330597535), (9.879014, 0.06502351), (9.887347, 0.09644818), (9.89568, 0.12733604)];
    let mut state = BattleState::default();
    let modes = BattleModes::default();
    for pair in frames.windows(2) {
        let ((before, _), (now, alpha)) = (pair[0], pair[1]);
        let key = now == 9.870681;
        if key {
            state.pull_up(modes);
        }
        state.stance.advance((now - before) as f32);
        if key {
            state.pull_up(modes);
        }
        assert!((state.stance.alpha - alpha).abs() < 2e-5, "at {now} s: {} where the game had {alpha}", state.stance.alpha);
    }
}
