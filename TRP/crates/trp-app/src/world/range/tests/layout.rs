//! Tests of the range's layout, its level file and the benches' sight.

use super::*;

/// The range written as a level file and read back is the same range:
/// every box with its turn, colour and surface, every thing, start and
/// lamp, and the indoors. And a prop stood elsewhere and turned in the
/// file takes its solids, its things and its start with it.
#[test]
fn the_range_goes_through_a_level_file() {
    use crate::world::level::{Level, Stand};
    use std::sync::Arc;
    let range = Range::default();
    let text = serde_json::to_string(&crate::world::level::to_gltf(&range.layout())).unwrap();
    let mut warnings = Vec::new();
    let level = Level::of_gltf(&serde_json::from_str(&text).unwrap(), &mut |line| warnings.push(line));
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!((level.props.len(), level.lamps.len()), (range.props.len(), hall_lamps().len()));
    // (The range's own has no indoors since its hall went: none written.)
    match (indoors(), level.indoors.as_ref()) {
        (Some((centre, size, _)), Some(found)) => assert!(found.centre.distance(centre) < 1e-4 && found.size.distance(size) < 1e-4),
        (None, None) => {}
        (made, read) => panic!("the indoors {made:?} became {read:?}"),
    }
    // The sampler's squares, each by its surface.
    let squares = level.squares.as_ref().unwrap();
    assert_eq!(squares.len(), surface_squares().len());
    for (surface, found) in squares {
        let (made, read) = (surface_squares().iter().find(|square| square.surface == *surface).unwrap(), Square::of(*surface, found));
        assert!(made.centre.distance(read.centre) < 1e-4 && made.size.distance(read.size) < 1e-4 && made.yaw == read.yaw, "{made:?} and {read:?}");
    }
    let again = Range::of(Some(Arc::new(level.clone())));
    assert_eq!(again.solids.len(), range.solids.len());
    for (a, b) in range.solids.iter().zip(&again.solids) {
        let colour = a.color.to_srgba().to_u8_array().iter().zip(b.color.to_srgba().to_u8_array()).all(|(x, y)| x.abs_diff(y) <= 1);
        let same = a.centre.distance(b.centre) < 1e-4 && a.size.distance(b.size) < 1e-4 && (a.yaw - b.yaw).abs() < 1e-5;
        assert!(same && colour && a.surface == b.surface && a.overhead == b.overhead, "{} became {}", a.name, b.name);
    }
    assert_eq!(again.site.blocks.len(), range.site.blocks.len());
    for (a, b) in range.site.blocks.iter().zip(&again.site.blocks) {
        let near = (0..3).all(|k| (a.low[k] - b.low[k]).abs() < 1e-4 && (a.high[k] - b.high[k]).abs() < 1e-4);
        assert!(near && (a.turn - b.turn).abs() < 1e-5, "{a:?} became {b:?}");
    }
    for (a, b) in range.things.iter().zip(&again.things) {
        assert!(a.name == b.name && a.place == b.place && a.hit == b.hit && a.thing.hinge == b.thing.hinge);
    }
    for name in start_names() {
        let (a, b) = (range.place(&name).unwrap(), again.place(&name).unwrap());
        assert!((0..3).all(|k| (a[k] - b[k]).abs() < 1e-4), "{name}: {a:?} and {b:?}");
        let (a, b) = (range.start_view(&name).unwrap(), again.start_view(&name).unwrap());
        assert!(((a[0] - b[0] + 180.0).rem_euclid(360.0) - 180.0).abs() < 1e-3 && a[1] == b[1], "{name}: {a:?} and {b:?}");
    }

    // The rifles' table 6 m down the lanes and a quarter turned, its
    // start as it would go with it.
    let prop = range.props.iter().find(|prop| prop.name == "rifles table").unwrap();
    let anchor = prop.stand.at;
    let stand = Stand { at: anchor + Vec3::new(0.0, 0.0, -6.0), yaw: std::f32::consts::FRAC_PI_2 };
    let mut moved = level;
    moved.props.insert("rifles table".to_owned(), stand);
    moved.starts.remove("rifles");
    let turned = Range::of(Some(Arc::new(moved)));
    for at in prop.solids.clone() {
        let (was, now) = (&range.solids[at], &turned.solids[at]);
        assert!(now.centre.distance(stand.point(anchor, was.centre)) < 1e-4 && (now.yaw - stand.yaw).abs() < 1e-6);
    }
    for at in prop.things.clone() {
        let (was, now) = (&range.things[at], &turned.things[at]);
        let middle = |block: &Block| in_bevy(std::array::from_fn(|k| (block.low[k] + block.high[k]) / 2.0));
        assert!(middle(&now.hit).distance(stand.point(anchor, middle(&was.hit))) < 1e-4 && (now.hit.turn - (was.hit.turn - stand.yaw)).abs() < 1e-6);
    }
    let (was, now) = (range.place("rifles").unwrap(), turned.place("rifles").unwrap());
    assert!(in_bevy(now).distance(stand.point(anchor, in_bevy(was))) < 1e-4, "{was:?} to {now:?}");
    assert!((turned.start_view("rifles").unwrap()[0] - (range.start_view("rifles").unwrap()[0] - 90.0)).abs() < 1e-3);
    // The table's top is stood on where it now is, and not where it was.
    let top = stand.point(anchor, anchor);
    assert!(turned.ground_height(top.x, -top.z) >= WEAPON_TABLE.y);
    assert_eq!(turned.ground_height(anchor.x, -anchor.z), 0.0);
}

/// Each bench is in sight before its table, and not beside it nor
/// behind it: its zone ends at the table's ends (cut so that the interaction
/// boxes match the tables), where the game's triggers about the main
/// table, the armor bench's, reached 0.6 and 0.8 m past them.
#[test]
fn each_bench_is_in_sight_only_before_its_table() {
    let range = Range::default();
    let radius = 0.366;
    for kind in BenchKind::ALL {
        let bench = range.benches.iter().find(|bench| bench.kind == kind).expect("the bench stands");
        let t = kind.table();
        // Along its front, 0.6 m out, from its middle.
        let (a, b) = (kind.point(t.at(t.depth() + 0.6, t.low[1], 0.0)), kind.point(t.at(t.depth() + 0.6, t.low[1], t.length())));
        let (middle, along) = ((a + b) / 2.0, (b - a).normalize());
        let feet = |past: f32| Vec3::new(middle.x, 0.0, middle.z) + along * ((b - a).length() / 2.0 + past);
        assert!(bench.in_sight(feet(-t.length() / 2.0), radius), "{kind:?}: before the middle");
        assert!(bench.in_sight(feet(-0.1), radius), "{kind:?}: before its end");
        assert!(!bench.in_sight(feet(0.4 + radius), radius) && !bench.in_sight(feet(-t.length() - 0.4 - radius), radius), "{kind:?}: beside either end");
        // Not from behind it, nor from beyond its zone before it.
        let behind = kind.point(t.at(-0.5 - radius, t.low[1], t.length() / 2.0));
        let far = kind.before(BENCH_REACH + radius + 0.1);
        assert!(!bench.in_sight(Vec3::new(behind.x, 0.0, behind.z), radius) && !bench.in_sight(far, radius), "{kind:?}: behind or far");
        // Where its start stands, it is in sight.
        let (start, ..) = made_start(if kind == BenchKind::Gun { "workbench" } else { "armor-bench" }).unwrap();
        assert!(bench.in_sight(start, radius), "{kind:?}: from its start");
    }
    // The game's own triggers, about the main table where the armor
    // bench stands it, reached past its ends.
    let kind = BenchKind::Armor;
    let t = kind.table();
    let (a, b) = (kind.point(t.at(t.depth() + 0.6, t.low[1], 0.0)), kind.point(t.at(t.depth() + 0.6, t.low[1], t.length())));
    let past = |at: Vec3| {
        BENCH_TRIGGERS_GAME.iter().any(|(low, high)| {
            let (p, q) = (kind.point(*low), kind.point(*high));
            let game = Workbench { kind, place: Transform::IDENTITY, trigger: [p.min(q), p.max(q)] };
            game.in_sight(at, radius)
        })
    };
    let along = (b - a).normalize();
    assert!(past(b + along * (0.4 + radius)) || past(a - along * (0.4 + radius)));
}

/// The things of the row stand a pace apart and none in another; the
/// tables of weapons' cases and boxes are clear of one another, a case
/// over its own boxes, a table's two boxes side by side under it; the
/// lower platform reaches to the knee-high wall;
/// the workbench's table is against that wall, short of the first
/// table; and the ring is clear of everything.
#[test]
fn the_range_is_laid_out_as_drawn() {
    let range = Range::default();
    let on_tables = |placed: &Placed| placed.holds.is_some_and(|holds| (GUN_CASE_HOLDS..MORE_HOLDS).contains(&holds));
    // A table's magazine box and its box of rounds, by its place.
    let box_of = |placed: &Placed| {
        placed.holds.and_then(|holds| {
            let magazines = holds.checked_sub(MAGAZINE_BOX_HOLDS).filter(|at| *at < AMMO_TABLES);
            magazines.or_else(|| holds.checked_sub(AMMO_BOX_HOLDS).filter(|at| *at < AMMO_TABLES))
        })
    };
    let things: Vec<(Block, bool, Option<usize>)> = range.things.iter().filter(|p| p.leaf.is_none()).map(|p| (p.hit, on_tables(p), box_of(p))).collect();
    for (i, (a, a_table, a_box)) in things.iter().enumerate() {
        for (b, b_table, b_box) in &things[i + 1..] {
            let apart = |k: usize| (a.low[k] - b.high[k]).max(b.low[k] - a.high[k]);
            let least = if *a_table && *b_table { 0.2 } else { 0.5 };
            assert!(apart(0).max(apart(1)).max(apart(2)) > 0.0, "{a:?} and {b:?} are in one another");
            // (A table's two boxes: side by side, apart all the same.)
            if a_box.is_some() && a_box == b_box {
                continue;
            }
            // (A case and its box: one over the other.)
            if !(*a_table && *b_table && apart(1) > 0.3) {
                assert!(apart(0).max(apart(2)) >= least, "{a:?} and {b:?} are under {least} m apart");
            }
        }
    }
    // The lower platform's left side is the platform's right side; the
    // platform's left side the stairs' (TRP's range).
    let shelf = range.solids.iter().find(|s| s.size.distance(Vec3::new(3.0, SHELF, 5.0)) < 1e-4).unwrap();
    assert!((shelf.centre.x - shelf.size.x / 2.0 - HALL_LEFT).abs() < 1e-5);
    let platform = range.solids.iter().find(|s| s.size.distance(Vec3::new(5.0, PLATFORM, 5.0)) < 1e-4).unwrap();
    assert!((platform.centre.x - platform.size.x / 2.0 - STAIRS_LEFT).abs() < 1e-5);
    // The gun bench's top against the knee-high wall, lying along it
    // (longer across the lanes than deep), short of the first table.
    let board = range.solids.iter().find(|s| s.name == "gun bench").expect("the gun bench's table");
    assert!((board.centre.z - board.size.z / 2.0 - (KNEE_WALL_Z + WALL_THICK / 2.0)).abs() < 1e-4, "{:?} {:?}", board.centre, board.size);
    assert!(board.size.x > board.size.z && (board.size.x - GUN_TABLE.length()).abs() < 1e-4, "{:?}", board.size);
    let ([first_table, _], _) = weapon_table(0);
    assert!(board.centre.x + board.size.x / 2.0 < first_table - WEAPON_TABLE.x / 2.0, "{:?} {:?}", board.centre, board.size);
    // The armor bench's top along the stairs in line with the cabinets
    // (its back on their backs' line, longer along the row than deep),
    // past the rigs' cabinet on the start's side by the row's gap.
    let board = range.solids.iter().find(|s| s.name == "armor bench").expect("the armor bench's table");
    assert!((board.centre.x - board.size.x / 2.0 - CABINETS_BACK).abs() < 1e-4, "{:?} {:?}", board.centre, board.size);
    assert!(board.size.z > board.size.x && (board.size.z - ARMOR_TABLE.length()).abs() < 1e-4, "{:?}", board.size);
    assert!((board.centre.z - board.size.z / 2.0 - (CABINETS_NEAR + CABINETS_GAP)).abs() < 1e-4, "{:?} {:?}", board.centre, board.size);
    // Each bench's top 0.894 m up, as the game's tables are.
    for name in ["gun bench", "armor bench"] {
        let top = range.solids.iter().find(|s| s.name == name).unwrap();
        assert!((top.centre.y + top.size.y / 2.0 - 0.894).abs() < 2e-3, "{name}: {:?}", top.centre);
    }
    // Each block of the ring 3 m clear of everything else's footprint.
    let ring = &range.solids[..12];
    for solid in &range.solids[12..] {
        for block in ring {
            let off = (Vec2::new(block.centre.x - solid.centre.x, block.centre.z - solid.centre.z).abs() - Vec2::new(solid.size.x, solid.size.z) / 2.0).max(Vec2::ZERO);
            assert!(off.length() > 3.0, "a block of the ring is {:.1} m from {:?}", off.length(), solid.centre);
        }
    }
}
