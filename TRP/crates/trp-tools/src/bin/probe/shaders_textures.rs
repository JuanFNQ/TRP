//! Shaders, compute shaders, BC7 textures and their statistics.

use anyhow::Result;
use trp_data::unity::{class, Assets};

// Each shader of a bundle by its name (`shaders`, the game's own
// shaders bundle, unless another is given with `--bundle <key>`
// first); with a part of a name and a folder, those shaders' DirectX
// 11 programs written there as they are kept (`<name>_<n>.dxbc`,
// each cut from the decompressed blob at its own `DXBC` header),
// for the SDK's `fxc /dumpbin` to read.
pub fn shaders(assets: &mut Assets, args: &[String], wanted: Option<&str>) -> Result<()> {
    let arg = |i: usize| args.get(i).map(String::as_str);
    use trp_data::unity::Value;
    // `--uses <name>`: the shaders whose programs name that
    // property (their parsed form's name table).
    if wanted == Some("--uses") {
        let property = arg(2).ok_or_else(|| anyhow::anyhow!("a property's name needed"))?;
        for file in assets.load("shaders")? {
            for obj in assets.objects_of(file, 48).collect::<Vec<_>>() {
                let shader = assets.read(obj)?;
                let name = shader.get("m_ParsedForm").and_then(|form| form.get("m_Name")).and_then(Value::as_str).unwrap_or("").to_owned();
                if format!("{shader:?}").contains(&format!("{property:?}")) {
                    println!("{} {name}", obj.path_id);
                }
            }
        }
        return Ok(());
    }
    // `--stencil`: each shader's passes that write the stencil
    // (their state's reference, by figure or by the property that
    // sets it, its write mask and front comparison and pass
    // operation).
    if wanted == Some("--stencil") {
        for file in assets.load("shaders")? {
            for obj in assets.objects_of(file, 48).collect::<Vec<_>>() {
                let shader = assets.read(obj)?;
                let Some(form) = shader.get("m_ParsedForm") else { continue };
                let name = form.get("m_Name").and_then(Value::as_str).unwrap_or("").to_owned();
                for sub in form.get("m_SubShaders").and_then(Value::as_array).unwrap_or(&[]) {
                    for (p, pass) in sub.get("m_Passes").and_then(Value::as_array).unwrap_or(&[]).iter().enumerate() {
                        let Some(state) = pass.get("m_State") else { continue };
                        let figure = |v: Option<&Value>| -> (f32, String) {
                            let Some(v) = v else { return (0.0, String::new()) };
                            (v.get("val").and_then(Value::as_f32).unwrap_or(0.0), v.get("name").and_then(Value::as_str).unwrap_or("").to_owned())
                        };
                        let (reference, by) = figure(state.get("stencilRef"));
                        let (write, _) = figure(state.get("stencilWriteMask"));
                        let (comp, _) = figure(state.get("stencilOpFront").and_then(|o| o.get("comp")));
                        let (op, op_by) = figure(state.get("stencilOpFront").and_then(|o| o.get("pass")));
                        if reference != 0.0 || (!by.is_empty() && by != "<noninit>") || op != 0.0 {
                            let lightmode = format!("{:?}", pass.get("m_Tags"));
                            let mode = lightmode.split("LIGHTMODE").nth(1).and_then(|s| s.split('"').nth(4)).unwrap_or("");
                            println!("{} {name} pass {p} {mode}: ref {reference} ({by}) write mask {write} comp {comp} pass op {op} ({op_by})", obj.path_id);
                        }
                    }
                }
            }
        }
        return Ok(());
    }
    // A shader's DirectX 11 programs written out by their blob
    // entries, and the blob itself.
    let write_programs = |shader: &Value, name: &str, folder: &str| -> Result<()> {
        let platforms = shader.get("platforms").and_then(Value::as_uints).unwrap_or(&[]);
        let Some(at) = platforms.iter().position(|&p| p == 4) else {
            println!("  no DirectX 11 programs ({platforms:?})");
            return Ok(());
        };
        // Every segment of the platform's blob unpacked: the first
        // holds the table (and in some shaders the programs too), a
        // later one the programs whose entries name it (`Hidden/HBAO`).
        let all = |name: &str| shader.get(name).and_then(Value::as_array).and_then(|all| all.get(at)).and_then(Value::as_uints).map(|v| v.iter().map(|&n| n as usize).collect::<Vec<_>>()).unwrap_or_default();
        let (offsets, packed, sizes) = (all("offsets"), all("compressedLengths"), all("decompressedLengths"));
        let blob = shader.get("compressedBlob").and_then(Value::as_bytes).unwrap_or(&[]);
        let segments = offsets.iter().zip(&packed).zip(&sizes).map(|((&offset, &packed), &size)| lz4_flex::block::decompress(&blob[offset..offset + packed], size)).collect::<Result<Vec<_>, _>>()?;
        let plain = segments.first().cloned().unwrap_or_default();
        std::fs::create_dir_all(folder)?;
        std::fs::write(std::path::Path::new(folder).join(format!("{}.blob", name.replace(['/', ' '], "_"))), &plain)?;
        // The blob's table: how many entries, then each one's
        // place, length and segment; a pass's programs name
        // their entry (`m_BlobIndex`). Each program is written
        // by its entry, cut at its own `DXBC` header.
        let word = |at: usize| plain.get(at..at + 4).map(|b| u32::from_le_bytes(b.try_into().expect("four")) as usize);
        let entries = word(0).unwrap_or(0);
        let mut n = 0;
        for entry in 0..entries {
            let (Some(start), Some(length), Some(segment)) = (word(4 + 12 * entry), word(8 + 12 * entry), word(12 + 12 * entry)) else { break };
            let Some(bytes) = segments.get(segment).and_then(|s| s.get(start..start + length)) else { continue };
            // The whole entry: a program's, or the parameters a program's variant
            // names (`m_ParameterBlobIndices`): `local/blob_params_s92.pl <file>`.
            std::fs::write(std::path::Path::new(folder).join(format!("{}_b{entry}.entry", name.replace(['/', ' '], "_"))), bytes)?;
            let Some(at) = bytes.windows(4).position(|w| w == b"DXBC") else { continue };
            let Some(size) = bytes.get(at + 24..at + 28).map(|b| u32::from_le_bytes(b.try_into().expect("four")) as usize) else { continue };
            let Some(program) = bytes.get(at..at + size) else { continue };
            std::fs::write(std::path::Path::new(folder).join(format!("{}_b{entry}.dxbc", name.replace(['/', ' '], "_"))), program)?;
            n += 1;
        }
        println!("  {n} programs written to {folder}, by their blob entries (of {entries})");
        Ok(())
    };
    // `--build <file> <number> [folder] [depth]`: a shader of one of
    // the build's own files (`globalgamemanagers.assets`), read by
    // the shader's tree borrowed from the shaders bundle; its parsed
    // form printed to the depth asked.
    if wanted == Some("--build") {
        let file = arg(2).ok_or_else(|| anyhow::anyhow!("a file of the build needed"))?;
        let id: i64 = arg(3).ok_or_else(|| anyhow::anyhow!("the shader's number needed"))?.parse()?;
        let tree = trp_data::interface::screen::tree_of(&assets.install, "shaders", 48)?;
        let mut pictures = trp_data::interface::screen::ScenePictures::new(&assets.install)?;
        let shader = pictures.read_by(file, id, 48, &tree)?;
        let name = shader.get("m_ParsedForm").and_then(|form| form.get("m_Name")).and_then(Value::as_str).unwrap_or("").to_owned();
        println!("{id} {name}");
        let depth = arg(5).map_or(Ok(0), str::parse)?;
        if depth > 0 {
            println!("{}", shader.get("m_ParsedForm").map(|form| form.pretty(depth, 40)).unwrap_or_default());
        }
        if let Some(folder) = arg(4) {
            write_programs(&shader, &name, folder)?;
        }
        return Ok(());
    }
    let (key, wanted, folder) = match wanted {
        Some("--bundle") => (arg(2).unwrap_or("shaders").to_owned(), arg(3), arg(4)),
        _ => ("shaders".to_owned(), wanted, arg(2)),
    };
    for file in assets.load(&key)? {
        for obj in assets.objects_of(file, 48).collect::<Vec<_>>() {
            let shader = assets.read(obj)?;
            let name = shader.get("m_ParsedForm").and_then(|form| form.get("m_Name")).and_then(trp_data::unity::Value::as_str).unwrap_or("").to_owned();
            if wanted.is_some_and(|wanted| !name.to_lowercase().contains(&wanted.to_lowercase())) {
                continue;
            }
            println!("{} {name}", obj.path_id);
            // With `TRP_PROBE_FIGURES` set, its properties' names (what
            // a material of it keeps).
            if std::env::var("TRP_PROBE_FIGURES").is_ok() {
                let props = shader.at("m_ParsedForm.m_PropInfo.m_Props").and_then(trp_data::unity::Value::as_array).map(|props| props.iter().filter_map(|p| p.get("m_Name")?.as_str().map(str::to_owned)).collect::<Vec<_>>()).unwrap_or_default();
                println!("  properties: {}", props.join(" "));
            }
            if let Some(folder) = folder {
                write_programs(&shader, &name, folder)?;
            }
        }
    }
    Ok(())
}

// A compute shader of one of the build's own files
// (`compute resources.assets 2569 [folder] [depth]`), read by the
// tree of the shaders bundle's: its form printed to the depth asked,
// and each variant's program for DirectX 11 written to the folder
// as `<name>_<kernel>_<n>.dxbc`, by the name of the kernel it is.
pub fn compute(assets: &mut Assets, args: &[String], file: &str) -> Result<()> {
    let arg = |i: usize| args.get(i).map(String::as_str);
    use trp_data::unity::Value;
    let id: i64 = arg(2).ok_or_else(|| anyhow::anyhow!("the compute shader's number needed"))?.parse()?;
    let tree = trp_data::interface::screen::tree_of(&assets.install, "shaders", 72)?;
    let mut pictures = trp_data::interface::screen::ScenePictures::new(&assets.install)?;
    let compute = pictures.read_by(file, id, 72, &tree)?;
    let name = compute.get("m_Name").and_then(Value::as_str).unwrap_or("").to_owned();
    println!("{id} {name}");
    let depth = arg(4).map_or(Ok(4), str::parse)?;
    println!("{}", compute.pretty(depth, 40));
    if let Some(folder) = arg(3).filter(|folder| *folder != "-") {
        std::fs::create_dir_all(folder)?;
        let variants: &[Value] = compute.get("variants").and_then(Value::as_array).unwrap_or(&[]);
        for variant in variants {
            for kernel in variant.get("kernels").and_then(Value::as_array).unwrap_or(&[]) {
                let kernel_name = kernel.get("name").and_then(Value::as_str).unwrap_or("kernel").to_owned();
                for (n, unique) in kernel.get("uniqueVariants").and_then(Value::as_array).unwrap_or(&[]).iter().enumerate() {
                    let Some(code) = unique.get("code").and_then(Value::as_bytes) else { continue };
                    // The program from its own `DXBC` header, as long as it says.
                    let Some(at) = code.windows(4).position(|w| w == b"DXBC") else { continue };
                    let size = code.get(at + 24..at + 28).map_or(code.len() - at, |b| u32::from_le_bytes(b.try_into().expect("four")) as usize);
                    let program = &code[at..(at + size).min(code.len())];
                    let path = std::path::Path::new(folder).join(format!("{name}_{kernel_name}_{n}.dxbc"));
                    std::fs::write(&path, program)?;
                    println!("  written {} ({} bytes)", path.display(), program.len());
                }
            }
        }
    }
    Ok(())
}

// `bc7bench <bundle> [threads] [scratch folder]`: every BC7 picture
// in a bundle made BC5 of two channels (normals' alpha and green) as
// the app did first (`texture2ddecoder`, then `compress::bc5`) and by
// `unity::bc7::to_bc5` on one thread and on several; every block of
// it decoded by both and compared; the result written to the scratch
// folder and read back, as a cache on the disk would; with the time
// each takes. Reads the install only.
pub fn bc7bench(assets: &mut Assets, args: &[String], key: &str) -> Result<()> {
    let arg = |i: usize| args.get(i).map(String::as_str);
    use std::time::Instant;
    use trp_data::unity::{bc7, extract::read_texture, texture};
    let threads: usize = arg(2).and_then(|t| t.parse().ok()).unwrap_or(8);
    let scratch = std::path::PathBuf::from(arg(3).unwrap_or("local/bc7bench_scratch"));
    std::fs::create_dir_all(&scratch)?;
    let ms = |at: Instant| at.elapsed().as_secs_f64() * 1000.0;
    let (mut sums, mut count, mut mismatched, mut bytes) = ([0.0f64; 6], 0, 0usize, 0usize);
    println!("{:<52} {:>9} {:>7} {:>7} {:>7} {:>7} {:>7} {:>7}", "picture", "size", "read", "before", "own 1", "own N", "write", "read c");
    for file in assets.load(key)? {
        for obj in assets.objects_of(file, class::TEXTURE_2D).collect::<Vec<_>>() {
            let at = Instant::now();
            let tex = read_texture(&assets, obj)?;
            let read = ms(at);
            if tex.format != texture::format::BC7 || tex.width % 4 != 0 || tex.height % 4 != 0 {
                continue;
            }
            let levels = texture::mips(&tex)?;
            let end = levels.last().map_or(0, |l| l.offset + l.size);
            let pick = |p: [u8; 4]| [p[3], p[1]];
            let at = Instant::now();
            let mut before = Vec::new();
            for level in &levels {
                let rg: Vec<u8> = texture::decode_rgba(&tex, *level)?.chunks_exact(4).flat_map(|p| pick([p[0], p[1], p[2], p[3]])).collect();
                before.extend(trp_data::formats::compress::bc5(level.width, level.height, &rg));
            }
            let before_ms = ms(at);
            let at = Instant::now();
            let one = bc7::to_bc5(&tex.data[..end], pick, 1);
            let one_ms = ms(at);
            let at = Instant::now();
            let many = bc7::to_bc5(&tex.data[..end], pick, threads);
            let many_ms = ms(at);
            anyhow::ensure!(one == many, "{}: one thread and {threads} differ", tex.name);
            // Decoding alone, by each decoder, on one thread.
            let at = Instant::now();
            let mut sink = 0u32;
            for level in &levels {
                sink = sink.wrapping_add(texture::decode_rgba(&tex, *level)?[0] as u32);
            }
            let theirs_ms = ms(at);
            let at = Instant::now();
            for block in tex.data[..end].chunks_exact(16) {
                sink = sink.wrapping_add(bc7::decode_block(block.try_into()?)[5][3] as u32);
            }
            let ours_ms = ms(at);
            println!("    decoding alone: texture2ddecoder {theirs_ms:.1} ms, ours {ours_ms:.1} ms ({sink})");
            // What BC5 keeps of the two channels: each value against
            // the BC7's own, in 255ths.
            let (mut off_sum, mut off_max, mut over2, mut values) = (0u64, 0u8, 0usize, 0usize);
            for (block, made) in tex.data[..end].chunks_exact(16).zip(many.chunks_exact(16)) {
                let two = bc7::decode_block(block.try_into()?).map(pick);
                for half in 0..2 {
                    let read = texture::decode_alpha_block(&made[half * 8..half * 8 + 8]);
                    for (got, pixel) in read.iter().zip(two) {
                        let off = got.abs_diff(pixel[half]);
                        off_sum += off as u64;
                        off_max = off_max.max(off);
                        over2 += (off > 2) as usize;
                        values += 1;
                    }
                }
            }
            println!("    BC5 against the BC7: off by {:.2} in 255 on average, {off_max} at most, more than 2 for {:.2}% of values", off_sum as f64 / values as f64, 100.0 * over2 as f64 / values as f64);
            for block in tex.data[..end].chunks_exact(16) {
                let block: &[u8; 16] = block.try_into()?;
                let mut pixels = [0u32; 16];
                texture2ddecoder::decode_bc7_block(block, &mut pixels);
                let reference = pixels.map(|p| {
                    let [b, g, r, a] = p.to_le_bytes();
                    [r, g, b, a]
                });
                if bc7::decode_block(block) != reference {
                    mismatched += 1;
                }
            }
            let path = scratch.join(format!("{}.bc5", tex.name));
            let at = Instant::now();
            std::fs::write(&path, &many)?;
            let write_ms = ms(at);
            let at = Instant::now();
            let back = std::fs::read(&path)?;
            let read_back_ms = ms(at);
            anyhow::ensure!(back == many, "{}: read back differs", tex.name);
            println!("{:<52} {:>9} {read:>7.1} {before_ms:>7.1} {one_ms:>7.1} {many_ms:>7.1} {write_ms:>7.1} {read_back_ms:>7.1}", tex.name, format!("{}x{}", tex.width, tex.height));
            for (sum, value) in sums.iter_mut().zip([read, before_ms, one_ms, many_ms, write_ms, read_back_ms]) {
                *sum += value;
            }
            count += 1;
            bytes += many.len();
            // `before` differs from `many` only where the old
            // encoder took the pixels of a level smaller than a
            // block from its edge; it is not compared.
            let _ = before;
        }
    }
    println!("{count} BC7 pictures, {:.1} MB as BC5; blocks decoded unlike texture2ddecoder: {mismatched}", bytes as f64 / 1e6);
    println!("ms in all: read from the bundle {:.0}, before {:.0}, own on 1 thread {:.0}, own on {threads} {:.0}, written {:.0}, read back {:.0}", sums[0], sums[1], sums[2], sums[3], sums[4], sums[5]);
    Ok(())
}

pub fn texstats(assets: &mut Assets, args: &[String], key: &str) -> Result<()> {
    let arg = |i: usize| args.get(i).map(String::as_str);
    use trp_data::unity::{extract::read_texture, texture};
    let filter = arg(2).unwrap_or("");
    for file in assets.load(key)? {
        for obj in assets.objects_of(file, class::TEXTURE_2D).collect::<Vec<_>>() {
            let tex = read_texture(&assets, obj)?;
            if !tex.name.contains(filter) {
                continue;
            }
            let mip = texture::mips(&tex)?[0];
            let Ok(rgba) = texture::decode_rgba(&tex, mip) else {
                println!("{:<60} fmt{} (not decodable)", tex.name, tex.format);
                continue;
            };
            let mut stats = [(255u8, 0u8, 0u64); 4];
            for px in rgba.chunks_exact(4) {
                for c in 0..4 {
                    let s = &mut stats[c];
                    *s = (s.0.min(px[c]), s.1.max(px[c]), s.2 + px[c] as u64);
                }
            }
            // `texstats <bundle> <word> <folder>`: each picture is
            // also written there to be looked at, at the level
            // nearest 512 across and with nothing see-through (a
            // folder of scratch: the pictures are the install's).
            if let Some(folder) = arg(3) {
                let levels = texture::mips(&tex)?;
                let level = levels.iter().copied().find(|level| level.width <= 512).unwrap_or(mip);
                let mut small = texture::decode_rgba(&tex, level)?;
                small.chunks_exact_mut(4).for_each(|px| px[3] = 255);
                std::fs::create_dir_all(folder)?;
                let out = std::path::Path::new(folder).join(format!("{}.png", tex.name));
                let mut encoder = png::Encoder::new(std::io::BufWriter::new(std::fs::File::create(&out)?), level.width, level.height);
                encoder.set_color(png::ColorType::Rgba);
                encoder.set_depth(png::BitDepth::Eight);
                encoder.write_header()?.write_image_data(&small)?;
                println!("    written to {}", out.display());
            }
            let n = (rgba.len() / 4) as u64;
            let chan = |c: usize| format!("{}..{} avg {}", stats[c].0, stats[c].1, stats[c].2 / n);
            println!(
                "{:<60} fmt{} srgb={} R[{}] G[{}] B[{}] A[{}]",
                tex.name, tex.format, tex.srgb, chan(0), chan(1), chan(2), chan(3)
            );
        }
    }
    Ok(())
}
