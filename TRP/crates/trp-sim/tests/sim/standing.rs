//! The character as the game's own screens stand it (`BodyData::load_standing`).

use trp_data::unity::Assets;
use trp_sim::math::{quat_mul, quat_rotate};

const SPINE: &str = "Root_Joint/Base HumanPelvis/Base HumanSpine1/Base HumanSpine2/Base HumanSpine3/Base HumanRibcage";

/// A bone's path and those above it, each once, a parent before its child.
fn chain(paths: &mut Vec<String>, path: &str) {
    for (end, _) in path.match_indices('/').chain(std::iter::once((path.len(), ""))) {
        if !paths.iter().any(|known| known == &path[..end]) {
            paths.push(path[..end].to_owned());
        }
    }
}

/// With nothing in its hands it stands straight, its arms down at its
/// sides (`inv_idle_hands_down`), its feet on the ground: the hands below
/// the hips and either side of them, the head above all of it.
#[test]
fn the_character_stands_as_the_game_s_screens_stand_it() {
    let Ok(install) = crate::fixture::install().ok_or(()) else {
        return;
    };
    let mut assets = Assets::new(install).expect("manifest");
    let mut paths = Vec::new();
    for side in ['L', 'R'] {
        chain(&mut paths, &format!("{SPINE}/Base Human{side}Collarbone/Base Human{side}Upperarm/Base Human{side}Forearm1/Base Human{side}Forearm2/Base Human{side}Forearm3/Base Human{side}Palm"));
        chain(&mut paths, &format!("Root_Joint/Base HumanPelvis/Base Human{side}Thigh1/Base Human{side}Thigh2/Base Human{side}Calf/Base Human{side}Foot"));
    }
    chain(&mut paths, "Root_Joint/Base HumanPelvis/Base HumanSpine1/Base HumanSpine2/Base HumanSpine3/Base HumanNeck/Base HumanHead");
    let started = std::time::Instant::now();
    let (_, standing) = trp_sim::body::BodyData::load_standing(&mut assets, &[], &paths).expect("body animator");
    let pose = standing.expect("asked for").expect("the figure");
    println!("the body's animator and the standing figure read in {:.0} ms", started.elapsed().as_secs_f32() * 1000.0);
    assert_eq!(pose.len(), paths.len());
    // Each bone in the root's axes, down its chain.
    let mut world: Vec<([f32; 3], [f32; 4])> = Vec::with_capacity(paths.len());
    for (index, path) in paths.iter().enumerate() {
        let parent = path.rfind('/').and_then(|at| paths.iter().position(|known| known == &path[..at]));
        let (position, rotation) = pose[index];
        world.push(match parent {
            Some(parent) => {
                let (at, turn) = world[parent];
                let shifted = quat_rotate(turn, position);
                (std::array::from_fn(|k| at[k] + shifted[k]), quat_mul(turn, rotation))
            }
            None => (position, rotation),
        });
    }
    let at = |end: &str| world[paths.iter().position(|path| path.ends_with(end)).expect(end)].0;
    let (pelvis, head) = (at("/Base HumanPelvis"), at("/Base HumanHead"));
    let hands = [at("/Base HumanLPalm"), at("/Base HumanRPalm")];
    let feet = [at("/Base HumanLFoot"), at("/Base HumanRFoot")];
    let print = |name: &str, p: [f32; 3]| println!("{name}: {:.3} {:.3} {:.3}", p[0], p[1], p[2]);
    print("pelvis", pelvis);
    print("head", head);
    print("left upper arm", at("/Base HumanLUpperarm"));
    print("left forearm", at("/Base HumanLForearm1"));
    print("left palm", hands[0]);
    print("right palm", hands[1]);
    print("left foot", feet[0]);
    print("right foot", feet[1]);
    for hand in hands {
        assert!(hand[1] < pelvis[1] + 0.1, "a hand at {hand:?}, the pelvis at {pelvis:?}");
        assert!((hand[0] - pelvis[0]).abs() > 0.12, "a hand at {hand:?} is not beside the hips at {pelvis:?}");
    }
    assert!(hands[0][0].signum() != hands[1][0].signum(), "both hands on one side: {hands:?}");
    for foot in feet {
        assert!(foot[1] < 0.2, "a foot off the ground: {foot:?}");
    }
    assert!(head[1] > 1.4 && head[1] > hands[0][1] + 0.6, "{head:?}");
}
