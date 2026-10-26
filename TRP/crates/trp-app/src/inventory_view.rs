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
    apply_item, can_pick_up, equip, go_together, is, move_item, pick_up, quick_find, rounds_to_load, split_item, split_most, take_out, takes_round, transfer_item, transfer_most, Applied, Bound,
    Equipment, Id, Item, Order, Place, Refusal, Rules, Spot, HAND_SLOTS,
};

use crate::controller::Controller;
use crate::ui_sound::{Sound, Ui, Way};
use crate::keys::Bindings;
use crate::range::Range;
use crate::weapon::{HandsRig, Stowed, WeaponRig};
use crate::{GameData, Options, View};

// ==== The carried things, the magazines' work and the windows' state ====

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
    /// A loose container being searched (`SEARCH` on a thing on the
    /// ground): lifted into `away` at the first number, shown in place of
    /// the second, to go back where it lay (the third) when the screen
    /// shuts (`search_ground`, `put_back_ground`).
    pub searched_ground: Option<(usize, usize, Vec3)>,
    /// The screen opened from the workbench's `REPAIR GEAR` (the user's): a
    /// repair kit let go on what it mends opens its repair window
    /// (`repair_view`), and nothing is moved.
    pub repairing: bool,
    /// Each repair window's own state, by its thing (`repair_view::Repair`).
    pub repairs: Vec<crate::repair_view::Repair>,
    /// The repair's figures (`trp_sim::repair::Settings`), read as the run
    /// begins.
    pub repair_settings: Option<std::sync::Arc<trp_sim::repair::Settings>>,
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
    /// (`icons`: its model drawn once and kept). Written without telling
    /// of a change to what is carried: a picture come is put in its
    /// spinner's place (`PicturesArrived`), not drawn with the screen
    /// afresh.
    pub pictures: HashMap<String, Picture>,
    /// Whether the pictures are made at all (`ICON GENERATION`), those that
    /// could not be, and the spinner shown in a box while its picture is
    /// still to come (where the game's view shows its loader).
    pub pictures_on: bool,
    pub picture_failed: std::collections::HashSet<String>,
    pub spinner: Handle<Image>,
    /// What is bound to the quick-use keys.
    pub bound: Bound,
    /// The grenade the grenade's key takes and its view shows (`quick_keys`).
    pub top_grenade: Option<Id>,
    /// A magazine being loaded or unloaded a round at a time
    /// (`work_magazine`), and the base times a round takes.
    pub magazine_work: Option<MagazineWork>,
    /// The picture's key a magazine at work is shown by (`shown_key`): as it
    /// began, asked again only where the game asks (`LoadMagazineProcess`:
    /// a round that adds or takes away a stack, the load's last round, an
    /// unload that leaves it empty; inventory spec, "A magazine's picture
    /// at work").
    pub frozen_keys: HashMap<Id, String>,
    /// Switches flipped since the hands last heard (the part, how it
    /// stands now): `settle` asks the gesture and the sound for one worn on
    /// the head (inventory spec, "A switch on the head").
    pub switched: Vec<(Id, bool)>,
    /// A fold of the weapon in hand (or of a part of it) asked from the
    /// menu, for its hands to take or not (`settle`; inventory spec, "A
    /// fold from the menu").
    pub fold_asked: Option<Id>,
    pub round_times: RoundTimes,
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
    /// A magazine out of the weapon with nowhere to go, until it leaves
    /// the hand and falls (firing spec, "The quick reload").
    pub falling: Vec<Item>,
    /// What has been put on the ground this frame and how it left the
    /// character, for the physics to set going (`loose`).
    pub thrown: Vec<(Id, crate::loose::How)>,
    /// The game's own views of a thing in a slot and in a grid, which a
    /// thing is drawn by with `TARKOV UI` on (none with it off).
    pub item_looks: [Option<std::sync::Arc<crate::health_view::ItemViewLook>>; 2],
    /// The magazines whose rounds the character knows (the profile's
    /// `CheckedMagazines`): every one carried as the run begins
    /// (`LocalPlayer.Create`), and the weapon's own as it is checked in the
    /// hands (`FirearmController.CheckAmmo`); a magazine looted later is
    /// not, and its view writes `?` for its count (`GetAmmoCountByLevel`).
    pub checked: std::collections::HashSet<Id>,
    /// What Strength makes of carrying (`skills`): at its elite level the
    /// weapons weigh nothing (`TotalWeightEliteSkill`), and the overweight
    /// limits are times one and its lift (`CarryingWeightRelativeModifier`).
    pub strength_elite: bool,
    pub carry: f32,
    /// The character's mastering level of its weapons (`--mastering`, the
    /// same for every weapon here), which a weapon's view in a slot shows.
    pub mastering: u32,
    /// The words of the keys of the four slots that have one of their own
    /// (the sheath, the holster, the sling, the back), by slot, which a
    /// thing in one is labelled with as a thing bound to a key is
    /// (`BindPanel`); kept as the bindings change (`name_slot_keys`).
    pub slot_keys: Vec<(&'static str, String)>,
    /// What the window that shows a thing says of it: its name, its kind
    /// and its description (`trp_data::inventory::item_texts`).
    pub texts: trp_data::inventory::ItemTexts,
    /// The game's window that shows a thing, its size in units, where it
    /// is drawn (`TARKOV UI`): opened where the game opens one.
    pub info_size: Option<Vec2>,
    /// Whether a container's window is the game's (`TARKOV UI`), which
    /// opens where the game opens one.
    pub game_grid_window: bool,
    /// The same of the game's window that edits a tag, and the most letters
    /// its field takes; and each tag being edited in one.
    pub tag_size: Option<Vec2>,
    pub tag_limit: u32,
    pub tag_edits: Vec<TagEdit>,
    /// The wish list (inventory spec, "The wish list"): kinds of thing,
    /// each in a group; empty as the run begins (the profile's not read).
    pub wishlist: trp_sim::item_menu::Wishlist,
}

/// The four slots with a key of their own and their bindings, as the
/// quick-use bar has them.
const SLOT_BINDINGS: [(&str, &str); 4] = [("Scabbard", "Knife"), ("Holster", "SecondaryWeapon"), ("FirstPrimaryWeapon", "PrimaryWeaponFirst"), ("SecondPrimaryWeapon", "PrimaryWeaponSecond")];

/// Keeps the words of the four slots' keys as the bindings have them.
fn name_slot_keys(bindings: Res<Bindings>, mut carried: ResMut<Carried>) {
    if !bindings.is_changed() && !carried.slot_keys.is_empty() {
        return;
    }
    let named: Vec<(&'static str, String)> = SLOT_BINDINGS.iter().map(|(slot, binding)| (*slot, bindings.caption(binding).to_owned())).collect();
    if carried.slot_keys != named {
        carried.slot_keys = named;
    }
}

/// Every magazine in or under an item (`EPlayerItems.NonQuestItems`'
/// magazines, for those carried as the run begins).
pub fn magazines_in(item: &Item) -> std::collections::HashSet<Id> {
    let mut found = std::collections::HashSet::new();
    let mut stack = vec![item];
    while let Some(item) = stack.pop() {
        if item.kind.class_names.iter().any(|class| class == "Magazine") {
            found.insert(item.id);
        }
        stack.extend(item.contents());
    }
    found
}

/// The pictures that have come since the screen last took them, or that
/// cannot be made, by template (`icons`): the boxes still showing a
/// spinner for one have it put in its place (`fill_pictures`).
#[derive(Resource, Default)]
pub struct PicturesArrived(pub Vec<String>);

/// The things whose box blinks (inventory spec, "A box that blinks"): a
/// move that waits on the hands, as the game's does (`ItemView.Blinking`,
/// a view being added or removed until its operation succeeds, which for
/// the character's own hands is when the weapon put in the slot they
/// last drew from has appeared in them, or the one taken out of its slot
/// has been put away and the next is in hand). Kept by `settle`, drawn by
/// `blink`; a blinking box takes no clicks (`ItemView.IsInteractive`).
#[derive(Resource, Default)]
pub struct Blinking(pub Vec<Id>);

/// The base times a magazine's round takes to load and to unload, from the
/// install's globals (`BaseLoadTime`, `BaseUnloadTime`), the elite level's
/// progress (`LoadTimeSpeedProgress`), and the time a magazine takes to
/// check (`BaseCheckTime`).
#[derive(Clone, Copy, Debug)]
pub struct RoundTimes {
    pub load: f32,
    pub unload: f32,
    pub progress: f32,
    pub check: f32,
    /// Whether a shot unknows the magazine it leaves neither empty nor full
    /// (`UncheckOnShot`).
    pub uncheck_on_shot: bool,
    /// Mag Drills' buffs as the skill's level gives them (`skills`): the
    /// percentages off a round's and a check's time, and the two elites.
    pub drills: trp_sim::skills::MagDrills,
}

impl Default for RoundTimes {
    /// As the install read on 2026-10-17 has them.
    fn default() -> Self {
        Self { load: 0.85, unload: 0.3, progress: 1.0, check: 3.0, uncheck_on_shot: true, drills: trp_sim::skills::MagDrills::default() }
    }
}

impl RoundTimes {
    pub fn of(db: &trp_data::Database) -> Self {
        let read = |name: &str, otherwise: f32| db.config.get(name).and_then(|value| value.as_f64()).map_or(otherwise, |value| value as f32);
        let base = Self::default();
        Self {
            load: read("BaseLoadTime", base.load),
            unload: read("BaseUnloadTime", base.unload),
            progress: read("LoadTimeSpeedProgress", base.progress),
            check: read("BaseCheckTime", base.check),
            uncheck_on_shot: db.config.get("UncheckOnShot").and_then(|value| value.as_bool()).unwrap_or(base.uncheck_on_shot),
            drills: base.drills,
        }
    }

    /// A check's time for a magazine of that modifier (`CheckTimeModifier`),
    /// less Mag Drills' check buff (0.8 a level).
    pub fn check(&self, modifier: f32) -> f32 {
        trp_sim::MagazineProcess::round_time(self.check, self.drills.check, modifier)
    }

    /// A round's time for a magazine of that modifier, less Mag Drills'
    /// load buff (0.6 a level).
    pub fn load(&self, modifier: f32) -> f32 {
        trp_sim::MagazineProcess::round_time(self.load, self.drills.load, modifier)
    }

    pub fn unload(&self, modifier: f32) -> f32 {
        trp_sim::MagazineProcess::round_time(self.unload, self.drills.unload, modifier)
    }

    /// The share of the first round's time each later one is quicker by, at
    /// Mag Drills' elite level (`MagDrillsLoadProgression`); a load only
    /// (an unload is never given it: `UnloadMagazineProcess(elite: false)`).
    pub fn load_elite(&self) -> Option<f32> {
        self.drills.load_progression.then_some(self.progress)
    }
}

/// A magazine at work, and what it stops on: the tab it began under, and
/// the medical items started then (`Controller::meds_started`).
pub struct MagazineWork {
    pub process: trp_sim::MagazineProcess,
    began: Option<(usize, u32)>,
    /// What its load component shows (inventory spec, "The load
    /// component"): its figures as it began, and the time since.
    pub shown: Shown,
}

/// A magazine's work as its load component shows it: a round's time as
/// begun with, the rounds to go and those done then, and the seconds since.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Shown {
    pub round: f32,
    pub to_go: u32,
    pub done: u32,
    pub since: f32,
}

impl Shown {
    /// The share of the work done (the spinner's fill) and the share of the
    /// present round (the loader's turn).
    pub fn shares(&self) -> (f32, f32) {
        let before = self.round * self.done as f32;
        let at = self.since + before;
        let whole = self.round * self.to_go as f32 + before;
        let done = if whole > 0.0 { (at / whole).clamp(0.0, 1.0) } else { 1.0 };
        let round = if self.round > 0.0 { (at % self.round) / self.round } else { 0.0 };
        (done, round)
    }
}

impl MagazineWork {
    /// A work begun with so many rounds to go (one for a check).
    pub fn new(process: trp_sim::MagazineProcess, to_go: u32) -> Self {
        let round = process.round_wait();
        Self { process, began: None, shown: Shown { round, to_go, done: 0, since: 0.0 } }
    }
}

/// A magazine loaded or unloaded a round at a time (inventory spec,
/// "Loading and unloading a magazine"), or checked (inventory spec,
/// "Checking a magazine"): each round when its wait is over, with its
/// sound; a check when its one wait is, the magazine's count known from
/// then on and its drop sound played. Each wait counts from the frame the
/// work began on (its timer starts in the click's frame, after that
/// frame's time). Stopped where it is by the inventory shut, a tab
/// changed, a medical item starting its work, or a round that cannot move.
fn work_magazine(time: Res<Time>, controller: Res<Controller>, selected: Res<Selected>, mut carried: ResMut<Carried>, mut sounds: MessageWriter<Sound>) {
    // Looked at first without a write, which would have the page drawn
    // afresh on every frame.
    if carried.magazine_work.is_none() {
        return;
    }
    // Its picture's key as it begins: held while it works.
    if let Some(magazine) = carried.magazine_work.as_ref().map(|work| work.process.magazine).filter(|magazine| !carried.frozen_keys.contains_key(magazine)) {
        if let Some(key) = carried.find(magazine).map(crate::icons::picture_key) {
            let carried = carried.bypass_change_detection();
            carried.frozen_keys.retain(|id, _| *id == magazine);
            carried.frozen_keys.insert(magazine, key);
        }
    }
    let Some(work) = carried.bypass_change_detection().magazine_work.as_mut() else { return };
    let first = work.began.is_none();
    if first {
        info!("a magazine's {:?} begun, a wait of {:.3} s", work.process.work, work.process.round_wait());
    }
    if !first {
        work.shown.since += time.delta_secs();
    }
    let (tab, meds) = *work.began.get_or_insert((selected.0, controller.meds_started));
    if !controller.inventory_open || selected.0 != tab || controller.meds_started != meds {
        info!("a magazine's work stopped: the inventory shut, a tab changed or a medical item begun");
        carried.magazine_work = None;
        return;
    }
    if first || !work.process.step(time.delta_secs()) {
        return;
    }
    let (magazine, source, check) = (work.process.magazine, work.process.source(), work.process.work == trp_sim::magazine_process::Work::Check);
    let carried = &mut *carried;
    if check {
        carried.magazine_work = None;
        let Some(found) = carried.find(magazine) else { return };
        let (name, group) = (carried.name(found), found.kind.sound.clone());
        info!("a magazine checked: {name}");
        carried.checked.insert(magazine);
        carried.note = format!("Magazine checked: {name}");
        sounds.write(Sound::Item(group, Way::Drop));
        return;
    }
    let worn = carried.equipment.item.id;
    let stacks_of = |carried: &Carried| carried.find(magazine).map_or(0, |found| found.loose.len());
    let stacks_before = stacks_of(carried);
    let roots = &mut [&mut carried.equipment.item, &mut carried.loot];
    let (moved, sound) = match source {
        Some(source) => (trp_sim::inventory::load_round(roots, source, magazine).is_ok(), Sound::Load),
        None => (trp_sim::inventory::unload_round(roots, magazine, worn, &carried.rules).is_ok(), Sound::Unload),
    };
    let left = roots.iter().find_map(|root| root.find(magazine)).map_or(0, trp_sim::inventory::magazine_rounds);
    // A magazine left empty or full by a round in or out is known
    // (`CheckMagazineAmmoDepend`, as each add or removal raises it).
    let capacity = roots.iter().find_map(|root| root.find(magazine)).map_or(0, |found| found.kind.capacity);
    if moved && trp_sim::inventory::known_after(left, capacity, carried.checked.contains(&magazine), false) {
        carried.checked.insert(magazine);
    }
    let done = match carried.magazine_work.as_mut() {
        Some(work) if moved => work.process.moved() || (source.is_none() && left == 0),
        _ => true,
    };
    if moved {
        sounds.write(sound);
    }
    // Its picture asked again where the game asks: a stack added or taken
    // away; at the end (the load's last round, an unload left empty, or a
    // round that could not move) its own key again.
    if done {
        carried.frozen_keys.remove(&magazine);
    } else if moved && stacks_of(carried) != stacks_before {
        if let Some(key) = carried.find(magazine).map(crate::icons::picture_key) {
            carried.frozen_keys.insert(magazine, key);
        }
    }
    if done {
        info!("a magazine's work over: {left} rounds in it{}", if moved { "" } else { " (a round could not move)" });
        carried.magazine_work = None;
    }
}

/// The picture's key a thing is shown by: a magazine at work's as it was
/// held (`Carried::frozen_keys`), else its own (`icons::picture_key`).
/// `PROVISIONAL`: a work stopped shows its own key again at once, where the
/// game's view keeps the old picture until the view is made anew.
pub(crate) fn shown_key(carried: &Carried, item: &Item) -> String {
    let working = carried.magazine_work.as_ref().is_some_and(|work| work.process.magazine == item.id);
    working.then(|| carried.frozen_keys.get(&item.id).cloned()).flatten().unwrap_or_else(|| crate::icons::picture_key(item))
}

/// A box's spinner, shown while its thing's picture is still to come, and
/// what is wanted in its place: the thing's picture's key (`icons::
/// picture_key`: its template, marked where folded), the plain shape of
/// its sort should no picture come, the box's inside and whether the thing
/// lies turned.
#[derive(Component)]
struct Loading {
    key: String,
    sort: &'static str,
    size: Vec2,
    turned: bool,
}

/// A window over the screen: a container's grids, what a thing is, or its
/// tag being edited.
#[derive(Clone, Copy, PartialEq)]
pub struct Win {
    pub item: Id,
    pub kind: WinKind,
    /// Its top left corner in the window, in pixels.
    pub at: Vec2,
    /// When it was opened, counted over the run (`WindowData.Index`).
    pub opened: u64,
    /// The size a window that shows a thing is stretched to, in units
    /// (`StretchWindow`); none as it opens.
    pub stretched: Option<[f32; 2]>,
    /// Whether it has been placed by its own size, once laid out.
    pub fitted: bool,
    /// Where its thing lay as it opened, and where each thing that held it
    /// lay, nearest first (`GridWindow._parents`): a container's window
    /// shuts when any of them is moved.
    pub held: [Option<(Id, Spot)>; 8],
}

/// What a window over the screen is (`WindowData.WindowType`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WinKind {
    /// What a thing is (`InfoWindow`).
    Info,
    /// A container's grids (`GridWindow`).
    Grids,
    /// A thing's tag being edited (`EditTagWindow`).
    Tag,
    /// A thing being repaired with a kit (`RepairWindow`; `repair_view`).
    Repair,
}

/// A tag being edited in its window (`EditTagWindow`): the thing's, the
/// name typed so far, the colour chosen, whether its field has the keys,
/// and what the name was when the field took them.
#[derive(Clone, Debug, PartialEq)]
pub struct TagEdit {
    pub item: Id,
    pub name: String,
    pub color: u32,
    pub focused: bool,
}

/// Whether a tag's field takes a letter (`ValidationInputField` as the tag's
/// is set, `Everything`: letters, figures and the underscore of any script
/// (`\w`), the space, and its list of marks).
fn tag_letter(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || "., /\\()[]{}<>-!?'\"@;:+=*^%&$|#№".contains(c)
}

/// The count of windows opened (`WindowData._nextIndex`).
static WINDOWS_OPENED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

// ==== Asking, the pictures, the reach and the loot in sight ====

/// What the window that asks how many is asking for: a stack split into a
/// place or put onto another, or a share of food or drink to be used
/// (`ItemUiContext.ShowSplitMenuDialog`), of `whole`, its most.
#[derive(Clone, Copy, PartialEq)]
enum Asked {
    Move(Target),
    Use { whole: u32 },
}

/// The window that asks how many of a stack are to go (`SplitDialog`):
/// which stack, where to, how many at the most and how many now. For a
/// use, how much of what is left, which is `most`.
#[derive(Clone, Copy, PartialEq)]
pub struct Asking {
    id: Id,
    asked: Asked,
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

    /// The window that asks how much of food or drink to use
    /// (`ShowSplitMenuDialog`, an `IntSlider`): its track runs from 1 to
    /// the whole, what is already gone lies before the first place the
    /// knob may stand (`(int)(whole - left + 1)`), and the figure is how
    /// much is used, from 1 to what is left: all of that to begin with.
    fn of_use(id: Id, whole: f32, left: f32, at: Vec2) -> Option<Self> {
        let whole_units = whole as u32;
        let first = ((whole - left + 1.0) as u32).max(1);
        let most = (whole_units + 1).checked_sub(first).filter(|most| *most >= 1)?;
        Some(Self { id, asked: Asked::Use { whole: whole_units }, title: "USE", most, count: most, at, typed: false })
    }

    /// For a use, the knob's place on the track as a share of it (the
    /// track running from 1 to the whole), and what is gone before the
    /// knob's first place, the same way; for a split, the stop reached.
    fn shares(&self) -> (f32, f32) {
        match self.asked {
            Asked::Use { whole } => {
                let span = whole.saturating_sub(1).max(1) as f32;
                let first = whole + 1 - self.most;
                let knob = first + self.count.clamp(1, self.most) - 1;
                ((knob - 1) as f32 / span, (first - 1) as f32 / span)
            }
            Asked::Move(_) => {
                let stops = Self::stops(self.most);
                let reached = stops.iter().rposition(|stop| *stop <= self.count).unwrap_or(0);
                (if stops.len() > 1 { reached as f32 / (stops.len() - 1) as f32 } else { 1.0 }, 0.0)
            }
        }
    }

    /// The figure for the slider held at a share of its track.
    fn slid_to(&self, share: f32) -> u32 {
        let share = share.clamp(0.0, 1.0);
        match self.asked {
            Asked::Use { whole } => {
                let first = whole + 1 - self.most;
                let knob = (1.0 + share * whole.saturating_sub(1) as f32).round() as u32;
                knob.clamp(first, whole) + 1 - first
            }
            Asked::Move(_) => {
                let stops = Self::stops(self.most);
                stops[((share * (stops.len() - 1) as f32).round() as usize).min(stops.len() - 1)]
            }
        }
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
    /// The workbench is in sight (`range::Workbench::in_sight`), with
    /// nothing else looked at.
    pub workbench: bool,
    /// The game's menu for it (`AvailableInteractionState`): every line
    /// it offers, and the one chosen, which the wheel moves and the
    /// interact key does.
    pub menu: Option<Menu>,
    /// The name the game's panel shows under the lines: a loose thing's
    /// alone (`TargetName`: its short name, a stack's count after it), the
    /// app's own look a thing's too; and the panel's error in place of the
    /// lines where a loose thing offers none ("No Space ({0})").
    pub name: Option<String>,
    pub error: Option<String>,
    /// A loose thing is near the look though none is looked at (the game's
    /// sense, `Player.Sense`): the panel's dot shows.
    pub sense: bool,
}

/// The game's own words for the menu's lines, by the names its code asks
/// for them by (the install's English texts); a line the install has no
/// words for is shown by its name.
#[derive(Resource, Default)]
pub struct MenuTexts(pub HashMap<String, String>);

impl MenuTexts {
    pub(crate) fn of(&self, action: Action) -> String {
        self.0.get(&action.name().to_lowercase()).cloned().unwrap_or_else(|| action.name().to_owned()).to_uppercase()
    }
}

/// How far down the crate's grids the screen's panel for them is scrolled,
/// in pixels.
#[derive(Resource, Default)]
pub struct LootScroll(f32);

/// The rows of each of the open container's grids that its panel shows
/// now, from the first to the one after the last: the game's container
/// panel makes the views of what is in sight alone (its `GridViewMagnifier`
/// on the panel's scroll area), and asks for their pictures then
/// (`icons::ask_icons`).
#[derive(Resource, Default, PartialEq)]
pub struct LootInSight(pub Vec<[u32; 2]>);

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

/// The class of the game's repair kits (`RepairKits`), by its name.
const REPAIR_KITS_CLASS: &str = "RepairKits";

/// Whether anything of a class is carried in or on a thing.
fn carries_class(item: &Item, class: &str) -> bool {
    is(item, class) || item.contents().any(|inside| carries_class(inside, class))
}

/// Whether a repair kit mends a thing: the thing wears and is of a class
/// its kit names (`TargetItemFilter`).
pub fn kit_mends(kit: &Item, thing: &Item) -> bool {
    is(kit, REPAIR_KITS_CLASS) && thing.repairable.is_some() && thing.kind.classes.iter().any(|class| kit.kind.repair_targets.contains(class))
}

/// A thing lying on the ground, as drawn.
#[derive(Component)]
struct LooseItem(Id);

// ==== The screen's measures, colours and tabs ====

/// The tabs in the game's order (`EInventoryTab`), with whether each can
/// be opened here. The captions are plain English words, not the game's
/// own texts.
///
/// The last is not the game's: its settings are a screen of its menu, and
/// the user asked for them here, as a tab at the far right (2026-10-12;
/// `options_view.rs`).
pub const TABS: [(&str, bool); 9] = [
    ("OVERALL", true),
    ("GEAR", true),
    ("HEALTH", true),
    ("SKILLS", true),
    ("MAP", false),
    ("TASKS", false),
    ("ACHIEVEMENTS", false),
    ("PRESTIGE", false),
    ("OPTIONS", true),
];
/// The OVERALL tab (`overall_view`).
pub const OVERALL: usize = 0;
/// The SKILLS tab (`skills_view`).
pub const SKILLS: usize = 3;
/// The tab the game opens on.
const GEAR: usize = 1;
/// The health tab, which shows the gear tab's page with the health panel
/// in its left panel (`ItemsPanel.UpdateEquipment`).
const HEALTH: usize = 2;
/// The options' tab.
pub const OPTIONS: usize = 8;

/// Whether a tab can be opened: one built, and with the options'
/// `INVENTORY SYSTEM` off, the options' alone.
fn tab_open(index: usize, features: &crate::options_view::Features) -> bool {
    TABS[index].1 && (features.inventory || index == OPTIONS)
}

/// The equipment panel's views in the order the game's panel names them,
/// each with a caption (an English word of the app's) and where it sits
/// if the install's own panel could not be read: left, top, width and
/// height in the panel's units, three to a row. Where the install's panel
/// is read (`trp_data::screen::GearPanel`), its places are used instead.
const EQUIPMENT: [(&str, &str, [f32; 4]); 11] = [
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
    ("Dogtag", "DOGTAG", [0.0, 270.0, 125.0, 50.0]),
];
/// The panel's own size where the install's is not read.
const PANEL_SIZE: [f32; 2] = [494.0, 650.0];
/// The equipment panel's size in its own units: the install's, or the
/// app's where that could not be read.
pub fn gear_panel(carried: &Carried) -> [f32; 2] {
    if carried.panel.slots.is_empty() {
        PANEL_SIZE
    } else {
        carried.panel.size
    }
}

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
pub const SCREEN_HEIGHT: f32 = 1080.0;
/// How high the game's bar of tabs is (`Tab Bar`), in those units.
const TAB_BAR: f32 = 43.0;
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
pub(crate) const DROP_AHEAD: f32 = 0.4;
const LOOSE_REACH: f32 = 0.25;
/// Where a magazine let fall lies: so far ahead of the character and so
/// far to its right, in metres. ESTIMATE: the game drops it from the
/// weapon's own magazine and lets it fall (`DropMod`); this is under
/// where a rifle is held.
const MAGAZINE_DROP: [f32; 2] = [0.35, 0.05];

/// A place on the ground so far ahead of a character standing at
/// `position` (the simulation's axes) and turned to `yaw`, and so far to
/// its right: in the engine's axes.
pub(crate) fn beside(position: [f32; 3], yaw: f32, [ahead, right]: [f32; 2]) -> Vec3 {
    let (sin, cos) = yaw.to_radians().sin_cos();
    Vec3::new(position[0] + sin * ahead + cos * right, position[1], -(position[2] + cos * ahead - sin * right))
}

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
/// The game's colour for the box under the mouse (`ItemView.
/// DefaultSelectedColor`: white at 77 of 255), laid over the box's own.
const POINTED: (Srgba, f32) = (Srgba::WHITE, 77.0 / 255.0);
/// What goes with it, in orange (the user, 2026-10-14): the game's own
/// orange for a box (a thing with a repair's bonus, `GridItemView`), laid
/// over the box at a share the app's, and its border.
const GOES_WITH: (Srgba, f32) = (Srgba::new(209.0 / 255.0, 153.0 / 255.0, 45.0 / 255.0, 1.0), 0.40);
const SLOT: Color = Color::srgba(0.07, 0.08, 0.08, 0.62);
const SHAPE: Color = Color::srgb(0.27, 0.26, 0.22);

// ==== The screen's components ====

#[derive(Component)]
struct Screen;
#[derive(Component)]
struct TabButton(usize);
#[derive(Component)]
struct Page(usize);
#[derive(Component)]
struct GearPage;
/// The game's bar of tabs (`TARKOV UI`): the node it is drawn into, as
/// large as the window; what of it is drawn afresh as the tabs change; and
/// a tab's button over it.
#[derive(Component)]
struct GameTabBar;
#[derive(Component)]
struct GameTabLook;
#[derive(Component)]
struct GameTab;
/// The game's back button over the bar (`TARKOV UI`): a click shuts the
/// screen, as the game's does.
#[derive(Component)]
struct GameBack;
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
    /// Its own border's colour, which a held thing over it tints.
    border: Color,
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
/// the quick-use bar, a window (above the bar, as it is drawn: its place
/// among the windows from `WINDOW_LAYER`), the window that asks how many.
/// The mouse goes to the highest under it.
#[derive(Component)]
pub(crate) struct Cover(pub(crate) u8);
const BAR_LAYER: u8 = 200;
const WINDOW_LAYER: u8 = BAR_LAYER + 1;
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
/// The held thing's own picture in it (`DraggedItemView._mainImage`).
#[derive(Component)]
struct GhostPicture;
/// The cells of a grid the held thing would take, in the colour of what a
/// drop there would do (`GridView._highlightPanel`).
#[derive(Component)]
struct DropMark;
#[derive(Component)]
pub(crate) struct Prompt;
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

/// What a drop where the mouse is would mark this frame (`drag`), drawn by
/// `show_drop`: the cells of a grid it would take (left, top, width,
/// height in pixels) and the slot under it, each in the colour of what the
/// drop would do.
#[derive(Resource, Default, PartialEq)]
struct DropShown {
    cells: Option<([f32; 4], Color)>,
    slot: Option<(Id, usize, Color)>,
}

/// What a drop would do, in the game's colours for it (`GridView.
/// GetHighlightColor`: a move green, a merge, split or hand-over yellow,
/// anything else red; each at 0.57).
fn drop_colour(would: Option<&Result<&'static str, Refusal>>) -> Color {
    match would {
        Some(Ok("moved" | "put in")) => Color::srgba(0.06, 0.38, 0.06, 0.57),
        Some(Ok(_)) => Color::srgba(0.69, 0.66, 0.0, 0.57),
        _ => Color::srgba(0.68, 0.0, 0.0, 0.57),
    }
}

/// Draws what `drag` says a drop would mark: the grid's cells over the
/// grid, under the held picture; the slot's border tinted (`SlotView.
/// HighlightItemViewPosition`; its own colours are not read: the grid's,
/// `PROVISIONAL`), every other slot's border its own.
fn show_drop(shown: Res<DropShown>, mut mark: Query<(&mut Node, &mut Visibility, &mut BackgroundColor), With<DropMark>>, mut slots: Query<(&SlotBox, &mut BorderColor)>) {
    if let Ok((mut node, mut visibility, mut ground)) = mark.single_mut() {
        match shown.cells {
            Some(([x, y, w, h], colour)) => {
                let place = (Val::Px(x), Val::Px(y), Val::Px(w), Val::Px(h));
                if (node.left, node.top, node.width, node.height) != place {
                    (node.left, node.top, node.width, node.height) = place;
                }
                if ground.0 != colour {
                    ground.0 = colour;
                }
                if *visibility != Visibility::Inherited {
                    *visibility = Visibility::Inherited;
                }
            }
            None => {
                if *visibility != Visibility::Hidden {
                    *visibility = Visibility::Hidden;
                }
            }
        }
    }
    for (slot, mut border) in &mut slots {
        let want = match shown.slot {
            Some((holder, at, colour)) if holder == slot.holder && at == slot.slot => colour,
            _ => slot.border,
        };
        if *border != BorderColor::all(want) {
            *border = BorderColor::all(want);
        }
    }
}

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
    /// A press on a window (`Window`'s pointer handlers): which, where the
    /// mouse went down, whether on its caption's bar, whether a thing in it
    /// took the press (an `ItemView` handles its own click and drag), and
    /// whether a drag of it has begun.
    window_press: Option<WindowPress>,
    /// Whether the slider of the window that asks how many is held.
    sliding: bool,
    /// A window that shows a thing being stretched: which, by which sides
    /// (`StretchArea.EStretchAreaType`), and the mouse's last place as the
    /// stretch has taken it (`_lastPosition`).
    stretching: Option<(usize, u8, Vec2)>,
}

#[derive(Clone, Copy)]
struct WindowPress {
    index: usize,
    from: Vec2,
    by_caption: bool,
    thing: bool,
    dragged: bool,
}

/// How far the mouse has to move with the button down before a thing is
/// carried and not clicked, in pixels. ESTIMATE: the engine's own default
/// for its screens (`EventSystem.pixelDragThreshold`); what the game's
/// scene sets is not read.
pub(crate) const DRAG_THRESHOLD: f32 = 10.0;
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

// ==== The plugin, the screen drawn and the carried model ====

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
            .init_resource::<GameScrolls>()
            .init_resource::<StashFills>()
            .init_resource::<LootInSight>()
            .init_resource::<MenuTexts>()
            .init_resource::<PicturesArrived>()
            .init_resource::<Blinking>()
            .init_resource::<HandsHeld>()
            .init_resource::<Dragging>()
            .init_resource::<DropShown>()
            .add_systems(Update, publish_drag.after(drag))
            .add_systems(Update, show_drop.after(drag).run_if(crate::running))
            .add_systems(Update, info_tips.after(rebuild).run_if(crate::running))
            .add_systems(Update, tint_unusable.after(rebuild))
            .add_systems(Update, work_magazine.after(drag).before(rebuild).run_if(crate::running))
            .add_systems(Update, tag_windows.after(drag).before(rebuild).run_if(crate::running))
            .add_systems(Update, (screen_sounds, tab_sounds).after(toggle).run_if(crate::running))
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
                    (scripted_drags, reach, toggle.in_set(ScreenKeys), choose_tab, ask, (stretch_windows, windows).chain(), drag, quick_keys, scroll_loot, (scroll_views, scroll_info_texts), make_stash_fills, name_slot_keys, rebuild, game_tabs, fill_pictures, light_related, foresee, settle, (blink, turn_spinners).chain(), show_ground).chain().run_if(crate::running),
                    crate::body_view::show_worn,
                    crate::body_view::show_head_parts,
                    crate::body_view::turn_hinges,
                )
                    .chain()
                    .in_set(ScreenDrawn)
                    .after(crate::controller::step)
                    .before(crate::animation::play_clips),
            );
    }
}

/// Where the screen is drawn afresh: what fills its game's panels each
/// frame comes after, so that a page built on a frame is filled on it
/// (else it shows the scene's own colours for a frame: the parts' shapes
/// red).
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ScreenDrawn;

/// Where the screen's own keys are heard (the inventory's, Escape): what
/// else answers to one of them on the screen comes after, so that a key
/// it is about to take is seen here as still its own.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ScreenKeys;

pub fn words(text: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (Text::new(text), TextFont { font_size: size, ..default() }, TextColor(color))
}

fn placed(left: f32, top: f32, width: f32, height: f32) -> Node {
    Node { position_type: PositionType::Absolute, left: Val::Px(left), top: Val::Px(top), width: Val::Px(width), height: Val::Px(height), ..default() }
}

impl Carried {
    /// Mag Drills' elite (`MagDrillsInstantCheck`; `ItemController`,
    /// `MoveOperation`): a magazine put into the character's own things is
    /// known at once.
    fn known_on_arrival(&mut self, id: Id) {
        let mine = self.equipment.item.find(id).is_some_and(|item| item.kind.class_names.iter().any(|class| class == "Magazine"));
        if self.round_times.drills.instant_check && mine {
            self.checked.insert(id);
        }
    }

    /// The game's short name for an item, or the database's own.
    pub(crate) fn name(&self, item: &Item) -> String {
        self.names.get(&item.kind.template).filter(|name| !name.is_empty()).cloned().unwrap_or_else(|| item.kind.name.clone())
    }

    /// The words an item's view writes as its caption
    /// (`GridItemView.ItemShortName`): an armour's plate or insert in an
    /// armour's slot by its slot's name in the install's words (an
    /// insert's own short name is empty), anything else by its short name.
    pub(crate) fn view_name(&self, item: &Item) -> String {
        if item.kind.class_names.iter().any(|name| name == "ArmorPlate") {
            if let Some((holder, Spot::Slot(index))) = self.lies(item.id) {
                if let Some(slot) = self.find(holder).and_then(|holder| holder.kind.slots.get(index)).filter(|slot| slot.armour) {
                    return self.texts.say(&slot.name).unwrap_or_else(|| slot.name.clone());
                }
            }
        }
        self.name(item)
    }

    /// An item by its number, on the character or in the crate.
    pub fn find(&self, id: Id) -> Option<&Item> {
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
    /// A kit let go on a thing it mends: the thing's repair window opened
    /// with that kit (`RepairItemContext`), as the game opens a window.
    pub(crate) fn open_repair(&mut self, kit: Id, thing: Id) {
        self.repairs.retain(|repair| repair.thing != thing);
        self.repairs.push(crate::repair_view::Repair { thing, kit, selected: None, who: None });
        if let Some(had) = self.windows.iter().position(|window| window.item == thing && window.kind == WinKind::Repair) {
            self.shut_window(had);
        }
        self.open_window(thing, WinKind::Repair, Vec2::ZERO);
    }

    pub(crate) fn tidy(&mut self) {
        self.bound.settle(&self.equipment.item);
        // A window shuts when its thing is gone; a container's also when
        // it, or anything that held it as the window opened, has been moved
        // (`GridWindow`: the item's or a parent's remove event, which a move
        // raises at its old place, `IRemoveHandler.OnItemRemoved`).
        while let Some(index) = self.windows.iter().position(|window| self.find(window.item).is_none() || (window.kind == WinKind::Grids && self.held_by(window.item) != window.held)) {
            self.shut_window(index);
        }
        if self.priority.is_some_and(|priority| !self.windows.iter().any(|window| window.kind == WinKind::Grids && window.item == priority)) {
            self.priority = None;
        }
    }

    /// Where a thing lies and where each thing that holds it lies, nearest
    /// first.
    fn held_by(&self, id: Id) -> [Option<(Id, Spot)>; 8] {
        let mut chain = [None; 8];
        let mut at = id;
        for link in &mut chain {
            let Some((holder, spot)) = self.lies(at) else { break };
            *link = Some((holder, spot));
            at = holder;
        }
        chain
    }

    /// Shuts a window (`Window.Close`): a container's that was the
    /// priority window is that no longer; then, as the install's
    /// `PriorityWindowMode` (Auto) has it, the top-most container's window
    /// left is picked (`ItemUiContext.OpenWindow`'s `OnClose`:
    /// `SetSelected`), brought to the top and made the priority window.
    fn shut_window(&mut self, index: usize) {
        if index >= self.windows.len() {
            return;
        }
        let gone = self.windows.remove(index);
        if gone.kind == WinKind::Grids && self.priority == Some(gone.item) {
            self.priority = None;
        }
        if self.priority_auto {
            if let Some(last) = self.windows.iter().rposition(|window| window.kind == WinKind::Grids) {
                self.raise(last);
            }
        }
    }

    /// A window picked (`Window.OnPointerClick`, `OnBeginDrag`:
    /// `SetAsLastSibling`, `WindowContext.SetSelected`): brought to the top
    /// and, a container's with the install's `PriorityWindowMode` (Auto),
    /// made the priority window.
    fn raise(&mut self, index: usize) {
        if index >= self.windows.len() {
            return;
        }
        let window = self.windows.remove(index);
        if window.kind == WinKind::Grids && self.priority_auto {
            self.priority = Some(window.item);
        }
        self.windows.push(window);
    }

    /// Opens a window on a thing, or brings the one it has to the top
    /// (`ItemUiContext.OpenWindow`: `SetAsTopWindow`, which does not pick
    /// it); a new container's window is then where Ctrl and a click sends
    /// things (`GridWindow.Show`, with `PriorityWindowMode` Auto).
    fn open_window(&mut self, id: Id, kind: WinKind, at: Vec2) {
        if let Some(had) = self.windows.iter().position(|window| window.item == id && window.kind == kind) {
            let window = self.windows.remove(had);
            self.windows.push(window);
            return;
        }
        // A new one shuts the oldest of its kind where four of its kind
        // are open, else the oldest of all where eight are
        // (`ItemUiContext.OpenWindow`).
        if self.windows.iter().filter(|window| window.kind == kind).count() >= 4 {
            if let Some(oldest) = self.windows.iter().position(|window| window.kind == kind) {
                self.shut_window(oldest);
            }
        } else if self.windows.len() >= 8 {
            self.shut_window(0);
        }
        self.tidy();
        let opened = WINDOWS_OPENED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.windows.push(Win { item: id, kind, at, opened, stretched: None, fitted: false, held: self.held_by(id) });
        if kind == WinKind::Grids && self.priority_auto {
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

    /// A loose container searched (`SEARCH`; `InteractionContextHelper`:
    /// the game shows it in the inventory's view beside the equipment):
    /// lifted off the ground into that view, where it lay noted. Whether
    /// it was there.
    fn search_ground(&mut self, id: Id) -> bool {
        let Some(at) = self.ground.iter().position(|(item, _)| item.id == id) else { return false };
        self.put_back_ground();
        let (item, lay) = self.ground.remove(at);
        self.away.push(Some(item));
        let which = self.away.len() - 1;
        let before = self.loot_of;
        self.bring(which);
        self.searched_ground = Some((which, before, lay));
        true
    }

    /// The screen shut: a loose container searched goes back where it lay.
    fn put_back_ground(&mut self) {
        let Some((which, before, lay)) = self.searched_ground.take() else { return };
        if self.loot_of == which {
            self.bring(before);
        }
        if let Some(item) = self.away.get_mut(which).and_then(Option::take) {
            self.ground.push((item, lay));
        }
        if which + 1 == self.away.len() {
            self.away.pop();
        }
    }
}

// ==== Acting on things, their pictures and item views ====

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
        Refusal::HandsBusy => "the hands are busy with the weapon".into(),
        Refusal::Malfunction => "you must fix malfunctions first".into(),
    }
}

fn spawn(mut commands: Commands, options: Res<Options>, features: Res<crate::options_view::Features>, look: Res<crate::health_view::HealthLook>) {
    let open = options.inventory;
    // With `TARKOV UI` the screen begins at the window's top, where the
    // game's has its bar of tabs (the debug figures are hidden while it
    // is open: `hud::hide_over_screen`); the game's own bar is drawn there
    // where it was read (`game_tabs`).
    let tarkov = features.tarkov_ui;
    let game_bar = tarkov && look.tab_bar.is_some();
    let root = Node {
        position_type: PositionType::Absolute,
        left: Val::Px(0.0),
        top: Val::Px(0.0),
        width: Val::Percent(100.0),
        height: Val::Percent(100.0),
        flex_direction: FlexDirection::Column,
        padding: UiRect { left: Val::Px(18.0), right: Val::Px(18.0), top: Val::Px(if tarkov { 0.0 } else { 64.0 }), bottom: Val::Px(18.0) },
        row_gap: Val::Px(12.0),
        ..default()
    };
    let shown = if open { Visibility::Inherited } else { Visibility::Hidden };
    // The app's own ground over the world; the game's screen has none, the
    // world blurred behind it (`screen_blur`).
    let ground = if tarkov { Color::NONE } else { BACK };
    commands.spawn((Screen, root, BackgroundColor(ground), GlobalZIndex(10), shown)).with_children(|screen| {
        // The row of tabs; with `TARKOV UI` as high as the game's bar of
        // tabs (`Tab Bar`, 43 units) and over the page, which is as large
        // as the window.
        let row = match tarkov {
            true => Node { column_gap: Val::Px(6.0), align_items: AlignItems::Center, height: Val::Vh(TAB_BAR / SCREEN_HEIGHT * 100.0), flex_shrink: 0.0, ..default() },
            false => Node { column_gap: Val::Px(6.0), align_items: AlignItems::Center, ..default() },
        };
        screen.spawn((row, ZIndex(1))).with_children(|row| {
            // With the game's bar, the row only keeps its place.
            if game_bar {
                return;
            }
            for (index, (caption, _)) in TABS.into_iter().enumerate() {
                let color = if !tab_open(index, &features) { FAINT } else { WORDS };
                let back = if index == GEAR { LINE } else { PANEL };
                // The options' tab stands apart, against the right edge.
                let margin = if index == OPTIONS { UiRect::left(Val::Auto) } else { UiRect::default() };
                row.spawn((TabButton(index), Button, Node { padding: UiRect::axes(Val::Px(14.0), Val::Px(6.0)), margin, ..default() }, BackgroundColor(back)))
                    .with_children(|tab| {
                        tab.spawn(words(caption, 14.0, color));
                    });
            }
        });
        if game_bar {
            let whole = Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() };
            screen.spawn((GameTabBar, whole, ZIndex(2)));
        }
        let page = |index: usize| {
            let node = Node { flex_grow: 1.0, column_gap: Val::Px(14.0), display: if index == GEAR { Display::Flex } else { Display::None }, ..default() };
            (Page(index), node)
        };
        // The Gear page is drawn afresh whenever something has moved
        // (`rebuild`); the HEALTH tab shows it too, with the health panel
        // where the equipment panel was. With `TARKOV UI` it is the whole
        // window, its panels where the game's screen has them.
        match tarkov {
            true => {
                let (index, node) = page(GEAR);
                let whole = Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), width: Val::Percent(100.0), height: Val::Percent(100.0), ..node };
                screen.spawn((index, whole, GearPage));
            }
            false => {
                screen.spawn((page(GEAR), GearPage));
            }
        }
        // The options' page is drawn by its own module. With `TARKOV UI`
        // the screen has no ground of the app's (the world blurred behind
        // it, as the game's): the app's own pages keep one of their own.
        let options_page = Node { flex_grow: 1.0, flex_direction: FlexDirection::Column, row_gap: Val::Px(8.0), min_height: Val::Px(0.0), display: Display::None, padding: UiRect::all(Val::Px(if tarkov { 10.0 } else { 0.0 })), ..default() };
        let own_ground = BackgroundColor(if tarkov { BACK } else { Color::NONE });
        screen.spawn((Page(OPTIONS), crate::options_view::OptionsPage, options_page, own_ground));
        // The OVERALL tab's page, as large as the window, its things where
        // the game's screen has them (`overall_view`).
        let overall = Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), width: Val::Percent(100.0), height: Val::Percent(100.0), display: Display::None, ..default() };
        screen.spawn((Page(OVERALL), crate::overall_view::OverallPage, overall.clone()));
        // The SKILLS tab's page, as large as the window (`skills_view`).
        screen.spawn((Page(SKILLS), crate::skills_view::SkillsPage, overall));
    });
    // The item the mouse holds, over everything: its picture where it has
    // one (`DraggedItemView`), else the app's box with its name; under it
    // the cells it would take in a grid (`GridView._highlightPanel`).
    let ghost = Node { position_type: PositionType::Absolute, border: UiRect::all(Val::Px(1.0)), padding: UiRect::all(Val::Px(2.0)), overflow: Overflow::clip(), ..default() };
    commands.spawn((Ghost, ghost, BorderColor::all(WORDS), BackgroundColor(THING.with_alpha(0.8)), GlobalZIndex(21), Visibility::Hidden)).with_children(|held| {
        let whole = Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() };
        held.spawn((GhostPicture, whole, ImageNode::default(), Visibility::Hidden));
        held.spawn((GhostWords, words("", 10.0, WORDS)));
    });
    commands.spawn((DropMark, Node { position_type: PositionType::Absolute, ..default() }, BackgroundColor(Color::NONE), GlobalZIndex(20), Visibility::Hidden));
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
    let wanted = [Action::Open, Action::Close, Action::Search, Action::Unlock, Action::Breach, Action::Take, Action::EditBuild];
    let mut names: Vec<&str> = wanted.iter().map(|action| action.name()).collect();
    // The workbench's name over its menu (the hideout's area 10), and the
    // panel's error.
    names.push(WORKBENCH_NAME);
    names.push(NO_SPACE);
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
    carried.spinner = images.add(spinner_image());
}

/// The spinner: a ring with a quarter of it open, fading from its head to
/// its tail, in the screen's light colour on nothing. The app's (the
/// game's loader's own picture is not read).
fn spinner_image() -> Image {
    const SIDE: usize = SPINNER_PIXELS;
    let middle = SIDE as f32 / 2.0;
    let (outer, inner) = (middle - 1.5, middle - 8.5);
    let mut data = vec![0u8; SIDE * SIDE * 4];
    for (index, pixel) in data.chunks_exact_mut(4).enumerate() {
        let (dx, dy) = ((index % SIDE) as f32 + 0.5 - middle, (index / SIDE) as f32 + 0.5 - middle);
        let reach = (dx * dx + dy * dy).sqrt();
        // A pixel's worth of soft edge on either side of the ring.
        let edge = ((outer - reach).min(reach - inner) + 0.5).clamp(0.0, 1.0);
        // Round from the top the way it turns, 0 to 1: the head at the
        // end, the tail a quarter after the start, open before that.
        let round = (dx.atan2(-dy) / std::f32::consts::TAU).rem_euclid(1.0);
        let along = ((round - 0.25) / 0.75).clamp(0.0, 1.0);
        let alpha = edge * along * along;
        pixel.copy_from_slice(&[226, 228, 214, (alpha * 235.0) as u8]);
    }
    let size = Extent3d { width: SIDE as u32, height: SIDE as u32, depth_or_array_layers: 1 };
    Image::new(size, TextureDimension::D2, data, TextureFormat::Rgba8UnormSrgb, RenderAssetUsages::default())
}

/// How much of an item's box its picture may take, each way.
const PICTURE_SHARE: f32 = 0.86;

/// An item as drawn: a box with its picture, its name, the stack and,
/// where there is room for it, its weight. `size` is the box's, in pixels,
/// and `turned` whether the thing lies turned a quarter. The picture keeps
/// its own proportions: it is drawn in the middle of the box at the
/// largest size that fits, so a larger box has a larger picture and never
/// a stretched one.
#[allow(clippy::too_many_arguments)]
pub(crate) fn item_view(parent: &mut ChildSpawnerCommands, carried: &Carried, item: &Item, node: Node, wide: bool, size: Vec2, turned: bool, layer: u8, s: f32) {
    // With `TARKOV UI` on, the game's own view of a thing in a slot or in
    // a grid.
    if let Some(look) = carried.item_looks[if wide { 0 } else { 1 }].clone() {
        game_item_view(parent, carried, item, node, size, turned, layer, s, &look);
        return;
    }
    let stack = if item.count > 1 { format!(" x{}", item.count) } else { String::new() };
    let weight = if wide { format!("   {:.2} kg", item.weight()) } else { String::new() };
    // The key it is bound to, as the game marks a bound thing.
    let key = carried.bound.key_of(item.id).map(|key| format!("   [{}]", QUICK_KEYS[key].1)).unwrap_or_default();
    let node = Node { border: UiRect::all(Val::Px(1.0)), padding: UiRect::all(Val::Px(2.0)), overflow: Overflow::clip(), ..node };
    // On the wish list: its border blue, its group named at the bottom.
    let wish = carried.wishlist.get(&item.kind.template);
    let frame = if wish.is_some() { WISHED } else { LINE };
    parent.spawn((ItemView(item.id, layer), RelativeCursorPosition::default(), node, BorderColor::all(frame), BackgroundColor(THING))).with_children(|thing| {
        picture_in(thing, carried, item, size, turned, None);
        // A magazine worked: the app's look of the game's load component, a
        // bar along its foot filled by the work's share done.
        if carried.magazine_work.as_ref().is_some_and(|work| work.process.magazine == item.id) {
            let track = Node { position_type: PositionType::Absolute, left: Val::Px(0.0), right: Val::Px(0.0), bottom: Val::Px(0.0), height: Val::Px(3.0), ..default() };
            thing.spawn((track, BackgroundColor(Color::BLACK.with_alpha(0.6)))).with_children(|track| {
                track.spawn((AppSpinBar, Node { width: Val::Percent(0.0), height: Val::Percent(100.0), ..default() }, BackgroundColor(WORDS)));
            });
        }
        if let Some(group) = wish {
            let mark = Node { position_type: PositionType::Absolute, left: Val::Px(2.0), bottom: Val::Px(1.0), ..default() };
            thing.spawn(mark).with_children(|mark| {
                mark.spawn(words(group.name().to_owned(), 9.0, WISHED));
            });
        }
        // Its tag, at the top left in its colour (the game's place for it).
        if !item.tag.name.trim().is_empty() {
            let plate = Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), padding: UiRect::horizontal(Val::Px(3.0)), ..default() };
            thing.spawn((plate, BackgroundColor(crate::health_view::tag_colour(item.tag.color)))).with_children(|plate| {
                plate.spawn(words(item.tag.name.clone(), 10.0, WORDS));
            });
        }
        thing.spawn(words(format!("{}{stack}{weight}{key}", carried.name(item)), 10.0, WORDS));
    });
}

/// The app's colour of a thing on the wish list.
const WISHED: Color = Color::srgb(0.35, 0.62, 0.86);

/// A special slot's view's side (`Special Slot Template`), in units.
const SPECIAL_SIDE: f32 = 64.0;

/// The durability's bar's width at a slot's view's right, which its value
/// is put left of (`DurabilitySlider`'s `LayoutElement`, 12 units in the
/// prefab).
const VALUE_BAR: f32 = 12.0;

/// The mastering mark's height in a slot's view, in units, as the user's
/// own capture has it at 1080 lines (20 pixels; the mark's sprite is 22 by
/// 29): `ESTIMATE` until the layout group that sizes it is read.
const MASTERING_MARK_HEIGHT: f32 = 20.0;

/// What a thing's view writes at its bottom right with `TARKOV UI`
/// (`GridItemView.UpdateInfo`, `SetItemValue`, `SetCountValue`), as runs
/// of words each with its colour (the text's own where none) and its size
/// (the text's own where none); nothing for nothing. In a slot (`slot`) a
/// weapon's rounds are written large (`SlotItemView.ValueFormat`).
fn item_value(carried: &Carried, item: &Item, slot: bool) -> Vec<(String, Option<Color>, Option<f32>)> {
    let is = |class: &str| item.kind.class_names.iter().any(|name| name == class);
    let hex = |hex: u32| Some(Color::srgb_u8((hex >> 16) as u8, (hex >> 8) as u8, hex as u8));
    let (cream, red_orange, red) = (hex(0xdadabc), hex(0xff5335), hex(0xff0000));
    // Two figures in one colour (`EItemValueFormat.TwoValues`).
    let two = |a: String, b: String, color: Option<Color>| vec![(a, color, None), (format!("/{b}"), color, None)];
    // The view's own format: the first figure in its colour, large in a
    // slot, the second plain (`ValueFormat`).
    let format = |a: String, b: u32, color: Option<Color>| vec![(a, color, slot.then_some(30.0)), (format!("/{b}"), None, None)];
    // A magazine's count as the character knows it (`Magazine.
    // GetAmmoCountByLevel`, with no skill of magazine drills: the app has
    // none): exact when full (one short in a weapon worn, its chamber's),
    // words when part full, `?` unchecked.
    let known = |magazine: &Item, count: u32, most: u32, color: Option<Color>, worn: bool| -> Vec<(String, Option<Color>, Option<f32>)> {
        let said = if count == 0 {
            "0".to_owned()
        } else if !carried.checked.contains(&magazine.id) {
            "?".to_owned()
        } else if count >= if worn { most.saturating_sub(1) } else { most } {
            count.to_string()
        } else {
            const SHARES: [&str; 4] = ["~ Empty", "< 1/2", "~ 1/2", "~ Full"];
            let share = SHARES[((3.33 * count as f32 / most.max(1) as f32 + 0.5) as usize).min(3)];
            return vec![(share.to_owned(), color, None)];
        };
        format(said, most, color)
    };
    let worn = carried.equipment.item.slots.iter().flatten().any(|slotted| slotted.id == item.id);
    if is("Weapon") {
        let magazine = item.slots.iter().zip(&item.kind.slots).find(|(_, holder)| holder.name == trp_sim::inventory::MAGAZINE_SLOT).and_then(|(part, _)| part.as_ref());
        let (count, most) = match magazine {
            Some(magazine) => (trp_sim::inventory::magazine_rounds(magazine), magazine.kind.capacity),
            // With no magazine only a weapon loaded by its barrels counts
            // its chambers (`GridItemView.UpdateInfo`, `OnlyBarrel`); any
            // other shows nothing.
            None if item.kind.reload_mode == "OnlyBarrel" => {
                let chambers: Vec<&Option<Item>> = item.slots.iter().zip(&item.kind.slots).filter(|(_, holder)| holder.name.starts_with("patron_in_weapon")).map(|(part, _)| part).collect();
                (chambers.iter().filter(|part| part.is_some()).count() as u32, chambers.len() as u32)
            }
            None => (0, 0),
        };
        if most == 0 {
            return Vec::new();
        }
        let color = if (count as f32) / (most as f32) < 0.15 { red } else { cream };
        return match magazine {
            Some(magazine) => known(magazine, count, most, color, worn),
            None => format(count.to_string(), most, color),
        };
    }
    // Armour, a plate or an insert, and a rig that holds plates: its own
    // and its parts' durability summed, over their greatest, white, red at
    // none (`GridItemView.UpdateInfo`: every `RepairableComponent` in it,
    // where it has an `ArmorComponent` or an `ArmorHolderComponent`; each
    // sum rounded). The character's armour takes no hits here, so each is
    // as made (its template's).
    if item.kind.armor_class > 0 || is("ArmorPlate") || item.kind.slots.iter().any(|slot| slot.armour) {
        fn summed(item: &Item, sums: &mut [f32; 2]) {
            sums[0] += item.durability()[0];
            sums[1] += item.durability()[1];
            for part in item.slots.iter().flatten() {
                summed(part, sums);
            }
        }
        let mut sums = [0.0, 0.0];
        summed(item, &mut sums);
        if sums[1] > f32::EPSILON {
            let colour = if sums[0] <= 0.0 { hex(0xff0000) } else { hex(0xffffff) };
            return two(sums[0].round().to_string(), sums[1].round().to_string(), colour);
        }
    }
    if is("Magazine") {
        return known(item, trp_sim::inventory::magazine_rounds(item), item.kind.capacity, cream, worn);
    }
    if is("AmmoBox") {
        return two(item.contents().map(|round| round.count).sum::<u32>().to_string(), item.kind.capacity.to_string(), cream);
    }
    if is("Meds") && item.kind.max_hp_resource > 0.0 {
        let left = item.resource.unwrap_or(item.kind.max_hp_resource);
        return two(format!("{}", left.round() as i32), format!("{}", item.kind.max_hp_resource.round() as i32), red_orange);
    }
    if is("FoodDrink") && item.kind.max_resource > 1.0 {
        let left = item.resource.unwrap_or(item.kind.max_resource);
        return two(format!("{}", left.round() as i32), format!("{}", item.kind.max_resource.round() as i32), red_orange);
    }
    if is("BarterItem") && item.kind.max_resource > 0.0 {
        let left = item.resource.unwrap_or(item.kind.max_resource);
        return two(format!("{}", left as i32), format!("{}", item.kind.max_resource), cream);
    }
    // A repair kit's points left over its greatest, white, red at none:
    // floored above half, ceiled below (`GridItemView.UpdateInfo`).
    if is("RepairKits") && item.kind.repair_resource > 0.0 {
        let (left, most) = (crate::repair_view::kit_resource(item), item.kind.repair_resource);
        let shown = if left > most / 2.0 { left.floor() } else { left.ceil() };
        let colour = if left.ceil() <= 0.0 { red } else { hex(0xffffff) };
        return two(format!("{}", shown as i32), format!("{}", most as i32), colour);
    }
    // A key's uses left (the app keeps no count of a key's uses: as new).
    if is("Key") && item.kind.key_uses > 0 {
        return two(item.kind.key_uses.to_string(), item.kind.key_uses.to_string(), red_orange);
    }
    // A stack's count, in its colour (`SetCountValue`, `GetStackColor`).
    if item.count != 1 {
        return vec![(item.count.to_string(), Some(crate::health_view::stack_color(&item.kind.class_names)), None)];
    }
    Vec::new()
}

// ==== The game's item view, the spinners and the blinking ====

/// A thing's view drawn as the game's own (`TARKOV UI`; `health_view::
/// ItemViewLook`): its ground in its kind's colour (`ItemView.
/// UpdateStaticInfo`), its border where its view has one on, its picture,
/// its caption at the top right (the short name between two spaces in
/// #b6c1c7: `GridItemView.GetItemName`), a stack's count at the bottom
/// right (`SetCountValue`, in its colour: `GetStackColor`), and the label
/// of the key it is bound to at the top left (`BindPanel`); `s` pixels to
/// a unit. A view's top and bottom rows are laid out by groups that fit
/// their contents while the game runs: the label is put at the top left
/// at its picture's own size, the count against the bottom row's right
/// and bottom edges, as those groups put them.
#[allow(clippy::too_many_arguments)]
pub(crate) fn game_item_view(parent: &mut ChildSpawnerCommands, carried: &Carried, item: &Item, node: Node, size: Vec2, turned: bool, layer: u8, s: f32, look: &crate::health_view::ItemViewLook) {
    // Unusable by a malfunction the shooter knows of: its picture red on a
    // dark red ground (`GridItemView`'s `UnusableDueToMalfunction`).
    let root = if carried.equipment.item.find(item.id).is_some() { &carried.equipment.item } else { &carried.loot };
    let unusable = trp_sim::inventory::unusable_by_malfunction(root, item);
    let ground = if unusable { Color::srgba_u8(83, 0, 0, 63) } else { crate::health_view::item_ground(&item.kind.color) };
    let border = look.border.unwrap_or(Color::NONE);
    // Cut at its outer edge, not inside its border: the game's border is a
    // picture over the view that takes no room, and cut inside it, a
    // caption's last letter and its dark rim at the right were lost.
    // On the wish list (`WishlistGridView`), its marks; a slot's view's
    // border lies outside it, and is not cut away.
    let wish = carried.wishlist.get(&item.kind.template).zip(look.wish.as_ref());
    let outside = if wish.is_some() { 3.0 * s } else { 0.0 };
    let node = Node {
        border: UiRect::all(Val::Px(if look.border.is_some() { 1.0 } else { 0.0 })),
        overflow: Overflow::clip(),
        overflow_clip_margin: OverflowClipMargin::border_box().with_margin(outside),
        ..node
    };
    let units = [size.x / s, size.y / s];
    let panel = &look.panel;
    let mut view = parent.spawn((ItemView(item.id, layer), Ground(ground, border), RelativeCursorPosition::default(), node, BorderColor::all(border), BackgroundColor(ground)));
    if unusable {
        view.insert(Unusable);
    }
    view.with_children(|thing| {
        // Its glow, under its picture.
        if let Some((_, (index, glow))) = wish.and_then(|(group, wish)| Some((group, wish.glow.as_ref()?))) {
            let [x, y, w, h] = panel.placed(*index, units).rect;
            crate::health_view::draw_sprite(thing, glow, [x * s, y * s, w * s, h * s], s);
        }
        picture_in(thing, carried, item, size, turned, look.loader.as_ref().map(|(sprite, side)| (sprite, Vec2::from(*side) * s)));
        // Its tag (`GridItemView.UpdateTag`, `ResizeTag`): the plate at the
        // view's top left in the tag's colour, the name on it where the
        // caption leaves room.
        if let Some(tag) = look.tag.as_ref().filter(|_| !item.tag.name.trim().is_empty()) {
            let [x, y, _, h] = panel.placed(tag.plate, units).rect;
            let caption = format!(" {} ", carried.view_name(item));
            let (width, named) = tag.width(panel, look.caption, &caption, &item.tag.name, units[0]);
            thing.spawn((placed(x * s, y * s, width * s, h * s), BackgroundColor(crate::health_view::tag_colour(item.tag.color)))).with_children(|plate| {
                let Some(text) = panel.nodes[tag.name].text.as_ref().filter(|_| named) else { return };
                let color = Color::srgba(text.color[0], text.color[1], text.color[2], text.color[3]);
                let frame = Node { width: Val::Percent(100.0), height: Val::Percent(100.0), align_items: AlignItems::Center, ..default() };
                plate.spawn(frame).with_children(|frame| match &tag.name_tmp {
                    Some((asset, material)) => {
                        let run = trp_data::tmp_text::TmpRun { text: item.tag.name.clone(), size: text.size * s, color: crate::health_view::screen_numbers(color), bold: text.style & 1 != 0 };
                        let layout = trp_data::tmp_text::TmpLayout { em: text.size * s, spacing: text.spacing, kerning: text.kerning, wrap: None, align: text.horizontal & 7 };
                        crate::health_view::tmp_words(frame, Node::default(), crate::health_view::TmpText { asset: asset.clone(), material: material.clone(), runs: vec![run], layout }, &mut |_| {});
                    }
                    None => {
                        let font = TextFont { font: tag.name_font.clone(), font_size: text.size * s, ..default() };
                        crate::health_view::effect_words(frame, Node::default(), &[(item.tag.name.clone(), font, color)], None, text.size * s, &mut |_, _, _| {});
                    }
                });
            });
        }
        // The caption, as its text lies in its rectangle.
        if let Some((index, text)) = look.caption.and_then(|index| Some((index, panel.nodes[index].text.as_ref()?))) {
            let [x, y, w, h] = panel.placed(index, units).rect;
            let justify = match text.horizontal {
                1 => JustifyContent::FlexStart,
                2 => JustifyContent::Center,
                _ => JustifyContent::FlexEnd,
            };
            let align = match text.vertical {
                256 => AlignItems::FlexStart,
                1024 => AlignItems::FlexEnd,
                _ => AlignItems::Center,
            };
            let frame = Node { justify_content: justify, align_items: align, ..placed(x * s, y * s, w * s, h * s) };
            let font = TextFont { font: look.caption_font.clone(), font_size: text.size * s, weight: crate::health_view::weight_of(text.style), ..default() };
            let size = font.font_size;
            // What of it fits its box, as its text's overflow cuts it (the
            // caption's: truncated, so "FRONT PL" of a long name, the
            // user's capture).
            let said = match look.caption_face.as_ref() {
                Some(face) => crate::health_view::fitted_words(&face.font, text, &format!(" {} ", carried.view_name(item)), w),
                None => format!(" {} ", carried.view_name(item)),
            };
            let color = Color::srgb_u8(0xb6, 0xc1, 0xc7);
            thing.spawn(frame).with_children(|frame| match &look.caption_tmp {
                // As the game draws it.
                Some((asset, material)) => {
                    let run = trp_data::tmp_text::TmpRun { text: said, size, color: crate::health_view::screen_numbers(color), bold: text.style & 1 != 0 };
                    let layout = trp_data::tmp_text::TmpLayout { em: text.size * s, spacing: text.spacing, kerning: text.kerning, wrap: None, align: text.horizontal & 7 };
                    crate::health_view::tmp_words(frame, Node::default(), crate::health_view::TmpText { asset: asset.clone(), material: material.clone(), runs: vec![run], layout }, &mut |_| {});
                }
                None => crate::health_view::effect_words(frame, Node::default(), &[(said, font, color)], look.caption_effect, size, &mut |_, _, _| {}),
            });
        }
        let is = |class: &str| item.kind.class_names.iter().any(|name| name == class);
        // A slot's view of a weapon or a knife has its durability's bar at
        // its right (`ValidateDurability`), and of a weapon its mastering
        // (`ValidateMastering`).
        let slotted = look.mastering.is_some();
        let bar = slotted && (is("Weapon") || is("Knife")) && look.durability.is_some();
        // The weapon's round: in a slot under the caption, in a grid at the
        // bottom left (`SetInscription`, `Weapon.AmmoCaliber`).
        if let Some((index, text)) = look.inscription.filter(|_| is("Weapon") && !item.kind.caliber.is_empty()).and_then(|index| Some((index, panel.nodes[index].text.as_ref()?))) {
            let [x, y, w, h] = panel.placed(index, units).rect;
            let frame = match text.horizontal {
                1 => Node { position_type: PositionType::Absolute, left: Val::Px(3.0 * s), bottom: Val::Px(3.0 * s), ..default() },
                _ => Node { justify_content: JustifyContent::FlexEnd, align_items: AlignItems::Center, ..placed(x * s, y * s, w * s, h * s) },
            };
            let font = TextFont { font: look.inscription_font.clone(), font_size: text.size * s, ..default() };
            let size = font.font_size;
            let said = item.kind.caliber.replace("Caliber", "");
            let color = crate::health_view::rgba(text.color);
            thing.spawn(frame).with_children(|frame| match &look.inscription_tmp {
                Some((asset, material)) => {
                    let run = trp_data::tmp_text::TmpRun { text: said, size, color: crate::health_view::screen_numbers(color), bold: text.style & 1 != 0 };
                    let layout = trp_data::tmp_text::TmpLayout { em: text.size * s, spacing: text.spacing, kerning: text.kerning, wrap: None, align: text.horizontal & 7 };
                    crate::health_view::tmp_words(frame, Node::default(), crate::health_view::TmpText { asset: asset.clone(), material: material.clone(), runs: vec![run], layout }, &mut |_| {});
                }
                None => crate::health_view::effect_words(frame, Node::default(), &[(said, font, color)], look.inscription_effect, size, &mut |_, _, _| {}),
            });
        }
        // What it holds or has left, against the bottom row's right and
        // bottom (left of the durability's bar where it has one).
        let runs = item_value(carried, item, slotted);
        if let (false, Some((index, text))) = (runs.is_empty(), look.value.and_then(|index| Some((index, panel.nodes[index].text.as_ref()?)))) {
            let row = panel.nodes[index].parent.and_then(|group| panel.nodes[group].parent).unwrap_or(index);
            let [x, y, w, h] = panel.placed(row, units).rect;
            let slider = if bar { VALUE_BAR } else { 0.0 };
            let (right, bottom) = ((units[0] - x - w).max(0.0) + slider, (units[1] - y - h).max(0.0));
            let at = Node { position_type: PositionType::Absolute, right: Val::Px(right * s), bottom: Val::Px(bottom * s), ..default() };
            let base = crate::health_view::rgba(text.color);
            let font = |size: Option<f32>| TextFont { font: look.value_font.clone(), font_size: size.unwrap_or(text.size) * s, ..default() };
            // Drawn with its font's rim (Bender Outline: the dark edge round
            // a medkit's red figures), as wide as the largest run asks.
            let largest = runs.iter().map(|(_, _, size)| size.unwrap_or(text.size)).fold(text.size, f32::max) * s;
            if let Some((asset, material)) = &look.value_tmp {
                // As the game draws it: each run at its own size and colour.
                let runs = runs
                    .into_iter()
                    .map(|(said, color, size)| trp_data::tmp_text::TmpRun { text: said, size: size.unwrap_or(text.size) * s, color: crate::health_view::screen_numbers(color.unwrap_or(base)), bold: text.style & 1 != 0 })
                    .collect();
                let layout = trp_data::tmp_text::TmpLayout { em: text.size * s, spacing: text.spacing, kerning: text.kerning, wrap: None, align: text.horizontal & 7 };
                crate::health_view::tmp_words(thing, at, crate::health_view::TmpText { asset: asset.clone(), material: material.clone(), runs, layout }, &mut |_| {});
            } else {
                let words: Vec<(String, TextFont, Color)> = runs.into_iter().map(|(said, color, size)| (said, font(size), color.unwrap_or(base))).collect();
                crate::health_view::effect_words(thing, at, &words, look.value_effect, largest, &mut |_, _, _| {});
            }
        }
        // The durability's bar: its track and its fill, full for a weapon
        // as made (`DurabilitySlider.Init`: the template's durability).
        if let (true, Some((track, fill))) = (bar, look.durability.as_ref()) {
            let [w, h] = units;
            crate::health_view::draw_sprite(thing, track, [(w - 15.0) * s, 26.0 * s, 12.0 * s, (h - 29.0) * s], s);
            let [now, most] = item.durability();
            let share = if most > 0.0 { (now / most).clamp(0.0, 1.0) } else { 1.0 };
            let (top, bottom) = (32.5, h - 9.0);
            let high = (bottom - top) * share;
            crate::health_view::draw_sprite(thing, fill, [(w - 10.0) * s, (bottom - high) * s, 2.0 * s, high * s], s);
        }
        // The weapon's mastering: its mark and its level, the game's level
        // and one, written with a nought before it (`SlotItemView.
        // UpdateInfo`); where the user's own capture has them at 1080
        // lines (the views' layout groups are not solved here).
        if let (true, Some((mark, size, color, font))) = (is("Weapon"), look.mastering.as_ref()) {
            let [_, h] = units;
            let high = MASTERING_MARK_HEIGHT;
            let wide = high * mark.size[0] / mark.size[1].max(1.0);
            crate::health_view::draw_sprite(thing, mark, [4.0 * s, (h - 8.0 - high) * s, wide * s, high * s], s);
            let at = Node { position_type: PositionType::Absolute, left: Val::Px(22.0 * s), bottom: Val::Px(3.0 * s), ..default() };
            let level = format!("0{}", carried.mastering + 1);
            thing.spawn(at).with_children(|at| {
                at.spawn((Text::new(level), TextFont { font: font.clone(), font_size: size * s, ..default() }, TextColor(*color), TextLayout::new_with_no_wrap()));
            });
        }
        // A part's marks at the bottom left: its class's ground and its
        // kind's mark over it (`ItemViewStats.SetItemIcon`).
        let class = crate::health_view::MOD_CLASSES.iter().find(|(class, _)| is(class)).map(|(_, name)| *name);
        let kind = crate::health_view::MOD_KINDS.iter().find(|(class, _)| is(class)).map(|(_, name)| *name);
        // A plate that may be taken out shows its armour class's mark in
        // their place (`ItemViewStats.SetStaticInfo`, `IsMoveAblePlate`: one
        // not in a slot, or of a class in a slot not locked); a built-in
        // insert, in its locked slot, the part's marks. `PROVISIONAL`: the
        // mark at the marks' place, its own object (`_armorClassIcon`) not
        // read.
        let movable_plate = is("ArmorPlate")
            && match carried.lies(item.id) {
                Some((holder, Spot::Slot(at))) => item.kind.armor_class > 0 && !carried.find(holder).is_some_and(|holder| holder.kind.slots.get(at).is_some_and(|slot| slot.locked)),
                _ => true,
            };
        if movable_plate {
            if let Some(mark) = look.mods.get(&format!("plate_{}", item.kind.armor_class)) {
                let ([_, h], [mw, mh]) = (units, mark.size);
                crate::health_view::draw_sprite(thing, mark, [3.0 * s, (h - 3.0 - mh) * s, mw * s, mh * s], s);
            }
        } else if let (Some(class), Some(kind)) = (class, kind) {
            let [_, h] = units;
            if let Some(ground) = look.mods.get(&format!("type_{class}")) {
                let [gw, gh] = ground.size;
                crate::health_view::draw_sprite(thing, ground, [3.0 * s, (h - 3.0 - gh) * s, gw * s, gh * s], s);
                if let Some(mark) = look.mods.get(&format!("mod_{kind}")) {
                    let [mw, mh] = mark.size;
                    crate::health_view::draw_sprite(thing, mark, [(3.0 + (gw - mw) / 2.0) * s, (h - 3.0 - (gh + mh) / 2.0) * s, mw * s, mh * s], s);
                }
            }
        }
        // Its border's edges over it, and its group's outlined icon at the
        // top of the bottom left column (above a part's marks and the
        // weapon's round, where those are in it).
        if let Some((group, wish)) = wish {
            for (index, edge) in &wish.edges {
                let [x, y, w, h] = panel.placed(*index, units).rect;
                crate::health_view::draw_sprite(thing, edge, [x * s, y * s, w * s, h * s], s);
            }
            if let Some(icon) = wish.icons.get(group.name()) {
                let [cx, cy, _, ch] = panel.placed(wish.column, units).rect;
                let marks = class.is_some() && kind.is_some();
                let round = is("Weapon") && !item.kind.caliber.is_empty();
                let below = if marks { wish.mod_height } else { 0.0 } + if round { wish.inscription_height.unwrap_or(0.0) } else { 0.0 };
                let [w, h] = [panel.asks(wish.icon, 0).1, panel.asks(wish.icon, 1).1];
                crate::health_view::draw_sprite(thing, icon, [cx * s, (cy + ch - below - h) * s, w * s, h * s], s);
            }
        }
        // A magazine worked: the game's load component over its view
        // (`ItemViewAnimation.StartLoading`), turned each frame
        // (`turn_spinners`).
        if let (Some(spin), true) = (look.spin.as_ref(), carried.magazine_work.as_ref().is_some_and(|work| work.process.magazine == item.id)) {
            let pieces = [(SpinRole::Back, &spin.back), (SpinRole::Fill, &spin.fill), (SpinRole::Loader, &spin.loader), (SpinRole::Stop, &spin.stop)];
            for (role, piece) in pieces {
                let Some((index, picture)) = piece else { continue };
                let [x, y, w, h] = spin.rect(*index, picture, units);
                let node = placed(x * s, y * s, w * s, h * s);
                match role {
                    SpinRole::Fill => {
                        thing.spawn((node, SpinPiece(role), SpinFill(picture.image.clone(), picture.cut)));
                    }
                    _ => {
                        let shown = if role == SpinRole::Stop { Visibility::Hidden } else { Visibility::Inherited };
                        thing.spawn((node, SpinPiece(role), ImageNode { image: picture.image.clone(), rect: Some(picture.cut), flip_y: true, ..default() }, shown));
                    }
                }
            }
        }
        // The key it is bound to, or of the slot it is in where that slot
        // has a key of its own.
        let worn_in = carried.equipment.item.slots.iter().zip(&carried.equipment.item.kind.slots).find(|(part, _)| part.as_ref().is_some_and(|part| part.id == item.id)).map(|(_, slot)| slot.name.as_str());
        let slot_key = worn_in.and_then(|slot| carried.slot_keys.iter().find(|(name, _)| *name == slot)).map(|(_, words)| words.as_str()).filter(|words| !words.is_empty());
        let key = carried.bound.key_of(item.id).map(|key| QUICK_KEYS[key].1).or(slot_key);
        if let (Some(key), Some(bind)) = (key, look.bind.as_ref()) {
            let [w, h] = bind.size;
            let label = ImageNode { image: bind.image.clone(), rect: Some(bind.cut), flip_y: true, ..default() };
            let frame = Node { justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..placed(0.0, 0.0, w * s, h * s) };
            let font = TextFont { font: bind.font.clone(), font_size: bind.text_size * s, ..default() };
            thing.spawn((frame, label)).with_children(|label| {
                label.spawn((Text::new(key), font, TextColor(bind.color), TextLayout::new_with_no_wrap()));
            });
        }
    });
}

/// A game's view of a thing (`TARKOV UI`): its own ground and border,
/// which it goes back to when it is no longer lit.
#[derive(Component, Clone, Copy)]
struct Ground(Color, Color);

/// A piece of the load component over a worked magazine's view.
#[derive(Component, Clone, Copy, PartialEq)]
struct SpinPiece(SpinRole);

#[derive(Clone, Copy, PartialEq, Debug)]
enum SpinRole {
    Back,
    Fill,
    Loader,
    Stop,
}

/// The spinner's fill, its picture and cut, until its material is made.
#[derive(Component)]
struct SpinFill(Handle<Image>, Rect);

/// The app's bar of a worked magazine's share done.
#[derive(Component)]
struct AppSpinBar;

/// The load component turned each frame (inventory spec, "The load
/// component"): the spinner filled by the work's share done, the loader
/// turned by the present round's (shown where more than one round is to
/// go), both red and the stop cross on with the mouse on the magazine's
/// view (`SetStopButtonStatus`); the app's bar filled the same.
#[allow(clippy::too_many_arguments)]
fn turn_spinners(
    mut commands: Commands,
    carried: Res<Carried>,
    views: Query<(&ItemView, &RelativeCursorPosition)>,
    mut materials: ResMut<Assets<crate::radial_fill::RadialFill>>,
    fresh: Query<(Entity, &SpinFill), Without<MaterialNode<crate::radial_fill::RadialFill>>>,
    mut pieces: Query<(&SpinPiece, Option<&mut UiTransform>, Option<&mut ImageNode>, &mut Visibility, Option<&MaterialNode<crate::radial_fill::RadialFill>>)>,
    mut bars: Query<&mut Node, With<AppSpinBar>>,
    mut was_hovered: Local<bool>,
) {
    // (The page is drawn afresh as each round moves: a view just made does
    // not yet know where the mouse is, and keeps the one before's; its
    // fill's material is made at the work's share and colour, so that
    // nothing flickers between the two.)
    let work = carried.magazine_work.as_ref();
    let (done, round) = work.map_or((0.0, 0.0), |work| work.shown.shares());
    let hovered = work.is_some_and(|work| views.iter().filter(|(view, _)| view.0 == work.process.magazine).any(|(_, at)| if at.normalized.is_none() { *was_hovered } else { at.cursor_over() }));
    *was_hovered = hovered;
    let color = if hovered { Color::srgb(1.0, 0.0, 0.0) } else { Color::WHITE };
    for (entity, fill) in &fresh {
        let mut made = crate::radial_fill::RadialFill::new(fill.0.clone(), fill.1, color, true);
        made.fill.x = done;
        let material = materials.add(made);
        commands.entity(entity).insert((MaterialNode(material), UiTransform::IDENTITY));
    }
    let Some(work) = work else { return };
    for (piece, turn, image, mut visibility, material) in &mut pieces {
        let wanted = match piece.0 {
            SpinRole::Stop => hovered,
            SpinRole::Loader => work.shown.to_go > 1,
            _ => true,
        };
        let wanted = if wanted { Visibility::Inherited } else { Visibility::Hidden };
        if *visibility != wanted {
            *visibility = wanted;
        }
        if let (Some(mut image), true) = (image, matches!(piece.0, SpinRole::Back | SpinRole::Loader)) {
            if image.color != color {
                image.color = color;
            }
        }
        // A whole turn clockwise each round (the engine's -360 degrees about
        // its forward axis; Bevy's clockwise is the other sign).
        if let (SpinRole::Loader, Some(mut turn)) = (piece.0, turn) {
            turn.rotation = Rot2::degrees(360.0 * round);
        }
        if let Some(fill) = material.and_then(|node| materials.get_mut(&node.0)) {
            fill.fill.x = done;
            fill.set_color(color);
        }
    }
    for mut bar in &mut bars {
        bar.width = Val::Percent(done * 100.0);
    }
}

/// An item's picture, whole in the middle of a box of that size: the
/// thing's own where it has been made; while it is still to come a
/// spinner, as the game's view shows its loader until its picture is read
/// or drawn (`ItemView._iconLoader`; icons spec, "When a picture is asked
/// for"): the game's own (`loader`, its picture and its size in pixels)
/// where the view is the game's, else the app's; the plain shape of its
/// sort where none is made at all.
fn picture_in(thing: &mut ChildSpawnerCommands, carried: &Carried, item: &Item, size: Vec2, turned: bool, loader: Option<(&crate::health_view::Sprite, Vec2)>) {
    let key = shown_key(carried, item);
    if let Some(picture) = carried.pictures.get(&key) {
        thing.spawn(picture_node(picture, size, turned));
        return;
    }
    let [across, down] = item.cells();
    let coming = carried.pictures_on && across > 0 && down > 0 && !carried.picture_failed.contains(&key);
    if coming {
        thing.spawn(spinner_node(carried, Loading { key, sort: icon_of(item), size, turned }, loader));
    } else if let Some(picture) = carried.icons.get(icon_of(item)) {
        thing.spawn(picture_node(picture, size, turned));
    }
}

/// A picture placed whole in the middle of a box of that size.
fn picture_node(picture: &Picture, size: Vec2, turned: bool) -> impl Bundle {
    // The box's inside, within its border.
    let room = (size - Vec2::splat(2.0)).max(Vec2::ONE);
    let shape = if turned { 1.0 / picture.shape } else { picture.shape };
    let share = if picture.fills { 1.0 } else { PICTURE_SHARE };
    let height = (room.y * share).min(room.x * share / shape);
    let width = height * shape;
    let over = placed((room.x - width) / 2.0, (room.y - height) / 2.0, width, height);
    let image = if turned { &picture.turned } else { &picture.upright };
    (ImageNode::new(image.clone()), over, ItemPicture)
}

/// An item's own picture in its view (`GridItemView.MainImage`).
#[derive(Component)]
struct ItemPicture;

/// An item's view drawn as unusable by a malfunction (`game_item_view`).
#[derive(Component)]
struct Unusable;

/// The picture of an unusable item's view, red (`MainImage.color = Color.red`),
/// as it comes (made with the view, or put in for its spinner).
fn tint_unusable(mut pictures: Query<(&mut ImageNode, &ChildOf), Added<ItemPicture>>, unusable: Query<(), With<Unusable>>) {
    for (mut image, parent) in &mut pictures {
        if unusable.contains(parent.parent()) {
            let alpha = image.color.alpha();
            image.color = Color::srgba(1.0, 0.0, 0.0, alpha);
        }
    }
}

/// How large a spinner is, as a share of the box's shorter side, and at
/// most and least in pixels; and how many turns it makes a second. The
/// app's.
const SPINNER_SHARE: f32 = 0.42;
const SPINNER_SIDE: [f32; 2] = [10.0, 28.0];
const SPINNER_TURNS: f32 = 1.1;
/// The app's spinner picture's side, in pixels.
const SPINNER_PIXELS: usize = 64;

/// A spinner's picture: its texture, its cut as `RadialFill` takes it
/// (left, lowest, right, highest row) and its colour. Every spinner of one
/// picture is drawn by one material, turned each frame (`fill_pictures`).
#[derive(Component, Clone, PartialEq)]
struct SpinnerPicture(Handle<Image>, Rect, Color);

/// A spinner in the middle of a box, turned each frame (`fill_pictures`):
/// the game's loader at its own size where it is had (a view of the
/// game's), else the app's.
fn spinner_node(carried: &Carried, loading: Loading, loader: Option<(&crate::health_view::Sprite, Vec2)>) -> impl Bundle {
    let room = (loading.size - Vec2::splat(2.0)).max(Vec2::ONE);
    if let Some((sprite, side)) = loader {
        let over = placed((room.x - side.x) / 2.0, (room.y - side.y) / 2.0, side.x, side.y);
        return (SpinnerPicture(sprite.image.clone(), sprite.cut, sprite.color), over, loading);
    }
    let side = (room.min_element() * SPINNER_SHARE).clamp(SPINNER_SIDE[0], SPINNER_SIDE[1]).min(room.min_element());
    let over = placed((room.x - side) / 2.0, (room.y - side) / 2.0, side, side);
    // The app's picture has its first row at the top, where the game's
    // textures have theirs lowest: its cut taken upside down.
    let pixels = SPINNER_PIXELS as f32;
    let cut = Rect { min: Vec2::new(0.0, pixels), max: Vec2::new(pixels, 0.0) };
    (SpinnerPicture(carried.spinner.clone(), cut, Color::WHITE), over, loading)
}

/// Turns the spinners, and puts in each one's place the picture that has
/// come for it, or the plain shape of its sort where none can be made:
/// the box and everything else on the screen stay as they are.
#[allow(clippy::too_many_arguments)]
fn fill_pictures(
    mut commands: Commands,
    time: Res<Time<Real>>,
    carried: Res<Carried>,
    features: Res<crate::options_view::Features>,
    mut arrived: ResMut<PicturesArrived>,
    mut materials: ResMut<Assets<crate::radial_fill::RadialFill>>,
    mut shared: Local<Vec<(SpinnerPicture, Handle<crate::radial_fill::RadialFill>)>>,
    fresh: Query<(Entity, &SpinnerPicture), Without<MaterialNode<crate::radial_fill::RadialFill>>>,
    spinners: Query<(Entity, &Loading, &ChildOf)>,
) {
    // A spinner is turned inside its upright node, by its picture's
    // material, never by turning the node: Bevy cuts a turned node at a
    // clip's edge as if it were not turned, pulling it out of shape (a box
    // half out of a scrolled panel, the user's capture, 2026-10-23), and a
    // box cuts what is in it, so a loader whose corners reached past its
    // box's edges could not turn at all (a medical case's small things,
    // 0.104.2). Upright, it is cut as any picture is.
    for (entity, picture) in &fresh {
        let handle = match shared.iter().find(|(had, _)| had == picture) {
            Some((_, handle)) => handle.clone(),
            None => {
                let handle = materials.add(crate::radial_fill::RadialFill::new(picture.0.clone(), picture.1, picture.2, features.tarkov_ui));
                shared.push((picture.clone(), handle.clone()));
                handle
            }
        };
        commands.entity(entity).insert(MaterialNode(handle));
    }
    if !spinners.is_empty() {
        let turn = (time.elapsed_secs() * SPINNER_TURNS).fract();
        for (_, handle) in shared.iter() {
            if let Some(material) = materials.get_mut(handle) {
                material.set_turn(turn);
            }
        }
    }
    let come = std::mem::take(&mut arrived.0);
    for (entity, loading, parent) in &spinners {
        if !come.contains(&loading.key) {
            continue;
        }
        let picture = carried.pictures.get(&loading.key).or_else(|| carried.icons.get(loading.sort));
        if let Some(picture) = picture {
            // First among the box's children, so that its words are drawn
            // over it as before.
            let shown = commands.spawn(picture_node(picture, loading.size, loading.turned)).id();
            commands.entity(parent.parent()).insert_children(0, &[shown]);
        }
        commands.entity(entity).despawn();
    }
}

/// How a blinking box's picture shows, `since` seconds after it began, as
/// the game's (`ItemViewAnimation.CG_StartBlinking`): faded out to nothing
/// over half a second and back over the next, round and round, each half a
/// tween with the tweening library's default easing (out, quadratic).
fn blink_alpha(since: f32) -> f32 {
    let out_quad = |x: f32| 1.0 - (1.0 - x) * (1.0 - x);
    let into = since.rem_euclid(1.0);
    if into < 0.5 { 1.0 - out_quad(into / 0.5) } else { out_quad((into - 0.5) / 0.5) }
}

/// Fades the pictures of the boxes that blink (`Blinking`), each from when
/// it began; a box that has stopped is whole again. Only a picture whose
/// fading changes is written.
/// A magazine being loaded or unloaded blinks too (`ItemViewAnimation.
/// StartLoading`), and takes its clicks.
/// The worked magazine's ground blinks with it (`StartBackgroundBlinking`):
/// to an alpha of 0.6 and back, half a second each way (the tween's
/// default easing, out and quadratic), whatever lights it meanwhile; the
/// page drawn afresh as the work stops gives it its colour again.
#[allow(clippy::type_complexity)]
fn blink(
    time: Res<Time<Real>>,
    blinking: Res<Blinking>,
    carried: Res<Carried>,
    views: Query<&ItemView>,
    mut pictures: Query<(&mut ImageNode, &ChildOf), (Without<Loading>, Without<SpinPiece>)>,
    mut grounds: Query<(&ItemView, &Ground, &mut BackgroundColor)>,
    mut began: Local<HashMap<Id, f64>>,
    mut blinked: Local<bool>,
) {
    let now = time.elapsed_secs_f64();
    let worked = carried.magazine_work.as_ref().map(|work| work.process.magazine);
    let blinks = || blinking.0.iter().copied().chain(worked);
    // With nothing blinking now or on the frame before (when a picture
    // that stopped was made whole again), nothing is looked at.
    let any = blinks().next().is_some();
    if !any && !*blinked {
        return;
    }
    *blinked = any;
    began.retain(|id, _| blinks().any(|blinking| blinking == *id));
    for id in blinks() {
        began.entry(id).or_insert(now);
    }
    for (mut image, parent) in &mut pictures {
        let Ok(view) = views.get(parent.parent()) else { continue };
        let alpha = began.get(&view.0).map_or(1.0, |at| blink_alpha((now - at) as f32));
        if image.color.alpha() != alpha {
            image.color.set_alpha(alpha);
        }
    }
    let Some(at) = worked.and_then(|id| began.get(&id)) else { return };
    let since = (now - at) as f32;
    for (view, ground, mut back) in &mut grounds {
        if Some(view.0) != worked {
            continue;
        }
        let base = ground.0.alpha();
        let lit = 1.0 - blink_alpha(since);
        let alpha = base + (0.6 - base) * lit;
        if back.0 != ground.0.with_alpha(alpha) {
            back.0 = ground.0.with_alpha(alpha);
        }
    }
}

/// A slot as drawn: whatever is in it, filling it. `size` is the slot's,
/// in pixels.
#[allow(clippy::too_many_arguments)]
fn slot_box(parent: &mut ChildSpawnerCommands, carried: &Carried, holder: &Item, slot: usize, node: Node, size: Vec2, layer: u8, s: f32) {
    slot_box_drawn(parent, carried, holder, slot, node, size, layer, true, true, s);
}

/// A slot's box, with its own border and ground (`drawn`) or with none,
/// where the game's own slot view is drawn under it (`TARKOV UI`); what is
/// in it in a slot's view (`wide`: the equipment's, `SlotItemView`) or a
/// grid's (a part in the inspect window's slot: `ModSlotView` makes a
/// `GridItemView`, `ItemUiContext.CreateItemView`).
#[allow(clippy::too_many_arguments)]
fn slot_box_drawn(parent: &mut ChildSpawnerCommands, carried: &Carried, holder: &Item, slot: usize, node: Node, size: Vec2, layer: u8, drawn: bool, wide: bool, s: f32) {
    let node = Node { border: UiRect::all(Val::Px(1.0)), ..node };
    let (border, ground) = if drawn { (LINE, SLOT) } else { (Color::NONE, Color::NONE) };
    parent.spawn((SlotBox { holder: holder.id, slot, layer, border }, RelativeCursorPosition::default(), node, BorderColor::all(border), BackgroundColor(ground))).with_children(|view| {
        if let Some(item) = holder.slots.get(slot).and_then(Option::as_ref) {
            let inside = (size - Vec2::splat(2.0)).max(Vec2::ONE);
            item_view(view, carried, item, Node { width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() }, wide, inside, false, layer, s);
        }
    });
}

/// A grid as drawn: its cells, and each thing in it where it lies.
fn grid_box(parent: &mut ChildSpawnerCommands, carried: &Carried, holder: &Item, grid: usize, cell: f32, layer: u8) {
    grid_box_drawn(parent, carried, holder, grid, cell, layer, true);
}

/// A grid's box, with its own ground and cell lines (`drawn`) or with
/// none, where the game's own grid view is drawn under it (`TARKOV UI`).
fn grid_box_drawn(parent: &mut ChildSpawnerCommands, carried: &Carried, holder: &Item, grid: usize, cell: f32, layer: u8, drawn: bool) {
    let cells = holder.kind.grids[grid].cells;
    let (width, height) = (cells[0] as f32 * cell, cells[1] as f32 * cell);
    let node = Node { width: Val::Px(width), height: Val::Px(height), flex_shrink: 0.0, ..default() };
    let outline = Outline::new(Val::Px(if drawn { 1.0 } else { 0.0 }), Val::ZERO, LINE);
    let ground = if drawn { BACK } else { Color::NONE };
    parent.spawn((GridBox { holder: holder.id, grid, cells, layer }, RelativeCursorPosition::default(), node, outline, BackgroundColor(ground))).with_children(|inside| {
        for column in (1..cells[0]).filter(|_| drawn) {
            inside.spawn((placed(column as f32 * cell, 0.0, 1.0, height), BackgroundColor(PANEL)));
        }
        for row in (1..cells[1]).filter(|_| drawn) {
            inside.spawn((placed(0.0, row as f32 * cell, width, 1.0), BackgroundColor(PANEL)));
        }
        for (place, item) in &holder.grids[grid] {
            let [w, h] = item.cells();
            let (w, h) = if place.turned { (h, w) } else { (w, h) };
            let size = Vec2::new(w as f32 * cell - 2.0, h as f32 * cell - 2.0);
            let node = placed(place.x as f32 * cell + 1.0, place.y as f32 * cell + 1.0, size.x, size.y);
            item_view(inside, carried, item, node, false, size, place.turned, layer, cell / CELL);
        }
    });
}

// ==== The quick bar, the windows stretched, the inspect window's parts ====

/// What the mouse and the keys do in the screen, in the app's own words.
const HELP: &str = "Hold a thing with the left button and let it go on a cell, a slot or another thing; R turns it.\nCtrl and a click sends it to the other side. Alt and a click puts it on. A double click uses medicine, food or drink, opens a container, or shows what a thing is. The right button on a thing opens its menu.\nCtrl held as a stack is let go asks how many.\nA quick-use key let go over a thing in the vest or the pockets binds it.\nThe discard key puts the thing under the mouse down.";

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
/// What a quick-use view is for, by the game's name of its binding
/// (`EBoundItem`, or the view's own key): a slot with a key of its own (the
/// sheath, the holster, the sling, the back) with what is in it, or one of
/// the seven keys with what is bound to it.
pub(crate) fn quick_thing<'a>(carried: &'a Carried, key: &str) -> (&'static str, Option<&'a Item>) {
    let worn = &carried.equipment.item;
    match key {
        "Grenade" | "G" => ("ThrowGrenade", carried.top_grenade.and_then(|id| worn.find(id))),
        "ItemV" | "V" => ("Knife", worn.in_slot("Scabbard")),
        "Item1" | "1" => ("SecondaryWeapon", worn.in_slot("Holster")),
        "Item2" | "2" => ("PrimaryWeaponFirst", worn.in_slot("FirstPrimaryWeapon")),
        "Item3" | "3" => ("PrimaryWeaponSecond", worn.in_slot("SecondPrimaryWeapon")),
        other => match other.trim_start_matches("Item").parse::<usize>().ok().and_then(|number| number.checked_sub(4)).filter(|at| *at < QUICK_KEYS.len()) {
            Some(at) => (QUICK_KEYS[at].0, carried.bound.items[at].and_then(|id| worn.find(id))),
            None => ("", None),
        },
    }
}

/// The words of a binding's keys, or the app's where the install has none.
pub(crate) fn quick_key_words(bindings: &Bindings, binding: &str, own: &str) -> String {
    match bindings.caption(binding) {
        "" => QUICK_KEYS.iter().find(|(name, _)| *name == binding).map_or(own, |(_, word)| word).to_owned(),
        word => word.to_owned(),
    }
}

fn quick_bar(commands: &mut Commands, carried: &Carried, bindings: &Bindings, units: Units, window: Vec2, look: &crate::health_view::HealthLook, said: &crate::health_view::Words) {
    let s = units.0;
    let thing = |key: &str| quick_thing(carried, key);
    let key_words = |binding: &str, own: &str| quick_key_words(bindings, binding, own);
    // With `TARKOV UI`, the game's own panel where the game's screen has it
    // (`health_view::draw_quick`), each view's thing in its place over its
    // ground (`QuickSlotItemView`: its kind's colour at 0.302, its picture
    // turned and fitted), the views' labels and borders over them.
    if let (true, Some(quick)) = (look.tarkov_ui, look.quick.as_ref()) {
        let screen = [window.x / s.max(1e-3), SCREEN_HEIGHT];
        let [x, y, width, height] = quick.quick_in(screen);
        let words: HashMap<String, (String, String)> = quick
            .slots
            .iter()
            .map(|slot| {
                let (binding, item) = thing(&slot.key);
                let own = slot.key.trim_start_matches("Item").replace("10", "0");
                (slot.key.clone(), (key_words(binding, &own), item.map_or(String::new(), |item| carried.name(item))))
            })
            .collect();
        let size = [width, height];
        commands.spawn((Overlay, Cover(BAR_LAYER), RelativeCursorPosition::default(), placed(x * s, y * s, width * s, height * s), GlobalZIndex(12))).with_children(|bar| {
            crate::health_view::draw_quick(bar, look, said, size, &words, 0, s);
            for (key, [ix, iy, iw, ih]) in crate::health_view::quick_places(look, size) {
                let (_, Some(item)) = thing(&key) else { continue };
                let place = Node { overflow: Overflow::clip(), ..placed(ix * s, iy * s, iw * s, ih * s) };
                let ground = crate::health_view::item_ground(&item.kind.color);
                bar.spawn((place, BackgroundColor(ground))).with_children(|place| quick_picture(place, carried, item, [iw, ih], s));
            }
            crate::health_view::draw_quick(bar, look, said, size, &words, 1, s);
        });
        return;
    }
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
    commands.spawn((Overlay, Cover(BAR_LAYER), RelativeCursorPosition::default(), node, BackgroundColor(PANEL.with_alpha(0.96)), GlobalZIndex(12))).with_children(|bar| {
        bar.spawn((placed(8.0 * s, 4.0 * s, 300.0 * s, 18.0 * s), words("QUICK USE", 10.0, FAINT)));
        for (key, middle, side) in views {
            // What the view is for: a slot with a key of its own, or one
            // of the seven keys.
            let (binding, item) = thing(key);
            if binding.is_empty() {
                continue;
            }
            let caption = key_words(binding, key);
            let size = Vec2::splat(side * s);
            let view = placed((middle - side / 2.0 + margin) * s, (height - side - 10.0) * s, size.x, size.y);
            bar.spawn((Node { border: UiRect::all(Val::Px(1.0)), overflow: Overflow::clip(), ..view }, BorderColor::all(LINE), BackgroundColor(SLOT))).with_children(|view| {
                if let Some(item) = item {
                    picture_in(view, carried, item, size, false, None);
                    view.spawn((placed(2.0, size.y - 16.0, size.x - 4.0, 14.0), words(carried.name(item), 9.0, WORDS)));
                }
                view.spawn((placed(3.0, 1.0, size.x - 6.0, 14.0), words(caption, 10.0, FAINT)));
            });
        }
    });
}

/// A window that shows a thing stretched by its edges and corners
/// (`StretchWindow`, `StretchArea.OnDrag`): a press on one of its
/// stretching places, on top where the mouse is, takes it; the mouse's
/// travel since the last place the stretch took (`_lastPosition`), in
/// units, along the sides it stretches (out for the right and the bottom,
/// in for the left and the top), added to its preferred size, held within
/// nought and its most (`_maxSize`); laid out again, the window as large
/// as that or as what it holds asks, whichever is more, its preferred size
/// then what it came to, and the mouse's last place moved on by as much
/// as it did; the window moved so that its other side stays
/// (`anchoredPosition` by the change at its pivot, the middle), and back
/// onto the screen (`CorrectPositionResolution`).
#[allow(clippy::too_many_arguments)]
fn stretch_windows(
    buttons: Res<ButtonInput<MouseButton>>,
    window: Query<&Window, With<PrimaryWindow>>,
    controller: Res<Controller>,
    mut carried: ResMut<Carried>,
    mut clicks: ResMut<Clicks>,
    areas: Query<(&WinStretch, &RelativeCursorPosition)>,
    look: Res<crate::health_view::HealthLook>,
    (game, held, said, units): (Res<crate::GameData>, Res<HandsHeld>, Res<crate::health_view::Words>, Res<Units>),
) {
    if !controller.inventory_open || !buttons.pressed(MouseButton::Left) {
        if clicks.stretching.is_some() {
            clicks.stretching = None;
        }
        if !controller.inventory_open {
            return;
        }
    }
    let Ok(screen) = window.single() else { return };
    let room = Vec2::new(screen.width(), screen.height());
    let s = units.0.max(1e-3);
    // A window that shows a thing, or the game's window of a container's
    // grids, placed by its own size as it is first laid out (`OpenWindow`:
    // alone, in the middle of the screen; else 40 right of and below the
    // one opened last, then `CorrectPositionResolution`): it opened where
    // one of its size as made would be, and what it holds sizes it.
    let unfitted: Vec<usize> = carried.windows.iter().enumerate().filter(|(_, open)| !open.fitted && (matches!(open.kind, WinKind::Info | WinKind::Repair) || (open.kind == WinKind::Grids && look.grid_window.is_some()))).map(|(index, _)| index).collect();
    for index in unfitted {
        let open = carried.windows[index];
        let size = carried.find(open.item).and_then(|item| match open.kind {
            WinKind::Grids => grid_built(&look, &carried, item, false).map(|built| Vec2::from(built.size) * s),
            WinKind::Repair => crate::health_view::repair_built(&look, &crate::health_view::RepairWords::default()).map(|built| Vec2::from(built.size) * s).or(Some(Vec2::new(420.0, 260.0))),
            _ => {
                let figures = trp_sim::item_attributes::figures(item, &game.db, &|key| carried.texts.say(key));
                let (parts, _) = info_parts(&carried, item, &held, controller.health.as_ref(), &game, &said, open.stretched);
                crate::health_view::info_built(&look, &crate::health_view::InfoWords::default(), Some(&figures), &parts, false).map(|built| Vec2::from(built.size) * s)
            }
        });
        let alone = carried.windows.len() == 1;
        let fitted = carried.windows.get_mut(index).expect("an open window");
        if let Some(size) = size {
            fitted.at = if alone { ((room - size) / 2.0).min(room - size).max(Vec2::ZERO) } else { on_screen(fitted.at, size, room) };
        }
        fitted.fitted = true;
    }
    let Some(cursor) = screen.cursor_position() else { return };
    if buttons.just_pressed(MouseButton::Left) && clicks.stretching.is_none() && carried.asking.is_none() {
        if let Some((index, sides)) = areas.iter().filter(|(_, at)| at.cursor_over()).map(|(area, _)| (area.0, area.1)).max_by_key(|(index, _)| *index) {
            clicks.stretching = Some((index, sides, cursor));
        }
    }
    let Some((index, sides, last)) = clicks.stretching else { return };
    let travel = cursor - last;
    if travel == Vec2::ZERO {
        return;
    }
    let Some(win) = carried.windows.get(index).copied() else {
        clicks.stretching = None;
        return;
    };
    let Some(item) = carried.find(win.item) else { return };
    let figures = trp_sim::item_attributes::figures(item, &game.db, &|key| carried.texts.say(key));
    let size_at = |stretched: Option<[f32; 2]>| {
        let (parts, _) = info_parts(&carried, item, &held, controller.health.as_ref(), &game, &said, stretched);
        crate::health_view::info_built(&look, &crate::health_view::InfoWords::default(), Some(&figures), &parts, false).map(|built| built.size)
    };
    let Some(before) = size_at(win.stretched) else { return };
    let preferred = win.stretched.or_else(|| crate::health_view::info_preferred(&look)).unwrap_or(before);
    let most = look.info_stretch.unwrap_or([f32::MAX, f32::MAX]);
    let (horizontal, vertical) = (sides & 12 != 0, sides & 3 != 0);
    let sense = [if sides & 8 != 0 { 1.0 } else { -1.0 }, if sides & 2 != 0 { 1.0 } else { -1.0 }];
    let asked = [
        if horizontal { (preferred[0] + travel.x / s * sense[0]).clamp(0.0, most[0]) } else { preferred[0] },
        if vertical { (preferred[1] + travel.y / s * sense[1]).clamp(0.0, most[1]) } else { preferred[1] },
    ];
    let Some(after) = size_at(Some(asked)) else { return };
    let change = [after[0] - before[0], after[1] - before[1]];
    let mut at = win.at;
    if sides & 4 != 0 {
        at.x -= change[0] * s;
    }
    if sides & 1 != 0 {
        at.y -= change[1] * s;
    }
    let size = Vec2::new(after[0], after[1]) * s;
    at = at.min(room - size).max(Vec2::ZERO);
    clicks.stretching = Some((index, sides, last + Vec2::new(change[0] * sense[0], change[1] * sense[1]) * s));
    if let Some(open) = carried.windows.get_mut(index) {
        (open.stretched, open.at) = (Some(after), at);
    }
}

/// What can never be under the mouse's reach (a locked slot's part): a
/// layer no cover is.
const OUT_OF_REACH: u8 = 254;

/// A button of the window that shows a thing: the thing, its line, and
/// the window's layer among the covers.
#[derive(Component)]
pub(crate) struct InfoButtonHit(Id, trp_sim::item_menu::Line, u8);

/// A stretching place of a window that shows a thing: the window, and its
/// sides (`StretchArea.EStretchAreaType`).
#[derive(Component)]
struct WinStretch(usize, u8);

/// A dropdown row's arrow in a window that shows a thing: the thing, the
/// row's place among its figures' rows, and the window's layer.
#[derive(Component)]
struct InfoTip(Id, usize, u8);

/// A dropdown row's tooltip while the mouse rests on its arrow, on top
/// where the mouse is (`HoverTrigger`: shown as the mouse comes onto it,
/// shut as it leaves; `CompactCharacteristicDropdownPanel.ShowTooltip`): its
/// lines (`health_view::draw_tip`), its bottom left corner at the mouse
/// (`ESTIMATE`: the tooltip's own offset is not read).
#[allow(clippy::too_many_arguments)]
fn info_tips(
    mut commands: Commands,
    controller: Res<Controller>,
    carried: Res<Carried>,
    look: Res<crate::health_view::HealthLook>,
    game: Res<crate::GameData>,
    window: Query<&Window, With<PrimaryWindow>>,
    units: Res<Units>,
    (tips, covers): (Query<(&InfoTip, &RelativeCursorPosition)>, Query<(&Cover, &RelativeCursorPosition)>),
    mut shown: Local<Option<(Id, usize, Entity)>>,
) {
    let top = cover_under(covers.iter());
    let under = if controller.inventory_open { tips.iter().find(|(tip, at)| at.cursor_over() && tip.2 == top).map(|(tip, _)| (tip.0, tip.1)) } else { None };
    if under == shown.map(|(id, row, _)| (id, row)) {
        return;
    }
    if let Some((_, _, entity)) = shown.take() {
        commands.entity(entity).despawn();
    }
    let (Some((id, row)), Ok(screen)) = (under, window.single()) else { return };
    let (Some(item), Some(cursor)) = (carried.find(id), screen.cursor_position()) else { return };
    let figures = trp_sim::item_attributes::figures(item, &game.db, &|key| carried.texts.say(key));
    let Some(lines) = figures.rows.get(row).map(|row| row.tip.clone()).filter(|lines| !lines.is_empty()) else { return };
    let entity = crate::health_view::draw_tip(&mut commands, &look, &lines, cursor, Vec2::new(screen.width(), screen.height()), units.0);
    *shown = Some((id, row, entity));
}

/// What the window that shows a thing holds besides its figures
/// (inspect-window spec), and where each of its slots is in the things:
/// its parts' panel's slots in the game's order (`CreateModSlots`:
/// `AllSlots`, the thing's own slots, then each part's after them, parts
/// of parts in turn, a weapon's chambers last; none for a rocket launcher),
/// each named in the install's words, with its picture's name, locked as a
/// raid locks it (`GetModLockedState`: a slot that is locked, an armour's
/// plate in armour worn, a part not to be changed in a raid, a vital
/// part's slot) and, empty, with its filters' marks; its buttons, the
/// thing's lines but Inspect (`InitInteractionButtonsPanel`); a weapon's
/// mastering, its level (`--mastering`), no points into it (not kept:
/// `PROVISIONAL`) and the points the level takes (the install's mastering
/// group's `Level2` for the first, `Level3` after); the size it is
/// stretched to.
#[allow(clippy::type_complexity)]
pub(crate) fn info_parts(carried: &Carried, item: &Item, held: &HandsHeld, health: Option<&trp_sim::health::Health>, game: &crate::GameData, said: &crate::health_view::Words, stretched: Option<[f32; 2]>) -> (crate::health_view::InfoParts, Vec<(Id, usize)>) {
    use crate::health_view::{InfoButtonLook, InfoMastering, InfoParts, InfoSlot};
    let db = &game.db;
    let is = |thing: &Item, class: &str| thing.kind.class_names.iter().any(|name| name == class);
    let chamber = |holder: &Item, index: usize| holder.kind.slots[index].name.starts_with("patron_in_weapon");
    fn flatten<'a>(slots: Vec<(&'a Item, usize)>, out: &mut Vec<(&'a Item, usize)>) {
        out.extend(slots.iter().copied());
        for (holder, index) in slots {
            if let Some(part) = holder.slots.get(index).and_then(Option::as_ref) {
                flatten((0..part.kind.slots.len()).map(|at| (part, at)).collect(), out);
            }
        }
    }
    let mut all = Vec::new();
    if !item.kind.slots.is_empty() && !is(item, "RocketLauncher") {
        flatten((0..item.kind.slots.len()).filter(|&at| !chamber(item, at)).map(|at| (item, at)).collect(), &mut all);
        all.extend((0..item.kind.slots.len()).filter(|&at| chamber(item, at)).map(|at| (item, at)));
    }
    let worn = matches!(carried.lies(item.id), Some((holder, Spot::Slot(_))) if holder == carried.equipment.item.id);
    let launcher = is(item, "Launcher");
    let slots = all
        .iter()
        .map(|(holder, number)| {
            let holder_slot = &holder.kind.slots[*number];
            let part = holder.slots.get(*number).and_then(Option::as_ref);
            let template = db.items.get(&holder.kind.template);
            let armour = template.and_then(|template| trp_sim::armor::ArmorSlot::of(template, &holder_slot.name));
            let locked_slot = armour.as_ref().is_some_and(|slot| slot.locked);
            let plate = part.is_some_and(|part| is(part, "ArmorPlate"));
            let fixed = part.is_some_and(|part| is(part, "Mod") && part.kind.not_raid_moddable);
            let key = slot_name(&holder_slot.name);
            let name = carried.texts.say(key).unwrap_or_else(|| key.to_owned()).to_uppercase();
            // `ModSlotView.Show`: `Slots/` and the slot's `Name` (the first of
            // the weapon's mod kinds its id has: `mod_mount_004` is
            // `mod_mount`), as the caption.
            let picture = if launcher { "grenade_patron_in_weapon".to_owned() } else { key.to_lowercase() };
            InfoSlot {
                name,
                picture,
                armour: holder_slot.armour,
                full: part.is_some(),
                locked: (plate && worn && !locked_slot) || locked_slot || fixed || holder_slot.required,
                marks: if part.is_none() { crate::edit_build::filter_marks(db, holder_slot) } else { Vec::new() },
            }
        })
        .collect();
    let at = all.iter().map(|(holder, number)| (holder.id, *number)).collect();
    let buttons = menu_lines(carried, item.id, held, health, db, said)
        .into_iter()
        .filter(|(line, ..)| *line != trp_sim::item_menu::Line::Inspect)
        .map(|(line, can, caption)| InfoButtonLook { name: line.name(), caption, enabled: can.is_ok(), arrow: line.has_lines() })
        .collect();
    let mastering = is(item, "Weapon").then(|| {
        let level = carried.mastering.min(2);
        let group = db.config.get("Mastering").and_then(|groups| groups.as_array()).and_then(|groups| {
            groups.iter().find(|group| group.get("Templates").and_then(|list| list.as_array()).is_some_and(|list| list.iter().any(|id| id.as_str() == Some(item.kind.template.as_str()))))
        });
        let threshold = |key: &str| group.and_then(|group| group.get(key)).and_then(|value| value.as_f64()).map(|value| value as f32);
        let most = if level == 0 { threshold("Level2") } else { threshold("Level3") };
        InfoMastering { level, points: 0.0, most: most.unwrap_or(1500.0) }
    });
    (InfoParts { slots, buttons, mastering, stretched }, at)
}

/// A slot's name as the game names it (`Slot.Name`): the first of the
/// kinds of weapon part (`EWeaponModType`, in its order) its own name
/// holds, so `mod_mount_004` is `mod_mount` ("MOUNT"); else its own name.
pub(crate) fn slot_name(id: &str) -> &str {
    const KINDS: [&str; 29] = [
        "mod_mount", "mod_scope", "mod_tactical", "mod_stock", "mod_magazine", "mod_barrel", "mod_handguard", "mod_muzzle", "mod_sight_front", "mod_sight_rear", "mod_foregrip", "mod_reciever", "mod_charge", "mod_pistol_grip", "mod_launcher",
        "mod_bipod", "mod_mag_shaft", "mod_silencer", "mod_tactical_2", "chamber0", "chamber1", "patron_in_weapon", "mod_gas_block", "mod_equipment", "mod_equipment_000", "mod_equipment_001", "mod_nvg", "mod_flashlight", "mod_muzzle_001",
    ];
    KINDS.iter().find(|kind| id.contains(**kind)).copied().unwrap_or(id)
}

/// A container's slots shown with it as its special slots: every one but
/// an armour's (`SearchableSlotView.CreateSlots` leaves out each
/// `ArmorSlot`: an armoured rig's plates and inserts show only where the
/// thing is looked at or modded).
/// The game's window of a container's grids laid out for a container,
/// where the game's window was read (`TARKOV UI`): its caption the
/// container's short name, in the locale's case.
fn grid_built(look: &crate::health_view::HealthLook, carried: &Carried, item: &Item, picked: bool) -> Option<crate::health_view::GridWindowBuilt> {
    let cells: Vec<[u32; 2]> = item.kind.grids.iter().map(|grid| grid.cells).collect();
    crate::health_view::grid_window_built(look, &carried.name(item), &item.kind.layout, &cells, picked)
}

fn special_slots(item: &Item) -> Vec<usize> {
    (0..item.kind.slots.len()).filter(|&index| !item.kind.slots[index].armour).collect()
}

/// A thing's picture in a quick-use view (`QuickSlotItemView.UpdateScale`):
/// turned 45 degrees and made as large as fits the view (`room`, units):
/// the picture's own size (its cells, 63 units to a cell) turned, each of
/// its two sides brought within the view's, the smaller share taken (a
/// square picture by its height alone).
pub(crate) fn quick_picture(place: &mut ChildSpawnerCommands, carried: &Carried, item: &Item, room: [f32; 2], s: f32) {
    let Some(picture) = carried.pictures.get(&shown_key(carried, item)).or_else(|| carried.icons.get(icon_of(item))) else { return };
    let size = quick_fit(item.cells(), room);
    let node = placed((room[0] - size.x) / 2.0 * s, (room[1] - size.y) / 2.0 * s, size.x * s, size.y * s);
    place.spawn((ImageNode::new(picture.upright.clone()), node, UiTransform { rotation: Rot2::degrees(-45.0), ..default() }));
}

/// How large a thing's picture of so many cells is drawn in a quick-use
/// view so large (units), before it is turned (`QuickSlotItemView.
/// UpdateScale`: its size turned 45 degrees, `(w - h, w + h)` over the
/// square root of two, each part within the view's, the smaller share).
fn quick_fit(cells: [u32; 2], room: [f32; 2]) -> Vec2 {
    let own = Vec2::new(cells[0] as f32 * CELL, cells[1] as f32 * CELL);
    let turned = Vec2::new((own.x - own.y).abs(), own.x + own.y) * std::f32::consts::FRAC_1_SQRT_2;
    let share = [room[0] / turned.x, room[1] / turned.y].into_iter().filter(|share| share.is_finite()).fold(f32::MAX, f32::min);
    own * share
}

/// A window over the page (`GridWindow`, `InfoWindow`): a caption with
/// the thing's name, by which it is moved, a button that shuts it, and
/// under it the container's grids or what the thing is. The window whose
/// container Ctrl and a click sends things to has its caption lit.
#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_arguments)]
fn window_box(commands: &mut Commands, carried: &Carried, window: Win, index: usize, units: Units, look: &crate::health_view::HealthLook, said: &crate::health_view::Words, (game, previews, held, health): (&crate::GameData, &crate::info_preview::InfoPreviews, &HandsHeld, Option<&trp_sim::health::Health>), scrolled: f32) {
    let Some(item) = carried.find(window.item) else { return };
    let (cell, layer) = (units.cell(), WINDOW_LAYER + index as u8);
    // The game's own window that shows a thing (`TARKOV UI`), with a
    // place over its caption bar to move it by and one over its cross.
    if window.kind == WinKind::Tag {
        return tag_window_box(commands, carried, window, index, units, look, said);
    }
    if window.kind == WinKind::Repair {
        return repair_window_box(commands, carried, window, index, units, look, said, (game, previews));
    }
    if let (WinKind::Info, true) = (window.kind, look.info.is_some()) {
        let s = units.0;
        let template = &item.kind.template;
        let words = crate::health_view::InfoWords {
            tag: (!item.tag.name.trim().is_empty()).then(|| (item.tag.name.clone(), item.tag.color)),
            wish: carried.wishlist.get(template).map(|group| group.name()),
            caption: carried.texts.names.get(template).cloned().unwrap_or_else(|| carried.name(item)),
            kind: carried.texts.kinds.get(template).cloned().unwrap_or_default(),
            weight: format!("{:.3}{}", item.weight(), carried.texts.kg),
            description: carried.texts.descriptions.get(template).cloned().unwrap_or_default(),
        };
        // Its figures (inspect-window spec), its parts' slots, its buttons,
        // a weapon's mastering, and its model, its loader until the model
        // is in.
        let figures = trp_sim::item_attributes::figures(item, &game.db, &|key| carried.texts.say(key));
        let (parts, at) = info_parts(carried, item, held, health, game, said, window.stretched);
        let model = previews.picture(window.item);
        let Some(built) = crate::health_view::info_built(look, &words, Some(&figures), &parts, model.is_none()) else { return };
        let Some(places) = built.places() else { return };
        let [w, h] = places.size;
        commands
            .spawn((Overlay, WinRoot(index), Cover(layer), RelativeCursorPosition::default(), placed(window.at.x, window.at.y, w * s, h * s), GlobalZIndex(13 + index as i32)))
            .with_children(|inside| {
                crate::health_view::draw_info_window(inside, look, said, &words, &built, index, s, (window.opened, scrolled));
                // The description's scroll view, where it scrolls: its area
                // for the wheel, its bar for the mouse's presses.
                if let Some(scroll) = &built.scroll {
                    let [x, y, w, h] = scroll.area;
                    inside.spawn((InfoScrollArea { opened: window.opened, layer, scroll: scroll.clone() }, RelativeCursorPosition::default(), placed(x * s, y * s, w * s, h * s)));
                    let [x, y, w, h] = scroll.bar;
                    inside.spawn((InfoScrollBar(window.opened), RelativeCursorPosition::default(), placed(x * s, y * s, w * s, h * s)));
                }
                // Each slot's part as its view draws it, and the slot to
                // take one from or put one in; a locked one at half its
                // alpha (`ModLock`: its group's, as the panel's ground
                // shows through) and out of the mouse's reach.
                let ground = Color::srgb(0.0667, 0.0706, 0.0706);
                for ((slot, (holder, number)), [x, y, w, h]) in parts.slots.iter().zip(&at).zip(&built.slots) {
                    let Some(holder) = carried.find(*holder) else { continue };
                    let reach = if slot.locked { OUT_OF_REACH } else { layer };
                    slot_box_drawn(inside, carried, holder, *number, placed(x * s, y * s, w * s, h * s), Vec2::new(w * s, h * s), reach, false, false, s);
                    if slot.locked {
                        inside.spawn((placed(x * s, y * s, w * s, h * s), BackgroundColor(ground.with_alpha(0.5)), bevy::ui::FocusPolicy::Pass));
                    }
                }
                // Its buttons, each a line of the thing's menu.
                for ((button, [x, y, w, h]), ground) in parts.buttons.iter().zip(&built.buttons).zip(&built.grounds) {
                    if let Some(line) = trp_sim::item_menu::Line::ALL.into_iter().find(|line| line.name() == button.name) {
                        let mut hit = inside.spawn((InfoButtonHit(window.item, line, layer), RelativeCursorPosition::default(), placed(x * s, y * s, w * s, h * s)));
                        if let Some((node, colors, alpha)) = ground {
                            hit.insert(crate::health_view::InfoButtonLight { piece: crate::health_view::info_piece(index, *node), colors: *colors, alpha: *alpha, layer });
                        }
                    }
                }
                // Its stretching places, on its edges and corners, outside it.
                for (sides, [x, y, w, h]) in &built.stretch {
                    inside.spawn((WinStretch(index, *sides), crate::cursor::StretchPointer(index, *sides), RelativeCursorPosition::default(), placed(x * s, y * s, w * s, h * s)));
                }
                // Each dropdown row's arrow, which says its lines under the
                // mouse.
                for ([x, y, w, h], row) in &built.tips {
                    inside.spawn((InfoTip(window.item, *row, layer), RelativeCursorPosition::default(), placed(x * s, y * s, w * s, h * s)));
                }
                // The model's picture over the preview's room, where a drag
                // turns it and the wheel zooms it.
                if let Some([x, y, w, h]) = built.preview() {
                    let room = placed(x * s, y * s, w * s, h * s);
                    match model {
                        Some(image) => inside.spawn((crate::info_preview::InfoPicture(window.item, layer), RelativeCursorPosition::default(), ImageNode::new(image), room)),
                        None => inside.spawn((crate::info_preview::InfoPicture(window.item, layer), RelativeCursorPosition::default(), room)),
                    };
                }
                let [x, y, w, h] = places.caption;
                inside.spawn((WinCaption(index), RelativeCursorPosition::default(), placed(x * s, y * s, w * s, h * s)));
                let [x, y, w, h] = places.close;
                inside.spawn((WinClose(index), Button, placed(x * s, y * s, w * s, h * s)));
            });
        return;
    }
    let lit = window.kind == WinKind::Grids && carried.priority == Some(window.item);
    // The game's own window of a container's grids (`TARKOV UI`), with a
    // place over its caption's bar to move it by and one over its cross;
    // the things in its grids as the crate's are drawn.
    if window.kind == WinKind::Grids {
        if let Some(built) = grid_built(look, carried, item, lit) {
            let s = units.0;
            let [w, h] = built.size;
            commands
                .spawn((Overlay, WinRoot(index), Cover(layer), RelativeCursorPosition::default(), placed(window.at.x, window.at.y, w * s, h * s), GlobalZIndex(13 + index as i32)))
                .with_children(|inside| {
                    crate::health_view::draw_grid_window(inside, look, said, &built, index, s);
                    for (grid, [x, y, w, h]) in built.grids.iter().copied().enumerate() {
                        inside.spawn(placed(x * s, y * s, w * s, h * s)).with_children(|holder| grid_box_drawn(holder, carried, item, grid, cell, layer, false));
                    }
                    let [x, y, w, h] = built.bar;
                    inside.spawn((WinCaption(index), RelativeCursorPosition::default(), placed(x * s, y * s, w * s, h * s)));
                    let [x, y, w, h] = built.close;
                    inside.spawn((WinClose(index), Button, placed(x * s, y * s, w * s, h * s)));
                });
            return;
        }
    }
    let root = Node { position_type: PositionType::Absolute, left: Val::Px(window.at.x), top: Val::Px(window.at.y), flex_direction: FlexDirection::Column, border: UiRect::all(Val::Px(1.0)), ..default() };
    commands
        .spawn((Overlay, WinRoot(index), Cover(layer), RelativeCursorPosition::default(), root, BorderColor::all(LINE), BackgroundColor(PANEL), GlobalZIndex(13 + index as i32)))
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
                if window.kind == WinKind::Grids {
                    let wrap = Node { flex_wrap: FlexWrap::Wrap, column_gap: Val::Px(5.0), row_gap: Val::Px(5.0), max_width: Val::Px(8.0 * cell + 20.0), ..default() };
                    body.spawn(wrap).with_children(|grids| {
                        for grid in 0..item.kind.grids.len() {
                            grid_box(grids, carried, item, grid, cell, layer);
                        }
                        for special in special_slots(item) {
                            slot_box(grids, carried, item, special, Node { width: Val::Px(cell), height: Val::Px(cell), ..default() }, Vec2::splat(cell), layer, cell / CELL);
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
                    carried.texts.descriptions.get(&item.kind.template).cloned().unwrap_or_default(),
                ];
                let (description, rest) = lines.split_last().expect("five lines");
                for line in rest.iter().filter(|line| !line.is_empty()) {
                    body.spawn((Node { max_width: Val::Px(320.0), ..default() }, words(line.clone(), 12.0, WORDS)));
                }
                // Medicine's, food's and drink's figures as the game's window
                // lists them (`item_attributes::figures`), a line each, its
                // colour tags left out.
                if is(item, "Meds") || is(item, "FoodDrink") {
                    let figures = trp_sim::item_attributes::figures(item, &game.db, &|key| carried.texts.say(key));
                    for row in &figures.rows {
                        let mut value = untagged(row.value.as_deref().unwrap_or(""));
                        if let Some(base) = row.colored.filter(|_| !value.is_empty()) {
                            if base > 0.0 {
                                value.insert(0, '+');
                            } else if base < 0.0 && !value.contains('-') {
                                value.insert(0, '-');
                            }
                        }
                        let line = if value.is_empty() { row.name.clone() } else { format!("{}   {value}", row.name) };
                        body.spawn((Node { max_width: Val::Px(320.0), ..default() }, words(line, 12.0, WORDS)));
                    }
                }
                if !description.is_empty() {
                    body.spawn((Node { max_width: Val::Px(320.0), ..default() }, words(description.clone(), 12.0, WORDS)));
                }
            });
        });
}

/// Words with TextMeshPro's tags (`<color=...>`, `</color>`) left out.
fn untagged(words: &str) -> String {
    let mut out = String::with_capacity(words.len());
    let mut inside = false;
    for c in words.chars() {
        match c {
            '<' => inside = true,
            '>' if inside => inside = false,
            _ if !inside => out.push(c),
            _ => {}
        }
    }
    out
}

// ==== The repair window ====

/// The repair window (`RepairWindow`): the game's own (`TARKOV UI`), its
/// kit's picture as its repairer's avatar, the thing's model in its
/// preview, a place over its caption to move it by, its cross, its slider,
/// its top's button and its REPAIR (`repair_view::spots`); else the app's
/// look of the same figures, in words, with its REPAIR.
#[allow(clippy::too_many_arguments)]
fn repair_window_box(commands: &mut Commands, carried: &Carried, window: Win, index: usize, units: Units, look: &crate::health_view::HealthLook, said: &crate::health_view::Words, (game, previews): (&crate::GameData, &crate::info_preview::InfoPreviews)) {
    let Some(item) = carried.find(window.item) else { return };
    let Some(repair) = carried.repairs.iter().find(|repair| repair.thing == window.item) else { return };
    let (layer, s) = (WINDOW_LAYER + index as u8, units.0);
    let model = previews.picture(window.item);
    let Some((shown, _)) = crate::repair_view::repair_words(carried, item, repair, &game.db, said, model.is_none()) else { return };
    if let Some(built) = crate::health_view::repair_built(look, &shown) {
        let [w, h] = built.size;
        commands
            .spawn((Overlay, WinRoot(index), Cover(layer), RelativeCursorPosition::default(), placed(window.at.x, window.at.y, w * s, h * s), GlobalZIndex(13 + index as i32)))
            .with_children(|inside| {
                crate::health_view::draw_repair_window(inside, look, said, &built, index, s);
                if let Some(kit) = carried.find(repair.kit) {
                    let [x, y, w, h] = built.avatar;
                    inside.spawn(placed(x * s, y * s, w * s, h * s)).with_children(|room| picture_in(room, carried, kit, Vec2::new(w * s, h * s), false, None));
                }
                let [x, y, w, h] = built.preview;
                let room = placed(x * s, y * s, w * s, h * s);
                match model {
                    Some(image) => inside.spawn((crate::info_preview::InfoPicture(window.item, layer), RelativeCursorPosition::default(), ImageNode::new(image), room)),
                    None => inside.spawn((crate::info_preview::InfoPicture(window.item, layer), RelativeCursorPosition::default(), room)),
                };
                crate::repair_view::spots(inside, &built, index, s);
                let [x, y, w, h] = built.caption;
                inside.spawn((WinCaption(index), RelativeCursorPosition::default(), placed(x * s, y * s, w * s, h * s)));
                let [x, y, w, h] = built.close;
                inside.spawn((WinClose(index), Button, placed(x * s, y * s, w * s, h * s)));
            });
        return;
    }
    // The app's own look: the same figures in words.
    let root = Node { position_type: PositionType::Absolute, left: Val::Px(window.at.x), top: Val::Px(window.at.y), flex_direction: FlexDirection::Column, border: UiRect::all(Val::Px(1.0)), ..default() };
    let plain = |said: &str| crate::health_view::rich_spans(said).map_or_else(|| said.to_owned(), |spans| spans.into_iter().map(|(words, _)| words).collect::<String>());
    commands
        .spawn((Overlay, WinRoot(index), Cover(layer), RelativeCursorPosition::default(), root, BorderColor::all(LINE), BackgroundColor(PANEL), GlobalZIndex(13 + index as i32)))
        .with_children(|inside| {
            let caption = Node { justify_content: JustifyContent::SpaceBetween, align_items: AlignItems::Center, column_gap: Val::Px(16.0), padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)), ..default() };
            inside.spawn((WinCaption(index), RelativeCursorPosition::default(), caption, BackgroundColor(LINE))).with_children(|caption| {
                caption.spawn(words(words_caption(&shown.caption), 11.0, WORDS));
                caption.spawn((WinClose(index), Button, Node { padding: UiRect::horizontal(Val::Px(6.0)), ..default() }, BackgroundColor(PANEL))).with_children(|close| {
                    close.spawn(words("X", 11.0, WORDS));
                });
            });
            let body = Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(4.0), padding: UiRect::all(Val::Px(6.0)), ..default() };
            inside.spawn(body).with_children(|body| {
                let lines = [shown.dropdown.clone(), plain(&shown.charge)].into_iter().chain(shown.lines.iter().map(|line| plain(line))).chain(shown.warnings.iter().map(|(_, said)| said.clone())).chain([plain(&shown.durability), shown.repair_dp.clone(), format!("{} / {}", shown.in_stash, shown.to_pay)]);
                for line in lines.filter(|line| !line.is_empty()) {
                    body.spawn(words(line, 12.0, WORDS));
                }
                let ground = if shown.interactable { FAINT } else { LINE };
                body.spawn((crate::repair_view::RepairSpot::Press(index), RelativeCursorPosition::default(), Node { padding: UiRect::axes(Val::Px(10.0), Val::Px(4.0)), ..default() }, BackgroundColor(ground))).with_children(|button| {
                    button.spawn(words("REPAIR", 12.0, WORDS));
                });
            });
        });
}

/// The caption's words in the app's look: upper case, as its windows'.
fn words_caption(caption: &str) -> String {
    caption.to_uppercase()
}

// ==== Tags, asking and the stash's fills ====

/// Opens the window that edits a thing's tag (`ItemUiContext.EditTag`,
/// `OpenWindow`), with its name and colour as they stand, where the game
/// opens a window; or brings its own to the top.
fn open_tag(carried: &mut Carried, id: Id, cursor: Vec2, screen: Option<(Vec2, f32)>) {
    let Some(tag) = carried.find(id).filter(|item| trp_sim::inventory::takes_tag(item)).map(|item| item.tag.clone()) else { return };
    let at = match (carried.tag_size, screen) {
        (Some(size), Some((screen, s))) => game_window_at(carried, size, screen, s),
        _ => cursor + Vec2::new(14.0, 14.0),
    };
    if !carried.tag_edits.iter().any(|edit| edit.item == id) {
        carried.tag_edits.push(TagEdit { item: id, name: tag.name, color: tag.color, focused: false });
    }
    carried.open_window(id, WinKind::Tag, at);
}

/// A place of the window that edits a tag, by the window's number: its
/// field, a colour cell, its button.
#[derive(Component, Clone, Copy, PartialEq, Debug)]
struct TagPlace(usize, TagSpot);

#[derive(Clone, Copy, PartialEq, Debug)]
enum TagSpot {
    Field,
    Color(u32),
    Save,
}

/// The window that edits a tag: the game's own (`TARKOV UI`), else the
/// app's look of the same: its caption and cross, the seven colours with
/// the chosen one's border, the count of letters, the field (its name or
/// its placeholder, a caret while it has the keys) and its button.
#[allow(clippy::too_many_arguments)]
fn tag_window_box(commands: &mut Commands, carried: &Carried, window: Win, index: usize, units: Units, look: &crate::health_view::HealthLook, said: &crate::health_view::Words) {
    let Some(edit) = carried.tag_edits.iter().find(|edit| edit.item == window.item) else { return };
    let (layer, s) = (WINDOW_LAYER + index as u8, units.0);
    if let Some(places) = crate::health_view::tag_places(look) {
        let [w, h] = places.size;
        commands
            .spawn((Overlay, WinRoot(index), Cover(layer), RelativeCursorPosition::default(), placed(window.at.x, window.at.y, w * s, h * s), GlobalZIndex(13 + index as i32)))
            .with_children(|inside| {
                let words = crate::health_view::TagWords { name: &edit.name, color: edit.color, focused: edit.focused };
                crate::health_view::draw_tag_window(inside, look, said, &words, index, s);
                let at = |[x, y, w, h]: [f32; 4]| placed(x * s, y * s, w * s, h * s);
                inside.spawn((WinCaption(index), RelativeCursorPosition::default(), at(places.caption)));
                inside.spawn((TagPlace(index, TagSpot::Field), Button, at(places.input)));
                for (n, cell) in places.cells.iter().enumerate() {
                    inside.spawn((TagPlace(index, TagSpot::Color(n as u32)), Button, at(*cell)));
                }
                inside.spawn((TagPlace(index, TagSpot::Save), Button, at(places.save)));
                inside.spawn((WinClose(index), Button, at(places.close)));
            });
        return;
    }
    let root = Node { position_type: PositionType::Absolute, left: Val::Px(window.at.x), top: Val::Px(window.at.y), flex_direction: FlexDirection::Column, border: UiRect::all(Val::Px(1.0)), ..default() };
    commands
        .spawn((Overlay, WinRoot(index), Cover(layer), RelativeCursorPosition::default(), root, BorderColor::all(LINE), BackgroundColor(PANEL), GlobalZIndex(13 + index as i32)))
        .with_children(|inside| {
            let caption = Node { justify_content: JustifyContent::SpaceBetween, align_items: AlignItems::Center, column_gap: Val::Px(16.0), padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)), ..default() };
            inside.spawn((WinCaption(index), RelativeCursorPosition::default(), caption, BackgroundColor(LINE))).with_children(|caption| {
                caption.spawn(words(said.word("ADD/EDIT CONTAINER TAG", "ADD/EDIT CONTAINER TAG"), 11.0, WORDS));
                caption.spawn((WinClose(index), Button, Node { padding: UiRect::horizontal(Val::Px(6.0)), ..default() }, BackgroundColor(PANEL))).with_children(|close| {
                    close.spawn(words("X", 11.0, WORDS));
                });
            });
            let body = Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(6.0), padding: UiRect::all(Val::Px(8.0)), ..default() };
            inside.spawn(body).with_children(|body| {
                body.spawn(Node { column_gap: Val::Px(10.0), align_items: AlignItems::Center, ..default() }).with_children(|row| {
                    for n in 0..crate::health_view::TAG_COLOURS.len() as u32 {
                        let chosen = n == edit.color;
                        let cell = Node { width: Val::Px(22.0), height: Val::Px(22.0), border: UiRect::all(Val::Px(if chosen { 2.0 } else { 0.0 })), ..default() };
                        row.spawn((TagPlace(index, TagSpot::Color(n)), Button, cell, BackgroundColor(crate::health_view::tag_colour(n)), BorderColor::all(WORDS)));
                    }
                    row.spawn(words(format!("{}/{}", edit.name.chars().count(), carried.tag_limit), 14.0, FAINT));
                });
                let field = Node { width: Val::Px(320.0), height: Val::Px(30.0), align_items: AlignItems::Center, padding: UiRect::horizontal(Val::Px(6.0)), border: UiRect::all(Val::Px(1.0)), ..default() };
                body.spawn((TagPlace(index, TagSpot::Field), Button, field, BorderColor::all(if edit.focused { WORDS } else { LINE }))).with_children(|field| {
                    let shown = if edit.name.is_empty() { said.word("container tag", "container tag") } else { edit.name.clone() };
                    let caret = if edit.focused { "|" } else { "" };
                    field.spawn(words(format!("{shown}{caret}"), 16.0, if edit.name.is_empty() { FAINT } else { WORDS }));
                });
                body.spawn((TagPlace(index, TagSpot::Save), Button, Node { align_self: AlignSelf::Center, padding: UiRect::axes(Val::Px(24.0), Val::Px(4.0)), ..default() }, BackgroundColor(LINE))).with_children(|save| {
                    save.spawn(words(said.word("Save", "Save").to_uppercase(), 14.0, WORDS));
                });
            });
        });
}

/// The windows that edit a tag (`EditTagWindow`; inventory spec, "A tag"):
/// a press on its field gives the field the keys and a press anywhere else
/// takes them away (TextMeshPro's field); while it has them the game's
/// bindings hear nothing, a letter its validation lets through is added up
/// to the field's most, Backspace takes the last off and Enter lets the
/// field go; a colour cell chooses its colour; the button saves the tag
/// (an empty name with colour 0) where it changed, and shuts the window
/// (`EditTagWindow.Save`). Escape and the cross shut it as any window
/// (`windows`). An edit goes with its window.
#[allow(clippy::too_many_arguments)]
fn tag_windows(
    buttons: Res<ButtonInput<MouseButton>>,
    mut typed: MessageReader<bevy::input::keyboard::KeyboardInput>,
    controller: Res<Controller>,
    mut carried: ResMut<Carried>,
    places: Query<(&Interaction, &TagPlace)>,
    mut bindings: ResMut<Bindings>,
) {
    let open = |carried: &Carried, item: Id| carried.windows.iter().any(|window| window.kind == WinKind::Tag && window.item == item);
    if carried.tag_edits.iter().any(|edit| !open(&carried, edit.item)) {
        let carried = &mut *carried;
        let windows = carried.windows.clone();
        carried.tag_edits.retain(|edit| windows.iter().any(|window| window.kind == WinKind::Tag && window.item == edit.item));
    }
    let typing = controller.inventory_open && carried.tag_edits.iter().any(|edit| edit.focused);
    if bindings.typing != typing {
        bindings.typing = typing;
    }
    if carried.tag_edits.is_empty() {
        typed.clear();
        return;
    }
    let item_of = |carried: &Carried, window: usize| carried.windows.get(window).filter(|window| window.kind == WinKind::Tag).map(|window| window.item);
    // A press: on a field it has the keys, anywhere else none has.
    if buttons.just_pressed(MouseButton::Left) {
        let pressed: Vec<TagPlace> = places.iter().filter(|(interaction, _)| **interaction == Interaction::Pressed).map(|(_, place)| *place).collect();
        let field = pressed.iter().find(|place| place.1 == TagSpot::Field).and_then(|place| item_of(&carried, place.0));
        if carried.tag_edits.iter().any(|edit| edit.focused != (Some(edit.item) == field)) {
            for edit in &mut carried.tag_edits {
                edit.focused = Some(edit.item) == field;
            }
        }
        for place in pressed {
            let Some(item) = item_of(&carried, place.0) else { continue };
            match place.1 {
                TagSpot::Color(n) => {
                    if let Some(edit) = carried.tag_edits.iter_mut().find(|edit| edit.item == item && edit.color != n) {
                        edit.color = n;
                    }
                }
                TagSpot::Save => {
                    let Some(edit) = carried.tag_edits.iter().find(|edit| edit.item == item).cloned() else { continue };
                    let color = if edit.name.is_empty() { 0 } else { edit.color };
                    let carried = &mut *carried;
                    let changed = carried.find(item).is_some_and(|thing| thing.tag.name != edit.name || thing.tag.color != color);
                    let name = carried.find(item).map(|thing| carried.name(thing)).unwrap_or_default();
                    if changed && (trp_sim::inventory::set_tag(&mut carried.equipment.item, item, &edit.name, color) || trp_sim::inventory::set_tag(&mut carried.loot, item, &edit.name, color)) {
                        carried.note = if edit.name.is_empty() { format!("{name}: tag reset") } else { format!("{name}: tagged {}", edit.name) };
                    }
                    carried.windows.retain(|window| !(window.kind == WinKind::Tag && window.item == item));
                    carried.tag_edits.retain(|edit| edit.item != item);
                }
                TagSpot::Field => {}
            }
        }
    }
    // The keys, to the field that has them.
    let limit = carried.tag_limit as usize;
    let Some(at) = carried.tag_edits.iter().position(|edit| edit.focused) else {
        typed.clear();
        return;
    };
    let before = carried.tag_edits[at].clone();
    let mut edit = before.clone();
    for key in typed.read().filter(|key| key.state.is_pressed()) {
        use bevy::input::keyboard::Key;
        match &key.logical_key {
            Key::Backspace => {
                edit.name.pop();
            }
            Key::Enter => edit.focused = false,
            _ => {
                for c in key.text.as_deref().unwrap_or_default().chars().filter(|c| !c.is_control() && tag_letter(*c)) {
                    if edit.name.chars().count() < limit {
                        edit.name.push(c);
                    }
                }
            }
        }
    }
    if edit != before {
        carried.tag_edits[at] = edit;
    }
}

/// The window that asks how many of a stack (`SplitDialog`, with its
/// stepped slider): the figure, the slider with a stop at each of the
/// game's notches, a button to go through with it and one to drop it.
fn asking_box(commands: &mut Commands, asking: &Asking, window: Vec2) {
    const WIDTH: f32 = 260.0;
    let (share, gone) = asking.shares();
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
    commands.spawn((Overlay, Cover(ASKING_LAYER), RelativeCursorPosition::default(), root, BorderColor::all(WORDS), BackgroundColor(PANEL), GlobalZIndex(30))).with_children(|inside| {
        inside.spawn(Node { justify_content: JustifyContent::SpaceBetween, ..default() }).with_children(|top| {
            top.spawn(words(asking.title, 12.0, WORDS));
            top.spawn((AskClose, Button, Node { padding: UiRect::horizontal(Val::Px(6.0)), ..default() }, BackgroundColor(LINE))).with_children(|close| {
                close.spawn(words("X", 11.0, WORDS));
            });
        });
        inside.spawn(words(format!("{}   of {}", asking.count, asking.most), 16.0, WORDS));
        let bar = Node { width: Val::Percent(100.0), height: Val::Px(14.0), ..default() };
        inside.spawn((AskBar, RelativeCursorPosition::default(), bar, BackgroundColor(BACK))).with_children(|bar| {
            // What is gone of food or drink lies before the knob's first
            // place, darker (the game's notches not available).
            if gone > 0.0 {
                bar.spawn((Node { width: Val::Percent(gone * 100.0), height: Val::Percent(100.0), ..default() }, BackgroundColor(LINE)));
            }
            bar.spawn((Node { width: Val::Percent((share - gone).max(0.0) * 100.0), height: Val::Percent(100.0), ..default() }, BackgroundColor(FAINT)));
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
///
/// Behind the slots, the character's own model where the options have it
/// (`CHARACTER MODEL`, `figure_view`), over the whole panel; else the
/// app's paper figure.
/// What the page's left panel shows: the tab it is drawn for (GEAR or
/// HEALTH), the game's health panel with the words and the health it
/// shows, and the screen's size in its units, which the left panel's
/// width goes by.
pub struct LeftPanel<'a> {
    pub tab: usize,
    pub look: &'a crate::health_view::HealthLook,
    pub said: &'a crate::health_view::Words,
    pub health: Option<&'a trp_sim::health::Health>,
    pub screen: [f32; 2],
    /// The crate panel's stripes cut to their masks (`StashFills`).
    pub stash_fills: &'a HashMap<usize, Handle<Image>>,
}

/// The crate panel's stripes cut to their masks (`TARKOV UI`), made at the
/// window's scale (`make_stash_fills`), by the mask's object.
#[derive(Resource, Default)]
pub struct StashFills {
    scale: u32,
    images: HashMap<usize, Handle<Image>>,
}

/// Makes the crate panel's stripes (`StashFills`) afresh as the window's
/// scale or shape changes, before the page is drawn with them.
fn make_stash_fills(look: Res<crate::health_view::HealthLook>, window: Query<&Window, With<PrimaryWindow>>, mut images: ResMut<Assets<Image>>, mut fills: ResMut<StashFills>) {
    let (Some(game), Ok(window)) = (look.panel.as_ref().filter(|_| look.tarkov_ui && look.stash.is_some()), window.single()) else { return };
    if window.height() <= 0.0 {
        return;
    }
    let s = window.height() / SCREEN_HEIGHT;
    let key = (s * 997.0 + window.width()).to_bits();
    if fills.scale == key {
        return;
    }
    let Some([_, _, width, height]) = game.stash_in([window.width() / s, SCREEN_HEIGHT]) else { return };
    for (_, old) in fills.images.drain() {
        images.remove(&old);
    }
    fills.images = crate::health_view::stash_fill_images(&look, [width, height], s, &mut images);
    fills.scale = key;
}

// ==== The GEAR page, the loot's scrolling, the game's tabs and scroll views ====

#[allow(clippy::too_many_arguments)]
fn gear_page(gear: &mut ChildSpawnerCommands, carried: &Carried, units: Units, scroll: f32, game_scrolls: [f32; 2], figure: Option<&Handle<Image>>, shows: &LeftPanel) {
    let (s, cell) = (units.0, units.cell());
    let worn = &carried.equipment.item;
    let size = gear_panel(carried);
    // The left panel as wide as the game's on a screen of this shape
    // (`HealthPanel::left_panel`), the same on both tabs, so that what is
    // beside it stays where it is.
    let game = shows.look.panel.as_ref();
    let left_size = game.map(|game| game.left_panel(shows.screen));
    // With `TARKOV UI` the page is as large as the window and laid out as
    // the game's screen is on a screen of this shape: the left side where
    // the game's is (`HealthPanel::left_side_in`), the equipment panel
    // where the game's left panel has it (`gear_in`), and the game's own
    // panels drawn in the left panel from its corner.
    let tarkov = game.zip(left_size).filter(|_| shows.look.tarkov_ui).map(|(game, left_size)| (game.left_side_in(shows.screen), game.gear_in(left_size)));
    let left = match (tarkov, left_size) {
        (Some(([x, y, ..], gear_at)), Some([width, height])) => Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(8.0),
            padding: UiRect { left: Val::Px(gear_at[0] * s), top: Val::Px(gear_at[1] * s), ..default() },
            ..placed(x * s, y * s, width * s, height * s)
        },
        _ => Node {
            width: Val::Px(left_size.map_or(size[0] + 60.0, |left| left[0]) * s),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: Val::Px(8.0),
            padding: UiRect::vertical(Val::Px(6.0)),
            flex_shrink: 0.0,
            ..default()
        },
    };
    let ground = tarkov.and_then(|_| crate::health_view::left_ground(shows.look)).unwrap_or(PANEL);
    let game_left = left_size.filter(|_| tarkov.is_some()).map(|left_size| (left_size, [0.0, 0.0]));
    let doll_at = tarkov.map_or([0.0, 0.0], |(_, gear_at)| [gear_at[0], gear_at[1]]);
    gear.spawn((left, BackgroundColor(ground))).with_children(|left| {
        let health_tab = shows.tab == HEALTH;
        // The left panel's foot first, under both tabs' panels.
        if let Some((left_size, origin)) = game_left {
            crate::health_view::draw_foot(left, shows.look, shows.said, left_size, origin, s);
        }
        if health_tab && shows.look.tarkov_ui {
            match game_left {
                Some((left_size, origin)) => crate::health_view::draw_panel(left, shows.look, shows.said, shows.health, left_size, origin, s),
                None => {
                    left.spawn(words("The game's health panel could not be read from the install.", 12.0, FAINT));
                }
            }
            return;
        }
        let panel = Node { width: Val::Px(size[0] * s), height: Val::Px(size[1] * s), flex_shrink: 0.0, ..default() };
        // With `TARKOV UI`, the game's own equipment panel under the
        // page's slots, where they are: its outline of the character and
        // each slot's view, full or empty, with its header.
        let game_gear = !health_tab && game_left.is_some();
        if game_gear {
            let worn = &carried.equipment.item;
            let present = |slot: &str| EQUIPMENT.iter().any(|(name, ..)| *name == slot) && worn.slot_index(slot).is_some();
            let full = |slot: &str| worn.slot_index(slot).and_then(|index| worn.slots[index].as_ref()).is_some();
            crate::health_view::draw_gear(left, shows.look, shows.said, size, doll_at, s, &present, &full);
        }
        left.spawn(panel).with_children(|doll| {
            // No model with `TARKOV UI` on: the game's own gear page has
            // a flat outline of the character instead (`CharacterGear`,
            // drawn under the slots by `health_view::draw_gear`).
            match figure {
                _ if shows.look.tarkov_ui => {}
                Some(figure) => {
                    doll.spawn((placed(0.0, 0.0, size[0] * s, size[1] * s), ImageNode::new(figure.clone())));
                }
                None => {
                    for [left, top, width, height, round] in FIGURE {
                        doll.spawn((Node { border_radius: BorderRadius::all(Val::Px(round * s)), ..placed(left * s, top * s, width * s, height * s) }, BackgroundColor(SHAPE)));
                    }
                }
            }
            // The HEALTH tab in the app's own look has the figure alone.
            if health_tab {
                return;
            }
            for (slot, caption, own) in EQUIPMENT {
                let Some(index) = worn.slot_index(slot) else { continue };
                let [left, top, width, height] = carried.panel.slot(slot).unwrap_or(own);
                let node = placed(left * s, top * s, width * s, height * s);
                if game_gear {
                    // The game's view lies under it: the box takes the
                    // mouse and holds what is in it, and draws nothing.
                    slot_box_drawn(doll, carried, worn, index, node, Vec2::new(width * s, height * s), 0, false, true, s);
                    continue;
                }
                // The caption sits over the view, as the game's header does.
                doll.spawn((placed(left * s, (top - CAPTION) * s, width * s, CAPTION * s), words(caption, 10.0, FAINT)));
                slot_box(doll, carried, worn, index, node, Vec2::new(width * s, height * s), 0, s);
            }
        });
        if health_tab {
            // The app's own health panel over the figure, its boxes where
            // the game's views are: put as the game's panel would be with
            // its equipment panel where this one is.
            let width = left_size.map_or(size[0] + 60.0, |left| left[0]);
            let gear_top = game.zip(left_size).map_or(0.0, |(game, left_size)| game.gear_in(left_size)[1]);
            let doll = [(width - size[0]) / 2.0, 6.0 / s];
            crate::health_view::draw_own_panel(left, shows.look, shows.said, shows.health, left_size, [0.0, doll[1] - gear_top], doll, s);
            return;
        }
        // The app's notes, narrower than the panel above them so that the
        // quick-use bar, which reaches into the left panel's foot, leaves
        // them whole.
        let notes = Val::Px((size[0] - 60.0) * s);
        if let Some((left_size, origin)) = game_left {
            // The game's strip of figures (`TARKOV UI`: weight, health,
            // hydration, energy) where the app has its weight and its
            // help; the app's note of what a click did, still, under the
            // slots.
            crate::health_view::draw_strip(left, shows.look, shows.said, left_size, origin, s);
            left.spawn((Node { max_width: notes, ..default() }, words(carried.note.clone(), 12.0, FAINT)));
            return;
        }
        // The carried weight, which is what the controller goes by.
        left.spawn(words(format!("WEIGHT   {:.1} kg", carried.equipment.carried_weight(carried.strength_elite)), 14.0, WORDS));
        left.spawn((Node { max_width: notes, ..default() }, words(carried.note.clone(), 12.0, FAINT)));
        // What the mouse and the keys do here, where the quick-use bar
        // does not reach.
        left.spawn((Node { max_width: notes, ..default() }, words(HELP, 10.0, FAINT)));
    });
    // The containers, with `TARKOV UI` as the game's own column draws them
    // (`ContainersPanel`): as large as the left panel and beside it, so far
    // from it as the left side's row has them; a row for each container
    // slot, as many units down from the column's top as the column's
    // padding says and so far apart, each with its header and slot panel
    // and the container's grids to the right of the panel, each grid
    // drawn as the game's grid view; the page's own boxes for the slots
    // and grids over them, drawing nothing of their own. The rows are in
    // the game's scroll view: cut to its parent (`Scrollview Parent`, a
    // mask, its ground), moved up as far as it is scrolled
    // (`GameScrolls`, `scroll_views`), with its bar at the
    // column's right while they are higher than it shows.
    if let (Some(views), Some(([x, y, ..], _)), Some(left_size)) = (shows.look.containers.as_ref(), tarkov, left_size) {
        let gap = game.map_or(5.0, |game| game.spacing);
        let column_at = [x + left_size[0] + gap, y];
        let rows = views.column.unwrap_or_default();
        let slot_x = views.slot_panel.map_or(0.0, |panel| views.panel.rect(panel, left_size)[0]);
        let after = crate::health_view::grids_after(shows.look);
        // The rows laid out first: the column is as high as they are with
        // its padding (`ContentSizeFitter` on its `VerticalLayoutGroup`).
        let mut laid = Vec::new();
        let mut y = rows.padding[2];
        for (slot, _) in CONTAINERS {
            let Some(index) = worn.slot_index(slot) else { continue };
            let item = worn.slots[index].as_ref();
            let grids = item.map_or_else(Vec::new, |item| {
                let cells: Vec<[u32; 2]> = item.kind.grids.iter().map(|grid| grid.cells).collect();
                crate::health_view::container_grids(shows.look, &item.kind.layout, &cells)
            });
            // A thing not shown in its slot (the pockets) leaves the row
            // its grids and its special slots (`SlotView.Show`).
            let panel_shown = item.is_none_or(|item| !item.kind.not_shown_in_slot);
            let specials = item.map_or(0, |item| special_slots(item).len());
            let start = if panel_shown { SLOT_SIDE } else if specials > 0 { SPECIAL_SIDE } else { 0.0 };
            let height = grids.iter().fold(start, |height, [_, top, _, h]| height.max(top + h));
            laid.push((slot, index, item, [slot_x, y], grids));
            y += height + rows.spacing;
        }
        let whole = if laid.is_empty() { rows.padding[2] + rows.padding[3] } else { y - rows.spacing + rows.padding[3] };
        let scrolling = views.scrolling;
        let view = scrolling.and_then(|scrolling| scrolling.view).map_or([0.0, 0.0, left_size[0], left_size[1]], |view| views.panel.rect(view, left_size));
        let most = (whole - view[3]).max(0.0);
        let scrolled = if scrolling.is_none_or(|scrolling| scrolling.clamped) { game_scrolls[COLUMN].clamp(0.0, most) } else { game_scrolls[COLUMN] };
        let mask = views.panel.find("Scrollview Parent").map_or([0.0, 0.0, left_size[0], left_size[1]], |parent| views.panel.rect(parent, left_size));
        let ground = crate::health_view::containers_ground(shows.look).unwrap_or(Color::NONE);
        gear.spawn(placed(column_at[0] * s, column_at[1] * s, left_size[0] * s, left_size[1] * s)).with_children(|column| {
            let cut = Node { overflow: Overflow::clip(), ..placed(mask[0] * s, mask[1] * s, mask[2] * s, mask[3] * s) };
            column.spawn((cut, BackgroundColor(ground))).with_children(|cut| {
                // Where the wheel scrolls it: the scroll view's own
                // rectangle (what is outside it is not the scroll view's
                // to the engine's events).
                let wheel = placed((view[0] - mask[0]) * s, (view[1] - mask[1]) * s, view[2] * s, view[3] * s);
                cut.spawn((ScrollWheel { kind: COLUMN, whole, shown: view[3], frame: column_at, size: left_size, content_top: -mask[1], grids: Vec::new() }, wheel, RelativeCursorPosition::default()));
                let content = placed(-mask[0] * s, (-mask[1] - scrolled) * s, left_size[0] * s, whole * s);
                cut.spawn((ScrollContent(COLUMN), content)).with_children(|column| {
                    for (slot, index, item, at, grids) in &laid {
                        let (index, at) = (*index, *at);
                        let panel_shown = item.is_none_or(|item| !item.kind.not_shown_in_slot);
                        crate::health_view::draw_container_row(column, shows.look, shows.said, left_size, slot, item.is_some(), panel_shown, at, s);
                        let side = SLOT_SIDE * s;
                        if panel_shown {
                            slot_box_drawn(column, carried, worn, index, placed(at[0] * s, at[1] * s, side, side), Vec2::splat(side), 0, false, true, s);
                        }
                        let Some(item) = item else { continue };
                        let left_of_grids = if panel_shown { at[0] + after } else { at[0] };
                        let mut right = 0.0f32;
                        for (grid, [x, top, w, h]) in grids.iter().copied().enumerate() {
                            crate::health_view::draw_grid_view(column, shows.look, shows.said, [left_of_grids + x, at[1] + top], [w, h], s);
                            column.spawn(placed((left_of_grids + x) * s, (at[1] + top) * s, w * s, h * s)).with_children(|holder| grid_box_drawn(holder, carried, item, grid, cell, 0, false));
                            right = right.max(x + w);
                        }
                        // Special slots (the pockets have three): the
                        // game's panel of them last in the row, as far
                        // after the grids as the row's spacing, 10 more in
                        // the pockets' (`ContainersPanel.Show`), the
                        // page's boxes over their views.
                        let specials = special_slots(item);
                        if !specials.is_empty() {
                            let spacing = after - SLOT_SIDE + if *slot == "Pockets" { 10.0 } else { 0.0 };
                            let places = crate::health_view::draw_special_slots(column, shows.look, shows.said, left_size, [left_of_grids + right + spacing, at[1]], specials.len(), s);
                            for (special, [x, y, w, h]) in specials.into_iter().zip(places) {
                                slot_box_drawn(column, carried, item, special, placed(x * s, y * s, w * s, h * s), Vec2::new(w * s, h * s), 0, false, true, s);
                            }
                        }
                    }
                });
            });
            // The bar, shown while the rows are higher than the view
            // (`ScrollbarVisibility.AutoHide`), and its handle, placed
            // and tinted as it is scrolled and worked
            // (`scroll_containers`).
            let Some((scrolling, bar)) = scrolling.and_then(|scrolling| Some((scrolling, scrolling.bar?))) else { return };
            if scrolling.hides && whole <= view[3] + 0.01 {
                return;
            }
            let [bx, by, bw, bh] = views.panel.rect(bar.object, left_size);
            let color = views.panel.nodes[bar.object].image.map_or(Color::BLACK, |image| crate::health_view::rgba(image.color));
            column.spawn((ScrollBarNode(COLUMN), placed(bx * s, by * s, bw * s, bh * s), BackgroundColor(color), RelativeCursorPosition::default()));
            let value = if most > 0.0 { 1.0 - scrolled / most } else { 1.0 };
            let handle = bar.handle_in(&views.panel, left_size, value, (view[3] / whole).clamp(0.0, 1.0));
            if let (Some([hx, hy, hw, hh]), Some(handle_object)) = (handle, bar.handle) {
                let color = views.panel.nodes[handle_object].image.map_or(Color::WHITE, |image| crate::health_view::rgba(image.color));
                column.spawn((ScrollHandle(COLUMN, color), placed(hx * s, hy * s, hw * s, hh * s), BackgroundColor(color)));
            }
        });
        // The left side's frame, over both its panels.
        crate::health_view::draw_border(gear, shows.look, shows.said, [left_size[0] * 2.0 + gap, left_size[1]], [x, column_at[1]], s);
    } else {
    // The containers: each in its slot, with its grids beside it.
    let middle = Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(6.0), flex_grow: 1.0, flex_shrink: 1.0, min_width: Val::Px(0.0), overflow: Overflow::clip(), ..default() };
    gear.spawn(middle).with_children(|column| {
        for (slot, caption) in CONTAINERS {
            let Some(index) = worn.slot_index(slot) else { continue };
            column.spawn(words(caption, 10.0, FAINT));
            column.spawn(Node { column_gap: Val::Px(8.0), align_items: AlignItems::FlexStart, ..default() }).with_children(|row| {
                let side = Val::Px(SLOT_SIDE * s);
                slot_box(row, carried, worn, index, Node { width: side, height: side, flex_shrink: 0.0, ..default() }, Vec2::splat(SLOT_SIDE * s), 0, s);
                let Some(item) = worn.slots[index].as_ref() else { return };
                row.spawn(Node { flex_wrap: FlexWrap::Wrap, column_gap: Val::Px(5.0), row_gap: Val::Px(5.0), ..default() }).with_children(|grids| {
                    for grid in 0..item.kind.grids.len() {
                        grid_box(grids, carried, item, grid, cell, 0);
                    }
                    // Special slots (the pockets have three).
                    for special in special_slots(item) {
                        slot_box(grids, carried, item, special, Node { width: Val::Px(cell), height: Val::Px(cell), ..default() }, Vec2::splat(cell), 0, s);
                    }
                });
            });
        }
    });
    }
    // With `TARKOV UI`, the game's own panel for a container opened where
    // the game's screen has it (`health_view::draw_stash`), and nothing
    // there while none is open, as the game shows none; the container's
    // grids laid out as the containers' column lays a container's
    // (`container_grids`), in the panel's scroll view (`scroll_views`).
    let game_stash = game.filter(|_| tarkov.is_some()).and_then(|game| game.stash_in(shows.screen)).and_then(|rect| Some((rect, crate::health_view::stash_places(shows.look, [rect[2], rect[3]])?)));
    if let Some(([x, y, width, height], places)) = game_stash {
        let look = shows.look;
        gear.spawn(placed(x * s, y * s, width * s, height * s)).with_children(|view| {
            if !carried.loot_open {
                return;
            }
            // A table of weapons' gun case or magazine box by its class (the
            // user's names for them), else the container's short name, in
            // capitals (`ItemController.ContainerName`).
            let name = crate::range::weapon_crate_label(carried.loot_of).unwrap_or_else(|| carried.name(&carried.loot)).to_uppercase();
            let loot = &carried.loot;
            let present: Vec<bool> = crate::health_view::FILTERS
                .iter()
                .enumerate()
                .map(|(place, classes)| place == 0 || loot.grids.iter().flatten().any(|(_, item)| classes.iter().any(|class| item.kind.class_names.iter().any(|name| name == class))))
                .collect();
            let size = [width, height];
            crate::health_view::draw_stash(view, look, shows.said, size, &name, &present, shows.stash_fills, 0, s);
            let cells: Vec<[u32; 2]> = loot.kind.grids.iter().map(|grid| grid.cells).collect();
            let rects = crate::health_view::container_grids(look, &loot.kind.layout, &cells);
            // The grids' view as high as its own layout, else as its row
            // with the row's padding below.
            let bottom = rects.iter().fold(0.0f32, |bottom, [_, top, _, h]| bottom.max(top + h));
            let whole = match look.layouts.get(&loot.kind.layout).filter(|(_, own)| own.len() >= cells.len()) {
                Some((own, _)) => own[1],
                None => bottom + look.containers.as_ref().and_then(|views| views.grids_row).map_or(0.0, |row| row.padding[3]),
            };
            let shown = places.view[3];
            let most = (whole - shown).max(0.0);
            let scrolled = game_scrolls[CRATE].clamp(0.0, most);
            let [mx, my, mw, mh] = places.mask;
            view.spawn(Node { overflow: Overflow::clip(), ..placed(mx * s, my * s, mw * s, mh * s) }).with_children(|cut| {
                let [vx, vy, vw, vh] = places.view;
                let grids = rects.iter().zip(&cells).map(|(&rect, cells)| (rect, cells[1])).collect();
                let wheel = ScrollWheel { kind: CRATE, whole, shown, frame: [x, y], size, content_top: places.content[1] - my, grids };
                cut.spawn((wheel, placed((vx - mx) * s, (vy - my) * s, vw * s, vh * s), RelativeCursorPosition::default()));
                let content = placed((places.content[0] - mx) * s, (places.content[1] - my - scrolled) * s, mw * s, whole * s);
                cut.spawn((ScrollContent(CRATE), content)).with_children(|grids| {
                    for (grid, [gx, gy, gw, gh]) in rects.iter().copied().enumerate() {
                        crate::health_view::draw_grid_view(grids, look, shows.said, [gx, gy], [gw, gh], s);
                        grids.spawn(placed(gx * s, gy * s, gw * s, gh * s)).with_children(|holder| grid_box_drawn(holder, carried, loot, grid, cell, 0, false));
                    }
                });
            });
            // Its bar, while the grids are higher than it shows.
            if let (Some(([bx, by, bw, bh], bar)), Some(stash)) = (places.bar, look.stash.as_ref()) {
                if !(places.scrolling.hides && whole <= shown + 0.01) {
                    let color = stash.panel.nodes[bar.object].image.map_or(Color::BLACK, |image| crate::health_view::rgba(image.color));
                    view.spawn((ScrollBarNode(CRATE), placed(bx * s, by * s, bw * s, bh * s), BackgroundColor(color), RelativeCursorPosition::default()));
                    let value = if most > 0.0 { 1.0 - scrolled / most } else { 1.0 };
                    if let (Some([hx, hy, hw, hh]), Some(handle)) = (bar.handle_in(&stash.panel, size, value, (shown / whole).clamp(0.0, 1.0)), bar.handle) {
                        let color = stash.panel.nodes[handle].image.map_or(Color::WHITE, |image| crate::health_view::rgba(image.color));
                        view.spawn((ScrollHandle(CRATE, color), placed(hx * s, hy * s, hw * s, hh * s), BackgroundColor(color)));
                    }
                }
            }
            crate::health_view::draw_stash(view, look, shows.said, size, &name, &present, shows.stash_fills, 1, s);
        });
        return;
    }
    // What is open beside them: the crate, or nothing. Its grids are in a
    // panel as high as the screen leaves, scrolled by the wheel or by the
    // bar at its side when they are higher than that.
    let right = match game.filter(|_| tarkov.is_some()).and_then(|game| game.stash_in(shows.screen)) {
        // With `TARKOV UI`, where the game's panel for it is.
        Some([x, y, width, height]) => Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(6.0), ..placed(x * s, y * s, width * s, height * s) },
        None => Node { width: Val::Px(STASH_WIDTH * s), flex_direction: FlexDirection::Column, row_gap: Val::Px(6.0), flex_shrink: 0.0, ..default() },
    };
    gear.spawn(right).with_children(|view| {
        if !carried.loot_open {
            view.spawn(words("Nothing is open. Look at a container on the range from close by and press the key that works it.", 11.0, FAINT));
            return;
        }
        // A table of weapons' gun case or magazine box by its class: each
        // is one of the game's containers, which all its kind are named by.
        let caption = crate::range::weapon_crate_label(carried.loot_of).unwrap_or_else(|| carried.name(&carried.loot));
        view.spawn(words(caption.to_uppercase(), 11.0, FAINT));
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
            row.spawn((LootArea { shown, whole }, area, ScrollPosition(Vec2::new(0.0, scrolled)), RelativeCursorPosition::default())).with_children(|area| {
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
    carried: Res<Carried>,
    mut in_sight: ResMut<LootInSight>,
    wheel: Res<bevy::input::mouse::AccumulatedMouseScroll>,
    buttons: Res<ButtonInput<MouseButton>>,
    window: Query<&Window, With<PrimaryWindow>>,
    units: Res<Units>,
    mut scroll: ResMut<LootScroll>,
    mut area: Query<(&LootArea, &mut ScrollPosition, &RelativeCursorPosition)>,
    mut thumb: Query<(&LootThumb, &mut Node, &RelativeCursorPosition)>,
    mut held: Local<Option<f32>>,
    mut last_shown: Local<Option<usize>>,
) {
    // Each container opens at its top (the user, 2026-10-14, night: "The
    // scroll position of crates is shared between containers, make sure
    // containers always start at the top"): the screen opened on one, or
    // another one shown, scrolls the panel back up.
    let showing = (controller.inventory_open && carried.loot_open).then_some(carried.loot_of);
    if *last_shown != showing {
        *last_shown = showing;
        if scroll.0 != 0.0 {
            scroll.0 = 0.0;
        }
    }
    if !controller.inventory_open {
        *held = None;
        return;
    }
    let Ok((sizes, mut position, over)) = area.single_mut() else { return };
    let most = (sizes.whole - sizes.shown).max(0.0);
    let mut to = scroll.0;
    if over.cursor_over() && wheel.delta.y != 0.0 {
        to -= wheel.delta.y.signum() * units.cell();
    }
    // The thumb held: it follows the mouse, and the grids with it.
    let cursor = window.single().ok().and_then(|window| window.cursor_position()).map(|at| at.y);
    if let Ok((bar, _, over)) = thumb.single() {
        if buttons.just_pressed(MouseButton::Left) && over.cursor_over() {
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
    if position.y != to {
        position.y = to;
    }
    if let Ok((bar, mut node, _)) = thumb.single_mut() {
        let top = Val::Px(to / bar.whole * bar.shown);
        if node.top != top {
            node.top = top;
        }
    }
    // The rows in sight, grid by grid: the grids lie one under another,
    // `GRID_GAP` apart.
    let cell = units.cell().max(1.0);
    let (from, until) = (to, to + sizes.shown);
    let mut top = 0.0;
    let rows: Vec<[u32; 2]> = carried
        .loot
        .kind
        .grids
        .iter()
        .map(|grid| {
            let height = grid.cells[1] as f32;
            let first = ((from - top) / cell).floor().clamp(0.0, height);
            let end = ((until - top) / cell).ceil().clamp(first, height);
            top += height * cell + GRID_GAP;
            [first as u32, end as u32]
        })
        .collect();
    if in_sight.0 != rows {
        in_sight.0 = rows;
    }
}

/// The game's bar of tabs (`TARKOV UI`), drawn into its node as the game's
/// (`health_view::draw_tab_bar`): afresh as the tab chosen or the one under
/// the mouse changes, and with its buttons as the window's scale or width
/// does. A button over each tab, the later over the earlier where they
/// overlap, as the engine gives the mouse to what is drawn last. The
/// game's prestige tab is not worked in a raid (`Tab.SetInteractable`):
/// it is never lit under the mouse. Each tab's fill cut to its mask is
/// made at the window's scale (`health_view::tab_fill_images`).
#[allow(clippy::too_many_arguments)]
fn game_tabs(
    mut commands: Commands,
    look: Res<crate::health_view::HealthLook>,
    said: Res<crate::health_view::Words>,
    selected: Res<Selected>,
    window: Query<&Window, With<PrimaryWindow>>,
    mut images: ResMut<Assets<Image>>,
    bar: Query<Entity, With<GameTabBar>>,
    shown: Query<Entity, With<GameTabLook>>,
    buttons: Query<(&TabButton, &Interaction), With<GameTab>>,
    controller: Res<Controller>,
    mut drawn: Local<Option<(u32, u32, usize, Option<usize>)>>,
    (mut fills, mut filled): (Local<HashMap<usize, Handle<Image>>>, Local<Option<(u32, u32)>>),
) {
    let (Ok(bar), Ok(window)) = (bar.single(), window.single()) else { return };
    if window.height() <= 0.0 {
        return;
    }
    // Shut, the bar is not kept: Bevy lays out every node there is, shown
    // or not, each frame (`findings.md`). It is drawn whole as the screen
    // opens, its tabs' pictures kept while the window keeps its size.
    if !controller.inventory_open {
        if drawn.take().is_some() {
            commands.entity(bar).despawn_related::<Children>();
        }
        return;
    }
    let s = window.height() / SCREEN_HEIGHT;
    let screen = [window.width() / s, SCREEN_HEIGHT];
    // A tab not built is not lit under the mouse, as the game's prestige
    // tab, which does not work in a raid, is not.
    let hovered = buttons.iter().find(|(_, interaction)| **interaction != Interaction::None).map(|(tab, _)| tab.0).filter(|&index| TABS[index].1);
    let now = (s.to_bits(), screen[0].to_bits(), selected.0, hovered);
    if *drawn == Some(now) {
        return;
    }
    let whole = drawn.is_none_or(|was| (was.0, was.1) != (now.0, now.1));
    *drawn = Some(now);
    let draw = |look_node: &mut ChildSpawnerCommands, fills: &HashMap<usize, Handle<Image>>| {
        let dimmed: Vec<bool> = TABS.iter().map(|(_, built)| !built).collect();
        crate::health_view::draw_tab_bar(look_node, &look, &said, screen, Some(selected.0), hovered, TABS[OPTIONS].0, &dimmed, fills, s);
    };
    if *filled != Some((now.0, now.1)) {
        *filled = Some((now.0, now.1));
        for (_, old) in fills.drain() {
            images.remove(&old);
        }
        *fills = crate::health_view::tab_fill_images(&look, screen, s, &mut images);
    }
    if whole {
        let places = crate::health_view::tab_places(&look, screen);
        let fills = &*fills;
        commands.entity(bar).despawn_related::<Children>().with_children(|bar| {
            let whole = Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() };
            bar.spawn((GameTabLook, whole)).with_children(|look_node| draw(look_node, fills));
            for (index, [x, y, w, h]) in places.into_iter().enumerate().filter(|(index, _)| *index < TABS.len()) {
                bar.spawn((TabButton(index), GameTab, Button, placed(x * s, y * s, w * s, h * s)));
            }
            if let Some([x, y, w, h]) = crate::health_view::back_place(&look, screen) {
                bar.spawn((GameBack, Button, placed(x * s, y * s, w * s, h * s)));
            }
        });
        return;
    }
    if let Ok(look_node) = shown.single() {
        commands.entity(look_node).despawn_related::<Children>().with_children(|look_node| draw(look_node, &fills));
    }
}

/// How far the game's scroll views (`TARKOV UI`) are scrolled, in the
/// game's units from their tops, and how fast each springs back where it is
/// elastic: the containers' column (`COLUMN`), kept while the screen is shut
/// (the game's keeps its place: its `AutoZeroing` is off), and the panel of
/// a container opened (`CRATE`), at its top as another container opens (the
/// user, 2026-10-14, night: "make sure containers always start at the
/// top").
#[derive(Resource, Default)]
pub struct GameScrolls {
    at: [f32; 2],
    speed: [f32; 2],
    /// Each window that shows a thing's description, by its opening
    /// (`Win::opened`): at its top as it opens (a new window from the
    /// template), kept while it is open.
    info: HashMap<u64, f32>,
}
const COLUMN: usize = 0;
const CRATE: usize = 1;

/// A game's scroll view as drawn (`TARKOV UI`), where the wheel scrolls it:
/// which (`GameScrolls`), how high what scrolls is and how high it shows,
/// the corner on the screen of the panel its bar is placed in and that
/// panel's size, and the top of what moves, unscrolled, in the mask's
/// frame; all in units. With them its grids, each with its rows, from what
/// moves' corner, for what of them is in sight (`LootInSight`).
#[derive(Component)]
struct ScrollWheel {
    kind: usize,
    whole: f32,
    shown: f32,
    frame: [f32; 2],
    size: [f32; 2],
    content_top: f32,
    grids: Vec<([f32; 4], u32)>,
}
/// What of a scroll view moves as it is scrolled; its bar; its bar's
/// handle with the handle's own colour.
#[derive(Component)]
struct ScrollContent(usize);
#[derive(Component)]
struct ScrollBarNode(usize);
#[derive(Component)]
struct ScrollHandle(usize, Color);

/// A scroll bar held with the left button: where it was pressed (in the
/// units of the panel it is placed in), whether the mouse has moved far
/// enough for a drag and, once it has, where on the handle it was taken
/// (from the handle's middle, up), and whether it was pressed on this
/// frame.
#[derive(Clone, Copy)]
struct BarHeld {
    from: [f32; 2],
    dragging: Option<f32>,
    first: bool,
}

/// What the scroll views keep between frames: each one's bar held, chosen
/// and its handle's tint, and the container last opened.
#[derive(Default)]
struct ScrollState {
    held: [Option<BarHeld>; 2],
    chosen: [bool; 2],
    fade: [Fade; 2],
    last_crate: Option<usize>,
}

/// A tint changing over the bar's own time: from, to, how long since it
/// began, and the tint now.
#[derive(Clone, Copy, Default)]
struct Fade {
    from: [f32; 4],
    to: [f32; 4],
    since: f32,
    now: Option<[f32; 4]>,
}

/// The game's scroll views (`TARKOV UI`: the containers' column and the
/// panel of a container opened) are scrolled as the game's
/// (`ScrollRectNoDrag` and its `Scrollbar`): by the wheel over the view, so
/// many units a notch (`m_ScrollSensitivity`), while its bar shows
/// (`ScrollRectNoDrag.OnScroll`); by the bar's handle held and moved once
/// the mouse has gone as far as a drag takes, the handle kept under the
/// mouse where it was taken (`Scrollbar.OnBeginDrag`, `UpdateDrag`); and by
/// the bar held elsewhere, a handle's length toward where it was pressed on
/// every frame until the handle is under it, twice on the frame it is
/// pressed (`Scrollbar.ClickRepeat`, which waits for each frame's end and so
/// runs again at the end of the first). A view that stops at its ends
/// (`MovementType.Clamped`) is held to them; an elastic one is let past and
/// springs back over its elasticity, three times as long on a frame it was
/// scrolled (`ScrollRect.LateUpdate`, `Mathf.SmoothDamp`). The handle is
/// tinted as the bar is idle, under the mouse, held or chosen (pressed,
/// until something else is), the tint going over linearly in the bar's own
/// time (`Selectable.DoStateTransition`, `Graphic.CrossFadeColor`). The
/// rows of the opened container's grids in sight are kept for its pictures
/// (`LootInSight`, as the game's `GridViewMagnifier` makes the views in
/// sight alone).
#[allow(clippy::too_many_arguments)]
fn scroll_views(
    time: Res<Time>,
    controller: Res<Controller>,
    carried: Res<Carried>,
    look: Res<crate::health_view::HealthLook>,
    wheel: Res<bevy::input::mouse::AccumulatedMouseScroll>,
    buttons: Res<ButtonInput<MouseButton>>,
    window: Query<&Window, With<PrimaryWindow>>,
    units: Res<Units>,
    mut scroll: ResMut<GameScrolls>,
    mut in_sight: ResMut<LootInSight>,
    covers: Query<(&Cover, &RelativeCursorPosition)>,
    areas: Query<(&ScrollWheel, &RelativeCursorPosition)>,
    bars: Query<(&ScrollBarNode, &RelativeCursorPosition)>,
    mut contents: Query<(&ScrollContent, &mut Node), Without<ScrollHandle>>,
    mut handles: Query<(&ScrollHandle, &mut Node, &mut BackgroundColor), Without<ScrollContent>>,
    mut state: Local<ScrollState>,
) {
    let ScrollState { held, chosen, fade, last_crate } = &mut *state;
    let showing = (controller.inventory_open && carried.loot_open).then_some(carried.loot_of);
    if *last_crate != showing {
        *last_crate = showing;
        scroll.at[CRATE] = 0.0;
        scroll.speed[CRATE] = 0.0;
    }
    if !controller.inventory_open {
        *held = [None, None];
        return;
    }
    let s = units.0.max(1e-3);
    let on_page = cover_under(covers.iter()) == 0;
    let cursor_px = window.single().ok().and_then(|window| window.cursor_position());
    for (sizes, over_view) in &areas {
        let kind = sizes.kind;
        let source = match kind {
            COLUMN => look.containers.as_ref().and_then(|views| Some((views.scrolling?, &views.panel))),
            _ => look.stash.as_ref().and_then(|views| Some((views.scrolling?, &views.panel))),
        };
        let Some((scrolling, panel)) = source else { continue };
        let most = (sizes.whole - sizes.shown).max(0.0);
        let shows = !scrolling.hides || sizes.whole > sizes.shown + 0.01;
        let mut to = scroll.at[kind];
        let mut wheeled = false;
        if over_view.cursor_over() && on_page && shows && scrolling.vertical {
            // A notch reads one (`docs/findings.md`); the engine's scroll
            // event is down for positive, the content's place up.
            let notches = match wheel.unit {
                bevy::input::mouse::MouseScrollUnit::Line => wheel.delta.y,
                bevy::input::mouse::MouseScrollUnit::Pixel => wheel.delta.y.signum(),
            };
            if notches != 0.0 {
                to -= notches * scrolling.sensitivity;
                wheeled = true;
            }
        }
        let clamp = |at: f32| if scrolling.clamped { at.clamp(0.0, most) } else { at };
        to = clamp(to);
        let share = if sizes.whole > 0.0 { (sizes.shown / sizes.whole).clamp(0.0, 1.0) } else { 1.0 };
        let value_of = |at: f32| if most > 0.0 { 1.0 - at / most } else { 1.0 };
        let at_of = |value: f32| (1.0 - value.clamp(0.0, 1.0)) * most;
        let cursor = cursor_px.map(|at| [at.x / s - sizes.frame[0], at.y / s - sizes.frame[1]]);
        let over_bar = bars.iter().any(|(bar, over)| bar.0 == kind && over.cursor_over()) && on_page;
        if let Some(bar_look) = scrolling.bar.filter(|_| shows) {
            let handle_at = |at: f32| bar_look.handle_in(panel, sizes.size, value_of(at), share);
            if buttons.just_pressed(MouseButton::Left) {
                // A press elsewhere chooses something else, or nothing.
                chosen[kind] = over_bar;
                held[kind] = cursor.filter(|_| over_bar).map(|from| BarHeld { from, dragging: None, first: true });
            }
            if !buttons.pressed(MouseButton::Left) {
                held[kind] = None;
            }
            let sliding = bar_look.sliding.map(|sliding| panel.rect(sliding, sizes.size));
            if let (Some(mut press), Some(cursor), Some([_, sy, _, sh])) = (held[kind], cursor, sliding) {
                // A drag begins once the mouse is so far from where it was
                // pressed, the handle taken where the press was on it, or
                // by its middle.
                let moved = Vec2::new(cursor[0] - press.from[0], cursor[1] - press.from[1]).length() * s;
                if press.dragging.is_none() && moved >= DRAG_THRESHOLD {
                    let offset = handle_at(to).filter(|&rect| inside(rect, press.from)).map_or(0.0, |[_, hy, _, hh]| (hy + hh / 2.0) - press.from[1]);
                    press.dragging = Some(offset);
                }
                match press.dragging {
                    Some(offset) => {
                        if let Some(value) = dragged_to([sy, sh], share, offset, cursor[1], bar_look.bottom_up) {
                            to = clamp(at_of(value));
                        }
                    }
                    None => {
                        for _ in 0..if press.first { 2 } else { 1 } {
                            let Some(value) = handle_at(to).and_then(|handle| stepped(value_of(to), share, handle, press.from, bar_look.bottom_up)) else { break };
                            to = clamp(at_of(value));
                        }
                    }
                }
                press.first = false;
                held[kind] = Some(press);
            }
            // The handle where the bar puts it, and tinted.
            let tints = bar_look.tints.filter(|_| bar_look.tinted == bar_look.handle);
            let wanted = tints.map(|tints| {
                let tint = if held[kind].is_some() {
                    tints.pressed
                } else if chosen[kind] {
                    tints.selected
                } else if over_bar {
                    tints.highlighted
                } else {
                    tints.normal
                };
                (tint.map(|channel| channel * tints.multiplier), tints.fade)
            });
            let fade = &mut fade[kind];
            if let Some((wanted, length)) = wanted {
                let now = fade.now.unwrap_or(wanted);
                if fade.now.is_none() || fade.to != wanted {
                    *fade = Fade { from: now, to: wanted, since: 0.0, now: Some(now) };
                }
                fade.since += time.delta_secs();
                let share = if length > 0.0 { (fade.since / length).min(1.0) } else { 1.0 };
                let (from, target) = (fade.from, fade.to);
                fade.now = Some(std::array::from_fn(|channel| from[channel] + (target[channel] - from[channel]) * share));
            }
            for (own, mut node, mut color) in handles.iter_mut().filter(|(own, ..)| own.0 == kind) {
                let Some([hx, hy, hw, hh]) = handle_at(to) else { continue };
                let place = [Val::Px(hx * s), Val::Px(hy * s), Val::Px(hw * s), Val::Px(hh * s)];
                if [node.left, node.top, node.width, node.height] != place {
                    [node.left, node.top, node.width, node.height] = place;
                }
                let tint = fade.now.unwrap_or([1.0; 4]);
                let base = own.1.to_srgba();
                let tinted = Color::srgba(base.red * tint[0], base.green * tint[1], base.blue * tint[2], base.alpha * tint[3]);
                if color.0 != tinted {
                    color.0 = tinted;
                }
            }
        } else {
            held[kind] = None;
        }
        // Back from past an end, where it is elastic.
        if scrolling.elastic {
            let target = to.clamp(0.0, most);
            let mut speed = scroll.speed[kind];
            if target != to {
                let length = scrolling.elasticity * if wheeled { 3.0 } else { 1.0 };
                to = smooth_damp(to, target, &mut speed, length, time.delta_secs());
                if speed.abs() < 1.0 {
                    speed = 0.0;
                }
            } else {
                speed = 0.0;
            }
            scroll.speed[kind] = speed;
        }
        if scroll.at[kind] != to {
            scroll.at[kind] = to;
        }
        for (_, mut node) in contents.iter_mut().filter(|(own, _)| own.0 == kind) {
            let top = Val::Px((sizes.content_top - to) * s);
            if node.top != top {
                node.top = top;
            }
        }
        // The opened container's rows in sight, grid by grid.
        if kind == CRATE {
            let (from, until) = (to, to + sizes.shown);
            let rows: Vec<[u32; 2]> = sizes
                .grids
                .iter()
                .map(|&([_, top, _, height], rows)| {
                    let row = height / rows.max(1) as f32;
                    let first = ((from - top) / row).floor().clamp(0.0, rows as f32);
                    let end = ((until - top) / row).ceil().clamp(first, rows as f32);
                    [first as u32, end as u32]
                })
                .collect();
            if in_sight.0 != rows {
                in_sight.0 = rows;
            }
        }
    }
}

/// A window that shows a thing: its description's scroll view's area, with
/// the window's opening and layer, and the view as built; and its bar.
#[derive(Component)]
struct InfoScrollArea {
    opened: u64,
    layer: u8,
    scroll: crate::health_view::InfoScroll,
}
#[derive(Component)]
struct InfoScrollBar(u64);

/// The bar held, whether it is chosen, and its handle's tint, of each
/// window's description, by its opening.
#[derive(Default)]
struct InfoScrollState {
    held: Option<BarHeld>,
    chosen: bool,
    fade: Fade,
}

/// The windows' descriptions are scrolled as the game's
/// (`DescriptionPanel/Scroll Area`: `ScrollRectNoDrag`, inspect-window
/// spec, "The description"), as `scroll_views` scrolls the screen's own: by
/// the wheel over the area on the window on top there, so many units a
/// notch (10), only while the bar shows; by the handle dragged; by the bar
/// held off it, a handle's length toward the press each frame. Elastic
/// with an elasticity of nought, it is back within its ends on the same
/// frame (`SmoothDamp` over nought), so it stops at them; no inertia. The
/// handle is tinted as the bar's state is. A new window begins at the top;
/// a shut one's place is let go.
#[allow(clippy::too_many_arguments)]
fn scroll_info_texts(
    time: Res<Time>,
    controller: Res<Controller>,
    carried: Res<Carried>,
    wheel: Res<bevy::input::mouse::AccumulatedMouseScroll>,
    buttons: Res<ButtonInput<MouseButton>>,
    units: Res<Units>,
    mut scroll: ResMut<GameScrolls>,
    (covers, areas, bars): (Query<(&Cover, &RelativeCursorPosition)>, Query<(&InfoScrollArea, &RelativeCursorPosition)>, Query<(&InfoScrollBar, &RelativeCursorPosition)>),
    mut contents: Query<(&crate::health_view::InfoScrollContent, &mut Node), Without<crate::health_view::InfoScrollHandle>>,
    mut handles: Query<(&crate::health_view::InfoScrollHandle, &mut Node, &mut BackgroundColor), Without<crate::health_view::InfoScrollContent>>,
    mut state: Local<HashMap<u64, InfoScrollState>>,
) {
    let open = |opened: &u64| carried.windows.iter().any(|window| window.opened == *opened);
    scroll.info.retain(|opened, _| open(opened));
    state.retain(|opened, _| open(opened));
    if !controller.inventory_open {
        state.values_mut().for_each(|each| each.held = None);
        return;
    }
    let s = units.0.max(1e-3);
    let top = cover_under(covers.iter());
    for (area, over_view) in &areas {
        let (opened, info) = (area.opened, &area.scroll);
        let InfoScrollState { held, chosen, fade } = state.entry(opened).or_default();
        let on_top = top == area.layer;
        let mut to = scroll.info.get(&opened).copied().unwrap_or(0.0);
        if over_view.cursor_over() && on_top && info.vertical {
            let notches = match wheel.unit {
                bevy::input::mouse::MouseScrollUnit::Line => wheel.delta.y,
                bevy::input::mouse::MouseScrollUnit::Pixel => wheel.delta.y.signum(),
            };
            to -= notches * info.sensitivity;
        }
        let most = info.most();
        to = to.clamp(0.0, most);
        let share = info.share();
        // The mouse in the window's units, by where it is on the bar.
        let on_bar = bars.iter().find(|(bar, _)| bar.0 == opened).map(|(_, at)| at);
        let over_bar = on_top && on_bar.is_some_and(|at| at.cursor_over());
        let cursor = on_bar.and_then(share_of).map(|share| [info.bar[0] + share.x * info.bar[2], info.bar[1] + share.y * info.bar[3]]);
        if buttons.just_pressed(MouseButton::Left) {
            *chosen = over_bar;
            *held = cursor.filter(|_| over_bar).map(|from| BarHeld { from, dragging: None, first: true });
        }
        if !buttons.pressed(MouseButton::Left) {
            *held = None;
        }
        if let (Some(mut press), Some(cursor)) = (*held, cursor) {
            let [_, sy, _, sh] = info.sliding;
            let moved = Vec2::new(cursor[0] - press.from[0], cursor[1] - press.from[1]).length() * s;
            if press.dragging.is_none() && moved >= DRAG_THRESHOLD {
                let rect = info.handle_at(to);
                let offset = if inside(rect, press.from) { (rect[1] + rect[3] / 2.0) - press.from[1] } else { 0.0 };
                press.dragging = Some(offset);
            }
            match press.dragging {
                Some(offset) => {
                    if let Some(value) = dragged_to([sy, sh], share, offset, cursor[1], info.bottom_up()) {
                        to = info.at_of(value).clamp(0.0, most);
                    }
                }
                None => {
                    for _ in 0..if press.first { 2 } else { 1 } {
                        let Some(value) = stepped(info.value_of(to), share, info.handle_at(to), press.from, info.bottom_up()) else { break };
                        to = info.at_of(value).clamp(0.0, most);
                    }
                }
            }
            press.first = false;
            *held = Some(press);
        }
        // The handle tinted as the bar is idle, under the mouse, held or
        // chosen, over the tint's own time.
        if let Some(tints) = info.tints {
            let tint = if held.is_some() {
                tints.pressed
            } else if *chosen {
                tints.selected
            } else if over_bar {
                tints.highlighted
            } else {
                tints.normal
            };
            let wanted = tint.map(|channel| channel * tints.multiplier);
            let now = fade.now.unwrap_or(wanted);
            if fade.now.is_none() || fade.to != wanted {
                *fade = Fade { from: now, to: wanted, since: 0.0, now: Some(now) };
            }
            fade.since += time.delta_secs();
            let share = if tints.fade > 0.0 { (fade.since / tints.fade).min(1.0) } else { 1.0 };
            let (from, target) = (fade.from, fade.to);
            fade.now = Some(std::array::from_fn(|channel| from[channel] + (target[channel] - from[channel]) * share));
        }
        if scroll.info.get(&opened) != Some(&to) {
            scroll.info.insert(opened, to);
        }
        for (own, mut node, mut color) in handles.iter_mut().filter(|(own, ..)| own.0 == opened) {
            let [hx, hy, hw, hh] = info.handle_at(to);
            let place = [Val::Px(hx * s), Val::Px(hy * s), Val::Px(hw * s), Val::Px(hh * s)];
            if [node.left, node.top, node.width, node.height] != place {
                [node.left, node.top, node.width, node.height] = place;
            }
            let tint = fade.now.unwrap_or([1.0; 4]);
            let base = own.1.to_srgba();
            let tinted = Color::srgba(base.red * tint[0], base.green * tint[1], base.blue * tint[2], base.alpha * tint[3]);
            if color.0 != tinted {
                color.0 = tinted;
            }
        }
        for (_, mut node) in contents.iter_mut().filter(|(own, _)| own.0 == opened) {
            let top = Val::Px(-to * s);
            if node.top != top {
                node.top = top;
            }
        }
    }
}

// ==== Pointing: what lies under the mouse, foresight, what goes with a thing lit ====

/// Unity's `Mathf.SmoothDamp` (its published source): a value eased
/// toward a target as a critically damped spring reaching it in about so
/// long (at least a ten-thousandth of a second), with its speed kept
/// between frames; never past the target.
pub(crate) fn smooth_damp(current: f32, target: f32, speed: &mut f32, length: f32, step: f32) -> f32 {
    let length = length.max(0.0001);
    let omega = 2.0 / length;
    let x = omega * step;
    let decay = 1.0 / (1.0 + x + 0.48 * x * x + 0.235 * x * x * x);
    let change = current - target;
    let temp = (*speed + omega * change) * step;
    *speed = (*speed - omega * temp) * decay;
    let mut output = target + (change + temp) * decay;
    if (target - current > 0.0) == (output > target) {
        output = target;
        *speed = if step > 0.0 { (output - target) / step } else { 0.0 };
    }
    output
}

/// Whether a point lies in a rectangle (left, top, width, height).
fn inside(rect: [f32; 4], point: [f32; 2]) -> bool {
    (rect[0]..rect[0] + rect[2]).contains(&point[0]) && (rect[1]..rect[1] + rect[3]).contains(&point[1])
}

/// A scroll bar's value with its handle dragged to the mouse
/// (`Scrollbar.UpdateDrag`): the handle's share of its sliding area kept
/// where the mouse took it, `offset` up from its middle; `sliding` is the
/// sliding area's top and height, `cursor` the mouse's height, y down.
/// None where the handle fills the area.
fn dragged_to(sliding: [f32; 2], share: f32, offset: f32, cursor: f32, bottom_up: bool) -> Option<f32> {
    let [top, height] = sliding;
    let remaining = height * (1.0 - share);
    if remaining <= 0.0 {
        return None;
    }
    let corner = (top + height - cursor) - offset - share * height / 2.0;
    let value = (corner / remaining).clamp(0.0, 1.0);
    Some(if bottom_up { value } else { 1.0 - value })
}

/// A scroll bar's value after one step of the bar held off its handle
/// (`Scrollbar.ClickRepeat`): a handle's share toward where it was
/// pressed, kept to four places. None once the handle is under the press.
fn stepped(value: f32, share: f32, handle: [f32; 4], press: [f32; 2], bottom_up: bool) -> Option<f32> {
    if inside(handle, press) {
        return None;
    }
    // Where the press is from the handle's middle, up.
    let up = (handle[1] + handle[3] / 2.0) - press[1];
    let change = if up < 0.0 { share } else { -share };
    let value = value + if bottom_up { -change } else { change };
    Some((value.clamp(0.0, 1.0) * 10000.0).round() / 10000.0)
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
    (scroll, scrolls): (Res<LootScroll>, Res<GameScrolls>),
    window: Query<&Window, With<PrimaryWindow>>,
    mut units: ResMut<Units>,
    page: Query<Entity, With<GearPage>>,
    over: Query<Entity, With<Overlay>>,
    figure: Res<crate::figure_view::FigureImage>,
    look: Res<crate::health_view::HealthLook>,
    said: Res<crate::health_view::Words>,
    stash_fills: Res<StashFills>,
    (game, previews, held): (Res<crate::GameData>, Res<crate::info_preview::InfoPreviews>, Res<HandsHeld>),
    mut kept: Local<((bool, usize), u32)>,
) {
    let _clock = crate::render_timing::Clock::new("inventory rebuild");
    let (was, afresh) = &mut *kept;
    let (now, size) = window.single().map_or((*units, Vec2::ZERO), |window| (Units(window.height() / SCREEN_HEIGHT), Vec2::new(window.width(), window.height())));
    let shows = (controller.inventory_open, selected.0);
    // Nothing is drawn while the screen is shut: what lies over it goes as
    // it shuts, and the page is drawn afresh as it opens, whatever changed
    // meanwhile (a reload's magazines, a thing taken up).
    if !controller.inventory_open {
        if *was != shows {
            for entity in &over {
                commands.entity(entity).despawn();
            }
            *was = shows;
        }
        *afresh = 0;
        return;
    }
    if !carried.is_changed() && now == *units && shows == *was && !figure.is_changed() && !look.is_changed() && !stash_fills.is_changed() && !previews.is_changed() {
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
    let left = LeftPanel {
        tab: shows.1,
        look: &look,
        said: &said,
        health: controller.health.as_ref(),
        screen: if size.y > 0.0 { [size.x / now.0, SCREEN_HEIGHT] } else { [SCREEN_HEIGHT * 16.0 / 9.0, SCREEN_HEIGHT] },
        stash_fills: &stash_fills.images,
    };
    for page in &page {
        commands.entity(page).despawn_related::<Children>().with_children(|gear| gear_page(gear, &carried, now, scroll.0, scrolls.at, figure.0.as_ref(), &left));
    }
    for entity in &over {
        commands.entity(entity).despawn();
    }
    // What lies over the page is the page's on both its tabs: the
    // quick-use bar, the windows, the window that asks how many.
    if !(shows.0 && (shows.1 == GEAR || shows.1 == HEALTH)) {
        return;
    }
    quick_bar(&mut commands, &carried, &bindings, now, size, &look, &said);
    for (index, window) in carried.windows.iter().enumerate() {
        let scrolled = scrolls.info.get(&window.opened).copied().unwrap_or(0.0);
        window_box(&mut commands, &carried, *window, index, now, &look, &said, (&game, &previews, &held, controller.health.as_ref()), scrolled);
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
    mut buttons: MessageWriter<bevy::input::mouse::MouseButtonInput>,
    mut typed: MessageWriter<bevy::input::keyboard::KeyboardInput>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut controller: ResMut<Controller>,
    mut wheel: MessageWriter<bevy::input::mouse::MouseWheel>,
    mut mouse: ResMut<crate::keys::ScriptedMouse>,
    mut frame: Local<u32>,
    mut notches_left: Local<i32>,
) {
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
    // The scope zoom's turns (`--zoom`): the simulation's next frame.
    for (_, notches) in options.zoom.iter().filter(|(at, _)| *frame == *at) {
        controller.zoom_notches += notches;
    }
    for (_, notches) in options.wheel.iter().filter(|(at, _)| *frame == *at) {
        if controller.inventory_open || controller.edit_build {
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
        // A letter, a figure, the space, Backspace or Enter pressed is also
        // what it types, as a keyboard's key is (for a tag's field).
        if let (true, crate::keys::Key::Board(code)) = (down, key) {
            use bevy::input::keyboard::{Key, KeyboardInput};
            let debug = format!("{code:?}");
            let letter = debug.strip_prefix("Key").or_else(|| debug.strip_prefix("Digit")).filter(|rest| rest.len() == 1).map(str::to_lowercase).or_else(|| (*code == KeyCode::Space).then(|| " ".to_owned()));
            let logical = match (code, &letter) {
                (KeyCode::Backspace, _) => Some(Key::Backspace),
                (KeyCode::Enter, _) => Some(Key::Enter),
                (_, Some(letter)) => Some(Key::Character(letter.as_str().into())),
                _ => None,
            };
            if let Some(logical_key) = logical {
                typed.write(KeyboardInput { key_code: *code, logical_key, state: bevy::input::ButtonState::Pressed, text: letter.map(|letter| letter.as_str().into()), repeat: false, window: entity });
            }
        }
        match key {
            crate::keys::Key::Board(key) if down => keys.press(*key),
            crate::keys::Key::Board(key) if up => keys.release(*key),
            crate::keys::Key::Mouse(pressed) if down => button(*pressed, bevy::input::ButtonState::Pressed),
            crate::keys::Key::Mouse(pressed) if up => button(*pressed, bevy::input::ButtonState::Released),
            _ => {}
        }
    }
    // A move of the mouse with no button (`--move`).
    if let Some(&(_, x, y)) = options.moves.iter().find(|(at, ..)| *at == *frame) {
        window.set_cursor_position(Some(Vec2::new(x, y)));
    }
    let Some(since) = frame.checked_sub(options.drag_at) else { return };
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
pub(crate) fn cover_under<'a>(covers: impl Iterator<Item = (&'a Cover, &'a RelativeCursorPosition)>) -> u8 {
    covers.filter(|(_, at)| at.cursor_over()).map(|(cover, _)| cover.0).max().unwrap_or(0)
}

/// The thing under the mouse on what is drawn on top there: the smallest
/// is the one meant. With it, where in it the mouse is, as a share of it.
/// A box that blinks is not there to the mouse (`ItemView.IsInteractive`).
fn item_under<'a>(items: impl Iterator<Item = (&'a ItemView, &'a RelativeCursorPosition, &'a ComputedNode)>, cover: u8, but: Option<Id>, blinking: &Blinking) -> Option<(Id, Vec2)> {
    let over = items.filter(|(view, at, _)| view.1 == cover && Some(view.0) != but && at.cursor_over() && !blinking.0.contains(&view.0));
    over.min_by(|a, b| a.2.size().length_squared().total_cmp(&b.2.size().length_squared())).map(|(view, at, _)| (view.0, share_of(at).unwrap_or_default()))
}

/// How long the mouse rests on a weapon before its rig is made ready
/// ahead of its being hung, in seconds. `ESTIMATE`: the app's choice.
const RESTS_ON: f32 = 0.25;

/// A weapon the mouse rests on, or one taken up, has its rig made ready
/// before it is hung, so that the hands do not wait for it then: once no
/// weapon hung waits for one, and one at a time (`weapon::Preparing::
/// ask_ahead`). One that is not hung within a while is let go of again
/// (`weapon::free_rigs`); it is asked for once a rest, so that a mouse
/// left on it does not have it made again and again.
#[allow(clippy::too_many_arguments)]
fn foresee(
    time: Res<Time>,
    controller: Res<Controller>,
    carried: Res<Carried>,
    drag: Res<Drag>,
    blinking: Res<Blinking>,
    covers: Query<(&Cover, &RelativeCursorPosition)>,
    views: Query<(&ItemView, &RelativeCursorPosition, &ComputedNode)>,
    preparing: Option<ResMut<crate::weapon::Preparing>>,
    stowed: Option<Res<Stowed>>,
    mut resting: Local<Option<(Id, f32, bool)>>,
) {
    let (Some(mut preparing), Some(stowed)) = (preparing, stowed) else { return };
    if !controller.inventory_open {
        *resting = None;
        return;
    }
    let (pointed, taken) = match &drag.0 {
        Some(held) => (Some(held.id), true),
        None => (item_under(views.iter(), cover_under(covers.iter()), None, &blinking).map(|(id, _)| id), false),
    };
    let now = time.elapsed_secs();
    let Some(pointed) = pointed else {
        *resting = None;
        return;
    };
    let (since, asked) = match *resting {
        Some((id, since, asked)) if id == pointed => (since, asked),
        _ => {
            *resting = Some((pointed, now, false));
            (now, false)
        }
    };
    if asked || (!taken && now - since < RESTS_ON) {
        return;
    }
    *resting = Some((pointed, since, true));
    let Some(item) = carried.find(pointed).filter(|item| trp_sim::inventory::is(item, "Weapon")) else { return };
    let template = &trp_sim::inventory::rig_key(item);
    if !stowed.has_rig(template) && stowed.awaited(template) {
        preparing.ask_ahead(template);
    }
}

/// The box under the mouse in the game's own colour for it, and the boxes
/// of the things that go with it in orange (inventory spec, "The box under
/// the mouse, and what goes with it"); while a thing is carried, what goes
/// with that. The orange only with `COMPATIBLE ITEMS HIGHLIGHT` on (settings
/// spec, "Features"). Worked out again only when what is pointed at
/// changes or the screen has been drawn afresh, and only a box whose
/// colour changes is written, so that nothing else is drawn anew.
fn light_related(
    controller: Res<Controller>,
    features: Res<crate::options_view::Features>,
    carried: Res<Carried>,
    drag: Res<Drag>,
    blinking: Res<Blinking>,
    covers: Query<(&Cover, &RelativeCursorPosition)>,
    mut views: Query<(&ItemView, &RelativeCursorPosition, &ComputedNode, &mut BackgroundColor, &mut BorderColor, Option<&Ground>)>,
    fresh: Query<(), Added<ItemView>>,
    mut lit_for: Local<Option<(Option<Id>, bool)>>,
) {
    let _clock = crate::render_timing::Clock::new("inventory light_related");
    if !controller.inventory_open {
        *lit_for = None;
        return;
    }
    let pointed = match &drag.0 {
        Some(held) => Some(held.id),
        // On the frame the page is drawn afresh its boxes do not yet know
        // where the mouse is: what was pointed at stays lit, or every box
        // the mouse lights goes dark for that frame (a blink).
        None if !fresh.is_empty() => lit_for.and_then(|(pointed, _)| pointed),
        None => item_under(views.iter().map(|(view, at, node, ..)| (view, at, node)), cover_under(covers.iter()), None, &blinking).map(|(id, _)| id),
    };
    // (A thing pointed at and then taken is the same thing: its own box
    // goes back to its colour.)
    let now = (pointed, drag.0.is_some());
    if *lit_for == Some(now) && fresh.is_empty() {
        return;
    }
    *lit_for = Some(now);
    // Every thing on the screen by its number, once.
    let mut things = HashMap::new();
    fn index<'a>(item: &'a Item, into: &mut HashMap<Id, &'a Item>) {
        into.insert(item.id, item);
        item.contents().for_each(|inside| index(inside, into));
    }
    index(&carried.equipment.item, &mut things);
    index(&carried.loot, &mut things);
    let source = pointed.and_then(|id| things.get(&id).copied());
    let over = |(tint, share): (Srgba, f32)| Color::Srgba(THING.to_srgba().mix(&tint, share));
    for (view, _, _, mut back, mut border, ground) in &mut views {
        let (want_back, want_border) = match (source, ground) {
            // The game's own view: its ground, white under the mouse
            // (`ItemView.UpdateColor`), as the game's own colours go.
            (Some(source), Some(ground)) if source.id == view.0 => (if drag.0.is_some() { ground.0 } else { crate::health_view::ITEM_SELECTED }, ground.1),
            (Some(source), Some(_)) if features.compatible_highlight && things.get(&view.0).is_some_and(|thing| go_together(source, thing)) => (Color::Srgba(GOES_WITH.0.with_alpha(GOES_WITH.1)), Color::Srgba(GOES_WITH.0)),
            (_, Some(ground)) => (ground.0, ground.1),
            (Some(source), None) if source.id == view.0 => (if drag.0.is_some() { THING } else { over(POINTED) }, LINE),
            (Some(source), None) if features.compatible_highlight && things.get(&view.0).is_some_and(|thing| go_together(source, thing)) => (over(GOES_WITH), Color::Srgba(GOES_WITH.0)),
            _ => (THING, LINE),
        };
        if back.0 != want_back {
            back.0 = want_back;
        }
        if *border != BorderColor::all(want_border) {
            *border = BorderColor::all(want_border);
        }
    }
}

// ==== Using a thing, the menu of a right click, clicks ====

/// How a thing is used: by a double click ("Use", where that cannot be
/// "Use All"), or by the menu's "Use" or "Use All".
#[derive(Clone, Copy, PartialEq)]
enum Using {
    DoubleClick,
    Use,
    All,
}

/// Medicine, food or drink used from the screen (`ItemView.OnClick`, the
/// install's `ItemQuickUseMode` letting a double click; the menu's lines):
/// a double click is "Use", and where that cannot be, "Use All"
/// (`ItemContextInteractionsSwitcher`). "Use" is medicine on the part the
/// inventory names (`Health::use_part`), and for food or drink whose whole
/// is above 1 the window that asks how much (`ShowSplitMenuDialog`); "Use
/// All" is food or drink whole. Either only if it can do anything now
/// (`HasPartsToApply`), else the refusal is said. Whether the thing was
/// one of those (a double click on one is never taken as anything else).
fn use_thing(carried: &mut Carried, id: Id, health: Option<&trp_sim::health::Health>, db: &trp_data::Database, cursor: Vec2, asked: &mut AskedFor, using: Using) -> bool {
    use trp_sim::health::Refused;
    let Some(item) = carried.find(id) else { return false };
    if !(is(item, "FoodDrink") || is(item, "Meds")) {
        return false;
    }
    let (name, template, held) = (carried.name(item), item.kind.template.clone(), item.resource);
    let owned = carried.equipment.item.find(id).is_some();
    let medicine = trp_sim::health::Medicine::of(db, &template);
    carried.note = match (medicine, health) {
        (_, None) => format!("{name}: with health switched off nothing is used"),
        (None, _) => format!("{name}: its figures were not read"),
        (Some(_), _) if !owned => format!("{name}: only what the character has is used here"),
        (Some(medicine), Some(health)) => {
            let resource = held.unwrap_or_else(|| medicine.full());
            let share = medicine.food() && medicine.max_resource > 1.0 && using != Using::All;
            match health.has_parts_to_apply(&medicine, resource) {
                Err(Refused::Dead) => format!("{name}: the dead use nothing"),
                Err(Refused::Incompatible) => format!("{name}: it can do nothing now"),
                Err(Refused::Depleted) => format!("{name}: nothing is left in it"),
                Ok(()) if share => {
                    carried.asking = Asking::of_use(id, medicine.max_resource, resource, cursor);
                    String::new()
                }
                Ok(()) if medicine.food() => {
                    asked.1 = Some((id, None, None));
                    String::new()
                }
                Ok(()) => {
                    asked.1 = Some((id, health.use_part(&medicine, resource), None));
                    String::new()
                }
            }
        }
    };
    true
}

/// The thing put down on the ground a step ahead of where the eyes are
/// (the game's `ThrowItem`, which a raid carries out without asking; the
/// game throws the thing and it falls where it falls). A backpack on the
/// back is not put down at once: its slot is one the game animates, and
/// it goes by the body's gesture for it, as it does by its own key
/// (interactions spec, "A backpack dropped").
fn put_down(carried: &mut Carried, id: Id, eye: Option<&GlobalTransform>, floor: f32) {
    if carried.equipment.worn("Backpack").is_some_and(|pack| pack.id == id) {
        carried.pack_asked = true;
        return;
    }
    let Some(eye) = eye else { return };
    let ahead = eye.forward().as_vec3().with_y(0.0).normalize_or_zero();
    let at = Vec3::new(eye.translation().x, floor, eye.translation().z) + ahead * DROP_AHEAD;
    let name = carried.find(id).map(|item| carried.name(item)).unwrap_or_default();
    match take_out(&mut [&mut carried.equipment.item, &mut carried.loot], id, &carried.rules) {
        Ok(item) => {
            carried.ground.push((item, at));
            carried.thrown.push((id, crate::loose::How::Thrown));
            carried.note = format!("{name}: put down");
        }
        Err(refusal) => carried.note = format!("{name}: {}", why(&refusal)),
    }
    carried.tidy();
}

/// A thing's menu (`trp_sim::item_menu`): its lines in a raid by what the
/// thing is, where it lies, what the hands hold and what health says, each
/// with its caption in the install's words.
fn menu_lines(carried: &Carried, id: Id, held: &HandsHeld, health: Option<&trp_sim::health::Health>, db: &trp_data::Database, said: &crate::health_view::Words) -> Vec<(trp_sim::item_menu::Line, trp_sim::item_menu::Can, String)> {
    let Some(item) = carried.find(id) else { return Vec::new() };
    let equipment = carried.equipment.item.id;
    let place = carried.lies(id);
    let in_hands = held.item.and_then(|weapon| carried.find(weapon));
    let magazine_in_hands = in_hands.and_then(|weapon| weapon.kind.slots.iter().zip(&weapon.slots).find(|(slot, _)| slot.name == "mod_magazine").and_then(|(_, held)| held.as_ref()).map(|magazine| magazine.id));
    let health = health.map(|health| match trp_sim::health::Medicine::of(db, &item.kind.template) {
        Some(medicine) => health.has_parts_to_apply(&medicine, item.resource.unwrap_or_else(|| medicine.full())).map_err(trp_sim::item_menu::refusal_key),
        None => Err(trp_sim::item_menu::refusal_key(trp_sim::health::Refused::Incompatible)),
    });
    let facts = trp_sim::item_menu::Facts {
        worn: matches!(place, Some((holder, Spot::Slot(_))) if holder == equipment),
        fitted: matches!(place, Some((holder, Spot::Slot(_))) if holder != equipment),
        own: carried.equipment.item.find(id).is_some(),
        something_in_hands: held.item.is_some(),
        magazine_in_hands,
        checked: carried.checked.contains(&id),
        // Whether a fold would be refused where it lies, tried on a copy.
        fold_refused: trp_sim::inventory::can_fold(item) && {
            let (mut worn, mut loot) = (carried.equipment.item.clone(), carried.loot.clone());
            trp_sim::inventory::fold(&mut [&mut worn, &mut loot], id).is_err()
        },
        health,
    };
    trp_sim::item_menu::lines(item, &facts).into_iter().map(|(line, can)| (line, can, said.word(line.name(), line.name()))).collect()
}

/// What a line of the menu does, of those a raid shows (inventory spec,
/// "The menu of a right click"): Inspect opens the window that shows the
/// thing, Open its own window, Equip puts it on (as Alt and a click),
/// Unequip takes it off to the first place found for it (`Uninstall`:
/// `QuickFindAppropriatePlace`, as Ctrl and a click), Use and Use All use
/// it, Discard puts it down; the rest are not built, and say so.
#[allow(clippy::too_many_arguments)]
fn do_line(
    carried: &mut Carried,
    id: Id,
    line: trp_sim::item_menu::Line,
    caption: &str,
    cursor: Vec2,
    screen: Option<(Vec2, f32)>,
    health: Option<&trp_sim::health::Health>,
    db: &trp_data::Database,
    asked: &mut AskedFor,
    eye: Option<&GlobalTransform>,
    floor: f32,
    in_hands: Option<Id>,
) -> Option<Sound> {
    use trp_sim::item_menu::Line;
    match line {
        Line::Inspect => open_view(carried, id, false, cursor, screen),
        Line::Open => open_view(carried, id, true, cursor, screen),
        Line::Equip => click(carried, id, false, [false, false, true], cursor, screen),
        Line::Unequip => click(carried, id, false, [true, false, false], cursor, screen),
        Line::Use | Line::UseAll => {
            use_thing(carried, id, health, db, cursor, asked, if line == Line::Use { Using::Use } else { Using::All });
            None
        }
        Line::Discard => {
            put_down(carried, id, eye, floor);
            None
        }
        // A weapon's magazine out, in or swapped from the menu (firing
        // spec, "A magazine taken out or put in from the inventory"): a move
        // of one thing on the weapon in hand is its hands' (`settle`); a
        // swap of two is made at once.
        Line::Unload => unload_weapon(carried, id),
        Line::Load => {
            let next = carried.find(id).and_then(|weapon| trp_sim::inventory::magazine_for(&carried.equipment.item, weapon));
            match next {
                Some(next) => fit_magazine(carried, next, id),
                None => {
                    let name = carried.find(id).map(|item| carried.name(item)).unwrap_or_default();
                    carried.note = format!("{name}: can't find any non-empty magazine");
                    None
                }
            }
        }
        Line::Reload => {
            let name = carried.find(id).map(|item| carried.name(item)).unwrap_or_default();
            match trp_sim::inventory::menu_reload(&mut carried.equipment.item, id) {
                Ok(trp_sim::inventory::MenuReload::Swapped(_, new)) => {
                    carried.note = format!("{name}: magazines swapped");
                    carried.tidy();
                    let kind = carried.find(new).map(|magazine| magazine.kind.clone())?;
                    let address = carried.lies(new);
                    Some(move_sound(carried, &kind, address))
                }
                Ok(trp_sim::inventory::MenuReload::Fit(next)) => fit_magazine(carried, next, id),
                Err(Refusal::NoAction) => {
                    carried.note = format!("{name}: can't find any non-empty magazine");
                    None
                }
                Err(Refusal::NoRoom) => {
                    carried.note = format!("{name}: no place for the old magazine (the game throws it down, which is not built)");
                    None
                }
                Err(refusal) => {
                    carried.note = format!("{name}: {}", why(&refusal));
                    None
                }
            }
        }
        Line::Install => {
            let name = carried.find(id).map(|item| carried.name(item)).unwrap_or_default();
            match trp_sim::inventory::menu_install(&mut carried.equipment.item, id, in_hands) {
                Ok(trp_sim::inventory::MenuInstall::Swapped(..)) => {
                    carried.note = format!("{name}: put in, the other magazine swapped out");
                    carried.tidy();
                    let kind = carried.find(id).map(|magazine| magazine.kind.clone())?;
                    let address = carried.lies(id);
                    Some(move_sound(carried, &kind, address))
                }
                Ok(trp_sim::inventory::MenuInstall::Fit(weapon, _)) => fit_magazine(carried, id, weapon),
                Err(Refusal::NoAction) => {
                    carried.note = format!("{name}: can't find appropriate slot");
                    None
                }
                Err(refusal) => {
                    carried.note = format!("{name}: {}", why(&refusal));
                    None
                }
            }
        }
        Line::Uninstall => click(carried, id, false, [true, false, false], cursor, screen),
        // A switch flipped (`ItemUiContext.ToggleItem`): at once; the gesture
        // and the sound, for one worn on the head, are `settle`'s.
        Line::TurnOn | Line::TurnOff => {
            let name = carried.find(id).map(|item| carried.name(item)).unwrap_or_default();
            let switched = trp_sim::inventory::toggle(&mut carried.equipment.item, id).or_else(|| trp_sim::inventory::toggle(&mut carried.loot, id));
            if let Some((part, on)) = switched {
                carried.switched.push((part, on));
                carried.note = format!("{name}: {}", if on { "on" } else { "off" });
            }
            None
        }
        // A magazine checked (`ItemUiContext.CheckMagazine`; inventory spec,
        // "Checking a magazine"): one wait, as a load's process is kept and
        // stopped; nothing if it is known already.
        Line::CheckMagazine => {
            let (name, modifier) = carried.find(id).map(|item| (carried.name(item), item.kind.check_modifier)).unwrap_or_default();
            if !carried.checked.contains(&id) {
                let time = carried.round_times.check(modifier);
                carried.magazine_work = Some(MagazineWork::new(trp_sim::MagazineProcess::check(id, time), 1));
                carried.note = format!("{name}: checking, {time:.2} s");
            }
            None
        }
        // A fold turned (`ItemUiContext.FoldItem`; inventory spec, "A fold
        // from the menu"): the stock's sound as the line is done; on the
        // weapon in hand the hands are asked (`settle`), anywhere else it
        // is turned at once, refused where a weapon in a grid has no room
        // for its new size.
        Line::Fold | Line::Unfold => {
            let name = carried.find(id).map(|item| carried.name(item)).unwrap_or_default();
            let on_hands = in_hands.is_some_and(|weapon| carried.equipment.item.find(weapon).is_some_and(|weapon| weapon.find(id).is_some()));
            if on_hands {
                carried.fold_asked = Some(id);
            } else {
                let roots = &mut [&mut carried.equipment.item, &mut carried.loot];
                carried.note = match trp_sim::inventory::fold(roots, id) {
                    Ok((_, folded)) => format!("{name}: {}", if folded { "folded" } else { "unfolded" }),
                    Err(_) => format!("{name}: can't fold the stock now"),
                };
                carried.tidy();
            }
            Some(Sound::Ui(Ui::MenuStock))
        }
        // A tag edited in its window (`ItemUiContext.EditTag`; inventory
        // spec, "A tag"), or reset at once (`ResetTag`: no name, colour 0).
        Line::Tag => {
            open_tag(carried, id, cursor, screen);
            None
        }
        Line::ResetTag => {
            let name = carried.find(id).map(|item| carried.name(item)).unwrap_or_default();
            if trp_sim::inventory::set_tag(&mut carried.equipment.item, id, "", 0) || trp_sim::inventory::set_tag(&mut carried.loot, id, "", 0) {
                carried.note = format!("{name}: tag reset");
            }
            None
        }
        // A weapon taken apart (`ItemUiContext.Disassemble`; inventory
        // spec, "Disassemble"): its sound, then each of its own parts sent
        // where Ctrl and a click sends it, those that may not come off here
        // staying.
        Line::Disassemble => {
            let name = carried.find(id).map(|item| carried.name(item)).unwrap_or_default();
            let targets = carried.other_side(id);
            let roots = &mut [&mut carried.equipment.item, &mut carried.loot];
            let came = trp_sim::inventory::disassemble(roots, id, &targets, Order::OTHER_SIDE, &carried.rules);
            let off = came.iter().filter(|(_, done)| done.is_ok()).count();
            carried.note = format!("{name}: {off} of its {} parts off", came.len());
            carried.tidy();
            Some(Sound::Ui(Ui::MenuWeaponDisassemble))
        }
        // A magazine unloaded a round at a time (`ItemUiContext.UnloadAmmo`;
        // inventory spec, "Loading and unloading a magazine").
        Line::UnloadAmmo if carried.find(id).is_some_and(|item| is(item, "Magazine") && trp_sim::inventory::magazine_rounds(item) > 0) => {
            let modifier = carried.find(id).map_or(0.0, |item| item.kind.load_unload_modifier);
            let time = carried.round_times.unload(modifier);
            let rounds = carried.find(id).map_or(0, trp_sim::inventory::magazine_rounds);
            carried.magazine_work = Some(MagazineWork::new(trp_sim::MagazineProcess::unload(id, time, None), rounds));
            let name = carried.find(id).map(|item| carried.name(item)).unwrap_or_default();
            carried.note = format!("{name}: unloading, {time:.3} s a round");
            None
        }
        _ => {
            let name = carried.find(id).map(|item| carried.name(item)).unwrap_or_default();
            carried.note = format!("{name}: {} is not built yet", caption.to_uppercase());
            None
        }
    }
}

/// A weapon's magazine taken out by its menu's Unload (`ItemUiContext.
/// UnloadWeapon`): to the first place found for it, the equipment's and,
/// with something open beside it, that's, the side it is from first.
fn unload_weapon(carried: &mut Carried, weapon: Id) -> Option<Sound> {
    let magazine = carried.find(weapon).and_then(|gun| gun.slot_index(trp_sim::inventory::MAGAZINE_SLOT).and_then(|slot| gun.slots[slot].as_ref()).map(|magazine| magazine.id))?;
    let (worn, loot) = (carried.equipment.item.id, carried.loot.id);
    let targets = match (carried.loot_open, carried.equipment.item.find(magazine).is_some()) {
        (false, _) => vec![worn],
        (true, true) => vec![worn, loot],
        (true, false) => vec![loot, worn],
    };
    let (name, kind) = carried.find(magazine).map(|item| (carried.name(item), item.kind.clone()))?;
    let roots = &mut [&mut carried.equipment.item, &mut carried.loot];
    let done = quick_find(roots, magazine, &targets, Order::UNLOAD_WEAPON, &carried.rules);
    carried.note = match &done {
        Ok(_) => format!("{name}: out of the weapon"),
        Err(refusal) => format!("{name}: {}", why(refusal)),
    };
    let went = done.ok().map(|(into, applied)| match applied {
        Applied::Put(spot) => Went::To(into, spot),
        _ => Went::Into(into),
    });
    carried.tidy();
    went.map(|went| move_sound(carried, &kind, went.address(carried)))
}

/// A magazine moved into a weapon's empty magazine slot (the menu's Load,
/// Reload with none fitted, Install): a move of one thing.
fn fit_magazine(carried: &mut Carried, magazine: Id, weapon: Id) -> Option<Sound> {
    let slot = carried.find(weapon)?.slot_index(trp_sim::inventory::MAGAZINE_SLOT)?;
    let (name, kind) = carried.find(magazine).map(|item| (carried.name(item), item.kind.clone()))?;
    let roots = &mut [&mut carried.equipment.item, &mut carried.loot];
    let done = move_item(roots, magazine, weapon, Spot::Slot(slot), &carried.rules);
    carried.note = match &done {
        Ok(()) => format!("{name}: into the weapon"),
        Err(refusal) => format!("{name}: {}", why(refusal)),
    };
    carried.tidy();
    done.ok().map(|()| move_sound(carried, &kind, Some((weapon, Spot::Slot(slot)))))
}

/// A thing's window opened (`ItemUiContext.OpenItem` for one that holds
/// things, `Inspect` for anything): a container sounds its group's own
/// opening, or the interface's; a window that shows a thing, the
/// inspector's, even where it is open.
fn open_view(carried: &mut Carried, id: Id, grids: bool, cursor: Vec2, screen: Option<(Vec2, f32)>) -> Option<Sound> {
    let kind = carried.find(id)?.kind.clone();
    let sound = if grids { Sound::Item(kind.sound.clone(), Way::Open) } else { Sound::Ui(Ui::InspectorOpen) };
    // Where the game opens a window (a container's placed by its own size
    // once it is laid out: `fit_windows`); the app's own look's a little
    // below and right of the mouse.
    let at = match (grids, carried.info_size, screen) {
        (false, Some(size), Some((screen, s))) => game_window_at(carried, size, screen, s),
        (true, _, Some((screen, s))) if carried.game_grid_window => game_window_at(carried, Vec2::ZERO, screen, s),
        _ => cursor + Vec2::new(14.0, 14.0),
    };
    carried.open_window(id, if grids { WinKind::Grids } else { WinKind::Info }, at);
    Some(sound)
}

/// A click on a thing (`ItemView.OnClick`, the left button): a double
/// click with neither Ctrl nor Shift held opens a container in a window
/// of its own and shows what anything else is (medicine, food and drink
/// are used before this is asked: `use_by_double_click`); else Ctrl
/// sends the thing to the other side, Shift to the sorting table (a raid
/// has none), and Alt puts it on.
fn click(carried: &mut Carried, id: Id, double: bool, [ctrl, shift, alt]: [bool; 3], cursor: Vec2, screen: Option<(Vec2, f32)>) -> Option<Sound> {
    let item = carried.find(id)?;
    let (name, kind) = (carried.name(item), item.kind.clone());
    if double && !ctrl && !shift {
        let grids = Carried::opens(item);
        return open_view(carried, id, grids, cursor, screen);
    }
    let targets = carried.other_side(id);
    let worn = carried.equipment.item.id;
    let roots = &mut [&mut carried.equipment.item, &mut carried.loot];
    // What it came to, and where it went for its sound.
    let (note, went) = if ctrl {
        match quick_find(roots, id, &targets, Order::OTHER_SIDE, &carried.rules) {
            Ok((into, Applied::Merged)) => ("merged".to_owned(), Some(Went::Into(into))),
            Ok((into, Applied::Topped(count))) => (format!("{count} handed over"), Some(Went::Into(into))),
            Ok((into, Applied::Put(spot))) => ("sent over".to_owned(), Some(Went::To(into, spot))),
            Err(refusal) => (why(&refusal), None),
        }
    } else if shift {
        ("there is no sorting table in a raid".to_owned(), None)
    } else if alt {
        match equip(roots, id, &carried.rules) {
            Ok(slot) => ("put on".to_owned(), Some(Went::To(worn, Spot::Slot(slot)))),
            Err(Refusal::AlreadyThere) => ("it is on already".to_owned(), None),
            Err(Refusal::NoRoom) => ("no free slot for it".to_owned(), None),
            Err(Refusal::NoAction) => ("it is not a thing to put on".to_owned(), None),
            Err(refusal) => (why(&refusal), None),
        }
    } else if carried.magazine_work.as_ref().is_some_and(|work| work.process.magazine == id) {
        // A plain click on a magazine being worked stops it (`ItemView`:
        // `StopProcesses`).
        carried.magazine_work = None;
        ("stopped".to_owned(), None)
    } else {
        return None;
    };
    carried.note = format!("{name}: {note}");
    carried.tidy();
    went.map(|went| move_sound(carried, &kind, went.address(carried)))
}

/// Where a thing went: into a place of a holder, or into another thing
/// (a stack it merged with: where that lies).
#[derive(Clone, Copy)]
enum Went {
    To(Id, Spot),
    Into(Id),
}

impl Went {
    fn address(self, carried: &Carried) -> Option<(Id, Spot)> {
        match self {
            Went::To(holder, spot) => Some((holder, spot)),
            Went::Into(other) => carried.lies(other),
        }
    }

    /// Where a drop on that target puts a thing.
    fn of(target: Target) -> Self {
        match target {
            Target::Item(onto) => Went::Into(onto),
            Target::Cell(holder, grid, place) => Went::To(holder, Spot::Cell(grid, place)),
            Target::Slot(holder, slot) => Went::To(holder, Spot::Slot(slot)),
        }
    }
}

/// The sound of a thing of that kind gone to a place
/// (`ItemUiContext.PlayOperationSound`): a part into a slot of a weapon
/// sounds the interface's installing (a magazine's `MenuInstallMag`, else
/// its class's: `ItemViewFactory.GetModClass`, `GUISounds.PlayItemSound`, a
/// vital part's, a gear part's, any other part's the functional one's);
/// anything else its group's `use` into a slot of the equipment or into a
/// magazine, its `drop` anywhere else (a grid, a slot of another thing).
fn move_sound(carried: &Carried, kind: &trp_sim::inventory::Kind, address: Option<(Id, Spot)>) -> Sound {
    let of = |class: &str| kind.class_names.iter().any(|name| name == class);
    let parent = address.filter(|(_, spot)| !matches!(spot, Spot::Cell(..))).and_then(|(holder, _)| carried.find(holder));
    if parent.is_some_and(|parent| is(parent, "Weapon")) && of("Mod") {
        let ui = if of("Magazine") {
            Some(Ui::InstallMag)
        } else if of("FunctionalMod") || of("AuxiliaryMod") || of("ArmorPlate") {
            Some(Ui::InstallModFunc)
        } else if of("GearMod") {
            Some(Ui::InstallModGear)
        } else if of("MasterMod") {
            Some(Ui::InstallModVital)
        } else {
            None
        };
        if let Some(ui) = ui {
            return Sound::Ui(ui);
        }
    }
    let used = parent.is_some_and(|parent| parent.id == carried.equipment.item.id || is(parent, "Magazine"));
    Sound::Item(kind.sound.clone(), if used { Way::Use } else { Way::Drop })
}

/// Where the game opens a window (`ItemUiContext.OpenWindow`), so large in
/// units, on a screen so large in pixels, `s` to a unit: 40 units right
/// of and below the top left corner of the window opened last of those
/// open, or with none open in the middle of the screen; moved back onto
/// the screen where it would leave it (`CorrectPositionResolution`).
fn game_window_at(carried: &Carried, size: Vec2, screen: Vec2, s: f32) -> Vec2 {
    let size = size * s;
    let at = match carried.windows.iter().max_by_key(|window| window.opened) {
        Some(last) => last.at + Vec2::new(40.0, 40.0) * s,
        None => (screen - size) / 2.0,
    };
    at.min(screen - size).max(Vec2::ZERO)
}

// ==== Dragging, asking and windows ====

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
    blinking: Res<Blinking>,
    camera: Query<&GlobalTransform, With<crate::weapon::Eye>>,
    (items, slots, grids, covers, info_buttons): (
        Query<(&ItemView, &RelativeCursorPosition, &ComputedNode)>,
        Query<(&SlotBox, &RelativeCursorPosition)>,
        Query<(&GridBox, &RelativeCursorPosition, &ComputedNode, Option<&bevy::ui::CalculatedClip>)>,
        Query<(&Cover, &RelativeCursorPosition)>,
        Query<(&InfoButtonHit, &RelativeCursorPosition)>,
    ),
    mut ghost: Query<(&mut Node, &mut Visibility, &mut BorderColor, &mut BackgroundColor), With<Ghost>>,
    (mut ghost_words, mut ghost_picture, mut drop_shown): (Query<&mut Text, With<GhostWords>>, Query<(&mut ImageNode, &mut Visibility), (With<GhostPicture>, Without<Ghost>)>, ResMut<DropShown>),
    (mut asked, game, drops, mut sounds, mut menu, in_hands, said): (
        ResMut<AskedFor>,
        Res<crate::GameData>,
        Query<(&crate::health_view::HealthDrop, &RelativeCursorPosition, &InheritedVisibility)>,
        MessageWriter<Sound>,
        ResMut<crate::item_menu_view::ItemMenu>,
        Res<HandsHeld>,
        Res<crate::health_view::Words>,
    ),
) {
    let _clock = crate::render_timing::Clock::new("inventory drag");
    let Ok((mut node, mut shown, mut border, mut ground)) = ghost.single_mut() else { return };
    // Nothing marked unless a thing is held over a place this frame.
    if *drop_shown != DropShown::default() {
        *drop_shown = DropShown::default();
    }
    let cell = units.cell();
    let cursor = window.single().ok().and_then(Window::cursor_position);
    // Nothing is taken with the screen shut, while the window that asks
    // how many is up, or while a window is being moved.
    let free = controller.inventory_open && carried.asking.is_none() && clicks.moving.is_none() && clicks.stretching.is_none() && !clicks.sliding;
    let (true, Some(cursor)) = (free, cursor) else {
        (drag.0, clicks.pressed) = (None, None);
        *shown = Visibility::Hidden;
        if !controller.inventory_open && menu.0.is_some() {
            menu.0 = None;
        }
        return;
    };
    let cover = cover_under(covers.iter());
    let under = |but: Option<Id>| item_under(items.iter(), cover, but, &blinking);
    let held_key = |left: KeyCode, right: KeyCode| keys.pressed(left) || keys.pressed(right);
    let (ctrl, shift, alt) = (held_key(KeyCode::ControlLeft, KeyCode::ControlRight), held_key(KeyCode::ShiftLeft, KeyCode::ShiftRight), held_key(KeyCode::AltLeft, KeyCode::AltRight));
    let left = MouseButton::Left;
    let screen = window.single().ok().map(|window| (Vec2::new(window.width(), window.height()), units.0));
    // The menu of a right click (inventory spec, "The menu of a right
    // click"): any button let go shuts it (`SimpleContextMenu.Update`),
    // and one let go over a line that can be done does that line first.
    if buttons.get_just_released().next().is_some() {
        if let Some(open) = menu.0.take() {
            // A line of the wish list's own (`WishlistContextInteractions.
            // ExecuteInteractionInternal`): the thing's kind put in a group,
            // moved to it, or taken off.
            if let Some((line, Ok(()), caption)) = open.sub.as_ref().and_then(|sub| sub.hover.and_then(|index| sub.lines.get(index).cloned())) {
                if let Some(item) = carried.find(open.id) {
                    let (template, name) = (item.kind.template.clone(), carried.name(item));
                    if trp_sim::item_menu::wish(&mut carried.wishlist, &template, line) {
                        carried.note = format!("{name}: wish list, {caption}");
                    }
                }
                (drag.0, clicks.pressed) = (None, None);
                return;
            }
            if let Some((line, Ok(()), caption)) = open.hover.and_then(|index| open.lines.get(index).cloned()) {
                let sound = do_line(&mut carried, open.id, line, &caption, cursor, screen, controller.health.as_ref(), &game.db, &mut asked, camera.single().ok(), controller.movement.position[1], in_hands.item);
                if let Some(sound) = sound {
                    sounds.write(sound);
                }
                (drag.0, clicks.pressed) = (None, None);
                return;
            }
        }
    }
    // A button of a window that shows a thing, on top where the mouse is,
    // clicked: its line done (`InteractionButtonsContainer`'s button:
    // `ExecuteInteraction`), as the menu's; the wish list's opens its own
    // lines in the menu, where the mouse is.
    if drag.0.is_none() && clicks.pressed.is_none() && buttons.just_released(left) {
        let hit = info_buttons.iter().find(|(hit, at)| at.cursor_over() && hit.2 == cover).map(|(hit, _)| (hit.0, hit.1));
        if let Some((id, line)) = hit {
            let lines = menu_lines(&carried, id, &in_hands, controller.health.as_ref(), &game.db, &said);
            if let Some((line, Ok(()), caption)) = lines.iter().find(|(own, ..)| *own == line).cloned() {
                if line.has_lines() {
                    let index = lines.iter().position(|(own, ..)| *own == line).unwrap_or(0);
                    menu.0 = Some(crate::item_menu_view::OpenMenu { id, lines: vec![lines[index].clone()], cursor, hover: None, sub: None });
                } else if let Some(sound) = do_line(&mut carried, id, line, &caption, cursor, screen, controller.health.as_ref(), &game.db, &mut asked, camera.single().ok(), controller.movement.position[1], in_hands.item) {
                    sounds.write(sound);
                }
            }
            return;
        }
    }
    // The right button let go on a thing opens its menu where the mouse
    // is (`ItemView.OnClick`, `ShowContextMenu`).
    if drag.0.is_none() && buttons.just_released(MouseButton::Right) {
        if let Some((id, _)) = under(None) {
            let lines = menu_lines(&carried, id, &in_hands, controller.health.as_ref(), &game.db, &said);
            if !lines.is_empty() {
                menu.0 = Some(crate::item_menu_view::OpenMenu { id, lines, cursor, hover: None, sub: None });
            }
            return;
        }
    }
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
        // A plain click does nothing (`ItemView.OnClick`, but for a
        // magazine being worked): what is carried is not touched, so the
        // page is not drawn afresh (it was, and the boxes the mouse lights
        // went dark for a frame: the user's "UI blinking", 2026-10-23).
        let worked = carried.magazine_work.as_ref().is_some_and(|work| work.process.magazine == pressed.id);
        if !(double || ctrl || shift || alt || worked) {
            return;
        }
        if double && !ctrl && !shift && use_thing(&mut carried, pressed.id, controller.health.as_ref(), &game.db, cursor, &mut asked, Using::DoubleClick) {
            return;
        }
        if let Some(sound) = click(&mut carried, pressed.id, double, [ctrl, shift, alt], cursor, screen) {
            sounds.write(sound);
        }
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
            // Taken up: its group's sound (`ItemView.OnBeginDrag`).
            sounds.write(Sound::Item(item.kind.sound.clone(), Way::Pickup));
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
        if let Some(id) = which {
            put_down(&mut carried, id, camera.single().ok(), controller.movement.position[1]);
            (drag.0, clicks.pressed) = (None, None);
        }
    }
    let Some(held) = drag.0.as_mut() else {
        *shown = Visibility::Hidden;
        return;
    };
    // `R` turns it a quarter, whatever its shape (`DraggedItemView.
    // RotateByInput`: a square thing's picture turns too).
    if keys.just_pressed(KeyCode::KeyR) {
        held.turned = !held.turned;
    }
    let [w, h] = if held.turned { [held.cells[1], held.cells[0]] } else { held.cells };
    // It hangs by its middle (`DraggedItemView.SetInCenter`: its pivot at
    // the mouse), and a grid takes its place from its corner
    // (`GridView.CalculateItemLocation`).
    held.grab = Vec2::new(w as f32, h as f32) * cell / 2.0;
    // Where it would go if let go now, on what is drawn on top there, and
    // the grid's top left corner on the screen.
    let grid_under = || grids.iter().find(|(grid, at, ..)| grid.layer == cover && at.cursor_over());
    let on_cell = || {
        let (grid, at, ..) = grid_under()?;
        let corner = (share_of(at)? * Vec2::new(grid.cells[0] as f32, grid.cells[1] as f32) * cell - held.grab) / cell;
        let (x, y) = (corner.x.round().clamp(0.0, grid.cells[0] as f32), corner.y.round().clamp(0.0, grid.cells[1] as f32));
        Some(Target::Cell(grid.holder, grid.grid, Place { x: x as u32, y: y as u32, turned: held.turned }))
    };
    let on_slot = || slots.iter().find(|(slot, at)| slot.layer == cover && at.cursor_over()).map(|(slot, _)| Target::Slot(slot.holder, slot.slot));
    let target = under(Some(held.id)).map(|(onto, _)| Target::Item(onto)).or_else(on_cell).or_else(on_slot);
    // Tried on copies first, as the game tries a move before it shows
    // whether the place will take the thing. With Ctrl held it is part of
    // a stack that would go, and one is tried.
    let would = target.map(|target| {
        let (mut worn, mut loot) = (carried.equipment.item.clone(), carried.loot.clone());
        act(&mut [&mut worn, &mut loot], held.id, target, &carried.rules, ctrl.then_some(1))
    });
    *shown = Visibility::Inherited;
    // Its own picture, as large as its cells (`DraggedItemView.RotateItem`:
    // 63 units a cell and one, `GetCellPixelSize`), turned with it, white
    // (a thing examined: every thing here is); else the app's box with its
    // name, while its picture is still to come.
    let picture = carried.find(held.id).and_then(|item| carried.pictures.get(&shown_key(&carried, item)));
    let (show_picture, words_now) = match picture {
        Some(picture) => {
            let size = (Vec2::new(w as f32, h as f32) * CELL + Vec2::ONE) * units.0;
            (node.left, node.top, node.width, node.height) = (Val::Px(cursor.x - size.x / 2.0), Val::Px(cursor.y - size.y / 2.0), Val::Px(size.x), Val::Px(size.y));
            (node.border, node.padding) = (UiRect::ZERO, UiRect::ZERO);
            *border = BorderColor::all(Color::NONE);
            ground.0 = Color::NONE;
            if let Ok((mut image, _)) = ghost_picture.single_mut() {
                let wanted = if held.turned { &picture.turned } else { &picture.upright };
                if image.image != *wanted {
                    image.image = wanted.clone();
                }
            }
            (true, String::new())
        }
        None => {
            (node.left, node.top) = (Val::Px(cursor.x - held.grab.x), Val::Px(cursor.y - held.grab.y));
            (node.width, node.height) = (Val::Px(w as f32 * cell - 2.0), Val::Px(h as f32 * cell - 2.0));
            (node.border, node.padding) = (UiRect::all(Val::Px(1.0)), UiRect::all(Val::Px(2.0)));
            *border = BorderColor::all(WORDS);
            ground.0 = THING.with_alpha(0.8);
            (false, held.name.clone())
        }
    };
    if let Ok((_, mut visibility)) = ghost_picture.single_mut() {
        let want = if show_picture { Visibility::Inherited } else { Visibility::Hidden };
        if *visibility != want {
            *visibility = want;
        }
    }
    if let Ok(mut text) = ghost_words.single_mut() {
        if **text != words_now {
            **text = words_now;
        }
    }
    // What a drop here would mark (`DraggedItemView.UpdateTargetUnderCursor`):
    // over a grid the cells it would take, from its corner, kept inside the
    // grid (`GridView.HighlightItemViewPosition`); over a slot, the slot;
    // each in the colour of what the drop would do.
    let colour = drop_colour(would.as_ref());
    if let (Some((grid, at, laid, clip)), Some(Target::Cell(_, _, place))) = (grid_under(), on_cell()) {
        if let Some(share) = share_of(at) {
            let size = laid.size() * laid.inverse_scale_factor();
            let corner = cursor - share * size;
            let (x0, y0) = (place.x.min(grid.cells[0]), place.y.min(grid.cells[1]));
            let (x1, y1) = ((place.x + w).min(grid.cells[0]), (place.y + h).min(grid.cells[1]));
            let (left, top) = (corner.x + x0 as f32 * cell, corner.y + y0 as f32 * cell);
            let mut lit = Rect::new(left, top, left + (x1 - x0) as f32 * cell, top + (y1 - y0) as f32 * cell);
            // Cut to what shows of the grid: the panel lies in the grid
            // under its scroll view's mask, so its cells past the view's
            // edge are hidden as the grid's are (the engine's clip is in
            // the screen's own pixels).
            if let Some(clip) = clip {
                let k = laid.inverse_scale_factor();
                lit = lit.intersect(Rect::from_corners(clip.clip.min * k, clip.clip.max * k));
            }
            if !lit.is_empty() {
                drop_shown.cells = Some(([lit.min.x, lit.min.y, lit.width(), lit.height()], colour));
            }
        }
    } else if let Some((slot, _)) = slots.iter().find(|(slot, at)| slot.layer == cover && at.cursor_over()) {
        drop_shown.slot = Some((slot.holder, slot.slot, colour));
    }
    if !buttons.pressed(left) {
        let (id, name) = (held.id, held.name.clone());
        drag.0 = None;
        *shown = Visibility::Hidden;
        // Let go on a part of the HEALTH tab (`HealthBarButton.OnDrop`):
        // medicine, food or drink the character has is used on that part
        // (`HealthController.ApplyItem`: taken in hand as by a quick-use
        // key, the part its list), if it can do anything to it
        // (`CanApplyItem`); else nothing.
        let part = drops.iter().find(|(_, at, shown)| shown.get() && at.cursor_over()).map(|(drop, ..)| trp_sim::health::Part::ALL[drop.0]);
        if let (None, Some(part)) = (target, part) {
            let carried = &mut *carried;
            let medicine = carried.equipment.item.find(id).and_then(|item| Some((item, trp_sim::health::Medicine::of(&game.db, &item.kind.template)?)));
            carried.note = match (medicine, controller.health.as_ref()) {
                (Some((item, medicine)), Some(health)) => {
                    let resource = item.resource.unwrap_or_else(|| medicine.full());
                    if health.part_to_apply(&medicine, resource, Some(part)).is_some() {
                        asked.1 = Some((id, Some(part), None));
                        String::new()
                    } else {
                        format!("{name}: it can do nothing there now")
                    }
                }
                (Some(_), None) => format!("{name}: with health switched off nothing is used"),
                (None, _) => format!("{name}: only medicine, food and drink the character has are used on a part"),
            };
            return;
        }
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
            carried.asking = Some(Asking { id, asked: Asked::Move(target), title: if onto { "TRANSFER" } else { "SPLIT" }, most, count: most / 2 + 1, at: cursor, typed: false });
            return;
        }
        // Rounds let go on a magazine load it, a round at a time, as many as
        // it has room for (inventory spec, "Loading and unloading a
        // magazine").
        let onto = match target {
            Target::Item(onto) => Some(onto),
            Target::Slot(holder, slot) => in_slot(roots, holder, slot),
            Target::Cell(..) => None,
        };
        // In the screen opened from the workbench's REPAIR GEAR, a repair
        // kit let go on what it mends asks for its repair window
        // (`Carried::repair_asked`), and nothing is moved (the user's).
        if carried.repairing {
            let mends = onto.and_then(|onto| Some((roots.iter().find_map(|root| root.find(id))?, roots.iter().find_map(|root| root.find(onto))?))).filter(|(kit, thing)| kit_mends(kit, thing));
            if let (Some((_, thing)), Some(onto)) = (mends, onto) {
                carried.note = format!("{name}: repairing {}", thing.kind.name);
                carried.open_repair(id, onto);
                return;
            }
        }
        let loading = onto.and_then(|onto| {
            let (rounds, magazine) = (roots.iter().find_map(|root| root.find(id))?, roots.iter().find_map(|root| root.find(onto))?);
            let full = trp_sim::inventory::magazine_rounds(magazine) >= magazine.kind.capacity;
            (is(rounds, "Ammo") && is(magazine, "Magazine") && !magazine.kind.cartridges.is_empty()).then(|| (onto, rounds_to_load(magazine, rounds), takes_round(magazine, &rounds.kind), magazine.kind.load_unload_modifier, full))
        });
        if let Some((magazine, count, takes, modifier, full)) = loading {
            carried.note = if takes && count > 0 {
                let time = carried.round_times.load(modifier);
                carried.magazine_work = Some(MagazineWork::new(trp_sim::MagazineProcess::load(magazine, id, count, time, carried.round_times.load_elite()), count));
                format!("{name}: {count} to load, {time:.3} s a round")
            } else {
                format!("{name}: {}", why(&if full { Refusal::NoRoom } else { Refusal::Filter }))
            };
            return;
        }
        let kind = roots.iter().find_map(|root| root.find(id)).map(|item| item.kind.clone());
        let done = act(roots, id, target, &carried.rules, part);
        if done.is_ok() {
            carried.known_on_arrival(id);
        }
        if let (Ok(_), Some(kind)) = (&done, kind) {
            // Where it lies now; a stack merged whole, or a part of one,
            // where the target says.
            let address = part.is_none().then(|| carried.lies(id)).flatten().or_else(|| Went::of(target).address(carried));
            sounds.write(move_sound(carried, &kind, address));
        }
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
    mut sounds: MessageWriter<Sound>,
    mut wanted: ResMut<AskedFor>,
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
        match asking.asked {
            // Food or drink: that share of its whole, to the body whole
            // (`ApplyItem(foodDrink, Common, amount / max)`).
            Asked::Use { whole } => {
                wanted.1 = Some((asking.id, None, Some(count as f32 / whole.max(1) as f32)));
                carried.note = format!("{name}: {count} of {whole} to be used");
            }
            Asked::Move(target) => {
                let kind = carried.find(asking.id).map(|item| item.kind.clone());
                let done = act(&mut [&mut carried.equipment.item, &mut carried.loot], asking.id, target, &carried.rules, Some(count));
                if let (Ok(_), Some(kind)) = (&done, kind) {
                    sounds.write(move_sound(carried, &kind, Went::of(target).address(carried)));
                }
                carried.note = match done {
                    Ok(what) => format!("{name}: {count} {what}"),
                    Err(refusal) => format!("{name}: {}", why(&refusal)),
                };
                carried.tidy();
            }
        }
        (carried.asking, clicks.sliding) = (None, false);
        return;
    }
    let before = asking;
    // The slider: held, it goes to the stop nearest the mouse.
    let over = bar.iter().next();
    if buttons.just_pressed(MouseButton::Left) && over.is_some_and(RelativeCursorPosition::cursor_over) {
        clicks.sliding = true;
    }
    if !buttons.pressed(MouseButton::Left) {
        clicks.sliding = false;
    }
    if let (true, Some(at)) = (clicks.sliding, over.and_then(share_of)) {
        (asking.count, asking.typed) = (asking.slid_to(at.x), false);
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
    mut roots: Query<(&WinRoot, &mut Node, &RelativeCursorPosition, &ComputedNode)>,
    close: Query<(&Interaction, &WinClose), Changed<Interaction>>,
    mut sounds: MessageWriter<Sound>,
) {
    if !controller.inventory_open {
        clicks.moving = None;
        if !carried.windows.is_empty() || carried.priority.is_some() {
            // Each window that shows a thing sounds its closing as it
            // shuts (`InfoWindow.Close`).
            for _ in carried.windows.iter().filter(|window| window.kind == WinKind::Info) {
                sounds.write(Sound::Ui(Ui::InspectorClose));
            }
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
            // Escape sounds its own (`Window.TranslateCommand`); a window
            // that shows a thing its closing besides.
            if crossed.is_none() {
                sounds.write(Sound::Ui(Ui::MenuEscape));
            }
            if matches!(carried.windows[index].kind, WinKind::Info | WinKind::Repair) {
                sounds.write(Sound::Ui(Ui::InspectorClose));
            }
            carried.shut_window(index);
            carried.tidy();
        }
        clicks.moving = None;
        clicks.window_press = None;
        return;
    }
    let Ok(screen) = window.single() else { return };
    let Some(cursor) = screen.cursor_position() else { return };
    let room = Vec2::new(screen.width(), screen.height());
    // A press on the window on top where the mouse is.
    if carried.asking.is_none() && buttons.just_pressed(MouseButton::Left) {
        let on = roots.iter().filter(|(.., at, _)| at.cursor_over()).map(|(root, ..)| root.0).max();
        clicks.window_press = on.filter(|index| *index < carried.windows.len()).map(|index| WindowPress {
            index,
            from: cursor,
            by_caption: captions.iter().any(|(caption, at)| caption.0 == index && at.cursor_over()),
            thing: false,
            dragged: false,
        });
    }
    if let Some(mut press) = clicks.window_press {
        // (A thing pressed is had from the frame after the press: `drag`
        // runs after this.)
        press.thing |= clicks.pressed.is_some();
        if buttons.pressed(MouseButton::Left) {
            // A drag begun on the window, not from a thing in it, picks it
            // (`OnBeginDrag`); begun on its caption's bar it moves it, held
            // where the mouse holds it as the drag begins (`UIDragComponent`).
            if !press.thing && !press.dragged && cursor.distance(press.from) > DRAG_THRESHOLD {
                press.dragged = true;
                carried.raise(press.index);
                let top = carried.windows.len() - 1;
                if press.by_caption {
                    clicks.moving = Some((top, cursor - carried.windows[top].at));
                }
            }
            clicks.window_press = Some(press);
        } else {
            // Let go on it with no drag and not on a thing: a click, which
            // picks it (`OnPointerClick`).
            let over = roots.iter().filter(|(.., at, _)| at.cursor_over()).map(|(root, ..)| root.0).max();
            if !press.thing && !press.dragged && over == Some(press.index) {
                carried.raise(press.index);
            }
            clicks.window_press = None;
        }
    }
    if !buttons.pressed(MouseButton::Left) {
        clicks.moving = None;
    }
    // Moved: where it is drawn goes with the mouse at once, kept on the
    // screen and at whole pixels (`CorrectPositionResolution`), and where it
    // is kept is changed without the page being drawn afresh for it.
    if let Some((index, hold)) = clicks.moving {
        let size = roots.iter().find(|(root, ..)| root.0 == index).map_or(Vec2::ZERO, |(.., computed)| computed.size() * computed.inverse_scale_factor());
        let at = on_screen(cursor - hold, size, room);
        for (root, mut node, ..) in &mut roots {
            if root.0 == index {
                (node.left, node.top) = (Val::Px(at.x), Val::Px(at.y));
            }
        }
        if let Some(window) = carried.bypass_change_detection().windows.get_mut(index) {
            window.at = at;
        }
    }
}

/// A window's top left corner kept on a screen so large
/// (`RectTransformExtensions.CorrectPositionResolution`): a left edge off
/// the screen moved right, else a right edge moved left; a bottom edge off
/// it moved up, else a top edge moved down (so a window higher than the
/// screen keeps its bottom on it); then at whole pixels.
fn on_screen(at: Vec2, size: Vec2, room: Vec2) -> Vec2 {
    let mut at = at;
    if at.x < 0.0 {
        at.x = 0.0;
    } else if at.x + size.x > room.x {
        at.x = room.x - size.x;
    }
    if at.y + size.y > room.y {
        at.y = room.y - size.y;
    } else if at.y < 0.0 {
        at.y = 0.0;
    }
    at.round()
}

/// A weapon a quick-use key asks for, for `settle` to take in hand; and an
/// item to be used, with the part it is for (let go on that part of the
/// HEALTH tab, or the one the inventory's "Use" names; none, the common
/// body) and food's or drink's share of its whole (none, what is left).
/// And whether the quick stroke was asked for (`QuickKnife`).
/// And the workbench's asks (`BenchAsk`).
#[derive(Resource, Default)]
pub struct AskedFor(Option<Id>, Option<(Id, Option<trp_sim::health::Part>, Option<f32>)>, bool, BenchAsk);

impl AskedFor {
    /// What the workbench asks of the hands.
    pub fn bench(&mut self) -> &mut BenchAsk {
        &mut self.3
    }
}

/// The workbench's asks of the hands (the user, 2026-10-21: "when the
/// workbench menu is opened, the gun is holstered. pulled out and racked
/// when the menu is closed, if anything changed"): the weapon in hand put
/// away as its screen opens, the hands left empty while it stands open,
/// and that weapon taken in hand again as it shuts, by its slot as a key
/// takes it; until it is, the ask stands (a rig still being made for the
/// build is waited for). With a change built (`rack`) it is drawn racking,
/// for the look of it alone (the user: "I don't want the rack to actually
/// change things. Just a nice visual thing": `Firearm::rack_drawn`, no
/// round moved).
#[derive(Default, Clone, Copy)]
pub struct BenchAsk {
    pub put_away: bool,
    pub take: Option<Id>,
    pub rack: bool,
}

/// The grenade the grenade's own key takes (`ThrowGrenade`; grenades-and-
/// knife spec, "Taking them in hand"): of the grenades lying directly in
/// the vest and then the pockets, the first by its kind's order
/// (`GetThrowablePriorityGrenadesList`: frag, flash, stun, smoke, gas,
/// incendiary, sonar), which is what the quick bar's grenade view chooses
/// unless its selector chose another. Within a container, by grid and then
/// by row and column (`PROVISIONAL`: the game takes a grid's own order of
/// its items, and sorts with a sort that keeps no order among equals).
fn top_grenade(carried: &Carried, db: &trp_data::Database) -> Option<Id> {
    const KINDS: [&str; 7] = ["frag_grenade", "flash_grenade", "stun_grenade", "smoke_grenade", "gas_grenade", "incendiary_grenade", "sonar"];
    let mut found: Vec<(usize, Id)> = Vec::new();
    for slot in ["TacticalVest", "Pockets"] {
        let Some(container) = carried.equipment.worn(slot) else { continue };
        for grid in &container.grids {
            let mut lying: Vec<&(Place, Item)> = grid.iter().filter(|(_, item)| is(item, "ThrowWeap")).collect();
            lying.sort_by_key(|(place, _)| (place.y, place.x));
            for (_, item) in lying {
                let kind = db.items.get(&item.kind.template).and_then(|template| template.props["ThrowType"].as_str()).and_then(|kind| KINDS.iter().position(|k| *k == kind)).unwrap_or(KINDS.len());
                found.push((kind, item.id));
            }
        }
    }
    found.sort_by_key(|(kind, _)| *kind);
    found.first().map(|(_, id)| *id)
}

/// The thing the mouse carries in the screen, if any (`Drag`), for what
/// lights up under it (the HEALTH tab's parts: `health_view`).
#[derive(Resource, Default)]
pub struct Dragging(pub Option<Id>);

fn publish_drag(drag: Res<Drag>, mut dragging: ResMut<Dragging>) {
    let now = drag.0.as_ref().map(|held| held.id);
    if dragging.0 != now {
        dragging.0 = now;
    }
}

// ==== The quick keys, the hands and the slot keys ====

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
    game: Res<GameData>,
    features: Res<crate::options_view::Features>,
    mut carried: ResMut<Carried>,
    mut asked: ResMut<AskedFor>,
    blinking: Res<Blinking>,
    items: Query<(&ItemView, &RelativeCursorPosition, &ComputedNode)>,
    covers: Query<(&Cover, &RelativeCursorPosition)>,
) {
    // With the options' `INVENTORY SYSTEM` off nothing is bound or used.
    if !features.inventory {
        return;
    }
    // The grenade the key takes and its view shows: the equipment's top
    // priority grenade, which the view sets at the raid's start, and again
    // when one is gone, to the first of the list by kind
    // (`FastAccessGrenadeItemView`; chosen otherwise in the game's grenade
    // selector, which is not built).
    let top = if features.grenades_and_knife { top_grenade(&carried, &game.db) } else { None };
    if carried.top_grenade != top {
        carried.top_grenade = top;
    }
    // The grenade's own key takes the first grenade within reach; the
    // knife's twice, the quick stroke (grenades-and-knife spec, "Taking
    // them in hand"). With the options' switch for them off, neither.
    if !controller.inventory_open && options.keys_live() && features.grenades_and_knife {
        if bindings.fired("ThrowGrenade") {
            match carried.top_grenade {
                Some(id) => asked.0 = Some(id),
                None => carried.note = "no grenade lies in the vest or the pockets".to_owned(),
            }
        }
        if bindings.fired("QuickKnife") {
            asked.2 = true;
        }
    }
    // The goggles' key switches the switch of what is worn on the head
    // (`Player.ToggleGoggles`).
    if !controller.inventory_open && options.keys_live() && bindings.fired("ToggleGoggles") {
        let head = carried.equipment.worn("Headwear").map(|head| head.id);
        if let Some(switched) = head.and_then(|head| trp_sim::inventory::toggle(&mut carried.equipment.item, head)) {
            carried.switched.push(switched);
        }
    }
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
        let Some((id, _)) = item_under(items.iter(), cover_under(covers.iter()), None, &blinking) else { return };
        // Only what the character has is bound.
        let Some(item) = carried.equipment.item.find(id) else { return };
        let name = carried.name(item);
        carried.note = match carried.bound.bind(&carried.equipment.item, id, key) {
            true => format!("{name}: bound to {word}"),
            false => format!("{name}: not bound (a key takes a weapon, a grenade, a knife, medicine, food, drink or a rangefinder that lies in the vest or the pockets)"),
        };
        return;
    }
    if !options.keys_live() {
        return;
    }
    let Some(item) = carried.bound.items[key].and_then(|id| carried.equipment.item.find(id)) else { return };
    if is(item, "Weapon") {
        asked.0 = Some(item.id);
    } else if is(item, "ThrowWeap") && features.grenades_and_knife {
        // A grenade is taken in hand to be thrown (`TryProceed`).
        asked.0 = Some(item.id);
    } else if is(item, "PortableRangeFinder") {
        // The rangefinder is taken in hand to be aimed through
        // (`ProceedPortableRangeFinder`; rangefinder spec).
        asked.0 = Some(item.id);
    } else if let Some(medicine) = trp_sim::health::Medicine::of(&game.db, &item.kind.template) {
        // Medicine is used only if it can do something to the body now
        // (`Player.SetQuickSlotItem`: `CanApplyItem(item, Common)`); food
        // and drink if anything is left (health spec, "The hands").
        let (id, name, resource) = (item.id, carried.name(item), item.resource.unwrap_or_else(|| medicine.full()));
        match controller.health.as_ref() {
            Some(health) if health.part_to_apply(&medicine, resource, None).is_some() => asked.0 = Some(id),
            Some(_) => carried.note = format!("{name}: it can do nothing now"),
            None => carried.note = format!("{name}: with health switched off nothing is used"),
        }
    } else {
        // A knife or a grenade: not built yet.
        let name = carried.name(item);
        info!("the key {word}: {name} would be taken in hand; that is not built yet");
        carried.note = format!("{name}: the key {word} takes it in hand in the game; that is not built yet");
    }
}

/// What the hands have been through, for `settle`.
#[derive(Default)]
struct Hands {
    settled: bool,
    /// Whether the last slot was taken by the holster's key pressed twice
    /// (`Player.FastSlotSelection`), and when a weapon was last put away
    /// for the quick draw (`QuickdrawWeaponFast`, `QuickdrawTime`): the
    /// pistol drawn within a second after is drawn quick (stamina spec, "A
    /// drain"; `trp_sim::quick_draw`).
    fast: bool,
    quickdraw: Option<f32>,
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
    /// A weapon asked for by its slot's key while its rig was still being
    /// made (`weapon::Preparing`), and its slot: the key is heard when the
    /// rig comes, unless another is pressed first or the weapon leaves the
    /// slot. (Not the game's: its weapons have no rig to wait for.)
    waiting: Option<(Id, &'static str)>,
    /// A weapon put into the slot the hands last drew from and taken in
    /// hand for it, until it has appeared there; and the weapon in hand
    /// taken out of its slot, until it has been put away and the next is
    /// in hand. Their boxes blink (`Blinking`).
    moved_in: Option<Id>,
    moved_out: Option<Id>,
    /// The item being used in the hands (medicine, food or drink), until
    /// it has been put away.
    using: Option<Id>,
    /// The part the item waiting to be used is for (let go on the HEALTH
    /// tab's, or the inventory's "Use"), or the common body (a quick-use
    /// key's); and food's or drink's share asked (none: what is left).
    use_part: Option<trp_sim::health::Part>,
    use_amount: Option<f32>,
    /// The weapon held, the item in use and whether the hands were putting
    /// something away, as the last frame left them: a change of the hands
    /// begins as they start putting away, or take something into empty
    /// hands.
    was: (Option<Id>, Option<Id>, bool),
    /// An item whose use was asked for while its rig was still being made,
    /// as `waiting` is for a weapon.
    use_waiting: Option<Id>,
    /// An item to be used when what is in hand has gone.
    next_use: Option<Id>,
    /// The item waiting to be used is the sheath's knife for its quick
    /// stroke (grenades-and-knife spec).
    use_quick: bool,
    /// The weapon last taken in hand (`LastEquippedWeaponOrKnifeItem`):
    /// what comes back when an item's use is over.
    last_weapon: Option<Id>,
    /// The weapon in hand and the magazine in its slot as the last frame
    /// left them, and a magazine its hands are moving for the inventory
    /// (firing spec, "A magazine taken out or put in from the inventory").
    fitted: Option<(Id, Option<Id>)>,
    magazine_moving: Option<Id>,
    /// A shot has been made since the weapon's magazine was last set beside
    /// its logic's count (`UncheckOnShot`).
    fired: bool,
    /// The weapon whose hands are putting a round into its chamber.
    loading_chamber: Option<Id>,
    /// The weapon was put away for the workbench: the empty hands do not
    /// take it again from its slot until asked (`BenchAsk`).
    holstered: bool,
}

/// A weapon's magazine, if one is fitted.
fn fitted_magazine(weapon: &Item) -> Option<Id> {
    weapon.slot_index(trp_sim::inventory::MAGAZINE_SLOT).and_then(|slot| weapon.slots[slot].as_ref()).map(|magazine| magazine.id)
}

/// A weapon taken in hand comes as the item has it: its magazine's rounds,
/// and its fold (shotgun spec, "The stock": the animator is told as it is
/// drawn) with its logic, its recoil and hold, and the width the body's
/// animator goes by.
fn fold_as_item(weapon: &mut HandsRig, rig: &WeaponRig, controller: &mut Controller, folded: bool, (magazine, chamber, variant): (Option<Option<u32>>, Option<bool>, Option<i32>)) {
    let Some(firearm) = weapon.firearm.as_mut() else { return };
    // Its magazine's or tube's rounds as the item holds them, and its
    // chamber of its own (a cylinder's or barrels' rounds lie in chambers
    // of their own: the logic's alone); the fitted magazine's clips' index
    // (`magAnimationIndex`), where the item's magazine is not the build's.
    if let Some(magazine) = magazine.filter(|_| firearm.single_chamber()) {
        firearm.magazine = magazine;
    }
    if let Some(variant) = variant {
        firearm.set_magazine_variant(variant);
    }
    if let Some(chamber) = chamber.filter(|_| firearm.single_chamber()) {
        firearm.chamber = chamber;
    }
    firearm.start_folded(folded);
    let folded = firearm.stock_folded();
    weapon.hands.set_folded(folded, false);
    controller.movement.body.set_weapon_width(if folded { weapon.folded_width.unwrap_or(rig.width) } else { rig.width });
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
/// And the knife's, the sheath's (`SelectKnife`; grenades-and-knife spec).
const SLOT_KEYS: [(&str, &str); 4] = [("PrimaryWeaponFirst", "FirstPrimaryWeapon"), ("PrimaryWeaponSecond", "SecondPrimaryWeapon"), ("SecondaryWeapon", "Holster"), ("Knife", "Scabbard")];

// ==== The hands' moves settled, and the reach ====

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
    mut weapon: Option<ResMut<HandsRig>>,
    mut stowed: Option<ResMut<Stowed>>,
    mut controller: ResMut<Controller>,
    (features, skills): (Res<crate::options_view::Features>, Option<Res<crate::skills::Skills>>),
    mut blinking: ResMut<Blinking>,
    mut shown: Query<&mut Visibility>,
    mut hands: Local<Hands>,
    mut again: MessageReader<crate::death::BeginAgain>,
    (mut held, drag): (ResMut<HandsHeld>, Res<Drag>),
) {
    // The run begun again after a death: the hands start over, as the
    // character is made (`death`).
    if again.read().count() > 0 {
        *hands = Hands::default();
    }
    // The weapon's magazine checked in the hands is known from now on.
    if std::mem::take(&mut controller.magazine_checked) {
        let magazine = hands.item.and_then(|weapon| carried.find(weapon)).and_then(|weapon| weapon.slots.iter().zip(&weapon.kind.slots).find(|(_, slot)| slot.name == "mod_magazine").and_then(|(part, _)| part.as_ref()).map(|magazine| magazine.id));
        if let Some(magazine) = magazine {
            carried.checked.insert(magazine);
        }
    }
    let _clock = crate::render_timing::Clock::new("inventory settle");
    // A weapon hung where the hands can take it with no rig yet has its
    // rig made ready before any other (`weapon::Preparing`); the hands
    // take it once it is in the world.
    // (The sheath's knife only with the options' switch for it on.)
    let hung: Vec<String> = HAND_SLOTS
        .into_iter()
        .filter(|slot| features.grenades_and_knife || *slot != "Scabbard")
        .filter_map(|slot| carried.equipment.worn(slot))
        .map(trp_sim::inventory::rig_key)
        .collect();
    let awaited: Vec<&String> = hung.iter().filter(|template| stowed.as_deref().is_some_and(|stowed| stowed.awaited(template))).collect();
    if let Some(preparing) = preparing.as_deref_mut() {
        for template in &awaited {
            preparing.ask(template);
        }
    }
    let game = &*game;
    if carried.is_changed() || !hands.settled {
        // With the weight switched off what is carried weighs nothing.
        let weight = if controller.movement.systems.weight { carried.equipment.carried_weight(carried.strength_elite) } else { 0.0 };
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
    let quick_knife = std::mem::take(&mut asked.2);
    // A grenade let go of in a throw is gone from the inventory as the hand
    // lets it go (`ThrowGrenade`, an inventory operation).
    // It flies on as a thing of the world's (`loose`, `How::Grenade`) until
    // its fuse.
    if std::mem::take(&mut controller.grenade_thrown) {
        let low = std::mem::take(&mut controller.throws).last().copied().unwrap_or(false);
        if let Some(gone) = hands.using.and_then(|id| trp_sim::inventory::use_up(&mut carried.equipment.item, id)) {
            carried.note = format!("{}: thrown", carried.name(&gone));
            let m = &controller.movement;
            let at = beside(m.position, m.yaw, [0.0, 0.0]);
            carried.thrown.push((gone.id, crate::loose::How::Grenade { low }));
            carried.ground.push((gone, at));
        }
    }
    let dropped = asked.1.take();
    // The workbench's asks (`BenchAsk`), with the weapon's handling on.
    let bench_put_away = std::mem::take(&mut asked.3.put_away) && features.weapon_handling;
    let bench_take = asked.3.take.filter(|_| features.weapon_handling);
    let bench_rack = asked.3.rack;
    // What health says of an item it was given: what it holds now (a
    // medkit's points, food's share left), or that it is used up and gone.
    for event in std::mem::take(&mut controller.items_said) {
        match event {
            trp_sim::health::Event::Resource(id, left) => {
                if carried.equipment.item.find(id).is_some_and(|item| item.resource != Some(left)) {
                    trp_sim::inventory::set_resource(&mut carried.equipment.item, id, left);
                }
            }
            trp_sim::health::Event::UsedUp(id) => {
                if let Some(gone) = trp_sim::inventory::use_up(&mut carried.equipment.item, id) {
                    carried.note = format!("{}: used up", carried.name(&gone));
                }
            }
            _ => {}
        }
    }
    // What the weapon's tube took leaves the pockets and the vest, and
    // what is left there is what the next filling has to go by; it goes
    // onto the tube's top in its item (the round for an empty chamber with
    // it: at rest it is taken off for the chamber, below).
    let taken = std::mem::take(&mut controller.rounds_taken);
    if let (true, Some(weapon)) = (taken > 0, hands.item) {
        let rounds = trp_sim::inventory::take_rounds_of_kinds(&mut carried.equipment.item, weapon, taken);
        if let Some(tube) = carried.equipment.item.find(weapon).filter(|gun| gun.kind.reload_mode == "InternalMagazine").and_then(fitted_magazine) {
            trp_sim::inventory::put_rounds_into(&mut carried.equipment.item, tube, &rounds);
            // Left full, it is known (`CheckMagazineAmmoDepend`).
            let held = carried.find(tube).map(|item| (trp_sim::inventory::magazine_rounds(item), item.kind.capacity));
            if held.is_some_and(|(count, capacity)| trp_sim::inventory::known_after(count, capacity, false, false)) {
                carried.checked.insert(tube);
            }
        }
    }
    controller.rounds_in_reach = hands.item.map_or(0, |weapon| trp_sim::inventory::rounds_in_reach(&carried.equipment.item, weapon));
    // A magazine's reload makes its exchange as it begins: the magazine
    // its key chose goes into the weapon, and the old one, with what the
    // weapon left in it, to the vest or the pockets, or (the quick way,
    // or with no room there) out to fall as it leaves the hand (firing
    // spec, "The quick reload").
    let reloads = std::mem::take(&mut controller.swaps);
    let reloaded = !reloads.is_empty();
    for (left, quick) in reloads {
        let (Some(weapon), Some(next)) = (hands.item, controller.magazine_asked.take()) else { continue };
        let m = &controller.movement;
        let at = beside(m.position, m.yaw, MAGAZINE_DROP);
        // One still on its way down from a reload before is on the ground.
        let fallen = std::mem::take(&mut carried.falling);
        carried.thrown.extend(fallen.iter().map(|old| (old.id, crate::loose::How::LetFall)));
        carried.ground.extend(fallen.into_iter().map(|old| (old, at)));
        if let Some(old) = trp_sim::inventory::swap_magazine(&mut carried.equipment.item, weapon, Some(next), left, quick) {
            carried.falling.push(old);
        }
    }
    // A switch flipped on what is worn on the head (inventory spec, "A
    // switch on the head"): the hands asked for its gesture where the
    // character's state lets them act (the gesture itself is refused while
    // the thing in hand is not at rest, so never under the open screen), and
    // its sound, the face shield's, night vision's or thermal vision's.
    let switched = if carried.switched.is_empty() { Vec::new() } else { std::mem::take(&mut carried.switched) };
    for (part, on) in switched {
        let Some(item) = carried.equipment.worn("Headwear").and_then(|head| head.find(part)) else { continue };
        let face_shield = item.kind.face_shield;
        let sound = if face_shield {
            0
        } else if is(item, "ThermalVision") {
            4
        } else {
            2
        } + usize::from(!on);
        controller.toggle_sounds.push(sound);
        info!("a switch on the head {}: its sound {}", if on { "on" } else { "off" }, trp_data::sound::TOGGLE_SOUNDS[sound]);
        if controller.movement.state_suits_hands() {
            use trp_sim::Interaction;
            let gesture = match (face_shield, on) {
                (true, true) => Interaction::FaceshieldOnGear,
                (true, false) => Interaction::FaceshieldOffGear,
                (false, true) => Interaction::NightVisionOnGear,
                (false, false) => Interaction::NightVisionOffGear,
            };
            let plays = controller.movement.set_interaction(gesture);
            info!("its gesture {gesture:?}: {}", if plays { "plays" } else { "refused (the thing in hand not at rest, or a sprint)" });
        }
    }
    // What the weapon's own reloads changed in its magazine slot is the
    // hands' doing, not a move to tell them of (below).
    if reloaded {
        hands.fitted = hands.item.and_then(|id| carried.equipment.item.find(id)).map(|gun| (gun.id, fitted_magazine(gun)));
    }
    if std::mem::take(&mut controller.magazines_hidden) > 0 && !carried.falling.is_empty() {
        let m = &controller.movement;
        let at = beside(m.position, m.yaw, MAGAZINE_DROP);
        let fallen = std::mem::take(&mut carried.falling);
        carried.thrown.extend(fallen.iter().map(|old| (old.id, crate::loose::How::LetFall)));
        carried.ground.extend(fallen.into_iter().map(|old| (old, at)));
        carried.note = "the old magazine was let fall: it lies on the ground".to_owned();
    }
    controller.next_magazine = hands.item.and_then(|weapon| {
        let next = trp_sim::inventory::next_magazine(&carried.equipment.item, weapon)?;
        let magazine = carried.equipment.item.find(next)?;
        Some((next, trp_sim::inventory::magazine_rounds(magazine), magazine.kind.magazine_animation))
    });
    // The build the next magazine would make, where it is of another kind
    // than the one fitted, has its rig made ready ahead (`Preparing::
    // ask_ahead`), so that a reload's change of the model finds it built
    // before the new magazine shows (`Firearm::carry_on_from`).
    // A magazine taken up with the screen open is the next one where the
    // weapon in hand takes it: let go on the weapon it is put in by the
    // hands, and the game shows its own model at the insert's `MagShow`
    // (`InsertMagOperation.OnMagAppeared`: `Firearms.SetupMod` with the
    // magazine's model made there and then).
    let taken = drag.0.as_ref().map(|held| held.id).filter(|id| carried.find(*id).is_some_and(|item| is(item, "Magazine")));
    let build_with = |next: Id| -> Option<String> {
        let gun = carried.equipment.item.find(hands.item?)?;
        let magazine = carried.find(next)?;
        let slot = gun.slot_index(trp_sim::inventory::MAGAZINE_SLOT)?;
        if !magazine.kind.among(&gun.kind.slots.get(slot)?.takes) || gun.slots[slot].as_ref().is_some_and(|fitted| fitted.kind.template == magazine.kind.template) {
            return None;
        }
        let mut with = gun.clone();
        with.slots[slot] = Some(magazine.clone());
        Some(trp_sim::inventory::rig_key(&with))
    };
    let next_build = taken.and_then(build_with).or_else(|| controller.next_magazine.and_then(|(next, ..)| build_with(next)));
    if let Some(stowed) = stowed.as_deref_mut() {
        if stowed.next_build != next_build {
            stowed.next_build = next_build.clone();
        }
    }
    if let (Some(key), Some(preparing), Some(stowed)) = (next_build, preparing.as_deref_mut(), stowed.as_deref()) {
        if !stowed.has_rig(&key) && stowed.awaited(&key) {
            preparing.ask_ahead(&key);
        }
    }
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
            // Half a pace ahead and a little to the right.
            let at = beside(m.position, m.yaw, [DROP_AHEAD, 0.2]);
            for _ in 0..ejected {
                carried.ground.push((Item::new(kind.clone()), at));
            }
            carried.note = "the chambered round was thrown out as the bolt was opened: it lies on the ground".to_owned();
        }
    }
    // A live round a single barrel's reload took out goes to the vest or
    // the pockets where it has room, and else down on the ground (more-
    // weapons spec, "One barrel loaded alone, and the rocket launcher").
    let unloaded = std::mem::take(&mut controller.rounds_unloaded);
    let round = hands
        .item
        .and_then(|weapon| carried.equipment.item.find(weapon))
        .and_then(|weapon| game.db.items.get(&weapon.kind.template)?.props.get("defAmmo")?.as_str().map(str::to_owned));
    if let (true, Some(round)) = (unloaded > 0, round) {
        if let Ok(kind) = trp_sim::inventory::Kind::of(&game.db, &round).map(std::sync::Arc::new) {
            let m = &controller.movement;
            let at = beside(m.position, m.yaw, [DROP_AHEAD, 0.2]);
            for _ in 0..unloaded {
                if let Some(left) = trp_sim::inventory::put_unloaded(&mut carried.equipment.item, Item::new(kind.clone())) {
                    carried.ground.push((left, at));
                    carried.note = "the round taken out of the barrel had no room: it lies on the ground".to_owned();
                }
            }
        }
    }
    // The weapon in hand's magazine moved in the inventory (firing spec,
    // "A magazine taken out or put in from the inventory"): one thing moved
    // out or in, its hands do it, the magazine's box blinking until they
    // are done; two swapped, its logic simply has the new one. The one that
    // leaves takes the rounds the logic had in it. While the hands cannot
    // take such a move, the inventory refuses it (`Rules::busy_weapon`).
    if std::mem::take(&mut controller.magazines_moved) > 0 {
        hands.magazine_moving = None;
    }
    let fitted = hands.item.and_then(|id| carried.equipment.item.find(id)).map(|gun| (gun.id, fitted_magazine(gun)));
    let mut firearm = weapon.as_deref_mut().and_then(|rig| rig.firearm.as_mut());
    if let (Some(firearm), Some((was_gun, was)), Some((gun, now))) = (firearm.as_deref_mut(), hands.fitted, fitted) {
        if was_gun == gun && was != now {
            let left = firearm.magazine.unwrap_or(0);
            let rounds = |carried: &Carried, id: Id| carried.find(id).map_or(0, trp_sim::inventory::magazine_rounds);
            let variant = |carried: &Carried, id: Id| carried.find(id).map_or(0, |magazine| magazine.kind.magazine_animation);
            if let Some(old) = was {
                trp_sim::inventory::set_rounds_of(&mut carried.equipment.item, old, left);
                trp_sim::inventory::set_rounds_of(&mut carried.loot, old, left);
            }
            match (was, now) {
                (Some(old), None) => {
                    if firearm.pull_out_magazine() {
                        hands.magazine_moving = Some(old);
                    } else {
                        firearm.magazine = None;
                    }
                }
                (None, Some(new)) => {
                    let rounds = rounds(&*carried, new);
                    let variant = variant(&*carried, new);
                    firearm.next_magazine_variant(variant);
                    if firearm.insert_magazine(rounds) {
                        hands.magazine_moving = Some(new);
                    } else {
                        firearm.magazine = Some(rounds);
                        firearm.set_magazine_variant(variant);
                    }
                }
                (Some(_), Some(new)) => {
                    firearm.magazine = Some(rounds(&*carried, new));
                    firearm.set_magazine_variant(variant(&*carried, new));
                }
                (None, None) => {}
            }
        }
    }
    // The weapon in hand's fold (inventory spec, "A fold from the menu";
    // shotgun spec, "The stock"): asked from the menu, its hands take it or
    // not, and the item is folded at once where they do (`FoldOperation`:
    // done, then the hands' `FoldStockOperation`; refused, undone); begun by
    // the key, the item folds with it.
    let fold_asked = if carried.fold_asked.is_some() { carried.fold_asked.take() } else { None };
    let begun = if controller.folds_begun > 0 { std::mem::take(&mut controller.folds_begun) } else { 0 };
    if let Some(gun) = hands.item.filter(|_| fold_asked.is_some() || begun > 0) {
        let mut turned = vec![gun; begun as usize];
        if let Some(part) = fold_asked {
            if firearm.as_deref_mut().is_some_and(|firearm| firearm.fold_stock()) {
                turned.push(part);
            } else {
                carried.note = "the hands can't fold the stock now".to_owned();
            }
        }
        for id in turned {
            if let Err(refusal) = trp_sim::inventory::fold(&mut [&mut carried.equipment.item], id) {
                warn!("the weapon in hand's fold not turned with its hands': {refusal:?}");
            }
        }
    }
    // The weapon in hand's detachable magazine as its logic counts it
    // (inventory spec, "Checking a magazine"): at rest, rounds its logic
    // has fired or chambered are gone from the item too, the magazine
    // known or not by the game's rule, a shot unknowing it where it is
    // left neither empty nor full (`UncheckOnShot`). A tube's shells are
    // not kept in its item, so it is left alone.
    hands.fired |= std::mem::take(&mut controller.shots_made) > 0;
    // The chambered round as its hands have it (firing spec, "A round into
    // the chamber from the inventory"): a round the inventory has just put
    // into the empty chamber is loaded by the hands (the move was let only
    // where they would take it); a load given up for a put-away is undone,
    // the round put back where a pick-up puts it (`PROVISIONAL`: the game
    // puts it back where it was).
    let given_up = controller.chamber_given_up && std::mem::take(&mut controller.chamber_given_up);
    let round_in = |carried: &Carried, gun: Id| carried.equipment.item.find(gun).and_then(trp_sim::inventory::chambered).map(|round| round.kind.clone());
    if let (true, Some(gun)) = (given_up, hands.loading_chamber.take()) {
        if let Some(kind) = round_in(&*carried, gun) {
            trp_sim::inventory::set_chambered(&mut carried.equipment.item, gun, None);
            let rules = carried.rules;
            let (put, left) = trp_sim::inventory::pick_up(&mut carried.equipment.item, Item::new(kind), &rules);
            if let (Err(_), Some(left)) = (put, left) {
                let m = &controller.movement;
                carried.ground.push((left, beside(m.position, m.yaw, [DROP_AHEAD, 0.2])));
            }
            carried.note = "the round was not put into the chamber".to_owned();
        }
    }
    if let (Some(firearm), Some((was_gun, was)), Some((gun, now))) = (firearm.as_deref_mut(), hands.fitted, fitted) {
        let same = was_gun == gun && was == now;
        let in_item = round_in(&*carried, gun);
        if same && firearm.single_chamber() && in_item.is_some() && !firearm.chamber && firearm.can_load_chamber() {
            if firearm.load_chamber() {
                hands.loading_chamber = Some(gun);
            }
        } else if same && firearm.single_chamber() && in_item.is_none() && firearm.chamber && firearm.can_discharge() {
            // The chambered round the inventory has just taken out (firing
            // spec, "The chambered round taken out from the inventory"): the
            // hands discharge it, the move let only where they would; the
            // chamber is empty at their mark. (It had been put back, of the
            // magazine's top kind or the weapon's own: the user's report of
            // 2026-10-25, "PS spawned in the chamber infinitely".) `PROVISIONAL`:
            // the round moved at once, where the game moves it at the mark.
            firearm.discharge();
        } else if same && firearm.can_move_magazine() {
            hands.loading_chamber = None;
            // The weapon in hand's magazine, or tube, as its logic counts it
            // (inventory spec, "Checking a magazine"): at rest, rounds its
            // logic has fired or chambered are gone from the item too, the
            // magazine known or not by the game's rule, a shot unknowing it
            // where it is left neither empty nor full (`UncheckOnShot`). A
            // cylinder's rounds lie in chambers of its own: left alone.
            let mut fed = None;
            let held = now.and_then(|magazine| carried.find(magazine).map(|item| (magazine, trp_sim::inventory::magazine_rounds(item), item.kind.capacity)));
            if let (Some(count), Some((magazine, rounds, capacity))) = (firearm.magazine.filter(|_| firearm.single_chamber()), held) {
                if count < rounds {
                    // The last round to leave it is the one now chambered.
                    fed = carried.find(magazine).and_then(|item| trp_sim::inventory::magazine_round_at(item, rounds - count - 1));
                    trp_sim::inventory::set_rounds_of(&mut carried.equipment.item, magazine, count);
                    let shot = std::mem::take(&mut hands.fired) && carried.round_times.uncheck_on_shot;
                    if trp_sim::inventory::known_after(count, capacity, carried.checked.contains(&magazine), shot) {
                        carried.checked.insert(magazine);
                    } else {
                        carried.checked.remove(&magazine);
                    }
                }
            }
            // A failure to feed's round, out of the magazine and not into
            // the chamber (`MalfState.MalfunctionedAmmo`): kept with the
            // weapon for its repair to throw out (`crate::wear`).
            if firearm.malfunction() == trp_sim::malfunction::Malf::Feed && !firearm.chamber {
                if let (Some(kind), Some(gun)) = (fed.clone(), carried.equipment.item.find_mut(gun)) {
                    gun.fed_round = Some(kind);
                }
                fed = None;
            }
            // Its chamber of its own as its logic has it: a round fed is
            // the one in it, one fired or thrown out is gone.
            if firearm.single_chamber() {
                match (firearm.chamber, in_item.is_some()) {
                    (true, false) | (true, true) if fed.is_some() => {
                        trp_sim::inventory::set_chambered(&mut carried.equipment.item, gun, fed);
                    }
                    (true, false) => {
                        let top = now.and_then(|magazine| carried.find(magazine)).and_then(|item| trp_sim::inventory::magazine_round_at(item, 0));
                        let own = || game.db.items.get(&carried.equipment.item.find(gun)?.kind.template)?.props.get("defAmmo")?.as_str().and_then(|round| trp_sim::inventory::Kind::of(&game.db, round).ok()).map(std::sync::Arc::new);
                        if let Some(kind) = top.or_else(own) {
                            trp_sim::inventory::set_chambered(&mut carried.equipment.item, gun, Some(kind));
                        }
                    }
                    (false, true) => {
                        trp_sim::inventory::set_chambered(&mut carried.equipment.item, gun, None);
                    }
                    _ => {}
                }
            }
        }
    }
    if hands.fitted.map(|(gun, _)| gun) != fitted.map(|(gun, _)| gun) {
        hands.fired = false;
    }
    hands.fitted = fitted;
    // Nothing goes into the chamber of the weapon in hand whose hands would
    // not take it now.
    let closed = hands.item.filter(|_| !firearm.as_deref().is_some_and(|firearm| firearm.can_load_chamber()));
    if carried.rules.chamber_closed != closed {
        carried.bypass_change_detection().rules.chamber_closed = closed;
    }
    // And nothing leaves it where they would not let the round out.
    let round_kept = hands.item.filter(|_| !firearm.as_deref().is_some_and(|firearm| firearm.can_discharge()));
    if carried.rules.chamber_held != round_kept {
        carried.bypass_change_detection().rules.chamber_held = round_kept;
    }
    let busy = hands.item.filter(|_| firearm.is_some_and(|firearm| !firearm.can_move_magazine()));
    if carried.rules.busy_weapon != busy {
        carried.bypass_change_detection().rules.busy_weapon = busy;
    }
    let carried = &*carried;
    let (Some(mut rig), Some(mut weapon)) = (rig, weapon) else { return };
    let (rig, weapon, controller) = (&mut *rig, &mut *weapon, &mut *controller);
    let open = controller.inventory_open;
    // What hangs in each slot a weapon is held from, of the kinds built.
    let built: Vec<String> = stowed.as_deref().map_or_else(|| vec![options.weapon.clone()], |stowed| stowed.built.clone());
    // A weapon is by its build (`trp_sim::inventory::rig_key`); the one in
    // hand hangs where it hangs while the rig of a build it has just been
    // given is still being made (it keeps its old one meanwhile).
    let in_hand = hands.item;
    let hangs = |slot: &str| carried.equipment.worn(slot).filter(|weapon| Some(weapon.id) == in_hand || built.contains(&trp_sim::inventory::rig_key(weapon))).map(|weapon| weapon.id);
    let kind_of = |id: Id| carried.equipment.item.find(id).map(trp_sim::inventory::rig_key);
    let folded_of = |id: Id| carried.equipment.item.find(id).and_then(trp_sim::inventory::folding_part).is_some_and(|part| part.folded);
    let magazine_of = |id: Id| {
        let gun = carried.equipment.item.find(id);
        let fitted = gun.and_then(|gun| gun.in_slot(trp_sim::inventory::MAGAZINE_SLOT));
        (gun.map(|_| fitted.map(trp_sim::inventory::magazine_rounds)), gun.filter(|gun| trp_sim::inventory::chamber_slot(gun).is_some()).map(|gun| trp_sim::inventory::chambered(gun).is_some()), fitted.map(|magazine| magazine.kind.magazine_animation))
    };
    let slot_of = |id: Id| HAND_SLOTS.into_iter().find(|slot| hangs(slot) == Some(id));
    let first_but = |but: Option<Id>| HAND_SLOTS.into_iter().find_map(|slot| hangs(slot).filter(|id| Some(*id) != but).map(|id| (id, slot)));
    // A slot's key, or a quick-use key bound to a weapon.
    // With the weapon's handling switched off nothing is drawn or put
    // away by a key (settings spec, "Features").
    let live = !open && options.keys_live() && features.weapon_handling;
    // The holster's key pressed twice takes the holster's weapon quick
    // (`QuickSelectSecondaryWeapon`: `SetSlotItem(1, fast)`); every slot
    // taken says whether it was so (`FastSlotSelection`).
    let quick_pistol = live && bindings.fired("QuickSecondaryWeapon");
    let pressed = if quick_pistol { Some("Holster") } else { SLOT_KEYS.iter().find(|(key, _)| live && bindings.fired(key)).map(|(_, slot)| *slot) };
    if pressed.is_some() || quick.is_some() {
        hands.fast = quick_pistol;
    }
    let quick_draw = trp_sim::quick_draw::QuickDraw::read(&game.db.config);
    let level_of = |weapon: &HandsRig| skills.as_deref().filter(|s| s.on()).zip(weapon.class).map_or(0, |(s, class)| s.levels.level(class));
    let pistol = |id: Option<Id>| {
        id.and_then(|id| carried.equipment.item.find(id)).and_then(|item| game.db.items.get(&item.kind.template)).and_then(|t| t.props.get("weapClass")).and_then(|c| c.as_str())
            == Some("pistol")
    };
    // A key for a weapon whose rig is still to come waits for it.
    let worn_in = |slot: &str| carried.equipment.worn(slot);
    let coming = |slot: &str| worn_in(slot).filter(|weapon| hangs(slot).is_none() && stowed.as_deref().is_some_and(|stowed| stowed.awaited(&trp_sim::inventory::rig_key(weapon))));
    if let Some(slot) = pressed {
        hands.waiting = coming(slot).map(|weapon| (weapon.id, slot));
    }
    let waited = hands.waiting.filter(|(id, slot)| live && hangs(slot) == Some(*id));
    if waited.is_some() || hands.waiting.is_some_and(|(id, slot)| worn_in(slot).is_none_or(|weapon| weapon.id != id) || coming(slot).is_none() && hangs(slot).is_none()) {
        hands.waiting = None;
    }
    let key = pressed.and_then(|slot| hangs(slot).map(|id| (id, slot))).or(waited);
    let asked_for = key.or_else(|| quick.filter(|_| live).and_then(|id| slot_of(id).map(|slot| (id, slot)))).or_else(|| bench_take.and_then(|id| slot_of(id).map(|slot| (id, slot))));
    // A quick-use key's item to be used: its rig made first if it has none
    // yet (as a weapon's key waits for its rig), and a key for anything
    // else meanwhile forgets it.
    let item_of = |id: Id| carried.equipment.item.find(id).filter(|item| !is(item, "Weapon"));
    if let Some(id) = quick.filter(|id| live && item_of(*id).is_some()) {
        (hands.use_waiting, hands.use_part, hands.use_amount, hands.use_quick) = (Some(id), None, None, false);
    } else if asked_for.is_some() {
        hands.use_waiting = None;
    }
    // The quick stroke (`PlayerInputTranslator.method_4`): the sheath's
    // knife, with no knife in hand already, with stamina for a stroke.
    if quick_knife && live {
        let knife = carried.equipment.worn("Scabbard").filter(|knife| is(knife, "Knife")).map(|knife| knife.id);
        let knife_in_hand = hands.item.is_some_and(|id| Some(id) == knife) || weapon.knife.as_ref().is_some_and(|knife| knife.in_hand());
        match knife {
            Some(id) if !knife_in_hand && controller.movement.stamina.can_melee() => {
                (hands.use_waiting, hands.use_part, hands.use_amount, hands.use_quick) = (Some(id), None, None, true);
            }
            // Too tired (`Physical.InvokeInsufficient`).
            Some(_) if !knife_in_hand => info!("the quick stroke: too tired for it"),
            _ => {}
        }
    }
    // An item let go on a part of the HEALTH tab, or used from the screen,
    // with the screen open.
    if let Some((id, part, amount)) = dropped.filter(|(id, ..)| features.weapon_handling && item_of(*id).is_some()) {
        (hands.use_waiting, hands.use_part, hands.use_amount) = (Some(id), part, amount);
    }
    let (use_part, use_amount) = (hands.use_part, hands.use_amount);
    let mut use_ready = None;
    if let Some(id) = hands.use_waiting {
        match (item_of(id).map(|item| item.kind.template.clone()), stowed.as_deref()) {
            (Some(template), Some(stowed)) if stowed.built.contains(&template) => use_ready = Some(id),
            (Some(template), Some(stowed)) if stowed.awaited(&template) => {
                if let Some(preparing) = preparing.as_deref_mut() {
                    preparing.ask(&template);
                }
            }
            _ => hands.use_waiting = None,
        }
        if use_ready.is_some() {
            hands.use_waiting = None;
        }
    }
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
    // An item is taken in hand to be used (health spec, "The hands"): its
    // rig is changed for whatever was there, and its controller puts it to
    // work as it spawns, on the common body as a quick-use key names it
    // (`TargetBodyParts`), food and drink all that is left. Refused (it can
    // do nothing after all), it gives way as a use cut short does.
    let quick_stroke = hands.use_quick;
    let take_item = |show: &mut dyn FnMut(Option<Entity>, bool), rig: &mut WeaponRig, weapon: &mut HandsRig, controller: &mut Controller, stowed: Option<&mut Stowed>, id: Id, (part, amount): (Option<trp_sim::health::Part>, Option<f32>)| {
        let (Some(stowed), Some(item)) = (stowed, carried.equipment.item.find(id)) else { return false };
        if let Some(empty) = weapon.empty.as_mut() {
            empty.hide();
        }
        if stowed.held().is_some() {
            show(rig.weapon_root, false);
        }
        if !change(stowed, Some(&item.kind.template), rig, weapon, controller) {
            return false;
        }
        show(rig.weapon_root, true);
        show(Some(rig.root), true);
        controller.armed = false;
        if let (Some(meds), Some(health)) = (weapon.meds.as_mut(), controller.health.as_mut()) {
            let resource = item.resource.unwrap_or_else(|| meds.medicine().full());
            // Food and drink go to the body whole, whatever part they
            // were let go on (`PlayerHealthController.ApplyItem`).
            let parts = if meds.medicine().food() { [None] } else { [part] };
            if meds.take(health, id, resource, &parts, amount, None).is_none() {
                meds.drop();
            }
        }
        // A grenade is drawn, the rangefinder too; the knife strikes its
        // quick stroke.
        if let Some(grenade) = weapon.grenade.as_mut() {
            grenade.draw();
        }
        if let Some(rangefinder) = weapon.rangefinder.as_mut() {
            rangefinder.draw();
        }
        if let Some(knife) = weapon.knife.as_mut() {
            let speed = controller.movement.stamina.melee_speed();
            if quick_stroke {
                knife.draw_quick(speed);
            } else {
                knife.draw();
            }
        }
        first_pose(weapon, controller);
        true
    };
    let take = |show: &mut dyn FnMut(Option<Entity>, bool), rig: &mut WeaponRig, weapon: &mut HandsRig, controller: &mut Controller, stowed: Option<&mut Stowed>, kind: Option<String>, (folded, magazine): (bool, (Option<Option<u32>>, Option<bool>, Option<i32>))| {
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
        fold_as_item(weapon, rig, controller, folded, magazine);
        weapon.draw_held();
        first_pose(weapon, controller);
    };
    if first {
        // As the character is made: with the first weapon it has in hand
        // and at rest, or with none.
        match first_but(None) {
            Some((id, slot)) => {
                (hands.item, hands.active, hands.last_weapon) = (Some(id), Some(slot), Some(id));
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
                fold_as_item(weapon, rig, controller, folded_of(id), magazine_of(id));
                if let Some(firearm) = weapon.firearm.as_mut() {
                    // One it began with in hand was drawn before: a
                    // round is chambered (its item's from its magazine's
                    // top, `settle` at rest).
                    firearm.carried();
                    firearm.start_drawn();
                }
                if let Some(knife) = weapon.knife.as_mut() {
                    knife.start_drawn();
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
                match stowed.as_deref_mut() {
                    // Begun with the empty hands' rig in hand, every weapon's
                    // made when first wanted (`weapon::spawn_weapon`): they
                    // come up as they do when a weapon is put away.
                    Some(stowed) if stowed.held().is_none() => {
                        controller.movement.body.set_weapon_width(rig.width);
                        controller.movement.body.set_hands_kind(rig.kind);
                        if let Some(empty) = weapon.empty.as_mut() {
                            empty.draw();
                        }
                        first_pose(weapon, controller);
                    }
                    stowed => empty_handed(&mut show, rig, weapon, controller, stowed),
                }
            }
        }
    } else if let Some(using) = hands.using {
        // An item in the hands: a key for anything else has it give way
        // (`MedsController.Drop`: its work cut short 0.6 s on), and what
        // was asked for comes when it has been put away; a use over by
        // itself gives the hands back to the weapon last held, or the first
        // in the slots' order, or the empty hands
        // (`SetControllerInsteadRemovedOne`, `TrySetLastEquippedWeapon`).
        let other_use = use_ready.filter(|id| *id != using);
        if asked_for.is_some() || other_use.is_some() {
            (hands.next, hands.next_use) = (asked_for, other_use);
            // A firearm put away for the holster's weapon taken quick, not
            // lying (`Remove.Start`, `CheckForFastWeaponSwitch`): the arms
            // pay, and it goes away at its quick speed. PROVISIONAL: read
            // from code, not recorded, and not run (no start has a rifle in
            // hand and a pistol in the holster).
            let quick_away = asked_for.is_some_and(|(_, slot)| slot == "Holster") && hands.fast && !controller.movement.prone && controller.armed;
            if quick_away && weapon.firearm.is_some() {
                let now = controller.movement.frame_clock();
                let level = level_of(weapon);
                controller.movement.stamina.fast_switch(level, now);
                let speed = quick_draw.map(|q| q.speed(pistol(hands.item), level, true));
                if let (Some(firearm), Some(speed)) = (weapon.firearm.as_mut(), speed) {
                    firearm.set_animator_speeds(1.0, speed);
                }
                hands.quickdraw = Some(now);
            }
            weapon.give_way();
        }
        if gone || !weapon.using() {
            hands.using = None;
            match hands.next_use.take() {
                Some(id) => {
                    if take_item(&mut show, rig, weapon, controller, stowed.as_deref_mut(), id, (use_part, use_amount)) {
                        hands.using = Some(id);
                    }
                }
                None => {
                    let back = hands.last_weapon.and_then(|id| slot_of(id).map(|slot| (id, slot)));
                    match hands.next.take().filter(|(id, slot)| hangs(slot) == Some(*id)).or(back).or_else(|| first_but(None)) {
                        Some((id, slot)) => {
                            (hands.item, hands.active, hands.last_weapon) = (Some(id), Some(slot), Some(id));
                            take(&mut show, rig, weapon, controller, stowed.as_deref_mut(), kind_of(id), (folded_of(id), magazine_of(id)));
                            // The holster's weapon drawn within a second of
                            // the quick put-away (`SetAnimatorAndProceduralValues`):
                            // the arms pay again and it is drawn quick, the
                            // reload's speed 1. (The hand shake when that
                            // leaves the arms empty is not built.)
                            let now = controller.movement.frame_clock();
                            if slot == "Holster" && hands.quickdraw.take().is_some_and(|at| now - at < trp_sim::quick_draw::QUICK_WITHIN) {
                                let level = level_of(weapon);
                                controller.movement.stamina.fast_switch(level, now);
                                let speed = quick_draw.map(|q| q.speed(pistol(Some(id)), level, false));
                                if let (Some(firearm), Some(speed)) = (weapon.firearm.as_mut(), speed) {
                                    firearm.set_animator_speeds(1.0, speed);
                                }
                            }
                        }
                        None => empty_handed(&mut show, rig, weapon, controller, stowed.as_deref_mut()),
                    }
                }
            }
        }
    } else if controller.armed {
        // The weapon in hand built anew (a part put on or taken off, in the
        // inventory or at the workbench, or a magazine of another kind put
        // in): once its new build's rig is in the world, the hands take it
        // in the old one's place. At rest (the logic idle and its animation
        // over), with what that one had in it (`Firearm::take_over`) and its
        // item's magazine and chamber; through a reload or a magazine put
        // in, while no magazine is in sight, going on with it where the old
        // one was (`Firearm::carry_on_from`: its operation, its animator's
        // states and times), its magazine hidden until the new one shows.
        // The screen as it stands is told to it afresh. The app's: the game
        // changes the part on the weapon's own model.
        let at_rest = weapon.firearm.as_ref().is_some_and(|firearm| firearm.can_move_magazine() && !firearm.hands_busy());
        let unseen = weapon.firearm.as_ref().is_some_and(|firearm| firearm.magazine_out_of_sight());
        // A magazine going in already shown: the game's model shows the new
        // one from `MagShow` on. Where the new build's rig was not there by
        // then, the old one's magazine (another kind) is not shown, and the
        // new rig takes over the moment it is there, the reload going on.
        let shown_in = weapon.firearm.as_ref().is_some_and(|firearm| firearm.magazine_shown_going_in());
        if shown_in && !hands.going {
            let other = hands.item.and_then(kind_of).zip(stowed.as_deref()).is_some_and(|(key, stowed)| stowed.held().is_some_and(|held| held != key.as_str()));
            if other {
                controller.magazine_drawn = Some(false);
            }
        }
        if !hands.going && (at_rest || unseen || shown_in) {
            if let (Some(id), Some(stowed)) = (hands.item, stowed.as_deref_mut()) {
                if let Some(key) = kind_of(id).filter(|key| stowed.held().is_some_and(|held| held != key.as_str()) && stowed.built.contains(key)) {
                    let before = stowed.held().map(str::to_owned);
                    show(rig.weapon_root, false);
                    if change(stowed, Some(&key), rig, weapon, controller) {
                        let old_hands = before.as_deref().and_then(|before| stowed.hands_of(before));
                        let old = old_hands.and_then(|old| old.firearm.as_ref());
                        let mut carried_on = false;
                        if let (Some(firearm), Some(old)) = (weapon.firearm.as_mut(), old) {
                            carried_on = !at_rest && firearm.carry_on_from(old);
                            if !carried_on {
                                firearm.take_over(old);
                            }
                        }
                        // The weapon's motion goes on as it was, and the
                        // frames still to be drawn keep their poses on the
                        // new rig's bones.
                        let map = before.as_deref().and_then(|before| stowed.rig_of(before)).filter(|_| carried_on).map(|old_rig| crate::weapon::bone_map(old_rig, rig));
                        if let (true, Some(old_hands)) = (carried_on, old_hands) {
                            weapon.hands.carry_on_from(&old_hands.hands);
                        }
                        show(rig.weapon_root, true);
                        show(Some(rig.root), true);
                        if carried_on {
                            // (The logic has its magazine as the reload
                            // goes; the new build's is out of sight, or
                            // already shown.)
                            fold_as_item(weapon, rig, controller, folded_of(id), (None, None, None));
                            controller.magazine_drawn = Some(shown_in && !unseen);
                        } else {
                            fold_as_item(weapon, rig, controller, folded_of(id), magazine_of(id));
                            if let Some(firearm) = weapon.firearm.as_mut() {
                                firearm.start_drawn();
                            }
                        }
                        match map {
                            Some(map) => {
                                if let Some(pose) = weapon.animator().map(|animator| animator.pose_now()) {
                                    let read = weapon.frame.read(&pose);
                                    controller.rig_carried_over(&map, pose, read);
                                }
                            }
                            None => first_pose(weapon, controller),
                        }
                        hands.told_open = !open;
                        info!("the weapon in hand built anew: its rig {key}{}", if carried_on { ", the reload going on" } else { "" });
                    }
                }
            }
        }
        // The weapon in hand has left the slot it was drawn from: it is
        // put away, and the next in the slots' order is taken. Or a key
        // asks for another, or for an item to be used: it is put away for
        // that one.
        let held = hands.item;
        let left = held.is_none_or(|id| slot_of(id) != hands.active);
        if !hands.going {
            let other = asked_for.filter(|(id, _)| Some(*id) != held);
            if bench_put_away && !left && other.is_none() && use_ready.is_none() {
                // Put away for the workbench: the hands are left empty,
                // and the slot is not drawn from again until it asks.
                (hands.going, hands.next, hands.next_use, hands.holstered) = (true, None, None, true);
                weapon.hide_held();
            } else if left || other.is_some() || use_ready.is_some() {
                hands.going = true;
                hands.next = if left && use_ready.is_none() { first_but(held) } else { other.filter(|_| use_ready.is_none()) };
                hands.next_use = use_ready;
                // Taken out of its slot, it waits on the hands to put it
                // away (`Player.TryRemoveFromHands`).
                if left {
                    hands.moved_out = held;
                }
                weapon.hide_held();
            }
        } else if let Some(id) = use_ready {
            // A key pressed while it is going takes the place of what
            // was to come next.
            (hands.next, hands.next_use) = (None, Some(id));
        } else if let Some(other) = asked_for.filter(|(id, _)| Some(*id) != held || left) {
            (hands.next, hands.next_use) = (Some(other), None);
        }
        if gone || (hands.going && !weapon.held_in_hand()) {
            (hands.going, hands.item) = (false, None);
            controller.armed = false;
            // An item to be used; else what was to come next, if it still
            // hangs where it did.
            let next_use = hands.next_use.take();
            if let Some(id) = next_use.filter(|id| take_item(&mut show, rig, weapon, controller, stowed.as_deref_mut(), *id, (use_part, use_amount))) {
                hands.using = Some(id);
                hands.moved_out = None;
            } else {
                match hands.next.take().filter(|(id, slot)| hangs(slot) == Some(*id)) {
                    Some((id, slot)) => {
                        (hands.item, hands.active, hands.last_weapon) = (Some(id), Some(slot), Some(id));
                        take(&mut show, rig, weapon, controller, stowed.as_deref_mut(), kind_of(id), (folded_of(id), magazine_of(id)));
                    }
                    None => {
                        empty_handed(&mut show, rig, weapon, controller, stowed.as_deref_mut());
                        hands.moved_out = None;
                    }
                }
            }
        }
    } else if let Some(id) = use_ready.filter(|id| take_item(&mut show, rig, weapon, controller, stowed.as_deref_mut(), *id, (use_part, use_amount))) {
        // Empty hands: an item to be used is taken at once, the empty
        // hands going as they do for a weapon.
        hands.using = Some(id);
    } else {
        // Empty hands: a key's weapon, or one put into the slot the
        // hands last drew from, is taken at once.
        let back = hands.active.filter(|_| !hands.holstered).and_then(|slot| hangs(slot).map(|id| (id, slot)));
        if let Some((id, slot)) = asked_for.or(back) {
            (hands.item, hands.active, hands.last_weapon, hands.holstered) = (Some(id), Some(slot), Some(id), false);
            take(&mut show, rig, weapon, controller, stowed.as_deref_mut(), kind_of(id), (folded_of(id), magazine_of(id)));
            // The workbench's weapon built anew comes up racking.
            if bench_take == Some(id) && bench_rack {
                if let Some(firearm) = weapon.firearm.as_mut() {
                    firearm.rack_drawn();
                }
            }
            // Put there by a move, it waits on the hands to take it
            // (`Player.TrySetInHands`).
            if asked_for.is_none() {
                hands.moved_in = Some(id);
            }
        }
    }
    // The workbench's weapon in hand again (or gone from the character):
    // its ask is answered.
    if let Some(id) = asked.3.take {
        if (hands.item == Some(id) && !hands.going) || carried.equipment.item.find(id).is_none() || !features.weapon_handling {
            (asked.3.take, asked.3.rack) = (None, false);
        }
    }
    // The hands are done with a move when what they took has appeared in
    // them (`SpawnOperation.WeaponAppeared`, at the draw's last mark).
    if appeared {
        (hands.moved_in, hands.moved_out) = (None, None);
    }
    // What blinks: those, and a weapon put into the slot the hands last
    // drew from while its rig is still being made (the hands' taking it
    // waits for that here).
    let rig_coming = (!controller.armed && !hands.going)
        .then(|| hands.active.and_then(|slot| carried.equipment.worn(slot)))
        .flatten()
        .filter(|weapon| Some(weapon.id) != hands.item && stowed.as_deref().is_some_and(|stowed| stowed.awaited(&trp_sim::inventory::rig_key(weapon))))
        .map(|weapon| weapon.id);
    let now: Vec<Id> = hands.moved_in.into_iter().chain(hands.moved_out).chain(rig_coming).chain(hands.magazine_moving).collect();
    if blinking.0 != now {
        blinking.0 = now;
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
        if let Some(grenade) = weapon.grenade.as_mut() {
            grenade.set_inventory(open);
        }
        if let Some(rangefinder) = weapon.rangefinder.as_mut() {
            rangefinder.set_inventory(open);
        }
        if let Some(knife) = weapon.knife.as_mut() {
            knife.set_inventory(open);
        }
    }
    // What the hands hold, and a change of them begun, for the battle
    // screen's quick bar.
    let (item, using, going) = hands.was;
    // (Not the first weapon taken as the character is made: it is in the
    // hands before the battle screen shows.)
    let began = !first && ((hands.going && !going) || (!going && item.is_none() && hands.item.is_some()) || (using.is_none() && hands.using.is_some()));
    if began {
        held.changes = held.changes.wrapping_add(1);
    }
    hands.was = (hands.item, hands.using, hands.going);
    let now = hands.using.or(hands.item);
    if held.item != now {
        held.item = now;
    }
}

/// What is in the hands (a weapon, or an item being used) and how many
/// changes of the hands have begun (`Player.HandsChangingEvent`), for the
/// battle screen's quick bar (`battle_view`).
#[derive(Resource, Default)]
pub struct HandsHeld {
    pub item: Option<Id>,
    pub changes: u32,
}


/// What is looked at from within the game's reach for what can be opened
/// or picked up (`Player.InteractionRaycast`): a thing on the ground, or a
/// door, container or trunk of the range (a door within
/// `DOOR_RAYCAST_DISTANCE`, the rest within `LOOT_RAYCAST_DISTANCE`), and
/// the prompt for what the game offers to do to it. Nothing is looked at
/// unless the movement state lets the player (`CanInteract` of its
/// animator state) and the thing in hand does (a rifle only while its
/// logic rests or fires; an item being used never).
pub(crate) fn reach(
    mut commands: Commands,
    view: Res<View>,
    range: Res<Range>,
    game: Res<GameData>,
    mut controller: ResMut<Controller>,
    carried: Res<Carried>,
    bindings: Res<Bindings>,
    texts: Res<MenuTexts>,
    hands: Option<Res<HandsRig>>,
    features: Res<crate::options_view::Features>,
    camera: Query<&GlobalTransform, With<crate::weapon::Eye>>,
    mut reach: ResMut<Reach>,
    mut prompt: Query<(Entity, &mut Visibility), With<Prompt>>,
    mut shown: Local<Option<(Menu, String)>>,
    (loose, skills, look): (Res<crate::loose::Loose>, Res<crate::skills::Skills>, Res<crate::health_view::HealthLook>),
) {
    // The game's line starts a little behind the point the player looks
    // from (`BEHIND_CAST`) and ends the reach ahead of it.
    let float = |name: &str, or: f32| game.hard.float(name).unwrap_or(or);
    let (within, behind, door) = (float("LOOT_RAYCAST_DISTANCE", 1.3), float("BEHIND_CAST", 0.15), float("DOOR_RAYCAST_DISTANCE", 1.0));
    // Nothing is offered while a thing is being taken up, a door kicked,
    // or a container searched is still to be shown.
    let busy = controller.movement.acting() || controller.movement.searching() || controller.movement.kicking();
    // An item being used lets nothing be done (`MedsController.CanInteract`).
    let hands_let = hands.as_deref().and_then(|hands| hands.firearm.as_ref()).is_none_or(|firearm| firearm.can_interact())
        && hands.as_deref().and_then(|hands| hands.meds.as_ref()).is_none_or(|meds| !meds.in_hand());
    let allowed = controller.movement.can_interact() && hands_let;
    // With the options' `WORLD INTERACTIONS` off nothing is looked at to
    // be worked; with `INVENTORY SYSTEM` off nothing that would go into the
    // inventory is (a thing on the ground, a container to search).
    let playing = view.played() && !controller.inventory_open && !controller.edit_build && !busy && allowed && features.world_interactions;
    let mut seen = Reach::default();
    let mut loose_takes = false;
    if let (true, Ok(eye)) = (playing, camera.single()) {
        let (from, to) = (eye.translation() - eye.forward() * behind, eye.translation() + eye.forward() * within);
        seen.thing = range.looked_at(from, to, door + behind).filter(|index| features.inventory || range.things[*index].holds.is_none());
        // A thing on the ground is looked at when the line passes within
        // a hand's breadth of it; the nearest along the line is meant.
        let along = to - from;
        let near = |at: Vec3| {
            let share = ((at - from).dot(along) / along.length_squared()).clamp(0.0, 1.0);
            ((from + along * share).distance(at) < LOOSE_REACH).then_some(share)
        };
        // (A thing the physics has, by its centre of mass; one that lies
        // as a box, by the box's middle.)
        let centre = |item: &Item, at: Vec3| loose.places.get(&item.id).map_or(at + Vec3::Y * 0.06, |place| place.centre);
        let looked_at = carried.ground.iter().filter_map(|(item, at)| near(centre(item, *at)).map(|share| (share, item)));
        // Every loose thing is looked at; the game offers to take one only
        // if a pick-up would find it a place (`GetAvailableInteractionState`),
        // which a pick-up under way has.
        let nearest = looked_at.min_by(|a, b| a.0.total_cmp(&b.0)).map(|(_, item)| item);
        let already = |id: Id| reach.loose == Some(id) && reach.thing == seen.thing;
        seen.loose = nearest.filter(|_| features.inventory).map(|item| item.id);
        loose_takes = nearest.is_some_and(|item| already(item.id) || can_pick_up(&carried.equipment.item, item, &carried.rules));
        // Nothing looked at: the sense (`Player.InteractionRaycast`,
        // `GameWorld.InteractionSense`): a sphere cast 1.5 m along the same
        // line, 0.1 m wide times one and Perception's `PerceptionLootDot`
        // (elite: from 3 m over the character straight down, 1.1 m wide,
        // 2.35 m long), finding a loose thing. PROVISIONAL: a loose thing
        // is found where the sphere comes within the look's own hand's
        // breadth of its middle (`LOOSE_REACH`), the thing's colliders not
        // being had; and the not aiming is the weapon's (`Player.Sense`).
        if seen.thing.is_none() && seen.loose.is_none() && features.inventory {
            let perception = skills.level(trp_sim::skills::Skill::Perception);
            let buff = |id: &str| trp_sim::skills::buffs_of(trp_sim::skills::Skill::Perception).iter().find(|buff| buff.id == id).map(|buff| buff.value(perception));
            let elite = matches!(buff("PerceptionmEliteNoIdea"), Some(trp_sim::skills::BuffValue::Flag(true)));
            let dot = match buff("PerceptionLootDot") {
                Some(trp_sim::skills::BuffValue::Figure(figure)) => figure,
                _ => 0.0,
            };
            let (start, way, length, radius) = if elite {
                let feet = controller.movement.position;
                (Vec3::new(feet[0], feet[1] + 3.0, -feet[2]), Vec3::NEG_Y, 2.35, 1.1)
            } else {
                (from, eye.forward().as_vec3(), 1.5, 0.1 * (1.0 + dot))
            };
            let aimed = hands.as_deref().and_then(|hands| hands.firearm.as_ref()).is_some_and(|firearm| firearm.aiming());
            seen.sense = !aimed
                && carried.ground.iter().any(|(item, at)| {
                    let middle = centre(item, *at);
                    let along = (middle - start).dot(way).clamp(0.0, length);
                    (start + way * along).distance(middle) < radius + LOOSE_REACH
                });
        }
    }
    // The game's menu for it (`InteractionContextHelper.GetAvailableActions`):
    // a thing on the ground is taken; a door, a container or a trunk
    // offers what its kind and its state have, a door's kick only from
    // where the door lets it be made and not lying down, a lock's key
    // only to a character that carries it.
    let feet = controller.movement.position;
    let lying = matches!(controller.movement.state, State::ProneIdle | State::ProneMove | State::FromProne);
    let threshold = float("DOOR_BREACH_THRESHOLD", 0.8);
    // With nothing else looked at, the workbench when its triggers have the
    // character (`range::Workbench::in_sight`): its line, which a weapon in
    // hand lets be chosen (the user's: "opens the edit build menu for the
    // current gun in-hand").
    seen.workbench = playing && seen.thing.is_none() && seen.loose.is_none() && range.workbench.is_some_and(|bench| bench.in_sight(Vec3::new(feet[0], feet[1], -feet[2]), range.radius));
    let armed = hands.as_deref().is_some_and(|hands| hands.firearm.is_some()) && controller.armed;
    // A loose thing (`InteractionContextHelper.GetAvailableActions`): TAKE
    // where a pick-up finds it a place; SEARCH for a container that is no
    // quest's (`IsContainer`); with neither, no lines and the panel's error
    // "No Space ({0})" with its name. (EXAMINE does not come: everything
    // counts as examined here; MODDING, an armour's slots opened as a
    // container, is not built.) Its name is its short name, a stack's
    // count in brackets after it (`LootItem.Name`, `TargetName`).
    let (mut menu, name) = match (seen.loose.and_then(|id| carried.ground.iter().find(|(item, _)| item.id == id)), seen.thing) {
        (Some((item, _)), _) => {
            let mut lines = Vec::new();
            if loose_takes {
                lines.push((Action::Take, false));
            }
            if Carried::opens(item) && !item.kind.quest {
                lines.push((Action::Search, false));
            }
            let name = if item.count > 1 { format!("{} ({})", carried.name(item), item.count) } else { carried.name(item) };
            if lines.is_empty() {
                seen.error = Some(texts.0.get(&NO_SPACE.to_lowercase()).cloned().unwrap_or_else(|| NO_SPACE.to_owned()).replace("{0}", &name));
            }
            seen.name = Some(name.clone());
            ((!lines.is_empty()).then(|| Menu::of(lines)), name)
        }
        (None, Some(index)) => {
            let placed = &range.things[index];
            let has_key = placed.thing.key.as_deref().is_some_and(|key| carries(&carried.equipment.item, key));
            // Less a door's three lines the game shows but never lets be
            // chosen (`NEVER_SHOWN`); the first line that can be is chosen
            // as before.
            let menu = placed.thing.menu(feet, has_key, lying, threshold).map(|menu| Menu::of(menu.lines.into_iter().filter(|(action, _)| !NEVER_SHOWN.contains(action)).collect()));
            (menu, placed.name.to_owned())
        }
        // The user's REPAIR GEAR, had only with a repair kit carried, as an
        // unlocking only with the key.
        (None, None) if seen.workbench => {
            let has_kit = carries_class(&carried.equipment.item, REPAIR_KITS_CLASS);
            (Some(Menu::of(vec![(Action::EditBuild, !armed), (Action::RepairGear, !has_kit)])), texts.0.get(WORKBENCH_NAME).cloned().unwrap_or_else(|| "Workbench".to_owned()))
        }
        (None, None) => (None, String::new()),
    };
    // The lines as they stood keep the choice the wheel made; when they
    // change (another thing, another state of it, the kick's angle
    // crossed) the menu is made afresh, with its first line chosen.
    let same = reach.thing == seen.thing && reach.loose == seen.loose && reach.workbench == seen.workbench;
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
    // With `TARKOV UI` the game's own panel is drawn (`action_view`); the
    // app's own look otherwise: every line, the chosen one light with dark
    // words, one that cannot be chosen dim, the error as a line of its own.
    let games = look.tarkov_ui && look.battle.as_ref().is_some_and(|views| views.action.is_some());
    let now = if games {
        None
    } else {
        reach.menu.clone().map(|menu| (menu, name.clone())).or_else(|| reach.error.clone().map(|error| (Menu::of(Vec::new()), error)))
    };
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

// ==== The ground shown, the screen toggled, the tabs and the sounds ====

/// A door's lines the game shows but has written as never to be chosen
/// (`InteractionContextHelper`: Bang & clear, Flash & clear, Move in),
/// which the app leaves out: the user's (2026-10-25), "Those are not
/// implemented in the game and are wasted text". The simulation's menu
/// keeps them, as the game's.
const NEVER_SHOWN: [Action; 3] = [Action::BangAndClear, Action::FlashAndClear, Action::MoveIn];

/// The game's text of the workbench area's name (`hideout_area_10_name`).
const WORKBENCH_NAME: &str = "hideout_area_10_name";
/// The game's text of the panel's error where a loose thing offers no line
/// (`InteractionContextHelper`: "No Space ({0})", the thing's name in it).
const NO_SPACE: &str = "No Space ({0})";

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
    things: Res<crate::loose::Loose>,
    shown: Query<(Entity, &LooseItem)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if !carried.is_changed() && !things.is_changed() {
        return;
    }
    // A thing the physics has is drawn by its own model (`loose`); the
    // rest as a plain box.
    let boxed = |item: &Item| things.stand_in(item.id, &trp_sim::inventory::rig_key(item));
    for (entity, loose) in &shown {
        if !carried.ground.iter().any(|(item, _)| item.id == loose.0 && boxed(item)) {
            commands.entity(entity).despawn();
        }
    }
    for (item, at) in carried.ground.iter().filter(|(item, _)| boxed(item) && !shown.iter().any(|(_, loose)| loose.0 == item.id)) {
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

/// How near the thing taken up must lie to be heard (`RemoveLootItem`'s
/// `TryPlayAtPoint` with a rolloff of 10).
const TAKEN_HEARD_WITHIN: f32 = 10.0;

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
    features: Res<crate::options_view::Features>,
    mut selected: ResMut<Selected>,
    mut started: Local<bool>,
    mut reaching: Local<Reaching>,
    (back, mut back_held, mut asked, mut sounds): (Query<&Interaction, With<GameBack>>, Local<bool>, ResMut<crate::workbench::WorkbenchAsked>, MessageWriter<Sound>),
) {
    let first = !std::mem::replace(&mut *started, true) && options.inventory;
    // Not while a thing is taken up or unlocked, nor in the approach to a
    // door (`StateLocksInventory`).
    let working = controller.movement.acting() || controller.movement.locks_inventory();
    // The inventory's key and the key that works what is looked at, as
    // the install binds them (`Tab` and `F` in the one read).
    let tab = options.keys_live() && !working && !controller.edit_build && bindings.fired("Inventory");
    // Escape shuts the screen too (`InventoryScreen.TranslateCommand`),
    // and one thing a press, as the game's tree of screens hands it down:
    // a window over the screen has it first (`windows`), the window that
    // asks how many next (`ask`), and the screen only with neither up.
    // The options tab has it while a key is being chosen or a way of
    // being pressed asked for: there Escape keeps what stood.
    let taken = !carried.windows.is_empty() || carried.asking.is_some() || options_tab.as_deref().is_some_and(|tab| tab.takes_escape());
    let escape = options.keys_live() && controller.inventory_open && keys.just_pressed(KeyCode::Escape) && !taken;
    // The game's back button, clicked: pressed on it and let go over it
    // (`ButtonFeedback.OnPointerClick`).
    let back_now = back.iter().copied().max_by_key(|interaction| match interaction {
        Interaction::Pressed => 2,
        Interaction::Hovered => 1,
        Interaction::None => 0,
    });
    let back_clicked = *back_held && back_now == Some(Interaction::Hovered);
    *back_held = back_now == Some(Interaction::Pressed);
    let escape = escape || (back_clicked && controller.inventory_open && !taken);
    // Dead, the screen shuts if it is open, and nothing opens it (the
    // game lets the inventory's view go with the character's life,
    // `Player.OnDead`).
    let dead = controller.health.as_ref().is_some_and(|health| !health.alive);
    let tab = if dead { controller.inventory_open } else { tab || escape };
    let key = !dead && options.keys_live() && !controller.inventory_open && bindings.fired("Interact") && features.world_interactions;
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
                    let picked = item.id;
                    let group = item.kind.sound.clone();
                    let (done, left) = pick_up(&mut carried.equipment.item, item, &carried.rules);
                    if done.is_ok() {
                        carried.known_on_arrival(picked);
                    }
                    // Gone from the ground whole, it is heard taken up
                    // (`LootItem.RemoveLootItem`); a stack only topped up
                    // from it stays, silent.
                    if done.is_ok() && left.is_none() && features.world_sounds && (lay - camera.single().map_or(lay, |eye| eye.translation())).length() <= TAKEN_HEARD_WITHIN {
                        sounds.write(Sound::Taken(group));
                    }
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
            // The bipod's gesture has passed its mark: the weapon's toggle is
            // over (`ToggleBipodOperation.OnBipodToggleEvent`).
            Acted::BipodToggled => {
                if let Some(firearm) = hands.as_deref_mut().and_then(|rig| rig.firearm.as_mut()) {
                    firearm.bipod_toggled();
                }
            }
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
                            carried.thrown.push((id, crate::loose::How::Thrown));
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
    if chosen == Some(Action::EditBuild) && reach.workbench {
        asked.0 = true;
        return;
    }
    // REPAIR GEAR: the screen opened for repairing (`Carried::repairing`).
    let repair_open = chosen == Some(Action::RepairGear) && reach.workbench && !controller.inventory_open;
    if repair_open {
        carried.repairing = true;
        carried.note = "a repair kit let go on what it mends repairs it".to_owned();
    }
    if let (Some(Action::Take), Some(id)) = (chosen, reach.loose) {
        if controller.movement.pick_up() {
            reaching.take = Some(id);
        }
        return;
    }
    // SEARCH on a loose container: the screen opens with it beside the
    // equipment, and it goes back where it lay when the screen shuts.
    // PROVISIONAL: at once, with no gesture and no wait (the game's goes
    // through the item's owner's interaction, not read further).
    let mut ground_search = false;
    if let (Some(Action::Search), Some(id)) = (chosen, reach.loose) {
        ground_search = carried.search_ground(id);
        if !ground_search {
            return;
        }
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
    if !(first || tab || search.is_some() || ground_search || repair_open) {
        return;
    }
    controller.inventory_open = first || search.is_some() || ground_search || repair_open || !controller.inventory_open;
    let open = controller.inventory_open;
    // The screen opens on its gear tab every time: the game makes the
    // screen afresh at each opening, with its tabs beginning at the
    // gear's (`InventoryScreen.Show`, `TabGroup`), and a container asks
    // for that tab outright (`ShowInventoryScreenLoot`). The tab last
    // looked at is not kept from one opening to the next. With the
    // options' `INVENTORY SYSTEM` off it opens on the options alone.
    let opens_on = if features.inventory { GEAR } else { OPTIONS };
    if open && selected.0 != opens_on {
        selected.0 = opens_on;
    }
    // The container searched is the one the screen shows beside the
    // equipment; at the start, with `--loot`, the range's first container
    // (the airdrop crate's until 0.96.0: the supply crate's since).
    let first_container = range.things.iter().position(|placed| placed.holds.is_some());
    let shows = search.or((first && options.loot).then_some(first_container).flatten()).filter(|_| open);
    if let Some(index) = shows {
        if let Some(holds) = range.things[index].holds {
            carried.bring(holds);
        }
    }
    // A container whose screen shuts is told to shut, with no hand; a loose
    // one searched goes back where it lay.
    if !open {
        carried.put_back_ground();
        carried.repairing = false;
        if let Some(index) = reaching.shown.take() {
            let range = &mut *range;
            if let Some(settings) = range.thing_settings.as_ref() {
                range.things[index].thing.tell(false, settings);
            }
        }
    }
    reaching.shown = shows.filter(|&index| range.things[index].holds.is_some()).or(reaching.shown.filter(|_| open));
    let with_crate = shows.is_some() || (open && carried.searched_ground.is_some());
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
    features: Res<crate::options_view::Features>,
    mut selected: ResMut<Selected>,
    mut tabs: Query<(&TabButton, &mut BackgroundColor), Without<GameTab>>,
    mut pages: Query<(&Page, &mut Node)>,
    mut shown: Local<Option<usize>>,
) {
    if let Some(index) = clicked.iter().find(|(interaction, tab)| **interaction == Interaction::Pressed && tab_open(tab.0, &features)).map(|(_, tab)| tab.0) {
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
        node.display = if page.0 == index || (page.0 == GEAR && index == HEALTH) { Display::Flex } else { Display::None };
    }
}

/// The screen's sounds as it opens and shuts (`InventoryScreen.Show`,
/// `Close`: the backpack's, where its `_playBackpackSounds` is on; read as
/// on, `PROVISIONAL`), not as the run begins.
fn screen_sounds(controller: Res<Controller>, mut was: Local<Option<bool>>, mut sounds: MessageWriter<Sound>) {
    let open = controller.inventory_open;
    if let Some(before) = was.replace(open).filter(|before| *before != open) {
        sounds.write(Sound::Ui(if before { Ui::BackpackClose } else { Ui::BackpackOpen }));
    }
}

/// A tab's sounds (its `ButtonFeedback`): the mouse come onto it, and a
/// click on it, the button let go over it; neither for a tab that does
/// not work (`Interactable`), as a tab not built here is not.
fn tab_sounds(features: Res<crate::options_view::Features>, tabs: Query<(Entity, &TabButton, &Interaction), Changed<Interaction>>, mut was: Local<HashMap<Entity, Interaction>>, mut sounds: MessageWriter<Sound>) {
    for (entity, tab, interaction) in &tabs {
        let before = was.insert(entity, *interaction).unwrap_or(Interaction::None);
        if !tab_open(tab.0, &features) {
            continue;
        }
        match (before, interaction) {
            (Interaction::None, Interaction::Hovered | Interaction::Pressed) => {
                sounds.write(Sound::Ui(Ui::ButtonOver));
            }
            (Interaction::Pressed, Interaction::Hovered) => {
                sounds.write(Sound::Ui(Ui::ButtonClick));
            }
            _ => {}
        }
    }
}

// ==== Tests ====

#[cfg(test)]
mod spin_tests {
    use super::Shown;

    /// The load component's shares (`ItemViewLoadAmmoComponent.
    /// StartAnimation`): the work's done by the time since it began over
    /// its rounds' time, the present round's by its remainder; rounds done
    /// before it began count to both.
    #[test]
    fn the_spinner_fills_by_the_work_and_the_loader_turns_by_the_round() {
        let shown = Shown { round: 0.3, to_go: 30, done: 0, since: 4.5 };
        let (done, round) = shown.shares();
        assert!((done - 0.5).abs() < 1e-6);
        assert!(round.abs() < 1e-4 || (round - 1.0).abs() < 1e-4, "{round}");
        let (_, round) = Shown { since: 0.15, ..shown }.shares();
        assert!((round - 0.5).abs() < 1e-5);
        // An unload begun with 10 out: 10 of 40 rounds' time already.
        let (done, _) = Shown { round: 0.3, to_go: 30, done: 10, since: 0.0 }.shares();
        assert!((done - 0.25).abs() < 1e-6);
        // A check: one round of its whole time.
        let (done, _) = Shown { round: 3.0, to_go: 1, done: 0, since: 4.0 }.shares();
        assert_eq!(done, 1.0);
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

    /// A thing's picture in a quick-use view as the game sizes it before
    /// turning it: a square one by its turned height alone (so that its
    /// corners reach the view's), a long one by its turned length.
    #[test]
    fn a_quick_use_picture_is_fitted_as_the_game_s() {
        let square = quick_fit([1, 1], [64.0, 64.0]);
        assert!((square.x - 64.0 / std::f32::consts::SQRT_2).abs() < 1e-3, "{square}");
        // A rifle four cells by two: (252, 126) turned is (89.1, 267.3);
        // its length bounds it, 64 / 267.3.
        let rifle = quick_fit([4, 2], [64.0, 64.0]);
        assert!((rifle.x - 252.0 * 64.0 / (378.0 / std::f32::consts::SQRT_2)).abs() < 1e-2, "{rifle}");
        assert!((rifle.x / rifle.y - 2.0).abs() < 1e-4);
    }

    /// Unity's `SmoothDamp`, which an elastic scroll view springs back by:
    /// with no time (its least, a ten-thousandth of a second) it lands within
    /// a hundredth of a unit on the first frame; with some it comes most of
    /// the way in that time and never passes the target.
    #[test]
    fn an_elastic_view_springs_back_as_unity_s() {
        let mut speed = 0.0;
        assert!(smooth_damp(-63.0, 0.0, &mut speed, 0.0, 1.0 / 120.0).abs() < 0.01);
        let (mut at, mut speed) = (-63.0f32, 0.0);
        for _ in 0..12 {
            at = smooth_damp(at, 0.0, &mut speed, 0.1, 1.0 / 120.0);
            assert!(at <= 0.0);
        }
        assert!(at > -40.0 && at < -5.0, "{at}");
        for _ in 0..240 {
            at = smooth_damp(at, 0.0, &mut speed, 0.1, 1.0 / 120.0);
        }
        assert!(at.abs() < 0.01, "{at}");
    }

    /// The containers' column's bar as the game's `Scrollbar`: a handle
    /// dragged follows the mouse over its share of the sliding area, from
    /// where it was taken; the bar held below its handle steps down a
    /// handle's share at a time, to four places, until the handle is under
    /// the press.
    #[test]
    fn the_column_s_bar_is_worked_as_the_game_s() {
        // A sliding area 10 down and 100 high, the handle half of it.
        let sliding = [10.0, 100.0];
        // Taken by its middle at the top (value 1: its middle 35 down).
        assert_eq!(dragged_to(sliding, 0.5, 0.0, 35.0, true), Some(1.0));
        // Moved 25 down of the 50 it can go: half way.
        assert_eq!(dragged_to(sliding, 0.5, 0.0, 60.0, true), Some(0.5));
        // Taken 5 above its middle, the same place of the mouse is 5 less.
        assert_eq!(dragged_to(sliding, 0.5, 5.0, 60.0, true), Some(0.4));
        // Past the ends, at the ends; a handle that fills the area, none.
        assert_eq!(dragged_to(sliding, 0.5, 0.0, 500.0, true), Some(0.0));
        assert_eq!(dragged_to(sliding, 1.0, 0.0, 50.0, true), None);
        // The handle at the top (2 to 62 down) and the bar pressed at 90.
        let handle = [0.0, 2.0, 6.0, 60.0];
        assert_eq!(stepped(1.0, 0.3, handle, [3.0, 90.0], true), Some(0.7));
        assert_eq!(stepped(0.2, 0.3, handle, [3.0, 90.0], true), Some(0.0));
        assert_eq!(stepped(0.123456, 0.1, [0.0, 50.0, 6.0, 10.0], [3.0, 10.0], true), Some(0.2235));
        assert_eq!(stepped(1.0, 0.3, handle, [3.0, 30.0], true), None);
    }

    /// The sounds of moves and windows as the game's
    /// (`ItemUiContext.PlayOperationSound`, `OpenItem`, `Inspect`): a part
    /// into a weapon's slot the interface's installing by its class, a
    /// thing put on its group's `use`, one sent into a grid its `drop`; a
    /// container's window its group's opening, a thing's the inspector's.
    /// And where the game's window that shows a thing opens
    /// (`OpenWindow`): in the middle of the screen, the next 40 units right
    /// of and below the one opened last, kept on the screen.
    #[test]
    fn moves_sound_and_windows_open_as_the_game_s() {
        let (mut carried, [bag, rifle, rounds, case]) = carried();
        let into_rifle = Some((rifle, Spot::Slot(0)));
        let sound = |carried: &Carried, classes: &[&str], address| move_sound(carried, &kind("part", classes, [1, 1]), address);
        assert!(matches!(sound(&carried, &["Mod", "GearMod", "Magazine"], into_rifle), Sound::Ui(Ui::InstallMag)));
        assert!(matches!(sound(&carried, &["Mod", "FunctionalMod", "Sights"], into_rifle), Sound::Ui(Ui::InstallModFunc)));
        assert!(matches!(sound(&carried, &["Mod", "GearMod", "Stock"], into_rifle), Sound::Ui(Ui::InstallModGear)));
        assert!(matches!(sound(&carried, &["Mod", "MasterMod", "Barrel"], into_rifle), Sound::Ui(Ui::InstallModVital)));
        assert!(matches!(sound(&carried, &["Mod", "GearMod", "Magazine"], Some((case, Spot::Cell(0, Place { x: 0, y: 0, turned: false })))), Sound::Item(_, Way::Drop)));
        let alt = [false, false, true];
        assert!(matches!(click(&mut carried, bag, false, alt, Vec2::ZERO, None), Some(Sound::Item(_, Way::Use))));
        assert!(matches!(click(&mut carried, rounds, false, [true, false, false], Vec2::ZERO, None), Some(Sound::Item(_, Way::Drop))));
        assert!(matches!(click(&mut carried, case, true, [false; 3], Vec2::ZERO, None), Some(Sound::Item(_, Way::Open))));
        carried.windows.clear();
        carried.info_size = Some(Vec2::new(670.0, 500.0));
        let (screen, s) = (Vec2::new(2560.0, 1440.0), 1440.0 / 1080.0);
        assert!(matches!(click(&mut carried, rifle, true, [false; 3], Vec2::ZERO, Some((screen, s))), Some(Sound::Ui(Ui::InspectorOpen))));
        let first = carried.windows[0].at;
        assert!((first - (screen - Vec2::new(670.0, 500.0) * s) / 2.0).length() < 1e-3, "{first}");
        click(&mut carried, rounds, true, [false; 3], Vec2::ZERO, Some((screen, s)));
        assert!((carried.windows[1].at - first - Vec2::splat(40.0 * s)).length() < 1e-3);
        // Brought to the top, the first is still not the one opened last.
        carried.open_window(rifle, WinKind::Info, first);
        assert!((game_window_at(&carried, Vec2::new(670.0, 500.0), screen, s) - first - Vec2::splat(80.0 * s)).length() < 1e-3);
        // Far down the screen, kept on it.
        carried.windows[0].at = Vec2::new(2400.0, 1300.0);
        carried.windows[0].opened = u64::MAX;
        assert_eq!(game_window_at(&carried, Vec2::new(670.0, 500.0), screen, s), screen - Vec2::new(670.0, 500.0) * s);
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
            searched_ground: None,
            repairing: false,
            repairs: Vec::new(),
            repair_settings: None,
            rules: Rules { in_raid: true, backpacks_deep: 0, busy_weapon: None, chamber_closed: None, chamber_held: None },
            fixed_wear: Vec::new(),
            note: String::new(),
            panel: Default::default(),
            ground: Vec::new(),
            thrown: Vec::new(),
            item_looks: Default::default(),
            icons: HashMap::new(),
            pictures: HashMap::new(),
            pictures_on: false,
            picture_failed: Default::default(),
            spinner: Handle::default(),
            bound: Default::default(),
            top_grenade: None,
            magazine_work: None,
            frozen_keys: HashMap::new(),
            switched: Vec::new(),
            fold_asked: None,
            round_times: RoundTimes::default(),
            strength_elite: false,
            carry: 1.0,
            windows: Vec::new(),
            priority: None,
            priority_auto: true,
            asking: None,
            pack_asked: false,
            falling: Vec::new(),
            checked: Default::default(),
            mastering: 0,
            slot_keys: Vec::new(),
            texts: Default::default(),
            info_size: None,
            game_grid_window: false,
            tag_size: None,
            tag_limit: crate::health_view::TAG_LIMIT,
            tag_edits: Vec::new(),
            wishlist: Default::default(),
        };
        (carried, ids)
    }

    /// A slot is named by the first kind of weapon part its name holds, in
    /// the game's order; a name that holds none is its own.
    #[test]
    fn a_slot_is_named_as_the_game_names_it() {
        assert_eq!(slot_name("mod_mount_004"), "mod_mount");
        assert_eq!(slot_name("mod_tactical_2"), "mod_tactical");
        assert_eq!(slot_name("mod_muzzle_001"), "mod_muzzle");
        assert_eq!(slot_name("patron_in_weapon_000"), "patron_in_weapon");
        assert_eq!(slot_name("Soft_armor_front"), "Soft_armor_front");
        assert_eq!(slot_name("mod_charge"), "mod_charge");
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
        click(&mut carried, rounds, false, ctrl, Vec2::ZERO, None);
        assert_eq!(holder(&carried, rounds), Some(carried.loot.id));
        click(&mut carried, bag, false, alt, Vec2::ZERO, None);
        click(&mut carried, rifle, false, alt, Vec2::ZERO, None);
        assert_eq!((holder(&carried, bag), holder(&carried, rifle)), (Some(worn), Some(worn)));
        assert!(carried.note.ends_with("put on"), "{}", carried.note);
        // Now the rounds have somewhere to go, and from there back.
        click(&mut carried, rounds, false, ctrl, Vec2::ZERO, None);
        assert_eq!(holder(&carried, rounds), Some(bag));
        click(&mut carried, rounds, false, ctrl, Vec2::ZERO, None);
        assert_eq!(holder(&carried, rounds), Some(carried.loot.id));
        // A double click on the rifle shows what it is; on the case, what
        // is in it, and the case is then where Ctrl sends the rounds.
        click(&mut carried, rifle, true, plain, Vec2::new(100.0, 100.0), None);
        assert_eq!((carried.windows.len(), carried.windows[0].kind, carried.priority), (1, WinKind::Info, None));
        click(&mut carried, case, true, plain, Vec2::new(200.0, 100.0), None);
        assert_eq!((carried.windows.len(), carried.windows[1].kind, carried.priority), (2, WinKind::Grids, Some(case)));
        click(&mut carried, rounds, false, ctrl, Vec2::ZERO, None);
        assert_eq!(holder(&carried, rounds), Some(case));
        // And out of it again: a thing in the priority container goes by
        // the plain rule, to the character.
        click(&mut carried, rounds, false, ctrl, Vec2::ZERO, None);
        assert_eq!(holder(&carried, rounds), Some(bag));
        // A Ctrl double click is no double click: it sends, twice.
        click(&mut carried, rounds, true, ctrl, Vec2::ZERO, None);
        assert_eq!(holder(&carried, rounds), Some(case));
    }

    /// A container's window as the game's (`ItemUiContext.OpenWindow`, with
    /// `PriorityWindowMode` Auto): opened again it is brought to the top
    /// but not picked; as any window shuts, the top-most container's window
    /// left is picked; and it shuts when its container is moved, even within
    /// the inventory.
    #[test]
    fn container_windows_pick_and_shut_as_the_game() {
        let (mut carried, [bag, rifle, _, case]) = carried();
        let plain = [false; 3];
        click(&mut carried, case, true, plain, Vec2::ZERO, None);
        click(&mut carried, bag, true, plain, Vec2::ZERO, None);
        assert_eq!(carried.priority, Some(bag));
        click(&mut carried, case, true, plain, Vec2::ZERO, None);
        assert_eq!((carried.windows.last().map(|window| window.item), carried.priority), (Some(case), Some(bag)));
        // A window that shows a thing shut: the case's, on top, is picked.
        click(&mut carried, rifle, true, plain, Vec2::ZERO, None);
        carried.shut_window(carried.windows.len() - 1);
        assert_eq!(carried.priority, Some(case));
        // The case's shut: the bag's.
        let at = carried.windows.iter().position(|window| window.item == case).expect("the case's window");
        carried.shut_window(at);
        assert_eq!((carried.windows.len(), carried.priority), (1, Some(bag)));
        // The bag put on, so moved: its window shuts.
        click(&mut carried, bag, false, [false, false, true], Vec2::ZERO, None);
        assert!(carried.windows.is_empty() && carried.priority.is_none());
    }

    /// A window is kept on the screen as the game keeps it: its left and
    /// bottom edges first, at whole pixels.
    #[test]
    fn a_window_is_kept_on_the_screen() {
        let room = Vec2::new(1920.0, 1080.0);
        assert_eq!(on_screen(Vec2::new(-30.0, 100.4), Vec2::new(261.0, 347.0), room), Vec2::new(0.0, 100.0));
        assert_eq!(on_screen(Vec2::new(1800.0, 900.0), Vec2::new(261.0, 347.0), room), Vec2::new(1659.0, 733.0));
        // Higher than the screen: its bottom kept on it.
        assert_eq!(on_screen(Vec2::new(10.0, 0.0), Vec2::new(261.0, 1200.0), room), Vec2::new(10.0, -120.0));
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

    /// The window that asks how much of food or drink to use is the
    /// game's `ShowSplitMenuDialog`: a track from 1 to the whole, the knob
    /// from `(int)(whole - left + 1)` on, the figure 1 to what is left,
    /// all of it to begin with.
    #[test]
    fn the_window_for_a_share_of_food_or_drink_is_the_game_s() {
        // A bottle of 60 with 40 left.
        let bottle = Asking::of_use(7, 60.0, 40.0, Vec2::ZERO).expect("something is left");
        assert_eq!((bottle.title, bottle.most, bottle.count), ("USE", 40, 40));
        // The knob at the track's end, and a third of it gone before its
        // first place (20 of the 59 steps from 1 to 60).
        let (knob, gone) = bottle.shares();
        assert!((knob - 1.0).abs() < 1e-6 && (gone - 20.0 / 59.0).abs() < 1e-6, "{knob} {gone}");
        // Held at the track's start it goes no lower than its first place:
        // a figure of 1; at its end, all that is left.
        assert_eq!((bottle.slid_to(0.0), bottle.slid_to(1.0)), (1, 40));
        assert_eq!(bottle.slid_to(30.0 / 59.0), 11);
        // Full, nothing is gone; empty, there is no window.
        let full = Asking::of_use(7, 60.0, 60.0, Vec2::ZERO).expect("full");
        assert_eq!((full.most, full.shares().1), (60, 0.0));
        assert!(Asking::of_use(7, 60.0, 0.0, Vec2::ZERO).is_none());
    }
}

/// Where the mouse is over a box, as a share of it from its top left
/// corner (0 to 1 each way): Bevy 0.17 measures it from the box's middle
/// (-0.5 to 0.5).
pub fn share_of(at: &RelativeCursorPosition) -> Option<Vec2> {
    at.normalized.map(|middle| middle + Vec2::splat(0.5))
}
