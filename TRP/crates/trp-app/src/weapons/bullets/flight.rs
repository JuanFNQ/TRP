//! The range as the bullets' world, and bullets flown each frame.

use super::*;

/// The feet a blast at `at` aims its fragments at (`Explosion`'s overlap):
/// the character's first where alive and within its reach, then each living
/// dummy's (`PROVISIONAL`: the game's order is its overlap's).
fn blast_feet(at: V3, reach: f32, character: Option<V3>, dummies: Option<&crate::world::dummy::Dummies>) -> Vec<V3> {
    let far = |feet: V3| (0..3).map(|k| (feet[k] - at[k]).powi(2)).sum::<f32>().sqrt();
    let mut feet: Vec<V3> = character.filter(|&feet| far(feet) <= reach).into_iter().collect();
    if let Some(dummies) = dummies {
        feet.extend(dummies.within(at, reach).into_iter().map(|(_, feet)| feet));
    }
    feet
}

/// The range as the bullets' world, and the dummy on it (`dummy`).
pub(super) struct RangeWorld<'a> {
    pub(super) range: &'a Range,
    pub(super) presets: &'a [trp_data::setup::hard_settings::BallisticPreset],
    dummy: Option<&'a mut crate::world::dummy::Dummies>,
    poppers: Option<&'a crate::world::poppers::Poppers>,
}

impl BulletWorld for RangeWorld<'_> {
    fn ray(&self, _: Asker, from: V3, to: V3, hits: &mut Vec<RayHit>) {
        self.range.bullet_hits(from, to, self.presets, hits);
        if let Some(dummy) = &self.dummy {
            dummy.ray(from, to, hits);
        }
        if let Some(poppers) = self.poppers {
            poppers.ray(from, to, hits);
        }
    }

    fn wearer(&self, character: u32) -> Option<&trp_sim::condition::armor::Wearer> {
        self.dummy.as_ref().and_then(|dummies| dummies.wearer(character))
    }

    fn delivered(&mut self, hit: &Hit) {
        if let Some(dummy) = self.dummy.as_mut() {
            dummy.take(hit);
        }
    }
}

/// Every part of a weapon's build in the game's order (`Weapon.Mods`, its
/// `AllSlots` flattened: the contents of every slot of the item itself,
/// then each of those parts' own in the same way, a part's before the
/// next's), but the rounds in its chambers. The first stock is the one
/// whose double action multiplier counts.
fn parts(item: &trp_sim::items::inventory::Item, out: &mut Vec<String>) {
    let held: Vec<&trp_sim::items::inventory::Item> =
        item.slots.iter().zip(&item.kind.slots).filter(|(_, kind)| !kind.name.starts_with("patron_in_weapon")).filter_map(|(slot, _)| slot.as_ref()).collect();
    out.extend(held.iter().map(|part| part.kind.template.clone()));
    for part in held {
        parts(part, out);
    }
}

/// A transform's place as drawn and its turn as Unity keeps it (the product
/// of the turns above it, whatever the scales), in Unity's axes.
fn unity_pose(entity: Entity, helper: &TransformHelper, transforms: &Query<(&Transform, Option<&ChildOf>)>) -> Option<(V3, [f32; 4])> {
    let at = helper.compute_global_transform(entity).ok()?.translation();
    let mut turn = Quat::IDENTITY;
    let mut next = Some(entity);
    while let Some(e) = next {
        let (transform, parent) = transforms.get(e).ok()?;
        turn = transform.rotation * turn;
        next = parent.map(ChildOf::parent);
    }
    Some(([at.x, at.y, -at.z], convert::rotation_back(turn)))
}

/// The frame's bullets: the shots' made, every bullet moved on.
#[allow(clippy::too_many_arguments)]
pub fn fly_bullets(
    mut bullets: ResMut<Bullets>,
    mut controller: ResMut<Controller>,
    hands: Option<Res<HandsRig>>,
    carried: Res<Carried>,
    held: Res<HandsHeld>,
    game: Res<crate::GameData>,
    weapon: Option<Res<WeaponRig>>,
    range: Res<Range>,
    time: Res<Time>,
    helper: TransformHelper,
    transforms: Query<(&Transform, Option<&ChildOf>)>,
    cameras: Query<(&Camera, &GlobalTransform), With<crate::MainCamera>>,
    mut dummy: Option<ResMut<crate::world::dummy::Dummies>>,
    poppers: Option<Res<crate::world::poppers::Poppers>>,
    (mut blasts, mut seen): (ResMut<bevy::ecs::message::Messages<crate::world::loose::Blasted>>, Local<bevy::ecs::message::MessageCursor<crate::world::loose::Blasted>>),
) {
    let bullets = &mut *bullets;
    let alive = controller.health.as_ref().is_none_or(|health| health.alive);
    // Each thrown grenade's blast's fragments, each character where the
    // blast finds it (the character before its move of the frame, a dummy
    // after: the game's player and bots so). A round's are made below (read
    // here and written there through the one store).
    let thrown: Vec<crate::world::loose::Blasted> = seen.read(&blasts).filter(|blasted| blasted.blasts && !blasted.shot).cloned().collect();
    for blasted in &thrown {
        let Some(grenade) = crate::effects::blast_view::grenade_of(&game.db, &blasted.template).filter(|g| g.fragments > 0) else { continue };
        let at = blasted.blast_point();
        let feet = blast_feet(at, grenade.reach, alive.then_some(controller.feet_before_move), dummy.as_deref());
        bullets.blast_fragments(&game.db, &grenade, at, &feet, &range);
    }
    // A new frame for the hits' places (the commutator's late update).
    bullets.commutator.new_frame();
    let calls = std::mem::take(&mut controller.bullet_calls);
    let gravity = game.engine.gravity;
    // The weapon in hand's figures, read again when it or its build changes.
    let item = held.item.and_then(|id| carried.find(id));
    let build = item.map(|item| {
        let mut list = Vec::new();
        parts(item, &mut list);
        (item.id, list)
    });
    if bullets.figures.as_ref().map(|(key, _)| key) != build.as_ref() {
        bullets.figures = item.zip(build).map(|(item, (id, list))| {
            let refs: Vec<&str> = list.iter().map(String::as_str).collect();
            let figures = WeaponFigures::of(&game.db, &item.kind.template, &refs);
            ((id, list), figures)
        });
    }
    let figures = bullets.figures.as_ref().and_then(|(_, f)| f.clone());
    // The shot's way from the fireport: worked out for the iron sights as
    // they come up, down again as they go (`method_14`, `method_12`).
    let aiming = hands.as_ref().is_some_and(|h| h.aiming);
    if aiming && !bullets.was_aiming {
        let range_of = figures.as_ref().map_or(0, |f| f.iron_sight_range);
        let point = controller.rig_pose().sight_points.and_then(|points| trp_sim::base::rig::iron_point(points, range_of));
        let default = figures.as_ref().and_then(|f| f.default_round.clone()).and_then(|t| bullets.round(&game.db, &t));
        bullets.local_way = match (point, default, figures.as_ref()) {
            (Some(point), Some(round), Some(f)) => iron_shot_direction(point, &round, f.speed_factor, f.iron_sight_range, gravity),
            _ => DOWN,
        };
    } else if !aiming {
        bullets.local_way = DOWN;
    }
    bullets.was_aiming = aiming;

    // Each shot's bullets, from where the fireport is drawn this frame.
    // PROVISIONAL: the fireport as drawn this frame
    // (the game's automatic fire takes it as last drawn, its single shot as
    // its animation has just posed it); the round fired the chamber's as the
    // frame ends; the weapon new; Unity's generator seeded from the clock.
    let made = |bullets: &mut Bullets, automatic_wanted: bool| {
        for call in calls.iter().filter(|call| call.automatic == automatic_wanted) {
            let (Some(rig), Some(hands), Some(f)) = (weapon.as_ref(), hands.as_ref(), figures.as_ref()) else { continue };
            let Some(fireport) = rig.fireport else { continue };
            let Some(template) = crate::weapons::cases::fired_round(&carried, &held, &game.db) else { continue };
            let Some(round) = bullets.round(&game.db, &template) else { continue };
            let Some((at, turn)) = unity_pose(fireport, &helper, &transforms) else { continue };
            let way = trp_sim::base::math::quat_rotate(turn, bullets.local_way);
            let back = hands.hands.settings.weapon_length / 5.0;
            let mut start: V3 = std::array::from_fn(|k| at[k] - way[k] * back);
            let mut way = way;
            // The wide field of view's squeeze, about the hands' root; not
            // once a scope's optic has been turned to its zeroing point
            // (`ShotNeedsFovAdjustments`; optics spec, "Zeroing").
            let squeeze = hands.hands.fov_scale();
            if squeeze < 1.0 && hands.hands.shot_squeeze() {
                if let Some((root_at, root_turn)) = unity_pose(rig.root, &helper, &transforms) {
                    (start, way) = squeezed(start, way, root_at, root_turn, squeeze);
                }
            }
            let unspread = way;
            // With the weapon's wear and heat (ballistics spec, "Spread";
            // malfunctions spec): its durability as its item has it, the
            // last shot's heat as its logic has it; and a revolver's double
            // action's penalty where its hammer was down, less its class's
            // skill's share; and its repair buff's spread, as its item has
            // it (malfunctions spec, "Repair buffs").
            let durability = item.map_or(100.0, |item| item.durability()[0]);
            let buff = trp_sim::items::repair::RepairBuff::factor(item.and_then(|item| item.buff.as_ref()), trp_sim::items::repair::BuffKind::WeaponSpread);
            let heat = hands.firearm.as_ref().map_or(1.0, trp_sim::Firearm::heat_spread);
            let double_action = f.double_action(call.hammer_down, hands.double_action_reduce);
            let amount = f.spread_worn(&round, durability, heat, double_action, buff);
            let way = spread(way, amount, &mut bullets.random);
            bullets.shots += 1;
            if double_action > 0.0 {
                info!(
                    "shot {}: the hammer down, a double action's penalty of {:.3} ({:.2} less {:.2} of it, times the stock's {:.2}) on a centre of impact of {:.3}: spread {:.4}",
                    bullets.shots, double_action, f.double_action_penalty, hands.double_action_reduce, f.stock_double_action, f.centre_of_impact, amount
                );
            }
            if bullets.shots <= 8 {
                info!(
                    "shot {}: from ({:.3}, {:.3}, {:.3}), the fireport at ({:.3}, {:.3}, {:.3}), its way ({:.4}, {:.4}, {:.4}) spread to ({:.4}, {:.4}, {:.4}); the fireport's own way ({:.4}, {:.4}, {:.4}), squeeze {:.3}, spread {:.4}",
                    bullets.shots + 0, start[0], start[1], start[2], at[0], at[1], at[2], unspread[0], unspread[1], unspread[2], way[0], way[1], way[2],
                    bullets.local_way[0], bullets.local_way[1], bullets.local_way[2], squeeze, amount
                );
            }
            let shot = bullets.shots;
            bullets.sim.shoot(shot, &round, start, way, f.speed_factor, f.shotgun_dispersion, &mut bullets.random);
        }
    };
    made(bullets, true);
    let mut world = RangeWorld { range: &range, presets: &bullets.presets, dummy: dummy.as_deref_mut(), poppers: poppers.as_deref() };
    bullets.hits = bullets.sim.tick(time.delta_secs().min(1.0), &mut world);
    // Each hit that sets off an explosive round's fuse (`ShotDelegate`): its
    // effect at the shot's first hit, turned to the surface, heard going in
    // only; its blast 8 cm off the surface (`blast_view`, `dummy`, the effect,
    // its sound and scorch: `loose::Blasted`), its fragments made now, the
    // first nine moving on the next frame (recorded, `1-launched`: on the
    // hit's own frame at its age nought). The characters as the world's tick
    // finds them: the character after its move (`DoWorldTick`: the players'
    // tick before the ballistics'; read, not recorded), the dummies moved.
    let fused: Vec<(V3, V3, bool, bool, trp_sim::combat::ballistics::Fuse)> = bullets.hits.iter().filter_map(|hit| Some((hit.point, hit.normal, hit.first_hit, hit.forward, hit.sets_off()?.clone()))).collect();
    for (point, normal, first, forward, fuse) in fused {
        let blasted = crate::world::loose::Blasted {
            template: fuse.round.clone(),
            at: point,
            normal,
            effect: fuse.effect.clone().filter(|_| first).map(|name| (name, if forward { 1.0 } else { 0.0 })),
            blasts: !fuse.dummy,
            shot: true,
        };
        info!("a round's fuse set off at ({:.2}, {:.2}, {:.2}), the surface's normal ({:.2}, {:.2}, {:.2}){}", point[0], point[1], point[2], normal[0], normal[1], normal[2], if fuse.dummy { ": its effect alone" } else { "" });
        if blasted.blasts {
            if let Some(grenade) = crate::effects::blast_view::grenade_of(&game.db, &fuse.round).filter(|g| g.fragments > 0) {
                let at = blasted.blast_point();
                let feet = blast_feet(at, grenade.reach, alive.then_some(controller.movement.position), dummy.as_deref());
                bullets.blast_fragments(&game.db, &grenade, at, &feet, &range);
            }
        }
        blasts.write(blasted);
    }
    made(bullets, false);
    // The underbarrel launcher's grenades (`LauncherShot`, at its fire mark,
    // after the frame's tick): from its own fireport along the fireport's own
    // down (the shot's way in launcher mode: `FindAimTransforms`), back a fifth
    // of the weapon's length and squeezed as any shot (`AdjustShotVectors`),
    // spread by its ghost's centre of impact, the round's factor and the
    // ghost's barrel deviation at full durability, no heat, at the round's
    // speed times its own.
    let launched = std::mem::take(&mut controller.launcher_bullets);
    let launcher_template = item.and_then(trp_sim::items::inventory::launcher_part).map(|part| part.kind.template.clone());
    if bullets.launcher_figures.as_ref().map(|(key, _)| key) != launcher_template.as_ref() {
        bullets.launcher_figures = launcher_template.map(|template| {
            let figures = WeaponFigures::of_launcher(&game.db, &template, &[]);
            (template, figures)
        });
    }
    for template in launched {
        let (Some(rig), Some(hands), Some(f)) = (weapon.as_ref(), hands.as_ref(), bullets.launcher_figures.as_ref().and_then(|(_, f)| f.clone())) else { continue };
        let Some(fireport) = rig.launcher_fireport else { continue };
        let Some(round) = bullets.round(&game.db, &template) else { continue };
        let Some((at, turn)) = unity_pose(fireport, &helper, &transforms) else { continue };
        let way = trp_sim::base::math::quat_rotate(turn, DOWN);
        let back = hands.hands.settings.weapon_length / 5.0;
        let mut start: V3 = std::array::from_fn(|k| at[k] - way[k] * back);
        let mut way = way;
        let squeeze = hands.hands.fov_scale();
        if squeeze < 1.0 && hands.hands.shot_squeeze() {
            if let Some((root_at, root_turn)) = unity_pose(rig.root, &helper, &transforms) {
                (start, way) = squeezed(start, way, root_at, root_turn, squeeze);
            }
        }
        let unspread = way;
        let amount = f.spread_worn(&round, 100.0, 1.0, 0.0, 1.0);
        let way = spread(way, amount, &mut bullets.random);
        bullets.shots += 1;
        let pitch = |v: V3| v[1].clamp(-1.0, 1.0).asin().to_degrees();
        let camera_pitch = cameras.iter().next().map_or(f32::NAN, |(_, place)| {
            let forward = place.forward();
            forward.y.clamp(-1.0, 1.0).asin().to_degrees()
        });
        info!(
            "the launcher's grenade (shot {}): from ({:.3}, {:.3}, {:.3}), its fireport at ({:.3}, {:.3}, {:.3}), its way ({:.4}, {:.4}, {:.4}) spread to ({:.4}, {:.4}, {:.4}), {:.2} degrees up where the camera looks {:.2} up ({:.2} above it); spread {:.4}, speed factor {:.3}",
            bullets.shots, start[0], start[1], start[2], at[0], at[1], at[2], unspread[0], unspread[1], unspread[2], way[0], way[1], way[2], pitch(way), camera_pitch, pitch(way) - camera_pitch, amount, f.speed_factor
        );
        let shot = bullets.shots;
        bullets.sim.shoot(shot, &round, start, way, f.speed_factor, f.shotgun_dispersion, &mut bullets.random);
    }
    // The fragments due this frame, after the frame's tick, each a shot of
    // its own (`CreateShot`: its seed from Unity's generator).
    let pending = std::mem::take(&mut bullets.pending);
    for (due, round, at, way, speed_factor) in pending {
        if due == 0 {
            bullets.shots += 1;
            let shot = bullets.shots;
            bullets.sim.shoot(shot, &round, at, way, speed_factor, 0.0, &mut bullets.random);
        } else {
            bullets.pending.push((due - 1, round, at, way, speed_factor));
        }
    }

    // What each hit plays: the effect queued by its material's entry, ten
    // a frame; a ricochet's own sound at once (`PlayHitEffect`); played in
    // the frame they were queued, as the game's (`courses3`: every one of
    // 388 on the hit's own frame).
    let camera = cameras.iter().next();
    let in_view = |point: V3| -> bool {
        let Some((camera, at)) = camera else { return false };
        let p = Vec3::new(point[0], point[1], -point[2]);
        if at.affine().inverse().transform_point3(p).z >= 0.0 {
            return false;
        }
        camera.world_to_ndc(at, p).is_some_and(|ndc| {
            let (x, y) = ((ndc.x + 1.0) * 0.5, (ndc.y + 1.0) * 0.5);
            x > -0.01 && x < 1.01 && y > -0.01 && y < 1.01
        })
    };
    let mut queue = Vec::new();
    bullets.ricochets.clear();
    bullets.wall_blood.clear();
    for hit in &bullets.hits {
        // A hit that set off a fuse plays the material's hit only where its
        // round says so (`ShowHitEffectOnExplode`).
        if hit.sets_off().is_some_and(|fuse| !fuse.hit_effect_too) {
            continue;
        }
        // A hit the dummy took plays the material it was.
        let taken = dummy.as_ref().and_then(|dummy| dummy.taken_by(hit));
        let Some(impact) = bullets.commutator.hit(hit, bullets.decal_shift, true, taken, in_view) else { continue };
        if impact.ricochet_sound {
            bullets.ricochets.push(impact.position);
        }
        // Gone through a living dummy: blood on what is behind within the
        // reach, at once (`CheckEnvironmentHitAfterBodyHit`). Everything the
        // range's ray meets is a ballistic thing and none a body.
        // `PROVISIONAL`: the reactive targets are not met (their layer not read).
        if let Some(line) = impact.blood_behind {
            let length = (line.way[0] * line.way[0] + line.way[1] * line.way[1] + line.way[2] * line.way[2]).sqrt();
            if length > 1e-9 {
                let way = line.way.map(|c| c / length);
                if let Some(found) = trp_sim::World::ray(&*range, line.from, way, bullets.blood_reach, trp_sim::weapons::mounting::Layer::Environment) {
                    bullets.wall_blood.push((found.point, found.normal));
                }
            }
        }
        if let Some(entry) = bullets.entry_of(impact.material) {
            queue.push((impact, entry));
        }
    }
    // A blade's hits (`EffectsCommutator.PlayKnifeHitEffect`): what was struck
    // plays its material's particles, light and sound, and lays no decal.
    // A blade shown on the character's model plays its own sound for the
    // material's.
    if !controller.knife_hits.is_empty() {
        for (meeting, shown_on_model) in std::mem::take(&mut controller.knife_hits) {
            let material = match meeting.struck {
                crate::weapons::knife::Struck::Dummy(_) => Some(trp_sim::combat::ballistics::material::BODY),
                // The surface there, as a bullet's cast finds it: a short ray into it.
                crate::weapons::knife::Struck::World { .. } => {
                    let (p, n) = (meeting.point, meeting.normal);
                    let (from, to): (V3, V3) = (std::array::from_fn(|k| p[k] + n[k] * 0.05), std::array::from_fn(|k| p[k] - n[k] * 0.05));
                    let mut met = Vec::new();
                    range.bullet_hits(from, to, &bullets.presets, &mut met);
                    met.iter().min_by(|a, b| a.distance.total_cmp(&b.distance)).map(|hit| hit.surface.material)
                }
            };
            let Some(material) = material else { continue };
            let impact = trp_sim::combat::impacts::Impact::knife(material, meeting.point, meeting.normal, bullets.decal_shift, in_view(meeting.point), shown_on_model);
            if let Some(entry) = bullets.entry_of(impact.material) {
                queue.push((impact, entry));
            }
        }
    }
    queue.truncate(trp_sim::combat::impacts::MAX_EFFECTS_PER_FRAME);
    bullets.effects = queue;

    for hit in bullets.hits.iter().take(64usize.saturating_sub(bullets.logged)) {
        let name = trp_sim::combat::ballistics::material::NAMES.get(hit.surface.material as usize).copied().unwrap_or("?");
        info!(
            "a bullet of shot {} (fragment {}, seed {}) met {} {} at ({:.2}, {:.2}, {:.2}) at {:.0} m/s: {}, {} following",
            hit.shot,
            hit.fragment,
            hit.seed,
            name,
            if hit.forward { "going in" } else { "coming out" },
            hit.point[0],
            hit.point[1],
            hit.point[2],
            (hit.velocity[0].powi(2) + hit.velocity[1].powi(2) + hit.velocity[2].powi(2)).sqrt(),
            hit.state.name(),
            hit.fragments
        );
    }
    bullets.logged += bullets.hits.len();
}
