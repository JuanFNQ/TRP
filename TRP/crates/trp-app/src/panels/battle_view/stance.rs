//! The stance followed and drawn.

use super::*;

/// Follows the character as the stance panel's script and the battle
/// screen do: what raises the panel, the figure, the sliders, the notches,
/// the fills and the noise.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(super) fn follow_stance(
    controller: Res<Controller>,
    look: Res<HealthLook>,
    time: Res<Time>,
    mut state: ResMut<BattleState>,
    (settings, held, picker): (Res<BattleModes>, Res<HandsHeld>, Res<crate::screens::ammo_picker::Picker>),
    (bindings, options): (Res<Bindings>, Res<crate::Options>),
    (carried, game, mut malfunctions): (Res<Carried>, Res<crate::GameData>, MessageReader<crate::weapons::wear::MalfunctionShown>),
    window: Query<&Window, With<PrimaryWindow>>,
    mut pieces: Query<(&Piece, &mut Visibility, &mut UiTransform, &mut Node, Option<&FullWidth>, Option<&mut ImageNode>, Option<&mut Own>, Option<&bevy::ui::ComputedNode>)>,
    mut fills: Query<(&PieceFill, &mut ImageNode), Without<Piece>>,
    mut notches: Query<(&SpeedNotch, &mut Own), Without<Piece>>,
) {
    let Some(views) = look.battle.as_ref().filter(|_| look.tarkov_ui) else { return };
    let m = &controller.movement;
    let alive = controller.health.as_ref().is_none_or(|health| health.alive);
    let up = !controller.inventory_open && !controller.edit_build && alive;
    let state = &mut *state;
    // The pistols' glow runs whether the screen is up or not: stepped;
    // killed by a quick draw of the holster's weapon begun
    // (`PlayerOnStartQuickdrawPistol`: `DOKill(complete: true)`, the views at
    // rest at once); begun afresh by a malfunction made known, 150 ms after
    // the shot (`ProcessMalfunctionExamined`), on the views whose pistol has
    // none known. `PROVISIONAL`: one made known while a glow runs begins it
    // afresh (the game starts a second tween on the same picture beside the
    // first, which of the two draws last not read).
    if let Some(glow) = state.quickdraw.as_mut() {
        glow.at += time.delta_secs();
    }
    if state.quickdraw.as_ref().is_some_and(|glow| glow_alpha(glow.at, glow.legs).is_none()) {
        state.quickdraw = None;
    }
    if held.quickdraws != state.quickdraws_seen {
        state.quickdraws_seen = held.quickdraws;
        state.quickdraw = None;
    }
    let mut malfunction_shown = false;
    for shown in malfunctions.read() {
        let keys = look.battle_quick.iter().flat_map(|quick| quick.slots.iter().map(|slot| slot.key.as_str()));
        let slots = glowing_slots(keys, |key| quick_thing(&carried, key).1, |item| is_pistol(&game.db, item));
        state.quickdraw = Some(QuickdrawGlow { slots, at: 0.0, legs: glow_legs(shown.glow) });
        malfunction_shown = true;
    }
    // The screen shut (the inventory open, the character dead): every
    // panel gone at once (`BattleUIScreen.Close`).
    if !up {
        if state.up {
            state.up = false;
            state.stance.close();
            state.health.close();
            state.quick.close();
        }
        return;
    }
    let pulls = controller.health_pulls;
    let now = Seen {
        pose_level: m.pose_level,
        prone: m.prone,
        max_speed: m.settings.max_speed,
        limit: m.speed_limit(),
        speed: if m.sprinting { m.character_speed } else { m.clamped_speed() },
        legs: m.stamina.legs.changes(),
        arms: m.stamina.arms.changes(),
        sprinting: m.sprinting,
        pulls,
    };
    if !state.up {
        // The screen shown, by the settings as they stand: each panel shut,
        // then as the settings have it with nothing raising it
        // (`UpdatePanelsVisibility(false)`): always shown, it fades in.
        state.up = true;
        state.modes = *settings;
        let modes = state.modes;
        state.stance.close();
        state.health.close();
        state.quick.close();
        state.sprint_holds = false;
        state.seen = None;
        state.hands_seen = held.changes;
        // The mount's icon as the character stands (`BattleUiBipodPanel.
        // Show`: `ChangeMountImage`), no flash.
        state.mount_side = m.mount_view().map(|view| view.side.number());
        state.mount_flash = None;
        state.refusals_seen = controller.mount_refusals;
        match modes.stamina {
            Shows::Always => state.stance.show(false),
            _ => state.stance.hide(0.0),
        }
        // (No health panel at all with `HEALTH SYSTEM` off.)
        if controller.health.is_some() {
            state.decide_health(false, modes);
        }
        match modes.quick {
            Shows::Always => state.quick.show(false),
            _ => state.quick.hide(0.0),
        }
    }
    let modes = state.modes;
    // A quick slot's key or the grenade's pressed (`PressSlot4` to `0`,
    // `ThrowGrenade`, on the key going down): the bar's own input node
    // shows it with no fade out, whatever the setting
    // (`InventoryScreenQuickAccessPanel.TranslateCommand`, `SetVisibility
    // (true)`: shown where `Always` or asked, `Never` too), before the
    // hands hear the key; up till the hands change or the screen comes up
    // again (`courses10`'s `battle-stances-quickslots`: 4 pressed, nothing
    // bound, the bar up the 4.3 s to the inventory's opening).
    if options.keys_live() && QUICK_PRESSES.iter().any(|name| bindings.fired(name)) {
        state.quick.show(false);
    }
    // The hands begin to change (`HandsChangingEvent`): the quick bar
    // raised (`method_14`).
    if held.changes != state.hands_seen {
        state.hands_seen = held.changes;
        state.raise_quick(modes);
    }
    // A malfunction made known: raised the same way (`ProcessMalfunctionExamined`'s
    // `method_14`), only where the screen is up.
    if malfunction_shown {
        state.raise_quick(modes);
    }
    // The magazine picker shown (`ShowAmmoSelector`: `SetVisibility(true)`,
    // `Never` too) holds the bar up with no fade out; shut
    // (`SetVisibility(false)`), the bar fades out, unless it is always shown.
    let picking = picker.shown.is_some();
    if picking != state.picking {
        state.picking = picking;
        if picking || modes.quick == Shows::Always {
            state.quick.show(false);
        } else {
            state.quick.hide(0.0);
        }
    }
    let figures = &views.figures;
    let mut raise_health = false;
    let mut raise_late = false;
    match state.seen {
        // The panel shown (`BattleStancePanel.Show`): set as the character
        // stands, nothing raised.
        None => {
            state.stance_image(figures, if m.prone { -0.35 } else { m.pose_level });
            state.max_speed(now.limit, m.clamped_speed(), now.max_speed);
            state.noise = noise_level(&controller);
        }
        Some(before) => {
            let mut pull = false;
            if now.legs != before.legs || now.arms != before.arms {
                pull = true;
            }
            if now.prone != before.prone {
                state.stance_image(figures, if m.prone { -0.35 } else { m.smoothed_pose_level });
                state.noise = noise_level(&controller);
                pull = true;
            } else if (now.pose_level - before.pose_level).abs() > 0.0 {
                state.stance_image(figures, if m.prone { -0.35 } else { m.pose_level });
                state.noise = noise_level(&controller);
                pull = true;
            }
            // A limit's change is heard in the movement's update, after the
            // frame's fades have stepped (`ProcessSpeedLimits`, the event
            // `OnMaxSpeedChangedEvent`): its raise is first stepped on the
            // next frame. The arms' drain begun or ended by the aim's key
            // raises it before (`Physical.Aim`, `HandsStamina.OnChanged`),
            // so the aim's key raises it twice (`courses10`'s
            // `battle-panels`: 0.0331 on the key's frame, 0.0650 on the next).
            if now.max_speed != before.max_speed || now.limit != before.limit {
                state.max_speed(now.limit, m.clamped_speed(), now.max_speed);
                state.noise = noise_level(&controller);
                raise_late = true;
            }
            // The speed told raises it by itself (`RaiseChangeSpeedEvent`),
            // but not as a limit moves it: the limit's event tells it then.
            // PROVISIONAL: read from the code, a limit's change alone not
            // compared.
            if now.speed != before.speed {
                state.speed = [now.speed, now.max_speed];
                state.noise = noise_level(&controller);
                pull |= !raise_late;
            }
            if now.sprinting != before.sprinting {
                // `PullUpBattleStancePanelBySprint`: held up while the
                // sprint lasts, out three seconds after it.
                state.noise = noise_level(&controller);
                state.sprint_holds = now.sprinting;
                if modes.stamina != Shows::Never {
                    if now.sprinting {
                        state.stance.show(false);
                    } else if modes.stamina == Shows::Autohide {
                        state.stance.hide(VISIBILITY_DURATION);
                    }
                }
            }
            if pull {
                state.pull_up(modes);
            }
            if controller.health.is_some() {
                raise_health = now.pulls[0] != before.pulls[0] || (now.pulls[1] != before.pulls[1] && !state.health_autohide);
            }
        }
    }
    state.seen = Some(now);
    // The mount's icon (`BattleUiBipodPanel`): set afresh as the mount's
    // data is set or cleared (`OnMountedStateChanged`: `ChangeMountImage`,
    // which stops a flash, the panel raised), and flashed red when a search
    // finds nothing (`OnNothingFound`: `OnMountUnavailable`, unless a flash
    // is under way; the panel raised).
    let side = m.mount_view().map(|view| view.side.number());
    if side != state.mount_side {
        state.mount_side = side;
        state.mount_flash = None;
        state.pull_up(modes);
    }
    if controller.mount_refusals != state.refusals_seen {
        state.refusals_seen = controller.mount_refusals;
        if state.mount_flash.is_none() {
            state.mount_flash = Some(0.0);
        }
        state.pull_up(modes);
    }
    if state.ladder.wait > 0 {
        state.ladder.wait -= 1;
        if state.ladder.wait == 0 {
            state.ladder.step();
        }
    }
    let dt = time.delta_secs();
    state.stance.advance(dt);
    state.health.advance(dt);
    state.quick.advance(dt);
    // Health's events are heard after the frame's fades have stepped: a
    // raise by damage is first stepped on the next frame, and a second in
    // the frame after starts from the step it had (`courses10`'s
    // `battle-health-autohide`: alpha 0 on the landing's frame, 0.0857 on
    // the next, raised again there by the jump's end on a broken leg).
    if raise_health {
        state.decide_health(true, modes);
    }
    if raise_late {
        state.pull_up(modes);
    }
    let mount = views.mount.as_ref();
    let flash_length = mount.map_or(0.0, |mount| mount.highlight);
    if let Some(at) = state.mount_flash.as_mut() {
        *at += dt;
        if *at >= flash_length {
            state.mount_flash = None;
        }
    }
    let red = state.mount_flash.map_or(0.0, |at| mount_flash_alpha(at, flash_length));

    // The pieces as the script leaves them.
    let Ok(window) = window.single() else { return };
    let s = window.height() / 1080.0;
    let stance = &views.stance;
    let own_size = stance.nodes[0].layout.size_delta;
    let under = |index: usize, top: usize| {
        let mut above = Some(index);
        while let Some(at) = above {
            if at == top {
                return true;
            }
            above = stance.nodes[at].parent;
        }
        false
    };
    // How far each slider's handle is moved from where the scene has it.
    let moved = |slider: Option<SliderParts>, share: f32| -> Option<(usize, [f32; 2])> {
        let slider = slider?;
        let handle = slider.handle?;
        let [x0, y0, ..] = stance.rect(handle, own_size);
        let [x1, y1, ..] = slider.handle_at(stance, own_size, share)?;
        Some((handle, [x1 - x0, y1 - y0]))
    };
    let stance_share = views.stance_slider.map_or(0.0, |slider| SliderParts::share(state.stance_value, slider.min, slider.max));
    let speed_share = SliderParts::share(state.speed[0].min(state.speed[1]), 0.0, state.speed[1]);
    let handles: Vec<(usize, [f32; 2])> = [moved(views.stance_slider, stance_share), moved(views.speed_slider, speed_share)].into_iter().flatten().collect();
    let final_image = views.final_notch.and_then(|notch| stance.find(&format!("{}/Image", stance.nodes[notch].path)));
    let notch_color = |had: bool| crate::screens::health_view::rgba(views.notch_colors[if had { 0 } else { 1 }]);
    let picture = |key: Option<(i32, i64)>| key.and_then(|key| look.battle_sprites.get(&key)).and_then(|picture| Some((look.images.get(picture.texture)?.clone()?, picture.cut)));
    let level = [m.stamina.legs.level(), m.stamina.arms.level()];
    let spent = [m.stamina.legs.exhausted(), m.stamina.arms.exhausted()];
    for (piece, mut visibility, mut transform, mut node, full, image, own, laid) in &mut pieces {
        let Some(index) = piece.0.checked_sub(STANCE).filter(|&index| index < SPAN) else { continue };
        if let Some(n) = figures.iter().position(|(object, _)| *object == index) {
            let want = if n == state.ladder.shown { Visibility::Inherited } else { Visibility::Hidden };
            if *visibility != want {
                *visibility = want;
            }
        }
        if let Some(mount) = mount {
            let shown = if index == mount.image {
                Some(state.mount_side.is_some())
            } else {
                mount.directions.iter().find(|(object, _)| *object == index).map(|(_, side)| state.mount_side == Some(*side))
            };
            if let Some(shown) = shown {
                let want = if shown { Visibility::Inherited } else { Visibility::Hidden };
                if *visibility != want {
                    *visibility = want;
                }
            }
        }
        if let Some((_, [dx, dy])) = handles.iter().find(|(handle, _)| under(index, *handle)) {
            let want = Val2::px(dx * s, dy * s);
            if transform.translation != want {
                transform.translation = want;
            }
        }
        for (fill, n) in [(views.legs_fill, 0), (views.arms_fill, 1)] {
            if fill == Some(index) {
                if let Some(full) = full {
                    // (Written as its edge moves a pixel: `set_fill_width`.)
                    crate::screens::health_view::set_fill_width(&mut node, laid, full.0 * level[n].clamp(0.0, 1.0));
                }
            }
        }
        if mount.is_some_and(|mount| mount.unavailable == index) {
            if let (Some(mut own), Some(color)) = (own, stance.nodes[index].image.map(|image| crate::screens::health_view::rgba(image.color))) {
                own.image = Some(color.with_alpha(red));
            }
        } else if Some(index) == final_image {
            if let Some(mut own) = own {
                own.ground = Some(notch_color(state.notches[20]));
            }
        } else if Some(index) == views.noise {
            if let (Some(mut image), Some((handle, cut))) = (image, picture(views.noise_sprites.get(state.noise).copied())) {
                if image.image != handle || image.rect != Some(cut) {
                    image.image = handle;
                    image.rect = Some(cut);
                }
            }
        }
    }
    // The fills' sprites: each its own, or the spent one under 15.
    for (fill, mut image) in &mut fills {
        let Some(index) = fill.0.checked_sub(STANCE).filter(|&index| index < SPAN) else { continue };
        for (object, n) in [(views.legs_fill, 0), (views.arms_fill, 1)] {
            if object == Some(index) {
                let key = if spent[n] { views.fill_sprites[2] } else { views.fill_sprites[n] };
                if let Some((handle, cut)) = picture(key) {
                    if image.image != handle || image.rect != Some(cut) {
                        image.image = handle;
                        image.rect = Some(cut);
                    }
                }
            }
        }
    }
    for (notch, mut own) in &mut notches {
        own.ground = Some(notch_color(state.notches[notch.0]));
    }
}
