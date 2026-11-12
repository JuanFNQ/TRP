//! The update, events, transition checks, lengths, weights and triggers.

use super::*;

impl Animator {
    /// Advances by `dt` seconds and returns what states were entered and left.
    pub fn update(&mut self, dt: f32) -> Vec<Event> {
        self.events.clear();
        self.passed.clear();
        self.looks.clear();
        let controller = self.controller.clone();
        for machine in 0..controller.machines.len() {
            let asked = self.asked[machine].is_some();
            let (before, held) = (self.machines[machine].current.state, self.machines[machine].held.is_some());
            self.step_machine(&controller, machine, dt);
            if controller.machines[machine].keeps_weights {
                // The engine works out the state it held at the update's
                // start first, unless a transition cut short holds it still
                // (`EvaluateStateMachine`), and only then turns to a state
                // asked for outright or to the transitions; then the state
                // entered, even where the transition takes no time.
                let now = &self.machines[machine];
                let mut order = Vec::with_capacity(3);
                if !held {
                    order.push(before);
                }
                if asked || now.current.state != before {
                    order.push(now.current.state);
                }
                order.extend(now.next.map(|next| next.state));
                for state in order {
                    self.keep_weights(&controller, machine, state);
                }
            }
        }
        // The states' scripts that set the third layer's weight
        // (`LayerWeightStateController`) as the engine tells them of the
        // update: its start as a state is entered, its curve at the state's
        // normalised time while it plays, its end as it is left, before the
        // pose is worked out. `PROVISIONAL`: the weight set taken into this
        // update's pose; a state played backwards gives its time as it runs.
        let set: Vec<f32> = self
            .looks
            .iter()
            .flat_map(|look| {
                controller.machines[look.machine].states[look.state].layer_weights.iter().map(move |script| match look.kind {
                    LookKind::Enter => script.start,
                    LookKind::Update => script.at(look.time),
                    LookKind::Exit => script.end,
                })
            })
            .collect();
        for weight in set {
            self.set_layer_weight(trp_data::weapons::states::LayerWeight::LAYER, weight);
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

    pub(super) fn look(&mut self, controller: &Controller, machine: usize, state: usize, kind: LookKind, time: f32, leaving: bool) {
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
    pub(super) fn state_length(&self, controller: &Controller, machine: usize, state: usize) -> f32 {
        let s = &controller.machines[machine].states[state];
        let motion = self.motion_length(controller, machine, state);
        let speed = (s.speed * s.speed_parameter.map_or(1.0, |p| self.values[p])).abs();
        if speed > 1e-6 {
            motion / speed
        } else {
            f32::INFINITY
        }
    }

    /// A state's length at its clips' own pace, its speed not counted: what
    /// the engine calls the state's length.
    fn motion_length(&self, controller: &Controller, machine: usize, state: usize) -> f32 {
        let motion = match controller.machines[machine].states[state].trees.first().and_then(Option::as_ref) {
            Some(tree) => {
                let mut leaves = Vec::new();
                self.leaves((machine, state, 0), tree, 0, 1.0, 1.0, &mut leaves);
                leaves.iter().map(|(clip, weight, factor, _)| controller.clips[*clip].clip.duration() * factor * weight).sum()
            }
            None => 0.0,
        };
        if motion > 0.0 {
            motion
        } else {
            EMPTY_STATE_LENGTH
        }
    }

    /// The clips of a blend tree with their weights, the factor on their
    /// length and their offset into the cycle. `at` is where the tree is:
    /// its machine, its state and its layer's slot.
    pub(super) fn leaves(&self, at: (usize, usize, usize), tree: &[NodeDef], index: usize, weight: f32, factor: f32, out: &mut Vec<(usize, f32, f32, f32)>) {
        let Some(node) = tree.get(index) else { return };
        let factor = factor * node.length_factor;
        if let Some(clip) = node.clip {
            out.push((clip, weight, factor, node.cycle_offset));
            return;
        }
        let weights = if node.missing {
            let (machine, state, slot) = at;
            self.kept.get(&(machine, state, slot, index)).cloned().unwrap_or_else(|| vec![0.0; node.children.len()])
        } else {
            self.node_weights(node)
        };
        for (child, w) in node.children.iter().zip(weights) {
            if w > 0.0 {
                self.leaves(at, tree, *child, weight * w, factor, out);
            }
        }
    }

    /// The engine's blend workspaces at one state's evaluation, slot by
    /// slot and node by node in their order: a node that has its parameter
    /// leaves its children's weights in its slot's workspace, and one
    /// `missing` it takes what is there, kept for the drawing (`kept`).
    /// The engine evaluates every blend node, whatever its weight.
    fn keep_weights(&mut self, controller: &Controller, machine: usize, state: usize) {
        let Some(s) = controller.machines[machine].states.get(state) else { return };
        for (slot, tree) in s.trees.iter().enumerate() {
            let Some(tree) = tree else { continue };
            for (index, node) in tree.iter().enumerate() {
                if node.clip.is_some() || node.children.is_empty() {
                    continue;
                }
                let n = node.children.len();
                if node.missing {
                    let space = self.workspace.get(&(machine, slot));
                    let taken = (0..n).map(|i| space.and_then(|w| w.get(i)).copied().unwrap_or(0.0)).collect();
                    self.kept.insert((machine, state, slot, index), taken);
                } else {
                    let weights = self.node_weights(node);
                    let space = self.workspace.entry((machine, slot)).or_default();
                    if space.len() < n {
                        space.resize(n, 0.0);
                    }
                    space[..n].copy_from_slice(&weights);
                }
            }
        }
    }

    /// A blend node's children's weights from the parameters as they stand.
    fn node_weights(&self, node: &NodeDef) -> Vec<f32> {
        let n = node.children.len();
        let value = |p: Option<usize>| p.map_or(0.0, |p| self.values[p]);
        match node.blend_type {
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
    pub(super) fn triggered(&self, def: &MachineDef, state: usize, start: f32, end: f32) -> Option<(TransitionDef, usize, f32)> {
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
    pub(super) fn consume_triggers(&mut self, controller: &Controller, t: &TransitionDef) {
        for c in &t.conditions {
            if let Some(p) = c.parameter.filter(|p| controller.parameters[*p].kind == ParameterKind::Trigger) {
                self.values[p] = 0.0;
            }
        }
    }

    pub(super) fn blend_weight(&self, machine: usize) -> f32 {
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
    /// For a blend out of states held still the share is of the state being
    /// *entered*, at its clips' own pace (its speed not counted), over the
    /// pace the held state was playing at. Recorded twice (the walk entered
    /// out of a vault and out of a sprint's vault, both held at 1.25 times
    /// the clips' pace: the blend's weight is the walk's own progress over
    /// 0.08, for a share of 0.1), and the engine's own rule
    /// (`EvaluateStateMachine`, `EvaluateState`: the duration over the
    /// entered state's length by the held state's speed). The
    /// entered state's own speed left out: recorded
    /// (`courses8\akm-blind-restrictions`, 19.55 s: blind fire's way out to
    /// the right, at 1.3, entered by a share of 0.2 out of the way out over
    /// the top held at 1.3, blended in 0.19 s, 0.2 × 1.233 ÷ 1.3; with its
    /// speed counted twice it was 0.15).
    pub(super) fn blend_length(&self, machine: usize, blend: &Blend) -> f32 {
        let controller = &self.controller;
        let state = &self.machines[machine];
        match (blend.fixed, blend.pace, state.next) {
            (true, ..) => blend.duration,
            (false, Some(pace), Some(next)) => blend.duration * self.motion_length(controller, machine, next.state) / pace,
            _ => blend.duration * self.state_length(controller, machine, state.current.state),
        }
    }

    /// How fast a state plays: its own speed times its speed parameter's.
    pub(super) fn state_pace(&self, def: &MachineDef, state: usize) -> f32 {
        let s = &def.states[state];
        (s.speed * s.speed_parameter.map_or(1.0, |p| self.values[p])).abs().max(1e-6)
    }
}
