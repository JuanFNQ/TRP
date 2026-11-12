//! Tests of particles meeting the ground and the rays shared among systems.

use super::*;

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

/// A falling particle meets the ground: its normal speed reflected and
/// kept by the bounce, all of it by one less the dampen; put back above
/// the ground by its move past it; a share of its life gone; one too
/// slow killed; each reported for the sub-emitters.
#[test]
fn a_particle_meets_the_ground() {
    let collision = trp_data::drawing::particles::Collision {
        world: true,
        dampen: MinMaxCurve::constant(0.2),
        bounce: MinMaxCurve::constant(0.5),
        lifetime_loss: MinMaxCurve::constant(0.05),
        min_kill_speed: 0.17,
        max_kill_speed: 10_000.0,
        radius_scale: 0.01,
        quality: 0,
        voxel_size: 0.5,
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

pub(super) fn low(quality: i64) -> ParticleSystemData {
    let collision = trp_data::drawing::particles::Collision {
        world: true,
        dampen: MinMaxCurve::constant(0.0),
        bounce: MinMaxCurve::constant(0.0),
        lifetime_loss: MinMaxCurve::constant(0.0),
        min_kill_speed: 0.0,
        max_kill_speed: 10_000.0,
        radius_scale: 0.01,
        quality,
        voxel_size: 0.2,
        collides_with: 4097,
    };
    ParticleSystemData { collision: Some(collision), limit_velocity: None, size_over_lifetime: None, gravity_modifier: MinMaxCurve::constant(0.0), world_space: true, ..fume() }
}

fn falling(system: &mut ParticleSystem, at: [f32; 3], seed: u32) {
    system.emit(EmitParams { position: at, velocity: [0.0, -5.0, 0.0], size: 0.2, lifetime: 5.0, color: [255; 4], rotation: 0.0, seed });
}

/// The frame's rays shared: an even share each, at most a system's
/// particles; what the small leave goes to the first in turn. A system
/// alive with none counts in the share.
#[test]
fn the_rays_are_shared_in_two_passes() {
    let data = Arc::new(low(1));
    let mut systems: Vec<ParticleSystem> = (0..4).map(|_| ParticleSystem::new(data.clone())).collect();
    for (s, n) in systems.iter_mut().zip([10, 90, 95, 0]) {
        for k in 0..n {
            falling(s, [0.0, 1.0, 0.0], k);
        }
    }
    // The last alive with none: played.
    systems[3].play(Placement::default());
    let mut all: Vec<&mut ParticleSystem> = systems.iter_mut().collect();
    assign_ray_budgets(&mut all, 128);
    // A share of 32: 10, 32, 32, 0; then the 54 left to the second,
    // none to the third.
    assert_eq!(systems.iter().map(|s| s.cubes.assigned()).collect::<Vec<_>>(), vec![10, 86, 32, 0]);
}

/// A low system takes part on its fifth frame and every fifth after;
/// a medium one on every frame.
#[test]
fn a_low_system_takes_part_one_frame_in_five() {
    let (mut slow, mut every) = (ParticleSystem::new(Arc::new(low(2))), ParticleSystem::new(Arc::new(low(1))));
    falling(&mut slow, [0.0, 1.0, 0.0], 1);
    falling(&mut every, [0.0, 1.0, 0.0], 1);
    let mut frames = Vec::new();
    for frame in 1..=11 {
        assign_ray_budgets(&mut [&mut slow, &mut every], 256);
        assert_eq!(every.cubes.assigned(), 1);
        if slow.cubes.assigned() > 0 {
            frames.push(frame);
        }
    }
    assert_eq!(frames, vec![5, 10]);
}

/// A ray met the ground far below where the particle was: the plane is
/// kept, not met. Frames later, with no rays at all, the particle
/// (from the same cube, going the same way) crosses it and meets it;
/// one in the next cube over finds nothing and falls through.
#[test]
fn a_kept_plane_is_met_where_the_particle_crosses_it() {
    let mut system = ParticleSystem::new(Arc::new(low(1)));
    falling(&mut system, [0.05, 0.40, 0.05], 1);
    assign_ray_budgets(&mut [&mut system], 256);
    assert!(system.step_frame(0.02, 1.0, Some(&Ground)).is_empty());
    assert_eq!(system.cubes.planes(), 1);
    // A second, a cube over in x, with no ray of its own.
    falling(&mut system, [0.25, 0.25, 0.05], 2);
    let mut met = Vec::new();
    for _ in 0..3 {
        met.extend(system.step_frame(0.02, 1.0, Some(&Ground)));
    }
    assert_eq!(met.len(), 1);
    assert_eq!(met[0].seed, 1);
    assert!(system.particles[0].position[1] > 0.0 && system.particles[0].velocity[1].abs() < 1e-6, "{:?}", system.particles[0]);
    assert!(system.particles[1].position[1] < 0.0, "{:?}", system.particles[1]);
}

/// With none of the frame's rays given, nothing is ever met.
#[test]
fn with_no_rays_nothing_is_met() {
    let mut system = ParticleSystem::new(Arc::new(low(2)));
    falling(&mut system, [0.05, 0.05, 0.05], 1);
    assert!(system.step_frame(0.02, 1.0, Some(&Ground)).is_empty());
    assert!(system.particles[0].position[1] < 0.0);
}

/// The cursor: with rays for four of six, the first four get them;
/// on the next frame the last two and, wrapped, the first two.
#[test]
fn the_rays_go_round_the_particles() {
    let mut system = ParticleSystem::new(Arc::new(low(1)));
    for k in 0..6 {
        falling(&mut system, [0.05 + 0.2 * k as f32, 0.40, 0.05], k);
    }
    system.cubes.assigned = 4;
    system.step_frame(0.02, 1.0, Some(&Ground));
    assert_eq!(system.cubes.planes(), 4);
    assert_eq!(system.cubes.cursor, 4);
    system.cubes.assigned = 4;
    system.step_frame(0.02, 1.0, Some(&Ground));
    // The two new, then wrapped: the cursor past the end.
    assert_eq!(system.cubes.planes(), 6);
    assert_eq!(system.cubes.cursor, 8);
    system.cubes.assigned = 4;
    system.step_frame(0.02, 1.0, Some(&Ground));
    assert_eq!(system.cubes.cursor, 4);
}
