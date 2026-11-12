//! The windows' state, and finding, opening, raising and shutting them, bodies included.

use super::*;

/// The picture's key a thing is shown by: a magazine at work's as it was
/// held (`Carried::frozen_keys`), a weapon being reloaded's as the reload
/// began (`Carried::reload_key`), else its own (`icons::picture_key`).
/// `PROVISIONAL`: a work stopped shows its own key again at once, where the
/// game's view keeps the old picture until the view is made anew.
pub(crate) fn shown_key(carried: &Carried, item: &Item) -> String {
    if let Some((_, key)) = carried.reload_key.as_ref().filter(|(weapon, _)| *weapon == item.id) {
        return key.clone();
    }
    let working = carried.magazine_work.as_ref().is_some_and(|work| work.process.magazine == item.id);
    working.then(|| carried.frozen_keys.get(&item.id).cloned()).flatten().unwrap_or_else(|| crate::screens::icons::picture_key(item))
}

/// A box's spinner, shown while its thing's picture is still to come (and
/// on where none can be made, as the game's view keeps its loader while it
/// has no picture: `ItemView.IconChangedHandler`), and what is wanted in
/// its place: the thing's picture's key (`icons::picture_key`: its
/// template, marked where folded), the room its picture fills and whether the thing lies turned; in a
/// quick-use view, the picture's own place, turned 45 degrees there
/// (`quick_picture`).
#[derive(Component)]
pub(super) struct Loading {
    pub(super) key: String,
    pub(super) room: Rect,
    pub(super) turned: bool,
    /// A quick-use view's: the picture's place and its colour (`SlotMark`).
    pub(super) quick: Option<(Rect, Color)>,
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
    /// A container's window: the size it was last placed by, in pixels
    /// (`fit_windows` keeps its middle as it changes).
    pub size: Option<Vec2>,
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

/// A tag being edited in its window (`EditTagWindow`): the thing's, its
/// field (the name typed so far, its caret and selection, whether it has
/// the keys: `text_field`), and the colour chosen.
#[derive(Clone, Debug, PartialEq)]
pub struct TagEdit {
    pub item: Id,
    pub field: crate::screens::text_field::TextField,
    pub color: u32,
}

/// Whether a tag's field takes a letter (`ValidationInputField` as the tag's
/// is set, `Everything`: letters, figures and the underscore of any script
/// (`\w`), the space, and its list of marks).
pub(super) fn tag_letter(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || "., /\\()[]{}<>-!?'\"@;:+=*^%&$|#№".contains(c)
}

/// The count of windows opened (`WindowData._nextIndex`).
static WINDOWS_OPENED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Whether the windows over the screen are up: with the inventory open,
/// and over the edit build screen (a part inspected from its menu: the
/// game's windows are the `ItemUiContext`'s, over whichever screen).
pub(crate) fn windows_up(controller: &Controller) -> bool {
    controller.inventory_open || controller.edit_build
}

/// Whether the windows are drawn over the edit build screen (`rebuild`),
/// whose own layer lies over the inventory's windows' (the screen at 40).
pub(super) static WINDOWS_OVER_EDIT_BUILD: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// A window's place among the screen's layers: over the page and over the
/// game scene's panel of an examine (`using_view::USING_Z`: the game's
/// windows are the `ItemUiContext`'s, on the preloader's canvas, order
/// 100, over the game scene's, 1), the last opened on top; over the edit
/// build screen while it is open.
pub(super) fn window_z(index: usize) -> GlobalZIndex {
    let base = if WINDOWS_OVER_EDIT_BUILD.load(std::sync::atomic::Ordering::Relaxed) { 45 } else { 14 };
    GlobalZIndex(base + index as i32)
}

/// A part of the edit build screen inspected (`ModdingContextInteractions`'
/// Inspect): a copy of it kept as a thing observed, its window opened as
/// the game opens one; the sound of the inspector's opening.
pub(crate) fn inspect_observed(carried: &mut Carried, item: Item, cursor: Vec2, screen: Option<(Vec2, f32)>) -> Option<Sound> {
    let id = item.id;
    if carried.find(id).is_none() {
        carried.observed.push(item);
    }
    open_view(carried, id, false, cursor, screen)
}

impl Carried {
    /// An item by its number, on the character or in the crate (or one
    /// inspected from the edit build screen).
    pub fn find(&self, id: Id) -> Option<&Item> {
        self.equipment.item.find(id).or_else(|| self.loot.find(id)).or_else(|| self.observed.iter().find_map(|item| item.find(id)))
    }

    /// An item by its number on a dummy's body, wherever its gear is kept
    /// (`body`): a weapon hung there (`slung_view`).
    pub fn find_on_bodies(&self, id: Id) -> Option<&Item> {
        (0..self.bodies.len()).find_map(|which| self.body(which)?.find(id))
    }

    /// Whether a thing is one the game opens in a window of its own
    /// (`Item.IsContainer`: what is searched, a plain container, one that
    /// locks).
    pub(super) fn opens(item: &Item) -> bool {
        ["SearchableItem", "SimpleContainer", "LockableContainer", "StationaryContainer"].iter().any(|class| is(item, class))
    }

    /// The things Ctrl and a click looks in for a place for an item
    /// (`ItemUiContext.QuickFindAppropriatePlace`): the container whose
    /// window is the priority one, unless the item lies in it; else what
    /// is open beside the equipment, unless the item lies in that, with
    /// the equipment after it for a thing the character has; else the
    /// equipment.
    pub(super) fn other_side(&self, id: Id) -> Vec<Id> {
        trp_sim::items::inventory::other_side(&self.equipment.item, self.loot_open.then_some(&self.loot), self.priority, id)
    }

    /// After anything has moved: a key lets go of what no longer lies
    /// where it may be bound, and a window of what is no longer there, or
    /// has gone into itself, is shut.
    /// A kit let go on a thing it mends: the thing's repair window opened
    /// with that kit (`RepairItemContext`), as the game opens a window.
    /// Opens a thing's inspect window, alone in the middle (a check's:
    /// `repair_view`'s `--buffed`).
    pub(crate) fn open_info(&mut self, thing: Id) {
        self.open_window(thing, WinKind::Info, Vec2::ZERO);
    }

    pub(crate) fn open_repair(&mut self, kit: Id, thing: Id) {
        self.repairs.retain(|repair| repair.thing != thing);
        self.repairs.push(crate::screens::repair_view::Repair { thing, kit, selected: None, who: None });
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
    pub(super) fn shut_window(&mut self, index: usize) {
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
    pub(super) fn raise(&mut self, index: usize) {
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
    pub(super) fn open_window(&mut self, id: Id, kind: WinKind, at: Vec2) {
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
        self.windows.push(Win { item: id, kind, at, opened, stretched: None, fitted: false, size: None, held: self.held_by(id) });
        if kind == WinKind::Grids && self.priority_auto {
            self.priority = Some(id);
        }
    }

    /// Where an item lies: what holds it, and where in that.
    pub(super) fn lies(&self, id: Id) -> Option<(Id, Spot)> {
        self.equipment.item.holder_of(id).or_else(|| self.loot.holder_of(id))
    }

    /// Has the container kept as `which` be the one the screen shows.
    pub(super) fn bring(&mut self, which: usize) {
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
    pub(super) fn search_ground(&mut self, id: Id) -> bool {
        let Some(at) = self.ground.iter().position(|(item, _)| item.id == id) else { return false };
        self.put_back_body();
        self.put_back_ground();
        let (item, lay) = self.ground.remove(at);
        self.away.push(Some(item));
        let which = self.away.len() - 1;
        let before = self.loot_of;
        self.bring(which);
        self.searched_ground = Some((which, before, lay));
        true
    }

    /// A dead dummy's body searched (`SEARCH` on a body;
    /// `ShowInventoryScreenLoot` with its equipment, which the loot panel
    /// shows as the game's `ComplexStashPanel`): its gear brought into the
    /// view beside the equipment. Whether it had any.
    pub(crate) fn search_body(&mut self, which: usize) -> bool {
        self.put_back_body();
        self.put_back_ground();
        let Some(gear) = self.bodies.get_mut(which).and_then(Option::take) else { return false };
        self.away.push(Some(gear));
        let at = self.away.len() - 1;
        let before = self.loot_of;
        self.bring(at);
        self.searched_body = Some((which, at, before));
        true
    }

    /// The screen shut: a body searched keeps its gear again.
    pub(crate) fn put_back_body(&mut self) {
        let Some((which, at, before)) = self.searched_body.take() else { return };
        if self.loot_of == at {
            self.bring(before);
        }
        if let Some(gear) = self.away.get_mut(at).and_then(Option::take) {
            if let Some(kept) = self.bodies.get_mut(which) {
                *kept = Some(gear);
            }
        }
        if at + 1 == self.away.len() {
            self.away.pop();
        }
    }

    /// Whether the loot shown is a body's gear (the game's
    /// `InventoryEquipment` as the loot item).
    pub(crate) fn shows_body(&self) -> bool {
        self.searched_body.is_some_and(|(_, at, _)| at == self.loot_of)
    }

    /// A dummy's gear, wherever it is kept now.
    pub(crate) fn body(&self, which: usize) -> Option<&Item> {
        match self.searched_body {
            Some((shown, at, _)) if shown == which && at == self.loot_of => Some(&self.loot),
            _ => self.bodies.get(which).and_then(Option::as_ref),
        }
    }

    /// The same, to change (a face shield on it marked: `shield_marks`).
    pub(crate) fn body_mut(&mut self, which: usize) -> Option<&mut Item> {
        match self.searched_body {
            Some((shown, at, _)) if shown == which && at == self.loot_of => Some(&mut self.loot),
            _ => self.bodies.get_mut(which).and_then(Option::as_mut),
        }
    }

    /// The screen shut: a loose container searched goes back where it lay.
    pub(super) fn put_back_ground(&mut self) {
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
