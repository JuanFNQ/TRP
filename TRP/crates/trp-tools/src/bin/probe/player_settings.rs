//! The key bindings, the player's scripts, the engine, the hard settings and the ragdoll.

use anyhow::Result;
use trp_data::unity::Assets;

// The install's own key bindings (`Control.ini`), those whose name
// has that part: `keys Weapon`.
pub fn keys(assets: &mut Assets, filter: Option<&str>) -> Result<()> {
    let wanted = filter.unwrap_or("").to_lowercase();
    let words = |source: &trp_data::setup::install::KeySource| {
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
    Ok(())
}

// `player-script-values <class>`: each script of that class on the
// player's prefab, its fields whole as the type tree reads them
// (the hit reaction's curves, the full-body IK's settings).
pub fn player_script_values(assets: &mut Assets, wanted: &str) -> Result<()> {
    let files = assets.load(trp_data::character::player::PLAYER_BUNDLE)?;
    let objects: Vec<_> = files.iter().flat_map(|&f| assets.objects_of(f, trp_data::unity::class::MONO_BEHAVIOUR)).collect();
    for obj in objects {
        if trp_data::unity::extract::script_class(&assets, obj).as_deref() != Some(wanted) {
            continue;
        }
        println!("{:?}: {:#?}", obj, assets.read(obj)?);
    }
    Ok(())
}

pub fn player_scripts(assets: &mut Assets, wanted: &str) -> Result<()> {
    use trp_data::unity::{class, Value};
    let files = assets.load(trp_data::character::player::PLAYER_BUNDLE)?;
    let objects: Vec<_> = files.iter().flat_map(|&f| assets.objects_of(f, class::MONO_BEHAVIOUR)).collect();
    for obj in objects {
        if trp_data::unity::extract::script_class(&assets, obj).as_deref() != Some(wanted) {
            continue;
        }
        let v = assets.read(obj)?;
        // A scriptable object (a walk's preset) hangs from no game
        // object: its fields as they are.
        let Some(owner) = v.get("m_GameObject").and_then(Value::as_pptr).and_then(|p| assets.resolve_loaded(obj.file, p)) else {
            println!("{wanted} {} (no game object): {v:#?}", obj.path_id);
            continue;
        };
        let go = assets.read(owner)?;
        let name = go.get("m_Name").and_then(trp_data::unity::Value::as_str).unwrap_or("").to_owned();
        let layer = go.get("m_Layer").and_then(Value::as_i64).unwrap_or(0);
        let mut parent = String::new();
        let mut shapes = Vec::new();
        for c in go.get("m_Component").and_then(Value::as_array).unwrap_or(&[]) {
            let Some(c) = c.get("component").and_then(Value::as_pptr).and_then(|p| assets.resolve_loaded(owner.file, p)) else { continue };
            let class_id = assets.class_id(c);
            let value = assets.read(c)?;
            if class_id == Some(class::TRANSFORM) {
                let father = value.get("m_Father").and_then(Value::as_pptr).and_then(|p| assets.resolve_loaded(c.file, p));
                let pos = value.get("m_LocalPosition").and_then(Value::as_vec3);
                parent = father
                    .and_then(|f| assets.read(f).ok())
                    .and_then(|t| t.get("m_GameObject").and_then(Value::as_pptr).and_then(|p| assets.resolve_loaded(c.file, p)))
                    .and_then(|g| assets.read(g).ok())
                    .and_then(|g| g.get("m_Name").and_then(Value::as_str).map(str::to_owned))
                    .unwrap_or_default();
                parent = format!("{parent} at {pos:?}");
            } else if matches!(class_id, Some(65 | 135 | 136)) {
                let get = |n: &str| value.get(n).map(|v| v.as_vec3().map(|x| format!("{x:?}")).or(v.as_f32().map(|f| f.to_string())).unwrap_or_default()).unwrap_or_default();
                shapes.push(format!("class {} centre {} size {} radius {} height {} dir {} trigger {}", class_id.unwrap_or(0), get("m_Center"), get("m_Size"), get("m_Radius"), get("m_Height"), get("m_Direction"), get("m_IsTrigger")));
            }
        }
        let mut fields: Vec<String> = ["BodyPartType", "BodyPartColliderType", "ArmorPlateColliderType", "_typeOfMaterial", "_presetId", "HitType"].iter().filter_map(|f| v.get(f).and_then(Value::as_i64).map(|x| format!("{f} {x}"))).collect();
        // A ballistic collider's own figures (`BallisticCollider`).
        let figures: Vec<String> = ["PenetrationLevel", "PenetrationChance", "RicochetChance", "FragmentationChance", "TrajectoryDeviationChance", "TrajectoryDeviation", "penetrationDamageMod"].iter().filter_map(|f| v.get(f).and_then(Value::as_f32).map(|x| x.to_string())).collect();
        if !figures.is_empty() {
            fields.push(format!("figures [{}]", figures.join(", ")));
        }
        println!("{name} (layer {layer}) under {parent} {}: {}", fields.join(" "), shapes.join("; "));
    }
    Ok(())
}

pub fn ragdoll(assets: &mut Assets) -> Result<()> {
    let ragdoll = trp_data::character::ragdoll::RagdollData::load(&mut *assets)?;
    for body in &ragdoll.bodies {
        println!("body {}: mass {} drag {} angular drag {}", body.bone, body.mass, body.drag, body.angular_drag);
        for collider in &body.colliders {
            println!("    {} (part {:?}, surface {}) {:?} at {:?}", collider.object, collider.part, collider.surface, collider.shape, collider.local);
        }
    }
    for joint in &ragdoll.joints {
        println!(
            "joint {} to {}: anchor {:?} axis {:?} swing axis {:?} twist {}..{} swing {} {} springs {:?} {:?} collision {} projection {:?} auto anchor {} connected anchor {:?} preprocessing {}",
            joint.bone, joint.connected, joint.anchor, joint.axis, joint.swing_axis, joint.low_twist.limit, joint.high_twist.limit, joint.swing1.limit, joint.swing2.limit, joint.twist_spring, joint.swing_spring, joint.enable_collision, joint.projection, joint.auto_anchor, joint.connected_anchor, joint.preprocessing
        );
    }
    Ok(())
}

// A build's parts that glow with its heat (`trp_data::heat`).
pub fn hot(assets: &mut Assets, key: &str) -> Result<()> {
    let db = trp_data::Database::load(&assets.install)?;
    let weapon = trp_data::weapons::weapon::assemble_key(&mut *assets, &db, key)?;
    for part in &weapon.hot {
        let node = &weapon.hierarchy.nodes[part.node];
        println!("{} ({}): centre {:?} size {:?} visible {} skinned {:?}; {} materials given heat; glowing slots {:?}", node.name, part.node, part.center, part.size, part.visible, node.renderer.as_ref().map(|r| r.skinned), part.materials, part.slots);
        if let Some(haze) = &part.haze {
            println!("    shimmer: box centre {:?} extent {:?}, life delta {:?}, count {:?}, life {:?}, size {:?}", haze.center, haze.extent, haze.lifetime_delta, haze.count.keys, haze.life.keys, haze.size.keys);
        }
    }
    // The shared system the shimmer goes into.
    if let Ok(system) = trp_data::drawing::particles::muzzle_heat_system(&mut *assets) {
        println!("system {}: at most {}, world space {}, unbuilt {:?}; start life {:?}, size {:?}, rotation {:?}, colour {:?}", system.name, system.max_particles, system.world_space, system.unbuilt, system.start_lifetime, system.start_size, system.start_rotation, system.start_color);
        let m = &system.renderer.material;
        println!("  material {} ({}): distort {:?} blur {:?} inv fade {:?} movement {:?}; max size {}", m.name, m.shader, m.float("_Distort"), m.float("_Blur"), m.float("_InvFade"), m.color("_TextureMovement"), system.renderer.max_size);
    }
    Ok(())
}

pub fn engine(assets: &mut Assets, args: &[String]) -> Result<()> {
    let arg = |i: usize| args.get(i).map(String::as_str);
    println!("{:?}", trp_data::setup::engine::EngineSettings::load(&assets.install)?);
    let hands = trp_data::character::player::HandsData::load(&mut *assets)?;
    println!("turn-away: threshold {} m, against a character {} m", hands.turn_away.threshold, hands.turn_away.player_threshold);
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
    println!("the load's walk: {} entries in the normal preset", hands.walk_overweight.len());
    for entry in &hands.walk_overweight {
        let span = entry.curve.keys.last().map_or(0.0, |k| k.time);
        let peak = entry.curve.keys.iter().map(|k| k.value.abs()).fold(0.0f32, f32::max);
        println!("  curve over 0..{span} (peak {peak}) shift {} vals {:?}", entry.time_shift, entry.vals);
    }
    println!("mouse: {:?}", hands.mouse.vals);
    println!("movement: {:?}", hands.movement.vals);
    println!("camera offset {:?}, sway {} / {}", hands.camera_offset, hands.sway_frequency, hands.sway_damping);
    let db = trp_data::Database::load(&assets.install)?;
    let weapon = trp_data::weapons::weapon::assemble_named(&mut *assets, &db, arg(1).unwrap_or("59d6088586f774275f37482f"))?;
    println!(
        "{}: {:.3} kg, ergonomics {}, {} cells wide",
        weapon.name, weapon.total_weight, weapon.total_ergonomics, weapon.cell_width
    );
    Ok(())
}

pub fn hardsettings(assets: &mut Assets, filter: Option<&str>) -> Result<()> {
    let settings = trp_data::setup::hard_settings::HardSettings::load(&assets.install)?;
    let wanted = |name: &str| filter.map_or(true, |f| name.to_lowercase().contains(&f.to_lowercase()));
    for (name, value) in settings.numbers() {
        if wanted(name) {
            println!("{name} = {value}");
        }
    }
    for (name, [x, y]) in settings.pairs() {
        if wanted(name) {
            println!("{name} = ({x}, {y})");
        }
    }
    for (name, [x, y, z]) in settings.vectors() {
        if wanted(name) {
            println!("{name} = ({x}, {y}, {z})");
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
    Ok(())
}
