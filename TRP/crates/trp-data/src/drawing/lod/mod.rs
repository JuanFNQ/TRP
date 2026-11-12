//! A model's levels of detail: which of its renderers are drawn at what share of the screen's height.
//! Unity's `LODGroup`, class 205, read as the install saves them (third-person spec, "Levels of
//! detail").
//!
//! A group lists levels, the fullest first, each with the least height
//! (`screenRelativeHeight`, a share of the view's height) at which it is
//! still drawn and the renderers it draws. The engine's pick
//! (`LODGroupManager::CalculateLODMask`, read with Ghidra)
//! is `trp_sim::lod`'s; this is the data. A renderer may be listed at
//! several levels; one listed at none is no group's and always drawn.
//!
//! **TRP's own measure**, a pass of its own for the frame's cost
//! (third-person spec, "TRP's own rule"): each coarser level's *deviation*, the largest
//! distance between its surface and the fullest level's, both ways
//! (`deviation`), measured once from the install's meshes as a group is
//! read for a picker (`measured`, kept for the run), so that a level can
//! be drawn wherever that distance comes to less than a pixel.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use anyhow::{anyhow, Result};

use crate::unity::{class, Assets, ObjRef, Value};

mod shape_measure;
mod nearest;
mod shape_sources;

pub use shape_measure::*;
pub use nearest::*;
pub use shape_sources::*;
use nearest::{Grid};

/// One `LODGroup` as saved.
#[derive(Debug, Clone, PartialEq)]
pub struct LodGroup {
    /// Its own object (the `LODGroup` component), where read from a bundle:
    /// what its measure is kept by (`measured`).
    pub id: Option<ObjRef>,
    /// The game object it is on, whose transform places it.
    pub object: Option<ObjRef>,
    /// Its size (`m_Size`): times the object's largest scale, the world
    /// size the heights are reckoned by.
    pub size: f32,
    /// The point the camera's distance is taken to (`m_LocalReferencePoint`),
    /// in its object's own axes (the game's).
    pub reference: [f32; 3],
    /// How a switch is made (`m_FadeMode`): 0 at once, 1 cross-faded,
    /// 2 SpeedTree's.
    pub fade_mode: i64,
    /// Whether a cross-fade is played over time (`m_AnimateCrossFading`).
    pub animate_cross_fading: bool,
    pub levels: Vec<LodLevel>,
    /// TRP's measure: each level's deviation from the fullest (`deviation`),
    /// metres in its object's own axes (before that object's scale), the
    /// fullest's 0, never less than a finer level's; empty where not
    /// measured (read for no picker, or its meshes unread), which a picker
    /// takes as the fullest level at any distance.
    pub deviation: Vec<f32>,
}

/// One level of a group (`LOD`).
#[derive(Debug, Clone, PartialEq)]
pub struct LodLevel {
    /// The least share of the view's height at which it is drawn
    /// (`screenRelativeHeight`).
    pub height: f32,
    /// The share of the way to the next level over which it cross-fades
    /// (`fadeTransitionWidth`), with a fade mode that does.
    pub fade_width: f32,
    pub renderers: Vec<ObjRef>,
}

impl LodGroup {
    /// Each level's least height, the fullest first.
    pub fn heights(&self) -> Vec<f32> {
        self.levels.iter().map(|level| level.height).collect()
    }

    /// Whether no level lists a renderer: such a group governs nothing (an
    /// armband's `Skin` group), and its things are drawn at any distance.
    pub fn governs_nothing(&self) -> bool {
        self.levels.iter().all(|level| level.renderers.is_empty())
    }

    /// The levels a renderer is drawn at, a bit each (the first level the
    /// lowest bit; the engine's own masks have eight); 0 for one no level
    /// lists.
    pub fn levels_of(&self, renderer: ObjRef) -> u8 {
        self.levels.iter().take(8).enumerate().filter(|(_, level)| level.renderers.contains(&renderer)).fold(0, |bits, (at, _)| bits | 1 << at)
    }

    /// Whether it lists the renderer at any level.
    pub fn lists(&self, renderer: ObjRef) -> bool {
        self.levels_of(renderer) != 0
    }
}

/// Reads one group.
pub fn read(assets: &Assets, group: ObjRef) -> Result<LodGroup> {
    let value = assets.read(group)?;
    let object = value.get("m_GameObject").and_then(Value::as_pptr).and_then(|p| assets.resolve_loaded(group.file, p));
    let size = value.req("m_Size")?.as_f32().ok_or_else(|| anyhow!("a group's size is not a number"))?;
    let reference = value.req("m_LocalReferencePoint")?.as_vec3().ok_or_else(|| anyhow!("a group's point is not a vector"))?;
    let fade_mode = value.get("m_FadeMode").and_then(Value::as_i64).unwrap_or(0);
    let animate_cross_fading = value.get("m_AnimateCrossFading").and_then(|v| v.as_bool().or_else(|| v.as_i64().map(|n| n != 0))).unwrap_or(false);
    let mut levels = Vec::new();
    for lod in value.req("m_LODs")?.as_array().unwrap_or(&[]) {
        let height = lod.get("screenRelativeHeight").and_then(Value::as_f32).ok_or_else(|| anyhow!("a level's height is not a number"))?;
        let fade_width = lod.get("fadeTransitionWidth").and_then(Value::as_f32).unwrap_or(0.0);
        let renderers = lod
            .at("renderers")
            .and_then(Value::as_array)
            .unwrap_or(&[])
            .iter()
            .filter_map(|r| r.get("renderer")?.as_pptr())
            .filter_map(|p| assets.resolve_loaded(group.file, p))
            .collect();
        levels.push(LodLevel { height, fade_width, renderers });
    }
    Ok(LodGroup { id: Some(group), object, size, reference, fade_mode, animate_cross_fading, levels, deviation: Vec::new() })
}

/// Every group in the files, those that govern nothing left out; one that
/// cannot be read is left out too.
pub fn of_files(assets: &Assets, files: &[usize]) -> Vec<Arc<LodGroup>> {
    files.iter().flat_map(|&file| assets.objects_of(file, class::LOD_GROUP)).filter_map(|group| read(assets, group).ok()).filter(|group| !group.governs_nothing()).map(Arc::new).collect()
}

/// A skin's place among its part's levels of detail (`arms::load_levels`,
/// `dress::load_levels`): its bundle's group, and the levels it is drawn
/// at, a bit each.
#[derive(Debug, Clone, PartialEq)]
pub struct SkinLod {
    pub group: Arc<LodGroup>,
    pub levels: u8,
}

#[cfg(test)]
mod tests;
