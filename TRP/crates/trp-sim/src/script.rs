//! A scripted input sequence: the files the recording plugin plays
//! (`hosts/spt-oracle/scenarios`), read here so that the replay and a host
//! can give the simulation the same input the game was given.

use std::path::Path;

use anyhow::{Context, Result};
use serde_json::Value as Json;

pub struct Scenario {
    pub name: String,
    pub duration: f32,
    /// Key, from and to in seconds: w a s d shift q e aim fire step_left
    /// step_right.
    pub holds: Vec<(String, f32, f32)>,
    /// From, to, yaw rate and pitch rate in degrees per second: yaw to the
    /// right, pitch downward. The game takes these through its mouse
    /// sensitivity, as it takes a mouse.
    pub turns: Vec<(f32, f32, f32, f32)>,
    /// Key and time: jump crouch prone reload wheel_up wheel_down stance_up
    /// stance_down auto single check_magazine check_chamber.
    pub presses: Vec<(String, f32)>,
    /// Where the script waits on the character and not on the time
    /// (recorder 0.4.9 on), in the order of their times.
    pub gates: Vec<Gate>,
}

/// A place in a script where its clock stands still until the character's
/// legs are where the rest of the script needs them. All the other times
/// of a script are its own clock's, so what follows a gate follows it by
/// as long as the script says, however long the gate was shut.
#[derive(Debug, Clone, PartialEq)]
pub struct Gate {
    /// The script's time the clock stands at while the gate is shut.
    pub at: f32,
    /// What opens it.
    pub opens: Opens,
    /// A key pressed again and again while it is shut (as a press of the
    /// script's is), and how many seconds apart.
    pub press: Option<(String, f32)>,
    /// The longest it stays shut, in seconds: it opens then whatever the
    /// legs hold, so that a script cannot hang.
    pub timeout: f32,
}

/// What a gate waits for: the legs' stamina under a figure, or at it or
/// over it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Opens {
    LegsBelow(f32),
    LegsFrom(f32),
}

impl Scenario {
    pub fn load(path: &Path) -> Result<Self> {
        let v: Json = serde_json::from_str(&std::fs::read_to_string(path).with_context(|| path.display().to_string())?)
            .with_context(|| path.display().to_string())?;
        let number = |v: &Json, name: &str| v.get(name).and_then(Json::as_f64).unwrap_or(0.0) as f32;
        let text = |v: &Json, name: &str| v.get(name).and_then(Json::as_str).unwrap_or("").to_owned();
        let list = |name: &str| v.get(name).and_then(Json::as_array).cloned().unwrap_or_default();
        let gate = |g: &Json| Gate {
            at: number(g, "at"),
            opens: match g.get("legs_below").and_then(Json::as_f64) {
                Some(below) => Opens::LegsBelow(below as f32),
                None => Opens::LegsFrom(number(g, "legs_from")),
            },
            press: g.get("press").and_then(Json::as_str).map(|key| (key.to_owned(), number(g, "every"))),
            timeout: number(g, "timeout"),
        };
        let mut gates: Vec<Gate> = list("gate").iter().map(gate).collect();
        gates.sort_by(|a, b| a.at.total_cmp(&b.at));
        Ok(Self {
            name: v
                .get("name")
                .and_then(Json::as_str)
                .map(str::to_owned)
                .unwrap_or_else(|| path.file_stem().unwrap_or_default().to_string_lossy().into_owned()),
            duration: number(&v, "duration"),
            holds: list("hold").iter().map(|h| (text(h, "key"), number(h, "from"), number(h, "to"))).collect(),
            turns: list("turn")
                .iter()
                .map(|t| (number(t, "from"), number(t, "to"), number(t, "yaw"), number(t, "pitch")))
                .collect(),
            presses: list("press").iter().map(|p| (text(p, "key"), number(p, "at"))).collect(),
            gates,
        })
    }

    pub fn held(&self, key: &str, time: f32) -> bool {
        self.holds.iter().any(|(k, from, to)| k == key && time >= *from && time < *to)
    }

    /// The yaw and pitch rates asked at a time, in degrees per second.
    pub fn turn_rate(&self, time: f32) -> [f32; 2] {
        let mut rate = [0.0; 2];
        for (_, _, yaw, pitch) in self.turns.iter().filter(|t| time >= t.0 && time < t.1) {
            rate[0] += yaw;
            rate[1] += pitch;
        }
        rate
    }

    /// How far the script asks the view to turn between two times, yaw and
    /// pitch in degrees: each turn's rate for as long as it runs between them.
    pub fn turned(&self, before: f32, now: f32) -> [f32; 2] {
        let mut turned = [0.0; 2];
        for (from, to, yaw, pitch) in &self.turns {
            let span = (now.min(*to) - before.max(*from)).max(0.0);
            turned[0] += yaw * span;
            turned[1] += pitch * span;
        }
        turned
    }

    /// The presses made in a frame that takes the time from `before` to
    /// `now`.
    pub fn pressed(&self, before: f32, now: f32) -> impl Iterator<Item = &str> {
        self.presses.iter().filter(move |(_, at)| before < *at && now >= *at).map(|(key, _)| key.as_str())
    }

    /// How long the scenario runs before its script does anything.
    pub fn quiet(&self) -> f32 {
        let holds = self.holds.iter().map(|(_, from, _)| *from);
        let turns = self.turns.iter().map(|(from, ..)| *from);
        let presses = self.presses.iter().map(|(_, at)| *at);
        let gates = self.gates.iter().map(|gate| gate.at);
        holds.chain(turns).chain(presses).chain(gates).fold(self.duration, f32::min)
    }
}

/// A script's own clock: it runs with the frames, and stands still at a
/// gate for as long as the gate is shut. The recorder keeps the same clock
/// by the same steps (`hosts/spt-oracle/Plugin.cs`, `Tick`) and writes
/// what it shows on every frame, so a replay follows the recording's and
/// needs none of its own.
#[derive(Debug, Clone, Default)]
pub struct Clock {
    time: f32,
    /// The gates gone through.
    passed: usize,
    /// At a shut gate: for how long, and how long until its key is next
    /// pressed.
    shut: Option<(f32, f32)>,
}

impl Clock {
    /// The script's time now.
    pub fn time(&self) -> f32 {
        self.time
    }

    /// Whether the script has run its length.
    pub fn done(&self, scenario: &Scenario) -> bool {
        self.time >= scenario.duration
    }

    /// One frame of `dt` seconds, for a character whose legs hold `legs`
    /// as it begins: the script's time for the frame, and the gate that
    /// presses its key in it, if one does.
    pub fn frame<'a>(&mut self, scenario: &'a Scenario, dt: f32, legs: f32) -> (f32, Option<&'a Gate>) {
        let Some(gate) = scenario.gates.get(self.passed).filter(|gate| self.time + dt >= gate.at) else {
            self.time += dt;
            return (self.time, None);
        };
        self.time = gate.at;
        let (shut_for, until_press) = match self.shut {
            Some((shut_for, until_press)) => (shut_for + dt, until_press - dt),
            None => (0.0, 0.0),
        };
        let open = match gate.opens {
            Opens::LegsBelow(figure) => legs < figure,
            Opens::LegsFrom(figure) => legs >= figure,
        };
        if open || shut_for >= gate.timeout {
            self.shut = None;
            self.passed += 1;
            return (self.time, None);
        }
        match &gate.press {
            Some((_, every)) if until_press <= 0.0 => {
                self.shut = Some((shut_for, *every));
                (self.time, Some(gate))
            }
            _ => {
                self.shut = Some((shut_for, until_press));
                (self.time, None)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn script(gates: Vec<Gate>) -> Scenario {
        Scenario { name: String::new(), duration: 3.0, holds: Vec::new(), turns: Vec::new(), presses: Vec::new(), gates }
    }

    /// The clock stands at a gate until the legs are where it wants them,
    /// pressing the gate's key at its spacing, and then runs on; a gate
    /// that never opens gives way at its timeout.
    #[test]
    fn a_gate_holds_the_clock_until_the_legs_open_it() {
        let scenario = script(vec![
            Gate { at: 1.0, opens: Opens::LegsBelow(15.0), press: Some(("jump".into(), 0.25)), timeout: 60.0 },
            Gate { at: 1.5, opens: Opens::LegsFrom(16.0), press: None, timeout: 2.0 },
        ]);
        let (mut clock, mut legs, mut presses, mut frames) = (Clock::default(), 40.0f32, Vec::new(), 0);
        // Each press takes 14 off the legs; nothing refills them.
        while clock.time() < 1.5 {
            let (time, pressed) = clock.frame(&scenario, 0.0078125, legs);
            frames += 1;
            if pressed.is_some() {
                presses.push(frames);
                legs -= 14.0;
            }
            assert!(time <= 1.5 && frames < 1000);
        }
        // The gate is reached on the 128th frame of 1/128 s and presses at
        // once, then every 32 frames: two presses take the legs to 12. The
        // gate opens on the frame after and the clock runs on to the next.
        assert_eq!(presses, [128, 160]);
        assert_eq!(frames, 128 + 33 + 64, "{}", clock.time());
        // The second gate never opens on legs of 12: two seconds, then on.
        let mut waited = 0;
        while clock.time() <= 1.5 {
            clock.frame(&scenario, 0.0078125, legs);
            waited += 1;
        }
        assert!((256..=258).contains(&waited), "{waited}");
        assert!(!clock.done(&scenario));
    }
}
