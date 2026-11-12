//! Images' and texts' looks read from a scene's graphics.

use super::*;

/// The engine's layout of a screen transform in its parent's rectangle.
pub fn place(layout: Layout, outer: Rect) -> Rect {
    let mut rect = Rect { at: [0.0; 2], size: [0.0; 2] };
    for axis in 0..2 {
        let (low, high) = (layout.anchor_min[axis] * outer.size[axis], layout.anchor_max[axis] * outer.size[axis]);
        let own = (high - low + layout.size_delta[axis]) * layout.scale[axis];
        let pivot = low + (high - low) * layout.pivot[axis] + layout.offset[axis];
        rect.size[axis] = own;
        rect.at[axis] = outer.at[axis] + pivot - layout.pivot[axis] * own;
    }
    rect
}

/// How an image of a screen is drawn (`UnityEngine.UI.Image`), read by the
/// engine's own order of its fields: its sprite, its colour, and how the
/// sprite fills its rectangle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImageLook {
    /// The sprite: the file it is in, counted from 1 among the scene's
    /// outside files (0 is the scene's own), and its number there.
    pub sprite: Option<(i32, i64)>,
    pub color: [f32; 4],
    /// Simple, sliced, tiled or filled (`Image.Type`, 0 to 3).
    pub kind: u32,
    pub preserve_aspect: bool,
    pub fill_center: bool,
    /// Horizontal, vertical, radial 90, 180 or 360 (`Image.FillMethod`).
    pub fill_method: u32,
    pub fill_amount: f32,
    pub fill_clockwise: bool,
    pub fill_origin: u32,
    /// The sprite's pixels per unit is multiplied by this.
    pub pixels_per_unit: f32,
}

/// How a text of a screen is drawn (`TMPro.TextMeshProUGUI`), read by the
/// engine's own order of its fields: what it says as saved, its colour,
/// its size and how it lies in its rectangle.
#[derive(Debug, Clone, PartialEq)]
pub struct TextLook {
    pub text: String,
    /// Its font (a `TMP_FontAsset`): the file it is in, counted from 1
    /// among the scene's outside files, and its number there.
    pub font: (i32, i64),
    pub color: [f32; 4],
    pub size: f32,
    /// The least and the most size, where it is sized to fit.
    pub auto_size: Option<[f32; 2]>,
    /// How far, in hundredths, its letters may be narrowed to fit before
    /// it is made smaller (`m_charWidthMaxAdj`).
    pub narrowing: f32,
    /// Bold 1, italic 2, lower case 8, upper case 16 (`FontStyles`).
    pub style: u32,
    /// Left 1, centre 2, right 4 (`HorizontalAlignmentOptions`).
    pub horizontal: u32,
    /// Top 256, middle 512, bottom 1024 (`VerticalAlignmentOptions`).
    pub vertical: u32,
    /// What is added after each letter, in hundredths of the size
    /// (`m_characterSpacing`).
    pub spacing: f32,
    /// Whether the font's kerning is applied between two letters
    /// (`m_enableKerning`), and the margins inside its rectangle (left,
    /// top, right, bottom: `m_margin`).
    pub kerning: bool,
    pub margin: [f32; 4],
    /// The material it is drawn with (`m_sharedMaterial`): its font
    /// asset's own, or a preset of its own (an outline, a shadow), by the
    /// file it is in (counted as `font`) and its number; none where unset.
    pub material: Option<(i32, i64)>,
    /// Whether its words are broken into lines at its width
    /// (`m_enableWordWrapping`), and what becomes of what does not fit
    /// (`m_overflowMode`: overflowing 0, an ellipsis 1, masked 2, cut
    /// off 3).
    pub wraps: bool,
    pub overflow: u32,
}

pub(super) fn color(r: &mut Reader) -> Result<[f32; 4]> {
    Ok([r.f32()?, r.f32()?, r.f32()?, r.f32()?])
}

/// A flag, which the engine keeps as one byte padded to four.
pub(super) fn flag(r: &mut Reader) -> Result<bool> {
    Ok(r.u32()? & 0xff != 0)
}

/// What every graphic of a screen begins with (`Graphic`, then
/// `MaskableGraphic`): its material, its colour, whether it takes the
/// mouse and how far out, whether a mask cuts it, and whom it tells when it
/// is cut. None where that is anyone, whose calls are not read past.
fn graphic(r: &mut Reader) -> Result<Option<[f32; 4]>> {
    r.skip(12)?;
    let color = color(r)?;
    r.skip(4 + 16 + 4)?;
    Ok((r.i32()? == 0).then_some(color))
}

/// An image's look from its script's fields.
pub fn image_look(fields: &[u8]) -> Option<ImageLook> {
    let read = |r: &mut Reader| -> Result<Option<ImageLook>> {
        let Some(color) = graphic(r)? else { return Ok(None) };
        let sprite = pointer(r)?;
        let (kind, preserve_aspect, fill_center, fill_method, fill_amount) = (r.u32()?, flag(r)?, flag(r)?, r.u32()?, r.f32()?);
        let (fill_clockwise, fill_origin, _sprite_mesh, pixels_per_unit) = (flag(r)?, r.u32()?, flag(r)?, r.f32()?);
        let sprite = (sprite.path != 0).then_some((sprite.file, sprite.path));
        Ok(Some(ImageLook { sprite, color, kind, preserve_aspect, fill_center, fill_method, fill_amount, fill_clockwise, fill_origin, pixels_per_unit }))
    };
    read(&mut Reader::new(fields)).ok().flatten()
}

/// A text's look from its script's fields.
pub fn text_look(fields: &[u8]) -> Option<TextLook> {
    let read = |r: &mut Reader| -> Result<Option<TextLook>> {
        if graphic(r)?.is_none() {
            return Ok(None);
        }
        let text = string(r)?;
        r.skip(4)?; // right to left
        let font = pointer(r)?;
        let material = pointer(r)?;
        let materials = r.i32()?.max(0) as usize;
        r.skip(12 * materials + 12)?;
        let materials = r.i32()?.max(0) as usize;
        r.skip(12 * materials + 4)?; // and the colour in bytes
        let color = color(r)?;
        // A gradient and how, the sprites, a style sheet, the face's colour.
        r.skip(4 + 4 + 64 + 12 + 12 + 4 + 12 + 4 + 4 + 4)?;
        let size = r.f32()?;
        r.skip(4 + 4)?; // the size it began with, the weight
        let (auto, least, most) = (flag(r)?, r.f32()?, r.f32()?);
        let (style, mut horizontal, mut vertical) = (r.u32()?, r.u32()?, r.u32()?);
        // The alignment as it was once kept (`m_textAlignment`): where it is
        // not marked converted (0xFFFF) TextMeshPro takes it over the two
        // above as the text loads (`TMP_Text.LoadDefaultSettings`, its
        // published source), its low byte the horizontal and the next the
        // vertical.
        let legacy = r.u32()?;
        if legacy != 0xFFFF && legacy > 0xFF {
            (horizontal, vertical) = (legacy & 0xFF, legacy & 0xFF00);
        }
        let spacing = r.f32()?;
        // The spacing of words, lines and paragraphs, the narrowing, the
        // wrapping and its ratio, the overflow, two linked texts; then
        // the kerning; the padding twice, rich text, control characters,
        // orthographic, culling, two mappings, the line offset, the
        // sorting, the static scale, the buffer, the descender, the
        // page; then the margins.
        r.skip(4 * 4)?;
        let narrowing = r.f32()?;
        let (wraps, _ratio, overflow) = (flag(r)?, r.f32()?, r.u32()?);
        r.skip(12 + 12)?;
        let kerning = flag(r)?;
        r.skip(4 * 14)?;
        let margin = [r.f32()?, r.f32()?, r.f32()?, r.f32()?];
        let material = (material.path != 0).then_some((material.file, material.path));
        Ok(Some(TextLook { text, font: (font.file, font.path), color, size, auto_size: auto.then_some([least, most]), narrowing, style, horizontal, vertical, spacing, kerning, margin, material, wraps, overflow }))
    };
    read(&mut Reader::new(fields)).ok().flatten()
}
