//! Runs the game's frame loop at a frame rate of its own, whatever rate the
//! host draws at.
//!
//! The game moves the player and runs the weapon's logic once per drawn
//! frame, steps the weapon's springs once per physics step, and times its
//! aiming camera by a mixture of the two. Several of those rules work per
//! frame, so in the game the result changes with the frame rate: the
//! aiming camera is half as fast again at 120 frames a second as at 60, a
//! stop runs down twice as fast. A host must not inherit that from its own
//! frame rate. The [`Pacer`] gives the simulation frames of a fixed length
//! instead, and says how to draw between their results.
//!
//! What is drawn is taken from the frames in which a physics step ran, and
//! blended from one such frame to the next. The springs change only there,
//! and the camera's uneven steps even out over a physics step, so the
//! blend is smooth at any rate of drawing. It runs one physics step behind
//! the simulation. The view's own turning is not part of it: a host shows
//! that at once.

/// The frame rate the game was recorded and compared at.
pub const REFERENCE_FRAME_RATE: f32 = 120.0;

#[derive(Debug, Clone, Copy)]
pub struct Pacer {
    /// Seconds in one frame of the simulation and in one physics step,
    /// and the most that one frame of the host may count for.
    frame: f64,
    physics: f64,
    longest: f64,
    /// The host's time and the simulation's, in seconds.
    host: f64,
    /// Frames of the simulation run so far.
    frames: u64,
    /// Physics steps run so far.
    steps: u64,
    /// The most by which a physics step's frame has ended after the step
    /// itself was due: nothing where the two rates divide evenly.
    late: f64,
}

/// One frame of the simulation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frame {
    /// Its length in seconds.
    pub dt: f32,
    /// The physics steps that fall in it. What it leaves is drawn from
    /// if there is at least one.
    pub physics_steps: u32,
    /// How much of what the host has gathered since the simulation's last
    /// frame belongs to this one, 0 to 1: the mouse's travel is the case.
    /// The host's frames and the simulation's do not line up, so what has
    /// been gathered when a frame comes due covers anything from one of
    /// the simulation's frames to two. Given whole to the frame, the view
    /// would turn by uneven amounts on frames of even length, and whatever
    /// follows the view with a lag (the body in a jump, and the weapon it
    /// carries there) would shake against it. The frame takes the share
    /// that is its own time's; the rest stays with the host for the next.
    pub input_share: f32,
}

impl Pacer {
    /// `frame_rate` frames of the simulation a second, with the engine's
    /// physics step and the longest frame it allows.
    pub fn new(frame_rate: f32, physics_step: f32, longest_frame: f32) -> Self {
        Self {
            frame: 1.0 / f64::from(frame_rate.max(1.0)),
            // The engine stores its step as a single-precision figure; where
            // that is a whole number of steps a second, the whole number is meant.
            physics: {
                let rate = 1.0 / f64::from(physics_step);
                if (rate - rate.round()).abs() < 1e-3 { 1.0 / rate.round() } else { f64::from(physics_step) }
            },
            longest: f64::from(longest_frame),
            host: 0.0,
            frames: 0,
            steps: 0,
            late: 0.0,
        }
    }

    /// Seconds in one frame of the simulation.
    pub fn frame_length(&self) -> f32 {
        self.frame as f32
    }

    /// Seconds in one physics step.
    pub fn physics_step(&self) -> f32 {
        self.physics as f32
    }

    /// Seconds simulated so far.
    pub fn simulated(&self) -> f32 {
        (self.frames as f64 * self.frame) as f32
    }

    /// The host has drawn a frame `dt` seconds after the last. Returns how
    /// many frames of the simulation are now due; call [`Pacer::frame`]
    /// for each.
    pub fn pass(&mut self, dt: f32) -> u32 {
        self.host += f64::from(dt.max(0.0)).min(self.longest);
        (self.host / self.frame + 1e-9 - self.frames as f64).floor().max(0.0) as u32
    }

    /// Begins the next frame of the simulation.
    pub fn frame(&mut self) -> Frame {
        // What the host holds was gathered from the end of the last frame
        // to now; this frame's own length of that is its share.
        let gathered = self.host - self.frames as f64 * self.frame;
        let input_share = if gathered > self.frame { (self.frame / gathered) as f32 } else { 1.0 };
        self.frames += 1;
        let simulated = self.frames as f64 * self.frame;
        let due = (simulated / self.physics + 1e-9).floor() as u64;
        let physics_steps = (due - self.steps) as u32;
        self.steps = due;
        if physics_steps > 0 {
            self.late = self.late.max(simulated - due as f64 * self.physics);
        }
        Frame { dt: self.frame as f32, physics_steps, input_share }
    }

    /// How far to draw from the frame before last that had a physics step
    /// toward the last one, 0 to 1.
    pub fn blend(&self) -> f32 {
        let since = self.host - self.late - self.steps as f64 * self.physics;
        (since / self.physics).clamp(0.0, 1.0) as f32
    }

    /// The host's time as counted here, in seconds: its frames' lengths,
    /// each cut to the longest allowed.
    pub fn host_time(&self) -> f32 {
        self.host as f32
    }

    /// The moment drawn, in the simulation's own seconds: one physics step
    /// behind the host's time.
    pub fn drawn_time(&self) -> f32 {
        (self.host - self.late - self.physics) as f32
    }

    /// How many frames of the simulation before its last one the moment
    /// drawn lies. What is drawn runs one physics step behind the host's
    /// time, and the simulation's last frame up to a frame behind that
    /// time, so this is between one frame less than a physics step and a
    /// whole one (1 to 2 at 120 frames and 60 steps a second). What
    /// changes on every frame, not only with the physics steps (the
    /// player's place, the body's pose), is drawn from the two frames
    /// either side of that moment; taken from frames a physics step apart
    /// it would be drawn off its path on the frames between.
    pub fn frames_behind(&self) -> f32 {
        let drawn = self.host - self.late - self.physics;
        ((self.frames as f64 * self.frame - drawn) / self.frame).max(0.0) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// At any rate of drawing the simulation gets the same frames, and
    /// the blend runs evenly from 0 to 1 over each physics step.
    #[test]
    fn frames_do_not_depend_on_the_host() {
        for host_rate in [30.0f32, 60.0, 97.0, 144.0, 360.0] {
            let mut pacer = Pacer::new(120.0, 1.0 / 60.0, 1.0 / 12.0);
            let (mut frames, mut steps, mut last_blend, mut last_steps) = (0u32, 0u32, 0.0f32, 0u32);
            for _ in 0..(host_rate as usize * 2) {
                for _ in 0..pacer.pass(1.0 / host_rate) {
                    let frame = pacer.frame();
                    assert!((frame.dt - 1.0 / 120.0).abs() < 1e-7);
                    frames += 1;
                    steps += frame.physics_steps;
                }
                let blend = pacer.blend();
                // Within one physics step the blend only goes forward.
                if steps == last_steps {
                    assert!(blend >= last_blend - 1e-5, "{host_rate}: {last_blend} then {blend}");
                }
                (last_blend, last_steps) = (blend, steps);
            }
            // Two seconds: 240 frames and 120 physics steps, give or take the last.
            assert!((239..=240).contains(&frames), "{host_rate}: {frames} frames");
            assert!((119..=120).contains(&steps), "{host_rate}: {steps} steps");
        }
    }

    /// A mouse turning at a steady rate turns the view by the same amount
    /// on every frame of the simulation, whatever the rate of drawing, and
    /// none of its travel is lost. Given whole to the first frame due, the
    /// frames get uneven amounts wherever the two rates do not divide.
    #[test]
    fn steady_input_reaches_the_frames_evenly() {
        for host_rate in [60.0f32, 97.0, 144.0, 165.0, 240.0, 360.0] {
            let mut pacer = Pacer::new(120.0, 1.0 / 60.0, 1.0 / 12.0);
            // A degree a second of host time, so a frame's fair share is 1/120.
            let (mut held, mut held_whole) = (0.0f64, 0.0f64);
            let (mut shared, mut whole) = (Vec::new(), Vec::new());
            for _ in 0..(host_rate as usize * 2) {
                held += 1.0 / f64::from(host_rate);
                held_whole += 1.0 / f64::from(host_rate);
                for _ in 0..pacer.pass(1.0 / host_rate) {
                    let frame = pacer.frame();
                    let taken = held * f64::from(frame.input_share);
                    held -= taken;
                    shared.push(taken);
                    whole.push(std::mem::take(&mut held_whole));
                }
            }
            let spread = |turns: &[f64]| {
                let turns = &turns[2..];
                let (least, most) = turns.iter().fold((f64::MAX, f64::MIN), |(a, b), t| (a.min(*t), b.max(*t)));
                (most - least) * 120.0
            };
            println!(
                "drawn at {host_rate} a second: a frame's turn varies by {:.1}% of itself shared by time, {:.0}% given whole",
                100.0 * spread(&shared),
                100.0 * spread(&whole)
            );
            assert!(spread(&shared) < 0.01, "{host_rate}: {}", spread(&shared));
            let total: f64 = shared.iter().sum::<f64>() + held;
            assert!((total - 2.0).abs() < 1e-6, "{host_rate}: {total}");
        }
    }

    /// What is drawn follows the host's clock, one physics step behind,
    /// however unevenly the host's frames come: something the simulation
    /// moves evenly is drawn where the host's own time puts it. So a host
    /// whose frames come in clumps draws unevenly only in so far as its
    /// frames are shown at other times than its clock says.
    #[test]
    fn what_is_drawn_follows_the_hosts_time() {
        // Even frames at three rates, and frames in clumps of four to
        // 24.2 ms, as a window presented through Vulkan was measured to
        // get them at 165 a second.
        let patterns: [&[f32]; 4] = [&[1.0 / 60.0], &[1.0 / 144.0], &[1.0 / 165.0], &[0.0186, 0.0017, 0.0010, 0.0029]];
        for pattern in patterns {
            let mut pacer = Pacer::new(120.0, 1.0 / 60.0, 1.0 / 12.0);
            // The simulation moves a point one unit each frame.
            let (mut at, mut before, mut now, mut host) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
            let mut worst = 0.0f64;
            for i in 0..4000 {
                let dt = pattern[i % pattern.len()];
                host += f64::from(dt);
                for _ in 0..pacer.pass(dt) {
                    at += 1.0;
                    if pacer.frame().physics_steps > 0 {
                        (before, now) = (now, at);
                    }
                }
                let drawn = before + (now - before) * f64::from(pacer.blend());
                // 120 units a second, a sixtieth of a second ago.
                let expected = (host - 1.0 / 60.0) * 120.0;
                if i > 40 {
                    worst = worst.max((drawn - expected).abs());
                }
            }
            println!("host frames of {pattern:?} s: what is drawn is within {worst:.5} of a frame's travel of the host's time");
            assert!(worst < 0.01, "{pattern:?}: {worst}");
        }
    }

    /// What changes on every frame is drawn from the two frames either
    /// side of the moment drawn, and that moment is the one the blend
    /// between physics steps shows: the host's time, a physics step ago.
    /// Something that speeds up is then drawn nearer its path than from
    /// frames a physics step apart.
    #[test]
    fn every_frame_is_drawn_at_the_same_moment() {
        let patterns: [&[f32]; 4] = [&[1.0 / 60.0], &[1.0 / 144.0], &[1.0 / 165.0], &[0.0186, 0.0017, 0.0010, 0.0029]];
        for pattern in patterns {
            let mut pacer = Pacer::new(120.0, 1.0 / 60.0, 1.0 / 12.0);
            // The simulation's own count of frames, and a point that
            // speeds up: where it is after n frames is n squared.
            let mut frames: Vec<f64> = vec![0.0];
            let (mut before, mut now, mut host) = (0.0f64, 0.0f64, 0.0f64);
            let (mut worst_time, mut worst_near, mut worst_apart) = (0.0f64, 0.0f64, 0.0f64);
            for i in 0..4000 {
                let dt = pattern[i % pattern.len()];
                host += f64::from(dt);
                for _ in 0..pacer.pass(dt) {
                    let n = frames.len() as f64;
                    frames.push(n);
                    if pacer.frame().physics_steps > 0 {
                        (before, now) = (now, n);
                    }
                }
                if i <= 40 {
                    continue;
                }
                let behind = f64::from(pacer.frames_behind());
                assert!(behind > 0.99 && behind < 2.01, "{pattern:?}: {behind} frames behind");
                let last = frames.len() - 1;
                let (newer, older) = (frames[last - behind.floor() as usize], frames[last - behind.ceil() as usize]);
                let at = older + (newer - older) * (1.0 - behind.fract());
                // The frame's number drawn, by the frames and by the steps.
                let expected = (host - 1.0 / 60.0) * 120.0;
                let by_steps = before + (now - before) * f64::from(pacer.blend());
                worst_time = worst_time.max((at - expected).abs()).max((by_steps - expected).abs());
                // The point, drawn between neighbouring frames and
                // between frames a physics step apart.
                let near = older * older + (newer * newer - older * older) * (1.0 - behind.fract());
                let apart = before * before + (now * now - before * before) * f64::from(pacer.blend());
                worst_near = worst_near.max((near - expected * expected).abs());
                worst_apart = worst_apart.max((apart - expected * expected).abs());
            }
            println!(
                "host frames of {pattern:?} s: the moment drawn within {worst_time:.5} of a frame; a point speeding up is drawn within {worst_near:.3} of its path between neighbouring frames, {worst_apart:.3} between frames a physics step apart"
            );
            assert!(worst_time < 0.01, "{pattern:?}: {worst_time}");
            // A quarter of a frame's change of pace at the worst, against a
            // whole one (a host drawing at the physics rate lands on the
            // frames themselves and sees neither).
            assert!(worst_near < 0.26 && worst_near <= worst_apart + 1e-3, "{pattern:?}: {worst_near} {worst_apart}");
        }
    }

    /// A physics step falls in every other frame at 120 frames a second,
    /// and a long frame of the host is cut to the engine's limit.
    #[test]
    fn physics_steps_and_the_longest_frame() {
        let mut pacer = Pacer::new(120.0, 1.0 / 60.0, 1.0 / 12.0);
        assert_eq!(pacer.pass(1.0), 10);
        let steps: Vec<u32> = (0..10).map(|_| pacer.frame().physics_steps).collect();
        assert_eq!(steps, [0, 1, 0, 1, 0, 1, 0, 1, 0, 1]);
    }
}
