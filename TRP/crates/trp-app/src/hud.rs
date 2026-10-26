//! The stance, speed and stamina display at the bottom left, showing what
//! the game's own panel there shows (`docs/spec/stamina.md`, "The game's
//! own display"): how high the character stands, the speed setting over a
//! row of notches lit up to the movement state's limit, and how full the
//! legs and the arms are. The drawing is plain bars; the game's art for
//! it is not read.
//!
//! And, at the top right, a panel of the app's own in the inventory
//! screen's colours (the user, 2026-10-16: "Let's move the debug corner to
//! the top right of the display, combine all information into an
//! organized pop-up. (Debug/FPS information included only show when f1 is
//! pressed, hidden when closed)"): **the frame rate**, **the debug
//! figures** (what the movement and the thing in hand are doing just now)
//! and **the commands** (`debug_menu`; the guide to the keys that stood
//! there went, the user, 2026-10-26: "get rid of all the controls"; the
//! keys are the options' CONTROLS tab's). `F1` shows it and hides it, the
//! mouse freed while it is shown; hidden as a run begins (`--fps on`
//! starts with it shown), and while the inventory screen is open.

use bevy::prelude::*;

use crate::animation::ClipPlayer;
use crate::controller::Controller;
use crate::inventory_view::{words, BACK, FAINT, LINE, WORDS};

use crate::weapon::HandsRig;
use crate::Options;

/// The game's panel has a notch every twentieth of the top speed and one
/// at the top.
const NOTCHES: usize = 21;
const BAR_WIDTH: f32 = 168.0;
const NOTCH_WIDTH: f32 = 6.0;
const STANCE_HEIGHT: f32 = 64.0;

const TRACK: Color = Color::srgba(0.0, 0.0, 0.0, 0.55);
const LEGS: Color = Color::srgb(0.80, 0.86, 0.78);
const ARMS: Color = Color::srgb(0.86, 0.80, 0.62);
const SPENT: Color = Color::srgb(0.82, 0.27, 0.20);
const SET: Color = Color::srgb(0.92, 0.92, 0.88);
const ALLOWED: Color = Color::srgba(0.92, 0.92, 0.88, 0.32);
const REFUSED: Color = Color::srgba(0.0, 0.0, 0.0, 0.55);
const LABEL: Color = Color::srgba(0.92, 0.92, 0.88, 0.8);

/// A bar that fills along its length.
#[derive(Component)]
enum Fill {
    Legs,
    Arms,
    Stance,
}

/// One notch of the speed row, counted from the slow end.
#[derive(Component)]
struct Notch(usize);

/// The line that says the stance in words.
#[derive(Component)]
struct StanceWord;

/// The frame-rate readout, in the panel at the top right.
#[derive(Component)]
struct FrameRateText;

/// What the readout is worked out from: the last second of drawn frames,
/// newest last, each with how long it took by the engine's clock and what
/// the simulation cost in it, in seconds.
#[derive(Resource, Default)]
struct FrameTimes {
    frames: std::collections::VecDeque<(f32, f32)>,
    /// Seconds since the readout was last written.
    since: f32,
}

/// How much time the readout sums up, and how often it is rewritten, in
/// seconds: often enough to follow, seldom enough to read.
const FRAME_WINDOW: f32 = 1.0;
const FRAME_REFRESH: f32 = 0.25;

/// The debug panel's figures, each under a name of its own, with the
/// least room it is given so that the row does not shift as figures
/// change: the movement state, the speed over the ground, the speed
/// setting as smoothed and as limited, the stance, the lean, the sprint's
/// pace, the weapon (its fire mode and rounds), the state of the animator
/// of the thing in hand, and a script being played.
const DEBUG: [(&str, f32); 9] =
    [("STATE", 92.0), ("SPEED", 66.0), ("SETTING", 70.0), ("STANCE", 48.0), ("LEAN", 40.0), ("SPRINT", 48.0), ("WEAPON", 120.0), ("HANDS", 150.0), ("SCRIPT", 0.0)];
const SCRIPT_CELL: usize = 8;

/// A figure of the debug panel, and the cell it stands in, by its place.
#[derive(Component)]
pub struct DebugValue(usize);
#[derive(Component)]
pub struct DebugCell(usize);

/// The panel at the top right, the guide to the keys in it, and whether
/// the player wants it shown.
#[derive(Component)]
struct DebugPopup;
/// The small tag at the top right that says how to open it, while it is
/// shut (the user, 2026-10-16: "Add "Press f1 for debug menu" a small tag
/// on the top right").
#[derive(Component)]
struct DebugTag;
#[derive(Resource)]
pub struct GuideShown(pub bool);

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FrameTimes>()
            .add_systems(Startup, (spawn, spawn_top))
            // The guide after a capture's own presses are in, which come
            // before the weapon's clips: a key pressed by hand is in sooner.
            .add_systems(Update, (show, frame_rate, guide.after(crate::animation::play_clips), hide_over_screen));
    }
}

/// A panel of the app's own, laid out as told: the inventory screen's
/// dark ground and its line about it.
fn panel(layout: Node) -> impl Bundle {
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
fn spawn_top(mut commands: Commands, options: Res<Options>) {
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
        tag.spawn((words("PRESS F1 FOR DEBUG MENU", 10.0, FAINT), TextLayout::new_with_no_wrap()));
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
            head.spawn(words("F1 CLOSES THIS", 10.0, FAINT));
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
        crate::debug_menu::spawn_commands(popup);
    });
}

/// What of the HUD the game's screen hides while it is open: the meters of
/// the stance, speed and stamina.
#[derive(Component)]
struct ScreenHides;

/// The meters of the stance, speed and stamina, which the game's own
/// stance panel takes the place of with `TARKOV UI` (`battle_view`).
#[derive(Component)]
struct PlainMeters;

/// With `TARKOV UI` the inventory screen is the game's, which shows
/// nothing of the app's under its left side: the meters are hidden while
/// it is open, and not drawn at all where the game's stance panel is.
fn hide_over_screen(
    controller: Res<Controller>,
    features: Res<crate::options_view::Features>,
    look: Res<crate::health_view::HealthLook>,
    mut row: Query<(&mut Node, Has<PlainMeters>), With<ScreenHides>>,
) {
    let battle = look.tarkov_ui && look.battle.is_some();
    for (mut node, meters) in &mut row {
        let display = if (controller.inventory_open && features.tarkov_ui) || controller.edit_build || (meters && battle) { Display::None } else { Display::Flex };
        if node.display != display {
            node.display = display;
        }
    }
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
    // Written while the panel is shown, or as `F1` shows it (`guide` is
    // not ordered before this): a text written is measured and laid out
    // afresh whether it is seen or not.
    if !shown.is_some_and(|shown| shown.0) && !keys.just_pressed(KeyCode::F1) {
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

/// Keeps the panel at the top right: `F1` shows it and hides it, and it
/// stands back while the inventory screen or the edit build screen is open.
fn guide(
    keys: Res<ButtonInput<KeyCode>>,
    controller: Res<Controller>,
    mut shown: ResMut<GuideShown>,
    mut popup: Query<&mut Node, (With<DebugPopup>, Without<DebugTag>)>,
    (look, window, mut tag): (Res<crate::health_view::HealthLook>, Query<&Window, With<bevy::window::PrimaryWindow>>, Query<&mut Node, With<DebugTag>>),
) {
    let _clock = crate::render_timing::Clock::new("hud guide");
    let Ok(mut node) = popup.single_mut() else { return };
    if keys.just_pressed(KeyCode::F1) {
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

fn label(text: &str) -> impl Bundle {
    (Text::new(text), TextFont { font_size: 11.0, ..default() }, TextColor(LABEL))
}

fn spawn(mut commands: Commands, options: Res<crate::Options>) {
    let track = |width: f32, height: f32| {
        (Node { width: Val::Px(width), height: Val::Px(height), ..default() }, BackgroundColor(TRACK))
    };
    let _ = &options;
    commands
        .spawn((
            ScreenHides,
            PlainMeters,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(16.0),
                bottom: Val::Px(16.0),
                column_gap: Val::Px(10.0),
                align_items: AlignItems::FlexEnd,
                ..default()
            },
        ))
        .with_children(|panel| {
            // How high the character stands: a column that fills from the
            // bottom, empty lying down.
            panel.spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(3.0), ..default() }).with_children(
                |stance| {
                    stance
                        .spawn((
                            Node {
                                width: Val::Px(10.0),
                                height: Val::Px(STANCE_HEIGHT),
                                flex_direction: FlexDirection::ColumnReverse,
                                ..default()
                            },
                            BackgroundColor(TRACK),
                        ))
                        .with_children(|column| {
                            column.spawn((
                                Fill::Stance,
                                Node { width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
                                BackgroundColor(SET),
                            ));
                        });
                },
            );
            panel.spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(4.0), ..default() }).with_children(
                |bars| {
                    bars.spawn((StanceWord, label("standing")));
                    // The speed setting, over the notches the state allows.
                    bars.spawn(label("speed"));
                    bars.spawn(Node { column_gap: Val::Px(2.0), ..default() }).with_children(|row| {
                        for notch in 0..NOTCHES {
                            row.spawn((
                                Notch(notch),
                                Node { width: Val::Px(NOTCH_WIDTH), height: Val::Px(10.0), ..default() },
                                BackgroundColor(REFUSED),
                            ));
                        }
                    });
                    for (fill, name, colour) in [(Fill::Legs, "legs", LEGS), (Fill::Arms, "arms", ARMS)] {
                        bars.spawn(label(name));
                        bars.spawn(track(BAR_WIDTH, 7.0)).with_children(|bar| {
                            bar.spawn((
                                fill,
                                Node { width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
                                BackgroundColor(colour),
                            ));
                        });
                    }
                },
            );
        });
}

fn show(
    controller: Res<Controller>,
    mut fills: Query<(&Fill, &mut Node, &mut BackgroundColor), Without<Notch>>,
    mut notches: Query<(&Notch, &mut BackgroundColor), Without<Fill>>,
    mut word: Query<&mut Text, With<StanceWord>>,
) {
    let _clock = crate::render_timing::Clock::new("hud show");
    let m = &controller.movement;
    let stamina = &m.stamina;
    for (fill, mut node, mut colour) in &mut fills {
        match fill {
            Fill::Legs | Fill::Arms => {
                let (pool, full) = if matches!(fill, Fill::Legs) { (&stamina.legs, LEGS) } else { (&stamina.arms, ARMS) };
                // (Written only when it changes: a node written is laid
                // out afresh, `findings.md`.)
                let width = Val::Percent(pool.level().clamp(0.0, 1.0) * 100.0);
                if node.width != width {
                    node.width = width;
                }
                // The game's bars take another look under the threshold.
                let look = if pool.exhausted() { SPENT } else { full };
                if colour.0 != look {
                    colour.0 = look;
                }
            }
            Fill::Stance => {
                let level = if m.prone { 0.0 } else { m.smoothed_pose_level.clamp(0.0, 1.0) };
                // Fully crouched still shows a foot of column.
                let height = Val::Percent(if m.prone { 0.0 } else { 12.0 + 88.0 * level });
                if node.height != height {
                    node.height = height;
                }
            }
        }
    }
    // The setting in force is the sprint's pace while a sprint is paid for
    // and the clamped speed otherwise, over the character's top speed; the
    // notches up to the state's limit are the ones that can be set.
    let top = m.settings.max_speed.max(1e-3);
    let set = if m.sprinting { m.character_speed } else { m.clamped_speed() } / top;
    let allowed = m.speed_limit() / top;
    for (notch, mut colour) in &mut notches {
        let at = if notch.0 + 1 == NOTCHES { 1.0 } else { notch.0 as f32 * 0.05 };
        let look = if at <= set + 1e-3 {
            SET
        } else if (at - allowed).round_to(2) <= 0.0 {
            ALLOWED
        } else {
            REFUSED
        };
        if colour.0 != look {
            colour.0 = look;
        }
    }
    if let Ok(mut text) = word.single_mut() {
        let stance = match (m.prone, m.smoothed_pose_level) {
            (true, _) => String::from("lying"),
            (false, level) if level < 0.11 => String::from("crouched"),
            (false, level) if level > 0.95 => String::from("standing"),
            (false, level) => format!("stance {:.0}%", level * 100.0),
        };
        if **text != stance {
            **text = stance;
        }
    }
}

/// Keeps the frame-rate readout: frames a second and the time a frame
/// takes, both over the last second of drawn frames, the longest frame in
/// that second, and what the simulation cost a drawn frame. The times are
/// the engine's own (`Time<Real>`), which is what `--log-pacing` writes as
/// `dt_ms`. Written while the panel at the top right is shown.
fn frame_rate(
    real: Res<Time<Real>>,
    controller: Res<Controller>,
    guide: Option<Res<GuideShown>>,
    mut times: ResMut<FrameTimes>,
    mut readout: Query<&mut Text, With<FrameRateText>>,
) {
    let _clock = crate::render_timing::Clock::new("hud frame_rate");
    let Ok(mut text) = readout.single_mut() else { return };
    let shown = guide.as_ref().is_some_and(|guide| guide.0);
    if guide.as_ref().is_some_and(|guide| guide.is_changed()) {
        // Shown again, it is written at once.
        times.since = FRAME_REFRESH;
    }
    // Kept up while hidden too, so that it reads true the moment it shows.
    let took = real.delta_secs();
    if took <= 0.0 {
        return;
    }
    times.frames.push_back((took, controller.paced.cost));
    let mut total: f32 = times.frames.iter().map(|(took, _)| took).sum();
    while total > FRAME_WINDOW && times.frames.len() > 1 {
        total -= times.frames.pop_front().map_or(0.0, |(took, _)| took);
    }
    times.since += took;
    if times.since < FRAME_REFRESH || !shown {
        return;
    }
    times.since = 0.0;
    let count = times.frames.len() as f32;
    let longest = times.frames.iter().map(|(took, _)| *took).fold(0.0, f32::max);
    let simulation = times.frames.iter().map(|(_, cost)| cost).sum::<f32>() / count;
    **text = format!(
        "{:.0} fps   {:.2} ms a frame, longest {:.2}   simulation {:.2} ms",
        count / total,
        total / count * 1000.0,
        longest * 1000.0,
        simulation * 1000.0
    );
}

trait RoundTo {
    fn round_to(self, places: i32) -> f32;
}

impl RoundTo for f32 {
    /// Rounded to so many decimal places, as the game rounds the notch's
    /// place against the limit.
    fn round_to(self, places: i32) -> f32 {
        let scale = 10f32.powi(places);
        (self * scale).round() / scale
    }
}
