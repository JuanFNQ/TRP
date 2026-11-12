//! Tests of the dummies on the range, with their shared helpers.
use super::*;
use bevy::ecs::system::RunSystemOnce;
use trp_sim::condition::armor::Shot;

mod wear_weapons;
mod geometry;
mod economy;
mod blows;

use economy::{Seeded, economy_dummies, economy_makings, step_economy};

/// The install and its database, read once for these tests; none where
/// no install is configured (the test then passes with a note).
fn installed() -> Option<&'static (trp_data::Install, trp_data::Database)> {
    static READ: std::sync::OnceLock<Option<(trp_data::Install, trp_data::Database)>> = std::sync::OnceLock::new();
    let read = READ
        .get_or_init(|| {
            let install = trp_data::Install::locate().ok()?;
            let db = trp_data::Database::load(&install).expect("database");
            Some((install, db))
        })
        .as_ref();
    if read.is_none() {
        eprintln!("skipped: no install configured (set SPT_DIR or local/spt_dir.txt)");
    }
    read
}

/// Near one of the dummies (the game's axes): within `spread` of its
/// feet across and along, between its knees and its head up.
fn near(dummies: &Dummies, seeded: &mut Seeded, spread: f32) -> V3 {
    let which = (seeded.unit() * dummies.list.len() as f32) as usize % dummies.list.len();
    let feet = dummies.list[which].puppet.as_ref().expect("its character").movement.position;
    [feet[0] + seeded.within(-spread, spread), feet[1] + seeded.within(0.2, 2.0), feet[2] + seeded.within(-spread, spread)]
}
