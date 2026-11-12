//! What the camera draws of the explosions each frame.

use super::*;

/// What a frame's draws share, whichever camera draws them: the light's
/// figures, the time of day's harmonics, the grenade smoke's figures, the
/// engine's vertex lights, the way to the sun, the time.
pub(super) struct FrameFigures {
    pub(super) light: [Vec4; 4],
    pub(super) harmonics: [Vec4; 7],
    pub(super) smoke_globals: [Vec4; 4],
    pub(super) frame_lights: Vec<VertexLight>,
    pub(super) sun: Vec3,
    pub(super) time: f32,
}

/// A billboard's size as the engine draws it (particles spec, "Drawn":
/// `PrepareForRender`, `GenerateParticleGeometry`): the renderer's least and most
/// (`m_MinParticleSize`, `m_MaxParticleSize`) are shares of the view's
/// width where the particle is (`width`: at its depth along the view for a
/// perspective camera, below nought behind it); its size raised to the
/// least and held to the most, the least below nought making it nought, the
/// most below nought holding nothing.
pub(super) fn held_size(size: f32, least: f32, most: f32, width: f32) -> f32 {
    let finite = |v: f32| v.clamp(-f32::MAX, f32::MAX);
    let (low, high) = (finite(least * width), finite(most * width));
    let s = size.max(1e-6);
    let half = if low >= 0.0 { s.max(low) * 0.5 } else { 0.0 };
    let half = if high >= 0.0 { half.min(high * 0.5) } else { half };
    size * 2.0 * half / s
}

/// The systems' draws for one camera, in the engine's order among
/// transparent things: each system's particles and trails made facing that
/// camera and sorted from it, as the engine makes a particle system's for
/// each camera that draws it (the main camera, and the optic camera while
/// on: the effects' objects are of the world's layer).
pub(super) fn camera_draws(explosions: &Explosions, frame: &FrameFigures, camera: &Camera, eye: &GlobalTransform, near: f32) -> Vec<SystemDraw> {
    let FrameFigures { light, harmonics, smoke_globals, frame_lights, .. } = frame;
    let (light, harmonics, smoke_globals) = (*light, *harmonics, *smoke_globals);
    let view_from_world = eye.to_matrix().inverse();
    let clip_from_view = camera.clip_from_view();
    let clip_from_world = clip_from_view * view_from_world;
    // The view's width at a depth of one (a perspective camera's), or
    // whole (an orthographic one's), for a billboard's least and most size.
    let perspective = clip_from_view.w_axis.w == 0.0;
    let view_width = 2.0 / clip_from_view.x_axis.x;
    let (right, up, forward) = (eye.right().as_vec3(), eye.up().as_vec3(), eye.forward().as_vec3());
    let facing = -forward;
    // The way to the app's sun, in the view's axes.
    let sun = view_from_world.transform_vector3(frame.sun).normalize_or_zero();
    let mut draws: Vec<(i64, f32, SystemDraw)> = Vec::new();
    let game = crate::platform::convert::position;
    let mediators = explosions.copies.values().flat_map(|copies| copies.shown.iter().flatten().flat_map(|systems| copies.effect.systems.iter().zip(systems)));
    let older = explosions.shared.iter().map(|(ready, system)| (ready, system));
    let launchers = explosions.blasts.iter().flat_map(|b| b.effect.systems.iter().zip(&b.systems));
    {
        for (ready, system) in mediators.chain(older).chain(launchers) {
            // Its trails first, a draw of their own.
            if let Some(trail) = &ready.trail {
                let trails = system.drawn_trails();
                let mut corners: Vec<f32> = Vec::new();
                let mut indices: Vec<u32> = Vec::new();
                let mut middle = Vec3::ZERO;
                let mut count = 0.0f32;
                for points in trails.iter().filter(|t| t.len() >= 2) {
                    let first = (corners.len() / 12) as u32;
                    for (i, p) in points.iter().enumerate() {
                        let at = game(p.position);
                        let (prev, next) = (game(points[i.saturating_sub(1)].position), game(points[(i + 1).min(points.len() - 1)].position));
                        let along = (next - prev).normalize_or_zero();
                        let across = along.cross(eye.translation() - at).normalize_or_zero() * (p.width * 0.5);
                        middle += at;
                        count += 1.0;
                        for (side, v) in [(-1.0f32, 0.0f32), (1.0, 1.0)] {
                            let c = at + across * side;
                            corners.extend_from_slice(&[c.x, c.y, c.z, p.along, v, p.color[0], p.color[1], p.color[2], p.color[3], facing.x, facing.y, facing.z]);
                        }
                    }
                    for i in 0..points.len() as u32 - 1 {
                        let (a, b) = (first + 2 * i, first + 2 * i + 2);
                        indices.extend([a, a + 1, b + 1, a, b + 1, b]);
                    }
                }
                if !indices.is_empty() {
                    middle /= count.max(1.0);
                    let (vl_position, vl_color, vl_atten, vl_spot) = vertex_lights(frame_lights, middle);
                    let uniform = EffectUniform {
                        clip_from_world,
                        view_from_world,
                        camera: Vec4::new(near, frame.time / 20.0, frame.time, 0.0),
                        light: light[0],
                        light_view: sun.extend(0.0),
                        ambient: light[1],
                        probe: light[2],
                        level: light[3],
                        sh: harmonics,
                        smoke_a: smoke_globals[0],
                        smoke_b: smoke_globals[1],
                        smoke_c: smoke_globals[2],
                        smoke_d: smoke_globals[3],
                        vl_position,
                        vl_color,
                        vl_atten,
                        vl_spot,
                        ..trail.figures
                    };
                    let distance = (middle - eye.translation()).length() + ready.fudge;
                    draws.push((trail.queue, distance, SystemDraw { look: trail.look, uniform, pictures: trail.pictures.clone(), corners: Arc::new(corners), indices: Some(Arc::new(indices)) }));
                }
            }
            let Some(look) = ready.look else { continue };
            // A testing aid: the shaders named in `TRP_EFFECTS_LEFT_OUT`
            // (their entries, `distortion_wave,heat_distortion`) not drawn,
            // to set a capture beside one with them.
            if std::env::var("TRP_EFFECTS_LEFT_OUT").is_ok_and(|out| out.split(',').any(|name| name == look.entry())) {
                continue;
            }
            let mesh = ready.data.renderer.mesh.as_ref().filter(|_| ready.render_mode == 4);
            if system.particles.is_empty() || !(matches!(ready.render_mode, 0 | 1) || mesh.is_some()) {
                continue;
            }
            let mut corners = Vec::with_capacity(system.particles.len() * 48);
            let mut indices: Vec<u32> = Vec::new();
            let mut middle = Vec3::ZERO;
            let mut count = 0.0f32;
            // In the renderer's order (`m_SortMode`): as the system keeps
            // them (none: the engine's own order), far to near from the
            // camera, or by the life left (the engine's key for both
            // orders by age): the oldest in front, the youngest in front.
            let mut drawn: Vec<_> = system.drawn().collect();
            match ready.sort_mode {
                1 => drawn.sort_by(|a, b| (game(b.position) - eye.translation()).length_squared().total_cmp(&(game(a.position) - eye.translation()).length_squared())),
                2 => drawn.sort_by(|a, b| b.remaining.total_cmp(&a.remaining)),
                3 => drawn.sort_by(|a, b| a.remaining.total_cmp(&b.remaining)),
                _ => {}
            }
            for d in drawn {
                let at = game(d.position);
                middle += at;
                count += 1.0;
                let [u0, v0, u1, v1] = d.sheet;
                let corner = |p: Vec3, u: f32, v: f32, out: &mut Vec<f32>| {
                    out.extend_from_slice(&[p.x, p.y, p.z, u0 + (u1 - u0) * u, v0 + (v1 - v0) * v, d.color[0], d.color[1], d.color[2], d.color[3], facing.x, facing.y, facing.z]);
                };
                let velocity = game(d.velocity);
                if let Some(mesh) = mesh {
                    // The mesh at the particle, sized by it, turned as the
                    // system is (alignment 2, its own axes) and by the
                    // particle's turn about its z (`PROVISIONAL`: a turn of
                    // three axes not read); in the game's axes, then the
                    // app's.
                    let first = (corners.len() / 12) as u32;
                    let turn = Quat::from_array(system.placement.rotation) * Quat::from_rotation_z(d.rotation.to_radians());
                    for (k, p) in mesh.positions.iter().enumerate() {
                        let local = turn * (Vec3::from_array(*p) * d.size);
                        let place = game(add3(d.position, local.to_array()));
                        let n = mesh.normals.get(k).map_or(Vec3::Y, |n| game((turn * Vec3::from_array(*n)).to_array()));
                        let uv = mesh.uv0.get(k).copied().unwrap_or([0.0, 0.0]);
                        corners.extend_from_slice(&[place.x, place.y, place.z, uv[0], uv[1], d.color[0], d.color[1], d.color[2], d.color[3], n.x, n.y, n.z]);
                    }
                    for submesh in &mesh.submeshes {
                        indices.extend(submesh.iter().map(|i| first + i));
                    }
                } else if ready.render_mode == 1 && velocity.length_squared() > 1e-10 {
                    // Stretched along its way (`PROVISIONAL`: the particle
                    // at the head, its tail behind by its length; its
                    // picture's u along it), its width its size.
                    let r = &ready.data.renderer;
                    let along = velocity.normalize();
                    let length = d.size * r.length_scale + velocity.length() * r.velocity_scale;
                    let across = along.cross(at - eye.translation()).normalize_or_zero() * (d.size * 0.5);
                    let tail = at - along * length;
                    corner(tail - across, 0.0, 0.0, &mut corners);
                    corner(at - across, 1.0, 0.0, &mut corners);
                    corner(at + across, 1.0, 1.0, &mut corners);
                    corner(tail + across, 0.0, 1.0, &mut corners);
                } else {
                    // Facing the view, turned in its plane by its turn, its
                    // size held within the renderer's least and most on the
                    // screen (`held_size`).
                    let r = &ready.data.renderer;
                    let width = if perspective { view_width * -view_from_world.transform_point3(at).z } else { view_width };
                    let size = held_size(d.size, r.min_size, r.max_size, width);
                    let (s, c) = d.rotation.to_radians().sin_cos();
                    for (x, y) in [(-0.5f32, -0.5f32), (0.5, -0.5), (0.5, 0.5), (-0.5, 0.5)] {
                        let (rx, ry) = (x * c - y * s, x * s + y * c);
                        corner(at + (right * rx + up * ry) * size, x + 0.5, y + 0.5, &mut corners);
                    }
                }
            }
            middle /= count.max(1.0);
            let distance = (middle - eye.translation()).length() + ready.fudge;
            let (vl_position, vl_color, vl_atten, vl_spot) = if look.vertex_lit() > 0.0 { vertex_lights(frame_lights, middle) } else { ([Vec4::ZERO; 8], [Vec4::ZERO; 8], [Vec4::ZERO; 8], [Vec4::ZERO; 8]) };
            let uniform = EffectUniform {
                clip_from_world,
                view_from_world,
                camera: Vec4::new(near, frame.time / 20.0, frame.time, 0.0),
                light: light[0],
                light_view: sun.extend(0.0),
                ambient: light[1],
                probe: light[2],
                level: light[3],
                sh: harmonics,
                smoke_a: smoke_globals[0],
                smoke_b: smoke_globals[1],
                smoke_c: smoke_globals[2],
                smoke_d: smoke_globals[3],
                vl_position,
                vl_color,
                vl_atten,
                vl_spot,
                ..ready.figures
            };
            let indices = mesh.map(|_| Arc::new(indices));
            draws.push((ready.queue, distance, SystemDraw { look, uniform, pictures: ready.pictures.clone(), corners: Arc::new(corners), indices }));
        }
    }
    // By queue, then far to near.
    draws.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.total_cmp(&a.1)));
    draws.into_iter().map(|(_, _, d)| d).collect()
}
