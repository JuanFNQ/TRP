//! Inspection tool for the install's bundles.
//!
//! ```text
//! probe find <text>                 manifest keys containing <text>
//! probe info <key>                  files, type-tree flag and object counts
//! probe names <key> [class]         objects with their names
//! probe dump <key> <path_id> [depth] [items]
//! probe tree <key>                  GameObject hierarchy with components
//! probe scripts <key>               MonoBehaviours with script class and owner
//! probe preset <weapon template id> stock build of a weapon with prefab bundles
//! probe fbx <file> [records|view|slice ...]   a binary FBX file: its models, a drawing, a slab
//! probe overlay <weapon> <fbx> <trigger model> <out.png>   a weapon with such a model over it
//! probe custom [id]                 the weapons that are not the install's, and one as built
//! ```

use std::collections::BTreeMap;

use anyhow::{bail, Result};
use trp_data::unity::{class, Assets, ObjRef};
use trp_data::sketch::Drawn;
use trp_data::Install;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let arg = |i: usize| args.get(i).map(String::as_str);
    let install = Install::locate()?;
    let mut assets = Assets::new(install)?;

    match (arg(0), arg(1)) {
        // The build's scenes, each with the file it is, and the inventory
        // screen's equipment panel as the install lays it out.
        (Some("scenes"), filter) => {
            let wanted = filter.unwrap_or("").to_lowercase();
            for (index, path) in trp_data::screen::scene_paths(&assets.install)?.iter().enumerate().filter(|(_, path)| path.to_lowercase().contains(&wanted)) {
                println!("level{index}  {path}");
            }
            let panel = trp_data::screen::GearPanel::load(&assets.install)?;
            println!("the equipment panel, from {}: {} x {}", panel.file, panel.size[0], panel.size[1]);
            for (name, [left, top, width, height]) in &panel.slots {
                println!("  {name}: left {left} top {top}, {width} x {height}");
            }
            let [width, height, up, right] = panel.quick;
            println!("the quick-use panel: {width} x {height}, {up} over the screen's lower edge, its middle {right} to the right of the screen's");
            for (key, middle, side) in &panel.quick_slots {
                println!("  {key}: its middle {middle} along the row, {side} square");
            }
        }
        // The install's own key bindings (`Control.ini`), those whose name
        // has that part: `keys Weapon`.
        (Some("keys"), filter) => {
            let wanted = filter.unwrap_or("").to_lowercase();
            let words = |source: &trp_data::install::KeySource| {
                let wheel = source.axis.iter().map(|(axis, up)| format!("{axis} {}", if *up { "+" } else { "-" }));
                source.keys.iter().cloned().chain(wheel).collect::<Vec<_>>().join(" + ")
            };
            for binding in assets.install.key_bindings().iter().filter(|binding| binding.name.to_lowercase().contains(&wanted)) {
                let variants: Vec<String> = binding.variants.iter().filter(|source| !source.is_empty()).map(words).collect();
                println!("{:28} {:12} {}", binding.name, binding.press, variants.join("   or   "));
            }
            // The axes the keys push, each pair's two ways.
            for axis in assets.install.axis_bindings().iter().filter(|axis| axis.name.to_lowercase().contains(&wanted)) {
                for [positive, negative] in axis.pairs.iter().filter(|pair| pair.iter().any(|way| !way.is_empty())) {
                    println!("{:28} {:12} {}   against   {}", axis.name, "axis", words(positive), words(negative));
                }
            }
            let display = assets.install.display_settings();
            println!("display: vsync {:?}, mode {:?} (0 full screen, 1 borderless, 2 window), resolution {:?}", display.vsync, display.mode, display.resolution);
            for name in ["DoubleClickTimeout", "MouseSensitivity", "MouseAimingSensitivity", "OpticSensitivity"] {
                println!("{name}: {:?}", assets.install.control_setting(name));
            }
            for name in ["InvertedXAxis", "InvertedYAxis"] {
                println!("{name}: {:?}", assets.install.control_flag(name));
            }
            // The engine's own axes that the bindings name, and what each
            // binding multiplies its axis by.
            for axis in assets.install.engine_axes()?.iter().filter(|axis| axis.name.starts_with("Mouse")) {
                println!(
                    "engine axis {:20} x {} a count, dead zone {}, kind {} (1 the mouse's movement), axis {} (2 the wheel)",
                    axis.name, axis.sensitivity, axis.dead, axis.kind, axis.axis
                );
            }
            for name in ["TurnX", "TurnY", "LookX", "LookY"] {
                println!("{name}: its binding multiplies by {:?}", assets.install.axis_sensitivity(name));
            }
            for name in ["PriorityWindowMode", "ItemQuickUseMode"] {
                println!("{name}: {}", assets.install.game_setting(name).unwrap_or_else(|| "(not set)".into()));
            }
        }
        // The objects of one of the install's scene files whose name has
        // that part, each with what hangs from it so many levels down and
        // how each is laid out on its screen: `screen level49 "Slot" 1`.
        (Some("screen"), Some(file)) => {
            let scene = trp_data::screen::Scene::load(&assets.install, file, true)?;
            let wanted = arg(2).unwrap_or("").to_lowercase();
            let depth: usize = arg(3).and_then(|d| d.parse().ok()).unwrap_or(1);
            fn show(scene: &trp_data::screen::Scene, index: usize, indent: usize, depth: usize) {
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
        }
        // Game objects of the game's own `resources.assets` (no type trees)
        // whose name has a word: each component's class and path id; a
        // transform's place and turn; the first figures of anything else,
        // as floats and as integers, to be read by hand.
        (Some("resources"), Some(wanted)) => {
            let path = assets.install.root.join("EscapeFromTarkov_Data").join("resources.assets");
            let data = std::fs::read(&path)?;
            let meta = trp_data::unity::serialized::SerializedFile::parse(&data)?;
            let bytes_of = |path_id: i64| meta.objects.iter().find(|o| o.path_id == path_id).map(|o| (o.class_id, &data[o.offset..o.offset + o.size]));
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
                if !name.to_lowercase().contains(&wanted) {
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
                            let floats: Vec<String> = bytes[12..bytes.len().min(12 + 4 * 48)].chunks_exact(4).map(|c| {
                                let (f, i) = (f32::from_le_bytes([c[0], c[1], c[2], c[3]]), i32::from_le_bytes([c[0], c[1], c[2], c[3]]));
                                if f.abs() > 1e-6 && f.abs() < 1e6 { format!("{f}") } else { format!("i{i}") }
                            }).collect();
                            println!("  class {class} ({} bytes, {id}): {}", bytes.len(), floats.join(" "));
                        }
                    }
                }
            }
        }
        // The game's icon camera as `icon::Studio::read` takes it, beside
        // what was read by hand on 2026-10-14.
        (Some("studio"), _) => {
            let read = trp_data::icon::Studio::read(&assets.install)?;
            println!("read: {read:#?}");
            println!("the same as read by hand: {}", read == trp_data::icon::Studio::as_read());
        }
        // An item's icon settings as its prefab has them (`icon::pose_in`):
        // the object they are on, its renderers, and for a weapon the
        // object its hands hold.
        (Some("icon"), Some(wanted)) => {
            let db = trp_data::Database::load(&assets.install)?;
            let item = db.item(wanted)?;
            let key = item.prefab_key().ok_or_else(|| anyhow::anyhow!("no prefab"))?.to_owned();
            let files = assets.load(&key)?;
            println!("{} {}: {key}", item.id, item.name);
            match trp_data::icon::pose_in(&assets, &files)? {
                Some((object, pose)) => {
                    let tree = trp_data::unity::extract::read_hierarchy(&assets, object)?;
                    println!("  PreviewPivot on {} ({} nodes, {} with a renderer): {pose:?}", tree.nodes[0].name, tree.nodes.len(), tree.nodes.iter().filter(|n| n.renderer.is_some()).count());
                    for node in tree.nodes.iter().take(6) {
                        println!("    {} parent {:?} at {:?} turned {:?} scale {:?}{}", node.name, node.parent, node.translation, node.rotation, node.scale, if node.renderer.is_some() { " (drawn)" } else { "" });
                    }
                }
                None => println!("  no PreviewPivot"),
            }
            if let Ok(weapon) = trp_data::weapon::assemble(&mut assets, &db, &item.id) {
                println!("  assembled: root {} ({} nodes)", weapon.hierarchy.nodes[0].name, weapon.hierarchy.nodes.len());
            }
        }
        // An item template of the database, by id or by a part of its
        // name: what it weighs, the cells it takes, its slots and grids.
        (Some("item"), Some(wanted)) => {
            let db = trp_data::Database::load(&assets.install)?;
            let mut found: Vec<&trp_data::database::ItemTemplate> =
                db.items.values().filter(|item| item.id == wanted || item.name.to_lowercase().contains(&wanted.to_lowercase())).collect();
            found.sort_by(|a, b| a.name.cmp(&b.name));
            let name_of = |id: &str| db.items.get(id).map_or("?", |item| item.name.as_str()).to_owned();
            let full = found.len() <= 3;
            for item in found {
                let mut chain = Vec::new();
                let mut at = item.parent.as_str();
                while let Some(parent) = db.items.get(at) {
                    chain.push(parent.name.as_str());
                    at = parent.parent.as_str();
                }
                println!("{} {}  <- {}", item.id, item.name, chain.join(" <- "));
                if !full {
                    continue;
                }
                let number = |name: &str| item.props.get(name).and_then(|v| v.as_f64()).map_or("-".to_owned(), |v| v.to_string());
                println!(
                    "  weight {}  cells {} x {}  extra left {} right {} up {} down {}  stack {}",
                    number("Weight"),
                    number("Width"),
                    number("Height"),
                    number("ExtraSizeLeft"),
                    number("ExtraSizeRight"),
                    number("ExtraSizeUp"),
                    number("ExtraSizeDown"),
                    number("StackMaxSize")
                );
                let flags: Vec<&str> = item
                    .props
                    .as_object()
                    .into_iter()
                    .flatten()
                    .filter(|(name, value)| name.starts_with("Blocks") && value.as_bool() == Some(true))
                    .map(|(name, _)| name.as_str())
                    .collect();
                if !flags.is_empty() {
                    println!("  {}", flags.join(" "));
                }
                let filters = |holder: &serde_json::Value| {
                    let lists = holder.pointer("/_props/filters").and_then(|f| f.as_array()).cloned().unwrap_or_default();
                    let ids = |list: &serde_json::Value, name: &str| {
                        let ids: Vec<String> =
                            list.get(name).and_then(|f| f.as_array()).into_iter().flatten().filter_map(|id| id.as_str()).map(|id| format!("{id} {}", name_of(id))).collect();
                        ids
                    };
                    let (mut takes, mut refuses) = (Vec::new(), Vec::new());
                    for list in &lists {
                        takes.extend(ids(list, "Filter"));
                        refuses.extend(ids(list, "ExcludedFilter"));
                    }
                    (takes, refuses)
                };
                for slot in item.props.get("Slots").and_then(|s| s.as_array()).into_iter().flatten() {
                    let (takes, _) = filters(slot);
                    let shown: Vec<&String> = takes.iter().take(4).collect();
                    println!(
                        "  slot {}{}: {} it takes{}{}",
                        slot.get("_name").and_then(|n| n.as_str()).unwrap_or("?"),
                        if slot.get("_required").and_then(|r| r.as_bool()) == Some(true) { " (required)" } else { "" },
                        takes.len(),
                        shown.iter().map(|s| format!("; {s}")).collect::<String>(),
                        if takes.len() > shown.len() { "; ..." } else { "" }
                    );
                }
                for grid in item.props.get("Grids").and_then(|g| g.as_array()).into_iter().flatten() {
                    let (takes, refuses) = filters(grid);
                    let cells = |name: &str| grid.pointer(&format!("/_props/{name}")).and_then(|v| v.as_i64()).unwrap_or(0);
                    println!(
                        "  grid {}: {} x {}; takes {}; refuses {}",
                        grid.get("_name").and_then(|n| n.as_str()).unwrap_or("?"),
                        cells("cellsH"),
                        cells("cellsV"),
                        if takes.is_empty() { "anything".to_owned() } else { takes.join(", ") },
                        if refuses.is_empty() { "nothing".to_owned() } else { refuses.join(", ") }
                    );
                }
            }
        }
        // A look into one of the database's files: `probe json
        // templates/profiles.json /Standard/usec/character/Inventory 2`.
        (Some("json"), Some(file)) => {
            let path = assets.install.database_dir().join(file);
            let value: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path)?)?;
            let at = value.pointer(arg(2).unwrap_or("")).ok_or_else(|| anyhow::anyhow!("nothing at {}", arg(2).unwrap_or("")))?;
            let depth: usize = arg(3).map(|d| d.parse()).transpose()?.unwrap_or(1);
            print_json(at, depth, 0);
        }
        (Some("find"), Some(text)) => {
            let mut keys: Vec<&String> =
                assets.manifest.entries.keys().filter(|k| k.contains(text)).collect();
            keys.sort();
            for key in keys {
                println!("{key}");
            }
        }
        (Some("info"), Some(key)) => {
            println!("dependencies: {:?}", assets.manifest.dependencies(key));
            let own = assets.load(key)?;
            for (index, file) in assets.files().iter().enumerate() {
                let mark = if own.contains(&index) { "*" } else { " " };
                let mut counts: BTreeMap<i32, usize> = BTreeMap::new();
                for o in &file.meta.objects {
                    *counts.entry(o.class_id).or_default() += 1;
                }
                let counts: Vec<String> = counts
                    .iter()
                    .map(|(id, n)| match class::name(*id) {
                        "" => format!("#{id}={n}"),
                        name => format!("{name}={n}"),
                    })
                    .collect();
                println!(
                    "{mark}[{index}] {} <- {}\n    v{} {} typetrees={} externals={:?}\n    {}",
                    file.name,
                    file.bundle_key,
                    file.meta.version,
                    file.meta.unity_version,
                    file.meta.has_type_trees,
                    file.meta.externals,
                    counts.join(" ")
                );
            }
        }
        (Some("names"), Some(key)) => {
            let filter: Option<i32> = arg(2).map(|c| c.parse()).transpose()?;
            for file in assets.load(key)? {
                let objects: Vec<_> = assets.file(file).meta.objects.clone();
                for o in objects {
                    if filter.is_some_and(|c| c != o.class_id) {
                        continue;
                    }
                    let obj = ObjRef { file, path_id: o.path_id };
                    let name = match assets.read(obj) {
                        Ok(v) => v.get("m_Name").and_then(|n| n.as_str()).unwrap_or("").to_owned(),
                        Err(e) => format!("<unreadable: {e}>"),
                    };
                    println!("{:>20} {:<22} {:>9}  {name}", o.path_id, class_label(o.class_id), o.size);
                }
            }
        }
        (Some("dump"), Some(key)) => {
            let path_id: i64 = arg(2).ok_or_else(|| anyhow::anyhow!("path id needed"))?.parse()?;
            let depth = arg(3).map_or(Ok(6), str::parse)?;
            let items = arg(4).map_or(Ok(12), str::parse)?;
            for file in assets.load(key)? {
                let obj = ObjRef { file, path_id };
                if assets.info(obj).is_some() {
                    println!("{}", assets.read(obj)?.pretty(depth, items));
                }
            }
        }
        (Some("script"), Some(key)) => {
            // Every script of a bundle of the class named, as stored, with
            // the object each is on.
            let wanted = arg(2).ok_or_else(|| anyhow::anyhow!("a script's class name needed"))?;
            let depth = arg(3).map_or(Ok(6), str::parse)?;
            let items = arg(4).map_or(Ok(12), str::parse)?;
            for file in assets.load(key)? {
                for obj in assets.objects_of(file, class::MONO_BEHAVIOUR).collect::<Vec<_>>() {
                    if trp_data::unity::extract::script_class(&assets, obj).as_deref() != Some(wanted) {
                        continue;
                    }
                    let value = assets.read(obj)?;
                    let owner = value
                        .get("m_GameObject")
                        .and_then(|v| v.as_pptr())
                        .and_then(|p| assets.resolve_loaded(obj.file, p))
                        .and_then(|owner| assets.read(owner).ok())
                        .and_then(|owner| owner.get("m_Name").and_then(|n| n.as_str()).map(str::to_owned))
                        .unwrap_or_default();
                    println!("{wanted} {} on {owner}\n{}", obj.path_id, value.pretty(depth, items));
                }
            }
        }
        (Some("preset"), Some(weapon)) => {
            let db = trp_data::Database::load(&assets.install)?;
            let preset = db.default_preset(weapon)?;
            println!("{} ({})", preset.name, preset.id);
            for part in &preset.items {
                let item = db.item(&part.template)?;
                println!(
                    "  {:<16} {:<44} {}",
                    part.slot.as_deref().unwrap_or("-"),
                    item.name,
                    item.prefab_key().unwrap_or("-")
                );
            }
        }
        (Some("weapon"), Some(template)) => {
            use trp_data::unity::extract::{read_material, read_mesh, read_texture};
            let db = trp_data::Database::load(&assets.install)?;
            let weapon = trp_data::weapon::assemble_named(&mut assets, &db, template)?;
            println!("{} with {} nodes", weapon.name, weapon.hierarchy.nodes.len());
            for s in &weapon.skipped {
                println!("  SKIPPED {s}");
            }
            // With a third word: every bone whose path has it, at rest.
            if let Some(filter) = arg(2) {
                for (i, node) in weapon.hierarchy.nodes.iter().enumerate() {
                    let path = weapon.hierarchy.path(i);
                    if path.contains(filter) {
                        println!("  [{i}] {path}  at {:?} rot {:?}", node.translation, node.rotation);
                    }
                }
                return Ok(());
            }
            for (i, node) in weapon.hierarchy.nodes.iter().enumerate() {
                if let Some(grip) = &node.grip {
                    println!(
                        "  [{i}] {}  grip for the {} hand, kind {}, {} finger rotations, at {:?} rot {:?}",
                        weapon.hierarchy.path(i),
                        if grip.left_hand { "left" } else { "right" },
                        grip.kind,
                        grip.fingers.len(),
                        node.translation,
                        node.rotation
                    );
                }
                let Some(r) = &node.renderer else { continue };
                let mesh = read_mesh(&assets, r.mesh)?;
                let tris: usize = mesh.submeshes.iter().map(|s| s.len() / 3).sum();
                println!(
                    "  [{i}] {}  mesh={} verts={} tris={} skinned={}",
                    weapon.hierarchy.path(i),
                    mesh.name,
                    mesh.positions.len(),
                    tris,
                    r.skinned
                );
                for m in r.materials.iter().flatten() {
                    let mat = read_material(&assets, *m)?;
                    let mut textures: Vec<String> = Vec::new();
                    for (slot, tex) in &mat.textures {
                        let t = read_texture(&assets, *tex)?;
                        textures.push(format!("{slot}={} {}x{} fmt{} mips{}", t.name, t.width, t.height, t.format, t.mip_count));
                    }
                    textures.sort();
                    println!("        material {}: {}", mat.name, textures.join("; "));
                }
            }
        }
        // A model of S.T.A.L.K.E.R.'s (`docs/spec/outfits.md`), by its
        // file or by its path under that game's `meshes`: its meshes with
        // their pictures, and its bones at rest, each beside the middle of
        // the vertices that go mostly with it.
        (Some("ogf"), Some(wanted)) => {
            let stalker = trp_data::xray::Stalker::locate();
            let (bytes, file) = match std::path::Path::new(wanted).is_file() {
                true => (std::fs::read(wanted)?, wanted.to_owned()),
                false => stalker.read(&format!("meshes/{wanted}"))?,
            };
            let model = trp_data::xray::read(&bytes)?;
            println!("{file}: {} bones, {} meshes", model.bones.len(), model.parts.len());
            for part in &model.parts {
                let told = match stalker.read(&format!("textures/{}.dds", part.texture)) {
                    Ok((bytes, file)) => match trp_data::xray::read_picture(&bytes) {
                        Ok(p) => format!("{} x {}, {} levels, {:?}, {file}", p.width, p.height, p.levels, p.packing),
                        Err(e) => format!("not read ({e:#}), {file}"),
                    },
                    Err(e) => format!("{e:#}"),
                };
                let (mut low, mut high) = ([f32::MAX; 3], [f32::MIN; 3]);
                for p in &part.positions {
                    for axis in 0..3 {
                        (low[axis], high[axis]) = (low[axis].min(p[axis]), high[axis].max(p[axis]));
                    }
                }
                println!("  mesh: {} vertices, {} triangles, from {:.3} {:.3} {:.3} to {:.3} {:.3} {:.3}; picture {}: {told}", part.positions.len(), part.triangles.len() / 3, low[0], low[1], low[2], high[0], high[1], high[2], part.texture);
            }
            for (index, bone) in model.bones.iter().enumerate() {
                // The bone's own line: to the first bone that hangs from
                // it, or else on from the one it hangs from. Its vertices
                // are measured along that, from its joint, and across.
                let next = model.bones.iter().find(|other| other.parent == Some(index)).map(|child| child.at);
                let line = next.map(|at| [at[0] - bone.at[0], at[1] - bone.at[1], at[2] - bone.at[2]]).or_else(|| bone.parent.map(|parent| {
                    let from = model.bones[parent].at;
                    [bone.at[0] - from[0], bone.at[1] - from[1], bone.at[2] - from[2]]
                }));
                let long = line.map_or(0.0, |v| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt());
                let mut sum = [0.0f32; 3];
                let mut count = 0;
                let (mut back, mut ahead, mut wide) = (f32::MAX, f32::MIN, 0.0f32);
                for part in &model.parts {
                    for (p, links) in part.positions.iter().zip(&part.links) {
                        if links.iter().any(|(linked, share)| *linked as usize == index && *share > 0.5) {
                            sum = [sum[0] + p[0], sum[1] + p[1], sum[2] + p[2]];
                            count += 1;
                            if let Some(line) = line.filter(|_| long > 1e-5) {
                                let off = [p[0] - bone.at[0], p[1] - bone.at[1], p[2] - bone.at[2]];
                                let along = (off[0] * line[0] + off[1] * line[1] + off[2] * line[2]) / long;
                                let across = (off[0] * off[0] + off[1] * off[1] + off[2] * off[2] - along * along).max(0.0).sqrt();
                                (back, ahead, wide) = (back.min(along), ahead.max(along), wide.max(across));
                            }
                        }
                    }
                }
                let measured = if count > 0 && long > 1e-5 { format!(", from {:.1} to {:.1} cm along its {:.1} cm, {:.1} cm across", back * 100.0, ahead * 100.0, long * 100.0, wide * 100.0) } else { String::new() };
                let middle = if count > 0 { format!("{count:4} vertices about {:7.3} {:7.3} {:7.3}{measured}", sum[0] / count as f32, sum[1] / count as f32, sum[2] / count as f32) } else { "no vertices".to_owned() };
                let parent = bone.parent.map_or("-", |parent| model.bones[parent].name.as_str());
                println!("  {index:2} {:<20} < {:<18} at {:7.3} {:7.3} {:7.3}   {middle}", bone.name, parent, bone.at[0], bone.at[1], bone.at[2]);
            }
        }
        // The other game's packed archives (`trp_data::xray_archive`): each
        // with its count of files and how long its table took, and the
        // names that have the word; with `get`, also each such file read,
        // unpacked and checked.
        (Some("xdb"), Some(part)) => {
            let stalker = trp_data::xray::Stalker::locate();
            let began = std::time::Instant::now();
            let archives = stalker.archives();
            println!("{} archives, their tables read in {:.2} s", archives.len(), began.elapsed().as_secs_f32());
            let get = args.get(2).is_some_and(|word| word == "get");
            for archive in archives {
                let names = archive.names_with(part);
                println!("{}: {} files, {} with {part:?}", archive.path.display(), archive.len(), names.len());
                for name in names.iter().take(40) {
                    if get {
                        let began = std::time::Instant::now();
                        match archive.read(name) {
                            Ok(Some(bytes)) => println!("  {name}: {} bytes, checked, in {:.1} ms", bytes.len(), began.elapsed().as_secs_f32() * 1000.0),
                            Ok(None) => println!("  {name}: not there"),
                            Err(e) => println!("  {name}: {e:#}"),
                        }
                    } else {
                        println!("  {name}");
                    }
                }
            }
        }
        // A piece of the character's clothing (a body's, a legs', a head's
        // or the arms' bundle): the bones its mesh hangs on, each where it
        // is at rest in the mesh's space.
        (Some("skin"), Some(key)) => {
            let part = trp_data::arms::load(&mut assets, key)?;
            let mesh = trp_data::unity::extract::read_mesh(&assets, part.mesh)?;
            println!("{key}: {} vertices, {} bones", mesh.positions.len(), part.bone_paths.len());
            for (path, bind) in part.bone_paths.iter().zip(&mesh.bind_poses) {
                match trp_data::outfit::rest_place(bind) {
                    Some(at) => println!("  {:7.3} {:7.3} {:7.3}  {path}", at[0], at[1], at[2]),
                    None => println!("  (no place)  {path}"),
                }
            }
        }
        (Some("weapons"), filter) => {
            // Every weapon of the database that has a stock preset: its
            // class, how many states and clips its animator has, and two
            // marks by which those with the same states, and with the same
            // clips, are told (a count of the install's distinct
            // animators, for a library of them).
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};
            const CLASSES: [&str; 12] = [
                "AssaultRifle", "AssaultCarbine", "Smg", "MarksmanRifle", "MachineGun", "SniperRifle", "Shotgun", "Pistol", "Revolver",
                "GrenadeLauncher", "SpecialWeapon", "RocketLauncher",
            ];
            let db = trp_data::Database::load(&assets.install)?;
            let mut items: Vec<_> = db.items.values().filter(|item| item.name.starts_with("weapon_") && db.is_a(item, "Weapon")).collect();
            items.sort_by(|a, b| a.name.cmp(&b.name));
            let mark = |words: &[String]| {
                let mut hasher = DefaultHasher::new();
                words.hash(&mut hasher);
                hasher.finish() & 0xFFFF_FFFF
            };
            for item in items {
                if filter.is_some_and(|f| !item.name.contains(f)) {
                    continue;
                }
                let class = CLASSES.iter().find(|class| db.is_a(item, class)).copied().unwrap_or("-");
                let read = (|| -> anyhow::Result<_> {
                    let weapon = trp_data::weapon::assemble_named(&mut assets, &db, &item.id)?;
                    let table = trp_data::states::StateTable::load(&assets, &weapon)?;
                    let mut states: Vec<String> = table.states.iter().map(|state| state.full_name.clone()).collect();
                    states.sort();
                    let mut clips: Vec<String> = table.states.iter().flat_map(|state| state.clips.iter().map(|clip| clip.0.clone())).collect();
                    clips.sort();
                    clips.dedup();
                    let layers: Vec<String> = table.layers.iter().map(|layer| layer.name.clone()).collect();
                    Ok((states.len(), clips.len(), mark(&states), mark(&clips), layers.join("+")))
                })();
                match read {
                    Ok((states, clips, states_mark, clips_mark, layers)) => {
                        println!("{class:<16} {:<52} {} states {states:>3} clips {clips:>3} same-states {states_mark:08x} same-clips {clips_mark:08x} layers {layers}", item.name, item.id)
                    }
                    Err(e) => println!("{class:<16} {:<52} {} not read: {e:#}", item.name, item.id),
                }
            }
        }
        (Some("states"), Some(template)) => {
            // Every animator state of a weapon with its clip and events.
            let db = trp_data::Database::load(&assets.install)?;
            let weapon = trp_data::weapon::assemble_named(&mut assets, &db, template)?;
            let table = trp_data::states::StateTable::load(&assets, &weapon)?;
            let filter = arg(2).map(str::to_lowercase);
            // `states <weapon> parameters`: the animator's parameters,
            // each with its kind and what it starts at.
            if filter.as_deref() == Some("parameters") {
                for p in &table.parameters {
                    println!("parameter {:?} {} = {}", p.kind, p.name, p.default);
                }
                return Ok(());
            }
            for layer in &table.layers {
                let on = layer.mask.iter().filter(|m| m.1 > 0.0).count();
                println!(
                    "layer {} (machine {}): weight {}, {}, moves {} of {} masked bones, starts in {}",
                    layer.name,
                    layer.machine,
                    layer.default_weight,
                    if layer.additive { "additive" } else { "override" },
                    on,
                    layer.mask.len(),
                    layer.default_state.as_deref().unwrap_or("-")
                );
            }
            for state in &table.states {
                if filter.as_ref().is_some_and(|f| !state.full_name.to_lowercase().contains(f)) {
                    continue;
                }
                let clip = state
                    .clips
                    .iter()
                    .map(|(name, obj)| {
                        format!("{name} {:.3} s", trp_data::states::clip_length(&assets, *obj).unwrap_or(-1.0))
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                println!(
                    "[{}] {}  speed {}{}  {clip}{}",
                    state.machine,
                    state.full_name,
                    state.speed,
                    if state.looping { " loop" } else { "" },
                    match &state.blend_parameter {
                        Some(p) => format!("  by {p} at {:?}", state.thresholds),
                        None => String::new(),
                    }
                );
                for e in &state.events {
                    let conditions: Vec<String> = e
                        .conditions
                        .iter()
                        .map(|c| {
                            format!("{} kind{} mode{} {}/{}/{}", c.parameter, c.kind, c.mode, c.float_value, c.int_value, c.bool_value)
                        })
                        .collect();
                    println!(
                        "      {:.3} {}{}{}{}",
                        e.time,
                        e.name,
                        if e.string_param.is_empty() { String::new() } else { format!(" \"{}\"", e.string_param) },
                        if e.int_param != 0 { format!(" {}", e.int_param) } else { String::new() },
                        if conditions.is_empty() { String::new() } else { format!("   if {}", conditions.join(" & ")) }
                    );
                }
                for t in &state.transitions {
                    println!("      -> {}", describe_transition(t));
                }
            }
            for (machine, t) in &table.any_state {
                if filter.as_ref().is_some_and(|f| !t.to.as_deref().unwrap_or("").to_lowercase().contains(f)) {
                    continue;
                }
                println!("[{machine}] any state -> {}", describe_transition(t));
            }
        }
        (Some("controller"), Some(template)) => {
            // The weapon's animator controller, as stored: states, the
            // table that names them, and the scripts attached to states.
            let db = trp_data::Database::load(&assets.install)?;
            let weapon = trp_data::weapon::assemble_named(&mut assets, &db, template)?;
            let depth = arg(2).map_or(Ok(8), str::parse)?;
            let items = arg(3).map_or(Ok(40), str::parse)?;
            let mut controller = weapon
                .prefab
                .req("_originalAnimatorController")?
                .as_pptr()
                .and_then(|p| assets.resolve_loaded(weapon.prefab_file, p))
                .ok_or_else(|| anyhow::anyhow!("no animator controller"))?;
            let mut v = assets.read(controller)?;
            while let Some(base) =
                v.get("m_Controller").and_then(|p| p.as_pptr()).and_then(|p| assets.resolve_loaded(controller.file, p))
            {
                println!("override of:");
                controller = base;
                v = assets.read(controller)?;
            }
            println!("controller {} in file {}", assets.name_of(controller).unwrap_or_default(), controller.file);
            println!("{}", v.pretty(depth, items));
            let mut events_shown = false;
            for b in v.get("m_StateMachineBehaviours").and_then(|b| b.as_array()).unwrap_or(&[]) {
                let Some(obj) = b.as_pptr().and_then(|p| assets.resolve_loaded(controller.file, p)) else { continue };
                let script = trp_data::unity::extract::script_class(&assets, obj).unwrap_or_default();
                let behaviour = assets.read(obj)?;
                println!("behaviour {} [{script}]\n{}", obj.path_id, behaviour.pretty(4, 6));
                if events_shown {
                    continue;
                }
                if let Some(data) =
                    behaviour.get("EventsData").and_then(|p| p.as_pptr()).and_then(|p| assets.resolve_loaded(obj.file, p))
                {
                    println!("events data:\n{}", assets.read(data)?.pretty(depth, items));
                    events_shown = true;
                }
            }
        }
        (Some("engine"), _) => {
            println!("{:?}", trp_data::engine::EngineSettings::load(&assets.install)?);
            let hands = trp_data::player::HandsData::load(&mut assets)?;
            println!(
                "walk: step frequency {}, intensity {:?}, {} entries in the normal preset",
                hands.walk_step_frequency,
                hands.walk_intensity_range,
                hands.walk_normal.len()
            );
            for entry in &hands.walk_normal {
                let span = entry.curve.keys.last().map_or(0.0, |k| k.time);
                println!("  curve over 0..{span} shift {} vals {:?}", entry.time_shift, entry.vals);
            }
            println!("mouse: {:?}", hands.mouse.vals);
            println!("movement: {:?}", hands.movement.vals);
            println!("camera offset {:?}, sway {} / {}", hands.camera_offset, hands.sway_frequency, hands.sway_damping);
            let db = trp_data::Database::load(&assets.install)?;
            let weapon = trp_data::weapon::assemble_named(&mut assets, &db, arg(1).unwrap_or("59d6088586f774275f37482f"))?;
            println!(
                "{}: {:.3} kg, ergonomics {}, {} cells wide",
                weapon.name, weapon.total_weight, weapon.total_ergonomics, weapon.cell_width
            );
        }
        (Some("hardsettings"), filter) => {
            let settings = trp_data::hard_settings::HardSettings::load(&assets.install)?;
            let wanted = |name: &str| filter.map_or(true, |f| name.to_lowercase().contains(&f.to_lowercase()));
            for (name, value) in settings.numbers() {
                if wanted(name) {
                    println!("{name} = {value}");
                }
            }
            for (name, curve) in settings.curves() {
                if wanted(name) {
                    let keys: Vec<String> = curve
                        .keys
                        .iter()
                        .map(|k| format!("({:.3}, {:.3} | {:.2} {:.2})", k.time, k.value, k.in_slope, k.out_slope))
                        .collect();
                    println!("{name}: {}", keys.join(" "));
                }
            }
            // The lists of the game's own numbers (movement states).
            for name in ["NO_AIM_STATES", "UnsuitableStates"] {
                if wanted(name) {
                    println!("{name}: {:?}", settings.bytes(name)?);
                }
            }
        }
        (Some("rawfile"), Some(rel)) => {
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
        }
        (Some("rootmotion"), filter) => {
            use trp_data::rootmotion::{Node, RootMotionTable};
            fn show(node: &Node, depth: usize, max_depth: usize) {
                let pad = "  ".repeat(depth + 1);
                match node {
                    Node::Clip { name, looping, length, velocity, uses_curves, .. } => println!(
                        "{pad}clip {name}  {length:.2}s loop={looping} v=({:.3} {:.3} {:.3}){}",
                        velocity[0],
                        velocity[1],
                        velocity[2],
                        if *uses_curves { " curves" } else { "" }
                    ),
                    Node::Tree { name, blend, param_x, param_y, thresholds, positions, children, steps_x, steps_y, .. } => {
                        let layout = if *blend == trp_data::rootmotion::BlendType::Simple1D {
                            format!("thresholds={thresholds:?}")
                        } else {
                            format!("positions={positions:?}")
                        };
                        println!(
                            "{pad}tree {name}  {blend:?} x={param_x:#010x} y={param_y:#010x} {layout} grid={steps_x}x{steps_y} children={}",
                            children.len()
                        );
                        if depth < max_depth {
                            for child in children {
                                show(child, depth + 1, max_depth);
                            }
                        }
                    }
                }
            }
            let table = RootMotionTable::load(&mut assets)?;
            for p in &table.parameters {
                println!("param {:<18} {:#010x}  {}..{} step {}", p.name, trp_data::unity::anim::path_hash(&p.name), p.min, p.max, p.step);
            }
            let depth: usize = arg(2).map_or(Ok(0), str::parse)?;
            let mut states: Vec<_> = table.states.iter().collect();
            states.sort_by(|a, b| a.1 .0.cmp(&b.1 .0));
            println!("{} states", states.len());
            for (hash, (name, node)) in states {
                if filter.is_some_and(|f| !name.to_lowercase().contains(&f.to_lowercase())) {
                    continue;
                }
                println!("state {name}  {hash:#010x}");
                show(node, 0, depth);
            }
        }
        (Some("clips"), Some(template)) => {
            use trp_data::unity::anim::{path_hash, Clip};
            let db = trp_data::Database::load(&assets.install)?;
            let weapon = trp_data::weapon::assemble_named(&mut assets, &db, template)?;
            let paths: std::collections::HashMap<u32, String> = (0..weapon.hierarchy.nodes.len())
                .map(|i| weapon.hierarchy.path(i))
                .map(|p| (path_hash(&p), p))
                .collect();
            let filter = arg(2).unwrap_or("");
            for (name, obj) in weapon.clips(&assets)? {
                if !name.contains(filter) {
                    continue;
                }
                let clip = Clip::read(&assets, obj)?;
                let bound = clip.tracks.iter().filter(|t| paths.contains_key(&t.path)).count();
                println!(
                    "{name:<34} -> {:<44} {:>6.2}s loop={} tracks={} on-rig={} from {}",
                    clip.name,
                    clip.duration(),
                    clip.looping,
                    clip.tracks.len(),
                    bound,
                    assets.file(obj.file).bundle_key.rsplit('/').next().unwrap_or("")
                );
                if arg(3) == Some("check") {
                    // Compares the clip's first frame with the prefab's rest pose.
                    use trp_data::unity::anim::{euler_to_quat, Property};
                    let nodes: std::collections::HashMap<u32, usize> = (0..weapon.hierarchy.nodes.len())
                        .map(|i| (path_hash(&weapon.hierarchy.path(i)), i))
                        .collect();
                    let mut values = Vec::new();
                    clip.sample(0.0, &mut values);
                    let (mut count, mut close, mut worst, mut custom) = (0, 0, (0.0f32, String::new()), 0);
                    for t in clip.tracks.iter().filter(|t| t.property == Property::Euler) {
                        custom += (t.custom_type != 0) as usize;
                        let Some(&node) = nodes.get(&t.path) else { continue };
                        let q = euler_to_quat([values[t.curve], values[t.curve + 1], values[t.curve + 2]]);
                        let rest = weapon.hierarchy.nodes[node].rotation;
                        let dot: f32 = q.iter().zip(&rest).map(|(a, b)| a * b).sum();
                        let angle = 2.0 * dot.abs().min(1.0).acos().to_degrees();
                        count += 1;
                        close += (angle < 1.0) as usize;
                        if angle > worst.0 {
                            worst = (angle, weapon.hierarchy.nodes[node].name.clone());
                        }
                    }
                    println!(
                        "    euler tracks on rig: {count}, within 1 degree of rest: {close}, largest difference {:.2} degrees at {}, non-zero customType: {custom}",
                        worst.0, worst.1
                    );
                }
                // `tracks [word] [time]`: the clip's tracks whose bone path
                // ends in the word, at that time (seconds; the start if
                // none is given).
                if !filter.is_empty() && arg(3) == Some("tracks") {
                    let word = arg(4).unwrap_or("");
                    let time = arg(5).and_then(|t| t.parse::<f32>().ok()).unwrap_or(0.0);
                    let mut values = Vec::new();
                    clip.sample(time, &mut values);
                    for t in &clip.tracks {
                        let path = paths.get(&t.path).map_or("<not on rig>", String::as_str);
                        if !path.ends_with(word) {
                            continue;
                        }
                        let n = if t.property == trp_data::unity::anim::Property::Rotation { 4 } else { 3 };
                        println!("    {:?} {path} at {time} s = {:?}", t.property, &values[t.curve..t.curve + n]);
                    }
                }
                // `path [word] [from] [to] [step]`: where the first bone
                // whose path ends in the word is through the clip, in the
                // rig's own frame: every bone above it as the clip has it
                // (as the prefab has it where the clip says nothing), one
                // line a time, with how far it has turned from the line
                // before and its speed. A break in the speed at a steady
                // period is the clip's own: curves kept as samples run in
                // straight lines from one to the next.
                if !filter.is_empty() && arg(3) == Some("path") {
                    use trp_data::unity::anim::{euler_to_quat, Property};
                    let word = arg(4).unwrap_or("");
                    let figure = |index: usize, otherwise: f32| arg(index).and_then(|t| t.parse::<f32>().ok()).unwrap_or(otherwise);
                    let (from, to, step) = (figure(5, 0.0), figure(6, clip.duration()), figure(7, 1.0 / 120.0).max(1e-4));
                    let Some(bone) = (0..weapon.hierarchy.nodes.len()).find(|&i| weapon.hierarchy.path(i).ends_with(word)) else {
                        println!("    no bone's path ends in {word}");
                        continue;
                    };
                    // The bone and everything above it, the bone first.
                    let mut chain = vec![bone];
                    while let Some(parent) = weapon.hierarchy.nodes[*chain.last().unwrap()].parent {
                        chain.push(parent);
                    }
                    let tracks_of = |node: usize| {
                        let hash = path_hash(&weapon.hierarchy.path(node));
                        let find = |wanted: &[Property]| clip.tracks.iter().find(|t| t.path == hash && wanted.contains(&t.property)).copied();
                        (find(&[Property::Position]), find(&[Property::Rotation, Property::Euler]))
                    };
                    let tracks: Vec<_> = chain.iter().map(|&node| tracks_of(node)).collect();
                    let (cubics, samples, rate, _) = clip.kept();
                    for (&node, (place, turn)) in chain.iter().zip(&tracks) {
                        let kept = |curve: Option<usize>| match curve {
                            None => "the prefab's",
                            Some(c) if c < cubics => "cubics",
                            Some(c) if c < cubics + samples => "samples",
                            Some(_) => "one figure",
                        };
                        println!("    {}: place {}, turn {}", weapon.hierarchy.path(node), kept(place.map(|t| t.curve)), kept(turn.map(|t| t.curve)));
                    }
                    println!("    samples are {rate} a second");
                    let mul = |a: [f32; 4], b: [f32; 4]| -> [f32; 4] {
                        [
                            a[3] * b[0] + a[0] * b[3] + a[1] * b[2] - a[2] * b[1],
                            a[3] * b[1] - a[0] * b[2] + a[1] * b[3] + a[2] * b[0],
                            a[3] * b[2] + a[0] * b[1] - a[1] * b[0] + a[2] * b[3],
                            a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2],
                        ]
                    };
                    let turned = |q: [f32; 4], v: [f32; 3]| -> [f32; 3] {
                        let p = mul(mul(q, [v[0], v[1], v[2], 0.0]), [-q[0], -q[1], -q[2], q[3]]);
                        [p[0], p[1], p[2]]
                    };
                    let mut values = Vec::new();
                    let mut last: Option<([f32; 3], [f32; 4])> = None;
                    let mut time = from;
                    while time <= to + 1e-6 {
                        clip.sample(time, &mut values);
                        // From the bone up: each bone's place and turn in
                        // its parent takes what is below it along.
                        let (mut at, mut facing) = ([0.0f32; 3], [0.0f32, 0.0, 0.0, 1.0]);
                        for (&node, (place, turn)) in chain.iter().zip(&tracks) {
                            let rest = &weapon.hierarchy.nodes[node];
                            let p = place.map_or(rest.translation, |t| [values[t.curve], values[t.curve + 1], values[t.curve + 2]]);
                            let q = match turn {
                                Some(t) if t.property == Property::Euler => euler_to_quat([values[t.curve], values[t.curve + 1], values[t.curve + 2]]),
                                Some(t) => [values[t.curve], values[t.curve + 1], values[t.curve + 2], values[t.curve + 3]],
                                None => rest.rotation,
                            };
                            let moved = turned(q, at);
                            at = [moved[0] + p[0], moved[1] + p[1], moved[2] + p[2]];
                            facing = mul(q, facing);
                        }
                        let mut line = format!("    {time:.4} sample {:6.2}  at {:8.2} {:8.2} {:8.2} mm", time * rate, at[0] * 1000.0, at[1] * 1000.0, at[2] * 1000.0);
                        if let Some((was, faced)) = last {
                            let dot: f32 = (0..4).map(|k| faced[k] * facing[k]).sum();
                            let angle = 2.0 * dot.abs().min(1.0).acos().to_degrees();
                            line.push_str(&format!(
                                "  speed {:8.1} {:8.1} {:8.1} mm/s  turning {:7.1} deg/s",
                                (at[0] - was[0]) / step * 1000.0,
                                (at[1] - was[1]) / step * 1000.0,
                                (at[2] - was[2]) / step * 1000.0,
                                angle / step
                            ));
                        }
                        println!("{line}");
                        last = Some((at, facing));
                        time += step;
                    }
                }
                // `series [word] [from] [to] [step]`: the same tracks at
                // each of a run of times, one line a time, and how each is
                // kept (as cubics from key to key, as samples at a steady
                // rate with straight lines between, or as one figure).
                if !filter.is_empty() && arg(3) == Some("series") {
                    let word = arg(4).unwrap_or("");
                    let figure = |index: usize, otherwise: f32| arg(index).and_then(|t| t.parse::<f32>().ok()).unwrap_or(otherwise);
                    let (from, to, step) = (figure(5, 0.0), figure(6, clip.duration()), figure(7, 1.0 / 120.0).max(1e-4));
                    let chosen: Vec<_> = clip.tracks.iter().filter(|t| paths.get(&t.path).is_some_and(|p| p.ends_with(word))).collect();
                    let (cubics, samples, rate, figures) = clip.kept();
                    println!("    {cubics} curves as cubics, {samples} as samples at {rate} a second, {figures} that never change");
                    for t in &chosen {
                        let kept = if t.curve < cubics {
                            "cubics"
                        } else if t.curve < cubics + samples {
                            "samples"
                        } else {
                            "one figure"
                        };
                        println!("    {:?} {} ({kept}, curve {})", t.property, paths.get(&t.path).map_or("", String::as_str), t.curve);
                    }
                    let mut values = Vec::new();
                    let mut time = from;
                    while time <= to + 1e-6 {
                        clip.sample(time, &mut values);
                        let mut line = format!("    {time:.4}");
                        for t in &chosen {
                            let n = if t.property == trp_data::unity::anim::Property::Rotation { 4 } else { 3 };
                            for value in &values[t.curve..t.curve + n] {
                                line.push_str(&format!(" {value:.5}"));
                            }
                            line.push_str(" |");
                        }
                        println!("{line}");
                        time += step;
                    }
                }
            }
        }
        (Some("blendcheck"), filter) => {
            // Freeform directional trees of the root motion table: where the
            // stored weights and the computed ones part.
            use trp_data::rootmotion::{freeform_directional, BlendType, Node, RootMotionTable};
            let table = RootMotionTable::load(&mut assets)?;
            fn visit(table: &RootMotionTable, state: &str, node: &Node, limit: usize) {
                let Node::Tree { blend, param_x, param_y, positions, steps_x, steps_y, weights, children, .. } = node else {
                    return;
                };
                if *blend == BlendType::FreeformDirectional2D && positions.len() == children.len() {
                    let range = |hash: u32| table.parameters.iter().find(|p| trp_data::unity::anim::path_hash(&p.name) == hash);
                    let (Some(px), Some(py)) = (range(*param_x), range(*param_y)) else { return };
                    let n = children.len();
                    println!("{state}: {} by {} over {:?}, grid {steps_x}x{steps_y}", px.name, py.name, positions);
                    let mut shown = 0;
                    for x in 0..*steps_x {
                        for y in 0..*steps_y {
                            let at = [px.min + x as f32 * px.step, py.min + y as f32 * py.step];
                            let mine = freeform_directional(positions, at);
                            let stored = &weights[(x * steps_y + y) * n..(x * steps_y + y + 1) * n];
                            let gap = mine.iter().zip(stored).map(|(a, b)| (a - b).abs()).fold(0.0f32, f32::max);
                            if gap > 0.01 && shown < limit {
                                shown += 1;
                                println!("   at {at:?}: stored {stored:.3?}  computed {mine:.3?}");
                            }
                        }
                    }
                }
                for child in children {
                    visit(table, state, child, limit);
                }
            }
            for (name, node) in table.states.values() {
                if filter.is_none_or(|f| name.contains(f)) {
                    visit(&table, name, node, 6);
                }
            }
            // Which weight of direction against distance fits the stored
            // grids best: the largest and the mean difference for each.
            fn scan(table: &RootMotionTable, node: &Node, k: f32, worst: &mut f32, sum: &mut f64, count: &mut u64) {
                let Node::Tree { blend, param_x, param_y, positions, steps_x, steps_y, weights, children, .. } = node else {
                    return;
                };
                if *blend == BlendType::FreeformDirectional2D && positions.len() == children.len() {
                    let range = |hash: u32| table.parameters.iter().find(|p| trp_data::unity::anim::path_hash(&p.name) == hash);
                    if let (Some(px), Some(py)) = (range(*param_x), range(*param_y)) {
                        let n = children.len();
                        for x in 0..*steps_x {
                            for y in 0..*steps_y {
                                let at = [px.min + x as f32 * px.step, py.min + y as f32 * py.step];
                                let mine = trp_data::rootmotion::freeform_directional_weighted(positions, at, k);
                                for c in 0..n {
                                    let gap = (mine[c] - weights[(x * steps_y + y) * n + c]).abs();
                                    *worst = worst.max(gap);
                                    *sum += gap as f64;
                                    *count += 1;
                                }
                            }
                        }
                    }
                }
                for child in children {
                    scan(table, child, k, worst, sum, count);
                }
            }
            for k in [0.3183, 0.6366, 0.9, 0.98, 1.0, 1.01, 1.02, 1.1, 1.5708, 2.0] {
                let (mut worst, mut sum, mut count) = (0.0f32, 0.0f64, 0u64);
                for (_, node) in table.states.values() {
                    scan(&table, node, k, &mut worst, &mut sum, &mut count);
                }
                println!("direction weight {k}: largest difference {worst:.4}, mean {:.6}", sum / count.max(1) as f64);
            }
        }
        (Some("bodystates"), filter) => {
            // States of the body animator with their clips and transitions.
            let files = assets.load("assets/content/characters/controllers/player_anim_controller.bundle")?;
            // The game asks for the controller of this name. `-` as the
            // filter shows every state.
            let wanted = arg(2).unwrap_or("Player_anim_controller");
            let filter = filter.filter(|f| *f != "-").map(str::to_lowercase);
            for &file in &files {
                for obj in assets.objects_of(file, class::ANIMATOR_CONTROLLER).collect::<Vec<_>>() {
                    let name = assets.name_of(obj).unwrap_or_default();
                    println!("controller {name}");
                    if name != wanted {
                        continue;
                    }
                    let table = trp_data::states::StateTable::from_controller(&assets, obj, &Default::default())?;
                    if arg(3) == Some("parameters") {
                        for p in &table.parameters {
                            println!("parameter {:?} {} = {}", p.kind, p.name, p.default);
                        }
                        for (machine, list) in table.selectors.iter().enumerate() {
                            for (index, selector) in list.iter().enumerate() {
                                let choices: Vec<String> = selector
                                    .choices
                                    .iter()
                                    .map(|c| {
                                        format!(
                                            "{}{} ({} conditions)",
                                            c.to.as_deref().unwrap_or(""),
                                            c.to_selector.map_or(String::new(), |s| format!("point {s}")),
                                            c.conditions.len()
                                        )
                                    })
                                    .collect();
                                println!(
                                    "[{machine}] {} point {index}: {}",
                                    if selector.entry { "entry" } else { "exit" },
                                    choices.join(" | ")
                                );
                            }
                        }
                    }
                    for layer in &table.layers {
                        println!(
                            "layer {} (machine {}, slot {}): weight {}, {}, {} masked bones, starts in {}",
                            layer.name,
                            layer.machine,
                            layer.sync_slot,
                            layer.default_weight,
                            if layer.additive { "additive" } else { "override" },
                            layer.mask.len(),
                            layer.default_state.as_deref().unwrap_or("-")
                        );
                    }
                    for state in &table.states {
                        if filter.as_ref().is_some_and(|f| !state.full_name.to_lowercase().contains(f)) {
                            continue;
                        }
                        let clips: Vec<&str> = state.clips.iter().map(|c| c.0.as_str()).collect();
                        println!(
                            "[{}] {}  speed {}{}{}{}  {}{}",
                            state.machine,
                            state.full_name,
                            state.speed,
                            state.speed_parameter.as_ref().map_or(String::new(), |p| format!(" x {p}")),
                            state.movement.as_ref().map_or(String::new(), |m| {
                                format!(
                                    "  <movement {}{}{}{}{}{}{}{}{}>",
                                    m.state,
                                    if m.can_interact { " interacts" } else { "" },
                                    if m.sensitivity != 1.0 || m.rotation_speed_clamp != 99.0 {
                                        format!(" turning x{} up to {}", m.sensitivity, m.rotation_speed_clamp)
                                    } else {
                                        String::new()
                                    },
                                    if m.default_state { " default" } else { "" },
                                    if m.disable_root_motion { " no-root-motion" } else { "" },
                                    match m.kind {
                                        1 => " in",
                                        2 => " out",
                                        _ => "",
                                    },
                                    match m.direction {
                                        0 => String::new(),
                                        1 => " right".to_owned(),
                                        5 => " left".to_owned(),
                                        other => format!(" direction {other}"),
                                    },
                                    if m.length > 0.0 { format!(" length {:.3} s", m.length) } else { String::new() },
                                    if m.unique { " own-object" } else { "" }
                                )
                            }),
                            if state.looping { " loop" } else { "" },
                            clips.join(", "),
                            state.blend_parameter.as_ref().map_or(String::new(), |p| format!("  by {p}"))
                        );
                        if let Some(pose) = state.movement.as_ref().and_then(|m| m.first_person_pose.as_ref()) {
                            let keys: Vec<String> =
                                pose.blend.keys.iter().map(|k| format!("{:.3}: {:.3}", k.time, k.value)).collect();
                            println!(
                                "      first-person pose {}: position {:?}, rotation {:?}, camera rotation {:?}, camera position {:?}, blend {}",
                                pose.name,
                                pose.position,
                                pose.rotation,
                                pose.camera_rotation,
                                pose.camera_position,
                                keys.join(", ")
                            );
                        }
                        // The marks the game's logic waits for, each by when
                        // in the state it comes.
                        for e in &state.events {
                            println!("      {:.3} {}{}", e.time, e.name, if e.enabled { "" } else { " (off)" });
                        }
                        for t in &state.transitions {
                            println!("      -> {}", describe_transition(t));
                        }
                        if arg(3) == Some("tree") {
                            println!("      trees by layer: {:?}", state.tree_of_layer);
                            for (t, i, node) in
                                state.trees.iter().enumerate().flat_map(|(t, tree)| tree.iter().enumerate().map(move |(i, n)| (t, i, n)))
                            {
                                let i = format!("{t}.{i}");
                                match &node.clip {
                                    Some((name, _)) => {
                                        println!("      node {i}: clip {name}  length x {:.3}  offset {}", node.duration, node.cycle_offset)
                                    }
                                    None => println!(
                                        "      node {i}: blend type {} by {} / {}  children {:?}  thresholds {:?}  positions {:?}",
                                        node.blend_type,
                                        node.parameter_x.as_deref().unwrap_or("-"),
                                        node.parameter_y.as_deref().unwrap_or("-"),
                                        node.children,
                                        node.thresholds,
                                        node.positions
                                    ),
                                }
                            }
                        }
                    }
                    for (machine, t) in &table.any_state {
                        if filter.as_ref().is_some_and(|f| !t.to.as_deref().unwrap_or("").to_lowercase().contains(f)) {
                            continue;
                        }
                        println!("[{machine}] any state -> {}", describe_transition(t));
                    }
                }
            }
        }
        (Some("bodyclips"), Some(filter)) => {
            // Clips of the body animator: what they move on the player's
            // skeleton and which animator parameters they carry curves for.
            use trp_data::unity::anim::{euler_to_quat, path_hash, Clip, Property};
            let player = assets.load("assets/content/commonprefabs/playersuperior.bundle")?;
            let mut paths: std::collections::HashMap<u32, String> = std::collections::HashMap::new();
            for &file in &player {
                for (_, root) in trp_data::unity::extract::exported_game_objects(&assets, file)? {
                    let hierarchy = trp_data::unity::extract::read_hierarchy(&assets, root)?;
                    for i in 0..hierarchy.nodes.len() {
                        let path = hierarchy.path(i);
                        // The animator sits on `Player`, so its paths start below that.
                        if let Some(below) = path.strip_prefix("Player/") {
                            paths.insert(path_hash(below), below.to_owned());
                        }
                    }
                }
            }
            const CURVES: [&str; 15] = [
                "Turn",
                "Weapon_3rd",
                "First_Person_Curve_Weight",
                "Position_cache",
                "Hand_Left",
                "Hand_Right",
                "Shoulder_Left",
                "Shoulder_Right",
                "Elbow_Left",
                "Elbow_Right",
                "Leftstance_curve",
                "Mouse_look",
                "Aiming_Layer",
                "FirstAction",
                "FootStep",
            ];
            let curve_names: std::collections::HashMap<u32, &str> = CURVES.iter().map(|n| (path_hash(n), *n)).collect();
            let wanted = arg(3).unwrap_or("");
            let files = assets.load("assets/content/characters/controllers/player_anim_controller.bundle")?;
            let all: Vec<usize> = (0..assets.files().len()).collect();
            let _ = files;
            for file in all {
                for obj in assets.objects_of(file, class::ANIMATION_CLIP).collect::<Vec<_>>() {
                    let Some(name) = assets.name_of(obj) else { continue };
                    if !name.to_lowercase().contains(&filter.to_lowercase()) {
                        continue;
                    }
                    let clip = Clip::read(&assets, obj)?;
                    let on_rig = clip.tracks.iter().filter(|t| paths.contains_key(&t.path)).count();
                    println!(
                        "{name:<44} {:>6.2}s loop={} tracks={} on-skeleton={} floats={} from {}",
                        clip.duration(),
                        clip.looping,
                        clip.tracks.len(),
                        on_rig,
                        clip.floats.len(),
                        assets.file(file).bundle_key.rsplit('/').next().unwrap_or("")
                    );
                    if arg(2) != Some("tracks") {
                        continue;
                    }
                    let mut values = Vec::new();
                    for f in &clip.floats {
                        let samples: Vec<String> = (0..=4)
                            .map(|i| {
                                clip.sample(clip.duration() * i as f32 / 4.0, &mut values);
                                format!("{:.3}", values[f.curve])
                            })
                            .collect();
                        let label = curve_names.get(&f.name).map_or_else(|| format!("#{:08x}", f.name), |n| (*n).to_owned());
                        println!("    curve {label} (custom {}) = {}", f.custom_type, samples.join(" "));
                    }
                    clip.sample(0.0, &mut values);
                    for t in &clip.tracks {
                        let Some(path) = paths.get(&t.path) else { continue };
                        if !path.contains(wanted) {
                            continue;
                        }
                        let v = &values[t.curve..];
                        match t.property {
                            Property::Euler => println!("    {path}: euler {:?} = quat {:?}", &v[..3], euler_to_quat([v[0], v[1], v[2]])),
                            Property::Rotation => println!("    {path}: rotation {:?}", &v[..4]),
                            Property::Position => println!("    {path}: position {:?}", &v[..3]),
                            Property::Scale => {}
                        }
                    }
                }
            }
        }
        (Some("texstats"), Some(key)) => {
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
        }
        (Some("tree"), Some(key)) => {
            for file in assets.load(key)? {
                print_tree(&assets, file)?;
            }
        }
        (Some("scripts"), Some(key)) => {
            for file in assets.load(key)? {
                for obj in assets.objects_of(file, class::MONO_BEHAVIOUR).collect::<Vec<_>>() {
                    let owner = assets
                        .read(obj)
                        .ok()
                        .and_then(|v| v.get("m_GameObject").and_then(|g| g.as_pptr()))
                        .and_then(|p| assets.resolve_loaded(file, p))
                        .and_then(|go| assets.read(go).ok())
                        .and_then(|go| go.get("m_Name").and_then(|n| n.as_str()).map(str::to_owned))
                        .unwrap_or_default();
                    println!("{:>20} {:<34} {owner}", obj.path_id, script_name(&assets, obj));
                }
            }
        }
        // One of the install's weapons at rest from its side and from
        // above, and over it, half seen through, a model of one's own
        // (a binary FBX file whose barrel runs along -z with y up), the
        // two laid with their triggers together: `overlay <weapon> <fbx>
        // <the fbx's trigger model> <out.png> [pixels a centimetre]
        // [ahead behind above below]`. The crosses are the weapon's
        // markers for the palms (left blue, right green) at rest.
        (Some("overlay"), Some(template)) => {
            use trp_data::unity::extract::read_mesh;
            let (Some(file), Some(trigger_model), Some(out)) = (arg(2), arg(3), arg(4)) else { bail!("overlay <weapon> <fbx> <trigger model> <out.png>") };
            let scale = arg(5).and_then(|s| s.parse().ok()).unwrap_or(12.0);
            let db = trp_data::Database::load(&assets.install)?;
            let weapon = trp_data::weapon::assemble_named(&mut assets, &db, template)?;
            let tree = &weapon.hierarchy;
            let root = (0..tree.nodes.len()).find(|&i| tree.path(i) == "Weapon_root/Weapon_root_anim/weapon").ok_or_else(|| anyhow::anyhow!("no weapon bone"))?;
            // A point of a node, on the weapon's own bone at rest.
            let on_weapon = |node: usize, mut p: [f32; 3]| -> Option<[f32; 3]> {
                let mut at = node;
                while at != root {
                    let n = &tree.nodes[at];
                    let q = n.rotation;
                    let v: [f32; 3] = std::array::from_fn(|k| p[k] * n.scale[k]);
                    // The turn of a vector by a quaternion.
                    let t = [2.0 * (q[1] * v[2] - q[2] * v[1]), 2.0 * (q[2] * v[0] - q[0] * v[2]), 2.0 * (q[0] * v[1] - q[1] * v[0])];
                    let turned = [v[0] + q[3] * t[0] + q[1] * t[2] - q[2] * t[1], v[1] + q[3] * t[1] + q[2] * t[0] - q[0] * t[2], v[2] + q[3] * t[2] + q[0] * t[1] - q[1] * t[0]];
                    p = std::array::from_fn(|k| turned[k] + n.translation[k]);
                    at = n.parent?;
                }
                Some(p)
            };
            let mut theirs: Vec<[[f32; 3]; 3]> = Vec::new();
            let (mut low, mut high) = ([f32::MAX; 3], [f32::MIN; 3]);
            for (index, node) in tree.nodes.iter().enumerate() {
                let Some(renderer) = &node.renderer else { continue };
                let path = tree.path(index).to_lowercase();
                if node.name.contains("LOD1") || node.name.contains("LOD2") || path.contains("base human") {
                    continue;
                }
                let Ok(mesh) = read_mesh(&assets, renderer.mesh) else { continue };
                let Some(placed) = mesh.positions.iter().map(|p| on_weapon(index, *p)).collect::<Option<Vec<_>>>() else { continue };
                if node.name.to_lowercase().contains("trigger") {
                    for p in &placed {
                        (0..3).for_each(|k| (low[k], high[k]) = (low[k].min(p[k]), high[k].max(p[k])));
                    }
                }
                for triangle in mesh.submeshes.iter().flat_map(|indices| indices.chunks_exact(3)) {
                    if triangle.iter().all(|&i| (i as usize) < placed.len()) {
                        theirs.push([0, 1, 2].map(|k| placed[triangle[k] as usize]));
                    }
                }
            }
            if low[0] > high[0] {
                bail!("the weapon has no trigger mesh to lay the two together by");
            }
            let trigger: [f32; 3] = std::array::from_fn(|k| (low[k] + high[k]) * 0.5);
            // On the weapon's bone the barrel runs along -y and up is z.
            let ours_of = |p: [f32; 3]| [-(p[1] - trigger[1]) * 100.0, (p[2] - trigger[2]) * 100.0, (p[0] - trigger[0]) * 100.0];
            let mut triangles: Vec<Drawn> = theirs.iter().map(|corners| (corners.map(ours_of), [150, 155, 165], 1.0)).collect();
            let scene = trp_data::fbx::read(&std::fs::read(file)?)?;
            let unit = scene.unit as f32;
            let blade = scene.models.iter().find(|model| model.name == trigger_model).and_then(|model| model.mesh.as_ref()).ok_or_else(|| anyhow::anyhow!("the file has no model {trigger_model} with a mesh"))?;
            let (mut low, mut high) = ([f32::MAX; 3], [f32::MIN; 3]);
            for p in &blade.positions {
                (0..3).for_each(|k| (low[k], high[k]) = (low[k].min(p[k]), high[k].max(p[k])));
            }
            let middle: [f32; 3] = std::array::from_fn(|k| (low[k] + high[k]) * 0.5);
            println!("the weapon's trigger is at {trigger:.4?} on its bone; the model's at {middle:.2?}");
            let skip: Vec<&str> = args.iter().filter_map(|a| a.strip_prefix("--without=")).collect();
            for model in scene.models.iter().filter(|model| !skip.contains(&model.name.as_str())) {
                let Some(mesh) = &model.mesh else { continue };
                for corners in mesh.positions.chunks_exact(3) {
                    let placed = [0, 1, 2].map(|k| [-(corners[k][2] - middle[2]) * unit, (corners[k][1] - middle[1]) * unit, (corners[k][0] - middle[0]) * unit]);
                    triangles.push((placed, [235, 120, 30], 0.5));
                }
            }
            let named = |name: &str| (0..tree.nodes.len()).find(|&i| tree.nodes[i].name == name).and_then(|i| on_weapon(i, [0.0; 3])).map(ours_of);
            let mut marks = Vec::new();
            marks.extend(named("weapon_L_IK_marker").map(|at| (at, [30, 60, 220])));
            marks.extend(named("weapon_R_IK_marker").map(|at| (at, [20, 150, 40])));
            return trp_data::sketch::views(&triangles, &marks, &[], out, scale, window_of(&args, 6));
        }
        // A weapon that is not the install's (`trp_data::custom`), by its
        // item template: what its file made of it. `custom` alone lists
        // them, with whatever was left out and why.
        (Some("custom"), which) => {
            let db = trp_data::Database::load(&assets.install)?;
            for note in &db.custom_notes {
                println!("left out: {note}");
            }
            for (id, definition) in &db.custom {
                println!("{id} {} on the rig of {}", definition.name, db.item(&definition.donor)?.name);
            }
            let Some(id) = which else { return Ok(()) };
            let weapon = trp_data::weapon::assemble(&mut assets, &db, id)?;
            let custom = weapon.custom.as_ref().ok_or_else(|| anyhow::anyhow!("{id} is one of the install's own"))?;
            println!(
                "{}: {:.3} kg loaded, ergonomics {}, {} cells wide, a magazine of {:?}, held closer {}",
                weapon.name, weapon.total_weight, weapon.total_ergonomics, weapon.cell_width, weapon.magazine_capacity, weapon.held_closer
            );
            for part in &custom.parts {
                let triangles: usize = part.mesh.submeshes.iter().map(|s| s.len() / 3).sum();
                println!("  {} on {}: {} vertices, {triangles} triangles", part.mesh.name, weapon.hierarchy.path(weapon.hierarchy.nodes[part.node].parent.unwrap_or(0)), part.mesh.positions.len());
            }
            let started = std::time::Instant::now();
            let read = trp_data::custom::pictures_of(&custom.materials);
            println!("its pictures were read in {:.2} s", started.elapsed().as_secs_f32());
            for (index, (material, pictures)) in custom.materials.iter().zip(read).enumerate() {
                let pictures = pictures?;
                println!("  material {}", material.name);
                // What the parts drawn with it see of its colour: the mean
                // of the picture where their vertices lie on it, and how
                // far over it they reach.
                if let Some(colour) = &pictures.colour {
                    let (mut sums, mut count, mut low, mut high) = ([0u64; 3], 0u64, [f32::MAX; 2], [f32::MIN; 2]);
                    for part in custom.parts.iter().filter(|part| part.materials.contains(&index)) {
                        for uv in &part.mesh.uv0 {
                            (0..2).for_each(|k| (low[k], high[k]) = (low[k].min(uv[k]), high[k].max(uv[k])));
                            let x = ((uv[0].rem_euclid(1.0)) * colour.width as f32) as usize % colour.width as usize;
                            let y = ((uv[1].rem_euclid(1.0)) * colour.height as f32) as usize % colour.height as usize;
                            let at = (y * colour.width as usize + x) * colour.channels;
                            (0..3).for_each(|k| sums[k] += colour.data[at + k] as u64);
                            count += 1;
                        }
                    }
                    let mean = sums.map(|sum| sum / count.max(1));
                    println!("    at its parts' vertices the colour is {mean:?} on average; they lie from {low:.3?} to {high:.3?} on the picture");
                }
                for (name, picture) in [("colour", &pictures.colour), ("normal", &pictures.normal), ("surface (occlusion, roughness, metal)", &pictures.surface)] {
                    let Some(picture) = picture else {
                        println!("    no {name}");
                        continue;
                    };
                    // The mean of each channel over the largest size.
                    let first = picture.width as usize * picture.height as usize * picture.channels;
                    let mut sums = vec![0u64; picture.channels];
                    for pixel in picture.data[..first].chunks_exact(picture.channels) {
                        pixel.iter().zip(&mut sums).for_each(|(value, sum)| *sum += *value as u64);
                    }
                    let means: Vec<String> = sums.iter().map(|sum| format!("{:.0}", *sum as f64 / (picture.width * picture.height) as f64)).collect();
                    println!("    {name}: {} x {}, {} sizes, {} bytes, means {}", picture.width, picture.height, picture.mips, picture.data.len(), means.join(" "));
                }
            }
        }
        // A binary FBX file, which is no part of the install: its models
        // with where each is and how large, and each mesh's connected
        // pieces. `fbx <file> records` prints the file's records instead.
        (Some("fbx"), Some(file)) => {
            let data = std::fs::read(file)?;
            if arg(2) == Some("records") {
                fn show(record: &trp_data::fbx::Record, indent: usize, depth: usize) {
                    let values: Vec<String> = record
                        .props
                        .iter()
                        .map(|prop| match prop {
                            trp_data::fbx::Prop::Int(v) => v.to_string(),
                            trp_data::fbx::Prop::Float(v) => v.to_string(),
                            trp_data::fbx::Prop::Text(v) => format!("{:?}", v.replace('\0', "|").replace('\u{1}', "")),
                            trp_data::fbx::Prop::Raw(v) => format!("<{} bytes>", v.len()),
                            trp_data::fbx::Prop::Ints(v) => format!("<{} ints>", v.len()),
                            trp_data::fbx::Prop::Floats(v) => format!("<{} floats>", v.len()),
                        })
                        .collect();
                    println!("{}{} {}", "  ".repeat(indent), record.name, values.join(" "));
                    if depth > 0 {
                        record.children.iter().for_each(|child| show(child, indent + 1, depth - 1));
                    }
                }
                let depth = arg(3).and_then(|d| d.parse().ok()).unwrap_or(2);
                show(&trp_data::fbx::parse(&data)?, 0, depth);
                return Ok(());
            }
            let scene = trp_data::fbx::read(&data)?;
            // `fbx <file> slice <model> <axis> <from> <to> [piece]`: what
            // of a model lies in a slab across one axis (x, y or z), as
            // the least and most of its corners there: for measuring a
            // feature of it.
            if arg(2) == Some("slice") {
                let (name, axis) = (arg(3).unwrap_or(""), arg(4).and_then(|a| "xyz".find(a)).unwrap_or(0));
                let bound = |i: usize, default: f32| arg(i).and_then(|v| v.parse::<f32>().ok()).unwrap_or(default);
                let (from, to) = (bound(5, f32::MIN), bound(6, f32::MAX));
                let piece: Option<u32> = arg(7).and_then(|p| p.parse().ok());
                for model in scene.models.iter().filter(|model| model.name == name) {
                    let Some(mesh) = &model.mesh else { continue };
                    let (mut low, mut high, mut count) = ([f32::MAX; 3], [f32::MIN; 3], 0);
                    for (corner, p) in mesh.positions.iter().enumerate() {
                        if p[axis] < from || p[axis] > to || piece.is_some_and(|piece| mesh.piece[corner / 3] != piece) {
                            continue;
                        }
                        count += 1;
                        (0..3).for_each(|k| (low[k], high[k]) = (low[k].min(p[k]), high[k].max(p[k])));
                    }
                    println!("{name}: {count} corners there, from {low:.2?} to {high:.2?}");
                    // With `points` as the last word: each of them, once.
                    if args.last().map(String::as_str) == Some("points") {
                        let mut points: Vec<[i32; 3]> = mesh
                            .positions
                            .iter()
                            .enumerate()
                            .filter(|(corner, p)| p[axis] >= from && p[axis] <= to && piece.is_none_or(|piece| mesh.piece[corner / 3] == piece))
                            .map(|(_, p)| p.map(|v| (v * 100.0).round() as i32))
                            .collect();
                        points.sort();
                        points.dedup();
                        for p in points {
                            println!("  {:7.2} {:7.2} {:7.2}", p[0] as f32 / 100.0, p[1] as f32 / 100.0, p[2] as f32 / 100.0);
                        }
                    }
                }
                return Ok(());
            }
            // `fbx <file> view <out.png> [pixels a centimetre]`: the model
            // from its left side and from above, each model in a colour of
            // its own, over a grid of the file's own centimetres with the
            // origin where the file has it. A file whose barrel runs along
            // -z with y up, as this project's first one does.
            if arg(2) == Some("view") {
                let out = arg(3).unwrap_or("local/shots/fbx_view.png");
                let scale = arg(4).and_then(|s| s.parse().ok()).unwrap_or(12.0);
                let unit = scene.unit as f32;
                let mut triangles: Vec<Drawn> = Vec::new();
                for (index, model) in scene.models.iter().enumerate() {
                    let Some(mesh) = &model.mesh else { continue };
                    println!("{:?} is {}", PALETTE[index % PALETTE.len()], model.name);
                    for corners in mesh.positions.chunks_exact(3) {
                        let placed = [0, 1, 2].map(|k| [-corners[k][2] * unit, corners[k][1] * unit, corners[k][0] * unit]);
                        triangles.push((placed, PALETTE[index % PALETTE.len()], 1.0));
                    }
                }
                return trp_data::sketch::views(&triangles, &[], &[], out, scale, window_of(&args, 5));
            }
            println!("{} cm a unit; up {:?}, front {:?}, third {:?}", scene.unit, scene.up, scene.front, scene.coord);
            let fewest: usize = arg(2).and_then(|n| n.parse().ok()).unwrap_or(200);
            for model in &scene.models {
                let parent = model.parent.map_or("-", |p| scene.models[p].name.as_str());
                let at = [model.world[0][3], model.world[1][3], model.world[2][3]];
                println!("{}  (under {parent})  at {:.3?}  materials {:?}", model.name, at, model.materials);
                let Some(mesh) = &model.mesh else { continue };
                let bounds = |wanted: &dyn Fn(usize) -> bool| {
                    let (mut low, mut high, mut count) = ([f32::MAX; 3], [f32::MIN; 3], 0usize);
                    for triangle in (0..mesh.piece.len()).filter(|&t| wanted(t)) {
                        count += 1;
                        for p in &mesh.positions[triangle * 3..triangle * 3 + 3] {
                            (0..3).for_each(|k| (low[k], high[k]) = (low[k].min(p[k]), high[k].max(p[k])));
                        }
                    }
                    (low, high, count)
                };
                let (low, high, count) = bounds(&|_| true);
                println!("  {count} triangles in {} pieces, from {low:.2?} to {high:.2?}", mesh.pieces);
                for piece in 0..mesh.pieces {
                    let (low, high, count) = bounds(&|t| mesh.piece[t] == piece);
                    if count < fewest {
                        continue;
                    }
                    let first = mesh.piece.iter().position(|&p| p == piece).unwrap_or(0);
                    let material = model.materials.get(mesh.materials[first] as usize).map_or("?", String::as_str);
                    println!("    piece {piece:4}: {count:6} triangles  {material:12} from {low:.2?} to {high:.2?}");
                }
            }
        }
        _ => bail!("usage: probe find|info|names|dump|tree ..."),
    }
    Ok(())
}

fn describe_transition(t: &trp_data::states::Transition) -> String {
    let conditions: Vec<String> = t
        .conditions
        .iter()
        .map(|c| match c.mode {
            1 => c.parameter.clone(),
            2 => format!("!{}", c.parameter),
            3 => format!("{} > {}", c.parameter, c.threshold),
            4 => format!("{} < {}", c.parameter, c.threshold),
            6 => format!("{} == {}", c.parameter, c.threshold),
            7 => format!("{} != {}", c.parameter, c.threshold),
            other => format!("{} ?{other} {}", c.parameter, c.threshold),
        })
        .collect();
    let interruption = match t.interruption {
        0 => String::new(),
        other => format!("  interruptible({other}{})", if t.ordered_interruption { " ordered" } else { "" }),
    };
    format!(
        "{}{}  blend {}{}  from {}{}{}{interruption}",
        t.to.as_deref().unwrap_or(if t.to_selector.is_some() { "" } else { "(exit)" }),
        t.to_selector.map_or(String::new(), |s| format!("(point {s})")),
        t.duration,
        if t.fixed_duration { " s" } else { " of the state" },
        t.offset,
        t.exit_time.map_or(String::new(), |e| format!("  after {e}")),
        if conditions.is_empty() { String::new() } else { format!("  if {}", conditions.join(" & ")) }
    )
}

/// Prints a JSON value down to `depth` levels: below that, an object or a
/// list is only counted.
fn print_json(value: &serde_json::Value, depth: usize, indent: usize) {
    let pad = "  ".repeat(indent);
    let short = |value: &serde_json::Value| match value {
        serde_json::Value::Object(map) => format!("{{{} entries}}", map.len()),
        serde_json::Value::Array(list) => format!("[{} items]", list.len()),
        other => {
            let text = other.to_string();
            if text.len() > 160 { format!("{}...", &text[..text.char_indices().nth(160).map_or(text.len(), |(i, _)| i)]) } else { text }
        }
    };
    match value {
        serde_json::Value::Object(map) if depth > 0 => {
            for (name, value) in map {
                match value {
                    serde_json::Value::Object(_) | serde_json::Value::Array(_) if depth > 1 => {
                        println!("{pad}{name}:");
                        print_json(value, depth - 1, indent + 1);
                    }
                    _ => println!("{pad}{name}: {}", short(value)),
                }
            }
        }
        serde_json::Value::Array(list) if depth > 0 => {
            for (index, value) in list.iter().enumerate() {
                match value {
                    serde_json::Value::Object(_) | serde_json::Value::Array(_) if depth > 1 => {
                        println!("{pad}[{index}]");
                        print_json(value, depth - 1, indent + 1);
                    }
                    _ => println!("{pad}[{index}] {}", short(value)),
                }
            }
        }
        other => println!("{pad}{}", short(other)),
    }
}

fn class_label(id: i32) -> String {
    match class::name(id) {
        "" => format!("#{id}"),
        name => name.to_owned(),
    }
}

fn print_tree(assets: &Assets, file: usize) -> Result<()> {
    for transform in assets.objects_of(file, class::TRANSFORM) {
        let t = assets.read(transform)?;
        let is_root = t.req("m_Father")?.as_pptr().is_some_and(|p| p.is_null());
        if is_root {
            print_node(assets, transform, 0)?;
        }
    }
    Ok(())
}

fn print_node(assets: &Assets, transform: ObjRef, depth: usize) -> Result<()> {
    let t = assets.read(transform)?;
    let go_ptr = t.req("m_GameObject")?.as_pptr().unwrap();
    let mut name = String::from("?");
    let mut components = Vec::new();
    if let Some(go_ref) = assets.resolve_loaded(transform.file, go_ptr) {
        let go = assets.read(go_ref)?;
        name = go.get("m_Name").and_then(|n| n.as_str()).unwrap_or("").to_owned();
        for c in go.req("m_Component")?.as_array().unwrap_or(&[]) {
            let Some(ptr) = c.get("component").and_then(|p| p.as_pptr()) else { continue };
            let Some(obj) = assets.resolve_loaded(go_ref.file, ptr) else { continue };
            match assets.class_id(obj) {
                Some(class::TRANSFORM) | None => {}
                Some(class::MONO_BEHAVIOUR) => components.push(script_name(assets, obj)),
                Some(id) => components.push(class_label(id)),
            }
        }
    }
    let pos = t.req("m_LocalPosition")?.as_vec3().unwrap_or_default();
    let rot = t.req("m_LocalRotation")?.as_vec4().unwrap_or_default();
    let scale = t.req("m_LocalScale")?.as_vec3().unwrap_or_default();
    let mut extra = String::new();
    if rot[3].abs() < 0.9999 {
        extra += &format!(" rot({:.3} {:.3} {:.3} {:.3})", rot[0], rot[1], rot[2], rot[3]);
    }
    if scale.iter().any(|s| (s - 1.0).abs() > 1e-4) {
        extra += &format!(" scale({:.3} {:.3} {:.3})", scale[0], scale[1], scale[2]);
    }
    println!(
        "{}{name}  ({:.3} {:.3} {:.3}){extra}  {}",
        "  ".repeat(depth),
        pos[0],
        pos[1],
        pos[2],
        components.join(", ")
    );
    for child in t.req("m_Children")?.as_array().unwrap_or(&[]) {
        if let Some(obj) = child.as_pptr().and_then(|p| assets.resolve_loaded(transform.file, p)) {
            print_node(assets, obj, depth + 1)?;
        }
    }
    Ok(())
}

/// Class name of the script behind a MonoBehaviour. Only the header fields
/// are needed, so an unreadable body does not matter.
fn script_name(assets: &Assets, behaviour: ObjRef) -> String {
    let script = assets
        .read(behaviour)
        .ok()
        .and_then(|v| v.get("m_Script").and_then(|s| s.as_pptr()))
        .and_then(|p| assets.resolve_loaded(behaviour.file, p))
        .and_then(|obj| assets.read(obj).ok())
        .and_then(|v| v.get("m_ClassName").and_then(|n| n.as_str()).map(str::to_owned));
    format!("[{}]", script.as_deref().unwrap_or("script"))
}

/// A window for `sketch::views` from four words of the command line, if
/// they are there: centimetres ahead, behind, above and below.
fn window_of(args: &[String], first: usize) -> Option<[f32; 4]> {
    let number = |i: usize| args.get(first + i)?.parse::<f32>().ok();
    Some([number(0)?, number(1)?, number(2)?, number(3)?])
}

/// Colours to tell parts apart by.
const PALETTE: [[u8; 3]; 8] = [[120, 130, 145], [200, 120, 60], [70, 150, 90], [170, 80, 160], [60, 130, 200], [190, 170, 60], [200, 70, 70], [90, 90, 90]];
