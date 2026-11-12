//! Vaults and climbs in a yard of walls and boxes.

use super::*;

/// Level ground with boxes on it, each endlessly wide and standing from
/// `near` to `far` metres ahead and from `low` to `high`, answering as a
/// host's world does: a line sees a box only from outside, a box's top is
/// ground, and the character is kept its capsule's reach from a box's
/// faces unless its top is a step up at most or lower than the capsule's
/// underside.
pub(super) struct Yard(pub(super) Vec<[f32; 4]>);

impl trp_sim::World for Yard {
    fn ground_height(&self, _x: f32, z: f32) -> f32 {
        self.0.iter().filter(|b| z >= b[0] && z <= b[1]).map(|b| b[3]).fold(0.0, f32::max)
    }

    fn line(&self, from: [f32; 3], to: [f32; 3]) -> Option<f32> {
        let length = (0..3).map(|k| (to[k] - from[k]).powi(2)).sum::<f32>().sqrt();
        let mut nearest: Option<f32> = (from[1] >= 0.0 && to[1] < 0.0).then(|| from[1] / (from[1] - to[1]));
        for [near, far, low, high] in &self.0 {
            let (mut enter, mut leave, mut misses) = (0.0f32, 1.0f32, false);
            for (k, (a, b)) in [(2, (*near, *far)), (1, (*low, *high))] {
                let (start, travel) = (from[k], to[k] - from[k]);
                if travel.abs() < 1e-9 {
                    misses |= start <= a || start >= b;
                    continue;
                }
                let (t0, t1) = ((a - start) / travel, (b - start) / travel);
                enter = enter.max(t0.min(t1));
                leave = leave.min(t0.max(t1));
            }
            if !misses && enter <= leave && enter > 0.0 {
                nearest = Some(nearest.map_or(enter, |n| n.min(enter)));
            }
        }
        nearest.map(|share| share * length)
    }

    fn terrain(&self, at: [f32; 3]) -> bool {
        at[1] <= 1e-3
    }

    /// The character is kept out of the boxes as the game's own controller
    /// keeps it (movement spec, "The character's own collision").
    fn obstructed(&self, from: [f32; 3], to: [f32; 3], capsule: trp_sim::locomotion::movement::Capsule) -> [f32; 3] {
        let settled = self.course().settle([from[2], from[1]], [to[2], to[1]], capsule, CAPSULE_RADIUS);
        [to[0], settled[1], settled[0]]
    }

    fn sets_feet(&self) -> bool {
        true
    }

    fn held(&self, at: [f32; 3], capsule: trp_sim::locomotion::movement::Capsule) -> bool {
        self.course().held([at[2], at[1]], capsule, CAPSULE_RADIUS)
    }
}

/// The game's capsule, from the player prefab.
const CAPSULE_RADIUS: f32 = 0.366;

impl Yard {
    /// The yard as a line of ground: its boxes, which stand on the ground
    /// one after another, and the ground between them.
    pub(super) fn course(&self) -> trp_sim::locomotion::collision::Course {
        use trp_sim::locomotion::collision::Tread;
        let mut boxes: Vec<[f32; 4]> = self.0.clone();
        boxes.sort_by(|a, b| a[0].total_cmp(&b[0]));
        let (mut treads, mut at) = (Vec::new(), f32::MIN);
        for [near, far, _, high] in boxes {
            treads.push(Tread::level(at, near, 0.0));
            treads.push(Tread::level(near, far, high));
            at = far;
        }
        treads.push(Tread::level(at, f32::MAX, 0.0));
        trp_sim::locomotion::collision::Course { treads }
    }
}

/// Runs a frame at a time at the recordings' rate until `done` says so or
/// `seconds` have passed; gives back the states passed through, in order,
/// and how long it took.
pub(super) fn until(m: &mut Movement, world: &dyn trp_sim::World, input: Input, seconds: f32, mut done: impl FnMut(&Movement) -> bool) -> (Vec<State>, f32) {
    let frame = 1.0 / 120.0;
    let mut states = vec![m.state];
    for index in 0..(seconds / frame) as usize {
        m.step(frame, &input, world);
        if states.last() != Some(&m.state) {
            states.push(m.state);
        }
        if done(m) {
            return (states, (index + 1) as f32 * frame);
        }
    }
    (states, seconds)
}

/// A vault and a climb sound their marks (footsteps spec, "The vault's
/// sounds"): the moves' states pass their four marks in the order and at
/// the spacing the body's states have them (a climb's grab before its
/// start), and each asks the surface's element at the obstacle's height,
/// with the move's kit bank at the grab and at the end. The game's own
/// sound is not compared here: the `courses2` vault recording is set
/// beside the rules in the spec.
#[test]
fn a_vault_and_a_climb_sound_their_marks() {
    use trp_sim::sound::steps::{BankOf, Play};
    let held = Input { move_dir: [0.0, 1.0], vault: true, ..Input::default() };
    let frame = 1.0 / 120.0;
    let gather = |m: &mut Movement, world: &dyn trp_sim::World| -> Vec<(f32, Play)> {
        let mut asked = Vec::new();
        let mut clock = 0.0;
        for _ in 0..(5.0 / frame) as usize {
            m.step(frame, &held, world);
            clock += frame;
            asked.extend(m.sounds.take().into_iter().filter(|play| matches!(play.bank, BankOf::Vault { .. } | BankOf::VaultGear(_))).map(|play| (clock, play)));
        }
        asked
    };
    let marks = |asked: &[(f32, Play)]| -> Vec<(usize, Option<usize>)> {
        asked.iter()
            .map(|(_, play)| match play.bank {
                BankOf::Vault { kind, mark, .. } => (kind, Some(mark)),
                BankOf::VaultGear(kind) => (kind, None),
                _ => unreachable!(),
            })
            .collect()
    };

    // The wall of the vault above: start, the kit and the grab, the
    // slide, the kit and the end, at 0.10, 0.15, 0.30 and 0.55 of the
    // state at its pace.
    let Some(mut m) = movement(0.0) else { return };
    let wall = Yard(vec![[0.5, 0.85, 0.0, 1.0]]);
    let asked = gather(&mut m, &wall);
    println!("the vault's marks: {:?}", asked.iter().map(|(t, play)| format!("{t:.3} {:?} {:.3}", play.bank, play.volume)).collect::<Vec<_>>());
    assert_eq!(marks(&asked), [(0, Some(0)), (0, None), (0, Some(1)), (0, Some(2)), (0, None), (0, Some(3))], "{asked:?}");
    let (figures, pace, _) = m.vault_parameters();
    let height = figures[0];
    for (_, play) in &asked {
        match play.bank {
            BankOf::Vault { surface, .. } => {
                assert_eq!(surface, trp_sim::sound::steps::CONCRETE);
                assert!((play.volume - height).abs() < 1e-5, "the height goes as the volume: {} against {height}", play.volume);
            }
            _ => assert!((play.volume - (0.1 + 0.9 * height.clamp(0.0, 1.0))).abs() < 1e-5, "the kit's share: {}", play.volume),
        }
        assert_eq!(play.blend, 0.0);
    }
    // The state's length as played is its clip's at this height (the
    // blend tree's `Jump_over_100` here) at the pace: taken from the
    // start-to-end gap, 0.45 of it, and the other marks set against that.
    let gap = |a: usize, b: usize| asked[b].0 - asked[a].0;
    let state_length = gap(0, 5) / 0.45;
    println!("the vault's state plays {state_length:.3} s at a pace of {pace:.3}");
    assert!((0.9..2.0).contains(&state_length), "{state_length}");
    assert!((gap(0, 2) - 0.05 * state_length).abs() < 2.5 * frame, "start to grab {:.3} against {:.3}", gap(0, 2), 0.05 * state_length);
    assert!((gap(0, 3) - 0.20 * state_length).abs() < 2.5 * frame, "start to slide {:.3} against {:.3}", gap(0, 3), 0.20 * state_length);
    assert_eq!(gap(1, 2), 0.0, "the kit and the grab on one frame");
    assert_eq!(gap(4, 5), 0.0, "the kit and the end on one frame");

    // The box of the climb: the kit and the grab first (0.25), then the
    // start (0.30), the slide (0.35), the kit and the end (0.50).
    let Some(mut m) = movement(0.0) else { return };
    let crate_ = Yard(vec![[0.5, 2.5, 0.0, 1.0]]);
    let asked = gather(&mut m, &crate_);
    println!("the climb's marks: {:?}", asked.iter().map(|(t, play)| format!("{t:.3} {:?} {:.3}", play.bank, play.volume)).collect::<Vec<_>>());
    assert_eq!(marks(&asked), [(2, None), (2, Some(1)), (2, Some(0)), (2, Some(2)), (2, None), (2, Some(3))], "{asked:?}");
    let (_, pace, _) = m.vault_parameters();
    let gap = |a: usize, b: usize| asked[b].0 - asked[a].0;
    let state_length = gap(1, 5) / 0.25;
    println!("the climb's state plays {state_length:.3} s at a pace of {pace:.3}");
    assert!((0.9..2.5).contains(&state_length), "{state_length}");
    assert!((gap(1, 2) - 0.05 * state_length).abs() < 2.5 * frame, "grab to start {:.3} against {:.3}", gap(1, 2), 0.05 * state_length);
    assert!((gap(1, 3) - 0.10 * state_length).abs() < 2.5 * frame, "grab to slide {:.3} against {:.3}", gap(1, 3), 0.10 * state_length);
    assert_eq!(gap(0, 1), 0.0, "the kit and the grab on one frame");
    assert_eq!(gap(4, 5), 0.0, "the kit and the end on one frame");
}

/// A wall is vaulted and a box climbed, by the game's own clips (vaulting
/// spec). Nothing here is compared with the game: no recording has one.
#[test]
fn a_wall_is_vaulted_and_a_box_climbed() {
    let Some(mut m) = movement(0.0) else { return };
    let held = Input { move_dir: [0.0, 1.0], vault: true, ..Input::default() };
    let rest = |m: &Movement| !m.is_vaulting && matches!(m.state, State::Idle | State::Move);

    // A wall a metre high and 0.35 m thick, its face half a metre ahead:
    // from standing, with forward and the key held.
    let wall = Yard(vec![[0.5, 0.85, 0.0, 1.0]]);
    let (legs, arms) = (m.stamina.legs.current, m.stamina.arms.current);
    let mut highest = 0.0f32;
    let (states, _) = until(&mut m, &wall, held, 1.0, |m| m.state == State::Vault);
    assert_eq!(states.last(), Some(&State::Vault), "{states:?}; measured {:?}", m.vaulting.obstacle);
    let started = m.position;
    println!(
        "the wall as measured: {:?}; the arms pay {:.1}, the legs {:.1}",
        m.vaulting.obstacle,
        arms - m.stamina.arms.current,
        legs - m.stamina.legs.current
    );
    assert!((arms - m.stamina.arms.current - 5.0).abs() < 0.05, "a hand is taken over 0.65 m");
    assert_eq!(legs, m.stamina.legs.current, "standing and unburdened, the legs pay nothing");
    assert!(!m.can_press_trigger() && m.take_aim_taken(), "no trigger, and the sights are taken down");
    let capsule = m.capsule();
    println!("the capsule in the vault: {:.3} to {:.3} m over the feet", capsule.underside, capsule.top);
    assert!(capsule.underside > 1.0, "clear of the wall's top: {capsule:?}");
    // The view does not answer the mouse while the vault's state lasts.
    let (yaw, pitch) = (m.yaw, m.pitch);
    let turning = Input { look_delta: [2.0, 1.0], ..held };
    let mut turned_in_vault = 0.0f32;
    let (states, took) = until(&mut m, &wall, turning, 4.0, |m| {
        highest = highest.max(m.position[1]);
        if m.state == State::Vault {
            turned_in_vault = (m.yaw - yaw).abs().max((m.pitch - pitch).abs());
        }
        rest(m)
    });
    println!(
        "the vault: {took:.2} s through {states:?}; {:.2} m forward, the feet up to {highest:.3} m; ends at {:.2} m, {:.3} m up",
        m.position[2] - started[2],
        m.position[2],
        m.position[1]
    );
    assert_eq!(states[..2], [State::Vault, State::VaultLanding], "{states:?}");
    assert!(m.position[2] > 0.85 + 0.3, "beyond the wall: {:?}", m.position);
    assert!(m.position[1].abs() < 0.09 && highest < 0.3, "the feet hardly leave the ground's level: {highest}");
    assert_eq!(turned_in_vault, 0.0, "the view is the move's own");
    assert!((m.yaw - yaw).abs() > 1.0, "and free again after it: {}", m.yaw - yaw);
    assert!(m.can_press_trigger());

    // A box a metre high and two deep: climbed, onto its top.
    let Some(mut m) = movement(0.0) else { return };
    let crate_ = Yard(vec![[0.5, 2.5, 0.0, 1.0]]);
    let arms = m.stamina.arms.current;
    let (states, _) = until(&mut m, &crate_, held, 1.0, |m| m.state == State::Climb);
    assert_eq!(states.last(), Some(&State::Climb), "{states:?}; measured {:?}", m.vaulting.obstacle);
    assert!((arms - m.stamina.arms.current - 8.0).abs() < 0.05, "one hand: {}", arms - m.stamina.arms.current);
    let from = m.position;
    let (states, took) = until(&mut m, &crate_, held, 4.0, rest);
    println!(
        "the climb: {took:.2} s through {states:?}; {:.2} m forward and {:.3} m up, to {:.2} m short of the box's face",
        m.position[2] - from[2],
        m.position[1] - from[1],
        0.5 - m.position[2]
    );
    // It comes to rest on the edge, and walks on from there.
    assert!((m.position[1] - 1.0).abs() < 0.1 && m.position[2] > 0.5 - 0.366, "on the box's edge: {:?}", m.position);
    until(&mut m, &crate_, held, 0.5, |_| false);
    // (Within the ground check of its top: nothing pulls a character down
    // that is.)
    assert!((m.position[1] - 1.0).abs() < 0.08 && m.position[2] > 0.5 && m.grounded, "on the box: {:?}", m.position);

    // A low kerb of a wall, 0.4 m high: vaulted with the hands on the
    // weapon, the view free, and at no cost.
    let Some(mut m) = movement(0.0) else { return };
    let low = Yard(vec![[0.5, 0.8, 0.0, 0.4]]);
    let (legs, arms) = (m.stamina.legs.current, m.stamina.arms.current);
    let (states, _) = until(&mut m, &low, held, 1.0, |m| m.state == State::Vault);
    assert_eq!(states.last(), Some(&State::Vault), "{states:?}; measured {:?}", m.vaulting.obstacle);
    assert!(m.can_press_trigger() && !m.take_aim_taken());
    assert_eq!((legs, arms), (m.stamina.legs.current, m.stamina.arms.current));
    let yaw = m.yaw;
    m.step(1.0 / 120.0, &Input { look_delta: [2.0, 0.0], ..held }, &low);
    assert!((m.yaw - yaw - 2.0).abs() < 1e-3, "the view turns: {}", m.yaw - yaw);
    let (states, took) = until(&mut m, &low, held, 4.0, rest);
    println!("the low vault: {took:.2} s through {states:?}; ends {:.2} m ahead, {:.3} m up", m.position[2], m.position[1]);
    assert!(m.position[2] > 0.8, "{:?}", m.position);
}

/// Medicine in hand, or a grenade's throw held: a wall that needs a hand is
/// not vaulted, and with no move measured before, nothing is (vaulting
/// spec, "What a try does").
#[test]
fn hands_full_refuse_a_vault_that_needs_a_hand() {
    let Some(mut m) = movement(0.0) else { return };
    let held = Input { move_dir: [0.0, 1.0], vault: true, hands_limit_vault: true, ..Input::default() };
    let wall = Yard(vec![[0.5, 0.85, 0.0, 1.0]]);
    let (states, _) = until(&mut m, &wall, held, 1.5, |m| m.state == State::Vault);
    assert!(!states.contains(&State::Vault), "{states:?}");
    // The hands free again: over it.
    let free = Input { hands_limit_vault: false, ..held };
    let (states, _) = until(&mut m, &wall, free, 1.5, |m| m.state == State::Vault);
    assert_eq!(states.last(), Some(&State::Vault), "{states:?}");
}

/// A sprint at a wall with the key held goes over it and on, and the key
/// held past its time does not jump when let go.
#[test]
fn a_sprint_vaults_and_a_long_hold_does_not_jump() {
    let Some(mut m) = movement(0.0) else { return };
    let frame = 1.0 / 120.0;
    // Up to a sprint on open ground, then a wall 0.9 m high across the way.
    let open = Yard(vec![]);
    until(&mut m, &open, walk([0.0, 1.0]), 0.5, |_| false);
    let sprint = Input { move_dir: [0.0, 1.0], sprint: true, ..Input::default() };
    let (states, _) = until(&mut m, &open, sprint, 4.0, |m| m.state == State::Sprint);
    assert_eq!(states.last(), Some(&State::Sprint));
    until(&mut m, &open, sprint, 1.5, |_| false);
    let wall = Yard(vec![[m.position[2] + 3.0, m.position[2] + 3.3, 0.0, 0.9]]);
    let far = m.position[2] + 3.3;
    let keyed = Input { vault: true, ..sprint };
    let (states, took) = until(&mut m, &wall, keyed, 3.0, |m| m.state == State::Vault);
    assert_eq!(states.last(), Some(&State::Vault), "{states:?}; measured {:?}", m.vaulting.obstacle);
    println!(
        "a sprint meets the wall {took:.2} s on, {:.2} m short of it at {:.2} m/s: {:?}",
        far - 0.3 - m.position[2],
        m.ground_speed(),
        m.vaulting.obstacle
    );
    let (states, took) = until(&mut m, &wall, sprint, 4.0, |m| m.state == State::Sprint);
    println!("the sprint's vault: {took:.2} s through {states:?}; {:.2} m past the wall", m.position[2] - far);
    assert_eq!(states.last(), Some(&State::Sprint), "the sprint is taken up again: {states:?}");
    assert!(m.position[2] > far && !m.is_vaulting);

    // The key alone: a tap jumps when it is let go, a long hold does not.
    let Some(mut m) = movement(0.0) else { return };
    let (key, release) = (Input { vault: true, ..Input::default() }, Input { jump: true, ..Input::default() });
    for _ in 0..12 {
        m.step(frame, &key, &Flat);
    }
    assert_eq!(m.state, State::Idle, "held, the key does not jump");
    m.step(frame, &release, &Flat);
    let (states, _) = until(&mut m, &Flat, Input::default(), 0.2, |m| m.state == State::Jump);
    assert_eq!(states.last(), Some(&State::Jump), "a tap of a tenth of a second jumps on release");
    let Some(mut m) = movement(0.0) else { return };
    for _ in 0..90 {
        m.step(frame, &key, &Flat);
    }
    m.step(frame, &release, &Flat);
    let (states, _) = until(&mut m, &Flat, Input::default(), 0.3, |m| m.state == State::Jump);
    assert_eq!(states, [State::Idle], "held three quarters of a second, it does not");
}
