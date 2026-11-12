//! The app's own stance, speed and stamina display, and the debug panel at the top right.
//!
//! The stance, speed and stamina display at the bottom left, showing what
//! the game's own panel there shows (`docs/spec/stamina.md`, "The game's
//! own display"): how high the character stands, the speed setting over a
//! row of notches lit up to the movement state's limit, and how full the
//! legs and the arms are. The drawing is plain bars; the game's art for
//! it is not read.
//!
//! And, at the top right, a panel of the app's own in the inventory
//! screen's colours: **the frame rate**, **the debug
//! figures** (what the movement and the thing in hand are doing just now)
//! and **the commands** (`debug_menu`; the guide to the keys that stood
//! there went, the keys being the options' CONTROLS tab's). `` ` `` shows it
//! and hides it (`F1` went to the gestures' keys), the
//! mouse freed while it is shown; hidden as a run begins (`--fps on`
//! starts with it shown), and while the inventory screen is open.

use bevy::prelude::*;

use crate::player::animation::ClipPlayer;
use crate::player::controller::Controller;
use crate::screens::inventory_view::{words, BACK, FAINT, LINE, WORDS};

use crate::weapons::weapon::HandsRig;
use crate::Options;

mod debug_guide;
mod meters;

pub use debug_guide::*;
use debug_guide::{PlainMeters, ScreenHides, guide, hidden_over_screen, hide_over_screen, label, spawn_top};
use meters::{frame_rate, show, spawn};

/// The game's panel has a notch every twentieth of the top speed and one
/// at the top.
/// The key that shows the panel and hides it: the one left of `1`
/// (`Backquote`, the game's `Console`, not built here), written `~` on
/// the panel.
pub const PANEL_KEY: KeyCode = KeyCode::Backquote;

const NOTCHES: usize = 21;
const BAR_WIDTH: f32 = 168.0;
const NOTCH_WIDTH: f32 = 6.0;
const STANCE_HEIGHT: f32 = 64.0;

const TRACK: Color = Color::srgba(0.0, 0.0, 0.0, 0.55);
const LEGS: Color = Color::srgb(0.80, 0.86, 0.78);
const ARMS: Color = Color::srgb(0.86, 0.80, 0.62);
const SPENT: Color = Color::srgb(0.82, 0.27, 0.20);
const SET: Color = Color::srgb(0.92, 0.92, 0.88);
const ALLOWED: Color = Color::srgba(0.92, 0.92, 0.88, 0.32);
const REFUSED: Color = Color::srgba(0.0, 0.0, 0.0, 0.55);
const LABEL: Color = Color::srgba(0.92, 0.92, 0.88, 0.8);

/// A bar that fills along its length.
#[derive(Component)]
enum Fill {
    Legs,
    Arms,
    Stance,
}

/// One notch of the speed row, counted from the slow end.
#[derive(Component)]
struct Notch(usize);

/// The line that says the stance in words.
#[derive(Component)]
struct StanceWord;

/// The frame-rate readout, in the panel at the top right.
#[derive(Component)]
struct FrameRateText;

/// What the readout is worked out from: the last second of drawn frames,
/// newest last, each with how long it took by the engine's clock and what
/// the simulation cost in it, in seconds.
#[derive(Resource, Default)]
struct FrameTimes {
    frames: std::collections::VecDeque<(f32, f32)>,
    /// Seconds since the readout was last written.
    since: f32,
}

/// How much time the readout sums up, and how often it is rewritten, in
/// seconds: often enough to follow, seldom enough to read.
const FRAME_WINDOW: f32 = 1.0;
const FRAME_REFRESH: f32 = 0.25;

/// The debug panel's figures, each under a name of its own, with the
/// least room it is given so that the row does not shift as figures
/// change: the movement state, the speed over the ground, the speed
/// setting as smoothed and as limited, the stance, the lean, the sprint's
/// pace, the weapon (its fire mode and rounds), the state of the animator
/// of the thing in hand, and a script being played.
const DEBUG: [(&str, f32); 9] =
    [("STATE", 92.0), ("SPEED", 66.0), ("SETTING", 70.0), ("STANCE", 48.0), ("LEAN", 40.0), ("SPRINT", 48.0), ("WEAPON", 120.0), ("HANDS", 150.0), ("SCRIPT", 0.0)];
const SCRIPT_CELL: usize = 8;

/// A figure of the debug panel, and the cell it stands in, by its place.
#[derive(Component)]
pub struct DebugValue(usize);
#[derive(Component)]
pub struct DebugCell(usize);

/// The panel at the top right, the guide to the keys in it, and whether
/// the player wants it shown.
#[derive(Component)]
struct DebugPopup;
/// The small tag at the top right that says how to open it, while it is
/// shut.
#[derive(Component)]
struct DebugTag;
#[derive(Resource)]
pub struct GuideShown(pub bool);

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FrameTimes>()
            .add_systems(Startup, (spawn, spawn_top))
            // The guide after a capture's own presses are in, which come
            // before the weapon's clips: a key pressed by hand is in sooner.
            .add_systems(Update, (show, frame_rate, guide.after(crate::player::animation::play_clips), hide_over_screen));
    }
}
