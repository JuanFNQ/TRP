//! Solids, surfaces, and the walls', windows' and crates' measures.

use super::*;

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
pub(super) const BULLET_LEAVES: u64 = 1 << 20;

pub(super) const BULLET_GROUND: u64 = 1 << 21;

/// The thing whose moving part a bullet's thing is (a door's leaf), by its
/// place among the range's things.
pub fn bullet_thing_part(thing: u64) -> Option<usize> {
    (BULLET_LEAVES..BULLET_GROUND).contains(&thing).then(|| (thing - BULLET_LEAVES) as usize)
}

/// The surfaces the range's boxes are made of (`ESurfaceSound`); anything
/// not said is concrete, the game's surface for a thing without one.
pub(super) const METAL: u8 = 1;

pub(super) const WOOD: u8 = 2;

pub(super) const PLASTIC: u8 = 16;

pub const METAL_SURFACE: u8 = METAL;

/// The sampler of surfaces on the ground: a course outside the hall's wide
/// opening (laid out in Blender, the range replaced by it): a strip for each of the game's
/// twenty surfaces, by number, its middle in Bevy's x and z (`COURSE`), in
/// two rows across the right of the hall, each strip `STRIP` across and
/// along (drawn 4 cm smaller, as they were laid out: 1.76 by 6 m). The
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
    pub fn of(surface: u8, found: &crate::world::level::LevelBox) -> Self {
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
        if let Some(squares) = crate::world::level::get().and_then(|level| level.squares.as_ref()) {
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
pub(super) fn ground_surface(x: f32, z: f32) -> u8 {
    surface_squares().iter().find(|square| square.covers(x, z)).map_or(trp_sim::sound::steps::CONCRETE, |square| square.surface)
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
    pub(super) hit: Block,
    /// Its place in the simulation's axes, which a search measures the
    /// character's distance from (`transform.position`).
    pub place: [f32; 3],
    /// The container's own item, by its place in `Carried::away`.
    pub holds: Option<usize>,
    /// A door's leaf among the site's blocks.
    pub(super) leaf: Option<usize>,
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
pub(super) const LEAF: [f32; 3] = [0.9, 2.0, 0.04];

/// The two walls across the lane, in Bevy's axes (x to the right of the
/// start, z behind it), each running left to right and centred on the
/// lane: the door wall behind the start, high, with a doorway in its middle
/// for two doors (from one hinge to the other, across); and the knee-high
/// wall ahead of it. Their thickness, the door wall's height, and how far
/// each reaches to either side of the lane.
pub(super) const DOOR_WALL_Z: f32 = 6.0;

pub const KNEE_WALL_Z: f32 = -5.6;

pub(super) const WALL_THICK: f32 = 0.4;

/// (The door wall rose to the hall's 8 m with the hall, 0.76.0 to 0.99.0;
/// with the hall gone it is as before it, 2.5 m.)
pub(super) const WALL_HIGH: f32 = 2.5;

pub(super) const KNEE_HIGH: f32 = 0.6;

pub(super) const WALL_HALF: f32 = 4.0;

pub(super) const DOORWAY: [f32; 2] = [-0.9, 0.9];

/// How far the knee-high wall reaches to either side of the lane: further
/// than the door wall, for the tables of weapons along it on both sides of
/// the workbench (TRP's range, drawn over the map: the
/// wall longer to the left, the arm of tables gone).
pub(super) const KNEE_LEFT: f32 = -9.2;

pub(super) const KNEE_RIGHT: f32 = 10.5;

/// A window in the door wall to the right of the doorway, open, to vault
/// through: from one side to the other across the lane, and its sill's
/// and its lintel's heights. Made to the vault's rules (`docs/spec/
/// vaulting.md`): a sill of 0.9 m is within the 0.25 to 1.1 m a vault
/// takes, the wall's 0.4 m within its 0.41 m of length, the 1 m opening
/// more than the 0.9 m of roof it asks over the sill, and the capsule a
/// vault shrinks to (1.0 to 1.73 m over the feet for this sill, 0.73 m
/// across) passes under the lintel and between the jambs.
pub(super) const WINDOW: [f32; 2] = [2.0, 3.0];

pub(super) const SILL: f32 = 0.9;

pub(super) const LINTEL: f32 = 1.9;

/// The tables of weapons along the knee-high wall, to the right of the
/// airdrop crate, one for each class of weapon, each with
/// its label drawn over it. The three classes the first five leave
/// without a place (submachine guns, machine guns, launchers) are the
/// app's. `main` fills them in this order: the gun cases are the range's
/// containers from `CONTAINERS`' last on, the magazine boxes after them.
pub const WEAPON_CRATES: [&str; 10] = ["RIFLES", "SMGS", "MACHINE GUNS", "SNIPERS", "SHOTGUNS", "PISTOLS", "LAUNCHERS", "CUSTOM", MEDICAL, GRENADES];

/// The last table holds no weapons: every medical item of the install in
/// its case, and every food and drink in its box under it (the case
/// `MEDICAL` and the box `PROVISIONS`). The app's.
pub const MEDICAL: &str = "MEDICAL";

pub const PROVISIONS: &str = "PROVISIONS";

/// And after it, every grenade of the install in its case, and every melee
/// weapon in its box (the app's, for there to be any to take: the starting
/// kit has a knife and no grenade).
pub const GRENADES: &str = "GRENADES";

pub const MELEE: &str = "MELEE";

/// Whether a table holds no weapons of a class, but what its two names say.
pub(super) fn of_its_own(name: &str) -> bool {
    name == MEDICAL || name == GRENADES
}

/// A table's box's name: the class's, or its own.
pub(super) fn box_name(name: &'static str) -> &'static str {
    match name {
        MEDICAL => PROVISIONS,
        GRENADES => MELEE,
        other => other,
    }
}

/// Barbed wire across the ground past the knee-high wall's left end, its
/// lowest and highest corners in Bevy's axes. It is no solid: the character walks into
/// it, slowed and cut (health spec, "Barbed wire"). The game's coils have
/// their boxes in the maps' scenes; this one's shape and place are the
/// app's.
pub const BARBED_WIRE: [Vec3; 2] = [Vec3::new(KNEE_LEFT - 2.0, 0.0, -6.5), Vec3::new(KNEE_LEFT, 0.7, -5.5)];

/// The storage cabinets against the staircase's side, where the wire was
/// seen from the start: each a cabinet with a drawer as the row's, but the
/// plates' box, with a lid, and the eyewear's dresser, tall, with its
/// drawer near its top (`Furniture`); its back to the stairs and its front
/// to the lane, from the start's side on in TRP's order, the plates
/// beside the armour; each label drawn over it. `main` fills them, after
/// the tables' magazine boxes. The app's.
pub const CABINETS: [&str; 8] = ["RIGS", "BACKPACKS", "HELMETS", "ARMOR", "PLATES", "MASKS AND HATS", "HEADSETS", "EYEWEAR"];

/// The plates' box and the eyewear's dresser among them.
pub const PLATES: &str = "PLATES";

pub const EYEWEAR: &str = "EYEWEAR";
