//! A level's scenes, props, clouds, colliders, ballistics, lights, layers and collisions.

use std::collections::BTreeMap;

use anyhow::Result;
use trp_data::unity::{class, Assets, ObjRef};

use crate::printing::print_prop_lods;

// Things laid in a level's scene, as `scene_props` reads them to be
// drawn elsewhere (`sceneprops bunker_2.unity 10_workbench/level3`):
// each object, what it draws (its meshes' corners and parts, their
// materials), and what could not be read.
pub fn sceneprops(assets: &mut Assets, args: &[String], scene: &str) -> Result<()> {
    let arg = |i: usize| args.get(i).map(String::as_str);
    let path = arg(2).unwrap_or("");
    let keep: Vec<&str> = arg(3).into_iter().collect();
    // (Every level of detail read and tagged, as the app reads them
    // with the levels of detail on.)
    let props = trp_data::level::scene_props::read(&assets.install, scene, path, &["StaticDeferredDecal"], &[], &keep, true)?;
    println!("{} objects, {} meshes, {} materials, {} pictures; the object read at {:?}, turned {:?}", props.nodes.len(), props.meshes.len(), props.materials.len(), props.pictures.len(), props.root.0, props.root.1);
    for node in props.nodes.iter().filter(|node| !node.drawn.is_empty()) {
        let drawn: Vec<String> = node.drawn.iter().map(|&(mesh, material)| {
            let (mesh, material) = (&props.meshes[mesh], &props.materials[material]);
            format!("{} ({} corners) in {} (queue {}, base {:?})", mesh.name, mesh.positions.len(), material.name, material.queue, material.base.map(|p| &props.pictures[p].name))
        }).collect();
        println!("{}{}{}: {}", node.name, if node.in_root { " [joined]" } else { "" }, node.lod.map_or(String::new(), |lod| format!(" [group {} levels {:#b}]", lod.group, lod.levels)), drawn.join("; "));
    }
    print_prop_lods(&props);
    for skipped in &props.skipped {
        println!("skipped: {skipped}");
    }
    let (low, high) = props.bounds();
    println!("all of it lies from {low:?} to {high:?}");
    for (index, node) in props.nodes.iter().enumerate().filter(|(_, node)| !node.boxes.is_empty() && (node.name.contains("raycast") || node.name.contains("trigger"))) {
        for (centre, size, trigger) in &node.boxes {
            let corners: Vec<[f32; 3]> = (0..8).map(|k| props.in_root(index, [centre[0] + size[0] / 2.0 * if k & 1 == 0 { -1.0 } else { 1.0 }, centre[1] + size[1] / 2.0 * if k & 2 == 0 { -1.0 } else { 1.0 }, centre[2] + size[2] / 2.0 * if k & 4 == 0 { -1.0 } else { 1.0 }])).collect();
            let lo: Vec<f32> = (0..3).map(|k| corners.iter().map(|c| c[k]).fold(f32::MAX, f32::min)).collect();
            let hi: Vec<f32> = (0..3).map(|k| corners.iter().map(|c| c[k]).fold(f32::MIN, f32::max)).collect();
            println!("box of {} (trigger {trigger}): middle {centre:?} size {size:?}; in the object read's axes from {lo:?} to {hi:?}", node.name);
        }
    }
    // Each object right under the second level (the area's things),
    // the box of what it draws.
    for (index, node) in props.nodes.iter().enumerate().filter(|(_, node)| node.parent.is_some_and(|p| props.nodes[p].parent == Some(0))) {
        let mut stack = vec![index];
        let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
        while let Some(at) = stack.pop() {
            stack.extend((0..props.nodes.len()).filter(|&c| props.nodes[c].parent == Some(at)));
            for &(mesh, _) in &props.nodes[at].drawn {
                for p in &props.meshes[mesh].positions {
                    let w = if props.nodes[at].in_root { *p } else { props.in_root(at, *p) };
                    for k in 0..3 { lo[k] = lo[k].min(w[k]); hi[k] = hi[k].max(w[k]); }
                }
            }
        }
        if lo[0] <= hi[0] {
            println!("  {:40} x {:6.2}..{:6.2}  y {:5.2}..{:5.2}  z {:6.2}..{:6.2}", node.name, lo[0], hi[0], lo[1], hi[1], lo[2], hi[2]);
        }
    }
    Ok(())
}

// The build's scenes, each with the file it is, and the inventory
// screen's equipment panel as the install lays it out.
// Customs' clouds and moon as the install has them (`clouds`, its
// settings, remap and map; the moon's sphere and picture), and the
// remap at a cloudiness if one is given (`clouds -0.1949`).
pub fn clouds(assets: &mut Assets, cloudiness: Option<&str>) -> Result<()> {
    let level = trp_data::level::tod::TimeOfDayLevel::customs(&assets.install)?;
    let clouds = trp_data::level::clouds::LevelClouds::read(&assets.install, &level)?;
    println!("{:#?}", clouds.settings);
    println!("ambient colour: {:?}", clouds.ambient_color);
    println!("map {} {}x{} format {} clamp {:?} point {}", clouds.map.name, clouds.map.width, clouds.map.height, clouds.map.format, clouds.map.clamp, clouds.map.point);
    for (n, curve) in [&clouds.remap.r, &clouds.remap.g, &clouds.remap.b, &clouds.remap.a].into_iter().enumerate() {
        println!("remap {}: {:?}", "RGBA".as_bytes()[n] as char, curve.keys.iter().map(|k| (k.time, k.value)).collect::<Vec<_>>());
    }
    if let Some(c) = cloudiness.and_then(|c| c.parse::<f32>().ok()) {
        println!("at {c}: normalised {} opacities {:?}", (c + 1.0) / 2.0, clouds.remap.at((c + 1.0) / 2.0));
    }
    let moon = trp_data::level::clouds::LevelMoon::read(&assets.install, &level)?;
    println!("moon mesh {}: {} vertices, {} triangles, {} normals, {} uvs; picture {} {}x{} format {} srgb {}; tiling {:?}", moon.mesh.name, moon.mesh.positions.len(), moon.mesh.triangles.len() / 3, moon.mesh.normals.len(), moon.mesh.uvs.len(), moon.picture.name, moon.picture.width, moon.picture.height, moon.picture.format, moon.picture.srgb, moon.tiling);
    for (v, p) in moon.mesh.positions.iter().enumerate().step_by((moon.mesh.positions.len() / 40).max(1)) {
        println!("  {v}: {:?} uv {:?}", p, moon.mesh.uvs.get(v));
    }
    Ok(())
}

// Factory's colliders by day near a point, within a reach
// (`level-colliders -9.77 -0.04 4.28 3 [--off]`): each one's scene and object, its
// layer, its shape and the box it lies in (`trp_data::level_colliders`;
// particles spec, "Meeting the world").
pub fn level_colliders(assets: &mut Assets, args: &[String], x: &str) -> Result<()> {
    let arg = |i: usize| args.get(i).map(String::as_str);
    let near = [x.parse()?, anyhow::Context::context(arg(2), "y")?.parse()?, anyhow::Context::context(arg(3), "z")?.parse()?];
    let reach: f32 = arg(4).map_or(Ok(3.0), str::parse)?;
    let started = std::time::Instant::now();
    let off_too = args.iter().any(|a| a == "--off");
    let named: Vec<&str> = args.windows(2).filter(|w| w[0] == "--scene").map(|w| w[1].as_str()).collect();
    let scenes: Vec<&str> = if named.is_empty() { trp_data::level::level_colliders::FACTORY_DAY.to_vec() } else { named };
    let level = trp_data::level::level_colliders::read_with(&assets.install, &scenes, near, reach, off_too)?;
    println!("{} colliders within {reach} m of {near:?}, read in {:.1} s", level.colliders.len(), started.elapsed().as_secs_f32());
    for c in &level.colliders {
        let shape = match &c.shape {
            trp_data::level::level_colliders::Shape::Box { centre, size } => format!("box {size:?} at {centre:?}"),
            trp_data::level::level_colliders::Shape::Sphere { centre, radius } => format!("sphere {radius} at {centre:?}"),
            trp_data::level::level_colliders::Shape::Capsule { centre, radius, height, direction } => format!("capsule {radius} by {height} along {direction} at {centre:?}"),
            trp_data::level::level_colliders::Shape::Mesh { convex, mesh } => format!("{}mesh {} ({} corners, {} triangles)", if *convex { "convex " } else { "" }, mesh.name, mesh.positions.len(), mesh.triangles.len()),
        };
        println!("  layer {:2}{}{} {}: {}: {shape}; from {:?} to {:?}", c.layer, if c.trigger { " trigger" } else { "" }, if c.on { "" } else { " OFF" }, c.scene.rsplit('/').next().unwrap_or(&c.scene), c.path, c.bounds.0, c.bounds.1);
    }
    for s in &level.skipped {
        println!("  skipped: {s}");
    }
    Ok(())
}

// A level's ballistic scripts (`level-ballistics [name part]
// [--scene <path end>]...`; Factory by day's unless scenes are
// named): each one's scene, object, place, material and figures
// (`trp_data::level_colliders::ballistics`).
pub fn level_ballistics(assets: &mut Assets, args: &[String], filter: Option<&str>) -> Result<()> {
    let filter = filter.filter(|f| !f.starts_with("--")).unwrap_or("").to_lowercase();
    let named: Vec<&str> = args.windows(2).filter(|w| w[0] == "--scene").map(|w| w[1].as_str()).collect();
    let scenes: Vec<&str> = if named.is_empty() { trp_data::level::level_colliders::FACTORY_DAY.to_vec() } else { named };
    let started = std::time::Instant::now();
    let all = trp_data::level::level_colliders::ballistics(&assets.install, &scenes)?;
    println!("{} ballistic scripts, read in {:.1} s", all.len(), started.elapsed().as_secs_f32());
    for b in all.iter().filter(|b| b.name.to_lowercase().contains(&filter)) {
        println!(
            "  {}{}{} {} at {:?}: material {} preset {} figures {:?}",
            if b.on { "" } else { "OFF " },
            if b.composer { "composer " } else { "" },
            b.scene.rsplit('/').next().unwrap_or(&b.scene),
            b.path,
            b.position,
            b.material,
            b.preset,
            b.figures
        );
    }
    Ok(())
}

// A level's light as its scene sets it up (Factory by day's unless a
// scene's path end is given): its sun, its level settings, its simple
// sky's figures and its dome (`docs/spec/lighting.md`).
pub fn level_light(assets: &mut Assets, args: &[String], scene: Option<&str>) -> Result<()> {
    let arg = |i: usize| args.get(i).map(String::as_str);
    let scene = scene.filter(|s| !s.starts_with("--"));
    let light = match scene {
        Some(scene) => trp_data::level::level_light::LevelLight::of_scene(&assets.install, scene)?,
        None => trp_data::level::level_light::LevelLight::factory_day(&assets.install)?,
    };
    let sun = &light.sun;
    println!("{}: the sun {} (rotation {:?}, its light going {:?})", light.file, sun.path, sun.rotation, sun.forward);
    println!("  colour {:?}, intensity {}, shadows {} at {}, layers {:#x}", sun.color, sun.intensity, sun.shadows, sun.shadow_strength, sun.culling_mask);
    let s = &light.settings;
    println!("level settings: ambient mode {}, sky {:?}, equator {:?}, ground {:?}, intensity {}", s.ambient_mode, s.sky_color, s.equator_color, s.ground_color, s.ambient_intensity);
    println!("  fog {} mode {} colour {:?} density {} from {} to {}", s.fog, s.fog_mode, s.fog_color, s.fog_density, s.fog_start, s.fog_end);
    println!("  sun colour {:?}, direction light shadow {}, min smoke ambient {:?}, height falloff {}, zero level {}, skybox {:?}", s.sun_color, s.direction_light_shadow, s.min_smoke_ambient, s.height_falloff, s.zero_level, s.skybox);
    let k = &light.sky;
    println!("simple sky: atmosphere {:?}, cloudness {:?}, harmonics daylight {}", k.atmosphere, k.cloudness, k.harmonics_daylight);
    for (name, g) in [("light colour", &k.light_color), ("add top ambient", &k.add_top_ambient), ("add ambient", &k.add_ambient)] {
        println!("  {name}: colours {:?} alphas {:?}{}", g.colors, g.alphas, if g.stepped { " stepped" } else { "" });
    }
    for (name, c) in [("bounce", &k.bounce), ("brightness", &k.brightness), ("saturation", &k.saturation), ("contrast", &k.contrast)] {
        println!("  {name}: {:?}", c.keys.iter().map(|key| (key.time, key.value, key.in_slope, key.out_slope)).collect::<Vec<_>>());
    }
    println!("dome: {:?}", light.dome);
    println!("outdoor: {:?}", light.outdoor);
    let camera_file = light.file_of(light.settings.camera)?;
    println!("the level's camera ({camera_file} {}): {:?}", light.settings.camera.1, trp_data::drawing::prism::PrismSettings::of_camera(&assets.install, &camera_file, light.settings.camera.1));
    println!("Cam2: {:?}", trp_data::drawing::prism::PrismSettings::of_cam2(&assets.install));
    match light.dome_sky(&assets.install) {
        Ok(sky) => {
            let c = &sky.cubemap;
            println!("dome sky: {} {}x{} format {} mips {} ({} bytes), tint {:?}, exposure {}, rotation {}", c.name, c.width, c.height, c.format, c.mip_count, c.data.len(), sky.tint, sky.exposure, sky.rotation);
            // Each face's largest level as a picture, rows as stored
            // (`--faces <folder>`).
            let faces = (1..6).find(|&n| arg(n) == Some("--faces")).and_then(|n| arg(n + 1));
            if let Some(folder) = faces {
                std::fs::create_dir_all(folder)?;
                let face_bytes = c.data.len() / 6;
                for face in 0..6 {
                    let blocks = &c.data[face * face_bytes..face * face_bytes + (c.width as usize * c.height as usize / 2)];
                    let rgba = trp_data::formats::compress::bc1_decoded(c.width, c.height, blocks);
                    let path = std::path::Path::new(folder).join(format!("face{face}.png"));
                    let mut encoder = png::Encoder::new(std::io::BufWriter::new(std::fs::File::create(&path)?), c.width, c.height);
                    encoder.set_color(png::ColorType::Rgba);
                    encoder.set_depth(png::BitDepth::Eight);
                    encoder.write_header()?.write_image_data(&rgba)?;
                }
                println!("  its faces written to {folder}");
            }
        }
        Err(e) => println!("dome sky: {e:#}"),
    }
    for (n, outside) in light.externals.iter().enumerate() {
        println!("  file {}: {outside}", n + 1);
    }
    Ok(())
}

pub fn scenes(assets: &mut Assets, filter: Option<&str>) -> Result<()> {
    let wanted = filter.unwrap_or("").to_lowercase();
    for (index, path) in trp_data::interface::screen::scene_paths(&assets.install)?.iter().enumerate().filter(|(_, path)| path.to_lowercase().contains(&wanted)) {
        println!("level{index}  {path}");
    }
    let panel = trp_data::interface::screen::GearPanel::load(&assets.install)?;
    println!("the equipment panel, from {}: {} x {}", panel.file, panel.size[0], panel.size[1]);
    for (name, [left, top, width, height]) in &panel.slots {
        println!("  {name}: left {left} top {top}, {width} x {height}");
    }
    let [width, height, up, right] = panel.quick;
    println!("the quick-use panel: {width} x {height}, {up} over the screen's lower edge, its middle {right} to the right of the screen's");
    for (key, middle, side) in &panel.quick_slots {
        println!("  {key}: its middle {middle} along the row, {side} square");
    }
    Ok(())
}

// The ragdoll a dead character becomes (`trp_data::ragdoll`).
// The engine's physics layers: their names (`TagManager`) and which
// collide with which (`PhysicsManager.m_LayerCollisionMatrix`, found
// as 32 masks after their count that agree with one another), and
// the physics settings' first figures (`layers`).
// `bundle-layers [bundle]`: how many of a bundle's game objects are
// on each layer (the effects' bundle unless told), with a few names:
// which cameras' masks take them.
pub fn bundle_layers(assets: &mut Assets, key: Option<&str>) -> Result<()> {
    use trp_data::unity::Value;
    let key = key.unwrap_or(trp_data::drawing::particles::EFFECTS);
    let files = assets.load(key)?;
    let mut layers: BTreeMap<i64, (usize, Vec<String>)> = BTreeMap::new();
    for file in files {
        let objects: Vec<ObjRef> = assets.objects_of(file, class::GAME_OBJECT).collect();
        for object in objects {
            let go = assets.read(object)?;
            let layer = go.get("m_Layer").and_then(Value::as_i64).unwrap_or(-1);
            let entry = layers.entry(layer).or_default();
            entry.0 += 1;
            if entry.1.len() < 6 {
                entry.1.push(go.get("m_Name").and_then(Value::as_str).unwrap_or("").to_owned());
            }
        }
    }
    for (layer, (count, names)) in &layers {
        println!("layer {layer}: {count} objects ({})", names.join(", "));
    }
    Ok(())
}

pub fn layers(assets: &mut Assets) -> Result<()> {
    let path = assets.install.root.join("EscapeFromTarkov_Data").join("globalgamemanagers");
    let data = std::fs::read(&path)?;
    let meta = trp_data::unity::serialized::SerializedFile::parse(&data)?;
    let object = |class_id: i32| meta.objects.iter().find(|o| o.class_id == class_id).map(|o| &data[o.offset..o.offset + o.size]);
    let word = |bytes: &[u8], at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
    // The names: the tags' list, then the layers' 32.
    let mut names: Vec<String> = Vec::new();
    if let Some(tags) = object(78) {
        let mut at = 0usize;
        let list = |at: &mut usize| -> Vec<String> {
            let count = word(tags, *at) as usize;
            *at += 4;
            (0..count)
                .map(|_| {
                    let len = word(tags, *at) as usize;
                    let text = String::from_utf8_lossy(&tags[*at + 4..*at + 4 + len]).into_owned();
                    *at = (*at + 4 + len + 3) & !3;
                    text
                })
                .collect()
        };
        let tag_list = list(&mut at);
        names = list(&mut at);
        println!("{} tags; {} layers", tag_list.len(), names.len());
    }
    let Some(physics) = object(55) else { anyhow::bail!("no physics settings") };
    let floats: Vec<String> = physics.chunks_exact(4).take(24).map(|c| {
        let v = f32::from_le_bytes(c.try_into().unwrap());
        if v.is_finite() && (v == 0.0 || v.abs() > 1e-6 && v.abs() < 1e6) { format!("{v}") } else { format!("i{}", i32::from_le_bytes(c.try_into().unwrap())) }
    }).collect();
    println!("physics settings' first words: {}", floats.join(" "));
    let rows = (0..physics.len().saturating_sub(4 * 33)).step_by(4).find(|&at| {
        word(physics, at) == 32 && (0..32).all(|i| (0..32).all(|j| (word(physics, at + 4 + 4 * i) >> j & 1) == (word(physics, at + 4 + 4 * j) >> i & 1)))
            && (0..32).any(|i| word(physics, at + 4 + 4 * i) != 0)
    });
    let Some(rows) = rows else { anyhow::bail!("no collision matrix found") };
    println!("the collision matrix at byte {rows}");
    let after: Vec<String> = physics[rows + 4 * 33..].chunks_exact(4).take(40).map(|c| {
        let v = f32::from_le_bytes(c.try_into().unwrap());
        if v.is_finite() && (v == 0.0 || v.abs() > 1e-6 && v.abs() < 1e6) { format!("{v}") } else { format!("i{}", i32::from_le_bytes(c.try_into().unwrap())) }
    }).collect();
    println!("after it: {}", after.join(" "));
    let bytes: Vec<String> = physics[rows + 4 * 33..].iter().take(64).map(|b| format!("{b:02x}")).collect();
    println!("its bytes: {}", bytes.join(" "));
    let name = |i: usize| names.get(i).filter(|n| !n.is_empty()).cloned().unwrap_or_else(|| format!("({i})"));
    for i in 0..32 {
        let mask = word(physics, rows + 4 + 4 * i);
        let with: Vec<String> = (0..32).filter(|j| mask >> j & 1 == 1).map(name).collect();
        if !names.get(i).is_some_and(|n| n.is_empty()) {
            println!("{i:2} {}: collides with {}", name(i), with.join(", "));
        }
    }
    Ok(())
}

// An item template of the database, by id or by a part of its
// name: what it weighs, the cells it takes, its slots and grids.
// An item's properties in the install's database, whole or those
// named (`props 590c678286f77426c9660122 UsePrefab medUseTime`).
// Every item that has a property, with its value: `probe with
// ExplosionEffectType`.
pub fn with(assets: &mut Assets, prop: &str) -> Result<()> {
    let db = trp_data::Database::load(&assets.install)?;
    let mut found: Vec<_> = db.items.values().filter_map(|item| Some((item.name.as_str(), item.id.as_str(), item.props.get(prop)?.to_string()))).collect();
    found.sort();
    for (name, id, value) in found {
        println!("{id} {name} {value}");
    }
    Ok(())
}

pub fn props(assets: &mut Assets, args: &[String], wanted: &str) -> Result<()> {
    let arg = |i: usize| args.get(i).map(String::as_str);
    let db = trp_data::Database::load(&assets.install)?;
    let item = db.items.get(wanted).ok_or_else(|| anyhow::anyhow!("no item {wanted}"))?;
    let names: Vec<&str> = (2..16).filter_map(arg).collect();
    println!("{} {}", item.id, item.name);
    match (names.is_empty(), item.props.as_object()) {
        (false, Some(props)) => {
            for name in names {
                println!("  {name}: {}", props.get(name).map_or("-".to_owned(), |value| value.to_string()));
            }
        }
        _ => println!("{}", serde_json::to_string_pretty(&item.props)?),
    }
    Ok(())
}

// A weapon that is not the install's (`trp_data::custom`), by its
// item template: what its file made of it. `custom` alone lists
// them, with whatever was left out and why.
// Every particle system of a bundle (the effects' where none is
// named) whose collision module is on: its object's path, its kind
// (0 planes, 1 the world's), its quality (0 high, 1 medium, 2 low:
// particles spec, "Meeting the world"), its cube's size
// (`voxelSize`), its most shapes, its layers, whether it meets
// moving bodies, its sphere's share. `probe collisions [bundle]`.
pub fn collisions(assets: &mut Assets, key: Option<&str>) -> Result<()> {
    assets.load("shaders")?;
    let files = assets.load(key.unwrap_or(trp_data::drawing::particles::EFFECTS))?;
    let name_of = |assets: &Assets, go: ObjRef| assets.read(go).ok().and_then(|g| g.get("m_Name").and_then(|n| n.as_str()).map(str::to_owned)).unwrap_or_default();
    let path_of = |assets: &Assets, mut go: ObjRef| {
        let mut names = vec![name_of(assets, go)];
        for _ in 0..32 {
            let transform = assets.read(go).ok().and_then(|g| g.get("m_Component")?.as_array()?.first()?.get("component")?.as_pptr()).and_then(|p| assets.resolve_loaded(go.file, p));
            let father = transform.and_then(|t| assets.read(t).ok()?.get("m_Father")?.as_pptr().and_then(|p| assets.resolve_loaded(t.file, p)));
            let Some(up) = father.and_then(|f| assets.read(f).ok()?.get("m_GameObject")?.as_pptr().and_then(|p| assets.resolve_loaded(f.file, p))) else { break };
            names.push(name_of(assets, up));
            go = up;
        }
        names.reverse();
        names.join("/")
    };
    let mut lines = BTreeMap::new();
    for &file in &files {
        for obj in assets.objects_of(file, class::PARTICLE_SYSTEM).collect::<Vec<_>>() {
            let s = assets.read(obj)?;
            let Some(m) = s.get("CollisionModule").filter(|m| m.get("enabled").and_then(|e| e.as_bool()) == Some(true)) else { continue };
            let int = |n: &str| m.get(n).and_then(|v| v.as_i64()).unwrap_or(-1);
            let float = |n: &str| m.get(n).and_then(|v| v.as_f32()).unwrap_or(f32::NAN);
            let go = s.get("m_GameObject").and_then(|p| p.as_pptr()).and_then(|p| assets.resolve_loaded(obj.file, p));
            let path = go.map(|go| path_of(&assets, go)).unwrap_or_default();
            let bits = m.get("collidesWith").and_then(|b| b.get("m_Bits")).and_then(|b| b.as_i64()).unwrap_or(-1) as u32;
            lines.insert((int("quality"), path.clone(), obj.path_id), format!("type {} quality {} voxel {} max {} layers {bits:#010x} dynamic {} radius {} [{}] {path}", int("type"), int("quality"), float("voxelSize"), int("maxCollisionShapes"), m.get("collidesWithDynamic").and_then(|v| v.as_bool()).unwrap_or(false), float("radiusScale"), obj.path_id));
        }
    }
    for line in lines.values() {
        println!("{line}");
    }
    Ok(())
}
