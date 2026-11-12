//! The weapon's root, length, line ends, rest clips and right-hand marker.

use super::*;

/// The bone the weapon's clips move, which the bone the procedural motion
/// moves hangs from.
const WEAPON_ROOT: &str = "Weapon_root";

/// Where that bone is and how it is turned in the frame the weapon's rig
/// hangs in, at rest.
pub(super) fn weapon_root(weapon: &Weapon) -> Result<([f32; 3], [f32; 4])> {
    let rig = base::rig::WeaponRig::new(&weapon.hierarchy);
    let bone = rig.bone(WEAPON_ROOT).context("the weapon has no root bone")?;
    Ok(rig.placed(bone))
}

/// The length of the line the game casts along the weapon to feel for
/// obstacles ("Turn-away" in the weapon-handling spec): from the bone the
/// procedural motion moves to the muzzle, or to the farthest muzzle
/// effect if one lies farther. Of the effects only those deepest in the
/// weapon's tree count, and one named for the extractor's smoke. Measured
/// on the rig at rest; the game measures with the weapon in its resting
/// clip.
pub fn weapon_length(weapon: &Weapon) -> Result<f32> {
    let rig = base::rig::WeaponRig::new(&weapon.hierarchy);
    weapon_length_on(&rig, &weapon_line_ends(weapon)?).context("the weapon has no moved bone")
}

/// The same length on a rig as its clips have posed it: the game takes it
/// again when the weapon appears in the hands and at the end of every
/// reload, in whatever pose the clip then has the weapon. `ends` are the
/// weapon's points it is measured to (`weapon_line_ends`).
pub fn weapon_length_on(rig: &base::rig::WeaponRig, ends: &[usize]) -> Option<f32> {
    let from = rig.placed(rig.moved_bone()?).0;
    let reach = |node: usize| {
        let at = rig.placed(node).0;
        (0..3).map(|k| (at[k] - from[k]).powi(2)).sum::<f32>().sqrt()
    };
    ends.iter().map(|&node| reach(node)).reduce(f32::max)
}

/// The length as the game comes by it (weapon-handling spec, "Turn-
/// away", "Measured squeezed"): the greatest distance from the moved
/// bone to an end as the world has them while the wide field of view's
/// squeeze is on the rig, divided by how much the muzzle's own axis is
/// squeezed. Exact only for a weapon lying along the view. `squeeze` is
/// the rig's scale along the view (`Hands::fov_scale`); the muzzle is the
/// last of `ends`.
pub fn weapon_length_squeezed(rig: &base::rig::WeaponRig, ends: &[usize], squeeze: f32) -> Option<f32> {
    let from = rig.placed(rig.moved_bone()?).0;
    let reach = |node: usize| {
        let at = rig.placed(node).0;
        let across = (at[0] - from[0]).powi(2) + (at[1] - from[1]).powi(2);
        (across + ((at[2] - from[2]) * squeeze).powi(2)).sqrt()
    };
    let length = ends.iter().map(|&node| reach(node)).reduce(f32::max)?;
    // The engine's word for a bone's scale under a squeeze it is turned
    // against: how far its own axis, squeezed, still reaches along itself.
    let axis = base::math::quat_rotate(rig.placed(*ends.last()?).1, [0.0, 1.0, 0.0]);
    let scale = axis[0] * axis[0] + axis[1] * axis[1] + squeeze * axis[2] * axis[2];
    Some(if scale < 1.0 { length / scale } else { length })
}

/// The points of a weapon its length is measured to: the muzzle, and the
/// muzzle effects that count.
pub fn weapon_line_ends(weapon: &Weapon) -> Result<Vec<usize>> {
    let hierarchy = &weapon.hierarchy;
    let muzzle = (0..hierarchy.nodes.len())
        .find(|&i| hierarchy.nodes[i].name == "fireport")
        .context("the weapon has no muzzle")?;
    // The muzzle effects that count (`UpdateJetsAndFumes`'s choice).
    Ok(trp_data::weapons::muzzle::effect_objects(hierarchy).into_iter().chain([muzzle]).collect())
}

/// The clip the weapon's animator plays at rest: the one its resting
/// state's blend tree gives for a weapon with nothing under the barrel.
///
/// That state chooses between two clips by a parameter (`LauncherID`) the
/// game only sets on a weapon that has an underbarrel launcher. Left at
/// the value the controller starts it with, the tree plays the second of
/// them, whose name speaks of a launcher; recorded, the rifle's left-hand
/// marker is that clip's to a tenth of a millimetre and 2.6 cm from the
/// other's.
pub fn rest_clip(assets: &Assets, weapon: &Weapon) -> Result<(String, trp_data::unity::ObjRef)> {
    let table = StateTable::load(assets, weapon)?;
    table.clip_by_default(REST_STATE).cloned().ok_or_else(|| anyhow!("the weapon's animator has no state {REST_STATE}"))
}

/// The second clip of a resting state that blends two by the view's pitch,
/// with the pitch at which the first and the second each play alone. The
/// empty hands rest so (`Aim_angle`, which the game sets from the view's
/// pitch every frame); a weapon whose resting state goes by anything else
/// has none.
pub fn rest_blend(assets: &Assets, weapon: &Weapon) -> Result<Option<(String, trp_data::unity::ObjRef, [f32; 2])>> {
    let table = StateTable::load(assets, weapon)?;
    let Some(state) = table.find(REST_STATE) else { return Ok(None) };
    let (first, _) = table.clip_by_default(REST_STATE).ok_or_else(|| anyhow!("state {REST_STATE} has no clip"))?;
    if state.blend_parameter.as_deref() != Some("Aim_angle") || state.clips.len() != 2 || state.thresholds.len() != 2 {
        return Ok(None);
    }
    let at = |name: &str| state.clips.iter().position(|(clip, _)| clip == name);
    let (Some(first), Some(other)) = (at(first), state.clips.iter().position(|(clip, _)| clip != first)) else { return Ok(None) };
    let (name, object) = state.clips[other].clone();
    Ok(Some((name, object, [state.thresholds[first], state.thresholds[other]])))
}

/// The name of the clip one of the weapon animator's states plays with its
/// parameters left alone, if the animator has the state: for a host that
/// plays a state's clip by name (`Hands.OUT TO IDLE` to draw the weapon,
/// `Hands.IDLE TO OUT` to put it away).
pub fn state_clip(assets: &Assets, weapon: &Weapon, state: &str) -> Result<Option<String>> {
    Ok(StateTable::load(assets, weapon)?.clip_by_default(state).map(|(name, _)| name.clone()))
}

/// The weapon's marker for the right hand, in the axes of the bone the
/// procedural motion moves, as the weapon's resting clip has it: where the
/// weapon is held when the body's hand takes it (`Hands::weapon_transform_carried`).
/// A host that plays the weapon's clips reads the marker off its rig instead.
pub fn right_hand_marker(assets: &Assets, weapon: &Weapon) -> Result<([f32; 3], [f32; 4])> {
    use trp_data::unity::anim::{euler_to_quat, path_hash, Clip, Property};
    const BONE: &str = "Weapon_root/Weapon_root_anim/weapon";
    let hierarchy = &weapon.hierarchy;
    let idle = rest_clip(assets, weapon).ok().map(|(_, clip)| Clip::read(assets, clip)).transpose()?;
    let mut values = Vec::new();
    if let Some(clip) = &idle {
        clip.sample(0.0, &mut values);
    }
    // A bone in its parent: at rest, with whatever the clip moves replaced.
    let local = |path: &str| -> Result<([f32; 3], [f32; 4])> {
        let node = (0..hierarchy.nodes.len())
            .find(|&i| hierarchy.path(i) == path)
            .map(|i| &hierarchy.nodes[i])
            .with_context(|| format!("the weapon has no {path}"))?;
        let (mut position, mut rotation) = (node.translation, node.rotation);
        let hash = path_hash(path);
        for track in idle.iter().flat_map(|c| &c.tracks).filter(|t| t.path == hash) {
            let v = &values[track.curve..];
            match track.property {
                Property::Position => position = [v[0], v[1], v[2]],
                Property::Rotation => rotation = [v[0], v[1], v[2], v[3]],
                Property::Euler => rotation = euler_to_quat([v[0], v[1], v[2]]),
                Property::Scale => {}
            }
        }
        Ok((position, rotation))
    };
    let (bone_position, bone_rotation) = local(BONE)?;
    let (marker_position, marker_rotation) = local(&format!("{BONE}/weapon_R_IK_marker"))?;
    let reach = base::math::quat_rotate(bone_rotation, marker_position);
    Ok((std::array::from_fn(|i| bone_position[i] + reach[i]), base::math::quat_mul(bone_rotation, marker_rotation)))
}

/// The nearer of the weapon prefab's two aim planes: its ordinary one,
/// unless the far plane is nearer still.
pub(super) fn aim_plane_depth(weapon: &Weapon) -> Result<f32> {
    let depth = |name: &str| -> Result<f32> {
        weapon.prefab.req(name)?.req("Depth")?.as_f32().with_context(|| format!("{name} has no depth"))
    };
    Ok(depth("DefaultAimPlane")?.min(depth("FarPlane")?))
}
