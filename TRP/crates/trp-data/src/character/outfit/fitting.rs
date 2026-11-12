//! Fitting an outfit to the install's body.

use super::*;

/// Where one of the model's bones sends what goes with it: laid so, to a
/// bone of the rig or to several.
#[derive(Clone, Copy)]
pub(super) enum Goes {
    /// With the trunk, to whichever of the rig's spine bones are at its
    /// height.
    Trunk,
    /// To one bone.
    To(Laid, usize),
    /// Shared among several along it (a forearm in three, a thigh in two,
    /// for the turn along them).
    Along(Laid, [Option<usize>; 3]),
}

/// The install's bone whose path ends with a name.
pub(super) fn theirs(skeleton: &[RestBone], name: &str) -> Result<(usize, V3)> {
    let found = skeleton.iter().position(|bone| bone.path == name || bone.path.ends_with(&format!("/{name}")));
    let index = found.ok_or_else(|| anyhow!("the character's own clothing has no bone {name}"))?;
    Ok((index, rest_place(&skeleton[index].bind).ok_or_else(|| anyhow!("the bone {name} has no place at rest"))?))
}

/// Fits a model to the install's skeleton: the rig it is drawn on, which
/// has the install's bones with the model's own joints, and each of its
/// meshes at rest about that rig.
///
/// The model is brought into the pose the install's clothing is made in
/// (arms forward, palms half turned) and keeps its own build: its trunk
/// and its head stand as they are, each limb's bone is turned at its own
/// joint, and nothing is stretched.
///
/// **What the character's own eyes see of it is what lies under the
/// install's waist.** The install draws the character's own body, for its
/// own eyes, as the mesh of the legs and no more (the upper body is a
/// shadow there), and that mesh ends at the waist. `waist` is the box
/// about that mesh at rest; its top is so far over the install's hips,
/// and the model's waist is as far over its own. The model's triangles
/// wholly at or under that are the legs' list, the arms' are the arms',
/// and everything else is the upper body's, a jacket's front with it,
/// which the model's own bones would give to the hips. Without a box the
/// lists go by the bones.
pub fn fit(model: &xray::Model, skeleton: &[RestBone], waist: Option<[V3; 2]>) -> Result<(Vec<MeshData>, Rig)> {
    let ours = |name: &str| -> Result<V3> { model.bones.iter().find(|bone| bone.name == name).map(|bone| bone.at).ok_or_else(|| anyhow!("the model has no bone {name}")) };
    let their = |name: &str| theirs(skeleton, &format!("Base Human{name}"));
    let missing = || anyhow!("two joints of a skeleton are at one place");

    // The two skeletons' own directions, each from its hips and its head.
    let (hips, their_hips) = ([ours("bip01_l_thigh")?, ours("bip01_r_thigh")?], [their("LThigh1")?.1, their("RThigh1")?.1]);
    let (head, their_head) = (ours("bip01_head")?, their("Head")?.1);
    let middle = |pair: [V3; 2]| scaled(add(pair[0], pair[1]), 0.5);
    let basis = Basis::of(hips[0], hips[1], middle(hips), head).ok_or_else(missing)?;
    let their_basis = Basis::of(their_hips[0], their_hips[1], middle(their_hips), their_head).ok_or_else(missing)?;

    // The ground under each: the model's lowest point, and the level the
    // install's meshes stand on, which is their space's origin
    // (PROVISIONAL: its own feet's joints are 2 and 6 cm over it).
    let ground = model.parts.iter().flat_map(|part| &part.positions).map(|p| dot(*p, basis.up)).fold(f32::MAX, f32::min);
    let their_ground = 0.0;

    // A thing that stands as it is in the model, about one point.
    let upright = |from: V3, to: V3| Laid { from, to, ours: [basis.right, basis.up, basis.ahead], theirs: [their_basis.right, their_basis.up, their_basis.ahead], long: 1.0, twist: 0.0, stretch: 1.0 };
    // The whole of the model so: on the install's ground at its own
    // height, the middle of its hips over the middle of the install's.
    let over = their_basis.parts(middle(their_hips));
    let stand = upright(middle(hips), their_basis.whole([over[0], their_ground + dot(middle(hips), basis.up) - ground, over[2]]));

    let mut rig = Rig::default();
    let put = |rig: &mut Rig, name: &str, joint: V3, follows: Follows| add_bone(rig, skeleton, name, joint, follows);
    let mut goes: HashMap<String, Goes> = HashMap::new();

    // The trunk: the hips as the install's, and the install's spine bones
    // hung one from another along the model's own back, each as far up
    // from the hips to the neck as the install's is.
    let (pelvis_at, neck_at) = (stand.place(ours("bip01_pelvis")?), stand.place(ours("bip01_neck")?));
    let their_height = |name: &str| their(name).map(|(_, at)| dot(at, their_basis.up));
    let (their_pelvis, their_neck) = (their_height("Pelvis")?, their_height("Neck")?);
    let mut spine = vec![put(&mut rig, "Pelvis", pelvis_at, Follows::Theirs)?];
    for name in ["Spine1", "Spine2", "Spine3"] {
        let up = ((their_height(name)? - their_pelvis) / (their_neck - their_pelvis).max(1e-6)).clamp(0.0, 1.0);
        let from = *spine.last().unwrap();
        spine.push(put(&mut rig, name, add(pelvis_at, scaled(sub(neck_at, pelvis_at), up)), Follows::Hung { from })?);
    }
    let chest = *spine.last().unwrap();
    spine.push(put(&mut rig, "Neck", neck_at, Follows::Hung { from: chest })?);
    let head_bone = put(&mut rig, "Head", stand.place(head), Follows::Hung { from: spine[4] })?;
    for name in ["root_stalker", "bip01", "bip01_pelvis", "bip01_spine", "bip01_spine1", "bip01_spine2", "bip01_neck", "bip01_tail"] {
        goes.insert(name.to_owned(), Goes::Trunk);
    }
    for name in ["bip01_head", "eye_left", "eye_right", "eyelid_1", "jaw_1"] {
        goes.insert(name.to_owned(), Goes::To(stand, head_bone));
    }

    for (side, their_side) in [("l", "L"), ("r", "R")] {
        let whole_ours = &ours;
        let ours = |name: &str| ours(&format!("bip01_{side}_{name}"));
        let their = |name: &str| their(&format!("{their_side}{name}")).map(|(_, at)| at);
        let named = |name: &str| format!("{their_side}{name}");

        // The collarbone and the shoulder's joint stand with the trunk.
        let collar = put(&mut rig, &named("Collarbone"), stand.place(ours("clavicle")?), Follows::Hung { from: chest })?;
        goes.insert(format!("bip01_{side}_clavicle"), Goes::To(stand, collar));

        // The arm hangs at the model's own shoulder. The model has a
        // thumb, a first finger and one bone for the other three, which
        // goes with the install's middle finger.
        let finger = |number: u8, joints: [&str; 3]| (joints.map(|joint| format!("bip01_{side}_{joint}")), number);
        let names = ArmNames {
            upper: format!("bip01_{side}_upperarm"),
            fore: vec![format!("bip01_{side}_forearm")],
            hand: format!("bip01_{side}_hand"),
            fingers: vec![finger(1, ["finger0", "finger01", "finger02"]), finger(2, ["finger1", "finger11", "finger12"]), finger(3, ["finger2", "finger21", "finger22"])],
            by: 2,
            beside: 1,
        };
        lay_arm(whole_ours, skeleton, &mut rig, &mut goes, their_side, &names, Hang::From { at: stand.place(ours("upperarm")?), collar })?;
        let mut send = |name: &str, how: Goes| goes.insert(format!("bip01_{side}_{name}"), how);

        // The leg hangs at the model's own hip and its foot is the
        // install's (it is on the ground): the knee is where the model's
        // own two lengths put it. The foot stands as it is, its sole on
        // the install's ground and its ankle over the install's (the two
        // skeletons have the ankle's joint at other heights over the
        // sole), and the calf ends at that ankle.
        let (hip, knee, ankle) = (ours("thigh")?, ours("calf")?, ours("foot")?);
        let (their_hip, their_knee, their_ankle) = (their("Thigh1")?, their("Calf")?, their("Foot")?);
        let over_sole = dot(ankle, basis.up) - ground;
        let ankle_at = add(their_ankle, scaled(their_basis.up, over_sole - (dot(their_ankle, their_basis.up) - their_ground)));
        let (hip_at, thigh_long, calf_long) = (stand.place(hip), length(sub(knee, hip)), length(sub(ankle, knee)));
        let knee_at = bend(hip_at, ankle_at, thigh_long, calf_long, off_line(their_hip, their_knee, their_ankle), their_basis.ahead);
        let calf_along = unit(sub(ankle_at, knee_at)).ok_or_else(missing)?;
        let hinge = cross(sub(knee, hip), sub(ankle, knee));
        let hinge_at = cross(sub(knee_at, hip_at), calf_along);
        let thigh = Laid::bone(hip, knee, hinge, hip_at, sub(knee_at, hip_at), hinge_at).ok_or_else(missing)?;
        let calf = Laid::bone(knee, ankle, hinge, knee_at, calf_along, hinge_at).ok_or_else(missing)?;
        let foot = upright(ankle, ankle_at);
        let thigh_bone = rig.bones.len();
        put(&mut rig, &named("Thigh1"), hip_at, Follows::Limb { from: spine[0], lower: thigh_bone + 2, end: thigh_bone + 3 })?;
        put(&mut rig, &named("Thigh2"), scaled(add(hip_at, knee_at), 0.5), Follows::Along { of: thigh_bone })?;
        put(&mut rig, &named("Calf"), knee_at, Follows::Lower)?;
        let foot_bone = put(&mut rig, &named("Foot"), ankle_at, Follows::Theirs)?;
        let toe_bone = put(&mut rig, &named("Toe"), foot.place(ours("toe0")?), Follows::Hung { from: foot_bone })?;
        send("thigh", Goes::Along(thigh, [Some(thigh_bone), Some(thigh_bone + 1), None]));
        send("calf", Goes::To(calf, thigh_bone + 2));
        send("foot", Goes::To(foot, foot_bone));
        send("toe0", Goes::To(foot, toe_bone));
    }
    // A bone the list has not goes as the nearest above it that the list
    // has. And which of the three parts a bone's share counts for.
    let listed = |index: usize| -> Option<&str> {
        let mut at = Some(index);
        while let Some(bone) = at {
            if goes.contains_key(&model.bones[bone].name) {
                return Some(model.bones[bone].name.as_str());
            }
            at = model.bones[bone].parent;
        }
        None
    };
    let how: Vec<Option<Goes>> = (0..model.bones.len()).map(|index| Some(listed(index).map_or(Goes::Trunk, |name| goes[name]))).collect();
    let zone: Vec<usize> = (0..model.bones.len())
        .map(|index| match listed(index) {
            Some(name) if ["upperarm", "forearm", "hand", "finger"].iter().any(|part| name.contains(part)) => ARMS,
            Some(name) if ["pelvis", "thigh", "calf", "foot", "toe", "tail"].iter().any(|part| name.contains(part)) => LEGS,
            _ => UPPER,
        })
        .collect();

    // What goes with the trunk is shared among the rig's spine bones by
    // its own height: wholly a bone's at the middle of that bone (between
    // its joint and the next), and evenly between two middles.
    let heights: Vec<f32> = spine.iter().map(|bone| dot(rig.bones[*bone].joint, their_basis.up)).chain([dot(rig.bones[head_bone].joint, their_basis.up)]).collect();
    let middles: Vec<f32> = heights.windows(2).map(|pair| (pair[0] + pair[1]) * 0.5).collect();
    let by_height = |height: f32| -> [(usize, f32); 2] {
        let above = middles.iter().position(|middle| height < *middle).unwrap_or(middles.len());
        match above {
            0 => [(spine[0], 1.0), (spine[0], 0.0)],
            n if n == middles.len() => [(spine[n - 1], 1.0), (spine[n - 1], 0.0)],
            n => {
                let upper = (height - middles[n - 1]) / (middles[n] - middles[n - 1]).max(1e-6);
                [(spine[n - 1], 1.0 - upper), (spine[n], upper)]
            }
        }
    };

    let trunk = |p: V3, n: V3| {
        let at = stand.place(p);
        (at, stand.normal(p, n), by_height(dot(at, their_basis.up)))
    };
    // The top of the install's legs (the higher of its box's two corners)
    // is so far over the install's hips; the model's waist is as far over
    // its own, which stand at its own height. The hips are the two
    // thighs' joints: the two skeletons have the bone they call the
    // pelvis at other heights in the body.
    let under = waist.map(|corners| {
        let top = dot(corners[0], their_basis.up).max(dot(corners[1], their_basis.up));
        (their_basis.up, dot(stand.place(middle(hips)), their_basis.up) + top - dot(middle(their_hips), their_basis.up))
    });
    let meshes = lay_meshes(model, &how, &zone, rig.bones.len(), &trunk, under)?;
    Ok((meshes, rig))
}
