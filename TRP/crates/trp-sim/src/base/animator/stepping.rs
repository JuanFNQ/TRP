//! Starting and cutting states short, stepping each machine and evaluating the mix.

use super::*;

impl Animator {
    /// Starts a transition with `left` seconds of the frame still to run.
    pub(super) fn start(&mut self, controller: &Controller, machine: usize, t: &TransitionDef, target: usize, left: f32, end: f32) {
        self.consume_triggers(controller, t);
        let source_length = self.state_length(controller, machine, self.machines[machine].current.state);
        let target_length = self.state_length(controller, machine, target);
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
        let target_length = self.state_length(controller, machine, target);
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

    pub(super) fn step_machine(&mut self, controller: &Controller, machine: usize, dt: f32) {
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
        let length = self.state_length(controller, machine, current.state);
        // A state held still by a transition that was cut short stays put.
        let held = self.machines[machine].held.is_some();
        let step = if length.is_finite() && !held { dt / length } else { 0.0 };
        let end = current.time + step;

        if let (Some(blend), Some(next)) = (self.machines[machine].blend, self.machines[machine].next) {
            // A transition under way can be cut short by another from the
            // side it allows. States held still start nothing.
            let from_source = matches!(blend.interruption, 1 | 3 | 4) && !held;
            let from_target = matches!(blend.interruption, 2 | 3 | 4);
            let next_length = self.state_length(controller, machine, next.state);
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
    /// What a layer's machine plays, each with its share of the layer: the
    /// state being left, with whatever is held still beside it, and the
    /// state being entered.
    pub(super) fn playing(&self, layer: &LayerDef) -> Vec<(Option<Playing>, f32)> {
        let state = &self.machines[layer.machine];
        let blend = self.blend_weight(layer.machine);
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
        playing
    }

    /// The share of a layer played by states with no motion on it that do
    /// not write defaults (`StateDef::writes_defaults` off): the layer
    /// leaves its bones as they stand by that share (the engine writes
    /// nothing there), where a state that writes them sets their defaults.
    fn kept_share(&self, controller: &Controller, layer: &LayerDef) -> f32 {
        let def = &controller.machines[layer.machine];
        let mut kept = 0.0;
        for (playing, weight) in self.playing(layer) {
            let Some(playing) = playing.filter(|_| weight > 0.0) else { continue };
            let Some(s) = def.states.get(playing.state) else { continue };
            let moved = s.trees.get(layer.slot).and_then(Option::as_ref).is_some_and(|tree| {
                let mut leaves = Vec::new();
                self.leaves((layer.machine, playing.state, layer.slot), tree, 0, 1.0, 1.0, &mut leaves);
                leaves.iter().any(|(_, share, ..)| *share > 0.0)
            });
            if !moved && !s.writes_defaults {
                kept += weight;
            }
        }
        kept
    }

    pub(super) fn shares(&self, controller: &Controller, layer: &LayerDef) -> Vec<Share> {
        let def = &controller.machines[layer.machine];
        let mut out = Vec::new();
        for (playing, weight) in self.playing(layer) {
            let Some(playing) = playing.filter(|_| weight > 0.0) else { continue };
            // A layer may have no state at all (a weapon's first layer).
            let Some(s) = def.states.get(playing.state) else { continue };
            let Some(tree) = s.trees.get(layer.slot).and_then(Option::as_ref) else { continue };
            let mut leaves = Vec::new();
            self.leaves((layer.machine, playing.state, layer.slot), tree, 0, 1.0, 1.0, &mut leaves);
            let reversed = s.speed * s.speed_parameter.map_or(1.0, |p| self.values[p]) < 0.0;
            for (clip, leaf_weight, _, offset) in leaves {
                out.push(Share {
                    clip,
                    weight: weight * leaf_weight,
                    previous: playing.previous,
                    now: playing.time,
                    offset: offset + s.cycle_offset,
                    reversed,
                });
            }
        }
        out
    }

    /// Each weighted layer's mix, as the machines and parameters now stand.
    pub(super) fn mixed(&self, controller: &Controller) -> Vec<LayerMix> {
        let mut layers = Vec::new();
        for (index, layer) in controller.layers.iter().enumerate() {
            let weight = self.weights[index];
            if weight <= 0.0 {
                continue;
            }
            let shares = self.shares(controller, layer);
            let total: f32 = shares.iter().map(|s| s.weight).sum();
            // A state with no motion on an override layer that does not
            // write defaults leaves the layer's bones as they stand
            // (`kept_share`, `Controller::pose_part`).
            let kept = if layer.additive { 0.0 } else { self.kept_share(controller, layer) };
            layers.push(LayerMix { layer: index, weight, shares, total, kept });
        }
        layers
    }

    /// Works out the frame's mix from where the machines now stand and,
    /// from it, the root's movement, the curve-driven parameters, the
    /// clips' events, the bones every update works out
    /// (`Controller::always`) and those a layer leaves standing on it (they
    /// carry to the next update); then, unless deferred, the rest of the
    /// pose (`Controller::pose_part`, as `pose_of` makes it later). Where it
    /// was asked for, the pose for drawing too, which nothing else is taken
    /// from (where deferred, only the bones a layer leaves standing).
    pub(super) fn evaluate(&mut self, controller: &Controller) {
        let count = controller.bones.len();
        let layers = self.mixed(controller);
        let mut samples = std::mem::take(&mut self.samples);
        // Each bone as it stood after the last update, for a layer that
        // leaves it as it stands: where that update was deferred, a bone it
        // did not work out is made from its mix.
        let standing = controller.standing(&layers);
        let first: Vec<bool> = (0..count).map(|bone| controller.always[bone] || standing[bone]).collect();
        let prior = self.prior(controller, &standing, false);
        // (Where deferred, the rest keeps what was last worked out.)
        let mut pose = self.pose.clone();
        let mut driven = Driven { curves: vec![None; controller.parameters.len()], root: [0.0; 3], turn: 0.0, passed: Vec::new() };
        controller.pose_part(&layers, &first, false, &mut pose, prior.as_deref(), &mut samples, Some(&mut driven));
        self.passed.append(&mut driven.passed);
        if !controller.scaled.is_empty() {
            self.scales = controller.scales_of(&layers, &mut samples[0]);
        }
        let Driven { curves, mut root, mut turn, .. } = driven;
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
        if !self.deferred {
            let rest: Vec<bool> = first.iter().map(|first| !first).collect();
            controller.pose_part(&layers, &rest, false, &mut pose, None, &mut samples, None);
        }
        let always = controller.always_list.iter().map(|&bone| pose[bone]).collect();
        let stood = (0..count).filter(|bone| standing[*bone] && !controller.always[*bone]).map(|bone| (bone, pose[bone])).collect();
        self.pose = pose;
        let drawn = if self.smooth {
            // The pose for drawing has always been worked out after the
            // parameters the clips drive were set: its blend trees read
            // them so.
            let layers = self.mixed(controller);
            let standing = controller.standing(&layers);
            let prior = self.prior(controller, &standing, true);
            let mut drawn = self.drawn.take().filter(|drawn| drawn.len() == count).unwrap_or_else(|| controller.rest_pose());
            let chosen = if self.deferred { standing.clone() } else { vec![true; count] };
            controller.pose_part(&layers, &chosen, true, &mut drawn, prior.as_deref(), &mut samples, None);
            let stood = (0..count).filter(|bone| standing[*bone]).map(|bone| (bone, drawn[bone])).collect();
            self.drawn = Some(drawn);
            Some(DrawnMix { layers, standing: stood })
        } else {
            self.drawn = None;
            None
        };
        self.mix = Mix { layers, always, standing: stood, drawn };
        self.complete = !self.deferred;
        self.samples = samples;
    }

    /// The pose (or, `drawn`, the pose for drawing) as the last update left
    /// it, for the layers that leave the bones `standing` as they stood:
    /// where that update was deferred, each of those bones it did not work
    /// out made from its mix. None for the pose for drawing where the last
    /// update made none.
    pub(super) fn prior(&self, controller: &Controller, standing: &[bool], drawn: bool) -> Option<Vec<Bone>> {
        // (Read only for a layer with a kept share, which leaves its bones
        // standing: with none, none is needed.)
        if !standing.contains(&true) {
            return None;
        }
        let count = controller.bones.len();
        let last = if drawn { self.drawn.clone()? } else { self.pose.clone() };
        if last.len() != count {
            return None;
        }
        let worked_out = |bone: usize| {
            if drawn {
                self.mix.drawn.as_ref().is_some_and(|mix| mix.stood(bone))
            } else {
                controller.always[bone] || self.mix.stood(bone)
            }
        };
        let missing: Vec<usize> = (0..count).filter(|&bone| !self.complete && standing[bone] && !worked_out(bone)).collect();
        if missing.is_empty() {
            return Some(last);
        }
        let made = if drawn { controller.drawn_of(&self.mix)? } else { controller.pose_of(&self.mix) };
        let mut prior = last;
        for bone in missing {
            prior[bone] = made[bone];
        }
        Some(prior)
    }

    /// `prior`'s pose (not the one for drawing) as it stands for the bones
    /// `asked`: each of them as `prior` gives it, the rest as the last
    /// update left them. The last mix is posed only where an asked bone is
    /// missing from the last update's pose (`bones_with`).
    pub(super) fn prior_for(&self, controller: &Controller, standing: &[bool], asked: &[bool]) -> Option<Vec<Bone>> {
        if !standing.contains(&true) {
            return None;
        }
        let count = controller.bones.len();
        let last = self.pose.clone();
        if last.len() != count {
            return None;
        }
        let missing = |bone: usize| !self.complete && standing[bone] && !(controller.always[bone] || self.mix.stood(bone));
        if !(0..count).any(|bone| asked[bone] && missing(bone)) {
            return Some(last);
        }
        self.prior(controller, standing, false)
    }
}
