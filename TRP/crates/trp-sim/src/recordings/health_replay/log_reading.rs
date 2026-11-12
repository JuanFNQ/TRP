//! Reading a health recording's log of frames and hands in use.

use super::*;

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
    /// Each one's time left in its state (`CurStateTimeLeft`; none where it
    /// lasts until something ends it).
    pub left: BTreeMap<(String, String), Option<f32>>,
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
        let list = v["effects"].as_array().cloned().unwrap_or_default();
        let key_of = |e: &Value| (e[0].as_str().unwrap_or("?").trim_start_matches('I').to_owned(), e[1].as_str().unwrap_or("?").to_owned());
        let effects = list.iter().map(|e| (key_of(e), e[2].as_str().unwrap_or("?").to_owned())).collect();
        let left = list.iter().map(|e| (key_of(e), e[3].as_f64().map(|t| t as f32))).collect();
        frames.push(Frame {
            t: v["t"].as_f64().unwrap_or(0.0) as f32,
            dt: v["clock_dt"].as_f64().or_else(|| v["dt"].as_f64()).unwrap_or(0.0) as f32,
            parts: p,
            destroyed,
            energy: pair("energy_now"),
            hydration: pair("hydration_now"),
            effects,
            left,
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
pub(super) fn effects_of(health: &Health) -> BTreeMap<(String, String), String> {
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
pub(super) fn prepare(health: &mut Health, scenario: &str) -> Result<()> {
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
        // A recording made by hand (recorder 0.14.0's `+` course, `courses8`):
        // nothing done first; the character is as its first frame has it.
        "anything" => {}
        other => bail!("no preparation known for the step {other:?}"),
    }
    Ok(())
}
