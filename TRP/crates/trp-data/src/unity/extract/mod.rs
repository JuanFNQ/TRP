//! Typed views of the objects this project needs: meshes, textures,
//! materials and transform hierarchies. All values stay in Unity's
//! left-handed space; converting is the renderer's job.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use anyhow::{anyhow, bail, Context, Result};

use super::assets::{Assets, ObjRef, ResourcePlace};
use super::class;
use super::value::Value;

mod meshes;
mod textures;
mod hierarchy;
mod colliders;

pub use meshes::*;
pub use textures::*;
pub use hierarchy::*;
pub use colliders::*;
use colliders::{BOX_COLLIDER, CAPSULE_COLLIDER, MESH_COLLIDER, SPHERE_COLLIDER, node_collider};

#[cfg(test)]
mod tests;

pub(crate) fn components(assets: &Assets, owner: ObjRef, go: &Value) -> Vec<ObjRef> {
    go.get("m_Component")
        .and_then(Value::as_array)
        .unwrap_or(&[])
        .iter()
        .filter_map(|c| c.get("component")?.as_pptr())
        .filter_map(|p| assets.resolve_loaded(owner.file, p))
        .collect()
}

/// A renderer set to cast its shadow and draw nothing (`m_CastShadows` 3).
fn shadows_only(renderer: &Value) -> bool {
    renderer.get("m_CastShadows").and_then(Value::as_i64) == Some(3)
}

fn materials(assets: &Assets, comp: ObjRef, renderer: &Value) -> Result<Vec<Option<ObjRef>>> {
    Ok(renderer
        .req("m_Materials")?
        .as_array()
        .unwrap_or(&[])
        .iter()
        .map(|m| m.as_pptr().and_then(|p| assets.resolve_loaded(comp.file, p)))
        .collect())
}

/// Class name of the script a MonoBehaviour instantiates.
pub fn script_class(assets: &Assets, behaviour: ObjRef) -> Option<String> {
    let v = assets.read(behaviour).ok()?;
    let script = assets.resolve_loaded(behaviour.file, v.get("m_Script")?.as_pptr()?)?;
    let script = assets.read(script).ok()?;
    script.get("m_ClassName")?.as_str().map(str::to_owned)
}

/// The prefab roots a bundle exports, read from its asset table.
pub fn exported_game_objects(assets: &Assets, file: usize) -> Result<Vec<(String, ObjRef)>> {
    let mut out = Vec::new();
    for bundle in assets.objects_of(file, class::ASSET_BUNDLE) {
        let v = assets.read(bundle)?;
        for pair in v.req("m_Container")?.as_array().unwrap_or(&[]) {
            let path = pair.req("first")?.as_str().unwrap_or("").to_owned();
            let Some(obj) = pair.req("second.asset")?.as_pptr().and_then(|p| assets.resolve_loaded(file, p))
            else {
                continue;
            };
            if assets.class_id(obj) == Some(class::GAME_OBJECT) {
                out.push((path, obj));
            }
        }
    }
    Ok(out)
}
