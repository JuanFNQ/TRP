//! First-person arms: a skinned mesh whose bundle carries no skeleton, only
//! the paths of the bones it expects to find on whatever it is attached to.

use anyhow::{anyhow, Result};

use crate::unity::extract::script_class;
use crate::unity::{class, Assets, ObjRef, Value};

pub struct Arms {
    pub mesh: ObjRef,
    pub materials: Vec<Option<ObjRef>>,
    /// One path per bone, in bind pose order, from the body's root bone.
    pub bone_paths: Vec<String>,
    /// A torso of the newer kind (`TorsoSkin`): its three meshes, bare,
    /// under armour and under a vest, of which the game draws one by what
    /// is worn (inventory spec, "The OVERALL tab"); `mesh` is the bare one.
    pub torso: Option<TorsoMeshes>,
}

/// A torso's three meshes (`TorsoSkin._base`, `_armor`, `_vest`).
#[derive(Debug, Clone, Copy)]
pub struct TorsoMeshes {
    pub base: ObjRef,
    pub armor: Option<ObjRef>,
    pub vest: Option<ObjRef>,
}

impl TorsoMeshes {
    /// Which of them the game draws (`TorsoSkin.ApplySkin`): the armour's
    /// where armour that is not vest-like is worn; the vest's where
    /// vest-like armour, a tactical vest or a backpack is; else the bare
    /// one.
    pub fn drawn(armour: bool, armour_vestlike: bool, vest: bool, backpack: bool) -> Torso {
        if armour && !armour_vestlike {
            Torso::Armour
        } else if (armour && armour_vestlike) || vest || backpack {
            Torso::Vest
        } else {
            Torso::Base
        }
    }

    /// The mesh drawn so, if the bundle has it: one it leaves empty draws
    /// nothing, as the engine's renderer with no mesh does.
    pub fn mesh(&self, torso: Torso) -> Option<ObjRef> {
        match torso {
            Torso::Base => Some(self.base),
            Torso::Armour => self.armor,
            Torso::Vest => self.vest,
        }
    }
}

/// Which of a torso's meshes is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Torso {
    Base,
    Armour,
    Vest,
}

impl Torso {
    pub const ALL: [Torso; 3] = [Torso::Base, Torso::Armour, Torso::Vest];
}

impl Arms {
    /// Bone paths as the weapon prefab names them. Its arm chains start at
    /// the collarbones, so everything up to the ribcage is dropped.
    pub fn weapon_bone_paths(&self) -> Vec<&str> {
        const TORSO: &str = "Base HumanRibcage/";
        self.bone_paths
            .iter()
            .map(|p| p.find(TORSO).map_or(p.as_str(), |at| &p[at + TORSO.len()..]))
            .collect()
    }
}

/// The skinned mesh of a part of the character: the arms, or a piece of
/// clothing (a body or a legs bundle is built the same way). Where the
/// bundle has the part at several levels of detail (a `LoddedSkin` listing
/// one `Skin` for each, the fullest first), the fullest is the one taken.
pub fn load(assets: &mut Assets, bundle_key: &str) -> Result<Arms> {
    let files = assets.load(bundle_key)?;
    let scripts: Vec<ObjRef> = files.iter().flat_map(|&f| assets.objects_of(f, class::MONO_BEHAVIOUR)).collect();
    let of_class = |name: &str| scripts.iter().copied().find(|&obj| script_class(assets, obj).as_deref() == Some(name));
    let fullest = match of_class("LoddedSkin") {
        Some(levels) => {
            let listed = assets.read(levels)?;
            let first = listed.req("_lods")?.as_array().and_then(|lods| lods.first());
            first.and_then(Value::as_pptr).and_then(|p| assets.resolve_loaded(levels.file, p))
        }
        None => None,
    };
    let skin_obj = fullest.or_else(|| of_class("Skin")).ok_or_else(|| anyhow!("{bundle_key} has no Skin component"))?;
    read_skin(assets, skin_obj)
}

/// The mesh, materials and bone paths of one `Skin` script, or of a script
/// that names the `Skin` beside it.
pub fn read_skin(assets: &Assets, mut skin_obj: ObjRef) -> Result<Arms> {
    let mut skin = assets.read(skin_obj)?;
    // A head's levels of detail are listed as `HeadSkin` scripts, each of
    // which names the `Skin` beside it; a newer torso's as `TorsoSkin`
    // scripts, each naming its `Skin` and its three meshes.
    let mut torso = None;
    if let Some(inner) = skin.get("_skin").and_then(Value::as_pptr).and_then(|p| assets.resolve_loaded(skin_obj.file, p)) {
        let mesh = |name: &str| skin.get(name).and_then(Value::as_pptr).and_then(|p| assets.resolve_loaded(skin_obj.file, p));
        if let Some(base) = mesh("_base") {
            torso = Some(TorsoMeshes { base, armor: mesh("_armor"), vest: mesh("_vest") });
        }
        skin_obj = inner;
        skin = assets.read(skin_obj)?;
    }
    if let Some(inner) = skin.get("Skin").and_then(Value::as_pptr).and_then(|p| assets.resolve_loaded(skin_obj.file, p)) {
        skin_obj = inner;
        skin = assets.read(skin_obj)?;
    }
    let renderer_obj = skin
        .req("_skinnedMeshRenderer")?
        .as_pptr()
        .and_then(|p| assets.resolve_loaded(skin_obj.file, p))
        .ok_or_else(|| anyhow!("Skin has no renderer"))?;
    let renderer = assets.read(renderer_obj)?;
    let resolve = |v: &Value| v.as_pptr().and_then(|p| assets.resolve_loaded(renderer_obj.file, p));
    let mesh = match torso {
        Some(torso) => torso.base,
        None => resolve(renderer.req("m_Mesh")?).ok_or_else(|| anyhow!("arms renderer has no mesh"))?,
    };
    Ok(Arms {
        torso,
        mesh,
        materials: renderer.req("m_Materials")?.as_array().unwrap_or(&[]).iter().map(resolve).collect(),
        bone_paths: skin
            .req("_bonePaths")?
            .as_array()
            .unwrap_or(&[])
            .iter()
            .filter_map(|p| p.as_str().map(str::to_owned))
            .collect(),
    })
}
