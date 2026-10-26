//! A knife's blade swept for what it meets (`docs/spec/grenades-and-knife.md`,
//! "The knife's hands"): while a stroke's blade is live, every frame, from
//! the blade's box (`KnifeCollider`'s own `BoxCollider`, as the rig draws it)
//! along the player's look by the stroke's reach (`KnifeCollider.
//! ManualUpdateClient`); the first thing met is hit, once a stroke, and the
//! stroke glances off (`KnifeHands::hit`).
//!
//! `PROVISIONAL`: the game casts the blade's box (its width and depth, a
//! quarter of its length thick); here a line down its middle is cast. What
//! the blade meets on the range is the range's boxes and ground; there is
//! nothing on it to hurt, and no sound of the blade meeting a surface yet.

use bevy::math::Affine3A;
use bevy::prelude::*;
use bevy::transform::helper::TransformHelper;
use trp_sim::World;

use crate::controller::Controller;
use crate::range::Range;
use crate::weapon::{HandsRig, WeaponRig};

/// The blade's box on a melee weapon's rig: the name of the object it is on
/// and its centre and size there, in the game's axes.
#[derive(Debug, Clone)]
pub struct Blade {
    pub node: String,
    pub centre: [f32; 3],
    pub size: [f32; 3],
}

impl Blade {
    /// The object on a rig with the knife's collider script, and its box.
    pub fn find(hierarchy: &trp_data::unity::extract::Hierarchy) -> Option<Self> {
        hierarchy.nodes.iter().find(|node| node.scripts.iter().any(|script| script == "KnifeCollider")).and_then(|node| {
            node.colliders.iter().find_map(|collider| match collider.shape {
                trp_data::unity::extract::ColliderShape::Box { centre, size } => Some(Self { node: node.name.clone(), centre, size }),
                _ => None,
            })
        })
    }
}

/// The sweep, once the rig is posed for the frame.
#[allow(clippy::too_many_arguments)]
pub fn sweep(
    hands: Option<ResMut<HandsRig>>,
    rig: Option<Res<WeaponRig>>,
    controller: Res<Controller>,
    range: Res<Range>,
    names: Query<&Name>,
    children: Query<&Children>,
    helper: TransformHelper,
    skills: Option<Res<crate::skills::Skills>>,
) {
    let (Some(mut hands), Some(rig)) = (hands, rig) else { return };
    let hands = &mut *hands;
    let (Some(knife), Some(blade)) = (hands.knife.as_mut(), hands.blade.as_ref()) else { return };
    if !knife.live() {
        return;
    }
    // The blade's object under the weapon's root, as the game draws it
    // with the first person's squeeze taken off (`loose::set_going`).
    let Some(from) = rig.weapon_root else { return };
    let mut stack = vec![from];
    let mut found = None;
    while let Some(entity) = stack.pop() {
        if names.get(entity).is_ok_and(|name| name.as_str() == blade.node) {
            found = Some(entity);
            break;
        }
        stack.extend(children.get(entity).into_iter().flatten());
    }
    let (Some(entity), Ok(root), ) = (found, helper.compute_global_transform(rig.root)) else { return };
    let Ok(now) = helper.compute_global_transform(entity) else { return };
    let (_, turn, at) = root.to_scale_rotation_translation();
    let placed = Affine3A::from_rotation_translation(turn, at) * root.affine().inverse() * now.affine();
    let centre = placed.transform_point3(crate::convert::position(blade.centre));
    let centre = [centre.x, centre.y, -centre.z];
    // The player's look, in the game's axes (yaw to the right, pitch down).
    let m = &controller.movement;
    let (yaw, pitch) = (m.yaw.to_radians(), m.pitch.to_radians());
    let look = [yaw.sin() * pitch.cos(), -pitch.sin(), yaw.cos() * pitch.cos()];
    // From a blade's length behind the box, along the look by the reach.
    let reach = match knife.stroke() {
        trp_sim::Stroke::Slash => knife.settings().slash_reach,
        trp_sim::Stroke::Stab => knife.settings().stab_reach,
    };
    let start: [f32; 3] = std::array::from_fn(|k| centre[k] - look[k] * blade.size[1]);
    let end: [f32; 3] = std::array::from_fn(|k| start[k] + look[k] * reach);
    if let Some(distance) = range.line(start, end) {
        let surface = range.surface(start, end);
        // A blow's damage, times one and Strength's melee buff
        // (`StrengthBuffMeleePowerInc`).
        let power = 1.0 + skills.as_ref().map_or(0.0, |skills| skills.buffs().strength_melee);
        if let Some(damage) = knife.hit().map(|damage| damage * power) {
            info!("the knife's {:?} met the range {distance:.2} m along its cast (surface {surface:?}): {damage} of damage, glancing off", knife.stroke());
        }
    }
}
