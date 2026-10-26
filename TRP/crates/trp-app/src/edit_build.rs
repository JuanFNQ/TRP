//! The edit build screen (workbench spec, "The edit build screen"),
//! opened at the workbench for the weapon in hand (the user, 2026-10-20).
//!
//! The weapon's build is a copy of its item, changed slot by slot from the
//! dropdowns; ASSEMBLE puts it on the weapon (`trp_sim::edit_build::
//! assemble`), whose rig of the hands follows (`weapon`, a build's key).
//! The preview is the build's model (`loose`'s, by its key) under its own
//! camera and the screen's own lights, drawn into a picture behind the
//! screen, turned by a drag. With `TARKOV UI` the screen is the game's own
//! (`health_view::EditBuildLook`): its scene's panel, each slot's view from
//! the slot's prefab, the empty view and a part's view as the game's, the
//! dropdown from the scene's; without it, the app's plain look at the same
//! places.
//!
//! What is drawn is kept, and only what changes is drawn again (0.96.2, the
//! user: "The UI flashses when equipping different items"): the screen's
//! own objects as its toggles and a build ready to assemble change; a
//! slot's view as the mouse comes onto it or its dropdown opens; the slots
//! all at once only when the preview's model is replaced (which is when the
//! game makes them again, `RefreshWeapon`), each at its place from the
//! frame it is made, the old ones and the old model kept until then; a
//! part's picture comes into its view's spinner (`inventory_view::
//! fill_pictures`) with nothing else drawn again.

use std::collections::HashMap;
use std::sync::Arc;

use bevy::asset::RenderAssetUsages;
use bevy::camera::primitives::MeshAabb;
use bevy::camera::visibility::RenderLayers;
use bevy::camera::{Exposure, RenderTarget};
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::math::{Affine3A, Vec3A};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};
use bevy::ui::RelativeCursorPosition;
use bevy::window::PrimaryWindow;
use trp_data::screen::ScenePanel;
use trp_sim::edit_build::{self as rules, BuildSlot, ModClass};
use trp_sim::inventory::{is, Id, Item, Kind};

use crate::convert;
use crate::health_view::{draw_nodes, EditBuildLook, Frame, HealthLook, Marks, Piece, Words};
use crate::inventory_view::{words, Carried, HandsHeld};
use crate::ui_sound::{Sound, Ui};

/// The preview's own layer: its model, camera and lights.
const LAYER: usize = 24;
/// The screen's height in its units.
const HIGH: f32 = 1080.0;
/// The game's colours of a slot by its class (`ModdingScreenSlotView.
/// UpdateInfo`), and of one that is a vital part missing, its name's ground
/// then.
const FUNCTIONAL: Color = Color::srgb_u8(242, 242, 242);
const VITAL: Color = Color::srgb_u8(16, 93, 165);
const GEAR: Color = Color::srgb_u8(167, 249, 142);
const MISSING: Color = Color::srgb_u8(214, 53, 53);
const MISSING_GROUND: Color = Color::srgba_u8(179, 27, 27, 220);
/// The app's look's words' colour (the scene's caption and toggles).
const INK: Color = Color::srgb(0.941, 0.937, 0.925);
/// What is drawn behind the preview where the game's menu shows its own
/// scene (`EnvironmentOverlay` disabled): a dark grey, `ESTIMATE`; very
/// slightly see-through, so that the range shows faintly behind it (the
/// user, 2026-10-20: "Make the background very slighly transparent"; how
/// much is the app's).
const BEHIND: Color = Color::srgb(0.055, 0.058, 0.060);
const BEHIND_ALPHA: f32 = 0.92;
/// The mouse's sideways and upward counts as the game's axes have them
/// (the install's input manager's 0.05 a count, the binding's 5), each
/// turning the preview by two times 0.3 of it (`TranslateAxes`,
/// `WeaponPreview.Rotate`): `PROVISIONAL`, the screen's axes taken as the
/// turn's.
const TURN_PER_COUNT: f32 = 0.05 * 5.0 * 2.0 * 0.3;
/// How far the mouse moves with its button down on the preview before the
/// drag that turns it begins (`DragTrigger.onBeginDrag`: the event
/// system's drag threshold, in pixels): Unity's own default, the game's
/// event system's not read, `PROVISIONAL`.
const DRAG_THRESHOLD: f32 = 10.0;
/// The arrows' turn a frame (`EditorStatic.WeaponRotationSpeed`):
/// `ESTIMATE`, its figure not read.
const ARROW_TURN: f32 = 1.5;
/// How long the game's notice stays: `ESTIMATE`.
const NOTICE_FOR: f32 = 3.0;
/// A part's view's cells (`ItemViewFactory.GetCellPixelSize`: 63 a cell
/// and one), the dropdown's grid's cells and the space between them
/// (`DropdownMenu/Container`'s `GridLayoutGroup`, its columns as many as
/// the square root of its views, rounded up: `Flexible`), and how far its
/// menu keeps off the screen's foot (`DropDownMenu._bottomMargin`).
const CELL: f32 = 63.0;
const GRID_CELL: f32 = 64.0;
const GRID_SPACING: f32 = 5.0;
const MENU_FOOT: f32 = 120.0;
/// The line from a bone to its slot's view (`LineRenderer`, its width not
/// read): a unit, `ESTIMATE`.
const LINE: f32 = 1.0;
/// The game's views of things carry the inventory's layer (`ItemView`):
/// one no screen of the inventory's has.
const VIEW_LAYER: u8 = 200;
/// Frames the preview's camera goes on drawing after it is turned or the
/// window changes, and after a model is put in (its meshes and pictures
/// may reach the card a few frames late): it draws nothing otherwise, the
/// picture kept (the app's).
const DRAWN_TURNED: u32 = 2;
const DRAWN_MODEL: u32 = 60;
/// How fast the loader turns, a second: `ESTIMATE` (its animation is not
/// read), as the inventory's spinners.
const LOADER_TURNS: f32 = 1.1;
/// What is laid out to a fraction of a pixel (the slots' lines).
const UNROUNDED: bevy::ui::LayoutConfig = bevy::ui::LayoutConfig { use_rounding: false };
/// Where the screen's pieces are numbered (`health_view::Piece`), past
/// every other panel's.
const EB_SCREEN: usize = 20 << 20;
const EB_SLOT: usize = 21 << 20;
const EB_EMPTY: usize = 22 << 20;

static TURNING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Whether the preview is being turned by a drag (the mouse held hidden,
/// `cursor`).
pub fn turning() -> bool {
    TURNING.load(std::sync::atomic::Ordering::Relaxed)
}

pub struct EditBuildPlugin;

impl Plugin for EditBuildPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EditBuild>().add_systems(Update, (release, open, keys, turn, preview, place, draw, click).chain().after(crate::inventory_view::ScreenDrawn).run_if(crate::running));
    }
}

/// A slot by the item it is on and its place among that item's slots.
type SlotId = (Id, usize);

/// The screen as it stands.
#[derive(Resource, Default)]
pub struct EditBuild {
    pub open: bool,
    /// The weapon in hand it was opened for, and the build as changed, its
    /// key, and how many times either has changed; whether a change has
    /// been built since it opened; the weapon to be taken in hand again as
    /// the screen has shut, and whether it is drawn racking
    /// (`inventory_view::BenchAsk`).
    weapon: Option<Id>,
    built: bool,
    give_back: Option<(Id, bool)>,
    build: Option<Item>,
    build_key: String,
    made: u64,
    /// The preview's turn (degrees); where the left button went down on the
    /// preview, and whether that has become a drag that turns it; where the
    /// cursor was last (a run nobody is at the keys of turns it by the
    /// cursor's travel: a capture's drag).
    yaw: f32,
    tilt: f32,
    pressed_at: Option<Vec2>,
    dragging: bool,
    cursor: Option<Vec2>,
    /// How far down the app's dropdown is scrolled, in pixels.
    scroll: f32,
    /// The toggles: vital parts, functional mods, equipment parts.
    toggles: [bool; 3],
    /// The build the preview's model is of, the slots of it shown, counted
    /// each time they change; the one whose dropdown is open, and whether a
    /// dropdown has opened since the slots were made (the button's picture,
    /// `SetMenuOpen`); the one the mouse is on.
    shown: Option<Item>,
    slots: Vec<BuildSlot>,
    slots_made: u64,
    slots_for: Option<(u64, [bool; 3], bool)>,
    menu: Option<SlotId>,
    menu_seen: bool,
    hovered: Option<SlotId>,
    /// Each shown slot's bone and view on the screen (the screen's units,
    /// from its top left), and what they were worked out for.
    placed: Vec<Option<([f32; 2], [f32; 2])>>,
    placed_for: Option<(f32, f32, u64, UVec2, u64, f32)>,
    /// The wheel's zoom not yet made, in metres.
    zoom: f32,
    notice: Option<(String, f32)>,
    /// What is read once: the screen's scene, every part of the game, the
    /// words of the slots' names, where each weapon's model stands (its
    /// `PreviewPivot`, by template), each slot's filters' marks (by its
    /// item's template and its place).
    scene: Option<Arc<trp_data::edit_build::EditBuildScene>>,
    /// The inspect window's own preview, whose lights and tonemap light the
    /// model here (the user's: `decisions.md`); none where it could not be
    /// read (the screen's own lights then).
    inspect: Option<Arc<trp_data::edit_build::InspectPreview>>,
    parts: Option<Arc<Vec<Arc<Kind>>>>,
    texts: HashMap<String, String>,
    places: HashMap<String, Option<trp_data::icon::PreviewPlace>>,
    filters: HashMap<(String, usize), Vec<&'static str>>,
    preview: Preview,
    drawn: Drawn,
    /// The game's screen made ready to draw, once.
    prepared: Option<Prepared>,
    /// The models to let go of as the screen has shut.
    released: Vec<String>,
    /// The build the characteristics are compared with
    /// (`ItemObserveScreen._compareItem`: the build as the screen was given
    /// it, with a part the mouse rests on in a dropdown put in its slot and
    /// taken out again as it leaves), counted each time it changes; and
    /// whether the panel is folded to its INFO button.
    compare: Option<Item>,
    compare_made: u64,
    stats_folded: bool,
}

/// The preview's entities: its turning parent, the model shown (its holder
/// and its key), one made and not yet shown, each slot's bone in the shown
/// model in the turning parent's frame; the camera, its picture and its
/// lights; how many more frames the camera draws.
#[derive(Default)]
struct Preview {
    rotator: Option<Entity>,
    holder: Option<Entity>,
    key: Option<String>,
    model: u64,
    pending: Option<Model>,
    /// The model last asked for (`Loose::model_for`).
    asked: Option<String>,
    bones: HashMap<SlotId, Affine3A>,
    camera: Option<Entity>,
    image: Option<Handle<Image>>,
    size: UVec2,
    lights: Vec<Entity>,
    frames: u32,
    /// The camera's place along its axis (the engine's z under the
    /// turning parent's parent: −1 as an item is first shown,
    /// `ResetRotator(-1)`), and the shown model's bounds as it was put in
    /// (Bevy's axes), which the zoom keeps in sight.
    camera_z: f32,
    bounds: Option<(Vec3, Vec3)>,
}

/// The zoom (the user, 2026-10-21: "We should be able to zoom it in and out
/// slightly"; the game's edit build screen has none: its inspect window's,
/// `ItemInfoWindowLabels.Zoom`, `WeaponPreview.Zoom`): a notch of the wheel
/// moves the camera 0.12 m along its axis, toward the model for a notch up;
/// moved in, it backs off a centimetre at a time until the whole of the
/// model's bounds as put in is in sight again; it is held between 2.7 m and
/// 0.45 m in front of the turning parent.
pub(crate) const ZOOM_PER_NOTCH: f32 = 0.12;
const ZOOM_BACK: f32 = 0.01;
pub(crate) const ZOOM_NEAREST: f32 = -0.45;
pub(crate) const ZOOM_FARTHEST: f32 = -2.7;
/// The camera's place as an item is first shown (`ResetRotator(-1)`).
pub(crate) const CAMERA_START: f32 = -1.0;
/// A wheel's travel given in pixels (a touchpad's) taken as notches: the
/// app's.
pub(crate) const PIXELS_A_NOTCH: f32 = 100.0;

/// Whether every corner of a box (Bevy's axes) lies in the preview camera's
/// sight with the camera at that place (`IsFullyVisibleByCamera`: inside
/// all six planes of its frustum): the camera looks along Bevy's −z from
/// the turning parent's height.
pub(crate) fn in_sight(scene: &trp_data::edit_build::EditBuildScene, camera_z: f32, aspect: f32, (low, high): (Vec3, Vec3)) -> bool {
    let eye = convert::position([0.0, scene.height, camera_z]);
    let tan = (scene.fov.to_radians() / 2.0).tan();
    (0..8).all(|corner| {
        let p = Vec3::new(if corner & 1 == 0 { low.x } else { high.x }, if corner & 2 == 0 { low.y } else { high.y }, if corner & 4 == 0 { low.z } else { high.z });
        let ahead = eye.z - p.z;
        (scene.near..=scene.far).contains(&ahead) && (p.y - eye.y).abs() <= ahead * tan && (p.x - eye.x).abs() <= ahead * tan * aspect
    })
}

/// The camera's place after a zoom by that much (`WeaponPreview.Zoom`).
pub(crate) fn zoomed(scene: &trp_data::edit_build::EditBuildScene, camera_z: f32, zoom: f32, aspect: f32, bounds: (Vec3, Vec3)) -> f32 {
    let mut z = camera_z + zoom;
    if zoom > 0.0 && !in_sight(scene, z, aspect, bounds) {
        // (No further back than its farthest, where nothing is ever
        // backed off past.)
        while !in_sight(scene, z, aspect, bounds) && z > ZOOM_FARTHEST {
            z -= ZOOM_BACK;
        }
    }
    z.clamp(ZOOM_FARTHEST, ZOOM_NEAREST)
}

/// A model put into the preview hidden, shown on the next frame once its
/// objects' places are worked out: what was put in, its key and the build
/// it is of.
struct Model {
    put: PutModel,
    key: String,
    build: Item,
}

/// A thing's model put into a preview (`WeaponPreview.PositionGameObject`),
/// hidden: its holder, its objects with their names and parents, and what
/// it draws that its bounds are of (`GetBounds`: the meshes on objects
/// that are on, but a lens's and the muzzle's jet), by object.
pub(crate) struct PutModel {
    pub(crate) holder: Entity,
    pub(crate) nodes: Vec<Entity>,
    pub(crate) names: Vec<String>,
    pub(crate) parents: Vec<Option<usize>>,
    pub(crate) bounded: Vec<(usize, Handle<Mesh>)>,
}

/// A thing's model put under a preview's turning parent, hidden, on its
/// layer: its pivot turned by `turn` (Bevy's) and scaled by the
/// `PreviewPivot`, its container at minus its pivot, the weapon's object
/// at its place in the container (`PositionGameObject`); with no
/// `PreviewPivot`, about the model's own first object (`PROVISIONAL`: the
/// game's bounds' middle).
pub(crate) fn put_model(commands: &mut Commands, ready: &crate::loose::Ready, rotator: Entity, layer: &RenderLayers, turn: Quat, place: Option<trp_data::icon::PreviewPlace>) -> PutModel {
    let (scale, pivot, object) = match place {
        Some(place) => (Vec3::from(place.scale), place.pivot, convert::transform(place.object.0, place.object.1, place.object.2)),
        None => (Vec3::ONE, [0.0; 3], Transform::IDENTITY),
    };
    let holder = commands.spawn((Transform { translation: Vec3::ZERO, rotation: turn, scale }, Visibility::Hidden, layer.clone(), ChildOf(rotator))).id();
    let container = commands.spawn((Transform::from_translation(convert::position([-pivot[0], -pivot[1], -pivot[2]])), Visibility::Inherited, layer.clone(), ChildOf(holder))).id();
    let top = commands.spawn((object, Visibility::Inherited, layer.clone(), ChildOf(container))).id();
    let mut nodes = Vec::with_capacity(ready.nodes.len());
    for (parent, own, on) in &ready.nodes {
        let under = parent.and_then(|parent| nodes.get(parent).copied()).unwrap_or(top);
        let shown = if *on { Visibility::Inherited } else { Visibility::Hidden };
        nodes.push(commands.spawn((*own, shown, layer.clone(), ChildOf(under))).id());
    }
    for (k, (node, mesh, material)) in ready.drawn.iter().enumerate() {
        if crate::loose::left_out(ready.lens.get(k).copied().flatten(), true) {
            continue;
        }
        commands.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(material.clone()), Transform::IDENTITY, layer.clone(), ChildOf(nodes[*node])));
    }
    let parents: Vec<Option<usize>> = ready.nodes.iter().map(|(parent, ..)| *parent).collect();
    // (An object is on where it and all it hangs from are.)
    let mut active = vec![true; ready.nodes.len()];
    for (index, (parent, _, on)) in ready.nodes.iter().enumerate() {
        active[index] = *on && parent.and_then(|parent| active.get(parent).copied()).unwrap_or(true);
    }
    let bounded = ready
        .drawn
        .iter()
        .filter(|(node, ..)| active.get(*node).copied().unwrap_or(false) && ready.names.get(*node).is_none_or(|name| !name.contains("linza") && name != "MuzzleJetCombinedMesh"))
        .map(|(node, mesh, _)| (*node, mesh.clone()))
        .collect();
    PutModel { holder, nodes, names: ready.names.clone(), parents, bounded }
}

/// A model's bounds in the world as it was put in (`GetBounds`): each
/// mesh's box through its object's place, those of no size left out; none
/// before its objects' places are had.
pub(crate) fn model_bounds(put: &PutModel, meshes: &Assets<Mesh>, globals: &Query<&GlobalTransform>) -> Option<(Vec3, Vec3)> {
    let mut bounds: Option<(Vec3, Vec3)> = None;
    for (node, mesh) in &put.bounded {
        let (Some(aabb), Ok(global)) = (meshes.get(mesh).and_then(MeshAabb::compute_aabb), globals.get(put.nodes[*node])) else { continue };
        if aabb.half_extents == Vec3A::ZERO {
            continue;
        }
        let affine = global.affine();
        for corner in 0..8 {
            let sign = Vec3A::new(if corner & 1 == 0 { -1.0 } else { 1.0 }, if corner & 2 == 0 { -1.0 } else { 1.0 }, if corner & 4 == 0 { -1.0 } else { 1.0 });
            let p = Vec3::from(affine.transform_point3a(aabb.center + aabb.half_extents * sign));
            bounds = Some(bounds.map_or((p, p), |(low, high)| (low.min(p), high.max(p))));
        }
    }
    bounds
}

/// A preview's lights, as the edit build screen's scene has them, on those
/// layers (`WeaponPreview._weaponPreviewLights`): as the game moves their
/// object to the world's middle (`DisplaySettingsChangedHandler`), a point
/// light's intensity by Bevy's fall-off where the engine's has halved it
/// at a fifth of its range, as the explosions' (`ESTIMATE`).
pub(crate) fn preview_lights(commands: &mut Commands, scene: &trp_data::edit_build::EditBuildScene, layer: &RenderLayers) -> Vec<Entity> {
    let pi = std::f32::consts::PI;
    let mut lights = Vec::new();
    for light in scene.lights.iter().filter(|light| light.on) {
        let color = Color::srgb(light.color[0], light.color[1], light.color[2]);
        let place = convert::transform(light.position, light.rotation, [1.0; 3]);
        let entity = match light.kind {
            1 => commands.spawn((Name::new(light.name.clone()), DirectionalLight { color, illuminance: light.intensity * pi * 1.2, shadows_enabled: false, ..default() }, place, layer.clone())).id(),
            _ => {
                let reach = 0.2 * light.range;
                let lumens = 4.0 * pi * reach * reach * light.intensity / (1.0 + 25.0 * 0.2 * 0.2) * pi * 1.2;
                commands.spawn((Name::new(light.name.clone()), PointLight { color, intensity: lumens, range: light.range, radius: 0.0, shadows_enabled: false, ..default() }, place, layer.clone())).id()
            }
        };
        lights.push(entity);
    }
    lights
}

/// What is drawn, and for what: the screen's layers, under and over one
/// another in the game's order (the preview's picture; the caption; the
/// slots' lines; the slots; the dropdown; the rest of the screen; the
/// notice), each slot's nodes and what its view shows.
#[derive(Default)]
struct Drawn {
    root: Option<Entity>,
    picture: Option<Entity>,
    under: Option<Entity>,
    lines: Option<Entity>,
    slots: Option<Entity>,
    dropdown: Option<Entity>,
    over: Option<Entity>,
    notice: Option<Entity>,
    scale: Option<(f32, f32)>,
    statics: Option<Statics>,
    statics_for: Option<(u64, [bool; 3])>,
    slots_for: Option<u64>,
    boxes: Vec<SlotNodes>,
    dropdown_for: Option<Option<SlotId>>,
    notice_for: Option<Option<String>>,
    /// The characteristics panel's layer, what it was drawn for, and its
    /// bars' comparison pieces (each with its colour), which pulse.
    stats: Option<Entity>,
    stats_for: Option<(u64, u64, bool)>,
    pulse: Vec<(usize, Color)>,
}

/// A slot's nodes: its own node (raised while the mouse is on it,
/// `SetAsLastSibling`) and its view's, and what the view shows.
struct SlotNodes {
    id: SlotId,
    own: Entity,
    view: Entity,
    shown: BoxShown,
}

/// What a slot's view shows that changes while it stands: its dropdown's
/// button (on the mouse or open), and that button's picture.
#[derive(Clone, Copy, PartialEq)]
struct BoxShown {
    button: bool,
    open: bool,
}

/// What the screen's own objects show.
#[derive(Clone, PartialEq)]
struct Statics {
    name: String,
    toggles: [bool; 3],
    ready: bool,
}

/// The game's screen made ready to draw: its panel with what its pictures
/// and texts ask of its layout put on it, its texts' words, and the
/// objects drawn by the app's systems; a slot view's view object and where
/// its pivot and its menu's anchor lie (`Frame::Is([0, 0])`).
struct Prepared {
    screen: ScenePanel,
    /// The characteristics panel's top in the screen.
    stats: Option<usize>,
    words: HashMap<usize, String>,
    empty_words: HashMap<usize, String>,
    sub_caption: Option<usize>,
    loader: Option<usize>,
    marks: [Option<usize>; 4],
    toggle: [Option<usize>; 4],
    buttons: Vec<usize>,
    save: Option<usize>,
    assemble: Option<usize>,
    assemble_button: Option<usize>,
    ready: Option<usize>,
    ready_button: Option<usize>,
    back: Option<usize>,
    dropdown: Option<usize>,
    none_holder: Option<usize>,
    grid: Option<usize>,
    caption: Vec<usize>,
    preview_panel: Option<usize>,
    menu_item: Option<usize>,
    item_rect: [f32; 4],
    menu_anchor: [f32; 2],
    bone_icon: Option<usize>,
    bone_panel: Option<usize>,
    bone_back: Option<usize>,
    bone_name: Option<usize>,
    slot_button: Option<usize>,
    slot_fade: Option<usize>,
    slot_debug: Option<usize>,
}

#[derive(Component)]
struct ScreenRoot;
#[derive(Component)]
struct PreviewPicture;
/// A slot's node placed each frame: its line, its bone's node or its view's
/// node, by the slot's place among those shown.
#[derive(Component, Clone, Copy)]
enum SlotPiece {
    Line(usize),
    Bone(usize),
    View(usize),
}
/// A slot's view's area the mouse is on.
#[derive(Component)]
struct SlotHover(SlotId);
/// The app's look's loader.
#[derive(Component)]
struct AppLoader;
/// The app's dropdown's list, scrolled by the wheel.
#[derive(Component)]
struct DropdownList;
#[derive(Component, Clone)]
enum Press {
    Open(SlotId),
    Pick(SlotId, Option<Arc<Kind>>),
    Toggle(usize),
    Assemble,
    Back,
    Shut,
    /// The characteristics panel folded or unfolded (`SwitchState`).
    Info,
}

/// The workbench's line chosen: the screen opens for the weapon in hand,
/// its build a copy of it; what is read once is read.
#[allow(clippy::too_many_arguments)]
fn open(
    mut asked: ResMut<crate::workbench::WorkbenchAsked>,
    mut hands_asked: ResMut<crate::inventory_view::AskedFor>,
    mut state: ResMut<EditBuild>,
    carried: Res<Carried>,
    held: Res<HandsHeld>,
    look: Res<HealthLook>,
    mut game: ResMut<crate::GameData>,
    mut controller: ResMut<crate::controller::Controller>,
    mut sounds: MessageWriter<Sound>,
) {
    if !std::mem::take(&mut asked.0) || state.open {
        return;
    }
    let Some(weapon) = held.item.and_then(|id| carried.equipment.item.find(id)).filter(|item| is(item, "Weapon")) else { return };
    let state = &mut *state;
    if state.scene.is_none() {
        state.scene = look.edit_build().map(|eb| eb.scene.clone());
    }
    if state.scene.is_none() {
        match trp_data::edit_build::EditBuildScene::load(&game.unity.install) {
            Ok(scene) => state.scene = Some(Arc::new(scene)),
            Err(e) => warn!("the edit build screen's scene could not be read: {e:#}"),
        }
    }
    if state.inspect.is_none() {
        match trp_data::edit_build::InspectPreview::load(&game.unity.install) {
            Ok(inspect) => state.inspect = Some(Arc::new(inspect)),
            Err(e) => warn!("the inspect window's own preview could not be read; the edit build screen's own lights light it: {e:#}"),
        }
    }
    if state.parts.is_none() {
        let began = std::time::Instant::now();
        let db = &game.db;
        let kinds = db.items.keys().filter_map(|id| Kind::of(db, id).ok().map(Arc::new));
        let mut parts = rules::all_parts(kinds);
        parts.sort_by(|a, b| a.name.cmp(&b.name));
        info!("the edit build screen: {} parts of the game, read in {:.2} s", parts.len(), began.elapsed().as_secs_f32());
        state.parts = Some(Arc::new(parts));
        let words: Vec<&str> = SLOT_WORDS.to_vec();
        state.texts = trp_data::inventory::texts(&game.unity.install, "en", &words).unwrap_or_default();
    }
    let template = weapon.kind.template.clone();
    if !state.places.contains_key(&template) {
        let key = game.db.items.get(&template).and_then(|item| item.prefab_key()).map(str::to_owned);
        let place = key.and_then(|key| trp_data::icon::preview_place(&mut game.unity, &key).unwrap_or_else(|e| {
            warn!("{template}'s place in the preview: {e:#}");
            None
        }));
        state.places.insert(template, place);
    }
    (state.open, state.weapon, state.build) = (true, Some(weapon.id), Some(weapon.clone()));
    // The build compared with is the build as given (`_compareItem`).
    // PROVISIONAL: the characteristics open unfolded (the panel's
    // `_expanded` not read; the user's capture shows it so).
    (state.compare, state.stats_folded) = (Some(weapon.clone()), false);
    state.compare_made += 1;
    state.build_key = trp_sim::inventory::rig_key(weapon);
    state.made += 1;
    (state.yaw, state.tilt, state.pressed_at, state.dragging, state.menu, state.menu_seen, state.hovered, state.notice) = (0.0, 0.0, None, false, None, false, None, None);
    (state.shown, state.slots_for, state.placed_for) = (None, None, None);
    state.slots.clear();
    state.placed.clear();
    state.slots_made += 1;
    state.toggles = [true; 3];
    controller.edit_build = true;
    // The weapon put away while the screen stands open (the user's).
    hands_asked.bench().put_away = true;
    (state.give_back, state.built) = (None, false);
    sounds.write(Sound::Ui(Ui::MenuModdingOpen));
    info!("the edit build screen opened for {}", weapon.kind.name);
}

/// The game's notice of a weapon built (`WEAPON_HAS_BEEN_BUILT`).
const BUILT: &str = "Weapon has been built";

/// Whether a build lacks a part one of its items cannot do without (a
/// vital slot empty, `MissingVitalParts`): such a build is not built
/// (the user, 2026-10-21: "The weapon cannot be built unless required parts
/// are installed ... the akm does not work without a handguard so we
/// shouldnt be able to build it"), its ASSEMBLE dim and its panel of a
/// build ready to assemble hidden.
fn vital_missing(build: &Item) -> bool {
    rules::build_slots(build).iter().any(|slot| slot.missing)
}

/// The words the slots' names are looked up by (`GetModSlotName`), and the
/// notice's.
const SLOT_WORDS: [&str; 31] = [
    BUILT,
    "Magazine", "Stock", "Foregrip", "Barrel", "Charge", "Receiver", "Shaft", "PGrip", "mod_mount", "mod_scope", "mod_tactical", "mod_handguard", "mod_muzzle", "mod_sight_front", "mod_sight_rear", "mod_launcher", "mod_bipod", "mod_silencer",
    "mod_tactical_2", "mod_gas_block", "mod_equipment", "mod_equipment_000", "mod_equipment_001", "mod_nvg", "mod_flashlight", "mod_muzzle_001", "mod_reciever", "mod_pistol_grip", "mod_charge", "mod_mag_shaft",
];

/// Escape: the dropdown shut if open, else the screen (`TranslateCommand`).
fn keys(keys: Res<ButtonInput<KeyCode>>, mut state: ResMut<EditBuild>, mut commands: Commands, mut sounds: MessageWriter<Sound>) {
    if !state.open || !keys.just_pressed(KeyCode::Escape) {
        return;
    }
    if state.menu.take().is_some() {
        state.menu_seen = true;
        return;
    }
    shut(&mut state, &mut commands, &mut sounds);
}

/// The screen shut: the character has its keys back once the mouse's
/// left button is up, so the click that shut it (BACK) is no shot; and the
/// weapon put away for it is asked for in hand again (racked as it is
/// drawn where a change was built: `inventory_view::chamber_out`).
fn release(mut state: ResMut<EditBuild>, buttons: Res<ButtonInput<MouseButton>>, mut controller: ResMut<crate::controller::Controller>, mut hands_asked: ResMut<crate::inventory_view::AskedFor>) {
    if !state.open && controller.edit_build && !buttons.pressed(MouseButton::Left) {
        controller.edit_build = false;
    }
    if let Some((weapon, rack)) = state.give_back.take() {
        let bench = hands_asked.bench();
        (bench.take, bench.rack) = (Some(weapon), rack);
    }
}

/// The screen shut: its nodes, its preview and its camera go, and the
/// keys go back to the character as the left button is let go
/// (`release`; `Close`: `MenuModdingClose`). With a change not built the
/// game asks first (not built: it shuts).
fn shut(state: &mut EditBuild, commands: &mut Commands, sounds: &mut MessageWriter<Sound>) {
    state.open = false;
    state.give_back = state.weapon.map(|weapon| (weapon, state.built));
    state.build = None;
    state.shown = None;
    state.menu = None;
    state.dragging = false;
    state.pressed_at = None;
    TURNING.store(false, std::sync::atomic::Ordering::Relaxed);
    if let Some(root) = state.drawn.root {
        commands.entity(root).despawn();
    }
    state.drawn = Drawn::default();
    let preview = std::mem::take(&mut state.preview);
    state.released = preview.key.iter().chain(preview.asked.iter()).cloned().collect();
    for entity in preview.rotator.into_iter().chain(preview.camera).chain(preview.lights) {
        commands.entity(entity).try_despawn();
    }
    sounds.write(Sound::Ui(Ui::MenuModdingClose));
}

/// A drag across the preview turns it (`DragTrigger`, `TranslateAxes`,
/// `Rotate`): begun once the mouse has moved past the drag's threshold
/// with its button down on the preview (a click is no turn), then by the
/// mouse's own travel, which the cursor's place (held in the window's
/// middle while it turns, `cursor`) has no part in (0.96.2, the user: "When
/// i click on different parts of the build UI the gun flings around": the
/// cursor's jump to the middle was taken for a move); its yaw less the
/// sideways move, its tilt the upward move, flipped while the weapon faces
/// away, held within ±30°; the arrows turn it too.
#[allow(clippy::too_many_arguments)]
fn turn(
    mut state: ResMut<EditBuild>,
    options: Res<crate::Options>,
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    motion: Res<AccumulatedMouseMotion>,
    wheel: Res<bevy::input::mouse::AccumulatedMouseScroll>,
    picture: Query<&Interaction, With<PreviewPicture>>,
    window: Query<&Window, With<PrimaryWindow>>,
    mut list: Query<&mut ScrollPosition, With<DropdownList>>,
) {
    if !state.open {
        return;
    }
    // The wheel scrolls the app's dropdown (a line a notch: the app's);
    // with none open it zooms.
    if state.menu.is_some() && wheel.delta.y != 0.0 {
        let s = window.single().map_or(1.0, |window| window.height() / HIGH);
        state.scroll = (state.scroll - wheel.delta.y * (CELL + 2.0) * s).max(0.0);
        for mut position in &mut list {
            position.0.y = state.scroll;
        }
    } else if wheel.delta.y != 0.0 {
        let notches = match wheel.unit {
            bevy::input::mouse::MouseScrollUnit::Line => wheel.delta.y,
            bevy::input::mouse::MouseScrollUnit::Pixel => wheel.delta.y / PIXELS_A_NOTCH,
        };
        state.zoom += notches * ZOOM_PER_NOTCH;
    }
    let cursor = window.single().ok().and_then(Window::cursor_position);
    if buttons.just_pressed(MouseButton::Left) && state.menu.is_none() && picture.iter().any(|interaction| *interaction == Interaction::Pressed) {
        state.pressed_at = Some(cursor.unwrap_or_default());
    }
    if !buttons.pressed(MouseButton::Left) {
        state.pressed_at = None;
        state.dragging = false;
    }
    if let (Some(at), false, Some(now)) = (state.pressed_at, state.dragging, cursor) {
        if now.distance(at) >= DRAG_THRESHOLD {
            state.dragging = true;
            state.cursor = Some(now);
        }
    }
    TURNING.store(state.dragging, std::sync::atomic::Ordering::Relaxed);
    let rotate = |state: &mut EditBuild, angle: f32, tilt: f32| {
        state.yaw -= angle;
        if state.yaw >= 360.0 {
            state.yaw -= 360.0;
        } else if state.yaw < -360.0 {
            state.yaw += 360.0;
        }
        let facing = if state.yaw.to_radians().cos() >= 0.0 { 1.0 } else { -1.0 };
        state.tilt = (state.tilt + tilt * facing).clamp(-30.0, 30.0);
    };
    if state.dragging {
        let delta = if options.unattended() {
            match (cursor, state.cursor) {
                (Some(now), Some(before)) => now - before,
                _ => Vec2::ZERO,
            }
        } else {
            motion.delta
        };
        if delta != Vec2::ZERO {
            rotate(&mut state, delta.x * TURN_PER_COUNT, delta.y * TURN_PER_COUNT);
        }
    }
    state.cursor = cursor;
    if keys.pressed(KeyCode::ArrowRight) {
        rotate(&mut state, ARROW_TURN * 0.3, 0.0);
    } else if keys.pressed(KeyCode::ArrowLeft) {
        rotate(&mut state, -ARROW_TURN * 0.3, 0.0);
    }
}

/// The turning parent's place and turn: Unity's `Euler(tilt, yaw, 0)` at
/// the preview's height.
pub(crate) fn rotator_place(scene: &trp_data::edit_build::EditBuildScene, yaw: f32, tilt: f32) -> Transform {
    let turn = Quat::from_rotation_y(yaw.to_radians()) * Quat::from_rotation_x(tilt.to_radians());
    Transform { translation: convert::position([0.0, scene.height, 0.0]), rotation: convert::rotation(turn.to_array()), scale: Vec3::ONE }
}

/// The preview: its camera and lights made once, its picture the window's
/// size; the build's model put in hidden once it is ready and shown on the
/// next frame, when its objects' places are had, the old one kept until
/// then; the turning parent turned. The camera draws only for a few frames
/// after something it sees changes, its picture kept otherwise.
#[allow(clippy::too_many_arguments)]
fn preview(
    mut commands: Commands,
    mut state: ResMut<EditBuild>,
    mut loose: ResMut<crate::loose::Loose>,
    mut images: ResMut<Assets<Image>>,
    window: Query<&Window, With<PrimaryWindow>>,
    mut transforms: Query<&mut Transform>,
    globals: Query<&GlobalTransform>,
    mut cameras: Query<&mut Camera>,
    meshes: Res<Assets<Mesh>>,
) {
    let state = &mut *state;
    if !state.open {
        // Shut: the models it asked for let go of, and the reader with them.
        for key in std::mem::take(&mut state.released) {
            loose.let_go(&key);
        }
        if loose.keep_reader {
            loose.keep_reader = false;
        }
        return;
    }
    // Open, the models' reader is kept between picks.
    if !loose.keep_reader {
        loose.keep_reader = true;
    }
    let (Some(scene), Ok(window), Some(build)) = (state.scene.clone(), window.single(), state.build.as_ref()) else { return };
    let layer = RenderLayers::layer(LAYER);
    let size = UVec2::new(window.physical_width().max(1), window.physical_height().max(1));
    if state.preview.camera.is_none() {
        let image = target(&mut images, size);
        // The camera as the scene's (`Weapon Camera`): its field of view
        // and planes; under the turning parent's place, a metre behind it,
        // looking along it (`ResetRotator(-1)`); drawing the preview's
        // layer at an exposure at which a light's intensity is the game's
        // (as the icons' studio), onto nothing, so that the screen's
        // ground shows round the weapon.
        let camera = commands
            .spawn((
                Name::new("Edit build preview's camera"),
                Camera3d::default(),
                Camera { order: -4, clear_color: ClearColorConfig::Custom(Color::NONE), ..default() },
                RenderTarget::Image(image.clone().into()),
                AmbientLight { color: Color::WHITE, brightness: 0.0, affects_lightmapped_meshes: false },
                Exposure { ev100: 0.0 },
                Projection::Perspective(PerspectiveProjection { fov: scene.fov.to_radians(), near: scene.near, far: scene.far, ..default() }),
                Transform::from_translation(convert::position([0.0, scene.height, CAMERA_START])),
                layer.clone(),
            ))
            .id();
        // The inspect window's two lights and photographic tonemap (the
        // user, 2026-10-24: "Can we give the workbench model the same lights
        // as the inspect menu?", chosen over the screen's own thirteen and
        // its tonemap at 2.5 with its contrast: `decisions.md`); the
        // screen's own where that preview could not be read.
        match state.inspect.clone() {
            Some(inspect) => {
                state.preview.lights = crate::info_preview::inspect_lights(&mut commands, &inspect, &layer);
                if let Some(exposure) = inspect.exposure {
                    commands.entity(camera).insert((bevy::render::view::Hdr, bevy::core_pipeline::tonemapping::Tonemapping::None, crate::preview_tonemap::PhotographicTonemap::of(exposure)));
                }
            }
            None => state.preview.lights = preview_lights(&mut commands, &scene, &layer),
        }
        let rotator = commands.spawn((Name::new("Edit build preview"), rotator_place(&scene, state.yaw, state.tilt), Visibility::Inherited, layer.clone())).id();
        (state.preview.camera, state.preview.image, state.preview.size, state.preview.rotator) = (Some(camera), Some(image), size, Some(rotator));
        state.preview.camera_z = CAMERA_START;
        state.zoom = 0.0;
        state.preview.frames = DRAWN_MODEL;
        return;
    }
    // The picture made again for the window's size.
    if state.preview.size != size {
        let image = target(&mut images, size);
        if let Some(camera) = state.preview.camera {
            commands.entity(camera).insert(RenderTarget::Image(image.clone().into()));
        }
        if let Some(old) = state.preview.image.replace(image) {
            images.remove(&old);
        }
        state.preview.size = size;
        state.preview.frames = state.preview.frames.max(DRAWN_TURNED);
    }
    let Some(rotator) = state.preview.rotator else { return };
    if let Ok(mut place) = transforms.get_mut(rotator) {
        let wanted = rotator_place(&scene, state.yaw, state.tilt);
        if place.rotation != wanted.rotation {
            place.rotation = wanted.rotation;
            state.preview.frames = state.preview.frames.max(DRAWN_TURNED);
        }
    }
    // The model made last frame: its objects' places are had, so each
    // slot's bone is found in it and it is shown in the old one's place,
    // and its slots are made (`RefreshWeapon`: `CreateModSlotViews`).
    if let Some(model) = state.preview.pending.take() {
        let turned = globals.get(rotator).map(|global| global.affine().inverse()).unwrap_or(Affine3A::IDENTITY);
        let found = bones(&model.build, &model.put.names, &model.put.parents);
        state.preview.bones = found.into_iter().filter_map(|(slot, node)| Some((slot, turned * globals.get(*model.put.nodes.get(node)?).ok()?.affine()))).collect();
        state.preview.bounds = model_bounds(&model.put, &meshes, &globals);
        if let Some(old) = state.preview.holder.replace(model.put.holder) {
            commands.entity(old).despawn();
        }
        commands.entity(model.put.holder).insert(Visibility::Inherited);
        // (The model shown before, no longer wanted here.)
        if let Some(old) = state.preview.key.as_ref().filter(|old| **old != model.key) {
            loose.let_go(old);
        }
        state.preview.key = Some(model.key);
        state.preview.model += 1;
        state.shown = Some(model.build);
        state.menu = None;
        state.menu_seen = false;
        state.preview.frames = DRAWN_MODEL;
    }
    // The build's model, by its key, once ready: put in hidden.
    if state.preview.key.as_deref() != Some(state.build_key.as_str()) {
        let key = state.build_key.clone();
        // A model asked for and no longer the build's (a pick made again
        // before it came) is let go of (`SetupItemPreview` cancels the wait
        // for the one before).
        if let Some(stale) = state.preview.asked.replace(key.clone()).filter(|asked| *asked != key && state.preview.key.as_ref() != Some(asked)) {
            loose.let_go(&stale);
        }
        if let Some(ready) = loose.model_for(&key) {
            // The pivot turned (0, −90, 0) and scaled by the `PreviewPivot`,
            // the container at minus its pivot, the weapon's object at its
            // place in the container (`PositionGameObject`); with no
            // `PreviewPivot`, about the model's own first object
            // (`PROVISIONAL`: the game's bounds' middle).
            // The pivot turned (0, −90, 0) (`ItemObserveScreen`).
            let place = state.places.get(&build.kind.template).copied().flatten();
            info!("the edit build screen's model {key}: placed by {place:?}; {} objects", ready.nodes.len());
            let pivot_turn = convert::rotation(Quat::from_rotation_y((-90.0f32).to_radians()).to_array());
            let put = put_model(&mut commands, &ready, rotator, &layer, pivot_turn, place);
            state.preview.pending = Some(Model { put, key, build: build.clone() });
        }
    }
    // The wheel's zoom, once the model's bounds are had (`Zoom`: none
    // before).
    let zoom = std::mem::take(&mut state.zoom);
    if let (true, Some(bounds), Some(camera)) = (zoom != 0.0, state.preview.bounds, state.preview.camera) {
        let aspect = size.x as f32 / size.y.max(1) as f32;
        let z = zoomed(&scene, state.preview.camera_z, zoom, aspect, bounds);
        if z != state.preview.camera_z {
            state.preview.camera_z = z;
            if let Ok(mut place) = transforms.get_mut(camera) {
                place.translation = convert::position([0.0, scene.height, z]);
            }
            state.preview.frames = state.preview.frames.max(DRAWN_TURNED);
        }
    }
    if let Some(Ok(mut camera)) = state.preview.camera.map(|camera| cameras.get_mut(camera)) {
        let drawing = state.preview.frames > 0;
        if camera.is_active != drawing {
            camera.is_active = drawing;
        }
    }
    state.preview.frames = state.preview.frames.saturating_sub(1);
}

pub(crate) fn target(images: &mut Assets<Image>, size: UVec2) -> Handle<Image> {
    let mut target = Image::new_uninit(Extent3d { width: size.x, height: size.y, depth_or_array_layers: 1 }, TextureDimension::D2, TextureFormat::Rgba8UnormSrgb, RenderAssetUsages::default());
    target.texture_descriptor.usage = TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::RENDER_ATTACHMENT;
    images.add(target)
}

/// Each slot's bone in the model (`ContainerCollectionView.GetBoneForSlot`):
/// the object named after the slot in its item's own model (searched from
/// the item's top, not into another part's model), by its place in the
/// model; a part's top is the last object hung from its slot's bone.
fn bones(build: &Item, names: &[String], parents: &[Option<usize>]) -> HashMap<SlotId, usize> {
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); parents.len()];
    for (child, parent) in parents.iter().enumerate() {
        if let Some(parent) = parent.filter(|&parent| parent < children.len()) {
            children[parent].push(child);
        }
    }
    let find = |top: usize, name: &str| -> Option<usize> {
        let mut queue = std::collections::VecDeque::from([top]);
        while let Some(node) = queue.pop_front() {
            if node != top && names[node] == name {
                return Some(node);
            }
            // (Not into another part's model, which hangs from a bone.)
            if node == top || !names[node].starts_with("mod_") {
                queue.extend(children[node].iter().copied());
            }
        }
        None
    };
    let mut out = HashMap::new();
    fn walk(item: &Item, top: usize, find: &dyn Fn(usize, &str) -> Option<usize>, children: &[Vec<usize>], out: &mut HashMap<SlotId, usize>) {
        for (index, (holder, part)) in item.kind.slots.iter().zip(&item.slots).enumerate() {
            let Some(bone) = find(top, &holder.name) else { continue };
            out.insert((item.id, index), bone);
            if let Some(part) = part.as_ref().filter(|part| is(part, "Mod")) {
                if let Some(&part_top) = children[bone].iter().max() {
                    walk(part, part_top, find, children, out);
                }
            }
        }
    }
    if !names.is_empty() {
        walk(build, 0, &find, &children, &mut out);
    }
    out
}

/// The slots shown, by the toggles, of the build the preview shows; and
/// where each lies this frame: its bone through the preview's camera onto
/// the screen, and its view on the ellipse (`UpdatePositions`), in the
/// screen's units from its top left. Worked out only when the turn, the
/// model, the window or the slots change.
fn place(mut state: ResMut<EditBuild>, window: Query<&Window, With<PrimaryWindow>>, cameras: Query<(&Camera, &GlobalTransform)>) {
    let state = &mut *state;
    if !state.open {
        return;
    }
    let (Ok(window), Some(scene)) = (window.single(), state.scene.clone()) else { return };
    // The slots shown (`CheckVisibility`): a dropdown whose slot is hidden
    // is shut. None while the model of a build just changed is made: the
    // game takes every slot's view away as a part is picked
    // (`RefreshWeapon`: `ClearModSlots`) and makes them again once the new
    // model is in (`CreateModSlotViews`), so nothing of the build before
    // can be clicked meanwhile.
    let refreshing = state.preview.key.as_deref() != Some(state.build_key.as_str());
    if state.slots_for != Some((state.preview.model, state.toggles, refreshing)) {
        state.slots_for = Some((state.preview.model, state.toggles, refreshing));
        let toggles = state.toggles;
        state.slots = if refreshing { Vec::new() } else { state.shown.as_ref().map(|shown| rules::build_slots(shown).into_iter().filter(|slot| toggles[class_toggle(slot.class)]).collect()).unwrap_or_default() };
        if state.menu.is_some_and(|menu| !state.slots.iter().any(|slot| (slot.owner, slot.index) == menu)) {
            state.menu = None;
        }
        state.slots_made += 1;
    }
    let size = UVec2::new(window.physical_width(), window.physical_height());
    let wanted = (state.yaw, state.tilt, state.preview.model, size, state.slots_made, state.preview.camera_z);
    if state.placed_for == Some(wanted) {
        return;
    }
    let Some(Ok((camera, _))) = state.preview.camera.map(|camera| cameras.get(camera)) else { return };
    // (The camera where it stands this frame: its own place is moved by
    // the zoom only as the frame ends.)
    let eye = GlobalTransform::from(Transform::from_translation(convert::position([0.0, scene.height, state.preview.camera_z])));
    let eye = &eye;
    let s = window.height() / HIGH;
    let wide = window.width() / s;
    let turned = rotator_place(&scene, state.yaw, state.tilt).compute_affine();
    let mut points: Vec<(usize, [f32; 2])> = Vec::new();
    for (at, slot) in state.slots.iter().enumerate() {
        let Some(bone) = state.preview.bones.get(&(slot.owner, slot.index)) else { continue };
        let world = (turned * *bone).translation;
        let Ok(pixel) = camera.world_to_viewport(eye, Vec3::from(world)) else { continue };
        // (The picture spans the window: its pixels to the screen's units;
        // the game's from the panel's bottom left.)
        let units = pixel * (HIGH / state.preview.size.y.max(1) as f32);
        points.push((at, [units.x, HIGH - units.y]));
    }
    // A camera not yet worked out places nothing: tried again next frame.
    if points.is_empty() && !state.slots.is_empty() && !state.preview.bones.is_empty() {
        return;
    }
    state.placed_for = Some(wanted);
    let ellipse = scene.panel.find("Ellipse Rect").map(|index| scene.panel.rect(index, [wide, HIGH])).unwrap_or([0.0, 0.0, wide * 0.6, 650.0]);
    let (a, b) = (ellipse[2] / 2.0, ellipse[3] / 2.0);
    let centre = [wide / 2.0, HIGH / 2.0];
    let views = rules::place_slots(&points.iter().map(|(_, p)| *p).collect::<Vec<_>>(), centre, a.max(b), b.min(a));
    let mut placed = vec![None; state.slots.len()];
    for ((at, bone), view) in points.iter().zip(views) {
        placed[*at] = Some(([bone[0], HIGH - bone[1]], [view[0], HIGH - view[1]]));
    }
    state.placed = placed;
}

/// Which toggle shows a class of slot.
fn class_toggle(class: ModClass) -> usize {
    match class {
        ModClass::Vital => 0,
        ModClass::Functional => 1,
        ModClass::Gear => 2,
    }
}

fn class_color(slot: &BuildSlot) -> Color {
    if slot.missing {
        return MISSING;
    }
    match slot.class {
        ModClass::Functional => FUNCTIONAL,
        ModClass::Vital => VITAL,
        ModClass::Gear => GEAR,
    }
}

/// A slot's bone's icon (`UpdateInfo`): its class's, full or empty.
fn bone_icon(slot: &BuildSlot) -> &'static str {
    match (slot.class, slot.part.is_some()) {
        (ModClass::Functional, true) => "icon_mod_tactical_big_installed",
        (ModClass::Functional, false) => "icon_mod_tactical_big_nomod",
        (ModClass::Vital, true) => "icon_mod_vital_big_installed",
        (ModClass::Vital, false) => "icon_mod_vital_big_nomod",
        (ModClass::Gear, true) => "icon_mod_gear_big_installed",
        (ModClass::Gear, false) => "icon_mod_gear_big_nomod",
    }
}

/// A slot's name as its bone shows it (`GetModSlotName`, the install's
/// words, upper case).
fn slot_said(texts: &HashMap<String, String>, slot: &BuildSlot) -> String {
    let key = rules::slot_words(&slot.name);
    texts.get(&key.to_lowercase()).cloned().unwrap_or_else(|| key.to_owned()).to_uppercase()
}

/// The marks of the kinds of part a slot takes (`SetFilters`): of each
/// kind its filters name (an item's own kind, `GetFilterTypes`) and its
/// excluded ones do not, its mark where it has one (`LoadModIconSprite`),
/// once each, in the filters' order.
pub(crate) fn filter_marks(db: &trp_data::Database, holder: &trp_data::inventory::Holder) -> Vec<&'static str> {
    let kind_of = |id: &String| -> Option<String> {
        let item = db.items.get(id)?;
        if item.node_type == "Node" { Some(id.clone()) } else { Some(item.parent.clone()) }
    };
    let refused: Vec<String> = holder.refuses.iter().filter_map(kind_of).collect();
    let mut marks: Vec<&'static str> = Vec::new();
    for kind in holder.takes.iter().filter_map(kind_of).filter(|kind| !refused.contains(kind)) {
        let names = db.class_names(&kind);
        let Some((_, mark)) = crate::health_view::MOD_KINDS.iter().find(|(class, _)| names.iter().any(|name| name == class)) else { continue };
        if !marks.contains(mark) {
            marks.push(mark);
        }
    }
    marks
}

/// The game's screen made ready to draw: what its pictures and texts ask
/// of its layout, its texts' words in the install's language.
fn prepare(eb: &EditBuildLook, look: &HealthLook, said: &Words) -> Prepared {
    let mut screen = eb.scene.panel.clone();
    let localized = |panel: &ScenePanel| -> HashMap<usize, String> {
        (0..panel.nodes.len()).filter_map(|index| trp_data::screen::localized(panel, index).map(|(key, case)| (index, trp_data::screen::in_case(said.word(&key, &key), case)))).collect()
    };
    let words = localized(&screen);
    let empty_words = localized(&eb.empty);
    look.ask_by_content(&mut screen, &eb.screen, &words);
    let find = |path: &str| screen.find(path);
    let toggles = ["VitalParts", "FunctionalMods", "EquipmentParts", "OnlyAvailable"];
    let buttons = ["NewBuildButton", "OpenBuildButton", "SaveAsBuildButton", "SelectItemButton", "FindPartsButton", "PublishBuildtButton", "DeleteBuildButton"].iter().filter_map(|name| find(&format!("ButtonsPanel/ContextButtons/{name}"))).collect();
    let slot = &eb.slot;
    let menu_item = slot.find("modding_menu_item");
    let (item_rect, menu_anchor) = match (menu_item, slot.find("modding_menu_item/CanvasGroup/Menu Anchor")) {
        (Some(item), anchor) => {
            // About the view's pivot: where the slot's place on the ellipse
            // puts it (`SetSlotScreenPosition`).
            let rect = slot.rect(item, [0.0, 0.0]);
            let pivot = slot.nodes[item].layout.pivot;
            let at = [rect[0] + pivot[0] * rect[2], rect[1] + (1.0 - pivot[1]) * rect[3]];
            let anchor = anchor.map_or([rect[0] - at[0], rect[3]], |anchor| {
                let a = slot.rect(anchor, [0.0, 0.0]);
                [a[0] - at[0], a[1] - at[1]]
            });
            ([rect[0] - at[0], rect[1] - at[1], rect[2], rect[3]], anchor)
        }
        (None, _) => ([-42.5, -64.0, 85.0, 64.0], [-47.5, 8.0]),
    };
    Prepared {
        stats: find("CharacteristicsPanel"),
        sub_caption: find("Sub-caption"),
        loader: find(crate::health_view::EB_LOADER),
        marks: toggles.map(|name| find(&format!("Toggle Group/{name}/Checkmark/Mark"))),
        toggle: toggles.map(|name| find(&format!("Toggle Group/{name}"))),
        buttons,
        save: find("ButtonsPanel/BottomButtons/Save"),
        assemble: find("ButtonsPanel/BottomButtons/Assemble"),
        assemble_button: find("ButtonsPanel/BottomButtons/Assemble/Button"),
        ready: find(crate::health_view::EB_READY),
        ready_button: find(&format!("{}/Button", crate::health_view::EB_READY)),
        back: find("BackButton"),
        dropdown: find(crate::health_view::EB_DROPDOWN),
        none_holder: find(&format!("{}/ItemViewContainer", crate::health_view::EB_DROPDOWN)),
        grid: find(&format!("{}/Container", crate::health_view::EB_DROPDOWN)),
        caption: ["Caption", "Sub-caption"].iter().filter_map(|path| find(path)).collect(),
        preview_panel: find("Preview Panel"),
        menu_item,
        item_rect,
        menu_anchor,
        bone_icon: slot.find("Bone Icon"),
        bone_panel: slot.find(crate::health_view::EB_SLOT_NAME),
        bone_back: slot.find(&format!("{}/Background", crate::health_view::EB_SLOT_NAME)),
        bone_name: slot.find(&format!("{}/Name", crate::health_view::EB_SLOT_NAME)),
        slot_button: slot.find(crate::health_view::EB_SLOT_BUTTON),
        slot_fade: slot.find(crate::health_view::EB_SLOT_FADE),
        slot_debug: slot.find("modding_menu_item/CanvasGroup/__DEBUG TEXT"),
        screen,
        words,
        empty_words,
    }
}

/// A full-screen layer of the screen, which takes no clicks itself.
fn layer() -> Node {
    Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() }
}

/// A node of that rectangle (the screen's units), at `s` pixels a unit.
fn rect_node(rect: [f32; 4], s: f32) -> Node {
    Node { position_type: PositionType::Absolute, left: Val::Px(rect[0] * s), top: Val::Px(rect[1] * s), width: Val::Px(rect[2] * s), height: Val::Px(rect[3] * s), ..default() }
}

/// A point's node, which what it holds hangs about (the screen's units).
fn point_node(at: [f32; 2], s: f32) -> Node {
    Node { position_type: PositionType::Absolute, left: Val::Px(at[0] * s), top: Val::Px(at[1] * s), width: Val::Px(0.0), height: Val::Px(0.0), ..default() }
}

/// The screen drawn: its layers made once, each drawn again only as what
/// it shows changes; the slots' nodes placed every frame they move; the
/// loader shown while the preview's model is made.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn draw(
    mut commands: Commands,
    mut state: ResMut<EditBuild>,
    carried: Res<Carried>,
    look: Res<HealthLook>,
    said: Res<Words>,
    game: Res<crate::GameData>,
    mut icons: ResMut<crate::icons::Icons>,
    window: Query<&Window, With<PrimaryWindow>>,
    time: Res<Time>,
    hovers: Query<(&SlotHover, &RelativeCursorPosition)>,
    mut placed: Query<(&SlotPiece, &mut Node, &mut UiTransform, &mut Visibility), (Without<Piece>, Without<AppLoader>)>,
    mut pieces: Query<(&Piece, &mut UiTransform, &mut Visibility), (Without<SlotPiece>, Without<AppLoader>)>,
    mut app_loader: Query<(&mut UiTransform, &mut Visibility), (With<AppLoader>, Without<SlotPiece>, Without<Piece>)>,
    mut picture: Query<&mut ImageNode, With<PreviewPicture>>,
    mut stats_images: Query<(&Piece, &mut ImageNode), Without<PreviewPicture>>,
) {
    let state = &mut *state;
    if !state.open {
        return;
    }
    let Ok(window) = window.single() else { return };
    let s = window.height() / HIGH;
    let wide = window.width() / s;
    let Some(build) = state.build.as_ref() else { return };
    let eb = look.edit_build();
    if state.prepared.is_none() {
        if let Some(eb) = eb {
            state.prepared = Some(prepare(eb, &look, &said));
        }
    }
    let game_look = eb.zip(state.prepared.as_ref());
    if state.notice.as_ref().is_some_and(|(_, until)| time.elapsed_secs() > *until) {
        state.notice = None;
    }
    // The layers, made once; all drawn again for another size.
    if state.drawn.root.is_none() {
        let root = commands.spawn((ScreenRoot, layer(), BackgroundColor(BEHIND.with_alpha(BEHIND_ALPHA)), GlobalZIndex(40))).id();
        let picture = commands.spawn((PreviewPicture, Button, ImageNode::default(), layer(), ChildOf(root))).id();
        let mut made = || commands.spawn((layer(), ChildOf(root))).id();
        let (under, lines, slots, stats, dropdown, over, notice) = (made(), made(), made(), made(), made(), made(), made());
        state.drawn = Drawn { root: Some(root), picture: Some(picture), under: Some(under), lines: Some(lines), slots: Some(slots), stats: Some(stats), dropdown: Some(dropdown), over: Some(over), notice: Some(notice), ..Drawn::default() };
    }
    if state.drawn.scale != Some((s, wide)) {
        state.drawn.scale = Some((s, wide));
        for layer in [state.drawn.under, state.drawn.lines, state.drawn.slots, state.drawn.stats, state.drawn.dropdown, state.drawn.over, state.drawn.notice].into_iter().flatten() {
            commands.entity(layer).despawn_related::<Children>();
        }
        (state.drawn.statics, state.drawn.statics_for, state.drawn.slots_for, state.drawn.dropdown_for, state.drawn.notice_for, state.drawn.stats_for) = (None, None, None, None, None, None);
        state.drawn.boxes.clear();
    }
    if let (Some(image), Some(entity)) = (state.preview.image.clone(), state.drawn.picture) {
        if let Ok(mut node) = picture.get_mut(entity) {
            if node.image != image {
                node.image = image;
            }
        }
    }
    // The slot the mouse is on: the one drawn last of those under it.
    let hovered = state.drawn.boxes.iter().rev().find_map(|nodes| hovers.iter().find(|(hover, at)| hover.0 == nodes.id && at.cursor_over()).map(|(hover, _)| hover.0));
    state.hovered = hovered;
    // The screen's own objects, worked out again as the build, the weapon
    // or the toggles change.
    let statics_for = (state.made, state.toggles);
    let statics = (state.drawn.statics_for != Some(statics_for)).then(|| {
        let name = carried.texts.names.get(&build.kind.template).cloned().unwrap_or_else(|| build.kind.name.clone());
        let ready = state.weapon.and_then(|id| carried.equipment.item.find(id)).is_some_and(|weapon| !rules::already_built(weapon, build)) && !vital_missing(build);
        Statics { name, toggles: state.toggles, ready }
    });
    state.drawn.statics_for = Some(statics_for);
    if let Some(statics) = statics.filter(|statics| state.drawn.statics.as_ref() != Some(statics)) {
        let (Some(under), Some(over)) = (state.drawn.under, state.drawn.over) else { return };
        commands.entity(under).despawn_related::<Children>();
        commands.entity(over).despawn_related::<Children>();
        match game_look {
            Some((eb, prepared)) => {
                commands.entity(under).with_children(|under| statics_under(under, &look, eb, prepared, &statics, &said, s, wide));
                commands.entity(over).with_children(|over| statics_over(over, &look, eb, prepared, &statics, &said, s, wide));
            }
            None => {
                commands.entity(over).with_children(|over| statics_app(over, &statics, s, wide));
            }
        }
        state.drawn.statics = Some(statics);
    }
    // The loader while the preview's model is made (`RefreshingWeapon`),
    // turning.
    let refreshing = state.preview.key.as_deref() != Some(state.build_key.as_str());
    let shown = if refreshing { Visibility::Inherited } else { Visibility::Hidden };
    let spin = Rot2::radians((time.elapsed_secs() * LOADER_TURNS).fract() * std::f32::consts::TAU);
    if let Some(loader) = game_look.and_then(|(_, prepared)| prepared.loader) {
        for (piece, mut turn, mut visibility) in &mut pieces {
            if piece.0 != EB_SCREEN + loader {
                continue;
            }
            if *visibility != shown {
                *visibility = shown;
            }
            if refreshing {
                turn.rotation = spin;
            }
        }
    }
    for (mut turn, mut visibility) in &mut app_loader {
        if *visibility != shown {
            *visibility = shown;
        }
        if refreshing {
            turn.rotation = spin;
        }
    }
    // The slots: all made again when the slots shown change, each at its
    // place from the first frame; a view drawn again as the mouse comes
    // onto it or its dropdown opens.
    if state.drawn.slots_for != Some(state.slots_made) {
        state.drawn.slots_for = Some(state.slots_made);
        let (Some(lines), Some(slots)) = (state.drawn.lines, state.drawn.slots) else { return };
        commands.entity(lines).despawn_related::<Children>();
        commands.entity(slots).despawn_related::<Children>();
        let mut boxes = Vec::with_capacity(state.slots.len());
        for (at, slot) in state.slots.iter().enumerate() {
            let place = state.placed.get(at).copied().flatten();
            let visibility = if place.is_some() { Visibility::Inherited } else { Visibility::Hidden };
            let (bone_at, view_at) = place.unwrap_or(([0.0, 0.0], [0.0, 0.0]));
            let (line_node, line_turn) = line_place(bone_at, view_at, s);
            commands.spawn((SlotPiece::Line(at), line_node, line_turn, UNROUNDED, BackgroundColor(class_color(slot)), visibility, ChildOf(lines)));
            let own = commands.spawn((layer(), ChildOf(slots))).id();
            let bone = commands.spawn((SlotPiece::Bone(at), point_node(bone_at, s), visibility, ChildOf(own))).id();
            let view = commands.spawn((SlotPiece::View(at), point_node(view_at, s), visibility, ChildOf(own))).id();
            let id = (slot.owner, slot.index);
            let shown = BoxShown { button: false, open: !state.menu_seen };
            let marks = slot_marks(&game, build, slot, &mut state.filters);
            commands.entity(bone).with_children(|bone| match game_look {
                Some((eb, prepared)) => bone_game(bone, &look, eb, prepared, slot, &state.texts, &said, s),
                None => bone_app(bone, slot, &state.texts, s),
            });
            commands.entity(view).with_children(|view| match game_look {
                Some((eb, prepared)) => view_game(view, &look, eb, prepared, &carried, &mut icons, state.shown.as_ref(), slot, shown, &marks, &said, s),
                None => view_app(view, &carried, &mut icons, state.shown.as_ref(), slot, s),
            });
            boxes.push(SlotNodes { id, own, view, shown });
        }
        state.drawn.boxes = boxes;
    } else {
        for at in 0..state.drawn.boxes.len() {
            let Some(slot) = state.slots.get(at) else { continue };
            let id = state.drawn.boxes[at].id;
            let open = state.menu == Some(id) || !state.menu_seen;
            let wanted = BoxShown { button: hovered == Some(id) || state.menu == Some(id), open };
            if state.drawn.boxes[at].shown == wanted {
                continue;
            }
            let nodes = &mut state.drawn.boxes[at];
            nodes.shown = wanted;
            let (view, own) = (nodes.view, nodes.own);
            // (Raised over the others while the mouse is on it.)
            commands.entity(own).insert(ZIndex(if wanted.button { 1 } else { 0 }));
            commands.entity(view).despawn_related::<Children>();
            let marks = slot_marks(&game, build, slot, &mut state.filters);
            commands.entity(view).with_children(|view| match game_look {
                Some((eb, prepared)) => view_game(view, &look, eb, prepared, &carried, &mut icons, state.shown.as_ref(), slot, wanted, &marks, &said, s),
                None => view_app(view, &carried, &mut icons, state.shown.as_ref(), slot, s),
            });
        }
    }
    // Every frame: the slots' nodes where they lie, written only where they
    // have moved.
    for (piece, mut node, mut turn, mut visibility) in &mut placed {
        let at = match *piece {
            SlotPiece::Line(at) | SlotPiece::Bone(at) | SlotPiece::View(at) => at,
        };
        let Some(Some((bone_at, view_at))) = state.placed.get(at).copied() else {
            if *visibility != Visibility::Hidden {
                *visibility = Visibility::Hidden;
            }
            continue;
        };
        if *visibility != Visibility::Inherited {
            *visibility = Visibility::Inherited;
        }
        let (wanted, rotation) = match *piece {
            SlotPiece::Line(_) => {
                let (node, turned) = line_place(bone_at, view_at, s);
                (node, turned.rotation)
            }
            SlotPiece::Bone(_) => (point_node(bone_at, s), Rot2::IDENTITY),
            SlotPiece::View(_) => (point_node(view_at, s), Rot2::IDENTITY),
        };
        if node.left != wanted.left || node.top != wanted.top || node.width != wanted.width || node.height != wanted.height {
            (node.left, node.top, node.width, node.height) = (wanted.left, wanted.top, wanted.width, wanted.height);
        }
        if turn.rotation != rotation {
            turn.rotation = rotation;
        }
    }
    // The dropdown of a slot: every part of the game it takes but the one
    // in it, and the view that empties it (`DropDownMenu.Show`).
    if state.drawn.dropdown_for != Some(state.menu) {
        state.drawn.dropdown_for = Some(state.menu);
        let Some(layer_entity) = state.drawn.dropdown else { return };
        commands.entity(layer_entity).despawn_related::<Children>();
        let found = state.menu.and_then(|menu| state.slots.iter().position(|slot| (slot.owner, slot.index) == menu));
        if let (Some(at), Some(parts)) = (found, state.parts.clone()) {
            let slot = state.slots[at].clone();
            let current = slot.part.and_then(|id| build.find(id)).map(|part| part.kind.template.clone());
            let offered: Vec<Arc<Kind>> = parts.iter().filter(|kind| Some(&kind.template) != current.as_ref() && rules::fits(build, slot.owner, slot.index, kind)).cloned().collect();
            let view_at = state.placed.get(at).copied().flatten().map_or([wide / 2.0, HIGH / 2.0], |(_, view)| view);
            state.scroll = 0.0;
            commands.entity(layer_entity).with_children(|layer| match game_look {
                Some((eb, prepared)) => dropdown_game(layer, &look, eb, prepared, &carried, &mut icons, (slot.owner, slot.index), &offered, view_at, &said, s, wide),
                None => dropdown_app(layer, &carried, &mut icons, (slot.owner, slot.index), &offered, view_at, s, wide),
            });
        }
    }
    // The characteristics panel at the bottom left (`CharacteristicsPanel.
    // Show(Item, _compareItem)`, at every change of the build and of what
    // it is compared with), its bars' comparison pulsing.
    let stats_for = (state.made, state.compare_made, state.stats_folded);
    if state.drawn.stats_for != Some(stats_for) {
        state.drawn.stats_for = Some(stats_for);
        if let Some(layer_entity) = state.drawn.stats {
            commands.entity(layer_entity).despawn_related::<Children>();
            state.drawn.pulse.clear();
            let say = |key: &str| carried.texts.say(key);
            let figures = trp_sim::item_attributes::figures(build, &game.db, &say);
            let compare_item = state.compare.as_ref().unwrap_or(build);
            let compared = trp_sim::item_attributes::figures(compare_item, &game.db, &say);
            let weight_name = say("Weight").unwrap_or_else(|| "Weight".to_owned()).to_uppercase();
            let weights = (build.weight(), compare_item.weight());
            let built = game_look.and_then(|(_, prepared)| crate::health_view::eb_stats_built(&look, &figures, &compared, weights, &weight_name, !state.stats_folded).zip(prepared.stats));
            match (built, game_look) {
                (Some((built, top)), Some((_, prepared))) => {
                    // Where the panel lies on the screen: its own anchors
                    // and pivot, at the size its rows make it.
                    let mut screen = prepared.screen.clone();
                    screen.nodes[top].layout.size_delta = built.size;
                    let [x, y, ..] = screen.rect(top, [wide, HIGH]);
                    commands.entity(layer_entity).with_children(|layer| {
                        crate::health_view::draw_eb_stats(layer, &look, &said, &built, [x, y], s);
                        for [sx, sy, sw, sh] in &built.switches {
                            layer.spawn((Button, Press::Info, rect_node([x + sx, y + sy, *sw, *sh], s)));
                        }
                    });
                    state.drawn.pulse = built.pulse.clone();
                }
                _ => {
                    commands.entity(layer_entity).with_children(|layer| stats_app(layer, &figures, &compared, weights, &weight_name, state.stats_folded, s));
                }
            }
        }
    }
    if !state.drawn.pulse.is_empty() {
        let alpha = crate::health_view::compare_alpha(time.elapsed_secs());
        for (piece, mut image) in &mut stats_images {
            if let Some((_, color)) = state.drawn.pulse.iter().find(|(index, _)| crate::health_view::EB_STATS + index == piece.0) {
                let wanted = color.with_alpha(alpha);
                if image.color != wanted {
                    image.color = wanted;
                }
            }
        }
    }
    // The notice (`NotificationManager`), over the bottom's middle.
    let notice = state.notice.as_ref().map(|(said, _)| said.clone());
    if state.drawn.notice_for.as_ref() != Some(&notice) {
        let Some(layer_entity) = state.drawn.notice else { return };
        commands.entity(layer_entity).despawn_related::<Children>();
        if let Some(said) = &notice {
            let row = Node { position_type: PositionType::Absolute, left: Val::Px(0.0), width: Val::Percent(100.0), top: Val::Px(1000.0 * s), justify_content: JustifyContent::Center, ..default() };
            commands.entity(layer_entity).with_children(|layer| {
                layer.spawn(row).with_children(|row| {
                    row.spawn((Node { padding: UiRect::axes(Val::Px(20.0 * s), Val::Px(8.0 * s)), ..default() }, BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.8)))).with_children(|plate| {
                        plate.spawn(words(said.clone(), 18.0 * s, INK));
                    });
                });
            });
        }
        state.drawn.notice_for = Some(notice);
    }
}

/// The characteristics in the app's own look: at the bottom left, a line
/// each (the durability, the weight, then the figures' rows), a difference
/// from the build compared with in brackets after a bar's figure; folded to
/// its heading, which folds and unfolds it.
fn stats_app(layer: &mut ChildSpawnerCommands, figures: &trp_sim::item_attributes::Figures, compared: &trp_sim::item_attributes::Figures, (weight, compare_weight): (f32, f32), weight_name: &str, folded: bool, s: f32) {
    use trp_sim::item_attributes::Show;
    let size = 13.0 * s;
    let column = Node { position_type: PositionType::Absolute, left: Val::Px(30.0 * s), bottom: Val::Px(40.0 * s), flex_direction: FlexDirection::Column, row_gap: Val::Px(3.0 * s), padding: UiRect::all(Val::Px(8.0 * s)), ..default() };
    layer.spawn((column, BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)))).with_children(|column| {
        let heading = if folded { "INFO  >" } else { "INFO  <" };
        column.spawn((Button, Press::Info, Node::default())).with_children(|button| {
            button.spawn(words(heading.to_owned(), size, INK));
        });
        if folded {
            return;
        }
        let mut lines = Vec::new();
        if let Some(durability) = figures.durability {
            lines.push(format!("{}   {:.0}/{:.0} ({:.0})", figures.durability_name, durability.now, durability.most, durability.peak));
        }
        let difference = compare_weight - weight;
        lines.push(if difference.abs() > 1e-4 { format!("{weight_name}   {weight:.3} ({}{difference:.3})", if difference > 0.0 { "+" } else { "" }) } else { format!("{weight_name}   {weight:.3}") });
        for row in &figures.rows {
            let mut value = row.value.clone().unwrap_or_default();
            if let Show::Bar { base, delta, .. } = row.show {
                let other = compared.rows.iter().find(|other| other.id == row.id).and_then(|other| match other.show {
                    Show::Bar { base, delta, .. } => Some(base * delta),
                    _ => None,
                });
                if let Some(more) = other.map(|change| change - base * delta).filter(|more| more.abs() > f32::EPSILON) {
                    value.push_str(&format!(" ({}{})", if more > 0.0 { "+" } else { "" }, more.round() as i64));
                }
            }
            lines.push(if value.is_empty() { row.name.clone() } else { format!("{}   {value}", row.name) });
        }
        for line in lines {
            column.spawn(words(line, size, INK));
        }
    });
}

/// A slot's line from its bone to its view (`LineRenderer`): a node as long
/// as the line, laid along it, at least a pixel thick and not rounded to
/// the screen's pixels (rounded, a line thinner than a pixel came out none
/// at some places).
fn line_place(from: [f32; 2], to: [f32; 2], s: f32) -> (Node, UiTransform) {
    let (from, to) = (Vec2::from(from), Vec2::from(to));
    let length = from.distance(to);
    let middle = (from + to) / 2.0;
    let thick = (LINE * s).max(1.0);
    let node = Node { position_type: PositionType::Absolute, left: Val::Px((middle.x - length / 2.0) * s), top: Val::Px(middle.y * s - thick / 2.0), width: Val::Px(length * s), height: Val::Px(thick), ..default() };
    let angle = (to.y - from.y).atan2(to.x - from.x);
    (node, UiTransform { rotation: Rot2::radians(angle), ..UiTransform::IDENTITY })
}

/// The marks of what a shown slot takes, by its item's template and its
/// place, worked out once.
fn slot_marks(game: &crate::GameData, build: &Item, slot: &BuildSlot, filters: &mut HashMap<(String, usize), Vec<&'static str>>) -> Vec<&'static str> {
    let Some(owner) = build.find(slot.owner) else { return Vec::new() };
    let key = (owner.kind.template.clone(), slot.index);
    if let Some(marks) = filters.get(&key) {
        return marks.clone();
    }
    let marks = owner.kind.slots.get(slot.index).map(|holder| filter_marks(&game.db, holder)).unwrap_or_default();
    filters.insert(key, marks.clone());
    marks
}

/// The game's caption and the weapon's name under it (`UpdateNameLabel`:
/// the item's name in the install's words).
#[allow(clippy::too_many_arguments)]
fn statics_under(layer: &mut ChildSpawnerCommands, look: &HealthLook, eb: &EditBuildLook, prepared: &Prepared, statics: &Statics, said: &Words, s: f32, wide: f32) {
    let mut words = prepared.words.clone();
    if let Some(sub) = prepared.sub_caption {
        words.insert(sub, statics.name.clone());
    }
    let marks = Marks { within: &prepared.caption, words: Some(&words), ..Default::default() };
    draw_nodes(layer, look, &prepared.screen, &eb.screen, EB_SCREEN, marks, said, Frame::Is([wide, HIGH]), [0.0, 0.0], s);
}

/// The rest of the game's screen: the panel of a build ready to assemble
/// (`SetItemReadyToAssembleActive`: a build not yet the weapon's, its
/// parts all had, which here they always are), the build's name, the
/// buttons (the saved builds' not built: dimmed as the game dims what does
/// not work, `SetUnlockStatus`; ASSEMBLE so while there is nothing to
/// build), the toggles (ONLY AVAILABLE dimmed: every part is had here), the
/// back button and the loader; and what takes their clicks.
#[allow(clippy::too_many_arguments)]
fn statics_over(layer: &mut ChildSpawnerCommands, look: &HealthLook, eb: &EditBuildLook, prepared: &Prepared, statics: &Statics, said: &Words, s: f32, wide: f32) {
    const DIM: f32 = 0.3;
    let mut set: HashMap<usize, bool> = HashMap::new();
    for (at, mark) in prepared.marks.iter().enumerate() {
        if let Some(mark) = mark {
            set.insert(*mark, at < 3 && statics.toggles[at]);
        }
    }
    if let Some(ready) = prepared.ready {
        set.insert(ready, statics.ready);
    }
    if let Some(loader) = prepared.loader {
        set.insert(loader, false);
    }
    let mut groups: HashMap<usize, f32> = prepared.buttons.iter().map(|&button| (button, DIM)).collect();
    groups.extend(prepared.save.map(|save| (save, DIM)));
    groups.extend(prepared.assemble.map(|assemble| (assemble, if statics.ready { 1.0 } else { DIM })));
    groups.extend(prepared.toggle[3].map(|only| (only, DIM)));
    let mut skip = prepared.caption.clone();
    skip.extend(prepared.preview_panel);
    let marks = Marks { skip: &skip, set: Some(&set), groups: Some(&groups), words: Some(&prepared.words), ..Default::default() };
    draw_nodes(layer, look, &prepared.screen, &eb.screen, EB_SCREEN, marks, said, Frame::Is([wide, HIGH]), [0.0, 0.0], s);
    // What takes the clicks, over what is drawn.
    let rect = |index: usize| prepared.screen.rect(index, [wide, HIGH]);
    for (at, toggle) in prepared.toggle.iter().take(3).enumerate() {
        if let Some(toggle) = toggle {
            layer.spawn((Button, Press::Toggle(at), rect_node(rect(*toggle), s)));
        }
    }
    if statics.ready {
        for button in [prepared.assemble_button, prepared.ready_button].into_iter().flatten() {
            layer.spawn((Button, Press::Assemble, rect_node(rect(button), s)));
        }
    }
    if let Some(back) = prepared.back {
        layer.spawn((Button, Press::Back, rect_node(rect(back), s)));
    }
}

/// A slot's bone as the game draws it (`ModdingScreenSlotView`): its icon
/// by its class and whether it holds a part, and a vital or gear slot's
/// name beside it, black on red where a vital part is missing.
#[allow(clippy::too_many_arguments)]
fn bone_game(bone: &mut ChildSpawnerCommands, look: &HealthLook, eb: &EditBuildLook, prepared: &Prepared, slot: &BuildSlot, texts: &HashMap<String, String>, said: &Words, s: f32) {
    let Some(icon) = prepared.bone_icon else { return };
    let named = matches!(slot.class, ModClass::Vital | ModClass::Gear);
    let mut panel = eb.slot.clone();
    let mut words = HashMap::new();
    if let (Some(name), true) = (prepared.bone_name, named) {
        words.insert(name, slot_said(texts, slot));
    }
    look.ask_by_content(&mut panel, &eb.slot_drawn, &words);
    let mut set = HashMap::new();
    set.extend(prepared.bone_panel.map(|panel| (panel, named)));
    let mut sprites = HashMap::new();
    if let Some(picture) = eb.bones.get(bone_icon(slot)) {
        sprites.insert(icon, *picture);
    }
    let mut colors = HashMap::new();
    colors.extend(prepared.bone_back.map(|back| (back, if slot.missing { MISSING_GROUND } else { Color::BLACK })));
    colors.extend(prepared.bone_name.map(|name| (name, if slot.missing { Color::BLACK } else { Color::WHITE })));
    let within = [icon];
    let marks = Marks { within: &within, set: Some(&set), sprites: Some(&sprites), colors: Some(&colors), words: Some(&words), ..Default::default() };
    draw_nodes(bone, look, &panel, &eb.slot_drawn, EB_SLOT, marks, said, Frame::Is([0.0, 0.0]), [0.0, 0.0], s);
}

/// A slot's view as the game draws it, about its place on the ellipse (its
/// pivot, the middle of its foot): its circle, the marks of what it takes
/// over it, its dropdown's button while the mouse is on it or its dropdown
/// is open (open or shut, `SetMenuOpen`), and the part in it as a part's
/// view (`CreateModdingItemView`, a part's cells, from its lower left) or
/// the empty view; what the mouse is found on.
#[allow(clippy::too_many_arguments)]
fn view_game(view: &mut ChildSpawnerCommands, look: &HealthLook, eb: &EditBuildLook, prepared: &Prepared, carried: &Carried, icons: &mut crate::icons::Icons, shown: Option<&Item>, slot: &BuildSlot, boxed: BoxShown, marks: &[&'static str], said: &Words, s: f32) {
    let Some(item) = prepared.menu_item else { return };
    let [x, y, w, h] = prepared.item_rect;
    let id = (slot.owner, slot.index);
    view.spawn((SlotHover(id), Button, RelativeCursorPosition::default(), rect_node([x, y, w, h], s)));
    // The prefab's own pieces, about the pivot.
    let pivot_in = |panel: &ScenePanel| {
        let rect = panel.rect(item, [0.0, 0.0]);
        let pivot = panel.nodes[item].layout.pivot;
        [-(rect[0] + pivot[0] * rect[2]), -(rect[1] + (1.0 - pivot[1]) * rect[3])]
    };
    let origin = pivot_in(&eb.slot);
    // (What draws nothing is not made: the dropdown's button while it is
    // hidden, the fade of a locked slot, which none is here, the empty
    // text a debug build of the game writes to.)
    let mut set = HashMap::new();
    set.extend(prepared.slot_button.map(|button| (button, true)));
    let mut skip: Vec<usize> = [prepared.slot_fade, prepared.slot_debug].into_iter().flatten().collect();
    skip.extend(prepared.slot_button.filter(|_| !boxed.button));
    let mut sprites = HashMap::new();
    if let (Some(button), Some(picture)) = (prepared.slot_button, eb.dropdown[if boxed.open { 0 } else { 1 }]) {
        sprites.insert(button, picture);
    }
    let within = [item];
    let pieces = Marks { within: &within, skip: &skip, set: Some(&set), sprites: Some(&sprites), ..Default::default() };
    draw_nodes(view, look, &eb.slot, &eb.slot_drawn, EB_SLOT, pieces, said, Frame::Is([0.0, 0.0]), origin, s);
    // The marks of what it takes, in a row over its top left
    // (`_filtersContainer`: from its left, their feet two over its top).
    let mut left = x;
    for kind in marks {
        let Some(sprite) = look.mod_mark(kind).and_then(|picture| look.sprite_of(picture, Color::WHITE)) else { continue };
        let [mw, mh] = sprite.size;
        crate::health_view::draw_sprite(view, &sprite, [left * s, (y - 2.0 - mh) * s, mw * s, mh * s], s);
        left += mw;
    }
    // Its dropdown's button's click.
    if let (Some(button), true) = (prepared.slot_button, boxed.button) {
        let rect = eb.slot.rect(button, [0.0, 0.0]);
        view.spawn((Button, Press::Open(id), rect_node([rect[0] + origin[0], rect[1] + origin[1], rect[2], rect[3]], s)));
    }
    // The part, or the empty view: a part of any cells in a view of the
    // part view's own size (`ItemViewContainer`, a row that sizes it by
    // its `LayoutElement`), as the dropdown's are.
    match slot.part.and_then(|part| shown?.find(part)) {
        Some(part) => {
            let size = eb.part_side();
            want_picture(carried, icons, part);
            let node = rect_node([x, y + h - size[1], size[0], size[1]], s);
            match eb.part_look.as_ref() {
                Some(part_look) => crate::inventory_view::game_item_view(view, carried, part, node, Vec2::new(size[0], size[1]) * s, false, VIEW_LAYER, s, part_look),
                None => crate::inventory_view::item_view(view, carried, part, node, false, Vec2::new(size[0], size[1]) * s, false, VIEW_LAYER, s),
            }
        }
        None => {
            let side = eb.empty.nodes.first().map_or([64.0, 64.0], |top| top.layout.size_delta);
            let marks = Marks { words: Some(&prepared.empty_words), ..Default::default() };
            draw_nodes(view, look, &eb.empty, &eb.empty_drawn, EB_EMPTY, marks, said, Frame::Is(side), [x, y + h - side[1]], s);
        }
    }
}

/// A part's picture asked for where it has none yet.
fn want_picture(carried: &Carried, icons: &mut crate::icons::Icons, item: &Item) {
    let key = crate::icons::picture_key(item);
    if !carried.pictures.contains_key(&key) {
        icons.want(key, item.cells());
    }
}

/// The game's dropdown of a slot (`DropDownMenu`): its top left at the
/// slot's view's menu anchor, kept on the screen and off its foot
/// (`CorrectPositionResolution`, rounded); the empty view over the parts'
/// views in a grid as square as their count (`GridLayoutGroup`,
/// `Flexible`: as many columns as the square root, rounded up), each in a
/// cell of its own; a click on one puts it in the slot.
#[allow(clippy::too_many_arguments)]
fn dropdown_game(layer: &mut ChildSpawnerCommands, look: &HealthLook, eb: &EditBuildLook, prepared: &Prepared, carried: &Carried, icons: &mut crate::icons::Icons, id: SlotId, offered: &[Arc<Kind>], view_at: [f32; 2], said: &Words, s: f32, wide: f32) {
    let Some(menu) = prepared.dropdown else { return };
    // Clicked anywhere else, it shuts (the app's).
    layer.spawn((Button, Press::Shut, self::layer()));
    let count = offered.len();
    let columns = (count as f32).sqrt().ceil() as usize;
    let rows = if columns == 0 { 0 } else { count.div_ceil(columns) };
    let span = |cells: usize| if cells == 0 { 0.0 } else { cells as f32 * GRID_CELL + (cells - 1) as f32 * GRID_SPACING };
    let mut panel = prepared.screen.clone();
    let side = eb.empty.nodes.first().map_or([64.0, 64.0], |top| top.layout.size_delta);
    let element = |preferred: [f32; 2]| trp_data::screen::Element { ignore: false, min: [-1.0, -1.0], preferred, flexible: [-1.0, -1.0] };
    if let Some(holder) = prepared.none_holder {
        panel.nodes[holder].element = Some(element(side));
    }
    if let Some(grid) = prepared.grid {
        panel.nodes[grid].element = Some(element([span(columns), span(rows)]));
    }
    let size = [wide, HIGH];
    let [mx, my, mw, mh] = panel.rect(menu, size);
    // Its pivot (its top left) at the anchor, then kept on the screen.
    let mut at = [view_at[0] + prepared.menu_anchor[0], view_at[1] + prepared.menu_anchor[1]];
    let (left, right, foot, top) = (at[0], wide - (at[0] + mw), HIGH - (at[1] + mh) - MENU_FOOT, at[1]);
    if left < 0.0 {
        at[0] -= left;
    } else if right < 0.0 {
        at[0] += right;
    }
    if foot < 0.0 {
        at[1] += foot;
    } else if top < 0.0 {
        at[1] -= top;
    }
    at = [at[0].round(), at[1].round()];
    let origin = [at[0] - mx, at[1] - my];
    let within = [menu];
    let set: HashMap<usize, bool> = [(menu, true)].into_iter().collect();
    let marks = Marks { within: &within, set: Some(&set), ..Default::default() };
    draw_nodes(layer, look, &panel, &eb.screen, EB_SCREEN, marks, said, Frame::Is(size), origin, s);
    // The empty view, which empties the slot.
    if let Some(holder) = prepared.none_holder {
        let [hx, hy, _, _] = panel.rect(holder, size);
        let place = [hx + origin[0], hy + origin[1]];
        layer.spawn((Button, Press::Pick(id, None), rect_node([place[0], place[1], side[0], side[1]], s))).with_children(|empty| {
            let marks = Marks { words: Some(&prepared.empty_words), ..Default::default() };
            draw_nodes(empty, look, &eb.empty, &eb.empty_drawn, EB_EMPTY, marks, said, Frame::Is(side), [0.0, 0.0], s);
        });
    }
    // The parts.
    let Some(grid) = prepared.grid else { return };
    let [gx, gy, _, _] = panel.rect(grid, size);
    for (n, kind) in offered.iter().enumerate() {
        let (column, row) = (n % columns.max(1), n / columns.max(1));
        let cell = [gx + origin[0] + column as f32 * (GRID_CELL + GRID_SPACING), gy + origin[1] + row as f32 * (GRID_CELL + GRID_SPACING), GRID_CELL, GRID_CELL];
        let item = Item::new(kind.clone());
        want_picture(carried, icons, &item);
        let cell_size = Vec2::splat(GRID_CELL * s);
        let inside = Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), width: Val::Px(cell_size.x), height: Val::Px(cell_size.y), ..default() };
        layer.spawn((Button, Press::Pick(id, Some(kind.clone())), rect_node(cell, s))).with_children(|cell| match eb.part_look.as_ref() {
            Some(part_look) => crate::inventory_view::game_item_view(cell, carried, &item, inside, cell_size, false, VIEW_LAYER, s, part_look),
            None => crate::inventory_view::item_view(cell, carried, &item, inside, false, cell_size, false, VIEW_LAYER, s),
        });
    }
}

/// The app's look of the screen's own objects (without `TARKOV UI`): the
/// caption, the weapon's name, the toggles, the buttons' column (ASSEMBLE
/// the one that works; the saved builds' dim), BACK, and the loader.
fn statics_app(layer: &mut ChildSpawnerCommands, statics: &Statics, s: f32, wide: f32) {
    let px = |v: f32| Val::Px(v * s);
    let at = |x: f32, y: f32| Node { position_type: PositionType::Absolute, left: px(x), top: px(y), ..default() };
    let centred = |top: f32| Node { position_type: PositionType::Absolute, left: px(0.0), width: Val::Percent(100.0), top: px(top), justify_content: JustifyContent::Center, column_gap: px(20.0), ..default() };
    layer.spawn(centred(14.0)).with_children(|row| {
        row.spawn(words("WEAPON BUILDS", 41.0 * s, INK));
    });
    layer.spawn(centred(55.0)).with_children(|row| {
        row.spawn(words(statics.name.clone(), 18.0 * s, INK));
    });
    let labels = ["Vital parts", "Functional mods", "Equipment parts", "Use only available parts"];
    layer.spawn(centred(98.0)).with_children(|row| {
        for (index, label) in labels.iter().enumerate() {
            let (on, live) = if index < 3 { (statics.toggles[index], true) } else { (false, false) };
            let ink = if live { INK } else { INK.with_alpha(0.3) };
            let mut toggle = row.spawn(Node { column_gap: px(5.0), align_items: AlignItems::Center, ..default() });
            if live {
                toggle.insert((Button, Press::Toggle(index)));
            }
            toggle.with_children(|toggle| {
                toggle.spawn((Node { width: px(16.0), height: px(16.0), border: UiRect::all(px(2.0)), ..default() }, BorderColor::all(ink), BackgroundColor(if on { ink } else { Color::NONE })));
                toggle.spawn(words(*label, 18.0 * s, ink));
            });
        }
    });
    let column = ["NEW BUILD", "OPEN ...", "SAVE AS ...", "SELECT WEAPON", "FIND PARTS", "PUBLISH ...", "DELETE BUILD"];
    let mut y = 66.0;
    for (index, label) in column.iter().enumerate() {
        if index == 6 {
            y += 5.0 + 3.0;
        }
        let ground = if index == 6 { Color::srgb(0.231, 0.0, 0.0) } else { Color::srgb(0.169, 0.176, 0.180) };
        layer.spawn((Node { width: px(150.0), height: px(18.0), padding: UiRect::left(px(35.0)), align_items: AlignItems::Center, ..at(45.0, y) }, BackgroundColor(ground.with_alpha(0.3)))).with_children(|button| {
            button.spawn(words(*label, 11.0 * s, Color::WHITE.with_alpha(0.3)));
        });
        y += 18.0 + 3.0;
    }
    y += 15.0;
    let ink = if statics.ready { Color::WHITE } else { Color::WHITE.with_alpha(0.3) };
    let mut assemble = layer.spawn((Node { width: px(51.0), height: px(50.0), justify_content: JustifyContent::Center, align_items: AlignItems::Center, border: UiRect::all(px(1.0)), ..at(45.0 + 51.0 + 20.0, y) }, BorderColor::all(ink), BackgroundColor(Color::srgb(0.2, 0.2, 0.2))));
    if statics.ready {
        assemble.insert((Button, Press::Assemble));
    }
    assemble.with_children(|button| {
        button.spawn(words("ASSEMBLE", 10.0 * s, ink));
    });
    layer.spawn((Button, Press::Back, Node { width: px(250.0), height: px(55.6), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..at(wide - 275.0, HIGH - 50.0 - 55.6) })).with_children(|back| {
        back.spawn(words("BACK", 36.0 * s, Color::srgb(0.906, 0.898, 0.831)));
    });
    layer.spawn((
        AppLoader,
        Node { width: px(88.0), height: px(88.0), border: UiRect::all(px(3.0)), border_radius: BorderRadius::all(px(44.0)), ..at(wide / 2.0 - 44.0, HIGH / 2.0 - 44.0) },
        BorderColor { top: INK, left: INK.with_alpha(0.0), right: INK.with_alpha(0.5), bottom: INK.with_alpha(0.25) },
        UiTransform::IDENTITY,
        Visibility::Hidden,
    ));
}

/// The app's look of a slot's bone: a square of its class's colour, full
/// where it holds a part, and a vital or gear slot's name beside it.
fn bone_app(bone: &mut ChildSpawnerCommands, slot: &BuildSlot, texts: &HashMap<String, String>, s: f32) {
    let px = |v: f32| Val::Px(v * s);
    let color = class_color(slot);
    bone.spawn((Node { position_type: PositionType::Absolute, left: px(-4.5), top: px(-4.5), width: px(9.0), height: px(9.0), border: UiRect::all(px(2.0)), ..default() }, BorderColor::all(color), BackgroundColor(if slot.part.is_some() { color } else { Color::NONE })));
    if matches!(slot.class, ModClass::Vital | ModClass::Gear) {
        let (ground, ink) = if slot.missing { (MISSING_GROUND, Color::BLACK) } else { (Color::BLACK, Color::WHITE) };
        bone.spawn((Node { position_type: PositionType::Absolute, left: px(12.5), top: px(-8.0), padding: UiRect::axes(px(2.0), px(1.0)), ..default() }, BackgroundColor(ground))).with_children(|plate| {
            plate.spawn(words(slot_said(texts, slot), 11.0 * s, ink));
        });
    }
}

/// The app's look of a slot's view: a box with the part's view in it, or
/// EMPTY; a click on it opens its dropdown.
fn view_app(view: &mut ChildSpawnerCommands, carried: &Carried, icons: &mut crate::icons::Icons, shown: Option<&Item>, slot: &BuildSlot, s: f32) {
    let id = (slot.owner, slot.index);
    let color = class_color(slot);
    let rect = [-32.0, -64.0, 64.0, 64.0];
    view.spawn((SlotHover(id), Button, Press::Open(id), RelativeCursorPosition::default(), Node { border: UiRect::all(Val::Px(s)), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..rect_node(rect, s) }, BorderColor::all(color), BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)))).with_children(|boxed| {
        match slot.part.and_then(|part| shown?.find(part)) {
            Some(part) => {
                want_picture(carried, icons, part);
                let size = Vec2::splat(62.0 * s);
                crate::inventory_view::item_view(boxed, carried, part, Node { width: Val::Px(size.x), height: Val::Px(size.y), ..default() }, false, size, false, VIEW_LAYER, s);
            }
            None => {
                boxed.spawn(words("EMPTY", 9.0 * s, color));
            }
        }
    });
}

/// The app's look of a slot's dropdown: a list of the parts it takes, the
/// line that empties it first, scrolled by the wheel.
#[allow(clippy::too_many_arguments)]
fn dropdown_app(layer: &mut ChildSpawnerCommands, carried: &Carried, icons: &mut crate::icons::Icons, id: SlotId, offered: &[Arc<Kind>], view_at: [f32; 2], s: f32, wide: f32) {
    let px = |v: f32| Val::Px(v * s);
    let columns = 6usize;
    let width = columns as f32 * (CELL + 2.0) + 10.0;
    let left = (view_at[0] + 32.0 + 30.0).min(wide - width - 10.0);
    let top = (view_at[1] - 64.0).clamp(10.0, HIGH - 120.0);
    layer.spawn((Button, Press::Shut, self::layer()));
    layer
        .spawn((DropdownList, ScrollPosition(Vec2::ZERO), Node { position_type: PositionType::Absolute, left: px(left), top: px(top), width: px(width), max_height: px(HIGH - top - 120.0), flex_wrap: FlexWrap::Wrap, padding: UiRect::all(px(5.0)), row_gap: px(2.0), column_gap: px(2.0), overflow: Overflow::scroll_y(), ..default() }, BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.922))))
        .with_children(|list| {
            list.spawn((Button, Press::Pick(id, None), Node { width: px(CELL), height: px(CELL), border: UiRect::all(px(1.0)), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() }, BorderColor::all(INK.with_alpha(0.4)))).with_children(|none| {
                none.spawn(words("NONE", 10.0 * s, INK));
            });
            for kind in offered {
                let item = Item::new(kind.clone());
                let [w, h] = item.cells();
                let size = Vec2::new(w as f32 * CELL + (w as f32 - 1.0) * 2.0, h as f32 * CELL + (h as f32 - 1.0) * 2.0) * s;
                want_picture(carried, icons, &item);
                list.spawn((Button, Press::Pick(id, Some(kind.clone())), Node { width: Val::Px(size.x), height: Val::Px(size.y), ..default() })).with_children(|cell| {
                    crate::inventory_view::item_view(cell, carried, &item, Node { width: Val::Px(size.x), height: Val::Px(size.y), ..default() }, false, size, false, VIEW_LAYER, s);
                });
            }
        });
}

/// A press on the screen's buttons: a slot's dropdown opened or shut, a
/// part chosen (`ToggleSelection`: the build changed, the dropdown shut,
/// the game's install sound of the slot's class: `PROVISIONAL`), a toggle,
/// ASSEMBLE, BACK.
#[allow(clippy::too_many_arguments)]
fn click(
    mut commands: Commands,
    mut state: ResMut<EditBuild>,
    pressed: Query<(&Interaction, &Press), Changed<Interaction>>,
    mut carried: ResMut<Carried>,
    mut sounds: MessageWriter<Sound>,
    time: Res<Time>,
) {
    if !state.open {
        return;
    }
    let state = &mut *state;
    // A part the mouse comes onto in a dropdown is put in the compared
    // build's same slot (`ModdingSelectableItemContext.Highlight`,
    // `OnCompareRequired`), and as the mouse leaves it the slot is given
    // what the build has in it (`ResetCompare`).
    for (interaction, press) in &pressed {
        let Press::Pick((owner, index), kind) = press else { continue };
        let (Some(compare), Some(build)) = (state.compare.as_mut(), state.build.as_ref()) else { continue };
        if compare.find(*owner).is_none() {
            continue;
        }
        let wanted = match interaction {
            Interaction::Hovered => kind.clone(),
            Interaction::None => build.find(*owner).and_then(|part| part.slots.get(*index).cloned().flatten()).map(|part| part.kind.clone()),
            Interaction::Pressed => continue,
        };
        rules::select(compare, *owner, *index, wanted.as_ref());
        state.compare_made += 1;
    }
    let Some(press) = pressed.iter().find(|(interaction, _)| **interaction == Interaction::Pressed).map(|(_, press)| press.clone()) else { return };
    // While the model of a pick is made, no slot is had (`place`): a press
    // on one drawn before is not heard.
    let refreshing = state.preview.key.as_deref() != Some(state.build_key.as_str());
    if refreshing && matches!(press, Press::Open(_) | Press::Pick(..)) {
        return;
    }
    match press {
        Press::Open(id) => {
            // (`DropDownButtonClickHandler`: open shuts it; another's opens
            // in its place.)
            state.menu = if state.menu == Some(id) { None } else { Some(id) };
            state.menu_seen = true;
            sounds.write(Sound::Ui(Ui::MenuDropdown));
        }
        Press::Shut => {
            state.menu = None;
            state.menu_seen = true;
        }
        Press::Pick((owner, index), kind) => {
            let Some(slot) = state.slots.iter().find(|slot| (slot.owner, slot.index) == (owner, index)).cloned() else { return };
            if let Some(build) = state.build.as_mut() {
                rules::select(build, slot.owner, slot.index, kind.as_ref());
                state.build_key = trp_sim::inventory::rig_key(build);
                info!("the edit build screen: {} picked for its slot {}", kind.as_ref().map_or("nothing", |kind| kind.name.as_str()), slot.index);
                state.made += 1;
            }
            let sound = match slot.class {
                ModClass::Vital => Ui::InstallModVital,
                ModClass::Gear => Ui::InstallModGear,
                ModClass::Functional => Ui::InstallModFunc,
            };
            sounds.write(Sound::Ui(sound));
            state.menu = None;
            state.menu_seen = true;
        }
        Press::Toggle(index) => {
            state.toggles[index] = !state.toggles[index];
        }
        Press::Info => {
            state.stats_folded = !state.stats_folded;
        }
        Press::Assemble => {
            let (Some(id), Some(build)) = (state.weapon, state.build.clone()) else { return };
            // A build with a vital part missing is not built (the user's).
            if vital_missing(&build) {
                return;
            }
            let Some(weapon) = carried.equipment.item.find_mut(id) else { return };
            if !rules::already_built(weapon, &build) {
                let gone = rules::assemble(weapon, &build);
                state.made += 1;
                state.built = true;
                sounds.write(Sound::Ui(Ui::MenuWeaponAssemble));
                info!("the weapon in hand built anew at the workbench: {} parts gone", gone.len());
            }
            let said = state.texts.get(&BUILT.to_lowercase()).cloned().unwrap_or_else(|| BUILT.to_owned());
            state.notice = Some((said, time.elapsed_secs() + NOTICE_FOR));
        }
        Press::Back => shut(state, &mut commands, &mut sounds),
    }
}
