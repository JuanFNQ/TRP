//! The whole trace: frames, screens shown, events and what followed.

use super::*;

/// A `.ui.jsonl` file read: its lines in order, and those not read.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Trace {
    pub lines: Vec<Line>,
    /// The lines not read: their numbers (from 1) and why.
    pub bad: Vec<(usize, String)>,
}

/// A screen and the frames it was shown on (`None`: no screen).
#[derive(Clone, Debug, PartialEq)]
pub struct Shown {
    pub screen: Option<String>,
    /// First and last frame of each time it was shown, in order.
    pub spans: Vec<(i64, i64)>,
}

/// The most elements the interface held at once, and the most drawn, with the frames.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Most {
    pub elements: usize,
    pub elements_frame: i64,
    pub drawn: usize,
    pub drawn_frame: i64,
}

/// What happened to an element on a frame: added (with all its fields), changed (with the
/// fields changed), or gone.
#[derive(Clone, Debug, PartialEq)]
pub enum Happened {
    Added(Vec<(String, Value)>),
    Changed(Vec<(String, Value)>),
    Gone,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Event {
    pub frame: i64,
    pub t: f64,
    pub happened: Happened,
}

/// An element followed through the file: its id, path and kind, and everything that happened
/// to it.
#[derive(Clone, Debug, PartialEq)]
pub struct Followed {
    pub id: i64,
    pub path: String,
    pub kind: String,
    pub events: Vec<Event>,
}

impl Trace {
    /// Reads a file's text: a line at a time, blank lines passed over, a line not read kept in
    /// `bad`.
    pub fn parse(text: &str) -> Trace {
        let mut trace = Trace::default();
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        for (index, raw) in text.lines().enumerate() {
            let raw = raw.trim();
            if raw.is_empty() {
                continue;
            }
            match Line::parse(index + 1, raw) {
                Ok(line) => trace.lines.push(line),
                Err(why) => trace.bad.push((index + 1, why)),
            }
        }
        trace
    }

    pub fn read(path: &Path) -> Result<Trace> {
        let text = std::fs::read_to_string(path).with_context(|| path.display().to_string())?;
        Ok(Trace::parse(&text))
    }

    pub fn first_frame(&self) -> Option<i64> {
        self.lines.first().map(|line| line.frame)
    }

    pub fn last_frame(&self) -> Option<i64> {
        self.lines.last().map(|line| line.frame)
    }

    /// The interface after every line of a frame at or before `frame`.
    pub fn state_at(&self, frame: i64) -> State {
        let mut state = State::default();
        for line in self.lines.iter().filter(|line| line.frame <= frame) {
            state.apply(line);
        }
        state
    }

    /// The frame of the last line at or before the time `t` (seconds).
    pub fn frame_at_t(&self, t: f64) -> Option<i64> {
        self.lines.iter().rev().find(|line| line.t <= t).map(|line| line.frame)
    }

    /// Each screen with the frames it was shown on, in the order first shown. A screen holds
    /// from the line that names it to the frame before the next that names another, the last
    /// to the file's last frame; before any line names one there is none.
    pub fn screens(&self) -> Vec<Shown> {
        let (Some(first), Some(last)) = (self.first_frame(), self.last_frame()) else { return Vec::new() };
        let mut spans: Vec<(Option<String>, i64, i64)> = Vec::new();
        let mut current: Option<String> = None;
        let mut since = first;
        for line in &self.lines {
            let Some(screen) = &line.screen else { continue };
            if *screen == current {
                continue;
            }
            if line.frame > since {
                spans.push((current.clone(), since, line.frame - 1));
            }
            current = screen.clone();
            since = line.frame;
        }
        spans.push((current, since, last.max(since)));
        let mut shown: Vec<Shown> = Vec::new();
        for (screen, from, to) in spans {
            match shown.iter_mut().find(|s| s.screen == screen) {
                Some(s) => s.spans.push((from, to)),
                None => shown.push(Shown { screen, spans: vec![(from, to)] }),
            }
        }
        shown
    }

    /// The most elements held at once and the most drawn, replaying every line (those drawn
    /// counted again only on a line that changes elements or canvases).
    pub fn most(&self) -> Most {
        let mut most = Most::default();
        let mut state = State::default();
        for line in &self.lines {
            state.apply(line);
            if line.add.is_empty() && line.set.is_empty() && line.gone.is_empty() && line.canvases.is_empty() {
                continue;
            }
            if state.elements.len() > most.elements {
                most.elements = state.elements.len();
                most.elements_frame = line.frame;
            }
            let drawn = state.elements.values().filter(|element| state.is_drawn(element)).count();
            if drawn > most.drawn {
                most.drawn = drawn;
                most.drawn_frame = line.frame;
            }
        }
        most
    }

    /// Every sound with its frame and time, in order.
    pub fn sounds(&self) -> Vec<(i64, f64, &[String])> {
        self.lines.iter().flat_map(|line| line.sounds.iter().map(move |sound| (line.frame, line.t, sound.as_slice()))).collect()
    }

    /// Every element whose path or words (at any time) hold `text`, ignoring case, in the order
    /// first seen, with everything that happened to it.
    pub fn follow(&self, text: &str) -> Vec<Followed> {
        let needle = text.to_lowercase();
        let holds = |value: &Value| value.as_str().is_some_and(|s| s.to_lowercase().contains(&needle));
        let mut found: BTreeMap<i64, usize> = BTreeMap::new();
        let mut followed: Vec<Followed> = Vec::new();
        for line in &self.lines {
            for change in line.add.iter().chain(&line.set) {
                let matches = change.fields.iter().any(|(key, value)| (key == "path" || key == "words") && holds(value));
                if matches && !found.contains_key(&change.id) {
                    found.insert(change.id, followed.len());
                    followed.push(Followed { id: change.id, path: String::new(), kind: String::new(), events: Vec::new() });
                }
            }
        }
        for line in &self.lines {
            for change in &line.add {
                let Some(&at) = found.get(&change.id) else { continue };
                let element = &mut followed[at];
                if let Some(path) = change.field("path").and_then(Value::as_str) {
                    element.path = path.to_owned();
                }
                if let Some(kind) = change.field("kind").and_then(Value::as_str) {
                    element.kind = kind.to_owned();
                }
                element.events.push(Event { frame: line.frame, t: line.t, happened: Happened::Added(change.fields.clone()) });
            }
            for change in &line.set {
                let Some(&at) = found.get(&change.id) else { continue };
                followed[at].events.push(Event { frame: line.frame, t: line.t, happened: Happened::Changed(change.fields.clone()) });
            }
            for id in &line.gone {
                let Some(&at) = found.get(id) else { continue };
                followed[at].events.push(Event { frame: line.frame, t: line.t, happened: Happened::Gone });
            }
        }
        followed
    }
}
