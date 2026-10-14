//! The game's own screens as its scenes hold them: the objects of one of
//! the install's scene files, each with its name, what it hangs from and
//! the rectangle its layout gives it (`docs/spec/inventory.md`, "Where the
//! views sit").
//!
//! A scene file (`EscapeFromTarkov_Data/level<N>`) is a serialized file
//! with no type trees, so the three kinds of object wanted are read by
//! the engine's own fixed layouts for them: an object's name and
//! components, a transform's place in the hierarchy, and a screen
//! transform's anchors, offset, size and pivot.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};

use crate::install::Install;
use crate::unity::reader::Reader;
use crate::unity::{class, SerializedFile};

/// The engine's class number for a transform laid out on a screen.
const RECT_TRANSFORM: i32 = 224;

/// How a screen transform is laid out in what it hangs from, in the
/// engine's terms: the two anchors as shares of the parent's rectangle,
/// the pivot's offset from the anchors, the size added to the anchors'
/// spread, and the pivot as a share of its own rectangle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Layout {
    pub anchor_min: [f32; 2],
    pub anchor_max: [f32; 2],
    pub offset: [f32; 2],
    pub size_delta: [f32; 2],
    pub pivot: [f32; 2],
    pub scale: [f32; 2],
}

/// One object of a scene.
#[derive(Debug, Clone)]
pub struct Node {
    pub name: String,
    pub active: bool,
    pub parent: Option<usize>,
    pub children: Vec<usize>,
    /// None for an object that is not laid out on a screen.
    pub layout: Option<Layout>,
    /// The classes of its scripts, where they were asked for.
    pub scripts: Vec<String>,
}

/// A rectangle on a screen: its lower left corner and its size, in the
/// screen's own units, y up.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub at: [f32; 2],
    pub size: [f32; 2],
}

/// A script on an object of a scene: the object it is on, and where its
/// own fields lie in the file.
#[derive(Debug, Clone, Copy)]
pub struct Behaviour {
    /// Its own number in the file, which is what points to it.
    pub id: i64,
    pub node: usize,
    fields: (usize, usize),
}

pub struct Scene {
    pub nodes: Vec<Node>,
    pub behaviours: Vec<Behaviour>,
    data: Vec<u8>,
}

struct Pointer {
    file: i32,
    path: i64,
}

fn pointer(r: &mut Reader) -> Result<Pointer> {
    Ok(Pointer { file: r.i32()?, path: r.i64()? })
}

fn pair(r: &mut Reader) -> Result<[f32; 2]> {
    Ok([r.f32()?, r.f32()?])
}

fn string(r: &mut Reader) -> Result<String> {
    let len = r.u32()? as usize;
    let text = String::from_utf8_lossy(r.bytes(len)?).into_owned();
    r.align(4);
    Ok(text)
}

/// The engine's class number for the build's own settings, which begin
/// with the list of its scenes.
const BUILD_SETTINGS: i32 = 141;

/// The build's scenes by their paths in the game's project, in order: the
/// scene at place `n` is the file `level<n>`.
pub fn scene_paths(install: &Install) -> Result<Vec<String>> {
    let path = install.root.join("EscapeFromTarkov_Data").join("globalgamemanagers");
    let data = std::fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
    let meta = SerializedFile::parse(&data).context("globalgamemanagers")?;
    let object = meta.objects.iter().find(|o| o.class_id == BUILD_SETTINGS).context("globalgamemanagers has no build settings")?;
    let mut r = Reader::new(&data[object.offset..object.offset + object.size]);
    let count = r.u32()? as usize;
    anyhow::ensure!(count < 10_000, "the build settings do not begin with the list of scenes");
    (0..count).map(|_| string(&mut r)).collect()
}

/// The equipment slots the panel's script names a view for, in the order
/// of its fields (`EquipmentTab`: earpiece, headwear, face cover, armband,
/// armour, eyewear, primary weapon, holster, secondary weapon, scabbard,
/// dogtag).
const GEAR_VIEWS: [&str; 11] =
    ["Earpiece", "Headwear", "FaceCover", "ArmBand", "ArmorVest", "Eyewear", "FirstPrimaryWeapon", "Holster", "SecondPrimaryWeapon", "Scabbard", "Dogtag"];

/// The inventory screen's equipment panel as the game lays it out: the
/// panel's size and, for each equipment slot it has a view for, that
/// view's left, top, width and height, in the screen's own units from the
/// panel's top left corner.
#[derive(Debug, Clone, Default)]
pub struct GearPanel {
    pub size: [f32; 2],
    pub slots: Vec<(String, [f32; 4])>,
    /// The scene file it was read from.
    pub file: String,
    /// The quick-use panel under the screen's middle: how wide and high
    /// it is, how far its lower edge is over the screen's and how far its
    /// middle is to the right of the screen's.
    pub quick: [f32; 4],
    /// Its views by the key each is for (`G`, `V`, `1` to `10`): the
    /// middle of each along the row as the scene has it, and its side.
    /// (The row is laid out again while the game runs; these are the
    /// places the scene was saved with.)
    pub quick_slots: Vec<(String, f32, f32)>,
}

impl GearPanel {
    /// The quick-use view a point of the screen lies on, by its key (`G`,
    /// `V`, `1` to `10`): the point in the engine's mouse units, from the
    /// screen's lower left corner, on a screen so wide and high. The row
    /// of views sits in the middle of its panel, and the screen's units
    /// are those of a screen 1080 high.
    pub fn quick_key_at(&self, point: [f32; 2], screen: [f32; 2]) -> Option<&str> {
        let [width, height, up, right] = self.quick;
        if width <= 0.0 || screen[1] <= 0.0 {
            return None;
        }
        let unit = screen[1] / 1080.0;
        let (x, y) = ((point[0] - screen[0] / 2.0) / unit, point[1] / unit);
        if y < up || y > up + height {
            return None;
        }
        let first = self.quick_slots.iter().map(|(_, middle, side)| middle - side / 2.0).fold(f32::MAX, f32::min);
        let last = self.quick_slots.iter().map(|(_, middle, side)| middle + side / 2.0).fold(f32::MIN, f32::max);
        let left = right - width / 2.0 + (width - (last - first)) / 2.0 - first;
        self.quick_slots.iter().find(|(_, middle, side)| (x - (left + middle)).abs() <= side / 2.0).map(|(key, _, _)| key.as_str())
    }

    /// Reads it from the install: the scene whose path names the game's
    /// common screens, and in it the panel that hangs under the inventory
    /// screen's own left side. The panel's size is its template's, which
    /// lies beside it (its own is set while the game runs).
    pub fn load(install: &Install) -> Result<Self> {
        const SCREEN: &str = "/InventoryScreen/Items Panel/LeftSide/Left Panel/Gear Panel";
        let scenes = scene_paths(install)?;
        let candidates = scenes.iter().enumerate().filter(|(_, path)| path.to_lowercase().contains("ui"));
        for (index, _) in candidates {
            let file = format!("level{index}");
            let Ok(scene) = Scene::load(install, &file, false) else { continue };
            let Some(panel) = (0..scene.nodes.len()).find(|&node| scene.nodes[node].name == "Gear Panel" && scene.path(node).ends_with(SCREEN)) else { continue };
            let beside = scene.nodes[panel].parent.and_then(|parent| scene.nodes[parent].children.iter().copied().find(|&child| scene.nodes[child].name == "Gear Panel Template"));
            let size = beside.and_then(|template| scene.nodes[template].layout).map_or([0.0, 0.0], |layout| layout.size_delta);
            // The panel's own script names its slot views one after
            // another (`EquipmentTab`: eleven of them and nothing else), so
            // it is the script on the panel whose fields are eleven
            // pointers to scripts on the panel's own children.
            let views = scene.behaviours.iter().filter(|behaviour| behaviour.node == panel).find_map(|behaviour| {
                // They follow whatever the script's parent classes keep.
                let fields = scene.fields(behaviour);
                (0..fields.len()).step_by(4).find_map(|start| {
                    let mut r = Reader::new(&fields[start..]);
                    let pointed: Vec<Pointer> = (0..GEAR_VIEWS.len()).map(|_| pointer(&mut r)).collect::<Result<_>>().ok()?;
                    let nodes: Vec<Option<usize>> = pointed.iter().map(|to| scene.owner(to.path).filter(|_| to.file == 0 && to.path != 0)).collect();
                    let on_panel = |node: &Option<usize>| node.is_some_and(|node| scene.nodes[node].parent == Some(panel));
                    // The dogtag's view is not on this panel; the ten others are.
                    (on_panel(&nodes[0]) && nodes.iter().filter(|node| on_panel(node)).count() >= 10).then_some(nodes)
                })
            });
            let views = views.with_context(|| format!("{file}: the equipment panel's script does not name its slot views"))?;
            let mut slots = Vec::new();
            for (slot, node) in GEAR_VIEWS.iter().zip(views) {
                let Some(rect) = node.and_then(|node| scene.rect_in(node, panel, size)) else { continue };
                slots.push((slot.to_string(), [rect.at[0], size[1] - rect.at[1] - rect.size[1], rect.size[0], rect.size[1]]));
            }
            anyhow::ensure!(!slots.is_empty(), "{file}: the equipment panel has no slot views");
            // The quick-use panel of the same screen: its views are named
            // for their keys ("Quick Slot 4 (off)").
            let (mut quick, mut quick_slots) = ([0.0; 4], Vec::new());
            let bar = (0..scene.nodes.len()).find(|&node| scene.nodes[node].name == "Quick Use Panel" && scene.path(node).ends_with("/InventoryScreen/Items Panel/Quick Use Panel"));
            if let Some((bar, layout)) = bar.and_then(|bar| Some((bar, scene.nodes[bar].layout?))) {
                quick = [layout.size_delta[0], layout.size_delta[1], layout.offset[1], layout.offset[0]];
                let mut below = scene.nodes[bar].children.clone();
                while let Some(node) = below.pop() {
                    below.extend(&scene.nodes[node].children);
                    let key = scene.nodes[node].name.strip_prefix("Quick Slot ").and_then(|rest| rest.split_whitespace().next());
                    if let (Some(key), Some(layout)) = (key, scene.nodes[node].layout) {
                        quick_slots.push((key.to_owned(), layout.offset[0], layout.size_delta[0]));
                    }
                }
                quick_slots.sort_by(|a, b| a.1.total_cmp(&b.1));
            }
            return Ok(Self { size, slots, file, quick, quick_slots });
        }
        anyhow::bail!("no scene of the install holds the inventory screen's equipment panel")
    }

    /// A slot view's rectangle by its name in the scene.
    pub fn slot(&self, name: &str) -> Option<[f32; 4]> {
        self.slots.iter().find(|(slot, _)| slot == name).map(|(_, rect)| *rect)
    }
}

impl Scene {
    /// Reads the scene file of that name (`level49`). With `scripts`, each
    /// object's scripts are named too, which reads the file the scripts
    /// are listed in as well.
    pub fn load(install: &Install, file: &str, scripts: bool) -> Result<Self> {
        let folder = install.root.join("EscapeFromTarkov_Data");
        Self::read(&folder, file, scripts)
    }

    fn read(folder: &Path, file: &str, scripts: bool) -> Result<Self> {
        let path = folder.join(file);
        let data = std::fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
        let meta = SerializedFile::parse(&data).with_context(|| file.to_owned())?;
        let bytes = |offset: usize, size: usize| data.get(offset..offset + size).context("an object lies outside its file");

        // The objects by their own number, then each transform onto its object.
        let mut nodes: Vec<Node> = Vec::new();
        let mut by_object: HashMap<i64, usize> = HashMap::new();
        let mut components: HashMap<i64, usize> = HashMap::new();
        for object in meta.objects.iter().filter(|o| o.class_id == class::GAME_OBJECT) {
            let mut r = Reader::new(bytes(object.offset, object.size)?);
            let count = r.i32()?.max(0) as usize;
            let index = nodes.len();
            for _ in 0..count {
                let component = pointer(&mut r)?;
                if component.file == 0 {
                    components.insert(component.path, index);
                }
            }
            r.skip(4)?; // layer
            let name = string(&mut r)?;
            r.skip(2)?; // tag
            let active = r.u8()? != 0;
            by_object.insert(object.path_id, index);
            nodes.push(Node { name, active, parent: None, children: Vec::new(), layout: None, scripts: Vec::new() });
        }
        let mut of_transform: HashMap<i64, usize> = HashMap::new();
        let mut fathers: Vec<(usize, i64)> = Vec::new();
        for object in meta.objects.iter().filter(|o| o.class_id == class::TRANSFORM || o.class_id == RECT_TRANSFORM) {
            let mut r = Reader::new(bytes(object.offset, object.size)?);
            let owner = pointer(&mut r)?;
            let Some(&index) = by_object.get(&owner.path).filter(|_| owner.file == 0) else { continue };
            r.skip(16 + 12)?; // turn, place
            let scale = pair(&mut r)?;
            r.skip(4)?;
            let children = r.i32()?.max(0) as usize;
            r.skip(children * 12)?;
            let father = pointer(&mut r)?;
            of_transform.insert(object.path_id, index);
            if father.file == 0 && father.path != 0 {
                fathers.push((index, father.path));
            }
            if object.class_id == RECT_TRANSFORM {
                let (anchor_min, anchor_max, offset, size_delta, pivot) = (pair(&mut r)?, pair(&mut r)?, pair(&mut r)?, pair(&mut r)?, pair(&mut r)?);
                nodes[index].layout = Some(Layout { anchor_min, anchor_max, offset, size_delta, pivot, scale });
            }
        }
        // Children in the order the file has their transforms, which is
        // not the order on the screen; a parent's own list would be, and
        // is not needed for where a thing sits.
        for (index, father) in fathers {
            if let Some(&parent) = of_transform.get(&father) {
                nodes[index].parent = Some(parent);
                nodes[parent].children.push(index);
            }
        }
        // Every script with the object it is on and where its own fields
        // begin: after the object, the enabled flag, the script and a name.
        let mut behaviours = Vec::new();
        for object in meta.objects.iter().filter(|o| o.class_id == class::MONO_BEHAVIOUR) {
            let mut r = Reader::new(bytes(object.offset, object.size)?);
            let owner = pointer(&mut r)?;
            let Some(&node) = by_object.get(&owner.path).filter(|_| owner.file == 0) else { continue };
            if r.skip(4 + 12).is_err() || string(&mut r).is_err() {
                continue;
            }
            let read = object.size - r.remaining();
            behaviours.push(Behaviour { id: object.path_id, node, fields: (object.offset + read, object.size - read) });
        }
        if scripts {
            // A script names its class in a file of its own, listed among
            // the scene's outside files.
            let mut outside: HashMap<i32, (Vec<u8>, SerializedFile)> = HashMap::new();
            for object in meta.objects.iter().filter(|o| o.class_id == class::MONO_BEHAVIOUR) {
                let mut r = Reader::new(bytes(object.offset, object.size)?);
                let owner = pointer(&mut r)?;
                r.skip(4)?; // enabled, padded
                let script = pointer(&mut r)?;
                let Some(&index) = by_object.get(&owner.path).filter(|_| owner.file == 0) else { continue };
                let Some(listed) = (script.file > 0).then(|| meta.externals.get(script.file as usize - 1)).flatten() else { continue };
                if !outside.contains_key(&script.file) {
                    let name = listed.rsplit('/').next().unwrap_or(listed);
                    let Ok(data) = std::fs::read(folder.join(name)) else { continue };
                    let Ok(meta) = SerializedFile::parse(&data) else { continue };
                    outside.insert(script.file, (data, meta));
                }
                let (data, scripts) = &outside[&script.file];
                let Some(found) = scripts.objects.iter().find(|o| o.path_id == script.path && o.class_id == class::MONO_SCRIPT) else { continue };
                // A script object begins with its name, which is its class's.
                if let Ok(name) = string(&mut Reader::new(&data[found.offset..found.offset + found.size])) {
                    nodes[index].scripts.push(name);
                }
            }
        }
        Ok(Self { nodes, behaviours, data })
    }

    /// A script's own fields as the file has them.
    pub fn fields(&self, behaviour: &Behaviour) -> &[u8] {
        &self.data[behaviour.fields.0..behaviour.fields.0 + behaviour.fields.1]
    }

    /// The object the script of that number is on.
    pub fn owner(&self, id: i64) -> Option<usize> {
        self.behaviours.iter().find(|behaviour| behaviour.id == id).map(|behaviour| behaviour.node)
    }

    /// The objects of that name.
    pub fn named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = usize> + 'a {
        self.nodes.iter().enumerate().filter(move |(_, node)| node.name == name).map(|(index, _)| index)
    }

    /// The first object of that name below another, however deep.
    pub fn below(&self, above: usize, name: &str) -> Option<usize> {
        self.nodes[above].children.iter().find_map(|&child| if self.nodes[child].name == name { Some(child) } else { self.below(child, name) })
    }

    /// The path of names from the scene's top down to an object.
    pub fn path(&self, index: usize) -> String {
        match self.nodes[index].parent {
            Some(parent) => format!("{}/{}", self.path(parent), self.nodes[index].name),
            None => self.nodes[index].name.clone(),
        }
    }

    /// An object's rectangle within the rectangle of one it hangs from,
    /// however far above, whose size is given: the engine's layout of a
    /// screen transform, level by level. An object on the way that is not
    /// laid out on a screen, or `above` not being above it, gives none.
    /// Sizes that a layout script would set while the game runs are as
    /// the scene file has them.
    pub fn rect_in(&self, index: usize, above: usize, size: [f32; 2]) -> Option<Rect> {
        if index == above {
            return Some(Rect { at: [0.0, 0.0], size });
        }
        let outer = self.rect_in(self.nodes[index].parent?, above, size)?;
        let layout = self.nodes[index].layout?;
        let mut rect = Rect { at: [0.0; 2], size: [0.0; 2] };
        for axis in 0..2 {
            let (low, high) = (layout.anchor_min[axis] * outer.size[axis], layout.anchor_max[axis] * outer.size[axis]);
            let own = (high - low + layout.size_delta[axis]) * layout.scale[axis];
            let pivot = low + (high - low) * layout.pivot[axis] + layout.offset[axis];
            rect.size[axis] = own;
            rect.at[axis] = outer.at[axis] + pivot - layout.pivot[axis] * own;
        }
        Some(rect)
    }
}
