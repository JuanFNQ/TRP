//! The test range as the simulation feels it: level ground and a few solid
//! boxes (the posts along the lane, the ring of blocks, a wall to walk up
//! to, and a platform with stairs to fall from). The weapon is pushed back
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
//! the ones recorded by hand on 2026-10-12 (a wooden crate on the floor, a
//! technical supply crate, a dead Scav, a drawer). Their shapes and places
//! are the app's; what each asks of the hand and how it opens are the
//! recorded things'. Where the airdrop crate stood, the hideout's
//! workbench (`Workbench`; workbench spec).

use bevy::prelude::*;
use trp_sim::World;
use trp_sim::collision::{Block, Site};
use trp_sim::movement::Capsule;
use trp_sim::things::{turn, Hinge, Thing, ThingSettings};
use trp_sim::Interaction;

/// A box, upright, in Bevy's axes, turned about the upright by `yaw`
/// (radians, the right hand's way about Bevy's up: square to the axes at
/// nought, as the range makes its own; a level file's may be turned), and
/// what it is made of for the sounds of moving (`BaseBallistic.
/// ESurfaceSound`'s number, `trp_data::sound::SURFACES`), and what it is
/// called in a level file (`level`).
#[derive(Clone, Copy)]
pub struct Solid {
    pub name: &'static str,
    pub centre: Vec3,
    pub size: Vec3,
    pub yaw: f32,
    pub color: Color,
    pub surface: u8,
    /// Over the character's reach: the hall's roofs. Drawn, and met by a
    /// line of sight, a cast or a blast's line, but not part of the
    /// character's own collision (its site): the site's ground under a
    /// point is the highest top over it, and a roof would be that ground
    /// everywhere under it.
    pub overhead: bool,
}

/// The numbers a bullet knows the doors' leaves (by their things' places)
/// and the ground by, past the boxes' (`Range::bullet_hits`).
const BULLET_LEAVES: u64 = 1 << 20;
const BULLET_GROUND: u64 = 1 << 21;

/// The thing whose moving part a bullet's thing is (a door's leaf), by its
/// place among the range's things.
pub fn bullet_thing_part(thing: u64) -> Option<usize> {
    (BULLET_LEAVES..BULLET_GROUND).contains(&thing).then(|| (thing - BULLET_LEAVES) as usize)
}

/// The surfaces the range's boxes are made of (`ESurfaceSound`); anything
/// not said is concrete, the game's surface for a thing without one.
const METAL: u8 = 1;
const WOOD: u8 = 2;
const PLASTIC: u8 = 16;
pub const METAL_SURFACE: u8 = METAL;

/// The sampler of surfaces on the ground (the user, 2026-10-16: "May want
/// to add different materials on the ground to walk on"): since 0.77.2 the
/// user's course outside the hall's wide opening (laid out in Blender,
/// 2026-10-19: "Here is the file for my new firing range", then "just
/// replace our range with this one"): a strip for each of the game's
/// twenty surfaces, by number, its middle in Bevy's x and z (`COURSE`), in
/// two rows across the right of the hall, each strip `STRIP` across and
/// along (drawn 4 cm smaller, as the user drew them: 1.76 by 6 m). The
/// strips are drawn on the ground and are no solids: a step's line down
/// meets the ground inside one, and hears its surface (`docs/spec/
/// footsteps.md`). Their places, sizes and colours are the app's and the
/// user's.
const COURSE: [[f32; 2]; 20] = [
    [33.0, -9.0],
    [35.0, -9.0],
    [37.0, -9.0],
    [39.0, -9.0],
    [31.0, -9.0],
    [29.0, -9.0],
    [27.0, -9.0],
    [25.0, -9.0],
    [41.0, -9.0],
    [39.0, -3.0],
    [21.0, -9.0],
    [23.0, -9.0],
    [31.0, -3.0],
    [35.0, -3.0],
    [33.0, -3.0],
    [41.0, -3.0],
    [27.0, -3.0],
    [25.0, -3.0],
    [29.0, -3.0],
    [37.0, -3.0],
];
const STRIP: Vec2 = Vec2::new(1.8, 6.04);
const SURFACE_COLORS: [[f32; 3]; 20] = [
    [0.60, 0.60, 0.58],
    [0.42, 0.46, 0.50],
    [0.50, 0.36, 0.22],
    [0.36, 0.27, 0.18],
    [0.30, 0.45, 0.20],
    [0.20, 0.20, 0.21],
    [0.60, 0.78, 0.80],
    [0.55, 0.52, 0.46],
    [0.58, 0.62, 0.66],
    [0.22, 0.30, 0.36],
    [0.30, 0.33, 0.38],
    [0.85, 0.84, 0.80],
    [0.38, 0.26, 0.15],
    [0.30, 0.33, 0.20],
    [0.40, 0.35, 0.28],
    [0.45, 0.32, 0.24],
    [0.20, 0.45, 0.70],
    [0.70, 0.88, 0.90],
    [0.65, 0.50, 0.32],
    [0.93, 0.94, 0.96],
];

/// A square of the sampler: its surface's number, its middle on the ground
/// (Bevy's axes), how far it reaches across (its own x and z), its turn
/// about the upright and its colour. Drawn 4 cm smaller, a little apart
/// from the next.
#[derive(Clone, Copy, Debug)]
pub struct Square {
    pub surface: u8,
    pub centre: Vec3,
    pub size: Vec2,
    pub yaw: f32,
    pub color: Color,
}

impl Square {
    /// A level file's square (`sampler part <n>`): as large as it is drawn
    /// there, and reaching 4 cm more, so that it is drawn as the file has it.
    pub fn of(surface: u8, found: &crate::level::LevelBox) -> Self {
        let bottom = (found.centre.y - found.size.y / 2.0).max(0.0);
        Self { surface, centre: Vec3::new(found.centre.x, bottom, found.centre.z), size: Vec2::new(found.size.x, found.size.z) + 0.04, yaw: found.yaw, color: found.color }
    }

    /// Whether a place on the ground (Bevy's x and z) is on it.
    fn covers(&self, x: f32, z: f32) -> bool {
        let own = Quat::from_rotation_y(-self.yaw) * Vec3::new(x - self.centre.x, 0.0, z - self.centre.z);
        own.x.abs() <= self.size.x / 2.0 && own.z.abs() <= self.size.y / 2.0
    }
}

/// The sampler's squares: a level file's, each where the file has it, else
/// the range's course where the sampler stands.
pub fn surface_squares() -> &'static [Square] {
    static SQUARES: std::sync::OnceLock<Vec<Square>> = std::sync::OnceLock::new();
    SQUARES.get_or_init(|| {
        if let Some(squares) = crate::level::get().and_then(|level| level.squares.as_ref()) {
            return squares.iter().map(|(surface, found)| Square::of(*surface, found)).collect();
        }
        let stand = stand_of(SAMPLER_PROP, sampler_anchor());
        SURFACE_COLORS
            .into_iter()
            .enumerate()
            .map(|(surface, [r, g, b])| {
                let [x, z] = COURSE[surface];
                let [x, z] = [x + SAMPLER_SHIFT[0], z + SAMPLER_SHIFT[1]];
                let centre = stand.point(sampler_anchor(), Vec3::new(x, 0.0, z));
                Square { surface: surface as u8, centre, size: STRIP, yaw: stand.yaw, color: Color::srgb(r, g, b) }
            })
            .collect()
    })
}

/// The surface of the ground at a place (Bevy's x and z): a square's of the
/// sampler, or concrete.
fn ground_surface(x: f32, z: f32) -> u8 {
    surface_squares().iter().find(|square| square.covers(x, z)).map_or(trp_sim::steps::CONCRETE, |square| square.surface)
}

/// A thing of the range the hands work: the game's model of it, the name
/// the prompt gives it, what of it moves, and what a line of sight meets.
#[derive(Clone)]
pub struct Placed {
    pub name: &'static str,
    pub thing: Thing,
    pub part: Part,
    /// What a line of sight meets of it when it is not a door, in the
    /// simulation's axes (a door is met by its leaf).
    hit: Block,
    /// Its place in the simulation's axes, which a search measures the
    /// character's distance from (`transform.position`).
    pub place: [f32; 3],
    /// The container's own item, by its place in `Carried::away`.
    pub holds: Option<usize>,
    /// A door's leaf among the site's blocks.
    leaf: Option<usize>,
}

/// What of a thing moves.
#[derive(Clone, Copy)]
pub enum Part {
    /// Nothing (a dead Scav, the airdrop crate).
    None,
    /// A door's leaf: how long it is from the hinge, how high and how
    /// thick. It turns about the door's hinge.
    Leaf { size: Vec3, color: Color },
    /// A lid or a drawer, in Bevy's axes as it stands shut: a box of
    /// `size` about `centre` (turned by `yaw` about the upright, with its
    /// prop's), turned by the thing's angle about the line through `pivot`
    /// along `axis` (a positive angle turns it the right hand's way about
    /// `axis`), and moved by `slide` as far as it is out.
    Moving { centre: Vec3, size: Vec3, yaw: f32, pivot: Vec3, axis: Vec3, slide: Vec3, color: Color },
}

/// A door's leaf: 0.9 m from its hinge to its edge, 2 m high and 4 cm
/// thick. The app's figures; the recorded doors' leaves are not read.
const LEAF: [f32; 3] = [0.9, 2.0, 0.04];

/// The two walls across the lane, in Bevy's axes (x to the right of the
/// start, z behind it), each running left to right and centred on the
/// lane: the door wall behind the start, high, with a doorway in its middle
/// for two doors (from one hinge to the other, across); and the knee-high
/// wall ahead of it. Their thickness, the door wall's height, and how far
/// each reaches to either side of the lane.
const DOOR_WALL_Z: f32 = 6.0;
pub const KNEE_WALL_Z: f32 = -5.6;
const WALL_THICK: f32 = 0.4;
/// (The door wall rose to the hall's 8 m with the hall, 0.76.0 to 0.99.0;
/// with the hall gone it is as before it, 2.5 m.)
const WALL_HIGH: f32 = 2.5;
const KNEE_HIGH: f32 = 0.6;
const WALL_HALF: f32 = 4.0;
const DOORWAY: [f32; 2] = [-0.9, 0.9];
/// How far the knee-high wall reaches to either side of the lane: further
/// than the door wall, for the tables of weapons along it on both sides of
/// the workbench (the user's range of 2026-10-26, drawn over the map: the
/// wall longer to the left, the arm of tables gone).
const KNEE_LEFT: f32 = -9.2;
const KNEE_RIGHT: f32 = 10.5;
/// A window in the door wall to the right of the doorway, open, to vault
/// through (the user, 2026-10-14, night: "add a small vaultable window by
/// the door"): from one side to the other across the lane, and its sill's
/// and its lintel's heights. Made to the vault's rules (`docs/spec/
/// vaulting.md`): a sill of 0.9 m is within the 0.25 to 1.1 m a vault
/// takes, the wall's 0.4 m within its 0.41 m of length, the 1 m opening
/// more than the 0.9 m of roof it asks over the sill, and the capsule a
/// vault shrinks to (1.0 to 1.73 m over the feet for this sill, 0.73 m
/// across) passes under the lintel and between the jambs.
const WINDOW: [f32; 2] = [2.0, 3.0];
const SILL: f32 = 0.9;
const LINTEL: f32 = 1.9;

/// The tables of weapons along the knee-high wall, to the right of the
/// airdrop crate, one for each class of weapon (the user, 2026-10-14:
/// "Let's make every weapon selectable. We can create a few more crates
/// lining the small wall with labels. Separated like (Rifles) (Pistols)
/// (Shotguns) (Snipers) (Custom)"; and the same night: "Lets split the
/// weapons and magazines. We can make a small table with the guns ontop,
/// and their magazines below, for each weapon type. The gun crates should
/// be long and short, like a gun case. Make them green. The magazines
/// should be in a black square box. Give them lids that open!"), each with
/// its label drawn over it. The three classes the user's five leave
/// without a place (submachine guns, machine guns, launchers) are the
/// app's. `main` fills them in this order: the gun cases are the range's
/// containers from `CONTAINERS`' last on, the magazine boxes after them.
pub const WEAPON_CRATES: [&str; 10] = ["RIFLES", "SMGS", "MACHINE GUNS", "SNIPERS", "SHOTGUNS", "PISTOLS", "LAUNCHERS", "CUSTOM", MEDICAL, GRENADES];
/// The last table holds no weapons: every medical item of the install in
/// its case, and every food and drink in its box under it (the user,
/// 2026-10-14, night: "A table of all of them", the case `MEDICAL` and the
/// box `PROVISIONS`). The app's.
pub const MEDICAL: &str = "MEDICAL";
pub const PROVISIONS: &str = "PROVISIONS";
/// And after it, every grenade of the install in its case, and every melee
/// weapon in its box (2026-10-17: the plan's step "A grenade and a knife by
/// their keys"; the app's, for there to be any to take: the user's starting
/// kit has a knife and no grenade).
pub const GRENADES: &str = "GRENADES";
pub const MELEE: &str = "MELEE";

/// Whether a table holds no weapons of a class, but what its two names say.
fn of_its_own(name: &str) -> bool {
    name == MEDICAL || name == GRENADES
}

/// A table's box's name: the class's, or its own.
fn box_name(name: &'static str) -> &'static str {
    match name {
        MEDICAL => PROVISIONS,
        GRENADES => MELEE,
        other => other,
    }
}
/// Barbed wire across the ground past the knee-high wall's left end, its
/// lowest and highest corners in Bevy's axes (the user, 2026-10-14, night:
/// "Falls + barbed wire"; moved from before the start on 2026-10-19: "Move
/// the barbed wire somewhere else", behind the start; and there by the
/// user's own range the same day). It is no solid: the character walks into
/// it, slowed and cut (health spec, "Barbed wire"). The game's coils have
/// their boxes in the maps' scenes; this one's shape and place are the
/// app's.
pub const BARBED_WIRE: [Vec3; 2] = [Vec3::new(KNEE_LEFT - 2.0, 0.0, -6.5), Vec3::new(KNEE_LEFT, 0.7, -5.5)];
/// The storage cabinets against the staircase's side, where the wire was
/// seen from the start (the user, 2026-10-19: "Where the barbed wire was,
/// against the staircase, make a few storage cabinets, like the drawer we
/// have in the back. They should be for Rigs, backpacks, helmets, armored
/// rigs and armor, masks and hats, headsets"; and then: "Add a plates box,
/// and eyewear dresser"): each a cabinet with a drawer as the row's, but the
/// plates' box, with a lid, and the eyewear's dresser, tall, with its
/// drawer near its top (`Furniture`); its back to the stairs and its front
/// to the lane, from the start's side on in the user's order, the plates
/// beside the armour; each label drawn over it. `main` fills them, after
/// the tables' magazine boxes. The app's.
pub const CABINETS: [&str; 8] = ["RIGS", "BACKPACKS", "HELMETS", "ARMOR", "PLATES", "MASKS AND HATS", "HEADSETS", "EYEWEAR"];
/// The plates' box and the eyewear's dresser among them.
pub const PLATES: &str = "PLATES";
pub const EYEWEAR: &str = "EYEWEAR";
/// What a place of the row against the staircase is.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Furniture {
    /// A cabinet with a drawer, as the row's (0.8 m high).
    Cabinet,
    /// A box with a lid hinged along its back, as the range's boxes.
    Box,
    /// A dresser, taller, its drawer near its top.
    Dresser,
}

impl Furniture {
    fn of(label: &str) -> Self {
        match label {
            PLATES => Self::Box,
            EYEWEAR => Self::Dresser,
            _ => Self::Cabinet,
        }
    }

    /// Its size: along the stairs, high, deep (from its back to its
    /// front); and the height of its drawer's middle.
    fn size(self) -> Vec3 {
        match self {
            Self::Cabinet => Vec3::new(0.8, 0.8, 0.5),
            Self::Box => Vec3::new(0.6, 0.6, 0.45),
            Self::Dresser => Vec3::new(1.0, 1.1, 0.5),
        }
    }

    fn drawer_height(self) -> f32 {
        match self {
            Self::Dresser => 0.95,
            _ => 0.64,
        }
    }

    /// What the prompt and the screen call it after its label.
    fn word(self) -> &'static str {
        match self {
            Self::Box => "box",
            Self::Dresser => "dresser",
            Self::Cabinet => "drawer",
        }
    }
}

/// Where the row's near end is along the stairs (Bevy's z: by the stairs'
/// foot), and the room between two of its places.
const CABINETS_NEAR: f32 = 3.6;
const CABINETS_GAP: f32 = 0.25;
/// The first cabinet by its place in `Carried::away`.
const CABINET_HOLDS: usize = MAGAZINE_BOX_HOLDS + WEAPON_CRATES.len();
/// The tables of weapons with a box of rounds beside their magazine box
/// (the user, 2026-10-24: "a separate box for their respective
/// ammunition"): the first of `WEAPON_CRATES`, all but the medical and the
/// grenades' tables; and the first box of rounds by its place in
/// `Carried::away`, after the cabinets. The app's.
pub const AMMO_TABLES: usize = 8;
const AMMO_BOX_HOLDS: usize = CABINET_HOLDS + CABINETS.len();
/// How far the two boxes under a table of weapons sit to each side of its
/// middle, along it.
const BOX_ASIDE: f32 = 0.23;
/// A ledge high enough to be hurt falling from (the game's falls hurt from
/// 3 m: health spec, "Falls"): a second flight of stairs leads on from the
/// platform, the same rise a step, to a ledge 6 m up. The app's.
const LEDGE: f32 = 6.0;
const LEDGE_STEPS: usize = 35;
/// Where the first table stands across the lane (its gun case half a metre
/// from the airdrop crate), and how far each next one is from the last.
const WEAPON_TABLE_FIRST: f32 = 1.5;
const WEAPON_TABLE_STEP: f32 = 1.2;
/// How many tables stand along the wall to the workbench's right; the rest
/// stand along it to its left, each from the bench out, every table's back
/// to the wall (the user's range of 2026-10-26: the arm turned towards the
/// start, "Rotate the last 4 weapon sets 90* over to the right to form an L
/// shape" of 2026-10-14, gone, its tables to the other side). The app's.
const ALONG_THE_WALL: usize = 4;
/// The workbench, where the airdrop crate and its clothing stood until
/// 0.96.0 (the user, 2026-10-20: "Let's add a workbench using the workbench
/// model, that when interacted with, opens the edit build menu for the
/// current gun in-hand"; of where: "Replace the old clothes crate with the
/// workbench. Move its contents to the corresponding crates"; of which:
/// the hideout's level 3; then, 0.96.1: "lets make the table smaller, just
/// the main metal table without all the extra stuff"): the hideout's own
/// main metal table of its level 3 bench (`10_workbench/level3/
/// highlight_transform/metal_table (2)` of the install's `bunker_2.unity`),
/// drawn by `workbench`, its back against the knee-high wall's face, its
/// middle where the crate's was. Its figures here are as `probe sceneprops
/// bunker_2.unity 10_workbench/level3 "highlight_transform/metal_table (2)"`
/// measures them, in the bench's own axes (the engine's, in metres): the
/// table's feet, its back, and its ends along the hideout's wall it stood
/// against; its middle along that wall stands at `BENCH_AT` across the
/// lanes (Bevy's x: the crate's middle, the app's).
const BENCH_FLOOR: f32 = -2.454;
const BENCH_WALL: f32 = 7.563946;
const BENCH_ENDS: [f32; 2] = [12.603228, 13.918263];
const BENCH_AT: f32 = 0.0;
/// The table as the range's solids, low and high corners in its own axes:
/// its top as a slab within its drawn corners, as measured, and a leg at
/// each corner, all metal (`ESTIMATE`, the slab's thickness and the legs'
/// side: the game's is its mesh collider, which the range's boxes are not;
/// until 0.128.0 one box about the whole table, which left nothing under it
/// to be seen or reached: the boxes of repair kits stand there now).
const BENCH_FRONT: f32 = 6.674887;
const BENCH_SLAB: f32 = 0.05;
const BENCH_LEG: f32 = 0.06;
fn bench_solids() -> Vec<([f32; 3], [f32; 3])> {
    let top = BENCH_TOP;
    let mut solids = vec![([BENCH_FRONT, top - BENCH_SLAB, BENCH_ENDS[0]], [BENCH_WALL, top, BENCH_ENDS[1]])];
    for x in [BENCH_FRONT, BENCH_WALL - BENCH_LEG] {
        for z in [BENCH_ENDS[0], BENCH_ENDS[1] - BENCH_LEG] {
            solids.push(([x, BENCH_FLOOR, z], [x + BENCH_LEG, top - BENCH_SLAB, z + BENCH_LEG]));
        }
    }
    solids
}
/// The boxes of repair kits under the workbench (the user, 2026-10-25: "I'd
/// like to add the armor and repair kits to the range, multiple copies
/// placed in boxes below the workbench"): the armour's kits and the
/// weapons', each the game's ammunition box as the tables' magazine boxes
/// are, side by side on the floor under the table's back half (clear of
/// its front stretcher), at a quarter of its length from either end; their
/// labels, and their places in `Carried::away`, after the tables' boxes of
/// rounds. The app's.
pub const REPAIR_BOXES: [&str; 2] = ["ARMOR REPAIR", "WEAPON REPAIR"];
pub const REPAIR_BOX_HOLDS: usize = AMMO_BOX_HOLDS + AMMO_TABLES;
/// Its top's height in its own axes (the yard's key lay there until 0.126.0).
const BENCH_TOP: f32 = -1.560655;
/// The game's two triggers that put the bench in a character's sight
/// (`HideoutInteractionTrigger` on `wokbench_lvl3_raycast_collider` and its
/// `wokbench_lvl3_area_trigger`, each a box collider; entered, the area is
/// in sight, `HideoutPlayerOwner.HideoutAreaInteraction`), as measured:
/// low and high corners in its own axes.
///
/// **Cut to the table** (the user, 2026-10-25: "can we make the workbench
/// hitbox smaller since it extends past the bench model", asked which:
/// "The interact zone"): the game's triggers are the whole level 3 bench's,
/// 3.8 m along its wall, and reached 0.61 m past the drawn table's one end
/// and 0.83 m past its other; each is cut along the wall to the table's ends
/// (`BENCH_ENDS`), its depth before the table kept. The app's, a departure.
const BENCH_TRIGGERS: [([f32; 3], [f32; 3]); 2] = [([5.746, -2.42, BENCH_ENDS[0]], [6.676, -0.44, 13.89]), ([6.116, -2.42, BENCH_ENDS[0]], [7.046, -0.44, BENCH_ENDS[1]])];
/// The game's triggers as measured, before the cut.
#[cfg(test)]
const BENCH_TRIGGERS_GAME: [([f32; 3], [f32; 3]); 2] = [([5.746, -2.42, 11.99], [6.676, -0.44, 13.89]), ([6.116, -2.42, 12.415], [7.046, -0.44, 14.745])];
/// The workbench's prop's name, for a level file.
const WORKBENCH_PROP: &str = "workbench";

/// Whether the workbench stands (the options' `WORKBENCH`, set once as the
/// run begins, before the range is made).
static WORKBENCH_ON: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

/// Sets whether the workbench stands (`WORKBENCH`).
pub fn set_workbench(on: bool) {
    WORKBENCH_ON.store(on, std::sync::atomic::Ordering::Relaxed);
}

/// The workbench as it stands, as made: its own axes (the engine's) to
/// Bevy's. Turned a quarter so its back is the knee-high wall's face, its
/// feet on the ground, its middle along that wall at `BENCH_AT`.
fn bench_made() -> Transform {
    let face = KNEE_WALL_Z + WALL_THICK / 2.0;
    let middle = (BENCH_ENDS[0] + BENCH_ENDS[1]) / 2.0;
    Transform::from_translation(Vec3::new(BENCH_AT + middle, -BENCH_FLOOR, face + BENCH_WALL)).with_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_2))
}

/// A point of the bench's own axes (the engine's) in Bevy's, as made.
fn bench_point(p: [f32; 3]) -> Vec3 {
    bench_made().transform_point(Vec3::new(p[0], p[1], -p[2]))
}

/// Where the bench's prop is made about: under its middle, at its wall.
fn bench_anchor() -> Vec3 {
    let at = bench_point([BENCH_WALL, BENCH_FLOOR, (BENCH_ENDS[0] + BENCH_ENDS[1]) / 2.0]);
    Vec3::new(at.x, 0.0, at.z)
}

/// A box of the bench's own axes (low and high corners) in Bevy's, as
/// made: its middle and size (upright, square to the lanes, as the bench
/// is turned a quarter).
fn bench_box(low: [f32; 3], high: [f32; 3]) -> (Vec3, Vec3) {
    let (a, b) = (bench_point(low), bench_point(high));
    ((a + b) / 2.0, (a - b).abs())
}

/// The workbench on the range: where its model's top stands (its own axes
/// to Bevy's), and the game's two triggers that put it in sight, as boxes
/// in Bevy's axes (low and high corners: upright, about each trigger where
/// the bench stands).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Workbench {
    pub place: Transform,
    pub triggers: [[Vec3; 2]; 2],
}

impl Workbench {
    /// Whether a character standing there (its feet, Bevy's axes), its
    /// capsule of that radius, is in one of the triggers: the bench is in
    /// its sight.
    pub fn in_sight(&self, feet: Vec3, radius: f32) -> bool {
        self.triggers.iter().any(|[low, high]| {
            let near = Vec3::new(feet.x.clamp(low.x, high.x), feet.y.clamp(low.y, high.y), feet.z.clamp(low.z, high.z));
            Vec2::new(feet.x - near.x, feet.z - near.z).length() <= radius && feet.y + 1.0 >= low.y && feet.y <= high.y
        })
    }
}
/// A table: how long along the wall, how high and how deep; its top's
/// thickness, and its legs' side. The app's.
const WEAPON_TABLE: Vec3 = Vec3::new(1.0, 0.85, 0.5);
const TABLE_SLAB: f32 = 0.04;
const TABLE_LEG: f32 = 0.05;
/// The gun case on a table, long and low (the game's own weapon box, the
/// `WeaponCrate` container whose grid is 5 by 2, is a long flat case), and
/// the magazine box under it, square (the game's `AmmoBox`, 3 by 3): their
/// sizes, and how far each lid opens. The app's shapes and angles
/// (`ESTIMATE`): the game's models of them are not read. The box's lid
/// opens no further than the table over it lets it.
const GUN_CASE: Vec3 = Vec3::new(0.95, 0.13, 0.34);
const MAGAZINE_BOX: Vec3 = Vec3::new(0.42, 0.42, 0.42);
const GUN_CASE_OPENS: f32 = 80.0;
const MAGAZINE_BOX_OPENS: f32 = 55.0;
/// The first gun case and the first magazine box by their places in
/// `Carried::away`.
const GUN_CASE_HOLDS: usize = 5;
const MAGAZINE_BOX_HOLDS: usize = GUN_CASE_HOLDS + WEAPON_CRATES.len();

#[derive(Clone, Resource)]
pub struct Range {
    pub solids: Vec<Solid>,
    /// The things the hands work and what they stand on, each moved and
    /// turned as one by a level file (`level`), and its solids and things
    /// among the range's.
    pub props: Vec<Prop>,
    /// The level file read at the start, if one was.
    level: Option<std::sync::Arc<crate::level::Level>>,
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
    /// The workbench, where it stands (`WORKBENCH`).
    pub workbench: Option<Workbench>,
}

/// A prop of the range: its name in a level file, where it stands (its
/// anchor's place, as made on the ground, and its turn), and which of the
/// range's solids and things are its.
#[derive(Clone)]
pub struct Prop {
    pub name: String,
    pub stand: crate::level::Stand,
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
/// Where the sampler was moved from its first place (the user's range of
/// 2026-10-26: from the lanes' right to the yard's left, behind the row of
/// things; `COURSE` keeps the strips' places as first laid).
const SAMPLER_SHIFT: [f32; 2] = [-49.0, 19.1];
const WIRE_PROP: &str = "barbed wire";
const SAMPLER_PROP: &str = "sampler";
const ROUNDS_PROP: &str = "rounds";

/// Where a prop stands by the level read at the start (as made, if none
/// was read or the file has it not).
fn stand_of(name: &str, anchor: Vec3) -> crate::level::Stand {
    crate::level::get().and_then(|level| level.props.get(name)).copied().unwrap_or(crate::level::Stand::made(anchor))
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
/// platform's right side on it since 0.76.0; the lower platform on its
/// left).
const STAIRS_LEFT: f32 = -19.4;
const STAIRS_FOOT: f32 = 4.4;
/// How far out the ring of blocks stands, all round the start: beyond the
/// hall since 0.76.0 (18 m before, two of it in the lanes).
const RING: f32 = 48.0;

/// The range is open to the sky (the user, 2026-10-21: "remove the walls
/// and windows from the range", the hall's walls and its roof): the hall
/// that stood about the lanes from 0.76.0 to 0.99.0 (its walls 8 m high
/// with their windows, its roof, its indoors and its lamps) is gone. Its
/// left wall stood on the platform's right side, which this still names;
/// the storage stands against the stairs' side (since the user's range of
/// 2026-10-26).
#[cfg(test)]
const HALL_LEFT: f32 = STAIRS_LEFT + 5.0;
/// Down the range past the firing line, across: the knee-high wall's ends.
const LANES: [f32; 2] = [KNEE_LEFT, KNEE_RIGHT];
/// Cover down the lanes, the user's: a block 2.9 m high turned 28.3
/// degrees, and a low wall 1.2 m high across the lanes (middle, size,
/// turn in degrees; Bevy's axes). The block 10 cm further from the left
/// wall than drawn, where it went 9 cm into it; the low wall square to the
/// lanes, drawn half a degree off.
/// (To the right of the lanes since the user's range of 2026-10-26, the
/// pair turned the other way: before it, to their left.)
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
const TABLE_TOP: f32 = 0.8;
const SHELF_TOP: f32 = 1.4;
/// The small white table by the yard door, where the door's key lies (the
/// user, 2026-10-25: "Move the key to a small white table next to the
/// door"): against the door wall's face on the lane's side, left of the
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
        Self::of(crate::level::get().map(|level| std::sync::Arc::new(level.clone())))
    }
}

impl Range {
    /// The range: its own boxes, or a level file's in their place, and its
    /// props where the file stands them (`level`).
    pub fn of(level: Option<std::sync::Arc<crate::level::Level>>) -> Self {
        let mut solids = Vec::new();
        let block = |centre: Vec3, size: Vec3, color: Color| Solid { name: "block", centre, size, yaw: 0.0, color, surface: trp_sim::steps::CONCRETE, overhead: false };
        match &level {
            // A level file's boxes, each as the file has it.
            Some(level) => {
                for found in &level.boxes {
                    let name: &'static str = Box::leak(found.name.clone().into_boxed_str());
                    let overhead = crate::level::is_roof(&found.name);
                    solids.push(Solid { name, centre: found.centre, size: found.size, yaw: found.yaw, color: found.color, surface: found.surface, overhead });
                }
            }
            None => Self::own_boxes(&mut solids),
        }
        let stand_of = |name: &str, anchor: Vec3| level.as_ref().and_then(|level| level.props.get(name)).copied().unwrap_or(crate::level::Stand::made(anchor));
        let mut props: Vec<Prop> = Vec::new();
        // Each prop: what it is made of is made, then put where it stands.
        let end = |name: &str, anchor: Vec3, at: (usize, usize), solids: &mut Vec<Solid>, things: &mut Vec<Placed>, props: &mut Vec<Prop>| {
            let stand = stand_of(name, anchor);
            if stand != crate::level::Stand::made(anchor) {
                for solid in &mut solids[at.0..] {
                    solid.centre = stand.point(anchor, solid.centre);
                    solid.yaw += stand.yaw;
                }
                for placed in &mut things[at.1..] {
                    placed.stand(anchor, stand);
                }
            }
            props.push(Prop { name: name.to_owned(), stand, solids: at.0..solids.len(), things: at.1..things.len() });
        };
        // The things for the hands. Each container's or trunk's body is a
        // solid; what moves is drawn apart (`Part`).
        let mut things = Vec::new();
        let body = |solids: &mut Vec<Solid>, at: [f32; 2], size: Vec3, color: Color| {
            solids.push(block(Vec3::new(at[0], size.y / 2.0, at[1]), size, color));
            *solids.last().unwrap()
        };
        let container = |name, thing, solid: Solid, part, holds| Placed {
            name,
            thing,
            part,
            hit: solid.block(),
            place: in_sim(solid.centre),
            holds: Some(holds),
            leaf: None,
        };
        // A lid on a body of `size` standing at `at`, hinged along the
        // body's far edge from the door wall, turning about `axis`.
        let lid = |at: [f32; 2], size: Vec3, axis: Vec3, color| Part::Moving {
            centre: Vec3::new(at[0], size.y + 0.02, at[1]),
            size: Vec3::new(size.x, 0.04, size.z),
            yaw: 0.0,
            pivot: Vec3::new(at[0], size.y + 0.02, at[1] + size.z / 2.0),
            axis,
            slide: Vec3::ZERO,
            color,
        };
        // The workbench where the airdrop crate stood (`BENCH_SOLIDS`): its
        // table's box, not drawn (its model is, `workbench`).
        let workbench_on = WORKBENCH_ON.load(std::sync::atomic::Ordering::Relaxed);
        if workbench_on {
            let at = (solids.len(), things.len());
            for (low, high) in bench_solids() {
                let (centre, size) = bench_box(low, high);
                solids.push(Solid { name: "workbench", centre, size, yaw: 0.0, color: UNSEEN, surface: METAL, overhead: false });
            }
            // The boxes of repair kits under it, opened as the tables'
            // magazine boxes (the gestures recorded for a lid hinged up),
            // their fronts to the lanes.
            let box_olive = Color::srgb(0.22, 0.23, 0.18);
            let depth = BENCH_WALL - BENCH_LEG - MAGAZINE_BOX.z / 2.0 - 0.01;
            let quarter = (BENCH_ENDS[1] - BENCH_ENDS[0]) / 4.0;
            for (index, name) in REPAIR_BOXES.into_iter().enumerate() {
                let along = if index == 0 { BENCH_ENDS[0] + quarter } else { BENCH_ENDS[1] - quarter };
                let floor = bench_point([depth, BENCH_FLOOR, along]);
                let body = block(floor + Vec3::Y * MAGAZINE_BOX.y / 2.0, MAGAZINE_BOX, box_olive);
                solids.push(Solid { surface: PLASTIC, ..body });
                let mut opens = Thing::container(Some(Interaction::ContainerHingeUpOpen), Some(Interaction::ContainerHingeUpClose), 0.0, -30.0);
                opens.open_angle = -MAGAZINE_BOX_OPENS;
                let lid = lid_of(floor + Vec3::Y * MAGAZINE_BOX.y, MAGAZINE_BOX, Vec3::Z, box_olive.lighter(0.04));
                let label: &'static str = Box::leak(format!("{} box", name.to_lowercase()).into_boxed_str());
                things.push(container(label, opens, body, lid, REPAIR_BOX_HOLDS + index));
            }
            end(WORKBENCH_PROP, bench_anchor(), at, &mut solids, &mut things, &mut props);
        }
        // Behind the door wall, from left to right. A technical supply crate,
        // its lid at waist height (recorded: gestures 29 and 30, the lid from
        // 0 to 4.78 degrees).
        let wood = Color::srgb(0.47, 0.36, 0.22);
        let green = Color::srgb(0.28, 0.34, 0.26);
        let hinged_up = || Thing::container(Some(Interaction::ContainerHingeUpOpen), Some(Interaction::ContainerHingeUpClose), 0.0, -30.0);
        let at = (solids.len(), things.len());
        let supply_size = Vec3::new(0.55, 0.84, 0.9);
        let supply = body(&mut solids, SUPPLY_CRATE, supply_size, green);
        let mut tech = hinged_up();
        tech.open_angle = 4.78;
        things.push(container("technical supply crate", tech, supply, lid(SUPPLY_CRATE, supply_size, Vec3::X, green.darker(0.05)), 2));
        end("supply crate", Vec3::new(SUPPLY_CRATE[0], 0.0, SUPPLY_CRATE[1]), at, &mut solids, &mut things, &mut props);
        // A drawer in a cabinet, its front to the door wall (recorded:
        // pulled open with 25 and pushed shut with a door's 20; it slides).
        let at = (solids.len(), things.len());
        let cabinet = body(&mut solids, DRAWER, Vec3::new(0.8, 0.8, 0.5), Color::srgb(0.40, 0.38, 0.33));
        let front = Vec3::new(DRAWER[0], 0.64, DRAWER[1] - 0.03);
        let drawer_front = Part::Moving {
            centre: front,
            size: Vec3::new(0.7, 0.15, 0.5),
            yaw: 0.0,
            pivot: front,
            axis: Vec3::Y,
            // ESTIMATE: how far the drawer comes out (its prefab's open
            // place is not read).
            slide: Vec3::new(0.0, 0.0, -0.35),
            color: Color::srgb(0.46, 0.43, 0.37),
        };
        let drawer = Thing::container(Some(Interaction::ContainerOpenPull), Some(Interaction::DoorPushForward), 0.0, 0.0);
        things.push(container("drawer", drawer, cabinet, drawer_front, 4));
        end("drawer", Vec3::new(DRAWER[0], 0.0, DRAWER[1]), at, &mut solids, &mut things, &mut props);
        // The storage against the staircase's side, its fronts to the lane
        // (`CABINETS`): cabinets and a dresser with a drawer as the row's
        // (pulled open with 25, pushed shut with a door's 20; it slides), and
        // a box with a lid hinged along its back by the stairs (the gestures
        // recorded for a lid hinged up, 29 and 30), lifted towards them.
        for (index, name) in CABINETS.into_iter().enumerate() {
            let begun = (solids.len(), things.len());
            let (at, furniture) = (cabinet_at(index), Furniture::of(name));
            let size = furniture.size();
            let anchor = Vec3::new(at[0], 0.0, at[1]);
            let (body_color, front_color) = match furniture {
                Furniture::Box => (Color::srgb(0.27, 0.31, 0.21), Color::srgb(0.25, 0.29, 0.19)),
                _ => (Color::srgb(0.40, 0.38, 0.33), Color::srgb(0.46, 0.43, 0.37)),
            };
            let body = Solid { surface: METAL, ..block(Vec3::new(at[0], size.y / 2.0, at[1]), Vec3::new(size.z, size.y, size.x), body_color) };
            solids.push(body);
            let named: &'static str = Box::leak(format!("{} {}", name.to_lowercase(), furniture.word()).into_boxed_str());
            if furniture == Furniture::Box {
                let mut opens = hinged_up();
                opens.open_angle = -GUN_CASE_OPENS;
                let lid = lid_of(Vec3::new(at[0], size.y, at[1]), size, Vec3::X, front_color);
                things.push(container(named, opens, body, lid, CABINET_HOLDS + index));
                end(named, anchor, begun, &mut solids, &mut things, &mut props);
                continue;
            }
            let front = Vec3::new(at[0] + 0.03, furniture.drawer_height(), at[1]);
            let drawer_front = Part::Moving {
                centre: front,
                size: Vec3::new(size.z, 0.15, size.x - 0.1),
                yaw: 0.0,
                pivot: front,
                axis: Vec3::Y,
                // ESTIMATE: as the row's drawer's.
                slide: Vec3::new(0.35, 0.0, 0.0),
                color: front_color,
            };
            let drawer = Thing::container(Some(Interaction::ContainerOpenPull), Some(Interaction::DoorPushForward), 0.0, 0.0);
            things.push(container(named, drawer, body, drawer_front, CABINET_HOLDS + index));
            end(named, anchor, begun, &mut solids, &mut things, &mut props);
        }
        // A dead Scav lying along the row (recorded: opened with a door's
        // gesture, 21, and shut with the default, 28; nothing moves). It is
        // 40 cm high so that it is in reach standing, as the recorded one
        // was (the game's reach is 1.3 m from the eye).
        let at = (solids.len(), things.len());
        let scav = body(&mut solids, SCAV, Vec3::new(1.7, 0.4, 0.5), Color::srgb(0.30, 0.31, 0.27));
        let dead = Thing::container(Some(Interaction::DoorPullBackward), Some(Interaction::ContainerCloseDefault), 0.0, 0.0);
        things.push(container("dead Scav", dead, scav, Part::None, 3));
        end("scav", Vec3::new(SCAV[0], 0.0, SCAV[1]), at, &mut solids, &mut things, &mut props);
        // A wooden crate on the floor, its lid hinged along its far edge
        // (recorded on the floor: gestures 29 and 30, the lid from 0 to -30
        // degrees).
        let at = (solids.len(), things.len());
        let crate_size = Vec3::new(1.0, 0.44, 0.6);
        let crate_body = body(&mut solids, WOODEN_CRATE, crate_size, wood);
        made_of(&mut solids, WOOD);
        things.push(container("wooden crate", hinged_up(), crate_body, lid(WOODEN_CRATE, crate_size, Vec3::NEG_X, wood.darker(0.05)), 1));
        end("wooden crate", Vec3::new(WOODEN_CRATE[0], 0.0, WOODEN_CRATE[1]), at, &mut solids, &mut things, &mut props);
        // The tables of weapons, against the knee-high wall to the airdrop
        // crate's right: on each a green gun case with the class's weapons,
        // under it a black box with what loads them, its front with the
        // table's. Each opens as the range's crates do (the gestures
        // recorded for a lid hinged up, 29 and 30), its lid hinged along
        // its back edge, by the wall, and lifted towards it.
        let (case_green, box_black) = (Color::srgb(0.24, 0.33, 0.19), Color::srgb(0.05, 0.05, 0.06));
        for (index, name) in WEAPON_CRATES.into_iter().enumerate() {
            let begun = (solids.len(), things.len());
            // The medical table's case is light, and its two hold no
            // weapons: their names say what they do hold.
            let medical = name == MEDICAL;
            let case_green = if medical { Color::srgb(0.74, 0.73, 0.68) } else { case_green };
            let (case_word, box_word) = if of_its_own(name) { ("case", "box") } else { ("weapon case", "magazine box") };
            let box_name = box_name(name);
            let ([x, z], front) = weapon_table(index);
            let (along, middle) = (Vec3::Y.cross(front), Vec3::new(x, 0.0, z));
            // A box's size in the range's axes, from its length along the
            // table, its height and its depth from back to front.
            let sized = |size: Vec3| along.abs() * size.x + Vec3::Y * size.y + front.abs() * size.z;
            // The table: its top, and a leg at each corner.
            solids.push(block(middle + Vec3::Y * (WEAPON_TABLE.y - TABLE_SLAB / 2.0), sized(Vec3::new(WEAPON_TABLE.x, TABLE_SLAB, WEAPON_TABLE.z)), wood));
            made_of(&mut solids, WOOD);
            for (across, out) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                let leg = middle + along * across * (WEAPON_TABLE.x - TABLE_LEG) / 2.0 + front * out * (WEAPON_TABLE.z - TABLE_LEG) / 2.0 + Vec3::Y * (WEAPON_TABLE.y - TABLE_SLAB) / 2.0;
                solids.push(block(leg, sized(Vec3::new(TABLE_LEG, WEAPON_TABLE.y - TABLE_SLAB, TABLE_LEG)), wood));
                made_of(&mut solids, WOOD);
            }
            // (Each thing's name is the prompt's, made once for the run.)
            let named = |of: &str, what: &str| -> &'static str { Box::leak(format!("{} {what}", of.to_lowercase()).into_boxed_str()) };
            let case = Solid { surface: PLASTIC, ..block(middle + Vec3::Y * (WEAPON_TABLE.y + GUN_CASE.y / 2.0), sized(GUN_CASE), case_green) };
            solids.push(case);
            let mut opens = hinged_up();
            opens.open_angle = -GUN_CASE_OPENS;
            let lid = lid_of(middle + Vec3::Y * (WEAPON_TABLE.y + GUN_CASE.y), GUN_CASE, front, case_green.darker(0.04));
            things.push(container(named(name, case_word), opens, case, lid, GUN_CASE_HOLDS + index));
            // The box under it, its front with the table's; a table of
            // weapons has its box of rounds beside it (the magazines' on
            // the right as one faces the table, the rounds' on the left).
            let rounds_too = index < AMMO_TABLES;
            let below = middle + front * (WEAPON_TABLE.z - MAGAZINE_BOX.z) / 2.0;
            let magazines_at = if rounds_too { below + along * BOX_ASIDE } else { below };
            let magazines = block(magazines_at + Vec3::Y * MAGAZINE_BOX.y / 2.0, sized(MAGAZINE_BOX), box_black);
            solids.push(magazines);
            let mut opens = hinged_up();
            opens.open_angle = -MAGAZINE_BOX_OPENS;
            let lid = lid_of(magazines_at + Vec3::Y * MAGAZINE_BOX.y, MAGAZINE_BOX, front, box_black.lighter(0.04));
            things.push(container(named(box_name, box_word), opens, magazines, lid, MAGAZINE_BOX_HOLDS + index));
            if rounds_too {
                // An olive box of rounds, the size of the magazines'.
                let rounds_olive = Color::srgb(0.20, 0.22, 0.13);
                let rounds_at = below - along * BOX_ASIDE;
                let rounds = block(rounds_at + Vec3::Y * MAGAZINE_BOX.y / 2.0, sized(MAGAZINE_BOX), rounds_olive);
                solids.push(rounds);
                let mut opens = hinged_up();
                opens.open_angle = -MAGAZINE_BOX_OPENS;
                let lid = lid_of(rounds_at + Vec3::Y * MAGAZINE_BOX.y, MAGAZINE_BOX, front, rounds_olive.lighter(0.04));
                things.push(container(named(name, "ammunition box"), opens, rounds, lid, AMMO_BOX_HOLDS + index));
            }
            end(&weapon_table_prop(index), middle, begun, &mut solids, &mut things, &mut props);
        }
        // A table and a shelf with things lying on them (`lying`), and
        // nothing more; and the rounds on the floor, which stand on nothing.
        let at = (solids.len(), things.len());
        body(&mut solids, TABLE, Vec3::new(1.0, TABLE_TOP, 0.6), wood);
        made_of(&mut solids, WOOD);
        end("table", Vec3::new(TABLE[0], 0.0, TABLE[1]), at, &mut solids, &mut things, &mut props);
        let at = (solids.len(), things.len());
        body(&mut solids, HIGH_SHELF, Vec3::new(0.9, SHELF_TOP, 0.35), wood);
        made_of(&mut solids, WOOD);
        end("high shelf", Vec3::new(HIGH_SHELF[0], 0.0, HIGH_SHELF[1]), at, &mut solids, &mut things, &mut props);
        let at = (solids.len(), things.len());
        end(ROUNDS_PROP, Vec3::new(ROUNDS[0], 0.0, ROUNDS[1]), at, &mut solids, &mut things, &mut props);
        // The small white table by the doorway, the yard door's key on it
        // (`key_lies`): its top, and a leg at each corner.
        let at = (solids.len(), things.len());
        let white = Color::srgb(0.9, 0.9, 0.88);
        let ([x, z], size) = (KEY_TABLE, KEY_TABLE_SIZE);
        solids.push(Solid { surface: WOOD, ..block(Vec3::new(x, size.y - TABLE_SLAB / 2.0, z), Vec3::new(size.x, TABLE_SLAB, size.z), white) });
        for (dx, dz) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
            let leg = Vec3::new(x + dx * (size.x - TABLE_LEG) / 2.0, (size.y - TABLE_SLAB) / 2.0, z + dz * (size.z - TABLE_LEG) / 2.0);
            solids.push(Solid { surface: WOOD, ..block(leg, Vec3::new(TABLE_LEG, size.y - TABLE_SLAB, TABLE_LEG), white) });
        }
        end(KEY_TABLE_PROP, Vec3::new(x, 0.0, z), at, &mut solids, &mut things, &mut props);
        // A car at the row's right end, its back to the row: the trunk opens
        // with the default gestures, its lid from 0 to 60 degrees (recorded).
        let at = (solids.len(), things.len());
        let red = Color::srgb(0.45, 0.16, 0.14);
        let car = body(&mut solids, CAR, Vec3::new(4.2, 1.0, 1.8), red);
        made_of(&mut solids, METAL);
        solids.push(block(Vec3::new(CAR[0] + 0.1, 1.225, CAR[1]), Vec3::new(2.0, 0.45, 1.6), red.darker(0.1)));
        made_of(&mut solids, METAL);
        let trunk_lid = Part::Moving {
            centre: Vec3::new(CAR[0] - 1.6, 1.02, CAR[1]),
            size: Vec3::new(1.0, 0.04, 1.6),
            yaw: 0.0,
            pivot: Vec3::new(CAR[0] - 1.1, 1.02, CAR[1]),
            axis: Vec3::NEG_Z,
            slide: Vec3::ZERO,
            color: red.darker(0.05),
        };
        let trunk_hit = Solid { surface: METAL, ..block(Vec3::new(CAR[0] - 1.6, 0.52, CAR[1]), Vec3::new(1.0, 1.04, 1.8), red) };
        things.push(Placed {
            name: "trunk",
            thing: Thing::trunk(None, None, 0.0, 60.0),
            part: trunk_lid,
            hit: trunk_hit.block(),
            place: in_sim(car.centre),
            holds: None,
            leaf: None,
        });
        end("car", Vec3::new(CAR[0], 0.0, CAR[1]), at, &mut solids, &mut things, &mut props);
        // Two doors in the doorway, hinged at its two ends and meeting in
        // its middle, each turning 110 degrees towards the start (recorded:
        // the doors' angles were 0 and 110). Their leaves lie along the
        // wall's face on the start's side. (In the simulation's axes the
        // start's side is +z, and a door turns towards it about straight
        // down from the left hinge and straight up from the right.)
        let at = (solids.len(), things.len());
        let face = DOOR_WALL_Z - WALL_THICK / 2.0 + LEAF[2] / 2.0;
        let doors = [
            (DOORWAY[0], [1.0, 0.0, 0.0], [0.0, -1.0, 0.0]),
            (DOORWAY[1], [-1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
        ];
        for (side, forward, axis) in doors {
            let hinge = Hinge { at: in_sim(Vec3::new(side, 0.0, face)), axis, forward };
            let thing = Thing::door(hinge, 0.0, 110.0);
            let leaf = leaf_block(&thing);
            things.push(Placed {
                name: "door",
                part: Part::Leaf { size: Vec3::from(LEAF), color: Color::srgb(0.36, 0.30, 0.24) },
                thing,
                hit: leaf,
                place: hinge.at,
                holds: None,
                leaf: None,
            });
        }
        end("doors", Vec3::new((DOORWAY[0] + DOORWAY[1]) / 2.0, 0.0, DOOR_WALL_Z), at, &mut solids, &mut things, &mut props);
        // The barbed wire and the sampler, which have no solids.
        for (name, anchor) in [(WIRE_PROP, wire_anchor()), (SAMPLER_PROP, sampler_anchor())] {
            let at = (solids.len(), things.len());
            end(name, anchor, at, &mut solids, &mut things, &mut props);
        }

        // The character's collision: the solids, then the doors' leaves as
        // they stand.
        let mut blocks: Vec<Block> = solids.iter().filter(|solid| !solid.overhead).map(Solid::block).collect();
        for placed in things.iter_mut().filter(|placed| matches!(placed.part, Part::Leaf { .. })) {
            placed.hit = leaf_block(&placed.thing);
            blocks.push(placed.hit);
            placed.leaf = Some(blocks.len() - 1);
        }
        // The workbench where its prop stands: its model's top and its
        // triggers.
        let workbench = workbench_on.then(|| {
            let anchor = bench_anchor();
            let stand = props.iter().find(|prop| prop.name == WORKBENCH_PROP).map_or(crate::level::Stand::made(anchor), |prop| prop.stand);
            let made = bench_made();
            let place = Transform::from_translation(stand.point(anchor, made.translation)).with_rotation(Quat::from_rotation_y(stand.yaw) * made.rotation);
            let triggers = BENCH_TRIGGERS.map(|(low, high)| {
                let corners: Vec<Vec3> = (0..8).map(|k| stand.point(anchor, bench_point([if k & 1 == 0 { low[0] } else { high[0] }, if k & 2 == 0 { low[1] } else { high[1] }, if k & 4 == 0 { low[2] } else { high[2] }]))).collect();
                [corners.iter().copied().fold(Vec3::INFINITY, Vec3::min), corners.iter().copied().fold(Vec3::NEG_INFINITY, Vec3::max)]
            });
            Workbench { place, triggers }
        });
        // The game's figure, until `main` has read it from the install.
        Self { solids, props, level, radius: 0.366, site: Site { ground: 0.0, blocks }, things, thing_settings: None, workbench }
    }

    /// The range's own boxes, without a level file: the ring of blocks,
    /// the walls, the cover in the lanes, the stairs, the platforms and the ledge.
    fn own_boxes(solids: &mut Vec<Solid>) {
        let block = |name: &'static str, centre: Vec3, size: Vec3, color: Color| Solid { name, centre, size, yaw: 0.0, color, surface: trp_sim::steps::CONCRETE, overhead: false };
        // A ring of blocks all round, to judge turning against. The eighth
        // stands 15 degrees on from its place, out of the second flight's
        // way to the ledge.
        for i in 0..12 {
            let angle = (i as f32 * 30.0 + if i == 7 { 15.0 } else { 0.0 }).to_radians();
            solids.push(block(
                "ring block",
                Vec3::new(angle.sin() * RING, 0.5, angle.cos() * RING),
                Vec3::ONE,
                Color::hsl(i as f32 * 30.0, 0.5, 0.45),
            ));
        }
        // The two walls across the lane: behind the start the door wall,
        // with its doorway and a lintel over it and a window to its right;
        // ahead the knee-high one, to vault and to lie at. (A block a metre
        // high stood off to the right, to climb, until the user had it
        // taken away on 2026-10-14: the ring's blocks are as high.)
        // Light concrete greys for the walls, the stairs and the platforms
        // (the user, 2026-10-19: "I want to make the whole picture
        // brighter"; darker greys and greens before): the faces turned from
        // the sun are what read darkest under the camera's exposure.
        let grey = HALL_GREY;
        let span = |name: &'static str, z: f32, left: f32, right: f32, low: f32, high: f32| {
            block(name, Vec3::new((left + right) / 2.0, (low + high) / 2.0, z), Vec3::new(right - left, high - low, WALL_THICK), grey)
        };
        solids.push(span("door wall", DOOR_WALL_Z, -WALL_HALF, DOORWAY[0], 0.0, WALL_HIGH));
        solids.push(span("door wall", DOOR_WALL_Z, DOORWAY[1], WINDOW[0], 0.0, WALL_HIGH));
        solids.push(span("window sill", DOOR_WALL_Z, WINDOW[0], WINDOW[1], 0.0, SILL));
        solids.push(span("window lintel", DOOR_WALL_Z, WINDOW[0], WINDOW[1], LINTEL, WALL_HIGH));
        solids.push(span("door wall", DOOR_WALL_Z, WINDOW[1], WALL_HALF, 0.0, WALL_HIGH));
        solids.push(span("doorway lintel", DOOR_WALL_Z, DOORWAY[0], DOORWAY[1], LEAF[1] + 0.05, WALL_HIGH));
        solids.push(span("knee-high wall", KNEE_WALL_Z, KNEE_LEFT, KNEE_RIGHT, 0.0, KNEE_HIGH));
        // Cover down the lanes (the hall about them gone since 0.100.0).
        for (name, centre, size, turn) in COVER {
            solids.push(Solid { yaw: turn.to_radians(), ..block(name, centre, size, HALL_GREY) });
        }
        // Somewhere to fall from, to the left of the start: stairs going the
        // lane's way up to a platform, and a lower platform on its right,
        // toward the knee-high wall (the stairs on the platform's left half
        // and the lower platform on its right since the user's range of
        // 2026-10-26; before, the other way round).
        let floor = |name: &'static str, low: Vec3, high: Vec3, color: Color| block(name, (low + high) / 2.0, high - low, color);
        let (left, stair_right, tread) = (STAIRS_LEFT, STAIRS_LEFT + 2.5, TREAD);
        for i in 0..STAIRS {
            let top = PLATFORM * (i + 1) as f32 / (STAIRS + 1) as f32;
            let near = STAIRS_FOOT - (i as f32) * tread;
            solids.push(floor(
                "stairs step",
                Vec3::new(left, 0.0, near - tread),
                Vec3::new(stair_right, top, near),
                Color::srgb(0.56, 0.56, 0.56 + 0.02 * (i % 2) as f32),
            ));
        }
        let far = STAIRS_FOOT - (STAIRS as f32) * tread;
        solids.push(floor("platform", Vec3::new(left, 0.0, far - 5.0), Vec3::new(left + 5.0, PLATFORM, far), Color::srgb(0.52, 0.52, 0.54)));
        solids.push(floor("lower platform", Vec3::new(left + 5.0, 0.0, far - 5.0), Vec3::new(left + 8.0, SHELF, far), Color::srgb(0.52, 0.54, 0.52)));
        // From the platform's far edge, on the half the first flight comes up
        // on, a second flight on the way the first goes, to the ledge beyond:
        // straight on from the start's side, no turning back (the user,
        // 2026-10-14).
        let rise = (LEDGE - PLATFORM) / LEDGE_STEPS as f32;
        let (flight_left, flight_right, edge) = (left, stair_right, far - 5.0);
        for i in 0..LEDGE_STEPS {
            let top = PLATFORM + rise * (i + 1) as f32;
            let near = edge - i as f32 * tread;
            solids.push(floor("flight step", Vec3::new(flight_left, 0.0, near - tread), Vec3::new(flight_right, top, near), Color::srgb(0.56, 0.56, 0.56 + 0.02 * (i % 2) as f32)));
        }
        let ledge = edge - LEDGE_STEPS as f32 * tread;
        solids.push(floor("ledge", Vec3::new(flight_left, 0.0, ledge - 3.0), Vec3::new(flight_right, LEDGE, ledge), Color::srgb(0.50, 0.50, 0.52)));
    }
}

impl Placed {
    /// Puts the thing, made about `anchor`, where its prop stands: its
    /// place, what a line of sight meets of it, its hinge and its moving
    /// part.
    fn stand(&mut self, anchor: Vec3, stand: crate::level::Stand) {
        let point = |p: [f32; 3]| in_sim(stand.point(anchor, in_bevy(p)));
        let way = |v: [f32; 3]| in_sim(stand.way(in_bevy(v)));
        self.place = point(self.place);
        self.hit = moved_block(self.hit, anchor, stand);
        if let Some(hinge) = self.thing.hinge.as_mut() {
            (hinge.at, hinge.axis, hinge.forward) = (point(hinge.at), way(hinge.axis), way(hinge.forward));
        }
        if let Part::Moving { centre, yaw, pivot, axis, slide, .. } = &mut self.part {
            (*centre, *pivot) = (stand.point(anchor, *centre), stand.point(anchor, *pivot));
            (*axis, *slide) = (stand.way(*axis), stand.way(*slide));
            *yaw += stand.yaw;
        }
    }
}

/// A box of the simulation's (z ahead), made about `anchor` (Bevy's axes),
/// where its prop stands: its middle moved, its turn the prop's more (a
/// turn of the block's is the other way about Bevy's up: `Range::boxes`).
fn moved_block(block: Block, anchor: Vec3, stand: crate::level::Stand) -> Block {
    let middle: [f32; 3] = std::array::from_fn(|k| (block.low[k] + block.high[k]) / 2.0);
    let half: [f32; 3] = std::array::from_fn(|k| (block.high[k] - block.low[k]) / 2.0);
    let moved = in_sim(stand.point(anchor, in_bevy(middle)));
    Block { low: std::array::from_fn(|k| moved[k] - half[k]), high: std::array::from_fn(|k| moved[k] + half[k]), turn: block.turn - stand.yaw }
}

/// A weapon table's prop's name: its label in small letters, and `table`.
fn weapon_table_prop(index: usize) -> String {
    format!("{} table", WEAPON_CRATES[index].to_lowercase())
}

/// A storage place's prop's name: its label in small letters, and what it
/// is (`rigs cabinet`, `plates box`).
fn cabinet_prop(index: usize) -> String {
    format!("{} {}", CABINETS[index].to_lowercase(), Furniture::of(CABINETS[index]).word())
}

/// The starts every range has (with a table's and a storage place's, by
/// their labels: `rifles`, `masks-and-hats`).
const STARTS: [&str; 23] = [
    "platform", "ledge", "wire", "shelf", "stairs", "doors", "yard", "open", "workbench", "window", "car", "wooden-crate", "supply-crate", "scav", "drawer", "table", "key-table", "high-shelf", "floor", "lanes", "dummy", "dummy-side", "surfaces",
];

/// Every start's name.
pub fn start_names() -> Vec<String> {
    let labels = WEAPON_CRATES.iter().chain(&CABINETS).map(|label| label.to_lowercase().replace(' ', "-"));
    STARTS.iter().map(|name| (*name).to_owned()).chain(labels).collect()
}

/// A start as the range makes it: where the feet are on the ground (Bevy's
/// axes), its view, and the prop it goes with and where that is made about.
fn made_start(name: &str) -> Option<(Vec3, [f32; 2], Option<(String, Vec3)>)> {
    let far = STAIRS_FOOT - STAIRS as f32 * TREAD;
    // Where to stand, in Bevy's axes, from where a thing is: the row's
    // things are stood before on the door wall's side.
    let by = |at: [f32; 2], across: f32, along: f32| [at[0] + across, at[1] + along];
    let doorway = (DOORWAY[0] + DOORWAY[1]) / 2.0;
    let of = |prop: &str, at: [f32; 2]| Some((prop.to_owned(), Vec3::new(at[0], 0.0, at[1])));
    let doors = of("doors", [doorway, DOOR_WALL_Z]);
    let ([x, z], owner): ([f32; 2], Option<(String, Vec3)>) = match name {
        "platform" => ([STAIRS_LEFT + 2.5, far - 3.0], None),
        // On the ledge 6 m up, by its far edge.
        "ledge" => ([STAIRS_LEFT + 1.25, far - 5.0 - LEDGE_STEPS as f32 * TREAD - 2.4], None),
        // Before the barbed wire, on the start's side.
        "wire" => ([(BARBED_WIRE[0].x + BARBED_WIRE[1].x) / 2.0, BARBED_WIRE[1].z + 1.0], Some((WIRE_PROP.to_owned(), wire_anchor()))),
        "shelf" => ([STAIRS_LEFT + 6.5, far - 3.0], None),
        "stairs" => ([STAIRS_LEFT + 1.25, STAIRS_FOOT + 2.0], None),
        "doors" => ([doorway, DOOR_WALL_Z - 1.3], doors),
        "yard" => ([doorway, DOOR_WALL_Z + 0.75], doors),
        // Out in the yard past its row of things, facing along it: 300 m of
        // floor clear ahead (a long sprint; the test below).
        "open" => ([doorway, ROW + 8.0], None),
        // Before the workbench's table, in the game's trigger, facing the
        // knee-high wall (its own axes: 6.1 m from its middle's line, at
        // the table's middle along it).
        "workbench" => {
            let at = bench_point([6.1, BENCH_FLOOR, (BENCH_ENDS[0] + BENCH_ENDS[1]) / 2.0]);
            ([at.x, at.z], Some((WORKBENCH_PROP.to_owned(), bench_anchor())))
        }
        // Before the window on the lane's side, to vault through it.
        "window" => ([(WINDOW[0] + WINDOW[1]) / 2.0, DOOR_WALL_Z - 1.3], None),
        "car" => (by(CAR, -3.0, 0.0), of("car", CAR)),
        "wooden-crate" => (by(WOODEN_CRATE, 0.0, -0.9), of("wooden crate", WOODEN_CRATE)),
        "supply-crate" => (by(SUPPLY_CRATE, 0.0, -1.075), of("supply crate", SUPPLY_CRATE)),
        "scav" => (by(SCAV, 0.0, -0.62), of("scav", SCAV)),
        "drawer" => (by(DRAWER, 0.0, -1.05), of("drawer", DRAWER)),
        "table" => (by(TABLE, 0.0, -1.0), of("table", TABLE)),
        "key-table" => (by(KEY_TABLE, 0.0, -0.9), of(KEY_TABLE_PROP, KEY_TABLE)),
        "high-shelf" => (by(HIGH_SHELF, 0.0, -0.7), of("high shelf", HIGH_SHELF)),
        "floor" => (by(ROUNDS, 0.0, -0.8), of(ROUNDS_PROP, ROUNDS)),
        // Down the hall's lanes, 4 m past the knee-high wall, facing
        // their far end.
        "lanes" => ([(LANES[0] + LANES[1]) / 2.0, KNEE_WALL_Z - 4.0], None),
        // 3.5 m before the dummy (`dummy`), facing it.
        "dummy" => ([crate::dummy::PLACES[0].at.x, crate::dummy::PLACES[0].at.z + 3.5], None),
        // 3 m to the standing dummy's own right, facing it from the side.
        "dummy-side" => ([crate::dummy::PLACES[0].at.x - 3.0, crate::dummy::PLACES[0].at.z], None),
        // Before the sampler of surfaces, on the start's side of it,
        // facing it.
        // At the near end of the course's row nearer the hall, facing along
        // it (the user's).
        "surfaces" => ([23.6 + SAMPLER_SHIFT[0], -3.0 + SAMPLER_SHIFT[1]], Some((SAMPLER_PROP.to_owned(), sampler_anchor()))),
        // Before a storage cabinet, by its label, as before the row's
        // drawer.
        // (The box, lower, a step nearer.)
        _ if cabinet_named(name).is_some() => {
            let index = cabinet_named(name)?;
            let [x, z] = cabinet_at(index);
            ([x + if Furniture::of(CABINETS[index]) == Furniture::Box { 0.85 } else { 1.05 }, z], of(&cabinet_prop(index), [x, z]))
        }
        // Before a table of weapons, by its label (`rifles`,
        // `machine-guns`), three quarters of a metre from its front.
        _ => {
            let index = weapon_crate_named(name)?;
            let ([x, z], front) = weapon_table(index);
            let at = Vec3::new(x, 0.0, z) + front * (WEAPON_TABLE.z / 2.0 + 0.75);
            ([at.x, at.z], of(&weapon_table_prop(index), [x, z]))
        }
    };
    let view = match name {
        "doors" => [180.0, 5.0],
        "window" => [180.0, 5.0],
        "ledge" => [0.0, 35.0],
        "wire" => [0.0, 25.0],
        // Facing the bench's wall, down at its table.
        "workbench" => [0.0, 25.0],
        "yard" => [0.0, 5.0],
        "open" => [90.0, 5.0],
        "car" => [90.0, 32.0],
        "wooden-crate" => [180.0, 60.0],
        "supply-crate" => [180.0, 35.0],
        "scav" => [180.0, 70.0],
        "drawer" => [180.0, 50.0],
        "table" => [180.0, 36.0],
        "key-table" => [180.0, 40.0],
        "high-shelf" => [180.0, 11.0],
        "floor" => [180.0, 80.0],
        "surfaces" => [90.0, 20.0],
        "lanes" | "dummy" => [0.0, 0.0],
        "dummy-side" => [90.0, 0.0],
        "platform" | "shelf" | "stairs" => [0.0, 0.0],
        // Facing a cabinet, to the left of the lane.
        _ if cabinet_named(name).is_some() => match Furniture::of(CABINETS[cabinet_named(name)?]) {
            Furniture::Cabinet => [-90.0, 50.0],
            Furniture::Box => [-90.0, 59.0],
            Furniture::Dresser => [-90.0, 39.0],
        },
        // (The gun case on the table, its front faced; the magazine box
        // under it is reached crouched.)
        _ => {
            let (_, front) = weapon_table(weapon_crate_named(name)?);
            [(-front.x).atan2(front.z).to_degrees(), 35.0]
        }
    };
    Some((Vec3::new(x, 0.0, z), view, owner))
}

/// The last box put among the solids is made of that.
fn made_of(solids: &mut [Solid], surface: u8) {
    if let Some(last) = solids.last_mut() {
        last.surface = surface;
    }
}

/// Where a table of weapons stands, by its class's place in
/// `WEAPON_CRATES`: the middle of its top across the lane and along it, and
/// the level way from its back to its front, the side it is stood before.
/// Every table's back is to the wall: the first `ALONG_THE_WALL` to the
/// workbench's right, the rest to its left, each from the bench out.
fn weapon_table(index: usize) -> ([f32; 2], Vec3) {
    let wall = KNEE_WALL_Z + WALL_THICK / 2.0 + 0.02;
    let across = match index.checked_sub(ALONG_THE_WALL) {
        None => WEAPON_TABLE_FIRST + index as f32 * WEAPON_TABLE_STEP,
        Some(on) => -WEAPON_TABLE_FIRST - on as f32 * WEAPON_TABLE_STEP,
    };
    ([across, wall + WEAPON_TABLE.z / 2.0], Vec3::Z)
}

/// Where a place of the storage stands, by its place in `CABINETS`: the
/// middle of its footprint, its back two centimetres off the stairs' right
/// side, each after the last by the gap.
fn cabinet_at(index: usize) -> [f32; 2] {
    let before: f32 = CABINETS[..index].iter().map(|label| Furniture::of(label).size().x + CABINETS_GAP).sum();
    let size = Furniture::of(CABINETS[index]).size();
    [STAIRS_LEFT + 2.5 + 0.02 + size.z / 2.0, CABINETS_NEAR - before - size.x / 2.0]
}

/// The hall's concrete: the middle of Tarkov's own concretes' pictures
/// (0.48 to 0.67: the hideout's concrete walls, `inventory.md`, findings
/// 2026-10-19). The app's.
const HALL_GREY: Color = Color::srgb(0.55, 0.55, 0.55);
/// The colour of a solid that is not drawn (the workbench's boxes, under
/// its own model).
pub const UNSEEN: Color = Color::NONE;

/// The indoors: what the indoor trigger and the stencil volume take
/// (`lighting`, `prism_view`), a level file's (`indoors`); none for the
/// range's own, all outdoors since its hall went (0.100.0). Its middle,
/// size and turn about the upright (Bevy's axes).
pub fn indoors() -> Option<(Vec3, Vec3, f32)> {
    crate::level::get().and_then(|level| level.indoors.as_ref().map(|found| (found.centre, found.size, found.yaw)))
}

/// How far the sun's shadows reach at the least (`shadows::with_baked_reach`):
/// the hall's furthest two corners apart (48.9 m), and a little more; for a
/// level file's indoors, its own corners apart and as much more, if
/// further.
pub fn baked_reach() -> f32 {
    match (crate::level::get(), indoors()) {
        (Some(_), Some((_, size, _))) => (size.length() + 3.0).max(52.0),
        _ => 52.0,
    }
}

/// Whether a place (Bevy's axes) is indoors.
pub fn in_hall(at: Vec3) -> bool {
    indoors().is_some_and(|(centre, size, yaw)| {
        let own = Quat::from_rotation_y(-yaw) * (at - centre);
        own.abs().cmple(size / 2.0).all()
    })
}

/// Where ceiling lamps hang (Bevy's axes): a level file's (`lamp`); none
/// on the range's own, which has no roof since its hall went (0.100.0).
pub fn hall_lamps() -> Vec<Vec3> {
    crate::level::get().map(|level| level.lamps.clone()).unwrap_or_default()
}

/// The lid of a box `size` long, high and deep whose top's middle is at
/// `top` and whose front is the way `front`: a slab on it, hinged along its
/// back edge and lifted towards the back by a negative angle (a turn the
/// left hand's way about the line along the box, `front`'s up and over).
fn lid_of(top: Vec3, size: Vec3, front: Vec3, color: Color) -> Part {
    let along = Vec3::Y.cross(front);
    let centre = top + Vec3::Y * 0.015;
    Part::Moving {
        centre,
        size: along.abs() * size.x + Vec3::Y * 0.03 + front.abs() * size.z,
        yaw: 0.0,
        pivot: centre - front * size.z / 2.0,
        axis: along,
        slide: Vec3::ZERO,
        color,
    }
}

/// Each label drawn over a container and where: the tables' over their
/// gun cases, the barbed wire's and the cabinets', in Bevy's axes.
pub fn weapon_crate_labels() -> impl Iterator<Item = (&'static str, Vec3)> {
    // Each over its prop where it stands.
    let over = |prop: &str, anchor: Vec3, at: Vec3| stand_of(prop, anchor).point(anchor, at);
    let tables = WEAPON_CRATES.into_iter().enumerate().map(move |(index, name)| {
        let ([x, z], _) = weapon_table(index);
        (name, over(&weapon_table_prop(index), Vec3::new(x, 0.0, z), Vec3::new(x, WEAPON_TABLE.y + GUN_CASE.y + 0.25, z)))
    });
    let [low, high] = BARBED_WIRE;
    let wire = ("BARBED WIRE", over(WIRE_PROP, wire_anchor(), Vec3::new((low.x + high.x) / 2.0, high.y + 0.25, (low.z + high.z) / 2.0)));
    let cabinets = CABINETS.into_iter().enumerate().map(move |(index, name)| {
        let [x, z] = cabinet_at(index);
        (name, over(&cabinet_prop(index), Vec3::new(x, 0.0, z), Vec3::new(x, Furniture::of(name).size().y + 0.25, z)))
    });
    // Each square of the sampler by its surface's name, on the ground.
    let squares = surface_squares().iter().map(|square| (trp_data::sound::SURFACES[square.surface as usize], square.centre + Vec3::Y * 0.05));
    tables.chain([wire]).chain(cabinets).chain(squares)
}

/// The barbed wire's box, its lowest and highest corners in Bevy's axes,
/// where it stands (turned, the upright box about it).
pub fn barbed_wire() -> [Vec3; 2] {
    let stand = stand_of(WIRE_PROP, wire_anchor());
    let [low, high] = BARBED_WIRE;
    let corners = [low.x, high.x].into_iter().flat_map(|x| [low.z, high.z].map(|z| stand.point(wire_anchor(), Vec3::new(x, 0.0, z))));
    let (least, most) = corners.fold((Vec3::INFINITY, Vec3::NEG_INFINITY), |(a, b), c| (a.min(c), b.max(c)));
    [Vec3::new(least.x, low.y + stand.at.y, least.z), Vec3::new(most.x, high.y + stand.at.y, most.z)]
}

/// How the barbed wire is drawn: a post at each corner and three strands
/// along each long side, plain thin boxes (where each is, and its size,
/// Bevy's axes), where the wire stands. The app's look; the game's coil is
/// not read.
pub fn barbed_wire_look() -> Vec<(Transform, Vec3)> {
    let [low, high] = BARBED_WIRE;
    let mut parts = Vec::new();
    for x in [low.x, high.x] {
        for z in [low.z, high.z] {
            parts.push((Vec3::new(x, high.y / 2.0, z), Vec3::new(0.04, high.y, 0.04)));
        }
    }
    for z in [low.z, (low.z + high.z) / 2.0, high.z] {
        for share in [0.3, 0.6, 0.9] {
            parts.push((Vec3::new((low.x + high.x) / 2.0, high.y * share, z), Vec3::new(high.x - low.x, 0.012, 0.012)));
        }
    }
    let stand = stand_of(WIRE_PROP, wire_anchor());
    parts.into_iter().map(|(at, size)| (Transform::from_translation(stand.point(wire_anchor(), at)).with_rotation(Quat::from_rotation_y(stand.yaw)), size)).collect()
}

/// Where `main` lays the things to be taken up, in Bevy's axes: on the
/// table, on the floor behind the door wall, on the shelf, each where its
/// prop stands.
pub fn lying() -> [[f32; 3]; 3] {
    let on = |prop: &str, at: [f32; 2], top: f32| stand_of(prop, Vec3::new(at[0], 0.0, at[1])).point(Vec3::new(at[0], 0.0, at[1]), Vec3::new(at[0], top, at[1])).to_array();
    [on("table", TABLE, TABLE_TOP), on(ROUNDS_PROP, ROUNDS, 0.0), on("high shelf", HIGH_SHELF, SHELF_TOP)]
}

/// Where `main` lays the yard door's key, which lay in the airdrop crate,
/// then on the workbench's table (Bevy's axes): since 0.126.0 on the small
/// white table by the doorway, at its top's middle, where that table stands
/// (`KEY_TABLE`; the user's, 2026-10-25).
pub fn key_lies() -> [f32; 3] {
    let anchor = Vec3::new(KEY_TABLE[0], 0.0, KEY_TABLE[1]);
    stand_of(KEY_TABLE_PROP, anchor).point(anchor, Vec3::new(KEY_TABLE[0], KEY_TABLE_SIZE.y, KEY_TABLE[1])).to_array()
}


/// What the screen calls the container whose contents lie at that place in
/// `Carried::away`, where the range names it: a cabinet by its label, a
/// table's gun case or magazine box by its class.
pub fn weapon_crate_label(holds: usize) -> Option<String> {
    if let Some(label) = holds.checked_sub(REPAIR_BOX_HOLDS).and_then(|at| REPAIR_BOXES.get(at)) {
        return Some(format!("{label} box"));
    }
    if let Some(label) = holds.checked_sub(AMMO_BOX_HOLDS).and_then(|at| WEAPON_CRATES.get(at)).filter(|_| holds - AMMO_BOX_HOLDS < AMMO_TABLES) {
        return Some(format!("{label} ammunition box"));
    }
    if let Some(label) = holds.checked_sub(CABINET_HOLDS).and_then(|at| CABINETS.get(at)) {
        return Some(format!("{label} {}", Furniture::of(label).word()));
    }
    let case = holds.checked_sub(GUN_CASE_HOLDS).and_then(|at| WEAPON_CRATES.get(at)).map(|label| if of_its_own(label) { format!("{label} case") } else { format!("{label} weapon case") });
    case.or_else(|| holds.checked_sub(MAGAZINE_BOX_HOLDS).and_then(|at| WEAPON_CRATES.get(at)).map(|label| if of_its_own(label) { format!("{} box", box_name(label)) } else { format!("{label} magazine box") }))
}

/// Which crate of weapons a start's name is: its label in small letters,
/// a dash for each space.
fn weapon_crate_named(name: &str) -> Option<usize> {
    WEAPON_CRATES.iter().position(|label| label.to_lowercase().replace(' ', "-") == name)
}

/// Which storage cabinet a start's name is, the same way (`rigs`,
/// `masks-and-hats`).
fn cabinet_named(name: &str) -> Option<usize> {
    CABINETS.iter().position(|label| label.to_lowercase().replace(' ', "-") == name)
}


/// A crate's label, at its place in the world, drawn where the camera
/// sees that place.
#[derive(Component)]
pub struct CrateLabel(Vec3);

/// How wide a label's box is, and how far off a label is still drawn, in
/// pixels and metres: the app's. Within that reach two crates side by
/// side are further apart on the screen than a label is wide.
const LABEL_WIDTH: f32 = 160.0;
const LABEL_REACH: f32 = 6.0;

pub fn spawn_labels(mut commands: Commands) {
    for (name, at) in weapon_crate_labels() {
        let node = Node { position_type: PositionType::Absolute, width: Val::Px(LABEL_WIDTH), justify_content: JustifyContent::Center, ..default() };
        commands.spawn((CrateLabel(at), node, Visibility::Hidden, GlobalZIndex(-1))).with_children(|label| {
            let inside = Node { padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)), ..default() };
            label.spawn((inside, BackgroundColor(Color::srgba(0.07, 0.08, 0.07, 0.7)))).with_children(|inside| {
                inside.spawn(crate::inventory_view::words(name, 14.0, Color::srgb(0.90, 0.88, 0.78)));
            });
        });
    }
}

/// Puts each label where the camera sees its crate; none is drawn behind
/// the camera, further off than `LABEL_REACH`, on the other side of the
/// hall's walls from it, or under an open screen.
pub fn place_labels(
    camera: Query<(&Camera, &GlobalTransform), With<crate::MainCamera>>,
    controller: Res<crate::controller::Controller>,
    mut labels: Query<(&CrateLabel, &mut Node, &mut Visibility)>,
) {
    let Ok((camera, eye)) = camera.single() else { return };
    for (label, mut node, mut shown) in &mut labels {
        let near = eye.translation().distance(label.0) < LABEL_REACH && in_hall(eye.translation()) == in_hall(label.0) && !controller.inventory_open;
        let at = camera.world_to_viewport(eye, label.0).ok().filter(|_| near);
        let wanted = if at.is_some() { Visibility::Inherited } else { Visibility::Hidden };
        if *shown != wanted {
            *shown = wanted;
        }
        if let Some(at) = at {
            let (left, top) = (Val::Px(at.x - LABEL_WIDTH / 2.0), Val::Px(at.y - 10.0));
            if node.left != left || node.top != top {
                (node.left, node.top) = (left, top);
            }
        }
    }
}

/// A door's leaf as the character's collision has it, as the door stands.
fn leaf_block(thing: &Thing) -> Block {
    let hinge = thing.hinge.expect("a door");
    let along = turn(hinge.forward, hinge.axis, thing.angle());
    let foot = std::array::from_fn(|k| hinge.at[k] + along[k] * LEAF[0] / 2.0);
    Block::turned(foot, LEAF, along)
}

impl Solid {
    fn corners(&self) -> (Vec3, Vec3) {
        (self.centre - self.size / 2.0, self.centre + self.size / 2.0)
    }

    /// The box in the simulation's axes (z ahead): its corners in its own
    /// axes about its middle, its turn the other way about the upright.
    fn block(&self) -> Block {
        let (low, high) = self.corners();
        Block { low: [low.x, low.y, -high.z], high: [high.x, high.y, -low.z], turn: -self.yaw }
    }

    /// How far along the line (Bevy's axes) the box is entered, as a share
    /// of the line. A line that starts inside it, or runs along a face,
    /// does not enter it.
    fn entered(&self, from: Vec3, to: Vec3) -> Option<f32> {
        self.block().entered(in_sim(from), in_sim(to))
    }

    /// Its own axes in the simulation's (z ahead), and how far it reaches
    /// along each.
    fn own_axes(&self) -> ([Vec3; 3], Vec3) {
        let (sin, cos) = self.yaw.sin_cos();
        ([Vec3::new(cos, 0.0, sin), Vec3::Y, Vec3::new(-sin, 0.0, cos)], self.size / 2.0)
    }
}

impl Placed {
    /// Where what moves of it is drawn, as it stands now, in Bevy's axes,
    /// and its size and colour; nothing for a thing with no moving part.
    pub fn part_shown(&self) -> Option<(Transform, Vec3, Color)> {
        match self.part {
            Part::None => None,
            Part::Leaf { size, color } => {
                let hinge = self.thing.hinge?;
                let along = in_bevy(turn(hinge.forward, hinge.axis, self.thing.angle()));
                let at = in_bevy(hinge.at) + along * size.x / 2.0 + Vec3::Y * size.y / 2.0;
                Some((Transform::from_translation(at).with_rotation(Quat::from_rotation_arc(Vec3::X, along)), size, color))
            }
            Part::Moving { centre, size, yaw, pivot, axis, slide, color } => {
                let turned = Quat::from_axis_angle(axis, self.thing.angle().to_radians());
                let at = pivot + turned * (centre - pivot) + slide * self.thing.out();
                Some((Transform::from_translation(at).with_rotation(turned * Quat::from_rotation_y(yaw)), size, color))
            }
        }
    }

    /// What a line of sight meets of it, in the simulation's axes.
    fn met(&self, site: &Site) -> Block {
        self.leaf.map_or(self.hit, |leaf| site.blocks[leaf])
    }
}

impl Range {
    /// Where a prop stands, by its name and the place it is made about.
    fn stand_of(&self, name: &str, anchor: Vec3) -> crate::level::Stand {
        self.props.iter().find(|prop| prop.name == name).map_or(crate::level::Stand::made(anchor), |prop| prop.stand)
    }

    /// A start and its view, where the level file puts it, else as made
    /// with its prop where that stands (Bevy's axes, the view's turn in
    /// degrees right of the lane and below level).
    fn start(&self, name: &str) -> Option<(Vec3, [f32; 2])> {
        let made = made_start(name);
        if let Some(found) = self.level.as_ref().and_then(|level| level.starts.get(name)) {
            return Some((found.feet, [found.yaw, made.map_or(0.0, |(_, view, _)| view[1])]));
        }
        let (feet, view, owner) = made?;
        Some(match owner {
            Some((prop, anchor)) => {
                let stand = self.stand_of(&prop, anchor);
                (stand.point(anchor, feet), [view[0] - stand.yaw.to_degrees(), view[1]])
            }
            None => (feet, view),
        })
    }

    /// The place in the range a capture can start the character at: the
    /// far edge of the platform, the shelf, the foot of the stairs; before
    /// the doors on the lane's side, or past them; or before one of the
    /// things to work or take up, within reach of it with the view turned
    /// as `start_view` says; or a level file's (`start <name>`). In the
    /// simulation's axes, feet on the floor.
    pub fn place(&self, name: &str) -> Option<[f32; 3]> {
        let (feet, _) = self.start(name)?;
        let at = [feet.x, -feet.z];
        Some([at[0], self.ground_height(at[0], at[1]), at[1]])
    }

    /// What `--export-level` writes (`level`): the boxes but the props',
    /// each prop where it stands with its parts, the starts, the lamps and
    /// the indoors.
    pub fn layout(&self) -> crate::level::Layout {
        use crate::level::{LevelBox, PropLayout, Start};
        let as_box = |name: String, solid: &Solid| LevelBox { name, centre: solid.centre, size: solid.size, yaw: solid.yaw, color: solid.color, surface: solid.surface };
        let of_prop: Vec<bool> = (0..self.solids.len()).map(|at| self.props.iter().any(|prop| prop.solids.contains(&at))).collect();
        let boxes = self.solids.iter().zip(&of_prop).filter(|(_, of)| !**of).map(|(solid, _)| as_box(solid.name.to_owned(), solid)).collect();
        let props = self
            .props
            .iter()
            .map(|prop| {
                let mut parts: Vec<LevelBox> = self.solids[prop.solids.clone()].iter().map(|solid| as_box(String::new(), solid)).collect();
                // What moves of its things, as it stands now.
                for placed in &self.things[prop.things.clone()] {
                    if let Some((place, size, color)) = placed.part_shown() {
                        let x = place.rotation * Vec3::X;
                        parts.push(LevelBox { name: String::new(), centre: place.translation, size, yaw: (-x.z).atan2(x.x), color, surface: trp_sim::steps::CONCRETE });
                    }
                }
                // The props without solids, drawn by their looks.
                let flat = |at: Vec3, size: Vec3, yaw: f32, color: Color| LevelBox { name: String::new(), centre: at, size, yaw, color, surface: trp_sim::steps::CONCRETE };
                match prop.name.as_str() {
                    WIRE_PROP => parts.extend(barbed_wire_look().into_iter().map(|(place, size)| flat(place.translation, size, prop.stand.yaw, Color::srgb(0.22, 0.20, 0.17)))),
                    // Each square named by its surface's number.
                    SAMPLER_PROP => parts.extend(surface_squares().iter().map(|square| LevelBox {
                        name: format!("{SAMPLER_PROP} part {}", square.surface + 1),
                        ..flat(square.centre + Vec3::Y * 0.005, Vec3::new(square.size.x - 0.04, 0.01, square.size.y - 0.04), square.yaw, square.color)
                    })),
                    ROUNDS_PROP => parts.push(flat(prop.stand.at + Vec3::Y * 0.02, Vec3::new(0.3, 0.04, 0.15), prop.stand.yaw, Color::srgb(0.6, 0.45, 0.2))),
                    _ => {}
                }
                PropLayout { name: prop.name.clone(), stand: prop.stand, parts }
            })
            .collect();
        let mut starts: Vec<(String, Start, Option<String>)> = Vec::new();
        let mut names = start_names();
        if let Some(level) = &self.level {
            let mut more: Vec<String> = level.starts.keys().filter(|name| !names.contains(name)).cloned().collect();
            more.sort();
            names.extend(more);
        }
        for name in names {
            let Some((feet, [yaw, _])) = self.start(&name) else { continue };
            let owner = made_start(&name).and_then(|(_, _, owner)| owner).map(|(prop, _)| prop);
            starts.push((name, Start { feet, yaw }, owner));
        }
        let indoors = indoors().map(|(centre, size, yaw)| LevelBox { name: "indoors".to_owned(), centre, size, yaw, color: Color::WHITE, surface: trp_sim::steps::CONCRETE });
        crate::level::Layout { boxes, props, starts, lamps: hall_lamps(), indoors }
    }

    /// Every box of the range as it stands, in Bevy's axes (middle, size,
    /// turn about the upright): the character's collision's (the solids
    /// and the doors' leaves) and the things' own (the containers, the
    /// tables). What a corpse falls against (`death`).
    pub fn boxes(&self) -> Vec<(Vec3, Vec3, Quat)> {
        let things = self.things.iter().filter(|placed| placed.leaf.is_none()).map(|placed| &placed.hit);
        self.site
            .blocks
            .iter()
            .chain(things)
            .map(|block| {
                let middle = [(block.low[0] + block.high[0]) / 2.0, (block.low[1] + block.high[1]) / 2.0, (block.low[2] + block.high[2]) / 2.0];
                let size = Vec3::new(block.high[0] - block.low[0], block.high[1] - block.low[1], block.high[2] - block.low[2]);
                // The game's turn about straight up, in Bevy's mirrored
                // axes: the other way about.
                (in_bevy(middle), size, Quat::from_rotation_y(-block.turn))
            })
            .collect()
    }

    /// Which way to look from a place of `place` (degrees right of the
    /// lane, degrees below level) for what is there to be in reach; at a
    /// level file's start its arrow's way, the pitch the range's own start
    /// of that name has (level, for one of the file's own).
    pub fn start_view(&self, name: &str) -> Option<[f32; 2]> {
        self.start(name).map(|(_, view)| view)
    }

    /// Locks one of the range's doors, the first or the second, with the
    /// key of that item template.
    pub fn lock_door(&mut self, which: usize, key: &str) {
        if let Some(door) = self.things.iter_mut().filter(|placed| placed.name == "door").nth(which) {
            door.thing.lock(Some(key.to_owned()));
        }
    }

    /// Moves every thing on by a frame of the simulation's clock: the game
    /// runs their routines after the character's update, so this comes
    /// after the character's frame. A door's leaf is put where it now stands.
    pub fn step_things(&mut self, dt: f32) {
        let Some(settings) = self.thing_settings.as_ref() else { return };
        for placed in &mut self.things {
            placed.thing.step(dt, settings);
            if let Some(leaf) = placed.leaf {
                self.site.blocks[leaf] = leaf_block(&placed.thing);
            }
        }
    }

    /// What a spent case's cast meets (`BouncingObject.Raycast` on the
    /// shells' mask, the simulation's axes): the first of the solids, the
    /// doors' leaves (wood) and the ground (a square of the sampler's, or
    /// concrete), as the steps' line down finds them, with how far, where,
    /// the face's normal and the surface.
    pub fn cast(&self, from: [f32; 3], way: [f32; 3], reach: f32) -> Option<trp_sim::shells::Hit> {
        let length = (way[0] * way[0] + way[1] * way[1] + way[2] * way[2]).sqrt();
        if length < 1e-9 {
            return None;
        }
        let to: [f32; 3] = std::array::from_fn(|k| from[k] + way[k] / length * reach);
        let solids = self.solids.iter().filter_map(|solid| solid.block().entered_face(from, to).map(|(t, normal)| (t, normal, solid.surface)));
        let leaves = self.things.iter().filter_map(|placed| placed.leaf).filter_map(|leaf| self.site.blocks[leaf].entered_face(from, to).map(|(t, normal)| (t, normal, WOOD)));
        let ground = (from[1] >= 0.0 && to[1] < 0.0).then(|| {
            let t = from[1] / (from[1] - to[1]);
            let at: [f32; 3] = std::array::from_fn(|k| from[k] + (to[k] - from[k]) * t);
            (t, [0.0, 1.0, 0.0], ground_surface(at[0], -at[2]))
        });
        let (t, normal, surface) = solids.chain(leaves).chain(ground).min_by(|a, b| a.0.total_cmp(&b.0))?;
        Some(trp_sim::shells::Hit { distance: t * reach, point: std::array::from_fn(|k| from[k] + (to[k] - from[k]) * t), normal, surface })
    }

    /// What a bullet's cast meets (`trp_sim::ballistics::BulletWorld`;
    /// ballistics spec, "Surfaces"): each box the ray enters through a face
    /// turned toward it (the roofs too; nothing it starts inside), each
    /// door's leaf, and the ground from above, each once, with the preset
    /// of the material its surface gives (the ground's by the sampler's
    /// square under the hit). Every box is a thing of its own; the ground
    /// is one collider whose thing is its material, as the game's terrain
    /// is (`TerrainBallistic`).
    pub fn bullet_hits(&self, from: [f32; 3], to: [f32; 3], presets: &[trp_data::hard_settings::BallisticPreset], hits: &mut Vec<trp_sim::ballistics::RayHit>) {
        use trp_sim::ballistics::{material, preset_surface, RayHit};
        let way: [f32; 3] = std::array::from_fn(|k| to[k] - from[k]);
        let length = (way[0] * way[0] + way[1] * way[1] + way[2] * way[2]).sqrt();
        if length < 1e-9 {
            return;
        }
        let at = |t: f32| -> [f32; 3] { std::array::from_fn(|k| from[k] + way[k] * t) };
        for (index, solid) in self.solids.iter().enumerate() {
            if let Some((t, normal)) = solid.block().entered_face(from, to) {
                let id = index as u64 + 1;
                hits.push(RayHit { distance: t * length, point: at(t), normal, collider: id, thing: id, mesh: false, surface: preset_surface(presets, material::of_surface(solid.surface)), body: None });
            }
        }
        for (index, leaf) in self.things.iter().enumerate().filter_map(|(index, placed)| placed.leaf.map(|leaf| (index, leaf))) {
            if let Some((t, normal)) = self.site.blocks[leaf].entered_face(from, to) {
                let id = BULLET_LEAVES + index as u64;
                hits.push(RayHit { distance: t * length, point: at(t), normal, collider: id, thing: id, mesh: false, surface: preset_surface(presets, material::of_surface(WOOD)), body: None });
            }
        }
        let ground = self.site.ground;
        if from[1] > ground && to[1] <= ground {
            let t = (from[1] - ground) / (from[1] - to[1]);
            let point = at(t);
            let made_of = material::of_surface(ground_surface(point[0], -point[2]));
            hits.push(RayHit { distance: t * length, point, normal: [0.0, 1.0, 0.0], collider: BULLET_GROUND, thing: BULLET_GROUND + 1 + made_of as u64, mesh: false, surface: preset_surface(presets, made_of), body: None });
        }
    }

    /// How far along the line from `from` to `to` (the simulation's axes)
    /// the first of the range's boxes or the ground is met, as a share of
    /// it: what a blast's line meets (`HighPolyWithTerrainMask`: the world's
    /// solids and its terrain). The range's boxes are the app's own, and no
    /// fragment pierces them (`PROVISIONAL`: they have no ballistic
    /// materials).
    pub fn entered(&self, from: [f32; 3], to: [f32; 3]) -> Option<f32> {
        let ground = self.site.ground;
        let under = (from[1] >= ground && to[1] < ground).then(|| (from[1] - ground) / (from[1] - to[1]));
        let things = self.things.iter().filter(|placed| placed.leaf.is_none()).map(|placed| &placed.hit);
        self.site.blocks.iter().chain(things).filter_map(|block| block.entered(from, to)).chain(under).min_by(f32::total_cmp)
    }

    /// Which thing a line of sight from `from` to `to` (Bevy's axes) meets
    /// first, before anything else solid, within `door_reach` of `from`
    /// for a door and anywhere along the line for the rest
    /// (`Player.InteractionRaycast`: a door within `DOOR_RAYCAST_DISTANCE`
    /// and the rest within `LOOT_RAYCAST_DISTANCE`, each with `BEHIND_CAST`).
    pub fn looked_at(&self, from: Vec3, to: Vec3, door_reach: f32) -> Option<usize> {
        let ground = (from.y >= 0.0 && to.y < 0.0).then(|| from.y / (from.y - to.y)).unwrap_or(f32::INFINITY);
        let solid = self.solids.iter().filter_map(|solid| solid.entered(from, to)).fold(ground, f32::min);
        let (sim_from, sim_to) = (in_sim(from), in_sim(to));
        let (index, share) = self
            .things
            .iter()
            .enumerate()
            .filter_map(|(index, placed)| placed.met(&self.site).entered(sim_from, sim_to).map(|share| (index, share)))
            .min_by(|a, b| a.1.total_cmp(&b.1))?;
        let length = from.distance(to);
        let near_enough = self.things[index].leaf.is_none() || share * length <= door_reach;
        (share <= solid + 1e-5 && near_enough).then_some(index)
    }
}

impl World for Range {
    fn ground_height(&self, x: f32, z: f32) -> f32 {
        self.site.height(x, z)
    }

    /// A foot that has come down onto the edge of a top it overhangs is
    /// held by it: the game's controller pushes a capsule out of what it
    /// has sunk into, and counts one pushed upward as on the ground
    /// (movement spec, "The character's own collision"). This is how a
    /// climb's end stays up, the axis still short of the top. A character
    /// that walks off a top is not held: it is level with the top when its
    /// axis passes the edge and falls clear of the corner (recorded).
    fn held(&self, at: [f32; 3], capsule: Capsule) -> bool {
        self.site.held(at, capsule, self.radius)
    }

    /// The solids, and the doors' leaves as they stand.
    fn line(&self, from: [f32; 3], to: [f32; 3]) -> Option<f32> {
        let length = (0..3).map(|k| (to[k] - from[k]).powi(2)).sum::<f32>().sqrt();
        let leaves = self.things.iter().filter_map(|placed| placed.leaf).filter_map(|leaf| self.site.blocks[leaf].entered(from, to));
        let (from, to) = (in_bevy(from), in_bevy(to));
        // The ground, from above only.
        let ground = (from.y >= 0.0 && to.y < 0.0).then(|| from.y / (from.y - to.y));
        let nearest = self.solids.iter().filter_map(|solid| solid.entered(from, to)).chain(ground).chain(leaves).reduce(f32::min)?;
        Some(nearest * length)
    }

    /// What a step's line down meets first: a box's surface, a door's leaf
    /// (wood), or the ground's where it is hit (a square of the sampler's,
    /// or concrete).
    fn surface(&self, from: [f32; 3], to: [f32; 3]) -> Option<u8> {
        let leaves = self.things.iter().filter_map(|placed| placed.leaf).filter_map(|leaf| self.site.blocks[leaf].entered(from, to)).map(|t| (t, WOOD));
        let (from, to) = (in_bevy(from), in_bevy(to));
        let ground = (from.y >= 0.0 && to.y < 0.0).then(|| from.y / (from.y - to.y)).map(|t| {
            let at = from + (to - from) * t;
            (t, ground_surface(at.x, at.z))
        });
        let solids = self.solids.iter().filter_map(|solid| solid.entered(from, to).map(|t| (t, solid.surface)));
        solids.chain(ground).chain(leaves).min_by(|a, b| a.0.total_cmp(&b.0)).map(|(_, surface)| surface)
    }

    /// Whatever a line hits above the ground is something standing on it.
    fn terrain(&self, at: [f32; 3]) -> bool {
        at[1] <= 1e-3
    }

    /// The game's own controller's move: out of each box the capsule is
    /// in by the shortest way, straight up where that is mostly upward,
    /// never upward in a vault or a climb, and 5 cm a move out of what it
    /// is deep in (`trp_sim::collision`). The doors' leaves are among them.
    fn obstructed(&self, from: [f32; 3], to: [f32; 3], capsule: Capsule) -> [f32; 3] {
        self.site.settle(from, to, capsule, self.radius)
    }

    /// A door's leaf the capsule is in lifts the hard cap on speed, as the
    /// game's controller lets every door's: a door swung into the character
    /// pushes it out of its way.
    fn lifts_cap(&self, at: [f32; 3], capsule: Capsule) -> bool {
        self.things.iter().filter_map(|placed| placed.leaf).any(|leaf| self.site.blocks[leaf].overlaps(at, capsule, self.radius))
    }

    /// That move puts the feet on the ground and on the boxes' tops too.
    fn sets_feet(&self) -> bool {
        true
    }

    /// A mount's rays (`trp_sim::mounting`): the solids and the doors' leaves
    /// as they stand, each face's normal, and the ground from above. Every
    /// layer the game names is the same here.
    fn ray(&self, from: [f32; 3], direction: [f32; 3], length: f32, _layer: trp_sim::mounting::Layer) -> Option<trp_sim::mounting::Hit> {
        let to: [f32; 3] = std::array::from_fn(|k| from[k] + direction[k] * length);
        let solids = self.solids.iter().filter_map(|solid| solid.block().entered_face(from, to));
        let leaves = self.things.iter().filter_map(|placed| placed.leaf).filter_map(|leaf| self.site.blocks[leaf].entered_face(from, to));
        let ground = (from[1] >= self.site.ground && to[1] < self.site.ground).then(|| ((from[1] - self.site.ground) / (from[1] - to[1]), [0.0, 1.0, 0.0]));
        let (share, normal) = solids.chain(leaves).chain(ground).min_by(|a, b| a.0.total_cmp(&b.0))?;
        let distance = share * length;
        Some(trp_sim::mounting::Hit { distance, point: std::array::from_fn(|k| from[k] + direction[k] * distance), normal })
    }

    /// The prone mount's room ahead: clear of the solids and above the ground.
    fn sphere_clear(&self, centre: [f32; 3], radius: f32, _layer: trp_sim::mounting::Layer) -> bool {
        centre[1] - radius >= self.site.ground && !self.solids.iter().any(|solid| solid.block().sphere_overlaps(centre, radius))
    }

    /// The room checks' boxes (movement spec, "Room to move"): against the
    /// site's boxes (the solids and the doors' leaves as they stand) and
    /// under the ground. The box is taken upright, turned by its yaw alone:
    /// the range is level, so a lying body's lay on it is nothing.
    fn box_overlaps(&self, centre: [f32; 3], half: [f32; 3], turn: [f32; 4]) -> bool {
        let forward = trp_sim::math::quat_rotate(turn, [0.0, 0.0, 1.0]);
        let yaw = forward[0].atan2(forward[2]).to_degrees();
        centre[1] - half[1] < self.site.ground || self.site.blocks.iter().any(|block| block.upright_box_overlaps(centre, half, yaw))
    }

    /// A lying body's room ahead and behind: the sphere tried every
    /// centimetre against the site's boxes and the ground (PROVISIONAL: the
    /// game sweeps it whole, which something thinner than a centimetre
    /// between two tries would meet and these not); what it starts in is
    /// not met, as the engine's sweep does not.
    fn sphere_cast(&self, from: [f32; 3], radius: f32, direction: [f32; 3], length: f32) -> Option<f32> {
        const TRY_EVERY: f32 = 0.01;
        let touches = |along: f32| {
            let at: [f32; 3] = std::array::from_fn(|k| from[k] + direction[k] * along);
            at[1] - radius < self.site.ground || self.site.blocks.iter().any(|block| block.sphere_overlaps(at, radius))
        };
        if touches(0.0) {
            return None;
        }
        let tries = (length / TRY_EVERY).ceil().max(1.0) as usize;
        (1..=tries).map(|i| length * i as f32 / tries as f32).find(|along| touches(*along))
    }

    /// The head's capsule (movement spec, "The head's push out of walls"):
    /// out of each of the site's boxes it reaches into (the solids and the
    /// doors' leaves as they stand) by the shortest way, and out of the
    /// ground, each lengthened by a millimetre, added up.
    fn capsule_push(&self, ends: [[f32; 3]; 2], radius: f32) -> [f32; 3] {
        const MARGIN: f32 = 0.001;
        let mut push = [0.0; 3];
        for (way, depth) in self.site.blocks.iter().filter_map(|block| block.capsule_push(ends, radius)) {
            for k in 0..3 {
                push[k] += way[k] * (depth + MARGIN);
            }
        }
        let under = self.site.ground - (ends[0][1].min(ends[1][1]) - radius);
        if under > 0.0 {
            push[1] += under + MARGIN;
        }
        push
    }

    /// A mount's way to its place: the box tried every 5 cm along the way
    /// against the solids (PROVISIONAL: the game sweeps it whole, which a
    /// thin wall between two tries would meet and these not).
    fn box_sweep(&self, from: [f32; 3], half: [f32; 3], yaw: f32, direction: [f32; 3], length: f32, _layer: trp_sim::mounting::Layer) -> bool {
        const TRY_EVERY: f32 = 0.05;
        let tries = (length / TRY_EVERY).ceil().max(1.0) as usize;
        (0..=tries).any(|i| {
            let along = length * i as f32 / tries as f32;
            let at: [f32; 3] = std::array::from_fn(|k| from[k] + direction[k] * along);
            self.solids.iter().any(|solid| solid.block().upright_box_overlaps(at, half, yaw))
        })
    }

    /// A mounted turn outward: the capsule moved aside against the site's
    /// boxes (PROVISIONAL: the game's prediction also turns the capsule about
    /// its pivot by twice the turn, not felt here: an upright capsule turns into
    /// itself about its own axis).
    fn turn_blocked(&self, at: [f32; 3], shift: [f32; 3], _turn: f32, capsule: Capsule) -> bool {
        let moved: [f32; 3] = std::array::from_fn(|k| at[k] + shift[k]);
        self.site.blocks.iter().any(|block| block.overlaps(moved, capsule, self.radius))
    }

    /// The camera's box is kept out of the solids; the doors' leaves are
    /// not among them.
    fn push_out(&self, centre: [f32; 3], half: [f32; 3], turn: [f32; 4], margin: f32) -> [f32; 3] {
        // The box's own axes, and how far it reaches along a direction.
        let turn = Quat::from_xyzw(turn[0], turn[1], turn[2], turn[3]);
        let (centre, half) = (Vec3::from(centre), Vec3::from(half));
        let axes = [turn * Vec3::X, turn * Vec3::Y, turn * Vec3::Z];
        let reach = |along: Vec3| (0..3).map(|k| along.dot(axes[k]).abs() * half[k]).sum::<f32>();

        // The ground: up by as far as the lowest corner is under it.
        let mut push = Vec3::ZERO;
        let under = reach(Vec3::Y) - centre.y;
        if under > 0.0 {
            push.y += under + margin;
        }
        // Each solid: out along whichever direction it takes the least to
        // part the two boxes (their faces' and their edges' crossings).
        for solid in &self.solids {
            // The solid in the simulation's axes, z ahead, and its own axes.
            let (middle, (faces, size)) = (Vec3::new(solid.centre.x, solid.centre.y, -solid.centre.z), solid.own_axes());
            let between = centre - middle;
            let mut least: Option<(f32, Vec3)> = None;
            let edges = faces.iter().flat_map(|face| axes.iter().map(move |axis| face.cross(*axis)));
            let mut apart = false;
            for along in faces.into_iter().chain(axes).chain(edges) {
                // Two edges that run alike cross in nothing.
                let Some(along) = along.try_normalize() else { continue };
                let gap = between.dot(along);
                let solid_reach = (0..3).map(|k| along.dot(faces[k]).abs() * size[k]).sum::<f32>();
                let overlap = reach(along) + solid_reach - gap.abs();
                if overlap <= 0.0 {
                    apart = true;
                    break;
                }
                if least.is_none_or(|(most, _)| overlap < most) {
                    least = Some((overlap, if gap < 0.0 { -along } else { along }));
                }
            }
            if let (false, Some((overlap, out))) = (apart, least) {
                push += out * (overlap + margin);
            }
        }
        push.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// How far a body's sphere goes from a start along a yaw before it
    /// meets anything of the range (the game's axes; yaw 0 along +z).
    fn clear_ahead(range: &Range, at: [f32; 3], yaw: f32) -> f32 {
        use trp_sim::movement::World as _;
        let (s, c) = yaw.to_radians().sin_cos();
        range.sphere_cast([at[0], at[1] + 1.0, at[2]], 0.4, [s, 0.0, c], 400.0).unwrap_or(400.0)
    }

    /// The open start has 40 m of floor clear ahead, for a long sprint.
    #[test]
    fn the_open_start_has_forty_metres_clear() {
        let range = Range::default();
        let at = range.place("open").unwrap();
        let [yaw, _] = range.start_view("open").unwrap();
        assert!(clear_ahead(&range, at, yaw) >= 300.0, "{}", clear_ahead(&range, at, yaw));
    }
    use trp_data::curve::{Curve, Key};
    use trp_sim::things::ThingState;

    /// The install's `DoorCurve` and `DelayToOpenContainer` (`probe
    /// hardsettings`), for the things to turn by without an install.
    fn thing_settings() -> ThingSettings {
        let key = |time, value, slope| Key { time, value, in_slope: slope, out_slope: slope };
        let curve = Curve { keys: vec![key(0.0, 0.0, 1.11), key(0.568, 0.546, 0.61), key(1.718, 0.976, 0.10), key(2.5, 1.0, 0.01)] };
        ThingSettings { curve, answers_after: 0.55 }
    }

    /// A box set into the ground is lifted out of it, and one set into the
    /// range's wall is moved out of the wall by the shortest way, each by
    /// the margin more; a box clear of everything is left alone.
    #[test]
    fn a_box_is_pushed_out_of_the_ground_and_the_wall() {
        let range = Range::default();
        let (half, level, margin) = ([0.065, 0.115, 0.115], [0.0, 0.0, 0.0, 1.0], 0.001);
        assert_eq!(range.push_out([0.0, 1.5, 0.0], half, level, margin), [0.0; 3]);
        let lifted = range.push_out([0.0, 0.1, 0.0], half, level, margin);
        assert!((lifted[1] - 0.016).abs() < 1e-5 && lifted[0] == 0.0 && lifted[2] == 0.0, "{lifted:?}");
        // Tipped a quarter turn about the view, its width stands upright.
        let tipped = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
        let lifted = range.push_out([0.0, 0.1, 0.0], half, tipped.to_array(), margin);
        assert!(lifted[1].abs() < 1e-5, "{lifted:?}");
        // The door wall's face on the start's side is 5.8 m behind the
        // start (-5.8 in the simulation's axes); the box's back is 3 cm into
        // it, to the window's right.
        let out = range.push_out([3.5, 1.5, -(DOOR_WALL_Z - WALL_THICK / 2.0) + 0.115 - 0.03], half, level, margin);
        assert!((out[2] - 0.031).abs() < 1e-5 && out[0].abs() < 1e-6 && out[1].abs() < 1e-6, "{out:?}");
    }

    /// The stairs are ridden up at a walk's pace by the game's own rules
    /// and the platform's side is a wall; off its edge there is nothing
    /// under the feet, and a character that finds itself against the
    /// platform's face on its way down is moved out of it a twentieth of
    /// a metre a move.
    #[test]
    fn the_platform_is_climbed_by_its_stairs_and_fallen_from() {
        let range = Range::default();
        let height = Capsule { underside: 0.0, top: 1.6, lift: true };
        // As the simulation moves: the motion, what stands in the way,
        // then the ground under where that leaves the feet. A walk's
        // pace at 120 frames a second.
        let walk = |mut at: [f32; 3], step: [f32; 2], until: &dyn Fn([f32; 3]) -> bool| {
            for frame in 0..4000 {
                if until(at) {
                    return (at, frame);
                }
                let to = range.obstructed(at, [at[0] + step[0], at[1], at[2] + step[1]], height);
                at = [to[0], to[1].max(range.ground_height(to[0], to[2])), to[2]];
            }
            (at, 4000)
        };
        let start = range.place("stairs").unwrap();
        assert_eq!(start[1], 0.0);
        // Where the stairs end, along the simulation's z.
        let top = STAIRS as f32 * TREAD - STAIRS_FOOT;
        let (at, frames) = walk(start, [0.0, 0.0235], &|at| at[2] > top + 1.0);
        let level = ((top + 1.0 - start[2]) / 0.0235).ceil();
        println!("the stairs: {frames} frames to the platform, {level} on level ground");
        assert!((at[1] - PLATFORM).abs() < 1e-4 && (frames as f32) < level * 1.25, "{at:?} after {frames} frames");
        // From the ground, the lower platform's right side stops the
        // character its radius away, and it stays on the ground.
        let (stopped, _) = walk([STAIRS_LEFT + 12.0, 0.0, top + 2.5], [-0.0235, 0.0], &|at| at[0] < STAIRS_LEFT + 7.0);
        assert!((stopped[0] - (STAIRS_LEFT + 8.0 + range.radius)).abs() < 1e-4 && stopped[1] == 0.0, "{stopped:?}");
        // Off the far edge, on the half the second flight does not go up:
        // nothing under the feet.
        let (edge, middle) = (top + 5.0, STAIRS_LEFT + 3.75);
        assert_eq!(range.ground_height(middle, edge - 0.01), PLATFORM);
        assert_eq!(range.ground_height(middle, edge + 0.01), 0.0);
        // Falling with its axis a centimetre past the edge, the capsule
        // is in the platform's face: out of it 5 cm a move, and down.
        let falling = range.obstructed([middle, 2.0, edge + 0.01], [middle, 1.99, edge + 0.01], height);
        assert!((falling[2] - (edge + 0.06)).abs() < 2e-3 && falling[1] < 2.0 && falling[0] == middle, "{falling:?}");
        // And it cannot walk back in under the platform's top.
        let (back, _) = walk([middle, 0.0, edge + 1.0], [0.0, -0.0235], &|at| at[2] < edge);
        assert!((back[2] - (edge + range.radius)).abs() < 1e-4, "{back:?}");
        // From the platform, the second flight climbs to the ledge, 6 m up
        // (health spec, "Falls"), on the way the first goes.
        let across = STAIRS_LEFT + 1.25;
        let ledge = top + 5.0 + LEDGE_STEPS as f32 * TREAD;
        let (up, frames) = walk([across, PLATFORM, top + 4.0], [0.0, 0.0235], &|at| at[2] > ledge + 1.0);
        println!("the second flight: {frames} frames to the ledge");
        assert!((up[1] - LEDGE).abs() < 1e-4, "{up:?} after {frames} frames");
        assert_eq!(range.place("ledge").map(|at| at[1]), Some(LEDGE));
    }

    /// The range written as a level file and read back is the same range:
    /// every box with its turn, colour and surface, every thing, start and
    /// lamp, and the indoors. And a prop stood elsewhere and turned in the
    /// file takes its solids, its things and its start with it.
    #[test]
    fn the_range_goes_through_a_level_file() {
        use crate::level::{Level, Stand};
        use std::sync::Arc;
        let range = Range::default();
        let text = serde_json::to_string(&crate::level::to_gltf(&range.layout())).unwrap();
        let mut warnings = Vec::new();
        let level = Level::of_gltf(&serde_json::from_str(&text).unwrap(), &mut |line| warnings.push(line));
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!((level.props.len(), level.lamps.len()), (range.props.len(), hall_lamps().len()));
        // (The range's own has no indoors since its hall went: none written.)
        match (indoors(), level.indoors.as_ref()) {
            (Some((centre, size, _)), Some(found)) => assert!(found.centre.distance(centre) < 1e-4 && found.size.distance(size) < 1e-4),
            (None, None) => {}
            (made, read) => panic!("the indoors {made:?} became {read:?}"),
        }
        // The sampler's squares, each by its surface.
        let squares = level.squares.as_ref().unwrap();
        assert_eq!(squares.len(), surface_squares().len());
        for (surface, found) in squares {
            let (made, read) = (surface_squares().iter().find(|square| square.surface == *surface).unwrap(), Square::of(*surface, found));
            assert!(made.centre.distance(read.centre) < 1e-4 && made.size.distance(read.size) < 1e-4 && made.yaw == read.yaw, "{made:?} and {read:?}");
        }
        let again = Range::of(Some(Arc::new(level.clone())));
        assert_eq!(again.solids.len(), range.solids.len());
        for (a, b) in range.solids.iter().zip(&again.solids) {
            let colour = a.color.to_srgba().to_u8_array().iter().zip(b.color.to_srgba().to_u8_array()).all(|(x, y)| x.abs_diff(y) <= 1);
            let same = a.centre.distance(b.centre) < 1e-4 && a.size.distance(b.size) < 1e-4 && (a.yaw - b.yaw).abs() < 1e-5;
            assert!(same && colour && a.surface == b.surface && a.overhead == b.overhead, "{} became {}", a.name, b.name);
        }
        assert_eq!(again.site.blocks.len(), range.site.blocks.len());
        for (a, b) in range.site.blocks.iter().zip(&again.site.blocks) {
            let near = (0..3).all(|k| (a.low[k] - b.low[k]).abs() < 1e-4 && (a.high[k] - b.high[k]).abs() < 1e-4);
            assert!(near && (a.turn - b.turn).abs() < 1e-5, "{a:?} became {b:?}");
        }
        for (a, b) in range.things.iter().zip(&again.things) {
            assert!(a.name == b.name && a.place == b.place && a.hit == b.hit && a.thing.hinge == b.thing.hinge);
        }
        for name in start_names() {
            let (a, b) = (range.place(&name).unwrap(), again.place(&name).unwrap());
            assert!((0..3).all(|k| (a[k] - b[k]).abs() < 1e-4), "{name}: {a:?} and {b:?}");
            let (a, b) = (range.start_view(&name).unwrap(), again.start_view(&name).unwrap());
            assert!(((a[0] - b[0] + 180.0).rem_euclid(360.0) - 180.0).abs() < 1e-3 && a[1] == b[1], "{name}: {a:?} and {b:?}");
        }

        // The rifles' table 6 m down the lanes and a quarter turned, its
        // start as it would go with it.
        let prop = range.props.iter().find(|prop| prop.name == "rifles table").unwrap();
        let anchor = prop.stand.at;
        let stand = Stand { at: anchor + Vec3::new(0.0, 0.0, -6.0), yaw: std::f32::consts::FRAC_PI_2 };
        let mut moved = level;
        moved.props.insert("rifles table".to_owned(), stand);
        moved.starts.remove("rifles");
        let turned = Range::of(Some(Arc::new(moved)));
        for at in prop.solids.clone() {
            let (was, now) = (&range.solids[at], &turned.solids[at]);
            assert!(now.centre.distance(stand.point(anchor, was.centre)) < 1e-4 && (now.yaw - stand.yaw).abs() < 1e-6);
        }
        for at in prop.things.clone() {
            let (was, now) = (&range.things[at], &turned.things[at]);
            let middle = |block: &Block| in_bevy(std::array::from_fn(|k| (block.low[k] + block.high[k]) / 2.0));
            assert!(middle(&now.hit).distance(stand.point(anchor, middle(&was.hit))) < 1e-4 && (now.hit.turn - (was.hit.turn - stand.yaw)).abs() < 1e-6);
        }
        let (was, now) = (range.place("rifles").unwrap(), turned.place("rifles").unwrap());
        assert!(in_bevy(now).distance(stand.point(anchor, in_bevy(was))) < 1e-4, "{was:?} to {now:?}");
        assert!((turned.start_view("rifles").unwrap()[0] - (range.start_view("rifles").unwrap()[0] - 90.0)).abs() < 1e-3);
        // The table's top is stood on where it now is, and not where it was.
        let top = stand.point(anchor, anchor);
        assert!(turned.ground_height(top.x, -top.z) >= WEAPON_TABLE.y);
        assert_eq!(turned.ground_height(anchor.x, -anchor.z), 0.0);
    }

    /// The knee-high wall is not walked onto: the capsule's foot meets
    /// its rim too low to be pushed up over it.
    #[test]
    fn the_low_wall_stops_a_walk() {
        let range = Range::default();
        let height = Capsule { underside: 0.0, top: 1.6, lift: true };
        // Ahead of the start, past the tables of weapons to the right of the
        // workbench (along the rest of the wall, the tables stand).
        let mut at = [8.0, 0.0, 2.0];
        for _ in 0..300 {
            let to = range.obstructed(at, [at[0], at[1], at[2] + 0.0235], height);
            at = [to[0], to[1].max(range.ground_height(to[0], to[2])), to[2]];
        }
        let face = -(KNEE_WALL_Z + WALL_THICK / 2.0);
        assert!((at[2] - (face - range.radius)).abs() < 1e-4 && at[1] == 0.0, "{at:?}");
    }

    /// The doors: shut, they stop a walk through the doorway; looked at
    /// from the lane's side within a metre they are offered, and each is
    /// pulled with the hinge to its own hand; opened, the doorway is clear.
    /// A door swung into a character standing in its way pushes it off,
    /// with the cap on speed lifted while the leaf is in it.
    #[test]
    fn the_doors_are_worked_and_walked_through() {
        let mut range = Range::default();
        range.thing_settings = Some(thing_settings());
        let height = Capsule { underside: 0.0, top: 1.6, lift: true };
        let doors: Vec<usize> = (0..range.things.len()).filter(|&i| range.things[i].name == "door").collect();
        assert_eq!(doors.len(), 2);
        let walk = |range: &Range, mut at: [f32; 3], step: [f32; 2], frames: usize| {
            for _ in 0..frames {
                at = range.obstructed(at, [at[0] + step[0], at[1], at[2] + step[1]], height);
            }
            at
        };
        // Through the middle of the doorway from the start's side (towards
        // -z in the simulation's axes): stopped the radius off the shut
        // leaves' face.
        let start = range.place("doors").unwrap();
        let stopped = walk(&range, start, [0.0, -0.0235], 200);
        let face = DOOR_WALL_Z - WALL_THICK / 2.0;
        assert!((stopped[2] - (-face + range.radius)).abs() < 1e-3, "{stopped:?}");
        // Looked at from 0.9 m (the eye at 1.6 m, level): offered; from
        // 1.2 m, out of a door's reach.
        let eye = |x: f32, z: f32| Vec3::new(x, 1.6, z);
        let across = (DOORWAY[0] + DOORWAY[1]) / 2.0 + 0.4;
        let seen = range.looked_at(eye(across, face - 0.9 - 0.15), eye(across, face + 0.4), 1.15);
        assert!(seen.is_some_and(|i| doors.contains(&i)), "{seen:?}");
        let seen = range.looked_at(eye(across, face - 1.2 - 0.15), eye(across, face + 0.1), 1.15);
        assert_eq!(seen, None);
        // Pulled, the one hinged nearer the start with one hand and the
        // other with the other.
        let gestures: Vec<u8> = doors.iter().map(|&i| range.things[i].thing.door_gesture(start)).collect();
        assert_eq!(gestures.iter().copied().collect::<std::collections::BTreeSet<_>>(), [24, 26].into());
        // From beyond, pushed.
        let beyond = range.place("yard").unwrap();
        assert!(doors.iter().all(|&i| range.things[i].thing.door_gesture(beyond) == 27));
        // Both opened; 3 s on they are open and the doorway is clear.
        let settings = thing_settings();
        for &i in &doors {
            range.things[i].thing.work(true, start, &settings);
        }
        for _ in 0..400 {
            range.step_things(1.0 / 120.0);
        }
        assert!(doors.iter().all(|&i| range.things[i].thing.state() == ThingState::Open));
        let through = walk(&range, start, [0.0, -0.0235], 200);
        assert!(through[2] < -(DOOR_WALL_Z + 2.0), "{through:?}");
        // Shut on a character standing where the first door's leaf will
        // pass: it is pushed out of the leaf's way, the cap lifted.
        let door = doors[0];
        let hinge = range.things[door].thing.hinge.unwrap();
        let opening = turn(hinge.forward, hinge.axis, 90.0);
        let mut standing: [f32; 3] = std::array::from_fn(|k| hinge.at[k] + hinge.forward[k] * 0.25 + opening[k] * 0.55);
        let first = standing;
        range.things[door].thing.work(false, standing, &settings);
        let mut lifted = false;
        for _ in 0..400 {
            range.step_things(1.0 / 120.0);
            lifted |= range.lifts_cap(standing, height);
            standing = range.obstructed(standing, standing, height);
        }
        println!("a door shut on a character standing at {first:?}: pushed to {standing:?}");
        assert!(lifted && range.things[door].thing.state() == ThingState::Shut);
        assert!(!range.site.blocks[range.things[door].leaf.unwrap()].overlaps(standing, height, range.radius), "{standing:?}");
    }

    /// The containers and the trunk each name the gestures recorded for
    /// their kind, and are looked at by their bodies.
    #[test]
    fn the_containers_ask_for_their_own_gestures() {
        let range = Range::default();
        let gesture = |name: &str| {
            let placed = range.things.iter().find(|p| p.name == name).unwrap();
            placed.thing.gesture([0.0; 3]).map(Interaction::index)
        };
        assert_eq!(gesture("wooden crate"), Some(29));
        assert_eq!(gesture("technical supply crate"), Some(29));
        assert_eq!(gesture("dead Scav"), Some(21));
        assert_eq!(gesture("drawer"), Some(25));
        assert_eq!(gesture("rigs drawer"), Some(25));
        assert_eq!(gesture("plates box"), Some(29));
        assert_eq!(gesture("eyewear dresser"), Some(25));
        // Each place of the storage is looked at from where its start puts
        // the eye, at its drawer or its lid, within the game's reach of
        // 1.3 m from the eye.
        for (index, label) in CABINETS.iter().enumerate() {
            let name = label.to_lowercase().replace(' ', "-");
            let feet = range.place(&name).unwrap();
            let eye = in_bevy(feet) + Vec3::Y * 1.6;
            let [x, z] = cabinet_at(index);
            let furniture = Furniture::of(label);
            let size = furniture.size();
            let target = match furniture {
                Furniture::Box => Vec3::new(x + size.z / 2.0 - 0.05, size.y, z),
                _ => Vec3::new(x + size.z / 2.0, furniture.drawer_height(), z),
            };
            assert!(eye.distance(target) < 1.3, "{label}: {:.2} m from the eye", eye.distance(target));
            let seen = range.looked_at(eye, eye + (target - eye) * 1.2, 1.15);
            assert_eq!(seen.map(|i| range.things[i].name), Some(format!("{} {}", label.to_lowercase(), furniture.word()).as_str()), "{label}");
        }
        assert_eq!(gesture("trunk"), Some(27));
        // The car's trunk seen from behind it, a metre off.
        let rear = CAR[0] - 2.1;
        let seen = range.looked_at(Vec3::new(rear - 1.0, 1.5, CAR[1]), Vec3::new(rear + 0.2, 0.6, CAR[1]), 1.15);
        assert_eq!(seen.map(|i| range.things[i].name), Some("trunk"));
        // Every place before a thing is clear of everything solid.
        let height = Capsule { underside: 0.0, top: 1.6, lift: true };
        for name in ["doors", "yard", "window", "workbench", "car", "wooden-crate", "supply-crate", "scav", "drawer", "table", "key-table", "high-shelf", "floor", "stairs", "rifles", "custom", "wire", "rigs", "plates", "headsets", "eyewear", "surfaces", "platform", "shelf", "launchers"] {
            let at = range.place(name).unwrap();
            assert_eq!(range.obstructed(at, at, height), at, "{name} stands in something");
        }
    }

    /// The range is open to the sky (since 0.100.0, the user's): nowhere is
    /// indoors, nothing is overhead, and the morning sun's line from high
    /// over each start comes down to the ground or what stands there, with
    /// no roof or wall of a hall in its way.
    #[test]
    fn the_range_is_open_to_the_sky() {
        let range = Range::default();
        assert!(indoors().is_none() && hall_lamps().is_empty());
        assert!(!range.solids.iter().any(|solid| solid.overhead || solid.name == "hall wall" || solid.name == "roof"));
        // The way Customs' recorded morning's light goes (Bevy's axes).
        let light = Vec3::new(0.845, -0.525, 0.098).normalize();
        for name in ["rifles", "rigs", "eyewear", "lanes", "dummy"] {
            let at = in_bevy(range.place(name).unwrap());
            assert!(!in_hall(at + Vec3::Y), "{name} is indoors");
            let from = at + Vec3::Y * 0.1 - light * 20.0;
            let hit = range.entered(in_sim(from), in_sim(at + Vec3::Y * 0.1)).map(|share| from + (at + Vec3::Y * 0.1 - from) * share);
            assert!(hit.is_none_or(|hit| hit.y < 2.6), "the sun's line to {name} meets something at {hit:?}");
        }
    }

    /// The window is vaulted through by the game's own rules (the
    /// simulation's measuring and choosing, with the install's settings):
    /// from the lane's side, its sill is an obstacle 0.9 m high and as deep
    /// as the wall, and the move is a vault; before the wall beside it,
    /// nothing is.
    #[test]
    fn the_window_is_vaulted_through() {
        use trp_sim::vaulting::{Move, Standing, Vaulting, VaultingSettings};
        let Ok(install) = trp_data::Install::locate() else {
            eprintln!("skipped: no install configured (set EFT_DIR or local/eft_dir.txt)");
            return;
        };
        let db = trp_data::Database::load(&install).expect("database");
        let settings = VaultingSettings::new(&db.config).expect("vaulting settings");
        let range = Range::default();
        let standing = Standing { vaulting: false, backing: false, grounded: true, in_transition: false, jumping: false, sprinting: false, can_vault: true, can_climb: true };
        // Standing 0.45 m before the wall's face on the lane's side (in the
        // simulation's axes the wall is at -6 m), facing it: half round.
        let try_at = |x: f32| {
            let feet = [x, 0.0, -(DOOR_WALL_Z - WALL_THICK / 2.0) + 0.45];
            let mut vaulting = Vaulting::default();
            vaulting.measure(&settings, &range, feet, 180.0, 0.0, false);
            vaulting.measure(&settings, &range, feet, 180.0, 0.0, false);
            (vaulting.choose(&settings, &standing), vaulting.obstacle.height, vaulting.obstacle.length)
        };
        let (chosen, height, length) = try_at((WINDOW[0] + WINDOW[1]) / 2.0);
        println!("before the window: {chosen:?}, an obstacle {height:.2} m high and {length:.2} m deep");
        assert_eq!(chosen, Some(Move::Vault));
        assert!((height - SILL).abs() < 0.11 && length <= 0.41, "{height} {length}");
        let (beside, ..) = try_at(WINDOW[1] + 0.5);
        assert_eq!(beside, None);
    }

    /// The window's sill takes a forward mount from the window's start
    /// (mounting spec, "The search"): a character 1.1 m before the wall's
    /// face, the grid's root at the hands, crouched (the root 0.87 m up, as
    /// the app's body has it) or half way up, finds the sill's top on the
    /// second frame, 2 cm over it, and its face turned round. Standing (the
    /// root 1.41 m up) the search's band, the root's height less half a
    /// metre, stops over the 0.9 m sill: refused, by the game's rule.
    #[test]
    fn the_window_sill_takes_a_mount_crouched() {
        let range = Range::default();
        let s = trp_sim::mounting::Settings::default();
        let feet = [(WINDOW[0] + WINDOW[1]) / 2.0, 0.0, -(DOOR_WALL_Z - 1.3)];
        let standing = trp_sim::mounting::Grid { root: [feet[0], 1.41, feet[2] - 0.3], yaw: 180.0, feet: 0.0, column_raise: -0.04, row_raise: -0.04 };
        let mut search = trp_sim::mounting::Search::standing(&range, &s, standing);
        assert_eq!((search.frame(), search.frame(), search.frame()), (None, None, Some(None)), "refused standing");
        for height in [0.87f32, 1.0, 1.2, 1.3] {
            let grid = trp_sim::mounting::Grid { root: [feet[0], height, feet[2] - 0.3], yaw: 180.0, feet: 0.0, column_raise: -0.04, row_raise: -0.04 };
            let mut search = trp_sim::mounting::Search::standing(&range, &s, grid);
            let first = search.frame();
            let point = search.frame().flatten();
            println!("root at {height}: first frame {first:?}, then {point:?}");
            assert_eq!(first, None);
            let point = point.expect("found on the second frame");
            assert!((point.at[1] - (SILL + 0.02)).abs() < 1e-3, "{point:?}");
            assert!((point.direction[2] + 1.0).abs() < 1e-4, "{point:?}");
        }
    }

    /// The workbench is in sight before its table, and no longer beside it:
    /// its triggers end at the table's ends (the user's cut), where the
    /// game's reached 0.6 and 0.8 m past them.
    #[test]
    fn the_workbench_is_in_sight_only_before_its_table() {
        let bench = Range::default().workbench.expect("the workbench stands");
        let (a, b) = (bench_point([6.2, BENCH_FLOOR, BENCH_ENDS[0]]), bench_point([6.2, BENCH_FLOOR, BENCH_ENDS[1]]));
        let (middle, along) = ((a + b) / 2.0, (b - a).normalize());
        let radius = 0.366;
        let feet = |past: f32| Vec3::new(middle.x, 0.0, middle.z) + along * ((b - a).length() / 2.0 + past);
        assert!(bench.in_sight(feet(-0.66), radius), "before the middle");
        assert!(bench.in_sight(feet(-0.1), radius), "before its end");
        assert!(!bench.in_sight(feet(0.4 + radius), radius) && !bench.in_sight(feet(-(b - a).length() - 0.4 - radius), radius), "beside either end");
        // The game's own reached there.
        let game = BENCH_TRIGGERS_GAME.map(|(low, high)| {
            let (p, q) = (bench_point(low), bench_point(high));
            [p.min(q), p.max(q)]
        });
        let past = Workbench { place: bench.place, triggers: game };
        assert!(past.in_sight(feet(0.4 + radius), radius) || past.in_sight(feet(-(b - a).length() - 0.4 - radius), radius));
    }

    /// The things of the row stand a pace apart and none in another; the
    /// tables of weapons' cases and boxes are clear of one another, a case
    /// over its own boxes, a table's two boxes side by side under it; the
    /// lower platform reaches to the knee-high wall;
    /// the workbench's table is against that wall, short of the first
    /// table; and the ring is clear of everything.
    #[test]
    fn the_range_is_laid_out_as_drawn() {
        let range = Range::default();
        let on_tables = |placed: &Placed| placed.holds.is_some_and(|holds| holds >= GUN_CASE_HOLDS);
        // A table's magazine box and its box of rounds, by its place.
        let box_of = |placed: &Placed| {
            placed.holds.and_then(|holds| {
                let magazines = holds.checked_sub(MAGAZINE_BOX_HOLDS).filter(|at| *at < AMMO_TABLES);
                magazines.or_else(|| holds.checked_sub(AMMO_BOX_HOLDS).filter(|at| *at < AMMO_TABLES))
            })
        };
        let things: Vec<(Block, bool, Option<usize>)> = range.things.iter().filter(|p| p.leaf.is_none()).map(|p| (p.hit, on_tables(p), box_of(p))).collect();
        for (i, (a, a_table, a_box)) in things.iter().enumerate() {
            for (b, b_table, b_box) in &things[i + 1..] {
                let apart = |k: usize| (a.low[k] - b.high[k]).max(b.low[k] - a.high[k]);
                let least = if *a_table && *b_table { 0.2 } else { 0.5 };
                assert!(apart(0).max(apart(1)).max(apart(2)) > 0.0, "{a:?} and {b:?} are in one another");
                // (A table's two boxes: side by side, apart all the same.)
                if a_box.is_some() && a_box == b_box {
                    continue;
                }
                // (A case and its box: one over the other.)
                if !(*a_table && *b_table && apart(1) > 0.3) {
                    assert!(apart(0).max(apart(2)) >= least, "{a:?} and {b:?} are under {least} m apart");
                }
            }
        }
        // The lower platform's left side is the platform's right side; the
        // platform's left side the stairs' (the user's range of 2026-10-26).
        let shelf = range.solids.iter().find(|s| s.size.distance(Vec3::new(3.0, SHELF, 5.0)) < 1e-4).unwrap();
        assert!((shelf.centre.x - shelf.size.x / 2.0 - HALL_LEFT).abs() < 1e-5);
        let platform = range.solids.iter().find(|s| s.size.distance(Vec3::new(5.0, PLATFORM, 5.0)) < 1e-4).unwrap();
        assert!((platform.centre.x - platform.size.x / 2.0 - STAIRS_LEFT).abs() < 1e-5);
        let board = range.solids.iter().find(|s| s.name == "workbench").expect("the workbench's table");
        assert!((board.centre.z - board.size.z / 2.0 - (KNEE_WALL_Z + WALL_THICK / 2.0)).abs() < 1e-4, "{:?} {:?}", board.centre, board.size);
        let ([first_table, _], _) = weapon_table(0);
        assert!(board.centre.x + board.size.x / 2.0 < first_table - WEAPON_TABLE.x / 2.0, "{:?} {:?}", board.centre, board.size);
        // Each block of the ring 3 m clear of everything else's footprint.
        let ring = &range.solids[..12];
        for solid in &range.solids[12..] {
            for block in ring {
                let off = (Vec2::new(block.centre.x - solid.centre.x, block.centre.z - solid.centre.z).abs() - Vec2::new(solid.size.x, solid.size.z) / 2.0).max(Vec2::ZERO);
                assert!(off.length() > 3.0, "a block of the ring is {:.1} m from {:?}", off.length(), solid.centre);
            }
        }
    }
}
