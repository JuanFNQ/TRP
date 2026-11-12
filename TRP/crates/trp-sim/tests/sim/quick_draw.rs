//! Tests of the quick put-away of a firearm for the holster's weapon.
//! After `docs/spec/stamina.md`, "Wounds and the fast weapon switch", with the
//! install's M4A1, the rifle of `courses8`'s `pistol-quickdraw`, which put
//! it away quick four times. These pass trivially where no install is
//! configured.

use trp_data::setup::hard_settings::HardSettings;
use trp_data::unity::Assets;
use trp_sim::{Firearm, FirearmEvent};

const M4A1: &str = "5447a9cd4bdc2dbd208b4567";
/// The FN Five-seveN, the holster's pistol of `pistol-quickdraw` and of
/// `quickaim`.
const FIVE_SEVEN: &str = "5d3eb3b0a4b93615055e84d2";
/// The recording's frames: 120 a second.
const DT: f32 = 1.0 / 120.0;

fn m4a1() -> Option<Firearm> {
    weapon(M4A1)
}

/// A weapon of the install's at rest in hand, as it is sold.
fn weapon(template: &str) -> Option<Firearm> {
    let install = crate::fixture::install()?;
    let db = crate::fixture::database().expect("database");
    let hard = HardSettings::load(&install).expect("settings object");
    let mut assets = Assets::new(install).expect("manifest");
    let weapon = trp_data::weapons::weapon::assemble(&mut assets, db, template).expect("the weapon");
    let animator = trp_sim::weapons::weapon_animator::WeaponAnimatorData::load(&assets, &weapon).expect("its animator");
    let mut firearm = Firearm::new(trp_sim::firearm_settings(&assets, db, &hard, &weapon).expect("its settings"), &animator);
    firearm.start_drawn();
    for _ in 0..30 {
        firearm.step(DT);
    }
    Some(firearm)
}

/// Put away quick from rest, it is told so (`FastHide`) and takes its
/// quick speed at once, says the arms pay once, plays `FAST IDLE TO OUT`,
/// and is out of the hands 0.502 s on, as the game's (1.2275 to 1.7294 s
/// in the recording, at the speed 1 of skill level 0).
#[test]
fn a_quick_put_away_plays_its_own_state_and_the_arms_pay_as_it_begins() {
    let Some(mut firearm) = m4a1() else { return };
    firearm.hide_quick(1.0);
    assert!(firearm.take_fast_switch(), "the arms pay as the put-away begins");
    assert!(!firearm.take_fast_switch(), "once");
    assert!(firearm.animator().flag("FastHide"));
    assert_eq!(firearm.animator().get("SpeedDraw"), 1.0);
    let mut states = Vec::new();
    let mut frames = 0;
    loop {
        frames += 1;
        let events = firearm.step(DT);
        let name = firearm.animator().state().0.to_owned();
        if states.last() != Some(&name) {
            states.push(name);
        }
        if events.contains(&FirearmEvent::Hidden) || frames > 240 {
            break;
        }
    }
    assert_eq!(states.first().map(String::as_str), Some("Hands.FAST IDLE TO OUT"), "{states:?}");
    let took = frames as f32 * DT;
    assert!((took - 0.502).abs() <= 2.0 * DT, "out of the hands after {took:.3} s; the game's 0.502");
}

/// Asked for in a draw, the quick put-away begins at the draw's end
/// (`SpawnOperation`'s put-away asked for meanwhile): the arms pay then,
/// not at the key, and the quick speed is taken then, after the idling's
/// start has given the skill's back.
#[test]
fn a_quick_put_away_asked_in_a_draw_begins_with_its_end() {
    let Some(mut firearm) = m4a1() else { return };
    firearm.hide(false);
    for _ in 0..240 {
        if firearm.step(DT).contains(&FirearmEvent::Hidden) {
            break;
        }
    }
    firearm.draw();
    firearm.step(DT);
    firearm.hide_quick(1.25);
    assert!(!firearm.take_fast_switch(), "nothing paid while it is drawn");
    let mut paid_at = None;
    for frame in 0..240 {
        let events = firearm.step(DT);
        if events.contains(&FirearmEvent::FastSwitch) {
            assert!(events.contains(&FirearmEvent::Appeared), "paid on the draw's end: {events:?}");
            paid_at = Some(frame);
            break;
        }
    }
    assert!(paid_at.is_some(), "the quick put-away began");
    assert_eq!(firearm.animator().get("SpeedDraw"), 1.25);
    assert!(firearm.animator().flag("FastHide"));
}

/// A draw's own speeds last until the weapon idles: its idling's start
/// gives the skill's back (`Idling.Start`, `SetAnimatorAndProceduralValues`;
/// the recording's pistol drawn at 1.5 and at 1 from its idle on).
#[test]
fn a_draw_s_quick_speed_lasts_until_the_weapon_idles() {
    let Some(mut firearm) = m4a1() else { return };
    firearm.hide(false);
    for _ in 0..240 {
        if firearm.step(DT).contains(&FirearmEvent::Hidden) {
            break;
        }
    }
    firearm.draw();
    firearm.set_animator_speeds(1.0, 1.5);
    let mut appeared = false;
    for _ in 0..240 {
        let events = firearm.step(DT);
        if events.contains(&FirearmEvent::Appeared) {
            appeared = true;
            break;
        }
        assert_eq!(firearm.animator().get("SpeedDraw"), 1.5, "quick through the draw");
    }
    assert!(appeared);
    assert_eq!(firearm.animator().get("SpeedDraw"), 1.0, "the skill's (level 0) once it idles");
}

/// Steps until the weapon is out of the hands; every event on the way.
fn until_hidden(firearm: &mut Firearm) -> Vec<FirearmEvent> {
    let mut said = Vec::new();
    for _ in 0..240 {
        let events = firearm.step(DT);
        let hidden = events.contains(&FirearmEvent::Hidden);
        said.extend(events);
        if hidden {
            break;
        }
    }
    said
}

/// Draws the weapon with the screen shut, the draw's end to turn the
/// sights over or not and the movement to let them up or not, a put-away
/// asked in the draw or not; the events of the frame it appeared on.
fn drawn(firearm: &mut Firearm, turn: bool, allowed: bool, hide_in_it: bool) -> Vec<FirearmEvent> {
    firearm.hide(false);
    until_hidden(firearm);
    firearm.draw();
    firearm.set_appearing(false, turn, allowed);
    firearm.step(DT);
    if hide_in_it {
        firearm.hide(false);
    }
    for _ in 0..240 {
        let events = firearm.step(DT);
        if events.contains(&FirearmEvent::Appeared) {
            return events;
        }
    }
    panic!("never appeared");
}

/// Put away from rest, the weapon says how its sights stood
/// (`Idling.HideWeapon`: the player's `PreviousWeaponAimState`), quick or
/// not, and the sights go down as it goes.
#[test]
fn a_put_away_from_rest_keeps_the_aim() {
    let Some(mut firearm) = m4a1() else { return };
    firearm.set_aim(true, true);
    assert!(firearm.aiming());
    firearm.hide_quick(1.0);
    let said = until_hidden(&mut firearm);
    assert_eq!(said.iter().filter(|e| matches!(e, FirearmEvent::AimKept(_))).collect::<Vec<_>>(), [&FirearmEvent::AimKept(true)]);
    assert!(!firearm.aiming());
    let Some(mut firearm) = m4a1() else { return };
    firearm.hide(false);
    assert!(until_hidden(&mut firearm).contains(&FirearmEvent::AimKept(false)));
}

/// A put-away asked in a shot is made straight from the shot's end
/// (`FireOperationBase.HideWeapon`: `Remove.Start`), not from rest: no aim
/// is kept, and the host keeps what it had.
#[test]
fn a_put_away_planned_in_a_shot_keeps_no_aim() {
    let Some(mut firearm) = m4a1() else { return };
    firearm.set_aim(true, true);
    firearm.set_trigger(true, true);
    let mut shot = false;
    for _ in 0..60 {
        if firearm.step(DT).iter().any(|e| matches!(e, FirearmEvent::Shot { .. })) {
            shot = true;
            break;
        }
    }
    assert!(shot, "a shot made");
    firearm.hide(false);
    let said = until_hidden(&mut firearm);
    assert!(said.contains(&FirearmEvent::Hidden), "put away");
    assert!(!said.iter().any(|e| matches!(e, FirearmEvent::AimKept(_))), "{said:?}");
}

/// The draw's end turns the sights over where the host says so
/// (`SpawnOperation.WeaponAppeared`: `ToggleAim` with `FastSlotSelection`
/// and `PreviousWeaponAimState`): up on the frame it appears, with the aim's
/// sounds (`Event::Aimed`); not where the movement refuses them, nor where
/// the host does not ask.
#[test]
fn a_draw_comes_up_aimed_where_the_host_says_so() {
    let Some(mut firearm) = m4a1() else { return };
    let events = drawn(&mut firearm, true, true, false);
    assert!(events.contains(&FirearmEvent::Aimed(true)), "{events:?}");
    assert!(firearm.aiming());
    for (turn, allowed) in [(true, false), (false, true)] {
        let Some(mut firearm) = m4a1() else { return };
        let events = drawn(&mut firearm, turn, allowed, false);
        assert!(!events.iter().any(|e| matches!(e, FirearmEvent::Aimed(_))), "{turn} {allowed}: {events:?}");
        assert!(!firearm.aiming());
    }
}

/// The screen as it stands is told first at the draw's end, as the game's
/// owner does (`SetHandsController`, from the draw's callback), and its
/// putting the sights down comes before they are turned up; a host that
/// tells it after the step, as before, puts them down again.
#[test]
fn a_draw_is_told_the_screen_before_its_sights_turn() {
    let Some(mut firearm) = m4a1() else { return };
    let events = drawn(&mut firearm, true, true, false);
    let told = events.iter().position(|e| matches!(e, FirearmEvent::LeftStance(trp_sim::weapons::left_stance::Call::Inventory(false))));
    let aimed = events.iter().position(|e| *e == FirearmEvent::Aimed(true));
    assert!(told.is_some() && told < aimed, "{events:?}");
    assert!(firearm.aiming());
    firearm.set_inventory(false);
    assert!(!firearm.aiming(), "told after, the rest puts them down (`Idling.SetInventoryOpened`)");
}

/// `quickaim` (recorder 0.16.0: the AKM put away
/// quick with its sights up, the Five-seveN drawn): the pistol's draw began
/// on frame 267 (2.338 s), went through `OUT TO IDLE` at `SpeedDraw` 1.5,
/// and its rest began on frame 342 (2.990 s, 0.652 s on, the state at 0.96)
/// with its sights up on that frame. The simulation's: up on the frame it
/// appears, within a frame of the game's.
#[test]
fn the_recorded_pistol_comes_up_aimed_as_its_rest_begins() {
    let Some(mut pistol) = weapon(FIVE_SEVEN) else { return };
    pistol.hide(false);
    until_hidden(&mut pistol);
    pistol.draw();
    pistol.set_animator_speeds(1.0, 1.5);
    pistol.set_appearing(false, true, true);
    let mut took = 0.0;
    for _ in 0..240 {
        let events = pistol.step(DT);
        took += DT;
        if events.contains(&FirearmEvent::Appeared) {
            assert!(events.contains(&FirearmEvent::Aimed(true)), "{events:?}");
            assert!(pistol.aiming());
            eprintln!("the pistol up after {took:.4} s; the game's 0.652");
            assert!((took - 0.652f32).abs() <= DT + 1e-4, "up after {took:.4} s; the game's 0.652");
            return;
        }
    }
    panic!("never appeared");
}

/// A put-away asked in the draw goes first, from rest with the sights down,
/// which sets the game's flag down before it is read: nothing comes up.
#[test]
fn a_draw_put_away_in_it_does_not_come_up_aimed() {
    let Some(mut firearm) = m4a1() else { return };
    let events = drawn(&mut firearm, true, true, true);
    assert!(events.contains(&FirearmEvent::AimKept(false)), "{events:?}");
    assert!(!events.contains(&FirearmEvent::Aimed(true)), "{events:?}");
    assert!(!firearm.aiming());
}

/// One recorded frame of `pistol-quickdraw` for the hand shake: its time,
/// the weapon motion's mask, the arms' stamina, the hands' level (the lower
/// of the arms' and the legs' shares), the oxygen's level, the sights, and
/// the hands' rotation spring.
struct Frame {
    t: f32,
    mask: u32,
    arms: f32,
    level: f32,
    oxygen: f32,
    aiming: bool,
    rot: [f32; 3],
}

fn frames_of(text: &str) -> Vec<Frame> {
    let number = |line: &str, name: &str| -> Option<f32> {
        let key = format!("\"{name}\":");
        let rest = line[line.find(&key)? + key.len()..].trim_start_matches('[');
        rest[..rest.find([',', ']', '}'])?].parse().ok()
    };
    let vector = |line: &str, name: &str| -> Option<[f32; 3]> {
        let key = format!("\"{name}\":[");
        let rest = &line[line.find(&key)? + key.len()..];
        let mut parts = rest[..rest.find(']')?].split(',').map(|v| v.parse::<f32>().ok());
        Some([parts.next()??, parts.next()??, parts.next()??])
    };
    text.lines()
        .skip(1)
        .filter_map(|line| {
            let arms = number(line, "hands_stamina")?;
            let level = (arms / number(line, "hands_capacity")?).min(number(line, "stamina")? / number(line, "stamina_capacity")?);
            Some(Frame {
                t: number(line, "t")?,
                mask: number(line, "mask")? as u32,
                arms,
                level,
                oxygen: number(line, "oxygen")? / number(line, "oxygen_capacity")?,
                aiming: line.contains("\"aiming\":true"),
                rot: vector(line, "hands_rot")?,
            })
        })
        .collect()
}

/// The weapon motion's physics step.
const STEP: f32 = 1.0 / 60.0;

/// The quick draw's hand shake beside the game's (`courses8`'s
/// `pistol-quickdraw`, recorder 0.14.0, by hand, standing still
/// with no look; stamina spec, "The quick draw's hand shake"). At the
/// fourth quick draw the arms are out: the game's weapon motion turns its
/// `HandShake` bit on (the mask 125 to 253) on the frame the Five-seveN's
/// rig is made and the legs pay the draw's charge, 21.435 s, and it is
/// still on when the take ends, 28.68 s, as the rule has it: 10 s by what
/// the arms lacked over the two charges against 30, and the arms (6.69,
/// then 0) could pay neither, so each lacked the whole 15: 10 s, its fade
/// from 26.435 s. The shake is random, so ours is set beside the game's by its reach:
/// the pistol's motion given the recorded stamina, oxygen and sights step
/// by step, its rig tied where the game's spring is put back to rest, and
/// the shake started on the game's frame, over several seeds, with the
/// shake and without. Also checks the figures read off the install.
#[test]
fn the_quick_draw_s_hand_shake_beside_the_recording() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../local/traces/courses8/pistol-quickdraw/anything.jsonl");
    let (Ok(text), Some(install)) = (std::fs::read_to_string(path), crate::fixture::install()) else {
        eprintln!("skipped: no recording of courses8's quick draw or no install");
        return;
    };
    let frames = frames_of(&text);
    drop(text);
    let db = crate::fixture::database().expect("database");
    let hard = HardSettings::load(&install).expect("settings object");
    let engine = trp_data::setup::engine::EngineSettings::load(&install).expect("engine settings");
    let mut assets = Assets::new(install).expect("manifest");
    let pistol = trp_data::weapons::weapon::assemble(&mut assets, db, FIVE_SEVEN).expect("the Five-seveN");
    let data = trp_data::character::player::HandsData::load(&mut assets).expect("the player prefab");

    // The figures, as the probe read them (`probe script ...
    // ProceduralWeaponAnimation`) and as `globals.json` has them.
    let shake = &data.hand_shake;
    assert!(!shake.uses_params && shake.curves == 0, "the install's shake uses neither its curves nor its parameters");
    assert_eq!((shake.intensity, shake.energy_multiplier, shake.damping, shake.damping_start_percent), (3.0, 1.0, true, 50.0));
    assert_eq!((shake.lower_limit, shake.fracture_limit), (0.05, 0.3));
    for random in [shake.hands_x, shake.hands_y].iter().chain(&shake.tremor) {
        assert_eq!((random.amplitude, random.delay, random.hardness), (0.1, [0.3, 0.6], 0.1));
    }
    let quick = trp_sim::weapons::quick_draw::QuickDraw::read(&db.config).expect("WeaponFastDrawSettings");
    assert_eq!((quick.shake_longest, quick.shake_strength), (10.0, 3.0));
    let cost = db.config["Stamina"]["WeaponFastSwitchConsumption"].as_f64().expect("the charge") as f32;

    // The game's: where its bit comes on, and what the arms lacked at the
    // draw's charge and at the put-away's before it (the last fall of the
    // arms before the shake): the whole charge where they could not pay
    // it (`Stamina::fast_switch`).
    let on = frames.iter().position(|f| f.mask & 128 != 0).expect("the game's shake");
    assert!(frames[on..].iter().all(|f| f.mask & 128 != 0), "on to the take's end");
    let lack_at = |arms: f32| if arms < cost { cost } else { 0.0 };
    let fall = (1..on).rev().find(|&i| frames[i].arms < frames[i - 1].arms - 1.0).expect("the put-away's charge");
    let (put_away, drawn) = (frames[fall - 1].arms, frames[on - 1].arms);
    let lack = lack_at(put_away) + lack_at(drawn);
    let duration = quick.shake_for(lack, 2.0 * cost);
    let (start, end) = (frames[on].t, frames.last().expect("frames").t);
    println!(
        "the game's shake on at {start:.3} s (frame {on}); the arms had {put_away:.2} at the put-away's charge ({:.3} s) and {drawn:.2} at the draw's, lacking {lack:.0} in all: {duration:.2} s, to {:.2} s; the take ends at {end:.2} s",
        frames[fall].t,
        start + duration
    );
    assert!((start - 21.435).abs() < 0.001 && (duration - 10.0).abs() < 1e-4 && start + duration > end);

    // Ours, frame by frame, with the shake and without.
    let view = trp_sim::ViewSettings { field_of_view: 74.0, head_bobbing: 1.0, ..trp_sim::ViewSettings::default() };
    let replay = |seed: u64, shaken: bool| -> (Vec<[f32; 3]>, bool) {
        let mut settings = trp_sim::hands_settings(data.clone(), db, &hard, &engine, &pistol, view).expect("its motion");
        settings.seed = seed;
        let mut hands = trp_sim::Hands::new(settings);
        let (mut time, mut ours, mut last) = (0.0f32, Vec::with_capacity(frames.len()), [1.0f32; 3]);
        let mut input = trp_sim::HandsInput { idle: true, pose_level: 1.0, smoothed_pose_level: 1.0, ..trp_sim::HandsInput::default() };
        let mut on_at_end = false;
        for (index, frame) in frames.iter().enumerate() {
            while time + STEP <= frame.t {
                hands.advance(STEP, &input);
                time += STEP;
            }
            // The game's rig made anew: its spring at rest on this frame.
            if frame.rot == [0.0; 3] && last != [0.0; 3] {
                hands.tie_to_new_rig();
            }
            if index == on && shaken {
                hands.start_hand_shake(duration, quick.shake_strength);
            }
            last = frame.rot;
            input.wind = trp_sim::Wind { stamina: frame.level.clamp(0.05, 1.0), oxygen: frame.oxygen, ..trp_sim::Wind::default() };
            input.aiming = frame.aiming;
            ours.push(hands.hands_rotation.current);
            on_at_end = hands.hand_shake_on();
        }
        (ours, on_at_end)
    };
    // The reach of the hands' spring in a span: the pitch's mean and
    // highest (the sag), and the roll's and the yaw's spread.
    let spans = [("from the hip", start, 22.28), ("aimed", 22.28, 27.35), ("the hip again", 27.35, end + 0.01)];
    let reach = |rot: &dyn Fn(usize) -> [f32; 3], from: f32, to: f32| -> [f32; 4] {
        let picked: Vec<usize> = (0..frames.len()).filter(|&i| frames[i].t >= from && frames[i].t < to).collect();
        let n = picked.len() as f32;
        let mean = picked.iter().map(|&i| rot(i)[0]).sum::<f32>() / n;
        let most = picked.iter().map(|&i| rot(i)[0]).fold(f32::MIN, f32::max);
        let spread = |c: usize| (picked.iter().map(|&i| rot(i)[c].powi(2)).sum::<f32>() / n).sqrt();
        [mean, most, spread(1), spread(2)]
    };
    let game: Vec<[f32; 4]> = spans.iter().map(|&(_, from, to)| reach(&|i| frames[i].rot, from, to)).collect();
    // Each figure of each span over the seeds, shaken and not (`TRP_SHAKE_SEEDS`
    // sets how many; 100 by default).
    let seeds: u64 = std::env::var("TRP_SHAKE_SEEDS").ok().and_then(|n| n.parse().ok()).unwrap_or(100);
    let mut runs = [vec![vec![Vec::new(); 4]; spans.len()], vec![vec![Vec::new(); 4]; spans.len()]];
    for seed in 1..=seeds {
        for (shaken, figures) in [true, false].into_iter().zip(runs.iter_mut()) {
            let (ours, on_at_end) = replay(seed, shaken);
            assert_eq!(on_at_end, shaken, "ours still shakes at the take's end where it was started");
            for (span, &(_, from, to)) in spans.iter().enumerate() {
                for (k, value) in reach(&|i| ours[i], from, to).into_iter().enumerate() {
                    figures[span][k].push(value);
                }
            }
        }
    }
    let names = ["the pitch's mean", "the pitch's highest", "the roll's spread", "the yaw's spread"];
    // A figure's least and most over the seeds, and how many of them reach the game's.
    let bounds = |values: &[f32], game: f32| {
        let (low, high) = values.iter().fold((f32::MAX, f32::MIN), |(l, h), &v| (l.min(v), h.max(v)));
        (low, high, values.iter().filter(|&&v| v >= game).count())
    };
    let mut within = [[false; 4]; 3];
    for (span, &(name, from, to)) in spans.iter().enumerate() {
        println!("{name}, {from:.2} to {to:.2} s:");
        for (k, label) in names.iter().enumerate() {
            let g = game[span][k];
            let ((wl, wh, wn), (ol, oh, on)) = (bounds(&runs[0][span][k], g), bounds(&runs[1][span][k], g));
            println!("  {label:22} the game's {g:7.3}; ours shaken {wl:7.3} to {wh:7.3} ({wn} of {seeds} reach it); unshaken {ol:7.3} to {oh:7.3} ({on})");
            within[span][k] = (wl..=wh).contains(&g);
        }
    }
    // Aimed and from the hip after: every figure of the game's within ours
    // shaken, and the sag and the roll out of reach without the shake.
    for span in [1, 2] {
        assert!(within[span].iter().all(|&w| w), "{}: the game's figures within ours shaken", spans[span].0);
        let unshaken_top = |k: usize| runs[1][span][k].iter().copied().fold(f32::MIN, f32::max);
        assert!((0..3).all(|k| game[span][k] > unshaken_top(k)), "{}: beyond ours unshaken", spans[span].0);
    }
}
