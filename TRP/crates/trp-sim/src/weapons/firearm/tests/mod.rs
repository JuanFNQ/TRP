//! Tests of the firearm in the hands, with their shared helpers.
use super::*;
use crate::base::animator::{Condition, Controller, LayerDef, MachineDef, StateDef, Target, TransitionDef};
use crate::base::marks::MarkTable;
use trp_data::weapons::states::{Parameter, ParameterKind};

mod drawing;
mod firing;
mod reloading;


const PARAMETERS: [(&str, ParameterKind, f32); 16] = [
    ("ReloadFast", ParameterKind::Trigger, 0.0),
    ("Active", ParameterKind::Bool, 0.0),
    ("Inventory", ParameterKind::Bool, 0.0),
    ("Fire", ParameterKind::Bool, 0.0),
    ("AmmoInChamber", ParameterKind::Float, 0.0),
    ("AmmoInMag", ParameterKind::Float, 0.0),
    ("MagInWeapon", ParameterKind::Bool, 1.0),
    ("CanReload", ParameterKind::Bool, 0.0),
    ("Reload", ParameterKind::Trigger, 0.0),
    ("FireMode", ParameterKind::Float, 0.0),
    ("FiremodeSwitch", ParameterKind::Trigger, 0.0),
    ("CheckAmmo", ParameterKind::Trigger, 0.0),
    ("Armed", ParameterKind::Bool, 1.0),
    ("FastHide", ParameterKind::Bool, 0.0),
    ("RelTypeOld", ParameterKind::Float, 0.0),
    ("RelTypeNew", ParameterKind::Float, 0.0),
];

/// The states of a rifle's animator, cut down to what the logic goes
/// by, with round numbers: each with its length in seconds, its marks
/// (as fractions of it) and its ways out. A way out is written as the
/// state it leads to, the point it waits for (a fraction of the state,
/// below zero for none), the blend (a fraction of the state) and its
/// conditions, each a parameter with `!` for "not set", `>0` or `<1`.
type Way = (&'static str, f32, f32, &'static [&'static str]);
const STATES: [(&str, f32, &[(&str, f32)], &[Way]); 19] = [
    ("SPAWN", 0.33, &[], &[("OUT TO IDLE", -1.0, 0.0, &["Active"])]),
    ("OUT TO IDLE", 0.967, &[("WeapIn", 0.99)], &[("IDLE", 0.98, 0.02, &["!Inventory"]), ("OPEN INVENTORY", 0.78, 0.22, &["Inventory"])]),
    (
        "IDLE",
        0.6,
        &[("IdleStart", 0.0)],
        &[
            ("OPEN INVENTORY", -1.0, 0.05, &["Inventory"]),
            ("RELOAD OUT ALL", -1.0, 0.1, &["ReloadFast", "MagInWeapon", "CanReload"]),
            ("IDLE TO OUT", -1.0, 0.05, &["!Active"]),
            ("RELOAD START", -1.0, 0.0, &["CanReload", "Reload", "MagInWeapon"]),
            ("DRY FIRE", -1.0, 0.0, &["Armed", "AmmoInChamber<1", "Fire"]),
            ("FIREMODE", -1.0, 0.0, &["FiremodeSwitch"]),
            ("FIRE", -1.0, 0.0, &["Fire", "AmmoInChamber>0"]),
            ("CHECK", -1.0, 0.0, &["CheckAmmo"]),
            ("DRY FIRE DISARMED", -1.0, 0.0, &["!Armed", "AmmoInChamber<1", "Fire"]),
        ],
    ),
    (
        "FIRE",
        0.0967,
        &[("FiringBullet", 0.0), ("ShellEject", 0.31), ("DelAmmoChamber", 0.665), ("AddAmmoInChamber", 0.776), ("FireEnd", 0.873)],
        &[("IDLE", 1.0, 0.0, &[]), ("DRY FIRE", 0.95, 0.0, &["FireMode<1", "AmmoInChamber<1", "AmmoInMag<1"])],
    ),
    ("DRY FIRE", 0.267, &[("Disarm", 0.0)], &[("IDLE", 1.0, 0.0, &["!Fire"])]),
    ("DRY FIRE DISARMED", 0.267, &[], &[("IDLE", 1.0, 0.0, &["!Fire"])]),
    ("RELOAD START", 0.444, &[], &[("RELOAD OUT", 0.3, 0.75, &[])]),
    ("RELOAD OUT", 1.0, &[("MagOut", 0.462), ("MagHide", 0.85)], &[("RELOAD IN", 0.78, 0.22, &["CanReload"])]),
    ("RELOAD OUT ALL", 1.0, &[("MagOut", 0.486), ("MagHide", 0.602)], &[("RELOAD IN", 0.9, 0.1, &[])]),
    (
        "RELOAD IN",
        1.2,
        &[("MagShow", 0.092), ("MagIn", 0.707)],
        &[
            ("RELOAD CHARGE END", 0.95, 0.05, &["MagInWeapon", "AmmoInMag>0", "AmmoInChamber<1"]),
            ("RELOAD END", 0.9, 0.1, &["AmmoInChamber>0", "!Inventory"]),
            ("RELOAD INVENTORY END", 0.94, 0.16, &["Inventory", "AmmoInChamber>0"]),
            ("RELOAD INVENTORY END", 0.94, 0.16, &["Inventory", "AmmoInMag<1"]),
            ("RELOAD END", 0.9, 0.1, &["!Inventory", "AmmoInMag<1"]),
        ],
    ),
    ("RELOAD END", 0.457, &[], &[("IDLE", 0.98, 0.03, &[])]),
    (
        "RELOAD CHARGE END",
        1.3,
        &[("Arm", 0.476), ("AddAmmoInChamber", 0.584)],
        &[("RELOAD INVENTORY END", 0.98, 0.1, &["Inventory"]), ("IDLE", 0.92, 0.19, &["!Inventory"])],
    ),
    ("RELOAD INVENTORY END", 0.333, &[], &[("IN INVENTORY", 0.25, 0.75, &[])]),
    ("OPEN INVENTORY", 0.667, &[], &[("IN INVENTORY", 0.5, 0.375, &[])]),
    ("IN INVENTORY", 0.333, &[], &[("CLOSE INVENTORY", -1.0, 0.042, &["Active", "!Inventory"]), ("IDLE TO OUT", -1.0, 0.5, &["!Active"])]),
    ("CLOSE INVENTORY", 0.667, &[], &[("IDLE", 0.96, 0.045, &["Active"])]),
    ("FIREMODE", 1.0, &[], &[("IDLE", 1.0, 0.0, &[])]),
    ("CHECK", 2.0, &[("StartUtilityOperation", 0.0)], &[("IDLE", 1.0, 0.0, &[])]),
    ("IDLE TO OUT", 0.833, &[("WeapOut", 0.968)], &[("SPAWN", 1.0, 0.0, &[])]),
];

/// The animator of that rifle, written out by hand.
fn animator() -> WeaponAnimatorData {
    let parameters: Vec<Parameter> =
        PARAMETERS.iter().map(|(name, kind, default)| Parameter { name: (*name).to_owned(), kind: *kind, default: *default }).collect();
    let parameter = |name: &str| parameters.iter().position(|p| p.name == name);
    let condition = |text: &str| {
        let (name, mode, threshold) = match text {
            t if t.starts_with('!') => (&t[1..], 2, 0.0),
            t if t.ends_with(">0") => (&t[..t.len() - 2], 3, 0.0),
            t if t.ends_with("<1") => (&t[..t.len() - 2], 4, 1.0),
            t => (t, 1, 0.0),
        };
        Condition { parameter: parameter(name), mode, threshold }
    };
    let place = |name: &str| STATES.iter().position(|state| state.0 == name).expect("a state of the rifle");
    let states = STATES
        .iter()
        .map(|(name, length, _, ways)| StateDef {
            name: format!("Hands.{name}"),
            speed: 1.0 / length,
            speed_parameter: None,
            looping: false,
            writes_defaults: true,
            cycle_offset: 0.0,
            trees: vec![None],
            transitions: ways
                .iter()
                .map(|(to, exit, blend, conditions)| TransitionDef {
                    target: Target::State(place(to)),
                    duration: *blend,
                    fixed: false,
                    offset: 0.0,
                    exit_time: (*exit >= 0.0).then_some(*exit),
                    conditions: conditions.iter().map(|text| condition(text)).collect(),
                    interruption: 0,
                })
                .collect(),
            movement: None,
            layer_weights: Vec::new(),
            loaded: true,
        })
        .collect();
    let machine = MachineDef { states, any_state: Vec::new(), selectors: Vec::new(), default_state: 0, keeps_weights: false };
    let layer = LayerDef { name: "Hands".to_owned(), machine: 0, slot: 0, default_weight: 1.0, additive: false, moves: Vec::new() };
    let marks: Vec<Vec<(&str, f32)>> = STATES.iter().map(|state| state.2.to_vec()).collect();
    let controller = Controller::by_hand(parameters, vec![machine], vec![layer]);
    WeaponAnimatorData::by_hand(controller, MarkTable::by_hand(&[marks])).expect("a layer for the hands")
}

/// A rifle with round numbers: 600 rounds a minute, 450 for single shots.
pub(crate) fn settings() -> FirearmSettings {
    FirearmSettings {
        fire_rate: 600.0,
        single_fire_rate: 450.0,
        modes: vec![FireMode::Single, FireMode::FullAuto],
        burst_count: 3,
        magazine_capacity: 30,
        bolt_catch: false,
        can_queue_second_shot: true,
        shot_strength: 1.0,
        rest_state: "Hands.IDLE".to_owned(),
        fire_state: "Hands.FIRE".to_owned(),
        fire_mode_state: "Hands.FIREMODE".to_owned(),
        semi_fire_state: "Hands.SA FIRE".to_owned(),
        internal_magazine: false,
        no_fire_mode_on_bolt_catch: false,
        bolt_action: false,
        open_bolt_reload: false,
        must_open_bolt: false,
        manual_bolt_catch: false,
        chamber_load: true,
        foldable: false,
        reload_speed: 1.0,
        draw_speed: 1.0,
        cylinder: None,
        barrels: 0,
        fast_reload: true,
        rocket_launcher: false,
        one_off: false,
        can_unload: true,
        no_chamber: false,
        magazine_variant: 0,
        magazine_external: false,
        unique_animation: 0,
        mastering: 0,
        belt_fed: false,
        stop_aiming_at: 0.1,
        mountable: true,
        bipod: None,
    }
}

/// That rifle in the hands and at rest.
pub(crate) fn rifle() -> Firearm {
    let mut weapon = Firearm::new(settings(), &animator());
    weapon.start_drawn();
    weapon
}

const DT: f32 = 1.0 / 60.0;

fn shots(events: &[Event]) -> usize {
    events.iter().filter(|e| matches!(e, Event::Shot { .. })).count()
}

fn run(weapon: &mut Firearm, seconds: f32) -> Vec<Event> {
    let mut all = Vec::new();
    for _ in 0..(seconds / DT).round() as usize {
        all.extend(weapon.step(DT));
    }
    all
}

/// The animator's states as they were entered over `seconds`, each
/// with the time of its step, counted from `from`.
fn states(weapon: &mut Firearm, seconds: f32, from: f32) -> Vec<(String, f32)> {
    let name = |weapon: &Firearm| {
        let (state, _, next) = weapon.animator().state();
        next.unwrap_or(state).trim_start_matches("Hands.").to_owned()
    };
    let mut last = name(weapon);
    let mut at = Vec::new();
    for step in 0..(seconds / DT).round() as usize {
        weapon.step(DT);
        let now = name(weapon);
        if now != last {
            at.push((now.clone(), from + step as f32 * DT));
            last = now;
        }
    }
    at
}

fn names(entered: &[(String, f32)]) -> Vec<&str> {
    entered.iter().map(|entry| entry.0.as_str()).collect()
}

/// Steps until `done`, and says how long that took.
fn run_until(weapon: &mut Firearm, done: impl Fn(&Firearm) -> bool) -> f32 {
    let mut steps = 0;
    while !done(weapon) {
        weapon.step(DT);
        steps += 1;
        assert!(steps < 1200, "never happened");
    }
    steps as f32 * DT
}

/// Steps until the weapon is at rest, a step at the least: what a
/// command asked of the animator shows only after its next update.
fn settle(weapon: &mut Firearm) -> f32 {
    weapon.step(DT);
    DT + run_until(weapon, |w| w.is_idle())
}
