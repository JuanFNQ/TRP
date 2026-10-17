//! A recording's health set beside the simulation's (`docs/spec/health.md`,
//! "Compared with what"), for `bin/health_replay` and the test that keeps
//! it: the character as the recording's first frame has it, the step's own
//! hurt done as the recorder's step did it (`hosts/spt-oracle/Courses.cs`,
//! recorder 0.8.0's course 1), the recorded character's skills, an item put
//! to use where the recording's comes in hand (food and drink by the share
//! the game used), jumps and sprints as the recording's movement made them,
//! and the health stepped by the player's whole-millisecond clock, as the
//! game's is. With the install's assets, the item's own hands too
//! (`MedsHands` on the item's rig), their animator's states set beside the
//! recording's frame by frame.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader};
use std::sync::Arc;

use anyhow::{bail, Context, Result};
use serde_json::Value;
use trp_data::unity::anim::path_hash;
use trp_data::unity::Assets;
use trp_data::Database;

use crate::health::{Chance, DamageKind, Health, HealthSettings, HealthSkills, Kind, Medicine, Part, Stage};
use crate::meds_hands::{MedsEvent, MedsHands};
use crate::weapon_animator::{WeaponAnimator, WeaponAnimatorData};

/// One frame of a recording, as far as health goes.
pub struct Frame {
    pub t: f32,
    /// The frame's time by the player's own clock, in whole milliseconds
    /// (`clock_dt`; the engine's `dt` where a recording has none).
    pub dt: f32,
    /// Each part's current and maximum, and whether it is destroyed.
    pub parts: [[f32; 2]; 7],
    pub destroyed: [bool; 7],
    pub energy: [f32; 2],
    pub hydration: [f32; 2],
    /// Each (effect, part) at work and its state, the effect's name without
    /// the leading `I` of the game's interface.
    pub effects: BTreeMap<(String, String), String>,
    pub hands: String,
    pub in_hands: String,
    pub resource: Option<f32>,
    /// The movement's state (`Jump`, `JumpLanding`, `Sprint`, …).
    pub state: String,
    /// The engine's own frame time, which animators run by.
    pub engine_dt: f32,
    /// The view's pitch, degrees.
    pub pitch: f32,
    /// The hands' animator: its state (the short name's hash, 24 bits), how
    /// far it has played, and its `Active`.
    pub anim: u32,
    pub anim_time: f32,
    pub active: bool,
    /// The feet's height, and whether the movement found the ground under
    /// them on the frame (`pos`, `grounded`): the game's falls are worked
    /// out from them.
    pub height: f32,
    pub grounded: bool,
    /// The character's overweight, 0 to 1, as the recording's header has
    /// it (the same on every frame).
    pub overweight: f32,
}

/// The state's short name's hash as a recording keeps it.
pub fn state_hash(name: &str) -> u32 {
    path_hash(name.rsplit('.').next().unwrap_or(name)) & 0x00ff_ffff
}

/// One use of an item, its hands beside the recording's.
#[derive(Default)]
pub struct HandsUse {
    pub template: String,
    /// The variants' count's start, as found from the recording's first draw.
    pub first: i32,
    /// Each side's states as they came, with their times (the game's named
    /// by ours of the same hash), and `Active` likewise.
    pub game: Vec<(String, f32)>,
    pub ours: Vec<(String, f32)>,
    pub game_active: Vec<(bool, f32)>,
    pub ours_active: Vec<(bool, f32)>,
    /// When the item left the hands: the game's controller changed, ours
    /// were put away.
    pub gone: [Option<f32>; 2],
    /// The largest gap in how far a state has played, on frames both are
    /// in the same one (but each state's first in the recording), and when.
    pub worst: (f32, f32),
    /// How many frames that was.
    pub compared: usize,
}

/// An (effect, part)'s states with their times.
pub type Log = BTreeMap<(String, String), Vec<(String, f32)>>;

/// Reads a recording: its step's name and its frames.
pub fn read(path: &str) -> Result<(String, Vec<Frame>)> {
    let file = std::fs::File::open(path).with_context(|| format!("opening {path}"))?;
    let mut scenario = String::new();
    let mut overweight = 0.0;
    let mut frames = Vec::new();
    for (index, line) in BufReader::new(file).lines().enumerate() {
        let line = line?;
        let v: Value = serde_json::from_str(&line)?;
        if index == 0 {
            scenario = v["scenario"].as_str().unwrap_or("").to_owned();
            overweight = v["overweight"].as_f64().unwrap_or(0.0) as f32;
            continue;
        }
        let Some(parts) = v["parts"].as_array() else { continue };
        let mut p = [[0.0; 2]; 7];
        let mut destroyed = [false; 7];
        for (i, part) in parts.iter().enumerate().take(7) {
            p[i] = [part[0].as_f64().unwrap_or(0.0) as f32, part[1].as_f64().unwrap_or(0.0) as f32];
            destroyed[i] = part[2].as_f64().is_some_and(|d| d != 0.0) || part[2].as_bool().unwrap_or(false);
        }
        let pair = |key: &str| -> [f32; 2] { [v[key][0].as_f64().unwrap_or(0.0) as f32, v[key][1].as_f64().unwrap_or(0.0) as f32] };
        let effects = v["effects"]
            .as_array()
            .map(|list| {
                list.iter()
                    .map(|e| {
                        let kind = e[0].as_str().unwrap_or("?").trim_start_matches('I').to_owned();
                        ((kind, e[1].as_str().unwrap_or("?").to_owned()), e[2].as_str().unwrap_or("?").to_owned())
                    })
                    .collect()
            })
            .unwrap_or_default();
        frames.push(Frame {
            t: v["t"].as_f64().unwrap_or(0.0) as f32,
            dt: v["clock_dt"].as_f64().or_else(|| v["dt"].as_f64()).unwrap_or(0.0) as f32,
            parts: p,
            destroyed,
            energy: pair("energy_now"),
            hydration: pair("hydration_now"),
            effects,
            hands: v["hands_controller"].as_str().unwrap_or("").to_owned(),
            in_hands: v["in_hands_tpl"].as_str().unwrap_or("").to_owned(),
            resource: v["in_hands_resource"].as_f64().map(|r| r as f32),
            state: v["state"].as_str().unwrap_or("").to_owned(),
            engine_dt: v["dt"].as_f64().unwrap_or(0.0) as f32,
            pitch: v["look"][1].as_f64().unwrap_or(0.0) as f32,
            anim: v["hands_anim"].as_f64().unwrap_or(0.0) as u32,
            anim_time: v["hands_anim_time"].as_f64().unwrap_or(0.0) as f32,
            active: v["hands_active"].as_bool().unwrap_or(false),
            height: v["pos"][1].as_f64().unwrap_or(0.0) as f32,
            grounded: v["grounded"].as_bool().unwrap_or(true),
            overweight,
        });
    }
    Ok((scenario, frames))
}

/// The simulation's effects as a recording names them.
fn effects_of(health: &Health) -> BTreeMap<(String, String), String> {
    health
        .effects
        .iter()
        .filter(|e| matches!(e.stage, Stage::Added | Stage::Started | Stage::Residual))
        .map(|e| {
            let stage = match e.stage {
                Stage::Added => "Added",
                Stage::Started => "Started",
                _ => "Residued",
            };
            ((e.kind.name().to_owned(), e.part.name().to_owned()), stage.to_owned())
        })
        .collect()
}

/// What the recorder's step did to the character before its first frame
/// (recorder 0.8.0's course 1; its second takes carry the time in their
/// names).
fn prepare(health: &mut Health, scenario: &str) -> Result<()> {
    match scenario {
        "med-bleed-medkit" => {
            health.apply_damage(Part::LeftLeg, 30.0, DamageKind::Undefined);
            health.do_bleed(false, Part::LeftLeg);
        }
        "med-fracture" => health.do_fracture(Part::LeftLeg),
        "med-heavy-bleed" => health.do_bleed(true, Part::RightLeg),
        "med-surgery" => health.destroy(Part::LeftArm, DamageKind::Undefined),
        "eat-drink" => {
            health.change_energy(-30.0);
            health.change_hydration(-30.0);
        }
        "med-on-health-tab" => {
            health.apply_damage(Part::RightArm, 25.0, DamageKind::Undefined);
        }
        // Course 2 (falls and barbed wire) does nothing to the character
        // first: the falls and the wire are the step.
        "fall-walk-off-ledge" | "50-fall-from-ledge" | "fall-high" | "fall-low" | "barbed-wire" => {}
        other => bail!("no preparation known for the step {other:?}"),
    }
    Ok(())
}

/// A replay's outcome.
#[derive(Default)]
pub struct Replay {
    pub scenario: String,
    /// What was done, line by line (items used, jumps, effects taken up).
    pub notes: Vec<String>,
    /// The recording's and the simulation's effects over time.
    pub recorded: Log,
    pub simulated: Log,
    /// Effects at work in the first frame that the step did not make, as
    /// the recording had them: taken up as started, their timers unknown.
    pub taken_up: Vec<(String, String)>,
    /// The largest gap of each part (current, and maximum where `true`
    /// marks a maximum the game changed, a mended part's random share), of
    /// energy and of hydration, with when.
    pub worst: [(f32, f32); 7],
    pub worst_maximum: [(f32, f32, bool); 7],
    pub worst_energy: (f32, f32),
    pub worst_hydration: (f32, f32),
    /// Every so often (when asked), the game's line and ours.
    pub samples: Vec<String>,
    /// The parts at the end: the game's, ours.
    pub end: [[[f32; 2]; 7]; 2],
    /// Each use's hands, where the assets were given.
    pub hands: Vec<HandsUse>,
    /// Barbed wire's cuts as the game made them (the barbed wire's step):
    /// when, on which part, and the tally done (the part's drop over its
    /// `EnvironmentDamageMultiplier`).
    pub cuts: Vec<(f32, Part, f32)>,
}

/// An item's rig's animator (its `UsePrefab`) and its number of variants.
fn rig_of(db: &Database, assets: &mut Assets, template: &str) -> Result<(WeaponAnimatorData, u32)> {
    let item = db.items.get(template).with_context(|| format!("no item {template}"))?;
    let path = item.props["UsePrefab"]["path"].as_str().with_context(|| format!("{template} has no UsePrefab"))?;
    let rig = trp_data::weapon::assemble_prefab(assets, path)?;
    let data = WeaponAnimatorData::load(assets, &rig)?;
    let variants = item.props["AnimationVariantsNumber"].as_u64().unwrap_or(0) as u32;
    Ok((data, variants))
}

/// The variants' count's start that gives the recording's first draw: each
/// start tried on an animator alone, made and told as the hands' is, run
/// over the recording's frames until its state leaves the first.
fn first_variant(data: &WeaponAnimatorData, variants: u32, frames: &[Frame]) -> i32 {
    if variants < 2 {
        return 0;
    }
    let score = |first: i32| {
        let mut animator = WeaponAnimator::new(data);
        animator.update(0.0001);
        animator.set("AnimationVariant", ((first + 1).rem_euclid(variants as i32)) as f32);
        animator.set_flag("Active", true);
        frames
            .iter()
            .take(240)
            .take_while(|frame| frame.hands == "MedsController")
            .filter(|frame| {
                animator.update(frame.engine_dt);
                state_hash(animator.state().0) == frame.anim
            })
            .count()
    };
    (0..variants as i32).max_by_key(|first| (score(*first), -first)).unwrap_or(0)
}

/// Replays a recording's health. `every`: a sample line so often;
/// `assets`: the install's, for the items' hands.
pub fn run(db: &Database, mut assets: Option<&mut Assets>, skills: HealthSkills, scenario: &str, frames: &[Frame], seed: u64, every: Option<f32>) -> Result<Replay> {
    let settings = Arc::new(HealthSettings::new(&db.config)?);
    let mut health = Health::new(settings, seed);
    health.skills = skills;
    let first = frames.first().context("no frames")?;
    let mut out = Replay { scenario: scenario.to_owned(), ..Default::default() };
    // The recorded character's own tops (a profile's).
    health.energy[2] = first.energy[1];
    health.hydration[2] = first.hydration[1];
    // The step's own doing first, for the effects it makes and their
    // timers; then the character as the first frame has it (the steps were
    // recorded one after another in a raid, each taking up what the one
    // before left): its parts, energy and hydration, the effects at work
    // the step did not make taken up as started, and what the step's damage
    // drew by chance that the game's did not, gone.
    let name = scenario.trim_end_matches(|c: char| c.is_ascii_digit()).trim_end_matches('-');
    prepare(&mut health, name)?;
    for (i, part) in first.parts.iter().enumerate() {
        health.parts[i].current = part[0];
        health.parts[i].maximum = part[1];
        health.parts[i].destroyed = first.destroyed[i];
    }
    health.energy[0] = first.energy[0];
    health.hydration[0] = first.hydration[0];
    let ours = effects_of(&health);
    for (key, stage) in &first.effects {
        let differs = ours.get(key).is_some_and(|s| s != stage);
        if !ours.contains_key(key) {
            if let (Some(kind), Some(part)) = (Kind::of_name(&key.0), Part::of_name(&key.1)) {
                health.take_up(kind, part);
                out.notes.push(format!("taken up from the first frame: {} on {} ({stage})", key.0, key.1));
            }
        }
        if !ours.contains_key(key) || differs {
            out.taken_up.push(key.clone());
        }
    }
    let unwanted: Vec<u32> = health.effects.iter().filter(|e| e.existing() && !first.effects.contains_key(&(e.kind.name().to_owned(), e.part.name().to_owned()))).map(|e| e.id).collect();
    for id in unwanted {
        if let Some(e) = health.effects.iter().find(|e| e.id == id) {
            out.notes.push(format!("not in the first frame, taken off: {} on {}", e.kind.name(), e.part.name()));
        }
        health.force_remove(id);
    }
    // Existence's loops began with the raid, before the recording: set so
    // that ours ticks where the game's first does (energy and hydration
    // down together by the loop's figures, the skills' cut taken).
    let energy_tick = health.settings.existence[0] * (1.0 - health.skills.health_energy);
    let hydration_tick = health.settings.existence[2] * (1.0 - health.skills.health_hydration);
    let mut elapsed = 0.0f32;
    for pair in frames.windows(2) {
        elapsed += pair[0].dt;
        let (energy_drop, hydration_drop) = (pair[0].energy[0] - pair[1].energy[0], pair[0].hydration[0] - pair[1].hydration[0]);
        if (hydration_drop - hydration_tick).abs() < 0.01 && energy_drop >= energy_tick - 0.01 {
            let at = elapsed + pair[1].dt;
            health.set_existence_clock(health.settings.existence[1] - at + pair[1].dt * 0.5);
            out.notes.push(format!("existence's first tick in the recording at {:.3} s: ours set to tick there", pair[1].t));
            break;
        }
    }

    let note = |log: &mut Log, now: &BTreeMap<(String, String), String>, before: &BTreeMap<(String, String), String>, t: f32| {
        for (key, stage) in now {
            if before.get(key) != Some(stage) {
                log.entry(key.clone()).or_default().push((stage.clone(), t));
            }
        }
        for key in before.keys() {
            if !now.contains_key(key) {
                log.entry(key.clone()).or_default().push(("gone".into(), t));
            }
        }
    };
    let (mut recorded_before, mut simulated_before) = (BTreeMap::new(), BTreeMap::new());
    let mut item = 0u64;
    let mut next_sample = 0.0;
    let mut hands: Option<MedsHands> = None;
    let mut use_now: Option<(HandsUse, BTreeMap<u32, String>)> = None;
    // The highest the feet have been since they were last on the ground
    // (`MovementContext._startFallingHeight`), and the weight's multiplier
    // of a fall's damage (the recording's own overweight, its header's).
    let mut fall_from = first.height;
    let fall_weight = 1.0 + (health.settings.fall_overweight - 1.0) * first.overweight.clamp(0.0, 1.0);
    for (index, frame) in frames.iter().enumerate() {
        // The ground check of the frame (`CheckFlying`), at the feet's
        // height before the frame's move: the recording's is after it, so
        // the frame before's. On the ground the health is handed the height
        // fallen since last on it (0 but on a landing's frame), with the
        // chances' outcomes the game drew, as the frame's effects show them
        // (a leg's bleeding or fracture come on the landing's frame); off
        // it, the highest point is kept.
        let checked = if index == 0 { frame.height } else { frames[index - 1].height };
        if frame.grounded {
            let fallen = fall_from - checked;
            fall_from = checked;
            if fallen > health.settings.fall_safe_height {
                let came = |kind: &str, part: Part| {
                    let key = (kind.to_owned(), part.name().to_owned());
                    frame.effects.contains_key(&key) && !recorded_before.contains_key(&key)
                };
                for leg in [Part::LeftLeg, Part::RightLeg] {
                    health.outcomes.push((Chance::LightBleeding, leg, came("LightBleeding", leg)));
                    health.outcomes.push((Chance::Fracture, leg, came("Fracture", leg)));
                }
                let damage = health.handle_fall(fallen, fall_weight);
                health.outcomes.clear();
                out.notes.push(format!("{:.3} s: a fall of {fallen:.4} m, {damage:.4} to each leg; the chances as the game's", frame.t));
            }
        } else if checked > fall_from {
            fall_from = checked;
        }
        // What the frame did comes before the health's update of it, as in
        // the game (an effect added in a frame is updated with that frame's
        // time): an item taken in hand where the recording's is, and put
        // to work by its hands as they spawn (the recording's medicine
        // effect comes on that frame every time).
        let med_now: Vec<&(String, String)> = frame.effects.keys().filter(|(kind, _)| kind == "MedEffect").collect();
        let med_before: Vec<&(String, String)> = recorded_before.keys().filter(|(kind, _): &&(String, String)| kind == "MedEffect").collect();
        let taken = frame.hands == "MedsController" && (index == 0 || frames[index - 1].hands != "MedsController");
        if taken {
            let template = if frame.in_hands.is_empty() { frames[index..].iter().find(|f| !f.in_hands.is_empty()).map(|f| f.in_hands.clone()).unwrap_or_default() } else { frame.in_hands.clone() };
            let medicine = Arc::new(Medicine::of(db, &template).with_context(|| format!("no medicine {template}"))?);
            let resource = frame.resource.filter(|r| *r > 0.0).unwrap_or_else(|| medicine.full());
            item += 1;
            // The part: the one the recording's effect came on (the
            // HEALTH tab's or the inventory's "Use" name one; the recorded
            // use speed of the surgical kit says a part was named).
            let key = med_now.iter().find(|key| !med_before.contains(key)).copied();
            // Food and drink: the share used, as the game's split dialog let
            // it be chosen (`ItemUiContext.ShowSplitMenuDialog`): what the
            // item held when its effect ended, taken from what it held as
            // it began, over its whole.
            let amount = match key.filter(|_| medicine.food() && medicine.max_resource > 0.0) {
                Some(key) => {
                    let end = frames[index..].iter().position(|f| !f.effects.contains_key(key)).map(|i| index + i);
                    let left = end.and_then(|e| frames[e].resource).unwrap_or(0.0);
                    Some(((resource - left) / medicine.max_resource).clamp(0.0, 1.0))
                }
                None => None,
            };
            let rig = match assets.as_deref_mut() {
                Some(assets) => Some(rig_of(db, assets, &template)?),
                None => None,
            };
            let first = rig.as_ref().map_or(0, |(data, variants)| first_variant(data, *variants, &frames[index..]));
            // The parts' list as the way of use makes it: a part dropped on
            // in the HEALTH tab's course, the part an item with a priority
            // of its own goes to by the inventory's "Use"
            // (`BodyPartsPriority`), else the common body, as a quick-use
            // key and "Use" name it (`TargetBodyParts`, the game's
            // continuous healing off as the user's is).
            let named = name.starts_with("med-on-health-tab") || !medicine.part_priority.is_empty();
            let part = key.filter(|_| named).and_then(|key| Part::of_name(&key.1));
            let mut made = MedsHands::new(rig.as_ref().map(|r| &r.0), medicine, rig.as_ref().map_or(0, |r| r.1));
            let started = made.take(&mut health, item, resource, &[part], amount, Some(first));
            out.notes.push(format!(
                "{:.3} s: {} ({template}) on {}, resource {resource}{}: {}",
                frame.t,
                db.items.get(&template).map_or("?", |i| i.name.as_str()),
                part.map_or("the common body", |part| part.name()),
                amount.map_or(String::new(), |a| format!(", a share of {a:.3}")),
                if started.is_some() { "started" } else { "REFUSED" }
            ));
            if let Some((data, _)) = &rig {
                let names = data.state_names().into_iter().map(|name| (state_hash(&name), name)).collect();
                use_now = Some((HandsUse { template: template.clone(), first, ..Default::default() }, names));
            }
            hands = Some(made);
        }
        // A use cut short: where ours has time left when the game's has
        // ended (no recording has one so far: the medicine course's "break
        // it off by sprinting" could not, sprinting being barred while an
        // item is at work).
        for key in &med_before {
            let left = health.existing(Kind::MedEffect, Part::of_name(&key.1)).map(|e| e.time_left());
            if !med_now.contains(key) && left.is_some_and(|left| left > 0.1) {
                health.cancel_items();
                out.notes.push(format!("{:.3} s: the use on {} cut short ({:.3} s of ours left)", frame.t, key.1, left.unwrap_or(0.0)));
            }
        }
        // What the movement did, as the game's movement tells its health: a
        // jump landed, a sprint on a damaged leg.
        if frame.state == "JumpLanding" && index > 0 && frames[index - 1].state != "JumpLanding" {
            let conditions = health.conditions();
            health.jump_landed(conditions);
            out.notes.push(format!("{:.3} s: a jump landed", frame.t));
        }
        // Barbed wire's cuts (the wire's step, where nothing else does
        // damage): each part's drop in the frame is a cut of the game's, its
        // tally the drop over the part's multiplier; done to ours as the
        // game's `ProceedBarb` does it, as barbed-wire damage of that tally.
        if name == "barbed-wire" && index > 0 {
            for part in Part::ALL {
                let drop = frames[index - 1].parts[part.index()][0] - frame.parts[part.index()][0];
                if drop > 1e-4 {
                    let tally = drop / health.settings.environment[part.index()];
                    out.cuts.push((frame.t, part, tally));
                    health.apply_damage(part, tally, DamageKind::Barbed);
                }
            }
        }
        let conditions = health.conditions();
        health.sprint_frame(frame.dt, frame.state == "Sprint", conditions);
        health.step(frame.dt);
        // What health did goes to the item's hands, and they move on by the
        // engine's frame.
        for event in health.take_events() {
            if let Some(hands) = hands.as_mut() {
                hands.on_health(&mut health, &event);
            }
        }
        let mut put_away = false;
        if let Some(made) = hands.as_mut() {
            put_away = made.step(&mut health, frame.engine_dt, frame.pitch).contains(&MedsEvent::Hidden);
        }
        if let Some((use_of, names)) = use_now.as_mut() {
            let in_game = frame.hands == "MedsController";
            if in_game {
                let name = names.get(&frame.anim).cloned().unwrap_or_else(|| format!("#{}", frame.anim));
                if use_of.game.last().is_none_or(|(last, _)| *last != name) {
                    use_of.game.push((name, frame.t));
                }
                if use_of.game_active.last().is_none_or(|(last, _)| *last != frame.active) {
                    use_of.game_active.push((frame.active, frame.t));
                }
            } else if use_of.gone[0].is_none() {
                use_of.gone[0] = Some(frame.t);
            }
            if put_away {
                use_of.gone[1] = Some(frame.t);
            }
            let ours_in = hands.as_ref().is_some_and(|h| h.in_hand());
            if let Some(animator) = hands.as_ref().filter(|h| h.in_hand()).and_then(|h| h.animator()) {
                let (name, time, _) = animator.state();
                if use_of.ours.last().is_none_or(|(last, _)| last != name) {
                    use_of.ours.push((name.to_owned(), frame.t));
                }
                let active = animator.flag("Active");
                if use_of.ours_active.last().is_none_or(|(last, _)| *last != active) {
                    use_of.ours_active.push((active, frame.t));
                }
                let first_of_state = index > 0 && frames[index - 1].anim != frame.anim;
                if in_game && state_hash(name) == frame.anim && !first_of_state {
                    use_of.compared += 1;
                    if (time - frame.anim_time).abs() > use_of.worst.0 {
                        use_of.worst = ((time - frame.anim_time).abs(), frame.t);
                    }
                }
            }
            if !in_game && !ours_in {
                out.hands.push(use_now.take().map(|(use_of, _)| use_of).unwrap_or_default());
                hands = None;
            }
        }
        if hands.as_ref().is_some_and(|h| !h.in_hand()) && use_now.is_none() {
            hands = None;
        }

        let simulated_now = effects_of(&health);
        note(&mut out.recorded, &frame.effects, &recorded_before, frame.t);
        note(&mut out.simulated, &simulated_now, &simulated_before, frame.t);
        recorded_before = frame.effects.clone();
        simulated_before = simulated_now;
        for i in 0..7 {
            let gap = (health.parts[i].current - frame.parts[i][0]).abs();
            if gap > out.worst[i].0 {
                out.worst[i] = (gap, frame.t);
            }
            let gap = (health.parts[i].maximum - frame.parts[i][1]).abs();
            let changed = (frame.parts[i][1] - first.parts[i][1]).abs() > 0.0;
            if gap > out.worst_maximum[i].0 {
                out.worst_maximum[i] = (gap, frame.t, changed);
            }
        }
        let gap = (health.energy[0] - frame.energy[0]).abs();
        if gap > out.worst_energy.0 {
            out.worst_energy = (gap, frame.t);
        }
        let gap = (health.hydration[0] - frame.hydration[0]).abs();
        if gap > out.worst_hydration.0 {
            out.worst_hydration = (gap, frame.t);
        }
        if let Some(every) = every.filter(|_| frame.t >= next_sample) {
            next_sample += every;
            let line = |parts: Vec<f32>| parts.iter().map(|p| format!("{p:.2}")).collect::<Vec<_>>().join(" ");
            out.samples.push(format!(
                "{:7.3} game  {} | energy {:.2} hydration {:.2} | {}\n        ours  {} | energy {:.2} hydration {:.2}",
                frame.t,
                line(frame.parts.iter().map(|p| p[0]).collect()),
                frame.energy[0],
                frame.hydration[0],
                frame.hands,
                line(health.parts.iter().map(|p| p.current).collect()),
                health.energy[0],
                health.hydration[0]
            ));
        }
    }
    if let Some((use_of, _)) = use_now.take() {
        out.hands.push(use_of);
    }
    if let Some(last) = frames.last() {
        out.end = [last.parts, std::array::from_fn(|i| [health.parts[i].current, health.parts[i].maximum])];
    }
    Ok(out)
}
