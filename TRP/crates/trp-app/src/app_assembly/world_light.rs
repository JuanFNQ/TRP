//! The world's light: Customs' light and clouds, the camera's filter and sharpening, and the shading chosen.

use super::*;

pub(super) struct WorldLight {
    pub(super) filter_table: Option<trp_data::drawing::prism::FilterTable>,
    pub(super) level_lighting: Option<rendering::lighting::LevelLighting>,
    pub(super) own_sharpening: Option<rendering::lighting::Sharpening>,
    pub(super) game_shading: bool,
    pub(super) figure_shading: bool,
    pub(super) boot_shadow: Option<Install>,
    pub(super) range_own_light: bool,
}

/// Reads Customs' light or the camera's sharpening alone, and settles the shading and the range's own light.
pub(super) fn read(options_state: &screens::options_view::OptionsState, options: &Options, unity: &UnityAssets, boot_shadow: bool) -> WorldLight {
    // The range lit as Customs in the recorded morning, with the options'
    // `CUSTOMS LIGHT` (`lighting`): read and worked out before the window
    // opens.
    let mut filter_table = None;
    let level_lighting = if options_state.in_force.customs_light {
        // At the options' TIME OF DAY as the run began (`--hour` over it).
        let moment = trp_sim::lighting::tod::Moment {
            hour: options_state.hour_started,
            cloudiness: options.cloudiness.unwrap_or(rendering::lighting::RANGE_MOMENT.cloudiness),
            wind: rendering::lighting::RANGE_MOMENT.wind,
        };
        // The clouds at the quality the run began with, with `CUSTOMS CLOUDS`.
        let clouds_quality = options_state.in_force.customs_clouds.then_some(options_state.clouds_started);
        match rendering::lighting::read(&unity.install, moment, clouds_quality) {
            Ok(mut light) => {
                if let Some(c) = &light.clouds {
                    let f = &c.figures;
                    println!(
                        "Customs' clouds at {} quality: the map's opacities {:?}, the wind {:.1} km/h, the sun's colour for them {:?}, their ambient {:?}{}",
                        options_state.clouds_started.key().to_lowercase(),
                        f.opacities,
                        f.scroll_speed,
                        f.sun_color,
                        f.ambient,
                        f.shadows.map_or(String::new(), |s| format!(", their shadows {} square over {} m", s.resolution, s.size))
                    );
                }
                // The camera's own colour filter, with `CAMERA COLOR FILTER`.
                if !options_state.in_force.camera_color_filter {
                    light.color_filter = None;
                }
                // The camera's sharpening and desaturation, with `CAMERA SHARPENING`.
                if !options_state.in_force.camera_sharpening {
                    light.sharpen = None;
                } else if let Some(s) = &light.sharpen {
                    info!("the camera's sharpening: strength {} (clamp {}), the weather's desaturation {:.3}, the ring {:?} by {}", s.strength, s.data.clamp, s.weather, s.data.min_max_radius, s.data.radius_falloff);
                }
                // The sky's scattering over the scene, with `SKY SCATTERING`.
                if !options_state.in_force.sky_scattering {
                    light.scattering = None;
                }
                if let Some(s) = &light.scattering {
                    println!("the sky's scattering over the scene: density {}, height falloff {}, zero level {}, glow {}", s.density, s.height_falloff, s.zero_level, s.sunrise_glow);
                }
                if let Some(filter) = &light.color_filter {
                    match filter.table(&unity.install) {
                        Ok(table) => {
                            println!("the camera's colour filter: {} at {}", filter.name().unwrap_or_default(), filter.amount);
                            filter_table = Some(table);
                        }
                        Err(e) => {
                            eprintln!("the camera's colour filter could not be read; it is not drawn: {e:#}");
                            light.color_filter = None;
                        }
                    }
                }
                println!(
                    "the range lit as Customs at {:02}:{:02} (cloudiness {}): the sun {:?} going {:?}, the ambient from above {:?}, the sky's dome of {} vertices",
                    moment.hour as u32,
                    (moment.hour.fract() * 60.0) as u32,
                    moment.cloudiness,
                    light.sun,
                    light.sun_forward,
                    light.top_ambient,
                    light.sky.mesh.positions.len()
                );
                Some(light)
            }
            Err(e) => {
                eprintln!("Customs' light could not be read; the range keeps its own: {e:#}");
                None
            }
        }
    } else {
        None
    };
    // Without Customs' light, the camera's sharpening read on its own, with
    // `CAMERA SHARPENING`: Customs'
    // camera's, with no weather's share, there being no weather.
    let own_sharpening = if level_lighting.is_none() && options_state.in_force.camera_sharpening {
        match rendering::lighting::camera_sharpening(&unity.install) {
            Ok(s) => {
                println!("the camera's sharpening, without Customs' light: strength {} (clamp {}), the ring {:?} by {}", s.strength, s.data.clamp, s.data.min_max_radius, s.data.radius_falloff);
                Some(s)
            }
            Err(e) => {
                eprintln!("the camera's sharpening could not be read; it is not drawn: {e:#}");
                None
            }
        }
    } else {
        None
    };
    // Tarkov's surface shading, with `TARKOV SHADING` and the level's light
    // (`shading`).
    let game_shading = options_state.in_force.tarkov_shading && level_lighting.is_some();
    // And for the OVERALL tab's own figure, with `OVERALL FIGURE`, whatever
    // the world's: its eight lights summed as the game sums them
    // (`figure_view`; inventory spec, "The character's light").
    let figure_shading = options_state.in_force.character_model && options_state.in_force.overall_figure;
    let boot_shadow = boot_shadow.then(|| unity.install.clone());
    // Without Customs' light, the range's own by the options' hour
    // (`range_day`).
    let range_own_light = level_lighting.is_none();
    if game_shading {
        println!("the scene lit by Tarkov's surface shading");
    }
    WorldLight { filter_table, level_lighting, own_sharpening, game_shading, figure_shading, boot_shadow, range_own_light }
}
