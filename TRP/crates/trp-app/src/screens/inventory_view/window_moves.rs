//! Windows handled each frame, bench asks and the published drag.

use super::*;

/// The windows over the page: the cross shuts one, a press on one brings
/// it to the top (and makes a container's the one Ctrl and a click sends
/// things to, as the install's `PriorityWindowMode` has it), and held by
/// its caption it is moved. They all shut with the screen
/// (`ItemUiContext.CloseAllWindows`).
pub(super) fn windows(
    buttons: Res<ButtonInput<MouseButton>>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    window: Query<&Window, With<PrimaryWindow>>,
    controller: Res<Controller>,
    mut carried: ResMut<Carried>,
    mut clicks: ResMut<Clicks>,
    captions: Query<(&WinCaption, &RelativeCursorPosition)>,
    mut roots: Query<(&WinRoot, &mut Node, &RelativeCursorPosition, &ComputedNode)>,
    close: Query<(&Interaction, &WinClose), Changed<Interaction>>,
    mut sounds: MessageWriter<Sound>,
) {
    if !windows_up(&controller) {
        clicks.moving = None;
        if !carried.observed.is_empty() {
            carried.observed.clear();
        }
        if !carried.windows.is_empty() || carried.priority.is_some() {
            // Each window that shows a thing sounds its closing as it
            // shuts (`InfoWindow.Close`).
            for _ in carried.windows.iter().filter(|window| window.kind == WinKind::Info) {
                sounds.write(Sound::Ui(Ui::InspectorClose));
            }
            carried.windows.clear();
            carried.priority = None;
        }
        return;
    }
    // Its cross shuts a window; Escape shuts the one on top
    // (`Window.TranslateCommand`), one a press.
    let crossed = close.iter().find(|(interaction, _)| **interaction == Interaction::Pressed).map(|(_, close)| close.0);
    let escaped = (keys.just_pressed(KeyCode::Escape) && !carried.windows.is_empty()).then(|| carried.windows.len() - 1);
    if let Some(index) = crossed.or(escaped) {
        if index < carried.windows.len() {
            // Escape sounds its own (`Window.TranslateCommand`); a window
            // that shows a thing its closing besides.
            if crossed.is_none() {
                sounds.write(Sound::Ui(Ui::MenuEscape));
                clicks.escape_taken = true;
            }
            if matches!(carried.windows[index].kind, WinKind::Info | WinKind::Repair) {
                sounds.write(Sound::Ui(Ui::InspectorClose));
            }
            carried.shut_window(index);
            carried.tidy();
            // Over the edit build screen, the Escape that shut the window
            // does not shut the screen too (`Window.TranslateCommand` takes it).
            if crossed.is_none() && controller.edit_build {
                keys.clear_just_pressed(KeyCode::Escape);
            }
        }
        clicks.moving = None;
        clicks.window_press = None;
        return;
    }
    let Ok(screen) = window.single() else { return };
    let Some(cursor) = screen.cursor_position() else { return };
    let room = Vec2::new(screen.width(), screen.height());
    // A press on the window on top where the mouse is.
    if carried.asking.is_none() && buttons.just_pressed(MouseButton::Left) {
        let on = roots.iter().filter(|(.., at, _)| at.cursor_over()).map(|(root, ..)| root.0).max();
        clicks.window_press = on.filter(|index| *index < carried.windows.len()).map(|index| WindowPress {
            index,
            from: cursor,
            by_caption: captions.iter().any(|(caption, at)| caption.0 == index && at.cursor_over()),
            thing: false,
            dragged: false,
        });
    }
    if let Some(mut press) = clicks.window_press {
        // (A thing pressed is had from the frame after the press: `drag`
        // runs after this.)
        press.thing |= clicks.pressed.is_some();
        if buttons.pressed(MouseButton::Left) {
            // A drag begun on the window, not from a thing in it, picks it
            // (`OnBeginDrag`); begun on its caption's bar it moves it, held
            // where the mouse holds it as the drag begins (`UIDragComponent`).
            if !press.thing && !press.dragged && cursor.distance(press.from) >= DRAG_THRESHOLD {
                press.dragged = true;
                carried.raise(press.index);
                let top = carried.windows.len() - 1;
                if press.by_caption {
                    clicks.moving = Some((top, cursor - carried.windows[top].at));
                }
            }
            clicks.window_press = Some(press);
        } else {
            // Let go on it with no drag and not on a thing: a click, which
            // picks it (`OnPointerClick`).
            let over = roots.iter().filter(|(.., at, _)| at.cursor_over()).map(|(root, ..)| root.0).max();
            if !press.thing && !press.dragged && over == Some(press.index) {
                carried.raise(press.index);
            }
            clicks.window_press = None;
        }
    }
    if !buttons.pressed(MouseButton::Left) {
        clicks.moving = None;
    }
    // Moved: where it is drawn goes with the mouse at once, kept on the
    // screen and at whole pixels (`CorrectPositionResolution`), and where it
    // is kept is changed without the page being drawn afresh for it.
    if let Some((index, hold)) = clicks.moving {
        let size = roots.iter().find(|(root, ..)| root.0 == index).map_or(Vec2::ZERO, |(.., computed)| computed.size() * computed.inverse_scale_factor());
        let at = on_screen(cursor - hold, size, room);
        for (root, mut node, ..) in &mut roots {
            if root.0 == index {
                (node.left, node.top) = (Val::Px(at.x), Val::Px(at.y));
            }
        }
        if let Some(window) = carried.bypass_change_detection().windows.get_mut(index) {
            window.at = at;
        }
    }
}

/// A window's top left corner kept on a screen so large
/// (`RectTransformExtensions.CorrectPositionResolution`): a left edge off
/// the screen moved right, else a right edge moved left; a bottom edge off
/// it moved up, else a top edge moved down (so a window higher than the
/// screen keeps its bottom on it); then at whole pixels (`Math.Round`: to
/// the even one at a half).
pub(super) fn on_screen(at: Vec2, size: Vec2, room: Vec2) -> Vec2 {
    let mut at = at;
    if at.x < 0.0 {
        at.x = 0.0;
    } else if at.x + size.x > room.x {
        at.x = room.x - size.x;
    }
    if at.y + size.y > room.y {
        at.y = room.y - size.y;
    } else if at.y < 0.0 {
        at.y = 0.0;
    }
    Vec2::new(at.x.round_ties_even(), at.y.round_ties_even())
}

/// A weapon a quick-use key asks for, for `settle` to take in hand; and an
/// item to be used, with the part it is for (let go on that part of the
/// HEALTH tab, or the one the inventory's "Use" names; none, the common
/// body) and food's or drink's share of its whole (none, what is left).
/// And whether the quick stroke was asked for (`QuickKnife`).
/// And the workbench's asks (`BenchAsk`).
#[derive(Resource, Default)]
pub struct AskedFor(pub(super) Option<Id>, pub(super) Option<(Id, Option<trp_sim::condition::health::Part>, Option<f32>)>, pub(super) bool, pub(super) BenchAsk);

impl AskedFor {
    /// What the workbench asks of the hands.
    pub fn bench(&mut self) -> &mut BenchAsk {
        &mut self.3
    }
}

/// The workbench's asks of the hands: the weapon in hand put
/// away as its screen opens, the hands left empty while it stands open,
/// and that weapon taken in hand again as it shuts, by its slot as a key
/// takes it; until it is, the ask stands (a rig still being made for the
/// build is waited for). With a change built (`rack`) it is drawn racking,
/// for the look of it alone, changing nothing (`Firearm::rack_drawn`, no
/// round moved).
#[derive(Default, Clone, Copy)]
pub struct BenchAsk {
    pub put_away: bool,
    pub take: Option<Id>,
    pub rack: bool,
}

/// The grenade the grenade's own key takes (`ThrowGrenade`; grenades-and-
/// knife spec, "Taking them in hand"): of the grenades lying directly in
/// the vest and then the pockets, the first by its kind's order
/// (`GetThrowablePriorityGrenadesList`: frag, flash, stun, smoke, gas,
/// incendiary, sonar), which is what the quick bar's grenade view chooses
/// unless its selector chose another. Within a container, by grid and then
/// by row and column (`PROVISIONAL`: the game takes a grid's own order of
/// its items, and sorts with a sort that keeps no order among equals).
pub(super) fn top_grenade(carried: &Carried, db: &trp_data::Database) -> Option<Id> {
    const KINDS: [&str; 7] = ["frag_grenade", "flash_grenade", "stun_grenade", "smoke_grenade", "gas_grenade", "incendiary_grenade", "sonar"];
    let mut found: Vec<(usize, Id)> = Vec::new();
    for slot in ["TacticalVest", "Pockets"] {
        let Some(container) = carried.equipment.worn(slot) else { continue };
        for grid in &container.grids {
            // (Only grenades known: `PlayerInputTranslator`'s `Examined`.)
            let mut lying: Vec<&(Place, Item)> = grid.iter().filter(|(_, item)| is(item, "ThrowWeap") && trp_sim::items::examine::known(item)).collect();
            lying.sort_by_key(|(place, _)| (place.y, place.x));
            for (_, item) in lying {
                let kind = db.items.get(&item.kind.template).and_then(|template| template.props["ThrowType"].as_str()).and_then(|kind| KINDS.iter().position(|k| *k == kind)).unwrap_or(KINDS.len());
                found.push((kind, item.id));
            }
        }
    }
    found.sort_by_key(|(kind, _)| *kind);
    found.first().map(|(_, id)| *id)
}

/// The thing the mouse carries in the screen, if any (`Drag`), for what
/// lights up under it (the HEALTH tab's parts: `health_view`).
#[derive(Resource, Default)]
pub struct Dragging(pub Option<Id>);

pub(super) fn publish_drag(drag: Res<Drag>, mut dragging: ResMut<Dragging>) {
    let now = drag.0.as_ref().map(|held| held.id);
    if dragging.0 != now {
        dragging.0 = now;
    }
}
