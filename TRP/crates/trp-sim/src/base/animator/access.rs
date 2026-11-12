//! Making the animator, setting its parameters and reading its state and pose.

use super::*;

impl Animator {
    /// Takes another animator's running state, its parameters, its
    /// machines' states and times and its layers' weights, where both run
    /// machines of the same shape (a weapon built anew with another part:
    /// the same controller bound to another tree). The pose follows at the
    /// next update. Whether it could.
    pub fn take_state(&mut self, from: &Animator) -> bool {
        let same = self.values.len() == from.values.len() && self.machines.len() == from.machines.len() && self.weights.len() == from.weights.len();
        if !same {
            return false;
        }
        self.values.clone_from(&from.values);
        self.machines.clone_from(&from.machines);
        self.weights.clone_from(&from.weights);
        (self.root_delta, self.root_turn, self.step) = (from.root_delta, from.root_turn, from.step);
        self.events.clone_from(&from.events);
        self.passed.clone_from(&from.passed);
        self.asked.clone_from(&from.asked);
        self.workspace.clone_from(&from.workspace);
        self.kept.clone_from(&from.kept);
        true
    }

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
        let pose: Vec<Bone> = controller.rest_pose();
        let mix = Mix { layers: Vec::new(), always: controller.always_list.iter().map(|&bone| pose[bone]).collect(), standing: Vec::new(), drawn: None };
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
                samples: [Vec::new(), Vec::new()],
                deferred: false,
                mix,
                complete: true,
                step: 0.0,
                asked: Vec::new(),
                looks: Vec::new(),
                workspace: HashMap::new(),
                kept: HashMap::new(),
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
                // (A layer's machine may have no state at all.)
                let name = |playing: &Playing| match def.states.get(playing.state) {
                    Some(s) => format!("{}{} at {:.3}", s.name, if s.loaded { "" } else { " (not read)" }, playing.time),
                    None => "no state".to_owned(),
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
    /// a weapon). The pose for drawing
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
    /// what the view's newest turn would make of the pose. **Not the last
    /// update's pose worked out again**, even with no parameter given: the
    /// blend trees read the parameters the clips drive as that update left
    /// them, after its pose was made, and a layer that leaves bones as they
    /// stood leaves them as that update did. The
    /// last update's pose is its mix's (`pose_of`).
    pub fn pose_with(&self, parameters: &[(usize, f32)]) -> Vec<([f32; 3], [f32; 4])> {
        let mut copy = self.clone();
        for (parameter, value) in parameters {
            copy.values[*parameter] = *value;
        }
        copy.deferred = false;
        let controller = copy.controller.clone();
        copy.evaluate(&controller);
        copy.pose
    }

    /// `pose_with`'s pose of the bones `wanted` alone, bit for bit, the
    /// rest of the returned pose left as the last update had it: the same
    /// mix and the same two passes of `Controller::pose_part` (the bones an
    /// update always works out or a layer leaves standing, from what stood
    /// before; then the rest), each bone worked out from its own samples
    /// and the layers' weights alone, so a bone comes out as the whole
    /// pose has it. Only the clips' curves the wanted bones read are
    /// sampled (`Controller::pose_part_by`, sparse), the animator is not
    /// copied, and nothing of root motion, curves or events is worked out.
    pub fn bones_with(&self, parameters: &[(usize, f32)], wanted: &[usize]) -> Vec<([f32; 3], [f32; 4])> {
        let controller = &*self.controller;
        let count = controller.bones.len();
        let mut values = self.values.clone();
        for (parameter, value) in parameters {
            values[*parameter] = *value;
        }
        // The layers' mix as `evaluate` makes it, from the parameters set:
        // on a copy of what the mix reads (the parameters, the machines and
        // the blend trees' workspaces) and nothing of the pose.
        let layers = Animator {
            controller: self.controller.clone(),
            values,
            machines: self.machines.clone(),
            weights: self.weights.clone(),
            root_delta: [0.0; 3],
            root_turn: 0.0,
            pose: Vec::new(),
            scales: Vec::new(),
            smooth: false,
            drawn: None,
            events: Vec::new(),
            passed: Vec::new(),
            samples: [Vec::new(), Vec::new()],
            deferred: self.deferred,
            mix: Mix { layers: Vec::new(), always: Vec::new(), standing: Vec::new(), drawn: None },
            complete: self.complete,
            step: self.step,
            asked: Vec::new(),
            looks: Vec::new(),
            workspace: self.workspace.clone(),
            kept: self.kept.clone(),
        }
        .mixed(controller);
        let standing = controller.standing(&layers);
        let first: Vec<bool> = (0..count).map(|bone| controller.always[bone] || standing[bone]).collect();
        let mut asked = vec![false; count];
        for &bone in wanted.iter().filter(|bone| **bone < count) {
            asked[bone] = true;
        }
        // What stood before, as `prior` has it for the wanted bones (no
        // value read there is a parameter): made from the last mix only
        // where a wanted bone is missing from the last update's pose.
        let prior = self.prior_for(controller, &standing, &asked);
        let mut pose = self.pose.clone();
        let mut samples = [Vec::new(), Vec::new()];
        let chosen: Vec<bool> = (0..count).map(|bone| asked[bone] && first[bone]).collect();
        controller.pose_part_by(&layers, &chosen, false, &mut pose, prior.as_deref(), &mut samples, None, true);
        let chosen: Vec<bool> = (0..count).map(|bone| asked[bone] && !first[bone]).collect();
        controller.pose_part_by(&layers, &chosen, false, &mut pose, None, &mut samples, None, true);
        pose
    }

    /// From the next update on, leaves the bones the controller does not
    /// always work out (`Controller::always`) to be made from the update's
    /// mix when they are asked for (`pose_of`, `drawn_of`), or works every
    /// bone out at each update as before (off, as an animator is made).
    /// Nothing of what the animator does goes by those bones: its states,
    /// times, parameters, root motion and events, and the bones it always
    /// works out, are the same either way. Where on, `pose`, `bone` and
    /// `drawn_pose` hold the rest as last worked out.
    pub fn set_deferred(&mut self, on: bool) {
        self.deferred = on;
    }

    pub fn deferred(&self) -> bool {
        self.deferred
    }

    /// The last update's mix: all a pose is made from.
    pub fn mix(&self) -> &Mix {
        &self.mix
    }

    /// The pose of every bone an update's mix makes (`Controller::pose_of`):
    /// what `pose` held after that update with nothing deferred.
    pub fn pose_of(&self, mix: &Mix) -> Vec<([f32; 3], [f32; 4])> {
        self.controller.pose_of(mix)
    }

    /// The pose to draw that an update's mix makes: the pose for drawing
    /// where one was asked for (`Controller::drawn_of`), the pose itself
    /// otherwise. What `drawn_pose` held after that update with nothing
    /// deferred.
    pub fn drawn_of(&self, mix: &Mix) -> Vec<([f32; 3], [f32; 4])> {
        self.controller.drawn_of(mix).unwrap_or_else(|| self.controller.pose_of(mix))
    }

    pub fn state_def(&self, machine: usize, state: usize) -> &StateDef {
        &self.controller.machines[machine].states[state]
    }
}
