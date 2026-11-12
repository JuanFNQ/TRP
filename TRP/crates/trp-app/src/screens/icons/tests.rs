//! Tests of the inventory's pictures and their keys.

use super::*;
use std::sync::Arc;
use trp_data::character::inventory::Holder;
use trp_sim::items::inventory::{Item, Kind};

/// The pictures drawn with the see-through glass are kept apart from
/// those drawn with it off, which stay in `v11`'s folder, unchanged.
#[test]
fn the_glass_s_pictures_are_cached_apart() {
    assert_eq!(cache_folder(true), "v12");
    assert_eq!(cache_folder(false), "v11");
}

fn thing(template: &str, classes: &[&str], tweak: impl FnOnce(&mut Kind)) -> Item {
    let named: Vec<String> = classes.iter().map(|class| (*class).to_owned()).chain(["Item".to_owned()]).collect();
    let mut kind = Kind { template: template.into(), name: template.into(), classes: named.clone(), class_names: named, cells: [2, 2], ..Kind::default() };
    tweak(&mut kind);
    Item::new(Arc::new(kind))
}

/// A helmet's picture is asked for by what its slots hold and by its
/// switches (the game's `IconsHash`): bare, its template; its face
/// shield fitted, another key, which names the shield; the shield
/// flipped down, another again, which names the shield's place; and
/// each key comes apart into those, for the icon thread to draw.
#[test]
fn a_helmet_s_picture_follows_its_shield() {
    let shield = thing("shield", &["FaceShield", "ArmoredEquipment"], |kind| {
        kind.hinge = true;
        kind.face_shield = true;
    });
    let mut helmet = thing("helmet", &["Helmet", "ArmoredEquipment"], |kind| kind.slots = vec![Holder { name: "mod_equipment".into(), ..Holder::default() }]);
    assert_eq!(picture_key(&helmet), "helmet");
    helmet.slots[0] = Some(shield);
    let up = picture_key(&helmet);
    assert_eq!(up, "helmet[mod_equipment=shield[]]");
    let helmet_id = helmet.id;
    trp_sim::items::inventory::toggle(&mut helmet, helmet_id).expect("its shield's switch");
    let down = picture_key(&helmet);
    assert_eq!(down, format!("{up}{SWITCHED}mod_equipment"));
    let parts = split_key(&down);
    assert_eq!((parts.template, parts.build, parts.on), ("helmet", Some(up.as_str()), vec!["mod_equipment"]));
    let build = trp_data::weapons::weapon::preset_of_key(parts.build.unwrap()).expect("its parts");
    assert_eq!(build.items.len(), 2);
    assert_eq!(build.items[1].slot.as_deref(), Some("mod_equipment"));
    // A thing that is its own switch is at its own place.
    let mut night = thing("nvg", &["NightVision"], |_| ());
    night.on = true;
    assert_eq!(picture_key(&night), format!("nvg{SWITCHED}."));
}

/// What the log is told while one picture is made ready: each event's
/// level and words (a warning, a part left out).
#[derive(Clone, Default)]
struct Heard(Arc<std::sync::Mutex<Vec<(bevy::log::Level, String)>>>);

impl<S: bevy::log::tracing::Subscriber> bevy::log::tracing_subscriber::Layer<S> for Heard {
    fn on_event(&self, event: &bevy::log::tracing::Event<'_>, _: bevy::log::tracing_subscriber::layer::Context<'_, S>) {
        struct Words(String);
        impl bevy::log::tracing::field::Visit for Words {
            fn record_debug(&mut self, field: &bevy::log::tracing::field::Field, value: &dyn std::fmt::Debug) {
                if field.name() == "message" {
                    self.0 = format!("{value:?}");
                }
            }
        }
        let mut words = Words(String::new());
        event.record(&mut words);
        self.0.lock().unwrap().push((*event.metadata().level(), words.0));
    }
}

/// A reason's words with what names one item taken out (ids, and the
/// names of assets, which have an underscore), to count the reasons
/// alike together.
fn reason_of(words: &str) -> String {
    let mut out = String::new();
    let mut run = String::new();
    let flush = |run: &mut String, out: &mut String| {
        if run.len() == 24 && run.chars().all(|c| c.is_ascii_hexdigit()) {
            out.push_str("<id>");
        } else if run.contains('_') && run.len() > 1 && !run.starts_with('_') {
            out.push('…');
        } else {
            out.push_str(run);
        }
        run.clear();
    };
    for c in words.chars() {
        if c.is_ascii_alphanumeric() || c == '_' {
            run.push(c);
        } else {
            flush(&mut run, &mut out);
            out.push(c);
        }
    }
    flush(&mut run, &mut out);
    out.chars().take(140).collect()
}

/// Every item's picture made ready as the app makes it (`Worker::
/// own_picture`, then `Worker::prepare`), without drawing it: each
/// template as `--carry` makes it (its default build where it has one)
/// and bare, and each of the database's builds; what fails, what the
/// log is told, a part left out for its material, nothing drawn, and a
/// turn, place or camera that is not a number. Its report is
/// `local/icon_sweep.txt` (or the file `TRP_ICON_SWEEP` names). Bound
/// to the install and long (minutes): `cargo test -p trp-app --
/// --ignored every_picture_is_made_ready --nocapture`.
#[test]
#[ignore]
fn every_picture_is_made_ready() {
    use bevy::log::tracing_subscriber::layer::SubscriberExt;
    let Ok(install) = trp_data::Install::locate() else {
        eprintln!("skipped: no install configured (set SPT_DIR or local/spt_dir.txt)");
        return;
    };
    let db = trp_data::Database::load(&install).expect("database");
    // What is asked: each key once, with its cells and where it came from.
    let mut asked: Vec<(String, [u32; 2], String)> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut no_model: Vec<String> = Vec::new();
    let mut not_items: Vec<String> = Vec::new();
    let mut add = |item: &Item, from: String, asked: &mut Vec<(String, [u32; 2], String)>| {
        let key = picture_key(item);
        if seen.insert(key.clone()) {
            asked.push((key, item.cells(), from));
        }
    };
    let mut templates: Vec<&trp_data::setup::database::ItemTemplate> = db.items.values().filter(|item| item.node_type == "Item").collect();
    templates.sort_by(|a, b| a.prefab_key().cmp(&b.prefab_key()).then(a.id.cmp(&b.id)));
    for template in &templates {
        if template.prefab_key().is_none() && !db.outfits.contains_key(&template.id) {
            no_model.push(format!("{} {}", template.id, template.name));
            continue;
        }
        let carried = trp_data::character::inventory::Loadout::of_preset(&db, &template.id).and_then(|list| Item::of_list(&db, &list)).map(|(item, _)| item);
        match Kind::of(&db, &template.id) {
            Ok(kind) => {
                if let Ok(item) = &carried {
                    add(item, "as carried".into(), &mut asked);
                }
                add(&Item::new(Arc::new(kind)), "bare".into(), &mut asked);
            }
            Err(e) => not_items.push(format!("{} {}: {e:#}", template.id, template.name)),
        }
    }
    for preset in &db.presets {
        match Item::of_list(&db, &trp_data::character::inventory::Loadout::of_build(preset)) {
            Ok((item, _)) => add(&item, format!("build {} {}", preset.id, preset.name), &mut asked),
            Err(e) => not_items.push(format!("build {} {}: {e:#}", preset.id, preset.name)),
        }
    }
    let only: Option<HashSet<String>> = std::env::var("TRP_ICON_SWEEP_ONLY").ok().map(|list| list.split(',').map(str::to_owned).collect());
    if let Some(only) = &only {
        asked.retain(|(key, ..)| only.contains(split_key(key).template));
    }
    println!("{} pictures to make ready, {} items with no model, {} not made into items", asked.len(), no_model.len(), not_items.len());
    // Each thread a worker of its own, its bundles let go every so many
    // items; batches taken in the order of their prefabs, which share
    // bundles.
    struct Outcome {
        key: String,
        cells: [u32; 2],
        from: String,
        made: Result<String, String>,
        notes: Vec<String>,
    }
    let next = std::sync::atomic::AtomicUsize::new(0);
    let outcomes = std::sync::Mutex::new(Vec::<Outcome>::new());
    let threads = std::env::var("TRP_ICON_SWEEP_THREADS").ok().and_then(|n| n.parse().ok()).unwrap_or(6usize);
    // The log's lower lines kept too where they have this word (to see
    // that they are heard: `bounds`, every model's).
    let hear = std::env::var("TRP_ICON_SWEEP_HEAR").ok();
    const BATCH: usize = 16;
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| {
                let heard = Heard::default();
                let subscriber = bevy::log::tracing_subscriber::registry().with(heard.clone());
                bevy::log::tracing::subscriber::with_default(subscriber, || {
                    let mut worker = Worker::new(&install, std::sync::Arc::new(trp_data::Database::load(&install).expect("database"))).expect("the icon thread's worker").for_pictures();
                    let mut since = 0;
                    loop {
                        let from = next.fetch_add(BATCH, std::sync::atomic::Ordering::Relaxed);
                        if from >= asked.len() {
                            break;
                        }
                        for (key, cells, origin) in &asked[from..(from + BATCH).min(asked.len())] {
                            if since >= 96 {
                                worker.unity = reader(&install).expect("the install's bundles");
                                since = 0;
                            }
                            since += 1;
                            heard.0.lock().unwrap().clear();
                            // Drawn as a weapon's container where the classes taken by the
                            // item's class alone would not have drawn it so (or the other way).
                            let template = split_key(key).template;
                            let changed = worker.db.items.get(template).filter(|item| !worker.db.is_a(item, "Weapon")).and_then(|item| {
                                let by_class = ["ThrowWeap", "Knife", "PortableRangeFinder"].iter().any(|class| worker.db.is_a(item, class));
                                let prefab = item.prefab_key()?.to_owned();
                                let by_container = trp_data::weapons::weapon::is_weapon_container(&mut worker.unity, &prefab).ok()?;
                                (by_class != by_container).then(|| format!("drawn as a weapon's container: {by_container}, by its class before: {by_class}"))
                            });
                            let made = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match worker.own_picture(key, *cells) {
                                Ok(Some(([w, h], ..))) => Ok((format!("its own picture {w}x{h}"), Vec::new())),
                                Ok(None) => worker.prepare(key, *cells).map(|made| match made {
                                    Made::Own([w, h], ..) => (format!("an empty picture {w}x{h}: no renderer on"), Vec::new()),
                                    Made::Model(prepared) => {
                                        let mut notes = Vec::new();
                                        let drawn = prepared.parts.iter().map(Vec::len).sum::<usize>() + prepared.extra.len();
                                        if drawn == 0 {
                                            notes.push("framed, but no mesh is drawn".to_owned());
                                        }
                                        if !prepared.turn.is_finite() || !prepared.place.is_finite() || !prepared.half_height.is_finite() || prepared.half_height <= 0.0 {
                                            notes.push(format!("not a number: turn {} place {} half height {}", prepared.turn, prepared.place, prepared.half_height));
                                        }
                                        // A turn of no length on a node drawn (no turn since
                                        // `convert::rotation` takes it so; not a number before).
                                        let still: Vec<&str> = prepared.hierarchy.nodes.iter().filter(|node| node.rotation.iter().all(|c| *c == 0.0)).map(|node| node.name.as_str()).collect();
                                        if !still.is_empty() {
                                            notes.push(format!("a node's turn of no length: {}", still.join(", ")));
                                        }
                                        (format!("a model of {drawn} meshes, its camera's half height {:.4}", prepared.half_height), notes)
                                    }
                                }),
                                Err(e) => Err(e),
                            }));
                            let made = match made {
                                Ok(Ok((said, notes))) => (Ok(said), notes),
                                Ok(Err(e)) => (Err(format!("{e:#}")), Vec::new()),
                                Err(panic) => (Err(format!("panicked: {}", panic.downcast_ref::<String>().cloned().or_else(|| panic.downcast_ref::<&str>().map(|s| (*s).to_owned())).unwrap_or_default())), Vec::new()),
                            };
                            let (made, mut notes) = made;
                            notes.extend(changed);
                            for (level, words) in heard.0.lock().unwrap().drain(..) {
                                if level <= bevy::log::Level::INFO || words.contains("left out") || hear.as_deref().is_some_and(|word| words.contains(word)) {
                                    notes.push(format!("{level}: {words}"));
                                }
                            }
                            outcomes.lock().unwrap().push(Outcome { key: key.clone(), cells: *cells, from: origin.clone(), made, notes });
                        }
                        let done = next.load(std::sync::atomic::Ordering::Relaxed).min(asked.len());
                        if done % 320 < BATCH {
                            println!("{done} of {}", asked.len());
                        }
                    }
                });
            });
        }
    });
    let mut outcomes = outcomes.into_inner().unwrap();
    outcomes.sort_by(|a, b| a.key.cmp(&b.key));
    // The counts by reason: each failure's, each note's.
    let mut reasons: std::collections::BTreeMap<String, (usize, Vec<String>)> = std::collections::BTreeMap::new();
    let mut count = |reason: String, key: &str| {
        let entry = reasons.entry(reason).or_default();
        entry.0 += 1;
        if entry.1.len() < 12 {
            entry.1.push(key.to_owned());
        }
    };
    let (mut made, mut own, mut empty) = (0, 0, 0);
    for outcome in &outcomes {
        match &outcome.made {
            Ok(said) if said.starts_with("its own") => own += 1,
            Ok(said) if said.starts_with("an empty") => {
                empty += 1;
                count("empty picture: no renderer on".to_owned(), &outcome.key);
            }
            Ok(_) => made += 1,
            Err(e) => count(format!("not made: {}", reason_of(e)), &outcome.key),
        }
        for note in outcome.notes.iter().collect::<std::collections::BTreeSet<_>>() {
            count(format!("note: {}", reason_of(note)), &outcome.key);
        }
    }
    let name = |key: &str| db.items.get(split_key(key).template).map_or("?", |item| item.name.as_str()).to_owned();
    let mut report = String::new();
    use std::fmt::Write as _;
    let _ = writeln!(report, "Every item's picture made ready as the app makes it (icons, every_picture_is_made_ready), not drawn.");
    let _ = writeln!(report, "{} asked: {made} models made ready, {own} pictures of their own, {empty} empty pictures, {} not made; {} items with no model; {} not made into items.\n", outcomes.len(), outcomes.iter().filter(|o| o.made.is_err()).count(), no_model.len(), not_items.len());
    let _ = writeln!(report, "== By reason (count, then up to 12 keys)");
    for (reason, (n, keys)) in &reasons {
        let _ = writeln!(report, "{n:5}  {reason}");
        for key in keys {
            let _ = writeln!(report, "         {} ({})", key.chars().take(160).collect::<String>(), name(key));
        }
    }
    let _ = writeln!(report, "\n== Items with no model (no prefab)");
    for line in &no_model {
        let _ = writeln!(report, "  {line}");
    }
    let _ = writeln!(report, "\n== Not made into items");
    for line in &not_items {
        let _ = writeln!(report, "  {line}");
    }
    let _ = writeln!(report, "\n== Every key with a failure or a note");
    for outcome in outcomes.iter().filter(|o| o.made.is_err() || !o.notes.is_empty()) {
        let _ = writeln!(report, "{} ({}, {}, {:?} cells): {}", outcome.key, name(&outcome.key), outcome.from, outcome.cells, outcome.made.as_ref().map_or_else(|e| format!("NOT MADE: {e}"), Clone::clone));
        for note in &outcome.notes {
            let _ = writeln!(report, "    {note}");
        }
    }
    // Every key, one line each, to set two sweeps side by side.
    let _ = writeln!(report, "\n== Every key");
    for outcome in &outcomes {
        let _ = writeln!(report, "{} ({}): {}", outcome.key, name(&outcome.key), outcome.made.as_ref().map_or_else(|e| format!("NOT MADE: {e}"), Clone::clone));
    }
    let path = std::env::var_os("TRP_ICON_SWEEP").map(PathBuf::from).unwrap_or_else(|| {
        let here = std::env::current_dir().unwrap_or_default();
        here.ancestors().map(|dir| dir.join("local")).find(|local| local.is_dir()).unwrap_or_default().join("icon_sweep.txt")
    });
    std::fs::write(&path, &report).expect("the sweep's report");
    println!("{}", report.lines().take_while(|line| !line.starts_with("== Items with no model")).collect::<Vec<_>>().join("\n"));
    println!("written to {}", path.display());
}
