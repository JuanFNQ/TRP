//! The page's state, its gradual fill, scrolling and the mouse heard.

use super::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(super) enum View {
    #[default]
    List,
    Grid,
}

/// The caption's two tabs (`SkillsAndMasteringScreen`'s `TabGroup`: SKILLS
/// at first, then the one last chosen, the group being made once,
/// `Awake`, and shown with none asked, `Show(null)`).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(super) enum PageTab {
    #[default]
    Skills,
    Mastering,
}

/// What the left button went down on.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum Pressed {
    Box(usize),
    Line(usize, usize),
    Tab(View),
}

/// What the mouse is on that shows a tooltip: a skill's icon (its
/// tooltip), a buff's icon in a skill's view.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum Hover {
    Skill(Skill),
    Buff(Skill, usize),
}

/// The bar's handle held: the page's, or a dropdown's list's.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum Held {
    Page,
    List(usize),
}

/// The page as it stands: its view, each dropdown's line chosen (the
/// filter's, the sort's), the list open, each list's scroll and the page's,
/// what the left button went down on, the handle held (the mouse's offset
/// from its middle), what the mouse is on, where the last drawing put what
/// shows a tooltip, whether the page showed on the frame before, and what
/// was drawn.
#[derive(Resource, Default)]
pub(super) struct SkillsState {
    pub(super) tab: PageTab,
    caption_pressed: Option<PageTab>,
    /// The MASTERING page's own, and the page's panel with its lists
    /// fitted to their lines' counts (`mastery_view::fit_lists`).
    pub(super) mastery: crate::screens::mastery_view::MasteryState,
    pub(super) fitted: Option<([usize; 2], ScenePanel)>,
    pub(super) view: View,
    pub(super) choice: [usize; 2],
    pub(super) open: Option<usize>,
    pub(super) list_scroll: [f32; 2],
    pub(super) scroll: f32,
    pub(super) pressed: Option<Pressed>,
    pub(super) holding: Option<(Held, f32)>,
    pub(super) hover: Option<Hover>,
    pub(super) targets: Vec<([f32; 4], Hover)>,
    showed: bool,
    pub(super) own_scroll: f32,
    pub(super) page: Option<Entity>,
    pub(super) drawn: Option<String>,
    pub(super) tip_drawn: Option<String>,
    pub(super) tip_size: [f32; 2],
    pub(super) fill: Fill,
}

/// The views of the list or the grid as the game makes them
/// (`SkillContainer.Show`, `UI.AddViewListAsync`, `AsyncViewList.
/// CreateViews`): anew each time either shows, one a frame in the game's
/// order after a `Task.Yield`, the first on the frame it shows
/// (`courses10`'s `inventory-tabs` and `health-skills`: one view on the
/// click's frame, all 43 forty-two frames later, in the list and the grid
/// alike). A sort but the first chosen while some are to make ends the
/// making (`TemplateList.OrderByDescending` keeps only the views made in
/// its order, and the making's next step reads past its end and fails); a
/// filter reaches only the views made
/// when it is chosen (`FilterBy`), those made after show whatever their
/// class. The page's bar goes by the views laid out the frame before
/// (`ScrollRect.LateUpdate` runs before the frame's layout: the same takes,
/// the bar shown and resized a frame after each row, gone a frame after a
/// filter leaves the list short).
#[derive(Default)]
pub(super) struct Fill {
    made: usize,
    stopped: bool,
    filtered_at: Option<usize>,
    pub(super) laid_out: usize,
}

impl Fill {
    /// The list or the grid shown anew: none made, none laid out.
    fn restart(&mut self) {
        *self = Fill::default();
    }

    /// The other view shown with the dropdowns' lines as they stand: each
    /// is told its line at once, before any view is made (`BindEvent`), so
    /// a filter reaches none of them and a sort but the first leaves none
    /// to make (`PROVISIONAL`: read from the code, not seen in a take; both
    /// were at their first lines whenever a view was shown).
    pub(super) fn switched(&mut self, choice: [usize; 2]) {
        self.restart();
        if choice[0] != 0 {
            self.filtered_at = Some(0);
        }
        if choice[1] != 0 {
            self.stopped = true;
        }
    }

    /// A frame's view made, the first on the frame it shows.
    pub(super) fn step(&mut self) {
        if !self.stopped {
            self.made = (self.made + 1).min(shown().count());
        }
    }

    pub(super) fn done(&self) -> bool {
        self.made >= shown().count()
    }

    /// A line of a dropdown chosen: the filter's (role 0) reaches the views
    /// made; a sort's but the first ends the making.
    pub(super) fn chosen(&mut self, role: usize, line: usize) {
        let making = !self.done() && !self.stopped;
        match role {
            0 => self.filtered_at = making.then_some(self.made),
            _ if line != 0 && making => self.stopped = true,
            _ => {}
        }
    }

    /// The skills drawn: those listed whose views are made and shown.
    #[cfg(test)]
    pub(super) fn listed(&self, levels: &SkillLevels, filter: usize, sort: usize) -> Vec<Skill> {
        self.listed_by(levels, filter, sort, &[])
    }

    pub(super) fn listed_by(&self, levels: &SkillLevels, filter: usize, sort: usize, recent: &[Option<f64>]) -> Vec<Skill> {
        let class = filter.checked_sub(1).map(|n| CLASSES[n]);
        let order: Vec<Skill> = shown().collect();
        listed_by(levels, 0, sort, recent)
            .into_iter()
            .filter(|skill| {
                let at = order.iter().position(|s| s == skill).unwrap_or(usize::MAX);
                at < self.made && (class.is_none_or(|class| skill.class().name() == class) || self.filtered_at.is_some_and(|from| at >= from))
            })
            .collect()
    }
}

/// The dropdowns' lines: the filter's, the sort's.
pub(super) fn lines_of(said: &Words, role: usize) -> Vec<String> {
    match role {
        0 => std::iter::once(said.word("All types", "All types")).chain(CLASSES.iter().map(|class| said.word(class, class))).collect(),
        _ => SORTS.iter().map(|sort| said.word(sort, sort)).collect(),
    }
}

/// Where the page's scrolled things lie: the view (its mask), the first
/// cell's corner unscrolled, the grid, its width, and how high all of it
/// is and how much shows.
pub(super) struct Scrolled {
    pub(super) view: [f32; 4],
    pub(super) corner: [f32; 2],
    pub(super) grid: trp_data::interface::screen::CellGrid,
    pub(super) width: f32,
    pub(super) whole: f32,
    pub(super) shown: f32,
}

pub(super) fn scrolled(look: &SkillsLook, view: View, count: usize, screen: [f32; 2]) -> Option<Scrolled> {
    let panel = &look.page.panel;
    let view_at = panel.find(SKILL_SCROLL)?;
    let content = panel.find(&format!("{SKILL_SCROLL}/Content"))?;
    let rect = panel.placed_in(view_at, screen).rect;
    let [cx, cy, cw, _] = panel.placed_in(content, screen).rect;
    let row = panel.nodes[content].group.unwrap_or_default();
    let grid = match view {
        View::List => look.page.list_grid?,
        View::Grid => look.page.thumb_grid?,
    };
    let width = cw - row.padding[0] - row.padding[1];
    let whole = row.padding[2] + row.padding[3] + grid.height(count, width);
    Some(Scrolled { view: rect, corner: [cx + row.padding[0], cy + row.padding[2]], grid, width, whole, shown: rect[3] })
}

/// The page's places on a screen so large: each dropdown's.
pub(super) fn places(look: &SkillsLook, role: usize, screen: [f32; 2]) -> Option<Places> {
    dropdown_places(&look.page.panel, SKILL_DROPDOWNS[role], look.page.line.as_ref(), screen)
}

pub(super) fn panel_rect(look: &SkillsLook, path: &str, screen: [f32; 2]) -> Option<[f32; 4]> {
    look.page.panel.find(path).map(|index| look.page.panel.placed_in(index, screen).rect)
}

/// The game's sounds, the mouse and the wheel on the page.
#[allow(clippy::too_many_arguments)]
pub(super) fn hear(
    controller: Res<crate::player::controller::Controller>,
    selected: Res<crate::screens::inventory_view::Selected>,
    look: Res<HealthLook>,
    features: Res<crate::screens::options_view::Features>,
    window: Query<&Window, With<PrimaryWindow>>,
    buttons: Res<ButtonInput<MouseButton>>,
    wheel: Res<bevy::input::mouse::AccumulatedMouseScroll>,
    mut own_list: Query<&mut ScrollPosition, With<OwnList>>,
    mut state: ResMut<SkillsState>,
    mut sounds: MessageWriter<Sound>,
    (mastery, carried, game_data, mut typed, mut bindings, said, clock, keys): (Option<Res<crate::player::skills::Mastery>>, Res<crate::screens::inventory_view::Carried>, Res<crate::GameData>, MessageReader<bevy::input::keyboard::KeyboardInput>, ResMut<crate::platform::keys::Bindings>, Res<Words>, Res<Time<Real>>, Res<ButtonInput<KeyCode>>),
) {
    let health: &HealthLook = &look;
    let shows = controller.inventory_open && selected.0 == crate::screens::inventory_view::SKILLS;
    let state = &mut *state;
    if !shows {
        (state.open, state.pressed, state.holding, state.hover, state.showed, state.caption_pressed) = (None, None, None, None, false, None);
        state.mastery.left();
        if bindings.searching {
            bindings.searching = false;
        }
        typed.clear();
        return;
    }
    let game = look.skills.as_ref().filter(|_| features.tarkov_ui && look.tarkov_ui);
    // Shown anew (`SkillsAndMasteringScreen.Show`: the caption's tab last
    // chosen; `SkillsScreen.Show` or `MasteringScreen.Show`): its view last
    // chosen, list or grid (each page's tab group is made once, `Awake`,
    // and shown with none asked: `courses10`'s `health-skills` opened on the
    // grid), its views made anew, the dropdowns at their first lines, each
    // with its sound (`UpdateValue`).
    if !state.showed {
        state.showed = true;
        (state.choice, state.scroll, state.list_scroll) = ([0, 0], 0.0, [0.0, 0.0]);
        state.fill.restart();
        match (state.tab, game.is_some_and(|look| look.mastery.is_some())) {
            (PageTab::Mastering, true) => state.mastery.shown(&mut sounds),
            _ => {
                state.tab = PageTab::Skills;
                state.mastery.left();
                if game.is_some() {
                    sounds.write(Sound::Ui(Ui::MenuDropdownSelect));
                    sounds.write(Sound::Ui(Ui::MenuDropdownSelect));
                }
            }
        }
    }
    let notches = match wheel.unit {
        bevy::input::mouse::MouseScrollUnit::Line => wheel.delta.y,
        bevy::input::mouse::MouseScrollUnit::Pixel => wheel.delta.y.signum(),
    };
    let Some(look) = game else {
        // The app's own list, scrolled by the wheel.
        if notches != 0.0 {
            for mut at in &mut own_list {
                at.0.y = (at.0.y - notches * 40.0).max(0.0);
                state.own_scroll = at.0.y;
            }
        }
        return;
    };
    let Ok(window) = window.single() else { return };
    let (screen, s) = screen_of(window);
    let Some(cursor_px) = window.cursor_position() else { return };
    let cursor = [cursor_px.x / s, cursor_px.y / s];
    // The caption's tabs (`Tab.OnPointerClick`: a click on the other shows
    // its page, as it shows anew: `MasteringScreen.Show`, `SkillsScreen.
    // Show`). The MASTERING tab where its page was read.
    let caption = |at: [f32; 2]| {
        [(PageTab::Skills, CAPTION_SKILLS), (PageTab::Mastering, CAPTION_MASTERING)]
            .into_iter()
            .filter(|(tab, _)| *tab == PageTab::Skills || look.mastery.is_some())
            .find(|(_, path)| panel_rect(look, path, screen).is_some_and(|rect| inside(rect, at)))
            .map(|(tab, _)| tab)
    };
    if buttons.just_pressed(MouseButton::Left) {
        state.caption_pressed = caption(cursor);
    }
    if buttons.just_released(MouseButton::Left) {
        if let (Some(pressed), Some(under)) = (state.caption_pressed.take(), caption(cursor)) {
            if pressed == under && under != state.tab {
                state.tab = under;
                (state.open, state.pressed, state.holding, state.hover) = (None, None, None, None);
                match under {
                    PageTab::Mastering => state.mastery.shown(&mut sounds),
                    PageTab::Skills => {
                        state.mastery.left();
                        (state.choice, state.scroll, state.list_scroll) = ([0, 0], 0.0, [0.0, 0.0]);
                        state.fill.restart();
                        sounds.write(Sound::Ui(Ui::MenuDropdownSelect));
                        sounds.write(Sound::Ui(Ui::MenuDropdownSelect));
                    }
                }
                return;
            }
        }
    }
    // The MASTERING page has the mouse and the keys while chosen.
    if let (PageTab::Mastering, Some(page)) = (state.tab, look.mastery.as_ref()) {
        let rows = mastery.as_deref().map(|mastery| crate::screens::mastery_view::rows(mastery, &carried.texts, &game_data.db)).unwrap_or_default();
        fit(&mut state.fitted, look, page, crate::screens::mastery_view::counts(&said, &rows));
        let panel = &state.fitted.as_ref().expect("fitted").1;
        let input = crate::screens::mastery_view::FieldInput { health, keys: &keys, pixels: s };
        crate::screens::mastery_view::hear(panel, page, &said, &rows, &mut state.mastery, cursor, screen, &buttons, notches, &mut typed, &mut sounds, clock.elapsed_secs(), input);
        if bindings.searching != state.mastery.field.focused {
            bindings.searching = state.mastery.field.focused;
        }
        return;
    }
    typed.clear();
    if bindings.searching {
        bindings.searching = false;
    }
    // (The scroll view goes by its views as laid out the frame before.)
    let count = state.fill.laid_out;
    let counts = [CLASSES.len() + 1, SORTS.len()];
    let places: [Option<Places>; 2] = std::array::from_fn(|role| places(look, role, screen));
    let over_box = |role: usize| places[role].as_ref().is_some_and(|at| inside(at.boxed, cursor));
    let over_list = |open: Option<usize>, role: usize| open == Some(role) && places[role].as_ref().is_some_and(|at| inside(at.open, cursor));
    let hovering = state.open.is_some_and(|role| over_box(role) || over_list(state.open, role));
    let page = scrolled(look, state.view, count, screen);
    let most = page.as_ref().map_or(0.0, |page| (page.whole - page.shown).max(0.0));
    state.scroll = state.scroll.clamp(0.0, most);
    // Any button pressed off the box and its list shuts it.
    if buttons.get_just_pressed().next().is_some() && state.open.is_some() && !hovering {
        state.open = None;
    }
    // The wheel: the open list's, else the page's.
    if notches != 0.0 {
        let list = state.open.filter(|role| places[*role].as_ref().is_some_and(|at| inside(at.area, cursor)));
        match (list, look.page.list_scroll) {
            (Some(role), Some(scrolling)) => {
                let (whole, shown) = places[role].as_ref().expect("its places").heights(counts[role]);
                state.list_scroll[role] = (state.list_scroll[role] - notches * scrolling.sensitivity).clamp(0.0, (whole - shown).max(0.0));
            }
            _ => {
                if let (Some(page), Some(scrolling)) = (page.as_ref(), look.page.view_scroll) {
                    if inside(page.view, cursor) && state.open.is_none() {
                        state.scroll = (state.scroll - notches * scrolling.sensitivity).clamp(0.0, most);
                    }
                }
            }
        }
    }
    let view_tab = |at: [f32; 2]| [View::List, View::Grid].into_iter().zip(VIEW_TABS).find(|(_, path)| panel_rect(look, path, screen).is_some_and(|rect| inside(rect, at))).map(|(view, _)| view);
    let page_handle = |scroll: f32| -> Option<[f32; 4]> {
        let page = page.as_ref()?;
        bar_handle(&look.page.panel, look.page.view_scroll?.bar?, screen, page.whole, page.shown, scroll)
    };
    let list_handle = |role: usize, scroll: f32| -> Option<[f32; 4]> {
        let (whole, shown) = places[role].as_ref()?.heights(counts[role]);
        bar_handle(&look.page.panel, scrolling_bar(look, role)?, screen, whole, shown, scroll)
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
        } else if let Some([_, hy, _, hh]) = page_handle(state.scroll).filter(|handle| inside(*handle, cursor) && most > 0.0) {
            state.holding = Some((Held::Page, cursor[1] - (hy + hh / 2.0)));
        }
    }
    // The handle held: what it scrolls follows it.
    if let Some((held, offset)) = state.holding {
        let (bar, handle, whole_less_shown) = match held {
            Held::Page => (look.page.view_scroll.and_then(|scroll| scroll.bar), page_handle(state.scroll), most),
            Held::List(role) => {
                let (whole, shown) = places[role].as_ref().map_or((0.0, 0.0), |at| at.heights(counts[role]));
                (scrolling_bar(look, role), list_handle(role, state.list_scroll[role]), (whole - shown).max(0.0))
            }
        };
        let sliding = bar.and_then(|bar| bar.sliding).and_then(|sliding| {
            let path = &look.page.panel.nodes[sliding].path;
            panel_rect(look, path, screen)
        });
        if let (Some([_, sy, _, sh]), Some([_, _, _, hh])) = (sliding, handle) {
            let share = ((cursor[1] - offset - hh / 2.0 - sy) / (sh - hh).max(1e-3)).clamp(0.0, 1.0);
            match held {
                Held::Page => state.scroll = share * whole_less_shown,
                Held::List(role) => state.list_scroll[role] = share * whole_less_shown,
            }
        }
    }
    if buttons.just_released(MouseButton::Left) {
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
                state.fill.chosen(role, line);
                sounds.write(Sound::Ui(Ui::MenuDropdownSelect));
                state.open = None;
            }
            // The other view shown (`Tab.OnPointerClick`), its views made
            // anew; none for the one shown.
            (Some(Pressed::Tab(view)), Some(Pressed::Tab(under))) if view == under && view != state.view => {
                state.view = view;
                state.fill.switched(state.choice);
            }
            _ => {}
        }
        state.holding = None;
    }
    // What the mouse is on, by where the last drawing put things; none
    // under an open list, none outside the view's mask.
    let over_open = state.open.is_some_and(|role| over_list(state.open, role) || over_box(role));
    let in_view = page.as_ref().is_some_and(|page| inside(page.view, cursor));
    let hover = (!over_open && in_view && state.holding.is_none()).then(|| state.targets.iter().find(|(rect, _)| inside(*rect, cursor)).map(|(_, hover)| *hover)).flatten();
    if state.hover != hover {
        state.hover = hover;
    }
}

/// The page's panel with the MASTERING page's lists fitted to their
/// lines' counts (`mastery_view::fit_lists`), kept until a count changes.
pub(super) fn fit(fitted: &mut Option<([usize; 2], ScenePanel)>, look: &SkillsLook, page: &crate::screens::mastery_view::MasteryLook, counts: [usize; 2]) {
    if fitted.as_ref().is_none_or(|(kept, _)| *kept != counts) {
        let mut panel = look.page.panel.clone();
        crate::screens::mastery_view::fit_lists(&mut panel, page, counts);
        *fitted = Some((counts, panel));
    }
}

/// A dropdown's list's bar.
pub(super) fn scrolling_bar(look: &SkillsLook, role: usize) -> Option<trp_data::interface::screen::ScrollBar> {
    let panel = &look.page.panel;
    let area = panel.find(&format!("{}/Open Panel/Scroll Area", SKILL_DROPDOWNS[role]))?;
    trp_data::interface::screen::Scrolling::of(panel, area, "ScrollRectNoDrag")?.bar
}
