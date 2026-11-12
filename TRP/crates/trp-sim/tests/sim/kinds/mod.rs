//! Tests of one weapon of each further kind, each run on its own animator from the install.
//! After `docs/spec/more-weapons.md`: a bolt action whose magazine comes out, a self-loader
//! filled through its action, a revolver, a double barrel and a launcher with a drum. These
//! pass trivially, with a note, where no install is configured.
//!
//! Run with `cargo test -p trp-sim --test sim kinds:: -- --nocapture` for what
//! each does.

use trp_data::unity::Assets;
use trp_sim::{FireMode, Firearm, FirearmEvent};

mod actions;
mod chambers;
mod revolvers;
mod by_hand;

use chambers::{MP18, RSHG2, dropped, events, shown, thrown_and_loaded};

const SV98: &str = "55801eed4bdc2d89578b4588";
const SKS: &str = "574d967124597745970e7c94";
const RHINO: &str = "61a4c8884f95bc3b2c5dc96f";
const MP43: &str = "5580223e4bdc2d1c128b457f";
const MSGL: &str = "6275303a9f372d6ea97f9ec7";
const MTS255: &str = "60db29ce99594040e04c4a27";
const FRAME: f32 = 1.0 / 120.0;

fn weapon_of(template: &str) -> Option<Firearm> {
    let Ok(install) = crate::fixture::install().ok_or(()) else {
        return None;
    };
    let db = crate::fixture::database().expect("database");
    let hard = trp_data::setup::hard_settings::HardSettings::load(&install).expect("settings object");
    let mut assets = Assets::new(install).expect("manifest");
    let weapon = trp_data::weapons::weapon::assemble(&mut assets, &db, template).expect("the weapon");
    let data = trp_sim::weapons::weapon_animator::WeaponAnimatorData::load(&assets, &weapon).expect("its controller");
    let settings = trp_sim::firearm_settings(&assets, &db, &hard, &weapon).expect("its settings");
    println!(
        "{}: {} rounds, modes {:?}, filled a round at a time {}, bolt worked by hand {}, catch {}",
        weapon.name, settings.magazine_capacity, settings.modes, settings.internal_magazine, settings.bolt_action, settings.bolt_catch
    );
    Some(Firearm::new(settings, &data))
}

/// Steps a weapon and says which states its animator entered and what it
/// said, each with its time.
fn run(w: &mut Firearm, seconds: f32) -> (Vec<(String, f32)>, Vec<(FirearmEvent, f32)>) {
    let name = |w: &Firearm| {
        let (state, _, next) = w.animator().state();
        next.unwrap_or(state).trim_start_matches("Hands.").to_owned()
    };
    let (mut entered, mut said, mut last) = (Vec::new(), Vec::new(), name(w));
    for frame in 0..(seconds / FRAME).round() as usize {
        let at = (frame + 1) as f32 * FRAME;
        said.extend(w.step(FRAME).into_iter().filter(|e| !matches!(e, FirearmEvent::Sound(_) | FirearmEvent::BodyAction(_))).map(|e| (e, at)));
        let now = name(w);
        if now != last {
            entered.push((now.clone(), at));
            last = now;
        }
    }
    (entered, said)
}

fn names(entered: &[(String, f32)]) -> Vec<&str> {
    entered.iter().map(|entry| entry.0.as_str()).collect()
}

fn told(entered: &[(String, f32)]) -> String {
    entered.iter().map(|(state, at)| format!("{state} {at:.2}")).collect::<Vec<_>>().join(" > ")
}


fn shots(said: &[(FirearmEvent, f32)]) -> usize {
    said.iter().filter(|(event, _)| matches!(event, FirearmEvent::Shot { .. })).count()
}

/// Each shot's hammer as it was made: down or not.
fn hammers(said: &[(FirearmEvent, f32)]) -> Vec<bool> {
    said.iter().filter_map(|(event, _)| if let FirearmEvent::Shot { hammer_down, .. } = event { Some(*hammer_down) } else { None }).collect()
}

/// Presses the trigger for a moment and lets the weapon come to rest.
fn press(w: &mut Firearm, seconds: f32) -> (Vec<(String, f32)>, Vec<(FirearmEvent, f32)>) {
    w.set_trigger(true, true);
    let (mut entered, mut said) = run(w, 0.3);
    w.set_trigger(false, true);
    let (more, more_said) = run(w, seconds);
    entered.extend(more.into_iter().map(|(state, at)| (state, at + 0.3)));
    said.extend(more_said);
    (entered, said)
}
