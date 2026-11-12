//! Animator controllers: states, parameters, the parameters' use and root motion.

use std::collections::BTreeMap;

use anyhow::Result;
use trp_data::unity::{class, Assets};

use crate::printing::describe_transition;

// Where every weapon's animator uses a parameter (`paramuse
// UniqueAnimationModId`): the weapons by their controller, then each
// transition, any-state transition, entry or exit choice whose
// conditions name it (with all their conditions), each blend tree
// and state speed that goes by it. A word after it narrows the
// weapons to those whose name has it.
pub fn paramuse(assets: &mut Assets, args: &[String], wanted: &str) -> Result<()> {
    let arg = |i: usize| args.get(i).map(String::as_str);
    let db = trp_data::Database::load(&assets.install)?;
    let only = arg(2).map(str::to_lowercase);
    let mut weapons: Vec<(&str, &str)> = db.items.values().filter(|i| db.is_a(i, "Weapon") && i.prefab_key().is_some()).map(|i| (i.name.as_str(), i.id.as_str())).collect();
    weapons.retain(|(name, _)| only.as_ref().is_none_or(|w| name.to_lowercase().contains(w)));
    weapons.sort();
    let mut groups: BTreeMap<String, (Vec<String>, Vec<String>)> = BTreeMap::new();
    let mut fresh = Assets::new(assets.install.clone())?;
    for (n, (name, id)) in weapons.iter().enumerate() {
        if n % 40 == 39 {
            fresh = Assets::new(assets.install.clone())?;
        }
        let Ok(weapon) = trp_data::weapons::weapon::assemble(&mut fresh, &db, id) else { continue };
        let Ok(table) = trp_data::weapons::states::StateTable::load(&fresh, &weapon) else { continue };
        let Some(param) = table.parameters.iter().find(|p| p.name == *wanted) else {
            groups.entry("(no such parameter)".into()).or_default().0.push(name.to_string());
            continue;
        };
        let conds = |c: &[trp_data::weapons::states::Condition]| c.iter().map(|c| format!("{} {} {}", c.parameter, match c.mode { 1 => "true", 2 => "false", 3 => ">", 4 => "<", 6 => "==", 7 => "!=", _ => "?" }, c.threshold)).collect::<Vec<_>>().join(" & ");
        let names = |c: &[trp_data::weapons::states::Condition]| c.iter().any(|c| c.parameter == *wanted);
        let mut uses = vec![format!("parameter {:?} starting at {}", param.kind, param.default)];
        for state in &table.states {
            for t in state.transitions.iter().filter(|t| names(&t.conditions)) {
                uses.push(format!("{} -> {} when {}", state.full_name, t.to.as_deref().unwrap_or("(selector)"), conds(&t.conditions)));
            }
            if state.speed_parameter.as_deref() == Some(wanted) {
                uses.push(format!("{} speed by it", state.full_name));
            }
            if state.blend_parameter.as_deref() == Some(wanted) {
                uses.push(format!("{} blends by it at {:?}: {}", state.full_name, state.thresholds, state.clips.iter().map(|c| c.0.as_str()).collect::<Vec<_>>().join(", ")));
            }
            for (k, tree) in state.trees.iter().enumerate() {
                for node in tree.iter().filter(|t| t.parameter_x.as_deref() == Some(wanted) || t.parameter_y.as_deref() == Some(wanted)) {
                    let kids: Vec<String> = node.children.iter().map(|&c| tree[c].clip.as_ref().map_or("(tree)".to_owned(), |c| c.0.clone())).collect();
                    uses.push(format!("{} tree {k} by it at {:?}: {}", state.full_name, node.thresholds, kids.join(", ")));
                }
            }
        }
        for (machine, t) in table.any_state.iter().filter(|(_, t)| names(&t.conditions)) {
            uses.push(format!("any state of machine {machine} -> {} when {}", t.to.as_deref().unwrap_or("(selector)"), conds(&t.conditions)));
        }
        for (machine, selectors) in table.selectors.iter().enumerate() {
            for (k, s) in selectors.iter().enumerate() {
                for choice in s.choices.iter().filter(|c| names(&c.conditions)) {
                    uses.push(format!("machine {machine} {} {k} -> {} when {}", if s.entry { "entry" } else { "exit" }, choice.to.clone().unwrap_or_else(|| format!("selector {:?}", choice.to_selector)), conds(&choice.conditions)));
                }
            }
        }
        let key = uses.join("\n  ");
        groups.entry(key).or_default().0.push(name.to_string());
    }
    for (uses, (names, _)) in &groups {
        println!("{} weapons: {}\n  {uses}\n", names.len(), names.join(", "));
    }
    Ok(())
}

pub fn states(assets: &mut Assets, args: &[String], template: &str) -> Result<()> {
    let arg = |i: usize| args.get(i).map(String::as_str);
    // Every animator state of a weapon with its clip and events. A
    // melee weapon, which has no preset, is its item's own prefab.
    let db = trp_data::Database::load(&assets.install)?;
    let weapon = match trp_data::weapons::weapon::assemble_named(&mut *assets, &db, template) {
        Ok(weapon) => weapon,
        Err(error) => match db.items.get(template).and_then(|item| item.props["Prefab"]["path"].as_str()).filter(|path| !path.is_empty()) {
            Some(path) => trp_data::weapons::weapon::assemble_prefab(&mut *assets, &path.to_owned())?,
            None => return Err(error),
        },
    };
    let table = trp_data::weapons::states::StateTable::load(&assets, &weapon)?;
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
                format!("{name} {:.3} s", trp_data::weapons::states::clip_length(&assets, *obj).unwrap_or(-1.0))
            })
            .collect::<Vec<_>>()
            .join(", ");
        // With its short name's hash as a recording keeps it (24 bits).
        let own = state.full_name.rsplit('.').next().unwrap_or(&state.full_name);
        println!(
            "[{}] {} #{}  speed {}{}  {clip}{}",
            state.machine,
            state.full_name,
            trp_data::unity::anim::path_hash(own) & 0x00ff_ffff,
            state.speed,
            if state.looping { " loop" } else { "" },
            match &state.blend_parameter {
                Some(p) => format!("  by {p} at {:?}", state.thresholds),
                None => String::new(),
            }
        );
        // Its scripts that set the third layer's weight.
        for w in &state.layer_weights {
            let keys: Vec<String> = w.curve.keys.iter().map(|k| format!("{:.3}: {:.2}", k.time, k.value)).collect();
            println!("      the third layer's weight: {} entering, {} leaving, between [{}] (wrap {:?} / {:?})", w.start, w.end, keys.join(", "), w.before, w.after);
        }
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
    Ok(())
}

pub fn controller(assets: &mut Assets, args: &[String], template: &str) -> Result<()> {
    let arg = |i: usize| args.get(i).map(String::as_str);
    // The weapon's animator controller, as stored: states, the
    // table that names them, and the scripts attached to states.
    let db = trp_data::Database::load(&assets.install)?;
    let weapon = trp_data::weapons::weapon::assemble_named(&mut *assets, &db, template)?;
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
    Ok(())
}

pub fn rootmotion(assets: &mut Assets, args: &[String], filter: Option<&str>) -> Result<()> {
    let arg = |i: usize| args.get(i).map(String::as_str);
    use trp_data::character::rootmotion::{Node, RootMotionTable};
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
                let layout = if *blend == trp_data::character::rootmotion::BlendType::Simple1D {
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
    let table = RootMotionTable::load(&mut *assets)?;
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
    Ok(())
}

// Each Animator of a bundle: the object it is on and the controller
// it names, through any override to the controller under it.
pub fn animator(assets: &mut Assets, key: &str) -> Result<()> {
    for file in assets.load(key)? {
        for obj in assets.objects_of(file, class::ANIMATOR).collect::<Vec<_>>() {
            let v = assets.read(obj)?;
            let owner = v.get("m_GameObject").and_then(|g| g.as_pptr()).and_then(|p| assets.resolve_loaded(file, p)).and_then(|go| assets.name_of(go)).unwrap_or_default();
            let mut controller = v.get("m_Controller").and_then(|p| p.as_pptr()).and_then(|p| assets.resolve_loaded(file, p));
            let mut chain = Vec::new();
            while let Some(at) = controller {
                chain.push(format!("{} (file {}, {})", assets.name_of(at).unwrap_or_default(), at.file, at.path_id));
                controller = assets.read(at).ok().and_then(|c| c.get("m_Controller").and_then(|p| p.as_pptr()).and_then(|p| assets.resolve_loaded(at.file, p)));
            }
            println!("animator on {owner}: {}", if chain.is_empty() { "no controller".to_owned() } else { chain.join(" over ") });
        }
    }
    Ok(())
}
