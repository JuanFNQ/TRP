//! The sounds of the player's own moving: footsteps, sprints, jumps, landings, crawls and turns.
//! After `docs/spec/footsteps.md`: when the game's player plays a footstep, a
//! sprint's, a stop, a jump, a landing, a crawl, a turn or its kit, from
//! which bank of which surface, how loud and at what blend value
//! (`Player`'s `Run`, `Sprint`, `Idle`, `StateChangedHandler`,
//! `PlayStepSound`, `PlayGroundedSound`, `PlaySoundBank`, `PlayTurnSound`,
//! `PlayGearSound`, `DefaultPlay`); and how a bank chooses its clips and
//! pitch (`SoundBank`). The movement runs the rules on every step
//! (`Movement::hear`) and the host plays what they ask for.

use anyhow::{Context, Result};
use serde_json::Value as Json;
use trp_data::setup::hard_settings::HardSettings;
use trp_data::audio::sound::Bank;

use crate::locomotion::movement::State;
use crate::weapons::recoil::Rng;

mod banks;
mod settings;
mod choosing;
mod update;

pub use banks::*;
pub use settings::*;
pub use choosing::*;
use settings::{CHECK_BUFFER};
use choosing::{delta_angle, inverse_lerp};

/// What the movement tells the rules on a step.
pub struct Moment {
    /// The movement's clock and its step.
    pub clock: f32,
    pub dt: f32,
    pub state: State,
    /// A footstep was heard on this step (`Movement::watch_footsteps`).
    pub footstep: bool,
    /// The crawl's marks passed on this step (`PlaySoundBank("Prone")`).
    pub crawl_marks: usize,
    /// The ground found on this step after being off it: how far below the
    /// highest point and how far above the take-off the feet are.
    pub landed: Option<(f32, f32)>,
    pub prone: bool,
    /// The stance and its smoothed value, 0 to 1.
    pub pose_level: f32,
    pub smoothed_pose_level: f32,
    /// The speed setting after its limits (`ClampedSpeed`), before them
    /// (`CharacterMovementSpeed`), and the top one (`MaxSpeed`).
    pub clamped_speed: f32,
    pub speed: f32,
    pub max_speed: f32,
    /// The movement's speed was set on this step (`SetCharacterMovementSpeed`:
    /// the wheel, a state entered, a sprint left), which alone has one's own
    /// rules work out again how quietly one walks.
    pub speed_set: bool,
    /// 0 to 1 by carried weight (`Physical.Overweight`), and for an
    /// upright walk (`WalkOverweight`), which cuts how quietly one walks.
    pub overweight: f32,
    pub walk_overweight: f32,
    pub leg_damaged: bool,
    /// The view's yaw, and the hands' angle from the body, in degrees.
    pub yaw: f32,
    pub hands_to_body: f32,
}

/// The rules' memory between steps.
#[derive(Clone, Debug)]
pub struct StepSounds {
    pub settings: StepSettings,
    /// The banks' figures by surface and bank, once the host has read
    /// them (`set_figures`): until then a bank's are 1 and 0.
    figures: Vec<(u8, [Option<Figures>; 9])>,
    /// The kit's banks' figures, by `trp_data::sound::GEAR_BANKS`' order
    /// (`set_kit_figures`).
    kit_figures: [Option<Figures>; 4],
    /// The surface the feet were last found on (`CurrentSurface`).
    pub surface: u8,
    /// The movement state as last seen, and the one before it
    /// (`MovementContext.PreviousState`).
    state: State,
    previous: State,
    /// A footstep heard since `Run` or `Sprint` began (`_playedAtLeastOneStep`).
    played: bool,
    /// Each kind's multiplier as last worked out (`_cachedMovementRolloff`,
    /// over the 70 m of a first-person rolloff).
    kept: [Option<f32>; 14],
    /// The kit's sounds waiting for their time (`Gear`): when, and how loud.
    gear_due: Vec<(f32, f32)>,
    /// The last turn sound's time (`_lastTimeTurnSound`), and the idle
    /// check's wait: until when, and whether the frame after it is skipped
    /// too (the `yield return null` after the wait).
    last_turn: f32,
    turn_wait: Option<(f32, bool)>,
    /// The yaw's turn on each of the last ten steps, in degrees a second
    /// (`AverageRotationSpeed`, a `RollingMedian(10)`), where the next goes,
    /// how many there are, and the last yaw.
    turns: [f32; 10],
    turn_at: usize,
    turn_count: usize,
    last_yaw: Option<f32>,
    /// No landing sound until then (`_nextJumpAfter`).
    next_landing: f32,
    rng: Rng,
    /// What the rules have asked for and the host has not taken.
    asked: Vec<Play>,
    /// Covert Movement's buffs (`skills::Covert`: every factor 1
    /// untrained), how quietly one walks (`CovertEfficiency`), and the
    /// speed it was last worked out at (`_lastSpeedCovertCheck`), or that it
    /// is to be worked out afresh.
    covert: crate::condition::skills::Covert,
    efficiency: f32,
    efficiency_speed: f32,
    efficiency_due: bool,
    /// A character seen from outside (`FirstPersonPointOfView` false): a
    /// footstep, a sprint's step and the stop take no volume of their
    /// bank's (`PlayStepSound`, `Sprint`, `StateChangedHandler`).
    pub third_person: bool,
    /// How far the listener is from a character seen from outside, set by
    /// the host before each step: past a surface check its steps, sprints,
    /// landings and crawls are not played. None for one's own, which is
    /// never out of range.
    pub listener: Option<f32>,
}

/// The surfaces the game counts as soft (`MovementContext.SoftSurface`:
/// asphalt, concrete, gravel, soil, wood, thick wood and puddle, by their
/// numbers in `BaseBallistic.ESurfaceSound`), which only Covert Movement's
/// figures go by.
fn soft(surface: u8) -> bool {
    matches!(surface, 0 | 2 | 3 | 5 | 7 | 9 | 12)
}

impl Default for StepSounds {
    fn default() -> Self {
        Self::new(StepSettings::default())
    }
}

/// The game's falloff of a turn's share with the stance: a full turn a
/// second standing, two crouched fully.
const TURN_FULL: f32 = 360.0;
/// The turn sound's own wait (`maxLengthTurnSound`).
const TURN_AGAIN: f32 = 0.6;
/// A drop the landing is heard from (`LandingThreshold`), and the share of
/// it that is heard whole (`LANDING_VOLUME_MULT`).
const LANDING_THRESHOLD: f32 = 0.3;
const LANDING_WHOLE: f32 = 2.5;
/// No footstep heard in a `Run` this long after the last, and one is
/// played as it is left.
const RUN_LEFT_QUIET: f32 = 0.66;
/// A crawl's mark is heard this long after the last footstep or mark.
const CRAWL_GAP: f32 = 0.5;
/// The smoothed stance under which a step is crouched low (`Player.Pose`).
const DUCK_BELOW: f32 = 0.11;
/// The slowest footstep's blend value at full overweight or with a leg
/// damaged (`PhysicalBase.MinStepSound`).
const MIN_STEP: f32 = 0.6;

impl StepSounds {
}

#[cfg(test)]
mod tests;
