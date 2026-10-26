//! A recording's bullets set beside the simulation's (`trp_sim::ballistics`;
//! `docs/spec/ballistics.md`, "Compared"; `docs/spec/body-hits.md`): each
//! recorded bullet made again with its recorded start, way, speed and seed,
//! flown by the game's rules frame by frame, and its place set beside the
//! recorded one; each frame's cast answered by the hit recorded for that
//! bullet in that frame (the recording holds no colliders), and what the
//! rules make of the hit (its state, speed, damage, penetration, the bullets
//! that follow, and theirs in turn) set beside what was recorded.
//!
//! A hit on a character's collider is answered as that collider of the
//! player prefab (by its object's name), on the character the frame's body
//! hits name; each character dressed in its recorded kit (the world file's
//! `bots`), its armour new, and taken up with the health its first recorded
//! hit found. Each hit delivered to it (`trp_sim::armor::apply_shot`) is set
//! beside the recorded one (`body_hits`): the damage and penetration as they
//! came, what the armour left, the material, and every part's health after.
//!
//! cargo run -p trp-sim --bin bullets -- <trace.jsonl>... [--every] [--worn <template>=<durability>]...

use std::collections::HashMap;
use std::io::BufRead;
use std::sync::Arc;

use anyhow::{Context, Result};
use serde_json::Value;
use trp_data::ragdoll::{HitColliderData, RagdollData};
use trp_data::unity::Assets;
use trp_data::{Database, Install};
use trp_sim::armor::{self, ArmorRules, VestSkills, Wearer, Worn};
use trp_sim::ballistics::{Asker, Ballistics, BodyRef, BulletWorld, Hit, RayHit, Round, Surface, V3};
use trp_sim::health::{Health, HealthSettings, Part};

/// A recorded hit: whose (its seed and fragment number), where, which way
/// the cast that found it looked, and what it met.
#[derive(Clone, Debug)]
struct Recorded {
    seed: i32,
    fragment: i32,
    point: V3,
    normal: V3,
    forward: bool,
    surface: Surface,
    body: Option<BodyRef>,
    material: String,
    collider: String,
    state: String,
    speed: f32,
    velocity: V3,
    penetration: f32,
    damage: f32,
    cos: f32,
    age: f32,
    fragments: usize,
    blocked: bool,
    deflected: bool,
}

/// A character the recording's shots hit.
struct Victim {
    wearer: Wearer,
    health: Health,
}

/// The frame's recorded hits, as a world: a bullet's cast meets the hit
/// recorded for it, if its way is the cast's. The characters it hits, and
/// what each hit delivered did.
struct RecordedWorld<'a> {
    hits: &'a [Recorded],
    things: &'a HashMap<String, u64>,
    victims: &'a mut HashMap<u32, Victim>,
    /// The frame's recorded body hits, in the order the game delivered
    /// them: each delivered with the damage, penetration and armour share
    /// the game's came with (the world's step, which those hang on, is not
    /// always recorded), so that the armour's and health's rules are set
    /// beside the game's on the game's figures.
    records: &'a [&'a Value],
    /// What each delivered hit did, and every part's health after it.
    delivered: Vec<Delivered>,
}

struct Delivered {
    hit: Hit,
    outcome: Option<armor::ShotOutcome>,
    after: [f32; 7],
}

impl BulletWorld for RecordedWorld<'_> {
    fn ray(&self, asker: Asker, from: V3, _to: V3, out: &mut Vec<RayHit>) {
        for hit in self.hits.iter().filter(|h| h.seed == asker.seed && h.fragment == asker.fragment && h.forward == asker.forward) {
            let d = [hit.point[0] - from[0], hit.point[1] - from[1], hit.point[2] - from[2]];
            let reach = ((_to[0] - from[0]).powi(2) + (_to[1] - from[1]).powi(2) + (_to[2] - from[2]).powi(2)).sqrt();
            let along = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
            if std::env::var_os("TRP_BULLETS_DEBUG").is_some() {
                println!("  cast of seed {} ({}): the recorded hit {along:.4} m along a cast of {reach:.4} m", asker.seed, if asker.forward { "forward" } else { "backward" });
            }
            // The recorded point stands for the game's collider: one a hair
            // past our cast's end (our segment's end differs from the game's
            // by rounding) is taken at its end.
            let distance = if along > reach && along <= reach + 0.001 { reach } else { along };
            let thing = self.things.get(&hit.collider).copied().unwrap_or(0);
            out.push(RayHit { distance, point: hit.point, normal: hit.normal, collider: thing, thing, mesh: false, surface: hit.surface, body: hit.body });
        }
    }

    fn wearer(&self, character: u32) -> Option<&Wearer> {
        self.victims.get(&character).map(|v| &v.wearer)
    }

    fn delivered(&mut self, hit: &Hit) {
        let Some(body) = hit.body else { return };
        if !hit.forward || hit.out_of_life {
            return;
        }
        let mut fed = hit.clone();
        if let Some(r) = self.records.get(self.delivered.len()) {
            fed.damage = f(r, "damage_in");
            fed.no_damage = false;
            fed.penetration = f(r, "penetration_in");
            fed.armor_damage = f(r, "armor_damage_in");
        }
        let Some(victim) = self.victims.get_mut(&body.character) else { return };
        // Health changed between hits by the character's own doing (a bot's
        // medicine, not run here): taken up again as the game's hit found it.
        if let Some(before) = self.records.get(self.delivered.len()).and_then(|r| seven(r.get("health_before"))) {
            let ours: [f32; 7] = std::array::from_fn(|i| victim.health.part(Part::ALL[i]).current);
            if ours.iter().zip(&before).any(|(a, b)| (a - b).abs() > 0.01) {
                let maximum = std::array::from_fn(|i| victim.health.part(Part::ALL[i]).maximum.max(before[i]));
                println!("  health taken up again before a hit: {ours:?} ours, {before:?} the game's");
                victim.health.take_up_parts(before, maximum);
            }
        }
        let outcome = armor::apply_shot(Some(&mut victim.wearer), &mut victim.health, &fed);
        let after = std::array::from_fn(|i| victim.health.part(Part::ALL[i]).current);
        self.delivered.push(Delivered { hit: hit.clone(), outcome, after });
    }
}

fn v3(v: &Value) -> Option<V3> {
    let a = v.as_array()?;
    Some([a.first()?.as_f64()? as f32, a.get(1)?.as_f64()? as f32, a.get(2)?.as_f64()? as f32])
}
fn f(v: &Value, name: &str) -> f32 {
    v.get(name).and_then(Value::as_f64).unwrap_or(f64::NAN) as f32
}
fn s<'a>(v: &'a Value, name: &str) -> &'a str {
    v.get(name).and_then(Value::as_str).unwrap_or("")
}
fn seven(v: Option<&Value>) -> Option<[f32; 7]> {
    let a = v?.as_array()?;
    let mut out = [0.0; 7];
    for (k, x) in a.iter().take(7).enumerate() {
        out[k] = x.as_f64()? as f32;
    }
    Some(out)
}

/// A number after `"name":` in a line, without parsing it whole.
fn quick(line: &str, name: &str) -> Option<f64> {
    let key = format!("\"{name}\":");
    let at = line.find(&key)? + key.len();
    let rest = &line[at..];
    let end = rest.find([',', '}', ']']).unwrap_or(rest.len());
    rest[..end].parse().ok()
}

/// The equipment's template (`Equipment`): the kit's root.
const EQUIPMENT: &str = "55d7217a4bdc2d86028b456d";

/// A recorded kit (`[template, slot, parent's template]`, depth first) as
/// the things in the equipment's slots: each thing's parent the last thing
/// before it of its parent's template.
fn kit_tree(kit: &[Value]) -> Vec<Worn> {
    let entries: Vec<(String, String, String)> = kit.iter().filter_map(|e| Some((e.get(0)?.as_str()?.to_owned(), e.get(1)?.as_str()?.to_owned(), e.get(2)?.as_str()?.to_owned()))).collect();
    let mut parent_of: Vec<Option<usize>> = Vec::with_capacity(entries.len());
    for (k, (_, _, parent)) in entries.iter().enumerate() {
        parent_of.push(if parent == EQUIPMENT { None } else { (0..k).rev().find(|&j| entries[j].0 == *parent) });
    }
    fn build(at: usize, entries: &[(String, String, String)], parent_of: &[Option<usize>]) -> Worn {
        let children = (0..entries.len()).filter(|&k| parent_of[k] == Some(at)).map(|k| build(k, entries, parent_of)).collect();
        Worn { template: entries[at].0.clone(), slot: entries[at].1.clone(), children }
    }
    (0..entries.len()).filter(|&k| parent_of[k].is_none() && entries[k].2 == EQUIPMENT).map(|k| build(k, &entries, &parent_of)).collect()
}

/// The kits of the characters a world file follows, by profile.
fn kits(world: &std::path::Path) -> HashMap<String, Vec<Worn>> {
    let mut out = HashMap::new();
    let Ok(file) = std::fs::File::open(world) else { return out };
    for line in std::io::BufReader::new(file).lines().map_while(Result::ok) {
        if !line.contains("\"setup\":{") {
            continue;
        }
        let Ok(v) = serde_json::from_str::<Value>(&line) else { continue };
        for bot in v.get("bots").and_then(Value::as_array).into_iter().flatten() {
            let Some(setup) = bot.get("setup") else { continue };
            let (Some(profile), Some(kit)) = (setup.get("profile").and_then(Value::as_str), setup.get("kit").and_then(Value::as_array)) else { continue };
            out.entry(profile.to_owned()).or_insert_with(|| kit_tree(kit));
        }
    }
    out
}

#[derive(Default)]
struct Tally {
    flights: usize,
    flight_frames: usize,
    flight_worst: f32,
    flight_sum: f64,
    hits: usize,
    matched: usize,
    states_agree: usize,
    missing: usize,
    /// Of those, after a frame whose step the recording does not hold.
    missing_unknown_step: usize,
    body: usize,
    body_matched: usize,
    body_agree: usize,
    worst: [f32; 6],
    delivered: usize,
    delivered_recorded: usize,
    delivered_agree: usize,
    /// Worst gaps of the delivered: damage in, damage out, a part's health.
    delivered_worst: [f32; 3],
}

/// The frame's new bullets, each made with its recorded start, way, speed
/// and seed.
fn make_new(v: &Value, sim: &mut Ballistics, db: &Database, rounds: &mut HashMap<String, Arc<Round>>, tally: &mut Tally, frame: i64) {
    let Some(news) = v.get("bullets_new").and_then(Value::as_array) else { return };
    for b in news {
        let ammo = s(b, "ammo").to_owned();
        let round = match rounds.get(&ammo) {
            Some(r) => r.clone(),
            None => {
                let Some(r) = db.items.get(&ammo).and_then(Round::of) else { continue };
                let r = Arc::new(r);
                rounds.insert(ammo.clone(), r.clone());
                r
            }
        };
        let (Some(id), Some(start), Some(direction)) = (b.get("id").and_then(Value::as_i64), b.get("start").and_then(v3), b.get("direction").and_then(v3)) else { continue };
        let seed = b.get("seed").and_then(Value::as_i64).unwrap_or(0) as i32;
        let fragment = b.get("fragment").and_then(Value::as_i64).unwrap_or(0) as i32;
        let speed_factor = f(b, "initial_speed") / round.initial_speed;
        sim.create(id as u32, &round, fragment, seed, start, direction, speed_factor);
        let pen = sim.tables().normal(seed, round.penetration_power, round.penetration_deviation);
        if (pen - f(b, "penetration")).abs() > 1e-3 {
            println!("frame {frame}: bullet {id}'s penetration {} recorded, {pen} ours", f(b, "penetration"));
        }
        tally.flights += 1;
    }
}

/// A character's collider of the player prefab as a hit's: its part, type,
/// plate and damage share (a body part's from the globals, a plate's the
/// prefab's), and its figures as the game sets them.
fn body_of(c: &HitColliderData, rules: &ArmorRules, character: u32) -> (BodyRef, [f32; 6]) {
    let mut figures = c.figures;
    let mut damage_mod = c.damage_mod;
    if c.plate == 0 {
        if let Some([level, chance, share]) = rules.colliders.get(c.collider_type as usize).copied().flatten() {
            figures[0] = level;
            figures[1] = chance;
            damage_mod = share;
        }
    }
    (BodyRef { character, part: c.part, collider: c.collider_type, plate: c.plate, damage_mod }, figures)
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let every = args.iter().any(|a| a == "--every");
    // `--worn <template>=<durability>`: a character's piece of armour as it
    // came into the raid (a bot's spawns worn; the recording does not say).
    let worn_at: Vec<(String, f32)> = args
        .windows(2)
        .filter(|w| w[0] == "--worn")
        .filter_map(|w| {
            let (template, durability) = w[1].split_once('=')?;
            Some((template.to_owned(), durability.parse().ok()?))
        })
        .collect();
    let traces: Vec<&String> = args.iter().skip(1).filter(|a| !a.starts_with("--") && !a.contains('=')).collect();
    anyhow::ensure!(!traces.is_empty(), "a trace");
    let install = Install::locate()?;
    let db = Database::load(&install)?;
    let degradation = db.config.get("Ballistic").and_then(|b| b.get("GlobalDamageDegradationCoefficient")).and_then(Value::as_f64).context("the degradation coefficient")? as f32;
    println!("damage's degradation coefficient {degradation}");
    let rules = Arc::new(ArmorRules::new(&db.config)?);
    let health_settings = Arc::new(HealthSettings::new(&db.config)?);
    let mut assets = Assets::new(install.clone())?;
    let colliders: HashMap<String, HitColliderData> = RagdollData::hit_colliders(&mut assets)?.into_iter().map(|c| (c.object.clone(), c)).collect();
    println!("the player prefab's hit colliders: {}", colliders.len());
    for path in traces {
        let mut tally = Tally::default();
        let mut sim = Ballistics::new(degradation, [0.0, -9.81, 0.0]);
        let mut rounds: HashMap<String, Arc<Round>> = HashMap::new();
        let mut things: HashMap<String, u64> = HashMap::new();
        // Each recorded bullet's age the frame before, to find the world's step.
        let mut ages: HashMap<String, f32> = HashMap::new();
        // The characters hit: by profile their number, and each dressed.
        let world_path = std::path::PathBuf::from(path.replace(".jsonl", ".world.jsonl"));
        let kits = kits(&world_path);
        println!("{}: {} kits", world_path.display(), kits.len());
        let mut numbers: HashMap<String, u32> = HashMap::new();
        let mut victims: HashMap<u32, Victim> = HashMap::new();
        let mut last_victim: Option<u32> = None;
        let file = std::fs::File::open(path).with_context(|| path.to_string())?;
        let mut frames = 0usize;
        // Whether the last frame's step was a recorded bullet's (else Unity's
        // own frame time, on another clock than the world's).
        let mut last_step_known = true;
        for line in std::io::BufReader::new(file).lines() {
            let line = line?;
            if !line.starts_with("{\"frame\"") && !line.contains("\"frame\":") {
                continue;
            }
            frames += 1;
            let busy = line.contains("\"bullets\":{\"") || line.contains("\"shot_hits\":[{") || line.contains("\"bullets_new\":[{");
            if !busy {
                if !sim.bullets().is_empty() {
                    let dt = quick(&line, "dt").unwrap_or(0.0) as f32;
                    let none: Vec<Recorded> = Vec::new();
                    sim.tick(dt, &mut RecordedWorld { hits: &none, things: &things, victims: &mut victims, records: &[], delivered: Vec::new() });
                    last_step_known = false;
                }
                ages.clear();
                continue;
            }
            let v: Value = serde_json::from_str(&line)?;
            let frame = v.get("frame").and_then(Value::as_i64).unwrap_or(-1);
            // The frame's recorded hits on characters others shot (the
            // player's own are another's bullets'), and each character's first
            // taken up.
            let body_hits: Vec<&Value> = v.get("body_hits").and_then(Value::as_array).map(|all| all.iter().filter(|h| h.get("main").and_then(Value::as_bool) == Some(false)).collect()).unwrap_or_default();
            for h in &body_hits {
                let profile = s(h, "profile").to_owned();
                let next = numbers.len() as u32 + 1;
                let number = *numbers.entry(profile.clone()).or_insert(next);
                last_victim = Some(number);
                if let std::collections::hash_map::Entry::Vacant(e) = victims.entry(number) {
                    let worn = kits.get(&profile).cloned().unwrap_or_default();
                    let mut wearer = Wearer::dress(&db, rules.clone(), &worn, VestSkills::default());
                    for piece in &mut wearer.pieces {
                        if let Some((_, durability)) = worn_at.iter().find(|(t, _)| *t == piece.template) {
                            piece.durability = *durability;
                        }
                    }
                    let before = seven(h.get("health_before")).unwrap_or([0.0; 7]);
                    let mut health = Health::new(health_settings.clone(), 1);
                    health.take_up_parts(before, before);
                    println!(
                        "frame {frame}: {profile} taken up with health {before:?}, armour {:?}, plates on {:#x}",
                        wearer.pieces.iter().map(|p| format!("{} class {} {:.0}", p.template, p.class, p.durability)).collect::<Vec<_>>(),
                        wearer.plates_on()
                    );
                    e.insert(Victim { wearer, health });
                }
            }
            // The world's step: a recorded bullet's age grown since the frame
            // before, else the frame's own time.
            let bullets = v.get("bullets").and_then(Value::as_object);
            let mut step = None;
            if let Some(map) = bullets {
                for (id, b) in map {
                    let (Some(age), Some(state)) = (b.get(3).and_then(Value::as_f64), b.get(2).and_then(Value::as_str)) else { continue };
                    if state == "Flying" {
                        if let Some(before) = ages.get(id) {
                            step.get_or_insert(age as f32 - before);
                        }
                    }
                }
            }
            // A shot made before the frame's step (automatic fire) shows on
            // its own frame with an age already: that age is the step, if it
            // is still flying (one that hit shows its age at the hit).
            let news: Vec<i64> = v.get("bullets_new").and_then(Value::as_array).map(|all| all.iter().filter_map(|b| b.get("id")?.as_i64()).collect()).unwrap_or_default();
            let early_age = bullets
                .and_then(|map| {
                    news.iter().find_map(|id| {
                        let b = map.get(&id.to_string())?;
                        (b.get(2)?.as_str()? == "Flying").then_some(())?;
                        b.get(3)?.as_f64()
                    })
                })
                .filter(|&age| age > 0.0);
            // A shot made before the step that hit in it: made before the
            // step all the same.
            let made_early = bullets.is_some_and(|map| news.iter().any(|id| map.get(&id.to_string()).and_then(|b| b.get(3)?.as_f64()).is_some_and(|age| age > 0.0)));
            let dt = step.or(early_age.map(|a| a as f32)).unwrap_or_else(|| f(&v, "dt"));
            let step_known = step.is_some() || early_age.is_some() || !sim.bullets().iter().any(|b| b.depth == 0);
            let steps_known = step_known && last_step_known;
            if made_early {
                make_new(&v, &mut sim, &db, &mut rounds, &mut tally, frame);
            }
            // The frame's recorded hits.
            let recorded: Vec<Recorded> = v
                .get("shot_hits")
                .and_then(Value::as_array)
                .map(|all| {
                    all.iter()
                        .filter_map(|h| {
                            let ballistic = h.get("ballistic")?.as_array()?;
                            let mut figures: [f32; 6] = std::array::from_fn(|k| ballistic.get(k).and_then(Value::as_f64).unwrap_or(0.0) as f32);
                            let material = s(h, "material").to_owned();
                            let number = trp_sim::ballistics::material::named(&material).unwrap_or(0);
                            // A character's collider: the prefab's, on the
                            // character the frame's hits name.
                            let body = if s(h, "body_part").is_empty() {
                                None
                            } else {
                                let c = colliders.get(s(h, "met")).or_else(|| colliders.get(s(h, "collider")));
                                c.zip(last_victim).map(|(c, who)| {
                                    let (body, ours) = body_of(c, &rules, who);
                                    if ours.iter().zip(&figures).any(|(a, b)| (a - b).abs() > 1e-4) {
                                        println!("frame {frame}: {}'s figures recorded {figures:?}, ours {ours:?}", c.object);
                                    }
                                    figures = ours;
                                    body
                                })
                            };
                            Some(Recorded {
                                seed: h.get("seed")?.as_i64()? as i32,
                                fragment: h.get("fragment")?.as_i64()? as i32,
                                point: v3(h.get("point")?)?,
                                normal: v3(h.get("normal")?)?,
                                forward: h.get("forward")?.as_bool()?,
                                surface: Surface { material: number, figures },
                                body,
                                material,
                                collider: format!("{}|{}", s(h, "met"), s(h, "collider")),
                                state: s(h, "state").to_owned(),
                                speed: f(h, "speed"),
                                velocity: v3(h.get("velocity")?)?,
                                penetration: f(h, "penetration"),
                                damage: f(h, "damage"),
                                cos: f(h, "cos"),
                                age: f(h, "age"),
                                fragments: h.get("fragments").and_then(Value::as_u64).unwrap_or(0) as usize,
                                blocked: h.get("blocked").and_then(Value::as_bool).unwrap_or(false),
                                deflected: h.get("deflected").and_then(Value::as_bool).unwrap_or(false),
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();
            for r in &recorded {
                let n = things.len() as u64 + 1;
                things.entry(r.collider.clone()).or_insert(n);
            }
            let mut world = RecordedWorld { hits: &recorded, things: &things, victims: &mut victims, records: &body_hits, delivered: Vec::new() };
            let ours: Vec<Hit> = if sim.bullets().is_empty() { Vec::new() } else { sim.tick(dt, &mut world) };
            let delivered = std::mem::take(&mut world.delivered);
            if !made_early {
                make_new(&v, &mut sim, &db, &mut rounds, &mut tally, frame);
            }
            // Each recorded hit beside ours.
            for r in &recorded {
                tally.hits += 1;
                let on_body = r.body.is_some();
                if on_body {
                    tally.body += 1;
                }
                let Some(o) = ours.iter().find(|o| o.seed == r.seed && o.fragment == r.fragment) else {
                    tally.missing += 1;
                    if !steps_known {
                        tally.missing_unknown_step += 1;
                    }
                    if every || on_body {
                        println!("frame {frame}: recorded hit of seed {} fragment {} on {} ({}) not ours{}", r.seed, r.fragment, r.material, r.state, if steps_known { "" } else { " (this or the last frame's step not recorded)" });
                    }
                    continue;
                };
                tally.matched += 1;
                if on_body {
                    tally.body_matched += 1;
                }
                let speed_at = |v: V3| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
                let gaps = [
                    (o.speed - r.speed).abs(),
                    (speed_at(o.velocity) - speed_at(r.velocity)).abs(),
                    (o.damage - r.damage).abs(),
                    (o.penetration - r.penetration).abs(),
                    (o.cos - r.cos).abs(),
                    (o.age - r.age).abs(),
                ];
                for k in 0..6 {
                    tally.worst[k] = tally.worst[k].max(gaps[k]);
                }
                let agree = o.state.name() == r.state && o.fragments == r.fragments && o.blocked_by.is_some() == r.blocked && o.deflected_by.is_some() == r.deflected;
                if agree {
                    tally.states_agree += 1;
                    if on_body {
                        tally.body_agree += 1;
                    }
                }
                if every || !agree || (on_body && (gaps[2] > 0.01 || gaps[3] > 0.01)) {
                    println!(
                        "frame {frame}: seed {} fragment {} on {} {} {}: {} / ours {} ({} / {} fragments{}{}); speed {:.3}/{:.3}, at the hit {:.3}/{:.3}, damage {:.3}/{:.3}, penetration {:.3}/{:.3}, cos {:.4}/{:.4}, age {:.5}/{:.5}",
                        r.seed, r.fragment, r.material, r.collider, if r.forward { "forward" } else { "backward" }, r.state, o.state.name(), r.fragments, o.fragments,
                        if r.blocked || o.blocked_by.is_some() { format!(", blocked {}/{}", r.blocked, o.blocked_by.is_some()) } else { String::new() },
                        if r.deflected || o.deflected_by.is_some() { format!(", deflected {}/{}", r.deflected, o.deflected_by.is_some()) } else { String::new() },
                        r.speed, o.speed, speed_at(r.velocity), speed_at(o.velocity), r.damage, o.damage, r.penetration, o.penetration, r.cos, o.cos, r.age, o.age
                    );
                }
            }
            // Each hit delivered to a character beside the recorded, in order.
            tally.delivered += delivered.len();
            tally.delivered_recorded += body_hits.len();
            for (k, h) in body_hits.iter().enumerate() {
                let Some(Delivered { hit, outcome, after: ours_after }) = delivered.get(k) else {
                    println!("frame {frame}: recorded body hit {k} on {} {} not delivered by ours", s(h, "part"), s(h, "collider"));
                    continue;
                };
                let Some(o) = outcome else {
                    // Not taken: on the dead, as the game's (its result none).
                    if h.get("result").is_some_and(Value::is_null) {
                        tally.delivered_agree += 1;
                    } else {
                        println!("frame {frame}: recorded body hit {k} taken by the game, not by ours (the character dead here)");
                    }
                    continue;
                };
                let ours_after = *ours_after;
                let after = seven(h.get("health_after")).unwrap_or([0.0; 7]);
                let health_gap = after.iter().zip(&ours_after).map(|(a, b)| (a - b).abs()).fold(0.0f32, f32::max);
                // The damage our own flight brought (the step's clock aside).
                let flown = if hit.no_damage { 0.0 } else { hit.damage };
                let gaps = [(f(h, "damage_in") - flown).abs(), (f(h, "damage_out") - o.taken.damage).abs(), health_gap];
                for i in 0..3 {
                    tally.delivered_worst[i] = tally.delivered_worst[i].max(gaps[i]);
                }
                let material = trp_sim::ballistics::material::NAMES.get(o.taken.material as usize).copied().unwrap_or("?");
                let same_place = s(h, "part") == o.part.name() && s(h, "collider") == armor::COLLIDER_TYPES[hit.body.map_or(0, |b| b.collider) as usize];
                let agree = same_place && material == s(h, "material") && gaps[1] < 0.01 && gaps[2] < 0.01;
                if agree {
                    tally.delivered_agree += 1;
                }
                if every || !agree {
                    println!(
                        "frame {frame}: body hit on {} {} {}: damage in {:.4}/{:.4}, out {:.4}/{:.4}, penetration {:.3}/{:.3}, material {}/{material}, blocked {}/{}, health after {:?} / ours {:?}",
                        s(h, "part"),
                        s(h, "collider"),
                        s(h, "plate"),
                        f(h, "damage_in"),
                        flown,
                        f(h, "damage_out"),
                        o.taken.damage,
                        f(h, "penetration_in"),
                        hit.penetration,
                        s(h, "material"),
                        h.get("blocked").and_then(Value::as_bool).unwrap_or(false),
                        hit.blocked_by.is_some(),
                        after,
                        ours_after
                    );
                }
            }
            last_step_known = step_known;
            // Each first bullet's place beside ours.
            ages.clear();
            if let Some(map) = bullets {
                for (id, b) in map {
                    let (Some(at), Some(age), Some(state)) = (b.get(0).and_then(v3), b.get(3).and_then(Value::as_f64), b.get(2).and_then(Value::as_str)) else { continue };
                    ages.insert(id.clone(), age as f32);
                    let Ok(number) = id.parse::<u32>() else { continue };
                    let Some(ours) = sim.bullets().iter().find(|o| o.shot == number && o.depth == 0) else { continue };
                    if state != "Flying" {
                        continue;
                    }
                    let gap = ((ours.position[0] - at[0]).powi(2) + (ours.position[1] - at[1]).powi(2) + (ours.position[2] - at[2]).powi(2)).sqrt();
                    tally.flight_frames += 1;
                    tally.flight_sum += gap as f64;
                    tally.flight_worst = tally.flight_worst.max(gap);
                    if every && gap > 0.01 {
                        println!("frame {frame}: bullet {id} at age {age:.4} is {gap:.4} m from ours (age {:.4})", ours.age);
                    }
                }
            }
        }
        println!("== {path}: {frames} frames");
        println!(
            "  flights: {} bullets, {} frames in flight, places off by {:.5} m on average, {:.5} at most",
            tally.flights,
            tally.flight_frames,
            tally.flight_sum / tally.flight_frames.max(1) as f64,
            tally.flight_worst
        );
        println!(
            "  hits: {} recorded ({} on characters), {} matched ({} with the state, fragments, blocked and deflected the game's; on characters {} of {}), {} not ours ({} after a frame whose step is not recorded)",
            tally.hits, tally.body, tally.matched, tally.states_agree, tally.body_agree, tally.body_matched, tally.missing, tally.missing_unknown_step
        );
        println!(
            "  worst gaps: speed made {:.4}, speed at the hit {:.4}, damage {:.4}, penetration {:.4}, cos {:.5}, age {:.6}",
            tally.worst[0], tally.worst[1], tally.worst[2], tally.worst[3], tally.worst[4], tally.worst[5]
        );
        println!(
            "  delivered to characters, each on the game's damage and penetration: {} recorded, {} ours, {} agreeing (place, material, damage out, every part's health); worst gaps: damage in as our flight brought it {:.4}, out {:.4}, a part's health {:.4}",
            tally.delivered_recorded, tally.delivered, tally.delivered_agree, tally.delivered_worst[0], tally.delivered_worst[1], tally.delivered_worst[2]
        );
    }
    Ok(())
}
