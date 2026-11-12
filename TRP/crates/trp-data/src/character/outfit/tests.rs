//! Tests of an outfit of the other game's: its bones, its build and how its rig follows the install's.

use super::*;

/// A stick figure of the other game's kind (y up, z ahead): its
/// bones, each with a place, the left side's given and the right
/// side's mirrored.
fn stick_figure() -> Vec<(String, V3)> {
    let middle = [("bip01_pelvis", [0.0, 1.0, 0.0]), ("bip01_spine", [0.0, 1.1, 0.0]), ("bip01_spine1", [0.0, 1.2, 0.0]), ("bip01_neck", [0.0, 1.55, 0.0]), ("bip01_head", [0.0, 1.65, 0.0])];
    let left = [
        ("thigh", [-0.1, 1.0, 0.0]),
        ("calf", [-0.1, 0.55, 0.05]),
        ("foot", [-0.1, 0.1, 0.0]),
        ("toe0", [-0.1, 0.0, 0.15]),
        ("clavicle", [-0.05, 1.5, 0.0]),
        ("upperarm", [-0.2, 1.5, 0.0]),
        ("forearm", [-0.5, 1.5, -0.05]),
        ("hand", [-0.75, 1.5, 0.05]),
        ("finger0", [-0.78, 1.5, 0.1]),
        ("finger01", [-0.8, 1.5, 0.13]),
        ("finger02", [-0.82, 1.5, 0.16]),
        ("finger1", [-0.85, 1.5, 0.09]),
        ("finger11", [-0.89, 1.5, 0.09]),
        ("finger12", [-0.92, 1.49, 0.09]),
        ("finger2", [-0.85, 1.5, 0.05]),
        ("finger21", [-0.89, 1.5, 0.05]),
        ("finger22", [-0.92, 1.49, 0.05]),
    ];
    let mut bones: Vec<(String, V3)> = middle.iter().map(|(name, at)| ((*name).to_owned(), *at)).collect();
    for (name, at) in left {
        bones.push((format!("bip01_l_{name}"), at));
        bones.push((format!("bip01_r_{name}"), [-at[0], at[1], at[2]]));
    }
    bones
}

/// The same of the install's kind (z up, ahead along minus y), with
/// its joints at other places and more of them: narrower, its arms
/// forward.
fn their_stick_figure() -> Vec<(String, V3)> {
    let middle = [("Pelvis", [0.0, 0.0, 0.95]), ("Spine1", [0.0, 0.02, 1.0]), ("Spine2", [0.0, 0.05, 1.15]), ("Spine3", [0.0, 0.08, 1.35]), ("Neck", [0.0, 0.06, 1.55]), ("Head", [0.0, 0.02, 1.65])];
    let left = [
        ("Thigh1", [-0.08, 0.0, 0.95]),
        ("Thigh2", [-0.09, -0.02, 0.72]),
        ("Calf", [-0.1, -0.04, 0.5]),
        ("Foot", [-0.12, 0.05, 0.06]),
        ("Toe", [-0.12, -0.1, 0.02]),
        ("Collarbone", [-0.05, 0.0, 1.52]),
        ("Upperarm", [-0.16, 0.04, 1.47]),
        ("Forearm1", [-0.34, -0.17, 1.38]),
        ("Forearm2", [-0.355, -0.25, 1.38]),
        ("Forearm3", [-0.37, -0.33, 1.385]),
        ("Palm", [-0.39, -0.42, 1.39]),
        ("Digit11", [-0.35, -0.46, 1.4]),
        ("Digit12", [-0.33, -0.5, 1.41]),
        ("Digit13", [-0.31, -0.53, 1.41]),
        ("Digit21", [-0.375, -0.525, 1.42]),
        ("Digit22", [-0.365, -0.58, 1.405]),
        ("Digit23", [-0.355, -0.6, 1.39]),
        ("Digit31", [-0.4, -0.525, 1.405]),
        ("Digit32", [-0.39, -0.58, 1.387]),
        ("Digit33", [-0.375, -0.6, 1.37]),
        ("Digit41", [-0.415, -0.52, 1.385]),
        ("Digit42", [-0.41, -0.565, 1.365]),
        ("Digit43", [-0.395, -0.58, 1.345]),
        ("Digit51", [-0.42, -0.515, 1.36]),
        ("Digit52", [-0.418, -0.55, 1.34]),
        ("Digit53", [-0.405, -0.557, 1.327]),
    ];
    let mut bones: Vec<(String, V3)> = middle.iter().map(|(name, at)| ((*name).to_owned(), *at)).collect();
    for (name, at) in left {
        bones.push((format!("L{name}"), at));
        bones.push((format!("R{name}"), [-at[0], at[1], at[2]]));
    }
    bones
}

/// The two as a model and a skeleton. The model's one mesh has a
/// vertex at every joint, going wholly with that joint's bone, and
/// after those what `more` lists: a vertex at one bone's joint going
/// wholly with another bone.
fn made(more: &[(&str, &str)]) -> (xray::Model, Vec<RestBone>) {
    let figure = stick_figure();
    let index = |name: &str| figure.iter().position(|(known, _)| known == name).unwrap();
    let bones = figure.iter().map(|(name, at)| xray::Bone { name: name.clone(), parent: None, at: *at, axes: NO_TURN }).collect();
    let whole = |bone: usize| [(bone as u16, 1.0), (0, 0.0), (0, 0.0), (0, 0.0)];
    let mut part = xray::Part { texture: "test".into(), positions: Vec::new(), normals: Vec::new(), uv: Vec::new(), links: Vec::new(), triangles: Vec::new() };
    for (at, with) in (0..figure.len()).map(|bone| (bone, bone)).chain(more.iter().map(|(at, with)| (index(at), index(with)))) {
        part.positions.push(figure[at].1);
        part.normals.push([0.0, 1.0, 0.0]);
        part.uv.push([0.0, 0.0]);
        part.links.push(whole(with));
    }
    let corners = |names: [&str; 3]| names.map(|name| index(name) as u32);
    part.triangles.extend(corners(["bip01_l_thigh", "bip01_l_calf", "bip01_l_foot"]));
    part.triangles.extend(corners(["bip01_l_hand", "bip01_l_finger1", "bip01_l_finger2"]));
    part.triangles.extend(corners(["bip01_head", "bip01_neck", "bip01_spine1"]));
    // A bone's matrix: a point of the mesh less the bone's place.
    let skeleton = their_stick_figure()
        .into_iter()
        .map(|(name, at)| RestBone { path: format!("Root/Base Human{name}"), bind: [1.0, 0.0, 0.0, -at[0], 0.0, 1.0, 0.0, -at[1], 0.0, 0.0, 1.0, -at[2], 0.0, 0.0, 0.0, 1.0] })
        .collect();
    (xray::Model { bones, parts: vec![part] }, skeleton)
}

fn near(a: V3, b: V3) -> bool {
    length(sub(a, b)) < 1e-4
}

fn apart(a: V3, b: V3) -> f32 {
    length(sub(a, b))
}

/// The rig's bone for one of the install's, by the end of its name.
fn bone(rig: &Rig, name: &str) -> usize {
    rig.bones.iter().position(|bone| bone.path.ends_with(&format!("Base Human{name}"))).unwrap_or_else(|| panic!("no bone {name}"))
}

#[test]
fn a_bone_s_place_at_rest_is_read_off_its_matrix() {
    // A quarter turn about z and a shift: the bone's own origin is
    // where the matrix gives nothing.
    let bind = [0.0, -1.0, 0.0, 2.0, 1.0, 0.0, 0.0, -3.0, 0.0, 0.0, 1.0, -5.0, 0.0, 0.0, 0.0, 1.0];
    let at = rest_place(&bind).unwrap();
    let through = |row: usize| bind[row * 4] * at[0] + bind[row * 4 + 1] * at[1] + bind[row * 4 + 2] * at[2] + bind[row * 4 + 3];
    assert!((0..3).all(|row| through(row).abs() < 1e-5), "{at:?}");
}

/// The model keeps its own build: its trunk and head stand as they
/// are (as broad, as tall), every bone of a limb and of a finger keeps
/// its length, and each joint is where the rig has that bone's joint.
/// The hand is on the install's hand and the sole on its ground.
#[test]
fn the_model_keeps_its_build() {
    let (model, skeleton) = made(&[]);
    let (meshes, rig) = fit(&model, &skeleton, None).unwrap();
    let mesh = &meshes[0];
    let at = |name: &str| mesh.positions[model.bones.iter().position(|bone| bone.name == name).unwrap()];
    let ours: HashMap<String, V3> = stick_figure().into_iter().collect();
    let theirs: HashMap<String, V3> = their_stick_figure().into_iter().collect();
    // As broad and as tall as it was, on the install's ground.
    assert!((apart(at("bip01_l_thigh"), at("bip01_r_thigh")) - 0.2).abs() < 1e-5);
    assert!((apart(at("bip01_l_upperarm"), at("bip01_r_upperarm")) - 0.4).abs() < 1e-5);
    assert!(near(at("bip01_head"), [0.0, 0.0, 1.65]) && near(at("bip01_l_upperarm"), [-0.2, 0.0, 1.5]), "{:?}", at("bip01_head"));
    // Every bone as long as it was.
    let pairs = [("thigh", "calf"), ("calf", "foot"), ("upperarm", "forearm"), ("forearm", "hand"), ("hand", "finger2"), ("finger0", "finger01"), ("finger01", "finger02"), ("finger1", "finger11"), ("finger11", "finger12"), ("finger2", "finger21"), ("finger21", "finger22")];
    for side in ["l", "r"] {
        for (from, to) in pairs {
            let (from, to) = (format!("bip01_{side}_{from}"), format!("bip01_{side}_{to}"));
            let (was, is) = (apart(ours[&from], ours[&to]), apart(at(&from), at(&to)));
            assert!((was - is).abs() < 1e-4, "{from} to {to}: {was} long, now {is}");
        }
    }
    // Each joint where the rig has it, and going wholly with it.
    let joints = [("upperarm", "Upperarm"), ("forearm", "Forearm1"), ("hand", "Palm"), ("finger0", "Digit11"), ("finger11", "Digit22"), ("finger22", "Digit33"), ("thigh", "Thigh1"), ("calf", "Calf"), ("foot", "Foot"), ("toe0", "Toe"), ("clavicle", "Collarbone")];
    for (side, their_side) in [("l", "L"), ("r", "R")] {
        for (name, their_name) in joints {
            let vertex = model.bones.iter().position(|bone| bone.name == format!("bip01_{side}_{name}")).unwrap();
            let rig_bone = bone(&rig, &format!("{their_side}{their_name}"));
            assert!(near(mesh.positions[vertex], rig.bones[rig_bone].joint), "{side} {name}: {:?} for {:?}", mesh.positions[vertex], rig.bones[rig_bone].joint);
            assert_eq!((mesh.joints[vertex][0] as usize, mesh.weights[vertex][0]), (rig_bone, 1.0), "{side} {name}");
        }
    }
    // The hand on the install's hand, knuckle on knuckle; the foot
    // over the install's ankle with its own ankle's height over its
    // sole, which is on the ground.
    assert!(near(at("bip01_l_finger2"), theirs["LDigit31"]), "{:?}", at("bip01_l_finger2"));
    assert!(near(at("bip01_l_foot"), [-0.12, 0.05, 0.1]) && near(at("bip01_l_toe0"), [-0.12, -0.1, 0.0]), "{:?} {:?}", at("bip01_l_foot"), at("bip01_l_toe0"));
    // Every vertex's shares add up to one, the three triangles are a
    // leg's, an arm's and the rest's, and a mesh's matrices move
    // nothing.
    assert!(mesh.weights.iter().all(|shares| (shares.iter().sum::<f32>() - 1.0).abs() < 1e-5));
    assert_eq!(mesh.submeshes.iter().map(Vec::len).collect::<Vec<_>>(), [3, 3, 3]);
    assert_eq!(mesh.bind_poses.len(), rig.bones.len());
    // A bone is listed after what it hangs from.
    for (index, bone) in rig.bones.iter().enumerate() {
        match bone.follows {
            Follows::Hung { from } | Follows::Along { of: from } | Follows::Limb { from, .. } | Follows::Held { from, .. } => assert!(from < index, "{}", bone.path),
            Follows::Theirs | Follows::Lower => {}
        }
    }
}

/// What goes with the trunk is shared among the rig's spine bones by
/// its height; what lies at the far end of a forearm or a thigh goes
/// with the last of the install's bones along it.
#[test]
fn shares_go_by_where_a_vertex_lies() {
    let more = [("bip01_l_thigh", "bip01_pelvis"), ("bip01_r_upperarm", "bip01_spine1"), ("bip01_l_hand", "bip01_l_forearm"), ("bip01_r_calf", "bip01_r_thigh")];
    let (model, skeleton) = made(&more);
    let (meshes, rig) = fit(&model, &skeleton, None).unwrap();
    let mesh = &meshes[0];
    let first = model.bones.len();
    let went = |vertex: usize| -> Vec<(&str, f32)> { (0..4).filter(|slot| mesh.weights[vertex][*slot] > 1e-5).map(|slot| (rig.bones[mesh.joints[vertex][slot] as usize].path.rsplit("Base Human").next().unwrap(), mesh.weights[vertex][slot])).collect() };
    // The hip, with the pelvis: where the thigh hangs, and the
    // pelvis's.
    assert!(near(mesh.positions[first], rig.bones[bone(&rig, "LThigh1")].joint));
    assert_eq!(went(first), [("Pelvis", 1.0)]);
    // The shoulder, with the chest: where the arm hangs, and shared
    // by the two spine bones about that height.
    assert!(near(mesh.positions[first + 1], rig.bones[bone(&rig, "RUpperarm")].joint));
    let chest = went(first + 1);
    assert!(chest.iter().all(|(bone, _)| ["Spine3", "Neck"].contains(bone)) && chest.len() == 2, "{chest:?}");
    // The wrist, with the forearm: the last third of the forearm's.
    assert!(near(mesh.positions[first + 2], rig.bones[bone(&rig, "LPalm")].joint), "{:?}", mesh.positions[first + 2]);
    assert_eq!(went(first + 2), [("LForearm3", 1.0)]);
    // The knee, with the thigh: the lower half of the thigh's.
    assert!(near(mesh.positions[first + 3], rig.bones[bone(&rig, "RCalf")].joint));
    assert_eq!(went(first + 3), [("RThigh2", 1.0)]);
}

/// A forearm turns with the hand: what lies at its far end is turned
/// about it as the hand is, and what lies at the elbow is not.
#[test]
fn a_forearm_is_twisted_towards_the_hand() {
    let (model, _) = made(&[]);
    let at = |name: &str| model.bones.iter().find(|bone| bone.name == name).unwrap().at;
    let theirs: HashMap<String, V3> = their_stick_figure().into_iter().collect();
    let (elbow, wrist, knuckle, first) = (at("bip01_l_forearm"), at("bip01_l_hand"), at("bip01_l_finger2"), at("bip01_l_finger1"));
    let hinge = cross(sub(elbow, at("bip01_l_upperarm")), sub(wrist, elbow));
    let their_fore = sub(theirs["LPalm"], theirs["LForearm1"]);
    let their_hinge = cross(sub(theirs["LForearm1"], theirs["LUpperarm"]), their_fore);
    let mut fore = Laid::bone(elbow, wrist, hinge, theirs["LForearm1"], their_fore, their_hinge).unwrap();
    let hand = Laid::bone(wrist, knuckle, sub(first, knuckle), theirs["LPalm"], sub(theirs["LDigit31"], theirs["LPalm"]), sub(theirs["LDigit21"], theirs["LDigit31"])).unwrap();
    fore.twist = fore.twist_to(&hand);
    assert!(fore.twist.abs() > 0.05, "the two figures' hands are turned alike: {}", fore.twist);
    // A direction across the forearm, at its far end: the forearm
    // and the hand leave it turned the same way about the forearm.
    let across = fore.ours[1];
    let (by_forearm, by_hand) = (fore.turn(across, 1.0), hand.turn(across, 0.0));
    let about = |v: V3| dot(v, fore.theirs[2]).atan2(dot(v, fore.theirs[1]));
    assert!((about(by_forearm) - about(by_hand)).abs() < 1e-4, "{} and {}", about(by_forearm), about(by_hand));
    // At the elbow it is where the elbow's own axes have it.
    assert!(near(fore.turn(across, 0.0), fore.theirs[1]));
}

/// While the install's skeleton is at rest, so is the outfit's rig:
/// every limb is bent where the fit bent it.
#[test]
fn at_rest_the_rig_moves_nothing() {
    let (model, skeleton) = made(&[]);
    let (_, rig) = fit(&model, &skeleton, None).unwrap();
    let posed = rig.pose(&vec![Move::STILL; rig.bones.len()]);
    for (bone, moved) in rig.bones.iter().zip(&posed) {
        let off = (0..3).map(|axis| apart(moved.axes[axis], NO_TURN[axis])).fold(length(moved.shift), f32::max);
        assert!(off < 1e-4, "{} is moved at rest: {moved:?}", bone.path);
    }
}

/// A hand that moves is followed: the arm hangs where it hung, ends
/// at the hand, and keeps both its lengths; what lies along the
/// forearm stays on it. And out of reach the arm is straight.
#[test]
fn a_limb_reaches_with_its_own_lengths() {
    let (model, skeleton) = made(&[]);
    let (_, rig) = fit(&model, &skeleton, None).unwrap();
    let [upper, fore, second, palm, thigh, calf, foot] = ["LUpperarm", "LForearm1", "LForearm2", "LPalm", "RThigh1", "RCalf", "RFoot"].map(|name| bone(&rig, name));
    let joint = |index: usize| rig.bones[index].joint;
    let (upper_long, fore_long) = (apart(joint(upper), joint(fore)), apart(joint(fore), joint(palm)));
    let mut theirs = vec![Move::STILL; rig.bones.len()];
    theirs[palm].shift = [0.03, 0.06, 0.02];
    // And the other leg's foot lifted, as in a step, the install's
    // own knee ahead of where it was.
    theirs[foot].shift = [0.0, -0.1, 0.25];
    theirs[calf].shift = [0.0, -0.15, 0.1];
    let posed = rig.pose(&theirs);
    let (shoulder, elbow, wrist) = (posed[upper].of(joint(upper)), posed[fore].of(joint(fore)), theirs[palm].of(joint(palm)));
    assert!(near(shoulder, joint(upper)) && posed[palm] == theirs[palm]);
    assert!(near(posed[upper].of(joint(fore)), elbow), "the upper arm ends at the elbow");
    assert!(near(posed[fore].of(joint(palm)), wrist), "the forearm ends at the hand: {:?} for {wrist:?}", posed[fore].of(joint(palm)));
    assert!((apart(shoulder, elbow) - upper_long).abs() < 1e-4 && (apart(elbow, wrist) - fore_long).abs() < 1e-4);
    assert!(near(posed[second].of(joint(second)), posed[fore].of(joint(second))), "a bone along the forearm stays on it");
    // The leg the same, its knee further ahead (ahead is minus y) for
    // the foot lifted.
    let (hip, knee, ankle) = (posed[thigh].of(joint(thigh)), posed[calf].of(joint(calf)), theirs[foot].of(joint(foot)));
    assert!(near(hip, joint(thigh)) && near(posed[calf].of(joint(foot)), ankle));
    assert!((apart(hip, knee) - apart(joint(thigh), joint(calf))).abs() < 1e-4 && knee[1] < joint(calf)[1] - 0.05, "{knee:?}");
    // Out of reach: straight from the shoulder towards the hand, and
    // short of it.
    theirs[palm].shift = [-1.0, 0.0, 0.0];
    let posed = rig.pose(&theirs);
    let (shoulder, end, hand) = (posed[upper].of(joint(upper)), posed[fore].of(joint(palm)), theirs[palm].of(joint(palm)));
    assert!((apart(shoulder, end) - (upper_long + fore_long)).abs() < 1e-3, "{}", apart(shoulder, end));
    assert!(apart(unit(sub(end, shoulder)).unwrap(), unit(sub(hand, shoulder)).unwrap()) < 1e-3);
}

/// Arms alone, as the other game has them for first person: a thumb
/// and four fingers to a hand, a bone more for the forearm's turn, the
/// arms hanging and the forearms ahead.
fn arms_alone() -> xray::Model {
    let left = [
        ("l_upperarm", [-0.18, 0.05, 0.0]),
        ("l_forearm", [-0.23, -0.25, 0.05]),
        ("l_forearm_twist", [-0.31, -0.25, 0.25]),
        ("l_hand", [-0.33, -0.25, 0.3]),
        ("l_finger0", [-0.33, -0.23, 0.31]),
        ("l_finger01", [-0.325, -0.215, 0.35]),
        ("l_finger02", [-0.325, -0.215, 0.375]),
        ("bip01_l_finger1", [-0.37, -0.23, 0.355]),
        ("bip01_l_finger11", [-0.37, -0.23, 0.39]),
        ("bip01_l_finger12", [-0.36, -0.23, 0.41]),
        ("bip01_l_finger2", [-0.37, -0.25, 0.355]),
        ("bip01_l_finger21", [-0.375, -0.25, 0.395]),
        ("bip01_l_finger22", [-0.365, -0.25, 0.42]),
        ("bip01_l_finger3", [-0.365, -0.27, 0.35]),
        ("bip01_l_finger31", [-0.365, -0.27, 0.39]),
        ("bip01_l_finger32", [-0.35, -0.27, 0.41]),
        ("bip01_l_finger4", [-0.355, -0.285, 0.35]),
        ("bip01_l_finger41", [-0.35, -0.29, 0.385]),
        ("bip01_l_finger42", [-0.34, -0.29, 0.4]),
    ];
    let mut bones = vec![xray::Bone { name: "bip01".into(), parent: None, at: [0.0; 3], axes: NO_TURN }];
    for (name, at) in left {
        bones.push(xray::Bone { name: name.to_owned(), parent: None, at, axes: NO_TURN });
        bones.push(xray::Bone { name: name.replacen("l_", "r_", 1), parent: None, at: [-at[0], at[1], at[2]], axes: NO_TURN });
    }
    let mut part = xray::Part { texture: "test".into(), positions: Vec::new(), normals: Vec::new(), uv: Vec::new(), links: Vec::new(), triangles: vec![1, 3, 5] };
    // A vertex at every joint but the root's, going wholly with it.
    for (index, bone) in bones.iter().enumerate().skip(1) {
        part.positions.push(bone.at);
        part.normals.push([0.0, 1.0, 0.0]);
        part.uv.push([0.0, 0.0]);
        part.links.push([(index as u16, 1.0), (0, 0.0), (0, 0.0), (0, 0.0)]);
    }
    xray::Model { bones, parts: vec![part] }
}

/// A piece made longer along its bone ends where it is asked to, and
/// is no broader for it.
#[test]
fn a_piece_is_made_longer_along_its_bone_alone() {
    let laid = Laid::bone([0.0; 3], [0.0, 0.0, 2.0], [0.0, 1.0, 0.0], [1.0, 1.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]).unwrap();
    assert!(near(laid.place([0.0, 0.0, 2.0]), [3.0, 1.0, 1.0]));
    let longer = laid.as_long_as(3.0);
    assert!(near(longer.place([0.0; 3]), [1.0, 1.0, 1.0]), "its joint stays");
    assert!(near(longer.place([0.0, 0.0, 2.0]), [4.0, 1.0, 1.0]), "its end is three away");
    assert!(near(longer.place([0.0, 0.5, 1.0]), [2.5, 1.5, 1.0]), "half way along, and as far to the side as it was");
    // The piece after the last joint is carried at its own length.
    let carried = longer.carried_to([0.0, 0.0, 2.0], [4.0, 1.0, 1.0]);
    assert!(near(carried.place([0.0, 0.0, 2.5]), [4.5, 1.0, 1.0]));
}

/// The arms for first person keep their own lengths, their hands are
/// on the install's knuckle on knuckle, **every joint of every finger
/// is on the install's own**, and each arm goes back from its hand
/// lying as the install's lies: whatever the install's shoulder does,
/// the forearm is turned as the install's forearm and held at the
/// wrist.
#[test]
fn arms_alone_go_back_from_the_hand() {
    let (_, skeleton) = made(&[]);
    let model = arms_alone();
    // At the file's own size first.
    let (meshes, rig, size) = fit_arms(&model, &skeleton, Some(1.0)).unwrap();
    assert_eq!(size, 1.0);
    let mesh = &meshes[0];
    let theirs: HashMap<String, V3> = their_stick_figure().into_iter().collect();
    let vertex = |name: &str| mesh.positions[model.bones.iter().position(|bone| bone.name == name).unwrap() - 1];
    let was = |name: &str| model.bones.iter().find(|bone| bone.name == name).unwrap().at;
    // Knuckle on knuckle, and the upper arm as long as it was.
    assert!(near(vertex("bip01_l_finger2"), theirs["LDigit31"]) && near(vertex("bip01_r_finger2"), theirs["RDigit31"]), "{:?}", vertex("bip01_l_finger2"));
    assert!((apart(was("l_upperarm"), was("l_forearm")) - apart(vertex("l_upperarm"), vertex("l_forearm"))).abs() < 1e-4);
    // **The hand is laid by its whole row of knuckles**: the middle
    // of the line from its first finger's knuckle to its little
    // finger's is on the middle of the install's, and what is the
    // hand's own is as far from there as it was (it is not stretched).
    let middle = |first: V3, little: V3| scaled(add(first, little), 0.5);
    let (row, their_row) = (middle(was("bip01_l_finger1"), was("bip01_l_finger4")), middle(theirs["LDigit21"], theirs["LDigit51"]));
    assert!((apart(vertex("l_hand"), their_row) - apart(was("l_hand"), row)).abs() < 1e-4, "the hand's own joint is {} from the row's middle, and was {}", apart(vertex("l_hand"), their_row), apart(was("l_hand"), row));
    // **And it bends at the install's wrist**, wherever the model has
    // the joint: the rig's wrist is the install's, and the forearm
    // ends there.
    let wrist_on_rig = rig.bones[bone(&rig, "LPalm")].joint;
    assert!(near(wrist_on_rig, theirs["LPalm"]), "the wrist is at {wrist_on_rig:?}");
    // The fingers' joints are the install's, the thumb's too, on the
    // mesh and on the rig.
    for (name, their_name) in [("l_finger0", "LDigit11"), ("l_finger01", "LDigit12"), ("l_finger02", "LDigit13"), ("bip01_l_finger1", "LDigit21"), ("bip01_l_finger11", "LDigit22"), ("bip01_l_finger41", "LDigit52"), ("bip01_r_finger42", "RDigit53"), ("r_finger01", "RDigit12")] {
        assert!(near(vertex(name), theirs[their_name]), "{name} is at {:?} for {:?}", vertex(name), theirs[their_name]);
        let on_rig = &rig.bones[bone(&rig, their_name)];
        assert!(near(on_rig.joint, on_rig.their_joint), "{their_name} on the rig");
    }
    // The forearm and the upper arm lie as the install's.
    let lies = |from: V3, to: V3| unit(sub(to, from)).unwrap();
    assert!(near(lies(vertex("l_forearm"), wrist_on_rig), lies(theirs["LForearm1"], theirs["LPalm"])));
    assert!(near(lies(vertex("l_upperarm"), vertex("l_forearm")), lies(theirs["LUpperarm"], theirs["LForearm1"])));
    // Five fingers, each with the install's own; every triangle the
    // arms'; shares that add up to one.
    for (name, their_name) in [("l_finger02", "LDigit13"), ("bip01_l_finger1", "LDigit21"), ("bip01_l_finger31", "LDigit42"), ("bip01_r_finger42", "RDigit53"), ("l_forearm_twist", "LForearm3")] {
        let at = model.bones.iter().position(|bone| bone.name == name).unwrap() - 1;
        assert_eq!(mesh.joints[at][0] as usize, bone(&rig, their_name), "{name}");
    }
    assert_eq!(mesh.submeshes.iter().map(Vec::len).collect::<Vec<_>>(), [0, 3, 0]);
    assert!(mesh.weights.iter().all(|shares| (shares.iter().sum::<f32>() - 1.0).abs() < 1e-5));
    // At rest nothing moves. With the install's hand moved and its
    // forearm turned, the hand is the install's and the forearm is
    // turned as the install's, its wrist still on the hand.
    let still = rig.pose(&vec![Move::STILL; rig.bones.len()]);
    assert!(still.iter().all(|moved| *moved == Move::STILL));
    let [palm, fore, upper] = ["LPalm", "LForearm1", "LUpperarm"].map(|name| bone(&rig, name));
    let mut moved = vec![Move::STILL; rig.bones.len()];
    moved[palm].shift = [0.02, -0.05, 0.03];
    let tipped = arc([0.0, -1.0, 0.0], [0.0, -1.0, 0.3]);
    moved[fore] = Move::about(tipped, rig.bones[fore].their_joint, rig.bones[fore].their_joint);
    let posed = rig.pose(&moved);
    let wrist = rig.bones[palm].joint;
    assert_eq!(posed[palm], moved[palm]);
    assert_eq!(posed[fore].axes, tipped);
    assert!(near(posed[fore].of(wrist), moved[palm].of(wrist)), "the forearm's end is on the hand");
    assert!(near(posed[upper].of(rig.bones[fore].joint), posed[fore].of(rig.bones[fore].joint)), "the upper arm's end is on the elbow");
}

/// Left to itself, the arms' model is made the size at which its hand
/// is as broad as the install's: the first finger's knuckle and the
/// little finger's as far apart, both knuckles on the install's when
/// the two hands lie alike, and every bone longer by as much.
#[test]
fn arms_alone_are_made_the_size_of_the_install_s_hand() {
    let (_, skeleton) = made(&[]);
    let model = arms_alone();
    let theirs: HashMap<String, V3> = their_stick_figure().into_iter().collect();
    let was = |name: &str| model.bones.iter().find(|bone| bone.name == name).unwrap().at;
    let (meshes, rig, size) = fit_arms(&model, &skeleton, None).unwrap();
    let (broad, their_broad) = (apart(was("bip01_l_finger1"), was("bip01_l_finger4")), apart(theirs["LDigit21"], theirs["LDigit51"]));
    assert!((size - their_broad / broad).abs() < 1e-5 && size > 1.05, "{size} for {} over {}", their_broad, broad);
    let vertex = |name: &str| meshes[0].positions[model.bones.iter().position(|bone| bone.name == name).unwrap() - 1];
    assert!((apart(vertex("bip01_l_finger1"), vertex("bip01_l_finger4")) - their_broad).abs() < 1e-4);
    assert!((apart(was("l_upperarm"), was("l_forearm")) * size - apart(vertex("l_upperarm"), vertex("l_forearm"))).abs() < 1e-4);
    // The hand's own is larger by as much about the middle of its
    // row of knuckles, which is on the middle of the install's.
    let middle = |first: V3, little: V3| scaled(add(first, little), 0.5);
    let (row, their_row) = (middle(was("bip01_r_finger1"), was("bip01_r_finger4")), middle(theirs["RDigit21"], theirs["RDigit51"]));
    assert!((apart(vertex("r_hand"), their_row) - apart(was("r_hand"), row) * size).abs() < 1e-4);
    assert!(near(vertex("bip01_l_finger2"), theirs["LDigit31"]) && near(vertex("l_finger01"), theirs["LDigit12"]));
    assert!(rig.pose(&vec![Move::STILL; rig.bones.len()]).iter().all(|moved| *moved == Move::STILL));
}

/// A point of a hand, given across it, behind its knuckles' line and
/// through it, as the install's joints have the hand.
fn in_hand(skeleton: &[RestBone], side: &str, parts: V3) -> V3 {
    let frame = HandFrame::of(skeleton, side).unwrap();
    add(add(add(frame.centre, scaled(frame.across, parts[0])), scaled(frame.along, -parts[1])), scaled(frame.through, parts[2]))
}

/// A palm's box is taken from edge to edge, both sides of the hand,
/// in the slices behind the knuckles: of what goes with the palm's
/// own bone, each edge the middle one of the slices'.
#[test]
fn a_palm_s_box_is_measured_from_edge_to_edge() {
    let (_, skeleton) = made(&[]);
    let mut skin = Skin { bones: vec!["LPalm".into(), "LDigit11".into(), "RPalm".into()], points: Vec::new() };
    for (slice, back) in PALM_SLICES.into_iter().enumerate() {
        // One slice has less of the hand in it (a mesh with few
        // points there): the middle one of the five is not it.
        let short = if slice == 1 { 0.02 } else { 0.0 };
        skin.points.push((in_hand(&skeleton, "L", [-0.05 + short, back, -0.03]), 0));
        skin.points.push((in_hand(&skeleton, "L", [0.04, back, 0.02]), 0));
        // The thumb reaches further, and is not the palm's.
        skin.points.push((in_hand(&skeleton, "L", [-0.09, back, -0.05]), 1));
    }
    // Ahead of the knuckles, and behind the last slice: neither counts.
    skin.points.push((in_hand(&skeleton, "L", [0.1, -0.01, 0.0]), 0));
    skin.points.push((in_hand(&skeleton, "L", [0.1, 0.08, 0.0]), 0));
    let found = palm_box(&skin, &skeleton, "L").unwrap().unwrap();
    for (found, wanted) in found.iter().flatten().zip([-0.05, 0.04, -0.03, 0.02]) {
        assert!((found - wanted).abs() < 1e-5, "{found} for {wanted}");
    }
    // A skin with no palm on a side has no box there.
    assert!(palm_box(&skin, &skeleton, "R").unwrap().is_none());
}

/// A bone hung from another stays on it, and turns as the install's
/// own turns: a head turned a quarter to one side, a spine bent.
#[test]
fn a_hung_bone_stays_where_it_hangs() {
    let (model, skeleton) = made(&[]);
    let (_, rig) = fit(&model, &skeleton, None).unwrap();
    let [spine, neck, head, collar, upper] = ["Spine2", "Neck", "Head", "LCollarbone", "LUpperarm"].map(|name| bone(&rig, name));
    let mut theirs = vec![Move::STILL; rig.bones.len()];
    // The head a quarter turn about what is up for the install (z),
    // and the second spine bone tipped, each about its own joint as
    // the install's skeleton has it.
    let quarter = [[0.0, 1.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 0.0, 1.0]];
    theirs[head] = Move::about(quarter, rig.bones[head].their_joint, rig.bones[head].their_joint);
    let tipped = arc([0.0, 0.0, 1.0], [0.0, -0.3, 1.0]);
    for above in [spine, bone(&rig, "Spine3"), neck, collar, upper] {
        theirs[above] = Move::about(tipped, rig.bones[spine].their_joint, rig.bones[spine].their_joint);
    }
    let posed = rig.pose(&theirs);
    let joint = |index: usize| rig.bones[index].joint;
    assert!(near(posed[head].of(joint(head)), posed[neck].of(joint(head))), "the head is on the neck");
    assert_eq!(posed[head].axes, quarter);
    assert!(near(posed[collar].of(joint(collar)), posed[bone(&rig, "Spine3")].of(joint(collar))));
    // The shoulder went with the chest; the arm still hangs there.
    assert!(apart(posed[collar].of(joint(upper)), joint(upper)) > 0.05);
    assert!(near(posed[upper].of(joint(upper)), posed[collar].of(joint(upper))));
    // And the spine bone itself turned about the outfit's joint for
    // it, not the install's.
    assert!(near(posed[spine].of(joint(spine)), posed[bone(&rig, "Spine1")].of(joint(spine))));
}
