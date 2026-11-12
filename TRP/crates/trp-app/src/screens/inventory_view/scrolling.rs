//! The game's scroll views and the inspect window's scrolling texts.

use super::*;

/// How far the game's scroll views (`TARKOV UI`) are scrolled, in the
/// game's units from their tops, and how fast each springs back where it is
/// elastic: the containers' column (`COLUMN`), kept while the screen is shut
/// (the game's keeps its place: its `AutoZeroing` is off), and the panel of
/// a container opened (`CRATE`), at its top as another container opens, so
/// that a container always starts at the top.
#[derive(Resource, Default)]
pub struct GameScrolls {
    pub(super) at: [f32; 2],
    pub(super) speed: [f32; 2],
    /// Each window that shows a thing's description, by its opening
    /// (`Win::opened`): at its top as it opens (a new window from the
    /// template), kept while it is open.
    pub(super) info: HashMap<u64, f32>,
}

pub(super) const COLUMN: usize = 0;

pub(super) const CRATE: usize = 1;

/// A game's scroll view as drawn (`TARKOV UI`), where the wheel scrolls it:
/// which (`GameScrolls`), how high what scrolls is and how high it shows,
/// the corner on the screen of the panel its bar is placed in and that
/// panel's size, and the top of what moves, unscrolled, in the mask's
/// frame; all in units. With them its grids, each with its rows, from what
/// moves' corner, for what of them is in sight (`LootInSight`).
#[derive(Component)]
pub(super) struct ScrollWheel {
    pub(super) kind: usize,
    pub(super) whole: f32,
    pub(super) shown: f32,
    pub(super) frame: [f32; 2],
    pub(super) size: [f32; 2],
    pub(super) content_top: f32,
    pub(super) grids: Vec<([f32; 4], u32)>,
}

/// What of a scroll view moves as it is scrolled; its bar; its bar's
/// handle with the handle's own colour.
#[derive(Component)]
pub(super) struct ScrollContent(pub(super) usize);

#[derive(Component)]
pub(super) struct ScrollBarNode(pub(super) usize);

#[derive(Component)]
pub(super) struct ScrollHandle(pub(super) usize, pub(super) Color);

/// A scroll bar held with the left button: where it was pressed (in the
/// units of the panel it is placed in), whether the mouse has moved far
/// enough for a drag and, once it has, where on the handle it was taken
/// (from the handle's middle, up), and whether it was pressed on this
/// frame.
#[derive(Clone, Copy)]
pub(super) struct BarHeld {
    pub(super) from: [f32; 2],
    pub(super) dragging: Option<f32>,
    pub(super) first: bool,
}

/// What the scroll views keep between frames: each one's bar held, chosen
/// and its handle's tint, and the container last opened.
#[derive(Default)]
pub(super) struct ScrollState {
    pub(super) held: [Option<BarHeld>; 2],
    pub(super) chosen: [bool; 2],
    fade: [Fade; 2],
    last_crate: Option<usize>,
}

/// A tint changing over the bar's own time: from, to, how long since it
/// began, and the tint now.
#[derive(Clone, Copy, Default)]
pub(super) struct Fade {
    pub(super) from: [f32; 4],
    pub(super) to: [f32; 4],
    pub(super) since: f32,
    pub(super) now: Option<[f32; 4]>,
}

/// The game's scroll views (`TARKOV UI`: the containers' column and the
/// panel of a container opened) are scrolled as the game's
/// (`ScrollRectNoDrag` and its `Scrollbar`): by the wheel over the view, so
/// many units a notch (`m_ScrollSensitivity`), while its bar shows
/// (`ScrollRectNoDrag.OnScroll`); by the bar's handle held and moved once
/// the mouse has gone as far as a drag takes, the handle kept under the
/// mouse where it was taken (`Scrollbar.OnBeginDrag`, `UpdateDrag`); and by
/// the bar held elsewhere, a handle's length toward where it was pressed on
/// every frame until the handle is under it, twice on the frame it is
/// pressed (`Scrollbar.ClickRepeat`, which waits for each frame's end and so
/// runs again at the end of the first). A view that stops at its ends
/// (`MovementType.Clamped`) is held to them; an elastic one is let past and
/// springs back over its elasticity, three times as long on a frame it was
/// scrolled (`ScrollRect.LateUpdate`, `Mathf.SmoothDamp`). The handle is
/// tinted as the bar is idle, under the mouse, held or chosen (pressed,
/// until something else is), the tint going over linearly in the bar's own
/// time (`Selectable.DoStateTransition`, `Graphic.CrossFadeColor`). The
/// rows of the opened container's grids in sight are kept for its pictures
/// (`LootInSight`, as the game's `GridViewMagnifier` makes the views in
/// sight alone).
#[allow(clippy::too_many_arguments)]
pub(super) fn scroll_views(
    time: Res<Time>,
    controller: Res<Controller>,
    carried: Res<Carried>,
    look: Res<crate::screens::health_view::HealthLook>,
    wheel: Res<bevy::input::mouse::AccumulatedMouseScroll>,
    buttons: Res<ButtonInput<MouseButton>>,
    window: Query<&Window, With<PrimaryWindow>>,
    units: Res<Units>,
    mut scroll: ResMut<GameScrolls>,
    mut in_sight: ResMut<LootInSight>,
    covers: Query<(&Cover, &RelativeCursorPosition)>,
    areas: Query<(&ScrollWheel, &RelativeCursorPosition)>,
    bars: Query<(&ScrollBarNode, &RelativeCursorPosition)>,
    mut contents: Query<(&ScrollContent, &mut Node), Without<ScrollHandle>>,
    mut handles: Query<(&ScrollHandle, &mut Node, &mut BackgroundColor), Without<ScrollContent>>,
    (mut views, mut state): (Query<(&ItemView, &mut Visibility)>, Local<ScrollState>),
) {
    let ScrollState { held, chosen, fade, last_crate } = &mut *state;
    let showing = (controller.inventory_open && carried.loot_open).then_some(carried.loot_of);
    if *last_crate != showing {
        *last_crate = showing;
        scroll.at[CRATE] = 0.0;
        scroll.speed[CRATE] = 0.0;
    }
    if !controller.inventory_open {
        *held = [None, None];
        return;
    }
    let s = units.0.max(1e-3);
    let on_page = cover_under(covers.iter()) == 0;
    let cursor_px = window.single().ok().and_then(|window| window.cursor_position());
    for (sizes, over_view) in &areas {
        let kind = sizes.kind;
        let source = match kind {
            COLUMN => look.containers.as_ref().and_then(|views| Some((views.scrolling?, &views.panel))),
            _ => look.stash.as_ref().and_then(|views| Some((views.scrolling?, &views.panel))),
        };
        let Some((scrolling, panel)) = source else { continue };
        let most = (sizes.whole - sizes.shown).max(0.0);
        let shows = !scrolling.hides || sizes.whole > sizes.shown + 0.01;
        let mut to = scroll.at[kind];
        let mut wheeled = false;
        if over_view.cursor_over() && on_page && shows && scrolling.vertical {
            // A notch reads one; the engine's scroll
            // event is down for positive, the content's place up.
            let notches = match wheel.unit {
                bevy::input::mouse::MouseScrollUnit::Line => wheel.delta.y,
                bevy::input::mouse::MouseScrollUnit::Pixel => wheel.delta.y.signum(),
            };
            if notches != 0.0 {
                to -= notches * scrolling.sensitivity;
                wheeled = true;
            }
        }
        let clamp = |at: f32| if scrolling.clamped { at.clamp(0.0, most) } else { at };
        to = clamp(to);
        let share = if sizes.whole > 0.0 { (sizes.shown / sizes.whole).clamp(0.0, 1.0) } else { 1.0 };
        let value_of = |at: f32| if most > 0.0 { 1.0 - at / most } else { 1.0 };
        let at_of = |value: f32| (1.0 - value.clamp(0.0, 1.0)) * most;
        let cursor = cursor_px.map(|at| [at.x / s - sizes.frame[0], at.y / s - sizes.frame[1]]);
        let over_bar = bars.iter().any(|(bar, over)| bar.0 == kind && over.cursor_over()) && on_page;
        if let Some(bar_look) = scrolling.bar.filter(|_| shows) {
            let handle_at = |at: f32| bar_look.handle_in(panel, sizes.size, value_of(at), share);
            if buttons.just_pressed(MouseButton::Left) {
                // A press elsewhere chooses something else, or nothing.
                chosen[kind] = over_bar;
                held[kind] = cursor.filter(|_| over_bar).map(|from| BarHeld { from, dragging: None, first: true });
            }
            if !buttons.pressed(MouseButton::Left) {
                held[kind] = None;
            }
            let sliding = bar_look.sliding.map(|sliding| panel.rect(sliding, sizes.size));
            if let (Some(mut press), Some(cursor), Some([_, sy, _, sh])) = (held[kind], cursor, sliding) {
                // A drag begins once the mouse is so far from where it was
                // pressed, the handle taken where the press was on it, or
                // by its middle.
                let moved = Vec2::new(cursor[0] - press.from[0], cursor[1] - press.from[1]).length() * s;
                if press.dragging.is_none() && moved >= DRAG_THRESHOLD {
                    let offset = handle_at(to).filter(|&rect| inside(rect, press.from)).map_or(0.0, |[_, hy, _, hh]| (hy + hh / 2.0) - press.from[1]);
                    press.dragging = Some(offset);
                }
                match press.dragging {
                    Some(offset) => {
                        if let Some(value) = dragged_to([sy, sh], share, offset, cursor[1], bar_look.bottom_up) {
                            to = clamp(at_of(value));
                        }
                    }
                    None => {
                        for _ in 0..if press.first { 2 } else { 1 } {
                            let Some(value) = handle_at(to).and_then(|handle| stepped(value_of(to), share, handle, press.from, bar_look.bottom_up)) else { break };
                            to = clamp(at_of(value));
                        }
                    }
                }
                press.first = false;
                held[kind] = Some(press);
            }
            // The handle where the bar puts it, and tinted.
            let tints = bar_look.tints.filter(|_| bar_look.tinted == bar_look.handle);
            let wanted = tints.map(|tints| {
                let tint = if held[kind].is_some() {
                    tints.pressed
                } else if chosen[kind] {
                    tints.selected
                } else if over_bar {
                    tints.highlighted
                } else {
                    tints.normal
                };
                (tint.map(|channel| channel * tints.multiplier), tints.fade)
            });
            let fade = &mut fade[kind];
            if let Some((wanted, length)) = wanted {
                let now = fade.now.unwrap_or(wanted);
                if fade.now.is_none() || fade.to != wanted {
                    *fade = Fade { from: now, to: wanted, since: 0.0, now: Some(now) };
                }
                fade.since += time.delta_secs();
                let share = if length > 0.0 { (fade.since / length).min(1.0) } else { 1.0 };
                let (from, target) = (fade.from, fade.to);
                fade.now = Some(std::array::from_fn(|channel| from[channel] + (target[channel] - from[channel]) * share));
            }
            for (own, mut node, mut color) in handles.iter_mut().filter(|(own, ..)| own.0 == kind) {
                let Some([hx, hy, hw, hh]) = handle_at(to) else { continue };
                let place = [Val::Px(hx * s), Val::Px(hy * s), Val::Px(hw * s), Val::Px(hh * s)];
                if [node.left, node.top, node.width, node.height] != place {
                    [node.left, node.top, node.width, node.height] = place;
                }
                let tint = fade.now.unwrap_or([1.0; 4]);
                let base = own.1.to_srgba();
                let tinted = Color::srgba(base.red * tint[0], base.green * tint[1], base.blue * tint[2], base.alpha * tint[3]);
                if color.0 != tinted {
                    color.0 = tinted;
                }
            }
        } else {
            held[kind] = None;
        }
        // Back from past an end, where it is elastic.
        if scrolling.elastic {
            let target = to.clamp(0.0, most);
            let mut speed = scroll.speed[kind];
            if target != to {
                let length = scrolling.elasticity * if wheeled { 3.0 } else { 1.0 };
                to = smooth_damp(to, target, &mut speed, length, time.delta_secs());
                if speed.abs() < 1.0 {
                    speed = 0.0;
                }
            } else {
                speed = 0.0;
            }
            scroll.speed[kind] = speed;
        }
        if scroll.at[kind] != to {
            scroll.at[kind] = to;
        }
        for (_, mut node) in contents.iter_mut().filter(|(own, _)| own.0 == kind) {
            let top = Val::Px((sizes.content_top - to) * s);
            if node.top != top {
                node.top = top;
            }
        }
        // The opened container's rows in sight, grid by grid, as the game's
        // panel finds them (`GridViewMagnifier`, `GridView.
        // MagnifyIfPossible`): the scroll view's rectangle in its first
        // grid's cells, its foot and its height each rounded to whole
        // cells; the things that take any of those rows have views, the
        // rest none, however much of them the mask would show (the game stops
        // drawing a thing when most of it is outside the scroll window). The other grids are not
        // magnified: all their rows.
        if kind == CRATE {
            let until = to + sizes.shown;
            let rows: Vec<[u32; 2]> = sizes
                .grids
                .iter()
                .enumerate()
                .map(|(grid, &([_, top, _, _], rows))| match grid {
                    0 => magnified_rows(until - top, sizes.shown, rows),
                    _ => [0, rows],
                })
                .collect();
            if in_sight.0 != rows {
                in_sight.0 = rows;
            }
            // The first grid's things out of those rows not drawn.
            if let (Some(&[first, end]), Some(grid)) = (in_sight.0.first(), carried.loot.grids.first()) {
                for (view, mut shown) in &mut views {
                    let Some((place, item)) = grid.iter().find(|(_, item)| item.id == view.0).filter(|_| view.1 == 0) else { continue };
                    let [w, h] = item.cells();
                    let high = if place.turned { w } else { h };
                    let wanted = if place.y < end && place.y + high > first { Visibility::Inherited } else { Visibility::Hidden };
                    if *shown != wanted {
                        *shown = wanted;
                    }
                }
            }
        }
    }
}

/// The rows of a magnified grid that have views (`GridView.
/// MagnifyIfPossible`): the scroll view's foot, so far below the grid's
/// top, and its height, in units, each rounded to whole cells; the rows
/// between, within the grid's.
pub(super) fn magnified_rows(foot: f32, shown: f32, rows: u32) -> [u32; 2] {
    let foot = (foot / CELL).round();
    let first = (foot - (shown / CELL).round()).clamp(0.0, rows as f32);
    [first as u32, foot.clamp(first, rows as f32) as u32]
}

/// A window that shows a thing: its description's scroll view's area, with
/// the window's opening and layer, and the view as built; and its bar.
#[derive(Component)]
pub(super) struct InfoScrollArea {
    pub(super) opened: u64,
    pub(super) layer: u8,
    pub(super) scroll: crate::screens::health_view::InfoScroll,
}

#[derive(Component)]
pub(super) struct InfoScrollBar(pub(super) u64);

/// The bar held, whether it is chosen, and its handle's tint, of each
/// window's description, by its opening.
#[derive(Default)]
pub(super) struct InfoScrollState {
    pub(super) held: Option<BarHeld>,
    pub(super) chosen: bool,
    fade: Fade,
}

/// The windows' descriptions are scrolled as the game's
/// (`DescriptionPanel/Scroll Area`: `ScrollRectNoDrag`, inspect-window
/// spec, "The description"), as `scroll_views` scrolls the screen's own: by
/// the wheel over the area on the window on top there, so many units a
/// notch (10), only while the bar shows; by the handle dragged; by the bar
/// held off it, a handle's length toward the press each frame. Elastic
/// with an elasticity of nought, it is back within its ends on the same
/// frame (`SmoothDamp` over nought), so it stops at them; no inertia. The
/// handle is tinted as the bar's state is. A new window begins at the top;
/// a shut one's place is let go.
#[allow(clippy::too_many_arguments)]
pub(super) fn scroll_info_texts(
    time: Res<Time>,
    controller: Res<Controller>,
    carried: Res<Carried>,
    wheel: Res<bevy::input::mouse::AccumulatedMouseScroll>,
    buttons: Res<ButtonInput<MouseButton>>,
    units: Res<Units>,
    mut scroll: ResMut<GameScrolls>,
    (covers, areas, bars): (Query<(&Cover, &RelativeCursorPosition)>, Query<(&InfoScrollArea, &RelativeCursorPosition)>, Query<(&InfoScrollBar, &RelativeCursorPosition)>),
    mut contents: Query<(&crate::screens::health_view::InfoScrollContent, &mut Node), Without<crate::screens::health_view::InfoScrollHandle>>,
    mut handles: Query<(&crate::screens::health_view::InfoScrollHandle, &mut Node, &mut BackgroundColor), Without<crate::screens::health_view::InfoScrollContent>>,
    mut state: Local<HashMap<u64, InfoScrollState>>,
) {
    let open = |opened: &u64| carried.windows.iter().any(|window| window.opened == *opened);
    scroll.info.retain(|opened, _| open(opened));
    state.retain(|opened, _| open(opened));
    if !windows_up(&controller) {
        state.values_mut().for_each(|each| each.held = None);
        return;
    }
    let s = units.0.max(1e-3);
    let top = cover_under(covers.iter());
    for (area, over_view) in &areas {
        let (opened, info) = (area.opened, &area.scroll);
        let InfoScrollState { held, chosen, fade } = state.entry(opened).or_default();
        let on_top = top == area.layer;
        let mut to = scroll.info.get(&opened).copied().unwrap_or(0.0);
        if over_view.cursor_over() && on_top && info.vertical {
            let notches = match wheel.unit {
                bevy::input::mouse::MouseScrollUnit::Line => wheel.delta.y,
                bevy::input::mouse::MouseScrollUnit::Pixel => wheel.delta.y.signum(),
            };
            to -= notches * info.sensitivity;
        }
        let most = info.most();
        to = to.clamp(0.0, most);
        let share = info.share();
        // The mouse in the window's units, by where it is on the bar.
        let on_bar = bars.iter().find(|(bar, _)| bar.0 == opened).map(|(_, at)| at);
        let over_bar = on_top && on_bar.is_some_and(|at| at.cursor_over());
        let cursor = on_bar.and_then(share_of).map(|share| [info.bar[0] + share.x * info.bar[2], info.bar[1] + share.y * info.bar[3]]);
        if buttons.just_pressed(MouseButton::Left) {
            *chosen = over_bar;
            *held = cursor.filter(|_| over_bar).map(|from| BarHeld { from, dragging: None, first: true });
        }
        if !buttons.pressed(MouseButton::Left) {
            *held = None;
        }
        if let (Some(mut press), Some(cursor)) = (*held, cursor) {
            let [_, sy, _, sh] = info.sliding;
            let moved = Vec2::new(cursor[0] - press.from[0], cursor[1] - press.from[1]).length() * s;
            if press.dragging.is_none() && moved >= DRAG_THRESHOLD {
                let rect = info.handle_at(to);
                let offset = if inside(rect, press.from) { (rect[1] + rect[3] / 2.0) - press.from[1] } else { 0.0 };
                press.dragging = Some(offset);
            }
            match press.dragging {
                Some(offset) => {
                    if let Some(value) = dragged_to([sy, sh], share, offset, cursor[1], info.bottom_up()) {
                        to = info.at_of(value).clamp(0.0, most);
                    }
                }
                None => {
                    for _ in 0..if press.first { 2 } else { 1 } {
                        let Some(value) = stepped(info.value_of(to), share, info.handle_at(to), press.from, info.bottom_up()) else { break };
                        to = info.at_of(value).clamp(0.0, most);
                    }
                }
            }
            press.first = false;
            *held = Some(press);
        }
        // The handle tinted as the bar is idle, under the mouse, held or
        // chosen, over the tint's own time.
        if let Some(tints) = info.tints {
            let tint = if held.is_some() {
                tints.pressed
            } else if *chosen {
                tints.selected
            } else if over_bar {
                tints.highlighted
            } else {
                tints.normal
            };
            let wanted = tint.map(|channel| channel * tints.multiplier);
            let now = fade.now.unwrap_or(wanted);
            if fade.now.is_none() || fade.to != wanted {
                *fade = Fade { from: now, to: wanted, since: 0.0, now: Some(now) };
            }
            fade.since += time.delta_secs();
            let share = if tints.fade > 0.0 { (fade.since / tints.fade).min(1.0) } else { 1.0 };
            let (from, target) = (fade.from, fade.to);
            fade.now = Some(std::array::from_fn(|channel| from[channel] + (target[channel] - from[channel]) * share));
        }
        if scroll.info.get(&opened) != Some(&to) {
            scroll.info.insert(opened, to);
        }
        for (own, mut node, mut color) in handles.iter_mut().filter(|(own, ..)| own.0 == opened) {
            let [hx, hy, hw, hh] = info.handle_at(to);
            let place = [Val::Px(hx * s), Val::Px(hy * s), Val::Px(hw * s), Val::Px(hh * s)];
            if [node.left, node.top, node.width, node.height] != place {
                [node.left, node.top, node.width, node.height] = place;
            }
            let tint = fade.now.unwrap_or([1.0; 4]);
            let base = own.1.to_srgba();
            let tinted = Color::srgba(base.red * tint[0], base.green * tint[1], base.blue * tint[2], base.alpha * tint[3]);
            if color.0 != tinted {
                color.0 = tinted;
            }
        }
        for (_, mut node) in contents.iter_mut().filter(|(own, _)| own.0 == opened) {
            let top = Val::Px(-to * s);
            if node.top != top {
                node.top = top;
            }
        }
    }
}
