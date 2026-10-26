//! The model in the window that shows a thing (inspect-window spec, "The
//! model"; the user, 2026-10-21: "Make it have the same 3d model as the
//! workbench"): each open window's own preview (`WeaponPreview`, from the
//! game's pool), the thing's model as its loot with its parts, turned by
//! the pose of its icon, the camera fitted to it (`PlaceInClosestPosition`)
//! and kept for its template; a drag on the preview turns it with no limit
//! to its tilt, the wheel zooms as the edit build screen's. Drawn with
//! `TARKOV UI` on, where the window is the game's.

use std::collections::HashMap;
use std::sync::Arc;

use bevy::camera::visibility::RenderLayers;
use bevy::camera::{Exposure, RenderTarget};
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;
use bevy::window::PrimaryWindow;

use trp_sim::inventory::Id;

use crate::convert;
use crate::edit_build::{in_sight, model_bounds, preview_lights, put_model, rotator_place, target, zoomed, PutModel, CAMERA_START, PIXELS_A_NOTCH, ZOOM_FARTHEST, ZOOM_NEAREST, ZOOM_PER_NOTCH};
use crate::inventory_view::{Carried, WinKind};

pub struct InfoPreviewPlugin;

impl Plugin for InfoPreviewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<InfoPreviews>().add_systems(Update, (turn, preview, spin_loaders).chain().after(crate::inventory_view::ScreenDrawn).run_if(crate::running));
    }
}

/// The layers the windows' models are drawn on, one to each of the four
/// windows that may show a thing at once (`ItemUiContext.OpenWindow`), past
/// the edit build screen's.
const LAYERS: [usize; 4] = [25, 26, 27, 28];

/// A pointer's travel in pixels turns the model by this many degrees each
/// way (`WeaponPreview.Rotate`).
const TURN_PER_PIXEL: f32 = 0.3;

/// How many frames the camera draws after what it sees changes: a turn's,
/// a new model's (its pictures arriving).
const DRAWN_TURNED: u32 = 2;
const DRAWN_MODEL: u32 = 60;

/// `PlaceInClosestPosition`: from 3 m behind, 0.15 m nearer a step for at
/// most 21 steps, backed off 0.01 m at a time.
const CLOSEST_FROM: f32 = -3.0;
const CLOSEST_STEP: f32 = 0.15;
const CLOSEST_STEPS: usize = 21;
const CLOSEST_BACK: f32 = 0.01;

/// The picture is twice the size of the preview's room (`CameraImage`).
const PICTURE_SCALE: f32 = 2.0;

/// The inspect window's own preview's lights (its template's two the camera
/// switches on as it draws: `CameraLightSwitcher`; directional, no shadows,
/// turned as they stand in the template, which the window's preview does
/// not turn with the model), on the previews' layers, a light's intensity
/// put as the edit build screen's are (`edit_build::preview_lights`).
pub(crate) fn inspect_lights(commands: &mut Commands, inspect: &trp_data::edit_build::InspectPreview, layers: &RenderLayers) -> Vec<Entity> {
    let pi = std::f32::consts::PI;
    inspect
        .lights
        .iter()
        .filter(|light| light.kind == 1)
        .map(|light| {
            let color = Color::srgb(light.color[0], light.color[1], light.color[2]);
            let place = convert::transform([0.0; 3], light.rotation, [1.0; 3]);
            commands.spawn((Name::new(light.name.clone()), DirectionalLight { color, illuminance: light.intensity * pi * 1.2, shadows_enabled: false, ..default() }, place, layers.clone())).id()
        })
        .collect()
}

/// The previews of the windows open.
#[derive(Resource, Default)]
pub struct InfoPreviews {
    views: Vec<View>,
    /// Their lights, on every layer of theirs, made once.
    lights: Vec<Entity>,
    /// The edit build screen's scene, whose preview's place and framing
    /// are taken (`PROVISIONAL`: the inspect window's pool's camera has the
    /// same field of view, its place not read); whether it could not be read
    /// (no model is drawn).
    scene: Option<Arc<trp_data::edit_build::EditBuildScene>>,
    no_scene: bool,
    /// The inspect window's own preview (`WeaponPreviewPool`'s template):
    /// its camera's planes, its two lights and its tonemap; none where it
    /// could not be read (the edit build screen's lights are taken then).
    inspect: Option<Option<Arc<trp_data::edit_build::InspectPreview>>>,
    /// Each template's `PreviewPivot` (none: it has none), and its camera's
    /// place as first fitted (`RendererExtensions._cachedPositions`, for the
    /// run).
    places: HashMap<String, Option<trp_data::icon::PreviewPlace>>,
    closest: HashMap<String, f32>,
    /// The window whose model a drag turns, and the cursor's last place.
    dragging: Option<(Id, Vec2)>,
}

struct View {
    item: Id,
    key: String,
    template: String,
    layer: usize,
    camera: Entity,
    image: Handle<Image>,
    size: UVec2,
    rotator: Entity,
    holder: Option<Entity>,
    pending: Option<PutModel>,
    /// The key of the model last put in.
    put_key: Option<String>,
    /// Whether its model is in (its loader shows until it is).
    ready: bool,
    yaw: f32,
    tilt: f32,
    camera_z: f32,
    bounds: Option<(Vec3, Vec3)>,
    zoom: f32,
    frames: u32,
}

impl InfoPreviews {
    /// The picture a window's model is drawn into, once it is in.
    pub fn picture(&self, item: Id) -> Option<Handle<Image>> {
        self.views.iter().find(|view| view.item == item && view.ready).map(|view| view.image.clone())
    }
}

/// Where a window's preview lies, which a drag on turns its model and the
/// wheel over zooms it.
#[derive(Component)]
pub struct InfoPicture(pub Id, pub u8);

/// The drag and the wheel on a preview (`ItemInfoWindowLabels.Rotate` and
/// `Zoom`): a drag begun on the preview of the window on top there turns
/// its model by the pointer's travel (`Rotate(dx, dy, 0, 0)`: the yaw back
/// by 0.3 of a degree a pixel across, the tilt by as much up, its sense as
/// the model faces, its tilt not held); the wheel over it zooms.
fn turn(
    mut previews: ResMut<InfoPreviews>,
    controller: Res<crate::controller::Controller>,
    buttons: Res<ButtonInput<MouseButton>>,
    wheel: Res<bevy::input::mouse::AccumulatedMouseScroll>,
    window: Query<&Window, With<PrimaryWindow>>,
    pictures: Query<(&InfoPicture, &RelativeCursorPosition)>,
    covers: Query<(&crate::inventory_view::Cover, &RelativeCursorPosition)>,
) {
    // (Written past its change's mark: the page is drawn afresh as a
    // model comes in, not as one turns.)
    let previews = previews.bypass_change_detection();
    if previews.views.is_empty() || !controller.inventory_open {
        previews.dragging = None;
        return;
    }
    let Ok(window) = window.single() else { return };
    let cursor = window.cursor_position();
    let top = crate::inventory_view::cover_under(covers.iter());
    let under = pictures.iter().find(|(picture, at)| at.cursor_over() && picture.1 == top).map(|(picture, _)| picture.0);
    if buttons.just_pressed(MouseButton::Left) {
        if let (Some(item), Some(at)) = (under, cursor) {
            previews.dragging = Some((item, at));
        }
    }
    if !buttons.pressed(MouseButton::Left) {
        previews.dragging = None;
    }
    if let (Some((item, last)), Some(now)) = (previews.dragging, cursor) {
        // (The pointer's travel in the screen's pixels, up for y.)
        let travel = (now - last) * window.scale_factor();
        if travel != Vec2::ZERO {
            if let Some(view) = previews.views.iter_mut().find(|view| view.item == item) {
                view.yaw -= travel.x * TURN_PER_PIXEL;
                if view.yaw >= 360.0 {
                    view.yaw -= 360.0;
                } else if view.yaw < -360.0 {
                    view.yaw += 360.0;
                }
                let facing = if view.yaw.to_radians().cos() >= 0.0 { 1.0 } else { -1.0 };
                view.tilt += -travel.y * TURN_PER_PIXEL * facing;
            }
            previews.dragging = Some((item, now));
        }
    }
    if wheel.delta.y != 0.0 {
        if let Some(view) = under.and_then(|item| previews.views.iter_mut().find(|view| view.item == item)) {
            let notches = match wheel.unit {
                bevy::input::mouse::MouseScrollUnit::Line => wheel.delta.y,
                bevy::input::mouse::MouseScrollUnit::Pixel => wheel.delta.y / PIXELS_A_NOTCH,
            };
            view.zoom += notches * ZOOM_PER_NOTCH;
        }
    }
}

/// Each window's preview: made as its window opens (its camera on a layer
/// of its own, at the turning parent's height a metre behind it,
/// `ResetRotator(-1)`; its picture twice its room), let go as it shuts;
/// the thing's model, once ready, put in hidden and shown on the next
/// frame with the camera fitted to its bounds (`PlaceInClosestPosition`,
/// kept for its template); turned and zoomed; the camera drawing only for
/// a few frames after something it sees changes.
#[allow(clippy::too_many_arguments)]
fn preview(
    mut commands: Commands,
    mut marked: ResMut<InfoPreviews>,
    carried: Res<Carried>,
    controller: Res<crate::controller::Controller>,
    look: Res<crate::health_view::HealthLook>,
    mut game: ResMut<crate::GameData>,
    (mut loose, mut images, meshes): (ResMut<crate::loose::Loose>, ResMut<Assets<Image>>, Res<Assets<Mesh>>),
    mut transforms: Query<&mut Transform>,
    globals: Query<&GlobalTransform>,
    mut cameras: Query<&mut Camera>,
    (held, said): (Res<crate::inventory_view::HandsHeld>, Res<crate::health_view::Words>),
) {
    // (A repair window's preview as a window's that shows a thing.)
    let open: Vec<(Id, String, String)> = if look.info.is_some() || look.repair.is_some() {
        carried
            .windows
            .iter()
            .filter(|window| window.kind == WinKind::Info || (window.kind == WinKind::Repair && look.repair.is_some()))
            .filter_map(|window| carried.find(window.item).map(|item| (window.item, trp_sim::inventory::rig_key(item), item.kind.template.clone())))
            .collect()
    } else {
        Vec::new()
    };
    if open.is_empty() && marked.views.is_empty() {
        return;
    }
    // (Its change marked only as a model comes in or a preview comes or
    // goes, which the page is drawn afresh for.)
    let previews = marked.bypass_change_detection();
    let mut changed = false;
    // A window shut: its preview let go of, and its model if no other
    // window shows it.
    let mut kept = Vec::new();
    for view in std::mem::take(&mut previews.views) {
        if open.iter().any(|(item, ..)| *item == view.item) {
            kept.push(view);
            continue;
        }
        commands.entity(view.camera).despawn();
        commands.entity(view.rotator).despawn();
        images.remove(&view.image);
        if !open.iter().any(|(_, key, _)| *key == view.key) {
            loose.let_go(&view.key);
        }
        changed = true;
    }
    previews.views = kept;
    if previews.scene.is_none() && !previews.no_scene && !open.is_empty() {
        previews.scene = look.edit_build().map(|eb| eb.scene.clone());
        if previews.scene.is_none() {
            match trp_data::edit_build::EditBuildScene::load(&game.unity.install) {
                Ok(scene) => previews.scene = Some(Arc::new(scene)),
                Err(e) => {
                    warn!("the inspect window draws no model: the edit build screen's scene could not be read: {e:#}");
                    previews.no_scene = true;
                }
            }
        }
    }
    let Some(scene) = previews.scene.clone() else { return };
    if previews.inspect.is_none() && !open.is_empty() {
        previews.inspect = Some(match trp_data::edit_build::InspectPreview::load(&game.unity.install) {
            Ok(inspect) => Some(Arc::new(inspect)),
            Err(e) => {
                warn!("the inspect window's own preview could not be read; the edit build screen's lights light it: {e:#}");
                None
            }
        });
    }
    let inspect = previews.inspect.clone().flatten();
    if previews.lights.is_empty() && !open.is_empty() {
        let layers = RenderLayers::from_layers(&LAYERS);
        previews.lights = match &inspect {
            Some(inspect) => inspect_lights(&mut commands, inspect, &layers),
            None => preview_lights(&mut commands, &scene, &layers),
        };
    }
    // The picture's size for a window: twice its preview's room as the
    // window is laid out now (its rows, slots, buttons and stretch).
    let picture_size = |item: Id| -> UVec2 {
        let repair = carried.windows.iter().any(|window| window.kind == WinKind::Repair && window.item == item) && !carried.windows.iter().any(|window| window.kind == WinKind::Info && window.item == item);
        let repair_room = repair.then(|| crate::health_view::repair_built(&look, &crate::health_view::RepairWords::default()).map(|built| built.preview)).flatten();
        let room = repair_room.or_else(|| carried.windows.iter().find(|window| window.kind == WinKind::Info && window.item == item).zip(carried.find(item)).and_then(|(window, thing)| {
            let figures = trp_sim::item_attributes::figures(thing, &game.db, &|key| carried.texts.say(key));
            let (parts, _) = crate::inventory_view::info_parts(&carried, thing, &held, controller.health.as_ref(), &game, &said, window.stretched);
            crate::health_view::info_built(&look, &crate::health_view::InfoWords::default(), Some(&figures), &parts, false).and_then(|built| built.preview())
        }));
        let room = room.map_or([670.0, 300.0], |rect| [rect[2], rect[3]]);
        UVec2::new((room[0] * PICTURE_SCALE).round().max(1.0) as u32, (room[1] * PICTURE_SCALE).round().max(1.0) as u32)
    };
    // A window laid out afresh (stretched, a part put in or taken out):
    // its picture made again at its new size.
    if carried.is_changed() || look.is_changed() {
        for view in &mut previews.views {
            let size = picture_size(view.item);
            if size != view.size {
                let image = target(&mut images, size);
                commands.entity(view.camera).insert(RenderTarget::Image(image.clone().into()));
                images.remove(&std::mem::replace(&mut view.image, image));
                view.size = size;
                view.frames = view.frames.max(DRAWN_TURNED);
                changed = true;
            }
        }
    }
    let new_sizes: Vec<(Id, UVec2)> = open.iter().filter(|(item, ..)| !previews.views.iter().any(|view| view.item == *item)).map(|(item, ..)| (*item, picture_size(*item))).collect();
    // A window opened: its preview made.
    for (item, key, template) in &open {
        if previews.views.iter().any(|view| view.item == *item) {
            continue;
        }
        let Some(layer) = LAYERS.iter().copied().find(|layer| !previews.views.iter().any(|view| view.layer == *layer)) else { continue };
        let Some(size) = new_sizes.iter().find(|(own, _)| own == item).map(|(_, size)| *size) else { continue };
        let image = target(&mut images, size);
        let layers = RenderLayers::layer(layer);
        let camera = commands
            .spawn((
                Name::new("Inspect window's preview camera"),
                Camera3d::default(),
                Camera { order: -5 - (layer - LAYERS[0]) as isize, clear_color: ClearColorConfig::Custom(Color::NONE), ..default() },
                RenderTarget::Image(image.clone().into()),
                AmbientLight { color: Color::WHITE, brightness: 0.0, affects_lightmapped_meshes: false },
                Exposure { ev100: 0.0 },
                Projection::Perspective(PerspectiveProjection { fov: scene.fov.to_radians(), near: inspect.as_ref().map_or(scene.near, |inspect| inspect.near), far: scene.far, ..default() }),
                Transform::from_translation(convert::position([0.0, scene.height, CAMERA_START])),
                layers.clone(),
            ))
            .id();
        // The pool's camera's photographic tonemap (Standard Assets'
        // `Tonemapping`, its exposure 2), on its picture in half floats
        // (`HdrModeSwitcher`), the engine's own tonemapping left out.
        if let Some(exposure) = inspect.as_ref().and_then(|inspect| inspect.exposure) {
            commands.entity(camera).insert((bevy::render::view::Hdr, bevy::core_pipeline::tonemapping::Tonemapping::None, crate::preview_tonemap::PhotographicTonemap::of(exposure)));
        }
        let rotator = commands.spawn((Name::new("Inspect window's preview"), rotator_place(&scene, 0.0, 0.0), Visibility::Inherited, layers)).id();
        if !previews.places.contains_key(template) {
            let prefab = game.db.items.get(template).and_then(|kind| kind.prefab_key()).map(str::to_owned);
            let place = prefab.and_then(|prefab| {
                trp_data::icon::preview_place(&mut game.unity, &prefab).unwrap_or_else(|e| {
                    warn!("{template}'s place in the inspect window's preview: {e:#}");
                    None
                })
            });
            previews.places.insert(template.clone(), place);
        }
        previews.views.push(View { item: *item, key: key.clone(), template: template.clone(), layer, camera, image, size, rotator, holder: None, pending: None, put_key: None, ready: false, yaw: 0.0, tilt: 0.0, camera_z: CAMERA_START, bounds: None, zoom: 0.0, frames: 0 });
        changed = true;
    }
    for view in &mut previews.views {
        let Some((_, key, template)) = open.iter().find(|(item, ..)| *item == view.item) else { continue };
        if *key != view.key || *template != view.template {
            (view.key, view.template) = (key.clone(), template.clone());
        }
        // The model put in last frame: its bounds had, it is shown in the
        // old one's place and the camera fitted to it.
        if let Some(put) = view.pending.take() {
            view.bounds = model_bounds(&put, &meshes, &globals);
            if let Some(old) = view.holder.replace(put.holder) {
                commands.entity(old).despawn();
            }
            commands.entity(put.holder).insert(Visibility::Inherited);
            let aspect = view.size.x as f32 / view.size.y.max(1) as f32;
            let z = *previews.closest.entry(view.template.clone()).or_insert_with(|| view.bounds.map_or(CAMERA_START, |bounds| closest(&scene, aspect, bounds)));
            view.camera_z = z;
            if let Ok(mut place) = transforms.get_mut(view.camera) {
                place.translation = convert::position([0.0, scene.height, z]);
            }
            if !view.ready {
                view.ready = true;
                changed = true;
            }
            view.frames = DRAWN_MODEL;
        }
        // Its model, by its key, once ready: put in hidden, its pivot
        // turned by the pose of its icon (`PreviewPivot.Icon.rotation`).
        if view.pending.is_none() && view.put_key.as_deref() != Some(view.key.as_str()) {
            if let Some(ready) = loose.model_for(&view.key) {
                let place = previews.places.get(&view.template).copied().flatten();
                let turn = place.map_or(Quat::IDENTITY, |place| convert::rotation(place.icon_rotation));
                view.pending = Some(put_model(&mut commands, &ready, view.rotator, &RenderLayers::layer(view.layer), turn, place));
                view.put_key = Some(view.key.clone());
            }
        }
        if let Ok(mut place) = transforms.get_mut(view.rotator) {
            let wanted = rotator_place(&scene, view.yaw, view.tilt);
            if place.rotation != wanted.rotation {
                place.rotation = wanted.rotation;
                view.frames = view.frames.max(DRAWN_TURNED);
            }
        }
        let zoom = std::mem::take(&mut view.zoom);
        if let (true, Some(bounds)) = (zoom != 0.0, view.bounds) {
            let aspect = view.size.x as f32 / view.size.y.max(1) as f32;
            let z = zoomed(&scene, view.camera_z, zoom, aspect, bounds);
            if z != view.camera_z {
                view.camera_z = z;
                if let Ok(mut place) = transforms.get_mut(view.camera) {
                    place.translation = convert::position([0.0, scene.height, z]);
                }
                view.frames = view.frames.max(DRAWN_TURNED);
            }
        }
        if let Ok(mut camera) = cameras.get_mut(view.camera) {
            let drawing = view.frames > 0 && controller.inventory_open;
            if camera.is_active != drawing {
                camera.is_active = drawing;
            }
        }
        view.frames = view.frames.saturating_sub(1);
    }
    if changed {
        marked.set_changed();
    }
}

/// How fast a window's loader turns, a second: `ESTIMATE` (its turning is
/// not read: its object has only its picture), as the edit build screen's
/// and the inventory's spinners.
const LOADER_TURNS: f32 = 1.1;

/// Each open window's loader turned while its model is made.
fn spin_loaders(time: Res<Time>, previews: Res<InfoPreviews>, carried: Res<Carried>, look: Res<crate::health_view::HealthLook>, mut pieces: Query<(&crate::health_view::Piece, &mut UiTransform)>) {
    let loading: Vec<usize> = carried
        .windows
        .iter()
        .enumerate()
        .filter(|(_, window)| window.kind == WinKind::Info && previews.picture(window.item).is_none())
        .filter_map(|(index, _)| crate::health_view::info_loader_piece(&look, index))
        .collect();
    if loading.is_empty() {
        return;
    }
    let spin = Rot2::radians((time.elapsed_secs() * LOADER_TURNS).fract() * std::f32::consts::TAU);
    for (piece, mut turn) in &mut pieces {
        if loading.contains(&piece.0) {
            turn.rotation = spin;
        }
    }
}

/// The camera's place nearest the model with all of its bounds in sight
/// (`PlaceInClosestPosition`), held between 2.7 m and 0.45 m.
fn closest(scene: &trp_data::edit_build::EditBuildScene, aspect: f32, bounds: (Vec3, Vec3)) -> f32 {
    let mut z = CLOSEST_FROM;
    for _ in 0..CLOSEST_STEPS {
        z += CLOSEST_STEP;
        if !in_sight(scene, z, aspect, bounds) {
            // (Never further back than where it began: a model too large
            // to be seen whole from there is seen from there.)
            while !in_sight(scene, z, aspect, bounds) && z > CLOSEST_FROM {
                z -= CLOSEST_BACK;
            }
            break;
        }
    }
    z.clamp(ZOOM_FARTHEST, ZOOM_NEAREST)
}
