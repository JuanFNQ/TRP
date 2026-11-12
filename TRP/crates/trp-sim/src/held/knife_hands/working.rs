//! Making the knife hands, drawing, striking, hiding and stepping them.

use super::*;

impl KnifeHands {
    pub fn new(animator: &WeaponAnimatorData, settings: KnifeSettings) -> Self {
        Self {
            animator: WeaponAnimator::new(animator),
            settings,
            operation: Operation::Away,
            stroke: Stroke::Slash,
            live: false,
            hide_asked: false,
            drawn_smooth: false,
            melee_speed: 1.0,
            told_body: Vec::new(),
            swing: Swing::default(),
        }
    }

    /// A stroke's frame, after its sweep (`Kick.Update`, after
    /// `KnifeCollider.ManualUpdate`): the swing's way eased by 0.9 toward
    /// the blade's step since the last such frame, `at` the blade's object
    /// where the sweep had it, and kept as the step's start. Only a slash or
    /// a stab does it; the quick stroke's operation never does.
    pub fn follow_blade(&mut self, at: [f32; 3]) {
        if self.operation == Operation::Kick {
            self.swing.follow(at);
        }
    }

    /// A hit's direction (`ProcessHit`'s `GlobalSwingDirection`, eased once
    /// more by the blade's step at the hit, and kept): not a unit, and from
    /// a quick stroke, whose controller starts at nought and never follows
    /// the blade, 0.9 of the way from the world's origin to the blade.
    pub fn hit_direction(&mut self, at: [f32; 3]) -> [f32; 3] {
        self.swing.at_hit(at)
    }

    /// The stroke's penetration (`SlashPenetration`, `StabPenetration`): the
    /// quick stroke strikes as a slash.
    pub fn penetration(&self) -> f32 {
        match self.stroke {
            Stroke::Slash => self.settings.slash_penetration,
            Stroke::Stab => self.settings.stab_penetration,
        }
    }

    /// The controller made anew: its swing at nought.
    fn new_controller(&mut self) {
        self.swing = Swing::default();
    }

    /// The character's melee speed now (`Physical.MeleeSpeed`: 1, or the
    /// exhausted hands' figure), for what the controller tells the body
    /// outside a stroke.
    pub fn set_melee_speed(&mut self, speed: f32) {
        self.melee_speed = speed;
    }

    /// The swing's speed and the glance told both animators at once
    /// (`SetMeleeSpeed`, `SetDeflected`).
    fn tell_swing(&mut self, deflected: Option<bool>, speed: Option<f32>) {
        if let Some(deflected) = deflected {
            self.animator.set("Deflected", deflected as u8 as f32);
            self.told_body.push(KnifeEvent::BodyDeflected(deflected));
        }
        if let Some(speed) = speed {
            self.animator.set("SwingSpeed", speed);
            self.told_body.push(KnifeEvent::BodySwingSpeed(speed));
        }
    }

    pub fn set_drawn_smooth(&mut self, on: bool) {
        self.drawn_smooth = on;
    }

    pub fn settings(&self) -> &KnifeSettings {
        &self.settings
    }

    /// Draws the knife (`SpawnOperation`): the animator made afresh and told
    /// it is active.
    pub fn draw(&mut self) {
        self.animator.reset();
        self.animator.set_flag("Active", true);
        self.hide_asked = false;
        self.live = false;
        self.operation = Operation::Spawn;
        self.new_controller();
    }

    /// In hand and at rest from the start, as a character is made with it
    /// (no draw played).
    pub fn start_drawn(&mut self) {
        self.animator.reset();
        self.animator.set_flag("Active", true);
        self.animator.play("Hands.IDLE", 0.0);
        self.hide_asked = false;
        self.live = false;
        self.operation = Operation::Idle;
        self.new_controller();
    }

    /// The quick stroke (`QuickKnifeKickController.Spawn`, `Start`): the
    /// animator made afresh, moved on by a physics step, told to strike at
    /// once (`QuickFire`, not `Active`) at the swing's `speed`
    /// (`Physical.MeleeSpeed`). It strikes as a slash reaching as one at a
    /// stab's cost. The body is told the swing's speed and no glance too.
    pub fn draw_quick(&mut self, speed: f32) {
        self.animator.reset();
        self.animator.set_flag("QuickFire", true);
        self.animator.set_flag("Active", false);
        self.melee_speed = speed;
        self.tell_swing(Some(false), Some(speed));
        self.animator.update(FIXED_DT);
        self.animator.deliver();
        self.stroke = Stroke::Slash;
        self.hide_asked = false;
        self.live = false;
        self.operation = Operation::Quick;
        self.new_controller();
    }

    /// A button at rest: a slash or a stab (`MakeKnifeKick`,
    /// `MakeAlternativeKick`), if the character `can` strike
    /// (`Physical.CanMeleeHit`), at the swing's `speed`, told the body's
    /// animator too (`Kick.Start`). Returns whether it struck.
    pub fn strike(&mut self, stroke: Stroke, can: bool, speed: f32) -> bool {
        if self.operation != Operation::Idle || !can {
            return false;
        }
        self.stroke = stroke;
        self.animator.set_flag(if stroke == Stroke::Slash { "Fire" } else { "AltFire" }, true);
        self.melee_speed = speed;
        self.tell_swing(Some(false), Some(speed));
        self.operation = Operation::Kick;
        true
    }

    /// The host's sweep met something with a ballistic collider: the hit is
    /// made (its damage returned, the stroke's, before the host's own
    /// factors) and the stroke glances off (`Deflected`). Only the first hit
    /// of a stroke counts; after it the blade is dead.
    pub fn hit(&mut self) -> Option<f32> {
        if !self.live {
            return None;
        }
        self.live = false;
        self.tell_swing(Some(true), None);
        Some(if self.stroke == Stroke::Slash { self.settings.slash_damage } else { self.settings.stab_damage })
    }

    /// The knife gives way to something else (`HideWeapon`): at rest at once
    /// (`Remove`), in a stroke when it is over.
    pub fn hide(&mut self) {
        match self.operation {
            Operation::Away | Operation::Remove => {}
            Operation::Idle => self.remove(),

            _ => self.hide_asked = true,
        }
    }

    /// The put-away (`Remove.Start`): the animator told the knife is no
    /// longer active, and both animators the character's melee speed.
    fn remove(&mut self) {
        self.animator.set_flag("Active", false);
        self.tell_swing(None, Some(self.melee_speed));
        self.hide_asked = false;
        self.operation = Operation::Remove;
    }

    pub fn set_inventory(&mut self, open: bool) {
        if self.operation == Operation::Idle {
            self.animator.set_flag("Inventory", open);
        }
    }

    pub fn look(&mut self) {
        if self.operation == Operation::Idle {
            self.animator.trigger("Look");
        }
    }

    pub fn in_hand(&self) -> bool {
        self.operation != Operation::Away
    }

    pub fn idling(&self) -> bool {
        self.operation == Operation::Idle
    }

    /// Whether the body is told the hands are busy (`ReloadFloat` 1): from
    /// the draw's start to its mark, and from the put-away's start
    /// (`SpawnOperation.Start`, `WeaponAppeared`, `Remove.Start`). A stroke
    /// does not tell it.
    pub fn hands_busy(&self) -> bool {
        matches!(self.operation, Operation::Spawn | Operation::Remove)
    }

    /// Whether the blade is live and has not hit yet: the host sweeps it.
    pub fn live(&self) -> bool {
        self.live
    }

    pub fn stroke(&self) -> Stroke {
        self.stroke
    }

    pub fn operation_name(&self) -> &'static str {
        match self.operation {
            Operation::Away => "",
            Operation::Spawn => "SpawnOperation",
            Operation::Idle => "Idling",
            Operation::Kick => "Kick",
            Operation::Quick => "QuickKnifeKickOperation",
            Operation::Remove => "Remove",
        }
    }

    pub fn animator(&self) -> &WeaponAnimator {
        &self.animator
    }

    pub fn step(&mut self, dt: f32) -> Vec<KnifeEvent> {
        let mut events = std::mem::take(&mut self.told_body);
        self.animator.set_drawn_smooth(self.drawn_smooth);
        self.animator.update(dt);
        for mark in self.animator.deliver() {
            match (mark.name.as_str(), self.operation) {
                ("WeapIn", Operation::Spawn) => {
                    self.operation = Operation::Idle;
                    events.push(KnifeEvent::Appeared);
                    if self.hide_asked {
                        self.remove();
                    }
                }
                ("WeapIn", Operation::Quick) => events.push(KnifeEvent::Appeared),
                ("FiringBullet", Operation::Kick) => {
                    self.live = true;
                    let (reach, cost) = match self.stroke {
                        Stroke::Slash => (self.settings.slash_reach, self.settings.slash_cost),
                        Stroke::Stab => (self.settings.stab_reach, self.settings.stab_cost),
                    };
                    events.push(KnifeEvent::Live { stroke: self.stroke, reach, cost });
                }
                ("FiringBullet", Operation::Quick) => {
                    self.live = true;
                    events.push(KnifeEvent::Live { stroke: Stroke::Slash, reach: self.settings.slash_reach, cost: self.settings.stab_cost });
                }
                // The stroke's end, or the rest's start where a stroke is
                // still under way (`Kick.OnIdleStart` calls its `OnFireEnd`):
                // a button taken between the stroke's end and its state's,
                // whose stroke the animator has not begun by the time it is
                // back at rest, is let go of there.
                ("FireEnd" | "IdleStart", Operation::Kick) => {
                    self.live = false;
                    self.animator.set_flag("Fire", false);
                    self.animator.set_flag("AltFire", false);
                    events.push(KnifeEvent::Dead);
                    self.operation = Operation::Idle;
                    if self.hide_asked {
                        self.remove();
                    }
                }
                ("FireEnd", Operation::Quick) => {
                    self.live = false;
                    self.animator.set_flag("QuickFire", false);
                    events.push(KnifeEvent::Dead);
                    events.push(KnifeEvent::Used);
                    // Its controller is let go as the hands go back.
                    self.operation = Operation::Away;
                }
                ("WeapOut", Operation::Remove) => {
                    self.operation = Operation::Away;
                    events.push(KnifeEvent::Hidden);
                }
                ("Sound", _) => events.push(KnifeEvent::Sound(mark.text)),
                ("ThirdAction", _) => events.push(KnifeEvent::BodyAction(mark.int)),
                _ => {}
            }
        }
        events
    }
}
