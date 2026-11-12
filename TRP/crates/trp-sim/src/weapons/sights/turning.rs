//! Calibrating and turning optics and collimators by distance.

use super::*;

impl Sights {
    /// The objects zeroing has turned and each one's turn in its parent,
    /// the game's axes, for a host to put on its own tree.
    pub fn turns(&self) -> impl Iterator<Item = (usize, [f32; 4])> + '_ {
        self.turned.iter().map(|&(node, _)| (node, self.rig.local(node).1))
    }

    /// Each variable scope's ring as its zoom has it (`ScopeZoomHandler.
    /// method_11`, run from `Init` on and after every move of the zoom, so
    /// always the zoom's): its turn in its parent, from its start pivot's at
    /// the widest view to its end pivot's at the narrowest (`game_slerp`).
    /// Only a part with a zoom handler has one, as only an adjustable
    /// optic's `Init` goes on.
    pub fn ring_turns(&self) -> impl Iterator<Item = (usize, [f32; 4])> + '_ {
        self.parts.iter().zip(&self.states).filter_map(|(part, state)| {
            let ring = part.zoom_ring?;
            let zoom = state.zoom.as_ref()?;
            Some((ring.node, game_slerp(ring.start, ring.end, zoom.factor(), !ring.inverse)))
        })
    }

    /// The calibration (`PreCalibrate`, `Calibrate`), in the player's late
    /// update, in first person: where asked and the current sight's item
    /// has points for its scope, the local point taken, the screen told the
    /// distance, a launcher's line measured and the part turned by distance;
    /// then, where asked or aiming while the field of view squeezes the
    /// weapon, the optics turned (not a launcher's) and the collimators.
    pub fn calibrate(&mut self, aiming: bool) {
        if self.calibration_asked {
            if let Some((p, scope)) = self.zeroed_scope() {
                self.local_point = self.points[p][scope][self.zeroing_index(p, scope)];
                if let Some(distance) = self.zeroing_distance() {
                    self.hud.push(SightHud::Zeroing(distance));
                    self.measure_launcher_line();
                    self.turn_by_distance(distance as f32);
                }
            }
        }
        if self.calibration_asked || (aiming && self.scale < 1.0) {
            if !self.launcher {
                self.turn_optics();
            }
            self.turn_collimators();
            self.calibration_asked = false;
        }
    }

    /// The zeroing point in the weapon's tree (`GetWorldCalibrationPoint`):
    /// the fireport, plus the local point turned by Euler (90, -cant, 0) in
    /// the fireport's axes; and the fireport's forward, the turns' up.
    fn world_point(&self) -> Option<(V3, V3)> {
        let (at, turn) = self.rig.placed(self.fireport?);
        let turn_point = quat_mul(turn, quat_euler([90.0, -self.scope_rotation(), 0.0]));
        let off = quat_rotate(turn_point, self.local_point);
        Some((std::array::from_fn(|k| at[k] + off[k]), quat_rotate(turn, [0.0, 0.0, 1.0])))
    }

    /// `Transform.LookAt`: an object turned in its parent so that its
    /// forward is toward `point` and its up as near `up` as that allows;
    /// `then` turns it further in its own axes. `PROVISIONAL`: on the tree
    /// at rest, where the game turns it in the world squeezed along the
    /// hands' root's forward at a wide field of view (the T1's at 25 m,
    /// 4e-5 radians apart at the install's 74 degrees: `tests/sim/zeroing.rs`).
    fn look_at(&mut self, node: usize, point: V3, up: V3, then: [f32; 4]) {
        let (at, _) = self.rig.placed(node);
        let Some(world) = quat_look(std::array::from_fn(|k| point[k] - at[k]), up) else { return };
        let parent = self.rig.parent(node).map_or(NO_TURN, |parent| self.rig.placed(parent).1);
        let back = [-parent[0], -parent[1], -parent[2], parent[3]];
        let local = quat_mul(quat_mul(back, world), then);
        self.note_turned(node);
        let place = self.rig.local(node).0;
        self.rig.set_local(node, (place, local));
    }

    /// `OpticLookAt`: where the current sight's prefab has an optic in any
    /// mode, each mode in turn its collimator and its optic's camera object
    /// turned to the point, up the fireport's forward (the T12's then losing
    /// its roll); the shot's squeeze off.
    fn turn_optics(&mut self) {
        let Some(cache) = self.current().cache else { return };
        if !self.parts[cache].modes.iter().any(|mode| mode.optic.is_some()) {
            return;
        }
        let Some((point, up)) = self.world_point() else { return };
        for m in 0..self.parts[cache].modes.len() {
            if let Some(node) = self.parts[cache].modes[m].collimator {
                self.look_at(node, point, up, quat_euler(COLLIMATOR_TURN));
            }
            let camera = self.parts[cache].modes[m].optic.as_ref().and_then(|optic| optic.camera.map(|node| (node, optic.thermal_at_45)));
            if let Some((node, thermal)) = camera {
                self.look_at(node, point, up, NO_TURN);
                // `IsThermalSightAvailableAt45Degrees`: turned about its z
                // by minus its local roll, where that less 1 is above 0.
                let (place, turn) = self.rig.local(node);
                let roll = quat_to_euler(turn)[2].rem_euclid(360.0);
                if thermal && roll - 1.0 > 0.0 {
                    self.rig.set_local(node, (place, quat_mul(turn, quat_euler([0.0, 0.0, -roll]))));
                }
            }
        }
        self.shot_squeeze = false;
    }

    /// `CollimatorLookAt`: where the prefab has a collimator in any mode,
    /// every mode's turned to the point, up the first one's forward as it
    /// stands, then by -90 degrees about its x.
    fn turn_collimators(&mut self) {
        let Some(cache) = self.current().cache else { return };
        let Some(first) = self.parts[cache].modes.iter().find_map(|mode| mode.collimator) else { return };
        let Some((point, _)) = self.world_point() else { return };
        let up = quat_rotate(self.rig.placed(first).1, [0.0, 0.0, 1.0]);
        for m in 0..self.parts[cache].modes.len() {
            if let Some(node) = self.parts[cache].modes[m].collimator {
                self.look_at(node, point, up, quat_euler(COLLIMATOR_TURN));
            }
        }
    }

    /// `method_13`: a launcher's line of sight, where its sight is not an
    /// optic and its prefab has a collimator: from the first collimator's
    /// object to the zeroing point, in the weapon bone's axes, measured as
    /// the iron sights' line is (in first person, the eye's shift too).
    pub(super) fn measure_launcher_line(&mut self) {
        if !self.launcher {
            return;
        }
        let sight = *self.current();
        if self.is_optic(&sight) {
            return;
        }
        let Some(cache) = sight.cache else { return };
        let Some(lens) = self.parts[cache].modes.iter().find_map(|mode| mode.collimator) else { return };
        let Some((point, _)) = self.world_point() else { return };
        let (at, turn) = self.rig.placed(self.weapon);
        let back = [-turn[0], -turn[1], -turn[2], turn[3]];
        let flat = |there: V3| {
            let v = quat_rotate(back, std::array::from_fn(|k| there[k] - at[k]));
            [-v[1], v[2]]
        };
        let (lens, point, bone) = (flat(self.rig.placed(lens).0), flat(point), flat(self.rig.placed(sight.bone).0));
        self.line_of_sight.angle = ((point[1] - lens[1]) / (point[0] - lens[0])).atan().to_degrees();
        if self.first_person {
            // `Vector2.normalized`: nought below 1e-5.
            let way = [lens[0] - point[0], lens[1] - point[1]];
            let size = (way[0] * way[0] + way[1] * way[1]).sqrt();
            let way = if size > 1e-5 { [way[0] / size, way[1] / size] } else { [0.0; 2] };
            let behind = lens[0] - bone[0];
            self.line_of_sight.shift = [lens[0] + way[0] * behind - bone[0], lens[1] + way[1] * behind - bone[1]];
        }
    }

    /// `RotateCurrentScopeToCorrectDistanceAngle`: the current sight's
    /// prefab's part turned by distance, where its table has this one: its
    /// turn set to Euler (angle, y, z), y and z its turn's own quaternion's
    /// (the game's, kept).
    fn turn_by_distance(&mut self, distance: f32) {
        let Some(cache) = self.current().cache else { return };
        let Some(turned) = self.parts[cache].angle_by_range.as_ref() else { return };
        let Some(&(_, angle)) = turned.table.iter().find(|(d, _)| *d == distance) else { return };
        let axis = turned.axis;
        self.note_turned(axis);
        let (place, turn) = self.rig.local(axis);
        self.rig.set_local(axis, (place, quat_euler([angle, turn[1], turn[2]])));
    }

    /// An object about to be turned, kept with its turn as its prefab has it.
    fn note_turned(&mut self, node: usize) {
        if !self.turned.iter().any(|&(n, _)| n == node) {
            self.turned.push((node, self.rig.local(node).1));
        }
    }

    /// `BattleUIScreen.SightChanged`: a sight item that is not a variable
    /// scope names its zoom where that is above 1, or any of its scope's is.
    pub(super) fn tell_sight_changed(&mut self) {
        let Some(p) = self.current().part else { return };
        if self.parts[p].adjustable.is_some() {
            return;
        }
        let Some((scope, mode)) = self.scope_and_mode() else { return };
        let zooms = self.parts[p].scopes.get(scope).map(|s| s.zooms.as_slice()).unwrap_or(&[]);
        let zoom = zooms.get(mode).copied().unwrap_or(0.0);
        if zoom > 1.0 || zooms.iter().any(|z| *z > 1.0) {
            self.hud.push(SightHud::Magnification(zoom));
        }
    }

    /// `BattleUIScreen.SmoothSightChanged`, a variable scope come to an end
    /// of its range or left one: at the widest the current sight's lowest
    /// zoom, at the narrowest its highest, between, the panel hidden.
    pub(super) fn tell_edge(&mut self, edge: Edge) {
        let zooms: Vec<f32> = match self.current().part {
            Some(p) => self.parts[p].scopes.get(self.states[p].scope).map(|s| s.zooms.clone()).unwrap_or_default(),
            None => {
                self.hud.push(SightHud::HideMagnification);
                return;
            }
        };
        let (lowest, highest) = if zooms.is_empty() { (0.0, 0.0) } else { (zooms.iter().copied().fold(f32::INFINITY, f32::min), zooms.iter().copied().fold(f32::NEG_INFINITY, f32::max)) };
        self.hud.push(match edge {
            Edge::Widest => SightHud::Magnification(lowest),
            Edge::Narrowest => SightHud::Magnification(highest),
            Edge::Between => SightHud::HideMagnification,
        });
    }
}

pub(super) fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

/// `Mathf.InverseLerp`.
/// `ScopeZoomHandler.Slerp`: the sine formula, not normalised; on the short
/// way `p` is turned over where the two stand more than half a turn apart,
/// else the arc goes as they stand (past half a turn). (Where they are one
/// turn the game divides by nought; here `p` comes back.)
pub fn game_slerp(p: [f32; 4], q: [f32; 4], t: f32, short_way: bool) -> [f32; 4] {
    let dot = p[0] * q[0] + p[1] * q[1] + p[2] * q[2] + p[3] * q[3];
    if short_way && dot < 0.0 {
        return game_slerp(p.map(|v| -v), q, t, true);
    }
    let angle = dot.clamp(-1.0, 1.0).acos();
    let sine = angle.sin();
    if sine.abs() < 1e-7 {
        return p;
    }
    let (a, b) = (((1.0 - t) * angle).sin(), (t * angle).sin());
    let scalar = 1.0 / sine;
    std::array::from_fn(|k| (p[k] * a + q[k] * b) * scalar)
}

pub(super) fn inverse_lerp(a: f32, b: f32, value: f32) -> f32 {
    if a == b {
        0.0
    } else {
        ((value - a) / (b - a)).clamp(0.0, 1.0)
    }
}

/// `Mathf.MoveTowards`.
pub(super) fn move_towards(current: f32, target: f32, step: f32) -> f32 {
    if (target - current).abs() <= step {
        target
    } else {
        current + (target - current).signum() * step
    }
}
