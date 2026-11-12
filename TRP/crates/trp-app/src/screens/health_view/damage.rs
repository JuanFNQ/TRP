//! Damage keys, armour slows, the death screen and the barbed wire.

use super::*;

/// The app's commands for trying the health out, the debug panel's: HURT THE NEXT PART does a
/// bullet's damage (`HURT_DAMAGE`, with the game's own chances of a
/// bleeding and a fracture) to the next part in turn, the limbs first
/// (`HURT_ORDER`), and DIE kills as a bullet through the head does. The
/// line under the inventory's slots says what was hurt.
pub(super) fn damage_keys(
    options: Res<crate::Options>,
    mut controller: ResMut<Controller>,
    mut carried: ResMut<crate::screens::inventory_view::Carried>,
    mut next: Local<usize>,
    mut received: MessageWriter<crate::effects::contusion_view::Received>,
    mut asked: MessageReader<crate::platform::debug_menu::DebugCommand>,
) {
    use crate::platform::debug_menu::DebugCommand;
    // The debug panel's two buttons (`debug_menu`; their keys `F7` and `F8`
    // went: they are commands of the debug menu alone).
    let asked: Vec<DebugCommand> = asked.read().copied().collect();
    if !options.keys_live() {
        return;
    }
    let hurt = asked.contains(&DebugCommand::HurtNext);
    let kill = asked.contains(&DebugCommand::Die);
    if !hurt && !kill {
        return;
    }
    let Some(health) = controller.health.as_mut().filter(|health| health.alive) else { return };
    if kill {
        let left = health.part(Part::Head).current;
        health.apply_damage(Part::Head, left + 1.0, DamageKind::Bullet);
        // A shot: the player told of it (`ApplyShot` → `ReceiveDamage`).
        received.write(crate::effects::contusion_view::Received { part: Part::Head, damage: left + 1.0, absorbed: 0.0, kind: DamageKind::Bullet, burn: 0.0 });
        carried.note = "Killed (a bullet through the head)".to_owned();
        return;
    }
    let part = HURT_ORDER[*next % HURT_ORDER.len()];
    *next += 1;
    let done = health.apply_damage(part, HURT_DAMAGE, DamageKind::Bullet);
    received.write(crate::effects::contusion_view::Received { part, damage: HURT_DAMAGE, absorbed: 0.0, kind: DamageKind::Bullet, burn: 0.0 });
    carried.note = format!("{} hurt by {done:.0}", part.name());
}

/// What HURT THE NEXT PART does to a part, and the parts it goes to in turn.
const HURT_DAMAGE: f32 = 15.0;

const HURT_ORDER: [Part; 7] = [Part::LeftArm, Part::RightArm, Part::LeftLeg, Part::RightLeg, Part::Stomach, Part::Chest, Part::Head];

/// The damage a run was told to do (`--damage`), on its drawn frame,
/// counted as the taps are: a bullet's, so that the game's own chances of
/// a bleeding or a fracture go with it. And the poisoned blade's blows
/// (`--poison`): the install's knife with a poison of its own (the
/// cultists' damask), its damage a knife's, then its poison as the game
/// puts it on after a blow (`Player.ApplyDamageInfo`,
/// `TryApplySideEffects`), from one blade for the run, its doses spent as
/// they go. In any run, so that a check made by hand can use them.
pub(super) fn damage_asked(options: Res<crate::Options>, game: Res<crate::GameData>, mut controller: ResMut<Controller>, mut frame: Local<u32>, mut blade: Local<Option<Option<(String, String, trp_sim::condition::health::SideEffect)>>>, mut received: MessageWriter<crate::effects::contusion_view::Received>) {
    *frame += 1;
    let Some(health) = controller.health.as_mut() else { return };
    for (_, part, amount, burn) in options.damage.iter().filter(|(at, ..)| *at == *frame) {
        // Nothing reaches the dead (`ApplyShot`).
        if health.alive {
            health.apply_damage(*part, *amount, DamageKind::Bullet);
            received.write(crate::effects::contusion_view::Received { part: *part, damage: *amount, absorbed: 0.0, kind: DamageKind::Bullet, burn: burn * amount });
        }
    }
    for (_, part, amount) in options.poison.iter().filter(|(at, ..)| *at == *frame) {
        if !health.alive {
            continue;
        }
        let db = &game.db;
        let found = blade.get_or_insert_with(|| {
            let mut knives: Vec<(String, String, trp_sim::condition::health::SideEffect)> = db
                .items
                .values()
                .filter(|item| db.class_names(&item.id).iter().any(|name| name == "Knife"))
                .filter_map(|item| Some((item.id.clone(), item.name.clone(), trp_sim::condition::health::SideEffect::of(&item.props)?)))
                .collect();
            knives.sort_by(|a, b| a.0.cmp(&b.0));
            knives.into_iter().next()
        });
        let Some((template, name, side)) = found.as_mut() else {
            warn!("--poison: no knife in the install carries a poison");
            continue;
        };
        let did = health.apply_damage(*part, *amount, DamageKind::Melee);
        received.write(crate::effects::contusion_view::Received { part: *part, damage: *amount, absorbed: 0.0, kind: DamageKind::Melee, burn: 0.0 });
        let put_on = health.try_apply_side_effects(did, template, side);
        info!("--poison on frame {}: the {name}'s blow took {did:.1} from the {}; its poison ({}) {}, {} of {} doses left", *frame, part.name(), side.buffs, if put_on { "put on" } else { "not put on" }, side.value, side.max);
    }
}

/// The game's text key of Immunity's notice (`HealthHelper`'s name of
/// `IImmunityPreventedNegativeEffect`).
pub(super) const IMMUNITY_NOTICE: &str = "IImmunityPreventedNegativeEffect";

/// What the armour and the backpack worn take off the speed
/// (`Player.UpdatePenalties`, `ESpeedLimit.Armor`) and off the weapon's
/// ergonomics (`RecalculateEquipmentParams`), worked out again whenever what
/// is carried or the skills change: the vest skills' shares, of the speed
/// only, while `SKILLS` is on (movement spec, "Speed limits").
pub(super) fn armour_slows(
    mut controller: ResMut<Controller>,
    carried: Res<crate::screens::inventory_view::Carried>,
    game: Res<crate::GameData>,
    skills: Option<Res<crate::player::skills::Skills>>,
    mut last: Local<Option<(Vec<trp_sim::condition::armor::Worn>, trp_sim::condition::armor::VestSkills)>>,
) {
    let skills_changed = skills.as_ref().is_some_and(|s| s.is_changed());
    if !carried.is_changed() && !skills_changed {
        return;
    }
    let vests = skills.as_deref().filter(|s| s.on()).map_or_else(trp_sim::condition::armor::VestSkills::default, |s| trp_sim::condition::armor::VestSkills::of(&s.levels));
    let worn = crate::world::dummy::worn_of(&carried.equipment.item);
    if last.as_ref().is_some_and(|(w, v)| *w == worn && *v == vests) {
        return;
    }
    let penalty = trp_sim::condition::armor::speed_penalty(&game.db, &worn, &vests);
    // And their share of the weapon's ergonomics, which no skill lessens
    // (weapon handling spec, "The gear's share of the ergonomics").
    let ergonomics = trp_sim::condition::armor::ergonomics_share(&game.db, &worn);
    if last.is_some() || penalty != 0.0 || ergonomics != 0.0 {
        info!("armour and backpack: {penalty:+.2}% speed, {:+.2}% ergonomics", ergonomics * 100.0);
    }
    *last = Some((worn, vests));
    controller.movement.set_armour_penalty(penalty);
    controller.gear_ergonomics = ergonomics;
}

/// Each hit given to the player, as the movement takes it
/// (`Player.ApplyHitDebuff`): a weapon's caps the speed for a while by the
/// stamina it burns, and a leg's ends a sprint (`Movement::hit`; movement
/// spec, "Speed limits"). Nothing reaches the dead.
pub(super) fn hits_taken(mut controller: ResMut<Controller>, mut received: MessageReader<crate::effects::contusion_view::Received>) {
    let alive = controller.health.as_ref().is_none_or(|health| health.alive);
    for hit in received.read() {
        if alive {
            controller.movement.hit(hit.burn, hit.part, hit.kind);
        }
    }
}

/// The death screen: over everything, shown once the character is dead
/// and the game's black is done (`death::SCREEN_AT`).
#[derive(Component)]
pub(super) struct DeathScreen;

#[derive(Component)]
pub(super) struct DeathCause;

pub(super) fn spawn_death_screen(mut commands: Commands) {
    let whole = Node {
        position_type: PositionType::Absolute,
        width: Val::Percent(100.0),
        height: Val::Percent(100.0),
        flex_direction: FlexDirection::Column,
        align_items: AlignItems::Center,
        justify_content: JustifyContent::Center,
        row_gap: Val::Px(14.0),
        ..default()
    };
    commands.spawn((DeathScreen, whole, BackgroundColor(Color::srgba(0.02, 0.0, 0.0, 0.94)), GlobalZIndex(40), Visibility::Hidden)).with_children(|screen| {
        screen.spawn((DeathCause, words("", 30.0, Color::srgb(0.82, 0.24, 0.20))));
        screen.spawn(words("ENTER: the run again, where it began", 15.0, FAINT));
    });
}

pub(super) fn show_death(
    controller: Res<Controller>,
    dying: Res<crate::player::death::Dying>,
    said: Res<Words>,
    mut screens: Query<&mut Visibility, With<DeathScreen>>,
    mut causes: Query<&mut Text, With<DeathCause>>,
) {
    let shut = dying.since().is_some_and(|since| since >= crate::player::death::SCREEN_AT);
    let dead = controller.health.as_ref().filter(|health| !health.alive && shut);
    let wanted = if dead.is_some() { Visibility::Inherited } else { Visibility::Hidden };
    for mut shown in &mut screens {
        if *shown != wanted {
            *shown = wanted;
        }
    }
    if let Some(health) = dead {
        let title = said.say("Killed in Action", "Killed in Action").to_uppercase();
        let cause = said.cause(health.cause.unwrap_or(DamageKind::Undefined));
        let now = format!("{title}\n{}{cause}", said.say("Killed by", "Killed by "));
        for mut text in &mut causes {
            if text.0 != now {
                text.0 = now.clone();
            }
        }
    }
}

/// The body's hit colliders that barbed wire is told of
/// (`BodyPartCollider`), each where its bone is, or halfway to the next
/// bone of its limb, and the part it is of. `PROVISIONAL`: the game's own
/// colliders (twenty-six, with their sizes, in the player's prefab) are not
/// read; these are the largest ones, at their bones.
const COLLIDERS: [(&str, Option<&str>, Part); 12] = [
    ("Base HumanHead", None, Part::Head),
    ("Base HumanSpine3", None, Part::Chest),
    ("Base HumanSpine2", None, Part::Chest),
    ("Base HumanPelvis", None, Part::Stomach),
    ("Base HumanLUpperarm", Some("Base HumanLForearm1"), Part::LeftArm),
    ("Base HumanLForearm1", Some("Base HumanLPalm"), Part::LeftArm),
    ("Base HumanRUpperarm", Some("Base HumanRForearm1"), Part::RightArm),
    ("Base HumanRForearm1", Some("Base HumanRPalm"), Part::RightArm),
    ("Base HumanLThigh1", Some("Base HumanLCalf"), Part::LeftLeg),
    ("Base HumanLCalf", Some("Base HumanLFoot"), Part::LeftLeg),
    ("Base HumanRThigh1", Some("Base HumanRCalf"), Part::RightLeg),
    ("Base HumanRCalf", Some("Base HumanRFoot"), Part::RightLeg),
];

/// Barbed wire on the range (`BarbedWire`, a `DamageTrigger`; health spec,
/// "Barbed wire"): while the character's capsule is in its box the speed
/// is held down and a sprint ended, and each collider in the box is cut by
/// how far it has moved.
pub(super) fn barbed_wire(
    mut controller: ResMut<Controller>,
    rig: Option<Res<BodyRig>>,
    parts: Option<Res<BodyParts>>,
    bones: Query<&GlobalTransform>,
    range: Res<Range>,
    mut tallies: Local<Vec<BarbTally>>,
    mut received: MessageWriter<crate::effects::contusion_view::Received>,
) {
    let controller = &mut *controller;
    let Some(health) = controller.health.as_mut().filter(|health| health.alive) else { return };
    let [low, high] = crate::world::range::barbed_wire();
    let feet = controller.movement.position;
    let feet = Vec3::new(feet[0], feet[1], -feet[2]);
    // The capsule's footprint over the box, and the box's height over the
    // feet.
    let nearest = Vec2::new(feet.x.clamp(low.x, high.x), feet.z.clamp(low.z, high.z));
    let inside = nearest.distance(Vec2::new(feet.x, feet.z)) <= range.radius && feet.y < high.y && feet.y + controller.movement.capsule_height() > low.y;
    let (Some(rig), Some(parts)) = (rig, parts) else {
        // No body to find the colliders on: the capsule stands for them.
        controller.movement.set_in_barbed_wire(inside, inside);
        return;
    };
    if !inside {
        controller.movement.set_in_barbed_wire(false, false);
        return;
    }
    if tallies.len() != COLLIDERS.len() {
        *tallies = vec![BarbTally::default(); COLLIDERS.len()];
    }
    let at = |end: &str| rig.bone(&parts, end).and_then(|bone| bones.get(bone).ok()).map(GlobalTransform::translation);
    let mut cut = false;
    for ((bone, toward, part), tally) in COLLIDERS.into_iter().zip(tallies.iter_mut()) {
        let Some(from) = at(bone) else { continue };
        let middle = toward.and_then(at).map_or(from, |to| (from + to) / 2.0);
        let within = middle.cmpge(low).all() && middle.cmple(high).all();
        if !within {
            continue;
        }
        cut = true;
        if let Some(damage) = tally.cut(middle.to_array()) {
            // `ApplyEnvironmentalDamage` → `ApplyDamageInfo`: health's, and
            // the player told of it, while alive.
            if health.alive {
                health.apply_damage(part, damage, DamageKind::Barbed);
                received.write(crate::effects::contusion_view::Received { part, damage, absorbed: 0.0, kind: DamageKind::Barbed, burn: 0.0 });
            }
        }
    }
    controller.movement.set_in_barbed_wire(true, cut);
}
