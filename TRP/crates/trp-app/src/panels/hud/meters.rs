//! The meters spawned and shown, and the frame rate.

use super::*;

pub(super) fn spawn(mut commands: Commands, options: Res<crate::Options>) {
    let track = |width: f32, height: f32| {
        (Node { width: Val::Px(width), height: Val::Px(height), ..default() }, BackgroundColor(TRACK))
    };
    let _ = &options;
    commands
        .spawn((
            ScreenHides,
            PlainMeters,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(16.0),
                bottom: Val::Px(16.0),
                column_gap: Val::Px(10.0),
                align_items: AlignItems::FlexEnd,
                ..default()
            },
        ))
        .with_children(|panel| {
            // How high the character stands: a column that fills from the
            // bottom, empty lying down.
            panel.spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(3.0), ..default() }).with_children(
                |stance| {
                    stance
                        .spawn((
                            Node {
                                width: Val::Px(10.0),
                                height: Val::Px(STANCE_HEIGHT),
                                flex_direction: FlexDirection::ColumnReverse,
                                ..default()
                            },
                            BackgroundColor(TRACK),
                        ))
                        .with_children(|column| {
                            column.spawn((
                                Fill::Stance,
                                Node { width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
                                BackgroundColor(SET),
                            ));
                        });
                },
            );
            panel.spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(4.0), ..default() }).with_children(
                |bars| {
                    bars.spawn((StanceWord, label("standing")));
                    // The speed setting, over the notches the state allows.
                    bars.spawn(label("speed"));
                    bars.spawn(Node { column_gap: Val::Px(2.0), ..default() }).with_children(|row| {
                        for notch in 0..NOTCHES {
                            row.spawn((
                                Notch(notch),
                                Node { width: Val::Px(NOTCH_WIDTH), height: Val::Px(10.0), ..default() },
                                BackgroundColor(REFUSED),
                            ));
                        }
                    });
                    for (fill, name, colour) in [(Fill::Legs, "legs", LEGS), (Fill::Arms, "arms", ARMS)] {
                        bars.spawn(label(name));
                        bars.spawn(track(BAR_WIDTH, 7.0)).with_children(|bar| {
                            bar.spawn((
                                fill,
                                Node { width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
                                BackgroundColor(colour),
                            ));
                        });
                    }
                },
            );
        });
}

pub(super) fn show(
    controller: Res<Controller>,
    (features, look): (Res<crate::screens::options_view::Features>, Res<crate::screens::health_view::HealthLook>),
    mut fills: Query<(&Fill, &mut Node, &mut BackgroundColor), Without<Notch>>,
    mut notches: Query<(&Notch, &mut BackgroundColor), Without<Fill>>,
    mut word: Query<&mut Text, With<StanceWord>>,
) {
    // Nothing written while the meters are hidden (a node or a text
    // written lays the interface out again, seen or not); written on the
    // frame they show, by the rule that shows them.
    if hidden_over_screen(&controller, &features, &look, true) {
        return;
    }
    let _clock = crate::platform::render_timing::Clock::new("hud show");
    let m = &controller.movement;
    let stamina = &m.stamina;
    for (fill, mut node, mut colour) in &mut fills {
        match fill {
            Fill::Legs | Fill::Arms => {
                let (pool, full) = if matches!(fill, Fill::Legs) { (&stamina.legs, LEGS) } else { (&stamina.arms, ARMS) };
                // (Written only when it changes: a node written is laid
                // out afresh.)
                let width = Val::Percent(pool.level().clamp(0.0, 1.0) * 100.0);
                if node.width != width {
                    node.width = width;
                }
                // The game's bars take another look under the threshold.
                let look = if pool.exhausted() { SPENT } else { full };
                if colour.0 != look {
                    colour.0 = look;
                }
            }
            Fill::Stance => {
                let level = if m.prone { 0.0 } else { m.smoothed_pose_level.clamp(0.0, 1.0) };
                // Fully crouched still shows a foot of column.
                let height = Val::Percent(if m.prone { 0.0 } else { 12.0 + 88.0 * level });
                if node.height != height {
                    node.height = height;
                }
            }
        }
    }
    // The setting in force is the sprint's pace while a sprint is paid for
    // and the clamped speed otherwise, over the character's top speed; the
    // notches up to the state's limit are the ones that can be set.
    let top = m.settings.max_speed.max(1e-3);
    let set = if m.sprinting { m.character_speed } else { m.clamped_speed() } / top;
    let allowed = m.speed_limit() / top;
    for (notch, mut colour) in &mut notches {
        let at = if notch.0 + 1 == NOTCHES { 1.0 } else { notch.0 as f32 * 0.05 };
        let look = if at <= set + 1e-3 {
            SET
        } else if (at - allowed).round_to(2) <= 0.0 {
            ALLOWED
        } else {
            REFUSED
        };
        if colour.0 != look {
            colour.0 = look;
        }
    }
    if let Ok(mut text) = word.single_mut() {
        let stance = match (m.prone, m.smoothed_pose_level) {
            (true, _) => String::from("lying"),
            (false, level) if level < 0.11 => String::from("crouched"),
            (false, level) if level > 0.95 => String::from("standing"),
            (false, level) => format!("stance {:.0}%", level * 100.0),
        };
        if **text != stance {
            **text = stance;
        }
    }
}

/// Keeps the frame-rate readout: frames a second and the time a frame
/// takes, both over the last second of drawn frames, the longest frame in
/// that second, and what the simulation cost a drawn frame. The times are
/// the engine's own (`Time<Real>`), which is what `--log-pacing` writes as
/// `dt_ms`. Written while the panel at the top right is shown.
pub(super) fn frame_rate(
    real: Res<Time<Real>>,
    controller: Res<Controller>,
    guide: Option<Res<GuideShown>>,
    mut times: ResMut<FrameTimes>,
    mut readout: Query<&mut Text, With<FrameRateText>>,
) {
    let _clock = crate::platform::render_timing::Clock::new("hud frame_rate");
    let Ok(mut text) = readout.single_mut() else { return };
    let shown = guide.as_ref().is_some_and(|guide| guide.0);
    if guide.as_ref().is_some_and(|guide| guide.is_changed()) {
        // Shown again, it is written at once.
        times.since = FRAME_REFRESH;
    }
    // Kept up while hidden too, so that it reads true the moment it shows.
    let took = real.delta_secs();
    if took <= 0.0 {
        return;
    }
    times.frames.push_back((took, controller.paced.cost));
    let mut total: f32 = times.frames.iter().map(|(took, _)| took).sum();
    while total > FRAME_WINDOW && times.frames.len() > 1 {
        total -= times.frames.pop_front().map_or(0.0, |(took, _)| took);
    }
    times.since += took;
    if times.since < FRAME_REFRESH || !shown {
        return;
    }
    times.since = 0.0;
    let count = times.frames.len() as f32;
    let longest = times.frames.iter().map(|(took, _)| *took).fold(0.0, f32::max);
    let simulation = times.frames.iter().map(|(_, cost)| cost).sum::<f32>() / count;
    **text = format!(
        "{:.0} fps   {:.2} ms a frame, longest {:.2}   simulation {:.2} ms",
        count / total,
        total / count * 1000.0,
        longest * 1000.0,
        simulation * 1000.0
    );
}

trait RoundTo {
    fn round_to(self, places: i32) -> f32;
}

impl RoundTo for f32 {
    /// Rounded to so many decimal places, as the game rounds the notch's
    /// place against the limit.
    fn round_to(self, places: i32) -> f32 {
        let scale = 10f32.powi(places);
        (self * scale).round() / scale
    }
}
