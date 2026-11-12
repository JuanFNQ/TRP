//! Laying meshes on the laid bones and fitting the arms.

use super::*;

/// The meshes of a model, each vertex where the bones it goes with lay
/// it (each bone's laying of it weighed by that bone's share) and with
/// the rig's bones those send it to: the four with the largest shares,
/// made to add up to one. A mesh's triangles are in three lists, by the
/// part most of a triangle's shares count for.
///
/// `trunk` lays what goes with the trunk: for a place and a normal, the
/// same laid, and the two spine bones at that height with the share of
/// each.
///
/// With `under` (a direction up and a height along it), the legs' list is
/// made by where a triangle lies and not by its bones: whatever is not
/// the arms' and has all three corners at or under that height is the
/// legs', and the rest the upper body's.
pub(super) fn lay_meshes(model: &xray::Model, how: &[Option<Goes>], zone: &[usize], bones: usize, trunk: &dyn Fn(V3, V3) -> (V3, V3, [(usize, f32); 2]), under: Option<(V3, f32)>) -> Result<Vec<MeshData>> {
    const NO_MOVE: [f32; 16] = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0];
    let mut fitted = Vec::with_capacity(model.parts.len());
    for part in &model.parts {
        let count = part.positions.len();
        let mut mesh = MeshData {
            name: part.texture.clone(),
            positions: Vec::with_capacity(count),
            normals: Vec::with_capacity(count),
            tangents: Vec::new(),
            uv0: part.uv.clone(),
            weights: Vec::with_capacity(count),
            joints: Vec::with_capacity(count),
            submeshes: vec![Vec::new(); 3],
            bind_poses: vec![NO_MOVE; bones],
            ..MeshData::default()
        };
        for ((p, n), links) in part.positions.iter().zip(&part.normals).zip(&part.links) {
            let (mut place, mut normal) = ([0.0f32; 3], [0.0f32; 3]);
            let mut shares: Vec<(usize, f32)> = Vec::with_capacity(8);
            let mut share = |bone: usize, by: f32| match shares.iter_mut().find(|(known, _)| *known == bone) {
                Some((_, sum)) => *sum += by,
                None => shares.push((bone, by)),
            };
            for (bone, by) in links.iter().filter(|(_, by)| *by > 0.0) {
                let (at, turned) = match how[*bone as usize].ok_or_else(|| anyhow!("a vertex goes with the bone {}, which has no place here", model.bones[*bone as usize].name))? {
                    Goes::Trunk => {
                        let (at, turned, spine) = trunk(*p, *n);
                        for (spine_bone, of_it) in spine {
                            share(spine_bone, by * of_it);
                        }
                        (at, turned)
                    }
                    Goes::To(laid, rig_bone) => {
                        share(rig_bone, *by);
                        (laid.place(*p), laid.normal(*p, *n))
                    }
                    Goes::Along(laid, bones) => {
                        // Wholly a bone's at the middle of its own part
                        // of the length, evenly between two middles.
                        let along: Vec<usize> = bones.iter().flatten().copied().collect();
                        let at = laid.share_along(*p) * along.len() as f32 - 0.5;
                        let lower = (at.floor().max(0.0) as usize).min(along.len() - 1);
                        let upper = (lower + 1).min(along.len() - 1);
                        let further = (at - lower as f32).clamp(0.0, 1.0);
                        share(along[lower], by * (1.0 - further));
                        share(along[upper], by * further);
                        (laid.place(*p), laid.normal(*p, *n))
                    }
                };
                place = add(place, scaled(at, *by));
                normal = add(normal, scaled(unit(turned).unwrap_or(turned), *by));
            }
            shares.sort_by(|a, b| b.1.total_cmp(&a.1));
            shares.truncate(4);
            let sum: f32 = shares.iter().map(|(_, by)| by).sum();
            ensure!(sum > 0.0, "a vertex of the model goes with no bone");
            let (mut joints, mut weights) = ([0u16; 4], [0.0f32; 4]);
            for (slot, (bone, by)) in shares.iter().enumerate() {
                (joints[slot], weights[slot]) = (*bone as u16, by / sum);
            }
            mesh.positions.push(place);
            mesh.normals.push(unit(normal).unwrap_or([0.0, 1.0, 0.0]));
            mesh.joints.push(joints);
            mesh.weights.push(weights);
        }
        for triangle in part.triangles.chunks_exact(3) {
            let mut by_zone = [0.0f32; 3];
            for corner in triangle {
                for (bone, by) in &part.links[*corner as usize] {
                    by_zone[zone[*bone as usize]] += by;
                }
            }
            let most = (0..3).max_by(|a, b| by_zone[*a].total_cmp(&by_zone[*b])).unwrap_or(UPPER);
            let list = match under {
                Some((up, height)) if most != ARMS => {
                    let below = triangle.iter().all(|corner| dot(mesh.positions[*corner as usize], up) <= height);
                    if below { LEGS } else { UPPER }
                }
                _ => most,
            };
            mesh.submeshes[list].extend_from_slice(triangle);
        }
        fitted.push(mesh);
    }
    Ok(fitted)
}

/// Fits a model of arms alone, the other game's for first person, to the
/// install's arms: a rig of two arms, each going back from the install's
/// own hand (`Hang::Hand`), and the model's meshes at rest about it.
/// Every triangle is the arms'.
///
/// Such a model names its bones `l_upperarm`, `l_forearm`, `l_hand`, a
/// thumb `l_finger0` and four fingers `l_finger1` to `4`, with or without
/// `bip01_` before them, and may have a bone more for the forearm's turn
/// (`l_forearm_twist`).
///
/// **The model is not drawn at its file's size.** The other game draws
/// the player's own arms in a space of their own with a field of view of
/// their own, so the file's size is no size in the world; and every hold
/// of the install's weapons is made for the install's hand. The whole
/// model, arms and hands alike, is made the size at which **its hand's
/// skeleton is the install's hand's**: the knuckles' joints as far apart,
/// the first finger's from the little finger's (1.27 times the file, for
/// the one skeleton measured). The fingers' joints are the install's at
/// any size (`lay_arm`), and their lengths with them; the size settles
/// how broad and how thick the palm, the fingers, the wrist and the arm
/// are, and at the joints' size they are in the model's own proportion to
/// fingers as long as the install's. `size` says another size outright,
/// in times the file's. The size used is the third value.
///
/// (A size taken off the skin on one side of the hand alone, out to the
/// little finger's edge, was tried and was wrong, 1.12: this model's palm
/// lies most of a centimetre to that side of the install's, and with
/// fingers as long as the install's its palm and its wrist were then a
/// ninth too small for them. `palm_box` measures a palm from edge to
/// edge.)
pub fn fit_arms(model: &xray::Model, skeleton: &[RestBone], size: Option<f32>) -> Result<(Vec<MeshData>, Rig, f32)> {
    // A bone by its name, with or without the prefix.
    let name_of = |name: &str| -> Result<String> {
        let found = [name.to_owned(), format!("bip01_{name}")].into_iter().find(|wanted| model.bones.iter().any(|bone| &bone.name == wanted));
        found.ok_or_else(|| anyhow!("the arms' model has no bone {name}"))
    };
    // The breadth of a hand on each skeleton, both hands' taken together.
    let (mut broad, mut their_broad) = (0.0, 0.0);
    for (side, their_side) in [("l", "L"), ("r", "R")] {
        let at = |name: &str| -> Result<V3> {
            let name = name_of(&format!("{side}_{name}"))?;
            Ok(model.bones.iter().find(|bone| bone.name == name).map(|bone| bone.at).unwrap_or_default())
        };
        let their = |name: &str| theirs(skeleton, &format!("Base Human{their_side}{name}")).map(|(_, at)| at);
        broad += length(sub(at("finger1")?, at("finger4")?));
        their_broad += length(sub(their("Digit21")?, their("Digit51")?));
    }
    ensure!(broad > 1e-4, "the arms' model has no breadth of hand");
    let size = size.unwrap_or(their_broad / broad);
    ensure!(size.is_finite() && (0.1..10.0).contains(&size), "the arms' model would be {size} times its size");
    let (meshes, rig) = lay_arms(model, skeleton, size)?;
    Ok((meshes, rig, size))
}
