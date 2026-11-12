//! The parts drawn: buttons, boxes, arrows, bars, rows and sections.

use super::*;

/// A chevron drawn as two edges of a square turned: `degrees` 45 points it
/// right, 135 down, 225 left.
pub(super) fn chevron(parent: &mut ChildSpawnerCommands, look: &Look, side: f32, degrees: f32, color: Color) {
    let node = Node { width: look.u(side), height: look.u(side), border: UiRect { top: look.u(2.0), right: look.u(2.0), ..default() }, flex_shrink: 0.0, ..default() };
    parent.spawn((node, BorderColor::all(color), UiTransform::from_rotation(Rot2::degrees(degrees))));
}

/// A button with its words in its middle: chosen, the game's light ground
/// with dark words; else see-through black, lit under the mouse; dim where
/// it does nothing.
pub(super) fn button(parent: &mut ChildSpawnerCommands, look: &Look, click: Click, words: &str, node: Node, chosen: bool, enabled: bool) {
    let (rest, lit, ink) = match (chosen, enabled) {
        (true, _) => (CHOSEN, CHOSEN_LIT, INK_DARK),
        (false, true) => (GROUND, GROUND_LIT, INK),
        (false, false) => (GROUND, GROUND, INK_FAINT),
    };
    let node = Node { justify_content: JustifyContent::Center, align_items: AlignItems::Center, flex_shrink: 0.0, ..node };
    let mut spawned = parent.spawn((click, Button, node, BackgroundColor(rest), Tint(rest, lit)));
    if enabled && click.feedback() {
        spawned.insert(Feedback);
    }
    spawned.with_children(|inside| {
        inside.spawn((look.strong(words, 17.0, ink), TextLayout::new_with_no_wrap()));
    });
}

/// A small button of a section's head or a row (DEFAULT, ALL TO 0).
pub(super) fn small_button(parent: &mut ChildSpawnerCommands, look: &Look, click: Click, words: &str) {
    let node = Node { height: look.u(30.0), padding: UiRect::horizontal(look.u(14.0)), border: UiRect::all(look.u(1.0)), ..default() };
    parent.spawn((click, Button, Feedback, node, BackgroundColor(GROUND), Tint(GROUND, GROUND_LIT), BorderColor::all(EDGE))).with_children(|inside| {
        inside.spawn(Node { height: Val::Percent(100.0), align_items: AlignItems::Center, ..default() }).with_children(|middle| {
            middle.spawn((look.strong(words, 14.0, INK), TextLayout::new_with_no_wrap()));
        });
    });
}

/// A box ticked or not, as the game's settings' toggles: dim where it
/// cannot be changed.
pub(super) fn tick_box(parent: &mut ChildSpawnerCommands, look: &Look, on: bool, enabled: bool) {
    let node = Node {
        width: look.u(24.0),
        height: look.u(24.0),
        border: UiRect::all(look.u(2.0)),
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        flex_shrink: 0.0,
        ..default()
    };
    let edge = match (enabled, on) {
        (false, _) => INK_FAINT,
        (true, true) => ON.with_alpha(0.8),
        (true, false) => INK_DIM,
    };
    parent.spawn((node, BorderColor::all(edge), BackgroundColor(WELL))).with_children(|inside| {
        if on {
            // A tick: two edges of a tall box, turned.
            let tick = Node { width: look.u(7.0), height: look.u(13.0), border: UiRect { right: look.u(3.0), bottom: look.u(3.0), ..default() }, margin: UiRect::bottom(look.u(3.0)), ..default() };
            inside.spawn((tick, BorderColor::all(if enabled { ON } else { INK_FAINT }), UiTransform::from_rotation(Rot2::degrees(45.0))));
        }
    });
}

/// A setting on or off: its box, the whole of it a button.
pub(super) fn toggle(parent: &mut ChildSpawnerCommands, look: &Look, which: Toggle, on: bool) {
    let node = Node { padding: UiRect::all(look.u(4.0)), ..default() };
    parent.spawn((Click::Toggle(which), Button, node, BackgroundColor(Color::NONE), Tint(Color::NONE, GROUND_LIT))).with_children(|inside| {
        tick_box(inside, look, on, true);
    });
}

/// A setting among several: its arrows either side of what is chosen, an
/// arrow dim at either end.
pub(super) fn stepper(parent: &mut ChildSpawnerCommands, look: &Look, which: Step, choices: (Vec<String>, usize)) {
    stepper_as(parent, look, which, choices, true);
}

/// A stepper, or one drawn dim with arrows that do nothing where what it
/// goes by is off.
pub(super) fn stepper_as(parent: &mut ChildSpawnerCommands, look: &Look, which: Step, (choices, at): (Vec<String>, usize), enabled: bool) {
    let node = Node { width: look.u(330.0), height: look.u(36.0), border: UiRect::all(look.u(1.0)), align_items: AlignItems::Center, flex_shrink: 0.0, ..default() };
    parent.spawn((node, BackgroundColor(GROUND), BorderColor::all(EDGE))).with_children(|row| {
        for (by, degrees) in [(-1i8, 225.0), (1, 45.0)] {
            let at_end = !enabled || if by < 0 { at == 0 } else { at + 1 >= choices.len() };
            let arrow = |row: &mut ChildSpawnerCommands| {
                let node = Node { width: look.u(40.0), height: Val::Percent(100.0), justify_content: JustifyContent::Center, align_items: AlignItems::Center, flex_shrink: 0.0, ..default() };
                let mut spawned = row.spawn((node, BackgroundColor(Color::NONE)));
                if enabled {
                    let lit = if at_end { Color::NONE } else { GROUND_LIT };
                    spawned.insert((Click::Step(which, by), Button, Tint(Color::NONE, lit)));
                }
                spawned.with_children(|inside| {
                    chevron(inside, look, 9.0, degrees, if at_end { INK_FAINT } else { INK });
                });
            };
            if by < 0 {
                arrow(row);
                row.spawn(Node { flex_grow: 1.0, justify_content: JustifyContent::Center, overflow: Overflow::clip(), ..default() }).with_children(|middle| {
                    middle.spawn((look.strong(choices.get(at).cloned().unwrap_or_default(), 17.0, if enabled { INK } else { INK_FAINT }), TextLayout::new_with_no_wrap()));
                });
            } else {
                arrow(row);
            }
        }
    });
}

/// A bar to slide, its handle at the end of its fill, and its figure in a
/// box to its right, as the game's own bar has it.
pub(super) fn slider(parent: &mut ChildSpawnerCommands, look: &Look, kind: Slide, value: f32, width: f32) {
    slider_as(parent, look, kind, value, width, true);
}

/// A bar, or one drawn dim that does not slide where what it sets is off.
pub(super) fn slider_as(parent: &mut ChildSpawnerCommands, look: &Look, kind: Slide, value: f32, width: f32, enabled: bool) {
    let [low, high] = kind.range();
    let share = ((value - low) / (high - low)).clamp(0.0, 1.0);
    let (fill_ink, figure_ink) = if enabled { slide_inks(kind, value) } else { (INK_FAINT, INK_FAINT) };
    parent.spawn(Node { column_gap: look.u(14.0), align_items: AlignItems::Center, flex_shrink: 0.0, ..default() }).with_children(|row| {
        // The bar heard over its whole height, drawn thin in its middle.
        let bar = Node { width: look.u(width), height: look.u(26.0), align_items: AlignItems::Center, flex_shrink: 0.0, ..default() };
        let mut spawned = row.spawn((bar, BackgroundColor(Color::NONE)));
        if enabled {
            spawned.insert((SlideBar(kind), RelativeCursorPosition::default()));
        }
        spawned.with_children(|bar| {
            let track = Node { width: Val::Percent(100.0), height: look.u(6.0), ..default() };
            bar.spawn((track, BackgroundColor(WELL))).with_children(|track| {
                let fill = Node { width: Val::Percent(share * 100.0), height: Val::Percent(100.0), justify_content: JustifyContent::FlexEnd, align_items: AlignItems::Center, ..default() };
                let mut fill_spawned = track.spawn((fill, BackgroundColor(fill_ink)));
                if enabled {
                    fill_spawned.insert(SlideFill(kind));
                }
                fill_spawned.with_children(|fill| {
                    let handle = Node { width: look.u(8.0), height: look.u(20.0), margin: UiRect::right(look.u(-4.0)), flex_shrink: 0.0, ..default() };
                    fill.spawn((handle, BackgroundColor(if enabled { INK } else { INK_FAINT })));
                });
            });
        });
        let least = if matches!(kind, Slide::Volume(_)) { 128.0 } else { 84.0 };
        let figure = Node { min_width: look.u(least), height: look.u(32.0), padding: UiRect::horizontal(look.u(8.0)), border: UiRect::all(look.u(1.0)), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() };
        let edge = if figure_ink == ELITE { ELITE.with_alpha(0.55) } else { EDGE };
        row.spawn((figure, BackgroundColor(GROUND), BorderColor::all(edge))).with_children(|figure| {
            let mut words = figure.spawn((look.strong(kind.shown(value), 16.0, figure_ink), TextLayout::new_with_no_wrap()));
            if enabled {
                words.insert(SlideValue(kind));
            }
        });
    });
}

/// A bar's fill and its figure's colours at a value: khaki and the ink,
/// and an elite's orange for a skill at elite or a family at its top level.
pub(super) fn slide_inks(kind: Slide, value: f32) -> (Color, Color) {
    let top = match kind {
        Slide::Skill(_) => value >= trp_sim::condition::skills::ELITE as f32,
        Slide::Mastering(_) => value >= (trp_sim::condition::mastering::TOP_LEVEL + 1) as f32,
        Slide::Level(top) => value >= top as f32,
        _ => false,
    };
    if top { (ELITE, ELITE) } else { (KHAKI, INK) }
}

/// A small word at a row's end: `RESTART` for what waits for a start, what
/// a switch needs.
pub(super) fn tag(parent: &mut ChildSpawnerCommands, look: &Look, words: &str, color: Color) {
    let node = Node { padding: UiRect::axes(look.u(8.0), look.u(3.0)), border: UiRect::all(look.u(1.0)), flex_shrink: 0.0, ..default() };
    parent.spawn((node, BorderColor::all(color.with_alpha(0.6)), BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)))).with_children(|inside| {
        inside.spawn((look.strong(words, 12.0, color), TextLayout::new_with_no_wrap()));
    });
}

/// A section's head, as the app's design has it: an arrow that folds it,
/// a stroke, its name, and at its right what it says of itself (a count,
/// a button).
pub(super) fn section_head(parent: &mut ChildSpawnerCommands, look: &Look, at: usize, folded: bool, name: &str, right: impl FnOnce(&mut ChildSpawnerCommands)) {
    section_head_as(parent, look, at, folded, name, KHAKI, right);
}

/// A section's head with its stroke in a colour of its own (a skill
/// class's).
pub(super) fn section_head_as(parent: &mut ChildSpawnerCommands, look: &Look, at: usize, folded: bool, name: &str, stroke: Color, right: impl FnOnce(&mut ChildSpawnerCommands)) {
    let node = Node {
        width: Val::Percent(100.0),
        height: look.u(52.0),
        padding: UiRect::horizontal(look.u(16.0)),
        margin: UiRect::top(look.u(if at == 0 { 0.0 } else { 14.0 })),
        column_gap: look.u(16.0),
        align_items: AlignItems::Center,
        border: UiRect::bottom(look.u(1.0)),
        flex_shrink: 0.0,
        ..default()
    };
    parent.spawn((Click::Fold(at), Button, node, BackgroundColor(HEAD), Tint(HEAD, HEAD_LIT), BorderColor::all(EDGE))).with_children(|head| {
        head.spawn(Node { width: look.u(14.0), justify_content: JustifyContent::Center, flex_shrink: 0.0, ..default() }).with_children(|arrow| {
            chevron(arrow, look, 9.0, if folded { 45.0 } else { 135.0 }, INK);
        });
        head.spawn((Node { width: look.u(3.0), height: look.u(24.0), flex_shrink: 0.0, ..default() }, BackgroundColor(stroke)));
        head.spawn((look.strong(name, 23.0, INK), TextLayout::new_with_no_wrap()));
        head.spawn(Node { flex_grow: 1.0, ..default() });
        right(head);
    });
}

/// A row of a setting: its name; its control at the right, and past it
/// what waits for a start.
pub(super) fn setting_row(parent: &mut ChildSpawnerCommands, look: &Look, stripe: bool, name: &str, waiting: bool, control: impl FnOnce(&mut ChildSpawnerCommands)) {
    row(parent, look, stripe, Row { name, waiting, ..default() }, control);
}

/// How a row of a setting is drawn beyond its name.
#[derive(Default)]
pub(super) struct Row<'a> {
    pub(super) name: &'a str,
    /// A line under its name, on the SKILLS page alone: a skill's
    /// description, a family's weapons, the level's experience. Empty,
    /// none.
    pub(super) what: &'a str,
    /// Drawn dim, its control doing nothing: what it goes by is off.
    pub(super) dim: bool,
    pub(super) waiting: bool,
    /// A word at its end, in its colour.
    pub(super) tag: Option<(&'a str, Color)>,
    /// An edge at its left in a colour (a skill's class's).
    pub(super) edge: Option<Color>,
    /// Its name lit as an elite (a skill at elite, a family at its top
    /// level).
    pub(super) top: bool,
}

/// A row of a setting as `Row` says.
pub(super) fn row(parent: &mut ChildSpawnerCommands, look: &Look, stripe: bool, spec: Row, control: impl FnOnce(&mut ChildSpawnerCommands)) {
    let rest = if stripe { ROW_ALT } else { ROW };
    let node = Node {
        width: Val::Percent(100.0),
        min_height: look.u(50.0),
        padding: UiRect::axes(look.u(18.0), look.u(7.0)),
        column_gap: look.u(20.0),
        align_items: AlignItems::Center,
        border: UiRect::left(look.u(if spec.edge.is_some() { 3.0 } else { 0.0 })),
        flex_shrink: 0.0,
        ..default()
    };
    let (name_ink, what_ink) = match (spec.dim, spec.top) {
        (true, _) => (INK_FAINT, INK_FAINT),
        (false, true) => (ELITE, INK_DIM),
        (false, false) => (INK, INK_DIM),
    };
    parent.spawn((node, BackgroundColor(rest), Lit(rest), BorderColor::all(spec.edge.unwrap_or(Color::NONE)), RelativeCursorPosition::default())).with_children(|row| {
        row.spawn(Node { flex_direction: FlexDirection::Column, flex_grow: 1.0, flex_basis: Val::Px(0.0), min_width: Val::Px(0.0), row_gap: look.u(3.0), ..default() }).with_children(|words| {
            words.spawn(look.text(spec.name, 19.0, name_ink));
            if !spec.what.is_empty() {
                words.spawn(look.text(spec.what, 14.0, what_ink));
            }
        });
        row.spawn(Node { column_gap: look.u(12.0), align_items: AlignItems::Center, justify_content: JustifyContent::FlexEnd, flex_shrink: 0.0, ..default() }).with_children(control);
        row.spawn(Node { min_width: look.u(112.0), column_gap: look.u(6.0), justify_content: JustifyContent::FlexEnd, flex_shrink: 0.0, ..default() }).with_children(|end| {
            if let Some((words, color)) = spec.tag {
                tag(end, look, words, color);
            }
            if spec.waiting {
                tag(end, look, "RESTART", AMBER);
            }
        });
    });
}

/// A note across the page (a switch that holds the page's settings off).
pub(super) fn note_row(parent: &mut ChildSpawnerCommands, look: &Look, words: &str, color: Color) {
    let node = Node { width: Val::Percent(100.0), padding: UiRect::axes(look.u(18.0), look.u(12.0)), margin: UiRect::bottom(look.u(10.0)), border: UiRect::left(look.u(3.0)), flex_shrink: 0.0, ..default() };
    parent.spawn((node, BackgroundColor(ROW), BorderColor::all(color))).with_children(|row| {
        row.spawn(look.text(words, 16.0, color));
    });
}
