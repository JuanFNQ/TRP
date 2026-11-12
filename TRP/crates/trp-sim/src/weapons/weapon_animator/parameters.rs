//! Setting and reading the weapon animator's parameters, states and poses.

use super::*;

impl WeaponAnimator {
    /// Whether the hands' layer is in the named state and on no way out
    /// of it.
    pub fn rests_in(&self, state: &str) -> bool {
        let (current, next) = self.animator.state(self.hands);
        current.name == state && next.is_none()
    }

    pub fn has(&self, parameter: &str) -> bool {
        self.animator.controller.parameter(parameter).is_some()
    }

    /// Whether the hands' layer has a state of this full name.
    pub fn has_state(&self, state: &str) -> bool {
        self.animator.controller.state(self.hands, state).is_some()
    }

    /// Sets a parameter by name; one the controller lacks is let be, as
    /// the engine lets it.
    pub fn set(&mut self, parameter: &str, value: f32) {
        self.animator.set_float(parameter, value);
        for part in &mut self.parts {
            part.animator.set_float(parameter, value);
        }
    }

    pub fn set_flag(&mut self, parameter: &str, on: bool) {
        self.animator.set_bool(parameter, on);
        for part in &mut self.parts {
            part.animator.set_bool(parameter, on);
        }
    }

    /// Whether the animator or one of its parts' has the parameter: a
    /// parameter set on it reaches them all.
    pub fn has_anywhere(&self, parameter: &str) -> bool {
        self.has(parameter) || self.parts.iter().any(|part| part.animator.controller.parameter(parameter).is_some())
    }

    /// Raises a trigger: the transition that uses it clears it.
    pub fn trigger(&mut self, parameter: &str) {
        self.animator.set_bool(parameter, true);
    }

    pub fn get(&self, parameter: &str) -> f32 {
        self.animator.float(parameter)
    }

    pub fn flag(&self, parameter: &str) -> bool {
        self.animator.float(parameter) != 0.0
    }

    pub fn set_layer_weight(&mut self, layer: &str, weight: f32) {
        if let Some(index) = self.animator.controller.layer(layer) {
            self.animator.set_layer_weight(index, weight);
        }
    }

    /// Asks the hands' layer for a state outright, by its full name, at a
    /// fraction of it (`Animator.Play`).
    pub fn play(&mut self, state: &str, at: f32) {
        if let Some(index) = self.animator.controller.state(self.hands, state) {
            self.animator.play(self.hands, index, at);
        }
    }

    /// The engine's update: every layer moves on, and the marks passed are
    /// queued.
    pub fn update(&mut self, dt: f32) {
        self.animator.update(dt);
        self.marks.collect(&self.animator);
        for part in &mut self.parts {
            part.animator.update(dt);
            part.marks.collect(&part.animator);
        }
        self.edit();
    }

    /// The marks queued since they were last delivered, each if its
    /// conditions hold now: the weapon's, then its parts'.
    pub fn deliver(&mut self) -> Vec<Mark> {
        let mut marks = self.marks.deliver(&self.animator);
        for part in &mut self.parts {
            marks.extend(part.marks.deliver(&part.animator));
        }
        marks
    }

    /// The next of the marks queued, if its conditions hold now; `None`
    /// when none is left. A handler that sets a parameter between two
    /// calls is seen by the second. The weapon's first, then its parts'.
    pub fn deliver_next(&mut self) -> Option<Mark> {
        if let Some(mark) = self.marks.deliver_next(&self.animator) {
            return Some(mark);
        }
        self.parts.iter_mut().find_map(|part| part.marks.deliver_next(&part.animator))
    }

    /// The hands' state by its full name, how far it has played (in
    /// cycles), and the state being blended into, if any.
    pub fn state(&self) -> (&str, f32, Option<&str>) {
        let (current, next) = self.animator.state(self.hands);
        (current.name.as_str(), self.animator.progress(self.hands).0, next.map(|state| state.name.as_str()))
    }

    /// Whether the hands' layer is in the named state with no transition
    /// under way: the engine's own answer to "is the current state X",
    /// which the game asks of `IDLE`.
    pub fn current_is(&self, state: &str) -> bool {
        self.animator.state(self.hands).0.name == state
    }

    /// The scale of each bone of the rig that some clip scales, by the
    /// bone's place, after the last update: an item used is drawn by its
    /// bone scaled up from nought.
    pub fn scales(&self) -> impl Iterator<Item = (usize, [f32; 3])> + '_ {
        self.animator.scales()
    }

    /// Every bone of the rig, each in its parent, after the last update.
    pub fn pose(&self) -> &[([f32; 3], [f32; 4])] {
        if self.composed() {
            &self.edited
        } else {
            self.animator.pose()
        }
    }

    /// Asks for the pose that is drawn to be taken on curves without
    /// corners from the next update on, or as the engine has it
    /// (`Animator::set_drawn_smooth`). The pose, the states, the times
    /// and the marks are the same either way.
    pub fn set_drawn_smooth(&mut self, on: bool) {
        self.animator.set_drawn_smooth(on);
    }

    /// Every bone of the rig **as it is to be drawn** after the last
    /// update: the pose, unless a smooth one was asked for, with the
    /// weapon's own edits made to it as they are to the pose.
    pub fn drawn_pose(&self) -> Vec<([f32; 3], [f32; 4])> {
        let mut pose = self.animator.drawn_pose().to_vec();
        if let Some(edits) = &self.edits {
            edits.apply(&mut pose);
        }
        for part in &self.parts {
            part.put(part.animator.drawn_pose(), &mut pose);
        }
        pose
    }

    /// The pose the bones have where the animator now stands, worked out
    /// afresh with no time passing: for a host that is to draw an
    /// animator it has only just made, before its first update.
    pub fn pose_now(&self) -> Vec<([f32; 3], [f32; 4])> {
        let mut pose = self.animator.pose_with(&[]);
        if let Some(edits) = &self.edits {
            edits.apply(&mut pose);
        }
        for part in &self.parts {
            part.put(&part.animator.pose_with(&[]), &mut pose);
        }
        pose
    }

    /// Every layer as it stands, for looking at.
    pub fn describe(&self) -> Vec<String> {
        self.animator.describe()
    }
}
