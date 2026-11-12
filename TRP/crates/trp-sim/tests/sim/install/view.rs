//! The view hung from the ribcage and the view and body turning evenly.

use super::*;

/// Where the first-person view sits comes from the body's animation: the
/// ribcage's height standing and crouched, its bob with each stride and
/// its shift in a lean.
#[test]
fn the_view_hangs_from_the_bodys_ribcage() {
    let Some(mut m) = movement(0.0) else { return };
    run(&mut m, 0.5, Input::default());
    let standing = m.body.ribcage();
    // The camera sits 0.04 m above; the recorded eye is at 1.508 m.
    println!("standing: ribcage {standing:?}");
    assert!((standing[1] - 1.468).abs() < 0.02, "{standing:?}");

    // A full lean to the right carries it 0.3 m aside and 0.06 m down.
    run(&mut m, 1.5, Input { lean: 1.0, ..Input::default() });
    let leaning = m.body.ribcage();
    println!("leaning right: {:+.3} m aside, {:+.3} m in height", leaning[0] - standing[0], leaning[1] - standing[1]);
    assert!((leaning[0] - standing[0] - 0.30).abs() < 0.05, "{leaning:?}");
    assert!((standing[1] - leaning[1] - 0.06).abs() < 0.03, "{leaning:?}");
    run(&mut m, 1.5, Input::default());

    // Fully crouched the recorded eye is 0.6 m lower.
    m.step(DT, &Input { pose_delta: -1.0, ..Input::default() }, &Flat);
    run(&mut m, 3.0, Input::default());
    let crouched = m.body.ribcage();
    println!("crouched: {:.3} m lower", standing[1] - crouched[1]);
    assert!((standing[1] - crouched[1] - 0.60).abs() < 0.06, "{crouched:?}");

    // Walking, it rises and falls with the stride.
    let Some(mut m) = movement(0.0) else { return };
    run(&mut m, 1.0, walk([0.0, 1.0]));
    let (mut low, mut high) = (f32::MAX, f32::MIN);
    for _ in 0..120 {
        m.step(DT, &walk([0.0, 1.0]), &Flat);
        let height = m.body.ribcage()[1];
        (low, high) = (low.min(height), high.max(height));
    }
    println!("walking: the ribcage moves between {low:.3} and {high:.3} m");
    assert!(high - low > 0.01 && high - low < 0.1, "{low} to {high}");
    assert!(high < standing[1], "walking is lower than standing: {high}");
}

/// While the body sprints its own weapon bone takes the first-person
/// weapon over entirely, and hands it back when the sprint ends.
#[test]
fn the_sprint_carries_the_weapon() {
    let Some(mut m) = movement(0.0) else { return };
    run(&mut m, 0.5, walk([0.0, 1.0]));
    assert!(m.carried().is_none(), "a walk does not carry the weapon");
    run(&mut m, 2.0, Input { move_dir: [0.0, 1.0], sprint: true, ..Input::default() });
    assert_eq!(m.state, State::Sprint);
    let carried = m.carried().expect("a sprint carries the weapon");
    println!("sprinting: the weapon is carried at {:?}, turned {:?}", carried.position, carried.rotation);
    assert_eq!(carried.authority, 1.0);
    // Below the ribcage, and turned well away from the view.
    assert!(carried.position[1] < -0.05, "{:?}", carried.position);
    assert!(carried.rotation[3].abs() < 0.95, "{:?}", carried.rotation);
    // The hand-over fades in and out with the animator's blends.
    run(&mut m, 0.15, walk([0.0, 1.0]));
    let fading = m.carried().map_or(0.0, |c| c.authority);
    assert!(fading > 0.0 && fading < 1.0, "{fading}");
    run(&mut m, 1.0, walk([0.0, 1.0]));
    assert!(m.carried().is_none());
}

/// On frames of even length the game's whole-millisecond clock for the
/// player makes a jump's travel uneven (8, 8 and 9 ms of it a frame at 120
/// a second). The place a host is given to draw has that taken out.
#[test]
fn a_jump_is_drawn_as_evenly_as_a_walk() {
    let Some(mut m) = movement(0.0) else { return };
    // The largest change of level travel from one frame to the next, as a
    // share of the travel, over the frames in the air.
    let (mut moved, mut drawn) = (Vec::new(), Vec::new());
    let (mut at, mut shown) = (m.position[2], m.drawn_position()[2]);
    for frame in 0..600 {
        m.frame(1.0 / 120.0, &Input { jump: frame == 300, ..walk([0.0, 1.0]) }, &Flat);
        if m.state == State::Jump && frame > 300 {
            moved.push(m.position[2] - at);
            drawn.push(m.drawn_position()[2] - shown);
        }
        (at, shown) = (m.position[2], m.drawn_position()[2]);
    }
    let ripple = |travel: &[f32]| travel.windows(2).map(|w| ((w[1] - w[0]) / w[0]).abs()).fold(0.0f32, f32::max);
    println!(
        "a running jump, {} frames in the air: travel a frame changes by up to {:.1}% as moved, {:.2}% as drawn",
        moved.len(),
        100.0 * ripple(&moved),
        100.0 * ripple(&drawn)
    );
    assert!(moved.len() > 60);
    assert!(ripple(&moved) > 0.1, "{}", ripple(&moved));
    assert!(ripple(&drawn) < 0.01, "{}", ripple(&drawn));
    // On the ground the two are one.
    assert_eq!(m.drawn_position(), m.position);
}

/// The movement state slows the view's turning and limits how far one
/// frame may turn it: half the rate in a sprint, a third in a jump from
/// standing and lying down, and none at all through a reload lying down.
#[test]
fn the_movement_state_slows_the_views_turning() {
    let Some(mut m) = movement(0.0) else { return };
    // How far one step turns the view when asked for `ask` degrees.
    fn turned(m: &mut Movement, ask: f32, held: Input) -> f32 {
        let before = m.yaw;
        m.step(DT, &Input { look_delta: [ask, 0.0], ..held }, &Flat);
        m.yaw - before
    }
    let still = Input::default();
    assert!((turned(&mut m, 10.0, still) - 10.0).abs() < 1e-4);

    let sprint = Input { sprint: true, ..walk([0.0, 1.0]) };
    run(&mut m, 0.5, walk([0.0, 1.0]));
    assert!((turned(&mut m, 10.0, walk([0.0, 1.0])) - 10.0).abs() < 1e-4, "walking turns freely");
    run(&mut m, 2.0, sprint);
    assert_eq!(m.state, State::Sprint);
    let (sprinting, far) = (turned(&mut m, 10.0, sprint), turned(&mut m, 60.0, sprint));
    run(&mut m, 2.0, still);

    m.step(DT, &Input { jump: true, ..still }, &Flat);
    run(&mut m, 0.3, still);
    assert_eq!(m.state, State::Jump);
    let jumping = turned(&mut m, 9.0, still);
    run(&mut m, 2.0, still);

    m.step(DT, &Input { prone: true, ..still }, &Flat);
    run(&mut m, 2.5, still);
    assert_eq!(m.state, State::ProneIdle);
    let (lying, lying_far) = (turned(&mut m, 9.0, still), turned(&mut m, 60.0, still));
    let reloading = Input { reloading: true, ..still };
    run(&mut m, 0.5, reloading);
    let in_reload = turned(&mut m, 9.0, reloading);
    run(&mut m, 0.5, still);
    let after = turned(&mut m, 9.0, still);
    println!(
        "asked for 10 degrees in a step: sprinting {sprinting:.2} (60 gives {far:.2}); for 9: in a standing jump {jumping:.2}, lying {lying:.2} (60 gives {lying_far:.2}), in a reload lying {in_reload:.2}, after it {after:.2}"
    );
    assert!((sprinting - 5.0).abs() < 1e-3 && (far - 10.0).abs() < 1e-3, "{sprinting} {far}");
    assert!((jumping - 2.97).abs() < 1e-3, "{jumping}");
    assert!((lying - 2.97).abs() < 1e-3 && (lying_far - 5.0).abs() < 1e-3, "{lying} {lying_far}");
    assert_eq!(in_reload, 0.0);
    assert!((after - 2.97).abs() < 1e-3, "{after}");
}

/// A host that draws more often than the simulation's frames come shows
/// the mouse's travel no frame has had yet. Shown as the next frame will
/// let it through, the view turns evenly through a jump, where only a third
/// gets through; shown whole it would step forward and back.
#[test]
fn the_view_shown_between_frames_turns_evenly() {
    for host_rate in [144.0f32, 240.0, 360.0] {
        let Some(mut m) = movement(0.0) else { return };
        let mut pacer = trp_sim::Pacer::new(120.0, 1.0 / 60.0, 1.0 / 12.0);
        // The mouse turns the view a steady degree per drawn frame.
        let (mut pending, mut jumped) = (0.0f32, false);
        let (mut shown, mut whole) = (Vec::new(), Vec::new());
        for host_frame in 0..(host_rate as usize * 2) {
            pending += 1.0;
            let due = pacer.pass(1.0 / host_rate);
            let share = pending / due.max(1) as f32;
            for _ in 0..due {
                pacer.frame();
                let jump = host_frame as f32 / host_rate > 0.5 && !std::mem::replace(&mut jumped, true);
                m.frame(pacer.frame_length(), &Input { look_delta: [share, 0.0], jump, ..Input::default() }, &Flat);
                pending = 0.0;
            }
            if m.state == State::Jump {
                shown.push(m.look_ahead([pending, 0.0], 0.0)[0]);
                whole.push(m.yaw + pending);
            }
        }
        let steps = |view: &[f32]| -> (f32, f32) {
            let steps: Vec<f32> = view.windows(2).map(|w| w[1] - w[0]).collect();
            (steps.iter().copied().fold(f32::MAX, f32::min), steps.iter().copied().fold(f32::MIN, f32::max))
        };
        let ((least, most), (least_whole, most_whole)) = (steps(&shown), steps(&whole));
        println!(
            "a standing jump drawn at {host_rate} a second, a degree of mouse a frame: the view shown turns {least:.3} to {most:.3} degrees a frame; with the travel shown whole, {least_whole:.3} to {most_whole:.3}"
        );
        assert!(shown.len() > 40, "{}", shown.len());
        assert!((least - 0.33).abs() < 1e-3 && (most - 0.33).abs() < 1e-3, "{least} {most}");
        assert!(least_whole < 0.0, "{least_whole}");
    }
}

/// In a jump the body turns after the view with a lag and carries the
/// weapon, so the weapon and arms are drawn by the angle between the two.
/// Two things made that angle change unevenly from frame to frame, and the
/// weapon shake against a view that turned smoothly: the mouse's travel
/// reaching the simulation's frames in uneven lots wherever the host's
/// rate and the simulation's do not divide, and the game's own turning of
/// the body by a clock in whole milliseconds. The first is shared out by
/// time; the second is taken out of what is drawn.
#[test]
fn the_body_follows_the_view_evenly_in_a_jump() {
    // How far the angle from body to view moves in a frame, set against
    // the mean of the frames either side of it: the largest such gap, in
    // degrees, while the body is turning after the view. A smooth run-in
    // has next to none; a ripple shows whole.
    let ripple = |host_rate: f32, by_time: bool, drawn: bool| -> Option<(f32, f32, usize)> {
        let mut m = movement(0.0)?;
        let mut pacer = trp_sim::Pacer::new(120.0, 1.0 / 60.0, 1.0 / 12.0);
        // The mouse turns at a steady 360 degrees a second.
        let (mut held, mut jumped) = (0.0f32, false);
        let mut angles = Vec::new();
        for host_frame in 0..(host_rate as usize * 2) {
            held += 360.0 / host_rate;
            let due = pacer.pass(1.0 / host_rate);
            for _ in 0..due {
                let frame = pacer.frame();
                // Given whole, the frame that comes due takes all there is.
                let turn = if by_time { held * frame.input_share } else { held };
                held -= turn;
                let jump = host_frame as f32 / host_rate > 0.5 && !std::mem::replace(&mut jumped, true);
                m.frame(frame.dt, &Input { look_delta: [turn, 0.0], jump, ..Input::default() }, &Flat);
                if m.state == State::Jump {
                    let body = if drawn { m.drawn_body_yaw() } else { m.body_yaw };
                    angles.push(trp_sim::base::math::delta_angle(body, m.yaw));
                }
            }
        }
        // The body stops turning some frames before the state ends; the
        // first hundred frames are all turning.
        let moved: Vec<f32> = angles.windows(2).take(100).map(|w| w[1] - w[0]).collect();
        let ripple = moved.windows(3).map(|w| (w[1] - (w[0] + w[2]) / 2.0).abs()).fold(0.0f32, f32::max);
        let widest = angles.iter().fold(0.0f32, |a, b| a.max(b.abs()));
        Some((ripple, widest, angles.len()))
    };
    for host_rate in [144.0f32, 165.0, 240.0] {
        let (Some((before, widest, frames)), Some((games, _, _)), Some((now, _, _))) =
            (ripple(host_rate, false, false), ripple(host_rate, true, false), ripple(host_rate, true, true))
        else {
            return;
        };
        println!(
            "a standing jump drawn at {host_rate} a second, the mouse at 360 degrees a second, the view up to {widest:.0} degrees ahead of the body over {frames} frames: the angle between them ripples by {before:.3} degrees a frame with the mouse's travel given whole, {games:.3} shared by time (the game's own, from its clock), {now:.3} as drawn"
        );
        assert!(frames > 110, "{frames}");
        assert!(games > 0.05, "{host_rate}: {games}");
        assert!(now < 0.02, "{host_rate}: {now}");
    }
}

/// Standing, the body lets the view lead it by up to 45 degrees. Past
/// that the animator plays a turn on the spot, whose clip swings the body
/// a quarter turn.
#[test]
fn a_turn_past_the_limit_turns_the_body_on_the_spot() {
    let Some(mut m) = movement(0.0) else { return };
    run(&mut m, 0.5, Input::default());
    // 40 degrees: the body stays where it is.
    for _ in 0..20 {
        m.step(DT, &Input { look_delta: [2.0, 0.0], free_look: true, ..Input::default() }, &Flat);
    }
    run(&mut m, 1.0, Input::default());
    println!("view turned 40 degrees: the body at {:.1}, state {:?}", m.body_yaw, m.body.state_names().0);
    assert!(m.body_yaw.abs() < 0.01, "{}", m.body_yaw);

    // 30 more: past the limit.
    let mut log = Vec::new();
    for frame in 0..90 {
        let turn = if frame < 15 { 2.0 } else { 0.0 };
        m.step(DT, &Input { look_delta: [turn, 0.0], free_look: true, ..Input::default() }, &Flat);
        if frame % 6 == 5 {
            log.push(format!("{:.0}", m.body_yaw));
        }
    }
    let (root, _) = (m.body.root_rotation(), ());
    println!(
        "view turned to 70 degrees: the body at {} by tenths of a second; ends at {:.1} in {:?}, the root bone turned {:.1} degrees in the pose",
        log.join(" "),
        m.body_yaw,
        m.body.state_names().0,
        2.0 * root[1].atan2(root[3]).to_degrees()
    );
    assert_eq!(m.state, State::Idle);
    assert!(m.body.state_names().0.ends_with("Idle_Aim"));
    // The clip turns the body a quarter turn in its first 0.4 s, on top
    // of the little the body had followed the view by then, so the body
    // ends ahead of the view. Its root bone keeps none of the turn.
    assert!(m.body_yaw > 90.0 && m.body_yaw < 95.0, "{}", m.body_yaw);
    assert!((m.yaw - 70.0).abs() < 1e-3 && (m.body_yaw - m.yaw).abs() < 45.0);
    assert!(root[1].abs() < 1e-3, "{root:?}");
}

/// A view flicked one way and then further back the other, as a mouse
/// does it: the second turn on the spot cuts into the first, and the view
/// comes to rest while it still plays, with the body drawn after it at the
/// limit. The game takes the angle between them with 360 added to each
/// bearing, which rounds it, and its body's turning is finer than a
/// bearing, so the angle comes to read the limit exactly and no third
/// turn follows (movement spec, "Looking"; eleventh recording, 59).
#[test]
fn a_body_held_at_the_limit_does_not_turn_again() {
    // From the lane's bearing, from the eleventh recording's, and after
    // the view has been round eight times, where a bearing in single
    // precision is at its coarsest.
    for bearing in [0.0, 20.565_096, 167.833_98, 2900.565] {
        let Some(mut m) = movement(0.0) else { return };
        (m.yaw, m.body_yaw) = (bearing, bearing);
        m.note_rotation();
        run(&mut m, 0.5, Input::default());
        let mut turns = Vec::new();
        let mut last = String::new();
        // At the rate the simulation is run at, where the body takes a
        // quarter of what it is over the limit each frame.
        const FRAME: f32 = 1.0 / 120.0;
        for frame in 0..600 {
            // 84 degrees to the left in a fifth of a second, a pause as
            // long, 168 to the right in two fifths: 420 degrees a second.
            let turn = match frame {
                0..24 => -3.5,
                48..96 => 3.5,
                _ => 0.0,
            };
            m.step(FRAME, &Input { look_delta: [turn, 0.0], free_look: true, ..Input::default() }, &Flat);
            let (playing, next) = m.body.state_names();
            let entering = next.unwrap_or(playing).to_string();
            if entering != last && entering.contains("Turn") {
                turns.push((frame, entering.clone()));
            }
            last = entering;
        }
        let [angle, _] = m.noted_angles();
        println!(
            "from a bearing of {bearing}, a flick left and a longer one back: turns begun {turns:?}; the view ends at {:.3}, the body at {:.5}, the angle between them reads {angle:?} in {:?}",
            m.yaw,
            m.body_yaw,
            m.body.state_names().0
        );
        assert_eq!(turns.len(), 2, "{turns:?}");
        assert!(turns[0].1.contains("Left") && turns[1].1.contains("Right"), "{turns:?}");
        assert!(m.body.state_names().0.ends_with("Idle_Aim"));
        assert_eq!(angle, m.settings.trunk_rotation_limit, "the body is held at the limit to the last place");
    }
}
