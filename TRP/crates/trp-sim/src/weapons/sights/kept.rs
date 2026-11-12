//! What the sight items keep, read off the sights and given back to them.

use super::*;

/// What a sight item keeps of itself (`SightComponent`'s saved fields:
/// `SelectedScope`, `ScopesSelectedModes`, `ScopesCurrentCalibPointIndexes`
/// and `ScopeZoomValue`). As made every figure is nought, and a list
/// shorter than the sight's scopes stands for noughts.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SightKept {
    pub scope: usize,
    pub modes: Vec<usize>,
    pub zeroing: Vec<usize>,
    pub zoom_value: f32,
}

impl Sights {
    /// What a sight part's item keeps now.
    pub fn kept(&self, part: usize) -> SightKept {
        let state = &self.states[part];
        SightKept { scope: state.scope, modes: state.modes.clone(), zeroing: state.zeroing.clone(), zoom_value: state.zoom_value }
    }

    /// Whether a sight part's item keeps just that.
    pub fn keeps(&self, part: usize, kept: &SightKept) -> bool {
        let state = &self.states[part];
        state.scope == kept.scope && state.modes == kept.modes && state.zeroing == kept.zeroing && state.zoom_value == kept.zoom_value
    }

    /// The sights as the weapon's items have them, as the weapon is taken
    /// in hand. The game's are the items' own (`Weapon.AimIndex`, each
    /// sight's `SightComponent`), read by every controller made on the
    /// weapon: the way aimed by, where the weapon has as many, and each
    /// sight part's scope, modes, zeroing and zoom, in the parts' order. A
    /// variable scope's handler is set up afresh on the zoom kept
    /// (`ScopeZoomHandler.Init`). No notice is shown.
    pub fn take_kept(&mut self, aim_index: usize, kept: &[SightKept]) {
        for p in 0..self.parts.len() {
            let Some(kept) = kept.get(p) else { continue };
            let scopes = &self.parts[p].scopes;
            let modes = (0..scopes.len()).map(|s| kept.modes.get(s).copied().filter(|mode| *mode < scopes[s].modes).unwrap_or(0)).collect();
            // (An index past a scope's points is read as nought, as the
            // game reads it: `zeroing_index`.)
            let zeroing = (0..scopes.len()).map(|s| kept.zeroing.get(s).copied().unwrap_or(0)).collect();
            let scope = if kept.scope < scopes.len() { kept.scope } else { 0 };
            let state = &mut self.states[p];
            (state.scope, state.modes, state.zeroing, state.zoom_value, state.prefab_mode) = (scope, modes, zeroing, kept.zoom_value, 0);
            if let Some(old) = state.zoom.take() {
                state.zoom = Some(ZoomHandler::new(old.data, old.curve, &mut state.zoom_value));
            }
            self.show_mode(p);
        }
        self.aim_index = if aim_index < self.list.len() { aim_index } else { 0 };
        self.calibration_asked = true;
        self.work_out_eye();
    }
}
