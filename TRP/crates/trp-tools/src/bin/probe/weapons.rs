//! Weapons as assembled: presets, templates, grenades, knives, colliders, parts' states and binds.

use std::collections::BTreeMap;

use anyhow::Result;
use trp_data::unity::Assets;

use crate::printing::describe_transition;

// The sounds as read (`sound`): the interface's of the kinds given
// and the things' of a group, each written as a WAVE file to the
// folder given (to listen to), with its length.
// A part's own animator (`partstates <template>`): the controller of
// the `Animator` on its prefab, its parameters and states with their
// clips, each clip's length and its own Unity events (`m_Events`:
// a method called by name on the animator's object).
pub fn partstates(assets: &mut Assets, template: &str) -> Result<()> {
    let db = trp_data::Database::load(&assets.install)?;
    let item = db.items.get(template).ok_or_else(|| anyhow::anyhow!("no such template"))?;
    let key = item.prefab_key().ok_or_else(|| anyhow::anyhow!("no prefab"))?;
    let files = assets.load(key)?;
    for &file in &files {
        for animator in assets.objects_of(file, trp_data::unity::class::ANIMATOR).collect::<Vec<_>>() {
            let value = assets.read(animator)?;
            let Some(controller) = value.get("m_Controller").and_then(|v| v.as_pptr()).and_then(|p| assets.resolve_loaded(animator.file, p)) else {
                println!("an animator with no controller");
                continue;
            };
            let table = trp_data::weapons::states::StateTable::from_controller(&assets, controller, &std::collections::HashMap::new())?;
            for p in &table.parameters {
                println!("parameter {:?} {} = {}", p.kind, p.name, p.default);
            }
            for state in &table.states {
                println!("[{}] {}  speed {}{}", state.machine, state.full_name, state.speed, if state.looping { " loop" } else { "" });
                for e in &state.events {
                    println!("      mark {:.3} {}{}", e.time, e.name, if e.string_param.is_empty() { String::new() } else { format!(" \"{}\"", e.string_param) });
                }
                for (name, obj) in &state.clips {
                    println!("      clip {name} {:.3} s", trp_data::weapons::states::clip_length(&assets, *obj).unwrap_or(-1.0));
                    if let Ok(clip) = assets.read(*obj) {
                        for event in clip.get("m_Events").and_then(|v| v.as_array()).unwrap_or(&[]) {
                            println!(
                                "        event {:.3} {} {:?}",
                                event.get("time").and_then(|v| v.as_f32()).unwrap_or(-1.0),
                                event.get("functionName").and_then(|v| v.as_str()).unwrap_or("?"),
                                event.get("data").and_then(|v| v.as_str()).unwrap_or("")
                            );
                        }
                    }
                }
                for t in &state.transitions {
                    println!("      -> {}", describe_transition(t));
                }
            }
        }
    }
    Ok(())
}

// What a part's own clips move (`partbinds <template>`): each clip
// of the controller on its prefab, and each node its curves bind
// (by its path's hash) with the properties, a curve that binds no
// node of the prefab as `UNBOUND`; then the nodes named
// `*_procedural` and whether any clip binds one (mounting spec,
// "The bipod").
pub fn partbinds(assets: &mut Assets, template: &str) -> Result<()> {
    let db = trp_data::Database::load(&assets.install)?;
    let item = db.items.get(template).ok_or_else(|| anyhow::anyhow!("no such template"))?;
    let key = item.prefab_key().ok_or_else(|| anyhow::anyhow!("no prefab"))?.to_owned();
    let (tree, clips) = trp_data::weapons::weapon::part_clip_binds(&mut *assets, &key)?;
    println!("{template} {key}: {} nodes, {} clips", tree.nodes.len(), clips.len());
    let mut bound = std::collections::BTreeSet::new();
    for (name, binds) in &clips {
        let mut nodes: Vec<(String, Vec<String>)> = Vec::new();
        for bind in binds {
            let label = bind.node.map_or_else(|| format!("UNBOUND {:08x}", bind.path), |node| tree.nodes[node].name.clone());
            let what = bind.property.map_or_else(|| "component".to_owned(), |p| format!("{p:?}"));
            match nodes.iter_mut().find(|(n, _)| *n == label) {
                Some((_, props)) => props.push(what),
                None => nodes.push((label, vec![what])),
            }
            bound.extend(bind.node);
        }
        let listed: Vec<String> = nodes.iter().map(|(n, props)| format!("{n} [{}]", props.join(","))).collect();
        println!("  clip {name}: {}", listed.join("; "));
    }
    let procedural: Vec<usize> = (0..tree.nodes.len()).filter(|&i| tree.nodes[i].name.ends_with("_procedural")).collect();
    let names: Vec<&str> = procedural.iter().map(|&i| tree.nodes[i].name.as_str()).collect();
    let moved: Vec<&str> = procedural.iter().filter(|i| bound.contains(*i)).map(|&i| tree.nodes[i].name.as_str()).collect();
    println!("  _procedural nodes: {} {names:?}; bound by a clip: {moved:?}", names.len());
    Ok(())
}

pub fn preset(assets: &mut Assets, weapon: &str) -> Result<()> {
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
    Ok(())
}

pub fn weapon(assets: &mut Assets, args: &[String], template: &str) -> Result<()> {
    let arg = |i: usize| args.get(i).map(String::as_str);
    use trp_data::unity::extract::{read_material, read_mesh, read_texture};
    let db = trp_data::Database::load(&assets.install)?;
    // The materials' shaders are in the shaders' bundle.
    if std::env::var("TRP_PROBE_FIGURES").is_ok() {
        assets.load("shaders")?;
    }
    // A template or a name, or a build by its key (`5447...[mod_...]`).
    let weapon = if template.contains('[') { trp_data::weapons::weapon::assemble_key(&mut *assets, &db, template)? } else { trp_data::weapons::weapon::assemble_named(&mut *assets, &db, template)? };
    println!("{} with {} nodes", weapon.name, weapon.hierarchy.nodes.len());
    for s in &weapon.skipped {
        println!("  SKIPPED {s}");
    }
    // With a third word: every bone whose path has it, at rest.
    if let Some(filter) = arg(2) {
        for (i, node) in weapon.hierarchy.nodes.iter().enumerate() {
            let path = weapon.hierarchy.path(i);
            if path.contains(filter) {
                println!("  [{i}] {path}  at {:?} rot {:?} scale {:?} active {}", node.translation, node.rotation, node.scale, node.active);
                // Its scripts, by class.
                if !node.scripts.is_empty() {
                    println!("      scripts {:?}", node.scripts);
                }
                // A skinned mesh's bones, by their places in the tree.
                if let Some(renderer) = node.renderer.as_ref().filter(|renderer| renderer.skinned) {
                    println!("      on bones {:?}", renderer.bones);
                }
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
            "  [{i}] {}  mesh={} verts={} tris={} skinned={} (weights {}, joints {}, bind poses {}, bones {})",
            weapon.hierarchy.path(i),
            mesh.name,
            mesh.positions.len(),
            tris,
            r.skinned,
            mesh.weights.len(),
            mesh.joints.len(),
            mesh.bind_poses.len(),
            r.bones.len()
        );
        // Each submesh's topology and index count, as stored.
        if let Some(sub) = assets.read(r.mesh).ok().and_then(|value| value.get("m_SubMeshes").and_then(|subs| subs.as_array().map(|subs| subs.to_vec()))) {
            let told: Vec<String> = sub.iter().map(|sm| format!("{:?}/{:?}", sm.get("topology").and_then(|t| t.as_i64()), sm.get("indexCount").and_then(|c| c.as_i64()))).collect();
            println!("      submeshes (topology/indices): {}", told.join(" "));
        }
        for m in r.materials.iter().flatten() {
            let mat = read_material(&assets, *m)?;
            let mut textures: Vec<String> = Vec::new();
            for (slot, tex) in &mat.textures {
                let t = read_texture(&assets, *tex)?;
                textures.push(format!("{slot}={} {}x{} fmt{} mips{}", t.name, t.width, t.height, t.format, t.mip_count));
            }
            textures.sort();
            println!("        material {}: {}", mat.name, textures.join("; "));
            // With `TRP_PROBE_FIGURES` set, its floats and colours too
            // (what tells which shader it was drawn by).
            if std::env::var("TRP_PROBE_FIGURES").is_ok() {
                let mut figures: Vec<String> = mat.floats.iter().map(|(k, v)| format!("{k}={v}")).chain(mat.colors.iter().map(|(k, v)| format!("{k}={v:?}"))).collect();
                figures.sort();
                let value = assets.read(*m)?;
                let shader = value
                    .get("m_Shader")
                    .and_then(trp_data::unity::Value::as_pptr)
                    .and_then(|p| assets.resolve_loaded(m.file, p))
                    .and_then(|s| assets.read(s).ok()?.get("m_ParsedForm")?.get("m_Name")?.as_str().map(str::to_owned))
                    .unwrap_or_else(|| "?".into());
                let queue = value.get("m_CustomRenderQueue").and_then(trp_data::unity::Value::as_i64).unwrap_or(-1);
                println!("          shader {shader:?}, queue {queue}; {} {}", mat.keywords.join(" "), figures.join(" "));
            }
        }
    }
    Ok(())
}

pub fn weapons(assets: &mut Assets, filter: Option<&str>) -> Result<()> {
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
            let weapon = trp_data::weapons::weapon::assemble_named(&mut *assets, &db, &item.id)?;
            let table = trp_data::weapons::states::StateTable::load(&assets, &weapon)?;
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
    Ok(())
}

pub fn grenade(assets: &mut Assets, bundle: &str) -> Result<()> {
    // A grenade's prefab beyond the hands: the parts a throw lets go
    // of, and the grenade thrown with its body and colliders.
    let rig = trp_data::weapons::weapon::assemble_prefab(&mut *assets, bundle)?;
    let data = trp_data::weapons::grenade::GrenadeData::read(&assets, &rig)?;
    println!("throwing parts {:?}", data.throwing_parts);
    println!("offset {:?}, collision sound {}", data.offset, data.collision_sound);
    println!("body {:?}", data.body);
    for (index, node) in data.thrown.nodes.iter().enumerate() {
        println!("[{index}] {} under {:?} at {:?} turned {:?} scaled {:?}, drawn {}, colliders {:?}", node.name, node.parent, node.translation, node.rotation, node.scale, node.renderer.is_some(), node.colliders);
    }
    Ok(())
}

pub fn knives(assets: &mut Assets) -> Result<()> {
    // Every melee weapon's strokes as its own animator marks them
    // for the body (`ThirdAction`, which sets the body's
    // `FirstAction`): the slash (`FIRE 1`), the stab (`FIRE 2`) and
    // the quick stroke (`Q FIRE`), and the body's state each number
    // enters (grenades-and-knife spec, "The knife's strokes seen
            // from outside").
    let db = trp_data::Database::load(&assets.install)?;
    let body_state = |action: i32| match action {
        8 => "Fire_1_Knife (from INACTIVE) / quick_Fire_Knife 0 (from idle_to_out)",
        12 => "Alt_Fire_Knife",
        13 => "Fire_1_Axe",
        14 => "Alt_Fire_Axe",
        15 => "quick_Fire_Axe 0 (from idle_to_out)",
        _ => "-",
    };
    let mut knives: Vec<_> = db.items.values().filter(|item| item.node_type == "Item" && db.is_a(item, "Knife")).collect();
    knives.sort_by(|a, b| a.name.cmp(&b.name));
    let mut sets: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for item in knives {
        let Some(path) = item.props["Prefab"]["path"].as_str().filter(|path| !path.is_empty()).map(str::to_owned) else {
            println!("{} {}: no prefab", item.id, item.name);
            continue;
        };
        let table = match trp_data::weapons::weapon::assemble_prefab(&mut *assets, &path).and_then(|rig| trp_data::weapons::states::StateTable::load(&assets, &rig)) {
            Ok(table) => table,
            Err(error) => {
                println!("{} {}: {path}: {error:#}", item.id, item.name);
                continue;
            }
        };
        let mut said = Vec::new();
        for wanted in ["Hands.FIRE 1", "Hands.FIRE 2", "Hands.Q FIRE"] {
            let marks: Vec<String> = table
                .states
                .iter()
                .filter(|state| state.full_name == wanted)
                .flat_map(|state| state.events.iter().filter(|e| e.name == "ThirdAction").map(|e| format!("{} at {:.3}", e.int_param, e.time)))
                .collect();
            said.push(format!("{}: {}", wanted.trim_start_matches("Hands."), if marks.is_empty() { "-".to_owned() } else { marks.join(", ") }));
        }
        // Any other state that marks something for the body but the
        // draw (1), the put-away (2) and the look (20, 21).
        for state in table.states.iter().filter(|state| !["Hands.FIRE 1", "Hands.FIRE 2", "Hands.Q FIRE"].contains(&state.full_name.as_str())) {
            let marks: Vec<String> = state
                .events
                .iter()
                .filter(|e| e.name == "ThirdAction" && ![1, 2, 20, 21].contains(&e.int_param))
                .map(|e| format!("{} at {:.3}", e.int_param, e.time))
                .collect();
            if !marks.is_empty() {
                said.push(format!("{}: {}", state.full_name.trim_start_matches("Hands."), marks.join(", ")));
            }
        }
        let key = said.join("; ");
        println!("{} {} ({path}): {key}", item.id, item.name);
        sets.entry(key).or_default().push(item.name.clone());
    }
    println!();
    for (marks, names) in &sets {
        println!("{} melee weapons: {marks}\n  {}", names.len(), names.join(", "));
    }
    println!();
    for action in [8, 12, 13, 14, 15] {
        println!("FirstAction {action}: the body's {}", body_state(action));
    }
    Ok(())
}

// An item's model as it lies loose (`ObjectsFactory.CreateLootPrefab`:
// a weapon with its parts, any other item its prefab), and every
// collider on it: its object, shape, layer, whether a trigger or
// switched off, and its material.
pub fn colliders(assets: &mut Assets, template: &str) -> Result<()> {
    let db = trp_data::Database::load(&assets.install)?;
    let item = db.item(template)?;
    let hierarchy = if db.is_a(item, "Weapon") {
        trp_data::weapons::weapon::assemble(&mut *assets, &db, template)?.hierarchy
    } else {
        let key = item.prefab_key().ok_or_else(|| anyhow::anyhow!("{template} has no model"))?.to_owned();
        trp_data::interface::icon::model_of(&mut *assets, &key)?.0
    };
    println!("{} ({}): {} objects", item.name, template, hierarchy.nodes.len());
    for (index, node) in hierarchy.nodes.iter().enumerate() {
        for c in &node.colliders {
            println!("  {index:>3} {:<28} layer {:>2}{}{} {:?} {:?}", node.name, c.layer, if c.trigger { " trigger" } else { "" }, if c.enabled { "" } else { " off" }, c.shape, c.material);
        }
    }
    Ok(())
}
