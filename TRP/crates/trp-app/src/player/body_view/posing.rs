//! Collarbones seen and kept, the outfit seen and posed, the upper body shown.

use super::*;

/// Works out where each kept collarbone goes: where the animated one is,
/// seen from the weapon rig's root with its squeeze left out, so that the
/// squeeze draws it in as the game's does. And the drawn ribcage: on that
/// root for the character's own eyes, and for another's where the animated
/// ribcage is, seen the same way.
///
/// For the character's own eyes on a mount, the root is moved off the
/// ribcage onto the mount's point (`Shown::rig_apart`), and the arms go
/// with it whole: recorded, the game's upper arms stay as far from the
/// weapon as off a mount while they leave the ribcage by up to half a
/// metre (`courses7\10-anything`). So the collarbones are seen from where
/// the root would be without the mount's move, and carried with it; seen
/// from the moved root they stayed on the body and the sleeves between
/// them and the upper arms stretched.
pub fn see_collarbones(
    rig: Option<Res<BodyRig>>,
    controller: Res<Controller>,
    helper: TransformHelper,
    mut kept: ResMut<KeptCollarbones>,
) {
    kept.collarbones.clear();
    kept.ribcage = Transform::IDENTITY;
    let Some(rig) = rig else { return };
    let Ok(ribcage) = helper.compute_global_transform(rig.ribcage) else { return };
    let (_, turn, mut at) = ribcage.to_scale_rotation_translation();
    let own_eyes = controller.movement.body.point_of_view() == trp_sim::PointOfView::FirstPerson;
    if own_eyes {
        at -= turn * crate::platform::convert::position(controller.shown().rig_apart);
    }
    let seen = |entity: Entity| -> Option<Transform> {
        let (_, bone_turn, bone_at) = helper.compute_global_transform(entity).ok()?.to_scale_rotation_translation();
        Some(Transform { translation: turn.inverse() * (bone_at - at), rotation: turn.inverse() * bone_turn, scale: Vec3::ONE })
    };
    for (animated, _) in &rig.collarbones {
        let Some(place) = seen(*animated) else { return };
        kept.collarbones.push(place);
    }
    if controller.movement.body.point_of_view() == trp_sim::PointOfView::ThirdPerson {
        if let Some(place) = rig.ribcage_animated.and_then(seen) {
            kept.ribcage = place;
        }
    }
}

/// Puts each kept collarbone there, and the drawn ribcage. In first person
/// on a mount the collarbones go with the arms where the ribcage's squeeze
/// puts them (`hands_view::squeeze_shift`).
pub fn keep_collarbones(rig: Option<Res<BodyRig>>, kept: Res<KeptCollarbones>, weapon: Option<Res<WeaponRig>>, controller: Res<Controller>, mut transforms: Query<&mut Transform>) {
    let Some(rig) = rig else { return };
    let shift = weapon.and_then(|weapon| transforms.get(weapon.root).ok().map(|root| crate::player::hands_view::squeeze_shift(root, controller.shown().rig_apart))).unwrap_or(Vec3::ZERO);
    for ((_, entity), place) in rig.collarbones.iter().zip(&kept.collarbones) {
        if let Ok(mut transform) = transforms.get_mut(*entity) {
            *transform = *place;
            transform.translation += shift;
        }
    }
    if let Ok(mut transform) = transforms.get_mut(rig.ribcage_drawn) {
        *transform = kept.ribcage;
    }
}

/// Works out where the bones of the outfit that is on go: the install's
/// bones as they stand this frame, each as the move from rest of what is
/// on it, are given to the outfit's rig, which answers with its own
/// (`trp_data::outfit::Rig::pose`: the hips, the hands and the feet as the
/// install's, everything between hung and turned and bent at the outfit's
/// own lengths).
///
/// What the game squeezes along the view in first person (the arms, with
/// the weapon) is taken out of that squeeze first and put back into it
/// after: the rig's lengths are true lengths.
pub fn see_outfit(
    on: Res<OutfitOn>,
    skeletons: Option<Res<OutfitSkeletons>>,
    weapon: Option<Res<WeaponRig>>,
    helper: TransformHelper,
    mut pose: ResMut<OutfitPose>,
) {
    use bevy::math::Affine3A;
    use trp_data::character::outfit::Move;
    pose.0.clear();
    let (Some(item), Some(skeletons), Some(weapon)) = (&on.item, skeletons, weapon) else { return };
    let Ok(root) = helper.compute_global_transform(weapon.root) else { return };
    let (_, turn, at) = root.to_scale_rotation_translation();
    let rigid = Affine3A::from_rotation_translation(turn, at);
    let unsqueezed = rigid * root.affine().inverse();
    // The body's, and the arms' for first person where it has those.
    for skeleton in skeletons.0.iter().filter(|skeleton| &skeleton.item == item) {
        let mut theirs = Vec::with_capacity(skeleton.theirs.len());
        for ((entity, rest), squeezed) in skeleton.theirs.iter().zip(&skeleton.their_rest).zip(&skeleton.squeezed) {
            let Ok(now) = helper.compute_global_transform(*entity) else { return };
            let moved = if *squeezed { unsqueezed * now.affine() * *rest } else { now.affine() * *rest };
            theirs.push(Move { axes: [moved.matrix3.x_axis.to_array(), moved.matrix3.y_axis.to_array(), moved.matrix3.z_axis.to_array()], shift: moved.translation.to_array() });
        }
        for (entity, moved) in skeleton.ours.iter().zip(skeleton.rig.pose(&theirs)) {
            let turn = Quat::from_mat3(&Mat3::from_cols(moved.axes[0].into(), moved.axes[1].into(), moved.axes[2].into())).normalize();
            pose.0.push((*entity, Transform { translation: moved.shift.into(), rotation: turn, scale: Vec3::ONE }));
        }
        // And the root's own place and turn, undone, for the squeezed
        // bones.
        pose.0.push((skeleton.unsqueeze, Transform { translation: turn.inverse() * -at, rotation: turn.inverse(), scale: Vec3::ONE }));
    }
}

/// Puts the outfit's bones there.
pub fn pose_outfit(pose: Res<OutfitPose>, mut transforms: Query<&mut Transform>) {
    for (entity, place) in &pose.0 {
        if let Ok(mut transform) = transforms.get_mut(*entity) {
            *transform = *place;
        }
    }
}

/// Shows the upper body from outside and leaves it a shadow in first
/// person; and the other way about for the hands as the player sees them,
/// which the game switches off from outside (`UpdatePlayerRenders`: the
/// hands' renderers are on in first person only, and the upper body's own
/// sleeves and hands are what an outside view has). Without a body drawn
/// there is nothing to take their place, and they stay.
///
/// With the options' `FIRST-PERSON BODY` off, none of it is drawn for the
/// character's own eyes: neither the legs, nor the upper body's shadow,
/// nor either kind of arms (settings spec, "Features").
pub fn show_upper_body(
    view: Res<View>,
    rig: Option<Res<BodyRig>>,
    outfit: Res<OutfitOn>,
    features: Res<crate::screens::options_view::Features>,
    mut pieces: Query<(&mut RenderLayers, Option<&mut crate::rendering::shadow_only::ShadowOnly>), (Or<(With<UpperBody>, With<LowerBody>)>, EvenParked)>,
    mut hands: Query<&mut Visibility, With<FirstPersonHands>>,
    mut outfit_arms: Query<(&OutfitArms, &mut Visibility), Without<FirstPersonHands>>,
) {
    let unseen = !features.first_person_body && *view == View::FirstPerson;
    // An outfit's own arms for first person: there, while it is on, and
    // while the options have them (`OutfitOn::arms`).
    // (`TRP_OUTFIT_ARMS=always` shows them in every view and leaves the
    // upper body a shadow in every view, to look at them from outside:
    // with `--pov first --view side`, the rig as the character's own eyes
    // have it, seen from beside.)
    let always = std::env::var("TRP_OUTFIT_ARMS").is_ok_and(|value| value == "always") && outfit.arms;
    for (arms, mut visibility) in &mut outfit_arms {
        let on = (always || *view == View::FirstPerson) && outfit.arms && outfit.item.as_deref() == Some(arms.item.as_str());
        let shown = if on && !unseen { Visibility::Inherited } else { Visibility::Hidden };
        if *visibility != shown {
            *visibility = shown;
        }
    }
    // The upper body a shadow alone for these eyes; all of it nowhere with
    // the switch off.
    let shadow_only = *view == View::FirstPerson || always;
    for (mut layers, upper) in &mut pieces {
        let layer = if unseen { NOWHERE } else { SEEN };
        if *layers != RenderLayers::layer(layer) {
            *layers = RenderLayers::layer(layer);
        }
        if let Some(mut upper) = upper {
            let wish = crate::rendering::shadow_only::ShadowOnly(shadow_only && !unseen);
            if *upper != wish {
                *upper = wish;
            }
        }
    }
    // The install's own arms: there unless the outfit that is on has arms
    // of its own for these eyes. An outfit without them leaves these.
    // (`TRP_OUTFIT_ARMS=both` draws them as well as the outfit's, one
    // through the other, to see where the outfit's skin lies inside the
    // install's and where outside it.)
    let both = std::env::var("TRP_OUTFIT_ARMS").is_ok_and(|value| value == "both");
    let no_upper_body = pieces.iter().all(|(_, upper)| upper.is_none());
    let shown = if !unseen && (!outfit.arms || both) && (*view == View::FirstPerson || rig.is_none() || no_upper_body) {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for mut visibility in &mut hands {
        if *visibility != shown {
            *visibility = shown;
        }
    }
}

/// Puts the skeleton where the body's animation has it, drawn down onto
/// the ground with the options' `GROUNDED FEET` (`feet_down`).
pub fn pose_body(controller: Res<Controller>, rig: Option<Res<BodyRig>>, down: Res<crate::player::feet_down::FeetDown>, mut transforms: Query<&mut Transform>) {
    let Some(rig) = rig else { return };
    let Some(pose) = controller.body_shown() else { return };
    if let Ok(mut root) = transforms.get_mut(rig.root) {
        *root = Transform { translation: pose.at - Vec3::Y * down.by, rotation: pose.turn, scale: Vec3::ONE };
    }
    for (entity, (position, rotation)) in rig.bones.iter().zip(&pose.bones) {
        if let Ok(mut transform) = transforms.get_mut(*entity) {
            *transform = convert::transform(*position, *rotation, [1.0; 3]);
        }
    }
    if let Some((bone, kept)) = down.spine {
        if let Some(mut transform) = rig.bones.get(bone).and_then(|entity| transforms.get_mut(*entity).ok()) {
            *transform = kept;
        }
    }
}
