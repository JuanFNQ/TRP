//! Tests of the stamina's pools: what each move and aim costs, and the refill.
use super::*;

/// This install's figures, as far as these tests use them.
fn settings() -> StaminaSettings {
    let flat = Graph { lower_left: 1.0, lower_right: 1.0, left_plateau: 1.0, right_plateau: 1.0, right_limit: 1.0, zero: 1.0 };
    StaminaSettings {
        capacity: 100.0,
        restoration: 4.5,
        hands_capacity: 80.0,
        hands_restoration: 2.0,
        oxygen_capacity: 350.0,
        oxygen_restoration: 4.0,
        sprint_drain: 4.0,
        jump_cost: 14.0,
        fast_switch_cost: 15.0,
        exhausted_melee_speed: 0.65,
        exhausted_melee_damage: 0.33,
        aim_drain: 1.0,
        aim_rangefinder_drain: 1.0,
        aim_by_pose: [0.15, 0.75, 1.0],
        restoration_by_pose: [1.45, 1.25, 1.0],
        overweight_by_pose: [-3.0, -2.0, 0.0],
        standup: [10.0, 20.0],
        pose_notch: [1.1, 3.0],
        walk: [0.2, 0.5],
        crouch: [0.17, 0.7],
        prone_cost: 1.0,
        vault_legs: [5.0, 20.0],
        vault_hand: [5.0, 16.0],
        climb_legs: [0.0, 20.0],
        climb_hand: [8.0, 20.0],
        climb_hands: [12.0, 25.0],
        hold_breath: 4.0,
        hold_breath_by_legs: [2.0, 1.0],
        legs_shake_hands: true,
        legs_deepen_breath: false,
        delta_level: 0.166,
        overweight: 0.0,
        walk_overweight: 0.0,
        endurance_capacity: 0.0,
        endurance_restoration: 0.0,
        endurance_jump: 0.0,
        endurance_breath: 0.0,
        strength_aim: 0.0,
        endurance_hands: 0.0,
        endurance_breath_elite: false,
        buff_capacity: 0.0,
        buff_restoration: 0.0,
        skill_buffs: [0.5, 0.5, 0.3, 1.0, 0.2],
        energy: 1.0,
        hydration: 1.0,
        energy_graph: flat,
        hydration_graph: flat,
        health: 1.0,
        arms_aim: 1.0,
        fatigue: [15.0, 0.42],
    }
}

const DT: f32 = 1.0 / 120.0;

fn run(stamina: &mut Stamina, now: &mut f32, seconds: f32) {
    for _ in 0..(seconds / DT).round() as usize {
        *now += DT;
        stamina.update(DT, *now);
    }
}

#[test]
fn a_wound_burns_the_legs_and_lets_them_refill() {
    let mut stamina = Stamina::new(settings());
    let mut now = 10.0;
    stamina.wound(30.0, now);
    assert!((stamina.legs.current - 70.0).abs() < 1e-4, "{}", stamina.legs.current);
    // No pause: the refill goes on at once, 4.5 a second.
    run(&mut stamina, &mut now, 1.0);
    assert!((stamina.legs.current - 74.5).abs() < 0.05, "{}", stamina.legs.current);
    assert_eq!(stamina.arms.current, 80.0, "the arms pay nothing");
}

#[test]
fn a_fast_switch_costs_the_arms_and_the_legs_what_the_arms_lack() {
    let mut stamina = Stamina::new(settings());
    let now = 10.0;
    assert_eq!(stamina.fast_switch(0, now), 0.0);
    assert!((stamina.arms.current - 65.0).abs() < 1e-4 && stamina.legs.current == 100.0);
    // At the class skill's level 50, half.
    stamina.fast_switch(50, now);
    assert!((stamina.arms.current - 57.5).abs() < 1e-4);
    // Arms with less than the cost: the legs pay it all, and the lack is
    // the whole charge (the legs' reading of it, after the arms paid).
    stamina.arms.current = 5.0;
    let lack = stamina.fast_switch(0, now);
    assert!((lack - 15.0).abs() < 1e-4 && stamina.arms.current == 0.0, "{lack}");
    assert!((stamina.legs.current - 85.0).abs() < 1e-4, "{}", stamina.legs.current);
}

#[test]
fn a_sprint_drains_the_legs_and_ends_when_they_are_spent() {
    let mut stamina = Stamina::new(settings());
    let mut now = 10.0;
    assert!(stamina.sprint(true, false, now));
    run(&mut stamina, &mut now, 5.0);
    assert!((stamina.legs.current - 80.0).abs() < 0.05, "{}", stamina.legs.current);
    assert!((stamina.oxygen.current - 330.0).abs() < 0.05, "the oxygen pays as well: {}", stamina.oxygen.current);
    // Empty 25 s in.
    run(&mut stamina, &mut now, 20.1);
    assert_eq!(stamina.legs.current, 0.0);
    assert!(!stamina.sprinting(), "spent legs end the sprint");
    assert!(!stamina.sprint(true, false, now), "and do not start another");
    // The pause is the sprint's second and a second more for an empty
    // pool, from the sprint's last frame; then 4.5 a second.
    run(&mut stamina, &mut now, 1.8);
    assert_eq!(stamina.legs.current, 0.0);
    run(&mut stamina, &mut now, 3.6);
    assert!(stamina.legs.current > EXHAUSTED && stamina.legs.current < 17.0, "{}", stamina.legs.current);
    assert!(stamina.sprint(true, false, now), "more than the threshold starts one again");
}

/// A berserk (`Physical.BerserkRestorationFactor`, `Stamina.ForceMode`):
/// the legs and the arms pay nothing, let every start through and
/// refill at once; the oxygen pays as ever; a fast switch's lack is the
/// last one paid for; over, the pools pay again.
#[test]
fn a_berserk_frees_the_legs_and_the_arms() {
    let mut stamina = Stamina::new(settings());
    let mut now = 10.0;
    stamina.arms.current = 5.0;
    assert!((stamina.fast_switch(0, now) - 15.0).abs() < 1e-4);
    assert!((stamina.legs.current - 85.0).abs() < 1e-4);
    // Spent legs and arms, their refill held off long.
    stamina.legs.current = 0.0;
    stamina.legs.hold_refill_until(100.0);
    assert!(!stamina.can_jump() && !stamina.can_vault(true) && !stamina.can_melee());
    stamina.set_berserk(true);
    assert!(stamina.legs.forced() && stamina.arms.forced() && !stamina.oxygen.forced());
    assert!(stamina.can_jump() && stamina.can_vault(true) && stamina.can_melee());
    // A jump: nothing off the legs, its cost off the oxygen.
    stamina.jump(now);
    assert_eq!(stamina.legs.current, 0.0);
    assert!((stamina.oxygen.current - 336.0).abs() < 1e-3, "{}", stamina.oxygen.current);
    // A sprint fully overweight starts; the legs refill under it at
    // once (4.5 a second), the oxygen pays for it.
    stamina.reweigh(1.0, 1.0);
    assert!(stamina.sprint(true, false, now));
    let oxygen = stamina.oxygen.current;
    run(&mut stamina, &mut now, 1.0);
    assert!(stamina.legs.current > 4.0 && stamina.legs.current < 5.0, "{}", stamina.legs.current);
    assert!(stamina.oxygen.current < oxygen, "the oxygen pays");
    // A fast switch costs nothing and its lack is the last one's.
    let arms = stamina.arms.current;
    assert!((stamina.fast_switch(0, now) - 15.0).abs() < 1e-4);
    assert_eq!(stamina.arms.current, arms);
    // Over: the legs pay for the running sprint again.
    stamina.reweigh(0.0, 0.0);
    stamina.set_berserk(false);
    let legs = stamina.legs.current;
    run(&mut stamina, &mut now, 1.0);
    assert!(stamina.legs.current < legs, "{} from {legs}", stamina.legs.current);
}

/// The features' two switches (`crate::Systems`): with the stamina off
/// nothing is paid for; with exhaustion off the pools are paid from as
/// ever, and spent ones hold nothing back.
#[test]
fn the_stamina_and_exhaustion_switched_off() {
    let mut stamina = Stamina::new(settings());
    stamina.drains = false;
    let mut now = 10.0;
    assert!(stamina.sprint(true, false, now));
    stamina.jump(now);
    run(&mut stamina, &mut now, 30.0);
    assert_eq!([stamina.legs.current, stamina.oxygen.current], [stamina.legs.capacity, stamina.oxygen.capacity]);
    assert!(stamina.sprinting());

    let mut stamina = Stamina::new(settings());
    stamina.holds_back = false;
    let mut now = 10.0;
    assert!(stamina.sprint(true, false, now));
    run(&mut stamina, &mut now, 30.0);
    assert_eq!(stamina.legs.current, 0.0, "the legs pay as ever");
    assert!(stamina.sprinting(), "and the sprint goes on");
    assert!(stamina.can_jump() && stamina.can_vault(true) && !stamina.exhausted() && !stamina.legs_spent());
    assert!(stamina.hold_breath(true), "a breath is held on spent legs");
    assert_eq!((stamina.breath_stamina(), stamina.breath_oxygen()), (1.0, 1.0));
}

#[test]
fn sprints_started_in_quick_succession_cost_more_each_time() {
    let mut stamina = Stamina::new(settings());
    let mut now = 10.0;
    let mut costs = Vec::new();
    for _ in 0..4 {
        let before = stamina.legs.current;
        stamina.sprint(true, false, now);
        costs.push(before - stamina.legs.current);
        stamina.sprint(false, false, now);
        now += 0.5;
    }
    assert_eq!(costs, [0.0, 0.5, 1.0, 1.5]);
    now += 2.5;
    let before = stamina.legs.current;
    stamina.sprint(true, false, now);
    assert_eq!(before - stamina.legs.current, 0.0, "after two quiet seconds a start is free again");
}

#[test]
fn a_jump_needs_more_than_the_threshold_and_pauses_the_refill() {
    let mut stamina = Stamina::new(settings());
    let mut now = 10.0;
    stamina.jump(now);
    assert_eq!(stamina.legs.current, 86.0);
    assert_eq!(stamina.oxygen.current, 336.0);
    // A second, and 0.14 more for a pool 14% down.
    run(&mut stamina, &mut now, 1.13);
    assert_eq!(stamina.legs.current, 86.0);
    run(&mut stamina, &mut now, 1.0);
    assert!((stamina.legs.current - 90.4).abs() < 0.2, "{}", stamina.legs.current);
    stamina.legs.current = 15.0;
    assert!(!stamina.can_jump());
    stamina.legs.current = 15.1;
    assert!(stamina.can_jump());
}

#[test]
fn raising_the_stance_is_charged_by_the_third_of_a_notch() {
    let mut stamina = Stamina::new(settings());
    // Nothing until three notches' rise is in the tally, and then a
    // third of a notch's cost for every third of a notch more.
    stamina.stance_raised(0.49, 10.0);
    assert_eq!(stamina.legs.current, 100.0);
    stamina.stance_raised(0.01, 10.0);
    assert!((stamina.legs.current - (100.0 - 1.1 / 3.0)).abs() < 1e-4);
    stamina.stance_raised(0.05, 10.0);
    assert!((stamina.legs.current - (100.0 - 1.1 / 3.0)).abs() < 1e-4);
    stamina.stance_raised(0.01, 10.0);
    assert!((stamina.legs.current - (100.0 - 2.2 / 3.0)).abs() < 1e-4);
}

#[test]
fn aiming_drains_the_arms_and_then_the_legs() {
    let mut stamina = Stamina::new(settings());
    stamina.aim_mass = 4.0;
    let mut now = 10.0;
    stamina.aim(true);
    run(&mut stamina, &mut now, 10.0);
    assert!((stamina.arms.current - 60.0).abs() < 0.05, "two a second standing: {}", stamina.arms.current);
    assert_eq!(stamina.legs.current, 100.0);
    run(&mut stamina, &mut now, 31.0);
    assert_eq!(stamina.arms.current, 0.0);
    assert!(stamina.legs.current < 99.0, "spent arms pass the cost on to the legs: {}", stamina.legs.current);
    stamina.aim(false);
    let legs = stamina.legs.current;
    run(&mut stamina, &mut now, 3.0);
    assert!(stamina.legs.current > legs && stamina.arms.current > 0.0);
}

#[test]
fn the_rangefinder_aims_at_its_own_rate() {
    let mut settings = settings();
    settings.aim_rangefinder_drain = 0.55;
    let mut stamina = Stamina::new(settings);
    // Its aim weighs 1 (`Physical.Aim(1)`), at its own rate.
    stamina.aim_mass = 1.0;
    stamina.set_rangefinder(true);
    let mut now = 10.0;
    stamina.aim(true);
    run(&mut stamina, &mut now, 10.0);
    assert!((stamina.arms.current - 74.5).abs() < 0.05, "0.55 a second standing, from 80: {}", stamina.arms.current);
    // Put away, the rate is the weapons' again, taken afresh.
    stamina.set_rangefinder(false);
    let before = stamina.arms.current;
    run(&mut stamina, &mut now, 1.0);
    assert!((before - stamina.arms.current - 1.0).abs() < 1e-3, "1 a second: {}", before - stamina.arms.current);
}

#[test]
fn thirst_reaches_the_cost_of_aiming_only_when_it_is_worked_out_again() {
    let mut settings = settings();
    // Thirst doubles every drain at nothing drunk, from half down.
    settings.hydration_graph = Graph { lower_left: 2.0, left_plateau: 0.5, ..settings.hydration_graph };
    let mut stamina = Stamina::new(settings);
    stamina.aim_mass = 4.0;
    let mut now = 10.0;
    let spent = |stamina: &mut Stamina, now: &mut f32| {
        let before = stamina.arms.current;
        run(stamina, now, 1.0);
        before - stamina.arms.current
    };
    stamina.aim(true);
    assert!((spent(&mut stamina, &mut now) - 2.0).abs() < 1e-3);
    // Thirstier with the sights up: the cost stays as it was taken.
    stamina.settings.hydration = 0.25;
    stamina.aim(true);
    assert!((spent(&mut stamina, &mut now) - 2.0).abs() < 1e-3, "the sights were up already");
    // Down and up again takes it afresh; so does a change of stance.
    stamina.aim(false);
    stamina.aim(true);
    assert!((spent(&mut stamina, &mut now) - 3.0).abs() < 1e-3);
    stamina.settings.hydration = 1.0;
    assert!((spent(&mut stamina, &mut now) - 3.0).abs() < 1e-3);
    stamina.set_pose(false, 0.0);
    assert!((spent(&mut stamina, &mut now) - 1.5).abs() < 1e-3, "crouched, three quarters of two");
}

#[test]
fn a_held_breath_costs_oxygen_and_is_let_go_when_it_runs_out() {
    let mut stamina = Stamina::new(settings());
    let mut now = 10.0;
    assert!(stamina.hold_breath(true));
    run(&mut stamina, &mut now, 5.0);
    assert!((stamina.oxygen.current - 330.0).abs() < 0.05, "four a second on full legs: {}", stamina.oxygen.current);
    // Emptier legs make it dearer: twice as dear on empty ones.
    stamina.legs.current = 50.0;
    stamina.legs.hold_refill_until(1e9);
    let before = stamina.oxygen.current;
    run(&mut stamina, &mut now, 1.0);
    assert!((before - stamina.oxygen.current - 6.0).abs() < 0.05, "{}", before - stamina.oxygen.current);
    // Legs under the threshold: asking again changes nothing, and a
    // breath let go is not taken again.
    stamina.legs.current = 10.0;
    assert!(stamina.hold_breath(true), "a breath already held stays held");
    assert!(!stamina.hold_breath(false));
    assert!(!stamina.hold_breath(true), "spent legs do not hold one");
    // It ends by itself when the oxygen is gone.
    stamina.legs.current = 100.0;
    stamina.oxygen.current = 16.0;
    assert!(stamina.hold_breath(true));
    run(&mut stamina, &mut now, 5.0);
    assert!(!stamina.holding_breath());
    assert!(!stamina.hold_breath(true), "nor does oxygen at the threshold or under");
    // What the weapon's breathing goes by.
    stamina.arms.current = 40.0;
    stamina.legs.current = 25.0;
    assert!((stamina.breath_stamina() - 0.25).abs() < 1e-6, "the lower of arms and legs here");
    stamina.arms.current = 0.0;
    assert_eq!(stamina.breath_stamina(), 0.05);
}

#[test]
fn getting_up_costs_legs_arms_and_oxygen() {
    let mut stamina = Stamina::new(settings());
    stamina.stand_up(10.0);
    assert_eq!((stamina.legs.current, stamina.arms.current, stamina.oxygen.current), (90.0, 70.0, 340.0));
    // Arms that cannot pay it all leave the legs to pay it again.
    stamina.arms.current = 4.0;
    stamina.stand_up(10.0);
    assert_eq!((stamina.legs.current, stamina.arms.current), (70.0, 0.0));
}

#[test]
fn a_crouched_walk_costs_and_an_upright_one_does_not() {
    let mut stamina = Stamina::new(settings());
    let mut now = 10.0;
    stamina.moving(true, false);
    run(&mut stamina, &mut now, 10.0);
    assert_eq!(stamina.legs.current, 100.0);
    stamina.set_pose(false, 0.0);
    run(&mut stamina, &mut now, 10.0);
    assert!((stamina.legs.current - 98.3).abs() < 0.01, "{}", stamina.legs.current);
    stamina.moving(false, false);
    run(&mut stamina, &mut now, 1.0);
    assert_eq!(stamina.legs.current, 100.0, "and comes back at the crouch's faster rate");
}
