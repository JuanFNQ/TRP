//! The graphics and sound pages, the voice list and the stepped settings.

use super::*;

/// The GRAPHICS page: four of the game's display settings, the pictures',
/// the shadows', the ambient occlusion's, the
/// clouds' and the frame rate's of the game's, and the app's own
/// multisampling, render thread and backend.
pub(super) fn graphics_page(page: &mut ChildSpawnerCommands, look: &Look, state: &OptionsState, sizes: &[[u32; 2]]) {
    let graphics = &state.graphics;
    let shut = |at: usize| state.shut(GRAPHICS_PAGE, at);
    let step = |c: &mut ChildSpawnerCommands, which: Step| stepper(c, look, which, step_choices(state, which, sizes));
    section_head(page, look, 0, shut(0), "DISPLAY", |_| {});
    if !shut(0) {
        // The frame rate's limit: its box ticked where it holds (the
        // game's `DisableGameFramerateLimit` the other way), its bar dim
        // where it does not.
        let fps_caption = state.said("Game FPS Limit").trim_end_matches(':').to_owned();
        let limited = !graphics.fps_limit_off;
        setting_row(page, look, false, "DISPLAY MODE", false, |c| step(c, Step::Mode));
        setting_row(page, look, true, "RESOLUTION", false, |c| step(c, Step::Resolution));
        setting_row(page, look, false, "VSYNC", false, |c| toggle(c, look, Toggle::Vsync, graphics.vsync));
        setting_row(page, look, true, &fps_caption, false, |c| {
            toggle(c, look, Toggle::FpsLimit, limited);
            slider_as(c, look, Slide::GameFps, graphics.game_fps as f32, 300.0, limited);
        });
        setting_row(page, look, false, "FIELD OF VIEW", false, |c| slider(c, look, Slide::Fov, graphics.field_of_view, 300.0));
    }
    section_head(page, look, 1, shut(1), "QUALITY", |_| {});
    if !shut(1) {
        let textures_waiting = (graphics.texture_quality, graphics.mip_streaming) != state.textures_started;
        let clouds_caption = state.said("Settings/Graphics/CloudsQuality").trim_end_matches(':').to_owned();
        setting_row(page, look, false, "TEXTURE QUALITY", textures_waiting, |c| step(c, Step::Texture));
        setting_row(page, look, true, "MIP STREAMING", textures_waiting, |c| toggle(c, look, Toggle::MipStreaming, graphics.mip_streaming));
        setting_row(page, look, false, "SHADOW QUALITY", graphics.shadow_quality != state.shadows_started, |c| step(c, Step::Shadow));
        setting_row(page, look, true, "SSAO", graphics.ssao != state.ssao_started, |c| step(c, Step::Ssao));
        // The levels of detail's bias, taken at once; dim where DISTANCE DETAIL is
        // off for this run, which draws the fullest at any distance.
        let lod_caption = state.said("Object LOD quality:").trim_end_matches(':').to_owned();
        let detailed = state.in_force.distance_detail || state.in_force.world_distance_detail;
        let lod = Row { name: &lod_caption, dim: !detailed, tag: (!detailed).then_some(("NEEDS DETAIL", RUST)), ..default() };
        row(page, look, false, lod, |c| stepper_as(c, look, Step::Lod, step_choices(state, Step::Lod, sizes), detailed));
        // The app's own: Tarkov's camera multisamples nothing.
        setting_row(page, look, true, "MSAA", false, |c| step(c, Step::Msaa));
        // The dust's resolution, the app's own: Tarkov draws it whole (FULL).
        setting_row(page, look, false, "DUST RESOLUTION", false, |c| step(c, Step::Dust));
        // The clouds last, dim where TARKOV LIGHTING is off for the next
        // start, which draws none.
        let lit = state.switches.on(Switch::TarkovLighting);
        let clouds = Row { name: &clouds_caption, dim: !lit, waiting: graphics.clouds_quality != state.clouds_started, tag: (!lit).then_some(("NEEDS LIGHTING", RUST)), ..default() };
        row(page, look, true, clouds, |c| stepper_as(c, look, Step::Clouds, step_choices(state, Step::Clouds, sizes), lit));
    }
    section_head(page, look, 2, shut(2), "RENDERING", |_| {});
    if !shut(2) {
        setting_row(page, look, false, "RENDER THREAD", graphics.render_thread != state.render_thread_started, |c| toggle(c, look, Toggle::RenderThread, graphics.render_thread));
        setting_row(page, look, true, "RENDERER", graphics.renderer != state.renderer_started, |c| step(c, Step::Renderer));
    }
}

/// The SOUNDS page: the game's own SOUND tab, of it what the app's sounds
/// go by, the overall volume and the interface's, in the game's eleven
/// steps (`SoundSettingsGroup`: 0 to 10, -80 to 0 decibels), as the install
/// has them until changed, each heard at once (`ui_sound`).
pub(super) fn sound_page(page: &mut ChildSpawnerCommands, look: &Look, state: &OptionsState) {
    let folded = state.shut(SOUNDS_PAGE, 0);
    section_head(page, look, 0, folded, "VOLUME", |_| {});
    if !folded {
        // The overall volume, everything the app plays; and the interface's.
        for (n, (kind, now)) in [(Slide::Volume(0), state.sound.overall), (Slide::Volume(1), state.sound.interface)].into_iter().enumerate() {
            let name = state.said(kind.caption().0).trim_end_matches(':').to_owned();
            setting_row(page, look, n % 2 == 1, &name, false, |c| slider(c, look, kind, now as f32, 300.0));
        }
    }
    voice_section(page, look, state);
}

/// The SOUNDS page's PLAYER VOICE: the PMC voices
/// the profile has, of both sides, each with its side in words at its left
/// (`side_mark`; the game's list, `VoiceSelector`, holds the profile's
/// side's alone, by name, and marks none), chosen from a list that drops under its box,
/// each heard as it is chosen as the game's list plays it; the speaker
/// beside the box plays a mutter of the one chosen again. The character
/// speaks with it at once: its pain, its breath and the gestures' phrases;
/// with HEALTH off for the run it says nothing (the row says what it
/// needs), and the speaker still plays.
fn voice_section(page: &mut ChildSpawnerCommands, look: &Look, state: &OptionsState) {
    let folded = state.shut(SOUNDS_PAGE, 1);
    section_head(page, look, 1, folded, "PLAYER VOICE", |_| {});
    if folded {
        return;
    }
    let shown = state.voices.iter().find(|(name, ..)| state.voice.as_deref() == Some(name.as_str())).map(|(_, words, usec)| (words.clone(), *usec));
    let spec = Row { name: "VOICE", tag: (!state.in_force.health).then_some(("NEEDS HEALTH", RUST)), ..default() };
    row(page, look, false, spec, |c| {
        voice_box(c, look, shown, !state.voices.is_empty(), state.voice_open);
        speaker_button(c, look, state.voice.is_some());
    });
    if state.voice_open {
        voice_list(page, look, state);
    }
}

/// The voice box: the voice chosen and an arrow, the whole of it the
/// button that opens and shuts the list (a dropdown's, as the game's
/// `DropDownBox`); dim where the game offers none.
fn voice_box(parent: &mut ChildSpawnerCommands, look: &Look, shown: Option<(String, bool)>, enabled: bool, open: bool) {
    let node = Node { width: look.u(VOICE_BOX), height: look.u(36.0), padding: UiRect::horizontal(look.u(14.0)), border: UiRect::all(look.u(1.0)), column_gap: look.u(10.0), align_items: AlignItems::Center, flex_shrink: 0.0, ..default() };
    let mut spawned = parent.spawn((node, BackgroundColor(GROUND), BorderColor::all(if open { INK_DIM } else { EDGE })));
    if enabled {
        spawned.insert((Click::VoiceList, Button, Tint(GROUND, GROUND_LIT)));
    }
    let ink = if enabled { INK } else { INK_FAINT };
    spawned.with_children(|inside| {
        if let Some((_, usec)) = &shown {
            side_mark(inside, look, *usec, false, 17.0);
        }
        inside.spawn(Node { flex_grow: 1.0, min_width: Val::Px(0.0), overflow: Overflow::clip(), ..default() }).with_children(|words| {
            words.spawn((look.strong(shown.map_or_else(|| "NONE OFFERED".to_owned(), |(words, _)| words), 17.0, ink), TextLayout::new_with_no_wrap()));
        });
        chevron(inside, look, 9.0, if open { 315.0 } else { 135.0 }, if enabled { INK_DIM } else { INK_FAINT });
    });
}

/// The voice box's width, and the speaker's side, in the page's units.
const VOICE_BOX: f32 = 330.0;

const SPEAKER: f32 = 36.0;

/// The speaker that plays a mutter of the voice chosen: a magnet, a cone
/// widening to the right and two waves, drawn in boxes; dim with no voice.
fn speaker_button(parent: &mut ChildSpawnerCommands, look: &Look, enabled: bool) {
    let node = Node { width: look.u(SPEAKER), height: look.u(SPEAKER), border: UiRect::all(look.u(1.0)), justify_content: JustifyContent::Center, align_items: AlignItems::Center, flex_shrink: 0.0, ..default() };
    let mut spawned = parent.spawn((node, BackgroundColor(GROUND), BorderColor::all(EDGE)));
    if enabled {
        spawned.insert((Click::VoicePreview, Button, Tint(GROUND, GROUND_LIT)));
    }
    let ink = if enabled { INK } else { INK_FAINT };
    spawned.with_children(|inside| {
        inside.spawn(Node { width: look.u(22.0), height: look.u(20.0), flex_shrink: 0.0, ..default() }).with_children(|icon| {
            let at = |left: f32, top: f32, width: f32, height: f32| Node { position_type: PositionType::Absolute, left: look.u(left), top: look.u(top), width: look.u(width), height: look.u(height), ..default() };
            // The magnet, then the cone in three steps, about a middle 10 down.
            icon.spawn((at(1.0, 7.0, 4.0, 6.0), BackgroundColor(ink)));
            for (n, high) in [8.0, 12.0, 16.0].into_iter().enumerate() {
                icon.spawn((at(5.0 + 2.0 * n as f32, 10.0 - high / 2.0, 2.0, high), BackgroundColor(ink)));
            }
            // Two waves: rings drawn on their right side only, about one
            // middle just past the cone.
            for side in [8.0, 14.0] {
                let ring = Node { border: UiRect::right(look.u(2.0)), border_radius: BorderRadius::all(look.u(side / 2.0)), ..at(13.0 - side / 2.0, 10.0 - side / 2.0, side, side) };
                icon.spawn((ring, BorderColor::all(ink)));
            }
        });
    });
}

/// The list of voices, open under the voice box: each by the game's words
/// for it, the one chosen lit as a choice is on the page.
fn voice_list(page: &mut ChildSpawnerCommands, look: &Look, state: &OptionsState) {
    // Its right edge under the box's: past the row's end (its padding, the
    // column of what waits, its gap) and the speaker with its own gap.
    let right = 18.0 + 112.0 + 20.0 + SPEAKER + 12.0;
    let line = Node { width: Val::Percent(100.0), justify_content: JustifyContent::FlexEnd, padding: UiRect::right(look.u(right)), flex_shrink: 0.0, ..default() };
    page.spawn(line).with_children(|line| {
        let list = Node { width: look.u(VOICE_BOX), flex_direction: FlexDirection::Column, border: UiRect::all(look.u(1.0)), flex_shrink: 0.0, ..default() };
        line.spawn((list, BackgroundColor(WELL), BorderColor::all(INK_DIM))).with_children(|list| {
            for (at, (name, words, usec)) in state.voices.iter().enumerate() {
                let chosen = state.voice.as_deref() == Some(name.as_str());
                let (rest, lit, ink) = if chosen { (CHOSEN, CHOSEN_LIT, INK_DARK) } else { (Color::NONE, GROUND_LIT, INK) };
                let node = Node { height: look.u(34.0), padding: UiRect::horizontal(look.u(14.0)), column_gap: look.u(10.0), align_items: AlignItems::Center, flex_shrink: 0.0, ..default() };
                list.spawn((Click::VoiceChoice(at), Button, node, BackgroundColor(rest), Tint(rest, lit))).with_children(|entry| {
                    side_mark(entry, look, *usec, chosen, 16.0);
                    entry.spawn((look.strong(words.as_str(), 16.0, ink), TextLayout::new_with_no_wrap()));
                });
            }
        });
    });
}

/// A voice's side at the left of its name, in words: the side in its colour,
/// darker on a line chosen, and the bar in the line's own ink.
fn side_mark(parent: &mut ChildSpawnerCommands, look: &Look, usec: bool, chosen: bool, size: f32) {
    let colour = match (usec, chosen) {
        (false, false) => BEAR_RED,
        (false, true) => BEAR_RED_DARK,
        (true, false) => USEC_BLUE,
        (true, true) => USEC_BLUE_DARK,
    };
    let bar = if chosen { INK_DARK } else { INK_DIM };
    parent.spawn((look.strong(if usec { "[USEC]" } else { "[BEAR]" }, size, colour), TextLayout::new_with_no_wrap())).with_children(|line| {
        line.spawn(look.span(" |", size, bar, true));
    });
}

/// The sides' colours (the app's): the BEAR's a red, the USEC's a blue, and
/// each darker for a line chosen, whose ground is light.
const BEAR_RED: Color = Color::srgb(0.90, 0.38, 0.32);

const BEAR_RED_DARK: Color = Color::srgb(0.60, 0.12, 0.08);

const USEC_BLUE: Color = Color::srgb(0.42, 0.65, 0.95);

const USEC_BLUE_DARK: Color = Color::srgb(0.08, 0.28, 0.62);

/// A stepped setting's choices in words, and the one chosen.
pub(super) fn step_choices(state: &OptionsState, step: Step, sizes: &[[u32; 2]]) -> (Vec<String>, usize) {
    use crate::panels::battle_view::Shows;
    let graphics = &state.graphics;
    let shows = |now: Shows| (Shows::ALL.iter().map(|shows| state.said(&format!("Ui/Settings/Visibility/{}", shows.name()))).collect(), Shows::ALL.iter().position(|shows| *shows == now).unwrap_or(0));
    let position = |found: Option<usize>| found.unwrap_or(0);
    match step {
        Step::Quick => shows(state.game.quick),
        Step::Stamina => shows(state.game.stamina),
        Step::Health => shows(state.game.health),
        Step::Colors => (["WhiteToRed", "GreenToRed"].iter().map(|name| state.said(&format!("Ui/Settings/ColorScheme/{name}"))).collect(), state.game.green_to_red as usize),
        Step::Mode => (Mode::ALL.iter().map(|mode| mode.word().to_owned()).collect(), position(Mode::ALL.iter().position(|mode| *mode == graphics.mode))),
        Step::Resolution => {
            let ascending: Vec<[u32; 2]> = sizes.iter().rev().copied().collect();
            (ascending.iter().map(|size| format!("{} x {}", size[0], size[1])).collect(), position(ascending.iter().position(|size| *size == graphics.resolution)))
        }
        Step::Texture => (["LOW", "MEDIUM", "HIGH"].map(str::to_owned).to_vec(), graphics.texture_quality.min(2) as usize),
        Step::Shadow => (crate::rendering::shadows::NAMES.iter().map(|name| name.to_string()).collect(), (graphics.shadow_quality as usize).min(crate::rendering::shadows::NAMES.len() - 1)),
        Step::Ssao => (trp_sim::lighting::hbao::SsaoMode::ALL.iter().map(|mode| state.said(&format!("ESSAOMode/{}", mode.key()))).collect(), position(trp_sim::lighting::hbao::SsaoMode::ALL.iter().position(|mode| *mode == graphics.ssao))),
        Step::Clouds => (
            trp_sim::lighting::clouds::CloudsQuality::ALL.iter().map(|quality| state.said(&format!("CloudsMode/{}", quality.key()))).collect(),
            position(trp_sim::lighting::clouds::CloudsQuality::ALL.iter().position(|quality| *quality == graphics.clouds_quality)),
        ),
        Step::Msaa => (MSAA.iter().map(|(_, word)| word.to_string()).collect(), position(MSAA.iter().position(|(samples, _)| *samples == graphics.msaa))),
        Step::Dust => {
            use crate::effects::offscreen_dust::DustResolution;
            (DustResolution::ALL.iter().map(|r| r.word().to_owned()).collect(), position(DustResolution::ALL.iter().position(|r| *r == graphics.dust_resolution)))
        }
        Step::Renderer => (RENDERERS.iter().map(|renderer| renderer.word().to_owned()).collect(), position(RENDERERS.iter().position(|renderer| *renderer == graphics.renderer))),
        Step::Lod => (
            trp_data::setup::install::LOD_BIAS_CHOICES.iter().map(|bias| lod_bias_word(*bias)).collect(),
            position(trp_data::setup::install::LOD_BIAS_CHOICES.iter().position(|bias| (bias - graphics.lod_bias).abs() < 0.01)),
        ),
    }
}

/// A bias as the game's menu shows it (`x.ToString("0.#")`).
pub(super) fn lod_bias_word(bias: f32) -> String {
    let tenths = (bias * 10.0).round() as i64;
    if tenths % 10 == 0 { format!("{}", tenths / 10) } else { format!("{}.{}", tenths / 10, tenths % 10) }
}

/// The two backends, in the RENDERER setting's order.
const RENDERERS: [crate::platform::backend::Renderer; 2] = [crate::platform::backend::Renderer::Vulkan, crate::platform::backend::Renderer::Dx12];

/// A stepped setting set to its choice at `at` (of `step_choices`).
pub(super) fn step_set(state: &mut OptionsState, step: Step, at: usize, sizes: &[[u32; 2]]) {
    use crate::panels::battle_view::Shows;
    let shows = Shows::ALL[at.min(Shows::ALL.len() - 1)];
    let graphics = &mut state.graphics;
    match step {
        Step::Quick => state.game.quick = shows,
        Step::Stamina => state.game.stamina = shows,
        Step::Health => state.game.health = shows,
        Step::Colors => state.game.green_to_red = at == 1,
        Step::Mode => graphics.mode = Mode::ALL[at.min(Mode::ALL.len() - 1)],
        Step::Resolution => {
            if let Some(size) = sizes.iter().rev().nth(at) {
                graphics.resolution = *size;
            }
        }
        Step::Texture => graphics.texture_quality = at.min(2) as u8,
        Step::Shadow => graphics.shadow_quality = at.min(crate::rendering::shadows::NAMES.len() - 1) as u8,
        Step::Ssao => graphics.ssao = trp_sim::lighting::hbao::SsaoMode::ALL[at.min(trp_sim::lighting::hbao::SsaoMode::ALL.len() - 1)],
        Step::Clouds => graphics.clouds_quality = trp_sim::lighting::clouds::CloudsQuality::ALL[at.min(trp_sim::lighting::clouds::CloudsQuality::ALL.len() - 1)],
        Step::Msaa => graphics.msaa = MSAA[at.min(MSAA.len() - 1)].0,
        Step::Dust => {
            use crate::effects::offscreen_dust::DustResolution;
            graphics.dust_resolution = DustResolution::ALL[at.min(DustResolution::ALL.len() - 1)];
        }
        Step::Renderer => graphics.renderer = RENDERERS[at.min(RENDERERS.len() - 1)],
        Step::Lod => graphics.lod_bias = trp_data::setup::install::LOD_BIAS_CHOICES[at.min(trp_data::setup::install::LOD_BIAS_CHOICES.len() - 1)],
    }
}
