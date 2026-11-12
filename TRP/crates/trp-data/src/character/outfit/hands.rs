//! Skins, the hand's frame, the palm's box and laying both arms.

use super::*;

/// A skin at rest, to measure a hand by: each of its points with the bone
/// it mostly goes with, and the bones' names on the install's skeleton
/// (what follows `Base Human` in a bone's path: `LPalm`, `RDigit21`).
#[derive(Debug, Clone, Default)]
pub struct Skin {
    pub(super) bones: Vec<String>,
    pub(super) points: Vec<(V3, u16)>,
}

impl Skin {
    /// Of meshes that hang on the same bones, the bones given by their
    /// paths.
    pub fn of<'a>(meshes: impl IntoIterator<Item = &'a MeshData>, paths: &[String]) -> Self {
        let bones = paths.iter().map(|path| path.rsplit("Base Human").next().unwrap_or_default().to_owned()).collect();
        let mut points = Vec::new();
        for mesh in meshes {
            for (p, (joints, shares)) in mesh.positions.iter().zip(mesh.joints.iter().zip(&mesh.weights)) {
                let most = (0..4).max_by(|a, b| shares[*a].total_cmp(&shares[*b])).unwrap_or(0);
                points.push((*p, joints[most]));
            }
        }
        Self { bones, points }
    }
}

/// A hand's own directions at rest, by the install's joints: across it
/// from the first finger's knuckle to the little finger's, along it from
/// the wrist towards the knuckles, and through it; all counted from the
/// middle of the knuckles' line.
#[derive(Clone, Copy)]
pub(super) struct HandFrame {
    pub(super) centre: V3,
    pub(super) across: V3,
    pub(super) along: V3,
    pub(super) through: V3,
}

impl HandFrame {
    pub(super) fn of(skeleton: &[RestBone], side: &str) -> Result<Self> {
        let their = |name: &str| theirs(skeleton, &format!("Base Human{side}{name}")).map(|(_, at)| at);
        let missing = || anyhow!("two joints of a hand are at one place");
        let (first, little, middle, wrist) = (their("Digit21")?, their("Digit51")?, their("Digit31")?, their("Palm")?);
        let across = unit(sub(little, first)).ok_or_else(missing)?;
        let along = sub(middle, wrist);
        let along = unit(sub(along, scaled(across, dot(along, across)))).ok_or_else(missing)?;
        Ok(Self { centre: scaled(add(first, little), 0.5), across, along, through: cross(across, along) })
    }

    /// Where a point is in the hand: how far across it, how far behind
    /// the knuckles' line, and how far through it.
    pub(super) fn parts(&self, p: V3) -> V3 {
        let from = sub(p, self.centre);
        [dot(from, self.across), -dot(from, self.along), dot(from, self.through)]
    }
}

/// Where a palm is measured: slices across the hand a centimetre thick,
/// so far behind the knuckles' line (metres).
pub(super) const PALM_SLICES: [f32; 5] = [0.025, 0.035, 0.045, 0.055, 0.065];

/// The box a palm's skin fills behind its knuckles, **to measure a fitted
/// hand beside the install's own** (nothing is fitted by it): from where
/// to where it reaches across the hand, and from where to where through
/// it, in metres from the middle of the knuckles' line as the install's
/// joints have it (`side` is `L` or `R`). Each of the four edges is the
/// middle one of those found in the slices (`PALM_SLICES`), and what is
/// measured is what goes mostly with the palm's own bone: neither the
/// thumb nor the fingers count, and both edges of the hand do. None where
/// the skin has no palm there.
///
/// A hand's breadth wants both its edges. One edge alone says how far
/// that edge is from wherever the two hands were laid together, and two
/// models do not have their joints at the same places inside the hand.
pub fn palm_box(skin: &Skin, skeleton: &[RestBone], side: &str) -> Result<Option<[[f32; 2]; 2]>> {
    let frame = HandFrame::of(skeleton, side)?;
    let palm = format!("{side}Palm");
    let Some(bone) = skin.bones.iter().position(|name| *name == palm) else { return Ok(None) };
    let mut edges: [Vec<f32>; 4] = Default::default();
    for back in PALM_SLICES {
        let mut found: Option<[f32; 4]> = None;
        for (p, _) in skin.points.iter().filter(|(_, of)| *of as usize == bone) {
            let [a, b, t] = frame.parts(*p);
            if (b - back).abs() < 0.005 {
                found = Some(found.map_or([a, a, t, t], |[a0, a1, t0, t1]| [a0.min(a), a1.max(a), t0.min(t), t1.max(t)]));
            }
        }
        if let Some(found) = found {
            for (edge, value) in edges.iter_mut().zip(found) {
                edge.push(value);
            }
        }
    }
    if edges[0].len() < 3 {
        return Ok(None);
    }
    let [a0, a1, t0, t1] = edges.map(|mut found| {
        found.sort_by(f32::total_cmp);
        found[found.len() / 2]
    });
    Ok(Some([[a0, a1], [t0, t1]]))
}

/// The arms' model laid on the install's arms at a size (`fit_arms`).
pub(super) fn lay_arms(model: &xray::Model, skeleton: &[RestBone], size: f32) -> Result<(Vec<MeshData>, Rig)> {
    let name_of = |name: &str| -> Result<String> {
        let found = [name.to_owned(), format!("bip01_{name}")].into_iter().find(|wanted| model.bones.iter().any(|bone| &bone.name == wanted));
        found.ok_or_else(|| anyhow!("the arms' model has no bone {name}"))
    };
    let sized = xray::Model {
        bones: model.bones.iter().map(|bone| xray::Bone { at: scaled(bone.at, size), ..bone.clone() }).collect(),
        parts: model.parts.iter().map(|part| xray::Part { positions: part.positions.iter().map(|p| scaled(*p, size)).collect(), ..part.clone() }).collect(),
    };
    let model = &sized;
    let ours = |name: &str| -> Result<V3> { model.bones.iter().find(|bone| bone.name == name).map(|bone| bone.at).ok_or_else(|| anyhow!("the arms' model has no bone {name}")) };
    let mut rig = Rig::default();
    let mut goes: HashMap<String, Goes> = HashMap::new();
    for (side, their_side) in [("l", "L"), ("r", "R")] {
        let finger = |number: u8, joints: [&str; 3]| -> Result<([String; 3], u8)> { Ok(([name_of(&format!("{side}_{}", joints[0]))?, name_of(&format!("{side}_{}", joints[1]))?, name_of(&format!("{side}_{}", joints[2]))?], number)) };
        let names = ArmNames {
            upper: name_of(&format!("{side}_upperarm"))?,
            fore: [format!("{side}_forearm"), format!("{side}_forearm_twist")].iter().filter_map(|name| name_of(name).ok()).collect(),
            hand: name_of(&format!("{side}_hand"))?,
            fingers: vec![
                finger(1, ["finger0", "finger01", "finger02"])?,
                finger(2, ["finger1", "finger11", "finger12"])?,
                finger(3, ["finger2", "finger21", "finger22"])?,
                finger(4, ["finger3", "finger31", "finger32"])?,
                finger(5, ["finger4", "finger41", "finger42"])?,
            ],
            by: 2,
            beside: 1,
        };
        ensure!(!names.fore.is_empty(), "the arms' model has no bone {side}_forearm");
        lay_arm(&ours, skeleton, &mut rig, &mut goes, their_side, &names, Hang::Hand)?;
    }
    let how: Vec<Option<Goes>> = model.bones.iter().map(|bone| goes.get(&bone.name).copied()).collect();
    let zone = vec![ARMS; model.bones.len()];
    let no_trunk = |p: V3, n: V3| (p, n, [(0, 0.0), (0, 0.0)]);
    let meshes = lay_meshes(model, &how, &zone, rig.bones.len(), &no_trunk, None)?;
    Ok((meshes, rig))
}
