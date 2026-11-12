//! Folding sights, zeroing, ballistics, the iron line and the launcher line.

use super::*;

impl Sights {
    /// The backup sights that fold, and whether each is shown
    /// (`AutoFoldableSight`).
    pub fn folding(&self) -> &[(usize, bool)] {
        &self.folding
    }

    /// The bones a rig reads for the current sight: its aim bone, and the
    /// iron sights' line where the game takes one for it.
    pub fn nodes(&self) -> crate::base::rig::SightNodes {
        let sight = self.current();
        crate::base::rig::SightNodes { weapon: self.weapon, bone: sight.bone, line: self.line.filter(|_| sight.align), muzzle: self.muzzle, own: sight.own }
    }

    // ---- Zeroing (optics spec, "Zeroing") ----

    /// The zeroing points of every sight item's scopes (`Weapon.
    /// RecalculateOpticCalibrationPoints`), from the weapon's default round
    /// (none: no points), its speed factor and the world's gravity; and
    /// whether the weapon is a launcher (`AdjustCollimatorsToTrajectory`).
    pub fn set_ballistics(&mut self, round: Option<&Round>, speed_factor: f32, gravity: V3, launcher: bool) {
        self.launcher = launcher;
        self.points = self
            .parts
            .iter()
            .map(|part| part.scopes.iter().map(|scope| round.map_or_else(Vec::new, |round| zeroing_points(&scope.calibration, round, speed_factor, gravity))).collect())
            .collect();
    }

    /// Whether the weapon is a launcher (`AdjustCollimatorsToTrajectory`).
    pub fn launcher(&self) -> bool {
        self.launcher
    }

    /// The current sight's item and its current scope, where that scope
    /// has zeroing points (`HasOpticCalibrationPoints`).
    pub(super) fn zeroed_scope(&self) -> Option<(usize, usize)> {
        let p = self.current().part?;
        let scope = self.states[p].scope;
        self.points.get(p)?.get(scope).filter(|points| !points.is_empty()).map(|_| (p, scope))
    }

    /// The zeroing keys (`OpticCalibrationSwitch`): refused unless aiming;
    /// the current sight's item moves its current scope's index one place
    /// up or down its points, held at either end. Whether it moved: the
    /// calibration is then asked for.
    pub fn step_zeroing(&mut self, up: bool, aiming: bool) -> bool {
        if !aiming {
            return false;
        }
        let Some((p, scope)) = self.zeroed_scope() else { return false };
        let last = self.points[p][scope].len() - 1;
        let index = &mut self.states[p].zeroing[scope];
        let was = *index;
        *index = if up { (*index + 1).min(last) } else { index.saturating_sub(1) };
        let moved = *index != was;
        self.calibration_asked |= moved;
        moved
    }

    /// The current sight's zeroing distance (`GetCurrentOpticCalibration
    /// Distance`): its item's current scope's distance at its index (the
    /// index back to 0 where it runs past the points), 0 where the scope
    /// lists none; none without an item.
    pub fn zeroing_distance(&self) -> Option<i32> {
        let p = self.current().part?;
        let scope = self.states[p].scope;
        let distances = &self.parts[p].scopes.get(scope)?.calibration;
        if distances.is_empty() {
            return Some(0);
        }
        Some(distances.get(self.zeroing_index(p, scope)).copied().unwrap_or(0))
    }

    /// `GetCurrentOpticCalibrationPointIndex`: the index, 0 where it is past
    /// the scope's points.
    pub(super) fn zeroing_index(&self, p: usize, scope: usize) -> usize {
        let index = self.states[p].zeroing.get(scope).copied().unwrap_or(0);
        let count = self.points.get(p).and_then(|s| s.get(scope)).map_or(0, Vec::len);
        if count > 0 && index >= count {
            0
        } else {
            index
        }
    }

    /// The local zeroing point in force (`_localCalibratePoint`): the game's
    /// axes, z along the flight, y up.
    pub fn local_point(&self) -> V3 {
        self.local_point
    }

    /// The sights have come up or gone down (`ProceduralWeaponAnimation.
    /// IsAiming`): up, a launcher's line measured with the point as it
    /// stands (`method_13`), the shot's squeeze back on where the sight
    /// lines up with the iron sights (`method_14`), and the calibration
    /// asked for (`method_2`); down, the squeeze on and a launcher's line
    /// gone (`method_12`).
    pub fn aim_changed(&mut self, aiming: bool) {
        if aiming {
            self.measure_launcher_line();
            self.iron_line_measured();
            self.calibration_asked = true;
        } else {
            self.sights_down();
        }
    }

    /// Whether the eye is the character's own: a launcher's eye is put on
    /// its line only then (`FirstPersonPointOfView`).
    pub fn set_first_person(&mut self, first_person: bool) {
        self.first_person = first_person;
    }

    /// `method_12`: the shot's squeeze on, a launcher's line gone.
    pub(super) fn sights_down(&mut self) {
        self.shot_squeeze = true;
        self.line_of_sight = SightLine::default();
    }

    /// `method_14`, as far as the squeeze goes: a sight that lines up with
    /// the iron sights' pair puts it back on (the line itself is the
    /// host's, read off its rig: `Hands::note_sights`).
    pub(super) fn iron_line_measured(&mut self) {
        if self.current().align && self.line.is_some() {
            self.shot_squeeze = true;
        }
    }

    /// Whether the shot and the recoil take the field of view's squeeze
    /// (`ShotNeedsFovAdjustments`): off once an optic has been turned.
    pub fn shot_squeeze(&self) -> bool {
        self.shot_squeeze
    }

    /// A launcher's line of sight in force (`_lineOfSightDeltaAngle`,
    /// `_cameraShiftToLineOfSight`, as `method_13` and `method_12` set
    /// them); none on any other weapon, whose line is its iron sights'.
    pub fn launcher_line(&self) -> Option<SightLine> {
        self.launcher.then_some(self.line_of_sight)
    }

    /// What the battle screen has been told since this was last asked.
    pub fn take_hud(&mut self) -> Vec<SightHud> {
        std::mem::take(&mut self.hud)
    }
}

/// A scope's zeroing points (`Weapon.CreateOpticCalibrationData`; optics
/// spec, "Zeroing"): its distances sorted, repeats dropped; the round flown
/// from nought along z at its initial speed times the weapon's speed
/// factor by the game's trajectory, sampled from 0 s every millisecond and
/// carried on from one distance to the next, a distance's point the first
/// sample at least that far from the start; each distance as written then
/// given its point. The game's axes: z along the flight, y up.
pub fn zeroing_points(distances: &[i32], round: &Round, speed_factor: f32, gravity: V3) -> Vec<V3> {
    let mut sorted: Vec<i32> = Vec::new();
    for &distance in distances {
        if !sorted.contains(&distance) {
            sorted.push(distance);
        }
    }
    sorted.sort_unstable();
    let speed = round.initial_speed * speed_factor;
    let mut flight = Trajectory::new([0.0; 3], [0.0, 0.0, speed], round.mass_g, round.diameter_mm, round.ballistic_coefficient, gravity);
    let (mut position, mut reach, mut time) = ([0.0f32; 3], 0.0f32, 0.0f32);
    let mut found = Vec::with_capacity(sorted.len());
    for &distance in &sorted {
        // The game squares the whole number, then takes it for a float.
        let want = distance.wrapping_mul(distance) as f32;
        let mut samples = 0;
        while reach < want && samples < POINT_SAMPLES {
            position = flight.at(time).0;
            reach = position[0] * position[0] + position[1] * position[1] + position[2] * position[2];
            time += POINT_STEP;
            samples += 1;
        }
        found.push(position);
    }
    distances.iter().map(|distance| found[sorted.binary_search(distance).unwrap_or(0)]).collect()
}

/// Shows and hides a build's backup sights that fold, on the tree a rig is
/// drawn from, as its sights have them (`AutoFoldableSight`, set as the
/// sights are found): a folded one shown beside a sight part that looks
/// straight ahead, an unfolded one without. A model of the build not in
/// hand (a picture, one on the ground) keeps its prefab's.
pub fn fold_sights(weapon: &mut trp_data::weapons::weapon::Weapon) {
    let Some(sights) = Sights::of_weapon(weapon) else { return };
    for &(node, shown) in sights.folding() {
        weapon.hierarchy.nodes[node].active = shown;
    }
}

/// Folds the backup sights of a model of a weapon as the game does whenever
/// it makes one (`ObjectsFactory.AssembleMods`, for an item that is a
/// weapon: its picture, one on the ground, the inspect window's and the
/// edit build screen's; in the hands before their own pass, `fold_sights`).
/// Only the folding sights below a sight part's slot are set (the iron
/// sights' own), folded where a sight part that is not an iron sight has
/// an aim bone, on or off, within 1 degree of straight ahead of the weapon
/// (not into a `mod_scope` below it); a barrel's, a handguard's or a
/// mount's folding sight keeps its prefab's. Optics spec, "The sights a
/// weapon has".
pub fn fold_sights_as_made(weapon: &mut trp_data::weapons::weapon::Weapon) {
    let nodes = &weapon.hierarchy.nodes;
    let rig = WeaponRig::new(&weapon.hierarchy);
    let reference = (0..nodes.len()).find(|&i| weapon.hierarchy.path(i) == WEAPON_BONE);
    let below = |mut index: usize, top: usize| loop {
        if index == top {
            return true;
        }
        match nodes[index].parent {
            Some(parent) => index = parent,
            None => return false,
        }
    };
    let mut straight = false;
    let mut set = Vec::new();
    for part in &weapon.sights {
        set.extend(weapon.folding_sights.iter().filter(|fold| below(fold.node, part.slot_bone)).copied());
        if straight || part.iron {
            continue;
        }
        let mut queue = std::collections::VecDeque::from([part.slot_bone]);
        while let Some(at) = queue.pop_front() {
            if nodes[at].name.contains(SIGHT_AIM_BONE) {
                // With no weapon bone to measure by, any aim bone does.
                straight |= reference.is_none_or(|weapon_bone| {
                    let (_, weapon_turn) = rig.placed(weapon_bone);
                    let (_, turn) = rig.placed(at);
                    let back = [-weapon_turn[0], -weapon_turn[1], -weapon_turn[2], weapon_turn[3]];
                    (180.0 - quat_to_euler(quat_mul(back, turn))[1].rem_euclid(360.0)).abs() < SCOPE_ROTATION_THRESHOLD
                });
            }
            queue.extend((0..nodes.len()).filter(|&kid| nodes[kid].parent == Some(at) && nodes[kid].name != SCOPE_SLOT));
        }
    }
    for fold in set {
        weapon.hierarchy.nodes[fold.node].active = fold.mode_on ^ straight;
    }
}
