//! The controller's definitions: bone rests, conditions, transitions, states, layers and clips.

use super::*;

/// A bone the animator keeps the pose of.
#[derive(Debug, Clone)]
pub struct BoneRest {
    /// Path below the animator's object, as clips name it.
    pub path: String,
    pub position: [f32; 3],
    pub rotation: [f32; 4],
    /// Its scale at rest, where the clips' scale of it is to be kept too
    /// (a rig of the hands': an item used is drawn by its bone scaled up
    /// from nought); none where it is not.
    pub scale: Option<[f32; 3]>,
}

#[derive(Debug, Clone, Copy)]
pub struct Condition {
    /// `None` for a parameter the controller does not have: never true.
    pub parameter: Option<usize>,
    /// The engine's numbering: 1 set, 2 not set, 3 greater, 4 less,
    /// 6 equal, 7 not equal.
    pub mode: i32,
    pub threshold: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Target {
    State(usize),
    /// An entry or exit point of a sub-state machine.
    Selector(usize),
    Nowhere,
}

#[derive(Debug, Clone)]
pub struct TransitionDef {
    pub target: Target,
    pub duration: f32,
    pub fixed: bool,
    pub offset: f32,
    pub exit_time: Option<f32>,
    pub conditions: Vec<Condition>,
    /// 0 none, 1 the state being left, 2 the state being entered, 3 the
    /// first then the second, 4 the second then the first.
    pub interruption: i32,
}

#[derive(Debug, Clone, Default)]
pub struct NodeDef {
    /// 0 one parameter, 1 simple directional, 2 freeform directional,
    /// 3 freeform Cartesian. A direct blend (4) is not worked out: its
    /// children share the weight equally.
    pub blend_type: i32,
    pub parameter_x: Option<usize>,
    pub parameter_y: Option<usize>,
    pub children: Vec<usize>,
    pub thresholds: Vec<f32>,
    pub positions: Vec<[f32; 2]>,
    /// For a leaf: the clip, by its place in the controller's list.
    pub clip: Option<usize>,
    /// What the node's own length is multiplied by.
    pub length_factor: f32,
    pub cycle_offset: f32,
    /// A blend whose parameter the controller does not have (or that names
    /// none): the engine works out no weights for it and its children take
    /// those its layer's workspace last held (`Animator::keep_weights`).
    pub missing: bool,
}

#[derive(Debug, Clone)]
pub struct StateDef {
    pub name: String,
    pub speed: f32,
    pub speed_parameter: Option<usize>,
    pub looping: bool,
    /// Whether it writes the defaults of what its motion does not move
    /// (`AnimState::writes_defaults`; `kept_share`).
    pub writes_defaults: bool,
    pub cycle_offset: f32,
    /// One blend tree per layer sharing the machine: the owner's first.
    pub trees: Vec<Option<Vec<NodeDef>>>,
    pub transitions: Vec<TransitionDef>,
    pub movement: Option<MovementBehaviour>,
    /// Its scripts that set the third layer's weight
    /// (`trp_data::states::LayerWeight`).
    pub layer_weights: Vec<trp_data::weapons::states::LayerWeight>,
    /// Whether its clips were read. Transitions into a state that was
    /// left out are never taken.
    pub loaded: bool,
}

#[derive(Debug, Clone, Default)]
pub struct MachineDef {
    pub states: Vec<StateDef>,
    pub any_state: Vec<TransitionDef>,
    /// Each a list of choices tried in order.
    pub selectors: Vec<Vec<(Target, Vec<Condition>)>>,
    pub default_state: usize,
    /// Whether any of its blend trees has a node `missing` its parameter:
    /// only then are its layers' workspaces followed.
    pub keeps_weights: bool,
}

#[derive(Debug, Clone)]
pub struct LayerDef {
    pub name: String,
    pub machine: usize,
    /// 0 for the machine's owner; otherwise its place among the layers
    /// synchronised with it.
    pub slot: usize,
    pub default_weight: f32,
    pub additive: bool,
    /// Which of the animator's bones the layer may move.
    pub moves: Vec<bool>,
}

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct BoneTracks {
    pub(super) position: Option<usize>,
    /// First curve of the rotation, and whether it is Euler angles.
    pub(super) rotation: Option<(usize, bool)>,
    pub(super) scale: Option<usize>,
}

/// A bone's place and turn in its parent.
pub(super) type Bone = ([f32; 3], [f32; 4]);

/// What a clip gives at a time (`Controller::sampled`).
#[derive(Debug, Clone, Default)]
pub struct Sampled {
    /// Each bone's position, rotation (three Euler angles or four) and scale
    /// as sampled, by its place; empty where the clip has no such track.
    pub bones: Vec<[Vec<f32>; 3]>,
    /// The same of the clip's reference pose, where it has one.
    pub references: Vec<[Vec<f32>; 3]>,
    /// Each parameter the clip drives, with its curve's value.
    pub curves: Vec<(usize, f32)>,
    /// The root's path and turn, as sampled among the rest and alone.
    pub root: Vec<f32>,
    pub root_alone: Vec<f32>,
}

#[derive(Clone)]
pub struct ClipDef {
    /// The clip cut down to what every update samples: the tracks of the
    /// bones the controller always works out (`Controller::always`), the
    /// curves that drive parameters and the root's. Its name, events and
    /// length are the clip's own.
    pub clip: Clip,
    /// The tracks of the controller's other bones, sampled only where the
    /// rest of a pose is made (`Controller::pose_of`): none where every bone
    /// is worked out at each update, or where the clip moves none of them.
    pub(super) rest: Option<Clip>,
    /// Each bone's tracks, in the part of the clip that holds that bone.
    pub(super) bones: Vec<BoneTracks>,
    /// Curves that drive parameters: (parameter, curve).
    pub(super) curves: Vec<(usize, usize)>,
    pub(super) root_path: Option<[usize; 3]>,
    /// The curves of the root's turning, a quaternion.
    pub(super) root_turn: Option<[usize; 4]>,
    /// Whether the clip has a reference pose for every curve it was read
    /// with: only such a clip adds anything on an additive layer, and its
    /// parts' references are cut from it.
    pub(super) referenced: bool,
}

impl ClipDef {
    /// Splits the clip into the curves of the bones every update works out
    /// (`always`) with the curves that drive parameters and the root's, and
    /// the curves of the other bones. A track two bones share (a bone asked
    /// for twice, as the body's root is) goes into both parts where the two
    /// are on either side. Each curve keeps its keys, samples and figure, so
    /// each samples to the bit as it did whole (`Clip::part`).
    pub(super) fn split(&mut self, always: &[bool]) {
        let count = self.clip.curve_count();
        let (mut kept, mut rest) = (vec![false; count], vec![false; count]);
        let mark = |wanted: &mut Vec<bool>, tracks: &BoneTracks| {
            let mut run = |first: usize, width: usize| wanted[first..first + width].iter_mut().for_each(|w| *w = true);
            if let Some(c) = tracks.position {
                run(c, 3);
            }
            if let Some((c, euler)) = tracks.rotation {
                run(c, if euler { 3 } else { 4 });
            }
            if let Some(c) = tracks.scale {
                run(c, 3);
            }
        };
        for (bone, tracks) in self.bones.iter().enumerate() {
            mark(if always[bone] { &mut kept } else { &mut rest }, tracks);
        }
        for float in &self.clip.floats {
            kept[float.curve] = true;
        }
        // A curve's number in a part: how many of the part's come before it.
        let numbers = |wanted: &[bool]| -> Vec<usize> {
            let mut next = 0;
            wanted
                .iter()
                .map(|w| {
                    let at = next;
                    next += usize::from(*w);
                    at
                })
                .collect()
        };
        let (in_kept, in_rest) = (numbers(&kept), numbers(&rest));
        for (bone, tracks) in self.bones.iter_mut().enumerate() {
            let to = if always[bone] { &in_kept } else { &in_rest };
            tracks.position = tracks.position.map(|c| to[c]);
            tracks.rotation = tracks.rotation.map(|(c, euler)| (to[c], euler));
            tracks.scale = tracks.scale.map(|c| to[c]);
        }
        for (_, curve) in &mut self.curves {
            *curve = in_kept[*curve];
        }
        self.root_path = self.root_path.map(|path| path.map(|c| in_kept[c]));
        self.root_turn = self.root_turn.map(|turn| turn.map(|c| in_kept[c]));
        self.rest = rest.contains(&true).then(|| self.clip.part(&rest));
        self.clip = self.clip.part(&kept);
    }
}
