//! The shots' bullets on the range.
//! After `docs/spec/ballistics.md`; the options' `BALLISTICS`: each shot made from the weapon's
//! fireport as the game makes it (moved back by a fifth of the weapon's length, squeezed for the
//! wide field of view, spread by the weapon and the round), flown by `trp_sim::ballistics` against
//! the range's surfaces (`Range::bullet_hits`) every frame. A shot of automatic fire moves on in
//! its own frame, a single shot the next, as the game's two ways of firing have it. A grenade's
//! fragments (`trp_sim::fragments`) are made from its blast as the game makes them, nine a frame
//! after the first, each moving on the frame after it is made (`courses3`'s `3-bot-blast`: every
//! one of 270 the game's), and never meet the thrower, whose body the range's world does not hold.
//! An underbarrel launcher's grenade is a shot as any other, made from the launcher's own fireport
//! straight along its barrel (`LauncherShot`), and an explosive round's hit that sets off its fuse
//! blasts where it hit, its fragments made at once (underbarrel-launcher spec, "The grenade").

use std::collections::HashMap;
use std::sync::Arc;

use bevy::prelude::*;
use bevy::transform::helper::TransformHelper;
use trp_sim::combat::ballistics::{iron_shot_direction, spread, squeezed, Asker, Ballistics, BulletWorld, Hit, RayHit, Round, WeaponFigures, V3};
use trp_sim::combat::impacts::{Commutator, Impact};
use trp_sim::base::unity_random::UnityRandom;

use crate::player::controller::Controller;
use crate::platform::convert;
use crate::screens::inventory_view::{Carried, HandsHeld};
use crate::world::range::Range;
use crate::weapons::weapon::{HandsRig, WeaponRig};

mod flight;

pub use flight::*;

/// The fireport's own down: the way a shot leaves it with the sights down
/// (`Vector3.down`).
const DOWN: V3 = [0.0, -1.0, 0.0];

/// A shot's call for its bullets (`Controller::bullet_calls`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BulletCall {
    /// Fired by automatic fire, before the frame's bullets move on, or by
    /// the animation's event, after.
    pub automatic: bool,
    /// A cylinder's hammer was down as the shot was made: its spread takes
    /// the double action's penalty.
    pub hammer_down: bool,
}

/// The ballistics calculator of the range, and what it goes by.
#[derive(Resource)]
pub struct Bullets {
    sim: Ballistics,
    /// Unity's own generator, for what the game draws from it: each
    /// bullet's seed, a pellet's way, the spread.
    random: UnityRandom,
    presets: Vec<trp_data::setup::hard_settings::BallisticPreset>,
    rounds: HashMap<String, Option<Arc<Round>>>,
    /// The weapon in hand's figures, by its item and its parts (a part
    /// changed at a bench reads them again).
    figures: Option<((trp_sim::items::inventory::Id, Vec<String>), Option<WeaponFigures>)>,
    /// Its underbarrel launcher's, by the launcher's template
    /// (`WeaponFigures::of_launcher`).
    launcher_figures: Option<(String, Option<WeaponFigures>)>,
    shots: u32,
    /// The way a shot leaves the fireport, in its own axes: down, or the
    /// iron sights' while they are up (taken as they come up).
    local_way: V3,
    was_aiming: bool,
    /// This frame's hits, in the order they came.
    pub hits: Vec<Hit>,
    /// How many hits have been written to the log.
    logged: usize,
    /// What the hits play (ballistics spec, "What a hit shows and sounds"):
    /// the frame's places played, the effects' entries that name materials
    /// (a material's the first naming it), the hard settings' shift along
    /// the normal; this frame's ricochets' places (their own sound) and its
    /// effects played, each with its entry.
    commutator: Commutator,
    pub entries: Vec<trp_data::drawing::particles::MaterialEntry>,
    decal_shift: f32,
    pub ricochets: Vec<[f32; 3]>,
    pub effects: Vec<(Impact, usize)>,
    /// How far behind a body's hit blood is looked for
    /// (`DRAW_BLOOD_ON_WALLS_MAX_DISTANCE`), and this frame's blood found
    /// there (`EmitBloodOnEnvironment`): each place and normal, in the
    /// order of the hits, laid before the frame's holes (`decal_view`).
    blood_reach: f32,
    pub wall_blood: Vec<([f32; 3], [f32; 3])>,
    /// The grenades' table, the run's fragment counter
    /// (`GlobalFragmentNumber`, from nought as the game's starts), and the
    /// fragments still to be made: the frames left, the round, the blast's
    /// place, the way and the speed's share.
    grenade_table: trp_sim::base::unity_random::PrecalculatedRandoms,
    fragment_number: i32,
    pending: Vec<(usize, Arc<Round>, V3, V3, f32)>,
}

impl Bullets {
    /// What a line of sight from `from` to `to` (the simulation's axes)
    /// meets of the range itself: its boxes, doors and ground, by the
    /// presets the bullets take (the rangefinder's ray, `rangefinder_view`).
    pub fn sight_hits(&self, range: &Range, from: V3, to: V3, hits: &mut Vec<RayHit>) {
        range.bullet_hits(from, to, &self.presets, hits);
    }

    pub fn new(hard: &trp_data::setup::hard_settings::HardSettings, db: &trp_data::Database, gravity: V3, entries: Vec<trp_data::drawing::particles::MaterialEntry>) -> Option<Self> {
        let decal_shift = hard.float("DECAL_SHIFT").ok()?;
        let blood_reach = hard.float("DRAW_BLOOD_ON_WALLS_MAX_DISTANCE").ok()?;
        let presets = hard.ballistic_presets().ok()?.to_vec();
        let degradation = db.config.get("Ballistic").and_then(|b| b.get("GlobalDamageDegradationCoefficient")).and_then(|v| v.as_f64())? as f32;
        let seed = crate::fixed_seed(0xb011).map(|seed| seed as i32).unwrap_or_else(|| std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |d| d.subsec_nanos() as i32));
        Some(Self {
            sim: Ballistics::new(degradation, gravity),
            random: UnityRandom::new(seed),
            presets,
            rounds: HashMap::new(),
            figures: None,
            launcher_figures: None,
            shots: 0,
            local_way: DOWN,
            was_aiming: false,
            hits: Vec::new(),
            logged: 0,
            commutator: Commutator::default(),
            entries,
            decal_shift,
            ricochets: Vec::new(),
            effects: Vec::new(),
            blood_reach,
            wall_blood: Vec::new(),
            grenade_table: trp_sim::combat::fragments::grenade_randoms(),
            fragment_number: 0,
            pending: Vec::new(),
        })
    }

    /// The bullets in flight as the frame's tick left them (and those of a
    /// single shot made after it, not yet moved): what the tracers are drawn
    /// from (`tracer_view`).
    pub fn in_flight(&self) -> &[trp_sim::combat::ballistics::Bullet] {
        self.sim.bullets()
    }

    /// The effects' entry a material plays (`Effects._dict`: the first entry
    /// naming it), if any.
    pub fn entry_of(&self, material: i32) -> Option<usize> {
        self.entries.iter().position(|e| e.materials.contains(&material))
    }

    fn round(&mut self, db: &trp_data::Database, template: &str) -> Option<Arc<Round>> {
        self.rounds.entry(template.to_owned()).or_insert_with(|| db.items.get(template).and_then(Round::of).map(Arc::new)).clone()
    }

    /// A blast's fragments (`ExplosionSharedMethods.Explosion`): aimed at the
    /// characters within its reach whose feet are given, then at random, the
    /// blast on the ground or in the air by the range below it; each a shot
    /// of its own, made at a frame's end nine a frame (`pending`).
    fn blast_fragments(&mut self, db: &trp_data::Database, grenade: &crate::effects::blast_view::Grenade, at: V3, feet: &[V3], range: &Range) {
        let Some(round) = self.round(db, &grenade.round) else { return };
        let below = trp_sim::World::ray(range, at, [0.0, -1.0, 0.0], trp_sim::combat::fragments::GROUND_REACH, trp_sim::weapons::mounting::Layer::Coarse).map(|hit| hit.distance);
        let lie = trp_sim::combat::fragments::Lie::of(below);
        let made = trp_sim::combat::fragments::fragments(grenade.fragments, &mut self.fragment_number, &self.grenade_table, at, feet, lie);
        info!("a blast's {} fragments ({} aimed, the blast {lie:?}), the fragment counter now {}", made.len(), made.iter().filter(|f| f.aimed.is_some()).count(), self.fragment_number);
        self.pending.extend(made.iter().map(|f| (f.frame, round.clone(), at, f.way, f.speed_factor)));
    }
}
