//! The HEALTH panel, its body parts, figures and slot views.

use super::*;

/// A part's view on the health panel (`BodyPartView`): itself, and what it
/// writes to as health changes, each an object of the panel.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PartView {
    pub view: usize,
    /// Its figures (`_valueText`).
    pub value: Option<usize>,
    /// The bar filled by the share left (`_healthBar`), and its border.
    pub bar: Option<usize>,
    pub border: Option<usize>,
    /// The part's shape in the silhouette, coloured by how hurt it is.
    pub shape: Option<usize>,
    /// The cross of a destroyed part.
    pub broken: Option<usize>,
    pub effects: Option<usize>,
    /// The damage of a raid gone by, which a raid has none of.
    pub damage: Option<usize>,
}

/// One of the health panel's figures (`HealthParameterPanel`, and with a
/// rate `BuffableHealthParameterPanel`): what it writes to and its colours.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ParameterView {
    pub panel: usize,
    pub current: Option<usize>,
    pub max: Option<usize>,
    pub ok_glow: Option<usize>,
    pub glow: Option<usize>,
    pub icon: Option<usize>,
    pub warning: [f32; 4],
    pub alternative: [f32; 4],
    /// The fewest digits its figure is written with, and the colour of
    /// the noughts put before it to make them up.
    pub least_digits: i32,
    pub noughts: [f32; 4],
    /// The rate's arrows and its figure, where it has a rate.
    pub up: Option<usize>,
    pub down: Option<usize>,
    pub rate: Option<usize>,
}

impl ParameterView {
    pub(super) fn read(panel: &ScenePanel, index: usize) -> Option<Self> {
        let (fields, buffs) = match panel.script(index, "BuffableHealthParameterPanel") {
            Some(fields) => (fields, true),
            None => (panel.script(index, "HealthParameterPanel")?, false),
        };
        // Its own fields in their order: five pointers, two colours, a
        // count and a colour; then the rate's three pointers.
        let mut r = Reader::new(fields.get(60..)?);
        let (warning, alternative, least_digits, noughts) = (color(&mut r).ok()?, color(&mut r).ok()?, r.i32().ok()?, color(&mut r).ok()?);
        let at = |n: usize| panel.pointer_at(fields, n);
        let rate = |n: usize| if buffs { at(n) } else { None };
        Some(Self { panel: index, current: at(0), max: at(12), ok_glow: at(24), glow: at(36), icon: at(48), warning, alternative, least_digits, noughts, up: rate(112), down: rate(124), rate: rate(136) })
    }
}

/// What the health panel shows, as its scripts name it.
#[derive(Debug, Clone, Default)]
pub struct HealthViews {
    /// The parts' views in the order of the panel's fields (head, chest,
    /// stomach, left arm, right arm, left leg, right leg).
    pub parts: Vec<PartView>,
    pub silhouette: Option<usize>,
    /// The figures by the panel's fields' names: overall health,
    /// poisoning, radiation, blood pressure, energy, hydration,
    /// temperature, weight (none on this panel).
    pub parameters: Vec<(&'static str, Option<ParameterView>)>,
    pub legend: Option<usize>,
}

/// The equipment panel's slot views as its script names them
/// (`EquipmentTab`: eleven, in `GEAR_VIEWS`' order), and what each view's
/// script names (`SlotView`: the full border, the search icon, the empty
/// slot's picture, a camera's, the place of what is in it, the empty
/// fill, the empty border, the chosen border and corner, the header's
/// words, the header).
fn slot_views(panel: &ScenePanel) -> Vec<(&'static str, SlotParts)> {
    let Some(named) = panel.script(0, "EquipmentTab").and_then(|fields| pointers_from(panel, fields, GEAR_VIEWS.len(), 10)) else { return Vec::new() };
    GEAR_VIEWS
        .iter()
        .zip(named)
        .filter_map(|(slot, view)| {
            let view = view?;
            let at = panel.script(view, "SlotView").and_then(|fields| pointers_from(panel, fields, 11, 6))?;
            let parts = SlotParts {
                view,
                full_border: at[0],
                back_image: at[2],
                background: at[5],
                empty_border: at[6],
                selected_border: at[7],
                selected_corner: at[8],
                header_text: at[9],
                header: at[10],
            };
            Some((*slot, parts))
        })
        .collect()
}

/// The figures a figures' panel names (`HealthParametersPanel`: overall
/// health, poisoning, radiation, blood pressure, energy, hydration,
/// temperature and weight, in its fields' order), so many of them at
/// least.
fn figures_of(panel: &ScenePanel, index: usize, least: usize) -> Vec<(&'static str, Option<ParameterView>)> {
    const FIGURES: [&str; 8] = ["OverallHealth", "Poisoning", "Radiation", "BloodPressure", "Energy", "Hydration", "Temperature", "Weight"];
    let figures = panel.script(index, "HealthParametersPanel").and_then(|fields| pointers_from(panel, fields, 8, least));
    FIGURES.iter().enumerate().map(|(n, name)| (*name, figures.as_ref().and_then(|at| at[n]).and_then(|index| ParameterView::read(panel, index)))).collect()
}

/// A figure of a list (`HealthViews::parameters`, `HealthPanel::
/// strip_figures`) by its field's name.
pub fn figure_named<'a>(figures: &'a [(&'static str, Option<ParameterView>)], name: &str) -> Option<&'a ParameterView> {
    figures.iter().find(|(field, _)| *field == name).and_then(|(_, view)| view.as_ref())
}

/// The pointers of a script's fields that come one after another from the
/// first place where `count` of them in a row lead into the panel (at
/// least `least` of them, the first among them): past what the script's
/// parent classes keep.
pub(super) fn pointers_from(panel: &ScenePanel, fields: &[u8], count: usize, least: usize) -> Option<Vec<Option<usize>>> {
    (0..fields.len()).step_by(4).find_map(|start| {
        let found: Vec<Option<usize>> = (0..count).map(|n| panel.pointer_at(fields, start + 12 * n)).collect();
        (found[0].is_some() && found.iter().flatten().count() >= least).then_some(found)
    })
}

impl HealthViews {
    pub(super) fn read(panel: &ScenePanel) -> Option<Self> {
        // The panel's own script: the figures, the silhouette, the seven
        // views and the legend.
        let fields = panel.script(0, "InventoryScreenHealthPanel")?;
        let named = pointers_from(panel, fields, 10, 10)?;
        let mut parts = Vec::new();
        for view in named[2..9].iter() {
            let view = (*view)?;
            let fields = panel.script(view, "BodyPartView")?;
            let at = pointers_from(panel, fields, 8, 7)?;
            parts.push(PartView { view, value: at[0], bar: at[1], border: at[2], shape: at[3], broken: at[4], effects: at[5], damage: at[6] });
        }
        let parameters = named[0].map_or_else(Vec::new, |figures| figures_of(panel, figures, 7));
        Some(Self { parts, silhouette: named[1], parameters, legend: named[9] })
    }

    /// A figure by its field's name.
    pub fn parameter(&self, name: &str) -> Option<&ParameterView> {
        self.parameters.iter().find(|(field, _)| *field == name).and_then(|(_, view)| view.as_ref())
    }
}

/// The inventory screen's health panel (`InventoryScreenHealthPanel`),
/// which its HEALTH tab shows in the left panel in place of the equipment
/// panel (`ItemsPanel.UpdateEquipment`), and what places the left panel
/// on the screen (`docs/spec/health.md`, "The HEALTH tab").
#[derive(Debug, Clone, Default)]
pub struct HealthPanel {
    pub panel: ScenePanel,
    pub views: HealthViews,
    /// The strip of figures the GEAR tab has under its equipment
    /// (`Health Parameters`: weight, health, hydration and energy in a
    /// row), and what its script names.
    pub strip: Option<ScenePanel>,
    pub strip_figures: Vec<(&'static str, Option<ParameterView>)>,
    /// The left panel's foot (`Bottom Bar`), a picture under both tabs'
    /// panels.
    pub foot: Option<ScenePanel>,
    /// The equipment panel as the scene has it (`Gear Panel`, its own
    /// size set while the game runs: its template's), and its slot views
    /// by their slots' names in the order its script names them
    /// (`GEAR_VIEWS`), with what each view's script names.
    pub gear_panel: Option<ScenePanel>,
    pub slot_views: Vec<(&'static str, SlotParts)>,
    /// The screen's left side (`Items Panel/LeftSide`), whose horizontal
    /// layout lays the left panel and the containers' panel side by side,
    /// so far apart.
    pub left_side: Layout,
    pub spacing: f32,
    /// Where the equipment panel lies in the left panel: its template's
    /// layout, beside it (its own is set while the game runs).
    pub gear: Layout,
    /// The left panel's own ground (its picture's colour, a plain one),
    /// and the left side's frame round both its panels (`Border`, out of
    /// the layout, drawn over them).
    pub left_ground: Option<[f32; 4]>,
    pub border: Option<ScenePanel>,
    /// The panel on the screen's right that holds a container opened or
    /// the stash (`Items Panel/Stash Panel`), by its own layout.
    pub stash: Option<Layout>,
}

/// A slot view (`SlotView`): itself, and what its script names, each an
/// object of the equipment panel: the borders full and empty, the empty
/// slot's picture and its fill, the borders of a slot chosen, its header
/// and the header's words.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SlotParts {
    pub view: usize,
    pub full_border: Option<usize>,
    pub back_image: Option<usize>,
    pub background: Option<usize>,
    pub empty_border: Option<usize>,
    pub selected_border: Option<usize>,
    pub selected_corner: Option<usize>,
    pub header_text: Option<usize>,
    pub header: Option<usize>,
}

impl HealthPanel {
    /// Reads it from the scene file that holds the inventory screen.
    pub fn load(install: &Install, file: &str) -> Result<Self> {
        const LEFT: &str = "/InventoryScreen/Items Panel/LeftSide/Left Panel";
        let scene = Scene::load(install, file, true)?;
        let left = (0..scene.nodes.len()).find(|&node| scene.nodes[node].name == "Left Panel" && scene.path(node).ends_with(LEFT)).with_context(|| format!("{file} has no inventory screen"))?;
        let child = |name: &str| scene.nodes[left].children.iter().copied().find(|&child| scene.nodes[child].name == name);
        let health = child("Health Panel").with_context(|| format!("{file}: the inventory's left panel has no health panel"))?;
        let gear = child("Gear Panel Template").and_then(|template| scene.nodes[template].layout).context("the equipment panel's template")?;
        let side = scene.nodes[left].parent.context("the left side")?;
        let left_side = scene.nodes[side].layout.context("the left side's layout")?;
        // The layout group's fields: its padding, the corner it lays out
        // from, then the space between its children.
        let group = scene.behaviour(side, "HorizontalLayoutGroup").context("the left side's layout group")?;
        let mut r = Reader::new(scene.fields(group));
        r.skip(16 + 4)?;
        let spacing = r.f32()?;
        let of = |top: usize| ScenePanel { file: file.to_owned(), ..ScenePanel::of(&scene, top) };
        let panel = of(health);
        let views = HealthViews::read(&panel).with_context(|| format!("{file}: the health panel's scripts do not name its views"))?;
        let strip = child("Health Parameters").map(of);
        let strip_figures = strip.as_ref().map_or_else(Vec::new, |strip| figures_of(strip, 0, 4));
        let foot = child("Bottom Bar").map(of);
        let gear_panel = child("Gear Panel").map(of);
        let slot_views = gear_panel.as_ref().map_or_else(Vec::new, slot_views);
        let left_ground = scene.behaviour(left, "Image").and_then(|script| image_look(scene.fields(script))).map(|image| image.color);
        let border = scene.nodes[side].children.iter().copied().find(|&child| scene.nodes[child].name == "Border").map(of);
        let stash = scene.nodes[side].parent.and_then(|items| scene.nodes[items].children.iter().copied().find(|&child| scene.nodes[child].name == "Stash Panel")).and_then(|stash| scene.nodes[stash].layout);
        Ok(Self { panel, views, left_side, spacing, gear, strip, strip_figures, foot, gear_panel, slot_views, left_ground, border, stash })
    }

    /// The left side's place on a screen so wide and high, in its units:
    /// its left, top, width and height, y down (the screen's items' panel
    /// is the whole screen).
    pub fn left_side_in(&self, screen: [f32; 2]) -> [f32; 4] {
        let side = place(self.left_side, Rect { at: [0.0, 0.0], size: screen });
        [side.at[0], screen[1] - side.at[1] - side.size[1], side.size[0], side.size[1]]
    }

    /// The right's panel's place (`stash`) the same way.
    pub fn stash_in(&self, screen: [f32; 2]) -> Option<[f32; 4]> {
        let stash = place(self.stash?, Rect { at: [0.0, 0.0], size: screen });
        Some([stash.at[0], screen[1] - stash.at[1] - stash.size[1], stash.size[0], stash.size[1]])
    }

    /// The left panel's size on a screen so wide and high, in the screen's
    /// units: the left side's width shared out by its layout group, which
    /// sets its children's widths and makes them all grow alike, between
    /// the left panel and the containers' panel, neither of which asks for
    /// any width of its own (the left panel's picture has no sprite, the
    /// containers' panel has nothing to lay it out, the border between
    /// them is left out of the layout), and the left side's height.
    pub fn left_panel(&self, screen: [f32; 2]) -> [f32; 2] {
        let side = place(self.left_side, Rect { at: [0.0, 0.0], size: screen });
        [(side.size[0] - self.spacing) / 2.0, side.size[1]]
    }

    /// The equipment panel's place in a left panel of that size: its left,
    /// top, width and height, y down.
    pub fn gear_in(&self, left: [f32; 2]) -> [f32; 4] {
        let rect = place(self.gear, Rect { at: [0.0, 0.0], size: left });
        [rect.at[0], left[1] - rect.at[1] - rect.size[1], rect.size[0], rect.size[1]]
    }
}
