//! Puts the weapon-motion simulation's results on the rig: the weapon
//! bone and the camera.

use bevy::prelude::*;

use crate::controller::Controller;
use crate::weapon::{Eye, HandsRig, WeaponRig};
use crate::{convert, MainCamera, View};

/// Transform of `entity` relative to `root`, from the local transforms as
/// they stand this frame.
pub fn in_root(
    entity: Entity,
    root: Entity,
    local: impl Fn(Entity) -> Option<Transform>,
    parents: &Query<&ChildOf>,
) -> Option<Transform> {
    let mut result = Transform::IDENTITY;
    let mut current = entity;
    while current != root {
        result = local(current)? * result;
        current = parents.get(current).ok()?.parent();
    }
    Some(result)
}

/// A point or direction from Bevy's axes back into the game's.
pub(crate) fn to_game(v: Vec3) -> [f32; 3] {
    [v.x, v.y, -v.z]
}

/// Draws the weapon's moved bone and the camera where the simulation's
/// last frames put them (`Controller::shown`): the simulation has already
/// worked out the procedural motion, the body's carrying of the weapon and
/// the camera's pose on frames of its own.
#[allow(clippy::too_many_arguments)]
pub fn apply_hands(
    view: Res<View>,
    controller: Res<Controller>,
    rig: Option<Res<WeaponRig>>,
    hands: Option<Res<HandsRig>>,
    down: Res<crate::feet_down::FeetDown>,
    eye: Query<Entity, With<Eye>>,
    globals: Query<&GlobalTransform>,
    mut transforms: Query<&mut Transform, Without<MainCamera>>,
    mut camera: Query<(&mut Transform, &mut Projection), With<MainCamera>>,
) {
    let (Some(rig), Some(hands)) = (rig, hands) else { return };
    let shown = controller.shown();

    // The weapon: offset and swing of the bone below the clip-driven one.
    let (position, rotation) = shown.weapon;
    if let Some(mut transform) = rig.weapon_root_anim.and_then(|e| transforms.get_mut(e).ok()) {
        *transform = Transform {
            translation: convert::position(position),
            rotation: convert::rotation(rotation),
            scale: Vec3::ONE,
        };
    }
    // Seen from outside, the rig (the weapon, the arms that hold it and
    // the shoulders hung on it) comes down with a body drawn down onto the
    // ground (`feet_down`); for the character's own eyes it stays where the
    // game has it, and so does the eye either way.
    let lowered = match (globals.get(rig.pivot).ok(), *view != View::FirstPerson && down.by != 0.0) {
        (Some(pivot), true) => pivot.affine().inverse().transform_vector3(Vec3::NEG_Y * down.by),
        _ => Vec3::ZERO,
    };

    // At a wide field of view the game squeezes the weapon and arms along
    // the view while they are drawn, so that they fill about the same part
    // of the screen. The camera keeps its place in the unsqueezed frame.
    // The squeeze is applied once: a recorded trace shows the weapon bone
    // scaled by exactly this factor along the view at rest.
    // It is the first-person picture's alone: the game puts it on as that
    // camera's frame is drawn and takes it off again, so a view from
    // outside sees the weapon and arms at their true size.
    let Ok(mut root) = transforms.get_mut(rig.root) else { return };
    let squeeze = if *view == View::FirstPerson { hands.hands.fov_scale() } else { 1.0 };
    root.scale = Vec3::new(1.0, 1.0, squeeze);
    root.translation = lowered;
    let root = *root;
    // The character's own eyes are where the first-person camera would be,
    // whatever camera the picture is drawn from: from the rig's own origin,
    // the pivot's, however the rig is drawn.
    let pose = shown.camera;
    let seeing = Transform {
        translation: root.rotation * convert::position(pose.position),
        rotation: root.rotation * convert::rotation(pose.rotation),
        scale: Vec3::ONE,
    };
    if let Some(mut eye) = eye.single().ok().and_then(|eye| transforms.get_mut(eye).ok()) {
        *eye = seeing;
    }
    if *view != View::FirstPerson {
        return;
    }
    let Ok((mut camera, mut projection)) = camera.single_mut() else { return };
    camera.translation = seeing.translation;
    camera.rotation = seeing.rotation;
    if let Projection::Perspective(perspective) = &mut *projection {
        perspective.fov = pose.field_of_view.to_radians();
    }
}
