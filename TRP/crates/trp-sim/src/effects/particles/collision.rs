//! Particles meeting the world and the ray budgets for it.

use super::*;

impl ParticleSystem {
    /// Each particle meeting the world over its last move, as the engine
    /// meets it (`CollisionModule::Update`, `WorldCollision`,
    /// `PerformPlaneCollisions` and `PhysXRaycast::ParticleIntersect`,
    /// decompiled; particles spec, "Meeting the world"): a sphere of half
    /// the particle's size (its width) times the module's share
    /// (`CollisionModule::Update` halves `radiusScale`), less 0.151 of that, swept
    /// from where it was to where it is and 0.151 of that further; at the
    /// first surface, its place the sphere's middle there drawn back by the
    /// 0.151; the move past it and the velocity each reflected about the
    /// surface, kept by one less the dampen, and their normal parts kept by
    /// the bounce; its age put on by the lifetime's loss; killed where that
    /// ends its life, or where its speed coming in was below the least or
    /// above the most. A local system's particles are met in the world's
    /// axes and put back.
    /// Each from `first` to before `end`, over its own move's time
    /// (`dt_of`, by its place in the list). At medium and low quality the
    /// meeting is found otherwise (`cached_meetings`); what it does to the
    /// particle is the same.
    pub(super) fn collide(&mut self, first: usize, end: usize, dt_of: &dyn Fn(usize) -> f32, world: &dyn World, collision: &trp_data::drawing::particles::Collision) -> Vec<Particle> {
        let meetings = if collision.quality >= 1 { self.cached_meetings(first, end, dt_of, world, collision) } else { self.swept_meetings(first, end, dt_of, world, collision) };
        let placement = self.placement;
        let world_space = self.data.world_space;
        let mut met = Vec::new();
        for (p, meeting) in self.particles.iter_mut().skip(first).zip(meetings) {
            let Some((hit, n)) = meeting else { continue };
            let (position, velocity) = if world_space { (p.position, p.velocity) } else { (placement.point(p.position), placement.way(p.velocity)) };
            let share = p.share();
            let keep = 1.0 - collision.dampen.evaluate(share, draw(p.seed, DAMPEN));
            let kept_normal = 1.0 - collision.bounce.evaluate(share, draw(p.seed, BOUNCE));
            let loss = collision.lifetime_loss.evaluate(share, draw(p.seed, LIFETIME_LOSS));
            let reflect = |v: [f32; 3]| sub(v, scale(n, 2.0 * dot(n, v)));
            let respond = |v: [f32; 3]| {
                let kept = scale(reflect(v), keep);
                sub(kept, scale(n, kept_normal * dot(n, kept)))
            };
            let new_velocity = respond(velocity);
            let new_position = add(hit, respond(sub(position, hit)));
            let alive_before = p.age <= p.lifetime;
            let speed_squared = dot(velocity, velocity);
            p.age += loss * p.lifetime;
            if p.age > p.lifetime || speed_squared < collision.min_kill_speed * collision.min_kill_speed || speed_squared > collision.max_kill_speed * collision.max_kill_speed {
                p.age = p.lifetime * 1.000_001 + 1e-6;
            }
            if world_space {
                p.position = new_position;
                p.velocity = new_velocity;
            } else {
                p.position = placement.point_back(new_position);
                p.velocity = placement.way_back(new_velocity);
            }
            if alive_before {
                met.push(p.clone());
            }
        }
        met
    }

    /// A particle's place and velocity in the world.
    fn in_world(&self, p: &Particle) -> ([f32; 3], [f32; 3]) {
        if self.data.world_space { (p.position, p.velocity) } else { (self.placement.point(p.position), self.placement.way(p.velocity)) }
    }

    /// At high quality, each particle from `first` on swept over its own
    /// move: where it met a surface (in the world) and the surface's
    /// normal.
    fn swept_meetings(&self, first: usize, end: usize, dt_of: &dyn Fn(usize) -> f32, world: &dyn World, collision: &trp_data::drawing::particles::Collision) -> Vec<Option<([f32; 3], [f32; 3])>> {
        (first..end)
            .map(|i| {
                let p = &self.particles[i];
                let (position, velocity) = self.in_world(p);
                let from = sub(position, scale(velocity, dt_of(i)));
                let travel = sub(position, from);
                let distance = length(travel);
                if distance <= 1e-7 {
                    return None;
                }
                intersect(world, false, from, scale(travel, 1.0 / distance), distance, 0.5 * p.size * collision.radius_scale)
            })
            .collect()
    }

    /// At medium and low quality (`CollisionModule::Update`, `WorldCollision`,
    /// `ReadCache` and `PerformPlaneCollisions`, decompiled; particles spec,
    /// "Meeting the world at medium and low quality"), each particle from
    /// `first` on: those from the cursor on, as many as the frame's rays
    /// left, each sweep a sphere as the high quality does (the still
    /// shapes alone) from where it was along its way for four cubes'
    /// width, whatever its move, and keep what it met by its cube and way
    /// (the newest kept); the cursor moved on by the rays it had, wrapping
    /// to `first` once past the end; the others read the plane kept for
    /// their own cube and way. Each plane found (its own ray's too) is met
    /// where the particle's middle crossed it this step going against its
    /// normal; none otherwise. A particle's step time is its own for its
    /// ray; the reading and the crossing take theirs by the engine's lanes
    /// of four, which on a range not begun at a multiple of four (the
    /// born, four at a time) are another particle's (`lane_dt`).
    fn cached_meetings(&mut self, first: usize, end: usize, dt_of: &dyn Fn(usize) -> f32, world: &dyn World, collision: &trp_data::drawing::particles::Collision) -> Vec<Option<([f32; 3], [f32; 3])>> {
        let voxel = collision.voxel_size;
        // The step time in lane `slot` of the four begun at `base`: that
        // particle's, or the particle's own past the range's end
        // (`PROVISIONAL`: what the engine holds in a lane past the end is
        // not read).
        let lane_dt = |base: usize, slot: usize, own: usize| if base + slot < end { dt_of(base + slot) } else { dt_of(own) };
        let begin = if self.cubes.cursor < end { self.cubes.cursor.max(first) } else { first };
        let budget = self.cubes.budget.max(0) as usize;
        let rayed = begin..end.min(begin + budget);
        self.cubes.cursor = begin + budget;
        self.cubes.budget = cubes::four_up(self.cubes.budget - rayed.len() as i32);
        // The planes are kept in the system's space.
        let placement = self.placement;
        let world_space = self.data.world_space;
        let to_system = |plane: cubes::Plane| if world_space { plane } else { cubes::Plane { normal: placement.way_back(plane.normal), point: placement.point_back(plane.point) } };
        let to_world = |plane: cubes::Plane| if world_space { plane } else { cubes::Plane { normal: placement.way(plane.normal), point: placement.point(plane.point) } };
        let mut found: Vec<Option<cubes::Plane>> = vec![None; end - first];
        for i in rayed.clone() {
            let p = &self.particles[i];
            let (position, velocity) = self.in_world(p);
            let dt = dt_of(i);
            let travel = scale(velocity, dt);
            let from = sub(position, travel);
            let squared = dot(travel, travel);
            // A step under a millionth of a second casts none (counted as
            // rayed all the same); `PROVISIONAL`: nor does a particle that
            // has not moved (the engine sweeps nought long, not settled).
            if dt < 1e-6 || squared <= 1e-30 {
                continue;
            }
            let dir = scale(travel, 1.0 / squared.sqrt());
            let Some((point, normal)) = intersect(world, true, from, dir, (4.0 * voxel).max(voxel), 0.5 * p.size * collision.radius_scale) else { continue };
            let plane = cubes::Plane { normal, point };
            self.cubes.planes.insert(cubes::key(from, dir, voxel), to_system(plane));
            found[i - first] = Some(plane);
        }
        // Each of the two readings counts its lanes from its own start.
        for range in [first..rayed.start, rayed.end..end] {
            let start = range.start;
            for i in range {
                let (position, velocity) = self.in_world(&self.particles[i]);
                let travel = scale(velocity, lane_dt(first, (i - start) & 3, i));
                found[i - first] = self.cubes.planes.get(&cubes::key(sub(position, travel), travel, voxel)).copied().map(to_world);
            }
        }
        // The crossing, in the engine's order of sums: the move's part
        // along the normal (z, x, then y); the time where the middle meets
        // the plane as the difference of the two places' parts along it.
        let along = |n: [f32; 3], v: [f32; 3]| (n[2] * v[2] + n[0] * v[0]) + n[1] * v[1];
        found
            .into_iter()
            .enumerate()
            .map(|(k, plane)| {
                let plane = plane?;
                let i = first + k;
                let (position, velocity) = self.in_world(&self.particles[i]);
                let travel = scale(velocity, lane_dt(first + (k & !3), i & 3, i));
                let from = sub(position, travel);
                let d = along(plane.normal, travel);
                if d.abs() < f32::EPSILON || d >= 0.0 {
                    return None;
                }
                let t = -(along(plane.normal, from) - along(plane.normal, plane.point)) / d;
                (0.0..=1.0).contains(&t).then(|| (add(from, scale(travel, t)), plane.normal))
            })
            .collect()
    }
}

/// What a particle's sphere meets going from `from` along the unit way
/// `dir` for `distance` (`PhysXRaycast::ParticleIntersect`, decompiled;
/// particles spec, "Meeting the world"): a sphere of `r` less 0.151 of it
/// swept that far and 0.151 of `r` further (among the still shapes alone
/// where `still`); where it met, its middle drawn back by that 0.151, and
/// the surface's normal; or where it began inside a shape, pushed out
/// along the least way out by the depth and the 0.151, and that way
/// turned about, into the shape (a reflection is the same either way; the
/// crossing of medium and low quality is not).
fn intersect(world: &dyn World, still: bool, from: [f32; 3], dir: [f32; 3], distance: f32, r: f32) -> Option<([f32; 3], [f32; 3])> {
    let pull = r * 0.151;
    let radius = (r - pull).max(1e-6);
    let (t, n) = if still { world.sweep_sphere_still(from, dir, distance + pull, radius) } else { world.sweep_sphere(from, dir, distance + pull, radius) }?;
    if t == 0.0 {
        return None;
    }
    Some(if t > 0.0 { (add(from, scale(dir, t - pull)), n) } else { (add(from, scale(n, pull - t)), scale(n, -1.0)) })
}

/// The frame's rays for meeting the world at medium and low quality shared
/// among the systems, once a frame before any steps
/// (`ParticleSystem::AssignRayBudgets`, decompiled; particles spec,
/// "Meeting the world at medium and low quality"): each system's share
/// nought; a low system takes part one frame in five (a count of four
/// frames, begun again when its quality was first seen); the systems that
/// take part are those alive, meeting the world, at medium or low quality,
/// on their frame; each gets the lesser of an even share of `total` and
/// its particles, then, in the same order, the lesser of what it still
/// lacks and what is left. `PROVISIONAL`: the order (the engine's list of
/// the systems it updates, not read) is the caller's; alive stands for
/// the engine's list.
pub fn assign_ray_budgets(systems: &mut [&mut ParticleSystem], total: u32) {
    let mut listed: Vec<usize> = Vec::new();
    for (i, s) in systems.iter_mut().enumerate() {
        s.cubes.assigned = 0;
        let Some(quality) = s.data.collision.as_ref().filter(|c| c.world).map(|c| c.quality) else { continue };
        // Only a system in the engine's list is looked at.
        if quality == 0 || !s.alive() {
            continue;
        }
        let period = if quality == 2 { 4 } else { 0 };
        if s.cubes.seen != Some(quality) {
            s.cubes.seen = Some(quality);
            s.cubes.countdown = period;
        }
        let ready = s.cubes.countdown == 0;
        s.cubes.countdown = if ready { period } else { s.cubes.countdown - 1 };
        if ready {
            listed.push(i);
        }
    }
    if listed.is_empty() {
        return;
    }
    let share = total / listed.len() as u32;
    let mut left = total;
    for &i in &listed {
        let s = &mut systems[i];
        let take = share.min(s.particles.len() as u32);
        s.cubes.assigned = take;
        left = left.saturating_sub(take);
    }
    for &i in &listed {
        let s = &mut systems[i];
        let more = (s.particles.len() as u32).saturating_sub(s.cubes.assigned).min(left);
        s.cubes.assigned += more;
        left -= more;
    }
}
