//! Stepping particles, their trails, lights and what is drawn.

use super::*;

impl ParticleSystem {
    /// The time passed: the system's own emission; each particle aged, those
    /// at the end of their life gone as the engine takes them away, the rest
    /// forced, limited and dragged, moved and turned.
    pub fn step(&mut self, dt: f32) {
        self.step_in(dt, None);
    }

    /// A frame's update as the engine makes it (particles spec, "A long
    /// frame"): a frame longer than `most` (`Time.maximumParticleDeltaTime`,
    /// the install's "Maximum Particle Timestep", 0.03 s) made in so many
    /// equal steps, the fewest none longer than it; the particles that met
    /// the world on any of them, as `step_in` gives them. A system paused
    /// unseen takes no step of the frame.
    pub fn step_frame(&mut self, dt: f32, most: f32, world: Option<&dyn World>) -> Vec<Particle> {
        // The frame's rays for medium and low quality: its share, raised to
        // a multiple of four (`assign_ray_budgets`; none where not given).
        self.cubes.budget = cubes::four_up(std::mem::take(&mut self.cubes.assigned) as i32);
        if dt <= 0.0 {
            return Vec::new();
        }
        if std::mem::take(&mut self.unseen) {
            return Vec::new();
        }
        let parts = if most > 0.0 && dt > most { (dt / most).ceil() as usize } else { 1 };
        let mut met = Vec::new();
        for _ in 0..parts {
            met.extend(self.step_in(dt / parts as f32, world));
        }
        met
    }

    /// The same in a world: after the particles move, those of a system
    /// that meets the world's colliders meet it (`collide`). The particles
    /// that met it this step and were alive before, as they are after: what
    /// a sub-emitter set off by a collision is born from.
    pub fn step_in(&mut self, dt: f32, world: Option<&dyn World>) -> Vec<Particle> {
        if dt <= 0.0 {
            return Vec::new();
        }
        if std::mem::take(&mut self.unseen) {
            return Vec::new();
        }
        let data = self.data.clone();
        let placement = self.placement;
        // Gravity is the world's down; a force in the other space turned.
        let down = if data.world_space { [0.0, GRAVITY, 0.0] } else { placement.way_back([0.0, GRAVITY, 0.0]) };
        let frame_draw = self.rng.unit();
        for p in &mut self.particles {
            p.age += dt;
        }
        kill(&mut self.particles);
        for p in &mut self.particles {
            advance(&data, &placement, down, frame_draw, p, dt);
        }
        let collision = data.collision.as_ref().filter(|c| c.world);
        let mut met = match (world, collision) {
            (Some(world), Some(collision)) => self.collide(0, self.particles.len(), &|_| dt, world, collision),
            _ => Vec::new(),
        };
        if let Some(trails) = &data.trails {
            for p in &mut self.particles {
                drop_trail_point(p, trails);
            }
        }
        // Those born this step, after the others' update, each lived
        // through its time to the step's end as the engine does (the
        // modules, its move, its meeting with the world).
        let first_born = self.particles.len();
        let aheads = self.emit_playing(dt);
        for (p, &ahead) in self.particles[first_born..].iter_mut().zip(&aheads) {
            p.age += ahead;
            advance(&data, &placement, down, frame_draw, p, ahead);
        }
        if let Some(world) = world {
            met.extend(self.meet_born(first_born, &aheads, world));
        }
        if let Some(trails) = &data.trails {
            for p in &mut self.particles[first_born..] {
                drop_trail_point(p, trails);
            }
        }
        met
    }

    /// The born, from `first_born` on, meeting the world four at a time in
    /// turn, each over its time from its birth (`aheads`, in turn)
    /// (`ParticleSystem::StartModules`, decompiled: at medium and low
    /// quality each four draw on the frame's rays and move the cursor); the
    /// particles that met it.
    pub fn meet_born(&mut self, first_born: usize, aheads: &[f32], world: &dyn World) -> Vec<Particle> {
        let data = self.data.clone();
        let Some(collision) = data.collision.as_ref().filter(|c| c.world) else { return Vec::new() };
        let end = self.particles.len();
        let mut met = Vec::new();
        for start in (first_born..end).step_by(4) {
            met.extend(self.collide(start, (start + 4).min(end), &|i| aheads.get(i - first_born).copied().unwrap_or(0.0), world, collision));
        }
        met
    }

    /// Each particle's trail as drawn (particles spec, "Trails"): its points
    /// from the particle's place now back to the oldest kept, in the world,
    /// each with its share along the trail (0 at the particle, by the
    /// distance), its width (`widthOverTrail` there, times the particle's
    /// size where `sizeAffectsWidth`) and colour (`colorOverLifetime` at the
    /// particle's share of life times `colorOverTrail` there, times the
    /// particle's colour where `inheritParticleColor`). `PROVISIONAL`: the
    /// share along by distance; the picture's 0 at the particle.
    pub fn drawn_trails(&self) -> Vec<Vec<TrailPoint>> {
        let Some(trails) = &self.data.trails else { return Vec::new() };
        let place = |at: [f32; 3]| if self.data.world_space { at } else { self.placement.point(at) };
        self.particles
            .iter()
            .zip(self.drawn())
            .filter(|(p, _)| !p.trail.is_empty())
            .map(|(p, drawn)| {
                let mut points: Vec<[f32; 3]> = vec![drawn.position];
                points.extend(p.trail.iter().rev().map(|&(at, _)| place(at)));
                let mut lengths = vec![0.0f32];
                for w in points.windows(2) {
                    let last = *lengths.last().unwrap_or(&0.0);
                    lengths.push(last + length(sub(w[1], w[0])));
                }
                let total = lengths.last().copied().unwrap_or(0.0).max(1e-6);
                let t = p.share();
                let life_colour = trails.color_over_lifetime.evaluate(t, draw(p.seed, TRAIL_COLOR));
                points
                    .iter()
                    .zip(&lengths)
                    .map(|(&position, &l)| {
                        let along = l / total;
                        let mut width = trails.width_over_trail.evaluate(along, draw(p.seed, TRAIL_WIDTH));
                        if trails.size_affects_width {
                            width *= drawn.size;
                        }
                        let trail_colour = trails.color_over_trail.evaluate(along, draw(p.seed, TRAIL_COLOR));
                        let mut color: [f32; 4] = std::array::from_fn(|k| life_colour[k] * trail_colour[k]);
                        if trails.inherit_particle_color {
                            color = std::array::from_fn(|k| color[k] * drawn.color[k]);
                        }
                        TrailPoint { position, along, width, color }
                    })
                    .collect()
            })
            .collect()
    }

    /// Its particles' lights this frame (`LightsModule`): the share
    /// `ratio` of its particles, in turn (`PROVISIONAL`: the first of each
    /// group the ratio makes; at random not built), at most `maxLights`;
    /// each the module's light at the particle's place, its colour tinted
    /// by the particle's where asked, its range times its curve at the
    /// particle's share of life and times its size where asked, its
    /// intensity times its curve and times the particle's alpha where asked.
    pub fn lights(&self) -> Vec<ParticleLight> {
        let Some(module) = self.data.lights.as_ref() else { return Vec::new() };
        let Some(light) = module.light.as_ref() else { return Vec::new() };
        if module.ratio <= 0.0 {
            return Vec::new();
        }
        let every = (1.0 / module.ratio).round().max(1.0) as usize;
        self.particles
            .iter()
            .zip(self.drawn())
            .enumerate()
            .filter(|(i, _)| i % every == 0)
            .take(module.max_lights as usize)
            .map(|(_, (p, drawn))| {
                let t = p.share();
                let mut color = [light.color[0], light.color[1], light.color[2]];
                if module.use_color {
                    color = std::array::from_fn(|k| color[k] * drawn.color[k]);
                }
                let mut range = light.range * module.range_curve.evaluate(t, draw(p.seed, LIGHT_RANGE));
                if module.size_affects_range {
                    range *= drawn.size;
                }
                let mut intensity = light.intensity * module.intensity_curve.evaluate(t, draw(p.seed, LIGHT_INTENSITY));
                if module.alpha_affects_intensity {
                    intensity *= drawn.color[3];
                }
                ParticleLight { position: drawn.position, color, range, intensity }
            })
            .collect()
    }

    /// Each particle as drawn, in the order the system keeps them (the
    /// engine's for a renderer that does not sort: `kill`).
    pub fn drawn(&self) -> impl Iterator<Item = Drawn> + '_ {
        self.particles.iter().map(|p| {
            let t = p.share();
            let tint = self.data.color_over_lifetime.as_ref().map_or([1.0; 4], |g| g.evaluate(t, engine_draw(p.seed, COLOR_ID, 0)));
            let (position, velocity) = if self.data.world_space { (p.position, p.velocity) } else { (self.placement.point(p.position), self.placement.way(p.velocity)) };
            Drawn {
                position,
                velocity,
                size: size_of(&self.data, p, t),
                rotation: p.rotation,
                color: std::array::from_fn(|i| p.color[i] * tint[i]),
                sheet: sheet_of(&self.data, p, t),
                remaining: (p.lifetime - p.age).max(0.0),
            }
        })
    }
}

/// A particle over `dt` by the modules its system has on: gravity (the
/// world's `down` in the system's space), the force, the speed's limit and
/// drag, its velocity over life, its move and its turn (`frame_draw` the
/// step's own draw, for a force drawn anew each frame).
fn advance(data: &ParticleSystemData, placement: &Placement, down: [f32; 3], frame_draw: f32, p: &mut Particle, dt: f32) {
    let t = p.share();
    let gravity = data.gravity_modifier.evaluate(t, draw(p.seed, GRAVITY_DRAW));
    p.velocity = add(p.velocity, scale(down, gravity * dt));
    if let Some(force) = &data.force {
        let pick = |k: u32| if force.randomize_per_frame { draw(p.seed ^ (frame_draw * u32::MAX as f32) as u32, FORCE + k) } else { engine_draw(p.seed, FORCE_ID, k as usize) };
        let f = [force.x.evaluate(t, pick(0)), force.y.evaluate(t, pick(1)), force.z.evaluate(t, pick(2))];
        let f = match (force.world_space, data.world_space) {
            (false, true) => placement.way(f),
            (true, false) => placement.way_back(f),
            _ => f,
        };
        p.velocity = add(p.velocity, scale(f, dt));
    }
    if let Some(limit) = &data.limit_velocity {
        let speed = length(p.velocity);
        let most = limit.magnitude.evaluate(t, engine_draw(p.seed, LIMIT_ID, 0));
        let mut wanted = speed;
        if speed > most {
            // The speed above the limit taken down by `dampen` over each
            // thirtieth of a second, as the engine does
            // (`ClampVelocityModule::Update`, decompiled).
            wanted = most + (speed - most) * (1.0 - limit.dampen.clamp(0.0, 1.0)).powf(30.0 * dt);
        }
        // The engine's drag: a speed taken off, by the particle's
        // cross-section where asked (`PROVISIONAL`: its size as drawn) and
        // by the speed squared where asked.
        let mut drag = limit.drag.evaluate(t, engine_draw(p.seed, DRAG_ID, 0));
        if drag > 0.0 {
            if limit.drag_by_size {
                let half = size_of(data, p, t) * 0.5;
                drag *= std::f32::consts::PI * half * half;
            }
            if limit.drag_by_velocity {
                drag *= wanted * wanted;
            }
            wanted = (wanted - drag * dt).max(0.0);
        }
        if speed > f32::EPSILON {
            p.velocity = p.velocity.map(|v| v * wanted / speed);
        }
    }
    // The velocity over its life (`VelocityModule`): a velocity the step
    // adds to the particle's own and keeps no further (the engine's added
    // velocity, emptied by the initial module each step and filled by this
    // one: decompiled), turned as a force is.
    let mut added = [0.0; 3];
    if let Some(v) = &data.velocity_over_lifetime {
        let a = [v.x.evaluate(t, engine_draw(p.seed, VELOCITY_ID, 0)), v.y.evaluate(t, engine_draw(p.seed, VELOCITY_ID, 1)), v.z.evaluate(t, engine_draw(p.seed, VELOCITY_ID, 2))];
        added = match (v.world_space, data.world_space) {
            (false, true) => placement.way(a),
            (true, false) => placement.way_back(a),
            _ => a,
        };
    }
    for k in 0..3 {
        p.position[k] += (p.velocity[k] + added[k]) * dt;
    }
    let sign = if p.flipped(data) { -1.0 } else { 1.0 };
    if let Some(rotation) = &data.rotation_over_lifetime {
        p.rotation += sign * rotation.evaluate(t, engine_draw(p.seed, ROTATION_ID, 0)).to_degrees() * dt;
    }
    if let Some(by_speed) = &data.rotation_by_speed {
        let span = by_speed.range[1] - by_speed.range[0];
        let at = if span.abs() > 1e-6 { ((length(p.velocity) - by_speed.range[0]) / span).clamp(0.0, 1.0) } else { 0.0 };
        p.rotation += sign * by_speed.curve.evaluate(at, engine_draw(p.seed, BY_SPEED_ID, 0)).to_degrees() * dt;
    }
}

/// One point of a drawn trail: where, its share along the trail (0 at the
/// particle), its width and colour.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrailPoint {
    pub position: [f32; 3],
    pub along: f32,
    pub width: f32,
    pub color: [f32; 4],
}

/// A trail's point dropped where the particle is, its first at once and
/// the next once it has gone `minVertexDistance` from the last; points
/// older than the trail's time (its `lifetime` share of the particle's)
/// let go. Every particle has one (`ratio` 1: the only one built).
fn drop_trail_point(p: &mut Particle, trails: &trp_data::drawing::particles::Trails) {
    let keep_for = trails.lifetime.evaluate(p.share(), draw(p.seed, TRAIL_LIFE)) * p.lifetime;
    let far_enough = p.trail.last().is_none_or(|&(at, _)| length(sub(p.position, at)) >= trails.min_vertex_distance);
    if far_enough {
        p.trail.push((p.position, p.age));
    }
    let age = p.age;
    p.trail.retain(|&(_, dropped)| age - dropped <= keep_for);
}

/// Those at the end of their life taken away as the engine does it
/// (`ParticleSystem::KillParticles`, decompiled: particles spec): in groups
/// of four from the first, each group's dead (as they were when the group
/// was looked at) from its last to its first, each one's place taken by the
/// last particle; a group in which any died looked at again.
fn kill(particles: &mut Vec<Particle>) {
    let mut i = 0;
    while i < particles.len() {
        let count = particles.len();
        let dead: [bool; 4] = std::array::from_fn(|k| i + k < count && particles[i + k].age >= particles[i + k].lifetime);
        let mut died = false;
        for k in (0..4).rev() {
            if dead[k] {
                particles.swap_remove(i + k);
                died = true;
            }
        }
        if !died {
            i += 4;
        }
    }
}
