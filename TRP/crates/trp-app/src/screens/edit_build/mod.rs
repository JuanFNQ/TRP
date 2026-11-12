//! The edit build screen, opened at the gun bench for a weapon and at the armor bench for gear.
//!
//! The edit build screen (workbench spec, "The edit build screen"),
//! opened at the gun bench for the weapon in hand,
//! and at the armor bench for the helmet or the armour worn (workbench
//! spec, "The armor bench"): the same screen, its parts every
//! thing a slot of the gear takes, its model the gear with its parts, the
//! hands left as they are, and each of the gear's own armour slots with no
//! object of its name in the model given a place of the app's on it
//! (`gear_points`), where the game shows none.
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
//! What is drawn is kept, and only what changes is drawn again (the
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
use trp_data::interface::screen::ScenePanel;
use trp_sim::items::edit_build::{self as rules, BuildSlot, ModClass};
use trp_sim::items::inventory::{is, Id, Item, Kind};

use crate::platform::convert;
use crate::screens::health_view::{draw_nodes, EditBuildLook, Frame, HealthLook, Marks, Piece, Words};
use crate::screens::inventory_view::{words, Carried, HandsHeld};
use crate::audio::ui_sound::{Sound, Ui};

mod preview_model;
mod screen_parts;
mod preview_view;
mod outline_bones;
mod drawing;
mod slot_views;
mod clicks;

pub(crate) use preview_model::*;
pub(crate) use screen_parts::*;
pub(crate) use preview_view::*;
pub(crate) use outline_bones::*;
use preview_model::{Model, Preview};
use screen_parts::{AppLoader, BUILT, BoxShown, Drawn, DropdownList, PartLocked, PartRefused, PartView, Prepared, Press, PreviewPicture, REFUSED_BORDER, REFUSED_DIM, ScreenRoot, SlotHover, SlotNodes, SlotPiece, Statics, compared_build, key_of, keys, open, release, shut, vital_missing};
use preview_view::{preview, turn};
use outline_bones::{ITEM_OVERLOOK, bone_icon, bones, built, class_color, gear_points, layer, mask_target, outline, place, point_node, prepare, rect_node, seen_whole, slot_said};
use drawing::{draw};
use slot_views::{bone_app, bone_game, dropdown_app, dropdown_game, line_place, slot_marks, statics_app, statics_over, statics_under, view_app, view_game};
use clicks::{click, part_clicks, tip};

/// The preview's own layer: its model, camera and lights.
const LAYER: usize = 24;
/// The layer of the outline's mask: the outlined part's meshes drawn again
/// white, which only the mask's camera draws (`HighLightMesh`).
const MASK_LAYER: usize = 31;
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
/// slightly see-through, so that the range shows faintly behind it (how
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
/// drag that turns it begins (`DragTrigger.onBeginDrag`): the game's one
/// event system's drag threshold (`inventory_view::DRAG_THRESHOLD`, 5
/// pixels, read from the install).
use crate::screens::inventory_view::DRAG_THRESHOLD;
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
/// A part's name's tooltip (`GridItemView.ShowTooltip`: `SimpleTooltip.
/// Show(name, null, 0.6)`): shown that many seconds after the mouse comes
/// onto the part's view.
const TIP_DELAY: f32 = 0.6;
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
        app.init_resource::<EditBuild>().add_systems(Update, (release, open, keys, turn, preview, place, draw, outline, click, part_clicks, tip).chain().after(crate::screens::inventory_view::ScreenDrawn).run_if(crate::running));
    }
}

/// A slot by the item it is on and its place among that item's slots.
type SlotId = (Id, usize);

/// The screen as it stands.
#[derive(Resource, Default)]
pub struct EditBuild {
    pub open: bool,
    /// The weapon in hand or the gear worn it was opened for (`gear`: the
    /// helmet or armour of the armor bench), and the build as changed, its
    /// key, and how many times either has changed; whether a change has
    /// been built since it opened; the weapon to be taken in hand again as
    /// the screen has shut, and whether it is drawn racking
    /// (`inventory_view::BenchAsk`).
    weapon: Option<Id>,
    gear: bool,
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
    scene: Option<Arc<trp_data::interface::edit_build::EditBuildScene>>,
    /// The inspect window's own preview, whose lights and tonemap light the
    /// model here; none where it could not be
    /// read (the screen's own lights then).
    inspect: Option<Arc<trp_data::interface::edit_build::InspectPreview>>,
    parts: Option<Arc<Vec<Arc<Kind>>>>,
    /// Every thing a slot of gear could take (`rules::all_gear_parts`).
    gear_parts: Option<Arc<Vec<Arc<Kind>>>>,
    texts: HashMap<String, String>,
    places: HashMap<String, Option<trp_data::interface::icon::PreviewPlace>>,
    filters: HashMap<(String, usize), Vec<&'static str>>,
    preview: Preview,
    drawn: Drawn,
    /// The game's screen made ready to draw, once.
    prepared: Option<Prepared>,
    /// The models to let go of as the screen has shut.
    released: Vec<String>,
    /// Whether the mouse is the menu's or a window's this frame (a part's
    /// window inspected from here), not the screen's.
    mouse_taken: bool,
    /// The part whose menu of a right click is up; the last press of the
    /// left button on a slot's part (its number, when, and how many in a
    /// row: a double click inspects it).
    menu_part: Option<Item>,
    clicked: Option<(Id, f64, u32)>,
    /// Each part inspected and its copy shown (`Carried::observed`).
    observed: HashMap<Id, Id>,
    /// The part the mouse rests on in a dropdown and its slot, which the
    /// build the characteristics are compared with has in that slot
    /// (`ItemObserveScreen._compareItem`, `OnCompareRequired`); that build
    /// is otherwise the build as it stands on the screen, the game's being
    /// given each slot's part again as the mouse leaves it (`ResetCompare`).
    /// Counted each time it changes; and whether the panel is folded to its
    /// INFO button.
    compare: Option<(SlotId, Option<Arc<Kind>>)>,
    compare_made: u64,
    stats_folded: bool,
}

#[cfg(test)]
mod tests;
