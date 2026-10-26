//! The hands that hold a grenade (`docs/spec/grenades-and-knife.md`, "The
//! grenade's hands"). To the game a grenade is taken in hand as a weapon
//! is: a rig of its own from the item's prefab (`GrenadePrefab`), an
//! animator, and a controller (`GrenadeHandsController`) whose operations
//! draw it, hold it idle, pull its pin for a throw from above or below and
//! wait on the button, throw it at the animator's mark, put the pin back,
//! and put it away. This is that controller on the rig's own animator; the
//! host makes the grenade that is thrown (`throw`), from what it is told
//! (`GrenadeEvent::Thrown`).

use crate::weapon_animator::{WeaponAnimator, WeaponAnimatorData};

/// What the grenade's hands tell the host.
#[derive(Debug, Clone, PartialEq)]
pub enum GrenadeEvent {
    /// Drawn: the draw's mark (`WeapIn`).
    Appeared,
    /// Put away (`WeapOut`): what is taken in hand next is made now.
    Hidden,
    /// The hand lets the grenade go (`FiringBullet`): the host makes the
    /// grenade thrown, from the hand's bones as they are now (`throw`).
    Thrown { low: bool },
    /// The throw's animation is over (`FireEnd`): the grenade is used up
    /// and the hands go back to what they held before it.
    Used,
    /// A mark of a sound, played by the rig's sound player.
    Sound(String),
    /// A mark of a sound played where the thrown grenade is (`SoundAtPoint`:
    /// the fuse).
    SoundAtPoint(String),
    BodyAction(i32),
}

/// The animator's values for a throw (`FirearmsAnimator.EGrenadeFire`).
const IDLE: f32 = 0.0;
const HOLD: f32 = 1.0;
const THROW: f32 = 2.0;
/// The weapon's mode for a grenade thrown (`Weapon.EFireMode.grenadeThrowing`),
/// which the idle tells the animator (planting a tripwire, 7, is not built).
const THROWING: f32 = 6.0;
/// What the animator is moved on by as the rig is made
/// (`BaseGrenadeHandsController.Setup`: `SkipTime(1 / 60)`).
const SETUP_SKIP: f32 = 1.0 / 60.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Throw {
    /// The pin pulled and the button held (`EThrowState.Idling`).
    Waiting,
    /// The button let go: the throw's animation plays (`Throwing`); at its
    /// end the grenade is used up and its controller let go.
    Throwing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Operation {
    Away,
    /// Drawn, until the draw's mark (`SpawnOperation`).
    Spawn,
    Idle,
    /// A throw from above, or with `low` from below (`HighThrowOperation`,
    /// `LowThrowOperation`).
    Throw { low: bool, stage: Throw },
    /// Being put away (`Remove`), until the mark.
    Remove,
}

pub struct GrenadeHands {
    animator: WeaponAnimator,
    operation: Operation,
    /// Whether the grenade may be put away, or the inventory opened, with
    /// its pin out and the button held (`CanBeHiddenDuringThrow`).
    hidden_in_throw: bool,
    /// A put-away asked for when it could not be done at once: done when
    /// the draw or the throw is over.
    hide_asked: bool,
    drawn_smooth: bool,
}

impl GrenadeHands {
    /// A grenade not yet in hand; `draw` brings it. `hidden_in_throw`: its
    /// item's `CanBeHiddenDuringThrow`.
    pub fn new(animator: &WeaponAnimatorData, hidden_in_throw: bool) -> Self {
        Self { animator: WeaponAnimator::new(animator), operation: Operation::Away, hidden_in_throw, hide_asked: false, drawn_smooth: false }
    }

    /// As `EmptyHands::set_drawn_smooth`: the drawing alone.
    pub fn set_drawn_smooth(&mut self, on: bool) {
        self.drawn_smooth = on;
    }

    /// Takes the grenade in hand (`Setup`, `SpawnOperation.Start`): the
    /// animator made afresh and moved on by a sixtieth of a second, then
    /// told it is active, which takes it from its first state into the draw.
    pub fn draw(&mut self) {
        self.animator.reset();
        self.animator.update(SETUP_SKIP);
        self.animator.deliver();
        self.animator.set_flag("Active", true);
        self.hide_asked = false;
        self.operation = Operation::Spawn;
    }

    /// A button goes down (`TryHighThrow`, `TryLowThrow`): at rest, the pin
    /// is pulled for a throw from above (`low` false) or below, and the
    /// throw waits on that button. Nothing else hears it.
    pub fn press(&mut self, low: bool) {
        if self.operation == Operation::Idle {
            self.animator.set(Self::parameter(low), HOLD);
            self.operation = Operation::Throw { low, stage: Throw::Waiting };
        }
    }

    /// A button comes up (`FinishHighThrow`, `FinishLowThrow`): the throw
    /// waiting on it goes (the animator's value to throw), whenever its
    /// animation lets it.
    pub fn release(&mut self, low: bool) {
        if let Operation::Throw { low: waiting_low, stage: Throw::Waiting } = self.operation {
            if waiting_low == low {
                self.animator.set(Self::parameter(low), THROW);
                self.operation = Operation::Throw { low, stage: Throw::Throwing };
            }
        }
    }

    fn parameter(low: bool) -> &'static str {
        if low { "GrenadeAltFire" } else { "GrenadeFire" }
    }

    /// The pin put back (`PutGrenadeBack`): both values to rest and the
    /// grenade idle; the animator plays the pin going back.
    fn put_back(&mut self) {
        self.animator.set("GrenadeAltFire", IDLE);
        self.animator.set("GrenadeFire", IDLE);
        self.operation = Operation::Idle;
        self.animator.set("FireMode", THROWING);
    }

    /// The grenade is to give way to something else (`HideGrenade`): at
    /// rest it goes at once (`Remove`: `Active` off, gone at `WeapOut`);
    /// waiting on the button, at once where the grenade allows it; drawn
    /// or thrown, when that is over; already let go of, it is gone.
    pub fn hide(&mut self) {
        match self.operation {
            Operation::Away | Operation::Remove => {}
            Operation::Idle => self.remove(),
            Operation::Throw { stage: Throw::Waiting, .. } if self.hidden_in_throw => self.remove(),

            _ => self.hide_asked = true,
        }
    }

    fn remove(&mut self) {
        self.animator.set_flag("Active", false);
        self.hide_asked = false;
        self.operation = Operation::Remove;
    }

    /// The inventory opens or shuts (`SetInventoryOpened`): at rest or
    /// drawn the animator is told; with the pin out where the grenade
    /// allows it, the animator is told and the pin goes back.
    pub fn set_inventory(&mut self, open: bool) {
        match self.operation {
            Operation::Idle | Operation::Spawn => self.animator.set_flag("Inventory", open),
            Operation::Throw { stage: Throw::Waiting, .. } if self.hidden_in_throw => {
                self.animator.set_flag("Inventory", open);
                self.put_back();
            }
            _ => {}
        }
    }

    /// The look at the thing in hand (`ExamineWeapon`), at rest.
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

    /// Whether the animator holds a throw, cooked or under way (its
    /// `GrenadeFire` at `Hold` or `Throw`): a vault is then refused but
    /// under the move's height without hands (`VaultingGameplayRestrictions.
    /// CheckGrenadeOperation`; vaulting spec, "What a try does").
    pub fn holds_throw(&self) -> bool {
        let fire = self.animator.get("GrenadeFire");
        fire == HOLD || fire == THROW
    }

    /// Whether a throw waits on its button, and which (`WaitingForHighThrow`,
    /// `WaitingForLowThrow`).
    pub fn waiting(&self) -> Option<bool> {
        match self.operation {
            Operation::Throw { low, stage: Throw::Waiting } => Some(low),
            _ => None,
        }
    }

    /// The operation's name as the game's.
    pub fn operation_name(&self) -> &'static str {
        match self.operation {
            Operation::Away => "",
            Operation::Spawn => "SpawnOperation",
            Operation::Idle => "Idling",
            Operation::Throw { low: false, .. } => "HighThrowOperation",
            Operation::Throw { low: true, .. } => "LowThrowOperation",
            Operation::Remove => "Remove",
        }
    }

    pub fn animator(&self) -> &WeaponAnimator {
        &self.animator
    }

    /// One frame of `dt` seconds.
    pub fn step(&mut self, dt: f32) -> Vec<GrenadeEvent> {
        let mut events = Vec::new();
        self.animator.set_drawn_smooth(self.drawn_smooth);
        self.animator.update(dt);
        for mark in self.animator.deliver() {
            match (mark.name.as_str(), self.operation) {
                ("WeapIn", Operation::Spawn) => {
                    self.operation = Operation::Idle;
                    self.animator.set("FireMode", THROWING);
                    events.push(GrenadeEvent::Appeared);
                    if self.hide_asked {
                        self.remove();
                    }
                }
                ("FiringBullet", Operation::Throw { low, stage: Throw::Throwing }) => {
                    self.animator.set(Self::parameter(low), IDLE);
                    events.push(GrenadeEvent::Thrown { low });
                }
                // The use is over: the game's "used" call gives the hands
                // back and the grenade's controller is let go, with no
                // put-away of its own.
                ("FireEnd", Operation::Throw { stage: Throw::Throwing, .. }) => {
                    self.operation = Operation::Away;
                    events.push(GrenadeEvent::Used);
                }
                ("WeapOut", Operation::Remove) => {
                    self.operation = Operation::Away;
                    events.push(GrenadeEvent::Hidden);
                }
                ("Sound", _) => events.push(GrenadeEvent::Sound(mark.text)),
                ("SoundAtPoint", _) => events.push(GrenadeEvent::SoundAtPoint(mark.text)),
                ("ThirdAction", _) => events.push(GrenadeEvent::BodyAction(mark.int)),
                _ => {}
            }
        }
        events
    }
}

/// What a throw goes by (`BaseGrenadeHandsController.Throw`, for the player).
#[derive(Debug, Clone, Copy)]
pub struct ThrowInput {
    /// `EFTHardSettings.GrenadeForce`.
    pub force: f32,
    pub low: bool,
    /// Strength's throwing buff (`StrengthBuffThrowDistanceInc`).
    pub strength_buff: f32,
    /// Throwing's elite buff (`ThrowingEliteBuff`).
    pub elite: bool,
    /// The hands' stamina, 0 to 1 (`HandsStamina.NormalValue`).
    pub hands_stamina: f32,
    /// The drop bone's up, in the world (the weapon root's).
    pub drop_up: [f32; 3],
    /// A random direction of length 1 (`Random.onUnitSphere`), used below
    /// half the hands' stamina.
    pub random: [f32; 3],
    /// The player's velocity.
    pub velocity: [f32; 3],
}

/// The force a grenade is thrown with (`Throw`, `ThrowGrenadeInner`): the
/// grenade force, times 0.66 from below or one and Strength's buff from
/// above, and without Throwing's elite buff by `lerp(0.4, 1, stamina +
/// 0.5)`; along the drop bone's down five times over plus, below half the
/// hands' stamina, a random direction by how far under half; and the
/// player's velocity added. Given to the grenade as an impulse.
pub fn throw_force(input: &ThrowInput) -> [f32; 3] {
    let mut strength = if input.low { 0.66 } else { 1.0 + input.strength_buff };
    let direction = if input.elite {
        input.drop_up.map(|x| -x)
    } else {
        let wobble = (0.5 - input.hands_stamina).clamp(0.0, 1.0);
        let raw: [f32; 3] = std::array::from_fn(|k| -input.drop_up[k] * 5.0 + input.random[k] * wobble);
        strength *= 0.4 + 0.6 * (input.hands_stamina + 0.5).clamp(0.0, 1.0);
        let length = raw.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-6);
        raw.map(|x| x / length)
    };
    std::array::from_fn(|k| direction[k] * input.force * strength + input.velocity[k])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The recorded throw from above, standing with the hands' stamina
    /// full and Strength's buff 0.16 (`courses2`, `2-grenade-high`): a
    /// force 13.398 long, 11.55 × 1.16.
    #[test]
    fn a_throw_from_above_is_the_force_and_strength() {
        let input = ThrowInput {
            force: 11.55,
            low: false,
            strength_buff: 0.16,
            elite: false,
            hands_stamina: 1.0,
            drop_up: [0.0, -0.3, -0.954],
            random: [1.0, 0.0, 0.0],
            velocity: [0.0; 3],
        };
        let force = throw_force(&input);
        let length = force.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((length - 13.398).abs() < 0.001, "{length}");
        // From below a third less strong; and the player's velocity added.
        let low = throw_force(&ThrowInput { low: true, velocity: [1.0, 0.0, 0.0], ..input });
        let without = throw_force(&ThrowInput { low: true, ..input });
        assert!((low[0] - without[0] - 1.0).abs() < 1e-5);
        let length = without.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((length - 11.55 * 0.66).abs() < 0.001, "{length}");
        // Tired hands throw weaker and astray.
        let tired = throw_force(&ThrowInput { hands_stamina: 0.1, ..input });
        let length = tired.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((length - 13.398 * (0.4 + 0.6 * 0.6)).abs() < 0.001, "{length}");
    }
}
