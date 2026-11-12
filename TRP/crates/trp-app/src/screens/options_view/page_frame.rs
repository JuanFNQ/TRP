//! The left column, the page's head and the foot with APPLY.

use super::*;

/// The column at the left: TRP and its version, the pages, and under them
/// the run's end and the page put back as it comes.
pub(super) fn left_column(parent: &mut ChildSpawnerCommands, look: &Look, state: &OptionsState) {
    let column = Node { width: look.u(340.0), flex_direction: FlexDirection::Column, padding: UiRect::top(look.u(22.0)), flex_shrink: 0.0, ..default() };
    parent.spawn(column).with_children(|column| {
        column.spawn(Node { align_items: AlignItems::Baseline, column_gap: look.u(14.0), ..default() }).with_children(|brand| {
            brand.spawn((look.text("TRP", 92.0, INK), TextLayout::new_with_no_wrap()));
            brand.spawn((look.text(format!("v{}", env!("TRP_VERSION")), 32.0, KHAKI), TextLayout::new_with_no_wrap()));
        });
        column.spawn((look.text("TUSHONKA RUNTIME PROJECT", 15.0, INK_DIM), TextLayout::new_with_no_wrap(), Node { margin: UiRect::top(look.u(-6.0)), ..default() }));
        column.spawn(Node { height: look.u(46.0), flex_shrink: 0.0, ..default() });
        for (index, name) in PAGES.into_iter().enumerate() {
            let chosen = index == state.page;
            let (rest, lit, ink) = if chosen { (CHOSEN, CHOSEN_LIT, INK_DARK) } else { (GROUND, GROUND_LIT, INK) };
            let node = Node {
                width: Val::Percent(100.0),
                height: look.u(52.0),
                padding: UiRect::horizontal(look.u(22.0)),
                margin: UiRect::bottom(look.u(4.0)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                flex_shrink: 0.0,
                ..default()
            };
            column.spawn((Click::Page(index), Button, Feedback, node, BackgroundColor(rest), Tint(rest, lit))).with_children(|item| {
                item.spawn((look.strong(name, 23.0, ink), TextLayout::new_with_no_wrap()));
                if chosen {
                    chevron(item, look, 10.0, 45.0, INK_DARK);
                }
            });
        }
        column.spawn(Node { flex_grow: 1.0, ..default() });
        let wide = |height: f32| Node { width: Val::Percent(100.0), height: look.u(height), margin: UiRect::top(look.u(8.0)), ..default() };
        // QUIT ends the run: everything here is already kept. RESET TO
        // DEFAULT puts the page shown back as it comes (`reset_page`).
        button(column, look, Click::Quit, "QUIT", wide(52.0), false, true);
        let node = Node { width: Val::Percent(100.0), height: look.u(44.0), margin: UiRect::top(look.u(10.0)), column_gap: look.u(12.0), padding: UiRect::left(look.u(18.0)), justify_content: JustifyContent::FlexStart, ..default() };
        column.spawn((Click::Reset, Button, Feedback, node, BackgroundColor(GROUND), Tint(GROUND, GROUND_LIT))).with_children(|reset| {
            reset.spawn(Node { height: Val::Percent(100.0), align_items: AlignItems::Center, column_gap: look.u(12.0), ..default() }).with_children(|middle| {
                // A ring, the game's "back as it was".
                let ring = Node { width: look.u(18.0), height: look.u(18.0), border: UiRect::all(look.u(2.0)), border_radius: BorderRadius::all(look.u(9.0)), ..default() };
                middle.spawn((ring, BorderColor { top: INK_DIM, right: INK_DIM, bottom: INK_DIM, left: Color::NONE }));
                middle.spawn((look.text("RESET TO DEFAULT", 16.0, INK_DIM), TextLayout::new_with_no_wrap()));
            });
        });
    });
}

/// The page's title; on the CONTROLS page its categories, its search
/// field and its columns' heads under it, which do not scroll.
pub(super) fn page_head(parent: &mut ChildSpawnerCommands, look: &Look, state: &OptionsState) {
    let node = Node { padding: UiRect::new(look.u(4.0), Val::Px(0.0), look.u(20.0), look.u(12.0)), flex_shrink: 0.0, ..default() };
    parent.spawn(node).with_children(|head| {
        head.spawn((look.text(PAGES[state.page], 40.0, INK), TextLayout::new_with_no_wrap()));
    });
    if state.page != CONTROLS_PAGE {
        return;
    }
    let row = Node { width: Val::Percent(100.0), height: look.u(46.0), column_gap: look.u(4.0), align_items: AlignItems::Stretch, margin: UiRect::bottom(look.u(8.0)), flex_shrink: 0.0, ..default() };
    parent.spawn((row, BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.36)))).with_children(|row| {
        for (index, name) in CATEGORIES.into_iter().enumerate() {
            let chosen = index == state.category && state.search.is_empty();
            let node = Node { padding: UiRect::horizontal(look.u(18.0)), ..default() };
            button(row, look, Click::Category(index), name, node, chosen, true);
        }
        row.spawn(Node { flex_grow: 1.0, ..default() });
        // The search field: what it holds, a mark where the next letter
        // goes while it has the keys, the game's magnifier after it.
        let focused = state.search_focused;
        let node = Node { width: look.u(300.0), padding: UiRect::horizontal(look.u(14.0)), border: UiRect::all(look.u(1.0)), align_items: AlignItems::Center, column_gap: look.u(10.0), flex_shrink: 0.0, ..default() };
        let edge = if focused { INK_DIM } else { EDGE };
        row.spawn((Click::Search, SearchField, Button, node, BackgroundColor(GROUND), Tint(GROUND, GROUND_LIT), BorderColor::all(edge))).with_children(|field| {
            field.spawn(Node { flex_grow: 1.0, overflow: Overflow::clip(), ..default() }).with_children(|words| {
                let (said, ink) = match (state.search.is_empty(), focused) {
                    (true, false) => ("Search keybind...".to_owned(), INK_FAINT),
                    (_, true) => (format!("{}|", state.search), INK),
                    (false, false) => (state.search.clone(), INK),
                };
                words.spawn((look.text(said, 16.0, ink), TextLayout::new_with_no_wrap()));
            });
            // A magnifier: a ring and its handle.
            field.spawn(Node { width: look.u(18.0), height: look.u(18.0), flex_shrink: 0.0, ..default() }).with_children(|glass| {
                let ring = Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), width: look.u(12.0), height: look.u(12.0), border: UiRect::all(look.u(2.0)), border_radius: BorderRadius::all(look.u(6.0)), ..default() };
                glass.spawn((ring, BorderColor::all(INK_DIM)));
                let handle = Node { position_type: PositionType::Absolute, left: look.u(11.0), top: look.u(9.0), width: look.u(2.5), height: look.u(8.0), ..default() };
                glass.spawn((handle, BackgroundColor(INK_DIM), UiTransform::from_rotation(Rot2::degrees(-45.0))));
            });
        });
    });
    if state.category == MOUSE_CATEGORY && state.search.is_empty() {
        return;
    }
    let heads = Node { width: Val::Percent(100.0), height: look.u(38.0), padding: UiRect::right(look.u(16.0)), border: UiRect::bottom(look.u(1.0)), flex_shrink: 0.0, ..default() };
    parent.spawn((heads, BackgroundColor(HEAD), BorderColor::all(EDGE))).with_children(|heads| {
        for (n, (share, name)) in COLUMNS.into_iter().zip(["ACTION", "KEY", "SECONDARY", "PRESS TYPE"]).enumerate() {
            let node = Node { width: Val::Percent(share), height: Val::Percent(100.0), padding: UiRect::horizontal(look.u(18.0)), align_items: AlignItems::Center, justify_content: if n == 0 { JustifyContent::FlexStart } else { JustifyContent::Center }, ..default() };
            heads.spawn(node).with_children(|cell| {
                cell.spawn((look.strong(name, 15.0, INK_DIM), TextLayout::new_with_no_wrap()));
            });
        }
    });
}

/// The page's foot, with no bar across it: at its right a box with the
/// game's mark for what is said (an i in a ring), amber while something
/// waits for a start and grey else; the CONTROLS page's CLEAR KEY, which
/// unbinds the key last clicked; and APPLY, which starts TRP again to take what waits (a
/// scripted run keeps nothing and does not restart). At its left, the
/// page's last note in amber where an action left one (a key taken from
/// another binding, a file not written), kept up to date by `hover`.
pub(super) fn footer(parent: &mut ChildSpawnerCommands, look: &Look, state: &OptionsState, bindings: &Bindings) {
    let node = Node { width: Val::Percent(100.0), height: look.u(64.0), margin: UiRect::top(look.u(10.0)), column_gap: look.u(16.0), align_items: AlignItems::Center, flex_shrink: 0.0, ..default() };
    parent.spawn(node).with_children(|foot| {
        foot.spawn(Node { flex_grow: 1.0, flex_basis: Val::Px(0.0), min_width: Val::Px(0.0), padding: UiRect::left(look.u(4.0)), overflow: Overflow::clip(), ..default() }).with_children(|words| {
            words.spawn((Footer, look.text(state.note.clone(), 16.0, AMBER)));
        });
        let waits = !state.waiting().is_empty();
        let node = Node { height: Val::Percent(100.0), padding: UiRect::axes(look.u(18.0), look.u(10.0)), column_gap: look.u(16.0), align_items: AlignItems::Center, flex_shrink: 0.0, ..default() };
        foot.spawn((node, BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.56)))).with_children(|bar| {
            let ink = if waits { AMBER } else { INK_DIM };
            let ring = Node { width: look.u(24.0), height: look.u(24.0), border: UiRect::all(look.u(2.0)), border_radius: BorderRadius::all(look.u(12.0)), justify_content: JustifyContent::Center, align_items: AlignItems::Center, flex_shrink: 0.0, ..default() };
            bar.spawn((ring, BorderColor::all(ink))).with_children(|ring| {
                ring.spawn(look.strong("i", 14.0, ink));
            });
            if state.page == CONTROLS_PAGE {
                let clearable = state.selected_cell.is_some_and(|cell| !cell_source(&bindings.controls, cell).is_empty());
                button(bar, look, Click::ClearKey, "CLEAR KEY", Node { width: look.u(180.0), height: look.u(44.0), ..default() }, false, clearable);
            }
            let can = state.file.is_some() && waits;
            button(bar, look, Click::Apply, "APPLY", Node { width: look.u(220.0), height: look.u(44.0), ..default() }, can, can);
        });
    });
}
