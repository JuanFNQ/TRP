//! Spawning an assembled weapon as an entity tree.

use std::collections::{HashMap, VecDeque};

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::render::mesh::skinning::{SkinnedMesh, SkinnedMeshInverseBindposes};
use bevy::render::view::NoFrustumCulling;
use trp_data::unity::anim::path_hash;

use crate::animation::{ClipLibrary, ClipPlayer};
use trp_data::unity::extract::Hierarchy;
use crate::render_data::{Converted, PreparedLooks};
use crate::{convert, GameData, Options};

/// The player. Its origin is at the feet and it turns with the look yaw.
#[derive(Component)]
pub struct Player;

/// The frame the first-person rig hangs in: at the body's ribcage, turned
/// to where the player looks. It carries the look pitch; the weapon rig
/// hangs from it unmoved and the camera sits in it beside the weapon.
#[derive(Component)]
pub struct ViewPivot;

/// A mesh of the hands as the player sees them: the game draws it in first
/// person only (`body_view::show_upper_body`).
#[derive(Component)]
pub struct FirstPersonHands;

/// Where the character's own eyes are: the first-person camera's place in
/// the view's frame, kept there whatever camera the picture is drawn from.
/// What the character looks at goes by it.
#[derive(Component)]
pub struct Eye;

/// Entities of the spawned weapon.
#[derive(Resource)]
pub struct WeaponRig {
    /// Nodes by the hash of their path from the animated root, which is
    /// how animation tracks name them.
    pub by_hash: HashMap<u32, Entity>,
    /// The animated root, which clips do not move.
    pub root: Entity,
    /// The view pivot the root hangs from. The camera hangs from it too:
    /// it sits in the root's frame but is not squeezed with it.
    pub pivot: Entity,
    /// The bone the weapon hangs from, which the weapon's clips place.
    pub weapon_root: Option<Entity>,
    /// Its child, which the procedural motion moves.
    pub weapon_root_anim: Option<Entity>,
    /// The two arms, each reaching for its hold on the weapon.
    pub arms: Vec<crate::arms::ArmRig>,
    /// How the game's solver for each arm is set up, right then left, if
    /// the player prefab's could be read and is one this follows.
    pub limbs: Option<[trp_data::player::LimbData; 2]>,
    /// How many inventory cells wide the body's animator is told the
    /// thing in hand is.
    pub width: f32,
    /// And what kind of thing it is (`trp_sim::body::hands_kind`).
    pub kind: f32,
    /// Every bone of the rig in the order of its prefab's hierarchy, which
    /// is the order its animator poses them in.
    pub bones: Vec<Entity>,
}

/// The rig with no engine, for what the simulation takes from it as its
/// animator poses it: the bone the weapon hangs from, the marker for the
/// right hand and the sight's aim bone (both in the moved bone's axes),
/// and the turn of the animated camera bone.
pub struct RigFrame {
    rig: trp_sim::rig::WeaponRig,
    parent: Option<usize>,
    moved: Option<usize>,
    marker: Option<usize>,
    /// The bones the eye is lined up with while the sights are up.
    sights: Option<trp_sim::rig::SightNodes>,
    eye: Option<usize>,
    /// The left hand's marker and the weapon's marker for the vest: how
    /// far apart they are weighs the body's reach for the vest, for a
    /// character seen from outside.
    left_marker: Option<usize>,
    vest_marker: Option<usize>,
}

/// What the simulation takes from the rig, in the game's axes.
#[derive(Clone, Copy)]
pub struct RigPose {
    pub parent: ([f32; 3], [f32; 4]),
    pub marker: ([f32; 3], [f32; 4]),
    pub aim_bone: Option<[f32; 3]>,
    pub animated: [f32; 4],
    /// The muzzle in the moved bone's axes, and the line through the two
    /// iron sights, where the weapon has both.
    pub muzzle: Option<[f32; 3]>,
    pub sight_line: Option<trp_sim::SightLine>,
    /// How far the left hand's marker is from the weapon's marker for the
    /// vest, in metres, where the rig has both.
    pub vest_reach: Option<f32>,
}

impl Default for RigPose {
    fn default() -> Self {
        const NO_TURN: [f32; 4] = [0.0, 0.0, 0.0, 1.0];
        Self {
            parent: ([0.0; 3], NO_TURN),
            marker: ([0.0; 3], NO_TURN),
            aim_bone: None,
            animated: trp_sim::math::quat_euler([-90.0, 0.0, 0.0]),
            muzzle: None,
            sight_line: None,
            vest_reach: None,
        }
    }
}

impl RigFrame {
    fn new(hierarchy: &Hierarchy) -> Self {
        let rig = trp_sim::rig::WeaponRig::new(hierarchy);
        let named = |name: &str| hierarchy.nodes.iter().position(|n| n.name == name);
        Self {
            parent: rig.bone("Weapon_root"),
            moved: rig.moved_bone(),
            marker: rig.bone("Weapon_root/Weapon_root_anim/weapon/weapon_R_IK_marker"),
            // The aim bone of the sight the game aims by, and the two
            // points of the iron sights' line (weapon-handling spec, "The
            // sights"). The empty hands have none.
            sights: trp_sim::rig::SightNodes::find(hierarchy),
            eye: named("Camera_animated"),
            left_marker: rig.bone("Weapon_root/Weapon_root_anim/weapon/weapon_L_IK_marker"),
            vest_marker: rig.bone("Weapon_root/Weapon_root_anim/weapon/weapon_vest_IK_marker"),
            rig,
        }
    }

    /// Reads them off the rig as an animator of it has just posed it.
    pub fn read(&mut self, pose: &[([f32; 3], [f32; 4])]) -> RigPose {
        use trp_sim::math::{quat_mul, quat_rotate};
        self.rig.pose(pose, self.moved);
        let mut out = RigPose::default();
        if let Some(parent) = self.parent {
            out.parent = self.rig.local(parent);
        }
        if let Some(eye) = self.eye {
            out.animated = self.rig.local(eye).1;
        }
        let Some(moved) = self.moved else { return out };
        // A bone in the moved bone's own axes.
        let (at, turn) = self.rig.placed(moved);
        let back = [-turn[0], -turn[1], -turn[2], turn[3]];
        let within = |bone: usize| {
            let (bone_at, bone_turn) = self.rig.placed(bone);
            (quat_rotate(back, std::array::from_fn(|k| bone_at[k] - at[k])), quat_mul(back, bone_turn))
        };
        if let Some(marker) = self.marker {
            out.marker = within(marker);
        }
        if let (Some(left), Some(vest)) = (self.left_marker, self.vest_marker) {
            let (left, vest) = (self.rig.placed(left).0, self.rig.placed(vest).0);
            out.vest_reach = Some((0..3).map(|k| (left[k] - vest[k]).powi(2)).sum::<f32>().sqrt());
        }
        if let Some(sights) = &self.sights {
            if let Some((bone, muzzle)) = sights.in_moved(&self.rig) {
                (out.aim_bone, out.muzzle) = (Some(bone), muzzle);
            }
            out.sight_line = sights.line(&self.rig);
        }
        out
    }
}

/// The weapon-motion simulation for the weapon in hand.
#[derive(Resource)]
pub struct HandsRig {
    pub hands: trp_sim::Hands,
    /// The weapon's trigger, rounds and reloading, with its animator,
    /// when that could be read; and for the rig of the empty hands, their
    /// own controller on their own animator.
    pub firearm: Option<trp_sim::Firearm>,
    pub empty: Option<trp_sim::EmptyHands>,
    /// The rig with no engine, for what the simulation reads off it.
    pub frame: RigFrame,
    pub aiming: bool,
    /// The weapon's own factor on mouse sensitivity while aiming.
    pub aim_sensitivity: f32,
    /// The weapon's ergonomic weight, which aiming costs the arms by.
    pub aim_mass: f32,
    /// How many cells wide the body's animator is told the thing is.
    pub width: f32,
    /// How many cells wide the weapon is with its stock folded, if it has
    /// one that folds: what the body's animator is then told.
    pub folded_width: Option<f32>,
}

impl HandsRig {
    /// The animator of whatever this rig is: the weapon's, or the empty
    /// hands'.
    pub fn animator(&self) -> Option<&trp_sim::weapon_animator::WeaponAnimator> {
        self.firearm.as_ref().map(|firearm| firearm.animator()).or(self.empty.as_ref().map(|empty| empty.animator()))
    }
}

/// What stands by for the hands: every rig that is not in them just now,
/// each with its clips and its weapon motion. **A weapon's rig is made
/// ready on another thread** (`Preparing`: the crate's weapons from the
/// run's start, one at a time, a weapon hung before its turn first) and
/// put into the world when it is ready (`RigBuilder::finish`; the run's
/// first weapon at the start), and the
/// game's empty hands are a rig like a weapon's (inventory spec, "Empty
/// hands"), so the hands always hold one of them. The resources
/// `WeaponRig` and `HandsRig` are those of whichever is in the hands, and
/// everything that draws or steps the hands goes by them; taking a weapon
/// in hand or putting it away changes one set for another.
#[derive(Resource)]
pub struct Stowed {
    /// The rigs standing by, each with the item template of the weapon it
    /// is, or none for the empty hands.
    standing: Vec<(Option<String>, WeaponRig, HandsRig)>,
    /// What is in the hands: a weapon's template, or none for the empty
    /// hands.
    held: Option<String>,
    /// Whether it is the empty hands that stand by, and a weapon that is
    /// in hand.
    pub empty: bool,
    /// The templates of the weapons a rig was built for, whose entities
    /// are in the world: those the hands may take.
    pub built: Vec<String>,
    /// Those whose rig was put into the world this frame: its entities are
    /// there from the end of the frame, and the hands may take it from
    /// the next. (Taken in the same frame, the weapon was shown before its
    /// entities were there, and hidden by its own making after: floating
    /// hands until it was put away and drawn again.)
    fresh: Vec<String>,
    /// Those a rig could not be built for (a kind of weapon whose logic
    /// is not written, a model that is not there): not tried again.
    failed: Vec<String>,
    /// What a rig built later hangs from and shares with the first: the
    /// view's pivot, the first rig's nodes by their paths (the arms' bones
    /// and the camera's are every rig's), and the arms' solver.
    pivot: Entity,
    shared: HashMap<String, Entity>,
    limbs: Option<[trp_data::player::LimbData; 2]>,
    /// The field-of-view setting as it stands, for a rig built later.
    field_of_view: f32,
}

/// What putting a weapon's rig into the world needs of the app: the
/// stores of meshes, pictures and materials.
#[derive(SystemParam)]
pub struct RigBuilder<'w, 's> {
    commands: Commands<'w, 's>,
    meshes: ResMut<'w, Assets<Mesh>>,
    images: ResMut<'w, Assets<Image>>,
    materials: ResMut<'w, Assets<StandardMaterial>>,
}

impl RigBuilder<'_, '_> {
    /// Puts a rig made ready on another thread into the world, to stand
    /// by out of sight until it is taken in hand: its own weapon bone and
    /// what hangs from it, with the arms' bones shared with the first rig.
    /// Its entities are there from the end of the frame; the hands may
    /// take it from the next (`Stowed::fresh`).
    fn finish(&mut self, stowed: &mut Stowed, prepared: Prepared) {
        let began = std::time::Instant::now();
        let Prepared { template, weapon, mut hands, looks, parts, extra, kind, took } = prepared;
        let (meshes, materials) = looks.add(&mut self.meshes, &mut self.images, &mut self.materials);
        let drawn = |(mesh, material): &(usize, usize)| (meshes[*mesh].clone(), materials[*material].clone());
        let parts: Vec<Vec<(Handle<Mesh>, Handle<StandardMaterial>)>> = parts.iter().map(|node| node.iter().map(drawn).collect()).collect();
        let extra: Vec<(usize, Handle<Mesh>, Handle<StandardMaterial>)> = extra.iter().map(|(node, mesh, material)| (*node, meshes[*mesh].clone(), materials[*material].clone())).collect();
        let tree = spawn_nodes(&mut self.commands, &weapon.hierarchy, stowed.pivot, Some(&stowed.shared), &parts, &extra);
        info!(
            "{}: {} nodes, {} meshes, standing by, made ready in {:.2} s away from the frame (assembled {:.2}, its logic and animator {:.2}, its meshes and pictures {:.2}); put into the world in {:.1} ms",
            weapon.name,
            weapon.hierarchy.nodes.len(),
            tree.rendered,
            took.iter().sum::<f32>(),
            took[0],
            took[1],
            took[2],
            began.elapsed().as_secs_f32() * 1000.0
        );
        let rig = rig_of(&weapon.hierarchy, &tree.entities, &tree.by_path, stowed.pivot, stowed.limbs, weapon.cell_width as f32, kind);
        if let Some(root) = rig.weapon_root {
            self.commands.entity(root).insert(Visibility::Hidden);
        }
        // A weapon that lies in the crate has no round in its chamber (the
        // user, 2026-10-12), and works its action when it is first taken
        // in hand. Its view is the field of view as it now stands.
        if let Some(firearm) = &mut hands.firearm {
            firearm.found();
        }
        hands.hands.set_field_of_view(stowed.field_of_view);
        stowed.standing.push((Some(template.clone()), rig, hands));
        stowed.fresh.push(template);
    }
}

/// A weapon's rig made ready away from the frame, all but its entities:
/// the weapon as assembled, its motion and logic, and its meshes,
/// pictures and materials as the engine's own values. For each node of
/// the weapon's tree, its meshes (each with its material) by their places
/// in `looks`; and a weapon of one's own's parts, each with its node.
pub struct Prepared {
    template: String,
    weapon: trp_data::weapon::Weapon,
    hands: HandsRig,
    looks: PreparedLooks,
    parts: Vec<Vec<(usize, usize)>>,
    extra: Vec<(usize, usize, usize)>,
    kind: f32,
    /// Seconds: assembling it, its logic and animator, its meshes and
    /// pictures.
    took: [f32; 3],
}

/// What the thread that makes rigs ready starts from, the same for every
/// rig of the run.
#[derive(Clone)]
struct Seed {
    install: trp_data::Install,
    hard: trp_data::hard_settings::HardSettings,
    engine: trp_data::engine::EngineSettings,
    hands_data: Option<trp_data::player::HandsData>,
    view: trp_sim::ViewSettings,
    mastering: u32,
    weapon_skill: u32,
}

/// That thread's own reader of the install and its own database, made
/// there the first time and kept from one rig to the next (the bundles a
/// rig loaded stay loaded for the next that shares them).
struct Worker {
    unity: trp_data::unity::Assets,
    db: trp_data::Database,
    seed: Seed,
}

impl Worker {
    fn new(seed: &Seed) -> anyhow::Result<Self> {
        let began = std::time::Instant::now();
        let worker = Self { unity: trp_data::unity::Assets::new(seed.install.clone())?, db: trp_data::Database::load(&seed.install)?, seed: seed.clone() };
        info!("the thread that makes rigs ready: its reader of the install and its database in {:.2} s", began.elapsed().as_secs_f32());
        Ok(worker)
    }

    /// A weapon's rig, all but its entities.
    fn prepare(&mut self, template: &str) -> anyhow::Result<Prepared> {
        let began = std::time::Instant::now();
        let Seed { hard, engine, hands_data, view, mastering, weapon_skill, .. } = &self.seed;
        let weapon = trp_data::weapon::assemble(&mut self.unity, &self.db, template)?;
        let assembled = began.elapsed().as_secs_f32();
        let hands = hands_of(&mut self.unity, &self.db, hard, engine, hands_data.clone(), &weapon, *view, *mastering, *weapon_skill)
            .ok_or_else(|| anyhow::anyhow!("its motion could not be made"))?;
        // A weapon with no logic of its own cannot be worked.
        anyhow::ensure!(hands.firearm.is_some(), "it has no logic of its kind");
        let animated = began.elapsed().as_secs_f32();
        let mut looks = PreparedLooks::default();
        let mut parts = vec![Vec::new(); weapon.hierarchy.nodes.len()];
        for (index, node) in weapon.hierarchy.nodes.iter().enumerate() {
            let Some(renderer) = &node.renderer else { continue };
            match looks.mesh(&self.unity, renderer.mesh) {
                Ok(meshes) => {
                    for (slot, mesh) in meshes.into_iter().enumerate() {
                        parts[index].push((mesh, looks.material(&self.unity, renderer.materials.get(slot).copied().flatten())));
                    }
                }
                Err(e) => warn!("mesh on {}: {e:#}", node.name),
            }
        }
        let mut extra = Vec::new();
        if let Some(custom) = weapon.custom.as_deref() {
            let materials = looks.custom_materials(&custom.materials);
            for part in &custom.parts {
                for (slot, mesh) in looks.custom_mesh(&part.mesh).into_iter().enumerate() {
                    if let Some(material) = part.materials.get(slot).and_then(|index| materials.get(*index)) {
                        extra.push((part.node, mesh, *material));
                    }
                }
            }
        }
        let at = began.elapsed().as_secs_f32();
        let kind = kind_of(&self.db, template);
        Ok(Prepared { template: template.to_owned(), weapon, hands, looks, parts, extra, kind, took: [assembled, animated - assembled, at - animated] })
    }
}

/// Rigs being made ready on another thread, one at a time, in the order
/// asked: every weapon the run lays in the crate, from its start, and a
/// weapon hung where the hands can take it before its turn first of all
/// (`ask`). **Nothing of a rig's making is done in the frame** but putting
/// it into the world (`RigBuilder::finish`), so taking a weapon from the
/// crate does not stop the game for it: built in the frame, a rig had
/// cost 0.1 to 0.8 s there. The thread's reader is let go when nothing is
/// left to make.
#[derive(Resource)]
pub struct Preparing {
    queue: VecDeque<String>,
    seed: Seed,
    worker: Option<Worker>,
    task: Option<(String, bevy::tasks::Task<(Option<Worker>, anyhow::Result<Prepared>)>)>,
}

impl Preparing {
    /// Whether a rig is being made ready, or waits to be.
    pub fn busy(&self) -> bool {
        self.task.is_some() || !self.queue.is_empty()
    }

    /// Asks for a weapon's rig before every other waiting.
    pub fn ask(&mut self, template: &str) {
        if self.task.as_ref().is_some_and(|(making, _)| making == template) || self.queue.front().is_some_and(|first| first == template) {
            return;
        }
        self.queue.retain(|waiting| waiting != template);
        self.queue.push_front(template.to_owned());
    }
}

/// Each frame: what was put into the world on an earlier frame may be
/// taken in hand from now; a rig made ready is put into the world; and
/// the next waiting is begun, if none is being made.
pub fn prepare_rigs(preparing: Option<ResMut<Preparing>>, stowed: Option<ResMut<Stowed>>, mut builder: RigBuilder) {
    let (Some(mut preparing), Some(mut stowed)) = (preparing, stowed) else { return };
    let stowed = &mut *stowed;
    if !stowed.fresh.is_empty() {
        let fresh = std::mem::take(&mut stowed.fresh);
        stowed.built.extend(fresh);
    }
    let preparing = &mut *preparing;
    let done = preparing.task.as_mut().and_then(|(_, task)| bevy::tasks::block_on(bevy::tasks::futures_lite::future::poll_once(task)));
    if let Some((worker, made)) = done {
        let (template, _) = preparing.task.take().expect("a task was polled");
        preparing.worker = worker;
        match made {
            Ok(prepared) => builder.finish(stowed, prepared),
            Err(e) => {
                warn!("the weapon {template} is not built: {e:#}");
                stowed.failed.push(template);
            }
        }
    }
    if preparing.task.is_some() {
        return;
    }
    while let Some(template) = preparing.queue.pop_front() {
        if stowed.has_rig(&template) || stowed.failed.contains(&template) {
            continue;
        }
        let (worker, seed, making) = (preparing.worker.take(), preparing.seed.clone(), template.clone());
        let task = bevy::tasks::AsyncComputeTaskPool::get().spawn(async move {
            match worker.map_or_else(|| Worker::new(&seed), Ok) {
                Ok(mut worker) => {
                    let made = worker.prepare(&making);
                    (Some(worker), made)
                }
                Err(e) => (None, Err(e)),
            }
        });
        preparing.task = Some((template, task));
        return;
    }
    // Nothing left to make: the thread's reader and its bundles go.
    preparing.worker = None;
}

/// A rig of the hands' as the engine has it: where its own weapon bone
/// and the arms' bones are.
fn rig_of(
    tree: &Hierarchy,
    entities: &[Entity],
    by_path: &HashMap<String, Entity>,
    pivot: Entity,
    limbs: Option<[trp_data::player::LimbData; 2]>,
    width: f32,
    kind: f32,
) -> WeaponRig {
    WeaponRig {
        root: entities[0],
        pivot,
        weapon_root: by_path.get("Weapon_root").copied(),
        weapon_root_anim: by_path.get("Weapon_root/Weapon_root_anim").copied(),
        arms: ['L', 'R'].into_iter().filter_map(|side| crate::arms::ArmRig::find(tree, entities, side)).collect(),
        limbs,
        by_hash: by_path.iter().map(|(path, entity)| (path_hash(path), *entity)).collect(),
        width,
        kind,
        bones: entities.to_vec(),
    }
}

/// What kind of thing the body's animator is told a weapon is. The game
/// takes anything that is not a pistol, a knife, a thing thrown or a
/// launcher for a rifle (`Player.GetWeaponAnimationType`); a revolver is
/// a pistol to it.
fn kind_of(db: &trp_data::Database, template: &str) -> f32 {
    let pistol = db.items.get(template).is_some_and(|item| db.is_a(item, "Pistol") || db.is_a(item, "Revolver"));
    if pistol { trp_sim::body::hands_kind::PISTOL } else { trp_sim::body::hands_kind::RIFLE }
}

impl Stowed {
    /// Whether a weapon has a rig, in the world already (or put there this
    /// frame).
    pub fn has_rig(&self, template: &str) -> bool {
        self.built.iter().chain(&self.fresh).any(|built| built == template)
    }

    /// Whether a weapon's rig is still to come: not there yet, and not
    /// one that could not be made.
    pub fn awaited(&self, template: &str) -> bool {
        !self.built.iter().any(|built| built == template) && !self.failed.iter().any(|failed| failed == template)
    }

    /// Changes what is in the hands for the rig of a weapon, or for the
    /// empty hands'. Whether there is such a rig; it is in the hands then
    /// (also if it was already).
    pub fn change_to(&mut self, what: Option<&str>, rig: &mut WeaponRig, hands: &mut HandsRig) -> bool {
        if self.held.as_deref() == what {
            return true;
        }
        let Some(at) = self.standing.iter().position(|(is, ..)| is.as_deref() == what) else { return false };
        let (_, standing_rig, standing_hands) = &mut self.standing[at];
        std::mem::swap(standing_rig, rig);
        std::mem::swap(standing_hands, hands);
        self.standing[at].0 = std::mem::replace(&mut self.held, what.map(str::to_owned));
        self.empty = self.held.is_some() && self.standing.iter().any(|(is, ..)| is.is_none());
        true
    }

    /// The field-of-view setting has been changed: every rig that stands
    /// by takes it, as the one in the hands has.
    pub fn set_field_of_view(&mut self, degrees: f32) {
        self.field_of_view = degrees;
        for (_, _, hands) in &mut self.standing {
            hands.hands.set_field_of_view(degrees);
        }
    }

    /// The weapon in the hands, by its item template.
    pub fn held(&self) -> Option<&str> {
        self.held.as_deref()
    }

}

/// One rig's entities: every node in the order of its prefab, and each by
/// its path from the animated root.
struct Tree {
    entities: Vec<Entity>,
    by_path: HashMap<String, Entity>,
    rendered: usize,
}

/// The entity of a rig already there that a node of another's is, where
/// the two prefabs have it alike outside the weapon's own tree.
fn shared_node(hierarchy: &Hierarchy, index: usize, shared: Option<&HashMap<String, Entity>>) -> Option<Entity> {
    let path = hierarchy.path(index);
    shared.and_then(|shared| shared.get(&path).copied()).filter(|_| !path.starts_with("Weapon_root"))
}

/// Spawns a rig's nodes under the view's pivot, with their meshes. With
/// `shared`, the nodes of a rig already there are used for every path the
/// two have alike outside the weapon's own tree (the arms' bones and the
/// camera's: the prefabs name them alike, and the body's meshes are
/// skinned to them once), so that they are posed by whichever rig is in
/// the hands; the weapon's own tree is this rig's alone.
#[allow(clippy::too_many_arguments)]
fn spawn_tree(
    commands: &mut Commands,
    unity: &mut trp_data::unity::Assets,
    converted: &mut Converted,
    meshes: &mut Assets<Mesh>,
    images: &mut Assets<Image>,
    materials: &mut Assets<StandardMaterial>,
    hierarchy: &Hierarchy,
    pivot: Entity,
    shared: Option<&HashMap<String, Entity>>,
    custom: Option<&trp_data::custom::Build>,
) -> Tree {
    // What each node draws, made here, on the frame's own thread.
    let mut parts: Vec<Vec<(Handle<Mesh>, Handle<StandardMaterial>)>> = vec![Vec::new(); hierarchy.nodes.len()];
    for (index, node) in hierarchy.nodes.iter().enumerate() {
        let Some(renderer) = node.renderer.as_ref().filter(|_| shared_node(hierarchy, index, shared).is_none()) else { continue };
        match converted.mesh(unity, renderer.mesh, meshes) {
            Ok(entry) => {
                for (slot, mesh) in entry.parts.iter().enumerate() {
                    let material = converted.material(unity, renderer.materials.get(slot).copied().flatten(), images, materials);
                    parts[index].push((mesh.clone(), material));
                }
            }
            Err(e) => warn!("mesh on {}: {e:#}", node.name),
        }
    }
    // A weapon that is not the install's: the parts of its own model,
    // each on the node made for it, with its own pictures.
    let mut extra = Vec::new();
    if let Some(custom) = custom {
        let looks = converted.custom_materials(&custom.materials, images, materials);
        for part in &custom.parts {
            for (slot, mesh) in converted.custom_mesh(&part.mesh, meshes).into_iter().enumerate() {
                let Some(material) = part.materials.get(slot).and_then(|index| looks.get(*index)) else { continue };
                extra.push((part.node, mesh, material.clone()));
            }
        }
    }
    spawn_nodes(commands, hierarchy, pivot, shared, &parts, &extra)
}

/// Spawns a rig's nodes under the view's pivot, each drawing what `parts`
/// has for it (a mesh with its material, one or more), and `extra` on the
/// nodes it names (a weapon of one's own's parts). Shared nodes as for
/// `spawn_tree`.
fn spawn_nodes(
    commands: &mut Commands,
    hierarchy: &Hierarchy,
    pivot: Entity,
    shared: Option<&HashMap<String, Entity>>,
    parts: &[Vec<(Handle<Mesh>, Handle<StandardMaterial>)>],
    extra: &[(usize, Handle<Mesh>, Handle<StandardMaterial>)],
) -> Tree {
    let mut entities: Vec<Entity> = Vec::with_capacity(hierarchy.nodes.len());
    let mut by_path = HashMap::new();
    let mut rendered = 0;
    for (index, node) in hierarchy.nodes.iter().enumerate() {
        let path = hierarchy.path(index);
        if let Some(entity) = shared_node(hierarchy, index, shared) {
            entities.push(entity);
            by_path.insert(path, entity);
            continue;
        }
        let visibility = if node.active { Visibility::Inherited } else { Visibility::Hidden };
        let mut transform = convert::transform(node.translation, node.rotation, node.scale);
        if index == 0 {
            // The rig's own origin is the view pivot's: the game puts it
            // at the body's ribcage every frame.
            transform.translation = Vec3::ZERO;
        }
        let entity = commands.spawn((Name::new(node.name.clone()), transform, visibility)).id();
        commands.entity(node.parent.map_or(pivot, |p| entities[p])).add_child(entity);
        entities.push(entity);
        by_path.insert(path, entity);
        let own = &parts[index];
        for (mesh, material) in own {
            let bundle = (Mesh3d(mesh.clone()), MeshMaterial3d(material.clone()));
            if own.len() == 1 {
                commands.entity(entity).insert(bundle);
            } else {
                let part = commands.spawn((bundle, Transform::default())).id();
                commands.entity(entity).add_child(part);
            }
            rendered += 1;
        }
    }
    for (node, mesh, material) in extra {
        let drawn = commands.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(material.clone()), Transform::default())).id();
        commands.entity(entities[*node]).add_child(drawn);
        rendered += 1;
    }
    Tree { entities, by_path, rendered }
}

/// A weapon's motion and logic, from the install: its springs, its
/// recoil, its animator and the operations run on it.
fn hands_of(
    unity: &mut trp_data::unity::Assets,
    db: &trp_data::Database,
    hard: &trp_data::hard_settings::HardSettings,
    engine: &trp_data::engine::EngineSettings,
    hands_data: Option<trp_data::player::HandsData>,
    weapon: &trp_data::weapon::Weapon,
    view: trp_sim::ViewSettings,
    mastering: u32,
    weapon_skill: u32,
) -> Option<HandsRig> {
    let Some(data) = hands_data else {
        warn!("no weapon motion data");
        return None;
    };
    let settings = match trp_sim::hands_settings(data, db, hard, engine, weapon, view) {
        Ok(settings) => settings,
        Err(e) => {
            warn!("weapon motion settings of {}: {e:#}", weapon.name);
            return None;
        }
    };
    // Aiming costs the arms by the weapon's ergonomic weight.
    let aim_mass = settings.ergonomic_weight();
    let hands = trp_sim::Hands::new(settings);
    info!("{}: {:.2} kg, ergonomics {}: time to aim {:.2} s", weapon.name, weapon.total_weight, weapon.total_ergonomics, 1.0 / hands.aiming_speed);
    // The weapon's logic goes by the weapon's own animator.
    let animator = trp_sim::weapon_animator::WeaponAnimatorData::load(unity, weapon);
    let firearm = match (trp_sim::firearm_settings(unity, db, hard, weapon), animator) {
        (Ok(settings), Ok(animator)) => {
            info!(
                "  {} rounds a minute, {} for single shots, magazine of {}; its animator has {} clips",
                settings.fire_rate,
                settings.single_fire_rate,
                settings.magazine_capacity,
                animator.clip_count()
            );
            let (reload_speed, draw_speed) = trp_sim::firearm::weapon_skill_speeds(weapon_skill);
            Some(trp_sim::Firearm::new(trp_sim::FirearmSettings { mastering, reload_speed, draw_speed, ..settings }, &animator))
        }
        (Err(e), _) | (_, Err(e)) => {
            warn!("weapon logic of {}: {e:#}", weapon.name);
            None
        }
    };
    let aim_sensitivity = db.items.get(&weapon.template).and_then(|t| t.prop_f32("AimSensitivity")).unwrap_or(1.0);
    Some(HandsRig {
        hands,
        firearm,
        empty: None,
        frame: RigFrame::new(&weapon.hierarchy),
        aiming: false,
        aim_sensitivity,
        aim_mass,
        width: weapon.cell_width as f32,
        folded_width: weapon.fold.map(|fold| fold.cell_width as f32),
    })
}

pub fn spawn_weapon(
    mut commands: Commands,
    mut game: ResMut<GameData>,
    options: Res<Options>,
    mut converted: ResMut<Converted>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut bindposes: ResMut<Assets<SkinnedMeshInverseBindposes>>,
    mut clip_player: ResMut<ClipPlayer>,
    mut controller: ResMut<crate::controller::Controller>,
) {
    let GameData { unity, db, hard, engine, hands_data } = &mut *game;
    let weapon = match trp_data::weapon::assemble(unity, db, &options.weapon) {
        Ok(weapon) => weapon,
        Err(e) => {
            error!("could not assemble weapon {}: {e:#}", options.weapon);
            return;
        }
    };
    for skipped in &weapon.skipped {
        warn!("part not placed: {skipped}");
    }

    let player = commands.spawn((Name::new("Player"), Player, Transform::default(), Visibility::default())).id();
    let pivot = commands.spawn((Name::new("View"), ViewPivot, Transform::default(), Visibility::default())).id();
    commands.entity(player).add_child(pivot);
    let eye = commands.spawn((Name::new("Eye"), Eye, Transform::default(), Visibility::default())).id();
    commands.entity(pivot).add_child(eye);

    // The body's animator picks how it carries the weapon by its width.
    controller.movement.body.set_weapon_width(weapon.cell_width as f32);

    let hierarchy = &weapon.hierarchy;
    let eye_index = hierarchy.nodes.iter().position(|n| n.name == "Camera_animated");
    let Tree { entities, by_path, rendered } =
        spawn_tree(&mut commands, unity, &mut converted, &mut meshes, &mut images, &mut materials, hierarchy, pivot, None, weapon.custom.as_deref());
    info!("{}: {} nodes, {} meshes", weapon.name, hierarchy.nodes.len(), rendered);

    // The game skins the arms to the body's skeleton and makes that follow
    // the weapon's arm bones. Until the body is in, the arms are skinned to
    // the weapon's arm bones directly.
    match trp_data::arms::load(unity, &options.arms) {
        Ok(arms) => match converted.mesh(unity, arms.mesh, &mut meshes) {
            Ok(entry) => {
                let joints: Vec<Entity> = arms
                    .weapon_bone_paths()
                    .iter()
                    .map(|path| {
                        by_path.get(*path).copied().unwrap_or_else(|| {
                            warn!("arms bone {path} is not on the weapon");
                            entities[0]
                        })
                    })
                    .collect();
                let inverse_bindposes = bindposes.add(SkinnedMeshInverseBindposes::from(entry.bind_poses.clone()));
                for (slot, mesh) in entry.parts.iter().enumerate() {
                    let material_ref = arms.materials.get(slot).copied().flatten();
                    let material = converted.material(unity, material_ref, &mut images, &mut materials);
                    let part = commands
                        .spawn((
                            Name::new("Arms"),
                            FirstPersonHands,
                            Mesh3d(mesh.clone()),
                            MeshMaterial3d(material),
                            SkinnedMesh { inverse_bindposes: inverse_bindposes.clone(), joints: joints.clone() },
                            // Skinned bounds are not tracked, and the arms are always in view.
                            NoFrustumCulling,
                            // The game's hands cast no shadow: their
                            // renderers are set so, and nothing changes
                            // it (recorded, tenth recording: drawn,
                            // shadow casting off). The arms' shadow on
                            // the ground is the upper body's sleeves.
                            bevy::pbr::NotShadowCaster,
                        ))
                        .id();
                    commands.entity(entities[0]).add_child(part);
                }
            }
            Err(e) => warn!("arms mesh: {e:#}"),
        },
        Err(e) => warn!("arms {}: {e:#}", options.arms),
    }

    match clips_of(unity, &weapon) {
        Some((library, idle, _)) => {
            clip_player.idle = idle;
            info!("{} animation clips, idle is {}", library.clips.len(), clip_player.idle);
            commands.insert_resource(library);
        }
        None => warn!("the weapon has no animation clips"),
    }

    let view = trp_sim::ViewSettings {
        field_of_view: options.field_of_view,
        head_bobbing: options.head_bobbing,
        seed: options.seed,
        aim_speed_bonus: options.aim_speed_bonus,
    };
    let in_hand = hands_of(unity, db, hard, engine, hands_data.clone(), &weapon, view, options.mastering, options.weapon_skill);
    if let Some(hands) = &in_hand {
        controller.movement.stamina.aim_mass = hands.aim_mass;
    }

    if let Some(i) = eye_index {
        // The game takes a half turn about x as this bone's rest.
        info!("camera bone rests at {:?}", hierarchy.nodes[i].rotation);
    }
    let limbs = trp_data::player::LimbData::load(unity).and_then(|limbs| {
        limbs.iter().try_for_each(trp_sim::arms::check)?;
        Ok(limbs)
    });
    if let Err(e) = &limbs {
        warn!("the arms' solver: {e:#}");
    }
    let limbs = limbs.ok();
    let rig = rig_of(hierarchy, &entities, &by_path, pivot, limbs, weapon.cell_width as f32, kind_of(db, &options.weapon));

    // Every other weapon's rig is made ready on another thread, the
    // crate's in their order from now, a weapon hung before its turn first
    // (`Preparing`).
    let mut standing = Vec::new();
    let built = vec![options.weapon.clone()];
    commands.insert_resource(Preparing {
        queue: options.weapons.iter().filter(|weapon| **weapon != options.weapon).cloned().collect(),
        seed: Seed { install: unity.install.clone(), hard: hard.clone(), engine: *engine, hands_data: hands_data.clone(), view, mastering: options.mastering, weapon_skill: options.weapon_skill },
        worker: None,
        task: None,
    });

    // The hands' other rig: the game's empty hands. Its arms and its
    // camera bone are the weapon rig's own entities; its weapon bone, with
    // the markers the arms are solved onto, is its own and hangs beside
    // the weapons'.
    let empty = match trp_data::weapon::assemble_prefab(unity, trp_data::weapon::EMPTY_HANDS) {
        Ok(empty) => Some(empty),
        Err(e) => {
            warn!("the empty hands' rig: {e:#}");
            None
        }
    };
    let empty_hands = empty.and_then(|empty| {
        let tree = spawn_tree(&mut commands, unity, &mut converted, &mut meshes, &mut images, &mut materials, &empty.hierarchy, pivot, Some(&by_path), None);
        // Their controller, on their own animator.
        let animator = match trp_sim::weapon_animator::WeaponAnimatorData::load(unity, &empty) {
            Ok(animator) => animator,
            Err(e) => {
                warn!("the empty hands' animator: {e:#}");
                return None;
            }
        };
        let data = hands_data.clone()?;
        let settings = match trp_sim::empty_hands_settings(data, db, hard, engine, &empty, view) {
            Ok(settings) => settings,
            Err(e) => {
                warn!("the empty hands' motion settings: {e:#}");
                return None;
            }
        };
        info!("empty hands: {} bones, {} clips", empty.hierarchy.nodes.len(), animator.clip_count());
        let rig = rig_of(&empty.hierarchy, &tree.entities, &tree.by_path, pivot, limbs, empty.cell_width as f32, trp_sim::body::hands_kind::EMPTY_HANDS);
        let hands = HandsRig {
            hands: trp_sim::Hands::new(settings),
            firearm: None,
            empty: Some(trp_sim::EmptyHands::new(&animator)),
            frame: RigFrame::new(&empty.hierarchy),
            aiming: false,
            aim_sensitivity: 1.0,
            aim_mass: 0.0,
            width: empty.cell_width as f32,
            folded_width: None,
        };
        Some((None, rig, hands))
    });
    let empty = empty_hands.is_some();
    standing.extend(empty_hands);
    if let Some(hands) = in_hand {
        commands.insert_resource(hands);
        commands.insert_resource(Stowed {
            standing,
            held: Some(options.weapon.clone()),
            empty,
            built,
            fresh: Vec::new(),
            failed: Vec::new(),
            pivot,
            shared: by_path.clone(),
            limbs,
            field_of_view: options.field_of_view,
        });
    }
    commands.insert_resource(rig);
}

/// The clips of a weapon or of a rig like one, by the names its animator
/// uses, with the one it rests by: for looking at one clip by its name
/// (`--play`). In play the rig is posed by its animator, not from here.
fn clips_of(unity: &trp_data::unity::Assets, weapon: &trp_data::weapon::Weapon) -> Option<(ClipLibrary, String, Option<(String, [f32; 2])>)> {
    let mut clips: HashMap<String, _> = match weapon.clips(unity) {
        Ok(clips) => clips.into_iter().collect(),
        Err(e) => {
            warn!("animation clips of {}: {e:#}", weapon.name);
            return None;
        }
    };
    let mut idle = String::new();
    match trp_sim::rest_clip(unity, weapon) {
        Ok((name, clip)) => {
            clips.insert(name.clone(), clip);
            idle = name;
        }
        Err(e) => warn!("resting clip of {}: {e:#}", weapon.name),
    }
    let other = trp_sim::rest_blend(unity, weapon).ok().flatten().map(|(name, clip, values)| {
        clips.insert(name.clone(), clip);
        (name, values)
    });
    Some((ClipLibrary { clips }, idle, other))
}
