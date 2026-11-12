//! Items' icons, their materials and bounds, and an item's template.

use anyhow::{bail, Result};
use trp_data::unity::Assets;

// The inventory screen's health panel as the app reads it, on a
// screen 1920 by 1080 (or so wide: `healthpanel 2560`): the left
// panel's size, then each object with its rectangle in it, whether
// it shows, its sprite (with its texture's size and format) and
// its text.
// The game's window that shows a thing (`infowindow [width]`): each
// of its objects as the prefab has it, placed in a screen that wide
// and 1080 high by its own anchors, with its picture and its words.
pub fn staticicons(assets: &mut Assets) -> Result<()> {
    let attributes = trp_data::interface::screen::attribute_icons(&assets.install)?;
    println!("attributes: {}", attributes.iter().map(|(name, _)| name.as_str()).collect::<Vec<_>>().join(" "));
    for (what, table) in ["health effects", "damage effects", "buffs"].iter().zip(trp_data::interface::screen::figure_icons(&assets.install)?) {
        println!("{what}: {}", table.iter().map(|(name, _)| name.as_str()).collect::<Vec<_>>().join(" "));
    }
    Ok(())
}

pub fn icon(assets: &mut Assets, wanted: &str) -> Result<()> {
    let db = trp_data::Database::load(&assets.install)?;
    let item = db.item(wanted)?;
    let key = item.prefab_key().ok_or_else(|| anyhow::anyhow!("no prefab"))?.to_owned();
    let files = assets.load(&key)?;
    println!("{} {}: {key}", item.id, item.name);
    match trp_data::interface::icon::pose_in(&assets, &files)? {
        Some((object, pose)) => {
            let tree = trp_data::unity::extract::read_hierarchy(&assets, object)?;
            println!("  PreviewPivot on {} ({} nodes, {} with a renderer): {pose:?}", tree.nodes[0].name, tree.nodes.len(), tree.nodes.iter().filter(|n| n.renderer.is_some()).count());
            for node in tree.nodes.iter().take(6) {
                println!("    {} parent {:?} at {:?} turned {:?} scale {:?}{}", node.name, node.parent, node.translation, node.rotation, node.scale, if node.renderer.is_some() { " (drawn)" } else { "" });
            }
        }
        None => println!("  no PreviewPivot"),
    }
    // The pivot's own picture, where it has one (`overrideIcon`).
    for &file in &files {
        for behaviour in assets.objects_of(file, trp_data::unity::class::MONO_BEHAVIOUR) {
            if trp_data::unity::extract::script_class(&assets, behaviour).as_deref() == Some("PreviewPivot") {
                let value = assets.read(behaviour)?;
                println!("  overrideIcon: {:?}", value.get("Icon").and_then(|icon| icon.get("overrideIcon")));
            }
        }
    }
    if let Ok(weapon) = trp_data::weapons::weapon::assemble(&mut *assets, &db, &item.id) {
        println!("  assembled: root {} ({} nodes)", weapon.hierarchy.nodes[0].name, weapon.hierarchy.nodes.len());
    }
    Ok(())
}

// An item's model as the icon thread reads it (a weapon's
// container's object, `weapon::is_weapon_container`; else the
// prefab from its pivot's object): each renderer that is on, its
// materials, their textures by property, colours and keywords.
pub fn iconmaterials(assets: &mut Assets, wanted: &str) -> Result<()> {
    let db = trp_data::Database::load(&assets.install)?;
    let item = db.item(wanted)?;
    let key = item.prefab_key().ok_or_else(|| anyhow::anyhow!("no prefab"))?.to_owned();
    let tree = if trp_data::weapons::weapon::is_weapon_container(&mut *assets, &key)? { trp_data::weapons::weapon::assemble_prefab(&mut *assets, &key)?.hierarchy } else { trp_data::interface::icon::model_of(&mut *assets, &key)?.0 };
    if std::env::var("TRP_PROBE_FIGURES").is_ok() {
        // The shaders' bundle, so that a material's shader is found,
        // and the cubemaps', its cube.
        assets.load("shaders")?;
        assets.load("cubemaps")?;
    }
    println!("{} {}: {} nodes", item.id, item.name, tree.nodes.len());
    let mut on = vec![false; tree.nodes.len()];
    for (index, node) in tree.nodes.iter().enumerate() {
        on[index] = (node.active || node.parent.is_none()) && node.parent.is_none_or(|parent| on[parent]);
        let Some(renderer) = node.renderer.as_ref() else { continue };
        println!("  {}{}{}", node.name, if on[index] { "" } else { " (off)" }, if renderer.shadows_only { " (its shadow alone)" } else { "" });
        for material in &renderer.materials {
            let Some(material) = material else {
                println!("    no material");
                continue;
            };
            match trp_data::unity::extract::read_material(&assets, *material) {
                Ok(data) => {
                    let mut textures: Vec<String> = data.textures.iter().map(|(slot, tex)| format!("{slot}={}", trp_data::unity::extract::read_texture(&assets, *tex).map_or_else(|e| format!("({e})"), |t| format!("{} fmt {} {}x{}{}", t.name, t.format, t.width, t.height, if t.srgb { " srgb" } else { "" })))).collect();
                    textures.sort();
                    println!("    {}: colour {:?}; keywords {:?}; {}", data.name, data.colors.get("_Color"), data.keywords, textures.join("; "));
                    // With `TRP_PROBE_FIGURES` set, its shader's name
                    // and every colour and figure it keeps.
                    if std::env::var("TRP_PROBE_FIGURES").is_ok() {
                        let shader = assets
                            .read(*material)
                            .ok()
                            .and_then(|m| m.get("m_Shader")?.as_pptr())
                            .and_then(|p| assets.resolve_loaded(material.file, p))
                            .and_then(|s| assets.read(s).ok()?.get("m_ParsedForm")?.get("m_Name")?.as_str().map(str::to_owned))
                            .unwrap_or_else(|| "(not found)".to_owned());
                        let mut colors: Vec<String> = data.colors.iter().map(|(k, v)| format!("{k}={v:?}")).collect();
                        colors.sort();
                        let mut floats: Vec<String> = data.floats.iter().map(|(k, v)| format!("{k}={v}")).collect();
                        floats.sort();
                        println!("      shader {shader}\n      colours {}\n      floats {}", colors.join(" "), floats.join(" "));
                    }
                }
                Err(e) => println!("    a material not read: {e:#}"),
            }
        }
    }
    Ok(())
}

// An item's picture framed as the game frames it
// (`ItemIconCreator.CenterByBounds`, `WeaponPreview.GetBounds`):
// each renderer switched on under objects that are on, drawn or
// not (a lower LOD's, one that casts its shadow alone), its mesh's
// box (`m_LocalAABB`, and its vertices' beside it) under the pose's
// turn; the camera's half height from them all, and from the drawn
// ones alone; and the share of the picture the drawn model takes.
pub fn iconbounds(assets: &mut Assets, wanted: &str) -> Result<()> {
    let db = trp_data::Database::load(&assets.install)?;
    let item = db.item(wanted)?;
    let key = item.prefab_key().ok_or_else(|| anyhow::anyhow!("no prefab"))?.to_owned();
    let files = assets.load(&key)?;
    let (cells_x, cells_y) = (item.props.get("Width").and_then(|v| v.as_u64()).unwrap_or(1), item.props.get("Height").and_then(|v| v.as_u64()).unwrap_or(1));
    let aspect = (cells_x * 63 + 1) as f32 / (cells_y * 63 + 1) as f32;
    let Some((object, pose)) = trp_data::interface::icon::pose_in(&assets, &files)? else { bail!("no PreviewPivot") };
    let tree = trp_data::unity::extract::read_hierarchy(&assets, object)?;
    println!("{} {}: {cells_x} x {cells_y} cells (aspect {aspect:.3}), {pose:?}", item.id, item.name);
    let rotate = |q: [f32; 4], v: [f32; 3]| -> [f32; 3] {
        let (u, s) = ([q[0], q[1], q[2]], q[3]);
        let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        let cross = |a: [f32; 3], b: [f32; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
        let (uv, uu, c) = (dot(u, v), dot(u, u), cross(u, v));
        std::array::from_fn(|i| 2.0 * uv * u[i] + (s * s - uu) * v[i] + 2.0 * s * c[i])
    };
    let qmul = |a: [f32; 4], b: [f32; 4]| -> [f32; 4] {
        [
            a[3] * b[0] + a[0] * b[3] + a[1] * b[2] - a[2] * b[1],
            a[3] * b[1] - a[0] * b[2] + a[1] * b[3] + a[2] * b[0],
            a[3] * b[2] + a[0] * b[1] - a[1] * b[0] + a[2] * b[3],
            a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2],
        ]
    };
    // Each node's place, turn and scale in the world: the root at
    // the pose's turn (its own replaced), the rest under it. The
    // turn normalised as the app's (`convert::rotation`), one of no
    // length none (the 0.2 BTC's, saved 0, 0, 0, 0).
    let length = pose.rotation.iter().map(|c| c * c).sum::<f32>().sqrt();
    let pose_turn = if length.is_finite() && length > f32::MIN_POSITIVE { pose.rotation.map(|c| c / length) } else { [0.0, 0.0, 0.0, 1.0] };
    let mut world: Vec<([f32; 3], [f32; 4], [f32; 3])> = Vec::new();
    let mut on: Vec<bool> = Vec::new();
    for (index, node) in tree.nodes.iter().enumerate() {
        let (at, turn, scale, up) = match node.parent {
            None => ([0.0; 3], pose_turn, node.scale, true),
            Some(parent) => {
                let (pt, pq, ps) = world[parent];
                let local = [node.translation[0] * ps[0], node.translation[1] * ps[1], node.translation[2] * ps[2]];
                let moved = rotate(pq, local);
                ([pt[0] + moved[0], pt[1] + moved[1], pt[2] + moved[2]], qmul(pq, node.rotation), [ps[0] * node.scale[0], ps[1] * node.scale[1], ps[2] * node.scale[2]], on[parent])
            }
        };
        world.push((at, turn, scale));
        on.push(up && (node.active || index == 0));
    }
    let mut all = ([f32::MAX; 3], [f32::MIN; 3]);
    let mut drawn = all;
    let grow = |into: &mut ([f32; 3], [f32; 3]), b: &([f32; 3], [f32; 3])| {
        for i in 0..3 {
            into.0[i] = into.0[i].min(b.0[i]);
            into.1[i] = into.1[i].max(b.1[i]);
        }
    };
    for (index, node) in tree.nodes.iter().enumerate() {
        let (mesh, kind) = match (&node.renderer, node.bounded) {
            (Some(renderer), _) if renderer.shadows_only => (renderer.mesh, "casts its shadow alone"),
            (Some(renderer), _) if renderer.skinned => (renderer.mesh, "skinned (its mesh's box, not its own)"),
            (Some(renderer), _) => (renderer.mesh, "drawn"),
            (None, Some(mesh)) => (mesh, "a lower LOD's"),
            (None, None) => continue,
        };
        let value = assets.read(mesh)?;
        let aabb = value.req("m_LocalAABB")?;
        let (center, extent) = (aabb.req("m_Center")?.as_vec3().unwrap_or_default(), aabb.req("m_Extent")?.as_vec3().unwrap_or_default());
        let vertices = trp_data::unity::extract::read_mesh(&assets, mesh).map(|m| {
            let mut b = ([f32::MAX; 3], [f32::MIN; 3]);
            for p in &m.positions {
                for i in 0..3 {
                    b.0[i] = b.0[i].min(p[i]);
                    b.1[i] = b.1[i].max(p[i]);
                }
            }
            b
        });
        let (at, turn, scale) = world[index];
        let mut boxed = ([f32::MAX; 3], [f32::MIN; 3]);
        for corner in 0..8 {
            let c: [f32; 3] = std::array::from_fn(|i| (center[i] + if corner >> i & 1 == 0 { -extent[i] } else { extent[i] }) * scale[i]);
            let p = rotate(turn, c);
            let p = [p[0] + at[0], p[1] + at[1], p[2] + at[2]];
            grow(&mut boxed, &(p, p));
        }
        let size: [f32; 3] = std::array::from_fn(|i| boxed.1[i] - boxed.0[i]);
        println!(
            "  {} ({kind}{}): mesh box centre {center:?} half {extent:?}; vertices {:?}; in the studio {:.3} x {:.3} (depth {:.3})",
            node.name,
            if on[index] { "" } else { ", under an object that is off" },
            vertices.as_ref().map(|b| (b.0, b.1)).map_err(|e| e.to_string()),
            size[0],
            size[1],
            size[2],
        );
        if on[index] && !node.name.contains("linza") && node.name != "MuzzleJetCombinedMesh" {
            grow(&mut all, &boxed);
            if kind == "drawn" {
                grow(&mut drawn, &boxed);
            }
        }
    }
    let half = |b: &([f32; 3], [f32; 3])| [(b.1[0] - b.0[0]) / 2.0, (b.1[1] - b.0[1]) / 2.0];
    let camera = |h: [f32; 2]| if h[0] / h[1] > aspect { h[0] / aspect } else { h[1] } / pose.bounds_scale;
    let (h_all, h_drawn) = (half(&all), half(&drawn));
    let (c_all, c_drawn) = (camera(h_all), camera(h_drawn));
    println!("  every renderer on: half {:.4} x {:.4}, the camera's half height {c_all:.4}", h_all[0], h_all[1]);
    println!("  the drawn alone:   half {:.4} x {:.4}, the camera's half height {c_drawn:.4}", h_drawn[0], h_drawn[1]);
    println!(
        "  the drawn model's box takes {:.3} of the picture's width and {:.3} of its height by every renderer's, {:.3} and {:.3} by its own",
        h_drawn[0] / (c_all * aspect),
        h_drawn[1] / c_all,
        h_drawn[0] / (c_drawn * aspect),
        h_drawn[1] / c_drawn
    );
    Ok(())
}

pub fn item(assets: &mut Assets, wanted: &str) -> Result<()> {
    let db = trp_data::Database::load(&assets.install)?;
    let mut found: Vec<&trp_data::setup::database::ItemTemplate> =
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
    Ok(())
}
