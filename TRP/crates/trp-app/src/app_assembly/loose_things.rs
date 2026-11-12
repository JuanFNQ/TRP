//! The ragdoll, the loose things on the ground, the crates filled to be searched, and what the run begins with.

use super::*;

pub(super) struct LooseInputs<'a> {
    pub(super) unity: &'a mut UnityAssets,
    pub(super) hard: &'a HardSettings,
    pub(super) db: &'a std::sync::Arc<Database>,
    pub(super) options_state: &'a screens::options_view::OptionsState,
    pub(super) options: &'a Options,
    pub(super) rounds_drawn: bool,
    pub(super) away: &'a mut Vec<Option<trp_sim::items::inventory::Item>>,
    pub(super) ground: &'a Vec<(trp_sim::items::inventory::Item, Vec3)>,
    pub(super) laid_from: usize,
    pub(super) controller: &'a Controller,
    pub(super) equipment: &'a trp_sim::items::inventory::Equipment,
    pub(super) loot: &'a trp_sim::items::inventory::Item,
    pub(super) range: &'a world::range::Range,
}

pub(super) struct LooseThings {
    pub(super) ragdoll: player::death::Ragdoll,
    pub(super) loose: world::loose::Loose,
    pub(super) physics_clock: player::death::PhysicsClock,
    pub(super) searcher: trp_sim::items::search::Searcher,
    pub(super) start_of_run: player::death::StartOfRun,
}

/// Reads the ragdoll, makes the loose things and the searcher, fills the crates, and keeps what the run begins with.
pub(super) fn make(inputs: LooseInputs) -> LooseThings {
    let LooseInputs { mut unity, hard, db, options_state, options, rounds_drawn, mut away, ground, laid_from, controller, equipment, loot, range } = inputs;
    // (The battle screen's panels of a sight, the zeroing and the
    // magnification, whatever `TARKOV UI` says: read
    // with the health panel, above.)
    // The ragdoll the character becomes when it dies, and the figure its
    // bodies go by (`death`).
    let ragdoll = player::death::Ragdoll {
        data: match trp_data::character::ragdoll::RagdollData::load(&mut unity) {
            Ok(data) => Some(data),
            Err(e) => {
                eprintln!("the player's ragdoll could not be read; a corpse will stand where it died: {e:#}");
                None
            }
        },
        max_depenetration: hard.float("CorpseMaxDepenetrationVelocity").unwrap_or_else(|e| {
            eprintln!("{e:#}; the engine's own figure is used for a corpse");
            10.0
        }),
        physics: match trp_data::setup::engine::PhysicsSettings::load(&unity.install) {
            Ok(settings) => Some(settings),
            Err(e) => {
                eprintln!("the engine's physics settings could not be read; a corpse will stand where it died: {e:#}");
                None
            }
        },
    };
    // Things on the ground in the game's engine, with the options' `ITEM
    // PHYSICS` on (`loose`).
    let mut loose = world::loose::Loose::new(unity.install.clone(), db.clone(), ragdoll.physics.clone().filter(|_| options_state.in_force.item_physics), &hard, options.seed);
    // With `MAGAZINE ROUNDS`, a magazine's and a chamber's rounds on the
    // ground, drawn and with their colliders in its body.
    loose.rounds = rounds_drawn;
    // The game's step of its physics, at most `MaxSmoothPhysicsDeltaTime`.
    let physics_clock = player::death::PhysicsClock::new(hard.float("MaxSmoothPhysicsDeltaTime").unwrap_or(0.03));
    // The range's crates filled and hidden, to be searched (`SEARCHING`;
    // inventory spec, "Searching a container"); the run's dice for it.
    let mut searcher = trp_sim::items::search::Searcher::new(options.seed ^ 0x5ea7_c4ed);
    if options_state.in_force.searching {
        fill_crates(&unity.install, &db, &mut away, &mut searcher);
        // What `--lay` puts on the floor is the world's, not the
        // character's: a bag or a vest among it is not yet searched
        // (inventory spec, "Searching a body").
        for (item, _) in ground.iter().skip(laid_from) {
            searcher.hide_thing(item);
        }
    }
    // What the run begins with, for `Enter` after a death to put back.
    platform::run_log::phase("the ragdoll, the loose things, the crates filled");
    let start_of_run = player::death::StartOfRun {
        controller: controller.clone(),
        equipment: equipment.clone(),
        loot: loot.clone(),
        searcher: searcher.clone(),
        away: away.clone(),
        ground: ground.clone(),
        range: range.clone(),
    };
    LooseThings { ragdoll, loose, physics_clock, searcher, start_of_run }
}
