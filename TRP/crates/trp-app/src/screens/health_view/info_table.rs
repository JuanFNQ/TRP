//! The inspect window's rows, and the window drawn.

use super::*;

/// The thing's figures put into the window (`ItemSpecificationPanel.
/// RecreateAttributeBars`, `CompactCharacteristicPanel.Show` and
/// `SetValues`, `CharacteristicPanel.SetValues`, `DurabilityPanel.
/// SetValues`, `ProcessCompactCells`): a weapon's durability row; a bar row
/// in the column of bars for each bar, a compact or a dropdown row in the
/// compact grid for the rest, each with its icon at its own place (none
/// where it has none), its name and its value at the widths their font
/// draws them, coloured by its sign where it is so shown; each bar's fill
/// and change; the grid in two columns, or one where a row's words would
/// not fit its cell, filled down each column first.
#[allow(clippy::too_many_arguments)]
pub(super) fn info_rows(look: &HealthLook, panel: &mut trp_data::interface::screen::ScenePanel, drawn: &mut Drawn, texts: &mut HashMap<usize, String>, set: &mut HashMap<usize, bool>, colors: &mut HashMap<usize, Color>, sprites: &mut HashMap<usize, Picture>, figures: &trp_sim::items::item_attributes::Figures) -> Vec<(usize, usize)> {
    let mut tips = Vec::new();
    use trp_sim::items::item_attributes::Show;
    // A text's size as its font draws it, its margins with it
    // (`TMP_Text.preferredWidth` and `preferredHeight`), which the row's
    // layout gives it.
    let size_of = |panel: &trp_data::interface::screen::ScenePanel, index: usize, said: &str| -> Option<[f32; 2]> {
        let text = panel.nodes[index].text.as_ref()?;
        let face = look.tmp_faces.get(&(panel.file_of(text.font.0)?, text.font.1))?;
        Some([face.font.width(said, text.size, text.spacing, text.style & 1 != 0) + text.margin[0] + text.margin[2], face.font.height(text.size) + text.margin[1] + text.margin[3]])
    };
    let prefer = |panel: &mut trp_data::interface::screen::ScenePanel, index: usize, size: [f32; 2]| {
        let element = panel.nodes[index].element.unwrap_or(trp_data::interface::screen::Element { ignore: false, min: [-1.0, -1.0], preferred: [-1.0, -1.0], flexible: [-1.0, -1.0] });
        panel.nodes[index].element = Some(trp_data::interface::screen::Element { preferred: size, ..element });
    };
    let off = |panel: &mut trp_data::interface::screen::ScenePanel, drawn: &mut Drawn, index: usize| {
        panel.nodes[index].active = false;
        drawn.shown[index] = Shown::Never;
    };
    // The weapon's durability row: "present/greatest (the template's)",
    // the greatest red under 10; its fill and border in the low colour at
    // 15% of the template's or under; its change to the greatest's share.
    // (With none, the row and all under it are not drawn: its pieces are
    // drawn apart, and its being off does not hide them.)
    if let (None, Some(row)) = (figures.durability, panel.find(INFO_DURABILITY)) {
        let path = format!("{}/", panel.nodes[row].path);
        for index in 0..panel.nodes.len() {
            if index == row || panel.nodes[index].path.starts_with(&path) {
                drawn.shown[index] = Shown::Never;
            }
        }
    }
    if let (Some(durability), Some(row)) = (figures.durability, panel.find(INFO_DURABILITY)) {
        panel.nodes[row].active = true;
        set.insert(row, true);
        let path = panel.nodes[row].path.clone();
        let at = |panel: &trp_data::interface::screen::ScenePanel, end: &str| panel.find(&format!("{path}/{end}"));
        let round = |value: f32| ((value as f64 * 10.0).round() / 10.0) as f32;
        let tint = if durability.most < 10.0 { "#FF0000" } else { "#FFFFFF" };
        let said = format!("{}/<color={tint}>{}</color> ({})", round(durability.now), round(durability.most), durability.peak.round());
        if let Some(value) = at(panel, "Value") {
            texts.insert(value, said);
        }
        if let Some(name) = at(panel, "Name") {
            texts.insert(name, figures.durability_name.clone());
        }
        let share = durability.now / durability.peak.max(1e-6);
        let look_of = trp_data::interface::screen::row_look(&panel.subtree(row), "DurabilityPanel").and_then(|look| look.bar);
        if let Some(bars) = look_of {
            let color = rgba(if share > 0.15 { bars.current } else { bars.low });
            for end in ["bars/Full", "bars/Border"] {
                if let Some(index) = at(panel, end) {
                    colors.insert(index, color);
                }
            }
        }
        if let Some(full) = at(panel, "bars/Full") {
            panel.nodes[full].layout.anchor_max[0] = share;
        }
        if let Some(delta) = at(panel, "bars/Delta") {
            panel.nodes[delta].layout.anchor_max[0] = inverse_lerp(0.0, durability.peak, durability.most);
        }
        if let Some(compare) = at(panel, "bars/CompareDelta") {
            off(panel, drawn, compare);
        }
    }
    let Some(rows) = look.info_rows.as_ref() else { return tips };
    let (Some(bars), Some(grid)) = (panel.find(INFO_BARS), panel.find(INFO_COMPACT)) else { return tips };
    // The compact rows' widths of their words, for the columns.
    let mut compact_widths = Vec::new();
    let mut cells = Vec::new();
    for (number, row) in figures.rows.iter().enumerate() {
        let (template, row_look) = match row.show {
            Show::Bar { .. } => (&rows.bar, &rows.bar_look),
            Show::Compact => (&rows.compact, &rows.compact_look),
            Show::Dropdown => (&rows.dropdown, &rows.compact_look),
        };
        let holder = if matches!(row.show, Show::Bar { .. }) { bars } else { grid };
        let top = graft_row(panel, drawn, template, holder);
        let at = |end: &str| template.0.find(end).map(|index| top + index);
        if let (Show::Dropdown, Some(hover), false) = (&row.show, at("Hover"), row.tip.is_empty()) {
            tips.push((hover, number));
        }
        let mut words_width = 0.0;
        if let Some(name) = at("Name") {
            let size = size_of(panel, name, &row.name).unwrap_or_default();
            prefer(panel, name, size);
            words_width += size[0];
            texts.insert(name, row.name.clone());
        }
        if let Some(value) = at("Value") {
            match &row.value {
                // (`Colored`: "+" before a positive one; "-" before a
                // negative one that has none; each its colour.)
                Some(said) => {
                    let mut said = said.clone();
                    if let Some(base) = row.colored {
                        let (good, bad) = (rgba(row_look.increasing), rgba(row_look.decreasing));
                        if base < 0.0 {
                            colors.insert(value, if row.less_is_good { good } else { bad });
                            if !said.contains('-') {
                                said.insert(0, '-');
                            }
                        } else if base > 0.0 {
                            colors.insert(value, if row.less_is_good { bad } else { good });
                            said.insert(0, '+');
                        }
                    }
                    // (Measured as drawn: a colour tag takes no room.)
                    let drawn_words = rich_spans(&said).map_or_else(|| said.clone(), |spans| spans.into_iter().map(|(words, _)| words).collect());
                    let size = size_of(panel, value, &drawn_words).unwrap_or_default();
                    prefer(panel, value, size);
                    if !said.is_empty() {
                        words_width += size[0] + 5.0;
                    }
                    // Enhanced by a repair kit: the value in the
                    // enhancement's blue, its tooltip under the mouse on it
                    // (`CompactCharacteristicPanel.Show`: `HoverTooltipArea`
                    // on `ValueText`).
                    if row.enhanced {
                        let [r, g, b] = trp_sim::items::item_attributes::ENHANCED;
                        colors.insert(value, Color::srgb_u8(r, g, b));
                        if !row.tip.is_empty() {
                            tips.push((value, number));
                        }
                    }
                    panel.nodes[value].active = true;
                    texts.insert(value, said);
                }
                _ => off(panel, drawn, value),
            }
        }
        // Its icon: the attribute's, else none (`Icon.SetActive`).
        if let Some(icon) = at("Icon") {
            use trp_sim::items::item_attributes::Table;
            let table = match row.table {
                Table::Attribute => &look.attribute_icons,
                Table::Health => &look.figure_icons[0],
                Table::Damage => &look.figure_icons[1],
                Table::Buff => &look.figure_icons[2],
            };
            match table.get(row.id) {
                Some(picture) => {
                    sprites.insert(icon, *picture);
                }
                None => off(panel, drawn, icon),
            }
        }
        if let Show::Bar { range, base, delta } = row.show {
            let change = base * delta;
            let place = |value: f32| inverse_lerp(range[0], range[1], value);
            let (total, own) = (place(base + change), place(base));
            if let Some(full) = at("bars/Full") {
                panel.nodes[full].layout.anchor_max[0] = total.min(own);
            }
            if let Some(index) = at("bars/Delta") {
                panel.nodes[index].layout.anchor_min[0] = total.min(own);
                panel.nodes[index].layout.anchor_max[0] = total.max(own);
                let inverted = range[0] > range[1];
                let lowers = (change < 0.0 && !inverted) || (change > 0.0 && inverted);
                if let Some(picture) = if lowers { rows.sub } else { rows.add } {
                    sprites.insert(index, picture);
                }
                if let Some(bar) = &row_look.bar {
                    let better = if row.less_is_good { change <= 0.0 } else { change > 0.0 };
                    colors.insert(index, rgba(if better { bar.add } else { bar.sub }));
                }
            }
        } else {
            if row.show == Show::Compact {
                compact_widths.push(words_width);
            }
            cells.push((row.show == Show::Dropdown, top));
        }
    }
    // (The dropdown rows are made after all the compact rows, and follow
    // them in the grid.)
    cells.sort_by_key(|(dropdown, _)| *dropdown);
    let cells: Vec<usize> = cells.into_iter().map(|(_, top)| top).collect();
    // The grid: its columns (`ProcessCompactCells`: two, unless 30 and a
    // compact row's words are wider than a cell of two), each cell as wide
    // as its columns share the grid (`FlexibleGridLayoutGroup`), the rows
    // filled down each column first (`GridLayoutGroup`, the vertical axis
    // first, from the top left), the grid as tall as its rows.
    let Some(figures_of) = panel.grid(grid) else { return tips };
    let width = panel.rect(grid, fitted_size(panel))[2];
    let two = (width - figures_of.spacing[0] - figures_of.padding[0] - figures_of.padding[1]) / 2.0;
    let columns = if compact_widths.iter().any(|&words| 30.0 + words > two) { 1 } else { figures_of.columns.unwrap_or(2).max(1) as usize };
    let cell_width = (width - (columns as f32 - 1.0) * figures_of.spacing[0] - figures_of.padding[0] - figures_of.padding[1]) / columns as f32;
    let count = cells.len();
    let down = if count > columns { count.div_ceil(columns) } else { 1 };
    for (n, &top) in cells.iter().enumerate() {
        let (x, y) = if figures_of.start_axis == 1 { (n / down, n % down) } else { (n % columns, n / columns) };
        let layout = &mut panel.nodes[top].layout;
        layout.anchor_min = [0.0, 1.0];
        layout.anchor_max = [0.0, 1.0];
        layout.pivot = [0.0, 1.0];
        layout.offset = [figures_of.padding[0] + (cell_width + figures_of.spacing[0]) * x as f32, -(figures_of.padding[2] + (figures_of.cell[1] + figures_of.spacing[1]) * y as f32)];
        layout.size_delta = [cell_width, figures_of.cell[1]];
    }
    let rows_down = count.div_ceil(columns);
    let height = figures_of.padding[2] + figures_of.padding[3] + (figures_of.cell[1] + figures_of.spacing[1]) * rows_down as f32 - figures_of.spacing[1];
    if height > 0.0 {
        let element = panel.nodes[grid].element.unwrap_or(trp_data::interface::screen::Element { ignore: false, min: [-1.0, -1.0], preferred: [-1.0, -1.0], flexible: [-1.0, -1.0] });
        panel.nodes[grid].element = Some(trp_data::interface::screen::Element { min: [element.min[0], height], preferred: [element.preferred[0], height], ..element });
    }
    tips
}

/// The window's own preferred size as made (its `LayoutElement`'s, 670 by
/// 500), which stretching it starts from.
pub(crate) fn info_preferred(look: &HealthLook) -> Option<[f32; 2]> {
    look.info.as_ref()?.nodes.first()?.element.map(|element| element.preferred).filter(|size| size[0] > 0.0 && size[1] > 0.0)
}

/// The piece the `index`th window that shows a thing draws its loader as,
/// which `info_preview` turns while it shows.
pub(crate) fn info_loader_piece(look: &HealthLook, index: usize) -> Option<usize> {
    look.info.as_ref()?.find(INFO_LOADER).map(|loader| INFO + (index << 12) + loader)
}

/// The window that shows a thing (`InfoWindow`) as `info_built` laid it
/// out, in a frame at its top left corner, `s` pixels to a unit, the
/// `index`th window open. Its model is drawn by the caller into its
/// preview's room (`InfoBuilt::preview`); its description in its box, cut
/// to it (`Mask`; the game's scrolls, not built).
pub fn draw_info_window(column: &mut ChildSpawnerCommands, look: &HealthLook, said: &Words, words: &InfoWords, built: &InfoBuilt, index: usize, s: f32, (opened, scrolled): (u64, f32)) {
    let InfoBuilt { panel, drawn, texts, set, colors, sprites, groups, icons, size, .. } = built;
    let size = *size;
    // (The description is drawn below, and a bar's handle where it moves.)
    let skip: Vec<usize> = panel.find(INFO_DESCRIPTION).into_iter().chain(built.scroll.as_ref().map(|scroll| scroll.handle)).collect();
    let base = INFO + (index << 12);
    draw_nodes(column, look, panel, drawn, base, Marks { words: Some(texts), skip: &skip, set: Some(set), colors: Some(colors), sprites: Some(sprites), groups: Some(groups), ..Marks::default() }, said, Frame::Is(size), [0.0, 0.0], s);
    let place = |x: f32, y: f32, w: f32, h: f32| Node { position_type: PositionType::Absolute, left: Val::Px(x * s), top: Val::Px(y * s), width: Val::Px(w * s), height: Val::Px(h * s), ..default() };
    // An empty slot's filters' marks (`filter_icon`, white: its prefab's
    // colour not read).
    for ([x, y, w, h], picture) in icons {
        if let Some(sprite) = look.sprite_of(*picture, Color::WHITE) {
            draw_sprite(column, &sprite, [x * s, y * s, w * s, h * s], s);
        }
    }
    // The description's bar's handle, where it is scrolled to
    // (`inventory_view::scroll_info_texts` moves it).
    if let Some(scroll) = &built.scroll {
        let [x, y, w, h] = scroll.handle_at(scrolled);
        column.spawn((place(x, y, w, h), BackgroundColor(scroll.color), InfoScrollHandle(opened, scroll.color)));
    }
    let moved = if built.scroll.is_some() { -scrolled * s } else { 0.0 };
    if let (Some(scroll), Some(description)) = (panel.find(INFO_SCROLL), panel.find(INFO_DESCRIPTION)) {
        let Some(text) = panel.nodes[description].text.clone() else { return };
        let [x, y, w, h] = panel.rect(scroll, size);
        let font = TextFont { font: look.styled_font(&panel, text.font, text.style), font_size: text.size * s, weight: weight_of(text.style), ..default() };
        let effect = look.text_effect(&panel, text.font);
        let width = (w - text.margin[0] - text.margin[2]) * s;
        let tmp = look.tmp_of(&panel, &text);
        // In its area, clipped to it, moved up as far as it is scrolled.
        let moving = (Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(moved), width: Val::Percent(100.0), ..default() }, InfoScrollContent(opened));
        column.spawn(Node { overflow: Overflow::clip(), ..place(x, y, w, h) }).with_children(|frame| {
            frame.spawn(moving).with_children(|clip| {
                let room = Node { margin: UiRect::new(Val::Px(text.margin[0] * s), Val::ZERO, Val::Px(text.margin[1] * s), Val::ZERO), ..default() };
                match tmp {
                    // As the game draws it, its lines broken at the box's width.
                    Some((asset, material)) => {
                        let run = trp_data::interface::tmp_text::TmpRun { text: words.description.clone(), size: text.size * s, color: screen_numbers(rgba(text.color)), bold: text.style & 1 != 0 };
                        let layout = trp_data::interface::tmp_text::TmpLayout { em: text.size * s, spacing: text.spacing, kerning: text.kerning, wrap: Some(width), align: text.horizontal & 7, lead: 0.0 };
                        tmp_words(clip, room, TmpText { asset, material, runs: vec![run], layout }, &mut |_| {});
                    }
                    None => effect_words(clip, Node { width: Val::Px(width), ..room }, &[(words.description.clone(), font, rgba(text.color))], effect, text.size * s, &mut |_, _, _| {}),
                }
            });
        });
    }
}
