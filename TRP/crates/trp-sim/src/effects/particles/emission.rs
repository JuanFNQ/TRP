//! Playing a system and emitting particles from it.

use super::*;

impl ParticleSystem {
    pub fn new(data: Arc<ParticleSystemData>) -> Self {
        Self { data, particles: Vec::new(), placement: Placement::default(), playing: None, unseen: false, rng: Rng::new(0x5eed), cubes: Cubes::default() }
    }

    /// One whose own draws (its start values, shape, bursts) come from
    /// `seed`.
    pub fn seeded(data: Arc<ParticleSystemData>, seed: u64) -> Self {
        Self { rng: Rng::new(seed), ..Self::new(data) }
    }

    /// Played where it is (`Play`, the start of its cycle): its delay
    /// drawn, its bursts to come. A system that pauses while its renderer
    /// is not seen (`cullingMode` 1 or 2, or 0 and looping), played with
    /// nothing to draw, takes no step on the frame it is played: its
    /// renderer has not been seen drawn, and the step's time is not caught
    /// up (recorder 0.11.0's `courses3`, the grenade's `explosion_center`
    /// and `HeatDistortion` a frame behind the effect's other systems from
    /// their first frame; particles spec, "Compared with"). `PROVISIONAL`:
    /// modes 2 and 0 taken as 1; seen on every frame after (a system out
    /// of the camera's sight is never paused here).
    pub fn play(&mut self, placement: Placement) {
        self.placement = placement;
        let delay = self.data.start_delay.evaluate(0.0, self.rng.unit()).max(0.0);
        let bursts = self.data.emission.as_ref().map_or(0, |e| e.bursts.len());
        self.playing = Some(Playing { time: 0.0, delay, cycles_done: vec![0; bursts], carried: 0.0, done: false });
        let pauses = matches!(self.data.culling_mode, 1 | 2) || (self.data.culling_mode == 0 && self.data.looping);
        self.unseen = pauses && self.particles.is_empty();
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

    /// One emitted with its place, velocity, size and life given and the
    /// rest the system's start values at its cycle's start
    /// (`ParticleSystem.Emit(EmitParams)` with only those four set): its
    /// colour, its turn (and its flip) and its seed drawn; whether it was
    /// (none past its most alive, none with no life).
    pub fn emit_given(&mut self, position: [f32; 3], velocity: [f32; 3], size: f32, lifetime: f32) -> bool {
        if self.particles.len() >= self.data.max_particles as usize || lifetime <= 0.0 {
            return false;
        }
        let data = self.data.clone();
        let rng = &mut self.rng;
        let rotation = data.start_rotation.evaluate(0.0, rng.unit()).to_degrees();
        let color = data.start_color.evaluate(0.0, rng.unit());
        let seed = (rng.unit() * u32::MAX as f32) as u32;
        let mut p = Particle { position, velocity, size, color, rotation, lifetime, age: 0.0, seed, trail: if data.trails.is_some() { vec![(position, 0.0)] } else { Vec::new() } };
        if p.flipped(&data) {
            p.rotation = -p.rotation;
        }
        self.particles.push(p);
        true
    }

    /// So many born on its shape with its start values, at the share
    /// `cycle` of its cycle; how many were (none past its most alive, none
    /// with no life).
    fn emit_shaped(&mut self, count: u32, cycle: f32) -> usize {
        let data = self.data.clone();
        let mut born = 0;
        for _ in 0..count {
            if self.particles.len() >= data.max_particles as usize {
                return born;
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
                born += 1;
            }
        }
        born
    }

    /// The system's own emission over the time passed: its rate, and its
    /// bursts whose times fall in it; for each particle born, in the order
    /// born, the time from its birth to the step's end, which the engine
    /// lives it through at once (`ParticleSystem::Update1Incremental`,
    /// `StartParticles`, `StartModules`, `EmissionModule::EmitCommon`,
    /// `AccumulateBursts`, decompiled; particles spec, "What happens to a
    /// particle"): the rate's first, the `i`th of them through
    /// `(i + carried) / amount` of the emitting time (`carried` the share of
    /// a particle left over after this step's, `amount` the rate's
    /// particles this step); then the bursts', each through the share of
    /// the emitting time after the last burst that fired (its first
    /// cycle's time: the engine keeps one share for them all); each share
    /// held between a millionth and the whole.
    pub(super) fn emit_playing(&mut self, dt: f32) -> Vec<f32> {
        let mut aheads = Vec::new();
        let Some(mut playing) = self.playing.take() else { return aheads };
        let before = playing.time - playing.delay;
        playing.time += dt;
        let now = playing.time - playing.delay;
        let duration = self.data.duration.max(1e-3);
        if !playing.done && now >= 0.0 {
            let data = self.data.clone();
            if let Some(emission) = &data.emission {
                // The emitting time ends at the cycle's end: the engine
                // holds a system's time there (`Update1Incremental`).
                let start = before.max(0.0);
                let end = if data.looping { now } else { now.min(duration) };
                let span = (end - start).max(0.0);
                let share = |s: f32| span * s.clamp(1e-6, 1.0);
                let rate = emission.rate_over_time.evaluate((now / duration).min(1.0), self.rng.unit());
                let amount = rate * span;
                playing.carried += amount;
                let whole = playing.carried.floor();
                let mut from_rate = 0;
                if whole >= 1.0 {
                    playing.carried -= whole;
                    from_rate = whole as u32;
                }
                // The bursts that fire: each while its time is before the
                // step's end.
                let mut fired: Vec<(u32, f32)> = Vec::new();
                let mut burst_share = 1.0;
                for (b, burst) in emission.bursts.iter().enumerate() {
                    let cycles = if burst.cycles <= 0 { i64::MAX } else { burst.cycles };
                    while playing.cycles_done[b] < cycles {
                        let at = burst.time + playing.cycles_done[b] as f32 * burst.interval.max(1e-4);
                        if at >= end || (at > duration && !data.looping) {
                            break;
                        }
                        playing.cycles_done[b] += 1;
                        burst_share = 1.0 - if span > 0.0 { ((burst.time - start) / span).clamp(0.0, 1.0) } else { 0.0 };
                        if let Some(count) = burst_count(burst, end / duration, &mut self.rng) {
                            fired.push((count, (at / duration).min(1.0)));
                        }
                    }
                }
                let spacing = if amount >= 1e-4 { 1.0 / amount } else { 1.0 };
                let born = self.emit_shaped(from_rate, (now / duration).min(1.0));
                aheads.extend((0..born).map(|i| share((i as f32 + playing.carried) * spacing)));
                for (count, cycle) in fired {
                    let born = self.emit_shaped(count, cycle);
                    aheads.extend(std::iter::repeat_n(share(burst_share), born));
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
        aheads
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
            if let Some(count) = burst_count(burst, cycle, &mut self.rng) {
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
}

/// A burst's particles as the engine counts them
/// (`EmissionModule::AccumulateBurst`, `RangedRandom`, decompiled): none
/// where its chance fails (a chance below one fires where a draw falls
/// below it; nought never); between two constants a whole number drawn
/// evenly from the lesser to the greater, both counted; a constant or a
/// curve (at `cycle` of its cycle) cut to its whole part.
pub(super) fn burst_count(burst: &trp_data::drawing::particles::Burst, cycle: f32, rng: &mut Rng) -> Option<u32> {
    if burst.probability <= 0.0 || (burst.probability < 1.0 && rng.unit() >= burst.probability) {
        return None;
    }
    let c = &burst.count;
    let n = match c.mode {
        3 => {
            let (lo, hi) = (c.min_scalar.min(c.scalar).trunc(), c.min_scalar.max(c.scalar).trunc());
            let span = hi - lo + 1.0;
            lo + (rng.unit() * span).floor().min(span - 1.0)
        }
        0 => c.scalar.trunc(),
        _ => c.evaluate(cycle, rng.unit()).trunc(),
    };
    Some(n.max(0.0) as u32)
}
