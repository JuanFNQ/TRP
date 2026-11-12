//! A text drawn as TextMeshPro draws it in the game.
//! After `docs/spec/inventory.md`, "A text drawn as the game's": laid out by its font asset's glyphs, and
//! each glyph's quad shaded from the asset's signed distance field by the
//! game's material through the `TextMeshPro/Distance Field` shader's
//! formulas, the quads laid one over another in order, as the screen draws
//! them. The words come out as an image of their own.
//!
//! The layout (TextMeshPro's `GenerateTextMesh`): each letter's glyph at the
//! pen, by its bearings, the pen moved by its advance and, after every
//! letter, by what the asset adds (`normalSpacingOffset`), the text's own
//! spacing (`m_characterSpacing`) and, for a letter made bold by the shader,
//! the asset's `boldSpacing`, each in hundredths of the size; the font's
//! kerning between two letters where the text keeps it. A bold letter is
//! drawn in the asset's bold typeface where it has one (its weight table's
//! 700), else made bold by the shader (`_WeightBold`).
//!
//! The shader (its vertex and fragment programs, as TextMeshPro publishes
//! them): with `c` the field's value at the pixel (a half on the glyph's
//! edge, rising a `_GradientScale`'s share of a half for each of the
//! atlas's pixels inward), `scale` = √2 · (the screen's pixels to the
//! atlas's) · `_GradientScale` · (1 + `_Sharpness`) (the screen space's
//! measure of a pixel, `rsqrt(dot(pixelSize, pixelSize))`, comes to √2 on a
//! screen's canvas), `weight` = (`_WeightNormal` or `_WeightBold` over four,
//! plus `_FaceDilate`) · `_ScaleRatioA` / 2, `bias` = ½ − `weight` +
//! ½ / `scale`, `sd` = (`bias` − `c`) · `scale`, `outline` = `_OutlineWidth`
//! · `_ScaleRatioA` · `scale`, `softness` likewise of `_OutlineSoftness`:
//! the face's alpha is 1 − saturate((`sd` − `outline`/2 + `softness`/2) /
//! (1 + `softness`)), the outline's share saturate(`sd` + `outline`/2) ·
//! √min(1, `outline`), the colour the face's toward the outline's by that
//! share, times the face's alpha (each colour times its own alpha first);
//! under it the underlay, where on (`UNDERLAY_ON`), by the field at its
//! offset: saturate(`c` · `layerScale` − `layerBias`), `layerScale` =
//! `scale` / (1 + `_UnderlaySoftness` · `_ScaleRatioC` · `scale`),
//! `layerBias` = (½ − `weight`) · `layerScale` − ½ − `_UnderlayDilate` ·
//! `_ScaleRatioC` / 2 · `layerScale`, added where the face leaves room; the
//! whole times the letter's own alpha. The face's colour is the material's
//! times the letter's.

use std::collections::HashMap;
use std::sync::Arc;

use crate::interface::screen::{MaterialProperties, TmpFont, TmpGlyph};

mod face;
mod layout;

pub use face::*;
pub use layout::*;
use layout::glyph_index;

/// A run of words of one size, colour and weight: their size in the
/// screen's pixels, their colour as the screen's numbers (gamma), bold or
/// not.
#[derive(Clone, Debug, PartialEq)]
pub struct TmpRun {
    pub text: String,
    pub size: f32,
    pub color: [f32; 4],
    pub bold: bool,
}

/// How the words are laid out: the text's own size in pixels (`m_fontSize`,
/// which the spacings are hundredths of, whatever size a run is drawn at:
/// `currentEmScale`), the text's own spacing (hundredths of that size),
/// whether the font's kerning is applied, where lines are broken (a width
/// in pixels, else only at a line's end), and how each line lies across
/// the widest (left 1, centre 2, right 4).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TmpLayout {
    pub em: f32,
    pub spacing: f32,
    pub kerning: bool,
    pub wrap: Option<f32>,
    pub align: u32,
    /// Where the first line's first letter starts, in pixels: the room a
    /// `<sprite>` before it takes (an armour class's mark in a tooltip),
    /// drawn apart; none elsewhere.
    pub lead: f32,
}

/// The words drawn: the image's size in pixels, its pixels (straight alpha,
/// the screen's numbers, rows from the top), and where in it the words'
/// box lies as TextMeshPro aligns them (from the first line's ascender to
/// the last's descender, as wide as the widest line's advances without the
/// spacing after its last letter): left, top, width, height in pixels.
pub struct TmpImage {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
    pub layout: [f32; 4],
}

/// A glyph placed: its face, its glyph, where its quad lies (left, top, in
/// pixels from the first line's top, y down), the screen's pixels to its
/// atlas's, its colour and whether the shader makes it bold.
struct Placed<'a> {
    face: &'a TmpFace,
    glyph: TmpGlyph,
    at: [f32; 2],
    per: f32,
    color: [f32; 4],
    bold: bool,
}

/// One letter as laid out on its line, before the line is placed.
struct Letter<'a> {
    face: &'a TmpFace,
    glyph: Option<TmpGlyph>,
    letter: char,
    pen: f32,
    per: f32,
    size: f32,
    color: [f32; 4],
    bold: bool,
}

#[cfg(test)]
mod tests;
