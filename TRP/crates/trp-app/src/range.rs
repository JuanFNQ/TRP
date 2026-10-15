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
//! technical supply crate, a dead Scav, a drawer), beside the airdrop crate
//! with the rifle's kit. Their shapes and places are the app's; what each
//! asks of the hand and how it opens are the recorded things'.

use bevy::prelude::*;
use trp_sim::World;
use trp_sim::collision::{Block, Site};
use trp_sim::movement::Capsule;
use trp_sim::things::{turn, Hinge, Thing, ThingSettings};
use trp_sim::Interaction;

/// A box, upright and square to the axes, in Bevy's axes.
#[derive(Clone, Copy)]
pub struct Solid {
    pub centre: Vec3,
    pub size: Vec3,
    pub color: Color,
}

/// A thing of the range the hands work: the game's model of it, the name
/// the prompt gives it, what of it moves, and what a line of sight meets.
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
    /// `size` about `centre`, turned by the thing's angle about the line
    /// through `pivot` along `axis` (a positive angle turns it the right
    /// hand's way about `axis`), and moved by `slide` as far as it is out.
    Moving { centre: Vec3, size: Vec3, pivot: Vec3, axis: Vec3, slide: Vec3, color: Color },
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
const KNEE_WALL_Z: f32 = -5.6;
const WALL_THICK: f32 = 0.4;
const WALL_HIGH: f32 = 2.5;
const KNEE_HIGH: f32 = 0.6;
const WALL_HALF: f32 = 4.0;
const DOORWAY: [f32; 2] = [-0.9, 0.9];
/// How far the knee-high wall reaches to the right of the lane: further
/// than the door wall, for the tables of weapons along it.
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
pub const WEAPON_CRATES: [&str; 8] = ["RIFLES", "SMGS", "MACHINE GUNS", "SNIPERS", "SHOTGUNS", "PISTOLS", "LAUNCHERS", "CUSTOM"];
/// Where the first table stands across the lane (its gun case half a metre
/// from the airdrop crate), and how far each next one is from the last.
const WEAPON_TABLE_FIRST: f32 = 1.5;
const WEAPON_TABLE_STEP: f32 = 1.2;
/// How many tables stand along the wall; the rest stand in an arm turned
/// towards the start, the whole an L (the user, 2026-10-14, night: "Rotate
/// the last 4 weapon sets 90* over to the right to form an L shape"),
/// their fronts to the lane and their backs to the right. The arm's tables'
/// middle across the lane, and how far its first one's near end is from
/// the wall's face: a character before it stands clear of the row's last
/// table. The app's.
const ALONG_THE_WALL: usize = 4;
const ARM_X: f32 = 6.8;
const ARM_FROM_WALL: f32 = 0.4;
/// The airdrop crate's lid: hinged along its back edge, by the wall, and
/// lifted towards it so far (`ESTIMATE`: its prefab is not read), as the
/// tables' boxes are.
const AIRDROP_OPENS: f32 = 80.0;
/// What the airdrop crate holds since the tables took the weapons (the
/// user, 2026-10-14, night: 'Give the OG green box a "clothing" label and
/// a lid'): its label, over it and on the screen.
pub const AIRDROP_LABEL: &str = "CLOTHING";
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

#[derive(Resource)]
pub struct Range {
    pub solids: Vec<Solid>,
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
/// lane's way up to the platform, and the lower platform on the
/// platform's right reaches to the knee-high wall's left end.
const STAIRS_LEFT: f32 = -WALL_HALF - 8.0;
const STAIRS_FOOT: f32 = 4.4;
/// How far out the ring of blocks stands, all round the start.
const RING: f32 = 18.0;

/// Where each of the range's things stands: the middle of its footprint,
/// in Bevy's axes. The airdrop crate is against the knee-high wall ahead
/// of the start; the rest stand shoulder to shoulder in a row behind the
/// door wall, facing it, with the car at the row's right end, its trunk to
/// the row. The block to climb is off to the right.
const AIRDROP: [f32; 2] = [0.0, KNEE_WALL_Z + WALL_THICK / 2.0 + 0.3];
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
/// Where `main` lays the things to be taken up, in Bevy's axes: on the
/// table, on the floor behind the door wall, on the shelf.
pub const LYING: [[f32; 3]; 3] =
    [[TABLE[0], TABLE_TOP, TABLE[1]], [ROUNDS[0], 0.0, ROUNDS[1]], [HIGH_SHELF[0], SHELF_TOP, HIGH_SHELF[1]]];

/// A point or a way of the simulation (z ahead) in Bevy's axes (z behind),
/// and back: the same flip.
fn in_bevy(v: [f32; 3]) -> Vec3 {
    Vec3::new(v[0], v[1], -v[2])
}

fn in_sim(v: Vec3) -> [f32; 3] {
    [v.x, v.y, -v.z]
}

impl Default for Range {
    fn default() -> Self {
        let mut solids = Vec::new();
        let block = |centre: Vec3, size: Vec3, color: Color| Solid { centre, size, color };
        // A ring of blocks all round, to judge turning against.
        for i in 0..12 {
            let angle = i as f32 / 12.0 * std::f32::consts::TAU;
            solids.push(block(
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
        let grey = Color::srgb(0.55, 0.55, 0.58);
        let span = |z: f32, left: f32, right: f32, low: f32, high: f32| {
            block(Vec3::new((left + right) / 2.0, (low + high) / 2.0, z), Vec3::new(right - left, high - low, WALL_THICK), grey)
        };
        solids.push(span(DOOR_WALL_Z, -WALL_HALF, DOORWAY[0], 0.0, WALL_HIGH));
        solids.push(span(DOOR_WALL_Z, DOORWAY[1], WINDOW[0], 0.0, WALL_HIGH));
        solids.push(span(DOOR_WALL_Z, WINDOW[0], WINDOW[1], 0.0, SILL));
        solids.push(span(DOOR_WALL_Z, WINDOW[0], WINDOW[1], LINTEL, WALL_HIGH));
        solids.push(span(DOOR_WALL_Z, WINDOW[1], WALL_HALF, 0.0, WALL_HIGH));
        solids.push(span(DOOR_WALL_Z, DOORWAY[0], DOORWAY[1], LEAF[1] + 0.05, WALL_HIGH));
        solids.push(span(KNEE_WALL_Z, -WALL_HALF, KNEE_RIGHT, 0.0, KNEE_HIGH));
        // Somewhere to fall from, to the left of the start: stairs going the
        // lane's way up to a platform, and a lower platform beside it that
        // reaches to the knee-high wall.
        let floor = |low: Vec3, high: Vec3, color: Color| Solid { centre: (low + high) / 2.0, size: high - low, color };
        let (left, stair_right, tread) = (STAIRS_LEFT, STAIRS_LEFT + 2.5, TREAD);
        for i in 0..STAIRS {
            let top = PLATFORM * (i + 1) as f32 / (STAIRS + 1) as f32;
            let near = STAIRS_FOOT - (i as f32) * tread;
            solids.push(floor(
                Vec3::new(left, 0.0, near - tread),
                Vec3::new(stair_right, top, near),
                Color::srgb(0.50, 0.52, 0.47 + 0.01 * (i % 2) as f32),
            ));
        }
        let far = STAIRS_FOOT - (STAIRS as f32) * tread;
        solids.push(floor(Vec3::new(left, 0.0, far - 5.0), Vec3::new(left + 5.0, PLATFORM, far), Color::srgb(0.45, 0.47, 0.50)));
        solids.push(floor(Vec3::new(left + 5.0, 0.0, far - 5.0), Vec3::new(left + 8.0, SHELF, far), Color::srgb(0.42, 0.50, 0.45)));

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
            pivot: Vec3::new(at[0], size.y + 0.02, at[1] + size.z / 2.0),
            axis,
            slide: Vec3::ZERO,
            color,
        };
        // The airdrop crate with the kit, against the knee-high wall ahead
        // of the start: since 2026-10-14, night, a lid on it hinged along its
        // back edge, which opens with the gestures recorded for a lid hinged
        // up (PROVISIONAL: its prefab, and whatever gestures that names, are
        // not read).
        let airdrop_size = Vec3::new(1.0, 0.5, 0.6);
        let airdrop_green = Color::srgb(0.27, 0.33, 0.21);
        let airdrop = body(&mut solids, AIRDROP, airdrop_size, airdrop_green);
        let airdrop_opens = Thing::container(Some(Interaction::ContainerHingeUpOpen), Some(Interaction::ContainerHingeUpClose), 0.0, -AIRDROP_OPENS);
        let airdrop_lid = lid_of(Vec3::new(AIRDROP[0], airdrop_size.y, AIRDROP[1]), airdrop_size, Vec3::Z, airdrop_green.darker(0.04));
        things.push(container("airdrop crate", airdrop_opens, airdrop, airdrop_lid, 0));
        // Behind the door wall, from left to right. A technical supply crate,
        // its lid at waist height (recorded: gestures 29 and 30, the lid from
        // 0 to 4.78 degrees).
        let wood = Color::srgb(0.47, 0.36, 0.22);
        let green = Color::srgb(0.28, 0.34, 0.26);
        let hinged_up = || Thing::container(Some(Interaction::ContainerHingeUpOpen), Some(Interaction::ContainerHingeUpClose), 0.0, -30.0);
        let supply_size = Vec3::new(0.55, 0.84, 0.9);
        let supply = body(&mut solids, SUPPLY_CRATE, supply_size, green);
        let mut tech = hinged_up();
        tech.open_angle = 4.78;
        things.push(container("technical supply crate", tech, supply, lid(SUPPLY_CRATE, supply_size, Vec3::X, green.darker(0.05)), 2));
        // A drawer in a cabinet, its front to the door wall (recorded:
        // pulled open with 25 and pushed shut with a door's 20; it slides).
        let cabinet = body(&mut solids, DRAWER, Vec3::new(0.8, 0.8, 0.5), Color::srgb(0.40, 0.38, 0.33));
        let front = Vec3::new(DRAWER[0], 0.64, DRAWER[1] - 0.03);
        let drawer_front = Part::Moving {
            centre: front,
            size: Vec3::new(0.7, 0.15, 0.5),
            pivot: front,
            axis: Vec3::Y,
            // ESTIMATE: how far the drawer comes out (its prefab's open
            // place is not read).
            slide: Vec3::new(0.0, 0.0, -0.35),
            color: Color::srgb(0.46, 0.43, 0.37),
        };
        let drawer = Thing::container(Some(Interaction::ContainerOpenPull), Some(Interaction::DoorPushForward), 0.0, 0.0);
        things.push(container("drawer", drawer, cabinet, drawer_front, 4));
        // A dead Scav lying along the row (recorded: opened with a door's
        // gesture, 21, and shut with the default, 28; nothing moves). It is
        // 40 cm high so that it is in reach standing, as the recorded one
        // was (the game's reach is 1.3 m from the eye).
        let scav = body(&mut solids, SCAV, Vec3::new(1.7, 0.4, 0.5), Color::srgb(0.30, 0.31, 0.27));
        let dead = Thing::container(Some(Interaction::DoorPullBackward), Some(Interaction::ContainerCloseDefault), 0.0, 0.0);
        things.push(container("dead Scav", dead, scav, Part::None, 3));
        // A wooden crate on the floor, its lid hinged along its far edge
        // (recorded on the floor: gestures 29 and 30, the lid from 0 to -30
        // degrees).
        let crate_size = Vec3::new(1.0, 0.44, 0.6);
        let crate_body = body(&mut solids, WOODEN_CRATE, crate_size, wood);
        things.push(container("wooden crate", hinged_up(), crate_body, lid(WOODEN_CRATE, crate_size, Vec3::NEG_X, wood.darker(0.05)), 1));
        // The tables of weapons, against the knee-high wall to the airdrop
        // crate's right: on each a green gun case with the class's weapons,
        // under it a black box with what loads them, its front with the
        // table's. Each opens as the range's crates do (the gestures
        // recorded for a lid hinged up, 29 and 30), its lid hinged along
        // its back edge, by the wall, and lifted towards it.
        let (case_green, box_black) = (Color::srgb(0.24, 0.33, 0.19), Color::srgb(0.05, 0.05, 0.06));
        for (index, name) in WEAPON_CRATES.into_iter().enumerate() {
            let ([x, z], front) = weapon_table(index);
            let (along, middle) = (Vec3::Y.cross(front), Vec3::new(x, 0.0, z));
            // A box's size in the range's axes, from its length along the
            // table, its height and its depth from back to front.
            let sized = |size: Vec3| along.abs() * size.x + Vec3::Y * size.y + front.abs() * size.z;
            // The table: its top, and a leg at each corner.
            solids.push(block(middle + Vec3::Y * (WEAPON_TABLE.y - TABLE_SLAB / 2.0), sized(Vec3::new(WEAPON_TABLE.x, TABLE_SLAB, WEAPON_TABLE.z)), wood));
            for (across, out) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                let leg = middle + along * across * (WEAPON_TABLE.x - TABLE_LEG) / 2.0 + front * out * (WEAPON_TABLE.z - TABLE_LEG) / 2.0 + Vec3::Y * (WEAPON_TABLE.y - TABLE_SLAB) / 2.0;
                solids.push(block(leg, sized(Vec3::new(TABLE_LEG, WEAPON_TABLE.y - TABLE_SLAB, TABLE_LEG)), wood));
            }
            // (Each thing's name is the prompt's, made once for the run.)
            let named = |what: &str| -> &'static str { Box::leak(format!("{} {what}", name.to_lowercase()).into_boxed_str()) };
            let case = block(middle + Vec3::Y * (WEAPON_TABLE.y + GUN_CASE.y / 2.0), sized(GUN_CASE), case_green);
            solids.push(case);
            let mut opens = hinged_up();
            opens.open_angle = -GUN_CASE_OPENS;
            let lid = lid_of(middle + Vec3::Y * (WEAPON_TABLE.y + GUN_CASE.y), GUN_CASE, front, case_green.darker(0.04));
            things.push(container(named("weapon case"), opens, case, lid, GUN_CASE_HOLDS + index));
            // The box under it, its front with the table's.
            let below = middle + front * (WEAPON_TABLE.z - MAGAZINE_BOX.z) / 2.0;
            let magazines = block(below + Vec3::Y * MAGAZINE_BOX.y / 2.0, sized(MAGAZINE_BOX), box_black);
            solids.push(magazines);
            let mut opens = hinged_up();
            opens.open_angle = -MAGAZINE_BOX_OPENS;
            let lid = lid_of(below + Vec3::Y * MAGAZINE_BOX.y, MAGAZINE_BOX, front, box_black.lighter(0.04));
            things.push(container(named("magazine box"), opens, magazines, lid, MAGAZINE_BOX_HOLDS + index));
        }
        // A table and a shelf with things lying on them (`LYING`), and
        // nothing more.
        body(&mut solids, TABLE, Vec3::new(1.0, TABLE_TOP, 0.6), wood);
        body(&mut solids, HIGH_SHELF, Vec3::new(0.9, SHELF_TOP, 0.35), wood);
        // A car at the row's right end, its back to the row: the trunk opens
        // with the default gestures, its lid from 0 to 60 degrees (recorded).
        let red = Color::srgb(0.45, 0.16, 0.14);
        let car = body(&mut solids, CAR, Vec3::new(4.2, 1.0, 1.8), red);
        solids.push(block(Vec3::new(CAR[0] + 0.1, 1.225, CAR[1]), Vec3::new(2.0, 0.45, 1.6), red.darker(0.1)));
        let trunk_lid = Part::Moving {
            centre: Vec3::new(CAR[0] - 1.6, 1.02, CAR[1]),
            size: Vec3::new(1.0, 0.04, 1.6),
            pivot: Vec3::new(CAR[0] - 1.1, 1.02, CAR[1]),
            axis: Vec3::NEG_Z,
            slide: Vec3::ZERO,
            color: red.darker(0.05),
        };
        let trunk_hit = Solid { centre: Vec3::new(CAR[0] - 1.6, 0.52, CAR[1]), size: Vec3::new(1.0, 1.04, 1.8), color: red };
        things.push(Placed {
            name: "trunk",
            thing: Thing::trunk(None, None, 0.0, 60.0),
            part: trunk_lid,
            hit: trunk_hit.block(),
            place: in_sim(car.centre),
            holds: None,
            leaf: None,
        });

        let mut blocks: Vec<Block> = solids.iter().map(Solid::block).collect();
        // Two doors in the doorway, hinged at its two ends and meeting in
        // its middle, each turning 110 degrees towards the start (recorded:
        // the doors' angles were 0 and 110). Their leaves lie along the
        // wall's face on the start's side. (In the simulation's axes the
        // start's side is +z, and a door turns towards it about straight
        // down from the left hinge and straight up from the right.)
        let face = DOOR_WALL_Z - WALL_THICK / 2.0 + LEAF[2] / 2.0;
        let doors = [
            (DOORWAY[0], [1.0, 0.0, 0.0], [0.0, -1.0, 0.0]),
            (DOORWAY[1], [-1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
        ];
        for (end, forward, axis) in doors {
            let hinge = Hinge { at: in_sim(Vec3::new(end, 0.0, face)), axis, forward };
            let thing = Thing::door(hinge, 0.0, 110.0);
            let leaf = leaf_block(&thing);
            blocks.push(leaf);
            things.push(Placed {
                name: "door",
                part: Part::Leaf { size: Vec3::from(LEAF), color: Color::srgb(0.36, 0.30, 0.24) },
                thing,
                hit: leaf,
                place: hinge.at,
                holds: None,
                leaf: Some(blocks.len() - 1),
            });
        }
        // The game's figure, until `main` has read it from the install.
        Self { solids, radius: 0.366, site: Site { ground: 0.0, blocks }, things, thing_settings: None }
    }
}

/// Where a table of weapons stands, by its class's place in
/// `WEAPON_CRATES`: the middle of its top across the lane and along it, and
/// the level way from its back to its front, the side it is stood before.
/// Along the wall a table's back is to the wall; in the arm, to the right.
fn weapon_table(index: usize) -> ([f32; 2], Vec3) {
    let wall = KNEE_WALL_Z + WALL_THICK / 2.0 + 0.02;
    match index.checked_sub(ALONG_THE_WALL) {
        None => ([WEAPON_TABLE_FIRST + index as f32 * WEAPON_TABLE_STEP, wall + WEAPON_TABLE.z / 2.0], Vec3::Z),
        Some(on) => ([ARM_X, wall + ARM_FROM_WALL + WEAPON_TABLE.x / 2.0 + on as f32 * WEAPON_TABLE_STEP], Vec3::NEG_X),
    }
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
        pivot: centre - front * size.z / 2.0,
        axis: along,
        slide: Vec3::ZERO,
        color,
    }
}

/// Each label drawn over a container and where: the tables' over their
/// gun cases, and the airdrop crate's over it, a hand above the lid, in
/// Bevy's axes.
pub fn weapon_crate_labels() -> impl Iterator<Item = (&'static str, Vec3)> {
    let tables = WEAPON_CRATES.into_iter().enumerate().map(|(index, name)| {
        let ([x, z], _) = weapon_table(index);
        (name, Vec3::new(x, WEAPON_TABLE.y + GUN_CASE.y + 0.25, z))
    });
    tables.chain(std::iter::once((AIRDROP_LABEL, Vec3::new(AIRDROP[0], 0.5 + 0.25, AIRDROP[1]))))
}

/// Where in `Carried::away` the gun case of a class's table keeps what it
/// holds, by the class's place in `WEAPON_CRATES`.
pub fn gun_case_of(class: usize) -> usize {
    GUN_CASE_HOLDS + class
}

/// What the screen calls the container whose contents lie at that place in
/// `Carried::away`, where the range names it: the airdrop crate by what it
/// holds, a table's gun case or magazine box by its class.
pub fn weapon_crate_label(holds: usize) -> Option<String> {
    if holds == 0 {
        return Some(AIRDROP_LABEL.to_owned());
    }
    let case = holds.checked_sub(GUN_CASE_HOLDS).and_then(|at| WEAPON_CRATES.get(at)).map(|label| format!("{label} weapon case"));
    case.or_else(|| holds.checked_sub(MAGAZINE_BOX_HOLDS).and_then(|at| WEAPON_CRATES.get(at)).map(|label| format!("{label} magazine box")))
}

/// Which crate of weapons a start's name is: its label in small letters,
/// a dash for each space.
fn weapon_crate_named(name: &str) -> Option<usize> {
    WEAPON_CRATES.iter().position(|label| label.to_lowercase().replace(' ', "-") == name)
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
/// the camera, further off than `LABEL_REACH`, or under an open screen.
pub fn place_labels(
    camera: Query<(&Camera, &GlobalTransform), With<crate::MainCamera>>,
    controller: Res<crate::controller::Controller>,
    mut labels: Query<(&CrateLabel, &mut Node, &mut Visibility)>,
) {
    let Ok((camera, eye)) = camera.single() else { return };
    for (label, mut node, mut shown) in &mut labels {
        let near = eye.translation().distance(label.0) < LABEL_REACH && !controller.inventory_open;
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

    /// The box in the simulation's axes (z ahead).
    fn block(&self) -> Block {
        let (low, high) = self.corners();
        Block::square([low.x, low.y, -high.z], [high.x, high.y, -low.z])
    }

    /// How far along the line the box is entered, as a share of the line.
    /// A line that starts inside it does not hit it.
    fn entered(&self, from: Vec3, to: Vec3) -> Option<f32> {
        let (low, high) = self.corners();
        let (mut enter, mut leave) = (0.0f32, 1.0f32);
        for k in 0..3 {
            let (start, travel) = (from[k], to[k] - from[k]);
            if travel.abs() < 1e-9 {
                // A line that runs along a face does not enter the box.
                if start <= low[k] || start >= high[k] {
                    return None;
                }
                continue;
            }
            let (a, b) = ((low[k] - start) / travel, (high[k] - start) / travel);
            enter = enter.max(a.min(b));
            leave = leave.min(a.max(b));
        }
        (enter <= leave && enter > 0.0).then_some(enter)
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
            Part::Moving { centre, size, pivot, axis, slide, color } => {
                let turned = Quat::from_axis_angle(axis, self.thing.angle().to_radians());
                let at = pivot + turned * (centre - pivot) + slide * self.thing.out();
                Some((Transform::from_translation(at).with_rotation(turned), size, color))
            }
        }
    }

    /// What a line of sight meets of it, in the simulation's axes.
    fn met(&self, site: &Site) -> Block {
        self.leaf.map_or(self.hit, |leaf| site.blocks[leaf])
    }
}

impl Range {
    /// The place in the range a capture can start the character at: the
    /// far edge of the platform, the shelf, the foot of the stairs; before
    /// the doors on the lane's side, or past them; or before one of the
    /// things to work or take up, within reach of it with the view turned
    /// as `start_view` says. In the simulation's axes, feet on the floor.
    pub fn place(&self, name: &str) -> Option<[f32; 3]> {
        let far = STAIRS_FOOT - STAIRS as f32 * TREAD;
        // Where to stand, in Bevy's axes, from where a thing is: the row's
        // things are stood before on the door wall's side.
        let by = |at: [f32; 2], across: f32, along: f32| [at[0] + across, at[1] + along];
        let doorway = (DOORWAY[0] + DOORWAY[1]) / 2.0;
        let bevy = match name {
            "platform" => [STAIRS_LEFT + 2.5, far - 3.0],
            "shelf" => [STAIRS_LEFT + 6.5, far - 3.0],
            "stairs" => [STAIRS_LEFT + 1.25, STAIRS_FOOT + 2.0],
            "doors" => [doorway, DOOR_WALL_Z - 1.3],
            "yard" => [doorway, DOOR_WALL_Z + 0.75],
            // Before the airdrop crate, the clothing.
            "clothing" => [AIRDROP[0], AIRDROP[1] + 0.3 + 0.75],
            // Before the window on the lane's side, to vault through it.
            "window" => [(WINDOW[0] + WINDOW[1]) / 2.0, DOOR_WALL_Z - 1.3],
            "car" => by(CAR, -3.0, 0.0),
            "wooden-crate" => by(WOODEN_CRATE, 0.0, -0.9),
            "supply-crate" => by(SUPPLY_CRATE, 0.0, -1.075),
            "scav" => by(SCAV, 0.0, -0.62),
            "drawer" => by(DRAWER, 0.0, -1.05),
            "table" => by(TABLE, 0.0, -1.0),
            "high-shelf" => by(HIGH_SHELF, 0.0, -0.7),
            "floor" => by(ROUNDS, 0.0, -0.8),
            // Before a table of weapons, by its label (`rifles`,
            // `machine-guns`), three quarters of a metre from its front.
            _ => {
                let ([x, z], front) = weapon_table(weapon_crate_named(name)?);
                let at = Vec3::new(x, 0.0, z) + front * (WEAPON_TABLE.z / 2.0 + 0.75);
                [at.x, at.z]
            }
        };
        let at = [bevy[0], -bevy[1]];
        Some([at[0], self.ground_height(at[0], at[1]), at[1]])
    }

    /// Which way to look from a place of `place` (degrees right of the
    /// lane, degrees below level) for what is there to be in reach.
    pub fn start_view(name: &str) -> Option<[f32; 2]> {
        Some(match name {
            "doors" => [180.0, 5.0],
            "window" => [180.0, 5.0],
            "clothing" => [0.0, 45.0],
            "yard" => [0.0, 5.0],
            "car" => [90.0, 32.0],
            "wooden-crate" => [180.0, 60.0],
            "supply-crate" => [180.0, 35.0],
            "scav" => [180.0, 70.0],
            "drawer" => [180.0, 50.0],
            "table" => [180.0, 36.0],
            "high-shelf" => [180.0, 11.0],
            "floor" => [180.0, 80.0],
            // (The gun case on the table, its front faced; the magazine box
            // under it is reached crouched.)
            _ => {
                let (_, front) = weapon_table(weapon_crate_named(name)?);
                [(-front.x).atan2(front.z).to_degrees(), 35.0]
            }
        })
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
            // The solid in the simulation's axes, z ahead.
            let (middle, size) = (Vec3::new(solid.centre.x, solid.centre.y, -solid.centre.z), solid.size / 2.0);
            let between = centre - middle;
            let mut least: Option<(f32, Vec3)> = None;
            let faces = [Vec3::X, Vec3::Y, Vec3::Z];
            let edges = faces.iter().flat_map(|face| axes.iter().map(move |axis| face.cross(*axis)));
            let mut apart = false;
            for along in faces.into_iter().chain(axes).chain(edges) {
                // Two edges that run alike cross in nothing.
                let Some(along) = along.try_normalize() else { continue };
                let gap = between.dot(along);
                let overlap = reach(along) + along.abs().dot(size) - gap.abs();
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
        // From the ground, the platform's left side stops the character
        // its radius away, and it stays on the ground.
        let (stopped, _) = walk([STAIRS_LEFT - 1.0, 0.0, top + 2.5], [0.0235, 0.0], &|at| at[0] > STAIRS_LEFT + 1.0);
        assert!((stopped[0] - (STAIRS_LEFT - range.radius)).abs() < 1e-4 && stopped[1] == 0.0, "{stopped:?}");
        // Off the far edge: nothing under the feet.
        let (edge, middle) = (top + 5.0, STAIRS_LEFT + 2.5);
        assert_eq!(range.ground_height(middle, edge - 0.01), PLATFORM);
        assert_eq!(range.ground_height(middle, edge + 0.01), 0.0);
        // Falling with its axis a centimetre past the edge, the capsule
        // is in the platform's face: out of it 5 cm a move, and down.
        let falling = range.obstructed([middle, 2.0, edge + 0.01], [middle, 1.99, edge + 0.01], height);
        assert!((falling[2] - (edge + 0.06)).abs() < 2e-3 && falling[1] < 2.0 && falling[0] == middle, "{falling:?}");
        // And it cannot walk back in under the platform's top.
        let (back, _) = walk([middle, 0.0, edge + 1.0], [0.0, -0.0235], &|at| at[2] < edge);
        assert!((back[2] - (edge + range.radius)).abs() < 1e-4, "{back:?}");
    }

    /// The knee-high wall is not walked onto: the capsule's foot meets
    /// its rim too low to be pushed up over it.
    #[test]
    fn the_low_wall_stops_a_walk() {
        let range = Range::default();
        let height = Capsule { underside: 0.0, top: 1.6, lift: true };
        // Ahead of the start, to the left of the airdrop crate (to its
        // right the crates of weapons stand along the wall).
        let mut at = [-2.5, 0.0, 2.0];
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
        assert_eq!(gesture("trunk"), Some(27));
        assert_eq!(gesture("airdrop crate"), Some(29));
        // The car's trunk seen from behind it, a metre off.
        let rear = CAR[0] - 2.1;
        let seen = range.looked_at(Vec3::new(rear - 1.0, 1.5, CAR[1]), Vec3::new(rear + 0.2, 0.6, CAR[1]), 1.15);
        assert_eq!(seen.map(|i| range.things[i].name), Some("trunk"));
        // Every place before a thing is clear of everything solid.
        let height = Capsule { underside: 0.0, top: 1.6, lift: true };
        for name in ["doors", "yard", "window", "clothing", "car", "wooden-crate", "supply-crate", "scav", "drawer", "table", "high-shelf", "floor", "stairs", "rifles", "custom"] {
            let at = range.place(name).unwrap();
            assert_eq!(range.obstructed(at, at, height), at, "{name} stands in something");
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

    /// The things of the row stand a pace apart and none in another; the
    /// tables of weapons' cases and boxes are clear of one another, a case
    /// over its own box, and the first half a metre from the airdrop crate;
    /// the lower platform reaches to the knee-high wall; the airdrop crate
    /// is against that wall; and the ring is clear of everything.
    #[test]
    fn the_range_is_laid_out_as_drawn() {
        let range = Range::default();
        let on_tables = |placed: &Placed| placed.holds.is_some_and(|holds| holds >= GUN_CASE_HOLDS);
        let things: Vec<(Block, bool)> = range.things.iter().filter(|p| p.leaf.is_none()).map(|p| (p.hit, on_tables(p))).collect();
        for (i, (a, a_table)) in things.iter().enumerate() {
            for (b, b_table) in &things[i + 1..] {
                let apart = |k: usize| (a.low[k] - b.high[k]).max(b.low[k] - a.high[k]);
                let least = if *a_table && *b_table { 0.2 } else { 0.5 };
                assert!(apart(0).max(apart(1)).max(apart(2)) > 0.0, "{a:?} and {b:?} are in one another");
                // (A case and its box: one over the other.)
                if !(*a_table && *b_table && apart(1) > 0.3) {
                    assert!(apart(0).max(apart(2)) >= least, "{a:?} and {b:?} are under {least} m apart");
                }
            }
        }
        // The lower platform's right side is the knee-high wall's left end.
        let shelf = range.solids.iter().find(|s| s.size.distance(Vec3::new(3.0, SHELF, 5.0)) < 1e-4).unwrap();
        assert!((shelf.centre.x + shelf.size.x / 2.0 - (-WALL_HALF)).abs() < 1e-5);
        let crate_back = AIRDROP[1] - 0.3;
        assert!((crate_back - (KNEE_WALL_Z + WALL_THICK / 2.0)).abs() < 1e-5);
        let ring = &range.solids[..12];
        for solid in &range.solids[12..] {
            for block in ring {
                let apart = Vec2::new(solid.centre.x, solid.centre.z).distance(Vec2::new(block.centre.x, block.centre.z));
                assert!(apart > (solid.size.x.max(solid.size.z) / 2.0 + 3.0), "a block of the ring is {apart:.1} m from {:?}", solid.centre);
            }
        }
    }
}
