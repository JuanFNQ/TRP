//! Animation clips and the blend trees checked against them.

use anyhow::Result;
use trp_data::unity::Assets;

pub fn clips(assets: &mut Assets, args: &[String], template: &str) -> Result<()> {
    let arg = |i: usize| args.get(i).map(String::as_str);
    use trp_data::unity::anim::{path_hash, Clip};
    let db = trp_data::Database::load(&assets.install)?;
    let weapon = trp_data::weapons::weapon::assemble_named(&mut *assets, &db, template)?;
    let paths: std::collections::HashMap<u32, String> = (0..weapon.hierarchy.nodes.len())
        .map(|i| weapon.hierarchy.path(i))
        .map(|p| (path_hash(&p), p))
        .collect();
    let filter = arg(2).unwrap_or("");
    for (name, obj) in weapon.clips(&assets)? {
        if !name.contains(filter) {
            continue;
        }
        let clip = Clip::read(&assets, obj)?;
        let bound = clip.tracks.iter().filter(|t| paths.contains_key(&t.path)).count();
        println!(
            "{name:<34} -> {:<44} {:>6.2}s loop={} tracks={} on-rig={} from {}",
            clip.name,
            clip.duration(),
            clip.looping,
            clip.tracks.len(),
            bound,
            assets.file(obj.file).bundle_key.rsplit('/').next().unwrap_or("")
        );
        if arg(3) == Some("check") {
            // Compares the clip's first frame with the prefab's rest pose.
            use trp_data::unity::anim::{euler_to_quat, Property};
            let nodes: std::collections::HashMap<u32, usize> = (0..weapon.hierarchy.nodes.len())
                .map(|i| (path_hash(&weapon.hierarchy.path(i)), i))
                .collect();
            let mut values = Vec::new();
            clip.sample(0.0, &mut values);
            let (mut count, mut close, mut worst, mut custom) = (0, 0, (0.0f32, String::new()), 0);
            for t in clip.tracks.iter().filter(|t| t.property == Property::Euler) {
                custom += (t.custom_type != 0) as usize;
                let Some(&node) = nodes.get(&t.path) else { continue };
                let q = euler_to_quat([values[t.curve], values[t.curve + 1], values[t.curve + 2]]);
                let rest = weapon.hierarchy.nodes[node].rotation;
                let dot: f32 = q.iter().zip(&rest).map(|(a, b)| a * b).sum();
                let angle = 2.0 * dot.abs().min(1.0).acos().to_degrees();
                count += 1;
                close += (angle < 1.0) as usize;
                if angle > worst.0 {
                    worst = (angle, weapon.hierarchy.nodes[node].name.clone());
                }
            }
            println!(
                "    euler tracks on rig: {count}, within 1 degree of rest: {close}, largest difference {:.2} degrees at {}, non-zero customType: {custom}",
                worst.0, worst.1
            );
        }
        // `tracks [word] [time]`: the clip's tracks whose bone path
        // ends in the word, at that time (seconds; the start if
        // none is given).
        if !filter.is_empty() && arg(3) == Some("tracks") {
            let word = arg(4).unwrap_or("");
            let time = arg(5).and_then(|t| t.parse::<f32>().ok()).unwrap_or(0.0);
            let mut values = Vec::new();
            clip.sample(time, &mut values);
            for t in &clip.tracks {
                let path = paths.get(&t.path).map_or("<not on rig>", String::as_str);
                if !path.ends_with(word) {
                    continue;
                }
                let n = if t.property == trp_data::unity::anim::Property::Rotation { 4 } else { 3 };
                println!("    {:?} {path} at {time} s = {:?}", t.property, &values[t.curve..t.curve + n]);
            }
        }
        // `path [word] [from] [to] [step]`: where the first bone
        // whose path ends in the word is through the clip, in the
        // rig's own frame: every bone above it as the clip has it
        // (as the prefab has it where the clip says nothing), one
        // line a time, with how far it has turned from the line
        // before and its speed. A break in the speed at a steady
        // period is the clip's own: curves kept as samples run in
        // straight lines from one to the next.
        if !filter.is_empty() && arg(3) == Some("path") {
            use trp_data::unity::anim::{euler_to_quat, Property};
            let word = arg(4).unwrap_or("");
            let figure = |index: usize, otherwise: f32| arg(index).and_then(|t| t.parse::<f32>().ok()).unwrap_or(otherwise);
            let (from, to, step) = (figure(5, 0.0), figure(6, clip.duration()), figure(7, 1.0 / 120.0).max(1e-4));
            let Some(bone) = (0..weapon.hierarchy.nodes.len()).find(|&i| weapon.hierarchy.path(i).ends_with(word)) else {
                println!("    no bone's path ends in {word}");
                continue;
            };
            // The bone and everything above it, the bone first.
            let mut chain = vec![bone];
            while let Some(parent) = weapon.hierarchy.nodes[*chain.last().unwrap()].parent {
                chain.push(parent);
            }
            let tracks_of = |node: usize| {
                let hash = path_hash(&weapon.hierarchy.path(node));
                let find = |wanted: &[Property]| clip.tracks.iter().find(|t| t.path == hash && wanted.contains(&t.property)).copied();
                (find(&[Property::Position]), find(&[Property::Rotation, Property::Euler]))
            };
            let tracks: Vec<_> = chain.iter().map(|&node| tracks_of(node)).collect();
            let (cubics, samples, rate, _) = clip.kept();
            for (&node, (place, turn)) in chain.iter().zip(&tracks) {
                let kept = |curve: Option<usize>| match curve {
                    None => "the prefab's",
                    Some(c) if c < cubics => "cubics",
                    Some(c) if c < cubics + samples => "samples",
                    Some(_) => "one figure",
                };
                println!("    {}: place {}, turn {}", weapon.hierarchy.path(node), kept(place.map(|t| t.curve)), kept(turn.map(|t| t.curve)));
            }
            println!("    samples are {rate} a second");
            let mul = |a: [f32; 4], b: [f32; 4]| -> [f32; 4] {
                [
                    a[3] * b[0] + a[0] * b[3] + a[1] * b[2] - a[2] * b[1],
                    a[3] * b[1] - a[0] * b[2] + a[1] * b[3] + a[2] * b[0],
                    a[3] * b[2] + a[0] * b[1] - a[1] * b[0] + a[2] * b[3],
                    a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2],
                ]
            };
            let turned = |q: [f32; 4], v: [f32; 3]| -> [f32; 3] {
                let p = mul(mul(q, [v[0], v[1], v[2], 0.0]), [-q[0], -q[1], -q[2], q[3]]);
                [p[0], p[1], p[2]]
            };
            let mut values = Vec::new();
            let mut last: Option<([f32; 3], [f32; 4])> = None;
            let mut time = from;
            while time <= to + 1e-6 {
                clip.sample(time, &mut values);
                // From the bone up: each bone's place and turn in
                // its parent takes what is below it along.
                let (mut at, mut facing) = ([0.0f32; 3], [0.0f32, 0.0, 0.0, 1.0]);
                for (&node, (place, turn)) in chain.iter().zip(&tracks) {
                    let rest = &weapon.hierarchy.nodes[node];
                    let p = place.map_or(rest.translation, |t| [values[t.curve], values[t.curve + 1], values[t.curve + 2]]);
                    let q = match turn {
                        Some(t) if t.property == Property::Euler => euler_to_quat([values[t.curve], values[t.curve + 1], values[t.curve + 2]]),
                        Some(t) => [values[t.curve], values[t.curve + 1], values[t.curve + 2], values[t.curve + 3]],
                        None => rest.rotation,
                    };
                    let moved = turned(q, at);
                    at = [moved[0] + p[0], moved[1] + p[1], moved[2] + p[2]];
                    facing = mul(q, facing);
                }
                let mut line = format!("    {time:.4} sample {:6.2}  at {:8.2} {:8.2} {:8.2} mm", time * rate, at[0] * 1000.0, at[1] * 1000.0, at[2] * 1000.0);
                if let Some((was, faced)) = last {
                    let dot: f32 = (0..4).map(|k| faced[k] * facing[k]).sum();
                    let angle = 2.0 * dot.abs().min(1.0).acos().to_degrees();
                    line.push_str(&format!(
                        "  speed {:8.1} {:8.1} {:8.1} mm/s  turning {:7.1} deg/s",
                        (at[0] - was[0]) / step * 1000.0,
                        (at[1] - was[1]) / step * 1000.0,
                        (at[2] - was[2]) / step * 1000.0,
                        angle / step
                    ));
                }
                println!("{line}");
                last = Some((at, facing));
                time += step;
            }
        }
        // `series [word] [from] [to] [step]`: the same tracks at
        // each of a run of times, one line a time, and how each is
        // kept (as cubics from key to key, as samples at a steady
        // rate with straight lines between, or as one figure).
        if !filter.is_empty() && arg(3) == Some("series") {
            let word = arg(4).unwrap_or("");
            let figure = |index: usize, otherwise: f32| arg(index).and_then(|t| t.parse::<f32>().ok()).unwrap_or(otherwise);
            let (from, to, step) = (figure(5, 0.0), figure(6, clip.duration()), figure(7, 1.0 / 120.0).max(1e-4));
            let chosen: Vec<_> = clip.tracks.iter().filter(|t| paths.get(&t.path).is_some_and(|p| p.ends_with(word))).collect();
            let (cubics, samples, rate, figures) = clip.kept();
            println!("    {cubics} curves as cubics, {samples} as samples at {rate} a second, {figures} that never change");
            for t in &chosen {
                let kept = if t.curve < cubics {
                    "cubics"
                } else if t.curve < cubics + samples {
                    "samples"
                } else {
                    "one figure"
                };
                println!("    {:?} {} ({kept}, curve {})", t.property, paths.get(&t.path).map_or("", String::as_str), t.curve);
            }
            let mut values = Vec::new();
            let mut time = from;
            while time <= to + 1e-6 {
                clip.sample(time, &mut values);
                let mut line = format!("    {time:.4}");
                for t in &chosen {
                    let n = if t.property == trp_data::unity::anim::Property::Rotation { 4 } else { 3 };
                    for value in &values[t.curve..t.curve + n] {
                        line.push_str(&format!(" {value:.5}"));
                    }
                    line.push_str(" |");
                }
                println!("{line}");
                time += step;
            }
        }
    }
    Ok(())
}

pub fn blendcheck(assets: &mut Assets, filter: Option<&str>) -> Result<()> {
    // Freeform directional trees of the root motion table: where the
    // stored weights and the computed ones part.
    use trp_data::character::rootmotion::{freeform_directional, BlendType, Node, RootMotionTable};
    let table = RootMotionTable::load(&mut *assets)?;
    fn visit(table: &RootMotionTable, state: &str, node: &Node, limit: usize) {
        let Node::Tree { blend, param_x, param_y, positions, steps_x, steps_y, weights, children, .. } = node else {
            return;
        };
        if *blend == BlendType::FreeformDirectional2D && positions.len() == children.len() {
            let range = |hash: u32| table.parameters.iter().find(|p| trp_data::unity::anim::path_hash(&p.name) == hash);
            let (Some(px), Some(py)) = (range(*param_x), range(*param_y)) else { return };
            let n = children.len();
            println!("{state}: {} by {} over {:?}, grid {steps_x}x{steps_y}", px.name, py.name, positions);
            let mut shown = 0;
            for x in 0..*steps_x {
                for y in 0..*steps_y {
                    let at = [px.min + x as f32 * px.step, py.min + y as f32 * py.step];
                    let mine = freeform_directional(positions, at);
                    let stored = &weights[(x * steps_y + y) * n..(x * steps_y + y + 1) * n];
                    let gap = mine.iter().zip(stored).map(|(a, b)| (a - b).abs()).fold(0.0f32, f32::max);
                    if gap > 0.01 && shown < limit {
                        shown += 1;
                        println!("   at {at:?}: stored {stored:.3?}  computed {mine:.3?}");
                    }
                }
            }
        }
        for child in children {
            visit(table, state, child, limit);
        }
    }
    for (name, node) in table.states.values() {
        if filter.is_none_or(|f| name.contains(f)) {
            visit(&table, name, node, 6);
        }
    }
    // Which weight of direction against distance fits the stored
    // grids best: the largest and the mean difference for each.
    fn scan(table: &RootMotionTable, node: &Node, k: f32, worst: &mut f32, sum: &mut f64, count: &mut u64) {
        let Node::Tree { blend, param_x, param_y, positions, steps_x, steps_y, weights, children, .. } = node else {
            return;
        };
        if *blend == BlendType::FreeformDirectional2D && positions.len() == children.len() {
            let range = |hash: u32| table.parameters.iter().find(|p| trp_data::unity::anim::path_hash(&p.name) == hash);
            if let (Some(px), Some(py)) = (range(*param_x), range(*param_y)) {
                let n = children.len();
                for x in 0..*steps_x {
                    for y in 0..*steps_y {
                        let at = [px.min + x as f32 * px.step, py.min + y as f32 * py.step];
                        let mine = trp_data::character::rootmotion::freeform_directional_weighted(positions, at, k);
                        for c in 0..n {
                            let gap = (mine[c] - weights[(x * steps_y + y) * n + c]).abs();
                            *worst = worst.max(gap);
                            *sum += gap as f64;
                            *count += 1;
                        }
                    }
                }
            }
        }
        for child in children {
            scan(table, child, k, worst, sum, count);
        }
    }
    for k in [0.3183, 0.6366, 0.9, 0.98, 1.0, 1.01, 1.02, 1.1, 1.5708, 2.0] {
        let (mut worst, mut sum, mut count) = (0.0f32, 0.0f64, 0u64);
        for (_, node) in table.states.values() {
            scan(&table, node, k, &mut worst, &mut sum, &mut count);
        }
        println!("direction weight {k}: largest difference {worst:.4}, mean {:.6}", sum / count.max(1) as f64);
    }
    Ok(())
}
