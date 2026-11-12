//! The HUD and FEATURES pages' bodies, with the switches' rows.

use super::*;

/// The HUD page: of the game's own GAME tab, when the battle screen's
/// quick bar, stance panel and health panel show, and the health's
/// colours, in the game's words; then how the screens are drawn (the
/// app's own); and which of the notices of experience and of the weapon's
/// malfunctions show (the app's own: the game shows them all).
pub(super) fn hud_page(page: &mut ChildSpawnerCommands, look: &Look, state: &OptionsState, sizes: &[[u32; 2]]) {
    let shut = |at: usize| state.shut(HUD_PAGE, at);
    section_head(page, look, 0, shut(0), "IN-RAID INTERFACE", |_| {});
    if !shut(0) {
        let rows = [(Step::Quick, "Ui/Settings/QuickSlotsVisibility"), (Step::Stamina, "Ui/Settings/StanimaVisibility"), (Step::Health, "Ui/Settings/HeathVisibility"), (Step::Colors, "Ui/Settings/HealthColorScheme")];
        for (n, (step, key)) in rows.into_iter().enumerate() {
            setting_row(page, look, n % 2 == 1, &state.said(key), false, |control| stepper(control, look, step, step_choices(state, step, sizes)));
        }
    }
    // The screens' switch first, then HIDE UNUSED TABS, the app's own.
    let rows = HUD_SWITCHES.len() + 1;
    let on = HUD_SWITCHES.iter().filter(|switch| state.switches.on(**switch)).count() + usize::from(state.hide_tabs);
    section_head(page, look, 1, shut(1), "SCREENS", |head| {
        head.spawn((look.strong(format!("{on} / {rows} ON"), 15.0, count_ink(on, rows)), TextLayout::new_with_no_wrap()));
    });
    if !shut(1) {
        for (n, switch) in HUD_SWITCHES.into_iter().enumerate() {
            switch_row(page, look, state, switch, n % 2 == 1, false);
        }
        setting_switch_row(page, look, "HIDE UNUSED TABS", (Toggle::HideTabs, state.hide_tabs), HUD_SWITCHES.len() % 2 == 1);
    }
    let on = Notices::ALL.iter().filter(|(which, ..)| state.notices.on(*which)).count();
    section_head(page, look, 2, shut(2), "NOTIFICATIONS", |head| {
        head.spawn((look.strong(format!("{on} / {} ON", Notices::ALL.len()), 15.0, count_ink(on, Notices::ALL.len())), TextLayout::new_with_no_wrap()));
    });
    if !shut(2) {
        for (n, (which, name, _)) in Notices::ALL.into_iter().enumerate() {
            setting_row(page, look, n % 2 == 1, name, false, |c| toggle(c, look, Toggle::Notice(which), state.notices.on(which)));
        }
    }
}

/// A section's count of what is on: green with all of it, dim with some,
/// faint with none.
fn count_ink(on: usize, of: usize) -> Color {
    match on {
        0 => INK_FAINT,
        _ if on == of => ON,
        _ => INK_DIM,
    }
}

/// The FEATURES page's last section, THE RANGE: the range's hour, at its
/// place `at`. Lit as Customs (TARKOV LIGHTING) the hour is the one the run
/// began at, and its bar is dim and does not slide.
fn range_section(page: &mut ChildSpawnerCommands, look: &Look, state: &OptionsState, at: usize) {
    section_head(page, look, at, state.shut(FEATURES_PAGE, at), "THE RANGE", |_| {});
    if state.shut(FEATURES_PAGE, at) {
        return;
    }
    if state.in_force.customs_light {
        let spec = Row { name: "TIME OF DAY", dim: true, tag: Some(("SET AT START", INK_DIM)), ..default() };
        row(page, look, false, spec, |control| slider_as(control, look, Slide::Hour, state.hour_started, 300.0, false));
    } else {
        // DEFAULT: back to the hour every run starts at (`RANGE_MOMENT`).
        setting_row(page, look, false, "TIME OF DAY", false, |control| {
            slider(control, look, Slide::Hour, state.hour, 300.0);
            small_button(control, look, Click::HourDefault, "DEFAULT");
        });
    }
}

/// The FEATURES page: the switches by their sections (EXPERIMENTAL, then
/// CHARACTER with the character's name under its switches), then THE
/// RANGE.
pub(super) fn features_page(page: &mut ChildSpawnerCommands, look: &Look, state: &OptionsState) {
    for (at, (name, switches)) in SECTIONS.into_iter().enumerate() {
        let folded = state.shut(FEATURES_PAGE, at);
        let on = switches.iter().filter(|switch| state.switches.on(**switch)).count();
        section_head(page, look, at, folded, name, |head| {
            head.spawn((look.strong(format!("{on} / {} ON", switches.len()), 15.0, count_ink(on, switches.len())), TextLayout::new_with_no_wrap()));
        });
        if folded {
            continue;
        }
        for (n, switch) in switches.iter().enumerate() {
            switch_row(page, look, state, *switch, n % 2 == 1, true);
        }
        if at == CHARACTER_SECTION {
            name_row(page, look, state, switches.len() % 2 == 1);
        }
    }
    range_section(page, look, state, SECTIONS.len());
}

/// CHARACTER's last row: the character's name, which the OVERALL tab
/// shows under the character, in a field typed into as the CONTROLS
/// page's search field is (`type_name`: up to `NAME_MOST` letters; Escape
/// or Enter lets the field go), taken at once.
fn name_row(page: &mut ChildSpawnerCommands, look: &Look, state: &OptionsState, stripe: bool) {
    setting_row(page, look, stripe, "CHARACTER NAME", false, |control| {
        let focused = state.name_focused;
        let node = Node { width: look.u(300.0), height: look.u(38.0), padding: UiRect::horizontal(look.u(14.0)), border: UiRect::all(look.u(1.0)), align_items: AlignItems::Center, flex_shrink: 0.0, ..default() };
        let edge = if focused { INK_DIM } else { EDGE };
        control.spawn((Click::Name, NameField, Button, node, BackgroundColor(GROUND), Tint(GROUND, GROUND_LIT), BorderColor::all(edge))).with_children(|field| {
            field.spawn(Node { flex_grow: 1.0, overflow: Overflow::clip(), ..default() }).with_children(|words| {
                let (said, ink) = match (state.character_name.is_empty(), focused) {
                    (true, false) => ("No name".to_owned(), INK_FAINT),
                    (_, true) => (format!("{}|", state.character_name), INK),
                    (false, false) => (state.character_name.clone(), INK),
                };
                words.spawn((look.text(said, 16.0, ink), TextLayout::new_with_no_wrap()));
            });
        });
    });
}

/// A switch's row: its box (the whole row a button), its name, on the
/// FEATURES page what it does (`explained`), and at its end what it needs
/// where that is off, and RESTART where it was thrown since the run began.
/// Where a need is off, or it is not built (`Switch::built`, tagged NOT
/// BUILT), it is drawn dim and does nothing.
fn switch_row(page: &mut ChildSpawnerCommands, look: &Look, state: &OptionsState, switch: Switch, stripe: bool, explained: bool) {
    let (name, what) = switch.words();
    let on = state.switches.on(switch);
    let allowed = state.switches.allowed(switch);
    let waiting = !switch.at_once() && on != state.switches_started.on(switch);
    let missing: Vec<&str> = switch.requires().iter().filter(|need| !state.switches.on(**need)).map(|need| need.words().0).collect();
    let rest = if stripe { ROW_ALT } else { ROW };
    let node = Node { width: Val::Percent(100.0), min_height: look.u(50.0), padding: UiRect::axes(look.u(18.0), look.u(6.0)), column_gap: look.u(20.0), align_items: AlignItems::Center, flex_shrink: 0.0, ..default() };
    let mut row = page.spawn((node, BackgroundColor(rest), Lit(rest), RelativeCursorPosition::default()));
    if allowed {
        row.insert((Click::Feature(switch), Button));
    }
    row.with_children(|row| {
        tick_box(row, look, on, allowed);
        // On, its name in the ink; off, dim; not allowed, faint.
        let name_ink = match (allowed, on) {
            (false, _) => INK_FAINT,
            (true, true) => INK,
            (true, false) => INK_DIM,
        };
        row.spawn(Node { width: look.u(330.0), flex_shrink: 0.0, ..default() }).with_children(|cell| {
            cell.spawn((look.text(name, 19.0, name_ink), TextLayout::new_with_no_wrap()));
        });
        row.spawn(Node { flex_grow: 1.0, flex_basis: Val::Px(0.0), min_width: Val::Px(0.0), ..default() }).with_children(|cell| {
            if explained {
                cell.spawn(look.text(what, 15.0, if allowed { INK_DIM } else { INK_FAINT }));
            }
        });
        row.spawn(Node { column_gap: look.u(8.0), justify_content: JustifyContent::FlexEnd, flex_shrink: 0.0, ..default() }).with_children(|end| {
            if !switch.built() {
                tag(end, look, "NOT BUILT", RUST);
            } else if !missing.is_empty() {
                tag(end, look, &format!("NEEDS {}", missing.join(", ")), RUST);
            }
            if waiting {
                tag(end, look, "RESTART", AMBER);
            }
        });
    });
}

/// A setting of the app's own drawn as a switch's row beside the
/// switches (SCREENS' HIDE UNUSED TABS): its box and its name, the whole
/// row a button; it needs nothing and is taken at once.
fn setting_switch_row(page: &mut ChildSpawnerCommands, look: &Look, name: &str, (which, on): (Toggle, bool), stripe: bool) {
    let rest = if stripe { ROW_ALT } else { ROW };
    let node = Node { width: Val::Percent(100.0), min_height: look.u(50.0), padding: UiRect::axes(look.u(18.0), look.u(6.0)), column_gap: look.u(20.0), align_items: AlignItems::Center, flex_shrink: 0.0, ..default() };
    page.spawn((node, BackgroundColor(rest), Lit(rest), RelativeCursorPosition::default(), Click::Toggle(which), Button)).with_children(|row| {
        tick_box(row, look, on, true);
        row.spawn((look.text(name, 19.0, if on { INK } else { INK_DIM }), TextLayout::new_with_no_wrap()));
    });
}
