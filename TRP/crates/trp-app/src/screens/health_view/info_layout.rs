//! The inspect window's words, places and scroll, and its parts built.

use super::*;

/// What the window that shows a thing writes of it
/// (`ItemSpecificationPanel`, into `ItemInfoWindowLabels`): its name in
/// the caption (`ItemName`), its kind as the handbook files it
/// (`_handbookNode.Category`, " > " between), its weight with the unit
/// (`GetTruncatedWeightString`, three places) and its description.
#[derive(Default)]
pub struct InfoWords {
    /// Its tag, where it has one named (`InfoWindow.UpdateTag`).
    pub tag: Option<(String, u32)>,
    /// Its kind's group on the wish list, where it is on it
    /// (`ItemSpecificationPanel._wishlistIcon`).
    pub wish: Option<&'static str>,
    pub caption: String,
    pub kind: String,
    pub weight: String,
    pub description: String,
}

/// The objects of the window that shows a thing its script turns off for
/// a thing of the range (`HealthLook::read`); its weight's figure, its
/// caption bar, its cross, its preview and its description's box.
pub(super) const INFO_OFF: [&str; 0] = [];

/// Its panel of a thing's parts' slots (`_modsPanel`), on for a thing with
/// slots, its grid of slots and its scrollbar; a weapon's mastering row,
/// its value, its badge under the preview and the preview's corner beside
/// it (`_masteringBg`), on for a weapon; its buttons' grid and its three
/// templates (`SimpleSpecifiedContextButtons`: Discard's own, the wish
/// list's own, the rest).
pub(super) const INFO_MODS: &str = "Inner/Contents/DescriptionPanel/ModsPanel";

pub(super) const INFO_MODS_CONTENT: &str = "Inner/Contents/DescriptionPanel/ModsPanel/Padding/Content";

pub(super) const INFO_MODS_BAR: &str = "Inner/Contents/DescriptionPanel/ModsPanel/Scrollbar";

pub(super) const INFO_MASTERING: &str = "Inner/Contents/DescriptionPanel/CharacteristicsPanel/Mastering";

pub(super) const INFO_MASTERING_VALUE: &str = "Inner/Contents/DescriptionPanel/CharacteristicsPanel/Mastering/Value";

pub(super) const INFO_MASTERING_BADGE: &str = "Inner/Contents/Preview Panel/MasteringAndQuestPanel/MasteringPanel";

pub(super) const INFO_CORNER: &str = "Inner/Contents/Preview Panel/bottom_left";

pub(super) const INFO_BUTTONS: &str = "Inner/Contents/InteractionButtonsPanel/InteractionButtonsContainer";

pub(super) const INFO_BUTTON_TEMPLATES: [&str; 3] = [
    "Inner/Contents/InteractionButtonsPanel/InteractionButtonsContainer/Button Template",
    "Inner/Contents/InteractionButtonsPanel/InteractionButtonsContainer/Wishlist Template",
    "Inner/Contents/InteractionButtonsPanel/InteractionButtonsContainer/Dispose Template",
];

/// The window's stretching places (`StretchButtons`' `StretchArea`s), each
/// with its sides (`EStretchAreaType`: top 1, bottom 2, left 4, right 8).
const INFO_STRETCH: [(&str, u8); 8] = [
    ("StretchButtons/TopStretch", 1),
    ("StretchButtons/BottomStretch", 2),
    ("StretchButtons/LeftStretch", 4),
    ("StretchButtons/RightStretch", 8),
    ("StretchButtons/TopLeftStretch", 5),
    ("StretchButtons/TopRightStretch", 9),
    ("StretchButtons/BottomLeftStretch", 6),
    ("StretchButtons/BottomRightStretch", 10),
];

/// A slot's pieces its script shows as the slot is full or empty.
pub(super) const MOD_SLOT_PIECES: [&str; 6] = ["CanvasGroup/Empty Border", "CanvasGroup/Full Border", "CanvasGroup/Background", "CanvasGroup/Background Image", "CanvasGroup/Text", "CanvasGroup/filters"];

/// Its loader (shown until its model is in), its column of bars and the
/// weapon's durability row at its top, its compact grid.
pub(super) const INFO_LOADER: &str = "Inner/Contents/Preview Panel/Loader";

pub(super) const INFO_DURABILITY: &str = "Inner/Contents/DescriptionPanel/CharacteristicsPanel/Durability";

pub(super) const INFO_BARS: &str = "Inner/Contents/DescriptionPanel/CharacteristicsPanel";

pub(super) const INFO_COMPACT: &str = "Inner/Contents/DescriptionPanel/CompactPanel";

/// Its mark of the wish list, which its script shows for a thing on it.
pub(super) const INFO_WISH: &str = "Inner/Contents/Preview Panel/MasteringAndQuestPanel/WishlistIcon";

/// Its tag's plate and name, which its script shows for a thing tagged.
pub(super) const INFO_TAG: &str = "Inner/Caption Panel/TagPanel";

const INFO_TAG_TEXT: &str = "Inner/Caption Panel/TagPanel/TagText";

pub(super) const INFO_MASS: &str = "Inner/Caption Panel/MassText";

const INFO_CAPTION: &str = "Inner/Caption Panel";

const INFO_CLOSE: &str = "Inner/Caption Panel/Close Button";

const INFO_PREVIEW: &str = "Inner/Contents/Preview Panel/Camera Image";

pub(super) const INFO_SCROLL: &str = "Inner/Contents/DescriptionPanel/DescriptionPanel/Scroll Area";

pub(super) const INFO_DESCRIPTION: &str = "Inner/Contents/DescriptionPanel/DescriptionPanel/Scroll Area/Contents/Description";

/// The window that shows a thing as the game lays it out (`TARKOV UI`):
/// its size, and where in it its caption bar (by which it is moved) and
/// its cross lie, in units from its top left corner.
pub struct InfoPlaces {
    pub size: [f32; 2],
    pub caption: [f32; 4],
    pub close: [f32; 4],
}

pub fn info_places(look: &HealthLook) -> Option<InfoPlaces> {
    let info = look.info.as_ref()?;
    let size = info_size(info);
    let rect = |path: &str| info.find(path).map(|index| info.rect(index, size));
    Some(InfoPlaces { size, caption: rect(INFO_CAPTION)?, close: rect(INFO_CLOSE)? })
}

/// The window's own size, as it lies in a screen (its anchors at a point).
pub(super) fn info_size(info: &trp_data::interface::screen::ScenePanel) -> [f32; 2] {
    let [_, _, w, h] = info.placed_in(0, [1920.0, 1080.0]).rect;
    [w, h]
}

/// The window that shows a thing laid out for it (`TARKOV UI`): the game's
/// window with what its script writes (`InfoWords`), a weapon's durability
/// row, a copy of a row template for each of its figures (inspect-window
/// spec, "Where the figures go"), its loader while its model is made, all
/// laid out by the game's layout; the window as tall as what it holds asks
/// and at least its own height (its `ContentSizeFitter` and
/// `LayoutElement`).
pub struct InfoBuilt {
    pub(super) panel: trp_data::interface::screen::ScenePanel,
    pub(super) drawn: Drawn,
    pub(super) texts: HashMap<usize, String>,
    pub(super) set: HashMap<usize, bool>,
    pub(super) colors: HashMap<usize, Color>,
    pub(super) sprites: HashMap<usize, Picture>,
    pub(super) groups: HashMap<usize, f32>,
    /// The kinds' marks of an empty slot's filters, each where it lies.
    pub(super) icons: Vec<([f32; 4], Picture)>,
    pub size: [f32; 2],
    /// Where each slot of the parts' panel lies, and each button, in the
    /// order given; each stretching place with its sides; in units from
    /// the window's top left corner.
    pub slots: Vec<[f32; 4]>,
    pub buttons: Vec<[f32; 4]>,
    /// Beside `buttons`, each one's ground (`_backgroundImage`): its node,
    /// its colours at rest and under the mouse, and its group's alpha.
    pub grounds: Vec<Option<(usize, [Color; 2], f32)>>,
    pub stretch: Vec<(u8, [f32; 4])>,
    /// Each dropdown row's arrow (`_dropdownImage`), with the row's place
    /// among the figures' rows.
    pub tips: Vec<([f32; 4], usize)>,
    /// The description's scroll view, where its text is taller than its
    /// area (else its bar is hidden: `AutoHide`).
    pub scroll: Option<InfoScroll>,
}

/// The description's scroll view as the window is built
/// (`DescriptionPanel/Scroll Area`'s `ScrollRectNoDrag`; inspect-window
/// spec, "The description"): where its area, its bar and its bar's sliding
/// area lie, in units from the window's top left; its handle (its node,
/// layout and colour) and how the bar runs and is tinted; how tall the text
/// is and how much of it shows; the units a notch of the wheel moves it.
#[derive(Clone)]
pub struct InfoScroll {
    pub area: [f32; 4],
    pub bar: [f32; 4],
    pub sliding: [f32; 4],
    pub(super) handle: usize,
    handle_layout: trp_data::interface::screen::Layout,
    bottom_up: bool,
    pub tints: Option<trp_data::interface::screen::Tints>,
    pub color: Color,
    pub whole: f32,
    pub shown: f32,
    pub sensitivity: f32,
    pub vertical: bool,
}

impl InfoScroll {
    /// How far it scrolls, units.
    pub fn most(&self) -> f32 {
        (self.whole - self.shown).max(0.0)
    }

    /// The handle's share of its bar (`Scrollbar.size`).
    pub fn share(&self) -> f32 {
        if self.whole > 0.0 { (self.shown / self.whole).clamp(0.0, 1.0) } else { 1.0 }
    }

    /// The bar's value with the text moved `at` units up, and back.
    pub fn value_of(&self, at: f32) -> f32 {
        let most = self.most();
        if most > 0.0 { 1.0 - at / most } else { 1.0 }
    }
    pub fn at_of(&self, value: f32) -> f32 {
        (1.0 - value.clamp(0.0, 1.0)) * self.most()
    }

    /// Whether the bar runs from the bottom up.
    pub fn bottom_up(&self) -> bool {
        self.bottom_up
    }

    /// Where the handle lies with the text moved `at` units up.
    pub fn handle_at(&self, at: f32) -> [f32; 4] {
        trp_data::interface::screen::handle_on(self.sliding, self.handle_layout, self.bottom_up, self.value_of(at), self.share())
    }
}

/// What of the description moves as it is scrolled, and the bar's handle
/// with its own colour, by the window's opening (`Win::opened`).
#[derive(Component)]
pub(crate) struct InfoScrollContent(pub(crate) u64);

#[derive(Component)]
pub(crate) struct InfoScrollHandle(pub(crate) u64, pub(crate) Color);

/// The description's scroll view (`ScrollRectNoDrag`, vertical, its bar
/// hidden while the text fits): the text's height as its font lays it out
/// at the area's width less its margins, with its margins above and below
/// (`Contents`' `ContentSizeFitter` at its preferred height), against the
/// area's height; none where it fits by a hundredth, or where the text is
/// not one of the game's fonts (`PROVISIONAL`: then taken to fit).
fn info_scroll(look: &HealthLook, panel: &trp_data::interface::screen::ScenePanel, size: [f32; 2], description: &str) -> Option<InfoScroll> {
    let area = panel.find(INFO_SCROLL)?;
    let scrolling = trp_data::interface::screen::Scrolling::of(panel, area, "ScrollRectNoDrag")?;
    let bar = scrolling.bar?;
    let text = panel.nodes[panel.find(INFO_DESCRIPTION)?].text.as_ref()?;
    let [_, _, w, h] = panel.rect(area, size);
    let (asset, _) = look.tmp_of(panel, text)?;
    let face = look.tmp_faces.get(&asset)?;
    let run = trp_data::interface::tmp_text::TmpRun { text: description.to_owned(), size: text.size, color: [1.0; 4], bold: text.style & 1 != 0 };
    let layout = trp_data::interface::tmp_text::TmpLayout { em: text.size, spacing: text.spacing, kerning: text.kerning, wrap: Some(w - text.margin[0] - text.margin[2]), align: text.horizontal & 7, lead: 0.0 };
    let [_, tall] = trp_data::interface::tmp_text::measure(face, &[run], &layout);
    let whole = tall + text.margin[1] + text.margin[3];
    if whole <= h + 0.01 {
        return None;
    }
    let (handle, sliding) = (bar.handle?, bar.sliding?);
    let color = panel.nodes[handle].image.map_or(Color::WHITE, |image| rgba(image.color));
    Some(InfoScroll {
        area: panel.rect(area, size),
        bar: panel.rect(bar.object, size),
        sliding: panel.rect(sliding, size),
        handle,
        handle_layout: panel.nodes[handle].layout,
        bottom_up: bar.bottom_up,
        tints: bar.tints.filter(|_| bar.tinted == bar.handle),
        color,
        whole,
        shown: h,
        sensitivity: scrolling.sensitivity,
        vertical: scrolling.vertical,
    })
}

/// A slot of the parts' panel as the window shows it (`ModSlotView.Show`,
/// `SetLocked`; inspect-window spec, "The parts' panel").
pub struct InfoSlot {
    /// Its name in the install's words, upper case
    /// (`slot.Name.Localized().ToUpper()`), shown while it is empty.
    pub name: String,
    /// Its picture's name (`Slots/<name>`, lower case), and whether it is
    /// an armour's (`armor_plate`'s picture where its own has none).
    pub picture: String,
    pub armour: bool,
    pub full: bool,
    /// Locked (`ModLock`: the slot at half its alpha, nothing taken from it).
    pub locked: bool,
    /// A chamber not checked (`EModLockedState.ChamberUnchecked`): locked,
    /// its picture the question's at its own size and its name shown, over
    /// a full slot's border, its part not shown (`ModSlotView.SetLocked`,
    /// `SetUnknownIconStatus`).
    pub unchecked: bool,
    /// The marks of the kinds of part it takes (`SetFilters`), shown while
    /// it is empty.
    pub marks: Vec<&'static str>,
    /// What it says under the mouse while locked (`ModSlotView.TooltipData`,
    /// or an armour plate's words: `inventory_view::info_parts`); empty
    /// where it says nothing.
    pub tip: String,
}

/// A button of the window's grid of them (`InteractionButtonsContainer`):
/// its line's name (its template and icon go by it), its caption, whether
/// it can be done now, whether it opens lines of its own.
pub struct InfoButtonLook {
    pub name: &'static str,
    pub caption: String,
    pub enabled: bool,
    pub arrow: bool,
}

/// A weapon's mastering as the window shows it: its level (0 to 2), its
/// points into it and the points the level takes.
pub struct InfoMastering {
    pub level: u32,
    pub points: f32,
    pub most: f32,
}

/// What the window holds besides its figures (inspect-window spec): its
/// parts' slots, its buttons, a weapon's mastering, and the size it is
/// stretched to (its `LayoutElement`'s preferred size, in units; none as
/// made, 670 by 500).
#[derive(Default)]
pub struct InfoParts {
    pub slots: Vec<InfoSlot>,
    pub buttons: Vec<InfoButtonLook>,
    pub mastering: Option<InfoMastering>,
    pub stretched: Option<[f32; 2]>,
}

impl InfoBuilt {
    /// Where its caption bar and its cross lie.
    pub fn places(&self) -> Option<InfoPlaces> {
        let rect = |path: &str| self.panel.find(path).map(|index| self.panel.rect(index, self.size));
        Some(InfoPlaces { size: self.size, caption: rect(INFO_CAPTION)?, close: rect(INFO_CLOSE)? })
    }

    /// Where its model is drawn (`Camera Image`), in units from its top
    /// left corner.
    pub fn preview(&self) -> Option<[f32; 4]> {
        self.panel.find(INFO_PREVIEW).map(|index| self.panel.rect(index, self.size))
    }
}

/// The window laid out for a thing with those figures (none: its frame
/// alone), its loader shown while `loading`.
pub fn info_built(look: &HealthLook, words: &InfoWords, figures: Option<&trp_sim::items::item_attributes::Figures>, parts: &InfoParts, loading: bool) -> Option<InfoBuilt> {
    let info = look.info.as_ref()?;
    let mut panel = info.clone();
    let mut drawn = Drawn { shown: look.info_drawn.shown.clone(), pictures: look.info_drawn.pictures.clone() };
    // Stretched (`StretchArea.OnDrag`): its preferred size the one dragged
    // to, which it is never smaller than what it holds asks; first, as the
    // grids within it lay out by its width.
    if let Some(stretched) = parts.stretched {
        let element = panel.nodes[0].element.unwrap_or(trp_data::interface::screen::Element { ignore: false, min: [-1.0, -1.0], preferred: [-1.0, -1.0], flexible: [-1.0, -1.0] });
        panel.nodes[0].element = Some(trp_data::interface::screen::Element { preferred: stretched, ..element });
    }
    // The weight's figure sized to its words (its `ContentSizeFitter`),
    // its icon at its left.
    if let (Some(mass), Some(font)) = (panel.find(INFO_MASS), look.info_mass_font.as_ref()) {
        if let Some(text) = panel.nodes[mass].text.clone() {
            let width = font.width(&words.weight, text.size, text.spacing, text.style & 1 != 0) + text.margin[0] + text.margin[2];
            panel.nodes[mass].layout.size_delta[0] = width;
        }
    }
    let mut texts = HashMap::new();
    for (path, said_now) in [("Inner/Caption Panel/Caption", &words.caption), ("Inner/Caption Panel/Item Type", &words.kind), (INFO_MASS, &words.weight)] {
        if let Some(index) = panel.find(path) {
            texts.insert(index, said_now.clone());
        }
    }
    // Its tag's plate, coloured and sized to its name (`UpdateTag`,
    // `ResizeTag`: the name's drawn width and 12, at least 40).
    let mut set = HashMap::new();
    let mut colors = HashMap::new();
    if let (Some((name, color)), Some(plate), Some(text)) = (&words.tag, panel.find(INFO_TAG), panel.find(INFO_TAG_TEXT)) {
        set.insert(plate, true);
        colors.insert(plate, tag_colour(*color));
        texts.insert(text, name.clone());
        let drawn = panel.nodes[text].text.as_ref().and_then(|look_of| Some((look_of, look.tmp_faces.get(&(panel.file_of(look_of.font.0)?, look_of.font.1))?))).map_or(0.0, |(text, face)| face.font.width(name, text.size, text.spacing, text.style & 1 != 0));
        panel.nodes[plate].layout.size_delta[0] = (drawn + 12.0).max(40.0);
    } else if let Some(text) = panel.find(INFO_TAG_TEXT) {
        // With no tag its text goes with its plate (the pieces are drawn
        // apart, and the plate's being off does not hide it).
        set.insert(text, false);
    }
    // Its mark of the wish list: the group's icon (`_wishlistIcon`).
    let mut sprites = HashMap::new();
    if let (Some(group), Some(icon)) = (words.wish, panel.find(INFO_WISH)) {
        if let Some(picture) = look.wish_icons.get(group) {
            set.insert(icon, true);
            panel.nodes[icon].active = true;
            // Its row lays it out at its sprite's own size (an `Image`'s
            // preferred size, which the layout here does not ask of it).
            panel.nodes[icon].element = Some(trp_data::interface::screen::Element { ignore: false, min: [-1.0, -1.0], preferred: picture.size, flexible: [-1.0, -1.0] });
            sprites.insert(icon, *picture);
        }
    }
    // Its loader while the model is made (`SetupItemPreview`'s first
    // call), and the picture it is drawn into after.
    if let Some(loader) = panel.find(INFO_LOADER) {
        set.insert(loader, loading);
    }
    let mut tip_nodes = Vec::new();
    if let Some(figures) = figures {
        tip_nodes = info_rows(look, &mut panel, &mut drawn, &mut texts, &mut set, &mut colors, &mut sprites, figures);
    }
    let mut groups = HashMap::new();
    let mut marks = PartsMarks { texts: &mut texts, set: &mut set, colors: &mut colors, sprites: &mut sprites, groups: &mut groups };
    info_mastering(look, &mut panel, &mut marks, parts.mastering.as_ref());
    let (button_tops, grounds): (Vec<usize>, Vec<Option<(usize, [Color; 2], f32)>>) = info_buttons(look, &mut panel, &mut drawn, &mut marks, &parts.buttons).into_iter().unzip();
    let (slot_tops, filters) = info_slots(look, &mut panel, &mut drawn, &mut marks, &parts.slots);
    let size = fitted_size(&panel);
    let rect = |index: usize| panel.rect(index, size);
    let slots = slot_tops.iter().map(|&top| rect(top)).collect();
    let buttons = button_tops.iter().map(|&top| rect(top)).collect();
    let stretch = INFO_STRETCH.iter().filter_map(|(path, sides)| panel.find(path).map(|index| (*sides, rect(index)))).collect();
    let tips = tip_nodes.iter().map(|&(node, row)| (rect(node), row)).collect();
    // The description's bar, hidden while its text fits.
    let scroll = info_scroll(look, &panel, size, &words.description);
    if scroll.is_none() {
        let bar = panel.find(INFO_SCROLL).and_then(|area| trp_data::interface::screen::Scrolling::of(&panel, area, "ScrollRectNoDrag")).and_then(|scrolling| scrolling.bar);
        if let Some(bar) = bar {
            set.insert(bar.object, false);
        }
    }
    // The filters' marks: in their row at the slot's foot (a row from the
    // lower left, 2 apart), each at its own size (`SetNativeSize`).
    let mut icons = Vec::new();
    for (row, pictures) in filters {
        let [x, y, _, h] = rect(row);
        let spacing = panel.nodes[row].group.map_or(2.0, |group| group.spacing);
        let mut left = x;
        for picture in pictures {
            icons.push(([left, y + h - picture.size[1], picture.size[0], picture.size[1]], picture));
            left += picture.size[0] + spacing;
        }
    }
    Some(InfoBuilt { panel, drawn, texts, set, colors, sprites, groups, icons, size, slots, buttons, grounds, stretch, tips, scroll })
}
