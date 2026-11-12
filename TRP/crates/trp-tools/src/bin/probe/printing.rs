//! Printing JSON, object trees, transitions and props' levels of detail.

use anyhow::Result;
use trp_data::unity::{class, Assets, ObjRef};

pub(super) fn describe_transition(t: &trp_data::weapons::states::Transition) -> String {
    let conditions: Vec<String> = t
        .conditions
        .iter()
        .map(|c| match c.mode {
            1 => c.parameter.clone(),
            2 => format!("!{}", c.parameter),
            3 => format!("{} > {}", c.parameter, c.threshold),
            4 => format!("{} < {}", c.parameter, c.threshold),
            6 => format!("{} == {}", c.parameter, c.threshold),
            7 => format!("{} != {}", c.parameter, c.threshold),
            other => format!("{} ?{other} {}", c.parameter, c.threshold),
        })
        .collect();
    let interruption = match t.interruption {
        0 => String::new(),
        other => format!("  interruptible({other}{})", if t.ordered_interruption { " ordered" } else { "" }),
    };
    format!(
        "{}{}  blend {}{}  from {}{}{}{interruption}",
        t.to.as_deref().unwrap_or(if t.to_selector.is_some() { "" } else { "(exit)" }),
        t.to_selector.map_or(String::new(), |s| format!("(point {s})")),
        t.duration,
        if t.fixed_duration { " s" } else { " of the state" },
        t.offset,
        t.exit_time.map_or(String::new(), |e| format!("  after {e}")),
        if conditions.is_empty() { String::new() } else { format!("  if {}", conditions.join(" & ")) }
    )
}

/// Prints a JSON value down to `depth` levels: below that, an object or a
/// list is only counted.
pub(super) fn print_json(value: &serde_json::Value, depth: usize, indent: usize) {
    let pad = "  ".repeat(indent);
    let short = |value: &serde_json::Value| match value {
        serde_json::Value::Object(map) => format!("{{{} entries}}", map.len()),
        serde_json::Value::Array(list) => format!("[{} items]", list.len()),
        other => {
            let text = other.to_string();
            if text.len() > 160 { format!("{}...", &text[..text.char_indices().nth(160).map_or(text.len(), |(i, _)| i)]) } else { text }
        }
    };
    match value {
        serde_json::Value::Object(map) if depth > 0 => {
            for (name, value) in map {
                match value {
                    serde_json::Value::Object(_) | serde_json::Value::Array(_) if depth > 1 => {
                        println!("{pad}{name}:");
                        print_json(value, depth - 1, indent + 1);
                    }
                    _ => println!("{pad}{name}: {}", short(value)),
                }
            }
        }
        serde_json::Value::Array(list) if depth > 0 => {
            for (index, value) in list.iter().enumerate() {
                match value {
                    serde_json::Value::Object(_) | serde_json::Value::Array(_) if depth > 1 => {
                        println!("{pad}[{index}]");
                        print_json(value, depth - 1, indent + 1);
                    }
                    _ => println!("{pad}[{index}] {}", short(value)),
                }
            }
        }
        other => println!("{pad}{}", short(other)),
    }
}

pub(super) fn class_label(id: i32) -> String {
    match class::name(id) {
        "" => format!("#{id}"),
        name => name.to_owned(),
    }
}

pub(super) fn print_tree(assets: &Assets, file: usize) -> Result<()> {
    for transform in assets.objects_of(file, class::TRANSFORM) {
        let t = assets.read(transform)?;
        let is_root = t.req("m_Father")?.as_pptr().is_some_and(|p| p.is_null());
        if is_root {
            print_node(assets, transform, 0)?;
        }
    }
    Ok(())
}

fn print_node(assets: &Assets, transform: ObjRef, depth: usize) -> Result<()> {
    let t = assets.read(transform)?;
    let go_ptr = t.req("m_GameObject")?.as_pptr().unwrap();
    let mut name = String::from("?");
    let mut components = Vec::new();
    if let Some(go_ref) = assets.resolve_loaded(transform.file, go_ptr) {
        let go = assets.read(go_ref)?;
        name = go.get("m_Name").and_then(|n| n.as_str()).unwrap_or("").to_owned();
        // An object switched off, and a component that is (`m_Enabled` 0):
        // each marked.
        if go.get("m_IsActive").and_then(|v| v.as_i64()).is_some_and(|on| on == 0) {
            name += " [off]";
        }
        for c in go.req("m_Component")?.as_array().unwrap_or(&[]) {
            let Some(ptr) = c.get("component").and_then(|p| p.as_pptr()) else { continue };
            let Some(obj) = assets.resolve_loaded(go_ref.file, ptr) else { continue };
            let off = || assets.read(obj).ok().and_then(|v| v.get("m_Enabled").and_then(|e| e.as_i64())).is_some_and(|on| on == 0);
            match assets.class_id(obj) {
                Some(class::TRANSFORM) | None => {}
                Some(class::MONO_BEHAVIOUR) => components.push(script_name(assets, obj) + if off() { " (off)" } else { "" }),
                Some(id) => components.push(class_label(id).to_string() + if off() { " (off)" } else { "" }),
            }
        }
    }
    let pos = t.req("m_LocalPosition")?.as_vec3().unwrap_or_default();
    let rot = t.req("m_LocalRotation")?.as_vec4().unwrap_or_default();
    let scale = t.req("m_LocalScale")?.as_vec3().unwrap_or_default();
    let mut extra = String::new();
    if rot[3].abs() < 0.9999 {
        extra += &format!(" rot({:.3} {:.3} {:.3} {:.3})", rot[0], rot[1], rot[2], rot[3]);
    }
    if scale.iter().any(|s| (s - 1.0).abs() > 1e-4) {
        extra += &format!(" scale({:.3} {:.3} {:.3})", scale[0], scale[1], scale[2]);
    }
    println!(
        "{}{name}  ({:.3} {:.3} {:.3}){extra}  {}",
        "  ".repeat(depth),
        pos[0],
        pos[1],
        pos[2],
        components.join(", ")
    );
    for child in t.req("m_Children")?.as_array().unwrap_or(&[]) {
        if let Some(obj) = child.as_pptr().and_then(|p| assets.resolve_loaded(transform.file, p)) {
            print_node(assets, obj, depth + 1)?;
        }
    }
    Ok(())
}

/// Class name of the script behind a MonoBehaviour. Only the header fields
/// are needed, so an unreadable body does not matter.
pub(super) fn script_name(assets: &Assets, behaviour: ObjRef) -> String {
    let script = assets
        .read(behaviour)
        .ok()
        .and_then(|v| v.get("m_Script").and_then(|s| s.as_pptr()))
        .and_then(|p| assets.resolve_loaded(behaviour.file, p))
        .and_then(|obj| assets.read(obj).ok())
        .and_then(|v| v.get("m_ClassName").and_then(|n| n.as_str()).map(str::to_owned));
    format!("[{}]", script.as_deref().unwrap_or("script"))
}

/// A window for `sketch::views` from four words of the command line, if
/// they are there: centimetres ahead, behind, above and below.
pub(super) fn window_of(args: &[String], first: usize) -> Option<[f32; 4]> {
    let number = |i: usize| args.get(first + i)?.parse::<f32>().ok();
    Some([number(0)?, number(1)?, number(2)?, number(3)?])
}

/// Colours to tell parts apart by.
pub(super) const PALETTE: [[u8; 3]; 8] = [[120, 130, 145], [200, 120, 60], [70, 150, 90], [170, 80, 160], [60, 130, 200], [190, 170, 60], [200, 70, 70], [90, 90, 90]];

/// A scene's props' groups of levels of detail as read (`scene_props`):
/// each group's object, size, point, fade mode and levels' heights, and the
/// objects each level draws; then where each level is left at the install's
/// view (bias 2, 74 degrees) and at TRP's default (bias 2, 50 degrees).
pub(super) fn print_prop_lods(props: &trp_data::level::scene_props::Props) {
    println!("{} groups of levels of detail", props.lods.len());
    for (at, lod) in props.lods.iter().enumerate() {
        let group = &lod.group;
        let drawn: Vec<String> = (0..group.levels.len())
            .map(|level| {
                let names: Vec<&str> = props.nodes.iter().filter(|node| node.lod.is_some_and(|own| own.group == at && own.levels & (1 << level) != 0)).map(|node| node.name.as_str()).collect();
                format!("level {level} ({:.4}): {}", group.levels[level].height, names.join(", "))
            })
            .collect();
        let reach = |fov: f32| -> Vec<String> { group.heights().iter().map(|&height| format!("{:.1}", trp_sim_free_reach(group.size, height, fov))).collect() };
        println!(
            "  group {at} on {} (size {:.3}, point {:?}, fade {}): {}; each level left at {} m (74 degrees), {} m (50)",
            props.nodes[lod.node].name,
            group.size,
            group.reference,
            group.fade_mode,
            drawn.join("; "),
            reach(74.0).join(", "),
            reach(50.0).join(", ")
        );
    }
}

/// Where a level stops being drawn at the bias of 2 (the documented rule,
/// `trp_sim::lod::reach`, which the probe does not link): `size × 2 / (2 ×
/// height × tan(fov / 2))`.
fn trp_sim_free_reach(size: f32, height: f32, fov: f32) -> f32 {
    size * 2.0 / (2.0 * height * (fov.to_radians() / 2.0).tan())
}
