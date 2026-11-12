//! Tests of the animators' split into the bones each controller follows and its clips in two.
//! After `docs/spec/body-animator.md`, "Layers": the bones each controller follows and carries; its clips split in two, each part sampling
//! to the bit as the clip whole; and the pose an update's mix makes, asked at once or long
//! after, the pose the whole evaluation made, frame after frame, with the pose for drawing
//! smooth and not. Each passes with a note where no install is configured.

use std::sync::Arc;

use trp_data::unity::Assets;
use trp_sim::base::animator::{Animator, Controller, Mix};
use trp_sim::locomotion::body::BodyData;
use trp_sim::base::rig::SightNodes;
use trp_sim::weapons::weapon_animator::WeaponAnimatorData;

const AKM: &str = "59d6088586f774275f37482f";
const RHINO: &str = "61a4c8884f95bc3b2c5dc96f";
const SPINE: &str = "Root_Joint/Base HumanPelvis/Base HumanSpine1/Base HumanSpine2/Base HumanSpine3";

/// The body's further bones as the replay asks for them (the root again, the pelvis, the first
/// spine bone and both legs to the toes), and the neck and the head.
fn drawn() -> Vec<String> {
    let pelvis = "Root_Joint/Base HumanPelvis";
    let mut paths = vec!["Root_Joint".to_owned(), pelvis.to_owned(), format!("{pelvis}/Base HumanSpine1")];
    for side in ['L', 'R'] {
        let mut path = pelvis.to_owned();
        for bone in ["Thigh1", "Thigh2", "Calf", "Foot", "Toe"] {
            path = format!("{path}/Base Human{side}{bone}");
            paths.push(path.clone());
        }
    }
    paths.push(format!("{SPINE}/Base HumanNeck"));
    paths.push(format!("{SPINE}/Base HumanNeck/Base HumanHead"));
    paths
}

/// A controller as loaded, every bone worked out at each update, and the same split at the
/// bones its host follows, with its bones' paths.
struct Both {
    name: &'static str,
    whole: Arc<Controller>,
    split: Arc<Controller>,
}

impl Both {
    fn paths(&self) -> Vec<&str> {
        self.whole.bones.iter().map(|bone| bone.path.as_str()).collect()
    }
}

/// The body's, its 38 bones followed (`BodyData::load_with`).
fn body(assets: &mut Assets) -> Both {
    let drawn = drawn();
    let split = BodyData::load_with(assets, &drawn).expect("the body's animator");
    let whole = BodyData::load_unsplit(assets, &drawn).expect("the body's animator whole");
    Both { name: "the body", whole: whole.controller().clone(), split: split.controller().clone() }
}

/// A rig's in a puppet's hands, its followed bones the puppet's (`puppet::followed_rig_bones`):
/// a weapon's with the sight it is first aimed by, the empty hands' with the sight found on them.
fn rig(assets: &mut Assets, name: &'static str, template: Option<&str>) -> Both {
    let db = crate::fixture::database().expect("database");
    let (weapon, sights) = match template {
        Some(template) => {
            let weapon = trp_data::weapons::weapon::assemble(assets, db, template).expect("the weapon");
            let sights = SightNodes::of_weapon(&weapon);
            (weapon, sights)
        }
        None => {
            let rig = trp_data::weapons::weapon::assemble_prefab(assets, trp_data::weapons::weapon::EMPTY_HANDS).expect("the empty hands' rig");
            let sights = SightNodes::find(&rig.hierarchy);
            (rig, sights)
        }
    };
    let data = WeaponAnimatorData::load(assets, &weapon).expect("its animator");
    let followed = trp_sim::characters::puppet::followed_rig_bones(&weapon.hierarchy, sights.as_ref());
    let split = data.following(&followed).expect("split");
    Both { name, whole: data.controller().clone(), split: split.controller().clone() }
}

fn assets() -> Option<Assets> {
    let install = crate::fixture::install()?;
    Some(Assets::new(install).expect("manifest"))
}

/// The bones each controller follows (what the simulation reads of its pose after each update)
/// and those it carries (a layer may leave them as they stood: an override layer one of whose
/// states writes no defaults and may play no clip there), printed for the body, the AKM, the
/// Rhino and the empty hands. The body follows its 38; the Rhino carries its cylinder (its
/// `FIRE` state's empty `Cylinder` layer, which writes no defaults); every rig follows its
/// moved bone and markers and leaves its arms and fingers to be made when asked.
#[test]
fn the_followed_and_carried_bones() {
    let Some(mut assets) = assets() else { return };
    let all = [body(&mut assets), rig(&mut assets, "the AKM", Some(AKM)), rig(&mut assets, "the Rhino", Some(RHINO)), rig(&mut assets, "the empty hands", None)];
    for both in &all {
        let c = &both.split;
        let paths = both.paths();
        let count = |set: &[bool]| set.iter().filter(|b| **b).count();
        let (followed, carried, always) = (c.followed(), c.carried(), c.always());
        let end = |bone: usize| paths[bone].rsplit('/').next().unwrap_or(paths[bone]);
        let named = |set: &[bool]| (0..set.len()).filter(|b| set[*b]).map(end).collect::<Vec<_>>().join(", ");
        println!(
            "{}: {} bones, {} followed, {} carried ({} of them not followed: worked out on an update a layer leaves them standing), {} worked out at every update, {} made when asked",
            both.name,
            paths.len(),
            count(followed),
            count(carried),
            (0..paths.len()).filter(|b| carried[*b] && !followed[*b]).count(),
            count(always),
            paths.len() - count(always)
        );
        println!("  followed: {}", named(followed));
        println!("  carried: {}", named(carried));
        // Every bone followed is worked out at every update (a carried one on the updates a layer
        // leaves it standing); the whole controller works out every bone.
        assert!((0..paths.len()).all(|b| !followed[b] || always[b]), "{}", both.name);
        assert!(both.whole.always().iter().all(|a| *a), "{} whole", both.name);
        assert_eq!(both.whole.carried(), carried, "{}: the same layers carry, split or not", both.name);
        assert!(count(always) < paths.len(), "{}: nothing left to make when asked", both.name);
    }
    let [body, akm, rhino, empty] = &all;
    assert_eq!(body.split.followed().iter().filter(|b| **b).count(), 38, "the body's own bones");
    // The legs are made from the mix, not at every update.
    let legs = body.paths().iter().enumerate().filter(|(_, path)| path.ends_with("Base HumanLCalf") || path.ends_with("Base HumanRFoot")).map(|(bone, _)| bone).collect::<Vec<_>>();
    assert!(!legs.is_empty() && legs.iter().all(|&bone| !body.split.always()[bone]), "the legs {legs:?}");
    // The Rhino's `Cylinder` layer: every bone it moves (the cylinder is the revolver's magazine
    // part) carried, and not followed.
    let layer = rhino.split.layer("Cylinder").expect("the Rhino's cylinder layer");
    let moved: Vec<usize> = (0..rhino.paths().len()).filter(|b| rhino.split.layers[layer].moves[*b]).collect();
    println!("the Rhino's Cylinder layer moves {:?}", moved.iter().map(|b| rhino.paths()[*b]).collect::<Vec<_>>());
    assert!(!moved.is_empty() && moved.iter().all(|b| rhino.split.carried()[*b]), "the Rhino's cylinder carried");
    assert!(moved.iter().any(|b| rhino.paths()[*b].ends_with("mod_magazine") && !rhino.split.always()[*b]));
    for both in [akm, rhino, empty] {
        let paths = both.paths();
        let at = |end: &str| paths.iter().position(|path| path.ends_with(end));
        let moved = at("Weapon_root/Weapon_root_anim").expect("a moved bone");
        assert!(both.split.followed()[moved], "{}: the moved bone", both.name);
        let palm = at("Base HumanRPalm").expect("a palm");
        assert!(!both.split.always()[palm], "{}: the right palm is drawing alone", both.name);
    }
}

/// Every clip of the body's and the AKM's controllers split in two gives each bone, each
/// parameter and the root, sampled and sampled alone, on straight lines and smooth, exactly
/// what the clip whole gives, at times all through it and past both ends.
#[test]
fn a_split_clip_samples_as_the_clip_whole() {
    let Some(mut assets) = assets() else { return };
    let bits = |values: &[f32]| values.iter().map(|v| v.to_bits()).collect::<Vec<_>>();
    for both in [body(&mut assets), rig(&mut assets, "the AKM", Some(AKM))] {
        let (mut values, mut clips) = (0usize, 0usize);
        for clip in 0..both.whole.clips.len() {
            let length = both.whole.clips[clip].clip.duration();
            assert_eq!(both.split.clips[clip].clip.name, both.whole.clips[clip].clip.name);
            assert_eq!(both.split.clips[clip].clip.events, both.whole.clips[clip].clip.events);
            assert_eq!(length.to_bits(), both.split.clips[clip].clip.duration().to_bits());
            clips += 1;
            for step in -2..=40 {
                let time = length * step as f32 / 37.0 + 0.0013 * (step % 3) as f32;
                for smooth in [false, true] {
                    let (whole, split) = (both.whole.sampled(clip, time, smooth), both.split.sampled(clip, time, smooth));
                    for bone in 0..whole.bones.len() {
                        for k in 0..3 {
                            assert_eq!(bits(&whole.bones[bone][k]), bits(&split.bones[bone][k]), "{} clip {clip} bone {bone} track {k} at {time} (smooth {smooth})", both.name);
                            assert_eq!(bits(&whole.references[bone][k]), bits(&split.references[bone][k]), "{} clip {clip} bone {bone}'s reference", both.name);
                            values += whole.bones[bone][k].len();
                        }
                    }
                    let curves = |s: &trp_sim::base::animator::Sampled| s.curves.iter().map(|(p, v)| (*p, v.to_bits())).collect::<Vec<_>>();
                    assert_eq!(curves(&whole), curves(&split), "{} clip {clip} curves at {time}", both.name);
                    assert_eq!(bits(&whole.root), bits(&split.root), "{} clip {clip} root at {time}", both.name);
                    assert_eq!(bits(&whole.root_alone), bits(&split.root_alone), "{} clip {clip} root alone at {time}", both.name);
                    values += whole.curves.len() + whole.root.len();
                }
            }
        }
        println!("{}: {clips} clips, {values} values sampled split and whole, every one the same to the bit", both.name);
    }
}

/// A small generator of its own, so that the drive repeats.
struct Draws(u64);

impl Draws {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }

    fn below(&mut self, n: usize) -> usize {
        ((self.next() * n as f32) as usize).min(n.saturating_sub(1))
    }
}

/// One frame's drive of an animator: parameters set at random (floats over the ranges the game
/// gives them, flags and triggers on and off), now and then a layer's weight, now and then a
/// state of any layer asked for outright (read or not), and a step of 1/144 to 1/30 s.
#[derive(Clone)]
struct Drive {
    set: Vec<(usize, f32)>,
    weights: Vec<(usize, f32)>,
    play: Option<(usize, usize)>,
    dt: f32,
}

/// The states of a controller's override layers that leave the layer's bones as they stood
/// when they play: they write no defaults and have no tree on the layer's slot (no motion, or
/// their clips not read), by layer and state.
fn standing_states(controller: &Controller) -> Vec<(usize, usize)> {
    let mut found = Vec::new();
    for (index, layer) in controller.layers.iter().enumerate().filter(|(_, layer)| !layer.additive) {
        for (state, def) in controller.machines[layer.machine].states.iter().enumerate() {
            if !def.writes_defaults && def.trees.get(layer.slot).and_then(Option::as_ref).is_none() {
                found.push((index, state));
            }
        }
    }
    found
}

fn drive(draws: &mut Draws, controller: &Controller, standing: &[(usize, usize)]) -> Drive {
    let mut set = Vec::new();
    for (parameter, p) in controller.parameters.iter().enumerate() {
        if draws.next() > 0.08 {
            continue;
        }
        let value = match p.kind {
            trp_data::weapons::states::ParameterKind::Float => {
                if draws.next() < 0.2 {
                    (draws.next() - 0.5) * 180.0
                } else {
                    (draws.next() - 0.5) * 4.0
                }
            }
            trp_data::weapons::states::ParameterKind::Int => draws.below(16) as f32 - 1.0,
            _ => (draws.next() < 0.5) as u8 as f32,
        };
        set.push((parameter, value));
    }
    let mut weights = Vec::new();
    for layer in 1..controller.layers.len() {
        if draws.next() < 0.03 {
            weights.push((layer, [0.0, 0.25, 0.6, 1.0][draws.below(4)]));
        }
    }
    // Now and then a state that leaves its layer's bones standing, as often any state.
    let play = if !standing.is_empty() && draws.next() < 0.012 {
        Some(standing[draws.below(standing.len())])
    } else {
        (draws.next() < 0.015).then(|| {
            let layer = draws.below(controller.layers.len());
            let states = controller.machines[controller.layers[layer].machine].states.len();
            (layer, draws.below(states.max(1)))
        })
    };
    Drive { set, weights, play, dt: 1.0 / 144.0 + draws.next() * (1.0 / 30.0 - 1.0 / 144.0) }
}

fn apply(animator: &mut Animator, drive: &Drive) -> Vec<trp_sim::base::animator::Event> {
    for &(parameter, value) in &drive.set {
        animator.set(parameter, value);
    }
    for &(layer, weight) in &drive.weights {
        animator.set_layer_weight(layer, weight);
    }
    if let Some((layer, state)) = drive.play {
        animator.play(layer, state, 0.0);
    }
    animator.update(drive.dt)
}

type Pose = Vec<([f32; 3], [f32; 4])>;

fn pose_bits(pose: &[([f32; 3], [f32; 4])]) -> Vec<u32> {
    pose.iter().flat_map(|(p, q)| p.iter().chain(q).map(|v| v.to_bits())).collect()
}

/// The pose an update's mix makes (`pose_of`, `drawn_of`) is the pose the whole evaluation made
/// on that update, to the bit, over many frames of the body, the AKM and the empty hands driven
/// at random (states asked for outright, read or not, layers weighed in and out), with the pose
/// for drawing smooth and not: one animator on the controller whole, every bone worked out at
/// each update; one on it split, deferred, each frame's mix kept and its pose made only after
/// the last frame. Every frame both give the same events, root motion, parameters, states and
/// bones worked out at every update.
#[test]
fn the_pose_a_mix_makes_is_the_whole_evaluations() {
    let Some(mut assets) = assets() else { return };
    let all = [body(&mut assets), rig(&mut assets, "the AKM", Some(AKM)), rig(&mut assets, "the Rhino", Some(RHINO)), rig(&mut assets, "the empty hands", None)];
    for both in &all {
        let standing = standing_states(&both.whole);
        println!("{}: {} states that leave their layer's bones standing", both.name, standing.len());
        for smooth in [false, true] {
            let mut whole = Animator::new(both.whole.clone());
            let mut split = Animator::new(both.split.clone());
            whole.set_drawn_smooth(smooth);
            split.set_drawn_smooth(smooth);
            split.set_deferred(true);
            let mut draws = Draws(0x9e37_79b9_7f4a_7c15 ^ both.whole.bones.len() as u64);
            let mut kept: Vec<(Mix, Pose, Pose)> = Vec::new();
            let (frames, mut kept_frames, mut standing_frames, mut most_standing) = (900, 0, 0, 0);
            for frame in 0..frames {
                // A run of frames now and then with nothing deferred: from deferred to not, the
                // bones a layer leaves standing are made from the last mix where they must be.
                split.set_deferred(frame % 97 >= 9);
                let drive = drive(&mut draws, &both.whole, &standing);
                let (said, said_split) = (apply(&mut whole, &drive), apply(&mut split, &drive));
                let what = format!("{} (smooth {smooth}), frame {frame}", both.name);
                assert_eq!(said, said_split, "the events, {what}");
                assert_eq!(whole.looks(), split.looks(), "the looks, {what}");
                assert_eq!(whole.passed_events().collect::<Vec<_>>(), split.passed_events().collect::<Vec<_>>(), "the clips' events, {what}");
                assert_eq!(whole.root_motion().map(f32::to_bits), split.root_motion().map(f32::to_bits), "the root's travel, {what}");
                assert_eq!(whole.root_turn().to_bits(), split.root_turn().to_bits(), "the root's turn, {what}");
                for parameter in 0..both.whole.parameters.len() {
                    assert_eq!(whole.get(parameter).to_bits(), split.get(parameter).to_bits(), "parameter {parameter}, {what}");
                }
                assert_eq!(whole.describe(), split.describe(), "the layers, {what}");
                let scales = |a: &Animator| a.scales().map(|(bone, s)| (bone, s.map(f32::to_bits))).collect::<Vec<_>>();
                assert_eq!(scales(&whole), scales(&split), "the scales, {what}");
                for bone in (0..both.whole.bones.len()).filter(|b| both.split.always()[*b]) {
                    assert_eq!(pose_bits(&[whole.bone(bone)]), pose_bits(&[split.bone(bone)]), "bone {bone} worked out at the update, {what}");
                }
                // A pose made at once from the mix is the update's.
                assert_eq!(pose_bits(whole.pose()), pose_bits(&split.pose_of(split.mix())), "the pose made at once, {what}");
                let (stood, stood_drawn) = split.mix().standing();
                let stood = stood.max(stood_drawn.unwrap_or(0));
                standing_frames += usize::from(stood > 0);
                most_standing = most_standing.max(stood);
                if frame % 3 == 0 {
                    kept.push((split.mix().clone(), whole.pose().to_vec(), whole.drawn_pose().to_vec()));
                    kept_frames += 1;
                }
            }
            // Made long after, from the mixes kept: the poses the updates made.
            let mut differ = 0;
            for (n, (mix, pose, drawn)) in kept.iter().enumerate() {
                let (made, made_drawn) = (split.pose_of(mix), split.drawn_of(mix));
                if pose_bits(&made) != pose_bits(pose) || pose_bits(&made_drawn) != pose_bits(drawn) {
                    differ += 1;
                    let bad: Vec<usize> = (0..pose.len()).filter(|b| pose_bits(&[made[*b]]) != pose_bits(&[pose[*b]]) || pose_bits(&[made_drawn[*b]]) != pose_bits(&[drawn[*b]])).collect();
                    println!("{} (smooth {smooth}): kept mix {n} differs at bones {bad:?}", both.name);
                }
            }
            println!(
                "{} (smooth {smooth}): {frames} frames driven ({standing_frames} with bones a layer left standing worked out beside the followed, at most {most_standing}), {kept_frames} mixes kept and posed after the last, {differ} differ",
                both.name
            );
            assert_eq!(differ, 0, "{} (smooth {smooth})", both.name);
            assert!(smooth || pose_bits(whole.drawn_pose()) == pose_bits(whole.pose()));
        }
    }
}
