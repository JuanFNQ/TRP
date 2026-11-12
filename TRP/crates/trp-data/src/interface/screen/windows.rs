//! The repair, grid, info and tag windows and their rows.

use super::*;

/// What the repair window's scripts keep beside its objects
/// (`RepairerParametersPanel`, `ConditionCharacteristicsSlider`): the
/// object shown only for a trader (`_repairerStandingGameObject`), the
/// icons a kit's points take in place of the cash's
/// (`_repairPointsInStashSprite`, `_repairPointsToPaySprite`, each a file
/// and a number), the sum's colours when it is more than is had and
/// otherwise (`_criticalColor`, `_defaultColor`), and the condition bar's
/// (its border and current durability coloured so, critical for a broken
/// item).
#[derive(Debug, Clone, Default)]
pub struct RepairFigures {
    pub standing: Option<usize>,
    pub in_stash: Option<(String, i64)>,
    pub to_pay: Option<(String, i64)>,
    pub sum_colors: [[f32; 4]; 2],
    pub slider_colors: [[f32; 4]; 2],
}

pub fn repair_figures(panel: &ScenePanel) -> Result<RepairFigures> {
    let parameters = panel.find("Inner/Contents/RepairerPanel").context("the repair window's repairer panel")?;
    let fields = panel.script(parameters, "RepairerParametersPanel").context("its script")?;
    let mut r = Reader::new(fields);
    skip_odin(&mut r)?;
    // Its 23 objects' pointers, in its fields' order, then its two colours.
    let start = r.pos;
    let mut pointers = Vec::new();
    for _ in 0..23 {
        pointers.push(pointer(&mut r)?);
    }
    let sum_colors = [color(&mut r)?, color(&mut r)?];
    let sprite = |at: usize| -> Option<(String, i64)> { pointers.get(at).filter(|to| to.path != 0).and_then(|to| Some((panel.file_of(to.file)?, to.path))) };
    let standing = panel.pointer_at(fields, start + 12 * 3);
    let slider = panel.find("Inner/Contents/RepairerPanel/ExceptionItemGroup/ConditionPanel/ConditionSlider").context("the condition's slider")?;
    let fields = panel.script(slider, "ConditionCharacteristicsSlider").context("its script")?;
    let mut r = Reader::new(fields);
    skip_odin(&mut r)?;
    r.skip(9 * 12)?;
    let slider_colors = [color(&mut r)?, color(&mut r)?];
    Ok(RepairFigures { standing, in_stash: sprite(17), to_pay: sprite(21), sum_colors, slider_colors })
}

/// The game's window of a container's grids (`GridWindow`; inventory
/// spec, "A container's window"): its prefab (`ItemUiContext.
/// _gridWindowTemplate`), the objects its script works, and the colours
/// it gives its icon and its caption's ground, as it is the priority
/// window or not (`_iconIdleColor`, `_iconSelectedColor`,
/// `_headerIdleColor`, `_headerSelectedColor`).
#[derive(Debug, Clone)]
pub struct GridWindowLook {
    pub panel: ScenePanel,
    pub caption_panel: usize,
    pub icon: usize,
    pub caption: usize,
    pub close: usize,
    pub priority: Option<usize>,
    pub icon_colors: [[f32; 4]; 2],
    pub header_colors: [[f32; 4]; 2],
}

/// Where the grid window's colours lie among its script's fields: the
/// icon's two after the eleven references of the window and its own (128
/// bytes in), the caption ground's two after one more reference (172:
/// `local\gridwindow_fields_s130.txt`).
const GRID_WINDOW_ICON_COLORS: usize = 128;

const GRID_WINDOW_HEADER_COLORS: usize = 172;

pub fn grid_window(install: &Install) -> Result<GridWindowLook> {
    let panel = context_window(install, "GridWindow").context("the item context points to no window of a container's grids")?;
    let find = |path: &str| panel.find(path).with_context(|| format!("the grid window has no {path}"));
    let fields = panel.script(0, "GridWindow").context("the grid window's script")?;
    let colors = |at: usize| -> Option<[[f32; 4]; 2]> {
        let mut r = Reader::new(fields.get(at..)?);
        Some([color(&mut r).ok()?, color(&mut r).ok()?])
    };
    let caption_panel = find("Caption Panel")?;
    let own = panel.nodes[caption_panel].image.map_or([1.0; 4], |image| image.color);
    Ok(GridWindowLook {
        caption_panel,
        icon: find("Caption Panel/Icon")?,
        caption: find("Caption Panel/Caption")?,
        close: find("Caption Panel/Close Button")?,
        priority: panel.find("Caption Panel/Priority"),
        icon_colors: colors(GRID_WINDOW_ICON_COLORS).unwrap_or([[1.0; 4]; 2]),
        header_colors: colors(GRID_WINDOW_HEADER_COLORS).unwrap_or([own; 2]),
        panel,
    })
}

/// A window the preloader's item context makes (`ItemUiContext`, in the
/// scene's shared file), found among the prefabs its script points to by
/// the script on the prefab's top.
pub(super) fn context_window(install: &Install, class: &str) -> Result<ScenePanel> {
    let index = scene_paths(install)?.iter().position(|path| path.ends_with("UI/PreloaderUIScene.unity")).context("the build has no preloader's scene")?;
    let file = format!("level{index}");
    let scene = Scene::load(install, &file, true)?;
    let context = (0..scene.nodes.len()).find(|&node| scene.path(node).ends_with("/UIContext")).with_context(|| format!("{file} has no item context"))?;
    let panel = ScenePanel::of(&scene, context);
    let fields = panel.script(0, "ItemUiContext").context("the item context's script")?;
    let mut tried = std::collections::HashSet::new();
    for start in (0..fields.len().saturating_sub(11)).step_by(4) {
        let mut r = Reader::new(&fields[start..]);
        let to = pointer(&mut r)?;
        if to.file < 1 || to.path <= 0 || to.path >= 1 << 40 || !tried.insert((to.file, to.path)) {
            continue;
        }
        let Some(shared) = panel.file_of(to.file).filter(|name| name.starts_with("sharedassets")) else { continue };
        let Ok(prefab) = Scene::load_prefab_of_script(install, &shared, to.path) else { continue };
        let window = ScenePanel { file: shared.clone(), ..ScenePanel::of(&prefab, 0) };
        if window.script(0, class).is_some() {
            return Ok(window);
        }
    }
    bail!("the item context points to no {class}")
}

/// The inspect window's row templates (inspect-window spec, "A row"): a
/// bar row (`CharacteristicPanel`), a compact row
/// (`CompactCharacteristicPanel`) and a row with a dropdown
/// (`CompactCharacteristicDropdownPanel`), the prefabs the window's script
/// points to in its own file.
#[derive(Debug, Clone, Default)]
pub struct InfoRows {
    pub bar: ScenePanel,
    pub compact: ScenePanel,
    pub dropdown: ScenePanel,
    /// A slot of the parts' panel (`_modSlotViewPrefab`, `ModSlotView`), a
    /// prefab of another file (`resources.assets`), where it was found.
    pub mod_slot: Option<ScenePanel>,
    /// The picture such a slot shows for a chamber not checked
    /// (`ModSlotView._uncheckedChamberSprite`: `icon_question`), its file and
    /// number, where it was found.
    pub unchecked_chamber: Option<(String, i64)>,
}

/// The picture a slot of the parts' panel shows for a chamber not checked
/// (`ModSlotView._uncheckedChamberSprite`): the script's last fields are
/// `_canvasGroup`, `_slotName`, `_filtersGroup` and that sprite, each a
/// pointer (12 bytes), so it is the pointer two after the one to the slot's
/// name (`CanvasGroup/Text`), the one between leading to its filters
/// (`CanvasGroup/filters`).
pub fn unchecked_chamber_sprite(slot: &ScenePanel) -> Option<(String, i64)> {
    let fields = slot.script(0, "ModSlotView")?;
    let (text, filters) = (slot.find("CanvasGroup/Text")?, slot.find("CanvasGroup/filters")?);
    let name = (0..fields.len().saturating_sub(11)).step_by(4).find(|&start| slot.pointer_at(fields, start) == Some(text))?;
    if slot.transform_at(fields, name + 12) != Some(filters) && slot.pointer_at(fields, name + 12) != Some(filters) {
        return None;
    }
    let to = pointer(&mut Reader::new(fields.get(name + 24..)?)).ok()?;
    Some((slot.file_of(to.file)?, to.path)).filter(|(_, id)| *id != 0)
}

/// The inspect window's row templates, by its own script's pointers into
/// its file (`ItemSpecificationPanel`), each known by the script on the
/// prefab's top.
pub fn info_rows(install: &Install, window: &ScenePanel) -> Result<InfoRows> {
    let fields = window.script(0, "ItemSpecificationPanel").context("the window has no specification panel")?;
    let (mut bar, mut compact, mut dropdown, mut mod_slot) = (None, None, None, None);
    let mut tried = std::collections::HashSet::new();
    for start in (0..fields.len().saturating_sub(11)).step_by(4) {
        let mut r = Reader::new(&fields[start..]);
        let Ok(to) = pointer(&mut r) else { continue };
        if !(0..64).contains(&to.file) || to.path <= 0 || to.path >= 1 << 40 || !tried.insert((to.file, to.path)) {
            continue;
        }
        // (A pointer of the window's own file to one of its objects is not
        // a prefab of its own.)
        if to.file == 0 && window.ids.contains_key(&to.path) {
            continue;
        }
        let Some(file) = window.file_of(to.file) else { continue };
        let Ok(prefab) = Scene::load_prefab_of_script(install, &file, to.path) else { continue };
        let mut panel = ScenePanel::of(&prefab, 0);
        panel.file = file.clone();
        if panel.script(0, "ModSlotView").is_some() {
            mod_slot.get_or_insert(panel);
        } else if panel.script(0, "CharacteristicPanel").is_some() {
            bar.get_or_insert(panel);
        } else if panel.script(0, "CompactCharacteristicDropdownPanel").is_some() {
            dropdown.get_or_insert(panel);
        } else if panel.script(0, "CompactCharacteristicPanel").is_some() {
            compact.get_or_insert(panel);
        }
    }
    let unchecked_chamber = mod_slot.as_ref().and_then(unchecked_chamber_sprite);
    Ok(InfoRows {
        bar: bar.context("the window's bar row is not had")?,
        compact: compact.context("the window's compact row is not had")?,
        dropdown: dropdown.context("the window's dropdown row is not had")?,
        mod_slot,
        unchecked_chamber,
    })
}

/// The most letters a text field takes (`TMP_InputField.m_CharacterLimit`),
/// from its script's stored fields, by TextMeshPro's published field order:
/// after the selectable's trigger names (the last, `Disabled`), its
/// interactable flag and target graphic, the field's six pointers, its
/// scroll sensitivity and eight settings (content, input and keyboard
/// types, the asterisk, the line type, the two keyboard flags, the
/// validation), its regular expression (a string) and its global point
/// size. None where the trigger is not found.
pub fn input_limit(fields: &[u8]) -> Option<u32> {
    let needle = b"\x08\x00\x00\x00Disabled";
    let at = fields.windows(needle.len()).position(|window| window == needle)? + needle.len();
    let regex = at + 4 + 12 + 6 * 12 + 4 + 8 * 4;
    let word = |at: usize| fields.get(at..at + 4).map(|word| u32::from_le_bytes(word.try_into().unwrap_or_default()));
    let length = word(regex)? as usize;
    let after = regex + 4 + length.div_ceil(4) * 4;
    word(after + 4)
}

/// The game's window that edits a thing's tag (`EditTagWindow`, its colours'
/// panel and its input): the prefab the preloader's item context makes it
/// from (`ItemUiContext._editTagWindowTemplate`), found as the window that
/// shows a thing is.
pub fn tag_window(install: &Install) -> Result<ScenePanel> {
    item_context_window(install, "EditTagWindow")
}

/// A window the preloader's item context makes from a prefab of the
/// scene's shared files, found among the prefabs its script points to by
/// the window's own script.
fn item_context_window(install: &Install, script: &str) -> Result<ScenePanel> {
    let index = scene_paths(install)?.iter().position(|path| path.ends_with("UI/PreloaderUIScene.unity")).context("the build has no preloader's scene")?;
    let file = format!("level{index}");
    let scene = Scene::load(install, &file, true)?;
    let context = (0..scene.nodes.len()).find(|&node| scene.path(node).ends_with("/UIContext")).with_context(|| format!("{file} has no item context"))?;
    let panel = ScenePanel::of(&scene, context);
    let fields = panel.script(0, "ItemUiContext").context("the item context's script")?;
    let mut tried = std::collections::HashSet::new();
    for start in (0..fields.len().saturating_sub(11)).step_by(4) {
        let mut r = Reader::new(&fields[start..]);
        let to = pointer(&mut r)?;
        if to.file < 1 || to.path <= 0 || to.path >= 1 << 40 || !tried.insert((to.file, to.path)) {
            continue;
        }
        let Some(shared) = panel.file_of(to.file).filter(|name| name.starts_with("sharedassets")) else { continue };
        let Ok(prefab) = Scene::load_prefab_of_script(install, &shared, to.path) else { continue };
        let window = ScenePanel { file: shared.clone(), ..ScenePanel::of(&prefab, 0) };
        if window.script(0, script).is_some() {
            return Ok(window);
        }
    }
    bail!("the item context points to no {script}")
}

#[cfg(test)]
mod input_tests {
    /// The tag's field as the install stores it: the limit, 16, after the
    /// trigger names, the pointers, the settings, an empty expression and
    /// the point size, 24.
    #[test]
    fn a_text_field_s_limit_is_read_after_its_settings() {
        let mut fields = vec![0u8; 252];
        fields.extend_from_slice(b"\x08\x00\x00\x00Disabled");
        fields.extend(std::iter::repeat_n(0u8, 4 + 12 + 72 + 36));
        fields.extend_from_slice(&0u32.to_le_bytes());
        fields.extend_from_slice(&24f32.to_le_bytes());
        fields.extend_from_slice(&16u32.to_le_bytes());
        assert_eq!(super::input_limit(&fields), Some(16));
        assert_eq!(super::input_limit(&[0u8; 64]), None);
    }
}
