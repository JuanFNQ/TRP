//! The page's state, the mouse and the keys.

use super::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(crate) enum View {
    #[default]
    List,
    Grid,
}

/// What the left button went down on.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum Pressed {
    Box(usize),
    Line(usize, usize),
    Tab(View),
    Search,
}

/// The bar's handle held: the page's, or a dropdown's list's.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum Held {
    Page,
    List(usize),
}

/// The page as it stands: its view, each dropdown's line chosen (the
/// filter's, the sort's), the list open, each list's scroll and the
/// page's, what the left button went down on, the handle held, the search
/// field (what is typed in it, its caret and selection, whether it has the
/// keys: `text_field`), the row under the mouse in the grid and where the
/// last drawing put each row's icon, each weapon's picture's key and cells
/// (its default build's), and the tooltip's size.
#[derive(Default)]
pub(crate) struct MasteryState {
    pub(crate) view: View,
    pub(super) choice: [usize; 2],
    pub(super) open: Option<usize>,
    pub(super) list_scroll: [f32; 2],
    pub(super) scroll: f32,
    pub(super) pressed: Option<Pressed>,
    pub(super) holding: Option<(Held, f32)>,
    pub(crate) field: TextField,
    pub(super) hover: Option<String>,
    pub(super) targets: Vec<([f32; 4], String)>,
    pub(super) pictures: HashMap<String, Option<(String, [u32; 2])>>,
    /// Each weapon's picture turned as the page turns it, by the picture
    /// it was made from (`slanted`).
    pub(super) slanted: HashMap<AssetId<Image>, Option<(Handle<Image>, [u32; 2])>>,
    pub(super) tip_size: [f32; 2],
}

impl MasteryState {
    /// The page shown (`MasteringScreen.Show`): its view last chosen, list
    /// or grid (its tab group made once, `Awake`, and shown with none
    /// asked: `courses10`'s `health-skills` opened MASTERING on its grid),
    /// the dropdowns at their first lines, each with its sound
    /// (`UpdateValue`); what is typed stays (the field is not emptied).
    pub(crate) fn shown(&mut self, sounds: &mut MessageWriter<Sound>) {
        (self.choice, self.scroll, self.list_scroll, self.open, self.pressed, self.holding, self.hover) = ([0, 0], 0.0, [0.0, 0.0], None, None, None, None);
        sounds.write(Sound::Ui(Ui::MenuDropdownSelect));
        sounds.write(Sound::Ui(Ui::MenuDropdownSelect));
    }

    /// The page left: nothing held, the field without the keys.
    pub(crate) fn left(&mut self) {
        (self.open, self.pressed, self.holding, self.hover) = (None, None, None, None);
        self.field.blur();
    }
}

/// The page's panel with each dropdown's list as high as it shows of its
/// lines (at most `_maxVisibleHeight`, `DropDownBox.Update`): the filter's
/// lines are as many as the rows' types and one.
pub(crate) fn fit_lists(panel: &mut ScenePanel, look: &MasteryLook, counts: [usize; 2]) {
    let most_seen = look.screen.dropdown_most_seen.unwrap_or(120.0);
    let line_height = look.screen.line.as_ref().map_or(28.0, |line| line.asks(0, 1).0.max(1.0));
    for (selector, count) in MASTERING_DROPDOWNS.iter().zip(counts) {
        let row = panel.find(&format!("{selector}/Open Panel/Scroll Area/Content")).and_then(|content| panel.nodes[content].group).unwrap_or_default();
        let whole = row.padding[2] + row.padding[3] + count as f32 * line_height + count.saturating_sub(1) as f32 * row.spacing;
        if let Some(area) = panel.find(&format!("{selector}/Open Panel/Scroll Area")) {
            let element = panel.nodes[area].element.get_or_insert(trp_data::interface::screen::Element { ignore: false, min: [-1.0, -1.0], preferred: [-1.0, -1.0], flexible: [-1.0, -1.0] });
            element.min[1] = whole.min(most_seen);
        }
    }
}

/// Where the page's scrolled rows lie: the view (its mask), the first
/// cell's corner unscrolled, the grid, its width, and how high all of it
/// is and how much shows.
pub(crate) struct Scrolled {
    pub(super) view: [f32; 4],
    pub(super) corner: [f32; 2],
    pub(super) grid: trp_data::interface::screen::CellGrid,
    pub(super) width: f32,
    pub(super) whole: f32,
    pub(super) shown: f32,
}

pub(super) fn scrolled(panel: &ScenePanel, look: &MasteryLook, view: View, count: usize, screen: [f32; 2]) -> Option<Scrolled> {
    let view_at = panel.find(MASTERING_SCROLL)?;
    let content = panel.find(&format!("{MASTERING_SCROLL}/Content"))?;
    let rect = panel.placed_in(view_at, screen).rect;
    let [cx, cy, cw, _] = panel.placed_in(content, screen).rect;
    let row = panel.nodes[content].group.unwrap_or_default();
    let grid = match view {
        View::List => look.screen.list_grid?,
        View::Grid => look.screen.thumb_grid?,
    };
    let width = cw - row.padding[0] - row.padding[1];
    let whole = row.padding[2] + row.padding[3] + grid.height(count, width);
    Some(Scrolled { view: rect, corner: [cx + row.padding[0], cy + row.padding[2]], grid, width, whole, shown: rect[3] })
}

pub(super) fn rect_of(panel: &ScenePanel, path: &str, screen: [f32; 2]) -> Option<[f32; 4]> {
    panel.find(path).map(|index| panel.placed_in(index, screen).rect)
}

/// A dropdown's list's bar.
pub(super) fn scrolling_bar(panel: &ScenePanel, role: usize) -> Option<trp_data::interface::screen::ScrollBar> {
    let area = panel.find(&format!("{}/Open Panel/Scroll Area", MASTERING_DROPDOWNS[role]))?;
    trp_data::interface::screen::Scrolling::of(panel, area, "ScrollRectNoDrag")?.bar
}

/// What a letter typed into the search field may be: anything printed.
fn search_letter(c: char) -> bool {
    !c.is_control()
}

/// The search field's own rules: its most letters and whether it selects
/// all as it takes the keys, as the install has them (20 and yes, `probe
/// masteringpage`); none of either where the field was not read.
fn search_rules(look: &MasteryLook) -> Rules {
    Rules { limit: look.screen.search.map_or(0, |field| field.limit as usize), keeps: search_letter, select_all: look.screen.search.is_some_and(|field| field.select_all) }
}

/// What the page's search field is typed and pressed with: the screens'
/// look (its font, for where its letters lie), the keys held, and the
/// screen's pixels to a unit.
pub(crate) struct FieldInput<'a> {
    pub(crate) health: &'a HealthLook,
    pub(crate) keys: &'a ButtonInput<KeyCode>,
    pub(crate) pixels: f32,
}

/// The page's mouse, wheel and keys, in screen units (`skills_view::hear`
/// hands them on while the MASTERING tab is chosen): the dropdowns, the
/// views' tabs, the bars' handles, the wheel over the rows or an open
/// list, the search field and what is typed into it, and the row under
/// the mouse in the grid. `panel` is the page's, its lists fitted
/// (`fit_lists`).
#[allow(clippy::too_many_arguments)]
pub(crate) fn hear(
    panel: &ScenePanel,
    look: &MasteryLook,
    said: &Words,
    rows: &[Row],
    state: &mut MasteryState,
    cursor: [f32; 2],
    screen: [f32; 2],
    buttons: &ButtonInput<MouseButton>,
    notches: f32,
    typed: &mut MessageReader<bevy::input::keyboard::KeyboardInput>,
    sounds: &mut MessageWriter<Sound>,
    now: f32,
    input: FieldInput,
) {
    let counts = [lines_of(said, rows, 0).len(), SORTS.len()];
    state.choice[0] = state.choice[0].min(counts[0] - 1);
    let count = listed(rows, state.choice, &state.field.text).len();
    let places: [Option<Places>; 2] = std::array::from_fn(|role| dropdown_places(panel, MASTERING_DROPDOWNS[role], look.screen.line.as_ref(), screen));
    let over_box = |role: usize| places[role].as_ref().is_some_and(|at| inside(at.boxed, cursor));
    let over_list = |open: Option<usize>, role: usize| open == Some(role) && places[role].as_ref().is_some_and(|at| inside(at.open, cursor));
    let hovering = state.open.is_some_and(|role| over_box(role) || over_list(state.open, role));
    let page = scrolled(panel, look, state.view, count, screen);
    let most = page.as_ref().map_or(0.0, |page| (page.whole - page.shown).max(0.0));
    state.scroll = state.scroll.clamp(0.0, most);
    let search_rect = rect_of(panel, MASTERING_SEARCH, screen);
    let over_search = search_rect.is_some_and(|rect| inside(rect, cursor));
    // Where the mouse is in the field's words (`FieldPlace::spot`), and
    // where it is on the screen in pixels; the end where the words' place
    // is not had.
    let (rules, mods) = (search_rules(look), crate::screens::text_field::mods(input.keys));
    let spot = |said: &str| {
        let place = look.screen.search.and_then(|field| field_place(input.health, panel, field.text, said, Frame::In(screen), 1.0));
        let end = said.chars().count();
        place.map_or(Spot { gap: end, letter: end, word: None }, |place| place.spot(said, cursor))
    };
    let cursor_px = [cursor[0] * input.pixels, cursor[1] * input.pixels];
    // Any button pressed off the box and its list shuts it; off the field,
    // the field lets the keys go.
    if buttons.get_just_pressed().next().is_some() {
        if state.open.is_some() && !hovering {
            state.open = None;
        }
        if !over_search {
            state.field.blur();
        }
    }
    if notches != 0.0 {
        let list = state.open.filter(|role| places[*role].as_ref().is_some_and(|at| inside(at.area, cursor)));
        match (list, look.screen.list_scroll) {
            (Some(role), Some(scrolling)) => {
                let (whole, shown) = places[role].as_ref().expect("its places").heights(counts[role]);
                state.list_scroll[role] = (state.list_scroll[role] - notches * scrolling.sensitivity).clamp(0.0, (whole - shown).max(0.0));
            }
            _ => {
                if let (Some(page), Some(scrolling)) = (page.as_ref(), look.screen.view_scroll) {
                    if inside(page.view, cursor) && state.open.is_none() {
                        state.scroll = (state.scroll - notches * scrolling.sensitivity).clamp(0.0, most);
                    }
                }
            }
        }
    }
    let view_tab = |at: [f32; 2]| [View::List, View::Grid].into_iter().zip(MASTERING_TABS).find(|(_, path)| rect_of(panel, path, screen).is_some_and(|rect| inside(rect, at))).map(|(view, _)| view);
    let page_handle = |scroll: f32| -> Option<[f32; 4]> {
        let page = page.as_ref()?;
        bar_handle(panel, look.screen.view_scroll?.bar?, screen, page.whole, page.shown, scroll)
    };
    let list_handle = |role: usize, scroll: f32| -> Option<[f32; 4]> {
        let (whole, shown) = places[role].as_ref()?.heights(counts[role]);
        bar_handle(panel, scrolling_bar(panel, role)?, screen, whole, shown, scroll)
    };
    if buttons.just_pressed(MouseButton::Left) {
        state.pressed = None;
        if let Some(role) = state.open.filter(|role| over_list(state.open, *role)) {
            let at = places[role].as_ref().expect("over its list");
            if let Some([_, hy, _, hh]) = list_handle(role, state.list_scroll[role]).filter(|handle| inside(*handle, cursor)) {
                state.holding = Some((Held::List(role), cursor[1] - (hy + hh / 2.0)));
            } else if let Some(line) = at.line_at(cursor, state.list_scroll[role], counts[role]) {
                state.pressed = Some(Pressed::Line(role, line));
            }
        } else if let Some(role) = (0..2).find(|&role| over_box(role)) {
            state.pressed = Some(Pressed::Box(role));
        } else if let Some(view) = view_tab(cursor) {
            state.pressed = Some(Pressed::Tab(view));
        } else if over_search {
            state.pressed = Some(Pressed::Search);
            // Taking the keys, the field selects all it says; a press on
            // it while it has them puts its caret under the mouse
            // (`TextField::press`).
            let at = spot(&state.field.text);
            state.field.press(at, cursor_px, mods, &rules, now);
        } else if let Some([_, hy, _, hh]) = page_handle(state.scroll).filter(|handle| inside(*handle, cursor) && most > 0.0) {
            state.holding = Some((Held::Page, cursor[1] - (hy + hh / 2.0)));
        }
    }
    if let Some((held, offset)) = state.holding {
        let (bar, handle, whole_less_shown) = match held {
            Held::Page => (look.screen.view_scroll.and_then(|scroll| scroll.bar), page_handle(state.scroll), most),
            Held::List(role) => {
                let (whole, shown) = places[role].as_ref().map_or((0.0, 0.0), |at| at.heights(counts[role]));
                (scrolling_bar(panel, role), list_handle(role, state.list_scroll[role]), (whole - shown).max(0.0))
            }
        };
        let sliding = bar.and_then(|bar| bar.sliding).and_then(|sliding| rect_of(panel, &panel.nodes[sliding].path, screen));
        if let (Some([_, sy, _, sh]), Some([_, _, _, hh])) = (sliding, handle) {
            let share = ((cursor[1] - offset - hh / 2.0 - sy) / (sh - hh).max(1e-3)).clamp(0.0, 1.0);
            match held {
                Held::Page => state.scroll = share * whole_less_shown,
                Held::List(role) => state.list_scroll[role] = share * whole_less_shown,
            }
        }
    }
    // The button held from a press on the field: its selection's end
    // follows the mouse (`TextField::drag`).
    if state.pressed == Some(Pressed::Search) && buttons.pressed(MouseButton::Left) && !buttons.just_pressed(MouseButton::Left) {
        let at = spot(&state.field.text);
        state.field.drag(at, cursor_px, crate::screens::inventory_view::DRAG_THRESHOLD);
    }
    if buttons.just_released(MouseButton::Left) {
        state.field.release();
        let now = if let Some(role) = state.open.filter(|role| over_list(state.open, *role)) {
            places[role].as_ref().and_then(|at| at.line_at(cursor, state.list_scroll[role], counts[role])).map(|line| Pressed::Line(role, line))
        } else if let Some(role) = (0..2).find(|&role| over_box(role)) {
            Some(Pressed::Box(role))
        } else {
            view_tab(cursor).map(Pressed::Tab)
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
                state.choice[role] = line;
                sounds.write(Sound::Ui(Ui::MenuDropdownSelect));
                state.open = None;
            }
            (Some(Pressed::Tab(view)), Some(Pressed::Tab(under))) if view == under => state.view = view,
            _ => {}
        }
        state.holding = None;
    }
    // The keys, to the field while it has them (`TextField::key`: the
    // inventory spec, "A text field"); a key after Enter goes nowhere.
    for key in typed.read().filter(|key| key.state.is_pressed()) {
        if let (true, Some(key)) = (state.field.focused, field_key(key, mods)) {
            state.field.key(&key, mods, &rules, now);
        }
    }
    // The row under the mouse in the grid, by where the last drawing put
    // its icon; none under an open list, outside the view's mask, or with
    // a handle held.
    let over_open = state.open.is_some_and(|role| over_list(state.open, role) || over_box(role));
    let in_view = page.as_ref().is_some_and(|page| inside(page.view, cursor));
    let hover = (!over_open && in_view && state.holding.is_none() && state.view == View::Grid).then(|| state.targets.iter().find(|(rect, _)| inside(*rect, cursor)).map(|(_, template)| template.clone())).flatten();
    if state.hover != hover {
        state.hover = hover;
    }
}
