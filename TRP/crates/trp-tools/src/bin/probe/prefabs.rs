//! Prefabs, resources and sprites found by name in the bundles.

use anyhow::Result;
use trp_data::unity::Assets;

// The inventory screen's bar of tabs as the app reads it: each tab
// with what its scripts name, its words in the install's English
// and how wide they are in their font.
// A sprite of the build's resources by its path (`rsprite <path>
// <out.png>`): its cut and size, written as a picture, its top up.
pub fn rsprite(assets: &mut Assets, args: &[String], path: &str) -> Result<()> {
    let arg = |i: usize| args.get(i).map(String::as_str);
    let (file, id) = trp_data::interface::screen::resource_sprite(&assets.install, path).ok_or_else(|| anyhow::anyhow!("no resource sprite at {path}"))?;
    let mut pictures = trp_data::interface::screen::ScenePictures::new(&assets.install)?;
    let cut = pictures.sprite(&file, id)?;
    println!("{path}: {file} {id}, {:?} {:?} border {:?}", cut.name, cut.rect, cut.border);
    if let Some(out_path) = arg(2) {
        let (w, h, out) = trp_data::interface::screen::sprite_rgba(&mut pictures, &file, id)?;
        let mut encoder = png::Encoder::new(std::io::BufWriter::new(std::fs::File::create(out_path)?), w, h);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.write_header()?.write_image_data(&out)?;
    }
    Ok(())
}

// A sprite of one of the build's files by its name (`nsprite
// sharedassets44.assets Intelligence <out.png>`): its size, written
// as a picture, its top up (the exe's icon: trp-app's `build.rs`).
pub fn nsprite(assets: &mut Assets, args: &[String], file: &str) -> Result<()> {
    let arg = |i: usize| args.get(i).map(String::as_str);
    let name = arg(2).ok_or_else(|| anyhow::anyhow!("which sprite?"))?;
    let (w, h, out) = trp_data::interface::screen::sprite_rgba_named(&assets.install, file, name)?;
    println!("{file} {name:?}: {w}x{h}");
    if let Some(out_path) = arg(3) {
        let mut encoder = png::Encoder::new(std::io::BufWriter::new(std::fs::File::create(out_path)?), w, h);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.write_header()?.write_image_data(&out)?;
    }
    Ok(())
}

// A model's objects by a bundle's key (`model assets/content/items/
// ammo/patrons/patron_725.bundle`), as the icons read it: each one's
// path, whether it is on, where it lies in its parent, whether it
// draws, its scripts.
pub fn model(assets: &mut Assets, key: &str) -> Result<()> {
    let (hierarchy, pose) = trp_data::interface::icon::model_of(&mut *assets, key)?;
    println!("{key}: {} objects, posed {}", hierarchy.nodes.len(), pose.is_some());
    for (index, node) in hierarchy.nodes.iter().enumerate() {
        let [x, y, z] = node.translation;
        println!("  {:60} {} at ({x:.4}, {y:.4}, {z:.4}){}  [{}]", hierarchy.path(index), if node.active { "on " } else { "off" }, if node.renderer.is_some() { " draws" } else { "" }, node.scripts.join(", "));
    }
    Ok(())
}

// A sprite of one of the build's files by its number (`sprite
// resources.assets 4222`): its cut, and its texture's pixels where
// it is small (16 by 16 at most), each row from the lowest.
pub fn sprite(assets: &mut Assets, args: &[String], file: &str) -> Result<()> {
    let arg = |i: usize| args.get(i).map(String::as_str);
    let id: i64 = arg(2).ok_or_else(|| anyhow::anyhow!("which sprite?"))?.parse()?;
    let mut pictures = trp_data::interface::screen::ScenePictures::new(&assets.install)?;
    let cut = pictures.sprite(file, id)?;
    println!("{cut:?}");
    let texture = pictures.texture(&cut.texture.0, cut.texture.1)?;
    println!("texture {}x{} format {} srgb {}", texture.width, texture.height, texture.format, texture.srgb);
    if texture.width <= 16 && texture.height <= 16 {
        let levels = trp_data::unity::texture::mips(&texture)?;
        let pixels = trp_data::unity::texture::decode_rgba(&texture, *levels.first().ok_or_else(|| anyhow::anyhow!("no levels"))?)?;
        for row in pixels.chunks(texture.width as usize * 4) {
            println!("  {:?}", row.chunks(4).collect::<Vec<_>>());
        }
    }
    Ok(())
}

pub fn prefab(assets: &mut Assets, args: &[String], wanted: &str) -> Result<()> {
    let arg = |i: usize| args.get(i).map(String::as_str);
    // Or a script's prefab by its file and number
    // (`prefab sharedassets44.assets:9060`).
    let (file, id, scene) = match wanted.split_once(':') {
        Some((file, id)) => {
            let id: i64 = id.parse()?;
            (file.to_owned(), id, trp_data::interface::screen::Scene::load_prefab_of_script(&assets.install, file, id)?)
        }
        None => {
            let found = trp_data::interface::screen::resource_paths(&assets.install)?.into_iter().find(|(path, ..)| path == wanted).ok_or_else(|| anyhow::anyhow!("no resource is at {wanted:?}"))?;
            let (_, file, id) = found;
            let scene = trp_data::interface::screen::Scene::load_prefab(&assets.install, &file, id)?;
            (file, id, scene)
        }
    };
    let panel = trp_data::interface::screen::ScenePanel::of(&scene, 0);
    let own = panel.nodes[0].layout.size_delta;
    let size = [arg(2).and_then(|w| w.parse().ok()).unwrap_or(own[0]), arg(3).and_then(|h| h.parse().ok()).unwrap_or(own[1])];
    println!("{wanted} ({file} {id}): {} objects, {size:?}", panel.nodes.len());
    if let Some(group) = panel.nodes[0].group {
        println!("  its row: {group:?}");
    }
    // The pointers of its own scripts into other files.
    for (class, fields) in &panel.nodes[0].scripts {
        for start in (0..fields.len().saturating_sub(11)).step_by(4) {
            let mut r = trp_data::unity::reader::Reader::new(&fields[start..]);
            if let (Ok(to_file), Ok(to)) = (r.i32(), r.i64()) {
                if (0..64).contains(&to_file) && to > 0 && to < 1 << 40 {
                    let named = panel.pointer_at(fields, start).or_else(|| panel.transform_at(fields, start)).map(|node| panel.nodes[node].path.clone()).unwrap_or_default();
                    println!("  {class} at {start}: file {to_file} ({}) object {to} {named:?}", panel.file_of(to_file).unwrap_or_default());
                }
            }
        }
    }
    let mut pictures = trp_data::interface::screen::ScenePictures::new(&assets.install)?;
    for (index, node) in panel.nodes.iter().enumerate() {
        let [x, y, w, h] = panel.placed(index, size).rect;
        let classes: Vec<&str> = node.scripts.iter().map(|(class, _)| class.as_str()).collect();
        let mut line = format!("{:55} {} ({x:.1}, {y:.1}) {w:.1} x {h:.1} [{}]", node.path, if node.active { "on " } else { "off" }, classes.join(", "));
        if let Some(image) = &node.image {
            line += &format!("  image {:?} kind {}", image.color, image.kind);
            if image.kind == 3 {
                line += &format!(" fill method {} origin {} amount {} clockwise {}", image.fill_method, image.fill_origin, image.fill_amount, image.fill_clockwise);
            }
            if let Some((outside, sprite)) = image.sprite {
                let name = if outside == 0 { file.clone() } else { panel.externals.get(outside as usize - 1).map(|n| n.rsplit('/').next().unwrap_or(n).to_owned()).unwrap_or_default() };
                match pictures.sprite(&name, sprite) {
                    Ok(cut) => {
                        let texture = pictures
                            .texture(&cut.texture.0, cut.texture.1)
                            .map(|t| {
                                // Its first level's pixels: the least, most and mean of each channel.
                                let pixels = trp_data::unity::texture::mips(&t).ok().and_then(|levels| trp_data::unity::texture::decode_rgba(&t, *levels.first()?).ok()).unwrap_or_default();
                                let stats: Vec<String> = (0..4)
                                    .map(|c| {
                                        let values: Vec<u32> = pixels.iter().skip(c).step_by(4).map(|&v| v as u32).collect();
                                        let mean = values.iter().sum::<u32>() as f32 / values.len().max(1) as f32;
                                        format!("{}..{} ~{mean:.0}", values.iter().min().unwrap_or(&0), values.iter().max().unwrap_or(&0))
                                    })
                                    .collect();
                                format!("{}x{} format {} srgb {} mips {} rgba {}", t.width, t.height, t.format, t.srgb, t.mip_count, stats.join(" "))
                            })
                            .unwrap_or_else(|e| format!("{e:#}"));
                        line += &format!(" sprite {:?} {:?} border {:?} of {texture}", cut.name, cut.rect, cut.border)
                    }
                    Err(e) => line += &format!(" sprite {name} {sprite}: {e:#}"),
                }
            }
        }
        if let Some(text) = &node.text {
            line += &format!("  text {:?} size {} colour {:?} align {}/{} style {} font {:?} spacing {} kerning {} margin {:?} wraps {} overflow {}", text.text, text.size, text.color, text.horizontal, text.vertical, text.style, text.font, text.spacing, text.kerning, text.margin, text.wraps, text.overflow);
        }
        // `PROBE_LAYOUT=1`: its own layout as kept, its row and what
        // it asks of the row it is in; a grid's own figures (padding,
        // corner, axis, cell, spacing, constraint and its count).
        if std::env::var_os("PROBE_LAYOUT").is_some() {
            let l = &node.layout;
            line += &format!("\n      anchors {:?}-{:?} at {:?} size {:?} pivot {:?} turn {} group {:?} element {:?}", l.anchor_min, l.anchor_max, l.offset, l.size_delta, l.pivot, l.turn, node.group, node.element);
            for (class, fields) in node.scripts.iter().filter(|(class, _)| class.ends_with("GridLayoutGroup")) {
                let word = |at: usize| fields.get(at..at + 4).map(|b| [b[0], b[1], b[2], b[3]]);
                let ints: Vec<i32> = [0, 4, 8, 12, 16, 20, 24].iter().filter_map(|&at| word(at).map(i32::from_le_bytes)).collect();
                let floats: Vec<f32> = [28, 32, 36, 40].iter().filter_map(|&at| word(at).map(f32::from_le_bytes)).collect();
                let tail: Vec<i32> = [44, 48].iter().filter_map(|&at| word(at).map(i32::from_le_bytes)).collect();
                line += &format!("\n      {class}: padding and corner {ints:?} cell and spacing {floats:?} constraint {tail:?}");
            }
        }
        println!("{line}");
    }
    Ok(())
}

// The containers' column as the app reads it, and a container's
// own layout of its grids (`containers [layout name]`).
// Each picture of a screen's panel as the engine draws it
// (`panelimages grid|quick [width height]`): its rectangle, its
// kind, colour, sprite, the sprite's border and pixels per unit,
// and its scripts (a border kept a pixel to a pixel,
// `PixelPerfectSpriteScaler`).
pub fn panelimages(assets: &mut Assets, args: &[String], which: &str) -> Result<()> {
    let arg = |i: usize| args.get(i).map(String::as_str);
    let gear = trp_data::interface::screen::GearPanel::load(&assets.install)?;
    let (panel, size) = match which {
        "grid" => (trp_data::interface::screen::ContainerViews::load(&assets.install, &gear.file)?.grid.ok_or_else(|| anyhow::anyhow!("no grid view"))?, [64.0, 64.0]),
        "quick" => {
            let mut quick = trp_data::interface::screen::QuickViews::load(&assets.install, &gear.file)?;
            for slot in quick.slots.clone() {
                quick.panel.nodes[slot.object].active = true;
            }
            let rect = quick.quick_in([1920.0, 1080.0]);
            (quick.panel, [rect[2], rect[3]])
        }
        other => anyhow::bail!("which panel? grid or quick, not {other}"),
    };
    let size = [arg(2).and_then(|w| w.parse().ok()).unwrap_or(size[0]), arg(3).and_then(|h| h.parse().ok()).unwrap_or(size[1])];
    let mut pictures = trp_data::interface::screen::ScenePictures::new(&assets.install)?;
    for (index, node) in panel.nodes.iter().enumerate() {
        let Some(image) = &node.image else { continue };
        let [x, y, w, h] = panel.rect(index, size);
        let classes: Vec<&str> = node.scripts.iter().map(|(class, _)| class.as_str()).collect();
        let mut line = format!("{:60} {} ({x:.2}, {y:.2}) {w:.2} x {h:.2} kind {} colour {:?} fill centre {} ppu x{} [{}]", node.path, if node.active { "on " } else { "off" }, image.kind, image.color, image.fill_center, image.pixels_per_unit, classes.join(", "));
        if let Some((outside, sprite)) = image.sprite {
            match panel.file_of(outside).map(|name| (pictures.sprite(&name, sprite), name)) {
                Some((Ok(cut), _)) => {
                    line += &format!("\n      sprite {:?} {:?} border {:?}", cut.name, cut.rect, cut.border);
                    // Its texture's sampling: at its nearest pixel
                    // or blended, clamped or repeated.
                    if let Ok(texture) = pictures.texture(&cut.texture.0, cut.texture.1) {
                        line += &format!(" texture {}x{} point {} clamp {:?} mips {}", texture.width, texture.height, texture.point, texture.clamp, texture.mip_count);
                    }
                }
                Some((Err(e), name)) => line += &format!("\n      sprite {name} {sprite}: {e:#}"),
                None => line += &format!("\n      sprite in file {outside} {sprite}"),
            }
        }
        println!("{line}");
    }
    Ok(())
}

// The build's own Resources whose paths have a part (`resources
// ugui/layouts`): each path with its file and object.
pub fn resources_paths(assets: &mut Assets, filter: Option<&str>) -> Result<()> {
    let wanted = filter.unwrap_or("").to_lowercase();
    for (path, file, id) in trp_data::interface::screen::resource_paths(&assets.install)?.iter().filter(|(path, ..)| path.contains(&wanted)) {
        println!("{path}  {file} {id}");
    }
    Ok(())
}

// One object of the game's own `resources.assets` by its number: its
// class and, for most, its name, which comes first in it; and its
// first figures (`object 84`).
pub fn object(assets: &mut Assets, id: &str) -> Result<()> {
    let path = assets.install.root.join("EscapeFromTarkov_Data").join("resources.assets");
    let data = std::fs::read(&path)?;
    let meta = trp_data::unity::serialized::SerializedFile::parse(&data)?;
    let id: i64 = id.parse()?;
    let object = meta.objects.iter().find(|o| o.path_id == id).ok_or_else(|| anyhow::anyhow!("no object {id}"))?;
    let bytes = &data[object.offset..object.offset + object.size];
    let len = u32::from_le_bytes(bytes[0..4].try_into()?) as usize;
    let name = (len < 200).then(|| String::from_utf8_lossy(&bytes[4..4 + len.min(bytes.len() - 4)]).into_owned()).unwrap_or_default();
    println!("object {id}: class {}, {} bytes, named {name:?}", object.class_id, bytes.len());
    let text: String = bytes.iter().take(4096).map(|&b| if b.is_ascii_graphic() || b == b' ' { b as char } else { '.' }).collect();
    println!("{text}");
    Ok(())
}

// Game objects of the game's own `resources.assets` (no type trees)
// whose name has a word: each component's class and path id; a
// transform's place and turn; the first figures of anything else,
// as floats and as integers, to be read by hand.
pub fn resources(assets: &mut Assets, args: &[String], wanted: &str) -> Result<()> {
    let arg = |i: usize| args.get(i).map(String::as_str);
    let path = assets.install.root.join("EscapeFromTarkov_Data").join("resources.assets");
    let data = std::fs::read(&path)?;
    let meta = trp_data::unity::serialized::SerializedFile::parse(&data)?;
    let bytes_of = |path_id: i64| meta.objects.iter().find(|o| o.path_id == path_id).map(|o| (o.class_id, &data[o.offset..o.offset + o.size]));
    // A script's class, from the file its pointer names (a script
    // object begins with its name), as `screen::Scene` finds it.
    let folder = path.parent().map(std::path::Path::to_path_buf).unwrap_or_default();
    let mut outside: std::collections::HashMap<i32, (Vec<u8>, trp_data::unity::serialized::SerializedFile)> = std::collections::HashMap::new();
    let mut script_of = |file: i32, id: i64| -> Option<String> {
        let listed = meta.externals.get(usize::try_from(file).ok()?.checked_sub(1)?)?;
        if !outside.contains_key(&file) {
            let name = listed.rsplit('/').next().unwrap_or(listed);
            let data = std::fs::read(folder.join(name)).ok()?;
            let parsed = trp_data::unity::serialized::SerializedFile::parse(&data).ok()?;
            outside.insert(file, (data, parsed));
        }
        let (data, parsed) = &outside[&file];
        let found = parsed.objects.iter().find(|o| o.path_id == id)?;
        let bytes = &data[found.offset..found.offset + found.size];
        let len = u32::from_le_bytes(bytes.get(0..4)?.try_into().ok()?) as usize;
        Some(String::from_utf8_lossy(bytes.get(4..4 + len)?).into_owned())
    };
    let wanted = wanted.to_lowercase();
    for object in meta.objects.iter().filter(|o| o.class_id == 1) {
        let mut r = trp_data::unity::reader::Reader::new(&data[object.offset..object.offset + object.size]);
        let count = r.u32()? as usize;
        let mut components = Vec::new();
        for _ in 0..count {
            let (file, id) = (r.i32()?, r.i64()?);
            components.push((file, id));
        }
        let _layer = r.u32()?;
        let len = r.u32()? as usize;
        let name = String::from_utf8_lossy(r.bytes(len)?).into_owned();
        // `#<number>`: the game object of that number.
        let by_number = wanted.strip_prefix('#').and_then(|n| n.parse::<i64>().ok());
        if by_number.map_or(!name.to_lowercase().contains(&wanted), |n| n != object.path_id) {
            continue;
        }
        println!("{name} (game object {}):", object.path_id);
        for (file, id) in components {
            let Some((class, bytes)) = bytes_of(id).filter(|_| file == 0) else {
                println!("  component in file {file}, {id}");
                continue;
            };
            let mut r = trp_data::unity::reader::Reader::new(bytes);
            r.skip(12)?;
            match class {
                4 => {
                    let q = [r.f32()?, r.f32()?, r.f32()?, r.f32()?];
                    let p = [r.f32()?, r.f32()?, r.f32()?];
                    let s = [r.f32()?, r.f32()?, r.f32()?];
                    let children = r.u32()?;
                    let mut names = Vec::new();
                    for _ in 0..children {
                        let (_, child) = (r.i32()?, r.i64()?);
                        // The child transform's own game object, and its name.
                        let name = bytes_of(child).and_then(|(_, t)| {
                            let go = i64::from_le_bytes(t[4..12].try_into().ok()?);
                            let (_, g) = bytes_of(go)?;
                            let n = u32::from_le_bytes(g[0..4].try_into().ok()?) as usize;
                            let at = 4 + 12 * n + 4;
                            let len = u32::from_le_bytes(g[at..at + 4].try_into().ok()?) as usize;
                            Some(String::from_utf8_lossy(&g[at + 4..at + 4 + len]).into_owned())
                        });
                        names.push(name.unwrap_or_default());
                    }
                    let (_, father) = (r.i32()?, r.i64()?);
                    println!("  transform: turned {q:?} at {p:?} scale {s:?}, children {names:?}, parent {father}");
                }
                _ => {
                    let floats: Vec<String> = bytes[12..if arg(2) == Some("all") { bytes.len() } else { bytes.len().min(12 + 4 * 48) }].chunks_exact(4).map(|c| {
                        let (f, i) = (f32::from_le_bytes([c[0], c[1], c[2], c[3]]), i32::from_le_bytes([c[0], c[1], c[2], c[3]]));
                        if f.abs() > 1e-6 && f.abs() < 1e6 { format!("{f}") } else { format!("i{i}") }
                    }).collect();
                    // A script's class, where it is one.
                    let script = (class == 114 && bytes.len() >= 28)
                        .then(|| script_of(i32::from_le_bytes(bytes[16..20].try_into().unwrap()), i64::from_le_bytes(bytes[20..28].try_into().unwrap())))
                        .flatten()
                        .map_or(String::new(), |name| format!(" {name}"));
                    println!("  class {class}{script} ({} bytes, {id}): {}", bytes.len(), floats.join(" "));
                }
            }
        }
    }
    Ok(())
}

// One object's script's stored fields, as bytes in fours with
// what each would be as an integer and a float
// (`scriptbytes tagwindow <object> <script>`).
// A scene object's script as words, and its placement: `scenescript
// level44 "BattleUIScreen Variant" AcceptableMagsPanel AmmoSelector`
// (the screen's root, then the end of the object's path).
// Every script on a prefab's objects whose path ends so, whether
// it is on, and its fields as words (`prefabfields
// prefabs/ugui/layouts/slot_layout MasteringSlider`).
pub fn prefabfields(assets: &mut Assets, args: &[String], wanted: &str) -> Result<()> {
    let arg = |i: usize| args.get(i).map(String::as_str);
    let found = trp_data::interface::screen::resource_paths(&assets.install)?.into_iter().find(|(path, ..)| path == wanted).ok_or_else(|| anyhow::anyhow!("no resource is at {wanted:?}"))?;
    let scene = trp_data::interface::screen::Scene::load_prefab(&assets.install, &found.1, found.2)?;
    let end = arg(2).unwrap_or("");
    for index in (0..scene.nodes.len()).filter(|&index| scene.path(index).ends_with(end)) {
        println!("{}  {:?}", scene.path(index), scene.nodes[index].layout);
        for behaviour in scene.behaviours.iter().filter(|behaviour| behaviour.node == index) {
            let fields = scene.fields(behaviour);
            println!("  {} ({}) {}: {} bytes", scene.class_of(behaviour).unwrap_or("?"), behaviour.id, if behaviour.enabled { "on" } else { "off" }, fields.len());
            for (n, word) in fields.chunks(4).take(80).enumerate() {
                let mut bytes = [0u8; 4];
                bytes[..word.len()].copy_from_slice(word);
                println!("    {:3}: {:02x?}  {}  {}", n * 4, bytes, i32::from_le_bytes(bytes), f32::from_le_bytes(bytes));
            }
        }
    }
    Ok(())
}

pub fn scriptbytes(assets: &mut Assets, args: &[String], window: &str) -> Result<()> {
    let arg = |i: usize| args.get(i).map(String::as_str);
    let panel = if window == "tagwindow" { trp_data::interface::screen::tag_window(&assets.install)? } else { trp_data::interface::screen::split_dialog(&assets.install)? };
    let index: usize = arg(2).ok_or_else(|| anyhow::anyhow!("an object's number"))?.parse()?;
    let script = arg(3).ok_or_else(|| anyhow::anyhow!("a script's name"))?;
    let fields = panel.script(index, script).ok_or_else(|| anyhow::anyhow!("no {script} on it"))?;
    for (at, word) in fields.chunks(4).enumerate() {
        let mut four = [0u8; 4];
        four[..word.len()].copy_from_slice(word);
        println!("{:4} {:02x?} {:12} {}", at * 4, word, i32::from_le_bytes(four), f32::from_le_bytes(four));
    }
    Ok(())
}

// A prefab's script read as a run of the game's enum dictionaries
// (`SerializableEnumDictionary`: a list of key names, then a list
// of pointers): `prefabdicts sharedassets44.assets:11223
// InteractionButtonsContainer SimpleSpecifiedContextButtons`.
pub fn prefabdicts(assets: &mut Assets, args: &[String], wanted: &str) -> Result<()> {
    let arg = |i: usize| args.get(i).map(String::as_str);
    let (file, id) = wanted.split_once(':').ok_or_else(|| anyhow::anyhow!("file:id"))?;
    let scene = trp_data::interface::screen::Scene::load_prefab_of_script(&assets.install, file, id.parse()?)?;
    let panel = trp_data::interface::screen::ScenePanel::of(&scene, 0);
    let end = arg(2).unwrap_or("");
    let class = arg(3).ok_or_else(|| anyhow::anyhow!("a script's name"))?;
    let index = (0..panel.nodes.len()).find(|&index| panel.nodes[index].path.ends_with(end)).ok_or_else(|| anyhow::anyhow!("no object's path ends with {end:?}"))?;
    let fields = panel.script(index, class).ok_or_else(|| anyhow::anyhow!("no {class} there"))?;
    let mut at = 0usize;
    let word = |at: usize| fields.get(at..at + 4).map(|w| i32::from_le_bytes(w.try_into().unwrap_or_default()));
    while let Some(count) = word(at).filter(|count| (0..64).contains(count)) {
        at += 4;
        let mut keys = Vec::new();
        for _ in 0..count {
            let len = word(at).unwrap_or(0).max(0) as usize;
            keys.push(String::from_utf8_lossy(fields.get(at + 4..at + 4 + len).unwrap_or_default()).into_owned());
            at += 4 + len.div_ceil(4) * 4;
        }
        let values = word(at).unwrap_or(0).max(0) as usize;
        at += 4;
        let mut pointers = Vec::new();
        for _ in 0..values {
            let to_file = word(at).unwrap_or(0);
            let to = fields.get(at + 4..at + 12).map_or(0, |w| i64::from_le_bytes(w.try_into().unwrap_or_default()));
            let inside = panel.pointer_at(fields, at).map(|node| panel.nodes[node].path.clone());
            pointers.push(format!("({to_file}, {to}{})", inside.map(|path| format!(" = {path}")).unwrap_or_default()));
            at += 12;
        }
        println!("{} keys: {:?}\n  values: {}", keys.len(), keys, pointers.join(", "));
    }
    println!("read {at} of {} bytes", fields.len());
    Ok(())
}
