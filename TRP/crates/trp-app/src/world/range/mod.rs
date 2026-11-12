//! The test range as the simulation feels it, level ground and a few solid boxes.
//! The boxes are the posts along the lane, the ring of blocks, a wall to walk up
//! to, and a platform with stairs to fall from. The weapon is pushed back
//! by them, the character stops at them, stands on their tops, and vaults
//! or climbs the low ones.
//!
//! This is the app standing in for a host game's world: the simulation
//! asks a host a handful of things (`trp_sim::World`: how high the ground
//! is, what a line hits, where a move ends, how far a box has to move to
//! be clear), and a real host answers them from its own. Where a move
//! ends is answered by the rules of the game's own controller for the
//! player (`trp_sim::collision`; movement spec, "The character's own
//! collision"), so the character meets a wall, a step and an edge here as
//! it does in the game.
//!
//! The range also has things for the hands to work (`docs/spec/interactions.md`),
//! each the game's model of its kind (`trp_sim::things`) on a shape of the
//! app's: two doors in the high wall, a car's trunk, and containers like
//! the ones recorded by hand (a wooden crate on the floor, a
//! technical supply crate, a dead Scav, a drawer). Their shapes and places
//! are the app's; what each asks of the hand and how it opens are the
//! recorded things'. Where the airdrop crate stood, the hideout's
//! workbench (`Workbench`; workbench spec).

use bevy::prelude::*;
use trp_sim::World;
use trp_sim::locomotion::collision::{Block, Site};
use trp_sim::locomotion::movement::Capsule;
use trp_sim::held::things::{turn, Hinge, Thing, ThingSettings};
use trp_sim::Interaction;

mod surfaces;
mod benches;
mod building;
mod starts;
mod labels;
mod world_queries;
mod as_world;

pub use surfaces::*;
pub use benches::*;
pub use starts::*;
pub use labels::*;
use surfaces::{BULLET_GROUND, BULLET_LEAVES, DOORWAY, DOOR_WALL_Z, KNEE_HIGH, KNEE_LEFT, KNEE_RIGHT, LEAF, LINTEL, METAL, PLASTIC, SILL, WALL_HALF, WALL_HIGH, WALL_THICK, WINDOW, WOOD, box_name, ground_surface, of_its_own};
use benches::{ALONG_THE_WALL, AMMO_BOX_HOLDS, BENCH_LEG, BOX_ASIDE, CABINETS_BACK, CABINETS_GAP, CABINETS_NEAR, CABINET_HOLDS, Furniture, GUN_CASE, GUN_CASE_HOLDS, GUN_CASE_OPENS, LEDGE, LEDGE_STEPS, MAGAZINE_BOX, MAGAZINE_BOX_HOLDS, MAGAZINE_BOX_OPENS, TABLE_LEG, TABLE_SLAB, WEAPON_TABLE, WEAPON_TABLE_FIRST, WEAPON_TABLE_STEP};
#[cfg(test)]
use benches::{ARMOR_TABLE, BENCH_REACH, GUN_TABLE};
#[cfg(test)]
use benches::BENCH_TRIGGERS_GAME;
use starts::{cabinet_at, cabinet_prop, made_of, made_start, weapon_table, weapon_table_prop};
use labels::{HALL_GREY, cabinet_named, leaf_block, lid_of, weapon_crate_named};

#[derive(Clone, Resource)]
pub struct Range {
    pub solids: Vec<Solid>,
    /// The things the hands work and what they stand on, each moved and
    /// turned as one by a level file (`level`), and its solids and things
    /// among the range's.
    pub props: Vec<Prop>,
    /// The level file read at the start, if one was.
    level: Option<std::sync::Arc<crate::world::level::Level>>,
    /// The radius of the character's capsule, in metres, from the player
    /// prefab. (The player's controller is the game's own and keeps no
    /// skin outside it: recorded, a character stops the radius from a
    /// wall.)
    pub radius: f32,
    /// The boxes as the character's collision has them, in the
    /// simulation's axes: the solids, then the doors' leaves as they stand.
    site: Site,
    /// The things the hands work.
    pub things: Vec<Placed>,
    /// What they go by, from the install; until it is read nothing turns.
    pub thing_settings: Option<ThingSettings>,
    /// The benches that stand, where (`GUN BENCH`, `ARMOR BENCH`).
    pub benches: Vec<Workbench>,
}

/// A prop of the range: its name in a level file, where it stands (its
/// anchor's place, as made on the ground, and its turn), and which of the
/// range's solids and things are its.
#[derive(Clone)]
pub struct Prop {
    pub name: String,
    pub stand: crate::world::level::Stand,
    solids: std::ops::Range<usize>,
    things: std::ops::Range<usize>,
}

/// Where the props without solids are made about (Bevy's axes): the barbed
/// wire's footprint's middle, the sampler's, the rounds on the floor.
fn wire_anchor() -> Vec3 {
    let [low, high] = BARBED_WIRE;
    Vec3::new((low.x + high.x) / 2.0, 0.0, (low.z + high.z) / 2.0)
}
fn sampler_anchor() -> Vec3 {
    Vec3::new(31.0 + SAMPLER_SHIFT[0], 0.0, -6.0 + SAMPLER_SHIFT[1])
}
/// Where the sampler was moved from its first place (TRP's range: from the
/// lanes' right to the yard's left, behind the row of
/// things; `COURSE` keeps the strips' places as first laid).
const SAMPLER_SHIFT: [f32; 2] = [-49.0, 19.1];
const WIRE_PROP: &str = "barbed wire";
const SAMPLER_PROP: &str = "sampler";
const ROUNDS_PROP: &str = "rounds";

/// Where a prop stands by the level read at the start (as made, if none
/// was read or the file has it not).
fn stand_of(name: &str, anchor: Vec3) -> crate::world::level::Stand {
    crate::world::level::get().and_then(|level| level.props.get(name)).copied().unwrap_or(crate::world::level::Stand::made(anchor))
}

/// The platform's height, in metres: a fall of 0.71 s, long enough for the
/// sights to go down. The lower platform (the shelf) beside it is a fall
/// too short for that.
const PLATFORM: f32 = 2.5;
const SHELF: f32 = 1.0;
/// How many stairs lead up to the platform, and how deep each is. Each
/// rises 0.1 m, which the game's capsule rides up as it walks into it (a
/// rise up to 0.36 of its radius, 13 cm, pushes its rounded foot more up
/// than back).
const STAIRS: usize = 24;
const TREAD: f32 = 0.4;
/// Where the stairs begin, in Bevy's axes: their left side, to the left of
/// the start, and their foot, behind the start's line. They lead the
/// lane's way up to the platform, outside the hall's left wall (the
/// platform's right side on it; the lower platform on its
/// left).
const STAIRS_LEFT: f32 = -19.4;
const STAIRS_FOOT: f32 = 4.4;
/// How far out the ring of blocks stands, all round the start: beyond the
/// hall (where 18 m would put two of it in the lanes).
const RING: f32 = 48.0;

/// The range is open to the sky: the hall
/// that stood about the lanes from 0.76.0 to 0.99.0 (its walls 8 m high
/// with their windows, its roof, its indoors and its lamps) is gone. Its
/// left wall stood on the platform's right side, which this still names;
/// the storage stands against the stairs' side (TRP's range).
#[cfg(test)]
const HALL_LEFT: f32 = STAIRS_LEFT + 5.0;
/// Down the range past the firing line, across: the knee-high wall's ends.
const LANES: [f32; 2] = [KNEE_LEFT, KNEE_RIGHT];
/// Cover down the lanes, TRP's own: a block 2.9 m high turned 28.3
/// degrees, and a low wall 1.2 m high across the lanes (middle, size,
/// turn in degrees; Bevy's axes). The block 10 cm further from the left
/// wall than drawn, where it went 9 cm into it; the low wall square to the
/// lanes, drawn half a degree off.
/// (To the right of the lanes on TRP's range, the pair turned the other
/// way from where it first stood, to their left.)
const COVER: [(&str, Vec3, Vec3, f32); 2] = [
    ("cover block", Vec3::new(11.8, 1.449, -14.8), Vec3::new(2.898, 2.898, 8.923), -28.32),
    ("low wall", Vec3::new(7.07, 0.6, -11.8), Vec3::new(2.883, 1.2, 0.4), 0.0),
];

/// Where each of the range's things stands: the middle of its footprint,
/// in Bevy's axes. They stand shoulder to shoulder in a row behind the
/// door wall, facing it, with the car at the row's right end, its trunk to
/// the row. The block to climb is off to the right.
const ROW: f32 = DOOR_WALL_Z + 4.5;
const SUPPLY_CRATE: [f32; 2] = [-5.6, ROW - 0.8];
const DRAWER: [f32; 2] = [-4.0, ROW];
const SCAV: [f32; 2] = [-2.0, ROW];
const WOODEN_CRATE: [f32; 2] = [0.3, ROW];
const TABLE: [f32; 2] = [1.9, ROW];
const HIGH_SHELF: [f32; 2] = [3.7, ROW];
const ROUNDS: [f32; 2] = [-1.0, DOOR_WALL_Z + 2.4];
const CAR: [f32; 2] = [6.9, ROW - 1.5];
/// Four more containers of `courses10`'s takes, on along the row past the
/// car's front, the row's 0.85 m between each: a weapon box, a
/// toolbox, a safe and an airdrop weapon crate (`main::MORE_CONTAINERS`).
const WEAPON_BOX: [f32; 2] = [10.55, ROW];
const TOOLBOX: [f32; 2] = [12.45, ROW];
const SAFE: [f32; 2] = [13.85, ROW];
const AIRDROP_CRATE: [f32; 2] = [15.55, ROW];
/// Their sizes (along the row, high, deep), the game's own models' as
/// boxes: the weapon box's, the toolbox's and the safe's colliders as the
/// takes recorded them (`weapon-box-search-a`'s `plastic_suitcase_140_open`,
/// `dead-scav-search-a`'s `scontainer_toolbox_01`, `search-and-loot`'s
/// `safe_01_A`, each in its own axes), the airdrop crate's collider mesh in
/// the install (`scontainer_crate.bundle`, `scontainer_airdrop_box_04_
/// COLLIDER`'s bounds, its up the mesh's z). A lid's 4 cm are in the height.
const WEAPON_BOX_SIZE: Vec3 = Vec3::new(1.43, 0.6, 0.63);
const TOOLBOX_SIZE: Vec3 = Vec3::new(0.7, 0.31, 0.33);
const SAFE_SIZE: Vec3 = Vec3::new(0.36, 0.54, 0.36);
const AIRDROP_CRATE_SIZE: Vec3 = Vec3::new(1.29, 0.66, 0.91);
/// The safe's door, on its front (as recorded: 0.29 m across, 0.49 high).
const SAFE_DOOR: Vec3 = Vec3::new(0.29, 0.49, 0.04);
const TABLE_TOP: f32 = 0.8;
const SHELF_TOP: f32 = 1.4;
/// The small white table by the yard door, where the door's key lies: against the door wall's face on the lane's side, left of the
/// doorway and clear of its left leaf's swing (the leaf turns between its
/// hinge and the doorway's middle); its length along the wall, height and
/// depth, and its prop's name. The app's.
const KEY_TABLE: [f32; 2] = [DOORWAY[0] - 0.75, DOOR_WALL_Z - WALL_THICK / 2.0 - 0.21];
const KEY_TABLE_SIZE: Vec3 = Vec3::new(0.6, 0.75, 0.4);
const KEY_TABLE_PROP: &str = "key table";

/// A point or a way of the simulation (z ahead) in Bevy's axes (z behind),
/// and back: the same flip.
fn in_bevy(v: [f32; 3]) -> Vec3 {
    Vec3::new(v[0], v[1], -v[2])
}

fn in_sim(v: Vec3) -> [f32; 3] {
    [v.x, v.y, -v.z]
}

impl Default for Range {
    /// The range, with the level read at the start if one was.
    fn default() -> Self {
        Self::of(crate::world::level::get().map(|level| std::sync::Arc::new(level.clone())))
    }
}

#[cfg(test)]
mod tests;
