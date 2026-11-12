//! Sprites drawn, stretching, mastering looks and tooltip looks.

use super::*;

/// A rectangle of a view (left, top, width, height, units) laid out at one
/// height and how each figure changes with the height (the layout of
/// padding and anchors being linear in it).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stretching {
    pub at: [f32; 4],
    pub per: [f32; 4],
    pub height: f32,
}

impl Stretching {
    pub fn at(&self, height: f32) -> [f32; 4] {
        std::array::from_fn(|n| self.at[n] + self.per[n] * (height - self.height))
    }
}

/// A weapon's mastering in a slot's view (`SlotItemView`: `_masteringIcon`,
/// `_masteringLabel`, `_masteringSlider`): the mark at its own size, its
/// left and its bottom from the view's bottom; the level's words' size,
/// colour and font (`Level`, written right of the mark); and the slider.
#[derive(Clone)]
pub struct MasteringLook {
    pub mark: Sprite,
    pub mark_at: [f32; 2],
    pub size: f32,
    pub color: Color,
    pub font: Handle<Font>,
    pub slider: Option<MasteringSlider>,
}

/// The mastering's slider (`MasteringContainer/MasteringSlider`): its
/// track, its fill laid out empty and full (a share between, between
/// them), and its range (0 to 100, from the bottom up).
#[derive(Clone)]
pub struct MasteringSlider {
    pub track: Sprite,
    pub fill: Sprite,
    pub track_at: Stretching,
    pub empty: Stretching,
    pub full: Stretching,
    pub range: trp_data::interface::screen::SliderRange,
}

impl MasteringSlider {
    /// The fill's rectangle in a view of that height at that share.
    pub fn fill_at(&self, height: f32, share: f32) -> [f32; 4] {
        let (empty, full) = (self.empty.at(height), self.full.at(height));
        std::array::from_fn(|n| empty[n] + (full[n] - empty[n]) * share)
    }
}

/// One of the game's pictures as the inventory draws it by hand: its
/// texture's cut at its sprite's own size in units, how far in a sliced
/// one keeps its edges whole (left, bottom, right, top), and its colour.
#[derive(Clone)]
pub struct Sprite {
    pub image: Handle<Image>,
    pub cut: Rect,
    pub size: [f32; 2],
    pub border: [f32; 4],
    pub color: Color,
}

/// Draws a picture into a node of that place and size, in pixels: sliced
/// where it keeps edges, else stretched, its lowest row first as the engine
/// keeps it and drawn turned up; `s` pixels to a unit.
pub fn draw_sprite(parent: &mut ChildSpawnerCommands, sprite: &Sprite, rect: [f32; 4], s: f32) {
    let [x, y, w, h] = rect;
    let node = Node { position_type: PositionType::Absolute, left: Val::Px(x), top: Val::Px(y), width: Val::Px(w), height: Val::Px(h), ..default() };
    if sprite.border.iter().any(|&edge| edge > 0.0) {
        let slices = Slices { image: sprite.image.clone(), cut: sprite.cut, border: sprite.border, per: s, fill_center: true, color: sprite.color, flip_x: false, tiled: false };
        parent.spawn((node, EXACT)).with_children(|frame| nine_slice(frame, &slices, [w, h], usize::MAX));
    } else {
        parent.spawn((node, EXACT, ImageNode { image: sprite.image.clone(), color: sprite.color, rect: Some(sprite.cut), flip_y: true, ..default() }));
    }
}

/// Draws a picture tiled over a node of that place and size, in pixels,
/// each tile the picture's own size in units (an `Image` of the tiled
/// kind with no border), in that colour.
pub fn draw_sprite_tiled(parent: &mut ChildSpawnerCommands, sprite: &Sprite, rect: [f32; 4], s: f32, color: Color) {
    let [x, y, w, h] = rect;
    let node = Node { position_type: PositionType::Absolute, left: Val::Px(x), top: Val::Px(y), width: Val::Px(w), height: Val::Px(h), ..default() };
    let slices = Slices { image: sprite.image.clone(), cut: sprite.cut, border: [0.0; 4], per: s, fill_center: true, color, flip_x: false, tiled: true };
    parent.spawn((node, EXACT)).with_children(|frame| nine_slice(frame, &slices, [w, h], usize::MAX));
}

/// Draws a sliced picture whose border is kept a pixel of the sprite to a
/// pixel of the screen (`PixelPerfectSpriteScaler`), its middle left out,
/// into a node of that place and size, in pixels.
pub fn draw_sprite_pixels(parent: &mut ChildSpawnerCommands, sprite: &Sprite, rect: [f32; 4]) {
    let [x, y, w, h] = rect;
    let node = Node { position_type: PositionType::Absolute, left: Val::Px(x), top: Val::Px(y), width: Val::Px(w), height: Val::Px(h), ..default() };
    parent.spawn((node, EXACT)).with_children(|frame| nine_slice(frame, &pixel_slices(sprite), [w, h], usize::MAX));
}

/// The pieces `draw_sprite_pixels` draws in a rectangle of that size, each
/// where it lies in the rectangle and its picture: for a frame that is
/// sized again with its pieces kept.
pub fn sprite_pixel_pieces(sprite: &Sprite, size: [f32; 2]) -> Vec<(Node, ImageNode)> {
    slice_pieces(&pixel_slices(sprite), size).into_iter().map(|(.., node, piece)| (node, piece)).collect()
}

fn pixel_slices(sprite: &Sprite) -> Slices {
    Slices { image: sprite.image.clone(), cut: sprite.cut, border: sprite.border, per: 1.0, fill_center: false, color: sprite.color, flip_x: false, tiled: false }
}

/// A picture's piece laid out where the game puts it, to a fraction of a
/// pixel, not rounded to the screen's pixels: rounded one by one, pieces
/// that meet in the game's units fell a pixel apart or over one another
/// at a scale of a third (a slot's header, the middle tiled between two
/// ends, a pixel short of its right end at 2560 by 1440), where the game
/// draws them edge to edge. Texts keep their rounding (sharp).
pub(crate) const EXACT: bevy::ui::LayoutConfig = bevy::ui::LayoutConfig { use_rounding: false };

/// The game's two tooltips' looks (`crate::tooltip`), from their prefabs
/// where they were read (`Preloader UI/UIContext/SimpleTooltip` and
/// `MultiLineTooltip`, `probe scenepanel level49 UIContext/SimpleTooltip`),
/// the app's otherwise.
impl HealthLook {
    /// A tooltip's picture by its object's path, in its own colour, and how
    /// far it lies out of what it hangs from, in pixels (left, top, right,
    /// bottom): its offsets kept a pixel to a unit of them
    /// (`PixelPerfectSpriteScaler.UpdateScale`, all its sides).
    fn tip_picture(&self, panel: Option<&trp_data::interface::screen::ScenePanel>, drawn: &Drawn, path: &str) -> Option<(Sprite, [f32; 4])> {
        let panel = panel?;
        let index = panel.find(path)?;
        let node = &panel.nodes[index];
        let picture = drawn.pictures.get(index).copied().flatten()?;
        let sprite = self.sprite_of(picture, node.image.map_or(Color::WHITE, |image| rgba(image.color)))?;
        let l = &node.layout;
        let (low, high) = ([l.offset[0] - l.size_delta[0] * l.pivot[0], l.offset[1] - l.size_delta[1] * l.pivot[1]], [l.offset[0] + l.size_delta[0] * (1.0 - l.pivot[0]), l.offset[1] + l.size_delta[1] * (1.0 - l.pivot[1])]);
        Some((sprite, [-low[0], high[1], high[0], -low[1]]))
    }

    /// How a tooltip's text is drawn, from the first text under `panel`.
    fn tip_words(&self, panel: Option<&trp_data::interface::screen::ScenePanel>) -> crate::ui::tooltip::TipWords {
        let (font, size, color, _) = self.tooltip_words();
        let text = panel.and_then(|panel| panel.nodes.iter().find_map(|node| node.text.as_ref()).map(|text| (panel, text)));
        crate::ui::tooltip::TipWords {
            font: text.map_or(font, |(panel, text)| self.font(panel, text.font)),
            size: text.map_or(size, |(_, text)| text.size),
            color: text.map_or(color, |(_, text)| rgba(text.color)),
            tmp: text.and_then(|(panel, text)| self.tmp_of(panel, text)),
            spacing: text.map_or(0.0, |(_, text)| text.spacing),
            kerning: text.is_none_or(|(_, text)| text.kerning),
        }
    }

    /// The simple tooltip's look: its words, its ground's padding (the
    /// `Background`'s group: 6 units at the sides, 2 above, 5 below), the
    /// most it is wide (its `LayoutElement`: 300 units), its frame and the
    /// frame's shadow (`Border`, `Border Shadow`, 4 pixels out).
    pub(crate) fn simple_tip_look(&self) -> crate::ui::tooltip::TipLook {
        let panel = self.tooltip.as_ref();
        let (.., frame) = self.tooltip_words();
        let padding = panel.and_then(|panel| panel.nodes[panel.find("Background")?].group).map_or([6.0, 6.0, 2.0, 5.0], |group| group.padding);
        let widest = panel.and_then(|panel| panel.nodes.first()?.element).map(|element| element.preferred[0]).filter(|width| *width > 0.0).unwrap_or(300.0);
        crate::ui::tooltip::TipLook {
            words: self.tip_words(panel),
            padding,
            spacing: 0.0,
            widest,
            border: self.tip_picture(panel, &self.tooltip_drawn, "Background/Border").map(|(sprite, _)| sprite),
            shadow: self.tip_picture(panel, &self.tooltip_drawn, "Background/Border Shadow"),
            frame,
            separator: None,
        }
    }

    /// The tooltip of lines' look: its words, its padding (10 units at the
    /// sides, 5 above and below) and its rows' spacing (3), its frame and
    /// shadow, and a break's line (`MultiLineRow/Line`).
    pub(crate) fn lines_tip_look(&self) -> crate::ui::tooltip::TipLook {
        let panel = self.multi_tooltip.as_ref();
        let (.., frame) = self.tooltip_words();
        let group = panel.and_then(|panel| panel.nodes.first()?.group);
        crate::ui::tooltip::TipLook {
            words: self.tip_words(panel),
            padding: group.map_or([10.0, 10.0, 5.0, 5.0], |group| group.padding),
            spacing: group.map_or(3.0, |group| group.spacing),
            widest: f32::INFINITY,
            border: self.tip_picture(panel, &self.multi_tooltip_drawn, "Border").map(|(sprite, _)| sprite),
            shadow: self.tip_picture(panel, &self.multi_tooltip_drawn, "Border Shadow"),
            frame,
            separator: self.tip_picture(panel, &self.multi_tooltip_drawn, "MultiLineRow/Line").map(|(sprite, _)| sprite),
        }
    }

    /// An armour class's mark (`armor_classes_<class>`), as a tooltip's
    /// line puts it before its words.
    pub(crate) fn class_mark(&self, class: u32) -> Option<Sprite> {
        self.class_marks.get(&class).and_then(|picture| self.sprite_of(*picture, Color::WHITE))
    }
}

/// What a part of the window puts on its pieces, besides their places.
pub(super) struct PartsMarks<'a> {
    pub(super) texts: &'a mut HashMap<usize, String>,
    pub(super) set: &'a mut HashMap<usize, bool>,
    pub(super) colors: &'a mut HashMap<usize, Color>,
    pub(super) sprites: &'a mut HashMap<usize, Picture>,
    pub(super) groups: &'a mut HashMap<usize, f32>,
}

/// An element's preferred size set, its least and its share of what is
/// left kept.
pub(crate) fn prefer_size(panel: &mut trp_data::interface::screen::ScenePanel, index: usize, size: [f32; 2]) {
    let element = panel.nodes[index].element.unwrap_or(trp_data::interface::screen::Element { ignore: false, min: [-1.0, -1.0], preferred: [-1.0, -1.0], flexible: [-1.0, -1.0] });
    panel.nodes[index].element = Some(trp_data::interface::screen::Element { preferred: size, ..element });
}

/// Words cut to what of them fits a width (units) as TextMeshPro cuts a
/// line that does not break (`m_overflowMode`): truncated (3), from the
/// first letter that passes the width (`GenerateTextMesh`: the pen, with
/// the kerning before it where the text keeps it, plus the letter's
/// advance, against the room and a ten-thousandth; so `M-LOK 4.1"` fits a
/// one-cell caption whole only kerned); with an ellipsis (1), as many as
/// leave room for "…", and it; any other mode, or words that fit, as they
/// are. Its margins come off the width.
pub(crate) fn fitted_words(face: &trp_data::interface::tmp_text::TmpFace, text: &trp_data::interface::screen::TextLook, words: &str, width: f32) -> String {
    let room = width + 0.0001 - text.margin[0] - text.margin[2];
    let bold = text.style & 1 != 0;
    let said = if text.style & 16 != 0 { words.to_uppercase() } else { words.to_owned() };
    // TextMeshPro tests a line against its room at its visible letters
    // only, so a trailing space never cuts it.
    let wide = |words: &str| face.line_width(words.trim_end(), text.size, text.spacing, bold, text.kerning);
    if text.wraps || !matches!(text.overflow, 1 | 3) || wide(&said) <= room {
        return said;
    }
    let tail = if text.overflow == 1 { "\u{2026}" } else { "" };
    let mut kept: Vec<char> = said.chars().collect();
    while !kept.is_empty() && wide(&format!("{}{tail}", kept.iter().collect::<String>().trim_end())) > room {
        kept.pop();
    }
    format!("{}{tail}", kept.iter().collect::<String>())
}

/// A text's size as its font draws it, its margins with it: its width
/// with the font's kerning where the text keeps it
/// (`CalculatePreferredValues`), rounded up to a hundredth as
/// `TmpFont::width` rounds it.
pub(crate) fn text_size(look: &HealthLook, panel: &trp_data::interface::screen::ScenePanel, index: usize, said: &str) -> Option<[f32; 2]> {
    let text = panel.nodes[index].text.as_ref()?;
    let face = look.tmp_faces.get(&(panel.file_of(text.font.0)?, text.font.1))?;
    let width = face.line_width(said, text.size, text.spacing, text.style & 1 != 0, text.kerning);
    let width = ((width * 100.0) as i64 + 1) as f32 / 100.0;
    Some([width + text.margin[0] + text.margin[2], face.font.height(text.size) + text.margin[1] + text.margin[3]])
}

/// A copy of one of the panel's own objects and all under it put last
/// under `holder`, the original left undrawn; the place of the copy's top.
pub(crate) fn copy_within(panel: &mut trp_data::interface::screen::ScenePanel, drawn: &mut Drawn, top: usize, holder: usize) -> usize {
    let under = |index: usize, nodes: &[trp_data::interface::screen::PanelNode]| {
        let mut at = Some(index);
        while let Some(here) = at {
            if here == top {
                return true;
            }
            at = nodes[here].parent;
        }
        false
    };
    let copied: Vec<usize> = (0..panel.nodes.len()).filter(|&index| under(index, &panel.nodes)).collect();
    let first = panel.nodes.len();
    let new_of = |old: usize| copied.iter().position(|&i| i == old).map(|at| first + at);
    let holder_path = panel.nodes[holder].path.clone();
    for &old in &copied {
        let mut node = panel.nodes[old].clone();
        node.parent = if old == top { Some(holder) } else { node.parent.and_then(new_of) };
        node.active |= old == top;
        node.path = format!("{holder_path}/#{first}/{}", node.path);
        panel.nodes.push(node);
        drawn.shown.push(drawn.shown.get(old).copied().unwrap_or(Shown::Never));
        drawn.pictures.push(drawn.pictures.get(old).copied().flatten());
    }
    first
}
