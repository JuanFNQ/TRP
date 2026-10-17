//! An animator: parameters, state machines, blend trees, transitions, root
//! motion, curve-driven parameters and the pose of a chosen set of bones
//! through the layer stack. See `docs/spec/body-animator.md`.
//!
//! It runs any controller read by `trp_data::states`; nothing here knows
//! about bodies or weapons.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use trp_data::rootmotion::{freeform_cartesian, freeform_directional, simple_directional, weight_1d};
use trp_data::states::{self, AnimState, MovementBehaviour, ParameterKind, StateTable, TreeNode};
use trp_data::unity::anim::{euler_to_quat, path_hash, Clip, Property, MOTION_CURVE};
use trp_data::unity::{Assets, ObjRef};

use crate::math::{quat_lerp, quat_mul, quat_normalize, quat_rotate};

/// A state with no motion lasts this long, as in the engine.
const EMPTY_STATE_LENGTH: f32 = 1.0;
/// How many entry and exit points a transition may pass through.
const SELECTOR_DEPTH: usize = 8;

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
}

#[derive(Debug, Clone)]
pub struct StateDef {
    pub name: String,
    pub speed: f32,
    pub speed_parameter: Option<usize>,
    pub looping: bool,
    pub cycle_offset: f32,
    /// One blend tree per layer sharing the machine: the owner's first.
    pub trees: Vec<Option<Vec<NodeDef>>>,
    pub transitions: Vec<TransitionDef>,
    pub movement: Option<MovementBehaviour>,
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
struct BoneTracks {
    position: Option<usize>,
    /// First curve of the rotation, and whether it is Euler angles.
    rotation: Option<(usize, bool)>,
    scale: Option<usize>,
}

pub struct ClipDef {
    pub clip: Clip,
    bones: Vec<BoneTracks>,
    /// Curves that drive parameters: (parameter, curve).
    curves: Vec<(usize, usize)>,
    root_path: Option<[usize; 3]>,
    /// The curves of the root's turning, a quaternion.
    root_turn: Option<[usize; 4]>,
}

pub struct Controller {
    pub parameters: Vec<states::Parameter>,
    by_name: HashMap<String, usize>,
    pub machines: Vec<MachineDef>,
    pub layers: Vec<LayerDef>,
    pub clips: Vec<ClipDef>,
    pub bones: Vec<BoneRest>,
    /// The bone whose track holds the root's path through each clip. That
    /// path is handed out as root motion, so it is taken off the bone.
    pub root_bone: Option<usize>,
    /// Parameters that some clip carries a curve for.
    curve_driven: Vec<bool>,
    /// The bones whose scale is kept that some clip scales.
    pub scaled: Vec<usize>,
}

impl Controller {
    /// Builds the controller from what was read of it, reading the clips of
    /// the states `wanted` picks and of no others.
    pub fn load(
        assets: &Assets,
        table: &StateTable,
        bones: Vec<BoneRest>,
        wanted: &dyn Fn(&AnimState) -> bool,
    ) -> Result<Self> {
        let by_name: HashMap<String, usize> =
            table.parameters.iter().enumerate().map(|(i, p)| (p.name.clone(), i)).collect();
        let by_hash: HashMap<u32, usize> =
            table.parameters.iter().enumerate().map(|(i, p)| (path_hash(&p.name), i)).collect();
        let bone_hashes: Vec<u32> = bones.iter().map(|b| path_hash(&b.path)).collect();
        let scale_hashes: Vec<u32> = bones.iter().filter(|b| b.scale.is_some()).map(|b| path_hash(&b.path)).collect();
        let parameter = |name: &Option<String>| name.as_ref().and_then(|n| by_name.get(n).copied());
        let conditions = |list: &[states::Condition]| -> Vec<Condition> {
            list.iter()
                .map(|c| Condition { parameter: by_name.get(&c.parameter).copied(), mode: c.mode, threshold: c.threshold })
                .collect()
        };

        // States by machine, and where each sits in its machine's list.
        let machine_count = table.states.iter().map(|s| s.machine + 1).max().unwrap_or(0).max(table.selectors.len());
        let mut place: HashMap<&str, usize> = HashMap::new();
        let mut counts = vec![0usize; machine_count];
        for s in &table.states {
            place.insert(s.full_name.as_str(), counts[s.machine]);
            counts[s.machine] += 1;
        }
        let target = |to: &Option<String>, selector: Option<usize>| match (to, selector) {
            (Some(name), _) => place.get(name.as_str()).map_or(Target::Nowhere, |i| Target::State(*i)),
            (None, Some(index)) => Target::Selector(index),
            _ => Target::Nowhere,
        };
        let transition = |t: &states::Transition| TransitionDef {
            target: target(&t.to, t.to_selector),
            duration: t.duration,
            fixed: t.fixed_duration,
            offset: t.offset,
            exit_time: t.exit_time,
            conditions: conditions(&t.conditions),
            interruption: t.interruption,
        };

        let mut clips: Vec<ClipDef> = Vec::new();
        let mut clip_of: HashMap<ObjRef, usize> = HashMap::new();
        let mut curve_driven = vec![false; table.parameters.len()];
        let mut machines: Vec<MachineDef> = (0..machine_count).map(|_| MachineDef::default()).collect();
        for s in &table.states {
            let loaded = wanted(s);
            let mut trees: Vec<Option<Vec<NodeDef>>> = Vec::new();
            // A state with no list of trees by layer has one tree, its own.
            let slots: Vec<i32> = if s.tree_of_layer.is_empty() { vec![0] } else { s.tree_of_layer.clone() };
            for tree_index in slots {
                let Some(nodes) = usize::try_from(tree_index).ok().and_then(|i| s.trees.get(i)).filter(|_| loaded) else {
                    trees.push(None);
                    continue;
                };
                let mut out = Vec::with_capacity(nodes.len());
                for node in nodes {
                    let clip = match &node.clip {
                        Some((_, obj)) => Some(match clip_of.get(obj) {
                            Some(index) => *index,
                            None => {
                                let def = clip_def(&Clip::read(assets, *obj)?, &bone_hashes, &scale_hashes, &by_hash);
                                for (parameter, _) in &def.curves {
                                    curve_driven[*parameter] = true;
                                }
                                clips.push(def);
                                clip_of.insert(*obj, clips.len() - 1);
                                clips.len() - 1
                            }
                        }),
                        None => None,
                    };
                    out.push(node_def(node, clip, &parameter));
                }
                trees.push(Some(out));
            }
            machines[s.machine].states.push(StateDef {
                name: s.full_name.clone(),
                speed: s.speed,
                speed_parameter: parameter(&s.speed_parameter),
                looping: s.looping,
                cycle_offset: s.cycle_offset,
                trees,
                transitions: s.transitions.iter().map(transition).collect(),
                movement: s.movement.clone(),
                loaded,
            });
        }
        for (machine, t) in &table.any_state {
            machines[*machine].any_state.push(transition(t));
        }
        for (machine, list) in table.selectors.iter().enumerate() {
            machines[machine].selectors = list
                .iter()
                .map(|s| s.choices.iter().map(|c| (target(&c.to, c.to_selector), conditions(&c.conditions))).collect())
                .collect();
        }

        let mut layers = Vec::new();
        for layer in &table.layers {
            if let Some(default) = layer.default_state.as_deref().and_then(|name| place.get(name)) {
                if layer.sync_slot == 0 {
                    machines[layer.machine].default_state = *default;
                }
            }
            layers.push(LayerDef {
                name: layer.name.clone(),
                machine: layer.machine,
                slot: layer.sync_slot,
                default_weight: layer.default_weight,
                additive: layer.additive,
                moves: bone_hashes
                    .iter()
                    .map(|hash| layer.mask.is_empty() || layer.mask.iter().any(|(h, w)| h == hash && *w > 0.0))
                    .collect(),
            });
        }
        let scaled = (0..bones.len()).filter(|i| clips.iter().any(|clip| clip.bones[*i].scale.is_some())).collect();
        Ok(Self {
            parameters: table.parameters.clone(),
            by_name,
            machines,
            layers,
            clips,
            bones,
            root_bone: None,
            curve_driven,
            scaled,
        })
    }

    /// A controller written out by hand, with no clips and no bones: a
    /// state of it lasts a second over its speed. For trying the logic
    /// that goes by an animator where there is no install to read one from.
    pub fn by_hand(parameters: Vec<states::Parameter>, machines: Vec<MachineDef>, layers: Vec<LayerDef>) -> Self {
        Self {
            by_name: parameters.iter().enumerate().map(|(i, p)| (p.name.clone(), i)).collect(),
            curve_driven: vec![false; parameters.len()],
            parameters,
            machines,
            layers,
            clips: Vec::new(),
            bones: Vec::new(),
            root_bone: None,
            scaled: Vec::new(),
        }
    }

    pub fn parameter(&self, name: &str) -> Option<usize> {
        self.by_name.get(name).copied()
    }

    pub fn layer(&self, name: &str) -> Option<usize> {
        self.layers.iter().position(|l| l.name == name)
    }

    /// A state's place in a layer's machine, by its full name.
    pub fn state(&self, layer: usize, full_name: &str) -> Option<usize> {
        let machine = self.layers.get(layer)?.machine;
        self.machines[machine].states.iter().position(|s| s.name == full_name)
    }
}

fn node_def(node: &TreeNode, clip: Option<usize>, parameter: &dyn Fn(&Option<String>) -> Option<usize>) -> NodeDef {
    NodeDef {
        blend_type: node.blend_type,
        parameter_x: parameter(&node.parameter_x),
        parameter_y: parameter(&node.parameter_y),
        children: node.children.clone(),
        thresholds: node.thresholds.clone(),
        positions: node.positions.clone(),
        clip,
        length_factor: if node.duration > 0.0 { node.duration } else { 1.0 },
        cycle_offset: node.cycle_offset,
    }
}

/// Cuts a clip down to the animator's bones and the curves it reads: a
/// bone's scale only where it is kept (`scaled`).
fn clip_def(clip: &Clip, bones: &[u32], scaled: &[u32], parameters: &HashMap<u32, usize>) -> ClipDef {
    let reduced = clip.reduced(
        |t| bones.contains(&t.path) && (t.property != Property::Scale || scaled.contains(&t.path)),
        |f| match f.custom_type {
            0 => parameters.contains_key(&f.name),
            // The root's path and its turning.
            MOTION_CURVE => (7..=13).contains(&f.name),
            _ => false,
        },
    );
    let tracks = bones
        .iter()
        .map(|hash| {
            let mut found = BoneTracks::default();
            for t in reduced.tracks.iter().filter(|t| t.path == *hash) {
                match t.property {
                    Property::Position => found.position = Some(t.curve),
                    Property::Rotation => found.rotation = Some((t.curve, false)),
                    Property::Euler => found.rotation = Some((t.curve, true)),
                    Property::Scale => found.scale = Some(t.curve),
                }
            }
            found
        })
        .collect();
    let curves = reduced
        .floats
        .iter()
        .filter(|f| f.custom_type == 0)
        .filter_map(|f| Some((*parameters.get(&f.name)?, f.curve)))
        .collect();
    ClipDef { root_path: reduced.root_path(), root_turn: reduced.root_turn(), bones: tracks, curves, clip: reduced }
}

/// Whether a component of the root's travel (0 x, 1 y, 2 z) stays in the
/// clip's pose instead of going out as root motion.
fn stays_in_pose(clip: &Clip, component: usize) -> bool {
    if component == 1 {
        clip.root_stays_upright
    } else {
        clip.root_stays_level
    }
}

/// A state being played: which one, and how far through it, in cycles.
#[derive(Debug, Clone, Copy)]
struct Playing {
    state: usize,
    time: f32,
    /// Where it was before the last update.
    previous: f32,
}

#[derive(Debug, Clone, Copy)]
struct Blend {
    duration: f32,
    fixed: bool,
    elapsed: f32,
    interruption: i32,
    /// For a blend out of states held still (`Held`): how fast the
    /// current one of them was playing. A length that is a share of the
    /// state is then that share of the state being entered, over this.
    pace: Option<f32>,
}

/// What a transition leaves behind when one out of the state it was
/// entering cuts it short: the blend as the update before left it, held
/// still, and the new transition blends out of that
/// (`body-animator.md`, "A transition cut short").
#[derive(Debug, Clone)]
struct Held {
    /// Where the current state stood when it was held. (Its own clock is
    /// moved on once more in the update that holds it, which is what the
    /// engine reports of it from then on.)
    current_at: f32,
    /// The states held beside the current one, each where it stood, with
    /// its share of the whole. The current one has what is left.
    others: Vec<(Playing, f32)>,
    /// How far the blend moved and turned the root in a second in the
    /// update before it was held: it goes on giving that, by its share.
    motion: ([f32; 3], f32),
}

#[derive(Debug, Clone)]
struct MachineState {
    current: Playing,
    next: Option<Playing>,
    blend: Option<Blend>,
    held: Option<Held>,
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
struct Share {
    clip: usize,
    weight: f32,
    previous: f32,
    now: f32,
    offset: f32,
}

#[derive(Clone)]
pub struct Animator {
    pub controller: Arc<Controller>,
    values: Vec<f32>,
    machines: Vec<MachineState>,
    weights: Vec<f32>,
    root_delta: [f32; 3],
    /// How far the root turned over the last update, degrees to the right.
    root_turn: f32,
    pose: Vec<([f32; 3], [f32; 4])>,
    /// The scale of each bone of the controller's `scaled`, in its order.
    scales: Vec<[f32; 3]>,
    /// Whether a second pose is worked out at each update **for drawing**,
    /// and that pose: the same states at the same moments, their clips'
    /// sampled curves taken on curves without corners in place of the
    /// engine's straight lines (`Clip::sample_smooth`). Nothing of the
    /// animator goes by it.
    smooth: bool,
    drawn: Option<Vec<([f32; 3], [f32; 4])>>,
    events: Vec<Event>,
    /// The clips' animation events passed on the last update, each by its
    /// clip's place in the controller and its own in the clip.
    passed: Vec<(usize, usize)>,
    samples: Vec<f32>,
    /// How long the update being worked out is, in seconds.
    step: f32,
    /// States asked for outright since the last update, by machine: the
    /// state and the point in its play.
    asked: Vec<Option<(usize, f32)>>,
    /// What the last update looked at, in order.
    looks: Vec<Look>,
}

impl Animator {
    pub fn new(controller: Arc<Controller>) -> Self {
        let values = controller.parameters.iter().map(|p| p.default).collect();
        let machines = controller
            .machines
            .iter()
            .map(|m| MachineState {
                current: Playing { state: m.default_state, time: 0.0, previous: 0.0 },
                next: None,
                blend: None,
                held: None,
            })
            .collect();
        let weights = controller.layers.iter().map(|l| l.default_weight).collect();
        let pose = controller.bones.iter().map(|b| (b.position, b.rotation)).collect();
        let scales = controller.scaled.iter().map(|i| controller.bones[*i].scale.unwrap_or([1.0; 3])).collect();
        // The first layer always plays in full, whatever weight it stores.
        let mut animator =
            Self {
                controller,
                values,
                machines,
                weights,
                root_delta: [0.0; 3],
                root_turn: 0.0,
                pose,
                scales,
                smooth: false,
                drawn: None,
                events: Vec::new(),
                passed: Vec::new(),
                samples: Vec::new(),
                step: 0.0,
                asked: Vec::new(),
                looks: Vec::new(),
            };
        animator.asked = vec![None; animator.machines.len()];
        if let Some(first) = animator.weights.first_mut() {
            *first = 1.0;
        }
        animator
    }

    pub fn set(&mut self, parameter: usize, value: f32) {
        self.values[parameter] = value;
    }

    pub fn set_float(&mut self, name: &str, value: f32) {
        if let Some(index) = self.controller.parameter(name) {
            self.values[index] = value;
        }
    }

    pub fn set_bool(&mut self, name: &str, value: bool) {
        self.set_float(name, value as u8 as f32);
    }

    pub fn get(&self, parameter: usize) -> f32 {
        self.values[parameter]
    }

    pub fn float(&self, name: &str) -> f32 {
        self.controller.parameter(name).map_or(0.0, |i| self.values[i])
    }

    pub fn set_layer_weight(&mut self, layer: usize, weight: f32) {
        if let Some(w) = self.weights.get_mut(layer) {
            *w = weight;
        }
    }

    /// The state a layer is in, and the one it is blending into if any.
    pub fn state(&self, layer: usize) -> (&StateDef, Option<&StateDef>) {
        let machine = self.controller.layers[layer].machine;
        let (def, state) = (&self.controller.machines[machine], &self.machines[machine]);
        (&def.states[state.current.state], state.next.map(|n| &def.states[n.state]))
    }

    /// How far a layer's current state has played, in cycles, and how far
    /// its transition has run, 0 to 1 (0 with none under way).
    pub fn progress(&self, layer: usize) -> (f32, f32) {
        let machine = self.controller.layers[layer].machine;
        let state = &self.machines[machine];
        (state.current.time, self.blend_weight(machine))
    }

    /// How far the state a layer is blending into has played, in cycles.
    pub fn next_progress(&self, layer: usize) -> Option<f32> {
        let machine = self.controller.layers[layer].machine;
        self.machines[machine].next.map(|n| n.time)
    }

    /// Every layer as it stands, for looking at: its name and weight, the
    /// state it is in and how far through, and the state it is entering.
    pub fn describe(&self) -> Vec<String> {
        let controller = &self.controller;
        controller
            .layers
            .iter()
            .enumerate()
            .map(|(index, layer)| {
                let (def, state) = (&controller.machines[layer.machine], &self.machines[layer.machine]);
                let name = |playing: &Playing| {
                    let s = &def.states[playing.state];
                    format!("{}{} at {:.3}", s.name, if s.loaded { "" } else { " (not read)" }, playing.time)
                };
                format!(
                    "{} (slot {}, weight {:.2}{}): {}{}",
                    layer.name,
                    layer.slot,
                    self.weights[index],
                    if layer.additive { ", additive" } else { "" },
                    name(&state.current),
                    state.next.as_ref().map_or(String::new(), |next| format!(" -> {}", name(next)))
                )
            })
            .collect()
    }

    /// Plays every state that does not loop to its end, where one that has
    /// been in it for a while is. Several of the body's layers rest in
    /// short clips that run from a start pose to the pose they then hold.
    pub fn settle(&mut self) {
        let controller = self.controller.clone();
        for (machine, state) in self.machines.iter_mut().enumerate() {
            let def = &controller.machines[machine];
            if state.next.is_none() && def.states.get(state.current.state).is_some_and(|s| !s.looping) {
                state.current.time = state.current.time.max(1.0);
                state.current.previous = state.current.time;
            }
        }
    }

    /// Puts a layer's current state at a point in its play, in cycles.
    pub fn set_progress(&mut self, layer: usize, time: f32) {
        let machine = self.controller.layers[layer].machine;
        let current = &mut self.machines[machine].current;
        (current.time, current.previous) = (time, time);
    }

    /// The state of a layer's machine whose own name hashes to `hash`, as
    /// the engine's `shortNameHash` gives it (its low 24 bits suffice, as
    /// the recorder writes them).
    pub fn state_by_short_hash(&self, layer: usize, hash: u32) -> Option<usize> {
        let machine = self.controller.layers[layer].machine;
        self.controller.machines[machine].states.iter().position(|state| {
            let own = state.name.rsplit('.').next().unwrap_or(&state.name);
            path_hash(own) & 0x00ff_ffff == hash & 0x00ff_ffff
        })
    }

    /// Puts a layer's machine in a state outright, at a point in its play
    /// (in cycles), with nothing blending: for a run that takes up where a
    /// recording of one stood. A layer synchronised with another's machine
    /// moves that machine.
    pub fn start_in(&mut self, layer: usize, state: usize, time: f32) {
        let machine = self.controller.layers[layer].machine;
        if state < self.controller.machines[machine].states.len() {
            let playing = &mut self.machines[machine];
            playing.current = Playing { state, time, previous: time };
            (playing.next, playing.blend, playing.held) = (None, None, None);
            self.asked[machine] = None;
        }
    }

    /// Asks a layer for a state outright, at a point in its play (in
    /// cycles): the engine's `Play`. The next update enters it there and
    /// runs none of it; whatever transition was under way is dropped.
    pub fn play(&mut self, layer: usize, state: usize, time: f32) {
        let machine = self.controller.layers[layer].machine;
        if state < self.controller.machines[machine].states.len() {
            self.asked[machine] = Some((state, time));
        }
    }

    /// How many layers the controller has.
    pub fn layer_count(&self) -> usize {
        self.controller.layers.len()
    }

    /// Whether a layer is its machine's own, not one synchronised with
    /// another layer's.
    pub fn owns_machine(&self, layer: usize) -> bool {
        self.controller.layers.get(layer).is_some_and(|def| def.slot == 0)
    }

    /// The states the last update had a look at, in order.
    pub fn looks(&self) -> &[Look] {
        &self.looks
    }

    /// Whether a layer is in a transition.
    pub fn in_transition(&self, layer: usize) -> bool {
        self.machines[self.controller.layers[layer].machine].next.is_some()
    }

    pub fn layer_weight(&self, layer: usize) -> f32 {
        self.weights.get(layer).copied().unwrap_or(0.0)
    }

    /// The pose of every bone after the last update.
    pub fn pose(&self) -> &[([f32; 3], [f32; 4])] {
        &self.pose
    }

    /// The scale of each bone whose scale is kept and that some clip
    /// scales, by the bone's place, after the last update.
    pub fn scales(&self) -> impl Iterator<Item = (usize, [f32; 3])> + '_ {
        self.controller.scaled.iter().copied().zip(self.scales.iter().copied())
    }

    /// Asks for a pose for drawing beside the pose, from the next update
    /// on, or for none. **How a result is drawn, and no part of the
    /// result**: the pose, the parameters the clips drive, the root's
    /// movement and the events are what they are without it.
    ///
    /// The game's clips keep most of their curves as 30 samples a second,
    /// which the engine joins by straight lines; where a weapon is turned
    /// fast in one hand and the other hand's place on it is kept by a
    /// curve that undoes the turn, the hand is right at each sample and
    /// up to 3 mm off between them, 30 times a second (the look all over
    /// a weapon; `docs/findings.md`, 2026-10-13). The pose for drawing
    /// takes the samples on curves without corners.
    pub fn set_drawn_smooth(&mut self, on: bool) {
        self.smooth = on;
        if !on {
            self.drawn = None;
        }
    }

    /// The pose to draw: the pose for drawing where one was asked for and
    /// has been worked out, and the pose itself otherwise.
    pub fn drawn_pose(&self) -> &[([f32; 3], [f32; 4])] {
        self.drawn.as_deref().unwrap_or(&self.pose)
    }

    /// The root's movement over the last update, in the animated object's
    /// own axes.
    pub fn root_motion(&self) -> [f32; 3] {
        self.root_delta
    }

    /// How far the root turned over the last update about the vertical, in
    /// degrees, positive to the right.
    pub fn root_turn(&self) -> f32 {
        self.root_turn
    }

    /// A bone's place in its parent after the last update.
    pub fn bone(&self, index: usize) -> ([f32; 3], [f32; 4]) {
        self.pose[index]
    }

    /// The pose the bones would have now with some parameters at other
    /// values and nothing else different: no time passes, and nothing of
    /// this animator changes. For a host that draws between the frames:
    /// what the view's newest turn would make of the pose.
    pub fn pose_with(&self, parameters: &[(usize, f32)]) -> Vec<([f32; 3], [f32; 4])> {
        let mut copy = self.clone();
        for (parameter, value) in parameters {
            copy.values[*parameter] = *value;
        }
        let controller = copy.controller.clone();
        copy.evaluate(&controller);
        copy.pose
    }

    pub fn state_def(&self, machine: usize, state: usize) -> &StateDef {
        &self.controller.machines[machine].states[state]
    }

    /// Advances by `dt` seconds and returns what states were entered and left.
    pub fn update(&mut self, dt: f32) -> Vec<Event> {
        self.events.clear();
        self.passed.clear();
        self.looks.clear();
        let controller = self.controller.clone();
        for machine in 0..controller.machines.len() {
            self.step_machine(&controller, machine, dt);
        }
        self.step = dt;
        self.evaluate(&controller);
        std::mem::take(&mut self.events)
    }

    /// The clips' animation events passed on the last update (`m_Events`):
    /// each a clip with a share in the pose whose time went over the
    /// event's (`PROVISIONAL`: the engine's own rule for the clips of a
    /// blend and of a transition is not read; here every clip with a share
    /// above none passes its own).
    pub fn passed_events(&self) -> impl Iterator<Item = &trp_data::unity::anim::ClipEvent> + '_ {
        self.passed.iter().filter_map(|&(clip, event)| self.controller.clips.get(clip)?.clip.events.get(event))
    }

    fn look(&mut self, controller: &Controller, machine: usize, state: usize, kind: LookKind, time: f32, leaving: bool) {
        let looping = controller.machines[machine].states[state].looping;
        self.looks.push(Look { machine, state, kind, time, looping, leaving });
    }

    fn check(&self, conditions: &[Condition]) -> bool {
        conditions.iter().all(|c| {
            let Some(parameter) = c.parameter else { return false };
            let value = self.values[parameter];
            match c.mode {
                1 => value != 0.0,
                2 => value == 0.0,
                3 => value > c.threshold,
                4 => value < c.threshold,
                6 => value == c.threshold,
                7 => value != c.threshold,
                _ => false,
            }
        })
    }

    /// Follows a transition's destination through entry and exit points to
    /// a state whose clips are loaded.
    fn resolve(&self, def: &MachineDef, mut target: Target) -> Option<usize> {
        for _ in 0..SELECTOR_DEPTH {
            match target {
                Target::State(index) => return def.states.get(index).filter(|s| s.loaded).map(|_| index),
                Target::Selector(index) => {
                    target = def.selectors.get(index)?.iter().find(|(_, conditions)| self.check(conditions))?.0;
                }
                Target::Nowhere => return None,
            }
        }
        None
    }

    /// Seconds one cycle of a state takes with the parameters as they are.
    fn state_length(&self, controller: &Controller, def: &MachineDef, state: usize) -> f32 {
        let s = &def.states[state];
        let motion = match s.trees.first().and_then(Option::as_ref) {
            Some(tree) => {
                let mut leaves = Vec::new();
                self.leaves(tree, 0, 1.0, 1.0, &mut leaves);
                leaves.iter().map(|(clip, weight, factor, _)| controller.clips[*clip].clip.duration() * factor * weight).sum()
            }
            None => 0.0,
        };
        let motion = if motion > 0.0 { motion } else { EMPTY_STATE_LENGTH };
        let speed = (s.speed * s.speed_parameter.map_or(1.0, |p| self.values[p])).abs();
        if speed > 1e-6 {
            motion / speed
        } else {
            f32::INFINITY
        }
    }

    /// The clips of a blend tree with their weights, the factor on their
    /// length and their offset into the cycle.
    fn leaves(&self, tree: &[NodeDef], index: usize, weight: f32, factor: f32, out: &mut Vec<(usize, f32, f32, f32)>) {
        let Some(node) = tree.get(index) else { return };
        let factor = factor * node.length_factor;
        if let Some(clip) = node.clip {
            out.push((clip, weight, factor, node.cycle_offset));
            return;
        }
        let n = node.children.len();
        let value = |p: Option<usize>| p.map_or(0.0, |p| self.values[p]);
        let weights: Vec<f32> = match node.blend_type {
            0 if node.thresholds.len() == n => {
                (0..n).map(|i| weight_1d(&node.thresholds, i, value(node.parameter_x))).collect()
            }
            1 if node.positions.len() == n => {
                simple_directional(&node.positions, [value(node.parameter_x), value(node.parameter_y)])
            }
            2 if node.positions.len() == n => {
                freeform_directional(&node.positions, [value(node.parameter_x), value(node.parameter_y)])
            }
            3 if node.positions.len() == n => {
                freeform_cartesian(&node.positions, [value(node.parameter_x), value(node.parameter_y)])
            }
            _ => vec![1.0 / n.max(1) as f32; n],
        };
        for (child, w) in node.children.iter().zip(weights) {
            if w > 0.0 {
                self.leaves(tree, *child, weight * w, factor, out);
            }
        }
    }

    /// The first transition out of `state` that may start in a frame that
    /// takes the state from `start` to `end` (in cycles), with the state
    /// it leads to and the share of the frame that is left once it starts.
    ///
    /// A transition waiting for a point in its state starts at that point,
    /// not at the frame's edge, and what is left of the frame goes to the
    /// blend and the new state. One started by a condition alone starts
    /// with the frame over. (Recorded: the landing passes its exit point
    /// 0.29 of a frame before the frame ends, and the walk it blends into
    /// has moved on by 0.29 of a frame's worth.)
    fn triggered(&self, def: &MachineDef, state: usize, start: f32, end: f32) -> Option<(TransitionDef, usize, f32)> {
        let own = &def.states[state].transitions;
        let from_any = def.any_state.iter().map(|t| (t, true));
        for (t, any) in from_any.chain(own.iter().map(|t| (t, false))) {
            if !self.check(&t.conditions) {
                continue;
            }
            let mut left = 0.0;
            if let Some(exit) = t.exit_time {
                // A point in the first cycle comes round every cycle,
                // whether the state loops or not, and is taken only in a
                // frame that crosses it (the game's own animator code
                // tests it so, and the weapon's dry fire is recorded
                // leaving only at a cycle's end); a later one is passed
                // once.
                let (from, to, point) = if exit <= 1.0 {
                    let (from, to) = (start.fract(), end - start.floor());
                    let point = [exit, exit + 1.0].into_iter().find(|p| from <= *p && *p <= to);
                    (from, to, point)
                } else {
                    (start, end, (end >= exit).then_some(exit))
                };
                let Some(point) = point else { continue };
                // Long past the point, the whole frame is left.
                left = if to > from { ((to - point) / (to - from)).clamp(0.0, 1.0) } else { 0.0 };
            }
            let Some(target) = self.resolve(def, t.target) else { continue };
            // A transition from any state does not lead back to the same one.
            if any && target == state {
                continue;
            }
            return Some((t.clone(), target, left));
        }
        None
    }

    /// Clears the triggers a transition used.
    fn consume_triggers(&mut self, controller: &Controller, t: &TransitionDef) {
        for c in &t.conditions {
            if let Some(p) = c.parameter.filter(|p| controller.parameters[*p].kind == ParameterKind::Trigger) {
                self.values[p] = 0.0;
            }
        }
    }

    fn blend_weight(&self, machine: usize) -> f32 {
        let state = &self.machines[machine];
        let Some(blend) = state.blend else { return 0.0 };
        let total = self.blend_length(machine, &blend);
        if total > 0.0 {
            (blend.elapsed / total).clamp(0.0, 1.0)
        } else {
            1.0
        }
    }

    /// How long a blend under way takes, in seconds: a length in seconds
    /// as it is, and a share of the state as that share of the state
    /// being left as it now plays.
    ///
    /// PROVISIONAL, for a blend out of states held still: the share is
    /// then of the state being *entered*, over the pace the held state was
    /// playing at. Recorded twice (the walk entered out of a vault and out
    /// of a sprint's vault, both held at 1.25 times the clips' pace: the
    /// blend's weight is the walk's own progress over 0.08, for a share of
    /// 0.1), and what the engine really goes by is not known.
    fn blend_length(&self, machine: usize, blend: &Blend) -> f32 {
        let controller = &self.controller;
        let (def, state) = (&controller.machines[machine], &self.machines[machine]);
        match (blend.fixed, blend.pace, state.next) {
            (true, ..) => blend.duration,
            (false, Some(pace), Some(next)) => blend.duration * self.state_length(controller, def, next.state) / pace,
            _ => blend.duration * self.state_length(controller, def, state.current.state),
        }
    }

    /// How fast a state plays: its own speed times its speed parameter's.
    fn state_pace(&self, def: &MachineDef, state: usize) -> f32 {
        let s = &def.states[state];
        (s.speed * s.speed_parameter.map_or(1.0, |p| self.values[p])).abs().max(1e-6)
    }

    /// Starts a transition with `left` seconds of the frame still to run.
    fn start(&mut self, controller: &Controller, machine: usize, t: &TransitionDef, target: usize, left: f32, end: f32) {
        let def = &controller.machines[machine];
        self.consume_triggers(controller, t);
        let source_length = self.state_length(controller, def, self.machines[machine].current.state);
        let target_length = self.state_length(controller, def, target);
        let total = if t.fixed { t.duration } else { t.duration * source_length };
        self.events.push(Event::Entered { machine, state: target });
        let state = &mut self.machines[machine];
        // The new state gets the rest of the frame, blend or none.
        // (Recorded: the sidestep's way in ends 0.08 of a frame into a
        // frame and its hold, entered with no blend, stands 0.92 of a
        // frame on when that frame ends.)
        let head = if target_length.is_finite() { left / target_length } else { 0.0 };
        if total <= 0.0 || !total.is_finite() {
            // No blend: the new state takes over at once.
            let left = state.current.state;
            state.current = Playing { state: target, time: t.offset + head, previous: t.offset };
            state.next = None;
            state.blend = None;
            self.events.push(Event::Left { machine, state: left });
            self.look(controller, machine, left, LookKind::Exit, end, false);
            self.look(controller, machine, target, LookKind::Enter, t.offset + head, false);
            return;
        }
        let leaving = state.current.state;
        state.current.previous = state.current.time;
        state.current.time = end;
        state.next = Some(Playing { state: target, time: t.offset + head, previous: t.offset });
        state.blend = Some(Blend { duration: t.duration, fixed: t.fixed, elapsed: left, interruption: t.interruption, pace: None });
        self.look(controller, machine, leaving, LookKind::Update, end, true);
        self.look(controller, machine, target, LookKind::Enter, t.offset + head, false);
    }

    /// Starts a transition out of the state a transition under way was
    /// entering: what the update before made of the one cut short is held
    /// still, and the new one blends out of that. `left` is how much of
    /// the frame is still to run, and `end` where the current state's own
    /// clock stands at the frame's end.
    ///
    /// Recorded (thirteenth recording, the way out of a vault): a curve
    /// only the newly entered states carry reads on the frame of the cut
    /// exactly what it read the frame before, the state being left reports
    /// its clock one frame further on and then still, and the root goes on
    /// at the pace it had.
    fn cut_short(&mut self, controller: &Controller, machine: usize, t: &TransitionDef, target: usize, left: f32, end: f32) {
        let def = &controller.machines[machine];
        self.consume_triggers(controller, t);
        let Some(next) = self.machines[machine].next else { return };
        // The blend as the last update left it.
        let reached = self.blend_weight(machine);
        let base = controller.layers.first().map_or(0, |l| l.machine);
        let motion = if machine == base && self.step > 0.0 {
            (self.root_delta.map(|v| v / self.step), self.root_turn / self.step)
        } else {
            ([0.0; 3], 0.0)
        };
        let current = self.machines[machine].current;
        let pace = match self.machines[machine].blend.and_then(|blend| blend.pace) {
            Some(pace) => pace,
            None => self.state_pace(def, current.state),
        };
        let target_length = self.state_length(controller, def, target);
        let head = if target_length.is_finite() { left / target_length } else { 0.0 };
        self.events.push(Event::Entered { machine, state: target });
        let state = &mut self.machines[machine];
        let (mut others, current_at) = match state.held.take() {
            Some(held) => (held.others, held.current_at),
            None => {
                // The state being left reports this frame's play too.
                state.current.previous = current.time;
                state.current.time = end;
                (Vec::new(), current.time)
            }
        };
        for (_, share) in &mut others {
            *share *= 1.0 - reached;
        }
        others.push((Playing { state: next.state, time: next.time, previous: next.time }, reached));
        state.held = Some(Held { current_at, others, motion });
        state.next = Some(Playing { state: target, time: t.offset + head, previous: t.offset });
        state.blend = Some(Blend { duration: t.duration, fixed: t.fixed, elapsed: left, interruption: t.interruption, pace: Some(pace) });
        let leaving = state.current;
        self.look(controller, machine, leaving.state, LookKind::Update, leaving.time, true);
        self.look(controller, machine, next.state, LookKind::Update, next.time, true);
        self.look(controller, machine, target, LookKind::Enter, t.offset + head, false);
    }

    fn step_machine(&mut self, controller: &Controller, machine: usize, dt: f32) {
        let def = &controller.machines[machine];
        if def.states.is_empty() {
            return;
        }
        let current = self.machines[machine].current;
        if let Some((target, time)) = self.asked[machine].take() {
            // A state asked for outright replaces whatever was playing and
            // has run none of its time when the update ends.
            let state = &mut self.machines[machine];
            let entering = state.next.take();
            let held = state.held.take().map_or(Vec::new(), |held| held.others);
            state.current = Playing { state: target, time, previous: time };
            state.blend = None;
            let gone = held.iter().map(|(held, _)| (held.state, held.time));
            let gone: Vec<(usize, f32)> = gone.chain(entering.map(|next| (next.state, next.time))).chain([(current.state, current.time)]).collect();
            for (left, at) in gone {
                self.events.push(Event::Left { machine, state: left });
                self.look(controller, machine, left, LookKind::Exit, at, false);
            }
            self.events.push(Event::Entered { machine, state: target });
            self.look(controller, machine, target, LookKind::Enter, time, false);
            return;
        }
        let length = self.state_length(controller, def, current.state);
        // A state held still by a transition that was cut short stays put.
        let held = self.machines[machine].held.is_some();
        let step = if length.is_finite() && !held { dt / length } else { 0.0 };
        let end = current.time + step;

        if let (Some(blend), Some(next)) = (self.machines[machine].blend, self.machines[machine].next) {
            // A transition under way can be cut short by another from the
            // side it allows. States held still start nothing.
            let from_source = matches!(blend.interruption, 1 | 3 | 4) && !held;
            let from_target = matches!(blend.interruption, 2 | 3 | 4);
            let next_length = self.state_length(controller, def, next.state);
            let next_end = next.time + if next_length.is_finite() { dt / next_length } else { 0.0 };
            let by_source = || {
                from_source
                    .then(|| self.triggered(def, current.state, current.time, end))
                    .flatten()
                    .filter(|(_, target, _)| *target != next.state)
                    .map(|(t, target, left)| (t, target, left, true))
            };
            let by_target = || {
                from_target
                    .then(|| self.triggered(def, next.state, next.time, next_end))
                    .flatten()
                    .map(|(t, target, left)| (t, target, left, false))
            };
            // The setting also says which side is asked first.
            let cut = if blend.interruption == 4 { by_target().or_else(by_source) } else { by_source().or_else(by_target) };
            if let Some((t, target, left, from_source)) = cut {
                let state = &mut self.machines[machine];
                if from_source {
                    // The state being entered is dropped.
                    self.events.push(Event::Left { machine, state: next.state });
                    state.next = None;
                    state.blend = None;
                    self.look(controller, machine, next.state, LookKind::Exit, next.time, true);
                    self.start(controller, machine, &t, target, left * dt, end);
                } else {
                    // The blend as it stands is held still, and the new
                    // state is blended into out of that. The state being
                    // left goes on being the current one.
                    self.cut_short(controller, machine, &t, target, left * dt, end);
                }
                return;
            }

            let total = self.blend_length(machine, &blend);
            let state = &mut self.machines[machine];
            state.current.previous = current.time;
            state.current.time = end;
            state.next = Some(Playing { state: next.state, time: next_end, previous: next.time });
            let elapsed = blend.elapsed + dt;
            if elapsed >= total {
                state.current = state.next.take().unwrap();
                state.blend = None;
                // Whatever was held still goes with the state left.
                let others = state.held.take().map_or(Vec::new(), |held| held.others);
                self.events.extend(others.iter().map(|(held, _)| Event::Left { machine, state: held.state }));
                self.events.push(Event::Left { machine, state: current.state });
                for (held, _) in &others {
                    self.look(controller, machine, held.state, LookKind::Exit, held.time, false);
                }
                self.look(controller, machine, current.state, LookKind::Exit, end, false);
                self.look(controller, machine, next.state, LookKind::Update, next_end, false);
            } else {
                state.blend = Some(Blend { elapsed, ..blend });
                self.look(controller, machine, current.state, LookKind::Update, end, true);
                self.look(controller, machine, next.state, LookKind::Update, next_end, false);
            }
            return;
        }

        match self.triggered(def, current.state, current.time, end) {
            Some((t, target, left)) => self.start(controller, machine, &t, target, left * dt, end),
            None => {
                let state = &mut self.machines[machine];
                state.current.previous = current.time;
                state.current.time = end;
                self.look(controller, machine, current.state, LookKind::Update, end, false);
            }
        }
    }

    /// What each clip contributes to a layer this frame.
    fn shares(&self, controller: &Controller, layer: &LayerDef) -> Vec<Share> {
        let def = &controller.machines[layer.machine];
        let state = &self.machines[layer.machine];
        let blend = self.blend_weight(layer.machine);
        let mut out = Vec::new();
        // The state being left, with whatever is held still beside it, and
        // the state being entered.
        let others = state.held.as_ref().map_or(&[][..], |held| &held.others[..]);
        let beside: f32 = others.iter().map(|(_, share)| share).sum();
        // A current state that is held is drawn where it was held, not
        // where its own clock has since been reported.
        let current = match &state.held {
            Some(held) => Playing { time: held.current_at, previous: held.current_at, ..state.current },
            None => state.current,
        };
        let mut playing = vec![(Some(current), (1.0 - blend) * (1.0 - beside).max(0.0))];
        playing.extend(others.iter().map(|(held, share)| (Some(*held), (1.0 - blend) * share)));
        playing.push((state.next, blend));
        for (playing, weight) in playing {
            let Some(playing) = playing.filter(|_| weight > 0.0) else { continue };
            // A layer may have no state at all (a weapon's first layer).
            let Some(s) = def.states.get(playing.state) else { continue };
            let Some(tree) = s.trees.get(layer.slot).and_then(Option::as_ref) else { continue };
            let mut leaves = Vec::new();
            self.leaves(tree, 0, 1.0, 1.0, &mut leaves);
            for (clip, leaf_weight, _, offset) in leaves {
                out.push(Share {
                    clip,
                    weight: weight * leaf_weight,
                    previous: playing.previous,
                    now: playing.time,
                    offset: offset + s.cycle_offset,
                });
            }
        }
        out
    }

    /// Works out the root's movement, the curve-driven parameters and the
    /// bones' pose from where the machines now stand; and, where it was
    /// asked for, the pose for drawing, which nothing else is taken from.
    fn evaluate(&mut self, controller: &Controller) {
        let (pose, curves, mut root, mut turn) = self.worked_out(controller, false);
        if !controller.scaled.is_empty() {
            self.scales = self.scaled(controller);
        }
        // A parameter that clips drive holds its default when none of the
        // clips playing carries its curve.
        for (parameter, driven) in controller.curve_driven.iter().enumerate() {
            if *driven {
                self.values[parameter] = curves[parameter].unwrap_or(controller.parameters[parameter].default);
            }
        }
        // A state can say that its animation is not to move the character.
        let machine = controller.layers.first().map_or(0, |l| l.machine);
        // States held still go on moving the root as they last did, by
        // what is left of their share.
        if let Some(Held { motion: (travel, turned), .. }) = self.machines.get(machine).and_then(|m| m.held.as_ref()) {
            let share = 1.0 - self.blend_weight(machine);
            for k in 0..3 {
                root[k] += travel[k] * self.step * share;
            }
            turn += turned * self.step * share;
        }
        let still = controller.machines.get(machine).is_some_and(|def| {
            def.states
                .get(self.machines[machine].current.state)
                .and_then(|s| s.movement.as_ref())
                .is_some_and(|m| m.disable_root_motion)
        });
        self.root_delta = if still { [0.0; 3] } else { root };
        self.root_turn = if still { 0.0 } else { turn };
        self.pose = pose;
        self.drawn = self.smooth.then(|| self.worked_out(controller, true).0);
    }

    /// The scale of each bone of the controller's `scaled`, from where the
    /// machines now stand: blended as a place is, layer over layer (an
    /// additive layer adds what its clips scale from their reference pose).
    fn scaled(&mut self, controller: &Controller) -> Vec<[f32; 3]> {
        let mut scales: Vec<[f32; 3]> = controller.scaled.iter().map(|i| controller.bones[*i].scale.unwrap_or([1.0; 3])).collect();
        let mut samples = std::mem::take(&mut self.samples);
        for (index, layer) in controller.layers.iter().enumerate() {
            let layer_weight = self.weights[index];
            if layer_weight <= 0.0 {
                continue;
            }
            let shares = self.shares(controller, layer);
            let total: f32 = shares.iter().map(|s| s.weight).sum();
            if total <= 0.0 {
                continue;
            }
            let mut sums = vec![[0.0f32; 3]; scales.len()];
            for share in &shares {
                let def = &controller.clips[share.clip];
                let length = def.clip.duration();
                let t = share.now + share.offset;
                def.clip.sample(if def.clip.looping { t.rem_euclid(1.0) * length } else { t.clamp(0.0, 1.0) * length }, &mut samples);
                let reference = &def.clip.reference_pose;
                for (j, i) in controller.scaled.iter().enumerate() {
                    if !layer.moves[*i] {
                        continue;
                    }
                    let rest = controller.bones[*i].scale.unwrap_or([1.0; 3]);
                    let read = |values: &[f32]| def.bones[*i].scale.map_or(rest, |c| [values[c], values[c + 1], values[c + 2]]);
                    let s = read(&samples);
                    if layer.additive {
                        if reference.len() == samples.len() {
                            let s0 = read(reference);
                            (0..3).for_each(|k| sums[j][k] += (s[k] - s0[k]) * share.weight);
                        }
                    } else {
                        (0..3).for_each(|k| sums[j][k] += s[k] * share.weight);
                    }
                }
            }
            let amount = (layer_weight * total).clamp(0.0, 1.0);
            for (j, i) in controller.scaled.iter().enumerate() {
                if !layer.moves[*i] {
                    continue;
                }
                for k in 0..3 {
                    if layer.additive {
                        scales[j][k] += sums[j][k] * layer_weight;
                    } else if index == 0 {
                        scales[j][k] = sums[j][k] / total;
                    } else {
                        scales[j][k] += (sums[j][k] / total - scales[j][k]) * amount;
                    }
                }
            }
        }
        self.samples = samples;
        scales
    }

    /// The bones' pose, the parameters the clips' curves give and the
    /// root's travel and turn, from where the machines now stand. With
    /// `smooth`, the clips' sampled curves are taken on curves without
    /// corners: for the pose that is drawn, and for nothing else.
    #[allow(clippy::type_complexity)]
    fn worked_out(&mut self, controller: &Controller, smooth: bool) -> (Vec<([f32; 3], [f32; 4])>, Vec<Option<f32>>, [f32; 3], f32) {
        let bone_count = controller.bones.len();
        let mut pose: Vec<([f32; 3], [f32; 4])> = controller.bones.iter().map(|b| (b.position, b.rotation)).collect();
        let mut curves: Vec<Option<f32>> = vec![None; controller.parameters.len()];
        let mut root = [0.0f32; 3];
        let mut turn = 0.0f32;
        let mut samples = std::mem::take(&mut self.samples);

        for (index, layer) in controller.layers.iter().enumerate() {
            let layer_weight = self.weights[index];
            if layer_weight <= 0.0 {
                continue;
            }
            let shares = self.shares(controller, layer);
            let total: f32 = shares.iter().map(|s| s.weight).sum();
            if total <= 0.0 {
                continue;
            }
            // The root follows the first layer alone, unless its state says
            // the animation is not to move the character.
            let moves_root = index == 0;
            let mut position = vec![[0.0f32; 3]; bone_count];
            let mut rotation = vec![[0.0f32; 4]; bone_count];
            let mut layer_curves: Vec<(f32, f32)> = vec![(0.0, 0.0); controller.parameters.len()];
            for share in &shares {
                let def = &controller.clips[share.clip];
                let length = def.clip.duration();
                // The clip's events its time went over on this update.
                if !smooth && share.weight > 0.0 && length > 0.0 && share.now > share.previous {
                    let (from, to) = (share.previous + share.offset, share.now + share.offset);
                    for (n, event) in def.clip.events.iter().enumerate() {
                        let at = event.time / length;
                        let passed = if def.clip.looping {
                            (to - at).floor() > (from - at).floor()
                        } else {
                            from.clamp(0.0, 1.0) < at && at <= to.clamp(0.0, 1.0)
                        };
                        if passed {
                            self.passed.push((share.clip, n));
                        }
                    }
                }
                let at = |cycles: f32| {
                    let t = cycles + share.offset;
                    if def.clip.looping { t.rem_euclid(1.0) * length } else { t.clamp(0.0, 1.0) * length }
                };
                if let (true, Some(path)) = (moves_root, def.root_path) {
                    // Whole cycles count by the clip's start-to-end
                    // displacement; the rest by the path between the two times.
                    let value = |samples: &mut Vec<f32>, time: f32| -> [f32; 3] {
                        def.clip.sample(time, samples);
                        path.map(|c| samples[c])
                    };
                    let (first, last) = (value(&mut samples, 0.0), value(&mut samples, length));
                    let (from, to) = (value(&mut samples, at(share.previous)), value(&mut samples, at(share.now)));
                    let cycles = if def.clip.looping {
                        (share.now + share.offset).floor() - (share.previous + share.offset).floor()
                    } else {
                        0.0
                    };
                    for k in 0..3 {
                        if !stays_in_pose(&def.clip, k) {
                            root[k] += share.weight * (to[k] - from[k] + cycles * (last[k] - first[k]));
                        }
                    }
                }
                if let (true, Some(curves)) = (moves_root, def.root_turn) {
                    // The root's turning about the vertical, the same way.
                    let mut bearing = |time: f32| -> f32 {
                        def.clip.sample(time, &mut samples);
                        2.0 * samples[curves[1]].atan2(samples[curves[3]]).to_degrees()
                    };
                    let whole = bearing(length) - bearing(0.0);
                    let cycles = if def.clip.looping {
                        (share.now + share.offset).floor() - (share.previous + share.offset).floor()
                    } else {
                        0.0
                    };
                    let (from, to) = (bearing(at(share.previous)), bearing(at(share.now)));
                    turn += share.weight * (to - from + cycles * whole);
                }
                if smooth {
                    def.clip.sample_smooth(at(share.now), &mut samples);
                } else {
                    def.clip.sample(at(share.now), &mut samples);
                }
                let reference = &def.clip.reference_pose;
                for (parameter, curve) in &def.curves {
                    // On an additive layer a curve gives what it has moved
                    // from the clip's own reference pose, as a bone does.
                    let value = match (layer.additive, reference.len() == samples.len()) {
                        (false, _) => samples[*curve],
                        (true, true) => samples[*curve] - reference[*curve],
                        (true, false) => continue,
                    };
                    layer_curves[*parameter].0 += value * share.weight;
                    layer_curves[*parameter].1 += share.weight;
                }
                for (i, (bone, tracks)) in controller.bones.iter().zip(&def.bones).enumerate() {
                    if !layer.moves[i] {
                        continue;
                    }
                    let read = |values: &[f32]| {
                        let p = tracks.position.map_or(bone.position, |c| [values[c], values[c + 1], values[c + 2]]);
                        let q = match tracks.rotation {
                            Some((c, true)) => euler_to_quat([values[c], values[c + 1], values[c + 2]]),
                            Some((c, false)) => [values[c], values[c + 1], values[c + 2], values[c + 3]],
                            None => bone.rotation,
                        };
                        (p, q)
                    };
                    let (mut p, mut q) = read(&samples);
                    if let (true, Some(path)) = (controller.root_bone == Some(i), def.root_path) {
                        // The part of the root's travel that leaves as root
                        // motion comes off the bone; the rest stays in the pose.
                        for (k, curve) in path.iter().enumerate() {
                            if !stays_in_pose(&def.clip, k) {
                                p[k] -= samples[*curve];
                            }
                        }
                    }
                    if let (true, Some(c)) = (controller.root_bone == Some(i), def.root_turn) {
                        // So does the root's turning: the clip's bone turns
                        // with the character, and the character is turned
                        // by the root motion, so the pose is left facing
                        // the way the root now does.
                        let back = quat_normalize([-samples[c[0]], -samples[c[1]], -samples[c[2]], samples[c[3]]]);
                        p = quat_rotate(back, p);
                        q = quat_mul(back, q);
                    }
                    if layer.additive {
                        // What the clip adds is its difference from its
                        // own reference pose; a clip without one adds nothing.
                        if reference.len() != samples.len() {
                            continue;
                        }
                        let (p0, q0) = read(reference);
                        // The clips' differences are blended with one
                        // another first, like the clips of any blend tree,
                        // and what comes of that is added to the pose once
                        // (`body-animator.md`, "Layers"; settled by the
                        // recordings: added one by one, each by its own
                        // weight, the ribcage was up to a centimetre out
                        // wherever the view was both pitched and turned
                        // from the body, and more the steeper the pitch).
                        let delta = quat_mul([-q0[0], -q0[1], -q0[2], q0[3]], q);
                        let side = if delta[3] < 0.0 { -1.0 } else { 1.0 };
                        for k in 0..3 {
                            position[i][k] += (p[k] - p0[k]) * share.weight;
                        }
                        for k in 0..4 {
                            rotation[i][k] += delta[k] * share.weight * side;
                        }
                        continue;
                    }
                    // Rotations are summed on the same side of the sphere
                    // and normalised afterwards.
                    let dot: f32 = (0..4).map(|k| rotation[i][k] * q[k]).sum();
                    let side = if dot < 0.0 { -1.0 } else { 1.0 };
                    for k in 0..3 {
                        position[i][k] += p[k] * share.weight;
                    }
                    for k in 0..4 {
                        rotation[i][k] += q[k] * share.weight * side;
                    }
                }
            }
            if layer.additive {
                // What the layer's clips add between them: their blended
                // difference, with no turn standing in for whatever share
                // of the layer no clip with a reference pose fills.
                let filled: f32 = shares
                    .iter()
                    .filter(|s| {
                        let clip = &controller.clips[s.clip].clip;
                        clip.reference_pose.len() == clip.curve_count()
                    })
                    .map(|s| s.weight)
                    .sum();
                let unfilled = (1.0 - filled).max(0.0);
                for i in 0..bone_count {
                    if !layer.moves[i] {
                        continue;
                    }
                    for k in 0..3 {
                        pose[i].0[k] += position[i][k] * layer_weight;
                    }
                    let mut delta = rotation[i];
                    delta[3] += unfilled;
                    pose[i].1 = quat_mul(pose[i].1, quat_lerp([0.0, 0.0, 0.0, 1.0], quat_normalize(delta), layer_weight));
                }
                // An additive layer's curves add to what the layers below
                // made of the parameter, by the layer's weight.
                for (parameter, (sum, covered)) in layer_curves.iter().enumerate() {
                    if *covered > 0.0 && *sum != 0.0 {
                        let below = curves[parameter].unwrap_or(controller.parameters[parameter].default);
                        curves[parameter] = Some(below + sum * layer_weight);
                    }
                }
            }
            if !layer.additive {
                // An override layer replaces what is below it by its
                // weight, and only as far as its clips cover the frame.
                let amount = (layer_weight * total).clamp(0.0, 1.0);
                for i in 0..bone_count {
                    if !layer.moves[i] {
                        continue;
                    }
                    let p = position[i].map(|v| v / total);
                    let q = quat_normalize(rotation[i]);
                    if index == 0 {
                        pose[i] = (p, q);
                    } else {
                        for k in 0..3 {
                            pose[i].0[k] += (p[k] - pose[i].0[k]) * amount;
                        }
                        pose[i].1 = quat_lerp(pose[i].1, q, amount);
                    }
                }
                // Curves take no notice of masks.
                for (parameter, (sum, covered)) in layer_curves.iter().enumerate() {
                    if *covered <= 0.0 {
                        continue;
                    }
                    let value = sum / covered;
                    curves[parameter] = Some(match curves[parameter] {
                        Some(below) if index > 0 => below + (value - below) * (layer_weight * covered).clamp(0.0, 1.0),
                        // On the first layer a clip without the curve
                        // stands for the parameter's default.
                        _ => sum + (total - covered).max(0.0) * controller.parameters[parameter].default,
                    });
                }
            }
        }

        self.samples = samples;
        (pose, curves, root, turn)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use trp_data::states::Parameter;

    fn state(name: &str, transitions: Vec<TransitionDef>) -> StateDef {
        StateDef {
            name: name.to_owned(),
            speed: 1.0,
            speed_parameter: None,
            looping: false,
            cycle_offset: 0.0,
            // No motion: such a state lasts one second.
            trees: vec![None],
            transitions,
            movement: None,
            loaded: true,
        }
    }

    fn to(target: Target, conditions: Vec<Condition>) -> TransitionDef {
        TransitionDef { target, duration: 0.0, fixed: true, offset: 0.0, exit_time: None, conditions, interruption: 0 }
    }

    fn when(parameter: usize, mode: i32, threshold: f32) -> Condition {
        Condition { parameter: Some(parameter), mode, threshold }
    }

    /// One layer over the given states, with a boolean `go`, a trigger
    /// `fire` and a float `level`.
    fn animator(states: Vec<StateDef>, any_state: Vec<TransitionDef>, selectors: Vec<Vec<(Target, Vec<Condition>)>>) -> Animator {
        let parameters = vec![
            Parameter { name: "go".into(), kind: ParameterKind::Bool, default: 0.0 },
            Parameter { name: "fire".into(), kind: ParameterKind::Trigger, default: 0.0 },
            Parameter { name: "level".into(), kind: ParameterKind::Float, default: 0.0 },
        ];
        let by_name = parameters.iter().enumerate().map(|(i, p)| (p.name.clone(), i)).collect();
        let controller = Controller {
            curve_driven: vec![false; parameters.len()],
            parameters,
            by_name,
            machines: vec![MachineDef { states, any_state, selectors, default_state: 0 }],
            layers: vec![LayerDef { name: "base".into(), machine: 0, slot: 0, default_weight: 1.0, additive: false, moves: Vec::new() }],
            clips: Vec::new(),
            bones: Vec::new(),
            root_bone: None,
            scaled: Vec::new(),
        };
        Animator::new(Arc::new(controller))
    }

    const DT: f32 = 0.05;

    #[test]
    fn a_transition_waits_for_its_moment_and_then_blends() {
        // From the first state to the second, half way through the first,
        // blending for a fifth of a second.
        let wait = TransitionDef { exit_time: Some(0.5), duration: 0.2, ..to(Target::State(1), Vec::new()) };
        let mut a = animator(vec![state("first", vec![wait]), state("second", Vec::new())], Vec::new(), Vec::new());
        let (mut entered, mut left) = (None, None);
        for step in 1..=20 {
            for event in a.update(DT) {
                match event {
                    Event::Entered { state: 1, .. } => entered = Some(step as f32 * DT),
                    Event::Left { state: 0, .. } => left = Some(step as f32 * DT),
                    _ => {}
                }
            }
            // While the two blend, both are reported and the weight rises.
            if entered.is_some() && left.is_none() {
                assert_eq!(a.state(0).1.map(|s| s.name.as_str()), Some("second"));
                assert!(a.progress(0).1 < 1.0);
            }
        }
        // The new state is entered when the blend starts and the old one
        // left when it ends.
        assert!((entered.unwrap() - 0.5).abs() <= DT, "{entered:?}");
        assert!((left.unwrap() - entered.unwrap() - 0.2).abs() <= DT, "{left:?}");
        assert_eq!(a.state(0).0.name, "second");
    }

    #[test]
    fn conditions_hold_a_transition_back_and_a_trigger_is_used_up() {
        let fire = to(Target::State(1), vec![when(1, 1, 0.0)]);
        let back = to(Target::State(0), vec![when(0, 2, 0.0), when(2, 3, 0.5)]);
        let mut a = animator(vec![state("rest", vec![fire]), state("act", vec![back])], Vec::new(), Vec::new());
        a.update(DT);
        assert_eq!(a.state(0).0.name, "rest");
        a.set_bool("fire", true);
        a.update(DT);
        assert_eq!(a.state(0).0.name, "act");
        assert_eq!(a.float("fire"), 0.0, "the trigger was not cleared");
        // Back needs the flag down and the level above a half; both hold only now.
        a.update(DT);
        assert_eq!(a.state(0).0.name, "act");
        a.set_float("level", 0.75);
        a.update(DT);
        assert_eq!(a.state(0).0.name, "rest");
    }

    #[test]
    fn a_way_from_any_state_can_pass_through_a_choice_of_entries() {
        // From anywhere, when `go` is set: to an entry point that picks the
        // third state at a high level and the second otherwise.
        let any = to(Target::Selector(0), vec![when(0, 1, 0.0)]);
        let choice = vec![(Target::State(2), vec![when(2, 3, 0.5)]), (Target::State(1), Vec::new())];
        let states = || vec![state("rest", Vec::new()), state("low", Vec::new()), state("high", Vec::new())];
        let mut a = animator(states(), vec![any.clone()], vec![choice.clone()]);
        a.set_bool("go", true);
        a.update(DT);
        assert_eq!(a.state(0).0.name, "low");
        // Already there, the same way does not lead back into it.
        assert!(a.update(DT).is_empty());

        let mut a = animator(states(), vec![any], vec![choice]);
        a.set_bool("go", true);
        a.set_float("level", 1.0);
        a.update(DT);
        assert_eq!(a.state(0).0.name, "high");
    }

    #[test]
    fn a_layer_is_taken_up_where_a_recording_has_it() {
        // A blend under way, as a run begins.
        let blend = TransitionDef { duration: 0.4, ..to(Target::State(1), Vec::new()) };
        let mut a = animator(vec![state("rest", vec![blend]), state("Base Layer.Breath", Vec::new())], Vec::new(), Vec::new());
        a.update(DT);
        assert!(a.state(0).1.is_some());
        // The recorder writes the low 24 bits of the hash of the state's
        // own name, the name without its layer's.
        let hash = path_hash("Breath") & 0x00ff_ffff;
        assert_eq!(a.state_by_short_hash(0, hash), Some(1));
        assert_eq!(a.state_by_short_hash(0, hash ^ 1), None);
        // Put there outright: no blend left, and the next update plays on
        // from the figure given.
        a.start_in(0, 1, 3.7);
        assert_eq!(a.state(0).0.name, "Base Layer.Breath");
        assert!(a.state(0).1.is_none());
        assert_eq!(a.progress(0), (3.7, 0.0));
        a.update(DT);
        assert!((a.progress(0).0 - (3.7 + DT)).abs() < 1e-6, "{:?}", a.progress(0));
        assert!(a.owns_machine(0) && !a.owns_machine(1));
        assert_eq!(a.layer_count(), 1);
    }
}
