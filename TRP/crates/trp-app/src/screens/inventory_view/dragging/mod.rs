//! Dragging a thing between grids, slots and windows.

use super::*;

mod letting_go;
mod menu_clicks;

/// The mouse on the things of the screen (`ItemView`, `GridView` and
/// `SlotView` in the game). The button pressed and let go on a thing is a
/// click (`click`). Moved far enough with the button down, the mouse
/// takes the thing, carries it and lets it go: over another thing it is
/// dropped on that, over a grid it goes to the cell its top left corner
/// is nearest, over a slot into the slot. `R` turns it a quarter while it
/// is held. With Ctrl held as it is let go only part of a stack goes, and
/// where more than one may, a window asks how many (`ask`).
pub(super) fn drag(
    time: Res<Time<Real>>,
    (buttons, keys): (Res<ButtonInput<MouseButton>>, Res<ButtonInput<KeyCode>>),
    bindings: Res<Bindings>,
    window: Query<&Window, With<PrimaryWindow>>,
    controller: Res<Controller>,
    mut carried: ResMut<Carried>,
    mut drag: ResMut<Drag>,
    mut clicks: ResMut<Clicks>,
    units: Res<Units>,
    blinking: Res<Blinking>,
    camera: Query<&GlobalTransform, With<crate::weapons::weapon::Eye>>,
    (items, slots, grids, covers, info_buttons): (
        Query<(&ItemView, &RelativeCursorPosition, &ComputedNode)>,
        Query<(&SlotBox, &RelativeCursorPosition)>,
        Query<(&GridBox, &RelativeCursorPosition, &ComputedNode, Option<&bevy::ui::CalculatedClip>)>,
        Query<(&Cover, &RelativeCursorPosition)>,
        Query<(&InfoButtonHit, &RelativeCursorPosition)>,
    ),
    mut ghost: Query<(&mut Node, &mut Visibility, &mut BorderColor, &mut BackgroundColor), With<Ghost>>,
    (mut ghost_words, mut ghost_picture, mut drop_shown): (Query<&mut Text, With<GhostWords>>, Query<(&mut ImageNode, &mut Visibility), (With<GhostPicture>, Without<Ghost>)>, ResMut<DropShown>),
    (mut asked, game, drops, mut sounds, mut menu, in_hands, said, mut feedback, mut tip): (
        ResMut<AskedFor>,
        Res<crate::GameData>,
        Query<(&crate::screens::health_view::HealthDrop, &RelativeCursorPosition, &InheritedVisibility)>,
        MessageWriter<Sound>,
        ResMut<crate::screens::item_menu_view::ItemMenu>,
        Res<HandsHeld>,
        Res<crate::screens::health_view::Words>,
        ResMut<crate::weapons::wear::Feedback>,
        ResMut<crate::ui::tooltip::SimpleTip>,
    ),
) {
    let _clock = crate::platform::render_timing::Clock::new("inventory drag");
    let Ok((mut node, mut shown, mut border, mut ground)) = ghost.single_mut() else { return };
    // Nothing marked unless a thing is held over a place this frame.
    if *drop_shown != DropShown::default() {
        *drop_shown = DropShown::default();
    }
    let cell = units.cell();
    let cursor = window.single().ok().and_then(Window::cursor_position);
    // Nothing is taken with the screen shut, while the window that asks
    // how many is up, or while a window is being moved.
    let free = controller.inventory_open && carried.asking.is_none() && clicks.moving.is_none() && clicks.stretching.is_none() && !clicks.sliding;
    let (true, Some(cursor)) = (free, cursor) else {
        (drag.0, clicks.pressed) = (None, None);
        *shown = Visibility::Hidden;
        if !windows_up(&controller) && menu.0.is_some() {
            menu.0 = None;
        }
        return;
    };
    let cover = cover_under(covers.iter());
    let under = |but: Option<Id>| item_under(items.iter(), cover, but, &blinking);
    let held_key = |left: KeyCode, right: KeyCode| keys.pressed(left) || keys.pressed(right);
    let (ctrl, shift, alt) = (held_key(KeyCode::ControlLeft, KeyCode::ControlRight), held_key(KeyCode::ShiftLeft, KeyCode::ShiftRight), held_key(KeyCode::AltLeft, KeyCode::AltRight));
    let left = MouseButton::Left;
    let screen = window.single().ok().map(|window| (Vec2::new(window.width(), window.height()), units.0));
    let ended = menu_clicks::menus_and_clicks(
        menu_clicks::Changed { carried: carried.reborrow(), drag: drag.reborrow(), clicks: clicks.reborrow(), menu: menu.reborrow(), sounds: &mut sounds, asked: asked.reborrow(), feedback: feedback.reborrow() },
        menu_clicks::Read { time: &time, buttons: &buttons, controller: &controller, game: &game, in_hands: &in_hands, said: &said },
        menu_clicks::Mouse { cursor, screen, cover, mods: [ctrl, shift, alt] },
        &camera,
        &info_buttons,
        &under,
    );
    if ended {
        return;
    }
    // Moved far enough with the button down: the thing is taken.
    let scale = window.single().map_or(1.0, Window::scale_factor);
    if let Some(pressed) = clicks.pressed.filter(|pressed| pressed.cursor.distance(cursor) * scale >= DRAG_THRESHOLD) {
        clicks.pressed = None;
        if let (Some(item), Some((_, spot))) = (carried.find(pressed.id), carried.lies(pressed.id)) {
            // In a grid the mouse holds it where it took hold of it;
            // out of a slot, by the middle of its first cell.
            let (turned, grab) = match spot {
                Spot::Cell(_, place) => {
                    let [w, h] = item.cells();
                    let size = if place.turned { Vec2::new(h as f32, w as f32) } else { Vec2::new(w as f32, h as f32) };
                    (place.turned, pressed.at * size * cell)
                }
                _ => (false, Vec2::splat(cell / 2.0)),
            };
            drag.0 = Some(Held { id: pressed.id, turned, grab, cells: item.cells(), name: carried.name(item), layer: cover, corner: Vec2::ZERO, moved: None });
            // Taken up: its group's sound (`ItemView.OnBeginDrag`).
            sounds.write(Sound::Item(item.kind.sound.clone(), Way::Pickup));
        }
    }
    // The key for it puts the thing held, or the one under the mouse,
    // down on the ground a step ahead of the character (the game's
    // `ThrowItem`, which a raid carries out without asking; the game
    // throws the thing and it falls where it falls).
    if bindings.fired("ThrowItem") {
        let which = drag.0.as_ref().map(|held| held.id).or_else(|| under(None).map(|(id, _)| id));
        // A backpack on the back is not put down at once: its slot is
        // one the game animates, and it goes by the body's gesture for
        // it, as it does by its own key (interactions spec, "A backpack
        // dropped").
        if let Some(id) = which {
            put_down(&mut carried, id, camera.single().ok(), controller.movement.position[1]);
            (drag.0, clicks.pressed) = (None, None);
        }
    }
    let Some(held) = drag.0.as_mut() else {
        *shown = Visibility::Hidden;
        return;
    };
    // Its picture's lower left corner is kept as the mouse moves it, on the
    // frame the drag begins and on each the mouse moves while the button is
    // down (`DraggedItemView.OnDrag`: `ItemContext.SetPosition`, its middle
    // at the mouse less half its size, 63 units a cell and one); not on the
    // frame it is let go (`StandaloneInputModule`: the release is heard
    // before the move). A move on the frame of `R` is kept at the size
    // before the turn (`PROVISIONAL`: which of the event system and the
    // held picture is updated first is not read).
    if buttons.pressed(left) && held.moved != Some(cursor) {
        let [w, h] = if held.turned { [held.cells[1], held.cells[0]] } else { held.cells };
        let half = (Vec2::new(w as f32, h as f32) * CELL + Vec2::ONE) * units.0 / 2.0;
        (held.corner, held.moved) = (Vec2::new(cursor.x - half.x, cursor.y + half.y), Some(cursor));
    }
    // `R` turns it a quarter, whatever its shape (`DraggedItemView.
    // RotateByInput`: a square thing's picture turns too). Its picture is
    // put in the middle again, but its corner is not kept anew
    // (`RotateItem`): until the mouse moves, a grid takes it from the corner
    // before the turn with the size after it (`courses10`'s `drag-and-drop`,
    // frame 2686: the M4A1 turned back lit at the backpack's (2, 4), and
    // (0, 2) once the mouse moved at 2713; frame 2926, the bottle turned
    // and let go unmoved, put at (0, 1)).
    if keys.just_pressed(KeyCode::KeyR) {
        held.turned = !held.turned;
    }
    let [w, h] = if held.turned { [held.cells[1], held.cells[0]] } else { held.cells };
    // It hangs by its middle (`DraggedItemView.SetInCenter`: its pivot at
    // the mouse).
    held.grab = Vec2::new(w as f32, h as f32) * cell / 2.0;
    // Where it would go if let go now, on what is drawn on top there, and
    // the grid's top left corner on the screen.
    let grid_under = || grids.iter().find(|(grid, at, ..)| grid.layer == cover && at.cursor_over());
    let on_cell = || {
        let (grid, at, ..) = grid_under()?;
        let origin = cursor - share_of(at)? * Vec2::new(grid.cells[0] as f32, grid.cells[1] as f32) * cell;
        let (x, y) = grid_location(held.corner, origin, cell, h, grid.cells);
        Some(Target::Cell(grid.holder, grid.grid, Place { x, y, turned: held.turned }))
    };
    let on_slot = || slots.iter().find(|(slot, at)| slot.layer == cover && at.cursor_over()).map(|(slot, _)| Target::Slot(slot.holder, slot.slot));
    let target = under(Some(held.id)).map(|(onto, _)| Target::Item(onto)).or_else(on_cell).or_else(on_slot);
    // Tried on copies first, as the game tries a move before it shows
    // whether the place will take the thing. With Ctrl held it is part of
    // a stack that would go, and one is tried.
    let would = target.map(|target| {
        let (mut worn, mut loot) = (carried.equipment.item.clone(), carried.loot.clone());
        act(&mut [&mut worn, &mut loot], held.id, target, &carried.rules, ctrl.then_some(1))
    });
    // Rounds held over a magazine that would take none: why (`Magazine.
    // Apply`: the wrong round, or full). A load that would go through
    // keeps the refused colour, as the game's highlight has it
    // (`GridView.GetHighlightColor` knows no `LoadMagOperationResult`).
    let onto = match target {
        Some(Target::Item(onto)) => Some(onto),
        Some(Target::Slot(holder, slot)) => carried.find(holder).and_then(|holder| holder.slots.get(slot)?.as_ref().map(|item| item.id)),
        _ => None,
    };
    let unloaded = onto.and_then(|onto| carried.find(held.id).zip(carried.find(onto))).and_then(|(rounds, magazine)| trp_sim::items::inventory::rounds_onto_magazine(magazine, rounds)).and_then(Result::err);
    let refusal = unloaded.or_else(|| would.as_ref().and_then(|would| would.as_ref().err().cloned())).filter(|_| buttons.pressed(left));
    *shown = Visibility::Inherited;
    // Its own picture, as large as its cells (`DraggedItemView.RotateItem`:
    // 63 units a cell and one, `GetCellPixelSize`), turned with it, white
    // (a thing examined: every thing here is); else the app's box with its
    // name, while its picture is still to come.
    let picture = carried.find(held.id).and_then(|item| carried.pictures.get(&shown_key(&carried, item)));
    let (show_picture, words_now) = match picture {
        Some(picture) => {
            let size = (Vec2::new(w as f32, h as f32) * CELL + Vec2::ONE) * units.0;
            (node.left, node.top, node.width, node.height) = (Val::Px(cursor.x - size.x / 2.0), Val::Px(cursor.y - size.y / 2.0), Val::Px(size.x), Val::Px(size.y));
            (node.border, node.padding) = (UiRect::ZERO, UiRect::ZERO);
            *border = BorderColor::all(Color::NONE);
            ground.0 = Color::NONE;
            if let Ok((mut image, _)) = ghost_picture.single_mut() {
                let wanted = if held.turned { &picture.turned } else { &picture.upright };
                if image.image != *wanted {
                    image.image = wanted.clone();
                }
                // A thing not known, black at 0.85 (`DraggedItemView`).
                let tint = if carried.find(held.id).is_some_and(|item| !trp_sim::items::examine::known(item)) { Color::srgba(0.0, 0.0, 0.0, 0.85) } else { Color::WHITE };
                if image.color != tint {
                    image.color = tint;
                }
            }
            (true, String::new())
        }
        None => {
            (node.left, node.top) = (Val::Px(cursor.x - held.grab.x), Val::Px(cursor.y - held.grab.y));
            (node.width, node.height) = (Val::Px(w as f32 * cell - 2.0), Val::Px(h as f32 * cell - 2.0));
            (node.border, node.padding) = (UiRect::all(Val::Px(1.0)), UiRect::all(Val::Px(2.0)));
            *border = BorderColor::all(WORDS);
            ground.0 = THING.with_alpha(0.8);
            (false, held.name.clone())
        }
    };
    if let Ok((_, mut visibility)) = ghost_picture.single_mut() {
        let want = if show_picture { Visibility::Inherited } else { Visibility::Hidden };
        if *visibility != want {
            *visibility = want;
        }
    }
    if let Ok(mut text) = ghost_words.single_mut() {
        if **text != words_now {
            **text = words_now;
        }
    }
    // What a drop here would mark (`DraggedItemView.UpdateTargetUnderCursor`):
    // over a grid the cells it would take, from its corner, kept inside the
    // grid (`GridView.HighlightItemViewPosition`); over a slot, the slot;
    // each in the colour of what the drop would do: a grid's own colours,
    // a slot's its own (`slot_colour`).
    let colour = drop_colour(would.as_ref());
    if let (Some((grid, at, laid, clip)), Some(Target::Cell(_, _, place))) = (grid_under(), on_cell()) {
        if let Some(share) = share_of(at) {
            let size = laid.size() * laid.inverse_scale_factor();
            let corner = cursor - share * size;
            let (x0, y0) = (place.x.min(grid.cells[0]), place.y.min(grid.cells[1]));
            let (x1, y1) = ((place.x + w).min(grid.cells[0]), (place.y + h).min(grid.cells[1]));
            let (left, top) = (corner.x + x0 as f32 * cell, corner.y + y0 as f32 * cell);
            let mut lit = Rect::new(left, top, left + (x1 - x0) as f32 * cell, top + (y1 - y0) as f32 * cell);
            // Cut to what shows of the grid: the panel lies in the grid
            // under its scroll view's mask, so its cells past the view's
            // edge are hidden as the grid's are (the engine's clip is in
            // the screen's own pixels).
            if let Some(clip) = clip {
                let k = laid.inverse_scale_factor();
                lit = lit.intersect(Rect::from_corners(clip.clip.min * k, clip.clip.max * k));
            }
            if !lit.is_empty() {
                drop_shown.cells = Some(([lit.min.x, lit.min.y, lit.width(), lit.height()], colour));
            }
        }
        // A grid, or a thing in one, that refuses it says why at the
        // mouse where the game's grid does (`refusal_tip`).
        if let Some(words) = refusal.as_ref().and_then(|refusal| refusal_tip(refusal, &carried, Over::Grid)) {
            tip.show(("refused", onto.unwrap_or(grid.holder)), words, 0.0);
        }
    } else if let Some((slot, _)) = slots.iter().find(|(slot, at)| slot.layer == cover && at.cursor_over()) {
        drop_shown.slot = Some((slot.holder, slot.slot, slot_colour(would.as_ref())));
        // A round carried over a chamber it fits: that chamber known
        // (`SlotView.HighlightItemViewPosition`, not a preview:
        // `CheckChamber(weapon, true)`).
        if let Some(gun) = round_over_chamber(&carried, held.id, slot.holder, slot.slot).filter(|gun| !carried.checked_chambers.contains(gun)) {
            carried.know_chamber(gun, true);
        }
        // A slot that refuses it says why at the mouse, at once, until the
        // mouse leaves it or the thing is let go (`DisableHighlightSilent`).
        if let Some(words) = refusal.as_ref().and_then(|refusal| refusal_tip(refusal, &carried, Over::Slot)) {
            tip.show(("refused", slot.slot as u64), words, 0.0);
        }
    }
    if !buttons.pressed(left) {
        let (id, name) = (held.id, held.name.clone());
        drag.0 = None;
        *shown = Visibility::Hidden;
        letting_go::let_go(
            (id, name),
            target,
            ctrl,
            cursor,
            &drops,
            letting_go::Landing { carried: carried.reborrow(), asked: asked.reborrow(), sounds: &mut sounds, controller: &controller, game: &game, in_hands: &in_hands },
        );
    }
}
