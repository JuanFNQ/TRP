//! Laying out, wrapping, sizing, drawing and shading a text.

use super::*;

/// Lays out the words: each glyph placed, the widest line's width and the
/// last line's bottom (the text's height), in pixels.
pub(super) fn lay_out<'a>(face: &'a TmpFace, runs: &[TmpRun], layout: &TmpLayout) -> (Vec<Placed<'a>>, f32, f32) {
    // The lines, each its letters at their pens and its width as it is
    // aligned by, ascender, descender. A line is aligned by where its last
    // letter that shows ends (`lineInfo.maxAdvance`: that letter's advance
    // with all that follows it, less the asset's and the text's spacing; a
    // space at its end not counted, a bold letter's spacing counted).
    let mut lines: Vec<(Vec<Letter>, f32, f32, f32)> = Vec::new();
    let mut letters: Vec<Letter> = Vec::new();
    let mut pen = layout.lead;
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
    (placed, widest, bottom)
}

/// The size a text sized to fit is drawn at (`TMP_Text`'s auto-size, as
/// `TextMeshProUGUI.GenerateTextMesh` searches for it): from its own size
/// kept to its least and most, made smaller by half the way down to the
/// least each time the words do not fit, larger by half the way up to the
/// last size that did not each time they do, in twentieths of a point,
/// until the two are within a twentieth or it is at its most. `fits` says
/// whether the words fit at a size. A text that may narrow its letters
/// first (`m_charWidthMaxAdj`) is not searched so.
pub fn auto_size(own: f32, least: f32, most: f32, fits: impl Fn(f32) -> bool) -> f32 {
    let step = |size: f32| (size * 20.0 + 0.5).floor() / 20.0;
    let (mut size, mut low, mut high) = (own.clamp(least, most), least, most);
    for _ in 0..100 {
        if !fits(size) {
            if size <= least {
                break;
            }
            high = size;
            size = step(size - ((size - low) / 2.0).max(0.05)).max(least);
            continue;
        }
        if high - low > 0.051 && size < most {
            low = size;
            size = step(size + ((high - size) / 2.0).max(0.05)).min(most);
            continue;
        }
        break;
    }
    size
}

/// The words' size as laid out, without drawing them: the widest line's
/// width and the text's height (its last line's bottom), in pixels.
pub fn measure(face: &TmpFace, runs: &[TmpRun], layout: &TmpLayout) -> [f32; 2] {
    let (_, widest, bottom) = lay_out(face, runs, layout);
    [widest, bottom]
}

/// Lays out and draws the words.
pub fn draw(face: &TmpFace, material: &TmpMaterial, runs: &[TmpRun], layout: &TmpLayout) -> TmpImage {
    let (placed, widest, bottom) = lay_out(face, runs, layout);
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
pub(super) fn glyph_index(face: &TmpFace, letter: char) -> u32 {
    face.font.glyph_index_of(letter).unwrap_or(0)
}

/// A line broken where its words pass the width: each new line begins at
/// a word, its pens moved back to its start.
pub(super) fn wrap(line: (Vec<Letter>, f32, f32, f32), width: f32) -> Vec<(Vec<Letter>, f32, f32, f32)> {
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
    // Each carried-down piece from its own first letter; the first where
    // the line began (at nothing, or after the first line's lead).
    for line in out.iter_mut().skip(1) {
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
