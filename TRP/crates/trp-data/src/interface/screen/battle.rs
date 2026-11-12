//! The battle views: mount icons, action panel, grid groups and sliders.

use super::*;

/// The battle screen's two panels of the character (the battle screen's own object,
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
    /// The in-raid interaction panel (`ActionPanel`), where the scene has
    /// it; else why it could not be read.
    pub action: Option<ActionPanelParts>,
    pub action_missing: Option<String>,
    /// The mount's icon in the stance panel (`_bipodPanel`, a
    /// `BattleUiBipodPanel` on `MountStance`), where the scene has it.
    pub mount: Option<MountIcon>,
}

/// The stance panel's mount icon (`BattleUiBipodPanel`; mounting spec,
/// "The battle screen's icon"): the image shown while mounted
/// (`_mountImage`, `MountStance/Mount`), each side's arrow and the game's
/// number of its side (`_mountDirections`: 0 forward, -1 left, 1 right),
/// the red image flashed when nothing is found (`_mountUnavailableImage`,
/// at alpha 0 in the scene) and the flash's length (`_highlightDuration`).
#[derive(Debug, Clone, Default)]
pub struct MountIcon {
    pub image: usize,
    pub directions: Vec<(usize, i32)>,
    pub unavailable: usize,
    pub highlight: f32,
}

impl MountIcon {
    /// Read from the panel's script: after what every screen element has,
    /// its list of directions (each a pointer to an object and the side's
    /// number), then its two pointers and the flash's length.
    pub(super) fn read(panel: &ScenePanel) -> Option<Self> {
        let node = (0..panel.nodes.len()).find(|&index| panel.script(index, "BattleUiBipodPanel").is_some())?;
        let fields = panel.script(node, "BattleUiBipodPanel")?;
        let mut r = Reader::new(fields.get(UI_ELEMENT..)?);
        let count = r.u32().ok()? as usize;
        let mut directions = Vec::new();
        for n in 0..count {
            let at = UI_ELEMENT + 4 + 16 * n;
            let side = i32::from_le_bytes(fields.get(at + 12..at + 16)?.try_into().ok()?);
            directions.push((panel.pointer_at(fields, at)?, side));
        }
        let after = UI_ELEMENT + 4 + 16 * count;
        let image = panel.pointer_at(fields, after)?;
        let unavailable = panel.pointer_at(fields, after + 12)?;
        let highlight = f32::from_le_bytes(fields.get(after + 24..after + 28)?.try_into().ok()?);
        Some(Self { image, directions, unavailable, highlight })
    }

    /// Every object the script turns on and off.
    pub fn scripted(&self) -> Vec<usize> {
        std::iter::once(self.image).chain(self.directions.iter().map(|(object, _)| *object)).collect()
    }
}

/// The battle screen's interaction panel (`ActionPanel`; inventory spec,
/// "The menu of what is looked at"): its objects, the pointer's three
/// sprites (`UIPointer`: over something to do, refused, and the sense of
/// a thing near), and how far the panel moves while aiming
/// (`_aimingShift`, in the canvas's units, y up).
#[derive(Debug, Clone)]
pub struct ActionPanelParts {
    pub panel: ScenePanel,
    /// The pointer (`Cursor`), a line's ground and words (the template's
    /// `Image` and `Text`), the list's ground (`Background`), the name's
    /// ground and words (`Item Panel`), the error's (`Error Panel`).
    pub cursor: usize,
    pub line_ground: usize,
    pub line_text: usize,
    pub list_ground: usize,
    pub name_ground: usize,
    pub name_text: usize,
    pub error_ground: usize,
    pub error_text: usize,
    pub hover_sprite: Option<(i32, i64)>,
    pub unavailable_sprite: Option<(i32, i64)>,
    pub sense_sprite: Option<(i32, i64)>,
    pub aiming_shift: [f32; 2],
}

impl ActionPanelParts {
    pub(super) fn read(panel: ScenePanel) -> Result<Self> {
        let find = |path: &str| panel.find(path).with_context(|| format!("the interaction panel has no {path}"));
        let cursor = find("Cursor")?;
        // The pointer's own: its image, then its three sprites, each a
        // reference (a file's number and an object's).
        let sprites = panel.script(cursor, "UIPointer").context("the interaction panel's pointer has no UIPointer")?;
        let sprite = |at: usize| -> Option<(i32, i64)> {
            let to = pointer(&mut Reader::new(sprites.get(at..)?)).ok()?;
            (to.path != 0).then_some((to.file, to.path))
        };
        // The panel's own script: its eight references, then the shift
        // (two floats, 140 bytes in: `local\actionpanel_fields_s130.txt`).
        let shift = panel.script(0, "ActionPanel").and_then(|fields| {
            let x = f32::from_le_bytes(fields.get(140..144)?.try_into().ok()?);
            let y = f32::from_le_bytes(fields.get(144..148)?.try_into().ok()?);
            Some([x, y])
        });
        Ok(Self {
            cursor,
            line_ground: find("Interaction Buttons Container/Interaction Button Template/Image")?,
            line_text: find("Interaction Buttons Container/Interaction Button Template/Text")?,
            list_ground: find("Interaction Buttons Container/Background")?,
            name_ground: find("Item Panel/Image")?,
            name_text: find("Item Panel/Text")?,
            error_ground: find("Error Panel/Image")?,
            error_text: find("Error Panel/Text")?,
            hover_sprite: sprite(12),
            unavailable_sprite: sprite(24),
            sense_sprite: sprite(36),
            aiming_shift: shift.unwrap_or([0.0, -200.0]),
            panel,
        })
    }
}

/// A grid of a screen (`GridLayoutGroup`), read by the engine's order of
/// its fields: its padding (left, right, top, bottom), its corner and the
/// axis it fills first (horizontal 0, vertical 1), its cells' size, the
/// space between them, and its columns where it holds them to a number
/// (`m_Constraint` 1, `FixedColumnCount`, and `m_ConstraintCount`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct GridGroup {
    pub padding: [f32; 4],
    /// Where its cells lie in it (`childAlignment`: upper left 0 to lower
    /// right 8, row by row).
    pub alignment: u32,
    pub start_corner: u32,
    pub start_axis: u32,
    pub cell: [f32; 2],
    pub spacing: [f32; 2],
    pub columns: Option<i32>,
}

fn grid_group(fields: &[u8]) -> Option<GridGroup> {
    let read = |r: &mut Reader| -> Result<GridGroup> {
        let padding = [r.i32()? as f32, r.i32()? as f32, r.i32()? as f32, r.i32()? as f32];
        let alignment = r.u32()?;
        let (start_corner, start_axis) = (r.u32()?, r.u32()?);
        let (cell, spacing) = (pair(r)?, pair(r)?);
        let constraint = r.u32().ok();
        let count = r.i32().ok();
        let columns = (constraint == Some(1)).then_some(count).flatten();
        Ok(GridGroup { padding, alignment, start_corner, start_axis, cell, spacing, columns })
    };
    read(&mut Reader::new(fields)).ok()
}

impl ScenePanel {
    /// The grid on one of its objects, a `GridLayoutGroup` or the game's
    /// own kind of it (`FlexibleGridLayoutGroup`, whose cells are as wide
    /// as its columns share its width).
    pub fn grid(&self, index: usize) -> Option<GridGroup> {
        self.script(index, "GridLayoutGroup").or_else(|| self.script(index, "FlexibleGridLayoutGroup")).and_then(grid_group)
    }
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
        views.mount = MountIcon::read(&views.stance);
        // The interaction panel, where the scene has it whole.
        match panel_of("ActionPanel").and_then(ActionPanelParts::read) {
            Ok(parts) => views.action = Some(parts),
            Err(e) => views.action_missing = Some(format!("{e:#}")),
        }
        Ok(views)
    }
}
