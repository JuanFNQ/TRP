//! The GEAR page.

use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn gear_page(gear: &mut ChildSpawnerCommands, carried: &Carried, units: Units, scroll: f32, game_scrolls: [f32; 2], figure: Option<&Handle<Image>>, shows: &LeftPanel) {
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
    let ground = tarkov.and_then(|_| crate::screens::health_view::left_ground(shows.look)).unwrap_or(PANEL);
    let game_left = left_size.filter(|_| tarkov.is_some()).map(|left_size| (left_size, [0.0, 0.0]));
    let doll_at = tarkov.map_or([0.0, 0.0], |(_, gear_at)| [gear_at[0], gear_at[1]]);
    gear.spawn((left, BackgroundColor(ground))).with_children(|left| {
        let health_tab = shows.tab == HEALTH;
        // The left panel's foot first, under both tabs' panels.
        if let Some((left_size, origin)) = game_left {
            crate::screens::health_view::draw_foot(left, shows.look, shows.said, left_size, origin, s);
        }
        if health_tab && shows.look.tarkov_ui {
            match game_left {
                Some((left_size, origin)) => crate::screens::health_view::draw_panel(left, shows.look, shows.said, shows.health, left_size, origin, s),
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
            let full = |slot: &str| worn.slot_index(slot).and_then(|index| worn.slots[index].as_ref().or_else(|| carried.leaving_from(worn.id, index))).is_some();
            crate::screens::health_view::draw_gear(left, shows.look, shows.said, size, doll_at, s, &present, &full);
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
                    slot_box_drawn(doll, carried, worn, index, node, Vec2::new(width * s, height * s), 0, empty_line(worn, index), true, s);
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
            crate::screens::health_view::draw_own_panel(left, shows.look, shows.said, shows.health, left_size, [0.0, doll[1] - gear_top], doll, s);
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
            crate::screens::health_view::draw_strip(left, shows.look, shows.said, left_size, origin, s);
            left.spawn((Node { max_width: notes, ..default() }, words(carried.note.clone(), 12.0, FAINT)));
            return;
        }
        // The carried weight, which is what the controller goes by.
        left.spawn(words(format!("WEIGHT   {:.1} kg", carried.told_weight), 14.0, WORDS));
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
        let after = crate::screens::health_view::grids_after(shows.look);
        // The rows laid out first: the column is as high as they are with
        // its padding (`ContentSizeFitter` on its `VerticalLayoutGroup`).
        let mut laid = Vec::new();
        let mut y = rows.padding[2];
        let unsearched_size = row_unsearched_size(shows.look);
        for (slot, _) in CONTAINERS {
            let Some(index) = worn.slot_index(slot) else { continue };
            let item = worn.slots[index].as_ref();
            // A container put on before it was searched (a bag found
            // elsewhere) shows its unsearched panel in place of its grids
            // (`SearchableSlotView`, `SearchableView`).
            let hidden = item.is_some_and(|item| trp_sim::items::search::searchable(item) && !carried.searcher.searched(item.id));
            let grids = item.filter(|_| !hidden).map_or_else(Vec::new, |item| {
                let cells: Vec<[u32; 2]> = item.kind.grids.iter().map(|grid| grid.cells).collect();
                crate::screens::health_view::container_grids(shows.look, &item.kind.layout, &cells)
            });
            // A thing not shown in its slot (the pockets) leaves the row
            // its grids and its special slots (`SlotView.Show`).
            let panel_shown = item.is_none_or(|item| !item.kind.not_shown_in_slot);
            let specials = item.map_or(0, |item| special_slots(item).len());
            let start = if panel_shown { SLOT_SIDE } else if specials > 0 { SPECIAL_SIDE } else { 0.0 };
            let layout = item.map_or("", |item| item.kind.layout.as_str());
            let mut height = start.max(crate::screens::health_view::grids_height(shows.look, layout, &grids));
            if hidden {
                height = height.max(unsearched_size[1]);
            }
            laid.push((slot, index, item, [slot_x, y], grids));
            y += height + rows.spacing;
        }
        let whole = if laid.is_empty() { rows.padding[2] + rows.padding[3] } else { y - rows.spacing + rows.padding[3] };
        let scrolling = views.scrolling;
        let view = scrolling.and_then(|scrolling| scrolling.view).map_or([0.0, 0.0, left_size[0], left_size[1]], |view| views.panel.rect(view, left_size));
        let most = (whole - view[3]).max(0.0);
        let scrolled = if scrolling.is_none_or(|scrolling| scrolling.clamped) { game_scrolls[COLUMN].clamp(0.0, most) } else { game_scrolls[COLUMN] };
        let mask = views.panel.find("Scrollview Parent").map_or([0.0, 0.0, left_size[0], left_size[1]], |parent| views.panel.rect(parent, left_size));
        let ground = crate::screens::health_view::containers_ground(shows.look).unwrap_or(Color::NONE);
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
                        crate::screens::health_view::draw_container_row(column, shows.look, shows.said, left_size, slot, item.is_some(), panel_shown, at, s);
                        let side = SLOT_SIDE * s;
                        if panel_shown {
                            slot_box_drawn(column, carried, worn, index, placed(at[0] * s, at[1] * s, side, side), Vec2::splat(side), 0, empty_line(worn, index), true, s);
                        }
                        let Some(item) = item else { continue };
                        let left_of_grids = if panel_shown { at[0] + after } else { at[0] };
                        let mut right = 0.0f32;
                        if row_search(column, carried, shows, left_size, item, at, panel_shown, s) {
                            right = unsearched_size[0];
                        }
                        for (grid, [x, top, w, h]) in grids.iter().copied().enumerate() {
                            crate::screens::health_view::draw_grid_view(column, shows.look, shows.said, [left_of_grids + x, at[1] + top], [w, h], s);
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
                            let full: Vec<bool> = specials.iter().map(|&special| item.slots.get(special).is_some_and(Option::is_some) || carried.leaving_from(item.id, special).is_some()).collect();
                            let places = crate::screens::health_view::draw_special_slots(column, shows.look, shows.said, left_size, [left_of_grids + right + spacing, at[1]], &full, s);
                            for (special, [x, y, w, h]) in specials.into_iter().zip(places) {
                                slot_box_drawn(column, carried, item, special, placed(x * s, y * s, w * s, h * s), Vec2::new(w * s, h * s), 0, empty_line(item, special), true, s);
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
            let color = views.panel.nodes[bar.object].image.map_or(Color::BLACK, |image| crate::screens::health_view::rgba(image.color));
            column.spawn((ScrollBarNode(COLUMN), placed(bx * s, by * s, bw * s, bh * s), BackgroundColor(color), RelativeCursorPosition::default()));
            let value = if most > 0.0 { 1.0 - scrolled / most } else { 1.0 };
            let handle = bar.handle_in(&views.panel, left_size, value, (view[3] / whole).clamp(0.0, 1.0));
            if let (Some([hx, hy, hw, hh]), Some(handle_object)) = (handle, bar.handle) {
                let color = views.panel.nodes[handle_object].image.map_or(Color::WHITE, |image| crate::screens::health_view::rgba(image.color));
                column.spawn((ScrollHandle(COLUMN, color), placed(hx * s, hy * s, hw * s, hh * s), BackgroundColor(color)));
            }
        });
        // The left side's frame, over both its panels.
        crate::screens::health_view::draw_border(gear, shows.look, shows.said, [left_size[0] * 2.0 + gap, left_size[1]], [x, column_at[1]], s);
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
                // A container put on before it was searched: its search
                // in place of its grids.
                if trp_sim::items::search::searchable(item) {
                    let search = search_shown(carried, item);
                    if search.button {
                        row.spawn(Node { flex_direction: FlexDirection::Column, ..default() }).with_children(|column| search_words(column, item.id, &search));
                    }
                    if search.unsearched {
                        return;
                    }
                }
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
    let game_stash = game.filter(|_| tarkov.is_some()).and_then(|game| game.stash_in(shows.screen)).and_then(|rect| Some((rect, crate::screens::health_view::stash_places(shows.look, [rect[2], rect[3]])?)));
    // A body searched: the game's panel for a corpse's gear in that place
    // (`ComplexStashPanel`; inventory spec, "Searching a body").
    if let (true, Some((rect, _)), Some(left_size)) = (carried.loot_open && carried.shows_body() && shows.loot_shown, game_stash.as_ref(), left_size) {
        body_panel(gear, carried, units, game_scrolls, shows, *rect, left_size);
        return;
    }
    if let Some(([x, y, width, height], places)) = game_stash {
        let look = shows.look;
        gear.spawn(placed(x * s, y * s, width * s, height * s)).with_children(|view| {
            if !carried.loot_open || !shows.loot_shown {
                return;
            }
            // A table of weapons' gun case or magazine box by its class (the
            // user's names for them), else the container's short name, in
            // capitals (`ItemController.ContainerName`).
            let name = crate::world::range::weapon_crate_label(carried.loot_of).unwrap_or_else(|| carried.name(&carried.loot)).to_uppercase();
            let loot = &carried.loot;
            // A kind lit by a thing of it known there, not by one still
            // hidden (`GridView.PrepareItems`: only what `IsItemKnown`;
            // `ItemFoundHandler`: each thing as it is found).
            let present: Vec<bool> = crate::screens::health_view::FILTERS
                .iter()
                .enumerate()
                .map(|(place, classes)| place == 0 || loot.grids.iter().flatten().any(|(_, item)| carried.searcher.known(item.id) && classes.iter().any(|class| item.kind.class_names.iter().any(|name| name == class))))
                .collect();
            let size = [width, height];
            // Its search (inventory spec, "Searching a container"): its
            // parts as it stands, a click where they take one, and its
            // grids not shown until it has been opened (`OnShowGrids`). On
            // the frame a search begins its timer still says what it was
            // left with: the panel is shown, and its search begun, after
            // the frame's scripts have been updated, and the timer writes
            // its words as it is updated (`TimerText.Show` writes none:
            // `courses10`'s searches, "0:05", "0:03", "0:02" on the panel's
            // first frame, "0:00" on the next).
            let mut search = search_shown(carried, loot);
            if carried.searcher.searching(loot.id).is_some_and(|under_way| carried.search_clock <= under_way.began) {
                search.timer = carried.search_timer_said.clone().unwrap_or_default();
            }
            let (hits, spinners) = crate::screens::health_view::draw_stash(view, look, shows.said, size, &name, &present, shows.stash_fills, 0, s, &search);
            for (hit, [hx, hy, hw, hh]) in hits {
                view.spawn((SearchClick(loot.id, hit), placed(hx * s, hy * s, hw * s, hh * s), RelativeCursorPosition::default()));
            }
            // Its loaders turning, as the app turns a loader.
            for (path, [lx, ly, lw, lh]) in spinners {
                if let Some((sprite, covers)) = crate::screens::health_view::stash_loader(look, path) {
                    view.spawn((SpinnerPicture(sprite.image.clone(), sprite.cut, sprite.color, covers), placed(lx * s, ly * s, lw * s, lh * s), SearchSpinner(loot.id)));
                }
            }
            if search.unsearched {
                crate::screens::health_view::draw_stash(view, look, shows.said, size, &name, &present, shows.stash_fills, 1, s, &search);
                return;
            }
            let cells: Vec<[u32; 2]> = loot.kind.grids.iter().map(|grid| grid.cells).collect();
            let rects = crate::screens::health_view::container_grids(look, &loot.kind.layout, &cells);
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
                        crate::screens::health_view::draw_grid_view(grids, look, shows.said, [gx, gy], [gw, gh], s);
                        grids.spawn(placed(gx * s, gy * s, gw * s, gh * s)).with_children(|holder| grid_box_drawn(holder, carried, loot, grid, cell, 0, false));
                    }
                });
            });
            // Its bar, while the grids are higher than it shows.
            if let (Some(([bx, by, bw, bh], bar)), Some(stash)) = (places.bar, look.stash.as_ref()) {
                if !(places.scrolling.hides && whole <= shown + 0.01) {
                    let color = stash.panel.nodes[bar.object].image.map_or(Color::BLACK, |image| crate::screens::health_view::rgba(image.color));
                    view.spawn((ScrollBarNode(CRATE), placed(bx * s, by * s, bw * s, bh * s), BackgroundColor(color), RelativeCursorPosition::default()));
                    let value = if most > 0.0 { 1.0 - scrolled / most } else { 1.0 };
                    if let (Some([hx, hy, hw, hh]), Some(handle)) = (bar.handle_in(&stash.panel, size, value, (shown / whole).clamp(0.0, 1.0)), bar.handle) {
                        let color = stash.panel.nodes[handle].image.map_or(Color::WHITE, |image| crate::screens::health_view::rgba(image.color));
                        view.spawn((ScrollHandle(CRATE, color), placed(hx * s, hy * s, hw * s, hh * s), BackgroundColor(color)));
                    }
                }
            }
            crate::screens::health_view::draw_stash(view, look, shows.said, size, &name, &present, shows.stash_fills, 1, s, &search);
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
        if carried.shows_body() {
            body_list(view, carried, s, cell);
            return;
        }
        // A table of weapons' gun case or magazine box by its class: each
        // is one of the game's containers, which all its kind are named by.
        let caption = crate::world::range::weapon_crate_label(carried.loot_of).unwrap_or_else(|| carried.name(&carried.loot));
        view.spawn(words(caption.to_uppercase(), 11.0, FAINT));
        // Its search in the app's own look: how it stands, and a word to
        // click to begin or stop it; nothing of its grids until opened.
        let search = search_shown(carried, &carried.loot);
        if search.button {
            search_words(view, carried.loot.id, &search);
        }
        if search.unsearched {
            return;
        }
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
