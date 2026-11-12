//! Tests of the models' pictures by their levels of detail and the mip streaming.
use super::*;
use bevy::render::render_resource::{Extent3d, TextureDescriptor, TextureUsages};

/// A picture of the given size, format and levels, its bytes counting up.
fn picture(width: u32, height: u32, format: TextureFormat, levels: u32, bytes: usize) -> Image {
    let mut image = Image::default();
    image.texture_descriptor = TextureDescriptor {
        label: None,
        size: Extent3d { width, height, depth_or_array_layers: 1 },
        mip_level_count: levels,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format,
        usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
        view_formats: &[],
    };
    image.data = Some((0..bytes).map(|at| at as u8).collect());
    image
}

#[test]
fn the_settings_are_the_game_s_figures() {
    let at = |quality, streaming| TextureSettings { quality, streaming };
    assert_eq!([0, 1, 2].map(|q| at(q, false).limit()), [2, 1, 0]);
    assert_eq!([0, 1, 2].map(|q| at(q, true).limit()), [0, 0, 0]);
    assert_eq!([0, 1, 2].map(|q| at(q, true).bias()), [2.0, 1.0, 0.0]);
    assert_eq!([0, 1, 2].map(|q| at(q, true).most_reduction()), [4, 3, 2]);
}

#[test]
fn a_compressed_picture_is_cut_at_whole_blocks() {
    // 256 by 128 in BC1, nine levels: 8 bytes a block of 4 by 4.
    let blocks: Vec<usize> = (0..9).map(|level| ((256u32 >> level).max(1).div_ceil(4) * (128u32 >> level).max(1).div_ceil(4)) as usize * 8).collect();
    let total = blocks.iter().sum();
    let image = picture(256, 128, TextureFormat::Bc1RgbaUnorm, 9, total);
    let (levels, deepest) = levels_of(&image).expect("levels");
    assert_eq!(levels.iter().map(|(range, _)| range.len()).collect::<Vec<_>>(), blocks);
    assert_eq!(levels.last().map(|(range, _)| range.end), Some(total));
    // Level 5 is 8 by 4, whole blocks; level 6 is 4 by 2, not.
    assert_eq!(deepest, 5);
    let data = image.data.clone().expect("data");
    let cut = cut(&image, &levels, &data, 2);
    assert_eq!((cut.texture_descriptor.size.width, cut.texture_descriptor.size.height, cut.texture_descriptor.mip_level_count), (64, 32, 7));
    assert_eq!(cut.data.as_deref(), Some(&data[levels[2].0.start..total]));
}

#[test]
fn a_picture_of_one_level_or_short_data_is_not_streamed() {
    assert!(levels_of(&picture(64, 64, TextureFormat::Rgba8Unorm, 1, 64 * 64 * 4)).is_none());
    assert!(levels_of(&picture(64, 64, TextureFormat::Rgba8Unorm, 7, 64 * 64 * 4)).is_none());
}
