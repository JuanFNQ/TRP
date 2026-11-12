//! Clicks on the screen and its parts, and the tooltip.

use super::*;

/// A press on the screen's buttons: a slot's dropdown opened or shut, a
/// part chosen (`ToggleSelection`: the build changed, the dropdown shut,
/// the game's install sound of the slot's class: `PROVISIONAL`), a toggle,
/// ASSEMBLE, BACK.
#[allow(clippy::too_many_arguments)]
pub(super) fn click(
    mut commands: Commands,
    mut state: ResMut<EditBuild>,
    pressed: Query<(&Interaction, &Press), Changed<Interaction>>,
    lines: Query<(&Interaction, &Press)>,
    mut carried: ResMut<Carried>,
    mut sounds: MessageWriter<Sound>,
    time: Res<Time>,
) {
    if !state.open || state.mouse_taken {
        return;
    }
    let state = &mut *state;
    // A part the mouse comes onto in a dropdown is put in the compared
    // build's same slot (`ModdingSelectableItemContext.Highlight`,
    // `OnCompareRequired`), and as the mouse leaves it the slot is given
    // what the build has in it (`ResetCompare`): the build compared with is
    // the build as it stands with the part under the mouse in its slot.
    if pressed.iter().any(|(_, press)| matches!(press, Press::Pick(..))) {
        let under = lines.iter().find_map(|(interaction, press)| match press {
            Press::Pick(slot, kind) if *interaction != Interaction::None => Some((*slot, kind.clone())),
            _ => None,
        });
        if under.as_ref().map(|(slot, kind)| (slot, kind.as_ref().map(|kind| &kind.template))) != state.compare.as_ref().map(|(slot, kind)| (slot, kind.as_ref().map(|kind| &kind.template))) {
            state.compare = under;
            state.compare_made += 1;
        }
    }
    // (A dropdown shut by a click elsewhere leaves nothing under the mouse.)
    if state.compare.as_ref().is_some_and(|(slot, _)| state.menu != Some(*slot)) {
        state.compare = None;
        state.compare_made += 1;
    }
    let Some(press) = pressed.iter().find(|(interaction, _)| **interaction == Interaction::Pressed).map(|(_, press)| press.clone()) else { return };
    // While the model of a pick is made, no slot is had (`place`): a press
    // on one drawn before is not heard.
    let refreshing = state.preview.key.as_deref() != Some(state.build_key.as_str());
    if refreshing && matches!(press, Press::Open(_) | Press::Pick(..)) {
        return;
    }
    match press {
        Press::Open(id) => {
            // (A locked slot's never opens: it has no button.)
            if state.slots.iter().any(|slot| (slot.owner, slot.index) == id && slot.locked) {
                return;
            }
            // (`DropDownButtonClickHandler`: open shuts it; another's opens
            // in its place.)
            state.menu = if state.menu == Some(id) { None } else { Some(id) };
            state.menu_seen = true;
            sounds.write(Sound::Ui(Ui::MenuDropdown));
        }
        Press::Shut => {
            state.menu = None;
            state.menu_seen = true;
        }
        Press::Pick((owner, index), kind) => {
            let Some(slot) = state.slots.iter().find(|slot| (slot.owner, slot.index) == (owner, index)).cloned() else { return };
            // A part the slot would refuse is not picked: the game's
            // warning says why, and the dropdown stays open
            // (`ItemObserveScreen.ToggleSelection`, `Select` failed:
            // `DisplaySingletonWarningNotification`).
            let refused = kind.as_ref().zip(state.build.as_ref()).and_then(|(kind, build)| rules::refusal(build, slot.owner, slot.index, kind));
            if let Some(refusal) = refused {
                let said = crate::screens::inventory_view::why(&refusal, &carried.texts);
                info!("the edit build screen: refused, {said}");
                state.notice = Some((said, time.elapsed_secs() + NOTICE_FOR));
                return;
            }
            if let Some(build) = state.build.as_mut() {
                rules::select(build, slot.owner, slot.index, kind.as_ref());
                state.build_key = key_of(build, state.gear);
                info!("the edit build screen: {} picked for its slot {}", kind.as_ref().map_or("nothing", |kind| kind.name.as_str()), slot.index);
                state.made += 1;
            }
            let sound = match slot.class {
                ModClass::Vital => Ui::InstallModVital,
                ModClass::Gear => Ui::InstallModGear,
                ModClass::Functional => Ui::InstallModFunc,
            };
            sounds.write(Sound::Ui(sound));
            state.menu = None;
            state.menu_seen = true;
            // (The dropdown shut, nothing is under the mouse in it.)
            state.compare = None;
            state.compare_made += 1;
        }
        Press::Toggle(index) => {
            state.toggles[index] = !state.toggles[index];
        }
        Press::Info => {
            state.stats_folded = !state.stats_folded;
        }
        Press::Assemble => {
            let (Some(id), Some(build)) = (state.weapon, state.build.clone()) else { return };
            // A build with a vital part missing is not built (the app's rule).
            if vital_missing(&build) {
                return;
            }
            let Some(weapon) = carried.equipment.item.find_mut(id) else { return };
            if !built(weapon, &build, state.gear) {
                let gone = rules::assemble(weapon, &build);
                state.made += 1;
                state.built = true;
                sounds.write(Sound::Ui(Ui::MenuWeaponAssemble));
                info!("the {} built anew at a bench: {} parts gone", if state.gear { "gear worn" } else { "weapon in hand" }, gone.len());
            }
            // (The game's notice is a weapon's: none for gear, whose screen
            // in the game, its modding screen, puts a part on as it is
            // picked and says nothing.)
            if !state.gear {
                let said = state.texts.get(&BUILT.to_lowercase()).cloned().unwrap_or_else(|| BUILT.to_owned());
                state.notice = Some((said, time.elapsed_secs() + NOTICE_FOR));
            }
        }
        Press::Back => shut(state, &mut commands, &mut sounds),
    }
}

/// Two presses of the left button on one part within this many seconds are
/// a double click (the engine's own, `StandaloneInputModule`, as the
/// inventory's: `inventory_view::DOUBLE_CLICK`).
const DOUBLE_CLICK: f64 = 0.3;

/// A part's view's own clicks (`ModdingSelectableItemView.OnClick`): the
/// right button let go on one, in a slot's view or a dropdown, opens the
/// game's menu of a right click where the mouse is (`ShowContextMenu`) with
/// one line, INSPECT; a double click of the left on a
/// slot's part, which picks nothing (`CanSelect` off), inspects it. Any
/// button let go shuts the menu (`SimpleContextMenu.Update`), and one let go
/// on INSPECT opens the part's window over the screen, a copy of it shown
/// (`inventory_view::inspect_observed`).
#[allow(clippy::too_many_arguments)]
pub(super) fn part_clicks(
    mut state: ResMut<EditBuild>,
    mut carried: ResMut<Carried>,
    mut menu: ResMut<crate::screens::item_menu_view::ItemMenu>,
    buttons: Res<ButtonInput<MouseButton>>,
    said: Res<Words>,
    time: Res<Time>,
    window: Query<&Window, With<PrimaryWindow>>,
    views: Query<(&PartView, &RelativeCursorPosition, &InheritedVisibility, Has<Press>), Without<PartLocked>>,
    windows: Query<&RelativeCursorPosition, With<crate::screens::inventory_view::WinRoot>>,
    mut sounds: MessageWriter<Sound>,
) {
    let state = &mut *state;
    let (true, Ok(window)) = (state.open, window.single()) else {
        state.menu_part = None;
        return;
    };
    let Some(cursor) = window.cursor_position() else { return };
    let screen = Some((Vec2::new(window.width(), window.height()), window.height() / HIGH));
    // (Shown as a copy with numbers of its own, the build's parts having
    // those of the weapon in hand's; the same part's copy again where it
    // is still shown, whose window is then raised.)
    let inspect = |state: &mut EditBuild, carried: &mut Carried, part: Item, sounds: &mut MessageWriter<Sound>| {
        info!("the edit build screen: {} inspected", part.kind.name);
        let copy = match state.observed.get(&part.id).and_then(|copy| carried.observed.iter().find(|shown| shown.id == *copy)) {
            Some(shown) => shown.clone(),
            None => {
                let copy = part.renumbered();
                state.observed.insert(part.id, copy.id);
                copy
            }
        };
        if let Some(sound) = crate::screens::inventory_view::inspect_observed(carried, copy, cursor, screen) {
            sounds.write(sound);
        }
    };
    // The menu up: any button let go shuts it, doing its line if on it.
    if let Some(part) = state.menu_part.as_ref() {
        if menu.0.is_none() {
            state.menu_part = None;
        } else if buttons.get_just_released().next().is_some() {
            let chosen = menu.0.take().and_then(|open| open.hover.and_then(|index| open.lines.get(index).cloned()));
            let part = part.clone();
            state.menu_part = None;
            if let Some((trp_sim::items::item_menu::Line::Inspect, Ok(()), _)) = chosen {
                inspect(state, &mut carried, part, &mut sounds);
            }
            return;
        }
        return;
    }
    if windows.iter().any(RelativeCursorPosition::cursor_over) || state.dragging {
        return;
    }
    let menu_open = state.menu.is_some();
    let under = views
        .iter()
        .filter(|(_, at, shown, in_menu)| shown.get() && at.cursor_over() && (*in_menu || !menu_open))
        .max_by_key(|(.., in_menu)| *in_menu)
        .map(|(view, _, _, in_menu)| (view.0.clone(), in_menu));
    let Some((part, in_menu)) = under else {
        if buttons.just_pressed(MouseButton::Left) {
            state.clicked = None;
        }
        return;
    };
    if buttons.just_released(MouseButton::Right) {
        let line = trp_sim::items::item_menu::Line::Inspect;
        menu.0 = Some(crate::screens::item_menu_view::OpenMenu { id: part.id, lines: vec![(line, Ok(()), said.word(line.name(), line.name()))], cursor, hover: None, sub: None, button: None });
        state.menu_part = Some(part);
        return;
    }
    // (A dropdown's part is picked by a click: `CanSelect`.)
    if in_menu {
        return;
    }
    // The press counts the clicks; the second's letting go is the double
    // click (`OnClick`, `doubleClick`).
    if buttons.just_pressed(MouseButton::Left) {
        let now = time.elapsed_secs_f64();
        let count = state.clicked.filter(|(id, at, _)| *id == part.id && now - at < DOUBLE_CLICK).map_or(1, |(.., count)| count + 1);
        state.clicked = Some((part.id, now, count));
    }
    if buttons.just_released(MouseButton::Left) && state.clicked.is_some_and(|(id, _, count)| id == part.id && count % 2 == 0) {
        inspect(state, &mut carried, part, &mut sounds);
    }
}

/// A part's name while the mouse rests on its view (`GridItemView.
/// OnPointerEnter`, `ShowTooltip`: its name as the locale has it,
/// `BriefItemName` changing only a weapon's), after `TIP_DELAY`, in the
/// game's simple tooltip (`crate::tooltip`: following the mouse, kept on
/// the screen). None while the preview is turned, a menu is up or a window
/// is over the mouse; while a dropdown is open, only for its parts.
pub(super) fn tip(
    state: Res<EditBuild>,
    carried: Res<Carried>,
    menu: Res<crate::screens::item_menu_view::ItemMenu>,
    views: Query<(Entity, &PartView, &RelativeCursorPosition, &InheritedVisibility, Has<Press>, Option<&PartRefused>)>,
    windows: Query<&RelativeCursorPosition, With<crate::screens::inventory_view::WinRoot>>,
    mut tooltip: ResMut<crate::ui::tooltip::SimpleTip>,
) {
    let free = state.open && !state.dragging && menu.0.is_none() && !windows.iter().any(RelativeCursorPosition::cursor_over);
    let menu_open = state.menu.is_some();
    let over = views
        .iter()
        .filter(|(_, _, at, shown, in_menu, _)| free && shown.get() && at.cursor_over() && (*in_menu || !menu_open))
        .max_by_key(|(_, _, _, _, in_menu, _)| *in_menu)
        .map(|(entity, view, _, _, _, refused)| (entity, (view, refused)));
    let Some((entity, (view, refused))) = over else { return };
    // A part refused shows its reason at once (`SelectableItemView.
    // OnPointerEnter`: `Tooltip.Show(_tooltipText)`, no delay); any other
    // its name after the delay.
    let (said, delay) = match refused {
        Some(PartRefused(reason)) => (reason.clone(), 0.0),
        None => (carried.brief_name(&view.0, carried.texts.names.get(&view.0.kind.template).filter(|name| !name.is_empty()).cloned().unwrap_or_else(|| view.0.kind.name.clone())), TIP_DELAY),
    };
    tooltip.show(("part", entity.to_bits()), said, delay);
}
