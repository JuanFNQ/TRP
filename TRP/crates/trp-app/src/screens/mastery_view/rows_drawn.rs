//! The rows, their pictures and views, and the tooltip drawn.

use super::*;

/// What the page's own objects show as it stands, for the SKILLS page's
/// drawing of the whole page (`skills_view::draw_page`): the search
/// field's words, each dropdown's chosen line, the lists open, the views'
/// tabs, and each dropdown's box lit under the mouse.
#[allow(clippy::too_many_arguments)]
pub(crate) fn page_marks(panel: &ScenePanel, look: &MasteryLook, said: &Words, rows: &[Row], state: &MasteryState, cursor: Option<[f32; 2]>, screen: [f32; 2], set: &mut HashMap<usize, bool>, texts: &mut HashMap<usize, String>, colors: &mut HashMap<usize, Color>, skip: &mut Vec<usize>) {
    let find = |path: &str| panel.find(path);
    let empty = state.field.text.is_empty();
    set.extend(find(&format!("{MASTERING_SEARCH}/Text Area/Placeholder")).map(|at| (at, empty)));
    set.extend(find(&format!("{MASTERING_SEARCH}/Text Area/Text")).map(|at| (at, !empty)));
    if let Some(at) = find(&format!("{MASTERING_SEARCH}/Text Area/Text")) {
        texts.insert(at, state.field.text.clone());
    }
    // With the keys, its words are drawn over its selection's box or its
    // caret (`draw_field`).
    if let (true, Some(field)) = (state.field.focused && !empty, look.screen.search) {
        skip.push(field.text);
    }
    for (view, tab) in [View::List, View::Grid].into_iter().zip(MASTERING_TABS) {
        let chosen = state.view == view;
        for (end, on) in [("Normal", !chosen), ("Selected", chosen)] {
            set.extend(find(&format!("{tab}/{end}")).map(|at| (at, on)));
        }
    }
    for (role, selector) in MASTERING_DROPDOWNS.iter().enumerate() {
        let open = state.open == Some(role);
        for (end, on) in [("Open Panel", open), ("Button/Closed Image", !open), ("Button/Open Image", open)] {
            set.extend(find(&format!("{selector}/{end}")).map(|at| (at, on)));
        }
        if let (Some(at), Some(line)) = (find(&format!("{selector}/Text")), lines_of(said, rows, role).get(state.choice[role])) {
            texts.insert(at, line.clone());
        }
        let lit = cursor.is_some_and(|cursor| dropdown_places(panel, selector, look.screen.line.as_ref(), screen).is_some_and(|at| inside(at.boxed, cursor) || (open && inside(at.open, cursor))));
        if let (true, Some(ground), Some(light)) = (lit, find(&format!("{selector}/Background")), look.screen.dropdown_highlight) {
            if let Some(image) = panel.nodes[ground].image {
                let c = image.color;
                colors.insert(ground, Color::srgba(c[0] * light[0], c[1] * light[1], c[2] * light[2], c[3] * light[3]));
            }
        }
        skip.extend(find(&format!("{selector}/Open Panel")));
    }
    skip.extend(look.screen.view_scroll.and_then(|scroll| scroll.bar).and_then(|bar| bar.handle));
    skip.extend([MASTERING_LIST, MASTERING_GRID].iter().filter_map(|path| find(path)));
}

/// What the page's drawing needs besides the look: the inventory's
/// pictures and the icons to ask for those missing, the database (a
/// weapon's default build), whether the system is on.
pub(crate) struct Pictures<'a> {
    pub(crate) carried: &'a crate::screens::inventory_view::Carried,
    pub(crate) icons: &'a mut crate::screens::icons::Icons,
    pub(crate) db: &'a trp_data::Database,
    pub(crate) on: bool,
    pub(crate) images: &'a mut Assets<Image>,
}

/// A picture turned as the page turns it (`TURN_DEGREES`, the engine's 45
/// the other way about), in a square of its own as large as the turned
/// picture's corners reach, `((w + h) / √2)` a side, a pixel to a pixel:
/// each of its pixels sampled between the four nearest of the picture
/// (their colours weighed by their alpha, so that the clear ground does
/// not darken the edges). Bevy cuts a turned image by moving each corner
/// of its square inside the cut and its picture's place with it, as if it
/// were upright, which drew a rifle cut by its frame thinner and longer
/// than it is;
/// this one is drawn upright, and cut as any picture is.
pub(super) fn slanted(images: &mut Assets<Image>, upright: &Handle<Image>) -> Option<(Handle<Image>, [u32; 2])> {
    let image = images.get(upright)?;
    let (w, h) = (image.width() as usize, image.height() as usize);
    let data = image.data.as_ref().filter(|data| data.len() >= w * h * 4 && w > 0 && h > 0)?;
    let side = (((w + h) as f32) * std::f32::consts::FRAC_1_SQRT_2).ceil() as usize;
    let (sin, cos) = (-TURN_DEGREES).to_radians().sin_cos();
    let texel = |x: isize, y: isize| -> [f32; 4] {
        if x < 0 || y < 0 || x >= w as isize || y >= h as isize {
            return [0.0; 4];
        }
        let at = (y as usize * w + x as usize) * 4;
        let a = data[at + 3] as f32 / 255.0;
        [data[at] as f32 * a, data[at + 1] as f32 * a, data[at + 2] as f32 * a, a]
    };
    let mut out = vec![0u8; side * side * 4];
    let (half_out, half_w, half_h) = (side as f32 / 2.0, w as f32 / 2.0, h as f32 / 2.0);
    for v in 0..side {
        for u in 0..side {
            // The pixel's middle from the square's, the screen's y down;
            // turned back onto the picture (turned the other way about on
            // the screen: its right edge up and to the right).
            let (px, py) = (u as f32 + 0.5 - half_out, v as f32 + 0.5 - half_out);
            let (sx, sy) = (px * cos - py * sin + half_w - 0.5, px * sin + py * cos + half_h - 0.5);
            let (x0, y0) = (sx.floor(), sy.floor());
            let (fx, fy) = (sx - x0, sy - y0);
            let (x0, y0) = (x0 as isize, y0 as isize);
            let mut c = [0.0f32; 4];
            for (dx, dy, weight) in [(0, 0, (1.0 - fx) * (1.0 - fy)), (1, 0, fx * (1.0 - fy)), (0, 1, (1.0 - fx) * fy), (1, 1, fx * fy)] {
                let t = texel(x0 + dx, y0 + dy);
                for i in 0..4 {
                    c[i] += t[i] * weight;
                }
            }
            if c[3] > 1e-4 {
                let at = (v * side + u) * 4;
                for i in 0..3 {
                    out[at + i] = (c[i] / c[3]).round().clamp(0.0, 255.0) as u8;
                }
                out[at + 3] = (c[3] * 255.0).round().clamp(0.0, 255.0) as u8;
            }
        }
    }
    let size = bevy::render::render_resource::Extent3d { width: side as u32, height: side as u32, depth_or_array_layers: 1 };
    let made = Image::new(size, bevy::render::render_resource::TextureDimension::D2, out, image.texture_descriptor.format, bevy::asset::RenderAssetUsages::RENDER_WORLD);
    Some((images.add(made), [w as u32, h as u32]))
}

/// A weapon as the MASTERING page pictures it: its stock build, or the
/// weapon alone where the install has no build of it (the X-17, the
/// AK-50), as the game's `GetPresetItem` falls back to the bare item.
pub(super) fn stock_or_bare(db: &trp_data::Database, template: &str) -> trp_data::character::inventory::Loadout {
    use trp_data::character::inventory::{Carried, Loadout};
    Loadout::of_preset(db, template).unwrap_or_else(|_| {
        let id = "bare".to_owned();
        let alone = Carried { id: id.clone(), template: template.to_owned(), parent: None, slot: None, place: None, count: 1, folded: false };
        Loadout { equipment: id, items: vec![alone] }
    })
}

/// A weapon's picture's key and cells (`ItemIconView.Show` with
/// `GetPresetItem`: its default build's), found once.
fn picture_of<'a>(state: &'a mut MasteryState, db: &trp_data::Database, template: &str) -> Option<&'a (String, [u32; 2])> {
    state
        .pictures
        .entry(template.to_owned())
        .or_insert_with(|| {
            let item = trp_sim::items::inventory::Item::of_list(db, &stock_or_bare(db, template)).map(|(item, _)| item).ok()?;
            Some((crate::screens::icons::picture_key(&item), item.cells()))
        })
        .as_ref()
}

/// Whether each listed row's picture is had, for the drawing's key.
pub(crate) fn pictures_had(state: &mut MasteryState, rows: &[Row], pictures: &mut Pictures) -> Vec<bool> {
    let listed = listed(rows, state.choice, &state.field.text);
    listed
        .iter()
        .map(|&n| {
            let Some((key, cells)) = picture_of(state, pictures.db, &rows[n].template).cloned() else { return false };
            let had = pictures.carried.pictures.contains_key(&key);
            if !had {
                pictures.icons.want(key, cells);
            }
            had
        })
        .collect()
}

/// What the mouse lights, for the drawing's key: each dropdown's box (its
/// list with it, open), the page's bar, the open list's bar.
pub(crate) fn lit(panel: &ScenePanel, look: &MasteryLook, state: &MasteryState, cursor: Option<[f32; 2]>, screen: [f32; 2]) -> [bool; 4] {
    let Some(cursor) = cursor else { return [false; 4] };
    let boxes: [bool; 2] = std::array::from_fn(|role| {
        let open = state.open == Some(role);
        dropdown_places(panel, MASTERING_DROPDOWNS[role], look.screen.line.as_ref(), screen).is_some_and(|at| inside(at.boxed, cursor) || (open && inside(at.open, cursor)))
    });
    let over = |bar: Option<trp_data::interface::screen::ScrollBar>| bar.and_then(|bar| rect_of(panel, &panel.nodes[bar.object].path, screen)).is_some_and(|rect| inside(rect, cursor));
    let page_bar = over(look.screen.view_scroll.and_then(|scroll| scroll.bar));
    let list_bar = state.open.is_some_and(|role| over(scrolling_bar(panel, role)));
    [boxes[0], boxes[1], page_bar, list_bar]
}

/// The page's key for drawing it afresh: what it shows.
pub(crate) fn key(state: &MasteryState, rows: &[Row], had: &[bool]) -> String {
    let field = &state.field;
    format!("{:?} {:?} {:?} {:?} {} {:?} {} {} {} {:?} {rows:?} {had:?}", state.view, state.choice, state.open, state.list_scroll, state.scroll, field.text, field.focused, field.caret, field.select, field.blink_since)
}

/// The search field while it has the keys (`TMP_InputField`; inventory
/// spec, "A text field"): its selection's box or its caret where they
/// stand in its words (`draw_field_marks`), then its words over them
/// (`page_marks` leaves them out of the page's drawing for that: the
/// game's box and caret lie under the text).
#[allow(clippy::too_many_arguments)]
fn draw_field(column: &mut ChildSpawnerCommands, look: &HealthLook, mastery: &MasteryLook, panel: &ScenePanel, said: &Words, state: &MasteryState, screen: [f32; 2], s: f32) {
    let Some(field) = mastery.screen.search.filter(|_| state.field.focused) else { return };
    let Some(place) = field_place(look, panel, field.text, &state.field.text, Frame::In(screen), s) else { return };
    draw_field_marks(column, &place, &field, &state.field, s);
    let Some(page_drawn) = look.skills.as_ref().map(|skills| &skills.page_drawn).filter(|_| !state.field.text.is_empty()) else { return };
    let words: HashMap<usize, String> = [(field.text, state.field.text.clone())].into();
    let set: HashMap<usize, bool> = [(field.text, true)].into();
    let marks = Marks { set: Some(&set), words: Some(&words), within: &[field.text], ..Marks::default() };
    draw_nodes(column, look, panel, page_drawn, crate::screens::skills_view::PAGE, marks, said, Frame::In(screen), [0.0, 0.0], s);
}

/// The page's handles as their bars are lit, and the page's rows where
/// they lie: drawn after the page's objects (`skills_view::draw_page`).
/// Gives where each row's icon lies in the grid, for the mouse.
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_rows(column: &mut ChildSpawnerCommands, look: &HealthLook, mastery: &MasteryLook, panel: &ScenePanel, said: &Words, rows: &[Row], state: &mut MasteryState, pictures: &mut Pictures, cursor: Option<[f32; 2]>, screen: [f32; 2], s: f32) {
    let listed = listed(rows, state.choice, &state.field.text);
    let page = scrolled(panel, mastery, state.view, listed.len(), screen);
    let place = |[x, y, w, h]: [f32; 4]| Node { position_type: PositionType::Absolute, left: Val::Px(x * s), top: Val::Px(y * s), width: Val::Px(w * s), height: Val::Px(h * s), ..default() };
    let mut targets = Vec::new();
    if let Some(page) = page.as_ref() {
        let [vx, vy, _, vh] = page.view;
        let cell = page.grid.cell_in(page.width);
        let list = state.view == View::List;
        column.spawn((Node { overflow: Overflow::clip(), ..place(page.view) }, Visibility::Inherited)).with_children(|clip| {
            for (n, &at) in listed.iter().enumerate() {
                let [gx, gy] = page.grid.place(n, page.width);
                let (x, y) = (page.corner[0] + gx, page.corner[1] + gy - state.scroll);
                if y + cell[1] < vy || y > vy + vh {
                    continue;
                }
                let row = &rows[at];
                let picture = picture_of(state, pictures.db, &row.template).and_then(|(key, cells)| Some((pictures.carried.pictures.get(key)?, *cells)));
                let built = built_view(mastery, said, row, list, picture.is_some(), pictures.on);
                let marks = Marks { set: Some(&built.set), words: Some(&built.texts), groups: Some(&built.groups), sprites: Some(&built.sprites), ..Marks::default() };
                draw_nodes(clip, look, &built.panel, &built.drawn, VIEWS + (n << 8), marks, said, Frame::Is(cell), [x - vx, y - vy], s);
                // The weapon's picture, in its frame that cuts it.
                if let (Some((picture, cells)), Some(frame), Some(image)) = (picture, built.frame, built.picture) {
                    let [fx, fy, fw, fh] = built.panel.rect(frame, cell);
                    let [ix, iy, iw, ih] = built.panel.rect(image, cell);
                    let middle = [ix + iw / 2.0 - fx, iy + ih / 2.0 - fy];
                    let alpha = if pictures.on { 1.0 } else { DIM };
                    let upright = picture.upright().clone();
                    let turned = state.slanted.entry(upright.id()).or_insert_with(|| slanted(pictures.images, &upright)).clone();
                    clip.spawn((Node { overflow: Overflow::clip(), ..place([x - vx + fx, y - vy + fy, fw, fh]) }, Visibility::Inherited)).with_children(|frame| match turned {
                        // At its own size, a unit to a pixel, turned and
                        // scaled as `ItemIconView.UpdateScale` has it (the
                        // smaller of its width over its size's turned
                        // width and its height over its turned height),
                        // about the picture's object's middle.
                        Some((turned, [w, h])) => {
                            let (w, h) = (w as f32, h as f32);
                            let share = [w / ((w - h).abs() * std::f32::consts::FRAC_1_SQRT_2), h / ((w + h) * std::f32::consts::FRAC_1_SQRT_2)].into_iter().filter(|share| share.is_finite()).fold(f32::MAX, f32::min);
                            let side = ((w + h) * std::f32::consts::FRAC_1_SQRT_2).ceil() * share;
                            let node = place([middle[0] - side / 2.0, middle[1] - side / 2.0, side, side]);
                            frame.spawn((ImageNode::new(turned).with_color(Color::WHITE.with_alpha(alpha)), node));
                        }
                        // Its pixels not to be had: turned by the screen.
                        None => {
                            let own = [cells[0] as f32 * crate::screens::inventory_view::CELL, cells[1] as f32 * crate::screens::inventory_view::CELL];
                            let size = crate::screens::inventory_view::quick_fit(cells, own);
                            let node = place([middle[0] - size.x / 2.0, middle[1] - size.y / 2.0, size.x, size.y]);
                            frame.spawn((crate::screens::inventory_view::picture_image(picture, alpha), node, UiTransform { rotation: Rot2::degrees(TURN_DEGREES), ..default() }));
                        }
                    });
                }
                if let Some(icon) = built.icon {
                    let [ix, iy, iw, ih] = built.panel.rect(icon, cell);
                    targets.push(([x + ix, y + iy, iw, ih], row.template.clone()));
                }
            }
        });
        // The bar's handle as its bar is lit; none where all shows.
        if let Some(scrolling) = mastery.screen.view_scroll {
            if let (Some(bar), false) = (scrolling.bar, scrolling.hides && page.whole <= page.shown) {
                if let Some(rect) = bar_handle(panel, bar, screen, page.whole, page.shown, state.scroll) {
                    column.spawn((place(rect), EXACT, BackgroundColor(handle_colour(panel, bar, cursor, screen, state.holding.map(|(held, _)| held) == Some(Held::Page)))));
                }
            }
        }
    }
    state.targets = targets;
    draw_field(column, look, mastery, panel, said, state, screen, s);
    // The list open, over all of it.
    let Some(role) = state.open else { return };
    let selector = MASTERING_DROPDOWNS[role];
    let find = |path: &str| panel.find(path);
    let (Some(open), Some(area)) = (find(&format!("{selector}/Open Panel")), find(&format!("{selector}/Open Panel/Scroll Area"))) else { return };
    let page_drawn = look.skills.as_ref().map(|skills| &skills.page_drawn);
    let Some(page_drawn) = page_drawn else { return };
    let handle = find(&format!("{selector}/Open Panel/Scrollbar/Sliding Area/Handle"));
    let skip: Vec<usize> = handle.into_iter().collect();
    let set: HashMap<usize, bool> = [(open, true)].into();
    let marks = Marks { set: Some(&set), within: &[open], skip: &skip, ..Marks::default() };
    draw_nodes(column, look, panel, page_drawn, crate::screens::skills_view::PAGE, marks, said, Frame::In(screen), [0.0, 0.0], s);
    let [ax, ay, aw, ah] = panel.placed_in(area, screen).rect;
    let content = find(&format!("{selector}/Open Panel/Scroll Area/Content"));
    let row = content.and_then(|content| panel.nodes[content].group).unwrap_or_default();
    let [cx, cy, cw, _] = content.map_or([ax, ay, aw, ah], |content| panel.placed_in(content, screen).rect);
    let lines = lines_of(said, rows, role);
    if let Some(line) = &mastery.screen.line {
        let line_height = line.asks(0, 1).0.max(1.0);
        let width = cw - row.padding[0] - row.padding[1];
        let text = (0..line.nodes.len()).find(|&index| line.nodes[index].text.is_some());
        column.spawn((Node { overflow: Overflow::clip(), ..place([ax, ay, aw, ah]) }, Visibility::Inherited)).with_children(|clip| {
            for (n, words) in lines.iter().enumerate() {
                let top = cy - ay + row.padding[2] + n as f32 * (line_height + row.spacing) - state.list_scroll[role];
                if top + line_height < 0.0 || top > ah {
                    continue;
                }
                let said_line: HashMap<usize, String> = text.map(|text| (text, words.clone())).into_iter().collect();
                let marks = Marks { words: Some(&said_line), ..Marks::default() };
                draw_nodes(clip, look, line, &mastery.line_drawn, LINES + (n << 8), marks, said, Frame::Is([width, line_height]), [cx - ax + row.padding[0], top], s);
            }
        });
    }
    let places = dropdown_places(panel, selector, mastery.screen.line.as_ref(), screen);
    if let (Some(at), Some(bar)) = (places, scrolling_bar(panel, role)) {
        let (whole, shown) = at.heights(lines.len());
        if whole > shown {
            if let Some(rect) = bar_handle(panel, bar, screen, whole, shown, state.list_scroll[role]) {
                column.spawn((place(rect), EXACT, BackgroundColor(handle_colour(panel, bar, cursor, screen, state.holding.map(|(held, _)| held) == Some(Held::List(role))))));
            }
        }
    }
}

/// A bar's handle's colour, its own tinted by its bar's state (held, under
/// the mouse, neither).
fn handle_colour(panel: &ScenePanel, bar: trp_data::interface::screen::ScrollBar, cursor: Option<[f32; 2]>, screen: [f32; 2], held: bool) -> Color {
    let own = bar.handle.and_then(|handle| panel.nodes[handle].image).map_or([1.0; 4], |image| image.color);
    let bar_rect = rect_of(panel, &panel.nodes[bar.object].path, screen);
    let over = cursor.is_some_and(|cursor| bar_rect.is_some_and(|rect| inside(rect, cursor)));
    let tint = bar.tints.map_or([1.0; 4], |tints| {
        let t = if held { tints.pressed } else if over { tints.highlighted } else { tints.normal };
        t.map(|c| (c * tints.multiplier).min(1.0))
    });
    Color::srgba(own[0] * tint[0], own[1] * tint[1], own[2] * tint[2], own[3] * tint[3])
}

/// A row's view as built for drawing: its panel, what is drawn of it,
/// what its scripts write and put on it, and the places of its picture,
/// its frame and its icon (what the mouse finds).
pub(super) struct Built {
    pub(super) panel: ScenePanel,
    pub(super) drawn: Drawn,
    pub(super) texts: HashMap<usize, String>,
    set: HashMap<usize, bool>,
    sprites: HashMap<usize, Picture>,
    groups: HashMap<usize, f32>,
    picture: Option<usize>,
    frame: Option<usize>,
    icon: Option<usize>,
}

/// A row's view in the list (`MasteringPanel.Show` with its
/// `MasteringIcon`) or the grid (`MasteringIcon.Show`): in the list the
/// weapon's short name, its type, "Level: N", the fill; in both the points
/// and the level's mark; the picture's loader while the picture is not
/// had. Dimmed with the options' `WEAPON MASTERING` off.
fn built_view(look: &MasteryLook, said: &Words, row: &Row, list: bool, had: bool, on: bool) -> Built {
    let (mut panel, drawn) = if list { (look.screen.detailed.clone(), look.detailed_drawn.clone()) } else { (look.screen.thumb.clone(), look.thumb_drawn.clone()) };
    let icon = if list { "Skill Icon/" } else { "" };
    let (level, points, mark) = row_words(said, &row.shown);
    let mut texts = HashMap::new();
    let mut set = HashMap::new();
    let mut sprites = HashMap::new();
    let mut groups = HashMap::new();
    let mut put = |panel: &ScenePanel, path: &str, words: String| {
        if let Some(index) = panel.find(path) {
            texts.insert(index, words);
        }
    };
    put(&panel, &format!("{icon}Mastering/Text"), mark);
    if list {
        put(&panel, "Name", row.short.clone());
        put(&panel, "Type", row.kind.clone());
        put(&panel, "Level", level);
        put(&panel, "Progress", points);
        if let Some(fill) = panel.find("Fill Bar/Current Fill") {
            if let Some(image) = panel.nodes[fill].image.as_mut() {
                image.fill_amount = row.shown.share;
            }
            if let Some(picture) = look.fill {
                sprites.insert(fill, picture);
            }
        }
    } else {
        put(&panel, "Points", points);
    }
    let picture = panel.find(&format!("{icon}{PICTURE}"));
    set.extend(picture.map(|index| (index, false)));
    set.extend(panel.find(&format!("{icon}{LOADER}")).map(|index| (index, !had)));
    if !on {
        groups.insert(0, DIM);
    }
    let frame = panel.find(&format!("{icon}{FRAME}"));
    let icon_at = if list { panel.find("Skill Icon") } else { Some(0) };
    Built { panel, drawn, texts, set, sprites, groups, picture, frame, icon: icon_at }
}

/// The mastering tooltip for a row (`MasteringTooltip.Show`): the
/// weapon's name and type, "Level: N", the fill, the points as .NET
/// writes a `Single`; laid out as its groups lay it, its size what they
/// ask.
fn built_tip(look: &HealthLook, mastery: &MasteryLook, said: &Words, row: &Row) -> Option<(ScenePanel, Drawn, HashMap<usize, String>, [f32; 2])> {
    let (mut panel, drawn) = mastery.tooltip.clone()?;
    let (level, _, _) = row_words(said, &row.shown);
    let points = format!("<color=#c5c3b2><b>{}</b></color>/{}", crate::screens::health_view::trimmed_figure(row.shown.into), row.shown.takes);
    let mut texts = HashMap::new();
    for (path, words) in [("Header/Name", row.name.clone()), ("Header/Type", row.kind.clone()), ("Progress Panel/Level", level), ("Progress Panel/Progress", points)] {
        texts.extend(panel.find(path).map(|index| (index, words)));
    }
    if let Some(fill) = panel.find("Progress Panel/Fill Bar/Current Fill") {
        if let Some(image) = panel.nodes[fill].image.as_mut() {
            image.fill_amount = row.shown.share;
        }
    }
    look.ask_by_content(&mut panel, &drawn, &texts);
    let size = [panel.asks(0, 0).1, panel.asks(0, 1).1];
    Some((panel, drawn, texts, size))
}

/// The tooltip over a grid's icon: drawn into its layer, its size given
/// back for the layer's node.
pub(crate) fn draw_tip(tip: &mut ChildSpawnerCommands, look: &HealthLook, mastery: &MasteryLook, said: &Words, rows: &[Row], state: &mut MasteryState, s: f32) -> Option<[f32; 2]> {
    let row = rows.iter().find(|row| Some(&row.template) == state.hover.as_ref())?;
    let (panel, drawn, texts, size) = built_tip(look, mastery, said, row)?;
    let marks = Marks { words: Some(&texts), ..Marks::default() };
    draw_nodes(tip, look, &panel, &drawn, TIP, marks, said, Frame::Is(size), [0.0, 0.0], s);
    state.tip_size = size;
    Some(size)
}

/// The tooltip's key for drawing it afresh.
pub(crate) fn tip_key(state: &MasteryState, rows: &[Row]) -> String {
    let row = rows.iter().find(|row| Some(&row.template) == state.hover.as_ref());
    format!("mastering {row:?}")
}

/// Where the tooltip's corner goes: 25 right of the mouse and 25 down,
/// moved back onto the screen where it would leave it
/// (`CorrectPositionResolution`), on whole pixels; none with no row under
/// the mouse.
pub(crate) fn tip_at(state: &MasteryState, cursor: [f32; 2], screen: [f32; 2]) -> Option<[f32; 2]> {
    state.hover.as_ref()?;
    let size = state.tip_size;
    let mut at = [cursor[0] + TIP_OFFSET, cursor[1] + TIP_OFFSET];
    if at[0] + size[0] > screen[0] {
        at[0] = screen[0] - size[0];
    }
    at[0] = at[0].max(0.0);
    if at[1] + size[1] > screen[1] {
        at[1] = screen[1] - size[1];
    }
    at[1] = at[1].max(0.0);
    Some(at)
}

/// The rows in the app's own look (`TARKOV UI` off), under its list of
/// skills: each begun family's weapons, their level and points in the
/// game's words, faint with the options' `WEAPON MASTERING` off.
pub(crate) fn draw_own(column: &mut ChildSpawnerCommands, said: &Words, rows: &[Row], on: bool) {
    use crate::screens::inventory_view::{words, FAINT, WORDS};
    column.spawn((Node { margin: UiRect::vertical(Val::Px(6.0)), ..default() }, words("WEAPON MASTERING", 12.0, FAINT)));
    if rows.is_empty() {
        column.spawn(words("No family begun: draw a weapon, or set a family's level on the options' SKILLS page.", 10.0, FAINT));
    }
    let colour = if on { WORDS } else { FAINT };
    for row in rows {
        let (level, _, _) = row_words(said, &row.shown);
        column.spawn(Node { column_gap: Val::Px(10.0), ..default() }).with_children(|line| {
            line.spawn((Node { width: Val::Px(220.0), ..default() }, words(row.short.clone(), 13.0, colour)));
            line.spawn((Node { width: Val::Px(140.0), ..default() }, words(row.kind.clone(), 12.0, FAINT)));
            line.spawn(words(level, 12.0, colour));
            line.spawn(words(format!("{}/{}", row.shown.into as i32, row.shown.takes), 12.0, FAINT));
        });
    }
}
