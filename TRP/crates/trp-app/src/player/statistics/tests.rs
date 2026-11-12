//! Tests of the experience a thing taken from the world gives.
use super::*;

/// `courses10`'s `examine-looted-item`: the keycard (`LootExperience`
/// 20) taken, "Looting experience - Items (10)" and "Searching
/// experience - Items (4)"; the install's shares 0.5, 0.15, 0.125.
#[test]
fn a_kind_counted_again_gives_less() {
    let attempts = [0.5, 0.15, 0.125];
    assert_eq!(experience(20, 1, &attempts, false, false), 10);
    assert_eq!(experience(20, 2, &attempts, false, false), 4, "the product in double precision");
    assert_eq!(experience(20, 3, &attempts, false, false), 3);
    assert_eq!(experience(20, 4, &attempts, false, false), 0, "nothing past the table");
    assert_eq!(experience(20, 1, &attempts, true, false), 20);
    assert_eq!(experience(0, 1, &attempts, true, false), 0);
    // SIMPLE XP GAIN: the whole of it every time.
    assert_eq!((experience(20, 1, &attempts, false, true), experience(20, 9, &attempts, false, true)), (20, 20));
}

fn thing(template: &str, stack_max: u32) -> Item {
    Item::new(std::sync::Arc::new(trp_sim::items::inventory::Kind { template: template.into(), stack_max, ..Default::default() }))
}

fn at(x: u32) -> trp_sim::items::inventory::Place {
    trp_sim::items::inventory::Place { x, y: 0, turned: false }
}

/// A thing whose grids are searchable (`SearchableItem`: a vest, a
/// backpack, the pockets).
fn searchable(template: &str) -> Item {
    let kind = trp_sim::items::inventory::Kind { template: template.into(), stack_max: 1, class_names: vec!["SearchableItem".into()], ..Default::default() };
    Item::new(std::sync::Arc::new(kind))
}

/// A thing looted, dropped and taken up again gives no more experience. The IFAK (20) from
/// the armory's medical case (known, never searched) into the rig:
/// looted alone, 10; put down and taken up again into the rig,
/// nothing. A rifle laid on the floor taken onto the back (a slot):
/// looted, not found; moved later into the backpack's grid, found
/// then; a box of rounds split into a magazine (not a searchable
/// place): looted alone; a bag's things still hidden: not found by
/// lying in its grid.
#[test]
fn a_thing_put_down_and_taken_up_again_gives_nothing_more() {
    let attempts = vec![0.5, 0.15, 0.125];
    let loot = |item: &Item| match item.kind.template.as_str() {
        "ifak" | "rifle" | "rounds" | "bandage" => 20,
        _ => 0,
    };
    let shares = Shares { attempts: attempts.clone(), elite: false, loot: &loot };
    let mut searcher = trp_sim::items::search::Searcher::default();
    let (mut rig, mut backpack, magazine) = (searchable("rig"), searchable("backpack"), thing("magazine", 1));
    rig.grids = vec![Vec::new()];
    backpack.grids = vec![Vec::new()];
    let mut equipment = thing("equipment", 1);
    equipment.slots = vec![Some(rig), Some(backpack), None, Some(magazine)];
    let ifak = thing("ifak", 1);
    let mut case = thing("medical case", 1);
    case.grids = vec![vec![(at(0), ifak.clone())]];
    let mut statistics = Statistics::default();
    statistics.look(&equipment, &case, false, true, &shares, &searcher);
    // Out of the case into the rig.
    case.grids[0].clear();
    equipment.slots[0].as_mut().unwrap().grids[0].push((at(0), ifak.clone()));
    statistics.look(&equipment, &case, false, true, &shares, &searcher);
    assert_eq!((statistics.looting, statistics.searching), (10, 0), "looted alone, as a searched container's");
    // Put down, then taken up into the rig again.
    equipment.slots[0].as_mut().unwrap().grids[0].clear();
    statistics.look(&equipment, &case, false, true, &shares, &searcher);
    statistics.count(&ifak, true, 20, &attempts, false);
    equipment.slots[0].as_mut().unwrap().grids[0].push((at(0), ifak));
    statistics.look(&equipment, &case, false, true, &shares, &searcher);
    assert_eq!((statistics.looting, statistics.searching), (10, 0), "nothing more");
    // A rifle off the floor onto the back: `taken`'s looting (10).
    let rifle = thing("rifle", 1);
    statistics.count(&rifle, true, 20, &attempts, false);
    equipment.slots[2] = Some(rifle.clone());
    statistics.look(&equipment, &case, false, true, &shares, &searcher);
    assert_eq!((statistics.looting, statistics.searching), (20, 0), "a slot is known: not found");
    // Moved from the back into the backpack's grid: found then (the
    // kind's second count, 3.0000001).
    equipment.slots[2] = None;
    equipment.slots[1].as_mut().unwrap().grids[0].push((at(0), rifle));
    statistics.look(&equipment, &case, false, true, &shares, &searcher);
    assert_eq!((statistics.looting, statistics.searching), (20, 4));
    // A round split off into the magazine: looted, not found.
    equipment.slots[3].as_mut().unwrap().loose.push(thing("rounds", 60));
    statistics.look(&equipment, &case, false, true, &shares, &searcher);
    assert_eq!((statistics.looting, statistics.searching), (30, 4));
    // A bag from beside whose grid is still hidden, put on the back:
    // its bandage stays unfound until the bag is searched.
    let mut bag = searchable("bag");
    bag.grids = vec![vec![(at(0), thing("bandage", 1))]];
    searcher.hide_thing(&bag);
    let mut crate_ = thing("crate", 1);
    crate_.grids = vec![vec![(at(0), bag.clone())]];
    statistics.look(&equipment, &crate_, false, true, &shares, &searcher);
    crate_.grids[0].clear();
    equipment.slots[2] = Some(bag);
    let before = statistics.searching;
    statistics.look(&equipment, &crate_, false, true, &shares, &searcher);
    assert_eq!(statistics.searching, before, "hidden: not found by lying there");
}

/// `courses10`'s `search-and-loot`: the safe's teapot (100) found,
/// "Searching experience - Items (50)" (510, told 541); sent by Ctrl to
/// the backpack, "Looting experience - Items (16)" (751, told 767: the
/// kind's second count, 15.0000006), not found again; the money (0)
/// nothing. Then what the take does not show: a split's part looted
/// and found, a round chambered nothing, a body's gear found at once
/// and its things then looted alone.
#[test]
fn a_thing_found_and_then_moved_in_is_counted_once_each_way() {
    let attempts = vec![0.5, 0.15, 0.125];
    let loot = |item: &Item| match item.kind.template.as_str() {
        "teapot" => 100,
        "flare" => 20,
        "helmet" => 40,
        _ => 0,
    };
    let shares = Shares { attempts: attempts.clone(), elite: false, loot: &loot };
    let searcher = trp_sim::items::search::Searcher::default();
    let mut backpack = searchable("backpack");
    backpack.grids = vec![Vec::new()];
    let mut equipment = thing("equipment", 1);
    equipment.slots = vec![Some(backpack), Some(thing("rifle", 1))];
    let (teapot, money) = (thing("teapot", 1), thing("money", 500_000));
    let mut safe = thing("safe", 1);
    safe.grids = vec![vec![(at(0), teapot.clone()), (at(1), money.clone())]];
    let mut statistics = Statistics::default();
    assert_eq!(statistics.look(&equipment, &safe, false, true, &shares, &searcher), None);
    assert_eq!((statistics.looting, statistics.searching), (0, 0), "the run's own counts for nothing");
    // The search's two finds (`Statistics::found`).
    statistics.count(&teapot, false, 100, &attempts, false);
    statistics.count(&money, false, 0, &attempts, false);
    assert_eq!(statistics.searching, 50);
    // Each sent to the backpack: out of the safe, into its grid.
    for (n, sent) in [teapot, money].into_iter().enumerate() {
        safe.grids[0].retain(|(_, item)| item.id != sent.id);
        equipment.slots[0].as_mut().unwrap().grids[0].push((at(n as u32), sent));
        statistics.look(&equipment, &safe, false, true, &shares, &searcher);
    }
    assert_eq!((statistics.looting, statistics.searching), (16, 50));
    // A part of a flare stack split off into the backpack: a new
    // thing, looted and found (its first and second counts, 10 and 4:
    // 3.0000001).
    equipment.slots[0].as_mut().unwrap().grids[0].push((at(2), thing("flare", 20)));
    statistics.look(&equipment, &safe, false, true, &shares, &searcher);
    assert_eq!((statistics.looting, statistics.searching), (26, 54));
    // A flare chambered by the hands (a stack in a slot): nothing.
    equipment.slots[1].as_mut().unwrap().slots = vec![Some(thing("flare", 20))];
    statistics.look(&equipment, &safe, false, true, &shares, &searcher);
    assert_eq!((statistics.looting, statistics.searching), (26, 54));
    // A body's gear shown: every item found at once, the gear first;
    // its helmet then sent over is looted alone.
    let helmet = thing("helmet", 1);
    let mut body = thing("equipment", 1);
    body.slots = vec![Some(helmet.clone())];
    assert_eq!(statistics.look(&equipment, &body, true, true, &shares, &searcher), Some((2, 20)));
    assert_eq!(statistics.look(&equipment, &body, true, true, &shares, &searcher), None, "once");
    statistics.count(&helmet, false, 40, &attempts, false);
    assert_eq!(statistics.searching, 74, "its search's find gives nothing more");
    body.slots[0] = None;
    equipment.slots[0].as_mut().unwrap().grids[0].push((at(3), helmet));
    statistics.look(&equipment, &body, true, true, &shares, &searcher);
    assert_eq!((statistics.looting, statistics.searching), (33, 74), "40 x 0.15: 6.0000002");
}
