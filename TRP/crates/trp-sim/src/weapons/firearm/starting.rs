//! Blind fire and starting the firearm drawn, folded or taken over.

use super::*;

impl Firearm {
    /// Whether the weapon may start blind fire now (`FirearmController.
    /// CanSetBlindFire`): its logic idling, not a rocket launcher, the
    /// inventory shut. So not in a shot, automatic fire, a reload, a look,
    /// a draw or a put-away.
    pub fn can_set_blind_fire(&self) -> bool {
        !self.settings.rocket_launcher && self.is_idling() && !self.inventory
    }

    /// Blind fire's figure as the movement's gate took it
    /// (`Movement::take_blind_fire`), told to the operation in hand
    /// (`FirearmOperation.BlindFire`): the idling and every fire operation
    /// pass it on, every other operation passes 0. Passed on, the flag is
    /// on for a side, and a start puts the sights down without the aim
    /// key's call (no kit's rattle: `BlindFire_Internal` sets `IsAiming`).
    /// Returns what was passed on, for the weapon's motion
    /// (`Hands::start_blind_fire`).
    pub fn set_blind_fire(&mut self, side: i32) -> i32 {
        let passes = matches!(
            self.operation,
            Operation::Idle | Operation::Single { .. } | Operation::Automatic { .. } | Operation::CylinderFire { .. } | Operation::BarrelsFire { .. }
        );
        let passed = if passes { side } else { 0 };
        self.blind_fire = passed != 0;
        if self.blind_fire {
            self.aiming = false;
        }
        passed
    }

    /// Whether the weapon's blind-fire flag is on.
    pub fn blind_fire(&self) -> bool {
        self.blind_fire
    }

    /// An operation that ends blind fire has begun (`SetBlindFire(0)` at
    /// its start): the host ends it in the movement and the weapon's motion
    /// (`Event::BlindFireEnded`). Only told with the flag on.
    pub(super) fn ends_blind_fire(&mut self) {
        if self.blind_fire {
            self.blind_fire = false;
            self.events.push(Event::BlindFireEnded);
        }
    }

    /// Asks for every animation of the weapon to be drawn on curves
    /// without corners through its clips' samples, or as the engine has
    /// them (`WeaponAnimator::set_drawn_smooth`): the host's to choose,
    /// by the options' switch `SMOOTH FIRST PERSON ANIMATIONS`.
    /// **How it is drawn, and no more**: the animator's pose,
    /// its states, its times and its marks are the same either way, and
    /// so is everything here that goes by them. As the engine has them
    /// unless asked.
    pub fn set_drawn_smooth(&mut self, on: bool) {
        self.drawn_smooth = on;
    }

    /// Leaves the bones of the rig its animator need not work out to be
    /// made from each update's mixes when asked (`WeaponAnimator::
    /// set_deferred_posing`, `pose_of`), from the next step on, or works
    /// every bone out (off, as made). Nothing here goes by those bones.
    pub fn set_deferred_posing(&mut self, on: bool) {
        self.deferred_posing = on;
    }

    /// Has the weapon in the hands and at rest, as one that was drawn a
    /// while ago: what its draw and the idling operation's start have told
    /// the animator by then.
    pub fn start_drawn(&mut self) {
        self.fresh();
        self.animator.set_flag("Active", true);
        self.animator.set_layer_weight("LActions", 0.0);
        self.animator_fold();
        // (As its draw would have told it.)
        let compatible = self.ammo_compatible();
        self.animator_ammo_compatible(compatible);
        self.animator.play(&self.settings.rest_state, 0.0);
        self.animator.update(0.0);
        self.animator.deliver();
        self.to_idle();
    }

    /// Has the weapon in one of its fire modes with no switch played: as
    /// it was left when it was last in the hands. Before `draw` or
    /// `start_drawn`.
    pub fn start_in_mode(&mut self, mode: FireMode) {
        if let Some(index) = self.settings.modes.iter().position(|has| *has == mode) {
            self.mode = index;
        }
    }

    /// The same weapon built anew with other parts (the app's builds:
    /// `inventory::rig_key`) takes over what the build it replaces had in
    /// it and how that was set, before `start_drawn`: the rounds in its
    /// chambers (a cylinder's, barrels'; where it has as many), the turn of
    /// its cylinder, its own chamber, its magazine's count, its fire mode
    /// (where it has it) and its hammer. The app's: the game changes a part
    /// on the weapon's own model and its logic goes on as it was.
    pub fn take_over(&mut self, from: &Firearm) {
        if self.in_chambers.len() == from.in_chambers.len() {
            self.in_chambers = from.in_chambers.clone();
            self.camora = from.camora;
            if self.shown.len() == from.shown.len() {
                self.shown = from.shown.clone();
            }
        }
        (self.chamber, self.magazine, self.hammer_closed, self.armed) = (from.chamber, from.magazine, from.hammer_closed, from.armed);
        (self.mag_variant, self.incoming_variant) = (from.mag_variant, from.incoming_variant);
        (self.mag_external, self.incoming_external) = (from.mag_external, from.incoming_external);
        (self.unfit, self.incoming_unfit) = (from.unfit, from.incoming_unfit);
        if let Some(mode) = from.settings.modes.get(from.mode).copied() {
            self.start_in_mode(mode);
        }
    }

    /// A build left behind while in hand (the app's: the weapon in hand
    /// built anew, its new build's rig taken in this one's place) is put
    /// away at once, unseen, as its own put-away would leave it: so that
    /// taken in hand again (the weapon built back to this build) it is
    /// drawn as any weapon is. Left as it was, at rest, its next draw would
    /// never begin (`draw` waits for `Away`), and the hands would wait on
    /// it for ever.
    pub fn put_away_unseen(&mut self) {
        self.trigger_to(false);
        self.aiming = false;
        self.hands_busy = false;
        self.quick_speed = None;
        self.operation = Operation::Away;
        self.events.clear();
    }

    /// The same weapon built anew with another part (a magazine of another
    /// kind put in by a reload) takes over everything the build it replaces
    /// was doing, its operation and its animator's states and times with
    /// it, where its animator runs machines of the same shape: the reload
    /// goes on through the change, on the new build's tree (the app's: the
    /// game changes the part on the weapon's own model). Its own settings
    /// stay. Whether it could.
    pub fn carry_on_from(&mut self, from: &Firearm) -> bool {
        let mut animator = self.animator.clone();
        if !animator.take_state(&from.animator) {
            return false;
        }
        let settings = self.settings.clone();
        *self = from.clone();
        (self.settings, self.animator) = (settings, animator);
        true
    }

    /// Whether no magazine is in sight in the hands now: a reload's old one
    /// has left the hand (`MagHide`) and the new one is not yet shown
    /// (`MagShow`), or one put in from the inventory is not yet shown: when
    /// the weapon's model may change its magazine unseen.
    pub fn magazine_out_of_sight(&self) -> bool {
        matches!(self.operation, Operation::Reload { mag_hidden: true, mag_shown: false, .. } | Operation::InventoryMagIn { shown: false, .. })
    }

    /// Whether a magazine going in is in sight now: a reload's or one put
    /// in from the inventory, from its `MagShow` (where the game makes the
    /// new magazine's model on the weapon) to the operation's end.
    pub fn magazine_shown_going_in(&self) -> bool {
        matches!(self.operation, Operation::Reload { mag_shown: true, .. } | Operation::InventoryMagIn { shown: true, .. })
    }

    /// Has the hammer cocked or down, and the bolt caught open or home,
    /// as something before left them: for a replay that begins where a
    /// recording stood. After `start_drawn`.
    pub fn start_with(&mut self, armed: Option<bool>, bolt_caught: Option<bool>) {
        if let Some(armed) = armed {
            self.armed = armed;
            self.animator.set_flag("Armed", armed);
            self.animator.set_layer_weight("Hammer", if armed { 0.0 } else { 1.0 });
        }
        if let Some(caught) = bolt_caught {
            self.set_bolt_catch(caught);
        }
    }

    /// Has the stock folded, as the weapon was left: before `draw` or
    /// `start_drawn`, which tell the animator.
    pub fn start_folded(&mut self, folded: bool) {
        self.folded = folded && self.settings.foldable;
    }

    /// Whether the stock is folded.
    pub fn stock_folded(&self) -> bool {
        self.folded
    }

    /// The stock as the animator is told it (`FirearmsAnimator.Fold`): the
    /// layer that holds it folded, and the flag its two states go by.
    pub(super) fn animator_fold(&mut self) {
        self.animator.set_layer_weight("Stock", self.folded as u8 as f32);
        self.animator.set_flag("StockFolded", self.folded);
    }
}
