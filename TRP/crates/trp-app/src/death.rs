//! The character's death as the game shows it (health spec, "Death";
//! `docs/spec/physics.md`).
//!
//! The body becomes a ragdoll, as the game's corpse makes one
//! (`Corpse.CreateCorpse`, `CorpseRagdoll`): the player prefab's rigid
//! bodies and joints (`trp_data::ragdoll`) on a copy of the body as it is
//! drawn at that moment, in **the game's own physics engine, PhysX 4.1**,
//! set up from the install's physics settings and stepped as the game
//! steps it (`trp_physics`: the corpse there is the one `bin/corpse` sets
//! beside four recorded deaths). The weapon in hand is let go a moment
//! later, a body of its own on the corpse's sling (`Player.DropItemDead`,
//! `CorpseRagdoll.AttachWeapon`). The view rides the chest, where the
//! camera's place hangs (`Cam`, under the spine). The picture fades (the
//! camera's `DeathFade`) as the raid's end blacks it out over a second,
//! and then the death screen (`health_view`). `Enter` there begins the run again where it began, in
//! place: what the run started with is put back (`StartOfRun`).
//!
//! The engine works in the game's axes (z ahead): what goes in is turned
//! from Bevy's and what comes out turned back (`convert`).

use std::collections::HashMap;

use bevy::math::Affine3A;
use bevy::prelude::*;
use bevy::mesh::skinning::SkinnedMesh;
use bevy::camera::primitives::Aabb;
use bevy::transform::helper::TransformHelper;
use trp_data::ragdoll::RagdollData;
use trp_physics::corpse::Stepper;
use trp_physics::{Body, BodyDesc, Collider, Material, Shape, World};

use crate::body_view::{BodyParts, BodyRig, OutfitArms, OutfitOn};
use crate::controller::Controller;
use crate::inventory_view::Carried;
use crate::range::Range;
use crate::weapon::{Eye, FirstPersonHands, WeaponRig};
use crate::{convert, MainCamera, View};

/// The camera's death effects (`DeathFade` on the camera prefab, health
/// spec, "Death"): the colour fades over 2 s, and from 1.8 s the eyes
/// close over 0.25 s, both by the prefab's curve (from 0 with no slope to
/// 1 with a slope of 2: the square of the time's share); the colour's
/// strength is that over 1.5, the eyes' over 2. Measured: the recorded
/// fade's colour stands at 0.251 one second after the death.
const FADE_TIME: f32 = 2.0;
const EYES_DELAY: f32 = 1.8;
const EYES_TIME: f32 = 0.25;
const FADE_SHARE: f32 = 1.0 / 1.5;
const EYES_SHARE: f32 = 0.5;
/// The raid's end (`BaseLocalGame.InitiateGameStopping`, `Stop`; health
/// spec, "How soon the raid ends"): a black image over everything, its
/// opacity raised by each frame's length from the frame of the death
/// (`PreloaderUI.StartBlackScreenShow(1, 1, …)`, `Co_ShowBlackImage`), whole
/// after a second, until it is 0.05 past whole; then the raid is let go.
/// Recorded: `courses2`'s deaths end 1.04 s after the death.
const BLACK_STEP: f32 = 1.0;
const BLACK_DONE: f32 = 1.05;
/// The death screen (the app's, which the game has not) comes when the
/// game's black is done and its raid would end.
pub const SCREEN_AT: f32 = BLACK_DONE / BLACK_STEP;
/// The weapon let go (`CorpseRagdoll.AttachWeapon`, health spec, "Death"):
/// a body of 1 kg, or 2 kg for a pistol, which is held fast to the right
/// forearm; anything else hangs from the pelvis on a spring (33, damped by
/// 3, its tolerance Unity's 0.025) between a point by the right palm in the
/// weapon's axes and a point of the pelvis, in the game's axes. Each joint
/// with its mass scale, the connected body's mass over the weapon's.
pub(crate) const WEAPON_MASS: f32 = 1.0;
const PISTOL_MASS: f32 = 2.0;
pub(crate) const STRAP_SPRING: f32 = 33.0;
pub(crate) const STRAP_DAMPER: f32 = 3.0;
pub(crate) const STRAP_BY_PALM: [f32; 3] = [0.0, 0.1, -0.1];
pub(crate) const STRAP_ON_PELVIS: [f32; 3] = [-0.215, 0.164, -0.159];
/// A new Rigidbody's angular drag, Unity's default (the weapon's).
pub(crate) const UNITY_ANGULAR_DRAG: f32 = 0.05;

pub struct DeathPlugin;

impl Plugin for DeathPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Dying>().add_message::<BeginAgain>();
    }
}

/// A place and turn of Bevy's in the game's axes, and back.
fn to_game(at: Vec3, turn: Quat) -> ([f32; 3], [f32; 4]) {
    ([at.x, at.y, -at.z], convert::rotation_back(turn))
}

fn from_game(at: [f32; 3], turn: [f32; 4]) -> Affine3A {
    Affine3A::from_rotation_translation(convert::rotation(turn), convert::position(at))
}

/// A piece of the corpse, drawn as a piece of the living body is: shown as
/// that one is when the character dies.
#[derive(Component)]
pub struct CorpseTwin(pub Entity);

/// A piece of the corpse's head, or of what is worn on it: for the
/// character's own eyes a shadow and no more, as the game draws no head
/// for them (`shadow_only`).
#[derive(Component)]
#[require(crate::shadow_only::ShadowOnly)]
pub struct CorpseHead;

/// What a death puts in the world, taken away when the run begins again.
#[derive(Component)]
pub struct OfDeath;

/// An outfit's skeleton on the corpse (`figure_view::Builder::outfit`):
/// for each of its rig's bones the corpse's bone it follows, that bone's
/// own matrix for a point of a mesh, and the entity its meshes hang on.
pub struct CorpseOutfit {
    pub item: String,
    pub rig: trp_data::outfit::Rig,
    pub theirs: Vec<Option<usize>>,
    pub rest: Vec<Affine3A>,
    pub ours: Vec<Entity>,
}

/// The corpse as built with the body (`body_view::spawn_body`), hidden
/// until the character dies: its skeleton by path (a parent before what
/// hangs from it), and its outfits.
#[derive(Resource)]
pub struct Corpse {
    pub root: Entity,
    pub paths: Vec<String>,
    pub bones: Vec<Entity>,
    pub parents: Vec<Option<usize>>,
    pub outfits: Vec<CorpseOutfit>,
}

/// The install's ragdoll and the figure a corpse's bodies go by, read at
/// the start (`main`): how fast one is pushed out of what it is in
/// (`EFTHardSettings.CorpseMaxDepenetrationVelocity`).
#[derive(Resource)]
pub struct Ragdoll {
    pub data: Option<RagdollData>,
    pub max_depenetration: f32,
    /// The engine's settings (`globalgamemanagers`).
    pub physics: Option<trp_data::engine::PhysicsSettings>,
}

/// What the run began with, put back when it begins again: the
/// controller as made (the character, its health, its hands), the
/// inventory's things and the range.
#[derive(Resource)]
pub struct StartOfRun {
    pub controller: Controller,
    pub equipment: trp_sim::inventory::Equipment,
    pub loot: trp_sim::inventory::Item,
    pub away: Vec<Option<trp_sim::inventory::Item>>,
    pub ground: Vec<(trp_sim::inventory::Item, Vec3)>,
    pub range: Range,
}

/// The run is begun again: what keeps state of its own about the hands
/// starts over (`inventory_view`).
#[derive(Message)]
pub struct BeginAgain;

/// A death under way.
#[derive(Resource, Default)]
pub struct Dying(Option<Death>);

impl Dying {
    /// Seconds since the character died, if it has.
    pub fn since(&self) -> Option<f32> {
        self.0.as_ref().map(|death| death.since)
    }
}

pub struct Death {
    since: f32,
    /// Each corpse bone's place in its parent as it stood at death, and the
    /// body that moves it now (the body's pose is the bone's).
    locals: Vec<Transform>,
    moved: Vec<Option<Body>>,
    /// The outfit the corpse wears, by its place among the corpse's.
    outfit: Option<usize>,
    /// The chest's body, and where the character's eyes were in it.
    eyes: Option<(Body, Transform)>,
    /// The weapon let go: its body and the bone it hangs from on the rig;
    /// and that bone's place in its parent as it was, to be put back.
    weapon: Option<(Body, Entity)>,
    weapon_rest: Option<(Entity, Transform)>,
    /// The game's engine with the corpse in it.
    engine: World,
    /// The weapon heard as it lands (`landing_sound`), and each body's
    /// surface (its bone's hit collider's `SurfaceSound`).
    landing: trp_sim::landing::Landing,
    surfaces: std::collections::HashMap<Body, u8>,
}


/// An affine as a transform.
fn transform_of(affine: Affine3A) -> Transform {
    Transform::from_matrix(Mat4::from(affine))
}

/// Whether an entity hangs, at any depth, from another.
fn under(entity: Entity, root: Entity, parents: &Query<&ChildOf>) -> bool {
    let mut at = entity;
    while let Ok(parent) = parents.get(at) {
        at = parent.parent();
        if at == root {
            return true;
        }
    }
    false
}

/// The game's own step of the physics, fed every drawn frame
/// (`trp_physics::corpse::Stepper`), and what it steps by this frame.
#[derive(Resource)]
pub struct PhysicsClock {
    stepper: Stepper,
    now: f32,
}

impl PhysicsClock {
    pub fn new(most: f32) -> Self {
        Self { stepper: Stepper::new(most), now: 0.0 }
    }

    /// What the physics steps by this frame.
    pub fn now(&self) -> f32 {
        self.now
    }
}

/// When the character has just died: the corpse takes the body's pose as
/// it is drawn this frame, and the game's engine is made with the ragdoll
/// on it, the weapon let go, and the range to fall against.
#[allow(clippy::too_many_arguments)]
pub fn die(
    mut commands: Commands,
    controller: Res<Controller>,
    mut dying: ResMut<Dying>,
    rigs: (Option<Res<Corpse>>, Option<Res<BodyParts>>, Option<Res<BodyRig>>, Option<Res<WeaponRig>>),
    (ragdoll, outfit_on, range): (Res<Ragdoll>, Res<OutfitOn>, Res<Range>),
    helper: TransformHelper,
    (parents, children, meshes, eye, locals_of): (Query<&ChildOf>, Query<&Children>, Query<&Aabb, (With<Mesh3d>, Without<SkinnedMesh>)>, Query<Entity, With<Eye>>, Query<&Transform>),
    originals: Query<&Visibility, (Without<CorpseTwin>, crate::park::EvenParked)>,
    mut twins: Query<(&CorpseTwin, &mut Visibility), crate::park::EvenParked>,
    (mut clock, time): (ResMut<PhysicsClock>, Res<Time>),
    (node_colliders, seen): (Query<&crate::weapon::NodeColliders>, Query<&InheritedVisibility>),
    (hands_held, carried, game): (Res<crate::inventory_view::HandsHeld>, Res<crate::inventory_view::Carried>, Res<crate::GameData>),
) {
    // Every frame counts towards the step, as the game's average does.
    let clock = &mut *clock;
    clock.now = clock.stepper.frame(time.delta_secs());
    let dead = controller.health.as_ref().is_some_and(|health| !health.alive);
    if !dead || dying.0.is_some() {
        return;
    }
    let (Some(corpse), Some(parts), Some(body), Some(weapon)) = rigs else { return };
    let (Some(data), Some(physics)) = (ragdoll.data.as_ref(), ragdoll.physics.as_ref()) else { return };
    // Drawn, as the body is this frame: what hangs from the weapon rig's
    // root with the first person's squeeze along the view taken off.
    let Ok(root) = helper.compute_global_transform(weapon.root) else { return };
    let (_, turn, at) = root.to_scale_rotation_translation();
    let unsqueezed = Affine3A::from_rotation_translation(turn, at) * root.affine().inverse();
    let on_rig = |entity: Entity| -> Option<Affine3A> {
        let now = helper.compute_global_transform(entity).ok()?.affine();
        Some(if under(entity, weapon.root, &parents) { unsqueezed * now } else { now })
    };
    // And for the character's own eyes, what hangs from the ribcage
    // (the arms, the neck and head, the weapon in hand) hangs from the
    // weapon rig's root, which stands for it where the view needs it:
    // the corpse is the body as others see it, so all that is moved to
    // where the body's own ribcage is, as it hangs there.
    let (animated, drawn_ribcage) = body.ribcages();
    let rebase = match (animated.and_then(|ribcage| helper.compute_global_transform(ribcage).ok()), on_rig(drawn_ribcage)) {
        (Some(ribcage), Some(drawn)) => ribcage.affine() * drawn.inverse(),
        _ => Affine3A::IDENTITY,
    };
    let drawn = |entity: Entity| -> Option<Affine3A> { Some(if under(entity, weapon.root, &parents) { rebase * on_rig(entity)? } else { on_rig(entity)? }) };
    // Each bone of the corpse where the body has it; one the body does not
    // draw stands where its parent is.
    let mut world: Vec<Affine3A> = Vec::with_capacity(corpse.bones.len());
    let mut locals = Vec::with_capacity(corpse.bones.len());
    for (index, path) in corpse.paths.iter().enumerate() {
        let parent = corpse.parents[index].map_or(Affine3A::IDENTITY, |parent| world[parent]);
        let here = body.drawn(&parts, &weapon, path).and_then(drawn).unwrap_or(parent);
        world.push(here);
        let local = transform_of(parent.inverse() * here);
        locals.push(local);
        commands.entity(corpse.bones[index]).insert(local);
    }
    let bone_of = |name: &str| corpse.paths.iter().position(|path| path.rsplit('/').next() == Some(name));
    let game_pose = |bone: usize| {
        let (_, turn, at) = world[bone].to_scale_rotation_translation();
        to_game(Vec3::from(at), turn)
    };

    // The engine, as the install sets it up, and the range: its boxes on the
    // layer the world's colliders are on, and the ground.
    let mut engine = crate::loose::range_world(physics, &range);
    let deadbody = physics.layer("Deadbody").unwrap_or(23);
    // The corpse.
    let poses: HashMap<String, ([f32; 3], [f32; 4])> = data.bodies.iter().filter_map(|b| Some((b.bone.clone(), game_pose(bone_of(&b.bone)?)))).collect();
    let bodies = trp_physics::corpse::build(&mut engine, data, &poses, controller.movement.velocity, ragdoll.max_depenetration, deadbody);
    let mut moved = vec![None; corpse.bones.len()];
    for (bone, body) in &bodies {
        if let Some(index) = bone_of(bone) {
            moved[index] = Some(*body);
        }
    }
    let mass_of = |bone: &str| data.bodies.iter().find(|b| b.bone == bone).map_or(1.0, |b| b.mass);

    // The weapon in hand, a body of its own: the box its meshes fill on the
    // rig, hung from the pelvis by a spring, or a pistol held fast to the
    // right forearm.
    let (mut held, mut weapon_rest) = (None, None);
    if let Some(weapon_root) = weapon.weapon_root.filter(|_| controller.armed) {
        if let Some(at) = drawn(weapon_root) {
            let into = at.inverse();
            // The weapon's own colliders, as `DropItemDead` switches them on:
            // every one of its parts' (switched off in the prefab, switched on
            // then) but triggers and those on the shells'
            // layer, in the weapon's axes as the game has them. A mesh
            // collider is not taken (`PROVISIONAL`: none on the weapons
            // looked at); a weapon with none falls as the box its meshes
            // fill (`PROVISIONAL`).
            let shells = physics.layer("Shells");
            let (mut low, mut high) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
            let mut colliders = Vec::new();
            let mut stack = vec![weapon_root];
            while let Some(entity) = stack.pop() {
                let shown = seen.get(entity).is_ok_and(|visible| visible.get());
                if let (true, Some(global)) = (shown, drawn(entity)) {
                    let to_weapon = into * global;
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
                        let (at, turn) = to_game(Vec3::from(place), turn);
                        for c in own.0.iter().filter(|c| !c.trigger && Some(c.layer as u16) != shells) {
                            colliders.extend(trp_physics::loose::collider(c, at, turn, scale.abs().to_array(), &mut |_| None));
                        }
                    }
                }
                if let Ok(below) = children.get(entity) {
                    stack.extend(below.iter());
                }
            }
            if std::env::var("TRP_DEATH_DEBUG").is_ok() {
                let mut all = vec![weapon_root];
                let (mut with, mut shown_with) = (0, 0);
                while let Some(entity) = all.pop() {
                    if let Ok(own) = node_colliders.get(entity) {
                        with += own.0.len();
                        if seen.get(entity).is_ok_and(|visible| visible.get()) {
                            shown_with += own.0.len();
                        }
                    }
                    if let Ok(below) = children.get(entity) {
                        all.extend(below.iter());
                    }
                }
                info!("under the weapon's bone: {with} colliders, {shown_with} on shown objects; {} taken", colliders.len());
            }
            if colliders.is_empty() && low.cmple(high).all() {
                let half = ((high - low) / 2.0).max(Vec3::splat(0.01));
                let centre = (low + high) / 2.0;
                colliders.push(Collider { shape: Shape::Box { half: half.to_array() }, at: [centre.x, centre.y, -centre.z], turn: [0.0, 0.0, 0.0, 1.0], material: Material::default() });
            }
            if !colliders.is_empty() {
                let pistol = weapon.kind == trp_sim::body::hands_kind::PISTOL;
                let mass = if pistol { PISTOL_MASS } else { WEAPON_MASS };
                let (_, turn, place) = at.to_scale_rotation_translation();
                let (weapon_at, weapon_turn) = to_game(Vec3::from(place), turn);
                let count = colliders.len();
                let body = engine.add_body(&BodyDesc {
                    at: weapon_at,
                    turn: weapon_turn,
                    colliders,
                    mass,
                    drag: 0.0,
                    angular_drag: UNITY_ANGULAR_DRAG,
                    // The character's velocity, not held as the corpse's is.
                    velocity: controller.movement.velocity,
                    spin: [0.0; 3],
                    max_depenetration: ragdoll.max_depenetration,
                    continuous: false,
                    layer: deadbody,
                });
                let palm = bone_of("Base HumanRPalm").map(|bone| game_pose(bone).0);
                match (pistol, bodies.get("Base HumanRForearm1"), bodies.get("Base HumanPelvis"), palm) {
                    (true, Some(forearm), ..) => engine.fixed_joint(body, *forearm, mass_of("Base HumanRForearm1") / mass),
                    (false, _, Some(pelvis), Some(palm)) => {
                        // The palm in the weapon's axes, and the strap's point
                        // by it.
                        let back = trp_physics::inverse(weapon_turn);
                        let palm = trp_physics::rotate(back, [palm[0] - weapon_at[0], palm[1] - weapon_at[1], palm[2] - weapon_at[2]]);
                        let on_weapon = [palm[0] + STRAP_BY_PALM[0], palm[1] + STRAP_BY_PALM[1], palm[2] + STRAP_BY_PALM[2]];
                        engine.spring_joint(body, on_weapon, *pelvis, STRAP_ON_PELVIS, STRAP_SPRING, STRAP_DAMPER, [0.0, 0.0], true, mass_of("Base HumanPelvis") / mass);
                    }
                    _ => {}
                }
                info!("the weapon let go on {count} colliders");
                engine.report_touches(body);
                held = Some((body, weapon_root));
                weapon_rest = locals_of.get(weapon_root).ok().map(|local| (weapon_root, *local));
            }
        }
    }

    // The eyes ride the chest, where the camera's place hangs: where they
    // are in the chest's bone now.
    let eyes = eye.single().ok().and_then(|eye| helper.compute_global_transform(eye).ok()).zip(bone_of("Base HumanSpine3").and_then(|bone| Some((moved[bone]?, bone)))).map(|(seen, (chest, bone))| {
        (chest, transform_of(world[bone].inverse() * seen.affine()))
    });

    // The corpse shows what the body has on, and the body itself goes.
    for (twin, mut visibility) in &mut twins {
        if let Ok(original) = originals.get(twin.0) {
            *visibility = *original;
        }
    }
    commands.entity(corpse.root).insert(Visibility::Inherited);
    commands.entity(body.root()).insert(Visibility::Hidden);
    let outfit = outfit_on.item.as_ref().filter(|_| outfit_on.body).and_then(|item| corpse.outfits.iter().position(|outfit| &outfit.item == item));
    info!("dead: {} bodies and {} joints in PhysX, the weapon {}; a step of {:.4} s a frame", bodies.len(), data.joints.len(), if held.is_some() { "let go" } else { "none" }, clock.now);
    // Its landing: the weapon in hand's sound, a body of the weapon's mass,
    // counted from the world's nought (`DropItemDead` keeps no place).
    let kind = hands_held
        .item
        .and_then(|id| carried.find(id))
        .and_then(|item| game.db.items.get(&item.kind.template))
        .and_then(|t| t.props.get("DropSoundType")?.as_str().map(trp_data::sound::drop_kind))
        .unwrap_or(0);
    let mass = if weapon.kind == trp_sim::body::hands_kind::PISTOL { PISTOL_MASS } else { WEAPON_MASS };
    let landing = trp_sim::landing::Landing::new(kind, mass, 0.0);
    let surfaces = bodies.iter().filter_map(|(bone, body)| Some((*body, data.bodies.iter().find(|b| &b.bone == bone)?.colliders.first()?.surface))).collect();
    dying.0 = Some(Death { since: 0.0, locals, moved, outfit, eyes, weapon: held, weapon_rest, engine, landing, surfaces });
}

/// While dying: the engine stepped as the game steps it, each moved bone of
/// the corpse where its body is, its outfit on it, the weapon where its body
/// is, and the eyes riding the chest; the living body's hands out of sight.
#[allow(clippy::too_many_arguments)]
pub fn follow(
    mut dying: ResMut<Dying>,
    corpse: Option<Res<Corpse>>,
    weapon: Option<Res<WeaponRig>>,
    view: Res<View>,
    (time, clock): (Res<Time>, Res<PhysicsClock>),
    parents: Query<&ChildOf>,
    mut transforms: ParamSet<(TransformHelper, Query<&mut Transform>)>,
    camera: Query<Entity, With<MainCamera>>,
    mut hands: Query<&mut Visibility, Or<(With<FirstPersonHands>, With<OutfitArms>)>>,
    mut head: Query<&mut crate::shadow_only::ShadowOnly, (With<CorpseHead>, crate::park::EvenParked)>,
    (range, mut landed): (Res<Range>, MessageWriter<crate::landing_sound::Landed>),
) {
    let (Some(death), Some(corpse)) = (dying.0.as_mut(), corpse) else { return };
    death.since += time.delta_secs();
    death.engine.step(clock.now);
    // The weapon's touches heard (a body of the corpse by its surface).
    if let Some((body, _)) = death.weapon {
        let surfaces = &death.surfaces;
        landed.write_batch(crate::landing_sound::hear(&mut death.engine, body, &mut death.landing, death.since, &range, |met| surfaces.get(&met).copied()));
    }
    let wish = crate::shadow_only::ShadowOnly(*view == View::FirstPerson);
    for mut shadow_only in &mut head {
        if *shadow_only != wish {
            *shadow_only = wish;
        }
    }
    for mut visibility in &mut hands {
        if *visibility != Visibility::Hidden {
            *visibility = Visibility::Hidden;
        }
    }
    // The bones, a parent before what hangs from it.
    let mut world: Vec<Affine3A> = Vec::with_capacity(corpse.bones.len());
    for (index, local) in death.locals.iter().enumerate() {
        let parent = corpse.parents[index].map_or(Affine3A::IDENTITY, |parent| world[parent]);
        let here = match death.moved[index] {
            Some(body) => {
                let (at, turn) = death.engine.pose(body);
                from_game(at, turn)
            }
            None => parent * local.compute_affine(),
        };
        world.push(here);
    }
    {
        let mut set = transforms.p1();
        for (index, moved) in death.moved.iter().enumerate() {
            if moved.is_some() {
                let parent = corpse.parents[index].map_or(Affine3A::IDENTITY, |parent| world[parent]);
                if let Ok(mut transform) = set.get_mut(corpse.bones[index]) {
                    *transform = transform_of(parent.inverse() * world[index]);
                }
            }
        }
        // The outfit, on the corpse's bones as they are.
        if let Some(outfit) = death.outfit.and_then(|at| corpse.outfits.get(at)) {
            use trp_data::outfit::Move;
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
                if let Ok(mut transform) = set.get_mut(*entity) {
                    *transform = Transform { translation: moved.shift.into(), rotation: turn, scale: Vec3::ONE };
                }
            }
        }
        // The rig at its true size: no squeeze with nobody's eyes on it.
        if let Some(weapon) = weapon.as_ref() {
            if let Ok(mut root) = set.get_mut(weapon.root) {
                root.scale = Vec3::ONE;
            }
        }
    }
    // The weapon and the eyes: each in its parent's axes as those stand.
    let mut placed: Vec<(Entity, Transform)> = Vec::new();
    {
        let helper = transforms.p0();
        let in_parent = |entity: Entity, wanted: Affine3A| -> Option<Transform> {
            let parent = parents.get(entity).ok()?.parent();
            let parent = helper.compute_global_transform(parent).ok()?;
            Some(transform_of(parent.affine().inverse() * wanted))
        };
        if let Some((body, weapon_root)) = death.weapon {
            let (at, turn) = death.engine.pose(body);
            if let Some(local) = in_parent(weapon_root, from_game(at, turn)) {
                placed.push((weapon_root, local));
            }
        }
        if let (View::FirstPerson, Some((chest, eyes)), Ok(camera)) = (*view, death.eyes, camera.single()) {
            let (at, turn) = death.engine.pose(chest);
            if let Some(local) = in_parent(camera, from_game(at, turn) * eyes.compute_affine()) {
                placed.push((camera, local));
            }
        }
    }
    let mut set = transforms.p1();
    for (entity, local) in placed {
        if let Ok(mut transform) = set.get_mut(entity) {
            transform.translation = local.translation;
            transform.rotation = local.rotation;
        }
    }
}

/// The picture fades and the eyes close (`DeathFade`): the colour's fade,
/// its look `PROVISIONAL` (the game's shader is not read: here the colour
/// drained by its strength), and the eyes as two lids of the shader's
/// colour, black, meeting in the middle at full strength (`PROVISIONAL`
/// likewise). Over all of it the raid's end blacks the screen out (`Black`),
/// whole after a second: the eyes' closing, from 1.8 s, is under it.
#[allow(clippy::type_complexity)]
pub fn fade(
    mut commands: Commands,
    dying: Res<Dying>,
    mut grading: Query<(Entity, Option<&mut bevy::render::view::ColorGrading>), With<MainCamera>>,
    mut lids: Query<(&Lid, &mut Node, &mut Visibility), Without<Black>>,
    mut black: Query<(&mut BackgroundColor, &mut Visibility), (With<Black>, Without<Lid>)>,
    mut made: Local<bool>,
) {
    if !*made {
        *made = true;
        for top in [true, false] {
            let mut node = Node { position_type: PositionType::Absolute, left: Val::Px(0.0), width: Val::Percent(100.0), height: Val::Percent(0.0), ..default() };
            if top {
                node.top = Val::Px(0.0);
            } else {
                node.bottom = Val::Px(0.0);
            }
            commands.spawn((Lid, node, BackgroundColor(Color::BLACK), GlobalZIndex(38), Visibility::Hidden));
        }
        let whole = Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() };
        commands.spawn((Black, whole, BackgroundColor(Color::NONE), GlobalZIndex(39), Visibility::Hidden));
    }
    let opacity = dying.since().map_or(0.0, |since| (since * BLACK_STEP).min(1.0));
    for (mut ground, mut visibility) in &mut black {
        let wanted = Color::srgba(0.0, 0.0, 0.0, opacity);
        if ground.0 != wanted {
            ground.0 = wanted;
        }
        let shown = if opacity > 0.0 { Visibility::Inherited } else { Visibility::Hidden };
        if *visibility != shown {
            *visibility = shown;
        }
    }
    let since = dying.since();
    let curve = |share: f32| share.clamp(0.0, 1.0).powi(2);
    let colour = since.map_or(0.0, |since| curve(since / FADE_TIME) * FADE_SHARE);
    let eyes = since.map_or(0.0, |since| curve((since - EYES_DELAY) / EYES_TIME) * EYES_SHARE);
    if let Ok((camera, grading)) = grading.single_mut() {
        let saturation = 1.0 - colour;
        match grading {
            Some(mut grading) => {
                if grading.global.post_saturation != saturation {
                    grading.global.post_saturation = saturation;
                }
            }
            None if colour > 0.0 => {
                let mut grading = bevy::render::view::ColorGrading::default();
                grading.global.post_saturation = saturation;
                commands.entity(camera).insert(grading);
            }
            None => {}
        }
    }
    for (_, mut node, mut visibility) in &mut lids {
        let wanted = Val::Percent(eyes * 100.0);
        if node.height != wanted {
            node.height = wanted;
        }
        let shown = if eyes > 0.0 { Visibility::Inherited } else { Visibility::Hidden };
        if *visibility != shown {
            *visibility = shown;
        }
    }
}

/// A lid of the eyes closing.
#[derive(Component)]
pub struct Lid;

/// The black the raid's end lays over everything (`PreloaderUI`'s
/// `_overlapBlackImage`).
#[derive(Component)]
pub struct Black;

/// Dead, with the death screen up, `Enter` begins the run again where it
/// began (the user, 2026-10-14, night, then 2026-10-14, late: "When dying
/// and pressing enter, the run should reset in place"): the controller as
/// the run made it, the inventory's things and the range as they were; the
/// corpse, its physics and the fade gone, and the living body back. Either
/// Enter, the main one or the keypad's, as the inventory's windows take
/// either (the keypad's alone did nothing until 0.101.1: "I cannot respawn
/// once dead").
#[allow(clippy::too_many_arguments)]
pub fn begin_again(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut dying: ResMut<Dying>,
    start: Option<Res<StartOfRun>>,
    mut controller: ResMut<Controller>,
    mut carried: ResMut<Carried>,
    mut range: ResMut<Range>,
    corpse: Option<Res<Corpse>>,
    body: Option<Res<BodyRig>>,
    of_death: Query<Entity, (With<OfDeath>, Without<ChildOf>)>,
    mut again: MessageWriter<BeginAgain>,
) {
    let ready = dying.since().is_some_and(|since| since >= SCREEN_AT);
    if !ready || !keys.any_just_pressed([KeyCode::Enter, KeyCode::NumpadEnter]) {
        return;
    }
    let Some(start) = start else { return };
    *controller = start.controller.clone();
    let carried = &mut *carried;
    carried.equipment = start.equipment.clone();
    carried.checked = crate::inventory_view::magazines_in(&carried.equipment.item);
    carried.loot = start.loot.clone();
    carried.away = start.away.clone();
    carried.ground = start.ground.clone();
    carried.falling.clear();
    carried.loot_open = false;
    carried.loot_of = 0;
    carried.windows.clear();
    carried.asking = None;
    carried.priority = None;
    carried.note.clear();
    *range = start.range.clone();
    for entity in &of_death {
        commands.entity(entity).despawn();
    }
    if let Some(corpse) = corpse {
        commands.entity(corpse.root).insert(Visibility::Hidden);
    }
    if let Some(body) = body {
        commands.entity(body.root()).insert(Visibility::Inherited);
    }
    // The weapon's bone back where it hung on the rig.
    if let Some((weapon_root, rest)) = dying.0.as_ref().and_then(|death| death.weapon_rest) {
        commands.entity(weapon_root).insert(rest);
    }
    dying.0 = None;
    again.write(BeginAgain);
    info!("the run begun again where it began");
}
