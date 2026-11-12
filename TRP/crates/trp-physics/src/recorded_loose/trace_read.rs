//! Reading a recorded throw and the model's key, colliders and hierarchy.

use super::*;

/// The things set going in a trace of recorder 0.9.0 on, with their
/// frames.
pub fn read(path: &str) -> Result<Trace> {
    let text = std::fs::read_to_string(path).with_context(|| path.to_owned())?;
    let mut lines = text.lines();
    let header: Value = serde_json::from_str(lines.next().ok_or_else(|| anyhow!("{path} is empty"))?)?;
    let mut things: Vec<Thrown> = Vec::new();
    let mut by_id: HashMap<String, usize> = HashMap::new();
    for line in lines {
        if !line.contains("\"loose\":") {
            continue;
        }
        let v: Value = serde_json::from_str(line)?;
        let t = f(&v["t"]).unwrap_or(0.0);
        let dt = f(&v["dt"]).unwrap_or(0.0);
        let step = v["phys"].as_array().filter(|p| p.get(1).and_then(Value::as_bool) != Some(false)).and_then(|p| p.first()).and_then(f);
        for e in v["thrown"].as_array().into_iter().flatten() {
            let id = e["id"].as_u64().ok_or_else(|| anyhow!("a thing thrown with no id"))?;
            let start = Start {
                at: v3(&e["pos"]).unwrap_or([0.0; 3]),
                turn: q4(&e["rot"]).unwrap_or([0.0, 0.0, 0.0, 1.0]),
                velocity: v3(&e["vel"]).unwrap_or([0.0; 3]),
                spin: v3(&e["ang"]).unwrap_or([0.0; 3]),
            };
            let look = e["look"].as_array().and_then(|a| Some([f(a.first()?)?, f(a.get(1)?)?]));
            let ground = e["ground"].as_array().and_then(|g| Some((v3(g.first()?)?, v3(g.get(1)?)?, g.get(2)?.as_str()?.to_owned())));
            by_id.insert(id.to_string(), things.len());
            things.push(Thrown {
                id,
                template: e["tpl"].as_str().unwrap_or("").to_owned(),
                kg: f(&e["kg"]).unwrap_or(0.0),
                start,
                player_pos: v3(&e["player_pos"]),
                player_vel: v3(&e["player_vel"]),
                look,
                camera_rot: q4(&e["camera_rot"]),
                weapon_root_anim: q4(&v["weapon_root_anim_q"]),
                capsule: v3(&v["capsule"]),
                dt,
                ground,
                setup: e["setup"].clone(),
                frames: Vec::new(),
                taken_up: None,
            });
        }
        for (id, entry) in v["loose"].as_object().into_iter().flatten() {
            // Things set going before the recording began are left out.
            let Some(&index) = by_id.get(id) else { continue };
            let a = entry.as_array().ok_or_else(|| anyhow!("a loose thing is not a list"))?;
            let (Some(at), Some(turn)) = (a.first().and_then(v3), a.get(1).and_then(q4)) else { continue };
            let moving = match (a.get(2).and_then(v3), a.get(3).and_then(v3), a.get(4).and_then(Value::as_bool)) {
                (Some(velocity), Some(spin), Some(asleep)) => Some((velocity, spin, asleep)),
                _ => None,
            };
            things[index].frames.push(Frame { t, dt, step, at, turn, moving });
        }
    }
    // A thing taken up goes far off (the game's pool): its last frame is
    // not where it lay.
    for thing in &mut things {
        let n = thing.frames.len();
        if n >= 2 && distance(thing.frames[n - 1].at, thing.frames[n - 2].at) > 100.0 {
            let taken = thing.frames.pop().map(|f| f.t);
            thing.taken_up = taken.map(|t| t - thing.frames[0].t);
        }
    }
    Ok(Trace { header, things })
}

/// A weapon's build as the recording's equipment holds it (its parts, by
/// the slots they are in), for a weapon of that template: its key
/// (`trp_data::weapon::build_key`), or the template where the equipment
/// holds none.
pub fn model_key(header: &Value, db: &trp_data::Database, template: &str) -> String {
    use trp_data::setup::database::{Preset, PresetItem};
    let equipment: Vec<&Value> = header["equipment"].as_array().into_iter().flatten().collect();
    if !db.item(template).is_ok_and(|item| db.is_a(item, "Weapon")) {
        return template.to_owned();
    }
    let Some(root) = equipment.iter().find(|e| e["tpl"].as_str() == Some(template)) else { return template.to_owned() };
    let root_id = root["id"].as_str().unwrap_or("").to_owned();
    let mut items = vec![PresetItem { id: root_id.clone(), template: template.to_owned(), parent: None, slot: None }];
    let mut parents = vec![root_id.clone()];
    while let Some(parent) = parents.pop() {
        for e in equipment.iter().filter(|e| e["in"].as_str() == Some(parent.as_str())) {
            let (Some(id), Some(tpl), Some(slot)) = (e["id"].as_str(), e["tpl"].as_str(), e["at"].as_str()) else { continue };
            // Its parts, not what it holds (rounds).
            if !db.item(tpl).is_ok_and(|item| db.is_a(item, "Mod")) {
                continue;
            }
            items.push(PresetItem { id: id.to_owned(), template: tpl.to_owned(), parent: Some(parent.clone()), slot: Some(slot.to_owned()) });
            parents.push(id.to_owned());
        }
    }
    trp_data::weapons::weapon::build_key(&Preset { id: root_id.clone(), name: String::new(), root: root_id, items, encyclopedia: None })
}

/// A thing's colliders as it lies loose, each with its object's name,
/// from its model as the app reads it (`trp-app`'s `icons::model`): a
/// weapon with the parts of its build, its sights folded as made and its
/// fold open; anything else its prefab.
pub fn model_colliders(assets: &mut trp_data::unity::Assets, db: &trp_data::Database, key: &str) -> Result<Vec<(String, Collider)>> {
    let hierarchy = model_hierarchy(assets, db, key)?;
    let unity = &*assets;
    let mut points = |mesh| trp_data::unity::extract::read_mesh(unity, mesh).ok().filter(|m| !m.positions.is_empty()).map(|m| std::sync::Arc::from(m.positions));
    Ok(loose::colliders_named(&hierarchy, &mut points))
}

/// A thing's model as it lies loose, as the app reads it (`model_colliders`).
pub fn model_hierarchy(assets: &mut trp_data::unity::Assets, db: &trp_data::Database, key: &str) -> Result<trp_data::unity::extract::Hierarchy> {
    let template = key.split('[').next().unwrap_or(key);
    let item = db.item(template)?;
    let prefab = item.prefab_key().ok_or_else(|| anyhow!("{template} has no model"))?.to_owned();
    let weapon = db.is_a(item, "Weapon");
    let held = !weapon && trp_data::weapons::weapon::is_weapon_container(assets, &prefab)?;
    Ok(if weapon || held {
        let mut assembled = if weapon { trp_data::weapons::weapon::assemble_key(assets, db, key)? } else { trp_data::weapons::weapon::assemble_prefab(assets, &prefab)? };
        if weapon {
            trp_sim::weapons::sights::fold_sights_as_made(&mut assembled);
        }
        assembled.fold_tree(false);
        assembled.hierarchy
    } else {
        let mut hierarchy = trp_data::interface::icon::model_of(assets, &prefab)?.0;
        // A round as an unspent round, its case's objects off (the app's
        // `icons::round_model`, `loose::model_key`).
        if db.is_a(item, "Ammo") {
            unspent(&mut hierarchy);
        }
        hierarchy
    })
}

/// A round's model as an unspent round: its objects named `full` on, those
/// named `shell` off (`AmmoPoolObject.SetUsed(false)`).
pub(super) fn unspent(hierarchy: &mut trp_data::unity::extract::Hierarchy) {
    for node in &mut hierarchy.nodes {
        if node.name.contains("shell") {
            node.active = false;
        } else if node.name.contains("full") {
            node.active = true;
        }
    }
}

/// What a recorded collider is paired with ours by: the object it is on,
/// after the point it hangs on where it is a round's in a magazine or a
/// chamber (`patron_001/patron_762x39_fmj_full_LOD1`, as
/// `loose::rounds_colliders` names ours).
pub fn pairing(path: &str) -> String {
    let parts: Vec<&str> = path.split('/').collect();
    let point = parts.iter().rev().skip(1).find(|part| trp_sim::items::magazine_view::point_number(part).is_some() || part.starts_with("patron_in_weapon"));
    match point {
        Some(point) => format!("{point}/{}", parts.last().copied().unwrap_or("")),
        None => last(path).to_owned(),
    }
}
