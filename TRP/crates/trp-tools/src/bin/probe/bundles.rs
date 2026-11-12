//! A bundle's keys, objects, names, dumps, scripts, trees and textures.

use std::collections::BTreeMap;

use anyhow::{bail, Result};
use trp_data::unity::{class, Assets, ObjRef};

use crate::printing::{class_label, print_json, print_tree, script_name};

// A look into one of the database's files: `probe json
// templates/profiles.json /Standard/usec/character/Inventory 2`.
pub fn json(assets: &mut Assets, args: &[String], file: &str) -> Result<()> {
    let arg = |i: usize| args.get(i).map(String::as_str);
    let path = assets.install.database_dir().join(file);
    let value: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path)?)?;
    let at = value.pointer(arg(2).unwrap_or("")).ok_or_else(|| anyhow::anyhow!("nothing at {}", arg(2).unwrap_or("")))?;
    let depth: usize = arg(3).map(|d| d.parse()).transpose()?.unwrap_or(1);
    print_json(at, depth, 0);
    Ok(())
}

pub fn find(assets: &mut Assets, text: &str) -> Result<()> {
    let mut keys: Vec<&String> =
        assets.manifest.entries.keys().filter(|k| k.contains(text)).collect();
    keys.sort();
    for key in keys {
        println!("{key}");
    }
    Ok(())
}

pub fn info(assets: &mut Assets, key: &str) -> Result<()> {
    println!("dependencies: {:?}", assets.manifest.dependencies(key));
    let own = assets.load(key)?;
    for (index, file) in assets.files().iter().enumerate() {
        let mark = if own.contains(&index) { "*" } else { " " };
        let mut counts: BTreeMap<i32, usize> = BTreeMap::new();
        for o in &file.meta.objects {
            *counts.entry(o.class_id).or_default() += 1;
        }
        let counts: Vec<String> = counts
            .iter()
            .map(|(id, n)| match class::name(*id) {
                "" => format!("#{id}={n}"),
                name => format!("{name}={n}"),
            })
            .collect();
        println!(
            "{mark}[{index}] {} <- {}\n    v{} {} typetrees={} externals={:?}\n    {}",
            file.name,
            file.bundle_key,
            file.meta.version,
            file.meta.unity_version,
            file.meta.has_type_trees,
            file.meta.externals,
            counts.join(" ")
        );
    }
    Ok(())
}

pub fn names(assets: &mut Assets, args: &[String], key: &str) -> Result<()> {
    let arg = |i: usize| args.get(i).map(String::as_str);
    let filter: Option<i32> = arg(2).map(|c| c.parse()).transpose()?;
    for file in assets.load(key)? {
        let objects: Vec<_> = assets.file(file).meta.objects.clone();
        for o in objects {
            if filter.is_some_and(|c| c != o.class_id) {
                continue;
            }
            let obj = ObjRef { file, path_id: o.path_id };
            let name = match assets.read(obj) {
                Ok(v) => v.get("m_Name").and_then(|n| n.as_str()).unwrap_or("").to_owned(),
                Err(e) => format!("<unreadable: {e}>"),
            };
            println!("{:>20} {:<22} {:>9}  {name}", o.path_id, class_label(o.class_id), o.size);
        }
    }
    Ok(())
}

pub fn dump(assets: &mut Assets, args: &[String], key: &str) -> Result<()> {
    let arg = |i: usize| args.get(i).map(String::as_str);
    let path_id: i64 = arg(2).ok_or_else(|| anyhow::anyhow!("path id needed"))?.parse()?;
    let depth = arg(3).map_or(Ok(6), str::parse)?;
    let items = arg(4).map_or(Ok(12), str::parse)?;
    for file in assets.load(key)? {
        let obj = ObjRef { file, path_id };
        if assets.info(obj).is_some() {
            println!("{}", assets.read(obj)?.pretty(depth, items));
        }
    }
    Ok(())
}

pub fn script(assets: &mut Assets, args: &[String], key: &str) -> Result<()> {
    let arg = |i: usize| args.get(i).map(String::as_str);
    // Every script of a bundle of the class named, as stored, with
    // the object each is on.
    let wanted = arg(2).ok_or_else(|| anyhow::anyhow!("a script's class name needed"))?;
    let depth = arg(3).map_or(Ok(6), str::parse)?;
    let items = arg(4).map_or(Ok(12), str::parse)?;
    for file in assets.load(key)? {
        for obj in assets.objects_of(file, class::MONO_BEHAVIOUR).collect::<Vec<_>>() {
            if trp_data::unity::extract::script_class(&assets, obj).as_deref() != Some(wanted) {
                continue;
            }
            let value = assets.read(obj)?;
            let owner = value
                .get("m_GameObject")
                .and_then(|v| v.as_pptr())
                .and_then(|p| assets.resolve_loaded(obj.file, p))
                .and_then(|owner| assets.read(owner).ok())
                .and_then(|owner| owner.get("m_Name").and_then(|n| n.as_str()).map(str::to_owned))
                .unwrap_or_default();
            println!("{wanted} {} on {owner}\n{}", obj.path_id, value.pretty(depth, items));
        }
    }
    Ok(())
}

// An object a bundle's object points to, by the field's name
// (`follow <bundle> <path id> <field> [depth] [items]`), dumped.
pub fn follow(assets: &mut Assets, args: &[String], key: &str) -> Result<()> {
    let arg = |i: usize| args.get(i).map(String::as_str);
    let path_id: i64 = arg(2).ok_or_else(|| anyhow::anyhow!("path id needed"))?.parse()?;
    let field = arg(3).ok_or_else(|| anyhow::anyhow!("field needed"))?;
    let depth = arg(4).map_or(Ok(6), str::parse)?;
    let items = arg(5).map_or(Ok(12), str::parse)?;
    for file in assets.load(key)? {
        let obj = ObjRef { file, path_id };
        if assets.info(obj).is_none() {
            continue;
        }
        let value = assets.read(obj)?;
        let target = value.get(field).and_then(|v| v.as_pptr()).and_then(|p| assets.resolve_loaded(file, p)).ok_or_else(|| anyhow::anyhow!("{field} leads nowhere loaded"))?;
        println!("{}", assets.read(target)?.pretty(depth, items));
    }
    Ok(())
}

// Every object of a bundle: its path id, its class and its name, if
// it has one (`objects <bundle>`); to find what to `dump`.
pub fn objects(assets: &mut Assets, key: &str) -> Result<()> {
    for file in assets.load(key)? {
        let objects: Vec<(i64, i32)> = assets.file(file).meta.objects.iter().map(|o| (o.path_id, o.class_id)).collect();
        for (path_id, class_id) in objects {
            let name = assets.read(ObjRef { file, path_id }).ok().and_then(|v| v.get("m_Name").and_then(|n| n.as_str()).map(str::to_owned)).unwrap_or_default();
            println!("{path_id:>22}  class {class_id:>4}  {name}");
        }
    }
    Ok(())
}

pub fn tree(assets: &mut Assets, key: &str) -> Result<()> {
    for file in assets.load(key)? {
        print_tree(&assets, file)?;
    }
    Ok(())
}

pub fn scripts(assets: &mut Assets, key: &str) -> Result<()> {
    for file in assets.load(key)? {
        for obj in assets.objects_of(file, class::MONO_BEHAVIOUR).collect::<Vec<_>>() {
            let owner = assets
                .read(obj)
                .ok()
                .and_then(|v| v.get("m_GameObject").and_then(|g| g.as_pptr()))
                .and_then(|p| assets.resolve_loaded(file, p))
                .and_then(|go| assets.read(go).ok())
                .and_then(|go| go.get("m_Name").and_then(|n| n.as_str()).map(str::to_owned))
                .unwrap_or_default();
            println!("{:>20} {:<34} {owner}", obj.path_id, script_name(&assets, obj));
        }
    }
    Ok(())
}

// A binary FBX file, which is no part of the install: its models
// with where each is and how large, and each mesh's connected
// pieces. `fbx <file> records` prints the file's records instead.
// `texture <key> <path_id> <out.png>`: a picture of a bundle by its
// object, its largest level as RGBA, its top up; its format and size
// said.
pub fn texture(assets: &mut Assets, args: &[String], key: &str) -> Result<()> {
    let arg = |i: usize| args.get(i).map(String::as_str);
    let (Some(id), Some(out)) = (arg(2).and_then(|i| i.parse::<i64>().ok()), arg(3)) else { bail!("texture <key> <path_id> <out.png>") };
    let files = assets.load(key)?;
    let obj = files.iter().map(|&file| ObjRef { file, path_id: id }).find(|&obj| assets.class_id(obj) == Some(class::TEXTURE_2D)).ok_or_else(|| anyhow::anyhow!("no picture {id} in {key}"))?;
    let texture = trp_data::unity::extract::read_texture(&assets, obj)?;
    let levels = trp_data::unity::texture::mips(&texture)?;
    let rgba = trp_data::unity::texture::decode_rgba(&texture, *levels.first().ok_or_else(|| anyhow::anyhow!("no levels"))?)?;
    let (w, h) = (texture.width as usize, texture.height as usize);
    let flipped: Vec<u8> = (0..h).rev().flat_map(|row| rgba[row * w * 4..(row + 1) * w * 4].iter().copied()).collect();
    trp_data::interface::icon::save_png(std::path::Path::new(out), w as u32, h as u32, &flipped)?;
    println!("{}: {w}x{h}, format {}, {} levels, srgb {}", texture.name, texture.format, texture.mip_count, texture.srgb);
    let (mut lit, mut sum) = (0usize, [0u64; 4]);
    for p in rgba.chunks_exact(4) {
        lit += (p[3] > 127) as usize;
        (0..4).for_each(|k| sum[k] += p[k] as u64);
    }
    println!("  mean rgba {:?}, alpha over half in {lit} of {} pixels", sum.map(|s| s / (w * h) as u64), w * h);
    Ok(())
}
