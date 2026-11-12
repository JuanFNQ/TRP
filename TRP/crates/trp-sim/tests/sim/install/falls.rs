//! Falling off ledges, foot motion, crouched walks and the ribcage lag.

use super::*;

/// Level ground that ends in a drop: `height` metres down from `edge`
/// metres ahead on.
pub(super) struct Ledge {
    edge: f32,
    pub(super) height: f32,
}

impl trp_sim::World for Ledge {
    fn ground_height(&self, _x: f32, z: f32) -> f32 {
        if z < self.edge { 0.0 } else { -self.height }
    }
}

/// The game has no falling state: a character that walks off a ledge goes
/// on walking, with gravity on it from the frame the ground check finds
/// nothing, its motion dying away from a quarter second in, the sights
/// due down from half a second, and the fall's cap on the speed when it
/// lands (movement spec, "Falling").
#[test]
fn a_walk_off_a_ledge_falls_and_lands() {
    let Some(mut m) = movement(0.0) else { return };
    let ledge = Ledge { edge: 6.0, height: 2.5 };
    let walking = walk([0.0, 1.0]);
    let frame = 1.0 / 120.0;
    let (mut left, mut landed, mut sights_due) = (None, None, None);
    let (mut pace_off, mut cap_before) = (0.0, 0.0);
    let mut states = Vec::new();
    for index in 0..(8.0 / frame) as usize {
        let (was_grounded, before) = (m.grounded, m.position);
        m.step(frame, &walking, &ledge);
        let time = index as f32 * frame;
        if left.is_none() && was_grounded && !m.grounded {
            (left, pace_off, cap_before) = (Some((time, before)), m.ground_speed(), m.clamped_speed());
        }
        if let (Some((since, _)), None) = (left, landed) {
            if !states.contains(&m.state) {
                states.push(m.state);
            }
            if sights_due.is_none() && m.long_fall() {
                sights_due = Some((time - since, -m.position[1]));
            }
            if m.grounded {
                landed = Some((time, m.position));
                break;
            }
        }
    }
    let ((left_at, from), (landed_at, to)) = (left.expect("it never left the ground"), landed.expect("it never landed"));
    let (air, forward, drop) = (landed_at - left_at, to[2] - from[2], from[1] - to[1]);
    println!(
        "walked off at {pace_off:.2} m/s, {:.2} m out; in the air {air:.3} s, {forward:.2} m forward and {drop:.2} m down; states in the air {states:?}",
        from[2]
    );
    let (due_after, dropped) = sights_due.expect("the fall was long enough for the sights");
    println!("the sights are due down {due_after:.3} s into the fall, {dropped:.2} m down");
    // A drop of 2.5 m takes 0.71 s; the character had been riding up to
    // 8 cm over the ground.
    assert!(states == [State::Move], "{states:?}");
    assert!((0.68..0.76).contains(&air), "{air}");
    assert!((2.4..2.6).contains(&drop), "{drop}");
    // A quarter second at the walk's pace, then less and less.
    let unslowed = pace_off * air;
    assert!(forward > pace_off * 0.25 && forward < 0.8 * unslowed, "{forward} against {unslowed}");
    assert!((0.5..0.52).contains(&due_after), "{due_after}");

    // The landing caps the selected speed for a while.
    for _ in 0..3 {
        m.step(frame, &walking, &ledge);
    }
    let capped = m.clamped_speed();
    for _ in 0..(4.0 / frame) as usize {
        m.step(frame, &walking, &ledge);
    }
    println!("the speed setting allowed: {cap_before:.3} before, {capped:.3} on landing, {:.3} four seconds on", m.clamped_speed());
    assert!(capped < cap_before - 0.05, "{capped} against {cap_before}");
    assert!((m.clamped_speed() - cap_before).abs() < 1e-3);
    assert!(m.grounded && m.freefall_time == 0.0 && !m.long_fall());
}

/// A sprint off the range's ledge, 6 m up, held on after the landing: the
/// landing caps the speed to nought for a while, and the sprint's own limit
/// under it is the game's `Mathf.Clamp` with its bounds crossed (0.1, the
/// lower, wins), which Rust's clamp would stop the program on (it did, in
/// the app).
#[test]
fn a_sprint_off_a_high_ledge_lands_and_goes_on() {
    let Some(mut m) = movement(0.0) else { return };
    let ledge = Ledge { edge: 12.0, height: 6.0 };
    let frame = 1.0 / 120.0;
    // A walk first: a sprint pressed before moving is ignored.
    let walking = walk([0.0, 1.0]);
    for _ in 0..(0.5 / frame) as usize {
        m.step(frame, &walking, &ledge);
    }
    let sprinting = Input { move_dir: [0.0, 1.0], sprint: true, ..Input::default() };
    let (mut lowest, mut landed, mut sprinted_on) = (f32::MAX, None, 0);
    for _ in 0..(8.0 / frame) as usize {
        m.step(frame, &sprinting, &ledge);
        if landed.is_none() && m.grounded && m.position[1] < -5.9 {
            landed = Some(m.state);
        }
        if landed.is_some() {
            lowest = lowest.min(m.speed_limit());
            sprinted_on += (m.state == State::Sprint) as usize;
        }
    }
    println!("landed in {landed:?}; the speed allowed after it at least {lowest:.3}; {sprinted_on} frames of sprint after it; now {:?} at {:.2} m/s", m.state, m.ground_speed());
    assert_eq!(landed, Some(State::Sprint));
    assert!(lowest < 0.1, "{lowest}");
}

/// The legs as the body's animator poses them, for a host that draws the
/// body: a foot's path through a walk has no jumps in it from one frame to
/// the next. (Whether it is the game's path is for a recording with the
/// leg bones to say: recorder 0.4.4.)
#[test]
fn a_foot_moves_evenly_through_a_walk() {
    use trp_sim::base::math::{quat_mul, quat_rotate};
    let Ok(install) = crate::fixture::install().ok_or(()) else {
        return;
    };
    let db = crate::fixture::database().expect("database");
    let mut assets = Assets::new(install).expect("manifest");
    let hard = trp_data::setup::hard_settings::HardSettings::load(&assets.install).expect("settings object");
    let engine = trp_data::setup::engine::EngineSettings::load(&assets.install).expect("engine settings");
    let settings = trp_sim::load_movement_settings(&mut assets, &db, &hard, &engine, 0.0).expect("movement settings");
    let chain = ["Root_Joint", "Base HumanPelvis", "Base HumanLThigh1", "Base HumanLThigh2", "Base HumanLCalf", "Base HumanLFoot"];
    let paths: Vec<String> = (1..=chain.len()).map(|n| chain[..n].join("/")).collect();
    let body = trp_sim::locomotion::body::BodyData::load_with(&mut assets, &paths).expect("body animator");
    let mut m = Movement::new(settings, trp_sim::Body::new(&body, 5.0));
    let foot = |m: &Movement| -> [f32; 3] {
        let bones = m.body.drawn_bones();
        let (at, _) = bones.iter().fold(([0.0f32; 3], [0.0f32, 0.0, 0.0, 1.0]), |(at, turn), (position, rotation)| {
            let shifted = quat_rotate(turn, *position);
            (std::array::from_fn(|k| at[k] + shifted[k]), quat_mul(turn, *rotation))
        });
        at
    };
    let frame = 1.0 / 120.0;
    let run = |m: &mut Movement, seconds: f32, input: Input| -> Vec<[f32; 3]> {
        (0..(seconds / frame) as usize)
            .map(|_| {
                m.step(frame, &input, &Flat);
                foot(m)
            })
            .collect()
    };
    let report = |name: &str, path: &[[f32; 3]]| -> (f32, f32) {
        let steps: Vec<f32> = path.windows(2).map(|w| (0..3).map(|k| (w[1][k] - w[0][k]).powi(2)).sum::<f32>().sqrt()).collect();
        let changes: Vec<f32> = steps.windows(2).map(|w| (w[1] - w[0]).abs()).collect();
        let mean = steps.iter().sum::<f32>() / steps.len() as f32;
        let most = steps.iter().cloned().fold(0.0, f32::max);
        let jump = changes.iter().cloned().fold(0.0, f32::max);
        let reach = |k: usize| {
            let (low, high) = path.iter().fold((f32::MAX, f32::MIN), |(low, high), p| (low.min(p[k]), high.max(p[k])));
            high - low
        };
        println!(
            "{name}: the left foot moves {:.2} mm a frame on average, {:.2} at most; the most that changes from one frame to the next is {:.2} mm; it ranges {:.0} mm sideways, {:.0} up and {:.0} along",
            mean * 1000.0, most * 1000.0, jump * 1000.0, reach(0) * 1000.0, reach(1) * 1000.0, reach(2) * 1000.0
        );
        (most, jump)
    };
    let still = run(&mut m, 3.0, Input::default());
    let (most_still, _) = report("standing", &still);
    run(&mut m, 2.0, walk([0.0, 1.0]));
    let walking = run(&mut m, 3.0, walk([0.0, 1.0]));
    let (most, jump) = report("walking", &walking);
    // Standing, the foot hardly moves; walking, no frame's move is out of
    // line with its neighbours'.
    assert!(most_still < 0.002, "{most_still}");
    assert!(jump < 0.5 * most, "{jump} against {most}");
}

/// The floor under a walk's speed comes from a curve the game keeps once
/// for the whole raid, keyed by whichever character's carried weight
/// changed last (movement spec, "The floor's curve is shared"). A crouched
/// walk goes at the floor, so its pace follows that key; an upright walk
/// only starts from it.
#[test]
fn a_crouched_walk_goes_at_the_shared_floor() {
    let pace = |keyed_by: Option<f32>, crouched: bool| -> Option<(f32, f32, f32)> {
        // An untrained character carrying 9 kg.
        let mut m = movement(9.0)?;
        if let Some(inertia) = keyed_by {
            m.settings.key_acceleration_range(inertia);
        }
        if crouched {
            m.step(DT, &Input { pose_delta: -1.0, ..Input::default() }, &Flat);
            run(&mut m, 1.5, Input::default());
        }
        let from = m.position;
        run(&mut m, 4.0, walk([0.0, 1.0]));
        let gone = ((m.position[0] - from[0]).powi(2) + (m.position[2] - from[2]).powi(2)).sqrt();
        Some((m.smoothed_speed, gone, m.settings.inertia))
    };
    let Some((own, own_gone, inertia)) = pace(None, true) else { return };
    let (laden, laden_gone, _) = pace(Some(1.0), true).unwrap();
    let (same, _, _) = pace(Some(inertia), true).unwrap();
    println!(
        "crouched, speed parameter and metres in 4 s: {own:.4} and {own_gone:.3} by the character's own key (inertia {inertia:.3}), {laden:.4} and {laden_gone:.3} by a fully laden character's"
    );
    assert!((same - own).abs() < 1e-6, "keyed by its own inertia: {same} against {own}");
    assert!(laden < own - 0.005 && laden_gone < own_gone - 0.02, "{laden} {own} {laden_gone} {own_gone}");
    let (upright, upright_gone, _) = pace(None, false).unwrap();
    let (upright_laden, upright_laden_gone, _) = pace(Some(1.0), false).unwrap();
    println!(
        "upright: {upright:.4} and {upright_gone:.3} by its own key, {upright_laden:.4} and {upright_laden_gone:.3} by a fully laden character's"
    );
    assert!((upright - upright_laden).abs() < 1e-4 && (upright_gone - upright_laden_gone).abs() < 0.005);

    // And the key is read back from the curve's value it gives.
    let Some(mut m) = movement(9.0) else { return };
    m.settings.key_acceleration_range(0.456);
    let value = m.settings.movement_acceleration_range.evaluate(m.settings.walk_inertia);
    let found = m.settings.acceleration_key_giving(value, 1e-4).expect("a key gives it");
    assert!((found - 0.456).abs() < 1e-3, "{found}");
}

/// The body's animation turns and bends the upper body to the view by
/// angles noted before the frame's own turn of the view, so the ribcage,
/// which the eye hangs on, goes round the feet a frame behind the view and
/// is at rest a frame after the view is (recorded: the tenth recording's
/// scenario 11, 1.6 mm sideways at 84 degrees a second on the frame the
/// view starts and on the frame it stops). `ribcage_posed_for` gives a
/// host the ribcage for whatever view it shows.
#[test]
fn the_ribcage_trails_the_view_by_a_frame() {
    let Some(mut m) = movement(0.0) else { return };
    let frame = 1.0 / 120.0;
    // Looking 80 degrees down, where the ribcage is far ahead of the feet.
    m.pitch = 80.0;
    m.note_rotation();
    for _ in 0..240 {
        m.frame(frame, &Input::default(), &Flat);
    }
    // Which way the ribcage lies from the feet, in the world, in degrees.
    let bearing = |m: &Movement, ribcage: [f32; 3]| ribcage[0].atan2(ribcage[2]).to_degrees() + m.body_yaw;
    let reach = |r: [f32; 3]| (r[0] * r[0] + r[2] * r[2]).sqrt();

    // Asked for the view the pose was made for, it is the ribcage as posed.
    let (as_posed, asked) = (m.ribcage(), m.ribcage_posed_for(m.posed_for()));
    let apart = (0..3).map(|k| (as_posed[k] - asked[k]).abs()).fold(0.0, f32::max);
    assert!(apart < 1e-5, "{apart}");

    // The view turns 300 degrees a second for ten frames: 2.5 degrees a
    // frame, 25 in all, short of what turns the feet.
    let turn = Input { look_delta: [2.5, 0.0], ..Input::default() };
    let mut trails = Vec::new();
    for _ in 0..10 {
        m.frame(frame, &turn, &Flat);
        // How far short of where the view as it stands would have it.
        trails.push(bearing(&m, m.ribcage_posed_for([m.yaw, m.pitch])) - bearing(&m, m.ribcage()));
    }
    let turning = trails[3..].iter().sum::<f32>() / (trails.len() - 3) as f32;
    m.frame(frame, &Input::default(), &Flat);
    let stopped = bearing(&m, m.ribcage_posed_for([m.yaw, m.pitch])) - bearing(&m, m.ribcage());
    println!(
        "looking 80 degrees down with the ribcage {:.2} m ahead of the feet, the view turning 2.5 degrees a frame: the ribcage trails it by {turning:.2} degrees ({:.0} mm); a frame after the view stops, by {stopped:.2}",
        reach(m.ribcage()),
        reach(m.ribcage()) * turning.to_radians() * 1000.0
    );
    assert_eq!(m.state, State::Idle);
    assert!((turning - 2.5).abs() < 0.3, "{turning}");
    assert!(stopped.abs() < 0.3, "{stopped}");
}

/// The drawn ribcage for another view (`Body::ribcage_posed_for`) is worked
/// out from the bones down to it alone (`Animator::bones_with`), and comes
/// out where the whole pose worked out again puts it (`ribcage_posed_whole`,
/// `Animator::pose_with`), to the last bit: standing, walking, turning the
/// view and the feet, sprinting, crouched and leaning, with the posing
/// deferred and not, over a spread of angles to the view and pitches.
#[test]
fn the_ribcage_for_another_view_is_the_whole_pose_s() {
    for deferred in [false, true] {
        let Some(mut m) = movement(0.0) else { return };
        m.body.set_deferred_posing(deferred);
        let frame = 1.0 / 120.0;
        let turn = |yaw: f32, pitch: f32, input: Input| Input { look_delta: [yaw, pitch], ..input };
        let phases: [(usize, Input); 7] = [
            (120, Input::default()),
            (120, turn(1.5, 0.0, walk([0.0, 1.0]))),
            (90, turn(-6.0, 0.4, Input::default())),
            (120, turn(0.8, -0.3, Input { sprint: true, ..walk([0.0, 1.0]) })),
            (40, Input { crouch: true, ..Input::default() }),
            (120, turn(2.0, 0.5, Input { lean: 1.0, ..walk([1.0, 0.5]) })),
            (90, turn(-3.0, -0.6, walk([-1.0, 0.0]))),
        ];
        let (mut asked, mut frames) = (0usize, 0usize);
        for (count, input) in phases {
            for n in 0..count {
                m.frame(frame, &input, &Flat);
                if n % 6 != 0 {
                    continue;
                }
                frames += 1;
                for delta in (-12..=12).map(|k| k as f32 * 15.0 + 0.37) {
                    for aim in (-4..=4).map(|k| k as f32 * 20.0 - 0.21) {
                        let (part, whole) = (m.body.ribcage_posed_for(delta, aim), m.body.ribcage_posed_whole(delta, aim));
                        assert_eq!(part.map(f32::to_bits), whole.map(f32::to_bits), "deferred {deferred}, {:?}, delta {delta}, aim {aim}: {part:?} against {whole:?}", m.state);
                        asked += 1;
                    }
                }
            }
        }
        println!("deferred {deferred}: {asked} ribcages over {frames} frames, each bit for bit the whole pose's");
    }
}
