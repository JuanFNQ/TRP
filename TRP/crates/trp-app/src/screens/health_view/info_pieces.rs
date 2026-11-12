//! The inspect window's mastering, buttons and slots.

use super::*;

/// A weapon's mastering (`ItemSpecificationPanel.method_1`,
/// `MasteringCharacteristicPanel.SetValues`): its row named
/// "WEAPON MASTERING (0N)" with "points/points the level takes" and its
/// fill to their share; its badge under the preview with "0N", and the
/// preview's corner beside it; none of it for anything else.
pub(super) fn info_mastering(look: &HealthLook, panel: &mut trp_data::interface::screen::ScenePanel, marks: &mut PartsMarks, mastering: Option<&InfoMastering>) {
    for path in [INFO_MASTERING, INFO_MASTERING_BADGE, INFO_CORNER] {
        if let Some(index) = panel.find(path) {
            marks.set.insert(index, mastering.is_some());
            panel.nodes[index].active |= mastering.is_some();
        }
    }
    let Some(mastering) = mastering else { return };
    let level = format!("0{}", mastering.level + 1);
    let at = |panel: &trp_data::interface::screen::ScenePanel, end: &str| panel.find(&format!("{INFO_MASTERING}/{end}"));
    if let Some(name) = at(panel, "Name") {
        marks.texts.insert(name, format!("WEAPON MASTERING ({level})"));
    }
    if let Some(value) = at(panel, "Value") {
        marks.texts.insert(value, format!("{}/{}", trimmed_figure(mastering.points), mastering.most.round()));
    }
    let share = inverse_lerp(0.0, mastering.most, mastering.points);
    if let Some(full) = at(panel, "bars/Full") {
        panel.nodes[full].layout.anchor_max[0] = share;
    }
    if let Some(delta) = at(panel, "bars/Delta") {
        panel.nodes[delta].layout.anchor_min[0] = share;
        panel.nodes[delta].layout.anchor_max[0] = share;
    }
    // The badge: its icon and its level at their own sizes in its row.
    let badge = format!("{INFO_MASTERING_BADGE}/MasteringText");
    if let Some(text) = panel.find(&badge) {
        let size = text_size(look, panel, text, &level).unwrap_or([20.0, 15.0]);
        prefer_size(panel, text, size);
        marks.texts.insert(text, level);
    }
    if let Some(icon) = panel.find(&format!("{INFO_MASTERING_BADGE}/MasteringIcon")) {
        let size = panel.nodes[icon].image.and_then(|_| Some(panel.nodes[icon].layout.size_delta)).filter(|size| size[0] > 0.0).unwrap_or([19.0, 25.0]);
        prefer_size(panel, icon, size);
    }
}

/// A figure as C# writes a float (`ToString()`): no decimals where it is
/// whole.
pub(crate) fn trimmed_figure(value: f32) -> String {
    let written = format!("{value}");
    written.strip_suffix(".0").map(str::to_owned).unwrap_or(written)
}

/// The window's buttons (`InteractionButtonsContainer.Show`, the thing's
/// lines but Inspect): a copy of a template for each (Discard's own, the
/// wish list's own, the rest's), its caption at its font's width, its icon,
/// its arrow where it opens lines of its own, at 0.3 where it cannot be
/// done; in the container's grid (`GridLayoutGroup`: three columns, filled
/// row by row from the top left, the block in its middle), the container
/// as large as the grid asks. The places of the copies' tops, each with its
/// ground's node, colours and alpha (`light_info_buttons`).
#[allow(clippy::type_complexity)]
pub(super) fn info_buttons(look: &HealthLook, panel: &mut trp_data::interface::screen::ScenePanel, drawn: &mut Drawn, marks: &mut PartsMarks, buttons: &[InfoButtonLook]) -> Vec<(usize, Option<(usize, [Color; 2], f32)>)> {
    let Some(holder) = panel.find(INFO_BUTTONS) else { return Vec::new() };
    let templates: Vec<Option<usize>> = INFO_BUTTON_TEMPLATES.iter().map(|path| panel.find(path)).collect();
    let mut tops = Vec::new();
    for button in buttons {
        let template = match button.name {
            "Discard" => templates[2],
            "AddToWishlist" => templates[1],
            _ => templates[0],
        };
        let Some(template) = template else { continue };
        let first = copy_within(panel, drawn, template, holder);
        let named = |panel: &trp_data::interface::screen::ScenePanel, name: &str| (first..panel.nodes.len()).find(|&index| panel.nodes[index].name == name);
        if let Some(text) = named(panel, "Text") {
            let caption = button.caption.to_uppercase();
            let width = text_size(look, panel, text, &caption).map_or(0.0, |size| size[0]);
            let flexible = panel.nodes[text].element.map_or([1.0, -1.0], |element| element.flexible);
            panel.nodes[text].element = Some(trp_data::interface::screen::Element { ignore: false, min: [-1.0, -1.0], preferred: [width, -1.0], flexible });
            marks.texts.insert(text, caption);
        }
        if let (Some(icon), Some(picture)) = (named(panel, "Icon"), look.menu_icons.get(button.name)) {
            marks.sprites.insert(icon, *picture);
        }
        if let Some(arrow) = named(panel, "SubmenuArrow") {
            if button.arrow {
                let size = drawn.pictures.get(arrow).copied().flatten().map_or([9.0, 11.0], |picture| picture.size);
                panel.nodes[arrow].element = Some(trp_data::interface::screen::Element { ignore: false, min: [-1.0, -1.0], preferred: size, flexible: [-1.0, -1.0] });
            } else {
                marks.set.insert(arrow, false);
            }
        }
        if !button.enabled {
            marks.groups.insert(first, 0.3);
        }
        let icon_holder = named(panel, "IconPlaceholder").and_then(|index| panel.nodes[index].parent);
        let ground = (first..panel.nodes.len()).find(|&index| panel.nodes[index].name == "Background" && panel.nodes[index].parent == icon_holder);
        let line_colors = panel.script(first, "ContextMenuButton").and_then(menu_line_colors);
        if let (Some(ground), Some([normal, _])) = (ground, line_colors) {
            marks.colors.insert(ground, normal);
        }
        tops.push((first, ground.zip(line_colors).map(|(ground, colors)| (ground, colors, if button.enabled { 1.0 } else { 0.3 }))));
    }
    // The templates themselves are not drawn.
    for template in templates.into_iter().flatten() {
        marks.set.insert(template, false);
    }
    let Some(grid) = panel.grid(holder) else { return tops };
    let columns = grid.columns.unwrap_or(3).max(1) as usize;
    let count = tops.len();
    let rows = count.div_ceil(columns);
    let [cell_w, cell_h] = grid.cell;
    let [space_w, space_h] = grid.spacing;
    let width = grid.padding[0] + grid.padding[1] + columns as f32 * (cell_w + space_w) - space_w;
    let height = if rows == 0 { 0.0 } else { grid.padding[2] + grid.padding[3] + rows as f32 * (cell_h + space_h) - space_h };
    let shown_columns = columns.min(count.max(1));
    let block = shown_columns as f32 * (cell_w + space_w) - space_w;
    let start = grid.padding[0] + (width - grid.padding[0] - grid.padding[1] - block) * (grid.alignment % 3) as f32 * 0.5;
    for (n, &(top, _)) in tops.iter().enumerate() {
        let layout = &mut panel.nodes[top].layout;
        layout.anchor_min = [0.0, 1.0];
        layout.anchor_max = [0.0, 1.0];
        layout.pivot = [0.0, 1.0];
        layout.offset = [start + (n % columns) as f32 * (cell_w + space_w), -(grid.padding[2] + (n / columns) as f32 * (cell_h + space_h))];
        layout.size_delta = [cell_w, cell_h];
    }
    let element = panel.nodes[holder].element.unwrap_or(trp_data::interface::screen::Element { ignore: false, min: [-1.0, -1.0], preferred: [-1.0, -1.0], flexible: [-1.0, -1.0] });
    panel.nodes[holder].element = Some(trp_data::interface::screen::Element { min: [width, height], preferred: [width, height], ..element });
    tops
}

/// The parts' panel (`CreateModSlots`): on for a thing with slots, a copy
/// of the slot's view for each (`ModSlotView`), full or empty as its slot
/// is (`SetSlotGraphics`: the full border, or the empty border, the ground,
/// the slot's picture at its own size, its name and its filters' marks),
/// in the grid (`GridLayoutGroup`: 64 square, 3 apart, as many to a row as
/// fit, the block in the middle of the top), the grid as tall as its rows;
/// the window at least as wide as six slots, or a third of them, ask with
/// the paddings about them. The places of the copies' tops, and each empty
/// slot's filters' row with its marks' pictures.
#[allow(clippy::type_complexity)]
pub(super) fn info_slots(look: &HealthLook, panel: &mut trp_data::interface::screen::ScenePanel, drawn: &mut Drawn, marks: &mut PartsMarks, slots: &[InfoSlot]) -> (Vec<usize>, Vec<(usize, Vec<Picture>)>) {
    let (Some(mods), Some(content)) = (panel.find(INFO_MODS), panel.find(INFO_MODS_CONTENT)) else { return (Vec::new(), Vec::new()) };
    let template = look.info_rows.as_ref().and_then(|rows| rows.mod_slot.as_ref());
    let (false, Some(template)) = (slots.is_empty(), template) else {
        // Off, and out of the layout (`ItemSpecificationPanel.Show`: the
        // panel's object switched off where the thing has no slot and no
        // chamber). The prefab has it on, 20 units tall: hidden but laid
        // out, it put 26 units between a medicine's rows and its
        // description.
        marks.set.insert(mods, false);
        panel.nodes[mods].active = false;
        return (Vec::new(), Vec::new());
    };
    panel.nodes[mods].active = true;
    marks.set.insert(mods, true);
    // (Its scrollbar is never wanted: the rows take as many as fit.)
    if let Some(bar) = panel.find(INFO_MODS_BAR) {
        marks.set.insert(bar, false);
    }
    let mut tops = Vec::new();
    let mut filters = Vec::new();
    for slot in slots {
        let top = graft_row(panel, drawn, template, content);
        let at = |end: &str| template.0.find(end).map(|index| top + index);
        // (A chamber not checked has its name and picture shown whatever it
        // holds: `SetLocked` after `Show`.)
        for (end, on) in [("CanvasGroup/Empty Border", !slot.full), ("CanvasGroup/Full Border", slot.full), ("CanvasGroup/Background", !slot.full), ("CanvasGroup/Text", !slot.full || slot.unchecked), ("CanvasGroup/filters", !slot.full)] {
            if let Some(index) = at(end) {
                marks.set.insert(index, on);
            }
        }
        if let Some(text) = at("CanvasGroup/Text") {
            // What of it fits the slot (its text's ellipsis: "…" where a
            // name is wider than the slot, `ModSlotView._slotName`).
            let fitted = template.0.find("CanvasGroup/Text").and_then(|index| {
                let look_of = template.0.nodes[index].text.as_ref()?;
                let face = look.tmp_faces.get(&(template.0.file_of(look_of.font.0)?, look_of.font.1))?;
                Some(fitted_words(face, look_of, &slot.name, template.0.rect(index, [64.0, 64.0])[2]))
            });
            marks.texts.insert(text, fitted.unwrap_or_else(|| slot.name.clone()));
        }
        // Its picture, at its own size in the middle (`SetSlotBackImage`).
        if let Some(back) = at("CanvasGroup/Background Image") {
            let picture = look.slot_pictures.get(&slot.picture).or_else(|| slot.armour.then(|| look.slot_pictures.get("armor_plate")).flatten()).copied();
            // A chamber not checked: the question's picture, at its own size
            // (`SetNativeSize`: 36 by 45 units, the take's 48 by 60 pixels).
            let picture = match slot.unchecked {
                true => look.info_rows.as_ref().and_then(|rows| rows.unchecked_chamber),
                false => picture.filter(|_| !slot.full),
            };
            match picture {
                Some(picture) => {
                    marks.set.insert(back, true);
                    marks.sprites.insert(back, picture);
                    panel.nodes[back].layout.size_delta = picture.size;
                }
                None => {
                    marks.set.insert(back, false);
                }
            }
        }
        if let (false, Some(row)) = (slot.full, at("CanvasGroup/filters")) {
            let pictures: Vec<Picture> = slot.marks.iter().filter_map(|mark| look.mod_pictures.get(&format!("mod_{mark}")).copied()).collect();
            if !pictures.is_empty() {
                filters.push((row, pictures));
            }
        }
        tops.push(top);
    }
    let size = fitted_size(panel);
    let Some(grid) = panel.grid(content) else { return (tops, filters) };
    let width = panel.rect(content, size)[2];
    let [cell_w, cell_h] = grid.cell;
    let [space_w, space_h] = grid.spacing;
    let inner = width - grid.padding[0] - grid.padding[1];
    let columns = (((inner + space_w) / (cell_w + space_w)).floor() as usize).max(1);
    let count = tops.len();
    let shown_columns = columns.min(count);
    let block = shown_columns as f32 * (cell_w + space_w) - space_w;
    let start = grid.padding[0] + (inner - block) * (grid.alignment % 3) as f32 * 0.5;
    for (n, &top) in tops.iter().enumerate() {
        let layout = &mut panel.nodes[top].layout;
        layout.anchor_min = [0.0, 1.0];
        layout.anchor_max = [0.0, 1.0];
        layout.pivot = [0.0, 1.0];
        layout.offset = [start + (n % columns) as f32 * (cell_w + space_w), -(grid.padding[2] + (n / columns) as f32 * (cell_h + space_h))];
        layout.size_delta = [cell_w, cell_h];
    }
    let rows = count.div_ceil(columns);
    let height = grid.padding[2] + grid.padding[3] + rows as f32 * (cell_h + space_h) - space_h;
    let element = panel.nodes[content].element.unwrap_or(trp_data::interface::screen::Element { ignore: false, min: [-1.0, -1.0], preferred: [-1.0, -1.0], flexible: [-1.0, -1.0] });
    panel.nodes[content].element = Some(trp_data::interface::screen::Element { min: [element.min[0], height], preferred: [element.preferred[0], height], ..element });
    // The window's least width (`_layoutElement.minWidth`): the paddings
    // of the groups from the grid up to the window, and six slots or a
    // third of them, whichever is more.
    let mut padding = grid.padding[0] + grid.padding[1];
    let mut above = panel.nodes[content].parent;
    while let Some(index) = above {
        padding += panel.nodes[index].group.map_or(0.0, |group| group.padding[0] + group.padding[1]);
        above = panel.nodes[index].parent;
    }
    let least_columns = count.div_ceil(3).max(6) as f32;
    let least = padding + cell_w * least_columns + space_w * (least_columns - 1.0);
    let root = panel.nodes[0].element.unwrap_or(trp_data::interface::screen::Element { ignore: false, min: [-1.0, -1.0], preferred: [-1.0, -1.0], flexible: [-1.0, -1.0] });
    panel.nodes[0].element = Some(trp_data::interface::screen::Element { min: [least, root.min[1]], ..root });
    (tops, filters)
}

/// A panel's top's size as its fitter makes it (`ContentSizeFitter`:
/// least 1, preferred 2, by axis), else as it is kept.
pub(super) fn fitted_size(panel: &trp_data::interface::screen::ScenePanel) -> [f32; 2] {
    let mut size = info_size(panel);
    if let Some(fitter) = panel.script(0, "ContentSizeFitter") {
        for (axis, kept) in size.iter_mut().enumerate() {
            let mode = fitter.get(4 * axis..4 * axis + 4).map_or(0, |word| u32::from_le_bytes(word.try_into().unwrap_or_default()));
            let (least, preferred, _) = panel.asks(0, axis);
            match mode {
                1 => *kept = least,
                2 => *kept = preferred,
                _ => {}
            }
        }
    }
    size
}

/// Mathf.InverseLerp: where a figure lies between two, held within 0 and
/// 1 (0 where they are the same).
pub(super) fn inverse_lerp(from: f32, to: f32, value: f32) -> f32 {
    if from == to { 0.0 } else { ((value - from) / (to - from)).clamp(0.0, 1.0) }
}

/// A copy of one of the window's row templates put last under `holder`,
/// its objects after the panel's own, numbered apart by their place; the
/// place of the copy's top.
pub(super) fn graft_row(panel: &mut trp_data::interface::screen::ScenePanel, drawn: &mut Drawn, (source, source_drawn): &(trp_data::interface::screen::ScenePanel, Drawn), holder: usize) -> usize {
    let first = panel.nodes.len();
    let under = panel.nodes[holder].path.clone();
    for (old, node) in source.nodes.iter().enumerate() {
        let mut node = node.clone();
        // Its texts' fonts and materials and its picture count in the
        // source's table of files: put them in the panel's (a slot's
        // template is `resources.assets`', the window `sharedassets49`'s).
        if let Some(text) = node.text.as_mut() {
            text.font.0 = rebase_file(panel, source, text.font.0);
            if let Some(material) = text.material.as_mut() {
                material.0 = rebase_file(panel, source, material.0);
            }
        }
        if let Some(sprite) = node.image.as_mut().and_then(|image| image.sprite.as_mut()) {
            sprite.0 = rebase_file(panel, source, sprite.0);
        }
        node.path = format!("{under}/#{first}/{}", node.path);
        node.parent = Some(node.parent.map_or(holder, |parent| first + parent));
        node.active |= old == 0;
        panel.nodes.push(node);
        drawn.shown.push(source_drawn.shown.get(old).copied().unwrap_or(Shown::Never));
        drawn.pictures.push(source_drawn.pictures.get(old).copied().flatten());
    }
    first
}

/// A file's number in `source`'s table of files (0 its own, else one past
/// its place among its outside files) as `panel` counts it, the file added
/// to `panel`'s outside files where it has it not.
fn rebase_file(panel: &mut trp_data::interface::screen::ScenePanel, source: &trp_data::interface::screen::ScenePanel, index: i32) -> i32 {
    let Some(name) = source.file_of(index) else { return index };
    if name == panel.file {
        return 0;
    }
    if let Some(at) = panel.externals.iter().position(|outside| outside.rsplit('/').next() == Some(name.as_str())) {
        return at as i32 + 1;
    }
    let full = if index == 0 { source.file.clone() } else { source.externals.get(index as usize - 1).cloned().unwrap_or(name) };
    panel.externals.push(full);
    panel.externals.len() as i32
}
