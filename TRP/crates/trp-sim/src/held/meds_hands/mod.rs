//! The hands that use an item: medicine, food or drink.
//! After `docs/spec/health.md`, "The hands". To the game the item is taken in hand as a weapon is: a rig
//! of its own, made from the item's `UsePrefab`, with an animator, and a
//! controller (`MedsController`) whose one operation (`MedsInHandsOperation`)
//! puts the item to work part by part while the animator draws it, uses it
//! and puts it away. This is that controller and its operation, on the rig's
//! own animator; health does the work (`Health::use_item`).
//!
//! Without a rig (a host that draws no hands, or an install without the
//! item's bundle) the operation runs alone: the item goes to work and
//! finishes as with one, and nothing waits for the put-away but the way
//! back's own 0.6 s.

use std::collections::VecDeque;
use std::sync::Arc;

use crate::condition::health::{Event as HealthEvent, Health, Kind, Medicine, Part};
use crate::weapons::weapon_animator::{WeaponAnimator, WeaponAnimatorData};

/// What the item's hands tell the host.
#[derive(Debug, Clone, PartialEq)]
pub enum MedsEvent {
    /// The item is drawn: its draw's last mark (`WeapIn`).
    Appeared,
    /// Handed over: the item leaves the hands, and what is taken in hand
    /// next is made now, at the later of the put-away's mark (`WeapOut`,
    /// `HideWeaponComplete`) and the drop's end (`DropWithDelay`). After a
    /// use ended by itself, the hands held before the item (the owner's
    /// used-callback, `PlayerOwner.ValidateOnUsedCallback`: the item's
    /// `PreviousItem`, else the empty hands), or what a key asked for
    /// meanwhile.
    Hidden,
    Sound(String),
    BodyAction(i32),
}

/// How long a use asked to give way goes on before it stops
/// (`MedsController.DropWithDelay`: 600 ms).
pub const DROP_DELAY: f32 = 0.6;

/// What the way back after a use waits past its 600 ms, counted from the
/// frame after the finish's (health spec, "The hands"): the game's timer is
/// the wall clock's, begun inside the finish frame's health update and
/// continued on the main thread after a later frame's behaviours, and
/// neither offset is recorded. ESTIMATE: the two uses the drop decides
/// hand over 2.3 to 8.8 ms past 0.6 s of frames (`courses8`'s Propital, the
/// medicine course's second Salewa), and the magazine's rounds, waited the
/// same way, end within 4.0 ms of theirs (78 frames for 0.646 s); 3 ms lies
/// inside both.
pub const RETURN_ALLOWANCE: f32 = 0.003;

/// The time the rig's animator is moved on by as it is made
/// (`MedsController.Setup`: `SkipTime(0.0001)`).
const SETUP_SKIP: f32 = 0.0001;

/// What a part is short of for the use to go a fifth faster
/// (`MedsInHandsOperation.GoNextBodyPart`).
const NEARLY_WHOLE: f32 = 10.0;

pub struct MedsHands {
    animator: Option<WeaponAnimator>,
    medicine: Arc<Medicine>,
    /// The item's number for health, what it holds, and for food or drink
    /// the share asked.
    item: u64,
    resource: f32,
    amount: Option<f32>,
    /// The parts still to go to, `None` for the common body
    /// (`_bodyPartQueue`).
    queue: VecDeque<Option<Part>>,
    /// The item's `AnimationVariantsNumber`.
    variants: i32,
    /// The operation's count of variants (`_roundRobin`) and the
    /// controller's (`_animationVariant`).
    round_robin: i32,
    variant: i32,
    /// The operation's state: the item put to work and listening to health
    /// (`started`, until `Finish` or a drop: `listening`), and over (its
    /// put-away's mark come, or the work refused: `over`, the operation's
    /// `Finished`).
    listening: bool,
    finished: bool,
    over: bool,
    in_hand: bool,
    /// Time left before a drop asked for takes effect, and whether one has.
    dropping: Option<f32>,
    dropped: bool,
    /// The drop begun inside this frame's health update (the way back after
    /// a finish): the frame's own time is not counted, as the game's wall
    /// clock had begun after the frame's time was taken.
    drop_from_next_frame: bool,
    /// Time left before a cancel asked for takes effect (`cancel`).
    cancelling: Option<f32>,
    drawn_smooth: bool,
    /// Whose eyes the character is seen with, which the animator is told
    /// as the hands spawn (`set_point_of_view`).
    point_of_view: crate::PointOfView,
}

impl MedsHands {
    /// The hands for an item, with its rig's animator if there is one;
    /// `take` puts it in hand. `variants`: its `AnimationVariantsNumber`.
    pub fn new(animator: Option<&WeaponAnimatorData>, medicine: Arc<Medicine>, variants: u32) -> Self {
        Self {
            animator: animator.map(WeaponAnimator::new),
            medicine,
            item: 0,
            resource: 0.0,
            amount: None,
            queue: VecDeque::new(),
            variants: variants as i32,
            round_robin: 0,
            variant: 0,
            listening: false,
            finished: false,
            over: false,
            in_hand: false,
            dropping: None,
            dropped: false,
            drop_from_next_frame: false,
            cancelling: None,
            drawn_smooth: false,
            point_of_view: crate::PointOfView::FirstPerson,
        }
    }

    /// As `EmptyHands::set_drawn_smooth`: the drawing alone.
    pub fn set_drawn_smooth(&mut self, on: bool) {
        self.drawn_smooth = on;
    }

    /// Whose eyes the character is seen with: the next `take` tells the
    /// animator, whose states blend a clip for one's own eyes and one for
    /// others' by it (`MedsController.Spawn`: `SetPointOfViewOnSpawn`, the
    /// player's own; the Slickers' `USE`, `item_slickers_use` and
    /// `item_slickers_use_3rd` by `ThirdPerson`). No other hands tell theirs.
    pub fn set_point_of_view(&mut self, point_of_view: crate::PointOfView) {
        self.point_of_view = point_of_view;
    }

    /// Takes the item in hand and puts it to work (`MedsController.Setup`
    /// and `Spawn`, `MedsInHandsOperation.Start`): the rig's animator made
    /// afresh and moved on by a ten-thousandth of a second, the variants'
    /// count started at `first` (if none, a random one of `0..variants`
    /// by health's chance, as the game's), the item set to work on the
    /// first part of `parts`
    /// (`None` the common body: what a quick-use key asks), and the
    /// animator told to draw it. `resource`: what the item holds;
    /// `amount`: food's or drink's share (the whole if none).
    ///
    /// Returns the medicine effect's number, or nothing if health refused
    /// the item: the operation is then over with the item still in hand,
    /// as the game's (`FailedToApply`), and its used-callback asks for the
    /// hands before at once, so the item goes 0.6 s on (`drop`).
    pub fn take(&mut self, health: &mut Health, item: u64, resource: f32, parts: &[Option<Part>], amount: Option<f32>, first: Option<i32>) -> Option<u32> {
        if let Some(animator) = &mut self.animator {
            animator.reset();
            animator.update(SETUP_SKIP);
            // As the hands spawn: whose eyes see them.
            let outside = self.point_of_view == crate::PointOfView::ThirdPerson;
            animator.set("ThirdPerson", outside as u8 as f32);
        }
        self.item = item;
        self.resource = resource;
        // Medicine is used whole; food and drink by the share asked, at
        // most what is left in it.
        self.amount = if self.medicine.food() {
            let left = if self.medicine.max_resource > 0.0 { resource / self.medicine.max_resource } else { 1.0 };
            Some(amount.unwrap_or(1.0).clamp(0.0, left))
        } else {
            None
        };
        self.queue = parts.iter().copied().collect();
        self.round_robin = first.unwrap_or_else(|| health.random_whole(0, self.variants));
        self.listening = false;
        self.finished = false;
        self.over = false;
        self.in_hand = true;
        self.dropping = None;
        self.dropped = false;
        self.drop_from_next_frame = false;
        self.cancelling = None;
        let started = self.go_next_part(health);
        if started.is_some() {
            self.listening = true;
            self.set_flag("Active", true);
        } else {
            self.drop();
        }
        started
    }

    /// The next part on the list put to work (`GoNextBodyPart`): the use
    /// sped up by the Surgery skill and by a fifth where the part (the
    /// common body: all of them) is short of under 10, and the next
    /// variant taken.
    fn go_next_part(&mut self, health: &mut Health) -> Option<u32> {
        let head = self.queue.front().copied().flatten();
        let started = health.use_item(self.medicine.clone(), self.item, self.resource, head, self.amount);
        if started.is_none() {
            self.over = true;
            return None;
        }
        let short = match head {
            Some(part) => health.part(part).maximum - health.part(part).current,
            None => Part::ALL.iter().map(|p| health.part(*p).maximum - health.part(*p).current).sum(),
        };
        // `SurgerySpeed.Value / 100`, as the game has it: the skill's own
        // figure (0.16 at level 40) taken for a percentage.
        let mut speed = 1.0 + health.skills.surgery_speed / 100.0;
        if short < NEARLY_WHOLE {
            speed += 0.2;
        }
        self.set("UseTimeMultiplier", speed);
        self.round_robin += 1;
        self.variant = self.repeat(self.round_robin);
        self.set("AnimationVariant", self.variant as f32);
        started
    }

    /// `Mathf.Repeat` over the variants; an item with none has no
    /// parameter for it, and its variant stays 0.
    fn repeat(&self, count: i32) -> i32 {
        if self.variants > 0 { count.rem_euclid(self.variants) } else { 0 }
    }

    fn set(&mut self, parameter: &str, value: f32) {
        if let Some(animator) = &mut self.animator {
            animator.set(parameter, value);
        }
    }

    fn set_flag(&mut self, parameter: &str, on: bool) {
        if let Some(animator) = &mut self.animator {
            animator.set_flag(parameter, on);
        }
    }

    /// What health did, for the item at work: a medicine effect taken off
    /// (`OnEffectRemoved`) moves on to the next part the item can still
    /// help, telling the animator a next limb, or finishes; what the item
    /// holds is kept; the item used up finishes it (`OnRemoveItem`).
    pub fn on_health(&mut self, health: &mut Health, event: &HealthEvent) {
        match event {
            HealthEvent::Resource(item, left) if *item == self.item => self.resource = *left,
            HealthEvent::UsedUp(item) if *item == self.item && self.listening => self.finish(),
            HealthEvent::Removed(Kind::MedEffect, _, _) if self.listening => {
                // The parts it can no longer help leave the list's head
                // (`NeedDequeue`): the common body always.
                while let Some(head) = self.queue.front().copied() {
                    let done = match head {
                        None => true,
                        Some(part) => health.part_to_apply(&self.medicine, self.resource, Some(part)).is_none(),
                    };
                    if !done {
                        break;
                    }
                    self.queue.pop_front();
                }
                // A medkit with resource left goes on (`HaveToContinue`).
                let more = !self.queue.is_empty() && self.medicine.max_hp_resource > 0.0 && self.resource >= f32::EPSILON;
                if self.animator.as_ref().is_some_and(|a| a.has("NextLimb")) {
                    self.set_flag("Active", false);
                    self.set_flag("NextLimb", more);
                }
                if more {
                    // Refused there, the operation is over and its
                    // used-callback asks for the hands before.
                    if self.go_next_part(health).is_none() {
                        self.go_back();
                    }
                } else {
                    self.finish();
                }
            }
            _ => {}
        }
    }

    /// The use is done (`Finish`): the animator puts the item away, and the
    /// used-callback asks for the hands before at once (`go_back`).
    fn finish(&mut self) {
        self.set_flag("NextLimb", false);
        self.set_flag("Active", false);
        if !self.finished {
            self.finished = true;
            self.listening = false;
            self.go_back();
        }
    }

    /// The way back after a use (the owner's used-callback,
    /// `PlayerOwner.ValidateOnUsedCallback`, run inside health's update):
    /// the hands held before are asked for (`Player.SetInHands`), and the
    /// item's controller is dropped as for any other ask
    /// (`MedsController.Drop`), so they come 0.6 s on by the wall clock or
    /// at the put-away's mark, whichever is later. A drop already asked (a
    /// key meanwhile: `TryProceed` emptied the callback) is left to run,
    /// with no second wait.
    fn go_back(&mut self) {
        if self.in_hand && self.dropping.is_none() && !self.dropped {
            self.dropping = Some(DROP_DELAY + RETURN_ALLOWANCE);
            self.drop_from_next_frame = true;
        }
    }

    /// Something else is to be taken in hand (`Drop`): 0.6 s later the
    /// item's work is cut short and the animator puts it away, or plays a
    /// draw under way back; at its put-away's mark it is gone (at once if
    /// the mark has come). Asked while a drop runs (the way back's too),
    /// nothing more: what was asked comes as it ends.
    pub fn drop(&mut self) {
        if self.in_hand && self.dropping.is_none() && !self.dropped {
            self.dropping = Some(DROP_DELAY);
        }
    }

    /// The use cut short by the player (`MedsInputTranslator`: the shoot
    /// key's or the aim key's command, `ToggleShooting`,
    /// `ToggleAlternativeShooting`, or the scope's, `ChangeScope`, call
    /// `MedsController.Remove`): 0.6 s later (`RemoveWithDelay`) the parts'
    /// list is emptied and the item's work stopped
    /// (`MedsInHandsOperation.Remove`: `CancelApplyingItem`, the local
    /// player's `RemoveMedEffect`); health then takes the effect off, which
    /// finishes the use, and the animator puts the item away. What the item
    /// did before is kept. Asked again meanwhile, nothing more.
    pub fn cancel(&mut self) {
        if self.in_hand && self.listening && self.cancelling.is_none() {
            self.cancelling = Some(DROP_DELAY);
        }
    }

    /// The drop taking effect (`HideWeapon`): the item's work stopped, and
    /// the hands handed over at once if the operation is over (its put-away's
    /// mark come, or the work refused) or there is no rig; else at the mark.
    fn hide(&mut self, health: &mut Health, events: &mut Vec<MedsEvent>) {
        self.dropped = true;
        health.cancel_items();
        self.listening = false;
        self.set_flag("Active", false);
        if self.over || self.animator.is_none() {
            self.hand_over(events);
        }
    }

    /// The put-away's mark (`HideWeaponComplete`): the operation is over.
    /// Before the drop has taken effect nothing waits on it yet, and the item
    /// stays in hand, its last state playing on (the Propital's `END`).
    fn put_away(&mut self, events: &mut Vec<MedsEvent>) {
        self.over = true;
        if self.dropped {
            self.hand_over(events);
        }
    }

    fn hand_over(&mut self, events: &mut Vec<MedsEvent>) {
        if self.in_hand {
            self.over = true;
            self.in_hand = false;
            events.push(MedsEvent::Hidden);
        }
    }

    pub fn in_hand(&self) -> bool {
        self.in_hand
    }

    /// Whether the item is at work, health listened to.
    pub fn at_work(&self) -> bool {
        self.listening
    }

    /// Whether a drop has been asked for.
    pub fn going(&self) -> bool {
        self.dropping.is_some() || self.dropped
    }

    pub fn item(&self) -> u64 {
        self.item
    }

    pub fn medicine(&self) -> &Arc<Medicine> {
        &self.medicine
    }

    /// The variant the controller holds (`AnimationVariant`), for the body.
    pub fn variant(&self) -> i32 {
        self.variant
    }

    pub fn animator(&self) -> Option<&WeaponAnimator> {
        self.animator.as_ref()
    }

    /// One frame of `dt` seconds. `pitch` is the view's, as the animator is
    /// told every frame (`SetAimAngle`): the surgical kit's clips are a
    /// blend by it. The rig's animator first (the hands' update), then the
    /// drop's and the cancel's 0.6 s, whose ends the game runs after the
    /// frame's behaviours (`Task.Delay` continued on Unity's
    /// synchronization context).
    pub fn step(&mut self, health: &mut Health, dt: f32, pitch: f32) -> Vec<MedsEvent> {
        let mut events = Vec::new();
        if !self.in_hand {
            return events;
        }
        if let Some(animator) = &mut self.animator {
            animator.set("Aim_angle", pitch);
            animator.set_drawn_smooth(self.drawn_smooth);
            animator.update(dt);
        }
        while let Some(mark) = self.animator.as_mut().and_then(|a| a.deliver_next()) {
            match mark.name.as_str() {
                // The draw begins (`OnOutUse`): the controller's handler,
                // then the operation's while it listens, both raising
                // `Active` and lowering `NextLimb`; the controller's takes
                // the next variant by its own count.
                "OutUse" => {
                    self.set_flag("Active", true);
                    self.set_flag("NextLimb", false);
                    self.variant = self.repeat(self.variant + 1);
                    self.set("AnimationVariant", self.variant as f32);
                }
                "WeapIn" => events.push(MedsEvent::Appeared),
                "WeapOut" => {
                    self.put_away(&mut events);
                    if !self.in_hand {
                        return events;
                    }
                }
                "Sound" => events.push(MedsEvent::Sound(mark.text)),
                "ThirdAction" => events.push(MedsEvent::BodyAction(mark.int)),
                _ => {}
            }
        }
        if let Some(left) = self.dropping {
            let left = if std::mem::take(&mut self.drop_from_next_frame) { left } else { left - dt };
            if left <= 0.0 {
                self.dropping = None;
                self.hide(health, &mut events);
            } else {
                self.dropping = Some(left);
            }
        }
        if let Some(left) = self.cancelling {
            if left - dt <= 0.0 {
                self.cancelling = None;
                self.queue.clear();
                health.cancel_items();
            } else {
                self.cancelling = Some(left - dt);
            }
        }
        events
    }
}

#[cfg(test)]
mod tests;
