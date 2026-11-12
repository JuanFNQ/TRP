//! The corpse, its outfit and ragdoll, the death state and the physics clock.

use super::*;

/// A place and turn of Bevy's in the game's axes, and back.
pub(super) fn to_game(at: Vec3, turn: Quat) -> ([f32; 3], [f32; 4]) {
    ([at.x, at.y, -at.z], convert::rotation_back(turn))
}

pub(super) fn from_game(at: [f32; 3], turn: [f32; 4]) -> Affine3A {
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
#[require(crate::rendering::shadow_only::ShadowOnly)]
pub struct CorpseHead;

/// What a death puts in the world, taken away when the run begins again.
#[derive(Component)]
pub struct OfDeath;

/// An outfit's skeleton on the corpse (`figure_view::Builder::outfit`):
/// for each of its rig's bones the corpse's bone it follows, that bone's
/// own matrix for a point of a mesh, and the entity its meshes hang on.
pub struct CorpseOutfit {
    pub item: String,
    pub rig: trp_data::character::outfit::Rig,
    pub theirs: Vec<Option<usize>>,
    pub rest: Vec<Affine3A>,
    pub ours: Vec<Entity>,
}

impl CorpseOutfit {
    /// Its skeleton's entities placed on the skeleton's bones as they stand
    /// (`world`: each bone in the skeleton's root's axes), as the body's
    /// outfits follow it (`body_view::see_outfit`): for the OVERALL tab's
    /// own figure as it moves (`figure_view`).
    pub fn placed(&self, world: &[Affine3A]) -> Vec<(Entity, Transform)> {
        use trp_data::character::outfit::Move;
        let theirs: Vec<Move> = self
            .theirs
            .iter()
            .zip(&self.rest)
            .map(|(bone, rest)| {
                let moved = bone.map_or(Affine3A::IDENTITY, |bone| world[bone]) * *rest;
                Move { axes: [moved.matrix3.x_axis.to_array(), moved.matrix3.y_axis.to_array(), moved.matrix3.z_axis.to_array()], shift: moved.translation.to_array() }
            })
            .collect();
        self.ours
            .iter()
            .zip(self.rig.pose(&theirs))
            .map(|(entity, moved)| {
                let turn = Quat::from_mat3(&Mat3::from_cols(moved.axes[0].into(), moved.axes[1].into(), moved.axes[2].into())).normalize();
                (*entity, Transform { translation: moved.shift.into(), rotation: turn, scale: Vec3::ONE })
            })
            .collect()
    }
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
/// (the hard settings' `CorpseMaxDepenetrationVelocity`).
#[derive(Resource)]
pub struct Ragdoll {
    pub data: Option<RagdollData>,
    pub max_depenetration: f32,
    /// The engine's settings (`globalgamemanagers`).
    pub physics: Option<trp_data::setup::engine::PhysicsSettings>,
}

/// What the run began with, put back when it begins again: the
/// controller as made (the character, its health, its hands), the
/// inventory's things and the range.
#[derive(Resource)]
pub struct StartOfRun {
    pub controller: Controller,
    pub equipment: trp_sim::items::inventory::Equipment,
    pub loot: trp_sim::items::inventory::Item,
    /// The crates hidden as the run began, to be searched again.
    pub searcher: trp_sim::items::search::Searcher,
    pub away: Vec<Option<trp_sim::items::inventory::Item>>,
    pub ground: Vec<(trp_sim::items::inventory::Item, Vec3)>,
    pub range: Range,
}

/// The run is begun again: what keeps state of its own about the hands
/// starts over (`inventory_view`).
#[derive(Message)]
pub struct BeginAgain;

/// A death under way.
#[derive(Resource, Default)]
pub struct Dying(pub(super) Option<Death>);

impl Dying {
    /// Seconds since the character died, if it has.
    pub fn since(&self) -> Option<f32> {
        self.0.as_ref().map(|death| death.since)
    }
}

pub struct Death {
    pub(super) since: f32,
    /// Each corpse bone's place in its parent as it stood at death, and the
    /// body that moves it now (the body's pose is the bone's).
    pub(super) locals: Vec<Transform>,
    pub(super) moved: Vec<Option<Body>>,
    /// The outfit the corpse wears, by its place among the corpse's.
    pub(super) outfit: Option<usize>,
    /// The chest's body, and where the character's eyes were in it.
    pub(super) eyes: Option<(Body, Transform)>,
    /// The weapon let go: its body and the bone it hangs from on the rig;
    /// and that bone's place in its parent as it was, to be put back.
    pub(super) weapon: Option<(Body, Entity)>,
    pub(super) weapon_rest: Option<(Entity, Transform)>,
    /// The game's engine with the corpse in it.
    pub(super) engine: World,
    /// The weapon heard as it lands (`landing_sound`), and each body's
    /// surface (its bone's hit collider's `SurfaceSound`).
    pub(super) landing: trp_sim::sound::landing::Landing,
    pub(super) surfaces: std::collections::HashMap<Body, u8>,
}

/// An affine as a transform.
pub(super) fn transform_of(affine: Affine3A) -> Transform {
    Transform::from_matrix(Mat4::from(affine))
}

/// Whether an entity hangs, at any depth, from another.
pub(super) fn under(entity: Entity, root: Entity, parents: &Query<&ChildOf>) -> bool {
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
    pub(super) stepper: Stepper,
    pub(super) now: f32,
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
