//! Pixels, masked fills, colour spaces, the drawn pieces and the weight limits.

use super::*;

/// A texture's pixels as the engine keeps them (RGBA, its lowest row
/// first), and whether it is sampled at its nearest pixel.
pub struct Pixels {
    pub(super) rgba: Vec<u8>,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) point: bool,
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
    pub(super) mask: (std::sync::Arc<Pixels>, Picture, f32),
    pub(super) fill: (std::sync::Arc<Pixels>, Picture, f32),
    pub(super) color: [f32; 4],
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
pub(super) fn masked_fills(panel: &trp_data::interface::screen::ScenePanel, drawn: &Drawn, kept: &[Option<trp_data::unity::extract::TextureData>], decoded: &mut HashMap<usize, std::sync::Arc<Pixels>>) -> HashMap<usize, std::sync::Arc<MaskedFill>> {
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
pub(super) fn sample(pixels: &Pixels, u: f32, v: f32) -> [f32; 4] {
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
#[derive(Default, Clone)]
pub(crate) struct Drawn {
    pub(crate) shown: Vec<Shown>,
    pub(crate) pictures: Vec<Option<Picture>>,
}

/// Where each panel's objects are numbered among the drawn pieces: the
/// health panel's from 0, the strip's and the foot's past it.
pub(super) const STRIP: usize = 1 << 20;

pub(super) const FOOT: usize = 2 << 20;

pub(super) const GEAR: usize = 3 << 20;

pub(super) const CONTAINERS: usize = 4 << 20;

pub(super) const GRID: usize = 5 << 20;

pub(super) const BORDER: usize = 6 << 20;

pub(super) const TABS: usize = 7 << 20;

pub(super) const STASH: usize = 8 << 20;

pub(super) const QUICK: usize = 9 << 20;

pub(super) const BACK_BUTTON: usize = 10 << 20;

/// The windows that show a thing, each window's past the one before it.
pub(super) const INFO: usize = 14 << 20;

/// The menu of a right click.
pub(super) const MENU: usize = 15 << 20;

/// The windows that edit a tag, each window's past the one before it.
pub(super) const TAG_WINDOW: usize = 16 << 20;

/// The menu's lines' templates below its top (an ordinary line's, the wish
/// list's, Discard's: `SimpleSpecifiedContextButtons`), and what holds the
/// lines.
pub(super) const MENU_TEMPLATES: [&str; 3] = ["InteractionButtonsContainer/Button Template", "InteractionButtonsContainer/Wishlist Template", "InteractionButtonsContainer/Dispose Template"];

pub(super) const MENU_LINES: &str = "InteractionButtonsContainer";

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
    pub(crate) covers: [f32; 4],
    /// How far in from each side a sliced image keeps the sprite whole
    /// (left, bottom, right, top), in its pixels.
    pub(super) border: [f32; 4],
    /// The sprite's own size, untrimmed, a unit to a pixel.
    pub(super) size: [f32; 2],
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

/// The experience each level takes beyond the one before
/// (`GlobalConfiguration.Experience.Level.Table`, `exp.level.exp_table`),
/// the first level's first.
pub fn level_experience(config: &serde_json::Value) -> Vec<u32> {
    let table = config.get("exp").and_then(|exp| exp.get("level")).and_then(|level| level.get("exp_table")).and_then(|table| table.as_array());
    table.map_or_else(Vec::new, |rows| rows.iter().filter_map(|row| row.get("exp")?.as_u64().map(|v| v as u32)).collect())
}
