//! Tests of the sounds of moving: footsteps and their lists, the kit, landings and turns.
use super::*;

fn bank(blend: Option<Vec<f32>>, lists: [Vec<usize>; 3]) -> Bank {
    Bank { name: "b".into(), base_volume: 0.68, base_pitch: 1.0, delta_volume: 0.05, delta_pitch: 0.15, rolloff: 52.5, blend, lists, source_type: 4, physical: false, limited: false, clip_length: 0.0 }
}

/// Concrete's run bank: no clip in its first list, the walk in its
/// second, the run in its third, blended from 0.6 to 1.01.
fn run_bank() -> Bank {
    bank(Some(vec![-0.2, 0.4, 0.6, 1.01]), [vec![], (0..6).collect(), (6..12).collect()])
}

#[test]
fn a_footstep_chooses_its_list_by_its_blend_value() {
    let (bank, mut shuffle, mut rng) = (run_bank(), Shuffle::default(), Rng::new(1));
    // Slow: the first list is empty and the second's clip plays whole.
    let slow = choose(&bank, &mut shuffle, 0.3, &mut rng);
    assert!(slow.clips[0].is_none() && slow.clips[1].is_some_and(|c| c < 6));
    assert!((slow.share - inverse_lerp(0.4, -0.2, 0.3)).abs() < 1e-6);
    // Between 0.4 and 0.6, a walking clip alone.
    let walk = choose(&bank, &mut shuffle, 0.5, &mut rng);
    assert!(walk.clips[0].is_some_and(|c| c < 6) && walk.clips[1].is_none() && walk.share == 1.0);
    // Between 0.6 and 1.01, a walking and a running clip, the walk's
    // share falling.
    let both = choose(&bank, &mut shuffle, 0.9, &mut rng);
    assert!(both.clips[0].is_some_and(|c| c < 6) && both.clips[1].is_some_and(|c| c >= 6));
    assert!((both.share - (1.01 - 0.9) / 0.41).abs() < 1e-5);
    // The pitch within its spread.
    assert!((both.pitch - 1.0).abs() <= 0.15);
}

/// The shuffle gives every clip of a list once before any again, and
/// two clips simply alternate (its swap never draws the last place).
#[test]
fn the_shuffle_goes_through_a_list_and_two_alternate() {
    let (mut shuffle, mut rng) = (Shuffle::default(), Rng::new(7));
    let mut seen: Vec<usize> = (0..6).map(|_| next_index(&mut shuffle, 6, &mut rng)).collect();
    seen.sort();
    assert_eq!(seen, (0..6).collect::<Vec<_>>());
    let mut two = Shuffle::default();
    let order: Vec<usize> = (0..6).map(|_| next_index(&mut two, 2, &mut rng)).collect();
    assert_eq!(order, [0, 1, 0, 1, 0, 1]);
}

fn moment(clock: f32, state: State) -> Moment {
    Moment {
        clock,
        dt: 1.0 / 120.0,
        state,
        footstep: false,
        crawl_marks: 0,
        landed: None,
        prone: false,
        pose_level: 1.0,
        smoothed_pose_level: 1.0,
        clamped_speed: 1.0,
        speed: 1.0,
        max_speed: 1.0,
        speed_set: false,
        overweight: 0.0,
        walk_overweight: 0.0,
        leg_damaged: false,
        yaw: 0.0,
        hands_to_body: 0.0,
    }
}

/// One's own quiet is worked out only where the movement's speed is set
/// (`SetCharacterMovementSpeed`): a walk held slow by a state's limit
/// keeps its quiet once the limit is gone, until the speed is set again
/// (`courses2`'s `7-sound-moving`); another's follows the clamped speed
/// on every frame.
#[test]
fn ones_own_quiet_waits_for_the_speed_to_be_set() {
    let mut levels = crate::condition::skills::SkillLevels::default();
    levels.set_level(crate::condition::skills::Skill::CovertMovement, 40);
    let covert = levels.buffs().covert;
    let run = |third_person: bool| {
        let mut sounds = StepSounds::default();
        sounds.third_person = third_person;
        sounds.set_covert(covert);
        let mut last = 0.0;
        let at = |clock: f32, clamped_speed: f32, speed_set: bool| Moment { clamped_speed, speed_set, ..moment(clock, State::Move) };
        sounds.update(&at(1.0, 0.3, true), &mut last, &mut || None);
        let slow = sounds.covert_efficiency();
        sounds.update(&at(1.1, 1.0, false), &mut last, &mut || None);
        let after = sounds.covert_efficiency();
        sounds.update(&at(1.2, 1.0, true), &mut last, &mut || None);
        (slow, after, sounds.covert_efficiency())
    };
    let (slow, kept, set) = run(false);
    assert_eq!(slow, 1.0);
    assert_eq!(kept, slow);
    assert_eq!(set, 0.0);
    let (slow, after, _) = run(true);
    assert_eq!(slow, 1.0);
    assert_eq!(after, 0.0);
}

/// A footstep at the top speed, unburdened: the run's 0.85 with the
/// speed share of 1 (the first footstep: as the source was made), the
/// bank's 0.68 less up to 0.05; and the kit 0.1 s later at 0.5 times
/// the gear's 0.5.
#[test]
fn a_footstep_and_its_kit() {
    let mut sounds = StepSounds::default();
    sounds.set_figures(vec![(CONCRETE, std::array::from_fn(|_| Some(Figures { base_volume: 0.68, delta_volume: 0.05 })))]);
    let mut last = 0.0;
    sounds.update(&moment(1.0, State::Move), &mut last, &mut || Some(CONCRETE));
    let mut walking = moment(1.2, State::Move);
    walking.footstep = true;
    sounds.update(&walking, &mut last, &mut || Some(CONCRETE));
    let asked = sounds.take();
    assert_eq!(asked.len(), 1);
    let step = asked[0];
    assert_eq!(step.bank, BankOf::Surface(CONCRETE, RUN));
    assert!(step.volume <= 0.85 * 0.68 + 1e-6 && step.volume >= 0.85 * 0.63 - 1e-6, "{}", step.volume);
    assert_eq!(step.blend, 1.0);
    sounds.update(&moment(1.25, State::Move), &mut last, &mut || None);
    assert!(sounds.take().is_empty());
    sounds.update(&moment(1.31, State::Move), &mut last, &mut || None);
    let kit = sounds.take();
    assert_eq!(kit.len(), 1);
    assert!(kit[0].kit && kit[0].bank == BankOf::Kit(GEAR));
    assert!((kit[0].volume - 0.5 * 0.5).abs() < 1e-6, "{}", kit[0].volume);
}

/// Covert Movement at level 40, walking slowly enough to walk quietly
/// in full: a step on a soft surface (concrete) 48% quieter and its
/// blend with it, on a loud one (metal) 24% (`CovertMovementVolumeBy
/// Speed`).
#[test]
fn covert_movement_quiets_a_quiet_step() {
    let mut levels = crate::condition::skills::SkillLevels::default();
    levels.set_level(crate::condition::skills::Skill::CovertMovement, 40);
    let covert = levels.buffs().covert;
    let step = |covert: Option<crate::condition::skills::Covert>, surface: u8| {
        let mut sounds = StepSounds::default();
        if let Some(covert) = covert {
            sounds.set_covert(covert);
        }
        sounds.set_figures(vec![(surface, std::array::from_fn(|_| Some(Figures { base_volume: 0.68, delta_volume: 0.0 })))]);
        let mut last = 0.0;
        let slow = |clock: f32, footstep: bool| Moment { clamped_speed: 0.15, speed: 0.15, footstep, ..moment(clock, State::Move) };
        sounds.update(&slow(1.0, false), &mut last, &mut || Some(surface));
        sounds.update(&slow(1.2, true), &mut last, &mut || Some(surface));
        let asked = sounds.take();
        (asked[0].volume, asked[0].blend)
    };
    let ((plain, plain_blend), (quiet, quiet_blend)) = (step(None, CONCRETE), step(Some(covert), CONCRETE));
    assert!((quiet / plain - 0.52).abs() < 1e-5, "{quiet} against {plain}");
    assert!((quiet_blend / plain_blend - 0.52).abs() < 1e-5, "{quiet_blend} against {plain_blend}");
    let (loud_plain, loud) = (step(None, 1).0, step(Some(covert), 1).0);
    assert!((loud / loud_plain - 0.76).abs() < 1e-5, "{loud} against {loud_plain}");
}

/// A walk left with no footstep heard in it plays one and counts it;
/// a sprint left for its stop plays the stop.
#[test]
fn leaving_a_walk_and_a_sprint() {
    let mut sounds = StepSounds::default();
    let mut last = 0.0;
    sounds.update(&moment(1.0, State::Move), &mut last, &mut || None);
    sounds.update(&moment(1.3, State::Idle), &mut last, &mut || None);
    let asked = sounds.take();
    assert!(asked.iter().any(|play| play.bank == BankOf::Surface(CONCRETE, RUN)));
    assert_eq!(last, 1.3);
    sounds.update(&moment(2.0, State::Sprint), &mut last, &mut || None);
    let mut step = moment(2.3, State::Sprint);
    step.footstep = true;
    sounds.update(&step, &mut last, &mut || None);
    sounds.take();
    sounds.update(&moment(2.6, State::SprintStop), &mut last, &mut || None);
    let asked = sounds.take();
    assert_eq!(asked.len(), 1);
    assert_eq!(asked[0].bank, BankOf::Surface(CONCRETE, STOP));
}

/// The landing: heard over 0.3 m, whole from 0.75 m; a turn to the
/// right standing is heard, one to the left is not.
#[test]
fn a_landing_and_the_turns() {
    let mut sounds = StepSounds::default();
    let mut last = 0.0;
    sounds.update(&moment(0.5, State::Jump), &mut last, &mut || None);
    let mut landed = moment(1.0, State::Jump);
    landed.landed = Some((0.2, 0.0));
    sounds.update(&landed, &mut last, &mut || None);
    assert!(sounds.take().iter().all(|play| play.bank != BankOf::Surface(CONCRETE, LANDING)));
    landed.landed = Some((1.0, -0.5));
    landed.clock = 2.0;
    sounds.update(&landed, &mut last, &mut || None);
    let asked = sounds.take();
    let land = asked.iter().find(|play| play.bank == BankOf::Surface(CONCRETE, LANDING)).expect("a landing");
    assert!((land.volume - 1.0).abs() < 1e-6);
    for (sign, heard) in [(1.0, true), (-1.0, false)] {
        let mut sounds = StepSounds::default();
        for frame in 0..20 {
            let mut idle = moment(10.0 + frame as f32 / 120.0, State::Idle);
            idle.yaw = sign * frame as f32 * 3.0;
            idle.hands_to_body = if frame >= 10 { sign * 50.0 } else { 0.0 };
            sounds.update(&idle, &mut last, &mut || None);
        }
        let turn = sounds.take().into_iter().find(|play| play.bank == BankOf::Surface(CONCRETE, TURN)).expect("a turn tried");
        assert_eq!(turn.volume > 0.0, heard, "{sign}: {}", turn.volume);
    }
}

/// A character seen from outside past its surface check and a metre
/// from the listener plays no step and no kit after it; within it, both;
/// one's own at any distance (`CheckSurface`; `courses3`'s `3-bot-watch`:
/// a Scav's steps heard to 52.6 m and none past 53.5 m).
#[test]
fn a_step_out_of_the_listener_s_range_is_not_played() {
    let mut settings = StepSettings::default();
    settings.widen_surface_checks([(RUN, 52.5), (RUN, 30.0), (SPRINT, 66.5), (TURN, 99.0)]);
    assert_eq!(settings.surface_checks, [52.5, 66.5, 40.0, 30.0]);
    let steps = |listener: Option<f32>, third_person: bool| {
        let mut sounds = StepSounds::new(settings.clone());
        sounds.third_person = third_person;
        sounds.listener = listener;
        let mut last = 0.0;
        sounds.update(&moment(1.0, State::Move), &mut last, &mut || Some(CONCRETE));
        let mut walking = moment(1.2, State::Move);
        walking.footstep = true;
        sounds.update(&walking, &mut last, &mut || Some(CONCRETE));
        sounds.update(&moment(1.31, State::Move), &mut last, &mut || None);
        sounds.take().len()
    };
    assert_eq!(steps(Some(53.4), true), 2);
    assert_eq!(steps(Some(53.5), true), 0);
    assert_eq!(steps(None, false), 2);
}
