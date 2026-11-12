//! Reloading a magazine and the bolt catch.

use super::*;

impl Firearm {
    /// How far a reload with the sights up has stiffened the weapon, 0 to 1.
    pub fn tactical_reload(&self) -> f32 {
        self.tactical.value(self.clock)
    }

    /// Swaps the magazine for a full one.
    pub fn reload(&mut self) {
        self.reload_with(self.settings.magazine_capacity);
    }

    /// Swaps the magazine for one holding `rounds`. What the player carries
    /// is for the host game to decide. Asked for during a shot, the reload
    /// follows it, if nothing else is to.
    pub fn reload_with(&mut self, rounds: u32) {
        self.reload_magazine(rounds, false);
    }

    /// The quick reload's key (`FirearmHandsInputTranslator.QuickReload`;
    /// firing spec, "The quick reload"): heard only with the animator at
    /// rest. With a magazine fitted, on a weapon that allows it, the old
    /// magazine is let fall and the new one is fitted the quicker way;
    /// otherwise this is the plain reload. `rounds` is what the new
    /// magazine holds. `Event::MagazineSwap` says which it was.
    pub fn quick_reload_with(&mut self, rounds: u32) {
        if !self.animator_idling() {
            return;
        }
        let quick = self.settings.fast_reload && self.magazine.is_some();
        self.reload_magazine(rounds, quick);
    }

    /// The reload's key heard (`FirearmHandsInputTranslator.Reload`, and
    /// `QuickReload` at rest): **a mount is left first, whatever follows**,
    /// a magazine or a round found or not, the weapon full or not; a weapon
    /// used once hears nothing (`IsOneOff`). Each reload here does it; a
    /// host that finds nothing to reload with and so asks for none tells
    /// the weapon of the key with this (without it the M700, filled a round
    /// at a time, would stay on its bipod).
    pub fn reload_key(&mut self) {
        if !self.settings.one_off {
            self.events.push(Event::LeaveMount { at_once: false });
        }
    }

    /// The reload's key with no magazine found to put in (`ReloadExternalMagazine`):
    /// the mount left, and a failure to feed with a magazine in reminds, as
    /// it does before the search. The round put into the chamber by hand
    /// that follows is `chamber_by_hand`'s.
    pub fn reload_none_found(&mut self) {
        self.reload_key();
        if self.magazine.is_some() && self.malfunction() == Malf::Feed {
            self.remind();
        }
    }

    /// The reload key's last resort, where its search found no magazine
    /// (`FirearmHandsInputTranslator.Reload`: `LoadAmmoToChamber`; firing
    /// spec, "A round into the chamber by the reload key"), in the game's
    /// order: nothing for a weapon with no chamber of its own (a cylinder's,
    /// barrels', none), nor with a round in it, nor with no round within
    /// reach that it takes (`round_in_reach`, the host's search); a
    /// malfunction turns the animator's `MisfireSlideUnknown` off and is
    /// reminded of (`ExamineMalfunction`), and nothing more; then, with no
    /// magazine in, the round is put in, with an empty one in, that one goes
    /// out first, and with one holding rounds, nothing. What is left to do is
    /// the inventory's, its moves the hands' as any other (`load_chamber`,
    /// `pull_out_magazine`), so refused where they would refuse them.
    pub fn chamber_by_hand(&mut self, round_in_reach: bool) -> ChamberByHand {
        if self.has_chambers() || self.settings.no_chamber || self.chamber || !round_in_reach {
            return ChamberByHand::Nothing;
        }
        if self.malfunction() != Malf::None {
            self.animator.set_flag("MisfireSlideUnknown", false);
            self.remind();
            return ChamberByHand::Nothing;
        }
        match self.magazine {
            None => ChamberByHand::Load,
            Some(0) => ChamberByHand::MagazineOutFirst,
            Some(_) => ChamberByHand::Nothing,
        }
    }

    fn reload_magazine(&mut self, rounds: u32, quick: bool) {
        self.reload_key();
        // No reload starts in blind fire (`CanStartReload`), nor with a
        // failure to feed, which reminds instead.
        if self.blind_fire {
            return;
        }
        if self.malfunction() == Malf::Feed {
            return self.remind();
        }
        match &mut self.operation {
            // A reload asked for while the last one's animation is still
            // playing out is taken like any other: the operation begins
            // at the key and the animator takes it up when it is back at
            // rest (the fifteenth recording, `69-reload-in-reload-tail`).
            Operation::Idle => {
                // A character who does not know the weapon well lowers
                // the sights to reload (and anyone a belt-fed gun's).
                if self.reload_lowers_sights() {
                    self.aiming = false;
                }
                self.idle_trigger(false);
                self.begin_reload(rounds, quick);
            }
            Operation::Single { planned, .. } | Operation::Automatic { planned, .. } if planned.is_none() => {
                *planned = Some(Planned::Reload(rounds, quick));
                self.trigger_to(false);
                self.work_bolt_for_reload();
            }
            Operation::Single { .. } => self.work_bolt_for_reload(),
            _ => {}
        }
    }

    /// A reload asked for in a bolt action's shot works the bolt, whether
    /// or not the reload is taken (`BoltActionFireOperation.ReloadMag`,
    /// `ReloadWithAmmo`).
    pub(super) fn work_bolt_for_reload(&mut self) {
        if self.settings.bolt_action {
            self.animator.set_flag("BoltActionReload", true);
        }
    }

    /// The reload operation's start: the animator is told what it is to
    /// do, and does it from its resting state. The host is told of the
    /// exchange, which the game makes now. A quick one raises `ReloadFast`
    /// where the plain one raises `Reload` (`FirearmsAnimator.Reload`).
    pub(super) fn begin_reload(&mut self, rounds: u32, quick: bool) {
        self.events.push(Event::MagazineSwap { left: self.magazine, quick });
        self.incoming = rounds;
        // A round is chambered only from a new magazine whose top round the
        // chamber takes, which the animator is told
        // (`ReloadExternalMagOperation.Start`).
        let compatible = rounds == 0 || rounds > self.incoming_unfit;
        self.chambers = !self.chamber && !self.settings.no_chamber && rounds > 0 && compatible;
        self.animator_ammo_compatible(compatible);
        let fitted = self.magazine.is_some();
        self.animator.set_flag("Inventory", false);
        self.animator.set_flag("IsExternalMag", true);
        self.animator.set_flag("CanReload", true);
        self.animator.set("RelTypeOld", if fitted { self.mag_variant as f32 } else { -1.0 });
        self.animator.set("RelTypeNew", self.incoming_variant as f32);
        self.animator.trigger(if quick { "ReloadFast" } else { "Reload" });
        // With a round chambered and none to chamber from the new
        // magazine, a bolt held open is let go as the reload begins
        // (`ReloadExternalMagOperation.Start`).
        let s = &self.settings;
        if s.bolt_catch && self.chamber && !self.chambers && !s.manual_bolt_catch && !s.must_open_bolt {
            self.set_bolt_catch(false);
        }
        (self.hands_busy, self.reload_flag) = (true, true);
        self.operation = Operation::Reload { mag_out: false, mag_shown: false, mag_in: false, catch_off: false, mag_hidden: false, hide: None };
    }

    /// The bolt is caught open or let go (`FirearmsAnimator.SetBoltCatch`):
    /// the animator's flag, the resting pose that goes with it, and the
    /// layer that holds the bolt back.
    pub(super) fn set_bolt_catch(&mut self, caught: bool) {
        self.animator.set_flag("BoltCatch", caught);
        self.animator.set("IdlePosition", caught as u8 as f32);
        self.animator.set_layer_weight("Catch", caught as u8 as f32);
    }

    /// The operation in hand, by the name of its kind, as a tool reading a
    /// recording beside the logic says it (`byhand`).
    pub fn operation_said(&self) -> String {
        let said = format!("{:?}", self.operation);
        said.split([' ', '(']).next().unwrap_or("").to_owned()
    }

    /// Whether the bolt is held open, as the animator has it.
    pub fn bolt_caught(&self) -> bool {
        self.animator.flag("BoltCatch")
    }
}
