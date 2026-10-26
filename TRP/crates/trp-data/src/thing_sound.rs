//! The sounds of the things the hands work (interactions spec, "A thing's
//! sounds"; sound spec, "Where the sounds are kept"): what each door,
//! container and trunk of a map's scene names for its opening, its squeak
//! and its shutting, and a door for its kick, with the figures that go
//! with them. They are no bundle's: each thing's prefab lies in one of the
//! build's own scene files (`level<N>`), which keep no type trees, and its
//! clips in the `sharedassets<N>.assets` beside it. The script is read by
//! the `LootableContainer` tree of a bundle that has one with its clips
//! (`TREE_BUNDLE`), a door's own fields after the base's by their declared
//! order, and a clip by the same bundle's `AudioClip` tree.

use std::collections::HashMap;

use anyhow::{Context, Result};

use crate::install::Install;
use crate::sound::{clip_of, Clip, VorbisSetups};
use crate::unity::reader::Reader;
use crate::unity::typetree::{Node, TypeTree};
use crate::unity::{class, Value};

/// A bundle that keeps a container's script with its tree, and its clips.
pub const TREE_BUNDLE: &str = "assets/content/location_objects/lootable/prefab/rand_loot_container_twitch.bundle";

/// The scenes the app reads: Customs' design scene (`custom_DesignStuff`:
/// its containers, corpses, safes and cases) and its dorms
/// (`custom_Obshezhitie`: its metal doors).
pub const SCENES: [&str; 2] = ["level8", "level10"];

/// The script's class, which says what the thing is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThingKind {
    Door,
    Container,
    Trunk,
}

impl ThingKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Door => "door",
            Self::Container => "container",
            Self::Trunk => "trunk",
        }
    }
}

/// One thing of a scene: what its script names. The clips are places in
/// `ThingSounds::clips`.
#[derive(Debug, Clone)]
pub struct ThingSet {
    pub scene: String,
    /// The object's path in the scene, and its own name.
    pub path: String,
    pub name: String,
    pub kind: ThingKind,
    pub open: Vec<usize>,
    pub squeak: Vec<usize>,
    pub shut: Vec<usize>,
    /// The three lists' rolloffs in metres (`_openSoundRolloff`,
    /// `_squeakSoundRolloff`, `_shutSoundRolloff`).
    pub rolloffs: [f32; 3],
    /// Seconds of the curve before its end that the shut sound plays
    /// (`ShutShift`).
    pub shut_shift: f32,
    /// Whether it has a handle to play first (`_handle`).
    pub has_handle: bool,
    /// Its angles shut and open (`CloseAngle`, `OpenAngle`), in degrees.
    pub angles: [f32; 2],
    /// A door's kick (`BreachSound`, `HitClip`) and their rolloffs.
    pub breach: Option<usize>,
    pub hit: Option<usize>,
    pub breach_rolloff: f32,
    pub hit_rolloff: f32,
}

/// Every thing of the scenes read, and every clip they name, each once,
/// with its samples if they were asked for.
#[derive(Debug, Clone, Default)]
pub struct ThingSounds {
    pub sets: Vec<ThingSet>,
    pub clips: Vec<(String, Option<Clip>)>,
}

impl ThingSounds {
    /// The first thing whose object is named so.
    pub fn named(&self, name: &str) -> Option<&ThingSet> {
        self.sets.iter().find(|set| set.name == name)
    }

    /// A clip's name.
    pub fn clip_name(&self, at: usize) -> &str {
        self.clips.get(at).map_or("", |(name, _)| name.as_str())
    }
}

/// The trees borrowed from `TREE_BUNDLE`: a container's script's, and a
/// clip's.
fn trees(install: &Install) -> Result<(TypeTree, TypeTree)> {
    let mut assets = crate::unity::Assets::new(install.clone())?;
    let files = assets.load(TREE_BUNDLE)?;
    let find = |class: i32, field: Option<&str>| {
        files.iter().find_map(|&file| {
            assets
                .file(file)
                .meta
                .types
                .iter()
                .find(|ty| ty.class_id == class && !ty.tree.is_empty() && field.is_none_or(|field| ty.tree.nodes.iter().any(|node| &*node.name == field)))
                .map(|ty| ty.tree.clone())
        })
    };
    let script = find(class::MONO_BEHAVIOUR, Some("OpenSound")).with_context(|| format!("{TREE_BUNDLE} keeps no container's script with its tree"))?;
    let clip = find(class::AUDIO_CLIP, None).with_context(|| format!("{TREE_BUNDLE} keeps no clip with its tree"))?;
    Ok((script, clip))
}

/// A script's tree without its header (the object, the flag, the script
/// and the name, which a scene's `fields` begin after) and, if `through`
/// names a field, cut after that field: the base class's part of a
/// subclass's script, which lays its own fields after it.
fn fields_tree(tree: &TypeTree, through: Option<&str>) -> Result<TypeTree> {
    let nodes = &tree.nodes;
    let root = nodes.first().context("an empty tree")?.clone();
    let name = nodes.iter().position(|node| node.level == 1 && &*node.name == "m_Name").context("no m_Name in the script's tree")?;
    let from = nodes[name].end;
    let to = match through {
        Some(field) => {
            let at = nodes.iter().position(|node| node.level == 1 && &*node.name == field).with_context(|| format!("no {field} in the script's tree"))?;
            nodes[at].end
        }
        None => nodes.len(),
    };
    let mut kept: Vec<Node> = vec![root];
    kept.extend(nodes[from..to].iter().cloned());
    Ok(TypeTree::from_nodes(kept))
}

/// Reads the clips a scene's things point to, each once, from the build's
/// files beside the scene.
struct ClipReader<'a> {
    install: &'a Install,
    tree: TypeTree,
    samples: bool,
    setups: VorbisSetups,
    places: HashMap<(String, i64), Option<usize>>,
    files: HashMap<String, Option<crate::screen::BuildFile>>,
    unread: Vec<String>,
}

impl ClipReader<'_> {
    /// The place of the clip a pointer of the scene's leads to, read if it
    /// was not yet; none for a null pointer or a clip that cannot be read.
    fn place(&mut self, scene: &crate::screen::Scene, scene_name: &str, pointer: crate::unity::PPtr, clips: &mut Vec<(String, Option<Clip>)>) -> Option<usize> {
        if pointer.path_id == 0 {
            return None;
        }
        let file = if pointer.file_id == 0 {
            scene_name.to_owned()
        } else {
            let external = scene.externals.get(pointer.file_id as usize - 1)?;
            external.rsplit('/').next().unwrap_or(external).to_owned()
        };
        let key = (file.clone(), pointer.path_id);
        if let Some(&place) = self.places.get(&key) {
            return place;
        }
        let place = match self.read(&file, pointer.path_id) {
            Ok((name, clip)) => {
                clips.push((name, clip));
                Some(clips.len() - 1)
            }
            Err(e) => {
                self.unread.push(format!("{file} {}: {e:#}", pointer.path_id));
                None
            }
        };
        self.places.insert(key, place);
        place
    }

    fn read(&mut self, file: &str, path_id: i64) -> Result<(String, Option<Clip>)> {
        let install = self.install;
        let build = self
            .files
            .entry(file.to_owned())
            .or_insert_with(|| crate::screen::BuildFile::open(install, file).ok())
            .as_mut()
            .with_context(|| format!("{file} cannot be opened"))?;
        let object = *build.meta.object(path_id).with_context(|| format!("no object {path_id} in {file}"))?;
        anyhow::ensure!(object.class_id == class::AUDIO_CLIP, "object {path_id} of {file} is of class {}, not a clip", object.class_id);
        let bytes = build.bytes(object.offset, object.size)?;
        let value = self.tree.read(&mut Reader::new(&bytes)).with_context(|| format!("clip {path_id} of {file}"))?;
        let name = value.get("m_Name").and_then(Value::as_str).unwrap_or_default().to_owned();
        if !self.samples {
            return Ok((name, None));
        }
        // The samples lie in the file the clip names beside the build's
        // own (`sharedassets8.resource`).
        let folder = install.root.join("EscapeFromTarkov_Data");
        let resource = |source: &str, offset: usize, size: usize| -> Result<Vec<u8>> {
            use std::io::{Read, Seek, SeekFrom};
            let name = source.rsplit('/').next().unwrap_or(source);
            let path = folder.join(name);
            let mut file = std::fs::File::open(&path).with_context(|| format!("opening {}", path.display()))?;
            file.seek(SeekFrom::Start(offset as u64))?;
            let mut out = vec![0; size];
            file.read_exact(&mut out)?;
            Ok(out)
        };
        let clip = clip_of(&value, &resource, &mut self.setups).with_context(|| format!("clip {name}"))?;
        Ok((name, Some(clip)))
    }
}

/// Every door, container and trunk of the scenes named (the build's
/// `level<N>` files), with the clips they name, read with their samples
/// if `samples`.
pub fn thing_sounds(install: &Install, scenes: &[&str], samples: bool) -> Result<ThingSounds> {
    let (script_tree, clip_tree) = trees(install)?;
    let base = fields_tree(&script_tree, Some("_openSoundRolloff"))?;
    let mut out = ThingSounds::default();
    let mut clips = ClipReader { install, tree: clip_tree, samples, setups: VorbisSetups::new(install), places: HashMap::new(), files: HashMap::new(), unread: Vec::new() };
    for &scene_name in scenes {
        let scene = crate::screen::Scene::load(install, scene_name, true).with_context(|| format!("reading {scene_name}"))?;
        for behaviour in &scene.behaviours {
            let kind = match scene.class_of(behaviour) {
                Some("Door") => ThingKind::Door,
                Some("LootableContainer") => ThingKind::Container,
                Some("Trunk") => ThingKind::Trunk,
                _ => continue,
            };
            let bytes = scene.fields(behaviour);
            let mut r = Reader::new(bytes);
            let path = scene.path(behaviour.node);
            let value = base.read(&mut r).with_context(|| format!("{scene_name} {path}: the script's base"))?;
            let mut list = |name: &str| -> Vec<usize> {
                value
                    .get(name)
                    .and_then(Value::as_array)
                    .unwrap_or(&[])
                    .iter()
                    .filter_map(|pointer| clips.place(&scene, scene_name, pointer.as_pptr()?, &mut out.clips))
                    .collect()
            };
            let (open, squeak, shut) = (list("OpenSound"), list("SqueakSound"), list("ShutSound"));
            let number = |name: &str| value.get(name).and_then(Value::as_f32).or_else(|| value.get(name).and_then(Value::as_i64).map(|n| n as f32)).unwrap_or(0.0);
            let rolloffs = [number("_openSoundRolloff"), number("_squeakSoundRolloff"), number("_shutSoundRolloff")];
            let has_handle = value.get("_handle").and_then(Value::as_pptr).is_some_and(|p| p.path_id != 0);
            // A door's own fields follow the base's in their declared
            // order: its hit clip, its breach sound, and their rolloffs.
            let (breach, hit, breach_rolloff, hit_rolloff) = if kind == ThingKind::Door {
                let pointer = |r: &mut Reader| -> Option<crate::unity::PPtr> {
                    let file_id = r.i32().ok()?;
                    let path_id = r.i64().ok()?;
                    Some(crate::unity::PPtr { file_id, path_id })
                };
                let hit_pointer = pointer(&mut r);
                let breach_pointer = pointer(&mut r);
                let hit_rolloff = r.i32().unwrap_or(40) as f32;
                let breach_rolloff = r.i32().unwrap_or(60) as f32;
                let hit = hit_pointer.and_then(|p| clips.place(&scene, scene_name, p, &mut out.clips));
                let breach = breach_pointer.and_then(|p| clips.place(&scene, scene_name, p, &mut out.clips));
                (breach, hit, breach_rolloff, hit_rolloff)
            } else {
                (None, None, 60.0, 40.0)
            };
            out.sets.push(ThingSet {
                scene: scene_name.to_owned(),
                name: scene.nodes[behaviour.node].name.clone(),
                path,
                kind,
                open,
                squeak,
                shut,
                rolloffs,
                shut_shift: number("ShutShift"),
                has_handle,
                angles: [number("CloseAngle"), number("OpenAngle")],
                breach,
                hit,
                breach_rolloff,
                hit_rolloff,
            });
        }
    }
    if !clips.unread.is_empty() {
        eprintln!("{} of the things' clips are not read and are silent: {}", clips.unread.len(), clips.unread.join("; "));
    }
    Ok(out)
}
