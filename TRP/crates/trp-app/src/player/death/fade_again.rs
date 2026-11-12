//! The fade to black and beginning the run again.

use super::*;

/// The picture fades and the eyes close (`DeathFade`): the colour's fade,
/// its look `PROVISIONAL` (the game's shader is not read: here the colour
/// drained by its strength), and the eyes as two lids of the shader's
/// colour, black, meeting in the middle at full strength (`PROVISIONAL`
/// likewise). Over all of it the raid's end blacks the screen out (`Black`),
/// whole after a second: the eyes' closing, from 1.8 s, is under it.
#[allow(clippy::type_complexity)]
pub fn fade(
    mut commands: Commands,
    dying: Res<Dying>,
    mut grading: Query<(Entity, Option<&mut bevy::render::view::ColorGrading>), With<MainCamera>>,
    mut lids: Query<(&Lid, &mut Node, &mut Visibility), Without<Black>>,
    mut black: Query<(&mut BackgroundColor, &mut Visibility), (With<Black>, Without<Lid>)>,
    mut made: Local<bool>,
) {
    if !*made {
        *made = true;
        for top in [true, false] {
            let mut node = Node { position_type: PositionType::Absolute, left: Val::Px(0.0), width: Val::Percent(100.0), height: Val::Percent(0.0), ..default() };
            if top {
                node.top = Val::Px(0.0);
            } else {
                node.bottom = Val::Px(0.0);
            }
            commands.spawn((Lid, node, BackgroundColor(Color::BLACK), GlobalZIndex(38), Visibility::Hidden));
        }
        let whole = Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() };
        commands.spawn((Black, whole, BackgroundColor(Color::NONE), GlobalZIndex(39), Visibility::Hidden));
    }
    let opacity = dying.since().map_or(0.0, |since| (since * BLACK_STEP).min(1.0));
    for (mut ground, mut visibility) in &mut black {
        let wanted = Color::srgba(0.0, 0.0, 0.0, opacity);
        if ground.0 != wanted {
            ground.0 = wanted;
        }
        let shown = if opacity > 0.0 { Visibility::Inherited } else { Visibility::Hidden };
        if *visibility != shown {
            *visibility = shown;
        }
    }
    let since = dying.since();
    let curve = |share: f32| share.clamp(0.0, 1.0).powi(2);
    let colour = since.map_or(0.0, |since| curve(since / FADE_TIME) * FADE_SHARE);
    let eyes = since.map_or(0.0, |since| curve((since - EYES_DELAY) / EYES_TIME) * EYES_SHARE);
    if let Ok((camera, grading)) = grading.single_mut() {
        let saturation = 1.0 - colour;
        match grading {
            Some(mut grading) => {
                if grading.global.post_saturation != saturation {
                    grading.global.post_saturation = saturation;
                }
            }
            None if colour > 0.0 => {
                let mut grading = bevy::render::view::ColorGrading::default();
                grading.global.post_saturation = saturation;
                commands.entity(camera).insert(grading);
            }
            None => {}
        }
    }
    for (_, mut node, mut visibility) in &mut lids {
        let wanted = Val::Percent(eyes * 100.0);
        if node.height != wanted {
            node.height = wanted;
        }
        let shown = if eyes > 0.0 { Visibility::Inherited } else { Visibility::Hidden };
        if *visibility != shown {
            *visibility = shown;
        }
    }
}

/// A lid of the eyes closing.
#[derive(Component)]
pub struct Lid;

/// The black the raid's end lays over everything (`PreloaderUI`'s
/// `_overlapBlackImage`).
#[derive(Component)]
pub struct Black;

/// Dead, with the death screen up, `Enter` begins the run again where it
/// began: the controller as
/// the run made it, the inventory's things and the range as they were; the
/// corpse, its physics and the fade gone, and the living body back. Either
/// Enter, the main one or the keypad's, as the inventory's windows take
/// either (the keypad's alone had once done nothing, and a dead character
/// could not respawn).
#[allow(clippy::too_many_arguments)]
pub fn begin_again(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut dying: ResMut<Dying>,
    start: Option<Res<StartOfRun>>,
    mut controller: ResMut<Controller>,
    mut carried: ResMut<Carried>,
    mut range: ResMut<Range>,
    corpse: Option<Res<Corpse>>,
    body: Option<Res<BodyRig>>,
    of_death: Query<Entity, (With<OfDeath>, Without<ChildOf>)>,
    mut again: MessageWriter<BeginAgain>,
    features: Res<crate::screens::options_view::Features>,
    mut dummies: Option<ResMut<crate::world::dummy::Dummies>>,
) {
    let ready = dying.since().is_some_and(|since| since >= SCREEN_AT);
    if !ready || !keys.any_just_pressed([KeyCode::Enter, KeyCode::NumpadEnter]) {
        return;
    }
    let Some(start) = start else { return };
    // The counts other systems remember a value of run on, whatever the
    // run begins again with: put back to the start's, the muzzle's flash
    // and the particles took `shots_fired` less the one they had seen for
    // some four billion new shots, and played them all in one frame (a
    // minute's freeze after any shot, then a death and Enter: dying in that
    // state gave no respawn, and the app crashed).
    let running = (controller.shots_fired, controller.launcher_shots_fired, controller.mount_refusals, controller.meds_started, controller.health_pulls);
    *controller = start.controller.clone();
    (controller.shots_fired, controller.launcher_shots_fired, controller.mount_refusals, controller.meds_started, controller.health_pulls) = running;
    let carried = &mut *carried;
    // A body searched keeps its gear, before the containers are put back.
    carried.put_back_body();
    carried.equipment = start.equipment.clone();
    carried.checked = crate::screens::inventory_view::magazines_in(&carried.equipment.item);
    carried.checked_chambers.clear();
    carried.loot = start.loot.clone();
    carried.away = start.away.clone();
    carried.searcher = start.searcher.clone();
    // The dummies' bodies keep what they hold, with numbers of their own
    // (a thing of the player's or a crate's put on a body is back where it
    // was as the run began, under the same number), each dummy told its
    // things' new numbers, hidden again as the crates are (inventory spec,
    // "Searching a body").
    crate::world::dummy::renumber_bodies(carried, dummies.as_deref_mut());
    if features.searching {
        for gear in carried.bodies.iter().flatten() {
            carried.searcher.hide_body(gear);
        }
    }
    carried.ground = start.ground.clone();
    carried.falling.clear();
    carried.loot_open = false;
    carried.loot_of = 0;
    carried.windows.clear();
    carried.asking = None;
    carried.priority = None;
    carried.note.clear();
    *range = start.range.clone();
    for entity in &of_death {
        commands.entity(entity).despawn();
    }
    if let Some(corpse) = corpse {
        commands.entity(corpse.root).insert(Visibility::Hidden);
    }
    if let Some(body) = body {
        commands.entity(body.root()).insert(Visibility::Inherited);
    }
    // The weapon's bone back where it hung on the rig.
    if let Some((weapon_root, rest)) = dying.0.as_ref().and_then(|death| death.weapon_rest) {
        commands.entity(weapon_root).insert(rest);
    }
    dying.0 = None;
    again.write(BeginAgain);
    info!("the run begun again where it began");
}
