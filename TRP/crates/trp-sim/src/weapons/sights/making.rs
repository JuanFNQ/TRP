//! Making the sights from a weapon and reading which sight is in use.

use super::*;

impl Sights {
    /// A weapon's, as built.
    pub fn of_weapon(weapon: &trp_data::weapons::weapon::Weapon) -> Option<Self> {
        Self::new(&weapon.hierarchy, weapon.sights.clone(), &weapon.folding_sights, AimPlanes::of(weapon))
    }

    /// Finds them (`FindAimTransforms`) on a weapon's tree with its sight
    /// parts. None for a tree with no aim bone at all.
    pub fn new(hierarchy: &Hierarchy, parts: Vec<SightPart>, folding: &[FoldingSight], planes: AimPlanes) -> Option<Self> {
        let nodes = &hierarchy.nodes;
        let weapon = (0..nodes.len()).find(|&i| hierarchy.path(i) == WEAPON_BONE)?;
        let moved = (0..nodes.len()).find(|&i| hierarchy.path(i) == MOVED_BONE);
        let rig = WeaponRig::new(hierarchy);
        let children = |of: usize| (0..nodes.len()).filter(move |&i| nodes[i].parent == Some(of));
        let below = |mut index: usize, top: usize| {
            let mut depth = 0;
            while index != top {
                index = nodes[index].parent?;
                depth += 1;
            }
            Some(depth)
        };
        let active = |mut index: usize| loop {
            if !nodes[index].active {
                return false;
            }
            match nodes[index].parent {
                Some(parent) => index = parent,
                None => return true,
            }
        };
        // How a bone is turned about the weapon's upright axis, from
        // straight back (`GetRotationRelativeToParent(...).eulerAngles.y`).
        let turned = |index: usize| {
            let (_, weapon_turn) = rig.placed(weapon);
            let (_, turn) = rig.placed(index);
            let back = [-weapon_turn[0], -weapon_turn[1], -weapon_turn[2], weapon_turn[3]];
            quat_to_euler(quat_mul(back, turn))[1].rem_euclid(360.0)
        };
        let within = |index: usize| (180.0 - turned(index)).abs() < SIGHT_TURN_LIMIT;
        // The first bone of that name below a bone, depth first, the bone
        // itself left out (`TransformTools.FindTransformRecursive`).
        let first_named = |top: usize, name: &str| -> Option<usize> {
            fn walk(nodes: &[trp_data::unity::extract::Node], at: usize, name: &str) -> Option<usize> {
                for kid in (0..nodes.len()).filter(|&i| nodes[i].parent == Some(at)) {
                    if nodes[kid].name == name {
                        return Some(kid);
                    }
                    if let Some(found) = walk(nodes, kid, name) {
                        return Some(found);
                    }
                }
                None
            }
            walk(nodes, top, name)
        };
        // The sight parts by their items' names (the template's id: `Item.
        // Name` is its localization key), the game's walk kept for alike.
        let mut by_name: Vec<usize> = (0..parts.len()).collect();
        by_name.sort_by(|&a, &b| parts[a].template.cmp(&parts[b].template));
        let mut list: Vec<Sight> = Vec::new();
        let mut straight = false;
        for &p in &by_name {
            let part = &parts[p];
            if part.iron && !part.on_sight {
                continue;
            }
            // Every bone whose name has `mod_aim_camera`, breadth first
            // from the slot's bone, on only, not into a `mod_scope` below.
            let mut found = Vec::new();
            let mut queue = std::collections::VecDeque::from([part.slot_bone]);
            while let Some(at) = queue.pop_front() {
                if nodes[at].name.contains(SIGHT_AIM_BONE) && active(at) {
                    found.push(at);
                }
                queue.extend(children(at).filter(|&kid| nodes[kid].name != SCOPE_SLOT));
            }
            // The prefab's modes are the slot's first child's.
            let cache = children(part.slot_bone).next().and_then(|first| parts.iter().position(|q| q.root == first && q.has_prefab_cache));
            for bone in found {
                let depth = below(bone, part.slot_bone).unwrap_or(0);
                if let Some(known) = list.iter_mut().find(|sight| sight.bone == bone) {
                    if depth < known.depth {
                        known.part = Some(p);
                    }
                    continue;
                }
                if !within(bone) {
                    continue;
                }
                let rotation = 180.0 - turned(bone);
                if rotation.abs() < SCOPE_ROTATION_THRESHOLD {
                    straight = true;
                }
                let optic_bone = cache.is_some_and(|c| parts[c].modes.iter().any(|mode| mode.optic.as_ref().is_some_and(|optic| optic.bone == Some(bone))));
                let own = nodes[bone].name == WEAPON_AIM_BONE;
                list.push(Sight { bone, part: Some(p), cache, depth, rotation, align: false, optic_bone, eye_guard_shift: if optic_bone { part.eye_guard_shift } else { 0.0 }, own });
            }
        }
        let folding = folding.iter().map(|fold| (fold.node, fold.mode_on ^ straight)).collect();
        let deepest = |part: &str| {
            let mut best: Option<(usize, usize)> = None;
            for index in (0..nodes.len()).filter(|&i| nodes[i].name.contains(part)) {
                let Some(depth) = below(index, weapon) else { continue };
                if best.is_none_or(|(_, d)| depth > d) {
                    best = Some((index, depth));
                }
            }
            best.map(|(index, _)| index)
        };
        let mut line = None;
        // The iron sights and the weapon's own bones are listed with their
        // line taken and no turn noted.
        let plain = |bone: usize| Sight { bone, part: None, cache: None, depth: 0, rotation: 0.0, align: true, optic_bone: false, eye_guard_shift: 0.0, own: nodes[bone].name == WEAPON_AIM_BONE };
        if !straight {
            // The iron sights: the first, by name, with an aim bone in
            // bounds; and the line through the rear and front points.
            for &p in by_name.iter().filter(|&&p| parts[p].iron) {
                if let Some(bone) = first_named(parts[p].slot_bone, SIGHT_AIM_BONE).filter(|&bone| within(bone)) {
                    list.push(Sight { part: Some(p), ..plain(bone) });
                    break;
                }
            }
            line = deepest("mod_align_rear").zip(deepest("mod_align_front")).map(|(rear, front)| [rear, front]);
        }
        // Else the first sight's bone anywhere on the weapon, else the
        // weapon's own.
        if list.is_empty() {
            if let Some(bone) = moved.and_then(|moved| first_named(moved, SIGHT_AIM_BONE)).filter(|&bone| within(bone)) {
                list.push(plain(bone));
            }
        }
        if list.is_empty() {
            if let Some(bone) = moved.and_then(|moved| first_named(moved, WEAPON_AIM_BONE)) {
                list.push(plain(bone));
            }
        }
        if list.is_empty() {
            return None;
        }
        let depths = list
            .iter()
            .map(|sight| {
                let (at, turn) = rig.placed(weapon);
                let back = [-turn[0], -turn[1], -turn[2], turn[3]];
                let there = rig.placed(sight.bone).0;
                -quat_rotate(back, std::array::from_fn(|k| there[k] - at[k]))[1]
            })
            .collect();
        // Each sight item's state as made, and its prefab's mode
        // (`SightModVisualControllers.OnEnable`: the first scope whose
        // modes are as many as the prefab's, else the first mode).
        let states = parts
            .iter()
            .map(|part| {
                let mut state = SightState { scope: 0, modes: vec![0; part.scopes.len()], zeroing: vec![0; part.scopes.len()], zoom_value: 0.0, prefab_mode: 0, zoom: None };
                if let (Some(data), Some(curve)) = (part.adjustable, part.zoom_curve.clone()) {
                    state.zoom = Some(ZoomHandler::new(data, curve, &mut state.zoom_value));
                }
                state
            })
            .collect();
        let muzzle = (0..nodes.len()).find(|&i| nodes[i].name == "fireport");
        let fireport = moved.and_then(|moved| first_named(moved, "fireport"));
        let points = parts.iter().map(|part| vec![Vec::new(); part.scopes.len()]).collect();
        let mut sights = Self {
            parts,
            states,
            list,
            aim_index: 0,
            straight,
            line,
            weapon,
            muzzle,
            folding,
            planes,
            depths,
            scale: 1.0,
            eye: SightEye::iron(0.0),
            rig,
            turned: Vec::new(),
            fireport,
            forced_optic: None,
            points,
            launcher: false,
            calibration_asked: false,
            local_point: [0.0; 3],
            shot_squeeze: true,
            line_of_sight: SightLine::default(),
            first_person: true,
            hud: Vec::new(),
            grenade_kept: None,
        };
        for part in 0..sights.parts.len() {
            sights.show_mode(part);
        }
        sights.work_out_eye();
        Some(sights)
    }

    /// The ways to aim, in the order the key goes through them.
    pub fn list(&self) -> &[Sight] {
        &self.list
    }

    /// An underbarrel launcher's sight made the only one, or the weapon's own
    /// given back (`ProceduralWeaponAnimation.IsGrenadeLauncher`, whose change
    /// finds the aim transforms again; underbarrel-launcher spec, "While the
    /// launcher is in use"). In grenade mode the search clears the list and
    /// the iron sights' pair, and adds the launcher's `mod_aim_camera` alone,
    /// with no part, lined up where it can be (`FindUnderbarrelWeaponSight`),
    /// then works out the eye (`method_11`); the weapon's `AimIndex` is kept
    /// and points at that one sight; the shot's direction goes back to the
    /// barrel's and a launcher's line is gone (`method_12`). It returns before
    /// the folding sights, which stay as they were; given back, the weapon's
    /// sights are as they were set aside, the folds as its sights had them.
    /// Whether anything changed.
    pub fn set_grenade(&mut self, bone: Option<usize>) -> bool {
        match bone {
            Some(bone) if self.grenade_kept.is_none() => {
                let (at, turn) = self.rig.placed(self.weapon);
                let back = [-turn[0], -turn[1], -turn[2], turn[3]];
                let there = self.rig.placed(bone).0;
                let depth = -quat_rotate(back, std::array::from_fn(|k| there[k] - at[k]))[1];
                let sight = Sight { bone, part: None, cache: None, depth: 0, rotation: 0.0, align: true, optic_bone: false, eye_guard_shift: 0.0, own: false };
                let kept = (std::mem::replace(&mut self.list, vec![sight]), std::mem::replace(&mut self.depths, vec![depth]), self.line.take());
                self.grenade_kept = Some(kept);
            }
            None if self.grenade_kept.is_some() => {
                if let Some((list, depths, line)) = self.grenade_kept.take() {
                    (self.list, self.depths, self.line) = (list, depths, line);
                }
            }
            _ => return false,
        }
        self.sights_down();
        self.work_out_eye();
        true
    }

    /// Whether an underbarrel launcher's sight is the only one (`set_grenade`).
    pub fn grenade(&self) -> bool {
        self.grenade_kept.is_some()
    }

    /// Whether a sight part looks straight ahead (the iron sights are then
    /// left out, and the backup sights that fold do).
    pub fn straight(&self) -> bool {
        self.straight
    }

    /// The sight parts, in the order the game walks the items.
    pub fn parts(&self) -> &[SightPart] {
        &self.parts
    }

    /// The weapon's `AimIndex`.
    pub fn aim_index(&self) -> usize {
        self.aim_index
    }

    /// What the sight items keep, taken from the same weapon's sights as
    /// last built, where the weapon in hand is built anew (the app's: the
    /// game keeps it on the items, which a magazine of another kind put in
    /// does not touch, where here each build has its rig and its sights,
    /// so the scope's state would change with the magazine): each sight item's scope, modes, zeroing and zoom, found
    /// by the item's id, else by its template in the order walked, where
    /// its scopes are as many; and the sight chosen, where the ways to aim
    /// are as many. No notice is shown (nothing was chosen).
    pub fn take_state(&mut self, from: &Sights) {
        let mut used = vec![false; from.parts.len()];
        for p in 0..self.parts.len() {
            let part = &self.parts[p];
            let free = |q: &usize| !used[*q] && from.parts[*q].scopes.len() == part.scopes.len();
            let found = (0..from.parts.len()).filter(free).find(|&q| from.parts[q].item == part.item).or_else(|| (0..from.parts.len()).filter(free).find(|&q| from.parts[q].template == part.template));
            let Some(q) = found else { continue };
            used[q] = true;
            self.states[p] = from.states[q].clone();
            self.show_mode(p);
        }
        if from.list.len() == self.list.len() {
            self.aim_index = from.aim_index;
        }
        self.calibration_asked = true;
        self.work_out_eye();
    }

    /// Whether the game's camera code takes the sight's figures
    /// (`AimIndex < ScopeAimTransforms.Count`).
    pub fn in_range(&self) -> bool {
        self.aim_index < self.list.len()
    }

    /// The sight aimed by (`CurrentScope`).
    pub fn current(&self) -> &Sight {
        &self.list[self.aim_index % self.list.len()]
    }
}
