//! The game's frame rate limit in a raid (`docs/spec/settings.md`, "The
//! frame rate limit"; `GraphicsSettingsController.ChangeFramerate`): with
//! VSync off and the limit not turned off, the frames are held to the
//! GAME FPS LIMIT, as the engine's `Application.targetFrameRate` holds the
//! game's; with VSync on the screen alone paces them, as in the game's
//! raids (its `targetFrameRate` is -1 there). NVIDIA Reflex, which lifts it
//! too, is not built.
//!
//! The renderer waits, just before it draws and presents the frame, for
//! the frame's time: a sleep to within a millisecond of it, then a short
//! spin, each frame's time counted from the last one's so that the frames
//! do not drift. There, on the render thread or not, the frames reach the
//! screen evenly: the engine stamps its frame clock as it presents (a wait
//! at the end of the main loop held the render thread's frames to the rate
//! but let them come 2 and 20 ms apart now and then). A run nobody is at
//! the keys of holds none unless told (`--fps-limit`), so that the timing
//! runs measure the frame uncapped as before.

use std::time::{Duration, Instant};

use bevy::prelude::*;
use bevy::render::extract_resource::{ExtractResource, ExtractResourcePlugin};
use bevy::render::{Render, RenderApp, RenderSystems};

/// What holds the frames this run.
#[derive(Resource)]
pub struct FrameLimit {
    /// The command line's word (`--fps-limit <n|off>`): a limit, or none;
    /// otherwise the options' (`OptionsState::graphics`).
    pub told: Option<Option<u32>>,
    /// Whether someone is at the keys: else only the command line holds.
    pub attended: bool,
}

impl FrameLimit {
    pub fn new(told: Option<Option<u32>>, attended: bool) -> Self {
        Self { told, attended }
    }
}

/// The frames a second the renderer holds this frame to, or none, handed
/// to it with the frame.
#[derive(Resource, Clone, Copy, Default, ExtractResource)]
struct Held(Option<u32>);

/// When the renderer let the last frame go.
#[derive(Resource, Default)]
struct LastLetGo(Option<Instant>);

pub struct FrameLimitPlugin;

impl Plugin for FrameLimitPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Held>().add_plugins(ExtractResourcePlugin::<Held>::default()).add_systems(Last, decide);
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else { return };
        render_app.init_resource::<LastLetGo>().add_systems(Render, hold.in_set(RenderSystems::Render).before(bevy::render::renderer::render_system));
    }
}

fn decide(limit: Option<Res<FrameLimit>>, state: Res<crate::options_view::OptionsState>, mut held: ResMut<Held>) {
    let wanted = limit.and_then(|limit| wanted(&limit, &state.graphics));
    if held.0 != wanted {
        held.0 = wanted;
    }
}

/// The frames a second the frames are held to now, or none.
fn wanted(limit: &FrameLimit, graphics: &crate::options_view::Graphics) -> Option<u32> {
    if graphics.vsync {
        return None;
    }
    let [low, high] = crate::options_view::GAME_FPS;
    match limit.told {
        Some(told) => told.map(|fps| fps.clamp(low, high)),
        None if !limit.attended => None,
        None => (!graphics.fps_limit_off).then(|| graphics.game_fps.clamp(low, high)),
    }
}

fn hold(held: Option<Res<Held>>, mut last: ResMut<LastLetGo>) {
    let Some(fps) = held.and_then(|held| held.0) else {
        last.0 = None;
        return;
    };
    let period = Duration::from_secs_f64(1.0 / fps as f64);
    let now = Instant::now();
    // The next frame's time from the last's; a frame that ran longer than
    // the period starts the count again from now.
    let at = match last.0 {
        Some(last) if last + period > now => last + period,
        _ => now,
    };
    let left = at.saturating_duration_since(now);
    if left > Duration::from_millis(1) {
        std::thread::sleep(left - Duration::from_millis(1));
    }
    while Instant::now() < at {
        std::hint::spin_loop();
    }
    last.0 = Some(at);
}
