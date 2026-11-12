//! Holding the weapon: each arm is solved onto the weapon as the game
//! solves it. See `docs/spec/hands-on-weapon.md`.
//!
//! The clips move two markers on the weapon, one per hand. A hand goes to
//! its marker; the left hand goes instead to the grip marker of the fitted
//! handguard while the clip keeps it on the weapon. The solver itself, and
//! the body's share in each arm, are the simulation's (`trp_sim::arms`):
//! this reads what the solver needs off the rig and writes its answer back.

use bevy::prelude::*;
use trp_data::unity::extract::Hierarchy;
use trp_sim::weapons::arms::{self, Placed, RigArm};

use crate::player::animation::ClipPlayer;
use crate::player::controller::Controller;
use crate::platform::convert;
use crate::player::hands_view::{in_root, to_game};
use crate::weapons::weapon::WeaponRig;

/// One arm's bones and what it reaches for.
pub struct ArmRig {
    /// The right arm, or the left.
    right: bool,
    upper: Entity,
    /// The forearm's three bones from the elbow down, then the palm.
    below: [Entity; 4],
    /// The marker the clips move, and the fixed reference beside it.
    marker: Entity,
    hand_reference: Entity,
    bend_goal: Entity,
    /// An underbarrel launcher's goal for the left elbow, which it bends
    /// toward while the launcher's bones are in use (`GatherUnderbarrelWeaponIK`:
    /// the launcher's `Bend_Goal_Left`), where the hand holds the launcher's grip.
    launcher_goal: Option<Entity>,
    /// The part's grip marker, with the finger pose it holds.
    grip: Option<(Entity, Vec<Quat>)>,
    /// For a grip whose finger pose is read as its bones stand
    /// (`trp_data::unity::extract::Grip::live`), those bones, in the order of
    /// the pose.
    live_grip: Option<Vec<Entity>>,
    /// The palm's bones below it, in the order grips list them.
    fingers: Vec<Entity>,
    /// The same bones by the skeleton's own numbering, the thumb first and
    /// each finger from the knuckle out, as the body's animation gives them.
    digits: Vec<Option<Entity>>,
}

fn descends_from(hierarchy: &Hierarchy, mut index: usize, ancestor: usize) -> bool {
    while let Some(parent) = hierarchy.nodes[index].parent {
        if parent == ancestor {
            return true;
        }
        index = parent;
    }
    false
}

fn depth(hierarchy: &Hierarchy, mut index: usize) -> usize {
    let mut depth = 0;
    while let Some(parent) = hierarchy.nodes[index].parent {
        depth += 1;
        index = parent;
    }
    depth
}

impl ArmRig {
    /// The hand's own bone.
    pub fn palm(&self) -> Entity {
        self.below[3]
    }

    /// The upper arm's bone.
    pub fn upper(&self) -> Entity {
        self.upper
    }

    /// The forearm's first bone, at the elbow.
    pub fn elbow(&self) -> Entity {
        self.below[0]
    }

    /// Finds one arm in an assembled weapon. `side` is `L` or `R`.
    pub fn find(hierarchy: &Hierarchy, entities: &[Entity], side: char) -> Option<Self> {
        let node = |path: &str| (0..hierarchy.nodes.len()).find(|&i| hierarchy.path(i) == path);
        let upper_path = format!("Base Human{side}Collarbone/Base Human{side}Upperarm");
        let upper = node(&upper_path)?;
        // The forearm's three bones hang one from the other, and the palm
        // from the last.
        let mut path = upper_path;
        let mut below = Vec::new();
        for name in ["Forearm1", "Forearm2", "Forearm3", "Palm"] {
            path.push_str(&format!("/Base Human{side}{name}"));
            below.push(node(&path)?);
        }
        let palm = below[3];
        let on_weapon = |name: &str| node(&format!("Weapon_root/Weapon_root_anim/weapon/{name}"));
        let marker = on_weapon(&format!("weapon_{side}_IK_marker"))?;
        let hand_reference = on_weapon(&format!("weapon_{side}_hand_marker"))?;
        let bend_goal = on_weapon(if side == 'L' { "Bend_Goal_Left" } else { "Bend_Goal_Right" })?;

        // The game takes the ordinary grip for this hand that sits deepest
        // in the weapon, with an underbarrel weapon's grip ahead of others.
        let grip_node = (0..hierarchy.nodes.len())
            .filter(|&i| {
                hierarchy.nodes[i].grip.as_ref().is_some_and(|g| g.left_hand == (side == 'L') && g.kind != 1)
            })
            .max_by_key(|&i| (hierarchy.nodes[i].grip.as_ref().map(|g| g.kind == 2), depth(hierarchy, i)));
        // An underbarrel weapon's grip hangs from its root, below which its
        // own goal for the left elbow is (`launcherRoot.FindTransform`).
        let launcher_goal = grip_node
            .filter(|&g| side == 'L' && hierarchy.nodes[g].grip.as_ref().is_some_and(|grip| grip.kind == 2))
            .and_then(|g| hierarchy.nodes[g].parent)
            .and_then(|root| (root + 1..hierarchy.nodes.len()).find(|&i| hierarchy.nodes[i].name == "Bend_Goal_Left" && descends_from(hierarchy, i, root)))
            .map(|i| entities[i]);
        let live_grip = grip_node.filter(|&g| hierarchy.nodes[g].grip.as_ref().is_some_and(|grip| grip.live)).map(|g| {
            (g + 1..hierarchy.nodes.len()).filter(|&i| descends_from(hierarchy, i, g)).map(|i| entities[i]).collect()
        });
        let grip = grip_node
            .map(|g| {
                // An uncached grip keeps its finger pose as bones below it.
                let stored = &hierarchy.nodes[g].grip.as_ref().unwrap().fingers;
                let pose: Vec<Quat> = if stored.is_empty() {
                    std::iter::once(g)
                        .chain((g + 1..hierarchy.nodes.len()).filter(|&i| descends_from(hierarchy, i, g)))
                        .map(|i| convert::rotation(hierarchy.nodes[i].rotation))
                        .collect()
                } else {
                    stored.iter().map(|q| convert::rotation(*q)).collect()
                };
                (entities[g], pose)
            });
        let fingers =
            (palm + 1..hierarchy.nodes.len()).filter(|&i| descends_from(hierarchy, i, palm)).map(|i| entities[i]).collect();
        let mut digits = Vec::new();
        for finger in 1..=5 {
            let mut path = hierarchy.path(palm);
            for joint in 1..=3 {
                path.push_str(&format!("/Base Human{side}Digit{finger}{joint}"));
                digits.push(node(&path).map(|i| entities[i]));
            }
        }
        Some(Self {
            right: side == 'R',
            upper: entities[upper],
            below: std::array::from_fn(|i| entities[below[i]]),
            marker: entities[marker],
            hand_reference: entities[hand_reference],
            bend_goal: entities[bend_goal],
            launcher_goal,
            grip,
            live_grip,
            fingers,
            digits,
        })
    }
}

/// Each of the rig's upper arms in its parent as the weapon's clip has it,
/// and as this last left it, right arm then left. A clip that does not
/// place the bone leaves last frame's figures standing; this tells the two
/// apart.
#[derive(Default)]
pub struct Shoulders {
    clip: [(Vec3, Quat); 2],
    written: [Option<(Vec3, Quat)>; 2],
}

fn in_game(t: Transform) -> Placed {
    (to_game(t.translation), convert::rotation_back(t.rotation))
}

pub fn place_hands(
    rig: Option<Res<WeaponRig>>,
    clips: Res<ClipPlayer>,
    controller: Res<Controller>,
    parents: Query<&ChildOf>,
    mut transforms: Query<&mut Transform>,
    mut shoulders: Local<Shoulders>,
) {
    let Some(rig) = rig else { return };
    let Some(limbs) = &rig.limbs else { return };
    let shown = controller.shown();
    // In first person on a mount the arms are drawn where the ribcage's
    // squeeze puts them, not the rig's (`hands_view::squeeze_shift`).
    let shift = transforms.get(rig.root).map_or(Vec3::ZERO, |root| crate::player::hands_view::squeeze_shift(root, shown.rig_apart));
    for arm in &rig.arms {
        let side = usize::from(!arm.right);
        // The arm as the body's animation has it, in the rig's frame, and
        // how much of it that animation claims.
        let mut body = shown.arms[side];
        // Seen from outside, the arm is solved from the body's own shoulder
        // (`SetShoulders` is first person's), which stays on the body when a
        // mount moves the rig onto its point: its place in the moved rig is
        // taken back by the move, and so is its elbow's goal. The arm then
        // reaches for the weapon from the body, its lengths kept: a hand out
        // of reach stops short on the handguard. (Carried with the rig, the
        // shoulders left the collarbones and the sleeves stretched.)
        if controller.movement.body.point_of_view() == trp_sim::PointOfView::ThirdPerson {
            let apart = shown.rig_apart;
            body.upper.0 = std::array::from_fn(|k| body.upper.0[k] - apart[k]);
            body.bend_goal = std::array::from_fn(|k| body.bend_goal[k] - apart[k]);
        }
        let hand = body.hand.clamp(0.0, 1.0);

        // Everything is read before anything is turned. The left elbow's
        // goal is an underbarrel launcher's while its bones are in use.
        let bend_goal = arm.launcher_goal.filter(|_| controller.launcher_bones).unwrap_or(arm.bend_goal);
        let gathered = {
            let at = |entity: Entity| in_root(entity, rig.root, |e| transforms.get(e).ok().copied(), &parents);
            (|| {
                let upper = transforms.get(arm.upper).ok()?;
                Some((
                    (upper.translation, upper.rotation),
                    at(arm.marker)?,
                    at(arm.hand_reference)?,
                    at(bend_goal)?,
                    arm.grip.as_ref().and_then(|(entity, _)| at(*entity)),
                    at(parents.get(arm.upper).ok()?.parent())?,
                ))
            })()
        };
        let Some((upper_local, marker, reference, goal, grip, upper_parent)) = gathered else { continue };

        // The rig's own upper arm, as the clip has it: the body's is put
        // on it by what the body's animation leaves of the shoulder.
        if shoulders.written[side] != Some(upper_local) {
            shoulders.clip[side] = upper_local;
        }
        let (clip_at, clip_turn) = shoulders.clip[side];
        let rig_upper = upper_parent * Transform { translation: clip_at, rotation: clip_turn, scale: Vec3::ONE };
        let on_rig = RigArm {
            upper: in_game(rig_upper),
            marker: in_game(marker),
            reference: to_game(reference.translation),
            grip: grip.map(in_game),
            // An obstacle that pushes the weapon back draws the elbows aside.
            goal: std::array::from_fn(|k| to_game(goal.translation)[k] + shown.elbows[side][k]),
        };
        let (bones, grip_weight) = arms::hold(&body, &on_rig, &limbs[side]);
        let solved = bones.map(|(at, turn)| (convert::position(at), convert::rotation(turn)));

        // The upper arm in its parent; each bone below it turned in the
        // one above.
        let placed = (
            upper_parent.compute_affine().inverse().transform_point3(solved[0].0 + shift),
            upper_parent.rotation.inverse() * solved[0].1,
        );
        if let Ok(mut upper) = transforms.get_mut(arm.upper) {
            (upper.translation, upper.rotation) = placed;
        }
        shoulders.written[side] = Some(placed);
        for (index, bone) in arm.below.iter().enumerate() {
            if let Ok(mut transform) = transforms.get_mut(*bone) {
                transform.rotation = solved[index].1.inverse() * solved[index + 1].1;
            }
        }

        // Fingers close on the part as the hand comes onto it. The clip's
        // pose is the start, so this needs a fresh one.
        // A grip read as its bones stand gives the pose they have now.
        if let (Some((_, pose)), true) = (&arm.grip, clips.sampled_this_frame() && grip_weight > 0.0) {
            let live: Option<Vec<Quat>> = arm.live_grip.as_ref().map(|bones| bones.iter().map(|bone| transforms.get(*bone).map_or(Quat::IDENTITY, |t| t.rotation)).collect());
            let held_pose = live.as_deref().unwrap_or(pose.get(1..).unwrap_or(&[]));
            for (finger, held) in arm.fingers.iter().zip(held_pose) {
                if let Ok(mut transform) = transforms.get_mut(*finger) {
                    transform.rotation = transform.rotation.lerp(*held, grip_weight);
                }
            }
        }
        // And open again as the body's animation takes the hand: its
        // fingers go with it, for the hand it gives fingers for.
        if let Some(theirs) = body.fingers {
            if hand >= 1.0 || (hand > 0.0 && clips.sampled_this_frame()) {
                for (digit, theirs) in arm.digits.iter().zip(theirs) {
                    if let Some(mut transform) = digit.and_then(|digit| transforms.get_mut(digit).ok()) {
                        transform.rotation = transform.rotation.slerp(convert::rotation(theirs), hand);
                    }
                }
            }
        }
    }
}
