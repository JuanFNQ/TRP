//! The dummies on the range (`docs/spec/body-hits.md`, "The dummy on the
//! range"; the options' `DUMMY`): PMCs down the lanes, each a copy of the
//! body on a skeleton of its own (`figure_view::Builder::dummy`) moved and
//! posed by a character of its own seen from outside with an AKM in its
//! hands (`trp_sim::puppet`: the same simulation as the player's), the
//! AKM's model in its hands, the player prefab's hit colliders on its bones,
//! the armour of a fresh copy of what the player wears (the debug panel's
//! DRESS AS YOU, MADE WHOLE dresses them again and makes them whole), a
//! PMC's health, and its death as the
//! game's: the ragdoll on the game's physics engine, the weapon let go on a
//! spring, the killing hit's push the frame after. One stands; the other
//! walks to its right and its left in turn. The bullets meet their colliders
//! (`Dummies::ray`), ask their armour (`BulletWorld::wearer`) and hand each
//! its hits (`Dummies::take`), all in the game's axes.

use std::collections::HashMap;
use std::sync::Arc;

use bevy::camera::primitives::Aabb;
use bevy::math::Affine3A;
use bevy::mesh::skinning::{SkinnedMesh, SkinnedMeshInverseBindposes};
use bevy::prelude::*;
use bevy::transform::helper::TransformHelper;
use trp_data::player::{HandsData, LimbData};
use trp_data::ragdoll::{HitColliderData, Shape};
use trp_physics::{Body, BodyDesc, Collider as PhysicsCollider, Material, Shape as PhysicsShape, World};
use trp_sim::armor::{self, ArmorRules, VestSkills, Wearer, Worn};
use trp_sim::ballistics::{material, BodyRef, Hit, RayHit, Surface, V3};
use trp_sim::health::{Health, HealthSettings, Part};
use trp_sim::puppet::{Puppet, PuppetPose};

use crate::convert;

/// Their colliders' numbers as ballistic things, apart from the range's;
/// each dummy's from its own base.
const THINGS: u64 = 1 << 40;
const THINGS_EACH: u64 = 1 << 16;

/// Where each stands (Bevy's axes), and whether it walks: down the hall's
/// lanes (x between the left wall's −4 and the knee-high wall's end at
/// 10.5), 14.4 m past the firing line (the knee-high wall at z −5.6), clear
/// of the low wall's cover, facing the firing line; the walker 4 m to the
/// first's right as the shooter sees them. This work's choice on the user's
/// range.
///
/// `PROVISIONAL`: this work's place and facing on the app's range (the game's
/// is not followed here); its armour's hinged piece counted as down (the
/// player's face shield's state not copied: `worn_of` carries no toggle), its
/// helmet's hinge drawn at the model's rest, the player's gear copied whole.
pub struct DummyPlace {
    pub at: Vec3,
    pub walks: bool,
}

/// (Nearer the firing line and to its left since the user's range of
/// 2026-10-26: before, 20 m out at 3.25 and 7.25.)
pub const PLACES: [DummyPlace; 2] = [DummyPlace { at: Vec3::new(-2.5, 0.0, -15.0), walks: false }, DummyPlace { at: Vec3::new(1.3, 0.0, -15.0), walks: true }];

/// How far either way of its place the walker goes before it turns back
/// (this work's).
const WALK_SPAN: f32 = 2.0;
/// The way they face: the game's yaw, half round (toward the firing line).
const FACING: f32 = 180.0;
/// The weapon in their hands: the AKM, the run's default.
pub const WEAPON: &str = "59d6088586f774275f37482f";

/// A dummy's place on the range, facing the firing line.
pub fn place(which: usize) -> Transform {
    Transform::from_translation(PLACES[which].at).with_rotation(Quat::from_rotation_y(std::f32::consts::PI))
}

/// A piece of a dummy drawn as a piece of the body is: shown as that one
/// was when the dummies were last dressed (a suit as it is now).
#[derive(Component)]
pub struct DummyTwin {
    pub original: Entity,
}

/// The dummies' skeletons, kept for pieces put on later (`clothing`).
#[derive(Resource)]
pub struct DummySkeletons(pub Vec<crate::figure_view::Skeleton>);

/// A dummy as built (`figure_view::Builder::finish_dummy`): its root and
/// bones by path (a parent before what hangs from it), each bone's place
/// in its parent as it stands, and its outfits' skeletons.
pub struct DummyRig {
    pub root: Entity,
    pub paths: Vec<String>,
    pub bones: Vec<Entity>,
    pub parents: Vec<Option<usize>>,
    pub rest: Vec<Transform>,
    pub outfits: Vec<crate::death::CorpseOutfit>,
}

impl DummyRig {
    fn bone(&self, name: &str) -> Option<usize> {
        self.paths.iter().position(|path| path.rsplit('/').next() == Some(name))
    }
}

/// Every dummy's, by its number.
#[derive(Resource)]
pub struct DummyRigs(pub Vec<DummyRig>);

/// What a dummy's character is made of, read at the start (`main`): the
/// weapon in its hands, assembled, with its animator, its logic's settings
/// and the motion's data; the movement's settings and the body's animator;
/// the arms' solver; the player's frame rate and the engine's step.
pub struct PuppetMaker {
    pub weapon: Arc<trp_data::weapon::Weapon>,
    pub animator: trp_sim::weapon_animator::WeaponAnimatorData,
    pub firearm: trp_sim::FirearmSettings,
    pub hands_data: HandsData,
    pub view: trp_sim::ViewSettings,
    pub movement: trp_sim::MovementSettings,
    pub body: trp_sim::body::BodyData,
    pub limbs: [LimbData; 2],
    pub frame_rate: f32,
    pub physics_step: f32,
    pub longest_frame: f32,
    pub smooth: bool,
}

impl PuppetMaker {
    /// A character standing at `at` (Bevy's axes), facing the firing line.
    fn make(&self, game: &crate::GameData, at: Vec3) -> anyhow::Result<Puppet> {
        let hands = trp_sim::Hands::new(trp_sim::hands_settings(self.hands_data.clone(), &game.db, &game.hard, &game.engine, &self.weapon, self.view)?);
        let firearm = trp_sim::Firearm::new(self.firearm.clone(), &self.animator);
        let width = self.weapon.cell_width as f32;
        let mut movement = trp_sim::Movement::new(self.movement.clone(), trp_sim::Body::new(&self.body, width));
        movement.position = [at.x, at.y, -at.z];
        movement.yaw = FACING;
        movement.body_yaw = FACING;
        movement.note_rotation();
        let pacer = trp_sim::Pacer::new(self.frame_rate, self.physics_step, self.longest_frame);
        Puppet::new(movement, hands, firearm, &self.weapon.hierarchy, self.limbs, width, pacer, self.smooth)
    }
}

/// What the dummies are made with, read at the start (`main`): the player
/// prefab's hit colliders, the armour's and health's figures, the killing
/// hit's force (`HIT_FORCE`), and their characters' makings (none where
/// they could not be read: the dummies then stand as the screens stand a
/// character).
#[derive(Resource)]
pub struct DummySource {
    pub colliders: Vec<HitColliderData>,
    pub rules: Arc<ArmorRules>,
    pub health: Arc<HealthSettings>,
    pub hit_force: f32,
    pub maker: Option<PuppetMaker>,
}

/// One of a dummy's hit colliders: as the prefab has it, the bone it is on
/// and its place there (Bevy's axes), what a hit on it is, and its figures
/// as the game sets them.
struct Collider {
    data: HitColliderData,
    bone: usize,
    local: Affine3A,
    body: BodyRef,
    figures: [f32; 6],
}

/// A collider where it stands this frame, in Bevy's axes.
#[derive(Clone, Copy)]
enum Placed {
    Sphere { centre: Vec3, radius: f32 },
    Box { centre: Vec3, axes: [Vec3; 3], half: Vec3 },
    Capsule { a: Vec3, b: Vec3, radius: f32 },
}

/// The killing hit, for the push (`ApplyCorpseImpulse`): the collider it
/// was on, the bullet's way, the point, the penetration as the armour left
/// it. All in the game's axes.
#[derive(Clone, Copy)]
struct Killing {
    collider: usize,
    direction: V3,
    point: V3,
    penetration: f32,
}

/// A dummy fallen: the game's engine with its ragdoll, each bone's body (or
/// none), the push still to come, and the weapon let go.
struct Fall {
    engine: World,
    moved: Vec<Option<Body>>,
    push: Option<(Body, V3, V3)>,
    weapon: Option<Body>,
    /// The weapon heard as it lands (`landing_sound`), the time since the
    /// death, and each body's surface (its bone's hit collider's
    /// `SurfaceSound`), which a weapon landing on it plays by.
    landing: trp_sim::landing::Landing,
    since: f32,
    surfaces: HashMap<Body, u8>,
}

/// The AKM's model in a dummy's hands: the frame it hangs in (the feet
/// turned with the view, then the ribcage turned by the view's pitch), the
/// model's nodes, its root's own place there, and the weapon's own tree
/// (`Weapon_root`), which a death lets go.
struct Model {
    player: Entity,
    pivot: Entity,
    tree: Vec<Entity>,
    root_local: Transform,
    weapon_root: Option<Entity>,
}

/// One dummy's character.
pub struct Dummy {
    which: usize,
    colliders: Vec<Collider>,
    placed: Vec<Placed>,
    pub wearer: Wearer,
    pub health: Health,
    /// Dressed at least once.
    dressed: bool,
    /// This frame's hits it took: the bullet (shot, fragment, seed) and the
    /// material it was, for its effect.
    taken: Vec<((u32, i32, i32), i32)>,
    killing: Option<Killing>,
    fall: Option<Fall>,
    hits: u32,
    /// Its character, where it could be made, and the way the walker heads
    /// along the lanes (+1 toward the world's +x).
    puppet: Option<Puppet>,
    heading: f32,
    model: Option<Model>,
    /// The skeleton's bones the body's animator gives, in its order; each
    /// arm's (the collarbone, the upper arm, the three forearm bones, the
    /// palm), right then left; and the rig's finger bones with the
    /// skeleton's of the same name.
    drawn: Vec<Option<usize>>,
    arm_bones: [[Option<usize>; 6]; 2],
    fingers: Vec<(usize, usize)>,
    /// Each skeleton bone in its parent as last posed.
    locals: Vec<Transform>,
}

impl Dummy {
    fn character(&self) -> u32 {
        self.which as u32 + 1
    }

    fn base(&self) -> u64 {
        THINGS + self.which as u64 * THINGS_EACH
    }
}

/// Every dummy, by its number.
#[derive(Resource)]
pub struct Dummies {
    pub list: Vec<Dummy>,
    /// Frames since they were made: the body's pieces show what is worn a
    /// frame or two after the start.
    age: u32,
}

impl Dummies {
    /// Each living dummy's sounds of moving: its number, where its feet are
    /// (the game's axes) and its rules, whose asks are to be played and
    /// taken (`step_sound::play_dummies`).
    pub fn movers(&mut self) -> impl Iterator<Item = (usize, [f32; 3], &mut trp_sim::steps::StepSounds)> + '_ {
        self.list.iter_mut().filter(|dummy| dummy.health.alive && dummy.fall.is_none()).filter_map(|dummy| {
            let puppet = dummy.puppet.as_mut()?;
            Some((dummy.which, puppet.movement.position, &mut puppet.movement.sounds))
        })
    }

    /// Every collider a ray from `from` to `to` (the game's axes) enters
    /// through a face turned toward it, one hit each: a plate's only while
    /// armour covers it.
    pub fn ray(&self, from: V3, to: V3, hits: &mut Vec<RayHit>) {
        let o = Vec3::new(from[0], from[1], -from[2]);
        let e = Vec3::new(to[0], to[1], -to[2]);
        let length = (e - o).length();
        if length <= 0.0 {
            return;
        }
        let d = (e - o) / length;
        if std::env::var_os("TRP_DUMMY_DEBUG").is_some() && (o.z < -15.0 || e.z < -15.0) {
            info!("a ray from {o:.2?} to {e:.2?}: {} dummies, {} colliders placed", self.list.len(), self.list.iter().map(|d| d.placed.len()).sum::<usize>());
        }
        for dummy in &self.list {
            let plates = dummy.wearer.plates_on();
            for (index, (c, placed)) in dummy.colliders.iter().zip(&dummy.placed).enumerate() {
                if c.body.plate != 0 && plates & c.body.plate == 0 {
                    continue;
                }
                let Some((t, normal)) = enter(*placed, o, d, length) else { continue };
                let p = o + d * t;
                let id = dummy.base() + index as u64;
                hits.push(RayHit {
                    distance: t,
                    point: [p.x, p.y, -p.z],
                    normal: [normal.x, normal.y, -normal.z],
                    collider: id,
                    thing: id,
                    mesh: false,
                    surface: Surface { material: material::BODY, figures: c.figures },
                    body: Some(c.body),
                });
            }
        }
    }

    /// The armour of the dummy that is that character.
    pub fn wearer(&self, character: u32) -> Option<&Wearer> {
        self.list.iter().find(|d| d.character() == character).map(|d| &d.wearer)
    }

    /// A hit handed on by the ballistics (`Player.ApplyShot`): through the
    /// dummy's armour into its health, its material kept for its effect,
    /// and the killing hit kept for the push.
    pub fn take(&mut self, hit: &Hit) {
        let Some(body) = hit.body else { return };
        let Some(dummy) = self.list.iter_mut().find(|d| d.character() == body.character) else { return };
        let alive = dummy.health.alive;
        let Some(outcome) = armor::apply_shot(Some(&mut dummy.wearer), &mut dummy.health, hit) else { return };
        dummy.taken.push(((hit.shot, hit.fragment, hit.seed), outcome.taken.material));
        let index = hit.collider.checked_sub(dummy.base()).map(|i| i as usize);
        let object = index.and_then(|i| dummy.colliders.get(i)).map_or("?", |c| c.data.object.as_str());
        let parts: Vec<String> = Part::ALL.iter().map(|p| format!("{:.1}", dummy.health.part(*p).current)).collect();
        dummy.hits += 1;
        info!(
            "dummy {}'s hit {}: {} on {} ({}{}), damage {:.2} came, {:.2} through the armour ({:.2} taken by it, {}{}), penetration {:.2}; health [{}]{}",
            dummy.which,
            dummy.hits,
            outcome.part.name(),
            object,
            armor::COLLIDER_TYPES.get(body.collider as usize).copied().unwrap_or("?"),
            if body.plate != 0 { ", a plate" } else { "" },
            outcome.damage_in,
            outcome.taken.damage,
            outcome.taken.absorbed,
            material::NAMES.get(outcome.taken.material as usize).copied().unwrap_or("?"),
            match (hit.blocked_by.is_some(), hit.deflected_by.is_some()) {
                (_, true) => ", deflected",
                (true, _) => ", blocked",
                _ => "",
            },
            outcome.taken.penetration,
            parts.join(", "),
            if dummy.health.alive { "" } else { ": dead" }
        );
        if alive && !dummy.health.alive {
            if let Some(collider) = index {
                dummy.killing = Some(Killing { collider, direction: hit.direction, point: hit.point, penetration: outcome.taken.penetration });
            }
        }
    }

    /// The material a bullet's hit on a dummy was, where one took it.
    pub fn taken_by(&self, hit: &Hit) -> Option<i32> {
        self.list.iter().flat_map(|d| &d.taken).find(|(bullet, _)| *bullet == (hit.shot, hit.fragment, hit.seed)).map(|(_, m)| *m)
    }
}

/// Where a ray from `o` along the unit `d`, within `length`, first enters
/// a shape from outside (nothing it starts inside), and the face's normal.
fn enter(shape: Placed, o: Vec3, d: Vec3, length: f32) -> Option<(f32, Vec3)> {
    let found = match shape {
        Placed::Sphere { centre, radius } => sphere(o, d, centre, radius),
        Placed::Box { centre, axes, half } => {
            let p = o - centre;
            let (lo, ld) = (Vec3::new(p.dot(axes[0]), p.dot(axes[1]), p.dot(axes[2])), Vec3::new(d.dot(axes[0]), d.dot(axes[1]), d.dot(axes[2])));
            if lo.abs().cmplt(half).all() {
                return None;
            }
            let (mut near, mut far, mut face) = (f32::MIN, f32::MAX, (0usize, 0.0f32));
            for k in 0..3 {
                if ld[k].abs() < 1e-9 {
                    if lo[k].abs() > half[k] {
                        return None;
                    }
                    continue;
                }
                let (t1, t2) = ((-half[k] - lo[k]) / ld[k], (half[k] - lo[k]) / ld[k]);
                let (tn, tf, sign) = if t1 < t2 { (t1, t2, -1.0) } else { (t2, t1, 1.0) };
                if tn > near {
                    near = tn;
                    face = (k, sign);
                }
                far = far.min(tf);
            }
            (near <= far && near >= 0.0).then(|| (near, axes[face.0] * face.1))
        }
        Placed::Capsule { a, b, radius } => {
            let ab = b - a;
            let along = ab.length_squared();
            let closest = |p: Vec3| if along > 0.0 { a + ab * ((p - a).dot(ab) / along).clamp(0.0, 1.0) } else { a };
            if (o - closest(o)).length_squared() < radius * radius {
                return None;
            }
            // The first of the body and the two caps the ray meets.
            let mut best: Option<f32> = None;
            if along > 0.0 {
                let (oa, bard) = (o - a, ab.dot(d));
                let (qa, qb, qc) = (along - bard * bard, along * d.dot(oa) - ab.dot(oa) * bard, along * oa.dot(oa) - ab.dot(oa).powi(2) - radius * radius * along);
                let h = qb * qb - qa * qc;
                if qa > 1e-12 && h >= 0.0 {
                    let t = (-qb - h.sqrt()) / qa;
                    let y = ab.dot(oa) + t * bard;
                    if t >= 0.0 && y > 0.0 && y < along {
                        best = Some(t);
                    }
                }
            }
            for end in [a, b] {
                if let Some((t, _)) = sphere(o, d, end, radius) {
                    best = Some(best.map_or(t, |b: f32| b.min(t)));
                }
            }
            best.map(|t| {
                let p = o + d * t;
                (t, (p - closest(p)).normalize_or_zero())
            })
        }
    };
    found.filter(|(t, _)| *t <= length)
}

/// A ray's entry into a sphere from outside.
fn sphere(o: Vec3, d: Vec3, centre: Vec3, radius: f32) -> Option<(f32, Vec3)> {
    let oc = o - centre;
    let c = oc.length_squared() - radius * radius;
    if c < 0.0 {
        return None;
    }
    let b = oc.dot(d);
    let h = b * b - c;
    if h < 0.0 {
        return None;
    }
    let t = -b - h.sqrt();
    (t >= 0.0).then(|| (t, (o + d * t - centre) / radius))
}

/// A collider placed by its bone's place in the world.
fn placed(c: &Collider, bone: Affine3A) -> Placed {
    let m = bone * c.local;
    let axis = |k: usize| Vec3::from(m.matrix3.col(k));
    let scale = Vec3::new(axis(0).length(), axis(1).length(), axis(2).length());
    match c.data.shape {
        Shape::Sphere { centre, radius } => Placed::Sphere { centre: m.transform_point3(convert::position(centre)), radius: radius * scale.max_element() },
        Shape::Box { centre, size } => Placed::Box {
            centre: m.transform_point3(convert::position(centre)),
            axes: [axis(0).normalize_or_zero(), axis(1).normalize_or_zero(), axis(2).normalize_or_zero()],
            half: Vec3::from(size) * scale / 2.0,
        },
        Shape::Capsule { centre, radius, height, direction } => {
            let k = (direction as usize).min(2);
            let across = (0..3).filter(|j| *j != k).map(|j| scale[j]).fold(0.0f32, f32::max);
            let radius = radius * across;
            let half = (height * scale[k] / 2.0 - radius).max(0.0);
            let centre = m.transform_point3(convert::position(centre));
            let way = axis(k).normalize_or_zero();
            Placed::Capsule { a: centre - way * half, b: centre + way * half, radius }
        }
    }
}

/// A dummy's hit colliders on its bones, with the figures the game sets.
fn colliders_of(source: &DummySource, rig: &DummyRig, which: usize) -> Vec<Collider> {
    let mut colliders = Vec::new();
    for c in &source.colliders {
        let Some(bone) = rig.bone(&c.bone) else {
            warn!("dummy {which} has no bone {} for the collider {}", c.bone, c.object);
            continue;
        };
        let local = c.local.map_or(Affine3A::IDENTITY, |(place, turn)| convert::transform(place, turn, [1.0; 3]).compute_affine());
        // A body part's level, chance and share as the character is set
        // up (`InitColliderSettings`); a plate's the prefab's.
        let mut figures = c.figures;
        let mut damage_mod = c.damage_mod;
        if c.plate == 0 {
            if let Some([level, chance, share]) = source.rules.colliders.get(c.collider_type as usize).copied().flatten() {
                figures[0] = level;
                figures[1] = chance;
                damage_mod = share;
            }
        }
        let body = BodyRef { character: which as u32 + 1, part: c.part, collider: c.collider_type, plate: c.plate, damage_mod };
        colliders.push(Collider { data: c.clone(), bone, local, body, figures });
    }
    colliders
}

/// The dummies made once their skeletons are (`body_view::spawn_body`):
/// each its colliders, its character, and the AKM's model in its hands.
#[allow(clippy::too_many_arguments)]
pub fn make_dummies(
    mut commands: Commands,
    source: Option<Res<DummySource>>,
    rigs: Option<Res<DummyRigs>>,
    dummies: Option<Res<Dummies>>,
    parts: Res<crate::body_view::BodyParts>,
    mut game: ResMut<crate::GameData>,
    mut converted: ResMut<crate::render_data::Converted>,
    (mut meshes, mut images, mut materials, mut bindposes): (ResMut<Assets<Mesh>>, ResMut<Assets<Image>>, ResMut<Assets<StandardMaterial>>, ResMut<Assets<SkinnedMeshInverseBindposes>>),
) {
    let (Some(source), Some(rigs), None) = (source, rigs, dummies) else { return };
    let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |d| d.subsec_nanos() as u64);
    let mut list = Vec::new();
    for (which, rig) in rigs.0.iter().enumerate() {
        let colliders = colliders_of(&source, rig, which);
        let wearer = Wearer { rules: source.rules.clone(), pieces: Vec::new(), skills: VestSkills::default(), invincible: false };
        let health = Health::new(source.health.clone(), seed + which as u64);
        // The skeleton's bones the body's animator gives, and its arms.
        let drawn: Vec<Option<usize>> = parts.bones.iter().map(|path| rig.paths.iter().position(|p| p == path).or_else(|| rig.bone(path.rsplit('/').next().unwrap_or(path)))).collect();
        let arm_bones = ['R', 'L'].map(|side| ["Collarbone", "Upperarm", "Forearm1", "Forearm2", "Forearm3", "Palm"].map(|name| rig.bone(&format!("Base Human{side}{name}"))));
        // Its character, and the AKM's model in its hands, hung as the
        // player's rig is (`weapon::spawn_weapon`): from the feet turned
        // with the view, at the ribcage turned by the view's pitch.
        let puppet = source.maker.as_ref().and_then(|maker| match maker.make(&game, PLACES[which].at) {
            Ok(puppet) => Some(puppet),
            Err(e) => {
                warn!("dummy {which}'s character could not be made: {e:#}");
                None
            }
        });
        let (model, fingers) = match (&puppet, source.maker.as_ref()) {
            (Some(_), Some(maker)) => {
                let player = commands.spawn((Name::new(format!("Dummy {which}'s hands")), Transform::from_translation(PLACES[which].at), Visibility::default())).id();
                let pivot = commands.spawn((Name::new(format!("Dummy {which}'s view")), Transform::default(), Visibility::default())).id();
                commands.entity(player).add_child(pivot);
                let hierarchy = &maker.weapon.hierarchy;
                let tree = crate::weapon::spawn_tree(&mut commands, &mut game.unity, &mut converted, &mut meshes, &mut images, &mut materials, &mut bindposes, hierarchy, pivot, None, None);
                let root = &hierarchy.nodes[0];
                let mut root_local = convert::transform(root.translation, root.rotation, root.scale);
                root_local.translation = Vec3::ZERO;
                // The fingers: the digits under each palm of the rig (a
                // grip's own finger pose has bones of the same names).
                let fingers = hierarchy
                    .nodes
                    .iter()
                    .enumerate()
                    .filter(|(index, node)| node.name.starts_with("Base Human") && node.name.contains("Digit") && hierarchy.path(*index).starts_with("Base Human"))
                    .filter_map(|(index, node)| Some((index, rig.bone(&node.name)?)))
                    .collect();
                let weapon_root = tree.by_path.get("Weapon_root").copied();
                (Some(Model { player, pivot, tree: tree.entities, root_local, weapon_root }), fingers)
            }
            _ => (None, Vec::new()),
        };
        info!(
            "dummy {which}: {} hit colliders, {} of the body animator's {} bones on its skeleton, {} fingers, {}",
            colliders.len(),
            drawn.iter().flatten().count(),
            drawn.len(),
            fingers.len(),
            if puppet.is_some() { "an AKM in its hands" } else { "standing as the screens stand a character" }
        );
        list.push(Dummy {
            which,
            colliders,
            placed: Vec::new(),
            wearer,
            health,
            dressed: false,
            taken: Vec::new(),
            killing: None,
            fall: None,
            hits: 0,
            puppet,
            heading: 1.0,
            model,
            drawn,
            arm_bones,
            fingers,
            locals: rig.rest.clone(),
        });
    }
    commands.insert_resource(Dummies { list, age: 0 });
}

/// What the player wears, as a tree of things for the armour.
pub(crate) fn worn_of(item: &trp_sim::inventory::Item) -> Vec<Worn> {
    item.slots
        .iter()
        .zip(&item.kind.slots)
        .filter_map(|(content, slot)| {
            let content = content.as_ref()?;
            Some(Worn { template: content.kind.template.clone(), slot: slot.name.clone(), children: worn_of(content) })
        })
        .collect()
}

/// The debug panel's DRESS AS YOU, MADE WHOLE (`F9` until 0.141.0), and
/// once at the start: each dummy dressed in a fresh copy of what
/// the player wears (each of its pieces shown as the body's is, its armour
/// whole), its health whole, and, after the first time, its character made
/// anew where it stands, standing again if it lay dead. And every frame
/// their suits as the player's.
#[allow(clippy::too_many_arguments)]
pub fn dress_dummies(
    options: Res<crate::Options>,
    game: Res<crate::GameData>,
    carried: Res<crate::inventory_view::Carried>,
    source: Option<Res<DummySource>>,
    dummies: Option<ResMut<Dummies>>,
    rigs: Option<Res<DummyRigs>>,
    originals: Query<(&Visibility, Option<&crate::body_view::SuitPiece>), (Without<DummyTwin>, crate::park::EvenParked)>,
    mut twins: Query<(&DummyTwin, &mut Visibility), crate::park::EvenParked>,
    mut transforms: Query<&mut Transform>,
    mut panel: MessageReader<crate::debug_menu::DebugCommand>,
) {
    // The debug panel's button (`debug_menu`; its key `F9` went, the user,
    // 2026-10-26: "just leave them as commands in the new debug menu").
    let pressed = panel.read().any(|command| *command == crate::debug_menu::DebugCommand::DressDummies);
    let (Some(source), Some(mut dummies), Some(rigs)) = (source, dummies, rigs) else { return };
    dummies.age += 1;
    let asked = options.keys_live() && pressed;
    let first = dummies.list.iter().any(|d| !d.dressed) && dummies.age >= 3;
    let now = asked || first;
    for (twin, mut visibility) in &mut twins {
        let Ok((original, suit)) = originals.get(twin.original) else { continue };
        if (now || suit.is_some()) && *visibility != *original {
            *visibility = *original;
        }
    }
    if !now {
        return;
    }
    let worn = worn_of(&carried.equipment.item);
    for dummy in &mut dummies.list {
        let rig = &rigs.0[dummy.which];
        dummy.wearer = Wearer::dress(&game.db, source.rules.clone(), &worn, VestSkills::default());
        let seed = dummy.health.random_whole(0, i32::MAX) as u64;
        dummy.health = Health::new(source.health.clone(), seed);
        dummy.killing = None;
        // After the first time, its character made anew where it stands,
        // and standing again if it lay dead.
        if dummy.dressed {
            if let Some(maker) = source.maker.as_ref() {
                match maker.make(&game, PLACES[dummy.which].at) {
                    Ok(puppet) => dummy.puppet = Some(puppet),
                    Err(e) => warn!("dummy {}'s character could not be made again: {e:#}", dummy.which),
                }
            }
            dummy.heading = 1.0;
        }
        if dummy.fall.take().is_some() {
            dummy.locals = rig.rest.clone();
            for (bone, rest) in rig.bones.iter().zip(&rig.rest) {
                if let Ok(mut transform) = transforms.get_mut(*bone) {
                    *transform = *rest;
                }
            }
            if let Ok(mut root) = transforms.get_mut(rig.root) {
                *root = place(dummy.which);
            }
            let world = world_of(rig, &rig.rest, &[], None, Affine3A::IDENTITY);
            pose_outfits(rig, &world, &mut transforms);
        }
        dummy.dressed = true;
        info!(
            "dummy {} dressed as the player is: {}; plates on {:#x}",
            dummy.which,
            if dummy.wearer.pieces.is_empty() {
                "no armour".to_owned()
            } else {
                dummy.wearer.pieces.iter().map(|p| format!("{} (class {}, {:.0})", game.db.items.get(&p.template).map_or(p.template.as_str(), |t| t.name.as_str()), p.class, p.durability)).collect::<Vec<_>>().join(", ")
            },
            dummy.wearer.plates_on()
        );
    }
}

/// Each living dummy's character on: the walker's keys to its right or its
/// left by where it is, its frames run, and its body, the weapon in its
/// hands and its arms drawn as its last frame left them.
pub fn run_dummies(dummies: Option<ResMut<Dummies>>, rigs: Option<Res<DummyRigs>>, range: Res<crate::range::Range>, time: Res<Time>, mut transforms: Query<&mut Transform>) {
    let (Some(mut dummies), Some(rigs)) = (dummies, rigs) else { return };
    for dummy in &mut dummies.list {
        if !dummy.dressed || dummy.fall.is_some() || !dummy.health.alive {
            continue;
        }
        let Some(puppet) = dummy.puppet.as_mut() else { continue };
        let place = &PLACES[dummy.which];
        let mut keys = [0.0f32; 2];
        if place.walks {
            let x = puppet.movement.position[0];
            if x > place.at.x + WALK_SPAN {
                dummy.heading = -1.0;
            } else if x < place.at.x - WALK_SPAN {
                dummy.heading = 1.0;
            }
            // Its right along the world's x: the yaw's cosine.
            keys[0] = dummy.heading * puppet.movement.yaw.to_radians().cos().signum();
        }
        puppet.step(time.delta_secs(), keys, &*range);
        let Some(pose) = puppet.pose().cloned() else { continue };
        pose_dummy(dummy, &rigs.0[dummy.which], &pose, &mut transforms);
    }
}

/// A dummy drawn as its character's last frame left it: its body's root
/// and the bones its animator gives; the weapon's frame and every bone of
/// its rig; each arm, from the upper arm to the palm, where its solver put
/// it on the weapon, and the fingers as the rig has them.
fn pose_dummy(dummy: &mut Dummy, rig: &DummyRig, pose: &PuppetPose, transforms: &mut Query<&mut Transform>) {
    let game = |v: [f32; 3]| Vec3::new(v[0], v[1], -v[2]);
    let root = Transform { translation: game(pose.body_at), rotation: convert::rotation(pose.body_turn), scale: Vec3::ONE };
    if let Ok(mut transform) = transforms.get_mut(rig.root) {
        *transform = root;
    }
    for (k, bone) in dummy.drawn.iter().enumerate() {
        let (Some(bone), Some((at, turn))) = (bone, pose.bones.get(k)) else { continue };
        dummy.locals[*bone] = convert::transform(*at, *turn, [1.0; 3]);
    }
    // The weapon's frame and its rig.
    let mut rig_world = Affine3A::IDENTITY;
    if let Some(model) = &dummy.model {
        let player = Transform { translation: game(pose.position), rotation: Quat::from_rotation_y(-pose.yaw.to_radians()), scale: Vec3::ONE };
        let pivot = Transform { translation: game(pose.ribcage), rotation: Quat::from_rotation_x(-pose.pitch.to_radians()), scale: Vec3::ONE };
        for (entity, transform) in [(model.player, player), (model.pivot, pivot)] {
            if let Ok(mut t) = transforms.get_mut(entity) {
                *t = transform;
            }
        }
        for (entity, (at, turn)) in model.tree.iter().zip(&pose.rig).skip(1) {
            if let Ok(mut t) = transforms.get_mut(*entity) {
                t.translation = convert::position(*at);
                t.rotation = convert::rotation(*turn);
            }
        }
        rig_world = player.compute_affine() * pivot.compute_affine() * model.root_local.compute_affine();
    }
    // The arms onto the weapon: each bone where the solver put it, in its
    // parent on the skeleton.
    let mut world = world_of(rig, &dummy.locals, &[], None, Affine3A::IDENTITY);
    let root_world = root.compute_affine();
    if dummy.model.is_some() {
        for (side, bones) in dummy.arm_bones.iter().enumerate() {
            let [Some(collar), Some(upper), Some(f1), Some(f2), Some(f3), Some(palm)] = *bones else { continue };
            let placed: [Affine3A; 5] = pose.arms[side].map(|(at, turn)| root_world.inverse() * rig_world * Affine3A::from_rotation_translation(convert::rotation(turn), convert::position(at)));
            let chain = [collar, upper, f1, f2, f3, palm];
            for k in 0..5 {
                let parent = if k == 0 { world[chain[0]] } else { placed[k - 1] };
                dummy.locals[chain[k + 1]] = Transform::from_matrix(Mat4::from(parent.inverse() * placed[k]));
            }
        }
        for (from, bone) in &dummy.fingers {
            if let Some((at, turn)) = pose.rig.get(*from) {
                dummy.locals[*bone] = convert::transform(*at, *turn, [1.0; 3]);
            }
        }
        world = world_of(rig, &dummy.locals, &[], None, Affine3A::IDENTITY);
    }
    for (bone, local) in rig.bones.iter().zip(&dummy.locals) {
        if let Ok(mut t) = transforms.get_mut(*bone) {
            *t = *local;
        }
    }
    pose_outfits(rig, &world, transforms);
}

/// Each bone of a dummy in its root's axes: where its body is in the
/// engine, else its place under its parent.
fn world_of(rig: &DummyRig, locals: &[Transform], moved: &[Option<Body>], engine: Option<&World>, root: Affine3A) -> Vec<Affine3A> {
    let into_root = root.inverse();
    let mut world: Vec<Affine3A> = Vec::with_capacity(rig.bones.len());
    for (index, local) in locals.iter().enumerate() {
        let parent = rig.parents[index].map_or(Affine3A::IDENTITY, |p| world[p]);
        let here = match (moved.get(index).copied().flatten(), engine) {
            (Some(body), Some(engine)) => {
                let (at, turn) = engine.pose(body);
                into_root * Affine3A::from_rotation_translation(convert::rotation(turn), convert::position(at))
            }
            _ => parent * local.compute_affine(),
        };
        world.push(here);
    }
    world
}

/// Its outfits' skeletons on its bones as they stand (`death::follow`'s).
fn pose_outfits(rig: &DummyRig, world: &[Affine3A], transforms: &mut Query<&mut Transform>) {
    use trp_data::outfit::Move;
    for outfit in &rig.outfits {
        let theirs: Vec<Move> = outfit
            .theirs
            .iter()
            .zip(&outfit.rest)
            .map(|(bone, rest)| {
                let moved = bone.map_or(Affine3A::IDENTITY, |bone| world[bone]) * *rest;
                Move { axes: [moved.matrix3.x_axis.to_array(), moved.matrix3.y_axis.to_array(), moved.matrix3.z_axis.to_array()], shift: moved.translation.to_array() }
            })
            .collect();
        for (entity, moved) in outfit.ours.iter().zip(outfit.rig.pose(&theirs)) {
            let turn = Quat::from_mat3(&Mat3::from_cols(moved.axes[0].into(), moved.axes[1].into(), moved.axes[2].into())).normalize();
            if let Ok(mut transform) = transforms.get_mut(*entity) {
                *transform = Transform { translation: moved.shift.into(), rotation: turn, scale: Vec3::ONE };
            }
        }
    }
}

/// Their colliders where their bones are this frame, their health stepped
/// (they bleed), and last frame's hits forgotten.
pub fn place_dummies(dummies: Option<ResMut<Dummies>>, rigs: Option<Res<DummyRigs>>, helper: TransformHelper, time: Res<Time>) {
    let (Some(mut dummies), Some(rigs)) = (dummies, rigs) else { return };
    for dummy in &mut dummies.list {
        let rig = &rigs.0[dummy.which];
        dummy.taken.clear();
        if dummy.dressed {
            dummy.health.step(time.delta_secs());
            let _ = dummy.health.take_events();
        }
        let bones: HashMap<usize, Affine3A> = dummy.colliders.iter().filter_map(|c| Some((c.bone, helper.compute_global_transform(rig.bones[c.bone]).ok()?.affine()))).collect();
        dummy.placed = dummy.colliders.iter().map(|c| placed(c, bones.get(&c.bone).copied().unwrap_or(Affine3A::IDENTITY))).collect();
        if std::env::var_os("TRP_DUMMY_DEBUG").is_some() && dummy.which == 0 && every_second() {
            for (c, p) in dummy.colliders.iter().zip(&dummy.placed).take(40).filter(|(c, _)| c.data.object.contains("Spine3") || c.data.object.contains("Head")) {
                let at = match p {
                    Placed::Sphere { centre, .. } | Placed::Box { centre, .. } => *centre,
                    Placed::Capsule { a, b, .. } => (*a + *b) / 2.0,
                };
                info!("dummy {} collider {} at {:.3?}", dummy.which, c.data.object, at);
            }
        }
    }
}

/// For `TRP_DUMMY_DEBUG`'s lines: true on every sixtieth call.
fn every_second() -> bool {
    static FRAMES: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    FRAMES.fetch_add(1, std::sync::atomic::Ordering::Relaxed) % 60 == 0
}

/// Their deaths (health spec, "Death"; `Corpse.CreateCorpse`,
/// `DropItemDead`, `ApplyCorpseImpulse`): a body made the game's ragdoll
/// where it stands, on the range's engine, the weapon in its hands let go
/// on a spring from the pelvis, pushed by the killing hit the frame after;
/// then each frame stepped as the game steps it, its bones where its
/// bodies are and the weapon where its own is.
#[allow(clippy::too_many_arguments)]
pub fn fall_dummies(
    dummies: Option<ResMut<Dummies>>,
    rigs: Option<Res<DummyRigs>>,
    source: Option<Res<DummySource>>,
    ragdoll: Res<crate::death::Ragdoll>,
    range: Res<crate::range::Range>,
    clock: Res<crate::death::PhysicsClock>,
    (time, game, mut landed): (Res<Time>, Res<crate::GameData>, MessageWriter<crate::landing_sound::Landed>),
    (parents, children, meshes, node_colliders, seen): (
        Query<&ChildOf>,
        Query<&Children>,
        Query<&Aabb, (With<Mesh3d>, Without<SkinnedMesh>)>,
        Query<&crate::weapon::NodeColliders>,
        Query<&InheritedVisibility>,
    ),
    mut set: ParamSet<(TransformHelper, Query<&mut Transform>)>,
) {
    let (Some(mut dummies), Some(rigs), Some(source)) = (dummies, rigs, source) else { return };
    for dummy in &mut dummies.list {
        let rig = &rigs.0[dummy.which];
        if dummy.fall.is_none() && dummy.dressed && !dummy.health.alive {
            let helper = set.p0();
            let (Some(data), Some(physics)) = (ragdoll.data.as_ref(), ragdoll.physics.as_ref()) else { return };
            let mut engine = crate::loose::range_world(physics, &range);
            let deadbody = physics.layer("Deadbody").unwrap_or(23);
            let global = |entity: Entity| helper.compute_global_transform(entity).ok().map(|g| g.affine());
            let game_pose = |affine: Affine3A| {
                let (_, turn, at) = affine.to_scale_rotation_translation();
                ([at.x, at.y, -at.z], convert::rotation_back(turn))
            };
            let poses: HashMap<String, ([f32; 3], [f32; 4])> = data.bodies.iter().filter_map(|b| Some((b.bone.clone(), game_pose(global(rig.bones[rig.bone(&b.bone)?])?)))).collect();
            let bodies = trp_physics::corpse::build(&mut engine, data, &poses, [0.0; 3], ragdoll.max_depenetration, deadbody);
            let mut moved = vec![None; rig.bones.len()];
            for (bone, body) in &bodies {
                if let Some(index) = rig.bone(bone) {
                    moved[index] = Some(*body);
                }
            }
            // The weapon let go (`death::die`'s): its own colliders, else the
            // box its meshes fill, a body of 1 kg on the strap's spring from
            // the pelvis, from a point by the right palm.
            let mut weapon = None;
            if let Some(weapon_root) = dummy.model.as_ref().and_then(|m| m.weapon_root) {
                if let Some(at) = global(weapon_root) {
                    let into = at.inverse();
                    let shells = physics.layer("Shells");
                    let (mut low, mut high) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
                    let mut colliders: Vec<PhysicsCollider> = Vec::new();
                    let mut stack = vec![weapon_root];
                    while let Some(entity) = stack.pop() {
                        let shown = seen.get(entity).is_ok_and(|visible| visible.get());
                        if let (true, Some(g)) = (shown, global(entity)) {
                            let to_weapon = into * g;
                            if let Ok(aabb) = meshes.get(entity) {
                                for corner in 0..8 {
                                    let sign = Vec3::new(if corner & 1 == 0 { -1.0 } else { 1.0 }, if corner & 2 == 0 { -1.0 } else { 1.0 }, if corner & 4 == 0 { -1.0 } else { 1.0 });
                                    let point = to_weapon.transform_point3(Vec3::from(aabb.center) + Vec3::from(aabb.half_extents) * sign);
                                    low = low.min(point);
                                    high = high.max(point);
                                }
                            }
                            if let Ok(own) = node_colliders.get(entity) {
                                let (scale, turn, place) = to_weapon.to_scale_rotation_translation();
                                let (at, turn) = ([place.x, place.y, -place.z], convert::rotation_back(turn));
                                for c in own.0.iter().filter(|c| !c.trigger && Some(c.layer as u16) != shells) {
                                    colliders.extend(trp_physics::loose::collider(c, at, turn, scale.abs().to_array(), &mut |_| None));
                                }
                            }
                        }
                        if let Ok(below) = children.get(entity) {
                            stack.extend(below.iter());
                        }
                    }
                    if colliders.is_empty() && low.cmple(high).all() {
                        let half = ((high - low) / 2.0).max(Vec3::splat(0.01));
                        let centre = (low + high) / 2.0;
                        colliders.push(PhysicsCollider { shape: PhysicsShape::Box { half: half.to_array() }, at: [centre.x, centre.y, -centre.z], turn: [0.0, 0.0, 0.0, 1.0], material: Material::default() });
                    }
                    if !colliders.is_empty() {
                        let (weapon_at, weapon_turn) = game_pose(at);
                        let body = engine.add_body(&BodyDesc {
                            at: weapon_at,
                            turn: weapon_turn,
                            colliders,
                            mass: crate::death::WEAPON_MASS,
                            drag: 0.0,
                            angular_drag: crate::death::UNITY_ANGULAR_DRAG,
                            velocity: [0.0; 3],
                            spin: [0.0; 3],
                            max_depenetration: ragdoll.max_depenetration,
                            continuous: false,
                            layer: deadbody,
                        });
                        let palm = rig.bone("Base HumanRPalm").and_then(|bone| global(rig.bones[bone])).map(|g| game_pose(g).0);
                        let mass_of = |bone: &str| data.bodies.iter().find(|b| b.bone == bone).map_or(1.0, |b| b.mass);
                        if let (Some(pelvis), Some(palm)) = (bodies.get("Base HumanPelvis"), palm) {
                            let back = trp_physics::inverse(weapon_turn);
                            let palm = trp_physics::rotate(back, [palm[0] - weapon_at[0], palm[1] - weapon_at[1], palm[2] - weapon_at[2]]);
                            let by = crate::death::STRAP_BY_PALM;
                            let on_weapon = [palm[0] + by[0], palm[1] + by[1], palm[2] + by[2]];
                            engine.spring_joint(body, on_weapon, *pelvis, crate::death::STRAP_ON_PELVIS, crate::death::STRAP_SPRING, crate::death::STRAP_DAMPER, [0.0, 0.0], true, mass_of("Base HumanPelvis") / crate::death::WEAPON_MASS);
                        }
                        engine.report_touches(body);
                        weapon = Some(body);
                    }
                }
            }
            // Its landing: the AKM's sound, a body of the weapon's mass,
            // counted from the world's nought (`DropItemDead` keeps no place).
            let kind = game.db.items.get(WEAPON).and_then(|t| t.props.get("DropSoundType")?.as_str().map(trp_data::sound::drop_kind)).unwrap_or(0);
            let landing = trp_sim::landing::Landing::new(kind, crate::death::WEAPON_MASS, 0.0);
            let surfaces = bodies.iter().filter_map(|(bone, body)| Some((*body, data.bodies.iter().find(|b| &b.bone == bone)?.colliders.first()?.surface))).collect();
            // The push: on the body whose own collider (its bone's) was hit.
            let push = dummy.killing.and_then(|k| {
                let c = dummy.colliders.get(k.collider)?;
                if c.data.local.is_some() || c.data.plate != 0 {
                    return None;
                }
                let body = bodies.get(&c.data.object)?;
                let force = source.hit_force * (0.3 + 0.7 * inverse_lerp(50.0, 20.0, k.penetration));
                Some((*body, std::array::from_fn(|i| k.direction[i] * force), k.point))
            });
            info!(
                "dummy {} dead: {} bodies in PhysX, the weapon {}{}",
                dummy.which,
                bodies.len(),
                if weapon.is_some() { "let go" } else { "none" },
                if push.is_some() { ", pushed by the killing hit" } else { "" }
            );
            dummy.fall = Some(Fall { engine, moved, push, weapon, landing, since: 0.0, surfaces });
            continue;
        }
        let Some(fall) = dummy.fall.as_mut() else { continue };
        if let Some((body, impulse, point)) = fall.push.take() {
            fall.engine.impulse_at(body, impulse, point);
        }
        fall.engine.step(clock.now());
        fall.since += time.delta_secs();
        // The weapon's touches heard: a body of the corpse by its hit
        // colliders' surface (`BodyPartCollider.SurfaceSound`).
        if let Some(body) = fall.weapon {
            let surfaces = &fall.surfaces;
            landed.write_batch(crate::landing_sound::hear(&mut fall.engine, body, &mut fall.landing, fall.since, &range, |met| surfaces.get(&met).copied()));
        }
        static FALL_FRAMES: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        if std::env::var_os("TRP_DUMMY_DEBUG").is_some() && FALL_FRAMES.fetch_add(1, std::sync::atomic::Ordering::Relaxed) % 60 == 0 {
            let pelvis = rig.bone("Base HumanPelvis").and_then(|b| fall.moved[b]).map(|b| fall.engine.pose(b).0);
            let weapon = fall.weapon.map(|b| fall.engine.pose(b).0);
            info!("dummy {} falling: the pelvis at {pelvis:.2?}, the weapon at {weapon:.2?}", dummy.which);
        }
        // The weapon in its parent where its body is.
        let weapon_local = match (fall.weapon, dummy.model.as_ref().and_then(|m| m.weapon_root)) {
            (Some(body), Some(entity)) => {
                let (at, turn) = fall.engine.pose(body);
                let wanted = Affine3A::from_rotation_translation(convert::rotation(turn), convert::position(at));
                let helper = set.p0();
                parents.get(entity).ok().and_then(|p| helper.compute_global_transform(p.parent()).ok()).map(|parent| (entity, Transform::from_matrix(Mat4::from(parent.affine().inverse() * wanted))))
            }
            _ => None,
        };
        let mut transforms = set.p1();
        let root = transforms.get(rig.root).map_or(Affine3A::IDENTITY, |t| t.compute_affine());
        let world = world_of(rig, &dummy.locals, &fall.moved, Some(&fall.engine), root);
        for (index, moved) in fall.moved.iter().enumerate() {
            if moved.is_some() {
                let parent = rig.parents[index].map_or(Affine3A::IDENTITY, |p| world[p]);
                if let Ok(mut transform) = transforms.get_mut(rig.bones[index]) {
                    *transform = Transform::from_matrix(Mat4::from(parent.inverse() * world[index]));
                }
            }
        }
        if let Some((entity, local)) = weapon_local {
            if let Ok(mut transform) = transforms.get_mut(entity) {
                transform.translation = local.translation;
                transform.rotation = local.rotation;
            }
        }
        pose_outfits(rig, &world, &mut transforms);
    }
}

/// Unity's `Mathf.InverseLerp`.
fn inverse_lerp(a: f32, b: f32, value: f32) -> f32 {
    if a != b { ((value - a) / (b - a)).clamp(0.0, 1.0) } else { 0.0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_ray_enters_each_shape_where_its_face_is() {
        let d = Vec3::Z;
        let o = Vec3::new(0.0, 0.0, -5.0);
        let (t, n) = enter(Placed::Sphere { centre: Vec3::ZERO, radius: 1.0 }, o, d, 10.0).expect("the sphere");
        assert!((t - 4.0).abs() < 1e-5 && (n - Vec3::NEG_Z).length() < 1e-5);
        let (t, n) = enter(Placed::Box { centre: Vec3::ZERO, axes: [Vec3::X, Vec3::Y, Vec3::Z], half: Vec3::splat(0.5) }, o, d, 10.0).expect("the box");
        assert!((t - 4.5).abs() < 1e-5 && (n - Vec3::NEG_Z).length() < 1e-5);
        // A capsule along x, met on its side and on a cap.
        let capsule = Placed::Capsule { a: Vec3::new(-1.0, 0.0, 0.0), b: Vec3::new(1.0, 0.0, 0.0), radius: 0.25 };
        let (t, n) = enter(capsule, o, d, 10.0).expect("its side");
        assert!((t - 4.75).abs() < 1e-5 && (n - Vec3::NEG_Z).length() < 1e-5);
        let (t, n) = enter(capsule, Vec3::new(-5.0, 0.0, 0.0), Vec3::X, 10.0).expect("its cap");
        assert!((t - 3.75).abs() < 1e-5 && (n - Vec3::NEG_X).length() < 1e-5);
        // Nothing from inside, nothing past the ray's end.
        assert!(enter(capsule, Vec3::ZERO, d, 10.0).is_none());
        assert!(enter(Placed::Sphere { centre: Vec3::ZERO, radius: 1.0 }, o, d, 3.0).is_none());
    }
}
