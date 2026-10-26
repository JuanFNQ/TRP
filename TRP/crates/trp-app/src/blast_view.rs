//! A grenade's blast on the character (grenade spec, "The blast";
//! `trp_sim::blast`): where the body's hit colliders stand as its skeleton
//! is posed, what of the range a line from the blast meets, and health told
//! of the damage and the contusion, and the hands' springs of the shake
//! (`Hands::grenade_shake`). Not here: the blast's effect, its sound
//! (`blast_sound`), the fragments' flight, armour's own damage.

use bevy::prelude::*;
use trp_sim::blast::{blast, blast_point, Explosive, HitPoint};
use trp_sim::health::{DamageKind, Part};

use crate::body_view::{BodyParts, BodyRig};
use crate::controller::Controller;
use crate::convert;
use crate::loose::Blasted;
use crate::range::Range;
use crate::weapon::WeaponRig;

/// The body's hit colliders as the skeleton stands (`PlayerBones.
/// BodyPartColliders`): each its part and its object's place in the game's
/// axes, from the player prefab's (`death::Ragdoll`) on the animated bones,
/// an arm's on the bone it is drawn on (the weapon rig's). None without a
/// body drawn.
fn hit_points(ragdoll: &crate::death::Ragdoll, rig: Option<&BodyRig>, parts: Option<&BodyParts>, weapon: Option<&WeaponRig>, bones: &Query<&GlobalTransform>) -> Vec<HitPoint> {
    let (Some(data), Some(rig), Some(parts)) = (ragdoll.data.as_ref(), rig, parts) else { return Vec::new() };
    let mut points = Vec::new();
    for body in &data.bodies {
        let arm = || {
            let path = parts.parts.iter().flat_map(|part| &part.bone_paths).find(|path| path.ends_with(&format!("/{}", body.bone)))?;
            rig.drawn(parts, weapon?, path)
        };
        let Some(bone) = rig.bone(parts, &body.bone).or_else(arm).and_then(|entity| bones.get(entity).ok()) else { continue };
        for collider in &body.colliders {
            let Some(part) = collider.part.and_then(|part| Part::ALL.get(part as usize)) else { continue };
            let local = collider.local.map_or(Vec3::ZERO, |(place, _)| convert::position(place));
            let at = bone.transform_point(local);
            points.push(HitPoint { part: *part, at: [at.x, at.y, -at.z] });
        }
    }
    points
}

/// Each grenade's blast told of this frame (`loose::Blasted`): what it does
/// to the character, its damage given to health as `GrenadeFragment`, part
/// by part, and its contusion (`Health::do_contusion`, at a strength of 1).
#[allow(clippy::too_many_arguments)]
pub fn grenade_blasts(
    mut blasts: MessageReader<Blasted>,
    mut controller: ResMut<Controller>,
    game: Res<crate::GameData>,
    range: Res<Range>,
    ragdoll: Res<crate::death::Ragdoll>,
    rig: Option<Res<BodyRig>>,
    parts: Option<Res<BodyParts>>,
    weapon: Option<Res<WeaponRig>>,
    bones: Query<&GlobalTransform>,
    mut hands: Option<ResMut<crate::weapon::HandsRig>>,
    mut contused: MessageWriter<crate::tinnitus::Contused>,
    camera: Query<&GlobalTransform, With<crate::MainCamera>>,
    carried: Res<crate::inventory_view::Carried>,
    frames: Res<bevy::diagnostic::FrameCount>,
    mut burned: MessageWriter<crate::contusion_view::BurnedEyes>,
    mut received: MessageWriter<crate::contusion_view::Received>,
) {
    for blasted in blasts.read() {
        let Some(item) = game.db.items.get(&blasted.template) else { continue };
        let number = |name: &str| item.prop_f32(name).unwrap_or(0.0);
        let three = |value: Option<&serde_json::Value>| -> [f32; 3] {
            let axis = |name: &str| value.and_then(|value| value.get(name)).and_then(serde_json::Value::as_f64).unwrap_or(0.0) as f32;
            [axis("x"), axis("y"), axis("z")]
        };
        let explosive = Explosive { max: number("MaxExplosionDistance"), min: number("MinExplosionDistance"), strength: number("Strength"), contusion: three(item.props.get("Contusion")) };
        let [thick, thin, _] = three(game.db.config.get("WallContusionAbsorption"));
        let points = hit_points(&ragdoll, rig.as_deref(), parts.as_deref(), weapon.as_deref(), &bones);
        let at = blast_point(blasted.at);
        let feet = controller.movement.position;
        let outcome = blast(&explosive, at, feet, &points, &|from, to| range.entered(from, to), [thick, thin]);
        let far = ((feet[0] - at[0]).powi(2) + (feet[1] - at[1]).powi(2) + (feet[2] - at[2]).powi(2)).sqrt();
        info!(
            "a blast {far:.2} m off, on the body's {} hit colliders: damage {:?}, contusion {:?}, shake {:.2}",
            points.len(),
            outcome.damage.iter().map(|(part, damage)| format!("{} {damage:.1}", part.name())).collect::<Vec<_>>(),
            outcome.contusion,
            outcome.shake
        );
        // The view shaken (`ForceEffector.GrenadeShake_CO`), the
        // character's own, alive.
        if outcome.shake > 0.0 && controller.health.as_ref().is_none_or(|health| health.alive) {
            if let Some(hands) = hands.as_mut() {
                hands.hands.grenade_shake(outcome.shake);
            }
        }
        let Some(health) = controller.health.as_mut() else { continue };
        for (part, damage) in &outcome.damage {
            // `ApplyEnvironmentalDamage` → `ApplyDamageInfo`: health's, and
            // the player told of it (`ReceiveDamage`), while alive.
            if health.alive {
                health.apply_damage(*part, *damage, DamageKind::GrenadeFragment);
                received.write(crate::contusion_view::Received { part: *part, damage: *damage, absorbed: 0.0, kind: DamageKind::GrenadeFragment, burn: 0.0 });
            }
        }
        if let Some(time) = outcome.contusion {
            health.do_contusion(time, 1.0);
            // The ears told (`EffectAddedEvent` → `TryStartContusion`).
            contused.write(crate::tinnitus::Contused { time });
        }
        // A flash (`ApplyLightAndSoundHealthEffects`; grenade spec, "A
        // flash"): from the camera's place as the head's (`PROVISIONAL`) and
        // its way as the look, the line to it clear of the range.
        let blindness = three(item.props.get("Blindness"));
        if blindness[1] > 0.0 && health.alive {
            let Ok(camera) = camera.single() else { continue };
            let eye = camera.translation();
            let head = [eye.x, eye.y, -eye.z];
            let forward = camera.forward();
            let look = [forward.x, forward.y, -forward.z];
            let blocked = range.entered(at, head).is_some();
            let struck = (!blocked).then(|| trp_sim::blast::flash(blindness, far, at, head, look)).flatten();
            info!("a flash {far:.2} m off on frame {}: {}", frames.0, match struck {
                Some((strength, share)) => format!("strength {strength:.2}, its burn's share {share:.2}"),
                None if blocked => "its light stopped by the range".to_owned(),
                None => "nothing (not faced, or too far)".to_owned(),
            });
            if let Some((strength, share)) = struck {
                let cover = trp_sim::inventory::blindness_duration(&carried.equipment);
                health.do_stun(blindness[2], strength, cover);
                // The camera told (`BurnEyesEvent` → `EyeBurn.Burn`).
                if let Some(share) = health.do_burn_eyes(share, strength, blindness[2], cover) {
                    burned.write(crate::contusion_view::BurnedEyes { at, strength: share });
                }
            }
        }
    }
}
