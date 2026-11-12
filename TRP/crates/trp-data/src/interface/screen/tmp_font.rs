//! TextMeshPro's fonts and glyphs, and a text's width.

use super::*;

/// A TextMeshPro font asset (`TMP_FontAsset`, a script's object of the
/// build's files) as far as a line's width goes: the size its glyphs were
/// sampled at and its scale (`m_FaceInfo`), each glyph's advance at that
/// size and its own scale (`m_GlyphTable`), each letter's glyph and scale
/// (`m_CharacterTable`), what every letter adds and what a letter drawn
/// bold adds besides, in hundredths of the size (`normalSpacingOffset`,
/// `boldSpacing`), and the asset its bold is taken from (`m_FontWeightTable`'s
/// weight 700, its regular typeface), where it has one.
#[derive(Debug, Clone, Default)]
pub struct TmpFont {
    pub point_size: f32,
    pub scale: f32,
    /// The face's lines above and below the baseline (`m_AscentLine`,
    /// `m_DescentLine`), at the sampling size.
    pub ascent: f32,
    pub descent: f32,
    /// The distance between two lines' baselines (`m_LineHeight`), at the
    /// sampling size.
    pub line_height: f32,
    glyphs: HashMap<u32, (f32, f32)>,
    letters: HashMap<u32, (u32, f32)>,
    /// Each glyph whole, by its index (`m_GlyphTable`).
    pub glyph_table: HashMap<u32, TmpGlyph>,
    /// The textures its glyphs' distance fields are kept in
    /// (`m_AtlasTextures`), each by its file and number.
    pub atlases: Vec<(String, i64)>,
    pub normal_spacing: f32,
    pub bold_spacing: f32,
    pub bold: Option<(String, i64)>,
    /// Its face's family and style (`m_FaceInfo`: "Bender", "Bold").
    pub family: String,
    pub style: String,
}

impl TmpFont {
    /// Reads it from its file and number. Its fields by the engine's order:
    /// the object's head and name, the asset's hash, material and its hash,
    /// its version, the source font's id and the font, how its atlas is
    /// filled, its face (index, family, style, sampling size, scale, units
    /// to the em and fifteen lines' places), its glyphs (52 bytes each:
    /// index, five metrics, the atlas rectangle, scale, atlas, class) and
    /// its letters (16 each: kind, letter, glyph, scale). The spacings are
    /// the last fields but two, after the weight table of ten pairs of
    /// typefaces and the old weights' empty list.
    pub fn read(install: &Install, file: &str, id: i64) -> Result<Self> {
        let (_, bytes) = object_bytes(install, file, id)?;
        let mut r = Reader::new(&bytes);
        r.skip(12 + 4 + 12)?;
        string(&mut r)?;
        r.skip(4 + 12 + 4)?;
        string(&mut r)?;
        string(&mut r)?;
        r.skip(12 + 4 + 4)?;
        let family = string(&mut r)?;
        let style = string(&mut r)?;
        let point_size = r.i32()? as f32;
        let scale = r.f32()?;
        // Units to the em, the line's height, then the ascent, the cap, the
        // mean line, the baseline and the descent, and nine more.
        r.skip(4)?;
        let line_height = r.f32()?;
        let ascent = r.f32()?;
        r.skip(3 * 4)?;
        let descent = r.f32()?;
        r.skip(9 * 4)?;
        let mut glyphs = HashMap::new();
        let mut glyph_table = HashMap::new();
        for _ in 0..r.u32()? {
            let index = r.u32()?;
            let (width, height, bearing_x, bearing_y) = (r.f32()?, r.f32()?, r.f32()?, r.f32()?);
            let advance = r.f32()?;
            let rect = [r.i32()?, r.i32()?, r.i32()?, r.i32()?];
            let scale = r.f32()?;
            let atlas = r.i32()?;
            r.skip(4)?;
            glyphs.insert(index, (advance, scale));
            glyph_table.insert(index, TmpGlyph { size: [width, height], bearing: [bearing_x, bearing_y], advance, rect, scale, atlas: atlas.max(0) as usize });
        }
        let mut letters = HashMap::new();
        for _ in 0..r.u32()? {
            r.skip(4)?;
            let (letter, glyph, scale) = (r.u32()?, r.u32()?, r.f32()?);
            letters.insert(letter, (glyph, scale));
        }
        // The atlases, each a pointer into this file or one of its own.
        let externals = BuildFile::open(install, file).map(|build| build.meta.externals.clone()).unwrap_or_default();
        let mut atlases = Vec::new();
        for _ in 0..r.u32()? {
            let to = pointer(&mut r)?;
            let name = match to.file {
                0 => Some(file.to_owned()),
                outside => externals.get(outside as usize - 1).map(|name| name.rsplit('/').next().unwrap_or(name).to_owned()),
            };
            if let Some(name) = name {
                atlases.push((name, to.path));
            }
        }
        let tail = |from_end: usize| -> Result<&[u8]> { bytes.get(bytes.len() - from_end..bytes.len() - from_end + 4).context("the font asset ends early") };
        let float = |from_end: usize| -> Result<f32> { Ok(f32::from_le_bytes(tail(from_end)?.try_into()?)) };
        let (normal_spacing, bold_spacing) = (float(20)?, float(12)?);
        // The weight table: ten pairs (regular, italic), then the old
        // weights' list, empty, before the spacings.
        let table = bytes.len() - 24 - 4 - 240;
        let bold = (u32::from_le_bytes(tail(24 + 4)?.try_into()?) == 0 && u32::from_le_bytes(bytes.get(table - 4..table).context("the weight table")?.try_into()?) == 10)
            .then(|| {
                let at = table + 24 * 7;
                let to = pointer(&mut Reader::new(&bytes[at..at + 12])).ok()?;
                (to.path != 0).then_some((to.file, to.path))
            })
            .flatten();
        let bold = match bold {
            Some((0, path)) => Some((file.to_owned(), path)),
            Some((outside, path)) => {
                let build = BuildFile::open(install, file)?;
                build.meta.externals.get(outside as usize - 1).map(|name| (name.rsplit('/').next().unwrap_or(name).to_owned(), path))
            }
            None => None,
        };
        Ok(Self { point_size, scale, ascent, descent, line_height, glyphs, letters, glyph_table, atlases, normal_spacing, bold_spacing, bold, family, style })
    }

    /// The index of the glyph a letter is drawn with.
    pub fn glyph_index_of(&self, letter: char) -> Option<u32> {
        self.letters.get(&(letter as u32)).map(|(glyph, _)| *glyph)
    }

    /// The glyph a letter is drawn with, and the letter's own scale.
    pub fn glyph_of(&self, letter: char) -> Option<(&TmpGlyph, f32)> {
        let (glyph, scale) = self.letters.get(&(letter as u32))?;
        Some((self.glyph_table.get(glyph)?, *scale))
    }

    /// How high a line of words is in it at a size, as TextMeshPro works
    /// out a text's preferred height: from the face's ascent to its
    /// descent at the size over the sampling size, rounded up to a
    /// hundredth.
    pub fn height(&self, size: f32) -> f32 {
        let height = (self.ascent - self.descent) * size / self.point_size.max(1.0) * self.scale;
        ((height * 100.0) as i64 + 1) as f32 / 100.0
    }

    /// How wide a line of words is in it at a size, as TextMeshPro works out
    /// a text's preferred width (`CalculatePreferredValues`): each letter's
    /// glyph's advance at the size over the sampling size, with the face's,
    /// the glyph's and the letter's scales, and what every letter but the
    /// last adds (`normalSpacingOffset`, the text's own `spacing`, and a
    /// bold letter's `boldSpacing` where `bold` and the asset has no bold
    /// of its own), in hundredths of the size; rounded up to a hundredth. A letter
    /// not in the asset is left out.
    pub fn width(&self, text: &str, size: f32, spacing: f32, bold: bool) -> f32 {
        let per = size / self.point_size.max(1.0) * self.scale;
        let added = (self.normal_spacing + spacing + if bold { self.bold_spacing } else { 0.0 }) * size / 100.0;
        let width: f32 = text
            .chars()
            .filter_map(|letter| self.letters.get(&(letter as u32)))
            .filter_map(|(glyph, letter_scale)| self.glyphs.get(glyph).map(|(advance, glyph_scale)| advance * per * glyph_scale * letter_scale + added))
            .sum();
        // The spacing after the last letter is not counted.
        let width = if text.chars().any(|letter| self.letters.contains_key(&(letter as u32))) { width - added } else { 0.0 };
        ((width * 100.0) as i64 + 1) as f32 / 100.0
    }
}

/// A glyph of a TextMeshPro font asset (`Glyph`): its size, where it
/// stands from the pen (its bearings: right, and up from the baseline) and
/// how far it moves the pen, in the atlas's pixels at the sampling size;
/// its rectangle in its atlas (left, bottom, width, height, in pixels from
/// the atlas's lower left corner); its scale; which atlas.
#[derive(Debug, Clone, Copy)]
pub struct TmpGlyph {
    pub size: [f32; 2],
    pub bearing: [f32; 2],
    pub advance: f32,
    pub rect: [i32; 4],
    pub scale: f32,
    pub atlas: usize,
}

/// How wide a line of words is in a font of the build's (its file's
/// bytes) at a size, in the screen's units, as TextMeshPro lays it out:
/// each letter's advance, and where the text keeps kerning
/// (`m_enableKerning`) the font's own between two (its `kern` table's
/// pairs), at the size over the font's units to the em. Where a letter is
/// not in the font, its replacement's (`.notdef`).
pub fn text_width(font: &[u8], text: &str, size: f32, kerning: bool) -> Option<f32> {
    let face = ttf_parser::Face::parse(font, 0).ok()?;
    let glyphs: Vec<ttf_parser::GlyphId> = text.chars().map(|letter| face.glyph_index(letter).unwrap_or(ttf_parser::GlyphId(0))).collect();
    let mut width: f32 = glyphs.iter().map(|&glyph| face.glyph_hor_advance(glyph).unwrap_or(0) as f32).sum();
    if let Some(kern) = face.tables().kern.filter(|_| kerning) {
        for pair in glyphs.windows(2) {
            width += kern.subtables.into_iter().filter(|table| table.horizontal && !table.variable).filter_map(|table| table.glyphs_kerning(pair[0], pair[1])).map(f32::from).sum::<f32>();
        }
    }
    Some(width * size / face.units_per_em() as f32)
}

#[cfg(test)]
impl TmpFont {
    /// A font asset of the glyphs given (each letter with its glyph's
    /// index), for a test of what draws from one.
    pub(crate) fn for_test(point_size: f32, ascent: f32, descent: f32, glyphs: &[(char, u32, TmpGlyph)], normal_spacing: f32, bold_spacing: f32) -> Self {
        Self {
            point_size,
            scale: 1.0,
            ascent,
            descent,
            line_height: ascent - descent,
            glyphs: glyphs.iter().map(|(_, index, glyph)| (*index, (glyph.advance, glyph.scale))).collect(),
            letters: glyphs.iter().map(|(letter, index, _)| (*letter as u32, (*index, 1.0))).collect(),
            glyph_table: glyphs.iter().map(|(_, index, glyph)| (*index, *glyph)).collect(),
            atlases: Vec::new(),
            normal_spacing,
            bold_spacing,
            bold: None,
            family: String::new(),
            style: String::new(),
        }
    }
}
