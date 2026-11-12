//! Tests of velocity over life, trails, sub-emitters, varying values, puffs, births and drag.

use super::*;

/// The velocity over life moves a particle by its own velocity plus the
/// module's, and leaves its own velocity as it was (the engine's added
/// velocity, emptied each step).
#[test]
fn the_velocity_over_life_is_added_and_not_kept() {
    let along = trp_data::drawing::particles::VelocityOverLifetime { x: MinMaxCurve::constant(0.0), y: MinMaxCurve::constant(0.0), z: MinMaxCurve::constant(1.0), world_space: true };
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

/// A particle with a trail drops a point each 0.2 m it goes; its trail
/// as drawn runs from where it is back to its birth, twice its size wide
/// (`widthOverTrail` 2, `sizeAffectsWidth`), white (the trail's colours,
/// not the particle's).
#[test]
fn a_trail_follows_its_particle() {
    let white = MinMaxGradient::constant([1.0; 4]);
    let trails = trp_data::drawing::particles::Trails {
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
    let emission = trp_data::drawing::particles::Emission {
        rate_over_time: MinMaxCurve::constant(0.0),
        rate_over_distance: MinMaxCurve::constant(0.0),
        bursts: vec![trp_data::drawing::particles::Burst { time: 0.0, count: MinMaxCurve::constant(3.0), cycles: 1, interval: 0.01, probability: 1.0 }],
    };
    let light = trp_data::drawing::particles::EffectLight { name: "Point Light".into(), position: [0.0; 3], kind: 2, color: [1.0, 0.8, 0.5, 1.0], intensity: 7.5, range: 7.0, enabled: false, driver: None };
    let lights = trp_data::drawing::particles::ParticleLights {
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
