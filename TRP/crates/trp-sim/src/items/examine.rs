//! Examining a thing, and which kinds of thing the character knows.
//! After `docs/spec/inventory.md`, "Examining": which kinds of thing
//! the character knows (`InventoryController.Examined`, the profile's
//! `Encyclopedia`), and the examine that makes one known
//! (`ExamineOperation`): its time, the item's `ExamineTime` over 1 +
//! Attention's buff, waited out, then its template learned.
//!
//! The list is the process's own, set once as a run begins: none set,
//! everything is known (the shooting range's controller, `examined: true`;
//! the switch `EXAMINING` off, the replay, the tests that do not ask).

use std::collections::HashSet;
use std::sync::{Arc, RwLock};

use crate::items::inventory::{Id, Item, Kind};

/// The character's list: the templates it began knowing, and those it has
/// learned since, in the order learned (`Profile.LearnInEncyclopedia`).
struct List {
    start: HashSet<String>,
    learned: Vec<String>,
}

static LIST: RwLock<Option<List>> = RwLock::new(None);

/// Sets the list: what the character begins knowing and what it has
/// learned before.
pub fn set_list(start: impl IntoIterator<Item = String>, learned: impl IntoIterator<Item = String>) {
    let start: HashSet<String> = start.into_iter().collect();
    let mut kept = Vec::new();
    for template in learned {
        if !start.contains(&template) && !kept.contains(&template) {
            kept.push(template);
        }
    }
    *LIST.write().unwrap_or_else(|e| e.into_inner()) = Some(List { start, learned: kept });
}

/// No list: everything is known.
pub fn know_everything() {
    *LIST.write().unwrap_or_else(|e| e.into_inner()) = None;
}

/// Whether there is a list at all.
pub fn on() -> bool {
    LIST.read().unwrap_or_else(|e| e.into_inner()).is_some()
}

/// Whether the character knows that kind (`InventoryController.Examined`):
/// known by default, or on the list, or there being no list.
pub fn knows(kind: &Kind) -> bool {
    if kind.examined_by_default {
        return true;
    }
    match &*LIST.read().unwrap_or_else(|e| e.into_inner()) {
        None => true,
        Some(list) => list.start.contains(&kind.template) || list.learned.iter().any(|t| *t == kind.template),
    }
}

/// The same of a thing.
pub fn known(item: &Item) -> bool {
    knows(&item.kind)
}

/// Adds templates to what the character began knowing (not kept as
/// learned).
pub fn know_from_start(templates: impl IntoIterator<Item = String>) {
    if let Some(list) = &mut *LIST.write().unwrap_or_else(|e| e.into_inner()) {
        for template in templates {
            list.learned.retain(|t| *t != template);
            list.start.insert(template);
        }
    }
}

/// Learns a template (`Profile.LearnInEncyclopedia`); whether it was new.
pub fn learn(template: &str) -> bool {
    match &mut *LIST.write().unwrap_or_else(|e| e.into_inner()) {
        Some(list) if !list.start.contains(template) && !list.learned.iter().any(|t| t == template) => {
            list.learned.push(template.to_owned());
            true
        }
        _ => false,
    }
}

/// What has been learned since the start, in the order learned.
pub fn learned() -> Vec<String> {
    LIST.read().unwrap_or_else(|e| e.into_inner()).as_ref().map_or_else(Vec::new, |list| list.learned.clone())
}

/// Forgets all that was learned (the debug menu's FORGET EXAMINED); how
/// many it was.
pub fn forget() -> usize {
    match &mut *LIST.write().unwrap_or_else(|e| e.into_inner()) {
        Some(list) => std::mem::take(&mut list.learned).len(),
        None => 0,
    }
}

/// How long an examine of that kind takes, in seconds (`ExamineOperation`):
/// its `ExamineTime` over 1 + Attention's `AttentionExamine`.
pub fn time(kind: &Kind, attention: f32) -> f32 {
    kind.examine_time / (1.0 + attention)
}

/// An examine under way.
#[derive(Debug, Clone, PartialEq)]
pub struct Examination {
    pub item: Id,
    pub kind: Arc<Kind>,
    /// Its whole time, in seconds, and when it began.
    pub time: f32,
    pub began: f64,
}

impl Examination {
    /// The share of it done by then, 0 to 1 (`ItemViewExamineComponent`:
    /// the time gone over the whole).
    pub fn done(&self, now: f64) -> f32 {
        if self.time <= 0.0 {
            return 1.0;
        }
        (((now - self.began) / self.time as f64) as f32).clamp(0.0, 1.0)
    }
}

/// Why an examine was not begun.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    /// Its kind is known: nothing happens.
    Known,
    /// Another is under way ("You can't examine two items at the same
    /// time").
    Busy,
}

/// The character's examining (`InventoryController._examining`): one at a
/// time.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Examiner {
    pub under_way: Option<Examination>,
}

impl Examiner {
    /// Begins an examine of a thing (`InventoryController.Examine`): refused
    /// for a thing known, then for one while another is under way.
    pub fn begin(&mut self, item: &Item, attention: f32, now: f64) -> Result<&Examination, Refused> {
        if known(item) {
            return Err(Refused::Known);
        }
        if self.under_way.is_some() {
            return Err(Refused::Busy);
        }
        let time = time(&item.kind, attention);
        Ok(self.under_way.insert(Examination { item: item.id, kind: item.kind.clone(), time, began: now }))
    }

    /// Whether that thing is being examined.
    pub fn examining(&self, id: Id) -> bool {
        self.under_way.as_ref().is_some_and(|under_way| under_way.item == id)
    }

    /// The examine under way, ended if its time is out: its template
    /// learned (`ExamineOperation.ExecuteInternal`), whatever became of the
    /// thing meanwhile.
    pub fn finish(&mut self, now: f64) -> Option<Examination> {
        let done = self.under_way.as_ref().is_some_and(|under_way| now >= under_way.began + under_way.time as f64);
        let examination = if done { self.under_way.take() } else { None }?;
        learn(&examination.kind.template);
        Some(examination)
    }
}
