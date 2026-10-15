//! TRP, the Tushonka Runtime Project: the first-person controller of the
//! game Single Player Tushonka runs on, rebuilt on Bevy.
//!
//! All game data is read at startup from the user's own install.

mod animation;
mod arms;
mod body_view;
mod controller;
mod convert;
mod cursor;
mod figure_view;
mod grid;
mod hands_view;
mod hud;
mod icons;
mod setup;
mod inventory_view;
mod keys;
mod options_view;
mod range;
mod render_data;
mod render_timing;
mod weapon;

use std::path::PathBuf;

use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use bevy::window::PrimaryWindow;
use trp_data::engine::EngineSettings;
use trp_data::hard_settings::HardSettings;
use trp_data::player::HandsData;
use trp_data::unity::Assets as UnityAssets;
use trp_data::{Database, Install};

use animation::ClipPlayer;
use controller::Controller;
use weapon::WeaponRig;

/// Kalashnikov AKM 7.62x39.
const DEFAULT_WEAPON: &str = "59d6088586f774275f37482f";
/// The crate on the range: the install's airdrop crate, a loot container
/// with one grid of ten by twelve cells, which is the smallest of the
/// install's that holds a rifle, a vest and a backpack together (its
/// weapon boxes are five by five at most).
const CRATE: &str = "6223349b3136504a544d1608";
/// How many cells the crate has here: the install's ten across, and
/// thirty down where the install's has twelve. **The app's, by the user's
/// word** (2026-10-12: larger inside, with a scroll bar, to hold more).
const CRATE_CELLS: [u32; 2] = [10, 30];
/// How many full stacks of its own rounds lie in the crate for each
/// weapon the kit is not made for, and how many full magazines for each
/// such weapon fed from magazines. The app's choice.
const SPARE_STACKS: usize = 2;
const SPARE_MAGAZINES: usize = 3;

/// For a weapon whose magazine comes out to be changed
/// (`ReloadMode` `ExternalMagazine`): the kind of magazine its stock
/// preset has, how many rounds that holds, and the round the weapon
/// names as its own (`defAmmo`).
fn fed_from_magazines(db: &trp_data::Database, weapon: &str) -> Option<(std::sync::Arc<trp_sim::inventory::Kind>, u32, std::sync::Arc<trp_sim::inventory::Kind>)> {
    let item = db.items.get(weapon)?;
    if item.props.get("ReloadMode")?.as_str()? != "ExternalMagazine" {
        return None;
    }
    let preset = db.default_preset(weapon).ok()?;
    let magazine = &preset.items.iter().find(|part| part.slot.as_deref() == Some(trp_sim::inventory::MAGAZINE_SLOT))?.template;
    let capacity = db.items.get(magazine)?.props.get("Cartridges")?.get(0)?.get("_max_count")?.as_u64()? as u32;
    let round = item.props.get("defAmmo")?.as_str()?;
    let kind = |template: &str| trp_sim::inventory::Kind::of(db, template).ok().map(std::sync::Arc::new);
    Some((kind(magazine)?, capacity, kind(round)?))
}

/// A weapon as its stock preset has it, with what loads it: fed from
/// magazines, its own full (as the hands have it when it is drawn) and,
/// with `spares`, `SPARE_MAGAZINES` more of the kind, full of its own
/// round, which its reloads take (firing spec, "The quick reload");
/// otherwise, with `spares`, `SPARE_STACKS` full stacks of that round,
/// which a tube, a cylinder or barrels are filled from. The weapon first.
fn armed(db: &trp_data::Database, weapon: &str, spares: bool) -> anyhow::Result<Vec<trp_sim::inventory::Item>> {
    use trp_sim::inventory::{fill_magazine, Item, Kind, MAGAZINE_SLOT};
    let (mut item, _) = trp_data::inventory::Loadout::of_preset(db, weapon).and_then(|list| Item::of_list(db, &list))?;
    let mut made = Vec::new();
    match fed_from_magazines(db, weapon) {
        Some((magazine, capacity, round)) => {
            if let Some(fitted) = item.slot_index(MAGAZINE_SLOT).and_then(|slot| item.slots[slot].as_mut()) {
                fill_magazine(fitted, round.clone(), capacity);
            }
            for _ in (0..SPARE_MAGAZINES).filter(|_| spares) {
                let mut spare = Item::new(magazine.clone());
                fill_magazine(&mut spare, round.clone(), capacity);
                made.push(spare);
            }
        }
        // A rocket launcher's controller has no state to reload in: it
        // has its rocket and no more (more-weapons spec).
        None if spares && !db.items.get(weapon).is_some_and(|item| db.is_a(item, "RocketLauncher")) => {
            let round = db.items.get(weapon).and_then(|item| item.props.get("defAmmo")?.as_str()).and_then(|round| db.items.get(round));
            if let Some(round) = round {
                let kind = std::sync::Arc::new(Kind::of(db, &round.id)?);
                for _ in 0..SPARE_STACKS {
                    let mut stack = Item::new(kind.clone());
                    stack.count = round.stack_max();
                    made.push(stack);
                }
            }
        }
        None => {}
    }
    made.insert(0, item);
    Ok(made)
}

/// Whether the simulation has the logic a weapon runs on
/// (`docs/spec/more-weapons.md`): every kind of the install's but a weapon
/// fired once and thrown away (`IsOneoff`, whose firing is not read; no
/// weapon with a stock preset in the install read is one).
fn logic_built(db: &trp_data::Database, weapon: &str) -> bool {
    let Some(item) = db.items.get(weapon) else { return false };
    !item.props.get("IsOneoff").and_then(serde_json::Value::as_bool).unwrap_or(false)
}

/// Which crate of weapons a weapon goes into (`range::WEAPON_CRATES`): a
/// weapon of one's own is custom; any other goes by the class its template
/// gives it (`weapClass`, which the weapon skills go by too; the item's
/// place among the database's classes would have the M32A1 a revolver).
fn crate_of(db: &trp_data::Database, weapon: &str) -> Option<usize> {
    if db.custom.contains_key(weapon) {
        return Some(7);
    }
    match db.items.get(weapon)?.props.get("weapClass")?.as_str()? {
        "assaultRifle" | "assaultCarbine" => Some(0),
        "smg" => Some(1),
        "machinegun" => Some(2),
        "sniperRifle" | "marksmanRifle" => Some(3),
        "shotgun" => Some(4),
        "pistol" => Some(5),
        "grenadeLauncher" | "specialWeapon" => Some(6),
        _ => None,
    }
}

/// The game's own weapon box and lootable ammunition box (`WeaponCrate`, a
/// grid of 5 by 2; `AmmoBox`, 3 by 3), which the tables of weapons' gun
/// cases and magazine boxes are.
const GUN_CASE: &str = "578f87ad245977356274f2cc";
const MAGAZINE_BOX: &str = "5909d45286f77465a8136dc6";

/// What each table of weapons holds, in `range::WEAPON_CRATES`' order: in
/// its gun case every weapon of the install of its class that has a stock
/// preset and runs on a logic built, and every weapon of one's own, by
/// name; in its magazine box what loads them (`armed`'s spares). Each is
/// its game's container, ten cells across as the range's crates are, and
/// as deep as what it holds (the app's: the game's own hold one weapon,
/// and nine things).
fn weapon_crates(db: &trp_data::Database) -> anyhow::Result<(Vec<trp_sim::inventory::Item>, Vec<trp_sim::inventory::Item>)> {
    use trp_sim::inventory::{Item, Kind};
    /// Deeper than any needs, before each is cut to what it holds.
    const DEEPEST: u32 = 2000;
    let deep = |template: &str| -> anyhow::Result<Kind> {
        let mut kind = Kind::of(db, template)?;
        kind.grids.first_mut().ok_or_else(|| anyhow::anyhow!("{template} has no grid"))?.cells = [CRATE_CELLS[0], DEEPEST];
        Ok(kind)
    };
    let (case_kind, box_kind) = (deep(GUN_CASE)?, deep(MAGAZINE_BOX)?);
    let (case_deep, box_deep) = (std::sync::Arc::new(case_kind.clone()), std::sync::Arc::new(box_kind.clone()));
    let mut cases: Vec<Item> = range::WEAPON_CRATES.iter().map(|_| Item::new(case_deep.clone())).collect();
    let mut boxes: Vec<Item> = range::WEAPON_CRATES.iter().map(|_| Item::new(box_deep.clone())).collect();
    let mut weapons: Vec<(&String, &String)> = db
        .items
        .values()
        .filter(|item| (item.name.starts_with("weapon_") && db.is_a(item, "Weapon")) || db.custom.contains_key(&item.id))
        .filter(|item| logic_built(db, &item.id) && db.default_preset(&item.id).is_ok())
        .map(|item| (&item.name, &item.id))
        .collect();
    weapons.sort();
    let mut counts = [0; range::WEAPON_CRATES.len()];
    for (name, weapon) in weapons {
        let Some(at) = crate_of(db, weapon) else {
            eprintln!("{name} is of no class a crate of weapons has");
            continue;
        };
        match armed(db, weapon, true) {
            Ok(made) => {
                // The weapon first, then what loads it.
                for (index, thing) in made.into_iter().enumerate() {
                    let (into, what) = if index == 0 { (&mut cases[at], "weapon case") } else { (&mut boxes[at], "magazine box") };
                    if into.stow(thing).is_err() {
                        eprintln!("the {} {what} has no room for {name}'s", range::WEAPON_CRATES[at]);
                    }
                }
                counts[at] += 1;
            }
            Err(e) => eprintln!("the tables of weapons have no {name}: {e:#}"),
        }
    }
    // Each cut to the rows it fills.
    let cut = |item: &mut Item, kind: &Kind| {
        let rows = item.grids[0].iter().map(|(place, thing)| place.y + if place.turned { thing.cells()[0] } else { thing.cells()[1] }).max().unwrap_or(0);
        let mut cut = kind.clone();
        cut.grids[0].cells = [CRATE_CELLS[0], rows.max(1)];
        item.kind = std::sync::Arc::new(cut);
        item.kind.grids[0].cells[1]
    };
    for (((case, held), label), count) in cases.iter_mut().zip(boxes.iter_mut()).zip(range::WEAPON_CRATES).zip(counts) {
        let (case_rows, box_rows) = (cut(case, &case_kind), cut(held, &box_kind));
        println!("the table of {label}: {count} weapons in its case ({case_rows} rows), what loads them in its box ({box_rows} rows)");
    }
    Ok((cases, boxes))
}
/// The key of the range's locked door: a key of the install's (the first
/// dormitory's marked room, by its name there). Any key would do; a door
/// is opened by the key whose item it names (`KeyTemplate.KeyId` is the
/// item's own id).
pub const DOOR_KEY: &str = "5780cf7f2459777de4559322";
/// The range's other containers, the install's items of the sorts recorded
/// by hand on 2026-10-12, in the order the range keeps them
/// (`range::Placed::holds`, after the airdrop crate): a wooden crate, a
/// technical supply crate, a dead Scav and a drawer. They start empty.
const CONTAINERS: [&str; 4] = ["578f87ad245977356274f2cc", "5d6fd45b86f774317075ed43", "5909e4b686f7747f5b744fa4", "578f87b7245977356274f2cd"];
/// Things lying about beyond the doors, to be taken up from a table, the
/// floor and a shelf (where `range::LYING` has them): the rifle's own
/// magazine (empty) and a stack of its rounds.
const LYING: [(&str, u32); 3] = [("59d625f086f774661516605d", 1), ("64b7af5a8532cf95ee0a0dbd", 30), ("59d625f086f774661516605d", 1)];
/// The window the app starts in, in pixels. The options tab may change
/// it for someone at the keys; a run nobody is at the keys of keeps it,
/// since its scripted clicks go by places in a window of this size.
pub const WINDOW_SIZE: [u32; 2] = [1600, 900];
/// Stock BEAR arms.
const DEFAULT_ARMS: &str = "assets/content/hands/bear/bear_hands_skin.bundle";
/// The clothing that goes with those arms by default: the same faction's
/// own upper body and legs (`DefaultBearBody` and `DefaultBearFeet` in the
/// install's customization list).
const DEFAULT_BODY: &str = "assets/content/characters/character/prefabs/tshirt_bear_turtleneck.bundle";
const DEFAULT_LEGS: &str = "assets/content/characters/character/prefabs/pants_bear_srvv.bundle";
/// The first of the same faction's heads in the install's customization
/// list. Like the upper body it is a shadow and nothing more in first
/// person.
const DEFAULT_HEAD: &str = "assets/content/characters/character/prefabs/bear_head_0.bundle";

#[derive(Resource)]
pub struct GameData {
    pub unity: UnityAssets,
    pub db: Database,
    pub hard: HardSettings,
    pub engine: EngineSettings,
    /// Tuning of the weapon motion, taken by the weapon when it is spawned.
    pub hands_data: Option<HandsData>,
}

#[derive(Resource, Clone, Copy, PartialEq, Eq)]
pub enum View {
    /// Playing: the camera rides the weapon's animated camera bone.
    FirstPerson,
    /// Free camera for looking at the rig from outside.
    Free,
    /// Playing, watched from a camera that stands outside the character
    /// and keeps its place beside it: the keys and the mouse are the
    /// character's, and the picture is what someone else would see of it.
    Watched,
}

impl View {
    /// Whether the keys and the mouse are the character's, and not the
    /// free camera's.
    pub fn played(self) -> bool {
        self != View::Free
    }
}

/// Where the watching camera stands from the character's feet (`View::Watched`).
#[derive(Resource, Default)]
struct Watching(Option<Vec3>);

#[derive(Resource)]
pub struct Options {
    /// Item template id of the weapon.
    pub weapon: String,
    /// What the airdrop crate holds besides the run's own weapon: those
    /// asked for (`--add-weapon`). Every weapon whose logic is built lies
    /// in the crates of weapons along the knee-high wall (`weapon_crates`).
    pub weapons: Vec<String>,
    /// `--measure-rigs <crate|all>`: the crate of weapons (by its label, or
    /// all of them) whose every weapon's rig is made ready and measured,
    /// not put into the world, before the run ends (`weapon::Measured`);
    /// then their templates.
    measure_crate: Option<String>,
    pub measure_rigs: Vec<String>,
    /// Manifest key of the arms bundle.
    pub arms: String,
    /// The bundles of the clothing the body is drawn in: its upper body and
    /// its legs. `none` leaves one out.
    pub body: String,
    pub legs: String,
    pub head: String,
    /// What the character wears over its clothing: item template ids or
    /// bundles, any number.
    pub wear: Vec<String>,
    /// Weapons asked into the crate beside the app's own (`--add-weapon`),
    /// as given: a template, or a part of a name.
    added: Vec<String>,
    /// Carried weight in kilograms.
    weight: f32,
    view: View,
    /// Whose eyes the character is posed for, where the run says
    /// (`--pov first|third`); otherwise its own while the picture is its
    /// own, and another's from any camera outside it.
    pov: Option<trp_sim::PointOfView>,
    /// Clips to play at start, in order.
    play: Vec<String>,
    /// Freeze the first clip at this many seconds.
    hold_at: Option<f32>,
    /// Keys held for the whole run, joined by `+`: w a s d shift q e jump aim fire.
    pub hold: Option<String>,
    /// The game's field-of-view setting: vertical degrees, 50 to 75.
    pub field_of_view: f32,
    /// Whether `--fov` gave it, which then has the say over what the
    /// options tab last kept.
    fov_given: bool,
    /// The game's head-bobbing setting, 0.2 to 1.
    pub head_bobbing: f32,
    /// The game's mouse sensitivity setting.
    pub sensitivity: f32,
    /// The game's setting for the mouse's sensitivity with the sights up.
    /// The weapon's own factor multiplies it.
    pub aiming_sensitivity: f32,
    /// Whether the command line gave either, which then has the say over
    /// what the options tab last kept.
    sensitivity_given: [bool; 2],
    /// Degrees a count of the mouse turns the view before the
    /// sensitivity (`keys::degrees_a_count`), and the game's two switches
    /// that turn an axis of the mouse about, across and up and down.
    pub degrees_a_count: f32,
    pub invert: [bool; 2],
    /// Degrees a count of the mouse turns the head's free look
    /// (`keys::look_degrees_a_count`): no sensitivity comes after it.
    pub look_degrees_a_count: f32,
    /// Seed of the recoil's random numbers.
    pub seed: u64,
    /// What the character's skills add to the aiming speed, as a fraction.
    pub aim_speed_bonus: f32,
    /// The character's mastering of the weapon, 0 to 3. From 2 a reload
    /// keeps the sights up.
    pub mastering: u32,
    /// The character's skill with each weapon's class (Pistols, Sniper
    /// rifles and so on), 0 to 51, taken as one level for them all: it
    /// sets how fast the weapon's reloading and drawing states play.
    pub weapon_skill: u32,
    /// Frames a second the simulation runs at, whatever the rate of drawing.
    pub frame_rate: f32,
    /// The character's Strength skill level, 0 to 60.
    strength: f32,
    /// The Endurance skill's level, 0 to 50: what the legs hold, how fast
    /// the pools refill and what a jump costs go by it.
    endurance: f32,
    /// Where the view starts, in degrees below level: a capture run
    /// ignores the mouse, so this is how it looks down or up.
    pitch: f32,
    /// Which way the character starts facing, in degrees to the right of
    /// the lane: a capture run walks up to the range's walls with it.
    yaw: f32,
    /// Whether `--yaw` or `--pitch` was given: a `--start` before one of
    /// the range's things otherwise turns the view to it.
    view_given: bool,
    /// Save one frame here and exit. Used to check the app unattended.
    screenshot: Option<PathBuf>,
    /// Frame on which that capture is taken.
    screenshot_frame: u32,
    /// Write one line per rendered frame here: the clip on screen and
    /// where the weapon and the hands are as the camera sees them.
    log_frames: Option<PathBuf>,
    /// Write one line per rendered frame here: how long the frame took,
    /// what the simulation ran and cost in it, and where the camera is.
    log_pacing: Option<PathBuf>,
    /// Degrees a second a capture run turns the view to the right, from a
    /// second in.
    pub turn: f32,
    /// One of the recorder's scripts to play as the input, in place of
    /// the keyboard and mouse (`--scenario <file>`), and how many seconds
    /// into the run it begins (`--scenario-at`).
    pub scenario: Option<trp_sim::script::Scenario>,
    pub scenario_at: f32,
    /// Whether a view from outside takes in the whole figure.
    whole: bool,
    /// A capture from outside sees the range from overhead, from this many
    /// metres over where the character starts (`--view map[:<metres>]`).
    map: Option<f32>,
    /// Where in the range the character starts, if not in its middle:
    /// `platform`, `shelf` or `stairs`.
    start: Option<String>,
    /// A starting character of the install's database whose equipment the
    /// character carries and wears, as `Standard/usec`; nothing if not
    /// given.
    loadout: Option<String>,
    /// Whether `--weight` was given, which then has the say over the
    /// equipment's own weight.
    weight_given: bool,
    /// Start with the inventory screen open (for a capture).
    inventory: bool,
    /// Start with the crate open in that screen (`--loot`).
    loot: bool,
    /// Start with what is in the crate put on by the inventory's own
    /// moves: the weapon on the sling, the vest and the backpack worn
    /// (`--equip`). A run nobody is at the keys of does so unless told
    /// `--bare`, so that it has its weapon in hand as before.
    equip: bool,
    bare: bool,
    /// Whether a weapon hangs where the hands take one from as the run
    /// begins (`--equip`, a run nobody is at the keys of, a loadout): the
    /// run begins with it in hand. Otherwise it begins with empty hands and
    /// every weapon's rig is made when it is first wanted (`weapon::
    /// spawn_weapon`).
    pub starts_armed: bool,
    /// Drags of the mouse to make in a run nobody is at the keys of, each
    /// from one place in the window to another, in pixels (`--drag
    /// x:y:x:y`, as often as wanted): how the inventory screen's moving
    /// of things is tried without a hand on the mouse.
    drags: Vec<[f32; 4]>,
    /// The drawn frame the first of them begins on (`--drag-at`; 90 unless
    /// told), for a screen that opens later than that.
    drag_at: u32,
    /// Keys to press in such a run: the drawn frame, the key, and for
    /// how many frames it is held (`--tap <frame>:<key>[:<frames>]` with
    /// a letter, a digit, `tab`, `delete`, `space`, `ctrl`, `alt`,
    /// `shift`, `lmb` and so on). With any given, the keys are live in
    /// the run: the inventory's, and, in a run that holds no keys of its
    /// own (`--hold`), the character's, heard through the bindings as a
    /// hand's are.
    taps: Vec<(u32, keys::Key, u32)>,
    /// The mouse's travel in such a run: from a drawn frame, for so many
    /// frames, so many counts a frame across and down (`--mouse
    /// <frame>:<frames>:<across>:<down>`). It goes through the bindings
    /// as a hand's does: the turn's, or with the free look's key held
    /// (`--tap <frame>:mmb:<frames>`) the head's.
    pub mouse: Vec<(u32, u32, Vec2)>,
    /// Notches of the wheel to turn in such a run: the drawn frame and
    /// how many, up counted as positive (`--wheel <frame>:<notches>`).
    pub wheel: Vec<(u32, i32)>,
    /// Whether the range's ground and boxes carry grid lines (`--grid off`
    /// leaves them plain, as captures made before 2026-10-10 have them).
    grid: bool,
    /// Whether the window is left to answer a key pressed with Alt in the
    /// system's own way (`--alt-menu`), which shows the pointer for a
    /// moment at each such key: for setting beside what the app does.
    pub alt_menu: bool,
    /// Whether the frame-rate readout at the bottom right starts shown
    /// (`--fps off`); F3 switches it while running.
    pub show_fps: bool,
    /// The options' switch `CUSTOM FIRST PERSON HANDS` as the run begins,
    /// where the command line says (`--custom-hands on|off`): otherwise
    /// it is as the options' file has it, and on without one.
    pub custom_hands: Option<bool>,
    /// The switch `SMOOTH FIRST PERSON ANIMATIONS` as the run begins, the
    /// same way (`--smooth-animations on|off`).
    pub smooth_animations: Option<bool>,
    /// The switch `ICON GENERATION` as the run begins, the same way
    /// (`--icons on|off`).
    pub icon_generation: Option<bool>,
    /// Any switch of the options' features as the run begins, by its name
    /// in the options' file (`--feature stamina=off`, as often as wanted).
    pub feature_switches: Vec<(String, bool)>,
    /// How the window presents its frames: `vsync` (the default), `off`,
    /// or `mailbox`.
    present: String,
    /// The graphics backend: `dx12` or `vulkan`. Left out, it is DirectX 12
    /// on Windows (see `backends`).
    backend: Option<String>,
    /// Whether frames are rendered on a thread of their own while the next
    /// is worked out, which is the engine's way (`--render-thread on`).
    /// Off unless asked for: with it, the frames still come evenly but the
    /// moment each is worked out, which is when the mouse is read, does
    /// not. Measured at 165 frames a second: one update in eight ran more
    /// than a quarter of a frame early or late (typically 10 ms after the
    /// last and then 1.7 ms), so a mouse moved evenly would turn the view
    /// by 1.65 and then 0.3 of a frame's share; without it, one in a
    /// thousand (`--log-pacing`). It also shows input a frame sooner.
    render_thread: bool,
}

impl Options {
    /// Whether nobody is at the keys: a run that saves a frame and exits,
    /// or one that plays a script and logs how it was drawn.
    /// Whether the inventory's keys are to be listened to: someone is at
    /// them, or the run was given presses of its own.
    fn keys_live(&self) -> bool {
        !self.unattended() || !self.taps.is_empty() || !self.mouse.is_empty()
    }

    fn unattended(&self) -> bool {
        self.screenshot.is_some() || (self.scenario.is_some() && self.log_pacing.is_some())
    }

    /// Whose eyes the character is seen with in a view: the game has a
    /// character seen by anyone but itself in third person, with a pose
    /// and a weapon motion of its own (`docs/spec/third-person.md`).
    pub fn point_of_view(&self, view: View) -> trp_sim::PointOfView {
        self.pov.unwrap_or(match view {
            View::FirstPerson => trp_sim::PointOfView::FirstPerson,
            View::Free | View::Watched => trp_sim::PointOfView::ThirdPerson,
        })
    }
}

/// Which graphics backends the renderer may pick from.
///
/// On Windows the engine would take Vulkan first. Measured on the machine
/// this was written on (an NVIDIA card, a 165 Hz screen, the window synced
/// to it): through Vulkan the 165 frames a second come in clumps of four,
/// one 18.6 ms after the last and three within 1 to 3 ms of each other, so
/// what moves on screen moves by a ninth, a ninth, a sixth and three times
/// a fair frame's share; through DirectX 12 every frame comes 6.06 ms after
/// the last, give or take 0.3 (`--log-pacing`). So DirectX 12 unless told
/// otherwise, by `--backend` or the engine's own `WGPU_BACKEND`.
fn backends(options: &Options) -> Option<bevy::render::settings::Backends> {
    use bevy::render::settings::Backends;
    match options.backend.as_deref() {
        Some("vulkan") => Some(Backends::VULKAN),
        Some("dx12") => Some(Backends::DX12),
        Some(other) => {
            eprintln!("ignoring unknown backend {other}");
            None
        }
        None if cfg!(windows) && std::env::var_os("WGPU_BACKEND").is_none() => Some(Backends::DX12),
        None => None,
    }
}

/// DirectX's newer shader compiler, DXC, as its two libraries: beside
/// `trp.exe`, or in the newest Windows 10 or 11 SDK that has both for this
/// machine's kind (`Windows Kits\10\bin\<version>\x64`). The engine looks
/// for them in the folder it is run from alone, and without them compiles
/// with the older FXC: measured on 2026-10-14, DXC built what a run draws
/// with in 0.43 to 0.61 s where FXC took 1.59 to 2.04, and the rifles
/// crate's pictures in 4.0 s where FXC took 7.2; a frame once all is built
/// takes the same (`docs/findings.md`).
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

/// `settings` is what the player has set in the install's own game: the
/// app starts with those, since they are what the game beside it shows,
/// and with the game's defaults where the install does not say.
fn parse_options(settings: &trp_data::PlayerSettings) -> Options {
    let mut options = Options {
        weapon: DEFAULT_WEAPON.to_owned(),
        weapons: Vec::new(),
        measure_crate: None,
        measure_rigs: Vec::new(),
        arms: DEFAULT_ARMS.to_owned(),
        body: DEFAULT_BODY.to_owned(),
        legs: DEFAULT_LEGS.to_owned(),
        head: DEFAULT_HEAD.to_owned(),
        wear: Vec::new(),
        added: Vec::new(),
        weight: 0.0,
        view: View::FirstPerson,
        pov: None,
        play: Vec::new(),
        hold_at: None,
        hold: None,
        field_of_view: settings.field_of_view.unwrap_or(DEFAULT_VIEW[0]).clamp(50.0, 75.0),
        fov_given: false,
        head_bobbing: settings.head_bobbing.unwrap_or(DEFAULT_VIEW[1]).clamp(0.2, 1.0),
        sensitivity: settings.mouse_sensitivity.unwrap_or(DEFAULT_VIEW[2]).clamp(0.1, 5.0),
        aiming_sensitivity: settings.aiming_sensitivity.unwrap_or(DEFAULT_VIEW[3]).clamp(0.1, 5.0),
        sensitivity_given: [false; 2],
        degrees_a_count: 0.125,
        invert: [false; 2],
        look_degrees_a_count: 0.125,
        seed: 1,
        aim_speed_bonus: 0.0,
        mastering: 0,
        weapon_skill: 0,
        frame_rate: trp_sim::REFERENCE_FRAME_RATE,
        strength: 0.0,
        endurance: 0.0,
        pitch: 0.0,
        yaw: 0.0,
        view_given: false,
        start: None,
        loadout: None,
        weight_given: false,
        inventory: false,
        loot: false,
        equip: false,
        bare: false,
        starts_armed: false,
        drags: Vec::new(),
        drag_at: 90,
        taps: Vec::new(),
        mouse: Vec::new(),
        wheel: Vec::new(),
        grid: true,
        alt_menu: false,
        show_fps: true,
        custom_hands: None,
        smooth_animations: None,
        icon_generation: None,
        feature_switches: Vec::new(),
        whole: false,
        map: None,
        screenshot: None,
        screenshot_frame: 90,
        log_frames: None,
        log_pacing: None,
        turn: 0.0,
        scenario: None,
        scenario_at: 2.0,
        present: String::from("vsync"),
        backend: None,
        render_thread: false,
    };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--weapon" => options.weapon = args.next().unwrap_or_default(),
            // Another weapon for the crate, by its template or a part of
            // its name (`--add-weapon sv-98`): any the database has a
            // stock preset for. Its rig is built when it is first hung on
            // a slot, and it can be worked if its kind of logic is built.
            "--add-weapon" => options.added.extend(args.next()),
            "--measure-rigs" => options.measure_crate = args.next(),
            "--arms" => options.arms = args.next().unwrap_or_default(),
            "--body" => options.body = args.next().unwrap_or_default(),
            "--legs" => options.legs = args.next().unwrap_or_default(),
            "--head" => options.head = args.next().unwrap_or_default(),
            "--wear" => options.wear.extend(args.next()),
            "--weight" => {
                options.weight = args.next().and_then(|v| v.parse().ok()).unwrap_or(0.0);
                options.weight_given = true;
            }
            "--loadout" => options.loadout = args.next(),
            "--inventory" => options.inventory = true,
            "--loot" => (options.inventory, options.loot) = (true, true),
            "--equip" => options.equip = true,
            "--bare" => options.bare = true,
            "--wheel" => {
                let given = args.next().unwrap_or_default();
                match given.split_once(':').and_then(|(frame, notches)| Some((frame.parse().ok()?, notches.parse().ok()?))) {
                    Some(turn) => options.wheel.push(turn),
                    None => eprintln!("ignoring --wheel {given}: wanted <frame>:<notches>"),
                }
            }
            "--tap" => {
                let given = args.next().unwrap_or_default();
                let mut parts = given.split(':');
                let frame = parts.next().and_then(|v| v.parse().ok());
                let key = parts.next().and_then(keys::tapped);
                match (frame, key) {
                    (Some(frame), Some(key)) => options.taps.push((frame, key, parts.next().and_then(|v| v.parse().ok()).unwrap_or(2))),
                    _ => eprintln!("--tap takes a frame and a key joined by a colon: a letter, a digit, tab, delete, space, ctrl, alt, shift, caps, enter, escape, lmb or rmb"),
                }
            }
            "--mouse" => {
                let given = args.next().unwrap_or_default();
                let numbers: Vec<f32> = given.split(':').filter_map(|v| v.parse().ok()).collect();
                match numbers[..] {
                    [frame, frames, across, down] => options.mouse.push((frame as u32, frames as u32, Vec2::new(across, down))),
                    _ => eprintln!("ignoring --mouse {given}: wanted <frame>:<frames>:<counts across a frame>:<counts down a frame>"),
                }
            }
            "--drag-at" => options.drag_at = args.next().and_then(|frame| frame.parse().ok()).unwrap_or(options.drag_at),
            "--drag" => {
                let numbers: Vec<f32> = args.next().unwrap_or_default().split(':').filter_map(|v| v.parse().ok()).collect();
                match numbers[..] {
                    [x, y, to_x, to_y] => options.drags.push([x, y, to_x, to_y]),
                    _ => eprintln!("--drag takes four numbers joined by colons: from x, from y, to x, to y"),
                }
            }
            "--view" => {
                options.view = match args.next().as_deref() {
                    Some("side") => View::Free,
                    // The whole figure from three metres off, for captures.
                    Some("body") => {
                        options.whole = true;
                        View::Free
                    }
                    // The whole figure likewise, with the keys the
                    // character's: a capture that presses keys of its own
                    // (`--tap`) is heard as a played run's.
                    Some("watch") => {
                        options.whole = true;
                        View::Watched
                    }
                    // The range from overhead, ahead at the top; `map:<metres>`
                    // from that height.
                    Some(map) if map.starts_with("map") => {
                        options.map = Some(map.strip_prefix("map:").and_then(|h| h.parse().ok()).unwrap_or(30.0));
                        View::Free
                    }
                    _ => View::FirstPerson,
                }
            }
            // The pose and weapon motion of the character's own eyes or of
            // another's, whatever the view: `--pov first --view side` is
            // the first-person rig looked at from outside.
            "--pov" => {
                options.pov = match args.next().as_deref() {
                    Some("first") => Some(trp_sim::PointOfView::FirstPerson),
                    Some("third") => Some(trp_sim::PointOfView::ThirdPerson),
                    other => {
                        eprintln!("--pov takes first or third, not {other:?}");
                        None
                    }
                }
            }
            "--play" => options.play.extend(args.next()),
            "--at" => options.hold_at = args.next().and_then(|v| v.parse().ok()),
            "--hold" => options.hold = args.next(),
            "--fov" => {
                options.field_of_view = args.next().and_then(|v| v.parse().ok()).unwrap_or(DEFAULT_VIEW[0]).clamp(50.0, 75.0);
                options.fov_given = true;
            }
            "--head-bobbing" => {
                options.head_bobbing = args.next().and_then(|v| v.parse().ok()).unwrap_or(DEFAULT_VIEW[1]).clamp(0.2, 1.0)
            }
            "--sensitivity" => {
                options.sensitivity = args.next().and_then(|v| v.parse().ok()).unwrap_or(DEFAULT_VIEW[2]).clamp(0.1, 5.0);
                options.sensitivity_given[0] = true;
            }
            "--aiming-sensitivity" => {
                options.aiming_sensitivity =
                    args.next().and_then(|v| v.parse().ok()).unwrap_or(DEFAULT_VIEW[3]).clamp(0.1, 5.0);
                options.sensitivity_given[1] = true;
            }
            "--strength" => options.strength = args.next().and_then(|v| v.parse().ok()).unwrap_or(0.0f32).clamp(0.0, 60.0),
            "--endurance" => options.endurance = args.next().and_then(|v| v.parse().ok()).unwrap_or(0.0f32).clamp(0.0, 50.0),
            "--pitch" => {
                options.pitch = args.next().and_then(|v| v.parse().ok()).unwrap_or(0.0f32).clamp(-90.0, 90.0);
                options.view_given = true;
            }
            "--yaw" => {
                options.yaw = args.next().and_then(|v| v.parse().ok()).unwrap_or(0.0f32);
                options.view_given = true;
            }
            "--start" => options.start = args.next(),
            "--grid" => options.grid = args.next().as_deref() != Some("off"),
            "--alt-menu" => options.alt_menu = true,
            "--fps" => options.show_fps = args.next().as_deref() != Some("off"),
            "--custom-hands" => options.custom_hands = Some(args.next().as_deref() != Some("off")),
            "--smooth-animations" => options.smooth_animations = Some(args.next().as_deref() != Some("off")),
            "--icons" => options.icon_generation = Some(args.next().as_deref() != Some("off")),
            "--feature" => {
                let given = args.next().unwrap_or_default();
                match given.split_once('=') {
                    Some((name, on)) => options.feature_switches.push((name.to_owned(), on != "off")),
                    None => eprintln!("--feature takes a switch's name and on or off: {given}"),
                }
            }
            // Taken before the rest (`setup::wanted`).
            "--setup" => {}
            "--seed" => options.seed = args.next().and_then(|v| v.parse().ok()).unwrap_or(1),
            "--aim-speed-bonus" => {
                options.aim_speed_bonus = args.next().and_then(|v| v.parse().ok()).unwrap_or(0.0f32).clamp(0.0, 5.0)
            }
            "--mastering" => options.mastering = args.next().and_then(|v| v.parse().ok()).unwrap_or(0u32).min(3),
            "--weapon-skill" => options.weapon_skill = args.next().and_then(|v| v.parse().ok()).unwrap_or(0u32).min(51),
            "--frame-rate" => {
                options.frame_rate = args.next().and_then(|v| v.parse().ok()).unwrap_or(trp_sim::REFERENCE_FRAME_RATE).clamp(30.0, 480.0)
            }
            "--log-frames" => options.log_frames = args.next().map(PathBuf::from),
            "--log-pacing" => options.log_pacing = args.next().map(PathBuf::from),
            "--turn" => options.turn = args.next().and_then(|v| v.parse().ok()).unwrap_or(0.0),
            "--scenario" => {
                let file = PathBuf::from(args.next().unwrap_or_default());
                match trp_sim::script::Scenario::load(&file) {
                    Ok(scenario) => options.scenario = Some(scenario),
                    Err(e) => eprintln!("ignoring the scenario: {e:#}"),
                }
            }
            "--scenario-at" => options.scenario_at = args.next().and_then(|v| v.parse().ok()).unwrap_or(2.0f32).max(0.0),
            "--present" => options.present = args.next().unwrap_or_default(),
            "--backend" => options.backend = args.next(),
            "--render-thread" => options.render_thread = args.next().as_deref() == Some("on"),
            "--screenshot" => options.screenshot = args.next().map(PathBuf::from),
            "--screenshot-frame" => {
                options.screenshot_frame = args.next().and_then(|v| v.parse().ok()).unwrap_or(90)
            }
            other => eprintln!("ignoring unknown argument {other}"),
        }
    }
    options
}

fn main() -> anyhow::Result<()> {
    // The version before anything else, so that whatever a run prints says
    // which build it came from (`docs\manual.md`, "Versions"); `--version`
    // prints it alone.
    println!("TRP {}", env!("CARGO_PKG_VERSION"));
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--version") {
        return Ok(());
    }
    // Where the games are: asked in the console on a first start, or with
    // `--setup` (`setup`).
    if setup::wanted(&args) {
        setup::run(&mut std::io::stdin().lock())?;
    }
    let install = Install::locate()?;
    let settings = install.player_settings();
    let mut options = parse_options(&settings);
    options.degrees_a_count = keys::degrees_a_count(&install);
    options.look_degrees_a_count = keys::look_degrees_a_count(&install);
    options.invert = ["InvertedXAxis", "InvertedYAxis"].map(|name| install.control_flag(name).unwrap_or(false));
    println!(
        "view settings: field of view {}, head bobbing {}, mouse sensitivity {} and {} with the sights up, a count of the mouse {} degrees before either{}",
        options.field_of_view,
        options.head_bobbing,
        options.sensitivity,
        options.aiming_sensitivity,
        options.degrees_a_count,
        if settings == trp_data::PlayerSettings::default() { " (the install has none of its own)" } else { "" }
    );
    let db = Database::load(&install)?;
    // A weapon may be named by a part of its name: the first of the
    // database's weapons, by name, that has it.
    let named = |wanted: &str| -> Option<String> {
        if db.items.contains_key(wanted) {
            return Some(wanted.to_owned());
        }
        let wanted = wanted.to_lowercase();
        let mut found: Vec<_> = db.items.values().filter(|item| item.name.starts_with("weapon_") && db.is_a(item, "Weapon") && item.name.to_lowercase().contains(&wanted)).collect();
        found.sort_by(|a, b| a.name.cmp(&b.name));
        found.first().map(|item| item.id.clone())
    };
    match named(&options.weapon) {
        Some(template) => options.weapon = template,
        None => eprintln!("no weapon is named {}", options.weapon),
    }
    for wanted in std::mem::take(&mut options.added) {
        match named(&wanted) {
            Some(template) if !options.weapons.contains(&template) => options.weapons.push(template),
            Some(_) => {}
            None => eprintln!("--add-weapon {wanted}: the database has no such weapon"),
        }
    }
    // The weapons of one's own lie in their crate of weapons
    // (`weapon_crates`); one that could not be read is said here.
    for note in &db.custom_notes {
        eprintln!("a weapon of one's own is left out: {note}");
    }
    // What the character carries: nothing, or a starting character's
    // equipment as the install's database has it (inventory spec). Its
    // weight is the carried weight unless `--weight` says otherwise, and
    // what sits in the slots the game draws is worn on the body.
    // What was asked to be worn whatever the equipment says (`--wear`).
    let fixed_wear = options.wear.clone();
    let mut equipment = trp_sim::inventory::Equipment::empty(&db)?;
    if let Some(loadout) = &options.loadout {
        let (edition, side) = loadout.split_once('/').unwrap_or((loadout.as_str(), "usec"));
        match trp_data::inventory::Loadout::load(&install, edition, side).and_then(|list| trp_sim::inventory::Equipment::of_loadout(&db, &list)) {
            Ok((worn, left_out)) => {
                for entry in left_out {
                    eprintln!("the loadout's {entry} has no place and is left out");
                }
                equipment = worn;
                if !options.weight_given {
                    options.weight = equipment.carried_weight(false);
                }
                options.wear.extend(equipment.drawn().into_iter().map(str::to_owned));
                println!("the loadout {loadout}: {:.2} kg carried", equipment.carried_weight(false));
            }
            Err(e) => eprintln!("the loadout {loadout} could not be read: {e:#}"),
        }
    }
    let mut names = trp_data::inventory::short_names(&install, "en").unwrap_or_else(|e| {
        eprintln!("the install's item names could not be read: {e:#}");
        Default::default()
    });
    // A weapon of one's own has no name in the install's texts: its
    // file's.
    for definition in db.custom.values() {
        names.extend(definition.names().map(|(id, name)| (id.to_owned(), name.to_owned())));
    }
    // Nor has an outfit of one's own.
    for definition in db.outfits.values() {
        names.insert(definition.id.clone(), definition.shown_name().to_owned());
    }
    // The crate on the range (inventory spec, "The crate"): the install's
    // airdrop crate, holding the app's weapon as its stock preset
    // has it and the vest and the backpack a BEAR of the Standard edition
    // starts with, each with what the database has in it, but for the
    // magazines and the rounds: those are the weapon's own (the user,
    // 2026-10-11; `Loadout::fit_to`). The two are made for the body at the
    // start and drawn once they are put on.
    // Where the game's own screen has the equipment's slot views.
    let panel = trp_data::screen::GearPanel::load(&install).unwrap_or_else(|e| {
        eprintln!("the install's equipment panel could not be read, the app's own arrangement is used: {e:#}");
        Default::default()
    });
    let rules = trp_sim::inventory::Rules::of(&db);
    let mut crate_kind = trp_sim::inventory::Kind::of(&db, CRATE)?;
    if let Some(grid) = crate_kind.grids.first_mut() {
        grid.cells = CRATE_CELLS;
    }
    let mut loot = trp_sim::inventory::Item::new(std::sync::Arc::new(crate_kind));
    // The tables of weapons along the knee-high wall: their gun cases, and
    // their magazine boxes.
    let (cases, boxes) = weapon_crates(&db)?;
    // The run's own weapon is the one in its class's gun case (the user,
    // 2026-10-14, night: "move the AKM into the rifle case"), and a run that
    // begins with it on takes it from there: its class, and its number.
    let own = crate_of(&db, &options.weapon).and_then(|class| {
        let found = cases.get(class)?.grids.iter().flatten().find(|(_, item)| item.kind.template == options.weapon);
        found.map(|(_, item)| (class, item.id))
    });
    // What a run nobody is at the keys of, or one begun with `--equip`,
    // has on from the start: the weapon it is run with, the vest and the
    // backpack.
    let mut put_on = Vec::new();
    put_on.extend(own.map(|(_, id)| id));
    {
        use trp_data::inventory::Loadout;
        use trp_sim::inventory::{Equipment, Item};
        let mut things = Vec::new();
        // Any other weapon asked for (`--add-weapon`), and the run's own
        // where no gun case has it, each as its stock preset has it and
        // with what loads it, in the crate; the run's own has the kit's
        // magazines in the vest.
        let weapons = std::iter::once(options.weapon.as_str()).filter(|_| own.is_none()).chain(options.weapons.iter().map(String::as_str).filter(|weapon| *weapon != options.weapon));
        for weapon in weapons {
            match armed(&db, weapon, weapon != options.weapon) {
                Ok(mut made) => {
                    if weapon == options.weapon {
                        put_on.push(made[0].id);
                    }
                    things.append(&mut made);
                }
                Err(e) => eprintln!("the crate has no {weapon}: {e:#}"),
            }
        }
        let kit = Loadout::load(&install, "Standard", "bear").and_then(|mut list| {
            let [magazines, rounds] = list.fit_to(&db, &options.weapon)?;
            println!("the crate's kit: {magazines} magazines and {rounds} lots of rounds changed for the weapon's own");
            Equipment::of_loadout(&db, &list)
        });
        match kit {
            Ok((mut outfit, _)) => {
                for worn in ["TacticalVest", "Backpack"].into_iter().filter_map(|slot| outfit.take_off(slot)) {
                    options.wear.push(worn.kind.template.clone());
                    put_on.push(worn.id);
                    things.push(worn);
                }
            }
            Err(e) => eprintln!("the crate has no vest and no backpack: {e:#}"),
        }
        // The outfits that are not the install's (`docs/spec/outfits.md`):
        // each goes on where the armour it is made from does.
        let mut outfits: Vec<&String> = db.outfits.keys().collect();
        outfits.sort();
        for id in outfits {
            match trp_sim::inventory::Kind::of(&db, id) {
                Ok(kind) => things.push(Item::new(std::sync::Arc::new(kind))),
                Err(e) => eprintln!("the crate has no {id}: {e:#}"),
            }
        }
        // The key of the range's locked door.
        match trp_sim::inventory::Kind::of(&db, DOOR_KEY) {
            Ok(kind) => things.push(Item::new(std::sync::Arc::new(kind))),
            Err(e) => eprintln!("the crate has no key: {e:#}"),
        }
        for thing in things {
            let (name, [width, height], weight) = (thing.kind.name.clone(), thing.cells(), thing.weight());
            match loot.stow(thing) {
                Ok(_) => println!("in the crate: {name}, {width} x {height} cells, {weight:.2} kg"),
                Err(_) => eprintln!("the crate has no room for {name} ({width} x {height} cells)"),
            }
        }
    }
    // The range's other containers, and the things lying about.
    let mut away: Vec<Option<trp_sim::inventory::Item>> = std::iter::once(Ok(None))
        .chain(CONTAINERS.iter().map(|template| {
            trp_sim::inventory::Kind::of(&db, template).map(|kind| Some(trp_sim::inventory::Item::new(std::sync::Arc::new(kind))))
        }))
        .collect::<anyhow::Result<_>>()?;
    // The weapons whose rigs are to be measured (`--measure-rigs`).
    if let Some(wanted) = options.measure_crate.as_deref() {
        for (holds, label) in cases.iter().zip(range::WEAPON_CRATES) {
            if wanted.eq_ignore_ascii_case("all") || wanted.eq_ignore_ascii_case(label) {
                let weapons = holds.grids.iter().flatten().map(|(_, thing)| thing).filter(|thing| trp_sim::inventory::is(thing, "Weapon"));
                options.measure_rigs.extend(weapons.map(|weapon| weapon.kind.template.clone()));
            }
        }
    }
    // The tables' gun cases, then their magazine boxes, after those.
    away.extend(cases.into_iter().chain(boxes).map(Some));
    let mut ground = Vec::new();
    for ((template, count), at) in LYING.into_iter().zip(range::LYING) {
        match trp_sim::inventory::Kind::of(&db, template) {
            Ok(kind) => {
                let mut item = trp_sim::inventory::Item::new(std::sync::Arc::new(kind));
                item.count = count;
                ground.push((item, Vec3::from(at)));
            }
            Err(e) => eprintln!("nothing lies at {at:?}: {e:#}"),
        }
    }
    // A run nobody is at the keys of has it all on from the start, put on
    // by the inventory's own moves; so does `--equip`.
    if (options.equip || options.unattended()) && !options.bare {
        use trp_sim::inventory::{move_item, Spot};
        let worn = equipment.item.id;
        // (The run's own weapon is in its gun case; the rest in the crate.)
        let mut case = own.and_then(|(class, _)| away.get_mut(range::gun_case_of(class))).and_then(Option::as_mut);
        for id in put_on {
            let mut roots: Vec<&mut trp_sim::inventory::Item> = vec![&mut equipment.item, &mut loot];
            roots.extend(case.as_deref_mut());
            let slots = 0..roots[0].kind.slots.len();
            if !slots.into_iter().any(|slot| move_item(&mut roots, id, worn, Spot::Slot(slot), &rules).is_ok()) {
                eprintln!("something in the crate goes into no slot");
            }
        }
    }
    options.starts_armed = trp_sim::inventory::HAND_SLOTS.into_iter().any(|slot| equipment.worn(slot).is_some());
    // Such a run is what the app's own figures are measured with, so it
    // weighs what it did before there was an inventory (what `--weight`
    // says; nothing, if nothing was said) unless the inventory is what it
    // is run for (`--equip`, `--inventory`, `--loot`).
    if options.unattended() && !(options.equip || options.inventory) {
        options.weight_given = true;
    }
    if !options.weight_given {
        options.weight = equipment.carried_weight(false);
    }
    println!("carried: {:.2} kg", options.weight);
    let mut unity = UnityAssets::new(install)?;
    let hard = HardSettings::load(&unity.install)?;
    let engine = EngineSettings::load(&unity.install)?;
    let mut movement_settings = trp_sim::load_movement_settings(&mut unity, &db, &hard, &engine, options.weight)?;
    movement_settings.set_strength(options.strength);
    // The body's animator decides the movement states and supplies the
    // motion. The weapon's width, which it also takes, is set once the
    // weapon is assembled.
    // The keys as the install binds them, and what the options tab has
    // kept over them and for the display (for someone at the keys only);
    // the features' switches as the run begins, which hold until it ends
    // (each is "restart required": `options_view::Features`).
    let mut bindings = keys::Bindings::of(&unity.install);
    let options_state = options_view::OptionsState::new(&mut options, &unity.install, &mut bindings, WINDOW_SIZE);
    // The clothing the body is drawn in names the bones the animator has
    // to pose for it.
    let mut clothing = body_view::BodyParts::load(&mut unity, &db, &[&options.body, &options.legs, &options.head], &options.wear, options_state.features.custom_hands);
    // And, with the options' `CHARACTER MODEL` on, the pose the game's
    // screens stand the character in, for the gear page's model of it.
    let standing: &[String] = if options_state.features.character_model { &clothing.standing } else { &[] };
    let body_data = match trp_sim::body::BodyData::load_standing(&mut unity, &clothing.bones, standing) {
        Ok((data, standing)) => {
            match standing {
                Some(Ok(pose)) => clothing.standing_pose = Some(pose),
                Some(Err(e)) => eprintln!("the character model is not drawn: {e:#}"),
                None => {}
            }
            data
        }
        Err(e) => {
            eprintln!("the body is not drawn: {e:#}");
            clothing = body_view::BodyParts::default();
            trp_sim::body::BodyData::load(&mut unity)?
        }
    };
    let body = trp_sim::Body::new(&body_data, 1.0);
    let mut movement = trp_sim::Movement::new(movement_settings, body);
    // What the two skills make of stamina.
    movement.stamina.settings.set_skills(options.endurance, options.strength);
    movement.stamina.resettle();
    movement.pitch = options.pitch;
    (movement.yaw, movement.body_yaw) = (options.yaw, options.yaw);
    movement.note_rotation();
    let hands_data = match HandsData::load(&mut unity) {
        Ok(data) => Some(data),
        Err(e) => {
            eprintln!("weapon motion data could not be read: {e:#}");
            None
        }
    };

    // The test range stops the character as far off as the game's capsule would.
    let mut range = range::Range::default();
    match trp_data::player::CapsuleData::load(&mut unity) {
        Ok(capsule) => {
            // The player's controller is the game's own, which keeps no skin
            // outside the capsule (movement spec, "The character's own
            // collision"): recorded, a character stops the radius from a wall.
            range.radius = capsule.radius;
        }
        Err(e) => eprintln!("the character's capsule could not be read: {e:#}"),
    }
    // The doors, containers and trunk turn by the install's curve and wait
    // its delay.
    match trp_sim::things::ThingSettings::new(&hard) {
        Ok(settings) => range.thing_settings = Some(settings),
        Err(e) => eprintln!("the doors and containers will not move: {e:#}"),
    }
    // The right-hand door is locked, its key in the crate: to be unlocked
    // with it, or kicked (which opens a door only from the side it swings
    // away from: here, from the yard).
    range.lock_door(1, DOOR_KEY);
    // A capture can start somewhere else in the range than its middle.
    if let Some(name) = &options.start {
        match range.place(name) {
            Some(at) => movement.position = at,
            None => eprintln!("ignoring unknown place {name}: platform, shelf, stairs, doors, yard, car, wooden-crate, supply-crate, scav, drawer, table, high-shelf or floor"),
        }
        if let (false, Some([yaw, pitch])) = (options.view_given, range::Range::start_view(name)) {
            movement.pitch = pitch;
            (movement.yaw, movement.body_yaw) = (yaw, yaw);
            movement.note_rotation();
        }
    }

    // The camera is kept out of the world by a box of its own.
    let camera_box = match trp_data::player::CameraBoxData::load(&mut unity) {
        Ok(shape) => Some(shape),
        Err(e) => {
            eprintln!("the camera's collider could not be read: {e:#}");
            None
        }
    };


    let mut player = ClipPlayer::new("");
    player.queue = options.play.iter().cloned().collect();
    player.hold_at = options.hold_at;
    player.by_name = !options.play.is_empty() || options.hold_at.is_some();
    let present_mode = match options.present.as_str() {
        "off" => bevy::window::PresentMode::AutoNoVsync,
        "mailbox" => bevy::window::PresentMode::Mailbox,
        _ => bevy::window::PresentMode::AutoVsync,
    };
    // A window without the focus is updated only some 60 times a second, on
    // a timer. An unattended run seldom has the focus and is there to show
    // what a played one does, so it updates as a focused window would.
    let mut updates = bevy::winit::WinitSettings::game();
    if options.unattended() {
        updates.unfocused_mode = bevy::winit::UpdateMode::Continuous;
    }
    let mut renderer = bevy::render::settings::WgpuSettings::default();
    if let Some(backends) = backends(&options) {
        renderer.backends = Some(backends);
    }
    // DirectX 12's shaders by its newer compiler where it is found
    // (`dxc_libraries`); the engine's own choice, the variable
    // `WGPU_DX12_COMPILER`, still has the last word.
    if bevy::render::settings::Dx12Compiler::from_env().is_none() {
        match dxc_libraries() {
            Some((dxc, dxil)) => {
                println!("DirectX 12's shaders by DXC: {}", dxc.display());
                renderer.dx12_shader_compiler =
                    bevy::render::settings::Dx12Compiler::DynamicDxc { dxc_path: dxc.display().to_string(), dxil_path: dxil.display().to_string() };
            }
            None if cfg!(windows) => println!("DirectX 12's shaders by FXC: DXC's dxcompiler.dll and dxil.dll are neither beside trp.exe nor in a Windows SDK"),
            None => {}
        }
    }
    let mut plugins = DefaultPlugins
        .set(WindowPlugin {
            primary_window: Some(Window {
                // `local\window_keys_test.ps1` finds the window by it.
                title: "TRP (Tushonka Runtime Project)".into(),
                resolution: bevy::window::WindowResolution::new(WINDOW_SIZE[0] as f32, WINDOW_SIZE[1] as f32),
                present_mode,
                ..default()
            }),
            ..default()
        })
        .set(bevy::render::RenderPlugin {
            render_creation: bevy::render::settings::RenderCreation::Automatic(renderer),
            ..default()
        });
    if !options.render_thread {
        plugins = plugins.disable::<bevy::render::pipelined_rendering::PipelinedRenderingPlugin>();
    }

    let mut controller =
        Controller::new(movement, trp_sim::Pacer::new(options.frame_rate, engine.fixed_timestep, engine.maximum_timestep));
    controller.camera_box = camera_box;
    // The simulation's systems as the options' switches have them for
    // this run (settings spec, "Features").
    controller.movement.systems = options_state.features.systems();
    if !controller.movement.systems.weight {
        controller.movement.set_weight(&hard, &db.config, 0.0)?;
    }

    // Until the renderer has reported, it counts as having work to do.
    let waiting = PipelinesWaiting(std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(1)));
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.55, 0.63, 0.70)))
        .insert_resource(AmbientLight { color: Color::WHITE, brightness: 400.0, ..default() })
        .insert_resource(controller)
        .insert_resource(inventory_view::Carried {
            equipment,
            names,
            loot,
            loot_open: false,
            away,
            loot_of: 0,
            rules,
            fixed_wear,
            note: String::new(),
            panel,
            ground,
            icons: Default::default(),
            pictures: Default::default(),
            pictures_on: options_state.features.icon_generation,
            picture_failed: Default::default(),
            spinner: Handle::default(),
            bound: Default::default(),
            windows: Vec::new(),
            priority: None,
            // The game's setting for which window Ctrl and a click sends
            // things to, as the install has it (`Auto` where it has none:
            // what the one read on 2026-10-12 had).
            priority_auto: unity.install.game_setting("PriorityWindowMode").is_none_or(|mode| mode == "Auto"),
            asking: None,
            pack_asked: false,
            falling: Vec::new(),
        })
        .insert_resource(bindings)
        .insert_resource(icons::Icons::new(unity.install.clone(), options_state.features.icon_generation, icons::signatures(&db)))
        .insert_resource(options_state.features.clone())
        .insert_resource(options_state)
        .insert_resource(GameData { unity, db, hard, engine, hands_data })
        .insert_resource(options.view)
        .insert_resource(options)
        .insert_resource(player)
        .init_resource::<render_data::Converted>()
        .insert_resource(range)
        .insert_resource(clothing)
        .init_resource::<body_view::KeptCollarbones>()
        .init_resource::<body_view::OutfitOn>()
        .init_resource::<body_view::OutfitPose>()
        .add_plugins(plugins)
        .add_plugins(grid::GridPlugin)
        .insert_resource(FrameMark(std::time::Instant::now()))
        .init_resource::<Watching>()
        .init_resource::<keys::ScriptedMouse>()
        .insert_resource(Warmup(true))
        .init_resource::<render_data::SharedPictures>()
        .insert_resource(waiting.clone())
        .insert_resource(updates)
        .add_plugins(hud::HudPlugin)
        .add_plugins(inventory_view::InventoryPlugin)
        .add_plugins(options_view::OptionsPlugin)
        .add_plugins(icons::IconsPlugin)
        .add_plugins(figure_view::FigurePlugin)
        .add_systems(Startup, (setup_world, range::spawn_labels, (weapon::spawn_weapon, body_view::spawn_body).chain()))
        .add_systems(
            Update,
            (
                (mark_frame, keys::hear, weapon::prepare_rigs, weapon::free_rigs).chain(),
                (switch_view, cursor::hold, cursor::keep_alt).chain(),
                controller::gather_input.run_if(running),
                controller::step.run_if(running),
                fly_camera,
                controller::apply,
                (show_things, range::place_labels),
                animation::play_clips,
                animation::pose_rig,
                hands_view::apply_hands,
                arms::place_hands,
                (body_view::show_outfit, body_view::show_upper_body).chain(),
                body_view::pose_body,
                (body_view::see_collarbones, body_view::keep_collarbones, body_view::see_outfit, body_view::pose_outfit).chain(),
                hud::debug,
                log_frame.run_if(running),
                log_pacing.run_if(running),
                capture.run_if(running),
                warm_up,
            )
                .chain(),
        );
    // What a rig brought to the graphics card, asked about here and
    // answered by the renderer (`weapon::Uploads`).
    let uploads = weapon::UploadWatch::default();
    app.insert_resource(uploads.clone());
    // The graphics card's places for samplers held, counted by the renderer
    // (`weapon::SamplerPlaces`).
    let places = weapon::SamplerPlaces::default();
    app.insert_resource(places.clone());
    if let Some(renderer) = app.get_sub_app_mut(bevy::render::RenderApp) {
        use bevy::render::{Render, RenderSet};
        renderer.insert_resource(waiting).add_systems(Render, count_waiting.in_set(RenderSet::Cleanup));
        renderer.insert_resource(uploads).add_systems(Render, weapon::watch_uploads.in_set(RenderSet::Cleanup));
        renderer.insert_resource(places).add_systems(Render, weapon::count_places.in_set(RenderSet::Cleanup));
        // Where a slow frame of the renderer's goes, if asked for.
        render_timing::add(renderer);
    }
    render_timing::add_main(&mut app);
    app.run();
    Ok(())
}

/// Whether the run is still warming up.
///
/// The engine builds what it draws a kind of thing with (a pipeline: its
/// shaders, for one layout of mesh and one sort of material) the first
/// time that kind of thing is in the picture. It builds in the background,
/// several at once, and leaves the thing out of the picture until its
/// pipeline is ready, which on DirectX is a second or more: the range, the
/// body and the weapon were missing from the first second and a half of
/// every run, and a weapon first drawn later (a character that begins with
/// empty hands) was missing again while its own were built.
///
/// So the run begins with what it will come to draw in the picture at
/// once, behind a curtain: the weapon though the character may begin
/// without it, and a thing as it lies on the ground. When the renderer
/// says it has nothing left to build, the curtain goes and the run begins.
/// Measured on the machine this was written on: 1.7 s with DirectX 12 and
/// 0.4 s with Vulkan (`--backend vulkan`), which is how long the range and
/// the weapon used to be missing for; nothing is built that would not have
/// been, and nothing of it is left once it is over. Not shown beforehand,
/// and so still a moment late the first time they are seen: the upper body
/// from outside (the free camera; a fifth of the kind and a second more on
/// DirectX), and whatever is first worn in a run that began bare.
#[derive(Resource)]
pub struct Warmup(pub bool);

/// Whether the run has begun: the warm-up is over. The simulation, the
/// keys, the scripted presses and drags of an unattended run and the
/// frames a capture counts all wait for it, so that a run's first frame
/// is the first one that shows it whole.
pub fn running(warmup: Res<Warmup>) -> bool {
    !warmup.0
}

/// How many pipelines the renderer still has to build, as it last said.
/// The renderer's own world writes it at the end of each of its frames.
#[derive(Resource, Clone)]
pub(crate) struct PipelinesWaiting(pub(crate) std::sync::Arc<std::sync::atomic::AtomicUsize>);

/// The frames a warm-up lasts at the least (what reaches the renderer does
/// so a frame or two after it is made), the frames in a row the renderer
/// must have had nothing to build, and the seconds after which the run
/// begins whatever the renderer says.
const WARMUP_FRAMES: u32 = 4;
const WARMUP_CALM: u32 = 8;
const WARMUP_LIMIT: f32 = 30.0;

/// What is in the scene only while the run warms up: the curtain, and
/// things shown for the building's sake.
#[derive(Component)]
struct WarmupProp;

/// In the renderer's world: how many pipelines are still to be built.
fn count_waiting(cache: Res<bevy::render::render_resource::PipelineCache>, waiting: Res<PipelinesWaiting>, mut frame: Local<u32>) {
    waiting.0.store(cache.waiting_pipelines().count(), std::sync::atomic::Ordering::Relaxed);
    *frame += 1;
    // `TRP_ICON_TRACE`: each pipeline still waiting, every 60 of the
    // renderer's frames, by its label and how far it has got (a new kind of
    // material's took 1 to 2.5 s on DirectX 12, 2026-10-14).
    if std::env::var_os("TRP_ICON_TRACE").is_some() && *frame % 60 == 0 {
        use bevy::render::render_resource::{CachedPipelineState, PipelineDescriptor};
        let pipelines: Vec<&bevy::render::render_resource::CachedPipeline> = cache.pipelines().collect();
        for id in cache.waiting_pipelines() {
            let Some(pipeline) = pipelines.get(id) else { continue };
            let state = match &pipeline.state {
                CachedPipelineState::Queued => "queued".to_owned(),
                CachedPipelineState::Creating(_) => "creating".to_owned(),
                CachedPipelineState::Ok(_) => "ok".to_owned(),
                CachedPipelineState::Err(e) => format!("error {e}"),
            };
            let (label, defs) = match &pipeline.descriptor {
                PipelineDescriptor::RenderPipelineDescriptor(d) => (d.label.clone(), d.fragment.as_ref().map_or(0, |f| f.shader_defs.len())),
                PipelineDescriptor::ComputePipelineDescriptor(d) => (d.label.clone(), d.shader_defs.len()),
            };
            info!("pipeline trace {}: {id} {label:?} {state}, {defs} defs", *frame);
        }
    }
}

/// Ends the warm-up when the renderer has caught up, and clears its
/// things away.
fn warm_up(
    mut commands: Commands,
    mut warmup: ResMut<Warmup>,
    waiting: Res<PipelinesWaiting>,
    time: Res<Time<Real>>,
    props: Query<Entity, With<WarmupProp>>,
    mut budget: ResMut<bevy::render::render_asset::RenderAssetBytesPerFrame>,
    preparing: Option<Res<weapon::Preparing>>,
    mut frames: Local<[u32; 2]>,
) {
    if !warmup.0 {
        return;
    }
    let [seen, calm] = &mut *frames;
    *seen += 1;
    // A rig asked for at the start is made ready on another thread; the
    // curtain waits for it, so that what it brings to the graphics card
    // goes up under it (the StG 44's uncompressed pictures had stood a
    // frame of play still for 100 ms). The inventory's pictures are not
    // waited for: as in the game they are asked for when the screen first
    // shows an item (`icons::ask_icons`; icons spec, "When a picture is
    // asked for").
    let preparing = preparing.is_some_and(|preparing| preparing.busy());
    let building = waiting.0.load(std::sync::atomic::Ordering::Relaxed) > 0 || *seen < WARMUP_FRAMES || preparing;
    *calm = if building { 0 } else { *calm + 1 };
    if *calm >= WARMUP_CALM || time.elapsed_secs() > WARMUP_LIMIT {
        warmup.0 = false;
        info!("the renderer had built what it draws with {:.2} s into the run ({} frames)", time.elapsed_secs(), *seen);
        for prop in &props {
            commands.entity(prop).despawn();
        }
        // From here on what goes up to the graphics card is spread over
        // frames, a picture or so a frame: a rig made ready away from the
        // frame (`weapon::Preparing`) brings tens of megabytes of pictures
        // at once, which in one frame stood it still for 137 ms (the
        // StG 44's, uncompressed). Under the curtain, all at once.
        budget.max_bytes = Some(UPLOAD_BUDGET);
    }
}

/// What goes up to the graphics card a frame after the curtain, at most
/// (a whole picture or mesh at least).
const UPLOAD_BUDGET: usize = 8 << 20;

/// When this frame's systems began, for `--log-pacing`.
#[derive(Resource)]
struct FrameMark(std::time::Instant);

fn mark_frame(mut mark: ResMut<FrameMark>) {
    mark.0 = std::time::Instant::now();
}

/// Writes one line for the frame about to be drawn, when asked to: how long
/// the frame took by the engine's clock and by a clock read here, what the
/// simulation ran in it and what that cost, and where the camera is in the
/// world. Whether the app draws evenly is read off these.
fn log_pacing(
    options: Res<Options>,
    real: Res<Time<Real>>,
    mark: Res<FrameMark>,
    controller: Res<Controller>,
    player: Query<&Transform, With<weapon::Player>>,
    pivot: Query<&Transform, With<weapon::ViewPivot>>,
    camera: Query<&Transform, With<MainCamera>>,
    body: Option<Res<body_view::BodyRig>>,
    parts: Res<body_view::BodyParts>,
    helper: TransformHelper,
    mut window: Query<&mut Window, With<PrimaryWindow>>,
    mut out: Local<Option<std::io::BufWriter<std::fs::File>>>,
    mut last: Local<Option<std::time::Instant>>,
    mut frame: Local<u32>,
) {
    use std::io::Write as _;
    let Some(path) = &options.log_pacing else { return };
    let (Ok(player), Ok(pivot), Ok(camera)) = (player.single(), pivot.single(), camera.single()) else { return };
    *frame += 1;
    // An unattended run is there to show what a played window does, and a
    // window is presented differently without the focus: it asks for it.
    let mut focused = false;
    if let Ok(mut window) = window.single_mut() {
        focused = window.focused;
        if options.unattended() && !focused && *frame % 60 == 30 {
            window.focused = true;
        }
    }
    if out.is_none() {
        match std::fs::File::create(path) {
            Ok(file) => {
                let mut file = std::io::BufWriter::new(file);
                let _ = writeln!(
                    file,
                    "frame t dt_ms wall_ms pacer_ms sim_frames physics_steps sim_ms update_ms blend cam_x cam_y cam_z yaw pitch state focused feet_x feet_y feet_z body_yaw lfoot_x lfoot_y lfoot_z rfoot_x rfoot_y rfoot_z pelvis_x pelvis_y pelvis_z rib_x rib_y rib_z script_t ltoe_x ltoe_y ltoe_z rtoe_x rtoe_y rtoe_z anim anim_next"
                );
                *out = Some(file);
            }
            Err(e) => {
                warn!("{}: {e}", path.display());
                return;
            }
        }
    }
    let now = std::time::Instant::now();
    let wall = last.replace(now).map_or(0.0, |before| (now - before).as_secs_f64() * 1000.0);
    // The camera hangs from the view pivot, and that from the player.
    let eye = player.transform_point(pivot.transform_point(camera.translation));
    let (yaw, pitch) = controller.look();
    // The point the view hangs at, in the world.
    let ribcage = player.transform_point(pivot.translation);
    let paced = controller.paced;
    // The drawn body's feet and hips, in the world, where a body is drawn.
    let drawn = |end: &str| {
        body.as_ref()
            .and_then(|rig| rig.bone(&parts, end))
            .and_then(|bone| helper.compute_global_transform(bone).ok())
            .map_or(Vec3::ZERO, |at| at.translation())
    };
    let (left, right, pelvis) = (drawn("/Base HumanLFoot"), drawn("/Base HumanRFoot"), drawn("/Base HumanPelvis"));
    let (left_toe, right_toe) = (drawn("/Base HumanLToe"), drawn("/Base HumanRToe"));
    // The moment drawn, by the clock of the script being played if one is.
    let script_time = controller.drawn_time() - options.scenario.as_ref().map_or(0.0, |_| options.scenario_at);
    // The state the body's animator is in and the one it is entering, by
    // their own names. These are the simulation's newest, a frame or two
    // ahead of what is drawn.
    let own = |name: &str| name.rsplit('.').next().unwrap_or(name).replace(' ', "_");
    let (anim, anim_next) = controller.movement.body.state_names();
    let (anim, anim_next) = (own(anim), anim_next.map_or(String::from("-"), own));
    let Some(out) = out.as_mut() else { return };
    let _ = writeln!(
        out,
        "{} {:.5} {:.4} {:.4} {:.4} {} {} {:.4} {:.4} {:.4} {:.5} {:.5} {:.5} {:.4} {:.4} {:?} {} {:.5} {:.5} {:.5} {:.4} {:.5} {:.5} {:.5} {:.5} {:.5} {:.5} {:.5} {:.5} {:.5} {:.5} {:.5} {:.5} {:.5} {:.5} {:.5} {:.5} {:.5} {:.5} {:.5} {anim} {anim_next}",
        *frame,
        real.elapsed_secs_f64(),
        real.delta_secs_f64() * 1000.0,
        wall,
        paced.host_dt * 1000.0,
        paced.frames,
        paced.physics_steps,
        paced.cost * 1000.0,
        (now - mark.0).as_secs_f64() * 1000.0,
        paced.blend,
        eye.x,
        eye.y,
        eye.z,
        yaw,
        pitch,
        controller.movement.state,
        focused as u8,
        player.translation.x,
        player.translation.y,
        player.translation.z,
        controller.movement.drawn_body_yaw(),
        left.x,
        left.y,
        left.z,
        right.x,
        right.y,
        right.z,
        pelvis.x,
        pelvis.y,
        pelvis.z,
        ribcage.x,
        ribcage.y,
        ribcage.z,
        script_time,
        left_toe.x,
        left_toe.y,
        left_toe.z,
        right_toe.x,
        right_toe.y,
        right_toe.z,
    );
}

/// Writes one line for the frame about to be drawn, when asked to. Places
/// are in the camera's own axes (x right, y up, -z ahead), in metres.
fn log_frame(
    options: Res<Options>,
    time: Res<Time>,
    player: Res<ClipPlayer>,
    rig: Option<Res<WeaponRig>>,
    held: Option<Res<weapon::HandsRig>>,
    controller: Res<Controller>,
    parents: Query<&ChildOf>,
    transforms: Query<&Transform>,
    camera: Query<&Transform, With<MainCamera>>,
    mut out: Local<Option<std::io::BufWriter<std::fs::File>>>,
    mut frame: Local<u32>,
) {
    use std::io::Write as _;
    let (Some(path), Some(rig), Ok(camera)) = (&options.log_frames, rig, camera.single()) else { return };
    *frame += 1;
    if out.is_none() {
        match std::fs::File::create(path) {
            Ok(file) => *out = Some(std::io::BufWriter::new(file)),
            Err(e) => {
                warn!("{}: {e}", path.display());
                return;
            }
        }
    }
    // The rig and the camera both hang from the view pivot.
    let seen = |entity: Entity| {
        hands_view::in_root(entity, rig.pivot, |e| transforms.get(e).ok().copied(), &parents)
            .map(|t| camera.rotation.inverse() * (t.translation - camera.translation))
    };
    let show = |v: Option<Vec3>| v.map_or("-".to_owned(), |v| format!("{:.4} {:.4} {:.4}", v.x, v.y, v.z));
    let weapon = rig.weapon_root_anim.and_then(seen);
    let hands: Vec<String> = rig.arms.iter().map(|arm| show(seen(arm.palm()))).collect();
    // The clip on screen, where clips are played by name; in play, the
    // state the rig's animator is in or on its way into, how far through
    // the state it is in, and whether it is on a way between two.
    let state = held.as_deref().and_then(|h| h.animator()).map(|animator| animator.state());
    let (clip, at, blend) = match (player.by_name, &state) {
        (false, Some((state, at, next))) => (next.unwrap_or(state).trim_start_matches("Hands.").replace(' ', "_"), *at, next.is_some() as u8 as f32),
        _ => {
            let (name, at) = player.status().map_or(("-", 0.0), |(name, at, _)| (name, at));
            (name.to_owned(), at, player.blend())
        }
    };
    // The state the body's layer of gestures is in or entering, how far
    // through its state it is, and how much of the left hand is the
    // body's just now.
    let gesture = controller.movement.body.interaction_state().map_or(String::from("- 0 0"), |(state, at, next)| {
        let name = next.unwrap_or(state).trim_start_matches("InteractionsLayer.").replace(' ', "_");
        format!("{name} {at:.3} {:.3}", controller.movement.arm_pose(false).hand)
    });
    let Some(out) = out.as_mut() else { return };
    let _ = writeln!(
        out,
        "{} t {:.4} clip {clip} {at:.3} blend {blend:.2} weapon {} hands {} camera {:.4} {:.4} {:.4} {:.4} gesture {gesture}",
        *frame,
        time.elapsed_secs(),
        show(weapon),
        hands.join(" | "),
        camera.rotation.x,
        camera.rotation.y,
        camera.rotation.z,
        camera.rotation.w
    );
    let _ = out.flush();
}

#[derive(Component)]
pub struct MainCamera;

/// What moves of one of the range's things, by its place in `Range::things`.
#[derive(Component)]
struct ThingPart(usize);

/// Puts each thing's moving part where the thing stands now. The things
/// move on the simulation's frames (`Range::step_things`), and are drawn
/// as the last of them left them.
fn show_things(range: Res<range::Range>, mut parts: Query<(&ThingPart, &mut Transform)>) {
    for (part, mut transform) in &mut parts {
        if let Some((place, _, _)) = range.things.get(part.0).and_then(|placed| placed.part_shown()) {
            if *transform != place {
                *transform = place;
            }
        }
    }
}


fn setup_world(
    mut commands: Commands,
    options: Res<Options>,
    range: Res<range::Range>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<grid::GridMaterial>>,
) {
    // The warm-up's curtain, over everything, in the sky's colour.
    let curtain = Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() };
    commands.spawn((WarmupProp, curtain, BackgroundColor(Color::srgb(0.55, 0.63, 0.70)), GlobalZIndex(1000)));
    commands.spawn((
        MainCamera,
        Camera3d::default(),
        // The field of view follows the weapon motion every frame.
        // The near plane is the game camera's, as a recorded trace reports it.
        Projection::Perspective(PerspectiveProjection {
            fov: options.field_of_view.to_radians(),
            near: 0.03,
            ..default()
        }),
        Transform::from_xyz(0.0, 1.6, 0.0),
    ));
    commands.spawn((
        DirectionalLight { illuminance: 12_000.0, shadows_enabled: true, ..default() },
        // The light also sees what only casts a shadow in first person.
        bevy::render::view::RenderLayers::from_layers(&[body_view::SEEN, body_view::SHADOW_ONLY]),
        Transform::from_xyz(3.0, 8.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    // A plain test range: ground, posts every 5 m along a lane to judge
    // speed against, a ring of blocks to judge turning against, and two
    // walls to push the weapon into. The simulation feels the same boxes.
    // All of it carries the same grid of lines, to read distance, speed and
    // height by.
    let lined = |lines: grid::Lines| grid::GridLines { lines: if options.grid { lines } else { lines.none() } };
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(400.0, 400.0))),
        MeshMaterial3d(materials.add(grid::GridMaterial {
            base: StandardMaterial { base_color: Color::srgb(0.32, 0.34, 0.30), perceptual_roughness: 0.95, ..default() },
            extension: lined(grid::Lines::ground()),
        })),
    ));
    // One material for each colour, with lines or without, shared by every
    // block of it: each material holds six of the graphics card's places
    // for samplers, of which DirectX 12 has 2,048 for all that is drawn
    // (`docs/findings.md`), and one for each of the range's 67 blocks and
    // parts had taken 402 of them. The lines go by where a point is, so a
    // shared material draws each block as its own did.
    let mut made: std::collections::HashMap<([u8; 4], bool), Handle<grid::GridMaterial>> = std::collections::HashMap::new();
    let mut material = |color: Color, lines: bool| {
        let key = (color.to_srgba().to_u8_array(), lines);
        made.entry(key)
            .or_insert_with(|| {
                let extension = if lines { lined(grid::Lines::solid()) } else { lined(grid::Lines::solid().none()) };
                materials.add(grid::GridMaterial { base: StandardMaterial::from_color(color), extension })
            })
            .clone()
    };
    for solid in &range.solids {
        commands.spawn((Mesh3d(meshes.add(Cuboid::from_size(solid.size))), MeshMaterial3d(material(solid.color, true)), Transform::from_translation(solid.centre)));
    }
    // What moves of the things the hands work: a door's leaf, a lid, a
    // drawer. Plain, without the grid.
    for (index, placed) in range.things.iter().enumerate() {
        let Some((place, size, color)) = placed.part_shown() else { continue };
        commands.spawn((ThingPart(index), Mesh3d(meshes.add(Cuboid::from_size(size))), MeshMaterial3d(material(color, false)), place));
    }

}

/// Applies the starting view, and switches views: `F4` between playing and
/// the free camera, `F5` between playing and playing watched from outside
/// (from where the free camera stands, if it was flying; else from three
/// metres off).
fn switch_view(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    options: Res<Options>,
    rig: Option<Res<WeaponRig>>,
    controller: Res<Controller>,
    globals: Query<&GlobalTransform>,
    mut view: ResMut<View>,
    mut watching: ResMut<Watching>,
    mut camera: Query<(Entity, &mut Transform), With<MainCamera>>,
    mut frame: Local<u32>,
) {
    *frame += 1;
    // Global transforms of the new entities exist from the second frame.
    let starting = *frame == 2;
    let wanted = if starting {
        *view
    } else if keys.just_pressed(KeyCode::F4) {
        match *view {
            View::FirstPerson | View::Watched => View::Free,
            View::Free => View::FirstPerson,
        }
    } else if keys.just_pressed(KeyCode::F5) {
        match *view {
            View::FirstPerson | View::Free => View::Watched,
            View::Watched => View::FirstPerson,
        }
    } else {
        return;
    };
    let (Some(rig), Ok((camera, mut transform))) = (rig, camera.single_mut()) else { return };
    // A camera already outside stays where it is when the keys change
    // hands; one that leaves the character's eyes is put beside it.
    let leaving_the_eyes = starting || *view == View::FirstPerson;
    match wanted {
        View::FirstPerson => {
            // The camera hangs from the rig's root; its place and rotation
            // there are set every frame with the weapon motion.
            commands.entity(rig.pivot).add_child(camera);
            *transform = Transform::default();
        }
        View::Free | View::Watched => {
            if leaving_the_eyes {
                let root = globals.get(rig.root).map_or(GlobalTransform::IDENTITY, |g| *g);
                commands.entity(camera).remove::<ChildOf>();
                *transform = outside_view(&root, &options, options.whole || (wanted == View::Watched && !starting));
            }
        }
    }
    watching.0 = (wanted == View::Watched).then(|| transform.translation - controller.shown().position);
    // Whether playing has the mouse goes by the view (`cursor::hold`).
    *view = wanted;
}

/// Where the camera stands for a view from outside, given the weapon
/// rig's root (at the ribcage, turned to the view): beside a held rifle,
/// measured from the root so that animation does not move the viewpoint;
/// for the whole figure, three metres off to the front and left of where
/// the player faces, level, looking at the middle of the body; or, for a
/// plan of the range, high over the character looking straight down, with
/// the lane's way (ahead from the start) at the top.
fn outside_view(root: &GlobalTransform, options: &Options, whole: bool) -> Transform {
    if let Some(height) = options.map {
        let under = Vec3::new(root.translation().x, 0.0, root.translation().z - 2.0);
        return Transform::from_translation(under + Vec3::Y * height).looking_at(under, Vec3::NEG_Z);
    }
    if !whole {
        let (target, from) = (Vec3::new(0.13, -0.12, -0.42), Vec3::new(-0.62, 0.03, -0.67));
        return Transform::from_translation(root.transform_point(from)).looking_at(root.transform_point(target), Vec3::Y);
    }
    let ahead = root.forward();
    let facing = Quat::from_rotation_y(f32::atan2(-ahead.x, -ahead.z));
    // The middle of the body is about half the ribcage's height up, and
    // the camera is kept over the ground for a body that lies.
    let ribcage = root.translation();
    let middle = Vec3::new(ribcage.x, ribcage.y * 0.52, ribcage.z);
    Transform::from_translation(middle + facing * Vec3::new(-2.4, 0.35, -1.6)).looking_at(middle, Vec3::Y)
}

fn fly_camera(
    time: Res<Time>,
    view: Res<View>,
    options: Res<Options>,
    rig: Option<Res<WeaponRig>>,
    globals: Query<&GlobalTransform>,
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    controller: Res<Controller>,
    watching: Res<Watching>,
    mut camera: Query<&mut Transform, With<MainCamera>>,
) {
    let Ok(mut transform) = camera.single_mut() else { return };
    if *view == View::FirstPerson {
        return;
    }
    // An unattended capture from outside keeps its place beside the rig,
    // so that a player who has walked or crawled away is still in it.
    if let (true, Some(rig)) = (options.unattended(), &rig) {
        if let Ok(root) = globals.get(rig.root) {
            *transform = outside_view(root, &options, options.whole);
        }
        return;
    }
    // A played run watched from outside: the camera keeps its place from
    // the character's feet, turned as it was left.
    if *view == View::Watched {
        if let Some(from_feet) = watching.0 {
            transform.translation = controller.shown().position + from_feet;
        }
        return;
    }
    if !buttons.pressed(MouseButton::Right) {
        return;
    }
    let (mut yaw, mut pitch, _) = transform.rotation.to_euler(EulerRot::YXZ);
    yaw -= motion.delta.x * 0.003;
    pitch = (pitch - motion.delta.y * 0.003).clamp(-1.54, 1.54);
    transform.rotation = Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.0);

    let axis = |pos: KeyCode, neg: KeyCode| keys.pressed(pos) as i32 as f32 - keys.pressed(neg) as i32 as f32;
    let step = transform.right() * axis(KeyCode::KeyD, KeyCode::KeyA)
        + transform.up() * axis(KeyCode::KeyE, KeyCode::KeyQ)
        + transform.forward() * axis(KeyCode::KeyW, KeyCode::KeyS);
    let speed = if keys.pressed(KeyCode::ShiftLeft) { 4.0 } else { 0.6 };
    transform.translation += step * speed * time.delta_secs();
}

fn capture(
    mut commands: Commands,
    options: Res<Options>,
    controller: Res<Controller>,
    mut frame: Local<u32>,
    mut written: Local<Option<u32>>,
    mut exit: EventWriter<AppExit>,
) {
    let Some(path) = &options.screenshot else {
        // A run that plays a script and logs how it was drawn ends with
        // the script.
        if let (Some(scenario), true) = (&options.scenario, options.unattended()) {
            if controller.drawn_time() > options.scenario_at + scenario.duration + 0.25 {
                exit.write(AppExit::Success);
            }
        }
        return;
    };
    *frame += 1;
    // The default leaves time for textures to reach the GPU.
    if *frame == options.screenshot_frame {
        // Whatever an earlier run left under this name goes first, so
        // that the file being there means this run's picture is written.
        let _ = std::fs::remove_file(path);
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path.clone()));
    }
    // The picture is written some frames after it is asked for, and a
    // good many more while the renderer is still building what it draws
    // with: the run ends once the file is there.
    if *frame > options.screenshot_frame {
        match (path.exists(), *written) {
            (true, None) => *written = Some(*frame),
            (true, Some(at)) if *frame > at + 5 => {
                exit.write(AppExit::Success);
            }
            (false, _) if *frame > options.screenshot_frame + 1800 => {
                warn!("the picture was not written: {}", path.display());
                exit.write(AppExit::Success);
            }
            _ => {}
        }
    }
}
