//! Ray hits, body references, the bullet world, bullets and hits.

use super::*;

/// What a ray meets (one hit for each collider, Unity's `RaycastNonAlloc`):
/// how far from the ray's start, where, the face's normal, the collider and
/// the ballistic thing it belongs to (several colliders may make one thing),
/// whether the collider is a mesh (the backward cast goes on through a
/// mesh), and what it is made of.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RayHit {
    pub distance: f32,
    pub point: V3,
    pub normal: V3,
    pub collider: u64,
    pub thing: u64,
    pub mesh: bool,
    pub surface: Surface,
    /// A character's hit collider (body hits spec), none for anything else.
    pub body: Option<BodyRef>,
}

/// A character's hit collider a ray met (body hits spec, "The hit
/// colliders"): whose (the host's number), its body part (`EBodyPart`),
/// collider type and plate (0 for a body part's own), and its damage share
/// (`penetrationDamageMod`). Its other figures are the hit's surface's.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BodyRef {
    pub character: u32,
    pub part: u8,
    pub collider: u8,
    pub plate: u16,
    pub damage_mod: f32,
}

impl BodyRef {
    /// `IsParentHasSameBodyPart`'s test of two hits: a plate's only the same
    /// plate of the same part; a plate and a body collider never the same.
    pub(super) fn same_part(&self, other: &BodyRef) -> bool {
        match (self.plate, other.plate) {
            (0, 0) => self.part == other.part,
            (a, b) if a != 0 && b != 0 => self.part == other.part && a == b,
            _ => false,
        }
    }
}

/// Which bullet asks (a replay's world answers by it; the app's ignores it).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Asker {
    /// The shot it is of (the host's number for it).
    pub shot: u32,
    pub fragment: i32,
    pub seed: i32,
    /// The cast looks along the bullet's way (the forward one), not back.
    pub forward: bool,
}

/// The world a bullet flies through: every collider on the bullets' layers
/// that a ray from `from` toward `to`, within its length, enters through a
/// face turned toward it (nothing it starts inside), one hit each.
pub trait BulletWorld {
    fn ray(&self, asker: Asker, from: V3, to: V3, hits: &mut Vec<RayHit>);

    /// The armour a character wears and its skills (body hits spec), none
    /// for one that wears none or is not known.
    fn wearer(&self, _character: u32) -> Option<&crate::condition::armor::Wearer> {
        None
    }

    /// A hit handed on as the game's shot delegate takes it, right after its
    /// bullet's step and before the bullets it made fly (`UpdateShots`): a
    /// body's damage is done here, so that its armour's wear is there for
    /// the next hit.
    fn delivered(&mut self, _hit: &Hit) {}
}

/// How a hit left the bullet (`EBulletState`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Flying,
    DeviationHit,
    RicochetHit,
    FragmentationHit,
    StopHit,
}

impl State {
    pub fn name(self) -> &'static str {
        match self {
            State::Flying => "Flying",
            State::DeviationHit => "DeviationHit",
            State::RicochetHit => "RicochetHit",
            State::FragmentationHit => "FragmentationHit",
            State::StopHit => "StopHit",
        }
    }
}

/// One bullet in flight (`Shot`).
#[derive(Debug, Clone)]
pub struct Bullet {
    pub shot: u32,
    pub fragment: i32,
    pub seed: i32,
    /// How many bullets came before it (`Parent` and theirs).
    pub depth: u32,
    /// How many of those ricocheted.
    pub(super) ricochets: u32,
    /// The things and colliders those before it hit.
    pub(super) before: Vec<(u64, u64)>,
    pub round: std::sync::Arc<Round>,
    /// The way it was made with (`Direction`: not always a unit).
    pub(super) direction: V3,
    pub initial_speed: f32,
    /// The speed it was made with (`Speed`).
    pub speed: f32,
    pub damage: f32,
    pub penetration: f32,
    pub(super) penetration_chance: f32,
    pub(super) ricochet_chance: f32,
    pub(super) fragmentation_chance: f32,
    pub(super) deviation_chance: f32,
    pub(super) min_fragments: i32,
    pub(super) max_fragments: i32,
    pub armor_damage: f32,
    pub(super) life: f32,
    /// Time since it was made.
    pub age: f32,
    pub position: V3,
    /// Where it was before its last step, and how many steps it has made
    /// (its `PositionHistory`'s last but one, and its length less one): what
    /// a tracer is drawn from. Its origin, and none, as it is made.
    pub previous: V3,
    pub steps: u32,
    pub velocity: V3,
    pub(super) trajectory: Trajectory,
    pub state: State,
    pub(super) done: bool,
    /// The thing and collider it hit.
    pub(super) hit_thing: u64,
    pub(super) hit_collider: u64,
    /// The pieces of armour that deflected and blocked it (`DeflectedBy`,
    /// `BlockedBy`: none for a bullet newly made).
    pub deflected_by: Option<u32>,
    pub blocked_by: Option<u32>,
    /// The forward hits on characters' colliders of the bullets before it,
    /// and the characters any of them hit (`AvoidAdditionalDamage`'s tests).
    pub(super) forward_parts: Vec<BodyRef>,
    pub(super) characters: Vec<u32>,
    /// Its hit on a character's collider, and whether it was forward.
    pub(super) hit_body: Option<BodyRef>,
    pub(super) hit_forward: bool,
    /// It does no damage (`AvoidAdditionalDamage`).
    pub no_damage: bool,
}

impl Bullet {
    pub(super) fn finished(&self) -> bool {
        self.done || self.age > self.life
    }

    /// It has hit something (`ShotProcessed`): what follows is bullets of
    /// their own.
    pub fn has_hit(&self) -> bool {
        self.done
    }
}

/// A hit, as the game's shot delegate sees it, and what came of it.
#[derive(Debug, Clone)]
pub struct Hit {
    pub shot: u32,
    pub fragment: i32,
    pub seed: i32,
    pub depth: u32,
    pub point: V3,
    pub normal: V3,
    pub forward: bool,
    pub thing: u64,
    pub collider: u64,
    pub surface: Surface,
    pub state: State,
    /// The speed it was made with.
    pub speed: f32,
    /// Its velocity at the hit.
    pub velocity: V3,
    pub damage: f32,
    pub penetration: f32,
    pub armor_damage: f32,
    /// The hit cosine: the face's normal on the segment's way.
    pub cos: f32,
    /// Its time since it was made, at the hit.
    pub age: f32,
    /// How many bullets it made.
    pub fragments: usize,
    /// Past its life: the delegate does nothing with it.
    pub out_of_life: bool,
    /// `IsFirstHit`: the shot's first bullet, or the one it turned into.
    pub first_hit: bool,
    /// A character's collider it hit, and by which armour pieces it was
    /// deflected and blocked there (body hits spec).
    pub body: Option<BodyRef>,
    pub deflected_by: Option<u32>,
    pub blocked_by: Option<u32>,
    /// It does no damage: an earlier bullet of its shot hit the same part
    /// (`AvoidAdditionalDamage`).
    pub no_damage: bool,
    /// The way the bullet was made with (`Direction`: not always a unit),
    /// which a killing hit pushes a corpse along.
    pub direction: V3,
    pub round: std::sync::Arc<Round>,
}

impl Hit {
    /// The fuse this hit sets off (`ClientGameWorld.ShotDelegate`): an explosive
    /// round's, within its life, at an age since its bullet was made of at least
    /// its arming time (a bullet it turned into starts its age afresh). Any
    /// other hit is a bullet's alone. Its damage is done all the same.
    pub fn sets_off(&self) -> Option<&Fuse> {
        if self.out_of_life {
            return None;
        }
        self.round.fuse.as_ref().filter(|fuse| self.age >= fuse.arm_time)
    }
}
