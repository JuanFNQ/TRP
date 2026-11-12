//! Reading the recorded world and the voice trace.

use super::*;

/// A bot as the world file sets it up.
#[derive(Debug, Clone)]
pub struct Setup {
    pub id: i64,
    pub profile: String,
    pub side: String,
    /// Its voice's customization id (`outfit.Voice`).
    pub voice: String,
}

/// A bot on one frame.
#[derive(Debug, Clone, Default)]
pub struct BotFrame {
    pub alive: bool,
    pub root: V3,
    pub head: Option<V3>,
    /// Each part's points now and at most.
    pub parts: Option<[[f32; 2]; 7]>,
}

#[derive(Debug, Clone, Default)]
pub struct Frame {
    pub time: f64,
    pub camera: Option<V3>,
    pub bots: BTreeMap<i64, BotFrame>,
}

/// A hit the recorder wrote (`body_hits`): on whom, where, of what, the
/// damage handed to the health, each part's points before and after, and
/// whether it lived after.
#[derive(Debug, Clone)]
pub struct BodyHit {
    pub profile: String,
    pub part: Part,
    pub kind: DamageKind,
    pub damage: f32,
    pub before: [f32; 7],
    pub after: [f32; 7],
    pub alive: bool,
}

/// A sound of a character's speech the recorder wrote.
#[derive(Debug, Clone)]
pub struct Spoken {
    pub frame: i64,
    pub clip: String,
    pub volume: f32,
    pub base_volume: f32,
    pub occlusion: f32,
    pub pos: V3,
    pub source_volume: f32,
    pub pitch: f32,
    pub max: f32,
    pub group: String,
}

fn v3(v: Option<&Value>) -> Option<V3> {
    let a = v?.as_array()?;
    Some([a.first()?.as_f64()? as f32, a.get(1)?.as_f64()? as f32, a.get(2)?.as_f64()? as f32])
}

fn seven(v: Option<&Value>) -> Option<[f32; 7]> {
    let a = v?.as_array()?;
    let mut out = [0.0; 7];
    for (k, slot) in out.iter_mut().enumerate() {
        *slot = a.get(k)?.as_f64()? as f32;
    }
    Some(out)
}

pub(super) fn gap(a: V3, b: V3) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

/// The world file: each bot's set-up, and every frame.
pub fn read_world(path: &str) -> Result<(Vec<Setup>, BTreeMap<i64, Frame>)> {
    let file = std::fs::File::open(path).with_context(|| path.to_owned())?;
    let mut setups = Vec::new();
    let mut heads: HashMap<i64, usize> = HashMap::new();
    let mut frames = BTreeMap::new();
    for line in std::io::BufReader::new(file).lines() {
        let line = line?;
        let Ok(v) = serde_json::from_str::<Value>(&line) else { continue };
        let Some(number) = v.get("frame").and_then(Value::as_i64) else { continue };
        let mut frame = Frame { time: v.get("time").and_then(Value::as_f64).unwrap_or(0.0), camera: v.get("camera").and_then(Value::as_array).and_then(|c| v3(c.first())), ..Frame::default() };
        for bot in v.get("bots").and_then(Value::as_array).into_iter().flatten() {
            let Some(id) = bot.get("id").and_then(Value::as_i64) else { continue };
            if let Some(setup) = bot.get("setup") {
                let text = |name: &str| setup.get(name).and_then(Value::as_str).unwrap_or("").to_owned();
                setups.push(Setup { id, profile: text("profile"), side: text("side"), voice: setup.pointer("/outfit/Voice").and_then(Value::as_str).unwrap_or("").to_owned() });
                if let Some(at) = setup.get("bone_names").and_then(Value::as_array).and_then(|names| names.iter().position(|n| n.as_str() == Some("Base HumanHead"))) {
                    heads.insert(id, at);
                }
            }
            let head = heads.get(&id).and_then(|&at| bot.get("bones")?.as_array()?.get(at)?.as_array()?.first().and_then(|p| v3(Some(p))));
            let parts = bot.get("parts").and_then(Value::as_array).and_then(|parts| {
                let mut out = [[0.0; 2]; 7];
                for (k, slot) in out.iter_mut().enumerate() {
                    let p = parts.get(k)?.as_array()?;
                    *slot = [p.first()?.as_f64()? as f32, p.get(1)?.as_f64()? as f32];
                }
                Some(out)
            });
            frame.bots.insert(id, BotFrame { alive: bot.get("alive").and_then(Value::as_bool).unwrap_or(true), root: v3(bot.get("pos")).unwrap_or_default(), head, parts });
        }
        frames.insert(number, frame);
    }
    Ok((setups, frames))
}

/// The trace's bodies' hits by frame and its sounds of speech.
pub fn read_trace(path: &str) -> Result<(BTreeMap<i64, Vec<BodyHit>>, Vec<Spoken>)> {
    let file = std::fs::File::open(path).with_context(|| path.to_owned())?;
    let mut hits: BTreeMap<i64, Vec<BodyHit>> = BTreeMap::new();
    let mut spoken = Vec::new();
    for line in std::io::BufReader::new(file).lines() {
        let line = line?;
        if !line.contains("\"body_hits\"") && !line.contains("Speech\"") && !line.contains("SimpleOccluded\"") {
            continue;
        }
        let Ok(v) = serde_json::from_str::<Value>(&line) else { continue };
        let Some(frame) = v.get("frame").and_then(Value::as_i64) else { continue };
        for hit in v.get("body_hits").and_then(Value::as_array).into_iter().flatten() {
            let text = |name: &str| hit.get(name).and_then(Value::as_str).unwrap_or("");
            let Some(part) = Part::of_name(text("part")) else { continue };
            let kind = KINDS.iter().copied().find(|k| k.name() == text("type")).unwrap_or(DamageKind::Undefined);
            let (Some(before), Some(after)) = (seven(hit.get("health_before")), seven(hit.get("health_after"))) else { continue };
            let damage = hit.get("damage_out").and_then(Value::as_f64).unwrap_or(0.0) as f32;
            hits.entry(frame).or_default().push(BodyHit { profile: text("profile").to_owned(), part, kind, damage, before, after, alive: hit.get("alive").and_then(Value::as_bool).unwrap_or(true) });
        }
        for sound in v.get("sounds").and_then(Value::as_array).into_iter().flatten() {
            let group = sound.pointer("/a/group").and_then(Value::as_str).unwrap_or("");
            let clip = sound.get("clip").and_then(Value::as_str).unwrap_or("");
            if !matches!(group, "ObservedPlayerSpeech" | "SimpleOccluded") || clip == "prewarm_sound" {
                continue;
            }
            let num = |v: Option<&Value>| v.and_then(Value::as_f64).unwrap_or(f64::NAN) as f32;
            spoken.push(Spoken {
                frame,
                clip: clip.to_owned(),
                volume: num(sound.get("volume")),
                base_volume: num(sound.get("base_volume")),
                occlusion: num(sound.get("occlusion")),
                pos: v3(sound.get("pos")).unwrap_or_default(),
                source_volume: num(sound.pointer("/a/volume")),
                pitch: num(sound.pointer("/a/pitch")),
                max: num(sound.pointer("/a/max")),
                group: group.to_owned(),
            });
        }
    }
    Ok((hits, spoken))
}
