//! A blade's strokes on the body, their marks and the strokes seen as recorded.

use super::*;

// The knife's strokes seen from outside (grenades-and-knife spec, "The
// knife's strokes seen from outside"): the body's `Weapon` layer.

const BAYONET: &str = "54491bb74bdc2d09088b4567";

/// The body's controller, read from the install.
fn body_data() -> Option<trp_sim::locomotion::body::BodyData> {
    let install = crate::fixture::install()?;
    let mut assets = Assets::new(install).expect("manifest");
    Some(trp_sim::locomotion::body::BodyData::load(&mut assets).expect("body animator"))
}

/// What the body is told each frame of these runs: standing on the ground.
fn standing() -> trp_sim::locomotion::body::BodyInput {
    trp_sim::locomotion::body::BodyInput { level: 1.0, grounded: true, transition_speed: 1.0, sideback_speed: 1.0, vaulting_speed: 1.0, ..Default::default() }
}

/// The body's `Weapon` layer: its state's own name, how far through it (in
/// cycles), and the own name of the state it is entering.
fn weapon_layer(body: &trp_sim::Body) -> (String, f32, Option<String>) {
    let (state, time, next) = body.layer_state("Weapon").expect("the Weapon layer");
    let own = |name: &str| name.rsplit('.').next().unwrap_or(name).to_owned();
    (own(state), time, next.map(own))
}

/// The number a recording keeps a state by: the low 24 bits of the hash of
/// its own name.
fn state_hash(own: &str) -> f32 {
    (trp_data::unity::anim::path_hash(own) & 0x00ff_ffff) as f32
}

/// Each number a blade marks for the body enters its stroke's state on the
/// `Weapon` layer from rest (8 a knife's slash, 12 its stab, 13 and 14 an
/// axe's) or from the put-away's state (8 and 15, the quick strokes), which
/// plays at the swing's speed and goes back to rest (the quick strokes to
/// `out_to_idle`) at its end; at the controller's own swing speed, 0, it
/// stands still.
#[test]
fn a_blade_s_strokes_have_their_states_on_the_body() {
    let Some(data) = body_data() else { return };
    let run = |body: &mut trp_sim::Body, seconds: f32| {
        for _ in 0..(seconds / FRAME).round() as usize {
            body.update(FRAME, &standing());
        }
    };
    let fresh = || {
        let mut body = trp_sim::Body::new(&data, 1.0);
        body.set_point_of_view(trp_sim::PointOfView::ThirdPerson);
        body
    };
    for (action, quick, machine, wanted) in [
        (8, false, "Knife_Anims_StateMachine", "Fire_1_Knife"),
        (12, false, "Knife_Anims_StateMachine", "Alt_Fire_Knife"),
        (13, false, "Axe_Anims_StateMachine", "Fire_1_Axe"),
        (14, false, "Axe_Anims_StateMachine", "Alt_Fire_Axe"),
        (8, true, "Knife_Anims_StateMachine", "quick_Fire_Knife 0"),
        (15, true, "Axe_Anims_StateMachine", "quick_Fire_Axe 0"),
    ] {
        let (length, clip) = data.state_length(&format!("Weapon.{machine}.{wanted}")).expect("the state is read");
        let mut body = fresh();
        run(&mut body, 0.2);
        assert_eq!(weapon_layer(&body).0, "INACTIVE");
        if quick {
            body.set_weapon_action(2);
            run(&mut body, 0.3);
            assert_eq!(weapon_layer(&body).0, "idle_to_out");
        }
        body.set_swing_speed(1.0);
        body.set_weapon_action(action);
        let mut entered = None;
        for frame in 1..60 {
            body.update(FRAME, &standing());
            let (state, _, next) = weapon_layer(&body);
            if state == wanted && next.is_none() {
                entered = Some(frame);
                break;
            }
        }
        let entered = entered.unwrap_or_else(|| panic!("{action}: never in {wanted}: {:?}", weapon_layer(&body)));
        // A stroke's own end mark (10), which the blade's states have and
        // its quick stroke has not; without it rest would enter the stroke
        // again.
        if !quick {
            body.set_weapon_action(10);
        }
        let (_, t0, _) = weapon_layer(&body);
        run(&mut body, 0.25);
        let (state, t1, _) = weapon_layer(&body);
        let back = if quick { "out_to_idle" } else { "INACTIVE" };
        run(&mut body, length + 0.3);
        println!(
            "{action}{}: {wanted} ({clip}, {length:.3} s) entered {entered} frames on, {:.4} of it in 0.25 s; then {}",
            if quick { " from idle_to_out" } else { "" },
            t1 - t0,
            weapon_layer(&body).0
        );
        assert_eq!(state, wanted);
        assert!(((t1 - t0) - 0.25 / length).abs() < 0.01, "{action}: {} against {}", t1 - t0, 0.25 / length);
        assert_eq!(weapon_layer(&body).0, back);
    }
    // Never told a swing's speed, the body's stroke stands at its start;
    // at an exhausted character's (0.65) it plays slower.
    let mut body = fresh();
    body.set_weapon_action(13);
    run(&mut body, 0.5);
    let (state, at, _) = weapon_layer(&body);
    println!("at the controller's own swing speed: {state} at {at}");
    assert!(state == "Fire_1_Axe" && at == 0.0);
    body.set_swing_speed(0.65);
    run(&mut body, 0.25);
    let (length, _) = data.state_length("Weapon.Axe_Anims_StateMachine.Fire_1_Axe").expect("the state is read");
    let (_, slow, _) = weapon_layer(&body);
    println!("at 0.65: {slow:.4} of it in 0.25 s");
    assert!((slow - 0.65 * 0.25 / length).abs() < 0.01);
}

/// A blade's strokes mark for the body the numbers its own animator has
/// (`probe knives`): the bayonet's slash 8 and stab 12, the ice pick's 13
/// and 14 (an axe's), each stroke's end 10, the draw 1, the put-away 2, both
/// quick strokes 8; and the body is told the swing's speed with no glance
/// as each stroke starts, the glance on a hit, the melee speed at the
/// put-away.
#[test]
fn a_blade_s_strokes_mark_their_numbers_for_the_body() {
    let marks = |said: &[(KnifeEvent, f32)]| said.iter().filter_map(|(event, _)| if let KnifeEvent::BodyAction(a) = event { Some(*a) } else { None }).collect::<Vec<_>>();
    for (name, template, slash, stab) in [("bayonet", BAYONET, 8, 12), ("ice pick", ICE_PICK, 13, 14)] {
        let Some((data, props)) = rig(template) else { return };
        let settings = KnifeSettings::of(&props);
        let mut hands = KnifeHands::new(&data, settings);
        let mut clock = 0.0;
        hands.draw();
        let drawn = run_knife(&mut hands, 1.2, &mut clock);
        assert_eq!(marks(&drawn), vec![1], "{name}");

        assert!(hands.strike(Stroke::Slash, true, 0.8));
        let said = run_knife(&mut hands, 1.2, &mut clock);
        assert_eq!(said.iter().take(2).map(|(event, _)| event.clone()).collect::<Vec<_>>(), vec![KnifeEvent::BodyDeflected(false), KnifeEvent::BodySwingSpeed(0.8)]);
        let slash_marks = marks(&said);

        assert!(hands.strike(Stroke::Stab, true, 1.0));
        let mut said = Vec::new();
        for _ in 0..144 {
            clock += FRAME;
            said.extend(hands.step(FRAME).into_iter().map(|event| (event, clock)));
            if hands.live() {
                hands.hit();
            }
        }
        let stab_marks = marks(&said);
        let glanced = said.iter().any(|(event, _)| *event == KnifeEvent::BodyDeflected(true));

        hands.set_melee_speed(0.7);
        hands.hide();
        let said = run_knife(&mut hands, 1.0, &mut clock);
        let away = (marks(&said), said.iter().any(|(event, _)| *event == KnifeEvent::BodySwingSpeed(0.7)));

        let mut quick = KnifeHands::new(&data, settings);
        quick.draw_quick(1.0);
        let said = run_knife(&mut quick, 1.5, &mut 0.0);
        let quick_marks = marks(&said);
        println!("{name}: slash {slash_marks:?}, stab {stab_marks:?} (glanced {glanced}), put away {away:?}, quick {quick_marks:?}");
        assert_eq!(slash_marks, vec![slash, 10], "{name}");
        assert_eq!(stab_marks, vec![stab, 10], "{name}");
        assert!(glanced);
        assert_eq!(away, (vec![2], true), "{name}");
        assert_eq!(quick_marks, vec![8], "{name}");
    }
}

/// One frame of a recording, as far as the body's `Weapon` layer and the
/// knife go.
pub(super) struct Seen {
    pub(super) frame: u32,
    dt: f32,
    controller: String,
    keys: Vec<String>,
    keys_up: Vec<String>,
    layers: Vec<f32>,
    action: i32,
    /// The knife's `Deflected` and `SwingSpeed` (each written only on the
    /// frame it changes).
    deflected: Option<f32>,
    swing: Option<f32>,
}

impl Seen {
    pub(super) fn of(line: &str) -> Option<Self> {
        let after = |name: &str| line.find(&format!("\"{name}\":")).map(|at| &line[at + name.len() + 3..]);
        let number = |name: &str| after(name).and_then(|rest| rest[..rest.find([',', '}', ']'])?].parse::<f32>().ok());
        let list = |name: &str| after(name).and_then(|rest| rest.strip_prefix('[')).map(|rest| &rest[..rest.find(']').unwrap_or(0)]);
        Some(Self {
            frame: number("frame")? as u32,
            dt: number("dt")?,
            controller: after("hands_controller").and_then(|rest| rest.strip_prefix('"')).map(|rest| rest[..rest.find('"').unwrap_or(0)].to_owned()).unwrap_or_default(),
            keys: list("keys_down").map(|keys| keys.split(',').map(|key| key.trim_matches('"').to_owned()).filter(|key| !key.is_empty()).collect()).unwrap_or_default(),
            keys_up: list("keys_up").map(|keys| keys.split(',').map(|key| key.trim_matches('"').to_owned()).filter(|key| !key.is_empty()).collect()).unwrap_or_default(),
            layers: list("layers")?.split(',').filter_map(|n| n.parse().ok()).collect(),
            action: number("p_FirstAction")? as i32,
            deflected: number("hp_Deflected"),
            swing: number("hp_SwingSpeed"),
        })
    }

    /// The `Weapon` layer: its state, how far through it, its weight, the
    /// state it is entering (-1 none).
    pub(super) fn weapon(&self) -> [f32; 4] {
        std::array::from_fn(|k| self.layers[4 + k])
    }

    fn knife(&self) -> bool {
        self.controller == "KnifeController" || self.controller == "QuickKnifeKickController"
    }
}

/// The body's `Weapon` layer set beside the recorded one over each frame a
/// knife's controller holds the hands.
#[derive(Default)]
pub(super) struct Tally {
    frames: usize,
    pub(super) state: usize,
    both: usize,
    /// Frames the recording has in a blade's stroke, and of them ours too.
    strokes: usize,
    strokes_right: usize,
    worst: (f32, u32, String),
    wrong: Vec<String>,
}

/// Runs the body over the frames `a..=b` of a recording, from the `Weapon`
/// layer's recorded state on frame `a`, told what to do either by the
/// recorded `FirstAction` and swing speed (`hands` none) or by a knife's
/// own hands given the recorded buttons, hits and melee speed (the
/// stamina is not run here); tallies the layer against the recording.
fn run_episode(data: &trp_sim::locomotion::body::BodyData, frames: &[Seen], heard: &[Vec<String>], a: usize, b: usize, mut hands: Option<&mut KnifeHands>, tally: &mut Tally) {
    let mut body = trp_sim::Body::new(data, 1.0);
    body.set_point_of_view(trp_sim::PointOfView::ThirdPerson);
    body.start_layers_as(&frames[a].layers, frames.get(a + 1).map(|f| f.layers.as_slice()));
    body.set_weapon_action(frames[a - 1].action);
    let quick = frames[a].controller == "QuickKnifeKickController";
    let mut swing = frames[a].swing.unwrap_or(1.0);
    match hands.as_deref_mut() {
        Some(hands) if quick => hands.draw_quick(swing),
        Some(hands) => hands.draw(),
        None => {}
    }
    const STROKES: [&str; 6] = ["Fire_1_Knife", "Alt_Fire_Knife", "quick_Fire_Knife 0", "Fire_1_Axe", "Alt_Fire_Axe", "quick_Fire_Axe 0"];
    for (k, seen) in frames[a..=b].iter().enumerate() {
        swing = seen.swing.unwrap_or(swing);
        body.update(seen.dt, &standing());
        let (state, time, next) = weapon_layer(&body);
        let [hash, at, _, entering] = seen.weapon();
        let same = state_hash(&state) == hash;
        let same_next = next.as_deref().map_or(-1.0, state_hash) == entering;
        let recorded_stroke = STROKES.iter().any(|s| state_hash(s) == hash);
        tally.frames += 1;
        tally.state += same as usize;
        tally.both += (same && same_next) as usize;
        tally.strokes += recorded_stroke as usize;
        tally.strokes_right += (recorded_stroke && same) as usize;
        if same && (time - at).abs() > tally.worst.0 {
            tally.worst = ((time - at).abs(), seen.frame, state.clone());
        }
        if !same || !same_next {
            tally.wrong.push(format!("{}: ours {state} {time:.4} -> {next:?}, recorded #{hash} {at:.4} -> #{entering}", seen.frame));
        }
        // The hands after the body, as the app steps them: the recorded
        // buttons, a hit where the recording shows the glance. The quick
        // stroke's controller is made after its first frame's update (its
        // `Q FIRE` at time 0 on that frame in the recording, where the
        // knife's draw has left `SPAWN` for `OUT TO IDLE`), so it is not
        // stepped on it.
        match hands.as_deref_mut() {
            Some(_) if quick && k == 0 => {}
            Some(hands) => {
                hands.set_melee_speed(swing);
                // The buttons as the game's input manager hears the keys
                // (the app's `Shoot` a slash, `Aim` a stab), a weapon's
                // slot key putting the knife away.
                for binding in &heard[a + k] {
                    match binding.as_str() {
                        "Shoot" => {
                            hands.strike(Stroke::Slash, true, swing);
                        }
                        "Aim" => {
                            hands.strike(Stroke::Stab, true, swing);
                        }
                        "PrimaryWeaponFirst" | "PrimaryWeaponSecond" | "SecondaryWeapon" => hands.hide(),
                        _ => {}
                    }
                }
                if seen.deflected == Some(1.0) {
                    hands.hit();
                }
                for event in hands.step(seen.dt) {
                    match event {
                        KnifeEvent::BodyAction(action) => body.set_weapon_action(action),
                        KnifeEvent::BodySwingSpeed(speed) => body.set_swing_speed(speed),
                        KnifeEvent::BodyDeflected(deflected) => body.set_deflected(deflected),
                        _ => {}
                    }
                }
            }
            None => {
                body.set_weapon_action(seen.action);
                body.set_swing_speed(swing);
            }
        }
    }
}

/// The body's part in the ice pick's strokes, seen from outside, against
/// `courses2`'s `2-outside-grenade-knife` (recorder 0.10.0, the character
/// seen from outside, the body's layers on every frame): on every frame a
/// knife's controller holds the hands, the `Weapon` layer's state and how
/// far through it, from where the recording has the layer as the knife
/// comes, told once by the recorded `FirstAction` and once by the ice
/// pick's own hands given the recorded buttons. Skipped where the
/// recording or the install is not there.
#[test]
fn the_knife_s_strokes_seen_from_outside_as_recorded() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../local/traces/courses2/2-outside-grenade-knife/outside-grenade-knife.jsonl");
    let Ok(text) = std::fs::read_to_string(path) else {
        eprintln!("skipped: no recording of courses2's knife from outside");
        return;
    };
    let (Some(data), Some((animator, props))) = (body_data(), rig(ICE_PICK)) else { return };
    let frames: Vec<Seen> = text.lines().filter_map(Seen::of).collect();
    // Every binding the game's input manager gives on each frame, from the
    // recorded keys (the install's bindings, each frame's own length).
    let controls = trp_sim::base::bindings::install_controls(&crate::fixture::install().expect("install"));
    let mut preset = trp_sim::base::bindings::Preset::new(&controls);
    let mut held = Vec::new();
    let heard: Vec<Vec<String>> = frames
        .iter()
        .map(|seen| {
            held.extend(seen.keys.iter().filter_map(|key| trp_sim::base::bindings::code(key)));
            held.retain(|key| !seen.keys_up.iter().any(|up| trp_sim::base::bindings::code(up) == Some(*key)));
            let given = preset.update(&|key| held.contains(&key), 0, seen.dt).given;
            given.iter().map(|group| controls.keys[*group].name.clone()).collect()
        })
        .collect();
    for (seen, names) in frames.iter().zip(&heard).filter(|(seen, names)| seen.knife() && !names.is_empty()) {
        println!("  frame {}: keys {:?} give {names:?}", seen.frame, seen.keys);
    }
    // Each run of frames under one knife's controller.
    let mut episodes = Vec::new();
    let mut k = 1;
    while k < frames.len() {
        if frames[k].knife() {
            let a = k;
            while k + 1 < frames.len() && frames[k + 1].controller == frames[a].controller {
                k += 1;
            }
            episodes.push((a, k));
        }
        k += 1;
    }
    let mut by_recorded = Tally::default();
    let mut by_hands = Tally::default();
    for &(a, b) in &episodes {
        run_episode(&data, &frames, &heard, a, b, None, &mut by_recorded);
        let mut hands = KnifeHands::new(&animator, KnifeSettings::of(&props));
        run_episode(&data, &frames, &heard, a, b, Some(&mut hands), &mut by_hands);
        println!("{} frames {} to {}", frames[a].controller, frames[a].frame, frames[b].frame);
    }
    for (name, tally) in [("by the recorded FirstAction", &by_recorded), ("by the ice pick's own hands", &by_hands)] {
        println!(
            "{name}: {} knife frames, the layer's state the game's on {} ({} with the state it enters too); in a stroke's state {} recorded, {} ours; largest gap in how far through {:.6} (frame {}, {})",
            tally.frames, tally.state, tally.both, tally.strokes, tally.strokes_right, tally.worst.0, tally.worst.1, tally.worst.2
        );
        for line in tally.wrong.iter().take(60) {
            println!("  {line}");
        }
    }
    // Two draws and two quick strokes, 1,874 frames, 1,005 of them in a
    // stroke's state; every one the game's, state, state entered and time.
    assert_eq!(episodes.len(), 4, "two drawn, two quick");
    for tally in [&by_recorded, &by_hands] {
        assert_eq!((tally.frames, tally.strokes), (1874, 1005));
        assert_eq!((tally.state, tally.both, tally.strokes_right), (tally.frames, tally.frames, tally.strokes), "{:?}", tally.wrong.first());
        assert!(tally.worst.0 < 1e-4, "{:?}", tally.worst);
    }
}
