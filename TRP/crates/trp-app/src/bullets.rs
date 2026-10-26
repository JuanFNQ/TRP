//! The shots' bullets on the range (`docs/spec/ballistics.md`; the
//! options' `BALLISTICS`): each shot made from the weapon's fireport as the
//! game makes it (moved back by a fifth of the weapon's length, squeezed for
//! the wide field of view, spread by the weapon and the round), flown by
//! `trp_sim::ballistics` against the range's surfaces (`Range::bullet_hits`)
//! every frame. A shot of automatic fire moves on in its own frame, a single
//! shot the next, as the game's two ways of firing have it.

use std::collections::HashMap;
use std::sync::Arc;

use bevy::prelude::*;
use bevy::transform::helper::TransformHelper;
use trp_sim::ballistics::{iron_shot_direction, spread, squeezed, Asker, Ballistics, BulletWorld, Hit, RayHit, Round, WeaponFigures, V3};
use trp_sim::impacts::{Commutator, Impact};
use trp_sim::unity_random::UnityRandom;

use crate::controller::Controller;
use crate::convert;
use crate::inventory_view::{Carried, HandsHeld};
use crate::range::Range;
use crate::weapon::{HandsRig, WeaponRig};

/// The fireport's own down: the way a shot leaves it with the sights down
/// (`Vector3.down`).
const DOWN: V3 = [0.0, -1.0, 0.0];

/// The ballistics calculator of the range, and what it goes by.
#[derive(Resource)]
pub struct Bullets {
    sim: Ballistics,
    /// Unity's own generator, for what the game draws from it: each
    /// bullet's seed, a pellet's way, the spread.
    random: UnityRandom,
    presets: Vec<trp_data::hard_settings::BallisticPreset>,
    rounds: HashMap<String, Option<Arc<Round>>>,
    /// The weapon in hand's figures, by its item.
    figures: Option<(trp_sim::inventory::Id, Option<WeaponFigures>)>,
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
    pub entries: Vec<trp_data::particles::MaterialEntry>,
    decal_shift: f32,
    pub ricochets: Vec<[f32; 3]>,
    pub effects: Vec<(Impact, usize)>,
}

impl Bullets {
    /// What a line of sight from `from` to `to` (the simulation's axes)
    /// meets of the range itself: its boxes, doors and ground, by the
    /// presets the bullets take (the rangefinder's ray, `rangefinder_view`).
    pub fn sight_hits(&self, range: &Range, from: V3, to: V3, hits: &mut Vec<RayHit>) {
        range.bullet_hits(from, to, &self.presets, hits);
    }

    pub fn new(hard: &trp_data::hard_settings::HardSettings, db: &trp_data::Database, gravity: V3, entries: Vec<trp_data::particles::MaterialEntry>) -> Option<Self> {
        let decal_shift = hard.float("DECAL_SHIFT").ok()?;
        let presets = hard.ballistic_presets().ok()?.to_vec();
        let degradation = db.config.get("Ballistic").and_then(|b| b.get("GlobalDamageDegradationCoefficient")).and_then(|v| v.as_f64())? as f32;
        let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |d| d.subsec_nanos() as i32);
        Some(Self {
            sim: Ballistics::new(degradation, gravity),
            random: UnityRandom::new(seed),
            presets,
            rounds: HashMap::new(),
            figures: None,
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
        })
    }

    /// The effects' entry a material plays (`Effects._dict`: the first entry
    /// naming it), if any.
    pub fn entry_of(&self, material: i32) -> Option<usize> {
        self.entries.iter().position(|e| e.materials.contains(&material))
    }

    fn round(&mut self, db: &trp_data::Database, template: &str) -> Option<Arc<Round>> {
        self.rounds.entry(template.to_owned()).or_insert_with(|| db.items.get(template).and_then(Round::of).map(Arc::new)).clone()
    }
}

/// The range as the bullets' world, and the dummy on it (`dummy`).
struct RangeWorld<'a> {
    range: &'a Range,
    presets: &'a [trp_data::hard_settings::BallisticPreset],
    dummy: Option<&'a mut crate::dummy::Dummies>,
    poppers: Option<&'a crate::poppers::Poppers>,
}

impl BulletWorld for RangeWorld<'_> {
    fn ray(&self, _: Asker, from: V3, to: V3, hits: &mut Vec<RayHit>) {
        self.range.bullet_hits(from, to, self.presets, hits);
        if let Some(dummy) = &self.dummy {
            dummy.ray(from, to, hits);
        }
        if let Some(poppers) = self.poppers {
            poppers.ray(from, to, hits);
        }
    }

    fn wearer(&self, character: u32) -> Option<&trp_sim::armor::Wearer> {
        self.dummy.as_ref().and_then(|dummies| dummies.wearer(character))
    }

    fn delivered(&mut self, hit: &Hit) {
        if let Some(dummy) = self.dummy.as_mut() {
            dummy.take(hit);
        }
    }
}

/// Every part of a weapon's build, depth first in its slots' order (the
/// game's `Mods`: its slots' contents, and theirs), but the rounds in its
/// chambers.
fn parts(item: &trp_sim::inventory::Item, out: &mut Vec<String>) {
    for (slot, kind) in item.slots.iter().zip(&item.kind.slots) {
        let Some(part) = slot else { continue };
        if kind.name.starts_with("patron_in_weapon") {
            continue;
        }
        out.push(part.kind.template.clone());
        parts(part, out);
    }
}

/// A transform's place as drawn and its turn as Unity keeps it (the product
/// of the turns above it, whatever the scales), in Unity's axes.
fn unity_pose(entity: Entity, helper: &TransformHelper, transforms: &Query<(&Transform, Option<&ChildOf>)>) -> Option<(V3, [f32; 4])> {
    let at = helper.compute_global_transform(entity).ok()?.translation();
    let mut turn = Quat::IDENTITY;
    let mut next = Some(entity);
    while let Some(e) = next {
        let (transform, parent) = transforms.get(e).ok()?;
        turn = transform.rotation * turn;
        next = parent.map(ChildOf::parent);
    }
    Some(([at.x, at.y, -at.z], convert::rotation_back(turn)))
}

/// The frame's bullets: the shots' made, every bullet moved on.
#[allow(clippy::too_many_arguments)]
pub fn fly_bullets(
    mut bullets: ResMut<Bullets>,
    mut controller: ResMut<Controller>,
    hands: Option<Res<HandsRig>>,
    carried: Res<Carried>,
    held: Res<HandsHeld>,
    game: Res<crate::GameData>,
    weapon: Option<Res<WeaponRig>>,
    range: Res<Range>,
    time: Res<Time>,
    helper: TransformHelper,
    transforms: Query<(&Transform, Option<&ChildOf>)>,
    cameras: Query<(&Camera, &GlobalTransform), With<crate::MainCamera>>,
    mut dummy: Option<ResMut<crate::dummy::Dummies>>,
    poppers: Option<Res<crate::poppers::Poppers>>,
) {
    let bullets = &mut *bullets;
    // A new frame for the hits' places (the commutator's late update).
    bullets.commutator.new_frame();
    let calls = std::mem::take(&mut controller.bullet_calls);
    let gravity = game.engine.gravity;
    // The weapon in hand's figures, read again when it changes.
    let item = held.item.and_then(|id| carried.find(id));
    if bullets.figures.as_ref().map(|(id, _)| *id) != item.map(|item| item.id) {
        bullets.figures = item.map(|item| {
            let mut list = Vec::new();
            parts(item, &mut list);
            let refs: Vec<&str> = list.iter().map(String::as_str).collect();
            (item.id, WeaponFigures::of(&game.db, &item.kind.template, &refs))
        });
    }
    let figures = bullets.figures.as_ref().and_then(|(_, f)| f.clone());
    // The shot's way from the fireport: worked out for the iron sights as
    // they come up, down again as they go (`method_14`, `method_12`).
    let aiming = hands.as_ref().is_some_and(|h| h.aiming);
    if aiming && !bullets.was_aiming {
        let range_of = figures.as_ref().map_or(0, |f| f.iron_sight_range);
        let point = controller.rig_pose().sight_points.and_then(|points| trp_sim::rig::iron_point(points, range_of));
        let default = figures.as_ref().and_then(|f| f.default_round.clone()).and_then(|t| bullets.round(&game.db, &t));
        bullets.local_way = match (point, default, figures.as_ref()) {
            (Some(point), Some(round), Some(f)) => iron_shot_direction(point, &round, f.speed_factor, f.iron_sight_range, gravity),
            _ => DOWN,
        };
    } else if !aiming {
        bullets.local_way = DOWN;
    }
    bullets.was_aiming = aiming;

    // Each shot's bullets, from where the fireport is drawn this frame.
    let made = |bullets: &mut Bullets, automatic_wanted: bool| {
        for _ in calls.iter().filter(|&&a| a == automatic_wanted) {
            let (Some(rig), Some(hands), Some(f)) = (weapon.as_ref(), hands.as_ref(), figures.as_ref()) else { continue };
            let Some(fireport) = rig.fireport else { continue };
            let Some(template) = crate::cases::fired_round(&carried, &held, &game.db) else { continue };
            let Some(round) = bullets.round(&game.db, &template) else { continue };
            let Some((at, turn)) = unity_pose(fireport, &helper, &transforms) else { continue };
            let way = trp_sim::math::quat_rotate(turn, bullets.local_way);
            let back = hands.hands.settings.weapon_length / 5.0;
            let mut start: V3 = std::array::from_fn(|k| at[k] - way[k] * back);
            let mut way = way;
            // The wide field of view's squeeze, about the hands' root; not
            // once a scope's optic has been turned to its zeroing point
            // (`ShotNeedsFovAdjustments`; optics spec, "Zeroing").
            let squeeze = hands.hands.fov_scale();
            if squeeze < 1.0 && hands.hands.shot_squeeze() {
                if let Some((root_at, root_turn)) = unity_pose(rig.root, &helper, &transforms) {
                    (start, way) = squeezed(start, way, root_at, root_turn, squeeze);
                }
            }
            let unspread = way;
            // With the weapon's wear and heat (ballistics spec, "Spread";
            // malfunctions spec): its durability as its item has it, the
            // last shot's heat as its logic has it.
            let durability = item.map_or(100.0, |item| item.durability()[0]);
            let heat = hands.firearm.as_ref().map_or(1.0, trp_sim::Firearm::heat_spread);
            let amount = f.spread_worn(&round, durability, heat);
            let way = spread(way, amount, &mut bullets.random);
            bullets.shots += 1;
            if bullets.shots <= 8 {
                info!(
                    "shot {}: from ({:.3}, {:.3}, {:.3}), the fireport at ({:.3}, {:.3}, {:.3}), its way ({:.4}, {:.4}, {:.4}) spread to ({:.4}, {:.4}, {:.4}); the fireport's own way ({:.4}, {:.4}, {:.4}), squeeze {:.3}, spread {:.4}",
                    bullets.shots + 0, start[0], start[1], start[2], at[0], at[1], at[2], unspread[0], unspread[1], unspread[2], way[0], way[1], way[2],
                    bullets.local_way[0], bullets.local_way[1], bullets.local_way[2], squeeze, amount
                );
            }
            let shot = bullets.shots;
            bullets.sim.shoot(shot, &round, start, way, f.speed_factor, f.shotgun_dispersion, &mut bullets.random);
        }
    };
    made(bullets, true);
    let mut world = RangeWorld { range: &range, presets: &bullets.presets, dummy: dummy.as_deref_mut(), poppers: poppers.as_deref() };
    bullets.hits = bullets.sim.tick(time.delta_secs().min(1.0), &mut world);
    made(bullets, false);

    // What each hit plays: the effect queued by its material's entry, ten
    // a frame; a ricochet's own sound at once (`PlayHitEffect`).
    // PROVISIONAL: played in the frame they were queued (the game's effects'
    // update may come before or after the world's tick).
    let camera = cameras.iter().next();
    let in_view = |point: V3| -> bool {
        let Some((camera, at)) = camera else { return false };
        let p = Vec3::new(point[0], point[1], -point[2]);
        if at.affine().inverse().transform_point3(p).z >= 0.0 {
            return false;
        }
        camera.world_to_ndc(at, p).is_some_and(|ndc| {
            let (x, y) = ((ndc.x + 1.0) * 0.5, (ndc.y + 1.0) * 0.5);
            x > -0.01 && x < 1.01 && y > -0.01 && y < 1.01
        })
    };
    let mut queue = Vec::new();
    bullets.ricochets.clear();
    for hit in &bullets.hits {
        // A hit the dummy took plays the material it was.
        let taken = dummy.as_ref().and_then(|dummy| dummy.taken_by(hit));
        let Some(impact) = bullets.commutator.hit(hit, bullets.decal_shift, true, taken, in_view) else { continue };
        if impact.ricochet_sound {
            bullets.ricochets.push(impact.position);
        }
        if let Some(entry) = bullets.entry_of(impact.material) {
            queue.push((impact, entry));
        }
    }
    queue.truncate(trp_sim::impacts::MAX_EFFECTS_PER_FRAME);
    bullets.effects = queue;

    for hit in bullets.hits.iter().take(64usize.saturating_sub(bullets.logged)) {
        let name = trp_sim::ballistics::material::NAMES.get(hit.surface.material as usize).copied().unwrap_or("?");
        info!(
            "a bullet of shot {} (fragment {}, seed {}) met {} {} at ({:.2}, {:.2}, {:.2}) at {:.0} m/s: {}, {} following",
            hit.shot,
            hit.fragment,
            hit.seed,
            name,
            if hit.forward { "going in" } else { "coming out" },
            hit.point[0],
            hit.point[1],
            hit.point[2],
            (hit.velocity[0].powi(2) + hit.velocity[1].powi(2) + hit.velocity[2].powi(2)).sqrt(),
            hit.state.name(),
            hit.fragments
        );
    }
    bullets.logged += bullets.hits.len();
}
