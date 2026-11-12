//! The body's bones, the arm pose and how the arms reach for it.

use super::*;

/// The bones followed, by their path below the animated object: the chain
/// from the root to the last spine bone, then what hangs from it.
pub(super) const BONES: [&str; 20] = [
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

pub(super) const SPINE: usize = 5;

pub(super) const WEAPON: usize = 5;

pub(super) const RIBCAGE: usize = 6;

pub(super) const CAMERA: usize = 7;

/// The bones from the root down to the ribcage, and on to the right palm.
pub(super) const TO_RIBCAGE: [usize; 6] = [0, 1, 2, 3, 4, RIBCAGE];

pub(super) const TO_RIGHT_PALM: [usize; 12] = [0, 1, 2, 3, 4, RIBCAGE, 8, 9, 10, 11, 12, 13];

/// Each arm's bones, right then left: the collarbone, the upper arm, the
/// three of the forearm and the palm.
pub(super) const ARMS: [[usize; 6]; 2] = [[8, 9, 10, 11, 12, 13], [14, 15, 16, 17, 18, 19]];

/// The left hand's finger bones follow the list above: fifteen, the thumb
/// first, each finger from the knuckle out.
pub(super) const LEFT_FINGERS: usize = BONES.len();

pub const FINGER_BONES: usize = 15;

/// After the fingers, the two points the body's animation would have the
/// elbows bend toward, right then left. They hang from the last spine bone.
pub(super) const BEND_GOALS: usize = LEFT_FINGERS + FINGER_BONES;

pub(super) const BEND_GOAL_NAMES: [&str; 2] = ["Bend_Goal_Right", "Bend_Goal_Left"];

/// Then the object the first-person camera hangs in (`Player.CameraContainer`,
/// whose child `Cam` is the camera): beside the ribcage on the last spine
/// bone, at the ribcage's own rest place, never animated. Where a clip
/// shifts the ribcage on its spine (blind fire's), the camera stays here
/// (blind-fire spec, "The weapon and the camera").
pub(super) const CAMERA_CONTAINER: usize = BEND_GOALS + BEND_GOAL_NAMES.len();

pub(super) const CAMERA_CONTAINER_NAME: &str = "CameraContainer";

/// The bones a host asked to have posed come after it.
pub(super) const DRAWN_FROM: usize = CAMERA_CONTAINER + 1;

/// The paths of a hand's finger bones below its palm, in that order. The
/// skeleton numbers the fingers 1 to 5 and each one's bones 1 to 3.
pub(super) fn finger_paths(palm: &str, side: char) -> Vec<String> {
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

/// How far the skeleton's limbs reach at rest, each as the sum of its
/// bones' lengths (each bone's place in its parent), so that no bone of the
/// limb, turned any way, lies farther from where it starts: the farther
/// leg from the pelvis to the foot (the thighs' two bones, the calf, the
/// foot); the neck and the head from the ribcage (the ribcage's and the
/// neck's places on the last spine bone, and the head's on the neck); the
/// farther arm from the ribcage to the palm (the collarbone, the upper arm,
/// the forearm's three, the palm). Read from the player prefab.
pub(super) fn limbs_reach(assets: &mut Assets, controller: &Controller) -> Result<[f32; 3]> {
    let length = |p: [f32; 3]| (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
    let mut legs = 0.0f32;
    for side in ['L', 'R'] {
        let mut path = BONES[1].to_owned();
        let paths: Vec<String> = ["Thigh1", "Thigh2", "Calf", "Foot"]
            .iter()
            .map(|name| {
                path.push_str(&format!("/Base Human{side}{name}"));
                path.clone()
            })
            .collect();
        let paths: Vec<&str> = paths.iter().map(String::as_str).collect();
        legs = legs.max(trp_data::character::body::bones(assets, &paths)?.iter().map(|bone| length(bone.position)).sum());
    }
    let neck = format!("{}/Base HumanNeck", BONES[4]);
    let head_path = format!("{neck}/Base HumanHead");
    let head: f32 = trp_data::character::body::bones(assets, &[&neck, &head_path])?.iter().map(|bone| length(bone.position)).sum::<f32>() + length(controller.bones[RIBCAGE].position);
    let arms = ARMS.iter().map(|arm| arm.iter().map(|&bone| length(controller.bones[bone].position)).sum::<f32>()).fold(0.0f32, f32::max);
    Ok([legs, head, arms])
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
/// lying down, vaulting and blind fire on the base layer, the layers that
/// shift the ribcage for blind fire and that bend the
/// body with the view, the layer of the hands' gestures on the world
/// and of the open inventory's pose (`docs/spec/interactions.md`), and
/// what only a character seen from outside plays: the body's own part in
/// a weapon drawn and put away, a grenade's pin and throw, a blade's
/// strokes, its reach for the vest in a reload, and its breathing
/// (`docs/spec/third-person.md`). The rest (other characters' states, a
/// stationary weapon's) are left out until the movement reaches them; the
/// animator never enters a state that was left out.
pub(super) fn wanted(state: &AnimState) -> bool {
    const READ: [&str; 43] = [
        // The left shoulder's layer (left-shoulder spec, "The body").
        "LS.",
        // Blind fire's four states, and the layer that shifts the ribcage
        // for it in one's own eyes (`docs/spec/blind-fire.md`, "The body").
        // (`FP_TurnTorsoOnDeltaRotation`, which no transition enters, is
        // left out.)
        "Base Layer.Stand.BlindFire_",
        "Additive_FP_Offsets.INACTIVE",
        "Additive_FP_Offsets.FP_BlindFire_RibcageOffset TOP",
        "Additive_FP_Offsets.FP_BlindFire_RibcageOffset R",
        // Seen from outside only.
        "Weapon.INACTIVE",
        "Weapon.INACTIVE_PRONE",
        "Weapon.idle_to_out",
        "Weapon.out_to_idle",
        // A grenade's pin, throws and pin put back, as the body plays them
        // (`FirstAction` 3 to 7, 9; grenades-and-knife spec, "The
        // grenade's hands").
        "Weapon.rgd5_",
        // A blade's strokes (grenades-and-knife spec, "The knife's strokes
        // seen from outside"): a knife's slash and stab (`FirstAction` 8 and
        // 12) and an axe's (13 and 14) from `INACTIVE`, the quick stroke's
        // (8, or an axe's 15) from `idle_to_out`. (Each machine's `look`
        // state, which no transition enters, is left out.)
        "Weapon.Knife_Anims_StateMachine.Fire_1_Knife",
        "Weapon.Knife_Anims_StateMachine.Alt_Fire_Knife",
        "Weapon.Knife_Anims_StateMachine.quick_Fire_Knife 0",
        "Weapon.Axe_Anims_StateMachine.Fire_1_Axe",
        "Weapon.Axe_Anims_StateMachine.Alt_Fire_Axe",
        "Weapon.Axe_Anims_StateMachine.quick_Fire_Axe 0",
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
        // Lying on a bipod (`ProneBipodsOn`): its clips hand the left arm
        // to the body (mounting spec, "The bipod").
        "Additive_Aiming.Prone_Bipods",
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
