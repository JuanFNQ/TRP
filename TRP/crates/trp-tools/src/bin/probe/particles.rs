//! Particle systems and every weapon's muzzle effects.

use anyhow::{bail, Result};
use trp_data::unity::{class, Assets, ObjRef};

// A particle system and every one below it: each one's object, the
// modules it has on, its shape's kind, its bursts, how its renderer
// draws and with what shader (particles spec). `probe particles
// <bundle> <the system's id>`.
pub fn particles(assets: &mut Assets, args: &[String], key: &str) -> Result<()> {
    let arg = |i: usize| args.get(i).map(String::as_str);
    let id: i64 = arg(2).ok_or_else(|| anyhow::anyhow!("a system's id needed"))?.parse()?;
    assets.load("shaders")?;
    let files = assets.load(key)?;
    let Some(&file) = files.iter().find(|&&f| assets.info(ObjRef { file: f, path_id: id }).is_some()) else { bail!("no object {id} in {key}") };
    let system = ObjRef { file, path_id: id };
    let game_object_of = |assets: &Assets, obj: ObjRef| assets.read(obj).ok()?.get("m_GameObject")?.as_pptr().and_then(|p| assets.resolve_loaded(obj.file, p));
    let components_of = |assets: &Assets, go: ObjRef| -> Vec<ObjRef> {
        assets.read(go).ok().and_then(|g| g.get("m_Component")?.as_array().map(|all| all.iter().filter_map(|pair| pair.get("component")?.as_pptr()).filter_map(|p| assets.resolve_loaded(go.file, p)).collect())).unwrap_or_default()
    };
    let mut stack = vec![(game_object_of(&assets, system).ok_or_else(|| anyhow::anyhow!("no game object"))?, 0usize)];
    while let Some((go, depth)) = stack.pop() {
        let name = assets.read(go)?.get("m_Name").and_then(|n| n.as_str()).unwrap_or("").to_owned();
        let components = components_of(&assets, go);
        let mut line = format!("{}{name}", "  ".repeat(depth));
        for &c in &components {
            match assets.info(c).map(|i| i.class_id) {
                Some(class::PARTICLE_SYSTEM) => {
                    let s = assets.read(c)?;
                    let on: Vec<&str> = s.entries().filter(|(k, v)| k.ends_with("Module") && v.get("enabled").and_then(|e| e.as_bool()) == Some(true)).map(|(k, _)| k.trim_end_matches("Module")).collect();
                    let shape = s.get("ShapeModule").filter(|m| m.get("enabled").and_then(|e| e.as_bool()) == Some(true)).and_then(|m| m.get("type")).and_then(|t| t.as_i64());
                    let bursts = s.at("EmissionModule.m_Bursts").and_then(|b| b.as_array()).map_or(0, |b| b.len());
                    let rate = s.at("EmissionModule.rateOverTime.scalar").and_then(|r| r.as_f32()).unwrap_or(0.0);
                    line += &format!(" [{}: {} space, shape {shape:?}, {bursts} bursts, rate {rate}, {} s{}]", c.path_id, ["local", "world", "custom"].get(s.get("moveWithTransform").and_then(|m| m.as_i64()).unwrap_or(0) as usize).unwrap_or(&"?"), s.get("lengthInSec").and_then(|l| l.as_f32()).unwrap_or(0.0), if s.get("looping").and_then(|l| l.as_bool()) == Some(true) { " looping" } else { "" });
                    line += &format!(" {}", on.join(" "));
                }
                Some(class::PARTICLE_SYSTEM_RENDERER) => {
                    let r = assets.read(c)?;
                    let shader = r
                        .get("m_Materials")
                        .and_then(|m| m.as_array())
                        .and_then(|all| all.first())
                        .and_then(|m| m.as_pptr())
                        .and_then(|p| assets.resolve_loaded(c.file, p))
                        .and_then(|m| assets.read(m).ok()?.get("m_Shader")?.as_pptr().and_then(|p| assets.resolve_loaded(m.file, p)))
                        .and_then(|s| assets.read(s).ok()?.get("m_ParsedForm")?.get("m_Name")?.as_str().map(str::to_owned))
                        .unwrap_or_default();
                    line += &format!(" | drawn {} with {shader}", r.get("m_RenderMode").and_then(|m| m.as_i64()).unwrap_or(-1));
                }
                Some(class::MONO_BEHAVIOUR) => {
                    if let Some(script) = trp_data::unity::extract::script_class(&assets, c) {
                        line += &format!(" <{script}>");
                    }
                }
                _ => {}
            }
        }
        println!("{line}");
        // Its transform's children, in their order.
        if let Some(&transform) = components.first() {
            let children: Vec<ObjRef> = assets.read(transform)?.get("m_Children").and_then(|c| c.as_array()).map(|all| all.iter().filter_map(|c| c.as_pptr()).filter_map(|p| assets.resolve_loaded(transform.file, p)).collect()).unwrap_or_default();
            for child in children.into_iter().rev() {
                if let Some(child_go) = game_object_of(&assets, child) {
                    stack.push((child_go, depth + 1));
                }
            }
        }
    }
    Ok(())
}

// Every weapon's muzzle effects as the game chooses them (the
// deepest `MuzzleEffect`s and `extractor_smoke`): each object's
// path, its jets and smokes, and where it is from the fireport in
// the weapon bone's axes, in centimetres. `probe muzzles [part of
// a caliber's name]`.
pub fn muzzles(assets: &mut Assets, wanted: Option<&str>) -> Result<()> {
    let db = trp_data::Database::load(&assets.install)?;
    let mut all: Vec<_> = db
        .items
        .values()
        .filter(|item| item.name.starts_with("weapon_") && db.is_a(item, "Weapon") && db.default_preset(&item.id).is_ok())
        .filter(|item| wanted.is_none_or(|w| item.props.get("ammoCaliber").and_then(|c| c.as_str()).is_some_and(|c| c.contains(w))))
        .map(|item| (item.name.clone(), item.id.clone(), item.props.get("ammoCaliber").and_then(|c| c.as_str()).unwrap_or("").to_owned()))
        .collect();
    all.sort();
    let rotate = |q: [f32; 4], v: [f32; 3]| {
        let u = [q[0], q[1], q[2]];
        let cross = |a: [f32; 3], b: [f32; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
        let t = cross(u, v).map(|c| 2.0 * c);
        let c = cross(u, t);
        [v[0] + q[3] * t[0] + c[0], v[1] + q[3] * t[1] + c[1], v[2] + q[3] * t[2] + c[2]]
    };
    let back = |q: [f32; 4]| [-q[0], -q[1], -q[2], q[3]];
    for (name, id, caliber) in all {
        let weapon = match trp_data::weapons::weapon::assemble(&mut *assets, &db, &id) {
            Ok(weapon) => weapon,
            Err(e) => {
                println!("{name} {caliber}: not assembled: {e:#}");
                continue;
            }
        };
        let h = &weapon.hierarchy;
        let from_root = |mut node: usize| {
            let (mut place, mut turn) = ([0.0f32; 3], [0.0f32, 0.0, 0.0, 1.0]);
            loop {
                let n = &h.nodes[node];
                let scaled = [place[0] * n.scale[0], place[1] * n.scale[1], place[2] * n.scale[2]];
                let turned = rotate(n.rotation, scaled);
                place = [turned[0] + n.translation[0], turned[1] + n.translation[1], turned[2] + n.translation[2]];
                turn = trp_data::unity::extract::quat_mul(n.rotation, turn);
                match n.parent {
                    Some(up) => node = up,
                    None => return (place, turn),
                }
            }
        };
        let bone = (0..h.nodes.len()).find(|&i| h.path(i) == trp_data::weapons::custom::WEAPON_BONE);
        let fireport = (0..h.nodes.len()).find(|&i| h.nodes[i].name == "fireport");
        let in_weapon = |node: usize| {
            let (at, turn) = from_root(node);
            let (bone_at, bone_turn) = bone.map(from_root).unwrap_or(([0.0; 3], [0.0, 0.0, 0.0, 1.0]));
            let fire_at = fireport.map(|f| from_root(f).0).unwrap_or(bone_at);
            (rotate(back(bone_turn), [at[0] - fire_at[0], at[1] - fire_at[1], at[2] - fire_at[2]]).map(|c| c * 100.0), trp_data::unity::extract::quat_mul(back(bone_turn), turn))
        };
        let muzzle = weapon.muzzle.as_ref();
        println!("{name} {id} {caliber}: fireport {}", fireport.map_or("none".to_owned(), |f| h.path(f)));
        for node in trp_data::weapons::muzzle::effect_objects(h) {
            let jets = muzzle.map_or(0, |m| m.jets.iter().filter(|jet| jet.node == node).count());
            let smokes = muzzle.map_or(0, |m| m.smokes.iter().filter(|smoke| smoke.node == node).count());
            let (at, turn) = in_weapon(node);
            println!(
                "  {} ({}): {jets} jets, {smokes} smokes, at {:.2} {:.2} {:.2} from the fireport, turned {:.3} {:.3} {:.3} {:.3}",
                h.path(node),
                h.nodes[node].scripts.join(" "),
                at[0],
                at[1],
                at[2],
                turn[0],
                turn[1],
                turn[2],
                turn[3]
            );
        }
        // A `MuzzleLight` of the weapon's own (`MuzzleManager.UpdateLights`
        // makes one at the fireport where none is found).
        for node in 0..h.nodes.len() {
            let n = &h.nodes[node];
            for (_, &obj) in n.scripts.iter().zip(&n.script_objects).filter(|(name, _)| name.as_str() == "MuzzleLight") {
                println!("  muzzle light at {}: {:?}", h.path(node), assets.read(obj).map(|v| format!("{v:?}")));
            }
        }
    }
    Ok(())
}
