//! Blind fire over and around cover.

use super::*;

impl Movement {
    // ------------------------------------------------------------ blind fire

    /// Blind fire's figure as the movement holds it (`MovementContext.
    /// BlindFire`; `docs/spec/blind-fire.md`): 1 over the top, -1 to the
    /// right, 0 none. While it is set the movement keys are not heard and
    /// nothing is vaulted.
    pub fn blind_fire(&self) -> i32 {
        self.blind_fire
    }

    /// What the body's animator is told of blind fire's side (`BlindFire`),
    /// which the lean alone may put to 0, and the float blind fire's state
    /// last worked out (`BlindFireFloat`).
    pub fn blind_fire_animator(&self) -> (f32, f32) {
        (self.blind_shown, self.blind_float)
    }

    /// Blind fire's figure as the gate last took it, once (`None` if it has
    /// taken nothing since this was last asked). The host hands it on to
    /// the thing in hand (`Firearm::set_blind_fire`) and to the weapon's
    /// motion (`Hands::start_blind_fire`), as the game's gate does
    /// (`blind_fire::tell`).
    pub fn take_blind_fire(&mut self) -> Option<i32> {
        self.blind_told.take()
    }

    /// Blind fire ended from outside its keys (`MovementContext.
    /// SetBlindFire(0)`): an operation of the weapon beginning that ends it
    /// (`FirearmEvent::BlindFireEnded`), a change of what is in the hands.
    /// The inventory's opening, a gesture of the hands, lying down and the
    /// approach to a door end it here of themselves.
    pub fn end_blind_fire(&mut self) {
        self.set_blind_fire(0, true, true);
    }

    /// A blind-fire command (`side`: 1 over the top, -1 to the right, 0 the
    /// end) as the movement state in hand passes it to the gate
    /// (`MovementState.BlindFire` and what overrides it): standing and the
    /// states built on it pass the side, the approach to a door passes
    /// nothing, and every other state passes 0, which ends blind fire.
    pub(super) fn ask_blind_fire(&mut self, side: i32, input: &Input) {
        let value = match self.state {
            state if state.takes_blind_fire() => side,
            State::Approach => return,
            _ => 0,
        };
        self.set_blind_fire(value, input.blind_firearm, input.blind_may_start);
    }

    /// The gate (`MovementContext.SetBlindFire(int)`): anything but a
    /// firearm in hand puts it to 0; a firearm takes the value if it may
    /// start blind fire or the value is 0, and a refusal changes nothing.
    /// Taken, the body's animator is told at once, and a start puts the
    /// sights down, as the weapon's operation does with them
    /// (`BlindFire_Internal`).
    fn set_blind_fire(&mut self, value: i32, firearm: bool, may_start: bool) {
        let value = if firearm { value } else { 0 };
        if firearm && !may_start && value != 0 {
            return;
        }
        self.blind_fire = value;
        self.blind_shown = value as f32;
        self.blind_told = Some(value);
        if value != 0 {
            self.aiming = false;
        }
    }

    /// Blind fire's state's own work each frame (`BlindeFireState.
    /// ManualAnimatorMoveUpdate`): standing's, then the float, the curve of
    /// the blind-fire state last entered read at the time of the one being
    /// entered if the animator is between two of them, else of the one
    /// playing, times the side (recorded: the state being entered wins;
    /// blind-fire spec, "The float").
    pub(super) fn update_blind_fire(&mut self, dt: f32, world: &dyn World) {
        self.update_idle(dt, world);
        let time = self.body.time_in(movement_state::BLIND_FIRE).unwrap_or(0.0);
        let curve = self.body.blind_fire_curve(self.blind_part, self.blind_side).map_or(0.0, |curve| curve.evaluate(time));
        self.blind_float = curve * self.blind_side;
    }

    /// Puts on the body's animator what the interactions' model asks.
    pub(super) fn show(&mut self, shows: Vec<Show>) {
        for show in shows {
            match show {
                // A gesture leaves blind fire as it is: the gestures'
                // subsystem never tells the player's hands of it
                // (`SendHandsInteractionStateChanged` comes from the hands'
                // own `Interact` and the backpack's drop, which ends it by
                // the weapon). Recorded: `courses8\headlamp`, the head
                // light's gesture at 27.62 s and 30.69 s in blind fire, held
                // to either side (blind-fire spec, "What ends it").
                Show::Play(index) => self.shown_interaction = Some(index),
                Show::Stop => self.shown_interaction = None,
                Show::Reset => {
                    self.shown_interaction = None;
                    self.body.reset_interactions();
                }
            }
        }
    }
}
