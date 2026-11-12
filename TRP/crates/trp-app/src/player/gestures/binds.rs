//! The menu's picks, the Gestures state and the binds kept.

use super::*;

/// What a click in the menu or a key can ask for: a hand sign by its
/// game's number, or a phrase by its trigger (`GestureCommands`: one number
/// either way, a hand sign's checked first).
pub(super) fn is_sign(number: i32) -> Option<Sign> {
    Sign::GESTURES.into_iter().find(|sign| sign.game_number() as i32 == number)
}

/// What the pointer is over in the menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum Pick {
    Sign(usize),
    /// A phrase: its group and its trigger.
    Phrase(usize, i32),
    /// A bind panel's slot.
    Slot(usize),
}

/// The binding menu: what it binds, where it opened, and the line under
/// the pointer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct BindMenu {
    pub(super) number: i32,
    pub(super) at: Vec2,
    pub(super) hovered: Option<usize>,
}

#[derive(Resource)]
pub struct Gestures {
    /// What each of F1 to F12 says or shows, 0 for nothing.
    pub binds: [i32; 12],
    /// The menu is open.
    pub open: bool,
    /// The quick list: each active phrase and its priority now
    /// (`_activeCommandsList`).
    pub(super) quick: HashMap<i32, i32>,
    /// The phrases active by a situation event, and until when.
    pub(super) timed: Vec<(i32, f64)>,
    /// What held each lasting situation last frame: its phrases, and what
    /// is looked at and the effects counted, to tell a fresh one.
    pub(super) held: HashSet<i32>,
    pub(super) looked: Option<(u8, u64)>,
    pub(super) effects: [usize; 5],
    /// The held list: when its wait began; whether it is up, and its row.
    pub(super) list_wait: Option<f64>,
    pub(super) list: Option<usize>,
    pub(super) quick_barred_until: f64,
    /// The prompt's phrase last said, and until when it blinks.
    pub(super) last_quick: Option<i32>,
    pub(super) blink: Option<(f64, f64)>,
    /// The menu's pointer, the group open under it, the binding menu.
    pub(super) pick: Option<Pick>,
    pub(super) group_open: Option<usize>,
    pub(super) bind_menu: Option<BindMenu>,
    /// What the menu and the prompt were last drawn as.
    pub(super) menu_drawn: Option<u64>,
    pub(super) prompt_drawn: Option<u64>,
}

impl Default for Gestures {
    fn default() -> Self {
        Self {
            binds: default_binds(),
            open: false,
            quick: HashMap::new(),
            timed: Vec::new(),
            held: HashSet::new(),
            looked: None,
            effects: [0; 5],
            list_wait: None,
            list: None,
            quick_barred_until: 0.0,
            last_quick: None,
            blink: None,
            pick: None,
            group_open: None,
            bind_menu: None,
            menu_drawn: None,
            prompt_drawn: None,
        }
    }
}

impl Gestures {
    /// The prompt's phrase: the highest priority, a tie to the name last
    /// in the alphabet (`PrioritizedCommand`).
    pub(super) fn top(&self) -> Option<i32> {
        self.quick.iter().max_by(|a, b| a.1.cmp(b.1).then_with(|| phrase::name(*a.0).cmp(phrase::name(*b.0)))).map(|(p, _)| *p)
    }

    /// The held list's rows: the active phrases by number (`RearrangeViews`).
    pub(super) fn rows(&self) -> Vec<i32> {
        let mut rows: Vec<i32> = self.quick.keys().copied().collect();
        rows.sort_unstable();
        rows
    }

    /// A phrase's situation holds (`SetPhraseActive` on): added to the
    /// quick list, or its priority restored where it was there already
    /// (not a hurt's).
    pub(super) fn activate(&mut self, trigger: i32, voiced: &HashSet<i32>) {
        let Some(priority) = phrase::priority(trigger) else { return };
        if !voiced.contains(&trigger) {
            return;
        }
        match self.quick.get_mut(&trigger) {
            Some(now) if !phrase::hurt(trigger) => *now = priority,
            Some(_) => {}
            None => {
                self.quick.insert(trigger, priority);
            }
        }
    }

    pub(super) fn deactivate(&mut self, trigger: i32) {
        self.quick.remove(&trigger);
        if self.quick.is_empty() {
            self.list = None;
        }
    }

    /// A situation the game raises for 5 s (`GesturesQuickPanel.
    /// PlayerPhraseSituation`): active, and until 5 s from now, a raise
    /// while it lasts putting its end off again (`Expires = now + 5`).
    pub(super) fn raise(&mut self, trigger: i32, now: f64, voiced: &HashSet<i32>) {
        self.timed.retain(|(t, _)| *t != trigger);
        self.timed.push((trigger, now + SITUATION_LIFE));
        self.activate(trigger, voiced);
    }

    /// The raised situations whose 5 s are over, gone, but where a lasting
    /// situation of the same phrase holds it.
    pub(super) fn expire(&mut self, now: f64) {
        let expired: Vec<i32> = self.timed.iter().filter(|(_, until)| *until <= now).map(|(t, _)| *t).collect();
        self.timed.retain(|(_, until)| *until > now);
        for trigger in expired {
            if !self.held.contains(&trigger) {
                self.deactivate(trigger);
            }
        }
    }

    /// Whether a phrase may be said now (`IsPhraseAvailable`): not
    /// situational, or its situation holding.
    pub(super) fn available(&self, trigger: i32) -> bool {
        !phrase::situational(trigger) || self.quick.contains_key(&trigger)
    }

    /// Binds a hand sign or a phrase to a key (`bind_into`).
    pub fn bind(&mut self, key: usize, number: i32) {
        bind_into(&mut self.binds, Some(key), number);
    }
}

/// The binds as the game has them with nothing kept (`GesturesCommandsStorage.
/// GetDefault`): F1 the mumble.
pub fn default_binds() -> [i32; 12] {
    let mut binds = [0; 12];
    binds[0] = phrase::MUMBLE;
    binds
}

/// Binds a hand sign or a phrase to a key, or to none (`GesturesMenu.Bind`):
/// it leaves any key it had, and the key's old one is dropped.
pub fn bind_into(binds: &mut [i32; 12], key: Option<usize>, number: i32) {
    for bound in binds.iter_mut() {
        if *bound == number {
            *bound = 0;
        }
    }
    if let Some(slot) = key.and_then(|key| binds.get_mut(key)) {
        *slot = number;
    }
}

/// The binds as the options' file keeps them: `{"F1": 104, ...}`, a key
/// missing holding nothing.
pub fn kept(file: &std::path::Path) -> Option<[i32; 12]> {
    let all = crate::platform::options_store::read(file)?;
    let kept = all.get(KEPT_AS)?.as_object()?;
    let mut binds = [0; 12];
    for (at, bind) in binds.iter_mut().enumerate() {
        *bind = kept.get(&format!("F{}", at + 1)).and_then(serde_json::Value::as_i64).unwrap_or(0) as i32;
    }
    Some(binds)
}

/// Keeps the binds in the options' file, all else in it as it was.
pub fn keep(file: &std::path::Path, binds: &[i32; 12]) {
    let map: serde_json::Map<String, serde_json::Value> = binds.iter().enumerate().filter(|(_, n)| **n != 0).map(|(at, n)| (format!("F{}", at + 1), serde_json::json!(n))).collect();
    crate::platform::options_store::keep(file, KEPT_AS, Some(serde_json::Value::Object(map)), "the gestures' binds");
}
