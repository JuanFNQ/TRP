//! The options' `GROUNDED FEET` (the user, 2026-10-16: "Can i see the float
//! fix first?"; foot IK tried in 0.27.0 and 0.27.1 and taken back in
//! 0.28.0: "lets revert back to the simple height fix. The ik stuff doesn't
//! work while sprinting, and when legs spread too much the start
//! twitching/bending backwards"). A departure from Tarkov's look, the
//! drawing alone.
//!
//! Tarkov's character counts as on the ground while ground is within its
//! ground check below it (7 to 15 cm by the movement's state), and on the
//! flat nothing pulls it down then; after a short drop it can come to rest
//! that far over the ground, and the game draws its body there, feet and all
//! (`docs/findings.md`, 2026-10-16). Here the body is drawn down by that
//! gap, eased in and out by a spring as long as the game's lag in the body's
//! height (0.066 s: `ESTIMATE`, the app's choice). Nothing of the
//! simulation or the eye moves. The shoulders and the ribcage the body is
//! drawn with hang from the weapon rig (`body_view::see_collarbones`), so
//! for the character's own eyes the drop stops at the waist (the first spine
//! bone kept where the animation has it, the first-person arms untouched),
//! and seen from outside the whole rig comes down with the body
//! (`hands_view`).

use bevy::prelude::*;
use trp_sim::World;

use crate::body_view::BodyParts;
use crate::controller::Controller;
use crate::convert;
use crate::range::Range;

/// How far the body is drawn down this frame, and what it keeps between
/// frames.
#[derive(Resource, Default)]
pub struct FeetDown {
    pub by: f32,
    speed: f32,
    /// The pelvis's and the first spine bone's places in `BodyParts::bones`,
    /// and each bone's parent there; looked for once.
    bones: Option<Option<([usize; 2], Vec<Option<usize>>)>>,
    /// The first spine bone in its parent as it is to be drawn this frame,
    /// for the character's own eyes (`body_view::pose_body`).
    pub spine: Option<(usize, Transform)>,
}

/// The spring's time, the game's for the lag in the body's height.
const FEET_DOWN_TIME: f32 = 0.066;

/// Works out how far the body is drawn down this frame: the gap between
/// the character's underside and the ground that holds it, while it is on
/// the ground and no further over it than its ground check reaches. Resting
/// on an edge's corner the ground under it is far below, and nothing is
/// done.
pub fn ground_feet(
    time: Res<Time>,
    view: Res<crate::View>,
    controller: Res<Controller>,
    range: Res<Range>,
    parts: Res<BodyParts>,
    features: Res<crate::options_view::Features>,
    mut down: ResMut<FeetDown>,
) {
    let down = &mut *down;
    down.spine = None;
    if !features.grounded_feet {
        (down.by, down.speed) = (0.0, 0.0);
        return;
    }
    let m = &controller.movement;
    let over = m.position[1] + m.capsule().underside - range.footing(m.position);
    let target = if m.grounded && over > 0.0 && over <= m.ground_check { over } else { 0.0 };
    down.by = crate::inventory_view::smooth_damp(down.by, target, &mut down.speed, FEET_DOWN_TIME, time.delta_secs());
    if (down.by - target).abs() < 1e-5 && down.speed.abs() < 1e-4 {
        (down.by, down.speed) = (target, 0.0);
    }
    if *view != crate::View::FirstPerson || down.by == 0.0 {
        return;
    }
    // For the character's own eyes: the first spine bone put back up by the
    // drop, in the pelvis's axes.
    let found = down.bones.get_or_insert_with(|| {
        let paths = &parts.bones;
        let at = |end: &str| paths.iter().position(|path| path.ends_with(end));
        let parents = paths.iter().map(|path| path.rsplit_once('/').and_then(|(parent, _)| paths.iter().position(|other| other == parent))).collect();
        Some(([at("/Base HumanPelvis")?, at("/Base HumanPelvis/Base HumanSpine1")?], parents))
    });
    let (Some(([pelvis, spine], parents)), Some(pose)) = (found.as_ref(), controller.body_shown()) else { return };
    if parents.len() != pose.bones.len() {
        return;
    }
    let local = |bone: usize| convert::transform(pose.bones[bone].0, pose.bones[bone].1, [1.0; 3]);
    let mut turn = local(*pelvis).rotation;
    let mut up = parents[*pelvis];
    while let Some(parent) = up {
        turn = local(parent).rotation * turn;
        up = parents[parent];
    }
    let mut kept = local(*spine);
    kept.translation += (pose.turn * turn).inverse() * (Vec3::Y * down.by);
    down.spine = Some((*spine, kept));
}
