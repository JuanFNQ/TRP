//! The rounds in a thing and comparing the recorded start and setup.

use super::*;

/// The round templates whose model is the prefab of that name (an object
/// of it as the game names a copy, `patron_762x39_fmj(Clone)`).
fn rounds_of_prefab<'a>(db: &'a trp_data::Database, name: &str) -> Vec<&'a trp_data::setup::database::ItemTemplate> {
    let name = name.trim_end_matches("(Clone)");
    let mut found: Vec<&trp_data::setup::database::ItemTemplate> = db
        .items
        .values()
        .filter(|item| db.is_a(item, "Ammo") && item.prefab_key().is_some_and(|key| key.rsplit('/').next().and_then(|file| file.split('.').next()) == Some(name)))
        .collect();
    found.sort_by(|a, b| a.id.cmp(&b.id));
    found
}

/// A thing's colliders with the rounds it showed in the game (the
/// recording names each round's point and its prefab: what the thing
/// held is not recorded), each hung where ours hangs it
/// (`loose::rounds_colliders`): a magazine's on its point of that number,
/// breadth first from the magazine's object (the thing's first, or the
/// first under a weapon's `mod_magazine`), a weapon's chambered round on
/// its `patron_in_weapon`. With what was found, for the report: for each
/// magazine, the counts for which our rule (`magazine_view::round_at`)
/// shows those points, and the count its weight gives.
pub fn with_rounds(assets: &mut trp_data::unity::Assets, db: &trp_data::Database, thing: &Thrown, key: &str) -> Result<(Vec<(String, Collider)>, Vec<String>)> {
    let hierarchy = model_hierarchy(assets, db, key)?;
    let unity_points = |assets: &trp_data::unity::Assets, mesh| trp_data::unity::extract::read_mesh(assets, mesh).ok().filter(|m| !m.positions.is_empty()).map(|m| std::sync::Arc::from(m.positions));
    let mut ours = loose::colliders_named(&hierarchy, &mut |mesh| unity_points(assets, mesh));
    let mut notes = Vec::new();
    // The magazine's object, and its points.
    let magazine = hierarchy
        .nodes
        .iter()
        .position(|node| node.name == "mod_magazine")
        .map(|slot| hierarchy.nodes.iter().position(|node| node.parent == Some(slot)))
        .unwrap_or(Some(0));
    let points = magazine.map_or_else(Vec::new, |from| trp_sim::items::magazine_view::points(hierarchy.nodes.iter().map(|node| (node.parent, node.name.as_str())), from));
    let chamber = hierarchy.find_below(0, "patron_in_weapon");
    // The rounds the game showed: on which point, of which prefab.
    let mut shown: Vec<(usize, String)> = Vec::new();
    let mut numbers = Vec::new();
    for c in thing.setup["colliders"].as_array().into_iter().flatten() {
        let path = c["path"].as_str().unwrap_or("");
        let parts: Vec<&str> = path.split('/').collect();
        let Some(at) = parts.iter().position(|part| trp_sim::items::magazine_view::point_number(part).is_some() || part.starts_with("patron_in_weapon")) else { continue };
        let Some(round) = parts.get(at + 1) else { continue };
        // Once a round (its own object's trigger, its colliders).
        let tag = format!("{}/{round}", parts[at]);
        if shown.iter().any(|(_, r)| r == &tag) {
            continue;
        }
        let node = match trp_sim::items::magazine_view::point_number(parts[at]) {
            Some(number) => {
                numbers.push(number);
                points.iter().find(|(_, n)| *n == number).map(|(node, _)| *node)
            }
            None => chamber,
        };
        match node {
            Some(node) => shown.push((node, tag)),
            None => notes.push(format!("the game shows a round on {}, which our model has not", parts[at])),
        }
    }
    let mut rounds = Vec::new();
    for (node, tag) in &shown {
        let prefab = tag.rsplit('/').next().unwrap_or("");
        let Some(round) = rounds_of_prefab(db, prefab).into_iter().next() else {
            notes.push(format!("no round's model is {prefab}"));
            continue;
        };
        let key = round.prefab_key().unwrap_or("").to_owned();
        let mut model = trp_data::interface::icon::model_of(assets, &key)?.0;
        unspent(&mut model);
        rounds.push((*node, model));
    }
    let placed = loose::placed(&hierarchy);
    let names: Vec<&str> = hierarchy.nodes.iter().map(|node| node.name.as_str()).collect();
    let hung: Vec<(usize, &trp_data::unity::extract::Hierarchy)> = rounds.iter().map(|(node, model)| (*node, model)).collect();
    ours.extend(loose::rounds_colliders(&placed, &names, &hung, &mut |mesh| unity_points(assets, mesh)));
    // Our rule beside the points the game showed: the counts that show
    // those, and the count a magazine's weight gives.
    if !numbers.is_empty() {
        numbers.sort_unstable();
        let showing: Vec<u32> = (0..=200u32)
            .filter(|&count| {
                let mut on: Vec<u32> = points.iter().enumerate().filter(|(place, (_, number))| trp_sim::items::magazine_view::round_at(*place, *number, count).is_some()).map(|(_, (_, number))| *number).collect();
                on.sort_unstable();
                on == numbers
            })
            .collect();
        let counts = match (showing.first(), showing.last()) {
            (Some(a), Some(b)) if a == b => format!("{a} rounds"),
            (Some(a), Some(b)) => format!("{a} to {b} rounds"),
            _ => "no count".to_owned(),
        };
        let mut note = format!("the game shows rounds on points {numbers:?} of {}; our rule shows those for {counts}", points.len());
        let template = key.split('[').next().unwrap_or(key);
        let prefab = shown.iter().find(|(node, _)| Some(*node) != chamber).map(|(_, tag)| tag.rsplit('/').next().unwrap_or("").to_owned());
        let kinds = prefab.map(|p| rounds_of_prefab(db, &p)).unwrap_or_default();
        if let (Ok(item), Some(first)) = (db.item(template), kinds.first()) {
            let weight = |item: &trp_data::setup::database::ItemTemplate| item.prop_f32("Weight").unwrap_or(0.0);
            if db.is_a(item, "Magazine") && kinds.iter().all(|k| (weight(k) - weight(first)).abs() < 1e-6) && weight(first) > 0.0 {
                let count = (thing.kg - weight(item)) / weight(first);
                let whole = count.round() as u32;
                note.push_str(&format!("; its weight, {} kg, gives {count:.2} ({whole}): {}", thing.kg, if showing.contains(&whole) { "the same" } else { "NOT the same" }));
            }
        }
        notes.push(note);
    }
    Ok((ours, notes))
}

/// The start set beside the rule that sets a thing going: thrown
/// (`GameWorld.ThrowItem`) from the recorded thrower, or let fall
/// (`DropMod`).
pub fn compare_start(thing: &Thrown, report: &mut Report) {
    let s = &thing.start;
    if thing.let_fall() {
        report.head("Its start (let fall: `DropMod`)");
        report.vector("velocity", Some(s.velocity), [0.0, -0.01, 0.0], 1e-6);
        let within = s.spin.iter().all(|w| w.abs() <= 3.0);
        report.put(within, format!("spin {} within 3 about each axis", show(s.spin)));
        report.note("its place and turn are the weapon's magazine's as drawn, which the recording does not hold");
        return;
    }
    report.head("Its start (thrown: `GameWorld.ThrowItem`)");
    let (Some(player), Some(velocity), Some(look), Some(capsule)) = (thing.player_pos, thing.player_vel, thing.look, thing.capsule) else {
        report.note("the thrower is not recorded");
        return;
    };
    // The capsule's centre across is taken as on the player's axis (the
    // recorder writes its height only).
    // Its `WeaponRoot` as every recorded throw has it (`loose::look_root`).
    let by = Thrower { yaw: look[0], pitch: look[1], chest: [player[0], player[1] + capsule[0] * 0.65, player[2]], velocity, dt: thing.dt, weapon_root: loose::look_root(look[1], look[0]) };
    let rule = loose::thrown(&by, false, &mut Dice::new(1));
    report.vector("place (0.65 of the capsule's height, a frame of the thrower's velocity on)", Some(s.at), rule.at, 1e-4);
    report.vector("velocity (2 m/s along the look turned 45 up, and half the thrower's)", Some(s.velocity), rule.velocity, 1e-4);
    let apart = angle(s.turn, rule.turn);
    report.put(apart <= 0.05, format!("turn (the look's: `WeaponRoot` turned 90 about its x): {apart:.3} degrees apart"));
    let within = s.spin[0].abs() <= 3.0 && s.spin[1].abs() <= 3.0 && (s.spin[2].abs() - 2.0).abs() < 1e-5;
    report.put(within, format!("spin {} (up to 3 about x and y, 2 one way or the other about z)", show(s.spin)));
    // The hands' own root (the recording's `Weapon_root_anim`, in the
    // player's axes) is not what the game turns it by.
    if let Some(anim) = thing.weapon_root_anim {
        let world = mul(mul(loose::euler(0.0, look[0], 0.0), anim), loose::euler(90.0, 0.0, 0.0));
        report.note(format!("by the hands' root (`Weapon_root_anim`) turned 90 about its x it would be {:.2} degrees off", angle(s.turn, world)));
    }
}

/// Unity's material from a recorded one (none: the engine's default).
fn material_of(m: &Value) -> Material {
    if m.is_null() {
        return Material::default();
    }
    let combine = |name: &str| match m[name].as_str() {
        Some("Minimum") => Combine::Minimum,
        Some("Multiply") => Combine::Multiply,
        Some("Maximum") => Combine::Maximum,
        _ => Combine::Average,
    };
    Material {
        dynamic_friction: f(&m["dynamic_friction"]).unwrap_or(f32::NAN),
        static_friction: f(&m["static_friction"]).unwrap_or(f32::NAN),
        bounciness: f(&m["bounciness"]).unwrap_or(f32::NAN),
        friction_combine: combine("friction_combine"),
        bounce_combine: combine("bounce_combine"),
    }
}

/// Whether a thing's colliders are swept (`LootVolumeForHighQuallityPhysicsClient`).
pub fn swept(colliders: &[Collider]) -> bool {
    loose::bounds(colliders).map_or(0.0, |(low, high)| (high[0] - low[0]) * (high[1] - low[1]) * (high[2] - low[2])) <= SWEPT_UNDER
}

/// The recorded set-up beside the body `loose::add` makes of `ours` (the
/// model's colliders) at the recorded start.
pub fn compare_setup(thing: &Thrown, ours: &[(String, Collider)], physics: &trp_data::setup::engine::PhysicsSettings, report: &mut Report) {
    let mut world = World::new(Settings::of(physics));
    let loot = physics.layer("Loot").unwrap_or(15);
    let colliders: Vec<Collider> = ours.iter().map(|(_, c)| c.clone()).collect();
    let is_swept = swept(&colliders);
    let (body, shift) = loose::add(&mut world, colliders, thing.kg, &thing.start, SWEPT_UNDER, loot);
    let Some(b) = thing.setup["bodies"].as_array().and_then(|a| a.first()) else {
        report.note("no body recorded");
        return;
    };
    report.head("Its body");
    report.number("mass (its weight with all in it)", f(&b["mass"]), thing.kg, 1e-5);
    report.number("drag", f(&b["drag"]), 0.0, 1e-6);
    report.number("angular drag", f(&b["angular_drag"]), 0.05, 1e-6);
    report.flag("gravity", b["gravity"].as_bool(), true);
    report.flag("kinematic", b["kinematic"].as_bool(), false);
    report.number("most speed out of what it is in", f(&b["max_depenetration"]), world.settings().max_depenetration, 1e-6);
    report.number("most spin", f(&b["max_angular"]), physics.max_angular_speed, 1e-6);
    report.number("sleep threshold", f(&b["sleep_threshold"]), physics.sleep_threshold, 1e-7);
    let solver = b["solver"].as_array().map(|s| s.iter().filter_map(f).collect::<Vec<_>>()).unwrap_or_default();
    report.number("solver iterations", solver.first().copied(), physics.solver_iterations as f32, 0.0);
    report.number("solver velocity iterations", solver.get(1).copied(), physics.solver_velocity_iterations as f32, 0.0);
    report.text("detection", b["detection"].as_str(), if is_swept { "Continuous" } else { "Discrete" });
    report.text("interpolation", b["interpolation"].as_str(), "None");
    report.text("constraints", b["constraints"].as_str(), "None");
    let (centre, turn, principal) = world.mass_frame(body);
    // The game's object is its centre of mass (`SetShift`): ours is the
    // model's first object, its centre of mass `shift` from it.
    report.vector("centre of mass in its object's axes (ours less the shift)", v3(&b["centre_of_mass"]), sub(centre, shift), 0.001);
    report.note(format!("our shift (the centre of mass in the model's first object's axes): {}", show(shift)));
    if let (Some(rp), Some(rt)) = (v3(&b["inertia"]), q4(&b["inertia_rot"])) {
        let (r, o) = (tensor(rt, rp), tensor(turn, principal));
        let most = r.iter().fold(0.0f32, |m, x| m.max(x.abs()));
        let apart = r.iter().zip(&o).fold(0.0f32, |m, (a, b)| m.max((a - b).abs()));
        report.put(
            apart <= most * 0.01,
            format!("inertia: recorded {} about axes turned {:?}, ours {} about {:?} (as tensors {:.2}% apart)", show(rp), rt, show(principal), turn, 100.0 * apart / most.max(1e-12)),
        );
    } else {
        report.put(false, format!("inertia: not recorded, ours {}", show(principal)));
    }

    report.head("Its colliders (recorded: those switched on, not triggers)");
    let (at, turn_of) = (thing.start.at, thing.start.turn);
    let mut recorded: Vec<&Value> = Vec::new();
    let mut besides = Vec::new();
    for c in thing.setup["colliders"].as_array().into_iter().flatten() {
        let on = c["enabled"].as_bool().unwrap_or(false);
        let trigger = c["trigger"].as_bool().unwrap_or(false);
        if on && !trigger {
            recorded.push(c);
        } else {
            besides.push(format!("{} ({}{}, {})", last(c["path"].as_str().unwrap_or("")), c["kind"].as_str().unwrap_or("?"), if trigger { ", a trigger" } else { ", off" }, c["layer"].as_str().unwrap_or("?")));
        }
    }
    if !besides.is_empty() {
        report.note(format!("recorded besides: {}", besides.join("; ")));
    }
    report.number("colliders switched on", Some(recorded.len() as f32), ours.len() as f32, 0.0);
    let mut matched = vec![false; recorded.len()];
    for (name, c) in ours {
        let Some(index) = recorded.iter().enumerate().position(|(i, r)| !matched[i] && pairing(r["path"].as_str().unwrap_or("")) == *name) else {
            report.put(false, format!("ours has {name} ({:?}), the game has not switched it on", kind_of(&c.shape)));
            continue;
        };
        matched[index] = true;
        let r = recorded[index];
        let (Some(pos), Some(rot)) = (v3(&r["pos"]), q4(&r["rot"])) else { continue };
        let scale = v3(&r["scale"]).unwrap_or([1.0; 3]);
        let s = [scale[0].abs(), scale[1].abs(), scale[2].abs()];
        let turn = mul(inverse(turn_of), rot);
        let centre = v3(&r["centre"]).unwrap_or([0.0; 3]);
        let place = add(into(at, turn_of, pos), rotate(turn, [centre[0] * s[0], centre[1] * s[1], centre[2] * s[2]]));
        let kind = r["kind"].as_str().unwrap_or("");
        match (&c.shape, kind) {
            (Shape::Box { half }, "BoxCollider") => {
                let size = v3(&r["size"]).map(|z| [z[0] * s[0] / 2.0, z[1] * s[1] / 2.0, z[2] * s[2] / 2.0]);
                report.vector(&format!("{name} box's half size"), size, *half, 1e-4);
            }
            (Shape::Sphere { radius }, "SphereCollider") => report.number(&format!("{name} radius"), f(&r["radius"]).map(|x| x * s[0].max(s[1]).max(s[2])), *radius, 1e-4),
            (Shape::Capsule { radius, height, direction }, "CapsuleCollider") => {
                report.number(&format!("{name} capsule's axis"), f(&r["direction"]), *direction as f32, 0.0);
                let along = (*direction).min(2) as usize;
                let across = (0..3).filter(|axis| *axis != along).map(|axis| s[axis]).fold(0.0f32, f32::max);
                report.number(&format!("{name} capsule's radius"), f(&r["radius"]).map(|x| x * across), *radius, 1e-4);
                report.number(&format!("{name} capsule's height"), f(&r["height"]).map(|x| x * s[along]), *height, 1e-4);
            }
            (Shape::Convex { points, scale: ours_scale }, "MeshCollider") => {
                report.flag(&format!("{name} convex"), r["convex"].as_bool(), true);
                report.number(&format!("{name} mesh's points"), f(&r["vertices"]), points.len() as f32, 0.0);
                report.vector(&format!("{name} scale"), Some(scale), *ours_scale, 1e-4);
                // The mesh's own bounds (Unity's, its centre and size)
                // beside the points'.
                if let Some(bounds) = r["mesh_bounds"].as_array() {
                    let (low, high) = points.iter().fold(([f32::MAX; 3], [f32::MIN; 3]), |(l, h), p| ([l[0].min(p[0]), l[1].min(p[1]), l[2].min(p[2])], [h[0].max(p[0]), h[1].max(p[1]), h[2].max(p[2])]));
                    let middle = [(low[0] + high[0]) / 2.0, (low[1] + high[1]) / 2.0, (low[2] + high[2]) / 2.0];
                    report.vector(&format!("{name} mesh's middle"), bounds.first().and_then(v3), middle, 1e-4);
                    report.vector(&format!("{name} mesh's size"), bounds.get(1).and_then(v3), sub(high, low), 1e-4);
                }
            }
            (shape, kind) => report.put(false, format!("{name}: recorded a {kind}, ours a {:?}", kind_of(shape))),
        }
        // Where it stands from the centre of mass, in the body's axes.
        report.vector(&format!("{name} place from the centre of mass"), Some(sub(place, v3(&b["centre_of_mass"]).unwrap_or([0.0; 3]))), sub(c.at, shift), 0.001);
        let apart = angle(turn, c.turn);
        report.put(apart <= 0.1, format!("{name} turn in the body: {apart:.3} degrees apart"));
        let m = &r["material"];
        let theirs = material_of(m);
        let named = m["name"].as_str().unwrap_or("none, the engine's default");
        report.put(theirs == c.material, format!("{name} material: recorded {named} {theirs:?}, ours {:?}", c.material));
        report.number(&format!("{name} contact offset"), f(&r["contact_offset"]), physics.contact_offset, 1e-6);
        report.text(&format!("{name} layer"), r["layer"].as_str(), "Loot");
    }
    for (r, _) in recorded.iter().zip(&matched).filter(|(_, m)| !**m) {
        report.put(false, format!("the game has {} ({}) switched on, ours has not", pairing(r["path"].as_str().unwrap_or("")), r["kind"].as_str().unwrap_or("?")));
    }
}

/// The game's own colliders as recorded (those switched on, not
/// triggers), in its object's axes (its object is its centre of mass): a
/// box, sphere or capsule by its recorded figures, a convex mesh by the
/// points of ours of the same object (the recording has its bounds only).
pub fn recorded_colliders(thing: &Thrown, ours: &[(String, Collider)]) -> Vec<Collider> {
    let (at, turn_of) = (thing.start.at, thing.start.turn);
    let mut out = Vec::new();
    for r in thing.setup["colliders"].as_array().into_iter().flatten() {
        if r["enabled"].as_bool() != Some(true) || r["trigger"].as_bool() == Some(true) {
            continue;
        }
        let (Some(pos), Some(rot)) = (v3(&r["pos"]), q4(&r["rot"])) else { continue };
        let scale = v3(&r["scale"]).unwrap_or([1.0; 3]);
        let s = [scale[0].abs(), scale[1].abs(), scale[2].abs()];
        let turn = mul(inverse(turn_of), rot);
        let centre = v3(&r["centre"]).unwrap_or([0.0; 3]);
        let place = add(into(at, turn_of, pos), rotate(turn, [centre[0] * s[0], centre[1] * s[1], centre[2] * s[2]]));
        let name = pairing(r["path"].as_str().unwrap_or(""));
        let shape = match r["kind"].as_str().unwrap_or("") {
            "BoxCollider" => v3(&r["size"]).map(|z| Shape::Box { half: [z[0] * s[0] / 2.0, z[1] * s[1] / 2.0, z[2] * s[2] / 2.0] }),
            "SphereCollider" => f(&r["radius"]).map(|radius| Shape::Sphere { radius: radius * s[0].max(s[1]).max(s[2]) }),
            "CapsuleCollider" => {
                let along = f(&r["direction"]).unwrap_or(1.0) as usize;
                let across = (0..3).filter(|axis| *axis != along).map(|axis| s[axis]).fold(0.0f32, f32::max);
                match (f(&r["radius"]), f(&r["height"])) {
                    (Some(radius), Some(height)) => Some(Shape::Capsule { radius: radius * across, height: height * s[along.min(2)], direction: along.min(2) as u8 }),
                    _ => None,
                }
            }
            "MeshCollider" => ours.iter().find_map(|(n, c)| match &c.shape {
                Shape::Convex { points, .. } if *n == name => Some(Shape::Convex { points: points.clone(), scale }),
                _ => None,
            }),
            _ => None,
        };
        if let Some(shape) = shape {
            out.push(Collider { shape, at: place, turn, material: material_of(&r["material"]) });
        }
    }
    out
}

fn kind_of(shape: &Shape) -> &'static str {
    match shape {
        Shape::Box { .. } => "box",
        Shape::Sphere { .. } => "sphere",
        Shape::Capsule { .. } => "capsule",
        Shape::Convex { .. } => "convex mesh",
        Shape::Mesh { .. } => "mesh",
    }
}
