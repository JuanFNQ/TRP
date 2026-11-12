//! A weapon's own animator, run as the engine runs it, with the marks the game's logic waits for.
//! A rig like one, the empty hands, is run the same. See `docs/spec/firing.md`, "The weapon's animator as a state machine".
//!
//! It is the game's `FirearmsAnimator` seen from below: parameters are set
//! by name, a state can be asked for outright, an update moves every
//! layer and queues the marks it passes, and `deliver` hands those out
//! later in the frame.

use std::sync::Arc;

use anyhow::{anyhow, Result};
use trp_data::weapons::custom::PoseEdits;
use trp_data::weapons::states::StateTable;
use trp_data::unity::Assets;
use trp_data::weapons::weapon::Weapon;

use crate::base::animator::{Animator, BoneRest, Controller, Mix};
use crate::base::marks::{Mark, MarkTable, Marks};

mod animator_data;
mod parameters;

pub use animator_data::*;
use animator_data::{Part};

/// The layer the weapon's states are on.
pub const HANDS_LAYER: &str = "Hands";

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
    /// The parts' animators (`WeaponAnimatorData::parts`), whose poses are
    /// written over their bones in what is given out.
    parts: Vec<Part>,
    /// Whether its animators leave the bones they need not work out to be
    /// made from their mixes when asked (`set_deferred_posing`).
    deferred: bool,
}

/// What one update of a weapon's animators made of its pose: the weapon's
/// own mix and each part's (`Animator::mix`), from which the whole pose, as
/// given out or as drawn, is made whenever asked (`WeaponAnimator::pose_of`,
/// `drawn_of`).
#[derive(Debug, Clone)]
pub struct WeaponMix {
    own: Mix,
    parts: Vec<Mix>,
}

impl WeaponAnimator {
    /// Takes another's running state, its machines' and its marks' (`Animator::
    /// take_state`), where both are of the same shape. Whether it could.
    pub fn take_state(&mut self, from: &WeaponAnimator) -> bool {
        let mut marks = self.marks.clone();
        if !marks.take_state(&from.marks) || !self.animator.take_state(&from.animator) {
            return false;
        }
        self.marks = marks;
        // The parts' too, where the builds have the same ones.
        if self.parts.len() == from.parts.len() {
            for (own, theirs) in self.parts.iter_mut().zip(&from.parts) {
                let mut marks = own.marks.clone();
                if marks.take_state(&theirs.marks) && own.animator.take_state(&theirs.animator) {
                    own.marks = marks;
                }
            }
        }
        self.edit();
        true
    }

    /// The animator as the prefab has it: every layer in its first state.
    pub fn new(data: &WeaponAnimatorData) -> Self {
        let mut made = Self {
            animator: Animator::new(data.controller.clone()),
            marks: Marks::new(data.marks.clone()),
            hands: data.hands,
            edits: data.edits.clone(),
            edited: Vec::new(),
            parts: data.parts.iter().map(Part::new).collect(),
            deferred: false,
        };
        made.edit();
        made
    }

    /// The animator as it is made afresh for a thing taken in hand: every
    /// parameter as the controller starts it, every layer in its first
    /// state, nothing waiting to be delivered. (Its posing deferred or not,
    /// as it was.)
    pub fn reset(&mut self) {
        self.animator = Animator::new(self.animator.controller.clone());
        self.marks.reset();
        for part in &mut self.parts {
            part.animator = Animator::new(part.animator.controller.clone());
            part.marks.reset();
        }
        self.set_deferred_posing(self.deferred);
        self.edit();
    }

    /// Leaves the bones its controllers do not always work out to be made
    /// from each update's mixes when asked (`posing`, `pose_of`, `drawn_of`),
    /// or works every bone out at each update (off, as it is made):
    /// `Animator::set_deferred`, on the weapon's animator and its parts'.
    /// Where on, `pose` and `drawn_pose` hold those bones as last worked out.
    pub fn set_deferred_posing(&mut self, on: bool) {
        self.deferred = on;
        self.animator.set_deferred(on);
        for part in &mut self.parts {
            part.animator.set_deferred(on);
        }
    }

    /// The last update's mixes.
    pub fn posing(&self) -> WeaponMix {
        WeaponMix { own: self.animator.mix().clone(), parts: self.parts.iter().map(|part| part.animator.mix().clone()).collect() }
    }

    /// Every bone of the rig, each in its parent, as an update's mixes make
    /// it: what `pose` gave after that update with nothing deferred.
    pub fn pose_of(&self, mix: &WeaponMix) -> Vec<([f32; 3], [f32; 4])> {
        let mut pose = self.animator.pose_of(&mix.own);
        if let Some(edits) = &self.edits {
            edits.apply(&mut pose);
        }
        for (part, own) in self.parts.iter().zip(&mix.parts) {
            part.put(&part.animator.pose_of(own), &mut pose);
        }
        pose
    }

    /// Every bone of the rig as it is to be drawn, as an update's mixes make
    /// it: what `drawn_pose` gave after that update with nothing deferred.
    pub fn drawn_of(&self, mix: &WeaponMix) -> Vec<([f32; 3], [f32; 4])> {
        let mut pose = self.animator.drawn_of(&mix.own);
        if let Some(edits) = &self.edits {
            edits.apply(&mut pose);
        }
        for (part, own) in self.parts.iter().zip(&mix.parts) {
            part.put(&part.animator.drawn_of(own), &mut pose);
        }
        pose
    }

    /// Whether what is given out is not the animator's own pose: the
    /// weapon has edits, or parts of its own.
    fn composed(&self) -> bool {
        self.edits.is_some() || !self.parts.is_empty()
    }

    /// Takes the animator's pose afresh, edits it where the weapon has
    /// edits, and writes the parts' poses over their bones.
    fn edit(&mut self) {
        if !self.composed() {
            return;
        }
        self.edited.clear();
        self.edited.extend_from_slice(self.animator.pose());
        if let Some(edits) = &self.edits {
            edits.apply(&mut self.edited);
        }
        for part in &self.parts {
            part.put(part.animator.pose(), &mut self.edited);
        }
    }
}
