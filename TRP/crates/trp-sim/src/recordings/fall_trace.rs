//! A recorded corpse's fall set beside the rule's, for the tools that compare it.
//! After `landing`'s "A corpse's fall" and the sound spec, "A body's fall". The
//! tools are `bin/corpse --fall` of `trp-physics` (the player's deaths) and
//! `bin/botsounds --falls` here (the bots'). A trace's `sounds` whose clip
//! is in some surface's prone drop are the falls recorded; a death (its
//! frame, the root, the surface under it as the recording tells it) gives
//! what the rule plays; and the two are set side by side: the bank, the
//! frame, the volume handed to the source (the asked times the bank's
//! draw), the pitch, the reach, the mixer's group and the place.

use std::collections::HashMap;

use serde_json::Value;
use trp_data::audio::sound::{MovementSounds, SURFACES};

use crate::sound::landing::{body_fall_bank, drawn_ranges};

/// The mixer's group a fall plays in: its banks' source kind's
/// (`Character`, 3), whose sources go to `CommonSounds`.
pub const FALL_GROUP: &str = "CommonSounds";

/// A fall the recorder wrote: its frame, its clip, the volume handed to the
/// source and the source as it was then set (pitch, falloff's nearest and
/// farthest, the mixer's group), the occlusion's share, where it was (less
/// the recording's origin) and, from recorder 0.11.0 on, how far from the
/// ear.
#[derive(Debug, Clone)]
pub struct Recorded {
    pub frame: i64,
    pub clip: String,
    pub volume: f32,
    pub pitch: f32,
    pub min: f32,
    pub max: f32,
    pub group: String,
    pub occlusion: f32,
    pub pos: [f32; 3],
    pub distance: Option<f32>,
}

/// What a death is: a name for the report, its frame, the corpse's root,
/// the surfaces the recording says may be under it (the surface the
/// player's own frame found underfoot; the surfaces of a bot's last step),
/// and where that came from; and how far it was from the ear (the camera),
/// where the recording tells it.
#[derive(Debug, Clone)]
pub struct Death {
    pub name: String,
    pub frame: i64,
    pub root: [f32; 3],
    pub surfaces: Vec<u8>,
    pub surface_from: String,
    pub ear: Option<f32>,
}

/// The verdicts of one death: each `None` where it could not be told.
#[derive(Debug, Clone, Default)]
pub struct Outcome {
    pub lines: Vec<String>,
    pub found: bool,
    pub same_bank: Option<bool>,
    pub same_frame: Option<bool>,
    pub volume_within: Option<bool>,
    pub pitch_within: Option<bool>,
    pub same_reach: Option<bool>,
    pub same_group: Option<bool>,
    /// No fall recorded, and the root farther from the ear than the banks
    /// reach: none is played, as the rule has it (`PlayAtPoint`).
    pub out_of_hearing: bool,
}

/// The surfaces' prone drops: each clip's bank, and the volume the rule
/// asks.
pub struct Falls<'a> {
    pub sounds: &'a MovementSounds,
    pub asked: f32,
    by_clip: HashMap<&'a str, usize>,
}

fn v3(v: Option<&Value>) -> Option<[f32; 3]> {
    let a = v?.as_array()?;
    Some([a.first()?.as_f64()? as f32, a.get(1)?.as_f64()? as f32, a.get(2)?.as_f64()? as f32])
}

fn gap(a: [f32; 3], b: [f32; 3]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

/// A surface's name.
pub fn surface_name(surface: u8) -> &'static str {
    SURFACES.get(surface as usize).copied().unwrap_or("?")
}

/// A surface's number by its name.
pub fn surface_of(name: &str) -> Option<u8> {
    SURFACES.iter().position(|s| *s == name).map(|i| i as u8)
}

impl<'a> Falls<'a> {
    pub fn new(sounds: &'a MovementSounds, asked: f32) -> Self {
        let mut by_clip = HashMap::new();
        for set in &sounds.surfaces {
            if let Some(at) = set.banks.get(crate::sound::steps::PRONE_DROP).copied().flatten() {
                for list in &sounds.banks[at].lists {
                    for &c in list {
                        by_clip.entry(sounds.clips[c].0.as_str()).or_insert(at);
                    }
                }
            }
        }
        Self { sounds, asked, by_clip }
    }

    /// The prone drop bank a clip is in.
    pub fn bank_of(&self, clip: &str) -> Option<usize> {
        self.by_clip.get(clip).copied()
    }

    /// The surfaces whose set's prone drop is a bank.
    pub fn surfaces_of(&self, bank: usize) -> Vec<u8> {
        self.sounds.surfaces.iter().filter(|set| set.banks.get(crate::sound::steps::PRONE_DROP).copied().flatten() == Some(bank)).map(|set| set.surface).collect()
    }

    /// A bank's clips (its first list: the falls' banks have no other).
    pub fn clips(&self, bank: usize) -> Vec<&str> {
        self.sounds.banks[bank].lists.iter().flatten().map(|&c| self.sounds.clips[c].0.as_str()).collect()
    }

    /// The falls among a frame's `sounds`.
    pub fn falls_in(&self, frame: i64, line: &Value) -> Vec<Recorded> {
        let mut out = Vec::new();
        for x in line.get("sounds").and_then(Value::as_array).into_iter().flatten() {
            let clip = x.get("clip").and_then(Value::as_str).unwrap_or("");
            if self.bank_of(clip).is_none() {
                continue;
            }
            let f = |path: &str| x.pointer(path).and_then(Value::as_f64).map(|v| v as f32);
            out.push(Recorded {
                frame,
                clip: clip.to_owned(),
                volume: f("/volume").unwrap_or(0.0),
                pitch: f("/a/pitch").unwrap_or(1.0),
                min: f("/a/min").unwrap_or(0.0),
                max: f("/a/max").unwrap_or(0.0),
                group: x.pointer("/a/group").and_then(Value::as_str).unwrap_or("").to_owned(),
                occlusion: f("/occlusion").unwrap_or(1.0),
                pos: v3(x.get("pos")).unwrap_or([0.0; 3]),
                distance: f("/distance"),
            });
        }
        out
    }

    /// One death beside the fall recorded nearest its root, from its frame
    /// to `frames` after, within `reach` m (each recorded fall taken once:
    /// `taken`).
    pub fn compare(&self, death: &Death, recorded: &[Recorded], taken: &mut [bool], frames: i64, reach: f32) -> Outcome {
        let mut out = Outcome::default();
        let surfaces = if death.surfaces.is_empty() { "not recorded".to_owned() } else { death.surfaces.iter().map(|&s| surface_name(s)).collect::<Vec<_>>().join(" or ") };
        out.lines.push(format!(
            "{}: dead on frame {}, the root at [{:.3}, {:.3}, {:.3}]{}, on {surfaces} ({})",
            death.name,
            death.frame,
            death.root[0],
            death.root[1],
            death.root[2],
            death.ear.map_or(String::new(), |ear| format!(", {ear:.2} m from the camera")),
            death.surface_from
        ));
        let nearest = recorded
            .iter()
            .enumerate()
            .filter(|(i, r)| !taken[*i] && r.frame >= death.frame && r.frame <= death.frame + frames && gap(r.pos, death.root) <= reach)
            .min_by(|a, b| (a.1.frame - death.frame, gap(a.1.pos, death.root)).partial_cmp(&(b.1.frame - death.frame, gap(b.1.pos, death.root))).unwrap_or(std::cmp::Ordering::Equal));
        // The rule's bank for each surface the recording allows.
        let ours: Vec<(u8, usize)> = death.surfaces.iter().filter_map(|&s| Some((s, body_fall_bank(self.sounds, Some(s))?))).collect();
        let Some((index, fall)) = nearest else {
            out.lines.push(format!("  recorded: no fall within {frames} frames and {reach} m"));
            // Every prone drop reaches as far (66.5 m in the install).
            let farthest = ours.iter().map(|(_, at)| self.sounds.banks[*at].rolloff).chain(self.by_clip.values().map(|&at| self.sounds.banks[at].rolloff)).fold(0.0f32, f32::max);
            if let Some(ear) = death.ear.filter(|&ear| ear > farthest) {
                out.out_of_hearing = true;
                out.lines.push(format!("  the rule plays none either: {ear:.2} m from the ear, past the banks' {farthest:.1} m (`PlayAtPoint` borrows no source)"));
                return out;
            }
            for (s, at) in &ours {
                out.lines.push(format!("  TRP: {}'s {} on frame {}", surface_name(*s), self.sounds.banks[*at].name, death.frame));
            }
            return out;
        };
        taken[index] = true;
        out.found = true;
        let Some(bank) = self.bank_of(&fall.clip) else { return out };
        let played_on: Vec<&str> = self.surfaces_of(bank).into_iter().map(surface_name).collect();
        out.lines.push(format!(
            "  recorded: {} on frame {} (+{}), volume {:.4}, pitch {:.4}, heard {:.1} to {:.1} m, in {}, occlusion {:.3}, at [{:.3}, {:.3}, {:.3}] ({:.3} m from the root){}; the clip is the prone drop of {}",
            fall.clip,
            fall.frame,
            fall.frame - death.frame,
            fall.volume,
            fall.pitch,
            fall.min,
            fall.max,
            fall.group,
            fall.occlusion,
            fall.pos[0],
            fall.pos[1],
            fall.pos[2],
            gap(fall.pos, death.root),
            fall.distance.map_or(String::new(), |d| format!(", {d:.2} m from the ear")),
            played_on.join(", ")
        ));
        // The bank the rule plays: the recording's surface's where it is
        // told, else the one the clip is in (the rule can then only be told
        // by its draws). The draws are the played bank's: what the rule asks
        // of whichever bank the surface gives.
        let at = match ours.iter().find(|(_, at)| *at == bank) {
            Some(&(_, at)) => {
                out.same_bank = Some(true);
                at
            }
            None if !ours.is_empty() => {
                out.same_bank = Some(false);
                ours[0].1
            }
            None => bank,
        };
        if at != bank {
            out.lines.push(format!("  TRP on the surface recorded: {} ({})", self.sounds.banks[at].name, self.clips(at).join(" ")));
        }
        let b = &self.sounds.banks[bank];
        let (volumes, pitches) = drawn_ranges(b, self.asked);
        out.same_frame = Some(fall.frame == death.frame);
        out.volume_within = Some(fall.volume >= volumes[0] - 1e-4 && fall.volume <= volumes[1] + 1e-4);
        out.pitch_within = Some(fall.pitch >= pitches[0] - 1e-4 && fall.pitch <= pitches[1] + 1e-4);
        // An occluded source's farthest is the occlusion's (not built).
        out.same_reach = (fall.occlusion >= 1.0 - 1e-4).then(|| (fall.max - b.rolloff).abs() < 1e-3);
        out.same_group = Some(fall.group == FALL_GROUP);
        let yes = |v: Option<bool>| match v {
            Some(true) => "same",
            Some(false) => "DIFFERS",
            None => "-",
        };
        out.lines.push(format!(
            "  TRP with the bank played: {} ({} clips: {}) on frame {}, asked {:.2} x {:.2}..{:.2} = {:.4}..{:.4}, pitch {:.2}..{:.2}, heard to {:.1} m, in {FALL_GROUP} (source kind {})",
            b.name,
            self.clips(bank).len(),
            self.clips(bank).join(" "),
            death.frame,
            self.asked,
            volumes[0] / self.asked.max(1e-6),
            volumes[1] / self.asked.max(1e-6),
            volumes[0],
            volumes[1],
            pitches[0],
            pitches[1],
            b.rolloff,
            b.source_type
        ));
        out.lines.push(format!(
            "  bank {}, frame {}, volume {}, pitch {}, reach {}{}, group {}",
            if out.same_bank.is_none() { "- (no surface recorded)" } else { yes(out.same_bank) },
            yes(out.same_frame),
            if out.volume_within == Some(true) { "within" } else { "OUTSIDE" },
            if out.pitch_within == Some(true) { "within" } else { "OUTSIDE" },
            yes(out.same_reach),
            if fall.occlusion < 1.0 - 1e-4 { format!(" (occluded to {:.3}: the game's occlusion, not built, sets its own farthest, {:.1} m)", fall.occlusion, fall.max) } else { String::new() },
            yes(out.same_group)
        ));
        out
    }
}

/// The verdicts summed up: how many deaths had a fall, and of those how
/// many had each figure the same (or within the draw).
pub fn summary(outcomes: &[Outcome]) -> String {
    let count = |f: &dyn Fn(&Outcome) -> Option<bool>| -> (usize, usize) {
        let told: Vec<bool> = outcomes.iter().filter(|o| o.found).filter_map(f).collect();
        (told.iter().filter(|v| **v).count(), told.len())
    };
    let found = outcomes.iter().filter(|o| o.found).count();
    let unheard = outcomes.iter().filter(|o| o.out_of_hearing).count();
    let (bank, banks) = count(&|o| o.same_bank);
    let (frame, frames) = count(&|o| o.same_frame);
    let (volume, volumes) = count(&|o| o.volume_within);
    let (pitch, pitches) = count(&|o| o.pitch_within);
    let (reach, reaches) = count(&|o| o.same_reach);
    let (group, groups) = count(&|o| o.same_group);
    format!(
        "{} deaths, {found} with a fall recorded, {unheard} out of hearing with none (as the rule): the bank {bank} of {banks} (where the surface is recorded), the frame {frame} of {frames}, the volume within the draw {volume} of {volumes}, the pitch within {pitch} of {pitches}, the reach {reach} of {reaches} (unoccluded), the group {group} of {groups}",
        outcomes.len()
    )
}
