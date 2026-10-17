//! Health on the screen and in the world (`docs/spec/health.md`): the
//! inventory's HEALTH tab, the death screen and the run begun again, and
//! the range's barbed wire cutting the body as the game's does.
//!
//! The HEALTH tab is the game's own health panel
//! (`InventoryScreenHealthPanel`) as the install's scene lays it out and
//! pictures it (`trp_data::screen::HealthPanel`): drawn in the inventory's
//! left panel where the GEAR tab has the equipment panel, the containers
//! beside it as on that tab (`ItemsPanel.UpdateEquipment`), and each of its
//! objects kept as the game's scripts keep it (`BodyPartView`,
//! `HealthParametersPanel`, `HealthParameterPanel`). The words are the
//! install's where it has them (`locales/global/en.json`), the app's plain
//! English where it has none.

use std::collections::HashMap;

use bevy::prelude::*;
use bevy::ui::widget::NodeImageMode;
use bevy::ui::RelativeCursorPosition;
use trp_sim::health::{BarbTally, DamageKind, Health, Kind, Part};

use crate::body_view::{BodyParts, BodyRig};
use crate::controller::Controller;
use crate::inventory_view::{words, FAINT, WORDS};
use crate::range::Range;

pub struct HealthViewPlugin;

impl Plugin for HealthViewPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (spawn_death_screen, make_pictures, share_item_looks.after(make_pictures)))
            .add_systems(Update, (fill_panel, fill_own, fill_effect_icons, show_effect_tooltip, light_drop_glow).after(crate::inventory_view::ScreenDrawn))
            .add_systems(Update, (show_death, damage_asked, damage_keys.after(crate::inventory_view::ScreenDrawn), show_version))
            .add_systems(Update, barbed_wire.after(crate::controller::step))
            // Texts drawn as the game draws them, as they are made or
            // changed, before the screen is laid out.
            .add_systems(PostUpdate, draw_tmp_texts.before(bevy::ui::UiSystems::Layout));
    }
}

/// The install's words for what the page and the death screen show.
#[derive(Resource, Default)]
pub struct Words(HashMap<String, String>);

impl Words {
    /// The words wanted, and those of the health panel's own texts
    /// (`extra`: a header's text is the key its words are found by,
    /// `LocalizedText`).
    pub fn load(install: &trp_data::Install, extra: &[String]) -> Self {
        let mut wanted: Vec<String> = Part::ALL.iter().map(|part| part.name().to_owned()).collect();
        let kinds = [Kind::LightBleeding, Kind::HeavyBleeding, Kind::Wound, Kind::Pain, Kind::Tremor, Kind::Dehydration, Kind::Exhaustion, Kind::ChronicFatigue];
        wanted.extend(kinds.iter().map(|kind| kind.name().to_owned()));
        wanted.extend(["Painkiller", "BrokenBone", "Encumbered", "OverEncumbered", "Killed in Action", "Killed by"].map(str::to_owned));
        wanted.extend(FIGURES.iter().map(|(_, key, _)| (*key).to_owned()));
        let causes = [
            DamageKind::Undefined,
            DamageKind::Fall,
            DamageKind::Barbed,
            DamageKind::Existence,
            DamageKind::Medicine,
            DamageKind::Bullet,
            DamageKind::Melee,
            DamageKind::Blunt,
            DamageKind::GrenadeFragment,
            DamageKind::Explosion,
            DamageKind::LightBleeding,
            DamageKind::HeavyBleeding,
            DamageKind::Dehydration,
            DamageKind::Exhaustion,
        ];
        wanted.extend(causes.iter().map(|kind| format!("DamageType_{}", kind.name())));
        wanted.push(SPECIAL_SLOTS_HEADER.to_owned());
        // The menu of a right click: its lines' captions and its reasons.
        wanted.extend(trp_sim::item_menu::Line::ALL.iter().map(|line| line.name().to_owned()));
        wanted.extend(trp_sim::item_menu::REASONS.iter().map(|key| (*key).to_owned()));
        wanted.extend(extra.iter().cloned());
        let names: Vec<&str> = wanted.iter().map(String::as_str).collect();
        match trp_data::inventory::texts(install, "en", &names) {
            Ok(texts) => Self(texts),
            Err(e) => {
                eprintln!("the install's texts could not be read; the health screen is in the app's words: {e:#}");
                Self::default()
            }
        }
    }

    /// The install's words for a name, found whatever its case, or the
    /// app's.
    fn say(&self, key: &str, otherwise: &str) -> String {
        self.0.get(&key.to_lowercase()).filter(|said| !said.is_empty()).cloned().unwrap_or_else(|| otherwise.to_owned())
    }

    /// The same, for the rest of the app (the menu of a right click).
    pub fn word(&self, key: &str, otherwise: &str) -> String {
        self.say(key, otherwise)
    }

    /// An effect's name as the game's screens give it (`HealthHelper.
    /// EffectName`: a fracture is a `BrokenBone`); low health, medicine at
    /// work and hunger have none there, and are not listed.
    fn effect(&self, kind: Kind) -> Option<String> {
        let (key, otherwise) = match kind {
            Kind::LowHealth | Kind::MedEffect | Kind::Existence => return None,
            Kind::PainKiller => ("Painkiller", "On painkillers"),
            Kind::Fracture => ("BrokenBone", "Fracture"),
            Kind::Encumbered => ("Encumbered", "Overweight"),
            Kind::OverEncumbered => ("OverEncumbered", "Critical overweight"),
            other => (other.name(), other.name()),
        };
        Some(self.say(key, otherwise))
    }

    fn cause(&self, kind: DamageKind) -> String {
        self.say(&format!("DamageType_{}", kind.name()), kind.name())
    }

    /// A part's name in the install's words (a chest is a `THORAX`).
    fn part(&self, part: Part) -> String {
        let otherwise = match part {
            Part::Head => "HEAD",
            Part::Chest => "THORAX",
            Part::Stomach => "STOMACH",
            Part::LeftArm => "LEFT ARM",
            Part::RightArm => "RIGHT ARM",
            Part::LeftLeg => "LEFT LEG",
            Part::RightLeg => "RIGHT LEG",
        };
        self.say(part.name(), otherwise).to_uppercase()
    }
}

/// The game's own panels of the inventory's left panel and their pictures:
/// the health panel (the HEALTH tab), and with `TARKOV UI` the GEAR tab's
/// strip of figures and the left panel's foot under both.
#[derive(Resource, Default)]
pub struct HealthLook {
    /// Whether the game's own panels are drawn (`TARKOV UI`), as the run
    /// began.
    pub tarkov_ui: bool,
    /// The game's line of its version at the screen's foot, over every
    /// screen (`trp_data::screen::version_label`), and how high over the
    /// foot it lies.
    version: Option<(trp_data::screen::ScenePanel, f32)>,
    pub panel: Option<trp_data::screen::HealthPanel>,
    /// Each panel's objects as drawn: the health panel's, the strip's and
    /// the foot's.
    health: Drawn,
    strip: Drawn,
    foot: Drawn,
    gear: Drawn,
    border: Drawn,
    /// The textures the pictures are cut from: read at the start, made
    /// images by `make_pictures`.
    textures: Vec<Option<trp_data::unity::extract::TextureData>>,
    pub(crate) images: Vec<Option<Handle<Image>>>,
    /// The views' glow as their button's animator holds it standing (its
    /// `Normal` clip: the alpha of `BackGlowImage`), lit only as a thing
    /// that can be used on the part is held over it (`Highlighted`).
    glow_alpha: Option<f32>,
    /// The glow lit as a thing that can be used on the part is carried over
    /// it (`HealthBarButton.OnPointerEnter`: the animator's `Highlighted`,
    /// its glow's alpha from 0 to 1 in a third of a second): the clip and
    /// its curve of the glow's alpha.
    glow_lit: Option<(trp_data::unity::anim::Clip, usize)>,
    /// A rate's arrows while the figure moves (their animator's
    /// `BuffPaneActive`, looping): the clip, and its curves of the arrow's
    /// height and alpha. Standing (`BuffPanePassive`), they are still and
    /// whole.
    bob: Option<(trp_data::unity::anim::Clip, usize, usize)>,
    /// The carried weight's two limits the weight's figure is coloured by
    /// (`Stamina.LowerOverweightLimit`, `UpperOverweightLimit`).
    weight_limits: Option<[f32; 2]>,
    /// The game's own fonts its panels' texts are drawn in, by their font
    /// asset (the file it is in among the scene's, and its number): read
    /// at the start, made fonts by `make_pictures`.
    font_data: Vec<((String, i64), Vec<u8>)>,
    fonts: HashMap<(String, i64), Handle<Font>>,
    /// The game's views of a thing in a slot and in a grid (`TARKOV UI`)
    /// and their pictures; made, at the start, what the inventory draws a
    /// thing by (`item_looks`).
    items: Option<trp_data::screen::ItemViews>,
    slot_item: Drawn,
    grid_item: Drawn,
    pub item_looks: [Option<std::sync::Arc<ItemViewLook>>; 2],
    /// The containers' column (`TARKOV UI`): the scene's panel and its
    /// pictures, a grid view's, each container slot's empty picture
    /// (`Slots/Equipment/<slot>`), and containers' own layouts of their
    /// grids by name, each with its size and its grids' rectangles.
    pub containers: Option<trp_data::screen::ContainerViews>,
    containers_drawn: Drawn,
    grid_drawn: Drawn,
    container_backs: HashMap<String, Picture>,
    /// The parts' marks by their keys (`ItemViewLook::mods`).
    mod_pictures: HashMap<String, Picture>,
    /// The health's effects' icons by the game's names
    /// (`trp_data::screen::EFFECT_ICONS`), and the game's simple tooltip,
    /// which says what one is with the mouse on it.
    pub(crate) effect_pictures: HashMap<&'static str, Picture>,
    tooltip: Option<trp_data::screen::ScenePanel>,
    /// The battle screen's panels of the character (`TARKOV UI`): the
    /// stance panel and the health panel as drawn, and the sprites their
    /// scripts put on them (the stamina's fills, the noise's levels), by
    /// the stance panel's pointer to each.
    pub(crate) battle: Option<trp_data::screen::BattleViews>,
    pub(crate) stance_drawn: Drawn,
    pub(crate) body_drawn: Drawn,
    /// The battle screen's quick bar (`QuickViews::load_battle`) as drawn.
    pub(crate) battle_quick: Option<trp_data::screen::QuickViews>,
    pub(crate) battle_quick_drawn: Drawn,
    pub(crate) battle_sprites: HashMap<(i32, i64), Picture>,
    /// The game's own settings of when they show and how the parts are
    /// coloured.
    pub(crate) battle_modes: crate::battle_view::BattleModes,
    /// What each font asset's material draws under its letters (a rim, a
    /// drop shadow), by the asset's file and number.
    text_effects: HashMap<(String, i64), TextEffect>,
    pub layouts: HashMap<String, ([f32; 2], Vec<[f32; 4]>)>,
    /// The bar of tabs (`TARKOV UI`): the scene's bar with its tabs laid
    /// out at their width (`TabBar::lay_out`), its pictures, each tab's
    /// words, the tabs' width, and each tab's masked fill by its mask's
    /// object (`MaskedFill`).
    pub tab_bar: Option<trp_data::screen::TabBar>,
    tabs_drawn: Drawn,
    /// The screen's back button as it is drawn at rest.
    back_drawn: Drawn,
    pub tab_words: Vec<String>,
    pub tab_width: f32,
    pub tab_fills: HashMap<usize, std::sync::Arc<MaskedFill>>,
    /// The panel for a container opened (`TARKOV UI`), its pictures, its
    /// masked stripes, and the font asset of the container's name, which
    /// sizes the name's box.
    pub stash: Option<trp_data::screen::StashViews>,
    stash_drawn: Drawn,
    pub stash_fills: HashMap<usize, std::sync::Arc<MaskedFill>>,
    name_font: Option<trp_data::screen::TmpFont>,
    /// The quick-use panel (`TARKOV UI`), its views on as its script shows
    /// them, and its pictures.
    pub quick: Option<trp_data::screen::QuickViews>,
    quick_drawn: Drawn,
    /// A font asset's bold typeface, where it has one (its weight table's
    /// 700), which TextMeshPro draws its bold in.
    bold_fonts: HashMap<(String, i64), (String, i64)>,
    /// The window that shows a thing (`TARKOV UI`; `InfoWindow`), its
    /// pictures, and the font asset of its weight's figure, which sizes
    /// the figure's box (`ContentSizeFitter`).
    pub(crate) info: Option<trp_data::screen::ScenePanel>,
    info_drawn: Drawn,
    info_mass_font: Option<trp_data::screen::TmpFont>,
    /// The menu of a right click (`TARKOV UI`; `ItemContextMenu`), its
    /// pictures with its templates of a line on, the font asset of a
    /// line's caption, which sizes the line, and each line's icon by the
    /// line's name (`Characteristics/Icons/<line>`).
    pub(crate) menu: Option<trp_data::screen::ScenePanel>,
    menu_drawn: Drawn,
    menu_font: Option<trp_data::screen::TmpFont>,
    menu_icons: HashMap<&'static str, Picture>,
    /// The font assets its texts are drawn from as the game draws them
    /// (`trp_data::tmp_text`), each with its atlas and its bold typeface's,
    /// by the asset's file and number; the materials they are drawn with,
    /// by theirs; and each asset's own material.
    pub(crate) tmp_faces: HashMap<(String, i64), std::sync::Arc<trp_data::tmp_text::TmpFace>>,
    pub(crate) tmp_materials: HashMap<(String, i64), trp_data::tmp_text::TmpMaterial>,
    tmp_default_material: HashMap<(String, i64), (String, i64)>,
}

/// A texture's pixels as the engine keeps them (RGBA, its lowest row
/// first), and whether it is sampled at its nearest pixel.
pub struct Pixels {
    rgba: Vec<u8>,
    width: u32,
    height: u32,
    point: bool,
}

/// A fill cut to its mask's shape, as the game's tabs are drawn: the mask's
/// picture sliced to the object (`FillingMask`, a `Mask` that does not
/// show its own picture) and the fill's picture tiled across it from its
/// lower left at its own size (`Filling`, `Image.Type.Tiled`); what of the
/// fill lies where the mask's alpha is above none is drawn
/// (`UNITY_UI_ALPHACLIP`, which a mask's stencil is written with), in the
/// fill's colour. Each picture with its texture's pixels and its units to
/// a pixel of the sprite (one over its image's multiplier).
pub struct MaskedFill {
    mask: (std::sync::Arc<Pixels>, Picture, f32),
    fill: (std::sync::Arc<Pixels>, Picture, f32),
    color: [f32; 4],
}

impl MaskedFill {
    /// Its pixels for an object so large (units) drawn so many pixels
    /// across and down: RGBA in sRGB, its top row first.
    pub fn compose(&self, size: [f32; 2], pixels: [u32; 2]) -> Vec<u8> {
        let [w, h] = size;
        let (mask, cut, per) = (&self.mask.0, &self.mask.1, self.mask.2);
        // The sliced mask's columns and rows (`Image.GenerateSlicedSprite`):
        // its transparent margins trimmed, its border kept whole, from the
        // object's left and bottom, in units, and their texels.
        let [l, b, r, t] = cut.border;
        let [c0, c1, c2, c3] = cut.covers;
        let pad = [c0 * cut.size[0], c1 * cut.size[1], (1.0 - c2) * cut.size[0], (1.0 - c3) * cut.size[1]];
        let xs = [pad[0] * per, l * per, w - r * per, w - pad[2] * per];
        let ys = [pad[1] * per, b * per, h - t * per, h - pad[3] * per];
        let us = [cut.cut.min.x, cut.cut.min.x + (l - pad[0]), cut.cut.max.x - (r - pad[2]), cut.cut.max.x];
        let vs = [cut.cut.min.y, cut.cut.min.y + (b - pad[1]), cut.cut.max.y - (t - pad[3]), cut.cut.max.y];
        let along = |at: f32, places: [f32; 4], texels: [f32; 4]| -> Option<f32> {
            if at < places[0] || at > places[3] {
                return None;
            }
            let span = if at < places[1] { 0 } else if at < places[2] { 1 } else { 2 };
            let (from, to) = (places[span], places[span + 1]);
            let share = if to > from { (at - from) / (to - from) } else { 0.0 };
            Some(texels[span] + share * (texels[span + 1] - texels[span]))
        };
        let (fill, fill_cut, fill_per) = (&self.fill.0, &self.fill.1, self.fill.2);
        let tile = [fill_cut.size[0] * fill_per, fill_cut.size[1] * fill_per];
        let mut out = vec![0u8; (pixels[0] * pixels[1] * 4) as usize];
        for row in 0..pixels[1] {
            // From the bottom, in units, at the pixel's middle.
            let y = h - (row as f32 + 0.5) * h / pixels[1] as f32;
            for column in 0..pixels[0] {
                let x = (column as f32 + 0.5) * w / pixels[0] as f32;
                let (Some(u), Some(v)) = (along(x, xs, us), along(y, ys, vs)) else { continue };
                if sample(mask, u, v)[3] <= 0.001 {
                    continue;
                }
                let fu = fill_cut.cut.min.x + (x % tile[0]) / tile[0] * fill_cut.cut.width();
                let fv = fill_cut.cut.min.y + (y % tile[1]) / tile[1] * fill_cut.cut.height();
                let colour = sample(fill, fu, fv);
                let at = ((row * pixels[0] + column) * 4) as usize;
                for channel in 0..3 {
                    out[at + channel] = to_srgb(colour[channel] * self.color[channel]);
                }
                out[at + 3] = ((colour[3] * self.color[3]).clamp(0.0, 1.0) * 255.0).round() as u8;
            }
        }
        out
    }
}

/// Each mask of a panel over a picture hanging from it (a `Mask` with a
/// child that draws), as that picture cut to the mask's shape
/// (`MaskedFill`), by the mask's object; the textures' pixels decoded
/// once (`decoded`, by their place among those kept).
fn masked_fills(panel: &trp_data::screen::ScenePanel, drawn: &Drawn, kept: &[Option<trp_data::unity::extract::TextureData>], decoded: &mut HashMap<usize, std::sync::Arc<Pixels>>) -> HashMap<usize, std::sync::Arc<MaskedFill>> {
    let mut pixels_of = |texture: usize| -> Option<std::sync::Arc<Pixels>> {
        if let Some(pixels) = decoded.get(&texture) {
            return Some(pixels.clone());
        }
        let data = kept.get(texture)?.as_ref()?;
        let level = *trp_data::unity::texture::mips(data).ok()?.first()?;
        let rgba = trp_data::unity::texture::decode_rgba(data, level).ok()?;
        let pixels = std::sync::Arc::new(Pixels { rgba, width: data.width, height: data.height, point: data.point });
        decoded.insert(texture, pixels.clone());
        Some(pixels)
    };
    let mut fills = HashMap::new();
    for (mask, node) in panel.nodes.iter().enumerate().filter(|(_, node)| node.scripts.iter().any(|(class, _)| class == "Mask")) {
        let Some(filling) = (0..panel.nodes.len()).find(|&index| panel.nodes[index].parent == Some(mask) && panel.nodes[index].image.is_some()) else { continue };
        let (Some(Some(mask_picture)), Some(Some(fill_picture))) = (drawn.pictures.get(mask).copied(), drawn.pictures.get(filling).copied()) else { continue };
        let (Some(mask_image), Some(fill_image)) = (node.image, panel.nodes[filling].image) else { continue };
        let (Some(mask_pixels), Some(fill_pixels)) = (pixels_of(mask_picture.texture), pixels_of(fill_picture.texture)) else { continue };
        let fill = MaskedFill {
            mask: (mask_pixels, mask_picture, 1.0 / mask_image.pixels_per_unit.max(1e-3)),
            fill: (fill_pixels, fill_picture, 1.0 / fill_image.pixels_per_unit.max(1e-3)),
            color: fill_image.color,
        };
        fills.insert(mask, std::sync::Arc::new(fill));
    }
    fills
}

/// A texture's colour at a place (texels from its lower left), filtered as
/// the graphics card filters an sRGB texture (in linear light, between
/// the four nearest texels, or the nearest alone where it is so set),
/// clamped at its edges: linear RGB and alpha.
fn sample(pixels: &Pixels, u: f32, v: f32) -> [f32; 4] {
    let (w, h) = (pixels.width as i64, pixels.height as i64);
    let texel = |x: i64, y: i64| -> [f32; 4] {
        let at = ((y.clamp(0, h - 1) * w + x.clamp(0, w - 1)) * 4) as usize;
        let p = &pixels.rgba[at..at + 4];
        [to_linear(p[0]), to_linear(p[1]), to_linear(p[2]), p[3] as f32 / 255.0]
    };
    if pixels.point {
        return texel(u.floor() as i64, v.floor() as i64);
    }
    let (x, y) = (u - 0.5, v - 0.5);
    let (x0, y0) = (x.floor(), y.floor());
    let (fx, fy) = (x - x0, y - y0);
    let (x0, y0) = (x0 as i64, y0 as i64);
    let corners = [texel(x0, y0), texel(x0 + 1, y0), texel(x0, y0 + 1), texel(x0 + 1, y0 + 1)];
    std::array::from_fn(|c| (corners[0][c] * (1.0 - fx) + corners[1][c] * fx) * (1.0 - fy) + (corners[2][c] * (1.0 - fx) + corners[3][c] * fx) * fy)
}

fn to_linear(value: u8) -> f32 {
    let c = value as f32 / 255.0;
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

fn to_srgb(value: f32) -> u8 {
    let c = value.clamp(0.0, 1.0);
    let c = if c <= 0.0031308 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 };
    (c * 255.0).round() as u8
}

/// One of the game's panels as drawn: whether each object shows, and its
/// picture, by its place in the panel.
#[derive(Default)]
pub(crate) struct Drawn {
    pub(crate) shown: Vec<Shown>,
    pub(crate) pictures: Vec<Option<Picture>>,
}

/// Where each panel's objects are numbered among the drawn pieces: the
/// health panel's from 0, the strip's and the foot's past it.
const STRIP: usize = 1 << 20;
const FOOT: usize = 2 << 20;
const GEAR: usize = 3 << 20;
const CONTAINERS: usize = 4 << 20;
const GRID: usize = 5 << 20;
const BORDER: usize = 6 << 20;
const TABS: usize = 7 << 20;
const STASH: usize = 8 << 20;
const QUICK: usize = 9 << 20;
const BACK_BUTTON: usize = 10 << 20;
/// The windows that show a thing, each window's past the one before it.
const INFO: usize = 14 << 20;
/// The menu of a right click.
const MENU: usize = 15 << 20;
/// The menu's lines' templates below its top (an ordinary line's, the wish
/// list's, Discard's: `SimpleSpecifiedContextButtons`), and what holds the
/// lines.
const MENU_TEMPLATES: [&str; 3] = ["InteractionButtonsContainer/Button Template", "InteractionButtonsContainer/Wishlist Template", "InteractionButtonsContainer/Dispose Template"];
const MENU_LINES: &str = "InteractionButtonsContainer";

/// Whether an object of the panel is drawn: never (it, or something it
/// hangs from, is off and nothing turns it on in a raid), always, or as a
/// script of the panel's has it (a destroyed part's cross, a figure's
/// glows, a rate's figure and arrows).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Shown {
    Never,
    Always,
    ByScript,
}

#[derive(Clone, Copy)]
pub(crate) struct Picture {
    pub(crate) texture: usize,
    /// The cut, in the texture's pixels from its first row, which is its
    /// lowest: the texture is kept as the engine has it and drawn turned
    /// up.
    pub(crate) cut: Rect,
    /// The share of the object's rectangle that the cut covers
    /// (`SpriteCut::covers`).
    covers: [f32; 4],
    /// How far in from each side a sliced image keeps the sprite whole
    /// (left, bottom, right, top), in its pixels.
    border: [f32; 4],
    /// The sprite's own size, untrimmed, a unit to a pixel.
    size: [f32; 2],
}

/// The carried weight's limits as the inventory's figures go by them
/// (`GlobalConfiguration.Stamina`): the least start of the four
/// overweight limits, and the base limit's end; the Strength skill's
/// share of them is not had, as the movement has none.
pub fn weight_limits(config: &serde_json::Value) -> Option<[f32; 2]> {
    let stamina = config.get("Stamina")?;
    let at = |name: &str, axis: &str| stamina.get(name)?.get(axis)?.as_f64().map(|v| v as f32);
    let lower = ["WalkOverweightLimits", "BaseOverweightLimits", "SprintOverweightLimits", "WalkSpeedOverweightLimits"].iter().map(|name| at(name, "x")).collect::<Option<Vec<f32>>>()?;
    Some([lower.into_iter().fold(f32::MAX, f32::min), at("BaseOverweightLimits", "y")?])
}

impl HealthLook {
    /// Reads the panels from the scene file that holds the inventory
    /// screen (`file`), and with `TARKOV UI` on (`pictures`) the pictures
    /// their objects draw and the clips their animators play; off, the
    /// health panel's layout alone, where the app's own look puts its
    /// boxes.
    pub fn read(install: &trp_data::Install, file: &str, pictures: bool, weight_limits: Option<[f32; 2]>, layouts: &[String]) -> Self {
        let panel = match trp_data::screen::HealthPanel::load(install, file) {
            Ok(panel) => panel,
            Err(e) => {
                eprintln!("the install's health panel could not be read; the HEALTH tab shows none of it: {e:#}");
                return Self { tarkov_ui: pictures, weight_limits, ..Self::default() };
            }
        };
        let mut look = Self { tarkov_ui: pictures, weight_limits, ..Self::default() };
        look.health.shown = health_shown(&panel);
        if let Some(strip) = &panel.strip {
            let scripted: Vec<usize> = panel.strip_figures.iter().filter_map(|(_, f)| f.as_ref()).flat_map(|f| [f.ok_glow, f.glow, f.up, f.down, f.rate]).flatten().collect();
            look.strip.shown = shown_of(&strip.nodes, &[], &[], &scripted);
        }
        if let Some(foot) = &panel.foot {
            look.foot.shown = shown_of(&foot.nodes, &[], &[], &[]);
        }
        if let Some(border) = &panel.border {
            look.border.shown = shown_of(&border.nodes, &[], &[], &[]);
        }
        if let Some(gear) = &panel.gear_panel {
            // Each slot's borders, fill and picture as it is full or empty
            // (`SlotView.SetSlotGraphics`); its header's arrow only out of
            // a raid (`SlotViewHeader.Show`, the header not clickable).
            let scripted: Vec<usize> = panel.slot_views.iter().flat_map(|(_, p)| [p.full_border, p.back_image, p.background, p.empty_border]).flatten().collect();
            let arrows: Vec<usize> = panel.slot_views.iter().filter_map(|(_, p)| gear.find(&format!("{}/ArrowHolder", gear.nodes[p.header?].path))).collect();
            look.gear.shown = shown_of(&gear.nodes, &[], &arrows, &scripted);
        }
        if !pictures {
            look.panel = Some(panel);
            return look;
        }
        match trp_data::screen::ScenePictures::new(install) {
            Ok(mut pictures) => {
                let mut reader = PictureReader { pictures: &mut pictures, textures: HashMap::new(), missing: Vec::new() };
                let mut read = |scene: &trp_data::screen::ScenePanel, shown: &[Shown], kept: &mut Vec<Option<trp_data::unity::extract::TextureData>>| reader.panel(scene, shown, file, kept);
                look.health.pictures = read(&panel.panel, &look.health.shown, &mut look.textures);
                if let Some(strip) = &panel.strip {
                    look.strip.pictures = read(strip, &look.strip.shown, &mut look.textures);
                }
                if let Some(foot) = &panel.foot {
                    look.foot.pictures = read(foot, &look.foot.shown, &mut look.textures);
                }
                if let Some(border) = &panel.border {
                    look.border.pictures = read(border, &look.border.shown, &mut look.textures);
                }
                if let Some(gear) = &panel.gear_panel {
                    look.gear.pictures = read(gear, &look.gear.shown, &mut look.textures);
                }
                // The containers' column: its panel with its row template,
                // its grid view, the slots' empty pictures, the layouts.
                match trp_data::screen::ContainerViews::load(install, file) {
                    Ok(views) => {
                        let template = &views.panel.nodes[views.template].path;
                        let by_slot: Vec<usize> = ["Full Border", "Empty Border", "Background", "BackImage"].iter().filter_map(|name| views.panel.find(&format!("{template}/Slot Panel/{name}"))).collect();
                        // The row template, and its special slots' panel
                        // and view, which a container with special slots
                        // turns on (`SearchableSlotView.CreateSlots`).
                        let mut turned = vec![views.template];
                        turned.extend(["Slot Template/Special Slots Panel Template", "Slot Template/Special Slot Template"].iter().filter_map(|end| (0..views.panel.nodes.len()).find(|&index| views.panel.nodes[index].path.ends_with(end))));
                        look.containers_drawn.shown = shown_of(&views.panel.nodes, &turned, &[], &by_slot);
                        look.containers_drawn.pictures = read(&views.panel, &look.containers_drawn.shown, &mut look.textures);
                        if let Some(grid) = &views.grid {
                            look.grid_drawn.shown = shown_of(&grid.nodes, &[], &[], &[]);
                            look.grid_drawn.pictures = read(grid, &look.grid_drawn.shown, &mut look.textures);
                        }
                        look.containers = Some(views);
                    }
                    Err(e) => eprintln!("the game's containers' column could not be read; it is drawn the app's way: {e:#}"),
                }

                look.layouts = trp_data::screen::rig_layouts(install, layouts);
                // The bar of tabs, its tabs laid out at the width the game
                // works out from their words in their fonts.
                match trp_data::screen::TabBar::load(install, file) {
                    Ok(mut bar) => {
                        let keys: Vec<&str> = bar.tabs.iter().map(|tab| tab.key.as_str()).collect();
                        let texts = trp_data::inventory::texts(install, "en", &keys).unwrap_or_default();
                        let say = |key: &str| texts.get(&key.to_lowercase()).cloned();
                        let width = bar.width(install, &say);
                        bar.lay_out(width);
                        look.tab_words = bar.tabs.iter().map(|tab| trp_data::screen::TabBar::words(tab, &say)).collect();
                        look.tab_width = width;
                        // Each tab's versions shown as it is chosen and as
                        // the mouse is on it; its words' samples, which
                        // only size it, never.
                        let turned: Vec<usize> = bar.tabs.iter().flat_map(|tab| [tab.selected, tab.hover]).flatten().collect();
                        let scripted: Vec<usize> = bar.tabs.iter().flat_map(|tab| [tab.normal, tab.selected, tab.hover, tab.idle]).flatten().collect();
                        // The prestige tab's mark of a level ready to finish
                        // (`NewState`, the screen's `_prestigeNewLevelIcon`):
                        // off unless the next level's task can be finished and
                        // has not been seen (`PrestigeLevelAvailabilityChanged`),
                        // which a character with no prestige never has.
                        let mut never: Vec<usize> = bar.tabs.iter().flat_map(|tab| tab.size_definers.clone()).collect();
                        never.extend((0..bar.panel.nodes.len()).filter(|&index| bar.panel.nodes[index].name == "NewState"));
                        look.tabs_drawn.shown = shown_of(&bar.panel.nodes, &turned, &never, &scripted);
                        look.tabs_drawn.pictures = read(&bar.panel, &look.tabs_drawn.shown, &mut look.textures);
                        // The back button at rest (`DefaultUIButtonAnimation`,
                        // normal: its ground clear, its words in their own
                        // colour), as the scene keeps it.
                        if let Some(back) = &bar.back {
                            look.back_drawn.shown = shown_of(&back.nodes, &[], &[], &[]);
                            look.back_drawn.pictures = read(back, &look.back_drawn.shown, &mut look.textures);
                        }
                        look.tab_bar = Some(bar);
                    }
                    Err(e) => eprintln!("the game's bar of tabs could not be read; the app's tabs are drawn: {e:#}"),
                }
                // The panel for a container opened, as a raid shows it: the
                // panel and its header turned on; the money, what is only in
                // the lobby, the sorting table's and the search's tabs and
                // the mark of what is had from outside a raid off
                // (`SimpleStashPanel.Show`); its scroll bars drawn with the
                // scrolling (`scroll_views`); its filters' versions as the
                // filter chosen (`FilterPanel.Show`, all).
                match trp_data::screen::StashViews::load(install, file) {
                    Ok(mut stash) => {
                        let turned: Vec<usize> = [Some(stash.simple), stash.header].into_iter().flatten().collect();
                        let bars: Vec<usize> = (0..stash.panel.nodes.len()).filter(|&index| stash.panel.nodes[index].scripts.iter().any(|(class, _)| class == "Scrollbar")).collect();
                        let mut never: Vec<usize> = [stash.money, stash.sorting_table, stash.search, stash.external].into_iter().flatten().chain(stash.lobby_only.iter().copied()).chain(bars).collect();
                        never.extend(stash.panel.find("Complex Loot Panel"));
                        // On and off as the script leaves them, which is what
                        // its rows lay out (the header's row is sized to it).
                        for &index in &turned {
                            stash.panel.nodes[index].active = true;
                        }
                        for &index in &never {
                            stash.panel.nodes[index].active = false;
                        }
                        let scripted: Vec<usize> = stash.filter_tabs.iter().flat_map(|tab| [tab.normal, tab.selected]).flatten().collect();
                        look.stash_drawn.shown = shown_of(&stash.panel.nodes, &turned, &never, &scripted);
                        look.stash_drawn.pictures = read(&stash.panel, &look.stash_drawn.shown, &mut look.textures);
                        look.name_font = stash.name.and_then(|name| stash.panel.nodes[name].text.as_ref()).and_then(|text| {
                            let file = stash.panel.file_of(text.font.0)?;
                            trp_data::screen::TmpFont::read(install, &file, text.font.1).ok()
                        });
                        look.stash = Some(stash);
                    }
                    Err(e) => eprintln!("the game's panel for a container opened could not be read; the app's is drawn: {e:#}"),
                }
                // The quick-use panel, every view on as its script shows
                // them (`InventoryScreenQuickAccessPanel.Show`).
                match trp_data::screen::QuickViews::load(install, file) {
                    Ok(mut quick) => {
                        for slot in quick.slots.clone() {
                            quick.panel.nodes[slot.object].active = true;
                        }
                        look.quick_drawn.shown = shown_of(&quick.panel.nodes, &[], &[], &[]);
                        look.quick_drawn.pictures = read(&quick.panel, &look.quick_drawn.shown, &mut look.textures);
                        look.quick = Some(quick);
                    }
                    Err(e) => eprintln!("the game's quick-use panel could not be read; the app's is drawn: {e:#}"),
                }
                // The battle screen's panels of the character: the stance's
                // figures shown as its script has them; the speed's notch
                // made by the script (`battle_view`); the effects' icon
                // made for each effect.
                look.battle_modes = crate::battle_view::BattleModes::read(install);
                match trp_data::screen::BattleViews::load(install, file) {
                    Ok(views) => {
                        let figures: Vec<usize> = views.figures.iter().map(|(object, _)| *object).collect();
                        look.stance_drawn.shown = shown_of(&views.stance.nodes, &[], &[], &figures);
                        look.stance_drawn.pictures = read(&views.stance, &look.stance_drawn.shown, &mut look.textures);
                        look.body_drawn.shown = shown_of(&views.health.nodes, &[], &[], &[]);
                        look.body_drawn.pictures = read(&views.health, &look.body_drawn.shown, &mut look.textures);
                        look.battle = Some(views);
                    }
                    Err(e) => eprintln!("the game's panels of the stance and of the health in a raid could not be read; the app's meters are drawn: {e:#}"),
                }
                match trp_data::screen::QuickViews::load_battle(install, file) {
                    Ok(quick) => {
                        look.battle_quick_drawn.shown = shown_of(&quick.panel.nodes, &[], &[], &[]);
                        look.battle_quick_drawn.pictures = read(&quick.panel, &look.battle_quick_drawn.shown, &mut look.textures);
                        look.battle_quick = Some(quick);
                    }
                    Err(e) => eprintln!("the game's quick bar in a raid could not be read; none is drawn: {e:#}"),
                }
                // The views of a thing, in a slot and in a grid.
                match trp_data::screen::ItemViews::load(install) {
                    Ok(items) => {
                        for (view, drawn) in [(&items.slot, &mut look.slot_item), (&items.grid, &mut look.grid_item)] {
                            if let Some(view) = view {
                                // What the views' scripts turn on for a
                                // weapon (`ValidateMastering`) and a part
                                // (`ItemViewStats`): their pictures read.
                                let named = |name: &str| (0..view.nodes.len()).filter(|&index| view.nodes[index].name == name).collect::<Vec<_>>();
                                let mut turned: Vec<usize> = ["MasteringContainer", "MasteringIcon", "Level", "ModIconContainer"].iter().flat_map(|name| named(name)).collect();
                                turned.extend(named("Image").into_iter().filter(|&index| view.nodes[index].parent.is_some_and(|parent| view.nodes[parent].name == "MasteringIcon")));
                                drawn.shown = shown_of(&view.nodes, &turned, &[], &[]);
                                drawn.pictures = read(view, &drawn.shown, &mut look.textures);
                            }
                        }
                        look.items = Some(items);
                    }
                    Err(e) => eprintln!("the game's views of a thing could not be read; things are drawn the app's way: {e:#}"),
                }
                // The window that shows a thing, as its script leaves it
                // for a thing with no tag (`InfoWindow.UpdateTag`), not on
                // the wish list (`_wishlistIcon`) and with its preview drawn
                // (its loader gone); the panel of the parts on it off
                // (`_modsPanel`, on for a thing with slots: not built).
                match trp_data::screen::info_window(install) {
                    Ok(mut info) => {
                        let off: Vec<usize> = INFO_OFF.iter().filter_map(|path| info.find(path)).collect();
                        for &index in &off {
                            info.nodes[index].active = false;
                        }
                        look.info_drawn.shown = shown_of(&info.nodes, &[], &off, &[]);
                        look.info_drawn.pictures = read(&info, &look.info_drawn.shown, &mut look.textures);
                        look.info_mass_font = info.find(INFO_MASS).and_then(|mass| info.nodes[mass].text.as_ref()).and_then(|text| trp_data::screen::TmpFont::read(install, &info.file_of(text.font.0)?, text.font.1).ok());
                        look.info = Some(info);
                    }
                    Err(e) => eprintln!("the game's window that shows a thing could not be read; the app's is drawn: {e:#}"),
                }
                // The menu of a right click, its templates of a line on.
                match trp_data::screen::context_menu(install) {
                    Ok(menu) => {
                        let templates: Vec<usize> = MENU_TEMPLATES.iter().filter_map(|path| menu.find(path)).collect();
                        look.menu_drawn.shown = shown_of(&menu.nodes, &templates, &[], &[]);
                        look.menu_drawn.pictures = read(&menu, &look.menu_drawn.shown, &mut look.textures);
                        let caption = menu.find(&format!("{}/Text", MENU_TEMPLATES[0])).and_then(|text| menu.nodes[text].text.as_ref());
                        look.menu_font = caption.and_then(|text| trp_data::screen::TmpFont::read(install, &menu.file_of(text.font.0)?, text.font.1).ok());
                        look.menu = Some(menu);
                    }
                    Err(e) => eprintln!("the game's menu of a right click could not be read; the app's is drawn: {e:#}"),
                }
                drop(read);
                // The tabs' fills and the crate panel's stripes cut to their
                // masks, from the pictures' pixels.
                let mut decoded = HashMap::new();
                if let Some(bar) = &look.tab_bar {
                    look.tab_fills = masked_fills(&bar.panel, &look.tabs_drawn, &look.textures, &mut decoded);
                }
                if let Some(stash) = &look.stash {
                    look.stash_fills = masked_fills(&stash.panel, &look.stash_drawn, &look.textures, &mut decoded);
                }
                // Each container slot's empty picture (`SearchableSlotView.
                // Show`: none for the pockets).
                for slot in ["TacticalVest", "Backpack", "SecuredContainer"] {
                    let Some((file, id)) = trp_data::screen::resource_sprite(install, &format!("Slots/Equipment/{slot}")) else { continue };
                    if let Some(picture) = reader.sprite(&file, id, &mut look.textures) {
                        look.container_backs.insert(slot.to_owned(), picture);
                    }
                }
                // A part's marks (`ItemViewFactory.LoadModClassIconSprite`,
                // `LoadModIconSprite`): its class's and its kind's.
                let marks = MOD_CLASSES.iter().map(|(_, name)| (format!("type_{name}"), format!("Mod Types/icon_type_mod_{name}")));
                let kinds = MOD_KINDS.iter().map(|(_, name)| (format!("mod_{name}"), format!("Mod Tactical Types/icon_mod_{name}")));
                for (key, path) in marks.chain(kinds) {
                    let Some((file, id)) = trp_data::screen::resource_sprite(install, &path) else { continue };
                    if let Some(picture) = reader.sprite(&file, id, &mut look.textures) {
                        look.mod_pictures.insert(key, picture);
                    }
                }
                // The sprites the stance panel's script puts on it.
                if let Some(views) = &look.battle {
                    let wanted: Vec<(i32, i64)> = views.fill_sprites.iter().flatten().chain(&views.noise_sprites).copied().collect();
                    for (outside, id) in wanted {
                        let Some(name) = views.stance.file_of(outside) else { continue };
                        if let Some(picture) = reader.sprite(&name, id, &mut look.textures) {
                            look.battle_sprites.insert((outside, id), picture);
                        }
                    }
                }
                // The icons of the menu's lines (`InteractionButtonsContainer`:
                // `Characteristics/Icons/<line>`).
                for line in trp_sim::item_menu::Line::ALL {
                    let Some((file, id)) = trp_data::screen::resource_sprite(install, &format!("Characteristics/Icons/{}", line.name())) else { continue };
                    if let Some(picture) = reader.sprite(&file, id, &mut look.textures) {
                        look.menu_icons.insert(line.name(), picture);
                    }
                }
                // The health's effects' icons (`StaticIcons.EffectIcons`).
                match trp_data::screen::effect_icons(install) {
                    Ok(icons) => {
                        for (name, (file, id)) in icons {
                            if let Some(picture) = reader.sprite(&file, id, &mut look.textures) {
                                look.effect_pictures.insert(name, picture);
                            }
                        }
                    }
                    Err(e) => eprintln!("the game's icons of the health's effects could not be read; the effects are written in words: {e:#}"),
                }
                if !reader.missing.is_empty() {
                    eprintln!("the inventory's pictures of {} could not be read; they are left out", reader.missing.join(", "));
                }
                // The fonts its texts are drawn in: each font asset's own
                // font, kept whole in the build's file beside it.
                let items = look.items.as_ref().map_or([None, None], |items| [items.slot.as_ref(), items.grid.as_ref()]);
                let containers = look.containers.as_ref().map(|views| &views.panel);
                let tabs = look.tab_bar.as_ref().map(|bar| &bar.panel);
                let stash = look.stash.as_ref().map(|stash| &stash.panel);
                let quick = look.quick.as_ref().map(|quick| &quick.panel);
                let back = look.tab_bar.as_ref().and_then(|bar| bar.back.as_ref());
                let battle = look.battle.as_ref().map_or([None, None], |views| [Some(&views.stance), Some(&views.health)]);
                let battle_quick = look.battle_quick.as_ref().map(|quick| &quick.panel);
                let info = look.info.as_ref();
                let menu = look.menu.as_ref();
                match trp_data::screen::version_label(install) {
                    Ok(version) => look.version = Some(version),
                    Err(e) => eprintln!("the game's line of its version could not be read; none is drawn: {e:#}"),
                }
                match trp_data::screen::simple_tooltip(install) {
                    Ok(tooltip) => look.tooltip = Some(tooltip),
                    Err(e) => eprintln!("the game's tooltip could not be read; the app's look is used for it: {e:#}"),
                }
                let version = look.version.as_ref().map(|(label, _)| label).or(look.tooltip.as_ref());
                let tooltip = look.tooltip.as_ref();
                let mut assets: std::collections::BTreeSet<(String, i64)> = [Some(&panel.panel), panel.strip.as_ref(), panel.foot.as_ref(), panel.gear_panel.as_ref(), items[0], items[1], containers, tabs, stash, quick, back, version, tooltip, battle[0], battle[1], battle_quick, info, menu]
                    .into_iter()
                    .flatten()
                    .flat_map(|scene| scene.nodes.iter().filter_map(|node| node.text.as_ref().and_then(|text| Some((scene.file_of(text.font.0)?, text.font.1)))))
                    .collect();
                // The materials the texts name for themselves, where they do.
                let text_materials: std::collections::BTreeSet<(String, i64)> = [Some(&panel.panel), panel.strip.as_ref(), panel.foot.as_ref(), panel.gear_panel.as_ref(), items[0], items[1], containers, tabs, stash, quick, back, version, tooltip, battle[0], battle[1], battle_quick, info, menu]
                    .into_iter()
                    .flatten()
                    .flat_map(|scene| scene.nodes.iter().filter_map(|node| node.text.as_ref().and_then(|text| text.material).and_then(|(file, id)| Some((scene.file_of(file)?, id)))))
                    .collect();
                // And each one's bold typeface, where it has one.
                for (name, id) in assets.clone() {
                    if let Some(bold) = trp_data::screen::TmpFont::read(install, &name, id).ok().and_then(|font| font.bold) {
                        look.bold_fonts.insert((name, id), bold.clone());
                        assets.insert(bold);
                    }
                }
                for (name, id) in &assets {
                    if let Some(effect) = TextEffect::of(install, name, *id) {
                        look.text_effects.insert((name.clone(), *id), effect);
                    }
                }
                let mut kept_fonts: HashMap<String, Vec<(String, Vec<u8>)>> = HashMap::new();
                let mut tmp_built: HashMap<(String, i64), trp_data::tmp_text::TmpFace> = HashMap::new();
                for (name, id) in assets {
                    if !kept_fonts.contains_key(&name) {
                        let read = trp_data::screen::build_fonts(install, &name).unwrap_or_else(|e| {
                            eprintln!("the game's fonts could not be read from {name}; the app's is used: {e:#}");
                            Vec::new()
                        });
                        kept_fonts.insert(name.clone(), read);
                    }
                    let asset = trp_data::screen::script_name(install, &name, id).unwrap_or_default();
                    let by_face = || trp_data::screen::TmpFont::read(install, &name, id).ok().and_then(|face| trp_data::screen::font_by_face(&face.family, &face.style, &kept_fonts[&name]));
                    let font_file = trp_data::screen::font_for(&asset, &kept_fonts[&name]).or_else(by_face).map(|(_, font)| font.clone());
                    match &font_file {
                        Some(font) => look.font_data.push(((name.clone(), id), font.clone())),
                        None => eprintln!("no font of the game's is had for its font asset {asset:?}; the app's is used"),
                    }
                    // The asset as the game draws from it: its glyphs, its
                    // atlas, its font's kerning, its own material.
                    let face = trp_data::screen::TmpFont::read(install, &name, id).and_then(|font| {
                        let (atlas_file, atlas_id) = anyhow::Context::context(font.atlases.first().cloned(), "the font asset has no atlas")?;
                        let atlas = pictures.texture(&atlas_file, atlas_id)?;
                        Ok(trp_data::tmp_text::TmpFace::new(font, &atlas, font_file.as_deref()))
                    });
                    match face {
                        Ok(face) => {
                            tmp_built.insert((name.clone(), id), face);
                        }
                        Err(e) => eprintln!("the font asset {asset:?} could not be read to draw from; its texts are drawn the app's way: {e:#}"),
                    }
                    if let Ok(material) = trp_data::screen::font_material(install, &name, id) {
                        look.tmp_default_material.insert((name.clone(), id), material);
                    }
                }
                // Each asset with its bold typeface's face, where it has one.
                let bolds: std::collections::BTreeSet<(String, i64)> = tmp_built.values().filter_map(|face| face.font.bold.clone()).collect();
                for key in bolds {
                    if let Some(face) = tmp_built.remove(&key) {
                        look.tmp_faces.insert(key, std::sync::Arc::new(face));
                    }
                }
                for (key, mut face) in tmp_built {
                    face.bold = face.font.bold.as_ref().and_then(|bold| look.tmp_faces.get(bold)).cloned();
                    look.tmp_faces.insert(key, std::sync::Arc::new(face));
                }
                // The materials: the assets' own and the texts' own.
                let wanted: std::collections::BTreeSet<(String, i64)> = look.tmp_default_material.values().cloned().chain(text_materials).collect();
                for (file, id) in wanted {
                    match trp_data::screen::material_properties(install, &file, id) {
                        Ok(props) => {
                            look.tmp_materials.insert((file, id), trp_data::tmp_text::TmpMaterial::of(&props));
                        }
                        Err(e) => eprintln!("a text's material could not be read; texts drawn with it are drawn the app's way: {e:#}"),
                    }
                }
                // The scene's clips that the panels' animators play.
                let shared: Vec<String> = panel.panel.externals.iter().map(|name| name.rsplit('/').next().unwrap_or(name).to_owned()).filter(|name| name.starts_with("sharedassets")).collect();
                match trp_data::screen::scene_clips(install, &mut pictures, &shared) {
                    Ok(clips) => {
                        use trp_data::unity::anim::{path_hash, Clip};
                        let curve = |clip: &Clip, path: &str, property: &str| clip.others.iter().find(|b| b.path == path_hash(path) && b.attribute == path_hash(property)).map(|b| b.curve);
                        for (_, _, clip) in &clips {
                            if let (Some(alpha), true) = (curve(clip, "BackGlowImage", "m_Color.a"), clip.name == "Normal") {
                                let mut values = Vec::new();
                                clip.sample(0.0, &mut values);
                                look.glow_alpha = values.get(alpha).copied();
                            }
                            if let (Some(alpha), true) = (curve(clip, "BackGlowImage", "m_Color.a"), clip.name == "Highlighted") {
                                look.glow_lit = Some((clip.clone(), alpha));
                            }
                            if let (Some(height), Some(alpha), true) = (curve(clip, "", "m_AnchoredPosition.y"), curve(clip, "", "m_Color.a"), clip.name == "BuffPaneActive") {
                                look.bob = Some((clip.clone(), height, alpha));
                            }
                        }
                    }
                    Err(e) => eprintln!("the health panel's animations could not be read; its glows show and its arrows stand still: {e:#}"),
                }
            }
            Err(e) => eprintln!("the health panel's pictures could not be read; it is drawn without them: {e:#}"),
        }
        look.panel = Some(panel);
        look
    }

    /// The panels' own texts that are keys to the install's words: the
    /// health panel's views' headers, and the slots' names, which the slot
    /// views' headers say (`SlotViewHeader.Show`).
    pub fn keys(&self) -> Vec<String> {
        let Some(game) = &self.panel else { return Vec::new() };
        let headers = game.views.parts.iter().filter_map(|view| game.panel.find(&format!("{}/Header/Text", game.panel.nodes[view.view].path))).filter_map(|text| game.panel.nodes[text].text.as_ref().map(|text| text.text.clone()));
        // And the containers' slots', which their rows' headers say.
        let containers = ["TacticalVest", "Pockets", "Backpack", "SecuredContainer"].map(str::to_owned);
        // And what the panel for a container opened calls what it holds.
        headers.chain(game.slot_views.iter().map(|(slot, _)| slot.to_string())).chain(containers).chain(["LOOT".to_owned()]).collect()
    }
}

/// What reads the game's pictures for its panels, each texture once.
struct PictureReader<'a> {
    pictures: &'a mut trp_data::screen::ScenePictures,
    textures: HashMap<(String, i64), usize>,
    missing: Vec<String>,
}

impl PictureReader<'_> {
    /// A sprite of that file and number as a picture, its texture among
    /// those kept (`kept`) once.
    fn sprite(&mut self, name: &str, id: i64, kept: &mut Vec<Option<trp_data::unity::extract::TextureData>>) -> Option<Picture> {
        let cut = self.pictures.sprite(name, id).ok().filter(|cut| !cut.turned)?;
        let texture = match self.textures.get(&cut.texture) {
            Some(&texture) => texture,
            None => {
                let texture = self.pictures.texture(&cut.texture.0, cut.texture.1).ok()?;
                kept.push(Some(texture));
                self.textures.insert(cut.texture.clone(), kept.len() - 1);
                kept.len() - 1
            }
        };
        let [x, y, w, h] = cut.rect;
        Some(Picture { texture, cut: Rect::new(x, y, x + w, y + h), covers: cut.covers(), border: cut.border, size: [cut.whole[2], cut.whole[3]] })
    }

    /// The pictures of a panel's objects that show, by their places.
    fn panel(&mut self, scene: &trp_data::screen::ScenePanel, shown: &[Shown], file: &str, kept: &mut Vec<Option<trp_data::unity::extract::TextureData>>) -> Vec<Option<Picture>> {
        let mut out = vec![None; scene.nodes.len()];
        for (index, node) in scene.nodes.iter().enumerate() {
            if shown[index] == Shown::Never {
                continue;
            }
            let Some((outside, id)) = node.image.and_then(|image| image.sprite) else { continue };
            let name = scene.file_of(outside).or_else(|| (outside == 0).then(|| file.to_owned()));
            out[index] = name.and_then(|name| self.sprite(&name, id, kept));
            if out[index].is_none() {
                self.missing.push(node.path.clone());
            }
        }
        out
    }
}

/// Which of the health panel's objects are drawn in a raid (`Shown`): as
/// `shown_of` has them, with the views and their effects' panels turned on
/// as the panel shows (`BodyPartView.Show`, `EffectsPanel.Show`); never the
/// legend or the damage panels, which are for the damage of a raid gone by
/// (`InventoryScreenHealthPanel.Show`, given none in a raid).
fn health_shown(panel: &trp_data::screen::HealthPanel) -> Vec<Shown> {
    let views = &panel.views;
    let turned_on: Vec<usize> = views.parts.iter().flat_map(|view| [Some(view.view), view.effects]).flatten().collect();
    let never: Vec<usize> = views.legend.into_iter().chain(views.parts.iter().filter_map(|view| view.damage)).collect();
    let mut scripted: Vec<usize> = views.parts.iter().filter_map(|view| view.broken).collect();
    for (_, figure) in &views.parameters {
        if let Some(f) = figure {
            scripted.extend([f.ok_glow, f.glow, f.up, f.down, f.rate].into_iter().flatten());
        }
    }
    shown_of(&panel.panel.nodes, &turned_on, &never, &scripted)
}

/// Which of a panel's objects are drawn: those on as the scene has them or
/// turned on as the panel shows, with all they hang from (the panel's top
/// is shown by its tab); never those of `never` or under them; as their
/// script has it, those of `scripted`.
fn shown_of(nodes: &[trp_data::screen::PanelNode], turned_on: &[usize], never: &[usize], scripted: &[usize]) -> Vec<Shown> {
    let on = |index: usize| nodes[index].active || turned_on.contains(&index);
    (0..nodes.len())
        .map(|index| {
            let mut above = nodes[index].parent;
            while let Some(at) = above {
                if never.contains(&at) || (nodes[at].parent.is_some() && !on(at)) {
                    return Shown::Never;
                }
                above = nodes[at].parent;
            }
            if never.contains(&index) {
                Shown::Never
            } else if scripted.contains(&index) {
                Shown::ByScript
            } else if on(index) || nodes[index].parent.is_none() {
                Shown::Always
            } else {
                Shown::Never
            }
        })
        .collect()
}

impl HealthLook {
    /// A text's font: the game's, where it is had, else the app's.
    fn font(&self, panel: &trp_data::screen::ScenePanel, font: (i32, i64)) -> Handle<Font> {
        panel.file_of(font.0).and_then(|file| self.fonts.get(&(file, font.1)).cloned()).unwrap_or_default()
    }

    /// A text's font as drawn in its style: bold in its font's bold
    /// typeface where it has one (as TextMeshPro takes it), else its own.
    /// What a text's font asset draws under its letters, if anything.
    pub fn text_effect(&self, panel: &trp_data::screen::ScenePanel, font: (i32, i64)) -> Option<TextEffect> {
        panel.file_of(font.0).and_then(|file| self.text_effects.get(&(file, font.1)).copied())
    }

    /// A text's font asset and material as the game draws it, where both
    /// are had (`TARKOV UI`): the asset by its file and number, the
    /// material the text names for itself, else the asset's own.
    pub fn tmp_of(&self, panel: &trp_data::screen::ScenePanel, text: &trp_data::screen::TextLook) -> Option<((String, i64), (String, i64))> {
        let asset = (panel.file_of(text.font.0)?, text.font.1);
        let material = text.material.and_then(|(file, id)| Some((panel.file_of(file)?, id))).filter(|key| self.tmp_materials.contains_key(key)).or_else(|| self.tmp_default_material.get(&asset).cloned())?;
        (self.tmp_faces.contains_key(&asset) && self.tmp_materials.contains_key(&material)).then_some((asset, material))
    }

    pub fn styled_font(&self, panel: &trp_data::screen::ScenePanel, font: (i32, i64), style: u32) -> Handle<Font> {
        let bold = (style & 1 != 0).then(|| panel.file_of(font.0).and_then(|file| self.bold_fonts.get(&(file, font.1)))).flatten();
        match bold.and_then(|bold| self.fonts.get(bold)) {
            Some(handle) => handle.clone(),
            None => self.font(panel, font),
        }
    }
}

/// What a font asset's material draws under its letters (TextMeshPro's
/// distance-field shader): a dark rim all round them, from its outline
/// (`_OutlineWidth`) and an underlay laid unmoved under them and dilated
/// (`UNDERLAY_ON`, `_UnderlayDilate`; Bender Outline: an outline of 0.1 and
/// an underlay of 0.6, black), or a drop shadow, an underlay moved
/// (`_UnderlayOffsetX`, `_UnderlayOffsetY`; Bender Shadowed: 0.3 right and
/// down, black at 0.33). Each as wide or as far, in pixels, for a letter of
/// one, the atlas sampled at the asset's point size: an underlay moves by
/// its offset times its scale ratio (`_ScaleRatioC`) times the gradient's
/// scale (`_GradientScale`) of the atlas's pixels; an outline and an
/// underlay grow by half their width times their ratio (`_ScaleRatioA`,
/// `_ScaleRatioC`) in the distance field's measure, whose whole spans twice
/// the atlas's padding (`_GradientScale` less one; 0.6 of Bender Outline
/// is about a pixel at a size of 11). The face's own dilation
/// (`_FaceDilate`) widens the letters as far, and the shader's soft edge
/// fades the rim over its ramp (`PROVISIONAL`: drawn as copies of the
/// words moved round, a fainter ring for the fade; the field's measure,
/// TextMeshPro's generator not read).
#[derive(Clone, Copy, Debug, Default)]
pub struct TextEffect {
    /// The rim's colour and how far out from the letters its edge lies
    /// (where it is half drawn), for a letter of one.
    pub rim: Option<(Color, f32)>,
    pub shadow: Option<(Color, [f32; 2])>,
    /// How much the letters themselves are widened each way, for a letter
    /// of one (`_FaceDilate`: bolder than the font's own).
    pub face: f32,
    /// How wide the rim's edge fades, in pixels (the shader's ramp, the
    /// same at every size).
    pub soft: f32,
}

impl TextEffect {
    /// Drawn at a share of its alpha, as its words are.
    pub fn faded(self, alpha: f32) -> Self {
        let fade = |color: Color| color.with_alpha(color.alpha() * alpha);
        Self { rim: self.rim.map(|(color, width)| (fade(color), width)), shadow: self.shadow.map(|(color, at)| (fade(color), at)), ..self }
    }

    fn of(install: &trp_data::Install, file: &str, id: i64) -> Option<Self> {
        let font = trp_data::screen::TmpFont::read(install, file, id).ok()?;
        let (material_file, material) = trp_data::screen::font_material(install, file, id).ok()?;
        let props = trp_data::screen::material_properties(install, &material_file, material).ok()?;
        let float = |name: &str| props.floats.get(name).copied().unwrap_or(0.0);
        let color = |name: &str| props.colors.get(name).copied().map_or(Color::BLACK, rgba);
        // An underlay is moved by so many of the atlas's pixels: its
        // offset times its ratio times the gradient's scale (the shader's
        // `_UnderlayOffset * _ScaleRatioC * _GradientScale / _TextureWidth`
        // of the texture's width).
        let per = float("_GradientScale") / font.point_size.max(1.0);
        let underlay = props.keywords.iter().any(|keyword| keyword == "UNDERLAY_ON");
        let offset = [float("_UnderlayOffsetX"), -float("_UnderlayOffsetY")].map(|o| o * float("_ScaleRatioC") * per);
        let moved = offset.iter().any(|o| o.abs() > 1e-4);
        // An outline and a dilation are in the distance field's own
        // measure, whose 0 to 1 spans twice the atlas's padding
        // (`_GradientScale` less one) in its pixels, the edge at a half.
        let padding = (float("_GradientScale") - 1.0).max(0.0);
        let spread = 2.0 * padding / font.point_size.max(1.0);
        // The letters widened by the face's dilation (the shader's
        // `weight`: `_FaceDilate * _ScaleRatioA * 0.5`), the rim's edge as
        // far again out as its own width (the underlay's bias: `weight` and
        // `_UnderlayDilate * _ScaleRatioC * 0.5`; an outline's,
        // `_OutlineWidth * _ScaleRatioA * 0.5`); both edges fade over the
        // shader's ramp, a distance of one over its scale (`_GradientScale`
        // times the screen's pixels to the atlas's), which comes to twice
        // the padding over the gradient's scale in pixels at any size.
        let weight = float("_FaceDilate") * float("_ScaleRatioA") * 0.5;
        let rim = [float("_OutlineWidth") * float("_ScaleRatioA"), if underlay && !moved { float("_UnderlayDilate") * float("_ScaleRatioC") } else { 0.0 }];
        let rim = rim[0].max(rim[1]) * 0.5;
        let effect = Self {
            rim: (rim > 1e-4).then(|| (if underlay && !moved { color("_UnderlayColor") } else { color("_OutlineColor") }, (weight + rim) * spread)),
            shadow: (underlay && moved).then(|| (color("_UnderlayColor"), offset)),
            face: weight * spread,
            soft: if float("_GradientScale") > 0.0 { 2.0 * padding / float("_GradientScale") } else { 0.0 },
        };
        (effect.rim.is_some() || effect.shadow.is_some() || effect.face > 1e-4).then_some(effect)
    }
}

/// A copy of words drawn under them as their font's rim or shadow.
#[derive(Component)]
pub struct EffectCopy;

/// Words drawn as the game draws them (`trp_data::tmp_text`): their font
/// asset and material (each by its file and number), their runs (sizes in
/// the window's units, colours as the screen's numbers) and how they are
/// laid out (a width to break lines at in the window's units). Drawn into
/// an image of their own by `draw_tmp_texts` as they are made and as they
/// change.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct TmpText {
    pub asset: (String, i64),
    pub material: (String, i64),
    pub runs: Vec<trp_data::tmp_text::TmpRun>,
    pub layout: trp_data::tmp_text::TmpLayout,
}

/// A colour as the screen's numbers (gamma), for a text drawn as the
/// game's.
pub fn screen_numbers(color: Color) -> [f32; 4] {
    let c = color.to_srgba();
    [c.red, c.green, c.blue, c.alpha]
}

/// The box a text drawn as the game's is aligned by, which its image
/// hangs from.
#[derive(Component)]
pub struct TmpBox;

/// Words drawn as the game draws them: a box laid out as `node` holding a
/// box as large as TextMeshPro aligns the words by (as wide as their
/// advances, as high as the font's ascender to its descender), and from
/// it their image, what the shader draws round them outside it. `mark`
/// puts the caller's markers on the image. Drawn once its image is made
/// (`draw_tmp_texts`), on the same frame.
pub fn tmp_words(parent: &mut ChildSpawnerCommands, node: Node, text: TmpText, mark: &mut dyn FnMut(&mut EntityCommands)) {
    parent.spawn(node).with_children(|frame| {
        let empty = Node { width: Val::Px(0.0), height: Val::Px(0.0), ..default() };
        frame.spawn((TmpBox, empty.clone())).with_children(|aligned| {
            let hung = Node { position_type: PositionType::Absolute, ..empty };
            let mut words = aligned.spawn((text, hung, ImageNode { color: Color::WHITE, ..default() }));
            mark(&mut words);
        });
    });
}

/// The images of the texts drawn as the game draws them, each made as its
/// words are first drawn or change and kept by what it is (the same words
/// alike drawn again take the same image), at the window's own pixels:
/// the image as large as it comes, its words' box where the layout puts
/// the node, by margins as wide as what lies outside it.
#[allow(clippy::type_complexity)]
fn draw_tmp_texts(
    look: Res<HealthLook>,
    window: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut images: ResMut<Assets<Image>>,
    mut texts: Query<(&TmpText, &mut ImageNode, &mut Node, &ChildOf), Changed<TmpText>>,
    mut boxes: Query<&mut Node, (With<TmpBox>, Without<TmpText>)>,
    mut made: Local<HashMap<String, (Handle<Image>, [f32; 2], [f32; 4])>>,
) {
    if texts.is_empty() {
        return;
    }
    let _clock = crate::render_timing::Clock::new("texts drawn as the game's");
    let scale = window.single().map_or(1.0, |window| window.scale_factor());
    // Kept while the screens draw the same texts; let go when it has grown
    // large (each image is kept as long as a text shows it).
    if made.len() > 4096 {
        made.clear();
    }
    for (text, mut image, mut node, parent) in &mut texts {
        let key = format!("{scale}|{text:?}");
        let (handle, size, layout) = match made.get(&key) {
            Some(kept) => kept.clone(),
            None => {
                let (Some(face), Some(material)) = (look.tmp_faces.get(&text.asset), look.tmp_materials.get(&text.material)) else { continue };
                let runs: Vec<trp_data::tmp_text::TmpRun> = text.runs.iter().map(|run| trp_data::tmp_text::TmpRun { size: run.size * scale, ..run.clone() }).collect();
                let layout = trp_data::tmp_text::TmpLayout { em: text.layout.em * scale, wrap: text.layout.wrap.map(|width| width * scale), ..text.layout };
                let drawn = trp_data::tmp_text::draw(face, material, &runs, &layout);
                let mut picture = Image::new(
                    bevy::render::render_resource::Extent3d { width: drawn.width as u32, height: drawn.height as u32, depth_or_array_layers: 1 },
                    bevy::render::render_resource::TextureDimension::D2,
                    drawn.rgba,
                    bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
                    bevy::asset::RenderAssetUsages::RENDER_WORLD,
                );
                picture.sampler = bevy::image::ImageSampler::nearest();
                let kept = (images.add(picture), [drawn.width as f32 / scale, drawn.height as f32 / scale], drawn.layout.map(|v| v / scale));
                made.insert(key, kept.clone());
                kept
            }
        };
        let [lx, ly, lw, lh] = layout;
        let [w, h] = size;
        image.image = handle;
        (node.left, node.top, node.width, node.height) = (Val::Px(-lx), Val::Px(-ly), Val::Px(w), Val::Px(h));
        if let Ok(mut aligned) = boxes.get_mut(parent.parent()) {
            (aligned.width, aligned.height) = (Val::Px(lw), Val::Px(lh));
        }
    }
}

/// Words with what their font asset draws under them: a box laid out as
/// `node` holding, in one cell of a grid, copies of the words in the
/// effect's colour moved twelve ways round for the rim (a whole ring and a
/// fainter one outside it, for its soft edge) or moved for the shadow,
/// copies in their own colour moved four ways by the face's dilation, and
/// the words themselves over them. `runs` are the words
/// and each one's font and colour; `size` the letters' height in pixels the
/// effect is measured by. `mark` puts the caller's markers on each text
/// (`None`) and each run (its place), and says whether it is a copy.
pub fn effect_words(parent: &mut ChildSpawnerCommands, node: Node, runs: &[(String, TextFont, Color)], effect: Option<TextEffect>, size: f32, mark: &mut dyn FnMut(&mut EntityCommands, Option<usize>, bool)) {
    let Some((_, first_font, first_color)) = runs.first() else { return };
    // Words in a box of a set width are broken into lines at it (a
    // window's description), each copy as wide; others run on one line.
    let wrapped = matches!(node.width, Val::Px(_));
    let layout = if wrapped { TextLayout::new_with_linebreak(LineBreak::WordBoundary) } else { TextLayout::new_with_no_wrap() };
    // Each copy: its colour (none: the words' own) and where it is moved.
    let mut copies: Vec<(Option<Color>, Vec2)> = Vec::new();
    // Twelve ways round, so that the rim is round.
    let round = |r: f32| (0..12).map(move |k| Vec2::from_angle(k as f32 * std::f32::consts::TAU / 12.0) * r);
    if let Some((color, width)) = effect.and_then(|effect| effect.rim) {
        // The rim's edge fades over the shader's ramp: whole to a quarter
        // of it inside its edge, then a fainter ring a quarter of it out.
        let (edge, soft) = (width * size, effect.map_or(0.0, |effect| effect.soft));
        if soft > 0.5 {
            copies.extend(round(edge + soft / 4.0).map(|at| (Some(color.with_alpha(color.alpha() * 0.4)), at)));
        }
        copies.extend(round((edge - soft / 4.0).max(0.5)).map(|at| (Some(color), at)));
    }
    if let Some((color, [x, y])) = effect.and_then(|effect| effect.shadow) {
        copies.push((Some(color), Vec2::new(x, y) * size));
    }
    // The letters widened: the words again in their own colours, moved
    // that far each way.
    let face = effect.map_or(0.0, |effect| effect.face * size);
    if face >= 0.15 {
        copies.extend([Vec2::X, Vec2::NEG_X, Vec2::Y, Vec2::NEG_Y].map(|way| (None, way * face)));
    }
    let cell = || Node { grid_row: GridPlacement::start(1), grid_column: GridPlacement::start(1), width: if wrapped { Val::Percent(100.0) } else { Val::Auto }, ..default() };
    parent.spawn(Node { display: Display::Grid, ..node }).with_children(|grid| {
        for (color, at) in copies.iter().copied().chain(std::iter::once((None, Vec2::ZERO))) {
            let copy = color.is_some();
            let moved = UiTransform { translation: Val2::px(at.x, at.y), ..UiTransform::IDENTITY };
            let mut text = grid.spawn((cell(), Text::new(""), first_font.clone(), TextColor(color.unwrap_or(*first_color)), layout, moved));
            if copy {
                text.insert(EffectCopy);
            }
            mark(&mut text, None, copy);
            text.with_children(|words| {
                for (at, (said, font, run_color)) in runs.iter().enumerate() {
                    let mut span = words.spawn((TextSpan::new(said.clone()), font.clone(), TextColor(color.unwrap_or(*run_color))));
                    if copy {
                        span.insert(EffectCopy);
                    }
                    mark(&mut span, Some(at), copy);
                }
            });
        }
    });
}

/// The weight a text is laid out at by its style (TextMeshPro's
/// `FontStyles.Bold`): Bevy finds a face by its family and the weight
/// asked, so a bold typeface of the same family as the regular one is only
/// had by asking for it bold.
pub fn weight_of(style: u32) -> FontWeight {
    if style & 1 != 0 { FontWeight::BOLD } else { FontWeight::NORMAL }
}

/// What the game's own view of a thing looks like (`TARKOV UI`), in a slot
/// (`SlotItemView`, `slot_layout`) or in a grid (`GridItemView`,
/// `grid_layout`): its prefab, where its caption and its value lie in it
/// (`Info Panel/Name` or `Info Panel/Caption`, and the bottom right
/// group's `Value`, by their names), their fonts, its border's colour
/// where its border is on, and the label of a thing bound to a key
/// (`BindPanel`: its picture, and its key's words).
pub struct ItemViewLook {
    pub panel: trp_data::screen::ScenePanel,
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
    /// A weapon's mastering in a slot (`SlotItemView.UpdateInfo`): the
    /// mark (`MasteringIcon/Image`) and its level's words' size, colour
    /// and font (`Level`).
    pub mastering: Option<(Sprite, f32, Color, Handle<Font>)>,
    /// A weapon's or a knife's durability in a slot (`DurabilitySlider`):
    /// its track (`Background`, sliced) and its fill (`Fill`).
    pub durability: Option<(Sprite, Sprite)>,
    /// A part's marks (`ItemViewStats.SetItemIcon`): its class's ground by
    /// `type_<class>` (`Mod Types/icon_type_mod_<class>`) and its kind's
    /// mark by `mod_<kind>` (`Mod Tactical Types/icon_mod_<kind>`).
    pub mods: HashMap<String, Sprite>,
}

/// A part's class and the name of its ground's picture
/// (`ItemViewFactory.GetModClass`, `GetModClassIconName`), by the
/// database's names of the game's classes, in the game's order of asking.
pub const MOD_CLASSES: [(&str, &str); 3] = [("FunctionalMod", "generic"), ("GearMod", "gear"), ("MasterMod", "vital")];

/// A part's kind and the name of its mark's picture (`GetModSubclass`,
/// `GetModIconName`), the same way: the first the part is of wins.
pub const MOD_KINDS: [(&str, &str); 20] = [
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
];

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

/// A picture's piece laid out where the game puts it, to a fraction of a
/// pixel, not rounded to the screen's pixels: rounded one by one, pieces
/// that meet in the game's units fell a pixel apart or over one another
/// at a scale of a third (a slot's header, the middle tiled between two
/// ends, a pixel short of its right end at 2560 by 1440), where the game
/// draws them edge to edge. Texts keep their rounding (sharp).
const EXACT: bevy::ui::LayoutConfig = bevy::ui::LayoutConfig { use_rounding: false };

/// What the window that shows a thing writes of it
/// (`ItemSpecificationPanel`, into `ItemInfoWindowLabels`): its name in
/// the caption (`ItemName`), its kind as the handbook files it
/// (`_handbookNode.Category`, " > " between), its weight with the unit
/// (`GetTruncatedWeightString`, three places) and its description.
pub struct InfoWords {
    pub caption: String,
    pub kind: String,
    pub weight: String,
    pub description: String,
}

/// The objects of the window that shows a thing its script turns off for
/// a thing of the range (`HealthLook::read`); its weight's figure, its
/// caption bar, its cross, its preview and its description's box.
const INFO_OFF: [&str; 4] = ["Inner/Caption Panel/TagPanel", "Inner/Contents/Preview Panel/Loader", "Inner/Contents/Preview Panel/MasteringAndQuestPanel/WishlistIcon", "Inner/Contents/DescriptionPanel/ModsPanel"];
const INFO_MASS: &str = "Inner/Caption Panel/MassText";
const INFO_CAPTION: &str = "Inner/Caption Panel";
const INFO_CLOSE: &str = "Inner/Caption Panel/Close Button";
const INFO_PREVIEW: &str = "Inner/Contents/Preview Panel/Camera Image";
const INFO_SCROLL: &str = "Inner/Contents/DescriptionPanel/DescriptionPanel/Scroll Area";
const INFO_DESCRIPTION: &str = "Inner/Contents/DescriptionPanel/DescriptionPanel/Scroll Area/Contents/Description";

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
fn info_size(info: &trp_data::screen::ScenePanel) -> [f32; 2] {
    let [_, _, w, h] = info.placed_in(0, [1920.0, 1080.0]).rect;
    [w, h]
}

/// The window that shows a thing (`InfoWindow`), as the game draws it
/// with what its script writes (`InfoWords`), in a frame at its top left
/// corner, `s` pixels to a unit, the `index`th window open. Its preview is
/// the thing's own picture, whole in the middle of the preview's room at
/// most four fifths of it each way (`ESTIMATE`; the game draws the thing's
/// model there, turned by a drag: not built); its description in its box,
/// cut to it (`Mask`; the game's scrolls, not built).
#[allow(clippy::too_many_arguments)]
pub fn draw_info_window(column: &mut ChildSpawnerCommands, look: &HealthLook, said: &Words, words: &InfoWords, picture: Option<(Handle<Image>, f32)>, index: usize, s: f32) {
    let Some(info) = &look.info else { return };
    let size = info_size(info);
    let mut panel = info.clone();
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
    let skip: Vec<usize> = panel.find(INFO_DESCRIPTION).into_iter().collect();
    let base = INFO + (index << 12);
    draw_nodes(column, look, &panel, &look.info_drawn, base, Marks { words: Some(&texts), skip: &skip, ..Marks::default() }, said, Frame::Is(size), [0.0, 0.0], s);
    let place = |x: f32, y: f32, w: f32, h: f32| Node { position_type: PositionType::Absolute, left: Val::Px(x * s), top: Val::Px(y * s), width: Val::Px(w * s), height: Val::Px(h * s), ..default() };
    if let (Some((image, shape)), Some(preview)) = (picture, panel.find(INFO_PREVIEW)) {
        let [x, y, w, h] = panel.rect(preview, size);
        let height = (h * 0.8).min(w * 0.8 / shape.max(1e-3));
        let width = height * shape;
        column.spawn((place(x + (w - width) / 2.0, y + (h - height) / 2.0, width, height), ImageNode::new(image)));
    }
    if let (Some(scroll), Some(description)) = (panel.find(INFO_SCROLL), panel.find(INFO_DESCRIPTION)) {
        let Some(text) = panel.nodes[description].text.clone() else { return };
        let [x, y, w, h] = panel.rect(scroll, size);
        let font = TextFont { font: look.styled_font(&panel, text.font, text.style), font_size: text.size * s, weight: weight_of(text.style), ..default() };
        let effect = look.text_effect(&panel, text.font);
        let width = (w - text.margin[0] - text.margin[2]) * s;
        let tmp = look.tmp_of(&panel, &text);
        column.spawn(Node { overflow: Overflow::clip(), ..place(x, y, w, h) }).with_children(|clip| {
            let room = Node { margin: UiRect::new(Val::Px(text.margin[0] * s), Val::ZERO, Val::Px(text.margin[1] * s), Val::ZERO), ..default() };
            match tmp {
                // As the game draws it, its lines broken at the box's width.
                Some((asset, material)) => {
                    let run = trp_data::tmp_text::TmpRun { text: words.description.clone(), size: text.size * s, color: screen_numbers(rgba(text.color)), bold: text.style & 1 != 0 };
                    let layout = trp_data::tmp_text::TmpLayout { em: text.size * s, spacing: text.spacing, kerning: text.kerning, wrap: Some(width), align: text.horizontal & 7 };
                    tmp_words(clip, room, TmpText { asset, material, runs: vec![run], layout }, &mut |_| {});
                }
                None => effect_words(clip, Node { width: Val::Px(width), ..room }, &[(words.description.clone(), font, rgba(text.color))], effect, text.size * s, &mut |_, _, _| {}),
            }
        });
    }
}

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
    panel: trp_data::screen::ScenePanel,
    drawn: Drawn,
    words: HashMap<usize, String>,
    pictures: HashMap<usize, Picture>,
    groups: HashMap<usize, f32>,
    colors: HashMap<usize, Color>,
    set: HashMap<usize, bool>,
    pub size: [f32; 2],
    pub lines: Vec<[f32; 4]>,
}

/// A line's two colours (`SimpleContextMenuButton._defaultBackgroundColor`,
/// `_selectedBackgroundColor`), from its script's fields: after the four
/// pointers the button and the image take, at 88 and 104.
fn menu_line_colors(fields: &[u8]) -> Option<[Color; 2]> {
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
    let source = look.menu.as_ref()?;
    let holder = source.find(MENU_LINES)?;
    let mut panel = source.clone();
    let mut drawn = Drawn { shown: look.menu_drawn.shown.clone(), pictures: look.menu_drawn.pictures.clone() };
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
        let path = match line.name {
            "Discard" => MENU_TEMPLATES[2],
            "AddToWishlist" => MENU_TEMPLATES[1],
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
            drawn.shown.push(look.menu_drawn.shown.get(old).copied().unwrap_or(Shown::Never));
            drawn.pictures.push(look.menu_drawn.pictures.get(old).copied().flatten());
        }
        let named = |panel: &trp_data::screen::ScenePanel, name: &str| (first..panel.nodes.len()).find(|&index| panel.nodes[index].name == name);
        let copy_top = first;
        if let Some(text_at) = named(&panel, "Text") {
            let caption = line.caption.to_uppercase();
            if let (Some(text), Some(font)) = (panel.nodes[text_at].text.clone(), look.menu_font.as_ref()) {
                let width = font.width(&caption, text.size, text.spacing, text.style & 1 != 0) + text.margin[0] + text.margin[2];
                let flexible = panel.nodes[text_at].element.map_or([1.0, -1.0], |element| element.flexible);
                panel.nodes[text_at].element = Some(trp_data::screen::Element { ignore: false, min: [-1.0, -1.0], preferred: [width, -1.0], flexible });
            }
            words.insert(text_at, caption);
        }
        if let (Some(icon), Some(picture)) = (named(&panel, "Icon"), look.menu_icons.get(line.name)) {
            pictures.insert(icon, *picture);
        }
        if let Some(arrow) = named(&panel, "SubmenuArrow") {
            if line.arrow {
                let size = drawn.pictures.get(arrow).copied().flatten().map_or([9.0, 11.0], |picture| picture.size);
                panel.nodes[arrow].element = Some(trp_data::screen::Element { ignore: false, min: [-1.0, -1.0], preferred: size, flexible: [-1.0, -1.0] });
            } else {
                set.insert(arrow, false);
            }
        }
        if !line.enabled {
            groups.insert(copy_top, 0.3);
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
pub fn draw_menu(column: &mut ChildSpawnerCommands, look: &HealthLook, said: &Words, built: &MenuBuilt, origin: [f32; 2], s: f32) {
    let marks = Marks { words: Some(&built.words), pictures: Some(&built.pictures), groups: Some(&built.groups), colors: Some(&built.colors), set: Some(&built.set), ..Marks::default() };
    draw_nodes(column, look, &built.panel, &built.drawn, MENU, marks, said, Frame::Is(built.size), origin, s);
}

/// The label of a thing bound to a key: its picture (the cut of its
/// texture) at its sprite's own size, in units, and its key's words.
pub struct BindLook {
    pub image: Handle<Image>,
    pub cut: Rect,
    pub size: [f32; 2],
    pub text_size: f32,
    pub color: Color,
    pub font: Handle<Font>,
}

/// The ground of a thing's view as the game colours it
/// (`ItemView.UpdateStaticInfo`): the database's colour of its kind
/// (`TaxonomyColorExtension.ColorMapping`) at 0.302.
pub fn item_ground(color: &str) -> Color {
    let [r, g, b] = match color {
        "blue" => [28, 65, 86],
        "yellow" => [104, 102, 40],
        "green" => [21, 45, 0],
        "red" => [109, 36, 24],
        "black" => [0, 0, 0],
        "grey" => [29, 29, 29],
        "violet" => [76, 42, 85],
        "orange" => [60, 25, 0],
        _ => [127, 127, 127],
    };
    Color::srgba_u8(r, g, b, 77)
}

/// A view's ground under the mouse (`ItemView.DefaultSelectedColor`).
pub const ITEM_SELECTED: Color = Color::srgba(1.0, 1.0, 1.0, 77.0 / 255.0);

/// The words a stack's count is in (`GridItemView.GetStackColor`):
/// rounds', money's and the rest's, by the database's names of the kind's
/// classes.
pub fn stack_color(class_names: &[String]) -> Color {
    let has = |name: &str| class_names.iter().any(|class| class == name);
    if has("Ammo") {
        Color::srgb_u8(0xda, 0xda, 0xbc)
    } else if has("Money") {
        Color::srgb_u8(0x8d, 0xb5, 0x00)
    } else {
        Color::srgb_u8(0xb6, 0xc1, 0xc7)
    }
}

/// Each item view's look made from its prefab, with its pictures and fonts.
fn item_looks(look: &HealthLook) -> [Option<std::sync::Arc<ItemViewLook>>; 2] {
    let Some(items) = &look.items else { return [None, None] };
    let make = |panel: &trp_data::screen::ScenePanel, drawn: &Drawn| -> ItemViewLook {
        let caption = panel.find("Info Panel/Name").or_else(|| panel.find("Info Panel/Caption"));
        let value = (0..panel.nodes.len()).find(|&index| panel.nodes[index].name == "Value" && panel.nodes[index].text.is_some());
        let font = |index: Option<usize>| index.and_then(|index| panel.nodes[index].text.as_ref()).map(|text| look.styled_font(panel, text.font, text.style)).unwrap_or_default();
        let border = panel.find("Border").filter(|&index| panel.nodes[index].active).and_then(|index| panel.nodes[index].image.map(|image| rgba(image.color)));
        let bind = (0..panel.nodes.len()).find(|&index| panel.nodes[index].scripts.iter().any(|(class, _)| class == "BindPanel")).and_then(|index| {
            let picture = drawn.pictures.get(index).copied().flatten()?;
            let image = look.images.get(picture.texture)?.clone()?;
            let text = panel.nodes.iter().skip(index + 1).find(|node| node.parent == Some(index) && node.text.is_some())?.text.clone()?;
            let size = [picture.cut.width(), picture.cut.height()];
            Some(BindLook { image, cut: picture.cut, size, text_size: text.size, color: rgba(text.color), font: look.font(panel, text.font) })
        });
        let sprite = |picture: &Picture, color: Color| -> Option<Sprite> {
            let image = look.images.get(picture.texture)?.clone()?;
            Some(Sprite { image, cut: picture.cut, size: picture.size, border: picture.border, color })
        };
        let drawn_sprite = |index: usize| -> Option<Sprite> {
            let picture = drawn.pictures.get(index).copied().flatten()?;
            sprite(&picture, panel.nodes[index].image.map_or(Color::WHITE, |image| rgba(image.color)))
        };
        let ends = |end: &str| (0..panel.nodes.len()).find(|&index| panel.nodes[index].path.ends_with(end));
        let inscription = panel.find("Info Panel/Info Label").or_else(|| (0..panel.nodes.len()).find(|&index| panel.nodes[index].name == "Inscription" && panel.nodes[index].text.is_some()));
        let mastering = ends("MasteringIcon/Image").zip(ends("MasteringIcon/Level")).and_then(|(icon, level)| {
            let text = panel.nodes[level].text.as_ref()?;
            Some((drawn_sprite(icon)?, text.size, rgba(text.color), look.font(panel, text.font)))
        });
        let durability = ends("DurabilityContaimer/Background").zip(ends("DurabilityContaimer/Fill Area/Fill")).and_then(|(track, fill)| Some((drawn_sprite(track)?, drawn_sprite(fill)?)));
        let mods = look.mod_pictures.iter().filter_map(|(key, picture)| Some((key.clone(), sprite(picture, Color::WHITE)?))).collect();
        let effect = |index: Option<usize>| index.and_then(|index| panel.nodes[index].text.as_ref()).and_then(|text| look.text_effect(panel, text.font));
        let tmp = |index: Option<usize>| index.and_then(|index| panel.nodes[index].text.as_ref()).and_then(|text| look.tmp_of(panel, text));
        ItemViewLook {
            caption_tmp: tmp(caption),
            inscription_tmp: tmp(inscription),
            value_tmp: tmp(value),
            panel: panel.clone(),
            caption,
            value,
            caption_font: font(caption),
            value_font: font(value),
            border,
            bind,
            inscription,
            inscription_font: font(inscription),
            caption_effect: effect(caption),
            value_effect: effect(value),
            inscription_effect: effect(inscription),
            mastering,
            durability,
            mods,
        }
    };
    [
        items.slot.as_ref().map(|panel| std::sync::Arc::new(make(panel, &look.slot_item))),
        items.grid.as_ref().map(|panel| std::sync::Arc::new(make(panel, &look.grid_item))),
    ]
}

/// The panels' textures made images, and their fonts fonts, once, at the
/// start.
fn make_pictures(mut look: ResMut<HealthLook>, mut images: ResMut<Assets<Image>>, mut fonts: ResMut<Assets<Font>>) {
    for (asset, data) in std::mem::take(&mut look.font_data) {
        match Font::try_from_bytes(data) {
            Ok(font) => {
                let handle = fonts.add(font);
                look.fonts.insert(asset, handle);
            }
            Err(e) => eprintln!("a font of the game's could not be made; the app's is used: {e}"),
        }
    }
    let textures = std::mem::take(&mut look.textures);
    look.images = textures
        .into_iter()
        .map(|texture| {
            let texture = texture?;
            match crate::render_data::build_image(&texture, crate::render_data::Role::Color) {
                // Sampled as the engine samples it: a texture set to point
                // filtering at its nearest pixel (the frames of a pixel or
                // two, whose middles are clear); and clamped at its edges
                // where it is set so (`TextureWrapMode.Clamp`), as the
                // interface's mostly are: repeated, a picture's edge took
                // in its far side's (a faint line along the top of the left
                // panel's foot, its bottom row's light).
                Ok(mut image) => {
                    use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
                    let wrap = |clamp: bool| if clamp { ImageAddressMode::ClampToEdge } else { ImageAddressMode::Repeat };
                    let filter = if texture.point { ImageFilterMode::Nearest } else { ImageFilterMode::Linear };
                    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
                        address_mode_u: wrap(texture.clamp[0]),
                        address_mode_v: wrap(texture.clamp[1]),
                        mag_filter: filter,
                        min_filter: filter,
                        mipmap_filter: ImageFilterMode::Linear,
                        ..default()
                    });
                    Some(images.add(image))
                }
                Err(e) => {
                    eprintln!("the health panel's picture {} could not be made: {e:#}", texture.name);
                    None
                }
            }
        })
        .collect();
    let looks = item_looks(&look);
    look.item_looks = looks;
}

/// The game's line of its version at the screen's foot.
#[derive(Component)]
struct VersionLine(u32);

/// Draws the game's line of its version (`TARKOV UI`) along the screen's
/// foot, over every screen as the game's preloader draws it: its text's
/// look and place as the scene has them (Bender at 16, its colour at a
/// third, 12 in from the left, in a band 31.5 high just over the foot), the
/// words the app's own (`TRP` and its version, where the game's say its
/// build and SPT's its own: the app's choice). Made afresh
/// as the window's height changes.
fn show_version(mut commands: Commands, look: Res<HealthLook>, window: Query<&Window, With<bevy::window::PrimaryWindow>>, shown: Query<(Entity, &VersionLine)>) {
    let Some((label, below)) = look.version.as_ref().filter(|_| look.tarkov_ui) else { return };
    let Ok(window) = window.single() else { return };
    let height = window.height().round() as u32;
    if height == 0 || shown.iter().any(|(_, line)| line.0 == height) || look.fonts.is_empty() {
        return;
    }
    for (entity, _) in &shown {
        commands.entity(entity).despawn();
    }
    let Some(text) = label.nodes.first().and_then(|node| node.text.as_ref()) else { return };
    let s = height as f32 / 1080.0;
    let band = label.nodes[0].layout.size_delta[1];
    let node = Node {
        position_type: PositionType::Absolute,
        left: Val::Px(0.0),
        bottom: Val::Px(below * s),
        width: Val::Percent(100.0),
        height: Val::Px(band * s),
        padding: UiRect { left: Val::Px(text.margin[0] * s), ..default() },
        align_items: AlignItems::Center,
        ..default()
    };
    let font = TextFont { font: look.styled_font(label, text.font, text.style), font_size: text.size * s, weight: weight_of(text.style), ..default() };
    let words = format!("TRP {}", env!("CARGO_PKG_VERSION"));
    commands.spawn((VersionLine(height), node, GlobalZIndex(30))).with_children(|line| {
        line.spawn((Text::new(words), font, TextColor(rgba(text.color)), TextLayout::new_with_no_wrap()));
    });
}

/// The game's views of a thing handed to the inventory, which draws every
/// thing by them with `TARKOV UI` on.
fn share_item_looks(look: Res<HealthLook>, mut carried: ResMut<crate::inventory_view::Carried>) {
    carried.item_looks = look.item_looks.clone();
    carried.info_size = info_places(&look).map(|places| Vec2::from(places.size));
}

/// One object of one of the game's panels as drawn, by its number among
/// them (`STRIP`, `FOOT`); a filled image's picture inside its clip, and
/// how wide the clip is when full; a text's words, its own and those put
/// before them.
#[derive(Component, Clone, Copy)]
pub struct Piece(pub usize);
#[derive(Component)]
pub(crate) struct PieceFill(pub(crate) usize);
#[derive(Component)]
pub(crate) struct FullWidth(pub(crate) f32);
#[derive(Component)]
struct PieceText(usize);
#[derive(Component)]
struct PieceSpan(usize);
/// A rate's arrow, which bobs while its figure moves: its top as laid out,
/// in pixels, and the pixels to a unit.
#[derive(Component)]
struct Bob(f32, f32);
/// The effects on a part, by the part's place in `Part::ALL`, in words
/// where the game has its icons (`EffectsPanel`, whose icons are the hard
/// settings' table, not read: `docs/spec/health.md`, "The HEALTH tab").
#[derive(Component)]
struct EffectWords(usize);

pub(crate) fn rgba(color: [f32; 4]) -> Color {
    Color::srgba(color[0], color[1], color[2], color[3])
}

/// Draws the game's health panel in the inventory's left panel: each
/// object where the game lays it out in a left panel of that size (the
/// screen's units), the left panel's top left corner at `origin` in what
/// it is drawn into, `s` pixels to a unit.
pub fn draw_panel(column: &mut ChildSpawnerCommands, look: &HealthLook, said: &Words, health: Option<&Health>, left: [f32; 2], origin: [f32; 2], s: f32) {
    let Some(game) = &look.panel else {
        column.spawn(words("The game's health panel could not be read from the install.", 12.0, FAINT));
        return;
    };
    if health.is_none() {
        column.spawn(words("The options' HEALTH SYSTEM is off: the character cannot be hurt, and is taken as fed and watered.", 13.0, WORDS));
        return;
    }
    let panel = &game.panel;
    let arrows: Vec<usize> = game.views.parameters.iter().filter_map(|(_, f)| f.as_ref()).flat_map(|f| [f.up, f.down]).flatten().collect();
    let glows: Vec<usize> = game.views.parts.iter().filter_map(|view| panel.find(&format!("{}/BackGlowImage", panel.nodes[view.view].path))).collect();
    let headers: Vec<usize> = game.views.parts.iter().filter_map(|view| panel.find(&format!("{}/Header/Text", panel.nodes[view.view].path))).collect();
    let marks = Marks { arrows: &arrows, glows: &glows, headers: &headers, ..Marks::default() };
    draw_nodes(column, look, panel, &look.health, 0, marks, said, Frame::In(left), origin, s);
    // The effects on each part in its view's effects panel: as the game's
    // icons where they were read (`EffectsPanel`: a grid of 24 by 22 from
    // its top left, left to right), else in words.
    for (n, view) in game.views.parts.iter().enumerate() {
        let Some(effects) = view.effects else { continue };
        let [x, y, w, h] = panel.placed_in(effects, left).rect;
        let at = Node { position_type: PositionType::Absolute, left: Val::Px((origin[0] + x) * s), top: Val::Px((origin[1] + y) * s), width: Val::Px(w * s), height: Val::Px(h * s), ..default() };
        if look.effect_pictures.is_empty() {
            column.spawn((at, Visibility::Inherited)).with_children(|cell| {
                cell.spawn((EffectWords(n), words("", 10.0 * s, FAINT)));
            });
        } else {
            let grid = Node { flex_direction: FlexDirection::Row, flex_wrap: FlexWrap::Wrap, align_content: AlignContent::FlexStart, ..at };
            column.spawn((EffectIcons(n, s, Vec::new()), grid, Visibility::Inherited));
        }
    }
    // Where a thing let go goes to a part (`HealthBarButton`, on the part's
    // view: what of the view is drawn takes the mouse, the engine's
    // pointer events going up to the button), over all of it.
    for (n, view) in game.views.parts.iter().enumerate() {
        let path = &panel.nodes[view.view].path;
        let mut area: Option<[f32; 4]> = None;
        for (index, node) in panel.nodes.iter().enumerate() {
            let below = index == view.view || node.path.starts_with(&format!("{path}/"));
            let drawn = look.health.shown.get(index).is_some_and(|shown| *shown != Shown::Never) && (node.image.is_some() || node.text.is_some());
            if !below || !drawn {
                continue;
            }
            let [x, y, w, h] = panel.placed_in(index, left).rect;
            area = Some(match area {
                None => [x, y, x + w, y + h],
                Some([x0, y0, x1, y1]) => [x0.min(x), y0.min(y), x1.max(x + w), y1.max(y + h)],
            });
        }
        let Some([x0, y0, x1, y1]) = area else { continue };
        let at = Node { position_type: PositionType::Absolute, left: Val::Px((origin[0] + x0) * s), top: Val::Px((origin[1] + y0) * s), width: Val::Px((x1 - x0) * s), height: Val::Px((y1 - y0) * s), ..default() };
        column.spawn((HealthDrop(n), RelativeCursorPosition::default(), at));
    }
}

/// Where a thing carried and let go is used on a part, by the part's
/// place in `Part::ALL`: over the part's view on the HEALTH tab, the
/// game's or the app's.
#[derive(Component)]
pub struct HealthDrop(pub usize);

/// Lights a part's glow while a thing that can be used on it is carried
/// over its view (`HealthBarButton.OnPointerEnter`: `Highlighted`, the
/// glow's alpha by its clip, from 0 to 1 in a third of a second; a thing
/// that can do nothing to it lights nothing), and puts it out as the
/// thing leaves or is let go (`Normal`). The game's look (`TARKOV UI`)
/// alone: the app's has no glow.
#[allow(clippy::too_many_arguments)]
fn light_drop_glow(
    look: Res<HealthLook>,
    controller: Res<Controller>,
    carried: Res<crate::inventory_view::Carried>,
    game: Res<crate::GameData>,
    dragging: Res<crate::inventory_view::Dragging>,
    time: Res<Time<Real>>,
    drops: Query<(&HealthDrop, &RelativeCursorPosition, &InheritedVisibility)>,
    mut glows: Query<(Option<&Piece>, Option<&SliceOf>, &mut ImageNode)>,
    mut lit: Local<[Option<f32>; 7]>,
) {
    let Some(panel) = look.panel.as_ref().filter(|_| look.tarkov_ui) else { return };
    let over = drops.iter().find(|(_, at, shown)| shown.get() && at.cursor_over()).map(|(drop, ..)| drop.0);
    let usable = |n: usize| -> bool {
        let (Some(id), Some(health)) = (dragging.0, controller.health.as_ref()) else { return false };
        let Some(item) = carried.equipment.item.find(id) else { return false };
        let Some(medicine) = trp_sim::health::Medicine::of(&game.db, &item.kind.template) else { return false };
        let resource = item.resource.unwrap_or_else(|| medicine.full());
        health.part_to_apply(&medicine, resource, Some(Part::ALL[n])).is_some()
    };
    let dt = time.delta_secs();
    for n in 0..lit.len() {
        lit[n] = if over == Some(n) && usable(n) { Some(lit[n].map_or(0.0, |t| t + dt)) } else { None };
    }
    let mut values = Vec::new();
    let alpha = |n: usize, values: &mut Vec<f32>| match (lit[n], &look.glow_lit) {
        (Some(t), Some((clip, curve))) => {
            clip.sample(t, values);
            values.get(*curve).copied().unwrap_or(1.0)
        }
        (Some(_), None) => 1.0,
        (None, _) => look.glow_alpha.unwrap_or(0.0),
    };
    let glow_of: Vec<Option<usize>> = panel.views.parts.iter().map(|view| panel.panel.find(&format!("{}/BackGlowImage", panel.panel.nodes[view.view].path))).collect();
    for (piece, slice, mut image) in &mut glows {
        let Some(index) = piece.map(|piece| piece.0).or(slice.map(|slice| slice.0)) else { continue };
        let Some(n) = glow_of.iter().position(|glow| *glow == Some(index)) else { continue };
        let want = alpha(n, &mut values);
        if (image.color.alpha() - want).abs() > 1e-4 {
            image.color = image.color.with_alpha(want);
        }
    }
}

/// A part's effects panel drawn with the game's icons: the part's place in
/// `Part::ALL`, the pixels to a unit, and what it shows (each icon's
/// effect, by the game's name, and whether it is left over, blinking).
#[derive(Component)]
struct EffectIcons(usize, f32, Vec<(&'static str, bool)>);

/// One effect's icon (`EffectIcon`): what its tooltip says (the effect's
/// name in the install's words, and while it has under two hours left, its
/// seconds left: `BuffDescription.TimeLeftDisplay`; a left-over one's name
/// alone), and whether it blinks.
#[derive(Component)]
struct EffectIcon {
    part: usize,
    kind: Kind,
    residual: bool,
}

/// The game's icon of an effect of the app's health, by the game's name
/// (`StaticIcons.EffectSprites`); none for what the game shows no icon of.
pub(crate) fn effect_icon(kind: Kind) -> Option<&'static str> {
    Some(match kind {
        Kind::LightBleeding => "LightBleeding",
        Kind::HeavyBleeding => "HeavyBleeding",
        Kind::Wound => "Wound",
        Kind::Fracture => "Fracture",
        Kind::Pain => "Pain",
        Kind::Tremor => "Tremor",
        Kind::PainKiller => "Painkiller",
        Kind::Exhaustion => "Exhaustion",
        Kind::Dehydration => "Dehydration",
        Kind::ChronicFatigue => "ChronicStaminaFatigue",
        Kind::Encumbered => "Encumbered",
        Kind::OverEncumbered => "OverEncumbered",
        Kind::LowHealth | Kind::Existence | Kind::MedEffect => return None,
    })
}

/// The cell an effect's icon takes in its panel's grid (`GridLayoutGroup`:
/// `m_CellSize`), in units.
const EFFECT_CELL: [f32; 2] = [24.0, 22.0];

/// Keeps each part's icons as its effects are (`EffectsPanel.UpdateEffects`:
/// the part's effects at work, then those left over, one icon of a kind),
/// and blinks those left over (`EffectIcon.Co_Blinking`: the alpha from 0.2
/// to 1 and back at one a second).
fn fill_effect_icons(mut commands: Commands, controller: Res<Controller>, look: Res<HealthLook>, time: Res<Time<Real>>, mut panels: Query<(Entity, &mut EffectIcons)>, mut icons: Query<(&EffectIcon, &mut ImageNode)>) {
    let Some(health) = controller.health.as_ref() else { return };
    for (entity, mut panel) in &mut panels {
        let part = Part::ALL[panel.0];
        let mut wanted: Vec<(&'static str, bool, Kind)> = Vec::new();
        for residual in [false, true] {
            for effect in health.effects.iter().filter(|e| e.part == part && if residual { e.stage == trp_sim::health::Stage::Residual } else { e.existing() }) {
                let Some(name) = effect_icon(effect.kind) else { continue };
                if !wanted.iter().any(|(had, ..)| *had == name) {
                    wanted.push((name, residual, effect.kind));
                }
            }
        }
        let shown: Vec<(&'static str, bool)> = wanted.iter().map(|(name, residual, _)| (*name, *residual)).collect();
        if panel.2 == shown {
            continue;
        }
        panel.2 = shown;
        let s = panel.1;
        let images = &look.images;
        commands.entity(entity).despawn_related::<Children>().with_children(|grid| {
            for (name, residual, kind) in wanted {
                let Some(picture) = look.effect_pictures.get(name) else { continue };
                let Some(image) = images.get(picture.texture).cloned().flatten() else { continue };
                let node = Node { width: Val::Px(EFFECT_CELL[0] * s), height: Val::Px(EFFECT_CELL[1] * s), flex_shrink: 0.0, ..default() };
                let drawn = ImageNode { image, rect: Some(picture.cut), flip_y: true, ..default() };
                grid.spawn((EffectIcon { part: panel.0, kind, residual }, node, drawn, RelativeCursorPosition::default()));
            }
        });
    }
    // The blink: a triangle between 0.2 and 1 taking 0.8 s each way.
    let phase = (time.elapsed_secs() / 0.8) % 2.0;
    let alpha = 0.2 + 0.8 * if phase < 1.0 { phase } else { 2.0 - phase };
    for (icon, mut image) in &mut icons {
        let want = if icon.residual { alpha } else { 1.0 };
        if (image.color.alpha() - want).abs() > 1e-3 {
            image.color = Color::WHITE.with_alpha(want);
        }
    }
}

/// The game's simple tooltip as the app draws it: its ground, frame and
/// text, with the mouse on an effect's icon.
#[derive(Component)]
struct EffectTooltip;
#[derive(Component)]
struct EffectTooltipText;

/// Shows the tooltip of the effect's icon the mouse is on (`EffectIcon`'s
/// hover: `SimpleTooltip.Show`), its lower left 10 units up and to the
/// right of the mouse (`Tooltip.SetPosition`), kept on the screen
/// (`CorrectPositionResolution`), its words as the game writes them; and
/// hides it as the mouse leaves. Made the first time it is wanted, as the
/// game's scene has it: a black ground 6 units in at the sides, 2 above and
/// 5 below its words, a frame of `border_generic`'s colour, the words in
/// Bender at 14 in their colour.
#[allow(clippy::too_many_arguments)]
fn show_effect_tooltip(
    mut commands: Commands,
    controller: Res<Controller>,
    look: Res<HealthLook>,
    said: Res<Words>,
    window: Query<&Window, With<bevy::window::PrimaryWindow>>,
    icons: Query<(&EffectIcon, &RelativeCursorPosition, &InheritedVisibility)>,
    mut tooltip: Query<(Entity, &mut Node, &mut Visibility), With<EffectTooltip>>,
    mut words: Query<(&mut Text, &mut TextFont), With<EffectTooltipText>>,
) {
    let Ok(window) = window.single() else { return };
    let s = window.height() / 1080.0;
    let over = icons.iter().find(|(_, at, shown)| shown.get() && at.cursor_over()).map(|(icon, ..)| icon);
    let (Some(icon), Some(health), Some(cursor)) = (over, controller.health.as_ref(), window.cursor_position()) else {
        for (_, _, mut visibility) in &mut tooltip {
            if *visibility != Visibility::Hidden {
                *visibility = Visibility::Hidden;
            }
        }
        return;
    };
    let part = Part::ALL[icon.part];
    let effect = health.effects.iter().find(|e| e.part == part && e.kind == icon.kind && if icon.residual { e.stage == trp_sim::health::Stage::Residual } else { e.existing() });
    let name = said.effect(icon.kind).unwrap_or_else(|| icon.kind.name().to_owned());
    let left = effect.map_or(0.0, |e| e.delay + e.work - e.whole).round();
    let line = if !icon.residual && left > 0.0 && left < 7200.0 { format!("{name} ({left:.0})") } else { name };
    // The game's tooltip's look, where it was read.
    let text = look.tooltip.as_ref().and_then(|panel| panel.nodes.iter().find_map(|node| node.text.as_ref()).map(|text| (panel, text)));
    let (font, size, color) = text.map_or((Handle::default(), 14.0, Color::srgb(0.651, 0.690, 0.710)), |(panel, text)| (look.font(panel, text.font), text.size, rgba(text.color)));
    let frame = look.tooltip.as_ref().and_then(|panel| panel.find("Background/Border")).and_then(|index| look.tooltip.as_ref()?.nodes[index].image.map(|image| rgba(image.color))).unwrap_or(Color::srgb(0.322, 0.349, 0.353));
    // Its lower left 10 up and right of the mouse, kept on the screen.
    let (width, height) = (window.width(), window.height());
    let at = |node: &mut Node| {
        node.left = Val::Px((cursor.x + 10.0 * s).min(width - 40.0 * s).max(0.0));
        node.bottom = Val::Px((height - cursor.y + 10.0 * s).min(height - 30.0 * s).max(0.0));
    };
    match tooltip.single_mut() {
        Ok((_, mut node, mut visibility)) => {
            at(&mut node);
            *visibility = Visibility::Inherited;
            for (mut text, mut text_font) in &mut words {
                if text.0 != line {
                    text.0 = line.clone();
                }
                if text_font.font_size != size * s {
                    text_font.font_size = size * s;
                }
            }
        }
        Err(_) => {
            let mut node = Node {
                position_type: PositionType::Absolute,
                padding: UiRect { left: Val::Px(6.0 * s), right: Val::Px(6.0 * s), top: Val::Px(2.0 * s), bottom: Val::Px(5.0 * s) },
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            };
            at(&mut node);
            commands.spawn((EffectTooltip, node, BackgroundColor(Color::BLACK), BorderColor::all(frame), GlobalZIndex(45))).with_children(|ground| {
                ground.spawn((EffectTooltipText, Text::new(line), TextFont { font, font_size: size * s, ..default() }, TextColor(color), TextLayout::new_with_no_wrap()));
            });
        }
    }
}

/// Draws the GEAR tab's strip of figures as the game's (`TARKOV UI`),
/// where the game lays it out in the left panel, as `draw_panel` draws.
pub fn draw_strip(column: &mut ChildSpawnerCommands, look: &HealthLook, said: &Words, left: [f32; 2], origin: [f32; 2], s: f32) {
    let Some((game, strip)) = look.panel.as_ref().and_then(|game| Some((game, game.strip.as_ref()?))) else { return };
    let arrows: Vec<usize> = game.strip_figures.iter().filter_map(|(_, f)| f.as_ref()).flat_map(|f| [f.up, f.down]).flatten().collect();
    draw_nodes(column, look, strip, &look.strip, STRIP, Marks { arrows: &arrows, ..Marks::default() }, said, Frame::In(left), origin, s);
}

/// Draws the game's equipment panel (`TARKOV UI`) as a panel of that size
/// with its top left at `origin`: its outline of the character and the
/// slot view of each slot the page has (`present`), full or empty as the
/// slot is (`full`), its header in the install's words for the slot
/// (`SlotViewHeader.Show`: the slot's name in capitals). The page's own
/// boxes for the slots, with what is in them, lie over these.
#[allow(clippy::too_many_arguments)]
pub fn draw_gear(column: &mut ChildSpawnerCommands, look: &HealthLook, said: &Words, size: [f32; 2], origin: [f32; 2], s: f32, present: &dyn Fn(&str) -> bool, full: &dyn Fn(&str) -> bool) {
    let Some((game, gear)) = look.panel.as_ref().and_then(|game| Some((game, game.gear_panel.as_ref()?))) else { return };
    let (mut set, mut skip, mut words) = (HashMap::new(), Vec::new(), HashMap::new());
    for (slot, parts) in &game.slot_views {
        if !present(slot) {
            skip.push(parts.view);
            continue;
        }
        let full = full(slot);
        for (part, on) in [(parts.full_border, full), (parts.back_image, !full), (parts.background, !full), (parts.empty_border, !full)] {
            if let Some(part) = part {
                set.insert(part, on);
            }
        }
        let text = parts.header.and_then(|header| (header..gear.nodes.len()).find(|&below| gear.nodes[below].text.is_some() && gear.nodes[below].path.starts_with(&gear.nodes[header].path)));
        if let Some(text) = text {
            words.insert(text, said.say(slot, slot).trim().to_uppercase());
        }
    }
    let marks = Marks { set: Some(&set), skip: &skip, words: Some(&words), ..Marks::default() };
    draw_nodes(column, look, gear, &look.gear, GEAR, marks, said, Frame::Is(size), origin, s);
}

/// Draws the left panel's foot (`Bottom Bar`), which lies under both tabs'
/// panels, as the game's (`TARKOV UI`).
pub fn draw_foot(column: &mut ChildSpawnerCommands, look: &HealthLook, said: &Words, left: [f32; 2], origin: [f32; 2], s: f32) {
    let Some(foot) = look.panel.as_ref().and_then(|game| game.foot.as_ref()) else { return };
    draw_nodes(column, look, foot, &look.foot, FOOT, Marks::default(), said, Frame::In(left), origin, s);
}

/// The left panel's own ground (`TARKOV UI`): its picture's colour.
pub fn left_ground(look: &HealthLook) -> Option<Color> {
    look.panel.as_ref()?.left_ground.map(rgba)
}

/// Draws the left side's frame round its two panels (`Border`), over
/// them, as the game's (`TARKOV UI`): `side` is the left side's size and
/// `origin` its corner, in units, in what it is drawn into.
pub fn draw_border(column: &mut ChildSpawnerCommands, look: &HealthLook, said: &Words, side: [f32; 2], origin: [f32; 2], s: f32) {
    let Some(border) = look.panel.as_ref().and_then(|game| game.border.as_ref()) else { return };
    draw_nodes(column, look, border, &look.border, BORDER, Marks::default(), said, Frame::In(side), origin, s);
}

/// Draws the game's bar of tabs (`TARKOV UI`) into a node as large as the
/// window (`screen`, units): its ground, then each tab over the one before
/// it, as its chosen version or its idle one (`Tab.UpdateVersion`) and,
/// idle, its graphic under the mouse or its own (`Tab.Hover`), with its
/// words and its fill cut to its mask (`fills`, by the mask's object); and
/// the app's own tab (its words `own`) drawn as the first tab is, without
/// its icon, at the bar's other end as far in. `chosen` and `hovered` are
/// tabs' places in the row, the app's own after the game's.
#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_arguments)]
pub fn draw_tab_bar(column: &mut ChildSpawnerCommands, look: &HealthLook, said: &Words, screen: [f32; 2], chosen: Option<usize>, hovered: Option<usize>, own: &str, dimmed: &[bool], fills: &HashMap<usize, Handle<Image>>, s: f32) {
    let Some(bar) = &look.tab_bar else { return };
    let tabs: Vec<usize> = bar.tabs.iter().map(|tab| tab.object).collect();
    let masks: Vec<usize> = look.tab_fills.keys().copied().collect();
    draw_nodes(column, look, &bar.panel, &look.tabs_drawn, TABS, Marks { skip: &tabs, ..Marks::default() }, said, Frame::In(screen), [0.0, 0.0], s);
    let draw = |column: &mut ChildSpawnerCommands, place: usize, tab: &trp_data::screen::TabParts, words: &str, icon: bool, origin: [f32; 2], mirror: Option<f32>| {
        let (is_chosen, is_hovered) = (chosen == Some(place), hovered == Some(place));
        let mut set = HashMap::new();
        for (part, on) in [(tab.normal, !is_chosen), (tab.selected, is_chosen), (tab.hover, is_hovered), (tab.idle, !is_hovered)] {
            if let Some(part) = part {
                set.insert(part, on);
            }
        }
        if !icon {
            let path = &bar.panel.nodes[tab.object].path;
            for index in (0..bar.panel.nodes.len()).filter(|&index| bar.panel.nodes[index].name == "Icon" && bar.panel.nodes[index].path.starts_with(path.as_str())) {
                set.insert(index, false);
            }
        }
        let words: HashMap<usize, String> = tab.labels.iter().map(|&label| (label, words.to_owned())).collect();
        // The prestige tab does not work in a raid (`InventoryScreen.Show`:
        // `SetInteractable(... && !InRaid)`): the game draws its group at
        // 0.3 (`SetUnlockStatus`), the whole tab see-through. The tabs the
        // app has not built are dimmed until they are (the user,
        // 2026-10-16: "Let's dim out the tabs that aren't made yet like the
        // prestige one"), and, the user's look for all five, by their words
        // and icon at 0.3 alone, the tab solid ("Dont make the tabs
        // transparent, sorry. just dim them out so they don't overlap
        // eachother").
        let mut groups: HashMap<usize, f32> = HashMap::new();
        if dimmed.get(place).copied().unwrap_or(false) {
            let path = &bar.panel.nodes[tab.object].path;
            let icons = (0..bar.panel.nodes.len()).filter(|&index| bar.panel.nodes[index].name == "Icon" && bar.panel.nodes[index].path.starts_with(path.as_str()));
            for part in tab.labels.iter().copied().chain(icons) {
                groups.insert(part, UNLOCKED_NOT);
            }
        }
        let marks = Marks { set: Some(&set), words: Some(&words), within: &[tab.object], skip_below: &masks, fills: Some(fills), groups: Some(&groups), mirror, ..Marks::default() };
        draw_nodes(column, look, &bar.panel, &look.tabs_drawn, TABS + ((place + 1) << 16), marks, said, Frame::In(screen), origin, s);
    };
    // The app's own tab, the options, after the game's last as the row
    // would lay another (the user's place for it, at the bar's right; the
    // game's back button keeps the bar's end), mirrored, its slanted edge to
    // the left (the user, 2026-10-16: "Flip the Options tab square so the
    // edge is on the left").
    let options = |column: &mut ChildSpawnerCommands| {
        if let Some(first) = bar.tabs.first() {
            let [x, _, w, _] = bar.panel.placed_in(first.object, screen).rect;
            let origin = [options_tab_left(look, screen) - x, 0.0];
            draw(column, bar.tabs.len(), first, own, false, origin, Some(x + w / 2.0));
        }
    };
    // In the order the game's tab group leaves them (`TabGroup.SelectTab`:
    // every tab put last in turn from the last to the first, then the one
    // chosen): each tab over the one after it, its slanted end on the
    // next, and the one chosen over all. The app's, as a ninth, under all.
    if chosen != Some(bar.tabs.len()) {
        options(column);
    }
    for (place, tab) in bar.tabs.iter().enumerate().rev().filter(|(place, _)| chosen != Some(*place)) {
        draw(column, place, tab, look.tab_words.get(place).map_or("", String::as_str), true, [0.0, 0.0], None);
    }
    match chosen {
        Some(place) if place == bar.tabs.len() => options(column),
        Some(place) => {
            if let Some(tab) = bar.tabs.get(place) {
                draw(column, place, tab, look.tab_words.get(place).map_or("", String::as_str), true, [0.0, 0.0], None);
            }
        }
        None => {}
    }
    if let Some(back) = &bar.back {
        draw_nodes(column, look, back, &look.back_drawn, BACK_BUTTON, Marks::default(), said, Frame::In(screen), [0.0, 0.0], s);
    }
}

/// The alpha of a tab that does not work (`CanvasGroup.SetUnlockStatus`).
const UNLOCKED_NOT: f32 = 0.3;

/// The left of the app's options tab, in units: after the game's last tab
/// as the row would lay another, less the slant the mirrored tab now has
/// on its left (the outline's right border, `tab_idle_outline`'s 33
/// pixels, which the first tab's next overlaps by the row's gap), so that
/// its slant meets the last tab's.
fn options_tab_left(look: &HealthLook, screen: [f32; 2]) -> f32 {
    let Some(bar) = &look.tab_bar else { return 0.0 };
    let Some(first) = bar.tabs.first() else { return 0.0 };
    let [x, ..] = bar.panel.placed_in(first.object, screen).rect;
    x + (look.tab_width + bar.gap) * bar.tabs.len() as f32 - bar.gap
}

/// Where the screen's back button takes the mouse (`TARKOV UI`), in units
/// from the screen's top left corner.
pub fn back_place(look: &HealthLook, screen: [f32; 2]) -> Option<[f32; 4]> {
    let back = look.tab_bar.as_ref()?.back.as_ref()?;
    Some(back.placed_in(0, screen).rect)
}

/// The game's filters of a container's panel, in their tabs' order
/// (`FilterPanel._filterRules`): the game's item classes each stands for,
/// by the install's database's nodes of the same names; all for the
/// first, none for the last (what was found in the raid, which the app
/// does not keep).
pub const FILTERS: [&[&str]; 18] = [
    &[],
    &["Weapon"],
    &["Magazine"],
    &["Ammo", "AmmoBox"],
    &["Meds"],
    &["FoodAndDrink", "Food", "Drink"],
    &["Knife"],
    &["Mod"],
    &["ThrowWeap"],
    &["BarterItem"],
    &["Vest"],
    &["Visors"],
    &["SearchableItem", "SimpleContainer"],
    &["Armor"],
    &["Info"],
    &["Key"],
    &["SpecItem"],
    &[],
];

/// Where the panel for a container opened (`TARKOV UI`) puts things, in the
/// stash panel's frame of that size (units, from its top left): what of the
/// grids shows (its mask), where the wheel scrolls them (its scroll view),
/// the corner the grids are laid from (the scroll view's content, with its
/// column's padding), its bar, and its scrolling.
pub struct StashPlaces {
    pub mask: [f32; 4],
    pub view: [f32; 4],
    pub content: [f32; 2],
    pub bar: Option<([f32; 4], trp_data::screen::ScrollBar)>,
    pub scrolling: trp_data::screen::Scrolling,
}

pub fn stash_places(look: &HealthLook, size: [f32; 2]) -> Option<StashPlaces> {
    let stash = look.stash.as_ref()?;
    let scrolling = stash.scrolling?;
    let scroll = stash.scroll?;
    let mask = stash.panel.nodes[scroll].parent.map_or_else(|| stash.panel.rect(scroll, size), |mask| stash.panel.rect(mask, size));
    let view = stash.panel.rect(scrolling.view.unwrap_or(scroll), size);
    let content = scrolling.content.or(stash.grids).map_or([view[0], view[1]], |content| {
        let [x, y, ..] = stash.panel.rect(content, size);
        let padding = stash.panel.nodes[content].group.map_or([0.0; 4], |group| group.padding);
        [x + padding[0], y + padding[2]]
    });
    let bar = scrolling.bar.map(|bar| (stash.panel.rect(bar.object, size), bar));
    Some(StashPlaces { mask, view, content, bar, scrolling })
}

/// Draws the panel for a container opened (`TARKOV UI`) as the game's in a
/// raid, into the stash panel's place of that size: under the grids
/// (`layer` 0) its ground, its header with what it holds (the install's
/// word for `LOOT`); over them (1) its frame, its column of filters (all
/// chosen; those of a kind the container holds nothing of at 0.15,
/// `present`) and the container's name in its box, sized to the name as
/// the game's text sizes it (`ContentSizeFitter`).
#[allow(clippy::too_many_arguments)]
pub fn draw_stash(column: &mut ChildSpawnerCommands, look: &HealthLook, said: &Words, size: [f32; 2], name: &str, present: &[bool], fills: &HashMap<usize, Handle<Image>>, layer: u8, s: f32) {
    let Some(stash) = &look.stash else { return };
    let mut panel = stash.panel.clone();
    // The name's box, as wide and high as its words with its padding.
    if let (Some(name_panel), Some(text), Some(font)) = (stash.name_panel, stash.name, look.name_font.as_ref()) {
        if let Some(look_of) = panel.nodes[text].text.clone() {
            let words = if look_of.style & 16 != 0 { name.to_uppercase() } else { name.to_owned() };
            let own = [font.width(&words, look_of.size, look_of.spacing, false) + look_of.margin[0] + look_of.margin[2], font.height(look_of.size)];
            panel.nodes[text].element = Some(trp_data::screen::Element { ignore: false, min: own, preferred: own, flexible: [-1.0, -1.0] });
            let padding = panel.nodes[name_panel].group.map_or([0.0; 4], |group| group.padding);
            panel.nodes[name_panel].layout.size_delta = [own[0] + padding[0] + padding[1], own[1] + padding[2] + padding[3]];
        }
    }
    let mut set = HashMap::new();
    let mut alphas = HashMap::new();
    for (place, tab) in stash.filter_tabs.iter().enumerate() {
        for (part, on) in [(tab.normal, place != 0), (tab.selected, place == 0)] {
            if let Some(part) = part {
                set.insert(part, on);
            }
        }
        if !present.get(place).copied().unwrap_or(place == 0) {
            for part in [tab.target, tab.icon].into_iter().flatten() {
                alphas.insert(part, 0.15);
            }
        }
    }
    let mut words = HashMap::new();
    if let Some(grid_name) = stash.grid_name {
        words.insert(grid_name, said.say("LOOT", "LOOT"));
    }
    if let Some(text) = stash.name {
        words.insert(text, name.to_owned());
    }
    let over: Vec<usize> = (0..panel.nodes.len()).filter(|&index| panel.nodes[index].parent == Some(stash.simple) && ["Border"].contains(&panel.nodes[index].name.as_str())).chain(stash.filters).chain(stash.name_panel).collect();
    let marks = match layer {
        0 => Marks { set: Some(&set), words: Some(&words), within: &[stash.simple], skip: &over, fills: Some(fills), alphas: Some(&alphas), ..Marks::default() },
        _ => Marks { set: Some(&set), words: Some(&words), within: &over, fills: Some(fills), alphas: Some(&alphas), ..Marks::default() },
    };
    draw_nodes(column, look, &panel, &look.stash_drawn, STASH + ((layer as usize) << 16), marks, said, Frame::Is(size), [0.0, 0.0], s);
}

/// The crate panel's stripes cut to their masks (`TARKOV UI`), as images so
/// many pixels to a unit, by the mask's object, in a stash panel so large.
pub fn stash_fill_images(look: &HealthLook, size: [f32; 2], s: f32, images: &mut Assets<Image>) -> HashMap<usize, Handle<Image>> {
    let Some(stash) = &look.stash else { return HashMap::new() };
    fill_images(&look.stash_fills, |mask| stash.panel.rect(mask, size), s, images)
}

/// Fills cut to their masks as images, each as large as its mask's object
/// (`rect`, units) at so many pixels to a unit.
fn fill_images(fills: &HashMap<usize, std::sync::Arc<MaskedFill>>, rect: impl Fn(usize) -> [f32; 4], s: f32, images: &mut Assets<Image>) -> HashMap<usize, Handle<Image>> {
    fills
        .iter()
        .map(|(&mask, fill)| {
            let [_, _, w, h] = rect(mask);
            let pixels = [((w * s).round() as u32).max(1), ((h * s).round() as u32).max(1)];
            let image = Image::new(
                bevy::render::render_resource::Extent3d { width: pixels[0], height: pixels[1], depth_or_array_layers: 1 },
                bevy::render::render_resource::TextureDimension::D2,
                fill.compose([w, h], pixels),
                bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
                bevy::asset::RenderAssetUsages::RENDER_WORLD,
            );
            (mask, images.add(image))
        })
        .collect()
}

/// Draws the quick-use panel (`TARKOV UI`) as the game's into its place of
/// that size: its header (its words the install's), and each view with
/// its key's words (`HotKey`, the binding's keys) and the name of what is
/// bound to it (`Caption`, its short name; none for nothing), by the
/// binding's name in the game's `EBoundItem` (`words`). Under what is put
/// in the views (`layer` 0) the panel but each view's caption, key label
/// and border, which lie over it (1).
#[allow(clippy::too_many_arguments)]
pub fn draw_quick(column: &mut ChildSpawnerCommands, look: &HealthLook, said: &Words, size: [f32; 2], words: &HashMap<String, (String, String)>, layer: u8, s: f32) {
    let Some(quick) = &look.quick else { return };
    draw_quick_views(column, look, quick, &look.quick_drawn, QUICK, said, size, words, None, layer, s);
}

/// Draws a quick-use panel's views (the inventory's, or the battle
/// screen's bar), as `draw_quick` draws the inventory's; `selected`, the
/// keys of the views whose thing is in the hands, which a view lights
/// where it has a ground for it (`QuickSlotView.SwitchVisualSelection`:
/// the ground white and the caption black where selected, else the ground
/// black at 0.6 and the caption white; the caption in the normal style).
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_quick_views(
    column: &mut ChildSpawnerCommands,
    look: &HealthLook,
    quick: &trp_data::screen::QuickViews,
    drawn: &Drawn,
    base: usize,
    said: &Words,
    size: [f32; 2],
    words: &HashMap<String, (String, String)>,
    selected: Option<&[String]>,
    layer: u8,
    s: f32,
) {
    let mut colors = HashMap::new();
    let mut unbold = Vec::new();
    if let Some(selected) = selected {
        for slot in quick.slots.iter().filter(|slot| slot.background.is_some()) {
            let lit = selected.contains(&slot.key);
            if let Some(ground) = slot.background {
                colors.insert(ground, if lit { Color::WHITE } else { Color::srgba(0.0, 0.0, 0.0, 0.6) });
            }
            if let Some(caption) = slot.caption {
                colors.insert(caption, if lit { Color::BLACK } else { Color::WHITE });
                unbold.push(caption);
            }
        }
    }
    let mut texts = HashMap::new();
    for slot in &quick.slots {
        let Some((key, caption)) = words.get(&slot.key) else { continue };
        if let Some(hot_key) = slot.hot_key {
            texts.insert(hot_key, key.clone());
        }
        if let Some(text) = slot.caption {
            texts.insert(text, caption.clone());
        }
    }
    let headers: Vec<usize> = quick.panel.find("Header/Text").into_iter().collect();
    let over: Vec<usize> = quick.slots.iter().flat_map(|slot| [slot.caption, slot.hot_key.and_then(|key| quick.panel.nodes[key].parent), slot.border]).flatten().collect();
    let marks = match layer {
        0 => Marks { words: Some(&texts), headers: &headers, skip: &over, colors: Some(&colors), unbold: &unbold, ..Marks::default() },
        _ => Marks { words: Some(&texts), headers: &headers, within: &over, colors: Some(&colors), unbold: &unbold, ..Marks::default() },
    };
    draw_nodes(column, look, &quick.panel, drawn, base + ((layer as usize) << 16), marks, said, Frame::Is(size), [0.0, 0.0], s);
}

/// Where each quick-use view puts its thing (`InstallPlace`), by its
/// binding's name, in the panel's place of that size (units).
pub fn quick_places(look: &HealthLook, size: [f32; 2]) -> Vec<(String, [f32; 4])> {
    let Some(quick) = &look.quick else { return Vec::new() };
    quick.slots.iter().filter_map(|slot| Some((slot.key.clone(), quick.panel.rect(slot.install?, size)))).collect()
}

/// Where the bar's tabs lie (`TARKOV UI`) in a window so large (units):
/// each tab's rectangle (left, top, width, height) in the row's order, the
/// app's own last, at the bar's other end.
pub fn tab_places(look: &HealthLook, screen: [f32; 2]) -> Vec<[f32; 4]> {
    let Some(bar) = &look.tab_bar else { return Vec::new() };
    let mut places: Vec<[f32; 4]> = bar.tabs.iter().map(|tab| bar.panel.placed_in(tab.object, screen).rect).collect();
    if let Some(&[_, top, width, height]) = places.first() {
        places.push([options_tab_left(look, screen), top, width, height]);
    }
    places
}

/// Each tab's fill cut to its mask (`TARKOV UI`), as an image so many
/// pixels to a unit, by the mask's object.
pub fn tab_fill_images(look: &HealthLook, screen: [f32; 2], s: f32, images: &mut Assets<Image>) -> HashMap<usize, Handle<Image>> {
    let Some(bar) = &look.tab_bar else { return HashMap::new() };
    fill_images(&look.tab_fills, |mask| bar.panel.placed_in(mask, screen).rect, s, images)
}

/// The containers' column's ground: the colour of its scroll view's
/// parent (`Scrollview Parent`, a plain image), which the page's column
/// takes as its own.
pub fn containers_ground(look: &HealthLook) -> Option<Color> {
    let views = look.containers.as_ref()?;
    let ground = views.panel.find("Scrollview Parent")?;
    views.panel.nodes[ground].image.map(|image| rgba(image.color))
}

/// One container slot's row as the game's row template draws it: its
/// header (the slot's name in the install's words, in capitals) and its
/// slot panel (its empty picture, `Slots/Equipment/<slot>`, its fill and
/// its borders, full or empty), with the slot panel's top left at `at`
/// (units, in what it is drawn into). The grids are drawn apart
/// (`draw_grid_view`), where `container_grids` puts them.
#[allow(clippy::too_many_arguments)]
pub fn draw_container_row(column: &mut ChildSpawnerCommands, look: &HealthLook, said: &Words, left: [f32; 2], slot: &str, full: bool, panel_shown: bool, at: [f32; 2], s: f32) {
    let Some(views) = &look.containers else { return };
    let (Some(header), Some(slot_panel)) = (views.header, views.slot_panel) else { return };
    let panel = &views.panel;
    let [x, y, ..] = panel.rect(slot_panel, left);
    let origin = [at[0] - x, at[1] - y];
    let path = &panel.nodes[slot_panel].path;
    let mut set = HashMap::new();
    let mut pictures = HashMap::new();
    for (name, on) in [("Full Border", full), ("Empty Border", !full), ("Background", !full)] {
        if let Some(part) = panel.find(&format!("{path}/{name}")) {
            set.insert(part, on);
        }
    }
    if let Some(back) = panel.find(&format!("{path}/BackImage")) {
        match look.container_backs.get(slot) {
            Some(picture) => {
                pictures.insert(back, *picture);
                set.insert(back, !full);
            }
            None => {
                set.insert(back, false);
            }
        }
    }
    let header_path = &panel.nodes[header].path;
    let arrow: Vec<usize> = panel.find(&format!("{header_path}/ArrowHolder")).into_iter().collect();
    let mut words = HashMap::new();
    if let Some(text) = (header..panel.nodes.len()).find(|&below| panel.nodes[below].text.is_some() && panel.nodes[below].path.starts_with(header_path.as_str())) {
        words.insert(text, said.say(slot, slot).trim().to_uppercase());
    }
    // A thing not shown in its slot (`Item.NotShownInSlot`: the pockets)
    // has the slot's panel off (`SlotView.Show`, its `_slotPlace`).
    let within = if panel_shown { vec![header, slot_panel] } else { vec![header] };
    let marks = Marks { set: Some(&set), words: Some(&words), within: &within, pictures: Some(&pictures), skip: &arrow, ..Marks::default() };
    draw_nodes(column, look, panel, &look.containers_drawn, CONTAINERS, marks, said, Frame::Is(left), origin, s);
}

/// The key of the special slots' header's words
/// (`SearchableSlotView.CreateSlots`).
const SPECIAL_SLOTS_HEADER: &str = "InventoryScreen/SpecialSlotsHeader";

/// A container's special slots as the game's row shows them
/// (`SearchableSlotView.CreateSlots`, its `_specSlotsPanel` turned on and
/// laid last in the row): the panel's header over its left (the words
/// `InventoryScreen/SpecialSlotsHeader` in capitals), and a view for each
/// slot (`_specSlotTemplate`, 64 units square, 10 apart in the panel's row):
/// its striped fill, its empty border a unit outside and its picture
/// (`SpecialSlot`) at its own size in its middle. `at` is the panel's top
/// left corner in units; each view's place is given back.
pub fn draw_special_slots(column: &mut ChildSpawnerCommands, look: &HealthLook, said: &Words, left: [f32; 2], at: [f32; 2], count: usize, s: f32) -> Vec<[f32; 4]> {
    let places: Vec<[f32; 4]> = (0..count).map(|n| [at[0] + n as f32 * (SPECIAL_SIDE + SPECIAL_GAP), at[1], SPECIAL_SIDE, SPECIAL_SIDE]).collect();
    let Some(views) = &look.containers else { return places };
    let panel = &views.panel;
    let ends = |end: &str| (0..panel.nodes.len()).find(|&index| panel.nodes[index].path.ends_with(end));
    if let (Some(template), Some(header)) = (ends("Slot Template/Special Slots Panel Template"), ends("Special Slots Panel Template/Header Panel/Header")) {
        let [x, y, ..] = panel.rect(template, left);
        let mut words = HashMap::new();
        if let Some(text) = (header..panel.nodes.len()).find(|&below| panel.nodes[below].text.is_some() && panel.nodes[below].path.starts_with(panel.nodes[header].path.as_str())) {
            words.insert(text, said.say(SPECIAL_SLOTS_HEADER, "Special slots").trim().to_uppercase());
        }
        let marks = Marks { words: Some(&words), within: &[header], ..Marks::default() };
        draw_nodes(column, look, panel, &look.containers_drawn, CONTAINERS + (1 << 16), marks, said, Frame::Is(left), [at[0] - x, at[1] - y], s);
    }
    let sprite = |end: &str| -> Option<Sprite> {
        let index = ends(&format!("Special Slot Template/{end}"))?;
        let picture = look.containers_drawn.pictures.get(index).copied().flatten()?;
        let image = look.images.get(picture.texture)?.clone()?;
        Some(Sprite { image, cut: picture.cut, size: [panel.nodes[index].layout.size_delta[0].max(picture.size[0]), panel.nodes[index].layout.size_delta[1].max(picture.size[1])], border: picture.border, color: panel.nodes[index].image.map_or(Color::WHITE, |image| rgba(image.color)) })
    };
    let (fill, border, back) = (sprite("Background"), sprite("Empty Border"), sprite("BackImage"));
    for &[x, y, w, h] in &places {
        if let Some(fill) = &fill {
            let node = Node { position_type: PositionType::Absolute, left: Val::Px(x * s), top: Val::Px(y * s), width: Val::Px(w * s), height: Val::Px(h * s), ..default() };
            let image = ImageNode { image: fill.image.clone(), color: fill.color, rect: Some(fill.cut), flip_y: true, image_mode: NodeImageMode::Tiled { tile_x: true, tile_y: true, stretch_value: s }, ..default() };
            column.spawn((node, image));
        }
        if let Some(border) = &border {
            draw_sprite(column, border, [(x - 1.0) * s, (y - 1.0) * s, (w + 2.0) * s, (h + 2.0) * s], 1.0);
        }
        if let Some(back) = &back {
            let [bw, bh] = back.size;
            draw_sprite(column, back, [(x + (w - bw) / 2.0) * s, (y + (h - bh) / 2.0) * s, bw * s, bh * s], s);
        }
    }
    places
}

/// A special slot's view's side and the gap between two in their panel's
/// row (`Special Slot Template`'s `LayoutElement`, the panel's
/// `HorizontalLayoutGroup`), in units.
const SPECIAL_SIDE: f32 = 64.0;
const SPECIAL_GAP: f32 = 10.0;

/// The game's view of one grid (`GridView`): its tile of cells and its
/// border, as large as the grid, its top left at `at`.
pub fn draw_grid_view(column: &mut ChildSpawnerCommands, look: &HealthLook, said: &Words, at: [f32; 2], size: [f32; 2], s: f32) {
    let Some(grid) = look.containers.as_ref().and_then(|views| views.grid.as_ref()) else { return };
    draw_nodes(column, look, grid, &look.grid_drawn, GRID, Marks::default(), said, Frame::Is(size), at, s);
}

/// Where a container's grids lie to the right of its slot panel, from the
/// row's grids' corner: its own layout's places where it has one with as
/// many grids (`TemplatedGridsView`), else the default row
/// (`GeneratedGridsView`'s row: each grid as large as its cells, 63 units
/// to a cell and one more, so far apart). Each left, top, width, height.
pub fn container_grids(look: &HealthLook, layout: &str, grids: &[[u32; 2]]) -> Vec<[f32; 4]> {
    if let Some((_, rects)) = look.layouts.get(layout).filter(|(_, rects)| rects.len() >= grids.len()) {
        return rects[..grids.len()].to_vec();
    }
    let row = look.containers.as_ref().and_then(|views| views.grids_row).unwrap_or_default();
    let mut x = row.padding[0];
    grids
        .iter()
        .map(|cells| {
            let size = [cells[0] as f32 * 63.0 + 1.0, cells[1] as f32 * 63.0 + 1.0];
            let rect = [x, row.padding[2], size[0], size[1]];
            x += size[0] + row.spacing;
            rect
        })
        .collect()
}

/// How far right of a container slot's panel its grids begin: the panel's
/// width and the row template's spacing (`Slot Template`'s row).
pub fn grids_after(look: &HealthLook) -> f32 {
    let Some(views) = &look.containers else { return 132.0 };
    let width = views.slot_panel.map_or(125.0, |panel| views.panel.nodes[panel].element.map_or(125.0, |e| e.min[0].max(0.0)));
    width + views.panel.nodes[views.template].group.map_or(7.0, |row| row.spacing)
}

/// A sliced picture: its texture and cut, its border (left, bottom,
/// right, top) in the sprite's pixels, the screen's pixels to one of
/// them, whether its middle is drawn, and its colour.
struct Slices {
    image: Handle<Image>,
    cut: Rect,
    border: [f32; 4],
    per: f32,
    fill_center: bool,
    color: Color,
    /// Mirrored across its frame's middle: each piece where its mirror
    /// image lies, its picture flipped.
    flip_x: bool,
    /// Its edges and middle repeated at the sprite's size, not stretched
    /// (`Image.Type.Tiled`).
    tiled: bool,
}

/// One piece of a sliced picture, by the drawn object it is of.
#[derive(Component)]
struct SliceOf(usize);

/// A sliced picture in a frame of that size, in pixels, as nine pieces:
/// the corners whole, the edges and the middle stretched (the texture
/// kept as the engine has it, its lowest row first, and drawn turned up).
fn nine_slice(frame: &mut ChildSpawnerCommands, slices: &Slices, size: [f32; 2], index: usize) {
    let [l, b, r, t] = slices.border;
    let (x0, y0, x1, y1) = (slices.cut.min.x, slices.cut.min.y, slices.cut.max.x, slices.cut.max.y);
    let (mut left, mut right, mut top, mut bottom) = (l * slices.per, r * slices.per, t * slices.per, b * slices.per);
    if left + right > size[0] && left + right > 0.0 {
        let k = size[0] / (left + right);
        (left, right) = (left * k, right * k);
    }
    if top + bottom > size[1] && top + bottom > 0.0 {
        let k = size[1] / (top + bottom);
        (top, bottom) = (top * k, bottom * k);
    }
    let columns = [(x0, x0 + l, 0.0, left), (x0 + l, x1 - r, left, size[0] - left - right), (x1 - r, x1, size[0] - right, right)];
    let rows = [(y1 - t, y1, 0.0, top), (y0 + b, y1 - t, top, size[1] - top - bottom), (y0, y0 + b, size[1] - bottom, bottom)];
    for (row, (ya, yb, at_y, h)) in rows.into_iter().enumerate() {
        for (column, (xa, xb, at_x, w)) in columns.into_iter().enumerate() {
            if (row == 1 && column == 1 && !slices.fill_center) || w <= 0.0 || h <= 0.0 || xb <= xa || yb <= ya {
                continue;
            }
            let at_x = if slices.flip_x { size[0] - at_x - w } else { at_x };
            let node = Node { position_type: PositionType::Absolute, left: Val::Px(at_x), top: Val::Px(at_y), width: Val::Px(w), height: Val::Px(h), ..default() };
            let mut piece = ImageNode { image: slices.image.clone(), color: slices.color, rect: Some(Rect::new(xa, ya, xb, yb)), flip_x: slices.flip_x, flip_y: true, ..default() };
            if slices.tiled && (row == 1 || column == 1) {
                piece.image_mode = NodeImageMode::Tiled { tile_x: column == 1, tile_y: row == 1, stretch_value: slices.per };
            }
            frame.spawn((SliceOf(index), node, piece));
        }
    }
}

/// What of a panel is drawn as its scripts have it: its rates' arrows,
/// the views' glows, the headers whose texts are keys to the install's
/// words; objects shown or hidden as the caller has them (`set`), those
/// not drawn with all under them (`skip`), and texts the caller writes
/// (`words`).
#[derive(Default)]
pub(crate) struct Marks<'a> {
    arrows: &'a [usize],
    glows: &'a [usize],
    headers: &'a [usize],
    set: Option<&'a HashMap<usize, bool>>,
    skip: &'a [usize],
    words: Option<&'a HashMap<usize, String>>,
    /// Only what lies in these objects (themselves among it), where any
    /// are named; and none of what lies under these.
    within: &'a [usize],
    skip_below: &'a [usize],
    /// Pictures the caller puts on objects (a slot's empty picture), drawn
    /// at their own size about the object's middle (`Image.SetNativeSize`).
    pictures: Option<&'a HashMap<usize, Picture>>,
    /// Images the caller has made for objects, drawn over the whole of
    /// each in place of what the object draws (a tab's fill cut to its
    /// mask, `MaskedFill`).
    fills: Option<&'a HashMap<usize, Handle<Image>>>,
    /// The alpha a script puts on objects' pictures (`ChangeImageAlpha`: a
    /// filter that does not work at 0.15).
    alphas: Option<&'a HashMap<usize, f32>>,
    /// The alpha a script puts on objects' groups (`CanvasGroup.alpha`: a
    /// tab that does not work at 0.3, `SetUnlockStatus`), which every
    /// picture and text in the object or under it is drawn at besides.
    groups: Option<&'a HashMap<usize, f32>>,
    /// Drawn mirrored across this line, in the panel's units before the
    /// origin is added (the app's options tab, its slanted edge to the
    /// left: the user's).
    mirror: Option<f32>,
    /// The colour a script puts on objects' pictures or texts (a quick
    /// view's ground and caption for the thing in the hands,
    /// `QuickSlotView.SwitchVisualSelection`), and texts it sets to the
    /// normal style (`fontStyle = Normal`).
    colors: Option<&'a HashMap<usize, Color>>,
    unbold: &'a [usize],
}

/// How a panel's top lies: by its own anchors in what it hangs from, so
/// large (the left panel), or as a rectangle of that size.
#[derive(Clone, Copy)]
pub(crate) enum Frame {
    In([f32; 2]),
    Is([f32; 2]),
}

/// Each object of one of the game's panels that shows, where the game lays
/// it out in a left panel of that size, its picture as the engine draws it
/// and its text, numbered from `base` among the drawn pieces.
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_nodes(column: &mut ChildSpawnerCommands, look: &HealthLook, panel: &trp_data::screen::ScenePanel, drawn: &Drawn, base: usize, marks: Marks, said: &Words, frame: Frame, origin: [f32; 2], s: f32) {
    let at = |x: f32, y: f32, w: f32, h: f32| Node {
        position_type: PositionType::Absolute,
        left: Val::Px((origin[0] + x) * s),
        top: Val::Px((origin[1] + y) * s),
        width: Val::Px(w * s),
        height: Val::Px(h * s),
        ..default()
    };
    for (index, node) in panel.nodes.iter().enumerate() {
        let mut visibility = match drawn.shown.get(index) {
            None | Some(Shown::Never) => continue,
            Some(Shown::Always) => Visibility::Inherited,
            Some(Shown::ByScript) => Visibility::Hidden,
        };
        // Left out with all under it, or shown as the caller has it; hidden
        // with all under it where the caller hides something above it (an
        // object off hides what hangs from it).
        let mut above = Some(index);
        let (mut skipped, mut inside, mut hidden) = (false, marks.within.is_empty(), false);
        while let Some(at) = above {
            skipped |= marks.skip.contains(&at) || (at != index && marks.skip_below.contains(&at));
            inside |= marks.within.contains(&at);
            hidden |= at != index && marks.set.and_then(|set| set.get(&at)) == Some(&false);
            above = panel.nodes[at].parent;
        }
        if skipped || !inside {
            continue;
        }
        let mut group = 1.0;
        let mut above = Some(index);
        while let Some(at) = above {
            group *= marks.groups.and_then(|groups| groups.get(&at)).copied().unwrap_or(1.0);
            above = panel.nodes[at].parent;
        }
        if let Some(&on) = marks.set.and_then(|set| set.get(&index)) {
            visibility = if on { Visibility::Inherited } else { Visibility::Hidden };
        }
        if hidden {
            visibility = Visibility::Hidden;
        }
        let piece = Piece(base + index);
        let mut placed = match frame {
            Frame::In(outer) => panel.placed_in(index, outer),
            Frame::Is(size) => panel.placed(index, size),
        };
        // A picture whose border is kept a pixel to a pixel
        // (`PixelPerfectSpriteScaler.UpdateScale`): its offsets from the
        // edges of what it hangs from, on the sides it scales
        // (`_sidesToScale`, all unless set), divided by the screen's scale,
        // so it lies that many pixels out, not units (a quick-use view's
        // shadowed frame, five pixels about it).
        if let (Some((_, fields)), Some(parent)) = (node.scripts.iter().find(|(class, _)| class == "PixelPerfectSpriteScaler"), node.parent) {
            let sides = fields.get(..4).and_then(|b| b.try_into().ok()).map_or(0xF, i32::from_le_bytes);
            let [px, py, pw, ph] = match frame {
                Frame::In(outer) => panel.placed_in(parent, outer).rect,
                Frame::Is(size) => panel.placed(parent, size).rect,
            };
            let [x, y, w, h] = placed.rect;
            let (mut left, mut top, mut right, mut bottom) = (x, y, x + w, y + h);
            let k = 1.0 / s.max(1e-3);
            if sides & 1 != 0 {
                top = py - (py - top) * k;
            }
            if sides & 2 != 0 {
                left = px - (px - left) * k;
            }
            if sides & 4 != 0 {
                bottom = py + ph + (bottom - (py + ph)) * k;
            }
            if sides & 8 != 0 {
                right = px + pw + (right - (px + pw)) * k;
            }
            placed.rect = [left, top, right - left, bottom - top];
        }
        if let Some(own) = marks.pictures.and_then(|pictures| pictures.get(&index)) {
            let [x, y, w, h] = placed.rect;
            placed.rect = [x + (w - own.size[0]) / 2.0, y + (h - own.size[1]) / 2.0, own.size[0], own.size[1]];
        }
        // A simple picture that keeps its sprite's shape
        // (`Image.preserveAspect`): narrowed or lowered to it, moved by the
        // object's pivot (`PreserveSpriteAspectRatio`).
        if let Some(picture) = node.image.filter(|image| image.preserve_aspect && image.kind == 0).and_then(|_| drawn.pictures.get(index).copied().flatten()) {
            let [x, y, w, h] = placed.rect;
            let (sprite, rect) = (picture.size[0] / picture.size[1].max(1e-3), w / h.max(1e-3));
            let pivot = node.layout.pivot;
            placed.rect = if sprite > rect {
                let height = w / sprite;
                [x, y + (h - height) * (1.0 - pivot[1]), w, height]
            } else {
                let width = h * sprite;
                [x + (w - width) * pivot[0], y, width, h]
            };
        }
        // Turned with what it hangs from (a slider's handle, turned to
        // point at its bar, and the arrow under it): its middle turned
        // about the pivot of each turned object above it, nearest first,
        // and it turned as much besides.
        let mut turn = node.layout.turn;
        let mut above = node.parent;
        while let Some(at) = above {
            let layout = panel.nodes[at].layout;
            if layout.turn.abs() > 1e-3 {
                let [ax, ay, aw, ah] = match frame {
                    Frame::In(outer) => panel.placed_in(at, outer).rect,
                    Frame::Is(size) => panel.placed(at, size).rect,
                };
                let pivot = [ax + layout.pivot[0] * aw, ay + (1.0 - layout.pivot[1]) * ah];
                let [x, y, w, h] = placed.rect;
                let (dx, dy) = (x + w / 2.0 - pivot[0], y + h / 2.0 - pivot[1]);
                let (sin, cos) = layout.turn.to_radians().sin_cos();
                placed.rect = [pivot[0] + dx * cos + dy * sin - w / 2.0, pivot[1] - dx * sin + dy * cos - h / 2.0, w, h];
                turn += layout.turn;
            }
            above = panel.nodes[at].parent;
        }
        if let Some(axis) = marks.mirror {
            let [x, y, w, h] = placed.rect;
            placed.rect = [2.0 * axis - x - w, y, w, h];
            placed.flip[0] = !placed.flip[0];
        }
        let [x, y, w, h] = placed.rect;
        if let Some(handle) = marks.fills.and_then(|fills| fills.get(&index)) {
            let fill = ImageNode { flip_x: placed.flip[0], ..ImageNode::new(handle.clone()).with_color(Color::WHITE.with_alpha(group)) };
            column.spawn((piece, at(x, y, w, h), EXACT, fill, visibility));
            continue;
        }
        // A mask that does not show its own picture (`Mask.showMaskGraphic`
        // off) draws nothing; what hangs from it is cut to it.
        let hidden_mask = node.scripts.iter().any(|(class, fields)| class == "Mask" && fields.first() == Some(&0));
        if let Some(image) = node.image.as_ref().filter(|_| !hidden_mask) {
            let mut color = marks.colors.and_then(|colors| colors.get(&index)).copied().unwrap_or_else(|| rgba(image.color));
            if let Some(&alpha) = marks.alphas.and_then(|alphas| alphas.get(&index)) {
                color = color.with_alpha(alpha);
            }
            if let (true, Some(alpha)) = (marks.glows.contains(&index), look.glow_alpha) {
                color = color.with_alpha(alpha);
            }
            color = color.with_alpha(color.alpha() * group);
            let own = marks.pictures.and_then(|pictures| pictures.get(&index).copied());
            let picture = own.or_else(|| drawn.pictures.get(index).copied().flatten()).and_then(|picture| Some((picture, look.images.get(picture.texture)?.clone()?)));
            match (image.sprite, picture) {
                // No sprite: the engine's plain white, coloured.
                (None, _) => {
                    column.spawn((piece, at(x, y, w, h), EXACT, BackgroundColor(color), visibility));
                }
                (Some(_), Some((picture, handle))) => {
                    // The cut's share of the rectangle, mirrored with it.
                    let mut c = picture.covers;
                    if placed.flip[0] {
                        c = [1.0 - c[2], c[1], 1.0 - c[0], c[3]];
                    }
                    if placed.flip[1] {
                        c = [c[0], 1.0 - c[3], c[2], 1.0 - c[1]];
                    }
                    let (dx, dy, dw, dh) = (x + w * c[0], y + h * (1.0 - c[3]), w * (c[2] - c[0]), h * (c[3] - c[1]));
                    let mut drawn = ImageNode { image: handle.clone(), color, rect: Some(picture.cut), flip_x: placed.flip[0], flip_y: !placed.flip[1], ..default() };
                    // Sliced: nine pieces, the border kept whole and its
                    // middle left out unless the image fills it
                    // (`Image.fillCenter`); the border a pixel to a pixel
                    // of the screen where the object keeps it so
                    // (`PixelPerfectSpriteScaler`), else a unit to a pixel
                    // of the sprite over its multiplier.
                    let [l, b, r, t] = picture.border;
                    // Tiled with a border (`Image.GenerateTiledSprite`):
                    // as sliced, the border whole, but its edges and its
                    // middle repeated at their own size, not stretched (the
                    // quick-use panel's header, its slanted end in its
                    // right border).
                    if (image.kind == 1 || image.kind == 2) && (l > 0.0 || b > 0.0 || r > 0.0 || t > 0.0) {
                        let pixel_perfect = node.scripts.iter().any(|(class, _)| class == "PixelPerfectSpriteScaler");
                        let per = if pixel_perfect { 1.0 } else { s / image.pixels_per_unit.max(1e-3) };
                        let slices = Slices { image: handle, cut: picture.cut, border: picture.border, per, fill_center: image.fill_center, color, flip_x: placed.flip[0], tiled: image.kind == 2 };
                        column.spawn((piece, at(dx, dy, dw, dh), EXACT, visibility)).with_children(|frame| nine_slice(frame, &slices, [dw * s, dh * s], base + index));
                        continue;
                    }
                    // Tiled: the sprite repeated at its own size, a unit
                    // to a pixel.
                    if image.kind == 2 {
                        drawn.image_mode = NodeImageMode::Tiled { tile_x: true, tile_y: true, stretch_value: s };
                    }
                    // Filled across from the left (`Image.Type.Filled`,
                    // horizontal, from the left): a clip as wide as the
                    // share filled, the picture whole inside it.
                    if image.kind == 3 && image.fill_method == 0 {
                        let clip = Node { overflow: Overflow::clip(), ..at(dx, dy, dw * image.fill_amount, dh) };
                        column.spawn((piece, FullWidth(dw * s), clip, EXACT, visibility)).with_children(|clip| {
                            let whole = Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), width: Val::Px(dw * s), height: Val::Px(dh * s), ..default() };
                            clip.spawn((PieceFill(base + index), whole, drawn));
                        });
                    } else if marks.arrows.contains(&index) {
                        column.spawn((piece, Bob((origin[1] + dy) * s, s), at(dx, dy, dw, dh), EXACT, drawn, visibility));
                    } else {
                        // Turned about its middle where the scene turns it
                        // (the sheath's picture, 45 degrees): the engine's
                        // counter-clockwise, Bevy's clockwise.
                        let mut piece = column.spawn((piece, at(dx, dy, dw, dh), EXACT, drawn, visibility));
                        if turn.abs() > 1e-3 {
                            piece.insert(UiTransform { rotation: Rot2::degrees(-turn), ..UiTransform::IDENTITY });
                        }
                    }
                }
                // A sprite that could not be read: left out.
                (Some(_), None) => {}
            }
        }
        if let Some(text) = &node.text {
            let justify = match text.horizontal {
                2 => JustifyContent::Center,
                4 => JustifyContent::FlexEnd,
                _ => JustifyContent::FlexStart,
            };
            let align = match text.vertical {
                256 => AlignItems::FlexStart,
                1024 => AlignItems::FlexEnd,
                _ => AlignItems::Center,
            };
            // A view's header says what the install's words have for its
            // own text (`LocalizedText`); every other text is as the scene
            // has it until a script writes to it.
            let mut said_now = match marks.words.and_then(|words| words.get(&index)) {
                Some(words) => words.clone(),
                None if marks.headers.contains(&index) => said.say(&text.text, &text.text),
                None => text.text.clone(),
            };
            if text.style & 16 != 0 {
                said_now = said_now.to_uppercase();
            }
            // In the game's own font where it is had.
            let style = if marks.unbold.contains(&index) { text.style & !1 } else { text.style };
            let font = TextFont { font: look.styled_font(panel, text.font, style), font_size: text.size * s, weight: weight_of(style), ..default() };
            let color = marks.colors.and_then(|colors| colors.get(&index)).copied().unwrap_or_else(|| rgba(text.color));
            let color = TextColor(color.with_alpha(color.alpha() * group));
            let frame = Node { justify_content: justify, align_items: align, ..at(x, y, w, h) };
            // (Its rim or shadow at the group's alpha as well.)
            let effect = look.text_effect(panel, text.font).map(|effect| effect.faded(group));
            let size = text.size * s;
            // As the game draws it, where its font asset is had: what a
            // script puts before the words (a figure's noughts), then the
            // words.
            if let Some((asset, material)) = look.tmp_of(panel, text) {
                let bold = style & 1 != 0;
                let run = |said: String| trp_data::tmp_text::TmpRun { text: said, size, color: screen_numbers(color.0), bold };
                let layout = trp_data::tmp_text::TmpLayout { em: text.size * s, spacing: text.spacing, kerning: text.kerning, wrap: None, align: text.horizontal & 7 };
                let words = TmpText { asset, material, runs: vec![run(String::new()), run(said_now)], layout };
                column.spawn((piece, frame, visibility)).with_children(|frame| {
                    tmp_words(frame, Node::default(), words, &mut |entity| {
                        entity.insert(PieceText(base + index));
                    });
                });
            } else {
                column.spawn((piece, frame, visibility)).with_children(|frame| {
                    effect_words(frame, Node::default(), &[(said_now, font, color.0)], effect, size, &mut |entity, run, _| {
                        match run {
                            None => entity.insert(PieceText(base + index)),
                            Some(_) => entity.insert(PieceSpan(base + index)),
                        };
                    });
                });
            }
        }
    }
}

/// The figures of the app's own panel: each one's field on the game's
/// panel, the key of its words in the install's texts and the app's word
/// where the install has none. Blood pressure, which the game never
/// writes (its `120/80` is the scene's own), is left out.
const FIGURES: [(&str, &str, &str); 6] = [
    ("OverallHealth", "Health", "HEALTH"),
    ("Hydration", "HYDRATION", "HYDRATION"),
    ("Energy", "ENERGY", "ENERGY"),
    ("Temperature", "Temperature", "TEMPERATURE"),
    ("Poisoning", "Poisoning", "POISONING"),
    ("Radiation", "Radiation", "RADIATION"),
];

/// The game's view of a part and its figures, in units: a view's bar
/// (its header is above it, its effects under it) and a figure's panel
/// (`probe healthpanel`).
const VIEW: [f32; 2] = [130.0, 19.0];
const HEADER: f32 = 21.0;
const FIGURE: [f32; 2] = [180.0, 46.0];

/// Where the app's own panel puts each part's view (in `Part::ALL`'s
/// order) and each figure (in `FIGURES`' order) where the game's panel
/// could not be read: the game's places on a 16:9 screen (read
/// 2026-10-15, at night, `probe healthpanel`), from the equipment panel's
/// top left corner, in units.
const OWN_PARTS: [[f32; 2]; 7] = [[325.1, 65.5], [196.1, 164.1], [196.1, 243.8], [362.2, 243.8], [29.8, 243.8], [315.1, 405.8], [75.1, 405.8]];
const OWN_FIGURES: [[f32; 2]; 6] = [[161.0, 612.0], [177.5, 729.0], [366.9, 729.0], [-14.6, 681.0], [177.5, 681.0], [366.9, 681.0]];

/// The app's own panel's parts: a part's figures and its bar's fill, by
/// the part's place in `Part::ALL`; a figure's value and its rate, by its
/// place in `FIGURES`.
#[derive(Component)]
struct OwnFigures(usize);
#[derive(Component)]
struct OwnFill(usize);
#[derive(Component)]
struct OwnValue(usize);
#[derive(Component)]
struct OwnRate(usize);

/// The app's colours for a part's bar: whole, hurt, badly hurt,
/// destroyed (the app's own since 0.10.0).
const GOOD: Color = Color::srgb(0.42, 0.60, 0.30);
const HURT: Color = Color::srgb(0.78, 0.60, 0.22);
const BAD: Color = Color::srgb(0.74, 0.22, 0.18);
const GONE: Color = Color::srgb(0.10, 0.10, 0.10);

fn shade(share: f32, destroyed: bool) -> Color {
    if destroyed {
        GONE
    } else if share > 0.6 {
        GOOD
    } else if share > 0.3 {
        HURT
    } else {
        BAD
    }
}

/// The app's own HEALTH panel (`TARKOV UI` off): a box for each part (its
/// name, its figures, a bar and the effects on it) and the figures, in the
/// app's style, each where the game's panel has it in a left panel of
/// that size, the panel's top left at `origin` (units, in what it is drawn
/// into); over the character's model, which the page draws where the gear
/// tab has it. Where the game's panel could not be read, at its places on
/// a 16:9 screen from the equipment panel's top left, `gear`.
#[allow(clippy::too_many_arguments)]
pub fn draw_own_panel(column: &mut ChildSpawnerCommands, look: &HealthLook, said: &Words, health: Option<&Health>, left: Option<[f32; 2]>, origin: [f32; 2], gear: [f32; 2], s: f32) {
    if health.is_none() {
        column.spawn(words("The options' HEALTH SYSTEM is off: the character cannot be hurt, and is taken as fed and watered.", 13.0, WORDS));
        return;
    }
    let game = look.panel.as_ref().zip(left);
    let place = |index: usize, fallback: [f32; 2]| match game {
        Some((game, left)) => {
            let [x, y, ..] = game.panel.rect(index, left);
            [origin[0] + x, origin[1] + y]
        }
        None => [gear[0] + fallback[0], gear[1] + fallback[1]],
    };
    let at = |[x, y]: [f32; 2], width: f32| Node { position_type: PositionType::Absolute, left: Val::Px(x * s), top: Val::Px(y * s), width: Val::Px(width * s), ..default() };
    for (n, part) in Part::ALL.iter().enumerate() {
        let view = game.and_then(|(game, _)| game.views.parts.get(n)).map(|view| view.view);
        let [x, y] = view.map_or([gear[0] + OWN_PARTS[n][0], gear[1] + OWN_PARTS[n][1]], |view| place(view, OWN_PARTS[n]));
        let node = Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), padding: UiRect::all(Val::Px(4.0)), border: UiRect::all(Val::Px(1.0)), ..at([x, y - HEADER], VIEW[0]) };
        // (A thing let go on the box is used on the part: `HealthDrop`.)
        column.spawn((HealthDrop(n), RelativeCursorPosition::default(), node, BorderColor::all(crate::inventory_view::LINE), BackgroundColor(crate::inventory_view::PANEL.with_alpha(0.85)))).with_children(|cell| {
            cell.spawn(words(said.part(*part), 10.0, FAINT));
            cell.spawn((OwnFigures(n), words("", 12.0, WORDS)));
            let track = Node { width: Val::Percent(100.0), height: Val::Px(5.0), ..default() };
            cell.spawn((track, BackgroundColor(crate::inventory_view::LINE))).with_children(|track| {
                track.spawn((OwnFill(n), Node { width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() }, BackgroundColor(GOOD)));
            });
            cell.spawn((EffectWords(n), words("", 10.0, FAINT)));
        });
    }
    for (n, (name, key, otherwise)) in FIGURES.iter().enumerate() {
        let figure = game.and_then(|(game, _)| game.views.parameter(name)).map(|figure| figure.panel);
        let [x, y] = figure.map_or([gear[0] + OWN_FIGURES[n][0], gear[1] + OWN_FIGURES[n][1]], |figure| place(figure, OWN_FIGURES[n]));
        // A box as a part's: its name and its rate over its figures.
        let node = Node {
            height: Val::Px(FIGURE[1] * s),
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::Center,
            row_gap: Val::Px(2.0),
            padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)),
            border: UiRect::all(Val::Px(1.0)),
            ..at([x, y], FIGURE[0])
        };
        column.spawn((node, BorderColor::all(crate::inventory_view::LINE), BackgroundColor(crate::inventory_view::PANEL.with_alpha(0.85)))).with_children(|cell| {
            cell.spawn(Node { justify_content: JustifyContent::SpaceBetween, ..default() }).with_children(|row| {
                row.spawn((words(said.say(key, otherwise).to_uppercase(), 10.0, FAINT), TextLayout::new_with_no_wrap()));
                row.spawn((OwnRate(n), words("", 10.0, FAINT), TextLayout::new_with_no_wrap()));
            });
            cell.spawn((OwnValue(n), words("", 15.0, WORDS), TextLayout::new_with_no_wrap()));
        });
    }
}

/// Keeps the app's own panel, every frame, and each part's effects in
/// words on either panel: the same figures as the game's panel shows.
#[allow(clippy::type_complexity)]
fn fill_own(
    controller: Res<Controller>,
    said: Res<Words>,
    mut texts: Query<(&mut Text, &mut TextColor, Option<&OwnFigures>, Option<&OwnValue>, Option<&OwnRate>, Option<&EffectWords>), Or<(With<OwnFigures>, With<OwnValue>, With<OwnRate>, With<EffectWords>)>>,
    mut fills: Query<(&OwnFill, &mut Node, &mut BackgroundColor)>,
) {
    let Some(health) = controller.health.as_ref() else { return };
    if texts.is_empty() {
        return;
    }
    let rows = figure_rows(health);
    for (mut text, mut color, figures, value, rate, effects) in &mut texts {
        let (now, tint) = if let Some(OwnFigures(n)) = figures {
            let state = health.part(Part::ALL[*n]);
            let shown = if state.maximum / state.current < 2.0 { state.current.floor() } else { state.current.ceil() };
            let words = if state.destroyed { format!("{shown} / {}   DESTROYED", state.maximum) } else { format!("{shown} / {}", state.maximum) };
            (words, WORDS)
        } else if let Some(OwnValue(n)) = value {
            let Some(&(_, [current, maximum], _, _, places, _)) = rows.iter().find(|row| row.0 == FIGURES[*n].0) else { continue };
            let (_, written) = figure(current, maximum, places, -1);
            // Temperature has no maximum written, as on the game's panel.
            (if places > 0 { written } else { format!("{written} / {maximum:.0}") }, WORDS)
        } else if let Some(OwnRate(n)) = rate {
            match rows.iter().find(|row| row.0 == FIGURES[*n].0).and_then(|row| row.5) {
                Some((value, _, rise_good)) if value != 0.0 => (format!("{value:+.2}/min"), if (value > 0.0) == rise_good { GOOD } else { BAD }),
                _ => (String::new(), FAINT),
            }
        } else if let Some(EffectWords(n)) = effects {
            let part = Part::ALL[*n];
            let mut names: Vec<String> = health
                .effects
                .iter()
                .filter(|e| e.part == part && e.existing() && !matches!(e.kind, Kind::Encumbered | Kind::OverEncumbered))
                .filter_map(|e| said.effect(e.kind))
                .collect();
            names.dedup();
            (names.join(", "), color.0)
        } else {
            continue;
        };
        if text.0 != now {
            text.0 = now;
        }
        if color.0 != tint {
            color.0 = tint;
        }
    }
    for (fill, mut node, mut back) in &mut fills {
        let state = health.part(Part::ALL[fill.0]);
        let share = if state.maximum > 0.0 { (state.current / state.maximum).clamp(0.0, 1.0) } else { 0.0 };
        let width = Val::Percent((share * 100.0).round());
        if node.width != width {
            node.width = width;
        }
        let tint = shade(share, state.destroyed);
        if back.0 != tint {
            back.0 = tint;
        }
    }
}

/// What a frame wants of a drawn object: shown or not, its colour, how
/// much of it is filled, its words and what goes before them.
#[derive(Default, Clone)]
struct Want {
    visible: Option<bool>,
    color: Option<Color>,
    fill: Option<f32>,
    before: Option<(String, Color)>,
    words: Option<(String, Color)>,
    /// An arrow: whether its figure moves (`SetBuffValue`'s `active`, its
    /// animator's `Active`), and how far up it is drawn, in units.
    moving: Option<bool>,
    raised: f32,
}

/// `ColorSchemeExtension.GetBodyPartColor` with the green-to-red scheme:
/// the share squared as a hue from red to green, full; black destroyed.
fn part_color(share: f32, destroyed: bool) -> Color {
    if destroyed {
        return Color::BLACK;
    }
    let share = share.clamp(0.0, 1.0);
    let [r, g, b] = hsv(share * share * 120.0, 1.0, 1.0);
    Color::srgb_u8(r, g, b)
}

/// `ColorUtilities.ColorFromHSV`: a hue in degrees, each channel a byte
/// rounded to the nearest (`Convert.ToByte`, a half to even).
pub(crate) fn hsv(hue: f32, saturation: f32, value: f32) -> [u8; 3] {
    let sector = (hue / 60.0).floor() as i32 % 6;
    let f = (hue / 60.0) as f64 - (hue / 60.0).floor() as f64;
    let value = value * 255.0;
    let byte = |x: f64| x.round_ties_even().clamp(0.0, 255.0) as u8;
    let v = byte(value as f64);
    let p = byte((value * (1.0 - saturation)) as f64);
    let q = byte(value as f64 * (1.0 - f * saturation as f64));
    let t = byte(value as f64 * (1.0 - (1.0 - f) * saturation as f64));
    match sector {
        0 => [v, t, p],
        1 => [q, v, p],
        2 => [p, v, t],
        3 => [p, q, v],
        4 => [t, p, v],
        _ => [v, p, q],
    }
}

/// A figure as `HealthParameterPanel.SetParameterValue` writes it: whole,
/// down where more than half is left and up where less (`F0`), or to so
/// many places (a half to even); and the noughts that make up the fewest
/// digits it is written with.
fn figure(current: f32, maximum: f32, places: i32, least: i32) -> (String, String) {
    let text = if places <= 0 {
        let whole = if current / maximum > 0.5 { current.floor() } else { current.ceil() };
        format!("{whole:.0}")
    } else {
        let scale = 10f64.powi(places);
        format!("{}", (current as f64 * scale).round_ties_even() / scale)
    };
    let short = (least - text.chars().count() as i32).max(0) as usize;
    ("0".repeat(short), text)
}

/// One of the figures the panel writes (`HealthParametersPanel.
/// UpdateMainHealthAndStats`): its field's name, its current and maximum,
/// its warning and whether it is reached from above, its places, and its
/// rate a minute with whether it shows it moving (short of its maximum)
/// and whether a rise is good.
type FigureRow = (&'static str, [f32; 2], f32, bool, i32, Option<(f32, bool, bool)>);

fn figure_rows(health: &Health) -> [FigureRow; 6] {
    let rates = health.rates();
    let settings = &health.settings;
    let common = health.common();
    [
        ("OverallHealth", [common[0].round_ties_even(), common[1]], 10.0, false, 0, Some((rates[0] * 60.0, common[0] < common[1], true))),
        ("Hydration", [health.hydration[0], health.hydration[2]], 10.0, false, 0, Some((rates[2] * 60.0, health.hydration[0] < health.hydration[2], true))),
        ("Energy", [health.energy[0], health.energy[2]], 10.0, false, 0, Some((rates[1] * 60.0, health.energy[0] < health.energy[2], true))),
        ("Temperature", [settings.temperature[2], settings.temperature[1]], 10.0, false, 1, Some((0.0, settings.temperature[2] < settings.temperature[1], false))),
        ("Poisoning", [settings.poisoning[2], settings.poisoning[1]], 100.0, true, 0, None),
        ("Radiation", [0.0, 100.0], -1.0, false, 0, None),
    ]
}

/// The rate's colours (`BuffableHealthParameterPanel`).
const RISING: Color = Color::srgb(0.349, 0.667, 0.835);
const FALLING: Color = Color::srgb(0.729, 0.0, 0.0);

/// Keeps the panel as the game's scripts keep it: each part's view on
/// every frame (`BodyPartView.Update`), the figures once a second and as
/// they are first drawn (`HealthParametersPanel.Update`, on the clock of
/// the world outside).
#[allow(clippy::type_complexity)]
fn fill_panel(
    controller: Res<Controller>,
    look: Res<HealthLook>,
    time: Res<Time<Real>>,
    carried: Res<crate::inventory_view::Carried>,
    added: Query<(), Added<Piece>>,
    mut pieces: Query<(&Piece, &mut Visibility, Option<&mut ImageNode>, Option<&mut Node>, Option<&FullWidth>, Option<&Bob>)>,
    mut fills: Query<(&PieceFill, &mut ImageNode), Without<Piece>>,
    mut slices: Query<(&SliceOf, &mut ImageNode), (Without<Piece>, Without<PieceFill>)>,
    mut texts: Query<(&PieceText, &mut Text, &mut TextColor, Has<EffectCopy>), (Without<PieceSpan>, Without<EffectWords>)>,
    mut spans: Query<(&PieceSpan, &mut TextSpan, &mut TextColor, Has<EffectCopy>), Without<PieceText>>,
    mut figures: Local<(f64, HashMap<usize, Want>)>,
    mut drawn_texts: Query<(&PieceText, &mut TmpText)>,
) {
    let (Some(game), Some(health)) = (&look.panel, controller.health.as_ref()) else { return };
    if pieces.is_empty() {
        return;
    }
    let (panel, views) = (&game.panel, &game.views);
    let mut wants: HashMap<usize, Want> = HashMap::new();
    let mut want = |index: Option<usize>, change: &dyn Fn(&mut Want)| {
        if let Some(index) = index {
            change(wants.entry(index).or_default());
        }
    };
    // The parts' views.
    for (n, view) in views.parts.iter().enumerate() {
        let state = health.part(Part::ALL[n]);
        let (current, maximum, destroyed) = (state.current, state.maximum, state.destroyed);
        let share = if maximum > 0.0 { current / maximum } else { 0.0 };
        let shown = if maximum / current < 2.0 { current.floor() } else { current.ceil() };
        let value = (format!("{shown}/{maximum}"), if share > 0.15 { Color::WHITE } else { Color::srgb(1.0, 0.0, 0.0) });
        want(view.value, &|w| w.words = Some(value.clone()));
        let shape = if destroyed {
            Color::BLACK
        } else if share > 0.8 {
            Color::srgba(1.0, 0.0, 0.0, 0.0)
        } else {
            Color::srgba(1.0, 0.0, 0.0, 1.0 - share / 0.8)
        };
        want(view.shape, &|w| w.color = Some(shape));
        want(view.bar, &|w| {
            w.fill = Some(share.clamp(0.0, 1.0));
            w.color = Some(part_color(share / 0.8, destroyed));
        });
        let border = view.border.and_then(|border| panel.nodes[border].image.map(|image| rgba(image.color)));
        want(view.border, &|w| w.color = Some(if destroyed { Color::srgb(1.0, 0.0, 0.0) } else { border.unwrap_or(Color::WHITE) }));
        want(view.broken, &|w| w.visible = Some(destroyed));
    }
    // The figures, once a second, or as they are drawn anew: the health
    // panel's, and the GEAR tab's strip's with the carried weight
    // (`_weight`, written as its weight changes, to a place, against the
    // upper limit, and coloured by the two limits: `SetWarningColor`).
    let now = time.elapsed_secs_f64();
    if !added.is_empty() || now - figures.0 >= 1.0 {
        figures.0 = now;
        figures.1.clear();
        let weight = carried.equipment.carried_weight(false);
        let mut rows: Vec<FigureRow> = figure_rows(health).to_vec();
        if let Some([_, upper]) = look.weight_limits {
            rows.push(("Weight", [weight, upper], 0.0, false, 1, None));
        }
        let mut panels = vec![(panel, views.parameters.as_slice(), 0)];
        if let Some(strip) = &game.strip {
            panels.push((strip, game.strip_figures.as_slice(), STRIP));
        }
        for ((name, [current, maximum], threshold, from_top, places, rate), (panel, list, base)) in rows.iter().flat_map(|row| panels.iter().map(move |at| (*row, *at))) {
            let Some(f) = trp_data::screen::figure_named(list, name) else { continue };
            let scene_color = |text: Option<usize>| text.and_then(|text| panel.nodes[text].text.as_ref()).map_or(Color::WHITE, |text| rgba(text.color));
            let (default, max_color) = (scene_color(f.current), scene_color(f.max));
            let (noughts, text) = figure(current, maximum, places, f.least_digits);
            let mut color = default;
            let mut set = |index: Option<usize>, change: &dyn Fn(&mut Want)| {
                if let Some(index) = index {
                    change(figures.1.entry(base + index).or_default());
                }
            };
            if threshold >= 0.0 {
                let warned = if from_top { current as i32 as f32 >= threshold } else { current as i32 as f32 <= threshold };
                if warned {
                    color = rgba(f.warning);
                }
                set(f.icon, &|w| w.color = Some(color));
                set(f.glow, &|w| w.visible = Some(warned));
                set(f.ok_glow, &|w| w.visible = Some(!warned));
            } else {
                set(f.ok_glow, &|w| w.visible = Some(true));
            }
            // The weight's colour, after: the default under the lower
            // limit, the other colour under the upper, the warning over it.
            if let (true, Some([lower, upper])) = (name == "Weight", look.weight_limits) {
                color = if current <= lower {
                    default
                } else if current < upper {
                    rgba(f.alternative)
                } else {
                    rgba(f.warning)
                };
                set(f.icon, &|w| w.color = Some(color));
            }
            set(f.current, &|w| {
                w.before = Some((noughts.clone(), rgba(f.noughts)));
                w.words = Some((text.clone(), color));
            });
            set(f.max, &|w| w.words = Some((format!("/{maximum:.0}"), max_color)));
            if let Some((value, moving, rise_good)) = rate {
                let good = if rise_good { value >= f32::EPSILON } else { value <= -1e-45 };
                let (up, down) = if rise_good { (RISING, FALLING) } else { (FALLING, RISING) };
                set(f.rate, &|w| {
                    w.visible = Some(value != 0.0);
                    w.words = Some((format!("{value:.2}"), if good { RISING } else { FALLING }));
                });
                set(f.up, &|w| {
                    w.visible = Some(value > 0.0);
                    w.color = Some(up);
                    w.moving = Some(moving);
                });
                set(f.down, &|w| {
                    w.visible = Some(value < 0.0);
                    w.color = Some(down);
                    w.moving = Some(moving);
                });
            }
        }
    }
    for (index, figure) in &figures.1 {
        wants.insert(*index, figure.clone());
    }
    // The arrows of a moving figure bob as their clip has them, on the
    // world's clock; a still one's stand whole where they are laid out.
    let bob = look.bob.as_ref().map(|(clip, height, alpha)| {
        let mut values = Vec::new();
        clip.sample((now % clip.duration().max(1e-3) as f64) as f32, &mut values);
        (values.get(*height).copied().unwrap_or(0.0), values.get(*alpha).copied().unwrap_or(1.0))
    });
    for w in wants.values_mut() {
        if let (Some(true), Some((height, alpha))) = (w.moving, bob) {
            w.raised = height;
            w.color = w.color.map(|color| color.with_alpha(alpha));
        }
    }
    for (piece, mut visibility, image, node, full, bob) in &mut pieces {
        let Some(w) = wants.get(&piece.0) else { continue };
        let mut node = node;
        if let (Some(bob), Some(node)) = (bob, node.as_mut()) {
            let top = Val::Px(bob.0 - w.raised * bob.1);
            if node.top != top {
                node.top = top;
            }
        }
        if let Some(on) = w.visible {
            let now = if on { Visibility::Inherited } else { Visibility::Hidden };
            if *visibility != now {
                *visibility = now;
            }
        }
        if let (Some(color), Some(mut image)) = (w.color, image) {
            if image.color != color {
                image.color = color;
            }
        }
        if let (Some(fill), Some(mut node), Some(full)) = (w.fill, node, full) {
            let width = Val::Px(full.0 * fill);
            if node.width != width {
                node.width = width;
            }
        }
    }
    for (piece, mut image) in &mut fills {
        if let Some(color) = wants.get(&piece.0).and_then(|w| w.color) {
            if image.color != color {
                image.color = color;
            }
        }
    }
    for (piece, mut image) in &mut slices {
        if let Some(color) = wants.get(&piece.0).and_then(|w| w.color) {
            if image.color != color {
                image.color = color;
            }
        }
    }
    // A text drawn as the game's: what goes before its words, and its
    // words, each in its colour.
    for (piece, mut drawn) in &mut drawn_texts {
        let Some(w) = wants.get(&piece.0) else { continue };
        let mut runs = drawn.runs.clone();
        if let (Some((before, tint)), Some(first)) = (&w.before, runs.first_mut()) {
            (first.text, first.color) = (before.clone(), screen_numbers(*tint));
        }
        if let (Some((said, tint)), Some(last)) = (&w.words, runs.last_mut()) {
            (last.text, last.color) = (said.clone(), screen_numbers(*tint));
        }
        if runs != drawn.runs {
            drawn.runs = runs;
        }
    }
    // (A copy drawn as its font's rim or shadow takes the words, and keeps
    // its own colour.)
    for (piece, mut text, mut color, copy) in &mut texts {
        if let Some((before, tint)) = wants.get(&piece.0).and_then(|w| w.before.as_ref()) {
            if text.0 != *before {
                text.0 = before.clone();
            }
            if !copy && color.0 != *tint {
                color.0 = *tint;
            }
        }
    }
    for (piece, mut span, mut color, copy) in &mut spans {
        if let Some((words, tint)) = wants.get(&piece.0).and_then(|w| w.words.as_ref()) {
            if span.0 != *words {
                span.0 = words.clone();
            }
            if !copy && color.0 != *tint {
                color.0 = *tint;
            }
        }
    }
}

/// The app's keys for trying the health out (the user, 2026-10-16: "Can we
/// add a hurt/death trigger somewhere? a bind or an area"; not the game's):
/// `F7` does a bullet's damage (`HURT_DAMAGE`, with the game's own chances
/// of a bleeding and a fracture) to the next part in turn, the limbs first
/// (`HURT_ORDER`), and `F8` kills as a bullet through the head does. The
/// line under the inventory's slots says what was hurt.
fn damage_keys(options: Res<crate::Options>, keys: Res<ButtonInput<KeyCode>>, mut controller: ResMut<Controller>, mut carried: ResMut<crate::inventory_view::Carried>, mut next: Local<usize>) {
    if !options.keys_live() {
        return;
    }
    let (hurt, kill) = (keys.just_pressed(KeyCode::F7), keys.just_pressed(KeyCode::F8));
    if !hurt && !kill {
        return;
    }
    let Some(health) = controller.health.as_mut().filter(|health| health.alive) else { return };
    if kill {
        let left = health.part(Part::Head).current;
        health.apply_damage(Part::Head, left + 1.0, DamageKind::Bullet);
        carried.note = "F8: killed (a bullet through the head)".to_owned();
        return;
    }
    let part = HURT_ORDER[*next % HURT_ORDER.len()];
    *next += 1;
    let done = health.apply_damage(part, HURT_DAMAGE, DamageKind::Bullet);
    carried.note = format!("F7: {} hurt by {done:.0}", part.name());
}

/// What `F7` does to a part, and the parts it goes to in turn.
const HURT_DAMAGE: f32 = 15.0;
const HURT_ORDER: [Part; 7] = [Part::LeftArm, Part::RightArm, Part::LeftLeg, Part::RightLeg, Part::Stomach, Part::Chest, Part::Head];

/// The damage a run nobody is at the keys of was told to do (`--damage`),
/// on its drawn frame, counted as the taps are: a bullet's, so that the
/// game's own chances of a bleeding or a fracture go with it.
fn damage_asked(options: Res<crate::Options>, mut controller: ResMut<Controller>, mut frame: Local<u32>) {
    *frame += 1;
    if !options.unattended() {
        return;
    }
    let Some(health) = controller.health.as_mut() else { return };
    for (_, part, amount) in options.damage.iter().filter(|(at, ..)| *at == *frame) {
        health.apply_damage(*part, *amount, DamageKind::Bullet);
    }
}

/// The death screen: over everything, shown once the character is dead
/// and its eyes have closed (`death`).
#[derive(Component)]
struct DeathScreen;
#[derive(Component)]
struct DeathCause;

fn spawn_death_screen(mut commands: Commands) {
    let whole = Node {
        position_type: PositionType::Absolute,
        width: Val::Percent(100.0),
        height: Val::Percent(100.0),
        flex_direction: FlexDirection::Column,
        align_items: AlignItems::Center,
        justify_content: JustifyContent::Center,
        row_gap: Val::Px(14.0),
        ..default()
    };
    commands.spawn((DeathScreen, whole, BackgroundColor(Color::srgba(0.02, 0.0, 0.0, 0.94)), GlobalZIndex(40), Visibility::Hidden)).with_children(|screen| {
        screen.spawn((DeathCause, words("", 30.0, Color::srgb(0.82, 0.24, 0.20))));
        screen.spawn(words("ENTER: the run again, where it began", 15.0, FAINT));
    });
}

fn show_death(
    controller: Res<Controller>,
    dying: Res<crate::death::Dying>,
    said: Res<Words>,
    mut screens: Query<&mut Visibility, With<DeathScreen>>,
    mut causes: Query<&mut Text, With<DeathCause>>,
) {
    let shut = dying.since().is_some_and(|since| since >= crate::death::SCREEN_AT);
    let dead = controller.health.as_ref().filter(|health| !health.alive && shut);
    let wanted = if dead.is_some() { Visibility::Inherited } else { Visibility::Hidden };
    for mut shown in &mut screens {
        if *shown != wanted {
            *shown = wanted;
        }
    }
    if let Some(health) = dead {
        let title = said.say("Killed in Action", "Killed in Action").to_uppercase();
        let cause = said.cause(health.cause.unwrap_or(DamageKind::Undefined));
        let now = format!("{title}\n{}{cause}", said.say("Killed by", "Killed by "));
        for mut text in &mut causes {
            if text.0 != now {
                text.0 = now.clone();
            }
        }
    }
}


/// The body's hit colliders that barbed wire is told of
/// (`BodyPartCollider`), each where its bone is, or halfway to the next
/// bone of its limb, and the part it is of. `PROVISIONAL`: the game's own
/// colliders (twenty-six, with their sizes, in the player's prefab) are not
/// read; these are the largest ones, at their bones.
const COLLIDERS: [(&str, Option<&str>, Part); 12] = [
    ("Base HumanHead", None, Part::Head),
    ("Base HumanSpine3", None, Part::Chest),
    ("Base HumanSpine2", None, Part::Chest),
    ("Base HumanPelvis", None, Part::Stomach),
    ("Base HumanLUpperarm", Some("Base HumanLForearm1"), Part::LeftArm),
    ("Base HumanLForearm1", Some("Base HumanLPalm"), Part::LeftArm),
    ("Base HumanRUpperarm", Some("Base HumanRForearm1"), Part::RightArm),
    ("Base HumanRForearm1", Some("Base HumanRPalm"), Part::RightArm),
    ("Base HumanLThigh1", Some("Base HumanLCalf"), Part::LeftLeg),
    ("Base HumanLCalf", Some("Base HumanLFoot"), Part::LeftLeg),
    ("Base HumanRThigh1", Some("Base HumanRCalf"), Part::RightLeg),
    ("Base HumanRCalf", Some("Base HumanRFoot"), Part::RightLeg),
];

/// Barbed wire on the range (`BarbedWire`, a `DamageTrigger`; health spec,
/// "Barbed wire"): while the character's capsule is in its box the speed
/// is held down and a sprint ended, and each collider in the box is cut by
/// how far it has moved.
fn barbed_wire(
    mut controller: ResMut<Controller>,
    rig: Option<Res<BodyRig>>,
    parts: Option<Res<BodyParts>>,
    bones: Query<&GlobalTransform>,
    range: Res<Range>,
    mut tallies: Local<Vec<BarbTally>>,
) {
    let controller = &mut *controller;
    let Some(health) = controller.health.as_mut().filter(|health| health.alive) else { return };
    let [low, high] = crate::range::BARBED_WIRE;
    let feet = controller.movement.position;
    let feet = Vec3::new(feet[0], feet[1], -feet[2]);
    // The capsule's footprint over the box, and the box's height over the
    // feet.
    let nearest = Vec2::new(feet.x.clamp(low.x, high.x), feet.z.clamp(low.z, high.z));
    let inside = nearest.distance(Vec2::new(feet.x, feet.z)) <= range.radius && feet.y < high.y && feet.y + controller.movement.capsule_height() > low.y;
    let (Some(rig), Some(parts)) = (rig, parts) else {
        // No body to find the colliders on: the capsule stands for them.
        controller.movement.set_in_barbed_wire(inside, inside);
        return;
    };
    if !inside {
        controller.movement.set_in_barbed_wire(false, false);
        return;
    }
    if tallies.len() != COLLIDERS.len() {
        *tallies = vec![BarbTally::default(); COLLIDERS.len()];
    }
    let at = |end: &str| rig.bone(&parts, end).and_then(|bone| bones.get(bone).ok()).map(GlobalTransform::translation);
    let mut cut = false;
    for ((bone, toward, part), tally) in COLLIDERS.into_iter().zip(tallies.iter_mut()) {
        let Some(from) = at(bone) else { continue };
        let middle = toward.and_then(at).map_or(from, |to| (from + to) / 2.0);
        let within = middle.cmpge(low).all() && middle.cmple(high).all();
        if !within {
            continue;
        }
        cut = true;
        if let Some(damage) = tally.cut(middle.to_array()) {
            health.apply_damage(part, damage, DamageKind::Barbed);
        }
    }
    controller.movement.set_in_barbed_wire(true, cut);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tab's fill cut to its mask (`MaskedFill`): the fill drawn where
    /// the mask is, nothing where it is clear, between texels as the
    /// graphics card filters them, and the fill tiled at its own size.
    #[test]
    fn a_tab_s_fill_is_cut_to_its_mask() {
        let picture = |w: f32, h: f32| Picture { texture: 0, cut: Rect::new(0.0, 0.0, w, h), covers: [0.0, 0.0, 1.0, 1.0], border: [0.0; 4], size: [w, h] };
        // A mask 4 by 4 whose left half is solid; a fill 2 by 1, red then
        // white.
        let mask: Vec<u8> = (0..16).flat_map(|at| [255, 255, 255, if at % 4 < 2 { 255 } else { 0 }]).collect();
        let fill = vec![255, 0, 0, 255, 255, 255, 255, 255];
        let masked = MaskedFill {
            mask: (std::sync::Arc::new(Pixels { rgba: mask, width: 4, height: 4, point: false }), picture(4.0, 4.0), 1.0),
            fill: (std::sync::Arc::new(Pixels { rgba: fill, width: 2, height: 1, point: true }), picture(2.0, 1.0), 1.0),
            color: [1.0; 4],
        };
        let out = masked.compose([4.0, 4.0], [4, 4]);
        let at = |column: usize, row: usize| &out[(row * 4 + column) * 4..(row * 4 + column) * 4 + 4];
        // Drawn on the left, the fill's red then white repeated; clear on
        // the right.
        assert_eq!(at(0, 0), [255, 0, 0, 255]);
        assert_eq!(at(1, 0), [255, 255, 255, 255]);
        assert_eq!(at(0, 3), [255, 0, 0, 255]);
        assert_eq!(at(2, 1), [0, 0, 0, 0]);
        assert_eq!(at(3, 2), [0, 0, 0, 0]);
        // Filtered between two texels, in linear light.
        let half = sample(&masked.mask.0, 2.0, 1.5);
        assert!((half[3] - 0.5).abs() < 1e-6, "{half:?}");
    }

    #[test]
    fn figures_are_written_as_the_panel_writes_them() {
        // Whole, down with more than half left, up with less; noughts to
        // three digits.
        assert_eq!(figure(75.6, 100.0, 0, 3), ("0".to_owned(), "75".to_owned()));
        assert_eq!(figure(0.4, 100.0, 0, 3), ("00".to_owned(), "1".to_owned()));
        assert_eq!(figure(363.0, 440.0, 0, 3), (String::new(), "363".to_owned()));
        // To a place, with no noughts asked for.
        assert_eq!(figure(36.6, 40.0, 1, -1), (String::new(), "36.6".to_owned()));
    }

    #[test]
    fn the_bars_go_from_green_to_red_by_the_square_of_the_share() {
        assert_eq!(hsv(120.0, 1.0, 1.0), [0, 255, 0]);
        assert_eq!(hsv(0.0, 1.0, 1.0), [255, 0, 0]);
        assert_eq!(hsv(60.0, 1.0, 1.0), [255, 255, 0]);
        // Half the share left: a quarter of the way from red to green.
        assert_eq!(hsv(0.25 * 120.0, 1.0, 1.0), [255, 128, 0]);
        assert_eq!(part_color(0.5, true), Color::BLACK);
    }
}

#[cfg(test)]
mod item_tests {
    use super::*;

    #[test]
    fn things_are_coloured_as_the_game_colours_them() {
        assert_eq!(item_ground("blue"), Color::srgba_u8(28, 65, 86, 77));
        assert_eq!(item_ground("violet"), Color::srgba_u8(76, 42, 85, 77));
        assert_eq!(item_ground("unknown"), Color::srgba_u8(127, 127, 127, 77));
        assert_eq!(stack_color(&["Ammo".to_owned()]), Color::srgb_u8(0xda, 0xda, 0xbc));
        assert_eq!(stack_color(&["Money".to_owned()]), Color::srgb_u8(0x8d, 0xb5, 0x00));
        assert_eq!(stack_color(&["Food".to_owned()]), Color::srgb_u8(0xb6, 0xc1, 0xc7));
    }
}
