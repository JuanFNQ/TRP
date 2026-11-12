//! Holding the breath, tiredness and the out-of-breath heave.

use super::*;

/// What the breath does to the weapon over half a minute standing still:
/// how far it dips the muzzle, how far it lifts the weapon, how many
/// breaths, and how far the roll and the yaw are pushed about.
fn breathe(wind: trp_sim::Wind, aiming: bool) -> Option<(f32, f32, usize, f32)> {
    let (mut hands, _) = rig()?;
    let (mut dip, mut rise, mut shake) = (0.0f32, 0.0f32, 0.0f32);
    let (mut peaks, mut rising, mut last) = (0, false, 0.0f32);
    let input = HandsInput { wind, aiming, ..still() };
    // A few rested breaths first: a held breath keeps the pace of the
    // last one drawn.
    for _ in 0..(4.0 / DT) as usize {
        hands.advance(DT, &HandsInput { aiming, ..still() });
    }
    for _ in 0..(30.0 / DT) as usize {
        hands.advance(DT, &input);
        let pitch = hands.hands_rotation.current[0];
        dip = dip.max(pitch);
        rise = rise.max(hands.hands_position.current[2]);
        shake = shake.max(hands.hands_rotation.current[1].abs()).max(hands.hands_rotation.current[2].abs());
        if rising && pitch < last && pitch > 0.3 * dip.max(0.05) {
            peaks += 1;
        }
        rising = pitch > last;
        last = pitch;
    }
    Some((dip, rise, peaks, shake))
}

/// A held breath stills the weapon to a small part of its rise and fall;
/// short of oxygen the breath is deeper and faster; and spent arms shake
/// (weapon-handling spec, "Breathing"). The game's own figures for these
/// are not on record: no recording holds a breath or runs a pool down.
#[test]
fn a_held_breath_stills_the_weapon_and_tiredness_stirs_it() {
    let rested = trp_sim::Wind::default();
    let Some(plain) = breathe(rested, true) else { return };
    let Some(held) = breathe(trp_sim::Wind { held: true, ..rested }, true) else { return };
    let Some(winded) = breathe(trp_sim::Wind { oxygen: 0.2, ..rested }, true) else { return };
    let Some(spent) = breathe(trp_sim::Wind { stamina: 0.05, ..rested }, true) else { return };
    let show = |name: &str, (dip, rise, peaks, shake): (f32, f32, usize, f32)| {
        println!("aiming, {name}: dips {dip:.3} degrees, lifts {:.2} mm, {peaks} breaths in 30 s, roll and yaw pushed up to {shake:.3} degrees", rise * 1000.0);
    };
    show("rested", plain);
    show("breath held", held);
    show("a fifth of the oxygen left", winded);
    show("the arms spent", spent);
    assert!(held.0 < plain.0 * 0.3 && held.1 < plain.1 * 0.3, "a held breath leaves a small part: {held:?} against {plain:?}");
    assert!(held.3 < plain.3 * 0.5, "and of the random pushes: {} against {}", held.3, plain.3);
    assert!(winded.0 > plain.0 * 1.2 && winded.2 > plain.2, "short of oxygen: deeper and faster: {winded:?} against {plain:?}");
    assert!(spent.3 > plain.3 * 1.1 && spent.0 > plain.0 * 2.0, "spent arms shake, and drop the muzzle: {} against {}", spent.3, plain.3);
}

/// The out-of-breath heave beside the game's (`courses8`,
/// `hunger-and-fatigue-eating-food`, recorder 0.14.0; out of breath at
/// full stamina, as it was reported): eating
/// raises the oxygen's capacity from 175 to 340 with the pool where it was,
/// so its level drops from full to 54% with the legs full, and it refills
/// at 5.6 a second. The game's camera heaves from that moment, less as the
/// oxygen comes back, and is still at nine tenths of it, five seconds
/// before the pool is full. Ours is given the recorded level step by step,
/// standing still with the sights down and head bobbing at the
/// recording's 1, the stock AKM in hand as there. Each time the recorded
/// hands change (the food taken out and put away, four times), the game's
/// camera spring is at rest on that frame and its breath starts its loop
/// again: ours is started afresh on the same frames, so no phase is
/// fitted. Skipped where the recording or the install is not there.
#[test]
fn the_out_of_breath_heave_follows_the_oxygen_as_recorded() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../local/traces/courses8/hunger-and-fatigue-eating-food/anything.jsonl");
    let (Ok(text), Some(install)) = (std::fs::read_to_string(path), crate::fixture::install()) else {
        eprintln!("skipped: no recording of courses8's eating or no install");
        return;
    };
    // A number by its field's name, the first of an array's.
    let number = |line: &str, name: &str| -> Option<f32> {
        let key = format!("\"{name}\":");
        let rest = line[line.find(&key)? + key.len()..].trim_start_matches('[');
        rest[..rest.find([',', ']', '}'])?].parse().ok()
    };
    let controller = |line: &str| -> Option<String> {
        let key = "\"hands_controller\":\"";
        let rest = &line[line.find(key)? + key.len()..];
        Some(rest[..rest.find('"')?].to_string())
    };
    // Each frame: its time, the oxygen's level, the camera spring's pitch,
    // and whether the hands changed on it.
    let mut held = None;
    let frames: Vec<(f32, f32, f32, bool)> = text
        .lines()
        .skip(1)
        .filter_map(|line| {
            let hands = controller(line);
            let changed = held.is_some() && hands != held;
            held = hands;
            Some((number(line, "t")?, number(line, "oxygen")? / number(line, "oxygen_capacity")?, number(line, "cam_rot")?, changed))
        })
        .collect();
    drop(text);
    let db = crate::fixture::database().expect("database");
    let hard = HardSettings::load(&install).expect("settings object");
    let engine = EngineSettings::load(&install).expect("engine settings");
    let mut assets = Assets::new(install).expect("manifest");
    let weapon = trp_data::weapons::weapon::assemble(&mut assets, &db, AKM).expect("weapon");
    let data = HandsData::load(&mut assets).expect("player prefab");
    let view = ViewSettings { field_of_view: 74.0, head_bobbing: 1.0, ..ViewSettings::default() };
    // Our camera's pitch at each recorded frame.
    let fresh = || Hands::new(trp_sim::hands_settings(data.clone(), &db, &hard, &engine, &weapon, view).expect("hands settings"));
    let mut hands = fresh();
    let (mut time, mut level) = (0.0f32, 1.0f32);
    let mut ours = Vec::with_capacity(frames.len());
    for &(t, oxygen, _, changed) in &frames {
        while time + DT <= t {
            hands.advance(DT, &HandsInput { wind: trp_sim::Wind { oxygen: level, ..trp_sim::Wind::default() }, ..still() });
            time += DT;
        }
        if changed {
            hands = fresh();
        }
        level = oxygen;
        ours.push(hands.camera_rotation.current[0]);
    }
    let span = |from: f32, to: f32| frames.iter().enumerate().filter(move |(_, f)| f.0 >= from && f.0 < to).map(|(i, _)| i);
    let (count, squares, most) = span(0.0, 60.0).fold((0, 0.0f32, 0.0f32), |(n, s, m), i| {
        let d = ours[i] - frames[i].2;
        (n + 1, s + d * d, m.max(d.abs()))
    });
    println!("{count} frames over the first minute: the camera's pitch {:.4} degrees from the game's at the root mean square, {most:.4} at most", (squares / count as f32).sqrt());
    // The heave by the breath's loop: its depth, the game's and ours.
    let reach = |values: &dyn Fn(usize) -> f32, from: f32| {
        let (low, high) = span(from, from + 3.5).fold((f32::MAX, f32::MIN), |(l, h), i| (l.min(values(i)), h.max(values(i))));
        if low > high { 0.0 } else { high - low }
    };
    let (game, sim) = (|i: usize| frames[i].2, |i: usize| ours[i]);
    let mut from = 17.5f32;
    while from < 52.0 {
        let level = span(from, from + 3.5).last().map_or(1.0, |i| frames[i].1);
        println!("{from:5.1} to {:5.1} s, oxygen {:.2} at the end: the game's heave {:.4} degrees, ours {:.4}", from + 3.5, level, reach(&game, from), reach(&sim, from));
        from += 3.5;
    }
    // The breaths: when each tops out, the game's and ours.
    let peaks = |values: &dyn Fn(usize) -> f32| -> Vec<f32> {
        let mut tops: Vec<f32> = Vec::new();
        for i in span(19.0, 45.0) {
            let t = frames[i].0;
            let highest = span(t - 0.6, t + 0.6).all(|j| values(j) <= values(i));
            if highest && values(i) > 0.005 && tops.last().is_none_or(|&top| t - top > 0.6) {
                tops.push(t);
            }
        }
        tops
    };
    let (game_peaks, sim_peaks) = (peaks(&game), peaks(&sim));
    println!("the breaths' tops, the game's: {game_peaks:.2?}");
    println!("the breaths' tops, ours:       {sim_peaks:.2?}");
    // When each stops: the last frame the camera is off its rest.
    let last = |values: &dyn Fn(usize) -> f32| span(0.0, 60.0).filter(|&i| values(i).abs() > 1e-3).last().map(|i| (frames[i].0, frames[i].1));
    let (game_end, sim_end) = (last(&game).expect("the game's camera heaves"), last(&sim).expect("ours heaves"));
    println!("the heave ends at {:.1} s (oxygen {:.3}) in the game, {:.1} s (oxygen {:.3}) here", game_end.0, game_end.1, sim_end.0, sim_end.1);
    assert!(span(0.0, 19.0).all(|i| frames[i].2 == 0.0 && ours[i] == 0.0), "no heave on full oxygen, in either");
    assert!((game_end.0 - sim_end.0).abs() < 0.1 && game_end.1 > 0.89 && game_end.1 < 0.91, "the heave ends together, at nine tenths of the oxygen");
    assert!((squares / count as f32).sqrt() < 0.003 && most < 0.03, "frame for frame");
    assert!(game_peaks.len() == sim_peaks.len() && game_peaks.iter().zip(&sim_peaks).all(|(g, s)| (g - s).abs() < 0.1), "breath for breath");
}

/// The stiff draw (`DrawElite`: Aim Drills' elite or the class's; skills
/// spec, "The stiff draw"): the sights brought up hold the breath and its
/// random pushes at three tenths of themselves for 2 s, then ease them back
/// to whole over the last second (`BreathEffector.Process`). Two rigs alike
/// but for the buff bring their sights up together, each frame drawn:
/// the weapon's rise and fall is a small part of the other's in the first
/// seconds, and the two are one again once the spring has let the stiff
/// seconds go. Not compared with the game: no recording at elite.
#[test]
fn a_stiff_draw_holds_the_breath_for_three_seconds() {
    let (Some((mut plain, _)), Some((mut stiff, _))) = (rig(), rig()) else { return };
    stiff.set_stiff_draw(true);
    // Rested breaths first, the sights down: the buff does nothing there.
    for hands in [&mut plain, &mut stiff] {
        for _ in 0..(4.0 / DT) as usize {
            hands.advance(DT, &still());
            hands.frame(DT, camera_bone(), None);
        }
    }
    assert_eq!(plain.hands_position.current, stiff.hands_position.current, "nothing before the sights come up");
    // The sights up, eight seconds: the weapon's lift (the breath's own
    // push) and its roll (the random pushes) in each window.
    let windows = [(0.5, 2.0), (6.0, 8.0)];
    let mut spans = [[[f32::MAX, f32::MIN]; 2]; 2];
    let aimed = HandsInput { aiming: true, ..still() };
    for (which, hands) in [&mut plain, &mut stiff].into_iter().enumerate() {
        for index in 0..(8.0 / DT) as usize {
            hands.advance(DT, &aimed);
            hands.frame(DT, camera_bone(), None);
            let since = (index + 1) as f32 * DT;
            for (window, &(from, to)) in windows.iter().enumerate() {
                if (from..to).contains(&since) {
                    let lift = hands.hands_position.current[2];
                    let span = &mut spans[which][window];
                    (span[0], span[1]) = (span[0].min(lift), span[1].max(lift));
                }
            }
        }
    }
    let size = |span: [f32; 2]| span[1] - span[0];
    let early = size(spans[1][0]) / size(spans[0][0]);
    let late = size(spans[1][1]) / size(spans[0][1]);
    println!(
        "the weapon's rise and fall with a stiff draw, of the plain one's: {:.2} from 0.5 to 2 s after the sights came up, {:.2} from 6 to 8 s",
        early, late
    );
    assert!(early < 0.5, "held stiff at first: {early:.2}");
    assert!((late - 1.0).abs() < 0.05, "whole again after: {late:.2}");
}

/// The hold-breath key: with the sights up and standing still it holds
/// the breath at the oxygen's cost, a second press lets it go, and so do
/// the sights coming down and walking off.
#[test]
fn the_breath_is_held_only_standing_with_the_sights_up() {
    let Some((_, mut m)) = rig() else { return };
    let press = |m: &mut Movement, aiming: bool| m.step(DT, &Input { aiming, hold_breath: true, ..Input::default() }, &Flat);
    let wait = |m: &mut Movement, seconds: f32, input: Input| {
        for _ in 0..(seconds / DT) as usize {
            m.step(DT, &input, &Flat);
        }
    };
    let aimed = Input { aiming: true, ..Input::default() };
    press(&mut m, false);
    assert!(!m.wind().held, "not without the sights");
    press(&mut m, true);
    assert!(m.wind().held);
    let oxygen = m.stamina.oxygen.current;
    wait(&mut m, 5.0, aimed);
    println!("five seconds of held breath cost {:.1} of {:.0} oxygen", oxygen - m.stamina.oxygen.current, m.stamina.oxygen.capacity);
    assert!((oxygen - m.stamina.oxygen.current - 20.0).abs() < 0.5, "four a second on rested legs");
    press(&mut m, true);
    assert!(!m.wind().held, "a second press lets it go");
    press(&mut m, true);
    wait(&mut m, 0.2, Input::default());
    assert!(!m.wind().held, "so do the sights coming down");
    press(&mut m, true);
    assert!(m.wind().held);
    wait(&mut m, 1.0, Input { move_dir: [0.0, 1.0], ..aimed });
    assert_eq!(m.state, State::Move);
    assert!(!m.wind().held, "and walking off");
    press(&mut m, true);
    assert!(!m.wind().held, "nor is one taken in a walk");
}
