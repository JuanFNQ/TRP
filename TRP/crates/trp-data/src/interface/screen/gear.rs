//! The GEAR panel and its slots.

use super::*;

/// The equipment slots the panel's script names a view for, in the order
/// of its fields (`EquipmentTab`: earpiece, headwear, face cover, armband,
/// armour, eyewear, primary weapon, holster, secondary weapon, scabbard,
/// dogtag).
pub(super) const GEAR_VIEWS: [&str; 11] =
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
