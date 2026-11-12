//! Before a thing is taken: the right-click menu's lines, a window's buttons and the clicks on a thing.

use super::*;

/// What the menus and the clicks change.
pub(super) struct Changed<'a, 'w> {
    pub(super) carried: Mut<'a, Carried>,
    pub(super) drag: Mut<'a, Drag>,
    pub(super) clicks: Mut<'a, Clicks>,
    pub(super) menu: Mut<'a, crate::screens::item_menu_view::ItemMenu>,
    pub(super) sounds: &'a mut MessageWriter<'w, Sound>,
    pub(super) asked: Mut<'a, AskedFor>,
    pub(super) feedback: Mut<'a, crate::weapons::wear::Feedback>,
}

/// What the menus and the clicks read.
pub(super) struct Read<'a> {
    pub(super) time: &'a Time<Real>,
    pub(super) buttons: &'a ButtonInput<MouseButton>,
    pub(super) controller: &'a Controller,
    pub(super) game: &'a crate::GameData,
    pub(super) in_hands: &'a HandsHeld,
    pub(super) said: &'a crate::screens::health_view::Words,
}

/// Where the mouse is, the screen's size and scale, the cover on top under the mouse, and Ctrl, Shift and Alt held.
pub(super) struct Mouse {
    pub(super) cursor: Vec2,
    pub(super) screen: Option<(Vec2, f32)>,
    pub(super) cover: u8,
    pub(super) mods: [bool; 3],
}

/// The menu of a right click, a window's buttons, the middle and right buttons and a click on a thing,
/// in the order the drag hears them; true where the drag ends there for this frame.
pub(super) fn menus_and_clicks(
    changed: Changed,
    read: Read,
    mouse: Mouse,
    camera: &Query<&GlobalTransform, With<crate::weapons::weapon::Eye>>,
    info_buttons: &Query<(&InfoButtonHit, &RelativeCursorPosition)>,
    under: &dyn Fn(Option<Id>) -> Option<(Id, Vec2)>,
) -> bool {
    let Changed { mut carried, mut drag, mut clicks, mut menu, sounds, mut asked, mut feedback } = changed;
    let Read { time, buttons, controller, game, in_hands, said } = read;
    let Mouse { cursor, screen, cover, mods: [ctrl, shift, alt] } = mouse;
    // A refused quick move's warning, among the notices (looked at before
    // it is taken: taking it marks what is carried changed).
    if carried.warning.is_some() {
        if let Some(words) = carried.warning.take() {
            feedback.warning(words);
        }
    }
    let left = MouseButton::Left;
    // The menu of a right click (inventory spec, "The menu of a right
    // click"): any button let go shuts it (`SimpleContextMenu.Update`),
    // and one let go over a line that can be done does that line first.
    if buttons.get_just_released().next().is_some() {
        if let Some(open) = menu.0.take() {
            // A line of the wish list's own (`WishlistContextInteractions.
            // ExecuteInteractionInternal`): the thing's kind put in a group,
            // moved to it, or taken off.
            if let Some((line, Ok(()), caption)) = open.sub.as_ref().and_then(|sub| sub.hover.and_then(|index| sub.lines.get(index).cloned())) {
                if let Some(item) = carried.find(open.id) {
                    let (template, name) = (item.kind.template.clone(), carried.name(item));
                    if trp_sim::items::item_menu::wish(&mut carried.wishlist, &template, line) {
                        carried.note = format!("{name}: wish list, {caption}");
                    }
                }
                (drag.0, clicks.pressed) = (None, None);
                return true;
            }
            if let Some((line, Ok(()), caption)) = open.hover.and_then(|index| open.lines.get(index).cloned()) {
                let sound = do_line(&mut carried, open.id, line, &caption, cursor, screen, controller.health.as_ref(), &game.db, &mut asked, camera.single().ok(), controller.movement.position[1], in_hands.item);
                if let Some(sound) = sound {
                    sounds.write(sound);
                }
                (drag.0, clicks.pressed) = (None, None);
                return true;
            }
        }
    }
    // A window's button's menu of lines goes with its window.
    if menu.0.as_ref().is_some_and(|open| open.button.is_some() && !carried.windows.iter().any(|window| window.kind == WinKind::Info && window.item == open.id)) {
        menu.0 = None;
    }
    // A button of a window that shows a thing the mouse comes onto, on top
    // where it is (`SimpleContextMenuButton.OnPointerEnter`, its `onHover`:
    // `CloseSubMenu`, then the line's own lines where it has some and can
    // be done, `CreateSubInteractions`): the wish list's lines open at once,
    // by the button (`courses10`'s `weapon-inspect-menu`, frame 1084, with
    // no click, not in a menu at the mouse).
    // Only as it comes onto it: let go there (the menu of lines shut by
    // `SimpleContextMenu.Update`), they open again only as the mouse comes
    // back.
    let over_button = info_buttons.iter().find(|(hit, at)| at.cursor_over() && hit.2 == cover).map(|(hit, _)| *hit);
    if over_button.as_ref().map(|hit| (hit.0, hit.1)) != clicks.info_hover {
        clicks.info_hover = over_button.as_ref().map(|hit| (hit.0, hit.1));
        if let Some(hit) = over_button {
            if menu.0.as_ref().is_some_and(|open| open.button.is_some()) {
                menu.0 = None;
            }
            if hit.1.has_lines() && menu.0.is_none() {
                let lines = menu_lines(&carried, hit.0, &in_hands, controller.health.as_ref(), &game.db, &said);
                let template = carried.find(hit.0).map(|item| item.kind.template.clone());
                if let (Some(found @ (_, Ok(()), _)), Some(template)) = (lines.into_iter().find(|(own, ..)| *own == hit.1), template) {
                    let sub = crate::screens::item_menu_view::SubMenu {
                        parent: 0,
                        lines: trp_sim::items::item_menu::wish_lines(&template, &carried.wishlist).into_iter().map(|(line, can)| (line, can, said.word(line.caption_key(), line.name()))).collect(),
                        hover: None,
                    };
                    let button = crate::screens::item_menu_view::WindowButton { rect: hit.3, piece: hit.4 };
                    let corner = Vec2::new(hit.3[0], hit.3[1]);
                    menu.0 = Some(crate::screens::item_menu_view::OpenMenu { id: hit.0, lines: vec![found], cursor: corner, hover: None, sub: Some(sub), button: Some(button) });
                }
            }
        }
    }
    // A button of a window that shows a thing, on top where the mouse is,
    // clicked: its line done (`InteractionButtonsContainer`'s button:
    // `ExecuteInteraction`), as the menu's; the wish list's does nothing
    // (`BaseItemContextInteractions`: `AddToWishlist` has only its lines).
    if drag.0.is_none() && clicks.pressed.is_none() && buttons.just_released(left) {
        if let Some(hit) = over_button {
            let lines = menu_lines(&carried, hit.0, &in_hands, controller.health.as_ref(), &game.db, &said);
            if let Some((line, Ok(()), caption)) = lines.iter().find(|(own, ..)| *own == hit.1).cloned() {
                if !line.has_lines() {
                    if let Some(sound) = do_line(&mut carried, hit.0, line, &caption, cursor, screen, controller.health.as_ref(), &game.db, &mut asked, camera.single().ok(), controller.movement.position[1], in_hands.item) {
                        sounds.write(sound);
                    }
                }
            }
            return true;
        }
    }
    // The right button let go on a thing opens its menu where the mouse
    // is (`ItemView.OnClick`, `ShowContextMenu`).
    // The middle button let go on a thing (`ItemView.ExecuteMiddleClick`):
    // the first of Examine, Fold, Unfold, Turn on and Turn off it can do,
    // else Check magazine; a line shown that cannot be done now says why,
    // once, and the next is tried (`ContextInteractions.ExecuteInteraction`).
    if drag.0.is_none() && buttons.just_released(MouseButton::Middle) {
        if let Some((id, _)) = under(None) {
            use trp_sim::items::item_menu::Line;
            let lines = menu_lines(&carried, id, &in_hands, controller.health.as_ref(), &game.db, &said);
            for wanted in [Line::Examine, Line::Fold, Line::Unfold, Line::TurnOn, Line::TurnOff, Line::CheckMagazine] {
                let Some((line, can, caption)) = lines.iter().find(|(line, ..)| *line == wanted).cloned() else { continue };
                match can {
                    Ok(()) => {
                        if let Some(sound) = do_line(&mut carried, id, line, &caption, cursor, screen, controller.health.as_ref(), &game.db, &mut asked, camera.single().ok(), controller.movement.position[1], in_hands.item) {
                            sounds.write(sound);
                        }
                        break;
                    }
                    Err(reason) if !reason.is_empty() => feedback.single_warning(said.word(reason, reason)),
                    Err(_) => {}
                }
            }
            return true;
        }
    }
    if drag.0.is_none() && buttons.just_released(MouseButton::Right) {
        if let Some((id, _)) = under(None) {
            let lines = menu_lines(&carried, id, &in_hands, controller.health.as_ref(), &game.db, &said);
            if !lines.is_empty() {
                menu.0 = Some(crate::screens::item_menu_view::OpenMenu { id, lines, cursor, hover: None, sub: None, button: None });
            }
            return true;
        }
    }
    // The button goes down on a thing: a click in the making, the second
    // of two on the same thing within the engine's time a double one.
    if drag.0.is_none() && buttons.just_pressed(left) {
        clicks.pressed = under(None).map(|(id, at)| Pressed { id, cursor, at });
        let now = time.elapsed_secs_f64();
        clicks.last = clicks.pressed.map(|pressed| {
            let again = clicks.last.filter(|last| last.id == pressed.id && now - last.at < DOUBLE_CLICK);
            Clicked { id: pressed.id, at: now, count: again.map_or(1, |last| last.count + 1) }
        });
    }
    // Let go where it went down: a click.
    if let Some(pressed) = clicks.pressed.filter(|_| !buttons.pressed(left)) {
        clicks.pressed = None;
        let double = clicks.last.is_some_and(|last| last.id == pressed.id && last.count % 2 == 0);
        // A plain click does nothing (`ItemView.OnClick`, but for a
        // magazine being worked): what is carried is not touched, so the
        // page is not drawn afresh (it was, and the boxes the mouse lights
        // went dark for a frame, the screen seeming to blink).
        let worked = carried.magazine_work.as_ref().is_some_and(|work| work.process.magazine == pressed.id);
        if !(double || ctrl || shift || alt || worked) {
            return true;
        }
        if double && !ctrl && !shift && use_thing(&mut carried, pressed.id, controller.health.as_ref(), &game.db, cursor, &mut asked, Using::DoubleClick) {
            return true;
        }
        // A thing sent by Ctrl is heard taken up, wherever it went
        // (`ItemView.OnClick`: its group's `pickup` after the move is run;
        // `courses10`'s `search-and-loot`, frames 751 and 882,
        // `container_metal pickup` and `item_money pickup` on the
        // release's frame); the menu's lines that send a thing so are
        // heard by the move's own (`ItemUiContext.RunWithSound`).
        let group = carried.find(pressed.id).map(|item| item.kind.sound.clone());
        if let Some(sound) = click(&mut carried, pressed.id, double, [ctrl, shift, alt], cursor, screen) {
            sounds.write(match group {
                Some(group) if ctrl => Sound::Item(group, Way::Pickup),
                _ => sound,
            });
        }
        return true;
    }
    false
}
