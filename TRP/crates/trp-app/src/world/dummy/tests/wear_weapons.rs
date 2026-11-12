//! Tests of the dummy's worn things, its hung and held weapon, and its pieces.

use super::*;

/// A shot on a dummy's vest (body hits spec, "Its armour's wear kept on
/// its things"): its wear written onto the vest's thing, kept there
/// through a search of the body (a shot during it reaching the gear on
/// the screen) and back on the body after, the armour dressed again
/// from its things at that wear; HEAL AND STAND UP renews it.
#[test]
fn a_shot_vests_wear_stays_on_its_thing() {
    let Some((install, db)) = installed() else { return };
    let kit = crate::dummy_kit(install, db, "usec").expect("the USEC's kit");
    let rules = Arc::new(ArmorRules::new(&db.config).expect("the armour's rules"));
    let skills = VestSkills::default();
    let mut wearer = wearer_of(db, rules.clone(), &kit, skills);
    assert!(!wearer.pieces.is_empty(), "the USEC's starting kit wears armour");
    for piece in &wearer.pieces {
        let thing = kit.find(piece.item.expect("a thing for each piece")).expect("its thing on the body");
        assert_eq!(thing.durability(), [piece.durability, piece.max_durability], "dressed at its thing's wear");
    }
    // A rifle's round on the first piece over a body's collider: the
    // first a hit there meets.
    let shot = Shot { damage: 50.0, penetration: 30.0, armor_damage: 0.5, deflected_by: None, blocked_by: None, stamina_burn: 0.0, heavy_bleeding: 0.0, light_bleeding: 0.0, weapon: true, melee: false };
    let shoot = |wearer: &mut Wearer| {
        let piece = wearer.pieces.iter().find(|piece| piece.colliders != 0).expect("a piece over a body's collider");
        let (collider, thing) = (piece.colliders.trailing_zeros() as u8, piece.item.expect("its thing"));
        wearer.take_shot(&shot, collider, 0, true);
        let piece = wearer.pieces.iter().find(|piece| piece.item == Some(thing)).expect("the piece");
        (thing, piece.durability, piece.max_durability)
    };
    let (thing, durability, most) = shoot(&mut wearer);
    let new = kit.find(thing).expect("the vest").durability();
    assert!(durability < new[0], "the shot wore it: {durability} of {new:?}");
    eprintln!("{}: new {new:?}, shot {durability:.3}/{most}", kit.find(thing).map_or("?", |vest| vest.kind.name.as_str()));
    // The body searched (`SEARCH`): its gear on the screen.
    let (mut carried, _) = crate::screens::inventory_view::tests::carried();
    carried.away = vec![None];
    carried.loot_of = 0;
    carried.bodies = vec![Some(kit)];
    assert!(wear_differs(&wearer, carried.body(0).expect("the body")));
    keep_wear(&wearer, carried.body_mut(0).expect("the body"));
    assert!(!wear_differs(&wearer, carried.body(0).expect("the body")));
    assert!(carried.search_body(0) && carried.shows_body(), "the body on the screen");
    assert_eq!(carried.body(0).and_then(|gear| gear.find(thing)).map(trp_sim::items::inventory::Item::durability), Some([durability, most]), "searched, the vest shows its wear");
    // A second shot while it is searched: onto the gear on the screen.
    let (_, worse, _) = shoot(&mut wearer);
    keep_wear(&wearer, carried.body_mut(0).expect("the body on the screen"));
    assert_eq!(carried.loot.find(thing).map(|vest| vest.durability()[0]), Some(worse));
    // The screen shut: the body keeps it, and its armour dressed again
    // from its things wears it.
    carried.put_back_body();
    let gear = carried.bodies[0].as_ref().expect("the body's gear back");
    assert_eq!(gear.find(thing).map(|vest| vest.durability()[0]), Some(worse));
    let again = wearer_of(db, rules.clone(), gear, skills);
    assert_eq!(again.pieces.iter().find(|piece| piece.item == Some(thing)).map(|piece| piece.durability), Some(worse));
    // HEAL AND STAND UP: the things made new, the armour new.
    let renewed = renew_armour(db, rules.clone(), carried.bodies[0].as_mut().expect("the body"), skills);
    let gear = carried.bodies[0].as_ref().expect("the body");
    assert_eq!(gear.find(thing).map(trp_sim::items::inventory::Item::durability), Some(new), "renewed: its template's wear");
    assert_eq!(renewed.pieces.iter().find(|piece| piece.item == Some(thing)).map(|piece| piece.durability), Some(new[0]));
    assert!(!wear_differs(&renewed, gear));
}

/// A dead dummy's hung weapon (body hits spec, "Its weapon in hand stays
/// in its slot"): it hangs while its thing is in the rifle's slot, a part
/// taken off the weapon leaving it so; once the thing leaves the slot its
/// model is hidden, its body and the strap's spring taken out of the
/// corpse's engine; HEAL AND STAND UP shows it again (`heal_dummies`).
#[test]
fn the_hung_weapon_goes_once_its_thing_leaves_the_slot() {
    let Some((install, db)) = installed() else { return };
    let kit = crate::dummy_kit(install, db, "bear").expect("the BEAR's kit");
    let slot = kit.slot_index(IN_HAND_SLOT).expect("the rifle's slot");
    let akm = in_slot(&kit, slot).expect("the AKM in it");
    assert!(still_in_hands(Some(&kit), (slot, akm)));
    assert!(!still_in_hands(None, (slot, akm)), "no body, nothing in hand");
    // The corpse's engine: the pelvis and the weapon on the strap's
    // spring.
    let mut engine = World::new(trp_physics::Settings::game());
    let ball = |at: [f32; 3], mass: f32| BodyDesc {
        at,
        turn: [0.0, 0.0, 0.0, 1.0],
        colliders: vec![PhysicsCollider { shape: PhysicsShape::Sphere { radius: 0.1 }, at: [0.0; 3], turn: [0.0, 0.0, 0.0, 1.0], material: Material::default() }],
        mass,
        drag: 0.0,
        angular_drag: crate::player::death::UNITY_ANGULAR_DRAG,
        velocity: [0.0; 3],
        spin: [0.0; 3],
        max_depenetration: 10.0,
        continuous: false,
        layer: 0,
    };
    let pelvis = engine.add_body(&ball([0.0, 1.0, 0.0], 10.0));
    let weapon = engine.add_body(&ball([0.5, 1.0, 0.0], crate::player::death::WEAPON_MASS));
    engine.spring_joint(weapon, [0.0; 3], pelvis, crate::player::death::STRAP_ON_PELVIS, crate::player::death::STRAP_SPRING, crate::player::death::STRAP_DAMPER, [0.0, 0.0], true, 10.0);
    let fall = Fall {
        engine,
        moved: Vec::new(),
        push: None,
        weapon: Some(weapon),
        landing: trp_sim::sound::landing::Landing::new(0, crate::player::death::WEAPON_MASS, 0.0),
        since: 0.0,
        surfaces: HashMap::new(),
        in_hands: Some((slot, akm)),
        hung: true,
    };
    let mut world = bevy::ecs::world::World::new();
    let player = world.spawn(Visibility::default()).id();
    let pivot = world.spawn(Visibility::default()).id();
    let model = Model { player, pivot, tree: Vec::new(), weapon_root: None, grip: Grip { root_local: Transform::default(), fingers: Vec::new() } };
    let mut health = Health::new(Arc::new(HealthSettings::new(&db.config).expect("the health's settings")), 1);
    health.alive = false;
    let wearer = Wearer::dress(db, Arc::new(ArmorRules::new(&db.config).expect("the armour's rules")), &worn_of(&kit), VestSkills::default());
    let dummy = Dummy {
        which: 0,
        colliders: Vec::new(),
        placed: Vec::new(),
        objects: Vec::new(),
        wearer,
        worn: false,
        health,
        followed: false,
        load: (0.0, Vec::new()),
        dressed: true,
        taken: Vec::new(),
        killing: None,
        fall: Some(fall),
        hits: 0,
        puppet: None,
        heading: 1.0,
        model: Some(model),
        empty_grip: None,
        own: Some(akm),
        holds: Some((slot, akm)),
        drawn: Vec::new(),
        arm_bones: [[None; 6]; 2],
        locals: Vec::new(),
        parents: Arc::from(Vec::<Option<usize>>::new()),
        colliders_frame: None,
        before_frame: None,
        placements: Mutex::new(Vec::new()),
        drawn_by: PosingDrawn::EveryFrame,
        held_frame: None,
        heard: Vec::new(),
    };
    world.insert_resource(Dummies { list: vec![dummy], bleeding_drops: Vec::new(), experience: Vec::new(), marks: Vec::new(), listened: false, economy: true, posing_log: PosingLog::default() });
    let (mut carried, _) = crate::screens::inventory_view::tests::carried();
    carried.bodies = vec![Some(kit)];
    world.insert_resource(carried);
    let state = |world: &mut bevy::ecs::world::World| {
        let dummies = world.resource::<Dummies>();
        let fall = dummies.list[0].fall.as_ref().expect("dead");
        let shown = dummies.weapon_trees().next().map(|(_, shown)| shown);
        (fall.hung, fall.weapon.is_some(), fall.engine.body_count(), *world.get::<Visibility>(player).expect("the model"), shown)
    };
    world.run_system_once(clear_dummy_weapons).expect("the system ran");
    assert_eq!(state(&mut world), (true, true, 2, Visibility::Inherited, Some(true)), "hung while the AKM is in its slot");
    // The run begun again (`death::begin_again`): the body's things new
    // numbers, the dummy holding, held as it died and wearing them by
    // those; the weapon still hangs, and none is hung on its back.
    let pieces_before = world.resource::<Dummies>().list[0].wearer.pieces.iter().filter(|piece| piece.item.is_some()).count();
    world.resource_scope(|world, mut dummies: Mut<Dummies>| {
        let mut carried = world.resource_mut::<crate::screens::inventory_view::Carried>();
        renumber_bodies(&mut carried, Some(&mut dummies));
    });
    let renewed = in_slot(world.resource::<crate::screens::inventory_view::Carried>().bodies[0].as_ref().expect("the body"), slot).expect("the AKM still in its slot");
    assert_ne!(renewed, akm, "a new number");
    {
        let dummies = world.resource::<Dummies>();
        let dummy = &dummies.list[0];
        assert_eq!((dummy.own, dummy.holds, dummy.fall.as_ref().and_then(|fall| fall.in_hands)), (Some(renewed), Some((slot, renewed)), Some((slot, renewed))));
        let gear = world.resource::<crate::screens::inventory_view::Carried>().bodies[0].clone().expect("the body");
        let found = dummy.wearer.pieces.iter().filter_map(|piece| piece.item).filter(|item| gear.find(*item).is_some()).count();
        assert_eq!(found, pieces_before, "every piece worn found by its new number");
        assert!(still_in_hands(Some(&gear), (slot, renewed)));
    }
    world.run_system_once(clear_dummy_weapons).expect("the system ran");
    assert_eq!(state(&mut world), (true, true, 2, Visibility::Inherited, Some(true)), "still hung after the run begins again");
    let akm = renewed;
    let _ = akm;
    // Its magazine taken off: another thing; it still hangs.
    {
        let mut carried = world.resource_mut::<crate::screens::inventory_view::Carried>();
        let body = carried.bodies[0].as_mut().expect("the body");
        let rifle = body.slots[slot].as_mut().expect("the AKM");
        let magazine = rifle.slot_index(trp_sim::items::inventory::MAGAZINE_SLOT).expect("its magazine's slot");
        assert!(rifle.slots[magazine].take().is_some(), "its magazine taken");
    }
    world.run_system_once(clear_dummy_weapons).expect("the system ran");
    assert_eq!(state(&mut world), (true, true, 2, Visibility::Inherited, Some(true)), "a part taken off leaves it hanging");
    // The AKM taken off the body: the model hidden, its body and spring
    // gone, the pelvis alone in the engine.
    let taken = world.resource_mut::<crate::screens::inventory_view::Carried>().bodies[0].as_mut().expect("the body").slots[slot].take();
    assert_eq!(taken.map(|rifle| rifle.id), Some(akm));
    world.run_system_once(clear_dummy_weapons).expect("the system ran");
    assert_eq!(state(&mut world), (false, false, 1, Visibility::Hidden, Some(false)), "gone with its thing");
    // Put back, it stays gone (the game's model is destroyed).
    world.resource_mut::<crate::screens::inventory_view::Carried>().bodies[0].as_mut().expect("the body").slots[slot] = Some(trp_sim::items::inventory::Item::new(Arc::new(trp_sim::items::inventory::Kind::of(db, WEAPON).expect("the AKM's kind"))));
    world.run_system_once(clear_dummy_weapons).expect("the system ran");
    assert_eq!(state(&mut world), (false, false, 1, Visibility::Hidden, Some(false)));
    // Its slots' views leave the AKM it held out for good (the corpse
    // keeps the living body's binding): another AKM in that slot hangs.
    assert_eq!(world.resource::<Dummies>().in_hand(0), Some(akm));
}

/// Where a dummy takes its own AKM in hand from as it stands up
/// (`held_of`; `TrySetLastEquippedWeapon`): its rifle's slot, or its
/// back's where it was moved; taken off its body, or another AKM put in
/// its place, its hands are empty.
#[test]
fn a_dummy_holds_its_own_akm_or_nothing() {
    let Some((install, db)) = installed() else { return };
    let mut kit = crate::dummy_kit(install, db, "usec").expect("the USEC's kit");
    let (rifle, back) = (kit.slot_index(IN_HAND_SLOT).expect("the rifle's slot"), kit.slot_index("SecondPrimaryWeapon").expect("the back's"));
    let own = in_slot(&kit, rifle);
    assert!(own.is_some());
    assert_eq!(held_of(Some(&kit), own), Some((rifle, own.expect("its AKM"))));
    assert_eq!(held_of(None, own), None, "no body, nothing");
    let akm = kit.slots[rifle].take();
    assert_eq!(held_of(Some(&kit), own), None, "taken: its hands empty");
    kit.slots[back] = akm;
    assert_eq!(held_of(Some(&kit), own), Some((back, own.expect("its AKM"))), "on its back: taken from there");
    kit.slots[back] = None;
    kit.slots[rifle] = Some(trp_sim::items::inventory::Item::new(Arc::new(trp_sim::items::inventory::Kind::of(db, WEAPON).expect("the AKM's kind"))));
    assert_eq!(held_of(Some(&kit), own), None, "another AKM is not its own");
}

/// A dummy's pieces by what its own body has on: a worn thing by its
/// mesh for what it is over, a part by its being fitted, its own body
/// and torso's case, an outfit's body in place of its own.
#[test]
fn a_dummys_pieces_go_by_what_its_body_has_on() {
    use trp_data::character::arms::Torso;
    use trp_data::character::dress::Under;
    let helmet = "helmet".to_owned();
    let visor = vec![("mod_equipment".to_owned(), "visor".to_owned())];
    let mut look = Look {
        on: vec![helmet.clone(), "vest".to_owned()],
        under: Under { armour: false, vest: true },
        torso: Some(Torso::ALL[0]),
        outfit: None,
        fitted: vec![(helmet.clone(), visor.clone())],
    };
    assert!(look.shows(&Shows::Worn { item: helmet.clone(), over: Vec::new() }));
    assert!(!look.shows(&Shows::Worn { item: "backpack".to_owned(), over: Vec::new() }));
    // A backpack's mesh for a vest under it, not for none.
    look.on.push("backpack".to_owned());
    assert!(look.shows(&Shows::Worn { item: "backpack".to_owned(), over: vec![Under { armour: false, vest: true }] }));
    assert!(!look.shows(&Shows::Worn { item: "backpack".to_owned(), over: vec![Under::default()] }));
    assert!(look.shows(&Shows::Head { item: helmet.clone(), path: visor.clone() }));
    assert!(!look.shows(&Shows::Head { item: "another".to_owned(), path: visor }));
    // Its own body; a torso of the newer kind by its case.
    assert!(look.shows(&Shows::Own { top: true, torso: None }));
    assert!(look.shows(&Shows::Own { top: true, torso: Some(Torso::ALL[0]) }));
    assert!(!look.shows(&Shows::Own { top: true, torso: Some(Torso::ALL[1]) }));
    // An outfit with a body hides the whole of its own; one of the
    // install's tops its top alone.
    look.outfit = Some(("seva".to_owned(), true, false));
    assert!(look.shows(&Shows::Outfit { item: "seva".to_owned() }));
    assert!(!look.shows(&Shows::Outfit { item: "another".to_owned() }));
    assert!(!look.shows(&Shows::Own { top: false, torso: None }));
    look.outfit = Some(("a top".to_owned(), false, true));
    assert!(!look.shows(&Shows::Own { top: true, torso: None }));
    assert!(look.shows(&Shows::Own { top: false, torso: None }));
}
