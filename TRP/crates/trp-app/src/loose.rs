//! Things on the ground, moved as the game's physics moves them
//! (`docs/spec/physics.md`, "Loose things"; `trp_physics::loose`): a thing
//! put down from the inventory or a backpack dropped is thrown as the
//! game's `ThrowItem` throws it, the magazine a quick reload lets go falls
//! from where the weapon had it (`DropMod`), and each, with its own model
//! and its prefab's colliders, falls in the game's engine against the
//! range until the game would take its body away.
//!
//! A thing's model is read on a thread of its own, as its picture's is
//! (`icons::Worker::model`): until it has come the thing is not drawn, and
//! its body, once made, is stepped through every step it missed, so that
//! it falls as it would have (it meets nothing that moves: the game's
//! `Loot` layer meets neither itself nor a corpse nor the character). Each
//! thing falls in a world of its own for that reason.
//!
//! A thing on the ground that was not thrown (the range's own, put out
//! when the run begins, or a round the weapon put down) is set on the
//! ground and let settle (this work's layout, not the game's: its things
//! are where the map puts them).

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use bevy::math::Affine3A;
use bevy::prelude::*;
use bevy::tasks::{block_on, futures_lite::future::poll_once, AsyncComputeTaskPool, Task};
use bevy::transform::helper::TransformHelper;
use trp_data::engine::PhysicsSettings;
use trp_physics::loose::{self, Dice, Start, Thrower, Watch};
use trp_physics::{Body, Collider, Material, Shape, World, V3};
use trp_sim::inventory::Id;

use crate::controller::Controller;
use crate::convert;
use crate::icons::{Model, Worker};
use crate::inventory_view::Carried;
use crate::range::Range;
use crate::weapon::WeaponRig;

/// How a thing left the character.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum How {
    /// Put down from the inventory, or a backpack off the shoulders
    /// (`GameWorld.ThrowItem` with no direction).
    Thrown,
    /// The magazine a quick reload lets go, where the weapon has it
    /// (`DropMod`).
    LetFall,
}

/// A model come from the thread: what it draws and its colliders, in the
/// game's axes, in its first object's.
struct Prepared {
    template: String,
    model: Model,
    colliders: Vec<Collider>,
}

/// A model as the frame keeps it: each node's parent, own transform and
/// whether it is on; what each draws; and its colliders.
struct Ready {
    nodes: Vec<(Option<usize>, Transform, bool)>,
    drawn: Vec<(usize, Handle<Mesh>, Handle<StandardMaterial>)>,
    colliders: Vec<Collider>,
    /// The marks of the pictures it shares with the rigs and the rest
    /// (`render_data::SharedPictures`), to let go of when nothing on the
    /// ground draws with it any more.
    marks: Vec<u64>,
}


enum Asked {
    Waiting,
    Ready(Arc<Ready>),
    /// Not to be had: the thing lies as the plain box it was before.
    Failed,
}

/// A thing on its way down.
struct Flight {
    id: Id,
    template: String,
    mass: f32,
    /// Where it set off, and how; or, for a thing set down, the place on
    /// the ground it is set on (in the game's axes).
    start: Start,
    placed: bool,
    /// How long after it set off it is first drawn
    /// (`ThrowLootMakeVisibleDelay` for a throw of the inventory's).
    hidden_for: f32,
    since: f32,
    /// The physics' steps and the frames' times since it set off, while
    /// its model was still to come.
    missed: Vec<(f32, f32)>,
    moving: Option<Moving>,
}

struct Moving {
    world: World,
    body: Body,
    swept: bool,
    watch: Watch,
}

/// A thing's model in the world.
#[derive(Component)]
pub struct LooseModel(pub Id);

/// Where a loose thing is: its centre of mass in the engine's axes,
/// which is what the eye looks for.
#[derive(Debug, Clone, Copy)]
pub struct Place {
    pub centre: Vec3,
}

#[derive(Resource)]
pub struct Loose {
    install: trp_data::Install,
    physics: Option<PhysicsSettings>,
    swept_under: f32,
    hidden_for: f32,
    worker: Option<Worker>,
    task: Option<Task<(Option<Worker>, String, anyhow::Result<Prepared>)>>,
    queue: VecDeque<String>,
    models: HashMap<String, Asked>,
    flights: Vec<Flight>,
    dice: Dice,
    /// Every loose thing's place, by item, once it has one.
    pub places: HashMap<Id, Place>,
}

impl Loose {
    pub fn new(install: trp_data::Install, physics: Option<PhysicsSettings>, hard: &trp_data::hard_settings::HardSettings, seed: u64) -> Self {
        let figure = |name: &str, otherwise: f32| {
            hard.float(name).unwrap_or_else(|e| {
                eprintln!("{e:#}; {otherwise} is taken for {name}");
                otherwise
            })
        };
        Self {
            install,
            physics,
            swept_under: figure("LootVolumeForHighQuallityPhysicsClient", loose::SWEPT_UNDER),
            hidden_for: figure("ThrowLootMakeVisibleDelay", 0.15),
            worker: None,
            task: None,
            queue: VecDeque::new(),
            models: HashMap::new(),
            flights: Vec::new(),
            dice: Dice::new(seed ^ 0x6c6f_6f73_6521),
            places: HashMap::new(),
        }
    }

    /// Whether a thing on the ground is drawn as the plain box: its model
    /// cannot be had, or the engine's settings could not be read.
    pub fn stand_in(&self, id: Id, template: &str) -> bool {
        self.physics.is_none() || matches!(self.models.get(template), Some(Asked::Failed)) || !(self.places.contains_key(&id) || self.flights.iter().any(|f| f.id == id))
    }

    fn ask(&mut self, template: &str) {
        if !self.models.contains_key(template) {
            self.models.insert(template.to_owned(), Asked::Waiting);
            self.queue.push_back(template.to_owned());
        }
    }
}

/// The range as the engine has it: the ground on the terrain's layer, and
/// every box of the range (`Range::boxes`) on the layer the world's
/// colliders are on.
pub fn range_world(physics: &PhysicsSettings, range: &Range) -> World {
    let mut world = World::new(trp_physics::Settings::of(physics));
    let (terrain, high_poly) = (physics.layer("Terrain").unwrap_or(11), physics.layer("HighPolyCollider").unwrap_or(12));
    world.add_ground(0.0, Material::default(), terrain);
    for (centre, size, turn) in range.boxes() {
        let shape = Shape::Box { half: [size.x / 2.0, size.y / 2.0, size.z / 2.0] };
        world.add_static([centre.x, centre.y, -centre.z], convert::rotation_back(turn), &[Collider { shape, at: [0.0; 3], turn: [0.0, 0.0, 0.0, 1.0], material: Material::default() }], high_poly);
    }
    world
}

/// A place and turn of the game's as Bevy's.
fn drawn(at: V3, turn: [f32; 4]) -> Transform {
    Transform { translation: convert::position(at), rotation: convert::rotation(turn), ..default() }
}

/// The thread: one model at a time, in the order asked; its colliders
/// with it, a convex mesh's points read with the model.
pub fn read_models(
    mut loose: ResMut<Loose>,
    carried: Res<Carried>,
    (mut meshes, mut images, mut materials, mut shared): (ResMut<Assets<Mesh>>, ResMut<Assets<Image>>, ResMut<Assets<StandardMaterial>>, ResMut<crate::render_data::SharedPictures>),
) {
    let loose = &mut *loose;
    // A model nothing on the ground draws with, and none on its way, is let
    // go of: its materials, its meshes and its share of the pictures.
    if loose.models.values().any(|asked| matches!(asked, Asked::Ready(_))) {
        let in_use: std::collections::HashSet<&str> = loose
            .flights
            .iter()
            .map(|f| f.template.as_str())
            .chain(carried.ground.iter().filter(|(item, _)| loose.places.contains_key(&item.id)).map(|(item, _)| item.kind.template.as_str()))
            .collect();
        let unused: Vec<String> = loose.models.iter().filter(|(template, asked)| matches!(asked, Asked::Ready(_)) && !in_use.contains(template.as_str())).map(|(template, _)| template.clone()).collect();
        for template in unused {
            if let Some(Asked::Ready(ready)) = loose.models.remove(&template) {
                shared.release(&ready.marks);
                info!("the model of {template} let go: nothing on the ground draws with it");
            }
        }
    }
    if let Some(task) = loose.task.as_mut() {
        let Some((worker, template, made)) = block_on(poll_once(task)) else { return };
        loose.task = None;
        loose.worker = worker;
        let asked = match made {
            Ok(Prepared { template, model, colliders }) => {
                let Model { hierarchy, looks, parts, extra, .. } = model;
                let bytes = looks.picture_bytes();
                let (mesh_handles, material_handles, _, kept) = looks.add_sharing(&mut meshes, &mut images, &mut materials, Some(&mut shared));
                let nodes = hierarchy
                    .nodes
                    .iter()
                    .enumerate()
                    .map(|(index, node)| {
                        // The first object's own place, turn and scale are
                        // the game's to set (`CreateLootPrefab`).
                        let own = if index == 0 { Transform::IDENTITY } else { convert::transform(node.translation, node.rotation, node.scale) };
                        (node.parent.filter(|_| index > 0), own, node.active)
                    })
                    .collect();
                let mut drawn = Vec::new();
                for (index, own) in parts.iter().enumerate() {
                    drawn.extend(own.iter().map(|(mesh, material)| (index, mesh_handles[*mesh].clone(), material_handles[*material].clone())));
                }
                drawn.extend(extra.iter().map(|(node, mesh, material)| (*node, mesh_handles[*mesh].clone(), material_handles[*material].clone())));
                info!(
                    "the model of {template} for the world: {} colliders, {} materials, {} of {} pictures already on the card; {:.1} MB of pictures sent",
                    colliders.len(),
                    material_handles.len(),
                    kept.already,
                    kept.marks.len(),
                    bytes.saturating_sub(kept.bytes_spared) as f64 / 1e6
                );
                Asked::Ready(Arc::new(Ready { nodes, drawn, colliders, marks: kept.marks }))
            }
            Err(e) => {
                info!("{template} lies as a box: its model could not be had: {e:#}");
                Asked::Failed
            }
        };
        loose.models.insert(template, asked);
    }
    let Some(template) = loose.queue.pop_front() else {
        // Nothing left to read: the thread's reader goes, away from the
        // frame (as the icons' does).
        if let Some(worker) = loose.worker.take() {
            AsyncComputeTaskPool::get().spawn(async move { drop(worker) }).detach();
        }
        return;
    };
    let (worker, install) = (loose.worker.take(), loose.install.clone());
    loose.task = Some(AsyncComputeTaskPool::get().spawn(async move {
        let mut worker = match worker.map_or_else(|| Worker::new(&install), Ok) {
            Ok(worker) => worker,
            Err(e) => return (None, template, Err(e)),
        };
        let made = prepare(&mut worker, &template);
        (Some(worker), template, made)
    }));
}

/// A thing's model for the world, and its colliders: its prefab's; or, for
/// a model with none (an outfit of the other game's), the box its meshes
/// fill (`PROVISIONAL`).
fn prepare(worker: &mut Worker, template: &str) -> anyhow::Result<Prepared> {
    let mut model = worker.model(template)?;
    model.looks.unskinned();
    // Marked, to draw with a picture already on the card where it is the
    // same (a rig's, or another thing's).
    model.looks.mark_pictures();
    let unity = &worker.unity;
    let mut points = |mesh| trp_data::unity::extract::read_mesh(unity, mesh).ok().filter(|m| !m.positions.is_empty()).map(|m| Arc::from(m.positions));
    let mut colliders = loose::colliders(&model.hierarchy, &mut points);
    if colliders.is_empty() {
        // Each drawn mesh's corners in the first object's axes (Bevy's,
        // then mirrored to the game's).
        let mut world: Vec<Mat4> = Vec::with_capacity(model.hierarchy.nodes.len());
        for (index, node) in model.hierarchy.nodes.iter().enumerate() {
            let own = if index == 0 { Mat4::IDENTITY } else { convert::transform(node.translation, node.rotation, node.scale).to_matrix() };
            world.push(node.parent.filter(|_| index > 0).map_or(Mat4::IDENTITY, |parent| world[parent]) * own);
        }
        let (mut low, mut high) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        let drawn = model.parts.iter().enumerate().flat_map(|(node, own)| own.iter().map(move |(mesh, _)| (node, *mesh))).chain(model.extra.iter().map(|(node, mesh, _)| (*node, *mesh)));
        for (node, mesh) in drawn {
            if let Some(bevy::mesh::VertexAttributeValues::Float32x3(points)) = model.looks.meshes[mesh].attribute(Mesh::ATTRIBUTE_POSITION) {
                for p in points {
                    let at = world[node].transform_point3(Vec3::from(*p));
                    (low, high) = (low.min(at), high.max(at));
                }
            }
        }
        anyhow::ensure!(low.x <= high.x, "it has no collider and draws nothing");
        let (centre, half) = ((low + high) / 2.0, ((high - low) / 2.0).max(Vec3::splat(0.005)));
        colliders.push(Collider { shape: Shape::Box { half: half.to_array() }, at: [centre.x, centre.y, -centre.z], turn: [0.0, 0.0, 0.0, 1.0], material: Material::default() });
    }
    Ok(Prepared { template: template.to_owned(), model, colliders })
}

/// What the inventory has let go this frame is set going as the game sets
/// it going; a thing on the ground with nowhere yet (the range's, or what
/// a restart put back) is set down; a thing taken up goes. And the weapon
/// in hand draws its magazine or not as its reload says.
#[allow(clippy::too_many_arguments)]
pub fn set_going(
    mut commands: Commands,
    mut loose: ResMut<Loose>,
    mut carried: ResMut<Carried>,
    mut controller: ResMut<Controller>,
    time: Res<Time>,
    weapon: Option<Res<WeaponRig>>,
    helper: TransformHelper,
    (names, children, models): (Query<&Name>, Query<&Children>, Query<(Entity, &LooseModel)>),
    mut again: MessageReader<crate::death::BeginAgain>,
) {
    let loose = &mut *loose;
    // A run begun again puts back what lay on the ground at its start, each
    // set down afresh.
    if again.read().count() > 0 {
        loose.flights.clear();
        loose.places.clear();

        for (entity, _) in &models {
            commands.entity(entity).despawn();
        }
    }
    // The weapon's magazine, as the game finds it (`GetModTransform`: the
    // first object under the weapon's magazine slot, looked for from its
    // root), drawn as the rig is, the first person's squeeze along the view
    // taken off.
    let magazine = weapon.as_ref().and_then(|rig| {
        let from = rig.weapon_root?;
        let mut stack = vec![from];
        while let Some(entity) = stack.pop() {
            if names.get(entity).is_ok_and(|name| name.as_str() == "mod_magazine") {
                return children.get(entity).ok().and_then(|below| below.first().copied());
            }
            stack.extend(children.get(entity).into_iter().flatten());
        }
        None
    });
    let unsqueezed = |entity: Entity| -> Option<(V3, [f32; 4])> {
        let rig = weapon.as_ref()?;
        let root = helper.compute_global_transform(rig.root).ok()?;
        let (_, turn, at) = root.to_scale_rotation_translation();
        let now = helper.compute_global_transform(entity).ok()?;
        let (_, turn_now, at_now) = (Affine3A::from_rotation_translation(turn, at) * root.affine().inverse() * now.affine()).to_scale_rotation_translation();
        Some(([at_now.x, at_now.y, -at_now.z], convert::rotation_back(turn_now)))
    };
    if let (Some(shown), Some(entity)) = (controller.magazine_drawn.take(), magazine) {
        commands.entity(entity).insert(if shown { Visibility::Inherited } else { Visibility::Hidden });
    }
    // With the options' `ITEM PHYSICS` off nothing more: what is put down
    // lies as a box where it is put (`inventory_view::show_ground`).
    if loose.physics.is_none() {
        if !carried.thrown.is_empty() {
            carried.bypass_change_detection().thrown.clear();
        }
        return;
    }
    let m = &controller.movement;
    let [height, _] = m.capsule_figures();
    let weapon_root = weapon.as_ref().and_then(|rig| rig.weapon_root).and_then(|entity| unsqueezed(entity)).map(|(_, turn)| turn);
    let by = Thrower {
        yaw: m.yaw,
        pitch: m.pitch,
        chest: [m.position[0], m.position[1] + height * 0.65, m.position[2]],
        velocity: m.velocity,
        dt: time.delta_secs(),
        weapon_root: weapon_root.unwrap_or_else(|| loose::euler(m.pitch, m.yaw, 0.0)),
    };
    let magazine_at = magazine.and_then(|entity| unsqueezed(entity));
    // (Taken without telling of a change to what is carried: the screen
    // is drawn afresh on one.)
    let thrown = if carried.thrown.is_empty() { Vec::new() } else { std::mem::take(&mut carried.bypass_change_detection().thrown) };
    for (id, how) in thrown {
        let Some((item, at)) = carried.ground.iter().find(|(item, _)| item.id == id) else { continue };
        let start = match (how, magazine_at) {
            (How::LetFall, Some((at, turn))) => loose::let_fall(at, turn, &mut loose.dice),
            // No weapon to let it fall from: where it would lie.
            (How::LetFall, None) => loose::let_fall([at.x, at.y + 1.0, -at.z], [0.0, 0.0, 0.0, 1.0], &mut loose.dice),
            (How::Thrown, _) => loose::thrown(&by, false, &mut loose.dice),
        };
        let hidden_for = if how == How::Thrown { loose.hidden_for } else { 0.0 };
        let (template, mass) = (item.kind.template.clone(), item.weight());
        loose.flights.retain(|f| f.id != id);
        loose.places.remove(&id);
        info!("{template} set going ({how:?}) at {:?}, turned {:?}, at {:?} m/s; the character at {:?}", start.at, start.turn, start.velocity, controller.movement.position);
        loose.flights.push(Flight { id, template: template.clone(), mass, start, placed: false, hidden_for, since: 0.0, missed: Vec::new(), moving: None });
        loose.ask(&template);
    }
    // Set down: the rest of what lies on the ground with nowhere yet.
    let unplaced: Vec<(Id, String, f32, Vec3)> = carried
        .ground
        .iter()
        .filter(|(item, _)| !loose.places.contains_key(&item.id) && !loose.flights.iter().any(|f| f.id == item.id))
        .map(|(item, at)| (item.id, item.kind.template.clone(), item.weight(), *at))
        .collect();
    for (id, template, mass, at) in unplaced {
        if matches!(loose.models.get(&template), Some(Asked::Failed)) {
            continue;
        }
        let start = Start { at: [at.x, at.y, -at.z], turn: [0.0, 0.0, 0.0, 1.0], velocity: [0.0; 3], spin: [0.0; 3] };
        loose.flights.push(Flight { id, template: template.clone(), mass, start, placed: true, hidden_for: 0.0, since: 0.0, missed: Vec::new(), moving: None });
        loose.ask(&template);
    }
    // Taken up, or gone with a restart: its flight, place and model go.
    let lying: std::collections::HashSet<Id> = carried.ground.iter().map(|(item, _)| item.id).collect();
    loose.flights.retain(|f| lying.contains(&f.id));
    loose.places.retain(|id, _| lying.contains(id));

    for (entity, model) in &models {
        if !lying.contains(&model.0) {
            commands.entity(entity).despawn();
        }
    }
}

/// Every thing on its way down, a step of the game's physics this frame
/// (`death::PhysicsClock`): its body made once its model is there and
/// stepped through what it missed, its watch kept, and its model drawn
/// where the body is; done, its body goes and it lies where it is.
#[allow(clippy::too_many_arguments)]
pub fn fly(
    mut commands: Commands,
    mut loose: ResMut<Loose>,
    clock: Res<crate::death::PhysicsClock>,
    time: Res<Time>,
    range: Res<Range>,
    mut models: Query<(&LooseModel, &mut Transform, &mut Visibility)>,
) {
    let loose = &mut *loose;
    let Some(physics) = loose.physics.clone() else { return };
    let (step, frame) = (clock.now(), time.delta_secs());
    let layer = physics.layer("Loot").unwrap_or(15);
    let mut done = Vec::new();
    for (index, flight) in loose.flights.iter_mut().enumerate() {
        flight.since += frame;
        let ready = match loose.models.get(&flight.template) {
            Some(Asked::Ready(ready)) => ready.clone(),
            Some(Asked::Failed) => {
                done.push(index);
                continue;
            }
            _ => {
                flight.missed.push((step, frame));
                continue;
            }
        };
        if flight.moving.is_none() {
            let mut world = range_world(&physics, &range);
            let mut start = flight.start;
            if flight.placed {
                // Set down: its colliders' underside a centimetre over the
                // ground where it is put.
                let lowest = loose::bounds(&ready.colliders).map_or(0.0, |(low, _)| low[1]);
                start.at[1] += 0.01 - lowest;
            }
            let (body, _) = loose::add(&mut world, ready.colliders.clone(), flight.mass, &start, loose.swept_under, layer);
            let swept = loose::bounds(&ready.colliders).is_some_and(|(low, high)| (high[0] - low[0]) * (high[1] - low[1]) * (high[2] - low[2]) <= loose.swept_under);
            let mut moving = Moving { world, body, swept, watch: Watch::default() };
            // What it missed while its model was read.
            let mut stopped = false;
            for (missed, frame) in std::mem::take(&mut flight.missed) {
                moving.world.step(missed);
                if moving.watch.frame(&moving.world, moving.body, moving.swept, frame) {
                    stopped = true;
                    break;
                }
            }
            let (at, turn) = moving.world.pose(moving.body);
            let object = drawn(at, turn);
            let id = flight.id;
            let root = commands
                .spawn((Name::new(format!("Loose {}", flight.template)), LooseModel(id), object, if flight.since >= flight.hidden_for { Visibility::Inherited } else { Visibility::Hidden }))
                .id();
            let mut entities = Vec::with_capacity(ready.nodes.len());
            for (parent, own, on) in &ready.nodes {
                let entity = commands.spawn((*own, if *on { Visibility::Inherited } else { Visibility::Hidden })).id();
                commands.entity(parent.map_or(root, |p| entities[p])).add_child(entity);
                entities.push(entity);
            }
            for (node, mesh, material) in &ready.drawn {
                let part = commands.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(material.clone()), Transform::default())).id();
                commands.entity(entities[*node]).add_child(part);
            }
            let centre = place_of(&moving.world, moving.body);
            loose.places.insert(id, Place { centre });
            flight.moving = Some(moving);
            if stopped {
                done.push(index);
            }
            continue;
        }
        let moving = flight.moving.as_mut().expect("moving");
        moving.world.step(step);
        let stop = moving.watch.frame(&moving.world, moving.body, moving.swept, frame);
        let (at, turn) = moving.world.pose(moving.body);
        let object = drawn(at, turn);
        loose.places.insert(flight.id, Place { centre: place_of(&moving.world, moving.body) });
        for (model, mut transform, mut visibility) in &mut models {
            if model.0 == flight.id {
                *transform = object;
                if flight.since >= flight.hidden_for {
                    *visibility = Visibility::Inherited;
                }
            }
        }
        if stop {
            done.push(index);
        }
    }
    for index in done.into_iter().rev() {
        let flight = loose.flights.remove(index);
        if flight.moving.is_some() {
            info!("{} lies still {:.2} s after it set off, at {:?}", flight.template, flight.since, loose.places.get(&flight.id).map(|place| place.centre));
        }
    }
}

/// A body's centre of mass in the engine's axes.
fn place_of(world: &World, body: Body) -> Vec3 {
    let (at, turn) = world.pose(body);
    let (centre, _) = world.mass_properties(body);
    let c = trp_physics::rotate(turn, centre);
    convert::position([at[0] + c[0], at[1] + c[1], at[2] + c[2]])
}
