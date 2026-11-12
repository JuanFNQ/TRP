//! The item view's, spinner's, wish's and tag plate's looks.

use super::*;

/// A quick-use view's figure (`TARKOV UI`; `QuickSlotItemView.
/// _resourceValue`, `quick_slot_layout`'s `Value`): Bender Outline at 11 in
/// its own red, its box as large as its words ask (`ContentSizeFitter`),
/// anchored at the view's bottom middle, 3 units up, by its own bottom
/// middle, as the prefab has it.
pub struct QuickValueLook {
    pub panel: trp_data::interface::screen::ScenePanel,
    pub value: usize,
    pub tmp: Option<((String, i64), (String, i64))>,
    pub font: Handle<Font>,
    pub effect: Option<TextEffect>,
}

impl QuickValueLook {
    /// The figure's box in a view so large (units: left, top, width,
    /// height from the view's top left), for these words.
    pub fn place(&self, look: &HealthLook, said: &str, view: [f32; 2]) -> Option<[f32; 4]> {
        let [w, h] = text_size(look, &self.panel, self.value, said)?;
        let layout = &self.panel.nodes[self.value].layout;
        let [ax, ay] = layout.anchor_min;
        let (pivot_x, pivot_y) = (ax * view[0] + layout.offset[0], ay * view[1] + layout.offset[1]);
        let (left, bottom) = (pivot_x - layout.pivot[0] * w, pivot_y - layout.pivot[1] * h);
        Some([left, view[1] - bottom - h, w, h])
    }
}

/// The quick-use view's figure's look, from its prefab, where read.
pub(super) fn quick_value_look(look: &HealthLook) -> Option<QuickValueLook> {
    let panel = look.items.as_ref()?.quick.as_ref()?;
    let value = (0..panel.nodes.len()).find(|&index| panel.nodes[index].name == "Value" && panel.nodes[index].text.is_some())?;
    let text = panel.nodes[value].text.as_ref()?;
    Some(QuickValueLook { tmp: look.tmp_of(panel, text), font: look.styled_font(panel, text.font, text.style), effect: look.text_effect(panel, text.font), panel: panel.clone(), value })
}

/// What the game's own view of a thing looks like (`TARKOV UI`), in a slot
/// (`SlotItemView`, `slot_layout`) or in a grid (`GridItemView`,
/// `grid_layout`): its prefab, where its caption and its value lie in it
/// (`Info Panel/Name` or `Info Panel/Caption`, and the bottom right
/// group's `Value`, by their names), their fonts, its border's colour
/// where its border is on, and the label of a thing bound to a key
/// (`BindPanel`: its picture, and its key's words).
pub struct ItemViewLook {
    pub panel: trp_data::interface::screen::ScenePanel,
    pub caption: Option<usize>,
    pub value: Option<usize>,
    pub caption_font: Handle<Font>,
    pub value_font: Handle<Font>,
    pub border: Option<Color>,
    pub bind: Option<BindLook>,
    /// Where a weapon's round is written (`SetInscription`: in a slot the
    /// top right `Info Panel/Info Label`, under the caption; in a grid the
    /// bottom left `Inscription`), and its font.
    pub inscription: Option<usize>,
    pub inscription_font: Handle<Font>,
    /// What the caption's, the value's and the round's fonts draw under
    /// them (the item views' Bender Outline: a dark rim).
    pub caption_effect: Option<TextEffect>,
    pub value_effect: Option<TextEffect>,
    pub inscription_effect: Option<TextEffect>,
    /// The caption's, the round's and the value's font asset and material
    /// as the game draws them (`TmpText`), where had.
    pub caption_tmp: Option<((String, i64), (String, i64))>,
    pub inscription_tmp: Option<((String, i64), (String, i64))>,
    pub value_tmp: Option<((String, i64), (String, i64))>,
    /// The caption's font asset, which measures what of it fits (its
    /// overflow cuts what does not: `m_overflowMode`).
    pub caption_face: Option<std::sync::Arc<trp_data::interface::tmp_text::TmpFace>>,
    /// A weapon's mastering in a slot (`SlotItemView.UpdateInfo`): the
    /// mark, its level's words and the slider of the points into the level.
    pub mastering: Option<MasteringLook>,
    /// A weapon's or a knife's durability in a slot (`DurabilitySlider`):
    /// its track (`Background`, sliced) and its fill (`Fill`).
    pub durability: Option<(Sprite, Sprite)>,
    /// A part's marks (`ItemViewStats.SetItemIcon`): its class's ground by
    /// `type_<class>` (`Mod Types/icon_type_mod_<class>`) and its kind's
    /// mark by `mod_<kind>` (`Mod Tactical Types/icon_mod_<kind>`).
    pub mods: HashMap<String, Sprite>,
    /// A repair buff's mark (`ItemViewBottomPanel.RepairBuffIcon`,
    /// `GridItemView.ChangeRepairBuffStatus`): each buff attribute's icon
    /// (`StaticIcons.GetAttributeIcon`) in the image's colour, and the
    /// spacing of the bottom right column it stands in over the value;
    /// none where the view has no such mark.
    pub buff_icons: HashMap<String, Sprite>,
    pub buff_spacing: Option<f32>,
    /// A tag's plate (`TagPanel`, `_tagColor`) and its name (`TagName`),
    /// with the name's and the caption's font assets as TextMeshPro has
    /// them, which size the plate (`ResizeTag`).
    pub tag: Option<TagPlateLook>,
    /// A thing on the wish list's marks (`WishlistGridView`).
    pub wish: Option<WishLook>,
    /// The load component over a magazine's view while it is worked.
    pub spin: Option<SpinLook>,
    /// The examination's view over a thing's view while it is examined.
    pub examine: Option<ExamineLook>,
    /// The ground of a thing not known (`NotExaminedBackground`: tiled,
    /// under the view's colour panel), its object and its picture.
    pub unexamined: Option<(usize, Sprite)>,
    /// A thing not yet found in a search (`UnsearchedBackground`: tiled
    /// over the whole view, every other part of it off), its object and its
    /// picture in its colour.
    pub unsearched: Option<(usize, Sprite)>,
    /// Its loader, shown while its picture is still to come (`ItemView.
    /// _iconLoader`: `Small Loader`, kept its own size in the view's
    /// middle), that size, and the share of it its picture's cut covers.
    pub loader: Option<(Sprite, [f32; 2], [f32; 4])>,
    /// Its loader while the thing is being used (`ItemView._drainLoader`:
    /// `Loader`, the same way), that size, and the share its cut covers.
    pub drain: Option<(Sprite, [f32; 2], [f32; 4])>,
    /// Its own frame, where its prefab has one (a grid's view, the edit
    /// build screen's parts; a slot's view has none).
    pub frame: Option<ViewFrame>,
}

/// A thing's view framed as the game frames it (`grid_layout`,
/// `modding_selectable_layout`): the view as large as its cells and one
/// unit more, lying over its cells' lines (`GridView.SetItemViewPosition`,
/// `ItemViewFactory.GetCellPixelSize`); its ground (`Color Panel`) that
/// many units inside its edges (left, top, right, bottom: 1 all round,
/// so the ground stops at the cells' lines); and its border (`Border`,
/// `border_generic`: a line a pixel wide round a picture of 16, sliced 2
/// in) over the whole view, a pixel of its sprite to a pixel of the
/// screen (`PixelPerfectSpriteScaler`), over the picture and under the
/// words.
pub struct ViewFrame {
    pub inset: [f32; 4],
    pub border: Sprite,
}

/// The load component (`ItemViewLoadAmmoComponent`; inventory spec, "The
/// load component") as `ItemViewLook::spin` has it: its panel, laid out
/// over a view, and its pieces, each its object and its picture: the
/// spinner's back and fill, the loader, the stop cross.
pub struct SpinLook {
    pub panel: trp_data::interface::screen::ScenePanel,
    pub back: Option<(usize, SpinPicture)>,
    pub fill: Option<(usize, SpinPicture)>,
    pub loader: Option<(usize, SpinPicture)>,
    pub stop: Option<(usize, SpinPicture)>,
}

/// The examination's view (`ItemViewExamineComponent`; inventory spec,
/// "Examining"): its panel, laid out over a view, its bar's ground and fill
/// and its loader, each its object and its picture, and its label's object,
/// size, colour and font (the percent done).
pub struct ExamineLook {
    pub panel: trp_data::interface::screen::ScenePanel,
    pub back: Option<(usize, Sprite)>,
    pub fill: Option<(usize, Sprite)>,
    pub loader: Option<(usize, Sprite)>,
    pub label: Option<(usize, f32, Color, Handle<Font>)>,
}

/// A piece's picture: its texture, its cut and the share of the piece's
/// rectangle the cut covers (a trimmed sprite).
#[derive(Clone)]
pub struct SpinPicture {
    pub image: Handle<Image>,
    pub cut: Rect,
    pub(super) covers: [f32; 4],
}

impl SpinLook {
    /// A piece's picture's rectangle over a view of that size, in units.
    pub fn rect(&self, index: usize, picture: &SpinPicture, view: [f32; 2]) -> [f32; 4] {
        let [x, y, w, h] = self.panel.placed(index, view).rect;
        let c = picture.covers;
        [x + w * c[0], y + h * (1.0 - c[3]), w * (c[2] - c[0]), h * (c[3] - c[1])]
    }
}

/// A view's marks of the wish list as `ItemViewLook::wish` has them: the
/// glow over it and its border's edges, each its object and its picture;
/// its icon's object, the column it is the first of
/// (`BottomLeftLayoutGroup`), and what the column asks for a part's marks
/// (`ModIconContainer`) and, where the weapon's round is written in it, for
/// that line (TextMeshPro's height of it); and the groups' outlined icons.
pub struct WishLook {
    pub glow: Option<(usize, Sprite)>,
    pub edges: Vec<(usize, Sprite)>,
    pub icon: usize,
    pub column: usize,
    pub mod_height: f32,
    pub inscription_height: Option<f32>,
    pub icons: HashMap<&'static str, Sprite>,
}

/// A view's tag plate as `ItemViewLook::tag` has it.
pub struct TagPlateLook {
    pub plate: usize,
    pub name: usize,
    pub name_tmp: Option<((String, i64), (String, i64))>,
    pub name_font: Handle<Font>,
    pub name_face: Option<std::sync::Arc<trp_data::interface::tmp_text::TmpFace>>,
    pub caption_face: Option<std::sync::Arc<trp_data::interface::tmp_text::TmpFace>>,
}

impl TagPlateLook {
    /// The plate's width in units and whether its name shows
    /// (`GridItemView.ResizeTag`): where the view's width less the
    /// caption's drawn width and 2 leaves less than 40 the plate is the
    /// view's width with no name, else the name's width and 12, from 40 to
    /// that room.
    pub fn width(&self, panel: &trp_data::interface::screen::ScenePanel, caption: Option<usize>, caption_words: &str, name: &str, view_width: f32) -> (f32, bool) {
        let drawn = |face: &Option<std::sync::Arc<trp_data::interface::tmp_text::TmpFace>>, index: Option<usize>, words: &str| {
            let text = index.and_then(|index| panel.nodes[index].text.as_ref());
            face.as_ref().zip(text).map_or(0.0, |(face, text)| face.line_width(words, text.size, text.spacing, text.style & 1 != 0, text.kerning))
        };
        let room = view_width - drawn(&self.caption_face, caption, caption_words) - 2.0;
        if room < 40.0 {
            return (view_width, false);
        }
        ((drawn(&self.name_face, Some(self.name), name) + 12.0).clamp(40.0, room), true)
    }
}

/// A part's class and the name of its ground's picture
/// (`ItemViewFactory.GetModClass`, `GetModClassIconName`), by the
/// database's names of the game's classes, in the game's order of asking.
pub const MOD_CLASSES: [(&str, &str); 5] = [("FunctionalMod", "generic"), ("GearMod", "gear"), ("MasterMod", "vital"), ("AuxiliaryMod", "gear"), ("ArmorPlate", "armor_plate")];

/// A part's kind and the name of its mark's picture (`GetModSubclass`,
/// `GetModIconName`), the same way: the first the part is of wins.
pub const MOD_KINDS: [(&str, &str); 21] = [
    ("Bipod", "bipod"),
    ("IronSight", "ironsight"),
    ("Launcher", "launcher"),
    ("LightLaser", "lightlaser"),
    ("Mount", "mount"),
    ("Muzzle", "muzzle"),
    ("RailCovers", "railcovers"),
    ("Sights", "sight"),
    ("TacticalCombo", "tactical"),
    ("Foregrip", "tactical"),
    ("Magazine", "magazine"),
    ("Gasblock", "gasblock"),
    ("Handguard", "handguard"),
    ("Receiver", "reciever"),
    ("PistolGrip", "pistol_grip"),
    ("Flashlight", "lightlaser"),
    ("AuxiliaryMod", "aux"),
    ("Charge", "charge"),
    ("Stock", "stock"),
    ("Barrel", "barrel"),
    // (A plate, or a built-in insert: `ArmorPlate`, the game's last.)
    ("ArmorPlate", "armor_plate"),
];
