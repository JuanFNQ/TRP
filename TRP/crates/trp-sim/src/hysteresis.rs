//! A value carried between 0 and 1 by two curves, one for each way
//! (`HysteresisFilter`; body animator spec, "A worn thing's parts and a
//! helmet's hinge"): a helmet's face shield going down and coming up.

use trp_data::curve::Curve;

/// The filter: which way it is going, its time along its curves and the
/// value it last gave.
#[derive(Debug, Clone, PartialEq)]
pub struct Hysteresis {
    forward: Curve,
    backward: Curve,
    on: bool,
    t: f32,
    target: f32,
    value: f32,
}

impl Hysteresis {
    /// A filter of those curves that has given nothing yet.
    pub fn new(forward: Curve, backward: Curve) -> Self {
        Self { forward, backward, on: false, t: 0.0, target: 0.0, value: 0.0 }
    }

    /// The value it last gave.
    pub fn value(&self) -> f32 {
        self.value
    }

    /// The forward curve's length (`GetDuration`: its last key's time).
    fn duration(&self) -> f32 {
        self.forward.keys.last().map_or(0.0, |key| key.time)
    }

    /// Sets the way it goes (`Set`). At once (`initial`), its time is put
    /// at the forward curve's end going on or at its start going off and
    /// its value is that curve's there, given back; else it is to move
    /// there over the frames to come (`update`).
    pub fn set(&mut self, on: bool, initial: bool) -> Option<f32> {
        self.on = on;
        let end = if on { self.duration() } else { 0.0 };
        if initial {
            self.t = end;
            self.target = end;
            self.value = if on { self.forward.evaluate(self.t) } else { self.backward.evaluate(self.t) };
            return Some(self.value);
        }
        self.target = end;
        None
    }

    /// A frame (`Update`): its time moves toward where it is going at a
    /// second a second, and the curve of its way there gives a value,
    /// which is taken (and given back) only where it is further that way
    /// than the last: going on, higher; going off, lower.
    pub fn update(&mut self, dt: f32) -> Option<f32> {
        if approximately(self.t, self.target) {
            return None;
        }
        self.t = move_towards(self.t, self.target, dt);
        let value = if self.on { self.forward.evaluate(self.t) } else { self.backward.evaluate(self.t) };
        if (self.on && self.value < value) || (!self.on && self.value > value) {
            self.value = value;
            return Some(value);
        }
        None
    }
}

/// The engine's `Mathf.Approximately`: within a millionth of the larger,
/// or eight of the smallest float.
fn approximately(a: f32, b: f32) -> bool {
    (b - a).abs() < (1e-6 * a.abs().max(b.abs())).max(f32::from_bits(1) * 8.0)
}

/// The engine's `Mathf.MoveTowards`.
fn move_towards(current: f32, target: f32, step: f32) -> f32 {
    if (target - current).abs() <= step {
        target
    } else {
        current + (target - current).signum() * step
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use trp_data::curve::Key;

    /// The ZSh-1-2M's own curves (its bundle's `HysteresisFilter`): down
    /// after 0.6 s over 0.4 s; up at the same lag, back the other way.
    fn zsh() -> Hysteresis {
        let key = |time, value, in_slope, out_slope| Key { time, value, in_slope, out_slope };
        let forward = Curve { keys: vec![key(0.0, 0.0, f32::INFINITY, f32::INFINITY), key(0.6, 0.0, 0.0, 2.5), key(1.0, 1.0, 2.5, f32::INFINITY)] };
        let backward = Curve { keys: vec![key(0.0, 0.0, 2.163_091_2, 2.163_091_2), key(0.4, 1.0, 2.5, -0.0), key(1.0, 1.0, f32::INFINITY, f32::INFINITY)] };
        Hysteresis::new(forward, backward)
    }

    #[test]
    fn set_at_once_it_is_at_its_end() {
        let mut filter = zsh();
        assert_eq!(filter.set(true, true), Some(1.0));
        assert_eq!(filter.update(0.01), None);
        let mut filter = zsh();
        assert_eq!(filter.set(false, true), Some(0.0));
    }

    #[test]
    fn a_change_waits_then_moves_and_never_back() {
        let mut filter = zsh();
        filter.set(false, true);
        filter.set(true, false);
        let dt = 1.0 / 120.0;
        let mut first = None;
        for frame in 1..=200 {
            if filter.update(dt).is_some() && first.is_none() {
                first = Some(frame);
            }
        }
        // Nothing for 0.6 s (the frame at 0.6 s itself as the sum of the
        // frames' times falls), then up to 1 over 0.4 s.
        assert!(first.is_some_and(|frame| (72..=73).contains(&frame)), "{first:?}");
        // (Its time stops within a millionth of the end, as the engine's
        // `Approximately` has it, short of the curve's last value.)
        assert!(filter.value() > 0.99999 && filter.value() <= 1.0, "{}", filter.value());
        // Back up: 0.6 s of the backward curve's flat 1, then down.
        filter.set(false, false);
        let mut values = Vec::new();
        for _ in 0..200 {
            values.extend(filter.update(dt));
        }
        assert!(values.windows(2).all(|pair| pair[1] < pair[0]));
        assert_eq!(filter.value(), 0.0);
        // Turned back halfway down, it keeps its value until the forward
        // curve passes it.
        filter.set(true, false);
        for _ in 0..96 {
            filter.update(dt);
        }
        let held = filter.value();
        filter.set(false, false);
        assert_eq!(filter.update(dt), None);
        assert_eq!(filter.value(), held);
    }
}
