//! Folding sights, slot bones, tactical devices and sights.

use std::collections::BTreeMap;

use anyhow::Result;
use trp_data::unity::{class, Assets};

// Every weapon's and weapon part's folding sights (`foldsights`;
// `AutoFoldableSight`): the items of each bundle (`S` a sight,
// `I` an iron sight), each script's object by its path in the
// prefab, its `Mode` and whether the prefab has the object on.
pub fn foldsights(assets: &mut Assets) -> Result<()> {
    use trp_data::unity::Value as V;
    let db = trp_data::Database::load(&assets.install)?;
    let mut by_key: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for item in db.items.values().filter(|i| db.is_a(i, "Mod") || db.is_a(i, "Weapon")) {
        let Some(key) = item.prefab_key() else { continue };
        let kind = if db.is_a(item, "IronSight") { "I" } else if db.is_a(item, "Sights") { "S" } else { "-" };
        by_key.entry(key.to_owned()).or_default().push(format!("{} {} {kind}", item.id, item.name));
    }
    let mut fresh = Assets::new(assets.install.clone())?;
    for (n, (key, items)) in by_key.iter().enumerate() {
        // A fresh set of bundles now and then, to keep the memory down.
        if n % 80 == 79 {
            fresh = Assets::new(assets.install.clone())?;
        }
        let Ok(files) = fresh.load(key) else {
            println!("{key}: not loaded");
            continue;
        };
        let mut lines = Vec::new();
        for file in files {
            for obj in fresh.objects_of(file, class::MONO_BEHAVIOUR).collect::<Vec<_>>() {
                if trp_data::unity::extract::script_class(&fresh, obj).as_deref() != Some("AutoFoldableSight") {
                    continue;
                }
                let value = fresh.read(obj)?;
                let mode = value.get("Mode").and_then(V::as_i64).unwrap_or(-1);
                let Some(mut go) = value.get("m_GameObject").and_then(V::as_pptr).and_then(|p| fresh.resolve_loaded(obj.file, p)) else { continue };
                let own = fresh.read(go)?;
                let active = own.get("m_IsActive").and_then(|a| a.as_bool().or(a.as_i64().map(|x| x != 0))).unwrap_or(true);
                // The object's path, up its fathers.
                let mut path = Vec::new();
                loop {
                    let g = fresh.read(go)?;
                    path.push(g.get("m_Name").and_then(V::as_str).unwrap_or("").to_owned());
                    let father = g
                        .get("m_Component")
                        .and_then(V::as_array)
                        .unwrap_or(&[])
                        .iter()
                        .filter_map(|c| c.get("component").and_then(V::as_pptr).and_then(|p| fresh.resolve_loaded(go.file, p)))
                        .find(|c| fresh.class_id(*c) == Some(class::TRANSFORM))
                        .and_then(|t| fresh.read(t).ok().and_then(|t| t.get("m_Father").and_then(V::as_pptr)).and_then(|p| fresh.resolve_loaded(go.file, p)))
                        .and_then(|f| fresh.read(f).ok().and_then(|f| f.get("m_GameObject").and_then(V::as_pptr)).and_then(|p| fresh.resolve_loaded(go.file, p)));
                    match father {
                        Some(f) => go = f,
                        None => break,
                    }
                }
                path.reverse();
                lines.push(format!("  {} mode {} prefab {}", path.join("/"), if mode == 0 { "On" } else if mode == 1 { "Off" } else { "?" }, if active { "on" } else { "off" }));
            }
        }
        if !lines.is_empty() {
            println!("{key}: {}", items.join("; "));
            for line in lines {
                println!("{line}");
            }
        }
    }
    Ok(())
}

// Each slot's bone in its item's own model, for every weapon and
// part with slots (`slotbones [word]`): the game finds it depth
// first, its case aside (`ObjectsFactory.AttachMods`,
// `TransformTools.FindTransformRecursive`); listed, a slot whose
// bone is under another `mod_` object, one with none, one with
// more than one object of its name, and one found only with its
// case aside.
pub fn slotbones(assets: &mut Assets, word: Option<&str>) -> Result<()> {
    let db = trp_data::Database::load(&assets.install)?;
    let mut items: Vec<&trp_data::setup::database::ItemTemplate> = db.items.values().filter(|item| item.props.get("Slots").and_then(serde_json::Value::as_array).is_some_and(|slots| !slots.is_empty()) && item.prefab_key().is_some()).filter(|item| word.is_none_or(|word| item.name.contains(word))).collect();
    items.sort_by(|a, b| a.name.cmp(&b.name));
    let (mut read, mut slots, mut nested, mut missing, mut doubled, mut cased) = (0, 0, 0, 0, 0, 0);
    for item in items {
        let Some(key) = item.prefab_key() else { continue };
        // A weapon's own model as its assembly opens it.
        let tree = match if key.starts_with("assets/content/weapons/") { trp_data::weapons::weapon::assemble_prefab(&mut *assets, key).map(|weapon| weapon.hierarchy) } else { trp_data::weapons::weapon::prefab_tree(&mut *assets, key) } {
            Ok(tree) => tree,
            Err(e) => {
                println!("{} {}: not read: {e:#}", item.id, item.name);
                continue;
            }
        };
        read += 1;
        let mut children: Vec<Vec<usize>> = vec![Vec::new(); tree.nodes.len()];
        for (index, node) in tree.nodes.iter().enumerate() {
            if let Some(parent) = node.parent {
                children[parent].push(index);
            }
        }
        let names = item.props.get("Slots").and_then(serde_json::Value::as_array).into_iter().flatten().filter_map(|slot| slot.get("_name").and_then(serde_json::Value::as_str));
        for name in names.filter(|name| !name.starts_with("chamber") && !name.starts_with("patron_in_weapon") && !name.starts_with("camora")) {
            slots += 1;
            // Depth first, its case aside.
            let mut first = None;
            let mut stack: Vec<usize> = children[0].iter().rev().copied().collect();
            while let Some(node) = stack.pop() {
                if tree.nodes[node].name.eq_ignore_ascii_case(name) {
                    first = Some(node);
                    break;
                }
                stack.extend(children[node].iter().rev());
            }
            let all: Vec<usize> = (1..tree.nodes.len()).filter(|&node| tree.nodes[node].name.eq_ignore_ascii_case(name)).collect();
            let Some(first) = first else {
                missing += 1;
                println!("{} {}: {name}: none", item.id, item.name);
                continue;
            };
            if all.len() > 1 {
                doubled += 1;
                let paths: Vec<String> = all.iter().map(|&node| tree.path(node)).collect();
                println!("{} {}: {name}: {} objects: {}", item.id, item.name, all.len(), paths.join(" | "));
            }
            if tree.nodes[first].name != name {
                cased += 1;
                println!("{} {}: {name}: found as {}", item.id, item.name, tree.nodes[first].name);
            }
            let mut above = tree.nodes[first].parent;
            while let Some(node) = above.filter(|&node| node != 0) {
                if tree.nodes[node].name.starts_with("mod_") {
                    nested += 1;
                    println!("{} {}: {name}: under {}", item.id, item.name, tree.path(first));
                    break;
                }
                above = tree.nodes[node].parent;
            }
        }
    }
    println!("{read} items read, {slots} slots: {nested} under another mod_ object, {missing} with no object, {doubled} with more than one, {cased} only with their case aside");
    Ok(())
}

// A build's tactical devices (`trp_data::tactical`): a preset by its
// id or name, or a weapon's key; or one device's template alone.
pub fn devices(assets: &mut Assets, wanted: &str) -> Result<()> {
    let db = trp_data::Database::load(&assets.install)?;
    let preset = db.presets.iter().find(|p| p.id == wanted || p.name == wanted).cloned();
    let (devices, hierarchy) = if let Some(preset) = preset {
        let weapon = trp_data::weapons::weapon::assemble_preset(&mut *assets, &db, &preset)?;
        println!("{} ({}) with {} nodes", preset.name, weapon.name, weapon.hierarchy.nodes.len());
        (weapon.devices.clone(), weapon.hierarchy.clone())
    } else if trp_data::weapons::tactical::DeviceClass::of(&db, wanted).is_some() {
        let device = trp_data::weapons::tactical::read_alone(&mut *assets, &db, wanted)?;
        (vec![device], trp_data::unity::extract::Hierarchy::default())
    } else {
        let weapon = trp_data::weapons::weapon::assemble_key(&mut *assets, &db, wanted)?;
        println!("{} with {} nodes", weapon.name, weapon.hierarchy.nodes.len());
        (weapon.devices.clone(), weapon.hierarchy.clone())
    };
    let name = |node: usize| hierarchy.nodes.get(node).map(|n| n.name.clone()).unwrap_or_else(|| format!("#{node}"));
    for d in &devices {
        let item = db.items.get(&d.template).map(|t| t.name.as_str()).unwrap_or("?");
        println!("{} {item}: {:?}, {} modes, root {}, off {:?}, modes {:?}, gun shadow hidden {}", d.template, d.class, d.modes, name(d.root), d.off_node.map(name), d.mode_nodes.iter().map(|&n| name(n)).collect::<Vec<_>>(), d.hides_gun_shadow);
        for l in &d.lights {
            println!(
                "  light {} mode {:?}: {} on {} colour {:?} intensity {} range {} angle {}/{} shadows {} res {} strength {} bias {} normal {} near {} cookie {:?} mask {:#x} ir {} flare {} shaft {:?}",
                name(l.node), l.mode, if l.spot { "spot" } else { "point" }, l.enabled, l.color, l.intensity, l.range, l.spot_angle, l.inner_spot_angle, l.shadows, l.shadow_resolution, l.shadow_strength, l.shadow_bias, l.shadow_normal_bias, l.shadow_near,
                l.cookie.as_ref().map(|c| (c.name.clone(), c.width, c.height, c.format)), l.culling_mask, l.infrared, l.flare, l.shaft
            );
        }
        for b in &d.lasers {
            println!(
                "  laser {} mode {:?}: start {} max {} point light {} mask {} size {} dot {:?} offset {} light range {} intensity {} factor {} angles {:?} cookie {:?} ir {}",
                name(b.node), b.mode, b.ray_start, b.max_distance, b.use_point_light, b.mask, b.beam_size, b.point_size, b.surface_offset, b.light_range, b.light_intensity, b.intensity_factor, b.angle_close_far, b.cookie.as_ref().map(|c| c.name.clone()), b.infrared
            );
            if let Some(m) = &b.beam {
                println!("    beam colour {:?} factor {} ik {} noise scale {} main {:?} noise {:?}", m.color, m.factor, m.ik_factor, m.noise_scale, m.main.as_ref().map(|t| (t.name.clone(), t.width, t.height, t.format)), m.noise.as_ref().map(|v| (v.name.clone(), v.size, v.format, v.data.len())));
                // The pictures' figures: the noise's bytes, and the soft
                // spot's middle row (its first level) by channel.
                if let Some(v) = &m.noise {
                    let n = v.data.len().max(1) as f64;
                    let mean = v.data.iter().map(|&b| b as f64).sum::<f64>() / n;
                    let cube = v.data.iter().map(|&b| (b as f64 / 255.0).powi(3)).sum::<f64>() / n;
                    println!("      noise bytes {}..{} mean {mean:.1}, mean of the cube {cube:.4}", v.data.iter().min().unwrap_or(&0), v.data.iter().max().unwrap_or(&0));
                }
                if let Some(t) = &m.main {
                    if let Ok(mips) = trp_data::unity::texture::mips(t) {
                        if let Some(first) = mips.first() {
                            if let Ok(rgba) = trp_data::unity::texture::decode_rgba(t, *first) {
                                let row = (t.height / 2) as usize * t.width as usize * 4;
                                let across: Vec<String> = (0..8).map(|i| {
                                    let at = row + (i * t.width as usize / 8) * 4;
                                    format!("{:?}", &rgba[at..at + 4])
                                }).collect();
                                println!("      main's middle row: {}", across.join(" "));
                            }
                        }
                    }
                }
            }
            if let Some(m) = &b.point {
                println!("    dot colour {:?} factor {} ik {} min fov {} size by view {:?} max dist {} main {:?} noise {:?}", m.color, m.factor, m.ik_factor, m.min_fov, m.size_view, m.max_dist, m.main.as_ref().map(|t| (t.name.clone(), t.width, t.height, t.format)), m.noise.as_ref().map(|t| (t.name.clone(), t.width, t.height, t.format)));
            }
        }
        for flare in &d.flares {
            println!(
                "  flare {} mode {:?}: total scale {} alpha {}, controller {:?}, infrared {}",
                name(flare.node),
                flare.mode,
                flare.total_scale,
                flare.total_alpha,
                flare.controller,
                flare.infrared
            );
            for f in &flare.flares {
                println!("    {f:?}");
            }
        }
    }
    Ok(())
}

pub fn sights(assets: &mut Assets, wanted: &str) -> Result<()> {
    let db = trp_data::Database::load(&assets.install)?;
    let weapon = trp_data::weapons::weapon::assemble_key(&mut *assets, &db, wanted)?;
    let h = &weapon.hierarchy;
    println!("{} with {} nodes", weapon.name, h.nodes.len());
    for s in &weapon.skipped {
        println!("  SKIPPED {s}");
    }
    for part in &weapon.sights {
        let name = db.items.get(&part.template).map(|t| t.name.as_str()).unwrap_or("?");
        println!("{} {name} on {} (root [{}] {}){}{}", part.template, h.nodes[part.slot_bone].name, part.root, h.nodes[part.root].name, if part.iron { " iron" } else { "" }, if part.on_sight { " on a sight" } else { "" });
        for (s, scope) in part.scopes.iter().enumerate() {
            println!("  scope {s}: {} modes, zooms {:?}, sensitivity {:?}, distances {:?}", scope.modes, scope.zooms, scope.sensitivity, scope.calibration);
        }
        if let Some(a) = &part.adjustable {
            println!("  adjustable: fov {:?} speed {} zoom sensitivity {} aiming sensitivity {:?}", a.fov, a.speed, a.zoom_sensitivity, a.sensitivity);
        }
        if !part.custom_aim_plane.is_empty() {
            println!("  custom aim plane {:?}", part.custom_aim_plane);
        }
        println!("  prefab cache {}, eyecup shift {}", part.has_prefab_cache, part.eye_guard_shift);
        if let Some(turned) = &part.angle_by_range {
            println!("  turns [{}] {} by the zeroing distance: {:?}", turned.axis, h.nodes[turned.axis].name, turned.table);
        }
        if let Some(curve) = &part.zoom_curve {
            println!("  zoom curve {:?}", curve.keys);
        }
        if let Some(ring) = &part.zoom_ring {
            println!("  zoom ring [{}] {}: start {:?} end {:?} inverse {}", ring.node, h.nodes[ring.node].name, ring.start, ring.end, ring.inverse);
        }
        for (m, mode) in part.modes.iter().enumerate() {
            let node = |n: Option<usize>| n.map(|n| format!("[{n}] {}", h.nodes[n].name)).unwrap_or_else(|| "-".into());
            let optic = mode.optic.as_ref().map(|o| format!("optic {} bone {} distance {} fov {:?} camera {}", node(Some(o.node)), node(o.bone), o.distance_to_camera, o.field_of_view, node(o.camera))).unwrap_or_else(|| "no optic".into());
            println!("  mode {m}: object {} {optic} collimator {} ignore-plane {}", node(mode.node), node(mode.collimator), mode.ignore_optics_for_camera_plane);
            if let Some(o) = &mode.optic {
                println!("    near {:?}, lens {}, aberration {}{}", o.near, node(o.lens), o.aberration, if o.thermal_at_45 { ", thermal at 45" } else { "" });
                if let Some(r) = &o.reticle {
                    let moved = r.shape.as_ref().map_or(0, |s| s.iter().filter(|d| d.iter().any(|c| *c != 0.0)).count());
                    println!("    reticle: {} vertices, {} triangles, {} moved by its blend shape, at {:?} turned {:?}, scale {}, colour {:?}", r.positions.len(), r.triangles.len() / 3, moved, r.position, r.rotation, r.scale, r.color);
                }
            }
        }
        for lens in &part.lenses {
            println!("  lens piece [{}] {} slot {}: {:?}", lens.node, h.nodes[lens.node].name, lens.slot, lens.kind);
        }
    }
    for fold in &weapon.folding_sights {
        println!("folds: [{}] {} mode {}", fold.node, h.path(fold.node), if fold.mode_on { "On" } else { "Off" });
    }
    for (i, node) in h.nodes.iter().enumerate() {
        if node.name.contains("aim_camera") {
            println!("aim bone [{i}] {} active {}", h.path(i), node.active);
        }
    }
    Ok(())
}
