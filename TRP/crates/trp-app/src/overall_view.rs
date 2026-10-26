//! The inventory's OVERALL tab (inventory spec, "The OVERALL tab"; the
//! user, 2026-10-18: "Can we add the overall page now ... Dim out the stats
//! for now, since we won't track them. I want to add the clothing drop
//! downs so we can change our appearance").
//!
//! With `TARKOV UI` it is the game's own page (`OverallScreen`), drawn from
//! the install's scene by `health_view::draw_overall`: the character's
//! model in its view, turned by a drag across it as the game's is
//! (`XCoordRotation`: the model's turn less the pointer's move across, in
//! the screen's pixels, times 0.3 degrees, kept within a turn; anew at 180
//! each time the page shows and each time a suit is put on, the model being
//! made again then, `ShowPreview`), the statistics dimmed, and the two
//! clothing dropdowns (`DropDownBox`): a click on a box opens or shuts its
//! list (the sound `MenuDropdown` as it opens), a click on a line chooses
//! its suit (`MenuDropdownSelect`) and shuts the list, any button pressed
//! with the mouse off the box and its list shuts it, the wheel over the
//! list scrolls it by its scroll view's step, its bar's handle is dragged.
//! The page shown says each dropdown's suit, which plays the choosing sound
//! once for each (`InventoryClothingSelectionPanel.PrepareDropDownBox`:
//! `UpdateValue` with its sound). Without `TARKOV UI` it is the app's own
//! look at the game's places. The suits themselves are `clothing`'s.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::body_view::SuitRole;
use crate::clothing::Clothing;
use crate::health_view::{OpenList, OverallShows, OVERALL_SELECTORS, OVERALL_VIEW};
use crate::inventory_view::{words, FAINT, LINE, PANEL, WORDS};
use crate::ui_sound::{Sound, Ui};

pub struct OverallPlugin;

impl Plugin for OverallPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<OverallState>().add_systems(Update, (hear, draw).chain().after(crate::inventory_view::ScreenDrawn).run_if(crate::running));
    }
}

/// The page's node, drawn into afresh as what it shows changes.
#[derive(Component)]
pub struct OverallPage;

/// What the left button went down on.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Pressed {
    Box(usize),
    Line(usize, usize),
}

/// The page as it stands: the list open, each list's scroll, what the left
/// button went down on, the model's drag and the bar's handle held (the
/// cursor's place in pixels where it went down and last, the handle's
/// offset), whether the page showed on the frame before, and what was drawn.
#[derive(Resource, Default)]
struct OverallState {
    open: Option<usize>,
    scroll: [f32; 2],
    pressed: Option<Pressed>,
    turning: Option<(Vec2, Vec2, bool)>,
    holding: Option<f32>,
    showed: bool,
    worn: [Option<usize>; 2],
    drawn: Option<String>,
}

/// The suits' roles in the dropdowns' order.
const ROLES: [SuitRole; 2] = [SuitRole::Upper, SuitRole::Lower];

/// How far down the app's own look puts the page, in the screen's units:
/// under its row of tabs, which is lower than the game's bar (the app's).
/// `ESTIMATE`: 60 is the app's own figure, with no game figure behind it (the
/// app's own row of tabs stands lower than the game's 43-unit bar,
/// `inventory.md`, "The bar of tabs"; with `TARKOV UI` the page sits at the
/// game's place and drops none); at 1080 lines the open list runs past the
/// window's foot.
const OWN_DROP: f32 = 60.0;

/// How far down the page is drawn: the game's look where it is, else the
/// app's own.
fn drop_of(features: &crate::options_view::Features, look: &crate::health_view::HealthLook) -> f32 {
    if features.tarkov_ui && look.tarkov_ui { 0.0 } else { OWN_DROP }
}

/// The screen's size in its units (1080 high) and the pixels to a unit.
pub(crate) fn screen_of(window: &Window) -> ([f32; 2], f32) {
    let s = window.height() / crate::inventory_view::SCREEN_HEIGHT;
    ([window.width() / s.max(1e-3), crate::inventory_view::SCREEN_HEIGHT], s)
}

pub(crate) fn inside([x, y, w, h]: [f32; 4], [px, py]: [f32; 2]) -> bool {
    px >= x && px < x + w && py >= y && py < y + h
}

/// Where a dropdown's things lie on a screen so large (left, top, width,
/// height in its units): its box, its list's panel open, its scroll area,
/// its content's column, its bar and the bar's sliding area.
pub(crate) struct Places {
    pub(crate) boxed: [f32; 4],
    pub(crate) open: [f32; 4],
    pub(crate) area: [f32; 4],
    pub(crate) content: [f32; 4],
    pub(crate) bar: Option<[f32; 4]>,
    pub(crate) row: trp_data::screen::RowGroup,
    pub(crate) line: f32,
}

fn places(overall: &trp_data::screen::OverallPanel, role: usize, screen: [f32; 2]) -> Option<Places> {
    dropdown_places(&overall.panel, OVERALL_SELECTORS[role], overall.line.as_ref(), screen)
}

/// A dropdown's places (`DropDownBox`) in a panel laid in a screen so
/// large, the dropdown by its path below the panel's top, its lines as
/// high as their template asks.
pub(crate) fn dropdown_places(panel: &trp_data::screen::ScenePanel, selector: &str, line: Option<&trp_data::screen::ScenePanel>, screen: [f32; 2]) -> Option<Places> {
    let at = |end: &str| panel.find(&format!("{selector}{end}")).map(|index| panel.placed_in(index, screen).rect);
    let content = panel.find(&format!("{selector}/Open Panel/Scroll Area/Content"));
    Some(Places {
        boxed: at("")?,
        open: at("/Open Panel")?,
        area: at("/Open Panel/Scroll Area")?,
        content: content.map(|content| panel.placed_in(content, screen).rect)?,
        bar: at("/Open Panel/Scrollbar"),
        row: content.and_then(|content| panel.nodes[content].group).unwrap_or_default(),
        line: line.map_or(28.0, |line| line.asks(0, 1).0.max(1.0)),
    })
}

impl Places {
    /// How high all the lines are, and how much of them shows.
    pub(crate) fn heights(&self, count: usize) -> (f32, f32) {
        let whole = self.row.padding[2] + self.row.padding[3] + count as f32 * self.line + count.saturating_sub(1) as f32 * self.row.spacing;
        (whole, self.area[3])
    }

    /// The line under a place, scrolled so far.
    pub(crate) fn line_at(&self, at: [f32; 2], scroll: f32, count: usize) -> Option<usize> {
        if !inside(self.area, at) {
            return None;
        }
        let y = at[1] - self.content[1] - self.row.padding[2] + scroll;
        let step = self.line + self.row.spacing;
        let n = (y / step).floor();
        (n >= 0.0 && y - n * step < self.line && (n as usize) < count).then_some(n as usize)
    }
}

/// The bar's handle for a list scrolled so far: its place, as the bar lays
/// it on its sliding area (`ScrollBar::handle_in`), on the screen.
fn handle_of(overall: &trp_data::screen::OverallPanel, screen: [f32; 2], whole: f32, shown: f32, scroll: f32) -> Option<[f32; 4]> {
    bar_handle(&overall.panel, overall.list_scroll?.bar?, screen, whole, shown, scroll)
}

/// A scroll bar's handle in a panel laid in a screen so large, for what it
/// scrolls so high, so much of it shown and scrolled so far from its top.
pub(crate) fn bar_handle(panel: &trp_data::screen::ScenePanel, bar: trp_data::screen::ScrollBar, screen: [f32; 2], whole: f32, shown: f32, scroll: f32) -> Option<[f32; 4]> {
    let top = trp_data::screen::place(panel.nodes[0].layout, trp_data::screen::Rect { at: [0.0, 0.0], size: screen });
    let most = (whole - shown).max(0.0);
    let value = if most > 0.0 { 1.0 - scroll / most } else { 1.0 };
    let share = if whole > 0.0 { (shown / whole).clamp(0.0, 1.0) } else { 1.0 };
    let [x, y, w, h] = bar.handle_in(panel, top.size, value, share)?;
    Some([x + top.at[0], y + (screen[1] - top.at[1] - top.size[1]), w, h])
}

/// The game's sounds, the mouse and the wheel on the page.
#[allow(clippy::too_many_arguments)]
fn hear(
    controller: Res<crate::controller::Controller>,
    selected: Res<crate::inventory_view::Selected>,
    look: Res<crate::health_view::HealthLook>,
    features: Res<crate::options_view::Features>,
    clothing: Option<ResMut<Clothing>>,
    window: Query<&Window, With<PrimaryWindow>>,
    buttons: Res<ButtonInput<MouseButton>>,
    wheel: Res<bevy::input::mouse::AccumulatedMouseScroll>,
    mut figure: ResMut<crate::figure_view::OverallFigure>,
    mut state: ResMut<OverallState>,
    mut sounds: MessageWriter<Sound>,
) {
    let shows = controller.inventory_open && selected.0 == crate::inventory_view::OVERALL;
    let state = &mut *state;
    if !shows {
        (state.open, state.pressed, state.turning, state.holding, state.showed) = (None, None, None, None, false);
        return;
    }
    let mut clothing = clothing.filter(|_| features.clothing);
    // Shown anew: the model made again, its turn its own; the dropdowns
    // told their suits, each with its sound.
    if !state.showed {
        state.showed = true;
        figure.yaw = crate::figure_view::YAW;
        if clothing.is_some() {
            sounds.write(Sound::Ui(Ui::MenuDropdownSelect));
            sounds.write(Sound::Ui(Ui::MenuDropdownSelect));
        }
    }
    if let Some(clothing) = clothing.as_ref() {
        if state.worn != clothing.worn {
            state.worn = clothing.worn;
            figure.yaw = crate::figure_view::YAW;
        }
    }
    let Ok(window) = window.single() else { return };
    let Some(overall) = &look.overall else { return };
    let (screen, s) = screen_of(window);
    let Some(cursor_px) = window.cursor_position() else { return };
    let cursor = [cursor_px.x / s, cursor_px.y / s - drop_of(&features, &look)];
    let counts: [usize; 2] = std::array::from_fn(|role| clothing.as_ref().map_or(0, |clothing| clothing.of(ROLES[role]).len()));
    let places: [Option<Places>; 2] = std::array::from_fn(|role| places(overall, role, screen).filter(|_| clothing.is_some()));
    // Over a box, or over the list open.
    let over_box = |role: usize| places[role].as_ref().is_some_and(|at| inside(at.boxed, cursor));
    let over_list = |open: Option<usize>, role: usize| open == Some(role) && places[role].as_ref().is_some_and(|at| inside(at.open, cursor));
    let hovering = state.open.is_some_and(|role| over_box(role) || over_list(state.open, role));
    // Any button pressed off the box and its list shuts it
    // (`BaseDropDownBox.Update`).
    let any_pressed = buttons.get_just_pressed().next().is_some();
    if any_pressed && state.open.is_some() && !hovering {
        state.open = None;
    }
    // The list's wheel and its bar.
    if let (Some(role), true) = (state.open, wheel.delta.y != 0.0) {
        if let (Some(at), Some(scrolling)) = (places[role].as_ref(), overall.list_scroll) {
            if inside(at.area, cursor) {
                let notches = match wheel.unit {
                    bevy::input::mouse::MouseScrollUnit::Line => wheel.delta.y,
                    bevy::input::mouse::MouseScrollUnit::Pixel => wheel.delta.y.signum(),
                };
                let (whole, shown) = at.heights(counts[role]);
                state.scroll[role] = (state.scroll[role] - notches * scrolling.sensitivity).clamp(0.0, (whole - shown).max(0.0));
            }
        }
    }
    if buttons.just_pressed(MouseButton::Left) {
        state.pressed = None;
        if let Some(role) = state.open.filter(|role| over_list(state.open, *role)) {
            let at = places[role].as_ref().expect("over its list");
            let (whole, shown) = at.heights(counts[role]);
            let handle = handle_of(overall, screen, whole, shown, state.scroll[role]);
            if let Some([_, hy, _, hh]) = handle.filter(|handle| inside(*handle, cursor)) {
                state.holding = Some(cursor[1] - (hy + hh / 2.0));
            } else if let Some(line) = at.line_at(cursor, state.scroll[role], counts[role]) {
                state.pressed = Some(Pressed::Line(role, line));
            }
        } else if let Some(role) = (0..2).find(|&role| over_box(role)) {
            state.pressed = Some(Pressed::Box(role));
        } else if panel_rect(overall, OVERALL_VIEW, screen).is_some_and(|view| inside(view, cursor)) {
            state.turning = Some((cursor_px, cursor_px, false));
        }
    }
    // The handle held: the list scrolled with it.
    if let (Some(offset), Some(role)) = (state.holding, state.open) {
        if let (Some(at), Some(bar)) = (places[role].as_ref(), overall.list_scroll.and_then(|scroll| scroll.bar)) {
            let (whole, shown) = at.heights(counts[role]);
            let sliding = bar.sliding.map(|sliding| overall.panel.placed_in(sliding, screen).rect);
            if let (Some([_, sy, _, sh]), Some([_, _, _, hh])) = (sliding, handle_of(overall, screen, whole, shown, state.scroll[role])) {
                let room = (sh - hh).max(1e-3);
                let share = ((cursor[1] - offset - hh / 2.0 - sy) / room).clamp(0.0, 1.0);
                state.scroll[role] = share * (whole - shown).max(0.0);
            }
        }
    }
    // The model turned by a drag across its view, once the mouse has moved
    // as far as the engine's drag begins (`EventSystem`'s threshold), by
    // each frame's move in the screen's pixels.
    if let Some((from, last, dragging)) = state.turning.as_mut() {
        let scale = window.scale_factor();
        // (The engine's input module sends the drag on the frame it
        // begins, with that frame's move: `ProcessDrag`.)
        if !*dragging && (cursor_px - *from).length() * scale >= crate::inventory_view::DRAG_THRESHOLD {
            *dragging = true;
        }
        if *dragging {
            let moved = (cursor_px.x - last.x) * scale;
            if moved != 0.0 {
                let speed = overall.turn_a_pixel.unwrap_or(0.3);
                let yaw = figure.yaw - moved * speed;
                figure.yaw = if yaw >= 360.0 { yaw - 360.0 } else if yaw < 0.0 { yaw + 360.0 } else { yaw };
            }
        }
        *last = cursor_px;
    }
    if buttons.just_released(MouseButton::Left) {
        let now = if let Some(role) = state.open.filter(|role| over_list(state.open, *role)) {
            places[role].as_ref().and_then(|at| at.line_at(cursor, state.scroll[role], counts[role])).map(|line| Pressed::Line(role, line))
        } else {
            (0..2).find(|&role| over_box(role)).map(Pressed::Box)
        };
        match (state.pressed.take(), now) {
            (Some(Pressed::Box(role)), Some(Pressed::Box(under))) if role == under => {
                if state.open == Some(role) {
                    state.open = None;
                } else {
                    state.open = Some(role);
                    sounds.write(Sound::Ui(Ui::MenuDropdown));
                }
            }
            (Some(Pressed::Line(role, line)), Some(Pressed::Line(under, under_line))) if role == under && line == under_line => {
                if let Some(clothing) = clothing.as_mut() {
                    if let Some(&at) = clothing.of(ROLES[role]).get(line) {
                        clothing.choose(at);
                    }
                }
                sounds.write(Sound::Ui(Ui::MenuDropdownSelect));
                state.open = None;
            }
            _ => {}
        }
        state.turning = None;
        state.holding = None;
    }
}

/// An object's place on the screen, by its path below the panel's top.
fn panel_rect(overall: &trp_data::screen::OverallPanel, path: &str, screen: [f32; 2]) -> Option<[f32; 4]> {
    overall.panel.find(path).map(|index| overall.panel.placed_in(index, screen).rect)
}

/// Draws the page afresh as what it shows changes, and tells the model's
/// view where it is and by which camera it is drawn.
#[allow(clippy::too_many_arguments)]
fn draw(
    mut commands: Commands,
    controller: Res<crate::controller::Controller>,
    selected: Res<crate::inventory_view::Selected>,
    look: Res<crate::health_view::HealthLook>,
    said: Res<crate::health_view::Words>,
    features: Res<crate::options_view::Features>,
    clothing: Option<Res<Clothing>>,
    window: Query<&Window, With<PrimaryWindow>>,
    page: Query<Entity, With<OverallPage>>,
    mut figure: ResMut<crate::figure_view::OverallFigure>,
    mut state: ResMut<OverallState>,
) {
    let Ok(page) = page.single() else { return };
    let Ok(window) = window.single() else { return };
    let (screen, s) = screen_of(window);
    if let Some(overall) = &look.overall {
        let view = panel_rect(overall, OVERALL_VIEW, screen).map(|[_, _, w, h]| [w, h]);
        if figure.rect != view {
            figure.rect = view;
        }
        if figure.camera != overall.camera {
            figure.camera = overall.camera;
        }
    }
    if !(controller.inventory_open && selected.0 == crate::inventory_view::OVERALL) {
        return;
    }
    let clothing = clothing.filter(|_| features.clothing);
    let drop = drop_of(&features, &look);
    let cursor = window.cursor_position().map(|at| [at.x / s, at.y / s - drop]);
    let names: [String; 2] = std::array::from_fn(|role| {
        let Some(clothing) = clothing.as_ref() else { return String::new() };
        // The suit asked for, else the first of the list (`Mathf.Max(
        // IndexOf, 0)`).
        let listed = clothing.of(ROLES[role]);
        clothing.wanted[role].or(listed.first().copied()).map(|at| clothing.suits[at].name.clone()).unwrap_or_default()
    });
    let lines: Vec<String> = match (state.open, clothing.as_ref()) {
        (Some(role), Some(clothing)) => clothing.of(ROLES[role]).into_iter().map(|at| clothing.suits[at].name.clone()).collect(),
        _ => Vec::new(),
    };
    let lit: [bool; 2] = std::array::from_fn(|role| {
        let (Some(overall), Some(cursor)) = (look.overall.as_ref(), cursor) else { return false };
        places(overall, role, screen).is_some_and(|at| inside(at.boxed, cursor) || (state.open == Some(role) && inside(at.open, cursor)))
    });
    // The bar's handle, tinted by its bar's state (`Selectable`'s colours
    // times its multiplier, laid on the handle's own colour).
    let handle = match (state.open, look.overall.as_ref()) {
        (Some(role), Some(overall)) => places(overall, role, screen).and_then(|at| {
            let (whole, shown) = at.heights(lines.len());
            let rect = handle_of(overall, screen, whole, shown, state.scroll[role])?;
            let bar = overall.list_scroll?.bar?;
            let own = bar.handle.and_then(|handle| overall.panel.nodes[handle].image).map_or([1.0; 4], |image| image.color);
            let over = cursor.is_some_and(|cursor| at.bar.is_some_and(|bar| inside(bar, cursor)));
            let tint = bar.tints.map_or([1.0; 4], |tints| {
                let t = if state.holding.is_some() { tints.pressed } else if over { tints.highlighted } else { tints.normal };
                t.map(|c| (c * tints.multiplier).min(1.0))
            });
            Some((rect, Color::srgba(own[0] * tint[0], own[1] * tint[1], own[2] * tint[2], own[3] * tint[3])))
        }),
        _ => None,
    };
    let loading = clothing.as_ref().is_some_and(|clothing| clothing.reading());
    let image = figure.image.clone();
    let key = format!("{screen:?} {names:?} {lit:?} {:?} {:?} {handle:?} {:?} {loading} {}", state.open, state.scroll, image.as_ref().map(|image| image.id()), features.tarkov_ui);
    if state.drawn.as_deref() == Some(key.as_str()) {
        return;
    }
    state.drawn = Some(key);
    let line_words: Vec<&str> = lines.iter().map(String::as_str).collect();
    let open = state.open.map(|role| OpenList { role, lines: &line_words, scroll: state.scroll[role], handle });
    let shows = OverallShows { names: [names[0].as_str(), names[1].as_str()], lit, open, figure: image, loading };
    commands.entity(page).despawn_related::<Children>().with_children(|page| {
        if features.tarkov_ui && look.tarkov_ui {
            crate::health_view::draw_overall(page, &look, &said, screen, &shows, s);
        } else {
            draw_own(page, &look, screen, &shows, clothing.is_some(), drop, s);
        }
    });
}

/// The app's own look of the page (`TARKOV UI` off): the model in its
/// view, the two dropdowns as plain boxes and their lists, and the
/// statistics' place dimmed, each where the game's page has it.
#[allow(clippy::too_many_arguments)]
fn draw_own(page: &mut ChildSpawnerCommands, look: &crate::health_view::HealthLook, screen: [f32; 2], shows: &OverallShows, clothing: bool, drop: f32, s: f32) {
    let Some(overall) = &look.overall else {
        page.spawn(words("The game's OVERALL tab could not be read from the install.", 12.0, FAINT));
        return;
    };
    let node = |[x, y, w, h]: [f32; 4]| Node { position_type: PositionType::Absolute, left: Val::Px(x * s), top: Val::Px((y + drop) * s), width: Val::Px(w * s), height: Val::Px(h * s), ..default() };
    // (Within the list's own box, not put down again.)
    let inner = |[x, y, w, h]: [f32; 4]| Node { position_type: PositionType::Absolute, left: Val::Px(x * s), top: Val::Px(y * s), width: Val::Px(w * s), height: Val::Px(h * s), ..default() };
    if let (Some(view), Some(figure)) = (panel_rect(overall, OVERALL_VIEW, screen), shows.figure.clone()) {
        page.spawn((node(view), ImageNode::new(figure)));
    }
    if let Some(stats) = panel_rect(overall, "RightSide", screen) {
        page.spawn((node(stats), BackgroundColor(PANEL.with_alpha(0.5)))).with_children(|stats| {
            stats.spawn((Node { margin: UiRect::all(Val::Px(12.0 * s)), ..default() }, words("STATISTICS: not kept (no profile)", 14.0 * s, FAINT)));
        });
    }
    if !clothing {
        return;
    }
    for role in 0..2 {
        let Some(at) = places(overall, role, screen) else { continue };
        let ground = if shows.lit[role] { LINE } else { PANEL };
        page.spawn((node(at.boxed), BackgroundColor(ground))).with_children(|boxed| {
            boxed.spawn((Node { margin: UiRect::left(Val::Px(8.0 * s)), align_self: AlignSelf::Center, ..default() }, words(shows.names[role], 14.0 * s, WORDS)));
        });
    }
    let Some(list) = &shows.open else { return };
    let Some(at) = places(overall, list.role, screen) else { return };
    page.spawn((Node { overflow: Overflow::clip(), ..node(at.area) }, BackgroundColor(Color::BLACK))).with_children(|area| {
        for (n, line) in list.lines.iter().enumerate() {
            let top = at.content[1] - at.area[1] + at.row.padding[2] + n as f32 * (at.line + at.row.spacing) - list.scroll;
            if top + at.line < 0.0 || top > at.area[3] {
                continue;
            }
            let rect = [at.content[0] - at.area[0] + at.row.padding[0], top, at.content[2] - at.row.padding[0] - at.row.padding[1], at.line];
            area.spawn((inner(rect), BackgroundColor(PANEL))).with_children(|row| {
                row.spawn((Node { margin: UiRect::left(Val::Px(6.0 * s)), align_self: AlignSelf::Center, ..default() }, words(*line, 13.0 * s, WORDS)));
            });
        }
    });
    if let Some((rect, color)) = list.handle {
        page.spawn((node(rect), BackgroundColor(color)));
    }
}

#[cfg(test)]
mod tests {
    use super::Places;

    /// A list's lines laid out in its content's column (its padding, the
    /// lines 28 high and 2 apart, as the scene has them), the one under the
    /// mouse found as it is scrolled; none between two lines.
    #[test]
    fn the_line_under_the_mouse_is_found() {
        let row = trp_data::screen::RowGroup { vertical: true, padding: [2.0, 1.0, 0.0, 0.0], alignment: 0, spacing: 2.0, expand: [true, false], control: [true, true] };
        let at = Places { boxed: [0.0; 4], open: [0.0, 100.0, 200.0, 122.0], area: [1.0, 101.0, 190.0, 120.0], content: [1.0, 101.0, 190.0, 0.0], bar: None, row, line: 28.0 };
        assert_eq!(at.line_at([50.0, 105.0], 0.0, 95), Some(0));
        assert_eq!(at.line_at([50.0, 131.0], 0.0, 95), Some(1));
        assert_eq!(at.line_at([50.0, 130.0], 0.0, 95), None);
        assert_eq!(at.line_at([50.0, 105.0], 60.0, 95), Some(2));
        assert_eq!(at.line_at([50.0, 300.0], 0.0, 95), None);
        assert_eq!(at.heights(95), (95.0 * 28.0 + 94.0 * 2.0, 120.0));
    }
}
