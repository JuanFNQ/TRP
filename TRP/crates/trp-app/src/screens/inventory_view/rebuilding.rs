//! Rebuilding the screen after a change, and scripted drags.

use super::*;

/// Unity's `Mathf.SmoothDamp` (its published source): a value eased
/// toward a target as a critically damped spring reaching it in about so
/// long (at least a ten-thousandth of a second), with its speed kept
/// between frames; never past the target.
pub(crate) fn smooth_damp(current: f32, target: f32, speed: &mut f32, length: f32, step: f32) -> f32 {
    let length = length.max(0.0001);
    let omega = 2.0 / length;
    let x = omega * step;
    let decay = 1.0 / (1.0 + x + 0.48 * x * x + 0.235 * x * x * x);
    let change = current - target;
    let temp = (*speed + omega * change) * step;
    *speed = (*speed - omega * temp) * decay;
    let mut output = target + (change + temp) * decay;
    if (target - current > 0.0) == (output > target) {
        output = target;
        *speed = if step > 0.0 { (output - target) / step } else { 0.0 };
    }
    output
}

/// Whether a point lies in a rectangle (left, top, width, height).
pub(super) fn inside(rect: [f32; 4], point: [f32; 2]) -> bool {
    (rect[0]..rect[0] + rect[2]).contains(&point[0]) && (rect[1]..rect[1] + rect[3]).contains(&point[1])
}

/// A scroll bar's value with its handle dragged to the mouse
/// (`Scrollbar.UpdateDrag`): the handle's share of its sliding area kept
/// where the mouse took it, `offset` up from its middle; `sliding` is the
/// sliding area's top and height, `cursor` the mouse's height, y down.
/// None where the handle fills the area.
pub(super) fn dragged_to(sliding: [f32; 2], share: f32, offset: f32, cursor: f32, bottom_up: bool) -> Option<f32> {
    let [top, height] = sliding;
    let remaining = height * (1.0 - share);
    if remaining <= 0.0 {
        return None;
    }
    let corner = (top + height - cursor) - offset - share * height / 2.0;
    let value = (corner / remaining).clamp(0.0, 1.0);
    Some(if bottom_up { value } else { 1.0 - value })
}

/// A scroll bar's value after one step of the bar held off its handle
/// (`Scrollbar.ClickRepeat`): a handle's share toward where it was
/// pressed, kept to four places. None once the handle is under the press.
pub(super) fn stepped(value: f32, share: f32, handle: [f32; 4], press: [f32; 2], bottom_up: bool) -> Option<f32> {
    if inside(handle, press) {
        return None;
    }
    // Where the press is from the handle's middle, up.
    let up = (handle[1] + handle[3] / 2.0) - press[1];
    let change = if up < 0.0 { share } else { -share };
    let value = value + if bottom_up { -change } else { change };
    Some((value.clamp(0.0, 1.0) * 10000.0).round() / 10000.0)
}

/// Draws the Gear page afresh when something has moved or been opened,
/// or the window has another height; and with it what lies over the page
/// while the screen is open on that tab: the quick-use bar, the windows,
/// the window that asks how many.
pub(super) fn rebuild(
    mut commands: Commands,
    carried: Res<Carried>,
    controller: Res<Controller>,
    selected: Res<Selected>,
    bindings: Res<Bindings>,
    (scroll, scrolls): (Res<LootScroll>, Res<GameScrolls>),
    window: Query<&Window, With<PrimaryWindow>>,
    mut units: ResMut<Units>,
    page: Query<Entity, With<GearPage>>,
    over: Query<Entity, With<Overlay>>,
    figure: Res<crate::screens::figure_view::FigureImage>,
    look: Res<crate::screens::health_view::HealthLook>,
    said: Res<crate::screens::health_view::Words>,
    stash_fills: Res<StashFills>,
    (game, previews, held, opening): (Res<crate::GameData>, Res<crate::screens::info_preview::InfoPreviews>, Res<HandsHeld>, Res<Opening>),
    mut kept: Local<((bool, usize, bool), u32, u8)>,
) {
    let _clock = crate::platform::render_timing::Clock::new("inventory rebuild");
    let (was, afresh, staged) = &mut *kept;
    let (now, size) = window.single().map_or((*units, Vec2::ZERO), |window| (Units(window.height() / SCREEN_HEIGHT), Vec2::new(window.width(), window.height())));
    let shows = (controller.inventory_open, selected.0, controller.edit_build);
    // Nothing is drawn while the screen is shut: what lies over it goes as
    // it shuts, and the page is drawn afresh as it opens, whatever changed
    // meanwhile (a reload's magazines, a thing taken up). Over the edit
    // build screen, its windows alone (a part inspected from it), drawn
    // afresh as they change, over the screen.
    if !controller.inventory_open {
        let changed = controller.edit_build && (carried.is_changed() || now != *units || look.is_changed() || previews.is_changed());
        if *was != shows || changed {
            for entity in &over {
                commands.entity(entity).despawn();
            }
            (*units, *was) = (now, shows);
            WINDOWS_OVER_EDIT_BUILD.store(controller.edit_build, std::sync::atomic::Ordering::Relaxed);
            if controller.edit_build {
                for (index, window) in carried.windows.iter().enumerate() {
                    let scrolled = scrolls.info.get(&window.opened).copied().unwrap_or(0.0);
                    window_box(&mut commands, &carried, *window, index, now, &look, &said, (&game, &previews, &held, controller.health.as_ref()), scrolled);
                }
            }
        }
        *afresh = 0;
        return;
    }
    WINDOWS_OVER_EDIT_BUILD.store(false, std::sync::atomic::Ordering::Relaxed);
    // The page's parts come as the game's do (`Opening`): none on the key's
    // frame and the next, the page on the one after, the quick-use bar and
    // what else lies over the page on the third; that last alone made then.
    let stage = opening.stage();
    let fresh = carried.is_changed() || now != *units || shows != *was || figure.is_changed() || look.is_changed() || stash_fills.is_changed() || previews.is_changed();
    if !fresh && stage == *staged {
        *afresh = 0;
        return;
    }
    // (The page again as the panel of what is looted comes.)
    let page_due = fresh || stage == 1 || (stage == 2 && *staged != 2 && carried.loot_open);
    *staged = stage;
    // Drawn afresh on frame after frame, something writes to what is
    // carried on every frame, and whatever goes by the mouse being over a
    // part of the page stops working (a part just made does not know
    // where the mouse is): said once, for whoever did it.
    *afresh = if carried.is_changed() { *afresh + 1 } else { 0 };
    if *afresh == 120 {
        // (Built with `--features track`, the engine names the line that wrote to it last.)
        let by = carried.changed_by().into_option().map(|place| format!(", last at {place}")).unwrap_or_default();
        warn!("the inventory's page has been drawn afresh on 120 frames running: something writes to what is carried on every frame{by}");
    }
    (*units, *was) = (now, shows);
    if page_due {
        let left = LeftPanel {
            tab: shows.1,
            look: &look,
            said: &said,
            health: controller.health.as_ref(),
            screen: if size.y > 0.0 { [size.x / now.0, SCREEN_HEIGHT] } else { [SCREEN_HEIGHT * 16.0 / 9.0, SCREEN_HEIGHT] },
            stash_fills: &stash_fills.images,
            loot_shown: stage >= 2,
        };
        for page in &page {
            let mut page = commands.entity(page);
            page.despawn_related::<Children>();
            if stage >= 1 {
                page.with_children(|gear| gear_page(gear, &carried, now, scroll.0, scrolls.at, figure.0.as_ref(), &left));
            }
        }
    }
    for entity in &over {
        commands.entity(entity).despawn();
    }
    // What lies over the page is the page's on both its tabs: the
    // quick-use bar, the windows, the window that asks how many.
    if !(shows.0 && (shows.1 == GEAR || shows.1 == HEALTH)) || stage < 2 {
        return;
    }
    quick_bar(&mut commands, &carried, &bindings, now, size, &look, &said);
    for (index, window) in carried.windows.iter().enumerate() {
        let scrolled = scrolls.info.get(&window.opened).copied().unwrap_or(0.0);
        window_box(&mut commands, &carried, *window, index, now, &look, &said, (&game, &previews, &held, controller.health.as_ref()), scrolled);
    }
    if let Some(asking) = &carried.asking {
        let pixels = window.single().map_or(1.0, |window| window.scale_factor());
        asking_box(&mut commands, asking, size, (now.0, pixels), &look, &said);
    }
}

/// The drags a run nobody is at the keys of was told to make (`--drag`):
/// one after another from a second and a half in, each with the mouse put
/// at its start, the left button pressed, the mouse moved to its end and
/// the button let go, a few frames apart so that the screen has seen
/// where the mouse is before the button changes. The mouse is the
/// window's own, so the real pointer is moved with it. The button is
/// pressed as a mouse's is, by the engine's own event for it, which the
/// next frame hears from its start: a button of the screen (a tab, a
/// window's cross, the options' cells) takes the click as it would a
/// hand's.
pub(super) fn scripted_drags(
    options: Res<Options>,
    mut window: Query<(Entity, &mut Window), With<PrimaryWindow>>,
    mut buttons: MessageWriter<bevy::input::mouse::MouseButtonInput>,
    mut typed: MessageWriter<bevy::input::keyboard::KeyboardInput>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut controller: ResMut<Controller>,
    mut wheel: MessageWriter<bevy::input::mouse::MouseWheel>,
    mut mouse: ResMut<crate::platform::keys::ScriptedMouse>,
    mut frame: Local<u32>,
    mut notches_left: Local<i32>,
) {
    const EACH: u32 = 16;
    if !options.unattended() {
        return;
    }
    *frame += 1;
    // The mouse's travel the run was told (`--mouse`): the bindings hear
    // it on the next frame in place of the real mouse's, which a run
    // nobody is at the keys of never listens to.
    let travel = options.mouse.iter().filter(|(from, frames, _)| (*from..from + frames).contains(&*frame)).map(|(.., counts)| *counts).sum();
    mouse.0 = Some(travel);
    // The wheel's notches the run was told to turn (`--wheel`), up
    // counted as positive: the menu's while the character plays. While
    // the screen is open they are the engine's own events for a wheel, a
    // notch a frame from then on, which the screen hears as it hears a
    // hand's: the crate's panel is scrolled if the mouse is over it (a
    // `--drag` from a place onto itself puts it there).
    let Ok((entity, mut window)) = window.single_mut() else { return };
    // The scope zoom's turns (`--zoom`): the simulation's next frame.
    for (_, notches) in options.zoom.iter().filter(|(at, _)| *frame == *at) {
        controller.zoom_notches += notches;
    }
    for (_, notches) in options.wheel.iter().filter(|(at, _)| *frame == *at) {
        if controller.inventory_open || controller.edit_build {
            *notches_left += notches;
        } else {
            controller.menu_notches += notches;
        }
    }
    // The mouse's own wheel (`--scroll`): the engine's events, which the
    // bindings hear whatever is shown.
    for (_, notches) in options.scroll.iter().filter(|(at, _)| *frame == *at) {
        *notches_left += notches;
    }
    if *notches_left != 0 {
        let notch = notches_left.signum();
        wheel.write(bevy::input::mouse::MouseWheel { unit: bevy::input::mouse::MouseScrollUnit::Line, x: 0.0, y: notch as f32, window: entity });
        *notches_left -= notch;
    }
    // The keys the run was told to press (`--tap`), by the same count.
    let mut button = |button: MouseButton, state: bevy::input::ButtonState| {
        buttons.write(bevy::input::mouse::MouseButtonInput { button, state, window: entity });
    };
    for (at, key, held) in &options.taps {
        let (down, up) = (*frame == *at, *frame == at + held);
        // A letter, a figure, the space, Backspace or Enter pressed is also
        // what it types, as a keyboard's key is (for a text field), and so
        // are the keys that move a field's caret.
        if let (true, crate::platform::keys::Key::Board(code)) = (down, key) {
            use bevy::input::keyboard::{Key, KeyboardInput};
            let debug = format!("{code:?}");
            let letter = debug.strip_prefix("Key").or_else(|| debug.strip_prefix("Digit")).filter(|rest| rest.len() == 1).map(str::to_lowercase).or_else(|| (*code == KeyCode::Space).then(|| " ".to_owned()));
            let logical = match (code, &letter) {
                (KeyCode::Backspace, _) => Some(Key::Backspace),
                (KeyCode::Enter, _) => Some(Key::Enter),
                (KeyCode::Delete, _) => Some(Key::Delete),
                (KeyCode::PageUp, _) => Some(Key::PageUp),
                (KeyCode::PageDown, _) => Some(Key::PageDown),
                (KeyCode::Home, _) => Some(Key::Home),
                (KeyCode::End, _) => Some(Key::End),
                (KeyCode::ArrowLeft, _) => Some(Key::ArrowLeft),
                (KeyCode::ArrowRight, _) => Some(Key::ArrowRight),
                (KeyCode::ArrowUp, _) => Some(Key::ArrowUp),
                (KeyCode::ArrowDown, _) => Some(Key::ArrowDown),
                (_, Some(letter)) => Some(Key::Character(letter.as_str().into())),
                _ => None,
            };
            if let Some(logical_key) = logical {
                typed.write(KeyboardInput { key_code: *code, logical_key, state: bevy::input::ButtonState::Pressed, text: letter.map(|letter| letter.as_str().into()), repeat: false, window: entity });
            }
        }
        match key {
            crate::platform::keys::Key::Board(key) if down => keys.press(*key),
            crate::platform::keys::Key::Board(key) if up => keys.release(*key),
            crate::platform::keys::Key::Mouse(pressed) if down => button(*pressed, bevy::input::ButtonState::Pressed),
            crate::platform::keys::Key::Mouse(pressed) if up => button(*pressed, bevy::input::ButtonState::Released),
            _ => {}
        }
    }
    // A move of the mouse with no button (`--move`).
    if let Some(&(_, x, y)) = options.moves.iter().find(|(at, ..)| *at == *frame) {
        window.set_cursor_position(Some(Vec2::new(x, y)));
    }
    let Some(since) = frame.checked_sub(options.drag_at) else { return };
    let Some([x, y, to_x, to_y]) = options.drags.get((since / EACH) as usize).copied() else { return };
    match since % EACH {
        0 => window.set_cursor_position(Some(Vec2::new(x, y))),
        4 => button(MouseButton::Left, bevy::input::ButtonState::Pressed),
        6 => window.set_cursor_position(Some(Vec2::new((x + to_x) / 2.0, (y + to_y) / 2.0))),
        8 => window.set_cursor_position(Some(Vec2::new(to_x, to_y))),
        12 => button(MouseButton::Left, bevy::input::ButtonState::Released),
        _ => {}
    }
}
