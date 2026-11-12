//! Resetting a page, typing names and searches, and listening for a key.

use super::*;

/// RESET TO DEFAULT: the page put back as it comes, the install's where
/// the game's own settings stand (the old `AS THE INSTALL HAS THEM`), the
/// recommended switches, the skills untrained.
pub(super) fn reset_page(state: &mut OptionsState, bindings: &mut Bindings) {
    match state.page {
        HUD_PAGE => {
            for switch in HUD_SWITCHES {
                state.switches.set(switch, switch.recommended());
            }
            state.game = state.install_game;
            state.notices = Notices::default();
            state.hide_tabs = true;
            state.note = "the screens' switches as recommended, the unused tabs hidden, the interface the install's again, every notice on".to_owned();
        }
        FEATURES_PAGE => {
            // The HUD page's own kept as it stands (where what it needs
            // still is on).
            let kept = HUD_SWITCHES.map(|switch| (switch, state.switches.on(switch)));
            state.switches = Switches::default();
            for (switch, on) in kept {
                state.switches.set(switch, on);
            }
            state.hour = crate::rendering::lighting::RANGE_MOMENT.hour;
            state.note = "every switch of the page as recommended, the range at 09:30 again".to_owned();
        }
        GRAPHICS_PAGE => {
            // (The backend, the multisampling and the dust's resolution
            // stay: the game has none of them.)
            state.graphics = Graphics { renderer: state.graphics.renderer, msaa: state.graphics.msaa, dust_resolution: state.graphics.dust_resolution, ..state.install_graphics.clone() };
            state.note = "the display is the install's again".to_owned();
        }
        SOUNDS_PAGE => {
            state.sound = state.install_sound;
            // The profile's own voice again, none kept.
            if let Some(file) = &state.file {
                crate::audio::voice_sound::keep(file, None);
            }
            state.voice = state.profile_voice.clone();
            state.voice_open = false;
            state.note = "the volumes and the voice are the install's again".to_owned();
        }
        CONTROLS_PAGE => {
            bindings.controls = bindings.install.clone();
            bindings.changed();
            state.mouse = state.install_mouse.clone();
            state.selected_cell = None;
            state.note = "the keys and the mouse are the install's again".to_owned();
        }
        _ => {
            state.skills = trp_sim::condition::skills::SkillLevels::default();
            state.mastering.all_to_nought();
            state.note = "every skill at 0, every family at Level 1".to_owned();
        }
    }
    state.keep(&bindings.controls);
}

/// A cell's keys taken off (CLEAR KEY, or a right click): the binding or
/// the axis's way is left with none, as the install's own may be.
pub(super) fn clear_cell(state: &mut OptionsState, bindings: &mut Bindings, cell: Cell) {
    if cell_source(&bindings.controls, cell).is_empty() {
        return;
    }
    match cell {
        Cell::Key(group, variant) => {
            bindings.controls.key_chosen(group, variant, Source::default());
        }
        Cell::Axis(axis, pair, way) => bindings.controls.axis_chosen(axis, pair, way, Source::default()),
    }
    state.note = format!("{}: unbound", cell_name(state, &bindings.controls, cell));
    bindings.changed();
    state.keep(&bindings.controls);
    state.dirty = true;
}

/// Starts TRP again as this run was started (the same program, its words
/// and its folder): APPLY, so that what waits for a start is taken now.
pub(super) fn restart() -> std::io::Result<()> {
    let program = std::env::current_exe()?;
    std::process::Command::new(program).args(std::env::args_os().skip(1)).current_dir(std::env::current_dir()?).spawn()?;
    Ok(())
}

/// The CHARACTER NAME field, as the search field: a click on it gives it
/// the keys (`clicks`), a click anywhere else takes them away, and while it
/// has them the game's bindings hear nothing; a letter is added (at most
/// `NAME_MOST`), Backspace takes the last off, and Enter or Escape let the
/// field go. The name is taken at once (the OVERALL tab shows it as it
/// is typed) and kept in the options' file with each change.
pub(super) fn type_name(
    mut state: ResMut<OptionsState>,
    mut bindings: ResMut<Bindings>,
    controller: Res<Controller>,
    selected: Res<Selected>,
    buttons: Res<ButtonInput<MouseButton>>,
    mut typed: MessageReader<KeyboardInput>,
    field: Query<&Interaction, With<NameField>>,
    mut muting: Local<bool>,
) {
    let shows = controller.inventory_open && selected.0 == OPTIONS && state.page == FEATURES_PAGE;
    if !shows && state.name_focused {
        state.name_focused = false;
    }
    if state.name_focused && buttons.get_just_pressed().next().is_some() && field.iter().all(|interaction| *interaction == Interaction::None) {
        (state.name_focused, state.dirty) = (false, true);
    }
    if state.name_focused {
        let before = state.character_name.clone();
        for key in typed.read().filter(|key| key.state.is_pressed()) {
            match &key.logical_key {
                Key::Backspace => {
                    state.character_name.pop();
                }
                Key::Enter | Key::Escape => (state.name_focused, state.dirty) = (false, true),
                _ => {
                    for c in key.text.as_deref().unwrap_or_default().chars().filter(|c| !c.is_control()) {
                        if state.character_name.chars().count() < NAME_MOST {
                            state.character_name.push(c);
                        }
                    }
                }
            }
        }
        if state.character_name != before {
            state.dirty = true;
            if let Some(path) = state.file.clone() {
                keep_name(&path, &state.character_name);
            }
        }
    } else {
        typed.clear();
    }
    if state.name_focused != *muting {
        *muting = state.name_focused;
        if *muting {
            bindings.muted = true;
        } else if state.listening.is_none() {
            bindings.muted = false;
        }
    }
}

/// The search field: a click on it gives it the keys (`clicks`), a click
/// anywhere else takes them away, and while it has them the game's
/// bindings hear nothing (as a tag's field: `inventory_view::tag_windows`),
/// a letter is added, Backspace takes the last off, and Enter or Escape
/// let the field go.
pub(super) fn type_search(
    mut state: ResMut<OptionsState>,
    mut bindings: ResMut<Bindings>,
    controller: Res<Controller>,
    selected: Res<Selected>,
    buttons: Res<ButtonInput<MouseButton>>,
    mut typed: MessageReader<KeyboardInput>,
    field: Query<&Interaction, With<SearchField>>,
    mut muting: Local<bool>,
) {
    let shows = controller.inventory_open && selected.0 == OPTIONS && state.page == CONTROLS_PAGE;
    if !shows && state.search_focused {
        state.search_focused = false;
    }
    if state.search_focused && buttons.get_just_pressed().next().is_some() && field.iter().all(|interaction| *interaction == Interaction::None) {
        (state.search_focused, state.dirty) = (false, true);
    }
    if state.search_focused {
        let before = state.search.clone();
        for key in typed.read().filter(|key| key.state.is_pressed()) {
            match &key.logical_key {
                Key::Backspace => {
                    state.search.pop();
                }
                Key::Enter | Key::Escape => (state.search_focused, state.dirty) = (false, true),
                _ => {
                    for c in key.text.as_deref().unwrap_or_default().chars().filter(|c| !c.is_control()) {
                        if state.search.chars().count() < 24 {
                            state.search.push(c);
                        }
                    }
                }
            }
        }
        if state.search != before {
            (state.page_scroll, state.dirty) = (0.0, true);
        }
    } else {
        typed.clear();
    }
    // The bindings muted while the field has the keys, and let go when it
    // gives them up (unless a key is being chosen, which mutes them too).
    if state.search_focused != *muting {
        *muting = state.search_focused;
        if *muting {
            bindings.muted = true;
        } else if state.listening.is_none() {
            bindings.muted = false;
        }
    }
}

/// Which other bindings lost keys to a change, in words.
pub(super) fn lost(state: &OptionsState, before: &Controls, after: &Controls, but: usize) -> String {
    let keys = before.keys.iter().zip(&after.keys).enumerate().filter(|(at, (was, is))| *at != but && was.variants != is.variants).map(|(_, (was, _))| state.name_of(&was.name));
    let axes = before.axes.iter().zip(&after.axes).filter(|(was, is)| was.pairs != is.pairs).map(|(was, _)| was.name.to_uppercase());
    let names: Vec<String> = keys.chain(axes).collect();
    if names.is_empty() { String::new() } else { format!("   (taken from {})", names.join(", ")) }
}

/// A key being chosen (`CommandKeyPair.ListenForKey`): once the click
/// that began it is over, the keys held are gathered, and the first of
/// them let go, a third going down or a notch of the wheel is the choice;
/// Escape among them drops it. Nothing else hears a key meanwhile, nor
/// until every key is up again.
pub(super) fn listen(
    mut state: ResMut<OptionsState>,
    mut bindings: ResMut<Bindings>,
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    scroll: Res<AccumulatedMouseScroll>,
) {
    let state = &mut *state;
    let Some(listening) = &mut state.listening else { return };
    let any_down = keys.get_pressed().next().is_some() || buttons.get_pressed().next().is_some();
    if !listening.armed {
        listening.armed = !any_down;
        return;
    }
    if listening.done {
        if !any_down {
            (state.listening, state.dirty) = (None, true);
            bindings.muted = false;
        }
        return;
    }
    let cell = listening.cell;
    // The wheel may end any binding's keys but the trigger's
    // (`ForbiddenForAxisWithDefaultKey`); an axis's keys are keys.
    let takes_wheel = matches!(cell, Cell::Key(group, _) if bindings.controls.keys[group].name != "Shoot");
    let wheel = if scroll.delta.y == 0.0 { 0 } else { scroll.delta.y.signum() as i32 };
    let had = listening.heard.source.keys.len();
    let chosen = listening.heard.frame(&bindings.valid, &|code| bindings.held(code, &keys, &buttons), wheel, takes_wheel);
    match chosen {
        Chosen::Going => state.dirty |= listening.heard.source.keys.len() != had,
        Chosen::Dropped => {
            listening.done = true;
            state.note = "kept as it was".to_owned();
            state.dirty = true;
        }
        Chosen::Done(source) => {
            listening.done = true;
            let before = bindings.controls.clone();
            let said = keys::words(&source);
            match cell {
                Cell::Key(group, variant) => {
                    let asks = bindings.controls.key_chosen(group, variant, source);
                    let name = state.name_of(&before.keys[group].name);
                    if asks {
                        state.asking = Some(group);
                        state.note = format!("{name}: {said}. Another binding has these keys pressed the same way, or this one had none: choose how it is pressed.");
                    } else {
                        state.note = format!("{name}: {said}{}", lost(state, &before, &bindings.controls, group));
                    }
                }
                Cell::Axis(axis, pair, way) => {
                    bindings.controls.axis_chosen(axis, pair, way, source);
                    let word = axis_word(&before.axes[axis].name, way);
                    state.note = format!("{word}: {said}{}", lost(state, &before, &bindings.controls, usize::MAX));
                }
            }
            bindings.changed();
            state.keep(&bindings.controls);
            state.dirty = true;
        }
    }
}
