//! Unity's particle system run (particles spec): a system played (its
//! start delay, its bursts and rate, each particle born on its shape with
//! the main module's start values) or particles emitted with the values the
//! caller gives (`ParticleSystem.Emit(EmitParams)`), each stepped by the
//! modules the system has on, and what each is drawn as. In the game's
//! axes; no engine types.

use std::sync::Arc;

use trp_data::particles::{ParticleSystemData, Shape};

use crate::recoil::Rng;

/// The world's gravity, metres a second a second (the install's physics
/// settings: physics spec).
const GRAVITY: f32 = -9.81;

/// What a particle is born with (`EmitParams`): the caller's values; the
/// system's start values for none.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EmitParams {
    pub position: [f32; 3],
    pub velocity: [f32; 3],
    pub size: f32,
    pub lifetime: f32,
    /// Its colour as bytes (`Color32`), red, green, blue, alpha.
    pub color: [u8; 4],
    /// Its turn in degrees.
    pub rotation: f32,
    /// Its own seed, which its draws for the modules come from.
    pub seed: u32,
}

/// A particle alive, in the system's space (the world's, or its own).
#[derive(Debug, Clone, PartialEq)]
pub struct Particle {
    pub position: [f32; 3],
    pub velocity: [f32; 3],
    /// Its size and colour as born.
    pub size: f32,
    pub color: [f32; 4],
    /// Its turn, degrees.
    pub rotation: f32,
    pub lifetime: f32,
    pub age: f32,
    pub seed: u32,
    /// Its trail's points (`TrailModule`), oldest first, each with its age
    /// when dropped, in the system's space.
    pub trail: Vec<([f32; 3], f32)>,
}

impl Particle {
    /// Its share of life, from 0 to 1.
    pub fn share(&self) -> f32 {
        if self.lifetime > f32::EPSILON { (self.age / self.lifetime).clamp(0.0, 1.0) } else { 1.0 }
    }

    /// Turning the other way (`randomizeRotationDirection`).
    fn flipped(&self, data: &ParticleSystemData) -> bool {
        draw(self.seed, FLIP) < data.flip_rotation
    }
}

/// What a particle is drawn as, in the world: its place, its velocity, its
/// size, its turn in degrees, its colour, its picture's place in a sheet
/// (`[u0, v0, u1, v1]`, the whole picture without one), and the seconds of
/// life it has left (what a renderer sorting by age goes by).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Drawn {
    pub position: [f32; 3],
    pub velocity: [f32; 3],
    pub size: f32,
    pub rotation: f32,
    pub color: [f32; 4],
    pub sheet: [f32; 4],
    pub remaining: f32,
}

/// The world a system's particles meet (the collision module's world): the
/// first surface a sphere meets going from `origin` along the unit way
/// `dir` within `distance`, its distance along the way (below nought where
/// the sphere starts inside a surface, by how far) and its normal. The app
/// asks the game's physics engine (`trp_physics::World::sweep_sphere`).
pub trait World {
    fn sweep_sphere(&self, origin: [f32; 3], dir: [f32; 3], distance: f32, radius: f32) -> Option<(f32, [f32; 3])>;
}

/// A particle's light this frame (the lights module): where it is, its
/// colour (the light's, tinted by the particle's where asked), its range
/// and its intensity.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParticleLight {
    pub position: [f32; 3],
    pub color: [f32; 3],
    pub range: f32,
    pub intensity: f32,
}

/// Where a system is in the world: its object's place, turn and size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placement {
    pub position: [f32; 3],
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
}

impl Default for Placement {
    fn default() -> Self {
        Self { position: [0.0; 3], rotation: [0.0, 0.0, 0.0, 1.0], scale: [1.0; 3] }
    }
}

impl Placement {
    fn point(&self, local: [f32; 3]) -> [f32; 3] {
        let scaled = std::array::from_fn(|k| local[k] * self.scale[k]);
        add(self.position, rotate(self.rotation, scaled))
    }

    fn way(&self, local: [f32; 3]) -> [f32; 3] {
        rotate(self.rotation, local)
    }

    fn way_back(&self, world: [f32; 3]) -> [f32; 3] {
        rotate(conjugate(self.rotation), world)
    }

    fn point_back(&self, world: [f32; 3]) -> [f32; 3] {
        let local = self.way_back(sub(world, self.position));
        std::array::from_fn(|k| if self.scale[k].abs() > 1e-9 { local[k] / self.scale[k] } else { local[k] })
    }
}

/// The modules' own draws for a particle, by its seed. `PROVISIONAL`: the
/// engine's own (a stream of its own for each module) are not documented;
/// these are a hash of the seed and the module's number, from 0 to 1.
fn draw(seed: u32, module: u32) -> f32 {
    let mut n = seed ^ module.wrapping_mul(0x9e37_79b9);
    n ^= n >> 16;
    n = n.wrapping_mul(0x7feb_352d);
    n ^= n >> 15;
    n = n.wrapping_mul(0x846c_a68b);
    n ^= n >> 16;
    (n >> 8) as f32 / (1u32 << 24) as f32
}

const SIZE: u32 = 1;
const ROTATION: u32 = 2;
const COLOR: u32 = 3;
const LIMIT: u32 = 4;
const DRAG: u32 = 5;
const GRAVITY_DRAW: u32 = 6;
const FLIP: u32 = 7;
const FORCE: u32 = 8;
const SHEET: u32 = 11;
const ROW: u32 = 12;
const START_FRAME: u32 = 13;
const BY_SPEED: u32 = 14;
const DAMPEN: u32 = 15;
const BOUNCE: u32 = 16;
const LIFETIME_LOSS: u32 = 17;
const LIGHT_RANGE: u32 = 18;
const LIGHT_INTENSITY: u32 = 19;
const TRAIL_COLOR: u32 = 20;
const TRAIL_WIDTH: u32 = 21;
const TRAIL_LIFE: u32 = 22;
const VELOCITY: u32 = 23;

/// `Mathf.SmoothDamp` as Unity publishes it (`UnityCsReference`): `current`
/// toward `target` over about `smooth_time`, its velocity carried, with no
/// most speed; the target taken where it would pass it.
pub fn smooth_damp(current: f32, target: f32, velocity: &mut f32, smooth_time: f32, dt: f32) -> f32 {
    let smooth_time = smooth_time.max(0.0001);
    let omega = 2.0 / smooth_time;
    let x = omega * dt;
    let exp = 1.0 / (1.0 + x + 0.48 * x * x + 0.235 * x * x * x);
    let change = current - target;
    let temp = (*velocity + omega * change) * dt;
    *velocity = (*velocity - omega * temp) * exp;
    let mut output = target + (change + temp) * exp;
    if (target - current > 0.0) == (output > target) {
        output = target;
        *velocity = if dt > 0.0 { (output - target) / dt } else { 0.0 };
    }
    output
}

/// A system's playing (`Play`): the time since it was played, its start
/// delay, each burst's cycles done, the rate's share of a particle carried,
/// and whether it has done emitting.
#[derive(Debug, Clone)]
struct Playing {
    time: f32,
    delay: f32,
    cycles_done: Vec<i64>,
    carried: f32,
    done: bool,
}

/// A system running: its figures, where it is, its particles (oldest
/// first) and its own draws.
#[derive(Debug, Clone)]
pub struct ParticleSystem {
    pub data: Arc<ParticleSystemData>,
    pub particles: Vec<Particle>,
    pub placement: Placement,
    playing: Option<Playing>,
    rng: Rng,
}

impl ParticleSystem {
    pub fn new(data: Arc<ParticleSystemData>) -> Self {
        Self { data, particles: Vec::new(), placement: Placement::default(), playing: None, rng: Rng::new(0x5eed) }
    }

    /// One whose own draws (its start values, shape, bursts) come from
    /// `seed`.
    pub fn seeded(data: Arc<ParticleSystemData>, seed: u64) -> Self {
        Self { rng: Rng::new(seed), ..Self::new(data) }
    }

    /// Played where it is (`Play`, the start of its cycle): its delay
    /// drawn, its bursts to come.
    pub fn play(&mut self, placement: Placement) {
        self.placement = placement;
        let delay = self.data.start_delay.evaluate(0.0, self.rng.unit()).max(0.0);
        let bursts = self.data.emission.as_ref().map_or(0, |e| e.bursts.len());
        self.playing = Some(Playing { time: 0.0, delay, cycles_done: vec![0; bursts], carried: 0.0, done: false });
    }

    /// Whether it may still emit or show anything.
    pub fn alive(&self) -> bool {
        !self.particles.is_empty() || self.playing.as_ref().is_some_and(|p| !p.done)
    }

    /// A particle born with these values; none where the system already
    /// has its most alive.
    pub fn emit(&mut self, p: EmitParams) -> bool {
        if self.particles.len() >= self.data.max_particles as usize || p.lifetime <= 0.0 {
            return false;
        }
        self.particles.push(Particle {
            position: p.position,
            velocity: p.velocity,
            size: p.size,
            color: p.color.map(|c| c as f32 / 255.0),
            rotation: p.rotation,
            lifetime: p.lifetime,
            age: 0.0,
            seed: p.seed,
            // A trail begins where its particle is born.
            trail: if self.data.trails.is_some() { vec![(p.position, 0.0)] } else { Vec::new() },
        });
        true
    }

    /// So many born on its shape with its start values, at the share
    /// `cycle` of its cycle.
    fn emit_shaped(&mut self, count: u32, cycle: f32) {
        let data = self.data.clone();
        for _ in 0..count {
            if self.particles.len() >= data.max_particles as usize {
                return;
            }
            let rng = &mut self.rng;
            let (at, way) = data.shape.as_ref().map_or(([0.0; 3], [0.0, 0.0, 1.0]), |shape| on_shape(shape, rng));
            let speed = data.start_speed.evaluate(cycle, rng.unit());
            let lifetime = data.start_lifetime.evaluate(cycle, rng.unit());
            let size = data.start_size.evaluate(cycle, rng.unit());
            let rotation = data.start_rotation.evaluate(cycle, rng.unit()).to_degrees();
            let color = data.start_color.evaluate(cycle, rng.unit());
            let seed = (rng.unit() * u32::MAX as f32) as u32;
            let velocity = scale(way, speed);
            // The scale moves where it is born on the shape, not its speed
            // (`scalingMode` Shape).
            let (position, velocity) = if data.world_space { (self.placement.point(at), self.placement.way(velocity)) } else { (std::array::from_fn(|k| at[k] * self.placement.scale[k]), velocity) };
            let mut p = Particle { position, velocity, size, color, rotation, lifetime, age: 0.0, seed, trail: if self.data.trails.is_some() { vec![(position, 0.0)] } else { Vec::new() } };
            if p.flipped(&data) {
                p.rotation = -p.rotation;
            }
            if lifetime > 0.0 {
                self.particles.push(p);
            }
        }
    }

    /// The system's own emission over the time passed: its bursts whose
    /// times fall in it, its rate.
    fn emit_playing(&mut self, dt: f32) {
        let Some(mut playing) = self.playing.take() else { return };
        let before = playing.time - playing.delay;
        playing.time += dt;
        let now = playing.time - playing.delay;
        let duration = self.data.duration.max(1e-3);
        if !playing.done && now >= 0.0 {
            let data = self.data.clone();
            if let Some(emission) = &data.emission {
                for (b, burst) in emission.bursts.iter().enumerate() {
                    let cycles = if burst.cycles <= 0 { i64::MAX } else { burst.cycles };
                    while playing.cycles_done[b] < cycles {
                        let at = burst.time + playing.cycles_done[b] as f32 * burst.interval.max(1e-4);
                        if at > now || (at > duration && !data.looping) {
                            break;
                        }
                        playing.cycles_done[b] += 1;
                        if self.rng.unit() <= burst.probability {
                            let count = burst.count.evaluate(at / duration, self.rng.unit()).round().max(0.0) as u32;
                            self.emit_shaped(count, (at / duration).min(1.0));
                        }
                    }
                }
                let rate = emission.rate_over_time.evaluate((now / duration).min(1.0), self.rng.unit());
                let span = now - before.max(0.0);
                playing.carried += rate * span.max(0.0);
                let whole = playing.carried.floor();
                if whole >= 1.0 {
                    playing.carried -= whole;
                    self.emit_shaped(whole as u32, (now / duration).min(1.0));
                }
            }
            if now >= duration {
                if data.looping {
                    playing.time = playing.delay + (now - duration);
                    playing.cycles_done.iter_mut().for_each(|c| *c = 0);
                } else {
                    playing.done = true;
                }
            }
        }
        self.playing = Some(playing);
    }

    /// The time passed: the system's own emission; each particle aged, those
    /// at the end of their life gone as the engine takes them away, the rest
    /// forced, limited and dragged, moved and turned.
    pub fn step(&mut self, dt: f32) {
        self.step_in(dt, None);
    }

    /// The same in a world: after the particles move, those of a system
    /// that meets the world's colliders meet it (`collide`). The particles
    /// that met it this step and were alive before, as they are after: what
    /// a sub-emitter set off by a collision is born from.
    pub fn step_in(&mut self, dt: f32, world: Option<&dyn World>) -> Vec<Particle> {
        if dt <= 0.0 {
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
            let t = p.share();
            let gravity = data.gravity_modifier.evaluate(t, draw(p.seed, GRAVITY_DRAW));
            p.velocity = add(p.velocity, scale(down, gravity * dt));
            if let Some(force) = &data.force {
                let pick = |k: u32| if force.randomize_per_frame { draw(p.seed ^ (frame_draw * u32::MAX as f32) as u32, FORCE + k) } else { draw(p.seed, FORCE + k) };
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
                let most = limit.magnitude.evaluate(t, draw(p.seed, LIMIT));
                let mut wanted = speed;
                if speed > most {
                    // The speed above the limit taken down by `dampen`
                    // over each thirtieth of a second, as the engine does
                    // (`ClampVelocityModule::Update`, decompiled).
                    wanted = most + (speed - most) * (1.0 - limit.dampen.clamp(0.0, 1.0)).powf(30.0 * dt);
                }
                // The engine's drag: a speed taken off, by the particle's
                // cross-section where asked (`PROVISIONAL`: its size as
                // drawn) and by the speed squared where asked.
                let mut drag = limit.drag.evaluate(t, draw(p.seed, DRAG));
                if drag > 0.0 {
                    if limit.drag_by_size {
                        let half = size_of(&data, p, t) * 0.5;
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
            // The velocity over its life (`VelocityModule`): a velocity the
            // step adds to the particle's own and keeps no further (the
            // engine's added velocity, emptied by the initial module each
            // step and filled by this one: decompiled), turned as a force is.
            let mut added = [0.0; 3];
            if let Some(v) = &data.velocity_over_lifetime {
                let a = [v.x.evaluate(t, draw(p.seed, VELOCITY)), v.y.evaluate(t, draw(p.seed, VELOCITY + 1)), v.z.evaluate(t, draw(p.seed, VELOCITY + 2))];
                added = match (v.world_space, data.world_space) {
                    (false, true) => placement.way(a),
                    (true, false) => placement.way_back(a),
                    _ => a,
                };
            }
            for k in 0..3 {
                p.position[k] += (p.velocity[k] + added[k]) * dt;
            }
            let sign = if p.flipped(&data) { -1.0 } else { 1.0 };
            if let Some(rotation) = &data.rotation_over_lifetime {
                p.rotation += sign * rotation.evaluate(t, draw(p.seed, ROTATION)).to_degrees() * dt;
            }
            if let Some(by_speed) = &data.rotation_by_speed {
                let span = by_speed.range[1] - by_speed.range[0];
                let at = if span.abs() > 1e-6 { ((length(p.velocity) - by_speed.range[0]) / span).clamp(0.0, 1.0) } else { 0.0 };
                p.rotation += sign * by_speed.curve.evaluate(at, draw(p.seed, BY_SPEED)).to_degrees() * dt;
            }
        }
        let met = match (world, data.collision.as_ref().filter(|c| c.world)) {
            (Some(world), Some(collision)) => self.collide(dt, world, collision),
            _ => Vec::new(),
        };
        if let Some(trails) = &data.trails {
            for p in &mut self.particles {
                drop_trail_point(p, trails);
            }
        }
        self.emit_playing(dt);
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
    fn collide(&mut self, dt: f32, world: &dyn World, collision: &trp_data::particles::Collision) -> Vec<Particle> {
        let placement = self.placement;
        let world_space = self.data.world_space;
        let mut met = Vec::new();
        for p in &mut self.particles {
            let (position, velocity) = if world_space { (p.position, p.velocity) } else { (placement.point(p.position), placement.way(p.velocity)) };
            let from = sub(position, scale(velocity, dt));
            let travel = sub(position, from);
            let distance = length(travel);
            if distance <= 1e-7 {
                continue;
            }
            let dir = scale(travel, 1.0 / distance);
            let r = 0.5 * p.size * collision.radius_scale;
            let pull = r * 0.151;
            let Some((t, n)) = world.sweep_sphere(from, dir, distance + pull, (r - pull).max(1e-6)) else { continue };
            if t == 0.0 {
                continue;
            }
            let hit = if t > 0.0 { add(from, scale(dir, t - pull)) } else { add(from, scale(n, pull - t)) };
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

    /// Set off by another system's particle (a sub-emitter): its bursts'
    /// counts born at `position` on its shape, turned as the system is, at
    /// `cycle` of a cycle (the parent system's where it takes the parent's
    /// duration). `PROVISIONAL`: each burst's count at once, by its chance;
    /// its rate not taken.
    pub fn emit_at(&mut self, position: [f32; 3], cycle: f32) {
        let Some(emission) = self.data.emission.clone() else { return };
        let placement = self.placement;
        self.placement.position = if self.data.world_space { position } else { placement.point_back(position) };
        for burst in &emission.bursts {
            if self.rng.unit() <= burst.probability {
                let count = burst.count.evaluate(cycle, self.rng.unit()).round().max(0.0) as u32;
                self.emit_shaped(count, cycle);
            }
        }
        self.placement = placement;
    }

    /// A particle's place in the world (a local system's put through its
    /// placement).
    pub fn world_place(&self, p: &Particle) -> [f32; 3] {
        if self.data.world_space { p.position } else { self.placement.point(p.position) }
    }

    /// Its cycle's share played, from 0 to 1 (what a sub-emitter taking its
    /// parent's duration samples its start values at).
    pub fn cycle(&self) -> f32 {
        let duration = self.data.duration.max(1e-3);
        self.playing.as_ref().map_or(0.0, |p| ((p.time - p.delay).max(0.0) / duration).min(1.0))
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
            let tint = self.data.color_over_lifetime.as_ref().map_or([1.0; 4], |g| g.evaluate(t, draw(p.seed, COLOR)));
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
fn drop_trail_point(p: &mut Particle, trails: &trp_data::particles::Trails) {
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

/// Where on its shape a particle is born and which way it goes, in the
/// system's own axes (`ShapeModule`, as the documentation describes each
/// kind). `PROVISIONAL`: a cone's way leans out from its axis by its angle
/// in proportion to how far out on the base it is born.
fn on_shape(shape: &Shape, rng: &mut Rng) -> ([f32; 3], [f32; 3]) {
    let arc = shape.arc.to_radians();
    let around = rng.unit() * arc;
    let thickness = shape.radius_thickness.clamp(0.0, 1.0);
    let (at, way) = match shape.kind {
        // A cone's base: a point on the disc (its area evenly), its way
        // leaning out with it.
        4 => {
            let inner = 1.0 - thickness;
            let reach = (inner * inner + (1.0 - inner * inner) * rng.unit()).sqrt();
            let lean = shape.angle.to_radians() * reach;
            let at = [shape.radius * reach * around.cos(), shape.radius * reach * around.sin(), 0.0];
            (at, [lean.sin() * around.cos(), lean.sin() * around.sin(), lean.cos()])
        }
        // A sphere or hemisphere: a point in its ball (its volume evenly,
        // within its thickness), its way out from the middle.
        0 | 2 => {
            let way = loop {
                let v = [rng.range(-1.0, 1.0), rng.range(-1.0, 1.0), rng.range(-1.0, 1.0)];
                let l = length(v);
                if l > 1e-3 && l <= 1.0 {
                    break scale(v, 1.0 / l);
                }
            };
            let way = if shape.kind == 2 && way[2] < 0.0 { [way[0], way[1], -way[2]] } else { way };
            let inner = 1.0 - thickness;
            let reach = (inner.powi(3) + (1.0 - inner.powi(3)) * rng.unit()).cbrt();
            (scale(way, shape.radius * reach), way)
        }
        // Anything else (not built): from the middle, along its axis.
        _ => ([0.0; 3], [0.0, 0.0, 1.0]),
    };
    let mut way = way;
    if shape.random_direction > 0.0 {
        let r = [rng.range(-1.0, 1.0), rng.range(-1.0, 1.0), rng.range(-1.0, 1.0)];
        way = normalized(add(scale(way, 1.0 - shape.random_direction), scale(normalized(r), shape.random_direction)));
    }
    if shape.spherical_direction > 0.0 && length(at) > 1e-6 {
        way = normalized(add(scale(way, 1.0 - shape.spherical_direction), scale(normalized(at), shape.spherical_direction)));
    }
    // The shape's own place, turn (Unity's order: z, then x, then y) and
    // size.
    let turn = euler(shape.rotation);
    let scaled = std::array::from_fn(|k| at[k] * shape.scale[k]);
    let mut at = add(shape.position, rotate(turn, scaled));
    if shape.random_position > 0.0 {
        at = add(at, [rng.range(-1.0, 1.0) * shape.random_position, rng.range(-1.0, 1.0) * shape.random_position, rng.range(-1.0, 1.0) * shape.random_position]);
    }
    (at, rotate(turn, way))
}

/// A particle's picture in its system's sheet (`UVModule`, a grid): the
/// frame its curve gives at its share of life (a share of the sheet, or of
/// a row), the sheet's cell; frame 0 at the top left.
fn sheet_of(data: &ParticleSystemData, p: &Particle, t: f32) -> [f32; 4] {
    let Some(sheet) = data.texture_sheet.as_ref().filter(|s| s.mode == 0) else { return [0.0, 0.0, 1.0, 1.0] };
    let [across, down] = sheet.tiles.map(|n| n.max(1));
    let frames = if sheet.single_row { across } else { across * down };
    let mut share = sheet.frame_over_time.evaluate(t, draw(p.seed, SHEET));
    if sheet.cycles > 1.0 {
        share = (share * sheet.cycles).fract();
    }
    let start = sheet.start_frame.evaluate(t, draw(p.seed, START_FRAME));
    let frame = ((share * frames as f32 + start).floor().max(0.0) as u32) % frames;
    let (column, row) = if sheet.single_row {
        let row = if sheet.random_row { ((draw(p.seed, ROW) * down as f32) as u32).min(down - 1) } else { sheet.row_index.min(down - 1) };
        (frame, row)
    } else {
        (frame % across, frame / across)
    };
    let (w, h) = (1.0 / across as f32, 1.0 / down as f32);
    // The picture's rows bottom first, the sheet's top first.
    let v0 = 1.0 - (row + 1) as f32 * h;
    [column as f32 * w, v0, (column + 1) as f32 * w, v0 + h]
}

/// A particle's size at its share of life.
fn size_of(data: &ParticleSystemData, p: &Particle, t: f32) -> f32 {
    p.size * data.size_over_lifetime.as_ref().map_or(1.0, |c| c.evaluate(t, draw(p.seed, SIZE)))
}

/// Unity's `Quaternion.Euler`: about z, then x, then y, degrees.
fn euler(degrees: [f32; 3]) -> [f32; 4] {
    let half = degrees.map(|d| d.to_radians() * 0.5);
    let about = |axis: usize| {
        let mut q = [0.0, 0.0, 0.0, half[axis].cos()];
        q[axis] = half[axis].sin();
        q
    };
    quat_mul(about(1), quat_mul(about(0), about(2)))
}

fn quat_mul(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    [
        a[3] * b[0] + a[0] * b[3] + a[1] * b[2] - a[2] * b[1],
        a[3] * b[1] - a[0] * b[2] + a[1] * b[3] + a[2] * b[0],
        a[3] * b[2] + a[0] * b[1] - a[1] * b[0] + a[2] * b[3],
        a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2],
    ]
}

fn conjugate(q: [f32; 4]) -> [f32; 4] {
    [-q[0], -q[1], -q[2], q[3]]
}

fn rotate(q: [f32; 4], v: [f32; 3]) -> [f32; 3] {
    let u = [q[0], q[1], q[2]];
    let cross = |a: [f32; 3], b: [f32; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    let t = cross(u, v).map(|c| 2.0 * c);
    let c = cross(u, t);
    [v[0] + q[3] * t[0] + c[0], v[1] + q[3] * t[1] + c[1], v[2] + q[3] * t[2] + c[2]]
}

fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn scale(a: [f32; 3], s: f32) -> [f32; 3] {
    [a[0] * s, a[1] * s, a[2] * s]
}

fn normalized(a: [f32; 3]) -> [f32; 3] {
    let l = length(a);
    if l > 1e-9 { scale(a, 1.0 / l) } else { [0.0, 0.0, 1.0] }
}

fn length(v: [f32; 3]) -> f32 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use trp_data::curve::{Curve, Key};
    use trp_data::particles::{Gradient, LimitVelocity, MinMaxCurve, MinMaxGradient, ParticleMaterial, ParticleRenderer};

    fn curve(points: &[(f32, f32)]) -> Curve {
        Curve { keys: points.iter().map(|&(time, value)| Key { time, value, in_slope: 0.0, out_slope: 0.0 }).collect() }
    }

    /// The fume system as the install has it (firing spec, "The muzzle's
    /// fumes").
    fn fume() -> ParticleSystemData {
        let constant = MinMaxCurve::constant;
        let white = MinMaxGradient { mode: 0, min_color: [1.0; 4], max_color: [1.0; 4], min_gradient: Gradient::default(), max_gradient: Gradient::default() };
        ParticleSystemData {
            name: "Effect Muzzle Smoke".into(),
            world_space: true,
            max_particles: 100,
            duration: 5.0,
            looping: false,
            start_delay: constant(0.1),
            flip_rotation: 0.0,
            shape: None,
            emission: None,
            texture_sheet: None,
            force: None,
            velocity_over_lifetime: None,
            rotation_by_speed: None,
            start_lifetime: constant(0.35),
            start_speed: constant(7.0),
            start_color: white.clone(),
            start_size: constant(0.5),
            start_rotation: constant(0.0),
            gravity_modifier: constant(0.0),
            size_over_lifetime: Some(MinMaxCurve { mode: 1, scalar: 1.0, min_scalar: 0.0, max_curve: curve(&[(0.0, 0.6127), (1.0, 1.0)]), min_curve: Curve::default() }),
            rotation_over_lifetime: Some(MinMaxCurve { mode: 3, scalar: 0.0873, min_scalar: -0.0873, max_curve: Curve::default(), min_curve: Curve::default() }),
            color_over_lifetime: Some(MinMaxGradient {
                mode: 1,
                max_gradient: Gradient { colors: vec![(0.0, [1.0, 0.247, 1.0]), (1.0, [1.0; 3])], alphas: vec![(0.0, 0.0), (0.1176, 1.0), (0.4412, 0.3373), (1.0, 0.0)], stepped: false },
                ..white
            }),
            limit_velocity: Some(LimitVelocity { magnitude: constant(0.0), dampen: 0.2, drag: constant(0.0), drag_by_size: true, drag_by_velocity: true }),
            sub_emitters: Vec::new(),
            collision: None,
            lights: None,
            trails: None,
            unbuilt: Vec::new(),
            renderer: ParticleRenderer {
                render_mode: 0,
                velocity_scale: 0.0,
                length_scale: 2.0,
                camera_velocity_scale: 0.0,
                sorting_fudge: 0.0,
                alignment: 0,
                sort_mode: 0,
                min_size: 0.0,
                max_size: 10.0,
                material: ParticleMaterial { name: String::new(), shader: String::new(), textures: Vec::new(), floats: Vec::new(), colors: Vec::new(), scales: Vec::new(), keywords: Vec::new(), render_queue: 3000 },
                trail_material: None,
                enabled: true,
                mesh: None,
                normal_direction: 1.0,
            },
        }
    }

    /// The ground at nought, as the world.
    struct Ground;

    impl World for Ground {
        fn sweep_sphere(&self, origin: [f32; 3], dir: [f32; 3], distance: f32, radius: f32) -> Option<(f32, [f32; 3])> {
            if dir[1] >= 0.0 {
                return None;
            }
            let t = (origin[1] - radius) / -dir[1];
            (t >= 0.0 && t <= distance).then_some((t, [0.0, 1.0, 0.0]))
        }
    }

    /// The velocity over life moves a particle by its own velocity plus the
    /// module's, and leaves its own velocity as it was (the engine's added
    /// velocity, emptied each step).
    #[test]
    fn the_velocity_over_life_is_added_and_not_kept() {
        let along = trp_data::particles::VelocityOverLifetime { x: MinMaxCurve::constant(0.0), y: MinMaxCurve::constant(0.0), z: MinMaxCurve::constant(1.0), world_space: true };
        let data = Arc::new(ParticleSystemData { velocity_over_lifetime: Some(along), limit_velocity: None, gravity_modifier: MinMaxCurve::constant(0.0), world_space: true, ..fume() });
        let mut system = ParticleSystem::new(data);
        system.emit(EmitParams { position: [0.0; 3], velocity: [2.0, 0.0, 0.0], size: 0.2, lifetime: 5.0, color: [255; 4], rotation: 0.0, seed: 1 });
        for _ in 0..10 {
            system.step(0.1);
        }
        let p = &system.particles[0];
        assert!((p.position[2] - 1.0).abs() < 1e-4 && (p.position[0] - 2.0).abs() < 1e-4, "{:?}", p.position);
        assert_eq!(p.velocity, [2.0, 0.0, 0.0]);
    }

    /// A falling particle meets the ground: its normal speed reflected and
    /// kept by the bounce, all of it by one less the dampen; put back above
    /// the ground by its move past it; a share of its life gone; one too
    /// slow killed; each reported for the sub-emitters.
    #[test]
    fn a_particle_meets_the_ground() {
        let collision = trp_data::particles::Collision {
            world: true,
            dampen: MinMaxCurve::constant(0.2),
            bounce: MinMaxCurve::constant(0.5),
            lifetime_loss: MinMaxCurve::constant(0.05),
            min_kill_speed: 0.17,
            max_kill_speed: 10_000.0,
            radius_scale: 0.01,
            quality: 0,
            collides_with: 4097,
        };
        let data = Arc::new(ParticleSystemData { collision: Some(collision), limit_velocity: None, size_over_lifetime: None, ..fume() });
        let mut system = ParticleSystem::new(data);
        system.emit(EmitParams { position: [0.0, 0.05, 0.0], velocity: [3.0, -5.0, 0.0], size: 0.2, lifetime: 2.0, color: [255; 4], rotation: 0.0, seed: 1 });
        system.emit(EmitParams { position: [5.0, 0.001, 0.0], velocity: [0.1, -0.1, 0.0], size: 0.2, lifetime: 2.0, color: [255; 4], rotation: 0.0, seed: 2 });
        let met = system.step_in(0.02, Some(&Ground));
        assert_eq!(met.len(), 2);
        let p = &system.particles[0];
        // Into the ground at 5 m/s: back out at 5 × 0.5 × 0.8, along it at
        // 3 × 0.8.
        assert!((p.velocity[1] - 2.0).abs() < 1e-4 && (p.velocity[0] - 2.4).abs() < 1e-4, "{:?}", p.velocity);
        assert!(p.position[1] > 0.0, "{:?}", p.position);
        assert!((p.age - (0.02 + 0.05 * 2.0)).abs() < 1e-5, "{}", p.age);
        // Coming in below the least speed: dead.
        assert!(system.particles[1].age > system.particles[1].lifetime);
        // Above the ground and going up: nothing more.
        assert!(system.step_in(0.02, Some(&Ground)).is_empty());
    }

    /// A particle with a trail drops a point each 0.2 m it goes; its trail
    /// as drawn runs from where it is back to its birth, twice its size wide
    /// (`widthOverTrail` 2, `sizeAffectsWidth`), white (the trail's colours,
    /// not the particle's).
    #[test]
    fn a_trail_follows_its_particle() {
        let white = MinMaxGradient::constant([1.0; 4]);
        let trails = trp_data::particles::Trails {
            mode: 0,
            ratio: 1.0,
            lifetime: MinMaxCurve::constant(1.0),
            min_vertex_distance: 0.2,
            texture_mode: 0,
            world_space: false,
            die_with_particles: true,
            size_affects_width: true,
            size_affects_lifetime: false,
            inherit_particle_color: false,
            color_over_lifetime: white.clone(),
            width_over_trail: MinMaxCurve::constant(2.0),
            color_over_trail: white,
        };
        let data = Arc::new(ParticleSystemData { trails: Some(trails), limit_velocity: None, size_over_lifetime: None, color_over_lifetime: None, ..fume() });
        let mut system = ParticleSystem::new(data);
        system.emit(EmitParams { position: [0.0; 3], velocity: [1.0, 0.0, 0.0], size: 0.05, lifetime: 2.0, color: [255, 0, 0, 255], rotation: 0.0, seed: 1 });
        for _ in 0..100 {
            system.step(0.01);
        }
        let trails = system.drawn_trails();
        assert_eq!(trails.len(), 1);
        let t = &trails[0];
        // Born at nought, a point each 0.2 m: 0, 0.2, 0.4, 0.6, 0.8 (within a
        // step's 1 cm), the particle at 1 m.
        assert_eq!(t.len(), 6, "{t:?}");
        assert!((t[0].position[0] - 1.0).abs() < 1e-4 && t[0].along == 0.0 && t.last().unwrap().position[0] == 0.0 && t.last().unwrap().along == 1.0, "{t:?}");
        assert!(t.iter().all(|p| (p.width - 0.1).abs() < 1e-6 && p.color == [1.0; 4]), "{t:?}");
    }

    /// `SmoothDamp` as the driven light uses it (its time the frame's times
    /// 5): from 0 toward 4 it eases in, the same in frames at any frame
    /// rate, and holds at 4.
    #[test]
    fn smooth_damp_eases_the_same_in_frames() {
        let run = |dt: f32| -> Vec<f32> {
            let (mut value, mut velocity) = (0.0f32, 0.0f32);
            (0..12).map(|_| {
                value = smooth_damp(value, 4.0, &mut velocity, dt * 5.0, dt);
                value
            }).collect()
        };
        let (at60, at144) = (run(1.0 / 60.0), run(1.0 / 144.0));
        println!("{at60:?}");
        assert!(at60.iter().zip(&at144).all(|(a, b)| (a - b).abs() < 1e-3), "{at60:?} {at144:?}");
        assert!((at60[0] - 0.246).abs() < 0.002, "{at60:?}");
        assert!(at60.windows(2).all(|w| w[1] >= w[0]) && *at60.last().unwrap() <= 4.0 && *at60.last().unwrap() > 3.5, "{at60:?}");
    }

    /// A system set off by another's particle is born there, on its own
    /// shape, by its bursts; one with a light module lights its particles.
    #[test]
    fn a_sub_emitter_is_born_where_it_is_set_off_and_particles_light() {
        let emission = trp_data::particles::Emission {
            rate_over_time: MinMaxCurve::constant(0.0),
            rate_over_distance: MinMaxCurve::constant(0.0),
            bursts: vec![trp_data::particles::Burst { time: 0.0, count: MinMaxCurve::constant(3.0), cycles: 1, interval: 0.01, probability: 1.0 }],
        };
        let light = trp_data::particles::EffectLight { name: "Point Light".into(), position: [0.0; 3], kind: 2, color: [1.0, 0.8, 0.5, 1.0], intensity: 7.5, range: 7.0, enabled: false, driver: None };
        let lights = trp_data::particles::ParticleLights {
            ratio: 1.0,
            light: Some(light),
            random_distribution: false,
            use_color: false,
            size_affects_range: true,
            alpha_affects_intensity: true,
            range_curve: MinMaxCurve::constant(1.0),
            intensity_curve: MinMaxCurve::constant(1.0),
            max_lights: 2,
        };
        let data = Arc::new(ParticleSystemData { emission: Some(emission), lights: Some(lights), color_over_lifetime: None, size_over_lifetime: None, ..fume() });
        let mut system = ParticleSystem::new(data);
        system.emit_at([1.0, 2.0, 3.0], 0.5);
        assert_eq!(system.particles.len(), 3);
        assert!(system.particles.iter().all(|p| p.position == [1.0, 2.0, 3.0]));
        let lit = system.lights();
        assert_eq!(lit.len(), 2, "at most two");
        assert!((lit[0].range - 7.0 * 0.5).abs() < 1e-5 && (lit[0].intensity - 7.5).abs() < 1e-5, "{lit:?}");
    }

    fn puff(at: [f32; 3]) -> EmitParams {
        EmitParams { position: at, velocity: [0.0, 0.0, 5.0], size: 0.4, lifetime: 0.2, color: [255, 255, 255, 220], rotation: 30.0, seed: 7 }
    }

    #[test]
    fn values_that_vary_and_gradients_go_by_their_mode() {
        let two = MinMaxCurve { mode: 3, scalar: 2.0, min_scalar: -2.0, max_curve: Curve::default(), min_curve: Curve::default() };
        assert_eq!((two.evaluate(0.3, 0.0), two.evaluate(0.3, 0.5), two.evaluate(0.3, 1.0)), (-2.0, 0.0, 2.0));
        let along = MinMaxCurve { mode: 1, scalar: 2.0, min_scalar: 0.0, max_curve: curve(&[(0.0, 0.0), (1.0, 1.0)]), min_curve: Curve::default() };
        assert!((along.evaluate(1.0, 0.7) - 2.0).abs() < 1e-5);
        let g = fume().color_over_lifetime.unwrap().max_gradient;
        // Before the first key the first's, after the last the last's;
        // between two, blended; colour and alpha apart.
        assert_eq!(g.evaluate(-1.0), [1.0, 0.247, 1.0, 0.0]);
        assert_eq!(g.evaluate(2.0), [1.0, 1.0, 1.0, 0.0]);
        let mid = g.evaluate(0.5);
        assert!((mid[1] - (0.247 + 1.0) / 2.0).abs() < 1e-5);
        let a = 0.3373 + (0.0 - 0.3373) * (0.5 - 0.4412) / (1.0 - 0.4412);
        assert!((mid[3] - a).abs() < 1e-4, "{mid:?}");
        let stepped = Gradient { stepped: true, ..g };
        assert_eq!(stepped.evaluate(0.2)[3], 0.3373);
    }

    #[test]
    fn a_puff_slows_grows_fades_and_goes() {
        let mut system = ParticleSystem::new(Arc::new(fume()));
        assert!(system.emit(puff([0.0; 3])));
        let born = system.drawn().next().unwrap();
        assert!((born.size - 0.4 * 0.6127).abs() < 1e-4);
        assert_eq!(born.color[3], 0.0, "faded in from nothing");
        // Its speed above the limit (0) falls by 0.8 each thirtieth of a
        // second, whatever the steps.
        let mut halves = system.clone();
        for _ in 0..30 {
            system.step(1.0 / 300.0);
        }
        halves.step(0.05);
        halves.step(0.05);
        let speed = |s: &ParticleSystem| s.particles[0].velocity[2];
        assert!((speed(&system) - 5.0 * 0.8f32.powi(3)).abs() < 1e-3, "{}", speed(&system));
        assert!((speed(&system) - speed(&halves)).abs() < 1e-3);
        assert!(system.particles[0].position[2] > 0.0);
        // It turns at a speed of its own within ±5 degrees a second.
        let turned = system.particles[0].rotation - 30.0;
        assert!(turned.abs() <= 5.0 * 0.1 + 1e-4 && turned != 0.0, "{turned}");
        let later = system.drawn().next().unwrap();
        assert!(later.size > born.size && later.color[3] > 0.0);
        system.step(0.2);
        assert!(system.particles.is_empty(), "gone at the end of its life");
    }

    #[test]
    fn no_more_than_the_most_are_born() {
        let mut system = ParticleSystem::new(Arc::new(fume()));
        let born = (0..150).filter(|_| system.emit(puff([0.0; 3]))).count();
        assert_eq!(born, 100);
        assert!(!system.emit(EmitParams { lifetime: 0.0, ..puff([0.0; 3]) }));
    }

    #[test]
    fn a_dead_particle_s_place_is_taken_by_the_last() {
        // Six particles, the second and the fourth at their end: in the
        // engine's order the group of the first four is looked at from its
        // last, the fourth's place taken by the sixth and the second's by
        // the fifth.
        let mut system = ParticleSystem::new(Arc::new(fume()));
        for (k, life) in [1.0, 0.05, 1.0, 0.05, 1.0, 1.0].into_iter().enumerate() {
            system.emit(EmitParams { position: [k as f32, 0.0, 0.0], lifetime: life, ..puff([0.0; 3]) });
        }
        system.step(0.1);
        let order: Vec<f32> = system.particles.iter().map(|p| p.position[0].round()).collect();
        assert_eq!(order, [0.0, 4.0, 2.0, 5.0]);
    }

    #[test]
    fn drag_takes_a_speed_off() {
        // A drag of 2 with no limit takes 2 m/s off a second, whatever the
        // speed: from 5 m/s, 4.8 after a tenth of a second.
        let mut data = fume();
        data.size_over_lifetime = None;
        data.limit_velocity = Some(LimitVelocity { magnitude: MinMaxCurve::constant(100.0), dampen: 0.0, drag: MinMaxCurve::constant(2.0), drag_by_size: false, drag_by_velocity: false });
        let mut system = ParticleSystem::new(Arc::new(data));
        system.emit(EmitParams { lifetime: 5.0, ..puff([0.0; 3]) });
        system.step(0.1);
        assert!((length(system.particles[0].velocity) - 4.8).abs() < 1e-4);
    }

    /// A system of the explosion's kind: a cone, two bursts, a rate.
    fn played() -> ParticleSystemData {
        use trp_data::particles::{Burst, Emission, Shape};
        let burst = |time, count| Burst { time, count: MinMaxCurve::constant(count), cycles: 1, interval: 0.01, probability: 1.0 };
        ParticleSystemData {
            duration: 1.0,
            start_delay: MinMaxCurve::constant(0.1),
            start_lifetime: MinMaxCurve::constant(5.0),
            start_speed: MinMaxCurve::constant(2.0),
            max_particles: 1000,
            shape: Some(Shape {
                kind: 4,
                radius: 0.4,
                radius_thickness: 1.0,
                angle: 30.0,
                length: 5.0,
                arc: 360.0,
                position: [0.0; 3],
                rotation: [0.0; 3],
                scale: [1.0; 3],
                align_to_direction: false,
                random_direction: 0.0,
                spherical_direction: 0.0,
                random_position: 0.0,
            }),
            emission: Some(Emission { rate_over_time: MinMaxCurve::constant(0.0), rate_over_distance: MinMaxCurve::constant(0.0), bursts: vec![burst(0.0, 4.0), burst(0.03, 3.0)] }),
            limit_velocity: None,
            size_over_lifetime: None,
            rotation_over_lifetime: None,
            color_over_lifetime: None,
            ..fume()
        }
    }

    #[test]
    fn a_system_played_bursts_after_its_delay_on_its_shape() {
        let mut system = ParticleSystem::seeded(Arc::new(played()), 3);
        system.play(Placement::default());
        system.step(0.05);
        assert!(system.particles.is_empty(), "not before its delay");
        system.step(0.06);
        assert_eq!(system.particles.len(), 4, "the first burst");
        system.step(0.03);
        assert_eq!(system.particles.len(), 7, "the second, once");
        for _ in 0..200 {
            system.step(0.01);
        }
        assert_eq!(system.particles.len(), 7, "no more after its cycle");
        // Born on the cone's base, going out within its angle.
        let mut fresh = ParticleSystem::seeded(Arc::new(played()), 9);
        fresh.play(Placement::default());
        fresh.step(0.1);
        for p in &fresh.particles {
            let away = (p.position[0].powi(2) + p.position[1].powi(2)).sqrt();
            assert!(away <= 0.4 + 0.2 * 0.001 + 1e-4, "{:?}", p.position);
            let lean = (p.velocity[2] / length(p.velocity)).acos().to_degrees();
            assert!(lean <= 30.0 + 1e-3 && (length(p.velocity) - 2.0).abs() < 1e-4, "{lean}");
        }
        // A rate: so many a second.
        let mut rated = played();
        rated.emission.as_mut().unwrap().bursts.clear();
        rated.emission.as_mut().unwrap().rate_over_time = MinMaxCurve::constant(20.0);
        let mut system = ParticleSystem::seeded(Arc::new(rated), 1);
        system.play(Placement::default());
        for _ in 0..60 {
            system.step(1.0 / 60.0);
        }
        assert!((17..=19).contains(&system.particles.len()), "{}", system.particles.len());
    }

    #[test]
    fn a_sphere_s_particles_go_out_from_its_middle() {
        let mut data = played();
        if let Some(shape) = data.shape.as_mut() {
            shape.kind = 0;
            shape.radius = 1.0;
        }
        let mut system = ParticleSystem::seeded(Arc::new(data), 4);
        system.play(Placement { position: [10.0, 0.0, 0.0], ..Placement::default() });
        system.step(0.2);
        assert!(!system.particles.is_empty());
        for p in &system.particles {
            let from = [p.position[0] - 10.0, p.position[1], p.position[2]];
            assert!(length(from) <= 1.0 + 0.02 && (length(p.velocity) - 2.0).abs() < 1e-3);
            // Its way is outward: along where it is, but for its motion
            // since it was born.
            assert!(from[0] * p.velocity[0] + from[1] * p.velocity[1] + from[2] * p.velocity[2] > 0.0);
        }
    }

    #[test]
    fn a_sheet_s_frame_is_its_cell() {
        use trp_data::particles::TextureSheet;
        let mut data = played();
        data.texture_sheet = Some(TextureSheet {
            mode: 0,
            tiles: [4, 4],
            single_row: false,
            random_row: true,
            row_index: 0,
            frame_over_time: MinMaxCurve::constant(0.3),
            start_frame: MinMaxCurve::constant(0.0),
            cycles: 1.0,
            time_mode: 0,
        });
        let mut system = ParticleSystem::seeded(Arc::new(data), 2);
        system.play(Placement::default());
        system.step(0.11);
        // 0.3 of 16 frames is frame 4: the second row's first cell, its
        // rows counted from the top.
        let d = system.drawn().next().unwrap();
        assert_eq!(d.sheet, [0.0, 0.5, 0.25, 0.75]);
    }

    #[test]
    fn a_local_system_is_drawn_where_it_is() {
        let mut data = played();
        data.world_space = false;
        let mut system = ParticleSystem::seeded(Arc::new(data), 6);
        // Turned a quarter about x: its z (the cone's axis) is the world's
        // minus y... as the effect's root turns it.
        let q = [std::f32::consts::FRAC_1_SQRT_2, 0.0, 0.0, std::f32::consts::FRAC_1_SQRT_2];
        system.play(Placement { position: [0.0, 1.0, 0.0], rotation: q, scale: [1.0; 3] });
        system.step(0.11);
        for d in system.drawn() {
            // The cone's axis (z) turned to the world's minus y.
            assert!(d.velocity[1] < -1.0, "{:?}", d.velocity);
            assert!((d.position[1] - 1.0).abs() < 0.5);
        }
    }
}
