//! The screen's measures, colours, tabs and panels.

use super::*;

/// The tabs in the game's order (`EInventoryTab`), with whether each can
/// be opened here. The captions are plain English words, not the game's
/// own texts.
///
/// The last is not the game's: its settings are a screen of its menu, and
/// the app has them here, as a tab (`options_view`), next to the others.
pub const TABS: [(&str, bool); 9] = [
    ("OVERALL", true),
    ("GEAR", true),
    ("HEALTH", true),
    ("SKILLS", true),
    ("MAP", false),
    ("TASKS", false),
    ("ACHIEVEMENTS", false),
    ("PRESTIGE", false),
    ("OPTIONS", true),
];

/// The OVERALL tab (`overall_view`).
pub const OVERALL: usize = 0;

/// The SKILLS tab (`skills_view`).
pub const SKILLS: usize = 3;

/// The tab the game opens on.
pub(crate) const GEAR: usize = 1;

/// The health tab, which shows the gear tab's page with the health panel
/// in its left panel (`ItemsPanel.UpdateEquipment`).
pub(crate) const HEALTH: usize = 2;

/// The options' tab.
pub const OPTIONS: usize = 8;

/// The tabs the HUD page's HIDE UNUSED TABS takes off the bar (Map, Tasks,
/// Prestige, Achievements): the game's last four, none of them built here.
pub const UNUSED_TABS: std::ops::Range<usize> = 4..8;

/// Whether the HUD page takes the unused tabs off the bar.
pub(super) fn tabs_hidden(options: Option<&crate::screens::options_view::OptionsState>) -> bool {
    options.is_some_and(|options| options.hide_tabs)
}

/// The plain row's tabs taken off where the HUD page asks, at once.
pub(super) fn hide_plain_tabs(options: Option<Res<crate::screens::options_view::OptionsState>>, mut tabs: Query<(&TabButton, &mut Node), Without<GameTab>>) {
    let hidden = tabs_hidden(options.as_deref());
    for (tab, mut node) in &mut tabs {
        let want = if hidden && UNUSED_TABS.contains(&tab.0) { Display::None } else { Display::Flex };
        if node.display != want {
            node.display = want;
        }
    }
}

/// Whether a tab can be opened: one built, and with the options'
/// `INVENTORY SYSTEM` off, the options' alone.
pub(super) fn tab_open(index: usize, features: &crate::screens::options_view::Features) -> bool {
    TABS[index].1 && (features.inventory || index == OPTIONS)
}

/// The equipment panel's views in the order the game's panel names them,
/// each with a caption (an English word of the app's) and where it sits
/// if the install's own panel could not be read: left, top, width and
/// height in the panel's units, three to a row. Where the install's panel
/// is read (`trp_data::screen::GearPanel`), its places are used instead.
pub(super) const EQUIPMENT: [(&str, &str, [f32; 4]); 11] = [
    ("Earpiece", "EARPIECE", [0.0, 20.0, 125.0, 125.0]),
    ("Headwear", "HEADWEAR", [184.0, 20.0, 125.0, 125.0]),
    ("FaceCover", "FACE COVER", [368.0, 20.0, 125.0, 125.0]),
    ("ArmBand", "ARMBAND", [0.0, 186.0, 125.0, 44.0]),
    ("ArmorVest", "BODY ARMOR", [184.0, 186.0, 125.0, 125.0]),
    ("Eyewear", "EYEWEAR", [368.0, 186.0, 125.0, 125.0]),
    ("FirstPrimaryWeapon", "ON SLING", [0.0, 354.0, 310.0, 125.0]),
    ("Holster", "HOLSTER", [368.0, 354.0, 125.0, 125.0]),
    ("SecondPrimaryWeapon", "ON BACK", [0.0, 524.0, 310.0, 125.0]),
    ("Scabbard", "SHEATH", [368.0, 524.0, 125.0, 125.0]),
    ("Dogtag", "DOGTAG", [0.0, 270.0, 125.0, 50.0]),
];

/// The panel's own size where the install's is not read.
const PANEL_SIZE: [f32; 2] = [494.0, 650.0];

/// The equipment panel's size in its own units: the install's, or the
/// app's where that could not be read.
pub fn gear_panel(carried: &Carried) -> [f32; 2] {
    if carried.panel.slots.is_empty() {
        PANEL_SIZE
    } else {
        carried.panel.size
    }
}

/// The figure behind the slots, as plain shapes in the panel's units:
/// left, top, width, height and how round the corners are. The app's own
/// drawing of a paper figure; the game has none in this panel (in its
/// menu the character stands in the scene behind it).
pub(super) const FIGURE: [[f32; 5]; 9] = [
    [204.0, 22.0, 86.0, 104.0, 43.0],
    [231.0, 118.0, 32.0, 30.0, 0.0],
    [150.0, 142.0, 194.0, 110.0, 34.0],
    [172.0, 200.0, 150.0, 190.0, 18.0],
    [106.0, 150.0, 50.0, 262.0, 24.0],
    [338.0, 150.0, 50.0, 262.0, 24.0],
    [168.0, 366.0, 158.0, 70.0, 20.0],
    [168.0, 408.0, 74.0, 238.0, 24.0],
    [252.0, 408.0, 74.0, 238.0, 24.0],
];

/// The containers panel's views, in the game's order.
pub(super) const CONTAINERS: [(&str, &str); 4] = [("TacticalVest", "TACTICAL RIG"), ("Pockets", "POCKETS"), ("Backpack", "BACKPACK"), ("SecuredContainer", "POUCH")];

/// The game lays its screens out 1080 units high (the inventory screen is
/// 1920 wide and as high as the screen); a unit here is that share of the
/// window's height.
pub const SCREEN_HEIGHT: f32 = 1080.0;

/// How high the game's bar of tabs is (`Tab Bar`), in those units.
pub(super) const TAB_BAR: f32 = 43.0;

/// The side of a grid's cell (`GridView`), a slot view's side and the
/// height of the caption over one, and the stash panel's width, in those
/// units: the first from the game's code, the others from its scene.
pub(crate) const CELL: f32 = 63.0;

pub(super) const SLOT_SIDE: f32 = 125.0;

pub(super) const CAPTION: f32 = 20.0;

pub(super) const STASH_WIDTH: f32 = 680.0;

/// How far ahead of the character a thing is put down, and how near the
/// line of sight has to pass a thing on the ground for it to count as
/// looked at. ESTIMATE, both: the game throws a thing and lets it fall,
/// and goes by the thing's own collider.
pub(crate) const DROP_AHEAD: f32 = 0.4;

pub(super) const LOOSE_REACH: f32 = 0.25;

/// Where a magazine let fall lies: so far ahead of the character and so
/// far to its right, in metres. ESTIMATE: the game drops it from the
/// weapon's own magazine and lets it fall (`DropMod`); this is under
/// where a rifle is held.
pub(super) const MAGAZINE_DROP: [f32; 2] = [0.35, 0.05];

/// A place on the ground so far ahead of a character standing at
/// `position` (the simulation's axes) and turned to `yaw`, and so far to
/// its right: in the engine's axes.
pub(crate) fn beside(position: [f32; 3], yaw: f32, [ahead, right]: [f32; 2]) -> Vec3 {
    let (sin, cos) = yaw.to_radians().sin_cos();
    Vec3::new(position[0] + sin * ahead + cos * right, position[1], -(position[2] + cos * ahead - sin * right))
}

/// How many pixels one of the game's screen units is in this window.
#[derive(Resource, Clone, Copy, PartialEq)]
pub(super) struct Units(pub(super) f32);

impl Units {
    /// A cell's side in pixels, a fraction included, as the game's grid
    /// view tiles its cells (63 units each at the screen's scale). Rounded
    /// to whole pixels, the things in a grid fell a fraction
    /// of a pixel more off the grid's own lines at each row in a window
    /// whose unit is not a whole share of a pixel (2560 by 1392: 80 pixels
    /// for 79.86), several pixels down a drawer scrolled far, the cells
    /// behind them out of place the further down the list.
    pub(super) fn cell(self) -> f32 {
        CELL * self.0
    }
}

pub const BACK: Color = Color::srgba(0.05, 0.06, 0.06, 0.94);

pub const PANEL: Color = Color::srgb(0.11, 0.12, 0.12);

pub const LINE: Color = Color::srgb(0.30, 0.31, 0.30);

pub const WORDS: Color = Color::srgb(0.80, 0.80, 0.74);

pub const FAINT: Color = Color::srgb(0.45, 0.46, 0.43);

pub(super) const THING: Color = Color::srgb(0.20, 0.24, 0.27);

/// The game's colour for the box under the mouse (`ItemView.
/// DefaultSelectedColor`: white at 77 of 255), laid over the box's own.
pub(super) const POINTED: (Srgba, f32) = (Srgba::WHITE, 77.0 / 255.0);

/// What goes with it, in orange: the game's own
/// orange for a box (a thing with a repair's bonus, `GridItemView`), laid
/// over the box at a share the app's, and its border.
pub(super) const GOES_WITH: (Srgba, f32) = (Srgba::new(209.0 / 255.0, 153.0 / 255.0, 45.0 / 255.0, 1.0), 0.40);

pub(super) const SLOT: Color = Color::srgba(0.07, 0.08, 0.08, 0.62);

pub(super) const SHAPE: Color = Color::srgb(0.27, 0.26, 0.22);
