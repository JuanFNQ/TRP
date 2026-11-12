//! Tooltips, context menus, small dialogs and the split dialog's look.

use super::*;

/// The game's simple tooltip (`ItemUiContext.Tooltip`, a `SimpleTooltip`:
/// `Preloader UI/UIContext/SimpleTooltip` in the preloader's scene), with
/// all under it: its ground, frame, frame's shadow and text.
pub fn simple_tooltip(install: &Install) -> Result<ScenePanel> {
    const TOOLTIP: &str = "/UIContext/SimpleTooltip";
    let index = scene_paths(install)?.iter().position(|path| path.ends_with("UI/PreloaderUIScene.unity")).context("the build has no preloader's scene")?;
    let file = format!("level{index}");
    let scene = Scene::load(install, &file, true)?;
    let tooltip = (0..scene.nodes.len()).find(|&node| scene.path(node).ends_with(TOOLTIP)).with_context(|| format!("{file} has no simple tooltip"))?;
    Ok(ScenePanel { file, ..ScenePanel::of(&scene, tooltip) })
}

/// The game's tooltip of lines (`ItemUiContext.MultiLineTooltip`:
/// `Preloader UI/UIContext/MultiLineTooltip` in the preloader's scene),
/// with all under it: its ground, frame, frame's shadow, header and the
/// template of a line (`MultiLineRow`).
pub fn multi_line_tooltip(install: &Install) -> Result<ScenePanel> {
    const TOOLTIP: &str = "/UIContext/MultiLineTooltip";
    let index = scene_paths(install)?.iter().position(|path| path.ends_with("UI/PreloaderUIScene.unity")).context("the build has no preloader's scene")?;
    let file = format!("level{index}");
    let scene = Scene::load(install, &file, true)?;
    let tooltip = (0..scene.nodes.len()).find(|&node| scene.path(node).ends_with(TOOLTIP)).with_context(|| format!("{file} has no tooltip of lines"))?;
    Ok(ScenePanel { file, ..ScenePanel::of(&scene, tooltip) })
}

/// The game's menu of a right click on a thing (`ItemUiContext.ContextMenu`,
/// a `SimpleContextMenu`: `Preloader UI/UIContext/Context Menu Area/
/// ItemContextMenu` in the preloader's scene), with all under it: its
/// ground and shadow, and the templates of a line (`Button Template`), of
/// the wish list's (`Wishlist Template`) and of Discard's (`Dispose
/// Template`).
pub fn context_menu(install: &Install) -> Result<ScenePanel> {
    const MENU: &str = "/UIContext/Context Menu Area/ItemContextMenu";
    let index = scene_paths(install)?.iter().position(|path| path.ends_with("UI/PreloaderUIScene.unity")).context("the build has no preloader's scene")?;
    let file = format!("level{index}");
    let scene = Scene::load(install, &file, true)?;
    let menu = (0..scene.nodes.len()).find(|&node| scene.path(node).ends_with(MENU)).with_context(|| format!("{file} has no item's menu"))?;
    Ok(ScenePanel { file, ..ScenePanel::of(&scene, menu) })
}

/// The load component a magazine's view shows while it is worked
/// (`ItemViewLoadAmmoComponent`): the Resource the item views' animation
/// makes it from (`ItemViewAnimation._loadAmmoComponentTemplate`).
pub fn load_ammo_component(install: &Install) -> Result<ScenePanel> {
    const WANTED: &str = "prefabs/ugui/layouts/itemviewcomponents/itemviewloadammocomponent";
    let paths = resource_paths(install)?;
    let (_, file, id) = paths.iter().find(|(path, ..)| path == WANTED).with_context(|| format!("no resource is at {WANTED}"))?;
    let scene = Scene::load_prefab(install, file, *id)?;
    Ok(ScenePanel { file: file.clone(), ..ScenePanel::of(&scene, 0) })
}

/// The examination's view over a thing's view while it is examined
/// (`ItemViewExamineComponent`): the Resource the item views' animation
/// makes it from (`ItemViewAnimation._examineComponentTemplate`).
pub fn examine_component(install: &Install) -> Result<ScenePanel> {
    const WANTED: &str = "prefabs/ugui/layouts/itemviewcomponents/itemviewexaminationcomponent";
    let paths = resource_paths(install)?;
    let (_, file, id) = paths.iter().find(|(path, ..)| path == WANTED).with_context(|| format!("no resource is at {WANTED}"))?;
    let scene = Scene::load_prefab(install, file, *id)?;
    Ok(ScenePanel { file: file.clone(), ..ScenePanel::of(&scene, 0) })
}

/// The game scene's panel of a thing being used or examined (`UsingPanel`:
/// `Game Scene/UsingPanel` in the game's own interface scene,
/// `GameUIScene`), with all under it, and what its script names: its
/// caption and the thing's name (`_caption`, `_itemName`), its bar's fill
/// (`_progressBarImage`), and the fill's colours for a use and for an
/// examine (`_defaultColor`, `_examineColor`).
#[derive(Debug, Clone)]
pub struct UsingLook {
    pub panel: ScenePanel,
    pub caption: Option<usize>,
    pub name: Option<usize>,
    pub fill: Option<usize>,
    pub default_color: [f32; 4],
    pub examine_color: [f32; 4],
}

pub fn using_panel(install: &Install) -> Result<UsingLook> {
    const PANEL: &str = "Game Scene/UsingPanel";
    let index = scene_paths(install)?.iter().position(|path| path.ends_with("UI/GameUIScene.unity")).context("the build has no game interface's scene")?;
    let file = format!("level{index}");
    let scene = Scene::load(install, &file, true)?;
    let top = (0..scene.nodes.len()).find(|&node| scene.path(node).ends_with(PANEL)).with_context(|| format!("{file} has no using panel"))?;
    let panel = ScenePanel { file, ..ScenePanel::of(&scene, top) };
    let fields = panel.script(0, "UsingPanel").context("the using panel's script")?;
    // Odin's data (`UIElement`), then its three pointers and two colours.
    let mut r = Reader::new(fields);
    skip_odin(&mut r)?;
    let at = fields.len() - r.remaining();
    let [caption, name, fill] = [0, 1, 2].map(|n| panel.pointer_at(fields, at + 12 * n));
    r.skip(36)?;
    let mut colour = || -> Result<[f32; 4]> { Ok([r.f32()?, r.f32()?, r.f32()?, r.f32()?]) };
    let (default_color, examine_color) = (colour()?, colour()?);
    Ok(UsingLook { panel, caption, name, fill, default_color, examine_color })
}

/// The menu that holds a line's own lines (`InteractionButtonsContainer.
/// SubMenuTemplate`, a `SimpleContextMenu`): the prefab the item's menu's
/// holder of lines points to in the scene's shared file, told from its
/// other pointers by having a holder of lines of its own. Laid out as the
/// item's menu is, from the same templates.
pub fn context_submenu(install: &Install) -> Result<ScenePanel> {
    let menu = context_menu(install)?;
    let holder = menu.find("InteractionButtonsContainer").context("the item's menu has no holder of lines")?;
    let fields = menu.script(holder, "InteractionButtonsContainer").context("the holder of lines' script")?;
    for start in (0..fields.len().saturating_sub(11)).step_by(4) {
        let to = pointer(&mut Reader::new(&fields[start..]))?;
        if to.file < 1 || to.path <= 0 || to.path >= 1 << 40 {
            continue;
        }
        let Some(shared) = menu.file_of(to.file).filter(|name| name.starts_with("sharedassets")) else { continue };
        let Ok(prefab) = Scene::load_prefab_of_script(install, &shared, to.path) else { continue };
        let panel = ScenePanel { file: shared.clone(), ..ScenePanel::of(&prefab, 0) };
        if panel.find("InteractionButtonsContainer").is_some() {
            return Ok(panel);
        }
    }
    anyhow::bail!("the item's menu points to no menu of lines of its own")
}

/// The game's window that shows a thing (`InfoWindow`, with its
/// `ItemSpecificationPanel` and `ItemInfoWindowLabels`): the prefab the
/// preloader's item context makes it from (`ItemUiContext._infoWindowTemplate`,
/// in the scene's shared file), found among the prefabs its script points
/// to by its own script.
pub fn info_window(install: &Install) -> Result<ScenePanel> {
    context_window(install, "InfoWindow").context("the item context points to no window that shows a thing")
}

/// The game's repair window (`RepairWindow`; workbench spec, "The repair
/// window"): its prefab (`ItemUiContext._repairWindowTemplate`), found as
/// the other windows of the item context are.
pub fn repair_window(install: &Install) -> Result<ScenePanel> {
    context_window(install, "RepairWindow").context("the item context points to no repair window")
}

/// The game's window that asks how many (`SplitDialog`, with its stepped
/// slider for a stack and its whole-number slider for food or drink;
/// inventory spec, "Part of a stack"): its prefab
/// (`ItemUiContext._splitDialogTemplate`), found as the other windows of
/// the item context are.
pub fn split_dialog(install: &Install) -> Result<ScenePanel> {
    context_window(install, "SplitDialog").context("the item context points to no window that asks how many")
}

/// The window that asks how many, with what its scripts point to: the
/// window that is moved to the mouse (`_window`), its caption bar and
/// title, its cross and its button, each button's tints and what it tints,
/// and its two sliders, the stepped one for a stack (`StepSlider`) and the
/// whole-number one for food or drink (`IntSlider`).
#[derive(Debug, Clone)]
pub struct SplitLook {
    pub panel: ScenePanel,
    pub window: usize,
    pub caption: usize,
    pub title: usize,
    pub close: SplitButton,
    pub accept: SplitButton,
    pub step: SplitSlider,
    pub int: SplitSlider,
}

/// A button of the window: its object, its tints (`Selectable.colors`, the
/// transition the tint) and the object whose picture they are laid on.
#[derive(Debug, Clone, Copy)]
pub struct SplitButton {
    pub object: usize,
    pub tints: Option<Tints>,
    pub tinted: Option<usize>,
}

/// One of the window's sliders: its object (the `Slider`, with its tints on
/// its handle), the handle the slider moves (`m_HandleRect`, laid at the
/// share of the value along the slider, `Slider.UpdateVisuals`), its field
/// (`_valueInput`, a `TMP_InputField`) and the field's text, the notch the
/// script copies for each step and, for the whole-number slider, the one
/// it copies for a place the knob may not stand
/// (`_notchNotAvailableTemplate`), and the row they are copied into
/// (`_notchContainer`). The field's selection colour, its caret's width in
/// units, its blink rate a second, its caret's own colour where it has
/// one (else the text's), and whether it selects all it has when it takes
/// the keys (`m_OnFocusSelectAll`).
#[derive(Debug, Clone, Copy)]
pub struct SplitSlider {
    pub object: usize,
    pub tints: Option<Tints>,
    pub tinted: Option<usize>,
    pub handle: usize,
    pub input: usize,
    pub text: usize,
    pub notch: usize,
    pub not_available: Option<usize>,
    pub container: usize,
    pub selection: [f32; 4],
    pub caret_width: f32,
    pub blink_rate: f32,
    pub caret_color: Option<[f32; 4]>,
    pub select_all: bool,
}

impl SplitLook {
    /// The window as its script leaves it (`SplitDialog.Show`): the
    /// whole-number slider shown or the stepped one, the other off; a notch
    /// copied into the shown one's row for each place but the last, from
    /// the template for a place the knob may stand or, where `false`, the
    /// other (`StepSlider.CreateNotch`, `IntSlider.Show`); its handle laid
    /// at that share of the slider (`Slider.UpdateVisuals`: its anchors
    /// across at the share, up and down whole); and the window's pivot at
    /// that place, in units from the screen's bottom left
    /// (`_window.position`).
    pub fn laid_out(&self, int: bool, notches: &[bool], share: f32, pivot: [f32; 2]) -> ScenePanel {
        let mut panel = self.panel.clone();
        let (shown, other) = if int { (self.int, self.step) } else { (self.step, self.int) };
        panel.nodes[shown.object].active = true;
        panel.nodes[other.object].active = false;
        for &available in notches {
            let template = if available { shown.notch } else { shown.not_available.unwrap_or(shown.notch) };
            panel.copy_into(template, shown.container);
        }
        let handle = &mut panel.nodes[shown.handle].layout;
        (handle.anchor_min, handle.anchor_max) = ([share, 0.0], [share, 1.0]);
        panel.nodes[self.window].layout.offset = pivot;
        panel
    }

    /// The slider of that kind.
    pub fn slider(&self, int: bool) -> &SplitSlider {
        if int {
            &self.int
        } else {
            &self.step
        }
    }
}

pub fn split_look(install: &Install) -> Result<SplitLook> {
    let panel = split_dialog(install)?;
    // `SplitDialog`'s seven references, in its fields' order.
    let fields = panel.script(0, "SplitDialog").context("the window's script")?;
    let at = |n: usize| panel.pointer_at(fields, n * 12);
    let window = panel.transform_at(fields, 0).context("the window's frame")?;
    let caption = at(1).context("the window's caption bar")?;
    let button = |object: usize| -> Result<SplitButton> {
        let fields = panel.script(object, "Button").context("a button's script")?;
        let (tints, end) = selectable(fields).context("a button's tints")?;
        Ok(SplitButton { object, tints, tinted: end.checked_sub(12).and_then(|before| panel.pointer_at(fields, before)) })
    };
    let close = button(at(2).context("the window's cross")?)?;
    let accept = button(at(3).context("the window's button")?)?;
    let step = split_slider(&panel, at(4).context("the stepped slider")?, "StepSlider")?;
    let int = split_slider(&panel, at(5).context("the whole-number slider")?, "IntSlider")?;
    let title = at(6).context("the window's title")?;
    Ok(SplitLook { window, caption, title, close, accept, step, int, panel })
}

/// One slider of the window that asks how many, by its script: the
/// stepped one's four references (`_slider`, `_valueInput`,
/// `_notchTemplate`, `_notchContainer`) or the whole-number one's five (the
/// template for a place not available before the row).
fn split_slider(panel: &ScenePanel, object: usize, class: &str) -> Result<SplitSlider> {
    let fields = panel.script(object, class).with_context(|| format!("its {class}"))?;
    let int = class == "IntSlider";
    let slider = panel.pointer_at(fields, 0).context("its slider")?;
    let input = panel.pointer_at(fields, 12).context("its field")?;
    let notch = panel.pointer_at(fields, 24).context("its notch")?;
    let not_available = if int { panel.pointer_at(fields, 36) } else { None };
    let container = panel.transform_at(fields, if int { 48 } else { 36 }).context("the row of its notches")?;
    // The slider: what every control has, then what it fills (none here),
    // its handle, its direction, its least and most, whether it keeps to
    // whole numbers and its value.
    let fields = panel.script(slider, "Slider").context("its slider's script")?;
    let (tints, end) = selectable(fields).context("its slider's tints")?;
    let tinted = end.checked_sub(12).and_then(|before| panel.pointer_at(fields, before));
    let handle = panel.transform_at(fields, end + 12).context("its handle")?;
    let InputField { text, selection, caret_width, blink_rate, caret_color, select_all, .. } = input_field(panel, input, "CustomTextMeshProInputField")?;
    Ok(SplitSlider { object: slider, tints, tinted, handle, input, text, notch, not_available, container, selection, caret_width, blink_rate, caret_color, select_all })
}

/// A text field as TextMeshPro's `TMP_InputField` keeps it: the object of
/// its text, its selection's colour, its caret's width in units, its blink
/// rate a second, its caret's own colour where it has one (else the
/// text's), whether it selects all it has when it takes the keys
/// (`m_OnFocusSelectAll`), and the most letters it takes
/// (`m_CharacterLimit`; any number where 0).
#[derive(Debug, Clone, Copy)]
pub struct InputField {
    pub text: usize,
    pub selection: [f32; 4],
    pub caret_width: f32,
    pub blink_rate: f32,
    pub caret_color: Option<[f32; 4]>,
    pub select_all: bool,
    pub limit: u32,
}

/// The field on an object of a panel, by its script's class (the engine's
/// own or one of the game's that adds no fields before them).
pub fn input_field(panel: &ScenePanel, input: usize, class: &str) -> Result<InputField> {
    // The field: what every control has, its six references (what shows
    // it, its text, its placeholder, a scroll bar and its handler, a
    // layout), its settings to the expression a figure is checked by, its
    // size and limit, its eight events (empty here: one with calls is not
    // read), its caret's colour and whether that is its own, its
    // selection's colour, its text, the caret's blink rate and width, and
    // after more settings whether it selects all as it takes the keys.
    let fields = panel.script(input, class).context("its field's script")?;
    let (_, end) = selectable(fields).context("its field's tints")?;
    let text = panel.pointer_at(fields, end + 12).context("its field's text")?;
    let mut r = Reader::new(fields.get(end + 72..).context("its field's settings")?);
    r.skip(4 * 9)?;
    string(&mut r)?;
    r.skip(4)?;
    let limit = r.i32()?.max(0) as u32;
    for _ in 0..8 {
        if r.u32()? != 0 {
            bail!("an event of its field has calls, which are not read");
        }
    }
    let caret = color(&mut r)?;
    let own_caret = flag(&mut r)?;
    let selection = color(&mut r)?;
    string(&mut r)?;
    let blink_rate = r.f32()?;
    let caret_width = r.i32()? as f32;
    // Read-only, rich text, the font asset, then the selection on focus.
    r.skip(4 + 4 + 12)?;
    let select_all = flag(&mut r)?;
    Ok(InputField { text, selection, caret_width, blink_rate, caret_color: own_caret.then_some(caret), select_all, limit })
}
