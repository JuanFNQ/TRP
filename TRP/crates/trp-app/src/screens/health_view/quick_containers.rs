//! The quick bar's views, the tabs' places, the containers and special slots.

use super::*;

/// Draws the quick-use panel (`TARKOV UI`) as the game's into its place of
/// that size: its header (its words the install's), and each view with
/// its key's words (`HotKey`, the binding's keys) and the name of what is
/// bound to it (`Caption`, its short name; none for nothing), by the
/// binding's name in the game's `EBoundItem` (`words`). Under what is put
/// in the views (`layer` 0) the panel but each view's caption, key label
/// and border, which lie over it (1).
#[allow(clippy::too_many_arguments)]
pub fn draw_quick(column: &mut ChildSpawnerCommands, look: &HealthLook, said: &Words, size: [f32; 2], words: &HashMap<String, (String, String)>, marks: &HashMap<String, SlotMark>, layer: u8, s: f32) {
    let Some(quick) = &look.quick else { return };
    draw_quick_views(column, look, quick, &look.quick_drawn, QUICK, said, size, words, None, marks, layer, s);
}

/// What a quick view's weapon makes of the view (`QuickSlotView.method_0`
/// and `method_1`, run as a weapon is set in it and as its malfunction's
/// state changes; the inventory's bar and the battle screen's alike): one
/// with a malfunction the shooter knows of draws the view's border red and
/// its `InstallPlace` (181, 0, 0, 92), switched on, and its picture red
/// (`QuickSlotItemView.UpdateInfo`); any other weapon its `InstallPlace`
/// (0, 0, 0, 52), switched on or off as the scene has it (the inventory's
/// off, so unseen; the battle bar's on, 2 of 255 darker than the scene's
/// 50). Nothing else changes a view's look so.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum SlotMark {
    Weapon,
    Malfunction,
}

impl SlotMark {
    /// A thing's mark, none but for a weapon (`Item is Weapon`;
    /// `IMalfunctionController.HasKnownMalfunction`).
    pub(crate) fn of(item: &trp_sim::items::inventory::Item) -> Option<Self> {
        if !trp_sim::items::inventory::is(item, "Weapon") {
            return None;
        }
        let known = item.malfunction.known && item.malfunction.malf != trp_sim::weapons::malfunction::Malf::None;
        Some(if known { Self::Malfunction } else { Self::Weapon })
    }

    /// Its view's `InstallPlace`.
    pub(crate) fn install(self) -> Color {
        match self {
            Self::Weapon => Color::srgba_u8(0, 0, 0, 52),
            Self::Malfunction => Color::srgba_u8(181, 0, 0, 92),
        }
    }

    /// Its view's picture (`MainImage.color`: red, else white).
    pub(crate) fn picture(mark: Option<Self>) -> Color {
        if mark == Some(Self::Malfunction) {
            Color::srgb(1.0, 0.0, 0.0)
        } else {
            Color::WHITE
        }
    }
}

/// Draws a quick-use panel's views (the inventory's, or the battle
/// screen's bar), as `draw_quick` draws the inventory's; `selected`, the
/// keys of the views whose thing is in the hands, which a view lights
/// where it has a ground for it (`QuickSlotView.SwitchVisualSelection`:
/// the ground white and the caption black where selected, else the ground
/// black at 0.6 and the caption white; the caption in the normal style).
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_quick_views(
    column: &mut ChildSpawnerCommands,
    look: &HealthLook,
    quick: &trp_data::interface::screen::QuickViews,
    drawn: &Drawn,
    base: usize,
    said: &Words,
    size: [f32; 2],
    words: &HashMap<String, (String, String)>,
    selected: Option<&[String]>,
    marks: &HashMap<String, SlotMark>,
    layer: u8,
    s: f32,
) {
    let mut colors = HashMap::new();
    let mut unbold = Vec::new();
    // A weapon's view (`SlotMark`): its border red (`Border.color`, over
    // its thing) and its `InstallPlace` (under it) as the mark has them.
    for slot in &quick.slots {
        let Some(&mark) = marks.get(&slot.key) else { continue };
        if let Some(install) = slot.install {
            colors.insert(install, mark.install());
        }
        if let (SlotMark::Malfunction, Some(border)) = (mark, slot.border) {
            colors.insert(border, Color::srgb(1.0, 0.0, 0.0));
        }
    }
    if let Some(selected) = selected {
        for slot in quick.slots.iter().filter(|slot| slot.background.is_some()) {
            let lit = selected.contains(&slot.key);
            if let Some(ground) = slot.background {
                colors.insert(ground, if lit { Color::WHITE } else { Color::srgba(0.0, 0.0, 0.0, 0.6) });
            }
            if let Some(caption) = slot.caption {
                colors.insert(caption, if lit { Color::BLACK } else { Color::WHITE });
                unbold.push(caption);
            }
        }
    }
    let mut texts = HashMap::new();
    for slot in &quick.slots {
        let Some((key, caption)) = words.get(&slot.key) else { continue };
        if let Some(hot_key) = slot.hot_key {
            texts.insert(hot_key, key.clone());
        }
        // The caption on one line, cut at its view's width as its text cuts
        // it (`m_overflowMode`: the battle bar's with an ellipsis, the
        // inventory's bare), so "Velociraptor .300 BLK" shows
        // "Velociraptor…" on the battle bar.
        if let Some(text) = slot.caption {
            let look_of = quick.panel.nodes[text].text.as_ref();
            let face = look_of.and_then(|look_of| look.tmp_faces.get(&(quick.panel.file_of(look_of.font.0)?, look_of.font.1)));
            let fitted = match (look_of, face) {
                (Some(look_of), Some(face)) => fitted_words(face, look_of, caption, quick.panel.rect(text, size)[2]),
                _ => caption.clone(),
            };
            texts.insert(text, fitted);
        }
    }
    let headers: Vec<usize> = quick.panel.find("Header/Text").into_iter().collect();
    let over: Vec<usize> = quick.slots.iter().flat_map(|slot| [slot.caption, slot.hot_key.and_then(|key| quick.panel.nodes[key].parent), slot.border]).flatten().collect();
    let drawn_marks = match layer {
        0 => Marks { words: Some(&texts), headers: &headers, skip: &over, colors: Some(&colors), unbold: &unbold, ..Marks::default() },
        _ => Marks { words: Some(&texts), headers: &headers, within: &over, colors: Some(&colors), unbold: &unbold, ..Marks::default() },
    };
    draw_nodes(column, look, &quick.panel, drawn, base + ((layer as usize) << 16), drawn_marks, said, Frame::Is(size), [0.0, 0.0], s);
    // A malfunction's `InstallPlace` switched on where the scene has its
    // picture off (the inventory's bar), under the thing: its rectangle in
    // the mark's colour (no sprite: the engine's plain white, coloured).
    if layer == 0 {
        for slot in quick.slots.iter().filter(|slot| marks.get(&slot.key) == Some(&SlotMark::Malfunction)) {
            let Some(install) = slot.install.filter(|&install| quick.panel.nodes[install].image.is_none()) else { continue };
            let [x, y, w, h] = quick.panel.rect(install, size);
            let node = Node { position_type: PositionType::Absolute, left: Val::Px(x * s), top: Val::Px(y * s), width: Val::Px(w * s), height: Val::Px(h * s), ..default() };
            column.spawn((node, EXACT, BackgroundColor(SlotMark::Malfunction.install())));
        }
    }
}

/// Where each quick-use view puts its thing (`InstallPlace`), by its
/// binding's name, in the panel's place of that size (units).
pub fn quick_places(look: &HealthLook, size: [f32; 2]) -> Vec<(String, [f32; 4])> {
    let Some(quick) = &look.quick else { return Vec::new() };
    quick.slots.iter().filter_map(|slot| Some((slot.key.clone(), quick.panel.rect(slot.install?, size)))).collect()
}

/// Where the bar's tabs lie (`TARKOV UI`) in a window so large (units):
/// each tab's rectangle (left, top, width, height) in the row's order, the
/// app's own last, as a ninth, after the game's first `shown`
/// (`draw_tab_bar`; those after them the caller leaves out).
pub fn tab_places(look: &HealthLook, screen: [f32; 2], shown: usize) -> Vec<[f32; 4]> {
    let Some(bar) = &look.tab_bar else { return Vec::new() };
    let mut places: Vec<[f32; 4]> = bar.tabs.iter().map(|tab| bar.panel.placed_in(tab.object, screen).rect).collect();
    if let Some(&[_, top, width, height]) = places.first() {
        places.push([options_tab_left(look, screen, shown), top, width, height]);
    }
    places
}

/// Each tab's fill cut to its mask (`TARKOV UI`), as an image so many
/// pixels to a unit, by the mask's object.
pub fn tab_fill_images(look: &HealthLook, screen: [f32; 2], s: f32, images: &mut Assets<Image>) -> HashMap<usize, Handle<Image>> {
    let Some(bar) = &look.tab_bar else { return HashMap::new() };
    fill_images(&look.tab_fills, |mask| bar.panel.placed_in(mask, screen).rect, s, images)
}

/// The containers' column's ground: the colour of its scroll view's
/// parent (`Scrollview Parent`, a plain image), which the page's column
/// takes as its own.
pub fn containers_ground(look: &HealthLook) -> Option<Color> {
    let views = look.containers.as_ref()?;
    let ground = views.panel.find("Scrollview Parent")?;
    views.panel.nodes[ground].image.map(|image| rgba(image.color))
}

/// One container slot's row as the game's row template draws it: its
/// header (the slot's name in the install's words, in capitals) and its
/// slot panel (its empty picture, `Slots/Equipment/<slot>`, its fill and
/// its borders, full or empty), with the slot panel's top left at `at`
/// (units, in what it is drawn into). The grids are drawn apart
/// (`draw_grid_view`), where `container_grids` puts them.
#[allow(clippy::too_many_arguments)]
pub fn draw_container_row(column: &mut ChildSpawnerCommands, look: &HealthLook, said: &Words, left: [f32; 2], slot: &str, full: bool, panel_shown: bool, at: [f32; 2], s: f32) {
    let Some(views) = &look.containers else { return };
    let (Some(header), Some(slot_panel)) = (views.header, views.slot_panel) else { return };
    let panel = &views.panel;
    let [x, y, ..] = panel.rect(slot_panel, left);
    let origin = [at[0] - x, at[1] - y];
    let path = &panel.nodes[slot_panel].path;
    let mut set = HashMap::new();
    let mut pictures = HashMap::new();
    for (name, on) in [("Full Border", full), ("Empty Border", !full), ("Background", !full)] {
        if let Some(part) = panel.find(&format!("{path}/{name}")) {
            set.insert(part, on);
        }
    }
    if let Some(back) = panel.find(&format!("{path}/BackImage")) {
        match look.container_backs.get(slot) {
            Some(picture) => {
                pictures.insert(back, *picture);
                set.insert(back, !full);
            }
            None => {
                set.insert(back, false);
            }
        }
    }
    let header_path = &panel.nodes[header].path;
    let arrow: Vec<usize> = panel.find(&format!("{header_path}/ArrowHolder")).into_iter().collect();
    let mut words = HashMap::new();
    if let Some(text) = (header..panel.nodes.len()).find(|&below| panel.nodes[below].text.is_some() && panel.nodes[below].path.starts_with(header_path.as_str())) {
        words.insert(text, said.say(slot, slot).trim().to_uppercase());
    }
    // A thing not shown in its slot (`Item.NotShownInSlot`: the pockets)
    // has the slot's panel off (`SlotView.Show`, its `_slotPlace`).
    let within = if panel_shown { vec![header, slot_panel] } else { vec![header] };
    let marks = Marks { set: Some(&set), words: Some(&words), within: &within, pictures: Some(&pictures), skip: &arrow, ..Marks::default() };
    draw_nodes(column, look, panel, &look.containers_drawn, CONTAINERS, marks, said, Frame::Is(left), origin, s);
}

/// The key of the special slots' header's words
/// (`SearchableSlotView.CreateSlots`).
pub(super) const SPECIAL_SLOTS_HEADER: &str = "InventoryScreen/SpecialSlotsHeader";

/// A container's special slots as the game's row shows them
/// (`SearchableSlotView.CreateSlots`, its `_specSlotsPanel` turned on and
/// laid last in the row): the panel's header over its left (the words
/// `InventoryScreen/SpecialSlotsHeader` in capitals), and a view for each
/// slot (`_specSlotTemplate`, 64 units square, 10 apart in the panel's row):
/// its striped fill, its empty border a unit outside and its picture
/// (`SpecialSlot`) at its own size in its middle. `at` is the panel's top
/// left corner in units; each view's place is given back.
/// A slot that holds a thing (`full`, one a slot) has its fill, its empty
/// border and its picture off and its full border on, six units outside
/// (`SlotView`'s `_slotBackground`, `_emptyBorder`, `SlotBackImage`,
/// `FullBorder`, switched by whether it holds a thing; the template's own
/// boxes, `probe containers`), so the empty slot's "SPEC" is not drawn
/// behind a thing.
pub fn draw_special_slots(column: &mut ChildSpawnerCommands, look: &HealthLook, said: &Words, left: [f32; 2], at: [f32; 2], full: &[bool], s: f32) -> Vec<[f32; 4]> {
    let places: Vec<[f32; 4]> = (0..full.len()).map(|n| [at[0] + n as f32 * (SPECIAL_SIDE + SPECIAL_GAP), at[1], SPECIAL_SIDE, SPECIAL_SIDE]).collect();
    let Some(views) = &look.containers else { return places };
    let panel = &views.panel;
    let ends = |end: &str| (0..panel.nodes.len()).find(|&index| panel.nodes[index].path.ends_with(end));
    if let (Some(template), Some(header)) = (ends("Slot Template/Special Slots Panel Template"), ends("Special Slots Panel Template/Header Panel/Header")) {
        let [x, y, ..] = panel.rect(template, left);
        let mut words = HashMap::new();
        if let Some(text) = (header..panel.nodes.len()).find(|&below| panel.nodes[below].text.is_some() && panel.nodes[below].path.starts_with(panel.nodes[header].path.as_str())) {
            words.insert(text, said.say(SPECIAL_SLOTS_HEADER, "Special slots").trim().to_uppercase());
        }
        let marks = Marks { words: Some(&words), within: &[header], ..Marks::default() };
        draw_nodes(column, look, panel, &look.containers_drawn, CONTAINERS + (1 << 16), marks, said, Frame::Is(left), [at[0] - x, at[1] - y], s);
    }
    let sprite = |end: &str| -> Option<Sprite> {
        let index = ends(&format!("Special Slot Template/{end}"))?;
        let picture = look.containers_drawn.pictures.get(index).copied().flatten()?;
        let image = look.images.get(picture.texture)?.clone()?;
        Some(Sprite { image, cut: picture.cut, size: [panel.nodes[index].layout.size_delta[0].max(picture.size[0]), panel.nodes[index].layout.size_delta[1].max(picture.size[1])], border: picture.border, color: panel.nodes[index].image.map_or(Color::WHITE, |image| rgba(image.color)) })
    };
    let (fill, border, back, full_border) = (sprite("Background"), sprite("Empty Border"), sprite("BackImage"), sprite("Full Border"));
    for (&[x, y, w, h], &holds) in places.iter().zip(full) {
        if holds {
            if let Some(full_border) = &full_border {
                draw_sprite(column, full_border, [(x - 6.0) * s, (y - 6.0) * s, (w + 12.0) * s, (h + 12.0) * s], 1.0);
            }
            continue;
        }
        if let Some(fill) = &fill {
            let node = Node { position_type: PositionType::Absolute, left: Val::Px(x * s), top: Val::Px(y * s), width: Val::Px(w * s), height: Val::Px(h * s), ..default() };
            let image = ImageNode { image: fill.image.clone(), color: fill.color, rect: Some(fill.cut), flip_y: true, image_mode: NodeImageMode::Tiled { tile_x: true, tile_y: true, stretch_value: s }, ..default() };
            column.spawn((node, image));
        }
        if let Some(border) = &border {
            draw_sprite(column, border, [(x - 1.0) * s, (y - 1.0) * s, (w + 2.0) * s, (h + 2.0) * s], 1.0);
        }
        if let Some(back) = &back {
            let [bw, bh] = back.size;
            draw_sprite(column, back, [(x + (w - bw) / 2.0) * s, (y + (h - bh) / 2.0) * s, bw * s, bh * s], s);
        }
    }
    places
}

/// A special slot's view's side and the gap between two in their panel's
/// row (`Special Slot Template`'s `LayoutElement`, the panel's
/// `HorizontalLayoutGroup`), in units.
const SPECIAL_SIDE: f32 = 64.0;

const SPECIAL_GAP: f32 = 10.0;

/// The game's view of one grid (`GridView`): its tile of cells and its
/// border, as large as the grid, its top left at `at`.
pub fn draw_grid_view(column: &mut ChildSpawnerCommands, look: &HealthLook, said: &Words, at: [f32; 2], size: [f32; 2], s: f32) {
    let Some(grid) = look.containers.as_ref().and_then(|views| views.grid.as_ref()) else { return };
    draw_nodes(column, look, grid, &look.grid_drawn, GRID, Marks::default(), said, Frame::Is(size), at, s);
}

/// Where a container's grids lie to the right of its slot panel, from the
/// row's grids' corner: its own layout's places where it has one with as
/// many grids (`TemplatedGridsView`), else the default row
/// (`GeneratedGridsView`'s row: each grid as large as its cells, 63 units
/// to a cell and one more, so far apart). Each left, top, width, height.
pub fn container_grids(look: &HealthLook, layout: &str, grids: &[[u32; 2]]) -> Vec<[f32; 4]> {
    // Each sized to its own grid's cells about its pivot
    // (`GridView.OnGridResized`): the layout's own size may be larger (the
    // Rush 100's main grid, 4 units wider and 6 higher than its cells:
    // drawn so, a strip of cells showed beside and under what filled it).
    if let Some((_, views)) = look.layouts.get(layout).filter(|(_, views)| views.len() >= grids.len()) {
        return views.iter().zip(grids).map(|(view, cells)| view.sized(*cells)).collect();
    }
    let row = look.containers.as_ref().and_then(|views| views.grids_row).unwrap_or_default();
    let mut x = row.padding[0];
    grids
        .iter()
        .map(|cells| {
            let size = [cells[0] as f32 * 63.0 + 1.0, cells[1] as f32 * 63.0 + 1.0];
            let rect = [x, row.padding[2], size[0], size[1]];
            x += size[0] + row.spacing;
            rect
        })
        .collect()
}

/// How high a container's grids' view stands in its row: its own layout's
/// height where it has one (`TemplatedGridsView`), else its grids' with
/// the default row's padding under them (`GeneratedGridsView`'s
/// `HorizontalLayoutGroup`, a unit), as the loot panel's view; none for
/// no grids. A row of one-cell grids is so 65 units high, not 64: the
/// pockets' row, drawn a pixel short at 1440 lines, put every row
/// under it a pixel high of the game's (in a capture of the game, the
/// backpack's frame on 774 for the game's 775).
pub fn grids_height(look: &HealthLook, layout: &str, rects: &[[f32; 4]]) -> f32 {
    if rects.is_empty() {
        return 0.0;
    }
    if let Some((own, _)) = look.layouts.get(layout).filter(|(_, views)| views.len() >= rects.len()) {
        return own[1];
    }
    let bottom = rects.iter().fold(0.0f32, |bottom, [_, top, _, h]| bottom.max(top + h));
    bottom + look.containers.as_ref().and_then(|views| views.grids_row).map_or(0.0, |row| row.padding[3])
}

/// How far right of a container slot's panel its grids begin: the panel's
/// width and the row template's spacing (`Slot Template`'s row).
pub fn grids_after(look: &HealthLook) -> f32 {
    let Some(views) = &look.containers else { return 132.0 };
    let width = views.slot_panel.map_or(125.0, |panel| views.panel.nodes[panel].element.map_or(125.0, |e| e.min[0].max(0.0)));
    width + views.panel.nodes[views.template].group.map_or(7.0, |row| row.spacing)
}
