//! Checks against a real install. They pass trivially, with a note, on a
//! machine where no install is configured.

use std::collections::HashMap;

use trp_data::unity::anim::{euler_to_quat, path_hash, Clip, Property};
use trp_data::unity::extract::{read_mesh, read_texture};
use trp_data::unity::Assets;
use trp_data::weapons::weapon::{assemble, Weapon};
use trp_data::{Database, Install};

mod particle_effects;
mod drawing;
mod clothing;
mod weapon_parts;
mod animation;
mod outfits;
mod screens;
mod sounds;
mod head_gear;
mod impacts;
mod locations;


const AKM: &str = "59d6088586f774275f37482f";

fn akm() -> Option<(Assets, Weapon)> {
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set SPT_DIR or local/spt_dir.txt)");
        return None;
    };
    let db = Database::load(&install).expect("database");
    let mut assets = Assets::new(install).expect("manifest");
    let weapon = assemble(&mut assets, &db, AKM).expect("assemble AKM");
    Some((assets, weapon))
}
