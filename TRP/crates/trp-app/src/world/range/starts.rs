//! Starting places, and where the weapon tables and cabinets stand.

use super::*;

impl Placed {
    /// Puts the thing, made about `anchor`, where its prop stands: its
    /// place, what a line of sight meets of it, its hinge and its moving
    /// part.
    pub(super) fn stand(&mut self, anchor: Vec3, stand: crate::world::level::Stand) {
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
fn moved_block(block: Block, anchor: Vec3, stand: crate::world::level::Stand) -> Block {
    let middle: [f32; 3] = std::array::from_fn(|k| (block.low[k] + block.high[k]) / 2.0);
    let half: [f32; 3] = std::array::from_fn(|k| (block.high[k] - block.low[k]) / 2.0);
    let moved = in_sim(stand.point(anchor, in_bevy(middle)));
    Block { low: std::array::from_fn(|k| moved[k] - half[k]), high: std::array::from_fn(|k| moved[k] + half[k]), turn: block.turn - stand.yaw }
}

/// A weapon table's prop's name: its label in small letters, and `table`.
pub(super) fn weapon_table_prop(index: usize) -> String {
    format!("{} table", WEAPON_CRATES[index].to_lowercase())
}

/// A storage place's prop's name: its label in small letters, and what it
/// is (`rigs cabinet`, `plates box`).
pub(super) fn cabinet_prop(index: usize) -> String {
    format!("{} {}", CABINETS[index].to_lowercase(), Furniture::of(CABINETS[index]).word())
}

/// The starts every range has (with a table's and a storage place's, by
/// their labels: `rifles`, `masks-and-hats`).
const STARTS: [&str; 30] = [
    "platform", "ledge", "wire", "shelf", "stairs", "doors", "yard", "open", "workbench", "armor-bench", "window", "car", "wooden-crate", "supply-crate", "scav", "drawer", "table", "key-table", "high-shelf", "floor", "lanes", "dummy", "dummy-side", "surfaces", "weapon-box", "toolbox", "safe", "airdrop-crate", "dummy-80", "dummy-180",
];

/// Every start's name.
pub fn start_names() -> Vec<String> {
    let labels = WEAPON_CRATES.iter().chain(&CABINETS).map(|label| label.to_lowercase().replace(' ', "-"));
    STARTS.iter().map(|name| (*name).to_owned()).chain(labels).collect()
}

/// A start as the range makes it: where the feet are on the ground (Bevy's
/// axes), its view, and the prop it goes with and where that is made about.
pub(super) fn made_start(name: &str) -> Option<(Vec3, [f32; 2], Option<(String, Vec3)>)> {
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
            let at = BenchKind::Gun.before(0.575);
            ([at.x, at.z], Some((BenchKind::Gun.prop().to_owned(), BenchKind::Gun.anchor())))
        }
        // Before the armor bench's table, in its zone, facing it.
        "armor-bench" => {
            let at = BenchKind::Armor.before(0.575);
            ([at.x, at.z], Some((BenchKind::Armor.prop().to_owned(), BenchKind::Armor.anchor())))
        }
        // Before the window on the lane's side, to vault through it.
        "window" => ([(WINDOW[0] + WINDOW[1]) / 2.0, DOOR_WALL_Z - 1.3], None),
        "car" => (by(CAR, -3.0, 0.0), of("car", CAR)),
        "wooden-crate" => (by(WOODEN_CRATE, 0.0, -0.9), of("wooden crate", WOODEN_CRATE)),
        "supply-crate" => (by(SUPPLY_CRATE, 0.0, -1.075), of("supply crate", SUPPLY_CRATE)),
        "scav" => (by(SCAV, 0.0, -0.62), of("scav", SCAV)),
        "drawer" => (by(DRAWER, 0.0, -1.05), of("drawer", DRAWER)),
        // Before the four past the car, as far off as their takes stood at
        // the key (the weapon box's front 0.94 m off, the toolbox's 0.84,
        // the safe's middle 1.13; the airdrop crate's take began open: as
        // the weapon box).
        "weapon-box" => (by(WEAPON_BOX, 0.0, -1.25), of("weapon box", WEAPON_BOX)),
        "toolbox" => (by(TOOLBOX, 0.0, -1.0), of("toolbox", TOOLBOX)),
        "safe" => (by(SAFE, 0.0, -1.1), of("safe", SAFE)),
        "airdrop-crate" => (by(AIRDROP_CRATE, 0.0, -1.4), of("airdrop crate", AIRDROP_CRATE)),
        "table" => (by(TABLE, 0.0, -1.0), of("table", TABLE)),
        "key-table" => (by(KEY_TABLE, 0.0, -0.9), of(KEY_TABLE_PROP, KEY_TABLE)),
        "high-shelf" => (by(HIGH_SHELF, 0.0, -0.7), of("high shelf", HIGH_SHELF)),
        "floor" => (by(ROUNDS, 0.0, -0.8), of(ROUNDS_PROP, ROUNDS)),
        // Down the hall's lanes, 4 m past the knee-high wall, facing
        // their far end.
        "lanes" => ([(LANES[0] + LANES[1]) / 2.0, KNEE_WALL_Z - 4.0], None),
        // 3.5 m before the dummy (`dummy`), facing it; so in the crowd
        // (`crowd-<n>`: `dummy::crowd`), its far rows down the lanes ahead
        // and some of it behind.
        "dummy" => ([crate::world::dummy::PLACES[0].at.x, crate::world::dummy::PLACES[0].at.z + 3.5], None),
        _ if crate::world::dummy::crowd_of(name).is_some() => ([crate::world::dummy::PLACES[0].at.x, crate::world::dummy::PLACES[0].at.z + 3.5], None),
        // 3 m to the standing dummy's own right, facing it from the side.
        "dummy-side" => ([crate::world::dummy::PLACES[0].at.x - 3.0, crate::world::dummy::PLACES[0].at.z], None),
        // Any number of metres behind the standing dummy, down the lanes,
        // facing its back (`dummy-80`, `dummy-180`: the levels of detail by
        // distance, `lod_view`; its head drawn to 76 m and its body to 177 m
        // at the install's view), or as far off that many degrees round it
        // toward its left, facing it (`dummy-25-40`).
        _ if behind_dummy(name).is_some() => {
            let (metres, degrees) = behind_dummy(name)?;
            let at = crate::world::dummy::PLACES[0].at;
            let turn = degrees.to_radians();
            ([at.x - metres * turn.sin(), at.z - metres * turn.cos()], None)
        }
        // Before the sampler of surfaces, on the start's side of it,
        // facing it.
        // At the near end of the course's row nearer the hall, facing along
        // it (TRP's own).
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
        // Facing the armor bench, as a cabinet.
        "armor-bench" => [-90.0, 25.0],
        "yard" => [0.0, 5.0],
        "open" => [90.0, 5.0],
        "car" => [90.0, 32.0],
        "wooden-crate" => [180.0, 60.0],
        "supply-crate" => [180.0, 35.0],
        "scav" => [180.0, 70.0],
        "drawer" => [180.0, 50.0],
        // (The four's pitches their takes' at the key.)
        "weapon-box" => [180.0, 50.0],
        "toolbox" => [180.0, 61.0],
        "safe" => [180.0, 59.0],
        "airdrop-crate" => [180.0, 50.0],
        "table" => [180.0, 36.0],
        "key-table" => [180.0, 40.0],
        "high-shelf" => [180.0, 11.0],
        "floor" => [180.0, 80.0],
        "surfaces" => [90.0, 20.0],
        "lanes" | "dummy" => [0.0, 0.0],
        _ if crate::world::dummy::crowd_of(name).is_some() => [0.0, 0.0],
        "dummy-side" => [90.0, 0.0],
        _ if behind_dummy(name).is_some() => [180.0 - behind_dummy(name)?.1, 0.0],
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

/// The metres a start `dummy-<metres>[-<degrees>]` stands from the standing
/// dummy, and the degrees round from behind it.
fn behind_dummy(name: &str) -> Option<(f32, f32)> {
    let rest = name.strip_prefix("dummy-")?;
    let (metres, degrees) = rest.split_once('-').unwrap_or((rest, "0"));
    let (metres, degrees) = (metres.parse::<f32>().ok()?, degrees.parse::<f32>().ok()?);
    (metres.is_finite() && metres > 0.0 && degrees.is_finite()).then_some((metres, degrees))
}

/// The last box put among the solids is made of that.
pub(super) fn made_of(solids: &mut [Solid], surface: u8) {
    if let Some(last) = solids.last_mut() {
        last.surface = surface;
    }
}

/// Where a table of weapons stands, by its class's place in
/// `WEAPON_CRATES`: the middle of its top across the lane and along it, and
/// the level way from its back to its front, the side it is stood before.
/// Every table's back is to the wall: the first `ALONG_THE_WALL` to the
/// workbench's right, the rest to its left, each from the bench out.
pub(super) fn weapon_table(index: usize) -> ([f32; 2], Vec3) {
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
pub(super) fn cabinet_at(index: usize) -> [f32; 2] {
    let before: f32 = CABINETS[..index].iter().map(|label| Furniture::of(label).size().x + CABINETS_GAP).sum();
    let size = Furniture::of(CABINETS[index]).size();
    [CABINETS_BACK + size.z / 2.0, CABINETS_NEAR - before - size.x / 2.0]
}
