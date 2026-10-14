//! What a character wears over its clothing: armour, vests, backpacks,
//! helmets, headsets. An item's bundle is one of two kinds
//! (`docs/spec/body-animator.md`, "Worn items"): a skin for the player's
//! skeleton, built like a piece of clothing, or a plain model that is hung
//! on the head.

use anyhow::{anyhow, Result};

use crate::arms::{self, Arms};
use crate::unity::extract::{exported_game_objects, read_hierarchy, script_class, Hierarchy};
use crate::unity::{class, Assets, ObjRef, Value};

/// What else is worn, which some skins choose their mesh by: a backpack
/// sits further out over a vest, a vest over armour.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Under {
    pub armour: bool,
    pub vest: bool,
}

pub enum Worn {
    /// Skinned to the bones it names by path, like clothing.
    Skinned(Arms),
    /// A model of its own, for the head bone.
    Rigid(Hierarchy),
}

/// Reads the worn form of the item in a bundle.
pub fn load(assets: &mut Assets, bundle_key: &str, under: Under) -> Result<Worn> {
    let files = assets.load(bundle_key)?;
    let scripts: Vec<ObjRef> = files.iter().flat_map(|&f| assets.objects_of(f, class::MONO_BEHAVIOUR)).collect();
    let of_class = |name: &str| scripts.iter().copied().find(|&obj| script_class(assets, obj).as_deref() == Some(name));
    let Some(dress) = of_class("SkinDress") else {
        // No skin: the bundle's own object is the model.
        let root = files
            .iter()
            .find_map(|&file| exported_game_objects(assets, file).ok()?.into_iter().next())
            .ok_or_else(|| anyhow!("{bundle_key} exports no object"))?;
        return Ok(Worn::Rigid(read_hierarchy(assets, root.1)?));
    };
    // The skins are listed by level of detail, the fullest first.
    let listed = assets.read(dress)?;
    let fullest = listed
        .req("_lods")?
        .as_array()
        .and_then(|lods| lods.first())
        .and_then(Value::as_pptr)
        .and_then(|p| assets.resolve_loaded(dress.file, p))
        .ok_or_else(|| anyhow!("{bundle_key}: the dress lists no skin"))?;
    let entry = assets.read(fullest)?;
    let pointed = |v: Option<&Value>| v.and_then(Value::as_pptr).and_then(|p| assets.resolve_loaded(fullest.file, p));
    // A plain skin, or one of the kinds that hold a skin and a mesh for
    // each thing it may be worn over.
    let (skin, mesh) = match script_class(assets, fullest).as_deref() {
        Some("VestSkin") => {
            let data = entry.req("Data")?;
            (pointed(entry.get("Skin")), pointed(data.get(if under.armour { "Armor" } else { "Body" })))
        }
        Some("BackpackSkin") => {
            let data = entry.req("Data")?;
            let name = match (under.armour, under.vest) {
                (true, true) => "ArmorVest",
                (true, false) => "Armor",
                (false, true) => "Vest",
                (false, false) => "Body",
            };
            (pointed(entry.get("Skin")), pointed(data.get(name)))
        }
        Some("TorsoSkin") => {
            let name = if under.armour { "_armor" } else if under.vest { "_vest" } else { "_base" };
            (pointed(entry.get("_skin")), pointed(entry.get(name)))
        }
        _ => (Some(fullest), None),
    };
    let skin = skin.ok_or_else(|| anyhow!("{bundle_key}: the dress's skin is missing"))?;
    let mut part = arms::read_skin(assets, skin)?;
    // A mesh left empty for one case falls back on the renderer's own.
    if let Some(mesh) = mesh {
        part.mesh = mesh;
    }
    Ok(Worn::Skinned(part))
}
