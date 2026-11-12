//! The 3D model in the window that shows a thing, the same model as the workbench's.
//!
//! By the inspect-window spec, "The model": each open window's own preview (`WeaponPreview`, from the
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

use trp_sim::items::inventory::Id;

use crate::platform::convert;
use crate::screens::edit_build::{in_sight, model_bounds, preview_lights, put_model, rotator_place, target, zoomed, PutModel, CAMERA_START, PIXELS_A_NOTCH, ZOOM_FARTHEST, ZOOM_NEAREST, ZOOM_PER_NOTCH};
use crate::screens::inventory_view::{Carried, WinKind};

mod previewing;

use previewing::{preview, turn};

pub struct InfoPreviewPlugin;

impl Plugin for InfoPreviewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<InfoPreviews>().add_systems(Update, (turn, preview, spin_loaders).chain().after(crate::screens::inventory_view::ScreenDrawn).run_if(crate::running));
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
pub(crate) fn inspect_lights(commands: &mut Commands, inspect: &trp_data::interface::edit_build::InspectPreview, layers: &RenderLayers) -> Vec<Entity> {
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

/// The inspect window's preview's lights for the game's own surface
/// (`game_surface::Way::Preview`): its template's directional lights as
/// `inspect_lights` turns them, each its colour times its intensity in the
/// game's numbers; the eye the camera's own; the top ambient as last set,
/// the level's (`PROVISIONAL`, as the world's glass has it).
pub(crate) fn preview_surface(inspect: &trp_data::interface::edit_build::InspectPreview, top_ambient: [f32; 3]) -> crate::rendering::game_surface::SurfaceView {
    crate::rendering::game_surface::SurfaceView {
        way: crate::rendering::game_surface::Way::Preview,
        eye: None,
        lights: inspect.lights.iter().filter(|light| light.kind == 1).map(|light| crate::rendering::game_surface::SurfaceView::light(convert::rotation(light.rotation), [light.color[0], light.color[1], light.color[2]], light.intensity)).collect(),
        ambient: top_ambient,
    }
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
    scene: Option<Arc<trp_data::interface::edit_build::EditBuildScene>>,
    no_scene: bool,
    /// The inspect window's own preview (`WeaponPreviewPool`'s template):
    /// its camera's planes, its two lights and its tonemap; none where it
    /// could not be read (the edit build screen's lights are taken then).
    inspect: Option<Option<Arc<trp_data::interface::edit_build::InspectPreview>>>,
    /// Each template's `PreviewPivot` (none: it has none), and its camera's
    /// place as first fitted (`RendererExtensions._cachedPositions`, for the
    /// run).
    places: HashMap<String, Option<trp_data::interface::icon::PreviewPlace>>,
    closest: HashMap<String, f32>,
    /// The window whose model a press on its preview is to turn, where the
    /// press was, the cursor's last place, and whether the drag has begun.
    dragging: Option<(Id, Vec2, Vec2, bool)>,
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

/// How fast a window's loader turns, a second: `ESTIMATE` (its turning is
/// not read: its object has only its picture), as the edit build screen's
/// and the inventory's spinners.
const LOADER_TURNS: f32 = 1.1;

/// Each open window's loader turned while its model is made.
fn spin_loaders(time: Res<Time>, previews: Res<InfoPreviews>, carried: Res<Carried>, look: Res<crate::screens::health_view::HealthLook>, mut pieces: Query<(&crate::screens::health_view::Piece, &mut UiTransform)>) {
    let loading: Vec<usize> = carried
        .windows
        .iter()
        .enumerate()
        .filter(|(_, window)| window.kind == WinKind::Info && previews.picture(window.item).is_none())
        .filter_map(|(index, _)| crate::screens::health_view::info_loader_piece(&look, index))
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
pub(crate) fn closest(scene: &trp_data::interface::edit_build::EditBuildScene, aspect: f32, bounds: (Vec3, Vec3)) -> f32 {
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
