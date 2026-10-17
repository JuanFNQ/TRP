//! A text drawn as TextMeshPro draws it in the game (`docs/spec/inventory.md`,
//! "A text drawn as the game's"): laid out by its font asset's glyphs, and
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

use crate::screen::{MaterialProperties, TmpFont, TmpGlyph};

/// A font asset ready to draw with: its glyphs, its first atlas's texels
/// (one byte each, rows from the bottom) and size, its font file's kerning
/// by pairs of glyphs (in the file's units to the em) and those units, and
/// the asset its bold is drawn with, where it has one.
pub struct TmpFace {
    pub font: TmpFont,
    pub atlas: Vec<u8>,
    pub atlas_size: [usize; 2],
    pub kerning: HashMap<(u16, u16), i16>,
    pub units_per_em: f32,
    pub bold: Option<Arc<TmpFace>>,
}

impl TmpFace {
    /// Made from its asset, its atlas's texture and its font file's bytes
    /// (for the kerning; none, none kept).
    pub fn new(font: TmpFont, atlas: &crate::unity::extract::TextureData, file: Option<&[u8]>) -> Self {
        let size = [atlas.width as usize, atlas.height as usize];
        let texels = match atlas.format {
            // Alpha8: one byte a texel.
            1 => atlas.data[..(size[0] * size[1]).min(atlas.data.len())].to_vec(),
            // Anything with four bytes a texel: its alpha.
            4 | 5 | 14 => atlas.data.chunks_exact(4).take(size[0] * size[1]).map(|p| if atlas.format == 5 { p[0] } else { p[3] }).collect(),
            _ => vec![0; size[0] * size[1]],
        };
        let (mut kerning, mut units_per_em) = (HashMap::new(), 1000.0);
        if let Some(face) = file.and_then(|bytes| ttf_parser::Face::parse(bytes, 0).ok()) {
            units_per_em = face.units_per_em() as f32;
            if let Some(kern) = face.tables().kern {
                for table in kern.subtables.into_iter().filter(|table| table.horizontal && !table.variable) {
                    if let ttf_parser::kern::Format::Format0(pairs) = &table.format {
                        for pair in pairs.pairs {
                            kerning.entry((pair.left().0, pair.right().0)).or_insert(pair.value);
                        }
                    }
                }
            }
        }
        Self { font, atlas: texels, atlas_size: size, kerning, units_per_em, bold: None }
    }

    /// The field's value at a place in the atlas (pixels from its lower
    /// left corner), blended between the four nearest texels.
    fn sample(&self, x: f32, y: f32) -> f32 {
        let [w, h] = self.atlas_size;
        let (x, y) = (x - 0.5, y - 0.5);
        let (x0, y0) = (x.floor(), y.floor());
        let (fx, fy) = (x - x0, y - y0);
        let at = |ix: f32, iy: f32| -> f32 {
            let (ix, iy) = (ix.clamp(0.0, w as f32 - 1.0) as usize, iy.clamp(0.0, h as f32 - 1.0) as usize);
            self.atlas[iy * w + ix] as f32 / 255.0
        };
        let top = at(x0, y0) * (1.0 - fx) + at(x0 + 1.0, y0) * fx;
        let bottom = at(x0, y0 + 1.0) * (1.0 - fx) + at(x0 + 1.0, y0 + 1.0) * fx;
        top * (1.0 - fy) + bottom * fy
    }
}

/// The material's figures the shader goes by.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TmpMaterial {
    pub face: [f32; 4],
    pub outline: [f32; 4],
    pub underlay: Option<[f32; 4]>,
    pub face_dilate: f32,
    pub outline_width: f32,
    pub outline_softness: f32,
    pub underlay_offset: [f32; 2],
    pub underlay_dilate: f32,
    pub underlay_softness: f32,
    pub weight_normal: f32,
    pub weight_bold: f32,
    pub ratio_a: f32,
    pub ratio_c: f32,
    pub gradient_scale: f32,
    pub sharpness: f32,
}

impl TmpMaterial {
    /// From the material's saved properties (the shader's defaults where it
    /// has none).
    pub fn of(props: &MaterialProperties) -> Self {
        let float = |name: &str, otherwise: f32| props.floats.get(name).copied().unwrap_or(otherwise);
        let color = |name: &str, otherwise: [f32; 4]| props.colors.get(name).copied().unwrap_or(otherwise);
        let underlay_on = props.keywords.iter().any(|keyword| keyword == "UNDERLAY_ON");
        Self {
            face: color("_FaceColor", [1.0; 4]),
            outline: color("_OutlineColor", [0.0, 0.0, 0.0, 1.0]),
            underlay: underlay_on.then(|| color("_UnderlayColor", [0.0, 0.0, 0.0, 0.5])),
            face_dilate: float("_FaceDilate", 0.0),
            outline_width: float("_OutlineWidth", 0.0),
            outline_softness: float("_OutlineSoftness", 0.0),
            underlay_offset: [float("_UnderlayOffsetX", 0.0), float("_UnderlayOffsetY", 0.0)],
            underlay_dilate: float("_UnderlayDilate", 0.0),
            underlay_softness: float("_UnderlaySoftness", 0.0),
            weight_normal: float("_WeightNormal", 0.0),
            weight_bold: float("_WeightBold", 0.5),
            ratio_a: float("_ScaleRatioA", 1.0),
            ratio_c: float("_ScaleRatioC", 1.0),
            gradient_scale: float("_GradientScale", 5.0),
            sharpness: float("_Sharpness", 0.0),
        }
    }
}

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

/// Lays out and draws the words.
pub fn draw(face: &TmpFace, material: &TmpMaterial, runs: &[TmpRun], layout: &TmpLayout) -> TmpImage {
    // The lines, each its letters at their pens and its width as it is
    // aligned by, ascender, descender. A line is aligned by where its last
    // letter that shows ends (`lineInfo.maxAdvance`: that letter's advance
    // with all that follows it, less the asset's and the text's spacing; a
    // space at its end not counted, a bold letter's spacing counted).
    let mut lines: Vec<(Vec<Letter>, f32, f32, f32)> = Vec::new();
    let mut letters: Vec<Letter> = Vec::new();
    let mut pen = 0.0f32;
    let mut line_end = 0.0f32;
    let (mut ascender, mut descender) = (0.0f32, 0.0f32);
    let mut previous: Option<(&TmpFace, u32)> = None;
    #[allow(clippy::type_complexity)]
    fn finish<'a>(letters: &mut Vec<Letter<'a>>, line_end: f32, ascender: f32, descender: f32, lines: &mut Vec<(Vec<Letter<'a>>, f32, f32, f32)>) {
        lines.push((std::mem::take(letters), line_end.max(0.0), ascender, descender));
    }
    for run in runs {
        // In the bold typeface where the asset has one; else the shader's.
        let (used, synthetic) = match (run.bold, &face.bold) {
            (true, Some(bold)) => (bold.as_ref(), false),
            (bold, _) => (face, bold),
        };
        let font = &used.font;
        let unit = run.size / font.point_size.max(1.0) * font.scale;
        ascender = ascender.max(font.ascent * unit);
        descender = descender.min(font.descent * unit);
        for letter in run.text.chars() {
            if letter == '\n' {
                finish(&mut letters, line_end, ascender, descender, &mut lines);
                (pen, line_end, previous) = (0.0, 0.0, None);
                (ascender, descender) = (font.ascent * unit, font.descent * unit);
                continue;
            }
            let Some((glyph, letter_scale)) = font.glyph_of(letter) else { continue };
            let per = unit * glyph.scale * letter_scale;
            // The font's kerning between this letter and the one before.
            if let (true, Some((before_face, before))) = (layout.kerning, previous) {
                if std::ptr::eq(before_face, used) {
                    if let Some(value) = used.kerning.get(&(before as u16, glyph_index(used, letter) as u16)) {
                        pen += *value as f32 * run.size / used.units_per_em;
                    }
                }
            }
            letters.push(Letter { face: used, glyph: Some(*glyph), letter, pen, per, size: run.size, color: run.color, bold: synthetic });
            let spacing = (font.normal_spacing + layout.spacing + if synthetic { font.bold_spacing } else { 0.0 }) * layout.em / 100.0;
            pen += glyph.advance * per + spacing;
            if !letter.is_whitespace() {
                line_end = pen - (font.normal_spacing + layout.spacing) * layout.em / 100.0;
            }
            previous = Some((used, glyph_index(used, letter)));
        }
    }
    finish(&mut letters, line_end, ascender, descender, &mut lines);
    // Broken at a width: a line's words carried down where it would pass it.
    if let Some(width) = layout.wrap {
        lines = lines.into_iter().flat_map(|line| wrap(line, width)).collect();
    }
    // The lines placed: each baseline a line's height under the one before
    // (the face's line height at the line's largest size), across by the
    // alignment within the widest.
    let widest = lines.iter().map(|line| line.1).fold(0.0f32, f32::max);
    let mut placed: Vec<Placed> = Vec::new();
    let mut baseline = lines.first().map_or(0.0, |line| line.2);
    let mut bottom = 0.0f32;
    for (index, (letters, width, ascender, descender)) in lines.iter().enumerate() {
        if index > 0 {
            let height = letters.iter().map(|l| l.face.font.line_height * l.size / l.face.font.point_size.max(1.0) * l.face.font.scale).fold(0.0f32, f32::max);
            baseline += if height > 0.0 { height } else { ascender - descender };
        }
        let shift = match layout.align {
            2 => (widest - width) / 2.0,
            4 => widest - width,
            _ => 0.0,
        };
        for letter in letters {
            let Some(glyph) = letter.glyph else { continue };
            placed.push(Placed { face: letter.face, glyph, at: [shift + letter.pen + glyph.bearing[0] * letter.per, baseline - glyph.bearing[1] * letter.per], per: letter.per, color: letter.color, bold: letter.bold });
            let _ = letter.letter;
        }
        bottom = baseline - descender;
    }
    // Each quad with room round it for what the shader draws outside the
    // glyph: the atlas's padding (its gradient's scale less one).
    let pad = material.gradient_scale - 1.0;
    let (mut x0, mut y0, mut x1, mut y1) = (0.0f32, 0.0f32, widest, bottom);
    for p in &placed {
        let [w, h] = [p.glyph.size[0] * p.per, p.glyph.size[1] * p.per];
        let room = pad * p.per;
        x0 = x0.min(p.at[0] - room);
        y0 = y0.min(p.at[1] - room);
        x1 = x1.max(p.at[0] + w + room);
        y1 = y1.max(p.at[1] + h + room);
    }
    let (left, top) = (x0.floor(), y0.floor());
    let (width, height) = (((x1.ceil() - left) as usize).max(1), ((y1.ceil() - top) as usize).max(1));
    let mut pixels = vec![[0.0f32; 4]; width * height];
    for p in &placed {
        shade(p, material, pad, [left, top], [width, height], &mut pixels);
    }
    // Straight alpha for the screen's blending.
    let rgba = pixels
        .iter()
        .flat_map(|[r, g, b, a]| {
            let a = a.clamp(0.0, 1.0);
            let straight = |v: f32| if a > 1e-5 { (v / a).clamp(0.0, 1.0) } else { 0.0 };
            [straight(*r), straight(*g), straight(*b), a].map(|v| (v * 255.0 + 0.5) as u8)
        })
        .collect();
    TmpImage { width, height, rgba, layout: [-left, -top, widest, bottom] }
}

/// The glyph a letter is drawn with, by its index.
fn glyph_index(face: &TmpFace, letter: char) -> u32 {
    face.font.glyph_index_of(letter).unwrap_or(0)
}

/// A line broken where its words pass the width: each new line begins at
/// a word, its pens moved back to its start.
fn wrap(line: (Vec<Letter>, f32, f32, f32), width: f32) -> Vec<(Vec<Letter>, f32, f32, f32)> {
    let (letters, line_width, ascender, descender) = line;
    if line_width <= width || letters.is_empty() {
        return vec![(letters, line_width, ascender, descender)];
    }
    let mut out = Vec::new();
    let mut current: Vec<Letter> = Vec::new();
    let mut start = 0.0f32;
    let mut word_start = 0usize;
    let end_of = |letter: &Letter| letter.pen + letter.glyph.map_or(0.0, |g| g.advance * letter.per);
    for letter in letters {
        if letter.letter == ' ' {
            word_start = current.len() + 1;
        }
        let ends = end_of(&letter) - start;
        if ends > width && word_start > 0 && word_start < current.len() + 1 && letter.letter != ' ' {
            // The word begun carried down.
            let carried: Vec<Letter> = current.drain(word_start..).collect();
            let last = current.iter().rev().find(|l| l.letter != ' ').map_or(0.0, |l| end_of(l) - start);
            out.push((std::mem::take(&mut current), last, ascender, descender));
            start = carried.first().map_or(letter.pen, |l| l.pen);
            current = carried;
            word_start = 0;
        }
        current.push(letter);
    }
    let last = current.iter().rev().find(|l| l.letter != ' ').map_or(0.0, |l| end_of(l) - start);
    out.push((current, last, ascender, descender));
    for line in &mut out {
        if let Some(first) = line.0.first().map(|l| l.pen) {
            for letter in &mut line.0 {
                letter.pen -= first;
            }
        }
    }
    out
}

/// One glyph's quad shaded into the image (premultiplied, each laid over
/// what is there as the screen blends: `One, OneMinusSrcAlpha`).
fn shade(p: &Placed, m: &TmpMaterial, pad: f32, origin: [f32; 2], size: [usize; 2], pixels: &mut [[f32; 4]]) {
    let face = p.face;
    let [gw, gh] = [p.glyph.size[0] * p.per, p.glyph.size[1] * p.per];
    let room = pad * p.per;
    let (qx0, qy0, qx1, qy1) = (p.at[0] - room, p.at[1] - room, p.at[0] + gw + room, p.at[1] + gh + room);
    // The shader's figures for this glyph.
    let scale = std::f32::consts::SQRT_2 * p.per * m.gradient_scale * (1.0 + m.sharpness);
    let weight = ((if p.bold { m.weight_bold } else { m.weight_normal }) / 4.0 + m.face_dilate) * m.ratio_a * 0.5;
    let bias = 0.5 - weight + 0.5 / scale;
    let outline = m.outline_width * m.ratio_a * scale;
    let softness = m.outline_softness * m.ratio_a * scale;
    let layer_scale = scale / (1.0 + m.underlay_softness * m.ratio_c * scale);
    let layer_bias = (0.5 - weight) * layer_scale - 0.5 - m.underlay_dilate * m.ratio_c * 0.5 * layer_scale;
    let layer_shift = [-m.underlay_offset[0] * m.ratio_c * m.gradient_scale, -m.underlay_offset[1] * m.ratio_c * m.gradient_scale];
    let premultiply = |c: [f32; 4]| [c[0] * c[3], c[1] * c[3], c[2] * c[3], c[3]];
    let face_color = premultiply([m.face[0] * p.color[0], m.face[1] * p.color[1], m.face[2] * p.color[2], m.face[3]]);
    let outline_color = premultiply(m.outline);
    let underlay = m.underlay.map(premultiply);
    let [rx, ry, _, rh] = p.glyph.rect;
    let (px0, py0) = (((qx0 - origin[0]).floor().max(0.0)) as usize, ((qy0 - origin[1]).floor().max(0.0)) as usize);
    let (px1, py1) = (((qx1 - origin[0]).ceil() as usize).min(size[0]), ((qy1 - origin[1]).ceil() as usize).min(size[1]));
    let saturate = |v: f32| v.clamp(0.0, 1.0);
    for py in py0..py1 {
        for px in px0..px1 {
            let (cx, cy) = (origin[0] + px as f32 + 0.5, origin[1] + py as f32 + 0.5);
            if cx < qx0 || cx > qx1 || cy < qy0 || cy > qy1 {
                continue;
            }
            // Where in the atlas: across from the glyph's left; up from its
            // bottom (the atlas's rows go up), its top at the rectangle's.
            let ax = rx as f32 + (cx - p.at[0]) / p.per;
            let ay = (ry + rh) as f32 - (cy - p.at[1]) / p.per;
            let c = face.sample(ax, ay);
            let sd = (bias - c) * scale;
            let face_alpha = 1.0 - saturate((sd - outline * 0.5 + softness * 0.5) / (1.0 + softness));
            let outline_alpha = saturate(sd + outline * 0.5) * outline.min(1.0).sqrt();
            let mut color = [0.0f32; 4];
            for k in 0..4 {
                color[k] = (face_color[k] + (outline_color[k] - face_color[k]) * outline_alpha) * face_alpha;
            }
            if let Some(under) = underlay {
                let d = face.sample(ax + layer_shift[0], ay - layer_shift[1]) * layer_scale;
                let share = saturate(d - layer_bias) * (1.0 - color[3]);
                for k in 0..4 {
                    color[k] += under[k] * share;
                }
            }
            for value in &mut color {
                *value *= p.color[3];
            }
            let dst = &mut pixels[py * size[0] + px];
            let keep = 1.0 - color[3];
            for k in 0..4 {
                dst[k] = color[k] + dst[k] * keep;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A face of one bar, 'I' (4 by 30 of the atlas's pixels, its left 2
    /// right of the pen, advancing 8), and a space (advancing 6), sampled at
    /// 48, its field made as the game's atlases have it: a half on the
    /// edge, a gradient's scale (11) of pixels to each half.
    fn face() -> TmpFace {
        let bar = TmpGlyph { size: [4.0, 30.0], bearing: [2.0, 30.0], advance: 8.0, rect: [20, 10, 4, 30], scale: 1.0, atlas: 0 };
        let space = TmpGlyph { size: [0.0, 0.0], bearing: [0.0, 0.0], advance: 6.0, rect: [0, 0, 0, 0], scale: 1.0, atlas: 0 };
        let font = TmpFont::for_test(48.0, 36.0, -10.0, &[('I', 1, bar), (' ', 2, space)], 10.0, 8.0);
        let size = 64usize;
        let mut atlas = vec![0u8; size * size];
        for y in 0..size {
            for x in 0..size {
                // The signed distance from the texel's middle to the bar
                // (x 20 to 24, y 10 to 40), inside positive.
                let (cx, cy) = (x as f32 + 0.5, y as f32 + 0.5);
                let outside = ((20.0 - cx).max(cx - 24.0).max(0.0)).hypot((10.0 - cy).max(cy - 40.0).max(0.0));
                let inside = (cx - 20.0).min(24.0 - cx).min(cy - 10.0).min(40.0 - cy);
                let d = if outside > 0.0 { -outside } else { inside };
                atlas[y * size + x] = ((0.5 + d / 22.0).clamp(0.0, 1.0) * 255.0).round() as u8;
            }
        }
        TmpFace { font, atlas, atlas_size: [size, size], kerning: HashMap::new(), units_per_em: 1000.0, bold: None }
    }

    fn plain() -> TmpMaterial {
        TmpMaterial { face: [1.0; 4], outline: [0.0, 0.0, 0.0, 1.0], underlay: None, face_dilate: 0.0, outline_width: 0.0, outline_softness: 0.0, underlay_offset: [0.0; 2], underlay_dilate: 0.0, underlay_softness: 0.0, weight_normal: 0.0, weight_bold: 0.5, ratio_a: 1.0, ratio_c: 1.0, gradient_scale: 11.0, sharpness: 0.0 }
    }

    /// The alpha along the image's row through the bar's middle.
    fn alphas(image: &TmpImage) -> Vec<f32> {
        let row = (image.layout[1] + 20.0) as usize;
        (0..image.width).map(|x| image.rgba[(row * image.width + x) * 4 + 3] as f32 / 255.0).collect()
    }

    /// The bar's left edge half drawn where its field is a half (two of the
    /// atlas's pixels right of the pen), the ramp across it the shader's
    /// (√2 · the gradient's scale over twice it: 0.71 a pixel); the spacing
    /// after each letter the asset's ten hundredths of the text's own size
    /// whatever size its run is; the box aligned by as wide as the last
    /// letter that shows, less its spacing.
    #[test]
    fn a_text_is_laid_out_and_shaded_as_text_mesh_pro_s() {
        let face = face();
        let run = |text: &str, size: f32, bold: bool| TmpRun { text: text.into(), size, color: [1.0; 4], bold };
        let layout = TmpLayout { em: 48.0, spacing: 0.0, kerning: false, wrap: None, align: 1 };
        let image = draw(&face, &plain(), &[run("I I ", 48.0, false)], &layout);
        // 'I' (8 and 4.8), ' ' (6 and 4.8), 'I' (8), its spacing not
        // counted, the space after it not either.
        assert!((image.layout[2] - (8.0 + 4.8 + 6.0 + 4.8 + 8.0)).abs() < 1e-3, "{:?}", image.layout);
        let a = alphas(&image);
        let left = image.layout[0] as usize + 2;
        assert!((a[left] - (0.5 + 0.5 * std::f32::consts::FRAC_1_SQRT_2)).abs() < 0.03, "inside the edge {}", a[left]);
        assert!((a[left - 1] - (0.5 - 0.5 * std::f32::consts::FRAC_1_SQRT_2)).abs() < 0.03, "outside the edge {}", a[left - 1]);
        // At twice the size the bar is twice as wide, the spacing the same.
        let large = draw(&face, &plain(), &[run("II", 96.0, false)], &layout);
        assert!((large.layout[2] - (16.0 + 4.8 + 16.0)).abs() < 1e-3, "{:?}", large.layout);
        // Bold by the shader: wider by its weight (a half over four, over
        // two: 0.0625 of the field, 1.375 pixels each way at this size),
        // and the asset's bold spacing after the letter.
        let bold = draw(&face, &plain(), &[run("II", 48.0, true)], &layout);
        assert!((bold.layout[2] - (8.0 + 4.8 + 3.84 + 8.0 + 3.84)).abs() < 1e-3, "{:?}", bold.layout);
        let covered = |a: &[f32]| a.iter().sum::<f32>();
        let (thin, thick) = (covered(&alphas(&draw(&face, &plain(), &[run("I", 48.0, false)], &layout))), covered(&alphas(&draw(&face, &plain(), &[run("I", 48.0, true)], &layout))));
        assert!((thick - thin - 2.0 * 1.375).abs() < 0.1, "{thin} {thick}");
    }
}
