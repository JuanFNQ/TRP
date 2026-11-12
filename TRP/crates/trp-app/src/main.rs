//! TRP, the Tushonka Runtime Project: the game's first-person controller rebuilt on Bevy, and its range.
//!
//! TRP, the Tushonka Runtime Project: the first-person controller of the
//! game Single Player Tushonka runs on, rebuilt on Bevy.
//!
//! All game data is read at startup from the player's own install.

// ==== What the app prints ====

// Every line the app prints goes to its console and to the run's log (`run_log`, through
// `trp_data::console`, as the libraries' do). Before the modules, so that they print with these.
macro_rules! println {
    () => {
        ::trp_data::console::line(false, ::std::format_args!(""))
    };
    ($($arg:tt)*) => {
        ::trp_data::console::line(false, ::std::format_args!($($arg)*))
    };
}
macro_rules! eprintln {
    () => {
        ::trp_data::console::line(true, ::std::format_args!(""))
    };
    ($($arg:tt)*) => {
        ::trp_data::console::line(true, ::std::format_args!($($arg)*))
    };
}

// ==== The modules ====

mod panels;
mod screens;
mod player;
mod assets;
mod platform;
mod effects;
mod audio;
mod world;
mod weapons;
mod rendering;
mod ui;


use std::path::PathBuf;

use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot, ScreenshotCaptured};
use bevy::window::PrimaryWindow;
use trp_data::setup::engine::EngineSettings;
use trp_data::setup::hard_settings::HardSettings;
use trp_data::character::player::HandsData;
use trp_data::unity::Assets as UnityAssets;
use trp_data::{Database, Install};

use player::animation::ClipPlayer;
use player::controller::Controller;
use weapons::weapon::WeaponRig;

mod startup_kit;
mod startup_crates;
mod app_options;
mod cli;
mod app_assembly;
mod warmup_log;
mod world_setup;
mod views_capture;

pub use startup_crates::*;
pub use app_options::*;
pub use warmup_log::*;
pub use world_setup::*;
use startup_kit::{BEAR_DOGTAG, CRATE, CRATE_CELLS, Crates, DEFAULT_WEAPON, GUN_CASE, MAGAZINE_BOX, OWN_SLOTS, armed, crate_of, dummy_kit, dummy_look, dummy_voice, logic_built, preset_build, table_loads};
use startup_crates::{CONTAINERS, DEFAULT_ARMS, DEFAULT_BODY, DEFAULT_HEAD, DEFAULT_LEGS, LYING, MORE_CONTAINERS, cabinet_of, fill_crates, gear_cabinets, repair_boxes, stow_more, unfinished_items, weapon_crates};
use app_options::{Watching};
use cli::{parse_options};
use app_assembly::{run};
use warmup_log::{FrameMark, WarmupProp, count_waiting, log_frame, log_pacing, mark_frame, warm_up, write_tape};
use world_setup::{setup_world, show_things};
use views_capture::{capture, fly_camera, switch_view};


// ==== Parsing the command line ====

/// DirectX's newer shader compiler, DXC, as its two libraries: beside
/// `trp.exe`, or in the newest Windows 10 or 11 SDK that has both for this
/// machine's kind (`Windows Kits\10\bin\<version>\x64`). The engine looks
/// for them in the folder it is run from alone, and without them compiles
/// with the older FXC: as measured, DXC built what a run draws
/// with in 0.43 to 0.61 s where FXC took 1.59 to 2.04, and the rifles
/// crate's pictures in 4.0 s where FXC took 7.2; a frame once all is built
/// takes the same.
fn dxc_libraries() -> Option<(std::path::PathBuf, std::path::PathBuf)> {
    if !cfg!(windows) {
        return None;
    }
    let both = |folder: &std::path::Path| {
        let (dxc, dxil) = (folder.join("dxcompiler.dll"), folder.join("dxil.dll"));
        (dxc.is_file() && dxil.is_file()).then_some((dxc, dxil))
    };
    let beside = std::env::current_exe().ok().and_then(|exe| exe.parent().and_then(both));
    beside.or_else(|| {
        let kits = std::path::PathBuf::from(std::env::var_os("ProgramFiles(x86)")?).join("Windows Kits").join("10").join("bin");
        let arch = if cfg!(target_arch = "aarch64") { "arm64" } else { "x64" };
        let mut versions: Vec<std::path::PathBuf> = std::fs::read_dir(&kits).ok()?.flatten().map(|entry| entry.path()).filter(|path| path.is_dir()).collect();
        // The newest first: the folders are named by version, `10.0.n.0`.
        versions.sort_by_key(|path| path.file_name().and_then(|name| name.to_str()).map(|name| name.split('.').map(|part| part.parse::<u32>().unwrap_or(0)).collect::<Vec<_>>()));
        versions.iter().rev().find_map(|version| both(&version.join(arch)))
    })
}

/// The game's own defaults for the four settings of its menu the app
/// takes: the field of view, head bobbing, and the mouse's sensitivity
/// from the hip and with the sights up (the defaults written in the
/// game's settings classes).
const DEFAULT_VIEW: [f32; 4] = [50.0, 0.2, 0.7, 0.5];

/// A part by the name `--damage` and `--poison` take it by.
fn part_named(name: &str) -> Option<trp_sim::condition::health::Part> {
    use trp_sim::condition::health::Part;
    Some(match name {
        "head" => Part::Head,
        "chest" | "thorax" => Part::Chest,
        "stomach" => Part::Stomach,
        "left-arm" => Part::LeftArm,
        "right-arm" => Part::RightArm,
        "left-leg" => Part::LeftLeg,
        "right-leg" => Part::RightLeg,
        _ => return None,
    })
}

/// `settings` is what the player has set in the install's own game: the
/// app starts with those, since they are what the game beside it shows,
/// and with the game's defaults where the install does not say.
/// `--seed`, in an unattended run: set once, as the options are read.
static FIXED_SEED: std::sync::OnceLock<Option<u64>> = std::sync::OnceLock::new();

/// The seed of a roll that a played run takes from the clock, anew each run
/// (the dummies' looks and health, the bullets', the holes' and the
/// explosions' random numbers): in an unattended run (a capture, a scripted
/// replay) `--seed` with `salt` instead, so that the run shows the same
/// each time it is made. None in a played run.
pub fn fixed_seed(salt: u64) -> Option<u64> {
    FIXED_SEED.get().copied().flatten().map(|seed| seed ^ salt)
}

// ==== main: the app assembled ====

fn main() -> anyhow::Result<()> {
    // The run's end, or what stopped its start, into its log (`run_log`).
    let result = run();
    match &result {
        Ok(()) => platform::run_log::ended(),
        Err(e) => platform::run_log::failed(e),
    }
    result
}
