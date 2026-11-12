//! The animator's running state: playing states, blends, events, looks and mixes.

use super::*;

/// A state being played: which one, and how far through it, in cycles.
#[derive(Debug, Clone, Copy)]
pub(super) struct Playing {
    pub(super) state: usize,
    pub(super) time: f32,
    /// Where it was before the last update.
    pub(super) previous: f32,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct Blend {
    pub(super) duration: f32,
    pub(super) fixed: bool,
    pub(super) elapsed: f32,
    pub(super) interruption: i32,
    /// For a blend out of states held still (`Held`): how fast the
    /// current one of them was playing. A length that is a share of the
    /// state is then that share of the state being entered, over this.
    pub(super) pace: Option<f32>,
}

/// What a transition leaves behind when one out of the state it was
/// entering cuts it short: the blend as the update before left it, held
/// still, and the new transition blends out of that
/// (`body-animator.md`, "A transition cut short").
#[derive(Debug, Clone)]
pub(super) struct Held {
    /// Where the current state stood when it was held. (Its own clock is
    /// moved on once more in the update that holds it, which is what the
    /// engine reports of it from then on.)
    pub(super) current_at: f32,
    /// The states held beside the current one, each where it stood, with
    /// its share of the whole. The current one has what is left.
    pub(super) others: Vec<(Playing, f32)>,
    /// How far the blend moved and turned the root in a second in the
    /// update before it was held: it goes on giving that, by its share.
    pub(super) motion: ([f32; 3], f32),
}

#[derive(Debug, Clone)]
pub(super) struct MachineState {
    pub(super) current: Playing,
    pub(super) next: Option<Playing>,
    pub(super) blend: Option<Blend>,
    pub(super) held: Option<Held>,
}

/// A state was entered or left on a layer's machine.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Event {
    /// Fired when a transition into the state starts.
    Entered { machine: usize, state: usize },
    /// Fired when the transition out of the state ends.
    Left { machine: usize, state: usize },
}

/// How an update came by a state it had a look at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LookKind {
    /// The state was entered in this update: by a transition that began,
    /// or outright.
    Enter,
    /// It went on playing.
    Update,
    /// It was left: the transition out of it ended, or it was replaced.
    Exit,
}

/// One state as an update of its machine had it: what the engine tells a
/// state's scripts, which is what the game's marks go by (firing spec,
/// "Marks").
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Look {
    pub machine: usize,
    pub state: usize,
    pub kind: LookKind,
    /// How far the state has played, in cycles.
    pub time: f32,
    pub looping: bool,
    /// A transition is under way on the machine and this is not the state
    /// it leads to.
    pub leaving: bool,
}

/// One clip's share of a layer this frame.
#[derive(Debug, Clone, Copy)]
pub(super) struct Share {
    pub(super) clip: usize,
    pub(super) weight: f32,
    pub(super) previous: f32,
    pub(super) now: f32,
    pub(super) offset: f32,
    /// The state plays at a negative speed: its time runs on as any
    /// state's, and its clip is taken from its end (recorded:
    /// the body's `LS_idle_aim_0 0` at speed -1: its normalized time 0.1
    /// to 0.9 over 0.4 s as its clip's `Leftstance_curve` went 0.9 to 0.1).
    pub(super) reversed: bool,
}

impl Share {
    /// Where in its clip, as a share of the clip's length, the share is
    /// at a time of its state (whole cycles and all): looped or held at
    /// the ends, and taken from the end where the state plays backwards.
    pub(super) fn clip_share(&self, looping: bool, cycles: f32) -> f32 {
        let t = cycles + self.offset;
        let t = if looping { t.rem_euclid(1.0) } else { t.clamp(0.0, 1.0) };
        if self.reversed {
            1.0 - t
        } else {
            t
        }
    }
}

/// One weighted layer of an update's mix: its place, its weight, each
/// clip's share with its times (`Animator::shares`), their sum, and the
/// share of states that leave its bones as they stood (`kept_share`).
#[derive(Debug, Clone)]
pub(super) struct LayerMix {
    pub(super) layer: usize,
    pub(super) weight: f32,
    pub(super) shares: Vec<Share>,
    pub(super) total: f32,
    pub(super) kept: f32,
}

/// What one update made of the pose: each weighted layer's mix, as it stood
/// before the update set the parameters the clips drive; the bones the
/// update worked out (`Controller::always`, in order); and the other bones a
/// layer left as they stood on it (a kept share: `Animator::kept_share`),
/// which carry from one update to the next and so are worked out with it,
/// each by its place. The rest of the pose is a function of it alone
/// (`Controller::pose_of`), whenever it is asked.
#[derive(Debug, Clone)]
pub struct Mix {
    pub(super) layers: Vec<LayerMix>,
    pub(super) always: Vec<Bone>,
    pub(super) standing: Vec<(usize, Bone)>,
    /// Where a pose for drawing was asked for, its mix.
    pub(super) drawn: Option<DrawnMix>,
}

impl Mix {
    /// Whether the update worked out a bone not among those it always does
    /// because a layer left it standing.
    pub(super) fn stood(&self, bone: usize) -> bool {
        self.standing.iter().any(|(at, _)| *at == bone)
    }

    /// How many bones the update worked out because a layer left them
    /// standing, beside those it always works out; and of the pose for
    /// drawing, where there is one.
    pub fn standing(&self) -> (usize, Option<usize>) {
        (self.standing.len(), self.drawn.as_ref().map(|drawn| drawn.standing.len()))
    }
}

/// The pose for drawing's part of a mix: each layer's mix as it stood once
/// the update had set the parameters the clips drive (as that pose always
/// has been worked out, after the pose itself), and the bones a layer left
/// standing on it as the update worked them out.
#[derive(Debug, Clone)]
pub(super) struct DrawnMix {
    pub(super) layers: Vec<LayerMix>,
    pub(super) standing: Vec<(usize, Bone)>,
}

impl DrawnMix {
    pub(super) fn stood(&self, bone: usize) -> bool {
        self.standing.iter().any(|(at, _)| *at == bone)
    }
}

/// What the update's working-out gives beside bones: the parameters the
/// clips' curves drive, the root's travel and turn, the clips' events
/// passed.
pub(super) struct Driven {
    pub(super) curves: Vec<Option<f32>>,
    pub(super) root: [f32; 3],
    pub(super) turn: f32,
    pub(super) passed: Vec<(usize, usize)>,
}
