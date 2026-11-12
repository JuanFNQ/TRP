//! The figure run against the three recorded takes.

use super::*;

/// One recorded frame of the figure.
pub(super) struct Seen {
    pub(super) frame: i64,
    pub(super) age: f32,
    pub(super) body: Vec<Vec<f32>>,
    pub(super) gui: String,
    pub(super) curves: Vec<f32>,
    pub(super) weapon: Option<Vec<f32>>,
    pub(super) bones: BTreeMap<String, [f32; 7]>,
    events: Vec<Json>,
}

impl Seen {
    pub(super) fn of(line: &Json) -> Option<Self> {
        if line.get("shown").and_then(Json::as_bool) != Some(true) {
            return None;
        }
        let numbers = |v: &Json| -> Vec<f32> { v.as_array().map(|list| list.iter().filter_map(Json::as_f64).map(|n| n as f32).collect()).unwrap_or_default() };
        Some(Self {
            frame: line.get("frame")?.as_i64()?,
            age: line.get("age_s")?.as_f64()? as f32,
            body: line.get("body_layers")?.as_array()?.iter().map(numbers).collect(),
            gui: line.get("gui_state").and_then(Json::as_str).unwrap_or("?").to_owned(),
            curves: numbers(line.get("curves")?),
            weapon: line.get("weapon_layers").and_then(Json::as_array).and_then(|layers| layers.iter().map(numbers).find(|layer| layer.first() == Some(&1.0))),
            bones: line
                .get("bones")
                .and_then(Json::as_object)
                .map(|bones| bones.iter().filter_map(|(key, v)| Some((key.clone(), numbers(v).try_into().ok()?))).collect())
                .unwrap_or_default(),
            events: line.get("events").and_then(Json::as_array).cloned().unwrap_or_default(),
        })
    }
}

/// The recorded bone a key names, ours beside it.
fn ours(bones: &FigureBones, key: &str) -> Option<Placed> {
    Some(match key {
        "head" => bones.head,
        "neck" => bones.neck,
        "ribcage" => bones.ribcage,
        "r_palm" => bones.palms[0],
        "l_palm" => bones.palms[1],
        "r_foot" => bones.feet[0],
        "l_foot" => bones.feet[1],
        "r_bend" => bones.goals[0],
        "l_bend" => bones.goals[1],
        "r_marker" => bones.markers[0]?,
        "l_marker" => bones.markers[1]?,
        "weapon_3rd" => bones.weapon_3rd,
        "weapon_root_anim" => bones.weapon_root_anim?,
        "weapon" => bones.weapon?,
        _ => return None,
    })
}

/// The largest of something, and where.
#[derive(Default)]
pub(super) struct Worst {
    value: f32,
    pub(super) frame: i64,
    pub(super) what: String,
}

impl Worst {
    fn take(&mut self, value: f32, frame: i64, what: impl FnOnce() -> String) {
        if value > self.value {
            *self = Self { value, frame, what: what() };
        }
    }
}

/// What one take came to.
#[derive(Default)]
pub(super) struct Tally {
    frames: usize,
    /// Frames on which every recorded layer of the body had our state and
    /// the state it enters.
    layers_same: usize,
    layer_time: Worst,
    /// The `GUI` layer's own, in seconds.
    gui_time: Worst,
    gui_same: usize,
    weapon_same: usize,
    weapon_frames: usize,
    weapon_time: Worst,
    pub(super) curve: Worst,
    bone: Worst,
    turn: Worst,
    by_bone: BTreeMap<String, f32>,
    picks_recorded: Vec<(i64, String)>,
    picks_ours: Vec<i64>,
    /// Frames on which what reached the poser (the body's events, the
    /// weapon's marks with their texts) was the recorded, in its order; and
    /// how many of each came in the whole take, recorded and ours.
    events_same: usize,
    events_recorded: BTreeMap<String, usize>,
    events_ours: BTreeMap<String, usize>,
    wrong: Vec<String>,
}

/// Each frame's `dt` in a trace, by its `frame`.
fn frame_times(path: &str) -> BTreeMap<i64, f32> {
    let Ok(file) = std::fs::File::open(path) else { return BTreeMap::new() };
    let number = |line: &str, key: &str| -> Option<f64> {
        let at = line.find(key)? + key.len();
        let rest = &line[at..];
        let end = rest.find([',', '}']).unwrap_or(rest.len());
        rest[..end].parse().ok()
    };
    std::io::BufReader::new(file)
        .lines()
        .map_while(Result::ok)
        .filter_map(|line| Some((number(&line, "\"frame\":")? as i64, number(&line, "\"dt\":")? as f32)))
        .collect()
}

/// An event as the recorder writes it, as ours.
fn recorded_event(event: &Json) -> FigureEvent {
    let text = event.get("param").and_then(Json::as_array).and_then(|param| param.get(4)).and_then(Json::as_str).unwrap_or("");
    FigureEvent { weapon: event.get("on").and_then(Json::as_str) == Some("weapon"), name: event.get("name").and_then(Json::as_str).unwrap_or("?").to_owned(), text: text.to_owned() }
}

pub(super) fn said(event: &FigureEvent) -> String {
    let on = if event.weapon { "weapon" } else { "body" };
    if event.text.is_empty() {
        format!("{on} {}", event.name)
    } else {
        format!("{on} {} {}", event.name, event.text)
    }
}

/// One take: the figure made as the recording's, its picks steered from the
/// recorded ones, stepped on each recorded frame's time; every frame set
/// beside the recording, weapon moves and all.
fn run_take(data: &FigureData, assets: &mut Assets, db: &Database, dir: &str, name: &str) -> Option<Tally> {
    let figure_path = format!("{dir}/{name}.figure.jsonl");
    let Ok(text) = std::fs::read_to_string(&figure_path) else {
        eprintln!("skipped: no recording {figure_path}");
        return None;
    };
    let lines: Vec<Json> = text.lines().filter_map(|line| serde_json::from_str(line).ok()).collect();
    let made = lines.iter().find_map(|line| line.get("made")).expect("a made line");
    let frames: Vec<Seen> = lines.iter().filter_map(Seen::of).collect();
    let item = made.get("item").and_then(Json::as_str).unwrap_or("");
    let kind = if made.get("item_class").and_then(Json::as_str) == Some("EmptyHands") { kind::NOTHING } else { kind_of(db, item) };
    let ui = frames[0].body.iter().find(|layer| layer[0] == 11.0).map(|_| ());
    assert!(ui.is_some(), "the GUI layer recorded");
    // The weapon as the trace's header lists the equipment.
    let built = (kind != kind::NOTHING).then(|| {
        let trace = std::fs::File::open(format!("{dir}/{name}.jsonl")).expect("the trace");
        let mut first = String::new();
        std::io::BufReader::new(trace).read_line(&mut first).expect("its header");
        let header: Json = serde_json::from_str(&first).expect("a header");
        let preset = trp_data::setup::database::Preset::recorded(&header, item).expect("the weapon in the header");
        let weapon = trp_data::weapons::weapon::assemble_preset(assets, db, &preset).expect("the recorded build");
        let chamber = made.get("chamber").and_then(Json::as_u64).unwrap_or(0) as u32;
        let folded = made.get("folded").and_then(Json::as_bool).unwrap_or(false);
        let loaded = Loaded::of(db, &weapon, chamber, folded);
        let flag = |key: &str| made.get(key).and_then(Json::as_bool);
        assert_eq!((Some(loaded.can_reload), Some(loaded.can_check_chamber)), (flag("can_reload"), flag("can_check_chamber")), "{name}: what the weapon can do");
        (FigureWeaponData::load(assets, &weapon).expect("the weapon in the figure's hands"), loaded)
    });
    let mut figure = Figure::new(data, kind, built.as_ref().map(|(held, loaded)| (held, *loaded)), 0);
    let moves: Vec<String> = made.get("weapon_moves").and_then(Json::as_array).map(|list| list.iter().filter_map(Json::as_str).map(str::to_owned).collect()).unwrap_or_default();
    assert_eq!(figure.weapon_moves(), moves.iter().map(String::as_str).collect::<Vec<_>>().as_slice(), "{name}: the weapon's moves");
    let mut tally = Tally::default();
    // Every recorded pick, in order, steered in.
    for seen in &frames {
        for event in seen.events.iter().filter(|e| e.get("name").and_then(Json::as_str) == Some("OnRandomizeAnimation")) {
            let pick = event.get("pick").and_then(Json::as_str).unwrap_or("idle");
            let number = |key: &str| event.get(key).and_then(Json::as_i64).unwrap_or(0);
            figure.steer(match pick {
                "random" => Pick::Random(number("ui_random_transition") as i32),
                "weapon_move" => Pick::WeaponMove(number("weapon_move") as usize),
                _ => Pick::Idle,
            });
            tally.picks_recorded.push((seen.frame, match pick {
                "random" => format!("random {}", number("ui_random_transition")),
                "weapon_move" => format!("weapon move {} ({})", number("weapon_move"), event.get("weapon_move_trigger").and_then(Json::as_str).unwrap_or("?")),
                other => other.to_owned(),
            }));
        }
    }
    let curve_names = ["Weapon_3rd", "Hand_Left", "Hand_Right", "Elbow_Left", "Elbow_Right"];
    // Each frame's time as the engine gave it (the trace's `dt`, by its
    // frame): the figure's own age is a difference of two clocks of the
    // engine's single precision, a hundredth of a millisecond coarse this far
    // into a run, which over the four frames of a transition of the weapon's
    // falls short of its end where the engine's frames reach it.
    let deltas = frame_times(&format!("{dir}/{name}.jsonl"));
    let mut previous = frames[0].age;
    for (k, seen) in frames.iter().enumerate() {
        if k > 0 {
            figure.step(deltas.get(&seen.frame).copied().unwrap_or(seen.age - previous));
            previous = seen.age;
            if figure.picks().len() > tally.picks_ours.len() {
                tally.picks_ours.push(seen.frame);
            }
        }
        tally.frames += 1;
        // The body's layers.
        let mut all = true;
        for recorded in &seen.body {
            let index = recorded[0] as usize;
            let (state, time, next) = layer(&figure, index);
            let same = hash(&state) as f32 == recorded[1];
            let same_next = next.as_deref().map_or(-1.0, |n| hash(n) as f32) == recorded[5];
            if same {
                tally.layer_time.take((time - recorded[2]).abs(), seen.frame, || format!("layer {index} {state}, {:.1} cycles of {:.3} s", recorded[2], recorded[3]));
                if index == data.layer() {
                    tally.gui_time.take((time - recorded[2]).abs() * recorded[3], seen.frame, || state.clone());
                }
            }
            if !(same && same_next) {
                all = false;
                if tally.wrong.len() < 20 {
                    tally.wrong.push(format!("frame {}: layer {index} ours {state} {time:.4} -> {next:?}, recorded #{} {:.4} -> #{}", seen.frame, recorded[1], recorded[2], recorded[5]));
                }
            }
        }
        tally.layers_same += all as usize;
        tally.gui_same += (layer(&figure, data.layer()).0 == seen.gui) as usize;
        // The weapon's hands.
        if let (Some(recorded), Some((state, time, next))) = (&seen.weapon, figure.weapon_state()) {
            tally.weapon_frames += 1;
            let own = |name: &str| name.rsplit('.').next().unwrap_or(name).to_owned();
            let same = hash(&own(state)) as f32 == recorded[1] && next.map_or(-1.0, |n| hash(&own(n)) as f32) == recorded[5];
            tally.weapon_same += same as usize;
            if same {
                tally.weapon_time.take((time - recorded[2]).abs(), seen.frame, || own(state));
            } else if tally.wrong.len() < 20 {
                tally.wrong.push(format!("frame {}: the weapon ours {state} {time:.4}, recorded #{} {:.4}", seen.frame, recorded[1], recorded[2]));
            }
        }
        for (name, recorded) in curve_names.iter().zip(&seen.curves) {
            tally.curve.take((figure.animator().float(name) - recorded).abs(), seen.frame, || (*name).to_owned());
        }
        let bones = figure.bones();
        for (key, recorded) in &seen.bones {
            let Some((at, turn)) = ours(&bones, key) else { continue };
            let gap = distance(at, [recorded[0], recorded[1], recorded[2]]);
            tally.bone.take(gap, seen.frame, || key.clone());
            tally.turn.take(apart(turn, [recorded[3], recorded[4], recorded[5], recorded[6]]), seen.frame, || key.clone());
            let worst = tally.by_bone.entry(key.clone()).or_default();
            *worst = worst.max(gap);
        }
        // What reached the poser.
        let recorded: Vec<FigureEvent> = seen.events.iter().map(recorded_event).collect();
        let heard: &[FigureEvent] = if k > 0 { figure.heard() } else { &[] };
        for event in &recorded {
            *tally.events_recorded.entry(said(event)).or_default() += 1;
        }
        for event in heard {
            *tally.events_ours.entry(said(event)).or_default() += 1;
        }
        if recorded.as_slice() == heard {
            tally.events_same += 1;
        } else if tally.wrong.len() < 20 {
            tally.wrong.push(format!("frame {}: events ours {:?}, recorded {:?}", seen.frame, heard.iter().map(said).collect::<Vec<_>>(), recorded.iter().map(said).collect::<Vec<_>>()));
        }
    }
    let ages: Vec<String> = frames.iter().filter(|seen| tally.picks_recorded.iter().any(|(frame, _)| *frame == seen.frame)).map(|seen| format!("{:.4}", seen.age)).collect();
    println!("== {name} ({}, kind {kind}): {} frames compared of {}", made.get("item_class").and_then(Json::as_str).unwrap_or("?"), tally.frames, frames.len());
    println!("  picks recorded (frame, pick): {:?} at {ages:?} s", tally.picks_recorded);
    println!("  our idle events on frames {:?}", tally.picks_ours);
    println!(
        "  body layers: every recorded layer's state and next ours on {} of {}; largest time gap {:.6} cycles (frame {}, {})",
        tally.layers_same, tally.frames, tally.layer_time.value, tally.layer_time.frame, tally.layer_time.what
    );
    println!("  GUI state the game's on {} of {}; largest time gap {:.6} s (frame {}, {})", tally.gui_same, tally.frames, tally.gui_time.value, tally.gui_time.frame, tally.gui_time.what);
    println!("  weapon hands' state the game's on {} of {}; largest time gap {:.6} cycles (frame {}, {})", tally.weapon_same, tally.weapon_frames, tally.weapon_time.value, tally.weapon_time.frame, tally.weapon_time.what);
    println!("  curves: largest gap {:.6} (frame {}, {})", tally.curve.value, tally.curve.frame, tally.curve.what);
    println!("  bones: largest gap {:.3} mm (frame {}, {}); largest turn {:.3} deg (frame {}, {})", tally.bone.value * 1000.0, tally.bone.frame, tally.bone.what, tally.turn.value, tally.turn.frame, tally.turn.what);
    println!("  by bone, mm: {}", tally.by_bone.iter().map(|(k, v)| format!("{k} {:.3}", v * 1000.0)).collect::<Vec<_>>().join(", "));
    println!("  events the game's on {} of {} frames; recorded {:?}; ours {:?}", tally.events_same, tally.frames, tally.events_recorded, tally.events_ours);
    for line in &tally.wrong {
        println!("  {line}");
    }
    Some(tally)
}

/// The three takes of recorder 0.17.0 (`overall2_s252`): the AKM, the
/// Rhino (its two checks of the cylinder among the rest), empty hands, each
/// from its making to its page's closing, frame by frame.
#[test]
fn the_figure_as_the_three_takes_recorded_it() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../local/traces/overall2_s252");
    if !std::path::Path::new(dir).exists() {
        eprintln!("skipped: no recordings of the OVERALL figure");
        return;
    }
    let Some((data, mut assets, db)) = data() else { return };
    for (name, frames) in [("anything-023708", 6979), ("anything-023818", 6662), ("anything-023928", 6737)] {
        let Some(tally) = run_take(&data, &mut assets, db, dir, name) else { continue };
        assert_eq!(tally.frames, frames, "{name}");
        assert_eq!((tally.layers_same, tally.gui_same, tally.weapon_same), (tally.frames, tally.frames, tally.weapon_frames), "{name}: {:?}", tally.wrong.first());
        // Every idle event on the recorded frame, and everything else that
        // reached the poser.
        let recorded: Vec<i64> = tally.picks_recorded.iter().map(|(frame, _)| *frame).collect();
        assert_eq!(tally.picks_ours, recorded, "{name}");
        assert_eq!(tally.events_same, tally.frames, "{name}: {:?}", tally.wrong.iter().find(|line| line.contains("events")));
        // (Stepped by the engine's own frame times, every layer's time is
        // the game's to a few millionths: 0.000002 s at most, the AKM's
        // look out after 58 s.)
        assert!(tally.gui_time.value < 1e-4, "{name}: {}", tally.gui_time.value);
        assert!(tally.layer_time.value < 1e-4 && tally.weapon_time.value < 1e-4, "{name}: {} {}", tally.layer_time.value, tally.weapon_time.value);
        assert!(tally.curve.value < 1e-5, "{name}: {}", tally.curve.value);
        assert!(tally.bone.value < 5e-4 && tally.turn.value < 0.3, "{name}: {} m, {} deg", tally.bone.value, tally.turn.value);
    }
}
