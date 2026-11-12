//! The animator controller loaded from the install and its node and clip definitions.

use super::*;

#[derive(Clone)]
pub struct Controller {
    pub parameters: Vec<states::Parameter>,
    pub(super) by_name: HashMap<String, usize>,
    pub machines: Vec<MachineDef>,
    pub layers: Vec<LayerDef>,
    pub clips: Vec<ClipDef>,
    pub bones: Vec<BoneRest>,
    /// The bone whose track holds the root's path through each clip. That
    /// path is handed out as root motion, so it is taken off the bone.
    pub root_bone: Option<usize>,
    /// Parameters that some clip carries a curve for.
    pub(super) curve_driven: Vec<bool>,
    /// The bones whose scale is kept that some clip scales.
    pub scaled: Vec<usize>,
    /// The bones its host follows: what is read of the pose after every
    /// update (`follow`; every bone unless told otherwise).
    pub(super) followed: Vec<bool>,
    /// The bones the layers' states as read may leave as they stood
    /// (`may_stand`, `Animator::kept_share`): each such bone's pose carries
    /// from one update to the next. Told, not used: an update works out the
    /// bones its layers leave standing on it (`standing`), foreseen or not.
    pub(super) carried: Vec<bool>,
    /// The bones every update works out: the followed ones, the root and
    /// the bones whose scale is kept (with, on each update, those a layer
    /// leaves standing). The rest are made from an update's mix (`pose_of`),
    /// at once where nothing is deferred.
    pub(super) always: Vec<bool>,
    /// The places of the `always` bones, in order: a mix keeps their poses
    /// in this order.
    pub(super) always_list: Vec<usize>,
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
                    let def = node_def(node, clip, &parameter);
                    machines[s.machine].keeps_weights |= def.missing;
                    out.push(def);
                }
                trees.push(Some(out));
            }
            machines[s.machine].states.push(StateDef {
                name: s.full_name.clone(),
                speed: s.speed,
                speed_parameter: parameter(&s.speed_parameter),
                looping: s.looping,
                writes_defaults: s.writes_defaults,
                cycle_offset: s.cycle_offset,
                trees,
                transitions: s.transitions.iter().map(transition).collect(),
                movement: s.movement.clone(),
                layer_weights: s.layer_weights.clone(),
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
        let mut controller = Self {
            parameters: table.parameters.clone(),
            by_name,
            machines,
            layers,
            clips,
            followed: vec![true; bones.len()],
            bones,
            root_bone: None,
            curve_driven,
            scaled,
            carried: Vec::new(),
            always: Vec::new(),
            always_list: Vec::new(),
        };
        controller.mark();
        Ok(controller)
    }

    /// Marks the bones its host follows (`followed`, by the bones' places),
    /// and splits every clip into the curves of the bones every update
    /// works out (`always`: the followed ones, the root and the bones whose
    /// scale is kept) and the rest, which `pose_of` makes from an update's
    /// mix (but those a layer leaves standing on an update, which it works
    /// out: `standing`). Once, on a controller as loaded (every clip whole)
    /// and after its root bone is set.
    pub fn follow(&mut self, followed: &[bool]) -> Result<()> {
        anyhow::ensure!(followed.len() == self.bones.len(), "{} bones followed of {}", followed.len(), self.bones.len());
        anyhow::ensure!(self.clips.iter().all(|clip| clip.rest.is_none()) && self.always.iter().all(|a| *a), "a controller's clips are split once");
        self.followed = followed.to_vec();
        self.mark();
        let always = self.always.clone();
        for clip in &mut self.clips {
            clip.split(&always);
        }
        Ok(())
    }

    /// The bones its host follows, by place.
    pub fn followed(&self) -> &[bool] {
        &self.followed
    }

    /// The bones the layers' states as read may leave as they stood, by
    /// place (told, not used: an update carries what its layers leave
    /// standing on it, `standing`).
    pub fn carried(&self) -> &[bool] {
        &self.carried
    }

    /// The bones every update works out, by place.
    pub fn always(&self) -> &[bool] {
        &self.always
    }

    /// What one of its clips gives at a time (seconds into the clip), on
    /// the engine's straight lines or, with `smooth`, on the curves without
    /// corners: each bone's tracks and their reference, each parameter's
    /// curve, the root's path and turn (sampled among the rest, and alone).
    /// For setting a clip split (`follow`) beside itself whole.
    pub fn sampled(&self, clip: usize, time: f32, smooth: bool) -> Sampled {
        let def = &self.clips[clip];
        let sample = |clip: &Clip| {
            let mut values = Vec::new();
            if smooth {
                clip.sample_smooth(time, &mut values);
            } else {
                clip.sample(time, &mut values);
            }
            values
        };
        let (kept, rest) = (sample(&def.clip), def.rest.as_ref().map(sample).unwrap_or_default());
        let in_part = |bone: usize, kept: &[f32], rest: &[f32]| -> [Vec<f32>; 3] {
            let values = if self.always[bone] { kept } else { rest };
            let tracks = def.bones[bone];
            let run = |first: Option<usize>, width: usize| first.filter(|c| c + width <= values.len()).map_or(Vec::new(), |c| values[c..c + width].to_vec());
            let width = if tracks.rotation.is_some_and(|(_, euler)| euler) { 3 } else { 4 };
            [run(tracks.position, 3), run(tracks.rotation.map(|(c, _)| c), width), run(tracks.scale, 3)]
        };
        let rest_reference = def.rest.as_ref().map_or(&[][..], |rest| &rest.reference_pose[..]);
        let root: Vec<usize> = def.root_path.into_iter().flatten().chain(def.root_turn.into_iter().flatten()).collect();
        Sampled {
            bones: (0..self.bones.len()).map(|bone| in_part(bone, &kept, &rest)).collect(),
            references: (0..self.bones.len()).map(|bone| if def.referenced { in_part(bone, &def.clip.reference_pose, rest_reference) } else { Default::default() }).collect(),
            curves: def.curves.iter().map(|&(parameter, curve)| (parameter, kept[curve])).collect(),
            root: root.iter().map(|&curve| kept[curve]).collect(),
            root_alone: root.iter().map(|&curve| def.clip.sample_curve(time, curve)).collect(),
        }
    }

    /// Works out the carried bones and the bones every update works out
    /// from the followed ones.
    pub(super) fn mark(&mut self) {
        let count = self.bones.len();
        let mut carried = vec![false; count];
        for layer in self.layers.iter().filter(|layer| self.may_stand(layer)) {
            for (carried, moves) in carried.iter_mut().zip(&layer.moves) {
                *carried |= *moves;
            }
        }
        self.carried = carried;
        self.always = (0..count).map(|bone| self.followed[bone] || self.root_bone == Some(bone) || self.scaled.contains(&bone)).collect();
        self.always_list = (0..count).filter(|bone| self.always[*bone]).collect();
    }

    /// Whether a layer may leave its bones as they stood, by some share
    /// (`Animator::kept_share`): an override layer one of whose states as
    /// read (or its machine's first) writes no defaults and may give no clip
    /// a share on the layer's slot, having no tree there or one that may give
    /// none a share (`surely_moves`). (A state not read can still be asked
    /// for outright, or a run taken up in it: an update carries whatever its
    /// layers leave standing, foreseen here or not: `Controller::standing`.)
    fn may_stand(&self, layer: &LayerDef) -> bool {
        !layer.additive
            && self.machines.get(layer.machine).is_some_and(|machine| {
                machine.states.iter().enumerate().any(|(index, state)| {
                    (state.loaded || index == machine.default_state)
                        && !state.writes_defaults
                        && !state.trees.get(layer.slot).and_then(Option::as_ref).is_some_and(|tree| surely_moves(tree, 0, 0))
                })
            })
    }

    /// Every bone at rest.
    pub(super) fn rest_pose(&self) -> Vec<Bone> {
        self.bones.iter().map(|b| (b.position, b.rotation)).collect()
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
            followed: Vec::new(),
            carried: Vec::new(),
            always: Vec::new(),
            always_list: Vec::new(),
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
    let (x, y) = (parameter(&node.parameter_x), parameter(&node.parameter_y));
    // The engine looks a blend's parameter up among the controller's and,
    // not finding it, leaves the weights as they were (`EvaluateBlendTree`;
    // the install's Rhinos', MTs-255-12's, RSh-12's and M32A1's misspelt
    // names: `more-weapons.md`, "A cylinder drawn between frames").
    let missing = clip.is_none()
        && !node.children.is_empty()
        && match node.blend_type {
            0 => x.is_none(),
            1..=3 => x.is_none() || y.is_none(),
            _ => false,
        };
    NodeDef {
        blend_type: node.blend_type,
        parameter_x: x,
        parameter_y: y,
        children: node.children.clone(),
        thresholds: node.thresholds.clone(),
        positions: node.positions.clone(),
        clip,
        length_factor: if node.duration > 0.0 { node.duration } else { 1.0 },
        cycle_offset: node.cycle_offset,
        missing,
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
    let referenced = reduced.reference_pose.len() == reduced.curve_count();
    ClipDef { root_path: reduced.root_path(), root_turn: reduced.root_turn(), bones: tracks, curves, clip: reduced, rest: None, referenced }
}

/// Whether a blend tree from its node `index` gives some clip a share for
/// any finite parameters: a clip does; a blend by one parameter over
/// thresholds in order (`weight_1d` gives the two about the value their
/// shares, or the end one all), or one whose children the animator shares
/// out equally, does if each of its children does. A blend missing its
/// parameter takes what its workspace last held, which may be nothing, and
/// a blend in two dimensions is not trusted to give any.
fn surely_moves(tree: &[NodeDef], index: usize, depth: usize) -> bool {
    let Some(node) = tree.get(index) else { return false };
    if node.clip.is_some() {
        return true;
    }
    if node.missing || node.children.is_empty() || depth > tree.len() {
        return false;
    }
    let n = node.children.len();
    let shares = match node.blend_type {
        0 if node.thresholds.len() == n => node.thresholds.windows(2).all(|pair| pair[0] <= pair[1]),
        1..=3 if node.positions.len() == n => false,
        // (`node_weights`: a share each.)
        _ => true,
    };
    shares && node.children.iter().all(|child| surely_moves(tree, *child, depth + 1))
}

/// Whether a component of the root's travel (0 x, 1 y, 2 z) stays in the
/// clip's pose instead of going out as root motion.
pub(super) fn stays_in_pose(clip: &Clip, component: usize) -> bool {
    if component == 1 {
        clip.root_stays_upright
    } else {
        clip.root_stays_level
    }
}
