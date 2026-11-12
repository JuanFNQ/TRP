//! Rebuilding the page and handling its clicks.

use super::*;

/// The sizes the window may be given: those the screens have a mode for,
/// the largest first, with the size the app starts at and the one in
/// force among them.
fn resolutions(monitors: &Query<&Monitor>, graphics: &Graphics) -> Vec<[u32; 2]> {
    let mut sizes: Vec<[u32; 2]> = monitors.iter().flat_map(|monitor| monitor.video_modes.iter().map(|mode| [mode.physical_size.x, mode.physical_size.y])).collect();
    sizes.retain(|size| size[0] >= 1024 && size[1] >= 720);
    sizes.extend([crate::WINDOW_SIZE, graphics.resolution]);
    sizes.sort_by(|a, b| (b[0], b[1]).cmp(&(a[0], a[1])));
    sizes.dedup();
    sizes.truncate(14);
    if !sizes.contains(&graphics.resolution) {
        sizes.push(graphics.resolution);
    }
    sizes
}

/// Draws the tab afresh when something on it has changed: the column at
/// the left, and the page chosen with its head, its body in a box that
/// scrolls with its bar at the right, and its footer. Nothing of it has a
/// ground of its own over the world but its rows and buttons.
#[allow(clippy::too_many_arguments)]
pub(super) fn rebuild(
    mut commands: Commands,
    mut state: ResMut<OptionsState>,
    controller: Res<Controller>,
    selected: Res<Selected>,
    bindings: Res<Bindings>,
    window: Query<&Window, With<PrimaryWindow>>,
    monitors: Query<&Monitor>,
    page: Query<Entity, With<OptionsPage>>,
    fonts: Option<Res<crate::screens::health_view::HealthLook>>,
    mut was: Local<(bool, Vec2, Option<AssetId<Font>>)>,
    mut said_fonts: Local<bool>,
) {
    let _clock = crate::platform::render_timing::Clock::new("options rebuild");
    let size = window.single().map_or(Vec2::ZERO, |window| Vec2::new(window.width(), window.height()));
    let shows = controller.inventory_open && selected.0 == OPTIONS;
    let look = Look::new(size.y, fonts.as_deref());
    if let Some(fonts) = fonts.as_deref().filter(|fonts| shows && !*said_fonts && !fonts.font_asset_names().is_empty()) {
        *said_fonts = true;
        if look.font == Handle::default() {
            println!("options: the game's Bender was not among the fonts read ({}); the app's font is used", fonts.font_asset_names().join(", "));
        } else {
            let name = |font: &Handle<Font>| fonts.font_name(font.id()).unwrap_or("the app's").to_owned();
            println!("options: drawn in {}, bold in {}", name(&look.font), name(&look.bold));
        }
    }
    let now = (shows, size, Some(look.font.id()));
    let changed = state.dirty || *was != now;
    *was = now;
    if !shows || !changed {
        return;
    }
    state.dirty = false;
    let state = &*state;
    let sizes = resolutions(&monitors, &state.graphics);
    for page in &page {
        commands.entity(page).despawn_related::<Children>().with_children(|page| {
            page.spawn(Node { width: Val::Percent(100.0), height: Val::Percent(100.0), padding: UiRect::new(look.u(14.0), look.u(8.0), Val::Px(0.0), look.u(6.0)), ..default() }).with_children(|whole| {
                left_column(whole, &look, state);
                let right = Node { flex_grow: 1.0, flex_basis: Val::Px(0.0), min_width: Val::Px(0.0), min_height: Val::Px(0.0), flex_direction: FlexDirection::Column, margin: UiRect::left(look.u(30.0)), ..default() };
                whole.spawn(right).with_children(|right| {
                    page_head(right, &look, state);
                    let frame = Node { flex_grow: 1.0, flex_basis: Val::Px(0.0), min_height: Val::Px(0.0), width: Val::Percent(100.0), column_gap: look.u(8.0), ..default() };
                    right.spawn(frame).with_children(|frame| {
                        let area = Node { flex_direction: FlexDirection::Column, flex_grow: 1.0, min_height: Val::Px(0.0), min_width: Val::Px(0.0), overflow: Overflow::scroll_y(), ..default() };
                        frame.spawn((PageArea, area, ScrollPosition(Vec2::new(0.0, state.page_scroll.max(0.0))), RelativeCursorPosition::default())).with_children(|area| {
                            area.spawn(Node { flex_direction: FlexDirection::Column, row_gap: look.u(2.0), flex_shrink: 0.0, width: Val::Percent(100.0), padding: UiRect::bottom(look.u(12.0)), ..default() }).with_children(|body| match state.page {
                                GRAPHICS_PAGE => graphics_page(body, &look, state, &sizes),
                                SOUNDS_PAGE => sound_page(body, &look, state),
                                HUD_PAGE => hud_page(body, &look, state, &sizes),
                                CONTROLS_PAGE => controls_page(body, &look, state, &bindings),
                                FEATURES_PAGE => features_page(body, &look, state),
                                _ => skills_page(body, &look, state),
                            });
                        });
                        // The bar: a thin track, its handle as long as the
                        // share of the page shown (`scroll_page`).
                        let bar = Node { width: look.u(6.0), flex_shrink: 0.0, ..default() };
                        frame.spawn((PageBar, RelativeCursorPosition::default(), bar, BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.46)))).with_children(|bar| {
                            let handle = Node { position_type: PositionType::Absolute, left: Val::Px(0.0), width: Val::Percent(100.0), top: Val::Px(0.0), height: Val::Percent(100.0), ..default() };
                            bar.spawn((PageHandle, handle, BackgroundColor(INK_DIM), Visibility::Hidden));
                        });
                    });
                    footer(right, &look, state, &bindings);
                });
            });
        });
    }
}

/// What a click on the tab does. A right click on a key's cell unbinds it.
///
/// **Its sounds** (`docs/spec/settings.md`, "The tab's sounds"), each with
/// what it does, on the press, where the tab acts (the game's on the
/// release, where its controls act): a button of the game's kind
/// `ButtonClick` (`Click::feedback`); a box ticked or not `MenuCheckBox`
/// (`UpdatableToggle`); a choice stepped to by an arrow, or a way of
/// pressing chosen, `MenuDropdownSelect`, and a way of pressing asked for
/// or a section opened `MenuDropdown`, the game's dropdown's
/// (`BaseDropDownBox`: shut, it is silent); a key's cell, the search field
/// and a bar nothing, as the game's plain buttons, field and sliders.
#[allow(clippy::too_many_arguments)]
pub(super) fn clicks(
    mut state: ResMut<OptionsState>,
    mut bindings: ResMut<Bindings>,
    controller: Res<Controller>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    pressed: Query<(&Interaction, &Click, Has<Feedback>), Changed<Interaction>>,
    under: Query<(&Interaction, &Click)>,
    monitors: Query<&Monitor>,
    mut quit: MessageWriter<AppExit>,
    mut battle_modes: ResMut<crate::panels::battle_view::BattleModes>,
    mut sounds: MessageWriter<Sound>,
    (mut voice_chosen, mut voice_heard): (MessageWriter<crate::audio::voice_sound::VoiceChosen>, MessageWriter<crate::audio::voice_sound::PreviewAsked>),
) {
    let is_pressed = |interaction: &Interaction| *interaction == Interaction::Pressed;
    // The screen shut: a key being chosen is dropped with it, and the list
    // of voices shut.
    if !controller.inventory_open {
        if state.listening.is_some() || state.asking.is_some() {
            (state.listening, state.asking, state.dirty) = (None, None, true);
            bindings.muted = false;
        }
        if state.voice_open {
            (state.voice_open, state.dirty) = (false, true);
        }
        return;
    }
    if state.listening.is_some() {
        return;
    }
    // The list of voices shut by Escape, nothing chosen (the game's
    // dropdown's).
    if state.voice_open && keys.just_pressed(KeyCode::Escape) {
        (state.voice_open, state.dirty) = (false, true);
        return;
    }
    let state = &mut *state;
    // How a binding is pressed, asked for: one of the four, or Escape,
    // which leaves it as it is (the game's list shut with nothing chosen).
    if let Some(group) = state.asking {
        let chosen = pressed.iter().find_map(|(interaction, click, _)| match click {
            Click::PressChoice(press) if is_pressed(interaction) => Some(*press),
            _ => None,
        });
        if chosen.is_some() {
            sounds.write(Sound::Ui(Ui::MenuDropdownSelect));
        }
        let kept = keys.just_pressed(KeyCode::Escape).then(|| bindings.controls.keys[group].press);
        if let Some(press) = chosen.or(kept) {
            let before = bindings.controls.clone();
            bindings.controls.press_chosen(group, press);
            state.note = format!("{}: {}{}", state.name_of(&before.keys[group].name), state.press_word(bindings.controls.keys[group].press).to_lowercase(), lost(state, &before, &bindings.controls, group));
            bindings.changed();
            state.keep(&bindings.controls);
            (state.asking, state.dirty) = (None, true);
        }
        return;
    }
    // A right click on a key's cell: unbound.
    if mouse.just_pressed(MouseButton::Right) {
        let cell = under.iter().find_map(|(interaction, click)| match click {
            Click::Key(cell) if *interaction != Interaction::None => Some(*cell),
            _ => None,
        });
        if let Some(cell) = cell {
            state.selected_cell = Some(cell);
            clear_cell(state, &mut bindings, cell);
            return;
        }
    }
    let Some((click, feedback)) = pressed.iter().find(|(interaction, ..)| is_pressed(interaction)).map(|(_, click, feedback)| (*click, feedback)) else { return };
    if feedback {
        sounds.write(Sound::Ui(Ui::ButtonClick));
    }
    let sizes = resolutions(&monitors, &state.graphics);
    let game_before = state.game;
    let voice_before = state.voice.clone();
    match click {
        Click::Page(page) => {
            if page != state.page {
                (state.page, state.page_scroll, state.page_held, state.selected_cell, state.search_focused, state.name_focused, state.voice_open) = (page, 0.0, None, None, false, false, false);
                state.note.clear();
            }
        }
        Click::Quit => {
            // The run is ended: what was changed is already kept.
            quit.write(AppExit::Success);
            return;
        }
        Click::Reset => reset_page(state, &mut bindings),
        Click::Apply => {
            if state.file.is_some() && !state.waiting().is_empty() {
                match restart() {
                    Ok(()) => {
                        quit.write(AppExit::Success);
                        return;
                    }
                    Err(e) => state.note = format!("TRP could not be started again: {e}"),
                }
            }
        }
        Click::Step(step, by) => {
            let (choices, at) = step_choices(state, step, &sizes);
            let to = (at as i64 + by as i64).clamp(0, choices.len() as i64 - 1) as usize;
            if to != at {
                step_set(state, step, to, &sizes);
                state.note.clear();
                state.keep(&bindings.controls);
                sounds.write(Sound::Ui(Ui::MenuDropdownSelect));
            }
        }
        Click::Toggle(which) => {
            let graphics = &mut state.graphics;
            match which {
                Toggle::Vsync => graphics.vsync = !graphics.vsync,
                Toggle::FpsLimit => graphics.fps_limit_off = !graphics.fps_limit_off,
                Toggle::MipStreaming => graphics.mip_streaming = !graphics.mip_streaming,
                Toggle::RenderThread => graphics.render_thread = !graphics.render_thread,
                Toggle::Invert(axis) => state.mouse.invert[axis] = !state.mouse.invert[axis],
                Toggle::Notice(which) => {
                    let on = !state.notices.on(which);
                    state.notices.set(which, on);
                }
                Toggle::HideTabs => state.hide_tabs = !state.hide_tabs,
            }
            state.note.clear();
            state.keep(&bindings.controls);
            sounds.write(Sound::Ui(Ui::MenuCheckBox));
        }
        Click::Feature(switch) => {
            // Thrown and kept for the next run. What goes by it holds the
            // switches as this run began (restart required).
            let on = !state.switches.on(switch);
            if state.switches.set(switch, on) {
                state.keep(&bindings.controls);
                sounds.write(Sound::Ui(Ui::MenuCheckBox));
            }
        }
        Click::Fold(at) => {
            let key = (state.page, at);
            // Folded shut, or opened again with the dropdown's sound.
            if !state.folded.insert(key) {
                state.folded.remove(&key);
                sounds.write(Sound::Ui(Ui::MenuDropdown));
            }
        }
        Click::Category(category) => (state.category, state.search, state.page_scroll, state.selected_cell) = (category, String::new(), 0.0, None),
        Click::Search => state.search_focused = true,
        Click::Name => state.name_focused = true,
        Click::ClearKey => {
            if let Some(cell) = state.selected_cell {
                clear_cell(state, &mut bindings, cell);
            }
        }
        Click::Key(cell) => {
            state.listening = Some(Listen { cell, heard: Listening::default(), armed: false, done: false });
            (state.selected_cell, state.search_focused) = (Some(cell), false);
            bindings.muted = true;
        }
        Click::Press(index) => {
            state.asking = Some(index);
            sounds.write(Sound::Ui(Ui::MenuDropdown));
        }
        Click::PressChoice(_) => {}
        Click::HourDefault => state.hour = crate::rendering::lighting::RANGE_MOMENT.hour,
        Click::SkillsUntrained => {
            if !state.skills.untrained() {
                state.skills = trp_sim::condition::skills::SkillLevels::default();
                state.keep(&bindings.controls);
            }
        }
        Click::MasteryUntrained => {
            if !state.mastering.untrained() {
                state.mastering.all_to_nought();
                state.keep(&bindings.controls);
            }
        }
        // The list opened (`MenuDropdown`, as the game's dropdown), or shut
        // with nothing chosen (silent).
        Click::VoiceList => {
            state.voice_open = !state.voice_open;
            if state.voice_open {
                sounds.write(Sound::Ui(Ui::MenuDropdown));
            }
        }
        // A voice chosen (`MenuDropdownSelect`), kept, and heard where it is
        // another (`VoiceSelector.DropdownValueChanged`).
        Click::VoiceChoice(at) => {
            state.voice_open = false;
            sounds.write(Sound::Ui(Ui::MenuDropdownSelect));
            if let Some((name, ..)) = state.voices.get(at).cloned().filter(|(name, ..)| state.voice.as_deref() != Some(name.as_str())) {
                if let Some(file) = &state.file {
                    crate::audio::voice_sound::keep(file, Some(&name));
                }
                state.voice = Some(name.clone());
                voice_heard.write(crate::audio::voice_sound::PreviewAsked(name));
            }
        }
        Click::VoicePreview => {
            if let Some(name) = state.voice.clone() {
                voice_heard.write(crate::audio::voice_sound::PreviewAsked(name));
            }
        }
    }
    // A setting of the HUD page's IN-RAID INTERFACE (the game's own GAME
    // tab's): kept, and handed to the battle screen,
    // which takes it as it is next shown.
    if state.game != game_before {
        *battle_modes = state.game;
    }
    // Another voice (chosen, or the profile's again): spoken with at once.
    if state.voice != voice_before {
        if let Some(name) = state.voice.clone() {
            voice_chosen.write(crate::audio::voice_sound::VoiceChosen(name));
        }
    }
    state.dirty = true;
}
