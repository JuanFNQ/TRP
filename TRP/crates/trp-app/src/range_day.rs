//! The range's own light by the hour (the options' TIME OF DAY, with
//! `CUSTOMS LIGHT` off; the user, 2026-10-25: "lets do the hour option, can
//! we put a slider in-game so we can change the time at any time?"): the
//! range's own sun, ambient and sky (the app's, not the game's: `lighting`'s
//! `RANGE_OWN_*`) carried through Customs' day and night by Customs' time of
//! day (`trp_sim::tod`, the game's rules): each, at an hour, the range's own
//! at the range's own hour times what Customs' changed by between the two,
//! so the range is as it was until the hour moves. The sun turns as Customs'
//! light turns (the moon's by night), from the range's own sun's way; its
//! light, the ambient and the sky's colour go by Customs' light, ambient and
//! sky at the horizon. The drawing alone: nothing of the simulation goes by
//! it. With `CUSTOMS LIGHT` on, the hour is taken as the run begins.

use std::sync::mpsc::{channel, Receiver};
use std::sync::Mutex;

use bevy::prelude::*;
use trp_sim::tod::{Atmosphere, Sky};

use crate::lighting::{linear, range_own_sun_turn, RangeSun, RANGE_MOMENT, RANGE_OWN_AMBIENT, RANGE_OWN_SKY, RANGE_OWN_SUN};

/// Customs' sky as read (on a thread as the run begins), the range's own
/// hour's look by it, and the hour last put on the range.
#[derive(Resource, Default)]
pub struct RangeDay {
    coming: Option<Mutex<Receiver<(Sky, Atmosphere)>>>,
    sky: Option<(Sky, Atmosphere)>,
    reference: Option<Look>,
    applied: Option<f32>,
}

/// What Customs' time of day gives at an hour: its light (colour times
/// intensity, linear) and the light's turn (the game's axes), its ambient
/// (linear), its sky's colour at the horizon (as the game keeps it), and
/// its day's share (`LerpValue`: 0 with the sun 20° under the horizon, 1
/// from 10° over it).
#[derive(Clone, Copy, Debug)]
struct Look {
    light: [f32; 3],
    turn: [f32; 4],
    ambient: [f32; 3],
    sky: [f32; 3],
    day: f32,
}

fn look(sky: &Sky, stored: Atmosphere, hour: f32) -> Look {
    let state = sky.settle(hour, RANGE_MOMENT.cloudiness, stored, 12);
    let light = state.light_color.map(|c| linear(c) * state.light_intensity);
    Look { light, turn: state.light_rotation, ambient: state.ambient_color.map(linear), sky: state.top_horizon_sky_color, day: state.lerp_value }
}

/// How much of the range's own light is left at full night, the day's share
/// fading it there (`ESTIMATE`: the app's own night, not the game's, whose
/// camera's exposure adapts to its darkness; chosen so that a weapon's light
/// shows on the range: 5% of the light, the ambient and the sky).
const NIGHT_SHARE: f32 = 0.05;

/// Starts reading Customs' sky for the range's own light by the hour.
pub fn start(app: &mut App, install: trp_data::Install) {
    let (send, receive) = channel();
    std::thread::spawn(move || {
        let read = || -> anyhow::Result<(Sky, Atmosphere)> {
            let level = trp_data::tod::TimeOfDayLevel::customs(&install)?;
            let sky = Sky::of_level(&level)?;
            let stored = trp_sim::tod::stored_atmosphere(&level.sky)?;
            Ok((sky, stored))
        };
        match read() {
            Ok(read) => {
                let _ = send.send(read);
            }
            Err(e) => eprintln!("Customs' time of day could not be read; the range's own light keeps its one hour: {e:#}"),
        }
    });
    app.insert_resource(RangeDay { coming: Some(Mutex::new(receive)), ..default() }).add_systems(Update, follow_hour);
}

/// A share of one figure by another, none where the other is none.
fn share(now: f32, then: f32) -> f32 {
    if then > 1e-6 {
        now / then
    } else {
        0.0
    }
}

/// The range's own light put at the options' hour whenever it changes.
fn follow_hour(
    mut day: ResMut<RangeDay>,
    options: Res<crate::options_view::OptionsState>,
    mut sun: Query<(&mut DirectionalLight, &mut Transform), With<RangeSun>>,
    mut ambient: ResMut<GlobalAmbientLight>,
    mut clear: ResMut<ClearColor>,
) {
    let day = &mut *day;
    if day.sky.is_none() {
        let Some(read) = day.coming.as_ref().and_then(|coming| coming.lock().ok()?.try_recv().ok()) else { return };
        day.reference = Some(look(&read.0, read.1, RANGE_MOMENT.hour));
        day.sky = Some(read);
        day.coming = None;
    }
    let (Some((sky, stored)), Some(reference)) = (day.sky.as_ref(), day.reference) else { return };
    let hour = options.hour;
    if day.applied == Some(hour) {
        return;
    }
    day.applied = Some(hour);
    let now = look(sky, *stored, hour);
    // The night's fade by Customs' day's share, none at the range's own hour.
    let fade = (NIGHT_SHARE + (1.0 - NIGHT_SHARE) * now.day) / (NIGHT_SHARE + (1.0 - NIGHT_SHARE) * reference.day).max(1e-6);
    // The light: its colour by Customs' light's change on each channel, its
    // strength by the most changed.
    let light: [f32; 3] = std::array::from_fn(|c| share(now.light[c], reference.light[c]));
    let most = light.iter().copied().fold(0.0, f32::max);
    let turn = crate::convert::rotation(now.turn) * crate::convert::rotation(reference.turn).inverse();
    if let Ok((mut sun, mut transform)) = sun.single_mut() {
        sun.illuminance = RANGE_OWN_SUN * most * fade;
        sun.color = if most > 0.0 { Color::linear_rgb(light[0] / most, light[1] / most, light[2] / most) } else { Color::WHITE };
        transform.rotation = turn * range_own_sun_turn().rotation;
    }
    let lit: [f32; 3] = std::array::from_fn(|c| share(now.ambient[c], reference.ambient[c]));
    let lit_most = lit.iter().copied().fold(0.0, f32::max);
    ambient.brightness = RANGE_OWN_AMBIENT * lit_most * fade;
    ambient.color = if lit_most > 0.0 { Color::linear_rgb(lit[0] / lit_most, lit[1] / lit_most, lit[2] / lit_most) } else { Color::WHITE };
    let seen: [f32; 3] = std::array::from_fn(|c| (RANGE_OWN_SKY[c] * share(now.sky[c], reference.sky[c]) * fade).clamp(0.0, 1.0));
    clear.0 = Color::srgb(seen[0], seen[1], seen[2]);
    info!(
        "the range's own light at {:02}:{:02} (the day's share {:.2}): the sun {:.0} lux, the ambient {:.1}, the sky {:?}",
        hour as u32 % 24,
        (hour.fract() * 60.0) as u32,
        now.day,
        sun.single().map(|(s, _)| s.illuminance).unwrap_or(0.0),
        ambient.brightness,
        seen
    );
}
