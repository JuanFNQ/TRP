//! Where the renderer's time goes in a slow frame: a tool, switched on by
//! `TRP_RENDER_TIMING` in the environment (its value, if a number, the
//! milliseconds a frame must pass to be printed). The clock is read at the
//! boundaries of the renderer's schedule (Bevy's `RenderSet`s, as
//! `Render::base_schedule` orders and nests them), and a frame of the
//! renderer longer than `SLOW_MS` is printed stage by stage, with how many
//! pipelines the renderer is still building. Nothing of the simulation or
//! of the drawing is changed by it.

use std::time::Instant;

use bevy::prelude::*;
use bevy::render::render_resource::PipelineCache;
use bevy::render::RenderSystems;

/// A frame longer than this is printed, in milliseconds, unless the
/// variable says another.
const SLOW_MS: f32 = 20.0;

/// A system's own clock, with the tool on: held for the system's run
/// (`let _clock = Clock::new("name")`), it prints the run if it took a
/// millisecond or more.
pub struct Clock {
    name: &'static str,
    began: Option<Instant>,
}

impl Clock {
    pub fn new(name: &'static str) -> Self {
        static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        let on = *ON.get_or_init(|| std::env::var_os("TRP_RENDER_TIMING").is_some());
        Self { name, began: on.then(Instant::now) }
    }
}

impl Drop for Clock {
    fn drop(&mut self) {
        if let Some(began) = self.began {
            let ms = began.elapsed().as_secs_f32() * 1000.0;
            if ms >= 1.0 {
                info!("system {}: {ms:.1} ms", self.name);
            }
        }
    }
}

fn slow_ms() -> f32 {
    std::env::var("TRP_RENDER_TIMING").ok().and_then(|value| value.parse().ok()).unwrap_or(SLOW_MS)
}

/// The stages between one reading and the next, in their order.
const STAGES: [&str; 12] = [
    "ExtractCommands",
    "PrepareAssets",
    "PrepareMeshes",
    "ManageViews",
    "Queue",
    "PhaseSort",
    "Prepare: resources",
    "Prepare: phase buffers and flush",
    "Prepare: bind groups",
    "Render",
    "Cleanup",
    "PostCleanup",
];

/// The clock at each reading this frame, and the renderer's frames so far.
#[derive(Resource, Default)]
struct Stamps {
    at: [Option<Instant>; STAGES.len() + 1],
    frame: u64,
    slow: f32,
}

fn stamp<const N: usize>(mut stamps: ResMut<Stamps>) {
    stamps.at[N] = Some(Instant::now());
}

/// Prints the frame if it was slow, and starts the next.
fn report(mut stamps: ResMut<Stamps>, cache: Res<PipelineCache>) {
    stamps.frame += 1;
    let now = Instant::now();
    if let Some(first) = stamps.at[0] {
        let whole = now.duration_since(first).as_secs_f32() * 1000.0;
        if whole > stamps.slow {
            let mut parts = Vec::new();
            for (index, name) in STAGES.iter().enumerate() {
                let (Some(from), Some(to)) = (stamps.at[index], stamps.at[index + 1]) else { continue };
                let ms = to.duration_since(from).as_secs_f32() * 1000.0;
                if ms >= 1.0 {
                    parts.push(format!("{name} {ms:.1}"));
                }
            }
            info!("render frame {}: {whole:.1} ms; {}; pipelines being built {}", stamps.frame, parts.join(", "), cache.waiting_pipelines().count());
        }
    }
    stamps.at = Default::default();
}

/// The main world's side: when its last schedule ended, and when its first
/// began, this frame. Between the two lie the handing over of what the
/// renderer draws (the extraction, Bevy's `ExtractSchedule`) and anything
/// the main thread waits for there.
#[derive(Resource, Default)]
struct MainStamps {
    first: Option<Instant>,
    last: Option<Instant>,
    frame: u64,
    slow: f32,
}

fn main_first(mut stamps: ResMut<MainStamps>) {
    let now = Instant::now();
    stamps.frame += 1;
    if let Some(last) = stamps.last {
        let between = now.duration_since(last).as_secs_f32() * 1000.0;
        if between > stamps.slow {
            info!("main frame {}: {between:.1} ms from the end of one frame's schedules to the next's (the extraction)", stamps.frame);
        }
    }
    stamps.first = Some(now);
}

fn main_last(mut stamps: ResMut<MainStamps>, ui: Res<UiStamps>, nodes: Query<(), With<Node>>, words: Query<(), With<Text>>) {
    let now = Instant::now();
    if let Some(first) = stamps.first {
        let within = now.duration_since(first).as_secs_f32() * 1000.0;
        if within > stamps.slow {
            // The interface's own stages, if they were read (`UiStamps`).
            let mut parts = Vec::new();
            for (index, name) in UI_STAGES.iter().enumerate() {
                let (Some(from), Some(to)) = (ui.at[index], ui.at[index + 1]) else { continue };
                parts.push(format!("{name} {:.1}", to.duration_since(from).as_secs_f32() * 1000.0));
            }
            // When each schedule was reached, from the first.
            let reached: Vec<String> = SCHEDULES.iter().zip(&ui.reached).filter_map(|(name, at)| at.map(|at| format!("{name} at {:.1}", at.duration_since(first).as_secs_f32() * 1000.0))).collect();
            parts.push(format!("[{}]", reached.join(", ")));
            let before_ui = ui.at[0].map(|at| at.duration_since(first).as_secs_f32() * 1000.0).unwrap_or(0.0);
            let after_ui = ui.at[UI_STAGES.len()].map(|at| now.duration_since(at).as_secs_f32() * 1000.0).unwrap_or(0.0);
            info!(
                "main frame {}: {within:.1} ms in the main world's schedules; before the interface's stages {before_ui:.1}, {}, after them {after_ui:.1}; the interface's nodes {}, texts {}",
                stamps.frame,
                parts.join(", "),
                nodes.iter().count(),
                words.iter().count()
            );
        }
    }
    stamps.last = Some(now);
}

/// The interface's stages in `PostUpdate` (Bevy's `UiSystems`), in order.
const UI_STAGES: [&str; 4] = ["Prepare", "Content (text measured)", "Layout", "PostLayout (text laid out)"];

/// The clock at the boundaries of the interface's stages this frame, and
/// when each of the main schedules was reached (a reading in each).
#[derive(Resource, Default)]
struct UiStamps {
    at: [Option<Instant>; UI_STAGES.len() + 1],
    reached: [Option<Instant>; SCHEDULES.len()],
}

const SCHEDULES: [&str; 3] = ["PreUpdate", "Update", "PostUpdate"];

fn ui_stamp<const N: usize>(mut stamps: ResMut<UiStamps>) {
    stamps.at[N] = Some(Instant::now());
}

fn reached<const N: usize>(mut stamps: ResMut<UiStamps>) {
    stamps.reached[N] = Some(Instant::now());
}

/// Adds the clock's readings to the main world's schedules, if asked for.
pub fn add_main(app: &mut App) {
    use bevy::ui::UiSystems;
    if std::env::var_os("TRP_RENDER_TIMING").is_none() {
        return;
    }
    app.add_systems(PreUpdate, reached::<0>).add_systems(Update, reached::<1>).add_systems(PostUpdate, reached::<2>.before(UiSystems::Prepare));
    app.insert_resource(MainStamps { slow: slow_ms(), ..default() }).init_resource::<UiStamps>().add_systems(First, main_first).add_systems(Last, main_last).add_systems(
        PostUpdate,
        (
            ui_stamp::<0>.before(UiSystems::Prepare),
            ui_stamp::<1>.after(UiSystems::Prepare).before(UiSystems::Content),
            ui_stamp::<2>.after(UiSystems::Content).before(UiSystems::Layout),
            ui_stamp::<3>.after(UiSystems::Layout).before(UiSystems::PostLayout),
            ui_stamp::<4>.after(UiSystems::PostLayout),
        ),
    );
}

/// Adds the clock's readings to the renderer's schedule, if asked for.
/// `TRP_GPU_MEMORY` in the environment: the graphics card's memory as the
/// renderer's allocator has it, every two seconds (wgpu's
/// `generate_allocator_report`: what is allocated, and the blocks reserved
/// for it).
pub fn add_memory(renderer: &mut SubApp) {
    if std::env::var_os("TRP_GPU_MEMORY").is_none() {
        return;
    }
    renderer.add_systems(bevy::render::Render, report_memory.after(RenderSystems::Cleanup));
}

fn report_memory(device: Res<bevy::render::renderer::RenderDevice>, pictures: Res<bevy::render::render_asset::RenderAssets<bevy::render::texture::GpuImage>>, mut last: Local<Option<std::time::Instant>>) {
    if last.is_some_and(|at| at.elapsed().as_secs_f32() < 2.0) {
        return;
    }
    *last = Some(std::time::Instant::now());
    // The pictures the renderer holds, by their sizes and formats (every
    // level, every layer).
    let (mut count, mut bytes) = (0usize, 0u64);
    for (_, picture) in pictures.iter() {
        let (block_w, block_h) = picture.texture_format.block_dimensions();
        let block = picture.texture_format.block_copy_size(None).unwrap_or(4) as u64;
        for level in 0..picture.mip_level_count {
            let (w, h) = ((picture.size.width >> level).max(1), (picture.size.height >> level).max(1));
            bytes += w.div_ceil(block_w) as u64 * h.div_ceil(block_h) as u64 * block * picture.size.depth_or_array_layers as u64;
        }
        count += 1;
    }
    if let Some(report) = device.wgpu_device().generate_allocator_report() {
        info!("graphics memory: {} MB allocated in {} MB of blocks, {} allocations; {count} pictures of {} MB", report.total_allocated_bytes >> 20, report.total_reserved_bytes >> 20, report.allocations.len(), bytes >> 20);
    }
}

pub fn add(renderer: &mut SubApp) {
    use RenderSystems::*;
    if std::env::var_os("TRP_RENDER_TIMING").is_none() {
        return;
    }
    renderer.insert_resource(Stamps { slow: slow_ms(), ..default() });
    // (The schedule by its full name: `Render` here is the stage.)
    renderer.add_systems(
        bevy::render::Render,
        (
            stamp::<0>.before(ExtractCommands),
            stamp::<1>.after(ExtractCommands).before(PrepareAssets),
            stamp::<2>.after(PrepareAssets).before(PrepareMeshes),
            stamp::<3>.after(PrepareMeshes).before(ManageViews),
            stamp::<4>.after(ManageViews).before(Queue),
            stamp::<5>.after(Queue).before(PhaseSort),
            stamp::<6>.after(PhaseSort).before(Prepare),
            stamp::<7>.in_set(Prepare).after(PrepareResources).before(PrepareResourcesCollectPhaseBuffers),
            stamp::<8>.in_set(Prepare).after(PrepareResourcesFlush).before(PrepareBindGroups),
            stamp::<9>.after(Prepare).before(Render),
            stamp::<10>.after(Render).before(Cleanup),
            stamp::<11>.after(Cleanup).before(PostCleanup),
            (stamp::<12>, report).chain().after(PostCleanup),
        ),
    );
}


/// What the world holds at one frame of the main world's (`TRP_CENSUS=<frame>`):
/// its entities, those placed in it, the meshes (seen, hidden, skinned,
/// casting shadows, by their material), the distinct meshes and standard
/// materials drawn, the interface's nodes and the lights, and the groups
/// at the top of the hierarchy that hold the most meshes. What the
/// engine's per-entity work each frame goes over. Nothing is changed by it.
pub fn add_census(app: &mut App) {
    let Some(at) = std::env::var("TRP_CENSUS").ok().and_then(|value| value.parse::<u64>().ok()) else { return };
    app.insert_resource(Census { at, frame: 0 }).add_systems(Last, census);
}

#[derive(Resource)]
struct Census {
    at: u64,
    frame: u64,
}

fn census(world: &mut World) {
    use bevy::camera::visibility::{InheritedVisibility, ViewVisibility};
    use bevy::light::NotShadowCaster;
    use bevy::mesh::skinning::SkinnedMesh;
    use std::collections::{HashMap, HashSet};
    let mut census = world.resource_mut::<Census>();
    census.frame += 1;
    if census.frame != census.at {
        return;
    }
    let frame = census.frame;
    let entities = world.entities().len();
    let placed = world.query::<&GlobalTransform>().iter(world).count();
    let meshes: Vec<(Entity, AssetId<Mesh>, bool, bool)> = world
        .query::<(Entity, &Mesh3d, Option<&ViewVisibility>, Option<&InheritedVisibility>)>()
        .iter(world)
        .map(|(entity, mesh, seen, inherited)| (entity, mesh.id(), seen.is_some_and(|s| s.get()), inherited.is_some_and(|i| i.get())))
        .collect();
    let seen = meshes.iter().filter(|m| m.2).count();
    let hidden = meshes.iter().filter(|m| !m.3).count();
    let skinned = world.query_filtered::<(), (With<Mesh3d>, With<SkinnedMesh>)>().iter(world).count();
    let casters = world.query_filtered::<(), (With<Mesh3d>, Without<NotShadowCaster>)>().iter(world).count();
    let standard: Vec<AssetId<StandardMaterial>> = world.query::<&MeshMaterial3d<StandardMaterial>>().iter(world).map(|m| m.id()).collect();
    let shadow_only = world.query_filtered::<(), With<MeshMaterial3d<crate::shadow_only::ShadowOnlyMaterial>>>().iter(world).count();
    let grid = world.query_filtered::<(), With<MeshMaterial3d<crate::grid::GridMaterial>>>().iter(world).count();
    let distinct_meshes: HashSet<_> = meshes.iter().map(|m| m.1).collect();
    let distinct_materials: HashSet<_> = standard.iter().collect();
    let nodes = world.query_filtered::<(), With<Node>>().iter(world).count();
    let points = world.query_filtered::<(), With<PointLight>>().iter(world).count();
    let spots = world.query_filtered::<(), With<SpotLight>>().iter(world).count();
    let suns = world.query_filtered::<(), With<DirectionalLight>>().iter(world).count();
    info!(
        "census at frame {frame}: {entities} entities, {placed} placed; {} meshes ({seen} seen by a camera, {hidden} hidden, {skinned} skinned, {casters} casting shadows; {} standard, {shadow_only} shadow-only, {grid} grid), {} distinct meshes, {} distinct standard materials; {nodes} interface nodes; lights: {suns} sun, {spots} spot, {points} point",
        meshes.len(),
        standard.len(),
        distinct_meshes.len(),
        distinct_materials.len()
    );
    // The groups at the top of the hierarchy by the meshes under them.
    let mut groups: HashMap<String, [usize; 2]> = HashMap::new();
    for (entity, _, seen, _) in &meshes {
        let mut top = *entity;
        while let Some(parent) = world.get::<ChildOf>(top) {
            top = parent.parent();
        }
        let name = world.get::<Name>(top).map_or_else(|| format!("{top} (no name)"), |name| name.as_str().to_owned());
        let count = groups.entry(name).or_default();
        count[0] += 1;
        count[1] += usize::from(*seen);
    }
    let mut groups: Vec<_> = groups.into_iter().collect();
    groups.sort_by(|a, b| b.1[0].cmp(&a.1[0]));
    for (name, [all, seen]) in groups.iter().take(25) {
        info!("census:   {all:5} meshes ({seen} seen) under {name}");
    }
    info!("census:   {} groups in all", groups.len());
    // Under the largest group, its meshes by the name of their nearest
    // named owner below the top (the parts, pieces of clothing, gear).
    let depth = std::env::var("TRP_CENSUS_DEPTH").ok().and_then(|value| value.parse::<usize>().ok()).unwrap_or(2);
    if let Some((top_name, _)) = groups.first() {
        let mut parts: HashMap<String, [usize; 2]> = HashMap::new();
        for (entity, _, seen, _) in &meshes {
            let mut at = *entity;
            let mut path = Vec::new();
            while let Some(parent) = world.get::<ChildOf>(at) {
                if let Some(name) = world.get::<Name>(at) {
                    path.push(name.as_str().to_owned());
                }
                at = parent.parent();
            }
            if world.get::<Name>(at).map(|n| n.as_str()) != Some(top_name.as_str()) {
                continue;
            }
            // The two names nearest the top.
            let key = path.iter().rev().take(depth).cloned().collect::<Vec<_>>().join(" / ");
            let count = parts.entry(key).or_default();
            count[0] += 1;
            count[1] += usize::from(*seen);
        }
        let mut parts: Vec<_> = parts.into_iter().collect();
        parts.sort_by(|a, b| b.1[0].cmp(&a.1[0]));
        for (name, [all, seen]) in parts.iter().take(30) {
            info!("census:   {top_name}: {all:5} meshes ({seen} seen) under {name}");
        }
        info!("census:   {top_name}: {} owners in all", parts.len());
    }
}

/// An experiment, not a feature (`TRP_DROP_HIDDEN=<frame>`): at that frame
/// every mesh hidden by its visibility is despawned, to measure what the
/// engine's per-entity work over them costs a frame. What is drawn after
/// is wrong wherever one of them would have been shown.
pub fn add_drop_hidden(app: &mut App) {
    let Some(at) = std::env::var("TRP_DROP_HIDDEN").ok().and_then(|value| value.parse::<u64>().ok()) else { return };
    app.insert_resource(DropHidden { at, frame: 0 }).add_systems(Last, drop_hidden);
}

#[derive(Resource)]
struct DropHidden {
    at: u64,
    frame: u64,
}

fn drop_hidden(mut commands: Commands, mut drop: ResMut<DropHidden>, meshes: Query<(Entity, &bevy::camera::visibility::InheritedVisibility), With<Mesh3d>>, parents: Query<&ChildOf>, names: Query<&Name>) {
    drop.frame += 1;
    if drop.frame != drop.at {
        return;
    }
    // `TRP_DROP_GROUP=<name>`: only those under the top of that name.
    let group = std::env::var("TRP_DROP_GROUP").ok().map(|group| group.replace('_', " "));
    let mut n = 0;
    for (entity, inherited) in &meshes {
        let mut top = entity;
        while let Ok(parent) = parents.get(top) {
            top = parent.parent();
        }
        if group.as_deref().is_some_and(|group| names.get(top).map(|n| n.as_str()) != Ok(group)) {
            continue;
        }
        if !inherited.get() || group.as_deref() == Some("Character model") {
            commands.entity(entity).despawn();
            n += 1;
        }
    }
    info!("experiment: {n} hidden meshes despawned at frame {}", drop.frame);
}
