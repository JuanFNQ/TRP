//! The player's body: its animator, run for what the movement and the
//! first-person view take from it. See `docs/spec/body-animator.md`.

use std::sync::Arc;

use anyhow::Result;
use trp_data::states::{AnimState, FirstPersonPose};
use trp_data::unity::Assets;

use crate::animator::{Animator, BoneRest, Controller, Event};
use crate::hands::{Carried, Held};
use crate::math::{quat_mul, quat_rotate};
use crate::PointOfView;

/// The bones followed, by their path below the animated object: the chain
/// from the root to the last spine bone, then what hangs from it.
const BONES: [&str; 20] = [
    "Root_Joint",
    "Root_Joint/Base HumanPelvis",
    "Root_Joint/Base HumanPelvis/Base HumanSpine1",
    "Root_Joint/Base HumanPelvis/Base HumanSpine1/Base HumanSpine2",
    "Root_Joint/Base HumanPelvis/Base HumanSpine1/Base HumanSpine2/Base HumanSpine3",
    "Root_Joint/Base HumanPelvis/Base HumanSpine1/Base HumanSpine2/Base HumanSpine3/Weapon_root_3rd_anim",
    "Root_Joint/Base HumanPelvis/Base HumanSpine1/Base HumanSpine2/Base HumanSpine3/Base HumanRibcage",
    "Root_Joint/Camera_animated_3rd",
    // The right arm, from the ribcage down to the palm.
    "Root_Joint/Base HumanPelvis/Base HumanSpine1/Base HumanSpine2/Base HumanSpine3/Base HumanRibcage/Base HumanRCollarbone",
    "Root_Joint/Base HumanPelvis/Base HumanSpine1/Base HumanSpine2/Base HumanSpine3/Base HumanRibcage/Base HumanRCollarbone/Base HumanRUpperarm",
    "Root_Joint/Base HumanPelvis/Base HumanSpine1/Base HumanSpine2/Base HumanSpine3/Base HumanRibcage/Base HumanRCollarbone/Base HumanRUpperarm/Base HumanRForearm1",
    "Root_Joint/Base HumanPelvis/Base HumanSpine1/Base HumanSpine2/Base HumanSpine3/Base HumanRibcage/Base HumanRCollarbone/Base HumanRUpperarm/Base HumanRForearm1/Base HumanRForearm2",
    "Root_Joint/Base HumanPelvis/Base HumanSpine1/Base HumanSpine2/Base HumanSpine3/Base HumanRibcage/Base HumanRCollarbone/Base HumanRUpperarm/Base HumanRForearm1/Base HumanRForearm2/Base HumanRForearm3",
    "Root_Joint/Base HumanPelvis/Base HumanSpine1/Base HumanSpine2/Base HumanSpine3/Base HumanRibcage/Base HumanRCollarbone/Base HumanRUpperarm/Base HumanRForearm1/Base HumanRForearm2/Base HumanRForearm3/Base HumanRPalm",
    // And the left.
    "Root_Joint/Base HumanPelvis/Base HumanSpine1/Base HumanSpine2/Base HumanSpine3/Base HumanRibcage/Base HumanLCollarbone",
    "Root_Joint/Base HumanPelvis/Base HumanSpine1/Base HumanSpine2/Base HumanSpine3/Base HumanRibcage/Base HumanLCollarbone/Base HumanLUpperarm",
    "Root_Joint/Base HumanPelvis/Base HumanSpine1/Base HumanSpine2/Base HumanSpine3/Base HumanRibcage/Base HumanLCollarbone/Base HumanLUpperarm/Base HumanLForearm1",
    "Root_Joint/Base HumanPelvis/Base HumanSpine1/Base HumanSpine2/Base HumanSpine3/Base HumanRibcage/Base HumanLCollarbone/Base HumanLUpperarm/Base HumanLForearm1/Base HumanLForearm2",
    "Root_Joint/Base HumanPelvis/Base HumanSpine1/Base HumanSpine2/Base HumanSpine3/Base HumanRibcage/Base HumanLCollarbone/Base HumanLUpperarm/Base HumanLForearm1/Base HumanLForearm2/Base HumanLForearm3",
    "Root_Joint/Base HumanPelvis/Base HumanSpine1/Base HumanSpine2/Base HumanSpine3/Base HumanRibcage/Base HumanLCollarbone/Base HumanLUpperarm/Base HumanLForearm1/Base HumanLForearm2/Base HumanLForearm3/Base HumanLPalm",
];
const SPINE: usize = 5;
const WEAPON: usize = 5;
const RIBCAGE: usize = 6;
const CAMERA: usize = 7;
/// The bones from the root down to the ribcage, and on to the right palm.
const TO_RIBCAGE: [usize; 6] = [0, 1, 2, 3, 4, RIBCAGE];
const TO_RIGHT_PALM: [usize; 12] = [0, 1, 2, 3, 4, RIBCAGE, 8, 9, 10, 11, 12, 13];
/// Each arm's bones, right then left: the collarbone, the upper arm, the
/// three of the forearm and the palm.
const ARMS: [[usize; 6]; 2] = [[8, 9, 10, 11, 12, 13], [14, 15, 16, 17, 18, 19]];
/// The left hand's finger bones follow the list above: fifteen, the thumb
/// first, each finger from the knuckle out.
const LEFT_FINGERS: usize = BONES.len();
pub const FINGER_BONES: usize = 15;
/// After the fingers, the two points the body's animation would have the
/// elbows bend toward, right then left. They hang from the last spine bone.
const BEND_GOALS: usize = LEFT_FINGERS + FINGER_BONES;
const BEND_GOAL_NAMES: [&str; 2] = ["Bend_Goal_Right", "Bend_Goal_Left"];

/// The paths of a hand's finger bones below its palm, in that order. The
/// skeleton numbers the fingers 1 to 5 and each one's bones 1 to 3.
fn finger_paths(palm: &str, side: char) -> Vec<String> {
    let mut paths = Vec::with_capacity(FINGER_BONES);
    for finger in 1..=5 {
        let mut path = palm.to_owned();
        for joint in 1..=3 {
            path.push_str(&format!("/Base Human{side}Digit{finger}{joint}"));
            paths.push(path.clone());
        }
    }
    paths
}

/// One arm as the body's animation poses it, for a host that draws arms.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ArmPose {
    /// How much of the hand is the body's and not the weapon's, 0 to 1:
    /// the arm's solver onto the weapon runs on what is left of 1.
    pub hand: f32,
    /// How much of the shoulder is the body's.
    pub shoulder: f32,
    /// How much of the elbow's bend is the body's: at 0 the elbow bends
    /// toward the weapon's own goal, at 1 toward `bend_goal`.
    pub elbow: f32,
    /// The point the body's animation would have the elbow bend toward,
    /// in the frame the weapon's rig hangs in.
    pub bend_goal: [f32; 3],
    /// The upper arm's place and turning in the frame the weapon's rig
    /// hangs in.
    pub upper: ([f32; 3], [f32; 4]),
    /// The three bones of the forearm and the palm, each in its parent:
    /// where it hangs and how it is turned.
    pub below: [([f32; 3], [f32; 4]); 4],
    /// The turning, each in its parent, of the hand's finger bones: the
    /// thumb first, each finger from the knuckle out. Given for the left
    /// hand only: the game lets the body's animation have that hand's
    /// fingers with the hand, and leaves the right hand's on the weapon.
    pub fingers: Option<[[f32; 4]; FINGER_BONES]>,
}

/// The states whose clips are read: standing, walking, sprinting, jumping,
/// lying down and vaulting on the base layer, the layers that bend the
/// body with the view, the layer of the hands' gestures on the world
/// and of the open inventory's pose (`docs/spec/interactions.md`), and
/// what only a character seen from outside plays: the body's own part in
/// a weapon drawn and put away, its reach for the vest in a reload, and
/// its breathing (`docs/spec/third-person.md`). The rest (the base
/// layer's own states for a door, grenades and blades, other characters)
/// are left out until the movement reaches them; the animator never
/// enters a state that was left out.
fn wanted(state: &AnimState) -> bool {
    const READ: [&str; 30] = [
        // Seen from outside only.
        "Weapon.INACTIVE",
        "Weapon.INACTIVE_PRONE",
        "Weapon.idle_to_out",
        "Weapon.out_to_idle",
        "UtilityOperation.",
        "Additive_FullBody.",
        // The base layer's own states for a door kicked: the approach, and
        // the kick that opens the door or does not.
        "Base Layer.Interactions.Approach",
        "Base Layer.Interactions.Stand_Kick",
        "Base Layer.Interactions.Stand_Kick_Fail",
        "InteractionsLayer.",
        "Base Layer.Vaulting.",
        "Sprint_ReloadHandsTrans.",
        "Base Layer.Stand.Idle_Aim",
        "Base Layer.Stand.Turn_Left_90_Aim",
        "Base Layer.Stand.Turn_Right_90_Aim",
        "Base Layer.Stand.SideStep_",
        "Base Layer.StateMachine_Move.MOVE",
        "Base Layer.StateMachine_Sprint.",
        "Base Layer.JUMP.",
        "Base Layer.Transition_Prone.",
        "Base Layer.Prone.Prone_Idle_Aim",
        "Base Layer.Prone.Prone",
        "Base Layer.Prone.Prone_Reload",
        "Additive_Aiming.Aiming",
        "Additive_Aiming.INACTIVE_SPRINT",
        "Additive_Aiming.INACTIVE_PRONE",
        "Additive_Aiming.Aiming_Prone",
        "Additive_PelvisOffsetOnLeaning.",
        "Additive_BodyTurningOnMove.",
        "Additive_ISaim.",
    ];
    // A name ending in a dot or an underscore stands for every state whose
    // name begins with it.
    let family = |name: &str| name.ends_with('.') || name.ends_with('_');
    const ROLL: [&str; 2] = ["Base Layer.Prone.SideStep_", "Additive_Aiming.Prone_SideStep_"];
    READ.iter().chain(&ROLL).any(|name| state.full_name == *name || (family(name) && state.full_name.starts_with(name)))
}

/// The game's numbers for the movement states the body's states name.
pub mod movement_state {
    pub const IDLE: i32 = 1;
    pub const PRONE_IDLE: i32 = 2;
    pub const PRONE_MOVE: i32 = 3;
    pub const RUN: i32 = 4;
    pub const SPRINT: i32 = 5;
    pub const JUMP: i32 = 6;
    pub const TRANSITION: i32 = 8;
    pub const BREACH_DOOR: i32 = 9;
    pub const SIDESTEP: i32 = 15;
    pub const APPROACH: i32 = 17;
    pub const PRONE_TO_STAND: i32 = 18;
    pub const TO_PRONE: i32 = 19;
    pub const ROLL: i32 = 22;
    pub const JUMP_LANDING: i32 = 23;
    pub const CLIMB_OVER: i32 = 24;
    pub const CLIMB_UP: i32 = 25;
    pub const VAULTING_FALL_DOWN: i32 = 26;
    pub const VAULTING_LANDING: i32 = 27;
}

/// The kind of thing in the hands, as the body's animator is told it: the
/// game's own numbers (`PlayerAnimator.EWeaponAnimationType`; body-animator
/// spec, "The kind of thing in hand").
pub mod hands_kind {
    pub const RIFLE: f32 = 0.0;
    pub const PISTOL: f32 = 1.0;
    pub const THROWN: f32 = 2.0;
    pub const KNIFE: f32 = 3.0;
    pub const EMPTY_HANDS: f32 = 4.0;
    pub const LAUNCHER: f32 = 5.0;
    /// The share of the way to the new kind that the animator's figure
    /// goes in a second of frames (`PlayerAnimatorSetWeaponIdSmooth`), and
    /// how near it has to be to be put there.
    pub const RATE: f32 = 7.0;
    pub const NEAR: f32 = 0.01;
}

/// Which part of a movement an animator state is, for movements with a way
/// in, a hold and a way out: the game's own numbers.
pub mod part {
    pub const HOLD: i32 = 0;
    pub const IN: i32 = 1;
    pub const OUT: i32 = 2;
}

/// A state of the base layer that names a movement state, as the animator
/// enters it: the movement state by the game's number, which part of the
/// movement the state is (`part`), the side it is for (1 right, -1
/// left, 0 neither), what it does to the view's turning (a factor on
/// each frame's turn and the most degrees a frame may turn), and whether
/// a thing may be looked at to be worked in it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Entered {
    pub state: i32,
    pub part: i32,
    pub side: f32,
    pub turn_factor: f32,
    pub turn_limit: f32,
    pub can_interact: bool,
}

#[derive(Clone, Copy)]
struct Ids {
    speed: Option<usize>,
    direct_x: Option<usize>,
    direct_y: Option<usize>,
    direct: Option<usize>,
    level: Option<usize>,
    tilt: Option<usize>,
    inert: Option<usize>,
    inert_float: Option<usize>,
    sprint: Option<usize>,
    sprint_speed: Option<usize>,
    jumping: Option<usize>,
    landing: Option<usize>,
    falling: Option<usize>,
    grounded: Option<usize>,
    prone: Option<usize>,
    transition_speed: Option<usize>,
    aim_angle: Option<usize>,
    delta_rotation: Option<usize>,
    is_aiming: Option<usize>,
    prone_reload: Option<usize>,
    reload_float: Option<usize>,
    step: Option<usize>,
    sidestep: Option<usize>,
    sideback_speed: Option<usize>,
    weapon_size: Option<usize>,
    weapon_type: Option<usize>,
    weapon_type_float: Option<usize>,
    foot_step: Option<usize>,
    weapon_3rd: Option<usize>,
    first_person: Option<usize>,
    hand_right: Option<usize>,
    hand_left: Option<usize>,
    shoulder_right: Option<usize>,
    shoulder_left: Option<usize>,
    elbow_right: Option<usize>,
    elbow_left: Option<usize>,
    aiming_layer: Option<usize>,
    vaulting_height: Option<usize>,
    vaulting_length: Option<usize>,
    over_obstacle: Option<usize>,
    forward_velocity: Option<usize>,
    do_vault: Option<usize>,
    do_climb: Option<usize>,
    is_vaulting: Option<usize>,
    vaulting_speed: Option<usize>,
    mouse_look: Option<usize>,
    is_interaction: Option<usize>,
    interaction_index: Option<usize>,
    inventory: Option<usize>,
    inventory_operation: Option<usize>,
    kick: Option<usize>,
    kick_succeed: Option<usize>,
    approached: Option<usize>,
    third_person: Option<usize>,
    third_person_float: Option<usize>,
    first_action: Option<usize>,
    speed_draw: Option<usize>,
}

/// The two layers of the body's animator the game weighs by who is
/// looking, by the numbers its code has for them: the body's own part in
/// what the hands do with a weapon (`Weapon`), off in first person, and
/// the reach for the vest (`UtilityOperation`), weighed each frame for a
/// character seen from outside (`MovementContext.PlayerAnimatorPointOfView`,
/// `Player.AdjustUtilityLayerWeight`).
const WEAPON_LAYER: usize = 1;
const UTILITY_LAYER: usize = 2;
/// How near the left hand's marker has to come to the weapon's marker for
/// the vest for that reach to begin, and how near for all of it, in
/// metres; and the share of its weight the layer loses a second once no
/// reload is in hand (`Player.UtilityLayerRange`, `UtilityLayerLerpSpeed`:
/// figures of the game's code, not of the install).
const UTILITY_RANGE: [f32; 2] = [0.5, 0.2];
const UTILITY_FADE: f32 = 3.0;

/// The body's controller, read once and shared by every body made from it.
#[derive(Clone)]
pub struct BodyData {
    controller: Arc<Controller>,
    ids: Ids,
    /// The layer the game fades by the `Aiming_Layer` curve.
    faded_layer: usize,
    /// The length written on the sidestep's way in.
    sidestep_length: f32,
    /// How many further bones a host asked to have posed, and which of
    /// them, if any, is the root the clips move along the ground.
    drawn: usize,
    drawn_root: Option<usize>,
    /// When in its state the gesture of a backpack's drop has the mark
    /// for the pack leaving the shoulders, as a share of the state.
    backpack_drop: Option<f32>,
    /// Which of the further bones asked for are the neck and the head,
    /// and the share of the free look's turn each takes
    /// (`PlayerBones.NeckChain`, `HeadChain`).
    head_bones: [Option<usize>; 2],
    head_shares: [f32; 2],
}

/// The neck and the head, by the ends of their paths on the skeleton.
const HEAD_BONES: [&str; 2] = ["/Base HumanNeck", "/Base HumanNeck/Base HumanHead"];
/// The turn under which the game leaves the head as the animation has it:
/// a tenth of a degree, as the length of the turn's three angles.
const HEAD_TURN_LEAST: f32 = 0.01;

/// The state of the interactions' layer for a backpack's drop, and the
/// mark in it the game's logic waits for.
const BACKPACK_DROP: &str = "InteractionsLayer.10 Backpack Drop";
const BACKPACK_DROP_MARK: &str = "OnBackpackDrop";

impl BodyData {
    pub fn load(assets: &mut Assets) -> Result<Self> {
        Self::load_with(assets, &[])
    }

    /// As `load`, with further bones for the animator to pose, each named
    /// by its path below the animated object: a host that draws the body
    /// names the bones its meshes hang on, and `Body::drawn_bone` gives
    /// each as the animation has it.
    pub fn load_with(assets: &mut Assets, drawn: &[String]) -> Result<Self> {
        let table = trp_data::body::controller(assets)?;
        let mut paths: Vec<String> = BONES.iter().map(|path| (*path).to_owned()).collect();
        paths.extend(finger_paths(BONES[ARMS[1][5]], 'L'));
        paths.extend(BEND_GOAL_NAMES.iter().map(|name| format!("{}/{name}", BONES[4])));
        paths.extend(drawn.iter().cloned());
        let paths: Vec<&str> = paths.iter().map(String::as_str).collect();
        let bones = trp_data::body::bones(assets, &paths)?
            .into_iter()
            .map(|b| BoneRest { path: b.path, position: b.position, rotation: b.rotation })
            .collect();
        let backpack_drop = table.states.iter().find(|state| state.full_name == BACKPACK_DROP).and_then(|state| state.event_time(BACKPACK_DROP_MARK));
        let mut controller = Controller::load(assets, &table, bones, &wanted)?;
        // The first bone followed is the one the clips move along the ground.
        controller.root_bone = Some(0);
        let id = |name: &str| controller.parameter(name);
        let ids = Ids {
            speed: id("Speed"),
            direct_x: id("Direct_X"),
            direct_y: id("Direct_Y"),
            direct: id("Direct"),
            level: id("Level"),
            tilt: id("Tilt"),
            inert: id("Inert"),
            inert_float: id("InertFloat"),
            sprint: id("Sprint"),
            sprint_speed: id("SprintSpeed"),
            jumping: id("IsJumping"),
            landing: id("Landing"),
            falling: id("FallingDown"),
            grounded: id("IsGrounded"),
            prone: id("Prone"),
            transition_speed: id("TransitionMultiplier"),
            aim_angle: id("Aim_angle"),
            delta_rotation: id("DeltaRotation"),
            is_aiming: id("isAiming"),
            prone_reload: id("ProneReload"),
            reload_float: id("ReloadFloat"),
            step: id("Step"),
            sidestep: id("SidestepFloat"),
            sideback_speed: id("SidebackSpeed"),
            weapon_size: id("WeapSizeModifier"),
            weapon_type: id("WeaponType"),
            weapon_type_float: id("WeaponTypeFloat"),
            foot_step: id("FootStep"),
            weapon_3rd: id("Weapon_3rd"),
            first_person: id("First_Person_Curve_Weight"),
            hand_right: id("Hand_Right"),
            hand_left: id("Hand_Left"),
            shoulder_right: id("Shoulder_Right"),
            shoulder_left: id("Shoulder_Left"),
            elbow_right: id("Elbow_Right"),
            elbow_left: id("Elbow_Left"),
            aiming_layer: id("Aiming_Layer"),
            vaulting_height: id("VaultingHeight"),
            vaulting_length: id("VaultingLength"),
            over_obstacle: id("OverObstacleHeight"),
            forward_velocity: id("AbsoluteForwardVelocity"),
            do_vault: id("DoVault"),
            do_climb: id("DoClimb"),
            is_vaulting: id("IsVaulting"),
            vaulting_speed: id("Vaulting_speed"),
            mouse_look: id("Mouse_look"),
            is_interaction: id("IsInteraction"),
            interaction_index: id("InteractionIndex"),
            inventory: id("Inventory"),
            inventory_operation: id("InventoryOperation"),
            kick: id("Kick"),
            kick_succeed: id("KickSucceed"),
            approached: id("Approached"),
            third_person: id("ThirdPerson"),
            third_person_float: id("ThirdPersonFloat"),
            first_action: id("FirstAction"),
            speed_draw: id("SpeedDraw"),
        };
        let base = controller.layers.first().map_or(0, |layer| layer.machine);
        let sidestep_length = controller
            .machines
            .get(base)
            .into_iter()
            .flat_map(|machine| &machine.states)
            .filter_map(|state| state.movement.as_ref())
            .find(|script| script.state == movement_state::SIDESTEP && script.kind == part::IN)
            .map_or(0.0, |script| script.length);
        let drawn_root = drawn.iter().position(|path| path == BONES[0]);
        let head_bones = HEAD_BONES.map(|end| drawn.iter().position(|path| path.ends_with(end)));
        // The prefab's own shares; a half each where they cannot be read
        // (what this install's prefab has).
        let head_shares = trp_data::player::head_chain(assets).unwrap_or([0.5, 0.5]);
        Ok(Self {
            controller: Arc::new(controller),
            ids,
            faded_layer: 6,
            sidestep_length,
            drawn: drawn.len(),
            drawn_root,
            backpack_drop,
            head_bones,
            head_shares,
        })
    }

    /// How many clips were read for it.
    pub fn clip_count(&self) -> usize {
        self.controller.clips.len()
    }

    /// How long one of the animator's states lasts at its own speed, in
    /// seconds, with the name of the first clip it plays: for looking at.
    pub fn state_length(&self, full_name: &str) -> Option<(f32, String)> {
        let state = self.controller.machines.iter().flat_map(|machine| &machine.states).find(|state| state.name == full_name)?;
        let clip = state.trees.first()?.as_ref()?.iter().find_map(|node| node.clip)?;
        let clip = &self.controller.clips.get(clip)?.clip;
        Some((clip.duration() / state.speed.abs().max(1e-6), clip.name.clone()))
    }
}

/// What the movement tells the body's animator each step.
#[derive(Debug, Clone, Copy, Default)]
pub struct BodyInput {
    /// The smoothed speed setting, 0 to 1.
    pub speed: f32,
    pub direction: [f32; 2],
    /// The direction as one number: left 0, front 1, right 2, back 3.
    pub discrete_direction: f32,
    /// The smoothed stance, 0 crouched to 1 standing.
    pub level: f32,
    pub tilt: f32,
    /// The player is asking to move.
    pub inert: bool,
    pub sprint: bool,
    pub sprint_speed: f32,
    pub jumping: bool,
    pub landing: bool,
    pub falling: bool,
    pub grounded: bool,
    /// The character is lying down or going to.
    pub prone: bool,
    /// How fast the clips that go down and get up play; 1 for a rested,
    /// unburdened character.
    pub transition_speed: f32,
    /// The view's pitch, degrees, positive downward.
    pub aim_angle: f32,
    /// The angle from the body to the view.
    pub delta_rotation: f32,
    /// The sights are up: an additive layer leans the body into them.
    pub aiming: bool,
    /// A reload is in hand. Lying down, the animator holds the body still
    /// for it; upright no state answers.
    pub reloading: bool,
    /// The hands are busy with the weapon, which in a sprint brings it up
    /// into view.
    pub hands_busy: bool,
    /// The step aside asked for: 1 right, -1 left, 0 none.
    pub step: i32,
    /// How far the body is stepped aside, -1 left to 1 right, which picks
    /// the clips that bend it with the view.
    pub sidestep: f32,
    /// How fast the step aside and the step back play.
    pub sideback_speed: f32,
    /// A vault or a climb has been chosen, and one is under way.
    pub do_vault: bool,
    pub do_climb: bool,
    pub vaulting: bool,
    /// The obstacle as last given: its height and length, the drop behind
    /// it, and the forward speed it was measured at.
    pub vault_figures: [f32; 4],
    /// The pace of a vault's and a climb's clips.
    pub vaulting_speed: f32,
    /// The gesture the interactions' layer is to play, by its number, or
    /// none (`IsInteraction`, `InteractionIndex`).
    pub interaction: Option<u8>,
    /// The inventory is open and the hands have gone down for it, and
    /// whether they are meanwhile doing something else (`Inventory`,
    /// `InventoryOperation`): the layer's pose for an open inventory.
    pub inventory: bool,
    pub inventory_operation: bool,
    /// A door is being kicked, whether the kick will open it, and whether
    /// the character stands where it is to (`Kick`, `KickSucceed`,
    /// `Approached`).
    pub kick: bool,
    pub kick_succeeds: bool,
    pub approached: bool,
}

/// The layer of the body's animator that plays the hands' gestures on the
/// world, and its resting state.
const INTERACTIONS_LAYER: &str = "InteractionsLayer";
const INTERACTIONS_REST: &str = "InteractionsLayer.Idle";

pub struct Body {
    /// The interactions' layer and its machine, where the controller has
    /// them, and the gestures whose states the last update left, by
    /// number.
    interactions: Option<(usize, usize)>,
    finished: Vec<u8>,
    /// The mark of a backpack's drop: when in its state it comes, whether
    /// the state playing has passed it, and whether that is still to be
    /// told.
    backpack_drop: Option<f32>,
    backpack_passed: bool,
    backpack_dropped: bool,
    animator: Animator,
    ids: Ids,
    faded_layer: usize,
    sidestep_length: f32,
    pose_changes: Vec<Option<FirstPersonPose>>,
    base_machine: usize,
    /// The movement states named by the base layer's states that the last
    /// update left.
    left: Vec<i32>,
    /// How many further bones are posed for a host that draws the body,
    /// and which of them is the root.
    drawn: usize,
    drawn_root: Option<usize>,
    /// The kind of thing in the hands (`hands_kind`), and the figure the
    /// animator's trees have for it, which follows it over some frames.
    kind: f32,
    kind_now: f32,
    /// Whose eyes the character is seen with, and for one seen from
    /// outside the weight of the layer that reaches for the vest.
    point_of_view: PointOfView,
    utility_weight: f32,
    /// The neck and the head among the bones drawn, the share of the free
    /// look's turn each takes, and that turn as last told.
    head_bones: [Option<usize>; 2],
    head_shares: [f32; 2],
    head_turn: [f32; 3],
}

impl Body {
    /// The gestures that came to their end since this was last asked, by
    /// number.
    pub fn take_finished_interactions(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.finished)
    }

    /// Whether the gesture of a backpack's drop has passed its mark for
    /// the pack leaving the shoulders since this was last asked.
    pub fn take_backpack_dropped(&mut self) -> bool {
        std::mem::take(&mut self.backpack_dropped)
    }

    /// Puts the interactions' layer back to rest outright, whatever it
    /// was playing (the game's `ResetInteraction`).
    pub fn reset_interactions(&mut self) {
        let Some((layer, _)) = self.interactions else { return };
        if let Some(rest) = self.animator.controller.state(layer, INTERACTIONS_REST) {
            self.animator.play(layer, rest, 0.0);
        }
    }

    /// The state the interactions' layer is in and how far through it, and
    /// the state it is on its way into: for looking at.
    pub fn interaction_state(&self) -> Option<(&str, f32, Option<&str>)> {
        let (layer, _) = self.interactions?;
        let (current, next) = self.animator.state(layer);
        Some((current.name.as_str(), self.animator.progress(layer).0, next.map(|state| state.name.as_str())))
    }

    /// A body standing still. `weapon_width` is the weapon's width in
    /// inventory cells, which picks how the body carries it.
    pub fn new(data: &BodyData, weapon_width: f32) -> Self {
        let mut animator = Animator::new(data.controller.clone());
        if let Some(size) = data.ids.weapon_size {
            animator.set(size, weapon_width);
        }
        let base_machine = data.controller.layers.first().map_or(0, |l| l.machine);
        let interactions = data.controller.layer(INTERACTIONS_LAYER).map(|layer| (layer, data.controller.layers[layer].machine));
        let mut body = Self {
            interactions,
            finished: Vec::new(),
            backpack_drop: data.backpack_drop,
            backpack_passed: false,
            backpack_dropped: false,
            animator,
            ids: data.ids,
            faded_layer: data.faded_layer,
            sidestep_length: data.sidestep_length,
            pose_changes: Vec::new(),
            base_machine,
            left: Vec::new(),
            drawn: data.drawn,
            drawn_root: data.drawn_root,
            kind: hands_kind::RIFLE,
            kind_now: hands_kind::RIFLE,
            point_of_view: PointOfView::FirstPerson,
            utility_weight: 0.0,
            head_bones: data.head_bones,
            head_shares: data.head_shares,
            head_turn: [0.0; 3],
        };
        // A character is made for its own eyes until told otherwise: the
        // layer of the body's own part in the weapon's handling is off.
        body.animator.set_layer_weight(WEAPON_LAYER, 0.0);
        // Settle the pose and the curves of the starting state: a body
        // that has been standing for a while.
        body.animator.settle();
        body.update(
            0.0,
            &BodyInput {
                speed: 1.0,
                level: 1.0,
                grounded: true,
                transition_speed: 1.0,
                vault_figures: [0.0, 1.0, 0.0, 0.0],
                vaulting_speed: 1.0,
                ..BodyInput::default()
            },
        );
        body
    }

    /// Tells the animator how wide the weapon in hand is, in inventory cells.
    pub fn set_weapon_width(&mut self, cells: f32) {
        if let Some(size) = self.ids.weapon_size {
            self.animator.set(size, cells);
        }
    }

    /// Tells the animator what kind of thing is in the hands (`hands_kind`).
    /// The whole number is the animator's at once; the figure its blend
    /// trees go by follows over the next frames, a share of the way each
    /// frame (`update`), as the game's does for a character that has hands.
    pub fn set_hands_kind(&mut self, kind: f32) {
        self.kind = kind;
        if let Some(id) = self.ids.weapon_type {
            self.animator.set(id, kind);
        }
    }

    /// The figure the animator's trees have for the kind of thing in hand.
    pub fn hands_kind(&self) -> f32 {
        self.kind_now
    }

    /// Whose eyes the character is seen with (`Player.PointOfView`). Seen
    /// from outside the animator has its flag and its figure for that, and
    /// the layer of the body's own part in the weapon's handling is on;
    /// what the clips' curves hand over of the weapon and the arms then
    /// no longer waits on `First_Person_Curve_Weight`, which is taken as
    /// 1 (`carried`, `arm_pose`, `held`). The layer that reaches for the
    /// vest keeps whatever weight it last had, as in the game.
    pub fn set_point_of_view(&mut self, point_of_view: PointOfView) {
        self.point_of_view = point_of_view;
        let outside = point_of_view == PointOfView::ThirdPerson;
        for id in [self.ids.third_person, self.ids.third_person_float].into_iter().flatten() {
            self.animator.set(id, outside as u8 as f32);
        }
        self.animator.set_layer_weight(WEAPON_LAYER, outside as u8 as f32);
    }

    pub fn point_of_view(&self) -> PointOfView {
        self.point_of_view
    }

    /// What the animator of the thing in hand says the body is to do with
    /// it, by the game's number (the mark `ThirdAction` of a weapon's
    /// states: 1 as it comes to the hands, 2 as it leaves them). The
    /// animator is told whoever is looking; only a character seen from
    /// outside shows it.
    pub fn set_weapon_action(&mut self, action: i32) {
        if let Some(id) = self.ids.first_action {
            self.animator.set(id, action as f32);
        }
    }

    /// How fast the body's own part in a weapon drawn and put away plays
    /// (`SpeedDraw`: the weapon's animator's figure, told to the body's
    /// too).
    pub fn set_draw_speed(&mut self, speed: f32) {
        if let Some(id) = self.ids.speed_draw {
            self.animator.set(id, speed);
        }
    }

    /// Weighs the layer that reaches for the vest, for a character seen
    /// from outside, once a frame (`Player.AdjustUtilityLayerWeight`).
    /// `reach` is how far the left hand's marker is from the weapon's
    /// marker for the vest, in metres, while a reload is in hand or the
    /// inventory is open, and the character is not sprinting; `None`
    /// otherwise, and the layer then fades. A weapon with no such marker
    /// has the layer off (`with_markers` false).
    pub fn weigh_reach_for_vest(&mut self, with_markers: bool, reach: Option<f32>, dt: f32) {
        if !with_markers {
            self.animator.set_layer_weight(UTILITY_LAYER, 0.0);
            return;
        }
        self.utility_weight = match reach {
            Some(distance) => ((distance - UTILITY_RANGE[0]) / (UTILITY_RANGE[1] - UTILITY_RANGE[0])).clamp(0.0, 1.0),
            None => self.utility_weight + (0.0 - self.utility_weight) * (dt * UTILITY_FADE).clamp(0.0, 1.0),
        };
        self.animator.set_layer_weight(UTILITY_LAYER, self.utility_weight);
    }

    /// The weight of the layer that reaches for the vest, as last set.
    pub fn reach_for_vest(&self) -> f32 {
        self.animator.layer_weight(UTILITY_LAYER)
    }

    /// How far the free look has the head turned, as the bones take it
    /// (`HeadLook::bones_turn`): after the animator has posed them the
    /// game turns the neck and the head each by its share of it, in the
    /// bone's own axes (`PlayerBones.RotateHead`), for the character's
    /// own eyes (whose head only casts a shadow) and for any other's.
    /// `drawn_bones` gives them so.
    pub fn set_head_turn(&mut self, turn: [f32; 3]) {
        self.head_turn = turn;
    }

    /// What the clips' curves that hand the weapon and the arms over to
    /// the body are weighed by: the clips' own `First_Person_Curve_Weight`
    /// for the character's own eyes, and 1 for anyone else's.
    fn first_person_weight(&self) -> f32 {
        match self.point_of_view {
            PointOfView::FirstPerson => self.ids.first_person.map_or(0.0, |id| self.animator.get(id)),
            PointOfView::ThirdPerson => 1.0,
        }
    }

    /// Advances by `dt` seconds. Returns the states the animator entered
    /// that name a movement state, in the order it entered them.
    pub fn update(&mut self, dt: f32, input: &BodyInput) -> Vec<Entered> {
        let ids = self.ids;
        self.kind_now = if (self.kind_now - self.kind).abs() > hands_kind::NEAR {
            self.kind_now + (self.kind - self.kind_now) * (dt * hands_kind::RATE).clamp(0.0, 1.0)
        } else {
            self.kind
        };
        if let Some(id) = ids.weapon_type_float {
            self.animator.set(id, self.kind_now);
        }
        let a = &mut self.animator;
        let mut set = |id: Option<usize>, value: f32| {
            if let Some(id) = id {
                a.set(id, value);
            }
        };
        let flag = |b: bool| b as u8 as f32;
        set(ids.speed, input.speed);
        set(ids.direct_x, input.direction[0]);
        set(ids.direct_y, input.direction[1]);
        set(ids.direct, input.discrete_direction);
        set(ids.level, input.level);
        set(ids.tilt, input.tilt);
        set(ids.inert, flag(input.inert));
        set(ids.inert_float, flag(input.inert));
        set(ids.sprint, flag(input.sprint));
        set(ids.sprint_speed, input.sprint_speed);
        set(ids.jumping, flag(input.jumping));
        set(ids.landing, flag(input.landing));
        set(ids.falling, flag(input.falling));
        set(ids.grounded, flag(input.grounded));
        set(ids.prone, flag(input.prone));
        set(ids.transition_speed, input.transition_speed);
        set(ids.aim_angle, input.aim_angle);
        set(ids.delta_rotation, input.delta_rotation);
        set(ids.is_aiming, flag(input.aiming));
        set(ids.prone_reload, flag(input.reloading));
        set(ids.reload_float, flag(input.hands_busy));
        set(ids.step, input.step as f32);
        set(ids.sidestep, input.sidestep);
        set(ids.sideback_speed, input.sideback_speed);
        set(ids.do_vault, flag(input.do_vault));
        set(ids.do_climb, flag(input.do_climb));
        set(ids.is_vaulting, flag(input.vaulting));
        set(ids.vaulting_height, input.vault_figures[0]);
        set(ids.vaulting_length, input.vault_figures[1]);
        set(ids.over_obstacle, input.vault_figures[2]);
        set(ids.forward_velocity, input.vault_figures[3]);
        set(ids.vaulting_speed, input.vaulting_speed);
        set(ids.is_interaction, flag(input.interaction.is_some()));
        if let Some(index) = input.interaction {
            set(ids.interaction_index, index as f32);
        }
        set(ids.inventory, flag(input.inventory));
        set(ids.inventory_operation, flag(input.inventory_operation));
        set(ids.kick, flag(input.kick));
        set(ids.kick_succeed, flag(input.kick_succeeds));
        set(ids.approached, flag(input.approached));
        // The game fades one layer by a curve of the clips playing.
        let fade = ids.aiming_layer.map_or(0.0, |id| a.get(id));
        a.set_layer_weight(self.faded_layer, 1.0 - fade);

        let events = self.animator.update(dt);
        // A gesture is over when its state has been left: the state's own
        // script says so in the game. The state's name begins with the
        // number its way in asks for.
        if let Some((_, interactions)) = self.interactions {
            self.finished.extend(events.iter().filter_map(|event| match event {
                Event::Left { machine, state } if *machine == interactions => {
                    let name = self.animator.state_def(*machine, *state).name.as_str();
                    name.rsplit('.').next()?.split(' ').next()?.parse::<u8>().ok()
                }
                _ => None,
            }));
        }
        // The backpack's drop has a mark of its own in its state, which
        // comes once as the state plays past it, entering or entered.
        if let (Some((layer, _)), Some(at)) = (self.interactions, self.backpack_drop) {
            let (current, next) = self.animator.state(layer);
            let time = if current.name == BACKPACK_DROP {
                Some(self.animator.progress(layer).0)
            } else if next.is_some_and(|next| next.name == BACKPACK_DROP) {
                self.animator.next_progress(layer)
            } else {
                None
            };
            match time {
                Some(time) if time >= at && !self.backpack_passed => (self.backpack_passed, self.backpack_dropped) = (true, true),
                Some(_) => {}
                None => self.backpack_passed = false,
            }
        }
        let named = |machine: &usize, state: &usize| {
            (*machine == self.base_machine)
                .then(|| self.animator.state_def(*machine, *state).movement.as_ref().map(|m| m.state))
                .flatten()
        };
        self.left = events
            .iter()
            .filter_map(|event| match event {
                Event::Left { machine, state } => named(machine, state),
                _ => None,
            })
            .collect();
        let mut entered = Vec::new();
        for event in &events {
            let Event::Entered { machine, state } = event else { continue };
            if *machine != self.base_machine {
                continue;
            }
            let Some(script) = self.animator.state_def(*machine, *state).movement.as_ref() else { continue };
            // The game numbers the directions; only the two sides are used
            // by what is built.
            let side = match script.direction {
                1 => 1.0,
                5 => -1.0,
                _ => 0.0,
            };
            entered.push(Entered {
                state: script.state,
                part: script.kind,
                side,
                turn_factor: script.sensitivity,
                turn_limit: script.rotation_speed_clamp,
                can_interact: script.can_interact,
            });
            // Every such state says what first-person pose it has, or
            // that it has none.
            self.pose_changes.push(script.first_person_pose.clone());
        }
        entered
    }

    /// The first-person pose of each scripted state entered since this was
    /// last asked, in order: `None` for a state that has none. For
    /// `Hands::assign_pose`.
    pub fn take_poses(&mut self) -> Vec<Option<FirstPersonPose>> {
        std::mem::take(&mut self.pose_changes)
    }

    /// The length the game's sidestep state runs its parameter over, in
    /// seconds: the one its ways in and out are given (movement spec, "The
    /// sidestep").
    pub fn sidestep_length(&self) -> f32 {
        self.sidestep_length
    }

    /// How many of the base layer's states that name this movement state
    /// had a part in the last update: the current one, one being blended
    /// into, and any the update left. The game's animator calls on the
    /// movement state once for each, so in a frame where the body goes
    /// from standing into a turn on the spot the standing state runs twice.
    pub fn calls(&self, movement: i32) -> usize {
        let (current, next) = self.animator.state(0);
        let playing = [Some(current), next].into_iter().flatten().filter_map(|s| s.movement.as_ref().map(|m| m.state));
        playing.chain(self.left.iter().copied()).filter(|named| *named == movement).count()
    }

    /// How far the animation moved the body in the last update, in the
    /// body's own axes (x right, y up, z ahead).
    pub fn motion(&self) -> [f32; 3] {
        self.animator.root_motion()
    }

    /// How far the animation turned the body in the last update, in
    /// degrees to the right. Only the turns on the spot do.
    pub fn turn(&self) -> f32 {
        self.animator.root_turn()
    }

    /// The root bone's own rotation in the pose.
    pub fn root_rotation(&self) -> [f32; 4] {
        self.animator.bone(0).1
    }

    /// The clips' `Mouse_look` curve: how far a vault or a climb has
    /// taken the view to its own limit, 0 to 1.
    pub fn mouse_look(&self) -> f32 {
        self.ids.mouse_look.map_or(0.0, |id| self.animator.get(id))
    }

    /// Whether the base layer is between two states.
    pub fn in_transition(&self) -> bool {
        self.animator.state(0).1.is_some()
    }

    /// The footstep curve: its sign flips at each foot plant.
    pub fn foot_step(&self) -> f32 {
        self.ids.foot_step.map_or(0.0, |id| self.animator.get(id))
    }

    /// The base layer's state, and the one it is blending into if any.
    pub fn state_names(&self) -> (&str, Option<&str>) {
        let (current, next) = self.animator.state(0);
        (current.name.as_str(), next.map(|s| s.name.as_str()))
    }

    /// How far the base layer's current state has played, in cycles, and
    /// how far its transition has run, 0 to 1.
    pub fn progress(&self) -> (f32, f32) {
        self.animator.progress(0)
    }

    /// How far the base layer's state that names a movement state has
    /// played, in cycles: the one being entered if that names it, else the
    /// current one.
    pub fn time_in(&self, movement: i32) -> Option<f32> {
        let (current, next) = self.animator.state(0);
        let names = |state: &crate::animator::StateDef| state.movement.as_ref().is_some_and(|m| m.state == movement);
        if next.is_some_and(names) {
            self.animator.next_progress(0)
        } else {
            names(current).then(|| self.animator.progress(0).0)
        }
    }

    /// The animator's layers as they stand, one line each, and the curves
    /// that decide who carries the weapon.
    pub fn describe(&self) -> Vec<String> {
        let curve = |id: Option<usize>| id.map_or(f32::NAN, |id| self.animator.get(id));
        let mut lines = self.animator.describe();
        lines.push(format!(
            "Weapon_3rd {:.3}, First_Person_Curve_Weight {:.3}, Hand_Right {:.3}, Aiming_Layer {:.3}",
            curve(self.ids.weapon_3rd),
            curve(self.ids.first_person),
            curve(self.ids.hand_right),
            curve(self.ids.aiming_layer)
        ));
        lines
    }

    /// Puts the base layer's current state at a point in its play, in
    /// cycles: a body that has stood a while is somewhere in its idle.
    pub fn set_progress(&mut self, cycles: f32) {
        self.animator.set_progress(0, cycles);
    }

    /// The rotation of the last spine bone in the body's axes.
    fn spine(&self) -> [f32; 4] {
        (0..SPINE).fold([0.0, 0.0, 0.0, 1.0], |acc, i| quat_mul(acc, self.animator.bone(i).1))
    }

    /// A bone's place and turning in the body's axes, down a chain of the
    /// bones followed: each turns and shifts what hangs from it.
    fn through(&self, chain: &[usize]) -> ([f32; 3], [f32; 4]) {
        chain.iter().fold(([0.0; 3], [0.0, 0.0, 0.0, 1.0]), |(at, turn), &i| {
            let (position, rotation) = self.animator.bone(i);
            let shifted = quat_rotate(turn, position);
            (std::array::from_fn(|k| at[k] + shifted[k]), quat_mul(turn, rotation))
        })
    }

    /// The further bones asked for at loading (`BodyData::load_with`), in
    /// the order asked: each one's place and turning in its parent, as the
    /// body's animation has it.
    pub fn drawn_bones(&self) -> Vec<([f32; 3], [f32; 4])> {
        // The root is the animator's own root bone, which has had the
        // clips' travel and turning over the ground taken off it; asked
        // for a second time it is a bone like any other and keeps them.
        let bone = |index: usize| match self.drawn_root {
            Some(root) if root == index => self.animator.bone(0),
            _ => self.animator.bone(BEND_GOALS + BEND_GOAL_NAMES.len() + index),
        };
        let mut bones: Vec<([f32; 3], [f32; 4])> = (0..self.drawn).map(bone).collect();
        // The free look's turn of the neck and the head, where it is more
        // than the game's least.
        let turn = self.head_turn;
        if turn.iter().map(|angle| angle * angle).sum::<f32>() >= HEAD_TURN_LEAST {
            for (at, share) in self.head_bones.iter().zip(self.head_shares) {
                if let Some(bone) = at.and_then(|at| bones.get_mut(at)) {
                    bone.1 = quat_mul(bone.1, crate::math::quat_euler(turn.map(|angle| angle * share)));
                }
            }
        }
        bones
    }

    /// Where the ribcage is, from the feet, in the body's axes. The
    /// first-person rig and camera hang there.
    pub fn ribcage(&self) -> [f32; 3] {
        self.through(&TO_RIBCAGE).0
    }

    /// The angle from the body round to the view, and the view's pitch,
    /// that the pose was last made for (the animator's `DeltaRotation` and
    /// `Aim_angle` as the last update had them).
    pub fn posed_for(&self) -> [f32; 2] {
        let value = |id: Option<usize>| id.map_or(0.0, |id| self.animator.get(id));
        [value(self.ids.delta_rotation), value(self.ids.aim_angle)]
    }

    /// Where the ribcage would be now had the pose been made for another
    /// angle to the view and another pitch, with everything else as it is.
    /// Costs one working-out of the whole pose.
    pub fn ribcage_posed_for(&self, delta_rotation: f32, aim_angle: f32) -> [f32; 3] {
        let (Some(delta), Some(aim)) = (self.ids.delta_rotation, self.ids.aim_angle) else { return self.ribcage() };
        let pose = self.animator.pose_with(&[(delta, delta_rotation), (aim, aim_angle)]);
        TO_RIBCAGE
            .iter()
            .fold(([0.0; 3], [0.0, 0.0, 0.0, 1.0]), |(at, turn), &i| {
                let (position, rotation) = pose[i];
                let shifted = quat_rotate(turn, position);
                (std::array::from_fn(|k| at[k] + shifted[k]), quat_mul(turn, rotation))
            })
            .0
    }

    /// One bone of an arm as the body's animation has it, from the feet in
    /// the body's axes: `joint` 0 is the collarbone, 1 the upper arm, 2 to
    /// 4 the three bones of the forearm and 5 the palm.
    pub fn arm_bone(&self, right: bool, joint: usize) -> ([f32; 3], [f32; 4]) {
        let mut chain = TO_RIBCAGE.to_vec();
        chain.extend_from_slice(&ARMS[usize::from(!right)][..=joint.min(5)]);
        self.through(&chain)
    }

    /// The body's own weapon bone, in the same axes.
    pub fn weapon_bone(&self) -> ([f32; 3], [f32; 4]) {
        self.through(&[0, 1, 2, 3, 4, WEAPON])
    }

    /// The body's own camera bone, which lends its rotation to the view.
    pub fn camera_bone(&self) -> [f32; 4] {
        self.animator.bone(CAMERA).1
    }

    /// Where the body holds the weapon now, if it holds it at all.
    ///
    /// `into_view` turns the body's axes into those of the frame the
    /// weapon's rig hangs in: at the ribcage, turned to where the player
    /// looks. `Movement::body_in_view` gives it.
    pub fn carried(&self, into_view: [f32; 4]) -> Option<Carried> {
        let curve = |id: Option<usize>| id.map_or(0.0, |id| self.animator.get(id));
        let authority = (curve(self.ids.weapon_3rd) * self.first_person_weight()).clamp(0.0, 1.0);
        if authority <= 0.0 {
            return None;
        }
        let spine = self.spine();
        let (weapon, ribcage) = (self.animator.bone(WEAPON), self.animator.bone(RIBCAGE));
        let offset: [f32; 3] = std::array::from_fn(|k| weapon.0[k] - ribcage.0[k]);
        Some(Carried {
            position: quat_rotate(into_view, quat_rotate(spine, offset)),
            rotation: quat_mul(into_view, quat_mul(spine, weapon.1)),
            authority,
        })
    }

    /// An arm as the body's animation has it, while that animation takes
    /// any of it from the weapon: the crawl takes both arms whole. `None`
    /// while the arm is the weapon's alone. The frame is that of `carried`.
    pub fn arm(&self, right: bool, into_view: [f32; 4]) -> Option<ArmPose> {
        let pose = self.arm_pose(right, into_view);
        (pose.hand > 0.0 || pose.shoulder > 0.0 || pose.elbow > 0.0).then_some(pose)
    }

    /// An arm as the body's animation has it, whatever share of it that
    /// animation claims. The arm's solver starts from this even when the
    /// arm is all the weapon's: the forearm and palm it finds are the
    /// body's (`arms::hold`). The frame is that of `carried`.
    pub fn arm_pose(&self, right: bool, into_view: [f32; 4]) -> ArmPose {
        let curve = |id: Option<usize>| id.map_or(0.0, |id| self.animator.get(id)).clamp(0.0, 1.0);
        let (hand, shoulder, elbow) = if right {
            (self.ids.hand_right, self.ids.shoulder_right, self.ids.elbow_right)
        } else {
            (self.ids.hand_left, self.ids.shoulder_left, self.ids.elbow_left)
        };
        let first_person = self.first_person_weight().clamp(0.0, 1.0);
        // The game puts the upper arm on the weapon rig's only for the
        // character's own eyes (`SetShoulders`): seen from outside the
        // shoulder is all the body's.
        let shoulder = match self.point_of_view {
            PointOfView::FirstPerson => curve(shoulder),
            PointOfView::ThirdPerson => 1.0,
        };
        let (hand, elbow) = (curve(hand) * first_person, curve(elbow) * first_person);
        let bones = ARMS[usize::from(!right)];
        // The upper arm from the ribcage, by way of the collarbone.
        let chain = [0, 1, 2, 3, 4, RIBCAGE, bones[0], bones[1]];
        let (upper, turn) = self.through(&chain);
        let ribcage = self.ribcage();
        let (goal, _) = self.through(&[0, 1, 2, 3, 4, BEND_GOALS + usize::from(!right)]);
        ArmPose {
            hand,
            shoulder,
            elbow,
            bend_goal: quat_rotate(into_view, std::array::from_fn(|k| goal[k] - ribcage[k])),
            upper: (
                quat_rotate(into_view, std::array::from_fn(|k| upper[k] - ribcage[k])),
                quat_mul(into_view, turn),
            ),
            below: std::array::from_fn(|i| self.animator.bone(bones[i + 2])),
            fingers: (!right).then(|| std::array::from_fn(|i| self.animator.bone(LEFT_FINGERS + i).1)),
        }
    }

    /// The body's right palm, while its animation has that hand to itself
    /// and the weapon has to go where the hand is. The frame is that of
    /// `carried`.
    pub fn held(&self, into_view: [f32; 4]) -> Option<Held> {
        let curve = |id: Option<usize>| id.map_or(0.0, |id| self.animator.get(id));
        let weight = 1.0 - (curve(self.ids.hand_right) * self.first_person_weight()).clamp(0.0, 1.0);
        if weight >= 1.0 {
            return None;
        }
        let (palm, turn) = self.through(&TO_RIGHT_PALM);
        let ribcage = self.ribcage();
        Some(Held {
            position: quat_rotate(into_view, std::array::from_fn(|k| palm[k] - ribcage[k])),
            rotation: quat_mul(into_view, turn),
            weight,
        })
    }
}
