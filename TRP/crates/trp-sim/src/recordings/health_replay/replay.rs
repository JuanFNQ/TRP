//! Running a health recording through the simulation.

use super::*;

/// Replays a recording's health. `every`: a sample line so often;
/// `assets`: the install's, for the items' hands.
pub fn run(db: &Database, mut assets: Option<&mut Assets>, skills: HealthSkills, scenario: &str, frames: &[Frame], seed: u64, every: Option<f32>) -> Result<Replay> {
    let settings = Arc::new(HealthSettings::new(&db.config)?);
    let mut health = Health::new(settings, seed);
    health.skills = skills;
    let first = frames.first().context("no frames")?;
    let mut out = Replay { scenario: scenario.to_owned(), ..Default::default() };
    // The recorded character's own tops (a profile's).
    health.energy[2] = first.energy[1];
    health.hydration[2] = first.hydration[1];
    // The step's own doing first, for the effects it makes and their
    // timers; then the character as the first frame has it (the steps were
    // recorded one after another in a raid, each taking up what the one
    // before left): its parts, energy and hydration, the effects at work
    // the step did not make taken up in their recorded state with their
    // time left (the causes first: a pain then is its cause's, a tremor its
    // pain's), and what the step's damage drew by chance that the game's
    // did not, gone.
    let name = scenario.trim_end_matches(|c: char| c.is_ascii_digit()).trim_end_matches('-');
    prepare(&mut health, name)?;
    for (i, part) in first.parts.iter().enumerate() {
        health.parts[i].current = part[0];
        health.parts[i].maximum = part[1];
        health.parts[i].destroyed = first.destroyed[i];
    }
    health.energy[0] = first.energy[0];
    health.hydration[0] = first.hydration[0];
    let ours = effects_of(&health);
    let rank = |kind: &str| match kind {
        "Pain" => 1,
        "Tremor" => 2,
        _ => 0,
    };
    let mut recorded: Vec<(&(String, String), &String)> = first.effects.iter().collect();
    recorded.sort_by_key(|(key, _)| rank(&key.0));
    let mut starving: Vec<(u32, Kind)> = Vec::new();
    for (key, stage) in recorded {
        let differs = ours.get(key).is_some_and(|s| s != stage);
        let state = match stage.as_str() {
            "Added" => Some(Stage::Added),
            "Started" => Some(Stage::Started),
            _ => None,
        };
        if !ours.contains_key(key) || differs {
            if let (Some(kind), Some(part), Some(state)) = (Kind::of_name(&key.0), Part::of_name(&key.1), state) {
                // The recorded time left is the first frame's after its
                // update, which the replay steps again: given back.
                let left = first.left.get(key).copied().flatten().map(|t| t + first.dt);
                let id = health.take_up_as(kind, part, state, left);
                if matches!(kind, Kind::Exhaustion | Kind::Dehydration) && state == Stage::Started {
                    starving.push((id, kind));
                }
                let left = left.map_or(String::new(), |t| format!(", {t:.3} s left"));
                out.notes.push(format!("taken up from the first frame: {} on {} ({stage}{left})", key.0, key.1));
            }
            out.taken_up.push(key.clone());
        }
    }
    let unwanted: Vec<u32> = health.effects.iter().filter(|e| e.existing() && !first.effects.contains_key(&(e.kind.name().to_owned(), e.part.name().to_owned()))).map(|e| e.id).collect();
    for id in unwanted {
        if let Some(e) = health.effects.iter().find(|e| e.id == id) {
            out.notes.push(format!("not in the first frame, taken off: {} on {}", e.kind.name(), e.part.name()));
        }
        health.force_remove(id);
    }
    // Existence's loops began with the raid, before the recording: set so
    // that ours ticks where the game's first does (energy and hydration
    // down together by the loop's figures, the skills' cut taken; one of
    // them at nothing already cannot fall).
    let energy_tick = health.settings.existence[0] * (1.0 - health.skills.health_energy);
    let hydration_tick = health.settings.existence[2] * (1.0 - health.skills.health_hydration);
    let mut elapsed = 0.0f32;
    for pair in frames.windows(2) {
        elapsed += pair[0].dt;
        let (energy_drop, hydration_drop) = (pair[0].energy[0] - pair[1].energy[0], pair[0].hydration[0] - pair[1].hydration[0]);
        let ate = energy_drop >= energy_tick - 0.01;
        let drank = (hydration_drop - hydration_tick).abs() < 0.01;
        let (no_energy, no_hydration) = (pair[0].energy[0] <= 0.0, pair[0].hydration[0] <= 0.0);
        if (drank && (ate || no_energy)) || (ate && (energy_drop - energy_tick).abs() < 0.01 && no_hydration) {
            let at = elapsed + pair[1].dt;
            health.set_existence_clock(health.settings.existence[1] - at + pair[1].dt * 0.5);
            out.notes.push(format!("existence's first tick in the recording at {:.3} s: ours set to tick there", pair[1].t));
            break;
        }
    }

    // Starving's loops (exhaustion's and strong dehydration's) taken up
    // likewise: each set to tick where the game's first does, a frame on
    // which every part not destroyed fell by the loop's damage; the
    // shorter loop's ticks claimed first (exhaustion's every 5 s and
    // dehydration's every 15 s fall on one frame only where both tick,
    // which no recording has), the first tick of a loop one whose next
    // comes a loop later, or past the effect's end.
    starving.sort_by(|a, b| loop_of(&health, a.1)[1].total_cmp(&loop_of(&health, b.1)[1]));
    let mut ticks: Vec<(usize, f32)> = Vec::new();
    let mut elapsed = 0.0f32;
    for (index, pair) in frames.windows(2).enumerate() {
        elapsed += pair[0].dt;
        let drops: Vec<f32> = (0..7).filter(|i| !pair[1].destroyed[*i]).map(|i| pair[0].parts[i][0] - pair[1].parts[i][0]).collect();
        if drops.iter().all(|d| *d > 1e-4 && (d - drops[0]).abs() < 1e-3) && !drops.is_empty() {
            ticks.push((index + 1, elapsed + pair[1].dt));
        }
    }
    let mut claimed = vec![false; ticks.len()];
    for (id, kind) in &starving {
        let every = loop_of(&health, *kind)[1];
        let part = health.effects.iter().find(|e| e.id == *id).map_or("?", |e| e.part.name());
        let key = (kind.name().to_owned(), part.to_owned());
        let end = frames.iter().position(|f| f.effects.get(&key).is_none_or(|s| s != "Started")).map_or(f32::INFINITY, |i| frames[..=i].iter().map(|f| f.dt).sum());
        let near = |at: f32, claimed: &[bool]| ticks.iter().enumerate().find(|(i, (_, t))| !claimed[*i] && (t - at).abs() < 0.05).map(|(i, _)| i);
        let first_tick = (0..ticks.len()).find(|&i| !claimed[i] && (near(ticks[i].1 + every, &claimed).is_some() || ticks[i].1 + every > end.min(elapsed)));
        if let Some(i) = first_tick {
            // The loop keeps what it ran over (`_damageCurLoopTime -=`), so
            // its k-th tick is the first frame by whose end it has run k
            // loops: each tick bounds how far in it began, and it is set in
            // the middle of what all of them leave.
            let (index, at) = ticks[i];
            let (mut lo, mut hi) = (f32::NEG_INFINITY, f32::INFINITY);
            let (mut next, mut k) = (Some(i), 1.0f32);
            while let Some(j) = next {
                claimed[j] = true;
                let (_, clock) = ticks[j];
                lo = lo.max(every * k - clock);
                hi = hi.min(every * k - clock + frames[ticks[j].0].dt);
                next = near(ticks[j].1 + every, &claimed);
                k += 1.0;
            }
            let into = if lo < hi { (lo + hi) * 0.5 } else { every - at + frames[index].dt * 0.5 };
            health.set_loop_clock(*id, into);
            out.notes.push(format!("{}'s first tick in the recording at {:.3} s: ours set to tick there ({} ticks)", kind.name(), frames[index].t, k as u32 - 1));
        }
    }

    let note = |log: &mut Log, now: &BTreeMap<(String, String), String>, before: &BTreeMap<(String, String), String>, t: f32| {
        for (key, stage) in now {
            if before.get(key) != Some(stage) {
                log.entry(key.clone()).or_default().push((stage.clone(), t));
            }
        }
        for key in before.keys() {
            if !now.contains_key(key) {
                log.entry(key.clone()).or_default().push(("gone".into(), t));
            }
        }
    };
    let (mut recorded_before, mut simulated_before) = (BTreeMap::new(), BTreeMap::new());
    let mut item = 0u64;
    let mut next_sample = 0.0;
    let mut hands: Option<MedsHands> = None;
    let mut use_now: Option<(HandsUse, BTreeMap<u32, String>)> = None;
    // The highest the feet have been since they were last on the ground
    // (`MovementContext._startFallingHeight`), and the weight's multiplier
    // of a fall's damage (the recording's own overweight, its header's).
    let mut fall_from = first.height;
    let fall_weight = 1.0 + (health.settings.fall_overweight - 1.0) * first.overweight.clamp(0.0, 1.0);
    for (index, frame) in frames.iter().enumerate() {
        // The ground check of the frame (`CheckFlying`), at the feet's
        // height before the frame's move: the recording's is after it, so
        // the frame before's. On the ground the health is handed the height
        // fallen since last on it (0 but on a landing's frame), with the
        // chances' outcomes the game drew, as the frame's effects show them
        // (a leg's bleeding or fracture come on the landing's frame); off
        // it, the highest point is kept.
        let checked = if index == 0 { frame.height } else { frames[index - 1].height };
        if frame.grounded {
            let fallen = fall_from - checked;
            fall_from = checked;
            if fallen > health.settings.fall_safe_height {
                let came = |kind: &str, part: Part| {
                    let key = (kind.to_owned(), part.name().to_owned());
                    frame.effects.contains_key(&key) && !recorded_before.contains_key(&key)
                };
                for leg in [Part::LeftLeg, Part::RightLeg] {
                    health.outcomes.push((Chance::LightBleeding, leg, came("LightBleeding", leg)));
                    health.outcomes.push((Chance::Fracture, leg, came("Fracture", leg)));
                }
                let damage = health.handle_fall(fallen, fall_weight);
                health.outcomes.clear();
                out.notes.push(format!("{:.3} s: a fall of {fallen:.4} m, {damage:.4} to each leg; the chances as the game's", frame.t));
            }
        } else if checked > fall_from {
            fall_from = checked;
        }
        // An item taken in hand where the recording's is, and put to work by
        // its hands as they spawn (the recording's medicine effect comes on
        // that frame every time). Where it comes in the frame is where the
        // hands before it let go: the empty hands at once, as the key or the
        // click asks, before the frame's health update, so the item's effect
        // and its hands run on their first frame (the medicine course, every
        // use from the empty hands: the effect's time left already less the
        // frame's); hands with a put-away of their own (a firearm's) on its
        // end, an event of their animator's, after the health update, so
        // both run from the next frame (`courses8`, every use from a rifle:
        // the effect's whole time left on its first frame, the hands'
        // animator not yet moved).
        let med_before: Vec<&(String, String)> = recorded_before.keys().filter(|(kind, _): &&(String, String)| kind == "MedEffect").collect();
        let taken = frame.hands == "MedsController" && (index == 0 || frames[index - 1].hands != "MedsController");
        let late = taken && index > 0 && frames[index - 1].hands != "EmptyHandsController";
        if taken && !late {
            item += 1;
            let (made, use_of) = take_in_hand(db, assets.as_deref_mut(), &mut health, frames, index, &med_before, name, item, &mut out.notes)?;
            use_now = use_of.or(use_now.take());
            hands = Some(made);
        }
        // A use cut short: where ours has time left when the game's has
        // ended (no recording has one so far: the medicine course's "break
        // it off by sprinting" could not, sprinting being barred while an
        // item is at work).
        for key in &med_before {
            let left = health.existing(Kind::MedEffect, Part::of_name(&key.1)).map(|e| e.time_left());
            if !frame.effects.contains_key(*key) && left.is_some_and(|left| left > 0.1) {
                health.cancel_items();
                out.notes.push(format!("{:.3} s: the use on {} cut short ({:.3} s of ours left)", frame.t, key.1, left.unwrap_or(0.0)));
            }
        }
        // What the movement did, as the game's movement tells its health: a
        // jump landed, a sprint on a damaged leg.
        if frame.state == "JumpLanding" && index > 0 && frames[index - 1].state != "JumpLanding" {
            let conditions = health.conditions();
            health.jump_landed(conditions);
            out.notes.push(format!("{:.3} s: a jump landed", frame.t));
        }
        // Barbed wire's cuts (the wire's step, where nothing else does
        // damage): each part's drop in the frame is a cut of the game's, its
        // tally the drop over the part's multiplier; done to ours as the
        // game's `ProceedBarb` does it, as barbed-wire damage of that tally.
        if name == "barbed-wire" && index > 0 {
            for part in Part::ALL {
                let drop = frames[index - 1].parts[part.index()][0] - frame.parts[part.index()][0];
                if drop > 1e-4 {
                    let tally = drop / health.settings.environment[part.index()];
                    out.cuts.push((frame.t, part, tally));
                    health.apply_damage(part, tally, DamageKind::Barbed);
                }
            }
        }
        let conditions = health.conditions();
        health.sprint_frame(frame.dt, frame.state == "Sprint", conditions);
        health.step(frame.dt);
        // What health did goes to the item's hands, and they move on by the
        // engine's frame.
        for event in health.take_events() {
            if let Some(hands) = hands.as_mut() {
                hands.on_health(&mut health, &event);
            }
        }
        let mut put_away = false;
        if let Some(made) = hands.as_mut() {
            put_away = made.step(&mut health, frame.engine_dt, frame.pitch).contains(&MedsEvent::Hidden);
        }
        if late {
            item += 1;
            let (made, use_of) = take_in_hand(db, assets.as_deref_mut(), &mut health, frames, index, &med_before, name, item, &mut out.notes)?;
            use_now = use_of.or(use_now.take());
            hands = Some(made);
        }
        if let Some((use_of, names)) = use_now.as_mut() {
            let in_game = frame.hands == "MedsController";
            if in_game {
                let name = names.get(&frame.anim).cloned().unwrap_or_else(|| format!("#{}", frame.anim));
                if use_of.game.last().is_none_or(|(last, _)| *last != name) {
                    use_of.game.push((name, frame.t));
                }
                if use_of.game_active.last().is_none_or(|(last, _)| *last != frame.active) {
                    use_of.game_active.push((frame.active, frame.t));
                }
            } else if use_of.gone[0].is_none() {
                use_of.gone[0] = Some(frame.t);
            }
            if put_away {
                use_of.gone[1] = Some(frame.t);
            }
            let ours_in = hands.as_ref().is_some_and(|h| h.in_hand());
            if let Some(animator) = hands.as_ref().filter(|h| h.in_hand()).and_then(|h| h.animator()) {
                let (name, time, _) = animator.state();
                if use_of.ours.last().is_none_or(|(last, _)| last != name) {
                    use_of.ours.push((name.to_owned(), frame.t));
                }
                let active = animator.flag("Active");
                if use_of.ours_active.last().is_none_or(|(last, _)| *last != active) {
                    use_of.ours_active.push((active, frame.t));
                }
                let first_of_state = index > 0 && frames[index - 1].anim != frame.anim;
                if in_game && state_hash(name) == frame.anim && !first_of_state {
                    use_of.compared += 1;
                    if (time - frame.anim_time).abs() > use_of.worst.0 {
                        use_of.worst = ((time - frame.anim_time).abs(), frame.t);
                    }
                }
            }
            if !in_game && !ours_in {
                out.hands.push(use_now.take().map(|(use_of, _)| use_of).unwrap_or_default());
                hands = None;
            }
        }
        if hands.as_ref().is_some_and(|h| !h.in_hand()) && use_now.is_none() {
            hands = None;
        }

        let simulated_now = effects_of(&health);
        note(&mut out.recorded, &frame.effects, &recorded_before, frame.t);
        note(&mut out.simulated, &simulated_now, &simulated_before, frame.t);
        recorded_before = frame.effects.clone();
        simulated_before = simulated_now;
        for i in 0..7 {
            let gap = (health.parts[i].current - frame.parts[i][0]).abs();
            if gap > out.worst[i].0 {
                out.worst[i] = (gap, frame.t);
            }
            let gap = (health.parts[i].maximum - frame.parts[i][1]).abs();
            let changed = (frame.parts[i][1] - first.parts[i][1]).abs() > 0.0;
            if gap > out.worst_maximum[i].0 {
                out.worst_maximum[i] = (gap, frame.t, changed);
            }
        }
        let gap = (health.energy[0] - frame.energy[0]).abs();
        if gap > out.worst_energy.0 {
            out.worst_energy = (gap, frame.t);
        }
        let gap = (health.hydration[0] - frame.hydration[0]).abs();
        if gap > out.worst_hydration.0 {
            out.worst_hydration = (gap, frame.t);
        }
        if let Some(every) = every.filter(|_| frame.t >= next_sample) {
            next_sample += every;
            let line = |parts: Vec<f32>| parts.iter().map(|p| format!("{p:.2}")).collect::<Vec<_>>().join(" ");
            out.samples.push(format!(
                "{:7.3} game  {} | energy {:.2} hydration {:.2} | {}\n        ours  {} | energy {:.2} hydration {:.2}",
                frame.t,
                line(frame.parts.iter().map(|p| p[0]).collect()),
                frame.energy[0],
                frame.hydration[0],
                frame.hands,
                line(health.parts.iter().map(|p| p.current).collect()),
                health.energy[0],
                health.hydration[0]
            ));
        }
    }
    if let Some((use_of, _)) = use_now.take() {
        out.hands.push(use_of);
    }
    if let Some(last) = frames.last() {
        out.end = [last.parts, std::array::from_fn(|i| [health.parts[i].current, health.parts[i].maximum])];
    }
    Ok(out)
}
