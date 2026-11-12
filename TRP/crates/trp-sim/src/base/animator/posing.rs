//! Posing a controller's parts from its clips.

use super::*;

impl Controller {
    /// The bones a mix's layers may leave as they stood: those of each
    /// layer with a kept share on it.
    pub(super) fn standing(&self, layers: &[LayerMix]) -> Vec<bool> {
        let mut standing = vec![false; self.bones.len()];
        for mix in layers.iter().filter(|mix| mix.kept > 0.0) {
            for (standing, moves) in standing.iter_mut().zip(&self.layers[mix.layer].moves) {
                *standing |= *moves;
            }
        }
        standing
    }

    /// A clip's `part` (0 its first, 1 its rest) sampled at `time` into
    /// `values` for the `chosen` bones alone (`pose_part_by`'s sparse
    /// sampling): each curve of their places and turns in that part, and
    /// in the first the root's travel and turn where the root is chosen;
    /// every other curve left at 0, which no chosen bone reads.
    fn sample_chosen(&self, def: &ClipDef, part: usize, chosen: &[bool], clip: &Clip, time: f32, values: &mut Vec<f32>) {
        values.clear();
        values.resize(clip.curve_count(), 0.0);
        let mut take = |first: usize, count: usize| {
            for curve in first..first + count {
                if let Some(value) = values.get_mut(curve) {
                    *value = clip.sample_curve(time, curve);
                }
            }
        };
        for (bone, tracks) in def.bones.iter().enumerate() {
            if !chosen.get(bone).copied().unwrap_or(false) || self.always[bone] != (part == 0) {
                continue;
            }
            if let Some(curve) = tracks.position {
                take(curve, 3);
            }
            if let Some((curve, euler)) = tracks.rotation {
                take(curve, if euler { 3 } else { 4 });
            }
            if part == 0 && self.root_bone == Some(bone) {
                for curve in def.root_path.into_iter().flatten().chain(def.root_turn.into_iter().flatten()) {
                    take(curve, 1);
                }
            }
        }
    }

    /// The pose of every bone an update's mix makes: the bones the update
    /// worked out as it left them, and the rest from the mix (`pose_part`),
    /// in the same arithmetic, bone by bone, as the update itself. A pure
    /// function of the mix: asked at once or later, the same.
    pub fn pose_of(&self, mix: &Mix) -> Vec<([f32; 3], [f32; 4])> {
        let mut pose = self.rest_pose();
        let mut chosen: Vec<bool> = self.always.iter().map(|always| !always).collect();
        for (&bone, placed) in self.always_list.iter().zip(&mix.always) {
            pose[bone] = *placed;
        }
        for &(bone, placed) in &mix.standing {
            pose[bone] = placed;
            chosen[bone] = false;
        }
        let mut samples = [Vec::new(), Vec::new()];
        self.pose_part(&mix.layers, &chosen, false, &mut pose, None, &mut samples, None);
        pose
    }

    /// The pose for drawing an update's mix makes, where one was asked for
    /// (`Animator::set_drawn_smooth`): the bones a layer left standing as
    /// the update worked them out, every other from the mix on the clips'
    /// smooth curves.
    pub fn drawn_of(&self, mix: &Mix) -> Option<Vec<([f32; 3], [f32; 4])>> {
        let drawn = mix.drawn.as_ref()?;
        let mut pose = self.rest_pose();
        let mut chosen = vec![true; self.bones.len()];
        for &(bone, placed) in &drawn.standing {
            pose[bone] = placed;
            chosen[bone] = false;
        }
        let mut samples = [Vec::new(), Vec::new()];
        self.pose_part(&drawn.layers, &chosen, true, &mut pose, None, &mut samples, None);
        Some(pose)
    }

    /// The scale of each bone of `scaled` from a mix: blended as a place
    /// is, layer over layer (an additive layer adds what its clips scale
    /// from their reference pose). (Those bones are always worked out: their
    /// curves are in the part every update samples.)
    pub(super) fn scales_of(&self, layers: &[LayerMix], samples: &mut Vec<f32>) -> Vec<[f32; 3]> {
        let mut scales: Vec<[f32; 3]> = self.scaled.iter().map(|i| self.bones[*i].scale.unwrap_or([1.0; 3])).collect();
        for mix in layers {
            let (index, layer_weight, total) = (mix.layer, mix.weight, mix.total);
            let layer = &self.layers[index];
            if total <= 0.0 {
                continue;
            }
            let mut sums = vec![[0.0f32; 3]; scales.len()];
            for share in &mix.shares {
                let def = &self.clips[share.clip];
                let length = def.clip.duration();
                def.clip.sample(share.clip_share(def.clip.looping, share.now) * length, samples);
                let reference = &def.clip.reference_pose;
                for (j, i) in self.scaled.iter().enumerate() {
                    if !layer.moves[*i] {
                        continue;
                    }
                    let rest = self.bones[*i].scale.unwrap_or([1.0; 3]);
                    let read = |values: &[f32]| def.bones[*i].scale.map_or(rest, |c| [values[c], values[c + 1], values[c + 2]]);
                    let s = read(samples);
                    if layer.additive {
                        if def.referenced {
                            let s0 = read(reference);
                            (0..3).for_each(|k| sums[j][k] += (s[k] - s0[k]) * share.weight);
                        }
                    } else {
                        (0..3).for_each(|k| sums[j][k] += s[k] * share.weight);
                    }
                }
            }
            let amount = (layer_weight * total).clamp(0.0, 1.0);
            for (j, i) in self.scaled.iter().enumerate() {
                if !layer.moves[*i] {
                    continue;
                }
                for k in 0..3 {
                    if layer.additive {
                        scales[j][k] += sums[j][k] * layer_weight;
                    } else if index == 0 {
                        scales[j][k] = sums[j][k] / total;
                    } else {
                        scales[j][k] += (sums[j][k] / total - scales[j][k]) * amount;
                    }
                }
            }
        }
        scales
    }

    /// Works out the bones `chosen` (by place) of a pose from a mix, layer
    /// over layer, each bone by itself: no bone's value goes by another's,
    /// so any set of them comes out as it would among all. Each starts from
    /// its rest; one a layer leaves as it stood takes its place in `prior`
    /// (the pose before the update) by that layer's kept share, where
    /// `prior` is given (`standing`: the update works such bones out, and a
    /// mix keeps them). With `smooth`, the clips' sampled curves are taken
    /// on curves without corners: for the pose that is drawn, and for
    /// nothing else. With `driven`, the update's own working-out: the curves
    /// that drive parameters, the root's travel and turn and the clips'
    /// events too.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn pose_part(&self, layers: &[LayerMix], chosen: &[bool], smooth: bool, pose: &mut [Bone], prior: Option<&[Bone]>, samples: &mut [Vec<f32>; 2], driven: Option<&mut Driven>) {
        self.pose_part_by(layers, chosen, smooth, pose, prior, samples, driven, false);
    }

    /// `pose_part`, the clips' curves sampled whole, or (`sparse`, for a few
    /// bones asked for between updates: `Animator::bones_with`) only those
    /// the chosen bones read, each by `Clip::sample_curve`, which gives a
    /// curve as `Clip::sample` gives it among the rest: the same keys and
    /// arithmetic, so the bones come out the same to the last bit. Not with
    /// `smooth` or `driven`, which read every curve.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn pose_part_by(&self, layers: &[LayerMix], chosen: &[bool], smooth: bool, pose: &mut [Bone], prior: Option<&[Bone]>, samples: &mut [Vec<f32>; 2], mut driven: Option<&mut Driven>, sparse: bool) {
        let sparse = sparse && !smooth && driven.is_none();
        let bone_count = self.bones.len();
        // Which parts of the clips the bones chosen are in; the curves and
        // the root are in the first.
        let needs = [
            driven.is_some() || chosen.iter().zip(&self.always).any(|(chosen, always)| *chosen && *always),
            chosen.iter().zip(&self.always).any(|(chosen, always)| *chosen && !*always),
        ];
        if !needs[0] && !needs[1] {
            return;
        }
        for (bone, place) in pose.iter_mut().enumerate().filter(|(bone, _)| chosen[*bone]) {
            *place = (self.bones[bone].position, self.bones[bone].rotation);
        }
        let prior = prior.filter(|prior| prior.len() == bone_count);
        // Whether a layer has set a bone on this update. A bone no layer
        // sets stays at its rest.
        let mut set = vec![false; bone_count];

        for mix in layers {
            let (index, layer_weight, total, kept) = (mix.layer, mix.weight, mix.total, mix.kept);
            let layer = &self.layers[index];
            // A state with no motion on an override layer that does not
            // write defaults leaves the layer's bones as they stand
            // (`m_WriteDefaultValues` off: the engine writes nothing of
            // them): one no layer below has set keeps where it was after
            // the last update, by that state's share, rather than going
            // back to rest. The Rhino's `FIRE` has no motion on its
            // `Cylinder` layer and writes no defaults (its `Hands` layer's
            // mask leaves the cylinder out): the cylinder stays at the
            // chamber it was turned to, where it went back to chamber 0's
            // angle until `FIRE ARM` or `IDLE` turned it again (the
            // cylinder seen twitching when shooting).
            if kept > 0.0 {
                if let Some(prior) = prior {
                    let unfilled = (1.0 - total).max(0.0);
                    let share = if unfilled > 0.0 { (kept / unfilled).clamp(0.0, 1.0) } else { 0.0 };
                    for i in 0..bone_count {
                        if !chosen[i] || !layer.moves[i] || set[i] {
                            continue;
                        }
                        let (p, q) = prior[i];
                        for k in 0..3 {
                            pose[i].0[k] += (p[k] - pose[i].0[k]) * share;
                        }
                        pose[i].1 = quat_lerp(pose[i].1, q, share);
                    }
                }
            }
            if total <= 0.0 {
                continue;
            }
            // The root follows the first layer alone, unless its state says
            // the animation is not to move the character.
            let moves_root = index == 0;
            let mut position = vec![[0.0f32; 3]; bone_count];
            let mut rotation = vec![[0.0f32; 4]; bone_count];
            let mut layer_curves: Vec<(f32, f32)> = vec![(0.0, 0.0); self.parameters.len()];
            for share in &mix.shares {
                let def = &self.clips[share.clip];
                let length = def.clip.duration();
                let at = |cycles: f32| share.clip_share(def.clip.looping, cycles) * length;
                if let Some(driven) = driven.as_deref_mut() {
                    // The clip's events its time went over on this update.
                    if share.weight > 0.0 && length > 0.0 && share.now > share.previous {
                        let (from, to) = (share.previous + share.offset, share.now + share.offset);
                        for (n, event) in def.clip.events.iter().enumerate() {
                            // (Played backwards, an event is met where the
                            // state's time reaches its place from the end.)
                            let at = if share.reversed { 1.0 - event.time / length } else { event.time / length };
                            let passed = if def.clip.looping {
                                (to - at).floor() > (from - at).floor()
                            } else {
                                from.clamp(0.0, 1.0) < at && at <= to.clamp(0.0, 1.0)
                            };
                            if passed {
                                driven.passed.push((share.clip, n));
                            }
                        }
                    }
                    if let (true, Some(path)) = (moves_root, def.root_path) {
                        // Whole cycles count by the clip's start-to-end
                        // displacement; the rest by the path between the two
                        // times. (Its three curves alone are sampled.)
                        let value = |time: f32| -> [f32; 3] { path.map(|c| def.clip.sample_curve(time, c)) };
                        let (first, last) = (value(0.0), value(length));
                        let (from, to) = (value(at(share.previous)), value(at(share.now)));
                        let cycles = if def.clip.looping {
                            (share.now + share.offset).floor() - (share.previous + share.offset).floor()
                        } else {
                            0.0
                        };
                        for k in 0..3 {
                            if !stays_in_pose(&def.clip, k) {
                                driven.root[k] += share.weight * (to[k] - from[k] + cycles * (last[k] - first[k]));
                            }
                        }
                    }
                    if let (true, Some(curves)) = (moves_root, def.root_turn) {
                        // The root's turning about the vertical, the same way.
                        let bearing = |time: f32| -> f32 { 2.0 * def.clip.sample_curve(time, curves[1]).atan2(def.clip.sample_curve(time, curves[3])).to_degrees() };
                        let whole = bearing(length) - bearing(0.0);
                        let cycles = if def.clip.looping {
                            (share.now + share.offset).floor() - (share.previous + share.offset).floor()
                        } else {
                            0.0
                        };
                        let (from, to) = (bearing(at(share.previous)), bearing(at(share.now)));
                        driven.turn += share.weight * (to - from + cycles * whole);
                    }
                }
                let time = at(share.now);
                let parts = [Some(&def.clip), def.rest.as_ref()];
                for (part, (clip, values)) in parts.into_iter().zip(samples.iter_mut()).enumerate() {
                    let Some(clip) = clip.filter(|_| needs[part]) else { continue };
                    if smooth {
                        clip.sample_smooth(time, values);
                    } else if sparse {
                        self.sample_chosen(def, part, chosen, clip, time, values);
                    } else {
                        clip.sample(time, values);
                    }
                }
                if driven.is_some() {
                    let (values, reference) = (&samples[0], &def.clip.reference_pose);
                    for (parameter, curve) in &def.curves {
                        // On an additive layer a curve gives what it has
                        // moved from the clip's own reference pose, as a
                        // bone does.
                        let value = match (layer.additive, def.referenced) {
                            (false, _) => values[*curve],
                            (true, true) => values[*curve] - reference[*curve],
                            (true, false) => continue,
                        };
                        layer_curves[*parameter].0 += value * share.weight;
                        layer_curves[*parameter].1 += share.weight;
                    }
                }
                for (i, (bone, tracks)) in self.bones.iter().zip(&def.bones).enumerate() {
                    if !chosen[i] || !layer.moves[i] {
                        continue;
                    }
                    // Each bone's curves are in its own part of the clip.
                    let (values, reference): (&[f32], &[f32]) = if self.always[i] {
                        (&samples[0], &def.clip.reference_pose)
                    } else {
                        match &def.rest {
                            Some(rest) => (&samples[1], &rest.reference_pose),
                            None => (&[], &[]),
                        }
                    };
                    let read = |values: &[f32]| {
                        let p = tracks.position.map_or(bone.position, |c| [values[c], values[c + 1], values[c + 2]]);
                        let q = match tracks.rotation {
                            Some((c, true)) => euler_to_quat([values[c], values[c + 1], values[c + 2]]),
                            Some((c, false)) => [values[c], values[c + 1], values[c + 2], values[c + 3]],
                            None => bone.rotation,
                        };
                        (p, q)
                    };
                    let (mut p, mut q) = read(values);
                    if let (true, Some(path)) = (self.root_bone == Some(i), def.root_path) {
                        // The part of the root's travel that leaves as root
                        // motion comes off the bone; the rest stays in the
                        // pose. (The root is always worked out: its curves
                        // are in the first part, with the bone's.)
                        for (k, curve) in path.iter().enumerate() {
                            if !stays_in_pose(&def.clip, k) {
                                p[k] -= values[*curve];
                            }
                        }
                    }
                    if let (true, Some(c)) = (self.root_bone == Some(i), def.root_turn) {
                        // So does the root's turning: the clip's bone turns
                        // with the character, and the character is turned
                        // by the root motion, so the pose is left facing
                        // the way the root now does.
                        let back = quat_normalize([-values[c[0]], -values[c[1]], -values[c[2]], values[c[3]]]);
                        p = quat_rotate(back, p);
                        q = quat_mul(back, q);
                    }
                    if layer.additive {
                        // What the clip adds is its difference from its
                        // own reference pose; a clip without one adds nothing.
                        if !def.referenced {
                            continue;
                        }
                        let (p0, q0) = read(reference);
                        // The clips' differences are blended with one
                        // another first, like the clips of any blend tree,
                        // and what comes of that is added to the pose once
                        // (`body-animator.md`, "Layers"; settled by the
                        // recordings: added one by one, each by its own
                        // weight, the ribcage was up to a centimetre out
                        // wherever the view was both pitched and turned
                        // from the body, and more the steeper the pitch).
                        let delta = quat_mul([-q0[0], -q0[1], -q0[2], q0[3]], q);
                        let side = if delta[3] < 0.0 { -1.0 } else { 1.0 };
                        for k in 0..3 {
                            position[i][k] += (p[k] - p0[k]) * share.weight;
                        }
                        for k in 0..4 {
                            rotation[i][k] += delta[k] * share.weight * side;
                        }
                        continue;
                    }
                    // Rotations are summed on the same side of the sphere
                    // and normalised afterwards.
                    let dot: f32 = (0..4).map(|k| rotation[i][k] * q[k]).sum();
                    let side = if dot < 0.0 { -1.0 } else { 1.0 };
                    for k in 0..3 {
                        position[i][k] += p[k] * share.weight;
                    }
                    for k in 0..4 {
                        rotation[i][k] += q[k] * share.weight * side;
                    }
                }
            }
            if layer.additive {
                // What the layer's clips add between them: their blended
                // difference, with no turn standing in for whatever share
                // of the layer no clip with a reference pose fills.
                let filled: f32 = mix.shares.iter().filter(|s| self.clips[s.clip].referenced).map(|s| s.weight).sum();
                let unfilled = (1.0 - filled).max(0.0);
                for i in 0..bone_count {
                    if !chosen[i] || !layer.moves[i] {
                        continue;
                    }
                    for k in 0..3 {
                        pose[i].0[k] += position[i][k] * layer_weight;
                    }
                    let mut delta = rotation[i];
                    delta[3] += unfilled;
                    pose[i].1 = quat_mul(pose[i].1, quat_lerp([0.0, 0.0, 0.0, 1.0], quat_normalize(delta), layer_weight));
                }
                // An additive layer's curves add to what the layers below
                // made of the parameter, by the layer's weight.
                if let Some(driven) = driven.as_deref_mut() {
                    for (parameter, (sum, covered)) in layer_curves.iter().enumerate() {
                        if *covered > 0.0 && *sum != 0.0 {
                            let below = driven.curves[parameter].unwrap_or(self.parameters[parameter].default);
                            driven.curves[parameter] = Some(below + sum * layer_weight);
                        }
                    }
                }
            } else {
                // An override layer replaces what is below it by its
                // weight, and only as far as its clips cover the frame.
                let amount = (layer_weight * total).clamp(0.0, 1.0);
                for i in 0..bone_count {
                    if !chosen[i] || !layer.moves[i] {
                        continue;
                    }
                    let p = position[i].map(|v| v / total);
                    let q = quat_normalize(rotation[i]);
                    if index == 0 {
                        pose[i] = (p, q);
                    } else {
                        for k in 0..3 {
                            pose[i].0[k] += (p[k] - pose[i].0[k]) * amount;
                        }
                        pose[i].1 = quat_lerp(pose[i].1, q, amount);
                    }
                    set[i] |= amount > 0.0;
                }
                // Curves take no notice of masks.
                if let Some(driven) = driven.as_deref_mut() {
                    for (parameter, (sum, covered)) in layer_curves.iter().enumerate() {
                        if *covered <= 0.0 {
                            continue;
                        }
                        let value = sum / covered;
                        driven.curves[parameter] = Some(match driven.curves[parameter] {
                            Some(below) if index > 0 => below + (value - below) * (layer_weight * covered).clamp(0.0, 1.0),
                            // On the first layer a clip without the curve
                            // stands for the parameter's default.
                            _ => sum + (total - covered).max(0.0) * self.parameters[parameter].default,
                        });
                    }
                }
            }
        }
    }
}
