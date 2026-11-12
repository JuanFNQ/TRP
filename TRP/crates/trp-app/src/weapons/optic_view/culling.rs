//! The skinned pieces the engine never culls, left out of the scope's view where it cannot see them.
//!
//! A skinned piece is drawn in every view (`NoFrustumCulling`: its bounds are not tracked as it
//! moves), so the scope's camera drew the arms, the character's own clothes and every dummy's
//! pieces wherever they stood: 39 of its 97 meshes standing at the range with the G36's scope,
//! 16 of 74 with this (counted 2026-11-11). The game culls a skinned renderer by its bounds for each camera. Here a
//! piece stays in the scope's list while a box round its joints, `REACH` wider each way, is within
//! the scope's view; the main camera's list is left as it was.

use super::*;
use bevy::camera::primitives::{Aabb, Frustum};
use bevy::camera::visibility::{NoFrustumCulling, VisibleEntities};
use bevy::math::Affine3A;
use bevy::mesh::skinning::SkinnedMesh;

/// How much wider than its joints' box a skinned piece may reach, each way, in metres.
/// `ESTIMATE`: taken as more than any piece of a character reaches beyond its joints; not
/// measured on the meshes, whose vertices leave the app's world once on the graphics card
/// (`render_data`). The scoped captures were the same pixel for pixel with it (the G36's
/// scope, the SPEAR's at two zooms, 2026-11-11).
const REACH: f32 = 1.0;

/// The scope's list without the never-culled skinned pieces whose joints' box misses its view.
/// After the engine's visibility, before it marks what no view saw (as `lod_view::pick_levels`).
#[allow(clippy::type_complexity)]
pub(super) fn cull_unseen(
    mut optic: Query<(&Camera, &Frustum, &mut VisibleEntities), With<OpticCamera>>,
    skinned: Query<&SkinnedMesh, With<NoFrustumCulling>>,
    joints: Query<&GlobalTransform>,
    mut boxes: Local<HashMap<(Entity, Entity, usize), Option<Aabb>>>,
) {
    let Ok((camera, frustum, mut seen)) = optic.single_mut() else { return };
    if !camera.is_active {
        return;
    }
    // One box a skeleton a frame: a dummy's pieces share their joints.
    boxes.clear();
    seen.get_mut(std::any::TypeId::of::<Mesh3d>()).retain(|&entity| {
        let Ok(skin) = skinned.get(entity) else { return true };
        let (Some(&first), Some(&last)) = (skin.joints.first(), skin.joints.last()) else { return true };
        let bound = *boxes.entry((first, last, skin.joints.len())).or_insert_with(|| joints_box(&skin.joints, &joints));
        // A piece whose joints are not all placed is kept.
        bound.is_none_or(|bound| frustum.intersects_obb(&bound, &Affine3A::IDENTITY, true, false))
    });
}

/// The box round a piece's joints as they stand, `REACH` wider each way.
fn joints_box(list: &[Entity], joints: &Query<&GlobalTransform>) -> Option<Aabb> {
    let (mut low, mut high) = (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY));
    for &joint in list {
        let at = joints.get(joint).ok()?.translation();
        (low, high) = (low.min(at), high.max(at));
    }
    Some(Aabb::from_min_max(low - Vec3::splat(REACH), high + Vec3::splat(REACH)))
}

/// With `RUST_LOG=info,trp_app::weapons::optic_view=debug` (or the crate's own target), every
/// 300 frames, each active view's meshes: all, and the never-culled among them.
#[allow(clippy::type_complexity)]
pub(super) fn count_drawn(cameras: Query<(&Camera, &VisibleEntities, Has<OpticCamera>), Or<(With<MainCamera>, With<OpticCamera>)>>, never: Query<(), With<NoFrustumCulling>>, mut frame: Local<u32>) {
    if !bevy::log::tracing::enabled!(bevy::log::Level::DEBUG) {
        return;
    }
    *frame += 1;
    if *frame % 300 != 0 {
        return;
    }
    for (_, seen, optic) in cameras.iter().filter(|(camera, ..)| camera.is_active) {
        let list = seen.get(std::any::TypeId::of::<Mesh3d>());
        let kept = list.iter().filter(|&&entity| never.contains(entity)).count();
        debug!("{} camera draws {} meshes, {kept} of them never culled", if optic { "the optic" } else { "the main" }, list.len());
    }
}
