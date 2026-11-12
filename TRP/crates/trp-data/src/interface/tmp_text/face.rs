//! A TextMeshPro face, its line widths and its material.

use super::*;

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

    /// How wide a line of words is as TextMeshPro measures it, unrounded:
    /// as `TmpFont::width`, with the font's kerning between each two
    /// letters where `kerning` (the text's `m_enableKerning`). It is both
    /// a text's preferred width (`CalculatePreferredValues`) and what
    /// `GenerateTextMesh` tests a line's last letter against its room by:
    /// the pen, the kerning of the letters before it in, plus the letter's
    /// own advance (a letter not in the asset left out).
    pub fn line_width(&self, text: &str, size: f32, spacing: f32, bold: bool, kerning: bool) -> f32 {
        let font = &self.font;
        let per = size / font.point_size.max(1.0) * font.scale;
        let added = (font.normal_spacing + spacing + if bold { font.bold_spacing } else { 0.0 }) * size / 100.0;
        let (mut width, mut previous) = (0.0f32, None);
        for letter in text.chars() {
            let Some((glyph, letter_scale)) = font.glyph_of(letter) else { continue };
            let index = glyph_index(self, letter);
            if let (true, Some(before)) = (kerning, previous) {
                if let Some(value) = self.kerning.get(&(before as u16, index as u16)) {
                    width += *value as f32 * size / self.units_per_em;
                }
            }
            width += glyph.advance * per * glyph.scale * letter_scale + added;
            previous = Some(index);
        }
        // The spacing after the last letter is not counted.
        if previous.is_some() {
            width - added
        } else {
            0.0
        }
    }

    /// Where a line's letters lie across from the first one's origin, laid
    /// as `line_width` measures them, for a caret among them. First, the
    /// gaps: each letter's origin (the pen as the letter is laid, its
    /// kerning with the letter before in) and, last, where the last
    /// letter's advance ends, the spacing after it counted
    /// (`TMP_CharacterInfo.origin`, `xAdvance`). Then each letter's box as
    /// it is drawn: from its origin by its glyph's bearing, as wide as the
    /// glyph (the quad's `bottomLeft` and `topRight`, without the padding
    /// its material adds round it). A letter not in the asset, and a
    /// space, have a box of no width at their origin.
    pub fn letter_places(&self, text: &str, size: f32, spacing: f32, bold: bool, kerning: bool) -> (Vec<f32>, Vec<[f32; 2]>) {
        let font = &self.font;
        let per = size / font.point_size.max(1.0) * font.scale;
        let added = (font.normal_spacing + spacing + if bold { font.bold_spacing } else { 0.0 }) * size / 100.0;
        let (mut pen, mut previous) = (0.0f32, None);
        let (mut gaps, mut boxes) = (Vec::new(), Vec::new());
        for letter in text.chars() {
            let Some((glyph, letter_scale)) = font.glyph_of(letter) else {
                gaps.push(pen);
                boxes.push([pen, pen]);
                continue;
            };
            let index = glyph_index(self, letter);
            if let (true, Some(before)) = (kerning, previous) {
                if let Some(value) = self.kerning.get(&(before as u16, index as u16)) {
                    pen += *value as f32 * size / self.units_per_em;
                }
            }
            let per = per * glyph.scale * letter_scale;
            gaps.push(pen);
            boxes.push([pen + glyph.bearing[0] * per, pen + (glyph.bearing[0] + glyph.size[0]) * per]);
            pen += glyph.advance * per + added;
            previous = Some(index);
        }
        gaps.push(pen);
        (gaps, boxes)
    }

    /// The field's value at a place in the atlas (pixels from its lower
    /// left corner), blended between the four nearest texels.
    pub(super) fn sample(&self, x: f32, y: f32) -> f32 {
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
