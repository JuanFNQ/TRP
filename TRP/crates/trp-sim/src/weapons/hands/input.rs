//! What the hands carry and take in, the shakes and the turn-away state.

use super::*;

/// Where the body's animation holds the weapon, in the frame the weapon's
/// rig hangs in (x right, y up, z ahead, from the rig's origin, at true
/// size), and how far the first-person weapon follows it, 0 to 1.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Carried {
    pub position: [f32; 3],
    pub rotation: [f32; 4],
    pub authority: f32,
    /// The body's `Position_cache` share (`POSITION_CACHE_FOR_WEAPON_PROCEDURAL`
    /// times the first-person curve, each at most 1): above 0 the weapon goes
    /// to the body's bone by it alone, the body's turn taken whatever else
    /// plays (`PlayerBones.ShiftWeaponRoot`; 1 in blind fire).
    pub position_cache: f32,
}

/// The body's right palm where its animation has it, in the same frame, and
/// how much of the hold is still the first-person rig's: at 1 the hand goes
/// to the weapon, at 0 the weapon goes to the hand.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Held {
    pub position: [f32; 3],
    pub rotation: [f32; 4],
    pub weight: f32,
}

/// The camera for one rendered frame.
#[derive(Debug, Clone, Copy)]
pub struct CameraPose {
    /// Position and rotation (`xyzw`) beside the weapon root.
    pub position: [f32; 3],
    pub rotation: [f32; 4],
    /// Vertical field of view in degrees.
    pub field_of_view: f32,
}

/// What the rest of the controller tells the hands each step.
#[derive(Debug, Clone, Copy, Default)]
pub struct HandsInput {
    /// Look direction in degrees.
    pub yaw: f32,
    pub pitch: f32,
    /// Movement the controller asked for, and what it got, in metres per second.
    pub motion: [f32; 3],
    pub velocity: [f32; 3],
    /// The animator's speed parameter.
    pub speed: f32,
    /// In the walking state: the bob runs.
    pub walking: bool,
    /// The movement state changed into one other than the walk since the
    /// last step (`Movement::take_walk_stop`): the bob's curves are rewound
    /// (`Hands::rewind_walk`), whatever state was left. Leaving the walk
    /// rewinds them without it.
    pub walk_stop: bool,
    pub sprinting: bool,
    /// Standing still.
    pub idle: bool,
    /// A foot came down since the last step: the seconds since the
    /// footstep before it.
    pub footstep: Option<f32>,
    /// Smoothed lean, -5 to 5, and the lean being moved to.
    pub tilt: f32,
    pub tilt_target: f32,
    /// Stance being moved to and the smoothed stance, 0 crouched to 1 standing.
    pub pose_level: f32,
    pub smoothed_pose_level: f32,
    pub aiming: bool,
    /// The character lies down (the game's prone flag, up from the key).
    pub prone: bool,
    /// The character crawls: the turn-away from obstacles is off.
    pub crawling: bool,
    /// Length of the rendered frame this step falls in, in seconds; zero
    /// when the host steps once per frame.
    pub frame_dt: f32,
    /// How tired the character is and whether it holds its breath
    /// (`Movement::wind`): rested, unless a host says otherwise.
    pub wind: Wind,
}

/// The turn-away from obstacles ("Turn-away" in the spec): how far the
/// weapon is drawn back and turned for the depth by which it would stick
/// into something.
#[derive(Debug, Clone, Default)]
pub(super) struct TurnAway {
    /// The depth as last measured, in metres, and how far back along the
    /// barrel the position spring held the weapon then; whether what it
    /// met is another character (`OverlapsWithPlayer`).
    pub(super) depth: f32,
    pub(super) held_back: f32,
    pub(super) with_character: bool,
    /// The value in use, the blend toward the far pose, and the camera's
    /// shift as it follows.
    pub(super) value: f32,
    pub(super) far: f32,
    pub(super) camera_shift: f32,
    /// What the last step made of it: for the moved bone, for the
    /// camera's resting place along the view, and for the elbows' goals,
    /// left then right.
    pub(super) position: [f32; 3],
    pub(super) rotation: [f32; 3],
    pub(super) camera: f32,
    pub(super) elbows: [[f32; 3]; 2],
}

/// The force reaction's state (`ForceEffector`; weapon-handling spec,
/// "Force reaction"): the force gathered since the last step and its
/// eased sum, the steps still given to it, the blasts' shakes under way,
/// and its draws.
#[derive(Debug, Clone)]
pub(super) struct ForceReact {
    pub(super) force: [f32; 3],
    pub(super) sum: [f32; 3],
    pub(super) steps_left: i32,
    pub(super) shakes: Vec<Shake>,
    pub(super) rng: Rng,
}

/// A blast's shake under way (`GrenadeShake_CO`): whether its first kick
/// has come, the time to its next, the time it has left, its power and its
/// wait.
#[derive(Debug, Clone, Copy)]
pub(super) struct Shake {
    started: bool,
    next: f32,
    pub(super) left: f32,
    pub(super) power: f32,
    wait: f32,
}

impl Shake {
    pub(super) fn new(strength: f32) -> Self {
        Self { started: false, next: 0.0, left: strength * 0.75, power: strength * 1.2, wait: 0.1 }
    }

    /// A frame of the coroutine: the power of the kick it makes, if it
    /// makes one (the first at once; then, once its wait is up, with less
    /// left, three quarters of the power and a wait a tenth longer, while
    /// any time is left).
    pub(super) fn frame(&mut self, dt: f32) -> Option<f32> {
        if !self.started {
            self.started = true;
        } else {
            self.next -= dt;
            if self.next > 0.0 {
                return None;
            }
            self.left -= self.wait;
            self.power *= 0.75;
            self.wait *= 1.1;
        }
        (self.left > 0.0).then(|| {
            self.next = self.wait;
            self.power
        })
    }
}

impl Default for ForceReact {
    fn default() -> Self {
        Self { force: [0.0; 3], sum: [0.0; 3], steps_left: -1, shakes: Vec::new(), rng: Rng::new(0x464f_5243) }
    }
}

/// `Vector3.Slerp`: the direction turned by the share of the angle between,
/// the length blended; straight where either is nought or they are as
/// one (`PROVISIONAL`: the engine's own code is not read).
pub(crate) fn slerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    let length = |v: [f32; 3]| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    let (la, lb) = (length(a), length(b));
    let straight = || std::array::from_fn(|k| a[k] + (b[k] - a[k]) * t);
    if la < 1e-5 || lb < 1e-5 {
        return straight();
    }
    let dot = ((a[0] * b[0] + a[1] * b[1] + a[2] * b[2]) / (la * lb)).clamp(-1.0, 1.0);
    if dot > 1.0 - 1e-5 || dot < -1.0 + 1e-5 {
        return straight();
    }
    let angle = dot.acos() * t;
    let ua = a.map(|x| x / la);
    // The part of b's direction square to a's, as a unit.
    let ub = b.map(|x| x / lb);
    let side: [f32; 3] = std::array::from_fn(|k| ub[k] - ua[k] * dot);
    let ls = length(side);
    let length_now = la + (lb - la) * t;
    std::array::from_fn(|k| (ua[k] * angle.cos() + side[k] / ls * angle.sin()) * length_now)
}
