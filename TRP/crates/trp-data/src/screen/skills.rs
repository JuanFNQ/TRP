//! The game's SKILLS page (`SkillsAndMasteringScreen`, `SkillsScreen`,
//! `SkillContainer`, `SkillPanel`, `SkillIcon`, `BuffIcon`; skills spec,
//! "The SKILLS tab"): its panel in the scene that holds the inventory
//! screen, the templates its scripts make a skill's views from and the
//! grids they are laid in, the pictures the scripts put on them, its
//! dropdowns' line and lists, its scroll view; the skill tooltip in the
//! preloader's scene (`ItemUiContext.SkillTooltip`); and the skills' and
//! the buffs' icons (`StaticIcons.SkillIdSprites`, `BuffIdSprites`).

use super::*;

/// The page's top below the inventory screen.
const PAGE: &str = "/InventoryScreen/SkillsAndMasteringPanel";
/// Where the skills' views are made, below the page's top.
pub const SKILL_LIST: &str = "BottomPanel/SkillsPanel/Lower Part/Scrollview/Content/Skill List";
pub const SKILL_GRID: &str = "BottomPanel/SkillsPanel/Lower Part/Scrollview/Content/Skill Grid";
pub const SKILL_SCROLL: &str = "BottomPanel/SkillsPanel/Lower Part/Scrollview";
/// The two dropdowns, filter and sort, below the page's top.
pub const SKILL_DROPDOWNS: [&str; 2] = ["BottomPanel/SkillsPanel/Options/Filter", "BottomPanel/SkillsPanel/Options/Sort"];

/// A grid of views (`ScenePanel::grid`), and whether it is the game's own
/// kind (`FlexibleGridLayoutGroup`), which widens its cells to fill its
/// width where its columns are held at a count.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CellGrid {
    pub grid: GridGroup,
    pub flexible: bool,
}

impl CellGrid {
    /// How many columns a grid so wide has: its count where it holds the
    /// columns at one (`Constraint.FixedColumnCount`), else as many cells
    /// as fit (`GridLayoutGroup.SetCellsAlongAxis`).
    pub fn columns(&self, width: f32) -> usize {
        let g = &self.grid;
        if let Some(count) = g.columns {
            return count.max(1) as usize;
        }
        let room = width - g.padding[0] - g.padding[1] + g.spacing[0];
        ((room / (g.cell[0] + g.spacing[0]).max(1e-3)).floor() as usize).max(1)
    }

    /// A cell's size in a grid so wide: the game's flexible grid's cells
    /// as wide as its columns share its width (`SetLayoutHorizontal`).
    pub fn cell_in(&self, width: f32) -> [f32; 2] {
        let g = &self.grid;
        match (self.flexible, g.columns) {
            (true, Some(count)) => {
                let n = count.max(1) as f32;
                [(width - (n - 1.0) * g.spacing[0] - g.padding[0] - g.padding[1]) / n, g.cell[1]]
            }
            _ => g.cell,
        }
    }

    /// Where the `n`th cell lies from the grid's top left corner, filled
    /// row by row from the upper left (the page's two grids' own corner and
    /// axis), and how high `count` cells stand.
    pub fn place(&self, n: usize, width: f32) -> [f32; 2] {
        let g = &self.grid;
        let columns = self.columns(width);
        let [w, h] = self.cell_in(width);
        let (column, row) = (n % columns, n / columns);
        [g.padding[0] + column as f32 * (w + g.spacing[0]), g.padding[2] + row as f32 * (h + g.spacing[1])]
    }

    pub fn height(&self, count: usize, width: f32) -> f32 {
        let g = &self.grid;
        let rows = count.div_ceil(self.columns(width));
        let h = self.cell_in(width)[1];
        g.padding[2] + g.padding[3] + rows as f32 * h + rows.saturating_sub(1) as f32 * g.spacing[1]
    }
}

/// The SKILLS page as the scene has it.
#[derive(Debug, Clone, Default)]
pub struct SkillsScreen {
    pub panel: ScenePanel,
    /// A skill's view in the list (`Detailed Skill Panel`: a `SkillPanel`
    /// and a `SkillIcon`) and in the grid (`Skill Icon`), each a panel of
    /// its own, and the grids they are laid in.
    pub detailed: ScenePanel,
    pub thumb: ScenePanel,
    pub list_grid: Option<CellGrid>,
    pub thumb_grid: Option<CellGrid>,
    /// The fill bar's pictures (`SkillPanel._normalFillSprite`,
    /// `_eliteFillSprite`) and a buff's grounds (`BuffIcon.
    /// _normalBackgroundSprite`, `_eliteBackgroundSprite`), each its file and
    /// number.
    pub fill_sprites: [Option<(String, i64)>; 2],
    pub buff_grounds: [Option<(String, i64)>; 2],
    /// A dropdown's line, its tints, its ground's colour under the mouse
    /// and the most of its list in sight, as the OVERALL tab's.
    pub line: Option<ScenePanel>,
    pub line_tints: Option<Tints>,
    pub dropdown_highlight: Option<[f32; 4]>,
    pub dropdown_most_seen: Option<f32>,
    /// A dropdown's list's scroll view, and the page's own.
    pub list_scroll: Option<Scrolling>,
    pub view_scroll: Option<Scrolling>,
    /// Each text the install's words fill (`LocalizedText`).
    pub localized: Vec<(usize, String, u32)>,
}

/// A pointer among a script's last `count` (twelve bytes each), the `n`th:
/// its file's name and its number, where it leads somewhere.
fn last_pointer(panel: &ScenePanel, fields: &[u8], count: usize, n: usize) -> Option<(String, i64)> {
    let at = fields.len().checked_sub(count * 12)? + n * 12;
    let to = pointer(&mut Reader::new(fields.get(at..)?)).ok()?;
    (to.path != 0).then(|| Some((panel.file_of(to.file)?, to.path))).flatten()
}

impl SkillsScreen {
    /// Reads it from the scene file that holds the inventory screen.
    pub fn load(install: &Install, file: &str) -> Result<Self> {
        let scene = Scene::load(install, file, true)?;
        let top = (0..scene.nodes.len()).find(|&node| scene.path(node).ends_with(PAGE)).with_context(|| format!("{file} has no skills page"))?;
        let panel = ScenePanel { file: file.to_owned(), ..ScenePanel::of(&scene, top) };
        let find = |path: &str| panel.find(path).with_context(|| format!("the skills page has no {path}"));
        let detailed_at = find(&format!("{SKILL_LIST}/Detailed Skill Panel"))?;
        let thumb_at = find(&format!("{SKILL_GRID}/Skill Icon"))?;
        let detailed = panel.subtree(detailed_at);
        let thumb = panel.subtree(thumb_at);
        let list_grid = panel.find(SKILL_LIST).and_then(|list| Some(CellGrid { grid: panel.grid(list)?, flexible: panel.script(list, "FlexibleGridLayoutGroup").is_some() }));
        let thumb_grid = panel.find(SKILL_GRID).and_then(|grid| Some(CellGrid { grid: panel.grid(grid)?, flexible: panel.script(grid, "FlexibleGridLayoutGroup").is_some() }));
        // `SkillPanel`'s fifteen pointers: the fill's two pictures are the
        // eleventh and twelfth.
        let fill_sprites = match detailed.script(0, "SkillPanel") {
            Some(fields) => [last_pointer(&detailed, fields, 15, 10), last_pointer(&detailed, fields, 15, 11)],
            None => [None, None],
        };
        // `BuffIcon`'s four: the grounds are the last two.
        let buff_grounds = match detailed.find("Buffs/Buff Icon").and_then(|icon| detailed.script(icon, "BuffIcon")) {
            Some(fields) => [last_pointer(&detailed, fields, 4, 2), last_pointer(&detailed, fields, 4, 3)],
            None => [None, None],
        };
        let [filter, sort] = SKILL_DROPDOWNS;
        let dropdown = panel.find(sort).and_then(|node| panel.script(node, "DropDownBox"));
        let tail = |fields: &[u8], back: usize| fields.len().checked_sub(back).and_then(|at| fields.get(at..at + 4)).map(|b| f32::from_le_bytes(b.try_into().expect("four")));
        let dropdown_most_seen = dropdown.and_then(|fields| tail(fields, 4));
        let dropdown_highlight = dropdown.and_then(|fields| {
            let at = 4 + 4 * 12 + 16;
            Some([tail(fields, at)?, tail(fields, at - 4)?, tail(fields, at - 8)?, tail(fields, at - 12)?])
        });
        let template = panel.find(&format!("{sort}/Element"));
        let line_tints = template.and_then(|line| panel.script(line, "Button")).and_then(selectable).and_then(|(tints, _)| tints);
        let line = template.map(|line| panel.subtree(line));
        let list_scroll = panel.find(&format!("{filter}/Open Panel/Scroll Area")).and_then(|area| Scrolling::of(&panel, area, "ScrollRectNoDrag"));
        let view_scroll = panel.find(SKILL_SCROLL).and_then(|view| Scrolling::of(&panel, view, "ScrollRectNoDrag"));
        let localized = (0..panel.nodes.len()).filter_map(|index| localized(&panel, index).map(|(key, case)| (index, key, case))).collect();
        Ok(Self { panel, detailed, thumb, list_grid, thumb_grid, fill_sprites, buff_grounds, line, line_tints, dropdown_highlight, dropdown_most_seen, list_scroll, view_scroll, localized })
    }
}

/// The game's skill tooltip (`ItemUiContext.SkillTooltip`: `Preloader UI/
/// UIContext/Skill Tooltip` in the preloader's scene), with all under it;
/// its three fills' pictures (`_baseFillSprite`, `_normalFillSprite`,
/// `_eliteFillSprite`) and a buff's line (`_buffPanelTemplate`) with its
/// icon's two grounds.
#[derive(Debug, Clone, Default)]
pub struct SkillTooltip {
    pub panel: ScenePanel,
    pub fill_sprites: [Option<(String, i64)>; 3],
    pub buff_line: Option<ScenePanel>,
    pub buff_grounds: [Option<(String, i64)>; 2],
}

pub fn skill_tooltip(install: &Install) -> Result<SkillTooltip> {
    const TOOLTIP: &str = "/UIContext/Skill Tooltip";
    let index = scene_paths(install)?.iter().position(|path| path.ends_with("UI/PreloaderUIScene.unity")).context("the build has no preloader's scene")?;
    let file = format!("level{index}");
    let scene = Scene::load(install, &file, true)?;
    let top = (0..scene.nodes.len()).find(|&node| scene.path(node).ends_with(TOOLTIP)).with_context(|| format!("{file} has no skill tooltip"))?;
    let panel = ScenePanel { file, ..ScenePanel::of(&scene, top) };
    // `SkillTooltip`'s eighteen pointers: the fills' three pictures are the
    // eleventh to the thirteenth.
    let fill_sprites = match panel.script(0, "SkillTooltip") {
        Some(fields) => [10, 11, 12].map(|n| last_pointer(&panel, fields, 18, n)),
        None => [None, None, None],
    };
    let line_at = panel.find("Skill Tooltip/Buff Container/Buff Panel");
    let buff_line = line_at.map(|line| panel.subtree(line));
    let buff_grounds = match line_at.and_then(|line| panel.find(&format!("{}/Buff Icon", panel.nodes[line].path))).and_then(|icon| panel.script(icon, "BuffIcon")) {
        Some(fields) => [last_pointer(&panel, fields, 4, 2), last_pointer(&panel, fields, 4, 3)],
        None => [None, None],
    };
    Ok(SkillTooltip { panel, fill_sprites, buff_line, buff_grounds })
}

/// One of the Resource `Static Icons`' tables (the game's enum
/// dictionaries: a list of key names, then as many pointers), the one whose
/// keys have all of `must`: each key and its sprite's file and number.
fn static_table(install: &Install, must: &[&str]) -> Result<Vec<(String, (String, i64))>> {
    let paths = resource_paths(install)?;
    let (_, file, id) = paths.iter().find(|(path, ..)| path == "static icons").context("no resource is at static icons")?;
    let (_, bytes) = object_bytes(install, file, *id)?;
    let table_at = |start: usize| -> Option<Vec<(String, (String, i64))>> {
        let mut r = Reader::new(bytes.get(start..)?);
        let count = r.u32().ok()? as usize;
        if !(2..=600).contains(&count) {
            return None;
        }
        let mut keys = Vec::with_capacity(count);
        for _ in 0..count {
            let len = r.u32().ok()? as usize;
            if !(1..=64).contains(&len) {
                return None;
            }
            let key = std::str::from_utf8(r.bytes(len).ok()?).ok()?;
            if !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                return None;
            }
            keys.push(key.to_owned());
            r.align(4);
        }
        if !must.iter().all(|wanted| keys.iter().any(|key| key == wanted)) || r.u32().ok()? as usize != count {
            return None;
        }
        let mut icons = Vec::new();
        for key in keys {
            let to = pointer(&mut r).ok()?;
            if to.file == 0 && to.path != 0 {
                icons.push((key, (file.clone(), to.path)));
            }
        }
        Some(icons)
    };
    (0..bytes.len()).step_by(4).find_map(table_at).with_context(|| format!("the static icons have no table with {must:?}"))
}

/// The skills' icons by their names (`StaticIcons.SkillIdSprites`, by
/// `ESkillId`).
pub fn skill_icons(install: &Install) -> Result<Vec<(String, (String, i64))>> {
    static_table(install, &["Endurance", "Strength", "MagDrills", "CovertMovement"])
}

/// The buffs' icons by their names (`StaticIcons.BuffIdSprites`, by
/// `EBuffId`).
pub fn buff_icons(install: &Install) -> Result<Vec<(String, (String, i64))>> {
    static_table(install, &["EnduranceBuffEnduranceInc", "StrengthBuffLiftWeightInc", "CovertMovementSoundVolume"])
}
