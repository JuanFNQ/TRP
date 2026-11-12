//! Tests of the game's screens as read and drawn.

use super::*;

/// A fill's width is left unwritten only where every place within a
/// pixel that draws its old width as laid out draws the new one the
/// same (`set_fill_width`): checked against the layout's rounding at a
/// thousand places a pixel, over widths from nought to a few hundred
/// pixels and steps from a hair to a few pixels.
#[test]
fn a_fill_is_written_whenever_its_drawn_width_would_change() {
    let width = |f: f32, w: f32| (f + w).round() - f.round();
    let mut seed = 0x2545_f491_u32;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed as f32 / u32::MAX as f32
    };
    let (mut skipped, mut written) = (0, 0);
    for _ in 0..4000 {
        let before = next() * 300.0;
        let after = (before + (next() - 0.5) * if next() < 0.5 { 0.2 } else { 6.0 }).max(0.0);
        let f = next();
        let drawn = width(f, before);
        if super::panel_pieces::same_drawn(before, after, drawn) {
            skipped += 1;
            for k in 0..1000 {
                let g = k as f32 / 1000.0;
                if width(g, before) == drawn {
                    assert_eq!(width(g, after), drawn, "left unwritten from {before} to {after}, drawn {drawn}, yet at {g} it draws otherwise");
                }
            }
        } else {
            written += 1;
        }
    }
    // Most small steps keep the edge where it was; most large ones do not.
    assert!(skipped > 1000 && written > 1000, "{skipped} left, {written} written");
}

/// The loot panel's search button's ground (`courses10`'s
/// `dead-scav-search-a`, its alpha over the screen's): off from the
/// panel's first frame, 0.892 after a 29 ms first step, 0.830 after 17
/// ms more; at the open (frame 559, 8.4 ms) 0.6564, then 0.6527 (11.9
/// ms), 0.628 again 0.1 s on.
#[test]
fn the_search_button_dims_as_the_game_s_does() {
    let idle = [1.0; 4];
    let off = [0.18382353, 0.18382353, 0.18382353, 0.628];
    let mut fade = crate::screens::split_view::Fading { from: idle, to: idle, since: 0.1 };
    assert!((step_tint(&mut fade, idle, off, false, 0.0292, 0.1)[3] - 0.892).abs() < 2e-3);
    assert!((step_tint(&mut fade, idle, off, false, 0.0166, 0.1)[3] - 0.830).abs() < 2e-3);
    for _ in 0..10 {
        step_tint(&mut fade, idle, off, false, 0.0085, 0.1);
    }
    assert_eq!(step_tint(&mut fade, idle, off, false, 0.0085, 0.1)[3], 0.628);
    assert!((step_tint(&mut fade, idle, off, true, 0.0084, 0.1)[3] - 0.6564).abs() < 5e-4);
    assert!((step_tint(&mut fade, idle, off, false, 0.0119, 0.1)[3] - 0.6527).abs() < 5e-4);
    for _ in 0..11 {
        step_tint(&mut fade, idle, off, false, 0.0084, 0.1);
    }
    assert_eq!(step_tint(&mut fade, idle, off, false, 0.0084, 0.1)[3], 0.628);
}

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
