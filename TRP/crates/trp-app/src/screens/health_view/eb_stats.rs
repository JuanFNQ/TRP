//! The edit build screen's stats bars built and drawn.

use super::*;

/// The edit build screen's characteristics panel laid out (`EbStatsLook`
/// filled by `eb_stats_built`): what `draw_eb_stats` draws, its size, the
/// places of its switcher and its INFO button (each folds or unfolds it),
/// and each bar's comparison piece with its colour, which pulses.
pub(crate) struct EbStatsBuilt {
    pub(super) panel: trp_data::interface::screen::ScenePanel,
    pub(super) drawn: Drawn,
    pub(super) texts: HashMap<usize, String>,
    pub(super) set: HashMap<usize, bool>,
    pub(super) colors: HashMap<usize, Color>,
    pub(super) sprites: HashMap<usize, Picture>,
    pub(crate) size: [f32; 2],
    pub(crate) switches: Vec<[f32; 4]>,
    pub(crate) pulse: Vec<(usize, Color)>,
}

/// The base the characteristics panel's pieces are numbered from.
pub(crate) const EB_STATS: usize = 23 << 20;

/// The comparison's alpha at a time (`CharacteristicPanel.Update`: 0.2 and
/// a ping-pong of 0.4, gone through at 0.8 a second: from 0.2 to 0.6 in
/// half a second and back in the next).
pub(crate) fn compare_alpha(seconds: f32) -> f32 {
    let travelled = seconds / 0.5 * 0.4;
    let phase = travelled % 0.8;
    0.2 + if phase < 0.4 { phase } else { 0.8 - phase }
}

/// One bar of the panel (`CharacteristicPanel.SetValues`): its fill to the
/// lesser of the figure with and without its change, the change to the
/// other; against the figure compared with (the build as the screen was
/// given it, or with the part the mouse rests on in its slot), the
/// difference drawn in the comparison's piece in the adding or the
/// subtracting colour, and the change's difference written after the value
/// in brackets.
#[allow(clippy::too_many_arguments)]
fn eb_bar(panel: &mut trp_data::interface::screen::ScenePanel, at: &dyn Fn(&trp_data::interface::screen::ScenePanel, &str) -> Option<usize>, (range, base, delta, less_is_good): ([f32; 2], f32, f32, bool), compare: Option<(f32, f32)>, row_look: &trp_data::interface::screen::RowLook, (add, sub): (Option<Picture>, Option<Picture>), (colors, sprites, set): (&mut HashMap<usize, Color>, &mut HashMap<usize, Picture>, &mut HashMap<usize, bool>), pulse: &mut Vec<(usize, Color)>) -> Option<String> {
    let place = |value: f32| inverse_lerp(range[0], range[1], value);
    let inverted = range[0] > range[1];
    let (own_change, (cbase, cdelta)) = (base * delta, compare.unwrap_or((base, delta)));
    let total = base + own_change;
    let (compare_change, compare_total) = (cbase * cdelta, cbase + cbase * cdelta);
    let difference = compare_total - total;
    let (n8, n9, n11, b) = (place(total), place(base), place(compare_total), place(cbase));
    let (n10, x) = (n8.min(n9), n8.max(n9));
    let (b2, n12) = (n11.min(b), (n11 - n8).abs());
    let (full, delta_piece, compare_piece) = (at(panel, "bars/Full"), at(panel, "bars/Delta"), at(panel, "bars/CompareDelta"));
    if let Some(piece) = delta_piece {
        let lowers = (own_change < 0.0 && !inverted) || (own_change > 0.0 && inverted);
        if let Some(picture) = if lowers { sub } else { add } {
            sprites.insert(piece, picture);
        }
        if let Some(bar) = &row_look.bar {
            let better = if less_is_good { own_change <= 0.0 } else { own_change > 0.0 };
            colors.insert(piece, rgba(if better { bar.add } else { bar.sub }));
        }
    }
    let anchor = |panel: &mut trp_data::interface::screen::ScenePanel, piece: Option<usize>, from: Option<f32>, to: f32| {
        if let Some(piece) = piece {
            if let Some(from) = from {
                panel.nodes[piece].layout.anchor_min[0] = from;
            }
            panel.nodes[piece].layout.anchor_max[0] = to;
        }
    };
    if difference.abs() > f32::EPSILON && compare_piece.is_some() {
        let piece = compare_piece.unwrap_or_default();
        set.insert(piece, true);
        if let Some(picture) = add {
            sprites.insert(piece, picture);
        }
        if let Some(bar) = &row_look.bar {
            let better = if less_is_good { difference <= 0.0 } else { difference > 0.0 };
            let color = rgba(if better { bar.add } else { bar.sub });
            colors.insert(piece, color.with_alpha(compare_alpha(0.0)));
            pulse.push((piece, color));
        }
        if difference > 0.0 {
            anchor(panel, full, None, n10.min(b2));
            anchor(panel, delta_piece, Some(n10.min(b2)), n8.min(n11));
            anchor(panel, compare_piece, Some(n8.min(n11)), n8.max(n11));
        } else {
            anchor(panel, full, None, n10.min(b2));
            if compare_total < base {
                anchor(panel, compare_piece, Some(n10.min(b2)), n10.min(b2) + n12);
                anchor(panel, delta_piece, Some(n10.min(b2) + n12), n8.max(n11));
            } else {
                anchor(panel, delta_piece, Some(n10.min(b2)), n8.max(n11) - n12);
                anchor(panel, compare_piece, Some(n8.max(n11) - n12), n8.max(n11));
            }
        }
    } else {
        if let Some(piece) = compare_piece {
            set.insert(piece, false);
        }
        anchor(panel, full, None, n10);
        anchor(panel, delta_piece, Some(n10), x);
    }
    let more = compare_change - own_change;
    (more.abs() > f32::EPSILON).then(|| format!("({}{})", if more > 0.0 { "+" } else { "" }, more.round() as i64))
}

/// The edit build screen's characteristics panel for a build's figures,
/// against those of the build it is compared with (`CharacteristicsPanel.
/// InstantiateItemFields`): its durability row (compared with nothing), its
/// weight (over 0 to 15 kg, less better, compared), then in one column each
/// bar and each compact row in the figures' order; folded to its INFO
/// button, or not (`SetState`).
#[allow(clippy::too_many_arguments)]
pub(crate) fn eb_stats_built(look: &HealthLook, figures: &trp_sim::items::item_attributes::Figures, compare: &trp_sim::items::item_attributes::Figures, (weight, compare_weight): (f32, f32), weight_name: &str, expanded: bool) -> Option<EbStatsBuilt> {
    use trp_sim::items::item_attributes::{Show, Table};
    let stats = look.edit_build.as_ref()?.stats.as_ref()?;
    let mut panel = stats.panel.clone();
    let mut drawn = stats.drawn.clone();
    let (mut texts, mut set, mut colors, mut sprites, mut pulse) = (HashMap::new(), HashMap::new(), HashMap::new(), HashMap::new(), Vec::new());
    let (add, sub) = look.info_rows.as_ref().map_or((None, None), |rows| (rows.add, rows.sub));
    for (path, on) in [("CharacteristicsExpanded", expanded), ("InfoButton", !expanded)] {
        if let Some(index) = panel.find(path) {
            set.insert(index, on);
            panel.nodes[index].active = on;
        }
    }
    let size_of = |panel: &trp_data::interface::screen::ScenePanel, index: usize, said: &str| -> Option<[f32; 2]> {
        let text = panel.nodes[index].text.as_ref()?;
        let face = look.tmp_faces.get(&(panel.file_of(text.font.0)?, text.font.1))?;
        Some([face.font.width(said, text.size, text.spacing, text.style & 1 != 0) + text.margin[0] + text.margin[2], face.font.height(text.size) + text.margin[1] + text.margin[3]])
    };
    let prefer = |panel: &mut trp_data::interface::screen::ScenePanel, index: usize, size: [f32; 2]| {
        let element = panel.nodes[index].element.unwrap_or(trp_data::interface::screen::Element { ignore: false, min: [-1.0, -1.0], preferred: [-1.0, -1.0], flexible: [-1.0, -1.0] });
        panel.nodes[index].element = Some(trp_data::interface::screen::Element { preferred: size, ..element });
    };
    let within = |top: String| move |panel: &trp_data::interface::screen::ScenePanel, end: &str| panel.find(&format!("{top}/{end}"));
    // The durability row, as the inspect window's.
    match (figures.durability, panel.find("CharacteristicsExpanded/Durability")) {
        (Some(durability), Some(row)) => {
            let at = within(panel.nodes[row].path.clone());
            let round = |value: f32| ((value as f64 * 10.0).round() / 10.0) as f32;
            let tint = if durability.most < 10.0 { "#FF0000" } else { "#FFFFFF" };
            if let Some(value) = at(&panel, "Value") {
                texts.insert(value, format!("{}/<color={tint}>{}</color> ({})", round(durability.now), round(durability.most), durability.peak.round()));
            }
            if let Some(name) = at(&panel, "Name") {
                texts.insert(name, figures.durability_name.clone());
            }
            let share = durability.now / durability.peak.max(1e-6);
            if let Some(bars) = trp_data::interface::screen::row_look(&panel.subtree(row), "DurabilityPanel").and_then(|look| look.bar) {
                let color = rgba(if share > 0.15 { bars.current } else { bars.low });
                for end in ["bars/Full", "bars/Border"] {
                    if let Some(index) = at(&panel, end) {
                        colors.insert(index, color);
                    }
                }
            }
            if let Some(full) = at(&panel, "bars/Full") {
                panel.nodes[full].layout.anchor_max[0] = share;
            }
            if let Some(delta) = at(&panel, "bars/Delta") {
                panel.nodes[delta].layout.anchor_max[0] = inverse_lerp(0.0, durability.peak, durability.most);
            }
            if let Some(compare) = at(&panel, "bars/CompareDelta") {
                set.insert(compare, false);
            }
        }
        (None, Some(row)) => {
            set.insert(row, false);
            panel.nodes[row].active = false;
        }
        _ => {}
    }
    // The weight (`_charWeight`): to three places.
    if let Some(row) = panel.find("CharacteristicsExpanded/Weight") {
        let at = within(panel.nodes[row].path.clone());
        let mut said = format!("{weight:.3}");
        if let Some(name) = at(&panel, "Name") {
            texts.insert(name, weight_name.to_owned());
        }
        // (Its change is none, so nothing is written after it.)
        eb_bar(&mut panel, &at, ([0.0, 15.0], weight, 0.0, true), Some((compare_weight, 0.0)), &stats.weight_look, (add, sub), (&mut colors, &mut sprites, &mut set), &mut pulse);
        if let Some(value) = at(&panel, "Value") {
            texts.insert(value, std::mem::take(&mut said));
        }
    }
    // The weight's icon, its attribute's (`Weight`).
    if let (Some(icon), Some(picture)) = (panel.find("CharacteristicsExpanded/Weight/Icon"), look.attribute_icons.get("Weight")) {
        sprites.insert(icon, *picture);
    }
    // The rows, in the container's grid (`_charPanelContainer`, a
    // `FlexibleGridLayoutGroup`): each a cell, in the figures' order.
    let Some(holder) = panel.find("CharacteristicsExpanded/CharsPanel") else { return None };
    let mut cells = Vec::new();
    for row in figures.rows.iter() {
        let bar = matches!(row.show, Show::Bar { .. });
        let (template, row_look) = if bar { (&stats.bar, &stats.bar_look) } else { (&stats.compact, &stats.compact_look) };
        let top = graft_row(&mut panel, &mut drawn, template, holder);
        cells.push(top);
        let at = |panel: &trp_data::interface::screen::ScenePanel, end: &str| template.0.find(end).map(|index| top + index).filter(|index| *index < panel.nodes.len());
        if let Some(name) = at(&panel, "Name") {
            let size = size_of(&panel, name, &row.name).unwrap_or_default();
            prefer(&mut panel, name, size);
            texts.insert(name, row.name.clone());
        }
        let mut said = row.value.clone().unwrap_or_default();
        if let Some(base) = row.colored {
            let (good, bad) = (rgba(row_look.increasing), rgba(row_look.decreasing));
            if let Some(value) = at(&panel, "Value") {
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
        }
        if let Show::Bar { range, base, delta } = row.show {
            let compared = compare.rows.iter().find(|other| other.id == row.id).and_then(|other| match other.show {
                Show::Bar { base, delta, .. } => Some((base, delta)),
                _ => None,
            });
            if let Some(more) = eb_bar(&mut panel, &at, (range, base, delta, row.less_is_good), compared, row_look, (add, sub), (&mut colors, &mut sprites, &mut set), &mut pulse) {
                said.push_str(&more);
            }
        }
        if let Some(value) = at(&panel, "Value") {
            let drawn_words = rich_spans(&said).map_or_else(|| said.clone(), |spans| spans.into_iter().map(|(words, _)| words).collect());
            let size = size_of(&panel, value, &drawn_words).unwrap_or_default();
            prefer(&mut panel, value, size);
            panel.nodes[value].active = true;
            texts.insert(value, said);
        }
        if let Some(icon) = at(&panel, "Icon") {
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
                None => {
                    set.insert(icon, false);
                }
            }
        }
    }
    // The grid's cells: its columns (its fixed count, one where it has
    // none), each as wide as they share it, filled down each column first
    // where its axis is the vertical one; the grid as tall as its rows.
    if let Some(grid) = panel.grid(holder) {
        let width = panel.rect(holder, fitted_size(&panel))[2].max(grid.cell[0]);
        let columns = grid.columns.unwrap_or(1).max(1) as usize;
        let cell_width = (width - (columns as f32 - 1.0) * grid.spacing[0] - grid.padding[0] - grid.padding[1]) / columns as f32;
        let cell_height = if grid.cell[1] > 0.0 { grid.cell[1] } else { 23.0 };
        let down = cells.len().div_ceil(columns).max(1);
        for (n, &top) in cells.iter().enumerate() {
            let (x, y) = if grid.start_axis == 1 { (n / down, n % down) } else { (n % columns, n / columns) };
            let layout = &mut panel.nodes[top].layout;
            layout.anchor_min = [0.0, 1.0];
            layout.anchor_max = [0.0, 1.0];
            layout.pivot = [0.0, 1.0];
            layout.offset = [grid.padding[0] + (cell_width + grid.spacing[0]) * x as f32, -(grid.padding[2] + (cell_height + grid.spacing[1]) * y as f32)];
            layout.size_delta = [cell_width, cell_height];
        }
        let rows_down = cells.len().div_ceil(columns);
        let height = if rows_down == 0 { 0.0 } else { grid.padding[2] + grid.padding[3] + (cell_height + grid.spacing[1]) * rows_down as f32 - grid.spacing[1] };
        let element = panel.nodes[holder].element.unwrap_or(trp_data::interface::screen::Element { ignore: false, min: [-1.0, -1.0], preferred: [-1.0, -1.0], flexible: [-1.0, -1.0] });
        panel.nodes[holder].element = Some(trp_data::interface::screen::Element { min: [element.min[0], height], preferred: [element.preferred[0], height], ..element });
    }
    let size = fitted_size(&panel);
    let switches = ["CharacteristicsExpanded/Switcher", "InfoButton"].iter().zip([expanded, !expanded]).filter(|(_, on)| *on).filter_map(|(path, _)| panel.find(path).map(|index| panel.rect(index, size))).collect();
    Some(EbStatsBuilt { panel, drawn, texts, set, colors, sprites, size, switches, pulse })
}

/// The characteristics panel as `eb_stats_built` laid it out, its top left
/// at `origin` (units).
pub(crate) fn draw_eb_stats(layer: &mut ChildSpawnerCommands, look: &HealthLook, said: &Words, built: &EbStatsBuilt, origin: [f32; 2], s: f32) {
    let marks = Marks { words: Some(&built.texts), set: Some(&built.set), colors: Some(&built.colors), sprites: Some(&built.sprites), ..Marks::default() };
    draw_nodes(layer, look, &built.panel, &built.drawn, EB_STATS, marks, said, Frame::Is(built.size), origin, s);
}
