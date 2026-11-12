//! Another game's OGF models and XDB archives, and FBX files.

use anyhow::Result;
use trp_data::formats::sketch::Drawn;

use crate::printing::{PALETTE, window_of};

// A model of S.T.A.L.K.E.R.'s (`docs/spec/outfits.md`), by its
// file or by its path under that game's `meshes`: its meshes with
// their pictures, and its bones at rest, each beside the middle of
// the vertices that go mostly with it.
pub fn ogf(wanted: &str) -> Result<()> {
    let stalker = trp_data::formats::xray::Stalker::locate();
    let (bytes, file) = match std::path::Path::new(wanted).is_file() {
        true => (std::fs::read(wanted)?, wanted.to_owned()),
        false => stalker.read(&format!("meshes/{wanted}"))?,
    };
    let model = trp_data::formats::xray::read(&bytes)?;
    println!("{file}: {} bones, {} meshes", model.bones.len(), model.parts.len());
    for part in &model.parts {
        let told = match stalker.read(&format!("textures/{}.dds", part.texture)) {
            Ok((bytes, file)) => match trp_data::formats::xray::read_picture(&bytes) {
                Ok(p) => format!("{} x {}, {} levels, {:?}, {file}", p.width, p.height, p.levels, p.packing),
                Err(e) => format!("not read ({e:#}), {file}"),
            },
            Err(e) => format!("{e:#}"),
        };
        let (mut low, mut high) = ([f32::MAX; 3], [f32::MIN; 3]);
        for p in &part.positions {
            for axis in 0..3 {
                (low[axis], high[axis]) = (low[axis].min(p[axis]), high[axis].max(p[axis]));
            }
        }
        println!("  mesh: {} vertices, {} triangles, from {:.3} {:.3} {:.3} to {:.3} {:.3} {:.3}; picture {}: {told}", part.positions.len(), part.triangles.len() / 3, low[0], low[1], low[2], high[0], high[1], high[2], part.texture);
    }
    for (index, bone) in model.bones.iter().enumerate() {
        // The bone's own line: to the first bone that hangs from
        // it, or else on from the one it hangs from. Its vertices
        // are measured along that, from its joint, and across.
        let next = model.bones.iter().find(|other| other.parent == Some(index)).map(|child| child.at);
        let line = next.map(|at| [at[0] - bone.at[0], at[1] - bone.at[1], at[2] - bone.at[2]]).or_else(|| bone.parent.map(|parent| {
            let from = model.bones[parent].at;
            [bone.at[0] - from[0], bone.at[1] - from[1], bone.at[2] - from[2]]
        }));
        let long = line.map_or(0.0, |v| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt());
        let mut sum = [0.0f32; 3];
        let mut count = 0;
        let (mut back, mut ahead, mut wide) = (f32::MAX, f32::MIN, 0.0f32);
        for part in &model.parts {
            for (p, links) in part.positions.iter().zip(&part.links) {
                if links.iter().any(|(linked, share)| *linked as usize == index && *share > 0.5) {
                    sum = [sum[0] + p[0], sum[1] + p[1], sum[2] + p[2]];
                    count += 1;
                    if let Some(line) = line.filter(|_| long > 1e-5) {
                        let off = [p[0] - bone.at[0], p[1] - bone.at[1], p[2] - bone.at[2]];
                        let along = (off[0] * line[0] + off[1] * line[1] + off[2] * line[2]) / long;
                        let across = (off[0] * off[0] + off[1] * off[1] + off[2] * off[2] - along * along).max(0.0).sqrt();
                        (back, ahead, wide) = (back.min(along), ahead.max(along), wide.max(across));
                    }
                }
            }
        }
        let measured = if count > 0 && long > 1e-5 { format!(", from {:.1} to {:.1} cm along its {:.1} cm, {:.1} cm across", back * 100.0, ahead * 100.0, long * 100.0, wide * 100.0) } else { String::new() };
        let middle = if count > 0 { format!("{count:4} vertices about {:7.3} {:7.3} {:7.3}{measured}", sum[0] / count as f32, sum[1] / count as f32, sum[2] / count as f32) } else { "no vertices".to_owned() };
        let parent = bone.parent.map_or("-", |parent| model.bones[parent].name.as_str());
        println!("  {index:2} {:<20} < {:<18} at {:7.3} {:7.3} {:7.3}   {middle}", bone.name, parent, bone.at[0], bone.at[1], bone.at[2]);
    }
    Ok(())
}

// The other game's packed archives (`trp_data::xray_archive`): each
// with its count of files and how long its table took, and the
// names that have the word; with `get`, also each such file read,
// unpacked and checked.
pub fn xdb(args: &[String], part: &str) -> Result<()> {
    let stalker = trp_data::formats::xray::Stalker::locate();
    let began = std::time::Instant::now();
    let archives = stalker.archives();
    println!("{} archives, their tables read in {:.2} s", archives.len(), began.elapsed().as_secs_f32());
    let get = args.get(2).is_some_and(|word| word == "get");
    for archive in archives {
        let names = archive.names_with(part);
        println!("{}: {} files, {} with {part:?}", archive.path.display(), archive.len(), names.len());
        for name in names.iter().take(40) {
            if get {
                let began = std::time::Instant::now();
                match archive.read(name) {
                    Ok(Some(bytes)) => println!("  {name}: {} bytes, checked, in {:.1} ms", bytes.len(), began.elapsed().as_secs_f32() * 1000.0),
                    Ok(None) => println!("  {name}: not there"),
                    Err(e) => println!("  {name}: {e:#}"),
                }
            } else {
                println!("  {name}");
            }
        }
    }
    Ok(())
}

pub fn fbx(args: &[String], fbx_from: &Option<String>, fbx_turn: Option<[f64; 3]>, file: &str) -> Result<()> {
    let arg = |i: usize| args.get(i).map(String::as_str);
    let data = std::fs::read(file)?;
    if arg(2) == Some("records") {
        fn show(record: &trp_data::formats::fbx::Record, indent: usize, depth: usize) {
            let values: Vec<String> = record
                .props
                .iter()
                .map(|prop| match prop {
                    trp_data::formats::fbx::Prop::Int(v) => v.to_string(),
                    trp_data::formats::fbx::Prop::Float(v) => v.to_string(),
                    trp_data::formats::fbx::Prop::Text(v) => format!("{:?}", v.replace('\0', "|").replace('\u{1}', "")),
                    trp_data::formats::fbx::Prop::Raw(v) => format!("<{} bytes>", v.len()),
                    trp_data::formats::fbx::Prop::Ints(v) => format!("<{} ints>", v.len()),
                    trp_data::formats::fbx::Prop::Floats(v) => format!("<{} floats>", v.len()),
                })
                .collect();
            println!("{}{} {}", "  ".repeat(indent), record.name, values.join(" "));
            if depth > 0 {
                record.children.iter().for_each(|child| show(child, indent + 1, depth - 1));
            }
        }
        let depth = arg(3).and_then(|d| d.parse().ok()).unwrap_or(2);
        show(&trp_data::formats::fbx::parse(&data)?, 0, depth);
        return Ok(());
    }
    // `fbx <file> motion [every] [bone ...]`: the file's take, and
    // where the bones named (ends of names; the hips, the head, the
    // hands and the feet where none is) are on every so many frames,
    // in metres in the file's axes.
    if arg(2) == Some("motion") {
        let motion = trp_data::formats::fbx::read_motion(&data)?;
        println!("take {:?}: {} bones, {} frames at {:.2} a second ({:.3} s), {} cm a unit", motion.take, motion.bones.len(), motion.frames, motion.rate, motion.seconds(), motion.unit);
        let every: usize = arg(3).and_then(|n| n.parse().ok()).unwrap_or(5).max(1);
        let named: Vec<&str> = args.iter().skip(if arg(3).is_some_and(|n| n.parse::<usize>().is_ok()) { 4 } else { 3 }).map(String::as_str).collect();
        let named = if named.is_empty() { vec!["Hips", "Head", "LeftHand", "RightHand", "LeftFoot", "RightFoot"] } else { named };
        let shown: Vec<usize> = named.iter().filter_map(|name| motion.bones.iter().position(|bone| bone.name.ends_with(name))).collect();
        let metres = motion.unit / 100.0;
        let line = |placed: &[([f64; 3], trp_data::formats::fbx::Turn)]| shown.iter().map(|&bone| format!("{} [{:6.3} {:6.3} {:6.3}]", motion.bones[bone].name.rsplit(':').next().unwrap_or(""), placed[bone].0[0] * metres, placed[bone].0[1] * metres, placed[bone].0[2] * metres)).collect::<Vec<_>>().join("  ");
        println!(" made  {}", line(&motion.placed(None)));
        for frame in (0..motion.frames).filter(|frame| frame % every == 0 || frame + 1 == motion.frames) {
            println!("{frame:5}  {}", line(&motion.placed(Some(frame))));
        }
        return Ok(());
    }
    let mut scene = trp_data::formats::fbx::read(&data)?;
    if let Some(node) = &fbx_from {
        scene.seen_from(node)?;
    }
    if let Some(turn) = fbx_turn {
        scene.turned(turn);
    }
    // `fbx <file> slice <model> <axis> <from> <to> [piece]`: what
    // of a model lies in a slab across one axis (x, y or z), as
    // the least and most of its corners there: for measuring a
    // feature of it.
    if arg(2) == Some("slice") {
        let (name, axis) = (arg(3).unwrap_or(""), arg(4).and_then(|a| "xyz".find(a)).unwrap_or(0));
        let bound = |i: usize, default: f32| arg(i).and_then(|v| v.parse::<f32>().ok()).unwrap_or(default);
        let (from, to) = (bound(5, f32::MIN), bound(6, f32::MAX));
        let piece: Option<u32> = arg(7).and_then(|p| p.parse().ok());
        for model in scene.models.iter().filter(|model| model.name == name) {
            let Some(mesh) = &model.mesh else { continue };
            let (mut low, mut high, mut count) = ([f32::MAX; 3], [f32::MIN; 3], 0);
            for (corner, p) in mesh.positions.iter().enumerate() {
                if p[axis] < from || p[axis] > to || piece.is_some_and(|piece| mesh.piece[corner / 3] != piece) {
                    continue;
                }
                count += 1;
                (0..3).for_each(|k| (low[k], high[k]) = (low[k].min(p[k]), high[k].max(p[k])));
            }
            println!("{name}: {count} corners there, from {low:.2?} to {high:.2?}");
            // With `points` as the last word: each of them, once.
            if args.last().map(String::as_str) == Some("points") {
                let mut points: Vec<[i32; 3]> = mesh
                    .positions
                    .iter()
                    .enumerate()
                    .filter(|(corner, p)| p[axis] >= from && p[axis] <= to && piece.is_none_or(|piece| mesh.piece[corner / 3] == piece))
                    .map(|(_, p)| p.map(|v| (v * 100.0).round() as i32))
                    .collect();
                points.sort();
                points.dedup();
                for p in points {
                    println!("  {:7.2} {:7.2} {:7.2}", p[0] as f32 / 100.0, p[1] as f32 / 100.0, p[2] as f32 / 100.0);
                }
            }
        }
        return Ok(());
    }
    // `fbx <file> view <out.png> [pixels a centimetre]`: the model
    // from its left side and from above, each model in a colour of
    // its own, over a grid of the file's own centimetres with the
    // origin where the file has it. A file whose barrel runs along
    // -z with y up, as this project's first one does.
    // `fbx <file> cut <model> <axis> <value> [step]`: the model's
    // section by a plane square to an axis (x, y or z), from its
    // triangles' edges: its bounds, and across each band of `step`
    // centimetres of the first other axis the least and most of the
    // second (a receiver's width at each height, for a cut along z).
    if arg(2) == Some("cut") {
        let (name, axis) = (arg(3).unwrap_or(""), arg(4).and_then(|a| "xyz".find(a)).unwrap_or(2));
        let value: f32 = arg(5).and_then(|v| v.parse().ok()).unwrap_or(0.0);
        let step: f32 = arg(6).and_then(|v| v.parse().ok()).unwrap_or(0.25);
        let (a, b) = match axis {
            0 => (1, 2),
            1 => (0, 2),
            _ => (1, 0),
        };
        for model in scene.models.iter().filter(|model| model.name == name) {
            let Some(mesh) = &model.mesh else { continue };
            let mut points: Vec<[f32; 3]> = Vec::new();
            for corners in mesh.positions.chunks_exact(3) {
                for (p, q) in [(corners[0], corners[1]), (corners[1], corners[2]), (corners[2], corners[0])] {
                    let (dp, dq) = (p[axis] - value, q[axis] - value);
                    if (dp <= 0.0) != (dq <= 0.0) {
                        let t = dp / (dp - dq);
                        points.push(std::array::from_fn(|k| p[k] + (q[k] - p[k]) * t));
                    }
                }
            }
            if points.is_empty() {
                println!("{name}: nothing at {value}");
                continue;
            }
            let (mut low, mut high) = ([f32::MAX; 3], [f32::MIN; 3]);
            for p in &points {
                (0..3).for_each(|k| (low[k], high[k]) = (low[k].min(p[k]), high[k].max(p[k])));
            }
            println!("{name}: {} points, from {low:.2?} to {high:.2?}", points.len());
            let mut band = (low[a] / step).floor() * step;
            while band <= high[a] {
                let inside: Vec<f32> = points.iter().filter(|p| p[a] >= band && p[a] < band + step).map(|p| p[b]).collect();
                if !inside.is_empty() {
                    let (least, most) = inside.iter().fold((f32::MAX, f32::MIN), |(l, h), &v| (l.min(v), h.max(v)));
                    println!("  {}{band:6.2} .. {:6.2}: {}{least:6.2} .. {most:6.2}", ["x", "y", "z"][a], band + step, ["x", "y", "z"][b]);
                }
                band += step;
            }
        }
        return Ok(());
    }
    // `fbx <file> split <model> <x0,y0,z0,x1,y1,z1> <out.png> [pixels
    // a centimetre] [window]`: one model drawn alone, its pieces
    // wholly inside the box blue and the rest red, each piece's
    // number listed by its side: for telling a model's parts apart
    // by its pieces (`custom::PieceFilter`).
    if arg(2) == Some("split") {
        let (name, out) = (arg(3).unwrap_or(""), arg(5).unwrap_or("local/shots/fbx_split.png"));
        let v: Vec<f32> = arg(4).unwrap_or("").split(',').filter_map(|v| v.trim().parse().ok()).collect();
        anyhow::ensure!(v.len() == 6, "the box is x0,y0,z0,x1,y1,z1");
        let scale = arg(6).and_then(|s| s.parse().ok()).unwrap_or(40.0);
        let unit = scene.unit as f32;
        let model = scene.models.iter().find(|model| model.name == name).ok_or_else(|| anyhow::anyhow!("no model {name}"))?;
        let mesh = model.mesh.as_ref().ok_or_else(|| anyhow::anyhow!("{name} has no mesh"))?;
        let mut low = vec![[f32::MAX; 3]; mesh.pieces as usize];
        let mut high = vec![[f32::MIN; 3]; mesh.pieces as usize];
        for (corner, p) in mesh.positions.iter().enumerate() {
            let piece = mesh.piece[corner / 3] as usize;
            (0..3).for_each(|k| (low[piece][k], high[piece][k]) = (low[piece][k].min(p[k]), high[piece][k].max(p[k])));
        }
        let inside: Vec<bool> = (0..mesh.pieces as usize).map(|piece| (0..3).all(|k| low[piece][k] >= v[k] && high[piece][k] <= v[k + 3])).collect();
        let (blue, red): (Vec<usize>, Vec<usize>) = (0..inside.len()).partition(|&piece| inside[piece]);
        println!("inside ({}): {blue:?}\noutside ({}): {red:?}", blue.len(), red.len());
        let triangles: Vec<Drawn> = mesh
            .positions
            .chunks_exact(3)
            .enumerate()
            .map(|(t, corners)| {
                let placed = [0, 1, 2].map(|k| [-corners[k][2] * unit, corners[k][1] * unit, corners[k][0] * unit]);
                (placed, if inside[mesh.piece[t] as usize] { [60, 110, 200] } else { [200, 70, 70] }, 1.0)
            })
            .collect();
        return trp_data::formats::sketch::views(&triangles, &[], &[], out, scale, window_of(&args, 7));
    }
    if arg(2) == Some("view") {
        let out = arg(3).unwrap_or("local/shots/fbx_view.png");
        let scale = arg(4).and_then(|s| s.parse().ok()).unwrap_or(12.0);
        let unit = scene.unit as f32;
        let mut triangles: Vec<Drawn> = Vec::new();
        for (index, model) in scene.models.iter().enumerate() {
            let Some(mesh) = &model.mesh else { continue };
            println!("{:?} is {}", PALETTE[index % PALETTE.len()], model.name);
            for corners in mesh.positions.chunks_exact(3) {
                let placed = [0, 1, 2].map(|k| [-corners[k][2] * unit, corners[k][1] * unit, corners[k][0] * unit]);
                triangles.push((placed, PALETTE[index % PALETTE.len()], 1.0));
            }
        }
        return trp_data::formats::sketch::views(&triangles, &[], &[], out, scale, window_of(&args, 5));
    }
    println!("{} cm a unit; up {:?}, front {:?}, third {:?}", scene.unit, scene.up, scene.front, scene.coord);
    let fewest: usize = arg(2).and_then(|n| n.parse().ok()).unwrap_or(200);
    for model in &scene.models {
        let parent = model.parent.map_or("-", |p| scene.models[p].name.as_str());
        let at = [model.world[0][3], model.world[1][3], model.world[2][3]];
        println!("{}  (under {parent})  at {:.3?}  materials {:?}", model.name, at, model.materials);
        let Some(mesh) = &model.mesh else { continue };
        let bounds = |wanted: &dyn Fn(usize) -> bool| {
            let (mut low, mut high, mut count) = ([f32::MAX; 3], [f32::MIN; 3], 0usize);
            for triangle in (0..mesh.piece.len()).filter(|&t| wanted(t)) {
                count += 1;
                for p in &mesh.positions[triangle * 3..triangle * 3 + 3] {
                    (0..3).for_each(|k| (low[k], high[k]) = (low[k].min(p[k]), high[k].max(p[k])));
                }
            }
            (low, high, count)
        };
        let (low, high, count) = bounds(&|_| true);
        println!("  {count} triangles in {} pieces, from {low:.2?} to {high:.2?}", mesh.pieces);
        for piece in 0..mesh.pieces {
            let (low, high, count) = bounds(&|t| mesh.piece[t] == piece);
            if count < fewest {
                continue;
            }
            let first = mesh.piece.iter().position(|&p| p == piece).unwrap_or(0);
            let material = model.materials.get(mesh.materials[first] as usize).map_or("?", String::as_str);
            println!("    piece {piece:4}: {count:6} triangles  {material:12} from {low:.2?} to {high:.2?}");
        }
    }
    Ok(())
}
