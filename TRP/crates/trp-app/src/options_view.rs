//! The options tab, the last of the inventory screen's tabs
//! (`docs/spec/settings.md`): the keys, the display, and a list of the
//! systems a host may come to switch on and off.
//!
//! The game keeps its settings on a screen of its menu; the user asked
//! for them here (2026-10-12). What the tab holds is the game's where the
//! game has it: the bindings are the install's, shown with the game's own
//! words for them and changed by the game's own rules
//! (`trp_sim::bindings`), and the display's settings are the four of the
//! game's the user named. How the tab looks is the app's.
//!
//! What is changed here is kept in the app's own file
//! (`local\options.json`), never in the install.

use std::collections::HashMap;
use std::path::PathBuf;

use bevy::app::AppExit;
use bevy::input::mouse::AccumulatedMouseScroll;
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;
use bevy::window::{Monitor, MonitorSelection, PresentMode, PrimaryWindow, VideoModeSelection, WindowMode};
use trp_sim::bindings::{code, name as key_name, Chosen, Controls, Listening, PressType, Source};

use crate::controller::Controller;
use crate::inventory_view::{words, Selected, BACK, FAINT, LINE, OPTIONS, PANEL, WORDS};
use crate::keys::{self, Bindings};
use crate::weapon::{HandsRig, Stowed};
use crate::Options;

/// How the window is shown, as the game's display setting has it
/// (`FullScreenMode`): the screen taken over, a borderless window over
/// all of it, or a window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Fullscreen,
    Borderless,
    Windowed,
}

impl Mode {
    const ALL: [Self; 3] = [Self::Fullscreen, Self::Borderless, Self::Windowed];

    fn word(self) -> &'static str {
        match self {
            Self::Fullscreen => "FULLSCREEN",
            Self::Borderless => "BORDERLESS",
            Self::Windowed => "WINDOWED",
        }
    }

    /// The game's own figure for a mode in its settings file.
    fn of_game(figure: i64) -> Self {
        match figure {
            0 => Self::Fullscreen,
            2 => Self::Windowed,
            _ => Self::Borderless,
        }
    }
}

/// The display's settings as they stand: whether frames wait for the
/// screen, the game's field of view (vertical degrees, 50 to 75), how the
/// window is shown and how many pixels it has.
#[derive(Debug, Clone, PartialEq)]
pub struct Graphics {
    pub vsync: bool,
    pub field_of_view: f32,
    pub mode: Mode,
    pub resolution: [u32; 2],
}

/// The game's limits for its field-of-view setting (`GameSettingsGroup`).
const FOV: [f32; 2] = [50.0, 75.0];

/// The game's settings of the mouse on its controls tab, as they stand
/// (`ControlSettingsGroup`): the sensitivity from the hip and with the
/// sights up, the sensitivity of a scope's zoom being adjusted (nothing
/// here has a scope: it is kept and does nothing), and the two switches
/// that turn an axis of the mouse about, across and up and down. The
/// double click's time, the fourth of the tab's bars, is the bindings'.
#[derive(Debug, Clone, PartialEq)]
pub struct Mouse {
    pub sensitivity: f32,
    pub aiming: f32,
    pub optic: f32,
    pub invert: [bool; 2],
}

/// What of the features' list is switched (settings spec, "Features"):
/// the app's own switches, none of them the game's. **Each is "restart
/// required"** (the user, 2026-10-14): as a resource, the switches as the
/// run began, which hold until it ends, so that nothing is made or
/// switched in play; one thrown on the tab is kept for the next run
/// (`OptionsState::features`), and the tab says so.
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct Features {
    /// `CUSTOM FIRST PERSON HANDS` (the user, 2026-10-13): an outfit's
    /// own arms are drawn for the character's own eyes. Off, the hands do
    /// not change with what is worn: they are the install's default arms.
    pub custom_hands: bool,
    /// `SMOOTH FIRST PERSON ANIMATIONS` (the user, 2026-10-14): every
    /// animation of the thing in hand (any weapon, empty hands) is drawn
    /// on curves without corners through its clips' samples. Off, all of
    /// them are drawn as the engine has them, the look all over a weapon
    /// with them. The drawing alone: the simulation is the same either
    /// way.
    pub smooth_animations: bool,
    /// `ICON GENERATION` (the user, 2026-10-14): the inventory's pictures
    /// are drawn from the items' models as the game draws them, and kept
    /// (`icons`). Off, none is drawn or read: each box has the plain shape
    /// of its sort.
    pub icon_generation: bool,
}

impl Default for Features {
    fn default() -> Self {
        Self { custom_hands: true, smooth_animations: true, icon_generation: true }
    }
}

/// A bar that is slid: the field of view, and the four of the game's
/// controls tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Slide {
    Fov,
    Sensitivity,
    Aiming,
    DoubleClick,
    Optic,
}

impl Slide {
    /// The game's own limits for each (`ControlSettingsGroup`,
    /// `GameSettingsGroup`).
    fn range(self) -> [f32; 2] {
        match self {
            Self::Fov => FOV,
            Self::Sensitivity | Self::Aiming => [0.1, 5.0],
            Self::DoubleClick => [0.3, 1.0],
            Self::Optic => [1.0, 5.0],
        }
    }

    /// What a bar is slid by: whole degrees, as the game's field of view
    /// is; hundredths for the rest (the app's: the game's bars are not
    /// stepped).
    fn step(self) -> f32 {
        if self == Self::Fov { 1.0 } else { 0.01 }
    }

    /// The figure as the game's own bar writes it (three places for the
    /// two sensitivities, two for the others).
    fn shown(self, value: f32) -> String {
        match self {
            Self::Fov => format!("{value:.0}"),
            Self::Sensitivity | Self::Aiming => format!("{value:.3}"),
            Self::DoubleClick | Self::Optic => format!("{value:.2}"),
        }
    }

    /// The game's own words for it, by the name its texts have them
    /// under, and the app's where the install has none.
    fn caption(self) -> (&'static str, &'static str) {
        match self {
            Self::Fov => ("", "FIELD OF VIEW"),
            Self::Sensitivity => ("Mouse sensitivity", "MOUSE SENSITIVITY"),
            Self::Aiming => ("Mouse sensitivity (aiming)", "MOUSE SENSITIVITY (AIMING)"),
            Self::DoubleClick => ("Settings/DoubleClickTimeout", "DOUBLE CLICK TIMEOUT"),
            Self::Optic => ("Settings/OpticSensitivity", "SCOPE ZOOM ADJUSTMENT SENSITIVITY"),
        }
    }
}

/// A cell of the list of bindings that takes keys: a variant of a
/// binding, or one way of one of an axis's pairs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Cell {
    Key(usize, usize),
    Axis(usize, usize, usize),
}

/// A key being chosen for a cell.
struct Listen {
    cell: Cell,
    heard: Listening,
    /// The click that began it is over: no key or button is down.
    armed: bool,
    /// The choice is made; it waits for every key to be let go.
    done: bool,
}

#[derive(Resource)]
pub struct OptionsState {
    /// Which of the tab's three parts is shown.
    sub: usize,
    listening: Option<Listen>,
    /// The binding whose way of being pressed is being chosen.
    asking: Option<usize>,
    note: String,
    /// How far down the list of bindings is scrolled, in pixels.
    scroll: f32,
    pub graphics: Graphics,
    /// What is switched of the features' list, as it is kept for the next
    /// run.
    pub features: Features,
    /// The switches as this run began, which it holds (`Features`): a
    /// switch thrown away from its own says so on the tab.
    pub started: Features,
    /// The mouse's settings, and what the install's own game has for them.
    pub mouse: Mouse,
    install_mouse: Mouse,
    /// The display's settings came from the app's file and are to be
    /// put on the window as the run begins.
    apply_at_start: bool,
    /// What the install's own game has for them.
    install_graphics: Graphics,
    /// The page is to be drawn afresh.
    dirty: bool,
    /// Where what is changed is kept; none in a run nobody is at the
    /// keys of, which neither reads nor writes it.
    file: Option<PathBuf>,
    /// The game's own words for the bindings, by their names in small
    /// letters (the install's English texts).
    names: HashMap<String, String>,
    /// The bar held by the mouse.
    sliding: Option<Slide>,
}

/// The app's own file for what is changed here: `options.json` in the
/// `local` folder beside the workspace (the nearest above where the app
/// is run from, as the install's path is looked for).
fn options_file() -> PathBuf {
    let here = std::env::current_dir().unwrap_or_default();
    let found = here.ancestors().map(|dir| dir.join("local")).find(|local| local.is_dir());
    found.unwrap_or_else(|| here.join("local")).join("options.json")
}

fn source_json(source: &Source) -> serde_json::Value {
    let keys: Vec<String> = source.keys.iter().map(|key| key_name(*key)).collect();
    match source.wheel {
        Some(up) => serde_json::json!({ "keys": keys, "wheel": up }),
        None => serde_json::json!({ "keys": keys }),
    }
}

fn source_of_json(value: &serde_json::Value) -> Source {
    let names = value.get("keys").and_then(|keys| keys.as_array());
    let keys = names.map(|names| names.iter().filter_map(|name| code(name.as_str()?)).collect()).unwrap_or_default();
    Source { keys, wheel: value.get("wheel").and_then(|up| up.as_bool()) }
}

fn save(path: &std::path::Path, controls: &Controls, graphics: &Graphics, mouse: &Mouse, features: &Features) -> std::io::Result<()> {
    let keys: Vec<serde_json::Value> = controls
        .keys
        .iter()
        .map(|group| serde_json::json!({ "name": group.name, "press": group.press.name(), "variants": group.variants.iter().map(source_json).collect::<Vec<_>>() }))
        .collect();
    let axes: Vec<serde_json::Value> = controls
        .axes
        .iter()
        .map(|axis| serde_json::json!({ "name": axis.name, "pairs": axis.pairs.iter().map(|pair| pair.iter().map(source_json).collect::<Vec<_>>()).collect::<Vec<_>>() }))
        .collect();
    let mode = match graphics.mode {
        Mode::Fullscreen => "fullscreen",
        Mode::Borderless => "borderless",
        Mode::Windowed => "windowed",
    };
    let all = serde_json::json!({
        "controls": { "double_click": controls.double_click, "keys": keys, "axes": axes },
        "mouse": { "sensitivity": mouse.sensitivity, "aiming_sensitivity": mouse.aiming, "optic_sensitivity": mouse.optic, "inverted_x": mouse.invert[0], "inverted_y": mouse.invert[1] },
        "graphics": { "vsync": graphics.vsync, "field_of_view": graphics.field_of_view, "mode": mode, "resolution": graphics.resolution },
        "features": {
            "custom_first_person_hands": features.custom_hands,
            "smooth_first_person_animations": features.smooth_animations,
            "icon_generation": features.icon_generation,
        },
    });
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder)?;
    }
    std::fs::write(path, serde_json::to_string_pretty(&all).unwrap_or_default())
}

/// Reads the app's file over the install's bindings and the display's
/// and the mouse's settings as the run began with them: a binding or an
/// axis the file names takes the file's keys, and the rest stay the
/// install's; a switch of the features' list the file has is as the file
/// has it. Says which of the first two parts the file had.
fn load(path: &std::path::Path, controls: &mut Controls, graphics: &mut Graphics, mouse: &mut Mouse, features: &mut Features) -> [bool; 2] {
    let Some(all) = std::fs::read_to_string(path).ok().and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok()) else { return [false; 2] };
    let switch = |name: &str| all.get("features").and_then(|kept| kept.get(name)?.as_bool());
    features.custom_hands = switch("custom_first_person_hands").unwrap_or(features.custom_hands);
    features.smooth_animations = switch("smooth_first_person_animations").unwrap_or(features.smooth_animations);
    features.icon_generation = switch("icon_generation").unwrap_or(features.icon_generation);
    if let Some(kept) = all.get("mouse") {
        let figure = |name: &str, kind: Slide, now: f32| kept.get(name).and_then(|v| v.as_f64()).map_or(now, |v| (v as f32).clamp(kind.range()[0], kind.range()[1]));
        mouse.sensitivity = figure("sensitivity", Slide::Sensitivity, mouse.sensitivity);
        mouse.aiming = figure("aiming_sensitivity", Slide::Aiming, mouse.aiming);
        mouse.optic = figure("optic_sensitivity", Slide::Optic, mouse.optic);
        for (at, name) in ["inverted_x", "inverted_y"].into_iter().enumerate() {
            mouse.invert[at] = kept.get(name).and_then(|v| v.as_bool()).unwrap_or(mouse.invert[at]);
        }
    }
    let list = |part: &str, list: &str| all.get(part).and_then(|part| part.get(list)?.as_array()).cloned().unwrap_or_default();
    let named = |entry: &serde_json::Value| entry.get("name").and_then(|name| name.as_str()).map(str::to_owned);
    for entry in list("controls", "keys") {
        let Some(group) = named(&entry).and_then(|name| controls.keys.iter_mut().find(|group| group.name == name)) else { continue };
        let variants = entry.get("variants").and_then(|variants| variants.as_array()).cloned().unwrap_or_default();
        group.variants = std::array::from_fn(|at| variants.get(at).map(source_of_json).unwrap_or_default());
        group.press = entry.get("press").and_then(|press| PressType::of(press.as_str()?)).unwrap_or(group.press);
    }
    for entry in list("controls", "axes") {
        let Some(axis) = named(&entry).and_then(|name| controls.axes.iter_mut().find(|axis| axis.name == name)) else { continue };
        let pairs = entry.get("pairs").and_then(|pairs| pairs.as_array()).cloned().unwrap_or_default();
        axis.pairs = std::array::from_fn(|pair| std::array::from_fn(|way| pairs.get(pair).and_then(|pair| pair.get(way)).map(source_of_json).unwrap_or_default()));
    }
    if let Some(seconds) = all.get("controls").and_then(|controls| controls.get("double_click")?.as_f64()) {
        controls.double_click = (seconds as f32).clamp(0.3, 1.0);
    }
    let Some(display) = all.get("graphics") else { return [all.get("controls").is_some(), false] };
    graphics.vsync = display.get("vsync").and_then(|vsync| vsync.as_bool()).unwrap_or(graphics.vsync);
    graphics.field_of_view = display.get("field_of_view").and_then(|fov| fov.as_f64()).map_or(graphics.field_of_view, |fov| (fov as f32).clamp(FOV[0], FOV[1]));
    graphics.mode = match display.get("mode").and_then(|mode| mode.as_str()) {
        Some("fullscreen") => Mode::Fullscreen,
        Some("borderless") => Mode::Borderless,
        Some("windowed") => Mode::Windowed,
        _ => graphics.mode,
    };
    let side = |at: usize| display.get("resolution")?.get(at)?.as_u64().map(|side| side as u32);
    if let (Some(width), Some(height)) = (side(0), side(1)) {
        graphics.resolution = [width.max(640), height.max(360)];
    }
    [all.get("controls").is_some(), true]
}

impl OptionsState {
    /// The options as the run begins: the display as the app starts it
    /// (a window of `size`, the frames waiting for the screen unless told
    /// otherwise), and, for someone at the keys, what the app's own file
    /// has over that and over the install's bindings. The field of view
    /// given on the command line stays.
    pub fn new(options: &mut Options, install: &trp_data::Install, bindings: &mut Bindings, size: [u32; 2]) -> Self {
        let mut graphics = Graphics { vsync: options.present == "vsync", field_of_view: options.field_of_view, mode: Mode::Windowed, resolution: size };
        let read = install.display_settings();
        let install_graphics = Graphics {
            vsync: read.vsync.unwrap_or(false),
            field_of_view: install.player_settings().field_of_view.unwrap_or(FOV[0]).clamp(FOV[0], FOV[1]),
            mode: read.mode.map_or(Mode::Borderless, Mode::of_game),
            resolution: read.resolution.unwrap_or(size),
        };
        // The mouse as the run begins with it: the install's, or what the
        // command line said; and what the install's own game has.
        let mut mouse = Mouse {
            sensitivity: options.sensitivity,
            aiming: options.aiming_sensitivity,
            optic: install.control_setting("OpticSensitivity").unwrap_or(1.0).clamp(1.0, 5.0),
            invert: options.invert,
        };
        let theirs = install.player_settings();
        let install_mouse = Mouse {
            sensitivity: theirs.mouse_sensitivity.unwrap_or(0.7).clamp(0.1, 5.0),
            aiming: theirs.aiming_sensitivity.unwrap_or(0.5).clamp(0.1, 5.0),
            optic: mouse.optic,
            invert: options.invert,
        };
        let file = (!options.unattended()).then(options_file);
        let mut apply_at_start = false;
        let mut features = Features::default();
        if let Some(path) = &file {
            let asked = graphics.field_of_view;
            let asked_mouse = mouse.clone();
            let [controls, display] = load(path, &mut bindings.controls, &mut graphics, &mut mouse, &mut features);
            if controls {
                bindings.changed();
            }
            if options.fov_given {
                graphics.field_of_view = asked;
            }
            if options.sensitivity_given[0] {
                mouse.sensitivity = asked_mouse.sensitivity;
            }
            if options.sensitivity_given[1] {
                mouse.aiming = asked_mouse.aiming;
            }
            (options.sensitivity, options.aiming_sensitivity, options.invert) = (mouse.sensitivity, mouse.aiming, mouse.invert);
            if controls || display {
                println!("options: {} read ({}{})", path.display(), if controls { "the keys" } else { "" }, if display { " the display" } else { "" });
            }
            options.field_of_view = graphics.field_of_view;
            apply_at_start = display;
        }
        // The game's own words for its bindings, for the ways of being
        // pressed and for what else its controls tab says.
        let mut wanted: Vec<&str> = bindings.controls.keys.iter().map(|group| group.name.as_str()).collect();
        wanted.extend(SAID.iter().map(|(name, _)| *name));
        let names = trp_data::inventory::texts(install, "en", &wanted).unwrap_or_else(|e| {
            eprintln!("the game's words for its bindings could not be read: {e:#}");
            HashMap::new()
        });
        // The command line has the last word on a switch, as on the rest.
        if let Some(on) = options.custom_hands {
            features.custom_hands = on;
        }
        if let Some(on) = options.smooth_animations {
            features.smooth_animations = on;
        }
        if let Some(on) = options.icon_generation {
            features.icon_generation = on;
        }
        let started = features.clone();
        Self { sub: 0, listening: None, asking: None, note: String::new(), scroll: 0.0, graphics, features, started, mouse, install_mouse, apply_at_start, install_graphics, dirty: true, file, names, sliding: None }
    }

    /// Whether Escape is the tab's own just now: a key is being chosen,
    /// which Escape drops, or a way of being pressed is asked for, which
    /// Escape leaves as it was. The inventory screen then does not shut
    /// for it.
    pub fn takes_escape(&self) -> bool {
        self.listening.is_some() || self.asking.is_some()
    }

    /// The game's own words for a line of its controls tab, in capitals,
    /// or the app's where the install has none.
    fn said(&self, name: &str) -> String {
        let ours = SAID.iter().find(|(known, _)| *known == name).map_or(name, |(_, ours)| *ours);
        self.names.get(&name.to_lowercase()).map_or_else(|| ours.to_owned(), |words| words.to_uppercase())
    }

    /// The game's word for a way of being pressed.
    fn press_word(&self, press: PressType) -> String {
        self.said(&format!("EPressType/{}", press.name()))
    }

    /// A bar's figure as it stands. The double click's time is the
    /// bindings' own.
    fn slider(&self, kind: Slide, controls: &Controls) -> f32 {
        match kind {
            Slide::Fov => self.graphics.field_of_view,
            Slide::Sensitivity => self.mouse.sensitivity,
            Slide::Aiming => self.mouse.aiming,
            Slide::DoubleClick => controls.double_click,
            Slide::Optic => self.mouse.optic,
        }
    }

    fn set_slider(&mut self, kind: Slide, value: f32, controls: &mut Controls) {
        match kind {
            Slide::Fov => self.graphics.field_of_view = value,
            Slide::Sensitivity => self.mouse.sensitivity = value,
            Slide::Aiming => self.mouse.aiming = value,
            Slide::DoubleClick => controls.double_click = value,
            Slide::Optic => self.mouse.optic = value,
        }
    }

    /// The game's words for a binding, in capitals, or its own name
    /// spaced out where the install has none.
    fn name_of(&self, name: &str) -> String {
        match self.names.get(&name.to_lowercase()) {
            Some(words) => words.to_uppercase(),
            None => {
                let mut spaced = String::new();
                for letter in name.chars() {
                    if letter.is_uppercase() && !spaced.is_empty() {
                        spaced.push(' ');
                    }
                    spaced.extend(letter.to_uppercase());
                }
                spaced
            }
        }
    }

    fn keep(&mut self, controls: &Controls) {
        if let Some(path) = &self.file {
            if let Err(e) = save(path, controls, &self.graphics, &self.mouse, &self.features) {
                self.note = format!("not kept: {} could not be written ({e})", path.display());
            }
        }
    }
}

/// The app's words for an axis's two ways.
const AXIS_WORDS: [(&str, [&str; 2]); 3] =
    [("MoveX", ["MOVE RIGHT", "MOVE LEFT"]), ("MoveY", ["MOVE FORWARD", "MOVE BACKWARD"]), ("LeanX", ["LEAN RIGHT SLOWLY", "LEAN LEFT SLOWLY"])];

/// What the game's controls tab says besides its bindings, each by the
/// name the install's texts have it under, with the app's words for an
/// install that has none.
const SAID: [(&str, &str); 10] = [
    ("EPressType/Release", "RELEASE"),
    ("EPressType/Continuous", "CONTINUOUS"),
    ("EPressType/Press", "PRESS"),
    ("EPressType/DoubleClick", "DOUBLE CLICK"),
    ("Mouse sensitivity", "MOUSE SENSITIVITY"),
    ("Mouse sensitivity (aiming)", "MOUSE SENSITIVITY (AIMING)"),
    ("Settings/DoubleClickTimeout", "DOUBLE CLICK TIMEOUT"),
    ("Settings/OpticSensitivity", "SCOPE ZOOM ADJUSTMENT SENSITIVITY"),
    ("Inverted X axis", "INVERTED X AXIS"),
    ("Inverted Y axis", "INVERTED Y AXIS"),
];

/// The systems a host might switch on or off, or feed from its own game:
/// a name, where it stands here, and what it is. **A list only** (the
/// user, 2026-10-12): nothing is switched by it yet. What is switched is
/// `Features`, shown over this list.
const FEATURES: [(&str, &str, &str); 22] = [
    ("MOVEMENT SYSTEM", "built", "Tarkov's walk, inertia, sprint, stance, lean, step aside, jump and crawl. Off, the host moves its own character and the rest follows it."),
    ("VAULTING AND CLIMBING", "built", "The jump key held takes the character over or onto what stands ahead."),
    ("STAMINA", "built", "The legs' and the arms' stamina, what drains and refills them, what running out does."),
    ("WEIGHT", "built", "Carried weight slows the character and adds inertia. Could be fed from the host's own inventory (a backpack's weight in STALKER)."),
    ("EXHAUSTION", "built", "Out of breath: the sway, no sprint and no jump on spent legs."),
    ("WEAPON MOTION", "built", "The springs, sway, breathing, the walk's bob, the lean's roll and the turn-away from walls."),
    ("RECOIL", "built", "The climb, the kick back and the camera's kick, by the weapon, its parts and the round."),
    ("WEAPON HANDLING", "built", "Firing, reloading, the checks, drawing and putting away, by the weapon's own animator."),
    ("AIMING", "in part", "The eye on the iron sights' line and the aimed field of view are built; optics are not."),
    ("FIRST-PERSON BODY", "built", "The legs and the arms drawn, the upper body as a shadow."),
    ("THIRD-PERSON BODY", "in part", "The character as others see it: its own pose, weapon and breath (F5, or the free camera). Built for the player's own; a second character is not."),
    ("INVENTORY SYSTEM", "built", "The equipment's slots, containers and their grids, moving things, the quick-use bar."),
    ("LOOTING SYSTEM", "in part", "Containers searched and things taken up are built; a search item by item is not."),
    ("WORLD INTERACTIONS", "built", "Doors, containers, a trunk, a kick, a lock and its key."),
    ("ADVANCED ATTACHMENT SYSTEM", "not built", "Parts put on a weapon and taken off it, and what each does to its handling. Weapons are their stock presets."),
    ("HEALTH", "not built", "The body's parts and what harms them."),
    ("ADVANCED HEALING SYSTEM", "not built", "Bleeding, fractures and their treatment, medicine used from the bar."),
    ("HUNGER AND THIRST", "a constant", "Both move stamina by a few percent. Taken as full; a host may set them (radiation likewise, from STALKER)."),
    ("SKILLS", "options", "Endurance, Strength, the weapon skills and mastering are options of a run, not a system that trains."),
    ("BALLISTICS AND DAMAGE", "not built", "Where a shot goes and what it does; malfunctions and overheating."),
    ("SOUND", "not built", "Steps, shots, the weapon's handling."),
    ("CO-OP", "not built", "Other players. Kept in mind, not begun."),
];

#[derive(Component)]
pub struct OptionsPage;
#[derive(Component)]
struct SubTab(usize);
#[derive(Component)]
struct KeyCell(Cell);
#[derive(Component)]
struct PressCell(usize);
#[derive(Component)]
struct PressChoice(PressType);
#[derive(Component)]
struct InstallKeys;
#[derive(Component)]
struct InstallDisplay;
#[derive(Component)]
struct VsyncChoice(bool);
#[derive(Component)]
struct ModeChoice(Mode);
#[derive(Component)]
struct ResolutionChoice([u32; 2]);
/// A bar that is slid, the part of it that is filled, and its figure.
#[derive(Component)]
struct SlideBar(Slide);
#[derive(Component)]
struct SlideFill(Slide);
#[derive(Component)]
struct SlideValue(Slide);
/// One of the two switches that turn an axis of the mouse about.
#[derive(Component)]
struct InvertChoice(usize);
/// A switch of `Features`.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum FeatureSwitch {
    CustomHands,
    SmoothAnimations,
    IconGeneration,
}

impl FeatureSwitch {
    const ALL: [Self; 3] = [Self::CustomHands, Self::SmoothAnimations, Self::IconGeneration];

    /// Its name on the tab, and what it does.
    fn words(self) -> (&'static str, &'static str) {
        match self {
            Self::CustomHands => (
                "CUSTOM FIRST PERSON HANDS",
                "An outfit that changes the body brings its own arms into first person. Off, the hands do not change with what is worn: they are Tarkov's default arms. Whatever an outfit has not got is Tarkov's default either way.",
            ),
            Self::SmoothAnimations => (
                "SMOOTH FIRST PERSON ANIMATIONS",
                "Every animation of what is in hand, weapons and empty hands, drawn on curves through its clips' samples, where Tarkov's engine joins them by straight lines (the stutter in a look over a weapon). Off, all are drawn as the engine has them, and each pose is worked out once a step instead of twice. Only the drawing changes.",
            ),
            Self::IconGeneration => (
                "ICON GENERATION",
                "The inventory's pictures drawn from each item's own model, as Tarkov draws them, once, and kept for later runs (the first run starts a few seconds later while it draws them). Off, none is drawn or read: each box shows the plain shape of its sort.",
            ),
        }
    }

    fn get(self, features: &Features) -> bool {
        match self {
            Self::CustomHands => features.custom_hands,
            Self::SmoothAnimations => features.smooth_animations,
            Self::IconGeneration => features.icon_generation,
        }
    }

    fn flip(self, features: &mut Features) {
        match self {
            Self::CustomHands => features.custom_hands = !features.custom_hands,
            Self::SmoothAnimations => features.smooth_animations = !features.smooth_animations,
            Self::IconGeneration => features.icon_generation = !features.icon_generation,
        }
    }
}
/// The button that ends the run (`QUIT`, the user's word for it).
#[derive(Component)]
struct ExitButton;
/// The scrolled list of bindings: how high all of it is, in pixels.
#[derive(Component)]
struct ListArea {
    whole: f32,
}

/// The height of a row of the list of bindings, in pixels: the game's 28
/// units on a screen 1080 high, at the size the app's words are.
const ROW: f32 = 26.0;
/// The second of the two shades the list's rows take by turns, a little
/// lighter than a cell's own.
const STRIPE: Color = Color::srgb(0.155, 0.165, 0.165);
/// The bindings that begin a group of their own in the list, which leaves
/// a little room before each: the app's grouping (the user, 2026-10-13),
/// over the game's own order, which is kept. The first group is the
/// axes' and what follows them down to the speed's two; then the weapon's
/// handling, the weapons taken in hand, what is worn and switched, the
/// other actions, the quick-use keys, and the screen's own.
const GROUP_STARTS: [&str; 6] = ["WeaponMounting", "PrimaryWeaponFirst", "ToggleGoggles", "Nidnod", "Slot4", "MakeScreenshot"];
const GROUP_GAP: f32 = 8.0;
/// The shares of the list's width the game's four columns have: what the
/// binding does, its two keys, and how it is pressed.
const COLUMNS: [f32; 4] = [40.0, 20.0, 20.0, 20.0];
const SUBS: [&str; 3] = ["CONTROLS", "GRAPHICS", "FEATURES"];

pub struct OptionsPlugin;

impl Plugin for OptionsPlugin {
    fn build(&self, app: &mut App) {
        // With the inventory's own systems: after the simulation's frames.
        app.add_systems(
            Update,
            (clicks, listen, slide, scroll_list, apply, rebuild)
                .chain()
                .run_if(crate::running)
                .after(crate::controller::step)
                // After the screen's own keys: Escape that the tab is about
                // to take is seen there as taken.
                .after(crate::inventory_view::ScreenKeys)
                .before(crate::animation::play_clips),
        );
    }
}

fn button(parent: &mut ChildSpawnerCommands, marker: impl Bundle, text: &str, lit: bool, width: Val) {
    let node = Node { width, padding: UiRect::axes(Val::Px(10.0), Val::Px(4.0)), justify_content: JustifyContent::Center, flex_shrink: 0.0, ..default() };
    parent.spawn((marker, Button, node, BackgroundColor(if lit { LINE } else { PANEL }))).with_children(|inside| {
        inside.spawn(words(text, 12.0, WORDS));
    });
}

/// A cell of the list of bindings, so many hundredths of its width wide.
fn cell(share: f32) -> Node {
    Node { width: Val::Percent(share), height: Val::Px(ROW - 3.0), padding: UiRect::horizontal(Val::Px(8.0)), align_items: AlignItems::Center, overflow: Overflow::clip(), ..default() }
}

/// A bar to slide with its figure beside it, as the game's own bar has
/// its figure to its right.
fn slider(row: &mut ChildSpawnerCommands, kind: Slide, value: f32, width: f32) {
    let [low, high] = kind.range();
    let share = ((value - low) / (high - low)).clamp(0.0, 1.0);
    row.spawn((SlideBar(kind), RelativeCursorPosition::default(), Node { width: Val::Px(width), height: Val::Px(16.0), flex_shrink: 0.0, ..default() }, BackgroundColor(LINE))).with_children(|bar| {
        bar.spawn((SlideFill(kind), Node { width: Val::Percent(share * 100.0), height: Val::Percent(100.0), ..default() }, BackgroundColor(WORDS)));
    });
    row.spawn(Node { width: Val::Px(52.0), flex_shrink: 0.0, ..default() }).with_children(|figure| {
        figure.spawn((SlideValue(kind), words(kind.shown(value), 13.0, WORDS)));
    });
}

/// The game's controls tab as its own screen lays it out
/// (`ControlSettingsTab`; settings spec, "The controls tab"): one large
/// box that scrolls, with the axes' two ways each and then every binding
/// the game's own screen shows, in four columns (what it does, its two
/// keys, how it is pressed); and under it the mouse's panel, a row of
/// four bars and a row of two switches.
fn controls_part(page: &mut ChildSpawnerCommands, state: &OptionsState, bindings: &Bindings) {
    let controls = &bindings.controls;
    page.spawn(Node { column_gap: Val::Px(12.0), align_items: AlignItems::Center, flex_shrink: 0.0, ..default() }).with_children(|top| {
        button(top, InstallKeys, "AS THE INSTALL HAS THEM", false, Val::Auto);
        let (said, color) = if state.note.is_empty() {
            ("Click a key's cell, hold the keys wanted and let one go (three at the most; a notch of the wheel may end it; Escape keeps what stood there). The dim commands do nothing here yet.".to_owned(), FAINT)
        } else {
            (state.note.clone(), WORDS)
        };
        top.spawn((Node { flex_shrink: 1.0, ..default() }, words(said, 12.0, color)));
    });
    let boxed = |grows: bool| {
        let node = Node {
            flex_direction: FlexDirection::Column,
            width: Val::Percent(100.0),
            padding: UiRect::all(Val::Px(6.0)),
            border: UiRect::all(Val::Px(1.0)),
            row_gap: Val::Px(3.0),
            flex_grow: if grows { 1.0 } else { 0.0 },
            flex_shrink: if grows { 1.0 } else { 0.0 },
            flex_basis: if grows { Val::Px(0.0) } else { Val::Auto },
            min_height: Val::Px(0.0),
            ..default()
        };
        (node, BackgroundColor(BACK), BorderColor(LINE))
    };
    let line = || Node { column_gap: Val::Px(3.0), height: Val::Px(ROW), width: Val::Percent(100.0), flex_shrink: 0.0, ..default() };
    let listened = state.listening.as_ref();
    // A row's cells are all of the row's shade, and the rows take the two
    // shades by turns: the eye keeps to a row across the list's width.
    let key_cell = |row: &mut ChildSpawnerCommands, at: Cell, source: &Source, shade: Color| {
        let text = match listened.filter(|listening| listening.cell == at && !listening.done) {
            Some(listening) if listening.heard.source.is_empty() => "PRESS ANY KEY...".to_owned(),
            Some(listening) => format!("{} + ...", keys::words(&listening.heard.source)),
            None if source.is_empty() => "NOT SET".to_owned(),
            None => keys::words(source),
        };
        let lit = listened.is_some_and(|listening| listening.cell == at);
        let color = if source.is_empty() && !lit { FAINT } else { WORDS };
        row.spawn((KeyCell(at), Button, cell(COLUMNS[1]), BackgroundColor(if lit { LINE } else { shade }))).with_children(|inside| {
            inside.spawn((words(text, 12.0, color), TextLayout::new_with_no_wrap()));
        });
    };
    let named = |row: &mut ChildSpawnerCommands, text: String, built: bool, shade: Color| {
        row.spawn((cell(COLUMNS[0]), BackgroundColor(shade))).with_children(|inside| {
            inside.spawn((words(text, 12.0, if built { WORDS } else { FAINT }), TextLayout::new_with_no_wrap()));
        });
    };
    let shown: Vec<usize> = controls.keys.iter().enumerate().filter(|(_, group)| !group.hidden()).map(|(index, _)| index).collect();
    // A little room before each binding that begins a group of its own.
    let begins = |index: usize| GROUP_STARTS.contains(&controls.keys[index].name.as_str());
    let rows = controls.axes.len() * 2 + shown.len();
    let whole = rows as f32 * ROW + shown.iter().filter(|index| begins(**index)).count() as f32 * GROUP_GAP;
    page.spawn(boxed(true)).with_children(|panel| {
        // The columns' heads stand over the list and do not scroll.
        panel.spawn(line()).with_children(|row| {
            for (share, text) in COLUMNS.into_iter().zip(["ACTION", "KEY", "KEY", "PRESS TYPE"]) {
                row.spawn(cell(share)).with_children(|inside| {
                    inside.spawn(words(text, 11.0, FAINT));
                });
            }
        });
        let area = Node { flex_direction: FlexDirection::Column, flex_grow: 1.0, flex_basis: Val::Px(0.0), min_height: Val::Px(0.0), width: Val::Percent(100.0), overflow: Overflow::scroll_y(), ..default() };
        panel.spawn((ListArea { whole }, area, ScrollPosition { offset_x: 0.0, offset_y: state.scroll.max(0.0) }, RelativeCursorPosition::default())).with_children(|list| {
            // How many rows the group in hand has so far: its first is of
            // the darker shade.
            let mut in_group = 0;
            let mut shade = || {
                in_group += 1;
                if in_group % 2 == 1 { PANEL } else { STRIPE }
            };
            for (index, axis) in controls.axes.iter().enumerate() {
                let ways = AXIS_WORDS.iter().find(|(name, _)| *name == axis.name).map_or(["+", "-"], |(_, ways)| *ways);
                for (way, word) in ways.into_iter().enumerate() {
                    let shade = shade();
                    list.spawn(line()).with_children(|row| {
                        named(row, word.to_owned(), keys::BUILT.contains(&axis.name.as_str()), shade);
                        key_cell(row, Cell::Axis(index, 0, way), &axis.pairs[0][way], shade);
                        key_cell(row, Cell::Axis(index, 1, way), &axis.pairs[1][way], shade);
                        row.spawn((cell(COLUMNS[3]), BackgroundColor(shade)));
                    });
                }
            }
            let mut in_group = 2 * controls.axes.len();
            for index in shown {
                let group = &controls.keys[index];
                if begins(index) {
                    list.spawn(Node { height: Val::Px(GROUP_GAP), flex_shrink: 0.0, ..default() });
                    in_group = 0;
                }
                in_group += 1;
                let shade = if in_group % 2 == 1 { PANEL } else { STRIPE };
                list.spawn(line()).with_children(|row| {
                    let built = keys::BUILT.contains(&group.name.as_str());
                    named(row, state.name_of(&group.name), built, shade);
                    key_cell(row, Cell::Key(index, 0), &group.variants[0], shade);
                    // How it is pressed: chosen from the four where the
                    // game lets it be chosen (its list of them, laid over
                    // the second key's cell while it is open), and not
                    // shown for a binding with no keys.
                    if state.asking == Some(index) {
                        let room = Node { width: Val::Percent(COLUMNS[2] + COLUMNS[3]), column_gap: Val::Px(3.0), justify_content: JustifyContent::FlexEnd, ..default() };
                        row.spawn(room).with_children(|choices| {
                            for press in PressType::ALL {
                                let node = Node { padding: UiRect::horizontal(Val::Px(7.0)), height: Val::Px(ROW - 3.0), align_items: AlignItems::Center, ..default() };
                                choices.spawn((PressChoice(press), Button, node, BackgroundColor(if press == group.press { FAINT } else { LINE }))).with_children(|inside| {
                                    inside.spawn((words(state.press_word(press), 11.0, WORDS), TextLayout::new_with_no_wrap()));
                                });
                            }
                        });
                        return;
                    }
                    key_cell(row, Cell::Key(index, 1), &group.variants[1], shade);
                    if group.is_empty() {
                        row.spawn((cell(COLUMNS[3]), BackgroundColor(shade)));
                    } else if group.press_shown() {
                        row.spawn((PressCell(index), Button, cell(COLUMNS[3]), BackgroundColor(shade))).with_children(|inside| {
                            inside.spawn(words(state.press_word(group.press), 12.0, WORDS));
                        });
                    } else {
                        row.spawn((cell(COLUMNS[3]), BackgroundColor(shade))).with_children(|inside| {
                            inside.spawn(words(state.press_word(group.press), 12.0, FAINT));
                        });
                    }
                });
            }
        });
    });
    // The mouse's panel: the four bars in a row, each under its name, and
    // the two switches under them.
    page.spawn(boxed(false)).with_children(|panel| {
        panel.spawn(Node { justify_content: JustifyContent::SpaceAround, width: Val::Percent(100.0), flex_wrap: FlexWrap::Wrap, row_gap: Val::Px(6.0), ..default() }).with_children(|bars| {
            for kind in [Slide::Sensitivity, Slide::Aiming, Slide::DoubleClick, Slide::Optic] {
                bars.spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(4.0), align_items: AlignItems::FlexStart, ..default() }).with_children(|bar| {
                    // Nothing here has a scope to zoom: its bar is kept dim.
                    bar.spawn(words(state.said(kind.caption().0), 12.0, if kind == Slide::Optic { FAINT } else { WORDS }));
                    bar.spawn(Node { column_gap: Val::Px(10.0), align_items: AlignItems::Center, ..default() }).with_children(|row| {
                        slider(row, kind, state.slider(kind, controls), 240.0);
                    });
                });
            }
        });
        panel.spawn(Node { justify_content: JustifyContent::Center, column_gap: Val::Px(48.0), width: Val::Percent(100.0), margin: UiRect::top(Val::Px(6.0)), ..default() }).with_children(|switches| {
            for (axis, name) in ["Inverted X axis", "Inverted Y axis"].into_iter().enumerate() {
                let on = state.mouse.invert[axis];
                let node = Node { column_gap: Val::Px(8.0), align_items: AlignItems::Center, padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)), ..default() };
                switches.spawn((InvertChoice(axis), Button, node, BackgroundColor(BACK))).with_children(|switch| {
                    let tick = Node { width: Val::Px(18.0), height: Val::Px(18.0), border: UiRect::all(Val::Px(1.0)), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() };
                    switch.spawn((tick, BorderColor(LINE), BackgroundColor(if on { FAINT } else { PANEL }))).with_children(|mark| {
                        mark.spawn(words(if on { "X" } else { "" }, 12.0, WORDS));
                    });
                    switch.spawn(words(state.said(name), 12.0, WORDS));
                });
            }
        });
    });
}

/// The display: the four of the game's settings the user named.
fn graphics_part(page: &mut ChildSpawnerCommands, state: &OptionsState, resolutions: &[[u32; 2]]) {
    let graphics = &state.graphics;
    let line = |page: &mut ChildSpawnerCommands, caption: &str, inside: &mut dyn FnMut(&mut ChildSpawnerCommands)| {
        page.spawn(Node { column_gap: Val::Px(6.0), align_items: AlignItems::Center, flex_wrap: FlexWrap::Wrap, row_gap: Val::Px(4.0), ..default() }).with_children(|row| {
            row.spawn(Node { width: Val::Px(200.0), flex_shrink: 0.0, ..default() }).with_children(|name| {
                name.spawn(words(caption, 12.0, FAINT));
            });
            inside(row);
        });
    };
    line(page, "VSYNC", &mut |row| {
        button(row, VsyncChoice(true), "ON", graphics.vsync, Val::Px(110.0));
        button(row, VsyncChoice(false), "OFF", !graphics.vsync, Val::Px(110.0));
    });
    line(page, "FIELD OF VIEW", &mut |row| {
        slider(row, Slide::Fov, graphics.field_of_view, 400.0);
        row.spawn(words("the game's setting, 50 to 75; aimed, 15 less", 11.0, FAINT));
    });
    line(page, "DISPLAY MODE", &mut |row| {
        for mode in Mode::ALL {
            button(row, ModeChoice(mode), mode.word(), graphics.mode == mode, Val::Px(130.0));
        }
    });
    line(page, "RESOLUTION", &mut |row| {
        for size in resolutions {
            button(row, ResolutionChoice(*size), &format!("{} x {}", size[0], size[1]), graphics.resolution == *size, Val::Px(130.0));
        }
    });
    page.spawn(words(
        "Each is put on the window as it is chosen. A borderless window is as large as the screen whatever the resolution says; the screen taken over is put into the nearest mode it has.",
        11.0,
        FAINT,
    ));
    let theirs = &state.install_graphics;
    page.spawn(Node { column_gap: Val::Px(12.0), align_items: AlignItems::Center, margin: UiRect::top(Val::Px(10.0)), ..default() }).with_children(|row| {
        button(row, InstallDisplay, "AS THE INSTALL HAS THEM", false, Val::Auto);
        let said = format!(
            "the install's own game: vsync {}, field of view {:.0}, {}, {} x {}",
            if theirs.vsync { "on" } else { "off" },
            theirs.field_of_view,
            theirs.mode.word().to_lowercase(),
            theirs.resolution[0],
            theirs.resolution[1]
        );
        row.spawn(words(said, 11.0, FAINT));
    });
    page.spawn(words(state.note.clone(), 12.0, WORDS));
}

fn features_part(page: &mut ChildSpawnerCommands, state: &OptionsState) {
    // What is switched: rows as the list's rows are, each with a switch
    // that is one.
    page.spawn(words("Switched here, each taking effect when the app is next started:", 12.0, WORDS));
    for switch in FeatureSwitch::ALL {
        let (name, what) = switch.words();
        page.spawn(Node { column_gap: Val::Px(10.0), align_items: AlignItems::Center, ..default() }).with_children(|row| {
            let on = switch.get(&state.features);
            let node = Node { width: Val::Px(46.0), padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)), border: UiRect::all(Val::Px(1.0)), justify_content: JustifyContent::Center, flex_shrink: 0.0, ..default() };
            row.spawn((switch, Button, node, BorderColor(if on { WORDS } else { LINE }), BackgroundColor(if on { LINE } else { PANEL }))).with_children(|inside| {
                inside.spawn(words(if on { "ON" } else { "OFF" }, 11.0, WORDS));
            });
            row.spawn(Node { width: Val::Px(290.0), flex_shrink: 0.0, ..default() }).with_children(|cell| {
                cell.spawn(words(name, 13.0, WORDS));
            });
            // Restart required: faint as a standing note; bright, and the
            // state the run holds said, once thrown away from it.
            let pending = on != switch.get(&state.started);
            row.spawn(Node { width: Val::Px(90.0), flex_shrink: 0.0, flex_direction: FlexDirection::Column, ..default() }).with_children(|cell| {
                cell.spawn(words("RESTART REQUIRED", 10.0, if pending { WORDS } else { FAINT }));
                if pending {
                    cell.spawn(words(if on { "off until then" } else { "on until then" }, 10.0, FAINT));
                }
            });
            row.spawn(words(what, 11.0, FAINT));
        });
    }
    page.spawn((
        Node { margin: UiRect::top(Val::Px(10.0)), ..default() },
        words(
            "Nothing below is switched yet. This is the list to be settled: the systems a host could switch off, or feed from its own game, while it runs.",
            12.0,
            WORDS,
        ),
    ));
    for (name, stands, what) in FEATURES {
        page.spawn(Node { column_gap: Val::Px(10.0), align_items: AlignItems::Center, ..default() }).with_children(|row| {
            // A switch as it will be, not yet one.
            row.spawn((Node { width: Val::Px(46.0), padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)), border: UiRect::all(Val::Px(1.0)), flex_shrink: 0.0, ..default() }, BorderColor(LINE))).with_children(|switch| {
                switch.spawn(words(if stands == "not built" { "-" } else { "ON" }, 11.0, FAINT));
            });
            row.spawn(Node { width: Val::Px(290.0), flex_shrink: 0.0, ..default() }).with_children(|cell| {
                cell.spawn(words(name, 13.0, WORDS));
            });
            row.spawn(Node { width: Val::Px(90.0), flex_shrink: 0.0, ..default() }).with_children(|cell| {
                cell.spawn(words(stands, 11.0, FAINT));
            });
            row.spawn(words(what, 11.0, FAINT));
        });
    }
}

/// The sizes the window may be given: those the screens have a mode for,
/// the largest first, with the size the app starts at and the one in
/// force among them.
fn resolutions(monitors: &Query<&Monitor>, graphics: &Graphics) -> Vec<[u32; 2]> {
    let mut sizes: Vec<[u32; 2]> = monitors.iter().flat_map(|monitor| monitor.video_modes.iter().map(|mode| [mode.physical_size.x, mode.physical_size.y])).collect();
    sizes.retain(|size| size[0] >= 1024 && size[1] >= 720);
    sizes.extend([crate::WINDOW_SIZE, graphics.resolution]);
    sizes.sort_by(|a, b| (b[0], b[1]).cmp(&(a[0], a[1])));
    sizes.dedup();
    sizes.truncate(14);
    if !sizes.contains(&graphics.resolution) {
        sizes.push(graphics.resolution);
    }
    sizes
}

/// Draws the tab afresh when something on it has changed.
fn rebuild(
    mut commands: Commands,
    mut state: ResMut<OptionsState>,
    controller: Res<Controller>,
    selected: Res<Selected>,
    bindings: Res<Bindings>,
    window: Query<&Window, With<PrimaryWindow>>,
    monitors: Query<&Monitor>,
    page: Query<Entity, With<OptionsPage>>,
    mut was: Local<(bool, Vec2)>,
) {
    let size = window.single().map_or(Vec2::ZERO, |window| Vec2::new(window.width(), window.height()));
    let shows = controller.inventory_open && selected.0 == OPTIONS;
    let changed = state.dirty || *was != (shows, size);
    *was = (shows, size);
    if !shows || !changed {
        return;
    }
    state.dirty = false;
    let state = &*state;
    let sizes = resolutions(&monitors, &state.graphics);
    for page in &page {
        commands.entity(page).despawn_related::<Children>().with_children(|page| {
            // The tab's parts to the left, and at the far right the
            // button that ends the run.
            page.spawn(Node { column_gap: Val::Px(6.0), width: Val::Percent(100.0), flex_shrink: 0.0, ..default() }).with_children(|row| {
                for (index, caption) in SUBS.into_iter().enumerate() {
                    button(row, SubTab(index), caption, index == state.sub, Val::Px(150.0));
                }
                row.spawn(Node { flex_grow: 1.0, ..default() });
                button(row, ExitButton, "QUIT", false, Val::Px(150.0));
            });
            match state.sub {
                0 => controls_part(page, state, &bindings),
                1 => graphics_part(page, state, &sizes),
                _ => features_part(page, state),
            }
        });
    }
}

/// What a click on the tab does.
fn clicks(
    mut state: ResMut<OptionsState>,
    mut bindings: ResMut<Bindings>,
    controller: Res<Controller>,
    keys: Res<ButtonInput<KeyCode>>,
    subs: Query<(&Interaction, &SubTab), Changed<Interaction>>,
    cells: Query<(&Interaction, &KeyCell), Changed<Interaction>>,
    press: Query<(&Interaction, &PressCell), Changed<Interaction>>,
    choices: Query<(&Interaction, &PressChoice), Changed<Interaction>>,
    install_keys: Query<&Interaction, (With<InstallKeys>, Changed<Interaction>)>,
    install_display: Query<&Interaction, (With<InstallDisplay>, Changed<Interaction>)>,
    (vsync, modes, sizes, custom_hands): (
        Query<(&Interaction, &VsyncChoice), Changed<Interaction>>,
        Query<(&Interaction, &ModeChoice), Changed<Interaction>>,
        Query<(&Interaction, &ResolutionChoice), Changed<Interaction>>,
        Query<(&Interaction, &FeatureSwitch), Changed<Interaction>>,
    ),
    inverts: Query<(&Interaction, &InvertChoice), Changed<Interaction>>,
    exit: Query<&Interaction, (With<ExitButton>, Changed<Interaction>)>,
    mut quit: EventWriter<AppExit>,
) {
    let pressed = |interaction: &Interaction| *interaction == Interaction::Pressed;
    // The screen shut: a key being chosen is dropped with it.
    if !controller.inventory_open {
        if state.listening.is_some() || state.asking.is_some() {
            (state.listening, state.asking, state.dirty) = (None, None, true);
            bindings.muted = false;
        }
        return;
    }
    if state.listening.is_some() {
        return;
    }
    let state = &mut *state;
    // How a binding is pressed, asked for: one of the four, or Escape,
    // which leaves it as it is (the game's list shut with nothing chosen).
    if let Some(group) = state.asking {
        let chosen = choices.iter().find(|(interaction, _)| pressed(interaction)).map(|(_, choice)| choice.0);
        let kept = keys.just_pressed(KeyCode::Escape).then(|| bindings.controls.keys[group].press);
        if let Some(press) = chosen.or(kept) {
            let before = bindings.controls.clone();
            bindings.controls.press_chosen(group, press);
            state.note = format!("{}: {}{}", state.name_of(&before.keys[group].name), state.press_word(bindings.controls.keys[group].press).to_lowercase(), lost(state, &before, &bindings.controls, group));
            bindings.changed();
            state.keep(&bindings.controls);
            (state.asking, state.dirty) = (None, true);
        }
        return;
    }
    // The run is ended: what was changed is already kept.
    if exit.iter().any(pressed) {
        quit.write(AppExit::Success);
        return;
    }
    if let Some((_, tab)) = subs.iter().find(|(interaction, _)| pressed(interaction)) {
        (state.sub, state.note, state.dirty) = (tab.0, String::new(), true);
        return;
    }
    // A switch of the features' list: thrown and kept for the next run.
    // What goes by it holds the switches as this run began (restart
    // required): nothing is handed on now.
    if let Some((_, switch)) = custom_hands.iter().find(|(interaction, _)| pressed(interaction)) {
        switch.flip(&mut state.features);
        state.note.clear();
        state.keep(&bindings.controls);
        state.dirty = true;
        return;
    }
    if let Some((_, choice)) = inverts.iter().find(|(interaction, _)| pressed(interaction)) {
        state.mouse.invert[choice.0] = !state.mouse.invert[choice.0];
        state.keep(&bindings.controls);
        state.dirty = true;
        return;
    }
    if let Some((_, cell)) = cells.iter().find(|(interaction, _)| pressed(interaction)) {
        state.listening = Some(Listen { cell: cell.0, heard: Listening::default(), armed: false, done: false });
        bindings.muted = true;
        state.dirty = true;
        return;
    }
    if let Some((_, cell)) = press.iter().find(|(interaction, _)| pressed(interaction)) {
        (state.asking, state.dirty) = (Some(cell.0), true);
        return;
    }
    if install_keys.iter().any(pressed) {
        bindings.controls = bindings.install.clone();
        bindings.changed();
        state.mouse = state.install_mouse.clone();
        state.note = "the keys and the mouse are the install's again".to_owned();
        state.keep(&bindings.controls);
        state.dirty = true;
        return;
    }
    let before = state.graphics.clone();
    if let Some((_, choice)) = vsync.iter().find(|(interaction, _)| pressed(interaction)) {
        state.graphics.vsync = choice.0;
    }
    if let Some((_, choice)) = modes.iter().find(|(interaction, _)| pressed(interaction)) {
        state.graphics.mode = choice.0;
    }
    if let Some((_, choice)) = sizes.iter().find(|(interaction, _)| pressed(interaction)) {
        state.graphics.resolution = choice.0;
    }
    if install_display.iter().any(pressed) {
        state.graphics = state.install_graphics.clone();
    }
    if state.graphics != before {
        state.note.clear();
        state.keep(&bindings.controls);
        state.dirty = true;
    }
}

/// Which other bindings lost keys to a change, in words.
fn lost(state: &OptionsState, before: &Controls, after: &Controls, but: usize) -> String {
    let keys = before.keys.iter().zip(&after.keys).enumerate().filter(|(at, (was, is))| *at != but && was.variants != is.variants).map(|(_, (was, _))| state.name_of(&was.name));
    let axes = before.axes.iter().zip(&after.axes).filter(|(was, is)| was.pairs != is.pairs).map(|(was, _)| was.name.to_uppercase());
    let names: Vec<String> = keys.chain(axes).collect();
    if names.is_empty() { String::new() } else { format!("   (taken from {})", names.join(", ")) }
}

/// A key being chosen (`CommandKeyPair.ListenForKey`): once the click
/// that began it is over, the keys held are gathered, and the first of
/// them let go, a third going down or a notch of the wheel is the choice;
/// Escape among them drops it. Nothing else hears a key meanwhile, nor
/// until every key is up again.
fn listen(
    mut state: ResMut<OptionsState>,
    mut bindings: ResMut<Bindings>,
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    scroll: Res<AccumulatedMouseScroll>,
) {
    let state = &mut *state;
    let Some(listening) = &mut state.listening else { return };
    let any_down = keys.get_pressed().next().is_some() || buttons.get_pressed().next().is_some();
    if !listening.armed {
        listening.armed = !any_down;
        return;
    }
    if listening.done {
        if !any_down {
            (state.listening, state.dirty) = (None, true);
            bindings.muted = false;
        }
        return;
    }
    let cell = listening.cell;
    // The wheel may end any binding's keys but the trigger's
    // (`ForbiddenForAxisWithDefaultKey`); an axis's keys are keys.
    let takes_wheel = matches!(cell, Cell::Key(group, _) if bindings.controls.keys[group].name != "Shoot");
    let wheel = if scroll.delta.y == 0.0 { 0 } else { scroll.delta.y.signum() as i32 };
    let had = listening.heard.source.keys.len();
    let chosen = listening.heard.frame(&bindings.valid, &|code| bindings.held(code, &keys, &buttons), wheel, takes_wheel);
    match chosen {
        Chosen::Going => state.dirty |= listening.heard.source.keys.len() != had,
        Chosen::Dropped => {
            listening.done = true;
            state.note = "kept as it was".to_owned();
            state.dirty = true;
        }
        Chosen::Done(source) => {
            listening.done = true;
            let before = bindings.controls.clone();
            let said = keys::words(&source);
            match cell {
                Cell::Key(group, variant) => {
                    let asks = bindings.controls.key_chosen(group, variant, source);
                    let name = state.name_of(&before.keys[group].name);
                    if asks {
                        state.asking = Some(group);
                        state.note = format!("{name}: {said}. Another binding has these keys pressed the same way, or this one had none: choose how it is pressed.");
                    } else {
                        state.note = format!("{name}: {said}{}", lost(state, &before, &bindings.controls, group));
                    }
                }
                Cell::Axis(axis, pair, way) => {
                    bindings.controls.axis_chosen(axis, pair, way, source);
                    let word = AXIS_WORDS.iter().find(|(name, _)| *name == before.axes[axis].name).map_or("", |(_, ways)| ways[way]);
                    state.note = format!("{word}: {said}{}", lost(state, &before, &bindings.controls, usize::MAX));
                }
            }
            bindings.changed();
            state.keep(&bindings.controls);
            state.dirty = true;
        }
    }
}

/// A bar held and slid: its figure follows the mouse along it, and what
/// is drawn of it is changed where it stands. It is kept once the bar is
/// let go; the double click's time is then put to the bindings, whose
/// combinations are made afresh as the game makes them when it is saved.
fn slide(
    mut state: ResMut<OptionsState>,
    mut bindings: ResMut<Bindings>,
    buttons: Res<ButtonInput<MouseButton>>,
    bars: Query<(&SlideBar, &RelativeCursorPosition)>,
    mut fills: Query<(&SlideFill, &mut Node)>,
    mut figures: Query<(&SlideValue, &mut Text)>,
) {
    if state.sliding.is_none() && !buttons.just_pressed(MouseButton::Left) {
        return;
    }
    let state = &mut *state;
    if buttons.just_pressed(MouseButton::Left) && state.listening.is_none() {
        state.sliding = bars.iter().find(|(_, over)| over.mouse_over()).map(|(bar, _)| bar.0);
    }
    let Some(kind) = state.sliding else { return };
    if !buttons.pressed(MouseButton::Left) {
        state.sliding = None;
        if kind == Slide::DoubleClick {
            bindings.changed();
        }
        state.keep(&bindings.controls);
        return;
    }
    let Some(at) = bars.iter().find(|(bar, _)| bar.0 == kind).and_then(|(_, over)| over.normalized) else { return };
    let ([low, high], step) = (kind.range(), kind.step());
    let value = (((low + at.x.clamp(0.0, 1.0) * (high - low)) / step).round() * step).clamp(low, high);
    if value == state.slider(kind, &bindings.controls) {
        return;
    }
    // The bindings are not marked as changed for a bar that is not theirs.
    if kind == Slide::DoubleClick {
        bindings.controls.double_click = value;
    } else {
        state.set_slider(kind, value, &mut bindings.bypass_change_detection().controls);
    }
    for (_, mut node) in fills.iter_mut().filter(|(fill, _)| fill.0 == kind) {
        node.width = Val::Percent((value - low) / (high - low) * 100.0);
    }
    for (_, mut text) in figures.iter_mut().filter(|(figure, _)| figure.0 == kind) {
        text.0 = kind.shown(value);
    }
}

/// The list of bindings is scrolled by the wheel, three rows a notch,
/// unless the wheel is being listened to.
fn scroll_list(
    mut state: ResMut<OptionsState>,
    options: Res<Options>,
    wheel: Res<AccumulatedMouseScroll>,
    mut area: Query<(&ListArea, &mut ScrollPosition, &RelativeCursorPosition, &ComputedNode)>,
    mut frame: Local<u32>,
) {
    // A run nobody is at the keys of turns the wheel by its own count of
    // drawn frames (`--wheel`), as the inventory's does.
    *frame += 1;
    let scripted: i32 = options.wheel.iter().filter(|(at, _)| options.unattended() && *frame == *at).map(|(_, notches)| *notches).sum();
    let Ok((sizes, mut position, over, laid)) = area.single_mut() else { return };
    // How high the part of the list shown is, as the screen's layout has
    // it; nothing before it has been laid out.
    let shown = laid.size().y * laid.inverse_scale_factor();
    if shown <= 0.0 {
        return;
    }
    let mut to = state.scroll - scripted as f32 * ROW * 3.0;
    if state.listening.is_none() && over.mouse_over() && wheel.delta.y != 0.0 {
        to -= wheel.delta.y.signum() * ROW * 3.0;
    }
    let to = to.clamp(0.0, (sizes.whole - shown).max(0.0));
    if to != state.scroll {
        state.bypass_change_detection().scroll = to;
    }
    if position.offset_y != to {
        position.offset_y = to;
    }
}

/// Puts the display's settings on the window and the view when they have
/// changed. Each is the game's setting of that name; the game puts them
/// on when its settings screen is saved, the app as each is chosen.
fn apply(
    state: Res<OptionsState>,
    mut options: ResMut<Options>,
    mut window: Query<&mut Window, With<PrimaryWindow>>,
    monitors: Query<&Monitor>,
    hands: Option<ResMut<HandsRig>>,
    stowed: Option<ResMut<Stowed>>,
    mut applied: Local<Option<Graphics>>,
) {
    // The mouse's settings are what the view is turned by from now on.
    let mouse = &state.mouse;
    if (options.sensitivity, options.aiming_sensitivity, options.invert) != (mouse.sensitivity, mouse.aiming, mouse.invert) {
        (options.sensitivity, options.aiming_sensitivity, options.invert) = (mouse.sensitivity, mouse.aiming, mouse.invert);
    }
    if applied.as_ref() == Some(&state.graphics) {
        return;
    }
    let now = state.graphics.clone();
    // As the run begins there is nothing to do, unless the app's file
    // said how the display is to be.
    let was = applied.replace(now.clone());
    let Some(was) = was.or_else(|| state.apply_at_start.then(|| Graphics { vsync: !now.vsync, field_of_view: f32::NAN, mode: Mode::Windowed, resolution: [0, 0] })) else { return };
    let Ok(mut window) = window.single_mut() else { return };
    if now.vsync != was.vsync {
        window.present_mode = if now.vsync { PresentMode::AutoVsync } else { PresentMode::AutoNoVsync };
    }
    if now.field_of_view != was.field_of_view {
        options.field_of_view = now.field_of_view;
        if let Some(mut hands) = hands {
            hands.hands.set_field_of_view(now.field_of_view);
        }
        if let Some(mut stowed) = stowed {
            stowed.set_field_of_view(now.field_of_view);
        }
    }
    if now.mode != was.mode || now.resolution != was.resolution {
        let [width, height] = now.resolution;
        match now.mode {
            Mode::Windowed => {
                window.mode = WindowMode::Windowed;
                window.resolution.set_physical_resolution(width, height);
            }
            Mode::Borderless => window.mode = WindowMode::BorderlessFullscreen(MonitorSelection::Current),
            Mode::Fullscreen => {
                // The screen's mode of that size with the most frames a
                // second, or the mode it is in if it has none.
                let modes = monitors.iter().flat_map(|monitor| monitor.video_modes.iter().copied());
                let wanted = modes.filter(|mode| mode.physical_size == UVec2::new(width, height)).max_by_key(|mode| (mode.refresh_rate_millihertz, mode.bit_depth));
                window.mode = WindowMode::Fullscreen(MonitorSelection::Current, wanted.map_or(VideoModeSelection::Current, VideoModeSelection::Specific));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use trp_sim::bindings::{AxisGroup, KeyGroup};

    fn source(keys: &[&str], wheel: Option<bool>) -> Source {
        Source { keys: keys.iter().map(|key| code(key).unwrap()).collect(), wheel }
    }

    #[test]
    fn what_is_changed_is_kept_and_read_back_over_the_install_s() {
        let group = |name: &str, press: PressType, keys: &[&str]| KeyGroup { name: name.to_owned(), variants: [source(keys, None), Source::default()], press };
        let install = Controls {
            keys: vec![group("ReloadWeapon", PressType::Release, &["R"]), group("Prone", PressType::Press, &["X"]), group("NextWalkPose", PressType::Press, &["C"])],
            axes: vec![AxisGroup { name: "MoveX".into(), pairs: [[source(&["D"], None), source(&["A"], None)], Default::default()] }],
            double_click: 0.3,
        };
        let mut changed = install.clone();
        changed.keys[0] = group("ReloadWeapon", PressType::Press, &["T", "LeftAlt"]);
        changed.keys[2].variants[1] = source(&["Z"], Some(false));
        changed.axes[0].pairs[1] = [source(&["RightArrow"], None), source(&["LeftArrow"], None)];
        changed.double_click = 0.45;
        let graphics = Graphics { vsync: false, field_of_view: 68.0, mode: Mode::Borderless, resolution: [2560, 1440] };
        let mouse = Mouse { sensitivity: 0.42, aiming: 0.31, optic: 2.5, invert: [false, true] };
        let path = std::env::temp_dir().join(format!("trp-options-test-{}.json", std::process::id()));
        save(&path, &changed, &graphics, &mouse, &Features { custom_hands: false, smooth_animations: false, icon_generation: false }).unwrap();
        // Read over an install that has since gained a binding: the new
        // one stays, the rest are as kept.
        let mut read = install.clone();
        read.keys.push(group("Jump", PressType::Release, &["Space"]));
        let mut display = Graphics { vsync: true, field_of_view: 50.0, mode: Mode::Windowed, resolution: [1600, 900] };
        let mut pointer = Mouse { sensitivity: 0.7, aiming: 0.5, optic: 1.0, invert: [false; 2] };
        // A switch of the features' list is on until a file says it is not.
        let mut switched = Features::default();
        assert!(switched.custom_hands);
        assert_eq!(load(&path, &mut read, &mut display, &mut pointer, &mut switched), [true, true]);
        assert!(!switched.custom_hands && !switched.smooth_animations && !switched.icon_generation);
        // A file kept before there was such a switch leaves it as it is.
        let older = path.with_extension("older.json");
        std::fs::write(&older, r#"{ "graphics": { "vsync": true } }"#).unwrap();
        let mut untold = Features::default();
        assert_eq!(load(&older, &mut read.clone(), &mut display.clone(), &mut pointer.clone(), &mut untold), [false, true]);
        assert!(untold.custom_hands && untold.smooth_animations && untold.icon_generation);
        let _ = std::fs::remove_file(&older);
        let _ = std::fs::remove_file(&path);
        assert_eq!(read.keys[..3], changed.keys[..]);
        assert_eq!(read.keys[3], group("Jump", PressType::Release, &["Space"]));
        assert_eq!(read.axes, changed.axes);
        assert_eq!(read.double_click, 0.45);
        assert_eq!(display, graphics);
        assert_eq!(pointer, mouse);
        // No file: nothing read, nothing changed.
        assert_eq!(load(&path, &mut read, &mut display, &mut pointer, &mut switched), [false, false]);
        assert_eq!(pointer, mouse);
        assert!(!switched.custom_hands);
    }

    #[test]
    fn a_bar_s_figure_is_written_as_the_game_writes_it() {
        assert_eq!(Slide::Sensitivity.shown(0.7), "0.700");
        assert_eq!(Slide::DoubleClick.shown(0.3), "0.30");
        assert_eq!(Slide::Fov.shown(74.0), "74");
        for kind in [Slide::Fov, Slide::Sensitivity, Slide::Aiming, Slide::DoubleClick, Slide::Optic] {
            let [low, high] = kind.range();
            assert!(low < high && kind.step() > 0.0, "{kind:?}");
        }
    }
}
