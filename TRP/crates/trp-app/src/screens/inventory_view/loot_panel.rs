//! Pictures, the reach, bodies' panels and lists, and the loot's scrolling.

use super::*;

/// A picture of a sort of thing: as it lies and turned a quarter, and how
/// wide it is for its height. It is drawn whole in an item's box at the
/// largest size that fits, so it grows and shrinks with the box and is
/// never pulled out of shape.
pub struct Picture {
    pub(super) upright: Handle<Image>,
    pub(super) turned: Handle<Image>,
    /// Width over height, as it lies.
    pub(super) shape: f32,
    /// It is the thing's own picture, made for its box: it fills it, as
    /// the game's does.
    pub(super) fills: bool,
}

impl Picture {
    /// A thing's own picture (`icons`), as it lies and turned a quarter.
    pub fn rendered(upright: Handle<Image>, turned: Handle<Image>, shape: f32) -> Self {
        Self { upright, turned, shape, fills: true }
    }

    /// Its image as it lies.
    pub(crate) fn upright(&self) -> &Handle<Image> {
        &self.upright
    }
}

/// What is looked at from within reach: a thing of the range the hands
/// work (`range::Range::things`), or a thing lying on the ground (which
/// comes first).
#[derive(Resource, Default, Clone, PartialEq)]
pub struct Reach {
    pub thing: Option<usize>,
    pub loose: Option<Id>,
    /// The bench in sight (`range::Workbench::in_sight`), with nothing else
    /// looked at.
    pub bench: Option<crate::world::range::BenchKind>,
    /// The game's menu for it (`AvailableInteractionState`): every line
    /// it offers, and the one chosen, which the wheel moves and the
    /// interact key does.
    pub menu: Option<Menu>,
    /// The name the game's panel shows under the lines: a loose thing's
    /// alone (`TargetName`: its short name, a stack's count after it where
    /// it can be taken; "Unknown item" for a kind not known), the
    /// app's own look a thing's too; and the panel's error in place of the
    /// lines where a loose thing offers none ("No Space ({0})").
    pub name: Option<String>,
    pub error: Option<String>,
    /// A loose thing is near the look though none is looked at (the game's
    /// sense, `Player.Sense`): the panel's dot shows.
    pub sense: bool,
    /// A dead dummy's body looked at (`Corpse`), by the dummy's number.
    pub body: Option<usize>,
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
pub struct LootScroll(pub(super) f32);

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
pub(super) struct LootArea {
    pub(super) shown: f32,
    pub(super) whole: f32,
}

#[derive(Component)]
pub(super) struct LootThumb {
    pub(super) shown: f32,
    pub(super) whole: f32,
}

/// Whether a thing, or anything in it, is of an item template.
pub(super) fn carries(item: &Item, template: &str) -> bool {
    item.kind.template == template || item.contents().any(|inside| carries(inside, template))
}

/// The class of the game's repair kits (`RepairKits`), by its name.
pub(super) const REPAIR_KITS_CLASS: &str = "RepairKits";

/// Whether anything of a class is carried in or on a thing.
pub(super) fn carries_class(item: &Item, class: &str) -> bool {
    is(item, class) || item.contents().any(|inside| carries_class(inside, class))
}

/// Whether a repair kit mends a thing: the thing wears and is of a class
/// its kit names (`TargetItemFilter`).
pub fn kit_mends(kit: &Item, thing: &Item) -> bool {
    is(kit, REPAIR_KITS_CLASS) && thing.repairable.is_some() && thing.kind.classes.iter().any(|class| kit.kind.repair_targets.contains(class))
}

/// A thing lying on the ground, as drawn.
#[derive(Component)]
pub(super) struct LooseItem(pub(super) Id);

/// `--search-body` (a testing aid): with the screen open, the first
/// dummy's body is searched as soon as it has gear, dead or not, as
/// `SEARCH` on it would.
pub(super) fn search_body_at_start(options: Res<crate::Options>, controller: Res<Controller>, mut carried: ResMut<Carried>, mut done: Local<bool>) {
    if *done || !options.search_body || !controller.inventory_open || !carried.bodies.first().is_some_and(Option::is_some) {
        return;
    }
    *done = carried.search_body(0);
    carried.loot_open = *done;
}

/// The game's word for a body (`CorpseItemController`'s `ContainerName`,
/// "Corpse" in the install's words), in capitals as the panel shows it.
fn body_name(carried: &Carried) -> String {
    carried.texts.say("Corpse").unwrap_or_else(|| "Corpse".to_owned()).to_uppercase()
}

/// Whether a slot of the body shown is drawn: one of the gear's that the
/// looter sees (`seen_on_body`: a Scav's armband slot is deleted, the
/// secured container hidden).
fn body_slot_seen(body: &Item, slot: &str) -> bool {
    body.slot_index(slot).is_some() && trp_sim::items::search::seen_on_body(slot, true)
}

/// A searchable thing's search drawn over its containers' row (`TARKOV UI`:
/// `draw_row_search`), its clicks and its loaders: in a column so large,
/// the row's slot panel at `at`. Whether its grids are still hidden.
#[allow(clippy::too_many_arguments)]
pub(super) fn row_search(column: &mut ChildSpawnerCommands, carried: &Carried, shows: &LeftPanel, left: [f32; 2], item: &Item, at: [f32; 2], panel_shown: bool, s: f32) -> bool {
    if !trp_sim::items::search::searchable(item) {
        return false;
    }
    let search = search_shown(carried, item);
    if !(search.unsearched || search.button) {
        return false;
    }
    let (hits, spinners) = crate::screens::health_view::draw_row_search(column, shows.look, shows.said, left, at, panel_shown, &search, 0, s);
    for (hit, [hx, hy, hw, hh]) in hits {
        column.spawn((SearchClick(item.id, hit), placed(hx * s, hy * s, hw * s, hh * s), RelativeCursorPosition::default()));
    }
    for (part, [lx, ly, lw, lh]) in spinners {
        if let Some((sprite, covers)) = crate::screens::health_view::row_loader(shows.look, part) {
            column.spawn((SpinnerPicture(sprite.image.clone(), sprite.cut, sprite.color, covers), placed(lx * s, ly * s, lw * s, lh * s), SearchSpinner(item.id)));
        }
    }
    search.unsearched
}

/// The unsearched panel's size in a containers' row (its `LayoutElement`
/// on the row's template), where the container's grids would be.
pub(super) fn row_unsearched_size(look: &crate::screens::health_view::HealthLook) -> [f32; 2] {
    look.containers
        .as_ref()
        .and_then(|views| views.panel.find(&format!("{}/{}", views.panel.nodes[views.template].path, crate::screens::health_view::ROW_UNSEARCHED)).and_then(|index| views.panel.nodes[index].element))
        .map_or([0.0, 0.0], |element| element.min)
}

/// A body's gear as the game shows a corpse's (`TARKOV UI`;
/// `ComplexStashPanel.Show`), in the stash panel's place: its header
/// (`LOOT`) and the body's name, and in its scroll view a column laid as
/// its `VerticalLayoutGroup` lays it, the gear's panel first (a copy of the
/// player's, `_equipmentPanelSource`, its slots as the looter sees them)
/// then a row for each container the looter sees (`ContainersPanel.Show`:
/// the vest, the pockets with the dog tag's view last in their row, the
/// backpack), each with its search (`SearchableSlotView`: none begun by
/// itself) and its grids once opened; no special slots (`CreateSlots`:
/// only one's own).
#[allow(clippy::too_many_arguments)]
pub(super) fn body_panel(gear: &mut ChildSpawnerCommands, carried: &Carried, units: Units, game_scrolls: [f32; 2], shows: &LeftPanel, [x, y, width, height]: [f32; 4], left_size: [f32; 2]) {
    let (s, cell) = (units.0, units.cell());
    let look = shows.look;
    let size = [width, height];
    let Some(places) = crate::screens::health_view::complex_places(look, size) else { return };
    let body = &carried.loot;
    let name = body_name(carried);
    let gear_size = gear_panel(carried);
    let after = crate::screens::health_view::grids_after(look);
    let unsearched_size = row_unsearched_size(look);
    // The slot panel's place in its row.
    let slot_in_row = look.containers.as_ref().and_then(|views| Some(views.panel.rect(views.slot_panel?, left_size)[0] - views.panel.rect(views.template, left_size)[0])).unwrap_or(0.0);
    let [left_pad, _, top_pad, bottom_pad] = places.padding;
    // The rows laid out first, under the gear's panel.
    struct Row<'a> {
        slot: &'static str,
        index: usize,
        item: Option<&'a Item>,
        at: [f32; 2],
        grids: Vec<[f32; 4]>,
        hidden: bool,
    }
    let mut rows: Vec<Row> = Vec::new();
    // `PROVISIONAL`: the gear's panel as high as the player's template's
    // size, where the game's column gives it its `LayoutElement`'s least
    // (780).
    let mut at_y = top_pad + gear_size[1] + places.spacing;
    for slot in trp_sim::items::search::BODY_CONTAINERS.into_iter().filter(|slot| *slot != "Dogtag") {
        let Some(index) = body.slot_index(slot).filter(|_| body_slot_seen(body, slot)) else { continue };
        let item = body.slots[index].as_ref();
        let hidden = item.is_some_and(|item| trp_sim::items::search::searchable(item) && !carried.searcher.searched(item.id));
        let grids = match item {
            Some(item) if !hidden => {
                let cells: Vec<[u32; 2]> = item.kind.grids.iter().map(|grid| grid.cells).collect();
                crate::screens::health_view::container_grids(look, &item.kind.layout, &cells)
            }
            _ => Vec::new(),
        };
        let panel_shown = item.is_none_or(|item| !item.kind.not_shown_in_slot);
        let start = if panel_shown { SLOT_SIDE } else { 0.0 };
        let layout = item.map_or("", |item| item.kind.layout.as_str());
        let mut height = start.max(crate::screens::health_view::grids_height(look, layout, &grids));
        if hidden {
            height = height.max(unsearched_size[1]);
        }
        if slot == "Pockets" && body_slot_seen(body, "Dogtag") {
            height = height.max(crate::screens::health_view::DOGTAG_SIDE);
        }
        rows.push(Row { slot, index, item, at: [left_pad + slot_in_row, at_y], grids, hidden });
        at_y += height + places.spacing;
    }
    let whole = at_y - places.spacing + bottom_pad;
    let [vx, vy, vw, vh] = places.view;
    let most = (whole - vh).max(0.0);
    let scrolled = game_scrolls[CRATE].clamp(0.0, most);
    gear.spawn(placed(x * s, y * s, width * s, height * s)).with_children(|view| {
        crate::screens::health_view::draw_complex(view, look, shows.said, size, &name, 0, s);
        view.spawn(Node { overflow: Overflow::clip(), ..placed(vx * s, vy * s, vw * s, vh * s) }).with_children(|cut| {
            let wheel = ScrollWheel { kind: CRATE, whole, shown: vh, frame: [x, y], size, content_top: places.content[1] - vy, grids: Vec::new() };
            cut.spawn((wheel, placed(0.0, 0.0, vw * s, vh * s), RelativeCursorPosition::default()));
            let content = placed((places.content[0] - vx) * s, (places.content[1] - vy - scrolled) * s, vw * s, whole * s);
            cut.spawn((ScrollContent(CRATE), content)).with_children(|column| {
                // The gear's panel, as the player's own is drawn, its
                // boxes over the slots' views.
                let present = |slot: &str| EQUIPMENT.iter().any(|(name, ..)| *name == slot) && body_slot_seen(body, slot);
                let full = |slot: &str| body.slot_index(slot).and_then(|index| body.slots[index].as_ref()).is_some();
                let doll = [left_pad, top_pad];
                crate::screens::health_view::draw_gear(column, look, shows.said, gear_size, doll, s, &present, &full);
                for (slot, _, own) in EQUIPMENT {
                    let Some(index) = body.slot_index(slot).filter(|_| present(slot)) else { continue };
                    let [left, top, w, h] = carried.panel.slot(slot).unwrap_or(own);
                    slot_box_drawn(column, carried, body, index, placed((doll[0] + left) * s, (doll[1] + top) * s, w * s, h * s), Vec2::new(w * s, h * s), 0, empty_line(body, index), true, s);
                }
                for row in &rows {
                    let panel_shown = row.item.is_none_or(|item| !item.kind.not_shown_in_slot);
                    crate::screens::health_view::draw_container_row(column, look, shows.said, left_size, row.slot, row.item.is_some(), panel_shown, row.at, s);
                    let side = SLOT_SIDE * s;
                    if panel_shown {
                        slot_box_drawn(column, carried, body, row.index, placed(row.at[0] * s, row.at[1] * s, side, side), Vec2::splat(side), 0, empty_line(body, row.index), true, s);
                    }
                    let left_of_grids = if panel_shown { row.at[0] + after } else { row.at[0] };
                    let mut right = 0.0f32;
                    if let Some(item) = row.item {
                        row_search(column, carried, shows, left_size, item, row.at, panel_shown, s);
                        if row.hidden {
                            right = unsearched_size[0];
                        }
                        for (grid, [gx, top, w, h]) in row.grids.iter().copied().enumerate() {
                            crate::screens::health_view::draw_grid_view(column, look, shows.said, [left_of_grids + gx, row.at[1] + top], [w, h], s);
                            column.spawn(placed((left_of_grids + gx) * s, (row.at[1] + top) * s, w * s, h * s)).with_children(|holder| grid_box_drawn(holder, carried, item, grid, cell, 0, false));
                            right = right.max(gx + w);
                        }
                    }
                    // The dog tag's view, last in the pockets' row, as far
                    // after what is before it as the row's spacing and the
                    // pockets' 10 more (`ContainersPanel.Show`).
                    if row.slot == "Pockets" {
                        if let Some(tag) = body.slot_index("Dogtag").filter(|_| body_slot_seen(body, "Dogtag")) {
                            let side = crate::screens::health_view::DOGTAG_SIDE;
                            let tag_x = left_of_grids + right + after - SLOT_SIDE + 10.0;
                            crate::screens::health_view::draw_dogtag_slot(column, look, shows.said, left_size, [tag_x, row.at[1]], body.slots[tag].is_some(), s);
                            slot_box_drawn(column, carried, body, tag, placed(tag_x * s, row.at[1] * s, side * s, side * s), Vec2::splat(side * s), 0, empty_line(body, tag), true, s);
                        }
                    }
                }
            });
        });
        // Its bar, while the column is higher than the view.
        if let (Some(([bx, by, bw, bh], bar)), Some(stash)) = (places.bar, look.stash.as_ref()) {
            if !(places.scrolling.hides && whole <= vh + 0.01) {
                let color = stash.panel.nodes[bar.object].image.map_or(Color::BLACK, |image| crate::screens::health_view::rgba(image.color));
                view.spawn((ScrollBarNode(CRATE), placed(bx * s, by * s, bw * s, bh * s), BackgroundColor(color), RelativeCursorPosition::default()));
                let value = if most > 0.0 { 1.0 - scrolled / most } else { 1.0 };
                if let (Some([hx, hy, hw, hh]), Some(handle)) = (bar.handle_in(&stash.panel, size, value, (vh / whole).clamp(0.0, 1.0)), bar.handle) {
                    let color = stash.panel.nodes[handle].image.map_or(Color::WHITE, |image| crate::screens::health_view::rgba(image.color));
                    view.spawn((ScrollHandle(CRATE, color), placed(hx * s, hy * s, hw * s, hh * s), BackgroundColor(color)));
                }
            }
        }
        crate::screens::health_view::draw_complex(view, look, shows.said, size, &name, 1, s);
    });
}

/// A body's gear in the app's own look: its name, its gear's slots each
/// under its caption, then each container it shows with its search and,
/// once opened, its grids, and the dog tag's slot.
pub(super) fn body_list(view: &mut ChildSpawnerCommands, carried: &Carried, s: f32, cell: f32) {
    let body = &carried.loot;
    view.spawn(words(body_name(carried), 11.0, FAINT));
    view.spawn(Node { flex_wrap: FlexWrap::Wrap, column_gap: Val::Px(6.0), row_gap: Val::Px(6.0), ..default() }).with_children(|slots| {
        for (slot, caption, _) in EQUIPMENT {
            let Some(index) = body.slot_index(slot).filter(|_| body_slot_seen(body, slot)) else { continue };
            slots.spawn(Node { flex_direction: FlexDirection::Column, ..default() }).with_children(|one| {
                one.spawn(words(caption, 9.0, FAINT));
                let side = SPECIAL_SIDE * s;
                slot_box(one, carried, body, index, Node { width: Val::Px(side), height: Val::Px(side), ..default() }, Vec2::splat(side), 0, s);
            });
        }
    });
    for (slot, caption) in CONTAINERS {
        let Some(index) = body.slot_index(slot).filter(|_| body_slot_seen(body, slot)) else { continue };
        view.spawn(words(caption, 10.0, FAINT));
        let Some(item) = body.slots[index].as_ref() else { continue };
        let search = search_shown(carried, item);
        if trp_sim::items::search::searchable(item) && search.button {
            search_words(view, item.id, &search);
        }
        if search.unsearched {
            continue;
        }
        view.spawn(Node { flex_wrap: FlexWrap::Wrap, column_gap: Val::Px(5.0), row_gap: Val::Px(5.0), ..default() }).with_children(|grids| {
            for grid in 0..item.kind.grids.len() {
                grid_box(grids, carried, item, grid, cell, 0);
            }
        });
    }
}

/// A container's search in the app's own look: how it stands, and a word
/// to click to begin or stop it.
pub(super) fn search_words(view: &mut ChildSpawnerCommands, id: Id, search: &crate::screens::health_view::StashSearch) {
    view.spawn(Node { column_gap: Val::Px(10.0), ..default() }).with_children(|row| {
        let state = match (search.unsearched, search.searching) {
            (true, true) => format!("UNSEARCHED: SEARCHING... {}", search.timer),
            (true, false) => "UNSEARCHED".to_owned(),
            (false, true) => format!("SEARCHING... {}", search.timer),
            (false, false) => "NOT ALL FOUND".to_owned(),
        };
        row.spawn(words(state, 11.0, WORDS));
        let (label, hit, on) = if search.searching { ("STOP", crate::screens::health_view::SearchHit::Stop, true) } else { ("SEARCH", crate::screens::health_view::SearchHit::Begin, search.can_start) };
        if on {
            row.spawn((SearchClick(id, hit), Node { padding: UiRect::axes(Val::Px(6.0), Val::Px(1.0)), ..default() }, BackgroundColor(LINE), RelativeCursorPosition::default())).with_children(|button| {
                button.spawn(words(label, 11.0, WORDS));
            });
        }
    });
}

/// How far down the window the crate's panel begins, in pixels: the
/// screen's own margin, the row of tabs and the panel's caption.
pub(super) const LOOT_PANEL_TOP: f32 = 134.0;

/// The gap between two grids of one container, and how wide the scroll
/// bar is, in pixels. The app's.
pub(super) const GRID_GAP: f32 = 5.0;

pub(super) const SCROLL_BAR: f32 = 12.0;

/// The crate's panel is scrolled: by the wheel while the mouse is over
/// it, a cell a notch, and by its bar's thumb held with the left button.
/// The grids are the container's own (`Overflow::scroll_y`); this keeps
/// how far they are scrolled and puts the thumb where that is.
pub(super) fn scroll_loot(
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
    // Each container opens at its top: the screen opened on one, or
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
