//! Sets a recording made by hand (recorder 0.6.0 on, `free-*.jsonl`)
//! beside the logic of what the hands hold.
//!
//!   byhand <folder> [--from S] [--to S] [--trigger-flag] [part of a name ...]
//!
//! A recording by hand has no script, so the simulation cannot be walked
//! through it as the replay walks a scenario. What the hands hold can be:
//! what happened to it is read off the recording (the inventory opening
//! and shutting, a reload begun, the weapon put away or taken in hand) and
//! given to the weapon's logic on the frame the game shows it, with the
//! game's own frame times; the two animators are then set side by side on
//! every frame. What the body did (its gestures, its movement state, its
//! flags for the inventory) is listed as recorded, to be read.
//!
//! With `--from` and `--to` every frame between the two times is listed.
//!
//! Recorder 0.6.2 on: the weapon is the one the recording was made with,
//! and every change of what is carried, of what the container looked at
//! holds and of the bound keys is set beside the inventory model's own
//! move for the click, the drag or the key before it (`inventory_moves`).

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use trp_data::hard_settings::HardSettings;
use trp_data::unity::anim::path_hash;
use trp_data::unity::Assets;
use trp_data::{Database, Install};
use trp_sim::bindings::{code, Code, Controls, Preset};
use trp_sim::inventory::{self, Applied, Bound, Id, Item, Order, Rules, Spot};
use trp_sim::things::{Hinge, Thing, ThingSettings};
use trp_sim::weapon_animator::WeaponAnimatorData;
use trp_sim::{EmptyHands, FireMode, Firearm, FirearmEvent, FirearmSettings, Interaction};
use serde_json::Value as Json;

const AKM: &str = "59d6088586f774275f37482f";
/// The states a weapon is drawn through when it has to work its action
/// (a rifle's, and a shotgun's in each of its modes).
const ARMING_DRAW: [&str; 3] = ["OUT TO IDLE ARM", "PA OUT TO IDLE ARM", "SA OUT TO IDLE ARM"];
/// States that loop for as long as nothing happens: how far through one
/// the two sides are says nothing, since neither began it with the other.
const RESTING: [&str; 3] = ["IDLE", "IN INVENTORY", "SPAWN"];

/// What one recorded frame says of the hands and of the body.
struct Frame {
    t: f32,
    dt: f32,
    /// The player's whole-millisecond frame time (`clock_dt`), which the
    /// weapon's operations go by; the engine's where a recording has none.
    clock_dt: f32,
    controller: String,
    operation: String,
    anim: u32,
    anim_time: f32,
    inventory: bool,
    hands_inventory: bool,
    magazine: Option<u32>,
    chamber: bool,
    trigger: bool,
    pitch: f32,
    state: String,
    stance: f32,
    gesture: bool,
    gesture_index: u32,
    body_inventory: bool,
    body_operation: bool,
    /// Recorder 0.6.1 on: the keys that went down in the frame, the
    /// player's feet, and the door or container last looked at.
    keys: Vec<String>,
    pos: [f32; 3],
    thing: Option<Seen>,
    /// Recorder 0.6.2 on: the keys that came up, what was held with any
    /// key down or up, where the mouse was and how large the screen; what
    /// is carried, what the container looked at holds and what is bound
    /// to the quick-use keys, each on the frame it changed and on the
    /// first; the item in the hands; the weapon's fire mode, and its
    /// animator's hammer and bolt flags.
    keys_up: Vec<String>,
    /// Recorder 0.7.0 on: the engine's two mouse axes as read this frame,
    /// the head's free look as the game has it after the frame (its
    /// rotation, whether the mouse has it, whether it is looking), the
    /// view's yaw, and the sights and the sprint.
    mouse_axes: [f32; 2],
    head_rot: Option<[f32; 3]>,
    mouse_look: bool,
    head_looking: bool,
    yaw: f32,
    aiming: bool,
    sprinting: bool,
    held: Vec<String>,
    mouse: Option<[f32; 2]>,
    screen: Option<[f32; 2]>,
    equipment: Option<Json>,
    loot: Option<Json>,
    bound: Option<Vec<(String, String)>>,
    in_hands: Option<String>,
    fire_mode: String,
    armed: Option<bool>,
    bolt_caught: Option<bool>,
    /// How far through its state the body's animator is, whether it is
    /// between two, and whether the thing being worked puts the player at
    /// a place of its own (the game's word for this one interaction).
    body_time: f32,
    body_blending: bool,
    working_snap: Option<bool>,
}

/// The door, container or trunk last looked at, as one frame has it.
struct Seen {
    kind: String,
    id: String,
    state: String,
    angle: f32,
    angles: [f32; 2],
    axis: String,
    /// Where it is from the player's feet, and how it is turned, in the world.
    from_player: [f32; 3],
    rot: [f32; 4],
    /// The game's number for the gesture it asks of a player standing here.
    gesture: u8,
}

/// An animator state as the recorder writes it: the low 24 bits of the
/// hash of the state's own name.
fn number(name: &str) -> u32 {
    path_hash(name.rsplit('.').next().unwrap_or(name)) & 0x00ff_ffff
}

fn own(name: &str) -> &str {
    name.rsplit('.').next().unwrap_or(name)
}

fn read(path: &Path) -> Result<(Json, Vec<Frame>)> {
    let text = std::fs::read_to_string(path)?;
    let mut lines = text.lines().filter(|line| !line.trim().is_empty());
    let header: Json = serde_json::from_str(lines.next().context("empty trace")?)?;
    let mut frames = Vec::new();
    for line in lines {
        let v: Json = serde_json::from_str(line)?;
        let number = |name: &str| v.get(name).and_then(Json::as_f64).map(|x| x as f32);
        let flag = |name: &str| v.get(name).and_then(Json::as_bool).unwrap_or(false);
        let word = |name: &str| v.get(name).and_then(Json::as_str).unwrap_or("").to_owned();
        let floats = |name: &str| -> Vec<f32> {
            v.get(name).and_then(Json::as_array).map_or_else(Vec::new, |a| a.iter().filter_map(Json::as_f64).map(|x| x as f32).collect())
        };
        let three = |name: &str| {
            let list = floats(name);
            [0, 1, 2].map(|k| list.get(k).copied().unwrap_or(0.0))
        };
        let thing = v.get("thing").and_then(Json::as_str).map(|kind| Seen {
            kind: kind.to_owned(),
            id: word("thing_id"),
            state: word("thing_state"),
            angle: number("thing_angle").unwrap_or(0.0),
            angles: {
                let list = floats("thing_angles");
                [list.first().copied().unwrap_or(0.0), list.get(1).copied().unwrap_or(0.0)]
            },
            axis: word("thing_axis"),
            from_player: three("thing_from_player"),
            rot: {
                let list = floats("thing_rot");
                [0, 1, 2, 3].map(|k| list.get(k).copied().unwrap_or(if k == 3 { 1.0 } else { 0.0 }))
            },
            gesture: number("thing_gesture").unwrap_or(0.0) as u8,
        });
        let words = |name: &str| v.get(name).and_then(Json::as_array).map_or_else(Vec::new, |a| a.iter().filter_map(Json::as_str).map(str::to_owned).collect::<Vec<_>>());
        let keys = words("keys_down");
        let pair = |name: &str| {
            let list = floats(name);
            (list.len() == 2).then(|| [list[0], list[1]])
        };
        let bound = v.get("bound").and_then(Json::as_object).map(|map| {
            map.iter().filter_map(|(key, id)| Some((key.clone(), id.as_str()?.to_owned()))).collect::<Vec<_>>()
        });
        frames.push(Frame {
            keys_up: words("keys_up"),
            mouse_axes: pair("mouse_axes").unwrap_or_default(),
            head_rot: v.get("head_rot").map(|_| three("head_rot")),
            mouse_look: flag("mouse_look"),
            head_looking: flag("head_looking"),
            yaw: v.get("look").and_then(|l| l.get(0)).and_then(Json::as_f64).unwrap_or(0.0) as f32,
            aiming: flag("aiming"),
            sprinting: flag("sprinting"),
            held: words("held"),
            mouse: pair("mouse"),
            screen: pair("screen"),
            equipment: v.get("equipment").cloned(),
            loot: v.get("loot").cloned(),
            bound,
            in_hands: v.get("in_hands").and_then(Json::as_str).map(str::to_owned),
            fire_mode: word("fire_mode"),
            armed: number("hp_Armed").map(|value| value != 0.0),
            bolt_caught: number("hp_BoltCatch").map(|value| value != 0.0),
            body_time: number("body_anim_time").unwrap_or(0.0),
            body_blending: flag("body_blending"),
            working_snap: v.get("working_snap").and_then(Json::as_bool),
            t: number("t").unwrap_or(0.0),
            dt: number("dt").unwrap_or(0.0),
            clock_dt: number("clock_dt").or_else(|| number("dt")).unwrap_or(0.0),
            controller: word("hands_controller"),
            operation: word("operation"),
            anim: number("hands_anim").unwrap_or(0.0) as u32,
            anim_time: number("hands_anim_time").unwrap_or(0.0),
            inventory: flag("inventory_open"),
            hands_inventory: flag("hands_inventory"),
            magazine: number("magazine").map(|n| n as u32),
            chamber: number("chamber").unwrap_or(0.0) > 0.0,
            trigger: flag("trigger"),
            pitch: v.get("look").and_then(|l| l.get(1)).and_then(Json::as_f64).unwrap_or(0.0) as f32,
            state: word("state"),
            stance: number("pose").unwrap_or(1.0),
            gesture: flag("p_IsInteraction"),
            gesture_index: number("p_InteractionIndex").unwrap_or(0.0) as u32,
            body_inventory: flag("p_Inventory"),
            body_operation: flag("p_InventoryOperation"),
            keys,
            pos: three("pos"),
            thing,
        });
    }
    Ok((header, frames))
}

#[derive(Clone, Copy, PartialEq)]
enum Held {
    Weapon,
    Empty,
    Nothing,
}

fn held_by(controller: &str) -> Held {
    match controller {
        "FirearmController" => Held::Weapon,
        "EmptyHandsController" => Held::Empty,
        _ => Held::Nothing,
    }
}

fn main() -> Result<()> {
    let mut folder: Option<PathBuf> = None;
    let mut wanted: Vec<String> = Vec::new();
    let (mut from, mut to) = (f32::MAX, f32::MIN);
    // `--trigger-flag`: the trigger by the recorded flag even where the
    // keys are on record, as before 0.36.1.
    let mut trigger_flag = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--trigger-flag" => trigger_flag = true,
            "--from" => from = args.next().and_then(|v| v.parse().ok()).context("--from needs seconds")?,
            "--to" => to = args.next().and_then(|v| v.parse().ok()).context("--to needs seconds")?,
            _ if folder.is_none() => folder = Some(PathBuf::from(arg)),
            _ => wanted.push(arg),
        }
    }
    let folder = folder.context("which folder of recordings made by hand?")?;

    let install = Install::locate()?;
    let db = Database::load(&install)?;
    let hard = HardSettings::load(&install)?;
    let thing_settings = ThingSettings::new(&hard)?;
    let rules = Rules::of(&db);
    let panel = trp_data::screen::GearPanel::load(&install).unwrap_or_default();
    let controls = trp_sim::bindings::install_controls(&install);
    // What the engine's mouse axes are multiplied by for the free look and
    // for the turn (`docs/spec/free-look.md`, "The scale"): the bindings'
    // own figures and the player's sensitivity.
    let mouse_scale = MouseScale {
        look: [install.axis_sensitivity("LookX").unwrap_or(5.0), install.axis_sensitivity("LookY").unwrap_or(5.0)],
        turn: [install.axis_sensitivity("TurnX").unwrap_or(5.0), install.axis_sensitivity("TurnY").unwrap_or(5.0)],
        sensitivity: install.player_settings().mouse_sensitivity.unwrap_or(1.0),
    };
    let mut assets = Assets::new(install)?;
    let empty_rig = trp_data::weapon::assemble_prefab(&mut assets, trp_data::weapon::EMPTY_HANDS)?;
    let empty_data = WeaponAnimatorData::load(&assets, &empty_rig)?;
    // Each weapon a recording was made with, read once.
    let mut weapons: Vec<(String, WeaponAnimatorData, FirearmSettings)> = Vec::new();
    let (arming_draw, spawn) = (ARMING_DRAW.map(number), number("SPAWN"));

    let mut files: Vec<PathBuf> = std::fs::read_dir(&folder)
        .with_context(|| folder.display().to_string())?
        .filter_map(|e| Some(e.ok()?.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "jsonl"))
        .filter(|p| wanted.is_empty() || wanted.iter().any(|w| p.to_string_lossy().contains(w.as_str())))
        .collect();
    files.sort();

    for file in files {
        let (header, frames) = read(&file)?;
        let Some(first) = frames.first() else { continue };
        let stem = file.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        // The weapon it was made with: the one in hand as it begins, else
        // the first one taken in hand, else the first one built.
        let weapon_id = recorded_weapon(&header, &frames).unwrap_or_else(|| AKM.to_owned());
        if !weapons.iter().any(|(id, ..)| *id == weapon_id) {
            let weapon = trp_data::weapon::assemble(&mut assets, &db, &weapon_id)?;
            let data = WeaponAnimatorData::load(&assets, &weapon)?;
            let settings = trp_sim::firearm_settings(&assets, &db, &hard, &weapon)?;
            weapons.push((weapon_id.clone(), data, settings));
        }
        let (_, weapon_data, settings) = weapons.iter().find(|(id, ..)| *id == weapon_id).expect("just read");
        // A recorded state's number back to a name: the weapon's and the
        // empty hands' states share most of theirs.
        let mut names: Vec<(u32, String)> = Vec::new();
        for name in weapon_data.state_names().into_iter().chain(empty_data.state_names()) {
            if !names.iter().any(|(n, _)| *n == number(&name)) {
                names.push((number(&name), own(&name).to_owned()));
            }
        }
        let name_of = |n: u32| names.iter().find(|(known, _)| *known == n).map_or_else(|| n.to_string(), |(_, name)| name.clone());
        // The trigger by the keys where they are on record (recorder 0.6.0
        // on): the `Shoot` binding given and taken back, as the game's
        // input manager hears the keys. The recorded trigger flag misses a
        // single shot, which takes the press and lets it go in one frame.
        let shots = binding_frames(&controls, &frames, "Shoot").filter(|_| !trigger_flag);
        let chamber_looks = binding_frames(&controls, &frames, "CheckChamber");
        let mode_of = |name: &str| match name {
            "single" => Some(FireMode::Single),
            "fullauto" => Some(FireMode::FullAuto),
            "burst" => Some(FireMode::Burst),
            "semiauto" => Some(FireMode::SemiAuto),
            "doubleaction" => Some(FireMode::DoubleAction),
            "doublet" => Some(FireMode::Doublet),
            _ => None,
        };
        println!(
            "{stem}: {} frames, {:.2} s, recorder {}; begins with {} in {}",
            frames.len(),
            frames.last().map_or(0.0, |f| f.t),
            header.get("recorder").and_then(Json::as_str).unwrap_or("?"),
            first.controller,
            name_of(first.anim)
        );

        // What the character carried, the game's own sum beside the
        // model's (recorder 0.5.0 on).
        if let Some(list) = header.get("equipment") {
            let game = header.get("inventory_weight").and_then(Json::as_f64).unwrap_or(0.0);
            match trp_sim::inventory::Equipment::of_recording(&db, list) {
                Ok((equipment, compared, left_out)) => {
                    let off = compared.iter().filter(|item| !item.agrees()).count();
                    println!(
                        "  carried: {} items, {game:.3} kg in the game and {:.3} here ({:.3} and {:.3} at elite Strength); {off} differ in cells or weight, {} found no place",
                        compared.len(),
                        equipment.carried_weight(false),
                        header.get("inventory_weight_elite").and_then(Json::as_f64).unwrap_or(0.0),
                        equipment.carried_weight(true),
                        left_out.len()
                    );
                    for item in compared.iter().filter(|item| !item.agrees()).take(6) {
                        println!(
                            "    {}: {} x {} cells and {:.3} kg in the game, {} x {} and {:.3} here",
                            item.name, item.cells[0][0], item.cells[0][1], item.weight[0], item.cells[1][0], item.cells[1][1], item.weight[1]
                        );
                    }
                    for entry in left_out.iter().take(6) {
                        println!("    no place for {entry}");
                    }
                }
                Err(e) => println!("  the recorded equipment could not be put together: {e:#}"),
            }
        }

        // What the body did, as recorded.
        println!("  the body, as recorded:");
        let mut gesture_began = 0.0;
        for pair in frames.windows(2) {
            let (a, b) = (&pair[0], &pair[1]);
            if b.gesture && !a.gesture {
                gesture_began = b.t;
                println!("    {:7.3}  gesture {} begins (movement state {}, stance {:.2})", b.t, b.gesture_index, b.state, b.stance);
            }
            if !b.gesture && a.gesture {
                println!("    {:7.3}  gesture {} over, {:.3} s after it began", b.t, a.gesture_index, b.t - gesture_began);
            }
            if b.state != a.state {
                println!("    {:7.3}  movement state {} (was {}), stance {:.2}, view {:.1} down", b.t, b.state, a.state, b.stance, b.pitch);
            }
            if b.inventory != a.inventory {
                let after = if b.inventory && gesture_began > 0.0 { format!(", {:.3} s after the last gesture began", b.t - gesture_began) } else { String::new() };
                println!("    {:7.3}  the inventory {}{after}", b.t, if b.inventory { "opens" } else { "shuts" });
            }
            if b.hands_inventory != a.hands_inventory {
                println!("    {:7.3}  the hands' animator is told the inventory is {}", b.t, if b.hands_inventory { "open" } else { "shut" });
            }
            if b.body_inventory != a.body_inventory {
                println!("    {:7.3}  the body's Inventory flag {}", b.t, if b.body_inventory { "up" } else { "down" });
            }
            if b.body_operation != a.body_operation {
                println!("    {:7.3}  the body's InventoryOperation flag {}", b.t, if b.body_operation { "up" } else { "down" });
            }
            if b.controller != a.controller {
                println!("    {:7.3}  the hands hold: {} (operation {})", b.t, b.controller, if b.operation.is_empty() { "-" } else { &b.operation });
            } else if b.operation != a.operation {
                println!("    {:7.3}  the weapon's operation: {} (magazine {:?}, chamber {})", b.t, b.operation, b.magazine, b.chamber as u8);
            }
        }

        // The hands' animator, the game's beside the simulation's.
        let mut firearm = Firearm::new(settings.clone(), weapon_data);
        let mut empty = EmptyHands::new(&empty_data);
        let mut held = held_by(&first.controller);
        // The rounds within reach, for a tube filled in the recording.
        let reach = |list: Option<&Json>| -> u32 {
            let entries = list.and_then(Json::as_array).map(Vec::as_slice).unwrap_or_default();
            let loose = entries.iter().filter(|e| {
                let at = e.get("at").and_then(Json::as_str).unwrap_or("");
                let kind = e.get("tpl").and_then(Json::as_str).unwrap_or("");
                db.class_names(kind).iter().any(|class| class == "Ammo") && at != "cartridges" && at != "patron_in_weapon"
            });
            loose.filter_map(|e| e.get("n").and_then(Json::as_u64)).sum::<u64>() as u32
        };
        let mut carried_now = header.get("equipment").cloned();
        match held {
            Held::Weapon => {
                firearm.chamber = first.chamber;
                firearm.magazine = first.magazine;
                if let Some(mode) = mode_of(&first.fire_mode) {
                    firearm.start_in_mode(mode);
                }
                firearm.start_drawn();
                firearm.start_with(first.armed, first.bolt_caught);
                if first.inventory {
                    firearm.set_inventory(true);
                    // A recording begun with the hands already down for
                    // the screen: the simulation's are taken down before
                    // the first frame is set beside the game's.
                    if name_of(first.anim) == "IN INVENTORY" {
                        for _ in 0..360 {
                            if firearm.animator().rests_in("Hands.IN INVENTORY") {
                                break;
                            }
                            firearm.step(1.0 / 120.0);
                        }
                    }
                }
            }
            Held::Empty => {
                empty.draw();
                for _ in 0..120 {
                    empty.step(1.0 / 120.0, first.pitch);
                }
                empty.set_inventory(first.inventory);
            }
            Held::Nothing => {}
        }
        let (mut game_entered, mut sim_entered): (Vec<(f32, u32)>, Vec<(f32, u32)>) = (Vec::new(), Vec::new());
        let (mut compared, mut differing, mut worst_time, mut worst_at) = (0usize, 0usize, 0.0f32, 0.0f32);
        let mut runs: Vec<(f32, f32, u32, u32)> = Vec::new();
        let mut said: Vec<String> = Vec::new();
        let mut last_sim = None;
        for (i, cur) in frames.iter().enumerate() {
            let prev = i.checked_sub(1).map(|p| &frames[p]);
            if cur.equipment.is_some() {
                carried_now = cur.equipment.clone();
            }
            // Whether what is held has an update of its animator in this
            // frame: all but something made after the frame's update.
            let mut updated = true;
            if let Some(prev) = prev {
                if cur.controller != prev.controller {
                    // The weapon's last frame: its put-away's last mark
                    // is what brought the next thing into the hands.
                    if held == Held::Weapon && firearm.in_hand() {
                        if firearm.step(cur.dt).contains(&FirearmEvent::Hidden) {
                            said.push(format!("{:.3}: the simulation's weapon is out of the hands on the frame the game's next controller shows", cur.t));
                        } else {
                            said.push(format!("{:.3}: the game's next controller shows while the simulation's weapon is still in hand", cur.t));
                        }
                    }
                    held = held_by(&cur.controller);
                    // A controller made after the frame's update of the
                    // animators shows its first state untouched, and has
                    // its first update in the next frame; one made before
                    // it (by the screen, say) has it in this one.
                    updated = !(cur.anim == spawn && cur.anim_time == 0.0);
                    match held {
                        Held::Weapon => {
                            empty.hide();
                            firearm = Firearm::new(settings.clone(), weapon_data);
                            // The rounds as they were before the draw's own
                            // chambering, which the game has done by now.
                            let arms = frames[i..].iter().take(30).any(|f| arming_draw.contains(&f.anim));
                            firearm.chamber = cur.chamber && !arms;
                            firearm.magazine = cur.magazine.map(|n| n + arms as u32);
                            if let Some(mode) = mode_of(&cur.fire_mode) {
                                firearm.start_in_mode(mode);
                            }
                            firearm.draw();
                            if cur.operation != "SpawnOperation" {
                                said.push(format!("{:.3}: the game's weapon is in {} on the first frame its controller shows, its draw cut short", cur.t, cur.operation));
                            }
                        }
                        Held::Empty => empty.draw(),
                        Held::Nothing => {}
                    }
                } else if held == Held::Weapon && cur.operation != prev.operation {
                    match cur.operation.as_str() {
                        // The recorded count is the new magazine's after a
                        // round has gone to an empty chamber.
                        // The quick reload is the same operation: the game's
                        // going on to `RELOAD OUT ALL` says it (`R` is not
                        // among the keys on record).
                        "ReloadExternalMagOperation" => {
                            let rounds = cur.magazine.unwrap_or(0) + (cur.chamber && !prev.chamber) as u32;
                            if frames[i..].iter().take(30).any(|f| name_of(f.anim) == "RELOAD OUT ALL") {
                                firearm.quick_reload_with(rounds)
                            } else {
                                firearm.reload_with(rounds)
                            }
                        }
                        // A tube filled from what was carried the frame
                        // before (an empty chamber has taken its round by
                        // now); as the app does, the same for every kind of
                        // magazine that stays in the weapon: the SKS's
                        // through its open bolt, barrels, a cylinder.
                        "ReloadInternalMagOperation" | "ReloadInternalMagWithOpenBoltOperation" | "ReloadMultiBarrelOperation" | "ReloadCylinderMagOperation" => {
                            firearm.chamber = prev.chamber;
                            firearm.reload_shells(reach(carried_now.as_ref()) + (cur.chamber && !prev.chamber) as u32)
                        }
                        "Remove" => firearm.hide(false),
                        "FoldStockOperation" => {
                            firearm.fold_stock();
                        }
                        // Which look it is shows in the state the game's
                        // animator goes to.
                        "UtilityOperation" => {
                            // Which look: the chamber's by its key where it
                            // is on record (within the half second its double
                            // press may take: not every key is on record);
                            // else by the first look's state the game goes to
                            // within half a second (which may wait for the
                            // screen to shut): `LOOK` the look all over,
                            // `FIREMODE CHECK` the selector, `CHECK CHAMBER`
                            // the chamber, `CHECK` the magazine's, or the
                            // chamber's where the weapon has barrels.
                            let keyed = chamber_looks.as_ref().is_some_and(|looks| looks.iter().any(|(at, given)| *given && *at <= i && cur.t - frames[*at].t <= 0.5));
                            let look = frames[i..].iter().take(60).map(|f| name_of(f.anim)).find(|name| matches!(name.as_str(), "LOOK" | "FIREMODE CHECK" | "CHECK CHAMBER" | "CHECK"));
                            match look.as_deref() {
                                _ if keyed => firearm.check_chamber(),
                                Some("LOOK") => {
                                    firearm.examine();
                                }
                                Some("FIREMODE CHECK") => {
                                    firearm.check_fire_mode();
                                }
                                Some("CHECK CHAMBER") => firearm.check_chamber(),
                                Some("CHECK") if settings.barrels > 0 => firearm.check_chamber(),
                                _ => _ = firearm.check_magazine(),
                            }
                        }
                        _ => {}
                    }
                }
                match &shots {
                    // With a screen open the key is the screen's, not the
                    // weapon's: a press is not handed on (its letting go
                    // may be).
                    Some(shots) => {
                        for (_, pressed) in shots.iter().filter(|(at, _)| *at == i) {
                            if held == Held::Weapon && !(*pressed && (cur.inventory || prev.inventory)) {
                                firearm.set_trigger(*pressed, true);
                            }
                        }
                    }
                    None if held == Held::Weapon && cur.trigger != prev.trigger => firearm.set_trigger(cur.trigger, true),
                    None => {}
                }
                if let Some(mode) = mode_of(&cur.fire_mode).filter(|_| held == Held::Weapon && cur.fire_mode != prev.fire_mode && cur.controller == prev.controller) {
                    firearm.set_fire_mode(mode);
                }
                // The screen opens and shuts before the frame's update of
                // the animators: the game's is blending on that frame.
                if prev.inventory != cur.inventory {
                    match held {
                        Held::Weapon => firearm.set_inventory(cur.inventory),
                        Held::Empty => empty.set_inventory(cur.inventory),
                        Held::Nothing => {}
                    }
                }
            }
            let events = match held {
                _ if !updated => Vec::new(),
                Held::Weapon => {
                    firearm.set_aim_angle(cur.pitch);
                    firearm.step_on_clock(cur.dt, cur.clock_dt)
                }
                Held::Empty => empty.step(cur.dt, cur.pitch),
                Held::Nothing => Vec::new(),
            };

            for event in &events {
                match event {
                    FirearmEvent::Appeared => said.push(format!("{:.3}: the simulation's {} appeared", cur.t, if held == Held::Weapon { "weapon has" } else { "empty hands have" })),
                    FirearmEvent::Hidden => said.push(format!("{:.3}: the simulation's weapon is out of the hands", cur.t)),
                    FirearmEvent::MagazineOut => said.push(format!("{:.3}: the simulation's magazine is out", cur.t)),
                    FirearmEvent::MagazineIn => said.push(format!("{:.3}: the simulation's magazine is in", cur.t)),
                    _ => {}
                }
            }
            // What has just appeared is told of the screen afresh: the
            // game's owner does it when the hands have changed.
            if events.contains(&FirearmEvent::Appeared) {
                match held {
                    Held::Weapon => firearm.set_inventory(cur.inventory),
                    Held::Empty => empty.set_inventory(cur.inventory),
                    Held::Nothing => {}
                }
            }

            let (state, time) = match held {
                Held::Weapon => (number(firearm.animator().state().0), firearm.animator().state().1),
                Held::Empty => (number(empty.animator().state().0), empty.animator().state().1),
                Held::Nothing => (0, 0.0),
            };
            if prev.is_none_or(|prev| prev.anim != cur.anim || prev.controller != cur.controller) {
                game_entered.push((cur.t, cur.anim));
            }
            if last_sim != Some((state, held == Held::Weapon)) {
                sim_entered.push((cur.t, state));
                last_sim = Some((state, held == Held::Weapon));
            }
            compared += 1;
            if state != cur.anim {
                differing += 1;
                match runs.last_mut() {
                    Some(run) if run.2 == cur.anim && run.3 == state && i > 0 && run.1 == frames[i - 1].t => run.1 = cur.t,
                    _ => runs.push((cur.t, cur.t, cur.anim, state)),
                }
            } else if !RESTING.contains(&name_of(state).as_str()) && (time - cur.anim_time).abs() > worst_time {
                (worst_time, worst_at) = ((time - cur.anim_time).abs(), cur.t);
            }
            if cur.t >= from && cur.t <= to {
                println!(
                    "    {:7.3} dt {:.4}  game {:<22} {:8.4}  sim {:<22} {:8.4}  screen {} told {}  game {} {:?}+{}  sim {} {:?}+{}",
                    cur.t,
                    cur.dt,
                    name_of(cur.anim),
                    cur.anim_time,
                    name_of(state),
                    time,
                    cur.inventory as u8,
                    cur.hands_inventory as u8,
                    cur.operation,
                    cur.magazine,
                    cur.chamber as u8,
                    firearm.operation_said(),
                    firearm.magazine,
                    firearm.chamber as u8
                );
            }
        }
        println!("  the hands' animator, the game's states as they are entered beside the simulation's:");
        for row in 0..game_entered.len().max(sim_entered.len()) {
            let game = game_entered.get(row).map_or_else(String::new, |(t, n)| format!("{t:7.3} {}", name_of(*n)));
            let sim = sim_entered.get(row).map_or_else(String::new, |(t, n)| format!("{t:7.3} {}", name_of(*n)));
            let late = match (game_entered.get(row), sim_entered.get(row)) {
                (Some((a, x)), Some((b, y))) if x == y => format!("{:+.3} s", b - a),
                (Some(_), Some(_)) => "another state".to_owned(),
                _ => String::new(),
            };
            println!("    {game:<32} {sim:<32} {late}");
        }
        println!(
            "  the state differs on {differing} of {compared} frames; where it agrees the time through it is at most {worst_time:.4} of a cycle apart (at {worst_at:.3} s)"
        );
        for (from, to, game, sim) in runs.iter().take(12) {
            println!("    {from:7.3} to {to:7.3}: the game in {}, the simulation in {}", name_of(*game), name_of(*sim));
        }
        for line in &said {
            println!("  {line}");
        }
        things_worked(&frames, &thing_settings);
        inventory_moves(&db, &rules, &panel, &header, &frames);
        keys_heard(&controls, &frames);
        free_look(&controls, &frames, &mouse_scale, (from, to));
    }
    Ok(())
}

/// What the engine's mouse axes are multiplied by: the look's bindings
/// and the turn's (across, then down), and the player's sensitivity.
struct MouseScale {
    look: [f32; 2],
    turn: [f32; 2],
    sensitivity: f32,
}

/// The head's free look set beside the game's (recorder 0.7.0 on): the
/// recorded keys through the game's input manager as `keys_heard` puts
/// them, the recorded mouse axes given to the look or to the turn by it
/// (`Preset::mouse`), the look's share handed to the head's own logic on
/// each frame the keys reach the character, and its rotation, its hold on
/// the mouse and its looking flag set beside the recorded ones. The turn's
/// share is set beside the view's own turn on that frame: whether it
/// turned, and by how much. The engine's mouse axes are positive to the
/// right and up; the look's binding across takes them as they come and
/// the one up and down the other way about (the install's `Control.ini`).
fn free_look(controls: &Controls, frames: &[Frame], scale: &MouseScale, (from, to): (f32, f32)) {
    use trp_sim::head_look::{HeadLook, HeadLookInput, HeadLookSettings};
    if frames.iter().all(|frame| frame.head_rot.is_none_or(|rot| rot == [0.0; 3])) {
        return;
    }
    let mut preset = Preset::new(controls);
    let mut held: Vec<Code> = Vec::new();
    let mut head = HeadLook::new(HeadLookSettings::default());
    let look_group = controls.keys.iter().position(|binding| binding.name == trp_sim::bindings::LOOK_BINDING);
    let (mut compared, mut worst, mut worst_at, mut off_frames) = (0usize, 0.0f32, 0.0f32, 0usize);
    let (mut control_differs, mut looking_differs) = (0usize, 0usize);
    // The turn: frames' axes the view turned on and the model gave the
    // turn travel, the one without the other each way, and the largest
    // difference in degrees where both turned.
    let (mut turned_both, mut turned_game, mut turned_model, mut turn_worst) = (0usize, 0usize, 0usize, 0.0f32);
    // The frames' axes the mouse moved along with the free look's keys
    // held, and of those how many gave the look nothing.
    let (mut moved_held, mut lost) = (0usize, 0usize);
    let mut presses = 0usize;
    let mut was: Option<[f32; 2]> = None;
    for (at, frame) in frames.iter().enumerate() {
        held.extend(frame.keys.iter().filter_map(|key| code(key)));
        held.retain(|key| !frame.keys_up.iter().any(|up| code(up) == Some(*key)));
        let heard = preset.update(&|key| held.contains(&key), 0, frame.dt);
        let moved = frame.mouse_axes;
        let mouse = preset.mouse(moved);
        let reset = look_group.is_some_and(|group| heard.given.contains(&group));
        presses += frame.keys.iter().filter(|key| key.as_str() == "Mouse2").count();
        let look_held = held.iter().any(|key| Some(*key) == code("Mouse2"));
        for axis in 0..2 {
            if look_held && moved[axis] != 0.0 {
                moved_held += 1;
                lost += usize::from(mouse.look[axis] == 0.0);
            }
        }
        // Each frame the keys reach the character: none with a screen open.
        if !frame.inventory {
            if reset {
                head.reset();
            }
            let pitch_before = at.checked_sub(1).map_or(frame.pitch, |before| frames[before].pitch);
            head.frame(&HeadLookInput {
                travel: [mouse.look[0] * scale.look[0], -mouse.look[1] * scale.look[1]],
                dt: frame.dt,
                aiming: frame.aiming,
                camera_pitch: pitch_before,
                sprinting: frame.sprinting,
            });
        }
        // The view's own turn on this frame beside the model's share of
        // the mouse for it (the yaw kept within half a turn).
        let now = [frame.yaw, frame.pitch];
        if let Some(before) = was {
            let mut turned = [now[0] - before[0], now[1] - before[1]];
            turned[0] -= 360.0 * (turned[0] / 360.0).round();
            let asked = [mouse.turn[0] * scale.turn[0] * scale.sensitivity, -mouse.turn[1] * scale.turn[1] * scale.sensitivity];
            if !frame.inventory && !frame.aiming {
                for axis in 0..2 {
                    match (turned[axis].abs() > 1e-4, asked[axis] != 0.0) {
                        (true, true) => {
                            turned_both += 1;
                            turn_worst = turn_worst.max((turned[axis] - asked[axis]).abs());
                        }
                        (true, false) => turned_game += 1,
                        (false, true) => turned_model += 1,
                        (false, false) => {}
                    }
                }
            }
        }
        was = Some(now);
        let Some(game) = frame.head_rot else { continue };
        let model = head.rotation();
        let off = (0..2).map(|k| (game[k] - model[k]).abs()).fold(0.0, f32::max);
        compared += 1;
        off_frames += usize::from(off > 0.01);
        if off > worst {
            (worst, worst_at) = (off, frame.t);
        }
        control_differs += usize::from(head.control() != frame.mouse_look);
        looking_differs += usize::from(head.looking() != frame.head_looking);
        if (from..=to).contains(&frame.t) {
            println!(
                "  look {at:5} {:7.3} axes {:+.3} {:+.3} look {:+.3} {:+.3} turn {:+.3} {:+.3}{} | head {:+8.3} {:+8.3} here {:+8.3} {:+8.3} | mouse {} {} looking {} {}",
                frame.t,
                moved[0],
                moved[1],
                mouse.look[0],
                mouse.look[1],
                mouse.turn[0],
                mouse.turn[1],
                if reset { " RESET" } else { "" },
                game[0],
                game[1],
                model[0],
                model[1],
                u8::from(frame.mouse_look),
                u8::from(head.control()),
                u8::from(frame.head_looking),
                u8::from(head.looking()),
            );
        }
    }
    println!(
        "  free look: {presses} presses of its key; of {moved_held} frames' travel along an axis with the key held, {lost} gave the look nothing; the head's rotation on {compared} frames: {off_frames} differ by more than 0.01 degrees, at most {worst:.4} (at {worst_at:.3} s); the mouse's hold differs on {control_differs}, the looking flag on {looking_differs}"
    );
    println!(
        "  the turn beside the view's: both turned on {turned_both} frames' axes (at most {turn_worst:.4} degrees apart), the game alone on {turned_game}, the model alone on {turned_model} (sights down, no screen open)"
    );
}

/// The weapon a recording was made with: what its first line names, else
/// the first item a weapon's controller had in the hands, by the list of
/// what was carried on that frame.
/// The frames a binding is given (true) and taken back (false) on, the
/// recorded keys put through the game's input manager as `keys_heard` puts
/// them; none where no key is on record.
fn binding_frames(controls: &Controls, frames: &[Frame], wanted: &str) -> Option<Vec<(usize, bool)>> {
    if frames.iter().all(|frame| frame.keys.is_empty() && frame.keys_up.is_empty()) {
        return None;
    }
    let mut preset = Preset::new(controls);
    let mut held: Vec<Code> = Vec::new();
    let mut out = Vec::new();
    for (at, frame) in frames.iter().enumerate() {
        held.extend(frame.keys.iter().filter_map(|key| code(key)));
        held.retain(|key| !frame.keys_up.iter().any(|up| code(up) == Some(*key)));
        let heard = preset.update(&|key| held.contains(&key), 0, frame.dt);
        for (group, given) in heard.given.iter().map(|group| (*group, true)).chain(heard.taken_back.iter().map(|group| (*group, false))) {
            if controls.keys[group].name == wanted {
                out.push((at, given));
            }
        }
    }
    Some(out)
}

/// The recorded keys put through the game's input manager as it is written
/// here (`trp_sim::bindings`, settings spec), by the install's own
/// bindings and each frame's own length: every binding it gives out is
/// listed with how long after its first key went down it came, and for
/// the three whose doing shows in a recording on the frame it is done (a
/// reload begun, the inventory opened or shut, the fire mode changed) the
/// frame the game did it on is set beside the model's.
///
/// The wheel is not on record, so a binding with a notch in it is never
/// given here; the bindings are the install's as it stands, which may not
/// be what it had when the recording was made.
fn keys_heard(controls: &Controls, frames: &[Frame]) {
    if frames.iter().all(|frame| frame.keys.is_empty() && frame.keys_up.is_empty()) {
        return;
    }
    let mut preset = Preset::new(controls);
    let mut held: Vec<Code> = Vec::new();
    // When each key last went down, and the frames the game did each of
    // the three things on.
    let mut went_down: Vec<(Code, f32)> = Vec::new();
    let did = |what: &dyn Fn(&Frame, &Frame) -> bool| -> Vec<usize> { (1..frames.len()).filter(|&at| what(&frames[at - 1], &frames[at])).collect() };
    let reloads = did(&|was, is| !was.operation.starts_with("Reload") && is.operation.starts_with("Reload"));
    let screens = did(&|was, is| was.inventory != is.inventory);
    let modes = did(&|was, is| was.fire_mode != is.fire_mode);
    let (mut same, mut other) = (0, 0);
    for (at, frame) in frames.iter().enumerate() {
        for key in frame.keys.iter().filter_map(|key| code(key)) {
            held.push(key);
            went_down.retain(|(known, _)| *known != key);
            went_down.push((key, frame.t));
        }
        held.retain(|key| !frame.keys_up.iter().any(|up| code(up) == Some(*key)));
        let heard = preset.update(&|key| held.contains(&key), 0, frame.dt);
        for (group, given) in heard.given.iter().map(|group| (*group, true)).chain(heard.taken_back.iter().map(|group| (*group, false))) {
            let binding = &controls.keys[group];
            let since = binding
                .variants
                .iter()
                .flat_map(|variant| &variant.keys)
                .filter_map(|key| went_down.iter().find(|(known, _)| known == key))
                .map(|(_, t)| frame.t - t)
                .fold(f32::MAX, f32::min);
            let after = if since == f32::MAX { String::new() } else { format!(", {since:.3} s after its key went down") };
            let recorded = match (binding.name.as_str(), given) {
                ("ReloadWeapon", true) => Some(("a reload begun", &reloads)),
                ("Inventory", true) => Some(("the inventory opened or shut", &screens)),
                ("ShootingMode", true) => Some(("the fire mode changed", &modes)),
                _ => None,
            };
            let beside = recorded.and_then(|(what, frames_done)| {
                // The game's frame nearest the model's, within half a second.
                let near = frames_done.iter().copied().min_by_key(|done| done.abs_diff(at))?;
                ((frames[near].t - frame.t).abs() < 0.5).then(|| {
                    if near == at {
                        same += 1;
                        format!("; {what} on that frame in the game")
                    } else {
                        other += 1;
                        format!("; {what} on frame {near} in the game ({:+} frames, {:+.3} s)", near as i64 - at as i64, frames[near].t - frame.t)
                    }
                })
            });
            println!(
                "  keys: {}{} on frame {at} ({:.3} s){after}{}",
                if given { "" } else { "taken back: " },
                binding.name,
                frame.t,
                beside.unwrap_or_default()
            );
        }
    }
    if same + other > 0 {
        println!("  keys: of {} bindings whose doing is on record, {same} came on the game's own frame", same + other);
    }
}

fn recorded_weapon(header: &Json, frames: &[Frame]) -> Option<String> {
    if let Some(named) = header.get("weapon").and_then(Json::as_str).filter(|id| !id.is_empty()) {
        return Some(named.to_owned());
    }
    let mut carried = header.get("equipment");
    let mut in_hands: Option<&str> = None;
    for frame in frames {
        carried = frame.equipment.as_ref().or(carried);
        in_hands = frame.in_hands.as_deref().or(in_hands);
        if frame.controller == "FirearmController" {
            let entries = carried?.as_array()?;
            let held = entries.iter().find(|entry| entry.get("id").and_then(Json::as_str) == in_hands)?;
            return held.get("tpl").and_then(Json::as_str).map(str::to_owned);
        }
    }
    None
}

/// One entry of a recorded list of items.
#[derive(Clone, PartialEq)]
struct Entry {
    id: String,
    template: String,
    holder: String,
    at: String,
    cell: Option<(u32, u32, bool)>,
    count: u32,
}

impl Entry {
    /// Where it lies, as the lists write it: the last four signs of what
    /// holds it, the slot or grid, and the cell.
    fn place(&self) -> String {
        place_text(&self.holder, &self.at, self.cell)
    }
}

fn tail(id: &str) -> &str {
    &id[id.len().saturating_sub(4)..]
}

fn place_text(holder: &str, at: &str, cell: Option<(u32, u32, bool)>) -> String {
    let cell = cell.map_or_else(String::new, |(x, y, turned)| format!("[{x},{y}{}]", if turned { " turned" } else { "" }));
    format!("{}/{at}{cell}", tail(holder))
}

fn entries(list: &Json) -> Vec<Entry> {
    let text = |entry: &Json, name: &str| entry.get(name).and_then(Json::as_str).unwrap_or("").to_owned();
    let whole = |entry: &Json, name: &str| entry.get(name).and_then(Json::as_u64).map(|v| v as u32);
    list.as_array().map_or_else(Vec::new, |list| {
        list.iter()
            .map(|entry| Entry {
                id: text(entry, "id"),
                template: text(entry, "tpl"),
                holder: text(entry, "in"),
                at: text(entry, "at"),
                cell: whole(entry, "x").zip(whole(entry, "y")).map(|(x, y)| (x, y, whole(entry, "r") == Some(1))),
                count: whole(entry, "n").unwrap_or(1),
            })
            .collect()
    })
}

/// What the mouse or a key did last before a change of the lists.
#[derive(Clone, PartialEq)]
enum Gesture {
    /// The left button down and up on one spot, with what was held.
    Click { held: Vec<String> },
    /// The left button let go somewhere else than it went down.
    Drag { held: Vec<String>, to: [f32; 2], screen: [f32; 2] },
    /// A quick-use key coming up: which of the seven.
    QuickKey(usize),
    /// The key that works what is looked at.
    Interact,
}

/// The model of what was carried and of the container looked at, put
/// together from two recorded lists, with each recorded id's own number.
struct Model {
    equipment: Item,
    loot: Option<Item>,
    ids: Vec<(String, Id)>,
}

impl Model {
    fn of(db: &Database, carried: &Json, loot: Option<&Json>) -> Result<Self> {
        let (equipment, mut ids, _) = Item::of_recording(db, carried)?;
        let loot = match loot {
            Some(list) => {
                let (item, more, _) = Item::of_recording(db, list)?;
                ids.extend(more);
                Some(item)
            }
            None => None,
        };
        Ok(Self { equipment, loot, ids })
    }

    fn id(&self, recorded: &str) -> Option<Id> {
        self.ids.iter().find(|(id, _)| id == recorded).map(|(_, id)| *id)
    }

    fn recorded(&self, id: Id) -> &str {
        self.ids.iter().find(|(_, own)| *own == id).map_or("????", |(recorded, _)| recorded.as_str())
    }

    fn find(&self, id: Id) -> Option<&Item> {
        self.equipment.find(id).or_else(|| self.loot.as_ref().and_then(|loot| loot.find(id)))
    }

    /// Where an item lies in the model, written as a recorded entry's place.
    fn place(&self, id: Id) -> String {
        let held = self.equipment.holder_of(id).or_else(|| self.loot.as_ref().and_then(|loot| loot.holder_of(id)));
        let Some((holder, spot)) = held else { return "nowhere".to_owned() };
        let Some(in_item) = self.find(holder) else { return "nowhere".to_owned() };
        let (at, cell) = match spot {
            Spot::Slot(slot) => (in_item.kind.slots[slot].name.clone(), None),
            Spot::Cell(grid, place) => (in_item.kind.grids[grid].name.clone(), Some((place.x, place.y, place.turned))),
            Spot::Loose => ("cartridges".to_owned(), None),
        };
        place_text(self.recorded(holder), &at, cell)
    }

    /// The model's own things as the moves take them: the equipment, and
    /// the container looked at if there is one.
    fn roots(&mut self) -> Vec<&mut Item> {
        let mut roots = vec![&mut self.equipment];
        roots.extend(self.loot.as_mut());
        roots
    }

    fn clone_of(&self) -> Self {
        Self { equipment: self.equipment.clone(), loot: self.loot.clone(), ids: self.ids.clone() }
    }
}

/// The quick-use keys in the game's order, by the recorder's names for them.
const BOUND_KEYS: [&str; 7] = ["Item4", "Item5", "Item6", "Item7", "Item8", "Item9", "Item10"];
/// And by the engine's names for the keys themselves.
const QUICK_KEYS: [&str; 7] = ["Alpha4", "Alpha5", "Alpha6", "Alpha7", "Alpha8", "Alpha9", "Alpha0"];
/// How far the mouse may move between the button going down and coming up
/// for the two to be a click, in pixels (the engine's drag threshold).
const CLICK_REACH: f32 = 10.0;
/// Two clicks so close together are a double click, in seconds.
const DOUBLE_CLICK: f32 = 0.3;

/// Every change of what is carried, of what the container looked at holds
/// and of the bound keys (recorder 0.6.2 on), each set beside the
/// inventory model's own move for the click, the drag or the key before
/// it. The model is put together afresh from the lists as they stood
/// before the change; the thing the mouse was on is not on record, so it
/// is taken from the change itself (the thing that moved), and the
/// model is asked where *it* would send that thing by the same click. A
/// window opened by a double click is not on record either: after one,
/// the model's answer is given for each container whose window it could
/// have been.
fn inventory_moves(db: &Database, rules: &Rules, panel: &trp_data::screen::GearPanel, header: &Json, frames: &[Frame]) {
    let Some(first) = frames.first() else { return };
    let Some(mut carried) = first.equipment.clone().or_else(|| header.get("equipment").cloned()) else { return };
    if frames.iter().all(|f| f.keys_up.is_empty() && f.bound.is_none()) {
        return;
    }
    let mut loot = first.loot.clone();
    let mut bound: Vec<(String, String)> = first.bound.clone().unwrap_or_default();
    let name = |template: &str| db.items.get(template).map_or_else(|| template.to_owned(), |item| item.name.clone());
    let (mut press, mut gesture): (Option<(f32, [f32; 2], Vec<String>)>, Option<(f32, Gesture)>) = (None, None);
    let mut last_plain_click: Option<(f32, [f32; 2])> = None;
    let mut window = false;
    let (mut compared, mut agreed) = (0usize, 0usize);
    let mut lines: Vec<String> = Vec::new();
    let tell = |lines: &mut Vec<String>, t: f32, what: String, game: String, model: String, same: Option<bool>| {
        let verdict = match same {
            Some(true) => "the same",
            Some(false) => "NOT the same",
            None => "",
        };
        lines.push(format!("{t:7.3}  {what}\n             the game:  {game}\n             the model: {model}  {verdict}"));
    };

    for (index, frame) in frames.iter().enumerate().skip(1) {
        let before = &frames[index - 1];
        // The gestures, as the keys and the button say them.
        if frame.inventory && !before.inventory {
            window = false;
        }
        let mouse = frame.mouse.unwrap_or_default();
        if frame.keys.iter().any(|key| key == "Mouse0") {
            press = Some((frame.t, mouse, frame.held.clone()));
        }
        if frame.keys.iter().any(|key| key == "F") {
            gesture = Some((frame.t, Gesture::Interact));
        }
        if frame.keys_up.iter().any(|key| key == "Mouse0") {
            if let Some((_, from, held)) = press.take() {
                let moved = ((mouse[0] - from[0]).powi(2) + (mouse[1] - from[1]).powi(2)).sqrt();
                if moved < CLICK_REACH {
                    let plain = !held.iter().any(|key| key.contains("Control") || key.contains("Shift"));
                    if plain && last_plain_click.is_some_and(|(t, at)| frame.t - t < DOUBLE_CLICK + 0.2 && (at[0] - mouse[0]).abs() < CLICK_REACH && (at[1] - mouse[1]).abs() < CLICK_REACH) {
                        window = true;
                        lines.push(format!("{:7.3}  a double click: a window opens (of what is not on record)", frame.t));
                    }
                    last_plain_click = plain.then_some((frame.t, mouse));
                    gesture = Some((frame.t, Gesture::Click { held }));
                } else {
                    gesture = Some((frame.t, Gesture::Drag { held, to: mouse, screen: frame.screen.unwrap_or([1920.0, 1080.0]) }));
                }
            }
        }
        if let Some(key) = frame.keys_up.iter().find_map(|key| QUICK_KEYS.iter().position(|quick| quick == key)) {
            if frame.inventory {
                gesture = Some((frame.t, Gesture::QuickKey(key)));
            }
        }
        let recent = gesture.clone().filter(|(t, _)| frame.t - t < 3.0).map(|(_, gesture)| gesture);
        let open = frame.inventory && frame.thing.as_ref().is_some_and(|seen| seen.kind == "LootableContainer" && seen.state == "Open");

        // A key bound or let go of.
        if let Some(now) = frame.bound.as_ref().filter(|now| **now != bound) {
            let model = Model::of(db, &carried, loot.as_ref());
            let gained = now.iter().find(|pair| !bound.contains(pair));
            match (model, gained) {
                (Ok(model), Some((key, item))) => {
                    let key_index = BOUND_KEYS.iter().position(|known| known == key);
                    let mut keys = Bound::default();
                    for (was_key, was_item) in &bound {
                        if let (Some(at), Some(id)) = (BOUND_KEYS.iter().position(|known| known == was_key), model.id(was_item)) {
                            keys.items[at] = Some(id);
                        }
                    }
                    let how = match &recent {
                        Some(Gesture::QuickKey(pressed)) if Some(*pressed) == key_index => format!("the key {} let go over a thing", QUICK_KEYS[*pressed]),
                        Some(Gesture::QuickKey(pressed)) => format!("the key {} let go, and another key bound", QUICK_KEYS[*pressed]),
                        Some(Gesture::Drag { to, screen, .. }) => {
                            let view = panel.quick_key_at(*to, *screen).map_or_else(|| "no view of the bar as read".to_owned(), |view| format!("the bar's view for {view} as the install's scene has it"));
                            format!("a thing dragged to ({:.0}, {:.0}): {view}", to[0], to[1])
                        }
                        _ => "nothing on record before it".to_owned(),
                    };
                    let thing = entries(&carried).into_iter().find(|entry| entry.id == *item);
                    let took = match (key_index, model.id(item)) {
                        (Some(at), Some(id)) => keys.bind(&model.equipment, id, at),
                        _ => false,
                    };
                    let after: Vec<String> = keys.items.iter().enumerate().filter_map(|(at, id)| id.map(|id| format!("{}: {}", BOUND_KEYS[at], tail(model.recorded(id))))).collect();
                    let game: Vec<String> = BOUND_KEYS.iter().filter_map(|known| now.iter().find(|(key, _)| key == known)).map(|(key, id)| format!("{key}: {}", tail(id))).collect();
                    compared += 1;
                    let same = took && after == game;
                    agreed += same as usize;
                    tell(
                        &mut lines,
                        frame.t,
                        format!("{how}; {} bound to {key}", thing.map_or_else(|| tail(item).to_owned(), |entry| format!("{} ({}) at {}", name(&entry.template), tail(&entry.id), entry.place()))),
                        game.join(", "),
                        if took { after.join(", ") } else { "refuses to bind it there".to_owned() },
                        Some(same),
                    );
                }
                (Ok(_), None) => lines.push(format!("{:7.3}  the bound keys: {} (a key let go of its thing)", frame.t, now.iter().map(|(key, id)| format!("{key}: {}", tail(id))).collect::<Vec<_>>().join(", "))),
                (Err(e), _) => lines.push(format!("{:7.3}  the lists could not be put together: {e:#}", frame.t)),
            }
            bound = now.clone();
        }

        // What is carried or what the container holds has changed.
        let (carried_after, loot_after) = (frame.equipment.clone().unwrap_or_else(|| carried.clone()), frame.loot.clone().or_else(|| loot.clone()));
        let was: Vec<Entry> = entries(&carried).into_iter().chain(loot.iter().flat_map(entries)).collect();
        let is: Vec<Entry> = entries(&carried_after).into_iter().chain(loot_after.iter().flat_map(entries)).collect();
        if (frame.equipment.is_some() || frame.loot.is_some()) && was != is {
            let found = |list: &[Entry], id: &str| list.iter().find(|entry| entry.id == id).cloned();
            // The things that lie somewhere else than they did, the new
            // ones that lie in nothing new, and the stacks that changed.
            let moved: Vec<(Entry, Entry)> = is.iter().filter_map(|now| found(&was, &now.id).filter(|old| old.place() != now.place()).map(|old| (old, now.clone()))).collect();
            let came: Vec<Entry> = is.iter().filter(|now| found(&was, &now.id).is_none() && found(&was, &now.holder).is_some()).cloned().collect();
            let went: Vec<Entry> = was.iter().filter(|old| found(&is, &old.id).is_none() && found(&is, &old.holder).is_some()).cloned().collect();
            let counted: Vec<(Entry, Entry)> = is.iter().filter_map(|now| found(&was, &now.id).filter(|old| old.count != now.count && old.place() == now.place()).map(|old| (old, now.clone()))).collect();
            let how = match &recent {
                Some(Gesture::Click { held }) if held.iter().any(|key| key.contains("Control")) => "Ctrl and a click",
                Some(Gesture::Click { held }) if held.iter().any(|key| key.contains("Alt")) => "Alt and a click",
                Some(Gesture::Click { .. }) => "a click",
                Some(Gesture::Drag { held, .. }) if held.iter().any(|key| key.contains("Control")) => "a drag with Ctrl held",
                Some(Gesture::Drag { .. }) => "a drag",
                Some(Gesture::QuickKey(_)) => "a quick-use key",
                Some(Gesture::Interact) => "the key that works what is looked at",
                None => "nothing on record",
            };
            match Model::of(db, &carried, loot.as_ref()) {
                Err(e) => lines.push(format!("{:7.3}  the lists could not be put together: {e:#}", frame.t)),
                Ok(model) => {
                    let whole = moved.len() == 1 && came.is_empty() && went.is_empty() && counted.is_empty();
                    if whole {
                        let (old, now) = &moved[0];
                        let what = format!("{how}: {} x{} ({}) from {}", name(&now.template), now.count, tail(&now.id), old.place());
                        let game = format!("to {}", now.place());
                        match (model.id(&now.id), &recent) {
                            (Some(id), Some(Gesture::Click { held })) if held.iter().any(|key| key.contains("Control")) => {
                                // Sent to the other side: with no window,
                                // and with each window it could have been.
                                let send = |priority: Option<Id>| {
                                    let mut tried = model.clone_of();
                                    let targets = inventory::other_side(&tried.equipment, tried.loot.as_ref().filter(|_| open), priority, id);
                                    match inventory::quick_find(&mut tried.roots(), id, &targets, Order::OTHER_SIDE, rules) {
                                        Ok(_) => format!("to {}", tried.place(id)),
                                        Err(refusal) => format!("refuses it ({refusal:?})"),
                                    }
                                };
                                let plain = send(None);
                                let mut said = plain.clone();
                                let mut same = plain == game;
                                if window {
                                    let holders: Vec<Id> = model.equipment.slots.iter().flatten().filter(|worn| !worn.kind.grids.is_empty()).map(|worn| worn.id).collect();
                                    let with: Vec<String> = holders.iter().map(|holder| (holder, send(Some(*holder)))).filter(|(_, sent)| *sent == game).map(|(holder, _)| name(&model.find(*holder).map_or(String::new(), |item| item.kind.template.clone()))).collect();
                                    said = format!("{plain} with no window the priority one; to the game's place with the window of: {}", if with.is_empty() { "none".to_owned() } else { with.join(", ") });
                                    same = same || !with.is_empty();
                                }
                                compared += 1;
                                agreed += same as usize;
                                tell(&mut lines, frame.t, what, game, said, Some(same));
                            }
                            (Some(id), Some(Gesture::Click { held })) if held.iter().any(|key| key.contains("Alt")) => {
                                let mut tried = model.clone_of();
                                let said = match inventory::equip(&mut tried.roots(), id, rules) {
                                    Ok(_) => format!("to {}", tried.place(id)),
                                    Err(refusal) => format!("refuses it ({refusal:?})"),
                                };
                                compared += 1;
                                agreed += (said == game) as usize;
                                let same = said == game;
                                tell(&mut lines, frame.t, what, game, said, Some(same));
                            }
                            (Some(id), _) => {
                                // A place chosen by hand: whether the model
                                // takes the thing there.
                                let mut tried = model.clone_of();
                                let into = tried.id(&now.holder);
                                let spot = into.and_then(|into| tried.find(into)).and_then(|holder| match now.cell {
                                    Some((x, y, turned)) => holder.grid_index(&now.at).map(|grid| Spot::Cell(grid, inventory::Place { x, y, turned })),
                                    None => holder.slot_index(&now.at).map(Spot::Slot),
                                });
                                let said = match (into, spot) {
                                    (Some(into), Some(spot)) => match inventory::move_item(&mut tried.roots(), id, into, spot, rules) {
                                        Ok(()) => "takes it there".to_owned(),
                                        Err(refusal) => format!("refuses it ({refusal:?})"),
                                    },
                                    _ => "has no such place".to_owned(),
                                };
                                compared += 1;
                                let same = said == "takes it there";
                                agreed += same as usize;
                                tell(&mut lines, frame.t, what, game, said, Some(same));
                            }
                            (None, _) => lines.push(format!("{:7.3}  {what}: the model has no such thing", frame.t)),
                        }
                    } else if let ([(old, now)], [new], true) = (counted.as_slice(), came.as_slice(), went.is_empty() && moved.is_empty()) {
                        // Part of a stack split off into a place of its own.
                        let split = old.count.saturating_sub(now.count);
                        let most = model.id(&old.id).map_or(0, |id| {
                            let spot = if new.cell.is_some() { Spot::Cell(0, inventory::Place { x: 0, y: 0, turned: false }) } else { Spot::Slot(0) };
                            inventory::split_most(&[&mut model.clone_of().equipment], id, spot)
                        });
                        let comes_up = most / 2 + 1;
                        compared += 1;
                        let same = new.count == split && split >= 1 && split <= most;
                        agreed += same as usize;
                        tell(
                            &mut lines,
                            frame.t,
                            format!("{how}, then the window that asks how many: {} x{} ({}) at {}", name(&old.template), old.count, tail(&old.id), old.place()),
                            format!("{split} split off to {} ({}), {} left", new.place(), tail(&new.id), now.count),
                            format!("1 to {most} may go; the window comes up at {comes_up}{}", if comes_up == split { ", which is what went" } else { "" }),
                            Some(same),
                        );
                    } else if let ([(a_old, a_now), (b_old, b_now)], true) = (counted.as_slice(), came.is_empty() && went.is_empty() && moved.is_empty()) {
                        // So many of one stack handed over to another.
                        let (from, onto) = if a_now.count < a_old.count { ((a_old, a_now), (b_old, b_now)) } else { ((b_old, b_now), (a_old, a_now)) };
                        let handed = from.0.count - from.1.count;
                        let most = match (model.id(&from.0.id), model.id(&onto.0.id)) {
                            (Some(id), Some(target)) => {
                                let mut tried = model.clone_of();
                                inventory::transfer_most(&tried.roots(), id, target)
                            }
                            _ => 0,
                        };
                        let comes_up = most / 2 + 1;
                        compared += 1;
                        let same = onto.1.count - onto.0.count == handed && handed >= 1 && handed <= most;
                        agreed += same as usize;
                        tell(
                            &mut lines,
                            frame.t,
                            format!("{how}, then the window that asks how many: {} x{} ({}) onto x{} ({})", name(&from.0.template), from.0.count, tail(&from.0.id), onto.0.count, tail(&onto.0.id)),
                            format!("{handed} handed over: x{} and x{}", from.1.count, onto.1.count),
                            format!("1 to {most} may go; the window comes up at {comes_up}{}", if comes_up == handed { ", which is what went" } else { "" }),
                            Some(same),
                        );
                    } else if let ([new], true) = (came.as_slice(), moved.is_empty() && went.is_empty() && counted.is_empty()) {
                        // Something taken up off the ground: the thing with
                        // everything in it, as the list after has it.
                        let mut alone: Vec<Json> = Vec::new();
                        let list = carried_after.as_array().cloned().unwrap_or_default();
                        let mut inside = vec![new.id.clone()];
                        for entry in &list {
                            let (id, holder) = (entry.get("id").and_then(Json::as_str).unwrap_or(""), entry.get("in").and_then(Json::as_str).unwrap_or(""));
                            if id == new.id {
                                let mut top = entry.clone();
                                if let Some(map) = top.as_object_mut() {
                                    map.retain(|key, _| !matches!(key.as_str(), "in" | "at" | "x" | "y" | "r"));
                                }
                                alone.insert(0, top);
                            } else if inside.iter().any(|known| known == holder) {
                                inside.push(id.to_owned());
                                alone.push(entry.clone());
                            }
                        }
                        let said = match Item::of_recording(db, &Json::Array(alone)) {
                            Ok((item, ..)) => {
                                let mut tried = model.clone_of();
                                let id = item.id;
                                tried.ids.push((new.id.clone(), id));
                                match inventory::pick_up(&mut tried.equipment, item, rules).0 {
                                    Ok((onto, Applied::Merged)) => format!("merged onto {}", tail(tried.recorded(onto))),
                                    Ok((onto, Applied::Topped(count))) => format!("{count} onto {}", tail(tried.recorded(onto))),
                                    Ok(_) => format!("to {}", tried.place(id)),
                                    Err(refusal) => format!("refuses it ({refusal:?})"),
                                }
                            }
                            Err(e) => format!("could not make the thing: {e:#}"),
                        };
                        let game = format!("to {}", new.place());
                        compared += 1;
                        let same = said == game;
                        agreed += same as usize;
                        tell(&mut lines, frame.t, format!("{how}: {} x{} ({}) taken up", name(&new.template), new.count, tail(&new.id)), game, said, Some(same));
                    } else {
                        let list = |what: &str, entries: Vec<String>| if entries.is_empty() { String::new() } else { format!(" {what}: {};", entries.join(", ")) };
                        lines.push(format!(
                            "{:7.3}  {how}; not set beside the model:{}{}{}{}",
                            frame.t,
                            list("moved", moved.iter().map(|(old, now)| format!("{} ({}) {} -> {}", name(&now.template), tail(&now.id), old.place(), now.place())).collect()),
                            list("came", came.iter().map(|entry| format!("{} x{} ({}) at {}", name(&entry.template), entry.count, tail(&entry.id), entry.place())).collect()),
                            list("went", went.iter().map(|entry| format!("{} x{} ({}) from {}", name(&entry.template), entry.count, tail(&entry.id), entry.place())).collect()),
                            list("stacks", counted.iter().map(|(old, now)| format!("{} ({}) x{} -> x{}", name(&now.template), tail(&now.id), old.count, now.count)).collect()),
                        ));
                    }
                    // What the bound keys keep after the move, by the model's
                    // rule on the lists as they now stand.
                    if !bound.is_empty() {
                        if let Ok(after) = Model::of(db, &carried_after, loot_after.as_ref()) {
                            let kept: Vec<&(String, String)> = bound.iter().filter(|(_, item)| after.id(item).is_some_and(|id| inventory::bindable(&after.equipment, id))).collect();
                            let game = frame.bound.as_ref().unwrap_or(&bound);
                            if kept.len() != game.len() || frame.bound.is_some() {
                                compared += 1;
                                let same = kept.len() == game.len() && kept.iter().all(|pair| game.contains(pair));
                                agreed += same as usize;
                                lines.push(format!(
                                    "{:7.3}  the bound keys after the move: the game keeps {}, the model {}  {}",
                                    frame.t,
                                    game.len(),
                                    kept.len(),
                                    if same { "the same" } else { "NOT the same" }
                                ));
                            }
                        }
                    }
                }
            }
        }
        if frame.equipment.is_some() {
            carried = carried_after;
        }
        if frame.loot.is_some() {
            loot = loot_after;
        }
    }
    if lines.is_empty() {
        return;
    }
    println!("  the inventory, each recorded change beside the model's own move:");
    for line in &lines {
        println!("    {line}");
    }
    println!("    {agreed} of {compared} are the game's");
}

/// A vector turned by a quaternion (x, y, z, w), as the engine turns it.
fn rotate(q: [f32; 4], v: [f32; 3]) -> [f32; 3] {
    let u = [q[0], q[1], q[2]];
    let cross = |a: [f32; 3], b: [f32; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    let t = cross(u, v).map(|x| 2.0 * x);
    let c = cross(u, t);
    std::array::from_fn(|k| v[k] + q[3] * t[k] + c[k])
}

/// One of the engine's axes by its name in the game's `EDoorAxis`.
fn axis_named(name: &str) -> [f32; 3] {
    match name {
        "X" => [1.0, 0.0, 0.0],
        "Y" => [0.0, 1.0, 0.0],
        "XNegative" => [-1.0, 0.0, 0.0],
        "YNegative" => [0.0, -1.0, 0.0],
        "ZNegative" => [0.0, 0.0, -1.0],
        _ => [0.0, 0.0, 1.0],
    }
}

/// The doors, containers and trunks worked in a recording (recorder 0.6.1
/// on), each set beside the model of its kind (`trp_sim::things`): worked
/// on the frame the game shows it worked (its state turns to being worked
/// on a frame a key went down), told to shut when its screen shuts, and
/// stepped with the game's frame times. Its angle and its state are then
/// compared frame by frame. A door's gesture by where the player stands is
/// tried on every frame it was looked at, for each way its forward axis
/// could lie (the recorder does not write it), against the game's own number.
fn length(v: [f32; 3]) -> f32 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

fn things_worked(frames: &[Frame], settings: &ThingSettings) {
    // Each thing worked, in the order it was first worked.
    let mut ids: Vec<String> = Vec::new();
    for seen in frames.iter().filter_map(|f| f.thing.as_ref()).filter(|s| s.state == "Interacting") {
        if !ids.contains(&seen.id) {
            ids.push(seen.id.clone());
        }
    }
    for id in ids {
        thing_worked(frames, settings, id);
    }
}

fn thing_worked(frames: &[Frame], settings: &ThingSettings, id: String) {
    fn seen_in<'a>(f: &'a Frame, id: &str) -> Option<&'a Seen> {
        f.thing.as_ref().filter(|s| s.id == id)
    }
    let seen = |f| seen_in(f, &id);
    let Some(first) = frames.iter().find_map(seen) else { return };
    let (shut, open) = (first.angles[0], first.angles[1]);
    // What the thing names for its two gestures, as the recording has it
    // asked of a player while it stood shut and while it stood open.
    let named = |state: &str| frames.iter().filter_map(seen).find(|s| s.state == state).and_then(|s| Interaction::of_game_number(s.gesture));
    // A door's hinge as a frame has it, for a way its forward axis could lie.
    let hinge_of = |f: &Frame, s: &Seen, forward_name: &str| {
        let axis = rotate(s.rot, axis_named(&s.axis));
        // The door's own forward axis turns with it: back to its way at an
        // angle of nothing.
        let own = rotate(s.rot, axis_named(forward_name));
        let forward = trp_sim::things::turn(own, axis, -s.angle);
        Hinge { at: std::array::from_fn(|k| f.pos[k] + s.from_player[k]), axis, forward }
    };
    // A door's gesture by side, for each way its forward axis could lie.
    let mut forward_found = None;
    if first.kind == "Door" {
        let candidates = ["X", "XNegative", "Y", "YNegative", "Z", "ZNegative"];
        let mut results = Vec::new();
        for forward_name in candidates.into_iter().filter(|name| !name.starts_with(&first.axis[..1])) {
            let (mut agree, mut looked) = (0, 0);
            for f in frames {
                let Some(s) = seen(f).filter(|s| s.state == "Shut" || s.state == "Open") else { continue };
                let mut door = Thing::door(hinge_of(f, s, forward_name), shut, open);
                door.at_rest(s.state == "Open");
                looked += 1;
                if door.door_gesture(f.pos) == s.gesture {
                    agree += 1;
                }
            }
            if agree == looked && looked > 0 && forward_found.is_none() {
                forward_found = Some(forward_name);
            }
            results.push(format!("forward {forward_name}: {agree} of {looked}"));
        }
        println!("  the door's gesture by side, the model's rule against the game's number on the frames it stood shut or open: {}", results.join(", "));
    }
    let mut thing = match first.kind.as_str() {
        "Door" => {
            let f = frames.iter().find(|f| seen(f).is_some()).expect("a frame that sees it");
            Thing::door(hinge_of(f, first, forward_found.unwrap_or("Y")), shut, open)
        }
        "Trunk" => Thing::trunk(named("Shut"), named("Open"), shut, open),
        _ => Thing::container(named("Shut"), named("Open"), shut, open),
    };
    thing.at_rest(first.state == "Open");
    if first.state == "Locked" {
        thing.lock(None);
    }
    println!("  the thing worked: {} {id}, angles {shut} shut and {open} open", first.kind);

    let (mut compared, mut worst, mut worst_at, mut states_off) = (0usize, 0.0f32, 0.0f32, 0usize);
    let mut shown = false;
    let mut said = Vec::new();
    let mut last_state = String::new();
    // A kick under way: what the model said of it at the key, and whether
    // it has landed.
    let mut kick: Option<(trp_sim::things::Breach, bool)> = None;
    // A container's search under way: how far it stood at the key, and
    // the time waited since (`OnContainerOpen` after `DelayToOpenContainer`,
    // the key's frame not counted, as `Movement::search` has it).
    let mut searching: Option<(f32, f32)> = None;
    // A lock being opened with its key: held from the key
    // (`LockForInteraction`), and unlocked when the hand is laid, 1.6 s
    // into the body's state for it, a firearm's animator giving no
    // progress of its own (interactions spec, "Unlocking"); the time in
    // that state so far.
    let mut unlocking: Option<f32> = None;
    for (i, f) in frames.iter().enumerate() {
        let prev = i.checked_sub(1).map(|p| &frames[p]);
        // A kick lands 0.45 of the way through the body's state for it
        // (`BreachDoorState.KickTime`), which is past its blend in.
        if let Some((breach, landed)) = &mut kick {
            if !*landed && f.state == "BreachDoor" && !f.body_blending && f.body_time >= 0.45 {
                *landed = true;
                if breach.opens { thing.kick_open() } else { thing.fail_breach() }
                said.push(format!("{:7.3}  the kick lands ({:.4} of the way through its state): the model's door {}", f.t, f.body_time, if breach.opens { "is thrown open" } else { "holds" }));
            }
            if f.state == "BreachDoor" && prev.is_some_and(|p| p.state == "Approach") {
                let off = ((f.pos[0] - breach.position[0]).powi(2) + (f.pos[2] - breach.position[2]).powi(2)).sqrt();
                if breach.snaps {
                    said.push(format!("{:7.3}  the kick's state begins: the player stands {:.1} mm from the place the model has the kick made from", f.t, off * 1000.0));
                }
            }
            if *landed && f.state != "BreachDoor" {
                kick = None;
            }
        }
        if let (Some(s), Some(p)) = (seen(f), prev) {
            let was = seen(p).map(|s| s.state.as_str()).unwrap_or("");
            // A kick: the key puts the player's movement into its approach
            // and from there into the kick's own state.
            let kicked = first.kind == "Door"
                && f.state == "Approach"
                && frames[i..].iter().find(|later| later.state != "Approach").is_some_and(|later| later.state == "BreachDoor");
            if s.state == "Interacting" && was != "Interacting" && !f.keys.is_empty() && kicked {
                if let Some(mut breach) = thing.start_breach(p.pos) {
                    // Which way the door's leaf lies is known only where
                    // the recording has it looked at shut or open. Without
                    // that, which side the kick comes from cannot be told,
                    // and what the kick did is taken from the recording.
                    let game_opens = frames[i..].iter().filter_map(seen).find(|later| later.state != "Interacting").is_some_and(|later| later.state == "Open");
                    let by = if forward_found.is_some() && breach.opens && !game_opens && thing.can_be_breached {
                        // The side was right for it and the game's door held:
                        // a door that cannot be breached (its `CanBeBreached`,
                        // not on record), taken so from here on.
                        thing.can_be_breached = false;
                        breach.opens = false;
                        "by the model it would open from this side, and the game's held: a door that cannot be breached (not on record), so"
                    } else if forward_found.is_some() {
                        "by the model"
                    } else {
                        breach.opens = frames[i..].iter().filter_map(seen).find(|later| later.state != "Interacting").is_some_and(|later| later.state == "Open");
                        "its leaf's own axis is not on record, so by the recording"
                    };
                    said.push(format!(
                        "{:7.3}  kicked (the key {}): {by} it {}, and the door {} the player at its place first (the game's word: {})",
                        f.t,
                        f.keys.join("+"),
                        if breach.opens { "will open" } else { "will hold" },
                        if breach.snaps { "puts" } else { "does not put" },
                        f.working_snap.map_or("not recorded".to_owned(), |snap| snap.to_string()),
                    ));
                    kick = Some((breach, false));
                }
            } else if s.state == "Interacting" && was == "Locked" && !f.keys.is_empty() {
                thing.hold();
                unlocking = Some(0.0);
                said.push(format!("{:7.3}  unlocked with its key (the key {}): held until the hand is laid", f.t, f.keys.join("+")));
            } else if s.state == "Interacting" && was != "Interacting" && !f.keys.is_empty() {
                let opens = was == "Shut";
                let gesture = thing.work(opens, f.pos, settings);
                if opens && first.kind == "LootableContainer" {
                    searching = Some((length(s.from_player), -1.0));
                }
                said.push(format!(
                    "{:7.3}  worked to {} (the key {}); the model's gesture {:?}, the game's {}",
                    f.t,
                    if opens { "open" } else { "shut" },
                    f.keys.join("+"),
                    gesture.map(Interaction::index),
                    f.gesture_index
                ));
            }
        }
        // The hand laid on the lock: more than 1.6 s of the body's state for
        // it, counted from the frame after the state is entered, as
        // `Movement::act` counts it.
        if let Some(time) = unlocking {
            if f.state == "DoorInteraction" {
                let entered = prev.is_some_and(|p| p.state != "DoorInteraction");
                let time = if entered { time } else { time + f.clock_dt };
                if time > 1.6 {
                    thing.unlock();
                    unlocking = None;
                    said.push(format!("{:7.3}  the hand is laid: unlocked, shut", f.t));
                } else {
                    unlocking = Some(time);
                }
            }
        }
        // The search's wait over: the character a quarter of a metre further
        // off than at the key, or in a jump, and the container is told to
        // shut (`OnContainerOpen`); else its screen opens.
        if let Some((at_key, waited)) = searching {
            // (The key's own frame, marked below nothing, is not counted.)
            let waited = if waited < 0.0 { 0.0 } else { waited + f.dt };
            searching = Some((at_key, waited));
            if waited + 1e-6 >= settings.answers_after {
                searching = None;
                let off = seen(f).map_or(f32::MAX, |s| length(s.from_player)) - at_key;
                if f.state == "Jump" || off > 0.25 {
                    thing.tell(false, settings);
                    said.push(format!("{:7.3}  the search's wait is over {off:.3} m further off than at the key: told to shut", f.t));
                } else {
                    said.push(format!("{:7.3}  the search's wait is over {off:.3} m further off than at the key: its screen is to open", f.t));
                }
            }
        }
        // The screen of a searched container shuts: it is told to shut.
        if let Some(p) = prev {
            if p.inventory && !f.inventory && shown {
                thing.tell(false, settings);
                said.push(format!("{:7.3}  the screen shuts: told to shut", f.t));
                shown = false;
            }
            if f.inventory && !p.inventory {
                shown = true;
            }
        }
        thing.step(f.dt, settings);
        let Some(s) = seen(f) else { continue };
        let model = format!("{:?}", thing.state());
        if model != s.state {
            states_off += 1;
        }
        if s.state != last_state {
            said.push(format!("{:7.3}  the game's {} {}, the model's {}", f.t, first.kind, s.state, model));
            last_state = s.state.clone();
        }
        compared += 1;
        let gap = (thing.angle() - s.angle).abs();
        if gap > worst {
            (worst, worst_at) = (gap, f.t);
        }
    }
    for line in &said {
        println!("    {line}");
    }
    println!("    on {compared} frames: the angle at most {worst:.3} degrees from the game's (at {worst_at:.3} s), the state differs on {states_off}");
}
