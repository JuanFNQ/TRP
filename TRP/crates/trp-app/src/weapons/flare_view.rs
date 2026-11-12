//! The tactical lights' lens flares.
//! After the tactical-devices spec, "The light shaft and the flare"; under the options' `TACTICAL
//! DEVICES`: the glare a light throws into the camera as it faces it. The game keeps them for every
//! light of the scene, and so here: the dummies' weapons' lights' (`dummy::DummyLights`, the debug
//! panel's `LIGHTS`), and one's own, the weapon in hand's, those of the weapons hung on the body
//! and, seen from outside, the head's on the model of what is worn. In first person one's own never
//! shows (the camera is behind it: its alpha sinks below nothing); seen from outside it does, as
//! another player's would. (The head's flares stay on their model in the game, which moves only its
//! own player's lights and lasers to the camera, `HeadLightsValidator`: facing away from the eyes,
//! below nothing, and not gathered in first person.)
//!
//! Each frame, once everything is placed: each lit light's alpha by how
//! squarely it faces the main camera (`trp_sim::flare::step_alpha`), each of
//! its flares' figures (`gpu_data`), the flashlights' crowding (`overlap`),
//! the eyes' protection from what the head wears (`Protection`), and their
//! corners handed to each camera that draws the world (the main one, and
//! the optic's while on, as the game registers it), which `muzzle_view`'s
//! pass adds to the frame by `flare.wgsl` (the game's `Custom/multiFlare`
//! and `Custom/ShitOnScreen`). Their two pictures are read on a worker of
//! their own as the run begins.
//!
//! `PROVISIONAL`: each light's place and way as this frame leaves them (the
//! game moves the alpha in its `Update` and reads the normal and noise
//! flares' places then, before the frame's animation); drawn after the
//! tonemapping, on the frame encoded for the camera's effects, as the
//! muzzle flash is (`muzzle_view`), where the game adds them before its
//! bloom and tonemapping.

use std::collections::HashMap;
use std::sync::Arc;

use bevy::ecs::query::QueryItem;
use bevy::prelude::*;
use bevy::render::extract_component::{ExtractComponent, ExtractComponentPlugin};
use bevy::render::render_resource::ShaderType;
use bevy::tasks::{AsyncComputeTaskPool, Task};
use trp_data::weapons::tactical::DeviceFlare;
use trp_sim::effects::flare::{self, FlareGpu, Protection, ShaderFigures};

/// The flares' systems; `on` is the options' `TACTICAL DEVICES` as the run
/// began. Off, nothing of them is read or drawn.
pub struct FlareViewPlugin {
    pub on: bool,
}

impl Plugin for FlareViewPlugin {
    fn build(&self, app: &mut App) {
        if !self.on {
            return;
        }
        app.add_plugins(ExtractComponentPlugin::<FlareScreen>::default()).add_systems(Startup, start_reading).add_systems(First, receive_pictures).add_systems(
            PostUpdate,
            flare_frame.run_if(resource_exists::<FlarePictures>).after(bevy::transform::TransformSystems::Propagate).after(bevy::camera::CameraUpdateSystems),
        );
    }
}

/// What the flares' shader is given: the camera's matrices and place, the
/// noise's time (`_Time.x`, seconds over 20), and each shader's figures
/// (`ShaderFigures`).
#[derive(Clone, Copy, Default, ShaderType)]
pub struct FlareUniform {
    pub clip_from_world: Mat4,
    pub view_from_world: Mat4,
    pub clip_from_view: Mat4,
    pub camera: Vec4,
    /// `Custom/multiFlare`'s: depth offset, falloff shift and strength,
    /// the taps' spread; then its intensity.
    pub multi: Vec4,
    pub multi_b: Vec4,
    /// `Custom/ShitOnScreen`'s: depth offset, silhouette's offset, falloff
    /// shift and strength; then the taps' spread, its intensity, its
    /// grain's strength; then its grain's scale.
    pub noise: Vec4,
    pub noise_b: Vec4,
    pub noise_scale: Vec4,
}

/// The flares to draw on a camera this frame: the figures, the two
/// pictures, and six corners a flare (`CORNER_NUMBERS` numbers each).
#[derive(Clone)]
pub struct FlareDraw {
    pub uniform: FlareUniform,
    pub atlas: Handle<Image>,
    pub noise: Handle<Image>,
    pub corners: Arc<Vec<f32>>,
    pub flares: u32,
}

/// Numbers a corner: its light's place (Bevy's axes), its corner, its
/// picture's rectangle, its size, its colour, its distance curve, its kind.
pub const CORNER_NUMBERS: usize = 20;

/// On a camera: the flares while any shows.
#[derive(Component, Clone, Default)]
pub struct FlareScreen(pub Option<FlareDraw>);

impl ExtractComponent for FlareScreen {
    type QueryData = &'static FlareScreen;
    type QueryFilter = ();
    type Out = FlareScreen;

    fn extract_component(screen: QueryItem<'_, '_, Self::QueryData>) -> Option<Self::Out> {
        screen.0.as_ref().is_some_and(|draw| draw.flares > 0).then(|| screen.clone())
    }
}

/// The flares' two pictures, as read (`trp_data::tactical::flare_pictures`).
#[derive(Resource)]
pub struct FlarePictures {
    atlas: Handle<Image>,
    noise: Handle<Image>,
}

/// The pictures being read.
#[derive(Resource)]
struct PicturesComing(Task<(Option<(Handle<Image>, Handle<Image>)>, crate::assets::background::PendingImages)>);

fn start_reading(mut commands: Commands, game: Res<crate::GameData>, images: Res<Assets<Image>>) {
    let install = game.unity.install.clone();
    let mut pending = crate::assets::background::PendingImages::new(&images);
    let task = AsyncComputeTaskPool::get().spawn(async move {
        let read = match trp_data::weapons::tactical::flare_pictures(&install) {
            Ok((atlas, noise)) => {
                info!("the lights' flares: {} ({} px) and {} ({} px)", atlas.name, atlas.width, noise.name, noise.width);
                crate::effects::particles_view::picture(&atlas, &mut pending).zip(crate::effects::particles_view::picture(&noise, &mut pending))
            }
            Err(e) => {
                warn!("no light's flare is drawn: its pictures could not be read: {e:#}");
                None
            }
        };
        (read, pending)
    });
    commands.insert_resource(PicturesComing(task));
}

fn receive_pictures(mut commands: Commands, coming: Option<ResMut<PicturesComing>>, mut images: ResMut<Assets<Image>>) {
    let Some(mut coming) = coming else { return };
    let Some((read, pending)) = crate::assets::background::take(&mut coming.0, false, "the lights' flares") else { return };
    commands.remove_resource::<PicturesComing>();
    pending.put(&mut images);
    if let Some((atlas, noise)) = read {
        commands.insert_resource(FlarePictures { atlas, noise });
    }
}

/// Each lit flare light's alpha, by its object, and the eyes' protection.
#[derive(Default)]
struct Kept {
    alphas: HashMap<Entity, f32>,
    protection: Option<Protection>,
}

/// One corner's numbers.
fn push_corners(corners: &mut Vec<f32>, g: &FlareGpu) {
    // Two triangles, as the game's strips: (−,−), (−,+), (+,+), then
    // (−,−), (+,+), (+,−).
    for corner in [[-1.0, -1.0], [-1.0, 1.0], [1.0, 1.0], [-1.0, -1.0], [1.0, 1.0], [1.0, -1.0f32]] {
        corners.extend_from_slice(&g.position);
        corners.extend_from_slice(&corner);
        corners.extend_from_slice(&g.uv);
        corners.extend_from_slice(&g.scale);
        corners.extend_from_slice(&g.color);
        corners.extend_from_slice(&[g.min_dist, g.dist_inverse, g.scale_range, g.alpha_range]);
        corners.push(g.kind as f32);
    }
}

fn figures(f: &ShaderFigures) -> (Vec4, Vec4) {
    (Vec4::new(f.depth_offset, f.fade_depth_offset, f.falloff_shift, f.falloff_strength), Vec4::new(f.checker_size, f.intensity, f.noise_intensity, 0.0))
}

/// One's own lights and what they are on: the weapon in hand's rig, the
/// weapons hung on the body, the head's devices and the body's models on
/// the head, and whose eyes the character is seen with.
#[derive(bevy::ecs::system::SystemParam)]
struct Own<'w> {
    rig: Option<Res<'w, crate::weapons::weapon::WeaponRig>>,
    held: Res<'w, crate::screens::inventory_view::HandsHeld>,
    slung: Option<Res<'w, crate::player::slung_view::Slung>>,
    tactical: Res<'w, crate::weapons::tactical_view::Tactical>,
    heads: Option<Res<'w, crate::player::body_view::HeadModels>>,
    controller: Res<'w, crate::player::controller::Controller>,
}

/// A frame of the flares, once everything is placed.
#[allow(clippy::too_many_arguments)]
fn flare_frame(
    pictures: Res<FlarePictures>,
    dummies: Option<Res<crate::world::dummy::Dummies>>,
    source: Option<Res<crate::world::dummy::DummySource>>,
    lights: Option<Res<crate::world::dummy::DummyLights>>,
    carried: Res<crate::screens::inventory_view::Carried>,
    own: Own,
    time: Res<Time>,
    nodes: Query<(&Transform, Option<&ChildOf>)>,
    globals: Query<&GlobalTransform>,
    main: Query<&GlobalTransform, With<crate::MainCamera>>,
    mut cameras: Query<(&Camera, &GlobalTransform, &mut FlareScreen)>,
    mut kept: Local<Kept>,
) {
    let dt = time.delta_secs();
    // The eyes' protection: what is worn as the level begins at once, a
    // change after the frame's step.
    let worn = trp_sim::items::inventory::blindness_protection(&carried.equipment);
    let protection = kept.protection.get_or_insert_with(|| Protection::new(worn));
    protection.step(dt);
    protection.tell(worn);
    let (alpha_mul, size_mul) = protection.multipliers();
    let mut flashlights = Vec::new();
    let mut others = Vec::new();
    let camera_at = main.single().ok().map(GlobalTransform::translation);
    // Each flare light on, by its object: a light's flares are on while its
    // mode's object is; an infrared one's never without night vision
    // (`IkLight`: none here).
    let mut shining: Vec<(Entity, &DeviceFlare)> = Vec::new();
    let shows = |light: &DeviceFlare, now: trp_sim::items::inventory::Light| now.on && light.mode == Some(now.mode as usize) && !light.infrared;
    // The dummies', in their first mode while the debug panel's `LIGHTS` is on.
    let dummies_on = trp_sim::items::inventory::Light { on: lights.is_some_and(|lights| lights.on), mode: 0 };
    if let (Some(dummies), Some(maker)) = (dummies.as_deref(), source.as_deref().and_then(|source| source.maker.as_ref())) {
        for (tree, _) in dummies.weapon_trees().filter(|(_, shown)| *shown) {
            for device in &maker.weapon.devices {
                shining.extend(device.flares.iter().filter(|light| shows(light, dummies_on)).filter_map(|light| Some((*tree.get(light.node)?, light))));
            }
        }
    }
    // The weapon in hand's.
    if let Some(rig) = own.rig.as_deref() {
        let states = crate::weapons::tactical_view::weapon_states(&carried, &own.held, rig.devices.len());
        for (device, &now) in rig.devices.iter().zip(&states) {
            shining.extend(device.flares.iter().filter(|light| shows(light, now)).filter_map(|light| Some((*rig.bones.get(light.node)?, light))));
        }
    }
    // Those of the weapons hung on the body.
    for hung in own.slung.iter().flat_map(|slung| slung.with_devices()) {
        let states = crate::weapons::tactical_view::states_of(hung.item_in(&carried), hung.devices.len());
        for (device, &now) in hung.devices.iter().zip(&states) {
            shining.extend(device.flares.iter().filter(|light| shows(light, now)).filter_map(|light| Some((*hung.objects.get(light.node)?, light))));
        }
    }
    // The head's, seen from outside, on the model of what is worn.
    if own.controller.movement.body.point_of_view() == trp_sim::PointOfView::ThirdPerson {
        if let Some(heads) = own.heads.as_deref() {
            for (template, now, device) in own.tactical.head_devices_shown() {
                let Some(on_body) = heads.device_on_body(template) else { continue };
                shining.extend(device.flares.iter().filter(|light| shows(light, now)).filter_map(|light| Some((on_body.object(light.node)?.0, light))));
            }
        }
    }
    let mut lit = Vec::new();
    if let Some(camera_at) = camera_at {
        for (entity, light) in shining {
            let Ok(global) = globals.get(entity) else { continue };
            let at = global.translation();
            // The object's forward as the game's (its z), in Bevy's axes.
            let forward = crate::weapons::tactical_view::kept_turn(entity, &nodes) * Vec3::NEG_Z;
            // Made whole as it is switched on (`OnEnable`: nothing).
            let alpha = kept.alphas.entry(entity).or_insert(0.0);
            *alpha = flare::step_alpha(*alpha, light.controller, at.to_array(), forward.to_array(), camera_at.to_array(), dt);
            let light_alpha = if light.controller.is_some() { *alpha } else { light.total_alpha };
            lit.push(entity);
            for f in light.flares.iter().filter(|f| f.kind != flare::OFF_SCREEN) {
                let g = flare::gpu_data(f, light_alpha, light.total_scale, at.to_array(), alpha_mul, size_mul);
                if f.kind == flare::FLASHLIGHT {
                    flashlights.push(g);
                } else {
                    others.push(g);
                }
            }
        }
    }
    // Switched off, a light's alpha goes to nothing (`OnDisable`).
    kept.alphas.retain(|entity, _| lit.contains(entity));
    flare::overlap(&mut flashlights);
    let mut corners = Vec::with_capacity((flashlights.len() + others.len()) * 6 * CORNER_NUMBERS);
    for g in others.iter().chain(&flashlights) {
        push_corners(&mut corners, g);
    }
    let count = (others.len() + flashlights.len()) as u32;
    let shown = (count > 0).then(|| Arc::new(corners));
    let (multi, multi_b) = figures(&flare::MULTI_FLARE);
    let (noise, noise_b) = figures(&flare::SHIT_ON_SCREEN);
    let noise_scale = Vec4::new(flare::SHIT_ON_SCREEN.noise_scale[0], flare::SHIT_ON_SCREEN.noise_scale[1], 0.0, 0.0);
    for (camera, eye, mut screen) in &mut cameras {
        let Some(corners) = shown.as_ref().filter(|_| camera.is_active) else {
            if screen.0.is_some() {
                screen.0 = None;
            }
            continue;
        };
        let clip_from_view = camera.clip_from_view();
        let view_from_world = eye.to_matrix().inverse();
        screen.0 = Some(FlareDraw {
            uniform: FlareUniform {
                clip_from_world: clip_from_view * view_from_world,
                view_from_world,
                clip_from_view,
                camera: eye.translation().extend(time.elapsed_secs() / 20.0),
                multi,
                multi_b,
                noise,
                noise_b,
                noise_scale,
            },
            atlas: pictures.atlas.clone(),
            noise: pictures.noise.clone(),
            corners: corners.clone(),
            flares: count,
        });
    }
}
