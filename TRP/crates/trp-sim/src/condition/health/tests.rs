//! Tests of the character's health: hunger, falls, damage, bleeding and effects at work.
use super::*;

pub(crate) fn settings() -> Arc<HealthSettings> {
    let odds = |k: f32, b: f32, threshold: f32, power: f32| Odds { power, k, b, threshold };
    let bleeding = |damage: f32, loop_time: f32, energy_loop: f32, odds: Odds| BleedingSettings {
        delay: 0.4,
        residue: 0.4,
        damage,
        loop_time,
        damage_dehydrated: 0.4,
        loop_dehydrated: 6.0,
        life_dehydrated: 30.0,
        energy_damage: 0.5,
        energy_loop,
        elite_duration: 22.0,
        odds,
    };
    // This install's figures (`globals.json`).
    Arc::new(HealthSettings {
        maximum: [35.0, 85.0, 70.0, 60.0, 60.0, 65.0, 65.0],
        environment: [0.125, 0.25, 0.17, 0.5, 0.5, 0.5, 0.5],
        over_damage_received: [0.7; 7],
        over_damage_factor: [1.0, 1.0, 1.5, 0.7, 0.7, 1.0, 1.0],
        energy: [0.0, 100.0, 100.0],
        hydration: [0.0, 100.0, 100.0],
        temperature: [28.0, 40.0, 36.6],
        poisoning: [0.0, 100.0, 0.0],
        light: bleeding(0.8, 6.0, 6.0, odds(0.5, -0.125, 0.35, 1.0)),
        heavy: bleeding(0.9, 4.0, 5.0, odds(0.45, -0.17, 0.5, 1.0)),
        fracture: Timing { delay: 5.0, residue: 5.0 },
        fracture_falling: odds(1.0, 0.38, 0.2, 1.0),
        fracture_hit: odds(0.3, 0.05, 0.3, 0.5),
        dehydration: Timing { delay: 50.0, residue: 5.0 },
        dehydration_damage: [1.0, 15.0],
        exhaustion: Timing { delay: 30.0, residue: 5.0 },
        exhaustion_damage: [1.0, 5.0],
        existence: [3.2, 60.0, 2.6, 60.0, 5.0, 5.0],
        low_health: Timing { delay: 0.4, residue: 5.0 },
        low_health_common: 130.0,
        berserk: [5.0, 10.0, 3.0],
        intoxication: [10.0, 15.0, 1.0, 3.0],
        med_start: [2.0, 0.0, 1.0, 2.0, 0.0, 0.0, 0.0],
        med_loop: 0.4,
        pain_tremor_delay: 30.0,
        tremor: Timing { delay: 15.0, residue: 0.2 },
        wound: [25.0, 60.0, 480.0],
        fatigue: [0.0001, 0.0063, 10.0, 180.0],
        fall_safe_height: 3.0,
        fall_damage_per_meter: 9.0,
        fall_overweight: 3.9,
        interruption_time: 2.0,
        stimulator: StimulatorSettings::default(),
    })
}

fn run(h: &mut Health, seconds: f32) {
    let steps = (seconds * 60.0).round() as usize;
    for _ in 0..steps {
        h.step(1.0 / 60.0);
    }
}

#[test]
fn a_run_starts_full_with_existence() {
    let h = Health::new(settings(), 1);
    assert_eq!(h.common(), [440.0, 440.0]);
    assert_eq!(h.energy[0], 100.0);
    assert!(h.active(Kind::Existence, Some(Part::Head)).is_some());
    assert_eq!(h.conditions(), Conditions::default());
}

#[test]
fn hunger_and_thirst_take_their_steps_a_minute() {
    let mut h = Health::new(settings(), 1);
    run(&mut h, 59.9);
    assert_eq!(h.energy[0], 100.0);
    run(&mut h, 0.2);
    assert!((h.energy[0] - 96.8).abs() < 1e-4, "{}", h.energy[0]);
    assert!((h.hydration[0] - 97.4).abs() < 1e-4, "{}", h.hydration[0]);
}

#[test]
fn falls_hurt_both_legs_above_three_metres() {
    let mut h = Health::new(settings(), 1);
    assert_eq!(h.handle_fall(3.0, 1.0), 0.0);
    let damage = h.handle_fall(5.0, 1.0);
    assert!((damage - 2.0f32.powf(1.5) * 9.0).abs() < 1e-4);
    assert!((h.part(Part::LeftLeg).current - (65.0 - damage)).abs() < 1e-3);
    assert!((h.part(Part::RightLeg).current - (65.0 - damage)).abs() < 1e-3);
    // A seven-metre fall destroys both legs: 72 each, the rest spread.
    let mut h = Health::new(settings(), 1);
    h.handle_fall(7.0, 1.0);
    assert!(h.part(Part::LeftLeg).destroyed && h.part(Part::RightLeg).destroyed);
    assert!(h.alive);
    assert!(h.common()[0] < 440.0 - 130.0);
}

/// A contusion on the head: started after 0.1 s, working its time,
/// fading for half of it; a second is a second (`IStackable`).
#[test]
fn a_contusion_works_its_time_and_stacks() {
    let mut h = Health::new(settings(), 1);
    h.do_contusion(10.0, 1.0);
    h.do_contusion(4.0, 1.0);
    let contusions = |h: &Health| h.effects.iter().filter(|e| e.kind == Kind::Contusion).map(|e| (e.part, e.stage)).collect::<Vec<_>>();
    assert_eq!(contusions(&h), [(Part::Head, Stage::Added), (Part::Head, Stage::Added)]);
    for _ in 0..12 {
        h.step(1.0 / 100.0);
    }
    assert_eq!(contusions(&h), [(Part::Head, Stage::Started), (Part::Head, Stage::Started)]);
    // The shorter one's 4 s, then its 2 s of fading.
    for _ in 0..500 {
        h.step(1.0 / 100.0);
    }
    assert_eq!(contusions(&h), [(Part::Head, Stage::Started), (Part::Head, Stage::Residual)]);
    for _ in 0..200 {
        h.step(1.0 / 100.0);
    }
    assert_eq!(contusions(&h), [(Part::Head, Stage::Started)]);
}

#[test]
fn a_stun_builds_holds_and_falls_and_a_flash_works_its_time() {
    let mut h = Health::new(settings(), 1);
    // The Zarya's 40 s at full strength, no cover: 40.2 s working.
    h.do_stun(40.0, 1.0, 1.0);
    let stun = |h: &Health| h.effects.iter().find(|e| e.kind == Kind::Stun).map(|e| (e.stage, e.current_strength));
    let at = |h: &mut Health, seconds: f32| {
        for _ in 0..(seconds * 100.0).round() as usize {
            h.step(1.0 / 100.0);
        }
    };
    at(&mut h, 0.1);
    let (stage, strength) = stun(&h).expect("the stun");
    assert_eq!(stage, Stage::Started);
    assert!(strength > 0.2 && strength < 0.8, "half its build-up: {strength}");
    // Held whole to 0.2 + a fifth of 40.2 s.
    at(&mut h, 7.0);
    assert!((stun(&h).expect("the stun").1 - 1.0).abs() < 1e-4);
    // Then down straight: half way through the fall, half its strength.
    let held = 0.2 + 0.2 * 40.2;
    at(&mut h, (held + 40.2) / 2.0 - 7.1);
    assert!((stun(&h).expect("the stun").1 - 0.5).abs() < 0.02, "{:?}", stun(&h));
    // Its 2 s of residue, then gone.
    at(&mut h, 25.0);
    assert!(stun(&h).is_none_or(|(stage, _)| stage == Stage::Removed));
    // The eyes burnt by a flash 15 m off (its share of 20 m, a quarter):
    // 0.2 + 40 × 0.25 s.
    let share = h.do_burn_eyes(0.25, 0.8, 40.0, 1.0);
    assert_eq!(share, Some(0.25));
    let flash = h.effects.iter().find(|e| e.kind == Kind::Flash).expect("the flash");
    assert!((flash.work - 10.2).abs() < 1e-5 && (flash.strength - 0.8).abs() < 1e-6);
}

#[test]
fn over_damage_is_shared_by_the_maxima() {
    let mut h = Health::new(settings(), 7);
    // 100 to a leg of 65: 35 over, times the leg's factor 1, shared by
    // the other six (375 of maxima) at 0.7 each.
    h.apply_damage(Part::LeftLeg, 100.0, DamageKind::Bullet);
    assert!(h.part(Part::LeftLeg).destroyed);
    let chest = 85.0 - 35.0 * 85.0 / 375.0 * 0.7;
    assert!((h.part(Part::Chest).current - chest).abs() < 1e-3, "{}", h.part(Part::Chest).current);
}

#[test]
fn a_destroyed_head_kills_but_bleeding_does_not_by_that() {
    let mut h = Health::new(settings(), 1);
    h.apply_damage(Part::Head, 40.0, DamageKind::Bullet);
    assert!(!h.alive);
    assert_eq!(h.cause, Some(DamageKind::Bullet));
    let mut h = Health::new(settings(), 1);
    h.apply_damage(Part::Head, 40.0, DamageKind::LightBleeding);
    assert!(h.part(Part::Head).destroyed);
    assert!(h.alive);
}

#[test]
fn a_light_bleeding_drains_every_part_every_six_seconds() {
    let mut h = Health::new(settings(), 1);
    h.do_bleed(false, Part::LeftArm);
    // 0.4 s of delay, then a step every 6 s.
    run(&mut h, 6.3);
    assert_eq!(h.common()[0], 440.0);
    run(&mut h, 0.2);
    assert!((h.common()[0] - (440.0 - 7.0 * 0.8)).abs() < 1e-3, "{}", h.common()[0]);
    assert!(h.active(Kind::LightBleeding, Some(Part::LeftArm)).is_some());
}

#[test]
fn the_screen_s_rates_follow_the_effects_at_work() {
    let mut h = Health::new(settings(), 1);
    // Existence alone: 3.2 energy and 2.6 hydration a minute.
    let r = h.rates();
    assert!(r[0] == 0.0 && (r[1] * 60.0 + 3.2).abs() < 1e-4 && (r[2] * 60.0 + 2.6).abs() < 1e-4, "{r:?}");
    // A light bleeding at work: 0.8 from each of seven parts every
    // 6 s, and its own energy.
    h.do_bleed(false, Part::LeftArm);
    run(&mut h, 0.5);
    let light = settings().light;
    let r = h.rates();
    assert!((r[0] + 0.8 * 7.0 / 6.0).abs() < 1e-5, "{r:?}");
    assert!((r[1] + 3.2 / 60.0 + light.energy_damage / light.energy_loop).abs() < 1e-5, "{r:?}");
    // A part with less than a loop's damage left: the rate becomes
    // what the loop took from the whole.
    h.parts[Part::RightArm.index()].current = 0.3;
    run(&mut h, 6.0);
    let r = h.rates();
    assert!((r[0] + (0.8 * 6.0 + 0.3) / 6.0).abs() < 1e-4, "{r:?}");
    // Stopped metabolism leaves energy and hydration still.
    h.metabolism_disabled = true;
    let r = h.rates();
    assert!(r[1] == 0.0 && r[2] == 0.0, "{r:?}");
}

#[test]
fn a_broken_leg_slows_and_a_painkiller_lets_it_run() {
    let mut h = Health::new(settings(), 1);
    h.do_fracture(Part::LeftLeg);
    let c = h.conditions();
    assert!(c.left_leg && !c.right_leg);
    assert_eq!(c.speed_limit(), Some(0.3));
    assert!(!c.can_sprint());
    // Its pain a second later; tremor 30 s after the pain starts, and
    // then its own 15 s.
    run(&mut h, 1.1);
    assert!(h.active(Kind::Pain, Some(Part::LeftLeg)).is_some());
    run(&mut h, 44.0);
    assert!(!h.conditions().tremor);
    run(&mut h, 1.2);
    assert!(h.conditions().tremor);
    h.add_by_item(Kind::PainKiller, Part::Head, "analgin", 1.0, 0.0, 80.0, 15.0, 80.0);
    let c = h.conditions();
    assert!(c.on_painkillers && c.can_sprint() && c.speed_limit().is_none() && !c.tremor_on());
}

/// A character brought to 124 of 440, every part but the head at 15
/// and the chest hit for one more: low health.
fn brought_low(elite: bool) -> Health {
    let mut h = Health::new(settings(), 1);
    h.skills.stress_berserk = elite;
    for part in Part::ALL.into_iter().filter(|p| *p != Part::Head) {
        h.parts[part.index()].current = 15.0;
    }
    h.apply_damage_info(Part::Chest, 1.0, DamageKind::Undefined, [0.0, 0.0], true);
    assert!(h.existing(Kind::LowHealth, None).is_some());
    h
}

/// Low health starting with Stress Resistance's elite
/// (`LowEdgeHealth.Started`): a berserk of strength 2 on the head, 5 s
/// after, working 10 s and fading 3; meanwhile the character is on
/// painkillers but its arms' damage stays (`UpdateArmsCondition` goes
/// by a painkiller alone). Without the elite, none.
#[test]
fn low_health_starts_a_berserk_at_elite_stress_resistance() {
    let mut h = brought_low(false);
    run(&mut h, 6.0);
    assert!(h.existing(Kind::Berserk, None).is_none());
    let mut h = brought_low(true);
    h.do_fracture(Part::LeftArm);
    h.do_fracture(Part::LeftLeg);
    run(&mut h, 0.5);
    let berserk = h.existing(Kind::Berserk, None).expect("added as low health started");
    assert_eq!((berserk.part, berserk.stage, berserk.strength), (Part::Head, Stage::Added, 2.0));
    assert!(!h.conditions().on_painkillers);
    run(&mut h, 4.8);
    assert_eq!(h.existing(Kind::Berserk, None).map(|e| e.stage), Some(Stage::Added));
    run(&mut h, 0.2);
    assert_eq!(h.existing(Kind::Berserk, None).map(|e| e.stage), Some(Stage::Started));
    assert_eq!(std::mem::take(&mut h.berserk_edges), [true]);
    let c = h.conditions();
    assert!(c.on_painkillers && c.left_arm && c.can_sprint() && c.speed_limit().is_none() && !c.lame(), "{c:?}");
    assert_eq!(c.aim_cost(), 1.0);
    run(&mut h, 1.1);
    assert_eq!(h.active(Kind::Berserk, None).map(|e| e.current_strength), Some(2.0));
    run(&mut h, 9.0);
    assert_eq!(h.existing(Kind::Berserk, None).map(|e| e.stage), None);
    assert!(h.effects.iter().any(|e| e.kind == Kind::Berserk && e.stage == Stage::Residual));
    assert_eq!(std::mem::take(&mut h.berserk_edges), [false]);
    assert!(!h.conditions().on_painkillers);
    run(&mut h, 3.0);
    assert!(h.effects.iter().all(|e| e.kind != Kind::Berserk));
    assert!(h.active(Kind::LowHealth, None).is_some(), "low health goes on");
}

/// Low health over takes its berserk with it (`RemoveSideEffect`, as
/// low health is removed after its 5 s residue): one at work is cut
/// to its residue.
#[test]
fn a_berserk_ends_with_low_health() {
    let mut h = brought_low(true);
    run(&mut h, 1.0);
    for part in Part::ALL {
        h.parts[part.index()].current = h.parts[part.index()].maximum;
    }
    run(&mut h, 4.6);
    assert!(h.active(Kind::Berserk, None).is_some());
    run(&mut h, 0.5);
    assert!(h.existing(Kind::LowHealth, None).is_none());
    assert!(h.effects.iter().any(|e| e.kind == Kind::Berserk && e.stage == Stage::Residual));
    // Low health taken off while its berserk waits: the berserk goes
    // through its start and its residue at once (`ForceResidue` of an
    // added effect), so its phrase is said and the stamina freed and
    // bound again, as the game's.
    let mut h = brought_low(true);
    run(&mut h, 1.0);
    let low = h.existing(Kind::LowHealth, None).map(|e| e.id).expect("low health");
    h.take_events();
    h.force_remove(low);
    assert!(h.effects.iter().all(|e| e.kind != Kind::Berserk));
    assert_eq!(h.berserk_edges, [true, false]);
    assert!(h.take_events().contains(&Event::Started(Kind::Berserk, Part::Head)));
}

/// A bleeding under a berserk (`BleedHealth`): its loop goes on and its
/// energy is taken, but no part loses anything.
#[test]
fn a_berserk_stops_a_bleeding_s_damage() {
    let mut h = brought_low(true);
    h.do_bleed(false, Part::LeftArm);
    run(&mut h, 5.5);
    assert!(h.active(Kind::Berserk, None).is_some());
    let (common, energy) = (h.common()[0], h.energy[0]);
    run(&mut h, 6.0);
    assert_eq!(h.common()[0], common);
    assert!((h.energy[0] - (energy - 0.5)).abs() < 1e-4, "{} from {energy}", h.energy[0]);
}

#[test]
fn a_medkit_heals_a_part_and_pays_for_a_cure() {
    let mut h = Health::new(settings(), 1);
    h.apply_damage(Part::Chest, 30.0, DamageKind::Fall);
    h.do_bleed(false, Part::Chest);
    run(&mut h, 0.5);
    let ifak = Arc::new(Medicine {
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
    });
    assert_eq!(h.part_to_apply(&ifak, 300.0, None), Some(Part::Chest));
    let id = h.use_item(ifak, 9, 300.0, None, None).expect("at work");
    assert!(h.index_of(id).is_some());
    run(&mut h, 2.0 + 3.0 + 0.2);
    assert!(h.existing(Kind::LightBleeding, Some(Part::Chest)).is_none());
    // No more than the part lacked: 30 of the kit's 50, and the cure's
    // 30 besides.
    assert_eq!(h.part(Part::Chest).current, 85.0);
    let events = h.take_events();
    let last = events.iter().rev().find_map(|e| if let Event::Resource(9, r) = e { Some(*r) } else { None }).expect("its resource");
    assert_eq!(last, 240.0);
}

#[test]
fn the_inventory_s_use_names_a_surgical_kit_s_own_part() {
    let mut h = Health::new(settings(), 1);
    h.apply_damage(Part::RightArm, 200.0, DamageKind::Fall);
    assert!(h.part(Part::RightArm).destroyed);
    let kit = |class: MedClass, priority: Vec<Part>| Medicine {
        template: "kit".into(),
        class,
        use_time: 16.0,
        part_time: Vec::new(),
        max_hp_resource: 5.0,
        hp_resource_rate: 0.0,
        part_priority: priority,
        energy: 0.0,
        hydration: 0.0,
        cures: vec![(Cure::DestroyedPart, CureSpec { delay: 0.0, duration: 0.0, fade_out: 0.0, cost: 1.0, kept: [25, 45] })],
        max_resource: 0.0,
        buffs: None,
    };
    let own = vec![Part::Stomach, Part::LeftLeg, Part::RightLeg, Part::LeftArm, Part::RightArm];
    // The first part it can help is one of its own: that part.
    assert_eq!(h.use_part(&kit(MedClass::Medical, own.clone()), 5.0), Some(Part::RightArm));
    // Not one of its own: the common body.
    assert_eq!(h.use_part(&kit(MedClass::Medical, vec![Part::Stomach]), 5.0), None);
    // A medkit's class, or no priority: the common body.
    assert_eq!(h.use_part(&kit(MedClass::MedKit, own), 5.0), None);
    assert_eq!(h.use_part(&kit(MedClass::Medical, Vec::new()), 5.0), None);
}

#[test]
fn barbed_wire_cuts_two_a_metre_in_whole_points() {
    let mut t = BarbTally::default();
    assert_eq!(t.cut([0.0, 0.0, 0.0]), None);
    assert_eq!(t.cut([0.3, 0.0, 0.0]), None);
    let d = t.cut([0.6, 0.0, 0.0]).expect("a point");
    assert!((d - 1.2).abs() < 1e-5);
    // A jump through starts over.
    assert_eq!(t.cut([2.0, 0.0, 0.0]), None);
}
