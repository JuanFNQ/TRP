//! The inventory screen (`docs/spec/inventory.md`, "The screen and its
//! tabs"): the game's row of tabs in the game's order, and under Gear the
//! equipment's slots laid over a figure of the character, each container's
//! grids, and whatever is open beside them: the crate on the range.
//!
//! Things are moved with the mouse, by the model's own moves
//! (`trp_sim::inventory::move_item`, `apply_item`): held and let go on a
//! cell, on a slot, or on another item. What the equipment then holds is
//! what the character carries, wears and has in hand (`settle`).
//!
//! And without dragging, as the game's item view takes a click
//! (`docs/spec/inventory.md`, "Clicks, keys and windows"): Ctrl and a
//! click sends a thing to the other side, Alt and a click puts it on, a
//! double click opens a container in a window of its own or shows what a
//! thing is; Ctrl held as a stack is let go asks how many in a window;
//! a quick-use key let go over a thing binds it to that key, and the bar
//! under the screen shows what each key has.
//!
//! It is drawn plainly: boxes and words. The figure and where the slots
//! sit on it are the app's own drawing in the order of the game's panel;
//! the game's prefab for the screen is not read, and none of its art is
//! used.

use std::collections::HashMap;

use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;
use bevy::window::PrimaryWindow;
use bevy::asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use trp_sim::things::{Action, Menu};
use trp_sim::{Acted, FirearmEvent, Interaction as Gesture, State, WorldInteraction};
use trp_sim::inventory::{
    apply_item, can_pick_up, equip, is, move_item, pick_up, quick_find, split_item, split_most, take_out, transfer_item, transfer_most, Applied, Bound, Equipment, Id,
    Item, Order, Place, Refusal, Rules, Spot, HAND_SLOTS,
};

use crate::controller::Controller;
use crate::keys::Bindings;
use crate::range::Range;
use crate::weapon::{HandsRig, Stowed, WeaponRig};
use crate::{GameData, Options, View};

/// What the character carries, what the crate on the range holds, and the
/// game's own short names for items (from the install's English texts).
#[derive(Resource)]
pub struct Carried {
    pub equipment: Equipment,
    pub names: HashMap<String, String>,
    /// The container last searched (at first the airdrop crate), with what
    /// is in it: the one the screen shows beside the equipment.
    pub loot: Item,
    /// Whether it is open in the screen.
    pub loot_open: bool,
    /// The range's other containers with what is in them, by their place
    /// in this list (`range::Placed::holds`); the one in `loot` has none here.
    pub away: Vec<Option<Item>>,
    /// Which of them is in `loot`.
    pub loot_of: usize,
    /// What moving things goes by besides the places' own rules.
    pub rules: Rules,
    /// What is drawn on the body whatever the equipment holds (`--wear`).
    pub fixed_wear: Vec<String>,
    /// What the last move came to, for the screen's bottom line.
    pub note: String,
    /// The game's own equipment panel as the install lays it out; empty
    /// if it could not be read.
    pub panel: trp_data::screen::GearPanel,
    /// What has been put down, each thing where it lies (the engine's axes).
    pub ground: Vec<(Item, Vec3)>,
    /// A picture for each sort of thing, by the sort's name (`icon_of`):
    /// what a thing shows until its own picture is made.
    pub icons: HashMap<&'static str, Picture>,
    /// Each item's own picture, by its template, as the game makes it
    /// (`icons`: its model drawn once and kept).
    pub pictures: HashMap<String, Picture>,
    /// What is bound to the quick-use keys.
    pub bound: Bound,
    /// The windows open over the screen, the last on top.
    pub windows: Vec<Win>,
    /// The container whose window was opened or picked last: where Ctrl
    /// and a click sends a thing (the game's priority window, which the
    /// install's `PriorityWindowMode` has as `Auto`).
    pub priority: Option<Id>,
    /// Whether the install has that setting as `Auto`.
    pub priority_auto: bool,
    /// The window that asks how many of a stack, while it is up.
    pub asking: Option<Asking>,
    /// The backpack's drop has been asked for, by its key or by the
    /// screen's key for putting a thing down: `toggle` begins it, and the
    /// pack leaves the shoulders at the gesture's own mark.
    pub pack_asked: bool,
}

/// A window over the screen: a container's grids, or what a thing is.
#[derive(Clone, Copy, PartialEq)]
pub struct Win {
    pub item: Id,
    pub grids: bool,
    /// Its top left corner in the window, in pixels.
    pub at: Vec2,
}

/// The window that asks how many of a stack are to go (`SplitDialog`):
/// which stack, where to, how many at the most and how many now.
#[derive(Clone, Copy, PartialEq)]
pub struct Asking {
    id: Id,
    target: Target,
    /// What it is headed: a split into a place, or a transfer to a stack.
    title: &'static str,
    most: u32,
    count: u32,
    at: Vec2,
    /// Whether a figure has been typed since it came up (the first key
    /// typed takes the place of what stood there).
    typed: bool,
}

impl Asking {
    /// The values the slider stops at (`StepSlider.Show`): a notch every
    /// twenty-fourth of the most, rounded up, the first notch at one, and
    /// the most itself at the end.
    fn stops(most: u32) -> Vec<u32> {
        let notch = most.div_ceil(24).max(1);
        let notches = if most / notch < 24 { most.div_ceil(notch) } else { 24 };
        let first = if notch == 1 { 1 } else { 0 };
        (first..notches).map(|at| (notch * at).max(1)).chain([most]).collect()
    }
}

/// A picture of a sort of thing: as it lies and turned a quarter, and how
/// wide it is for its height. It is drawn whole in an item's box at the
/// largest size that fits, so it grows and shrinks with the box and is
/// never pulled out of shape.
pub struct Picture {
    upright: Handle<Image>,
    turned: Handle<Image>,
    /// Width over height, as it lies.
    shape: f32,
    /// It is the thing's own picture, made for its box: it fills it, as
    /// the game's does.
    fills: bool,
}

impl Picture {
    /// A thing's own picture (`icons`), as it lies and turned a quarter.
    pub fn rendered(upright: Handle<Image>, turned: Handle<Image>, shape: f32) -> Self {
        Self { upright, turned, shape, fills: true }
    }
}

/// What is looked at from within reach: a thing of the range the hands
/// work (`range::Range::things`), or a thing lying on the ground (which
/// comes first).
#[derive(Resource, Default, Clone, PartialEq)]
pub struct Reach {
    pub thing: Option<usize>,
    pub loose: Option<Id>,
    /// The game's menu for it (`AvailableInteractionState`): every line
    /// it offers, and the one chosen, which the wheel moves and the
    /// interact key does.
    pub menu: Option<Menu>,
}

/// The game's own words for the menu's lines, by the names its code asks
/// for them by (the install's English texts); a line the install has no
/// words for is shown by its name.
#[derive(Resource, Default)]
pub struct MenuTexts(pub HashMap<String, String>);

impl MenuTexts {
    fn of(&self, action: Action) -> String {
        self.0.get(&action.name().to_lowercase()).cloned().unwrap_or_else(|| action.name().to_owned()).to_uppercase()
    }
}

/// How far down the crate's grids the screen's panel for them is scrolled,
/// in pixels.
#[derive(Resource, Default)]
pub struct LootScroll(f32);

/// The scrolled part of the panel beside the equipment, and the bar's
/// thumb: each with how high the part shown is and how high all of it,
/// in pixels.
#[derive(Component)]
struct LootArea {
    shown: f32,
    whole: f32,
}
#[derive(Component)]
struct LootThumb {
    shown: f32,
    whole: f32,
}

/// Whether a thing, or anything in it, is of an item template.
fn carries(item: &Item, template: &str) -> bool {
    item.kind.template == template || item.contents().any(|inside| carries(inside, template))
}

/// A thing lying on the ground, as drawn.
#[derive(Component)]
struct LooseItem(Id);

/// The tabs in the game's order (`EInventoryTab`), with whether each can
/// be opened here. The captions are plain English words, not the game's
/// own texts.
///
/// The last is not the game's: its settings are a screen of its menu, and
/// the user asked for them here, as a tab at the far right (2026-10-12;
/// `options_view.rs`).
pub const TABS: [(&str, bool); 9] = [
    ("OVERALL", false),
    ("GEAR", true),
    ("HEALTH", true),
    ("SKILLS", true),
    ("MAP", false),
    ("TASKS", false),
    ("ACHIEVEMENTS", false),
    ("PRESTIGE", false),
    ("OPTIONS", true),
];
/// The tab the game opens on.
const GEAR: usize = 1;
/// The options' tab.
pub const OPTIONS: usize = 8;

/// The equipment panel's views in the order the game's panel names them,
/// each with a caption (an English word of the app's) and where it sits
/// if the install's own panel could not be read: left, top, width and
/// height in the panel's units, three to a row. Where the install's panel
/// is read (`trp_data::screen::GearPanel`), its places are used instead.
const EQUIPMENT: [(&str, &str, [f32; 4]); 10] = [
    ("Earpiece", "EARPIECE", [0.0, 20.0, 125.0, 125.0]),
    ("Headwear", "HEADWEAR", [184.0, 20.0, 125.0, 125.0]),
    ("FaceCover", "FACE COVER", [368.0, 20.0, 125.0, 125.0]),
    ("ArmBand", "ARMBAND", [0.0, 186.0, 125.0, 44.0]),
    ("ArmorVest", "BODY ARMOR", [184.0, 186.0, 125.0, 125.0]),
    ("Eyewear", "EYEWEAR", [368.0, 186.0, 125.0, 125.0]),
    ("FirstPrimaryWeapon", "ON SLING", [0.0, 354.0, 310.0, 125.0]),
    ("Holster", "HOLSTER", [368.0, 354.0, 125.0, 125.0]),
    ("SecondPrimaryWeapon", "ON BACK", [0.0, 524.0, 310.0, 125.0]),
    ("Scabbard", "SHEATH", [368.0, 524.0, 125.0, 125.0]),
];
/// The panel's own size where the install's is not read.
const PANEL_SIZE: [f32; 2] = [494.0, 650.0];
/// The figure behind the slots, as plain shapes in the panel's units:
/// left, top, width, height and how round the corners are. The app's own
/// drawing of a paper figure; the game has none in this panel (in its
/// menu the character stands in the scene behind it).
const FIGURE: [[f32; 5]; 9] = [
    [204.0, 22.0, 86.0, 104.0, 43.0],
    [231.0, 118.0, 32.0, 30.0, 0.0],
    [150.0, 142.0, 194.0, 110.0, 34.0],
    [172.0, 200.0, 150.0, 190.0, 18.0],
    [106.0, 150.0, 50.0, 262.0, 24.0],
    [338.0, 150.0, 50.0, 262.0, 24.0],
    [168.0, 366.0, 158.0, 70.0, 20.0],
    [168.0, 408.0, 74.0, 238.0, 24.0],
    [252.0, 408.0, 74.0, 238.0, 24.0],
];
/// The containers panel's views, in the game's order.
const CONTAINERS: [(&str, &str); 4] = [("TacticalVest", "TACTICAL RIG"), ("Pockets", "POCKETS"), ("Backpack", "BACKPACK"), ("SecuredContainer", "POUCH")];

/// The game lays its screens out 1080 units high (the inventory screen is
/// 1920 wide and as high as the screen); a unit here is that share of the
/// window's height.
const SCREEN_HEIGHT: f32 = 1080.0;
/// The side of a grid's cell (`GridView`), a slot view's side and the
/// height of the caption over one, and the stash panel's width, in those
/// units: the first from the game's code, the others from its scene.
const CELL: f32 = 63.0;
const SLOT_SIDE: f32 = 125.0;
const CAPTION: f32 = 20.0;
const STASH_WIDTH: f32 = 680.0;

/// How far ahead of the character a thing is put down, and how near the
/// line of sight has to pass a thing on the ground for it to count as
/// looked at. ESTIMATE, both: the game throws a thing and lets it fall,
/// and goes by the thing's own collider.
const DROP_AHEAD: f32 = 0.4;
const LOOSE_REACH: f32 = 0.25;

/// How many pixels one of the game's screen units is in this window.
#[derive(Resource, Clone, Copy, PartialEq)]
struct Units(f32);

impl Units {
    fn cell(self) -> f32 {
        (CELL * self.0).round()
    }
}
pub const BACK: Color = Color::srgba(0.05, 0.06, 0.06, 0.94);
pub const PANEL: Color = Color::srgb(0.11, 0.12, 0.12);
pub const LINE: Color = Color::srgb(0.30, 0.31, 0.30);
pub const WORDS: Color = Color::srgb(0.80, 0.80, 0.74);
pub const FAINT: Color = Color::srgb(0.45, 0.46, 0.43);
const THING: Color = Color::srgb(0.20, 0.24, 0.27);
const SLOT: Color = Color::srgba(0.07, 0.08, 0.08, 0.62);
const SHAPE: Color = Color::srgb(0.27, 0.26, 0.22);

#[derive(Component)]
struct Screen;
#[derive(Component)]
struct TabButton(usize);
#[derive(Component)]
struct Page(usize);
#[derive(Component)]
struct GearPage;
/// An item as drawn, by its number, and what it is drawn on: the page
/// (0) or a window over it (its place among the windows, from 1).
#[derive(Component)]
struct ItemView(Id, u8);
/// A slot of an item as drawn.
#[derive(Component)]
struct SlotBox {
    holder: Id,
    slot: usize,
    layer: u8,
}
/// A grid of an item as drawn.
#[derive(Component)]
struct GridBox {
    holder: Id,
    grid: usize,
    cells: [u32; 2],
    layer: u8,
}
/// Something drawn over the page that the mouse does not reach through:
/// a window (its place among the windows, from 1), the quick-use bar, the
/// window that asks how many.
#[derive(Component)]
struct Cover(u8);
const BAR_LAYER: u8 = 200;
const ASKING_LAYER: u8 = 255;
/// Everything drawn over the page, made afresh with it.
#[derive(Component)]
struct Overlay;
/// A window's caption, by which it is moved, and its root.
#[derive(Component)]
struct WinCaption(usize);
#[derive(Component)]
struct WinRoot(usize);
#[derive(Component)]
struct WinClose(usize);
/// The parts of the window that asks how many.
#[derive(Component)]
struct AskBar;
#[derive(Component)]
struct AskAccept;
#[derive(Component)]
struct AskClose;
/// The item being moved, as it follows the mouse.
#[derive(Component)]
struct Ghost;
#[derive(Component)]
struct GhostWords;
#[derive(Component)]
struct Prompt;
/// A line of the menu for what is looked at.
#[derive(Component)]
struct PromptLine;
/// The tab that is shown.
#[derive(Resource)]
pub struct Selected(pub usize);

/// An item held by the mouse: which, whether it has been turned, where in
/// it the mouse took hold (in pixels from its top left), and its name.
struct Held {
    id: Id,
    turned: bool,
    grab: Vec2,
    cells: [u32; 2],
    name: String,
}

#[derive(Resource, Default)]
struct Drag(Option<Held>);

/// The mouse's button down on an item and not yet let go or moved far
/// enough to carry it: a click in the making. Which item, where the mouse
/// was, where in the item it took hold (as a share of it), and when.
#[derive(Clone, Copy)]
struct Pressed {
    id: Id,
    cursor: Vec2,
    at: Vec2,
}

/// The click before this one, for telling a double click.
#[derive(Clone, Copy)]
struct Clicked {
    id: Id,
    at: f64,
    count: u32,
}

#[derive(Resource, Default)]
struct Clicks {
    pressed: Option<Pressed>,
    last: Option<Clicked>,
    /// A window being moved by its caption: which, and where in it the
    /// mouse holds it.
    moving: Option<(usize, Vec2)>,
    /// Whether the slider of the window that asks how many is held.
    sliding: bool,
}

/// How far the mouse has to move with the button down before a thing is
/// carried and not clicked, in pixels. ESTIMATE: the engine's own default
/// for its screens (`EventSystem.pixelDragThreshold`); what the game's
/// scene sets is not read.
const DRAG_THRESHOLD: f32 = 10.0;
/// Two clicks on one thing within this many seconds are a double click:
/// the engine's own figure for its screens (`StandaloneInputModule`, in
/// its published source; a third click starts the count again at odd).
const DOUBLE_CLICK: f64 = 0.3;

/// Where an item held by the mouse would go.
#[derive(Clone, Copy, PartialEq)]
enum Target {
    Item(Id),
    Cell(Id, usize, Place),
    Slot(Id, usize),
}

pub struct InventoryPlugin;

impl Plugin for InventoryPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Selected(GEAR))
            .insert_resource(Units(0.0))
            .init_resource::<Drag>()
            .init_resource::<Clicks>()
            .init_resource::<AskedFor>()
            .init_resource::<Reach>()
            .init_resource::<LootScroll>()
            .init_resource::<MenuTexts>()
            .add_systems(Startup, (spawn, make_icons, menu_texts))
            // After the simulation's frames and before the weapon's clips:
            // a screen opened in one frame has the keys from the next, as
            // the game's has, and what the weapon is told here plays then.
            // None of it before the run has begun (`crate::Warmup`): until
            // then the weapon is in the picture whether the character
            // begins with it or not. What is worn is shown from the first
            // frame all the same, to be built with the rest.
            .add_systems(
                Update,
                (
                    // (The panel's scrolling goes by where the mouse is
                    // over the page as it stood, so before the page is
                    // drawn afresh.)
                    (scripted_drags, reach, toggle.in_set(ScreenKeys), choose_tab, ask, windows, drag, quick_keys, scroll_loot, rebuild, settle, show_ground).chain().run_if(crate::running),
                    crate::body_view::show_worn,
                )
                    .chain()
                    .after(crate::controller::step)
                    .before(crate::animation::play_clips),
            );
    }
}

/// Where the screen's own keys are heard (the inventory's, Escape): what
/// else answers to one of them on the screen comes after, so that a key
/// it is about to take is seen here as still its own.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ScreenKeys;

pub fn words(text: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (Text::new(text), TextFont { font_size: size, ..default() }, TextColor(color))
}

fn boxed(width: Val, min_height: f32) -> impl Bundle {
    let node = Node {
        width,
        min_height: Val::Px(min_height),
        border: UiRect::all(Val::Px(1.0)),
        padding: UiRect::all(Val::Px(4.0)),
        flex_direction: FlexDirection::Column,
        row_gap: Val::Px(2.0),
        ..default()
    };
    (node, BorderColor(LINE), BackgroundColor(PANEL))
}

fn placed(left: f32, top: f32, width: f32, height: f32) -> Node {
    Node { position_type: PositionType::Absolute, left: Val::Px(left), top: Val::Px(top), width: Val::Px(width), height: Val::Px(height), ..default() }
}

impl Carried {
    /// The game's short name for an item, or the database's own.
    fn name(&self, item: &Item) -> String {
        self.names.get(&item.kind.template).filter(|name| !name.is_empty()).cloned().unwrap_or_else(|| item.kind.name.clone())
    }

    /// An item by its number, on the character or in the crate.
    fn find(&self, id: Id) -> Option<&Item> {
        self.equipment.item.find(id).or_else(|| self.loot.find(id))
    }

    /// Whether a thing is one the game opens in a window of its own
    /// (`Item.IsContainer`: what is searched, a plain container, one that
    /// locks).
    fn opens(item: &Item) -> bool {
        ["SearchableItem", "SimpleContainer", "LockableContainer", "StationaryContainer"].iter().any(|class| is(item, class))
    }

    /// The things Ctrl and a click looks in for a place for an item
    /// (`ItemUiContext.QuickFindAppropriatePlace`): the container whose
    /// window is the priority one, unless the item lies in it; else what
    /// is open beside the equipment, unless the item lies in that, with
    /// the equipment after it for a thing the character has; else the
    /// equipment.
    fn other_side(&self, id: Id) -> Vec<Id> {
        trp_sim::inventory::other_side(&self.equipment.item, self.loot_open.then_some(&self.loot), self.priority, id)
    }

    /// After anything has moved: a key lets go of what no longer lies
    /// where it may be bound, and a window of what is no longer there, or
    /// has gone into itself, is shut.
    fn tidy(&mut self) {
        self.bound.settle(&self.equipment.item);
        let there: Vec<bool> = self.windows.iter().map(|window| self.find(window.item).is_some()).collect();
        let mut there = there.into_iter();
        self.windows.retain(|_| there.next().unwrap_or(false));
        if self.priority.is_some_and(|priority| !self.windows.iter().any(|window| window.grids && window.item == priority)) {
            self.priority = None;
        }
    }

    /// Opens a window on a thing, or brings the one it has to the top; a
    /// container's is then where Ctrl and a click sends things.
    fn open_window(&mut self, id: Id, grids: bool, at: Vec2) {
        let had = self.windows.iter().position(|window| window.item == id && window.grids == grids);
        let window = had.map(|had| self.windows.remove(had)).unwrap_or(Win { item: id, grids, at });
        self.windows.push(window);
        if grids && self.priority_auto {
            self.priority = Some(id);
        }
    }

    /// Where an item lies: what holds it, and where in that.
    fn lies(&self, id: Id) -> Option<(Id, Spot)> {
        self.equipment.item.holder_of(id).or_else(|| self.loot.holder_of(id))
    }

    /// Has the container kept as `which` be the one the screen shows.
    fn bring(&mut self, which: usize) {
        if which == self.loot_of || which >= self.away.len() {
            return;
        }
        let Some(coming) = self.away[which].take() else { return };
        let going = std::mem::replace(&mut self.loot, coming);
        self.away[self.loot_of] = Some(going);
        self.loot_of = which;
    }
}

/// Carries a drop out on the things given: on another item, on a cell or
/// on a slot (a slot that holds something stands for what it holds, as in
/// the game). Says what it came to.
fn act(roots: &mut [&mut Item], id: Id, target: Target, rules: &Rules, part: Option<u32>) -> Result<&'static str, Refusal> {
    // With Ctrl held only part of a stack goes (the game's "partial
    // transfer only"): so many split off into a place, or handed to a
    // stack of the kind; a thing that is no stack goes nowhere.
    if let Some(count) = part {
        return part_of(roots, id, target, count, rules);
    }
    let applied = |done: Applied| match done {
        Applied::Merged => "merged",
        Applied::Put(_) => "put in",
        Applied::Topped(_) => "filled up",
    };
    match target {
        Target::Item(onto) => apply_item(roots, id, onto, rules).map(applied),
        Target::Cell(holder, grid, place) => move_item(roots, id, holder, Spot::Cell(grid, place), rules).map(|()| "moved"),
        Target::Slot(holder, slot) => {
            let held = roots.iter().find_map(|root| root.find(holder)).and_then(|holder| holder.slots.get(slot)?.as_ref().map(|item| item.id));
            match held {
                Some(onto) if onto == id => Err(Refusal::AlreadyThere),
                Some(onto) => apply_item(roots, id, onto, rules).map(applied),
                None => move_item(roots, id, holder, Spot::Slot(slot), rules).map(|()| "moved"),
            }
        }
    }
}

/// What a slot's own target stands for: the thing in it, if there is one.
fn in_slot(roots: &[&mut Item], holder: Id, slot: usize) -> Option<Id> {
    roots.iter().find_map(|root| root.find(holder)).and_then(|holder| holder.slots.get(slot)?.as_ref().map(|item| item.id))
}

/// How many of a stack may go to a target with Ctrl held: onto a stack of
/// its kind as many as that has room for, into a place all but one.
fn most_of(roots: &[&mut Item], id: Id, target: Target) -> u32 {
    match target {
        Target::Item(onto) => transfer_most(roots, id, onto),
        Target::Cell(_, grid, place) => split_most(roots, id, Spot::Cell(grid, place)),
        Target::Slot(holder, slot) => match in_slot(roots, holder, slot) {
            Some(onto) => transfer_most(roots, id, onto),
            None => split_most(roots, id, Spot::Slot(slot)),
        },
    }
}

/// So many of a stack to a target.
fn part_of(roots: &mut [&mut Item], id: Id, target: Target, count: u32, rules: &Rules) -> Result<&'static str, Refusal> {
    if count == 0 || count > most_of(roots, id, target) {
        return Err(Refusal::NoAction);
    }
    let onto = match target {
        Target::Item(onto) => Some(onto),
        Target::Slot(holder, slot) => in_slot(roots, holder, slot),
        Target::Cell(..) => None,
    };
    match (onto, target) {
        (Some(onto), _) => transfer_item(roots, id, onto, count).map(|()| "handed over"),
        (None, Target::Cell(holder, grid, place)) => split_item(roots, id, count, holder, Spot::Cell(grid, place), rules).map(|_| "split"),
        (None, Target::Slot(holder, slot)) => split_item(roots, id, count, holder, Spot::Slot(slot), rules).map(|_| "split"),
        (None, Target::Item(_)) => Err(Refusal::NoAction),
    }
}

/// A refusal in words.
fn why(refusal: &Refusal) -> String {
    match refusal {
        Refusal::NoSuchPlace => "there is no such place".into(),
        Refusal::Taken => "the place is taken".into(),
        Refusal::Filter => "it does not go there".into(),
        Refusal::Blocked(by) => format!("what is in {by} is in the way"),
        Refusal::WouldBlock(slot) => format!("it would be in the way of what is in {slot}"),
        Refusal::Stack => "a slot takes one thing, not a stack".into(),
        Refusal::OutsideGrid => "it does not fit there".into(),
        Refusal::NoRoom => "no room".into(),
        Refusal::AlreadyThere => "it is there already".into(),
        Refusal::Cycle => "it cannot go into itself".into(),
        Refusal::TooDeep => "backpacks do not go that deep into one another".into(),
        Refusal::Vital(slot) => format!("the part in {slot} stays on in a raid"),
        Refusal::NotInRaid => "that part is not changed in a raid".into(),
        Refusal::MissingParts => "it lacks a part it cannot do without".into(),
        Refusal::Dogtag => "the dogtag's slot takes nothing".into(),
        Refusal::KeptOn(slot) => format!("it stays in {slot} through a raid"),
        Refusal::NoAction => "nothing to be done with the one on the other".into(),
    }
}

fn spawn(mut commands: Commands, options: Res<Options>) {
    let open = options.inventory;
    let root = Node {
        position_type: PositionType::Absolute,
        left: Val::Px(0.0),
        top: Val::Px(0.0),
        width: Val::Percent(100.0),
        height: Val::Percent(100.0),
        flex_direction: FlexDirection::Column,
        padding: UiRect { left: Val::Px(18.0), right: Val::Px(18.0), top: Val::Px(64.0), bottom: Val::Px(18.0) },
        row_gap: Val::Px(12.0),
        ..default()
    };
    let shown = if open { Visibility::Inherited } else { Visibility::Hidden };
    commands.spawn((Screen, root, BackgroundColor(BACK), GlobalZIndex(10), shown)).with_children(|screen| {
        // The row of tabs.
        screen.spawn(Node { column_gap: Val::Px(6.0), align_items: AlignItems::Center, ..default() }).with_children(|row| {
            for (index, (caption, enabled)) in TABS.into_iter().enumerate() {
                let color = if !enabled { FAINT } else { WORDS };
                let back = if index == GEAR { LINE } else { PANEL };
                // The options' tab stands apart, against the right edge.
                let margin = if index == OPTIONS { UiRect::left(Val::Auto) } else { UiRect::default() };
                row.spawn((TabButton(index), Button, Node { padding: UiRect::axes(Val::Px(14.0), Val::Px(6.0)), margin, ..default() }, BackgroundColor(back)))
                    .with_children(|tab| {
                        tab.spawn(words(caption, 14.0, color));
                    });
            }
        });
        let page = |index: usize| {
            let node = Node { flex_grow: 1.0, column_gap: Val::Px(14.0), display: if index == GEAR { Display::Flex } else { Display::None }, ..default() };
            (Page(index), node)
        };
        // The Gear page is drawn afresh whenever something has moved (`rebuild`).
        screen.spawn((page(GEAR), GearPage));
        // The options' page is drawn by its own module.
        let options_page = Node { flex_grow: 1.0, flex_direction: FlexDirection::Column, row_gap: Val::Px(8.0), min_height: Val::Px(0.0), display: Display::None, ..default() };
        screen.spawn((Page(OPTIONS), crate::options_view::OptionsPage, options_page));
        screen.spawn(page(2)).with_children(|health| {
            health.spawn(boxed(Val::Px(520.0), 80.0)).with_children(|panel| {
                panel.spawn(words("HEALTH", 12.0, FAINT));
                panel.spawn(words("There is no health system yet: the body's parts, bleeding, fractures and their treatment are not built.", 13.0, WORDS));
                panel.spawn(words("The character is taken as fully fed and watered (stamina spec).", 13.0, WORDS));
            });
        });
        screen.spawn(page(3)).with_children(|skills| {
            skills.spawn(boxed(Val::Px(520.0), 80.0)).with_children(|panel| {
                panel.spawn(words("SKILLS", 12.0, FAINT));
                panel.spawn(words(format!("Endurance   level {:.0}", options.endurance), 13.0, WORDS));
                panel.spawn(words(format!("Strength   level {:.0}", options.strength), 13.0, WORDS));
                panel.spawn(words(format!("Mastering of the weapon in hand   level {}", options.mastering), 13.0, WORDS));
                panel.spawn(words("Only what the controller uses is kept; every other skill is untrained.", 12.0, FAINT));
            });
        });
    });
    // The item the mouse holds, over everything.
    let ghost = Node { position_type: PositionType::Absolute, border: UiRect::all(Val::Px(1.0)), padding: UiRect::all(Val::Px(2.0)), overflow: Overflow::clip(), ..default() };
    commands.spawn((Ghost, ghost, BorderColor(WORDS), BackgroundColor(THING.with_alpha(0.8)), GlobalZIndex(20), Visibility::Hidden)).with_children(|held| {
        held.spawn((GhostWords, words("", 10.0, WORDS)));
    });
    // What the game shows when something that can be worked is looked at
    // from within reach: its menu, a line for each thing it offers.
    let prompt = Node {
        position_type: PositionType::Absolute,
        left: Val::Percent(50.0),
        top: Val::Percent(58.0),
        flex_direction: FlexDirection::Column,
        row_gap: Val::Px(2.0),
        ..default()
    };
    commands.spawn((Prompt, prompt, Visibility::Hidden));
}

/// Reads the game's own words for the menu's lines from the install.
fn menu_texts(game: Res<GameData>, mut texts: ResMut<MenuTexts>) {
    let wanted = [Action::Open, Action::Close, Action::Search, Action::Unlock, Action::Breach, Action::BangAndClear, Action::FlashAndClear, Action::MoveIn, Action::Take];
    let names: Vec<&str> = wanted.iter().map(|action| action.name()).collect();
    match trp_data::inventory::texts(&game.unity.install, "en", &names) {
        Ok(found) => texts.0 = found,
        Err(e) => warn!("the menu's own words could not be read: {e:#}"),
    }
}

/// The sort of thing an item is, for its picture: the nearest of its
/// classes that has one.
fn icon_of(item: &Item) -> &'static str {
    const SORTS: [(&str, &str); 16] = [
        ("Weapon", "weapon"),
        ("Magazine", "magazine"),
        ("Ammo", "ammo"),
        ("AmmoBox", "ammo"),
        ("Headwear", "headwear"),
        ("Vest", "vest"),
        ("Armor", "vest"),
        ("Backpack", "backpack"),
        ("Meds", "meds"),
        ("FoodDrink", "food"),
        ("Food", "food"),
        ("Drink", "food"),
        ("Headphones", "headset"),
        ("Knife", "knife"),
        ("Mod", "part"),
        ("FaceCover", "headwear"),
    ];
    let named = |class: &String| SORTS.iter().find(|(name, _)| name == class).map(|(_, sort)| *sort);
    item.kind.class_names.iter().find_map(named).unwrap_or("thing")
}

/// Makes the pictures: a plain shape for each sort of thing, drawn here
/// out of a few boxes and rounds. They stand in for the game's own
/// pictures of its items, which are not read; nothing of them is the
/// game's.
fn make_icons(mut images: ResMut<Assets<Image>>, mut carried: ResMut<Carried>) {
    // Each shape: whether it is round, then left, top, right and bottom
    // as shares of the picture.
    type Shape = (bool, [f32; 4]);
    let sorts: [(&'static str, &[Shape]); 11] = [
        ("weapon", &[(false, [0.04, 0.44, 0.62, 0.50]), (false, [0.30, 0.40, 0.72, 0.58]), (false, [0.70, 0.44, 0.97, 0.60]), (false, [0.44, 0.58, 0.54, 0.88]), (false, [0.62, 0.58, 0.68, 0.78])]),
        ("magazine", &[(false, [0.36, 0.14, 0.64, 0.90]), (false, [0.30, 0.10, 0.70, 0.22])]),
        ("ammo", &[(true, [0.16, 0.14, 0.32, 0.56]), (false, [0.16, 0.36, 0.32, 0.88]), (true, [0.42, 0.14, 0.58, 0.56]), (false, [0.42, 0.36, 0.58, 0.88]), (true, [0.68, 0.14, 0.84, 0.56]), (false, [0.68, 0.36, 0.84, 0.88])]),
        ("headwear", &[(true, [0.14, 0.16, 0.86, 1.30]), (false, [0.08, 0.72, 0.92, 0.82])]),
        ("vest", &[(false, [0.26, 0.28, 0.74, 0.92]), (false, [0.26, 0.08, 0.38, 0.30]), (false, [0.62, 0.08, 0.74, 0.30])]),
        ("backpack", &[(true, [0.22, 0.08, 0.78, 0.52]), (false, [0.22, 0.30, 0.78, 0.92]), (false, [0.12, 0.34, 0.18, 0.82]), (false, [0.82, 0.34, 0.88, 0.82])]),
        ("meds", &[(false, [0.40, 0.14, 0.60, 0.86]), (false, [0.14, 0.40, 0.86, 0.60])]),
        ("food", &[(false, [0.32, 0.22, 0.68, 0.88]), (true, [0.32, 0.12, 0.68, 0.32])]),
        ("headset", &[(false, [0.16, 0.14, 0.84, 0.22]), (false, [0.16, 0.14, 0.24, 0.50]), (false, [0.76, 0.14, 0.84, 0.50]), (false, [0.10, 0.46, 0.30, 0.86]), (false, [0.70, 0.46, 0.90, 0.86])]),
        ("knife", &[(false, [0.08, 0.45, 0.62, 0.54]), (false, [0.62, 0.40, 0.92, 0.60]), (false, [0.58, 0.34, 0.63, 0.66])]),
        ("part", &[(false, [0.18, 0.40, 0.82, 0.60]), (true, [0.38, 0.26, 0.62, 0.74])]),
    ];
    // The longer side of a picture, in pixels.
    const SIDE: f32 = 128.0;
    let mut paint = |shapes: &[Shape]| {
        // The picture is as large as its shapes reach and no larger, so
        // that it has the thing's own proportions: a rifle is long, a
        // magazine tall.
        let reach = |pick: fn(&[f32; 4]) -> f32, most: bool| {
            let all = shapes.iter().map(|(_, sides)| pick(sides));
            if most { all.fold(0.0, f32::max) } else { all.fold(1.0, f32::min) }
        };
        let (left, top) = (reach(|s| s[0], false), reach(|s| s[1], false).max(0.0));
        let (right, bottom) = (reach(|s| s[2], true), reach(|s| s[3], true).min(1.0));
        let (across, down) = ((right - left).max(0.01), (bottom - top).max(0.01));
        let scale = SIDE / across.max(down);
        let (width, height) = (((across * scale).round() as usize).max(1), ((down * scale).round() as usize).max(1));
        let inside = |x: usize, y: usize| {
            let (u, v) = (left + (x as f32 + 0.5) / width as f32 * across, top + (y as f32 + 0.5) / height as f32 * down);
            shapes.iter().any(|(round, [left, top, right, bottom])| match round {
                false => u >= *left && u <= *right && v >= *top && v <= *bottom,
                true => (((u - (left + right) / 2.0) / ((right - left) / 2.0)).powi(2) + ((v - (top + bottom) / 2.0) / ((bottom - top) / 2.0)).powi(2)) <= 1.0,
            })
        };
        // As it lies, and turned a quarter to the right for a thing that
        // lies turned in its grid.
        let mut image = |width: usize, height: usize, at: &dyn Fn(usize, usize) -> bool| {
            let mut data = vec![0u8; width * height * 4];
            for (index, pixel) in data.chunks_exact_mut(4).enumerate() {
                if at(index % width, index / width) {
                    pixel.copy_from_slice(&[196, 200, 184, 120]);
                }
            }
            let size = Extent3d { width: width as u32, height: height as u32, depth_or_array_layers: 1 };
            images.add(Image::new(size, TextureDimension::D2, data, TextureFormat::Rgba8UnormSrgb, RenderAssetUsages::default()))
        };
        Picture {
            upright: image(width, height, &inside),
            turned: image(height, width, &|x, y| inside(y, height - 1 - x)),
            shape: width as f32 / height as f32,
            fills: false,
        }
    };
    for (sort, shapes) in sorts {
        let picture = paint(shapes);
        carried.icons.insert(sort, picture);
    }
    let plain = paint(&[(false, [0.22, 0.22, 0.78, 0.78])]);
    carried.icons.insert("thing", plain);
}

/// How much of an item's box its picture may take, each way.
const PICTURE_SHARE: f32 = 0.86;

/// An item as drawn: a box with its picture, its name, the stack and,
/// where there is room for it, its weight. `size` is the box's, in pixels,
/// and `turned` whether the thing lies turned a quarter. The picture keeps
/// its own proportions: it is drawn in the middle of the box at the
/// largest size that fits, so a larger box has a larger picture and never
/// a stretched one.
fn item_view(parent: &mut ChildSpawnerCommands, carried: &Carried, item: &Item, node: Node, wide: bool, size: Vec2, turned: bool, layer: u8) {
    let stack = if item.count > 1 { format!(" x{}", item.count) } else { String::new() };
    let weight = if wide { format!("   {:.2} kg", item.weight()) } else { String::new() };
    // The key it is bound to, as the game marks a bound thing.
    let key = carried.bound.key_of(item.id).map(|key| format!("   [{}]", QUICK_KEYS[key].1)).unwrap_or_default();
    let node = Node { border: UiRect::all(Val::Px(1.0)), padding: UiRect::all(Val::Px(2.0)), overflow: Overflow::clip(), ..node };
    parent.spawn((ItemView(item.id, layer), RelativeCursorPosition::default(), node, BorderColor(LINE), BackgroundColor(THING))).with_children(|thing| {
        picture_in(thing, carried, item, size, turned);
        thing.spawn(words(format!("{}{stack}{weight}{key}", carried.name(item)), 10.0, WORDS));
    });
}

/// An item's picture, whole in the middle of a box of that size.
fn picture_in(thing: &mut ChildSpawnerCommands, carried: &Carried, item: &Item, size: Vec2, turned: bool) {
    // The thing's own picture where it has been made; until then, the
    // plain shape of its sort.
    let Some(picture) = carried.pictures.get(&item.kind.template).or_else(|| carried.icons.get(icon_of(item))) else { return };
    // The box's inside, within its border.
    let room = (size - Vec2::splat(2.0)).max(Vec2::ONE);
    let shape = if turned { 1.0 / picture.shape } else { picture.shape };
    let share = if picture.fills { 1.0 } else { PICTURE_SHARE };
    let height = (room.y * share).min(room.x * share / shape);
    let width = height * shape;
    let over = placed((room.x - width) / 2.0, (room.y - height) / 2.0, width, height);
    let image = if turned { &picture.turned } else { &picture.upright };
    thing.spawn((ImageNode::new(image.clone()), over));
}

/// A slot as drawn: whatever is in it, filling it. `size` is the slot's,
/// in pixels.
fn slot_box(parent: &mut ChildSpawnerCommands, carried: &Carried, holder: &Item, slot: usize, node: Node, size: Vec2, layer: u8) {
    let node = Node { border: UiRect::all(Val::Px(1.0)), ..node };
    parent.spawn((SlotBox { holder: holder.id, slot, layer }, RelativeCursorPosition::default(), node, BorderColor(LINE), BackgroundColor(SLOT))).with_children(|view| {
        if let Some(item) = holder.slots.get(slot).and_then(Option::as_ref) {
            let inside = (size - Vec2::splat(2.0)).max(Vec2::ONE);
            item_view(view, carried, item, Node { width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() }, true, inside, false, layer);
        }
    });
}

/// A grid as drawn: its cells, and each thing in it where it lies.
fn grid_box(parent: &mut ChildSpawnerCommands, carried: &Carried, holder: &Item, grid: usize, cell: f32, layer: u8) {
    let cells = holder.kind.grids[grid].cells;
    let (width, height) = (cells[0] as f32 * cell, cells[1] as f32 * cell);
    let node = Node { width: Val::Px(width), height: Val::Px(height), flex_shrink: 0.0, ..default() };
    let outline = Outline::new(Val::Px(1.0), Val::ZERO, LINE);
    parent.spawn((GridBox { holder: holder.id, grid, cells, layer }, RelativeCursorPosition::default(), node, outline, BackgroundColor(BACK))).with_children(|inside| {
        for column in 1..cells[0] {
            inside.spawn((placed(column as f32 * cell, 0.0, 1.0, height), BackgroundColor(PANEL)));
        }
        for row in 1..cells[1] {
            inside.spawn((placed(0.0, row as f32 * cell, width, 1.0), BackgroundColor(PANEL)));
        }
        for (place, item) in &holder.grids[grid] {
            let [w, h] = item.cells();
            let (w, h) = if place.turned { (h, w) } else { (w, h) };
            let size = Vec2::new(w as f32 * cell - 2.0, h as f32 * cell - 2.0);
            let node = placed(place.x as f32 * cell + 1.0, place.y as f32 * cell + 1.0, size.x, size.y);
            item_view(inside, carried, item, node, false, size, place.turned, layer);
        }
    });
}

/// What the mouse and the keys do in the screen, in the app's own words.
const HELP: &str = "Hold a thing with the left button and let it go on a cell, a slot or another thing; R turns it.\nCtrl and a click sends it to the other side. Alt and a click puts it on. A double click opens a container or shows what a thing is.\nCtrl held as a stack is let go asks how many.\nA quick-use key let go over a thing in the vest or the pockets binds it.\nThe discard key puts the thing under the mouse down.";

/// The quick-use keys in the bar's order: the game's name for each
/// binding and the word for it where the install's bindings have none.
const QUICK_KEYS: [(&str, &str); 7] = [("Slot4", "4"), ("Slot5", "5"), ("Slot6", "6"), ("Slot7", "7"), ("Slot8", "8"), ("Slot9", "9"), ("Slot0", "0")];

/// The quick-use panel where the install's scene could not be read: its
/// width and height, how far its lower edge is over the screen's and its
/// middle to the right of the screen's, and the side and step of its
/// views, as the install's scene had them on 2026-10-12.
const QUICK_PANEL: [f32; 4] = [854.2, 115.0, 30.0, 25.6];
const QUICK_SIDE: f32 = 64.0;
const QUICK_STEP: f32 = 70.0;

/// The quick-use bar (`InventoryScreenQuickAccessPanel`): a view for the
/// grenade key, one for each of the four slots that have a key of their
/// own (the sheath, the holster, the sling, the back) showing what is in
/// the slot, and one for each of the seven keys things are bound to.
/// Where each sits along the row is the install's scene's; it is shown
/// and nothing is done to it with the mouse.
fn quick_bar(commands: &mut Commands, carried: &Carried, bindings: &Bindings, units: Units, window: Vec2) {
    let s = units.0;
    let [width, height, up, right] = if carried.panel.quick[0] > 0.0 { carried.panel.quick } else { QUICK_PANEL };
    // The views by the scene's own key for each, left to right.
    let read = &carried.panel.quick_slots;
    let fallback: Vec<(String, f32, f32)> = ["G", "V", "1", "2", "3", "4", "5", "6", "7", "8", "9", "10"]
        .iter()
        .enumerate()
        .map(|(at, key)| (key.to_string(), at as f32 * QUICK_STEP + if at > 4 { 10.0 } else { 0.0 }, QUICK_SIDE))
        .collect();
    let views = if read.is_empty() { &fallback } else { read };
    let (first, last) = (views.iter().map(|(_, middle, side)| middle - side / 2.0).fold(f32::MAX, f32::min), views.iter().map(|(_, middle, side)| middle + side / 2.0).fold(f32::MIN, f32::max));
    let margin = (width - (last - first)) / 2.0 - first;
    let node = placed(window.x / 2.0 + (right - width / 2.0) * s, window.y - (up + height) * s, width * s, height * s);
    let worn = &carried.equipment.item;
    commands.spawn((Overlay, Cover(BAR_LAYER), RelativeCursorPosition::default(), node, BackgroundColor(PANEL.with_alpha(0.96)), GlobalZIndex(12))).with_children(|bar| {
        bar.spawn((placed(8.0 * s, 4.0 * s, 300.0 * s, 18.0 * s), words("QUICK USE", 10.0, FAINT)));
        for (key, middle, side) in views {
            // What the view is for: a slot with a key of its own, or one
            // of the seven keys.
            let (binding, item) = match key.as_str() {
                "G" => ("ThrowGrenade", None),
                "V" => ("Knife", worn.in_slot("Scabbard")),
                "1" => ("SecondaryWeapon", worn.in_slot("Holster")),
                "2" => ("PrimaryWeaponFirst", worn.in_slot("FirstPrimaryWeapon")),
                "3" => ("PrimaryWeaponSecond", worn.in_slot("SecondPrimaryWeapon")),
                other => match other.parse::<usize>().ok().and_then(|number| number.checked_sub(4)).filter(|at| *at < QUICK_KEYS.len()) {
                    Some(at) => (QUICK_KEYS[at].0, carried.bound.items[at].and_then(|id| worn.find(id))),
                    None => continue,
                },
            };
            let caption = match bindings.caption(binding) {
                "" => QUICK_KEYS.iter().find(|(name, _)| *name == binding).map_or(key.as_str(), |(_, word)| word).to_owned(),
                word => word.to_owned(),
            };
            let size = Vec2::splat(side * s);
            let view = placed((middle - side / 2.0 + margin) * s, (height - side - 10.0) * s, size.x, size.y);
            bar.spawn((Node { border: UiRect::all(Val::Px(1.0)), overflow: Overflow::clip(), ..view }, BorderColor(LINE), BackgroundColor(SLOT))).with_children(|view| {
                if let Some(item) = item {
                    picture_in(view, carried, item, size, false);
                    view.spawn((placed(2.0, size.y - 16.0, size.x - 4.0, 14.0), words(carried.name(item), 9.0, WORDS)));
                }
                view.spawn((placed(3.0, 1.0, size.x - 6.0, 14.0), words(caption, 10.0, FAINT)));
            });
        }
    });
}

/// A window over the page (`GridWindow`, `InfoWindow`): a caption with
/// the thing's name, by which it is moved, a button that shuts it, and
/// under it the container's grids or what the thing is. The window whose
/// container Ctrl and a click sends things to has its caption lit.
fn window_box(commands: &mut Commands, carried: &Carried, window: Win, index: usize, units: Units) {
    let Some(item) = carried.find(window.item) else { return };
    let (cell, layer) = (units.cell(), index as u8 + 1);
    let lit = window.grids && carried.priority == Some(window.item);
    let root = Node { position_type: PositionType::Absolute, left: Val::Px(window.at.x), top: Val::Px(window.at.y), flex_direction: FlexDirection::Column, border: UiRect::all(Val::Px(1.0)), ..default() };
    commands
        .spawn((Overlay, WinRoot(index), Cover(layer), RelativeCursorPosition::default(), root, BorderColor(LINE), BackgroundColor(PANEL), GlobalZIndex(13 + index as i32)))
        .with_children(|inside| {
            let caption = Node { justify_content: JustifyContent::SpaceBetween, align_items: AlignItems::Center, column_gap: Val::Px(16.0), padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)), ..default() };
            inside.spawn((WinCaption(index), RelativeCursorPosition::default(), caption, BackgroundColor(if lit { FAINT } else { LINE }))).with_children(|caption| {
                caption.spawn(words(carried.name(item).to_uppercase(), 11.0, WORDS));
                caption.spawn((WinClose(index), Button, Node { padding: UiRect::horizontal(Val::Px(6.0)), ..default() }, BackgroundColor(PANEL))).with_children(|close| {
                    close.spawn(words("X", 11.0, WORDS));
                });
            });
            let body = Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(4.0), padding: UiRect::all(Val::Px(6.0)), ..default() };
            inside.spawn(body).with_children(|body| {
                if window.grids {
                    let wrap = Node { flex_wrap: FlexWrap::Wrap, column_gap: Val::Px(5.0), row_gap: Val::Px(5.0), max_width: Val::Px(8.0 * cell + 20.0), ..default() };
                    body.spawn(wrap).with_children(|grids| {
                        for grid in 0..item.kind.grids.len() {
                            grid_box(grids, carried, item, grid, cell, layer);
                        }
                        for special in 0..item.kind.slots.len() {
                            slot_box(grids, carried, item, special, Node { width: Val::Px(cell), height: Val::Px(cell), ..default() }, Vec2::splat(cell), layer);
                        }
                    });
                    return;
                }
                // What the thing is, as far as the database says it: the
                // game's own window has its description, its figures and
                // its picture, none of which is read.
                let [width, height] = item.cells();
                let sort: Vec<&str> = item.kind.class_names.iter().skip(1).take(3).map(String::as_str).collect();
                let lines = [
                    item.kind.name.clone(),
                    sort.join("  <  "),
                    format!("{:.3} kg      {width} x {height} cells", item.weight()),
                    if item.kind.stack_max > 1 { format!("{} of {} to a stack", item.count, item.kind.stack_max) } else { String::new() },
                ];
                for line in lines.into_iter().filter(|line| !line.is_empty()) {
                    body.spawn((Node { max_width: Val::Px(320.0), ..default() }, words(line, 12.0, WORDS)));
                }
            });
        });
}

/// The window that asks how many of a stack (`SplitDialog`, with its
/// stepped slider): the figure, the slider with a stop at each of the
/// game's notches, a button to go through with it and one to drop it.
fn asking_box(commands: &mut Commands, asking: &Asking, window: Vec2) {
    const WIDTH: f32 = 260.0;
    let stops = Asking::stops(asking.most);
    let reached = stops.iter().rposition(|stop| *stop <= asking.count).unwrap_or(0);
    let share = if stops.len() > 1 { reached as f32 / (stops.len() - 1) as f32 } else { 1.0 };
    let at = asking.at.min(window - Vec2::new(WIDTH + 8.0, 120.0)).max(Vec2::ZERO);
    let root = Node {
        position_type: PositionType::Absolute,
        left: Val::Px(at.x),
        top: Val::Px(at.y),
        width: Val::Px(WIDTH),
        flex_direction: FlexDirection::Column,
        row_gap: Val::Px(8.0),
        padding: UiRect::all(Val::Px(8.0)),
        border: UiRect::all(Val::Px(1.0)),
        ..default()
    };
    commands.spawn((Overlay, Cover(ASKING_LAYER), RelativeCursorPosition::default(), root, BorderColor(WORDS), BackgroundColor(PANEL), GlobalZIndex(30))).with_children(|inside| {
        inside.spawn(Node { justify_content: JustifyContent::SpaceBetween, ..default() }).with_children(|top| {
            top.spawn(words(asking.title, 12.0, WORDS));
            top.spawn((AskClose, Button, Node { padding: UiRect::horizontal(Val::Px(6.0)), ..default() }, BackgroundColor(LINE))).with_children(|close| {
                close.spawn(words("X", 11.0, WORDS));
            });
        });
        inside.spawn(words(format!("{}   of {}", asking.count, asking.most), 16.0, WORDS));
        let bar = Node { width: Val::Percent(100.0), height: Val::Px(14.0), ..default() };
        inside.spawn((AskBar, RelativeCursorPosition::default(), bar, BackgroundColor(BACK))).with_children(|bar| {
            bar.spawn((Node { width: Val::Percent(share * 100.0), height: Val::Percent(100.0), ..default() }, BackgroundColor(FAINT)));
        });
        inside.spawn((AskAccept, Button, Node { padding: UiRect::axes(Val::Px(14.0), Val::Px(5.0)), align_self: AlignSelf::Center, ..default() }, BackgroundColor(LINE))).with_children(|accept| {
            accept.spawn(words("ACCEPT", 12.0, WORDS));
        });
        inside.spawn(words("Slide, or type a figure.   Enter goes through with it, Escape drops it.", 10.0, FAINT));
    });
}

/// The Gear page: the game's three parts side by side (`ItemsPanel`). On
/// the left the equipment's slot views where the game's own panel has
/// them, over a figure; in the middle each container in its slot with its
/// grids beside it; on the right what is open, in the width of the game's
/// stash panel.
fn gear_page(gear: &mut ChildSpawnerCommands, carried: &Carried, units: Units, scroll: f32) {
    let (s, cell) = (units.0, units.cell());
    let worn = &carried.equipment.item;
    let size = if carried.panel.slots.is_empty() { PANEL_SIZE } else { carried.panel.size };
    let left = Node {
        width: Val::Px((size[0] + 60.0) * s),
        flex_direction: FlexDirection::Column,
        align_items: AlignItems::Center,
        row_gap: Val::Px(8.0),
        padding: UiRect::vertical(Val::Px(6.0)),
        flex_shrink: 0.0,
        ..default()
    };
    gear.spawn((left, BackgroundColor(PANEL))).with_children(|left| {
        let panel = Node { width: Val::Px(size[0] * s), height: Val::Px(size[1] * s), flex_shrink: 0.0, ..default() };
        left.spawn(panel).with_children(|doll| {
            for [left, top, width, height, round] in FIGURE {
                doll.spawn((placed(left * s, top * s, width * s, height * s), BackgroundColor(SHAPE), BorderRadius::all(Val::Px(round * s))));
            }
            for (slot, caption, own) in EQUIPMENT {
                let Some(index) = worn.slot_index(slot) else { continue };
                let [left, top, width, height] = carried.panel.slot(slot).unwrap_or(own);
                // The caption sits over the view, as the game's header does.
                doll.spawn((placed(left * s, (top - CAPTION) * s, width * s, CAPTION * s), words(caption, 10.0, FAINT)));
                slot_box(doll, carried, worn, index, placed(left * s, top * s, width * s, height * s), Vec2::new(width * s, height * s), 0);
            }
        });
        // The carried weight, which is what the controller goes by.
        left.spawn(words(format!("WEIGHT   {:.1} kg", carried.equipment.carried_weight(false)), 14.0, WORDS));
        left.spawn((Node { max_width: Val::Px(size[0] * s), ..default() }, words(carried.note.clone(), 12.0, FAINT)));
        // What the mouse and the keys do here, where the quick-use bar
        // does not reach.
        left.spawn((Node { max_width: Val::Px(size[0] * s), ..default() }, words(HELP, 10.0, FAINT)));
    });
    // The containers: each in its slot, with its grids beside it.
    let middle = Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(6.0), flex_grow: 1.0, flex_shrink: 1.0, min_width: Val::Px(0.0), overflow: Overflow::clip(), ..default() };
    gear.spawn(middle).with_children(|column| {
        for (slot, caption) in CONTAINERS {
            let Some(index) = worn.slot_index(slot) else { continue };
            column.spawn(words(caption, 10.0, FAINT));
            column.spawn(Node { column_gap: Val::Px(8.0), align_items: AlignItems::FlexStart, ..default() }).with_children(|row| {
                let side = Val::Px(SLOT_SIDE * s);
                slot_box(row, carried, worn, index, Node { width: side, height: side, flex_shrink: 0.0, ..default() }, Vec2::splat(SLOT_SIDE * s), 0);
                let Some(item) = worn.slots[index].as_ref() else { return };
                row.spawn(Node { flex_wrap: FlexWrap::Wrap, column_gap: Val::Px(5.0), row_gap: Val::Px(5.0), ..default() }).with_children(|grids| {
                    for grid in 0..item.kind.grids.len() {
                        grid_box(grids, carried, item, grid, cell, 0);
                    }
                    // Special slots (the pockets have three).
                    for special in 0..item.kind.slots.len() {
                        slot_box(grids, carried, item, special, Node { width: Val::Px(cell), height: Val::Px(cell), ..default() }, Vec2::splat(cell), 0);
                    }
                });
            });
        }
    });
    // What is open beside them: the crate, or nothing. Its grids are in a
    // panel as high as the screen leaves, scrolled by the wheel or by the
    // bar at its side when they are higher than that.
    let right = Node { width: Val::Px(STASH_WIDTH * s), flex_direction: FlexDirection::Column, row_gap: Val::Px(6.0), flex_shrink: 0.0, ..default() };
    gear.spawn(right).with_children(|view| {
        if !carried.loot_open {
            view.spawn(words("Nothing is open. Look at a container on the range from close by and press the key that works it.", 11.0, FAINT));
            return;
        }
        view.spawn(words(carried.name(&carried.loot).to_uppercase(), 11.0, FAINT));
        let grids = &carried.loot.kind.grids;
        let whole: f32 = grids.iter().map(|grid| grid.cells[1] as f32 * cell).sum::<f32>() + GRID_GAP * grids.len().saturating_sub(1) as f32;
        // From under its caption down to the quick-use bar.
        let shown = ((SCREEN_HEIGHT - QUICK_PANEL[1] - QUICK_PANEL[3]) * s - LOOT_PANEL_TOP).max(cell);
        let scrolled = scroll.clamp(0.0, (whole - shown).max(0.0));
        view.spawn(Node { column_gap: Val::Px(6.0), height: Val::Px(shown.min(whole)), flex_shrink: 0.0, ..default() }).with_children(|row| {
            let area = Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(GRID_GAP),
                height: Val::Percent(100.0),
                overflow: Overflow::scroll_y(),
                flex_shrink: 0.0,
                ..default()
            };
            row.spawn((LootArea { shown, whole }, area, ScrollPosition { offset_x: 0.0, offset_y: scrolled }, RelativeCursorPosition::default())).with_children(|area| {
                for grid in 0..grids.len() {
                    grid_box(area, carried, &carried.loot, grid, cell, 0);
                }
            });
            // The bar: a track as high as the panel and a thumb as high
            // as the share of the grids that shows.
            if whole > shown {
                let track = Node { width: Val::Px(SCROLL_BAR), height: Val::Percent(100.0), flex_shrink: 0.0, ..default() };
                row.spawn((track, BackgroundColor(PANEL))).with_children(|track| {
                    let thumb = Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(0.0),
                        top: Val::Px(scrolled / whole * shown),
                        width: Val::Percent(100.0),
                        height: Val::Px(shown / whole * shown),
                        ..default()
                    };
                    track.spawn((LootThumb { shown, whole }, thumb, BackgroundColor(LINE), RelativeCursorPosition::default()));
                });
            }
        });
    });
}

/// How far down the window the crate's panel begins, in pixels: the
/// screen's own margin, the row of tabs and the panel's caption.
const LOOT_PANEL_TOP: f32 = 134.0;
/// The gap between two grids of one container, and how wide the scroll
/// bar is, in pixels. The app's.
const GRID_GAP: f32 = 5.0;
const SCROLL_BAR: f32 = 12.0;

/// The crate's panel is scrolled: by the wheel while the mouse is over
/// it, a cell a notch, and by its bar's thumb held with the left button.
/// The grids are the container's own (`Overflow::scroll_y`); this keeps
/// how far they are scrolled and puts the thumb where that is.
fn scroll_loot(
    controller: Res<Controller>,
    wheel: Res<bevy::input::mouse::AccumulatedMouseScroll>,
    buttons: Res<ButtonInput<MouseButton>>,
    window: Query<&Window, With<PrimaryWindow>>,
    units: Res<Units>,
    mut scroll: ResMut<LootScroll>,
    mut area: Query<(&LootArea, &mut ScrollPosition, &RelativeCursorPosition)>,
    mut thumb: Query<(&LootThumb, &mut Node, &RelativeCursorPosition)>,
    mut held: Local<Option<f32>>,
) {
    if !controller.inventory_open {
        *held = None;
        return;
    }
    let Ok((sizes, mut position, over)) = area.single_mut() else { return };
    let most = (sizes.whole - sizes.shown).max(0.0);
    let mut to = scroll.0;
    if over.mouse_over() && wheel.delta.y != 0.0 {
        to -= wheel.delta.y.signum() * units.cell();
    }
    // The thumb held: it follows the mouse, and the grids with it.
    let cursor = window.single().ok().and_then(|window| window.cursor_position()).map(|at| at.y);
    if let Ok((bar, _, over)) = thumb.single() {
        if buttons.just_pressed(MouseButton::Left) && over.mouse_over() {
            *held = cursor.map(|y| y - scroll.0 / bar.whole * bar.shown);
        }
        if !buttons.pressed(MouseButton::Left) {
            *held = None;
        }
        if let (Some(grabbed), Some(y)) = (*held, cursor) {
            to = (y - grabbed) / bar.shown * bar.whole;
        }
    }
    let to = to.clamp(0.0, most);
    if to != scroll.0 {
        scroll.0 = to;
    }
    if position.offset_y != to {
        position.offset_y = to;
    }
    if let Ok((bar, mut node, _)) = thumb.single_mut() {
        let top = Val::Px(to / bar.whole * bar.shown);
        if node.top != top {
            node.top = top;
        }
    }
}

/// Draws the Gear page afresh when something has moved or been opened,
/// or the window has another height; and with it what lies over the page
/// while the screen is open on that tab: the quick-use bar, the windows,
/// the window that asks how many.
fn rebuild(
    mut commands: Commands,
    carried: Res<Carried>,
    controller: Res<Controller>,
    selected: Res<Selected>,
    bindings: Res<Bindings>,
    scroll: Res<LootScroll>,
    window: Query<&Window, With<PrimaryWindow>>,
    mut units: ResMut<Units>,
    page: Query<Entity, With<GearPage>>,
    over: Query<Entity, With<Overlay>>,
    mut was: Local<(bool, usize)>,
    mut afresh: Local<u32>,
) {
    let (now, size) = window.single().map_or((*units, Vec2::ZERO), |window| (Units(window.height() / SCREEN_HEIGHT), Vec2::new(window.width(), window.height())));
    let shows = (controller.inventory_open, selected.0);
    if !carried.is_changed() && now == *units && shows == *was {
        *afresh = 0;
        return;
    }
    // Drawn afresh on frame after frame, something writes to what is
    // carried on every frame, and whatever goes by the mouse being over a
    // part of the page stops working (a part just made does not know
    // where the mouse is): said once, for whoever did it.
    *afresh = if carried.is_changed() { *afresh + 1 } else { 0 };
    if *afresh == 120 {
        warn!("the inventory's page has been drawn afresh on 120 frames running: something writes to what is carried on every frame");
    }
    (*units, *was) = (now, shows);
    for page in &page {
        commands.entity(page).despawn_related::<Children>().with_children(|gear| gear_page(gear, &carried, now, scroll.0));
    }
    for entity in &over {
        commands.entity(entity).despawn();
    }
    if shows != (true, GEAR) {
        return;
    }
    quick_bar(&mut commands, &carried, &bindings, now, size);
    for (index, window) in carried.windows.iter().enumerate() {
        window_box(&mut commands, &carried, *window, index, now);
    }
    if let Some(asking) = &carried.asking {
        asking_box(&mut commands, asking, size);
    }
}

/// The drags a run nobody is at the keys of was told to make (`--drag`):
/// one after another from a second and a half in, each with the mouse put
/// at its start, the left button pressed, the mouse moved to its end and
/// the button let go, a few frames apart so that the screen has seen
/// where the mouse is before the button changes. The mouse is the
/// window's own, so the real pointer is moved with it. The button is
/// pressed as a mouse's is, by the engine's own event for it, which the
/// next frame hears from its start: a button of the screen (a tab, a
/// window's cross, the options' cells) takes the click as it would a
/// hand's.
fn scripted_drags(
    options: Res<Options>,
    mut window: Query<(Entity, &mut Window), With<PrimaryWindow>>,
    mut buttons: EventWriter<bevy::input::mouse::MouseButtonInput>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut controller: ResMut<Controller>,
    mut wheel: EventWriter<bevy::input::mouse::MouseWheel>,
    mut mouse: ResMut<crate::keys::ScriptedMouse>,
    mut frame: Local<u32>,
    mut notches_left: Local<i32>,
) {
    const START: u32 = 90;
    const EACH: u32 = 16;
    if !options.unattended() {
        return;
    }
    *frame += 1;
    // The mouse's travel the run was told (`--mouse`): the bindings hear
    // it on the next frame in place of the real mouse's, which a run
    // nobody is at the keys of never listens to.
    let travel = options.mouse.iter().filter(|(from, frames, _)| (*from..from + frames).contains(&*frame)).map(|(.., counts)| *counts).sum();
    mouse.0 = Some(travel);
    // The wheel's notches the run was told to turn (`--wheel`), up
    // counted as positive: the menu's while the character plays. While
    // the screen is open they are the engine's own events for a wheel, a
    // notch a frame from then on, which the screen hears as it hears a
    // hand's: the crate's panel is scrolled if the mouse is over it (a
    // `--drag` from a place onto itself puts it there).
    let Ok((entity, mut window)) = window.single_mut() else { return };
    for (_, notches) in options.wheel.iter().filter(|(at, _)| *frame == *at) {
        if controller.inventory_open {
            *notches_left += notches;
        } else {
            controller.menu_notches += notches;
        }
    }
    if *notches_left != 0 {
        let notch = notches_left.signum();
        wheel.write(bevy::input::mouse::MouseWheel { unit: bevy::input::mouse::MouseScrollUnit::Line, x: 0.0, y: notch as f32, window: entity });
        *notches_left -= notch;
    }
    // The keys the run was told to press (`--tap`), by the same count.
    let mut button = |button: MouseButton, state: bevy::input::ButtonState| {
        buttons.write(bevy::input::mouse::MouseButtonInput { button, state, window: entity });
    };
    for (at, key, held) in &options.taps {
        let (down, up) = (*frame == *at, *frame == at + held);
        match key {
            crate::keys::Key::Board(key) if down => keys.press(*key),
            crate::keys::Key::Board(key) if up => keys.release(*key),
            crate::keys::Key::Mouse(pressed) if down => button(*pressed, bevy::input::ButtonState::Pressed),
            crate::keys::Key::Mouse(pressed) if up => button(*pressed, bevy::input::ButtonState::Released),
            _ => {}
        }
    }
    let Some(since) = frame.checked_sub(START) else { return };
    let Some([x, y, to_x, to_y]) = options.drags.get((since / EACH) as usize).copied() else { return };
    match since % EACH {
        0 => window.set_cursor_position(Some(Vec2::new(x, y))),
        4 => button(MouseButton::Left, bevy::input::ButtonState::Pressed),
        6 => window.set_cursor_position(Some(Vec2::new((x + to_x) / 2.0, (y + to_y) / 2.0))),
        8 => window.set_cursor_position(Some(Vec2::new(to_x, to_y))),
        12 => button(MouseButton::Left, bevy::input::ButtonState::Released),
        _ => {}
    }
}

/// What is drawn on top where the mouse is: the page (0), a window, the
/// bar or the window that asks how many.
fn cover_under<'a>(covers: impl Iterator<Item = (&'a Cover, &'a RelativeCursorPosition)>) -> u8 {
    covers.filter(|(_, at)| at.mouse_over()).map(|(cover, _)| cover.0).max().unwrap_or(0)
}

/// The thing under the mouse on what is drawn on top there: the smallest
/// is the one meant. With it, where in it the mouse is, as a share of it.
fn item_under<'a>(items: impl Iterator<Item = (&'a ItemView, &'a RelativeCursorPosition, &'a ComputedNode)>, cover: u8, but: Option<Id>) -> Option<(Id, Vec2)> {
    let over = items.filter(|(view, at, _)| view.1 == cover && Some(view.0) != but && at.mouse_over());
    over.min_by(|a, b| a.2.size().length_squared().total_cmp(&b.2.size().length_squared())).map(|(view, at, _)| (view.0, at.normalized.unwrap_or_default()))
}

/// A click on a thing (`ItemView.OnClick`, the left button): a double
/// click with neither Ctrl nor Shift held opens a container in a window
/// of its own and shows what anything else is (food and medicine it
/// would use, which is not built); else Ctrl sends the thing to the other
/// side, Shift to the sorting table (a raid has none), and Alt puts it on.
fn click(carried: &mut Carried, id: Id, double: bool, [ctrl, shift, alt]: [bool; 3], cursor: Vec2) {
    let Some(item) = carried.find(id) else { return };
    let name = carried.name(item);
    if double && !ctrl && !shift {
        if is(item, "FoodDrink") || is(item, "Meds") {
            carried.note = format!("{name}: a double click uses it in the game; there is no health system here yet");
            return;
        }
        let grids = Carried::opens(item);
        carried.open_window(id, grids, cursor + Vec2::new(14.0, 14.0));
        return;
    }
    let targets = carried.other_side(id);
    let roots = &mut [&mut carried.equipment.item, &mut carried.loot];
    let note = if ctrl {
        match quick_find(roots, id, &targets, Order::OTHER_SIDE, &carried.rules) {
            Ok((_, Applied::Merged)) => "merged".to_owned(),
            Ok((_, Applied::Topped(count))) => format!("{count} handed over"),
            Ok((_, Applied::Put(_))) => "sent over".to_owned(),
            Err(refusal) => why(&refusal),
        }
    } else if shift {
        "there is no sorting table in a raid".to_owned()
    } else if alt {
        match equip(roots, id, &carried.rules) {
            Ok(_) => "put on".to_owned(),
            Err(Refusal::AlreadyThere) => "it is on already".to_owned(),
            Err(Refusal::NoRoom) => "no free slot for it".to_owned(),
            Err(Refusal::NoAction) => "it is not a thing to put on".to_owned(),
            Err(refusal) => why(&refusal),
        }
    } else {
        return;
    };
    carried.note = format!("{name}: {note}");
    carried.tidy();
}

/// The mouse on the things of the screen (`ItemView`, `GridView` and
/// `SlotView` in the game). The button pressed and let go on a thing is a
/// click (`click`). Moved far enough with the button down, the mouse
/// takes the thing, carries it and lets it go: over another thing it is
/// dropped on that, over a grid it goes to the cell its top left corner
/// is nearest, over a slot into the slot. `R` turns it a quarter while it
/// is held. With Ctrl held as it is let go only part of a stack goes, and
/// where more than one may, a window asks how many (`ask`).
fn drag(
    time: Res<Time<Real>>,
    (buttons, keys): (Res<ButtonInput<MouseButton>>, Res<ButtonInput<KeyCode>>),
    bindings: Res<Bindings>,
    window: Query<&Window, With<PrimaryWindow>>,
    controller: Res<Controller>,
    mut carried: ResMut<Carried>,
    mut drag: ResMut<Drag>,
    mut clicks: ResMut<Clicks>,
    units: Res<Units>,
    camera: Query<&GlobalTransform, With<crate::weapon::Eye>>,
    (items, slots, grids, covers): (
        Query<(&ItemView, &RelativeCursorPosition, &ComputedNode)>,
        Query<(&SlotBox, &RelativeCursorPosition)>,
        Query<(&GridBox, &RelativeCursorPosition)>,
        Query<(&Cover, &RelativeCursorPosition)>,
    ),
    mut ghost: Query<(&mut Node, &mut Visibility, &mut BorderColor), With<Ghost>>,
    mut ghost_words: Query<&mut Text, With<GhostWords>>,
) {
    let Ok((mut node, mut shown, mut border)) = ghost.single_mut() else { return };
    let cell = units.cell();
    let cursor = window.single().ok().and_then(Window::cursor_position);
    // Nothing is taken with the screen shut, while the window that asks
    // how many is up, or while a window is being moved.
    let free = controller.inventory_open && carried.asking.is_none() && clicks.moving.is_none() && !clicks.sliding;
    let (true, Some(cursor)) = (free, cursor) else {
        (drag.0, clicks.pressed) = (None, None);
        *shown = Visibility::Hidden;
        return;
    };
    let cover = cover_under(covers.iter());
    let under = |but: Option<Id>| item_under(items.iter(), cover, but);
    let held_key = |left: KeyCode, right: KeyCode| keys.pressed(left) || keys.pressed(right);
    let (ctrl, shift, alt) = (held_key(KeyCode::ControlLeft, KeyCode::ControlRight), held_key(KeyCode::ShiftLeft, KeyCode::ShiftRight), held_key(KeyCode::AltLeft, KeyCode::AltRight));
    let left = MouseButton::Left;
    // The button goes down on a thing: a click in the making, the second
    // of two on the same thing within the engine's time a double one.
    if drag.0.is_none() && buttons.just_pressed(left) {
        clicks.pressed = under(None).map(|(id, at)| Pressed { id, cursor, at });
        let now = time.elapsed_secs_f64();
        clicks.last = clicks.pressed.map(|pressed| {
            let again = clicks.last.filter(|last| last.id == pressed.id && now - last.at < DOUBLE_CLICK);
            Clicked { id: pressed.id, at: now, count: again.map_or(1, |last| last.count + 1) }
        });
    }
    // Let go where it went down: a click.
    if let Some(pressed) = clicks.pressed.filter(|_| !buttons.pressed(left)) {
        clicks.pressed = None;
        let double = clicks.last.is_some_and(|last| last.id == pressed.id && last.count % 2 == 0);
        click(&mut carried, pressed.id, double, [ctrl, shift, alt], cursor);
        return;
    }
    // Moved far enough with the button down: the thing is taken.
    if let Some(pressed) = clicks.pressed.filter(|pressed| pressed.cursor.distance(cursor) > DRAG_THRESHOLD) {
        clicks.pressed = None;
        if let (Some(item), Some((_, spot))) = (carried.find(pressed.id), carried.lies(pressed.id)) {
            // In a grid the mouse holds it where it took hold of it;
            // out of a slot, by the middle of its first cell.
            let (turned, grab) = match spot {
                Spot::Cell(_, place) => {
                    let [w, h] = item.cells();
                    let size = if place.turned { Vec2::new(h as f32, w as f32) } else { Vec2::new(w as f32, h as f32) };
                    (place.turned, pressed.at * size * cell)
                }
                _ => (false, Vec2::splat(cell / 2.0)),
            };
            drag.0 = Some(Held { id: pressed.id, turned, grab, cells: item.cells(), name: carried.name(item) });
        }
    }
    // The key for it puts the thing held, or the one under the mouse,
    // down on the ground a step ahead of the character (the game's
    // `ThrowItem`, which a raid carries out without asking; the game
    // throws the thing and it falls where it falls).
    if bindings.fired("ThrowItem") {
        let which = drag.0.as_ref().map(|held| held.id).or_else(|| under(None).map(|(id, _)| id));
        // A backpack on the back is not put down at once: its slot is
        // one the game animates, and it goes by the body's gesture for
        // it, as it does by its own key (interactions spec, "A backpack
        // dropped").
        let worn_pack = carried.equipment.worn("Backpack").map(|pack| pack.id);
        if which.is_some() && which == worn_pack {
            carried.pack_asked = true;
            (drag.0, clicks.pressed) = (None, None);
        } else if let (Some(id), Ok(eye)) = (which, camera.single()) {
            let ahead = eye.forward().as_vec3().with_y(0.0).normalize_or_zero();
            let at = Vec3::new(eye.translation().x, controller.movement.position[1], eye.translation().z) + ahead * DROP_AHEAD;
            let carried = &mut *carried;
            let name = carried.find(id).map(|item| carried.name(item)).unwrap_or_default();
            match take_out(&mut [&mut carried.equipment.item, &mut carried.loot], id, &carried.rules) {
                Ok(item) => {
                    carried.ground.push((item, at));
                    carried.note = format!("{name}: put down");
                }
                Err(refusal) => carried.note = format!("{name}: {}", why(&refusal)),
            }
            carried.tidy();
            (drag.0, clicks.pressed) = (None, None);
        }
    }
    let Some(held) = drag.0.as_mut() else {
        *shown = Visibility::Hidden;
        return;
    };
    if keys.just_pressed(KeyCode::KeyR) && held.cells[0] != held.cells[1] {
        (held.turned, held.grab) = (!held.turned, Vec2::splat(cell / 2.0));
    }
    let [w, h] = if held.turned { [held.cells[1], held.cells[0]] } else { held.cells };
    // Where it would go if let go now, on what is drawn on top there.
    let on_cell = || {
        let (grid, at) = grids.iter().find(|(grid, at)| grid.layer == cover && at.mouse_over())?;
        let corner = (at.normalized? * Vec2::new(grid.cells[0] as f32, grid.cells[1] as f32) * cell - held.grab) / cell;
        let (x, y) = (corner.x.round().clamp(0.0, grid.cells[0] as f32), corner.y.round().clamp(0.0, grid.cells[1] as f32));
        Some(Target::Cell(grid.holder, grid.grid, Place { x: x as u32, y: y as u32, turned: held.turned }))
    };
    let on_slot = || slots.iter().find(|(slot, at)| slot.layer == cover && at.mouse_over()).map(|(slot, _)| Target::Slot(slot.holder, slot.slot));
    let target = under(Some(held.id)).map(|(onto, _)| Target::Item(onto)).or_else(on_cell).or_else(on_slot);
    // Tried on copies first, as the game tries a move before it shows
    // whether the place will take the thing. With Ctrl held it is part of
    // a stack that would go, and one is tried.
    let would = target.map(|target| {
        let (mut worn, mut loot) = (carried.equipment.item.clone(), carried.loot.clone());
        act(&mut [&mut worn, &mut loot], held.id, target, &carried.rules, ctrl.then_some(1))
    });
    *shown = Visibility::Inherited;
    (node.left, node.top) = (Val::Px(cursor.x - held.grab.x), Val::Px(cursor.y - held.grab.y));
    (node.width, node.height) = (Val::Px(w as f32 * cell - 2.0), Val::Px(h as f32 * cell - 2.0));
    border.0 = match &would {
        Some(Ok(_)) => Color::srgb(0.45, 0.80, 0.45),
        Some(Err(_)) => Color::srgb(0.85, 0.35, 0.30),
        None => WORDS,
    };
    if let Ok(mut text) = ghost_words.single_mut() {
        if **text != held.name {
            **text = held.name.clone();
        }
    }
    if !buttons.pressed(left) {
        let (id, name) = (held.id, held.name.clone());
        drag.0 = None;
        *shown = Visibility::Hidden;
        let Some(target) = target else { return };
        let carried = &mut *carried;
        let roots = &mut [&mut carried.equipment.item, &mut carried.loot];
        // With Ctrl held: how many may go. More than one, and the window
        // asks how many, beginning at half and one (`SplitDialog.Show`).
        let part = ctrl.then(|| most_of(roots, id, target));
        if let Some(most) = part.filter(|most| *most > 1) {
            let onto = match target {
                Target::Item(_) => true,
                Target::Slot(holder, slot) => in_slot(roots, holder, slot).is_some(),
                Target::Cell(..) => false,
            };
            carried.asking = Some(Asking { id, target, title: if onto { "TRANSFER" } else { "SPLIT" }, most, count: most / 2 + 1, at: cursor, typed: false });
            return;
        }
        let done = act(roots, id, target, &carried.rules, part);
        carried.note = match done {
            Ok(what) => format!("{name}: {what}"),
            Err(refusal) => format!("{name}: {}", why(&refusal)),
        };
        carried.tidy();
    }
}

/// The window that asks how many of a stack (`SplitDialog`): the slider
/// is slid with the mouse, a figure may be typed, Enter or its button
/// goes through with it (`ItemUiContext.TranslateCommand`), and Escape or
/// its cross drops it.
fn ask(
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    controller: Res<Controller>,
    mut carried: ResMut<Carried>,
    mut clicks: ResMut<Clicks>,
    bar: Query<&RelativeCursorPosition, With<AskBar>>,
    accept: Query<&Interaction, (With<AskAccept>, Changed<Interaction>)>,
    close: Query<&Interaction, (With<AskClose>, Changed<Interaction>)>,
) {
    let Some(mut asking) = carried.asking else {
        clicks.sliding = false;
        return;
    };
    let pressed = |interaction: &Interaction| *interaction == Interaction::Pressed;
    // Escape drops it, unless a window over the screen is open: that has
    // the key first, and this one the press after.
    let escape = keys.just_pressed(KeyCode::Escape) && carried.windows.is_empty();
    if !controller.inventory_open || escape || close.iter().any(pressed) {
        (carried.asking, clicks.sliding) = (None, false);
        return;
    }
    if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::NumpadEnter) || accept.iter().any(pressed) {
        let carried = &mut *carried;
        let name = carried.find(asking.id).map(|item| carried.name(item)).unwrap_or_default();
        let count = asking.count.clamp(1, asking.most);
        let done = act(&mut [&mut carried.equipment.item, &mut carried.loot], asking.id, asking.target, &carried.rules, Some(count));
        carried.note = match done {
            Ok(what) => format!("{name}: {count} {what}"),
            Err(refusal) => format!("{name}: {}", why(&refusal)),
        };
        (carried.asking, clicks.sliding) = (None, false);
        carried.tidy();
        return;
    }
    let before = asking;
    // The slider: held, it goes to the stop nearest the mouse.
    let over = bar.iter().next();
    if buttons.just_pressed(MouseButton::Left) && over.is_some_and(RelativeCursorPosition::mouse_over) {
        clicks.sliding = true;
    }
    if !buttons.pressed(MouseButton::Left) {
        clicks.sliding = false;
    }
    if let (true, Some(at)) = (clicks.sliding, over.and_then(|over| over.normalized)) {
        let stops = Asking::stops(asking.most);
        let stop = (at.x.clamp(0.0, 1.0) * (stops.len() - 1) as f32).round() as usize;
        (asking.count, asking.typed) = (stops[stop.min(stops.len() - 1)], false);
    }
    // A figure typed: the first key takes the place of what stood there.
    const DIGITS: [[KeyCode; 2]; 10] = [
        [KeyCode::Digit0, KeyCode::Numpad0],
        [KeyCode::Digit1, KeyCode::Numpad1],
        [KeyCode::Digit2, KeyCode::Numpad2],
        [KeyCode::Digit3, KeyCode::Numpad3],
        [KeyCode::Digit4, KeyCode::Numpad4],
        [KeyCode::Digit5, KeyCode::Numpad5],
        [KeyCode::Digit6, KeyCode::Numpad6],
        [KeyCode::Digit7, KeyCode::Numpad7],
        [KeyCode::Digit8, KeyCode::Numpad8],
        [KeyCode::Digit9, KeyCode::Numpad9],
    ];
    for (digit, codes) in DIGITS.iter().enumerate() {
        if codes.iter().any(|code| keys.just_pressed(*code)) {
            let so_far = if asking.typed { asking.count } else { 0 };
            (asking.count, asking.typed) = ((so_far * 10 + digit as u32).min(asking.most), true);
        }
    }
    if keys.just_pressed(KeyCode::Backspace) {
        (asking.count, asking.typed) = (asking.count / 10, true);
    }
    if asking != before {
        carried.asking = Some(asking);
    }
}

/// The windows over the page: the cross shuts one, a press on one brings
/// it to the top (and makes a container's the one Ctrl and a click sends
/// things to, as the install's `PriorityWindowMode` has it), and held by
/// its caption it is moved. They all shut with the screen
/// (`ItemUiContext.CloseAllWindows`).
fn windows(
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    window: Query<&Window, With<PrimaryWindow>>,
    controller: Res<Controller>,
    mut carried: ResMut<Carried>,
    mut clicks: ResMut<Clicks>,
    captions: Query<(&WinCaption, &RelativeCursorPosition)>,
    mut roots: Query<(&WinRoot, &mut Node, &RelativeCursorPosition)>,
    close: Query<(&Interaction, &WinClose), Changed<Interaction>>,
) {
    if !controller.inventory_open {
        clicks.moving = None;
        if !carried.windows.is_empty() || carried.priority.is_some() {
            carried.windows.clear();
            carried.priority = None;
        }
        return;
    }
    // Its cross shuts a window; Escape shuts the one on top
    // (`Window.TranslateCommand`), one a press.
    let crossed = close.iter().find(|(interaction, _)| **interaction == Interaction::Pressed).map(|(_, close)| close.0);
    let escaped = (keys.just_pressed(KeyCode::Escape) && !carried.windows.is_empty()).then(|| carried.windows.len() - 1);
    if let Some(index) = crossed.or(escaped) {
        if index < carried.windows.len() {
            carried.windows.remove(index);
            carried.tidy();
        }
        clicks.moving = None;
        return;
    }
    let Some(cursor) = window.single().ok().and_then(Window::cursor_position) else { return };
    if !buttons.pressed(MouseButton::Left) {
        clicks.moving = None;
    }
    if carried.asking.is_none() && buttons.just_pressed(MouseButton::Left) {
        // The one on top where the mouse is.
        let on = roots.iter().filter(|(_, _, at)| at.mouse_over()).map(|(root, ..)| root.0).max();
        if let Some(index) = on.filter(|index| *index < carried.windows.len()) {
            let by_caption = captions.iter().any(|(caption, at)| caption.0 == index && at.mouse_over());
            let top = carried.windows.len() - 1;
            let picked = carried.windows[index];
            if index != top || (picked.grids && carried.priority_auto && carried.priority != Some(picked.item)) {
                carried.open_window(picked.item, picked.grids, picked.at);
            }
            if by_caption {
                clicks.moving = Some((top, cursor - picked.at));
            }
        }
    }
    // Moved: where it is drawn goes with the mouse at once, and where it
    // is kept is changed without the page being drawn afresh for it.
    if let Some((index, hold)) = clicks.moving {
        let at = cursor - hold;
        for (root, mut node, _) in &mut roots {
            if root.0 == index {
                (node.left, node.top) = (Val::Px(at.x), Val::Px(at.y));
            }
        }
        if let Some(window) = carried.bypass_change_detection().windows.get_mut(index) {
            window.at = at;
        }
    }
}

/// A weapon a quick-use key asks for, for `settle` to take in hand.
#[derive(Resource, Default)]
pub struct AskedFor(Option<Id>);

/// The quick-use keys, 4 to 0 (`ECommand.SelectFastSlot4` and on; the
/// install has them on the key coming up). With the screen open, the key
/// binds the thing under the mouse to itself, if it is the character's
/// and lies where it may be bound (`ItemUiContext.BindItem`). In play it
/// takes what it is bound to in hand (`Player.SetQuickSlotItem`): a
/// weapon is drawn; medicine, food and drink are used, which is not built.
fn quick_keys(
    bindings: Res<Bindings>,
    options: Res<Options>,
    controller: Res<Controller>,
    mut carried: ResMut<Carried>,
    mut asked: ResMut<AskedFor>,
    items: Query<(&ItemView, &RelativeCursorPosition, &ComputedNode)>,
    covers: Query<(&Cover, &RelativeCursorPosition)>,
) {
    let Some(key) = (0..QUICK_KEYS.len()).find(|key| bindings.fired(QUICK_KEYS[*key].0)) else { return };
    let word = match bindings.caption(QUICK_KEYS[key].0) {
        "" => QUICK_KEYS[key].1,
        word => word,
    };
    let carried = &mut *carried;
    if controller.inventory_open {
        if carried.asking.is_some() {
            return;
        }
        let Some((id, _)) = item_under(items.iter(), cover_under(covers.iter()), None) else { return };
        // Only what the character has is bound.
        let Some(item) = carried.equipment.item.find(id) else { return };
        let name = carried.name(item);
        carried.note = match carried.bound.bind(&carried.equipment.item, id, key) {
            true => format!("{name}: bound to {word}"),
            false => format!("{name}: not bound (a key takes a weapon, a grenade, a knife, medicine, food or drink that lies in the vest or the pockets)"),
        };
        return;
    }
    if !options.keys_live() {
        return;
    }
    let Some(item) = carried.bound.items[key].and_then(|id| carried.equipment.item.find(id)) else { return };
    if is(item, "Weapon") {
        asked.0 = Some(item.id);
    } else {
        // Nothing here applies a thing to the body yet.
        let name = carried.name(item);
        info!("the key {word}: {name} would be used; there is no health system yet");
        carried.note = format!("{name}: the key {word} uses it in the game; there is no health system here yet");
    }
}

/// What the hands have been through, for `settle`.
#[derive(Default)]
struct Hands {
    settled: bool,
    /// The weapon in hand, or on its way out of the hands.
    item: Option<Id>,
    /// The slot the hands last drew from (the game's `ActiveSlot`); it
    /// stays so when that slot is emptied.
    active: Option<&'static str>,
    /// Whether the inventory was open when the thing in hand was last
    /// told of it.
    told_open: bool,
    /// The weapon has been told to go and is not out of the hands yet.
    going: bool,
    /// What is to be taken in hand when it has gone: a weapon and the
    /// slot it hangs in, or nothing (the game's scheduled process).
    next: Option<(Id, &'static str)>,
}

/// The rig that has just been taken in hand is drawn from its animator's
/// first pose on, and the simulation's next frame reads that of it.
fn first_pose(weapon: &mut HandsRig, controller: &mut Controller) {
    if let Some(pose) = weapon.animator().map(|animator| animator.pose_now()) {
        let read = weapon.frame.read(&pose);
        controller.rig_changed(pose, read);
    }
}

/// The keys of the three weapon slots by the game's names for them
/// (`PlayerInputTranslator`: the sling's, the back's, the holster's).
const SLOT_KEYS: [(&str, &str); 3] = [("PrimaryWeaponFirst", "FirstPrimaryWeapon"), ("PrimaryWeaponSecond", "SecondPrimaryWeapon"), ("SecondaryWeapon", "Holster")];

/// What the equipment makes of the character once something has moved.
///
/// The weapon in hand is one of those in the slots a weapon is held from
/// (the sling, the back, the holster), if it is of the kind the app has
/// built (inventory spec, "The weapon in hand"):
///
/// - A character made with one has the first of them in hand from the
///   start (`Player.SetFirstAvailableItem`).
/// - A slot's key takes what hangs there in hand (`Player.SetSlotItem`):
///   what is in hand is put away first, and the key of the thing already
///   in hand does nothing. The keys are the install's own (`keys.rs`: the
///   sling's is `2`, the back's `3`, the holster's `1`). So does a
///   quick-use key bound to a weapon.
/// - A weapon put into the slot the hands last drew from is taken in hand
///   at once; into any other it hangs until its key is pressed.
/// - The weapon in hand moved out of its slot, to another slot or away,
///   is put away, and the hands take the next weapon in the slots' order
///   or are empty (`SetControllerInsteadRemovedOne`).
///
/// The app builds one rifle, with one logic for it: a second rifle of the
/// same kind would be drawn with the first one's rounds. PROVISIONAL.
///
/// Being drawn and being put away are the weapon's own operations
/// (`Firearm::draw`, `hide`), which its animator plays out by its own
/// states from whatever it was doing: from rest, from the hold of an open
/// inventory, after a shot or a reload in hand. The weapon says when it
/// is out of the hands, and only then is the next thing made.
///
/// Empty hands are a rig of their own (inventory spec, "Empty hands"),
/// with a controller and an animator of their own, which stands by while
/// the weapon is in hand and is changed over with it (`Stowed`): they
/// appear when the weapon has gone, and go at once when the weapon is
/// drawn, as the game's do. If the install's empty hands could not be
/// read, the weapon and the arms are hidden instead.
///
/// The inventory opening and shutting is told to whatever is in the
/// hands at that moment, and what comes of it is the business of the
/// operation it then has in hand (`Firearm::set_inventory`): a weapon at
/// rest goes down and comes back by its animator's own states; one being
/// drawn does not hear of it. What has just appeared in the hands is told
/// of the screen afresh, as the game's owner tells it when the hands have
/// changed (`GamePlayerOwner.SetHandsController`): so a weapon drawn
/// under an open screen goes down as soon as it is up.
///
/// The carried weight is the equipment's sum, which the controller goes
/// by from the next frame (inventory spec, "Weight").
fn settle(
    mut carried: ResMut<Carried>,
    options: Res<Options>,
    game: Res<crate::GameData>,
    mut preparing: Option<ResMut<crate::weapon::Preparing>>,
    bindings: Res<Bindings>,
    mut asked: ResMut<AskedFor>,
    rig: Option<ResMut<WeaponRig>>,
    weapon: Option<ResMut<HandsRig>>,
    mut stowed: Option<ResMut<Stowed>>,
    mut controller: ResMut<Controller>,
    mut shown: Query<&mut Visibility>,
    mut hands: Local<Hands>,
) {
    // A weapon hung where the hands can take it with no rig yet has its
    // rig made ready before any other (`weapon::Preparing`); the hands
    // take it once it is in the world.
    let hung: Vec<String> = HAND_SLOTS.into_iter().filter_map(|slot| carried.equipment.worn(slot)).map(|weapon| weapon.kind.template.clone()).collect();
    let awaited: Vec<&String> = hung.iter().filter(|template| stowed.as_deref().is_some_and(|stowed| stowed.awaited(template))).collect();
    if let Some(preparing) = preparing.as_deref_mut() {
        for template in &awaited {
            preparing.ask(template);
        }
    }
    let game = &*game;
    if carried.is_changed() || !hands.settled {
        let weight = carried.equipment.carried_weight(false);
        if !options.weight_given && (weight - controller.movement.settings.weight).abs() > 1e-4 {
            match controller.movement.set_weight(&game.hard, &game.db.config, weight) {
                Ok(()) => info!("carried: {weight:.2} kg"),
                Err(e) => warn!("the carried weight: {e:#}"),
            }
        }
    }
    // The character is made with the first weapon it has in hand: if that
    // one's rig is still to come, the hands wait for it.
    if !hands.settled && !awaited.is_empty() {
        return;
    }
    let first = !std::mem::replace(&mut hands.settled, true);
    let quick = asked.0.take();
    // What the weapon's tube took leaves the pockets and the vest, and
    // what is left there is what the next filling has to go by.
    let taken = std::mem::take(&mut controller.rounds_taken);
    if let (true, Some(weapon)) = (taken > 0, hands.item) {
        trp_sim::inventory::take_rounds(&mut carried.equipment.item, weapon, taken);
    }
    controller.rounds_in_reach = hands.item.map_or(0, |weapon| trp_sim::inventory::rounds_in_reach(&carried.equipment.item, weapon));
    // A chambered round the weapon threw out (a magazine filled with the
    // bolt open) lies on the ground before the character, as the game
    // throws it there: a round of the kind the weapon names as its own.
    let ejected = std::mem::take(&mut controller.rounds_ejected);
    let round = hands
        .item
        .and_then(|weapon| carried.equipment.item.find(weapon))
        .and_then(|weapon| game.db.items.get(&weapon.kind.template)?.props.get("defAmmo")?.as_str().map(str::to_owned));
    if let (true, Some(round)) = (ejected > 0, round) {
        if let Ok(kind) = trp_sim::inventory::Kind::of(&game.db, &round).map(std::sync::Arc::new) {
            let m = &controller.movement;
            let (sin, cos) = m.yaw.to_radians().sin_cos();
            // Half a pace ahead and a little to the right, in the engine's axes.
            let at = Vec3::new(m.position[0] + sin * DROP_AHEAD + cos * 0.2, m.position[1], -(m.position[2] + cos * DROP_AHEAD - sin * 0.2));
            for _ in 0..ejected {
                carried.ground.push((Item::new(kind.clone()), at));
            }
            carried.note = "the chambered round was thrown out as the bolt was opened: it lies on the ground".to_owned();
        }
    }
    let carried = &*carried;
    let (Some(mut rig), Some(mut weapon)) = (rig, weapon) else { return };
    let (rig, weapon, controller) = (&mut *rig, &mut *weapon, &mut *controller);
    let open = controller.inventory_open;
    // What hangs in each slot a weapon is held from, of the kinds built.
    let built: Vec<String> = stowed.as_deref().map_or_else(|| vec![options.weapon.clone()], |stowed| stowed.built.clone());
    let hangs = |slot: &str| carried.equipment.worn(slot).filter(|weapon| built.contains(&weapon.kind.template)).map(|weapon| weapon.id);
    let kind_of = |id: Id| carried.equipment.item.find(id).map(|item| item.kind.template.clone());
    let slot_of = |id: Id| HAND_SLOTS.into_iter().find(|slot| hangs(slot) == Some(id));
    let first_but = |but: Option<Id>| HAND_SLOTS.into_iter().find_map(|slot| hangs(slot).filter(|id| Some(*id) != but).map(|id| (id, slot)));
    // A slot's key, or a quick-use key bound to a weapon.
    let live = !open && options.keys_live();
    let key = SLOT_KEYS.iter().find(|(key, _)| live && bindings.fired(key)).and_then(|(_, slot)| hangs(slot).map(|id| (id, *slot)));
    let asked_for = key.or_else(|| quick.filter(|_| live).and_then(|id| slot_of(id).map(|slot| (id, slot))));
    let said = controller.take_hands_events();
    let (gone, appeared) = (said.contains(&FirearmEvent::Hidden), said.contains(&FirearmEvent::Appeared));

    let mut show = |entity: Option<Entity>, on: bool| {
        if let Some(mut shown) = entity.and_then(|entity| shown.get_mut(entity).ok()) {
            *shown = if on { Visibility::Inherited } else { Visibility::Hidden };
        }
    };
    // What is in the hands is changed for another rig that stands by (a
    // weapon's, by its template, or the empty hands'), and the body's
    // animator is told what is now there. Whether there was such a rig.
    let change = |stowed: &mut Stowed, to: Option<&str>, rig: &mut WeaponRig, weapon: &mut HandsRig, controller: &mut Controller| {
        if !stowed.change_to(to, rig, weapon) {
            return false;
        }
        let folded = weapon.firearm.as_ref().is_some_and(|firearm| firearm.stock_folded());
        controller.movement.body.set_weapon_width(if folded { weapon.folded_width.unwrap_or(rig.width) } else { rig.width });
        controller.movement.body.set_hands_kind(rig.kind);
        // Aiming costs the arms by the weapon's own ergonomic weight.
        if to.is_some() {
            controller.movement.stamina.aim_mass = weapon.aim_mass;
        }
        true
    };
    // The weapon is out of sight and the empty hands appear.
    let empty_handed = |show: &mut dyn FnMut(Option<Entity>, bool), rig: &mut WeaponRig, weapon: &mut HandsRig, controller: &mut Controller, stowed: Option<&mut Stowed>| match stowed {
        Some(stowed) if stowed.held().is_some() => {
            show(rig.weapon_root, false);
            if change(stowed, None, rig, weapon, controller) {
                if let Some(empty) = weapon.empty.as_mut() {
                    empty.draw();
                }
                first_pose(weapon, controller);
            } else {
                // No rig for empty hands could be read: nothing is drawn.
                show(Some(rig.root), false);
            }
        }
        Some(_) => {}
        None => show(Some(rig.root), false),
    };
    // A weapon is taken in hand: its own rig is changed for whatever was
    // there. The empty hands go at once, with no state of their own
    // waited for.
    let take = |show: &mut dyn FnMut(Option<Entity>, bool), rig: &mut WeaponRig, weapon: &mut HandsRig, controller: &mut Controller, stowed: Option<&mut Stowed>, kind: Option<String>| {
        if let Some(stowed) = stowed.filter(|stowed| stowed.held() != kind.as_deref()) {
            if let Some(empty) = weapon.empty.as_mut() {
                empty.hide();
            }
            // Another weapon's own tree goes out of sight.
            if stowed.held().is_some() {
                show(rig.weapon_root, false);
            }
            change(stowed, kind.as_deref(), rig, weapon, controller);
        }
        show(rig.weapon_root, true);
        show(Some(rig.root), true);
        controller.armed = true;
        if let Some(firearm) = weapon.firearm.as_mut() {
            firearm.draw();
        }
        first_pose(weapon, controller);
    };
    if first {
        // As the character is made: with the first weapon it has in hand
        // and at rest, or with none.
        match first_but(None) {
            Some((id, slot)) => {
                (hands.item, hands.active) = (Some(id), Some(slot));
                controller.armed = true;
                // The rig of the weapon it has, if that is not the one
                // the run was begun with.
                if let (Some(stowed), Some(kind)) = (stowed.as_deref_mut(), kind_of(id)) {
                    if stowed.held() != Some(kind.as_str()) {
                        show(rig.weapon_root, false);
                        change(stowed, Some(&kind), rig, weapon, controller);
                        show(rig.weapon_root, true);
                    }
                }
                if let Some(firearm) = weapon.firearm.as_mut() {
                    // One it began with in hand was drawn before: a
                    // round is chambered.
                    firearm.carried();
                    firearm.start_drawn();
                }
                first_pose(weapon, controller);
            }
            None => {
                // A weapon the character does not begin with lies in the
                // crate as one found: with no round in its chamber (the
                // user, 2026-10-12; no stock preset in the install's
                // database has one). The draw that first takes it in hand
                // works the action (firing spec, "Being drawn").
                if let Some(firearm) = weapon.firearm.as_mut() {
                    firearm.found();
                }
                empty_handed(&mut show, rig, weapon, controller, stowed.as_deref_mut())
            }
        }
    } else if controller.armed {
        // The weapon in hand has left the slot it was drawn from: it is
        // put away, and the next in the slots' order is taken. Or a key
        // asks for another: it is put away for that one.
        let held = hands.item;
        let left = held.is_none_or(|id| slot_of(id) != hands.active);
        if !hands.going {
            let other = asked_for.filter(|(id, _)| Some(*id) != held);
            if left || other.is_some() {
                hands.going = true;
                hands.next = if left { first_but(held) } else { other };
                if let Some(firearm) = weapon.firearm.as_mut() {
                    firearm.hide(false);
                }
            }
        } else if let Some(other) = asked_for.filter(|(id, _)| Some(*id) != held || left) {
            // A key pressed while it is going takes the place of what
            // was to come next.
            hands.next = Some(other);
        }
        if gone || (hands.going && weapon.firearm.as_ref().is_none_or(|firearm| !firearm.in_hand())) {
            (hands.going, hands.item) = (false, None);
            controller.armed = false;
            // What was to come next, if it still hangs where it did.
            match hands.next.take().filter(|(id, slot)| hangs(slot) == Some(*id)) {
                Some((id, slot)) => {
                    (hands.item, hands.active) = (Some(id), Some(slot));
                    take(&mut show, rig, weapon, controller, stowed.as_deref_mut(), kind_of(id));
                }
                None => empty_handed(&mut show, rig, weapon, controller, stowed.as_deref_mut()),
            }
        }
    } else {
        // Empty hands: a key's weapon, or one put into the slot the
        // hands last drew from, is taken at once.
        let back = hands.active.and_then(|slot| hangs(slot).map(|id| (id, slot)));
        if let Some((id, slot)) = asked_for.or(back) {
            (hands.item, hands.active) = (Some(id), Some(slot));
            take(&mut show, rig, weapon, controller, stowed.as_deref_mut(), kind_of(id));
        }
    }
    // The screen opening and shutting, told to what is in the hands now;
    // and the screen as it stands, told to what has just appeared there.
    if open != hands.told_open || appeared {
        hands.told_open = open;
        if let Some(firearm) = weapon.firearm.as_mut() {
            firearm.set_inventory(open);
        }
        if let Some(empty) = weapon.empty.as_mut() {
            empty.set_inventory(open);
        }
    }
}


/// What is looked at from within the game's reach for what can be opened
/// or picked up (`Player.InteractionRaycast`): a thing on the ground, or a
/// door, container or trunk of the range (a door within
/// `DOOR_RAYCAST_DISTANCE`, the rest within `LOOT_RAYCAST_DISTANCE`), and
/// the prompt for what the game offers to do to it. Nothing is looked at
/// unless the movement state lets the player (`CanInteract` of its
/// animator state) and the thing in hand does (a rifle only while its
/// logic rests or fires).
fn reach(
    mut commands: Commands,
    view: Res<View>,
    range: Res<Range>,
    game: Res<GameData>,
    mut controller: ResMut<Controller>,
    carried: Res<Carried>,
    bindings: Res<Bindings>,
    texts: Res<MenuTexts>,
    hands: Option<Res<HandsRig>>,
    camera: Query<&GlobalTransform, With<crate::weapon::Eye>>,
    mut reach: ResMut<Reach>,
    mut prompt: Query<(Entity, &mut Visibility), With<Prompt>>,
    mut shown: Local<Option<(Menu, String)>>,
) {
    // The game's line starts a little behind the point the player looks
    // from (`BEHIND_CAST`) and ends the reach ahead of it.
    let float = |name: &str, or: f32| game.hard.float(name).unwrap_or(or);
    let (within, behind, door) = (float("LOOT_RAYCAST_DISTANCE", 1.3), float("BEHIND_CAST", 0.15), float("DOOR_RAYCAST_DISTANCE", 1.0));
    // Nothing is offered while a thing is being taken up, a door kicked,
    // or a container searched is still to be shown.
    let busy = controller.movement.acting() || controller.movement.searching() || controller.movement.kicking();
    let hands_let = hands.as_deref().and_then(|hands| hands.firearm.as_ref()).is_none_or(|firearm| firearm.can_interact());
    let allowed = controller.movement.can_interact() && hands_let;
    let playing = view.played() && !controller.inventory_open && !busy && allowed;
    let mut seen = Reach::default();
    if let (true, Ok(eye)) = (playing, camera.single()) {
        let (from, to) = (eye.translation() - eye.forward() * behind, eye.translation() + eye.forward() * within);
        seen.thing = range.looked_at(from, to, door + behind);
        // A thing on the ground is looked at when the line passes within
        // a hand's breadth of it; the nearest along the line is meant.
        let along = to - from;
        let near = |at: Vec3| {
            let share = ((at - from).dot(along) / along.length_squared()).clamp(0.0, 1.0);
            ((from + along * share).distance(at) < LOOSE_REACH).then_some(share)
        };
        let looked_at = carried.ground.iter().filter_map(|(item, at)| near(*at + Vec3::Y * 0.06).map(|share| (share, item)));
        // The game offers to take a thing only if a pick-up would find it
        // a place (`GetAvailableInteractionState`).
        let nearest = looked_at.min_by(|a, b| a.0.total_cmp(&b.0)).map(|(_, item)| item);
        let already = |id: Id| reach.loose == Some(id) && reach.thing == seen.thing;
        seen.loose = nearest.filter(|item| already(item.id) || can_pick_up(&carried.equipment.item, item, &carried.rules)).map(|item| item.id);
    }
    // The game's menu for it (`InteractionContextHelper.GetAvailableActions`):
    // a thing on the ground is taken; a door, a container or a trunk
    // offers what its kind and its state have, a door's kick only from
    // where the door lets it be made and not lying down, a lock's key
    // only to a character that carries it.
    let feet = controller.movement.position;
    let lying = matches!(controller.movement.state, State::ProneIdle | State::ProneMove | State::FromProne);
    let threshold = float("DOOR_BREACH_THRESHOLD", 0.8);
    let (mut menu, name) = match (seen.loose.and_then(|id| carried.ground.iter().find(|(item, _)| item.id == id)), seen.thing) {
        (Some((item, _)), _) => (Some(Menu::of(vec![(Action::Take, false)])), carried.name(item)),
        (None, Some(index)) => {
            let placed = &range.things[index];
            let has_key = placed.thing.key.as_deref().is_some_and(|key| carries(&carried.equipment.item, key));
            (placed.thing.menu(feet, has_key, lying, threshold), placed.name.to_owned())
        }
        (None, None) => (None, String::new()),
    };
    // The lines as they stood keep the choice the wheel made; when they
    // change (another thing, another state of it, the kick's angle
    // crossed) the menu is made afresh, with its first line chosen.
    let same = reach.thing == seen.thing && reach.loose == seen.loose;
    if let (Some(menu), Some(before)) = (menu.as_mut(), reach.menu.as_ref().filter(|_| same)) {
        if menu.lines == before.lines {
            menu.selected = before.selected;
        }
    }
    // The wheel: a notch up is a line up, a notch down a line down.
    let notches = std::mem::take(&mut controller.menu_notches);
    if let Some(menu) = menu.as_mut() {
        for _ in 0..notches.unsigned_abs() {
            menu.scroll(notches < 0);
        }
    }
    if controller.menu_shown != menu.is_some() {
        controller.menu_shown = menu.is_some();
    }
    seen.menu = menu;
    if *reach != seen {
        *reach = seen;
    }
    // Drawn as the game's panel has it: every line, the chosen one light
    // with dark words, one that cannot be chosen dim.
    let now = reach.menu.clone().map(|menu| (menu, name));
    if *shown == now {
        return;
    }
    let key = bindings.caption("Interact");
    for (panel, mut visible) in &mut prompt {
        let wanted = if now.is_some() { Visibility::Inherited } else { Visibility::Hidden };
        if *visible != wanted {
            *visible = wanted;
        }
        commands.entity(panel).despawn_related::<Children>();
        let Some((menu, name)) = &now else { continue };
        commands.entity(panel).with_children(|lines| {
            lines.spawn((PromptLine, words(format!("{key}   {}", name.to_uppercase()), 12.0, FAINT)));
            for (index, (action, disabled)) in menu.lines.iter().enumerate() {
                let chosen = menu.selected == Some(index);
                let (back, ink) = match (chosen, *disabled) {
                    (true, _) => (MENU_LIGHT, MENU_DARK),
                    (false, true) => (MENU_DARK, MENU_DIM),
                    (false, false) => (MENU_DARK, MENU_LIGHT),
                };
                lines.spawn((PromptLine, Node { padding: UiRect::axes(Val::Px(10.0), Val::Px(3.0)), ..default() }, BackgroundColor(back))).with_children(|line| {
                    line.spawn(words(texts.of(*action), 15.0, ink));
                });
            }
        });
    }
    *shown = now;
}

/// The menu's three colours as the game's buttons have them
/// (`InteractionButton`): the chosen line's ground and the other lines'
/// words, the words of a line that cannot be chosen, and the dark of the
/// rest.
const MENU_LIGHT: Color = Color::srgb(0.514, 0.514, 0.514);
const MENU_DIM: Color = Color::srgb(0.25, 0.25, 0.25);
const MENU_DARK: Color = Color::srgba(0.0, 0.0, 0.0, 0.85);

/// Draws each thing that lies on the ground as a plain box of about its
/// size in cells, and takes away those that were picked up. The boxes
/// are the app's; the game's models for loose items are not read.
fn show_ground(
    mut commands: Commands,
    carried: Res<Carried>,
    shown: Query<(Entity, &LooseItem)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if !carried.is_changed() {
        return;
    }
    for (entity, loose) in &shown {
        if !carried.ground.iter().any(|(item, _)| item.id == loose.0) {
            commands.entity(entity).despawn();
        }
    }
    for (item, at) in carried.ground.iter().filter(|(item, _)| !shown.iter().any(|(_, loose)| loose.0 == item.id)) {
        let [width, height] = item.cells();
        let size = Vec3::new(0.09 * width.min(5) as f32, 0.12, 0.09 * height.min(5) as f32);
        let colour = materials.add(StandardMaterial { base_color: Color::srgb(0.42, 0.44, 0.33), perceptual_roughness: 0.9, ..default() });
        commands.spawn((LooseItem(item.id), Mesh3d(meshes.add(Cuboid::from_size(size))), MeshMaterial3d(colour), Transform::from_translation(*at + Vec3::Y * 0.06)));
    }
}

/// What the character has reached for and the screen waits on: a
/// container searched, to be shown when the wait after its search is
/// over; the container whose screen is open; or a thing on the ground, to
/// be taken when the hand has it.
#[derive(Default)]
struct Reaching {
    search: Option<usize>,
    shown: Option<usize>,
    take: Option<Id>,
    /// The door being kicked, and the one being unlocked.
    kicked: Option<usize>,
    unlocked: Option<usize>,
    /// The backpack being dropped, until it is off the shoulders.
    pack: Option<Id>,
}

/// `Tab` opens and shuts the screen, as the game's inventory key does.
/// `F` takes up the thing looked at on the ground, or does to the door,
/// container or trunk looked at what the game offers (`docs/spec/interactions.md`):
///
/// - A thing on the ground is taken only standing still, as in the game:
///   the left hand reaches for it, and 0.6 s on it goes into the first
///   place that takes it (`Movement::pick_up`).
/// - A door is opened or shut, and so is a trunk or an open container
///   (`Thing::work`, `Movement::work`): the left hand makes the gesture the
///   thing asks of a character standing here, the character is held to
///   nothing, and the thing begins to move half a second or so later.
/// - A shut container is searched (`Movement::search`): its gesture, and
///   0.55 s after the key the screen opens with it beside the equipment,
///   which cuts the gesture short; unless the character has by then gone a
///   quarter of a metre further from it, or is in a jump, when it is told
///   to shut again. When its screen shuts it is told to shut.
///
/// Open, the mouse is the screen's: the view does not turn and the
/// weapon's keys do nothing (`Controller::inventory_open`). The inventory
/// key does nothing while a thing is being taken up.
fn toggle(
    bindings: Res<Bindings>,
    options: Res<Options>,
    reach: Res<Reach>,
    mut range: ResMut<Range>,
    mut carried: ResMut<Carried>,
    mut controller: ResMut<Controller>,
    mut screen: Query<&mut Visibility, With<Screen>>,
    mut hands: Option<ResMut<HandsRig>>,
    camera: Query<&GlobalTransform, With<crate::weapon::Eye>>,
    keys: Res<ButtonInput<KeyCode>>,
    options_tab: Option<Res<crate::options_view::OptionsState>>,
    mut selected: ResMut<Selected>,
    mut started: Local<bool>,
    mut reaching: Local<Reaching>,
) {
    let first = !std::mem::replace(&mut *started, true) && options.inventory;
    // Not while a thing is taken up or unlocked, nor in the approach to a
    // door (`StateLocksInventory`).
    let working = controller.movement.acting() || controller.movement.locks_inventory();
    // The inventory's key and the key that works what is looked at, as
    // the install binds them (`Tab` and `F` in the one read).
    let tab = options.keys_live() && !working && bindings.fired("Inventory");
    // Escape shuts the screen too (`InventoryScreen.TranslateCommand`),
    // and one thing a press, as the game's tree of screens hands it down:
    // a window over the screen has it first (`windows`), the window that
    // asks how many next (`ask`), and the screen only with neither up.
    // The options tab has it while a key is being chosen or a way of
    // being pressed asked for: there Escape keeps what stood.
    let taken = !carried.windows.is_empty() || carried.asking.is_some() || options_tab.as_deref().is_some_and(|tab| tab.takes_escape());
    let escape = options.keys_live() && controller.inventory_open && keys.just_pressed(KeyCode::Escape) && !taken;
    let tab = tab || escape;
    let key = options.keys_live() && !controller.inventory_open && bindings.fired("Interact");
    // What the hands' work on the world has come to.
    let mut search = None;
    for acted in controller.movement.take_acted() {
        match acted {
            Acted::PickedUp => {
                let carried = &mut *carried;
                let lies = reaching.take.take().and_then(|id| carried.ground.iter().position(|(item, _)| item.id == id));
                if let Some(at) = lies {
                    let (item, lay) = carried.ground.remove(at);
                    let name = carried.name(&item);
                    let (done, left) = pick_up(&mut carried.equipment.item, item, &carried.rules);
                    carried.note = match done {
                        Ok((_, Applied::Topped(count))) => format!("{name}: {count} picked up"),
                        Ok(_) => format!("{name}: picked up"),
                        Err(refusal) => format!("{name}: {}", why(&refusal)),
                    };
                    // What did not go stays where it lay: the thing
                    // itself, or the rest of a stack.
                    if let Some(item) = left {
                        carried.ground.insert(at, (item, lay));
                    }
                }
            }
            // The container's wait is over and the character is still at
            // it: its screen opens now (its own opening began on the same
            // frame, `Thing`).
            Acted::Searched => search = reaching.search.take(),
            Acted::LeftUnsearched => {
                if let Some(index) = reaching.search.take() {
                    let range = &mut *range;
                    if let Some(settings) = range.thing_settings.as_ref() {
                        range.things[index].thing.tell(false, settings);
                    }
                    carried.note = format!("the {}: left before it was open", range.things[index].name);
                }
            }
            // The key has been in the lock long enough: the door is
            // unlocked, and stands shut.
            Acted::Worked => {
                if let Some(index) = reaching.unlocked.take() {
                    range.things[index].thing.unlock();
                    carried.note = format!("the {}: unlocked", range.things[index].name);
                }
            }
            // The kick lands: the door is thrown open, or holds.
            Acted::Kicked { opens } => {
                if let Some(index) = reaching.kicked {
                    let thing = &mut range.things[index].thing;
                    if opens { thing.kick_open() } else { thing.fail_breach() }
                }
            }
            Acted::Done => reaching.kicked = None,
            // The pack is off the shoulders (`OnBackpackDropEvent`): what
            // is in hand idles again, and the pack lies on the ground a
            // step ahead, as a thing put down does (the game throws it).
            Acted::BackpackDropped => {
                if let Some(firearm) = hands.as_deref_mut().and_then(|rig| rig.firearm.as_mut()) {
                    firearm.backpack_dropped();
                }
                if let (Some(id), Ok(eye)) = (reaching.pack.take(), camera.single()) {
                    let ahead = eye.forward().as_vec3().with_y(0.0).normalize_or_zero();
                    let at = Vec3::new(eye.translation().x, controller.movement.position[1], eye.translation().z) + ahead * DROP_AHEAD;
                    let carried = &mut *carried;
                    let name = carried.find(id).map(|item| carried.name(item)).unwrap_or_default();
                    match take_out(&mut [&mut carried.equipment.item, &mut carried.loot], id, &carried.rules) {
                        Ok(item) => {
                            carried.ground.push((item, at));
                            carried.note = format!("{name}: dropped");
                        }
                        Err(refusal) => carried.note = format!("{name}: {}", why(&refusal)),
                    }
                    carried.tidy();
                }
            }
        }
    }
    // The backpack's own key (`Player.DropBackpack`), heard while the
    // character plays; under the screen the key that puts a thing down
    // asks for the same. With a pack on the back that may leave it, the
    // movement letting it (no sprint, no kick, no gesture playing) and
    // what is in hand at rest, the hands' operation for it begins and the
    // body makes the gesture.
    if options.keys_live() && !controller.inventory_open && bindings.fired("DropBackpack") {
        carried.pack_asked = true;
    }
    // (Looked at before it is written: a resource written every frame is
    // one that has changed every frame, and the screen is drawn afresh
    // whenever this one has.)
    let pack_asked = carried.pack_asked;
    if pack_asked {
        carried.pack_asked = false;
    }
    if pack_asked && reaching.pack.is_none() {
        let pack = carried.equipment.worn("Backpack").map(|pack| pack.id);
        let may_leave = pack.is_some_and(|id| {
            let (mut worn, mut loot) = (carried.equipment.item.clone(), carried.loot.clone());
            take_out(&mut [&mut worn, &mut loot], id, &carried.rules).is_ok()
        });
        if let (Some(id), true, true) = (pack, may_leave, controller.movement.can_drop_backpack()) {
            let hands_take = match hands.as_deref_mut() {
                Some(HandsRig { firearm: Some(firearm), .. }) => firearm.drop_backpack(),
                Some(HandsRig { empty: Some(empty), .. }) => empty.idling(),
                _ => true,
            };
            if hands_take {
                if controller.movement.drop_backpack() {
                    reaching.pack = Some(id);
                } else if let Some(firearm) = hands.as_deref_mut().and_then(|rig| rig.firearm.as_mut()) {
                    firearm.backpack_dropped();
                }
            }
        }
    }
    // The interact key does the menu's chosen line; with nothing to
    // choose it does nothing.
    let chosen = reach.menu.as_ref().and_then(Menu::chosen).filter(|_| key);
    if let (Some(Action::Take), Some(id)) = (chosen, reach.loose) {
        if controller.movement.pick_up() {
            reaching.take = Some(id);
        }
        return;
    }
    if let (Some(Action::Breach), Some(index)) = (chosen, reach.thing) {
        // A kick (`MovementContext.StartDoorBreak`): the door says where
        // it is made from, whether it puts the character there first, and
        // whether it will open; it is held from now, and the body's
        // animator takes the character through its approach and the kick.
        if !controller.movement.kicking() && !controller.movement.acting() {
            let feet = controller.movement.position;
            if let Some(breach) = range.things[index].thing.start_breach(feet) {
                controller.movement.kick(breach.opens, breach.snaps.then_some((breach.position, breach.view_target)));
                reaching.kicked = Some(index);
            }
        }
        return;
    }
    if let (Some(Action::Unlock), Some(index)) = (chosen, reach.thing) {
        // The key in the lock (`Player.StartInteraction`): the game's
        // other way to a thing, which holds the character; the door is
        // held for it and unlocked when the hand has been at it.
        let lock = WorldInteraction { interaction: Gesture::DoorKeyOpen, snap: false, height: controller.movement.position[1] + 1.0 };
        if controller.movement.unlock(lock) {
            range.things[index].thing.hold();
            reaching.unlocked = Some(index);
        }
        return;
    }
    if let (Some(action @ (Action::Open | Action::Close | Action::Search)), Some(index), None) = (chosen, reach.thing, reaching.search) {
        let range = &mut *range;
        let Some(settings) = range.thing_settings.as_ref() else { return };
        let feet = controller.movement.position;
        let placed = &mut range.things[index];
        // The gesture may be refused (in a sprint, with the weapon busy):
        // the thing is worked all the same.
        let gesture = placed.thing.work(action != Action::Close, feet, settings);
        if action == Action::Search {
            controller.movement.search(gesture, placed.place);
            reaching.search = Some(index);
        } else {
            controller.movement.work(gesture);
        }
        return;
    }
    if !(first || tab || search.is_some()) {
        return;
    }
    controller.inventory_open = first || search.is_some() || !controller.inventory_open;
    let open = controller.inventory_open;
    // The screen opens on its gear tab every time: the game makes the
    // screen afresh at each opening, with its tabs beginning at the
    // gear's (`InventoryScreen.Show`, `TabGroup`), and a container asks
    // for that tab outright (`ShowInventoryScreenLoot`). The tab last
    // looked at is not kept from one opening to the next.
    if open && selected.0 != GEAR {
        selected.0 = GEAR;
    }
    // The container searched is the one the screen shows beside the
    // equipment; at the start, with `--loot`, the airdrop crate.
    let shows = search.or((first && options.loot).then_some(0)).filter(|_| open);
    if let Some(index) = shows {
        if let Some(holds) = range.things[index].holds {
            carried.bring(holds);
        }
    }
    // A container whose screen shuts is told to shut, with no hand.
    if !open {
        if let Some(index) = reaching.shown.take() {
            let range = &mut *range;
            if let Some(settings) = range.thing_settings.as_ref() {
                range.things[index].thing.tell(false, settings);
            }
        }
    }
    reaching.shown = shows.filter(|&index| range.things[index].holds.is_some()).or(reaching.shown.filter(|_| open));
    let with_crate = shows.is_some();
    if carried.loot_open != with_crate {
        carried.loot_open = with_crate;
    }
    // Whether the screen has the mouse goes by its being open
    // (`cursor::hold`).
    for mut shown in &mut screen {
        *shown = if open { Visibility::Inherited } else { Visibility::Hidden };
    }
}

/// A click on a tab that can be opened shows its page.
fn choose_tab(
    clicked: Query<(&Interaction, &TabButton), Changed<Interaction>>,
    mut selected: ResMut<Selected>,
    mut tabs: Query<(&TabButton, &mut BackgroundColor)>,
    mut pages: Query<(&Page, &mut Node)>,
    mut shown: Local<Option<usize>>,
) {
    if let Some(index) = clicked.iter().find(|(interaction, tab)| **interaction == Interaction::Pressed && TABS[tab.0].1).map(|(_, tab)| tab.0) {
        if selected.0 != index {
            selected.0 = index;
        }
    }
    // The tab chosen is shown: by a click, or by the screen having been
    // opened, which puts it on its gear tab (`toggle`).
    let index = selected.0;
    if *shown == Some(index) {
        return;
    }
    *shown = Some(index);
    for (tab, mut back) in &mut tabs {
        back.0 = if tab.0 == index { LINE } else { PANEL };
    }
    for (page, mut node) in &mut pages {
        node.display = if page.0 == index { Display::Flex } else { Display::None };
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use trp_data::inventory::Holder;
    use trp_sim::inventory::{Kind, EQUIPMENT_TEMPLATE};

    use super::*;

    fn kind(name: &str, classes: &[&str], cells: [u32; 2]) -> Kind {
        let named: Vec<String> = std::iter::once(name).chain(classes.iter().copied()).chain(["Item"]).map(String::from).collect();
        Kind { template: name.into(), name: name.into(), classes: named.clone(), class_names: named, weight: 0.1, cells, ..Kind::default() }
    }

    fn holder(name: &str, cells: [u32; 2], takes: &str) -> Holder {
        Holder { name: name.into(), cells, takes: vec![takes.into()], ..Holder::default() }
    }

    /// A bare character beside an open crate that holds a backpack, a
    /// rifle, sixty rounds and a small case.
    fn carried() -> (Carried, [Id; 4]) {
        let slots = vec![holder("FirstPrimaryWeapon", [0, 0], "Weapon"), holder("Backpack", [0, 0], "Backpack"), holder("TacticalVest", [0, 0], "Vest")];
        let worn = Item::new(Arc::new(Kind { template: EQUIPMENT_TEMPLATE.into(), slots, ..kind("equipment", &[], [1, 1]) }));
        let grid = |cells| vec![holder("main", cells, "Item")];
        let mut loot = Item::new(Arc::new(Kind { grids: grid([8, 8]), ..kind("crate", &[], [1, 1]) }));
        let bag = Item::new(Arc::new(Kind { grids: grid([4, 4]), ..kind("bag", &["Backpack", "SearchableItem", "Equipment"], [3, 3]) }));
        let rifle = Item::new(Arc::new(kind("rifle", &["Weapon"], [4, 1])));
        let mut rounds = Item::new(Arc::new(Kind { stack_max: 60, ..kind("round", &["Ammo"], [1, 1]) }));
        rounds.count = 60;
        let case = Item::new(Arc::new(Kind { grids: grid([2, 2]), ..kind("case", &["SimpleContainer"], [1, 1]) }));
        let ids = [bag.id, rifle.id, rounds.id, case.id];
        for item in [bag, rifle, rounds, case] {
            loot.stow(item).unwrap();
        }
        let carried = Carried {
            equipment: Equipment { item: worn },
            names: HashMap::new(),
            loot,
            loot_open: true,
            away: Vec::new(),
            loot_of: 0,
            rules: Rules { in_raid: true, backpacks_deep: 0 },
            fixed_wear: Vec::new(),
            note: String::new(),
            panel: Default::default(),
            ground: Vec::new(),
            icons: HashMap::new(),
            pictures: HashMap::new(),
            bound: Default::default(),
            windows: Vec::new(),
            priority: None,
            priority_auto: true,
            asking: None,
            pack_asked: false,
        };
        (carried, ids)
    }

    /// The clicks of the game's item view: Alt puts a thing on, Ctrl
    /// sends it to the other side, a double click opens a container in a
    /// window, which is then where Ctrl sends things, and shows what
    /// anything else is.
    #[test]
    fn clicks_put_on_send_over_and_open() {
        let (mut carried, [bag, rifle, rounds, case]) = carried();
        let (ctrl, alt, plain) = ([true, false, false], [false, false, true], [false; 3]);
        let holder = |carried: &Carried, id: Id| carried.lies(id).map(|(holder, _)| holder);
        let worn = carried.equipment.item.id;
        // Ctrl on the rounds with nothing worn to hold them: they stay in
        // the crate (and go to its next free cell).
        click(&mut carried, rounds, false, ctrl, Vec2::ZERO);
        assert_eq!(holder(&carried, rounds), Some(carried.loot.id));
        click(&mut carried, bag, false, alt, Vec2::ZERO);
        click(&mut carried, rifle, false, alt, Vec2::ZERO);
        assert_eq!((holder(&carried, bag), holder(&carried, rifle)), (Some(worn), Some(worn)));
        assert!(carried.note.ends_with("put on"), "{}", carried.note);
        // Now the rounds have somewhere to go, and from there back.
        click(&mut carried, rounds, false, ctrl, Vec2::ZERO);
        assert_eq!(holder(&carried, rounds), Some(bag));
        click(&mut carried, rounds, false, ctrl, Vec2::ZERO);
        assert_eq!(holder(&carried, rounds), Some(carried.loot.id));
        // A double click on the rifle shows what it is; on the case, what
        // is in it, and the case is then where Ctrl sends the rounds.
        click(&mut carried, rifle, true, plain, Vec2::new(100.0, 100.0));
        assert_eq!((carried.windows.len(), carried.windows[0].grids, carried.priority), (1, false, None));
        click(&mut carried, case, true, plain, Vec2::new(200.0, 100.0));
        assert_eq!((carried.windows.len(), carried.windows[1].grids, carried.priority), (2, true, Some(case)));
        click(&mut carried, rounds, false, ctrl, Vec2::ZERO);
        assert_eq!(holder(&carried, rounds), Some(case));
        // And out of it again: a thing in the priority container goes by
        // the plain rule, to the character.
        click(&mut carried, rounds, false, ctrl, Vec2::ZERO);
        assert_eq!(holder(&carried, rounds), Some(bag));
        // A Ctrl double click is no double click: it sends, twice.
        click(&mut carried, rounds, true, ctrl, Vec2::ZERO);
        assert_eq!(holder(&carried, rounds), Some(case));
    }

    /// With Ctrl held a drop is part of a stack: all but one at the most
    /// into a free cell, and a thing that is no stack goes nowhere.
    #[test]
    fn ctrl_lets_go_of_part_of_a_stack() {
        let (mut carried, [_, rifle, rounds, case]) = carried();
        let cell = Target::Cell(case, 0, Place { x: 0, y: 0, turned: false });
        let roots = &mut [&mut carried.equipment.item, &mut carried.loot];
        assert_eq!((most_of(roots, rounds, cell), most_of(roots, rifle, cell)), (59, 0));
        assert_eq!(act(roots, rifle, cell, &carried.rules, Some(1)), Err(Refusal::NoAction));
        assert_eq!(act(roots, rounds, cell, &carried.rules, Some(60)), Err(Refusal::NoAction));
        assert_eq!(act(roots, rounds, cell, &carried.rules, Some(25)), Ok("split"));
        let split = roots[1].find(case).unwrap().grids[0][0].1.id;
        assert_eq!((roots[1].find(rounds).unwrap().count, roots[1].find(split).unwrap().count), (35, 25));
        // Onto the 35 from the 25: all but one may go, and a count is taken.
        assert_eq!(most_of(roots, split, Target::Item(rounds)), 24);
        assert_eq!(act(roots, split, Target::Item(rounds), &carried.rules, Some(10)), Ok("handed over"));
        assert_eq!((roots[1].find(rounds).unwrap().count, roots[1].find(split).unwrap().count), (45, 15));
    }

    /// The slider of the window that asks how many stops where the
    /// game's does: a notch every twenty-fourth of the most, rounded up,
    /// from one, and the most at the end.
    #[test]
    fn the_slider_has_the_game_s_stops() {
        assert_eq!(Asking::stops(1), [1]);
        assert_eq!(Asking::stops(10), [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
        assert_eq!(Asking::stops(24).len(), 24);
        let of_59 = Asking::stops(59);
        assert_eq!((of_59.len(), of_59[0], of_59[1], of_59[19], of_59[20]), (21, 1, 3, 57, 59));
        let of_25 = Asking::stops(25);
        assert_eq!((of_25.len(), of_25[0], of_25[1], of_25[12], of_25[13]), (14, 1, 2, 24, 25));
        // The window comes up at half and one of the most.
        assert_eq!((59 / 2 + 1, 29 / 2 + 1), (30, 15));
    }
}
