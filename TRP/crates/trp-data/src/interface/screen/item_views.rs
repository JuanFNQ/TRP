//! The item and container views, scrolling, scroll bars, tints and sliders.

use super::*;

/// The game's views of a thing in a slot and in a grid (`SlotItemView`,
/// `GridItemView`): their prefabs among the build's Resources, as
/// `ItemViewFactory.CreateFromPool` names them (`Prefabs/UGUI/Layouts/`
/// `slot_layout`, `grid_layout`), read as panels.
#[derive(Debug, Clone, Default)]
pub struct ItemViews {
    pub slot: Option<ScenePanel>,
    pub grid: Option<ScenePanel>,
    /// A quick-use view's thing (`QuickSlotItemView`, `quick_slot_layout`):
    /// its picture's place and its figure (`Value`); none where not read.
    pub quick: Option<ScenePanel>,
}

impl ItemViews {
    pub fn load(install: &Install) -> Result<Self> {
        let paths = resource_paths(install)?;
        let read = |name: &str| -> Result<ScenePanel> {
            let wanted = format!("prefabs/ugui/layouts/{name}");
            let (_, file, id) = paths.iter().find(|(path, ..)| *path == wanted).with_context(|| format!("no resource is at {wanted}"))?;
            let scene = Scene::load_prefab(install, file, *id)?;
            Ok(ScenePanel { file: file.clone(), ..ScenePanel::of(&scene, 0) })
        };
        Ok(Self { slot: Some(read("slot_layout")?), grid: Some(read("grid_layout")?), quick: read("quick_slot_layout").ok() })
    }
}

/// The inventory's containers' column as the game draws it
/// (`ContainersPanel`): the scene's panel with its row template (`Slot
/// Template`: a container slot's header and its panel, the grids to its
/// right), the view of one grid behind the default row of grids
/// (`GeneratedGridsView`, the template's `_containedGridsTemplate`, and its
/// own `_gridViewTemplate`), and that row's layout.
#[derive(Debug, Clone, Default)]
pub struct ContainerViews {
    pub panel: ScenePanel,
    /// The row template, its column (`Content`: rows so far apart, the
    /// first so far down), its header and its slot panel.
    pub template: usize,
    pub column: Option<RowGroup>,
    pub header: Option<usize>,
    pub slot_panel: Option<usize>,
    /// A grid's view (`GridView`) as a panel of its own, and the default
    /// row the grids of a container are laid in.
    pub grid: Option<ScenePanel>,
    pub grids_row: Option<RowGroup>,
    /// How the column scrolls, and its bar.
    pub scrolling: Option<Scrolling>,
}

/// A scroll view of a screen (`ScrollRect`, here `ScrollRectNoDrag`) as
/// its scene has it: its objects, how far a notch of the wheel moves it
/// and how it stops, and its bar.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Scrolling {
    /// What is seen of it (`m_Viewport`, else itself) and what moves
    /// (`m_Content`), objects of the panel.
    pub view: Option<usize>,
    pub content: Option<usize>,
    /// Whether it scrolls up and down, and across.
    pub vertical: bool,
    pub across: bool,
    /// Units a notch of the wheel moves it (`m_ScrollSensitivity`).
    pub sensitivity: f32,
    /// Whether it stops at its ends (`MovementType.Clamped`), or springs
    /// back from past them (`Elastic`) over so long (`m_Elasticity`, s);
    /// whether it runs on after a drag (`m_Inertia`) and how fast that
    /// slows (`m_DecelerationRate`).
    pub clamped: bool,
    pub elastic: bool,
    pub elasticity: f32,
    pub inertia: bool,
    pub deceleration: f32,
    /// Its bar up and down (`m_VerticalScrollbar`), and whether the bar is
    /// hidden while everything shows (`ScrollbarVisibility.AutoHide`).
    pub bar: Option<ScrollBar>,
    pub hides: bool,
}

/// A scroll bar of a screen (`Scrollbar`): its object, the area its
/// handle slides in and the handle (the handle's parent), whether it runs
/// from the bottom up (`Direction.BottomToTop`), and how the handle is
/// tinted as the mouse is on it or holds it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScrollBar {
    pub object: usize,
    pub handle: Option<usize>,
    pub sliding: Option<usize>,
    pub bottom_up: bool,
    pub tints: Option<Tints>,
    /// The object whose picture the tints are laid on (`m_TargetGraphic`).
    pub tinted: Option<usize>,
}

/// How a control of a screen tints what it draws by its state
/// (`Selectable.colors`, a `ColorBlock`, when its transition is the
/// tint): idle, under the mouse, held, chosen and off, each multiplied
/// by the multiplier, and how long a change takes, in seconds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tints {
    pub normal: [f32; 4],
    pub highlighted: [f32; 4],
    pub pressed: [f32; 4],
    pub selected: [f32; 4],
    pub disabled: [f32; 4],
    pub multiplier: f32,
    pub fade: f32,
}

/// What a control's fields begin with (`Selectable`): its navigation, its
/// transition and its tints, its sprites and its animation's triggers,
/// whether it works, and what it tints. Read up to its own class's
/// fields, which follow; the tints only where the transition is the tint.
pub(super) fn selectable(fields: &[u8]) -> Option<(Option<Tints>, usize)> {
    let read = |r: &mut Reader| -> Result<(Option<Tints>, usize)> {
        // Navigation: its mode, whether it wraps, four pointers.
        r.skip(4 + 4 + 4 * 12)?;
        let transition = r.u32()?;
        let (normal, highlighted, pressed, selected, disabled) = (color(r)?, color(r)?, color(r)?, color(r)?, color(r)?);
        let (multiplier, fade) = (r.f32()?, r.f32()?);
        // Its sprites by state, its animation's five triggers, whether it
        // works, and what it tints.
        r.skip(4 * 12)?;
        for _ in 0..5 {
            string(r)?;
        }
        r.skip(4 + 12)?;
        let tints = (transition == 1).then_some(Tints { normal, highlighted, pressed, selected, disabled, multiplier, fade });
        Ok((tints, fields.len() - r.remaining()))
    };
    read(&mut Reader::new(fields)).ok()
}

/// The tints of a control on that object of a panel, by its script of that
/// class (a `Selectable`'s, or a class built on one), where its transition
/// is the tint, and the object whose picture they are laid on
/// (`m_TargetGraphic`).
pub fn control_tints(panel: &ScenePanel, object: usize, class: &str) -> Option<(Tints, Option<usize>)> {
    let fields = panel.script(object, class)?;
    let (tints, end) = selectable(fields)?;
    Some((tints?, end.checked_sub(12).and_then(|before| panel.pointer_at(fields, before))))
}

/// A slider's own settings (`Slider`, after what every control has): what it
/// fills and its handle, its direction (`Slider.Direction`: 0 left to
/// right, 1 right to left, 2 bottom to top, 3 top to bottom), its least and
/// most, and whether it keeps to whole numbers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SliderRange {
    pub fill: Option<usize>,
    pub direction: u32,
    pub min: f32,
    pub max: f32,
    pub whole: bool,
}

impl SliderRange {
    /// The share a value fills (`Slider.normalizedValue`): clamped to its
    /// range, rounded where it keeps to whole numbers.
    pub fn share(&self, value: f32) -> f32 {
        let value = value.clamp(self.min.min(self.max), self.max.max(self.min));
        let value = if self.whole { value.round() } else { value };
        if (self.max - self.min).abs() < f32::EPSILON { 0.0 } else { (value - self.min) / (self.max - self.min) }
    }
}

/// The slider on that object of a panel (`Slider`), where it has one.
pub fn slider_range(panel: &ScenePanel, object: usize) -> Option<SliderRange> {
    let fields = panel.script(object, "Slider")?;
    let (_, end) = selectable(fields)?;
    let fill = panel.transform_at(fields, end);
    let mut r = Reader::new(fields.get(end + 24..)?);
    let direction = r.u32().ok()?;
    let (min, max) = (r.f32().ok()?, r.f32().ok()?);
    let whole = flag(&mut r).ok()?;
    Some(SliderRange { fill, direction, min, max, whole })
}

impl Scrolling {
    /// The scroll view on that object of a panel (its script of that
    /// class, a `ScrollRect` or a class built on one), by the engine's
    /// order of the fields: what moves, whether across and up and down,
    /// how it stops, its elasticity, inertia and deceleration, the
    /// wheel's step, what is seen of it, its two bars, whether each hides,
    /// and their spacing.
    pub fn of(panel: &ScenePanel, object: usize, class: &str) -> Option<Self> {
        let fields = panel.script(object, class)?;
        let mut r = Reader::new(fields);
        r.skip(12).ok()?;
        let (across, vertical) = (flag(&mut r).ok()?, flag(&mut r).ok()?);
        let movement = r.u32().ok()?;
        let (elasticity, inertia, deceleration) = (r.f32().ok()?, flag(&mut r).ok()?, r.f32().ok()?);
        let sensitivity = r.f32().ok()?;
        r.skip(12 + 12 + 12 + 4).ok()?;
        let hides = r.u32().ok()? == 1;
        let bar = panel.pointer_at(fields, 64).and_then(|object| ScrollBar::of(panel, object));
        let (clamped, elastic) = (movement == 2, movement == 1);
        Some(Self { view: panel.transform_at(fields, 40).or(Some(object)), content: panel.transform_at(fields, 0), vertical, across, sensitivity, clamped, elastic, elasticity, inertia, deceleration, bar, hides })
    }
}

impl ScrollBar {
    /// The bar whose script is on that object: after what every control
    /// has (`selectable`), its handle and its direction.
    pub(super) fn of(panel: &ScenePanel, object: usize) -> Option<Self> {
        let fields = panel.script(object, "Scrollbar")?;
        let (tints, at) = selectable(fields)?;
        let handle = panel.transform_at(fields, at);
        let direction = u32::from_le_bytes(fields.get(at + 12..at + 16)?.try_into().ok()?);
        let tinted = at.checked_sub(12).and_then(|before| panel.pointer_at(fields, before));
        Some(Self { object, handle, sliding: handle.and_then(|handle| panel.nodes[handle].parent), bottom_up: direction == 2, tints, tinted })
    }

    /// Where its handle lies in a panel of that size (the panel's top as
    /// large), the bar showing `size` of what it scrolls (a share) and at
    /// `value` (0 at the bottom of a bar that runs from the bottom up, 1 at
    /// its top): the handle's own layout with the anchors the bar gives it
    /// up and down its sliding area (`Scrollbar.UpdateVisuals`). Its
    /// left, top, width and height, y down.
    pub fn handle_in(&self, panel: &ScenePanel, whole: [f32; 2], value: f32, size: f32) -> Option<[f32; 4]> {
        Some(handle_on(panel.rect(self.sliding?, whole), panel.nodes[self.handle?].layout, self.bottom_up, value, size))
    }
}

/// Where a scroll bar's handle lies (`Scrollbar.UpdateVisuals`), from its
/// sliding area's rectangle (left, top, width, height, y down), the
/// handle's own layout, the bar's way, its value and the handle's share.
pub fn handle_on(sliding: [f32; 4], layout: Layout, bottom_up: bool, value: f32, size: f32) -> [f32; 4] {
    let [x, y, w, h] = sliding;
    let moved = value.clamp(0.0, 1.0) * (1.0 - size);
    let (low, high) = if bottom_up { (moved, moved + size) } else { (1.0 - moved - size, 1.0 - moved) };
    let rect = place(Layout { anchor_min: [0.0, low], anchor_max: [1.0, high], scale: [1.0, 1.0], ..layout }, Rect { at: [0.0, 0.0], size: [w, h] });
    [x + rect.at[0], y + h - rect.at[1] - rect.size[1], rect.size[0], rect.size[1]]
}

impl ContainerViews {
    /// Reads it from the scene that holds the inventory screen.
    pub fn load(install: &Install, file: &str) -> Result<Self> {
        const PANEL: &str = "/InventoryScreen/Items Panel/LeftSide/Containers Panel";
        let scene = Scene::load(install, file, true)?;
        let top = (0..scene.nodes.len()).find(|&node| scene.path(node).ends_with(PANEL)).with_context(|| format!("{file} has no containers panel"))?;
        let panel = ScenePanel { file: file.to_owned(), ..ScenePanel::of(&scene, top) };
        let template = (0..panel.nodes.len()).find(|&index| panel.nodes[index].scripts.iter().any(|(class, _)| class == "SearchableSlotView")).context("the containers panel has no row template")?;
        let column = panel.nodes[template].parent.and_then(|content| panel.nodes[content].group);
        let header = panel.find(&format!("{}/Header Panel/SlotViewHeader", panel.nodes[template].path));
        let slot_panel = panel.find(&format!("{}/Slot Panel", panel.nodes[template].path));
        // The template's grids' template, a script in another file, and
        // its grid view's, a script of that file's own.
        let outside = |fields: &[u8]| -> Option<(String, i64)> {
            (0..fields.len().saturating_sub(11)).step_by(4).find_map(|start| {
                let to = pointer(&mut Reader::new(&fields[start..])).ok()?;
                (to.file > 0 && to.path > 0 && to.path < 1 << 40).then(|| Some((panel.file_of(to.file)?, to.path))).flatten()
            })
        };
        let grids = panel.script(template, "SearchableItemView").and_then(outside).and_then(|(file, id)| Some((file.clone(), Scene::load_prefab_of_script(install, &file, id).ok()?)));
        let (mut grid, mut grids_row) = (None, None);
        if let Some((file, scene)) = grids {
            let generated = ScenePanel { file: file.clone(), ..ScenePanel::of(&scene, 0) };
            grids_row = generated.nodes[0].group;
            let template = generated.script(0, "GeneratedGridsView").and_then(|fields| {
                (0..fields.len().saturating_sub(11)).step_by(4).find_map(|start| {
                    let to = pointer(&mut Reader::new(&fields[start..])).ok()?;
                    (to.file == 0 && to.path > 0 && to.path < 1 << 40).then_some(to.path)
                })
            });
            grid = template.and_then(|id| Scene::load_prefab_of_script(install, &file, id).ok()).map(|scene| ScenePanel { file: file.clone(), ..ScenePanel::of(&scene, 0) });
        }
        let scrolling = panel.find("Scrollview Parent/Containers Scrollview").and_then(|view| Scrolling::of(&panel, view, "ScrollRectNoDrag"));
        Ok(Self { panel, template, column, header, slot_panel, grid, grids_row, scrolling })
    }
}

/// What a script built on Odin's `SerializedMonoBehaviour` (the game's
/// `UIElement`) keeps before its own fields (`SerializationData`): the
/// format, the bytes, the objects they refer to, the bytes as a string, the
/// prefab, its modifications' objects and its modifications, and the
/// nodes (each a name, an entry's kind and its data). Read past, so that
/// the reader is at the script's own fields.
pub(super) fn skip_odin(r: &mut Reader) -> Result<()> {
    r.skip(4)?;
    let bytes = r.u32()? as usize;
    r.skip(bytes)?;
    r.align(4);
    let objects = r.u32()? as usize;
    r.skip(objects * 12)?;
    string(r)?;
    r.skip(12)?;
    let objects = r.u32()? as usize;
    r.skip(objects * 12)?;
    for _ in 0..r.u32()? {
        string(r)?;
    }
    for _ in 0..r.u32()? {
        string(r)?;
        r.skip(1)?;
        r.align(4);
        string(r)?;
    }
    Ok(())
}
