//! The renderer's own systems, the wardrobe, and the pictures' streaming and the frame's timing.

use super::*;

/// Adds the rigs' uploads watch, the wardrobe, the renderer's counting and timing, and the pictures' streaming.
pub(super) fn add(mut app: App, waiting: PipelinesWaiting, wardrobe: Option<player::clothing::Clothing>) -> App {
    // What a rig brought to the graphics card, asked about here and
    // answered by the renderer (`weapon::Uploads`).
    let uploads = weapons::weapon::UploadWatch::default();
    app.insert_resource(uploads.clone());
    if let Some(wardrobe) = wardrobe {
        app.insert_resource(wardrobe);
    }
    if let Some(renderer) = app.get_sub_app_mut(bevy::render::RenderApp) {
        use bevy::render::{Render, RenderSystems};
        renderer.insert_resource(waiting).add_systems(Render, count_waiting.in_set(RenderSystems::Cleanup));
        renderer.insert_resource(uploads).add_systems(Render, weapons::weapon::watch_uploads.in_set(RenderSystems::Cleanup));
        // Where a slow frame of the renderer's goes, if asked for.
        platform::render_timing::add(renderer);
        platform::render_timing::add_memory(renderer);
    }
    // New pictures and materials taken in, and the levels the pictures want
    // worked out, after the frame and before the card takes what changed
    // (`streaming`). The copies made ready taken before the stores say what
    // changed, so that the card hears of a copy with its materials.
    app.add_systems(PostUpdate, assets::streaming::take_made.before(bevy::asset::AssetEventSystems));
    assets::streaming::watch_card(&mut app);
    app.add_systems(Last, (assets::streaming::take_new, assets::streaming::stream).chain());
    platform::render_timing::add_main(&mut app);
    platform::render_timing::add_census(&mut app);
    platform::render_timing::add_drop_hidden(&mut app);
    app
}
