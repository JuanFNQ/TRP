//! The hazes' uniform, batches and systems, prepared and drawn each frame.

use super::*;

/// What the haze's shader is given (`Particles/DistortAndBlur`'s figures
/// and the camera's).
#[derive(Clone, Copy, Default, ShaderType)]
pub struct HazeUniform {
    pub view_from_world: Mat4,
    pub clip_from_view: Mat4,
    /// `_Time.x`, `_Distort`, `_Blur`, `_InvFade` × 10.
    pub figures: Vec4,
    /// `_TextureMovement`.
    pub movement: Vec4,
    /// `_DistortTex_ST`.
    pub scale: Vec4,
    /// The camera's near plane, the most size on the screen, the
    /// projection's width scale (the most a share of the view's width:
    /// `explosion_view::held_size`).
    pub near: Vec4,
}

/// One system's haze to draw: its figures, its pictures, and each puff's
/// corners (as the fumes': eleven numbers each).
#[derive(Clone)]
pub struct HazeBatch {
    pub uniform: HazeUniform,
    pub noise: Handle<Image>,
    pub alpha: Handle<Image>,
    pub corners: Arc<Vec<f32>>,
}

/// The haze to draw this frame: each shared system's that shows (the
/// muzzles', then the hot parts'), drawn into the same values, and the
/// camera's distortion (its pixels, its blur's iterations).
#[derive(Clone)]
pub struct HazeDraw {
    pub batches: Vec<HazeBatch>,
    pub distortion: Vec2,
}

/// On the camera: the haze while any shows.
#[derive(Component, Clone, Default)]
pub struct HazeScreen(pub Option<HazeDraw>);

impl ExtractComponent for HazeScreen {
    type QueryData = &'static HazeScreen;
    type QueryFilter = ();
    type Out = HazeScreen;

    fn extract_component(haze: QueryItem<'_, '_, Self::QueryData>) -> Option<Self::Out> {
        haze.0.as_ref().is_some_and(|draw| draw.batches.iter().any(|batch| !batch.corners.is_empty())).then(|| haze.clone())
    }
}

/// A shared system drawn into the camera's distortion
/// (`Particles/DistortAndBlur`): the system, its material's pictures and
/// figures.
pub struct HazeSystem {
    pub(super) system: ParticleSystem,
    pub(super) noise: Handle<Image>,
    alpha: Handle<Image>,
    pub(super) uniform: HazeUniform,
}

impl HazeSystem {
    /// Made from its data; none without its material's two pictures
    /// (`what` names it in the warning).
    fn of(data: trp_data::drawing::particles::ParticleSystemData, what: &str, images: &mut impl crate::assets::background::AddImage) -> Option<Self> {
        for module in &data.unbuilt {
            warn!("{}: its {module} is on and not built: left out", data.name);
        }
        let m = &data.renderer.material;
        let (Some(noise), Some(alpha)) = (m.texture("_DistortTex").and_then(|t| picture(t, images)), m.texture("_AlphaTex").and_then(|t| picture(t, images))) else {
            warn!("{what} is not drawn: its material {} has not its two pictures", m.name);
            return None;
        };
        let uniform = HazeUniform {
            figures: Vec4::new(0.0, m.float("_Distort").unwrap_or(0.0), m.float("_Blur").unwrap_or(0.0), m.float("_InvFade").unwrap_or(1.0) * 10.0),
            movement: Vec4::from(m.color("_TextureMovement").unwrap_or([0.0; 4])),
            scale: Vec4::from(m.scale("_DistortTex").unwrap_or([1.0, 1.0, 0.0, 0.0])),
            near: Vec4::new(0.0, data.renderer.max_size, 1.0, 0.0),
            ..default()
        };
        Some(Self { system: ParticleSystem::new(Arc::new(data)), noise, alpha, uniform })
    }

    /// Its puffs' corners, while it has any.
    pub(super) fn corners(&self) -> Option<Arc<Vec<f32>>> {
        const CORNERS: [[f32; 2]; 4] = [[-0.5, -0.5], [0.5, -0.5], [0.5, 0.5], [-0.5, 0.5]];
        (!self.system.particles.is_empty()).then(|| {
            Arc::new(
                self.system
                    .drawn()
                    .flat_map(|p| {
                        let at = crate::platform::convert::position(p.position);
                        CORNERS.into_iter().flat_map(move |c| [at.x, at.y, at.z, c[0], c[1], p.size, p.rotation.to_radians(), p.color[0], p.color[1], p.color[2], p.color[3]])
                    })
                    .collect(),
            )
        })
    }
}

/// The shared haze systems (the muzzles', `HeatHaze`, and where it was
/// read the hot parts' shimmer's, `Heat`), the engine's draws, the shots
/// seen, the puffs due (`WaitForSeconds(DelayInSec)`: when, from which
/// object, by which of the muzzle's), the camera's distortion, and the
/// frames gone by.
#[derive(Resource)]
pub struct Hazes {
    muzzles: HazeSystem,
    barrels: Option<HazeSystem>,
    pub(super) rng: Rng,
    pub(super) shots_seen: [u32; 2],
    due: Vec<(f32, Entity, Arc<trp_data::weapons::muzzle::MuzzleData>, usize)>,
    distortion: Vec2,
    frame: u64,
}

/// The shared haze systems and the camera's distortion read, at the start;
/// none where the hard settings have no heat emitters
/// (`HEAT_EMITTER_ENABLED`, which the hot parts' shimmer goes by too:
/// `WeaponOverHeatSystem.LateUpdate`).
pub(crate) fn prepare_hazes(unity: &mut trp_data::unity::Assets, hard: &trp_data::setup::hard_settings::HardSettings, images: &mut impl crate::assets::background::AddImage) -> Option<Hazes> {
    if hard.float("HEAT_EMITTER_ENABLED").is_ok_and(|on| on == 0.0) {
        info!("the muzzles' heat haze and the hot parts' shimmer: off in the hard settings");
        return None;
    }
    let data = match trp_data::drawing::particles::muzzle_heat_haze_system(unity) {
        Ok(data) => data,
        Err(e) => {
            warn!("the muzzles' heat haze is not drawn: its system could not be read: {e:#}");
            return None;
        }
    };
    let distortion = match trp_data::drawing::camera_effects::read_distortion(&unity.install) {
        Ok(d) => d,
        Err(e) => {
            warn!("the muzzles' heat haze is not drawn: the camera's distortion could not be read: {e:#}");
            return None;
        }
    };
    info!(
        "the muzzles' heat haze: {} ({}, at most {}; on the screen at most {} of the view's width), the camera's distortion {} pixels, {} blur",
        data.name, data.renderer.material.shader, data.max_particles, data.renderer.max_size, distortion.pixels, distortion.blur_iterations
    );
    let muzzles = HazeSystem::of(data, "the muzzles' heat haze", images)?;
    let barrels = match trp_data::drawing::particles::muzzle_heat_system(unity) {
        Ok(data) => {
            info!("the hot parts' shimmer: {} ({}, at most {}; on the screen at most {} of the view's width)", data.name, data.renderer.material.shader, data.max_particles, data.renderer.max_size);
            HazeSystem::of(data, "the hot parts' shimmer", images)
        }
        Err(e) => {
            warn!("the hot parts' shimmer is not drawn: its system could not be read: {e:#}");
            None
        }
    };
    Some(Hazes { muzzles, barrels, rng: Rng::new(0x4a2e), shots_seen: [0; 2], due: Vec::new(), distortion: Vec2::new(distortion.pixels, distortion.blur_iterations as f32), frame: 0 })
}

/// A frame, once everything is placed: each shot's haze that its chance
/// gives queued for its delay; the puffs due emitted from where their
/// object is now; each hot part's shimmer given the weapon's heat and
/// emitted (`trp_sim::muzzle::BarrelHaze`); the systems stepped; what they
/// draw handed to each camera that draws it. Only the main camera has its
/// screen: the game's haze is drawn by that camera's distortion alone
/// (`DistortCameraFX`, which the optic camera has not; its renderer's own
/// material draws nothing).
#[allow(clippy::too_many_arguments)]
pub fn haze_frame(
    rig: Option<Res<crate::weapons::weapon::WeaponRig>>,
    controller: Res<crate::player::controller::Controller>,
    time: Res<Time>,
    mut hazes: ResMut<Hazes>,
    places: Query<&GlobalTransform>,
    parents: Query<&ChildOf>,
    mut cameras: Query<(&Camera, &GlobalTransform, &Projection, &mut HazeScreen)>,
    data: Option<Res<crate::GameData>>,
    mut shimmers: Query<(&GlobalTransform, &mut crate::weapons::heat_view::HotShimmer)>,
    owners: Query<(&crate::weapons::heat_view::HeatOwner, &GlobalTransform, &Visibility)>,
) {
    let hazes = &mut *hazes;
    let now = time.elapsed_secs();
    let dt = time.delta_secs();
    hazes.frame += 1;
    for (parts, shots) in crate::weapons::muzzle_view::fired_muzzles(rig.as_deref(), &controller, &mut hazes.shots_seen) {
        for _ in 0..shots.min(8) {
            for (index, entity) in parts.hazes.iter().enumerate() {
                let Some(entity) = *entity else { continue };
                let haze = &parts.data.hazes[index];
                if trp_sim::weapons::muzzle::haze_shot(haze, &mut hazes.rng) {
                    hazes.due.push((now + haze.delay, entity, parts.data.clone(), index));
                }
            }
        }
    }
    let game = |v: Vec3| [v.x, v.y, -v.z];
    let mut waiting = Vec::new();
    for (when, entity, data, index) in std::mem::take(&mut hazes.due) {
        if when > now {
            waiting.push((when, entity, data, index));
            continue;
        }
        // Its object gone (the weapon put away), nothing.
        let Ok(place) = places.get(entity) else { continue };
        let parent = parents.get(entity).ok().and_then(|child| places.get(child.parent()).ok());
        let from_parent = parent.map_or([0.0; 3], |p| {
            let (a, b) = (game(place.translation()), game(p.translation()));
            std::array::from_fn(|k| a[k] - b[k])
        });
        for puff in trp_sim::weapons::muzzle::haze_puffs(&data.hazes[index], game(place.translation()), from_parent, &mut hazes.rng) {
            hazes.muzzles.system.emit(puff);
        }
    }
    hazes.due = waiting;

    // The hot parts' shimmer (`WeaponOverHeatSystem`): each part given its
    // model's heat (`heat_view::HeatOwner`: the rig in hand's, a weapon's
    // on the ground or slung, any other rig's none).
    if let Some(barrels) = hazes.barrels.as_mut() {
        let eye = cameras.iter().find(|(camera, ..)| camera.is_active).map(|(_, eye, ..)| eye.translation());
        let frame = hazes.frame;
        for (place, mut shimmer) in &mut shimmers {
            let shimmer = &mut *shimmer;
            let at = game(place.translation());
            let state = shimmer.state.get_or_insert_with(|| trp_sim::weapons::muzzle::BarrelHaze::woken(now, &shimmer.haze));
            // Given the heat once it has moved a hundredth
            // (`WeaponPrefab.ManualUpdate`), its place kept then if not for
            // thirty frames (`PrepareForEffects`).
            let owner = owners.get(shimmer.owner).ok();
            let own = owner.map_or(0.0, |(owner, ..)| owner.given().max(0.0));
            if (own - shimmer.given).abs() >= 0.01 {
                shimmer.given = own;
                state.give(&shimmer.haze, own);
                if frame.saturating_sub(shimmer.synced) > 30 {
                    shimmer.previous = Some(at);
                    shimmer.synced = frame;
                }
            }
            // Within reach of the camera, its puffs and its place kept
            // (`ManualSyncEffects`, `SyncPosition`): the reach from its
            // model's own object (the game's from its `WeaponPrefab`: a
            // model's first object; for the rig in hand its `Weapon_root`,
            // not the hands' prefab's object, `PROVISIONAL`). One's own
            // slung weapon's only while it is shown (`HeatOwner::seen_only`).
            let from = owner.map_or(place.translation(), |(_, at, _)| at.translation());
            let unseen = owner.is_some_and(|(owner, _, shown)| owner.seen_only && *shown == Visibility::Hidden);
            if unseen || eye.is_none_or(|eye| eye.distance(from) > trp_sim::weapons::muzzle::BARREL_HAZE_REACH) {
                continue;
            }
            let velocity = shimmer.previous.map_or([0.0; 3], |p| std::array::from_fn(|k| (at[k] - p[k]) / dt.max(1e-6)));
            let room = (barrels.system.data.max_particles as usize).saturating_sub(barrels.system.particles.len());
            let to_world = |p: [f32; 3]| game(place.transform_point(crate::platform::convert::position(p)));
            for puff in state.puffs(&shimmer.haze, now, to_world, velocity, room, &mut hazes.rng) {
                barrels.system.emit_given(puff.position, puff.velocity, puff.size, puff.lifetime);
            }
            shimmer.previous = Some(at);
            shimmer.synced = frame;
        }
    }

    let most = data.as_ref().map_or(f32::MAX, |g| g.engine.maximum_particle_timestep);
    hazes.muzzles.system.step_frame(dt, most, None);
    if let Some(barrels) = hazes.barrels.as_mut() {
        barrels.system.step_frame(dt, most, None);
    }

    let shown: Vec<(&HazeSystem, Arc<Vec<f32>>)> = [Some(&hazes.muzzles), hazes.barrels.as_ref()].into_iter().flatten().filter_map(|system| system.corners().map(|corners| (system, corners))).collect();
    for (camera, eye, projection, mut screen) in &mut cameras {
        if shown.is_empty() || !camera.is_active {
            if screen.0.is_some() {
                screen.0 = None;
            }
            continue;
        }
        let clip_from_view = camera.clip_from_view();
        let batches = shown
            .iter()
            .map(|(system, corners)| {
                let mut uniform = system.uniform;
                uniform.view_from_world = eye.to_matrix().inverse();
                uniform.clip_from_view = clip_from_view;
                uniform.figures.x = now / 20.0;
                uniform.near = Vec4::new(crate::weapons::muzzle_view::near_of(projection), uniform.near.y, clip_from_view.x_axis.x, 0.0);
                HazeBatch { uniform, noise: system.noise.clone(), alpha: system.alpha.clone(), corners: corners.clone() }
            })
            .collect();
        screen.0 = Some(HazeDraw { batches, distortion: hazes.distortion });
    }
}
