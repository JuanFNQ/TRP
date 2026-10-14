//! The look all over a weapon as it is drawn: on curves without corners
//! through the clip's samples, where the engine joins them by straight
//! lines (the user, 2026-10-13: "Can we add interpolation the inspect to
//! make it smooth?"; `docs/findings.md` of that day), and since 2026-10-14
//! every animation of the weapon so, or none, as the host asks (the
//! options' switch `SMOOTH FIRST PERSON ANIMATIONS`). **How the result is
//! drawn, and no part of the result**: these check both halves on the
//! stock AKM's own animator from the install. They pass trivially, with a
//! note, where no install is configured.
//!
//! Run with `cargo test -p trp-sim --test look -- --nocapture` for the
//! figures.

use trp_data::unity::Assets;
use trp_data::{Database, Install};
use trp_sim::weapon_animator::{WeaponAnimator, WeaponAnimatorData};
use trp_sim::Firearm;

const AKM: &str = "59d6088586f774275f37482f";
const FRAME: f32 = 1.0 / 120.0;

type Pose = Vec<([f32; 3], [f32; 4])>;

fn akm() -> Option<(Firearm, WeaponAnimatorData)> {
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set EFT_DIR or local/eft_dir.txt)");
        return None;
    };
    let db = Database::load(&install).expect("database");
    let hard = trp_data::hard_settings::HardSettings::load(&install).expect("settings object");
    let mut assets = Assets::new(install).expect("manifest");
    let weapon = trp_data::weapon::assemble(&mut assets, &db, AKM).expect("the weapon");
    let data = WeaponAnimatorData::load(&assets, &weapon).expect("its controller");
    let settings = trp_sim::firearm_settings(&assets, &db, &hard, &weapon).expect("its settings");
    Some((Firearm::new(settings, &data), data))
}

fn turned(q: [f32; 4], v: [f32; 3]) -> [f32; 3] {
    // v + 2 w (u x v) + 2 u x (u x v), u the quaternion's vector part.
    let u = [q[0], q[1], q[2]];
    let cross = |a: [f32; 3], b: [f32; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    let (first, second) = (cross(u, v), cross(u, cross(u, v)));
    std::array::from_fn(|k| v[k] + 2.0 * q[3] * first[k] + 2.0 * second[k])
}

/// Where the left hand's marker is in the rig's own frame, in a pose.
fn marker(data: &WeaponAnimatorData, pose: &Pose) -> [f32; 3] {
    let chain = ["Weapon_root", "Weapon_root/Weapon_root_anim", "Weapon_root/Weapon_root_anim/weapon", "Weapon_root/Weapon_root_anim/weapon/weapon_L_IK_marker"];
    let mut at = [0.0f32; 3];
    for path in chain.iter().rev() {
        let (place, turn) = pose[data.bone(path).expect("a bone of the AKM's rig")];
        let moved = turned(turn, at);
        at = std::array::from_fn(|k| moved[k] + place[k]);
    }
    at
}

fn apart(a: [f32; 3], b: [f32; 3]) -> f32 {
    (0..3).map(|k| (a[k] - b[k]).powi(2)).sum::<f32>().sqrt()
}

/// How much a path's speed changes from step to step, summed and at its
/// worst: what the eye takes for a stutter.
fn roughness(path: &[[f32; 3]]) -> (f32, f32) {
    let jerks: Vec<f32> = path.windows(3).map(|w| apart(std::array::from_fn(|k| w[2][k] - w[1][k]), std::array::from_fn(|k| w[1][k] - w[0][k]))).collect();
    (jerks.iter().sum::<f32>() / jerks.len() as f32, jerks.iter().copied().fold(0.0, f32::max))
}

/// An animator asked for a pose for drawing is, in everything but that
/// pose, the animator it would have been: the same pose, the same states
/// at the same moments, the same marks, bit for bit, through a whole look.
/// The pose for drawing is the pose itself wherever none was asked for;
/// asked for, it is the pose at the clip's samples and another between
/// them, and the left hand's marker runs a smoother path by it.
#[test]
fn a_pose_for_drawing_changes_nothing_but_what_is_drawn() {
    let Some((_, data)) = akm() else { return };
    let look = data.state_names().into_iter().find(|name| name.ends_with("LOOK") && !name.contains("JAM")).expect("the AKM has a look");
    let (mut plain, mut smooth) = (WeaponAnimator::new(&data), WeaponAnimator::new(&data));
    smooth.set_drawn_smooth(true);
    for animator in [&mut plain, &mut smooth] {
        animator.play(&look, 0.0);
    }
    // A fifth of a frame at a time, to see between the samples: the look
    // is 3.33 s long, 100 samples.
    let step = FRAME / 5.0;
    let (mut straight, mut curved) = (Vec::new(), Vec::new());
    let (mut widest, mut at_samples, mut samples_met) = (0.0f32, 0.0f32, 0);
    for index in 0..(3.2 / step) as usize {
        plain.update(step);
        smooth.update(step);
        assert_eq!(plain.pose(), smooth.pose(), "the pose at step {index}");
        assert_eq!(plain.state(), smooth.state(), "the state at step {index}");
        assert_eq!(format!("{:?}", plain.deliver()), format!("{:?}", smooth.deliver()), "the marks at step {index}");
        assert_eq!(plain.drawn_pose().as_slice(), plain.pose(), "nothing asked for, the pose is what is drawn");
        let (as_it_is, as_drawn) = (marker(&data, &plain.pose().to_vec()), marker(&data, &smooth.drawn_pose()));
        let off = apart(as_it_is, as_drawn);
        widest = widest.max(off);
        // The look is a hundred samples long, so a hundred times how far
        // it has played is the sample it is at; every twentieth step
        // falls on one.
        let sample = smooth.state().1 * 100.0;
        if (sample - sample.round()).abs() < 0.01 {
            at_samples = at_samples.max(off);
            samples_met += 1;
        }
        straight.push(as_it_is);
        curved.push(as_drawn);
    }
    let ((mean, worst), (mean_drawn, worst_drawn)) = (roughness(&straight), roughness(&curved));
    println!("  the left hand's marker through the look, a step of {:.2} ms:", step * 1000.0);
    println!("    as the engine has it: its speed changes by {:.4} mm a step on average, {:.3} at the worst", mean * 1000.0, worst * 1000.0);
    println!("    as it is drawn:       {:.4} mm on average, {:.3} at the worst", mean_drawn * 1000.0, worst_drawn * 1000.0);
    println!("    the two are {:.2} mm apart at the most, {:.3} mm at the clip's own samples", widest * 1000.0, at_samples * 1000.0);
    assert!(samples_met > 80, "{samples_met} of the steps fell on a sample");
    assert!(at_samples < 2e-4, "{at_samples}: the drawn pose leaves the clip's samples");
    assert!(widest > 1e-3 && widest < 0.02, "{widest}");
    assert!(mean_drawn < 0.6 * mean && worst_drawn < 0.6 * worst, "{mean_drawn} for {mean}, {worst_drawn} for {worst}");
}

/// The weapon draws on curves without corners when its host asks, through
/// every animation (the user, 2026-10-14: the options' switch `SMOOTH
/// FIRST PERSON ANIMATIONS`), and as the engine has it when not asked.
/// Two of the same weapon, one asked and one not, stepped through a draw,
/// the rest, shots, a reload and the look all over: the same pose, the
/// same states and the same events on every frame; what is drawn is the
/// pose on every frame of the one, and otherwise on most of the other's
/// frames of the reload and the look.
#[test]
fn the_weapon_draws_smooth_when_asked_and_only_so() {
    let Some((mut plain, _)) = akm() else { return };
    let Some((mut smooth, _)) = akm() else { return };
    smooth.set_drawn_smooth(true);
    let run = |plain: &mut Firearm, smooth: &mut Firearm, seconds: f32, what: &str| -> (usize, usize) {
        let (mut frames, mut other) = (0, 0);
        for _ in 0..(seconds / FRAME).round() as usize {
            let (said, said_smooth) = (plain.step(FRAME), smooth.step(FRAME));
            assert_eq!(format!("{said:?}"), format!("{said_smooth:?}"), "the events, {what}");
            assert_eq!(plain.animator().pose(), smooth.animator().pose(), "the pose, {what}");
            assert_eq!(plain.animator().state(), smooth.animator().state(), "the state, {what}");
            assert_eq!(plain.animator().drawn_pose().as_slice(), plain.animator().pose(), "not asked, {what}");
            frames += 1;
            other += (smooth.animator().drawn_pose().as_slice() != smooth.animator().pose()) as usize;
        }
        (frames, other)
    };
    let both = |plain: &mut Firearm, smooth: &mut Firearm, what: &dyn Fn(&mut Firearm) -> bool| assert_eq!(what(plain), what(smooth));
    both(&mut plain, &mut smooth, &|w| {
        w.carried();
        w.draw();
        true
    });
    let (frames, other) = run(&mut plain, &mut smooth, 4.0, "the draw and the rest");
    println!("  the draw and the rest: {other} of {frames} frames drawn off the engine's lines");
    both(&mut plain, &mut smooth, &|w| {
        w.reload();
        true
    });
    let (frames, other) = run(&mut plain, &mut smooth, 6.0, "the reload");
    println!("  the reload: {other} of {frames} frames drawn off the engine's lines");
    assert!(other > frames / 4, "the reload is drawn as the engine has it on {} of {frames} frames", frames - other);
    both(&mut plain, &mut smooth, &|w| w.examine());
    let (frames, other) = run(&mut plain, &mut smooth, 3.0, "the look");
    println!("  three seconds of the look: {other} of {frames} frames drawn off the engine's lines");
    assert!(other > frames / 2);
    run(&mut plain, &mut smooth, 2.0, "the look's end and the rest");
    assert!(plain.is_idle() && smooth.is_idle());
    // Asked no more, it is drawn as the engine has it again from the next
    // frame.
    smooth.set_drawn_smooth(false);
    plain.step(FRAME);
    smooth.step(FRAME);
    assert_eq!(smooth.animator().drawn_pose().as_slice(), smooth.animator().pose());
}
