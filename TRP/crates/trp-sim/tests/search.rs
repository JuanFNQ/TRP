//! Tests of searching a container, its pace and its order.
//! After `docs/spec/inventory.md`, "Searching a container": the two seconds a container never searched takes to open,
//! each thing a second or two over the pace, in the order it lies, the
//! elite's luck, a search stopped and taken up again, one at a time.

use std::sync::Arc;

use trp_data::character::inventory::Holder;
use trp_data::level::loot::LootTable;
use trp_sim::items::inventory::{quick_find, Item, Kind, Order, Place, Rules, EQUIPMENT_TEMPLATE};
use trp_sim::items::search::{draw, draw_loot, find_time, on_body, scav_body, seen_on_body, Dice, Event, FillRules, Refused, Searcher, Skills, OPEN_TIME};

fn thing(name: &str) -> Item {
    Item::new(Arc::new(Kind { template: name.into(), name: name.into(), cells: [1, 1], ..Kind::default() }))
}

/// A crate of one grid, eight across, holding so many things in a row.
fn crate_of(things: usize) -> Item {
    let grid = Holder { name: "main".into(), cells: [8, 8], ..Holder::default() };
    let mut container = Item::new(Arc::new(Kind { template: "crate".into(), name: "crate".into(), cells: [5, 5], grids: vec![grid], ..Kind::default() }));
    for n in 0..things {
        container.put(0, thing(&format!("thing{n}")), Place { x: n as u32, y: 0, turned: false }).unwrap();
    }
    container
}

/// The searcher run on to `until` in steps of a frame, each event with the
/// time it was seen at.
fn run(searcher: &mut Searcher, container: &Item, from: f64, until: f64) -> Vec<(f64, Event)> {
    let mut seen = Vec::new();
    let mut now = from;
    while now <= until {
        for event in searcher.advance(now, |id| (id == container.id).then_some(container)) {
            seen.push((now, event));
        }
        now += 1.0 / 1000.0;
    }
    seen
}

#[test]
fn a_crate_opens_after_two_seconds_and_gives_up_its_things_one_by_one() {
    let container = crate_of(3);
    let mut searcher = Searcher::new(7);
    searcher.hide(&container);
    assert!(!searcher.searched(container.id));
    assert!(container.grids[0].iter().all(|(_, item)| !searcher.known(item.id)));
    let skills = Skills::default();
    assert_eq!(searcher.begin(&container, false, &skills, 10.0), Ok(vec![]));
    let seen = run(&mut searcher, &container, 10.0, 20.0);
    // Opened at 12 s, not before.
    let opened = seen.iter().find(|(_, event)| matches!(event, Event::Opened(_))).map(|(at, _)| *at).unwrap();
    assert!((opened - (10.0 + OPEN_TIME)).abs() < 0.002, "opened at {opened}");
    // Each thing in the order it lies, a second or two after the last.
    let found: Vec<(f64, u64)> = seen.iter().filter_map(|(at, event)| if let Event::Found { item, sounded, .. } = event { assert!(*sounded); Some((*at, *item)) } else { None }).collect();
    let order: Vec<u64> = container.grids[0].iter().map(|(_, item)| item.id).collect();
    assert_eq!(found.iter().map(|(_, id)| *id).collect::<Vec<_>>(), order);
    let mut last = opened;
    for (at, _) in &found {
        let gap = at - last;
        assert!((gap - 1.0).abs() < 0.002 || (gap - 2.0).abs() < 0.002, "a gap of {gap}");
        last = *at;
    }
    // Ended with the last, and every thing known.
    assert!(matches!(seen.last(), Some((at, Event::Ended(_))) if (*at - last).abs() < 1e-9));
    assert!(searcher.under_way.is_empty());
    assert!(searcher.searched(container.id) && !searcher.holds_unknown(&container));
}

/// A search made on a frame runs on the next (`ExecuteAsync`'s
/// `Task.Yield`), its wait to open counted from there, its timer from when
/// it was made: `courses10`'s `dead-scav-search-a`, made on frame 323
/// (2.9599 s, the panel's first), run on 324 (2.9765), opened on 559
/// (4.9835), not on 557 (4.9668) nor 558 (4.9751).
#[test]
fn a_search_runs_on_the_frame_after_it_is_made() {
    let container = crate_of(2);
    let mut searcher = Searcher::new(7);
    searcher.hide(&container);
    assert_eq!(searcher.begin(&container, false, &Skills { loot_speed: 0.8, ..Skills::default() }, 2.9599), Ok(vec![]));
    let mut opened = None;
    for now in [2.9599, 2.9765, 2.9857, 4.9668, 4.9751, 4.9835, 4.9954] {
        for event in searcher.advance(now, |id| (id == container.id).then_some(&container)) {
            if event == Event::Opened(container.id) && opened.is_none() {
                opened = Some(now);
            }
        }
    }
    assert_eq!(opened, Some(4.9835));
    assert_eq!(searcher.searching(container.id).map(|search| search.began), Some(2.9599));
}

/// Each wait is counted from the frame the last step was seen on, so a
/// search runs late by a part of a frame a step and never early
/// (`SearchContent`'s `Task.Delay` begun as the last resumed): `courses10`'s
/// `airdrop-crate-search`, at about 100 frames a second (10.05 ms) and
/// Attention 40, has every gap 2 to 10 ms over its 555 or 1111 ms, 0.16 s
/// over 22 finds.
#[test]
fn each_wait_is_counted_from_the_frame_the_last_step_landed() {
    let container = crate_of(8);
    let mut searcher = Searcher::new(4);
    searcher.hide(&container);
    let skills = Skills { loot_speed: 0.8, ..Skills::default() };
    let frame = 0.01005;
    searcher.begin(&container, false, &skills, 0.0).unwrap();
    let mut seen = Vec::new();
    let mut now = 0.0;
    while now < 20.0 && !searcher.under_way.is_empty() {
        now += frame;
        for event in searcher.advance(now, |id| (id == container.id).then_some(&container)) {
            seen.push((now, event));
        }
    }
    let steps: Vec<f64> = seen.iter().filter(|(_, event)| matches!(event, Event::Opened(_) | Event::Found { .. })).map(|(at, _)| *at).collect();
    assert_eq!(steps.len(), 9);
    // Its wait to open from the frame after it began (`Task.Yield`).
    assert!(steps[0] >= OPEN_TIME + frame && steps[0] < OPEN_TIME + 2.0 * frame + 1e-9, "opened at {}", steps[0]);
    let mut late = steps[0] - OPEN_TIME - frame;
    for pair in steps.windows(2) {
        let gap = pair[1] - pair[0];
        let wait = if gap < 1.0 { 0.555 } else { 1.111 };
        assert!(gap >= wait - 1e-9 && gap < wait + frame + 1e-9, "a gap of {gap}");
        late += gap - wait;
    }
    // Late by about half a frame a step, all of it kept.
    assert!(late > 0.02 && late < 9.0 * frame, "late by {late}");
}

#[test]
fn attention_makes_each_thing_quicker() {
    let mut dice = Dice::new(3);
    // Level 50 of Attention, 0.02 a level: twice as fast, half a second or
    // a whole one each.
    for _ in 0..200 {
        let wait = find_time(&mut dice, 2.0, false);
        assert!(wait == 0.5 || wait == 1.0, "{wait}");
    }
    // Waited in whole milliseconds, cut short: 1 / 1.5 is 0.666.
    let mut seen = std::collections::HashSet::new();
    for _ in 0..200 {
        seen.insert((find_time(&mut dice, 1.5, false) * 1000.0).round() as u32);
    }
    assert_eq!(seen, [666, 1333].into_iter().collect());
    assert_eq!(find_time(&mut dice, 1.0, true), 0.0);
}

#[test]
fn the_elite_luck_finds_everything_at_once_half_the_time() {
    let skills = Skills { lucky: 0.5, ..Skills::default() };
    let mut searcher = Searcher::new(11);
    let mut instant = 0;
    for _ in 0..1000 {
        let container = crate_of(2);
        searcher.hide(&container);
        assert_eq!(searcher.begin(&container, false, &skills, 0.0), Ok(vec![]));
        // Nothing on the frame it is made; it runs on the next.
        assert!(searcher.advance(0.0, |id| (id == container.id).then_some(&container)).is_empty());
        let run = searcher.advance(0.01, |id| (id == container.id).then_some(&container));
        if run.first() == Some(&Event::Lucky(container.id)) {
            instant += 1;
            assert_eq!(run.get(1), Some(&Event::Opened(container.id)));
            // All of it found as it runs, with no sound.
            assert_eq!(run.iter().filter(|event| matches!(event, Event::Found { sounded: false, .. })).count(), 2);
            assert!(matches!(run.last(), Some(Event::Ended(_))));
        } else {
            searcher.stop(container.id);
        }
    }
    assert!((440..=560).contains(&instant), "{instant} of 1000 at once");
    // Below the elite, never.
    let container = crate_of(1);
    searcher.hide(&container);
    assert_eq!(searcher.begin(&container, false, &Skills::default(), 0.0), Ok(vec![]));
}

#[test]
fn a_search_stopped_is_taken_up_where_it_was() {
    let container = crate_of(3);
    let mut searcher = Searcher::new(5);
    searcher.hide(&container);
    let skills = Skills::default();
    // Stopped while it opens: still unsearched, and the next waits again.
    searcher.begin(&container, false, &skills, 0.0).unwrap();
    run(&mut searcher, &container, 0.0, 1.5);
    assert!(searcher.stop(container.id));
    assert!(!searcher.searched(container.id));
    searcher.begin(&container, false, &skills, 5.0).unwrap();
    let seen = run(&mut searcher, &container, 5.0, 7.5);
    let opened = seen.iter().find(|(_, event)| matches!(event, Event::Opened(_))).map(|(at, _)| *at).unwrap();
    assert!((opened - 7.0).abs() < 0.002);
    // Stopped once open with things left: taken up with no wait to open,
    // the next found a second or two in.
    searcher.stop(container.id);
    let left = container.grids[0].iter().filter(|(_, item)| !searcher.known(item.id)).count();
    assert!(left > 0);
    assert_eq!(searcher.begin(&container, false, &skills, 20.0), Ok(vec![]));
    let seen = run(&mut searcher, &container, 20.0, 30.0);
    assert!(!seen.iter().any(|(_, event)| matches!(event, Event::Opened(_))));
    let first = seen.iter().find(|(_, event)| matches!(event, Event::Found { .. })).map(|(at, _)| *at).unwrap();
    assert!((first - 21.0).abs() < 0.002 || (first - 22.0).abs() < 0.002, "first found at {first}");
    assert!(!searcher.holds_unknown(&container));
}

#[test]
fn an_empty_crate_opens_and_ends() {
    let container = crate_of(0);
    let mut searcher = Searcher::new(2);
    searcher.hide(&container);
    searcher.begin(&container, false, &Skills::default(), 0.0).unwrap();
    let seen = run(&mut searcher, &container, 0.0, 3.0);
    let kinds: Vec<&Event> = seen.iter().map(|(_, event)| event).collect();
    assert_eq!(kinds, vec![&Event::Opened(container.id), &Event::Ended(container.id)]);
    // A crate not hidden is searched from the start: a search of it has
    // nothing to find and ends at once.
    let open = crate_of(2);
    assert!(searcher.searched(open.id) && !searcher.holds_unknown(&open));
}

#[test]
fn one_search_at_a_time_two_at_the_search_elite() {
    let (one, two, three) = (crate_of(1), crate_of(1), crate_of(1));
    let mut searcher = Searcher::new(9);
    for container in [&one, &two, &three] {
        searcher.hide(container);
    }
    let plain = Skills::default();
    searcher.begin(&one, false, &plain, 0.0).unwrap();
    assert_eq!(searcher.begin(&one, false, &plain, 0.0), Err(Refused::Searching));
    assert_eq!(searcher.begin(&two, false, &plain, 0.0), Err(Refused::Busy));
    let elite = Skills { double: true, ..Skills::default() };
    assert!(searcher.begin(&two, false, &elite, 0.0).is_ok());
    assert_eq!(searcher.begin(&three, false, &elite, 0.0), Err(Refused::Busy));
    // On a body, Search's speed is added to Attention's.
    let skills = Skills { loot_speed: 0.5, body_speed: 0.25, ..Skills::default() };
    assert_eq!(skills.speed(false), 1.5);
    assert_eq!(skills.speed(true), 1.75);
}

#[test]
fn the_loot_is_drawn_by_its_weights() {
    // The first whose running share reaches the roll.
    let list = [("a", 1.0), ("b", 3.0)];
    assert_eq!(draw(&list, 0.0), Some(&"a"));
    assert_eq!(draw(&list, 0.25), Some(&"a"));
    assert_eq!(draw(&list, 0.26), Some(&"b"));
    assert_eq!(draw(&list, 0.999), Some(&"b"));
    // So many as the count drawn, none of what is left out.
    let table = LootTable { counts: vec![(3, 1.0)], items: vec![("kept".into(), 1.0), ("out".into(), 100.0)] };
    let rules = FillRules { multiplier: 1.0, fit_attempts: 3, left_out: ["out".to_owned()].into_iter().collect(), ..FillRules::default() };
    let mut dice = Dice::new(4);
    for _ in 0..50 {
        assert_eq!(draw_loot(&table, &rules, &mut dice), vec!["kept".to_owned(); 3]);
    }
    // Halves to the even, as .NET rounds: 2.5 is 2.
    let rules = FillRules { multiplier: 0.5, ..rules };
    let table = LootTable { counts: vec![(5, 1.0)], ..table };
    assert_eq!(draw_loot(&table, &rules, &mut dice).len(), 2);
}

/// A kind of the game's searchable class (`SearchableItem`), of one grid.
fn searchable_of(name: &str, cells: [u32; 2]) -> Item {
    let grid = Holder { name: "main".into(), cells, ..Holder::default() };
    Item::new(Arc::new(Kind { template: name.into(), name: name.into(), cells: [2, 2], class_names: vec!["SearchableItem".into()], grids: vec![grid], ..Kind::default() }))
}

/// An equipment (the game's template) with a few of its slots.
fn equipment_with(slots: &[&str]) -> Item {
    let holders = slots.iter().map(|name| Holder { name: (*name).into(), ..Holder::default() }).collect();
    Item::new(Arc::new(Kind { template: EQUIPMENT_TEMPLATE.into(), name: "Equipment".into(), slots: holders, ..Kind::default() }))
}

#[test]
fn a_body_hides_what_its_containers_hold_not_what_it_wears() {
    let mut body = equipment_with(&["Headwear", "TacticalVest", "Backpack"]);
    let helmet = thing("helmet");
    let mut vest = searchable_of("vest", [4, 2]);
    let round = thing("round");
    let mut pouch = searchable_of("pouch", [1, 1]);
    let inside = thing("inside");
    pouch.put(0, inside.clone(), Place { x: 0, y: 0, turned: false }).unwrap();
    vest.put(0, round.clone(), Place { x: 0, y: 0, turned: false }).unwrap();
    let pouch_id = pouch.id;
    vest.put(0, pouch, Place { x: 2, y: 0, turned: false }).unwrap();
    let vest_id = vest.id;
    body.slots[0] = Some(helmet.clone());
    body.slots[1] = Some(vest);
    let mut searcher = Searcher::new(3);
    searcher.hide_body(&body);
    // What is worn is known: a slot hides nothing (`IsItemKnown`).
    assert!(searcher.known(helmet.id) && searcher.known(vest_id));
    // The vest is not searched, what lies in it is not found, and a
    // searchable thing in it is unsearched as well, what it holds unknown.
    assert!(!searcher.searched(vest_id));
    assert!(!searcher.known(round.id) && !searcher.known(pouch_id));
    assert!(!searcher.searched(pouch_id) && !searcher.known(inside.id));
    // The body itself is no container that is searched.
    assert!(searcher.searched(body.id));
    // Forgotten as it is dressed afresh.
    searcher.forget(&body);
    assert!(!searcher.hides_anything());
}

#[test]
fn a_bag_in_a_crate_is_found_then_searched_on_its_own() {
    let mut container = crate_of(0);
    let mut bag = searchable_of("bag", [2, 2]);
    let note = thing("note");
    bag.put(0, note.clone(), Place { x: 0, y: 0, turned: false }).unwrap();
    let bag_id = bag.id;
    container.put(0, bag, Place { x: 0, y: 0, turned: false }).unwrap();
    let mut searcher = Searcher::new(5);
    searcher.hide(&container);
    assert!(!searcher.known(bag_id) && !searcher.searched(bag_id) && !searcher.known(note.id));
    let skills = Skills::default();
    searcher.begin(&container, false, &skills, 0.0).unwrap();
    run(&mut searcher, &container, 0.0, 6.0);
    // Found where it lies, and still to be opened (`_searchedItems` holds
    // only what was searched, or one's own).
    assert!(searcher.known(bag_id) && !searcher.searched(bag_id) && !searcher.known(note.id));
    let bag = container.find(bag_id).unwrap().clone();
    searcher.begin(&bag, false, &skills, 10.0).unwrap();
    run(&mut searcher, &bag, 10.0, 15.0);
    assert!(searcher.searched(bag_id) && searcher.known(note.id));
}

#[test]
fn search_quickens_only_a_container_on_a_body() {
    // Level 100 of Search (0.01 a level) and no Attention: twice as fast on
    // a body, as fast as ever elsewhere.
    let skills = Skills { body_speed: 1.0, ..Skills::default() };
    assert_eq!(skills.speed(true), 2.0);
    assert_eq!(skills.speed(false), 1.0);
    let body = equipment_with(&["TacticalVest"]);
    assert!(on_body(&body) && !on_body(&crate_of(1)));
    // Each thing a half or a whole second after the last, from the open.
    let mut vest = searchable_of("vest", [4, 1]);
    for n in 0..3 {
        vest.put(0, thing(&format!("t{n}")), Place { x: n, y: 0, turned: false }).unwrap();
    }
    let mut searcher = Searcher::new(9);
    searcher.hide(&vest);
    searcher.begin(&vest, true, &skills, 0.0).unwrap();
    let seen = run(&mut searcher, &vest, 0.0, 6.0);
    let mut last = seen.iter().find(|(_, event)| matches!(event, Event::Opened(_))).map(|(at, _)| *at).unwrap();
    assert!((last - OPEN_TIME).abs() < 0.002);
    let mut found = 0;
    for (at, event) in &seen {
        if matches!(event, Event::Found { .. }) {
            let gap = at - last;
            assert!((gap - 0.5).abs() < 0.002 || (gap - 1.0).abs() < 0.002, "a gap of {gap}");
            last = *at;
            found += 1;
        }
    }
    assert_eq!(found, 3);
}

#[test]
fn a_scav_body_has_neither_dog_tag_nor_armband() {
    let mut player = equipment_with(&["Dogtag", "ArmBand", "Headwear", "SecuredContainer"]);
    player.slots[0] = Some(thing("tag"));
    player.slots[1] = Some(thing("band"));
    player.slots[2] = Some(thing("helmet"));
    let body = scav_body(&player);
    assert!(body.slots[0].is_none() && body.slots[1].is_none());
    assert_eq!(body.slots[2].as_ref().map(|item| item.kind.name.as_str()), Some("helmet"));
    // Numbers of its own, the player's untouched.
    assert_ne!(body.id, player.id);
    assert_ne!(body.slots[2].as_ref().unwrap().id, player.slots[2].as_ref().unwrap().id);
    assert!(player.slots[0].is_some());
    // What a looter sees: no armband slot on a Scav, never the secured
    // container; a Scav's dog tag slot shows, empty.
    assert!(!seen_on_body("ArmBand", true) && seen_on_body("ArmBand", false));
    assert!(!seen_on_body("SecuredContainer", true) && !seen_on_body("SecuredContainer", false));
    assert!(seen_on_body("Dogtag", true));
}

/// Sent toward a body with Ctrl and a click, a thing goes into none of its
/// `MobContainer`s (`ItemManipulator`'s quick find, the mover not the
/// equipment's owner): money, which looks in the secured container first,
/// goes to the backpack. Toward one's own, the secured container takes it.
#[test]
fn a_body_takes_nothing_into_its_secured_container() {
    let secured_of = || {
        let grid = Holder { name: "main".into(), cells: [2, 2], ..Holder::default() };
        Item::new(Arc::new(Kind { template: "secured".into(), name: "secured".into(), cells: [1, 1], class_names: vec!["MobContainer".into(), "SearchableItem".into()], grids: vec![grid], ..Kind::default() }))
    };
    let money = || Item::new(Arc::new(Kind { template: "roubles".into(), name: "roubles".into(), cells: [1, 1], class_names: vec!["Money".into()], ..Kind::default() }));
    let mut own = equipment_with(&["SecuredContainer", "Backpack"]);
    let mut body = equipment_with(&["SecuredContainer", "Backpack"]);
    let mut own_bag = searchable_of("own bag", [2, 2]);
    let cash = money();
    let cash_id = cash.id;
    own_bag.put(0, cash, Place { x: 0, y: 0, turned: false }).unwrap();
    own.slots[0] = Some(secured_of());
    own.slots[1] = Some(own_bag);
    body.slots[0] = Some(secured_of());
    body.slots[1] = Some(searchable_of("body bag", [2, 2]));
    let (body_secured, body_bag) = (body.slots[0].as_ref().unwrap().id, body.slots[1].as_ref().unwrap().id);
    let rules = Rules { own: Some(own.id), ..Rules { in_raid: true, backpacks_deep: 0, busy_weapon: None, chamber_closed: None, chamber_held: None, limits: None, own: None } };
    let body_id = body.id;
    let (into, _) = quick_find(&mut [&mut own, &mut body], cash_id, &[body_id], Order::OTHER_SIDE, &rules).expect("it goes onto the body");
    assert_eq!(into, body_bag);
    assert_ne!(into, body_secured);
    // Back to one's own: its secured container is first for money.
    let own_id = own.id;
    let own_secured = own.slots[0].as_ref().unwrap().id;
    let (into, _) = quick_find(&mut [&mut own, &mut body], cash_id, &[own_id], Order::OTHER_SIDE, &rules).expect("it comes back");
    assert_eq!(into, own_secured);
}
