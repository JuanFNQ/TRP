//! Fragments thrown from a bullet's hit.

use super::*;

impl Ballistics {
    /// What follows a hit (`CreateFragments`): a ricochet, fragments, the
    /// bullet turned on, or nothing.
    pub(super) fn fragments(&mut self, index: usize, hit: &RayHit, forward: bool, cos: f32, way: V3, world: &impl BulletWorld) -> Vec<Bullet> {
        let draw = self.tables.float(self.bullets[index].seed);
        let back = angle(scale(way, -1.0), hit.normal);
        let body = hit.body;
        let wearer = body.and_then(|r| world.wearer(r.character));
        // Deflected by heavy armour (`IsShotDeflectedByHeavyArmor`).
        let heavy = body.is_some_and(|r| {
            self.bullets[index].ricochets < 2 && back > 42.5 && back < 80.0 && wearer.is_some_and(|w| w.heavy_deflects(r.collider, r.plate, draw))
        });
        // A ricochet (`IsRicocheted`): at a character's collider its armour
        // deflects the bullet, and may block it on the way (`SetShotStatus`).
        let ricochet = heavy
            || (forward
                && match body {
                    Some(r) => {
                        let b = &self.bullets[index];
                        let (mut blocked, mut deflected) = (b.blocked_by, None);
                        if let Some(w) = wearer {
                            deflected = w.shot_status(r.collider, r.plate, back, b.penetration, draw, &mut blocked);
                        }
                        let b = &mut self.bullets[index];
                        b.blocked_by = blocked;
                        if deflected.is_some() {
                            b.deflected_by = deflected;
                        }
                        deflected.is_some()
                    }
                    None => {
                        let (b, s) = (&self.bullets[index], hit.surface);
                        s.ricochet_chance() >= EPSILON && back > 42.5 && back < 80.0 && b.ricochet_chance > 0.0 && (s.ricochet_chance() + b.ricochet_chance) * (1.0 - cos.abs()) > draw
                    }
                }
                && self.bullets[index].ricochets < 2);
        let tables = &self.tables;
        let b = &self.bullets[index];
        let s = hit.surface;
        if ricochet {
            let sum = (b.ricochet_chance + s.ricochet_chance()).min(1.0);
            let mirrored = sub(normalized(b.velocity), scale(hit.normal, 2.0 * cos));
            let turned = if heavy { [0.0; 3] } else { scale(tables.direction(b.seed), s.deviation()) };
            let m = Made {
                fragment: b.fragment + 1,
                seed: fast::next_seed(b.seed),
                origin: hit.point,
                direction: normalized(add(mirrored, turned)),
                initial_speed: b.initial_speed,
                speed: magnitude(b.velocity) * 0.7,
                damage: b.damage * sum * 0.5,
                penetration: b.penetration * sum * 0.5,
                penetration_chance: b.penetration_chance * 0.2,
                ricochet_chance: b.ricochet_chance * 0.2,
                fragmentation_chance: b.fragmentation_chance * 0.2,
                deviation_chance: 1.0,
                min_fragments: b.min_fragments,
                max_fragments: b.max_fragments,
            };
            self.bullets[index].state = State::RicochetHit;
            let b = &self.bullets[index];
            return vec![self.made(b.shot, &b.round, m, Some(b))];
        }
        // Through (`IsTargetPenetrated`): a character's collider always from
        // behind; from before when not blocked and the penetration, worn by
        // the armour over it, is above its level.
        let through = match body {
            Some(_) if !forward => true,
            Some(r) => b.blocked_by.is_none() && b.penetration * wearer.and_then(|w| w.factor(r.collider, r.plate, b.penetration)).unwrap_or(1.0) > s.level(),
            None => s.penetration_chance() >= EPSILON && b.penetration > s.level() && b.penetration_chance + s.penetration_chance() > draw,
        };
        if !through {
            self.bullets[index].state = State::StopHit;
            return Vec::new();
        }
        // A character's collider's share of what goes on, and the armour's
        // wear on each bullet that follows (`ApplyPenetratedDegradation`).
        let share = body.map_or(1.0, |r| ((b.penetration - s.level()) / 100.0 + r.damage_mod + b.round.penetration_damage_mod).clamp(0.0, 1.0));
        let worn = |m: &mut Made| {
            if let (true, Some(r), Some(w)) = (forward, body, wearer) {
                if let Some(factor) = w.factor(r.collider, r.plate, m.penetration) {
                    m.damage *= factor;
                    m.penetration *= factor;
                }
            }
        };
        let in_pieces = forward && b.fragmentation_chance + s.fragmentation_chance() > draw && s.fragmentation_chance() >= EPSILON && b.fragmentation_chance >= EPSILON;
        if in_pieces {
            self.bullets[index].state = State::FragmentationHit;
            let b = &self.bullets[index];
            let count = fast::int(b.seed, b.min_fragments, b.max_fragments - b.min_fragments);
            if count <= 0 {
                return Vec::new();
            }
            let most = b.max_fragments as f32;
            let damage = b.damage * share * 0.5 / most;
            let penetration = b.penetration * share * 0.5 / most;
            let origin = add(hit.point, scale(b.direction, 0.002));
            let along = normalized(b.velocity);
            let speed = magnitude(b.velocity) * (1.0f32).min(1.0 / most * 0.7);
            let mut pick = fast::next_seed(b.seed);
            let mut out = Vec::with_capacity(count as usize);
            for _ in 0..count {
                let direction = normalized(add(along, scale(tables.direction(pick), s.deviation())));
                pick = fast::next_seed(pick);
                let mut m = Made {
                    fragment: b.fragment + 1,
                    seed: pick,
                    origin,
                    direction,
                    initial_speed: b.initial_speed,
                    speed,
                    damage,
                    penetration,
                    penetration_chance: b.penetration_chance * 0.2,
                    ricochet_chance: b.ricochet_chance * 0.2,
                    fragmentation_chance: 0.0,
                    deviation_chance: 1.0,
                    min_fragments: 0,
                    max_fragments: 0,
                };
                worn(&mut m);
                out.push(self.made(b.shot, &b.round, m, Some(b)));
            }
            return out;
        }
        if b.deviation_chance < EPSILON || (s.material == material::CHAINFENCE && b.round.explosive()) {
            // Through, and nothing follows: it stays flying.
            return Vec::new();
        }
        let mut turned = normalized(b.velocity);
        if forward && s.deviation_chance() > draw {
            turned = add(turned, scale(tables.direction(b.seed), s.deviation()));
        }
        // From before at a character's collider: its share of the damage and
        // penetration, 0.8 to 1 of the speed and 0.2 to 1 of the chances by
        // its penetration chance.
        let (share, keep_speed, keep_chances) = if forward && body.is_some() {
            let p = s.penetration_chance();
            (share, 0.8 + 0.2 * p, 0.2 + 0.8 * p)
        } else {
            (1.0, 1.0, 1.0)
        };
        let mut m = Made {
            fragment: b.fragment,
            seed: fast::next_seed(b.seed),
            origin: add(hit.point, scale(turned, 0.002)),
            direction: turned,
            initial_speed: b.initial_speed,
            speed: magnitude(b.velocity) * keep_speed,
            damage: b.damage * share,
            penetration: b.penetration * share,
            penetration_chance: b.penetration_chance * keep_chances,
            ricochet_chance: b.ricochet_chance * keep_chances,
            fragmentation_chance: b.fragmentation_chance * keep_chances,
            deviation_chance: b.deviation_chance - 0.08,
            min_fragments: b.min_fragments,
            max_fragments: b.max_fragments,
        };
        worn(&mut m);
        self.bullets[index].state = State::DeviationHit;
        let b = &self.bullets[index];
        vec![self.made(b.shot, &b.round, m, Some(b))]
    }
}
