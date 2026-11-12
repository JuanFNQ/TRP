//! The controls page and its cells.

use super::*;

/// A line of the list of bindings: one way of an axis, or a binding.
#[derive(Clone, Copy)]
pub(super) enum Entry {
    Axis(usize, usize),
    Key(usize),
}

/// The list's lines in the game's order, each with its category: the
/// axes' ways first, then every binding the game's own screen shows.
pub(super) fn entries(controls: &Controls) -> Vec<(Entry, usize)> {
    let mut out: Vec<(Entry, usize)> = Vec::new();
    for (index, _) in controls.axes.iter().enumerate() {
        out.extend([(Entry::Axis(index, 0), 1), (Entry::Axis(index, 1), 1)]);
    }
    let mut category = 1;
    for (index, group) in controls.keys.iter().enumerate().filter(|(_, group)| !group.hidden()) {
        if let Some(at) = GROUP_STARTS.iter().position(|name| *name == group.name) {
            category = at + 2;
        }
        out.push((Entry::Key(index), category));
    }
    out
}

/// What a cell holds.
pub(super) fn cell_source(controls: &Controls, cell: Cell) -> Source {
    match cell {
        Cell::Key(group, variant) => controls.keys.get(group).map(|group| group.variants[variant].clone()).unwrap_or_default(),
        Cell::Axis(axis, pair, way) => controls.axes.get(axis).map(|axis| axis.pairs[pair][way].clone()).unwrap_or_default(),
    }
}

/// The name of the line a cell is on.
pub(super) fn cell_name(state: &OptionsState, controls: &Controls, cell: Cell) -> String {
    match cell {
        Cell::Key(group, _) => state.name_of(&controls.keys[group].name),
        Cell::Axis(axis, _, way) => axis_word(&controls.axes[axis].name, way).to_owned(),
    }
}

pub(super) fn axis_word(name: &str, way: usize) -> &'static str {
    AXIS_WORDS.iter().find(|(known, _)| *known == name).map_or(["+", "-"], |(_, ways)| *ways)[way]
}

/// The CONTROLS page's body: the bindings of the category chosen (or all
/// that match what is searched for), or the mouse's settings.
pub(super) fn controls_page(page: &mut ChildSpawnerCommands, look: &Look, state: &OptionsState, bindings: &Bindings) {
    let controls = &bindings.controls;

    if state.category == MOUSE_CATEGORY && state.search.is_empty() {
        for (n, kind) in [Slide::Sensitivity, Slide::Aiming, Slide::DoubleClick, Slide::Optic].into_iter().enumerate() {
            setting_row(page, look, n % 2 == 1, &state.said(kind.caption().0), false, |c| slider(c, look, kind, state.slider(kind, controls), 300.0));
        }
        for (axis, key) in ["Inverted X axis", "Inverted Y axis"].into_iter().enumerate() {
            setting_row(page, look, axis == 1, &state.said(key), false, |c| toggle(c, look, Toggle::Invert(axis), state.mouse.invert[axis]));
        }
        return;
    }
    let wanted = state.search.trim().to_lowercase();
    let lines: Vec<Entry> = entries(controls)
        .into_iter()
        .filter(|(entry, category)| {
            if !wanted.is_empty() {
                let (name, sources) = match *entry {
                    Entry::Axis(axis, way) => (axis_word(&controls.axes[axis].name, way).to_owned(), controls.axes[axis].pairs.iter().map(|pair| pair[way].clone()).collect::<Vec<_>>()),
                    Entry::Key(index) => (state.name_of(&controls.keys[index].name), controls.keys[index].variants.to_vec()),
                };
                name.to_lowercase().contains(&wanted) || sources.iter().any(|source| !source.is_empty() && keys::words(source).to_lowercase().contains(&wanted))
            } else {
                state.category == 0 || *category == state.category
            }
        })
        .map(|(entry, _)| entry)
        .collect();
    if lines.is_empty() {
        note_row(page, look, &format!("No action or key matches \"{}\".", state.search), INK_DIM);
        return;
    }
    let listened = state.listening.as_ref();
    for (n, entry) in lines.into_iter().enumerate() {
        let rest = if n % 2 == 1 { ROW_ALT } else { ROW };
        // A binding the app does nothing with yet has its name faint.
        let (name, built) = match entry {
            Entry::Axis(axis, way) => (axis_word(&controls.axes[axis].name, way).to_owned(), keys::BUILT.contains(&controls.axes[axis].name.as_str())),
            Entry::Key(index) => (state.name_of(&controls.keys[index].name), keys::BUILT.contains(&controls.keys[index].name.as_str())),
        };
        let node = Node { width: Val::Percent(100.0), height: look.u(42.0), padding: UiRect::right(look.u(16.0)), align_items: AlignItems::Stretch, flex_shrink: 0.0, ..default() };
        page.spawn((node, BackgroundColor(rest), Lit(rest), RelativeCursorPosition::default())).with_children(|row| {
            let column = |share: f32| Node { width: Val::Percent(share), padding: UiRect::axes(look.u(10.0), look.u(4.0)), align_items: AlignItems::Center, overflow: Overflow::clip(), ..default() };
            row.spawn(Node { padding: UiRect::left(look.u(18.0)), ..column(COLUMNS[0]) }).with_children(|cell| {
                cell.spawn((look.text(name, 17.0, if built { INK } else { INK_FAINT }), TextLayout::new_with_no_wrap()));
            });
            let key_cell = |row: &mut ChildSpawnerCommands, at: Cell, source: &Source| {
                let listening = listened.filter(|listening| listening.cell == at && !listening.done);
                let lit = listened.is_some_and(|listening| listening.cell == at);
                let (said, ink) = match listening {
                    Some(listening) if listening.heard.source.is_empty() => ("PRESS ANY KEY...".to_owned(), INK_DARK),
                    Some(listening) => (format!("{} + ...", keys::words(&listening.heard.source)), INK_DARK),
                    None if source.is_empty() => ("-".to_owned(), INK_FAINT),
                    None => (keys::words(source), if lit { INK_DARK } else { INK }),
                };
                let selected = state.selected_cell == Some(at);
                row.spawn(column(COLUMNS[1])).with_children(|outer| {
                    let (rest, hover) = if lit { (CHOSEN, CHOSEN_LIT) } else { (Color::srgba(0.0, 0.0, 0.0, 0.42), GROUND_LIT) };
                    let node = Node { width: Val::Percent(100.0), height: Val::Percent(100.0), border: UiRect::all(look.u(1.0)), justify_content: JustifyContent::Center, align_items: AlignItems::Center, overflow: Overflow::clip(), ..default() };
                    outer.spawn((Click::Key(at), Button, node, BackgroundColor(rest), Tint(rest, hover), BorderColor::all(if selected { AMBER } else { EDGE }))).with_children(|inside| {
                        inside.spawn((look.strong(said, 16.0, ink), TextLayout::new_with_no_wrap()));
                    });
                });
            };
            match entry {
                Entry::Axis(axis, way) => {
                    key_cell(row, Cell::Axis(axis, 0, way), &controls.axes[axis].pairs[0][way]);
                    key_cell(row, Cell::Axis(axis, 1, way), &controls.axes[axis].pairs[1][way]);
                    row.spawn(column(COLUMNS[3]));
                }
                Entry::Key(index) => {
                    let group = &controls.keys[index];
                    key_cell(row, Cell::Key(index, 0), &group.variants[0]);
                    // How it is pressed: chosen from the four where the
                    // game lets it be chosen (its list of them, laid over
                    // the second key's cell while it is open), and not
                    // shown for a binding with no keys.
                    if state.asking == Some(index) {
                        row.spawn(Node { column_gap: look.u(4.0), justify_content: JustifyContent::FlexEnd, ..column(COLUMNS[2] + COLUMNS[3]) }).with_children(|choices| {
                            for press in PressType::ALL {
                                let node = Node { padding: UiRect::horizontal(look.u(10.0)), height: Val::Percent(100.0), ..default() };
                                button(choices, look, Click::PressChoice(press), &state.press_word(press), node, press == group.press, true);
                            }
                        });
                        return;
                    }
                    key_cell(row, Cell::Key(index, 1), &group.variants[1]);
                    row.spawn(Node { justify_content: JustifyContent::Center, ..column(COLUMNS[3]) }).with_children(|cell| {
                        if group.is_empty() {
                            return;
                        }
                        let said = state.press_word(group.press);
                        if group.press_shown() {
                            let node = Node { width: Val::Percent(100.0), height: Val::Percent(100.0), justify_content: JustifyContent::Center, align_items: AlignItems::Center, column_gap: look.u(10.0), ..default() };
                            cell.spawn((Click::Press(index), Button, node, BackgroundColor(Color::NONE), Tint(Color::NONE, GROUND_LIT))).with_children(|inside| {
                                inside.spawn((look.text(said, 16.0, INK), TextLayout::new_with_no_wrap()));
                                chevron(inside, look, 7.0, 135.0, INK_DIM);
                            });
                        } else {
                            cell.spawn((look.text(said, 16.0, INK_FAINT), TextLayout::new_with_no_wrap()));
                        }
                    });
                }
            }
        });
    }
}
