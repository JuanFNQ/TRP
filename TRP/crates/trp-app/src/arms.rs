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
use trp_sim::arms::{self, Placed, RigArm};

use crate::animation::ClipPlayer;
use crate::controller::Controller;
use crate::convert;
use crate::hands_view::{in_root, to_game};
use crate::weapon::WeaponRig;

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
    /// The part's grip marker, with the finger pose it holds.
    grip: Option<(Entity, Vec<Quat>)>,
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
        let grip = (0..hierarchy.nodes.len())
            .filter(|&i| {
                hierarchy.nodes[i].grip.as_ref().is_some_and(|g| g.left_hand == (side == 'L') && g.kind != 1)
            })
            .max_by_key(|&i| (hierarchy.nodes[i].grip.as_ref().map(|g| g.kind == 2), depth(hierarchy, i)))
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
            grip,
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
    for arm in &rig.arms {
        let side = usize::from(!arm.right);
        // The arm as the body's animation has it, in the rig's frame, and
        // how much of it that animation claims.
        let body = shown.arms[side];
        let hand = body.hand.clamp(0.0, 1.0);

        // Everything is read before anything is turned.
        let gathered = {
            let at = |entity: Entity| in_root(entity, rig.root, |e| transforms.get(e).ok().copied(), &parents);
            (|| {
                let upper = transforms.get(arm.upper).ok()?;
                Some((
                    (upper.translation, upper.rotation),
                    at(arm.marker)?,
                    at(arm.hand_reference)?,
                    at(arm.bend_goal)?,
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
            upper_parent.compute_affine().inverse().transform_point3(solved[0].0),
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
        if let (Some((_, pose)), true) = (&arm.grip, clips.sampled_this_frame() && grip_weight > 0.0) {
            for (finger, held) in arm.fingers.iter().zip(pose.iter().skip(1)) {
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
