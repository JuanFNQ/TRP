//! Windows stretched and fitted, and each window's box built.

use super::*;

/// A window that shows a thing stretched by its edges and corners
/// (`StretchWindow`, `StretchArea.OnDrag`): a press on one of its
/// stretching places, on top where the mouse is, takes it; once the mouse
/// is the drag threshold from the press (`DRAG_THRESHOLD`: `OnDrag` only
/// for a drag begun), the mouse's travel since the last place the stretch
/// took (`_lastPosition`, the press's place at first), in
/// units, along the sides it stretches (out for the right and the bottom,
/// in for the left and the top), added to its preferred size, held within
/// nought and its most (`_maxSize`); laid out again, the window as large
/// as that or as what it holds asks, whichever is more, its preferred size
/// then what it came to, and the mouse's last place moved on by as much
/// as it did; the window moved so that its other side stays
/// (`anchoredPosition` by the change at its pivot, the middle), and back
/// onto the screen (`CorrectPositionResolution`).
#[allow(clippy::too_many_arguments)]
pub(super) fn stretch_windows(
    buttons: Res<ButtonInput<MouseButton>>,
    window: Query<&Window, With<PrimaryWindow>>,
    controller: Res<Controller>,
    mut carried: ResMut<Carried>,
    mut clicks: ResMut<Clicks>,
    areas: Query<(&WinStretch, &RelativeCursorPosition)>,
    look: Res<crate::screens::health_view::HealthLook>,
    (game, held, said, units): (Res<crate::GameData>, Res<HandsHeld>, Res<crate::screens::health_view::Words>, Res<Units>),
) {
    if !windows_up(&controller) || !buttons.pressed(MouseButton::Left) {
        if clicks.stretching.is_some() {
            clicks.stretching = None;
        }
        if !windows_up(&controller) {
            return;
        }
    }
    let Ok(screen) = window.single() else { return };
    let room = Vec2::new(screen.width(), screen.height());
    let s = units.0.max(1e-3);
    let Some(cursor) = screen.cursor_position() else { return };
    if buttons.just_pressed(MouseButton::Left) && clicks.stretching.is_none() && carried.asking.is_none() {
        if let Some((index, sides)) = areas.iter().filter(|(_, at)| at.cursor_over()).map(|(area, _)| (area.0, area.1)).max_by_key(|(index, _)| *index) {
            clicks.stretching = Some((index, sides, cursor, false));
        }
    }
    stretch_by(&mut carried, &mut clicks, cursor, screen.scale_factor(), room, s, (&controller, &look, &game, &held, &said));
}

/// A window that shows a thing, or the game's window of a container's
/// grids, placed by its own size as it is first laid out (`OpenWindow`:
/// alone, in the middle of the screen; else 40 right of and below the one
/// opened last, then `CorrectPositionResolution`): it opened where one of
/// its size as made would be, and what it holds sizes it. After the clicks
/// that open one and before the page is drawn: at its place on the frame
/// it opens (`courses10`'s `weapon-inspect-menu`, frame 259, the release's;
/// placed by a window's usual size, it would be 318 pixels low on that frame).
///
/// A container's window whose size changes after (its search over, its
/// grids shown in place of the unsearched panel) keeps its middle, kept on
/// the screen with its corner at whole pixels (`GridWindow.
/// OnRectTransformDimensionsChange`, `CorrectPosition`: the window's pivot
/// is its middle; `courses10`'s `backpack-search`, frame 562: the
/// Terraframe's ground from 1007.33,616.67 545.34 by 206.66 to 1013,488
/// 534.67 by 464, its middle kept at 1280.3,720).
#[allow(clippy::too_many_arguments)]
pub(super) fn fit_windows(
    window: Query<&Window, With<PrimaryWindow>>,
    controller: Res<Controller>,
    mut carried: ResMut<Carried>,
    look: Res<crate::screens::health_view::HealthLook>,
    (game, held, said, units): (Res<crate::GameData>, Res<HandsHeld>, Res<crate::screens::health_view::Words>, Res<Units>),
) {
    let resized = carried.is_changed() && look.grid_window.is_some() && carried.windows.iter().any(|open| open.fitted && open.size.is_some());
    if !windows_up(&controller) || (carried.windows.iter().all(|open| open.fitted) && !resized) {
        return;
    }
    let Ok(screen) = window.single() else { return };
    let room = Vec2::new(screen.width(), screen.height());
    let s = units.0.max(1e-3);
    if resized {
        let placed: Vec<usize> = carried.windows.iter().enumerate().filter(|(_, open)| open.fitted && open.size.is_some()).map(|(index, _)| index).collect();
        for index in placed {
            let open = carried.windows[index];
            let Some(size) = carried.find(open.item).and_then(|item| grid_built(&look, &carried, item, false, &said)).map(|built| Vec2::from(built.size) * s) else { continue };
            let Some(was) = open.size.filter(|was| *was != size) else { continue };
            let placed = &mut carried.windows[index];
            placed.at = regrown(open.at, was, size, room);
            placed.size = Some(size);
        }
    }
    let unfitted: Vec<usize> = carried.windows.iter().enumerate().filter(|(_, open)| !open.fitted && (matches!(open.kind, WinKind::Info | WinKind::Repair) || (open.kind == WinKind::Grids && look.grid_window.is_some()))).map(|(index, _)| index).collect();
    for index in unfitted {
        let open = carried.windows[index];
        let size = carried.find(open.item).and_then(|item| match open.kind {
            WinKind::Grids => grid_built(&look, &carried, item, false, &said).map(|built| Vec2::from(built.size) * s),
            WinKind::Repair => crate::screens::health_view::repair_built(&look, &crate::screens::health_view::RepairWords::default()).map(|built| Vec2::from(built.size) * s).or(Some(Vec2::new(420.0, 260.0))),
            _ => {
                let figures = trp_sim::items::item_attributes::figures(item, &game.db, &|key| carried.texts.say(key));
                let (parts, _) = info_parts(&carried, item, &held, controller.health.as_ref(), &game, &said, open.stretched);
                crate::screens::health_view::info_built(&look, &crate::screens::health_view::InfoWords::default(), Some(&figures), &parts, false).map(|built| Vec2::from(built.size) * s)
            }
        });
        let alone = carried.windows.len() == 1;
        // (Marked only where it moves: drawn afresh for nothing, the page
        // knew the mouse a frame late, the slot under it lit two frames
        // after the window opened, the game's one: `courses10`'s
        // `weapon-inspect-menu`, frame 260.)
        let fitted = carried.bypass_change_detection().windows.get_mut(index).expect("an open window");
        let was = fitted.at;
        if let Some(size) = size {
            fitted.at = if alone { ((room - size) / 2.0).min(room - size).max(Vec2::ZERO) } else { on_screen(fitted.at, size, room) };
        }
        fitted.size = size.filter(|_| open.kind == WinKind::Grids);
        fitted.fitted = true;
        if fitted.at != was {
            carried.set_changed();
        }
    }
}

/// Where a window's top left corner goes as its size changes from `was`
/// to `size`: its middle kept, then on the screen at whole pixels
/// (`GridWindow.OnRectTransformDimensionsChange`).
pub(super) fn regrown(at: Vec2, was: Vec2, size: Vec2, room: Vec2) -> Vec2 {
    on_screen(at + (was - size) / 2.0, size, room)
}

/// A stretch taken on by the mouse at `cursor` (`stretch_windows`).
#[allow(clippy::type_complexity)]
fn stretch_by(
    carried: &mut ResMut<Carried>,
    clicks: &mut ResMut<Clicks>,
    cursor: Vec2,
    scale: f32,
    room: Vec2,
    s: f32,
    (controller, look, game, held, said): (&Controller, &crate::screens::health_view::HealthLook, &crate::GameData, &HandsHeld, &crate::screens::health_view::Words),
) {
    let Some((index, sides, last, begun)) = clicks.stretching else { return };
    // Nothing until the drag begins (`OnDrag` comes only past the event
    // system's threshold), then the whole travel since the press
    // (`_lastPosition` is the press's place: `OnPointerDown`): `courses10`'s
    // `weapon-inspect-menu`, 1 to 4 pixels out (frames 497 to 500) nothing,
    // 7 (501) the window 7 wider.
    if !begun {
        if cursor.distance(last) * scale < DRAG_THRESHOLD {
            return;
        }
        clicks.stretching = Some((index, sides, last, true));
    }
    let travel = cursor - last;
    if travel == Vec2::ZERO {
        return;
    }
    let Some(win) = carried.windows.get(index).copied() else {
        clicks.stretching = None;
        return;
    };
    let Some(item) = carried.find(win.item) else { return };
    let figures = trp_sim::items::item_attributes::figures(item, &game.db, &|key| carried.texts.say(key));
    let size_at = |stretched: Option<[f32; 2]>| {
        let (parts, _) = info_parts(&carried, item, &held, controller.health.as_ref(), &game, &said, stretched);
        crate::screens::health_view::info_built(&look, &crate::screens::health_view::InfoWords::default(), Some(&figures), &parts, false).map(|built| built.size)
    };
    let Some(before) = size_at(win.stretched) else { return };
    let preferred = win.stretched.or_else(|| crate::screens::health_view::info_preferred(&look)).unwrap_or(before);
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
    // Kept on the screen, its top left then on a whole pixel
    // (`CorrectPositionResolution`: 833.33,68.67 to 833,69 at the take's
    // first step).
    let size = Vec2::new(after[0], after[1]) * s;
    at = on_screen(at, size, room);
    clicks.stretching = Some((index, sides, last + Vec2::new(change[0] * sense[0], change[1] * sense[1]) * s, true));
    if let Some(open) = carried.windows.get_mut(index) {
        (open.stretched, open.at) = (Some(after), at);
    }
}

/// A window over the page (`GridWindow`, `InfoWindow`): a caption with
/// the thing's name, by which it is moved, a button that shuts it, and
/// under it the container's grids or what the thing is. The window whose
/// container Ctrl and a click sends things to has its caption lit.
#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_arguments)]
pub(super) fn window_box(commands: &mut Commands, carried: &Carried, window: Win, index: usize, units: Units, look: &crate::screens::health_view::HealthLook, said: &crate::screens::health_view::Words, (game, previews, held, health): (&crate::GameData, &crate::screens::info_preview::InfoPreviews, &HandsHeld, Option<&trp_sim::condition::health::Health>), scrolled: f32) {
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
        // A thing not known (`ItemSpecificationPanel`; inventory spec,
        // "Examining"): "Unknown item", its kind "?", no description, no
        // wish list's mark; its figures "?", no durability, no parts'
        // panel and no mastering; its model black.
        let unknown = !trp_sim::items::examine::known(item);
        let words = if unknown {
            crate::screens::health_view::InfoWords {
                tag: (!item.tag.name.trim().is_empty()).then(|| (item.tag.name.clone(), item.tag.color)),
                wish: None,
                caption: carried.texts.say("Unknown item").unwrap_or_else(|| "Unknown item".to_owned()),
                kind: "?".to_owned(),
                weight: format!("{:.3}{}", item.weight(), carried.texts.kg),
                description: String::new(),
            }
        } else {
            crate::screens::health_view::InfoWords {
                tag: (!item.tag.name.trim().is_empty()).then(|| (item.tag.name.clone(), item.tag.color)),
                wish: carried.wishlist.get(template).map(|group| group.name()),
                // A weapon's with its build's name (`ItemSpecificationPanel.
                // ItemName`: `BriefItemName` of its full name; `courses10`'s
                // `weapon-inspect-menu`, "... assault rifle T.O 2K24").
                caption: carried.tooltip_name(item),
                kind: carried.texts.kinds.get(template).cloned().unwrap_or_default(),
                weight: format!("{:.3}{}", item.weight(), carried.texts.kg),
                description: carried.texts.descriptions.get(template).cloned().unwrap_or_default(),
            }
        };
        // Its figures (inspect-window spec), its parts' slots, its buttons,
        // a weapon's mastering, and its model, its loader until the model
        // is in.
        let mut figures = trp_sim::items::item_attributes::figures(item, &game.db, &|key| carried.texts.say(key));
        let (mut parts, mut at) = info_parts(carried, item, held, health, game, said, window.stretched);
        if unknown {
            unknown_figures(&mut figures);
            (parts.mastering, at) = (None, Vec::new());
            parts.slots.clear();
        }
        let model = previews.picture(window.item);
        let Some(built) = crate::screens::health_view::info_built(look, &words, Some(&figures), &parts, model.is_none()) else { return };
        let Some(places) = built.places() else { return };
        let [w, h] = places.size;
        commands
            .spawn((Overlay, WinRoot(index), Cover(layer), RelativeCursorPosition::default(), placed(window.at.x, window.at.y, w * s, h * s), window_z(index)))
            .with_children(|inside| {
                crate::screens::health_view::draw_info_window(inside, look, said, &words, &built, index, s, (window.opened, scrolled));
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
                for (n, ((slot, (holder, number)), [x, y, w, h])) in parts.slots.iter().zip(&at).zip(&built.slots).enumerate() {
                    let Some(holder) = carried.find(*holder) else { continue };
                    let reach = if slot.locked { OUT_OF_REACH } else { layer };
                    // A chamber not checked shows no part (`ModSlotView.
                    // SetLocked`: its item view off).
                    if !slot.unchecked {
                        slot_box_drawn(inside, carried, holder, *number, placed(x * s, y * s, w * s, h * s), Vec2::new(w * s, h * s), reach, SlotLook::Game(Some(PART_LINE_OUT)), false, s);
                    }
                    // A locked one faded, and saying why under the mouse.
                    if slot.locked {
                        inside.spawn((placed(x * s, y * s, w * s, h * s), BackgroundColor(ground.with_alpha(0.5)), bevy::ui::FocusPolicy::Pass));
                        if !slot.tip.is_empty() {
                            inside.spawn((InfoLockTip(window.item, n, slot.tip.clone(), layer), RelativeCursorPosition::default(), placed(x * s, y * s, w * s, h * s), bevy::ui::FocusPolicy::Pass));
                        }
                    }
                }
                // Its buttons, each a line of the thing's menu.
                for ((button, [x, y, w, h]), ground) in parts.buttons.iter().zip(&built.buttons).zip(&built.grounds) {
                    if let Some(line) = trp_sim::items::item_menu::Line::ALL.into_iter().find(|line| line.name() == button.name) {
                        let piece = ground.as_ref().map(|(node, ..)| crate::screens::health_view::info_piece(index, *node));
                        let rect = [window.at.x + x * s, window.at.y + y * s, w * s, h * s];
                        let mut hit = inside.spawn((InfoButtonHit(window.item, line, layer, rect, piece), RelativeCursorPosition::default(), placed(x * s, y * s, w * s, h * s)));
                        if let (Some((_, colors, alpha)), Some(piece)) = (ground, piece) {
                            hit.insert(crate::screens::health_view::InfoButtonLight { piece, colors: *colors, alpha: *alpha, layer });
                        }
                    }
                }
                // Its stretching places, on its edges and corners, outside it.
                for (sides, [x, y, w, h]) in &built.stretch {
                    inside.spawn((WinStretch(index, *sides), crate::platform::cursor::StretchPointer(index, *sides), RelativeCursorPosition::default(), placed(x * s, y * s, w * s, h * s)));
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
                    // (Black for a thing not known: `_cameraImage` tinted.)
                    let tint = if unknown { Color::BLACK } else { Color::WHITE };
                    match model {
                        Some(image) => inside.spawn((crate::screens::info_preview::InfoPicture(window.item, layer), RelativeCursorPosition::default(), ImageNode::new(image).with_color(tint), room)),
                        None => inside.spawn((crate::screens::info_preview::InfoPicture(window.item, layer), RelativeCursorPosition::default(), room)),
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
        if let Some(built) = grid_built(look, carried, item, lit, said) {
            let s = units.0;
            let [w, h] = built.size;
            commands
                .spawn((Overlay, WinRoot(index), Cover(layer), RelativeCursorPosition::default(), placed(window.at.x, window.at.y, w * s, h * s), window_z(index)))
                .with_children(|inside| {
                    crate::screens::health_view::draw_grid_window(inside, look, said, &built, index, s);
                    for (hit, [x, y, w, h]) in built.search_hits.iter().copied() {
                        inside.spawn((SearchClick(item.id, hit), placed(x * s, y * s, w * s, h * s), RelativeCursorPosition::default()));
                    }
                    for (path, [x, y, w, h]) in built.search_spinners.iter().copied() {
                        if let Some((sprite, covers)) = crate::screens::health_view::window_loader(look, path) {
                            inside.spawn((SpinnerPicture(sprite.image.clone(), sprite.cut, sprite.color, covers), placed(x * s, y * s, w * s, h * s), SearchSpinner(item.id)));
                        }
                    }
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
        .spawn((Overlay, WinRoot(index), Cover(layer), RelativeCursorPosition::default(), root, BorderColor::all(LINE), BackgroundColor(PANEL), window_z(index)))
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
                    // A searchable thing's search, and nothing of its grids
                    // until it has been opened.
                    if trp_sim::items::search::searchable(item) {
                        let search = search_shown(carried, item);
                        if search.button {
                            search_words(body, item.id, &search);
                        }
                        if search.unsearched {
                            return;
                        }
                    }
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
                    let figures = trp_sim::items::item_attributes::figures(item, &game.db, &|key| carried.texts.say(key));
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
