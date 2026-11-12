//! What the battle view shows, its modes and components, and the fader.

use super::*;

/// Where the two panels' objects are numbered among the drawn pieces (past
/// the inventory's, `health_view`).
pub(super) const STANCE: usize = 11 << 20;

pub(super) const BODY: usize = 12 << 20;

pub(super) const QUICK_BAR: usize = 13 << 20;

pub(super) const SPAN: usize = 1 << 20;

/// When one of the panels shows (`EVisibilityMode`): always, a while after
/// what raises it (the game's default), or never.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Shows {
    Always,
    #[default]
    Autohide,
    Never,
}

impl Shows {
    /// In the order of the game's own list of them (autohide, always
    /// shown, always hidden).
    pub const ALL: [Shows; 3] = [Shows::Autohide, Shows::Always, Shows::Never];

    /// Its name as the game keeps it (`Game.ini`), and the key of its words
    /// in the install's texts.
    pub fn name(self) -> &'static str {
        match self {
            Shows::Always => "Always",
            Shows::Autohide => "Autohide",
            Shows::Never => "Never",
        }
    }

    pub fn of(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|shows| shows.name() == name)
    }
}

/// The settings the battle screen's panels go by (the game's own: its
/// settings' GAME tab, `GameSettingsGroup`): when the quick bar, the stance
/// panel and the health panel show, and how the parts are coloured. As
/// the run begins, the install's (`Game.ini`; where it says nothing, the
/// game's defaults, autohide, autohide, autohide and white to red), or
/// what the options' HUD page has kept over them (`options_view`); a
/// change there is taken as the battle screen is next shown (the
/// inventory shut), as the game's panels read them as they are shown.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct BattleModes {
    pub quick: Shows,
    pub stamina: Shows,
    pub health: Shows,
    /// The parts coloured green to red (`EHealthColorScheme.GreenToRed`,
    /// "Polychrome") rather than white to red ("Monochrome").
    pub green_to_red: bool,
    /// How the weapon's tactical key works (`TacticalInputMode`: Press,
    /// the game's default, or Hold), which T+B flips and keeps at once
    /// (tactical-devices spec, "The keys"; `tactical_view`).
    pub tactical: trp_sim::weapons::tactical::InputMode,
}

impl BattleModes {
    pub fn read(install: &trp_data::Install) -> Self {
        let shows = |name: &str| install.game_setting(name).as_deref().and_then(Shows::of).unwrap_or_default();
        Self {
            quick: shows("QuickSlotsVisibility"),
            stamina: shows("StaminaVisibility"),
            health: shows("HealthVisibility"),
            green_to_red: install.game_setting("HealthColor").as_deref() == Some("GreenToRed"),
            tactical: install.game_setting("TacticalInputMode").as_deref().and_then(trp_sim::weapons::tactical::InputMode::of_word).unwrap_or_default(),
        }
    }

    /// The scheme's name as the game keeps it.
    pub fn color_name(&self) -> &'static str {
        if self.green_to_red {
            "GreenToRed"
        } else {
            "WhiteToRed"
        }
    }
}

/// One of the panels as drawn, made afresh as the window's size
/// changes: which (0 the stance's, 1 the health's, 2 the quick bar) and
/// for what size.
#[derive(Component)]
pub(super) struct BattleRoot {
    pub(super) panel: usize,
    pub(super) size: [u32; 2],
}

/// One of the twenty notches of the speed the stance panel's script makes
/// (`BattleStanceNotch`, at a twentieth of the top speed each).
#[derive(Component)]
pub(super) struct SpeedNotch(pub(super) usize);

/// An effect's icon on the health panel (`StackedEffectIcon`).
#[derive(Component)]
pub(super) struct BattleEffect;

/// What the quick bar was drawn for: the window, the thing in the hands,
/// and each view's key, words and thing; drawn afresh when any changes.
#[derive(Component, PartialEq)]
pub(super) struct QuickDrawn(pub(super) Vec<String>);

/// The colours a drawn object has of its own, before its panel's fade
/// (`CanvasGroup.alpha`, which every picture and text under the panel is
/// drawn at besides): its picture's, its ground's and its words'.
#[derive(Component, Clone, Copy)]
pub(super) struct Own {
    pub(super) image: Option<Color>,
    pub(super) ground: Option<Color>,
    pub(super) text: Option<Color>,
}

/// A panel's fade (`BattleUIComponentAnimation`): its alpha, and what it is
/// doing, one step after another: waiting so long, or fading to an alpha
/// in half a second from where it stands (`DOFade`, DOTween's default
/// ease, out quad). A step that ends hands over on that frame and the rest
/// of the frame's time is dropped: the game awaits each fade, then starts
/// its `Task.Delay`, and a fade made when a delay ends is first stepped on
/// the next frame (`courses8`'s check: the fire mode's panel, every frame).
#[derive(Default)]
pub(crate) struct Fader {
    pub(crate) alpha: f32,
    pub(super) steps: Vec<Step>,
    pub(super) at: f32,
    pub(super) from: f32,
}

#[derive(Clone, Copy)]
pub(super) enum Step {
    Wait(f32),
    Fade(f32),
}

/// The commands the quick bar's own input node takes as a key goes down
/// (`QuickUseSelectorUtil.SlotBySelectCommandDictionary`): the grenade's
/// and the quick slots 4 to 0. A slot is used on its key coming up, by
/// another command (`SelectFastSlot4` and on: `inventory_view`).
pub(super) const QUICK_PRESSES: [&str; 8] = ["ThrowGrenade", "PressSlot4", "PressSlot5", "PressSlot6", "PressSlot7", "PressSlot8", "PressSlot9", "PressSlot0"];

/// `BattleUIComponentAnimation`'s half a second's fade, and its three
/// seconds' stay before a panel raised a while fades out.
const FADE_TIME: f32 = 0.5;

pub(super) const VISIBILITY_DURATION: f32 = 3.0;

impl Fader {
    /// `Show`: to whole, then, raised a while, three seconds and out.
    pub(crate) fn show(&mut self, autohide: bool) {
        self.steps = vec![Step::Fade(1.0)];
        if autohide {
            self.steps.extend([Step::Wait(VISIBILITY_DURATION), Step::Fade(0.0)]);
        }
        self.begin();
    }

    /// `Hide`: after the delay, out.
    pub(crate) fn hide(&mut self, delay: f32) {
        self.steps = if delay > 0.0 { vec![Step::Wait(delay), Step::Fade(0.0)] } else { vec![Step::Fade(0.0)] };
        self.begin();
    }

    /// `Close`: gone at once.
    pub(crate) fn close(&mut self) {
        self.steps.clear();
        self.alpha = 0.0;
    }

    fn begin(&mut self) {
        self.at = 0.0;
        self.from = self.alpha;
    }

    pub(crate) fn advance(&mut self, dt: f32) {
        let Some(&step) = self.steps.first() else { return };
        let length = match step {
            Step::Wait(time) => time,
            Step::Fade(_) => FADE_TIME,
        };
        if dt < length - self.at {
            self.at += dt;
            if let Step::Fade(to) = step {
                let x = self.at / FADE_TIME;
                self.alpha = self.from + (to - self.from) * (1.0 - (1.0 - x) * (1.0 - x));
            }
            return;
        }
        if let Step::Fade(to) = step {
            self.alpha = to;
        }
        self.steps.remove(0);
        self.begin();
    }
}

/// What the panels' scripts follow of the character, as last seen: the
/// stance's target and whether lying, the top speed and the limit on it,
/// the speed the panel shows, each stamina pool's changes, the sprint, and
/// health's raising of its panel.
#[derive(Clone, Copy, PartialEq)]
pub(super) struct Seen {
    pub(super) pose_level: f32,
    pub(super) prone: bool,
    pub(super) max_speed: f32,
    pub(super) limit: f32,
    pub(super) speed: f32,
    pub(super) legs: u32,
    pub(super) arms: u32,
    pub(super) sprinting: bool,
    pub(super) pulls: [u32; 2],
}

/// The stance's figures stepped through one at a time (`LadderSetActive`):
/// the one the last ladder reached (`_currentStanceIndex`), the one shown,
/// the one it goes to, and the frames until the next step (two a step).
#[derive(Default)]
pub(super) struct Ladder {
    pub(super) reached: usize,
    pub(super) shown: usize,
    pub(super) to: usize,
    pub(super) wait: u32,
}

impl Ladder {
    pub(super) fn step(&mut self) {
        if self.shown != self.to {
            self.shown = if self.to > self.shown { self.shown + 1 } else { self.shown - 1 };
            self.wait = 2;
        } else {
            self.reached = self.to;
            self.wait = 0;
        }
    }
}
