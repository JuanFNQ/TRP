//! Tests of the particle system, with their shared helpers.
use super::*;
use trp_data::formats::curve::{Curve, Key};
use trp_data::drawing::particles::{Gradient, LimitVelocity, MinMaxCurve, MinMaxGradient, ParticleMaterial, ParticleRenderer};

mod collision;
mod lifetime;
mod playing;



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
        culling_mode: 3,
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
