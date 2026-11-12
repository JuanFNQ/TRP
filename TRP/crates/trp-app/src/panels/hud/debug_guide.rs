//! The top panel spawned, hidden over screens, and the debug and guide panels.

use super::*;

/// A panel of the app's own, laid out as told: the inventory screen's
/// dark ground and its line about it.
pub(super) fn panel(layout: Node) -> impl Bundle {
    let node = Node { border: UiRect::all(Val::Px(1.0)), padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)), ..layout };
    (node, BackgroundColor(BACK), BorderColor::all(LINE))
}

/// A section's name in the panel, over a line.
fn section(parent: &mut ChildSpawnerCommands, name: &str) {
    let head = Node { margin: UiRect::top(Val::Px(4.0)), padding: UiRect::bottom(Val::Px(2.0)), border: UiRect::bottom(Val::Px(1.0)), ..default() };
    parent.spawn((head, BorderColor::all(LINE))).with_children(|head| {
        head.spawn(words(name, 10.0, FAINT));
    });
}

/// The panel at the top right: its head, then the frame rate, the debug
/// figures (each name over its figure, in rows) and the guide to the keys,
/// each under its section's name.
pub(super) fn spawn_top(mut commands: Commands, options: Res<Options>) {
    commands.insert_resource(GuideShown(options.show_fps));
    let tag = Node {
        position_type: PositionType::Absolute,
        right: Val::Px(16.0),
        top: Val::Px(12.0),
        padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)),
        display: if options.show_fps { Display::None } else { Display::Flex },
        ..default()
    };
    commands.spawn((DebugTag, tag, BackgroundColor(BACK.with_alpha(0.6)), GlobalZIndex(11))).with_children(|tag| {
        tag.spawn((words("PRESS ~ FOR DEBUG MENU", 10.0, FAINT), TextLayout::new_with_no_wrap()));
    });
    let popup = Node {
        position_type: PositionType::Absolute,
        right: Val::Px(16.0),
        top: Val::Px(12.0),
        flex_direction: FlexDirection::Column,
        row_gap: Val::Px(4.0),
        display: if options.show_fps { Display::Flex } else { Display::None },
        ..default()
    };
    commands.spawn((DebugPopup, panel(popup), GlobalZIndex(11))).with_children(|popup| {
        popup.spawn(Node { justify_content: JustifyContent::SpaceBetween, column_gap: Val::Px(24.0), ..default() }).with_children(|head| {
            head.spawn(words("DEBUG", 11.0, WORDS));
            head.spawn(words("~ CLOSES THIS", 10.0, FAINT));
        });
        section(popup, "PERFORMANCE");
        popup.spawn((FrameRateText, words("", 12.0, WORDS), TextLayout::new_with_no_wrap()));
        section(popup, "CHARACTER");
        popup.spawn(Node { column_gap: Val::Px(16.0), row_gap: Val::Px(6.0), flex_wrap: FlexWrap::Wrap, max_width: Val::Px(760.0), ..default() }).with_children(|row| {
            for (index, (name, room)) in DEBUG.into_iter().enumerate() {
                let cell = Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(1.0), min_width: Val::Px(room), ..default() };
                row.spawn((DebugCell(index), cell)).with_children(|cell| {
                    cell.spawn(words(name, 10.0, FAINT));
                    cell.spawn((DebugValue(index), words("", 12.0, WORDS), TextLayout::new_with_no_wrap()));
                });
            }
        });
        section(popup, "COMMANDS");
        crate::platform::debug_menu::spawn_commands(popup);
    });
}

/// What of the HUD the game's screen hides while it is open: the meters of
/// the stance, speed and stamina.
#[derive(Component)]
pub(super) struct ScreenHides;

/// The meters of the stance, speed and stamina, which the game's own
/// stance panel takes the place of with `TARKOV UI` (`battle_view`).
#[derive(Component)]
pub(super) struct PlainMeters;

/// With `TARKOV UI` the inventory screen is the game's, which shows
/// nothing of the app's under its left side: the meters are hidden while
/// it is open, and not drawn at all where the game's stance panel is.
pub(super) fn hide_over_screen(
    controller: Res<Controller>,
    features: Res<crate::screens::options_view::Features>,
    look: Res<crate::screens::health_view::HealthLook>,
    mut row: Query<(&mut Node, Has<PlainMeters>), With<ScreenHides>>,
) {
    for (mut node, meters) in &mut row {
        let display = if hidden_over_screen(&controller, &features, &look, meters) { Display::None } else { Display::Flex };
        if node.display != display {
            node.display = display;
        }
    }
}

/// Whether a row the screens hide is hidden now: under the inventory with
/// `TARKOV UI`, under the edit build screen, and the plain meters under
/// the battle screen's own panels. The meters are written by the same
/// rule (`meters::show`), so that a hidden row is not laid out afresh.
pub(super) fn hidden_over_screen(controller: &Controller, features: &crate::screens::options_view::Features, look: &crate::screens::health_view::HealthLook, meters: bool) -> bool {
    let battle = look.tarkov_ui && look.battle.is_some();
    (controller.inventory_open && features.tarkov_ui) || controller.edit_build || (meters && battle)
}

/// The debug panel's figures as they stand.
#[allow(clippy::too_many_arguments)]
pub fn debug(
    player: Res<ClipPlayer>,
    controller: Res<Controller>,
    options: Res<Options>,
    hands: Option<Res<HandsRig>>,
    mut figures: Query<(&DebugValue, &mut Text)>,
    mut cells: Query<(&DebugCell, &mut Node)>,
    shown: Option<Res<GuideShown>>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    // Written while the panel is shown, or as its key shows it (`guide` is
    // not ordered before this): a text written is measured and laid out
    // afresh whether it is seen or not.
    if !shown.is_some_and(|shown| shown.0) && !keys.just_pressed(PANEL_KEY) {
        return;
    }
    let m = &controller.movement;
    // With nothing in hand there is no weapon to speak of.
    let weapon = match hands.as_deref().and_then(|h| h.firearm.as_ref()).filter(|_| controller.armed) {
        Some(f) => {
            let rounds = f.magazine.map_or(String::from("no magazine"), |n| n.to_string());
            let aiming = if f.aiming() { "  aiming" } else { "" };
            let pushed = if controller.overlap > 0.0 { format!("  {:.0} cm into something", controller.overlap * 100.0) } else { String::new() };
            format!("{:?}  {rounds} + {}{aiming}{pushed}", f.mode(), f.chamber as u8)
        }
        None => String::from("empty hands"),
    };
    // Where the animator of the thing in hand is: its state, how far
    // through it, and the state it is on its way into.
    let animator = match (player.by_name, hands.as_deref().and_then(|h| h.animator())) {
        (true, _) => player.status().map_or(String::from("no clip"), |(name, at, len)| format!("{name}  {at:.2} / {len:.2} s")),
        (false, Some(animator)) => {
            let (state, at, next) = animator.state();
            let short = |name: &str| name.trim_start_matches("Hands.").to_owned();
            format!("{}  {at:.2}{}", short(state), next.map_or(String::new(), |next| format!("  ->  {}", short(next))))
        }
        (false, None) => String::new(),
    };
    // A script being played says where it has got to.
    let script = options.scenario.as_ref().map(|scenario| {
        let at = controller.drawn_time() - options.scenario_at;
        match at {
            at if at < 0.0 => format!("{} in {:.1} s", scenario.name, -at),
            at if at <= scenario.duration => format!("{}  {at:.2} / {:.2} s", scenario.name, scenario.duration),
            _ => format!("{} done", scenario.name),
        }
    });
    let said = [
        format!("{:?}{}", m.state, if m.grounded { "" } else { "  airborne" }),
        format!("{:.2} m/s", m.ground_speed()),
        format!("{:.2} / {:.2}", m.smoothed_speed, m.clamped_speed()),
        format!("{:.2}", m.smoothed_pose_level),
        format!("{:+.1}", m.smoothed_tilt),
        format!("{:.2}", m.sprint_speed),
        weapon,
        animator,
        script.clone().unwrap_or_default(),
    ];
    for (figure, mut text) in &mut figures {
        if **text != said[figure.0] {
            **text = said[figure.0].clone();
        }
    }
    // The script's cell is there only while a script is played.
    for (cell, mut node) in cells.iter_mut().filter(|(cell, _)| cell.0 == SCRIPT_CELL) {
        let display = if script.is_some() { Display::Flex } else { Display::None };
        if node.display != display {
            node.display = display;
        }
        let _ = cell;
    }
}

/// Keeps the panel at the top right: its key shows it and hides it, and it
/// stands back while the inventory screen or the edit build screen is open.
pub(super) fn guide(
    keys: Res<ButtonInput<KeyCode>>,
    controller: Res<Controller>,
    mut shown: ResMut<GuideShown>,
    mut popup: Query<&mut Node, (With<DebugPopup>, Without<DebugTag>)>,
    (look, window, mut tag): (Res<crate::screens::health_view::HealthLook>, Query<&Window, With<bevy::window::PrimaryWindow>>, Query<&mut Node, With<DebugTag>>),
) {
    let _clock = crate::platform::render_timing::Clock::new("hud guide");
    let Ok(mut node) = popup.single_mut() else { return };
    if keys.just_pressed(PANEL_KEY) {
        shown.0 = !shown.0;
    }
    let display = if shown.0 && !controller.inventory_open && !controller.edit_build { Display::Flex } else { Display::None };
    if node.display != display {
        node.display = display;
    }
    // Under the game's quick bar where it is drawn (`TARKOV UI`), 8 units
    // clear of it.
    let top = match (look.battle_quick.as_ref().filter(|_| look.tarkov_ui), window.single()) {
        (Some(quick), Ok(window)) => {
            let s = window.height() / 1080.0;
            let [_, y, _, h] = quick.quick_in([window.width() / s.max(1e-3), 1080.0]);
            Val::Px((y + h + 8.0) * s)
        }
        _ => Val::Px(12.0),
    };
    if node.top != top {
        node.top = top;
    }
    // The tag where the panel would be, while the panel is shut.
    for mut tag in &mut tag {
        let display = if !shown.0 && !controller.inventory_open && !controller.edit_build { Display::Flex } else { Display::None };
        if tag.display != display {
            tag.display = display;
        }
        if tag.top != top {
            tag.top = top;
        }
    }
}

/// Whether the panel is open over play now (shown, and no screen over it):
/// the mouse is then the panel's (`cursor::hold`, `keys::hear`).
pub fn panel_open(shown: Option<&GuideShown>, controller: &Controller) -> bool {
    shown.is_some_and(|shown| shown.0) && !controller.inventory_open && !controller.edit_build
}

pub(super) fn label(text: &str) -> impl Bundle {
    (Text::new(text), TextFont { font_size: 11.0, ..default() }, TextColor(LABEL))
}
