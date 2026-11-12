//! A grenade's blast on the character.
//! After the grenade spec, "The blast"; `trp_sim::blast`: where the body's hit colliders stand as
//! its skeleton is posed, what of the range a line from the blast meets, and health told of the
//! damage and the contusion, and the hands' springs of the shake (`Hands::grenade_shake`). Not
//! here: the blast's effect, its sound (`blast_sound`), the fragments' flight (`bullets`), the
//! dummies' (`dummy::blast_dummies`). The character's own armour takes its share after the damage,
//! as a bot's does, its wear kept on its things, a face shield's glass marked past 0.4 of its
//! template durability (`shield_marks`).

use std::sync::Arc;

use bevy::prelude::*;
use trp_sim::condition::armor::{ArmorRules, VestSkills, Wearer};
use trp_sim::combat::blast::{blast, Explosive, HitPoint, Line};
use trp_sim::condition::health::{DamageKind, Part};

use crate::player::body_view::{BodyParts, BodyRig};
use crate::player::controller::Controller;
use crate::platform::convert;
use crate::world::loose::Blasted;
use crate::world::range::Range;
use crate::weapons::weapon::WeaponRig;

/// A grenade's figures (its item's, and its fragment round's): its blast's,
/// its flash's (`Blindness`), its fragments' count and round, and how far
/// its blast finds characters (`Explosion`'s overlap: the largest of twice
/// the greatest distance, the blindness's and the contusion's reaches).
pub struct Grenade {
    pub explosive: Explosive,
    pub blindness: [f32; 3],
    pub fragments: i32,
    pub round: String,
    pub reach: f32,
}

fn three(value: Option<&serde_json::Value>) -> [f32; 3] {
    let axis = |name: &str| value.and_then(|value| value.get(name)).and_then(serde_json::Value::as_f64).unwrap_or(0.0) as f32;
    [axis("x"), axis("y"), axis("z")]
}

pub fn grenade_of(db: &trp_data::Database, template: &str) -> Option<Grenade> {
    let item = db.items.get(template)?;
    let number = |name: &str| item.prop_f32(name).unwrap_or(0.0);
    let round = item.props.get("FragmentType").and_then(serde_json::Value::as_str).unwrap_or("").to_owned();
    let power = db.items.get(&round).and_then(|r| r.prop_f32("PenetrationPower")).unwrap_or(0.0);
    let explosive = Explosive {
        max: number("MaxExplosionDistance"),
        min: number("MinExplosionDistance"),
        // A grenade's `Strength`, an explosive round's `ExplosionStrength`
        // (`ExplosiveAmmoComponent.GetStrength`).
        strength: item.prop_f32("Strength").or_else(|| item.prop_f32("ExplosionStrength")).unwrap_or(0.0),
        contusion: three(item.props.get("Contusion")),
        armor: three(item.props.get("ArmorDistanceDistanceDamage")),
        power,
    };
    let blindness = three(item.props.get("Blindness"));
    let reach = (explosive.max * 2.0).max(blindness[1]).max(explosive.contusion[1]);
    Some(Grenade { explosive, blindness, fragments: number("FragmentsCount") as i32, round, reach })
}

/// What the character's own armour takes of a blast needs (`Player.
/// ApplyExplosionDamageToArmor`): the player prefab's hit colliders, each
/// with its collider type and plate (`trp_data::ragdoll::HitColliderData`),
/// and the armour's figures. Read as the run begins with the grenades.
#[derive(Resource)]
pub struct PlayerArmour {
    pub colliders: Vec<trp_data::character::ragdoll::HitColliderData>,
    pub rules: Arc<ArmorRules>,
}

/// The wall's absorption of a contusion (`WallContusionAbsorption`:
/// thickness taking it all, thickness taking none).
pub fn wall_absorption(db: &trp_data::Database) -> [f32; 2] {
    let [thick, thin, _] = three(db.config.get("WallContusionAbsorption"));
    [thick, thin]
}

/// The body's hit colliders as the skeleton stands (`PlayerBones.
/// BodyPartColliders`): each its part and its object's place in the game's
/// axes, from the player prefab's (`death::Ragdoll`) on the animated bones,
/// an arm's on the bone it is drawn on (the weapon rig's), each with its
/// collider type by its object where the armour's colliders are read; then
/// each plate whose plate is in (`plates_on`: the game's plate colliders'
/// objects are on so, `SetArmorPlateCollidersState`), on its bone. None
/// without a body drawn.
fn hit_points(ragdoll: &crate::player::death::Ragdoll, rig: Option<&BodyRig>, parts: Option<&BodyParts>, weapon: Option<&WeaponRig>, bones: &Query<&GlobalTransform>, armour: Option<&PlayerArmour>, plates_on: u16) -> Vec<HitPoint> {
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
            // Its collider type (`BodyPartColliderType`), which the armour's
            // share goes by.
            let kind = armour.and_then(|armour| armour.colliders.iter().find(|c| c.plate == 0 && c.object == collider.object)).map_or(u8::MAX, |c| c.collider_type);
            points.push(HitPoint { part: *part, at: [at.x, at.y, -at.z], collider: kind, plate: 0 });
        }
    }
    for c in armour.iter().flat_map(|armour| &armour.colliders).filter(|c| c.plate != 0 && plates_on & c.plate == c.plate) {
        let Some(part) = Part::ALL.get(c.part as usize) else { continue };
        let Some(bone) = rig.bone(parts, &c.bone).and_then(|entity| bones.get(entity).ok()) else { continue };
        let local = c.local.map_or(Vec3::ZERO, |(place, _)| convert::position(place));
        let at = bone.transform_point(local);
        points.push(HitPoint { part: *part, at: [at.x, at.y, -at.z], collider: c.collider_type, plate: c.plate });
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
    ragdoll: Res<crate::player::death::Ragdoll>,
    rig: Option<Res<BodyRig>>,
    parts: Option<Res<BodyParts>>,
    weapon: Option<Res<WeaponRig>>,
    bones: Query<&GlobalTransform>,
    mut hands: Option<ResMut<crate::weapons::weapon::HandsRig>>,
    mut contused: MessageWriter<crate::audio::tinnitus::Contused>,
    camera: Query<&GlobalTransform, With<crate::MainCamera>>,
    mut carried: ResMut<crate::screens::inventory_view::Carried>,
    mut burned: MessageWriter<crate::effects::contusion_view::BurnedEyes>,
    mut received: MessageWriter<crate::effects::contusion_view::Received>,
    (frames, armour, mut shield_marks): (Res<bevy::diagnostic::FrameCount>, Option<Res<PlayerArmour>>, Option<ResMut<crate::world::shield_marks::ShieldMarks>>),
) {
    for blasted in blasts.read().filter(|blasted| blasted.blasts) {
        let Some(grenade) = grenade_of(&game.db, &blasted.template) else { continue };
        let explosive = grenade.explosive;
        // The character's own armour as it stands (`GetPutOnArmorsNonAlloc`),
        // each piece at its thing's wear, and the plates it has in.
        let mut wearer = armour.as_deref().map(|armour| crate::world::dummy::wearer_of(&game.db, armour.rules.clone(), &carried.equipment.item, VestSkills::default()));
        let plates_on = wearer.as_ref().map_or(0, Wearer::plates_on);
        let points = hit_points(&ragdoll, rig.as_deref(), parts.as_deref(), weapon.as_deref(), &bones, armour.as_deref(), plates_on);
        let at = blasted.blast_point();
        // From where the character stood before this frame's move: the game
        // measures a blast so (recorded: `courses8`'s `zarya-behind-player`,
        // its contusion's length by the frame before's place to a
        // millionth of a second), its blast going off before the player's
        // move of the frame. An explosive round's blast goes off in the
        // world's tick after the players' (`DoWorldTick`): from where it now
        // stands (read, not recorded).
        let feet = if blasted.shot { controller.movement.position } else { controller.feet_before_move };
        // The range's boxes stop every fragment (`Range::entered`).
        let line = |from: [f32; 3], to: [f32; 3]| range.entered(from, to).map_or(Line::Through(0.0), Line::Stopped);
        let outcome = blast(&explosive, at, feet, &points, &line, wall_absorption(&game.db));
        // What a flash measures its facing by (`ApplyLightAndSoundHealthEffects`):
        // the way from the head's bone (`PlayerBones.Head`) as the skeleton
        // last stood, and the movement's own look (`MovementContext.
        // LookDirection`: yaw to the right, pitch down), not the camera's
        // (recorded: `courses3`'s `1-flash`, both blasts' facing the game's
        // to six figures so). Without a body drawn, the camera's place
        // stands for the head's.
        let head_bone = rig.as_deref().zip(parts.as_deref()).and_then(|(rig, parts)| rig.bone(parts, "Base HumanHead")).and_then(|entity| bones.get(entity).ok()).map(GlobalTransform::translation);
        let look = {
            let m = &controller.movement;
            let (yaw, pitch) = (m.yaw.to_radians(), m.pitch.to_radians());
            [yaw.sin() * pitch.cos(), -pitch.sin(), yaw.cos() * pitch.cos()]
        };
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
        let found_alive = health.alive;
        for (part, damage) in &outcome.damage {
            // `ApplyEnvironmentalDamage` → `ApplyDamageInfo`: health's, and
            // the player told of it (`ReceiveDamage`), while alive.
            if health.alive {
                health.apply_damage(*part, *damage, DamageKind::GrenadeFragment);
                received.write(crate::effects::contusion_view::Received { part: *part, damage: *damage, absorbed: 0.0, kind: DamageKind::GrenadeFragment, burn: 0.0 });
            }
        }
        // Then its armour's share (`ApplyExplosionDamageToArmor`), the
        // character found alive as the blast came (killed by it, still):
        // each piece's wear written back to its thing, a face shield's
        // glass past 0.4 of its template durability marked by the thrower,
        // the player (`shield_marks`).
        if let Some(wearer) = wearer.as_mut().filter(|_| found_alive && !outcome.armor.is_empty()) {
            let taken = wearer.take_blast(&outcome.armor);
            // (A repair buff the wear took it under gone with it.)
            crate::world::dummy::keep_wear(wearer, &mut carried.equipment.item);
            let marked = wearer.drain();
            info!(
                "the character's armour took {:?} of the blast{}",
                taken.iter().map(|(number, damage)| format!("{}: {damage:.2}", wearer.pieces.iter().find(|piece| piece.id == *number).and_then(|piece| game.db.items.get(&piece.template)).map_or("?", |t| t.name.as_str()))).collect::<Vec<_>>(),
                if marked.is_empty() { String::new() } else { format!(", {} face shield marked", marked.len()) }
            );
            if let Some(marks) = shield_marks.as_mut() {
                marks.own.extend(marked);
            }
        }
        if let Some(time) = outcome.contusion {
            health.do_contusion(time, 1.0);
            // The ears told (`EffectAddedEvent` → `TryStartContusion`).
            contused.write(crate::audio::tinnitus::Contused { time });
        }
        // A flash (`ApplyLightAndSoundHealthEffects`; grenade spec, "A
        // flash"): its distance the feet's (above), its facing from the
        // head's bone along the movement's look, the line to the head clear
        // of the range.
        let blindness = grenade.blindness;
        if blindness[1] > 0.0 && health.alive {
            let Some(eye) = head_bone.or_else(|| camera.single().ok().map(GlobalTransform::translation)) else { continue };
            let head = [eye.x, eye.y, -eye.z];
            let blocked = range.entered(at, head).is_some();
            let struck = (!blocked).then(|| trp_sim::combat::blast::flash(blindness, far, at, head, look)).flatten();
            info!("a flash {far:.2} m off on frame {}: {}", frames.0, match struck {
                Some((strength, share)) => format!("strength {strength:.2}, its burn's share {share:.2}"),
                None if blocked => "its light stopped by the range".to_owned(),
                None => "nothing (not faced, or too far)".to_owned(),
            });
            if let Some((strength, share)) = struck {
                let cover = trp_sim::items::inventory::blindness_duration(&carried.equipment);
                health.do_stun(blindness[2], strength, cover);
                // The camera told (`BurnEyesEvent` → `EyeBurn.Burn`).
                if let Some(share) = health.do_burn_eyes(share, strength, blindness[2], cover) {
                    burned.write(crate::effects::contusion_view::BurnedEyes { at, strength: share });
                }
            }
        }
    }
}
