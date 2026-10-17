//! The hands that use an item: medicine, food or drink (`docs/spec/health.md`,
//! "The hands"). To the game the item is taken in hand as a weapon is: a rig
//! of its own, made from the item's `UsePrefab`, with an animator, and a
//! controller (`MedsController`) whose one operation (`MedsInHandsOperation`)
//! puts the item to work part by part while the animator draws it, uses it
//! and puts it away. This is that controller and its operation, on the rig's
//! own animator; health does the work (`Health::use_item`).
//!
//! Without a rig (a host that draws no hands, or an install without the
//! item's bundle) the operation runs alone: the item goes to work and
//! finishes as with one, and nothing waits for the put-away.

use std::collections::VecDeque;
use std::sync::Arc;

use crate::health::{Event as HealthEvent, Health, Kind, Medicine, Part};
use crate::weapon_animator::{WeaponAnimator, WeaponAnimatorData};

/// What the item's hands tell the host.
#[derive(Debug, Clone, PartialEq)]
pub enum MedsEvent {
    /// The item is drawn: its draw's last mark (`WeapIn`).
    Appeared,
    /// Put away (`WeapOut`, `HideWeaponComplete`): the item leaves the
    /// hands, and what is taken in hand next is made now; after a use ended
    /// by itself, the last weapon held (`SetControllerInsteadRemovedOne`).
    Hidden,
    Sound(String),
    BodyAction(i32),
}

/// How long a use asked to give way goes on before it stops
/// (`MedsController.DropWithDelay`: 600 ms).
pub const DROP_DELAY: f32 = 0.6;

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
    /// (`started`, until `Finish` or a drop: `listening`), and over (put
    /// away, or the work refused: `over`).
    listening: bool,
    finished: bool,
    over: bool,
    in_hand: bool,
    /// Time left before a drop asked for takes effect, and whether one has.
    dropping: Option<f32>,
    dropped: bool,
    /// Time left before a cancel asked for takes effect (`cancel`).
    cancelling: Option<f32>,
    drawn_smooth: bool,
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
            cancelling: None,
            drawn_smooth: false,
        }
    }

    /// As `EmptyHands::set_drawn_smooth`: the drawing alone.
    pub fn set_drawn_smooth(&mut self, on: bool) {
        self.drawn_smooth = on;
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
    /// as the game's (`FailedToApply`).
    pub fn take(&mut self, health: &mut Health, item: u64, resource: f32, parts: &[Option<Part>], amount: Option<f32>, first: Option<i32>) -> Option<u32> {
        if let Some(animator) = &mut self.animator {
            animator.reset();
            animator.update(SETUP_SKIP);
            // As the hands spawn: first person.
            animator.set("ThirdPerson", 0.0);
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
        self.cancelling = None;
        let started = self.go_next_part(health);
        if started.is_some() {
            self.listening = true;
            self.set_flag("Active", true);
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
                    self.go_next_part(health);
                } else {
                    self.finish();
                }
            }
            _ => {}
        }
    }

    /// The use is done (`Finish`): the animator puts the item away.
    fn finish(&mut self) {
        self.set_flag("NextLimb", false);
        self.set_flag("Active", false);
        if !self.finished {
            self.finished = true;
            self.listening = false;
        }
    }

    /// Something else is to be taken in hand (`Drop`): 0.6 s later the
    /// item's work is cut short and the animator puts it away, or plays a
    /// draw under way back; at its put-away's mark it is gone.
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

    /// The drop taking effect (`HideWeapon`).
    fn hide(&mut self, health: &mut Health, events: &mut Vec<MedsEvent>) {
        self.dropped = true;
        health.cancel_items();
        self.listening = false;
        self.set_flag("Active", false);
        if self.over || self.animator.is_none() {
            self.put_away(events);
        }
    }

    fn put_away(&mut self, events: &mut Vec<MedsEvent>) {
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
    /// blend by it.
    pub fn step(&mut self, health: &mut Health, dt: f32, pitch: f32) -> Vec<MedsEvent> {
        let mut events = Vec::new();
        if !self.in_hand {
            return events;
        }
        if let Some(left) = self.dropping {
            if left - dt <= 0.0 {
                self.dropping = None;
                self.hide(health, &mut events);
            } else {
                self.dropping = Some(left - dt);
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
        let Some(animator) = &mut self.animator else {
            // No rig: the item is gone as its use is done.
            if self.finished {
                self.put_away(&mut events);
            }
            return events;
        };
        animator.set("Aim_angle", pitch);
        animator.set_drawn_smooth(self.drawn_smooth);
        animator.update(dt);
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
                    break;
                }
                "Sound" => events.push(MedsEvent::Sound(mark.text)),
                "ThirdAction" => events.push(MedsEvent::BodyAction(mark.int)),
                _ => {}
            }
        }
        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::health::{Cure, CureSpec, DamageKind, MedClass};

    fn ifak() -> Arc<Medicine> {
        Arc::new(Medicine {
            template: "ifak".into(),
            class: MedClass::MedKit,
            use_time: 3.0,
            part_time: Vec::new(),
            max_hp_resource: 300.0,
            hp_resource_rate: 50.0,
            part_priority: Vec::new(),
            energy: 0.0,
            hydration: 0.0,
            cures: vec![(Cure::LightBleeding, CureSpec { delay: 0.0, duration: 0.0, fade_out: 0.0, cost: 30.0, kept: [0, 0] })],
            max_resource: 0.0,
            buffs: None,
        })
    }

    /// Steps health and the hands together, as a host does, for so long;
    /// whether the hands were put away meanwhile.
    fn run(health: &mut Health, hands: &mut MedsHands, seconds: f32) -> bool {
        let mut hidden = false;
        for _ in 0..(seconds * 60.0).round() as usize {
            health.step(1.0 / 60.0);
            for event in health.take_events() {
                hands.on_health(health, &event);
            }
            hidden |= hands.step(health, 1.0 / 60.0, 0.0).contains(&MedsEvent::Hidden);
        }
        hidden
    }

    #[test]
    fn a_cancel_stops_the_use_six_tenths_of_a_second_later_keeping_what_it_did() {
        let mut health = Health::new(crate::health::tests::settings(), 1);
        health.apply_damage(Part::Chest, 20.0, DamageKind::Fall);
        health.apply_damage(Part::LeftArm, 20.0, DamageKind::Fall);
        let mut hands = MedsHands::new(None, ifak(), 0);
        assert!(hands.take(&mut health, 9, 300.0, &[Some(Part::Chest), Some(Part::LeftArm)], None, None).is_some());
        // Into the chest's work (a 2 s delay, then 3 s), the cancel asked.
        assert!(!run(&mut health, &mut hands, 3.0));
        let healed = health.part(Part::Chest).current;
        hands.cancel();
        // Not yet: 0.6 s go by first.
        assert!(!run(&mut health, &mut hands, 0.5));
        assert!(hands.at_work(), "still at work before the delay is up");
        // Then the work stops and the item is gone; the arm never begun.
        assert!(run(&mut health, &mut hands, 1.0));
        assert!(!hands.in_hand() && !hands.at_work());
        assert!(health.existing(Kind::MedEffect, Some(Part::LeftArm)).is_none(), "the next part is not gone on to");
        assert!(health.part(Part::Chest).current >= healed && health.part(Part::Chest).current < 85.0, "what was healed is kept, the rest not");
        assert_eq!(health.part(Part::LeftArm).current, 40.0);
    }

    #[test]
    fn a_kit_goes_on_to_the_next_part_named_then_finishes() {
        let mut health = Health::new(crate::health::tests::settings(), 1);
        health.apply_damage(Part::Chest, 20.0, DamageKind::Fall);
        health.apply_damage(Part::LeftArm, 20.0, DamageKind::Fall);
        let mut hands = MedsHands::new(None, ifak(), 0);
        assert!(hands.take(&mut health, 9, 300.0, &[Some(Part::Chest), Some(Part::LeftArm)], None, None).is_some());
        // The chest done (a 2 s delay and 3 s of work), the arm is begun.
        assert!(!run(&mut health, &mut hands, 5.2));
        assert_eq!(health.part(Part::Chest).current, 85.0);
        assert!(health.existing(Kind::MedEffect, Some(Part::LeftArm)).is_some(), "the next part at work");
        assert!(hands.at_work());
        // The arm done, the list is over: finished, and with no rig to
        // play the put-away, gone at once.
        assert!(run(&mut health, &mut hands, 5.2));
        assert_eq!(health.part(Part::LeftArm).current, 60.0);
        assert!(!hands.in_hand() && !hands.at_work());
    }

    #[test]
    fn the_common_body_is_left_after_one_part() {
        let mut health = Health::new(crate::health::tests::settings(), 1);
        health.apply_damage(Part::Chest, 20.0, DamageKind::Fall);
        health.apply_damage(Part::LeftArm, 20.0, DamageKind::Fall);
        let mut hands = MedsHands::new(None, ifak(), 0);
        assert!(hands.take(&mut health, 9, 300.0, &[None], None, None).is_some());
        assert!(run(&mut health, &mut hands, 5.2), "put away after the first part");
        assert!(health.existing(Kind::MedEffect, None).is_none());
        // The part health chose is whole, the other waits for another use.
        let whole = [(Part::Chest, 85.0), (Part::LeftArm, 60.0)].iter().filter(|(part, top)| health.part(*part).current == *top).count();
        assert_eq!(whole, 1, "one part healed");
    }

    #[test]
    fn a_use_given_way_stops_six_tenths_of_a_second_on() {
        let mut health = Health::new(crate::health::tests::settings(), 1);
        health.apply_damage(Part::Chest, 20.0, DamageKind::Fall);
        let mut hands = MedsHands::new(None, ifak(), 0);
        assert!(hands.take(&mut health, 9, 300.0, &[None], None, None).is_some());
        assert!(!run(&mut health, &mut hands, 1.0));
        hands.drop();
        assert!(!run(&mut health, &mut hands, 0.55));
        assert!(health.existing(Kind::MedEffect, None).is_some(), "still at work");
        assert!(run(&mut health, &mut hands, 0.1));
        assert!(health.existing(Kind::MedEffect, None).is_none_or(|e| !e.existing()), "cut short");
        assert_eq!(health.part(Part::Chest).current, 65.0, "nothing healed");
    }
}
