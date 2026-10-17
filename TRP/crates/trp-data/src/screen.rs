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

use anyhow::{bail, Context, Result};

use crate::install::Install;
use crate::unity::reader::Reader;
use crate::unity::{class, SerializedFile};

/// The engine's class number for a transform laid out on a screen.
const RECT_TRANSFORM: i32 = 224;

/// How a screen transform is laid out in what it hangs from, in the
/// engine's terms: the two anchors as shares of the parent's rectangle,
/// the pivot's offset from the anchors, the size added to the anchors'
/// spread, and the pivot as a share of its own rectangle.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Layout {
    pub anchor_min: [f32; 2],
    pub anchor_max: [f32; 2],
    pub offset: [f32; 2],
    pub size_delta: [f32; 2],
    pub pivot: [f32; 2],
    pub scale: [f32; 2],
    /// Its turn about the screen's own axis, in degrees, counter-clockwise
    /// as the screen is seen (its transform's rotation about z).
    pub turn: f32,
}

/// A rotation's turn about z, in degrees, from its quaternion (x, y, z, w):
/// what a screen's objects are turned by (none is turned about another axis).
fn z_turn([x, y, z, w]: [f32; 4]) -> f32 {
    let _ = (x, y);
    (2.0 * z.atan2(w)).to_degrees()
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
    /// Whether it is switched on (`Behaviour.enabled`): an image or a text
    /// switched off draws nothing.
    pub enabled: bool,
    fields: (usize, usize),
}

pub struct Scene {
    pub nodes: Vec<Node>,
    pub behaviours: Vec<Behaviour>,
    /// Each script's class by its number, where they were asked for.
    classes: HashMap<i64, String>,
    /// Each object by its own number, which is what points to it.
    objects: HashMap<i64, usize>,
    /// Each object by its transform's number, which is what a screen's
    /// scripts point to for a place on it (`RectTransform`).
    transforms: HashMap<i64, usize>,
    /// Each object by the numbers of its engine's own parts (a
    /// `CanvasGroup` and the rest), which scripts point to as well.
    components: HashMap<i64, usize>,
    /// Each of those parts' engine class (`Canvas` 223, `CanvasGroup` 225
    /// and the rest), by its number.
    component_classes: HashMap<i64, i32>,
    /// The files a pointer's first number counts in, from 1.
    pub externals: Vec<String>,
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
/// The line at the screen's foot that says the game's version
/// (`PreloaderUI._alphaVersionLabel`, `Preloader UI/BottomPanel/Content/
/// UpperPart/AlphaLabel` in the scene whose path ends in
/// `UI/PreloaderUIScene.unity`), which shows over every screen: its text's
/// look, and how high it lies over the screen's foot (its column's padding
/// below it, `Content`'s `VerticalLayoutGroup`).
pub fn version_label(install: &Install) -> Result<(ScenePanel, f32)> {
    const LABEL: &str = "/BottomPanel/Content/UpperPart/AlphaLabel";
    let index = scene_paths(install)?.iter().position(|path| path.ends_with("UI/PreloaderUIScene.unity")).context("the build has no preloader's scene")?;
    let file = format!("level{index}");
    let scene = Scene::load(install, &file, true)?;
    let label = (0..scene.nodes.len()).find(|&node| scene.path(node).ends_with(LABEL)).with_context(|| format!("{file} has no version label"))?;
    let below = scene.nodes[label]
        .parent
        .and_then(|upper| scene.nodes[upper].parent)
        .and_then(|content| scene.behaviour(content, "VerticalLayoutGroup"))
        .and_then(|group| row_group(scene.fields(group)))
        .map_or(0.0, |group| group.padding[3]);
    Ok((ScenePanel { file: file.clone(), ..ScenePanel::of(&scene, label) }, below))
}

/// The game's simple tooltip (`ItemUiContext.Tooltip`, a `SimpleTooltip`:
/// `Preloader UI/UIContext/SimpleTooltip` in the preloader's scene), with
/// all under it: its ground, frame, frame's shadow and text.
pub fn simple_tooltip(install: &Install) -> Result<ScenePanel> {
    const TOOLTIP: &str = "/UIContext/SimpleTooltip";
    let index = scene_paths(install)?.iter().position(|path| path.ends_with("UI/PreloaderUIScene.unity")).context("the build has no preloader's scene")?;
    let file = format!("level{index}");
    let scene = Scene::load(install, &file, true)?;
    let tooltip = (0..scene.nodes.len()).find(|&node| scene.path(node).ends_with(TOOLTIP)).with_context(|| format!("{file} has no simple tooltip"))?;
    Ok(ScenePanel { file, ..ScenePanel::of(&scene, tooltip) })
}

/// The game's menu of a right click on a thing (`ItemUiContext.ContextMenu`,
/// a `SimpleContextMenu`: `Preloader UI/UIContext/Context Menu Area/
/// ItemContextMenu` in the preloader's scene), with all under it: its
/// ground and shadow, and the templates of a line (`Button Template`), of
/// the wish list's (`Wishlist Template`) and of Discard's (`Dispose
/// Template`).
pub fn context_menu(install: &Install) -> Result<ScenePanel> {
    const MENU: &str = "/UIContext/Context Menu Area/ItemContextMenu";
    let index = scene_paths(install)?.iter().position(|path| path.ends_with("UI/PreloaderUIScene.unity")).context("the build has no preloader's scene")?;
    let file = format!("level{index}");
    let scene = Scene::load(install, &file, true)?;
    let menu = (0..scene.nodes.len()).find(|&node| scene.path(node).ends_with(MENU)).with_context(|| format!("{file} has no item's menu"))?;
    Ok(ScenePanel { file, ..ScenePanel::of(&scene, menu) })
}

/// The game's window that shows a thing (`InfoWindow`, with its
/// `ItemSpecificationPanel` and `ItemInfoWindowLabels`): the prefab the
/// preloader's item context makes it from (`ItemUiContext._infoWindowTemplate`,
/// in the scene's shared file), found among the prefabs its script points
/// to by its own script.
pub fn info_window(install: &Install) -> Result<ScenePanel> {
    let index = scene_paths(install)?.iter().position(|path| path.ends_with("UI/PreloaderUIScene.unity")).context("the build has no preloader's scene")?;
    let file = format!("level{index}");
    let scene = Scene::load(install, &file, true)?;
    let context = (0..scene.nodes.len()).find(|&node| scene.path(node).ends_with("/UIContext")).with_context(|| format!("{file} has no item context"))?;
    let panel = ScenePanel::of(&scene, context);
    let fields = panel.script(0, "ItemUiContext").context("the item context's script")?;
    let mut tried = std::collections::HashSet::new();
    for start in (0..fields.len().saturating_sub(11)).step_by(4) {
        let mut r = Reader::new(&fields[start..]);
        let to = pointer(&mut r)?;
        if to.file < 1 || to.path <= 0 || to.path >= 1 << 40 || !tried.insert((to.file, to.path)) {
            continue;
        }
        let Some(shared) = panel.file_of(to.file).filter(|name| name.starts_with("sharedassets")) else { continue };
        let Ok(prefab) = Scene::load_prefab_of_script(install, &shared, to.path) else { continue };
        let window = ScenePanel { file: shared.clone(), ..ScenePanel::of(&prefab, 0) };
        if window.script(0, "InfoWindow").is_some() {
            return Ok(window);
        }
    }
    bail!("the item context points to no window that shows a thing")
}

/// The health's effects the game has an icon for, in the order of the
/// hard settings' table of them (`StaticIcons.EffectSprites`' fields).
pub const EFFECT_ICONS: [&str; 28] = [
    "Painkiller",
    "Tremor",
    "Fracture",
    "Exhaustion",
    "TunnelVision",
    "Dehydration",
    "Contusion",
    "Wound",
    "LightBleeding",
    "HeavyBleeding",
    "Pain",
    "Berserk",
    "Stun",
    "StimulatorBuff",
    "StimulatorDebuff",
    "ChronicStaminaFatigue",
    "Encumbered",
    "OverEncumbered",
    "Intoxication",
    "LethalIntoxication",
    "ImmunityPreventedContamination",
    "HalloweenBuff",
    "MisfireEffect",
    "PanicEffect",
    "MildMusclePain",
    "SevereMusclePain",
    "ZombieInfection",
    "Frostbite",
];

/// Each effect's icon (`EFFECT_ICONS`) and its sprite's file and number:
/// the table is the last field of the Resource `Static Icons`
/// (`StaticIcons.EffectIcons`, an `EffectSprites` of one sprite's pointer a
/// field), read from its end.
pub fn effect_icons(install: &Install) -> Result<Vec<(&'static str, (String, i64))>> {
    let paths = resource_paths(install)?;
    let (_, file, id) = paths.iter().find(|(path, ..)| path == "static icons").context("no resource is at static icons")?;
    let (_, bytes) = object_bytes(install, file, *id)?;
    let start = bytes.len().checked_sub(EFFECT_ICONS.len() * 12).context("the static icons are too short")?;
    let mut r = Reader::new(&bytes[start..]);
    let mut icons = Vec::new();
    for name in EFFECT_ICONS {
        let to = pointer(&mut r)?;
        if to.file == 0 && to.path != 0 {
            icons.push((name, (file.clone(), to.path)));
        }
    }
    Ok(icons)
}

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
        let mut lists: Vec<(usize, Vec<i64>)> = Vec::new();
        for object in meta.objects.iter().filter(|o| o.class_id == class::TRANSFORM || o.class_id == RECT_TRANSFORM) {
            let mut r = Reader::new(bytes(object.offset, object.size)?);
            let owner = pointer(&mut r)?;
            let Some(&index) = by_object.get(&owner.path).filter(|_| owner.file == 0) else { continue };
            let turn = z_turn([r.f32()?, r.f32()?, r.f32()?, r.f32()?]);
            r.skip(12)?; // place
            let scale = pair(&mut r)?;
            r.skip(4)?;
            let children = r.i32()?.max(0) as usize;
            let list: Vec<i64> = (0..children).map(|_| pointer(&mut r).map(|child| child.path)).collect::<Result<_>>()?;
            let father = pointer(&mut r)?;
            of_transform.insert(object.path_id, index);
            if father.file == 0 && father.path != 0 {
                fathers.push((index, father.path));
            }
            lists.push((index, list));
            if object.class_id == RECT_TRANSFORM {
                let (anchor_min, anchor_max, offset, size_delta, pivot) = (pair(&mut r)?, pair(&mut r)?, pair(&mut r)?, pair(&mut r)?, pair(&mut r)?);
                nodes[index].layout = Some(Layout { anchor_min, anchor_max, offset, size_delta, pivot, scale, turn });
            }
        }
        for (index, father) in fathers {
            if let Some(&parent) = of_transform.get(&father) {
                nodes[index].parent = Some(parent);
            }
        }
        // Children in the order of their parent's own list, which is the
        // order the screen draws them in (each over the one before).
        for (index, list) in lists {
            nodes[index].children = list.iter().filter_map(|child| of_transform.get(child).copied()).filter(|&child| nodes[child].parent == Some(index)).collect();
        }
        // Every script with the object it is on and where its own fields
        // begin: after the object, the enabled flag, the script and a name.
        let mut behaviours = Vec::new();
        for object in meta.objects.iter().filter(|o| o.class_id == class::MONO_BEHAVIOUR) {
            let mut r = Reader::new(bytes(object.offset, object.size)?);
            let owner = pointer(&mut r)?;
            let Some(&node) = by_object.get(&owner.path).filter(|_| owner.file == 0) else { continue };
            let Ok(enabled) = r.u32().map(|word| word & 0xff != 0) else { continue };
            if r.skip(12).is_err() || string(&mut r).is_err() {
                continue;
            }
            let read = object.size - r.remaining();
            behaviours.push(Behaviour { id: object.path_id, node, enabled, fields: (object.offset + read, object.size - read) });
        }
        let mut classes = HashMap::new();
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
                    classes.insert(object.path_id, name.clone());
                    nodes[index].scripts.push(name);
                }
            }
        }
        let component_classes = meta.objects.iter().filter(|o| components.contains_key(&o.path_id)).map(|o| (o.path_id, o.class_id)).collect();
        Ok(Self { nodes, behaviours, classes, objects: by_object, transforms: of_transform, components, component_classes, externals: meta.externals.clone(), data })
    }

    /// The engine classes of an object's parts (its transform, a canvas, a
    /// group, its scripts as 114), with each one's number.
    pub fn parts_of(&self, node: usize) -> Vec<(i64, i32)> {
        let mut parts: Vec<(i64, i32)> = self.components.iter().filter(|(_, &at)| at == node).map(|(&id, _)| (id, self.component_classes.get(&id).copied().unwrap_or(0))).collect();
        parts.sort();
        parts
    }

    /// The class of a script, where the scene was read with its scripts.
    pub fn class_of(&self, behaviour: &Behaviour) -> Option<&str> {
        self.classes.get(&behaviour.id).map(String::as_str)
    }

    /// The script of that class on an object.
    pub fn behaviour(&self, node: usize, class: &str) -> Option<&Behaviour> {
        self.behaviours.iter().find(|behaviour| behaviour.node == node && self.class_of(behaviour) == Some(class))
    }

    /// A prefab of one of the build's own files (`resources.assets`) and
    /// all under it, read object by object from its root game object, as a
    /// scene of its own with its scripts named: the file is some 300 MB,
    /// and only what the prefab is made of is read.
    pub fn load_prefab(install: &Install, file: &str, root: i64) -> Result<Self> {
        let mut build = BuildFile::open(install, file)?;
        Self::load_prefab_from(install, &mut build, root)
    }

    /// The same from a file already open.
    fn load_prefab_from(install: &Install, build: &mut BuildFile, root: i64) -> Result<Self> {
        let externals = build.meta.externals.clone();
        let mut reader = PrefabReader {
            install: install.clone(),
            build,
            scene: Scene { nodes: Vec::new(), behaviours: Vec::new(), classes: HashMap::new(), objects: HashMap::new(), transforms: HashMap::new(), components: HashMap::new(), component_classes: HashMap::new(), externals, data: Vec::new() },
            outside: HashMap::new(),
        };
        reader.visit(root, None)?;
        Ok(reader.scene)
    }

    /// The prefab a script of one of the build's own files is on (a
    /// template a scene points to by its script): the script's game object
    /// and all under it, as `load_prefab` reads them.
    pub fn load_prefab_of_script(install: &Install, file: &str, script: i64) -> Result<Self> {
        let mut build = BuildFile::open(install, file)?;
        let object = *build.meta.object(script).with_context(|| format!("no object {script} in {file}"))?;
        let bytes = build.bytes(object.offset, object.size)?;
        let owner = pointer(&mut Reader { data: &bytes, pos: 0, big_endian: build.meta.big_endian })?;
        Self::load_prefab(install, file, owner.path)
    }

    /// A script's own fields as the file has them.
    pub fn fields(&self, behaviour: &Behaviour) -> &[u8] {
        &self.data[behaviour.fields.0..behaviour.fields.0 + behaviour.fields.1]
    }

    /// The object the script of that number is on.
    pub fn owner(&self, id: i64) -> Option<usize> {
        self.behaviours.iter().find(|behaviour| behaviour.id == id).map(|behaviour| behaviour.node)
    }

    /// The object that a pointer of the scene's own leads to: the object
    /// itself, or one of its scripts.
    pub fn pointed(&self, id: i64) -> Option<usize> {
        self.objects.get(&id).copied().or_else(|| self.owner(id))
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
        Some(place(self.nodes[index].layout?, outer))
    }
}

/// The engine's layout of a screen transform in its parent's rectangle.
pub fn place(layout: Layout, outer: Rect) -> Rect {
    let mut rect = Rect { at: [0.0; 2], size: [0.0; 2] };
    for axis in 0..2 {
        let (low, high) = (layout.anchor_min[axis] * outer.size[axis], layout.anchor_max[axis] * outer.size[axis]);
        let own = (high - low + layout.size_delta[axis]) * layout.scale[axis];
        let pivot = low + (high - low) * layout.pivot[axis] + layout.offset[axis];
        rect.size[axis] = own;
        rect.at[axis] = outer.at[axis] + pivot - layout.pivot[axis] * own;
    }
    rect
}

/// How an image of a screen is drawn (`UnityEngine.UI.Image`), read by the
/// engine's own order of its fields: its sprite, its colour, and how the
/// sprite fills its rectangle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImageLook {
    /// The sprite: the file it is in, counted from 1 among the scene's
    /// outside files (0 is the scene's own), and its number there.
    pub sprite: Option<(i32, i64)>,
    pub color: [f32; 4],
    /// Simple, sliced, tiled or filled (`Image.Type`, 0 to 3).
    pub kind: u32,
    pub preserve_aspect: bool,
    pub fill_center: bool,
    /// Horizontal, vertical, radial 90, 180 or 360 (`Image.FillMethod`).
    pub fill_method: u32,
    pub fill_amount: f32,
    pub fill_clockwise: bool,
    pub fill_origin: u32,
    /// The sprite's pixels per unit is multiplied by this.
    pub pixels_per_unit: f32,
}

/// How a text of a screen is drawn (`TMPro.TextMeshProUGUI`), read by the
/// engine's own order of its fields: what it says as saved, its colour,
/// its size and how it lies in its rectangle.
#[derive(Debug, Clone, PartialEq)]
pub struct TextLook {
    pub text: String,
    /// Its font (a `TMP_FontAsset`): the file it is in, counted from 1
    /// among the scene's outside files, and its number there.
    pub font: (i32, i64),
    pub color: [f32; 4],
    pub size: f32,
    /// The least and the most size, where it is sized to fit.
    pub auto_size: Option<[f32; 2]>,
    /// Bold 1, italic 2, lower case 8, upper case 16 (`FontStyles`).
    pub style: u32,
    /// Left 1, centre 2, right 4 (`HorizontalAlignmentOptions`).
    pub horizontal: u32,
    /// Top 256, middle 512, bottom 1024 (`VerticalAlignmentOptions`).
    pub vertical: u32,
    /// What is added after each letter, in hundredths of the size
    /// (`m_characterSpacing`).
    pub spacing: f32,
    /// Whether the font's kerning is applied between two letters
    /// (`m_enableKerning`), and the margins inside its rectangle (left,
    /// top, right, bottom: `m_margin`).
    pub kerning: bool,
    pub margin: [f32; 4],
    /// The material it is drawn with (`m_sharedMaterial`): its font
    /// asset's own, or a preset of its own (an outline, a shadow), by the
    /// file it is in (counted as `font`) and its number; none where unset.
    pub material: Option<(i32, i64)>,
}

fn color(r: &mut Reader) -> Result<[f32; 4]> {
    Ok([r.f32()?, r.f32()?, r.f32()?, r.f32()?])
}

/// A flag, which the engine keeps as one byte padded to four.
fn flag(r: &mut Reader) -> Result<bool> {
    Ok(r.u32()? & 0xff != 0)
}

/// What every graphic of a screen begins with (`Graphic`, then
/// `MaskableGraphic`): its material, its colour, whether it takes the
/// mouse and how far out, whether a mask cuts it, and whom it tells when it
/// is cut. None where that is anyone, whose calls are not read past.
fn graphic(r: &mut Reader) -> Result<Option<[f32; 4]>> {
    r.skip(12)?;
    let color = color(r)?;
    r.skip(4 + 16 + 4)?;
    Ok((r.i32()? == 0).then_some(color))
}

/// An image's look from its script's fields.
pub fn image_look(fields: &[u8]) -> Option<ImageLook> {
    let read = |r: &mut Reader| -> Result<Option<ImageLook>> {
        let Some(color) = graphic(r)? else { return Ok(None) };
        let sprite = pointer(r)?;
        let (kind, preserve_aspect, fill_center, fill_method, fill_amount) = (r.u32()?, flag(r)?, flag(r)?, r.u32()?, r.f32()?);
        let (fill_clockwise, fill_origin, _sprite_mesh, pixels_per_unit) = (flag(r)?, r.u32()?, flag(r)?, r.f32()?);
        let sprite = (sprite.path != 0).then_some((sprite.file, sprite.path));
        Ok(Some(ImageLook { sprite, color, kind, preserve_aspect, fill_center, fill_method, fill_amount, fill_clockwise, fill_origin, pixels_per_unit }))
    };
    read(&mut Reader::new(fields)).ok().flatten()
}

/// A text's look from its script's fields.
pub fn text_look(fields: &[u8]) -> Option<TextLook> {
    let read = |r: &mut Reader| -> Result<Option<TextLook>> {
        if graphic(r)?.is_none() {
            return Ok(None);
        }
        let text = string(r)?;
        r.skip(4)?; // right to left
        let font = pointer(r)?;
        let material = pointer(r)?;
        let materials = r.i32()?.max(0) as usize;
        r.skip(12 * materials + 12)?;
        let materials = r.i32()?.max(0) as usize;
        r.skip(12 * materials + 4)?; // and the colour in bytes
        let color = color(r)?;
        // A gradient and how, the sprites, a style sheet, the face's colour.
        r.skip(4 + 4 + 64 + 12 + 12 + 4 + 12 + 4 + 4 + 4)?;
        let size = r.f32()?;
        r.skip(4 + 4)?; // the size it began with, the weight
        let (auto, least, most) = (flag(r)?, r.f32()?, r.f32()?);
        let (style, horizontal, vertical) = (r.u32()?, r.u32()?, r.u32()?);
        r.skip(4)?; // the alignment as it was once kept
        let spacing = r.f32()?;
        // The spacing of words, lines and paragraphs, the narrowing, the
        // wrapping and its ratio, the overflow, two linked texts; then
        // the kerning; the padding twice, rich text, control characters,
        // orthographic, culling, two mappings, the line offset, the
        // sorting, the static scale, the buffer, the descender, the
        // page; then the margins.
        r.skip(4 * 5 + 4 + 4 + 4 + 12 + 12)?;
        let kerning = flag(r)?;
        r.skip(4 * 14)?;
        let margin = [r.f32()?, r.f32()?, r.f32()?, r.f32()?];
        let material = (material.path != 0).then_some((material.file, material.path));
        Ok(Some(TextLook { text, font: (font.file, font.path), color, size, auto_size: auto.then_some([least, most]), style, horizontal, vertical, spacing, kerning, margin, material }))
    };
    read(&mut Reader::new(fields)).ok().flatten()
}

/// A part of a screen as its scene has it: an object and everything under
/// it, in the order the screen draws them (each over those before), each
/// with its layout and what it draws.
#[derive(Debug, Clone, Default)]
pub struct ScenePanel {
    pub nodes: Vec<PanelNode>,
    /// The scene's outside files, which its sprites' pointers count in.
    pub externals: Vec<String>,
    /// The file it is read from, where a pointer that names no outside
    /// file leads (empty where the caller knows it).
    pub file: String,
    /// Each of its objects, and each script on them, by the number a
    /// pointer of the scene's own gives it.
    ids: HashMap<i64, usize>,
    /// Each of its objects by its transform's number.
    transforms: HashMap<i64, usize>,
}

#[derive(Debug, Clone)]
pub struct PanelNode {
    pub name: String,
    /// Its path below the panel's top (`Chest/ProgressBarImage/Value`);
    /// the top's own is empty.
    pub path: String,
    pub parent: Option<usize>,
    /// Whether it is on as the scene has it.
    pub active: bool,
    pub layout: Layout,
    pub image: Option<ImageLook>,
    pub text: Option<TextLook>,
    /// Its scripts: each one's class and own fields.
    pub scripts: Vec<(String, Vec<u8>)>,
    /// The row its children are laid out in while the game runs, if it
    /// has one, and what it asks of the row it is in.
    pub group: Option<RowGroup>,
    pub element: Option<Element>,
}

/// A horizontal layout group (`HorizontalLayoutGroup`), read by the
/// engine's order of its fields: its padding (left, right, top, bottom),
/// the corner it lays out from (`TextAnchor`, 0 to 8), the space between
/// its children, whether it makes them grow and whether it sets their
/// size, across and up and down.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct RowGroup {
    /// A column (`VerticalLayoutGroup`) rather than a row.
    pub vertical: bool,
    pub padding: [f32; 4],
    pub alignment: u32,
    pub spacing: f32,
    pub expand: [bool; 2],
    pub control: [bool; 2],
}

/// What a child asks of the group it is in (`LayoutElement`): its least,
/// preferred and flexible width and height, each below 0 where not set,
/// or to be left out of the layout.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Element {
    pub ignore: bool,
    pub min: [f32; 2],
    pub preferred: [f32; 2],
    pub flexible: [f32; 2],
}

fn row_group(fields: &[u8]) -> Option<RowGroup> {
    let read = |r: &mut Reader| -> Result<RowGroup> {
        let padding = [r.i32()? as f32, r.i32()? as f32, r.i32()? as f32, r.i32()? as f32];
        let (alignment, spacing) = (r.u32()?, r.f32()?);
        let expand = [flag(r)?, flag(r)?];
        let control = [flag(r)?, flag(r)?];
        Ok(RowGroup { vertical: false, padding, alignment, spacing, expand, control })
    };
    read(&mut Reader::new(fields)).ok()
}

fn element(fields: &[u8]) -> Option<Element> {
    let read = |r: &mut Reader| -> Result<Element> {
        let ignore = flag(r)?;
        let (min, preferred, flexible) = (pair(r)?, pair(r)?, pair(r)?);
        Ok(Element { ignore, min, preferred, flexible })
    };
    read(&mut Reader::new(fields)).ok()
}

/// Where an object of a panel is drawn: its left, top, width and height
/// from the top's top left corner, y down, and whether it is drawn
/// mirrored across and up and down (a scale of -1 on it or above it).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placed {
    pub rect: [f32; 4],
    pub flip: [bool; 2],
}

impl ScenePanel {
    /// The object of a scene read with its scripts, and all under it.
    pub fn of(scene: &Scene, top: usize) -> Self {
        fn walk(scene: &Scene, index: usize, parent: Option<usize>, path: String, panel: &mut ScenePanel, transforms: &HashMap<usize, i64>, parts: &HashMap<usize, Vec<i64>>) {
            let node = &scene.nodes[index];
            let Some(layout) = node.layout else { return };
            let here = panel.nodes.len();
            // (An image or a text switched off draws nothing.)
            let on = |class: &str| scene.behaviour(index, class).filter(|script| script.enabled);
            let image = on("Image").and_then(|script| image_look(scene.fields(script)));
            let text = ["CustomTextMeshProUGUI", "TextMeshProUGUI"].iter().find_map(|class| on(class)).and_then(|script| text_look(scene.fields(script)));
            let mut scripts = Vec::new();
            for behaviour in scene.behaviours.iter().filter(|behaviour| behaviour.node == index) {
                panel.ids.insert(behaviour.id, here);
                scripts.push((scene.class_of(behaviour).unwrap_or("").to_owned(), scene.fields(behaviour).to_vec()));
            }
            if let Some((&id, _)) = scene.objects.iter().find(|(_, &object)| object == index) {
                panel.ids.insert(id, here);
            }
            if let Some(&id) = transforms.get(&index) {
                panel.transforms.insert(id, here);
            }
            for &id in parts.get(&index).into_iter().flatten() {
                panel.ids.entry(id).or_insert(here);
            }
            let group = scene
                .behaviour(index, "HorizontalLayoutGroup")
                .and_then(|script| row_group(scene.fields(script)))
                .or_else(|| scene.behaviour(index, "VerticalLayoutGroup").and_then(|script| row_group(scene.fields(script))).map(|group| RowGroup { vertical: true, ..group }));
            let element = scene.behaviour(index, "LayoutElement").and_then(|script| element(scene.fields(script)));
            panel.nodes.push(PanelNode { name: node.name.clone(), path: path.clone(), parent, active: node.active, layout, image, text, scripts, group, element });
            for &child in &node.children {
                let name = &scene.nodes[child].name;
                let below = if path.is_empty() { name.clone() } else { format!("{path}/{name}") };
                walk(scene, child, Some(here), below, panel, transforms, parts);
            }
        }
        let mut panel = Self { externals: scene.externals.clone(), ..Self::default() };
        let transforms: HashMap<usize, i64> = scene.transforms.iter().map(|(&id, &index)| (index, id)).collect();
        let mut parts: HashMap<usize, Vec<i64>> = HashMap::new();
        // (A transform's number stays its place's alone: `transform_at`.)
        for (&id, &index) in scene.components.iter().filter(|(id, _)| !scene.transforms.contains_key(id)) {
            parts.entry(index).or_default().push(id);
        }
        walk(scene, top, None, String::new(), &mut panel, &transforms, &parts);
        panel
    }

    /// One of its objects by its path below the top.
    pub fn find(&self, path: &str) -> Option<usize> {
        self.nodes.iter().position(|node| node.path == path)
    }

    /// The fields of a script of that class on an object.
    pub fn script(&self, index: usize, class: &str) -> Option<&[u8]> {
        self.nodes[index].scripts.iter().find(|(name, _)| name == class).map(|(_, fields)| fields.as_slice())
    }

    /// The object of the panel that the pointer at that place in a
    /// script's fields leads to (none for one that is empty or leads
    /// outside the panel).
    pub fn pointer_at(&self, fields: &[u8], at: usize) -> Option<usize> {
        let to = pointer(&mut Reader::new(fields.get(at..)?)).ok()?;
        (to.file == 0 && to.path != 0).then(|| self.ids.get(&to.path).copied()).flatten()
    }

    /// The object of the panel whose place (its `RectTransform`) the
    /// pointer at that place in a script's fields leads to.
    pub fn transform_at(&self, fields: &[u8], at: usize) -> Option<usize> {
        let to = pointer(&mut Reader::new(fields.get(at..)?)).ok()?;
        (to.file == 0 && to.path != 0).then(|| self.transforms.get(&to.path).copied()).flatten()
    }

    /// Whether an object is on as the scene has it, it and everything
    /// between it and the top (the top itself is taken as shown).
    pub fn shown(&self, index: usize) -> bool {
        let node = &self.nodes[index];
        match node.parent {
            Some(parent) => node.active && self.shown(parent),
            None => true,
        }
    }

    /// Where an object is drawn with the top that size, by the engine's
    /// layout: each object's rectangle laid out in its parent's, unscaled,
    /// then scaled about its pivot with all under it.
    pub fn placed(&self, index: usize, size: [f32; 2]) -> Placed {
        // The object's own rectangle in its own frame, and that frame's
        // origin and scale in the top's.
        fn frame(panel: &ScenePanel, index: usize, size: [f32; 2]) -> (Rect, [f32; 2], [f32; 2]) {
            let Some(parent) = panel.nodes[index].parent else { return (Rect { at: [0.0, 0.0], size }, [0.0, 0.0], [1.0, 1.0]) };
            let (outer, origin, scale) = frame(panel, parent, size);
            let layout = panel.nodes[index].layout;
            // Laid out by its parent's row while the game runs, or by its
            // own anchors.
            let own = match panel.in_row(parent, index, outer) {
                Some(own) => own,
                None => {
                    // Sized to what it holds where a fitter says so and it
                    // lays out what it holds (`ContentSizeFitter`: least 1,
                    // preferred 2), about its pivot.
                    let mut own = place(Layout { scale: [1.0, 1.0], ..layout }, outer);
                    if let (Some(_), Some(fitter)) = (panel.nodes[index].group, panel.script(index, "ContentSizeFitter")) {
                        for axis in 0..2 {
                            let mode = fitter.get(4 * axis..4 * axis + 4).map_or(0, |word| u32::from_le_bytes(word.try_into().unwrap_or_default()));
                            if mode == 0 {
                                continue;
                            }
                            let (least, preferred, _) = panel.asks(index, axis);
                            let fitted = if mode == 1 { least } else { preferred };
                            let pivot = own.at[axis] + layout.pivot[axis] * own.size[axis];
                            own.size[axis] = fitted;
                            own.at[axis] = pivot - layout.pivot[axis] * fitted;
                        }
                    }
                    own
                }
            };
            let pivot = [own.at[0] + layout.pivot[0] * own.size[0], own.at[1] + layout.pivot[1] * own.size[1]];
            let local = Rect { at: [-layout.pivot[0] * own.size[0], -layout.pivot[1] * own.size[1]], size: own.size };
            let origin = [origin[0] + scale[0] * pivot[0], origin[1] + scale[1] * pivot[1]];
            (local, origin, [scale[0] * layout.scale[0], scale[1] * layout.scale[1]])
        }
        let (local, origin, scale) = frame(self, index, size);
        let corner = |axis: usize, at: f32| origin[axis] + scale[axis] * at;
        let (x0, x1) = (corner(0, local.at[0]), corner(0, local.at[0] + local.size[0]));
        let (y0, y1) = (corner(1, local.at[1]), corner(1, local.at[1] + local.size[1]));
        let (left, right, bottom, top) = (x0.min(x1), x0.max(x1), y0.min(y1), y0.max(y1));
        Placed { rect: [left, size[1] - top, right - left, top - bottom], flip: [scale[0] < 0.0, scale[1] < 0.0] }
    }

    /// An object's rectangle with the top that size: its left, top, width
    /// and height from the top's top left corner, y down.
    pub fn rect(&self, index: usize, size: [f32; 2]) -> [f32; 4] {
        self.placed(index, size).rect
    }

    /// Where an object is drawn with the top laid out by its own anchors in
    /// what it hangs from, of that size: from that one's top left corner.
    pub fn placed_in(&self, index: usize, outer: [f32; 2]) -> Placed {
        let top = place(self.nodes[0].layout, Rect { at: [0.0, 0.0], size: outer });
        let mut placed = self.placed(index, top.size);
        placed.rect[0] += top.at[0];
        placed.rect[1] += outer[1] - top.at[1] - top.size[1];
        placed
    }

    /// The file a pointer of the panel's leads into, by the number it
    /// gives it: its own file for 0, else one of its outside files.
    pub fn file_of(&self, outside: i32) -> Option<String> {
        match outside {
            0 => Some(self.file.clone()).filter(|file| !file.is_empty()),
            n => self.externals.get(n as usize - 1).map(|name| name.rsplit('/').next().unwrap_or(name).to_owned()),
        }
    }

    /// An object's own size along an axis where no group sets it: what its
    /// fitter makes it where it has one for that axis and lays out what it
    /// holds (`ContentSizeFitter`: least 1, preferred 2; a fitter sizes its
    /// object in a group that leaves that axis to it, as the quick-use
    /// panel's header in its column: its words and its padding, where it
    /// was kept at its saved width), else the size it is kept at.
    fn own_size(&self, index: usize, axis: usize) -> f32 {
        let node = &self.nodes[index];
        if let (Some(_), Some(fitter)) = (node.group, self.script(index, "ContentSizeFitter")) {
            let mode = fitter.get(4 * axis..4 * axis + 4).map_or(0, |word| u32::from_le_bytes(word.try_into().unwrap_or_default()));
            if mode != 0 {
                let (least, preferred, _) = self.asks(index, axis);
                return if mode == 1 { least } else { preferred };
            }
        }
        node.layout.size_delta[axis]
    }

    /// What an object asks of a layout along an axis (`LayoutUtility`): its
    /// least, preferred and flexible sizes; from its own layout group's
    /// children where it has a group (`CalcAlongAxis`: along the group
    /// their sum with the spacing, across it the largest, and the padding),
    /// and as its `LayoutElement` says where that says; each preferred at
    /// least its least. A picture's or a text's own sizes are not had: none.
    pub fn asks(&self, index: usize, axis: usize) -> (f32, f32, f32) {
        let node = &self.nodes[index];
        let (mut least, mut preferred, mut flexible) = (0.0f32, 0.0f32, 0.0f32);
        if let Some(group) = node.group {
            let main = if group.vertical { 1 } else { 0 };
            let laid: Vec<usize> = (0..self.nodes.len())
                .filter(|&child| self.nodes[child].parent == Some(index) && self.nodes[child].active && !self.nodes[child].element.is_some_and(|e| e.ignore))
                .collect();
            for (n, &child) in laid.iter().enumerate() {
                let (l, p, f) = if group.control[axis] {
                    let (l, p, f) = self.asks(child, axis);
                    (l, p, if group.expand[axis] { f.max(1.0) } else { f })
                } else {
                    let own = self.own_size(child, axis);
                    (own, own, 0.0)
                };
                if axis == main {
                    let gap = if n > 0 { group.spacing } else { 0.0 };
                    least += l + gap;
                    preferred += p + gap;
                    flexible += f;
                } else {
                    least = least.max(l);
                    preferred = preferred.max(p);
                    flexible = flexible.max(f);
                }
            }
            let pad = if axis == 0 { group.padding[0] + group.padding[1] } else { group.padding[2] + group.padding[3] };
            least += pad;
            preferred += pad;
        }
        if let Some(e) = node.element {
            if e.min[axis] >= 0.0 {
                least = e.min[axis];
            }
            if e.preferred[axis] >= 0.0 {
                preferred = e.preferred[axis];
            }
            if e.flexible[axis] >= 0.0 {
                flexible = e.flexible[axis];
            }
        }
        (least, preferred.max(least), flexible)
    }

    /// A child's own rectangle in its parent's as the parent's row or
    /// column lays it out (`HorizontalOrVerticalLayoutGroup`: `CalcAlongAxis`
    /// and `SetChildrenAlongAxis`), the children that are on and not left
    /// out in their order: along the group, each from its least towards its
    /// preferred size as far as the group's length allows, and what is left
    /// over shared by how much each may grow, from the start (left, or the
    /// top); across it, the group's breadth within each's least and, unless
    /// it may grow, preferred. None where the parent has no group.
    fn in_row(&self, parent: usize, index: usize, outer: Rect) -> Option<Rect> {
        let group = self.nodes[parent].group?;
        let (main, cross) = if group.vertical { (1, 0) } else { (0, 1) };
        let laid: Vec<usize> = (0..self.nodes.len())
            .filter(|&child| self.nodes[child].parent == Some(parent) && self.nodes[child].active && !self.nodes[child].element.is_some_and(|e| e.ignore))
            .collect();
        let at = laid.iter().position(|&child| child == index)?;
        let sizes = |child: usize, axis: usize| -> (f32, f32, f32) {
            if !group.control[axis] {
                let own = self.own_size(child, axis);
                return (own, own, 0.0);
            }
            let (least, preferred, flexible) = self.asks(child, axis);
            (least, preferred, if group.expand[axis] { flexible.max(1.0) } else { flexible })
        };
        let [left, right, top, bottom] = group.padding;
        // Each axis's padding before and after, and how its surplus is
        // shared out by the group's corner (`childAlignment`).
        let pad = |axis: usize| if axis == 0 { (left, right) } else { (top, bottom) };
        let align = |axis: usize| if axis == 0 { (group.alignment % 3) as f32 * 0.5 } else { (group.alignment / 3) as f32 * 0.5 };
        let start = |axis: usize, needed: f32| {
            let (before, after) = pad(axis);
            before + (outer.size[axis] - needed - before - after) * align(axis)
        };
        // Along the group.
        let (before, after) = pad(main);
        let (mut least, mut preferred, mut flexible) = (before + after, before + after, 0.0);
        for &child in &laid {
            let (l, p, f) = sizes(child, main);
            least += l + group.spacing;
            preferred += p + group.spacing;
            flexible += f;
        }
        if !laid.is_empty() {
            least -= group.spacing;
            preferred -= group.spacing;
        }
        let preferred = preferred.max(least);
        let (mut position, mut share) = (before, 0.0);
        let surplus = outer.size[main] - preferred;
        if surplus > 0.0 {
            if flexible == 0.0 {
                position = start(main, preferred - before - after);
            } else {
                share = surplus / flexible;
            }
        }
        let towards = if least != preferred { ((outer.size[main] - least) / (preferred - least)).clamp(0.0, 1.0) } else { 0.0 };
        let mut along = (0.0, 0.0);
        for (n, &child) in laid.iter().enumerate() {
            let (l, p, f) = sizes(child, main);
            let size = l + (p - l) * towards + f * share;
            if n == at {
                let own = self.own_size(child, main);
                along = if group.control[main] { (position, size) } else { (position + (size - own) * align(main), own) };
            }
            position += size + group.spacing;
        }
        // Across it.
        let (l, p, f) = sizes(index, cross);
        let (before, after) = pad(cross);
        let inner = outer.size[cross] - before - after;
        let most = if f > 0.0 { outer.size[cross] } else { p };
        let needed = if inner < l { l } else if inner > most { most } else { inner };
        let from = start(cross, needed);
        let own = self.own_size(index, cross);
        let across = if group.control[cross] { (from, needed) } else { (from + (needed - own) * align(cross), own) };
        // From the top left, y down; then as the engine keeps it, y up.
        let ((x, width), (y, height)) = if main == 0 { (along, across) } else { (across, along) };
        Some(Rect { at: [outer.at[0] + x, outer.at[1] + outer.size[1] - y - height], size: [width, height] })
    }
}

/// A part's view on the health panel (`BodyPartView`): itself, and what it
/// writes to as health changes, each an object of the panel.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PartView {
    pub view: usize,
    /// Its figures (`_valueText`).
    pub value: Option<usize>,
    /// The bar filled by the share left (`_healthBar`), and its border.
    pub bar: Option<usize>,
    pub border: Option<usize>,
    /// The part's shape in the silhouette, coloured by how hurt it is.
    pub shape: Option<usize>,
    /// The cross of a destroyed part.
    pub broken: Option<usize>,
    pub effects: Option<usize>,
    /// The damage of a raid gone by, which a raid has none of.
    pub damage: Option<usize>,
}

/// One of the health panel's figures (`HealthParameterPanel`, and with a
/// rate `BuffableHealthParameterPanel`): what it writes to and its colours.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ParameterView {
    pub panel: usize,
    pub current: Option<usize>,
    pub max: Option<usize>,
    pub ok_glow: Option<usize>,
    pub glow: Option<usize>,
    pub icon: Option<usize>,
    pub warning: [f32; 4],
    pub alternative: [f32; 4],
    /// The fewest digits its figure is written with, and the colour of
    /// the noughts put before it to make them up.
    pub least_digits: i32,
    pub noughts: [f32; 4],
    /// The rate's arrows and its figure, where it has a rate.
    pub up: Option<usize>,
    pub down: Option<usize>,
    pub rate: Option<usize>,
}

impl ParameterView {
    fn read(panel: &ScenePanel, index: usize) -> Option<Self> {
        let (fields, buffs) = match panel.script(index, "BuffableHealthParameterPanel") {
            Some(fields) => (fields, true),
            None => (panel.script(index, "HealthParameterPanel")?, false),
        };
        // Its own fields in their order: five pointers, two colours, a
        // count and a colour; then the rate's three pointers.
        let mut r = Reader::new(fields.get(60..)?);
        let (warning, alternative, least_digits, noughts) = (color(&mut r).ok()?, color(&mut r).ok()?, r.i32().ok()?, color(&mut r).ok()?);
        let at = |n: usize| panel.pointer_at(fields, n);
        let rate = |n: usize| if buffs { at(n) } else { None };
        Some(Self { panel: index, current: at(0), max: at(12), ok_glow: at(24), glow: at(36), icon: at(48), warning, alternative, least_digits, noughts, up: rate(112), down: rate(124), rate: rate(136) })
    }
}

/// What the health panel shows, as its scripts name it.
#[derive(Debug, Clone, Default)]
pub struct HealthViews {
    /// The parts' views in the order of the panel's fields (head, chest,
    /// stomach, left arm, right arm, left leg, right leg).
    pub parts: Vec<PartView>,
    pub silhouette: Option<usize>,
    /// The figures by the panel's fields' names: overall health,
    /// poisoning, radiation, blood pressure, energy, hydration,
    /// temperature, weight (none on this panel).
    pub parameters: Vec<(&'static str, Option<ParameterView>)>,
    pub legend: Option<usize>,
}

/// The equipment panel's slot views as its script names them
/// (`EquipmentTab`: eleven, in `GEAR_VIEWS`' order), and what each view's
/// script names (`SlotView`: the full border, the search icon, the empty
/// slot's picture, a camera's, the place of what is in it, the empty
/// fill, the empty border, the chosen border and corner, the header's
/// words, the header).
fn slot_views(panel: &ScenePanel) -> Vec<(&'static str, SlotParts)> {
    let Some(named) = panel.script(0, "EquipmentTab").and_then(|fields| pointers_from(panel, fields, GEAR_VIEWS.len(), 10)) else { return Vec::new() };
    GEAR_VIEWS
        .iter()
        .zip(named)
        .filter_map(|(slot, view)| {
            let view = view?;
            let at = panel.script(view, "SlotView").and_then(|fields| pointers_from(panel, fields, 11, 6))?;
            let parts = SlotParts {
                view,
                full_border: at[0],
                back_image: at[2],
                background: at[5],
                empty_border: at[6],
                selected_border: at[7],
                selected_corner: at[8],
                header_text: at[9],
                header: at[10],
            };
            Some((*slot, parts))
        })
        .collect()
}

/// The figures a figures' panel names (`HealthParametersPanel`: overall
/// health, poisoning, radiation, blood pressure, energy, hydration,
/// temperature and weight, in its fields' order), so many of them at
/// least.
fn figures_of(panel: &ScenePanel, index: usize, least: usize) -> Vec<(&'static str, Option<ParameterView>)> {
    const FIGURES: [&str; 8] = ["OverallHealth", "Poisoning", "Radiation", "BloodPressure", "Energy", "Hydration", "Temperature", "Weight"];
    let figures = panel.script(index, "HealthParametersPanel").and_then(|fields| pointers_from(panel, fields, 8, least));
    FIGURES.iter().enumerate().map(|(n, name)| (*name, figures.as_ref().and_then(|at| at[n]).and_then(|index| ParameterView::read(panel, index)))).collect()
}

/// A figure of a list (`HealthViews::parameters`, `HealthPanel::
/// strip_figures`) by its field's name.
pub fn figure_named<'a>(figures: &'a [(&'static str, Option<ParameterView>)], name: &str) -> Option<&'a ParameterView> {
    figures.iter().find(|(field, _)| *field == name).and_then(|(_, view)| view.as_ref())
}

/// The pointers of a script's fields that come one after another from the
/// first place where `count` of them in a row lead into the panel (at
/// least `least` of them, the first among them): past what the script's
/// parent classes keep.
fn pointers_from(panel: &ScenePanel, fields: &[u8], count: usize, least: usize) -> Option<Vec<Option<usize>>> {
    (0..fields.len()).step_by(4).find_map(|start| {
        let found: Vec<Option<usize>> = (0..count).map(|n| panel.pointer_at(fields, start + 12 * n)).collect();
        (found[0].is_some() && found.iter().flatten().count() >= least).then_some(found)
    })
}

impl HealthViews {
    fn read(panel: &ScenePanel) -> Option<Self> {
        // The panel's own script: the figures, the silhouette, the seven
        // views and the legend.
        let fields = panel.script(0, "InventoryScreenHealthPanel")?;
        let named = pointers_from(panel, fields, 10, 10)?;
        let mut parts = Vec::new();
        for view in named[2..9].iter() {
            let view = (*view)?;
            let fields = panel.script(view, "BodyPartView")?;
            let at = pointers_from(panel, fields, 8, 7)?;
            parts.push(PartView { view, value: at[0], bar: at[1], border: at[2], shape: at[3], broken: at[4], effects: at[5], damage: at[6] });
        }
        let parameters = named[0].map_or_else(Vec::new, |figures| figures_of(panel, figures, 7));
        Some(Self { parts, silhouette: named[1], parameters, legend: named[9] })
    }

    /// A figure by its field's name.
    pub fn parameter(&self, name: &str) -> Option<&ParameterView> {
        self.parameters.iter().find(|(field, _)| *field == name).and_then(|(_, view)| view.as_ref())
    }
}

/// The inventory screen's health panel (`InventoryScreenHealthPanel`),
/// which its HEALTH tab shows in the left panel in place of the equipment
/// panel (`ItemsPanel.UpdateEquipment`), and what places the left panel
/// on the screen (`docs/spec/health.md`, "The HEALTH tab").
#[derive(Debug, Clone, Default)]
pub struct HealthPanel {
    pub panel: ScenePanel,
    pub views: HealthViews,
    /// The strip of figures the GEAR tab has under its equipment
    /// (`Health Parameters`: weight, health, hydration and energy in a
    /// row), and what its script names.
    pub strip: Option<ScenePanel>,
    pub strip_figures: Vec<(&'static str, Option<ParameterView>)>,
    /// The left panel's foot (`Bottom Bar`), a picture under both tabs'
    /// panels.
    pub foot: Option<ScenePanel>,
    /// The equipment panel as the scene has it (`Gear Panel`, its own
    /// size set while the game runs: its template's), and its slot views
    /// by their slots' names in the order its script names them
    /// (`GEAR_VIEWS`), with what each view's script names.
    pub gear_panel: Option<ScenePanel>,
    pub slot_views: Vec<(&'static str, SlotParts)>,
    /// The screen's left side (`Items Panel/LeftSide`), whose horizontal
    /// layout lays the left panel and the containers' panel side by side,
    /// so far apart.
    pub left_side: Layout,
    pub spacing: f32,
    /// Where the equipment panel lies in the left panel: its template's
    /// layout, beside it (its own is set while the game runs).
    pub gear: Layout,
    /// The left panel's own ground (its picture's colour, a plain one),
    /// and the left side's frame round both its panels (`Border`, out of
    /// the layout, drawn over them).
    pub left_ground: Option<[f32; 4]>,
    pub border: Option<ScenePanel>,
    /// The panel on the screen's right that holds a container opened or
    /// the stash (`Items Panel/Stash Panel`), by its own layout.
    pub stash: Option<Layout>,
}

/// A slot view (`SlotView`): itself, and what its script names, each an
/// object of the equipment panel: the borders full and empty, the empty
/// slot's picture and its fill, the borders of a slot chosen, its header
/// and the header's words.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SlotParts {
    pub view: usize,
    pub full_border: Option<usize>,
    pub back_image: Option<usize>,
    pub background: Option<usize>,
    pub empty_border: Option<usize>,
    pub selected_border: Option<usize>,
    pub selected_corner: Option<usize>,
    pub header_text: Option<usize>,
    pub header: Option<usize>,
}

impl HealthPanel {
    /// Reads it from the scene file that holds the inventory screen.
    pub fn load(install: &Install, file: &str) -> Result<Self> {
        const LEFT: &str = "/InventoryScreen/Items Panel/LeftSide/Left Panel";
        let scene = Scene::load(install, file, true)?;
        let left = (0..scene.nodes.len()).find(|&node| scene.nodes[node].name == "Left Panel" && scene.path(node).ends_with(LEFT)).with_context(|| format!("{file} has no inventory screen"))?;
        let child = |name: &str| scene.nodes[left].children.iter().copied().find(|&child| scene.nodes[child].name == name);
        let health = child("Health Panel").with_context(|| format!("{file}: the inventory's left panel has no health panel"))?;
        let gear = child("Gear Panel Template").and_then(|template| scene.nodes[template].layout).context("the equipment panel's template")?;
        let side = scene.nodes[left].parent.context("the left side")?;
        let left_side = scene.nodes[side].layout.context("the left side's layout")?;
        // The layout group's fields: its padding, the corner it lays out
        // from, then the space between its children.
        let group = scene.behaviour(side, "HorizontalLayoutGroup").context("the left side's layout group")?;
        let mut r = Reader::new(scene.fields(group));
        r.skip(16 + 4)?;
        let spacing = r.f32()?;
        let of = |top: usize| ScenePanel { file: file.to_owned(), ..ScenePanel::of(&scene, top) };
        let panel = of(health);
        let views = HealthViews::read(&panel).with_context(|| format!("{file}: the health panel's scripts do not name its views"))?;
        let strip = child("Health Parameters").map(of);
        let strip_figures = strip.as_ref().map_or_else(Vec::new, |strip| figures_of(strip, 0, 4));
        let foot = child("Bottom Bar").map(of);
        let gear_panel = child("Gear Panel").map(of);
        let slot_views = gear_panel.as_ref().map_or_else(Vec::new, slot_views);
        let left_ground = scene.behaviour(left, "Image").and_then(|script| image_look(scene.fields(script))).map(|image| image.color);
        let border = scene.nodes[side].children.iter().copied().find(|&child| scene.nodes[child].name == "Border").map(of);
        let stash = scene.nodes[side].parent.and_then(|items| scene.nodes[items].children.iter().copied().find(|&child| scene.nodes[child].name == "Stash Panel")).and_then(|stash| scene.nodes[stash].layout);
        Ok(Self { panel, views, left_side, spacing, gear, strip, strip_figures, foot, gear_panel, slot_views, left_ground, border, stash })
    }

    /// The left side's place on a screen so wide and high, in its units:
    /// its left, top, width and height, y down (the screen's items' panel
    /// is the whole screen).
    pub fn left_side_in(&self, screen: [f32; 2]) -> [f32; 4] {
        let side = place(self.left_side, Rect { at: [0.0, 0.0], size: screen });
        [side.at[0], screen[1] - side.at[1] - side.size[1], side.size[0], side.size[1]]
    }

    /// The right's panel's place (`stash`) the same way.
    pub fn stash_in(&self, screen: [f32; 2]) -> Option<[f32; 4]> {
        let stash = place(self.stash?, Rect { at: [0.0, 0.0], size: screen });
        Some([stash.at[0], screen[1] - stash.at[1] - stash.size[1], stash.size[0], stash.size[1]])
    }

    /// The left panel's size on a screen so wide and high, in the screen's
    /// units: the left side's width shared out by its layout group, which
    /// sets its children's widths and makes them all grow alike, between
    /// the left panel and the containers' panel, neither of which asks for
    /// any width of its own (the left panel's picture has no sprite, the
    /// containers' panel has nothing to lay it out, the border between
    /// them is left out of the layout), and the left side's height.
    pub fn left_panel(&self, screen: [f32; 2]) -> [f32; 2] {
        let side = place(self.left_side, Rect { at: [0.0, 0.0], size: screen });
        [(side.size[0] - self.spacing) / 2.0, side.size[1]]
    }

    /// The equipment panel's place in a left panel of that size: its left,
    /// top, width and height, y down.
    pub fn gear_in(&self, left: [f32; 2]) -> [f32; 4] {
        let rect = place(self.gear, Rect { at: [0.0, 0.0], size: left });
        [rect.at[0], left[1] - rect.at[1] - rect.size[1], rect.size[0], rect.size[1]]
    }
}

/// The sprites a scene's images draw. They are in the build's own files
/// beside the scene (`sharedassets44.assets`), which keep no type trees: a
/// sprite and a texture are read there by the trees of the same classes
/// in a bundle of the install's, which the same engine wrote.
pub struct ScenePictures {
    install: Install,
    folder: std::path::PathBuf,
    sprite_tree: crate::unity::typetree::TypeTree,
    texture_tree: crate::unity::typetree::TypeTree,
    files: HashMap<String, BuildFile>,
}

/// A sprite: the texture it is cut from (its file and its number there),
/// the rectangle cut, in the texture's pixels from its lower left corner,
/// how far in from each side (left, bottom, right, top) a sliced image
/// keeps it whole, and its pixels to a unit of the screen.
#[derive(Debug, Clone, PartialEq)]
pub struct SpriteCut {
    pub name: String,
    pub texture: (String, i64),
    pub rect: [f32; 4],
    pub border: [f32; 4],
    pub pixels_per_unit: f32,
    /// The sprite's own rectangle, of which the cut is the part that is
    /// not see-through (`m_Rect`), and where the cut lies in it.
    pub whole: [f32; 4],
    pub offset: [f32; 2],
    /// Packed into an atlas turned or flipped, which is not drawn here.
    pub turned: bool,
}

impl SpriteCut {
    /// The share of an image's rectangle that the cut covers, as the
    /// engine draws a sprite trimmed of its see-through margin (an image
    /// not kept to the sprite's shape): from the left, from the bottom, to
    /// the right, to the top.
    pub fn covers(&self) -> [f32; 4] {
        let [w, h] = [self.whole[2].max(1e-6), self.whole[3].max(1e-6)];
        [self.offset[0] / w, self.offset[1] / h, (self.offset[0] + self.rect[2]) / w, (self.offset[1] + self.rect[3]) / h]
    }
}

/// The engine's class numbers for a sprite.
const SPRITE: i32 = 213;

/// The animation clips of the build's own files (`sharedassets44.assets`),
/// each by its number, read by the tree of a clip from a bundle of the
/// install's: the smallest that has one (`probe classfind 74`), or any
/// that has where that one is gone.
pub fn scene_clips(install: &Install, pictures: &mut ScenePictures, files: &[String]) -> Result<Vec<(String, i64, crate::unity::anim::Clip)>> {
    const SMALLEST: &str = "assets/content/weapons/weapon_root_anim_fix.bundle";
    let class = class::ANIMATION_CLIP;
    let tree = match tree_of(install, SMALLEST, class) {
        Ok(tree) => tree,
        Err(_) => {
            let assets = crate::unity::Assets::new(install.clone())?;
            let mut keys: Vec<(u64, String)> = assets.manifest.entries.keys().filter_map(|key| Some((std::fs::metadata(install.bundle_path(key)).ok()?.len(), key.clone()))).collect();
            keys.sort();
            keys.iter().find_map(|(_, key)| tree_of(install, key, class).ok()).context("no bundle of the install's has a clip to read clips by")?
        }
    };
    let mut clips = Vec::new();
    for name in files {
        for id in pictures.objects(name, class).unwrap_or_default() {
            if let Ok(clip) = pictures.read_by(name, id, class, &tree).and_then(|v| crate::unity::anim::Clip::of(&v)) {
                clips.push((name.clone(), id, clip));
            }
        }
    }
    Ok(clips)
}

/// The build's own Resources by their paths (`ResourceManager`, the
/// engine's class 147 in `globalgamemanagers`: its container, each path
/// in lower case with the object it is): the path, the file the object is
/// in and its number there.
pub fn resource_paths(install: &Install) -> Result<Vec<(String, String, i64)>> {
    const RESOURCE_MANAGER: i32 = 147;
    let mut build = BuildFile::open(install, "globalgamemanagers")?;
    let object = *build.meta.objects.iter().find(|object| object.class_id == RESOURCE_MANAGER).context("globalgamemanagers has no resource manager")?;
    let externals = build.meta.externals.clone();
    let bytes = build.bytes(object.offset, object.size)?;
    let mut r = Reader { data: &bytes, pos: 0, big_endian: build.meta.big_endian };
    let count = r.u32()? as usize;
    anyhow::ensure!(count < 1_000_000, "the resource manager does not begin with its container");
    let mut paths = Vec::with_capacity(count);
    for _ in 0..count {
        let path = string(&mut r)?;
        let to = pointer(&mut r)?;
        let file = match to.file {
            0 => "globalgamemanagers".to_owned(),
            n => externals.get(n as usize - 1).map(|name| name.rsplit('/').next().unwrap_or(name).to_owned()).unwrap_or_default(),
        };
        paths.push((path, file, to.path));
    }
    Ok(paths)
}

/// The game's views of a thing in a slot and in a grid (`SlotItemView`,
/// `GridItemView`): their prefabs among the build's Resources, as
/// `ItemViewFactory.CreateFromPool` names them (`Prefabs/UGUI/Layouts/`
/// `slot_layout`, `grid_layout`), read as panels.
#[derive(Debug, Clone, Default)]
pub struct ItemViews {
    pub slot: Option<ScenePanel>,
    pub grid: Option<ScenePanel>,
}

impl ItemViews {
    pub fn load(install: &Install) -> Result<Self> {
        let paths = resource_paths(install)?;
        let read = |name: &str| -> Result<ScenePanel> {
            let wanted = format!("prefabs/ugui/layouts/{name}");
            let (_, file, id) = paths.iter().find(|(path, ..)| *path == wanted).with_context(|| format!("no resource is at {wanted}"))?;
            let scene = Scene::load_prefab(install, file, *id)?;
            Ok(ScenePanel { file: file.clone(), ..ScenePanel::of(&scene, 0) })
        };
        Ok(Self { slot: Some(read("slot_layout")?), grid: Some(read("grid_layout")?) })
    }
}

/// The inventory's containers' column as the game draws it
/// (`ContainersPanel`): the scene's panel with its row template (`Slot
/// Template`: a container slot's header and its panel, the grids to its
/// right), the view of one grid behind the default row of grids
/// (`GeneratedGridsView`, the template's `_containedGridsTemplate`, and its
/// own `_gridViewTemplate`), and that row's layout.
#[derive(Debug, Clone, Default)]
pub struct ContainerViews {
    pub panel: ScenePanel,
    /// The row template, its column (`Content`: rows so far apart, the
    /// first so far down), its header and its slot panel.
    pub template: usize,
    pub column: Option<RowGroup>,
    pub header: Option<usize>,
    pub slot_panel: Option<usize>,
    /// A grid's view (`GridView`) as a panel of its own, and the default
    /// row the grids of a container are laid in.
    pub grid: Option<ScenePanel>,
    pub grids_row: Option<RowGroup>,
    /// How the column scrolls, and its bar.
    pub scrolling: Option<Scrolling>,
}

/// A scroll view of a screen (`ScrollRect`, here `ScrollRectNoDrag`) as
/// its scene has it: its objects, how far a notch of the wheel moves it
/// and how it stops, and its bar.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Scrolling {
    /// What is seen of it (`m_Viewport`, else itself) and what moves
    /// (`m_Content`), objects of the panel.
    pub view: Option<usize>,
    pub content: Option<usize>,
    /// Whether it scrolls up and down, and across.
    pub vertical: bool,
    pub across: bool,
    /// Units a notch of the wheel moves it (`m_ScrollSensitivity`).
    pub sensitivity: f32,
    /// Whether it stops at its ends (`MovementType.Clamped`), or springs
    /// back from past them (`Elastic`) over so long (`m_Elasticity`, s);
    /// whether it runs on after a drag (`m_Inertia`) and how fast that
    /// slows (`m_DecelerationRate`).
    pub clamped: bool,
    pub elastic: bool,
    pub elasticity: f32,
    pub inertia: bool,
    pub deceleration: f32,
    /// Its bar up and down (`m_VerticalScrollbar`), and whether the bar is
    /// hidden while everything shows (`ScrollbarVisibility.AutoHide`).
    pub bar: Option<ScrollBar>,
    pub hides: bool,
}

/// A scroll bar of a screen (`Scrollbar`): its object, the area its
/// handle slides in and the handle (the handle's parent), whether it runs
/// from the bottom up (`Direction.BottomToTop`), and how the handle is
/// tinted as the mouse is on it or holds it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScrollBar {
    pub object: usize,
    pub handle: Option<usize>,
    pub sliding: Option<usize>,
    pub bottom_up: bool,
    pub tints: Option<Tints>,
    /// The object whose picture the tints are laid on (`m_TargetGraphic`).
    pub tinted: Option<usize>,
}

/// How a control of a screen tints what it draws by its state
/// (`Selectable.colors`, a `ColorBlock`, when its transition is the
/// tint): idle, under the mouse, held, chosen and off, each multiplied
/// by the multiplier, and how long a change takes, in seconds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tints {
    pub normal: [f32; 4],
    pub highlighted: [f32; 4],
    pub pressed: [f32; 4],
    pub selected: [f32; 4],
    pub disabled: [f32; 4],
    pub multiplier: f32,
    pub fade: f32,
}

/// What a control's fields begin with (`Selectable`): its navigation, its
/// transition and its tints, its sprites and its animation's triggers,
/// whether it works, and what it tints. Read up to its own class's
/// fields, which follow; the tints only where the transition is the tint.
fn selectable(fields: &[u8]) -> Option<(Option<Tints>, usize)> {
    let read = |r: &mut Reader| -> Result<(Option<Tints>, usize)> {
        // Navigation: its mode, whether it wraps, four pointers.
        r.skip(4 + 4 + 4 * 12)?;
        let transition = r.u32()?;
        let (normal, highlighted, pressed, selected, disabled) = (color(r)?, color(r)?, color(r)?, color(r)?, color(r)?);
        let (multiplier, fade) = (r.f32()?, r.f32()?);
        // Its sprites by state, its animation's five triggers, whether it
        // works, and what it tints.
        r.skip(4 * 12)?;
        for _ in 0..5 {
            string(r)?;
        }
        r.skip(4 + 12)?;
        let tints = (transition == 1).then_some(Tints { normal, highlighted, pressed, selected, disabled, multiplier, fade });
        Ok((tints, fields.len() - r.remaining()))
    };
    read(&mut Reader::new(fields)).ok()
}

impl Scrolling {
    /// The scroll view on that object of a panel (its script of that
    /// class, a `ScrollRect` or a class built on one), by the engine's
    /// order of the fields: what moves, whether across and up and down,
    /// how it stops, its elasticity, inertia and deceleration, the
    /// wheel's step, what is seen of it, its two bars, whether each hides,
    /// and their spacing.
    pub fn of(panel: &ScenePanel, object: usize, class: &str) -> Option<Self> {
        let fields = panel.script(object, class)?;
        let mut r = Reader::new(fields);
        r.skip(12).ok()?;
        let (across, vertical) = (flag(&mut r).ok()?, flag(&mut r).ok()?);
        let movement = r.u32().ok()?;
        let (elasticity, inertia, deceleration) = (r.f32().ok()?, flag(&mut r).ok()?, r.f32().ok()?);
        let sensitivity = r.f32().ok()?;
        r.skip(12 + 12 + 12 + 4).ok()?;
        let hides = r.u32().ok()? == 1;
        let bar = panel.pointer_at(fields, 64).and_then(|object| ScrollBar::of(panel, object));
        let (clamped, elastic) = (movement == 2, movement == 1);
        Some(Self { view: panel.transform_at(fields, 40).or(Some(object)), content: panel.transform_at(fields, 0), vertical, across, sensitivity, clamped, elastic, elasticity, inertia, deceleration, bar, hides })
    }
}

impl ScrollBar {
    /// The bar whose script is on that object: after what every control
    /// has (`selectable`), its handle and its direction.
    fn of(panel: &ScenePanel, object: usize) -> Option<Self> {
        let fields = panel.script(object, "Scrollbar")?;
        let (tints, at) = selectable(fields)?;
        let handle = panel.transform_at(fields, at);
        let direction = u32::from_le_bytes(fields.get(at + 12..at + 16)?.try_into().ok()?);
        let tinted = at.checked_sub(12).and_then(|before| panel.pointer_at(fields, before));
        Some(Self { object, handle, sliding: handle.and_then(|handle| panel.nodes[handle].parent), bottom_up: direction == 2, tints, tinted })
    }

    /// Where its handle lies in a panel of that size (the panel's top as
    /// large), the bar showing `size` of what it scrolls (a share) and at
    /// `value` (0 at the bottom of a bar that runs from the bottom up, 1 at
    /// its top): the handle's own layout with the anchors the bar gives it
    /// up and down its sliding area (`Scrollbar.UpdateVisuals`). Its
    /// left, top, width and height, y down.
    pub fn handle_in(&self, panel: &ScenePanel, whole: [f32; 2], value: f32, size: f32) -> Option<[f32; 4]> {
        let [x, y, w, h] = panel.rect(self.sliding?, whole);
        let layout = panel.nodes[self.handle?].layout;
        let moved = value.clamp(0.0, 1.0) * (1.0 - size);
        let (low, high) = if self.bottom_up { (moved, moved + size) } else { (1.0 - moved - size, 1.0 - moved) };
        let rect = place(Layout { anchor_min: [0.0, low], anchor_max: [1.0, high], scale: [1.0, 1.0], ..layout }, Rect { at: [0.0, 0.0], size: [w, h] });
        Some([x + rect.at[0], y + h - rect.at[1] - rect.size[1], rect.size[0], rect.size[1]])
    }
}

impl ContainerViews {
    /// Reads it from the scene that holds the inventory screen.
    pub fn load(install: &Install, file: &str) -> Result<Self> {
        const PANEL: &str = "/InventoryScreen/Items Panel/LeftSide/Containers Panel";
        let scene = Scene::load(install, file, true)?;
        let top = (0..scene.nodes.len()).find(|&node| scene.path(node).ends_with(PANEL)).with_context(|| format!("{file} has no containers panel"))?;
        let panel = ScenePanel { file: file.to_owned(), ..ScenePanel::of(&scene, top) };
        let template = (0..panel.nodes.len()).find(|&index| panel.nodes[index].scripts.iter().any(|(class, _)| class == "SearchableSlotView")).context("the containers panel has no row template")?;
        let column = panel.nodes[template].parent.and_then(|content| panel.nodes[content].group);
        let header = panel.find(&format!("{}/Header Panel/SlotViewHeader", panel.nodes[template].path));
        let slot_panel = panel.find(&format!("{}/Slot Panel", panel.nodes[template].path));
        // The template's grids' template, a script in another file, and
        // its grid view's, a script of that file's own.
        let outside = |fields: &[u8]| -> Option<(String, i64)> {
            (0..fields.len().saturating_sub(11)).step_by(4).find_map(|start| {
                let to = pointer(&mut Reader::new(&fields[start..])).ok()?;
                (to.file > 0 && to.path > 0 && to.path < 1 << 40).then(|| Some((panel.file_of(to.file)?, to.path))).flatten()
            })
        };
        let grids = panel.script(template, "SearchableItemView").and_then(outside).and_then(|(file, id)| Some((file.clone(), Scene::load_prefab_of_script(install, &file, id).ok()?)));
        let (mut grid, mut grids_row) = (None, None);
        if let Some((file, scene)) = grids {
            let generated = ScenePanel { file: file.clone(), ..ScenePanel::of(&scene, 0) };
            grids_row = generated.nodes[0].group;
            let template = generated.script(0, "GeneratedGridsView").and_then(|fields| {
                (0..fields.len().saturating_sub(11)).step_by(4).find_map(|start| {
                    let to = pointer(&mut Reader::new(&fields[start..])).ok()?;
                    (to.file == 0 && to.path > 0 && to.path < 1 << 40).then_some(to.path)
                })
            });
            grid = template.and_then(|id| Scene::load_prefab_of_script(install, &file, id).ok()).map(|scene| ScenePanel { file: file.clone(), ..ScenePanel::of(&scene, 0) });
        }
        let scrolling = panel.find("Scrollview Parent/Containers Scrollview").and_then(|view| Scrolling::of(&panel, view, "ScrollRectNoDrag"));
        Ok(Self { panel, template, column, header, slot_panel, grid, grids_row, scrolling })
    }
}

/// What a script built on Odin's `SerializedMonoBehaviour` (the game's
/// `UIElement`) keeps before its own fields (`SerializationData`): the
/// format, the bytes, the objects they refer to, the bytes as a string, the
/// prefab, its modifications' objects and its modifications, and the
/// nodes (each a name, an entry's kind and its data). Read past, so that
/// the reader is at the script's own fields.
fn skip_odin(r: &mut Reader) -> Result<()> {
    r.skip(4)?;
    let bytes = r.u32()? as usize;
    r.skip(bytes)?;
    r.align(4);
    let objects = r.u32()? as usize;
    r.skip(objects * 12)?;
    string(r)?;
    r.skip(12)?;
    let objects = r.u32()? as usize;
    r.skip(objects * 12)?;
    for _ in 0..r.u32()? {
        string(r)?;
    }
    for _ in 0..r.u32()? {
        string(r)?;
        r.skip(1)?;
        r.align(4);
        string(r)?;
    }
    Ok(())
}

/// The inventory screen's bar of tabs (`Tab Bar`) as its scene has it: the
/// bar, the row of tabs and how far apart it puts them (`TabSizeController.
/// _gap`, less than none: they overlap), and each tab with what its scripts
/// name.
#[derive(Debug, Clone, Default)]
pub struct TabBar {
    pub panel: ScenePanel,
    pub row: Option<usize>,
    pub gap: f32,
    pub tabs: Vec<TabParts>,
    /// The screen's button at the bar's right end that shuts it
    /// (`BackButton`, a `DefaultUIButton`), with all under it.
    pub back: Option<ScenePanel>,
}

/// One tab of a bar (`Tab`, `TabElement`, `LocalizedText`): its object;
/// its place in the row (`OrderNumber`), what its width is taken from
/// (`_sizeDefiners`, the widest of them that shows) and what is added to
/// that (`_widthAdjustment`); its versions idle and chosen
/// (`_normalVersion`, `_selectedVersion`) and what shows with the mouse on
/// it and off it (`_hoverGraphic`, `_idleGraphic`); and its words, the key
/// of the install's words with the case they are put in (`EStringCase`:
/// none, lower, upper, title) and the texts that show them (`_labels`).
#[derive(Debug, Clone, Default)]
pub struct TabParts {
    pub object: usize,
    pub order: i32,
    pub size_definers: Vec<usize>,
    pub width_adjustment: f32,
    pub normal: Option<usize>,
    pub selected: Option<usize>,
    pub hover: Option<usize>,
    pub idle: Option<usize>,
    pub key: String,
    pub case: u32,
    pub labels: Vec<usize>,
    /// What a tab that does not work is dimmed by (`SetInteractable`): the
    /// object of its group whose alpha goes to 0.3 (`_canvasGroup`,
    /// `SetUnlockStatus`) and its picture whose alpha goes to 0.15
    /// (`_targetImage`).
    pub canvas_group: Option<usize>,
    pub target: Option<usize>,
}

impl TabBar {
    /// Reads it from the scene that holds the inventory screen.
    pub fn load(install: &Install, file: &str) -> Result<Self> {
        const BAR: &str = "/InventoryScreen/Tab Bar";
        let scene = Scene::load(install, file, true)?;
        let top = (0..scene.nodes.len()).find(|&node| scene.path(node).ends_with(BAR)).with_context(|| format!("{file} has no bar of tabs"))?;
        let panel = ScenePanel { file: file.to_owned(), ..ScenePanel::of(&scene, top) };
        let row = (0..panel.nodes.len()).find(|&index| panel.script(index, "TabSizeController").is_some());
        let gap = row.and_then(|row| panel.script(row, "TabSizeController")).and_then(|fields| {
            let mut r = Reader::new(fields);
            skip_odin(&mut r).ok()?;
            r.f32().ok()
        });
        let mut tabs = Vec::new();
        for object in row.map_or_else(Vec::new, |row| (0..panel.nodes.len()).filter(|&index| panel.nodes[index].parent == Some(row)).collect()) {
            let mut tab = TabParts { object, ..TabParts::default() };
            if let Some(fields) = panel.script(object, "TabElement") {
                let mut r = Reader::new(fields);
                let read = |r: &mut Reader, tab: &mut TabParts| -> Result<()> {
                    skip_odin(r)?;
                    let count = r.u32()? as usize;
                    let at = fields.len() - r.remaining();
                    tab.size_definers = (0..count).filter_map(|n| panel.transform_at(fields, at + 12 * n)).collect();
                    r.skip(12 * count)?;
                    tab.width_adjustment = r.f32()?;
                    tab.order = r.i32()?;
                    Ok(())
                };
                read(&mut r, &mut tab).with_context(|| format!("the tab {}'s element", panel.nodes[object].path))?;
            }
            if let Some(fields) = panel.script(object, "Tab") {
                tab.normal = panel.pointer_at(fields, 0);
                tab.selected = panel.pointer_at(fields, 12);
                tab.canvas_group = panel.pointer_at(fields, 24);
                tab.target = panel.pointer_at(fields, 36);
                tab.hover = panel.pointer_at(fields, 60);
                tab.idle = panel.pointer_at(fields, 72);
            }
            if let Some(fields) = panel.script(object, "LocalizedText") {
                let mut r = Reader::new(fields);
                let read = |r: &mut Reader, tab: &mut TabParts| -> Result<()> {
                    skip_odin(r)?;
                    tab.key = string(r)?;
                    let count = r.u32()? as usize;
                    let at = fields.len() - r.remaining();
                    tab.labels = (0..count).filter_map(|n| panel.pointer_at(fields, at + 12 * n)).collect();
                    r.skip(12 * count)?;
                    tab.case = r.u32()?;
                    Ok(())
                };
                read(&mut r, &mut tab).with_context(|| format!("the tab {}'s words", panel.nodes[object].path))?;
            }
            tabs.push(tab);
        }
        tabs.sort_by_key(|tab| tab.order);
        const BACK: &str = "/InventoryScreen/BackButton";
        let back = (0..scene.nodes.len()).find(|&node| scene.path(node).ends_with(BACK)).map(|node| ScenePanel { file: file.to_owned(), ..ScenePanel::of(&scene, node) });
        Ok(Self { panel, row, gap: gap.unwrap_or(0.0), tabs, back })
    }

    /// A tab's words: what the install's words have for its key (`say`),
    /// else the key, in the case its `LocalizedText` puts them.
    pub fn words(tab: &TabParts, say: &dyn Fn(&str) -> Option<String>) -> String {
        let words = say(&tab.key).unwrap_or_else(|| tab.key.clone());
        match tab.case {
            1 => words.to_lowercase(),
            2 => words.to_uppercase(),
            3 => words.split(' ').map(|word| word.chars().take(1).flat_map(char::to_uppercase).chain(word.chars().skip(1).flat_map(char::to_lowercase)).collect::<String>()).collect::<Vec<_>>().join(" "),
            _ => words,
        }
    }

    /// How wide every tab is (`TabSizeController.UpdateLayout`): of each
    /// tab, the widest of what its width is taken from (`TabElement.
    /// GetWidth`: each a text sized to its words, as TextMeshPro lays them
    /// out in its font, `TmpFont::width`, with its margins) and its
    /// adjustment; the widest of the tabs. A text drawn bold is laid out in
    /// its font's bold typeface where it has one, as TextMeshPro takes it.
    pub fn width(&self, install: &Install, say: &dyn Fn(&str) -> Option<String>) -> f32 {
        let mut fonts: HashMap<(String, i64), Option<TmpFont>> = HashMap::new();
        let mut font = |file: &str, id: i64| fonts.entry((file.to_owned(), id)).or_insert_with(|| TmpFont::read(install, file, id).ok()).clone();
        let mut widest = 0.0f32;
        for tab in &self.tabs {
            let words = Self::words(tab, say);
            let mut own = 0.0f32;
            for &definer in &tab.size_definers {
                let Some(text) = &self.panel.nodes[definer].text else { continue };
                let Some(file) = self.panel.file_of(text.font.0).or_else(|| (text.font.0 == 0).then(|| self.panel.file.clone())) else { continue };
                let shown = if text.style & 16 != 0 { words.to_uppercase() } else if text.style & 8 != 0 { words.to_lowercase() } else { words.clone() };
                let bold = text.style & 1 != 0;
                let Some(regular) = font(&file, text.font.1) else { continue };
                let laid = match regular.bold.clone().filter(|_| bold).and_then(|(file, id)| font(&file, id)) {
                    Some(alternative) => alternative.width(&shown, text.size, text.spacing, false),
                    None => regular.width(&shown, text.size, text.spacing, bold),
                };
                own = own.max(laid + text.margin[0] + text.margin[2]);
            }
            widest = widest.max(own + tab.width_adjustment);
        }
        widest
    }

    /// Lays the tabs out as the row does once their width is known
    /// (`TabSizeController.UpdateLayout`, `TabElement.SetWidth`): each as
    /// wide as the widest asks, its pivot `(width + gap) * OrderNumber`
    /// from the row's pivot (`localPosition`), here as its offset from its
    /// anchor in the row.
    pub fn lay_out(&mut self, width: f32) {
        let Some(row) = self.row else { return };
        let row_layout = self.panel.nodes[row].layout;
        for tab in &self.tabs {
            let layout = &mut self.panel.nodes[tab.object].layout;
            let anchor = (layout.anchor_min[0] - row_layout.pivot[0]) * row_layout.size_delta[0];
            layout.size_delta[0] = width;
            layout.offset[0] = (width + self.gap) * tab.order as f32 - anchor;
        }
    }
}

/// A TextMeshPro font asset (`TMP_FontAsset`, a script's object of the
/// build's files) as far as a line's width goes: the size its glyphs were
/// sampled at and its scale (`m_FaceInfo`), each glyph's advance at that
/// size and its own scale (`m_GlyphTable`), each letter's glyph and scale
/// (`m_CharacterTable`), what every letter adds and what a letter drawn
/// bold adds besides, in hundredths of the size (`normalSpacingOffset`,
/// `boldSpacing`), and the asset its bold is taken from (`m_FontWeightTable`'s
/// weight 700, its regular typeface), where it has one.
#[derive(Debug, Clone, Default)]
pub struct TmpFont {
    pub point_size: f32,
    pub scale: f32,
    /// The face's lines above and below the baseline (`m_AscentLine`,
    /// `m_DescentLine`), at the sampling size.
    pub ascent: f32,
    pub descent: f32,
    /// The distance between two lines' baselines (`m_LineHeight`), at the
    /// sampling size.
    pub line_height: f32,
    glyphs: HashMap<u32, (f32, f32)>,
    letters: HashMap<u32, (u32, f32)>,
    /// Each glyph whole, by its index (`m_GlyphTable`).
    pub glyph_table: HashMap<u32, TmpGlyph>,
    /// The textures its glyphs' distance fields are kept in
    /// (`m_AtlasTextures`), each by its file and number.
    pub atlases: Vec<(String, i64)>,
    pub normal_spacing: f32,
    pub bold_spacing: f32,
    pub bold: Option<(String, i64)>,
    /// Its face's family and style (`m_FaceInfo`: "Bender", "Bold").
    pub family: String,
    pub style: String,
}

impl TmpFont {
    /// Reads it from its file and number. Its fields by the engine's order:
    /// the object's head and name, the asset's hash, material and its hash,
    /// its version, the source font's id and the font, how its atlas is
    /// filled, its face (index, family, style, sampling size, scale, units
    /// to the em and fifteen lines' places), its glyphs (52 bytes each:
    /// index, five metrics, the atlas rectangle, scale, atlas, class) and
    /// its letters (16 each: kind, letter, glyph, scale). The spacings are
    /// the last fields but two, after the weight table of ten pairs of
    /// typefaces and the old weights' empty list.
    pub fn read(install: &Install, file: &str, id: i64) -> Result<Self> {
        let (_, bytes) = object_bytes(install, file, id)?;
        let mut r = Reader::new(&bytes);
        r.skip(12 + 4 + 12)?;
        string(&mut r)?;
        r.skip(4 + 12 + 4)?;
        string(&mut r)?;
        string(&mut r)?;
        r.skip(12 + 4 + 4)?;
        let family = string(&mut r)?;
        let style = string(&mut r)?;
        let point_size = r.i32()? as f32;
        let scale = r.f32()?;
        // Units to the em, the line's height, then the ascent, the cap, the
        // mean line, the baseline and the descent, and nine more.
        r.skip(4)?;
        let line_height = r.f32()?;
        let ascent = r.f32()?;
        r.skip(3 * 4)?;
        let descent = r.f32()?;
        r.skip(9 * 4)?;
        let mut glyphs = HashMap::new();
        let mut glyph_table = HashMap::new();
        for _ in 0..r.u32()? {
            let index = r.u32()?;
            let (width, height, bearing_x, bearing_y) = (r.f32()?, r.f32()?, r.f32()?, r.f32()?);
            let advance = r.f32()?;
            let rect = [r.i32()?, r.i32()?, r.i32()?, r.i32()?];
            let scale = r.f32()?;
            let atlas = r.i32()?;
            r.skip(4)?;
            glyphs.insert(index, (advance, scale));
            glyph_table.insert(index, TmpGlyph { size: [width, height], bearing: [bearing_x, bearing_y], advance, rect, scale, atlas: atlas.max(0) as usize });
        }
        let mut letters = HashMap::new();
        for _ in 0..r.u32()? {
            r.skip(4)?;
            let (letter, glyph, scale) = (r.u32()?, r.u32()?, r.f32()?);
            letters.insert(letter, (glyph, scale));
        }
        // The atlases, each a pointer into this file or one of its own.
        let externals = BuildFile::open(install, file).map(|build| build.meta.externals.clone()).unwrap_or_default();
        let mut atlases = Vec::new();
        for _ in 0..r.u32()? {
            let to = pointer(&mut r)?;
            let name = match to.file {
                0 => Some(file.to_owned()),
                outside => externals.get(outside as usize - 1).map(|name| name.rsplit('/').next().unwrap_or(name).to_owned()),
            };
            if let Some(name) = name {
                atlases.push((name, to.path));
            }
        }
        let tail = |from_end: usize| -> Result<&[u8]> { bytes.get(bytes.len() - from_end..bytes.len() - from_end + 4).context("the font asset ends early") };
        let float = |from_end: usize| -> Result<f32> { Ok(f32::from_le_bytes(tail(from_end)?.try_into()?)) };
        let (normal_spacing, bold_spacing) = (float(20)?, float(12)?);
        // The weight table: ten pairs (regular, italic), then the old
        // weights' list, empty, before the spacings.
        let table = bytes.len() - 24 - 4 - 240;
        let bold = (u32::from_le_bytes(tail(24 + 4)?.try_into()?) == 0 && u32::from_le_bytes(bytes.get(table - 4..table).context("the weight table")?.try_into()?) == 10)
            .then(|| {
                let at = table + 24 * 7;
                let to = pointer(&mut Reader::new(&bytes[at..at + 12])).ok()?;
                (to.path != 0).then_some((to.file, to.path))
            })
            .flatten();
        let bold = match bold {
            Some((0, path)) => Some((file.to_owned(), path)),
            Some((outside, path)) => {
                let build = BuildFile::open(install, file)?;
                build.meta.externals.get(outside as usize - 1).map(|name| (name.rsplit('/').next().unwrap_or(name).to_owned(), path))
            }
            None => None,
        };
        Ok(Self { point_size, scale, ascent, descent, line_height, glyphs, letters, glyph_table, atlases, normal_spacing, bold_spacing, bold, family, style })
    }

    /// The index of the glyph a letter is drawn with.
    pub fn glyph_index_of(&self, letter: char) -> Option<u32> {
        self.letters.get(&(letter as u32)).map(|(glyph, _)| *glyph)
    }

    /// The glyph a letter is drawn with, and the letter's own scale.
    pub fn glyph_of(&self, letter: char) -> Option<(&TmpGlyph, f32)> {
        let (glyph, scale) = self.letters.get(&(letter as u32))?;
        Some((self.glyph_table.get(glyph)?, *scale))
    }

    /// How high a line of words is in it at a size, as TextMeshPro works
    /// out a text's preferred height: from the face's ascent to its
    /// descent at the size over the sampling size, rounded up to a
    /// hundredth.
    pub fn height(&self, size: f32) -> f32 {
        let height = (self.ascent - self.descent) * size / self.point_size.max(1.0) * self.scale;
        ((height * 100.0) as i64 + 1) as f32 / 100.0
    }

    /// How wide a line of words is in it at a size, as TextMeshPro works out
    /// a text's preferred width (`CalculatePreferredValues`): each letter's
    /// glyph's advance at the size over the sampling size, with the face's,
    /// the glyph's and the letter's scales, and what every letter but the
    /// last adds (`normalSpacingOffset`, the text's own `spacing`, and a
    /// bold letter's `boldSpacing` where `bold` and the asset has no bold
    /// of its own), in hundredths of the size; rounded up to a hundredth. A letter
    /// not in the asset is left out.
    pub fn width(&self, text: &str, size: f32, spacing: f32, bold: bool) -> f32 {
        let per = size / self.point_size.max(1.0) * self.scale;
        let added = (self.normal_spacing + spacing + if bold { self.bold_spacing } else { 0.0 }) * size / 100.0;
        let width: f32 = text
            .chars()
            .filter_map(|letter| self.letters.get(&(letter as u32)))
            .filter_map(|(glyph, letter_scale)| self.glyphs.get(glyph).map(|(advance, glyph_scale)| advance * per * glyph_scale * letter_scale + added))
            .sum();
        // The spacing after the last letter is not counted.
        let width = if text.chars().any(|letter| self.letters.contains_key(&(letter as u32))) { width - added } else { 0.0 };
        ((width * 100.0) as i64 + 1) as f32 / 100.0
    }
}

/// A glyph of a TextMeshPro font asset (`Glyph`): its size, where it
/// stands from the pen (its bearings: right, and up from the baseline) and
/// how far it moves the pen, in the atlas's pixels at the sampling size;
/// its rectangle in its atlas (left, bottom, width, height, in pixels from
/// the atlas's lower left corner); its scale; which atlas.
#[derive(Debug, Clone, Copy)]
pub struct TmpGlyph {
    pub size: [f32; 2],
    pub bearing: [f32; 2],
    pub advance: f32,
    pub rect: [i32; 4],
    pub scale: f32,
    pub atlas: usize,
}

/// How wide a line of words is in a font of the build's (its file's
/// bytes) at a size, in the screen's units, as TextMeshPro lays it out:
/// each letter's advance, and where the text keeps kerning
/// (`m_enableKerning`) the font's own between two (its `kern` table's
/// pairs), at the size over the font's units to the em. Where a letter is
/// not in the font, its replacement's (`.notdef`).
pub fn text_width(font: &[u8], text: &str, size: f32, kerning: bool) -> Option<f32> {
    let face = ttf_parser::Face::parse(font, 0).ok()?;
    let glyphs: Vec<ttf_parser::GlyphId> = text.chars().map(|letter| face.glyph_index(letter).unwrap_or(ttf_parser::GlyphId(0))).collect();
    let mut width: f32 = glyphs.iter().map(|&glyph| face.glyph_hor_advance(glyph).unwrap_or(0) as f32).sum();
    if let Some(kern) = face.tables().kern.filter(|_| kerning) {
        for pair in glyphs.windows(2) {
            width += kern.subtables.into_iter().filter(|table| table.horizontal && !table.variable).filter_map(|table| table.glyphs_kerning(pair[0], pair[1])).map(f32::from).sum::<f32>();
        }
    }
    Some(width * size / face.units_per_em() as f32)
}

/// The inventory's quick-use panel (`Items Panel/Quick Use Panel`,
/// `InventoryScreenQuickAccessPanel`) as its scene has it, its top that
/// panel: its views in the row's order, the grenade's (`_grenadeSlot`),
/// the four slots' with a key of their own (`_boundSlots`: `ItemV`,
/// `Item1`, `Item2`, `Item3`) and the seven keys' things (`_boundItems`:
/// `Item4` to `Item10`), each by its binding's name in the game's
/// `EBoundItem` with what its view's script names (`QuickSlotView`).
#[derive(Debug, Clone, Default)]
pub struct QuickViews {
    pub panel: ScenePanel,
    pub slots: Vec<QuickSlotParts>,
}

/// A quick-use view (`QuickSlotView`): its object, its binding's name, and
/// where its thing goes (`InstallPlace`), its key's words (`HotKey`), the
/// thing's name (`Caption`) and its border (`Border`); and on the battle
/// screen's bar the ground that is lit for the thing in the hands
/// (`Background`) and the arrow of a selector (`_arrow`).
#[derive(Debug, Clone, Default)]
pub struct QuickSlotParts {
    pub key: String,
    pub object: usize,
    pub install: Option<usize>,
    pub hot_key: Option<usize>,
    pub caption: Option<usize>,
    pub border: Option<usize>,
    pub background: Option<usize>,
    pub arrow: Option<usize>,
}

impl QuickViews {
    /// Reads it from the scene that holds the inventory screen.
    pub fn load(install: &Install, file: &str) -> Result<Self> {
        Self::load_at(install, file, "/InventoryScreen/Items Panel/Quick Use Panel")
    }

    /// The battle screen's quick bar (`EFTBattleUIScreen Variant/
    /// QuickAccessPanel`, the same script), as it shows in a raid: every
    /// view on (`BoundSlotView.ShowMainSlot`, `BoundItemView.Show`), a
    /// weapon's view for its under-barrel weapon off (`ShowSubItem`: none
    /// of the app's weapons has one), and the bar as wide and high as its
    /// row asks (its `ContentSizeFitter`).
    pub fn load_battle(install: &Install, file: &str) -> Result<Self> {
        let mut quick = Self::load_at(install, file, "/EFTBattleUIScreen Variant/QuickAccessPanel")?;
        for slot in quick.slots.clone() {
            quick.panel.nodes[slot.object].active = true;
        }
        for node in quick.panel.nodes.iter_mut().filter(|node| node.name == "SubWeapon") {
            node.active = false;
        }
        if let Some(fitter) = quick.panel.script(0, "ContentSizeFitter").map(<[u8]>::to_vec) {
            for axis in 0..2 {
                let mode = fitter.get(4 * axis..4 * axis + 4).map_or(0, |word| u32::from_le_bytes(word.try_into().unwrap_or_default()));
                if mode != 0 {
                    let (least, preferred, _) = quick.panel.asks(0, axis);
                    quick.panel.nodes[0].layout.size_delta[axis] = if mode == 1 { least } else { preferred };
                }
            }
        }
        Ok(quick)
    }

    fn load_at(install: &Install, file: &str, end: &str) -> Result<Self> {
        let scene = Scene::load(install, file, true)?;
        let top = (0..scene.nodes.len()).find(|&node| scene.path(node).ends_with(end)).with_context(|| format!("{file} has no quick-use panel"))?;
        let panel = ScenePanel { file: file.to_owned(), ..ScenePanel::of(&scene, top) };
        let fields = panel.script(0, "InventoryScreenQuickAccessPanel").context("the quick-use panel's script")?;
        // Odin's data and the input node's list of its children (none on
        // the inventory's, fifteen on the battle screen's); the grenade's
        // view; then each dictionary as its keys' names and its views.
        let mut r = Reader::new(fields);
        skip_odin(&mut r)?;
        let children = r.u32()? as usize;
        r.skip(12 * children)?;
        let at = fields.len() - r.remaining();
        let mut views: Vec<(String, Option<usize>)> = vec![("Grenade".to_owned(), panel.pointer_at(fields, at))];
        r.skip(12)?;
        for _ in 0..2 {
            let keys: Vec<String> = (0..r.u32()?).map(|_| string(&mut r)).collect::<Result<_>>()?;
            let count = r.u32()? as usize;
            let at = fields.len() - r.remaining();
            views.extend(keys.into_iter().zip((0..count).map(|n| panel.pointer_at(fields, at + 12 * n))));
            r.skip(12 * count)?;
        }
        let slots = views
            .into_iter()
            .filter_map(|(key, object)| {
                let object = object?;
                let fields = ["BoundSlotView", "BoundItemView", "FastAccessGrenadeItemView"].iter().find_map(|class| panel.script(object, class))?;
                let named = pointers_from(&panel, fields, 6, 4).unwrap_or_default();
                let at = |n: usize| named.get(n).copied().flatten();
                Some(QuickSlotParts { key, object, install: at(0), hot_key: at(1), caption: at(2), border: at(3), background: at(4), arrow: at(5) })
            })
            .collect();
        Ok(Self { panel, slots })
    }

    /// The panel's place on a screen so wide and high, in its units: left,
    /// top, width and height, y down (its parent the whole screen).
    pub fn quick_in(&self, screen: [f32; 2]) -> [f32; 4] {
        let rect = place(self.panel.nodes[0].layout, Rect { at: [0.0, 0.0], size: screen });
        [rect.at[0], screen[1] - rect.at[1] - rect.size[1], rect.size[0], rect.size[1]]
    }
}

/// The battle screen's two panels of the character (`EftBattleUIScreen`,
/// in the scene that holds the inventory screen), each read with what its
/// script names: the stance panel in the screen's lower left corner
/// (`BattleStancePanel`) and the health panel in its upper left
/// (`CharacterHealthPanel`).
#[derive(Debug, Clone, Default)]
pub struct BattleViews {
    pub stance: ScenePanel,
    /// The stance's figures (`_battleStances`): each one's object and the
    /// stance it stands for (`PoseFactor`: 1 standing to 0 crouched, below
    /// that lying), in the list's order.
    pub figures: Vec<(usize, f32)>,
    /// The noise's icon (`_noiseLevel`, a `BattleUINoiseLevel`) and its
    /// sprites by the noise's level (`Sprites`).
    pub noise: Option<usize>,
    pub noise_sprites: Vec<(i32, i64)>,
    /// The speed's notch the script makes twenty of
    /// (`_battleStanceNotchTemplate`), its last (`_finalNotch`) and the row
    /// the twenty are put in (`_notchesPlaceholder`); a notch's colour where
    /// the setting can be had and where not (`BattleStanceNotch`).
    pub notch_template: Option<usize>,
    pub final_notch: Option<usize>,
    pub notches_place: Option<usize>,
    pub notch_colors: [[f32; 4]; 2],
    pub stance_slider: Option<SliderParts>,
    pub speed_slider: Option<SliderParts>,
    /// The legs' and the arms' fills (`_sprintFillImage`, `_handsFillImage`)
    /// and the sprites they are drawn with: the legs', the arms' and either
    /// one's spent (`_defaultFillSprite`, `_defaultFillSpriteForHands`,
    /// `_exhaustedFillSprite`).
    pub legs_fill: Option<usize>,
    pub arms_fill: Option<usize>,
    pub fill_sprites: [Option<(i32, i64)>; 3],
    pub health: ScenePanel,
    /// Its ground (`_background`) and each part's picture
    /// (`BodyPartImage._bodyPartImage`), by the game's name of the part.
    pub background: Option<usize>,
    pub parts: Vec<(&'static str, usize)>,
    /// Where its effects' icons go (`_effectsPanel`) and how its grid lays
    /// them out, and the icon made for each (`_effectIconTemplate`, a
    /// `StackedEffectIcon`) with its count (`_stackCount`).
    pub effects: Option<usize>,
    pub effects_grid: Option<GridGroup>,
    pub effect_template: Option<usize>,
    pub stack_count: Option<usize>,
}

/// A grid of a screen (`GridLayoutGroup`), read by the engine's order of
/// its fields: its padding (left, right, top, bottom), its corner and the
/// axis it fills first (horizontal 0, vertical 1), its cells' size, the
/// space between them.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct GridGroup {
    pub padding: [f32; 4],
    pub start_corner: u32,
    pub start_axis: u32,
    pub cell: [f32; 2],
    pub spacing: [f32; 2],
}

fn grid_group(fields: &[u8]) -> Option<GridGroup> {
    let read = |r: &mut Reader| -> Result<GridGroup> {
        let padding = [r.i32()? as f32, r.i32()? as f32, r.i32()? as f32, r.i32()? as f32];
        let _alignment = r.u32()?;
        let (start_corner, start_axis) = (r.u32()?, r.u32()?);
        Ok(GridGroup { padding, start_corner, start_axis, cell: pair(r)?, spacing: pair(r)? })
    };
    read(&mut Reader::new(fields)).ok()
}

/// A slider of a screen (`Slider`): its object, its handle and the area
/// the handle slides along, the way it runs (`Slider.Direction`: left to
/// right 0, right to left 1, bottom to top 2, top to bottom 3), and its
/// least, most and own values as the scene keeps them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SliderParts {
    pub object: usize,
    pub handle: Option<usize>,
    pub area: Option<usize>,
    pub direction: u32,
    pub min: f32,
    pub max: f32,
    pub value: f32,
}

impl SliderParts {
    /// The slider whose script is on that object: after what every control
    /// has (`selectable`), its fill and its handle, then its direction,
    /// least, most, whether whole numbers, and value.
    pub fn of(panel: &ScenePanel, object: usize) -> Option<Self> {
        let fields = panel.script(object, "Slider")?;
        let (_, at) = selectable(fields)?;
        let handle = panel.transform_at(fields, at + 12);
        let word = |n: usize| -> Option<[u8; 4]> { fields.get(at + 24 + 4 * n..at + 28 + 4 * n)?.try_into().ok() };
        let direction = u32::from_le_bytes(word(0)?);
        let (min, max, value) = (f32::from_le_bytes(word(1)?), f32::from_le_bytes(word(2)?), f32::from_le_bytes(word(4)?));
        Some(Self { object, handle, area: handle.and_then(|handle| panel.nodes[handle].parent), direction, min, max, value })
    }

    /// How far along the slider a value lies, 0 to 1, between a least and a
    /// most (`Slider.normalizedValue`: none where the two are one).
    pub fn share(value: f32, min: f32, max: f32) -> f32 {
        if (max - min).abs() < f32::EPSILON {
            0.0
        } else {
            ((value - min) / (max - min)).clamp(0.0, 1.0)
        }
    }

    /// Where its handle lies at a share of the way along in a panel of that
    /// size (left, top, width, height, y down), as the slider lays it out
    /// (`Slider.UpdateVisuals`): its anchors across the area the slider
    /// runs along both at the share (from the other end for a slider that
    /// runs backwards), and over the whole of it the other way, its own
    /// offset, size and pivot kept. (Not where the scene has it, which is
    /// not always at the scene's value.)
    pub fn handle_at(&self, panel: &ScenePanel, size: [f32; 2], share: f32) -> Option<[f32; 4]> {
        let handle = self.handle?;
        let [x, y, w, h] = panel.rect(self.area?, size);
        let outer = Rect { at: [x, size[1] - y - h], size: [w, h] };
        let axis = usize::from(self.direction >= 2);
        let at = if self.direction % 2 == 1 { 1.0 - share } else { share };
        let mut layout = panel.nodes[handle].layout;
        layout.anchor_min = [0.0, 0.0];
        layout.anchor_max = [1.0, 1.0];
        layout.anchor_min[axis] = at;
        layout.anchor_max[axis] = at;
        layout.scale = [1.0, 1.0];
        let rect = place(layout, outer);
        Some([rect.at[0], size[1] - rect.at[1] - rect.size[1], rect.size[0], rect.size[1]])
    }
}

impl BattleViews {
    /// Reads them from the scene that holds the inventory screen.
    pub fn load(install: &Install, file: &str) -> Result<Self> {
        const SCREEN: &str = "/EFTBattleUIScreen Variant";
        let scene = Scene::load(install, file, true)?;
        let panel_of = |name: &str| -> Result<ScenePanel> {
            let end = format!("{SCREEN}/{name}");
            let top = (0..scene.nodes.len()).find(|&node| scene.path(node).ends_with(&end)).with_context(|| format!("{file} has no {name}"))?;
            Ok(ScenePanel { file: file.to_owned(), ..ScenePanel::of(&scene, top) })
        };
        let mut views = Self { stance: panel_of("BattleStancePanel")?, health: panel_of("CharacterHealthPanel")?, ..Self::default() };
        // The stance panel's script: after what every screen element has,
        // its list of figures, each a pointer and a stance; then its
        // pointers, eleven, and its three sprites.
        let stance = &views.stance;
        let fields = stance.script(0, "BattleStancePanel").context("the stance panel's script")?;
        let mut r = Reader::new(fields.get(UI_ELEMENT..).context("the stance panel's script is short")?);
        let count = r.u32()? as usize;
        for n in 0..count {
            let at = UI_ELEMENT + 4 + 16 * n;
            let factor = f32::from_le_bytes(fields.get(at + 12..at + 16).context("a stance's figure")?.try_into()?);
            if let Some(object) = stance.pointer_at(fields, at) {
                views.figures.push((object, factor));
            }
        }
        let pointers = UI_ELEMENT + 4 + 16 * count;
        let at = |n: usize| stance.pointer_at(fields, pointers + 12 * n);
        views.noise = at(0);
        views.notch_template = at(1);
        views.final_notch = at(3);
        views.notches_place = stance.transform_at(fields, pointers + 12 * 5);
        views.stance_slider = at(6).and_then(|object| SliderParts::of(stance, object));
        views.speed_slider = at(7).and_then(|object| SliderParts::of(stance, object));
        views.legs_fill = at(8);
        views.arms_fill = at(9);
        let sprite = |n: usize| -> Option<(i32, i64)> {
            let to = pointer(&mut Reader::new(fields.get(pointers + 12 * (10 + n)..)?)).ok()?;
            (to.path != 0).then_some((to.file, to.path))
        };
        views.fill_sprites = [sprite(0), sprite(1), sprite(2)];
        // The noise's icon: its image, then its sprites.
        if let Some(fields) = views.noise.and_then(|noise| stance.script(noise, "BattleUINoiseLevel")) {
            let mut r = Reader::new(fields.get(12..).unwrap_or_default());
            let count = r.u32().unwrap_or(0) as usize;
            views.noise_sprites = (0..count).filter_map(|_| pointer(&mut r).ok()).map(|to| (to.file, to.path)).collect();
        }
        // A notch's two colours, then its image.
        if let Some(fields) = views.notch_template.and_then(|notch| stance.script(notch, "BattleStanceNotch")) {
            let mut r = Reader::new(fields);
            if let (Ok(available), Ok(unavailable)) = (color(&mut r), color(&mut r)) {
                views.notch_colors = [available, unavailable];
            }
        }
        // The health panel's script: its ground, its seven parts (each a
        // `BodyPartImage`, whose picture is the first of its own fields
        // after a screen element's), its effects' panel and its icon.
        let health = &views.health;
        let fields = health.script(0, "CharacterHealthPanel").context("the health panel's script")?;
        let at = |n: usize| health.pointer_at(fields, UI_ELEMENT + 12 * n);
        views.background = at(0);
        for (n, part) in ["Head", "Chest", "Stomach", "LeftArm", "RightArm", "LeftLeg", "RightLeg"].into_iter().enumerate() {
            let image = at(1 + n).and_then(|view| health.script(view, "BodyPartImage").and_then(|own| health.pointer_at(own, UI_ELEMENT)).or(Some(view)));
            if let Some(image) = image {
                views.parts.push((part, image));
            }
        }
        views.effects = at(8);
        views.effects_grid = views.effects.and_then(|effects| health.script(effects, "GridLayoutGroup")).and_then(grid_group);
        views.effect_template = at(9);
        views.stack_count = views.effect_template.and_then(|icon| health.script(icon, "StackedEffectIcon")).and_then(|own| {
            // After what every effect icon has: its count's text, the last
            // of its fields.
            own.len().checked_sub(12).and_then(|last| health.pointer_at(own, last))
        });
        Ok(views)
    }
}

/// The fields every element of the game's screens begins its own with
/// (`UIElement`: its disposables and the rest, all empty in a scene), in
/// bytes.
const UI_ELEMENT: usize = 40;

/// The inventory's panel on the right for a container opened in a raid
/// (`Stash Panel`'s `Simple Panel`, `SimpleStashPanel`; a corpse's gear
/// has another, `Complex Loot Panel`), read from the scene with what its
/// scripts name: its header (`_header`) and the text that says what it
/// holds (`_simpleGridName`), its column of filters (`_filterPanel`) and
/// their tabs, the money's count, the container's name in its box
/// (`_containerNamePanel`, `_containerName`), what is only in the lobby
/// (`_lobbyOnlyElements`), the sorting, the sorting table's and the
/// search's tabs, its scroll view (`_stashScroll`) and how it scrolls, the
/// mark of what is had from outside the raid, the pin, lock and priority
/// buttons; and where the container's grids are put (its
/// `SearchableItemView._gridsContainer`). The panel's top is `Stash
/// Panel`.
#[derive(Debug, Clone, Default)]
pub struct StashViews {
    pub panel: ScenePanel,
    pub simple: usize,
    pub header: Option<usize>,
    pub grid_name: Option<usize>,
    pub filters: Option<usize>,
    pub filter_tabs: Vec<FilterTabParts>,
    pub money: Option<usize>,
    pub name_panel: Option<usize>,
    pub name: Option<usize>,
    pub lobby_only: Vec<usize>,
    pub sort: Option<usize>,
    pub sorting_table: Option<usize>,
    pub search: Option<usize>,
    pub scroll: Option<usize>,
    pub external: Option<usize>,
    pub pin: Option<usize>,
    pub lock: Option<usize>,
    pub priority: Option<usize>,
    pub grids: Option<usize>,
    pub scrolling: Option<Scrolling>,
}

/// A filter's tab (`FilterTab`, a `Tab`): its versions idle and chosen,
/// the picture its working or not is shown on (`_targetImage`), and its
/// icon (`_filterIcon`), both drawn at 0.15 of their alpha when it does
/// not work (`SetInteractable`).
#[derive(Debug, Clone, Copy, Default)]
pub struct FilterTabParts {
    pub object: usize,
    pub normal: Option<usize>,
    pub selected: Option<usize>,
    pub target: Option<usize>,
    pub icon: Option<usize>,
}

impl StashViews {
    /// Reads it from the scene that holds the inventory screen.
    pub fn load(install: &Install, file: &str) -> Result<Self> {
        const PANEL: &str = "/InventoryScreen/Items Panel/Stash Panel";
        let scene = Scene::load(install, file, true)?;
        let top = (0..scene.nodes.len()).find(|&node| scene.path(node).ends_with(PANEL)).with_context(|| format!("{file} has no stash panel"))?;
        let panel = ScenePanel { file: file.to_owned(), ..ScenePanel::of(&scene, top) };
        let simple = (0..panel.nodes.len()).find(|&index| panel.script(index, "SimpleStashPanel").is_some()).context("the stash panel has no simple panel")?;
        let fields = panel.script(simple, "SimpleStashPanel").context("the simple panel's script")?;
        // Odin's data and the input node's own field, then the pointers.
        let mut r = Reader::new(fields);
        skip_odin(&mut r)?;
        r.skip(4)?;
        let mut at = fields.len() - r.remaining();
        let mut next = || {
            let found = panel.pointer_at(fields, at);
            at += 12;
            found
        };
        let (header, _simple_view, grid_name, filters, money, name_panel, name) = (next(), next(), next(), next(), next(), next(), next());
        let count = u32::from_le_bytes(fields.get(at..at + 4).context("the lobby's list")?.try_into()?) as usize;
        at += 4;
        let lobby_only = (0..count).filter_map(|n| panel.pointer_at(fields, at + 12 * n)).collect();
        at += 12 * count;
        let mut next = || {
            let found = panel.pointer_at(fields, at);
            at += 12;
            found
        };
        let (sort, sorting_table, search, scroll, external, pin, lock, priority) = (next(), next(), next(), next(), next(), next(), next(), next());
        // Where the grids go: the simple panel's `SearchableItemView`, after
        // Odin's data, its searchable view and its grids' template.
        let grids = panel.script(simple, "SearchableItemView").and_then(|fields| {
            let mut r = Reader::new(fields);
            skip_odin(&mut r).ok()?;
            panel.transform_at(fields, fields.len() - r.remaining() + 24)
        });
        // The filters' tabs, in their order (`_filterTabs`).
        let filter_tabs = filters
            .and_then(|filters| panel.script(filters, "FilterPanel"))
            .and_then(|fields| {
                let mut r = Reader::new(fields);
                skip_odin(&mut r).ok()?;
                let count = r.u32().ok()? as usize;
                let at = fields.len() - r.remaining();
                Some(
                    (0..count)
                        .filter_map(|n| panel.pointer_at(fields, at + 12 * n))
                        .map(|object| {
                            let tab = panel.script(object, "FilterTab").unwrap_or_default();
                            FilterTabParts {
                                object,
                                normal: panel.pointer_at(tab, 0),
                                selected: panel.pointer_at(tab, 12),
                                target: panel.pointer_at(tab, 36),
                                icon: tab.len().checked_sub(12).and_then(|end| panel.pointer_at(tab, end)),
                            }
                        })
                        .collect(),
                )
            })
            .unwrap_or_default();
        let scrolling = scroll.and_then(|scroll| Scrolling::of(&panel, scroll, "ScrollRectNoDrag"));
        header.context("the simple panel's script names no header")?;
        Ok(Self { panel, simple, header, grid_name, filters, filter_tabs, money, name_panel, name, lobby_only, sort, sorting_table, search, scroll, external, pin, lock, priority, grids, scrolling })
    }
}

/// A container's own layout of its grids (`TemplatedGridsView`, the
/// Resource `UI/Rig Layouts/<its layout's name>`): the prefab, and its grid
/// views in the order the container's grids take them (`_presetGridViews`).
pub fn rig_layout(install: &Install, name: &str) -> Result<(ScenePanel, Vec<usize>)> {
    let wanted = format!("ui/rig layouts/{}", name.to_lowercase());
    let paths = resource_paths(install)?;
    let (_, file, id) = paths.iter().find(|(path, ..)| *path == wanted).with_context(|| format!("no resource is at {wanted}"))?;
    let scene = Scene::load_prefab(install, file, *id)?;
    let panel = ScenePanel { file: file.clone(), ..ScenePanel::of(&scene, 0) };
    let views = layout_grids(&panel).context("the layout's script names no grid views")?;
    Ok((panel, views))
}

/// A layout's grid views in their order (`TemplatedGridsView.
/// _presetGridViews`: a count, then so many pointers to grid views).
fn layout_grids(panel: &ScenePanel) -> Option<Vec<usize>> {
    let fields = panel.script(0, "TemplatedGridsView")?;
    let is_grid = |index: Option<usize>| index.is_some_and(|index| panel.nodes[index].scripts.iter().any(|(class, _)| class == "GridView"));
    (0..fields.len().saturating_sub(4)).step_by(4).find_map(|start| {
        let count = u32::from_le_bytes(fields.get(start..start + 4)?.try_into().ok()?) as usize;
        if count == 0 || count > 64 {
            return None;
        }
        let views: Vec<Option<usize>> = (0..count).map(|n| panel.pointer_at(fields, start + 4 + 12 * n)).collect();
        views.iter().all(|&view| is_grid(view)).then(|| views.into_iter().flatten().collect::<Vec<usize>>())
    })
}

/// Containers' own layouts of their grids by name (`rig_layout`), read at
/// once: each with its size and its grids' rectangles (left, top, width,
/// height, y down) in their order. A name with no layout is left out.
pub fn rig_layouts(install: &Install, names: &[String]) -> HashMap<String, ([f32; 2], Vec<[f32; 4]>)> {
    let Ok(paths) = resource_paths(install) else { return HashMap::new() };
    let mut open: HashMap<String, BuildFile> = HashMap::new();
    let mut layouts = HashMap::new();
    for name in names {
        let wanted = format!("ui/rig layouts/{}", name.to_lowercase());
        let Some((_, file, id)) = paths.iter().find(|(path, ..)| *path == wanted) else { continue };
        if !open.contains_key(file) {
            let Ok(build) = BuildFile::open(install, file) else { continue };
            open.insert(file.clone(), build);
        }
        let Some(build) = open.get_mut(file) else { continue };
        let Ok(scene) = Scene::load_prefab_from(install, build, *id) else { continue };
        let panel = ScenePanel { file: file.clone(), ..ScenePanel::of(&scene, 0) };
        let Some(views) = layout_grids(&panel) else { continue };
        let size = panel.nodes[0].layout.size_delta;
        layouts.insert(name.clone(), (size, views.iter().map(|&view| panel.rect(view, size)).collect()));
    }
    layouts
}

/// A sprite among the build's own Resources by its path (`Slots/
/// Equipment/TacticalVest`, as `ResourcesCache.Pop<Sprite>` names it): its
/// file and number.
pub fn resource_sprite(install: &Install, path: &str) -> Option<(String, i64)> {
    let wanted = path.to_lowercase();
    let paths = resource_paths(install).ok()?;
    let mut build: Option<(String, BuildFile)> = None;
    for (at, file, id) in paths.into_iter().filter(|(at, ..)| *at == wanted) {
        let _ = at;
        if build.as_ref().is_none_or(|(name, _)| *name != file) {
            build = Some((file.clone(), BuildFile::open(install, &file).ok()?));
        }
        let (_, opened) = build.as_ref()?;
        if opened.meta.object(id).is_some_and(|object| object.class_id == SPRITE) {
            return Some((file, id));
        }
    }
    None
}

/// The fonts the build keeps whole in one of its own files (`Font`, the
/// engine's class 128, `resources.assets`): each one's name and its font
/// file's bytes (TrueType or OpenType), read by the engine's own order of
/// a font's fields; one whose bytes are not a font file is left out.
pub fn build_fonts(install: &Install, file: &str) -> Result<Vec<(String, Vec<u8>)>> {
    const FONT: i32 = 128;
    let mut build = BuildFile::open(install, file)?;
    let mut fonts = Vec::new();
    let objects: Vec<_> = build.meta.objects.iter().filter(|object| object.class_id == FONT).copied().collect();
    for object in objects {
        let Ok(bytes) = build.bytes(object.offset, object.size) else { continue };
        let big_endian = build.meta.big_endian;
        let read = |r: &mut Reader| -> Result<(String, Vec<u8>)> {
            let name = string(r)?;
            r.skip(4 + 12 + 4 + 12)?; // line spacing, material, size, texture
            r.skip(4 + 4 + 4 + 4 + 4)?; // ASCII start, tracking, spacing, padding, case
            let characters = r.u32()? as usize;
            r.skip(characters * 44)?;
            let kernings = r.u32()? as usize;
            r.skip(kernings * 8)?;
            r.skip(4)?; // pixel scale
            let size = r.u32()? as usize;
            Ok((name, r.bytes(size)?.to_vec()))
        };
        let Ok((name, font)) = read(&mut Reader { data: &bytes, pos: 0, big_endian }) else { continue };
        let signature = font.get(..4).unwrap_or(&[]);
        if [&[0u8, 1, 0, 0][..], b"OTTO", b"true", b"ttcf"].contains(&signature) {
            fonts.push((name, font));
        }
    }
    Ok(fonts)
}

/// The font a TextMeshPro font asset is made from, by its name: the font
/// whose name the asset's begins with, the longest such (`Jovanny Lemonad
/// - Bender Outline SDF` is made from `Jovanny Lemonad - Bender`).
pub fn font_for<'a>(asset: &str, fonts: &'a [(String, Vec<u8>)]) -> Option<&'a (String, Vec<u8>)> {
    fonts.iter().filter(|(name, _)| asset.starts_with(name.as_str())).max_by_key(|(name, _)| name.len())
}

/// A material's saved properties as the build keeps them with no type tree
/// (`Material`: its name, shader, keywords, flags, render queue, tags and
/// passes off, then its textures', whole numbers', floats' and colours'
/// lists, each a name and its value): its floats and colours by name.
#[derive(Debug, Clone, Default)]
pub struct MaterialProperties {
    pub name: String,
    /// The shader's keywords it turns on (`UNDERLAY_ON`).
    pub keywords: Vec<String>,
    pub floats: HashMap<String, f32>,
    pub colors: HashMap<String, [f32; 4]>,
}

/// Reads a material of one of the build's own files.
pub fn material_properties(install: &Install, file: &str, id: i64) -> Result<MaterialProperties> {
    let (class_id, bytes) = object_bytes(install, file, id)?;
    anyhow::ensure!(class_id == 21, "{file} {id} is no material");
    let mut r = Reader::new(&bytes);
    let name = string(&mut r)?;
    pointer(&mut r)?; // shader
    let strings = |r: &mut Reader| -> Result<Vec<String>> { (0..r.u32()?).map(|_| string(r)).collect() };
    let keywords = strings(&mut r)?;
    strings(&mut r)?; // invalid keywords
    r.skip(4)?; // lightmap flags
    r.skip(2)?; // instancing, double-sided GI
    r.align(4);
    r.skip(4)?; // render queue
    for _ in 0..r.u32()? {
        string(&mut r)?;
        string(&mut r)?;
    }
    strings(&mut r)?; // passes off
    for _ in 0..r.u32()? {
        string(&mut r)?;
        r.skip(12 + 8 + 8)?;
    }
    for _ in 0..r.u32()? {
        string(&mut r)?;
        r.skip(4)?;
    }
    let mut floats = HashMap::new();
    for _ in 0..r.u32()? {
        let key = string(&mut r)?;
        floats.insert(key, r.f32()?);
    }
    let mut colors = HashMap::new();
    for _ in 0..r.u32()? {
        let key = string(&mut r)?;
        colors.insert(key, [r.f32()?, r.f32()?, r.f32()?, r.f32()?]);
    }
    Ok(MaterialProperties { name, keywords, floats, colors })
}

/// The name of the shader a material of one of the build's own files is
/// drawn with (`m_Shader`): the shader's own name as its parsed form keeps
/// it, found as the first run of letters in its bytes that begins with the
/// name's usual heads (`TextMeshPro/`, `UI/`, `Hidden/`), as a build keeps
/// no type tree to read it by.
pub fn material_shader(install: &Install, file: &str, id: i64) -> Result<String> {
    let (_, bytes) = object_bytes(install, file, id)?;
    let mut r = Reader::new(&bytes);
    string(&mut r)?;
    let to = pointer(&mut r)?;
    let shader_file = match to.file {
        0 => file.to_owned(),
        outside => {
            let build = BuildFile::open(install, file)?;
            build.meta.externals.get(outside as usize - 1).map(|name| name.rsplit('/').next().unwrap_or(name).to_owned()).context("the shader's file")?
        }
    };
    let (_, shader) = object_bytes(install, &shader_file, to.path)?;
    for head in ["TextMeshPro/", "UI/", "Hidden/"] {
        if let Some(at) = shader.windows(head.len()).position(|w| w == head.as_bytes()) {
            let end = shader[at..].iter().position(|b| !(b.is_ascii_graphic() || *b == b' ')).map_or(shader.len(), |n| at + n);
            return Ok(String::from_utf8_lossy(&shader[at..end]).into_owned());
        }
    }
    bail!("no shader's name found in {shader_file} {}", to.path)
}

/// A TextMeshPro font asset's material (`m_Material`, after the asset's
/// name and its hash): its file and number.
pub fn font_material(install: &Install, file: &str, id: i64) -> Result<(String, i64)> {
    let (_, bytes) = object_bytes(install, file, id)?;
    let mut r = Reader::new(&bytes);
    r.skip(12 + 4 + 12)?;
    string(&mut r)?;
    r.skip(4)?;
    let to = pointer(&mut r)?;
    let name = match to.file {
        0 => file.to_owned(),
        outside => {
            let build = BuildFile::open(install, file)?;
            build.meta.externals.get(outside as usize - 1).map(|name| name.rsplit('/').next().unwrap_or(name).to_owned()).context("the material's file")?
        }
    };
    Ok((name, to.path))
}

/// The font a font asset was made from by its face's family and style,
/// where its own name does not begin with the font's (a font asset named
/// "DON'T USE! Jovanny Lemonad - Bender Bold SDF", whose face is Bender
/// Bold): the font named for the family and the style, or for the family
/// alone in its regular style.
pub fn font_by_face<'a>(family: &str, style: &str, fonts: &'a [(String, Vec<u8>)]) -> Option<&'a (String, Vec<u8>)> {
    let styled = format!("{family} {style}");
    fonts.iter().find(|(name, _)| name.ends_with(&styled)).or_else(|| fonts.iter().find(|(name, _)| style.eq_ignore_ascii_case("regular") && name.ends_with(family)))
}

/// The name of an object of one of the build's own files that is a
/// script's (a `TMP_FontAsset`'s, `BENDER SDF`): after the object it is
/// on, whether it is on, and its script.
pub fn script_name(install: &Install, file: &str, path_id: i64) -> Option<String> {
    let mut build = BuildFile::open(install, file).ok()?;
    let object = *build.meta.object(path_id).filter(|object| object.class_id == class::MONO_BEHAVIOUR)?;
    let bytes = build.bytes(object.offset, object.size).ok()?;
    let mut r = Reader { data: &bytes, pos: 0, big_endian: build.meta.big_endian };
    r.skip(12 + 4 + 12).ok()?;
    string(&mut r).ok()
}

/// What reads a prefab (`Scene::load_prefab`): its file, the scene made,
/// and the files its scripts are named in.
struct PrefabReader<'a> {
    install: Install,
    build: &'a mut BuildFile,
    scene: Scene,
    outside: HashMap<i32, Option<BuildFile>>,
}

impl PrefabReader<'_> {
    fn object(&mut self, id: i64) -> Result<(i32, Vec<u8>)> {
        let object = *self.build.meta.object(id).with_context(|| format!("no object {id}"))?;
        Ok((object.class_id, self.build.bytes(object.offset, object.size)?))
    }

    /// A script's class: the name of the script object it points to, in
    /// one of the file's outside files.
    fn class(&mut self, script: &Pointer) -> Option<String> {
        if script.file <= 0 {
            return None;
        }
        if !self.outside.contains_key(&script.file) {
            let name = self.build.meta.externals.get(script.file as usize - 1).map(|name| name.rsplit('/').next().unwrap_or(name).to_owned());
            let opened = name.and_then(|name| BuildFile::open(&self.install, &name).ok());
            self.outside.insert(script.file, opened);
        }
        let file = self.outside.get_mut(&script.file)?.as_mut()?;
        let object = *file.meta.object(script.path).filter(|object| object.class_id == class::MONO_SCRIPT)?;
        let big_endian = file.meta.big_endian;
        let bytes = file.bytes(object.offset, object.size).ok()?;
        string(&mut Reader { data: &bytes, pos: 0, big_endian }).ok()
    }

    /// A game object, its components and its children, as a node of the
    /// scene under `parent`.
    fn visit(&mut self, go: i64, parent: Option<usize>) -> Result<usize> {
        let big_endian = self.build.meta.big_endian;
        let (_, bytes) = self.object(go)?;
        let mut r = Reader { data: &bytes, pos: 0, big_endian };
        let count = r.i32()?.max(0) as usize;
        let components: Vec<Pointer> = (0..count).map(|_| pointer(&mut r)).collect::<Result<_>>()?;
        r.skip(4)?; // layer
        let name = string(&mut r)?;
        r.skip(2)?; // tag
        let active = r.u8()? != 0;
        let index = self.scene.nodes.len();
        self.scene.objects.insert(go, index);
        self.scene.nodes.push(Node { name, active, parent, children: Vec::new(), layout: None, scripts: Vec::new() });
        let mut children = Vec::new();
        for component in components.iter().filter(|component| component.file == 0) {
            self.scene.components.insert(component.path, index);
            let (class_id, bytes) = self.object(component.path)?;
            let mut r = Reader { data: &bytes, pos: 0, big_endian };
            match class_id {
                class::TRANSFORM | RECT_TRANSFORM => {
                    pointer(&mut r)?;
                    let turn = z_turn([r.f32()?, r.f32()?, r.f32()?, r.f32()?]);
                    r.skip(12)?; // place
                    let scale = pair(&mut r)?;
                    r.skip(4)?;
                    let count = r.i32()?.max(0) as usize;
                    children = (0..count).map(|_| pointer(&mut r).map(|child| child.path)).collect::<Result<_>>()?;
                    pointer(&mut r)?; // father
                    self.scene.transforms.insert(component.path, index);
                    if class_id == RECT_TRANSFORM {
                        let (anchor_min, anchor_max, offset, size_delta, pivot) = (pair(&mut r)?, pair(&mut r)?, pair(&mut r)?, pair(&mut r)?, pair(&mut r)?);
                        self.scene.nodes[index].layout = Some(Layout { anchor_min, anchor_max, offset, size_delta, pivot, scale, turn });
                    }
                }
                class::MONO_BEHAVIOUR => {
                    pointer(&mut r)?;
                    let enabled = flag(&mut r)?;
                    let script = pointer(&mut r)?;
                    string(&mut r)?;
                    let start = self.scene.data.len();
                    self.scene.data.extend_from_slice(&bytes[r.pos..]);
                    self.scene.behaviours.push(Behaviour { id: component.path, node: index, enabled, fields: (start, bytes.len() - r.pos) });
                    if let Some(class) = self.class(&script) {
                        self.scene.classes.insert(component.path, class.clone());
                        self.scene.nodes[index].scripts.push(class);
                    }
                }
                _ => {}
            }
        }
        // Each child transform's own game object, in the transform's order.
        for child in children {
            let (_, bytes) = self.object(child)?;
            let owner = pointer(&mut Reader { data: &bytes, pos: 0, big_endian })?;
            let node = self.visit(owner.path, Some(index))?;
            self.scene.nodes[index].children.push(node);
        }
        Ok(index)
    }
}

/// One object of one of the build's own files by its number: its class and
/// its bytes as kept (`probe object`).
pub fn object_bytes(install: &Install, file: &str, id: i64) -> Result<(i32, Vec<u8>)> {
    let mut build = BuildFile::open(install, file)?;
    let object = *build.meta.object(id).with_context(|| format!("{file} has no object {id}"))?;
    Ok((object.class_id, build.bytes(object.offset, object.size)?))
}

/// One of the build's own files, its table read from its head and its
/// objects by their ranges, as wanted: `resources.assets` is some 300 MB.
struct BuildFile {
    file: std::fs::File,
    meta: SerializedFile,
}

impl BuildFile {
    fn open(install: &Install, name: &str) -> Result<Self> {
        use std::io::Read;
        let path = install.root.join("EscapeFromTarkov_Data").join(name);
        let mut file = std::fs::File::open(&path).with_context(|| format!("opening {}", path.display()))?;
        let len = file.metadata()?.len() as usize;
        let mut first = [0u8; 48];
        file.read_exact(&mut first)?;
        let mut head = vec![0; SerializedFile::head_size(&first)?.min(len)];
        let mut file = std::fs::File::open(&path)?;
        file.read_exact(&mut head)?;
        let meta = SerializedFile::parse_head(&head, len).with_context(|| name.to_owned())?;
        Ok(Self { file, meta })
    }

    fn bytes(&mut self, offset: usize, size: usize) -> Result<Vec<u8>> {
        use std::io::{Read, Seek, SeekFrom};
        self.file.seek(SeekFrom::Start(offset as u64))?;
        let mut bytes = vec![0; size];
        self.file.read_exact(&mut bytes)?;
        Ok(bytes)
    }
}

/// The tree of a class as a bundle of the install's has it, to read the
/// build's own files by (`ScenePictures::read_by`).
pub fn tree_of(install: &Install, key: &str, class: i32) -> Result<crate::unity::typetree::TypeTree> {
    let mut assets = crate::unity::Assets::new(install.clone())?;
    let files = assets.load(key)?;
    files
        .iter()
        .find_map(|&file| assets.file(file).meta.types.iter().find(|ty| ty.class_id == class && !ty.tree.is_empty()).map(|ty| ty.tree.clone()))
        .with_context(|| format!("{key} has no object of class {class} with its tree"))
}

impl ScenePictures {
    /// Finds the trees: in the first bundle of the install's pictures for
    /// its screens (`assets/content/textures/gui/`) that has both classes.
    pub fn new(install: &Install) -> Result<Self> {
        let mut assets = crate::unity::Assets::new(install.clone())?;
        let mut keys: Vec<String> = assets.manifest.entries.keys().filter(|key| key.starts_with("assets/content/textures/gui/")).cloned().collect();
        keys.sort();
        for key in keys {
            let Ok(files) = assets.load(&key) else { continue };
            let tree = |class: i32| {
                files.iter().find_map(|&file| assets.file(file).meta.types.iter().find(|ty| ty.class_id == class && !ty.tree.is_empty()).map(|ty| ty.tree.clone()))
            };
            if let (Some(sprite_tree), Some(texture_tree)) = (tree(SPRITE), tree(class::TEXTURE_2D)) {
                let folder = install.root.join("EscapeFromTarkov_Data");
                return Ok(Self { install: install.clone(), folder, sprite_tree, texture_tree, files: HashMap::new() });
            }
        }
        anyhow::bail!("no bundle of the install's screen pictures has a sprite to read sprites by")
    }

    fn file(&mut self, name: &str) -> Result<&mut BuildFile> {
        if !self.files.contains_key(name) {
            let file = BuildFile::open(&self.install, name)?;
            self.files.insert(name.to_owned(), file);
        }
        Ok(self.files.get_mut(name).expect("just opened"))
    }

    fn read(&mut self, name: &str, path_id: i64, class: i32) -> Result<crate::unity::Value> {
        let tree = if class == SPRITE { self.sprite_tree.clone() } else { self.texture_tree.clone() };
        self.read_by(name, path_id, class, &tree)
    }

    /// An object of one of the build's files read by a tree of its class
    /// borrowed from a bundle (`tree_of`).
    pub fn read_by(&mut self, name: &str, path_id: i64, class: i32, tree: &crate::unity::typetree::TypeTree) -> Result<crate::unity::Value> {
        let file = self.file(name)?;
        let object = *file.meta.object(path_id).with_context(|| format!("no object {path_id} in {name}"))?;
        anyhow::ensure!(object.class_id == class, "object {path_id} in {name} is of class {}, not {class}", object.class_id);
        let big_endian = file.meta.big_endian;
        let bytes = file.bytes(object.offset, object.size)?;
        let mut r = Reader { data: &bytes, pos: 0, big_endian };
        tree.read(&mut r).with_context(|| format!("object {path_id} of {name}"))
    }

    /// The numbers of the objects of a class in one of the build's files.
    pub fn objects(&mut self, name: &str, class: i32) -> Result<Vec<i64>> {
        Ok(self.file(name)?.meta.objects.iter().filter(|object| object.class_id == class).map(|object| object.path_id).collect())
    }

    /// A sprite of that file and number.
    pub fn sprite(&mut self, name: &str, path_id: i64) -> Result<SpriteCut> {
        let v = self.read(name, path_id, SPRITE)?;
        let rd = v.req("m_RD")?;
        let texture = rd.req("texture")?.as_pptr().context("the sprite's texture")?;
        let externals = &self.file(name)?.meta.externals;
        let file = match texture.file_id {
            0 => name.to_owned(),
            n => externals.get(n as usize - 1).map(|outside| outside.rsplit('/').next().unwrap_or(outside).to_owned()).context("the texture's file")?,
        };
        let rect = rd.req("textureRect")?;
        let field = |value: &crate::unity::Value, name: &str| value.get(name).and_then(|f| f.as_f32()).unwrap_or(0.0);
        let border = v.req("m_Border")?.as_vec4().unwrap_or([0.0; 4]);
        let whole = v.req("m_Rect")?;
        let offset = rd.req("textureRectOffset")?;
        // Packed (the lowest bit), and turned or flipped (bits 2 to 5).
        let settings = rd.get("settingsRaw").and_then(|s| s.as_u32()).unwrap_or(0);
        Ok(SpriteCut {
            name: v.req("m_Name")?.as_str().unwrap_or("").to_owned(),
            texture: (file, texture.path_id),
            rect: [field(rect, "x"), field(rect, "y"), field(rect, "width"), field(rect, "height")],
            border,
            pixels_per_unit: v.req("m_PixelsToUnits")?.as_f32().unwrap_or(100.0),
            whole: [field(whole, "x"), field(whole, "y"), field(whole, "width"), field(whole, "height")],
            offset: [field(offset, "x"), field(offset, "y")],
            turned: settings & 1 != 0 && (settings >> 2) & 0xf != 0,
        })
    }

    /// A texture of that file and number, with its levels: in the file
    /// itself, or in the file beside it that its stream names.
    pub fn texture(&mut self, name: &str, path_id: i64) -> Result<crate::unity::extract::TextureData> {
        let v = self.read(name, path_id, class::TEXTURE_2D)?;
        let inline = v.req("image data")?.as_bytes().unwrap_or(&[]);
        let data = if !inline.is_empty() {
            inline.to_vec()
        } else {
            let stream = v.req("m_StreamData")?;
            let (path, offset, size) = (stream.req("path")?.as_str().unwrap_or(""), stream.req("offset")?.as_usize().unwrap_or(0), stream.req("size")?.as_usize().unwrap_or(0));
            let beside = self.folder.join(path.rsplit('/').next().unwrap_or(path));
            let read = || -> std::io::Result<Vec<u8>> {
                use std::io::{Read, Seek, SeekFrom};
                let mut file = std::fs::File::open(&beside)?;
                file.seek(SeekFrom::Start(offset as u64))?;
                let mut data = vec![0; size];
                file.read_exact(&mut data)?;
                Ok(data)
            };
            read().with_context(|| format!("reading {}", beside.display()))?
        };
        Ok(crate::unity::extract::TextureData { data, ..crate::unity::extract::texture_header(&v)? })
    }
}

#[cfg(test)]
impl TmpFont {
    /// A font asset of the glyphs given (each letter with its glyph's
    /// index), for a test of what draws from one.
    pub(crate) fn for_test(point_size: f32, ascent: f32, descent: f32, glyphs: &[(char, u32, TmpGlyph)], normal_spacing: f32, bold_spacing: f32) -> Self {
        Self {
            point_size,
            scale: 1.0,
            ascent,
            descent,
            line_height: ascent - descent,
            glyphs: glyphs.iter().map(|(_, index, glyph)| (*index, (glyph.advance, glyph.scale))).collect(),
            letters: glyphs.iter().map(|(letter, index, _)| (*letter as u32, (*index, 1.0))).collect(),
            glyph_table: glyphs.iter().map(|(_, index, glyph)| (*index, *glyph)).collect(),
            atlases: Vec::new(),
            normal_spacing,
            bold_spacing,
            bold: None,
            family: String::new(),
            style: String::new(),
        }
    }
}
