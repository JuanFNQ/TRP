//! The repair window built and drawn.

use super::*;

/// The repair windows' pieces, each window's past the one before it.
const REPAIR_WINDOW: usize = 30 << 20;

const REPAIR_STATUS: &str = "Inner/Contents/PreviewWarningPanel/RepairStatusPanel";

/// Its warnings' templates in `RepairWarningStatusPanel`'s order: an
/// error's, a warning's, an information's, a common buff's, a rare buff's.
pub(super) const REPAIR_TEMPLATES: [&str; 5] = [
    "Inner/Contents/PreviewWarningPanel/RepairStatusPanel/RepairErrorPanelTemplate",
    "Inner/Contents/PreviewWarningPanel/RepairStatusPanel/RepairWarningPanelTemplate",
    "Inner/Contents/PreviewWarningPanel/RepairStatusPanel/RepairInfoPanelTemplate",
    "Inner/Contents/PreviewWarningPanel/RepairStatusPanel/CommonBuffPanelTemplate",
    "Inner/Contents/PreviewWarningPanel/RepairStatusPanel/RareBuffPanelTemplate",
];

const REPAIR_PREVIEW: &str = "Inner/Contents/PreviewWarningPanel/Preview Panel/Camera Image";

const REPAIR_LOADER: &str = "Inner/Contents/PreviewWarningPanel/Preview Panel/Loader";

const REPAIR_PROCESS: &str = "Inner/Contents/PreviewWarningPanel/Preview Panel/RepairProcess";

const REPAIR_MASTERING: &str = "Inner/Contents/PreviewWarningPanel/Preview Panel/MasteringText";

const REPAIR_AVATAR: &str = "Inner/Contents/RepairerPanel/SelectRepairerPanel/Repairer/RepairerAvatarPanel/Avatar";

const REPAIR_RANK: &str = "Inner/Contents/RepairerPanel/SelectRepairerPanel/Repairer/RepairerAvatarPanel/Rank Panel";

const REPAIR_CHARGE: &str = "Inner/Contents/RepairerPanel/SelectRepairerPanel/Repairer/RepairerAvatarPanel/RepairKitChargeText";

const REPAIR_DROPDOWN: &str = "Inner/Contents/RepairerPanel/SelectRepairerPanel/Repairer/RepairerOptionsPanel/TraderDropdown/Text";

const REPAIR_PRICE_RATE: &str = "Inner/Contents/RepairerPanel/SelectRepairerPanel/Repairer/RepairerOptionsPanel/Standing/RepairPriceRate/Text";

const REPAIR_LINES: [&str; 4] = [
    "Inner/Contents/RepairerPanel/SelectRepairerPanel/Qualities/RepairerQualitiesPanel/RepairSpeedLabel",
    "Inner/Contents/RepairerPanel/SelectRepairerPanel/Qualities/RepairerQualitiesPanel/RepairQualityLabel",
    "Inner/Contents/RepairerPanel/SelectRepairerPanel/Qualities/RepairerQualitiesPanel/Repairability",
    "Inner/Contents/RepairerPanel/SelectRepairerPanel/Qualities/RepairerQualitiesPanel/EnhancementChance",
];

const REPAIR_SLIDER: &str = "Inner/Contents/RepairerPanel/ExceptionItemGroup/ConditionPanel/ConditionSlider";

const REPAIR_SUMS: &str = "Inner/Contents/RepairerPanel/ExceptionItemGroup/RepairSumPanel";

const REPAIR_BUTTON: &str = "Inner/Contents/ButtonsContainer/RepairButton";

/// What its scripts show and hide, besides the templates.
pub(super) const REPAIR_SCRIPTED: [&str; 6] = [REPAIR_LOADER, REPAIR_PROCESS, REPAIR_MASTERING, REPAIR_RANK, "Inner/Contents/RepairerPanel/SelectRepairerPanel/Repairer/RepairerOptionsPanel/Standing/Standing", "Inner/Contents/RepairerPanel/ExceptionItemGroup/ConditionPanel/ConditionSlider/bars/Slider"];

/// The condition's bar as `ConditionCharacteristicsSlider.SetSliderValues`
/// lays it out: fractions of the template durability (the current, the
/// greatest, the slider's top and its value), the degradation's range in
/// points, and the template durability.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct RepairCondition {
    pub current: f32,
    pub max: f32,
    pub top: f32,
    pub selected: f32,
    pub range: [f32; 2],
    pub template: f32,
}

/// What the repair window says and shows, as its scripts write it for a
/// kit (`RepairWindow`, `RepairerParametersPanel`, `RepairWarningStatusPanel`,
/// `ConditionCharacteristicsSlider`), worked out by `repair_view`.
#[derive(Clone, Debug, Default)]
pub(crate) struct RepairWords {
    pub caption: String,
    pub kind: String,
    pub weight: String,
    pub mastering: Option<String>,
    pub loading: bool,
    pub repairing: bool,
    /// The warnings shown, each its template (by `REPAIR_TEMPLATES`' order)
    /// and its words.
    pub warnings: Vec<(usize, String)>,
    pub dropdown: String,
    pub charge: String,
    pub price_rate: String,
    pub lines: [String; 4],
    /// The texts the install's words fill (`LocalizedText`), by their paths.
    pub labels: Vec<(String, String)>,
    pub condition: RepairCondition,
    pub durability: String,
    pub repair_time: String,
    pub repair_dp: String,
    pub in_stash: String,
    pub to_pay: String,
    pub to_pay_critical: bool,
    pub broken: bool,
    pub interactable: bool,
}

/// The repair window laid out for a thing: its panel with what its scripts
/// write, its size, and the places of what is clicked or drawn over it: the
/// preview, the kit's picture (the repairer's avatar), the slider's track
/// and the button to its top, the repair button, the caption and the cross.
pub(crate) struct RepairBuilt {
    pub(super) panel: trp_data::interface::screen::ScenePanel,
    pub(super) drawn: Drawn,
    pub(super) texts: HashMap<usize, String>,
    pub(super) set: HashMap<usize, bool>,
    pub(super) colors: HashMap<usize, Color>,
    pub(super) sprites: HashMap<usize, Picture>,
    pub(super) groups: HashMap<usize, f32>,
    pub(crate) size: [f32; 2],
    pub(crate) preview: [f32; 4],
    pub(crate) avatar: [f32; 4],
    pub(crate) slider: Option<[f32; 4]>,
    pub(crate) top_button: Option<[f32; 4]>,
    pub(crate) button: [f32; 4],
    pub(crate) caption: [f32; 4],
    pub(crate) close: [f32; 4],
}

/// The repair window laid out (`TARKOV UI`): the game's window with what its
/// scripts write (`RepairWords`), each text that sizes itself at its font's
/// size, a copy of a template for each warning in its panel, the condition's
/// bar's pieces at their fractions.
pub(crate) fn repair_built(look: &HealthLook, words: &RepairWords) -> Option<RepairBuilt> {
    let mut panel = look.repair.clone()?;
    let mut drawn = look.repair_drawn.clone();
    let mut texts = HashMap::new();
    let mut set = HashMap::new();
    let mut colors = HashMap::new();
    let mut sprites = HashMap::new();
    let mut groups = HashMap::new();
    let find = |panel: &trp_data::interface::screen::ScenePanel, path: &str| panel.find(path);
    // A text that sizes itself: its preferred size its words' at its font.
    let sized = |panel: &mut trp_data::interface::screen::ScenePanel, texts: &mut HashMap<usize, String>, index: usize, said: &str| {
        let plain = rich_spans(said).map_or_else(|| said.to_owned(), |spans| spans.into_iter().map(|(words, _)| words).collect());
        if let Some(size) = text_size(look, panel, index, &plain) {
            prefer_size(panel, index, size);
        }
        texts.insert(index, said.to_owned());
    };
    // The caption's row (`ItemInfoWindowLabels`), the weight sized to its words.
    for (path, said) in [("Inner/Caption Panel/Caption", &words.caption), ("Inner/Caption Panel/Item Type", &words.kind)] {
        if let Some(index) = find(&panel, path) {
            texts.insert(index, said.clone());
        }
    }
    if let Some(mass) = find(&panel, "Inner/Caption Panel/MassText") {
        if let Some([w, _]) = text_size(look, &panel, mass, &words.weight) {
            panel.nodes[mass].layout.size_delta[0] = w;
        }
        texts.insert(mass, words.weight.clone());
    }
    // The preview's loader, the repair's progress, a weapon's mastering.
    for (path, on) in [(REPAIR_LOADER, words.loading), (REPAIR_PROCESS, words.repairing), (REPAIR_MASTERING, words.mastering.is_some())] {
        if let Some(index) = find(&panel, path) {
            set.insert(index, on);
            panel.nodes[index].active |= on;
        }
    }
    if let (Some(level), Some(index)) = (&words.mastering, find(&panel, REPAIR_MASTERING)) {
        if let Some([w, _]) = text_size(look, &panel, index, level) {
            panel.nodes[index].layout.size_delta[0] = w;
        }
        texts.insert(index, level.clone());
    }
    // A kit is no trader: no rank, no standing (`TraderSelected`).
    for index in find(&panel, REPAIR_RANK).into_iter().chain(look.repair_figures.standing) {
        set.insert(index, false);
        panel.nodes[index].active = false;
    }
    for (path, said) in [(REPAIR_DROPDOWN, &words.dropdown), (REPAIR_CHARGE, &words.charge)] {
        if let Some(index) = find(&panel, path) {
            texts.insert(index, said.clone());
        }
    }
    if let Some(index) = find(&panel, REPAIR_PRICE_RATE) {
        sized(&mut panel, &mut texts, index, &words.price_rate);
    }
    for (path, said) in REPAIR_LINES.iter().zip(&words.lines) {
        if let Some(index) = find(&panel, path) {
            sized(&mut panel, &mut texts, index, said);
        }
    }
    for (path, said) in &words.labels {
        if let Some(index) = find(&panel, path) {
            sized(&mut panel, &mut texts, index, said);
        }
    }
    // The sums' figures, and the kit's points' icons in the cash's place.
    for (end, said) in [("RepairTime/Value", &words.repair_time), ("RepairDP/Value", &words.repair_dp), ("CashInStash/Value", &words.in_stash), ("TotalSum/Value", &words.to_pay)] {
        if let Some(index) = find(&panel, &format!("{REPAIR_SUMS}/{end}")) {
            sized(&mut panel, &mut texts, index, said);
        }
    }
    let [critical, default] = look.repair_figures.sum_colors;
    if let Some(index) = find(&panel, &format!("{REPAIR_SUMS}/TotalSum/Value")) {
        colors.insert(index, rgba(if words.to_pay_critical { critical } else { default }));
    }
    for (end, icon) in [("CashInStash/Icon", look.repair_icons[0]), ("TotalSum/Icon", look.repair_icons[1])] {
        if let (Some(index), Some(picture)) = (find(&panel, &format!("{REPAIR_SUMS}/{end}")), icon) {
            sprites.insert(index, picture);
        }
    }
    // The warnings, each a copy of its template under the panel.
    if let Some(status) = find(&panel, REPAIR_STATUS) {
        for (template, said) in &words.warnings {
            let Some(top) = REPAIR_TEMPLATES.get(*template).and_then(|path| find(&panel, path)) else { continue };
            let copy = copy_within(&mut panel, &mut drawn, top, status);
            set.insert(copy, true);
            texts.insert(copy + 1, said.clone());
        }
    }
    // The condition's bar (`SetSliderValues`), its border and its current
    // durability in the colour broken or not (`SetSliderActive`).
    let c = words.condition;
    let template = c.template.max(1.0);
    let bar = |end: &str| format!("{REPAIR_SLIDER}/bars/{end}");
    let anchor = |panel: &mut trp_data::interface::screen::ScenePanel, end: &str, from: Option<f32>, to: Option<f32>| {
        if let Some(index) = panel.find(&bar(end)) {
            if let Some(from) = from {
                panel.nodes[index].layout.anchor_min[0] = from;
            }
            if let Some(to) = to {
                panel.nodes[index].layout.anchor_max[0] = to;
            }
        }
    };
    let lowest = (c.max - c.range[0] / template).clamp(0.0, 1.0);
    anchor(&mut panel, "CurrentDurability", None, Some(c.current));
    anchor(&mut panel, "MaximumDurability", None, Some(c.max));
    anchor(&mut panel, "MaxDegradation", Some((c.max - c.range[1] / template).clamp(0.0, 1.0)), Some(lowest));
    anchor(&mut panel, "Slider", Some(0.01 + c.current), Some(c.max - c.range[0] / template));
    anchor(&mut panel, "MaximumRemovedDurability", Some(c.current + c.top), Some(c.max));
    // (The game's quirk: the selection's start a fraction over the template
    // again, so from about nought, under the current durability's.)
    anchor(&mut panel, "SelectedDurability", Some(c.current / template), Some(c.current + c.selected));
    let shown = c.top >= 0.001;
    if let Some(slider) = panel.find(&bar("Slider")) {
        set.insert(slider, shown);
        panel.nodes[slider].active = shown;
    }
    let value = if shown { ((c.selected - 0.001) / (c.top - 0.001).max(1e-6)).clamp(0.0, 1.0) } else { 0.0 };
    if let Some(handle) = panel.find(&bar("Slider/Handle Slide Area/Handle")) {
        panel.nodes[handle].layout.anchor_min = [value, 0.0];
        panel.nodes[handle].layout.anchor_max = [value, 1.0];
    }
    let [slider_critical, slider_default] = look.repair_figures.slider_colors;
    let tint = rgba(if words.broken { slider_critical } else { slider_default });
    for end in ["Border", "CurrentDurability"] {
        if let Some(index) = panel.find(&bar(end)) {
            colors.insert(index, tint);
        }
    }
    if let Some(index) = panel.find(&format!("{REPAIR_SLIDER}/Durability")) {
        texts.insert(index, words.durability.clone());
        colors.insert(index, Color::WHITE);
    }
    // The button, dim where it cannot be pressed (`PROVISIONAL`: the
    // button's own disabled look, `DefaultUIButtonAnimation`, not read).
    if let Some(button) = panel.find(REPAIR_BUTTON) {
        if !words.interactable {
            groups.insert(button, 0.5);
        }
    }
    let size = fitted_size(&panel);
    let rect = |path: &str| panel.find(path).map(|index| panel.rect(index, size));
    Some(RepairBuilt {
        preview: rect(REPAIR_PREVIEW).unwrap_or([0.0; 4]),
        avatar: rect(REPAIR_AVATAR).unwrap_or([0.0; 4]),
        slider: shown.then(|| rect(&bar("Slider/Handle Slide Area"))).flatten(),
        top_button: shown.then(|| rect(&bar("Slider/MaximumDurabilityButton"))).flatten(),
        button: rect(REPAIR_BUTTON).unwrap_or([0.0; 4]),
        caption: rect("Inner/Caption Panel").unwrap_or([0.0; 4]),
        close: rect("Inner/Caption Panel/Close Button").unwrap_or([0.0; 4]),
        panel,
        drawn,
        texts,
        set,
        colors,
        sprites,
        groups,
        size,
    })
}

/// The repair window drawn as built, its pieces numbered past the windows
/// before it.
pub(crate) fn draw_repair_window(column: &mut ChildSpawnerCommands, look: &HealthLook, said: &Words, built: &RepairBuilt, index: usize, s: f32) {
    let RepairBuilt { panel, drawn, texts, set, colors, sprites, groups, size, .. } = built;
    let base = REPAIR_WINDOW + (index << 12);
    let marks = Marks { words: Some(texts), set: Some(set), colors: Some(colors), sprites: Some(sprites), groups: Some(groups), ..Marks::default() };
    draw_nodes(column, look, panel, drawn, base, marks, said, Frame::Is(*size), [0.0, 0.0], s);
}
