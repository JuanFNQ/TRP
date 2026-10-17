//! A weapon's own animator (or that of a rig like one: the empty hands),
//! run as the engine runs it, with the marks the game's logic waits for.
//! See `docs/spec/firing.md`, "The weapon's animator as a state machine".
//!
//! It is the game's `FirearmsAnimator` seen from below: parameters are set
//! by name, a state can be asked for outright, an update moves every
//! layer and queues the marks it passes, and `deliver` hands those out
//! later in the frame.

use std::sync::Arc;

use anyhow::{anyhow, Result};
use trp_data::custom::PoseEdits;
use trp_data::states::StateTable;
use trp_data::unity::Assets;
use trp_data::weapon::Weapon;

use crate::animator::{Animator, BoneRest, Controller};
use crate::marks::{Mark, MarkTable, Marks};

/// The layer the weapon's states are on.
pub const HANDS_LAYER: &str = "Hands";

/// A weapon's controller, read once.
#[derive(Clone)]
pub struct WeaponAnimatorData {
    controller: Arc<Controller>,
    marks: Arc<MarkTable>,
    hands: usize,
    /// For a weapon of one's own on another's rig: the edits made to
    /// every pose (`trp_data::custom::PoseEdits`).
    edits: Option<Arc<PoseEdits>>,
}

impl WeaponAnimatorData {
    /// Reads the controller of a weapon, or of a prefab like one, with
    /// every clip of every state. The bones posed are the rig's own, in
    /// the order of its hierarchy.
    pub fn load(assets: &Assets, weapon: &Weapon) -> Result<Self> {
        Self::load_with(assets, weapon, false)
    }

    /// As `load`, keeping the clips' scale of the bones too: for an item's
    /// rig, whose item is drawn by its bone scaled up from nought. (A
    /// weapon's clips scale nearly every bone, by the look of it to what it
    /// rests at; they are not taken, `docs/findings.md`.)
    pub fn load_scaled(assets: &Assets, weapon: &Weapon) -> Result<Self> {
        Self::load_with(assets, weapon, true)
    }

    fn load_with(assets: &Assets, weapon: &Weapon, scaled: bool) -> Result<Self> {
        let table = StateTable::load(assets, weapon)?;
        let tree = &weapon.hierarchy;
        let mut bones: Vec<BoneRest> = (0..tree.nodes.len())
            .map(|index| BoneRest {
                path: tree.path(index),
                position: tree.nodes[index].translation,
                rotation: tree.nodes[index].rotation,
                scale: scaled.then_some(tree.nodes[index].scale),
            })
            .collect();
        // A weapon of one's own has the bones its edits move resting
        // where the edits have them; the animator is the donor's and
        // takes the donor's own rest, and its pose is edited afterwards.
        let edits = weapon.custom.as_ref().map(|custom| custom.edits.clone());
        for (bone, rest) in edits.iter().flat_map(|edits| edits.donor_rest()) {
            bones[*bone].position = *rest;
        }
        let controller = Controller::load(assets, &table, bones, &|_| true)?;
        let hands = controller.layer(HANDS_LAYER).ok_or_else(|| anyhow!("{} has no layer named {HANDS_LAYER}", weapon.name))?;
        let marks = MarkTable::new(&table, &controller);
        Ok(Self { controller: Arc::new(controller), marks: Arc::new(marks), hands, edits })
    }

    /// A controller written out by hand with its marks, for trying the
    /// logic where there is no install.
    pub fn by_hand(controller: Controller, marks: MarkTable) -> Result<Self> {
        let hands = controller.layer(HANDS_LAYER).ok_or_else(|| anyhow!("no layer named {HANDS_LAYER}"))?;
        Ok(Self { controller: Arc::new(controller), marks: Arc::new(marks), hands, edits: None })
    }

    /// One of the rig's bones by its path below the rig's root, as its
    /// place in the animator's pose.
    pub fn bone(&self, path: &str) -> Option<usize> {
        self.controller.bones.iter().position(|bone| bone.path == path)
    }

    /// Whether the hands' layer has a state of this full name.
    pub fn has_state(&self, state: &str) -> bool {
        self.controller.state(self.hands, state).is_some()
    }

    /// The full names of the hands' states.
    pub fn state_names(&self) -> Vec<String> {
        let machine = self.controller.layers[self.hands].machine;
        self.controller.machines[machine].states.iter().map(|state| state.name.clone()).collect()
    }

    /// How many clips were read for it.
    pub fn clip_count(&self) -> usize {
        self.controller.clips.len()
    }

    /// The paths of the bones some clip scales.
    pub fn scaled_bones(&self) -> Vec<&str> {
        self.controller.scaled.iter().map(|bone| self.controller.bones[*bone].path.as_str()).collect()
    }

    /// The marks of one of the hands' states, by its full name, each with
    /// the fraction of the state it sits at.
    pub fn marks_of(&self, state: &str) -> Vec<(String, f32)> {
        let machine = self.controller.layers[self.hands].machine;
        let Some(index) = self.controller.state(self.hands, state) else { return Vec::new() };
        self.marks.of(machine, index).map(|(mark, time)| (mark.name.clone(), time)).collect()
    }
}

#[derive(Clone)]
pub struct WeaponAnimator {
    animator: Animator,
    marks: Marks,
    hands: usize,
    /// The edits to every pose, for a weapon of one's own, and the last
    /// pose as edited. **What the animator itself does, its states, its
    /// times and its marks, is the donor's and is not touched**: the
    /// edits move bones in what is given out, after the fact.
    edits: Option<Arc<PoseEdits>>,
    edited: Vec<([f32; 3], [f32; 4])>,
}

impl WeaponAnimator {
    /// The animator as the prefab has it: every layer in its first state.
    pub fn new(data: &WeaponAnimatorData) -> Self {
        let mut made = Self {
            animator: Animator::new(data.controller.clone()),
            marks: Marks::new(data.marks.clone()),
            hands: data.hands,
            edits: data.edits.clone(),
            edited: Vec::new(),
        };
        made.edit();
        made
    }

    /// The animator as it is made afresh for a thing taken in hand: every
    /// parameter as the controller starts it, every layer in its first
    /// state, nothing waiting to be delivered.
    pub fn reset(&mut self) {
        self.animator = Animator::new(self.animator.controller.clone());
        self.marks.reset();
        self.edit();
    }

    /// Takes the animator's pose afresh and edits it, where the weapon
    /// has edits.
    fn edit(&mut self) {
        if let Some(edits) = &self.edits {
            self.edited.clear();
            self.edited.extend_from_slice(self.animator.pose());
            edits.apply(&mut self.edited);
        }
    }

    /// Whether the hands' layer is in the named state and on no way out
    /// of it.
    pub fn rests_in(&self, state: &str) -> bool {
        let (current, next) = self.animator.state(self.hands);
        current.name == state && next.is_none()
    }

    pub fn has(&self, parameter: &str) -> bool {
        self.animator.controller.parameter(parameter).is_some()
    }

    /// Sets a parameter by name; one the controller lacks is let be, as
    /// the engine lets it.
    pub fn set(&mut self, parameter: &str, value: f32) {
        self.animator.set_float(parameter, value);
    }

    pub fn set_flag(&mut self, parameter: &str, on: bool) {
        self.animator.set_bool(parameter, on);
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
        self.edit();
    }

    /// The marks queued since they were last delivered, each if its
    /// conditions hold now.
    pub fn deliver(&mut self) -> Vec<Mark> {
        self.marks.deliver(&self.animator)
    }

    /// The next of the marks queued, if its conditions hold now; `None`
    /// when none is left. A handler that sets a parameter between two
    /// calls is seen by the second.
    pub fn deliver_next(&mut self) -> Option<Mark> {
        self.marks.deliver_next(&self.animator)
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
        if self.edits.is_some() {
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
        pose
    }

    /// Every layer as it stands, for looking at.
    pub fn describe(&self) -> Vec<String> {
        self.animator.describe()
    }
}
