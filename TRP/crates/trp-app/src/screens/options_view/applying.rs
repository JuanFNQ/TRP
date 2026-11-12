//! Slides, scrolling, hovering, syncing and applying the options.

use super::*;

/// A bar held and slid: its figure follows the mouse along it, and what
/// is drawn of it is changed where it stands. It is kept once the bar is
/// let go; the double click's time is then put to the bindings, whose
/// combinations are made afresh as the game makes them when it is saved.
pub(super) fn slide(
    mut state: ResMut<OptionsState>,
    mut bindings: ResMut<Bindings>,
    buttons: Res<ButtonInput<MouseButton>>,
    bars: Query<(&SlideBar, &RelativeCursorPosition)>,
    mut fills: Query<(&SlideFill, &mut Node, &mut BackgroundColor)>,
    mut figures: Query<(&SlideValue, &mut Text, &mut TextColor)>,
) {
    if state.sliding.is_none() && !buttons.just_pressed(MouseButton::Left) {
        return;
    }
    let state = &mut *state;
    if buttons.just_pressed(MouseButton::Left) && state.listening.is_none() {
        state.sliding = bars.iter().find(|(_, over)| over.cursor_over()).map(|(bar, _)| bar.0);
    }
    let Some(kind) = state.sliding else { return };
    if !buttons.pressed(MouseButton::Left) {
        state.sliding = None;
        if kind == Slide::DoubleClick {
            bindings.changed();
        }
        state.keep(&bindings.controls);
        // A skill's or a family's row drawn again as let go: its name, its
        // elite's words and its section's count lit or not.
        if matches!(kind, Slide::Skill(_) | Slide::Mastering(_) | Slide::Level(_)) {
            state.dirty = true;
        }
        return;
    }
    let Some(at) = bars.iter().find(|(bar, _)| bar.0 == kind).and_then(|(_, over)| crate::screens::inventory_view::share_of(over)) else { return };
    let ([low, high], step) = (kind.range(), kind.step());
    let value = (((low + at.x.clamp(0.0, 1.0) * (high - low)) / step).round() * step).clamp(low, high);
    if value == state.slider(kind, &bindings.controls) {
        return;
    }
    // The bindings are not marked as changed for a bar that is not theirs.
    if kind == Slide::DoubleClick {
        bindings.controls.double_click = value;
    } else {
        state.set_slider(kind, value, &mut bindings.bypass_change_detection().controls);
    }
    let (fill_ink, figure_ink) = slide_inks(kind, value);
    for (_, mut node, mut ground) in fills.iter_mut().filter(|(fill, ..)| fill.0 == kind) {
        node.width = Val::Percent((value - low) / (high - low) * 100.0);
        ground.0 = fill_ink;
    }
    for (_, mut text, mut color) in figures.iter_mut().filter(|(figure, ..)| figure.0 == kind) {
        text.0 = kind.shown(value);
        color.0 = figure_ink;
    }
}

/// The page scrolls whole in its box: moved by the wheel over it or its
/// bar (unless a key or a bar is being taken), its handle dragged, or a
/// click on its bar (the handle's middle put there); the handle as long as
/// the share of the page shown, hidden where all of it shows.
#[allow(clippy::too_many_arguments)]
pub(super) fn scroll_page(
    mut state: ResMut<OptionsState>,
    options: Res<Options>,
    buttons: Res<ButtonInput<MouseButton>>,
    wheel: Res<AccumulatedMouseScroll>,
    mut area: Query<(&mut ScrollPosition, &RelativeCursorPosition, &ComputedNode), With<PageArea>>,
    bar: Query<(&RelativeCursorPosition, &ComputedNode), With<PageBar>>,
    mut handle: Query<(&mut Node, &mut Visibility), With<PageHandle>>,
    mut frame: Local<u32>,
) {
    // A run nobody is at the keys of turns the wheel by its own count of
    // drawn frames (`--wheel`), as the inventory's does.
    *frame += 1;
    let scripted: i32 = options.wheel.iter().filter(|(at, _)| options.unattended() && *frame == *at).map(|(_, notches)| *notches).sum();
    let Ok((mut position, over, laid)) = area.single_mut() else {
        if state.page_held.is_some() {
            state.bypass_change_detection().page_held = None;
        }
        return;
    };
    let k = laid.inverse_scale_factor();
    let (shown, whole) = (laid.size().y * k, laid.content_size().y * k);
    if shown <= 0.0 {
        return;
    }
    let most = (whole - shown).max(0.0);
    let Ok((bar_at, bar_laid)) = bar.single() else { return };
    let track = bar_laid.size().y * bar_laid.inverse_scale_factor();
    let long = if whole > 0.0 { (track * shown / whole).clamp(HANDLE_LEAST.min(track), track) } else { track };
    let room = (track - long).max(0.0);
    let mut to = state.page_scroll - scripted as f32 * NOTCH;
    if state.listening.is_none() && state.sliding.is_none() && (over.cursor_over() || bar_at.cursor_over()) && wheel.delta.y != 0.0 {
        to -= wheel.delta.y.signum() * NOTCH;
    }
    let mut held = state.page_held;
    if buttons.just_pressed(MouseButton::Left) && bar_at.cursor_over() && most > 0.0 {
        let top = if most > 0.0 { to.clamp(0.0, most) / most * room } else { 0.0 };
        held = crate::screens::inventory_view::share_of(bar_at).map(|at| {
            let y = at.y * track;
            if (top..=top + long).contains(&y) { y - top } else { long / 2.0 }
        });
    }
    if !buttons.pressed(MouseButton::Left) {
        held = None;
    }
    if let (Some(grab), Some(at)) = (held, crate::screens::inventory_view::share_of(bar_at)) {
        let top = (at.y * track - grab).clamp(0.0, room);
        to = if room > 0.0 { top / room * most } else { 0.0 };
    }
    let to = to.clamp(0.0, most);
    if (to, held) != (state.page_scroll, state.page_held) {
        let state = state.bypass_change_detection();
        (state.page_scroll, state.page_held) = (to, held);
    }
    if position.y != to {
        position.y = to;
    }
    let top = if most > 0.0 { to / most * room } else { 0.0 };
    for (mut node, mut visible) in &mut handle {
        let (want_top, want_long) = (Val::Px(top), Val::Px(long));
        if node.top != want_top || node.height != want_long {
            (node.top, node.height) = (want_top, want_long);
        }
        let want = if most > 0.0 { Visibility::Inherited } else { Visibility::Hidden };
        if *visible != want {
            *visible = want;
        }
    }
}

/// The rows and buttons under the mouse lit, and the page's last note
/// written at the foot as it changes.
pub(super) fn hover(
    state: Res<OptionsState>,
    mut rows: Query<(&RelativeCursorPosition, &Lit, &mut BackgroundColor), Without<Tint>>,
    mut buttons: Query<(&Interaction, &Tint, &mut BackgroundColor), Without<Lit>>,
    mut footer: Query<&mut Text, With<Footer>>,
) {
    for (at, lit, mut ground) in &mut rows {
        let want = if at.cursor_over() { ROW_LIT } else { lit.0 };
        if ground.0 != want {
            ground.0 = want;
        }
    }
    for (interaction, tint, mut ground) in &mut buttons {
        let want = if *interaction == Interaction::None { tint.0 } else { tint.1 };
        if ground.0 != want {
            ground.0 = want;
        }
    }
    let Ok(mut text) = footer.single_mut() else { return };
    if text.0 != state.note {
        text.0 = state.note.clone();
    }
}

/// The mouse come onto a button of the game's kind (`Feedback`): its
/// `ButtonOver` (`ButtonFeedback.PointerEnterHandler`). Kept by what the
/// button does, not by the button: the page is drawn afresh after a
/// click, and the same button drawn again under the mouse is not come
/// onto again. On the frame it is drawn afresh, what is under the mouse
/// is known on the next.
pub(super) fn button_over(controller: Res<Controller>, buttons: Query<(&Interaction, &Click), With<Feedback>>, drawn: Query<(), Added<Click>>, mut was: Local<Option<Click>>, mut sounds: MessageWriter<Sound>) {
    if !controller.inventory_open {
        *was = None;
        return;
    }
    if !drawn.is_empty() {
        return;
    }
    let now = buttons.iter().find(|(interaction, _)| **interaction != Interaction::None).map(|(_, click)| *click);
    if now.is_some() && now != *was {
        sounds.write(Sound::Ui(Ui::ButtonOver));
    }
    *was = now;
}

/// The switches taken at once (`Switch::at_once`) put on the run as they
/// are thrown, by a click, a need turned off or `RESET TO DEFAULT`: the
/// highlight on where its switch is and the run has the inventory. As the
/// run begins it is as the switches and the command line had it.
pub(super) fn live_switches(mut state: ResMut<OptionsState>, mut features: ResMut<Features>, mut was: Local<Option<bool>>) {
    let on = state.switches.on(Switch::CompatibleHighlight);
    let before = was.replace(on);
    if before.is_none_or(|before| before == on) {
        return;
    }
    let want = on && state.in_force.inventory;
    println!("options: COMPATIBLE ITEMS HIGHLIGHT {} at once", if want { "on" } else { "off" });
    if state.in_force.compatible_highlight != want {
        state.bypass_change_detection().in_force.compatible_highlight = want;
    }
    if features.compatible_highlight != want {
        features.compatible_highlight = want;
    }
}

/// The tab and the run's skills kept as one (`skills::Skills`): a level
/// changed on the tab is the run's at once (and kept when the bar is let
/// go); one a host game changed in play shows on the tab and is kept in
/// the app's file, fractions of a level and all.
pub(super) fn sync_skills(mut state: ResMut<OptionsState>, skills: Option<ResMut<crate::player::skills::Skills>>, mut agreed: Local<Option<trp_sim::condition::skills::SkillLevels>>) {
    let Some(mut skills) = skills else { return };
    if state.skills != skills.levels {
        if agreed.as_ref().is_some_and(|was| *was != skills.levels) {
            state.skills = skills.levels.clone();
            state.keep_due = true;
            state.dirty = true;
        } else {
            skills.levels = state.skills.clone();
        }
    }
    if agreed.as_ref() != Some(&skills.levels) {
        *agreed = Some(skills.levels.clone());
    }
}

/// The tab and the run's mastering kept as one (`skills::Mastery`), as the
/// skills are (`sync_skills`): a level changed on the tab is the run's at
/// once (and kept when the bar is let go); a family begun by a draw, or
/// one a host game changed in play, shows on the tab and is kept in the
/// app's file.
pub(super) fn sync_mastery(mut state: ResMut<OptionsState>, mastery: Option<ResMut<crate::player::skills::Mastery>>, mut agreed: Local<Option<trp_sim::condition::mastering::MasteringLevels>>) {
    let Some(mut mastery) = mastery else { return };
    if state.mastering != mastery.levels {
        if agreed.as_ref().is_some_and(|was| *was != mastery.levels) {
            state.mastering = mastery.levels.clone();
            state.keep_due = true;
            state.dirty = true;
        } else {
            mastery.levels = state.mastering.clone();
        }
    }
    if agreed.as_ref() != Some(&mastery.levels) {
        *agreed = Some(mastery.levels.clone());
    }
}

/// The tab and the run's experience kept as one (`experience::Experience`),
/// as the skills are: a level set on the tab is the run's at once; what
/// play earns shows on the tab and is kept in the app's file.
pub(super) fn sync_experience(mut state: ResMut<OptionsState>, experience: Option<ResMut<crate::player::experience::Experience>>, mut agreed: Local<Option<i64>>) {
    let Some(mut experience) = experience else { return };
    if state.experience != experience.total {
        if agreed.is_some_and(|was| was != experience.total) {
            state.experience = experience.total;
            state.keep_due = true;
            state.dirty = true;
        } else {
            experience.total = state.experience;
        }
    }
    if *agreed != Some(experience.total) {
        *agreed = Some(experience.total);
    }
}

/// What play changed (skills, mastering, experience earned or a host's)
/// kept in the app's file at most every two seconds: every award would
/// otherwise write it (a stimulant's energy, every frame). `ESTIMATE`: the
/// app's own two seconds.
pub(super) fn keep_in_play(mut state: ResMut<OptionsState>, bindings: Res<Bindings>, time: Res<Time<Real>>) {
    let now = time.elapsed_secs_f64();
    if state.keep_due && now - state.kept_at >= 2.0 {
        let state = state.bypass_change_detection();
        state.keep(&bindings.controls);
        (state.keep_due, state.kept_at) = (false, now);
    }
}

/// Bevy's multisampling for a count of samples.
pub fn msaa_of(samples: u8) -> bevy::render::view::Msaa {
    match samples {
        1 => bevy::render::view::Msaa::Off,
        2 => bevy::render::view::Msaa::Sample2,
        _ => bevy::render::view::Msaa::Sample4,
    }
}

/// Puts the display's settings on the window and the view when they have
/// changed, and the volumes to the sounds. Each is the game's setting of
/// that name; the game puts them on when its settings screen is saved, the
/// app as each is chosen.
#[allow(clippy::too_many_arguments)]
pub(super) fn apply(
    state: Res<OptionsState>,
    mut options: ResMut<Options>,
    mut window: Query<&mut Window, With<PrimaryWindow>>,
    monitors: Query<&Monitor>,
    hands: Option<ResMut<HandsRig>>,
    stowed: Option<ResMut<Stowed>>,
    // The scope's camera takes the main camera's multisampling (`optic_view::spawn_camera`).
    mut cameras: Query<&mut bevy::render::view::Msaa, Or<(With<crate::MainCamera>, With<crate::weapons::optic_view::OpticCamera>)>>,
    mut levels: ResMut<crate::audio::ui_sound::Levels>,
    mut applied: Local<Option<Graphics>>,
) {
    // The mouse's settings are what the view is turned by from now on.
    let mouse = &state.mouse;
    if (options.sensitivity, options.aiming_sensitivity, options.optic_sensitivity, options.invert) != (mouse.sensitivity, mouse.aiming, mouse.optic, mouse.invert) {
        (options.sensitivity, options.aiming_sensitivity, options.optic_sensitivity, options.invert) = (mouse.sensitivity, mouse.aiming, mouse.optic, mouse.invert);
    }
    // The volumes are heard as they are slid.
    if *levels != state.sound {
        *levels = state.sound;
    }
    if applied.as_ref() == Some(&state.graphics) {
        return;
    }
    let now = state.graphics.clone();
    // As the run begins there is nothing to do, unless the app's file
    // said how the display is to be.
    let was = applied.replace(now.clone());
    let Some(was) = was.or_else(|| state.apply_at_start.then(|| Graphics { vsync: !now.vsync, field_of_view: f32::NAN, mode: Mode::Windowed, resolution: [0, 0], ..now.clone() })) else { return };
    let Ok(mut window) = window.single_mut() else { return };
    if now.vsync != was.vsync {
        window.present_mode = if now.vsync { PresentMode::AutoVsync } else { PresentMode::AutoNoVsync };
    }
    if now.msaa != was.msaa {
        for mut msaa in &mut cameras {
            *msaa = msaa_of(now.msaa);
        }
    }
    if now.dust_resolution != was.dust_resolution {
        crate::effects::offscreen_dust::set(now.dust_resolution);
        info!("dust resolution: {}, from the next frame", now.dust_resolution.key());
    }
    if now.field_of_view != was.field_of_view {
        options.field_of_view = now.field_of_view;
        if let Some(mut hands) = hands {
            hands.hands.set_field_of_view(now.field_of_view);
        }
        if let Some(mut stowed) = stowed {
            stowed.set_field_of_view(now.field_of_view);
        }
    }
    if now.mode != was.mode || now.resolution != was.resolution {
        let [width, height] = now.resolution;
        match now.mode {
            Mode::Windowed => {
                window.mode = WindowMode::Windowed;
                window.resolution.set_physical_resolution(width, height);
            }
            Mode::Borderless => window.mode = WindowMode::BorderlessFullscreen(MonitorSelection::Current),
            Mode::Fullscreen => {
                // The screen's mode of that size with the most frames a
                // second, or the mode it is in if it has none.
                let modes = monitors.iter().flat_map(|monitor| monitor.video_modes.iter().copied());
                let wanted = modes.filter(|mode| mode.physical_size == UVec2::new(width, height)).max_by_key(|mode| (mode.refresh_rate_millihertz, mode.bit_depth));
                window.mode = WindowMode::Fullscreen(MonitorSelection::Current, wanted.map_or(VideoModeSelection::Current, VideoModeSelection::Specific));
            }
        }
    }
}
