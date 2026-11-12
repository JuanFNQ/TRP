//! Shooting bullets, ticking their flight, handling hits and the linecasts.

use super::*;

impl Ballistics {
    pub fn new(degradation: f32, gravity: V3) -> Self {
        Self { tables: PrecalculatedRandoms::ballistics(), degradation, gravity, bullets: Vec::new(), hits_scratch: Vec::new() }
    }

    pub fn tables(&self) -> &PrecalculatedRandoms {
        &self.tables
    }

    pub fn bullets(&self) -> &[Bullet] {
        &self.bullets
    }

    /// A shot's bullets (`InitiateShot`): one, or for a round of several
    /// projectiles as many, each spread by the weapon's shotgun dispersion.
    /// `random` is Unity's own generator (the seeds, the pellets' ways);
    /// `speed_factor` the weapon's.
    pub fn shoot(&mut self, shot: u32, round: &std::sync::Arc<Round>, origin: V3, direction: V3, speed_factor: f32, shotgun_dispersion: f32, random: &mut UnityRandom) {
        if round.initial_speed <= 0.0 {
            return;
        }
        if round.projectiles == 1 {
            let seed = random.range_int(0, 512);
            self.create(shot, round, 0, seed, origin, direction, speed_factor);
        } else {
            for i in 0..round.projectiles {
                let pick = random.range_int(0, 512);
                let way = normalized(add(scale(direction, 100.0), scale(self.tables.direction(pick), shotgun_dispersion)));
                let seed = random.range_int(0, 512);
                self.create(shot, round, i * 2, seed, origin, way, speed_factor);
            }
        }
    }

    /// A bullet as a shot makes it (`CreateShot`), its seed given: for a
    /// replay, the recorded one.
    pub fn create(&mut self, shot: u32, round: &std::sync::Arc<Round>, fragment: i32, seed: i32, origin: V3, direction: V3, speed_factor: f32) {
        let speed = round.initial_speed * speed_factor;
        let penetration = self.tables.normal(seed, round.penetration_power, round.penetration_deviation);
        let bullet = self.made(
            shot,
            round,
            Made {
                fragment,
                seed,
                origin,
                direction,
                initial_speed: speed,
                speed,
                damage: round.damage,
                penetration,
                penetration_chance: round.penetration_chance,
                ricochet_chance: round.ricochet_chance,
                fragmentation_chance: round.fragmentation_chance,
                deviation_chance: 1.0,
                min_fragments: round.min_fragments,
                max_fragments: round.max_fragments,
            },
            None,
        );
        self.bullets.push(bullet);
    }

    pub(super) fn made(&self, shot: u32, round: &std::sync::Arc<Round>, m: Made, parent: Option<&Bullet>) -> Bullet {
        let velocity = scale(m.direction, m.speed);
        let (depth, ricochets, before) = match parent {
            Some(p) => {
                let mut before = p.before.clone();
                before.push((p.hit_thing, p.hit_collider));
                (p.depth + 1, p.ricochets + u32::from(p.state == State::RicochetHit), before)
            }
            None => (0, 0, Vec::new()),
        };
        let (mut forward_parts, mut characters) = parent.map_or((Vec::new(), Vec::new()), |p| (p.forward_parts.clone(), p.characters.clone()));
        if let Some(body) = parent.and_then(|p| p.hit_body) {
            if parent.is_some_and(|p| p.hit_forward) {
                forward_parts.push(body);
            }
            if !characters.contains(&body.character) {
                characters.push(body.character);
            }
        }
        Bullet {
            shot,
            fragment: m.fragment,
            seed: m.seed,
            depth,
            ricochets,
            before,
            round: round.clone(),
            direction: m.direction,
            initial_speed: m.initial_speed,
            speed: m.speed,
            damage: m.damage,
            penetration: m.penetration,
            penetration_chance: m.penetration_chance,
            ricochet_chance: m.ricochet_chance,
            fragmentation_chance: m.fragmentation_chance,
            deviation_chance: m.deviation_chance,
            min_fragments: m.min_fragments,
            max_fragments: m.max_fragments,
            armor_damage: round.armor_damage,
            life: round.life,
            age: 0.0,
            position: m.origin,
            previous: m.origin,
            steps: 0,
            velocity,
            trajectory: Trajectory::new(m.origin, velocity, round.mass_g, round.diameter_mm, round.ballistic_coefficient, self.gravity),
            state: State::Flying,
            done: false,
            hit_thing: 0,
            hit_collider: 0,
            deflected_by: None,
            blocked_by: None,
            forward_parts,
            characters,
            hit_body: None,
            hit_forward: false,
            no_damage: false,
        }
    }

    /// One frame (`UpdateShots`): every bullet in flight moved on by `dt`,
    /// each hit handled and the bullets it makes moved on by what is left of
    /// the frame. The hits in the order they came.
    pub fn tick(&mut self, dt: f32, world: &mut impl BulletWorld) -> Vec<Hit> {
        self.bullets.retain(|b| !b.finished());
        let mut queue: std::collections::VecDeque<(usize, f32)> = (0..self.bullets.len()).map(|i| (i, dt)).collect();
        let mut hits = Vec::new();
        while let Some((index, time)) = queue.pop_front() {
            let Some((hit, children, left)) = self.update(index, time, &*world) else { continue };
            world.delivered(&hit);
            hits.push(hit);
            for child in children {
                self.bullets.push(child);
                queue.push_back((self.bullets.len() - 1, left));
            }
        }
        hits
    }

    /// `Shot.Tick` and `Update`: the bullet moved on; a hit handled.
    fn update(&mut self, index: usize, dt: f32, world: &impl BulletWorld) -> Option<(Hit, Vec<Bullet>, f32)> {
        let bullet = &mut self.bullets[index];
        if bullet.finished() {
            return None;
        }
        let (previous, previous_velocity) = (bullet.position, bullet.velocity);
        bullet.age += dt;
        (bullet.position, bullet.velocity) = bullet.trajectory.at(bullet.age);
        bullet.previous = previous;
        bullet.steps += 1;
        let asker = Asker { shot: bullet.shot, fragment: bullet.fragment, seed: bullet.seed, forward: true };
        let mut scratch = std::mem::take(&mut self.hits_scratch);
        let found = linecast_both_ways(world, asker, previous, self.bullets[index].position, &self.bullets[index].before, &mut scratch);
        self.hits_scratch = scratch;
        let (hit, forward) = found?;
        Some(self.handle(index, dt, previous, previous_velocity, hit, forward, world))
    }

    /// `HandleCollision` and `CreateFragments`.
    #[allow(clippy::too_many_arguments)]
    fn handle(&mut self, index: usize, dt: f32, previous: V3, previous_velocity: V3, hit: RayHit, forward: bool, world: &impl BulletWorld) -> (Hit, Vec<Bullet>, f32) {
        let degradation = self.degradation;
        let bullet = &mut self.bullets[index];
        let past = magnitude(sub(bullet.position, hit.point));
        let segment = sub(bullet.position, previous);
        let share = past / magnitude(segment);
        let left = dt * share;
        bullet.age -= left;
        let mut speed = magnitude(bullet.velocity);
        let before = magnitude(previous_velocity);
        speed -= (speed - before) * share;
        bullet.velocity = scale(normalized(bullet.velocity), speed);
        bullet.hit_thing = hit.thing;
        bullet.hit_collider = hit.collider;
        bullet.hit_body = hit.body;
        bullet.hit_forward = forward;
        // `AvoidAdditionalDamageWhereCollidersIntersect`.
        if let Some(body) = hit.body {
            bullet.no_damage = bullet.forward_parts.iter().any(|p| p.same_part(&body)) && bullet.characters.contains(&body.character);
        }
        let cos = dot(hit.normal, normalized(segment));
        if forward {
            let r = clamp01(magnitude(bullet.velocity) / bullet.initial_speed);
            let k = r + (1.0 - r) * clamp01(degradation);
            bullet.damage *= k;
            bullet.penetration *= k;
        }
        bullet.done = true;
        let way = normalized(segment);
        let children = self.fragments(index, &hit, forward, cos, way, world);
        let b = &self.bullets[index];
        let record = Hit {
            shot: b.shot,
            fragment: b.fragment,
            seed: b.seed,
            depth: b.depth,
            point: hit.point,
            normal: hit.normal,
            forward,
            thing: hit.thing,
            collider: hit.collider,
            surface: hit.surface,
            state: b.state,
            speed: b.speed,
            velocity: b.velocity,
            damage: b.damage,
            penetration: b.penetration,
            armor_damage: b.armor_damage,
            cos,
            age: b.age,
            fragments: children.len(),
            out_of_life: b.age > b.life,
            first_hit: b.fragment == 0 && b.depth <= 1,
            body: hit.body,
            deflected_by: b.deflected_by,
            blocked_by: b.blocked_by,
            no_damage: b.no_damage,
            direction: b.direction,
            round: b.round.clone(),
        };
        (record, children, left)
    }
}

/// What a bullet is made with.
pub(super) struct Made {
    pub(super) fragment: i32,
    pub(super) seed: i32,
    pub(super) origin: V3,
    pub(super) direction: V3,
    pub(super) initial_speed: f32,
    pub(super) speed: f32,
    pub(super) damage: f32,
    pub(super) penetration: f32,
    pub(super) penetration_chance: f32,
    pub(super) ricochet_chance: f32,
    pub(super) fragmentation_chance: f32,
    pub(super) deviation_chance: f32,
    pub(super) min_fragments: i32,
    pub(super) max_fragments: i32,
}

/// Unity's `Linecast` of the game's (`PhysicsExtensions.Linecast`): of the
/// ray's hits not ignored, the nearest (or, `reverse`, the farthest).
fn linecast(world: &impl BulletWorld, asker: Asker, from: V3, to: V3, reverse: bool, before: &[(u64, u64)], hits: &mut Vec<RayHit>) -> Option<RayHit> {
    hits.clear();
    world.ray(asker, from, to, hits);
    let reach = magnitude(sub(to, from));
    let mut best: Option<RayHit> = None;
    for hit in hits.iter().filter(|h| h.distance <= reach) {
        // `IsIgnoreSameCollider`: another collider of a thing one before
        // it hit.
        if before.iter().any(|&(thing, collider)| thing == hit.thing && hit.collider != collider) {
            continue;
        }
        let better = match best {
            None => true,
            Some(b) => if reverse { hit.distance > b.distance } else { hit.distance < b.distance },
        };
        if better {
            best = Some(*hit);
        }
    }
    best
}

/// `LinecastInBothSides`: forward from the segment's start, backward from
/// its end (through a mesh up to eight times); the nearer to the start, and
/// whether it was the forward one.
fn linecast_both_ways(world: &impl BulletWorld, asker: Asker, start: V3, end: V3, before: &[(u64, u64)], hits: &mut Vec<RayHit>) -> Option<(RayHit, bool)> {
    let ahead = linecast(world, asker, start, end, false, before, hits);
    let asker = Asker { forward: false, ..asker };
    let mut behind: Option<RayHit> = None;
    let mut from = end;
    let mut nudge = [0.0; 3];
    for i in 0..8 {
        let Some(found) = linecast(world, asker, from, start, true, before, hits) else { break };
        behind = Some(found);
        if !found.mesh {
            break;
        }
        if i == 0 {
            nudge = scale(normalized(sub(start, end)), 0.001);
        }
        from = add(found.point, nudge);
    }
    match (ahead, behind) {
        (Some(a), Some(b)) => {
            if a.distance < magnitude(sub(b.point, start)) {
                Some((a, true))
            } else {
                Some((b, false))
            }
        }
        (Some(a), None) => Some((a, true)),
        (None, Some(b)) => Some((b, false)),
        (None, None) => None,
    }
}
