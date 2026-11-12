//! Tracer rounds' traces on the range.
//! After the ballistics spec, "A tracer"; the options' `TRACERS`: the game's renderer read as the
//! run begins (`TracersLight`), and on every frame each bullet in flight that shows a tracer made
//! into its streak's corners (`trp_sim::tracers`) and handed to each camera that draws the world's
//! layer. `muzzle_view`'s pass adds them to the frame as the game's `Custom/TracerLight` does
//! (`tracer.wgsl`).

use std::sync::Arc;

use bevy::ecs::query::QueryItem;
use bevy::prelude::*;
use bevy::render::extract_component::{ExtractComponent, ExtractComponentPlugin};
use bevy::render::render_resource::ShaderType;
use trp_sim::combat::tracers::{self, Figures, Flight};

/// The tracers' systems; `on` is the options' `TRACERS` (with
/// `BALLISTICS`) as the run began. Off, nothing of the renderer is read and
/// no tracer is drawn.
pub struct TracerViewPlugin {
    pub on: bool,
}

impl Plugin for TracerViewPlugin {
    fn build(&self, app: &mut App) {
        if self.on {
            app.add_plugins(ExtractComponentPlugin::<TracerScreen>::default()).add_systems(
                PostUpdate,
                tracer_frame.run_if(resource_exists::<Tracers>).after(bevy::transform::TransformSystems::Propagate).after(bevy::camera::CameraUpdateSystems),
            );
        }
    }
}

/// What the tracers' shader is given: the camera's matrix and the
/// material's `_Brightness`.
#[derive(Clone, Copy, Default, ShaderType)]
pub struct TracerUniform {
    pub clip_from_world: Mat4,
    /// `_Brightness`, then nothing.
    pub brightness: Vec4,
}

/// The tracers to draw this frame: the figures, the picture, and each
/// tracer's twelve corners (place in the game's axes, picture's place,
/// colour: nine numbers each).
#[derive(Clone)]
pub struct TracerDraw {
    pub uniform: TracerUniform,
    pub picture: Handle<Image>,
    pub corners: Arc<Vec<f32>>,
    pub tracers: u32,
}

/// Numbers a corner.
pub const CORNER_NUMBERS: usize = 9;

/// On a camera: the tracers while any shows.
#[derive(Component, Clone, Default)]
pub struct TracerScreen(pub Option<TracerDraw>);

impl ExtractComponent for TracerScreen {
    type QueryData = &'static TracerScreen;
    type QueryFilter = ();
    type Out = TracerScreen;

    fn extract_component(screen: QueryItem<'_, '_, Self::QueryData>) -> Option<Self::Out> {
        screen.0.as_ref().is_some_and(|draw| draw.tracers > 0).then(|| screen.clone())
    }
}

/// The renderer as read: its figures, its material's picture and
/// brightness.
#[derive(Resource)]
pub struct Tracers {
    figures: Figures,
    picture: Handle<Image>,
    brightness: f32,
}

/// The tracers' renderer read, at the start.
pub(crate) fn prepare_tracers(unity: &mut trp_data::unity::Assets, images: &mut impl crate::assets::background::AddImage) -> Option<Tracers> {
    let data = match trp_data::drawing::particles::tracers_light(unity) {
        Ok(data) => data,
        Err(e) => {
            warn!("no tracer is drawn: the tracers' renderer could not be read: {e:#}");
            return None;
        }
    };
    let Some(picture) = crate::effects::particles_view::picture(&data.texture, images) else { return None };
    info!("the tracers: {} with {} ({} px), {} s at most", data.shader, data.texture.name, data.texture.width, data.max_flying_time);
    Some(Tracers { figures: Figures::of(&data), picture, brightness: data.brightness })
}

/// A frame, once everything is placed: every bullet in flight that shows a
/// tracer (`TracersLight.WorkCycle`), its corners once in the world, handed
/// to each camera that draws them (the main camera, and the optic camera
/// while on: the renderer is an object of the world's layer).
/// `PROVISIONAL`: at the bullets' places after this frame's tick (the game's
/// script and its world's tick run at the same execution order).
fn tracer_frame(
    tracers: Res<Tracers>,
    bullets: Option<Res<crate::weapons::bullets::Bullets>>,
    frame: Res<bevy::diagnostic::FrameCount>,
    mut logged: Local<(u32, u32)>,
    mut cameras: Query<(&Camera, &GlobalTransform, &mut TracerScreen)>,
) {
    let mut corners = Vec::new();
    let mut count = 0u32;
    let mut first = None;
    for flight in bullets.iter().flat_map(|bullets| bullets.in_flight()).filter_map(Flight::of) {
        let Some(tracer) = tracers::tracer(&flight, &tracers.figures) else { continue };
        for corner in tracers::corners(&tracer) {
            corners.extend_from_slice(&corner.position);
            corners.extend_from_slice(&corner.uv);
            corners.extend_from_slice(&corner.color);
        }
        first.get_or_insert(tracer);
        count += 1;
    }
    // When tracers begin and cease to show, the first sixteen times: a
    // capture's frame to take.
    let (before, lines) = &mut *logged;
    if (count > 0) != (*before > 0) && *lines < 16 {
        *lines += 1;
        match first {
            Some(t) => info!(
                "tracers shown from frame {}: {} of them, the first at ({:.1}, {:.1}, {:.1}), {:.2} m wide, {:.1} m long, colour ({:.3}, {:.3}, {:.3}, {:.2})",
                frame.0, count, t.head[0], t.head[1], t.head[2], t.size, t.length, t.color[0], t.color[1], t.color[2], t.color[3]
            ),
            None => info!("no tracer shown from frame {}", frame.0),
        }
    }
    *before = count;
    let shown = (count > 0).then(|| Arc::new(corners));
    for (camera, eye, mut screen) in &mut cameras {
        let Some(corners) = shown.as_ref().filter(|_| camera.is_active) else {
            if screen.0.is_some() {
                screen.0 = None;
            }
            continue;
        };
        screen.0 = Some(TracerDraw {
            uniform: TracerUniform { clip_from_world: camera.clip_from_view() * eye.to_matrix().inverse(), brightness: Vec4::new(tracers.brightness, 0.0, 0.0, 0.0) },
            picture: tracers.picture.clone(),
            corners: corners.clone(),
            tracers: count,
        });
    }
}
