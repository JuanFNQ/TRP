//! Everything the movement simulation reads from the game's data, gathered
//! into one value. Sources are described in `docs/spec/movement.md`.

use anyhow::{anyhow, Context, Result};
use trp_data::formats::curve::Curve;
use trp_data::setup::hard_settings::HardSettings;
use trp_data::character::rootmotion::RootMotionTable;
use serde_json::Value as Json;

use crate::locomotion::head_look::HeadLookSettings;
use crate::base::math::{inverse_lerp, lerp};
use crate::locomotion::stamina::StaminaSettings;
use crate::locomotion::vaulting::VaultingSettings;

mod reading;
mod weights;

pub use weights::*;

#[derive(Clone)]
pub struct MovementSettings {
    /// Shared, not copied, by the settings for another weight
    /// (`reweighed`): its copy had cost some 9 ms every time what was
    /// carried changed.
    pub table: std::sync::Arc<RootMotionTable>,
    pub move_state: u32,
    pub sprint_state: u32,

    /// Carried weight in kilograms, which the inertia values derive from.
    pub weight: f32,
    /// 0 for an unburdened character, rising to 1 with load.
    pub inertia: f32,

    /// Upper end of the speed wheel.
    pub max_speed: f32,

    /// The figures of the sounds of moving (`docs/spec/footsteps.md`).
    pub steps: crate::sound::steps::StepSettings,
    pub sprinting_speed: f32,
    /// Both of those for an untrained character and at the top of the
    /// Strength skill, and the share of the top walking speed that carried
    /// weight leaves.
    walk_speed_range: [f32; 2],
    sprint_speed_range: [f32; 2],
    walk_speed_limit: f32,
    /// Speed cap from carried weight; 1 means no cap.
    pub state_speed_limit: f32,
    /// The Strength skill's level, 0 to 60.
    strength: f32,
    /// What the hard caps on speed shrink to for a character too heavy to walk freely.
    sprint_speed_lower_limit: f32,
    /// After a jump the selected speed is capped for a while: the share of
    /// the top walking speed taken off, and for how many seconds.
    pub jump_penalty: f32,
    pub jump_penalty_duration: f32,
    /// A fall of this many metres or more caps the selected speed too: at
    /// what, and for how many seconds, by the height fallen.
    pub fall_threshold: f32,
    pub fall_speed_limit: Curve,
    pub fall_limit_duration: Curve,

    pub walk_inertia: f32,
    pub side_inertia: f32,
    pub diagonal_inertia: f32,
    pub move_time: f32,
    pub max_time_without_input: f32,
    pub exit_speed_threshold: f32,
    pub min_direction_blend_time: f32,
    pub sudden_changes_smoothness: f32,
    pub crouch_speed_acceleration: [f32; 2],
    /// Crawling: what the wanted speed is multiplied by, and what the
    /// direction's smoothing time is divided by.
    pub prone_speed_acceleration: f32,
    pub prone_direction_acceleration: f32,
    /// How fast the clips that go down and get up play.
    pub transition_speed: f32,

    pub pre_sprint_acceleration: f32,
    pub sprint_acceleration: f32,
    pub physical_sprint_speed: f32,
    /// What a sprint multiplies its turn rate and turn limit by: 1 unless
    /// the character is overweight for sprinting (`Physical.SprintSensitivity`).
    pub sprint_sensitivity: f32,
    pub starting_sprint_speed: f32,
    pub rotation_frame_span: usize,
    pub sprint_speed_inertia: Curve,

    pub tilt_inertia: f32,
    pub tilt_max_speed: f32,
    pub inertia_backward_coef: f32,
    /// How fast the step aside and the step back play: where that speed
    /// rests, what it gains a second on the way, and its top (movement
    /// spec, "The sidestep").
    pub sideback_speed: f32,
    pub sideback_acceleration: f32,
    pub sideback_max_speed: f32,
    /// How fast the first-person pose of a body state comes in and goes
    /// out, for `Hands::weapon_flip_speed`.
    pub weapon_flip_speed: f32,

    pub pose_increase_speed: f32,
    pub pose_decrease_speed: f32,
    pub pose_changing_speed: f32,

    pub idle_motion_preservation: f32,
    /// What the Strength skill multiplies the jump's push-off by; 1 untrained.
    pub jump_boost: f32,
    /// How much of the last motion carries over while a sprint comes to a stop.
    pub sprint_stop_motion_preservation: f32,
    /// The body animator's `SprintInertia` for this load: the sprint's stop
    /// blends its quick clip (low) and its long one (high) by it
    /// (`Inertia.SprintBrakeInertia` at the inertia; movement spec, "The
    /// sprint's stop").
    pub sprint_brake_inertia: f32,
    pub inertia_input_max_speed: f32,
    pub deceleration_speed: f32,
    pub direction_lerp_speed: f32,
    pub rotation_lerp_speed: f32,
    /// How far the look direction may turn from the body before the body follows, in degrees.
    pub trunk_rotation_limit: f32,
    /// How far the head's free look turns it (`docs/spec/free-look.md`).
    pub head_look: HeadLookSettings,
    /// How fast a lying body is laid to the ground, per second.
    pub prone_align_speed: f32,
    /// The movement states, by the game's numbers, in which the sights
    /// cannot be up.
    pub no_aim_states: Vec<i32>,
    /// The movement states in which the hands take no input: a press of
    /// the trigger made in one is dropped (firing spec, "Pressing the
    /// trigger").
    pub no_hands_states: Vec<i32>,
    /// Refuse the prone key while a reload is in hand, as the player of
    /// an online raid does. The offline player does not, and every
    /// recording is of that one: off unless a host sets it (movement
    /// spec, "Prone").
    pub prone_key_refused_in_reload: bool,
    pub crouch_acceleration_weight: Curve,
    /// How far under the selected speed the walk's speed may not fall, by
    /// the walk's inertia. Its second key is keyed by a character's
    /// inertia, and the game keeps one curve for every character in the
    /// raid: here it is keyed by this character's own until a host says
    /// otherwise (`key_acceleration_range`; movement spec, "The floor's
    /// curve is shared").
    pub movement_acceleration_range: Curve,
    /// Where that second key lies for a character of no inertia and for
    /// one of full inertia (`MinMovementAccelerationRangeRight`,
    /// `MaxMovementAccelerationRangeRight`): time, value.
    pub acceleration_range_right: [[f32; 2]; 2],
    /// The inertia that second key was last keyed by: this character's own
    /// as its settings are made, or another's a host gave
    /// (`key_acceleration_range`). A host that shares the key among several
    /// characters, as the game does, finds by it whose settings were worked
    /// out again since it last shared it. TRP's app shares nothing: each of
    /// its characters goes by its own (a deliberate departure from the game).
    pub floor_key: f32,

    pub air_control_same: f32,
    pub air_control_back: f32,
    pub air_control_other: f32,
    pub air_min_speed: f32,
    pub air_lerp: f32,
    pub jump_descending_exit_time: f32,
    pub lift_velocity_by_speed: Curve,
    pub jump_delay_by_speed: Curve,
    pub gravity: f32,

    /// One notch of the speed wheel and of the stance wheel.
    pub delta_speed: f32,
    pub delta_level: f32,
    /// The rate the body's lean closes on one made slowly
    /// (`TILT_CHANGING_SPEED`): the share of what is left, a second.
    pub tilt_changing_speed: f32,

    /// How fast the stance rises and falls and how fast going down and
    /// getting up play, each from an unburdened character to a fully
    /// overweight one, and how overweight this one is. Spent legs count
    /// as overweight for these (stamina spec, "What running out does").
    pub pose_increase_range: [f32; 2],
    pub pose_decrease_range: [f32; 2],
    pub transition_speed_range: [f32; 2],
    pub overweight: f32,
    /// The weight's share of the sprint's own limits (`SprintOverweightLimits`),
    /// kept for the weight's notice (`encumbered`).
    sprint_overweight: f32,
    /// What Strength makes of the weight's limits (`WeightLimits`), which
    /// the inertia and every overweight share above were worked out with.
    pub limits: WeightLimits,
    /// What stamina runs on.
    pub stamina: StaminaSettings,
    /// What vaulting and climbing run on (vaulting spec).
    pub vaulting: VaultingSettings,
    /// How long after the key a container or a trunk does what it was told,
    /// and a searched container is asked to be shown, in seconds: the
    /// install's `DelayToOpenContainer` (0.55; the class's default is 0.5).
    /// A door waits a half second of its own (interactions spec).
    pub thing_answers_after: f32,
    /// The box a roll needs to that side and the ray its ground is felt
    /// with (`RollCheckShift`, `RollCheckSize`, `RollCheckGroundCastLength`:
    /// 0.5, 0 and 0.2 m from the ribcage over the feet, 0.25 by 0.1 by
    /// 2.1 m, and 0.5 m in the install; movement spec, "Room to move").
    pub roll_check_shift: [f32; 3],
    pub roll_check_size: [f32; 3],
    pub roll_check_cast: f32,
    /// The head's two capsules on the player prefab (movement spec, "The
    /// head's push out of walls"); none where the settings were made
    /// without the install's prefab, and then nothing pushes the head.
    pub head: Option<trp_data::character::player::HeadCollidersData>,
}
