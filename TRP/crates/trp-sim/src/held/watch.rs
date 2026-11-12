//! The watch on the first-person hands' wrist: the time it shows and its three hands' turns.
//! After the body animator spec, "The watch on the wrist"
//! (`Watch.GetCurrentLocationTime`, `Watch.Update`).

use trp_data::formats::curve::Curve;

/// Seconds in a day.
const DAY: f64 = 86_400.0;

/// The time of day the watch shows, in seconds from midnight, on a clock
/// that stood at `start` (seconds from midnight) `elapsed_ms` whole
/// milliseconds ago (the game's `DateTimeExtensions.Now`: the time as the
/// run began plus a stopwatch's whole milliseconds). The watch runs on it
/// as `Watch.Init` sets it up in a raid with the raid's clock at 1 to 1
/// and no difference: the time passed is taken in 100 ns ticks through a
/// single-precision float (`_acceleration` 1), as the game takes it.
pub fn time_of_day(start: f64, elapsed_ms: u64) -> f64 {
    let ticks = elapsed_ms as i64 * 10_000;
    let passed = (ticks as f32) as i64;
    (start + passed as f64 / 10_000_000.0).rem_euclid(DAY)
}

/// The hour, minute and second hands' turns at a time of day (seconds from
/// midnight), in degrees about each hand's own z axis, which the game
/// sets as each hand's whole local turn (`Watch.Update`): the hour hand
/// 30 a hour of the day's (0 to 720), the minute hand 6 a minute of the
/// hour's, the second hand 6 a whole second of the minute's plus 6 times
/// the tick at the share of the second gone (the tick goes from 0 to 1,
/// with its jump and bounce near the second's end).
pub fn hands(seconds_of_day: f64, tick: &Curve) -> [f32; 3] {
    let total_hours = seconds_of_day / 3600.0;
    let total_minutes = seconds_of_day / 60.0;
    let in_minute = seconds_of_day % 60.0;
    let whole = in_minute as i32;
    let share = (in_minute - whole as f64) as f32;
    [total_hours as f32 * 30.0, total_minutes as f32 % 60.0 * 6.0, (whole as f32 + tick.evaluate(share)) * 6.0]
}

#[cfg(test)]
mod tests {
    use super::*;
    use trp_data::formats::curve::Key;

    /// The BEAR watch's tick (`bear_watch.bundle`, `Watch._curve`: `probe
    /// dump assets/content/hands/bear/bear_watch.bundle
    /// -550532836591993309 8 40`).
    fn bear_tick() -> Curve {
        let key = |time: f32, value: f32, in_slope: f32, out_slope: f32| Key { time, value, in_slope, out_slope };
        Curve {
            keys: vec![
                key(0.0, 0.0, -0.002_751_193, -0.002_751_193),
                key(0.851_106_17, -0.002_341_557_3, -0.002_751_193, 38.387_253),
                key(0.871_228_6, 0.770_104_7, 38.387_253, 1.202_458_4),
                key(0.901_080_07, 0.805_999_8, 1.202_458_4, 18.474_604),
                key(0.927_796_5, 1.299_576, 18.474_604, -20.763_084),
                key(0.954_802_6, 0.738_847_55, -20.763_084, 42.930_61),
                key(0.965_741_8, 1.208_475_8, 42.930_61, -15.955_746),
                key(0.988_831_1, 0.840_069, -15.955_746, 14.319_32),
                key(1.0, 1.0, 14.319_32, 14.319_32),
            ],
        }
    }

    #[test]
    fn hands_at_ten_past_ten() {
        // 10:08:30.0: the second hand on the half minute, the tick at 0.
        let [hour, minute, second] = hands(10.0 * 3600.0 + 8.0 * 60.0 + 30.0, &bear_tick());
        assert!((hour - 304.25).abs() < 1e-3, "{hour}");
        assert!((minute - 51.0).abs() < 1e-3, "{minute}");
        assert!((second - 180.0).abs() < 1e-3, "{second}");
    }

    #[test]
    fn the_second_hand_waits_then_ticks() {
        let tick = bear_tick();
        // Most of a second it stands (the tick's first segment stays within
        // a hundredth of a degree), then jumps on, past the next second
        // and back, and lands on it.
        let at = |share: f64| hands(42.0 + share, &tick)[2];
        assert!((at(0.5) - 252.0).abs() < 0.02, "{}", at(0.5));
        assert!(at(0.9278) > 252.0 + 6.0 * 1.29, "{}", at(0.9278));
        assert!((at(0.999_999) - 258.0).abs() < 0.01, "{}", at(0.999_999));
        assert!((hands(43.0, &tick)[2] - 258.0).abs() < 1e-3);
    }

    #[test]
    fn the_hour_hand_goes_round_twice_a_day() {
        let tick = bear_tick();
        assert!((hands(18.0 * 3600.0, &tick)[0] - 540.0).abs() < 1e-3);
        assert_eq!(hands(0.0, &tick), [0.0, 0.0, 0.0]);
    }

    #[test]
    fn the_clock_runs_from_its_start_and_wraps_at_midnight() {
        assert!((time_of_day(3600.0, 1500) - 3601.5).abs() < 1e-6);
        assert!((time_of_day(DAY - 1.0, 2000) - 1.0).abs() < 1e-6);
        // A day on, the float's step is 6.5 ms at most.
        let later = time_of_day(0.0, 86_399_000);
        assert!((later - 86_399.0).abs() < 0.007, "{later}");
    }
}
