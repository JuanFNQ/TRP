//! Creating the firearm and choosing its magazine's variants.

use super::*;

impl Firearm {
    /// A loaded weapon that is not in the hands yet: a full magazine and a
    /// round in the chamber. `draw` takes it in hand; `start_drawn` has it
    /// there already.
    pub fn new(mut settings: FirearmSettings, animator: &WeaponAnimatorData) -> Self {
        // The game has no mastering level above 2.
        settings.mastering = settings.mastering.min(crate::condition::mastering::TOP_LEVEL);
        let mut weapon = Self::made(settings, animator);
        if weapon.has_chambers() {
            weapon.fill_chambers(true);
        }
        weapon
    }

    /// The weapon as one found in a container: no
    /// round in its chamber, its magazine, tube or cylinder full. Barrels
    /// are chambers and nothing else: they are empty.
    pub fn found(&mut self) {
        // A rocket launcher cannot be loaded: it is found with its rocket
        // (the app's choice, as a full magazine is).
        if self.settings.rocket_launcher {
            self.fill_chambers(true);
        } else if self.settings.barrels > 0 {
            self.fill_chambers(false);
        } else if !self.has_chambers() {
            self.chamber = false;
            // `PROVISIONAL`: a weapon with no chamber found unarmed, as one
            // with a chamber is found with it empty (the item's own `Armed`
            // in the game's loot is not read).
            self.armed = false;
        }
    }

    /// The weapon as one the run begins with in hand, drawn before: a
    /// round chambered, or every chamber of its own loaded.
    pub fn carried(&mut self) {
        if self.has_chambers() {
            self.fill_chambers(true);
        } else {
            self.chamber = !self.settings.no_chamber;
            self.armed = true;
        }
    }

    /// The fitted magazine's `magAnimationIndex`, where the host's item has
    /// another magazine than the build the weapon was made from.
    pub fn set_magazine_variant(&mut self, variant: i32) {
        self.mag_variant = variant;
    }

    /// The `magAnimationIndex` of the magazine the next reload or magazine
    /// put in from the inventory brings (`FirearmsAnimator.Reload`'s new
    /// type, `InsertMagOperation`'s): the host's, as it picks the magazine.
    pub fn next_magazine_variant(&mut self, variant: i32) {
        self.incoming_variant = variant;
    }

    /// Whether the weapon's magazine is one that comes out, as the game's
    /// `Weapon.ReloadMode` reads with the magazine fitted: `ExternalMagazine`,
    /// or `ExternalMagazineWithInternalReloadSupport`, a weapon of a magazine
    /// of its own (`InternalMagazine`) fitted with one that comes out (the
    /// Model 700). Barrels never (`OnlyBarrel`).
    pub fn magazine_comes_out(&self) -> bool {
        self.settings.barrels == 0 && (!self.settings.internal_magazine || (self.magazine.is_some() && self.mag_external))
    }

    /// Whether the reload's key looks for a magazine to put in before
    /// anything else (`FirearmHandsInputTranslator.Reload`): a weapon whose
    /// magazine comes out, and one of a magazine of its own with none fitted
    /// (`InternalMagazine` and no magazine). The quick key looks for one
    /// only where the magazine comes out (`QuickReload`). Where none is
    /// found, a weapon of a magazine of its own fills it a round at a time
    /// (`ReloadWithAmmo`).
    pub fn reload_looks_for_magazine(&self) -> bool {
        self.magazine_comes_out() || (self.settings.internal_magazine && self.settings.barrels == 0 && self.magazine.is_none())
    }

    /// Whether the fitted magazine comes out (`ReloadMagType`
    /// `ExternalMagazine`), where the host's item has another magazine than
    /// the build the weapon was made from.
    pub fn set_magazine_external(&mut self, external: bool) {
        self.mag_external = external;
    }

    /// Whether the magazine the next reload or magazine put in from the
    /// inventory brings comes out: the host's, as it picks the magazine.
    pub fn next_magazine_external(&mut self, external: bool) {
        self.incoming_external = external;
    }

    /// The fitted magazine's rounds from its foot up to and with the first,
    /// from its top, that the weapon's chamber does not take (0 where it
    /// takes them all): the host's, as its inventory has them
    /// (`inventory::unfit_rounds`). Counted from the foot, the figure stays
    /// true as rounds are fed off the top, so a host whose item is told of
    /// them later may give it on every frame.
    pub fn set_magazine_unfit(&mut self, unfit: u32) {
        self.unfit = unfit;
    }

    /// The same of the magazine the next reload or magazine put in from the
    /// inventory brings: the host's, as it picks the magazine. It becomes
    /// the fitted one's as that magazine goes in, and is 0 again after.
    pub fn next_magazine_unfit(&mut self, unfit: u32) {
        self.incoming_unfit = unfit;
    }

    /// Whether the fitted magazine feeds the chamber (`Magazine.
    /// IsAmmoCompatible`): none fitted, none in it, or its top round one the
    /// chamber takes.
    pub fn ammo_compatible(&self) -> bool {
        self.magazine.is_none_or(|rounds| rounds == 0 || rounds > self.unfit)
    }

    /// The animator told whether the magazine feeds (`FirearmsAnimator.
    /// SetAmmoCompatible`: its `IncompatibleAmmo`, which only some weapons'
    /// animators have; firing spec, "Rounds the chamber does not take").
    pub(super) fn animator_ammo_compatible(&mut self, compatible: bool) {
        self.animator.set_flag("IncompatibleAmmo", !compatible);
    }

    /// Whether a reload's old magazine is still in the hand: the reload has
    /// begun and the old one has not yet been put away (`MagHide`).
    pub fn old_magazine_in_hand(&self) -> bool {
        matches!(self.operation, Operation::Reload { mag_hidden: false, .. })
    }

    /// Whether a round lies in the weapon's chamber of its own to be drawn
    /// there (`Firearms.SetRoundIntoWeapon`: the round's model at the
    /// chamber's point, always there while one is chambered, seen as the
    /// action opens): not a weapon with chambers of its own or none, and
    /// not from a shot until the next round is fed in sight
    /// (`MoveAmmoFromChamberToShellPort`, then `OnAddAmmoInChamber`).
    pub fn round_drawn_in_chamber(&self) -> bool {
        // One loaded by hand is drawn before the chamber has it: the clip
        // carries the chamber's point from the hand to the port.
        let by_hand = self.round_in_hand && matches!(self.operation, Operation::InventoryChamber);
        (self.chamber && !self.round_out || by_hand) && !self.has_chambers() && !self.settings.no_chamber
    }

    /// Whether a round is ready to fire: the chamber's, or on a weapon with
    /// no chamber, the magazine's top one (`Idling.SetTriggerPressed`).
    pub fn loaded(&self) -> bool {
        self.chamber || (self.settings.no_chamber && self.magazine.is_some_and(|rounds| rounds > 0))
    }

    fn made(settings: FirearmSettings, animator: &WeaponAnimatorData) -> Self {
        Self {
            in_chambers: Vec::new(),
            camora: 0,
            hammer_closed: false,
            shown: Vec::new(),
            fired_chambers: Vec::new(),
            reload_free: Vec::new(),
            reload_shells: Vec::new(),
            reload_loading: 0,
            reload_reach: 0,
            reload_cached: 0,
            reload_remove: -1,
            reload_unloads: false,
            moving_round: false,
            reload_quick: false,
            reload_fistful: false,
            aim_told: false,
            aim_flags: false,
            chamber: !settings.no_chamber,
            armed: true,
            mag_variant: settings.magazine_variant,
            incoming_variant: settings.magazine_variant,
            mag_external: settings.magazine_external,
            incoming_external: settings.magazine_external,
            unfit: 0,
            incoming_unfit: 0,
            round_out: false,
            round_in_hand: false,
            magazine: Some(settings.magazine_capacity),
            incoming: settings.magazine_capacity,
            chambers: false,
            folded: false,
            mode: 0,
            trigger: false,
            held: false,
            hands_free: true,
            aiming: false,
            overlap: 0.0,
            interrupted: false,
            clock: 0.0,
            tactical: Blend::default(),
            inventory: false,
            reload_flag: false,
            hands_busy: false,
            quick_speed: None,
            arms_damaged: [false; 2],
            appearing: None,
            shot_made: false,
            shot_hammer_down: false,
            operation: Operation::Away,
            events: Vec::new(),
            left_stance_seen: None,
            wear: None,
            auto_cycle: 60.0 / settings.fire_rate,
            burst_rate: 0.0,
            animator: WeaponAnimator::new(animator),
            marks: Vec::new(),
            drawn_smooth: false,
            deferred_posing: false,
            blind_fire: false,
            bipod_out: false,
            mount_tactical: false,
            launcher: None,
            settings,
        }
    }
}
