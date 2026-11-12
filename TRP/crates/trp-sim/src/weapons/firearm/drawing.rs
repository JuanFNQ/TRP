//! Skill speeds, mastering, and drawing and hiding the weapon.

use super::*;

impl Firearm {
    /// The class skill's two speeds (`weapon_skill_speeds`, or a level's
    /// buffs: `skills::WeaponBuffs`), which the animator is told as the
    /// weapon is taken in hand (`SyncWithCharacterSkills`): a change shows
    /// from the next draw, as in the game.
    pub fn set_skill_speeds(&mut self, reload_speed: f32, draw_speed: f32) {
        (self.settings.reload_speed, self.settings.draw_speed) = (reload_speed, draw_speed);
        self.animator_skill_speeds();
        // A weapon not in hand has no controller to tell the body.
        if self.operation == Operation::Away {
            self.events.retain(|event| !matches!(event, Event::BodyDrawSpeed(_)));
        }
    }

    /// The arms damaged, left and right (`EPhysicalCondition.LeftArmDamaged`,
    /// `RightArmDamaged`), which the controller listens to while the weapon
    /// is in hand (`FirearmController.PhysicalConditionChanged`): with either
    /// damaged the animator's two speeds are 1, not the skill's
    /// (`SetAnimatorAndProceduralValues`), told on a change at once and at
    /// the draw and every idling's start after. A weapon not in hand has no
    /// controller to hear it: it is told quietly, for its next draw.
    pub fn set_arms_damaged(&mut self, left: bool, right: bool) {
        if self.arms_damaged == [left, right] {
            return;
        }
        self.arms_damaged = [left, right];
        if self.operation != Operation::Away {
            self.animator_skill_speeds();
        }
    }

    /// The skill's two speeds told to the animator
    /// (`SetAnimatorAndProceduralValues`): 1 and 1 with an arm damaged.
    /// The body's animator is told the same two with it
    /// (`Event::BodyDrawSpeed`; none of its states goes by the reload's).
    pub(super) fn animator_skill_speeds(&mut self) {
        let (reload, draw) = if self.arms_damaged.contains(&true) { (1.0, 1.0) } else { (self.settings.reload_speed, self.settings.draw_speed) };
        self.animator.set("SpeedReload", reload);
        self.animator.set("SpeedDraw", draw);
        self.events.push(Event::BodyDrawSpeed(draw));
    }

    /// The character's mastering of the weapon in hand changed (its
    /// family's level, a host's or the options' change): taken at once,
    /// as the game does for the weapon in the hands
    /// (`OnCurrentWeaponBeingMastered`, `SyncWithCharacterSkills`: the
    /// animator's `WeaponLevel`). Held to 0 to 2.
    pub fn set_mastering(&mut self, level: u32) {
        self.settings.mastering = level.min(crate::condition::mastering::TOP_LEVEL);
        self.animator.set("WeaponLevel", self.settings.mastering as f32);
    }

    /// The character's mastering of this weapon, 0 to 2.
    pub fn mastering(&self) -> u32 {
        self.settings.mastering
    }

    /// What the weapon's mastering adds to the walk's cap with the sights
    /// up (`AimMovementSpeed`), which the movement takes as they come up
    /// (`Movement::aim_walk_bonus`).
    pub fn aim_walk_bonus(&self) -> f32 {
        crate::condition::mastering::aim_walk_bonus(self.settings.mastering)
    }

    /// Whether starting a reload puts the sights down
    /// (`DisableAimingOnReload`): below mastering 2, or a belt-fed machine
    /// gun's.
    pub(super) fn reload_lowers_sights(&self) -> bool {
        self.settings.mastering < MASTERING_TO_AIM_IN_A_RELOAD || self.settings.belt_fed
    }

    /// The animator's two speeds for what is under way alone
    /// (`FirearmsAnimator.SetSpeedParameters`), as the quick draw sets them;
    /// the weapon's next idling puts its skill's back.
    pub fn set_animator_speeds(&mut self, reload_speed: f32, draw_speed: f32) {
        self.animator.set("SpeedReload", reload_speed);
        self.animator.set("SpeedDraw", draw_speed);
    }

    /// The holster's weapon drawn quick, said just after `draw`
    /// (`SetAnimatorAndProceduralValues`' quick branch): the weapon's
    /// animator alone takes the quick draw's speed, the reload's 1. The
    /// body's animator is not told there, so it keeps what the weapon
    /// before told it until this one idles.
    pub fn draw_quick(&mut self, draw_speed: f32) {
        self.set_animator_speeds(1.0, draw_speed);
        self.events.retain(|event| !matches!(event, Event::BodyDrawSpeed(_)));
    }

    /// The logic and the animator as the game makes them for a weapon
    /// taken in hand: nothing left over from when it was last there but
    /// what the weapon itself holds (its rounds, its fire mode).
    pub(super) fn fresh(&mut self) {
        self.quick_speed = None;
        self.animator.reset();
        self.animator.set("WeaponLevel", self.settings.mastering as f32);
        // The skill's two speeds, which the game tells the animator as the
        // weapon is taken in hand (`SyncWithCharacterSkills`).
        self.animator_skill_speeds();
        // The build's part with its own animations, which the game tells
        // the animator as the hands are made (`SetMaxUniqueAnimationModId`:
        // the id, and a float at 1 for any).
        let unique = self.settings.unique_animation;
        self.animator.set("UniqueAnimationModId", unique as f32);
        self.animator.set("UniqueAnimationModIdFloat", if unique > 0 { 1.0 } else { 0.0 });
        // A build with a launcher tells the rifle's animator so as the hands are
        // made (`FirearmsAnimator.SetLauncher(false)`: `LauncherReady` off and
        // `LauncherID` -1, the AK's `IDLE` on `akm_idle_armed`; underbarrel-launcher
        // spec, "The rifle's rest").
        if self.launcher.is_some() {
            self.animator.set_flag("LauncherReady", false);
            self.animator.set("LauncherID", -1.0);
        }
        self.trigger_to(false);
        (self.aiming, self.interrupted) = (false, false);
        (self.inventory, self.reload_flag, self.hands_busy, self.shot_made, self.shot_hammer_down) = (false, false, false, false, false);
        self.marks.clear();
        // A weapon with a malfunction is made with it held on its animator
        // (`WeaponPrefab`'s spawn of a malfunctioned weapon).
        let malf = self.malfunction();
        if malf != Malf::None {
            self.malfunction_ints(-1, malf.number());
            self.animator.set_layer_weight("Malfunction", 1.0);
        }
    }

    /// Takes the weapon in hand (`SpawnOperation`): the animator leaves
    /// its first state for the draw, and at the draw's last mark the
    /// weapon has appeared (`Event::Appeared`) and idles. A weapon with
    /// rounds in its magazine and none chambered chambers one as it is
    /// drawn. The inventory opening meanwhile is not heard; but **when
    /// the weapon has appeared the game's owner tells the player how the
    /// screen stands afresh** (`GamePlayerOwner.SetHandsController`), so a
    /// host calls `set_inventory` on `Event::Appeared`, or says it before
    /// the step (`set_appearing`), and a weapon drawn under an open screen
    /// then goes down for it.
    pub fn draw(&mut self) {
        if self.operation != Operation::Away {
            return;
        }
        self.fresh();
        if self.has_chambers() {
            self.cases_drawn_away();
            self.count_chambers();
        }
        self.animator_values();
        self.animator.set_flag("Active", true);
        self.animator.set_layer_weight("LActions", 0.0);
        self.animator_fold();
        self.animator_rounds();
        if self.settings.no_chamber {
            self.animator.set_flag("Armed", self.armed);
        }
        self.hands_busy = true;
        // The round goes over now; the animator hears of it at the mark
        // of the bolt going home. (Chambers of their own have no bolt to
        // rack.) Only from a magazine whose top round the chamber takes,
        // which the animator is told (`SpawnOperation.Start`).
        let compatible = self.ammo_compatible();
        self.animator_ammo_compatible(compatible);
        let arms = !self.has_chambers() && !self.settings.no_chamber && !self.chamber && self.magazine.is_some_and(|rounds| rounds > 0) && compatible;
        if arms {
            self.take_round();
        }
        self.operation = Operation::Spawn { hide: None, arms, compatible };
    }

    /// Has a draw just begun (`draw`, before the animator has stepped) rack
    /// the weapon as it comes up, for the look of it alone (the app's: the
    /// workbench's weapon drawn after a change was built, a visual touch
    /// alone): the animator is told its
    /// chamber empty and a round in its magazine, so that it draws by the
    /// draw that racks, and the rounds as they are at the bolt's mark
    /// (`AddAmmoInChamber`); no round is moved. A draw that racks of
    /// itself, a weapon with chambers of its own and one with no magazine
    /// fitted are left as they are.
    pub fn rack_drawn(&mut self) {
        let has_chambers = self.has_chambers() || self.settings.no_chamber;
        let Operation::Spawn { arms, .. } = &mut self.operation else { return };
        let Some(rounds) = self.magazine.filter(|_| !*arms && !has_chambers) else { return };
        *arms = true;
        self.animator.set("AmmoInChamber", 0.0);
        self.animator.set("AmmoInMag", rounds.max(1) as f32);
    }

    /// Puts the weapon away (`HideWeapon`). At rest, and in a look at it,
    /// the sights go down, the trigger is let go and the animator is told
    /// at once; a shot in hand finishes first, and so do a reload up to
    /// where it hands over and a draw. `Event::Hidden` says when it is
    /// out of the hands.
    pub fn hide(&mut self, fast: bool) {
        self.hide_from(fast, true);
    }

    /// `hide`, for a put-away an operation planned meanwhile and makes at
    /// its end: `from_rest` where the game makes it through the resting
    /// state's (`idling.HideWeapon`: the reloads, the fold, the bipod, the
    /// draw), which keeps the aim (`Event::AimKept`), and not where it goes
    /// straight to its own (`Remove.Start` in a rack's, a malfunction's or
    /// a double barrel's reload's).
    pub(crate) fn hide_from(&mut self, fast: bool, from_rest: bool) {
        if !fast {
            self.quick_speed = None;
        }
        // The thing in hand changing leaves a mount at once (`HandsChangingEvent`).
        self.events.push(Event::LeaveMount { at_once: true });
        match &mut self.operation {
            // A malfunction's own operations finish first: the trigger let
            // go, the sights down, and the put-away made at their end
            // (`MalfunctionOperation.HideWeapon`, its `_plannedAction` run by
            // `SwitchToIdle`). Lost, the weapon would stay in hand when
            // dropped meanwhile.
            Operation::Malfunction { hide, .. } | Operation::ExamineMalfunction { hide } | Operation::RepairMalfunction { hide, .. } => {
                *hide = Some(fast);
                self.trigger_to(false);
                self.aiming = false;
            }
            Operation::Idle | Operation::Utility { .. } => {
                // The look over the weapon is a resting state of its own
                // (`UtilityOperation : Idling`), and keeps it too.
                if from_rest {
                    self.events.push(Event::AimKept(self.aiming));
                }
                if self.is_idling() {
                    self.aiming = false;
                    self.idle_trigger(false);
                }
                self.start_remove(fast);
            }
            Operation::Single { planned, .. } | Operation::Automatic { planned, .. } => {
                *planned = Some(Planned::Hide(fast));
                self.trigger_to(false);
                // A bolt action puts its sights down and works the bolt
                // before it goes (`BoltActionFireOperation.HideWeapon`).
                if self.settings.bolt_action {
                    self.aiming = false;
                    self.animator.set_flag("BoltActionReload", true);
                }
            }
            Operation::Reload { hide, .. } | Operation::Spawn { hide, .. } | Operation::Fold { hide, .. } | Operation::Bipod { hide } => *hide = Some(fast),
            // The rack plans it for its end (`RechamberOperation.HideWeapon`,
            // its `_plannedAction` run by `SwitchToIdle`).
            Operation::Rechamber { hide, .. } => *hide = Some(fast),
            Operation::ChambersReload { hide, aborted, .. } => (*hide, *aborted) = (Some(fast), true),
            // A shot from chambers of their own finishes first; a
            // put-away asked for meanwhile is lost.
            Operation::CylinderFire { .. } | Operation::BarrelsFire { .. } => {}
            // A tube being filled is cut short for it.
            Operation::Shells { hide, aborted, .. } => (*hide, *aborted) = (Some(fast), true),
            // A backpack's drop, and a magazine moved from the inventory,
            // take no put-away at all: one asked for meanwhile is lost
            // (the base operation's `ProhibitedCall`).
            // So does a cylinder's turn (`RollCylinderOperation` has no
            // `HideWeapon` of its own).
            Operation::Remove | Operation::Away | Operation::DropBackpack | Operation::InventoryMagOut { .. } | Operation::InventoryMagIn { .. } | Operation::InventoryDischarge | Operation::OwnChamberOut { .. } | Operation::RollCylinder => {}
            // A round being put into a cylinder's chamber is in as the hands
            // let go of it, and the weapon goes (`LoadCartridgeToCamora.
            // HideWeapon`: `SwitchToIdle`, which makes the inventory's move,
            // then the put-away).
            Operation::OwnChamberIn { chamber, .. } if self.settings.cylinder.is_some() => {
                let chamber = *chamber;
                self.own_chamber_loaded(chamber);
                self.own_cylinder_at_rest();
                self.start_remove(fast);
            }
            // A round being put into the chamber, or a barrel, is given up at
            // once, and the weapon goes (`LoadCartridgeToChamber.HideWeapon`).
            Operation::InventoryChamber | Operation::OwnChamberIn { .. } => {
                self.events.push(Event::ChamberGivenUp);
                self.aiming = false;
                self.trigger_to(false);
                self.animator.set_flag("LoadOne", false);
                self.start_remove(fast);
            }
            Operation::ToggleLauncher { .. } | Operation::LauncherIdle | Operation::LauncherFire { .. } | Operation::LauncherReload { .. } | Operation::LauncherLoad | Operation::LauncherDischarge | Operation::LauncherUtility { .. } => self.launcher_hide(fast),
        }
    }

    /// Puts the weapon away quick, for the holster's weapon taken quick
    /// (`Player.Proceed(Weapon)`'s `CheckForFastWeaponSwitch`): as `hide`
    /// with its animator told `FastHide` (`FAST IDLE TO OUT`), and as the
    /// put-away begins, at once or when what is under way lets it
    /// (`Remove.Start`), the animator takes `draw_speed` and
    /// `Event::FastSwitch` says the arms pay (`pistol-quickdraw`: the
    /// state, its frames and the charge's frame the game's).
    pub fn hide_quick(&mut self, draw_speed: f32) {
        self.quick_speed = Some(draw_speed);
        self.hide(true);
    }

    /// Takes the `Event::FastSwitch` a put-away begun just now gave, for
    /// a host that pays for it at once; whether there was one.
    pub fn take_fast_switch(&mut self) -> bool {
        let before = self.events.len();
        self.events.retain(|e| *e != Event::FastSwitch);
        self.events.len() != before
    }

    pub(super) fn start_remove(&mut self, fast: bool) {
        if let Some(speed) = self.quick_speed.take().filter(|_| fast) {
            self.set_animator_speeds(1.0, speed);
            self.events.push(Event::FastSwitch);
        }
        // The put-away ends blind fire (`Player.TryProceed`, and the
        // put-away's own start).
        self.ends_blind_fire();
        // The item is no longer on its launcher (`Remove.Start`).
        if let Some(launcher) = self.launcher.as_mut() {
            launcher.marked = false;
        }
        self.animator.set_flag("Active", false);
        self.animator.set_flag("FastHide", fast);
        self.hands_busy = true;
        self.operation = Operation::Remove;
    }

    /// The view's pitch, in degrees below level, which the game tells the
    /// animator every frame: a weapon whose resting state goes by it has
    /// the parameter, and the stock AKM's has not.
    pub fn set_aim_angle(&mut self, pitch: f32) {
        self.animator.set("Aim_angle", pitch);
    }
}
