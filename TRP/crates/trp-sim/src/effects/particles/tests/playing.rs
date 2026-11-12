//! Tests of playing a system: bursts, shapes, pauses, long frames, sheets and local space.

use super::*;

/// A system of the explosion's kind: a cone, two bursts, a rate.
fn played() -> ParticleSystemData {
    use trp_data::drawing::particles::{Burst, Emission, Shape};
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
        // Within the sphere, moved by its speed for its age: both
        // bursts fall in the step, and the engine lives all of a step's
        // burst particles through the time after the last that fired
        // (0.03 s after the delay's end, 0.07 s before the step's).
        let from = [p.position[0] - 10.0, p.position[1], p.position[2]];
        assert!((p.age - 0.07).abs() < 1e-5, "{}", p.age);
        assert!(length(from) <= 1.0 + 2.0 * 0.07 + 0.02 && (length(p.velocity) - 2.0).abs() < 1e-3);
        // Its way is outward: along where it is, but for its motion
        // since it was born.
        assert!(from[0] * p.velocity[0] + from[1] * p.velocity[1] + from[2] * p.velocity[2] > 0.0);
    }
}

#[test]
fn a_particle_born_in_a_step_lives_to_its_end() {
    // The first burst at the delay's end (0.1 s), the second 0.03 s
    // after: each lived through the rest of its step, as the recorded
    // grenade's systems are first seen.
    let mut system = ParticleSystem::seeded(Arc::new(played()), 5);
    system.play(Placement::default());
    system.step(0.11);
    assert_eq!(system.particles.len(), 4);
    for p in &system.particles {
        assert!((p.age - 0.01).abs() < 1e-5, "{}", p.age);
    }
    let before: Vec<[f32; 3]> = system.particles.iter().map(|p| p.position).collect();
    system.step(0.025);
    assert_eq!(system.particles.len(), 7, "the second burst, at 0.03 s");
    for (p, at) in system.particles.iter().zip(&before) {
        // The first four a step on, by their velocity.
        let moved = length(sub(p.position, *at));
        assert!((moved - 2.0 * 0.025).abs() < 1e-4, "{moved}");
    }
    for p in &system.particles[4..] {
        assert!((p.age - 0.005).abs() < 1e-5, "{}", p.age);
    }
}

#[test]
fn a_system_paused_unseen_takes_no_step_when_played() {
    // `cullingMode` 1, played with nothing to draw: its first frame's
    // time is lost, not caught up.
    let mut data = played();
    data.start_delay = MinMaxCurve::constant(0.0);
    data.culling_mode = 1;
    let mut system = ParticleSystem::seeded(Arc::new(data.clone()), 2);
    system.play(Placement::default());
    system.step(0.02);
    assert!(system.particles.is_empty(), "nothing the frame it is played");
    system.step(0.02);
    assert_eq!(system.particles.len(), 4);
    assert!(system.particles.iter().all(|p| (p.age - 0.02).abs() < 1e-5));
    data.culling_mode = 3;
    let mut always = ParticleSystem::seeded(Arc::new(data), 2);
    always.play(Placement::default());
    always.step(0.02);
    assert_eq!(always.particles.len(), 4, "always simulated: born at once");
}

#[test]
fn a_long_frame_is_stepped_in_equal_parts() {
    // 0.039 s with a longest step of 0.03: two steps of 0.0195, as the
    // engine makes it (`courses3`'s long frames, "A long frame"); a
    // frame within it, one step.
    let mut data = played();
    data.start_delay = MinMaxCurve::constant(0.0);
    data.gravity_modifier = MinMaxCurve::constant(1.0);
    let data = Arc::new(data);
    let mut whole = ParticleSystem::seeded(data.clone(), 7);
    whole.play(Placement::default());
    whole.step(0.01);
    let mut parts = ParticleSystem::seeded(data.clone(), 7);
    parts.play(Placement::default());
    parts.step(0.01);
    let mut by_hand = ParticleSystem::seeded(data, 7);
    by_hand.play(Placement::default());
    by_hand.step(0.01);
    parts.step_frame(0.039, 0.03, None);
    by_hand.step(0.0195);
    by_hand.step(0.0195);
    whole.step_frame(0.039, 1.0, None);
    assert!(!parts.particles.is_empty());
    for (a, b) in parts.particles.iter().zip(&by_hand.particles) {
        assert_eq!(a.position, b.position);
        assert_eq!(a.age, b.age);
    }
    // Stepped whole, a falling particle comes out lower by g dt² / 4.
    let low = parts.particles.iter().zip(&whole.particles).map(|(a, b)| a.position[1] - b.position[1]).fold(0.0f32, f32::max);
    assert!(low > 0.0, "the whole step falls further");
}

#[test]
fn a_burst_between_two_counts_draws_each_evenly() {
    use trp_data::drawing::particles::Burst;
    let count = MinMaxCurve { mode: 3, scalar: 7.0, min_scalar: 4.0, ..MinMaxCurve::constant(0.0) };
    let burst = Burst { time: 0.0, count, cycles: 1, interval: 0.01, probability: 1.0 };
    let mut rng = Rng::new(11);
    let mut seen = [0usize; 8];
    for _ in 0..8000 {
        seen[burst_count(&burst, 0.0, &mut rng).unwrap() as usize] += 1;
    }
    // 4, 5, 6 and 7 a quarter each; the ends as often as the middle.
    assert_eq!(seen[..4], [0, 0, 0, 0]);
    for &n in &seen[4..] {
        assert!((1800..2200).contains(&n), "{seen:?}");
    }
    let cut = Burst { count: MinMaxCurve::constant(7.9), ..burst.clone() };
    assert_eq!(burst_count(&cut, 0.0, &mut rng), Some(7), "a constant cut to its whole part");
    let never = Burst { probability: 0.0, ..burst };
    assert_eq!(burst_count(&never, 0.0, &mut rng), None);
}

#[test]
fn a_sheet_s_frame_is_its_cell() {
    use trp_data::drawing::particles::TextureSheet;
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
