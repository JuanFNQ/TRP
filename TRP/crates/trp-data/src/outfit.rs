//! An outfit that is not the install's: a body of S.T.A.L.K.E.R.'s, worn
//! in place of the character's own and moved by the install's skeleton
//! (`docs/spec/outfits.md`).
//!
//! Such an outfit is described by a file in the workspace's
//! `custom/outfits` folder. It becomes an item of the database (its
//! template that of one of the install's armours, under a name of its
//! own), and its model is read from the user's own install of the other
//! game where that lies (`crate::xray`), every time the program starts.
//!
//! The model has a skeleton of its own, with other bones at other places,
//! and a build of its own. It keeps the build and not the skeleton: at
//! rest it is brought into the pose the install's clothing is made in,
//! each piece turned at its own joint and nothing stretched (`fit`), and
//! it is drawn on a rig that has the install's bones at the model's own
//! joints (`Rig`), which is posed every frame from the install's skeleton
//! as that then stands: the same turns on other lengths.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{anyhow, ensure, Context, Result};
use serde::Deserialize;
use serde_json::Value as Json;

use crate::database::{Database, ItemTemplate};
use crate::unity::extract::MeshData;
use crate::xray::{self, Picture, Stalker};

/// An outfit of one's own, as its file describes it.
#[derive(Debug, Clone, Deserialize)]
pub struct Definition {
    /// The item template's id and its name, in the database's manner.
    pub id: String,
    pub name: String,
    /// What it is called where a short name is shown.
    #[serde(default)]
    pub short_name: String,
    /// The install's armour whose item this is, but for its name: its
    /// slot, its cells, its weight.
    pub donor: String,
    /// The model of its body, by its path under the other game's
    /// `meshes`, where it has one from there.
    #[serde(default)]
    pub visual: Option<String>,
    /// The model of the item as it lies in the world, the same way (the
    /// other game's `visual` of the item, where `visual` here is its
    /// `actor_visual`): its picture in the inventory is drawn from that
    /// (`docs/spec/icons.md`). Without it, the donor's picture.
    #[serde(default)]
    pub world_visual: Option<String>,
    /// The model of its arms as the character's own eyes see them, the
    /// same way, where the other game has one for it.
    #[serde(default)]
    pub arms: Option<String>,
    /// How many times its file's size that model is drawn, where the file
    /// says: without it, the size at which its hand is as broad as the
    /// install's (`fit_arms`).
    #[serde(default)]
    pub arms_size: Option<f32>,
    /// Clothing of the install's own in place of the character's, each by
    /// its bundle: a top (the upper body as others see it), and the hands
    /// as the character's own eyes see them. An outfit made of these is
    /// the install's own models on the install's own skeleton, with
    /// nothing fitted: it is there to set another outfit beside.
    #[serde(default)]
    pub their_top: Option<String>,
    #[serde(default)]
    pub their_hands: Option<String>,
}

/// The workspace's folder of such files: `custom/outfits` beside a
/// `Cargo.toml`, from where the program runs upwards.
pub fn folder() -> Option<PathBuf> {
    crate::custom::folder().map(|custom| custom.join("outfits")).filter(|folder| folder.is_dir())
}

/// Reads every definition of a folder; one that cannot be read is left
/// out with a word on why.
pub fn definitions(folder: &Path) -> (Vec<Definition>, Vec<String>) {
    let (mut found, mut notes) = (Vec::new(), Vec::new());
    let mut files: Vec<PathBuf> = std::fs::read_dir(folder).into_iter().flatten().flatten().map(|entry| entry.path()).filter(|path| path.extension().is_some_and(|e| e == "json")).collect();
    files.sort();
    for file in files {
        match std::fs::read_to_string(&file).map_err(anyhow::Error::from).and_then(|text| Ok(serde_json::from_str::<Definition>(&text)?)) {
            Ok(definition) => found.push(definition),
            Err(e) => notes.push(format!("{}: {e:#}", file.display())),
        }
    }
    (found, notes)
}

/// Adds an outfit to the database: the donor's template under this one's
/// id and name.
pub fn register(db: &mut Database, definition: Definition) -> Result<()> {
    ensure!(!db.items.contains_key(&definition.id), "its id is taken");
    let mut outfit: ItemTemplate = db.item(&definition.donor)?.clone();
    outfit.id = definition.id.clone();
    outfit.name = definition.name.clone();
    for key in ["Name", "ShortName", "Description"] {
        outfit.props[key] = Json::from(definition.name.clone());
    }
    // It is one piece: none of the donor's slots is kept. (An armour's
    // are for its plates and its soft inserts, and the inserts are parts
    // it may not be put on without.)
    outfit.props["Slots"] = Json::Array(Vec::new());
    db.items.insert(outfit.id.clone(), outfit);
    db.outfits.insert(definition.id.clone(), Arc::new(definition));
    Ok(())
}

/// Adds every outfit of the workspace's folder to the database. Whatever
/// is left out is noted in `Database::custom_notes`.
pub fn register_all(db: &mut Database) {
    let Some(folder) = folder() else { return };
    let (found, mut notes) = definitions(&folder);
    for definition in found {
        let name = definition.name.clone();
        if let Err(e) = register(db, definition) {
            notes.push(format!("{name}: {e:#}"));
        }
    }
    db.custom_notes.extend(notes);
}

impl Definition {
    /// What it is called for short.
    pub fn shown_name(&self) -> &str {
        if self.short_name.is_empty() {
            &self.name
        } else {
            &self.short_name
        }
    }
}

/// A point or a direction.
pub type V3 = [f32; 3];

fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn scaled(a: V3, by: f32) -> V3 {
    [a[0] * by, a[1] * by, a[2] * by]
}

fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: V3, b: V3) -> V3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

fn length(a: V3) -> f32 {
    dot(a, a).sqrt()
}

fn unit(a: V3) -> Option<V3> {
    let long = length(a);
    (long > 1e-6).then(|| scaled(a, 1.0 / long))
}

/// Where a bone of the install's skeleton is in its mesh's space at rest.
/// A mesh keeps, for each bone, the matrix that takes a point of the mesh
/// into the bone's own axes (rows `e00..e33`); the bone's own origin is
/// what that matrix takes to nothing.
pub fn rest_place(bind: &[f32; 16]) -> Option<V3> {
    let (a, b, c) = ([bind[0], bind[1], bind[2]], [bind[4], bind[5], bind[6]], [bind[8], bind[9], bind[10]]);
    let t = [bind[3], bind[7], bind[11]];
    let det = dot(a, cross(b, c));
    if det.abs() < 1e-12 {
        return None;
    }
    // The columns of the inverse of the matrix whose rows are a, b and c
    // are those rows' cross products, over the determinant.
    let inverse = [scaled(cross(b, c), 1.0 / det), scaled(cross(c, a), 1.0 / det), scaled(cross(a, b), 1.0 / det)];
    let at = |row: usize| -(inverse[0][row] * t[0] + inverse[1][row] * t[1] + inverse[2][row] * t[2]);
    Some([at(0), at(1), at(2)])
}

/// A bone of the install's skeleton as the clothing's meshes have it: its
/// path on the skeleton and the matrix (rows `e00..e33`) that takes a
/// point of the mesh into the bone's own axes.
#[derive(Debug, Clone)]
pub struct RestBone {
    pub path: String,
    pub bind: [f32; 16],
}

/// The three lists of triangles a fitted mesh has, by who sees them: the
/// legs, which the character's own eyes see; the arms, which they see in
/// place of the install's own arms; and the rest, which in first person
/// is a shadow and nothing more, as the install's upper body is.
pub const LEGS: usize = 0;
pub const ARMS: usize = 1;
pub const UPPER: usize = 2;

/// A move of something rigid: three axes and a shift. A point `p` goes to
/// `p.x * axes[0] + p.y * axes[1] + p.z * axes[2] + shift`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Move {
    pub axes: [V3; 3],
    pub shift: V3,
}

const NO_TURN: [V3; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

impl Move {
    /// The move that leaves everything where it is.
    pub const STILL: Move = Move { axes: NO_TURN, shift: [0.0; 3] };

    /// A direction, turned.
    pub fn turned(&self, v: V3) -> V3 {
        add(add(scaled(self.axes[0], v[0]), scaled(self.axes[1], v[1])), scaled(self.axes[2], v[2]))
    }

    /// A point, moved.
    pub fn of(&self, p: V3) -> V3 {
        add(self.turned(p), self.shift)
    }

    /// The move that turns so and leaves one point at a place.
    fn about(axes: [V3; 3], joint: V3, at: V3) -> Self {
        let turn = Move { axes, shift: [0.0; 3] };
        Move { axes, shift: sub(at, turn.turned(joint)) }
    }
}

/// One turn after another: `first`, then `then`.
fn turns(then: [V3; 3], first: [V3; 3]) -> [V3; 3] {
    let then = Move { axes: then, shift: [0.0; 3] };
    first.map(|axis| then.turned(axis))
}

/// The shortest turn that takes one direction to another.
fn arc(from: V3, to: V3) -> [V3; 3] {
    let (Some(u), Some(v)) = (unit(from), unit(to)) else { return NO_TURN };
    let (k, c) = (cross(u, v), dot(u, v));
    if c < -0.9999 {
        // Half a turn, about anything across the direction.
        let across = unit(cross(u, [1.0, 0.0, 0.0])).or_else(|| unit(cross(u, [0.0, 1.0, 0.0]))).unwrap_or([0.0, 0.0, 1.0]);
        return NO_TURN.map(|x| sub(scaled(across, 2.0 * dot(across, x)), x));
    }
    NO_TURN.map(|x| add(add(x, cross(k, x)), scaled(cross(k, cross(k, x)), 1.0 / (1.0 + c))))
}

/// How far a limb's middle joint is off the line from where the limb
/// hangs to where it ends, and which way: the side the limb is bent to.
fn off_line(hangs: V3, middle: V3, ends: V3) -> V3 {
    let to = sub(middle, hangs);
    unit(sub(ends, hangs)).map_or(to, |along| sub(to, scaled(along, dot(to, along))))
}

/// Where a limb of two bones is bent: its middle joint, for a limb that
/// hangs at one place and reaches for another with two lengths. It bends
/// to the side `towards` points (what of that lies across the limb's own
/// line), or `else_towards` where that says nothing. Out of reach, the
/// limb is straight and its end short of the place.
fn bend(hangs: V3, reaches: V3, first: f32, second: f32, towards: V3, else_towards: V3) -> V3 {
    let line = sub(reaches, hangs);
    let Some(along) = unit(line) else { return add(hangs, scaled(unit(towards).unwrap_or([0.0, 1.0, 0.0]), first)) };
    let far = length(line).clamp((first - second).abs() + 1e-5, first + second);
    let before = (first * first - second * second + far * far) / (2.0 * far);
    let off = (first * first - before * before).max(0.0).sqrt();
    let across = |v: V3| unit(sub(v, scaled(along, dot(v, along)))).filter(|_| length(sub(v, scaled(along, dot(v, along)))) > 1e-4);
    let side = across(towards).or_else(|| across(else_towards)).or_else(|| across([1.0, 0.0, 0.0])).or_else(|| across([0.0, 1.0, 0.0])).unwrap_or([0.0, 0.0, 1.0]);
    add(add(hangs, scaled(along, before)), scaled(side, off))
}

/// How a bone of an outfit's rig follows the install's skeleton, frame by
/// frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Follows {
    /// As the install's own bone: what is on it is carried exactly as that
    /// bone carries its own (the hips; a hand, which is on the weapon; a
    /// foot, which is on the ground).
    Theirs,
    /// Hung from another bone of the rig at this one's joint, and turned
    /// as the install's bone is turned.
    Hung { from: usize },
    /// The first bone of a limb of two (an upper arm, a thigh): hung from
    /// one bone and reaching for the joint of `end`, which is the
    /// install's own, with the outfit's own two lengths; bent at `lower`'s
    /// joint, to the side the install's own limb is bent.
    Limb { from: usize, lower: usize, end: usize },
    /// The second bone of such a limb: placed with the first.
    Lower,
    /// A bone along a limb's bone (the install's skeleton has a forearm in
    /// three and a thigh in two): hung from it, turned as the install's is
    /// and then as the limb's bone was turned to reach.
    Along { of: usize },
    /// Held by another bone of the rig at that bone's (or a third's)
    /// joint, and turned as the install's bone is turned: an arm seen
    /// from its own eyes goes back from the hand so, the forearm held at
    /// the wrist and the upper arm at the elbow, each lying exactly as the
    /// install's lies and as long as the outfit's own.
    Held { from: usize, at: usize },
}

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

/// Three directions at right angles: to the character's right, up and
/// ahead.
#[derive(Clone, Copy)]
struct Basis {
    right: V3,
    up: V3,
    ahead: V3,
}

impl Basis {
    /// From where two things lie to the left and to the right, and one
    /// low and one high. A model stands along the axes of its own space,
    /// so each direction is the axis nearest to it: a head a little
    /// behind the hips does not tip what is up.
    fn of(left: V3, right: V3, low: V3, high: V3) -> Option<Self> {
        let axis = |v: V3| -> Option<V3> {
            let most = (0..3).max_by(|a, b| v[*a].abs().total_cmp(&v[*b].abs()))?;
            let mut along = [0.0; 3];
            along[most] = v[most].signum();
            (v[most].abs() > 1e-6).then_some(along)
        };
        let (right, up) = (axis(sub(right, left))?, axis(sub(high, low))?);
        (dot(right, up) == 0.0).then(|| Self { right, up, ahead: cross(right, up) })
    }

    /// A vector's three parts along them.
    fn parts(&self, v: V3) -> V3 {
        [dot(v, self.right), dot(v, self.up), dot(v, self.ahead)]
    }

    /// The vector that has those parts.
    fn whole(&self, parts: V3) -> V3 {
        add(add(scaled(self.right, parts[0]), scaled(self.up, parts[1])), scaled(self.ahead, parts[2]))
    }
}

/// Three axes at right angles, the first along one direction and the
/// second as near another as that leaves it.
fn frame(along: V3, toward: V3) -> Option<[V3; 3]> {
    let x = unit(along)?;
    let y = unit(sub(toward, scaled(x, dot(toward, x))))?;
    Some([x, y, cross(x, y)])
}

/// A vector turned about an axis of length one.
fn turned(v: V3, axis: V3, angle: f32) -> V3 {
    let (sin, cos) = angle.sin_cos();
    add(add(scaled(v, cos), scaled(cross(axis, v), sin)), scaled(axis, dot(axis, v) * (1.0 - cos)))
}

/// How a piece of the model about one of its bones is laid at rest on the
/// outfit's rig: the bone's joint goes to a place, and its axes are turned
/// to the pose the install's clothing is made in. **Nothing is stretched**
/// but a finger seen by its own eyes: the piece keeps its size and its
/// shape. With a twist, what lies further along the bone is turned about
/// it by so much more, up to that angle at its far end (a forearm, which
/// turns with the hand).
#[derive(Clone, Copy)]
struct Laid {
    from: V3,
    to: V3,
    ours: [V3; 3],
    theirs: [V3; 3],
    /// The length of the model's bone.
    long: f32,
    twist: f32,
    /// How many times as long the piece is made along its bone, and no
    /// broader: one everywhere but a finger's joint of arms seen by their
    /// own eyes, which is made as long as the install's (`lay_arm`).
    stretch: f32,
}

impl Laid {
    /// A bone from its joint towards the next, laid from a place along a
    /// direction, with a second direction on each side to settle the turn
    /// about it.
    fn bone(from: V3, next: V3, toward: V3, to: V3, their_along: V3, their_toward: V3) -> Option<Self> {
        let (ours, theirs) = (frame(sub(next, from), toward)?, frame(their_along, their_toward)?);
        Some(Self { from, to, ours, theirs, long: length(sub(next, from)), twist: 0.0, stretch: 1.0 })
    }

    /// The same turn about another joint: a bone with none after it, laid
    /// as the one before it, at its own length.
    fn carried_to(&self, from: V3, to: V3) -> Self {
        Self { from, to, twist: 0.0, stretch: 1.0, ..*self }
    }

    /// The same laying, the piece made as long along its bone as takes
    /// the bone's end to a place that many metres from its joint.
    fn as_long_as(self, long: f32) -> Self {
        Self { stretch: long / self.long.max(1e-6), ..self }
    }

    /// How far along the bone a point is, from none at its joint to one
    /// at the next.
    fn share_along(&self, p: V3) -> f32 {
        (dot(sub(p, self.from), self.ours[0]) / self.long.max(1e-6)).clamp(0.0, 1.0)
    }

    fn turn(&self, v: V3, along: f32) -> V3 {
        let v = if self.twist != 0.0 { turned(v, self.ours[0], self.twist * along) } else { v };
        add(add(scaled(self.theirs[0], dot(v, self.ours[0])), scaled(self.theirs[1], dot(v, self.ours[1]))), scaled(self.theirs[2], dot(v, self.ours[2])))
    }

    fn place(&self, p: V3) -> V3 {
        let from_joint = sub(p, self.from);
        let longer = scaled(self.ours[0], dot(from_joint, self.ours[0]) * (self.stretch - 1.0));
        add(self.to, self.turn(add(from_joint, longer), self.share_along(p)))
    }

    fn normal(&self, p: V3, n: V3) -> V3 {
        self.turn(n, self.share_along(p))
    }

    /// The angle by which another laying turns things about this bone
    /// more than this one does: the hand's about the forearm.
    fn twist_to(&self, other: &Laid) -> f32 {
        // One of this bone's cross axes as the other laying leaves it,
        // seen in the axes this one is laid to.
        let theirs = other.turn(self.ours[1], 0.0);
        dot(theirs, self.theirs[2]).atan2(dot(theirs, self.theirs[1]))
    }
}

/// The names a model has for an arm's bones.
struct ArmNames {
    upper: String,
    /// The forearm's bone, and any other whose share is the forearm's (a
    /// bone some models have for its turn).
    fore: Vec<String>,
    hand: String,
    /// The fingers, each with its three joints from the knuckle out and
    /// the number the install's skeleton has for it (the thumb is 1).
    fingers: Vec<([String; 3], u8)>,
    /// The finger by whose knuckle the hand is laid, and the one whose
    /// knuckle beside that settles the hand's turn.
    by: usize,
    beside: usize,
}

/// What an arm hangs from.
#[derive(Clone, Copy)]
enum Hang {
    /// A place (the model's own shoulder), on a bone of the rig: the arm
    /// reaches from there to the hand with its own two lengths, and the
    /// elbow is wherever that puts it.
    From { at: V3, collar: usize },
    /// The hand: the arm goes back from it, the forearm and then the
    /// upper arm each lying as the install's lies, and its shoulder is
    /// wherever that puts it. For arms seen by their own eyes, of which
    /// the hand and the forearm are in the picture and the shoulder is
    /// not.
    Hand,
}

/// A bone of the install's put on a rig, with the outfit's joint for it.
fn add_bone(rig: &mut Rig, skeleton: &[RestBone], name: &str, joint: V3, follows: Follows) -> Result<usize> {
    let (index, their_joint) = theirs(skeleton, &format!("Base Human{name}"))?;
    rig.bones.push(RigBone { path: skeleton[index].path.clone(), bind: skeleton[index].bind, joint, their_joint, follows });
    Ok(rig.bones.len() - 1)
}

/// Lays one arm of a model at rest and puts its bones on the rig.
///
/// Its hand is the install's, for it is on the weapon. **The hand is laid
/// by its knuckles**: the knuckle of one finger on the install's, the
/// hand turned as the install's (along the wrist to that knuckle, the
/// next finger's knuckle beside it), and its wrist wherever that leaves
/// it, which two skeletons with the wrist's joint at other places in the
/// hand do not agree on. The rest of the arm goes by what it hangs from
/// (`Hang`).
///
/// **The fingers of arms seen by their own eyes are the install's
/// fingers**: every joint of every finger, the thumb's too, is put on the
/// install's own joint, and each piece of a finger made as long as the
/// install's (and no broader) so that the next joint falls on the next.
/// Every hold of the install's weapons is a pose of those joints; a
/// finger whose joints are elsewhere, turned by the same angles, ends
/// elsewhere, and the further the more it is bent: a thumb whose first
/// bone is a third longer and begins two centimetres away wraps a grip
/// two centimetres from where the install's thumb lies. A body seen from
/// outside keeps its fingers as they are.
fn lay_arm(ours: &dyn Fn(&str) -> Result<V3>, skeleton: &[RestBone], rig: &mut Rig, goes: &mut HashMap<String, Goes>, their_side: &str, names: &ArmNames, hang: Hang) -> Result<()> {
    let missing = || anyhow!("two joints of an arm are at one place");
    let named = |name: &str| format!("{their_side}{name}");
    let their = |name: &str| theirs(skeleton, &format!("Base Human{their_side}{name}")).map(|(_, at)| at);
    let (shoulder, elbow, wrist) = (ours(&names.upper)?, ours(&names.fore[0])?, ours(&names.hand)?);
    let (their_shoulder, their_elbow, their_wrist) = (their("Upperarm")?, their("Forearm1")?, their("Palm")?);
    let digit = |finger: usize, joint: usize| their(&format!("Digit{}{}", names.fingers[finger].1, joint + 1));

    // The hand.
    let (knuckle, beside) = (ours(&names.fingers[names.by].0[0])?, ours(&names.fingers[names.beside].0[0])?);
    let (their_knuckle, their_beside) = (digit(names.by, 0)?, digit(names.beside, 0)?);
    let (across, their_across) = (sub(beside, knuckle), sub(their_beside, their_knuckle));
    // **A hand of five fingers seen by its own eyes is laid by the whole
    // row of its knuckles and by its wrist**: the middle of the line from
    // the first finger's knuckle to the little finger's on the middle of
    // the install's, that line along the install's, and the hand turned
    // about it towards the install's wrist. Laid by two neighbouring
    // knuckles alone, a hand whose row of knuckles is bent a little
    // otherwise than the install's is rolled about its own length: the
    // one model measured stood a centimetre and more out of the
    // install's hand at the little finger.
    //
    // **And it bends at the install's wrist.** The wrist is taken where
    // that laying has the install's own, wherever the model's maker put
    // the joint (the one model measured has it two centimetres inside
    // the palm, and its skin bends behind it, where this puts it). So the
    // forearm comes away from the install's own wrist along the install's
    // own forearm, and is turned about that wrist; turned about a joint
    // inside the palm, it left the install's forearm by as much as the
    // wrist was bent.
    let of_number = |number: u8| names.fingers.iter().position(|finger| finger.1 == number);
    let (hand, wrist) = match (hang, of_number(2), of_number(5)) {
        (Hang::Hand, Some(first), Some(little)) => {
            let (first_at, little_at) = (ours(&names.fingers[first].0[0])?, ours(&names.fingers[little].0[0])?);
            let (their_first, their_little) = (digit(first, 0)?, digit(little, 0)?);
            let (middle, their_middle) = (scaled(add(first_at, little_at), 0.5), scaled(add(their_first, their_little), 0.5));
            let axes = frame(sub(little_at, first_at), sub(wrist, middle)).ok_or_else(missing)?;
            let their_axes = frame(sub(their_little, their_first), sub(their_wrist, their_middle)).ok_or_else(missing)?;
            let hand = Laid { from: middle, to: their_middle, ours: axes, theirs: their_axes, long: 1.0, twist: 0.0, stretch: 1.0 };
            let back = sub(their_wrist, their_middle);
            (hand, add(middle, add(scaled(axes[0], dot(back, their_axes[0])), scaled(axes[1], dot(back, their_axes[1])))))
        }
        _ => {
            let mut hand = Laid::bone(wrist, knuckle, across, their_wrist, sub(their_knuckle, their_wrist), their_across).ok_or_else(missing)?;
            hand.to = add(hand.to, sub(their_knuckle, hand.place(knuckle)));
            (hand, wrist)
        }
    };
    let wrist_at = hand.place(wrist);

    // The upper arm and the forearm, the elbow's axis (at right angles to
    // both) settling the turn about each; and the forearm twisted towards
    // the hand along its length. From a shoulder, the elbow is where the
    // model's own two lengths put it, bent to the side the install's arm
    // is bent at rest; from the hand, each bone lies as the install's.
    let (upper_long, fore_long) = (length(sub(elbow, shoulder)), length(sub(wrist, elbow)));
    let (shoulder_at, elbow_at) = match hang {
        Hang::From { at, .. } => (at, bend(at, wrist_at, upper_long, fore_long, off_line(their_shoulder, their_elbow, their_wrist), [0.0; 3])),
        Hang::Hand => {
            let (their_upper, their_fore) = (unit(sub(their_elbow, their_shoulder)).ok_or_else(missing)?, unit(sub(their_wrist, their_elbow)).ok_or_else(missing)?);
            let elbow_at = sub(wrist_at, scaled(their_fore, fore_long));
            (sub(elbow_at, scaled(their_upper, upper_long)), elbow_at)
        }
    };
    let fore_along = unit(sub(wrist_at, elbow_at)).ok_or_else(missing)?;
    let hinge = cross(sub(elbow, shoulder), sub(wrist, elbow));
    let hinge_at = cross(sub(elbow_at, shoulder_at), fore_along);
    let upper = Laid::bone(shoulder, elbow, hinge, shoulder_at, sub(elbow_at, shoulder_at), hinge_at).ok_or_else(missing)?;
    let mut fore = Laid::bone(elbow, wrist, hinge, elbow_at, fore_along, hinge_at).ok_or_else(missing)?;
    fore.twist = fore.twist_to(&hand);
    let along_fore = |share: f32| add(elbow_at, scaled(fore_along, fore_long * share));
    let first = rig.bones.len();
    let (upper_bone, fore_bone, palm) = match hang {
        Hang::From { collar, .. } => {
            add_bone(rig, skeleton, &named("Upperarm"), shoulder_at, Follows::Limb { from: collar, lower: first + 1, end: first + 4 })?;
            add_bone(rig, skeleton, &named("Forearm1"), elbow_at, Follows::Lower)?;
            for (name, share) in [("Forearm2", 1.0 / 3.0), ("Forearm3", 2.0 / 3.0)] {
                add_bone(rig, skeleton, &named(name), along_fore(share), Follows::Along { of: first + 1 })?;
            }
            add_bone(rig, skeleton, &named("Palm"), wrist_at, Follows::Theirs)?;
            (first, first + 1, first + 4)
        }
        Hang::Hand => {
            add_bone(rig, skeleton, &named("Palm"), wrist_at, Follows::Theirs)?;
            for (name, share) in [("Forearm1", 0.0), ("Forearm2", 1.0 / 3.0), ("Forearm3", 2.0 / 3.0)] {
                add_bone(rig, skeleton, &named(name), along_fore(share), Follows::Held { from: first, at: first })?;
            }
            add_bone(rig, skeleton, &named("Upperarm"), shoulder_at, Follows::Held { from: first + 1, at: first + 1 })?;
            (first + 4, first + 1, first)
        }
    };
    goes.insert(names.upper.clone(), Goes::To(upper, upper_bone));
    for name in &names.fore {
        goes.insert(name.clone(), Goes::Along(fore, [Some(fore_bone), Some(fore_bone + 1), Some(fore_bone + 2)]));
    }
    goes.insert(names.hand.clone(), Goes::To(hand, palm));

    // The fingers: each joint hangs where the piece before it carries it
    // and lies along the install's joint at rest; the last as the one
    // before it. A finger's turn about itself is settled by the line of
    // the knuckles; the thumb lies along that line, so its turn is
    // settled by the hand's own length.
    let (along_hand, their_along_hand) = (sub(knuckle, wrist), sub(their_knuckle, their_wrist));
    for (finger, (joint_names, number)) in names.fingers.iter().enumerate() {
        let (toward, their_toward) = if *number == 1 { (along_hand, their_along_hand) } else { (across, their_across) };
        let joints = [ours(&joint_names[0])?, ours(&joint_names[1])?, ours(&joint_names[2])?];
        let digits = [digit(finger, 0)?, digit(finger, 1)?, digit(finger, 2)?];
        let (first, second) = match hang {
            Hang::Hand => {
                let first = Laid::bone(joints[0], joints[1], toward, digits[0], sub(digits[1], digits[0]), their_toward).ok_or_else(missing)?;
                let second = Laid::bone(joints[1], joints[2], toward, digits[1], sub(digits[2], digits[1]), their_toward).ok_or_else(missing)?;
                (first.as_long_as(length(sub(digits[1], digits[0]))), second.as_long_as(length(sub(digits[2], digits[1]))))
            }
            Hang::From { .. } => {
                let first = Laid::bone(joints[0], joints[1], toward, hand.place(joints[0]), sub(digits[1], digits[0]), their_toward).ok_or_else(missing)?;
                let second = Laid::bone(joints[1], joints[2], toward, first.place(joints[1]), sub(digits[2], digits[1]), their_toward).ok_or_else(missing)?;
                (first, second)
            }
        };
        let last = second.carried_to(joints[2], second.place(joints[2]));
        let mut from = palm;
        for (joint, laid) in [first, second, last].into_iter().enumerate() {
            from = add_bone(rig, skeleton, &named(&format!("Digit{number}{}", joint + 1)), laid.to, Follows::Hung { from })?;
            goes.insert(joint_names[joint].clone(), Goes::To(laid, from));
        }
    }
    Ok(())
}

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
fn lay_meshes(model: &xray::Model, how: &[Option<Goes>], zone: &[usize], bones: usize, trunk: &dyn Fn(V3, V3) -> (V3, V3, [(usize, f32); 2]), under: Option<(V3, f32)>) -> Result<Vec<MeshData>> {
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

/// A skin at rest, to measure a hand by: each of its points with the bone
/// it mostly goes with, and the bones' names on the install's skeleton
/// (what follows `Base Human` in a bone's path: `LPalm`, `RDigit21`).
#[derive(Debug, Clone, Default)]
pub struct Skin {
    bones: Vec<String>,
    points: Vec<(V3, u16)>,
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
struct HandFrame {
    centre: V3,
    across: V3,
    along: V3,
    through: V3,
}

impl HandFrame {
    fn of(skeleton: &[RestBone], side: &str) -> Result<Self> {
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
    fn parts(&self, p: V3) -> V3 {
        let from = sub(p, self.centre);
        [dot(from, self.across), -dot(from, self.along), dot(from, self.through)]
    }
}

/// Where a palm is measured: slices across the hand a centimetre thick,
/// so far behind the knuckles' line (metres).
const PALM_SLICES: [f32; 5] = [0.025, 0.035, 0.045, 0.055, 0.065];

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
fn lay_arms(model: &xray::Model, skeleton: &[RestBone], size: f32) -> Result<(Vec<MeshData>, Rig)> {
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

/// Where one of the model's bones sends what goes with it: laid so, to a
/// bone of the rig or to several.
#[derive(Clone, Copy)]
enum Goes {
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
fn theirs(skeleton: &[RestBone], name: &str) -> Result<(usize, V3)> {
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

#[cfg(test)]
mod tests {
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
}
