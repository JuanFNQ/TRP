//! The tab bar and its parts.

use super::*;

/// The inventory screen's bar of tabs (`Tab Bar`) as its scene has it: the
/// bar, the row of tabs and how far apart it puts them (`TabSizeController.
/// _gap`, less than none: they overlap), and each tab with what its scripts
/// name.
#[derive(Debug, Clone, Default)]
pub struct TabBar {
    pub panel: ScenePanel,
    pub row: Option<usize>,
    pub gap: f32,
    pub tabs: Vec<TabParts>,
    /// The screen's button at the bar's right end that shuts it
    /// (`BackButton`, a `DefaultUIButton`), with all under it.
    pub back: Option<ScenePanel>,
    /// The icon of the app's own tab, the options, beside its word as the
    /// other tabs have theirs: the single cog beside the words of the game's
    /// GAME SETTINGS button on its raid screen (`Matchmaker Offline Raid
    /// Screen/Content/NonLayoutContainer/SettingsButton`, `IconIdle` and
    /// `Icon`), white at rest and black lit (`icon_gear`, `icon_gear_black`),
    /// from the menu scene's own file; each its file and number.
    pub settings_icons: [Option<(String, i64)>; 2],
}

/// One tab of a bar (`Tab`, `TabElement`, `LocalizedText`): its object;
/// its place in the row (`OrderNumber`), what its width is taken from
/// (`_sizeDefiners`, the widest of them that shows) and what is added to
/// that (`_widthAdjustment`); its versions idle and chosen
/// (`_normalVersion`, `_selectedVersion`) and what shows with the mouse on
/// it and off it (`_hoverGraphic`, `_idleGraphic`); and its words, the key
/// of the install's words with the case they are put in (`EStringCase`:
/// none, lower, upper, title) and the texts that show them (`_labels`).
#[derive(Debug, Clone, Default)]
pub struct TabParts {
    pub object: usize,
    pub order: i32,
    pub size_definers: Vec<usize>,
    pub width_adjustment: f32,
    pub normal: Option<usize>,
    pub selected: Option<usize>,
    pub hover: Option<usize>,
    pub idle: Option<usize>,
    pub key: String,
    pub case: u32,
    pub labels: Vec<usize>,
    /// What a tab that does not work is dimmed by (`SetInteractable`): the
    /// object of its group whose alpha goes to 0.3 (`_canvasGroup`,
    /// `SetUnlockStatus`) and its picture whose alpha goes to 0.15
    /// (`_targetImage`).
    pub canvas_group: Option<usize>,
    pub target: Option<usize>,
}

impl TabBar {
    /// Reads it from the scene that holds the inventory screen.
    pub fn load(install: &Install, file: &str) -> Result<Self> {
        const BAR: &str = "/InventoryScreen/Tab Bar";
        let scene = Scene::load(install, file, true)?;
        let top = (0..scene.nodes.len()).find(|&node| scene.path(node).ends_with(BAR)).with_context(|| format!("{file} has no bar of tabs"))?;
        let panel = ScenePanel { file: file.to_owned(), ..ScenePanel::of(&scene, top) };
        let row = (0..panel.nodes.len()).find(|&index| panel.script(index, "TabSizeController").is_some());
        let gap = row.and_then(|row| panel.script(row, "TabSizeController")).and_then(|fields| {
            let mut r = Reader::new(fields);
            skip_odin(&mut r).ok()?;
            r.f32().ok()
        });
        let mut tabs = Vec::new();
        for object in row.map_or_else(Vec::new, |row| (0..panel.nodes.len()).filter(|&index| panel.nodes[index].parent == Some(row)).collect()) {
            let mut tab = TabParts { object, ..TabParts::default() };
            if let Some(fields) = panel.script(object, "TabElement") {
                let mut r = Reader::new(fields);
                let read = |r: &mut Reader, tab: &mut TabParts| -> Result<()> {
                    skip_odin(r)?;
                    let count = r.u32()? as usize;
                    let at = fields.len() - r.remaining();
                    tab.size_definers = (0..count).filter_map(|n| panel.transform_at(fields, at + 12 * n)).collect();
                    r.skip(12 * count)?;
                    tab.width_adjustment = r.f32()?;
                    tab.order = r.i32()?;
                    Ok(())
                };
                read(&mut r, &mut tab).with_context(|| format!("the tab {}'s element", panel.nodes[object].path))?;
            }
            if let Some(fields) = panel.script(object, "Tab") {
                tab.normal = panel.pointer_at(fields, 0);
                tab.selected = panel.pointer_at(fields, 12);
                tab.canvas_group = panel.pointer_at(fields, 24);
                tab.target = panel.pointer_at(fields, 36);
                tab.hover = panel.pointer_at(fields, 60);
                tab.idle = panel.pointer_at(fields, 72);
            }
            if let Some(fields) = panel.script(object, "LocalizedText") {
                let mut r = Reader::new(fields);
                let read = |r: &mut Reader, tab: &mut TabParts| -> Result<()> {
                    skip_odin(r)?;
                    tab.key = string(r)?;
                    let count = r.u32()? as usize;
                    let at = fields.len() - r.remaining();
                    tab.labels = (0..count).filter_map(|n| panel.pointer_at(fields, at + 12 * n)).collect();
                    r.skip(12 * count)?;
                    tab.case = r.u32()?;
                    Ok(())
                };
                read(&mut r, &mut tab).with_context(|| format!("the tab {}'s words", panel.nodes[object].path))?;
            }
            tabs.push(tab);
        }
        tabs.sort_by_key(|tab| tab.order);
        const BACK: &str = "/InventoryScreen/BackButton";
        let back = (0..scene.nodes.len()).find(|&node| scene.path(node).ends_with(BACK)).map(|node| ScenePanel { file: file.to_owned(), ..ScenePanel::of(&scene, node) });
        // The menu scene's pictures are in its own shared file (a scene
        // `levelN`'s is `sharedassetsN.assets`), which is small.
        let menu = scene_paths(install).ok().and_then(|paths| paths.iter().position(|path| path.ends_with("UI/MenuUIScene.unity")));
        let settings_icons = menu.map_or([None, None], |index| sprites_named(install, &format!("sharedassets{index}.assets"), ["icon_gear", "icon_gear_black"]));
        Ok(Self { panel, row, gap: gap.unwrap_or(0.0), tabs, back, settings_icons })
    }

    /// A tab's words: what the install's words have for its key (`say`),
    /// else the key, in the case its `LocalizedText` puts them.
    pub fn words(tab: &TabParts, say: &dyn Fn(&str) -> Option<String>) -> String {
        in_case(say(&tab.key).unwrap_or_else(|| tab.key.clone()), tab.case)
    }

    /// How wide every tab is (`TabSizeController.UpdateLayout`): of each
    /// tab, the widest of what its width is taken from (`TabElement.
    /// GetWidth`: each a text sized to its words, as TextMeshPro lays them
    /// out in its font, `TmpFont::width`, with its margins) and its
    /// adjustment; the widest of the tabs. A text drawn bold is laid out in
    /// its font's bold typeface where it has one, as TextMeshPro takes it.
    pub fn width(&self, install: &Install, say: &dyn Fn(&str) -> Option<String>) -> f32 {
        let mut fonts: HashMap<(String, i64), Option<TmpFont>> = HashMap::new();
        let mut font = |file: &str, id: i64| fonts.entry((file.to_owned(), id)).or_insert_with(|| TmpFont::read(install, file, id).ok()).clone();
        let mut widest = 0.0f32;
        for tab in &self.tabs {
            let words = Self::words(tab, say);
            let mut own = 0.0f32;
            for &definer in &tab.size_definers {
                let Some(text) = &self.panel.nodes[definer].text else { continue };
                let Some(file) = self.panel.file_of(text.font.0).or_else(|| (text.font.0 == 0).then(|| self.panel.file.clone())) else { continue };
                let shown = if text.style & 16 != 0 { words.to_uppercase() } else if text.style & 8 != 0 { words.to_lowercase() } else { words.clone() };
                let bold = text.style & 1 != 0;
                let Some(regular) = font(&file, text.font.1) else { continue };
                let laid = match regular.bold.clone().filter(|_| bold).and_then(|(file, id)| font(&file, id)) {
                    Some(alternative) => alternative.width(&shown, text.size, text.spacing, false),
                    None => regular.width(&shown, text.size, text.spacing, bold),
                };
                own = own.max(laid + text.margin[0] + text.margin[2]);
            }
            widest = widest.max(own + tab.width_adjustment);
        }
        widest
    }

    /// Lays the tabs out as the row does once their width is known
    /// (`TabSizeController.UpdateLayout`, `TabElement.SetWidth`): each as
    /// wide as the widest asks, its pivot `(width + gap) * OrderNumber`
    /// from the row's pivot (`localPosition`), here as its offset from its
    /// anchor in the row.
    pub fn lay_out(&mut self, width: f32) {
        let Some(row) = self.row else { return };
        let row_layout = self.panel.nodes[row].layout;
        for tab in &self.tabs {
            let layout = &mut self.panel.nodes[tab.object].layout;
            let anchor = (layout.anchor_min[0] - row_layout.pivot[0]) * row_layout.size_delta[0];
            layout.size_delta[0] = width;
            layout.offset[0] = (width + self.gap) * tab.order as f32 - anchor;
        }
    }
}
