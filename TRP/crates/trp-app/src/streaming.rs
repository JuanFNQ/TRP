//! The models' pictures by their levels of detail, as the game's Unity keeps
//! them (`docs/spec/streaming.md`): with the game's texture quality and,
//! where asked for, its mip streaming.
//!
//! - **Mip streaming off** (`GraphicsSettingsController.ChangeTexturesQuality`):
//!   every picture loses its largest `2 - quality` levels for good (Unity's
//!   `globalTextureMipmapLimit`).
//! - **Mip streaming on** (`RefreshStreamingMemoryBudget`): a picture is kept
//!   on the graphics card from the level what draws with it needs, as near
//!   and as large on the screen as it is (the texel density of its mesh,
//!   its distance from each camera, the camera's field of view and height
//!   in pixels), with the game's bias of `2 - quality` levels; never more
//!   than `4 - quality` levels down; within the game's budget of 1,024 MB,
//!   the farthest given up first; and what nothing draws with goes down to
//!   the most it may (`streamingTextureDiscardUnusedMips`). A picture's
//!   levels are read again from its bundle when they are wanted, as the
//!   game reads them (`ReadAgain`), away from the frame.
//!
//! A picture on the card is never changed in place: a picture is cut to its
//! first level before it is added (`add`), and a change of level is a new
//! picture that the materials drawing with it are pointed at, the one before
//! let go. Only the models' pictures are taken (the engine's own, its
//! lighting's tables among them, are left alone: changing every picture as
//! it came drew the whole range black). The inventory's pictures are drawn
//! with every picture whole, as the game draws them (`ItemIconCreator`:
//! `requestedMipmapLevel` 0): their models' pictures do not come here. The
//! drawing alone: nothing of the simulation changes.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, OnceLock};

use bevy::camera::primitives::Aabb;
use bevy::prelude::*;
use bevy::render::render_resource::{TextureDimension, TextureFormat};
use bevy::tasks::{block_on, futures_lite::future::poll_once, AsyncComputeTaskPool, Task};

/// The game's two settings of its pictures (`Graphics.ini`: `TextureQuality`
/// 0 low to 2 high, `MipStreaming`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextureSettings {
    pub quality: u8,
    pub streaming: bool,
}

impl TextureSettings {
    /// The levels every picture loses with streaming off
    /// (`TextureQualityToMipBias`: `clamp(2 - quality, 0, 2)`).
    pub fn limit(&self) -> u32 {
        if self.streaming { 0 } else { 2u32.saturating_sub(self.quality as u32) }
    }

    /// The streaming's bias, in levels (the same figure).
    pub fn bias(&self) -> f32 {
        2.0 - self.quality.min(2) as f32
    }

    /// The most levels a picture may be brought down
    /// (`streamingMipmapsMaxLevelReduction = 4 - quality`).
    pub fn most_reduction(&self) -> u32 {
        4u32.saturating_sub(self.quality as u32)
    }
}

/// The pictures' quality the app starts at until the options change it:
/// HIGH (the user, 2026-10-22: "Start at HIGH instead"), streamed. The
/// install's own is what `AS THE INSTALL HAS THEM` takes.
pub const START_QUALITY: u8 = 2;

/// The game's budget for streamed pictures: 1,024 MB at every quality
/// (`RefreshStreamingMemoryBudget(1024f * Vector4.one, ...)`).
pub const BUDGET: u64 = 1024 << 20;
/// How much of the pictures may go up to the card in a frame: the game's
/// upload buffer (`Graphics.ini`: `MipStreamingBufferSize`, 32 MB in the
/// install read on 2026-10-15). `ESTIMATE`: Unity's buffer is a ring the
/// uploads pass through, not a limit a frame.
const UPLOAD_A_FRAME: usize = 32 << 20;
/// A picture brought down in detail only once it has been wanted lower for
/// so long, so that a thing passing in and out of sight does not send its
/// pictures up again and again. `ESTIMATE`: Unity's own delay is not
/// published.
const LOWER_AFTER: f32 = 2.0;
/// The levels are worked out so often. `ESTIMATE`.
const DECIDE_EVERY: f32 = 0.1;

/// The settings the run's pictures are made by, once, before any is.
static SETTINGS: OnceLock<TextureSettings> = OnceLock::new();
/// Pictures added since `take_new` last took them in; and how many have
/// been added whole, the game not streaming them.
static ADDED: Mutex<Vec<Streamed>> = Mutex::new(Vec::new());
static ADDED_WHOLE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// The settings the run's pictures are made by: once, before any is.
pub fn set(settings: TextureSettings) {
    let _ = SETTINGS.set(settings);
}

/// A picture made whole again from where it was read, as it was made the
/// first time: from its bundle's file (`trp_data::unity::ResourcePlace`),
/// so that nothing of it is kept in memory.
pub type ReadAgain = Arc<dyn Fn() -> Option<Image> + Send + Sync>;

/// Where a picture's levels are had again when it is streamed, or that it
/// is not.
pub enum Source {
    /// From its bundle's file.
    Bundle(ReadAgain),
    /// Kept here: a picture that has no bundle (a weapon of one's own's,
    /// the other game's), or whose levels are in its serialized file.
    Kept,
    /// Not streamed: the game's engine does not stream it
    /// (`m_StreamingMipmaps` off), and draws it whole while streaming is on.
    Whole,
}

/// Where a streamed picture's levels are had again.
#[derive(Clone)]
enum Whole {
    /// From its bundle.
    Read(ReadAgain),
    /// Kept here: a picture that has no bundle (a weapon of one's own's,
    /// the other game's).
    Kept(Arc<Vec<u8>>),
}

/// A model's picture into the engine's store as the settings have it: with
/// streaming off without the levels the quality leaves off (every picture,
/// as Unity's `globalTextureMipmapLimit` takes them all); with it on, one
/// the game streams from the most it may be brought down, to be raised as
/// it is needed, its levels had again from `source`. A picture of one
/// level, or before the settings are known, goes in as it is.
pub fn add(images: &mut Assets<Image>, mut image: Image, source: Source) -> Handle<Image> {
    let Some(settings) = SETTINGS.get().copied() else { return images.add(image) };
    if settings.streaming && matches!(source, Source::Whole) {
        ADDED_WHOLE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        return images.add(image);
    }
    let Some((levels, deepest)) = levels_of(&image) else { return images.add(image) };
    let Some(data) = image.data.take() else { return images.add(image) };
    let start = if settings.streaming { settings.most_reduction() } else { settings.limit() }.min(deepest);
    let first = images.add(cut(&image, &levels, &data, start));
    if settings.streaming {
        let whole = match source {
            Source::Bundle(again) => Whole::Read(again),
            Source::Kept | Source::Whole => Whole::Kept(Arc::new(data)),
        };
        let streamed = Streamed { template: image, whole, levels, deepest, loaded: start, lower_since: None, first: first.clone(), current: None, pending: None, arriving: None, stuck: false };
        if let Ok(mut added) = ADDED.lock() {
            added.push(streamed);
        }
    }
    first
}

/// A picture from `level` on, from its data whole.
fn cut(template: &Image, levels: &[(std::ops::Range<usize>, UVec2)], data: &[u8], level: u32) -> Image {
    let mut image = template.clone();
    let (start, size) = (levels[level as usize].0.start, levels[level as usize].1);
    image.texture_descriptor.size.width = size.x;
    image.texture_descriptor.size.height = size.y;
    image.texture_descriptor.mip_level_count = levels.len() as u32 - level;
    image.data = Some(data[start..levels.last().map_or(start, |last| last.0.end)].to_vec());
    image
}

/// A picture streamed, and what of it is on the card.
struct Streamed {
    /// The picture as made, without its data, and where its data is had
    /// again.
    template: Image,
    whole: Whole,
    /// Each level's place in the data and its size in pixels.
    levels: Vec<(std::ops::Range<usize>, UVec2)>,
    /// The deepest level a copy may start from (a compressed picture's
    /// first level whole blocks).
    deepest: u32,
    /// The level the card's copy starts from, and since when it has been
    /// wanted lower.
    loaded: u32,
    lower_since: Option<f32>,
    /// The copy first added, which the models were made with and which is
    /// kept as long as the picture is (a rig waits for it to be on the
    /// card); and the copy the materials draw with now, where it is
    /// another.
    first: Handle<Image>,
    current: Option<Handle<Image>>,
    /// A copy being made away from the frame, from its level; and whether
    /// one could not be (left as it is from then).
    pending: Option<(u32, Task<Option<Image>>)>,
    /// A copy made and added, from its level, on its way to the card: the
    /// materials are pointed at it once it is there (`take_made`).
    arriving: Option<(u32, Handle<Image>)>,
    stuck: bool,
}

impl Streamed {
    fn bytes_from(&self, level: u32) -> usize {
        self.levels[level as usize..].iter().map(|(range, _)| range.len()).sum()
    }

    /// Its copy from `level` on, made away from the frame.
    fn make(&self, level: u32) -> Task<Option<Image>> {
        let (template, levels, whole) = (self.template.clone(), self.levels.clone(), self.whole.clone());
        AsyncComputeTaskPool::get().spawn(async move {
            match whole {
                Whole::Kept(data) => Some(cut(&template, &levels, &data, level)),
                Whole::Read(again) => {
                    let data = again()?.data?;
                    (data.len() >= levels.last()?.0.end).then(|| cut(&template, &levels, &data, level))
                }
            }
        })
    }

    /// Whether anything but this holds the first copy.
    fn held(&self) -> bool {
        match &self.first {
            Handle::Strong(strong) => Arc::strong_count(strong) > 1,
            Handle::Uuid(..) => false,
        }
    }
}

/// The places of a picture's levels in its data, and their sizes; and the
/// deepest level a copy may start from.
fn levels_of(image: &Image) -> Option<(Vec<(std::ops::Range<usize>, UVec2)>, u32)> {
    let descriptor = &image.texture_descriptor;
    if descriptor.dimension != TextureDimension::D2 || descriptor.size.depth_or_array_layers != 1 || descriptor.mip_level_count < 2 {
        return None;
    }
    let format: TextureFormat = descriptor.format;
    let (block_w, block_h) = format.block_dimensions();
    let block_bytes = format.block_copy_size(None)? as usize;
    let (width, height) = (descriptor.size.width, descriptor.size.height);
    let mut at = 0;
    let mut levels = Vec::new();
    let mut deepest = 0;
    for level in 0..descriptor.mip_level_count {
        let (w, h) = ((width >> level).max(1), (height >> level).max(1));
        let bytes = w.div_ceil(block_w) as usize * h.div_ceil(block_h) as usize * block_bytes;
        // A compressed picture's first level is whole blocks.
        if w % block_w == 0 && h % block_h == 0 {
            deepest = level;
        }
        levels.push((at..at + bytes, UVec2::new(w, h)));
        at += bytes;
    }
    (image.data.as_ref()?.len() >= at).then_some((levels, deepest))
}

/// The pictures of the models as they stand.
#[derive(Resource)]
pub struct MipStreaming {
    pub settings: TextureSettings,
    /// By the first copy's id.
    pictures: HashMap<AssetId<Image>, Streamed>,
    /// Each copy's picture, by the copy's id (the first's own among them).
    family: HashMap<AssetId<Image>, AssetId<Image>>,
    /// The materials drawing with each picture, and the pictures each
    /// material draws with.
    users: HashMap<AssetId<Image>, HashSet<AssetId<StandardMaterial>>>,
    drawn_with: HashMap<AssetId<StandardMaterial>, Vec<AssetId<Image>>>,
    /// Each mesh's texel density: its surface over its pictures' (world
    /// area over UV area, the mesh's own size).
    density: HashMap<AssetId<Mesh>, f32>,
    since_decided: f32,
    clock: f32,
    reported: (usize, usize, usize),
}

impl MipStreaming {
    pub fn new(settings: TextureSettings) -> Self {
        set(settings);
        Self {
            settings,
            pictures: HashMap::new(),
            family: HashMap::new(),
            users: HashMap::new(),
            drawn_with: HashMap::new(),
            density: HashMap::new(),
            since_decided: 0.0,
            clock: 0.0,
            reported: (0, 0, 0),
        }
    }

    /// The copy to draw with for a picture named by any of its copies, if
    /// it is not that one.
    fn now_for(&self, named: AssetId<Image>) -> Option<Handle<Image>> {
        let picture = self.family.get(&named)?;
        let current = self.pictures.get(picture)?.current.as_ref()?;
        (current.id() != named).then(|| current.clone())
    }

    /// The materials drawing with a picture pointed at its copy from
    /// `level` (the first, where none), the copy before let go.
    fn point_to(&mut self, materials: &mut Assets<StandardMaterial>, id: AssetId<Image>, level: u32, copy: Option<Handle<Image>>) {
        let Some(streamed) = self.pictures.get_mut(&id) else { return };
        streamed.loaded = level;
        let before = std::mem::replace(&mut streamed.current, copy.clone()).map(|before| before.id());
        let to = copy.unwrap_or_else(|| streamed.first.clone());
        // The copies a material may name now: the first and the one before.
        let was = [Some(id), before];
        if let Some(before) = before {
            self.family.remove(&before);
        }
        self.family.insert(to.id(), id);
        for material in self.users.get(&id).into_iter().flatten() {
            let Some(material) = materials.get_mut(*material) else { continue };
            for slot in pictures_of(material) {
                if slot.as_ref().is_some_and(|named| named.id() != to.id() && was.contains(&Some(named.id()))) {
                    *slot = Some(to.clone());
                }
            }
        }
    }

    /// A picture nothing holds and nothing draws with any more, let go.
    fn forget(&mut self, picture: AssetId<Image>) {
        if let Some(streamed) = self.pictures.remove(&picture) {
            self.family.remove(&picture);
            if let (Some((_, copy)), Ok(mut lists)) = (&streamed.arriving, ON_CARD.lock()) {
                lists.0.retain(|id| *id != copy.id());
            }
            if let Some(current) = &streamed.current {
                self.family.remove(&current.id());
            }
        }
        self.users.remove(&picture);
    }
}

/// The picture fields of a material.
fn pictures_of(material: &mut StandardMaterial) -> [&mut Option<Handle<Image>>; 5] {
    [&mut material.base_color_texture, &mut material.normal_map_texture, &mut material.metallic_roughness_texture, &mut material.occlusion_texture, &mut material.emissive_texture]
}

/// The pictures added, taken in; new meshes' texel densities; each new
/// material noted with the pictures it draws with, and pointed at their
/// copies of the moment where it was made with older ones (a rig's picture
/// shared with a thing on the ground, `SharedPictures`); a material gone,
/// let go of. In `Last`, after the stores' events of the frame.
pub fn take_new(
    mut streaming: ResMut<MipStreaming>,
    meshes: Res<Assets<Mesh>>,
    mut mesh_events: MessageReader<AssetEvent<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut material_events: MessageReader<AssetEvent<StandardMaterial>>,
) {
    let streaming = &mut *streaming;
    if let Ok(mut added) = ADDED.lock() {
        for streamed in added.drain(..) {
            let id = streamed.first.id();
            streaming.family.insert(id, id);
            streaming.pictures.insert(id, streamed);
        }
    }
    for event in mesh_events.read() {
        match *event {
            AssetEvent::Added { id } => {
                if let Some(density) = meshes.get(id).and_then(texel_density) {
                    streaming.density.insert(id, density);
                }
            }
            AssetEvent::Removed { id } => {
                streaming.density.remove(&id);
            }
            _ => {}
        }
    }
    for event in material_events.read() {
        match *event {
            AssetEvent::Added { id } => {
                // (Taken to be changed only where a picture is: a material
                // changed is prepared again.)
                let Some(material) = materials.get(id) else { continue };
                let mut firsts = Vec::new();
                let mut stale = false;
                for slot in [&material.base_color_texture, &material.normal_map_texture, &material.metallic_roughness_texture, &material.occlusion_texture, &material.emissive_texture].into_iter().flatten() {
                    let Some(first) = streaming.family.get(&slot.id()).copied() else { continue };
                    firsts.push(first);
                    stale |= streaming.now_for(slot.id()).is_some();
                }
                if firsts.is_empty() {
                    continue;
                }
                for first in &firsts {
                    streaming.users.entry(*first).or_default().insert(id);
                }
                streaming.drawn_with.insert(id, firsts);
                if stale {
                    if let Some(material) = materials.get_mut(id) {
                        for slot in pictures_of(material) {
                            if let Some(now) = slot.as_ref().and_then(|named| streaming.now_for(named.id())) {
                                *slot = Some(now);
                            }
                        }
                    }
                }
            }
            AssetEvent::Removed { id } => {
                for picture in streaming.drawn_with.remove(&id).into_iter().flatten() {
                    let Some(users) = streaming.users.get_mut(&picture) else { continue };
                    users.remove(&id);
                    if users.is_empty() && !streaming.pictures.get(&picture).is_some_and(Streamed::held) {
                        streaming.forget(picture);
                    }
                }
            }
            _ => {}
        }
    }
}

/// A mesh's surface over its pictures' surface (Unity's
/// `Mesh.GetUVDistributionMetric`, taken here as the whole surface over the
/// whole of it in the pictures: `PROVISIONAL`, the engine's own sum is not
/// published), in its own size.
fn texel_density(mesh: &Mesh) -> Option<f32> {
    use bevy::mesh::VertexAttributeValues;
    let Some(VertexAttributeValues::Float32x3(positions)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION) else { return None };
    let Some(VertexAttributeValues::Float32x2(uvs)) = mesh.attribute(Mesh::ATTRIBUTE_UV_0) else { return None };
    let indices: Vec<usize> = mesh.indices()?.iter().collect();
    let (mut surface, mut picture) = (0.0f64, 0.0f64);
    for triangle in indices.chunks_exact(3) {
        let [a, b, c] = [triangle[0], triangle[1], triangle[2]];
        let (pa, pb, pc) = (Vec3::from(*positions.get(a)?), Vec3::from(*positions.get(b)?), Vec3::from(*positions.get(c)?));
        surface += ((pb - pa).cross(pc - pa).length() * 0.5) as f64;
        let (ua, ub, uc) = (Vec2::from(*uvs.get(a)?), Vec2::from(*uvs.get(b)?), Vec2::from(*uvs.get(c)?));
        picture += ((ub - ua).perp_dot(uc - ua).abs() * 0.5) as f64;
    }
    (picture > 1e-12 && surface > 0.0).then(|| (surface / picture) as f32)
}

/// The copies added and not yet seen on the card, and those the renderer
/// has seen there since (`see_on_card`).
static ON_CARD: Mutex<(Vec<AssetId<Image>>, Vec<AssetId<Image>>)> = Mutex::new((Vec::new(), Vec::new()));

/// In the renderer, after its pictures are readied: each copy waited for
/// that is on the card now, said to be.
pub fn see_on_card(images: Res<bevy::render::render_asset::RenderAssets<bevy::render::texture::GpuImage>>) {
    let Ok(mut lists) = ON_CARD.lock() else { return };
    if lists.0.is_empty() {
        return;
    }
    let (asked, there) = &mut *lists;
    asked.retain(|id| {
        let on = images.get(*id).is_some();
        if on {
            there.push(*id);
        }
        !on
    });
}

/// The renderer's side of the streaming: `see_on_card` after its pictures
/// are readied.
pub fn watch_card(app: &mut App) {
    use bevy::render::{Render, RenderApp, RenderSystems};
    if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
        render_app.add_systems(Render, see_on_card.after(RenderSystems::PrepareAssets).before(RenderSystems::Render));
    }
}

/// Copies made away from the frame, taken as they come, and the materials
/// drawing with a picture pointed at its copy once the copy is on the card.
///
/// A material changed is readied afresh by the engine, which lets go of
/// the one it had first, and a material that names a picture not yet on
/// the card is not ready and is not drawn (`unready`). The pictures
/// go up to the card a few a frame (`main::warm_up`, `UPLOAD_BUDGET`), so a
/// material pointed at its copy as the copy was added was not drawn until
/// the copy was up: the models flashed out as one walked up to them (the
/// user's report of 2026-10-26). Until it is there the material draws with
/// the copy it had, as Unity's streaming draws with the levels it has.
/// Pointed in `PostUpdate`, before the stores tell the renderer what
/// changed (`AssetEventSystems`).
pub fn take_made(mut streaming: ResMut<MipStreaming>, mut images: ResMut<Assets<Image>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let streaming = &mut *streaming;
    if !streaming.settings.streaming || streaming.pictures.is_empty() {
        return;
    }
    // Copies on the card now: their materials pointed at them.
    let there: Vec<AssetId<Image>> = ON_CARD.lock().map(|mut lists| std::mem::take(&mut lists.1)).unwrap_or_default();
    if !there.is_empty() {
        let up: Vec<(AssetId<Image>, u32, Handle<Image>)> = streaming
            .pictures
            .iter_mut()
            .filter(|(_, streamed)| streamed.arriving.as_ref().is_some_and(|(_, copy)| there.contains(&copy.id())))
            .filter_map(|(id, streamed)| streamed.arriving.take().map(|(level, copy)| (*id, level, copy)))
            .collect();
        for (id, level, copy) in up {
            streaming.point_to(&mut materials, id, level, Some(copy));
        }
    }
    let made: Vec<(AssetId<Image>, u32, Option<Image>)> = streaming
        .pictures
        .iter_mut()
        .filter_map(|(id, streamed)| {
            let (level, task) = streamed.pending.as_mut()?;
            let image = block_on(poll_once(task))?;
            let level = *level;
            streamed.pending = None;
            Some((*id, level, image))
        })
        .collect();
    for (id, level, image) in made {
        match image {
            Some(image) => {
                let copy = images.add(image);
                // `TRP_STREAM_AT_ONCE` in the environment points the
                // materials at it at once, as before 0.140.1, to set the two
                // beside each other.
                if std::env::var_os("TRP_STREAM_AT_ONCE").is_some() {
                    streaming.point_to(&mut materials, id, level, Some(copy));
                    continue;
                }
                if let Ok(mut lists) = ON_CARD.lock() {
                    lists.0.push(copy.id());
                }
                if let Some(streamed) = streaming.pictures.get_mut(&id) {
                    streamed.arriving = Some((level, copy));
                }
            }
            None => {
                warn!("mip streaming: a picture's levels could not be had again from its bundle; it is left as it is");
                if let Some(streamed) = streaming.pictures.get_mut(&id) {
                    streamed.stuck = true;
                }
            }
        }
    }
}

/// The levels the pictures want, worked out every `DECIDE_EVERY`, and the
/// card's copies brought to them: a picture raised at once (within the
/// upload a decision, the nearest first; its copy made away from the frame
/// and taken as it is ready, `take_made`), brought down once it has wanted
/// that for `LOWER_AFTER`. A thing drawn is counted whether it is in a
/// camera's sight or not, as long as it is shown (`PROVISIONAL`: Unity's
/// streaming is said to go by distance alone).
pub fn stream(
    time: Res<Time<Real>>,
    mut streaming: ResMut<MipStreaming>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    cameras: Query<(&Camera, &GlobalTransform, &Projection)>,
    drawn: Query<(&Mesh3d, &MeshMaterial3d<StandardMaterial>, &GlobalTransform, Option<&Aabb>, &InheritedVisibility)>,
) {
    let streaming = &mut *streaming;
    if !streaming.settings.streaming || streaming.pictures.is_empty() {
        return;
    }
    let dt = time.delta_secs();
    streaming.clock += dt;
    streaming.since_decided += dt;
    if streaming.since_decided < DECIDE_EVERY {
        return;
    }
    streaming.since_decided = 0.0;
    // Pictures nothing holds or draws with (one made and never drawn).
    let gone: Vec<AssetId<Image>> = streaming.pictures.iter().filter(|(id, streamed)| !streamed.held() && streaming.users.get(*id).is_none_or(HashSet::is_empty)).map(|(id, _)| *id).collect();
    for id in gone {
        streaming.forget(id);
    }
    let settings = streaming.settings;
    let most = settings.most_reduction();
    // Each camera that draws, where it is, its field of view and height.
    let eyes: Vec<(Vec3, f32, f32)> = cameras
        .iter()
        .filter(|(camera, ..)| camera.is_active)
        .filter_map(|(camera, place, projection)| {
            let Projection::Perspective(perspective) = projection else { return None };
            let height = camera.physical_viewport_size()?.y as f32;
            Some((place.translation(), (perspective.fov * 0.5).tan(), height))
        })
        .collect();
    // The level each picture wants, and how near what wants it is.
    let mut wanted: HashMap<AssetId<Image>, (f32, f32)> = HashMap::new();
    for (mesh, material, place, bounds, shown) in &drawn {
        if !shown.get() {
            continue;
        }
        let (Some(density), Some(material)) = (streaming.density.get(&mesh.id()), materials.get(&material.0)) else { continue };
        let (scale, _, _) = place.to_scale_rotation_translation();
        let grow = scale.abs().max_element();
        let centre = bounds.map_or(place.translation(), |bounds| place.transform_point(Vec3::from(bounds.center)));
        let reach = bounds.map_or(0.0, |bounds| Vec3::from(bounds.half_extents).length() * grow);
        let density = density * grow * grow;
        let pictures = [&material.base_color_texture, &material.normal_map_texture, &material.metallic_roughness_texture, &material.occlusion_texture, &material.emissive_texture];
        for (eye, tan_half, height) in &eyes {
            let distance = (centre.distance(*eye) - reach).max(0.0);
            for picture in pictures.iter().copied().flatten() {
                let Some(first) = streaming.family.get(&picture.id()).copied() else { continue };
                let size = streaming.pictures[&first].levels[0].1;
                let texels = size.x as f32 * size.y as f32;
                // Half the log2 of the picture's texels a square world unit
                // over the screen's pixels a square world unit at this
                // distance.
                let level = if distance <= 0.0 {
                    f32::NEG_INFINITY
                } else {
                    0.5 * (texels * distance * distance * tan_half * tan_half / (density * (height * 0.5) * (height * 0.5))).log2()
                } + settings.bias();
                let entry = wanted.entry(first).or_insert((f32::INFINITY, f32::INFINITY));
                entry.0 = entry.0.min(level);
                entry.1 = entry.1.min(distance);
            }
        }
    }
    // Whole levels, within what each may be; what nothing draws with goes
    // down to the most it may.
    let mut levels: HashMap<AssetId<Image>, (u32, f32)> = streaming
        .pictures
        .iter()
        .map(|(id, streamed)| {
            let floor = most.min(streamed.deepest);
            let (level, near) = wanted.get(id).map_or((floor, f32::INFINITY), |(level, near)| ((level.floor().max(0.0) as u32).min(floor), *near));
            (*id, (level, near))
        })
        .collect();
    // Within the budget: the farthest given up a level at a time.
    let mut total: u64 = levels.iter().map(|(id, (level, _))| streaming.pictures[id].bytes_from(*level) as u64).sum();
    if total > BUDGET {
        let mut farthest: Vec<AssetId<Image>> = levels.keys().copied().collect();
        farthest.sort_by(|a, b| levels[b].1.total_cmp(&levels[a].1));
        'down: loop {
            let mut moved = false;
            for id in &farthest {
                let streamed = &streaming.pictures[id];
                let (level, _) = levels.get_mut(id).expect("a level");
                if *level < most.min(streamed.deepest) {
                    total -= (streamed.bytes_from(*level) - streamed.bytes_from(*level + 1)) as u64;
                    *level += 1;
                    moved = true;
                    if total <= BUDGET {
                        break 'down;
                    }
                }
            }
            if !moved {
                break;
            }
        }
    }
    // The card's copies brought to them: raised the nearest first, within
    // the upload a frame; brought down after their wait.
    let mut order: Vec<(AssetId<Image>, u32, f32)> = levels.iter().map(|(id, (level, near))| (*id, *level, *near)).collect();
    order.sort_by(|a, b| a.2.total_cmp(&b.2));
    let mut sent = 0usize;
    let clock = streaming.clock;
    let mut changed = Vec::new();
    for (id, level, _) in order {
        let streamed = streaming.pictures.get_mut(&id).expect("a picture");
        if streamed.pending.is_some() || streamed.arriving.is_some() || streamed.stuck {
            continue;
        }
        if level < streamed.loaded {
            if sent > 0 && sent + streamed.bytes_from(level) > UPLOAD_A_FRAME {
                continue;
            }
            sent += streamed.bytes_from(level);
            streamed.lower_since = None;
            changed.push((id, level));
        } else if level > streamed.loaded {
            let since = *streamed.lower_since.get_or_insert(clock);
            if clock - since >= LOWER_AFTER {
                streamed.lower_since = None;
                changed.push((id, level));
            }
        } else {
            streamed.lower_since = None;
        }
    }
    for (id, level) in changed {
        // Down to the first copy's level, the first again at once; else a
        // new copy, made away from the frame.
        let streamed = streaming.pictures.get_mut(&id).expect("a picture");
        if level == most.min(streamed.deepest) {
            streaming.point_to(&mut materials, id, level, None);
        } else {
            streamed.pending = Some((level, streamed.make(level)));
        }
    }
    // (With the first copies kept beside the copies raised from them.)
    let on_card: usize = streaming
        .pictures
        .values()
        .map(|streamed| streamed.bytes_from(streamed.loaded) + if streamed.current.is_some() { streamed.bytes_from(most.min(streamed.deepest)) } else { 0 })
        .sum();
    let whole: usize = streaming.pictures.values().map(|streamed| streamed.bytes_from(0)).sum();
    let kept: usize = streaming.pictures.values().map(|streamed| if let Whole::Kept(data) = &streamed.whole { data.len() } else { 0 }).sum();
    let report = (streaming.pictures.len(), on_card >> 22, whole >> 22);
    if report != streaming.reported {
        streaming.reported = report;
        info!(
            "mip streaming: {} pictures, {} MB of them on the graphics card of {} MB whole ({} MB kept in memory, the rest read again from their bundles); {} added whole, the game not streaming them",
            streaming.pictures.len(),
            on_card >> 20,
            whole >> 20,
            kept >> 20,
            ADDED_WHOLE.load(std::sync::atomic::Ordering::Relaxed)
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::render::render_resource::{Extent3d, TextureDescriptor, TextureUsages};

    /// A picture of the given size, format and levels, its bytes counting up.
    fn picture(width: u32, height: u32, format: TextureFormat, levels: u32, bytes: usize) -> Image {
        let mut image = Image::default();
        image.texture_descriptor = TextureDescriptor {
            label: None,
            size: Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: levels,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        };
        image.data = Some((0..bytes).map(|at| at as u8).collect());
        image
    }

    #[test]
    fn the_settings_are_the_game_s_figures() {
        let at = |quality, streaming| TextureSettings { quality, streaming };
        assert_eq!([0, 1, 2].map(|q| at(q, false).limit()), [2, 1, 0]);
        assert_eq!([0, 1, 2].map(|q| at(q, true).limit()), [0, 0, 0]);
        assert_eq!([0, 1, 2].map(|q| at(q, true).bias()), [2.0, 1.0, 0.0]);
        assert_eq!([0, 1, 2].map(|q| at(q, true).most_reduction()), [4, 3, 2]);
    }

    #[test]
    fn a_compressed_picture_is_cut_at_whole_blocks() {
        // 256 by 128 in BC1, nine levels: 8 bytes a block of 4 by 4.
        let blocks: Vec<usize> = (0..9).map(|level| ((256u32 >> level).max(1).div_ceil(4) * (128u32 >> level).max(1).div_ceil(4)) as usize * 8).collect();
        let total = blocks.iter().sum();
        let image = picture(256, 128, TextureFormat::Bc1RgbaUnorm, 9, total);
        let (levels, deepest) = levels_of(&image).expect("levels");
        assert_eq!(levels.iter().map(|(range, _)| range.len()).collect::<Vec<_>>(), blocks);
        assert_eq!(levels.last().map(|(range, _)| range.end), Some(total));
        // Level 5 is 8 by 4, whole blocks; level 6 is 4 by 2, not.
        assert_eq!(deepest, 5);
        let data = image.data.clone().expect("data");
        let cut = cut(&image, &levels, &data, 2);
        assert_eq!((cut.texture_descriptor.size.width, cut.texture_descriptor.size.height, cut.texture_descriptor.mip_level_count), (64, 32, 7));
        assert_eq!(cut.data.as_deref(), Some(&data[levels[2].0.start..total]));
    }

    #[test]
    fn a_picture_of_one_level_or_short_data_is_not_streamed() {
        assert!(levels_of(&picture(64, 64, TextureFormat::Rgba8Unorm, 1, 64 * 64 * 4)).is_none());
        assert!(levels_of(&picture(64, 64, TextureFormat::Rgba8Unorm, 7, 64 * 64 * 4)).is_none());
    }
}
