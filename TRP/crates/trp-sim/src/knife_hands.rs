//! The hands that hold a knife or another melee weapon
//! (`docs/spec/grenades-and-knife.md`, "The knife's hands"): its rig from
//! the item's prefab, its animator, and its controller (`KnifeController`):
//! drawn, at rest, a slash on the left button and a stab on the right, the
//! blade live between the stroke's two marks, a hit glancing off; and the
//! quick stroke (`QuickKnifeKickController`), one stroke and back. This is
//! that controller on the rig's own animator. The host sweeps the blade for
//! what it meets while it is live (`KnifeEvent::Live`) and says when it hit
//! (`hit`).

use crate::weapon_animator::{WeaponAnimator, WeaponAnimatorData};

/// The engine's physics step (`Time.fixedDeltaTime`: a sixtieth of a second
/// in every recording's header, `fixed_dt`), which the quick stroke's
/// animator is moved on by as it is made.
const FIXED_DT: f32 = 1.0 / 60.0;

/// A stroke's kind (`EKickType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stroke {
    Slash,
    Stab,
}

/// What a melee weapon's item says of its strokes (its `KnifeComponent`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KnifeSettings {
    /// How far each stroke reaches (`PrimaryDistance`, `SecondryDistance`).
    pub slash_reach: f32,
    pub stab_reach: f32,
    /// What each stroke costs (`PrimaryConsumption`, `SecondryConsumption`),
    /// and a stroke glancing off a deflecting surface besides
    /// (`DeflectionConsumption`).
    pub slash_cost: f32,
    pub stab_cost: f32,
    pub deflection_cost: f32,
    /// Each stroke's damage (`knifeHitSlashDam`, `knifeHitStabDam`).
    pub slash_damage: f32,
    pub stab_damage: f32,
}

impl KnifeSettings {
    /// A melee weapon's figures from its item, as the database has them.
    pub fn of(props: &serde_json::Value) -> Self {
        let number = |name: &str| props[name].as_f64().unwrap_or(0.0) as f32;
        Self {
            slash_reach: number("PrimaryDistance"),
            stab_reach: number("SecondryDistance"),
            slash_cost: number("PrimaryConsumption"),
            stab_cost: number("SecondryConsumption"),
            deflection_cost: number("DeflectionConsumption"),
            slash_damage: number("knifeHitSlashDam"),
            stab_damage: number("knifeHitStabDam"),
        }
    }
}

/// What the knife's hands tell the host.
#[derive(Debug, Clone, PartialEq)]
pub enum KnifeEvent {
    /// Drawn: the draw's mark (`WeapIn`).
    Appeared,
    /// Put away (`WeapOut`): what is taken in hand next is made now.
    Hidden,
    /// The blade is live (`FiringBullet`): the stroke's stamina is to be
    /// spent (`ConsumeAsMelee`) and the host sweeps the blade every frame,
    /// by this reach along the look, until `Dead`; at most one hit a stroke.
    Live { stroke: Stroke, reach: f32, cost: f32 },
    /// The stroke is over (`FireEnd`).
    Dead,
    /// The quick stroke is over: the hands go back to what they held.
    Used,
    Sound(String),
    BodyAction(i32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Operation {
    Away,
    Spawn,
    Idle,
    /// A stroke (`Kick`).
    Kick,
    /// The quick stroke (`QuickKnifeKickOperation`), until its end lets
    /// the controller go.
    Quick,
    Remove,
}

pub struct KnifeHands {
    animator: WeaponAnimator,
    settings: KnifeSettings,
    operation: Operation,
    stroke: Stroke,
    /// The blade is live and has not hit yet.
    live: bool,
    hide_asked: bool,
    drawn_smooth: bool,
}

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
    }

    /// The quick stroke (`QuickKnifeKickController.Spawn`, `Start`): the
    /// animator made afresh, moved on by a physics step, told to strike at
    /// once (`QuickFire`, not `Active`) at the swing's `speed`
    /// (`Physical.MeleeSpeed`). It strikes as a slash reaching as one at a
    /// stab's cost.
    pub fn draw_quick(&mut self, speed: f32) {
        self.animator.reset();
        self.animator.set_flag("QuickFire", true);
        self.animator.set_flag("Active", false);
        self.animator.set("Deflected", 0.0);
        self.animator.set("SwingSpeed", speed);
        self.animator.update(FIXED_DT);
        self.animator.deliver();
        self.stroke = Stroke::Slash;
        self.hide_asked = false;
        self.live = false;
        self.operation = Operation::Quick;
    }

    /// A button at rest: a slash or a stab (`MakeKnifeKick`,
    /// `MakeAlternativeKick`), if the character `can` strike
    /// (`Physical.CanMeleeHit`), at the swing's `speed`. Returns whether it
    /// struck.
    pub fn strike(&mut self, stroke: Stroke, can: bool, speed: f32) -> bool {
        if self.operation != Operation::Idle || !can {
            return false;
        }
        self.stroke = stroke;
        self.animator.set_flag(if stroke == Stroke::Slash { "Fire" } else { "AltFire" }, true);
        self.animator.set("Deflected", 0.0);
        self.animator.set("SwingSpeed", speed);
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
        self.animator.set("Deflected", 1.0);
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

    fn remove(&mut self) {
        self.animator.set_flag("Active", false);
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
        let mut events = Vec::new();
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
                ("FireEnd", Operation::Kick) => {
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
