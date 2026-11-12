//! The rig's bones, an outfit's pieces and arms, and building an outfit.

use super::*;

/// A bone of an outfit's rig: one of the install's bones, with the joint
/// the outfit has for it.
#[derive(Debug, Clone)]
pub struct RigBone {
    /// The install's bone, by its path, and its own matrix for a point of
    /// a mesh (rows).
    pub path: String,
    pub bind: [f32; 16],
    /// Where the outfit has the bone's joint at rest, and where the
    /// install's own skeleton has it, both in the meshes' space.
    pub joint: V3,
    pub their_joint: V3,
    pub follows: Follows,
}

/// The skeleton an outfit is drawn on: the install's bones, each with the
/// outfit's own place for its joint, so that the outfit keeps its own
/// build (how broad it is, how long each limb) while every bone is turned
/// as the install's animation turns it. A bone is listed after whatever it
/// hangs from.
#[derive(Debug, Clone, Default)]
pub struct Rig {
    pub bones: Vec<RigBone>,
}

impl Rig {
    /// Where the outfit's bones are, given where the install's are: for
    /// each bone the move, from rest, of what is on the install's bone
    /// (its place now after its own matrix), and in return the move from
    /// rest of what is on the outfit's.
    pub fn pose(&self, theirs: &[Move]) -> Vec<Move> {
        let mut ours = theirs.to_vec();
        // The turn a limb's bone was given, over the install's, to reach.
        let mut reach: Vec<Option<[V3; 3]>> = vec![None; self.bones.len()];
        for (index, bone) in self.bones.iter().enumerate() {
            match bone.follows {
                Follows::Theirs | Follows::Lower => {}
                Follows::Hung { from } => ours[index] = Move::about(theirs[index].axes, bone.joint, ours[from].of(bone.joint)),
                Follows::Held { from, at } => ours[index] = Move::about(theirs[index].axes, self.bones[at].joint, ours[from].of(self.bones[at].joint)),
                Follows::Along { of } => {
                    let axes = reach[of].map_or(theirs[index].axes, |more| turns(more, theirs[index].axes));
                    ours[index] = Move::about(axes, bone.joint, ours[of].of(bone.joint));
                }
                Follows::Limb { from, lower, end } => {
                    let (low, last) = (&self.bones[lower], &self.bones[end]);
                    let (first, second) = (sub(low.joint, bone.joint), sub(last.joint, low.joint));
                    let (hangs, reaches) = (ours[from].of(bone.joint), theirs[end].of(last.joint));
                    // The side the install's own limb is bent to; where
                    // that one is straight, the side this limb is bent to
                    // at rest, as the install's first bone has turned it.
                    let side = off_line(theirs[index].of(bone.their_joint), theirs[lower].of(low.their_joint), theirs[end].of(last.their_joint));
                    let at_rest = theirs[index].turned(off_line(bone.joint, low.joint, last.joint));
                    let middle = bend(hangs, reaches, length(first), length(second), side, at_rest);
                    let turn = arc(theirs[index].turned(first), sub(middle, hangs));
                    ours[index] = Move::about(turns(turn, theirs[index].axes), bone.joint, hangs);
                    reach[index] = Some(turn);
                    let turn = arc(theirs[lower].turned(second), sub(reaches, middle));
                    ours[lower] = Move::about(turns(turn, theirs[lower].axes), low.joint, middle);
                    reach[lower] = Some(turn);
                }
            }
        }
        ours
    }

    /// The same rig with every point of it given in another space.
    pub fn in_space(&self, of: impl Fn(V3) -> V3) -> Self {
        Self { bones: self.bones.iter().map(|bone| RigBone { joint: of(bone.joint), their_joint: of(bone.their_joint), ..bone.clone() }).collect() }
    }
}

/// One mesh of an outfit, fitted: its vertices at rest about the outfit's
/// rig, in the space of the install's own meshes. A vertex's bones are the
/// rig's by their numbers, and the mesh's matrix for each is of no turn
/// and no shift: a bone of the rig is given to the mesh as the move, from
/// rest, of what is on it (`Rig::pose`).
pub struct Piece {
    pub mesh: MeshData,
    /// The picture's path under the other game's `textures`, and the
    /// picture where it was found, loose or packed.
    pub texture: String,
    pub picture: Option<Picture>,
}

/// The arms of an outfit as the character's own eyes see them: in the
/// other game that is a model of its own, finer than the body's arms,
/// which is drawn in first person in their place. Its meshes, and the rig
/// they are drawn on: an arm's bones and nothing else.
pub struct Arms {
    pub pieces: Vec<Piece>,
    pub rig: Rig,
    /// How many times its file's size the model is.
    pub size: f32,
}

/// An outfit read and fitted: its meshes, the rig they are drawn on, its
/// arms for first person where it has a model for them, and what was left
/// out and why.
pub struct Outfit {
    pub pieces: Vec<Piece>,
    pub rig: Rig,
    pub arms: Option<Arms>,
    pub notes: Vec<String>,
}

/// A model's meshes, each with the picture it names where that is found,
/// loose or packed; one that is not is noted.
fn pictured(stalker: &Stalker, meshes: Vec<MeshData>, model: &xray::Model, notes: &mut Vec<String>) -> Vec<Piece> {
    meshes
        .into_iter()
        .zip(&model.parts)
        .map(|(mesh, part)| {
            let name = format!("textures/{}.dds", part.texture);
            let picture = match stalker.read(&name).and_then(|(bytes, found)| xray::read_picture(&bytes).with_context(|| found)) {
                Ok(picture) => Some(picture),
                Err(e) => {
                    notes.push(format!("the picture {name}: {e:#}: the mesh it is for is not drawn"));
                    None
                }
            };
            Piece { mesh, texture: part.texture.clone(), picture }
        })
        .collect()
}

/// Reads an outfit's models from the other game's folders and fits them
/// to the install's skeleton: its body, and its arms for first person.
///
/// **Whatever cannot be drawn is left to the character's own**, and noted:
/// a body that is not named, not found, not read or without a picture
/// leaves the character's own body, and arms so leave the character's own
/// arms (`Outfit::has_body`, `Outfit::has_arms`). A picture that is not
/// found, loose or in the game's packed archives, leaves its own mesh out.
///
/// `waist` is the box about the install's own mesh of the legs at rest:
/// what of the body lies under its top is what the character's own eyes
/// see of it (`fit`).
pub fn build(definition: &Definition, stalker: &Stalker, skeleton: &[RestBone], waist: Option<[V3; 2]>) -> Outfit {
    let mut notes = Vec::new();
    let read = |name: &str| -> Result<xray::Model> {
        ensure!(stalker.known(), "the other game's folders are not known (local/gamma_dir.txt, local/anomaly_dir.txt)");
        let (bytes, found) = stalker.read(&format!("meshes/{name}"))?;
        xray::read(&bytes).with_context(|| found)
    };
    let (mut pieces, mut rig) = (Vec::new(), Rig::default());
    if let Some(visual) = &definition.visual {
        match read(visual).and_then(|model| fit(&model, skeleton, waist).map(|fitted| (model, fitted))) {
            Ok((model, (meshes, fitted))) => {
                pieces = pictured(stalker, meshes, &model, &mut notes);
                rig = fitted;
                if !pieces.iter().any(|piece| piece.picture.is_some()) {
                    notes.push(format!("none of its body's pictures is there ({visual}): the character's own body is drawn"));
                }
            }
            Err(e) => notes.push(format!("its body ({visual}) is not drawn, and the character's own is: {e:#}")),
        }
    }
    let arms = definition.arms.as_ref().and_then(|visual| {
        let fitted = |notes: &mut Vec<String>| -> Result<Arms> {
            let model = read(visual)?;
            let (meshes, rig, size) = fit_arms(&model, skeleton, definition.arms_size)?;
            let pieces = pictured(stalker, meshes, &model, notes);
            ensure!(pieces.iter().all(|piece| piece.picture.is_some()), "one of its pictures is not there");
            Ok(Arms { pieces, rig, size })
        };
        match fitted(&mut notes) {
            Ok(arms) => Some(arms),
            Err(e) => {
                notes.push(format!("its arms for first person ({visual}) are not drawn, and the character's own are: {e:#}"));
                None
            }
        }
    });
    Outfit { pieces, rig, arms, notes }
}

/// The box about some points, its lowest corner and its highest; none
/// about no points.
pub fn box_about(points: &[V3]) -> Option<[V3; 2]> {
    let first = *points.first()?;
    Some(points.iter().fold([first, first], |[low, high], p| [std::array::from_fn(|k| low[k].min(p[k])), std::array::from_fn(|k| high[k].max(p[k]))]))
}

impl Outfit {
    /// Whether it has a body to draw in place of the character's own.
    pub fn has_body(&self) -> bool {
        self.pieces.iter().any(|piece| piece.picture.is_some())
    }

    /// Whether it has arms to draw, for the character's own eyes, in
    /// place of the character's own.
    pub fn has_arms(&self) -> bool {
        self.arms.as_ref().is_some_and(|arms| arms.pieces.iter().any(|piece| piece.picture.is_some()))
    }
}
