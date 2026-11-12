//! Tests of the TextMeshPro text: its size to fit, its layout, kerning and shading.

use super::*;

/// TextMeshPro's search for a size: the window that asks how many's
/// field (16.7, sized from 5 to 18) comes to its most where the words
/// fit there, and to the largest twentieth that fits where they do not.
#[test]
fn a_text_sized_to_fit_comes_to_the_game_s_size() {
    assert_eq!(auto_size(16.7, 5.0, 18.0, |_| true), 18.0);
    let fitted = auto_size(16.7, 5.0, 18.0, |size| size <= 16.5);
    assert!(fitted <= 16.5 && fitted > 16.3, "{fitted}");
    assert_eq!(auto_size(16.7, 5.0, 18.0, |_| false), 5.0);
}

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
    let layout = TmpLayout { em: 48.0, spacing: 0.0, kerning: false, wrap: None, align: 1, lead: 0.0 };
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

/// A line's width as the game measures it: the advances and spacings
/// but the last spacing; a pair's kerning only where the text keeps it
/// (-100 of 1000 units to the em between two bars: 4.8 at 48), as the
/// layout draws it.
#[test]
fn a_line_s_width_takes_the_kerning_where_the_text_keeps_it() {
    let mut face = face();
    assert!((face.line_width("II", 48.0, 0.0, false, true) - (8.0 + 4.8 + 8.0)).abs() < 1e-3);
    face.kerning.insert((1, 1), -100);
    assert!((face.line_width("II", 48.0, 0.0, false, false) - (8.0 + 4.8 + 8.0)).abs() < 1e-3);
    let kerned = face.line_width("II", 48.0, 0.0, false, true);
    assert!((kerned - (8.0 + 4.8 + 8.0 - 4.8)).abs() < 1e-3, "{kerned}");
    let layout = TmpLayout { em: 48.0, spacing: 0.0, kerning: true, wrap: None, align: 1, lead: 0.0 };
    let drawn = measure(&face, &[TmpRun { text: "II".into(), size: 48.0, color: [1.0; 4], bold: false }], &layout)[0];
    assert!((kerned - drawn).abs() < 1e-3, "{kerned} {drawn}");
    assert_eq!(face.line_width(" ", 48.0, 0.0, false, true), 6.0);
    assert_eq!(face.line_width("", 48.0, 0.0, false, true), 0.0);
}

/// Where a caret stands among a line's letters: at each letter's origin,
/// the kerning with the letter before in it, and after the last at its
/// advance with the spacing (which a line's width leaves out); a space
/// at the end counted. A bar's box is its glyph's, 2 right of its origin
/// and 4 wide; a space's has no width.
#[test]
fn a_line_s_letters_lie_where_its_width_puts_them() {
    let mut face = face();
    let (gaps, boxes) = face.letter_places("II I ", 48.0, 0.0, false, true);
    let expected = [0.0, 12.8, 25.6, 36.4, 49.2, 60.0];
    assert!(gaps.iter().zip(expected).all(|(gap, wanted)| (gap - wanted).abs() < 1e-3) && gaps.len() == 6, "{gaps:?}");
    assert!((boxes[1][0] - 14.8).abs() < 1e-3 && (boxes[1][1] - 18.8).abs() < 1e-3, "{boxes:?}");
    assert_eq!(boxes[2], [gaps[2], gaps[2]]);
    assert!((gaps[4] - 4.8 - face.line_width("II I", 48.0, 0.0, false, true)).abs() < 1e-3);
    face.kerning.insert((1, 1), -100);
    let (kerned, _) = face.letter_places("II", 48.0, 0.0, false, true);
    assert!((kerned[1] - 8.0).abs() < 1e-3 && (kerned[2] - 4.8 - face.line_width("II", 48.0, 0.0, false, true)).abs() < 1e-3, "{kerned:?}");
    assert_eq!(face.letter_places("", 48.0, 0.0, false, true), (vec![0.0], vec![]));
    // A letter the asset has not takes no room.
    assert_eq!(face.letter_places("x", 48.0, 0.0, false, true).0, [0.0, 0.0]);
}

/// A lead (the room of a tooltip's armour mark) puts the first line's
/// first letter that far in and widens that line by it; a line after
/// it, and a piece of the first carried down by the wrap, start from
/// nothing.
#[test]
fn a_lead_moves_the_first_line_alone() {
    let face = face();
    let run = |text: &str| TmpRun { text: text.into(), size: 48.0, color: [1.0; 4], bold: false };
    let layout = |lead: f32, wrap: Option<f32>| TmpLayout { em: 48.0, spacing: 0.0, kerning: false, wrap, align: 1, lead };
    let xs = |lead: f32, wrap: Option<f32>, text: &str| lay_out(&face, &[run(text)], &layout(lead, wrap)).0.iter().map(|p| p.at[0]).collect::<Vec<_>>();
    // 'I' is 8 wide: 28 with a lead of 20; "II" under it 20.8.
    assert!((measure(&face, &[run("I")], &layout(20.0, None))[0] - 28.0).abs() < 1e-3);
    assert!((measure(&face, &[run("I\nII")], &layout(20.0, None))[0] - 28.0).abs() < 1e-3);
    let (bare, led) = (xs(0.0, None, "I\nI"), xs(20.0, None, "I\nI"));
    assert!((led[0] - bare[0] - 20.0).abs() < 1e-3 && (led[1] - bare[1]).abs() < 1e-3, "{bare:?} {led:?}");
    // "I I" in 30: the second 'I' (to 51.6) carried down, from nothing;
    // the first kept after the lead.
    let wrapped = xs(20.0, Some(30.0), "I I");
    assert!((wrapped[0] - bare[0] - 20.0).abs() < 1e-3, "{wrapped:?}");
    assert!((wrapped.last().copied().unwrap_or_default() - bare[0]).abs() < 1e-3, "{wrapped:?}");
}
