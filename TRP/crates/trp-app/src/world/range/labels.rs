//! The hall's lamps, the crates' labels, the barbed wire and what lies about.

use super::*;

/// The hall's concrete: the middle of Tarkov's own concretes' pictures
/// (0.48 to 0.67: the hideout's concrete walls, `inventory.md`). The app's.
pub(super) const HALL_GREY: Color = Color::srgb(0.55, 0.55, 0.55);

/// The colour of a solid that is not drawn (the workbench's boxes, under
/// its own model).
pub const UNSEEN: Color = Color::NONE;

/// The indoors: what the indoor trigger and the stencil volume take
/// (`lighting`, `prism_view`), a level file's (`indoors`); none for the
/// range's own, all outdoors since its hall went. Its middle,
/// size and turn about the upright (Bevy's axes).
pub fn indoors() -> Option<(Vec3, Vec3, f32)> {
    crate::world::level::get().and_then(|level| level.indoors.as_ref().map(|found| (found.centre, found.size, found.yaw)))
}

/// How far the sun's shadows reach at the least (`shadows::with_baked_reach`):
/// the hall's furthest two corners apart (48.9 m), and a little more; for a
/// level file's indoors, its own corners apart and as much more, if
/// further.
pub fn baked_reach() -> f32 {
    match (crate::world::level::get(), indoors()) {
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
/// on the range's own, which has no roof since its hall went.
pub fn hall_lamps() -> Vec<Vec3> {
    crate::world::level::get().map(|level| level.lamps.clone()).unwrap_or_default()
}

/// The lid of a box `size` long, high and deep whose top's middle is at
/// `top` and whose front is the way `front`: a slab on it, hinged along its
/// back edge and lifted towards the back by a negative angle (a turn the
/// left hand's way about the line along the box, `front`'s up and over).
pub(super) fn lid_of(top: Vec3, size: Vec3, front: Vec3, color: Color) -> Part {
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
    let squares = surface_squares().iter().map(|square| (trp_data::audio::sound::SURFACES[square.surface as usize], square.centre + Vec3::Y * 0.05));
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
/// then on the workbench's table (Bevy's axes): now on the small
/// white table by the doorway, at its top's middle, where that table stands
/// (`KEY_TABLE`; TRP's own).
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
pub(super) fn weapon_crate_named(name: &str) -> Option<usize> {
    WEAPON_CRATES.iter().position(|label| label.to_lowercase().replace(' ', "-") == name)
}

/// Which storage cabinet a start's name is, the same way (`rigs`,
/// `masks-and-hats`).
pub(super) fn cabinet_named(name: &str) -> Option<usize> {
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
                inside.spawn(crate::screens::inventory_view::words(name, 14.0, Color::srgb(0.90, 0.88, 0.78)));
            });
        });
    }
}

/// Puts each label where the camera sees its crate; none is drawn behind
/// the camera, further off than `LABEL_REACH`, on the other side of the
/// hall's walls from it, or under an open screen.
pub fn place_labels(
    camera: Query<(&Camera, &GlobalTransform), With<crate::MainCamera>>,
    controller: Res<crate::player::controller::Controller>,
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
pub(super) fn leaf_block(thing: &Thing) -> Block {
    let hinge = thing.hinge.expect("a door");
    let along = turn(hinge.forward, hinge.axis, thing.angle());
    let foot = std::array::from_fn(|k| hinge.at[k] + along[k] * LEAF[0] / 2.0);
    Block::turned(foot, LEAF, along)
}
