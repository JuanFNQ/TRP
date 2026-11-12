//! An outfit that is not the install's: a body of S.T.A.L.K.E.R.'s, moved by the install's skeleton.
//! It is worn in place of the character's own (`docs/spec/outfits.md`).
//!
//! Such an outfit is described by a file in the workspace's
//! `custom/outfits` folder. It becomes an item of the database (its
//! template that of one of the install's armours, under a name of its
//! own), and its model is read from the player's own install of the other
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

use crate::setup::database::{Database, ItemTemplate};
use crate::unity::extract::MeshData;
use crate::formats::xray::{self, Picture, Stalker};

mod rest_pose;
mod outfit_build;
mod arm_laying;
mod arm_fit;
mod hands;
mod fitting;

pub use rest_pose::*;
pub use outfit_build::*;
pub use arm_fit::*;
pub use hands::*;
pub use fitting::*;
use rest_pose::{add, arc, bend, cross, dot, length, off_line, scaled, sub, turns, unit};
use arm_laying::{ArmNames, Basis, Hang, Laid, add_bone, lay_arm};
use arm_fit::{lay_meshes};
use hands::lay_arms;
use fitting::{Goes, theirs};

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
    crate::weapons::custom::folder().map(|custom| custom.join("outfits")).filter(|folder| folder.is_dir())
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

#[cfg(test)]
mod tests;
