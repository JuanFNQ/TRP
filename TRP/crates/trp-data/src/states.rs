//! The weapon animator's states: which clip each one plays and how fast,
//! and the timed events the game's weapon logic waits for.
//!
//! The states come from the animator controller. The events are the
//! game's own: each state carries a script naming a list in a table that
//! ships beside the controller.

use std::collections::HashMap;

use anyhow::{anyhow, Result};

use crate::unity::{Assets, ObjRef, Value};
use crate::weapon::Weapon;

/// A condition on an animator parameter that an event needs to fire.
#[derive(Debug, Clone)]
pub struct EventCondition {
    pub parameter: String,
    /// 0 a whole number, 1 a float, 2 a flag, as the game numbers them
    /// (`EEventConditionParamTypes`).
    pub kind: i32,
    /// 0 equal, 1 not equal, 2 greater, 3 less, 4 at least, 5 at most
    /// (`EEventConditionModes`).
    pub mode: i32,
    pub float_value: f32,
    pub int_value: i32,
    pub bool_value: bool,
}

#[derive(Debug, Clone)]
pub struct StateEvent {
    pub name: String,
    /// When in the state it fires, as a fraction of the state's length.
    pub time: f32,
    pub int_param: i32,
    pub string_param: String,
    pub enabled: bool,
    pub conditions: Vec<EventCondition>,
}

/// One test of an animator parameter on a transition.
#[derive(Debug, Clone)]
pub struct Condition {
    pub parameter: String,
    /// Unity's numbering: 1 if true, 2 if false, 3 greater, 4 less,
    /// 6 equals, 7 not equal.
    pub mode: i32,
    pub threshold: f32,
}

/// What kind of value an animator parameter holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParameterKind {
    Float,
    Int,
    Bool,
    /// A boolean that the transition using it clears.
    Trigger,
}

#[derive(Debug, Clone)]
pub struct Parameter {
    pub name: String,
    pub kind: ParameterKind,
    /// The value it starts with; 0 or 1 for a boolean or a trigger.
    pub default: f32,
}

/// One of the choices at the entry or exit of a sub-state machine.
#[derive(Debug, Clone)]
pub struct SelectorChoice {
    /// The state it leads to, if it leads to a plain state.
    pub to: Option<String>,
    /// Or the further entry or exit point it leads to, by its place in
    /// the machine's list.
    pub to_selector: Option<usize>,
    pub conditions: Vec<Condition>,
}

/// The entry or exit point of a sub-state machine: choices tried in order.
#[derive(Debug, Clone)]
pub struct Selector {
    pub entry: bool,
    pub choices: Vec<SelectorChoice>,
}

/// The game's script on a body state that names the movement state the
/// player is in while the animator is in that state.
#[derive(Debug, Clone)]
pub struct MovementBehaviour {
    /// The game's own number for the movement state.
    pub state: i32,
    pub default_state: bool,
    pub disable_root_motion: bool,
    pub rotation_speed_clamp: f32,
    pub sensitivity: f32,
    /// Whether the player may look at a thing to work it while in this
    /// state (`CanInteract`; interactions spec, "What is looked at").
    pub can_interact: bool,
    pub animation_authority: f32,
    pub authority_speed: f32,
    /// Which part of a movement the state is, for movements that have a
    /// way in, a hold and a way out (the game's `EStateType`: 0 none or
    /// the hold, 1 in, 2 out).
    pub kind: i32,
    /// The side or direction the state is for (the game's
    /// `EMovementDirection`: 0 none, 1 right, 5 left).
    pub direction: i32,
    /// The length the game has stored for the state, in seconds.
    pub length: f32,
    /// The state gets a movement-state object of its own, and does not
    /// share the one its movement state's other animator states use.
    pub unique: bool,
    /// What the state does to the first-person weapon and camera while the
    /// animator is in it, if anything.
    pub first_person_pose: Option<FirstPersonPose>,
}

/// An offset the weapon's procedural motion blends in while a body state
/// is the one entered last: added to the rest of the weapon's position and
/// rotation springs and of the camera's.
#[derive(Debug, Clone)]
pub struct FirstPersonPose {
    pub name: String,
    pub position: [f32; 3],
    pub rotation: [f32; 3],
    pub camera_rotation: [f32; 3],
    pub camera_position: [f32; 3],
    /// How much of it applies, against the time it has been coming in.
    pub blend: crate::curve::Curve,
}

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
    /// Where in its cycle the state starts, as a fraction.
    pub cycle_offset: f32,
    /// The movement state it stands for, on the body's controller.
    pub movement: Option<MovementBehaviour>,
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

impl StateTable {
    /// The states of a weapon's animator.
    pub fn load(assets: &Assets, weapon: &Weapon) -> Result<Self> {
        let controller = weapon
            .prefab
            .req("_originalAnimatorController")?
            .as_pptr()
            .and_then(|p| assets.resolve_loaded(weapon.prefab_file, p))
            .ok_or_else(|| anyhow!("weapon has no animator controller"))?;
        let overrides: HashMap<String, ObjRef> = weapon.clips(assets)?.into_iter().collect();
        Self::from_controller(assets, controller, &overrides)
    }

    /// The states of any animator controller. `overrides` replaces clips
    /// by name, as an override controller does.
    pub fn from_controller(assets: &Assets, mut controller: ObjRef, overrides: &HashMap<String, ObjRef>) -> Result<Self> {
        let mut v = assets.read(controller)?;
        // An override controller swaps clips on a base it points to; the
        // states are the base's.
        while let Some(base) =
            v.get("m_Controller").and_then(Value::as_pptr).and_then(|p| assets.resolve_loaded(controller.file, p))
        {
            controller = base;
            v = assets.read(controller)?;
        }

        let names: HashMap<u32, &str> = v
            .req("m_TOS")?
            .as_array()
            .unwrap_or(&[])
            .iter()
            .filter_map(|pair| Some((pair.get("first")?.as_u32()?, pair.get("second")?.as_str()?)))
            .collect();
        let clips: Vec<Option<(String, ObjRef)>> = v
            .req("m_AnimationClips")?
            .as_array()
            .unwrap_or(&[])
            .iter()
            .map(|ptr| {
                let obj = assets.resolve_loaded(controller.file, ptr.as_pptr()?)?;
                let name = assets.name_of(obj)?;
                let obj = overrides.get(&name).copied().unwrap_or(obj);
                Some((name, obj))
            })
            .collect();

        let mut events: HashMap<String, Vec<StateEvent>> = HashMap::new();
        let mut movement: HashMap<String, MovementBehaviour> = HashMap::new();
        let mut tables: HashMap<ObjRef, Value> = HashMap::new();
        for b in v.get("m_StateMachineBehaviours").and_then(Value::as_array).unwrap_or(&[]) {
            let Some(obj) = b.as_pptr().and_then(|p| assets.resolve_loaded(controller.file, p)) else { continue };
            let behaviour = assets.read(obj)?;
            // The body's states name the movement state they stand for.
            if let (Some(state), Some(name)) = (
                behaviour.get("StateFullName").and_then(Value::as_str),
                behaviour.get("Name").and_then(Value::as_i64),
            ) {
                let flag = |field: &str| behaviour.get(field).and_then(Value::as_bool).unwrap_or(false);
                let number = |field: &str, or: f32| behaviour.get(field).and_then(Value::as_f32).unwrap_or(or);
                let first_person_pose = behaviour
                    .get("FirstPersonPose")
                    .and_then(Value::as_pptr)
                    .and_then(|p| assets.resolve_loaded(obj.file, p))
                    .and_then(|pose| {
                        let v = assets.read(pose).ok()?;
                        let vector = |field: &str| v.get(field).and_then(Value::as_vec3).unwrap_or_default();
                        Some(FirstPersonPose {
                            name: assets.name_of(pose).unwrap_or_default(),
                            position: vector("Position"),
                            rotation: vector("Rotation"),
                            camera_rotation: vector("CameraRotation"),
                            camera_position: vector("CameraPosition"),
                            blend: crate::player::curve(v.get("Blend")?).ok()?,
                        })
                    });
                movement.insert(
                    state.to_owned(),
                    MovementBehaviour {
                        state: name as i32,
                        default_state: flag("IsDefaultState"),
                        disable_root_motion: flag("DisableRootMotion"),
                        rotation_speed_clamp: number("RotationSpeedClamp", 99.0),
                        sensitivity: number("StateSensitivity", 1.0),
                        can_interact: flag("CanInteract"),
                        animation_authority: number("AnimationAuthority", 0.0),
                        authority_speed: number("AuthoritySpeed", 4.0),
                        kind: behaviour.get("Type").and_then(Value::as_i64).unwrap_or(0) as i32,
                        direction: behaviour.get("AdditionalDirectionInfo").and_then(Value::as_i64).unwrap_or(0) as i32,
                        length: number("StateLength", 0.0),
                        unique: flag("CreateUniqueMovementStateObject"),
                        first_person_pose,
                    },
                );
                continue;
            }
            let (Some(state), Some(list), Some(table)) = (
                behaviour.get("FullName").and_then(Value::as_str),
                behaviour.get("EventsListId").and_then(Value::as_i64),
                behaviour.get("EventsData").and_then(Value::as_pptr).and_then(|p| assets.resolve_loaded(obj.file, p)),
            ) else {
                continue;
            };
            if !tables.contains_key(&table) {
                tables.insert(table, assets.read(table)?);
            }
            let lists = tables[&table].req("_stateHashToEventsCollection")?.as_array().unwrap_or(&[]);
            let Some(list) = usize::try_from(list).ok().and_then(|i| lists.get(i)) else { continue };
            let parsed = list
                .req("_animationEvents")?
                .as_array()
                .unwrap_or(&[])
                .iter()
                .map(event)
                .collect::<Result<Vec<_>>>()?;
            events.entry(state.to_owned()).or_default().extend(parsed);
        }

        let name_of = |id: u32| names.get(&id).map_or_else(|| format!("#{id:08x}"), |n| (*n).to_owned());

        // Parameters, each with its place in a list of defaults of its kind.
        let defaults = v.req("m_Controller.m_DefaultValues.data")?;
        let default_floats = defaults.get("m_FloatValues").and_then(Value::as_floats).unwrap_or(&[]);
        let default_ints: Vec<i32> = match defaults.get("m_IntValues") {
            Some(Value::Ints(list)) => list.clone(),
            Some(Value::UInts(list)) => list.iter().map(|i| *i as i32).collect(),
            _ => Vec::new(),
        };
        let default_bools = defaults.get("m_BoolValues").and_then(Value::as_bytes).unwrap_or(&[]);
        let mut parameters = Vec::new();
        for p in v.req("m_Controller.m_Values.data.m_ValueArray")?.as_array().unwrap_or(&[]) {
            let (Some(id), Some(kind), Some(index)) = (
                p.get("m_ID").and_then(Value::as_u32),
                p.get("m_Type").and_then(Value::as_i64),
                p.get("m_Index").and_then(Value::as_usize),
            ) else {
                continue;
            };
            // The engine's numbering of value kinds.
            let (kind, default) = match kind {
                1 => (ParameterKind::Float, default_floats.get(index).copied().unwrap_or(0.0)),
                3 => (ParameterKind::Int, default_ints.get(index).copied().unwrap_or(0) as f32),
                4 => (ParameterKind::Bool, default_bools.get(index).copied().unwrap_or(0) as f32),
                9 => (ParameterKind::Trigger, default_bools.get(index).copied().unwrap_or(0) as f32),
                _ => continue,
            };
            parameters.push(Parameter { name: name_of(id), kind, default });
        }

        let conditions_of = |t: &Value| -> Vec<Condition> {
            t.get("m_ConditionConstantArray")
                .and_then(Value::as_array)
                .unwrap_or(&[])
                .iter()
                .filter_map(|c| {
                    let c = c.get("data")?;
                    Some(Condition {
                        parameter: name_of(c.get("m_EventID")?.as_u32()?),
                        mode: c.get("m_ConditionMode")?.as_i64()? as i32,
                        threshold: c.get("m_EventThreshold")?.as_f32()?,
                    })
                })
                .collect()
        };
        // A destination is a state by its place in the machine's list, or,
        // from this number on, an entry or exit point by its place in theirs.
        const FIRST_SELECTOR: usize = 30000;
        let destination = |index: Option<usize>, in_machine: &[Option<String>]| -> (Option<String>, Option<usize>) {
            match index {
                Some(i) if i >= FIRST_SELECTOR && i != u32::MAX as usize => (None, Some(i - FIRST_SELECTOR)),
                Some(i) => (in_machine.get(i).cloned().flatten(), None),
                None => (None, None),
            }
        };
        let transition = |t: &Value, in_machine: &[Option<String>]| -> Result<Transition> {
            let conditions = conditions_of(t);
            let (to, to_selector) = destination(t.req("m_DestinationState")?.as_usize(), in_machine);
            Ok(Transition {
                to,
                to_selector,
                interruption: t.get("m_InterruptionSource").and_then(Value::as_i64).unwrap_or(0) as i32,
                ordered_interruption: t.get("m_OrderedInterruption").and_then(Value::as_bool).unwrap_or(true),
                duration: t.req("m_TransitionDuration")?.as_f32().unwrap_or(0.0),
                fixed_duration: t.req("m_HasFixedDuration")?.as_bool().unwrap_or(false),
                offset: t.req("m_TransitionOffset")?.as_f32().unwrap_or(0.0),
                exit_time: t
                    .req("m_HasExitTime")?
                    .as_bool()
                    .unwrap_or(false)
                    .then(|| t.get("m_ExitTime").and_then(Value::as_f32).unwrap_or(1.0)),
                conditions,
            })
        };

        let mut states = Vec::new();
        let mut any_state = Vec::new();
        let mut selectors = Vec::new();
        let machines = v.req("m_Controller.m_StateMachineArray")?.as_array().unwrap_or(&[]);
        for (machine, m) in machines.iter().enumerate() {
            let constants = m.req("data.m_StateConstantArray")?.as_array().unwrap_or(&[]);
            // Transitions name their destination by its place in this list.
            let in_machine: Vec<Option<String>> = constants
                .iter()
                .map(|s| {
                    let id = s.at("data.m_FullPathID")?.as_u32()?;
                    names.get(&id).map(|n| (*n).to_owned())
                })
                .collect();
            for t in m.at("data.m_AnyStateTransitionConstantArray").and_then(Value::as_array).unwrap_or(&[]) {
                any_state.push((machine, transition(t.req("data")?, &in_machine)?));
            }
            let machine_selectors: Vec<Selector> = m
                .at("data.m_SelectorStateConstantArray")
                .and_then(Value::as_array)
                .unwrap_or(&[])
                .iter()
                .filter_map(|s| s.get("data"))
                .map(|s| Selector {
                    entry: s.get("m_IsEntry").and_then(Value::as_bool).unwrap_or(false),
                    choices: s
                        .get("m_TransitionConstantArray")
                        .and_then(Value::as_array)
                        .unwrap_or(&[])
                        .iter()
                        .filter_map(|t| t.get("data"))
                        .map(|t| {
                            let (to, to_selector) =
                                destination(t.get("m_Destination").and_then(Value::as_usize), &in_machine);
                            SelectorChoice { to, to_selector, conditions: conditions_of(t) }
                        })
                        .collect(),
                })
                .collect();
            selectors.push(machine_selectors);
            for s in constants {
                let s = s.req("data")?;
                let Some(full_name) = s.get("m_FullPathID").and_then(Value::as_u32).and_then(|id| names.get(&id))
                else {
                    continue;
                };
                let transitions = s
                    .req("m_TransitionConstantArray")?
                    .as_array()
                    .unwrap_or(&[])
                    .iter()
                    .map(|t| transition(t.req("data")?, &in_machine))
                    .collect::<Result<Vec<_>>>()?;
                let leaves: Vec<(String, ObjRef)> = s
                    .req("m_BlendTreeConstantArray")?
                    .as_array()
                    .unwrap_or(&[])
                    .iter()
                    .flat_map(|tree| tree.at("data.m_NodeArray").and_then(Value::as_array).unwrap_or(&[]))
                    .filter_map(|node| node.at("data.m_ClipID")?.as_u32())
                    .filter(|&id| id != u32::MAX)
                    .filter_map(|id| clips.get(id as usize).cloned().flatten())
                    .collect();
                // The first node of a blend tree names the parameter it blends over.
                let root = s
                    .req("m_BlendTreeConstantArray")?
                    .as_array()
                    .and_then(|trees| trees.first())
                    .and_then(|tree| tree.at("data.m_NodeArray")?.as_array()?.first())
                    .and_then(|node| node.get("data"));
                let blend_parameter = root
                    .filter(|_| leaves.len() > 1)
                    .and_then(|node| node.get("m_BlendEventID")?.as_u32())
                    .and_then(|id| names.get(&id))
                    .map(|name| (*name).to_owned());
                let thresholds = root
                    .and_then(|node| node.at("m_Blend1dData.data.m_ChildThresholdArray")?.as_floats())
                    .map(<[f32]>::to_vec)
                    .unwrap_or_default();
                let tree_of_layer: Vec<i32> = match s.get("m_BlendTreeConstantIndexArray") {
                    Some(Value::Ints(list)) => list.clone(),
                    Some(Value::UInts(list)) => list.iter().map(|i| *i as i32).collect(),
                    _ => Vec::new(),
                };
                let read_tree = |tree: &Value| -> Vec<TreeNode> {
                    tree.at("data.m_NodeArray")
                    .and_then(Value::as_array)
                    .unwrap_or(&[])
                    .iter()
                    .filter_map(|node| node.get("data"))
                    .map(|node| {
                        let parameter = |field: &str| {
                            node.get(field).and_then(Value::as_u32).filter(|id| *id != u32::MAX && *id != 0).map(name_of)
                        };
                        let floats = |path: &str| node.at(path).and_then(Value::as_floats).unwrap_or(&[]).to_vec();
                        let positions = node
                            .at("m_Blend2dData.data.m_ChildPositionArray")
                            .and_then(Value::as_array)
                            .unwrap_or(&[])
                            .iter()
                            .filter_map(|p| Some([p.get("x")?.as_f32()?, p.get("y")?.as_f32()?]))
                            .collect();
                        TreeNode {
                            blend_type: node.get("m_BlendType").and_then(Value::as_i64).unwrap_or(0) as i32,
                            parameter_x: parameter("m_BlendEventID"),
                            parameter_y: parameter("m_BlendEventYID"),
                            children: node
                                .get("m_ChildIndices")
                                .and_then(Value::as_uints)
                                .unwrap_or(&[])
                                .iter()
                                .map(|i| *i as usize)
                                .collect(),
                            thresholds: floats("m_Blend1dData.data.m_ChildThresholdArray"),
                            positions,
                            clip: node
                                .get("m_ClipID")
                                .and_then(Value::as_u32)
                                .filter(|id| *id != u32::MAX)
                                .and_then(|id| clips.get(id as usize).cloned().flatten()),
                            duration: node.get("m_Duration").and_then(Value::as_f32).unwrap_or(0.0),
                            cycle_offset: node.get("m_CycleOffset").and_then(Value::as_f32).unwrap_or(0.0),
                        }
                    })
                    .collect()
                };
                let trees: Vec<Vec<TreeNode>> =
                    s.req("m_BlendTreeConstantArray")?.as_array().unwrap_or(&[]).iter().map(read_tree).collect();
                let mut state_events = events.remove(*full_name).unwrap_or_default();
                state_events.sort_by(|a, b| a.time.total_cmp(&b.time));
                states.push(AnimState {
                    machine,
                    full_name: (*full_name).to_owned(),
                    speed: s.req("m_Speed")?.as_f32().unwrap_or(1.0),
                    speed_parameter: s
                        .get("m_SpeedParamID")
                        .and_then(Value::as_u32)
                        .filter(|id| *id != 0 && *id != u32::MAX)
                        .and_then(|id| names.get(&id))
                        .map(|name| (*name).to_owned()),
                    looping: s.req("m_Loop")?.as_bool().unwrap_or(false),
                    cycle_offset: s.get("m_CycleOffset").and_then(Value::as_f32).unwrap_or(0.0),
                    movement: movement.get(*full_name).cloned(),
                    clips: leaves,
                    blend_parameter,
                    thresholds,
                    events: state_events,
                    transitions,
                    trees,
                    tree_of_layer,
                });
            }
        }
        let mut layers = Vec::new();
        for (index, layer) in v.req("m_Controller.m_LayerArray")?.as_array().unwrap_or(&[]).iter().enumerate() {
            let layer = layer.req("data")?;
            let machine = layer.req("m_StateMachineIndex")?.as_usize().unwrap_or(index);
            let mask = layer
                .at("m_SkeletonMask.data.m_Data")
                .and_then(Value::as_array)
                .unwrap_or(&[])
                .iter()
                .filter_map(|e| Some((e.get("m_PathHash")?.as_u32()?, e.get("m_Weight")?.as_f32()?)))
                .collect();
            let default_index = machines
                .get(machine)
                .and_then(|m| m.at("data.m_DefaultState")?.as_usize());
            let in_machine: Vec<&AnimState> = states.iter().filter(|s| s.machine == machine).collect();
            layers.push(Layer {
                name: layer
                    .get("m_Binding")
                    .and_then(Value::as_u32)
                    .and_then(|id| names.get(&id))
                    .map_or_else(|| format!("layer {index}"), |n| (*n).to_owned()),
                machine,
                sync_slot: layer.get("m_StateMachineSynchronizedLayerIndex").and_then(Value::as_usize).unwrap_or(0),
                default_weight: layer.req("m_DefaultWeight")?.as_f32().unwrap_or(0.0),
                additive: layer.get("(int&)m_LayerBlendingMode").and_then(Value::as_i64).unwrap_or(0) == 1,
                mask,
                default_state: default_index.and_then(|i| in_machine.get(i)).map(|s| s.full_name.clone()),
            });
        }
        Ok(Self { parameters, states, layers, any_state, selectors })
    }

    pub fn find(&self, full_name: &str) -> Option<&AnimState> {
        self.states.iter().find(|s| s.full_name == full_name)
    }

    /// The clip a state plays while nothing has set the parameter its
    /// blend tree goes by: of the tree's clips, the one whose own value
    /// is nearest the value the controller starts that parameter with.
    /// For a state with one clip, that clip.
    pub fn clip_by_default(&self, full_name: &str) -> Option<&(String, ObjRef)> {
        let state = self.find(full_name)?;
        let start = state
            .blend_parameter
            .as_ref()
            .and_then(|name| self.parameters.iter().find(|p| &p.name == name))
            .map_or(0.0, |p| p.default);
        state.clip_at(start)
    }
}

/// Length of a clip in seconds, without decoding its curves.
pub fn clip_length(assets: &Assets, clip: ObjRef) -> Result<f32> {
    let v = assets.read(clip)?;
    let muscle = v.req("m_MuscleClip")?;
    let time = |name: &str| muscle.get(name).and_then(Value::as_f32).unwrap_or(0.0);
    Ok((time("m_StopTime") - time("m_StartTime")).max(0.0))
}

fn event(v: &Value) -> Result<StateEvent> {
    let parameter = v.req("Parameter")?;
    let mut conditions = Vec::new();
    for c in v.get("EventConditions").and_then(Value::as_array).unwrap_or(&[]) {
        conditions.push(EventCondition {
            parameter: c.req("ParameterName")?.as_str().unwrap_or("").to_owned(),
            kind: c.req("ConditionParamType")?.as_i64().unwrap_or(0) as i32,
            mode: c.req("ConditionMode")?.as_i64().unwrap_or(0) as i32,
            float_value: c.req("FloatValue")?.as_f32().unwrap_or(0.0),
            int_value: c.req("IntValue")?.as_i64().unwrap_or(0) as i32,
            bool_value: c.req("BoolValue")?.as_bool().unwrap_or(false),
        });
    }
    Ok(StateEvent {
        name: v.req("_functionName")?.as_str().unwrap_or("").to_owned(),
        time: v.req("_time")?.as_f32().unwrap_or(0.0),
        int_param: parameter.req("IntParam")?.as_i64().unwrap_or(0) as i32,
        string_param: parameter.req("StringParam")?.as_str().unwrap_or("").to_owned(),
        enabled: v.req("Enabled")?.as_bool().unwrap_or(true),
        conditions,
    })
}
