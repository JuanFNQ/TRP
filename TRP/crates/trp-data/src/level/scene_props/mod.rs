//! Things laid in a level's scene, to be drawn elsewhere: an object and all under it that is on.
//! Each with what it draws (its meshes, their materials and those materials' pictures), read from
//! the build's own files beside the scene. Those keep no type trees: a mesh and a
//! material are read by the trees of the same classes in a bundle of the
//! install's, which the same engine wrote (`screen::tree_of`), a picture by
//! the scene pictures' reader (`screen::ScenePictures`). The first is the
//! hideout's workbench, drawn with the game's own model (workbench spec).
//!
//! **Static batching.** A level's meshes that never move are joined by the
//! engine as the build is made into one mesh of many parts, their corners
//! already where they lie in the world (`StaticBatchUtility`); a renderer
//! so joined names the joined mesh and its own run of parts
//! (`m_StaticBatchInfo`). Its parts are taken out of the joined mesh here,
//! each with only the corners it uses, and brought into the axes of the
//! object read (`Props::root`): drawn there, not on their own objects.
//!
//! **Levels of detail.** A group of levels of detail (`LODGroup`) on an
//! object read lists which renderers each of its levels draws. Asked for
//! every level (`levels`), each renderer any level lists is read and its
//! object tagged with its group and levels (`PropNode::lod`, the groups in
//! `Props::lods`), for the app's picker of levels (`lod_view`) to choose
//! among; else only the nearest level's renderers are read and none is
//! tagged.

use std::collections::HashMap;

use anyhow::{bail, Context, Result};

use crate::setup::install::Install;
use crate::interface::screen::{scene_paths, tree_of, Scene, ScenePictures, MESH_FILTER};
use crate::unity::extract::{mesh_streamed, MeshData, TextureData};
use crate::unity::reader::Reader;
use crate::unity::{class, Value};

mod props_read;
mod prop_parts;

pub use props_read::*;
use prop_parts::{BOX_COLLIDER, LOD_GROUP, Renderer, box_collider, lod_group, material_of, part_of, texture_pointer, to_root};

/// The bundle the mesh's and the material's trees are borrowed from: the
/// effects', which has both.
const TREES: &str = crate::audio::sound::EFFECTS;

/// An object read and all under it that is on.
#[derive(Debug, Default)]
pub struct Props {
    /// Every object, the one read first, each after its parent; its own
    /// place, turn and scale under its parent (the first's: none).
    pub nodes: Vec<PropNode>,
    pub meshes: Vec<MeshData>,
    pub materials: Vec<PropMaterial>,
    pub pictures: Vec<TextureData>,
    /// Where the object read lies in its scene and how it is turned (its
    /// scale taken as one).
    pub root: ([f32; 3], [f32; 4]),
    /// What could not be read, with why.
    pub skipped: Vec<String>,
    /// With every level asked for: each group of levels of detail on an
    /// object read, by that object's place in `nodes` (the group's
    /// renderers pointed to in the scene's own file, file 0). Empty else.
    pub lods: Vec<crate::unity::extract::HierarchyLod>,
}

#[derive(Debug, Clone)]
pub struct PropNode {
    pub name: String,
    pub parent: Option<usize>,
    pub translation: [f32; 3],
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
    /// What it draws: a mesh (one part of a mesh: one material) and its
    /// material, by their places in `Props`.
    pub drawn: Vec<(usize, usize)>,
    /// Its meshes are in the axes of the object read, not its own (taken
    /// out of a statically batched mesh).
    pub in_root: bool,
    /// Its box colliders that are on, in its own axes: each one's middle
    /// and size (`BoxCollider.m_Center`, `m_Size`), and whether it is a
    /// trigger.
    pub boxes: Vec<([f32; 3], [f32; 3], bool)>,
    /// With every level asked for: the group among `Props::lods` that
    /// lists its renderer, and the levels it is drawn at, a bit each.
    pub lod: Option<crate::unity::extract::NodeLod>,
}

/// A material as the app draws the game's: its colour picture, its normal
/// map and its gloss (`_MainTex`, `_BumpMap`, `_SpecMap`, by their places
/// in `Props::pictures`), its tint (`_Color`), the queue it is drawn in
/// (`m_CustomRenderQueue`: −1 its shader's), its keywords and its floats.
#[derive(Debug, Clone, Default)]
pub struct PropMaterial {
    pub name: String,
    pub base: Option<usize>,
    pub normal: Option<usize>,
    pub gloss: Option<usize>,
    pub color: [f32; 4],
    pub queue: i32,
    pub keywords: Vec<String>,
    pub floats: HashMap<String, f32>,
}

impl Props {
    /// A point in an object's own axes in the axes of the object read: its
    /// place, scale and turn under each object above it.
    pub fn in_root(&self, mut node: usize, mut point: [f32; 3]) -> [f32; 3] {
        loop {
            let own = &self.nodes[node];
            let Some(parent) = own.parent else { return point };
            let scaled = [point[0] * own.scale[0], point[1] * own.scale[1], point[2] * own.scale[2]];
            let turned = crate::level::level_light::rotate(own.rotation, scaled);
            point = [turned[0] + own.translation[0], turned[1] + own.translation[1], turned[2] + own.translation[2]];
            node = parent;
        }
    }

    /// The box every drawn corner lies in, in the axes of the object read
    /// (lowest and highest), and those of each object that draws.
    pub fn bounds(&self) -> ([f32; 3], [f32; 3]) {
        let mut low = [f32::MAX; 3];
        let mut high = [f32::MIN; 3];
        for (node, own) in self.nodes.iter().enumerate() {
            for &(mesh, _) in &own.drawn {
                for p in &self.meshes[mesh].positions {
                    let at = if own.in_root { *p } else { self.in_root(node, *p) };
                    for k in 0..3 {
                        low[k] = low[k].min(at[k]);
                        high[k] = high[k].max(at[k]);
                    }
                }
            }
        }
        (low, high)
    }
}
