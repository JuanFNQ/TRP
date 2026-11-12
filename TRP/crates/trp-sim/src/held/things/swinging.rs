//! Working a thing, its state changes and the swing stepped on the clock.

use super::*;

impl Thing {
    /// The character works it (`MovementState.ExecuteDoorInteraction`):
    /// the gesture for its state and the character's place is worked out,
    /// it is held for the work (`LockForInteraction`) and told what to do
    /// (`Interact`). Returns the gesture to ask of the hand.
    pub fn work(&mut self, open: bool, from: [f32; 3], settings: &ThingSettings) -> Option<Interaction> {
        let gesture = self.gesture(from);
        if self.state != ThingState::Interacting {
            self.fallback = self.state;
            self.state = ThingState::Interacting;
        }
        self.tell(open, settings);
        gesture
    }

    /// Told to open or to shut with no hand (`Interact`), as a container is
    /// when its screen shuts, or when it was searched and the character
    /// went off before it was shown. A door does it at once and waits in
    /// its own routine; a container and a trunk wait first.
    pub fn tell(&mut self, open: bool, settings: &ThingSettings) {
        let to = if open { ThingState::Open } else { ThingState::Shut };
        match self.kind {
            Kind::Door => self.set_state(to, settings),
            Kind::Container | Kind::Trunk => self.timers.push(Timer { waited: 0.0, fresh: true, to }),
        }
    }

    /// Puts it on its way to a new state, if the game takes the order
    /// (`SetDoorState`, `CanStartInteraction`): not to the state it is in,
    /// nor to the one it is already on its way to, nor to the one its last
    /// swing ended in until a swing's length has passed since. Refused, a
    /// thing held for the work and not swinging goes back to what it was.
    fn set_state(&mut self, to: ThingState, settings: &ThingSettings) {
        let duration = settings.duration();
        let recent = self.previous.is_some_and(|(was, ended)| was == to && ended + self.record[1] >= self.clock);
        let swinging = self.record[0] < self.record[1];
        if self.state == to || self.result == Some(to) || recent {
            if !swinging && self.state == ThingState::Interacting {
                self.state = self.fallback;
            }
            return;
        }
        (self.result, self.breaching) = (Some(to), false);
        self.record = [0.0, duration];
        let target = if to == ThingState::Open { self.open_angle } else { self.shut_angle };
        let mut swing = Swing {
            wait: None,
            from: self.angle,
            turn: delta_angle(self.angle, target),
            opening: to == ThingState::Open,
            progress: 0.0,
            pace: CONTAINER_PACE,
            to,
            kicked: false,
            begun: false,
            shut_played: false,
        };
        match self.kind {
            Kind::Door => {
                swing.wait = Some((0.0, DOOR_WAITS, true));
                swing.pace = 1.0;
                self.swings.push(swing);
            }
            // The routine begins at once and runs its first frame now.
            Kind::Container | Kind::Trunk => {
                self.swings.push(swing);
            }
        }
    }

    /// One frame of `dt` seconds. The routines run after the character's
    /// own update in the game's frame, so a host steps a thing after the
    /// character.
    pub fn step(&mut self, dt: f32, settings: &ThingSettings) {
        self.clock += dt;
        let duration = settings.duration();
        let mut index = 0;
        while index < self.swings.len() {
            let mut swing = self.swings[index];
            if let Some((waited, length, fresh)) = swing.wait {
                let waited = if fresh { waited } else { waited + dt };
                if waited + 1e-6 < length {
                    self.swings[index].wait = Some((waited, length, false));
                    index += 1;
                    continue;
                }
                swing.wait = None;
            }
            if swing.progress >= if swing.kicked { KICK_LASTS } else { duration } {
                self.finish(swing);
                self.swings.remove(index);
                continue;
            }
            self.run_frame(&mut swing, dt, settings);
            self.swings[index] = swing;
            index += 1;
        }
        // A kick that failed: the frame it landed on does not count, and
        // the first frame three tenths of a second on puts the door back,
        // unless a swing is under way.
        if let Some((waited, fresh)) = self.failed_kick {
            let waited = if fresh { waited } else { waited + dt };
            if waited + 1e-6 < FAILED_KICK_SHOWS {
                self.failed_kick = Some((waited, false));
            } else {
                self.failed_kick = None;
                if self.record[0] >= self.record[1] {
                    self.state = self.fallback;
                }
            }
        }
        // A container's or a trunk's wait: the frame it was told on does
        // not count, and the frame it runs out on begins the swing, which
        // runs its first frame at once.
        let mut index = 0;
        while index < self.timers.len() {
            let timer = &mut self.timers[index];
            if !std::mem::take(&mut timer.fresh) {
                timer.waited += dt;
            }
            if timer.waited + 1e-6 < settings.answers_after {
                index += 1;
                continue;
            }
            let to = self.timers.remove(index).to;
            let before = self.swings.len();
            self.set_state(to, settings);
            if self.swings.len() > before {
                let mut swing = self.swings[before];
                self.run_frame(&mut swing, dt, settings);
                self.swings[before] = swing;
            }
        }
    }

    /// One frame of a swing past its wait: its first frame sounds the
    /// opening (`PlaySound` as the routine reaches the swing), the thing
    /// stands where the curve has it, the time moves on, and a shutting
    /// sounds its shut clip on the frame the time passes the shift before
    /// the curve's end (`ProgressTime > Duration - ShutShift`, read after
    /// the frame's `UpdateTime`).
    fn run_frame(&mut self, swing: &mut Swing, dt: f32, settings: &ThingSettings) {
        if !swing.begun {
            swing.begun = true;
            self.sounds.push(ThingSound::Swing { opens: swing.opening && !swing.kicked, squeaks: self.fallback != ThingState::Locked });
        }
        self.place(swing, settings);
        swing.progress += dt * swing.pace;
        if self.kind != Kind::Container {
            // The game's own routine keeps the shared record.
            self.record[0] = swing.progress;
        }
        if !swing.opening && !swing.shut_played && swing.progress > settings.duration() - self.shut_shift && self.fallback != ThingState::Locked {
            swing.shut_played = true;
            self.sounds.push(ThingSound::Shut);
        }
    }

    /// Where the curve has it at the swing's time so far.
    pub(super) fn place(&mut self, swing: &Swing, settings: &ThingSettings) {
        let share = if swing.kicked { kick_share(swing.progress) } else { settings.curve.evaluate(swing.progress) };
        self.angle = swing.from + share * swing.turn;
        self.out = if swing.opening { share } else { 1.0 - share };
    }

    /// The swing is over: the thing is at its new angle and in its new
    /// state (a door's or a trunk's the last order's, a container's its own).
    fn finish(&mut self, swing: Swing) {
        // A shutting that never reached its shift sounds its clip as it ends.
        if !swing.opening && !swing.shut_played && !swing.kicked && self.fallback != ThingState::Locked {
            self.sounds.push(ThingSound::Shut);
        }
        let state = match self.kind {
            Kind::Container => swing.to,
            Kind::Door | Kind::Trunk => self.result.unwrap_or(swing.to),
        };
        self.state = state;
        self.angle = swing.from + swing.turn;
        self.out = if state == ThingState::Open { 1.0 } else { 0.0 };
        if self.kind != Kind::Container {
            // A kick's swing ends as a breach, which is no state an order
            // is ever for: it refuses none after it.
            self.previous = (!swing.kicked).then_some((state, self.clock));
        }
    }

    /// Whether it is waiting or swinging.
    pub fn busy(&self) -> bool {
        !self.timers.is_empty() || !self.swings.is_empty() || self.failed_kick.is_some()
    }
}

/// How far through its turn a kicked door is, so many seconds after the
/// kick: the kick's own curve, from nothing at a slope of one to all of it
/// at a slope of one, half a second long.
fn kick_share(seconds: f32) -> f32 {
    let key = |time, value| trp_data::formats::curve::Key { time, value, in_slope: KICK_SLOPE, out_slope: KICK_SLOPE };
    Curve { keys: vec![key(0.0, 0.0), key(KICK_LASTS, 1.0)] }.evaluate(seconds)
}

/// A vector's part along the ground, at length one (nothing for none).
pub(super) fn flat_unit(v: [f32; 3]) -> [f32; 2] {
    let size = (v[0] * v[0] + v[2] * v[2]).sqrt();
    if size < f32::EPSILON { [0.0; 2] } else { [v[0] / size, v[2] / size] }
}

/// The shortest turn from one angle to another, in degrees, from -180 to
/// 180 (`Mathf.DeltaAngle`).
pub fn delta_angle(from: f32, to: f32) -> f32 {
    let mut d = (to - from).rem_euclid(360.0);
    if d > 180.0 {
        d -= 360.0;
    }
    d
}

/// `v` turned by `degrees` about the unit vector `axis`.
pub fn turn(v: [f32; 3], axis: [f32; 3], degrees: f32) -> [f32; 3] {
    let (sin, cos) = degrees.to_radians().sin_cos();
    let along = dot(v, axis);
    let across = cross(axis, v);
    std::array::from_fn(|k| v[k] * cos + across[k] * sin + axis[k] * along * (1.0 - cos))
}

pub(super) fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|k| a[k] - b[k])
}

pub(super) fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub(super) fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

/// `v` along `onto` (`Vector3.Project`: nothing onto nothing).
pub(super) fn project(v: [f32; 3], onto: [f32; 3]) -> [f32; 3] {
    let size = dot(onto, onto);
    if size < f32::EPSILON {
        return [0.0; 3];
    }
    let share = dot(v, onto) / size;
    onto.map(|o| o * share)
}
