//! What the dummies answer: hearing, rays, takes and blows, and their colliders.

use super::*;

impl Dummies {
    /// Every dummy's health's events taken (`Dummy::drain`): what its voice
    /// is to hear, all of it by now.
    pub fn hear_all(&mut self) {
        let (listened, drops) = (self.listened, &mut self.bleeding_drops);
        for dummy in self.list.iter_mut().filter(|dummy| dummy.dressed) {
            dummy.drain(listened, drops);
        }
    }

    /// Each dummy's weapon's model, its entities by the weapon's nodes
    /// (`DummySource::maker`'s weapon), where it has one, and whether it is
    /// shown: not once its dead body's weapon is gone (`clear_dummy_weapons`),
    /// when its devices light nothing.
    pub fn weapon_trees(&self) -> impl Iterator<Item = (&[Entity], bool)> + '_ {
        self.list.iter().filter_map(|dummy| dummy.model.as_ref().map(|model| (model.tree.as_slice(), dummy.weapon_shown())))
    }

    /// What a dummy's body leaves out of its slots' views (`Dummy::
    /// in_hand`), by its number.
    pub fn in_hand(&self, which: usize) -> Option<trp_sim::items::inventory::Id> {
        self.list.iter().find(|dummy| dummy.which == which).and_then(Dummy::in_hand)
    }

    /// A character's feet (the game's axes), while it stands.
    pub fn feet(&self, character: u32) -> Option<[f32; 3]> {
        let dummy = self.list.iter().find(|d| d.character() == character && d.fall.is_none())?;
        Some(dummy.puppet.as_ref()?.movement.position)
    }

    /// Each living dummy's sounds of moving: its number, where its feet are
    /// (the game's axes) and its rules, whose asks are to be played and
    /// taken (`step_sound::play_dummies`).
    pub fn movers(&mut self) -> impl Iterator<Item = (usize, [f32; 3], &mut trp_sim::sound::steps::StepSounds)> + '_ {
        self.list.iter_mut().filter(|dummy| dummy.health.alive && dummy.fall.is_none()).filter_map(|dummy| {
            let puppet = dummy.puppet.as_mut()?;
            Some((dummy.which, puppet.movement.position, &mut puppet.movement.sounds))
        })
    }

    /// Every collider a ray from `from` to `to` (the game's axes) enters
    /// through a face turned toward it, one hit each: a plate's only while
    /// armour covers it. A standing dummy's on the frame its colliders are
    /// of (`place_dummies`), placed where the ray meets its bound (the
    /// economy on; every one off).
    pub fn ray(&self, from: V3, to: V3, hits: &mut Vec<RayHit>) {
        self.ray_by(from, to, self.economy, hits);
    }

    /// `ray`, the bound asked first where `bound` says (the app's tests
    /// compare the two).
    pub(super) fn ray_by(&self, from: V3, to: V3, bound: bool, hits: &mut Vec<RayHit>) {
        let o = Vec3::new(from[0], from[1], -from[2]);
        let e = Vec3::new(to[0], to[1], -to[2]);
        let length = (e - o).length();
        if length <= 0.0 {
            return;
        }
        let d = (e - o) / length;
        if std::env::var_os("TRP_DUMMY_DEBUG").is_some() && (o.z < -15.0 || e.z < -15.0) {
            info!("a ray from {o:.2?} to {e:.2?}: {} dummies, {} colliders each", self.list.len(), self.list.first().map_or(0, |d| d.colliders.len()));
        }
        for dummy in &self.list {
            if bound && dummy.colliders_frame.is_some_and(|frame| !dummy.bound_meets(frame, from, to, 0.0)) {
                continue;
            }
            let colliders = dummy.colliders_on(dummy.colliders_frame);
            let plates = dummy.wearer.plates_on();
            for (index, (c, placed)) in dummy.colliders.iter().zip(colliders.placed()).enumerate() {
                if c.body.plate != 0 && plates & c.body.plate == 0 {
                    continue;
                }
                let Some((t, normal)) = enter(*placed, o, d, length) else { continue };
                let p = o + d * t;
                let id = dummy.base() + index as u64;
                hits.push(RayHit {
                    distance: t,
                    point: [p.x, p.y, -p.z],
                    normal: [normal.x, normal.y, -normal.z],
                    collider: id,
                    thing: id,
                    mesh: false,
                    surface: Surface { material: material::BODY, figures: c.figures },
                    body: Some(c.body),
                });
            }
        }
    }

    /// The dead dummy whose body the look's line meets first, and how far
    /// along (Bevy's axes): the game's look finds a corpse by its colliders
    /// on the dead bodies' layer (`GameWorld.FindInteractable`, `Corpse`),
    /// here the hit colliders on its bones, which lie as its ragdoll lies.
    pub fn body_looked_at(&self, from: Vec3, to: Vec3) -> Option<(usize, f32)> {
        let length = (to - from).length();
        if length <= 0.0 {
            return None;
        }
        let d = (to - from) / length;
        let mut nearest: Option<(usize, f32)> = None;
        for dummy in self.list.iter().filter(|dummy| !dummy.health.alive && dummy.dressed) {
            for placed in dummy.colliders_on(dummy.colliders_frame).placed() {
                let Some((t, _)) = enter(*placed, from, d, length) else { continue };
                if nearest.is_none_or(|(_, best)| t < best) {
                    nearest = Some((dummy.which, t));
                }
            }
        }
        nearest
    }

    /// The living dummies' feet (the game's axes) within `reach` of `at`, by
    /// character: those a blast finds (`Explosion`'s overlap).
    pub fn within(&self, at: [f32; 3], reach: f32) -> Vec<(u32, [f32; 3])> {
        self.list
            .iter()
            .filter(|dummy| dummy.dressed && dummy.health.alive && dummy.fall.is_none())
            .filter_map(|dummy| Some((dummy.character(), dummy.puppet.as_ref()?.movement.position)))
            .filter(|(_, feet)| (0..3).map(|k| (feet[k] - at[k]).powi(2)).sum::<f32>().sqrt() <= reach)
            .collect()
    }

    /// The armour of the dummy that is that character.
    pub fn wearer(&self, character: u32) -> Option<&Wearer> {
        self.list.iter().find(|d| d.character() == character).map(|d| &d.wearer)
    }

    /// A hit handed on by the ballistics (`Player.ApplyShot`): through the
    /// dummy's armour into its health, its material kept for its effect,
    /// and the killing hit kept for the push.
    pub fn take(&mut self, hit: &Hit) {
        let Some(body) = hit.body else { return };
        let Some(dummy) = self.list.iter_mut().find(|d| d.character() == body.character) else { return };
        let alive = dummy.health.alive;
        // What its health did before, then the part this hit reached, then
        // what the hit did: in the game's order, for its voice.
        dummy.drain(self.listened, &mut self.bleeding_drops);
        let Some(outcome) = armor::apply_shot(Some(&mut dummy.wearer), &mut dummy.health, hit) else { return };
        // Its armour's wear for its things (`keep_dummy_wear`).
        dummy.worn = true;
        if alive && self.listened {
            dummy.heard.push(Heard::Struck(outcome.part, outcome.taken.damage, trp_sim::condition::health::DamageKind::Bullet));
        }
        dummy.drain(self.listened, &mut self.bleeding_drops);
        // Then its movement, as any character's (`ApplyHitDebuff`: a shot's
        // hit is a bullet's, a blast's fragment's too, `GameWorld.
        // ShotDelegate`): the stamina it burns, its rate as the armour left
        // it times the damage the armour left.
        let burn = outcome.taken.stamina_burn * outcome.taken.damage;
        if dummy.fall.is_none() {
            if let Some(puppet) = dummy.puppet.as_mut() {
                puppet.movement.hit(burn, outcome.part, trp_sim::condition::health::DamageKind::Bullet);
            }
        }
        let which = dummy.which;
        self.marks.extend(dummy.wearer.drain().into_iter().map(|mark| (which, mark)));
        dummy.taken.push(((hit.shot, hit.fragment, hit.seed), outcome.taken.material));
        let index = hit.collider.checked_sub(dummy.base()).map(|i| i as usize);
        let object = index.and_then(|i| dummy.colliders.get(i)).map_or("?", |c| c.data.object.as_str());
        let parts: Vec<String> = Part::ALL.iter().map(|p| format!("{:.1}", dummy.health.part(*p).current)).collect();
        dummy.hits += 1;
        info!(
            "dummy {}'s hit {}: {} on {} ({}{}), damage {:.2} came, {:.2} through the armour ({:.2} taken by it, {}{}), penetration {:.2}, stamina burnt {:.2}; health [{}]{}",
            dummy.which,
            dummy.hits,
            outcome.part.name(),
            object,
            armor::COLLIDER_TYPES.get(body.collider as usize).copied().unwrap_or("?"),
            if body.plate != 0 { ", a plate" } else { "" },
            outcome.damage_in,
            outcome.taken.damage,
            outcome.taken.absorbed,
            material::NAMES.get(outcome.taken.material as usize).copied().unwrap_or("?"),
            match (hit.blocked_by.is_some(), hit.deflected_by.is_some()) {
                (_, true) => ", deflected",
                (true, _) => ", blocked",
                _ => "",
            },
            outcome.taken.penetration,
            burn,
            parts.join(", "),
            if dummy.health.alive { "" } else { ": dead" }
        );
        if alive && !dummy.health.alive {
            if let Some(collider) = index {
                dummy.killing = Some(Killing { collider, direction: hit.direction, point: hit.point, penetration: outcome.taken.penetration });
            }
        }
        if alive {
            let part = outcome.part;
            let bleeding = [trp_sim::condition::health::Kind::LightBleeding, trp_sim::condition::health::Kind::HeavyBleeding].into_iter().any(|kind| dummy.health.active(kind, Some(part)).is_some());
            let heavy = trp_sim::condition::experience::heavy_hit(outcome.taken.damage, dummy.health.part(part).maximum, bleeding);
            let whole = Part::ALL.iter().map(|p| dummy.health.part(*p).maximum).sum();
            self.experience.push(DummyHit { killed: !dummy.health.alive, head: part == Part::Head, damage: outcome.taken.damage, heavy, whole, bullet: true, melee: false });
        }
    }

    /// The material a bullet's hit on a dummy was, where one took it.
    pub fn taken_by(&self, hit: &Hit) -> Option<i32> {
        self.list.iter().flat_map(|d| &d.taken).find(|(bullet, _)| *bullet == (hit.shot, hit.fragment, hit.seed)).map(|(_, m)| *m)
    }

    /// Every living, standing dummy's hit colliders as a blade meets them
    /// (`knife::StrokeWorld::set_targets`), where `place_dummies` last put
    /// them (its character's newest frame then: the frame before's, as the
    /// knife's sweep runs before the dummies step): a plate's only while
    /// armour covers it, as a bullet's. Each in the game's axes, its place
    /// and turn and its shape there. A dead body's are on the dead bodies'
    /// layer (`Deadbody`), which a blade does not meet: none. Every living
    /// dummy's, whatever its bound (the posing economy: the
    /// engine's touches come in an order that goes by every body in its
    /// world, so one left out, however far, can change which collider a
    /// sweep meets first, which the app's test found; each is placed now
    /// where it was not, on the frames a stroke is live).
    pub fn blade_targets(&self) -> Vec<(crate::weapons::knife::Target, V3, [f32; 4], PhysicsCollider)> {
        let mut out = Vec::new();
        for dummy in self.list.iter().filter(|dummy| dummy.dressed && dummy.health.alive && dummy.fall.is_none()) {
            let colliders = dummy.colliders_on(dummy.colliders_frame);
            let plates = dummy.wearer.plates_on();
            for (index, ((c, placed), object)) in dummy.colliders.iter().zip(colliders.placed()).zip(colliders.objects()).enumerate() {
                if c.body.plate != 0 && plates & c.body.plate == 0 {
                    continue;
                }
                let (at, turn, shape) = blade_shape(*placed);
                let collider = PhysicsCollider { shape, at: [0.0; 3], turn: [0.0, 0.0, 0.0, 1.0], material: Material::default() };
                out.push((crate::weapons::knife::Target { which: dummy.which, index, object: *object }, at, turn, collider));
            }
        }
        out
    }

    /// A blow handed on by a knife's stroke (`BaseKnifeController.
    /// ProcessHit` → `GameWorld.HackShot` → `Player.ApplyShot`): through the
    /// dummy's armour into its health as a `Melee` hit (`armor::apply_blow`),
    /// then, where it took health, the knife's poison (`TryApplySideEffects`,
    /// `side`: the knife's template and its doses, one spent), its voice
    /// told, its movement the hit's (`ApplyHitDebuff`, the knife's burn rate
    /// times the damage left), its armour's wear kept, the player's
    /// experience of a hit by the melee weapon, the killing blow kept for
    /// the push (`ApplyCorpseImpulse`). Returns what it did; none where the
    /// dummy was not alive.
    pub fn take_blow(&mut self, blow: &crate::weapons::knife::Blow, side: Option<(&str, &mut trp_sim::condition::health::SideEffect)>) -> Option<armor::ShotOutcome> {
        let dummy = self.list.iter_mut().find(|d| d.which == blow.target.which)?;
        let alive = dummy.health.alive;
        let collider = dummy.colliders.get(blow.target.index)?;
        let (body, object) = (collider.body, collider.data.object.clone());
        dummy.drain(self.listened, &mut self.bleeding_drops);
        let outcome = armor::apply_blow(Some(&mut dummy.wearer), &mut dummy.health, body, blow.damage, blow.penetration, blow.burn_rate)?;
        dummy.worn = true;
        if alive && self.listened {
            dummy.heard.push(Heard::Struck(outcome.part, outcome.taken.damage, trp_sim::condition::health::DamageKind::Melee));
        }
        // Its poison, where the blow took health (`Player.ApplyDamageInfo`).
        let poisoned = side.is_some_and(|(template, side)| outcome.did_body > 0.0 && dummy.health.try_apply_side_effects(outcome.did_body, template, side));
        dummy.drain(self.listened, &mut self.bleeding_drops);
        let burn = outcome.taken.stamina_burn * outcome.taken.damage;
        if dummy.fall.is_none() {
            if let Some(puppet) = dummy.puppet.as_mut() {
                puppet.movement.hit(burn, outcome.part, trp_sim::condition::health::DamageKind::Melee);
            }
        }
        let which = dummy.which;
        self.marks.extend(dummy.wearer.drain().into_iter().map(|mark| (which, mark)));
        let parts: Vec<String> = Part::ALL.iter().map(|p| format!("{:.1}", dummy.health.part(*p).current)).collect();
        dummy.hits += 1;
        info!(
            "dummy {}'s hit {}: a {:?} on {} {} ({}{}), damage {:.2} came, {:.2} through the armour ({:.2} taken by it, {}), penetration {:.2}, stamina burnt {:.2}{}; health [{}]{}",
            dummy.which,
            dummy.hits,
            blow.stroke,
            outcome.part.name(),
            object,
            armor::COLLIDER_TYPES.get(body.collider as usize).copied().unwrap_or("?"),
            if body.plate != 0 { ", a plate" } else { "" },
            outcome.damage_in,
            outcome.taken.damage,
            outcome.taken.absorbed,
            material::NAMES.get(outcome.taken.material as usize).copied().unwrap_or("?"),
            outcome.taken.penetration,
            burn,
            if poisoned { ", poisoned" } else { "" },
            parts.join(", "),
            if dummy.health.alive { "" } else { ": dead" }
        );
        if alive && !dummy.health.alive {
            dummy.killing = Some(Killing { collider: blow.target.index, direction: blow.direction, point: blow.point, penetration: outcome.taken.penetration });
        }
        if alive {
            let part = outcome.part;
            let bleeding = [trp_sim::condition::health::Kind::LightBleeding, trp_sim::condition::health::Kind::HeavyBleeding].into_iter().any(|kind| dummy.health.active(kind, Some(part)).is_some());
            let heavy = trp_sim::condition::experience::heavy_hit(outcome.taken.damage, dummy.health.part(part).maximum, bleeding);
            let whole = Part::ALL.iter().map(|p| dummy.health.part(*p).maximum).sum();
            self.experience.push(DummyHit { killed: !dummy.health.alive, head: part == Part::Head, damage: outcome.taken.damage, heavy, whole, bullet: true, melee: true });
        }
        Some(outcome)
    }
}

/// A placed collider (Bevy's axes) as the engine takes a shape: its place
/// and turn in the game's axes (z turned over), and the shape about them,
/// a capsule along its own x.
pub(super) fn blade_shape(placed: Placed) -> (V3, [f32; 4], PhysicsShape) {
    let back = |v: Vec3| [v.x, v.y, -v.z];
    // A right-handed turn of the game's axes from two of its own: the
    // third their cross product.
    let turn_of = |x: Vec3, y: Vec3| {
        let (x, y) = (Vec3::from(back(x)).normalize_or_zero(), Vec3::from(back(y)).normalize_or_zero());
        let z = x.cross(y);
        trp_physics::turn_of_axes(x.to_array(), y.to_array(), z.to_array())
    };
    match placed {
        Placed::Sphere { centre, radius } => (back(centre), [0.0, 0.0, 0.0, 1.0], PhysicsShape::Sphere { radius }),
        Placed::Box { centre, axes, half } => (back(centre), turn_of(axes[0], axes[1]), PhysicsShape::Box { half: half.to_array() }),
        Placed::Capsule { a, b, radius } => {
            let along = b - a;
            let x = along.normalize_or(Vec3::X);
            let y = x.any_orthonormal_vector();
            (back((a + b) / 2.0), turn_of(x, y), PhysicsShape::Capsule { radius, height: along.length() + 2.0 * radius, direction: 0 })
        }
    }
}

/// Where a ray from `o` along the unit `d`, within `length`, first enters
/// a shape from outside (nothing it starts inside), and the face's normal.
pub(super) fn enter(shape: Placed, o: Vec3, d: Vec3, length: f32) -> Option<(f32, Vec3)> {
    let found = match shape {
        Placed::Sphere { centre, radius } => sphere(o, d, centre, radius),
        Placed::Box { centre, axes, half } => {
            let p = o - centre;
            let (lo, ld) = (Vec3::new(p.dot(axes[0]), p.dot(axes[1]), p.dot(axes[2])), Vec3::new(d.dot(axes[0]), d.dot(axes[1]), d.dot(axes[2])));
            if lo.abs().cmplt(half).all() {
                return None;
            }
            let (mut near, mut far, mut face) = (f32::MIN, f32::MAX, (0usize, 0.0f32));
            for k in 0..3 {
                if ld[k].abs() < 1e-9 {
                    if lo[k].abs() > half[k] {
                        return None;
                    }
                    continue;
                }
                let (t1, t2) = ((-half[k] - lo[k]) / ld[k], (half[k] - lo[k]) / ld[k]);
                let (tn, tf, sign) = if t1 < t2 { (t1, t2, -1.0) } else { (t2, t1, 1.0) };
                if tn > near {
                    near = tn;
                    face = (k, sign);
                }
                far = far.min(tf);
            }
            (near <= far && near >= 0.0).then(|| (near, axes[face.0] * face.1))
        }
        Placed::Capsule { a, b, radius } => {
            let ab = b - a;
            let along = ab.length_squared();
            let closest = |p: Vec3| if along > 0.0 { a + ab * ((p - a).dot(ab) / along).clamp(0.0, 1.0) } else { a };
            if (o - closest(o)).length_squared() < radius * radius {
                return None;
            }
            // The first of the body and the two caps the ray meets.
            let mut best: Option<f32> = None;
            if along > 0.0 {
                let (oa, bard) = (o - a, ab.dot(d));
                let (qa, qb, qc) = (along - bard * bard, along * d.dot(oa) - ab.dot(oa) * bard, along * oa.dot(oa) - ab.dot(oa).powi(2) - radius * radius * along);
                let h = qb * qb - qa * qc;
                if qa > 1e-12 && h >= 0.0 {
                    let t = (-qb - h.sqrt()) / qa;
                    let y = ab.dot(oa) + t * bard;
                    if t >= 0.0 && y > 0.0 && y < along {
                        best = Some(t);
                    }
                }
            }
            for end in [a, b] {
                if let Some((t, _)) = sphere(o, d, end, radius) {
                    best = Some(best.map_or(t, |b: f32| b.min(t)));
                }
            }
            best.map(|t| {
                let p = o + d * t;
                (t, (p - closest(p)).normalize_or_zero())
            })
        }
    };
    found.filter(|(t, _)| *t <= length)
}

/// A ray's entry into a sphere from outside.
fn sphere(o: Vec3, d: Vec3, centre: Vec3, radius: f32) -> Option<(f32, Vec3)> {
    let oc = o - centre;
    let c = oc.length_squared() - radius * radius;
    if c < 0.0 {
        return None;
    }
    let b = oc.dot(d);
    let h = b * b - c;
    if h < 0.0 {
        return None;
    }
    let t = -b - h.sqrt();
    (t >= 0.0).then(|| (t, (o + d * t - centre) / radius))
}

/// A collider placed by its bone's place in the world.
pub(super) fn placed(c: &Collider, bone: Affine3A) -> Placed {
    let m = bone * c.local;
    let axis = |k: usize| Vec3::from(m.matrix3.col(k));
    let scale = Vec3::new(axis(0).length(), axis(1).length(), axis(2).length());
    match c.data.shape {
        Shape::Sphere { centre, radius } => Placed::Sphere { centre: m.transform_point3(convert::position(centre)), radius: radius * scale.max_element() },
        Shape::Box { centre, size } => Placed::Box {
            centre: m.transform_point3(convert::position(centre)),
            axes: [axis(0).normalize_or_zero(), axis(1).normalize_or_zero(), axis(2).normalize_or_zero()],
            half: Vec3::from(size) * scale / 2.0,
        },
        Shape::Capsule { centre, radius, height, direction } => {
            let k = (direction as usize).min(2);
            let across = (0..3).filter(|j| *j != k).map(|j| scale[j]).fold(0.0f32, f32::max);
            let radius = radius * across;
            let half = (height * scale[k] / 2.0 - radius).max(0.0);
            let centre = m.transform_point3(convert::position(centre));
            let way = axis(k).normalize_or_zero();
            Placed::Capsule { a: centre - way * half, b: centre + way * half, radius }
        }
    }
}

/// A dummy's hit colliders on its bones, with the figures the game sets.
pub(super) fn colliders_of(source: &DummySource, rig: &DummyRig, which: usize) -> Vec<Collider> {
    let mut colliders = Vec::new();
    for c in &source.colliders {
        let Some(bone) = rig.bone(&c.bone) else {
            warn!("dummy {which} has no bone {} for the collider {}", c.bone, c.object);
            continue;
        };
        let local = c.local.map_or(Affine3A::IDENTITY, |(place, turn)| convert::transform(place, turn, [1.0; 3]).compute_affine());
        // A body part's level, chance and share as the character is set
        // up (`InitColliderSettings`); a plate's the prefab's.
        let mut figures = c.figures;
        let mut damage_mod = c.damage_mod;
        if c.plate == 0 {
            if let Some([level, chance, share]) = source.rules.colliders.get(c.collider_type as usize).copied().flatten() {
                figures[0] = level;
                figures[1] = chance;
                damage_mod = share;
            }
        }
        let body = BodyRef { character: which as u32 + 1, part: c.part, collider: c.collider_type, plate: c.plate, damage_mod };
        colliders.push(Collider { data: c.clone(), bone, local, body, figures });
    }
    colliders
}
