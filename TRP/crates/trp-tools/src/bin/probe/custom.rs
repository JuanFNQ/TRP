//! Custom weapons and a weapon drawn with a model over it.

use anyhow::{bail, Result};
use trp_data::unity::Assets;
use trp_data::formats::sketch::Drawn;

use crate::printing::window_of;

// One of the install's weapons at rest from its side and from
// above, and over it, half seen through, a model of one's own
// (a binary FBX file whose barrel runs along -z with y up), the
// two laid with their triggers together: `overlay <weapon> <fbx>
// <the fbx's trigger model> <out.png> [pixels a centimetre]
// [ahead behind above below]`. The crosses are the weapon's
// markers for the palms (left blue, right green) at rest.
pub fn overlay(assets: &mut Assets, args: &[String], template: &str) -> Result<()> {
    let arg = |i: usize| args.get(i).map(String::as_str);
    use trp_data::unity::extract::read_mesh;
    let (Some(file), Some(trigger_model), Some(out)) = (arg(2), arg(3), arg(4)) else { bail!("overlay <weapon> <fbx> <trigger model> <out.png>") };
    let scale = arg(5).and_then(|s| s.parse().ok()).unwrap_or(12.0);
    let db = trp_data::Database::load(&assets.install)?;
    let weapon = trp_data::weapons::weapon::assemble_named(&mut *assets, &db, template)?;
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
    let scene = trp_data::formats::fbx::read(&std::fs::read(file)?)?;
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
    return trp_data::formats::sketch::views(&triangles, &marks, &[], out, scale, window_of(&args, 6));
}

pub fn custom(assets: &mut Assets, which: Option<&str>) -> Result<()> {
    let db = trp_data::Database::load(&assets.install)?;
    for note in &db.custom_notes {
        println!("left out: {note}");
    }
    for (id, definition) in &db.custom {
        println!("{id} {} on the rig of {}", definition.name, db.item(&definition.donor)?.name);
    }
    let Some(id) = which else { return Ok(()) };
    let weapon = trp_data::weapons::weapon::assemble(&mut *assets, &db, id)?;
    let custom = weapon.custom.as_ref().ok_or_else(|| anyhow::anyhow!("{id} is one of the install's own"))?;
    println!(
        "{}: {:.3} kg loaded, ergonomics {}, {} cells wide, a magazine of {:?}, held closer {}",
        weapon.name, weapon.total_weight, weapon.total_ergonomics, weapon.cell_width, weapon.magazine_capacity, weapon.held_closer
    );
    // What the effects of a shot and a magazine's rounds find on it.
    let rounds = weapon.hierarchy.nodes.iter().filter(|node| node.name.starts_with("patron_")).count();
    println!(
        "  shell port {:?}, extraction {}, muzzle {}, rounds' points {rounds}",
        weapon.shell_port.map(|node| weapon.hierarchy.path(node)),
        weapon.extraction.is_some(),
        weapon.muzzle.as_ref().map_or("none".to_owned(), |m| format!("{} jets, {} smokes", m.jets.len(), m.smokes.len()))
    );
    // The magazine's mesh's bounds and its rounds' points, in the
    // axes of the weapon bone at rest (the part's), in centimetres.
    let h = &weapon.hierarchy;
    let rotate = |q: [f32; 4], v: [f32; 3]| {
        let u = [q[0], q[1], q[2]];
        let cross = |a: [f32; 3], b: [f32; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
        let t = cross(u, v).map(|c| 2.0 * c);
        let c = cross(u, t);
        [v[0] + q[3] * t[0] + c[0], v[1] + q[3] * t[1] + c[1], v[2] + q[3] * t[2] + c[2]]
    };
    let magazine = custom.parts.iter().find(|part| h.path(part.node).contains("/mod_magazine/"));
    for index in (0..h.nodes.len()).filter(|&index| h.nodes[index].name.starts_with("patron_")) {
        let mut at = [0.0f32; 3];
        let mut node = index;
        let mut inside = false;
        while let Some(parent) = h.nodes[node].parent {
            let n = &h.nodes[node];
            at = rotate(n.rotation, at);
            at = [at[0] + n.translation[0], at[1] + n.translation[1], at[2] + n.translation[2]];
            if magazine.is_some() && h.path(parent) == trp_data::weapons::custom::WEAPON_BONE {
                inside = true;
                break;
            }
            node = parent;
        }
        if inside {
            println!("    {} at {:.2} {:.2} {:.2} in the magazine's axes", h.nodes[index].name, at[0] * 100.0, at[1] * 100.0, at[2] * 100.0);
        } else {
            println!("    {}", h.path(index));
        }
    }
    if let Some(part) = magazine {
        let (mut low, mut high) = ([f32::MAX; 3], [f32::MIN; 3]);
        for p in &part.mesh.positions {
            (0..3).for_each(|k| (low[k], high[k]) = (low[k].min(p[k]), high[k].max(p[k])));
        }
        println!("    its mesh from {:.2} {:.2} {:.2} to {:.2} {:.2} {:.2}", low[0] * 100.0, low[1] * 100.0, low[2] * 100.0, high[0] * 100.0, high[1] * 100.0, high[2] * 100.0);
        // Its top, slab by slab: where its walls are at each height.
        // (The weapon bone's axes: x across, -y along the barrel,
        // z up.)
        for slab in 0..8 {
            let top = high[2] - slab as f32 * 0.005;
            let (mut l, mut r) = ([f32::MAX; 3], [f32::MIN; 3]);
            for p in part.mesh.positions.iter().filter(|p| p[2] <= top && p[2] > top - 0.005) {
                (0..3).for_each(|k| (l[k], r[k]) = (l[k].min(p[k]), r[k].max(p[k])));
            }
            if l[0] <= r[0] {
                println!("    {:.2} to {:.2} up: across {:.2} to {:.2}, along {:.2} to {:.2}", (top - 0.005) * 100.0, top * 100.0, l[0] * 100.0, r[0] * 100.0, l[1] * 100.0, r[1] * 100.0);
            }
        }
    }
    for part in &custom.parts {
        let triangles: usize = part.mesh.submeshes.iter().map(|s| s.len() / 3).sum();
        println!("  {} on {}: {} vertices, {triangles} triangles", part.mesh.name, weapon.hierarchy.path(weapon.hierarchy.nodes[part.node].parent.unwrap_or(0)), part.mesh.positions.len());
    }
    let started = std::time::Instant::now();
    // As read, before they are compressed to be drawn with: pixels.
    let read: Vec<_> = custom.materials.iter().map(trp_data::weapons::custom::pictures).collect();
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
    Ok(())
}
