//! The body animator's states and clips, outfits' fittings and skins.

use std::collections::BTreeMap;

use anyhow::Result;
use trp_data::unity::{class, Assets};

use crate::printing::describe_transition;

// A piece of the character's clothing (a body's, a legs', a head's
// or the arms' bundle): the bones its mesh hangs on, each where it
// is at rest in the mesh's space.
pub fn skin(assets: &mut Assets, key: &str) -> Result<()> {
    let part = trp_data::character::arms::load(&mut *assets, key)?;
    let mesh = trp_data::unity::extract::read_mesh(&assets, part.mesh)?;
    println!("{key}: {} vertices, {} bones", mesh.positions.len(), part.bone_paths.len());
    for (path, bind) in part.bone_paths.iter().zip(&mesh.bind_poses) {
        match trp_data::character::outfit::rest_place(bind) {
            Some(at) => println!("  {:7.3} {:7.3} {:7.3}  {path}", at[0], at[1], at[2]),
            None => println!("  (no place)  {path}"),
        }
    }
    Ok(())
}

pub fn bodystates(assets: &mut Assets, args: &[String], filter: Option<&str>) -> Result<()> {
    let arg = |i: usize| args.get(i).map(String::as_str);
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
            let table = trp_data::weapons::states::StateTable::from_controller(&assets, obj, &Default::default())?;
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
                // The script's own curve by the state's time
                // (`SmoothLerpAnimCurve`), each key with its slopes.
                if let Some(curve) = state.movement.as_ref().and_then(|m| m.curve.as_ref()) {
                    let keys: Vec<String> = curve
                        .keys
                        .iter()
                        .map(|k| format!("{:.4}: {:.4} (in {:.4}, out {:.4})", k.time, k.value, k.in_slope, k.out_slope))
                        .collect();
                    println!("      curve {}", keys.join(", "));
                }
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
    Ok(())
}

pub fn bodyclips(assets: &mut Assets, args: &[String], filter: &str) -> Result<()> {
    let arg = |i: usize| args.get(i).map(String::as_str);
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
    Ok(())
}

// The head things' fittings to one another (`fittings [all]`; body
// animator spec, "The head's things fitted to one another"): every
// equipment bundle's `CustomItem` and `CustomItemSkin` scripts and
// what the app reads of them as worn, and every body part's
// `ClippingRuleChanger`. `all` lists each bundle.
pub fn fittings(assets: &mut Assets, which: Option<&str>) -> Result<()> {
    let (bundles, parts, missing) = trp_data::character::dress::survey(&assets.install)?;
    let all = which == Some("all");
    let name = |kind: u8| match kind {
        1 => "face cover",
        2 => "visor",
        4 => "hat",
        8 => "headphones",
        16 => "helmet",
        _ => "?",
    };
    let (mut plain, mut skin, mut read, mut kinds) = (0, 0, 0, BTreeMap::new());
    let mut odd = Vec::new();
    for bundle in &bundles {
        plain += usize::from(bundle.plain_scripts > 0);
        skin += usize::from(bundle.skin_scripts > 0);
        read += usize::from(!bundle.read.is_empty());
        if (bundle.plain_scripts > 0 || bundle.skin_scripts > 0) && bundle.read.iter().all(|f| !f.drawn) {
            odd.push(format!("{}: {} plain, {} skinned scripts, none read as drawn", bundle.key, bundle.plain_scripts, bundle.skin_scripts));
        }
        for fitting in &bundle.read {
            if fitting.drawn {
                *kinds.entry(name(fitting.kind)).or_insert(0) += 1;
            }
            if fitting.submeshes[0] != fitting.submeshes[1] || fitting.bind_poses[0] != fitting.bind_poses[1] {
                odd.push(format!("{} {}: submeshes {:?}, bind poses {:?}", bundle.key, fitting.object, fitting.submeshes, fitting.bind_poses));
            }
        }
        if all && !bundle.read.is_empty() {
            let listed: Vec<String> = bundle.read.iter().map(|f| format!("{} {}{} ({:?} submeshes)", name(f.kind), f.object, if f.drawn { "" } else { ", not drawn" }, f.submeshes)).collect();
            println!("{}: {}", bundle.key, listed.join("; "));
        }
    }
    println!("{} equipment bundles: {} with CustomItem, {} with CustomItemSkin ({} with either); {} read as worn; drawn fittings by kind {kinds:?}", bundles.len(), plain, skin, bundles.iter().filter(|b| b.plain_scripts > 0 || b.skin_scripts > 0).count(), read);
    for line in &odd {
        println!("  odd: {line}");
    }
    let changers: Vec<&trp_data::character::dress::SurveyedPart> = parts.iter().filter(|part| part.any_changer).collect();
    let mut by_bits = BTreeMap::new();
    for part in &changers {
        *by_bits.entry(part.bits).or_insert(0) += 1;
        if all || part.bits == 0 {
            println!("  part {}: bits {}", part.key, part.bits);
        }
    }
    println!("{} body, feet and head bundles named by the customization list and in the install ({missing} more named, not there): {} hold a ClippingRuleChanger; on the part's own object, by bits {by_bits:?}", parts.len(), changers.len());
    Ok(())
}
