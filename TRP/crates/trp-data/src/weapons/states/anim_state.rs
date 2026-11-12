//! Transitions, blend trees, layers and an animator state.

use super::*;

/// A way out of a state, as the controller stores it.
#[derive(Debug, Clone)]
pub struct Transition {
    /// Full name of the state it leads to. `None` for the layer's exit and
    /// for destinations that are not plain states.
    pub to: Option<String>,
    /// The entry or exit point it leads to instead, by its place in the
    /// machine's list of them.
    pub to_selector: Option<usize>,
    /// Which side's transitions may cut this one short: 0 none, 1 the
    /// state being left, 2 the state being entered, 3 the first then the
    /// second, 4 the second then the first.
    pub interruption: i32,
    pub ordered_interruption: bool,
    /// Length of the blend: seconds if `fixed_duration`, otherwise a
    /// fraction of the state being left.
    pub duration: f32,
    pub fixed_duration: bool,
    /// Where in the destination the blend starts, as a fraction of it.
    pub offset: f32,
    /// The fraction of the state being left at which the transition may
    /// start, when it has to wait for one.
    pub exit_time: Option<f32>,
    pub conditions: Vec<Condition>,
}

/// One node of a state's blend tree. The first node of a state is its root.
#[derive(Debug, Clone)]
pub struct TreeNode {
    /// Unity's numbering: 0 one parameter, 1 simple directional, 2 freeform
    /// directional, 3 freeform Cartesian, 4 direct.
    pub blend_type: i32,
    pub parameter_x: Option<String>,
    pub parameter_y: Option<String>,
    /// Indices of the child nodes in the state's list.
    pub children: Vec<usize>,
    /// For one parameter: the value at which each child plays alone.
    pub thresholds: Vec<f32>,
    /// For two parameters: where each child sits.
    pub positions: Vec<[f32; 2]>,
    /// For a leaf: the clip.
    pub clip: Option<(String, ObjRef)>,
    /// For a leaf: what the clip's own length is multiplied by.
    pub duration: f32,
    pub cycle_offset: f32,
}

/// The game's script on a state that sets its animator's third layer's
/// weight (`LayerWeightStateController`: the weight `start` as the state is
/// entered, `curve` at its normalised time while it plays, `end` as it is
/// left). An underbarrel launcher's controller raises its `Leveling` layer
/// so (underbarrel-launcher spec, "Two animators").
#[derive(Debug, Clone)]
pub struct LayerWeight {
    pub start: f32,
    pub end: f32,
    pub curve: crate::formats::curve::Curve,
    /// How the curve goes on before its first key and after its last
    /// (`m_PreInfinity`, `m_PostInfinity`).
    pub before: crate::formats::curve::Wrap,
    pub after: crate::formats::curve::Wrap,
}

impl LayerWeight {
    /// The layer the script sets (`SetLayerWeight(2, ...)`).
    pub const LAYER: usize = 2;

    /// The weight at a normalised time (`LayerWeight.Evaluate`), the time
    /// taken back into the curve's keys by its wrap where it lies outside.
    pub fn at(&self, time: f32) -> f32 {
        self.curve.evaluate_wrapped(time, self.before, self.after)
    }
}

#[derive(Debug, Clone)]
pub struct AnimState {
    /// Index of the state machine the state is in; one per animator layer.
    pub machine: usize,
    /// Name with the layer in front, for example `Hands.FIRE`.
    pub full_name: String,
    pub speed: f32,
    /// A float parameter that multiplies the speed, if the state has one.
    pub speed_parameter: Option<String>,
    pub looping: bool,
    /// Whether the state writes the default value of whatever its motion
    /// does not move (`m_WriteDefaultValues`, on unless set off): off, its
    /// layer leaves such a bone as it stands (the Rhino's `FIRE`, with no
    /// motion on its `Cylinder` layer, leaves the cylinder at its chamber).
    pub writes_defaults: bool,
    /// Where in its cycle the state starts, as a fraction.
    pub cycle_offset: f32,
    /// The movement state it stands for, on the body's controller.
    pub movement: Option<MovementBehaviour>,
    /// Its scripts that set the third layer's weight (`LayerWeight`).
    pub layer_weights: Vec<LayerWeight>,
    /// The state's clip, or the clips its blend tree chooses between, in
    /// the tree's order.
    pub clips: Vec<(String, ObjRef)>,
    /// For a blend tree over one parameter: its name and the value at
    /// which each clip plays alone.
    pub blend_parameter: Option<String>,
    pub thresholds: Vec<f32>,
    pub events: Vec<StateEvent>,
    /// In the controller's order, which is the order they are tried in.
    pub transitions: Vec<Transition>,
    /// The state's blend trees in full; `clips` is their leaves. A state
    /// has one tree per layer that plays it: its own layer first, then
    /// each layer synchronised with that one, in the controller's order.
    pub trees: Vec<Vec<TreeNode>>,
    /// For each of those layers, which of `trees` it plays, or -1 for none.
    pub tree_of_layer: Vec<i32>,
}

/// One layer of the animator. Later layers override earlier ones on the
/// bones their mask lets through.
#[derive(Debug, Clone)]
pub struct Layer {
    pub name: String,
    /// Index of the layer's state machine, as in [`AnimState::machine`].
    pub machine: usize,
    /// 0 for the layer that owns the machine. A layer synchronised with
    /// it has its place among the layers sharing the machine, which is
    /// also its place in [`AnimState::tree_of_layer`].
    pub sync_slot: usize,
    pub default_weight: f32,
    pub additive: bool,
    /// Bones the layer may move, by the hash of their path, with a weight
    /// of 0 or 1. Empty means every bone.
    pub mask: Vec<(u32, f32)>,
    /// The state the layer starts in.
    pub default_state: Option<String>,
}

impl AnimState {
    /// The clip played for variant `index` of a state whose blend tree
    /// holds one clip per variant: the one whose name ends in `_<index>`,
    /// or the first when none does.
    pub fn clip_for(&self, index: i32) -> Option<&(String, ObjRef)> {
        let suffix = format!("_{index}");
        self.clips.iter().find(|(name, _)| name.ends_with(&suffix)).or_else(|| self.clips.first())
    }

    /// The clip played while the state's blend parameter stands at
    /// `value`: of the tree's clips, the one whose own value is nearest.
    /// For a state with one clip, that clip.
    pub fn clip_at(&self, value: f32) -> Option<&(String, ObjRef)> {
        self.clips
            .iter()
            .zip(&self.thresholds)
            .min_by(|a, b| (a.1 - value).abs().total_cmp(&(b.1 - value).abs()))
            .map(|(clip, _)| clip)
            .or_else(|| self.clips.first())
    }

    /// When the first enabled event of that name fires, as a fraction of the state.
    pub fn event_time(&self, name: &str) -> Option<f32> {
        self.events.iter().find(|e| e.enabled && e.name == name).map(|e| e.time)
    }

    /// The first transition from this state to the named one.
    pub fn transition_to(&self, full_name: &str) -> Option<&Transition> {
        self.transitions.iter().find(|t| t.to.as_deref() == Some(full_name))
    }
}

pub struct StateTable {
    pub parameters: Vec<Parameter>,
    pub states: Vec<AnimState>,
    pub layers: Vec<Layer>,
    /// Transitions that may be taken from any state of a machine, by the
    /// machine's index.
    pub any_state: Vec<(usize, Transition)>,
    /// The entry and exit points of each machine's sub-state machines.
    pub selectors: Vec<Vec<Selector>>,
}
