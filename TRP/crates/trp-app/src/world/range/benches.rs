//! The furniture, the benches, their tables and the workbench.

use super::*;

/// What a place of the row against the staircase is.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum Furniture {
    /// A cabinet with a drawer, as the row's (0.8 m high).
    Cabinet,
    /// A box with a lid hinged along its back, as the range's boxes.
    Box,
    /// A dresser, taller, its drawer near its top.
    Dresser,
}

impl Furniture {
    pub(super) fn of(label: &str) -> Self {
        match label {
            PLATES => Self::Box,
            EYEWEAR => Self::Dresser,
            _ => Self::Cabinet,
        }
    }

    /// Its size: along the stairs, high, deep (from its back to its
    /// front); and the height of its drawer's middle.
    pub(super) fn size(self) -> Vec3 {
        match self {
            Self::Cabinet => Vec3::new(0.8, 0.8, 0.5),
            Self::Box => Vec3::new(0.6, 0.6, 0.45),
            Self::Dresser => Vec3::new(1.0, 1.1, 0.5),
        }
    }

    pub(super) fn drawer_height(self) -> f32 {
        match self {
            Self::Dresser => 0.95,
            _ => 0.64,
        }
    }

    /// What the prompt and the screen call it after its label.
    pub(super) fn word(self) -> &'static str {
        match self {
            Self::Box => "box",
            Self::Dresser => "dresser",
            Self::Cabinet => "drawer",
        }
    }
}

/// Where the row's near end is along the stairs (Bevy's z: by the stairs'
/// foot), and the room between two of its places.
pub(super) const CABINETS_NEAR: f32 = 3.6;

pub(super) const CABINETS_GAP: f32 = 0.25;

/// The row's backs, across the lanes: 2 cm off the stairs' side.
pub(super) const CABINETS_BACK: f32 = STAIRS_LEFT + 2.5 + 0.02;

/// The first cabinet by its place in `Carried::away`.
pub(super) const CABINET_HOLDS: usize = MAGAZINE_BOX_HOLDS + WEAPON_CRATES.len();

/// The tables of weapons with a box of rounds beside their magazine box
///: the first of `WEAPON_CRATES`, all but the medical and the
/// grenades' tables; and the first box of rounds by its place in
/// `Carried::away`, after the cabinets. The app's.
pub const AMMO_TABLES: usize = 8;

pub(super) const AMMO_BOX_HOLDS: usize = CABINET_HOLDS + CABINETS.len();

/// How far the two boxes under a table of weapons sit to each side of its
/// middle, along it.
pub(super) const BOX_ASIDE: f32 = 0.23;

/// A ledge high enough to be hurt falling from (the game's falls hurt from
/// 3 m: health spec, "Falls"): a second flight of stairs leads on from the
/// platform, the same rise a step, to a ledge 6 m up. The app's.
pub(super) const LEDGE: f32 = 6.0;

pub(super) const LEDGE_STEPS: usize = 35;

/// Where the first table stands across the lane (its gun case half a metre
/// from the airdrop crate), and how far each next one is from the last.
pub(super) const WEAPON_TABLE_FIRST: f32 = 1.5;

pub(super) const WEAPON_TABLE_STEP: f32 = 1.2;

/// How many tables stand along the wall to the workbench's right; the rest
/// stand along it to its left, each from the bench out, every table's back
/// to the wall (TRP's range: the arm of tables once turned towards the
/// start is gone, its tables to the other side). The app's.
pub(super) const ALONG_THE_WALL: usize = 4;

/// The range's two benches: two tables of the hideout's own level 3 bench
/// (`10_workbench/level3` of the install's `bunker_2.unity`), each drawn by
/// `workbench`, lying along what it stands against, its back to it.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum BenchKind {
    /// The gun bench, where the airdrop crate once stood: the
    /// level's second metal table with its vise (`highlight_transform/
    /// metal_table2 (1)` and `highlight_transform/Vise`, less the AKS-74U
    /// held in the vise, `aksu`: the app's reading of "the vise table"),
    /// its back to the knee-high wall's face, its middle on the lane's
    /// (`BENCH_AT`). Until 0.159.0 the workbench, the main metal table.
    Gun,
    /// The armor bench, past the rigs' cabinet on the start's side, in line
    /// with the storage cabinets (the app's reading of "beside the clothing
    /// cabinets"): the level's main metal table against the
    /// hideout's wall (`highlight_transform/metal_table (2)`, the
    /// workbench's from 0.96.1 to 0.158.0), its back on the cabinets' backs'
    /// line, `CABINETS_GAP` past the rigs' cabinet.
    Armor,
}

/// A bench's table as `probe sceneprops bunker_2.unity 10_workbench/level3
/// "<its path>"` measures it, in the level's own axes (the engine's, in
/// metres): its drawn corners (its feet and top; its back the high side
/// across it), and whether it lies along the level's x (the vise table,
/// which stands end on to the hideout's wall there, its vise at its far end)
/// or along its z (the main table, along that wall). The game stands the
/// vise table across; here it lies along as the other does (both lie
/// horizontal), its back the side whose edge the vise is flush with.
pub(super) struct Table {
    pub(super) low: [f32; 3],
    pub(super) high: [f32; 3],
    along_x: bool,
}

pub(super) const GUN_TABLE: Table = Table { low: [5.756106, -2.454, 11.98292], high: [6.978802, -1.5600317, 12.606691], along_x: true };

pub(super) const ARMOR_TABLE: Table = Table { low: [6.674887, -2.454, 12.603228], high: [7.563946, -1.560655, 13.918263], along_x: false };

impl Table {
    pub(super) fn length(&self) -> f32 {
        if self.along_x { self.high[0] - self.low[0] } else { self.high[2] - self.low[2] }
    }

    pub(super) fn depth(&self) -> f32 {
        if self.along_x { self.high[2] - self.low[2] } else { self.high[0] - self.low[0] }
    }

    /// A point of the level's axes by how far it is in from the table's
    /// back, its height there, and how far along from the table's low end.
    pub(super) fn at(&self, from_back: f32, y: f32, along: f32) -> [f32; 3] {
        if self.along_x { [self.low[0] + along, y, self.high[2] - from_back] } else { [self.high[0] - from_back, y, self.low[2] + along] }
    }
}

/// The gun bench's middle across the lanes (Bevy's x: the crate's middle,
/// the app's).
const BENCH_AT: f32 = 0.0;

/// The tables as the range's solids: each one's top as a slab within its
/// drawn corners, as measured, and a leg at each corner, all metal
/// (`ESTIMATE`, the slab's thickness and the legs' side: the game's is its
/// mesh collider, which the range's boxes are not; not one box
/// about the whole table, which would leave nothing under it to be seen or
/// reached: the boxes of repair kits stand there now). The vise is drawn,
/// not solid (`ESTIMATE`: what it is without the rifle in it is not
/// measured).
const BENCH_SLAB: f32 = 0.05;

pub(super) const BENCH_LEG: f32 = 0.06;

/// The boxes of repair kits under the gun bench: the armour's kits and the weapons', each the
/// game's ammunition box as the tables' magazine boxes are, side by side on
/// the floor under the table, each just inside the legs of its back corner
/// (a centimetre off); their labels, and their places in
/// `Carried::away`, after the tables' boxes of rounds. The app's.
pub const REPAIR_BOXES: [&str; 2] = ["ARMOR REPAIR", "WEAPON REPAIR"];

pub const REPAIR_BOX_HOLDS: usize = AMMO_BOX_HOLDS + AMMO_TABLES;

/// The places in `Carried::away` of the four past the car (`WEAPON_BOX`),
/// after the boxes of repair kits, in their order along the row.
pub const MORE_HOLDS: usize = REPAIR_BOX_HOLDS + REPAIR_BOXES.len();

/// The game's two triggers that put the main table's bench in a character's
/// sight (`HideoutInteractionTrigger` on `wokbench_lvl3_raycast_collider`
/// and its `wokbench_lvl3_area_trigger`, each a box collider; entered, the
/// area is in sight, `HideoutPlayerOwner.HideoutAreaInteraction`), as
/// measured: low and high corners in the level's axes. They are the whole
/// level 3 bench's, 3.8 m along its wall, and reached 0.61 m past the main
/// table's one end and 0.83 m past its other.
#[cfg(test)]
pub(super) const BENCH_TRIGGERS_GAME: [([f32; 3], [f32; 3]); 2] = [([5.746, -2.42, 11.99], [6.676, -0.44, 13.89]), ([6.116, -2.42, 12.415], [7.046, -0.44, 14.745])];

/// **Each bench's zone cut to its table**: one box along the table from
/// end to end, from its front out as far as the game's raycast collider
/// reached before the main table (5.746 to its front's 6.675 in the level's
/// x), as high as the game's triggers. The app's, a departure.
pub(super) const BENCH_REACH: f32 = 0.929;

const BENCH_TRIGGER_Y: [f32; 2] = [-2.42, -0.44];

/// Which benches stand (the options' `GUN BENCH`, by its old key
/// `workbench`, and `ARMOR BENCH`), set once as the run begins, before the
/// range is made.
static WORKBENCH_ON: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

static ARMOR_BENCH_ON: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

/// Sets whether the gun bench stands (`GUN BENCH`).
pub fn set_workbench(on: bool) {
    WORKBENCH_ON.store(on, std::sync::atomic::Ordering::Relaxed);
}

/// Sets whether the armor bench stands (`ARMOR BENCH`).
pub fn set_armor_bench(on: bool) {
    ARMOR_BENCH_ON.store(on, std::sync::atomic::Ordering::Relaxed);
}

impl BenchKind {
    pub const ALL: [BenchKind; 2] = [BenchKind::Gun, BenchKind::Armor];

    pub(super) fn table(self) -> &'static Table {
        match self {
            BenchKind::Gun => &GUN_TABLE,
            BenchKind::Armor => &ARMOR_TABLE,
        }
    }

    /// Whether it stands (its switch).
    pub fn on(self) -> bool {
        match self {
            BenchKind::Gun => WORKBENCH_ON.load(std::sync::atomic::Ordering::Relaxed),
            BenchKind::Armor => ARMOR_BENCH_ON.load(std::sync::atomic::Ordering::Relaxed),
        }
    }

    /// Its prop's name, for a level file: the gun bench keeps the
    /// workbench's, which level files already name.
    pub fn prop(self) -> &'static str {
        match self {
            BenchKind::Gun => "workbench",
            BenchKind::Armor => "armor bench",
        }
    }

    /// Its name over its menu, and its solids' (TRP's own words).
    pub fn name(self) -> &'static str {
        match self {
            BenchKind::Gun => "Gun bench",
            BenchKind::Armor => "Armor bench",
        }
    }

    /// The level's objects drawn for it (their paths under
    /// `10_workbench/level3`), and of those, the ones left out by name with
    /// all under them.
    pub fn drawn(self) -> (&'static [&'static str], &'static [&'static str]) {
        match self {
            BenchKind::Gun => (&["highlight_transform/metal_table2 (1)", "highlight_transform/Vise"], &["aksu"]),
            BenchKind::Armor => (&["highlight_transform/metal_table (2)"], &[]),
        }
    }

    /// Its own axes (the level's, the engine's) to Bevy's, as made, its
    /// feet on the ground.
    pub(super) fn made(self) -> Transform {
        let t = self.table();
        match self {
            // Unturned, the level's x along the knee-high wall, its back
            // (the level's high z) on the wall's face, its middle at
            // `BENCH_AT`.
            BenchKind::Gun => Transform::from_translation(Vec3::new(BENCH_AT - (t.low[0] + t.high[0]) / 2.0, -t.low[1], KNEE_WALL_Z + WALL_THICK / 2.0 + t.high[2])),
            // Turned a half, the level's z along the stairs, its back (the
            // level's high x) on the cabinets' backs' line, its low end
            // `CABINETS_GAP` past the rigs' cabinet, on the start's side.
            BenchKind::Armor => Transform::from_translation(Vec3::new(CABINETS_BACK + t.high[0], -t.low[1], CABINETS_NEAR + CABINETS_GAP - t.low[2])).with_rotation(Quat::from_rotation_y(std::f32::consts::PI)),
        }
    }

    /// A point of its own axes in Bevy's, as made.
    pub(super) fn point(self, p: [f32; 3]) -> Vec3 {
        self.made().transform_point(Vec3::new(p[0], p[1], -p[2]))
    }

    /// A box of its own axes (two corners) in Bevy's, as made: its middle
    /// and size (upright and square to the lanes, as each is turned by
    /// halves).
    pub(super) fn boxed(self, low: [f32; 3], high: [f32; 3]) -> (Vec3, Vec3) {
        let (a, b) = (self.point(low), self.point(high));
        ((a + b) / 2.0, (a - b).abs())
    }

    /// Where its prop is made about: under its middle, at its back.
    pub(super) fn anchor(self) -> Vec3 {
        let t = self.table();
        let at = self.point(t.at(0.0, t.low[1], t.length() / 2.0));
        Vec3::new(at.x, 0.0, at.z)
    }

    /// Before its middle, its front that far off, on the ground.
    pub(super) fn before(self, off: f32) -> Vec3 {
        let t = self.table();
        let at = self.point(t.at(t.depth() + off, t.low[1], t.length() / 2.0));
        Vec3::new(at.x, 0.0, at.z)
    }

    /// Its solids, in its own axes: its top's slab and a leg at each
    /// corner.
    pub(super) fn solids(self) -> Vec<([f32; 3], [f32; 3])> {
        let t = self.table();
        let (top, length, depth) = (t.high[1], t.length(), t.depth());
        let corners = |a: [f32; 3], b: [f32; 3]| ([a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2])], [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])]);
        let mut solids = vec![corners(t.at(0.0, top - BENCH_SLAB, 0.0), t.at(depth, top, length))];
        for from_back in [0.0, depth - BENCH_LEG] {
            for along in [0.0, length - BENCH_LEG] {
                solids.push(corners(t.at(from_back, t.low[1], along), t.at(from_back + BENCH_LEG, top - BENCH_SLAB, along + BENCH_LEG)));
            }
        }
        solids
    }

    /// Its zone, in its own axes: before its front, end to end.
    pub(super) fn trigger(self) -> ([f32; 3], [f32; 3]) {
        let t = self.table();
        let (a, b) = (t.at(t.depth(), BENCH_TRIGGER_Y[0], 0.0), t.at(t.depth() + BENCH_REACH, BENCH_TRIGGER_Y[1], t.length()));
        ([a[0].min(b[0]), a[1], a[2].min(b[2])], [a[0].max(b[0]), b[1], a[2].max(b[2])])
    }
}

/// A bench on the range: which, where its model's top stands (its own axes
/// to Bevy's), and its zone, a box in Bevy's axes (low and high corners:
/// upright, where the bench stands).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Workbench {
    pub kind: BenchKind,
    pub place: Transform,
    pub trigger: [Vec3; 2],
}

impl Workbench {
    /// Whether a character standing there (its feet, Bevy's axes), its
    /// capsule of that radius, is in its zone: the bench is in its sight.
    pub fn in_sight(&self, feet: Vec3, radius: f32) -> bool {
        let [low, high] = self.trigger;
        let near = Vec3::new(feet.x.clamp(low.x, high.x), feet.y.clamp(low.y, high.y), feet.z.clamp(low.z, high.z));
        Vec2::new(feet.x - near.x, feet.z - near.z).length() <= radius && feet.y + 1.0 >= low.y && feet.y <= high.y
    }
}

/// A table: how long along the wall, how high and how deep; its top's
/// thickness, and its legs' side. The app's.
pub(super) const WEAPON_TABLE: Vec3 = Vec3::new(1.0, 0.85, 0.5);

pub(super) const TABLE_SLAB: f32 = 0.04;

pub(super) const TABLE_LEG: f32 = 0.05;

/// The gun case on a table, long and low (the game's own weapon box, the
/// `WeaponCrate` container whose grid is 5 by 2, is a long flat case), and
/// the magazine box under it, square (the game's `AmmoBox`, 3 by 3): their
/// sizes, and how far each lid opens. The app's shapes and angles
/// (`ESTIMATE`): the game's models of them are not read. The box's lid
/// opens no further than the table over it lets it.
pub(super) const GUN_CASE: Vec3 = Vec3::new(0.95, 0.13, 0.34);

pub(super) const MAGAZINE_BOX: Vec3 = Vec3::new(0.42, 0.42, 0.42);

pub(super) const GUN_CASE_OPENS: f32 = 80.0;

pub(super) const MAGAZINE_BOX_OPENS: f32 = 55.0;

/// The first gun case and the first magazine box by their places in
/// `Carried::away`.
pub(super) const GUN_CASE_HOLDS: usize = 5;

pub(super) const MAGAZINE_BOX_HOLDS: usize = GUN_CASE_HOLDS + WEAPON_CRATES.len();
