//! The right-click menu's looks and layout.

use super::*;

impl HealthLook {
    /// The game's simple tooltip's font, size, colour of its words and of
    /// its frame, where it was read; the app's otherwise.
    pub fn tooltip_words(&self) -> (Handle<Font>, f32, Color, Color) {
        let text = self.tooltip.as_ref().and_then(|panel| panel.nodes.iter().find_map(|node| node.text.as_ref()).map(|text| (panel, text)));
        let (font, size, color) = text.map_or((Handle::default(), 14.0, Color::srgb(0.651, 0.690, 0.710)), |(panel, text)| (self.font(panel, text.font), text.size, rgba(text.color)));
        let frame = self.tooltip.as_ref().and_then(|panel| panel.find("Background/Border")).and_then(|index| self.tooltip.as_ref()?.nodes[index].image.map(|image| rgba(image.color))).unwrap_or(Color::srgb(0.322, 0.349, 0.353));
        (font, size, color, frame)
    }
}

/// A line of the menu of a right click, for `menu_layout`: the line's
/// name (its template and icon go by it), its caption, whether it can be
/// done now, whether the mouse is on it, and whether it has lines of its
/// own (an arrow).
pub struct MenuLineLook<'a> {
    pub name: &'static str,
    pub caption: &'a str,
    pub enabled: bool,
    pub hovered: bool,
    pub arrow: bool,
}

/// The menu of a right click laid out as the game's (`TARKOV UI`): the
/// game's menu with a copy of a template for each line put in what holds
/// the lines, laid out by the game's layout; what of it is drawn and how;
/// its size in units, and each line's rectangle in units from its top
/// left.
pub struct MenuBuilt {
    pub(super) panel: trp_data::interface::screen::ScenePanel,
    pub(super) drawn: Drawn,
    pub(super) words: HashMap<usize, String>,
    pub(super) pictures: HashMap<usize, Picture>,
    pub(super) groups: HashMap<usize, f32>,
    pub(super) colors: HashMap<usize, Color>,
    pub(super) set: HashMap<usize, bool>,
    pub size: [f32; 2],
    pub lines: Vec<[f32; 4]>,
}

/// A line's two colours (`SimpleContextMenuButton._defaultBackgroundColor`,
/// `_selectedBackgroundColor`), from its script's fields: after the four
/// pointers the button and the image take, at 88 and 104.
pub(super) fn menu_line_colors(fields: &[u8]) -> Option<[Color; 2]> {
    let float = |at: usize| fields.get(at..at + 4).map(|word| f32::from_le_bytes(word.try_into().unwrap_or_default()));
    let color = |at: usize| Some(Color::srgba(float(at)?, float(at + 4)?, float(at + 8)?, float(at + 12)?));
    Some([color(88)?, color(104)?])
}

/// The menu laid out (`InteractionButtonsContainer.CreateContextButton`):
/// each line a copy of its template (Discard's own and the wish list's own,
/// `SimpleSpecifiedContextButtons`), its caption sized by the caption's
/// font, its icon at its own size, its arrow only where it has lines of
/// its own (`_subMenuArrow.enabled`), at 0.3 where it cannot be done
/// (`SetUnlockStatus`), and its ground the selected colour under the mouse.
/// None where the game's menu was not read.
pub fn menu_layout(look: &HealthLook, lines: &[MenuLineLook]) -> Option<MenuBuilt> {
    layout_menu(look, look.menu.as_ref()?, &look.menu_drawn, lines)
}

/// The menu of a line's own lines laid out the same way, from its own
/// copy of the templates (`SubMenuTemplate`).
pub fn submenu_layout(look: &HealthLook, lines: &[MenuLineLook]) -> Option<MenuBuilt> {
    layout_menu(look, look.submenu.as_ref()?, &look.submenu_drawn, lines)
}

fn layout_menu(look: &HealthLook, source: &trp_data::interface::screen::ScenePanel, source_drawn: &Drawn, lines: &[MenuLineLook]) -> Option<MenuBuilt> {
    let holder = source.find(MENU_LINES)?;
    let mut panel = source.clone();
    let mut drawn = Drawn { shown: source_drawn.shown.clone(), pictures: source_drawn.pictures.clone() };
    // The templates themselves are not drawn.
    let under = |index: usize, top: usize| {
        let mut at = Some(index);
        while let Some(i) = at {
            if i == top {
                return true;
            }
            at = source.nodes[i].parent;
        }
        false
    };
    let templates: Vec<usize> = MENU_TEMPLATES.iter().filter_map(|path| source.find(path)).collect();
    for index in 0..source.nodes.len() {
        if templates.iter().any(|&top| under(index, top)) {
            drawn.shown[index] = Shown::Never;
        }
    }
    let (mut words, mut pictures, mut groups, mut colors, mut set) = (HashMap::new(), HashMap::new(), HashMap::new(), HashMap::new(), HashMap::new());
    let mut tops = Vec::new();
    for line in lines {
        // (Both menus' `SimpleSpecifiedContextButtons`: the wish list's
        // line and its groups' in the wish list's template, Discard and
        // Remove from Wishlist in Discard's.)
        let path = match line.name {
            "Discard" | "RemoveFromWishlist" => MENU_TEMPLATES[2],
            "AddToWishlist" | "Quests" | "Hideout" | "Trading" | "Equipment" | "Other" => MENU_TEMPLATES[1],
            _ => MENU_TEMPLATES[0],
        };
        let Some(top) = source.find(path) else { continue };
        let copied: Vec<usize> = (0..source.nodes.len()).filter(|&index| under(index, top)).collect();
        let first = panel.nodes.len();
        let new_of = |old: usize| copied.iter().position(|&i| i == old).map(|at| first + at);
        for &old in &copied {
            let mut node = source.nodes[old].clone();
            node.parent = if old == top { Some(holder) } else { node.parent.and_then(new_of) };
            node.active |= old == top;
            panel.nodes.push(node);
            drawn.shown.push(source_drawn.shown.get(old).copied().unwrap_or(Shown::Never));
            drawn.pictures.push(source_drawn.pictures.get(old).copied().flatten());
        }
        let named = |panel: &trp_data::interface::screen::ScenePanel, name: &str| (first..panel.nodes.len()).find(|&index| panel.nodes[index].name == name);
        let copy_top = first;
        if let Some(text_at) = named(&panel, "Text") {
            let caption = line.caption.to_uppercase();
            if let (Some(text), Some(font)) = (panel.nodes[text_at].text.clone(), look.menu_font.as_ref()) {
                let width = font.width(&caption, text.size, text.spacing, text.style & 1 != 0) + text.margin[0] + text.margin[2];
                let flexible = panel.nodes[text_at].element.map_or([1.0, -1.0], |element| element.flexible);
                panel.nodes[text_at].element = Some(trp_data::interface::screen::Element { ignore: false, min: [-1.0, -1.0], preferred: [width, -1.0], flexible });
            }
            words.insert(text_at, caption);
        }
        if let (Some(icon), Some(picture)) = (named(&panel, "Icon"), look.menu_icons.get(line.name)) {
            pictures.insert(icon, *picture);
        }
        if let Some(arrow) = named(&panel, "SubmenuArrow") {
            if line.arrow {
                let size = drawn.pictures.get(arrow).copied().flatten().map_or([9.0, 11.0], |picture| picture.size);
                panel.nodes[arrow].element = Some(trp_data::interface::screen::Element { ignore: false, min: [-1.0, -1.0], preferred: size, flexible: [-1.0, -1.0] });
            } else {
                set.insert(arrow, false);
            }
        }
        // At 0.3 on the button's own group (`SimpleContextMenuButton.
        // _canvasGroup`, its first field after the 40 bytes of the base's
        // `SerializationData`): in Discard's template the button below the
        // divider, which stays whole (`courses10`'s `inspect-window`, frame
        // 2163: REMOVE FROM WISHLIST at 0.3, its divider at 1).
        if !line.enabled {
            let group = source.script(top, "ContextMenuButton").and_then(|fields| source.pointer_at(fields, 40)).and_then(new_of);
            groups.insert(group.unwrap_or(copy_top), 0.3);
        }
        // The ground the line's colours go on: the one beside its icon.
        let icon_holder = named(&panel, "IconPlaceholder").and_then(|index| panel.nodes[index].parent);
        let ground = (first..panel.nodes.len()).find(|&index| panel.nodes[index].name == "Background" && panel.nodes[index].parent == icon_holder);
        let line_colors = panel.script(copy_top, "ContextMenuButton").and_then(menu_line_colors);
        if let (Some(ground), Some([normal, selected])) = (ground, line_colors) {
            colors.insert(ground, if line.hovered { selected } else { normal });
        }
        tops.push(copy_top);
    }
    let size = [panel.asks(0, 0).1, panel.asks(0, 1).1];
    let rects = tops.iter().map(|&top| panel.rect(top, size)).collect();
    Some(MenuBuilt { panel, drawn, words, pictures, groups, colors, set, size, lines: rects })
}

/// Draws the menu as `menu_layout` laid it out, its top left at `origin`
/// in units, `s` pixels to a unit.
/// (`sub`: the menu of a line's own lines, numbered apart from the first.)
pub fn draw_menu(column: &mut ChildSpawnerCommands, look: &HealthLook, said: &Words, built: &MenuBuilt, origin: [f32; 2], s: f32, sub: bool) {
    // A line's icon is only its sprite swapped (`ContextMenuButton.Show`):
    // drawn in the icon's own 16 units square, kept to its shape.
    let marks = Marks { words: Some(&built.words), sprites: Some(&built.pictures), groups: Some(&built.groups), colors: Some(&built.colors), set: Some(&built.set), ..Marks::default() };
    let base = if sub { MENU + (1 << 19) } else { MENU };
    draw_nodes(column, look, &built.panel, &built.drawn, base, marks, said, Frame::Is(built.size), origin, s);
}

impl MenuBuilt {
    /// What of the menu takes the mouse, in units from its top left: its
    /// rectangle with its ground's shadow (`Background/Shadow`, 23 units
    /// out of the ground and a target of the mouse: `courses10`'s
    /// `inspect-window`, frame 205, the mouse on it between the lines).
    pub fn catch(&self) -> [f32; 4] {
        let [w, h] = self.size;
        let Some(shadow) = self.panel.find("InteractionButtonsContainer/Background/Shadow") else { return [0.0, 0.0, w, h] };
        let [x, y, sw, sh] = self.panel.rect(shadow, self.size);
        let (left, top) = (x.min(0.0), y.min(0.0));
        [left, top, (x + sw).max(w) - left, (y + sh).max(h) - top]
    }
}
