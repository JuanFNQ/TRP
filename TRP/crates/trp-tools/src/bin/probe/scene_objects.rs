//! A build's serialized files and scene objects: classes, pointers, fields and raw bytes.

use anyhow::Result;
use trp_data::unity::{class, Assets};

use crate::printing::class_label;

// One of the build's own serialized files (`sharedassets44.assets`):
// its engine version, whether it keeps its classes' type trees, and
// how many objects of each class it holds.
pub fn file(assets: &mut Assets, file: &str) -> Result<()> {
    let data = std::fs::read(assets.install.root.join("EscapeFromTarkov_Data").join(file))?;
    let meta = trp_data::unity::SerializedFile::parse(&data)?;
    println!("{file}: format {}, engine {}, type trees {}", meta.version, meta.unity_version, meta.has_type_trees);
    let mut classes: std::collections::BTreeMap<i32, usize> = std::collections::BTreeMap::new();
    for object in &meta.objects {
        *classes.entry(object.class_id).or_default() += 1;
    }
    for (class, count) in classes {
        println!("  class {class}: {count}");
    }
    Ok(())
}

// The animation clips of one of the build's own files, read by the
// tree of a clip in a bundle of the install's (`sceneclips
// sharedassets44.assets assets/content/weapons/animations/simple_animations.bundle
// [part of a name]`): each curve that drives a property of a
// component, with its values at the clip's start and end, the
// object's path and the property named where they are among the
// health panel's objects and the screen's usual properties.
pub fn sceneclips(assets: &mut Assets, args: &[String], file: &str) -> Result<()> {
    let arg = |i: usize| args.get(i).map(String::as_str);
    let donor = arg(2).ok_or_else(|| anyhow::anyhow!("a bundle that has a clip"))?;
    let wanted = arg(3).unwrap_or("").to_lowercase();
    let tree = trp_data::interface::screen::tree_of(&assets.install, donor, trp_data::unity::class::ANIMATION_CLIP)?;
    let mut files = trp_data::interface::screen::ScenePictures::new(&assets.install)?;
    // The names the clips' numbers may be for: every object's path
    // below a view of the health panel, and below the panel.
    let mut paths: std::collections::HashMap<u32, String> = std::collections::HashMap::new();
    if let Ok(gear) = trp_data::interface::screen::GearPanel::load(&assets.install) {
        if let Ok(health) = trp_data::interface::screen::HealthPanel::load(&assets.install, &gear.file) {
            for node in &health.panel.nodes {
                let mut path = node.path.as_str();
                loop {
                    paths.insert(trp_data::unity::anim::path_hash(path), path.to_owned());
                    match path.split_once('/') {
                        Some((_, rest)) => path = rest,
                        None => break,
                    }
                }
            }
        }
    }
    paths.insert(trp_data::unity::anim::path_hash(""), "(the root)".to_owned());
    let properties: std::collections::HashMap<u32, &str> = ["m_Color.r", "m_Color.g", "m_Color.b", "m_Color.a", "m_IsActive", "m_Enabled", "m_FillAmount", "m_LocalScale.x", "m_LocalScale.y", "m_AnchoredPosition.x", "m_AnchoredPosition.y", "m_SizeDelta.x", "m_SizeDelta.y", "m_Alpha", "m_fontColor.a"]
        .iter()
        .map(|name| (trp_data::unity::anim::path_hash(name), *name))
        .collect();
    for id in files.objects(file, trp_data::unity::class::ANIMATION_CLIP)? {
        let clip = match files.read_by(file, id, trp_data::unity::class::ANIMATION_CLIP, &tree).and_then(|v| trp_data::unity::anim::Clip::of(&v)) {
            Ok(clip) => clip,
            Err(e) => {
                println!("clip {id}: {e:#}");
                continue;
            }
        };
        if !clip.name.to_lowercase().contains(&wanted) {
            continue;
        }
        // Its values at nine times from its start to its end.
        let samples: Vec<Vec<f32>> = (0..9)
            .map(|n| {
                let mut values = Vec::new();
                clip.sample(clip.duration() * n as f32 / 8.0, &mut values);
                values
            })
            .collect();
        println!("clip {id} {:?}: {:.3} s{}", clip.name, clip.duration(), if clip.looping { ", looping" } else { "" });
        for b in &clip.others {
            let path = paths.get(&b.path).cloned().unwrap_or_else(|| format!("#{:08x}", b.path));
            let property = properties.get(&b.attribute).map_or_else(|| format!("#{:08x}", b.attribute), |name| name.to_string());
            let values: Vec<String> = samples.iter().map(|values| format!("{:.3}", values.get(b.curve).copied().unwrap_or(f32::NAN))).collect();
            println!("  {path} (class {}) {property}: {}", b.class, values.join(" "));
        }
    }
    Ok(())
}

// The smallest bundles of the install's that keep a class with its
// tree (`classfind 74`), smallest first, ten of them: where to
// borrow the tree to read the build's own files by.
pub fn classfind(assets: &mut Assets, class: &str) -> Result<()> {
    let class: i32 = class.parse()?;
    let mut keys: Vec<(u64, String)> = assets
        .manifest
        .entries
        .keys()
        .filter_map(|key| Some((std::fs::metadata(assets.install.bundle_path(key)).ok()?.len(), key.clone())))
        .collect();
    keys.sort();
    let mut found = 0;
    for (size, key) in keys {
        let Ok(bundle) = trp_data::unity::Bundle::open(&assets.install.bundle_path(&key)) else { continue };
        let has = bundle.nodes.iter().filter(|node| node.is_serialized_file()).any(|node| {
            trp_data::unity::SerializedFile::parse(bundle.file(node)).is_ok_and(|meta| meta.types.iter().any(|ty| ty.class_id == class && !ty.tree.is_empty()))
        });
        if has {
            println!("{size:>10}  {key}");
            found += 1;
            if found == 10 {
                break;
            }
        }
    }
    Ok(())
}

// Where a script's pointers lead in its panel (`points level44
// "InventoryScreen/Items Panel/Stash Panel/Simple Panel"
// SimpleStashPanel`): each place in its fields that names an
// object of the panel, a script on one, or one's place.
pub fn points(assets: &mut Assets, args: &[String], file: &str) -> Result<()> {
    let arg = |i: usize| args.get(i).map(String::as_str);
    let wanted = arg(2).ok_or_else(|| anyhow::anyhow!("which object?"))?;
    let class = arg(3).ok_or_else(|| anyhow::anyhow!("which script?"))?;
    let scene = trp_data::interface::screen::Scene::load(&assets.install, file, true)?;
    let top = (0..scene.nodes.len()).find(|&node| scene.path(node).ends_with(wanted)).ok_or_else(|| anyhow::anyhow!("no {wanted}"))?;
    let panel = trp_data::interface::screen::ScenePanel::of(&scene, top);
    let fields = panel.script(0, class).ok_or_else(|| anyhow::anyhow!("no {class} on it"))?;
    println!("{wanted} {class}: {} bytes of fields", fields.len());
    for at in (0..fields.len().saturating_sub(11)).step_by(4) {
        let object = panel.pointer_at(fields, at).map(|index| ("object or script", index));
        let place = panel.transform_at(fields, at).map(|index| ("place", index));
        if let Some((what, index)) = object.or(place) {
            println!("  {at:5}: {what} {}", panel.nodes[index].path);
        }
    }
    Ok(())
}

// One object of a build's file by its number, its bytes as words
// from a place on (`raw sharedassets0.assets 1234 0 64`), each
// as a number, a float and its letters.
// A TextMeshPro font asset's material and its floats and colours
// (`fontmaterial resources.assets 34143`).
// Each object whose path ends so, and those under it, with its
// parts' engine classes (223 a canvas: then its fields, which say
// whether it sorts on its own and by what order).
pub fn parts(assets: &mut Assets, args: &[String], file: &str) -> Result<()> {
    let arg = |i: usize| args.get(i).map(String::as_str);
    let scene = trp_data::interface::screen::Scene::load(&assets.install, file, true)?;
    let end = arg(2).unwrap_or("");
    let top = (0..scene.nodes.len()).find(|&index| scene.path(index).ends_with(end)).ok_or_else(|| anyhow::anyhow!("no object's path ends with {end:?}"))?;
    let top_path = scene.path(top);
    for index in (0..scene.nodes.len()).filter(|&index| scene.path(index).starts_with(&top_path)) {
        let parts = scene.parts_of(index);
        let classes: Vec<String> = parts.iter().map(|(_, class)| class.to_string()).collect();
        println!("{} [{}]", scene.path(index).trim_start_matches(&top_path), classes.join(" "));
        for (id, class) in parts.iter().filter(|(_, class)| *class == 223 || *class == 65) {
            let (_, bytes) = trp_data::interface::screen::object_bytes(&assets.install, file, *id)?;
            let words: Vec<String> = bytes.chunks(4).map(|w| format!("{:?}", w)).collect();
            println!("    canvas {class} {id}: {}", words.join(" "));
        }
    }
    Ok(())
}

// The pointers of a scene object's script into other files, each
// with the script it points to (`scriptptrs level49 UIContext ItemUiContext`).
pub fn scriptptrs(assets: &mut Assets, args: &[String], file: &str) -> Result<()> {
    let arg = |i: usize| args.get(i).map(String::as_str);
    let scene = trp_data::interface::screen::Scene::load(&assets.install, file, true)?;
    let end = arg(2).unwrap_or("");
    let class = arg(3).unwrap_or("");
    let top = (0..scene.nodes.len()).find(|&index| scene.path(index).ends_with(end)).ok_or_else(|| anyhow::anyhow!("no object's path ends with {end:?}"))?;
    let panel = trp_data::interface::screen::ScenePanel::of(&scene, top);
    let fields = panel.script(0, class).ok_or_else(|| anyhow::anyhow!("no {class} there"))?;
    for start in (0..fields.len().saturating_sub(11)).step_by(4) {
        let to_file = i32::from_le_bytes(fields[start..start + 4].try_into()?);
        let to = i64::from_le_bytes(fields[start + 4..start + 12].try_into()?);
        if (1..64).contains(&to_file) && to > 0 && to < 1 << 40 {
            let name = panel.file_of(to_file).unwrap_or_default();
            let script = trp_data::interface::screen::script_name(&assets.install, &name, to).unwrap_or_default();
            println!("  at {start}: {name} {to} {script}");
        }
    }
    Ok(())
}

// `build-objects <file> <class>`: the numbers of a class's objects in
// one of the build's own files.
pub fn build_objects(assets: &mut Assets, args: &[String], file: &str) -> Result<()> {
    let arg = |i: usize| args.get(i).map(String::as_str);
    let class: i32 = arg(2).ok_or_else(|| anyhow::anyhow!("a class needed"))?.parse()?;
    let mut pictures = trp_data::interface::screen::ScenePictures::new(&assets.install)?;
    println!("{:?}", pictures.objects(file, class)?);
    Ok(())
}

// `typed <file> <number> <class> [bundle] [depth]`: an object of one
// of the build's own files (which keep no trees) read by its class's
// tree borrowed from a bundle (the effects bundle unless told; the
// shaders' for a shader), and printed.
pub fn typed(assets: &mut Assets, args: &[String], file: &str) -> Result<()> {
    let arg = |i: usize| args.get(i).map(String::as_str);
    let id: i64 = arg(2).ok_or_else(|| anyhow::anyhow!("the object's number needed"))?.parse()?;
    let class: i32 = arg(3).ok_or_else(|| anyhow::anyhow!("its class needed"))?.parse()?;
    let key = arg(4).unwrap_or(trp_data::audio::sound::EFFECTS);
    let depth = arg(5).map_or(Ok(8), str::parse)?;
    let tree = trp_data::interface::screen::tree_of(&assets.install, key, class)?;
    let mut pictures = trp_data::interface::screen::ScenePictures::new(&assets.install)?;
    println!("{}", pictures.read_by(file, id, class, &tree)?.pretty(depth, 40));
    Ok(())
}

// The files a build's file points into, by their numbers in its
// pointers (`externals resources.assets`: 1 is the first).
pub fn externals(assets: &mut Assets, file: &str) -> Result<()> {
    let path = assets.install.root.join("EscapeFromTarkov_Data").join(file);
    let data = std::fs::read(&path)?;
    let meta = trp_data::unity::serialized::SerializedFile::parse(&data)?;
    for (at, name) in meta.externals.iter().enumerate() {
        println!("{} {name}", at + 1);
    }
    Ok(())
}

// An object's bytes from one place on, LZ4 blocks unpacked, as
// words (`unpack globalgamemanagers.assets 284 3024 710 1464`: a
// shader's compiled programs).
pub fn unpack(assets: &mut Assets, args: &[String], file: &str) -> Result<()> {
    let arg = |i: usize| args.get(i).map(String::as_str);
    let number = |at: usize| -> Result<usize> { Ok(arg(at).ok_or_else(|| anyhow::anyhow!("unpack takes an object, an offset and two lengths"))?.parse()?) };
    let (id, from, packed, unpacked) = (number(2)? as i64, number(3)?, number(4)?, number(5)?);
    let (_, bytes) = trp_data::interface::screen::object_bytes(&assets.install, file, id)?;
    let out = lz4_flex::block::decompress(&bytes[from..from + packed], unpacked)?;
    if let Some(path) = arg(6) {
        std::fs::write(path, &out)?;
    }
    for at in (0..out.len()).step_by(4) {
        let Some(word) = out.get(at..at + 4) else { break };
        let word: [u8; 4] = word.try_into()?;
        let letters: String = word.iter().map(|&b| if b.is_ascii_graphic() { b as char } else { '.' }).collect();
        println!("  {at:6}: {:08x} {:11} {:14} {letters}", u32::from_le_bytes(word), i32::from_le_bytes(word), f32::from_le_bytes(word));
    }
    Ok(())
}

pub fn raw(assets: &mut Assets, args: &[String], file: &str) -> Result<()> {
    let arg = |i: usize| args.get(i).map(String::as_str);
    let id: i64 = arg(2).ok_or_else(|| anyhow::anyhow!("which object?"))?.parse()?;
    let from: usize = arg(3).and_then(|n| n.parse().ok()).unwrap_or(0);
    let count: usize = arg(4).and_then(|n| n.parse().ok()).unwrap_or(64);
    let (class, bytes) = trp_data::interface::screen::object_bytes(&assets.install, file, id)?;
    println!("{file} {id}: class {class}, {} bytes", bytes.len());
    for at in (from..bytes.len().min(from + 4 * count)).step_by(4) {
        let Some(word) = bytes.get(at..at + 4) else { break };
        let word: [u8; 4] = word.try_into()?;
        let letters: String = word.iter().map(|&b| if b.is_ascii_graphic() { b as char } else { '.' }).collect();
        println!("  {at:6}: {:11} {:14} {letters}", i32::from_le_bytes(word), f32::from_le_bytes(word));
    }
    Ok(())
}

// The objects of one of the install's scene files whose name has
// that part, each with what hangs from it so many levels down and
// how each is laid out on its screen: `screen level49 "Slot" 1`.
pub fn screen(assets: &mut Assets, args: &[String], file: &str) -> Result<()> {
    let arg = |i: usize| args.get(i).map(String::as_str);
    let scene = trp_data::interface::screen::Scene::load(&assets.install, file, true)?;
    let wanted = arg(2).unwrap_or("").to_lowercase();
    let depth: usize = arg(3).and_then(|d| d.parse().ok()).unwrap_or(1);
    fn show(scene: &trp_data::interface::screen::Scene, index: usize, indent: usize, depth: usize) {
        let node = &scene.nodes[index];
        let layout = node.layout.map_or(String::new(), |l| {
            format!(
                "  anchors ({}, {})-({}, {})  offset ({}, {})  size ({}, {})  pivot ({}, {})  scale ({}, {})",
                l.anchor_min[0], l.anchor_min[1], l.anchor_max[0], l.anchor_max[1], l.offset[0], l.offset[1], l.size_delta[0], l.size_delta[1], l.pivot[0], l.pivot[1], l.scale[0], l.scale[1]
            )
        });
        let scripts = if node.scripts.is_empty() { String::new() } else { format!("  [{}]", node.scripts.join(", ")) };
        println!("{}{}{}{layout}{scripts}", "  ".repeat(indent), node.name, if node.active { "" } else { " (off)" });
        if depth > 0 {
            for &child in &node.children {
                show(scene, child, indent + 1, depth - 1);
            }
        }
    }
    println!("{file}: {} objects", scene.nodes.len());
    for index in (0..scene.nodes.len()).filter(|&index| scene.nodes[index].name.to_lowercase().contains(&wanted)) {
        println!("{}", scene.path(index));
        show(&scene, index, 1, depth);
    }
    Ok(())
}

// Each script's own fields on the objects of a scene whose path
// ends so, word by word as a whole number and as a real number
// (`fields level44 "Items Panel/LeftSide"`): how a layout script of
// the engine's is set, read by its known order of fields.
pub fn fields(assets: &mut Assets, args: &[String], file: &str) -> Result<()> {
    let arg = |i: usize| args.get(i).map(String::as_str);
    let scene = trp_data::interface::screen::Scene::load(&assets.install, file, true)?;
    let end = arg(2).unwrap_or("");
    // The files a pointer's first number counts from 1 in.
    let data = std::fs::read(assets.install.root.join("EscapeFromTarkov_Data").join(file))?;
    for (n, outside) in trp_data::unity::SerializedFile::parse(&data)?.externals.iter().enumerate() {
        println!("file {}: {outside}", n + 1);
    }
    for index in (0..scene.nodes.len()).filter(|&index| scene.path(index).ends_with(end)) {
        println!("{}  [{}]", scene.path(index), scene.nodes[index].scripts.join(", "));
        for behaviour in scene.behaviours.iter().filter(|behaviour| behaviour.node == index) {
            let fields = scene.fields(behaviour);
            println!("  the script {}: {} bytes of fields", behaviour.id, fields.len());
            for (n, word) in fields.chunks(4).take(400).enumerate() {
                let mut bytes = [0u8; 4];
                bytes[..word.len()].copy_from_slice(word);
                println!("    {:3}: {:02x?}  {}  {}", n * 4, bytes, i32::from_le_bytes(bytes), f32::from_le_bytes(bytes));
            }
        }
    }
    Ok(())
}

// What the scripts on the objects of a scene whose path ends so
// point to among the scene's own scripts: each such pointer in their
// fields, by where it lies, with the object it points to and that
// object's scripts (`pointers level44 "InventoryScreen/Items Panel/Stash Panel/Simple Panel"`).
pub fn pointers(assets: &mut Assets, args: &[String], file: &str) -> Result<()> {
    let arg = |i: usize| args.get(i).map(String::as_str);
    let scene = trp_data::interface::screen::Scene::load(&assets.install, file, true)?;
    let end = arg(2).unwrap_or("");
    for index in (0..scene.nodes.len()).filter(|&index| scene.path(index).ends_with(end)) {
        println!("{}  [{}]", scene.path(index), scene.nodes[index].scripts.join(", "));
        for behaviour in scene.behaviours.iter().filter(|behaviour| behaviour.node == index) {
            let fields = scene.fields(behaviour);
            println!("  the script {}: {} bytes of fields", behaviour.id, fields.len());
            for start in (0..fields.len().saturating_sub(11)).step_by(4) {
                let mut r = trp_data::unity::reader::Reader::new(&fields[start..]);
                let (Ok(file), Ok(path)) = (r.i32(), r.i64()) else { continue };
                let Some(to) = scene.owner(path).filter(|_| file == 0 && path != 0) else { continue };
                println!("    at {start}: {}  [{}]", scene.path(to), scene.nodes[to].scripts.join(", "));
            }
        }
    }
    Ok(())
}

pub fn scenescript(assets: &mut Assets, args: &[String], file: &str) -> Result<()> {
    let arg = |i: usize| args.get(i).map(String::as_str);
    let scene = trp_data::interface::screen::Scene::load(&assets.install, file, true)?;
    let root = arg(2).unwrap_or("");
    let top = (0..scene.nodes.len()).find(|&index| scene.path(index).ends_with(root)).ok_or_else(|| anyhow::anyhow!("no object's path ends with {root:?}"))?;
    let panel = trp_data::interface::screen::ScenePanel::of(&scene, top);
    let end = arg(3).unwrap_or("");
    let class = arg(4).ok_or_else(|| anyhow::anyhow!("a script's name"))?;
    let index = (0..panel.nodes.len()).find(|&index| panel.nodes[index].path.ends_with(end)).ok_or_else(|| anyhow::anyhow!("no object's path ends with {end:?}"))?;
    println!("{}: {:?}", panel.nodes[index].path, panel.nodes[index].layout);
    let fields = panel.script(index, class).ok_or_else(|| anyhow::anyhow!("no {class} there"))?;
    for (at, word) in fields.chunks(4).enumerate() {
        let mut four = [0u8; 4];
        four[..word.len()].copy_from_slice(word);
        println!("{:4} {:02x?} {:12} {}", at * 4, word, i32::from_le_bytes(four), f32::from_le_bytes(four));
    }
    Ok(())
}

// The objects of a loose file whose bytes hold a word (`holding
// sharedassets44.assets Intelligence`): number, class, size.
pub fn holding(assets: &mut Assets, args: &[String], rel: &str) -> Result<()> {
    let arg = |i: usize| args.get(i).map(String::as_str);
    let word = arg(2).ok_or_else(|| anyhow::anyhow!("which word?"))?.as_bytes();
    let data = std::fs::read(assets.install.root.join("EscapeFromTarkov_Data").join(rel))?;
    let meta = trp_data::unity::SerializedFile::parse(&data)?;
    for o in &meta.objects {
        if data[o.offset..o.offset + o.size].windows(word.len()).any(|w| w == word) {
            println!("{:>22}  class {:>4} {:<22} {:>9}", o.path_id, o.class_id, class_label(o.class_id), o.size);
            // The sprites of this file it points to: a pointer is the file's
            // number (0, itself) and the object's, at any place a four apart.
            let bytes = &data[o.offset..o.offset + o.size];
            for at in (0..bytes.len().saturating_sub(11)).step_by(4) {
                let file_id = i32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
                let path_id = i64::from_le_bytes(bytes[at + 4..at + 12].try_into().unwrap());
                if file_id == 0 && path_id != 0 && meta.objects.iter().any(|t| t.path_id == path_id && t.class_id == 213) {
                    println!("      at {at}: sprite {path_id}");
                } else if file_id > 0 && (file_id as usize) <= meta.externals.len() && path_id != 0 && path_id.unsigned_abs() < 1 << 48 {
                    println!("      at {at}: {} {path_id}", meta.externals[file_id as usize - 1]);
                }
            }
        }
    }
    Ok(())
}

// The sprites one object of a loose file points to (`spriteptrs
// resources.assets 34158`): each pointer found, a four apart, that
// leads to a sprite, with its name and size.
pub fn spriteptrs(assets: &mut Assets, args: &[String], rel: &str) -> Result<()> {
    let arg = |i: usize| args.get(i).map(String::as_str);
    let id: i64 = arg(2).ok_or_else(|| anyhow::anyhow!("which object?"))?.parse()?;
    let data = std::fs::read(assets.install.root.join("EscapeFromTarkov_Data").join(rel))?;
    let meta = trp_data::unity::SerializedFile::parse(&data)?;
    let o = meta.objects.iter().find(|o| o.path_id == id).ok_or_else(|| anyhow::anyhow!("no object {id}"))?;
    let bytes = &data[o.offset..o.offset + o.size];
    let mut pictures = trp_data::interface::screen::ScenePictures::new(&assets.install)?;
    for at in (0..bytes.len().saturating_sub(11)).step_by(4) {
        let file_id = i32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        let path_id = i64::from_le_bytes(bytes[at + 4..at + 12].try_into().unwrap());
        if path_id == 0 || file_id < 0 || file_id as usize > meta.externals.len() {
            continue;
        }
        let file = match file_id {
            0 => rel.to_owned(),
            n => meta.externals[n as usize - 1].rsplit('/').next().unwrap_or_default().to_owned(),
        };
        if let Ok(cut) = pictures.sprite(&file, path_id) {
            println!("  at {at}: {file} {path_id} {:?} rect {:?}", cut.name, cut.rect);
        }
    }
    Ok(())
}

pub fn rawfile(assets: &mut Assets, args: &[String], rel: &str) -> Result<()> {
    let arg = |i: usize| args.get(i).map(String::as_str);
    // A serialized file that sits loose in the data folder, not in a bundle.
    use trp_data::unity::SerializedFile;
    let path = assets.install.root.join("EscapeFromTarkov_Data").join(rel);
    let data = std::fs::read(&path)?;
    let meta = SerializedFile::parse(&data)?;
    println!(
        "{} v{} {} typetrees={} objects={} externals={}",
        rel,
        meta.version,
        meta.unity_version,
        meta.has_type_trees,
        meta.objects.len(),
        meta.externals.len()
    );
    let filter = arg(2).unwrap_or("");
    for o in meta.objects.iter().filter(|o| o.class_id == class::MONO_BEHAVIOUR) {
        // MonoBehaviour header: game object (12), enabled (4), script (12), then the name.
        let bytes = &data[o.offset..o.offset + o.size];
        if bytes.len() < 32 {
            continue;
        }
        let len = u32::from_le_bytes(bytes[28..32].try_into().unwrap()) as usize;
        let Some(name) = bytes.get(32..32 + len).and_then(|b| std::str::from_utf8(b).ok()) else { continue };
        if !filter.is_empty() && name.to_lowercase().contains(&filter.to_lowercase()) {
            let script_file = i32::from_le_bytes(bytes[16..20].try_into().unwrap());
            let script_path = i64::from_le_bytes(bytes[20..28].try_into().unwrap());
            println!("  {:>12} size {:>7} script(file {script_file}, path {script_path})  {name}", o.path_id, o.size);
        }
    }
    Ok(())
}
