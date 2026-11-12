//! Random idles, weapon operations, patrols and adding ammunition.

use super::*;

impl Figure {
    /// The idle's event (`OnRandomizeAnimation`): with a weapon animator,
    /// a pick (the host's, else drawn: 0 nothing, 1 a random transition, 2
    /// a weapon move; the gesture's 3 never comes of `Range(0, 3)`); with
    /// none, nothing, and no draw. A weapon move whose trigger the weapon's
    /// animator lacks plays nothing; one it has is begun (`TurnOffLights`):
    /// the body's `UI_WeaponOperation` on (it leaves its idle for
    /// `Hands_Operation_In` where the kind is 0 or 1), the transition
    /// nought, the weapon off patrol, the first `IdleStart` awaited.
    pub(super) fn randomize(&mut self) {
        let pick = if self.held.is_some() {
            self.steered.pop_front().unwrap_or_else(|| match self.dice.range_int(0, 3) {
                1 => Pick::Random(self.dice.range_int(1, self.random_count + 1)),
                2 => Pick::WeaponMove(self.dice.range_int(0, self.moves.len() as i32) as usize),
                _ => Pick::Idle,
            })
        } else {
            Pick::Idle
        };
        match pick {
            Pick::Random(value) => self.body.set_float(RANDOM_TRANSITION, value as f32),
            Pick::Idle => self.body.set_float(RANDOM_TRANSITION, 0.0),
            Pick::WeaponMove(index) => {
                self.move_picked = Some(index);
                let has = self.held.as_ref().is_some_and(|held| self.moves.get(index).is_some_and(|name| held.animator.has(name)));
                self.body.set_float(RANDOM_TRANSITION, 0.0);
                if has {
                    self.body.set_bool(WEAPON_OPERATION, true);
                    self.patrol(false);
                    self.patrol_delayed = true;
                } else {
                    self.move_picked = None;
                }
            }
        }
        self.picks.push((self.age, pick));
    }

    /// The move's start (`OnWeaponOperationStart`): the picked move's
    /// trigger raised on the weapon's animator (the last one picked, which
    /// stays); with none ever picked, the move ends at once.
    pub(super) fn operation_start(&mut self) {
        if let (Some(index), Some(held)) = (self.move_picked, self.held.as_mut()) {
            held.animator.trigger(self.moves[index]);
            return;
        }
        self.patrol_delayed = false;
        self.body.set_bool(WEAPON_OPERATION, false);
        self.hear(false, OPERATION_END, "");
        self.operation_end();
    }

    /// The move's end (`OnWeaponOperationEnd`): the weapon back on patrol and
    /// told again whether it can reload.
    pub(super) fn operation_end(&mut self) {
        if self.held.is_none() {
            return;
        }
        self.patrol(true);
        let can_reload = self.can_reload;
        if let Some(held) = self.held.as_mut() {
            held.animator.set_flag("CanReload", can_reload);
        }
    }

    /// The weapon on patrol or off (`Patrol`'s setter): its animator's flag,
    /// the left grip's blend toward the common grip off patrol, the
    /// alternative grip's hold (the crutch) toward whole on patrol.
    pub(super) fn patrol(&mut self, on: bool) {
        let now = self.age;
        let Some(held) = self.held.as_mut() else { return };
        held.animator.set_flag("Patrol", on);
        if let Some(blender) = held.blender.as_mut() {
            blender.aim(if on { 0.0 } else { 1.0 }, now);
        }
        held.crutch.aim(if on { 1.0 } else { 0.0 }, now);
    }

    /// The weapon in its idle (`OnIdleStart`): the first time after a move is
    /// picked (its way off patrol) it only clears the wait; after, the body
    /// is told the move is done.
    pub(super) fn idle_start(&mut self) {
        if self.patrol_delayed {
            self.patrol_delayed = false;
        } else {
            self.body.set_bool(WEAPON_OPERATION, false);
        }
    }

    /// A round put in by a reload (`OnAddAmmoInMag`, `OnAddAmmoInChamber`):
    /// counted down from a draw of 2 to 10, at nought the weapon told it can
    /// reload no more and the count drawn again.
    pub(super) fn add_ammo(&mut self) {
        if self.ammo_to_load < 0 {
            self.ammo_to_load = self.dice.range_int(AMMO_DRAW.0, AMMO_DRAW.1);
        }
        self.ammo_to_load -= 1;
        if self.ammo_to_load == 0 {
            if let Some(held) = self.held.as_mut() {
                held.animator.set_flag("CanReload", false);
            }
            self.ammo_to_load = self.dice.range_int(AMMO_DRAW.0, AMMO_DRAW.1);
        }
    }
}
