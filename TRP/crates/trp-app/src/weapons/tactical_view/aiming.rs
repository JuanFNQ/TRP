//! Lamps placed, the kept turn, and lasers aimed.

use super::*;

/// The lights placed for the frame, after the frame's transforms and
/// before the weapon is squeezed for the drawing: the weapon's on their
/// objects (the place as drawn, the turn as Unity keeps it: the product of
/// the turns above it, whatever the scales); the head's by the camera
/// (`HeadLightsValidator.Update`): at the camera, moved 0.2 m to its side,
/// up and ahead along its own axes as turned the frame before, then turned
/// as the camera, 10° down and 3° toward the middle, and by the field of
/// view below 50. Seen from outside, the head's on their objects on the
/// model of what is worn, as the weapon's are: the game moves them so for
/// its own player's eyes alone (`GamePlayerOwner.HeadLightsValidator`,
/// in its `LateUpdate`), and another player's, which is what a view from
/// outside shows, stay where their model has them. (In the hideout, the
/// one place the game shows its own player from outside, it pauses the
/// moving: `HideoutPlayerOwner`, `SetPause(!FirstPersonMode)`.) The
/// weapons hung on the body, on their objects as the one in hand.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(super) fn place_lamps(
    mut state: ResMut<Tactical>,
    nodes: Query<(&Transform, Option<&ChildOf>), Without<TacticalLamp>>,
    globals: Query<&GlobalTransform, Without<TacticalLamp>>,
    cameras: Query<(&GlobalTransform, &Projection), (With<crate::MainCamera>, Without<TacticalLamp>)>,
    parts: Query<(&crate::player::body_view::HeadPart, &GlobalTransform), Without<TacticalLamp>>,
    carried: Res<Carried>,
    controller: Res<Controller>,
    heads: Option<Res<crate::player::body_view::HeadModels>>,
    mut lamps: Query<(&mut Transform, &mut GlobalTransform), With<TacticalLamp>>,
) {
    let state = &mut *state;
    if !state.on {
        return;
    }
    let mut put = |entity: Entity, at: Vec3, turn: Quat| {
        if let Ok((mut transform, mut global)) = lamps.get_mut(entity) {
            *transform = Transform { translation: at, rotation: turn, scale: Vec3::ONE };
            *global = GlobalTransform::from(*transform);
        }
    };
    if let Some((_, weapon_lamps)) = &state.weapon_lamps {
        for lamp in weapon_lamps {
            let Some(node) = lamp.node else { continue };
            let Ok(global) = globals.get(node) else { continue };
            put(lamp.entity, global.translation(), kept_turn(node, &nodes));
        }
    }
    // The dummies', those of the weapons hung on the body and the OVERALL
    // figure's weapon's, on their weapons' objects, as the weapon in hand's.
    for lamp in state.dummy_lamps.iter().chain(&state.slung_lamps).chain(&state.figure_lamps).flat_map(|(_, lamps)| lamps) {
        let Some(node) = lamp.node else { continue };
        let Ok(global) = globals.get(node) else { continue };
        put(lamp.entity, global.translation(), kept_turn(node, &nodes));
    }
    let Ok((camera, projection)) = cameras.single() else { return };
    let fov = match projection {
        Projection::Perspective(p) => p.fov.to_degrees(),
        _ => 75.0,
    };
    let camera_at = camera.translation();
    let camera_turn = crate::platform::convert::rotation_back(camera.to_scale_rotation_translation().1);
    let at_unity = [camera_at.x, camera_at.y, -camera_at.z];
    // The game places the head's lights in its `Update`, before the camera
    // moves: by the camera as the frame before left it, a frame behind the
    // view (recorded: `courses8\helmet-headlamp`, the light's way to 0.03
    // degrees by the frame before's camera, 2.6 by the frame's own).
    let (head_at, head_turn, head_fov) = state.head_camera.replace((at_unity, camera_turn, fov)).unwrap_or((at_unity, camera_turn, fov));
    state.head_camera_used = Some((head_at, head_turn, head_fov));
    let outside = controller.movement.body.point_of_view() == trp_sim::PointOfView::ThirdPerson;
    for (id, lamp) in state.head_lamps.iter_mut() {
        if outside {
            let template = state.head_found.iter().find(|(owner, _)| owner == id).map(|(_, template)| template.as_str());
            let object = template.and_then(|template| {
                let node = state.head_data.get(template)?.as_deref()?.lights.get(lamp.light)?.node;
                heads.as_deref()?.device_on_body(template)?.object(node).map(|(entity, _)| entity)
            });
            if let Some((entity, global)) = object.and_then(|entity| globals.get(entity).ok().map(|global| (entity, global))) {
                put(lamp.entity, global.translation(), kept_turn(entity, &nodes));
                continue;
            }
        }
        // Its side, by where its part sits as first seen (`IsOnRight`:
        // its offset along the camera's right), left where none is drawn.
        let side = *state.head_side.entry(*id).or_insert_with(|| {
            let template = carried.equipment.item.find(*id).map(|item| item.kind.template.clone()).unwrap_or_default();
            let right = Vec3::from(turn(camera_turn, [1.0, 0.0, 0.0]));
            parts
                .iter()
                .filter(|(part, _)| part.path.last().is_some_and(|(_, of)| *of == template))
                .map(|(_, global)| {
                    let p = global.translation();
                    Vec3::new(p.x - camera_at.x, p.y - camera_at.y, -(p.z - camera_at.z))
                })
                .filter(|offset| offset.length() < 0.6)
                .map(|offset| offset.dot(right) > 0.0)
                .next()
                .unwrap_or(false)
        });
        lamp.right = side;
        let (place, turned) = head_pose(head_at, head_turn, head_fov, side, lamp.turned);
        lamp.turned = turned;
        put(lamp.entity, crate::platform::convert::position(place), crate::platform::convert::rotation(lamp.turned));
    }
}

/// A Unity rotation looking along `forward` with `up` above it
/// (`Quaternion.LookRotation`), in the game's axes.
fn look_rotation(forward: [f32; 3], up: [f32; 3]) -> [f32; 4] {
    let z = Vec3::from(forward).normalize_or(Vec3::Z);
    let mut x = Vec3::from(up).cross(z);
    if x.length_squared() < 1e-12 {
        x = Vec3::X.cross(z).cross(z) * -1.0;
        if x.length_squared() < 1e-12 {
            x = Vec3::X;
        }
    }
    let x = x.normalize();
    let y = z.cross(x);
    Quat::from_mat3(&Mat3::from_cols(x, y, z)).normalize().to_array()
}

/// `Quaternion.Lerp`: the shorter way, the result made whole again.
fn nlerp(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    let dot: f32 = (0..4).map(|i| a[i] * b[i]).sum();
    let sign = if dot < 0.0 { -1.0 } else { 1.0 };
    let q = Quat::from_array(std::array::from_fn(|i| a[i] * (1.0 - t) + sign * b[i] * t));
    q.normalize().to_array()
}

/// The turn as Unity keeps it of a node of a rig (the product of the turns
/// above it, whatever the scales), in Bevy's axes.
pub(crate) fn kept_turn<F: bevy::ecs::query::QueryFilter>(node: Entity, nodes: &Query<(&Transform, Option<&ChildOf>), F>) -> Quat {
    let mut rotation = Quat::IDENTITY;
    let mut next = Some(node);
    while let Some(e) = next {
        let Ok((transform, parent)) = nodes.get(e) else { break };
        rotation = transform.rotation * rotation;
        next = parent.map(ChildOf::parent);
    }
    rotation
}

/// The weapon in hand's devices' states, in the order the game walks them,
/// as its rig has `count` of them (all off where they do not match).
pub(crate) fn weapon_states(carried: &Carried, held: &HandsHeld, count: usize) -> Vec<Light> {
    states_of(held.item.and_then(|id| carried.equipment.item.find(id)), count)
}

/// A weapon's devices' states, in the order the game walks them, as its
/// model has `count` of them (all off where they do not match, or with no
/// weapon).
pub(crate) fn states_of(weapon: Option<&Item>, count: usize) -> Vec<Light> {
    let states: Vec<Light> = weapon.map(|weapon| tac::devices(weapon).iter().map(|d| d.light).collect()).unwrap_or_default();
    if states.len() == count {
        states
    } else {
        vec![Light::default(); count]
    }
}

/// A laser's flat picture made once by its name.
fn picture_of(state: &mut Tactical, texture: Option<&Arc<trp_data::unity::extract::TextureData>>, images: &mut Assets<Image>) -> Option<Handle<Image>> {
    let texture = texture?;
    state.pictures.entry(texture.name.clone()).or_insert_with(|| crate::effects::particles_view::picture(texture, images)).clone()
}

/// The beam's 3D noise made once by its name: one byte a texel, repeating,
/// sampled smoothly.
fn volume_of(state: &mut Tactical, volume: Option<&Arc<Volume>>, images: &mut Assets<Image>) -> Option<Handle<Image>> {
    let volume = volume?;
    state
        .pictures
        .entry(format!("{} (3D)", volume.name))
        .or_insert_with(|| {
            let [w, h, d] = volume.size;
            let texels = (w * h * d) as usize;
            if texels == 0 || volume.data.len() < texels {
                warn!("the laser's noise {} is not one byte a texel; its beam is not drawn", volume.name);
                return None;
            }
            let mut image = Image::new(Extent3d { width: w, height: h, depth_or_array_layers: d }, TextureDimension::D3, volume.data[..texels].to_vec(), TextureFormat::R8Unorm, RenderAssetUsages::RENDER_WORLD);
            image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
                address_mode_u: ImageAddressMode::Repeat,
                address_mode_v: ImageAddressMode::Repeat,
                address_mode_w: ImageAddressMode::Repeat,
                mag_filter: ImageFilterMode::Linear,
                min_filter: ImageFilterMode::Linear,
                mipmap_filter: ImageFilterMode::Linear,
                ..default()
            });
            Some(images.add(image))
        })
        .clone()
}

/// The first thing a laser's ray meets, in the game's axes: what a bullet
/// meets on the range and the dummies (`PROVISIONAL`: the range's colliders
/// for the game's mask, the terrain, the detailed colliders, the hit
/// colliders and the dead: layers 11, 12, 16 and 23).
fn first_hit(range: &crate::world::range::Range, dummies: Option<&crate::world::dummy::Dummies>, from: [f32; 3], to: [f32; 3]) -> Option<trp_sim::combat::ballistics::RayHit> {
    let mut hits = Vec::new();
    range.bullet_hits(from, to, &[], &mut hits);
    if let Some(dummies) = dummies {
        dummies.ray(from, to, &mut hits);
    }
    hits.into_iter().min_by(|a, b| a.distance.total_cmp(&b.distance))
}

/// Each laser shown this frame (`LaserBeam.LateUpdate`): one ray from 0.1 m
/// ahead of its object, 100 m along its way; with a hit at d (k = d/100),
/// the beam d + 0.1 long at a brightness of 1 − k, the dot on the surface
/// sized from 0.017 to 0.2 by k, and its small light 0.1 m off the surface
/// toward the normal less the beam, a quarter of the way turned from
/// looking at the dot to the beam's own turn, its brightness 3.1(1 − k), its
/// cone from 15° to 300° by k (the engine stops at 179°); with none, the
/// beam 100 m long at its full brightness, no dot, its light dark. Each
/// camera that draws the effects is given them (`muzzle_view` draws them);
/// an infrared one shows nothing without night vision (`_Factor` 0).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn aim_lasers(
    mut commands: Commands,
    mut state: ResMut<Tactical>,
    rig: Option<Res<WeaponRig>>,
    carried: Res<Carried>,
    held: Res<HandsHeld>,
    range: Res<crate::world::range::Range>,
    dummies: Option<Res<crate::world::dummy::Dummies>>,
    time: Res<Time>,
    mut images: ResMut<Assets<Image>>,
    nodes: Query<(&Transform, Option<&ChildOf>), Without<TacticalLamp>>,
    globals: Query<&GlobalTransform, Without<TacticalLamp>>,
    mut cameras: Query<(Entity, &Camera, &GlobalTransform, &Projection, Option<&mut LaserScreen>), (With<crate::weapons::muzzle_view::JetScreen>, Without<TacticalLamp>)>,
    main: Query<(&GlobalTransform, &Projection), (With<crate::MainCamera>, Without<TacticalLamp>)>,
    mut lamps: Query<(&mut Transform, &mut GlobalTransform, &mut SpotLight, &mut Visibility), With<TacticalLamp>>,
    (slung, controller, heads): (Option<Res<crate::player::slung_view::Slung>>, Res<Controller>, Option<Res<crate::player::body_view::HeadModels>>),
) {
    let state = &mut *state;
    if !state.on {
        return;
    }
    let outside = controller.movement.body.point_of_view() == trp_sim::PointOfView::ThirdPerson;
    // The lasers shown: which, where it starts and its turn (the game's
    // axes), and its figures.
    let mut seen: Vec<(LaserKey, [f32; 3], [f32; 4], DeviceLaser)> = Vec::new();
    if let Some(rig) = rig.as_deref() {
        let states = weapon_states(&carried, &held, rig.devices.len());
        for (d, device) in rig.devices.iter().enumerate() {
            let now = states[d];
            for (l, laser) in device.lasers.iter().enumerate() {
                if !now.on || laser.mode != Some(now.mode as usize) || laser.infrared {
                    continue;
                }
                let Some(&node) = rig.bones.get(laser.node) else { continue };
                let Ok(global) = globals.get(node) else { continue };
                let at = global.translation();
                seen.push((LaserKey::Weapon(d, l), [at.x, at.y, -at.z], crate::platform::convert::rotation_back(kept_turn(node, &nodes)), laser.clone()));
            }
        }
    }
    // The weapons hung on the body: a laser is drawn, not a renderer of the
    // model (`LaserBeam.LateUpdate`: `Graphics.DrawMesh`), so it stays on.
    for hung in slung.iter().flat_map(|slung| slung.with_devices()) {
        let states = states_of(hung.item_in(&carried), hung.devices.len());
        for (d, device) in hung.devices.iter().enumerate() {
            let now = states[d];
            for (l, laser) in device.lasers.iter().enumerate() {
                if !now.on || laser.mode != Some(now.mode as usize) || laser.infrared {
                    continue;
                }
                let Some(&node) = hung.objects.get(laser.node) else { continue };
                let Ok(global) = globals.get(node) else { continue };
                let at = global.translation();
                seen.push((LaserKey::Slung(hung.item, d, l), [at.x, at.y, -at.z], crate::platform::convert::rotation_back(kept_turn(node, &nodes)), laser.clone()));
            }
        }
    }
    if let Ok((camera, projection)) = main.single() {
        let fov = match projection {
            Projection::Perspective(p) => p.fov.to_degrees(),
            _ => 75.0,
        };
        let at = camera.translation();
        let camera_turn = crate::platform::convert::rotation_back(camera.to_scale_rotation_translation().1);
        // By the camera the head's lights went by this frame (`place_lamps`).
        let (head_at, head_turn, head_fov) = state.head_camera_used.unwrap_or(([at.x, at.y, -at.z], camera_turn, fov));
        for (id, template) in state.head_found.clone() {
            let Some(Some(device)) = state.head_data.get(&template).cloned() else { continue };
            let now = state.head_shown.get(&id).copied().unwrap_or_default();
            let side = state.head_side.get(&id).copied().unwrap_or(false);
            for (l, laser) in device.lasers.iter().enumerate() {
                if !now.on || laser.mode != Some(now.mode as usize) || laser.infrared {
                    continue;
                }
                let key = LaserKey::Head(id, l);
                // From outside, on its object (as `place_lamps` puts the
                // lights).
                let object = outside.then(|| heads.as_deref()?.device_on_body(&template)?.object(laser.node)).flatten().map(|(entity, _)| entity);
                if let Some((entity, global)) = object.and_then(|entity| globals.get(entity).ok().map(|global| (entity, global))) {
                    let at = global.translation();
                    seen.push((key, [at.x, at.y, -at.z], crate::platform::convert::rotation_back(kept_turn(entity, &nodes)), laser.clone()));
                    continue;
                }
                let before = state.head_laser_turns.get(&key).copied().unwrap_or([0.0, 0.0, 0.0, 1.0]);
                let (place, turned) = head_pose(head_at, head_turn, head_fov, side, before);
                state.head_laser_turns.insert(key, turned);
                seen.push((key, place, turned, laser.clone()));
            }
        }
    }
    let t = time.elapsed_secs();
    let to_bevy = |v: [f32; 3]| Vec3::new(v[0], v[1], -v[2]);
    let mut draws: Vec<(LaserUniform, [Handle<Image>; 4])> = Vec::new();
    let mut lit = Vec::new();
    for (key, start, turned, laser) in &seen {
        let (Some(beam), Some(point)) = (laser.beam.as_ref(), laser.point.as_ref()) else { continue };
        let f = turn(*turned, [0.0, 0.0, 1.0]);
        let from: [f32; 3] = std::array::from_fn(|i| start[i] + f[i] * laser.ray_start);
        let to: [f32; 3] = std::array::from_fn(|i| from[i] + f[i] * laser.max_distance);
        let hit = first_hit(&range, dummies.as_deref(), from, to);
        let figures = tac::laser_figures(laser, hit.as_ref().map(|h| h.distance));
        let (length, intensity) = (figures.length, figures.brightness);
        let mut uniform = LaserUniform {
            origin: to_bevy(*start).extend(laser.beam_size),
            axis: to_bevy(f).extend(length),
            beam_color: Vec4::from(beam.color),
            beam: Vec4::new(intensity, beam.factor, beam.noise_scale, t / 20.0),
            clock: Vec4::new((t / 8.0).sin(), 0.0, 0.0, 0.0),
            dot_color: Vec4::from(point.color),
            dot_view: Vec4::new(point.size_view[0], point.size_view[1], 0.0, 0.0),
            ..default()
        };
        if let Some(h) = &hit {
            let n = h.normal;
            let size = figures.dot_size;
            let facing = look_rotation(n, [0.0, 1.0, 0.0]);
            uniform.dot_center = to_bevy(h.point).extend(size);
            uniform.dot_x = to_bevy(turn(facing, [1.0, 0.0, 0.0])).extend(0.0);
            uniform.dot_y = to_bevy(turn(facing, [0.0, 1.0, 0.0])).extend(0.0);
            uniform.dot = Vec4::new(intensity, point.factor, point.min_fov, point.max_dist);
            // The dot's light.
            if laser.use_point_light {
                let q = Vec3::from(tac::laser_light_at(h.point, n, f, laser.surface_offset));
                let looking = look_rotation((Vec3::from(h.point) - q).to_array(), [0.0, 1.0, 0.0]);
                let way = nlerp(looking, *turned, 0.25);
                let (angle, light_intensity) = (figures.light_angle, figures.light_intensity);
                let color: [f32; 3] = std::array::from_fn(|i| beam.color[i] * beam.factor);
                lit.push((*key, crate::platform::convert::position(q.to_array()), crate::platform::convert::rotation(way), angle, light_intensity, color, laser.light_range, laser.cookie.clone()));
            }
        }
        let pictures = [picture_of(state, beam.main.as_ref(), &mut images), volume_of(state, beam.noise.as_ref(), &mut images), picture_of(state, point.main.as_ref(), &mut images), picture_of(state, point.noise.as_ref(), &mut images)];
        let [Some(a), Some(b), Some(c), Some(d)] = pictures else { continue };
        draws.push((uniform, [a, b, c, d]));
    }

    // The dots' lights: made once each, placed and lit; the others dark.
    let pi = std::f32::consts::PI;
    let mut kept = Vec::new();
    for (key, at, way, angle, intensity, color, reach, cookie_picture) in lit {
        let entity = match state.dot_lights.get(&key) {
            Some(&entity) => entity,
            None => {
                let mut entity = commands.spawn((SpotLight { shadows_enabled: false, radius: 0.0, ..default() }, Transform::default(), Visibility::Hidden, NotShadowCaster, TacticalLamp, Name::new("laser dot light")));
                if let Some(image) = cookie_picture.as_ref().and_then(|texture| picture_of(state, Some(texture), &mut images)) {
                    entity.insert(SpotLightTexture { image });
                }
                let id = entity.id();
                state.dot_lights.insert(key, id);
                continue;
            }
        };
        kept.push(entity);
        if let Ok((mut transform, mut global, mut spot, mut visibility)) = lamps.get_mut(entity) {
            *transform = Transform { translation: at, rotation: way, scale: Vec3::ONE };
            *global = GlobalTransform::from(*transform);
            let (color, intensity) = if crate::rendering::shading::on() {
                (Color::linear_rgb(color[0], color[1], color[2]), 4.0 * pi * intensity)
            } else {
                let l = crate::rendering::lighting::linear;
                (Color::linear_rgb(l(color[0]), l(color[1]), l(color[2])), 4.0 * pi * pi * crate::rendering::lighting::EXPOSURE_SCALE * reach * reach * l(intensity) * state.exposure_gain)
            };
            (spot.color, spot.intensity, spot.range, spot.outer_angle, spot.inner_angle) = (color, intensity, reach, (angle / 2.0).to_radians(), 0.0);
            let want = if intensity > 0.0 { Visibility::Inherited } else { Visibility::Hidden };
            if *visibility != want {
                *visibility = want;
            }
        }
    }
    for (_, &entity) in state.dot_lights.iter() {
        if !kept.contains(&entity) {
            if let Ok((.., mut visibility)) = lamps.get_mut(entity) {
                if *visibility != Visibility::Hidden {
                    *visibility = Visibility::Hidden;
                }
            }
        }
    }
    // Each camera that draws the effects: the lasers by its own eye.
    for (entity, camera, eye, projection, screen) in &mut cameras {
        let list: Vec<LaserDraw> = if camera.is_active {
            let view_from_world = eye.to_matrix().inverse();
            let clip_from_view = projection.get_clip_from_view();
            let fov = match projection {
                Projection::Perspective(p) => p.fov.to_degrees(),
                _ => 75.0,
            };
            let camera_at = eye.translation().extend(fov);
            draws
                .iter()
                .map(|(u, [main, noise, dot_main, dot_noise])| LaserDraw {
                    uniform: LaserUniform { view_from_world, clip_from_view, camera: camera_at, ..*u },
                    main: main.clone(),
                    noise: noise.clone(),
                    dot_main: dot_main.clone(),
                    dot_noise: dot_noise.clone(),
                })
                .collect()
        } else {
            Vec::new()
        };
        match screen {
            Some(mut screen) => {
                if !(screen.0.is_empty() && list.is_empty()) {
                    screen.0 = list;
                }
            }
            None => {
                commands.entity(entity).insert(LaserScreen(list));
            }
        }
    }
}
