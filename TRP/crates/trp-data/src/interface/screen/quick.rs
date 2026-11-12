//! The quick slots and the ammo selector's look.

use super::*;

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
    /// The battle screen's bar's magazine picker, where it is read.
    pub ammo_selector: Option<AmmoSelectorLook>,
}

/// The magazine picker of the battle screen's quick bar (`AmmoSelector` on
/// `AcceptableMagsPanel`; firing spec, "The magazine picker"), as the scene
/// has it: its object; the cancel's view and the ground its colour is set
/// on (`_cancelView`, `_cancelBackground`) and that view's size (its
/// `LayoutElement`'s preferred); where it is put from the weapon's view
/// (`_arrowOffset`, screen pixels, y up); the cancel's ground unchosen and
/// chosen (`_normalCancelColor`, `_selectedCancelColor`); and its flow's
/// padding (left, right, top, bottom), spacing (across, down) and width
/// (`FlowLayoutGroup`, its own width kept: the `ContentSizeFitter` fits
/// its height alone).
#[derive(Debug, Clone)]
pub struct AmmoSelectorLook {
    pub object: usize,
    pub cancel: usize,
    pub cancel_ground: usize,
    pub cancel_size: [f32; 2],
    pub arrow_offset: [f32; 2],
    pub normal: [f32; 4],
    pub selected: [f32; 4],
    pub padding: [f32; 4],
    pub spacing: [f32; 2],
    pub width: f32,
}

impl AmmoSelectorLook {
    pub(super) fn read(panel: &ScenePanel) -> Option<Self> {
        let object = panel.find("AcceptableMagsPanel")?;
        let fields = panel.script(object, "AmmoSelector")?;
        let float = |at: usize| fields.get(at..at + 4).map(|word| f32::from_le_bytes([word[0], word[1], word[2], word[3]]));
        // The class's own fields last: the two pointers (twelve bytes
        // each), the offset, the two colours.
        let tail = fields.len().checked_sub(40)?;
        let colour = |at: usize| -> Option<[f32; 4]> { Some([float(at)?, float(at + 4)?, float(at + 8)?, float(at + 12)?]) };
        let cancel = tail.checked_sub(24).and_then(|at| panel.pointer_at(fields, at)).or_else(|| panel.find("AcceptableMagsPanel/Cancel"))?;
        let cancel_ground = tail.checked_sub(12).and_then(|at| panel.pointer_at(fields, at)).or_else(|| panel.find("AcceptableMagsPanel/Cancel/Color Panel"))?;
        let flow = panel.script(object, "FlowLayoutGroup")?;
        let int = |at: usize| flow.get(at..at + 4).map_or(0.0, |word| i32::from_le_bytes([word[0], word[1], word[2], word[3]]) as f32);
        let flow_float = |at: usize| flow.get(at..at + 4).map_or(0.0, |word| f32::from_le_bytes([word[0], word[1], word[2], word[3]]));
        Some(Self {
            object,
            cancel,
            cancel_ground,
            cancel_size: panel.nodes[cancel].element.map_or([64.0, 64.0], |element| element.preferred),
            arrow_offset: [float(tail)?, float(tail + 4)?],
            normal: colour(tail + 8)?,
            selected: colour(tail + 24)?,
            padding: [int(0), int(4), int(8), int(12)],
            spacing: [flow_float(20), flow_float(24)],
            width: panel.nodes[object].layout.size_delta[0],
        })
    }

    /// Where each of so many views of these sizes lies in the picker, and
    /// the picker's own size, in its units from its top left: in a row
    /// from the left, a view that would pass the width beginning the next
    /// row under the tallest of the last, each at its row's top
    /// (`FlowLayoutGroup.SetLayout`, `LayoutRow`, the upper left
    /// alignment); the height the rows' with the padding.
    pub fn flow(&self, sizes: &[[f32; 2]]) -> (Vec<[f32; 2]>, [f32; 2]) {
        let [left, right, top, bottom] = self.padding;
        let room = self.width - left - right;
        let (mut x, mut y, mut row) = (0.0f32, top, 0.0f32);
        let mut places = Vec::with_capacity(sizes.len());
        for (index, &[w, h]) in sizes.iter().enumerate() {
            let w = w.min(room);
            if index > 0 && x + w > room {
                y += row + self.spacing[1];
                x = 0.0;
                row = 0.0;
            }
            places.push([left + x, y]);
            x += w + self.spacing[0];
            row = row.max(h);
        }
        (places, [self.width, y + row + bottom])
    }
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

    /// The battle screen's quick bar (`...BattleUIScreen Variant/
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
        let ammo_selector = AmmoSelectorLook::read(&panel);
        Ok(Self { panel, slots, ammo_selector })
    }

    /// The panel's place on a screen so wide and high, in its units: left,
    /// top, width and height, y down (its parent the whole screen).
    pub fn quick_in(&self, screen: [f32; 2]) -> [f32; 4] {
        let rect = place(self.panel.nodes[0].layout, Rect { at: [0.0, 0.0], size: screen });
        [rect.at[0], screen[1] - rect.at[1] - rect.size[1], rect.size[0], rect.size[1]]
    }
}
