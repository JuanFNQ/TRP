//! A popper of the hideout's shooting range (`docs/spec/range-targets.md`):
//! a steel plate on a hinge and the roller under it, folded back by a hit
//! on its face, stood up again 3 s after, rocked by a hit anywhere else.
//! Each part turns about one axis only, so its turn is an angle (degrees
//! from standing); the game's spherical blend between two turns about the
//! same axis is the angle blended.

use trp_data::curve::{Curve, Key};

/// The plate's and the roller's turns down (`_targetDownRotation`,
/// `_rollerDownRotation`: 86 and 102 degrees about x); up both stand at
/// none.
pub const PLATE_DOWN: f32 = 86.0;
pub const ROLLER_DOWN: f32 = 102.0;
/// The whole move's length (`_duration`).
pub const DURATION: f32 = 0.2;
/// The wait from the end of a fold to the unfold (`ResetDelay`).
pub const RESET_DELAY: f32 = 3.0;
/// A rock: so far past standing, so long each way (`Rocking`).
pub const ROCK_ANGLE: f32 = 10.0;
pub const ROCK_TIME: f32 = 0.05;
/// The face's radius a hit on the target lands within (`_sizes`).
pub const FACE_RADIUS: f32 = 0.155;

fn curve(keys: &[(f32, f32, f32)]) -> Curve {
    Curve { keys: keys.iter().map(|&(time, value, slope)| Key { time, value, in_slope: slope, out_slope: slope }).collect() }
}

/// The plate's curve going down (`_targetDownEase`).
pub fn plate_down() -> Curve {
    curve(&[(0.0, 0.0, 2.0), (0.571_075_9, 0.855_992_85, 0.718_145_85), (1.0, 1.0, 0.0)])
}
/// The plate's going up (`_targetUpEase`).
pub fn plate_up() -> Curve {
    curve(&[(0.0, 0.0, 1.131_709_5), (0.850_461_6, 0.962_475_5, 1.198_737_7), (1.0, 1.0, 0.250_935_7)])
}
/// The roller's going down (`_rollerDownEase`).
pub fn roller_down() -> Curve {
    curve(&[(0.0, 0.0, 2.0), (1.0, 1.0, 0.0)])
}
/// The roller's going up (`_rollerUpEase`): straight.
pub fn roller_up() -> Curve {
    curve(&[(0.0, 0.0, 1.0), (1.0, 1.0, 1.0)])
}

/// How a popper stands (`FoldingTargetState`): moving, or done folding or
/// unfolding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Moving,
    Folded,
    Unfolded,
}

/// A sound a popper plays (`FoldingPopperTarget.PlayAudio`): a fold's (which
/// of the two the draw gave), an unfold's, a rock's, each at its volume.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Sound {
    Fold { second: bool, volume: f32 },
    Unfold { volume: f32 },
    Rock,
}

/// What a move's sequence does: a fold, an unfold, a rock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Fold,
    Unfold,
    Rock,
}

/// One part's tween: from, to, how long, by which curve (none: straight).
#[derive(Debug, Clone)]
struct Tween {
    from: f32,
    to: f32,
    length: f32,
    ease: Option<Curve>,
}

impl Tween {
    /// Its angle `at` into it.
    fn at(&self, at: f32) -> f32 {
        if self.length <= 0.0 || at >= self.length {
            return self.to;
        }
        let share = at / self.length;
        let eased = match &self.ease {
            // DOTween's `EaseCurve`: the curve read at the share times its
            // last key's time.
            Some(curve) => curve.evaluate(share * curve.keys.last().map_or(1.0, |k| k.time)),
            None => share,
        };
        // `Quaternion.Slerp` clamps its blend.
        self.from + (self.to - self.from) * eased.clamp(0.0, 1.0)
    }
}

/// A sequence of steps, each the two parts' tweens together; ended (killed)
/// by the next.
#[derive(Debug, Clone)]
struct Sequence {
    id: u64,
    kind: Kind,
    steps: Vec<[Tween; 2]>,
    step: usize,
    time: f32,
}

/// What waits on what: a fold's end, then the reset's wait, then the
/// unfold (the default scenario's chain for one hit).
#[derive(Debug, Clone, Copy, PartialEq)]
enum Chain {
    Folding(u64),
    Waiting(f32),
}

/// One popper.
#[derive(Debug, Clone)]
pub struct Popper {
    /// The plate's and the roller's turns from standing, degrees.
    pub plate: f32,
    pub roller: f32,
    pub state: State,
    sequence: Option<Sequence>,
    next_id: u64,
    chains: Vec<Chain>,
    /// What it has played since the host last took them.
    pub sounds: Vec<Sound>,
}

impl Default for Popper {
    fn default() -> Self {
        Self { plate: 0.0, roller: 0.0, state: State::Unfolded, sequence: None, next_id: 1, chains: Vec::new(), sounds: Vec::new() }
    }
}

/// `Quaternion.Angle` between two turns about one axis: the angle between
/// them, nought where the turns' dot is past 0.999999 (`IsEqualUsingDot`).
fn angle_between(a: f32, b: f32) -> f32 {
    let difference = (a - b).abs();
    if (difference.to_radians() / 2.0).cos() > 0.999_999 {
        0.0
    } else {
        difference
    }
}

impl Popper {
    /// A report of its face's collider (`segment` 0) or of a miss: the
    /// default scenario's answer (`PopperTargetStandScenario.HandleHitTarget`).
    /// `draw` is the fold sound's draw (`Random.Range(0, 2)`, 0 or 1).
    pub fn hit(&mut self, on_target: bool, draw: u32) {
        if on_target {
            let id = self.fold(draw);
            self.chains.push(Chain::Folding(id));
        } else {
            self.rock();
        }
    }

    /// Starts a sequence, ending the one under way where it stands, and the
    /// chains waiting on it with it.
    fn start(&mut self, kind: Kind, steps: Vec<[Tween; 2]>) -> u64 {
        if let Some(old) = self.sequence.take() {
            self.chains.retain(|chain| *chain != Chain::Folding(old.id));
        }
        let id = self.next_id;
        self.next_id += 1;
        if kind != Kind::Rock {
            self.state = State::Moving;
        }
        self.sequence = Some(Sequence { id, kind, steps, step: 0, time: 0.0 });
        id
    }

    /// A fold from where it stands (`Fold`): each part over its share left
    /// of the whole move, the fold's sound at the plate's share.
    fn fold(&mut self, draw: u32) -> u64 {
        let plate_share = angle_between(self.plate, PLATE_DOWN) / PLATE_DOWN;
        let roller_share = angle_between(self.roller, ROLLER_DOWN) / ROLLER_DOWN;
        self.sounds.push(Sound::Fold { second: draw % 2 == 1, volume: plate_share });
        let tweens = [
            Tween { from: self.plate, to: PLATE_DOWN, length: plate_share * DURATION, ease: Some(plate_down()) },
            Tween { from: self.roller, to: ROLLER_DOWN, length: roller_share * DURATION, ease: Some(roller_down()) },
        ];
        self.start(Kind::Fold, vec![tweens])
    }

    /// An unfold from where it stands (`Unfold`).
    fn unfold(&mut self) -> u64 {
        let plate_share = angle_between(self.plate, 0.0) / PLATE_DOWN;
        let roller_share = angle_between(self.roller, 0.0) / ROLLER_DOWN;
        self.sounds.push(Sound::Unfold { volume: plate_share });
        let tweens = [
            Tween { from: self.plate, to: 0.0, length: plate_share * DURATION, ease: Some(plate_up()) },
            Tween { from: self.roller, to: 0.0, length: roller_share * DURATION, ease: Some(roller_up()) },
        ];
        self.start(Kind::Unfold, vec![tweens])
    }

    /// A rock (`Rocking`): past standing and back, straight in time.
    fn rock(&mut self) -> u64 {
        self.sounds.push(Sound::Rock);
        let out = [
            Tween { from: self.plate, to: ROCK_ANGLE, length: ROCK_TIME, ease: None },
            Tween { from: self.roller, to: ROCK_ANGLE, length: ROCK_TIME, ease: None },
        ];
        let back = [
            Tween { from: ROCK_ANGLE, to: 0.0, length: ROCK_TIME, ease: None },
            Tween { from: ROCK_ANGLE, to: 0.0, length: ROCK_TIME, ease: None },
        ];
        self.start(Kind::Rock, vec![out, back])
    }

    /// The popper `dt` on: its sequence moved on (a step ended moves on to
    /// the next in the same step), its waits counted, an unfold begun at a
    /// wait's end.
    pub fn step(&mut self, dt: f32) {
        self.run(dt);
        let mut ready = 0;
        for chain in &mut self.chains {
            if let Chain::Waiting(left) = chain {
                *left -= dt;
                if *left <= 0.0 {
                    ready += 1;
                }
            }
        }
        self.chains.retain(|chain| !matches!(chain, Chain::Waiting(left) if *left <= 0.0));
        for _ in 0..ready {
            self.unfold();
            // A move of nothing (from standing) ends at once.
            self.run(0.0);
        }
    }

    fn run(&mut self, dt: f32) {
        let Some(sequence) = self.sequence.as_mut() else { return };
        sequence.time += dt;
        loop {
            let tweens = &sequence.steps[sequence.step];
            let length = tweens[0].length.max(tweens[1].length);
            let at = sequence.time.min(length);
            self.plate = tweens[0].at(at);
            self.roller = tweens[1].at(at);
            if sequence.time < length {
                return;
            }
            if sequence.step + 1 < sequence.steps.len() {
                sequence.time -= length;
                sequence.step += 1;
                continue;
            }
            break;
        }
        let (id, kind) = (sequence.id, sequence.kind);
        self.sequence = None;
        match kind {
            Kind::Fold => {
                self.state = State::Folded;
                for chain in &mut self.chains {
                    if *chain == Chain::Folding(id) {
                        *chain = Chain::Waiting(RESET_DELAY);
                    }
                }
            }
            Kind::Unfold => self.state = State::Unfolded,
            Kind::Rock => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(popper: &mut Popper, seconds: f32) {
        let steps = (seconds / 0.001).round() as usize;
        for _ in 0..steps {
            popper.step(0.001);
        }
    }

    /// The curves give their keys' values at their keys.
    #[test]
    fn the_curves_pass_their_keys() {
        for curve in [plate_down(), plate_up(), roller_down(), roller_up()] {
            for key in &curve.keys {
                assert!((curve.evaluate(key.time) - key.value).abs() < 1e-6);
            }
        }
    }

    /// A hit folds it in 0.2 s (both parts down, its note folded), it stays
    /// down 3 s, and it stands up again in 0.2 s more, with the fold's sound
    /// at full and the unfold's.
    #[test]
    fn a_hit_folds_it_waits_and_stands_it_up() {
        let mut p = Popper::default();
        p.hit(true, 1);
        assert_eq!(p.sounds, vec![Sound::Fold { second: true, volume: 1.0 }]);
        run(&mut p, 0.1);
        assert_eq!(p.state, State::Moving);
        assert!(p.plate > 0.0 && p.plate < PLATE_DOWN);
        run(&mut p, 0.101);
        assert_eq!((p.state, p.plate, p.roller), (State::Folded, PLATE_DOWN, ROLLER_DOWN));
        run(&mut p, 2.95);
        assert_eq!(p.state, State::Folded);
        run(&mut p, 0.06);
        assert_eq!(p.state, State::Moving);
        run(&mut p, 0.21);
        assert_eq!((p.state, p.plate, p.roller), (State::Unfolded, 0.0, 0.0));
    }

    /// A miss rocks it 10 degrees and back in 0.1 s, its note unchanged.
    #[test]
    fn a_miss_rocks_it() {
        let mut p = Popper::default();
        p.sounds.clear();
        p.hit(false, 0);
        assert_eq!(p.sounds, vec![Sound::Rock]);
        run(&mut p, 0.05);
        assert!((p.plate - ROCK_ANGLE).abs() < 0.05);
        assert_eq!(p.state, State::Unfolded);
        run(&mut p, 0.051);
        assert_eq!(p.plate, 0.0);
    }

    /// A miss while it falls ends the fold where it stands and rocks it up:
    /// nothing comes of the hit after (no wait, no unfold).
    #[test]
    fn a_miss_mid_fold_stands_it_up_and_ends_the_hit() {
        let mut p = Popper::default();
        p.hit(true, 0);
        run(&mut p, 0.1);
        p.hit(false, 0);
        run(&mut p, 0.2);
        assert_eq!(p.plate, 0.0);
        assert_eq!(p.state, State::Moving);
        run(&mut p, 4.0);
        assert_eq!(p.plate, 0.0);
    }

    /// A second hit mid-fold folds it on from where it stands, over the
    /// share left, and its wait is the second's.
    #[test]
    fn a_second_hit_mid_fold_goes_on_from_where_it_stands() {
        let mut p = Popper::default();
        p.hit(true, 0);
        run(&mut p, 0.05);
        let at = p.plate;
        p.hit(true, 0);
        let Some(Sound::Fold { volume, .. }) = p.sounds.last().copied() else { panic!() };
        assert!((volume - (PLATE_DOWN - at) / PLATE_DOWN).abs() < 1e-4);
        run(&mut p, 0.2);
        assert_eq!(p.state, State::Folded);
        run(&mut p, 2.9);
        assert_eq!(p.state, State::Folded);
        run(&mut p, 0.4);
        assert_eq!((p.state, p.plate), (State::Unfolded, 0.0));
    }

    /// A miss while it lies waiting rocks it up from where it lies; its
    /// unfold still comes at the wait's end, a move of nothing.
    #[test]
    fn a_miss_while_down_rocks_it_up_and_the_wait_still_ends() {
        let mut p = Popper::default();
        p.hit(true, 0);
        run(&mut p, 1.0);
        assert_eq!(p.state, State::Folded);
        p.hit(false, 0);
        run(&mut p, 0.2);
        assert_eq!(p.plate, 0.0);
        assert_eq!(p.state, State::Folded);
        run(&mut p, 2.5);
        assert_eq!((p.state, p.plate), (State::Unfolded, 0.0));
    }
}
