//! Tests of a grenade's hands and a knife's on the install's own animators.
//! After `docs/spec/grenades-and-knife.md`: the F-1 drawn, its pin pulled, held
//! and thrown, and put back; the ice pick drawn, a slash and a stab with
//! the blade live between their marks and a hit glancing off, and the quick
//! stroke. These pass trivially, with a note, where no install is set up.

use trp_data::unity::Assets;
use trp_sim::weapons::weapon_animator::WeaponAnimatorData;
use trp_sim::{GrenadeEvent, GrenadeHands, KnifeEvent, KnifeHands, KnifeSettings, Stroke};

mod first_person;
mod blade_strokes;

use first_person::{run_knife};

const FRAME: f32 = 1.0 / 120.0;
const F1: &str = "5710c24ad2720bc3458b45a3";
const ICE_PICK: &str = "5c0126f40db834002a125382";

fn rig(template: &str) -> Option<(WeaponAnimatorData, serde_json::Value)> {
    let Ok(install) = crate::fixture::install().ok_or(()) else {
        return None;
    };
    let db = crate::fixture::database().expect("database");
    let mut assets = Assets::new(install).expect("manifest");
    let item = db.items.get(template).expect("the item");
    let path = item.props["Prefab"]["path"].as_str().expect("its prefab").to_owned();
    let rig = trp_data::weapons::weapon::assemble_prefab(&mut assets, &path).expect("its rig");
    Some((WeaponAnimatorData::load(&assets, &rig).expect("its animator"), item.props.clone()))
}

/// Steps the grenade's hands for so long; every state entered and every
/// event, each with the time it came.
fn run_grenade(hands: &mut GrenadeHands, seconds: f32, clock: &mut f32) -> (Vec<(String, f32)>, Vec<(GrenadeEvent, f32)>) {
    let (mut entered, mut said) = (Vec::new(), Vec::new());
    let mut last = hands.animator().state().0.to_owned();
    for _ in 0..(seconds / FRAME).round() as usize {
        *clock += FRAME;
        for event in hands.step(FRAME) {
            said.push((event, *clock));
        }
        let (state, _, next) = hands.animator().state();
        let now = next.unwrap_or(state).trim_start_matches("Hands.").to_owned();
        if now != last {
            entered.push((now.clone(), *clock));
            last = now;
        }
    }
    (entered, said)
}
