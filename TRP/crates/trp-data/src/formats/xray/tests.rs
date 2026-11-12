//! Tests of the X-Ray model's reading: its turns and a picture of one block.

use super::*;

#[test]
fn a_turn_of_nothing_leaves_the_axes() {
    let axes = axes_of([0.0; 3]);
    assert_eq!(axes, [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]);
}

/// A quarter turn about y alone, as `setXYZi` makes it: x goes to
/// minus z, and z to x.
#[test]
fn a_turn_about_one_axis() {
    let axes = axes_of([0.0, std::f32::consts::FRAC_PI_2, 0.0]);
    let near = |a: [f32; 3], b: [f32; 3]| a.iter().zip(&b).all(|(a, b)| (a - b).abs() < 1e-6);
    assert!(near(axes[0], [0.0, 0.0, -1.0]), "{axes:?}");
    assert!(near(axes[1], [0.0, 1.0, 0.0]) && near(axes[2], [1.0, 0.0, 0.0]), "{axes:?}");
}

/// A picture of one block: four letters, a header, eight bytes.
#[test]
fn a_picture_of_one_block() {
    let mut file = vec![0u8; 128 + 8];
    file[..4].copy_from_slice(b"DDS ");
    file[12..16].copy_from_slice(&4u32.to_le_bytes());
    file[16..20].copy_from_slice(&4u32.to_le_bytes());
    file[28..32].copy_from_slice(&3u32.to_le_bytes());
    file[84..88].copy_from_slice(b"DXT1");
    let picture = read_picture(&file).unwrap();
    // Three levels are listed and one is there.
    assert_eq!((picture.width, picture.height, picture.levels, picture.packing, picture.data.len()), (4, 4, 1, Packing::Bc1, 8));
}
