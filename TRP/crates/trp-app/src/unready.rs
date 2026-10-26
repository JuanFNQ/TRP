//! A check, for a run told to make it (`TRP_LOG_UNREADY` in the
//! environment): what is meant to be drawn and cannot be, frame by frame.
//!
//! Bevy 0.18 prepares a changed material afresh and lets go of the one it
//! had first (`prepare_erased_assets`: "any users will not see the old asset
//! after a new asset is extracted, even if the new asset is not yet ready").
//! A material that names a picture not yet on the graphics card is not
//! ready (`AsBindGroupError::RetryNextUpdate`) and is tried again the next
//! frame; until then nothing drawn with it is drawn. The app sends pictures
//! up a few a frame (`main::warm_up`, `UPLOAD_BUDGET`), so a material
//! pointed at a picture just made, or a thing made with pictures just
//! read, could wait some frames to be drawn: the models that flashed out
//! as one walked up to them, and the suit that left the body for a moment
//! (the user's report of 2026-10-26; `docs/findings.md`, "Models that
//! flash").
//!
//! Written, with the frame:
//! - `UNREADY`: things drawn the frame before whose material is not ready
//!   now (a flash);
//! - `WAITING`: things new or still waiting whose material has never been
//!   ready, where their count changes (a thing late to show);
//! - `UNHEARD`: a material the renderer takes this frame that names a
//!   picture the stores have not yet told it of (made after they told it of
//!   this frame's, in `Last`).

use bevy::asset::UntypedAssetId;
use bevy::pbr::{PreparedMaterial, RenderMaterialInstances};
use bevy::platform::collections::{HashMap, HashSet};
use bevy::prelude::*;
use bevy::render::erased_render_asset::ErasedRenderAssets;
use bevy::render::sync_world::MainEntity;
use bevy::render::{Render, RenderApp, RenderSystems};

pub struct UnreadyPlugin;

impl Plugin for UnreadyPlugin {
    fn build(&self, app: &mut App) {
        if std::env::var_os("TRP_LOG_UNREADY").is_none() {
            return;
        }
        info!("unready: written (TRP_LOG_UNREADY)");
        app.add_systems(Last, unheard.after(crate::streaming::stream));
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.add_systems(Render, count_unready.in_set(RenderSystems::PrepareResources));
        }
    }
}

/// The materials the renderer takes this frame (each one the stores told
/// it of), and any picture one names that the renderer has not been told
/// of.
fn unheard(
    frame: Res<bevy::diagnostic::FrameCount>,
    images: Res<Assets<Image>>,
    materials: Res<Assets<StandardMaterial>>,
    mut image_events: MessageReader<AssetEvent<Image>>,
    mut material_events: MessageReader<AssetEvent<StandardMaterial>>,
    mut heard: Local<HashSet<AssetId<Image>>>,
) {
    for event in image_events.read() {
        match *event {
            AssetEvent::Added { id } => {
                heard.insert(id);
            }
            AssetEvent::Removed { id } => {
                heard.remove(&id);
            }
            _ => {}
        }
    }
    for event in material_events.read() {
        let (AssetEvent::Added { id } | AssetEvent::Modified { id }) = *event else { continue };
        let Some(material) = materials.get(id) else { continue };
        for slot in [&material.base_color_texture, &material.normal_map_texture, &material.metallic_roughness_texture, &material.occlusion_texture, &material.emissive_texture].into_iter().flatten() {
            if !heard.contains(&slot.id()) {
                info!("UNHEARD on frame {}: material {id:?} names picture {:?}, {}", frame.0, slot.id(), if images.contains(slot.id()) { "in the store, not yet told of" } else { "not in the store" });
            }
        }
    }
}

/// What was ready the frame before (each thing with its material), and
/// how many were waiting.
#[derive(Default)]
struct Seen {
    ready: HashMap<MainEntity, UntypedAssetId>,
    waiting: usize,
}

/// Each frame, the things drawn with a material whose material was ready
/// the frame before and is not now, and those whose material has never
/// been.
fn count_unready(instances: Res<RenderMaterialInstances>, prepared: Res<ErasedRenderAssets<PreparedMaterial>>, frame: Option<Res<bevy::diagnostic::FrameCount>>, mut seen: Local<Seen>) {
    let frame = frame.map_or(0, |f| f.0);
    let mut ready = HashMap::with_capacity(seen.ready.len());
    let (mut lost, mut waiting) = (Vec::new(), 0usize);
    for (entity, instance) in &instances.instances {
        if prepared.get(instance.asset_id).is_some() {
            ready.insert(*entity, instance.asset_id);
        } else if seen.ready.contains_key(entity) {
            lost.push((*entity, instance.asset_id));
        } else {
            waiting += 1;
        }
    }
    if !lost.is_empty() {
        let some = lost.iter().take(4).map(|(e, id)| format!("{:?} {id:?}", e.id())).collect::<Vec<_>>().join("; ");
        info!("UNREADY on frame {frame}: {} things drawn the frame before have no material ready ({some})", lost.len());
    }
    if waiting != seen.waiting {
        info!("WAITING on frame {frame}: {waiting} things whose material has never been ready");
    }
    *seen = Seen { ready, waiting };
}
