//! Reads TRP's version from the workspace and makes the exe's icon from the install.
//!
//! The version the app prints first and shows (`--version`, the panel, a glTF it writes) is
//! read from `[workspace.metadata.trp]` in the workspace's `Cargo.toml` and handed to the code
//! as the environment variable `TRP_VERSION` (`env!("TRP_VERSION")`). It is not the packages'
//! own `version`, which stays at 0.0.0: a package's version is part of what cargo builds it
//! under, so raising it with every change (`docs/manual.md`, "Versions") rebuilt all four
//! crates and relinked every test binary; read this way, a raise rebuilds this crate alone.
use std::{env, fs, path::Path};

fn main() {
    let manifest = Path::new(&env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR")).join("../../Cargo.toml");
    println!("cargo:rerun-if-changed={}", manifest.display());
    let text = fs::read_to_string(&manifest).unwrap_or_else(|e| panic!("{}: {e}", manifest.display()));
    let mut in_table = false;
    let mut version = None;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_table = line == "[workspace.metadata.trp]";
        } else if in_table {
            if let Some(value) = line.strip_prefix("version").and_then(|rest| rest.trim_start().strip_prefix('=')) {
                version = Some(value.trim().trim_matches('"').to_string());
                break;
            }
        }
    }
    let version = version.expect("the workspace's Cargo.toml has no `version` under [workspace.metadata.trp]");
    println!("cargo:rustc-env=TRP_VERSION={version}");
    icon();
}

/// The exe's icon: the game's own picture of the hideout's Intelligence Center (the sprite
/// `icon_intelligence_center` of `resources.assets`, 44 by 50),
/// read from the install the app reads (`SPT_DIR` or `local/spt_dir.txt`), never kept in the
/// repo (AGENTS.md, rule 3). It is set square, scaled to the sizes Windows draws, written as
/// `trp.ico` in the build's own folder and compiled with the Windows SDK's resource compiler
/// into `trp` alone, as icon resource 1; the window takes the same one at run time
/// (`src/window_icon.rs`). Without an install the exe keeps the system's icon. The file made
/// is kept between builds, so raising the version does not read the install again.
fn icon() {
    println!("cargo:rerun-if-env-changed=SPT_DIR");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let out = Path::new(&env::var("OUT_DIR").expect("OUT_DIR")).to_path_buf();
    let ico = out.join("trp.ico");
    if !ico.is_file() {
        let made = trp_data::setup::install::Install::locate()
            .and_then(|install| trp_data::interface::screen::sprite_rgba_named(&install, "resources.assets", "icon_intelligence_center"));
        match made {
            Ok((width, height, pixels)) => {
                fs::write(&ico, ico_file(width, height, &pixels)).unwrap_or_else(|e| panic!("{}: {e}", ico.display()));
            }
            Err(e) => {
                println!("cargo:warning=trp.exe is built without its icon: {e:#}");
                return;
            }
        }
    }
    let rc = out.join("trp.rc");
    let named = ico.display().to_string().replace('\\', "\\\\");
    fs::write(&rc, format!("1 ICON \"{named}\"\n")).unwrap_or_else(|e| panic!("{}: {e}", rc.display()));
    embed_resource::compile_for(&rc, ["trp"], embed_resource::NONE)
        .manifest_required()
        .unwrap_or_else(|e| panic!("{}: {e}", rc.display()));
}

/// The sizes Windows draws an exe's icon at, from the title bar's at 100% to Explorer's largest.
const SIZES: [u32; 8] = [16, 20, 24, 32, 40, 48, 64, 256];

/// An icon file (Windows' `.ico`) of the picture at each of `SIZES`, each kept as a PNG inside it.
fn ico_file(width: u32, height: u32, pixels: &[u8]) -> Vec<u8> {
    let images: Vec<Vec<u8>> = SIZES.iter().map(|&side| png_of(side, &square(width, height, pixels, side))).collect();
    let mut file = Vec::new();
    file.extend_from_slice(&[0, 0, 1, 0]);
    file.extend_from_slice(&(SIZES.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * SIZES.len() as u32;
    for (side, image) in SIZES.iter().zip(&images) {
        // 256 is written as 0.
        let side = (*side % 256) as u8;
        file.extend_from_slice(&[side, side, 0, 0]);
        file.extend_from_slice(&1u16.to_le_bytes());
        file.extend_from_slice(&32u16.to_le_bytes());
        file.extend_from_slice(&(image.len() as u32).to_le_bytes());
        file.extend_from_slice(&offset.to_le_bytes());
        offset += image.len() as u32;
    }
    for image in images {
        file.extend_from_slice(&image);
    }
    file
}

fn png_of(side: u32, pixels: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, side, side);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header().and_then(|mut w| w.write_image_data(pixels)).expect("the icon's PNG");
    bytes
}

/// The picture centred in a transparent square of `side`, its longer edge filling it: each pixel
/// of the square is the average of the picture over the area it covers (a box filter, which is
/// sharp in shrinking) where it shrinks, and the picture read between its pixels (bilinear) where
/// it grows; with the colours weighted by their alpha, so that see-through pixels lend no colour.
fn square(width: u32, height: u32, pixels: &[u8], side: u32) -> Vec<u8> {
    let (w, h) = (width as f32, height as f32);
    let scale = side as f32 / w.max(h);
    let (left, top) = ((side as f32 - w * scale) / 2.0, (side as f32 - h * scale) / 2.0);
    // The source's pixel, premultiplied, zero outside it.
    let at = |x: i64, y: i64| -> [f32; 4] {
        if x < 0 || y < 0 || x >= width as i64 || y >= height as i64 {
            return [0.0; 4];
        }
        let p = &pixels[((y as usize) * width as usize + x as usize) * 4..][..4];
        let a = p[3] as f32 / 255.0;
        [p[0] as f32 * a, p[1] as f32 * a, p[2] as f32 * a, a]
    };
    let mut out = vec![0u8; (side * side * 4) as usize];
    for py in 0..side {
        for px in 0..side {
            let mut sum = [0.0f32; 4];
            if scale < 1.0 {
                // The square's pixel covers [x0, x1) by [y0, y1) of the source.
                let (x0, x1) = ((px as f32 - left) / scale, (px as f32 + 1.0 - left) / scale);
                let (y0, y1) = ((py as f32 - top) / scale, (py as f32 + 1.0 - top) / scale);
                let mut area = 0.0;
                for sy in y0.floor() as i64..y1.ceil() as i64 {
                    let cover_y = (y1.min(sy as f32 + 1.0) - y0.max(sy as f32)).max(0.0);
                    for sx in x0.floor() as i64..x1.ceil() as i64 {
                        let cover = cover_y * (x1.min(sx as f32 + 1.0) - x0.max(sx as f32)).max(0.0);
                        let p = at(sx, sy);
                        for c in 0..4 {
                            sum[c] += p[c] * cover;
                        }
                        area += cover;
                    }
                }
                for c in &mut sum {
                    *c /= area.max(f32::EPSILON);
                }
            } else {
                let sx = (px as f32 + 0.5 - left) / scale - 0.5;
                let sy = (py as f32 + 0.5 - top) / scale - 0.5;
                let (fx, fy) = (sx - sx.floor(), sy - sy.floor());
                let (ix, iy) = (sx.floor() as i64, sy.floor() as i64);
                for (dx, dy, weight) in [(0, 0, (1.0 - fx) * (1.0 - fy)), (1, 0, fx * (1.0 - fy)), (0, 1, (1.0 - fx) * fy), (1, 1, fx * fy)] {
                    let p = at(ix + dx, iy + dy);
                    for c in 0..4 {
                        sum[c] += p[c] * weight;
                    }
                }
            }
            let a = sum[3];
            let o = ((py * side + px) * 4) as usize;
            for c in 0..3 {
                out[o + c] = if a > 0.0 { (sum[c] / a).round().clamp(0.0, 255.0) as u8 } else { 0 };
            }
            out[o + 3] = (a * 255.0).round().clamp(0.0, 255.0) as u8;
        }
    }
    out
}

