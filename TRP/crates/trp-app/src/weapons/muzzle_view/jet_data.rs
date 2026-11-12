//! The smoke and jet uniforms, meshes and screens, made each frame.

use super::*;

/// What the smoke's shader is given for one trail (`Custom/MuzzleSmoke`'s
/// figures and the camera's).
#[derive(Clone, Copy, Default, ShaderType)]
pub struct SmokeUniform {
    pub view_from_world: Mat4,
    pub clip_from_view: Mat4,
    /// `_Width`, `_UVYScale`, `_DiffusionStrength`, `_StartFade`.
    pub a: Vec4,
    /// `_EndFade`, `_End`, `_InvFade`, `_Alpha`.
    pub b: Vec4,
    /// The camera's near plane.
    pub near: Vec4,
}

/// One trail to draw: its figures, its picture and its corners (place,
/// side, number, diffusion, direction, alpha: ten numbers each).
#[derive(Clone)]
pub struct SmokeDraw {
    pub uniform: SmokeUniform,
    pub picture: Handle<Image>,
    pub corners: Arc<Vec<f32>>,
}

/// On the camera while any trail shows: the trails.
#[derive(Component, Clone, Default)]
pub struct SmokeScreen(pub Vec<SmokeDraw>);

impl ExtractComponent for SmokeScreen {
    type QueryData = &'static SmokeScreen;
    type QueryFilter = ();
    type Out = SmokeScreen;

    /// Only while a trail shows.
    fn extract_component(smoke: QueryItem<'_, '_, Self::QueryData>) -> Option<Self::Out> {
        (!smoke.0.is_empty()).then(|| smoke.clone())
    }
}

/// What the jets' shader is given this frame (its figures, `Custom/
/// Additive`'s, and where the mesh and the camera stand).
#[derive(Component, Clone, Copy, Default, ShaderType)]
pub struct JetUniform {
    pub clip_from_world: Mat4,
    pub world_from_local: Mat4,
    pub projection: Vec4,
    pub tint: Vec4,
    pub rot_mx: Vec4,
    pub random_vals: Vec4,
    /// `_CellShift`, `_InvWindowSize`, `_RandomizePhase`.
    pub cell: Vec4,
    /// `_ShotVals`.
    pub shot: Vec4,
    /// `_ZShift`, `_HDR`.
    pub z_hdr: Vec4,
}

/// Which rig's jets the camera draws this frame, by the entity they hang
/// from.
#[derive(Component, Clone, Copy)]
pub struct JetRig(pub Entity);

/// On the camera while a flash shows: its figures and its rig.
#[derive(Component, Clone, Copy, Default)]
pub struct JetScreen {
    pub uniform: JetUniform,
    pub rig: Option<Entity>,
}

impl ExtractComponent for JetScreen {
    type QueryData = &'static JetScreen;
    type QueryFilter = ();
    type Out = (JetUniform, JetRig);

    /// Only while a flash shows.
    fn extract_component(jets: QueryItem<'_, '_, Self::QueryData>) -> Option<Self::Out> {
        jets.rig.map(|rig| (jets.uniform, JetRig(rig)))
    }
}

/// Each rig's jets: their corners and indices, and their picture.
#[derive(Clone)]
pub struct JetMesh {
    pub vertices: Arc<Vec<JetVertex>>,
    pub indices: Arc<Vec<u32>>,
    pub picture: Handle<Image>,
}

/// A picture of the muzzle's, read as stored (the game's gamma-space
/// project), its cells tiled.
pub(super) fn tiled(texture: &trp_data::unity::extract::TextureData, images: &mut Assets<Image>) -> Option<Handle<Image>> {
    let raw = trp_data::unity::extract::TextureData { srgb: false, ..texture.clone() };
    let mut image = crate::assets::render_data::build_image(&raw, crate::assets::render_data::Role::Color).map_err(|e| warn!("a picture of the muzzle's: {e:#}")).ok()?;
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Linear,
        ..default()
    });
    Some(images.add(image))
}

/// The rigs' jets, by the entity each hangs from.
#[derive(Resource, Clone, Default, ExtractResource)]
pub struct JetMeshes(pub HashMap<Entity, JetMesh>);

/// A rig's flash: its figures, its shot's time and its draws; its gun
/// smokes and their pictures.
pub(super) struct Flash {
    pub(super) data: Arc<MuzzleData>,
    pub(super) flash: MuzzleFlash,
    pub(super) figures: JetUniform,
    pub(super) smokes: Vec<Smoke>,
    pub(super) smoke_pictures: Vec<Option<Handle<Image>>>,
    /// The time the trails have still to age by, and where each trail's
    /// object was at the last frame (`smoke_frame`).
    pub(super) smoke_clock: f32,
    pub(super) smoke_last: Vec<Option<[f32; 3]>>,
    /// Its light (`MuzzleLight`): the reach drawn at the last shot, and the
    /// engine's light once made, beside the fireport.
    pub(super) light: MuzzleLight,
    pub(super) lamp: Option<Entity>,
}

/// Each rig's flash, by the entity its mesh hangs from; the draws; the
/// shots counted so far; the rig in hand when last seen; the lights' share
/// for the camera's exposure (`tactical_view::exposure_gain`).
#[derive(Resource)]
pub struct MuzzleViews {
    pub(super) flashes: HashMap<Entity, Flash>,
    pub(super) rng: Rng,
    /// The weapon's shots and its launcher's seen so far (`fired_muzzles`).
    pub(super) shots_seen: [u32; 2],
    /// The rig in hand's muzzles, by the entities their meshes hang from.
    pub(super) in_hand: Vec<Entity>,
    pub(super) exposure_gain: f32,
}

impl Default for MuzzleViews {
    fn default() -> Self {
        Self { flashes: HashMap::new(), rng: Rng::new(0x4d55_5a5a), shots_seen: [0; 2], in_hand: Vec::new(), exposure_gain: 1.0 }
    }
}

/// The rig in hand's muzzles: the weapon's own, then its launcher's.
fn muzzles_of(rig: Option<&crate::weapons::weapon::WeaponRig>) -> Vec<&crate::weapons::weapon::MuzzleParts> {
    rig.map_or(Vec::new(), |rig| [rig.muzzle.as_ref(), rig.launcher_muzzle.as_ref()].into_iter().flatten().collect())
}

/// The muzzle's light as the engine draws it.
#[derive(Component)]
pub struct MuzzleLamp;

/// The most steps a trail ages by in one drawn frame; what a longer frame
/// leaves over is let go, as the game's single long frame would take it.
const MOST_SMOKE_STEPS: u32 = 8;

/// The gun smokes' frame (`LateUpdateMuzzleEffectsValues`, given the main
/// camera), once everything is placed: each trail of the rig in hand worked
/// from its object's place and the main camera's forward, and the trails
/// handed to each camera that draws them. Only the main camera has their
/// screen: the game's trail draws itself for it alone
/// (`MuzzleSmoke.OnRenderObject`), not for the optic camera.
///
/// The game works a trail once a frame, and its drag and spread go by the
/// frame, not the time: its life grows with the frame's time cubed (the
/// AKM's at 60 frames a second 1.3 s, at 120 4.2 s, at 300 a minute). So it
/// ages here in steps at the game's own frame rate setting (the options'
/// `GameFramerate`, the install's 120), whatever the app draws at, its
/// object's place eased across the frame for the steps in it, and is drawn
/// every frame as it last stood (TRP's choice, at the game's frame rate
/// setting; firing spec, "The gun smoke").
#[allow(clippy::too_many_arguments)]
pub fn smoke_frame(
    rig: Option<Res<crate::weapons::weapon::WeaponRig>>,
    time: Res<Time>,
    options: Res<crate::screens::options_view::OptionsState>,
    mut views: ResMut<MuzzleViews>,
    places: Query<&GlobalTransform>,
    main: Query<&GlobalTransform, With<crate::MainCamera>>,
    mut cameras: Query<(&Camera, &GlobalTransform, &Projection, &mut SmokeScreen)>,
) {
    let Ok(main) = main.single() else { return };
    let mut trails: Vec<Trail> = Vec::new();
    for parts in muzzles_of(rig.as_deref()) {
        let Some(flash) = views.flashes.get_mut(&parts.parent) else { continue };
        let game = |v: Vec3| [v.x, v.y, -v.z];
        let forward = game(main.forward().as_vec3());
        let [low, high] = crate::screens::options_view::GAME_FPS;
        let step = 1.0 / options.graphics.game_fps.clamp(low, high) as f32;
        flash.smoke_clock += time.delta_secs();
        let steps = ((flash.smoke_clock / step) as u32).min(MOST_SMOKE_STEPS);
        flash.smoke_clock = if steps == MOST_SMOKE_STEPS { 0.0 } else { flash.smoke_clock - steps as f32 * step };
        for (i, (smoke, data)) in flash.smokes.iter_mut().zip(&flash.data.smokes).enumerate() {
            let Some(place) = parts.smokes.get(i).and_then(|entity| places.get(*entity).ok()) else { continue };
            let now = game(place.translation());
            let last = flash.smoke_last.get(i).copied().flatten().unwrap_or(now);
            let was_on = smoke.on();
            for k in 1..=steps {
                let share = k as f32 / steps as f32;
                let at: [f32; 3] = std::array::from_fn(|c| last[c] + (now[c] - last[c]) * share);
                smoke.frame(data, step, at, forward);
            }
            if was_on && !smoke.on() {
                debug!("a gun smoke's trail ended at {:.2} s, aged in steps of 1/{:.0} s", time.elapsed_secs(), 1.0 / step);
            }
            if let Some(slot) = flash.smoke_last.get_mut(i) {
                *slot = Some(now);
            }
            let corners = smoke.corners();
            let Some(picture) = flash.smoke_pictures.get(i).cloned().flatten() else { continue };
            if corners.is_empty() {
                continue;
            }
            let numbers: Vec<f32> = corners
                .iter()
                .flat_map(|c| {
                    let p = crate::platform::convert::position(c.position);
                    let d = crate::platform::convert::position(c.direction);
                    [p.x, p.y, p.z, c.side, c.number, c.diffusion, d.x, d.y, d.z, c.alpha]
                })
                .collect();
            let m = &data.material;
            trails.push(Trail { corners: Arc::new(numbers), picture, a: Vec4::new(m.width, m.uv_y_scale, m.diffusion_strength, m.start_fade), b: Vec4::new(m.end_fade, m.end, m.inv_fade, m.alpha) });
        }
    }
    for (camera, eye, projection, mut screen) in &mut cameras {
        if trails.is_empty() || !camera.is_active {
            if !screen.0.is_empty() {
                screen.0.clear();
            }
            continue;
        }
        let (view_from_world, clip_from_view, near) = (eye.to_matrix().inverse(), camera.clip_from_view(), near_of(projection));
        screen.0 = trails
            .iter()
            .map(|t| SmokeDraw { uniform: SmokeUniform { view_from_world, clip_from_view, a: t.a, b: t.b, near: Vec4::new(near, 0.0, 0.0, 0.0) }, picture: t.picture.clone(), corners: t.corners.clone() })
            .collect();
    }
}

/// A trail worked out this frame, whichever camera draws it: its corners,
/// its picture, its material's figures (`SmokeUniform`'s `a` and `b`).
pub(super) struct Trail {
    pub(super) corners: Arc<Vec<f32>>,
    pub(super) picture: Handle<Image>,
    pub(super) a: Vec4,
    b: Vec4,
}

/// Each camera's jets this frame, once everything is placed: the shot's
/// figures now, the mesh's and the camera's places; none while nothing
/// shows (`_ShotVals.x` 1: every pixel cut) or the camera is off. The jets'
/// mesh is an object of the world's layer (`MuzzleJet`'s, made on layer
/// 0), so the optic camera draws them too.
pub fn jet_screen(
    rig: Option<Res<crate::weapons::weapon::WeaponRig>>,
    time: Res<Time>,
    views: Res<MuzzleViews>,
    places: Query<&GlobalTransform>,
    mut cameras: Query<(&Camera, &GlobalTransform, &mut JetScreen)>,
) {
    // The first muzzle whose jets show (`PROVISIONAL`: one muzzle's jets
    // drawn at a time; a weapon's and its launcher's never show together
    // but within a switch's time).
    let shown = muzzles_of(rig.as_deref()).into_iter().filter(|parts| !parts.vertices.is_empty() && !parts.indices.is_empty()).find_map(|parts| {
        let flash = views.flashes.get(&parts.parent)?;
        let shot = flash.flash.shot_vals(&flash.data, time.elapsed_secs());
        (shot[0] < 1.0).then_some((parts.parent, flash, shot))
    });
    for (camera, place, mut screen) in &mut cameras {
        let Some((parent, flash, shot)) = shown.filter(|_| camera.is_active) else {
            if screen.rig.is_some() {
                screen.rig = None;
            }
            continue;
        };
        let Ok(mesh_place) = places.get(parent) else { continue };
        let clip_from_view = camera.clip_from_view();
        *screen = JetScreen {
            uniform: JetUniform {
                clip_from_world: clip_from_view * place.to_matrix().inverse(),
                world_from_local: mesh_place.to_matrix(),
                projection: Vec4::new(clip_from_view.x_axis.x, clip_from_view.y_axis.y, 0.0, 0.0),
                shot: Vec4::new(shot[0], shot[1], 0.0, 0.0),
                ..flash.figures
            },
            rig: Some(parent),
        };
    }
}
