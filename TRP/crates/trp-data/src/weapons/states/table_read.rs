//! Reading the state table from an animator controller, and clips' lengths.

use super::*;

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
        // The scripts that set the third layer's weight, by their place in
        // the controller's list, and the states each is on (its
        // `m_StateMachineBehaviourVectorDescription`: a state by its full
        // path's id, a range of places in the list of indices).
        let mut weight_scripts: HashMap<usize, LayerWeight> = HashMap::new();
        for (index, b) in v.get("m_StateMachineBehaviours").and_then(Value::as_array).unwrap_or(&[]).iter().enumerate() {
            let Some(obj) = b.as_pptr().and_then(|p| assets.resolve_loaded(controller.file, p)) else { continue };
            let behaviour = assets.read(obj)?;
            if let (Some(start), Some(end), Some(curve)) = (
                behaviour.get("StartValue").and_then(Value::as_f32),
                behaviour.get("EndValue").and_then(Value::as_f32),
                behaviour.get("LayerWeight").and_then(|c| crate::character::player::curve(c).ok()),
            ) {
                let (before, after) = behaviour.get("LayerWeight").map(crate::character::player::wraps).unwrap_or_default();
                weight_scripts.insert(index, LayerWeight { start, end, curve, before, after });
                continue;
            }
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
                            blend: crate::character::player::curve(v.get("Blend")?).ok()?,
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
                        curve: behaviour
                            .get("SmoothLerpAnimCurve")
                            .and_then(|v| crate::character::player::curve(v).ok())
                            .filter(|curve| !curve.keys.is_empty()),
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
        let mut layer_weights: HashMap<String, Vec<LayerWeight>> = HashMap::new();
        if !weight_scripts.is_empty() {
            let description = v.get("m_StateMachineBehaviourVectorDescription");
            let indices: Vec<usize> = match description.and_then(|d| d.get("m_StateMachineBehaviourIndices")) {
                Some(Value::UInts(list)) => list.iter().map(|&i| i as usize).collect(),
                Some(Value::Ints(list)) => list.iter().map(|&i| i as usize).collect(),
                _ => Vec::new(),
            };
            for pair in description.and_then(|d| d.get("m_StateMachineBehaviourRanges")).and_then(Value::as_array).unwrap_or(&[]) {
                let (Some(id), Some(start), Some(count)) = (
                    pair.at("first.m_StateID").and_then(Value::as_u32),
                    pair.at("second.m_StartIndex").and_then(Value::as_usize),
                    pair.at("second.m_Count").and_then(Value::as_usize),
                ) else {
                    continue;
                };
                let Some(state) = names.get(&id) else { continue };
                for index in indices.iter().skip(start).take(count) {
                    if let Some(script) = weight_scripts.get(index) {
                        layer_weights.entry((*state).to_owned()).or_default().push(script.clone());
                    }
                }
            }
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
                    writes_defaults: s.get("m_WriteDefaultValues").and_then(Value::as_bool).unwrap_or(true),
                    cycle_offset: s.get("m_CycleOffset").and_then(Value::as_f32).unwrap_or(0.0),
                    movement: movement.get(*full_name).cloned(),
                    layer_weights: layer_weights.get(*full_name).cloned().unwrap_or_default(),
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
