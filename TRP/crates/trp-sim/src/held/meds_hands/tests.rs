//! Tests of an item's use in the hands: its parts, cancels and the way back.
use super::*;
use crate::condition::health::{Cure, CureSpec, DamageKind, MedClass};

fn ifak() -> Arc<Medicine> {
    Arc::new(Medicine {
        template: "ifak".into(),
        class: MedClass::MedKit,
        use_time: 3.0,
        part_time: Vec::new(),
        max_hp_resource: 300.0,
        hp_resource_rate: 50.0,
        part_priority: Vec::new(),
        energy: 0.0,
        hydration: 0.0,
        cures: vec![(Cure::LightBleeding, CureSpec { delay: 0.0, duration: 0.0, fade_out: 0.0, cost: 30.0, kept: [0, 0] })],
        max_resource: 0.0,
        buffs: None,
    })
}

/// Steps health and the hands together, as a host does, for so long;
/// whether the hands were put away meanwhile.
fn run(health: &mut Health, hands: &mut MedsHands, seconds: f32) -> bool {
    let mut hidden = false;
    for _ in 0..(seconds * 60.0).round() as usize {
        health.step(1.0 / 60.0);
        for event in health.take_events() {
            hands.on_health(health, &event);
        }
        hidden |= hands.step(health, 1.0 / 60.0, 0.0).contains(&MedsEvent::Hidden);
    }
    hidden
}

#[test]
fn a_cancel_stops_the_use_six_tenths_of_a_second_later_keeping_what_it_did() {
    let mut health = Health::new(crate::condition::health::tests::settings(), 1);
    health.apply_damage(Part::Chest, 20.0, DamageKind::Fall);
    health.apply_damage(Part::LeftArm, 20.0, DamageKind::Fall);
    let mut hands = MedsHands::new(None, ifak(), 0);
    assert!(hands.take(&mut health, 9, 300.0, &[Some(Part::Chest), Some(Part::LeftArm)], None, None).is_some());
    // Into the chest's work (a 2 s delay, then 3 s), the cancel asked.
    assert!(!run(&mut health, &mut hands, 3.0));
    let healed = health.part(Part::Chest).current;
    hands.cancel();
    // Not yet: 0.6 s go by first.
    assert!(!run(&mut health, &mut hands, 0.5));
    assert!(hands.at_work(), "still at work before the delay is up");
    // Then the work stops and the item is gone; the arm never begun.
    assert!(run(&mut health, &mut hands, 1.0));
    assert!(!hands.in_hand() && !hands.at_work());
    assert!(health.existing(Kind::MedEffect, Some(Part::LeftArm)).is_none(), "the next part is not gone on to");
    assert!(health.part(Part::Chest).current >= healed && health.part(Part::Chest).current < 85.0, "what was healed is kept, the rest not");
    assert_eq!(health.part(Part::LeftArm).current, 40.0);
}

#[test]
fn a_kit_goes_on_to_the_next_part_named_then_finishes() {
    let mut health = Health::new(crate::condition::health::tests::settings(), 1);
    health.apply_damage(Part::Chest, 20.0, DamageKind::Fall);
    health.apply_damage(Part::LeftArm, 20.0, DamageKind::Fall);
    let mut hands = MedsHands::new(None, ifak(), 0);
    assert!(hands.take(&mut health, 9, 300.0, &[Some(Part::Chest), Some(Part::LeftArm)], None, None).is_some());
    // The chest done (a 2 s delay and 3 s of work), the arm is begun.
    assert!(!run(&mut health, &mut hands, 5.2));
    assert_eq!(health.part(Part::Chest).current, 85.0);
    assert!(health.existing(Kind::MedEffect, Some(Part::LeftArm)).is_some(), "the next part at work");
    assert!(hands.at_work());
    // The arm done, the list is over: finished, and with no rig to
    // play the put-away, gone as the way back's 0.6 s end.
    assert!(run(&mut health, &mut hands, 5.8));
    assert_eq!(health.part(Part::LeftArm).current, 60.0);
    assert!(!hands.in_hand() && !hands.at_work());
}

#[test]
fn the_common_body_is_left_after_one_part() {
    let mut health = Health::new(crate::condition::health::tests::settings(), 1);
    health.apply_damage(Part::Chest, 20.0, DamageKind::Fall);
    health.apply_damage(Part::LeftArm, 20.0, DamageKind::Fall);
    let mut hands = MedsHands::new(None, ifak(), 0);
    assert!(hands.take(&mut health, 9, 300.0, &[None], None, None).is_some());
    assert!(run(&mut health, &mut hands, 5.8), "handed back after the first part");
    assert!(health.existing(Kind::MedEffect, None).is_none());
    // The part health chose is whole, the other waits for another use.
    let whole = [(Part::Chest, 85.0), (Part::LeftArm, 60.0)].iter().filter(|(part, top)| health.part(*part).current == *top).count();
    assert_eq!(whole, 1, "one part healed");
}

#[test]
fn a_use_given_way_stops_six_tenths_of_a_second_on() {
    let mut health = Health::new(crate::condition::health::tests::settings(), 1);
    health.apply_damage(Part::Chest, 20.0, DamageKind::Fall);
    let mut hands = MedsHands::new(None, ifak(), 0);
    assert!(hands.take(&mut health, 9, 300.0, &[None], None, None).is_some());
    assert!(!run(&mut health, &mut hands, 1.0));
    hands.drop();
    assert!(!run(&mut health, &mut hands, 0.55));
    assert!(health.existing(Kind::MedEffect, None).is_some(), "still at work");
    assert!(run(&mut health, &mut hands, 0.1));
    assert!(health.existing(Kind::MedEffect, None).is_none_or(|e| !e.existing()), "cut short");
    assert_eq!(health.part(Part::Chest).current, 65.0, "nothing healed");
}

/// One frame of `dt` as a host runs it: health, what it did told to the
/// hands, then the hands. Whether the use finished in health's update,
/// and whether the hands were handed over.
fn frame(health: &mut Health, hands: &mut MedsHands, dt: f32) -> (bool, bool) {
    let working = hands.at_work();
    health.step(dt);
    for event in health.take_events() {
        hands.on_health(health, &event);
    }
    let finished = working && !hands.at_work();
    (finished, hands.step(health, dt, 0.0).contains(&MedsEvent::Hidden))
}

/// Runs frames of 1/120 s up to the use's finish; none hands over.
fn to_the_finish(health: &mut Health, hands: &mut MedsHands) {
    for _ in 0..1200 {
        let (finished, hidden) = frame(health, hands, 1.0 / 120.0);
        assert!(!hidden, "handed over before the finish");
        if finished {
            return;
        }
    }
    panic!("no finish in 10 s");
}

#[test]
fn a_use_over_by_itself_hands_back_six_tenths_of_a_second_on_from_the_next_frame() {
    let mut health = Health::new(crate::condition::health::tests::settings(), 1);
    health.apply_damage(Part::Chest, 20.0, DamageKind::Fall);
    let mut hands = MedsHands::new(None, ifak(), 0);
    assert!(hands.take(&mut health, 9, 300.0, &[None], None, None).is_some());
    to_the_finish(&mut health, &mut hands);
    // The finish frame's own time is not counted, and 72 frames make
    // 0.6 s, short of the allowance: the 73rd hands over.
    for k in 1..=72 {
        assert!(!frame(&mut health, &mut hands, 1.0 / 120.0).1, "handed over {k} frames on");
        assert!(hands.in_hand() && !hands.at_work());
    }
    assert!(frame(&mut health, &mut hands, 1.0 / 120.0).1, "handed over 0.6083 s on");
    assert!(!hands.in_hand());
}

#[test]
fn a_drop_asked_on_the_way_back_waits_no_longer() {
    let mut health = Health::new(crate::condition::health::tests::settings(), 1);
    health.apply_damage(Part::Chest, 20.0, DamageKind::Fall);
    let mut hands = MedsHands::new(None, ifak(), 0);
    assert!(hands.take(&mut health, 9, 300.0, &[None], None, None).is_some());
    to_the_finish(&mut health, &mut hands);
    for _ in 0..36 {
        assert!(!frame(&mut health, &mut hands, 1.0 / 120.0).1);
    }
    // A key meanwhile: what it asks comes as the way back's drop ends.
    hands.drop();
    for k in 37..=72 {
        assert!(!frame(&mut health, &mut hands, 1.0 / 120.0).1, "handed over {k} frames on");
    }
    assert!(frame(&mut health, &mut hands, 1.0 / 120.0).1, "on the way back's own frame");
}

#[test]
fn a_refused_item_goes_six_tenths_of_a_second_on() {
    // Nothing to mend: the kit is refused, and the hands before are
    // asked for at once.
    let mut health = Health::new(crate::condition::health::tests::settings(), 1);
    let mut hands = MedsHands::new(None, ifak(), 0);
    assert!(hands.take(&mut health, 9, 300.0, &[None], None, None).is_none());
    assert!(hands.in_hand() && !hands.at_work());
    assert!(!run(&mut health, &mut hands, 0.55));
    assert!(run(&mut health, &mut hands, 0.1));
    assert!(!hands.in_hand());
}
