//! Scope rotation, optics, zoom values and aiming sensitivity.

use super::*;

impl Sights {
    /// How far the weapon is turned about its barrel while aiming by the
    /// sight in use, degrees (`UpdateCurrentScopeRotation`): the sight's own
    /// turn from straight ahead, where it is at least the threshold (a
    /// canted sight's: the PK-06 on a 45 degree mount), else none. The game
    /// works it out as the sights are found and as they change, which is
    /// when the sight in use can change.
    pub fn scope_rotation(&self) -> f32 {
        if self.list.is_empty() {
            return 0.0;
        }
        let rotation = self.current().rotation;
        if rotation.abs() >= SCOPE_ROTATION_THRESHOLD {
            rotation
        } else {
            0.0
        }
    }

    /// A sight item's state.
    pub fn state(&self, part: usize) -> &SightState {
        &self.states[part]
    }

    /// Whether a sight is aimed through as an optic (`SightNBone.IsOptic`):
    /// its prefab's mode now has an `OpticSight`, the bone is that optic's,
    /// and its item names no aim plane of its own.
    pub fn is_optic(&self, sight: &Sight) -> bool {
        let Some(cache) = sight.cache else { return false };
        let has = self.parts[cache].modes.get(self.states[cache].prefab_mode).is_some_and(|mode| mode.optic.is_some());
        has && sight.optic_bone && sight.part.is_none_or(|p| self.parts[p].custom_aim_plane.is_empty())
    }

    /// The current sight is an optic.
    pub fn optic(&self) -> bool {
        self.is_optic(self.current())
    }

    /// The main camera's field of view while aiming: 35 degrees through an
    /// optic, the setting less 15 over anything else.
    pub fn aimed_field_of_view(&self, setting: f32) -> f32 {
        if self.optic() {
            OPTIC_FIELD_OF_VIEW
        } else {
            setting - crate::weapons::hands::AIM_FOV_DROP
        }
    }

    /// The current sight's scope and mode, its item's (where it has one).
    pub fn scope_and_mode(&self) -> Option<(usize, usize)> {
        let p = self.current().part?;
        let state = &self.states[p];
        Some((state.scope, state.modes.get(state.scope).copied().unwrap_or(0)))
    }

    /// Its magnification for the HUD (`GetCurrentOpticZoom`).
    pub fn zoom(&self) -> Option<f32> {
        let (scope, mode) = self.scope_and_mode()?;
        self.parts[self.current().part?].scopes.get(scope)?.zooms.get(mode).copied()
    }

    /// The zoom its item keeps (`ScopeZoomValue`): a variable scope's field
    /// of view, 0 on anything else.
    pub fn zoom_value(&self) -> f32 {
        self.current().part.map_or(0.0, |p| self.states[p].zoom_value)
    }

    /// The figure its item has for its scope and mode (`GetCurrentSensitivity`;
    /// 1 where it has none).
    pub fn item_sensitivity(&self) -> Option<f32> {
        let p = self.current().part?;
        let (scope, mode) = self.scope_and_mode()?;
        Some(self.parts[p].scopes.get(scope).and_then(|s| s.sensitivity.get(mode)).copied().unwrap_or(1.0))
    }

    /// What the mouse's aiming sensitivity is multiplied by while aiming
    /// (`FirearmController.UpdateSensitivity`): the weapon's own figure with
    /// no sight part; a variable scope's by its zoom while aimed through as
    /// an optic; else the item's figure for its scope and mode.
    /// `PROVISIONAL`: a variable scope's is its zoom's now, where the game
    /// keeps the last its optic camera reported (`AimingSmoothSensitivity`,
    /// each frame the camera is on): a frame apart at most as the sights
    /// come up.
    pub fn aim_sensitivity(&self, weapon: f32) -> f32 {
        let sight = self.current();
        let Some(p) = sight.part else { return weapon };
        if self.parts[p].adjustable.is_some() && self.is_optic(sight) {
            let handler = sight.cache.and_then(|c| self.states[c].zoom.as_ref()).or(self.states[p].zoom.as_ref());
            if let Some(handler) = handler {
                return handler.sensitivity();
            }
        }
        self.item_sensitivity().unwrap_or(1.0)
    }

    /// The optic whose camera is on while aiming (`OpticComponentUpdater`:
    /// the current sight's current mode's `OpticSight`, where the sight is
    /// an optic): the part whose prefab's modes it goes by, and that mode.
    pub fn live_optic(&self) -> Option<(usize, usize)> {
        if let Some(part) = self.forced_optic.filter(|&part| part < self.parts.len()) {
            return Some((part, self.states[part].prefab_mode));
        }
        let sight = self.current();
        if !self.is_optic(sight) {
            return None;
        }
        let cache = sight.cache?;
        Some((cache, self.states[cache].prefab_mode))
    }

    /// A part's optic made live, or none (`forced_optic`).
    pub fn force_optic(&mut self, part: Option<usize>) {
        self.forced_optic = part;
    }

    /// How far a variable scope's reticle mesh takes its blend shape, 0 to
    /// 1 (`ScopeZoomHandler.method_1`: clamp((1 − `ReticleBlendCurve` of the
    /// zoom) × 100, 0, 100) of 100, the zoom 0 at the widest): the live
    /// optic's, nought where nothing sets it.
    pub fn reticle_weight(&self) -> f32 {
        let Some((cache, _)) = self.live_optic() else { return 0.0 };
        let (Some(handler), Some(curve)) = (self.states[cache].zoom.as_ref(), self.parts[cache].reticle_curve.as_ref()) else { return 0.0 };
        ((1.0 - curve.evaluate(handler.factor())) * 100.0).clamp(0.0, 100.0) / 100.0
    }

    /// The optic camera's field of view while it is on: aiming, through an
    /// optic, a variable scope's zoom or a fixed one's mode's figure.
    pub fn optic_field_of_view(&self) -> Option<f32> {
        if let Some((part, mode)) = self.forced_optic.and_then(|_| self.live_optic()) {
            return self.parts[part].modes.get(mode)?.optic.as_ref()?.field_of_view;
        }
        let sight = self.current();
        if !self.is_optic(sight) {
            return None;
        }
        let cache = sight.cache?;
        match &self.states[cache].zoom {
            Some(handler) => Some(handler.field_of_view()),
            None => self.parts[cache].modes.get(self.states[cache].prefab_mode)?.optic.as_ref()?.field_of_view,
        }
    }

    /// The key to the next sight (`FirearmController.ChangeAimingMode`):
    /// A mount found with a canted sight in use (`WeaponMountingComponent.
    /// OnFoundPoint`: `ChangeAimingMode` to the first sight not turned aside):
    /// whether the sight changed. None straight, it stays.
    pub fn first_straight(&mut self) -> bool {
        if self.list.is_empty() || self.current().rotation.abs() < SCOPE_ROTATION_THRESHOLD {
            return false;
        }
        let Some(straight) = self.list.iter().position(|sight| sight.rotation.abs() < SCOPE_ROTATION_THRESHOLD) else { return false };
        self.aim_index = straight;
        let changed = self.cycle_scopes();
        self.tell_sight_changed();
        changed
    }
}
