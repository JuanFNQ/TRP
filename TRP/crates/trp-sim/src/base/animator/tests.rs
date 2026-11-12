//! Tests of the animator's transitions, conditions and layers.
use super::*;
use trp_data::weapons::states::Parameter;

fn state(name: &str, transitions: Vec<TransitionDef>) -> StateDef {
    StateDef {
        name: name.to_owned(),
        speed: 1.0,
        speed_parameter: None,
        looping: false,
        writes_defaults: true,
        cycle_offset: 0.0,
        // No motion: such a state lasts one second.
        trees: vec![None],
        transitions,
        movement: None,
        layer_weights: Vec::new(),
        loaded: true,
    }
}

fn to(target: Target, conditions: Vec<Condition>) -> TransitionDef {
    TransitionDef { target, duration: 0.0, fixed: true, offset: 0.0, exit_time: None, conditions, interruption: 0 }
}

fn when(parameter: usize, mode: i32, threshold: f32) -> Condition {
    Condition { parameter: Some(parameter), mode, threshold }
}

/// One layer over the given states, with a boolean `go`, a trigger
/// `fire` and a float `level`.
fn animator(states: Vec<StateDef>, any_state: Vec<TransitionDef>, selectors: Vec<Vec<(Target, Vec<Condition>)>>) -> Animator {
    let parameters = vec![
        Parameter { name: "go".into(), kind: ParameterKind::Bool, default: 0.0 },
        Parameter { name: "fire".into(), kind: ParameterKind::Trigger, default: 0.0 },
        Parameter { name: "level".into(), kind: ParameterKind::Float, default: 0.0 },
    ];
    let by_name = parameters.iter().enumerate().map(|(i, p)| (p.name.clone(), i)).collect();
    let controller = Controller {
        curve_driven: vec![false; parameters.len()],
        parameters,
        by_name,
        machines: vec![MachineDef { states, any_state, selectors, default_state: 0, keeps_weights: false }],
        layers: vec![LayerDef { name: "base".into(), machine: 0, slot: 0, default_weight: 1.0, additive: false, moves: Vec::new() }],
        clips: Vec::new(),
        bones: Vec::new(),
        root_bone: None,
        scaled: Vec::new(),
        followed: Vec::new(),
        carried: Vec::new(),
        always: Vec::new(),
        always_list: Vec::new(),
    };
    Animator::new(Arc::new(controller))
}

const DT: f32 = 0.05;

#[test]
fn a_transition_waits_for_its_moment_and_then_blends() {
    // From the first state to the second, half way through the first,
    // blending for a fifth of a second.
    let wait = TransitionDef { exit_time: Some(0.5), duration: 0.2, ..to(Target::State(1), Vec::new()) };
    let mut a = animator(vec![state("first", vec![wait]), state("second", Vec::new())], Vec::new(), Vec::new());
    let (mut entered, mut left) = (None, None);
    for step in 1..=20 {
        for event in a.update(DT) {
            match event {
                Event::Entered { state: 1, .. } => entered = Some(step as f32 * DT),
                Event::Left { state: 0, .. } => left = Some(step as f32 * DT),
                _ => {}
            }
        }
        // While the two blend, both are reported and the weight rises.
        if entered.is_some() && left.is_none() {
            assert_eq!(a.state(0).1.map(|s| s.name.as_str()), Some("second"));
            assert!(a.progress(0).1 < 1.0);
        }
    }
    // The new state is entered when the blend starts and the old one
    // left when it ends.
    assert!((entered.unwrap() - 0.5).abs() <= DT, "{entered:?}");
    assert!((left.unwrap() - entered.unwrap() - 0.2).abs() <= DT, "{left:?}");
    assert_eq!(a.state(0).0.name, "second");
}

#[test]
fn conditions_hold_a_transition_back_and_a_trigger_is_used_up() {
    let fire = to(Target::State(1), vec![when(1, 1, 0.0)]);
    let back = to(Target::State(0), vec![when(0, 2, 0.0), when(2, 3, 0.5)]);
    let mut a = animator(vec![state("rest", vec![fire]), state("act", vec![back])], Vec::new(), Vec::new());
    a.update(DT);
    assert_eq!(a.state(0).0.name, "rest");
    a.set_bool("fire", true);
    a.update(DT);
    assert_eq!(a.state(0).0.name, "act");
    assert_eq!(a.float("fire"), 0.0, "the trigger was not cleared");
    // Back needs the flag down and the level above a half; both hold only now.
    a.update(DT);
    assert_eq!(a.state(0).0.name, "act");
    a.set_float("level", 0.75);
    a.update(DT);
    assert_eq!(a.state(0).0.name, "rest");
}

#[test]
fn a_way_from_any_state_can_pass_through_a_choice_of_entries() {
    // From anywhere, when `go` is set: to an entry point that picks the
    // third state at a high level and the second otherwise.
    let any = to(Target::Selector(0), vec![when(0, 1, 0.0)]);
    let choice = vec![(Target::State(2), vec![when(2, 3, 0.5)]), (Target::State(1), Vec::new())];
    let states = || vec![state("rest", Vec::new()), state("low", Vec::new()), state("high", Vec::new())];
    let mut a = animator(states(), vec![any.clone()], vec![choice.clone()]);
    a.set_bool("go", true);
    a.update(DT);
    assert_eq!(a.state(0).0.name, "low");
    // Already there, the same way does not lead back into it.
    assert!(a.update(DT).is_empty());

    let mut a = animator(states(), vec![any], vec![choice]);
    a.set_bool("go", true);
    a.set_float("level", 1.0);
    a.update(DT);
    assert_eq!(a.state(0).0.name, "high");
}

#[test]
fn a_layer_is_taken_up_where_a_recording_has_it() {
    // A blend under way, as a run begins.
    let blend = TransitionDef { duration: 0.4, ..to(Target::State(1), Vec::new()) };
    let mut a = animator(vec![state("rest", vec![blend]), state("Base Layer.Breath", Vec::new())], Vec::new(), Vec::new());
    a.update(DT);
    assert!(a.state(0).1.is_some());
    // The recorder writes the low 24 bits of the hash of the state's
    // own name, the name without its layer's.
    let hash = path_hash("Breath") & 0x00ff_ffff;
    assert_eq!(a.state_by_short_hash(0, hash), Some(1));
    assert_eq!(a.state_by_short_hash(0, hash ^ 1), None);
    // Put there outright: no blend left, and the next update plays on
    // from the figure given.
    a.start_in(0, 1, 3.7);
    assert_eq!(a.state(0).0.name, "Base Layer.Breath");
    assert!(a.state(0).1.is_none());
    assert_eq!(a.progress(0), (3.7, 0.0));
    a.update(DT);
    assert!((a.progress(0).0 - (3.7 + DT)).abs() < 1e-6, "{:?}", a.progress(0));
    assert!(a.owns_machine(0) && !a.owns_machine(1));
    assert_eq!(a.layer_count(), 1);
}
