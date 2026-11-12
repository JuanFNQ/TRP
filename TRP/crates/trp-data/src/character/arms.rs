//! First-person arms: a skinned mesh that names its bones by path.
//!
//! First-person arms: a skinned mesh whose bundle carries no skeleton, only
//! the paths of the bones it expects to find on whatever it is attached to.

use std::sync::Arc;

use anyhow::{anyhow, Result};

use crate::drawing::lod::SkinLod;
use crate::unity::extract::{read_material, script_class, MaterialData};
use crate::unity::{class, Assets, ObjRef, Value};

#[derive(Debug, Clone)]
pub struct Arms {
    pub mesh: ObjRef,
    pub materials: Vec<Option<ObjRef>>,
    /// Beside `materials`: whether each is a see-through glass's (`is_glass`),
    /// as the hands' watch has over its face.
    pub glass: Vec<bool>,
    /// The bundle's other skins, drawn with this one (`load`): a hands
    /// bundle's watch case and its glass, where they are skins of their own.
    pub beside: Vec<Arms>,
    /// One path per bone, in bind pose order, from the body's root bone.
    pub bone_paths: Vec<String>,
    /// A torso of the newer kind (`TorsoSkin`): its three meshes, bare,
    /// under armour and under a vest, of which the game draws one by what
    /// is worn (inventory spec, "The OVERALL tab"); `mesh` is the bare one.
    pub torso: Option<TorsoMeshes>,
    /// Its renderer (`_skinnedMeshRenderer`), by which a group of levels of
    /// detail lists it.
    pub renderer: ObjRef,
    /// Where it is read as one of a part's levels of detail
    /// (`load_levels`, `dress::load_levels`): its bundle's group and the
    /// levels it is drawn at. None where read alone (`load`), drawn at any
    /// distance.
    pub lod: Option<SkinLod>,
    /// A head's mesh under a face cover (`HeadSkin`'s `Data.FaceCover`;
    /// body animator spec, "The head's own mesh under a face cover"), where
    /// the skin is a head's and names one: the game draws it in place of
    /// `mesh` (its `Data.Base`) while the face cover's slot holds a face
    /// cover that is not a half mask.
    pub covered: Option<ObjRef>,
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
/// The holster a pair of legs carries (`LegsView`): on the right leg or the
/// left, and its object's own place and turn, which the game gives the
/// player's holster bone for that leg (`PlayerBones.AdjustPistolHolster`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LegsHolster {
    pub right: bool,
    pub position: [f32; 3],
    pub rotation: [f32; 4],
}

/// The legs' holster, where the bundle's legs carry one; none where they
/// have no `LegsView` (no pistol is shown on such legs: `PlayerBody.
/// HaveHolster`).
pub fn legs_holster(assets: &mut Assets, bundle_key: &str) -> Result<Option<LegsHolster>> {
    let files = assets.load(bundle_key)?;
    let Some(view) = files.iter().flat_map(|&f| assets.objects_of(f, class::MONO_BEHAVIOUR)).find(|&obj| script_class(assets, obj).as_deref() == Some("LegsView")) else {
        return Ok(None);
    };
    let fields = assets.read(view)?;
    let right = fields.get("_isRightLeg").and_then(|v| v.as_bool().or_else(|| v.as_i64().map(|n| n != 0))).unwrap_or(true);
    let holster = fields.get("_holster").and_then(Value::as_pptr).and_then(|p| assets.resolve_loaded(view.file, p)).ok_or_else(|| anyhow!("{bundle_key}: its legs' holster names no object"))?;
    let transform = assets.read(holster)?;
    let position = transform.get("m_LocalPosition").and_then(Value::as_vec3).ok_or_else(|| anyhow!("{bundle_key}: the holster's place"))?;
    let rotation = transform.get("m_LocalRotation").and_then(Value::as_vec4).ok_or_else(|| anyhow!("{bundle_key}: the holster's turn"))?;
    Ok(Some(LegsHolster { right, position, rotation }))
}

/// A `LoddedSkin` lists skins (`_lods`, once `Skins`), all of which the
/// game sets up and switches on and off together (`LoddedSkin.Init`,
/// `EnableRenderers`). With a `LODGroup` on the bundle, as clothing and
/// heads have, they are levels of detail, the fullest first, and that one
/// is taken. Without one, as on hands, each is drawn: the first is taken
/// and the rest go `beside` it (a watch's case and its glass on four hands
/// bundles; one that cannot be read is left out).
pub fn load(assets: &mut Assets, bundle_key: &str) -> Result<Arms> {
    let files = assets.load(bundle_key)?;
    let scripts: Vec<ObjRef> = files.iter().flat_map(|&f| assets.objects_of(f, class::MONO_BEHAVIOUR)).collect();
    let of_class = |name: &str| scripts.iter().copied().find(|&obj| script_class(assets, obj).as_deref() == Some(name));
    let listed: Vec<ObjRef> = match of_class("LoddedSkin") {
        Some(levels) => {
            let listed = assets.read(levels)?;
            listed.req("_lods")?.as_array().unwrap_or(&[]).iter().filter_map(|skin| skin.as_pptr().and_then(|p| assets.resolve_loaded(levels.file, p))).collect()
        }
        None => Vec::new(),
    };
    let skin_obj = listed.first().copied().or_else(|| of_class("Skin")).ok_or_else(|| anyhow!("{bundle_key} has no Skin component"))?;
    let mut arms = read_skin(assets, skin_obj)?;
    let levels_of_detail = files.iter().any(|&file| assets.objects_of(file, class::LOD_GROUP).next().is_some());
    if !levels_of_detail {
        arms.beside = listed.iter().skip(1).filter_map(|&skin| read_skin(assets, skin).ok()).collect();
    }
    Ok(arms)
}

/// Every level of detail of a part (a body, its legs, a head; third-person
/// spec, "Levels of detail"): the skins its `LoddedSkin` lists, each with
/// the levels the bundle's group draws its renderer at (`Arms::lod`), the
/// fullest first, as `load` reads it. A skin no level lists is left out,
/// and so is a hooded form (`HoodedDress._hooded`: a second form of the
/// level it shares, `dress::hood_levels`). A bundle without a group (the
/// hands') or whose group governs none of them is `load`'s alone, with no
/// levels.
pub fn load_levels(assets: &mut Assets, bundle_key: &str) -> Result<Vec<Arms>> {
    let fullest = load(assets, bundle_key)?;
    let files = assets.load(bundle_key)?;
    let scripts: Vec<ObjRef> = files.iter().flat_map(|&f| assets.objects_of(f, class::MONO_BEHAVIOUR)).collect();
    let of_class = |name: &str| scripts.iter().copied().find(|&obj| script_class(assets, obj).as_deref() == Some(name));
    let Some(levels) = of_class("LoddedSkin") else { return Ok(vec![fullest]) };
    let listed: Vec<ObjRef> = assets.read(levels)?.req("_lods")?.as_array().unwrap_or(&[]).iter().filter_map(|skin| skin.as_pptr().and_then(|p| assets.resolve_loaded(levels.file, p))).collect();
    let Some(group) = crate::drawing::lod::of_files(assets, &files).into_iter().find(|group| group.lists(fullest.renderer)) else { return Ok(vec![fullest]) };
    // A hood's forms (`HoodedDress._hooded`), by their objects.
    let hooded: Vec<ObjRef> = of_class("HoodedDress")
        .and_then(|dress| {
            let value = assets.read(dress).ok()?;
            Some(value.get("_hooded")?.as_array()?.iter().filter_map(|v| v.as_pptr().and_then(|p| assets.resolve_loaded(dress.file, p))).collect())
        })
        .unwrap_or_default();
    let object_of = |component: ObjRef| assets.read(component).ok().and_then(|value| value.get("m_GameObject").and_then(Value::as_pptr).and_then(|p| assets.resolve_loaded(component.file, p)));
    let mut all = Vec::new();
    for (at, &skin) in listed.iter().enumerate() {
        if object_of(skin).is_some_and(|object| hooded.contains(&object)) {
            continue;
        }
        let mut level = if at == 0 { fullest.clone() } else { read_skin(assets, skin)? };
        let bits = group.levels_of(level.renderer);
        if bits == 0 {
            continue;
        }
        level.lod = Some(SkinLod { group: group.clone(), levels: bits });
        all.push(level);
    }
    if all.is_empty() {
        return Ok(vec![fullest]);
    }
    // TRP's measure of its levels (`lod::measured`), by the meshes each
    // draws in their own axes.
    let meshes: Vec<(ObjRef, u8)> = all.iter().filter_map(|level| Some((level.mesh, level.lod.as_ref()?.levels))).collect();
    let group = Arc::new(crate::drawing::lod::measured(Some(&*assets), &group, || crate::drawing::lod::skin_shapes(assets, group.levels.len(), &meshes)));
    for level in &mut all {
        if let Some(lod) = &mut level.lod {
            lod.group = group.clone();
        }
    }
    Ok(all)
}

/// Whether a material is the game's see-through reflective glass
/// (the game's `Glass` shader; body animator spec, "The watch on the wrist"): that
/// shader's figures (`_GlobalReflectionStrength`, `_FresPow`, `_FresAdd`,
/// `_Specular`) and a picture of its own (`_MainTex`; without one it is
/// a red dot's window, `sight::lens_shader`), drawn after the opaque
/// (queue 3000 on). `PROVISIONAL` as the window's test: by its properties,
/// the shaders' bundle not opened. Not drawn: its reflections are not built.
pub fn is_glass(data: &MaterialData) -> bool {
    data.queue >= 3000 && data.textures.contains_key("_MainTex") && ["_GlobalReflectionStrength", "_FresPow", "_FresAdd", "_Specular"].iter().all(|name| data.floats.contains_key(*name))
}

/// The mesh, materials and bone paths of one `Skin` script, or of a script
/// that names the `Skin` beside it.
pub fn read_skin(assets: &Assets, skin_obj: ObjRef) -> Result<Arms> {
    read_skin_with(assets, skin_obj, None)
}

/// The same with the mesh given in place of the renderer's, which may then
/// save none (an armband's skin: `trp_data::dress::ArmBand`).
pub fn read_skin_with(assets: &Assets, mut skin_obj: ObjRef, given: Option<ObjRef>) -> Result<Arms> {
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
    // A head's two meshes (`HeadSkinData`): `Base`, which the game sets on
    // the renderer as the head is put on (where named), and `FaceCover`.
    let mut head = (None, None);
    if let Some(inner) = skin.get("Skin").and_then(Value::as_pptr).and_then(|p| assets.resolve_loaded(skin_obj.file, p)) {
        if script_class(assets, skin_obj).as_deref() == Some("HeadSkin") {
            let mesh = |name: &str| skin.get("Data").and_then(|data| data.get(name)).and_then(Value::as_pptr).and_then(|p| assets.resolve_loaded(skin_obj.file, p));
            head = (mesh("Base"), mesh("FaceCover"));
        }
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
    let mesh = match (torso, given) {
        (Some(torso), _) => torso.base,
        (None, Some(given)) => given,
        (None, None) => match head.0 {
            Some(base) => base,
            None => resolve(renderer.req("m_Mesh")?).ok_or_else(|| anyhow!("arms renderer has no mesh"))?,
        },
    };
    let materials: Vec<Option<ObjRef>> = renderer.req("m_Materials")?.as_array().unwrap_or(&[]).iter().map(resolve).collect();
    let glass = materials.iter().map(|material| material.and_then(|material| read_material(assets, material).ok()).is_some_and(|data| is_glass(&data))).collect();
    Ok(Arms {
        torso,
        mesh,
        materials,
        glass,
        renderer: renderer_obj,
        lod: None,
        covered: head.1.filter(|covered| *covered != mesh),
        beside: Vec::new(),
        bone_paths: skin
            .req("_bonePaths")?
            .as_array()
            .unwrap_or(&[])
            .iter()
            .filter_map(|p| p.as_str().map(str::to_owned))
            .collect(),
    })
}
