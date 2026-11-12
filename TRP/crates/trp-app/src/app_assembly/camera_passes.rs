//! The camera's passes and the world's light, from the tonemap to the frame limit and the gamma.

use super::*;

pub(super) struct CameraInputs {
    pub(super) filter_table: Option<trp_data::drawing::prism::FilterTable>,
    pub(super) scope_pictures: bool,
    pub(super) game_shading: bool,
    pub(super) figure_shading: bool,
    pub(super) boot_shadow: Option<Install>,
    pub(super) limiter: platform::frame_limit::FrameLimit,
    pub(super) range_own_light: bool,
    pub(super) day_install: Install,
    pub(super) level_lighting: Option<rendering::lighting::LevelLighting>,
    pub(super) own_sharpening: Option<rendering::lighting::Sharpening>,
    pub(super) gamma_composed: bool,
}

/// Adds the camera's passes, the frame limit and the world's light, ending with the gamma passes.
pub(super) fn add(mut app: App, inputs: CameraInputs) -> App {
    let CameraInputs { filter_table, scope_pictures, game_shading, figure_shading, boot_shadow, limiter, range_own_light, day_install, level_lighting, own_sharpening, gamma_composed } = inputs;
    // The camera's PRISM, before its screen effects, and the sky, with
    // `CUSTOMS LIGHT`: the light figures, the environment's exposure, no
    // flat ambient of Bevy's (the ambient is the environment map's).
    app.add_plugins((rendering::prism_view::PrismPlugin, rendering::tod_sky_view::TodSkyPlugin, rendering::color_filter_view::ColorFilterPlugin { table: filter_table }, rendering::scattering_view::ScatteringPlugin, rendering::lighting::HallProbePlugin));
    app.add_plugins(screens::preview_tonemap::PreviewTonemapPlugin);
    app.add_plugins(screens::preview_outline::PreviewOutlinePlugin);
    // The ambient occlusion, between the depth's copy and the scattering
    // (`hbao_view`): after both their plugins.
    app.add_plugins(rendering::hbao_view::HbaoPlugin);
    // The dithered glass and its undithering, between the scattering and
    // the explosions (`glass_view`): after their plugins.
    app.add_plugins(effects::glass_view::GlassPlugin);
    // The clouds, between the depth's copy and the ambient occlusion
    // (`clouds_view`): after their plugins.
    app.add_plugins(rendering::clouds_view::CloudsPlugin);
    // The ambient highlight's twins (`highlight_view`).
    app.add_plugins(rendering::highlight_view::HighlightPlugin);
    // The scopes' lenses and their optic camera (`optic_view`).
    app.add_plugins(weapons::optic_view::OpticViewPlugin { pictures: scope_pictures });
    // The handheld rangefinder's readout (`rangefinder_view`).
    app.add_plugins(weapons::rangefinder_view::RangefinderViewPlugin);
    // Bevy's PBR lighting patched for the game's shading (`shading`).
    app.add_plugins(rendering::shading::ShadingPlugin { on: game_shading, figure: figure_shading });
    // The shadow at the OVERALL figure's feet, with its figure
    // (`boot_shadow`).
    if let Some(install) = boot_shadow {
        app.add_plugins(screens::boot_shadow::BootShadowPlugin { install });
    }
    // The game's frame rate limit (`frame_limit`).
    app.insert_resource(limiter).add_plugins(platform::frame_limit::FrameLimitPlugin);
    if range_own_light {
        world::range_day::start(&mut app, day_install);
    }
    if let Some(light) = level_lighting {
        let [grey, speed, fade] = light.outdoor;
        app.insert_resource(rendering::prism_view::PrismEnvironment { exposure: trp_sim::lighting::prism::EnvironmentExposure::outdoor(grey, speed, fade), indoor: light.indoor });
        app.insert_resource(GlobalAmbientLight { brightness: 0.0, ..default() });
        app.insert_resource(light);
    }
    if let Some(sharpening) = own_sharpening {
        app.insert_resource(rendering::sharpen_view::OwnSharpening(sharpening));
    }
    // The camera's effects in gamma space, and the screens with `TARKOV UI`.
    app.add_plugins(ui::gamma_ui::GammaUiPlugin { ui: gamma_composed });
    app
}
