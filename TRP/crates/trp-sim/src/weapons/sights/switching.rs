//! Changing scope, cycling scopes and magnification, the zoom frame and the eye.

use super::*;

impl Sights {
    /// the next, wrapping; mounted, any turned from straight ahead passed
    /// over. Whether the sight changed: the eye then goes to the new one
    /// at the aiming rate, and the main camera to its field of view
    /// (`CycleScopes`). The caller refuses it in blind fire and while the
    /// left hand is busy.
    pub fn change_aim_scope(&mut self, mounted: bool) -> bool {
        let count = self.list.len();
        let mut next = self.aim_index + 1;
        if next >= count {
            next = 0;
        }
        if mounted {
            while next != self.aim_index && self.list[next].rotation.abs() >= SCOPE_ROTATION_THRESHOLD {
                next += 1;
                if next >= count {
                    next = 0;
                }
            }
            if next == self.aim_index || self.list[next].rotation.abs() >= SCOPE_ROTATION_THRESHOLD {
                return false;
            }
        }
        if next == self.aim_index {
            return false;
        }
        self.aim_index = next;
        let changed = self.cycle_scopes();
        // `RaiseSightChangedEvent`, the index set whatever `CycleScopes` did.
        self.tell_sight_changed();
        changed
    }

    /// The weapon's sight index set (`CycleScopes`): the scope of the item
    /// it falls in is chosen by counting every sight item's scopes in the
    /// game's walk of the items, and set on the sight listed at that count,
    /// as the game does (on the G36 the carry handle's backup is its scope
    /// 1; on the M60E6 the Specter's backup leaves it on scope 0). Where the
    /// count runs past the list the game stops there, with the eye and the
    /// field of view as they were: false.
    pub(super) fn cycle_scopes(&mut self) -> bool {
        let index = self.aim_index;
        if index >= self.list.len() {
            return false;
        }
        let mut count = 0;
        for item in 0..self.parts.len() {
            let scopes = self.parts[item].scopes.len();
            let set = if index == count {
                Some(0)
            } else if index > count && index < count + scopes {
                Some(index - count)
            } else {
                count += scopes;
                continue;
            };
            let Some(listed) = self.list.get(count) else { return false };
            let target = listed.part.unwrap_or(item);
            let scopes_count = self.parts[target].scopes.len();
            if let Some(scope) = set.filter(|_| scopes_count > 0) {
                self.states[target].scope = scope % scopes_count;
            }
            break;
        }
        // A launcher's line, the iron sights' (the shot's squeeze back on)
        // and the calibration asked for (`method_13`, `method_14`,
        // `method_2`).
        self.measure_launcher_line();
        self.iron_line_measured();
        self.calibration_asked = true;
        self.work_out_eye();
        true
    }

    /// The magnification key (`GetSightStateAfterChangeMagnification`,
    /// `SetScopeMode`, `Firearms.UpdateScopesMode`): the scope the sight
    /// index counts to, among the sight items listed sorted by their names,
    /// goes to its next mode; every item's prefab shows its mode, and every
    /// variable scope snaps to the far end of its range. Whether it was
    /// done (the hands' animator then takes `ModSwitch`, and the main
    /// camera and the eye go by the new mode: `OnScopesModeUpdated`).
    pub fn change_magnification(&mut self) -> bool {
        let mut by_name: Vec<usize> = (0..self.parts.len()).filter(|&p| self.list.iter().any(|sight| sight.part == Some(p))).collect();
        if by_name.is_empty() {
            return false;
        }
        by_name.sort_by(|&a, &b| self.parts[a].template.cmp(&self.parts[b].template));
        let mut count = 0;
        for p in by_name {
            for scope in 0..self.parts[p].scopes.len() {
                if count == self.aim_index {
                    let modes = self.parts[p].scopes[scope].modes;
                    let next = self.states[p].modes[scope] + 1;
                    self.states[p].modes[scope] = if next >= modes { 0 } else { next };
                }
                count += 1;
            }
        }
        // The screen hears of the new modes (`SetScopeMode`), then of each
        // variable scope's snap to an end (`ForceChangeScopeState`).
        self.tell_sight_changed();
        for p in 0..self.parts.len() {
            self.show_mode(p);
            let state = &mut self.states[p];
            if let Some(handler) = state.zoom.as_mut() {
                if handler.snap(&mut state.zoom_value) {
                    let edge = handler.edge;
                    self.tell_edge(edge);
                }
            }
        }
        // `OnScopesModeUpdated`: the calibration asked for, the eye worked
        // out, the shot's way and squeeze as the sights going down leave
        // them, then a launcher's line and the iron sights' measured.
        self.calibration_asked = true;
        self.work_out_eye();
        self.sights_down();
        self.measure_launcher_line();
        self.iron_line_measured();
        true
    }

    /// A part's prefab shows the mode of its first scope that has as many
    /// modes as the prefab (`SightModVisualControllers.UpdateSightMode`;
    /// a prefab of one mode stays as it is).
    pub(super) fn show_mode(&mut self, part: usize) {
        let prefab_modes = self.parts[part].modes.len();
        if prefab_modes < 2 {
            return;
        }
        let state = &mut self.states[part];
        if let Some(scope) = self.parts[part].scopes.iter().position(|scope| scope.modes == prefab_modes) {
            let mode = state.modes.get(scope).copied().unwrap_or(0);
            if mode < prefab_modes {
                state.prefab_mode = mode;
            }
        }
    }

    /// One frame of the zoom's wheel (`ScopeZoomHandler`): aiming, only the
    /// variable scope aimed through as an optic takes it (its optic camera
    /// runs it); else every variable scope on the weapon does. `input` is
    /// the wheel's turn times `OpticSensitivity`, wheel up positive, `dt`
    /// the frame's time.
    pub fn zoom_frame(&mut self, input: f32, dt: f32, aiming: bool) {
        let live = if aiming {
            let sight = *self.current();
            match sight.cache.filter(|_| self.is_optic(&sight)) {
                Some(cache) => vec![cache],
                None => Vec::new(),
            }
        } else {
            (0..self.parts.len()).collect()
        };
        for p in live {
            let state = &mut self.states[p];
            if let Some(handler) = state.zoom.as_mut() {
                if handler.update(input, dt, &mut state.zoom_value) {
                    let edge = handler.edge;
                    self.tell_edge(edge);
                }
            }
        }
    }

    /// The squeeze of the weapon at the field of view has changed
    /// (`SetFovParams`): the eye goes by it.
    pub fn set_scale(&mut self, scale: f32) {
        if scale != self.scale {
            self.scale = scale;
            self.work_out_eye();
        }
    }

    /// The eye as last worked out (`FindCameraDepth`).
    pub fn eye(&self) -> SightEye {
        self.eye
    }

    /// Where the eye goes for the current sight (`UpdateFarPlane`,
    /// `FindCameraDepth`): the far plane is the nearest optic's bone, less
    /// its eyecup, or the prefab's far plane if that is nearer; the plane
    /// aimed at is the far one through an optic and the prefab's ordinary
    /// one (or the plane the sight names) else, no deeper than the far
    /// plane less the weapon bone's own place.
    pub(super) fn work_out_eye(&mut self) {
        let mut nearest = f32::INFINITY;
        let mut past = 0.0f32;
        for (i, sight) in self.list.iter().enumerate() {
            if self.is_optic(sight) {
                nearest = nearest.min(self.depths[i] - sight.eye_guard_shift);
                past = (nearest - self.planes.far).max(0.0);
            }
        }
        let far = nearest.min(self.planes.far);
        let sight = *self.current();
        let custom = sight.part.map(|p| self.parts[p].custom_aim_plane.as_str()).filter(|name| !name.is_empty()).and_then(|name| self.planes.custom.iter().find(|(n, _)| n == name)).map(|(_, depth)| *depth);
        let optic = self.is_optic(&sight);
        let plane = custom.unwrap_or(if optic { self.planes.far } else { self.planes.default });
        let depth = plane.min(far - self.planes.weapon_y);
        self.eye = if optic {
            let cache = sight.cache.map(|c| &self.parts[c]);
            let mode = cache.and_then(|c| c.modes.get(self.states[sight.cache.unwrap_or(0)].prefab_mode));
            let distance = mode
                .and_then(|m| m.optic.as_ref())
                .or_else(|| cache.and_then(|c| c.modes.iter().find_map(|m| m.optic.as_ref())))
                .map_or(0.0, |optic| optic.distance_to_camera);
            SightEye { depth, distance: distance + past }
        } else {
            SightEye::iron(depth)
        };
    }
}
