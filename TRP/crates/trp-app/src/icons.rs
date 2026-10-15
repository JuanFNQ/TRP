//! The inventory's pictures as the game makes them (`docs/spec/icons.md`):
//! each item's own model, posed by its prefab's own icon settings, drawn
//! once by a copy of the game's icon camera with its three lights, finished
//! with its shadow and halved, kept as a PNG in `local\icon_cache\` and
//! read from there by every later run.
//!
//! The model is read on a thread of its own (as a weapon's rig is); the
//! frame puts it in a studio nothing else sees, draws it to a picture,
//! reads the picture back, and takes the model away again. One at a time.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;

use bevy::prelude::*;
use bevy::render::camera::{Exposure, RenderTarget, ScalingMode};
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured};
use bevy::render::view::RenderLayers;
use bevy::tasks::{block_on, futures_lite::future::poll_once, AsyncComputeTaskPool, Task};
use trp_data::icon::{Pose, Studio};
use trp_data::unity::assets::ObjRef;
use trp_data::unity::extract::{Hierarchy, MeshData, Node};

use crate::convert;
use crate::inventory_view::{Carried, Picture};
use crate::render_data::PreparedLooks;

/// The studio's own render layer: its camera sees this alone, and its lights
/// light this alone.
const LAYER: usize = 23;
/// The game's pixels to a cell (`ItemViewFactory.GetCellPixelSize`: so many
/// cells at 63, and one more).
const CELL_PIXELS: u32 = 63;
/// The cache's own folder under `local\icon_cache`: a new one when what is
/// drawn changes, so that an older picture is not read. (`v2` since
/// 2026-10-14, night: a picture of `v1` may have been read back before all
/// of its model was on the graphics card, without some of its parts.)
const CACHE: &str = "v2";
/// How many frames a model stands in the studio at the least once all of
/// it is on the graphics card (`weapon::UploadWatch`) before it is read
/// back.
const SETTLE_FRAMES: u32 = 4;
/// What the studio waited before it waited for the card: so many frames,
/// and one more for each so many bytes of the model's pictures, where the
/// app sends 8 MB a frame (`main::UPLOAD_BUDGET`) and meshes besides, and
/// shares that with every rig and picture going up. Kept to say, in the
/// log, which pictures that had read back short.
const OLD_RULE_BYTES_A_FRAME: usize = 24 << 20;

/// An item's model as the icon thread makes it ready: its tree, what each
/// node draws, how the whole is posed in the studio and how much of it the
/// camera takes in.
pub struct PreparedIcon {
    template: String,
    /// The picture's size, in pixels (drawn at twice that).
    size: [u32; 2],
    hierarchy: Hierarchy,
    /// Whether the tree's first node keeps its own place and turn (a
    /// weapon's object, hung from its container) or is the posed object
    /// itself (any other item's prefab, whose own turn the pose replaces).
    keeps_own: bool,
    looks: PreparedLooks,
    parts: Vec<Vec<(usize, usize)>>,
    extra: Vec<(usize, usize, usize)>,
    /// The whole's turn and place in the studio, in the engine's axes, and
    /// the camera's half height.
    turn: Quat,
    place: Vec3,
    half_height: f32,
    /// The bytes of its pictures, for how long to let them reach the card.
    picture_bytes: usize,
    /// The icon camera as the install has it, with the first of a run.
    studio: Option<Studio>,
}

/// The turn of a model of the other game's that has no pose for a picture
/// of its own, in the install's axes: an outfit's lies on the ground face
/// up with its head towards minus z (the SEVA's visor is there), and is
/// looked down on with its head at the top: up turned to the camera, minus
/// z up, x to the left. (This work's choice, `PROVISIONAL`: the game draws
/// a model with no pose as it stands, which for one lying flat is its
/// edge; and the other game's own pictures are drawings, not models.)
const LAID_FACE_UP: Pose = Pose { rotation: [0.0, std::f32::consts::FRAC_1_SQRT_2, -std::f32::consts::FRAC_1_SQRT_2, 0.0], position: [0.0; 3], has_offset: false, bounds_scale: 1.0 };

/// The icon thread's own reader of the install and its database, and of
/// the other game's folders.
struct Worker {
    unity: trp_data::unity::Assets,
    db: trp_data::Database,
    stalker: trp_data::xray::Stalker,
    studio: Option<Studio>,
}

impl Worker {
    fn new(install: &trp_data::Install) -> anyhow::Result<Self> {
        let studio = Studio::read(install).unwrap_or_else(|e| {
            warn!("the game's icon camera could not be read ({e:#}): it is taken as it was read on 2026-10-14");
            Studio::as_read()
        });
        let stalker = trp_data::xray::Stalker::locate();
        Ok(Self { unity: trp_data::unity::Assets::new(install.clone())?, db: trp_data::Database::load(install)?, stalker, studio: Some(studio) })
    }

    /// An item's model, posed for its picture of `cells`.
    fn prepare(&mut self, template: &str, cells: [u32; 2]) -> anyhow::Result<PreparedIcon> {
        // An outfit of the other game's has that game's model of it, where
        // its file names one and it can be read; else its donor's.
        if let Some(world) = self.db.outfits.get(template).and_then(|outfit| outfit.world_visual.clone()) {
            match self.prepare_stalker(template, cells, &world) {
                Ok(prepared) => return Ok(prepared),
                Err(e) => warn!("the picture of {template} is its donor's: its own model {world} could not be drawn: {e:#}"),
            }
        }
        let item = self.db.item(template)?;
        let key = item.prefab_key().ok_or_else(|| anyhow::anyhow!("it has no model (a container of the character's own, as the pockets)"))?.to_owned();
        let weapon = self.db.is_a(item, "Weapon");
        // A weapon with its parts, hung from its container's pose; any
        // other item, its prefab from the object its pose is on.
        let (hierarchy, pose, custom) = if weapon {
            let assembled = trp_data::weapon::assemble(&mut self.unity, &self.db, template)?;
            let files = self.unity.load(&key)?;
            let pose = trp_data::icon::pose_in(&self.unity, &files)?.map(|(_, pose)| pose);
            (assembled.hierarchy, pose, assembled.custom)
        } else {
            let (hierarchy, pose) = trp_data::icon::model_of(&mut self.unity, &key)?;
            (hierarchy, pose, None)
        };
        let mut looks = PreparedLooks::default();
        let mut parts = vec![Vec::new(); hierarchy.nodes.len()];
        // What is on: a node and every node above it.
        let mut on = vec![false; hierarchy.nodes.len()];
        for (index, node) in hierarchy.nodes.iter().enumerate() {
            on[index] = node.active && node.parent.is_none_or(|parent| on[parent]);
            let Some(renderer) = node.renderer.as_ref().filter(|_| on[index]) else { continue };
            match looks.mesh(&self.unity, renderer.mesh) {
                Ok(meshes) => {
                    // A part whose material cannot be read is left out of
                    // the picture: drawn, it is the stand-in's magenta.
                    for (slot, mesh) in meshes.into_iter().enumerate() {
                        let material = looks.material(&self.unity, renderer.materials.get(slot).copied().flatten());
                        if !looks.is_fallback(material) {
                            parts[index].push((mesh, material));
                        }
                    }
                }
                Err(e) => warn!("mesh on {}: {e:#}", node.name),
            }
        }
        let mut extra = Vec::new();
        if let Some(custom) = custom.as_deref() {
            let materials = looks.custom_materials(&custom.materials);
            for part in &custom.parts {
                for (slot, mesh) in looks.custom_mesh(&part.mesh).into_iter().enumerate() {
                    if let Some(material) = part.materials.get(slot).and_then(|index| materials.get(*index)) {
                        extra.push((part.node, mesh, *material));
                    }
                }
            }
        }
        self.framed(template, cells, hierarchy, weapon, looks, parts, extra, pose)
    }

    /// An outfit of the other game's, by its own model of the item as it
    /// lies in the world (`docs/spec/icons.md`, "an item of the other
    /// game's"): its meshes with their pictures, read loose or from the
    /// game's packed archives, hung from one node of no turn.
    fn prepare_stalker(&mut self, template: &str, cells: [u32; 2], world: &str) -> anyhow::Result<PreparedIcon> {
        let (bytes, _) = self.stalker.read(&format!("meshes/{world}"))?;
        let model = trp_data::xray::read(&bytes)?;
        let mut looks = PreparedLooks::default();
        let mut extra = Vec::new();
        for part in &model.parts {
            let picture = match self.stalker.read(&format!("textures/{}.dds", part.texture)).and_then(|(bytes, _)| trp_data::xray::read_picture(&bytes)) {
                Ok(picture) => picture,
                Err(e) => {
                    warn!("the picture of {template}: a mesh of {world} is left out: {e:#}");
                    continue;
                }
            };
            let mesh = MeshData { name: part.texture.clone(), positions: part.positions.clone(), normals: part.normals.clone(), uv0: part.uv.clone(), submeshes: vec![part.triangles.clone()], ..default() };
            let material = looks.stalker_material(&picture);
            for mesh in looks.custom_mesh(&mesh) {
                extra.push((0, mesh, material));
            }
        }
        let node = Node {
            name: world.to_owned(),
            parent: None,
            translation: [0.0; 3],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0; 3],
            active: true,
            renderer: None,
            scripts: Vec::new(),
            grip: None,
            placer: None,
            transform: ObjRef { file: usize::MAX, path_id: 0 },
        };
        let hierarchy = Hierarchy { nodes: vec![node] };
        self.framed(template, cells, hierarchy, false, looks, Vec::new(), extra, Some(LAID_FACE_UP))
    }

    /// A model posed and framed for its picture of `cells`: each node in
    /// the studio with the pose's turn on the whole, and the camera's half
    /// height from the bounds of what is drawn.
    #[allow(clippy::too_many_arguments)]
    fn framed(
        &mut self,
        template: &str,
        cells: [u32; 2],
        hierarchy: Hierarchy,
        keeps_own: bool,
        mut looks: PreparedLooks,
        mut parts: Vec<Vec<(usize, usize)>>,
        extra: Vec<(usize, usize, usize)>,
        pose: Option<Pose>,
    ) -> anyhow::Result<PreparedIcon> {
        parts.resize(hierarchy.nodes.len(), Vec::new());
        let pose = pose.unwrap_or(Pose { rotation: [0.0, 0.0, 0.0, 1.0], position: [0.0; 3], has_offset: false, bounds_scale: 1.0 });
        let turn = convert::rotation(pose.rotation);
        let mut world: Vec<Mat4> = Vec::with_capacity(hierarchy.nodes.len());
        for (index, node) in hierarchy.nodes.iter().enumerate() {
            let own = if index == 0 && !keeps_own { Transform::from_scale(Vec3::from(node.scale)) } else { convert::transform(node.translation, node.rotation, node.scale) };
            let above = node.parent.map_or(Mat4::from_quat(turn), |parent| world[parent]);
            world.push(above * own.compute_matrix());
        }
        // The bounds: every drawn mesh's own box, its corners in the
        // studio, but for lenses and the muzzle's flash
        // (`WeaponPreview.GetBounds`).
        let (mut low, mut high) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        let mut take = |mesh: usize, matrix: Mat4| {
            let Some(bevy::render::mesh::VertexAttributeValues::Float32x3(points)) = looks.meshes[mesh].attribute(Mesh::ATTRIBUTE_POSITION) else { return };
            let (mut from, mut to) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
            for p in points {
                (from, to) = (from.min(Vec3::from(*p)), to.max(Vec3::from(*p)));
            }
            if from.x > to.x {
                return;
            }
            for corner in 0..8 {
                let c = Vec3::new(if corner & 1 == 0 { from.x } else { to.x }, if corner & 2 == 0 { from.y } else { to.y }, if corner & 4 == 0 { from.z } else { to.z });
                let at = matrix.transform_point3(c);
                (low, high) = (low.min(at), high.max(at));
            }
        };
        for (index, node) in hierarchy.nodes.iter().enumerate() {
            if node.name.contains("linza") || node.name == "MuzzleJetCombinedMesh" {
                continue;
            }
            for (mesh, _) in &parts[index] {
                take(*mesh, world[index]);
            }
        }
        for (node, mesh, _) in &extra {
            take(*mesh, world[*node]);
        }
        anyhow::ensure!(low.x <= high.x, "nothing of it is drawn");
        looks.unskinned();
        // The camera's half height: the bounds' over the bounds' scale, or
        // their half width over the picture's shape where they are wider
        // than it (`ItemIconCreator.CenterByBounds`).
        let size = [cells[0] * CELL_PIXELS + 1, cells[1] * CELL_PIXELS + 1];
        let aspect = size[0] as f32 / size[1] as f32;
        let half = (high - low) / 2.0;
        let half_height = if half.x / half.y.max(1e-6) > aspect { half.x / aspect } else { half.y } / pose.bounds_scale;
        let offset = if pose.has_offset { convert::position(pose.position) } else { Vec3::ZERO };
        let place = -(low + high) / 2.0 + offset;
        let picture_bytes = looks.picture_bytes();
        Ok(PreparedIcon { template: template.to_owned(), size, hierarchy, keeps_own, looks, parts, extra, turn, place, half_height, picture_bytes, studio: self.studio.take() })
    }
}

/// A picture being drawn: the model in the studio, its target, how long
/// it has stood there (or, once its reading back is asked, how long that
/// has been waited for), and the reading back asked, if one is: only its
/// answer is taken.
struct Drawing {
    template: String,
    size: [u32; 2],
    model: Entity,
    target: Handle<Image>,
    frames: u32,
    wait: u32,
    asked: Option<Entity>,
    tries: u32,
    /// All of the model is on the graphics card, and how many frames the
    /// old rule had waited for it.
    on_card: bool,
    old_rule: u32,
}

/// So many frames at the most a picture's reading back is waited for: the
/// engine drops one it cannot make (its target not on the graphics card
/// yet: "Unknown image for screenshot, skipping") and never answers, and
/// the studio had waited on it for ever, every picture after it with it.
/// It is asked again then, as a picture that came out empty is.
const ANSWER_FRAMES_AT_MOST: u32 = 120;

/// Every picture asked for, made, being made or to be made.
#[derive(Resource)]
pub struct Icons {
    install: trp_data::Install,
    /// The options' switch `ICON GENERATION` as the run began: off, no
    /// picture is drawn or read.
    on: bool,
    folder: Option<PathBuf>,
    /// For an item whose model is not the install's own, a word of what
    /// it is drawn from, put into its picture's name in the cache: another
    /// model, another picture (`signatures`).
    signatures: HashMap<String, String>,
    /// Templates asked for already, with their size.
    known: HashSet<(String, [u32; 2])>,
    queue: VecDeque<(String, [u32; 2])>,
    worker: Option<Worker>,
    task: Option<Task<(Option<Worker>, (String, [u32; 2]), anyhow::Result<PreparedIcon>)>>,
    ready: VecDeque<PreparedIcon>,
    /// Pictures on their way to the screen away from the frame: read from
    /// the cache (`LoadFromUserCacheAsync`), or drawn and being finished
    /// and kept (`receive_icons`).
    arriving: Vec<Task<Arrived>>,
    studio: Option<Studio>,
    camera: Option<Entity>,
    drawing: Option<Drawing>,
    /// How many were drawn, read from the cache, and could not be made.
    pub drawn: usize,
    pub cached: usize,
    pub failed: usize,
    /// Those counts as last told in the log.
    reported: (usize, usize, usize),
}

impl Icons {
    pub fn new(install: trp_data::Install, on: bool, signatures: HashMap<String, String>) -> Self {
        // `local` beside the workspace, as the options' file is looked for;
        // or the folder `TRP_ICON_CACHE` names (to try a run with a cache of
        // its own, an empty one say, and leave the kept one be).
        let here = std::env::current_dir().unwrap_or_default();
        let folder = match std::env::var_os("TRP_ICON_CACHE") {
            Some(folder) => Some(PathBuf::from(folder).join(CACHE)),
            None => here.ancestors().map(|dir| dir.join("local")).find(|local| local.is_dir()).map(|local| local.join("icon_cache").join(CACHE)),
        };
        if !on {
            println!("the inventory's pictures are not drawn (ICON GENERATION is off)");
        }
        Self {
            install,
            on,
            folder,
            signatures,
            known: HashSet::new(),
            queue: VecDeque::new(),
            worker: None,
            task: None,
            ready: VecDeque::new(),
            arriving: Vec::new(),
            studio: None,
            camera: None,
            drawing: None,
            drawn: 0,
            cached: 0,
            failed: 0,
            reported: (0, 0, 0),
        }
    }

    /// Whether any picture is still to be made.
    pub fn busy(&self) -> bool {
        !self.queue.is_empty() || self.task.is_some() || !self.ready.is_empty() || self.drawing.is_some() || !self.arriving.is_empty()
    }

    fn path(&self, template: &str, size: [u32; 2]) -> Option<PathBuf> {
        let signed = self.signatures.get(template).map_or(String::new(), |signature| format!("_{signature}"));
        self.folder.as_ref().map(|folder| folder.join(format!("{template}_{}x{}{signed}.png", size[0], size[1])))
    }
}

/// The words for the cache of the items whose models are not the
/// install's: an outfit of the other game's by its model of the item (a
/// checksum of its path; `donor` where it has none and has its donor's).
pub fn signatures(db: &trp_data::Database) -> HashMap<String, String> {
    db.outfits
        .iter()
        .map(|(id, outfit)| (id.clone(), outfit.world_visual.as_deref().map_or("donor".to_owned(), |world| format!("{:08x}", trp_data::xray_archive::crc32(world.as_bytes())))))
        .collect()
}

pub struct IconsPlugin;

impl Plugin for IconsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (ask_icons, receive_icons, prepare_icons, draw_icons, report_icons).chain());
    }
}

/// A picture come back from away from the frame: its item's template, the
/// cells it was asked for, its size in pixels, whether it was drawn (else
/// read from the cache), and as it lies and turned a quarter.
struct Arrived {
    template: String,
    cells: [u32; 2],
    size: [u32; 2],
    drawn: bool,
    made: anyhow::Result<(Vec<u8>, Vec<u8>)>,
}

/// A picture as the screen draws it, from RGBA with its colours not
/// multiplied, as it lies and turned a quarter.
fn picture_of(images: &mut Assets<Image>, [width, height]: [u32; 2], upright: Vec<u8>, turned: Vec<u8>) -> Picture {
    let make = |w: u32, h: u32, data: Vec<u8>| Image::new(Extent3d { width: w, height: h, depth_or_array_layers: 1 }, TextureDimension::D2, data, TextureFormat::Rgba8UnormSrgb, bevy::asset::RenderAssetUsages::default());
    Picture::rendered(images.add(make(width, height, upright)), images.add(make(height, width, turned)), width as f32 / height as f32)
}

/// A picture the screen waits for has come, or cannot: the screen puts it
/// in its spinner's place (`inventory_view::PicturesArrived`), and a
/// change to what is carried is not made of it, which would draw the whole
/// screen afresh.
fn arrived(carried: &mut Carried, ready: &mut crate::inventory_view::PicturesArrived, template: String, picture: Option<Picture>) {
    match picture {
        Some(picture) => {
            carried.pictures.insert(template.clone(), picture);
        }
        None => {
            carried.picture_failed.insert(template.clone());
        }
    }
    ready.0.push(template);
}

/// The pictures read or finished away from the frame, put on the screen
/// as they come; one the cache had none of, or none of its size, is asked
/// of the icon thread to be drawn.
fn receive_icons(mut icons: ResMut<Icons>, mut carried: ResMut<Carried>, mut ready: ResMut<crate::inventory_view::PicturesArrived>, mut images: ResMut<Assets<Image>>) {
    let _clock = crate::render_timing::Clock::new("receive_icons");
    if icons.arriving.is_empty() {
        return;
    }
    let mut done = Vec::new();
    icons.arriving.retain_mut(|task| match block_on(poll_once(task)) {
        Some(come) => {
            done.push(come);
            false
        }
        None => true,
    });
    for come in done {
        match (come.made, come.drawn) {
            (Ok((upright, turned)), drawn) => {
                let picture = picture_of(&mut images, come.size, upright, turned);
                arrived(carried.bypass_change_detection(), &mut ready, come.template, Some(picture));
                if drawn {
                    icons.drawn += 1;
                } else {
                    icons.cached += 1;
                }
            }
            (Err(_), false) => icons.queue.push_back((come.template, come.cells)),
            (Err(e), true) => {
                warn!("the picture of {} is not made: {e:#}", come.template);
                icons.failed += 1;
                arrived(carried.bypass_change_detection(), &mut ready, come.template, None);
            }
        }
    }
}

/// Every item the screen shows, by its template and its size, asked for
/// when the screen first shows it, as the game asks for an item view's
/// picture when the view is made (`ItemView.RefreshIcon`,
/// `ItemViewFactory.LoadItemIcon`, `ItemIconCreator.GetItemIcon`; icons
/// spec, "When a picture is asked for"): what is worn and carried, and
/// the container open beside it (not the container itself: it has no
/// model of an item). What lies in a container not yet opened, and on the
/// ground, is not asked for. Its picture is read from the cache away from
/// the frame, as the game reads it (`LoadFromUserCacheAsync`); one the
/// cache has not is asked of the icon thread to be drawn. Meanwhile its
/// box shows a spinner, where the game's shows its loader.
fn ask_icons(carried: Res<Carried>, in_sight: Res<crate::inventory_view::LootInSight>, mut icons: ResMut<Icons>) {
    let _clock = crate::render_timing::Clock::new("ask_icons");
    if !icons.on || !(carried.is_changed() || in_sight.is_changed()) {
        return;
    }
    // What the screen shows in boxes of their own, as the game's views
    // are made: what is in the equipment's slots, and what lies in their
    // grids (the panel of the four containers); what lies in the grids of
    // a container opened in a window; and of the open container what its
    // panel has in sight (`LootInSight`), as the game's panel makes views
    // only for what is in sight of its scroll area (`GridViewMagnifier`).
    // Not a weapon's parts or a magazine's rounds, nor what is in a
    // container lying in another: those have no box of their own until it
    // is opened.
    let mut seen: Vec<&trp_sim::inventory::Item> = Vec::new();
    for item in carried.equipment.item.contents() {
        seen.push(item);
        seen.extend(item.grids.iter().flatten().map(|(_, child)| child));
    }
    for window in carried.windows.iter().filter(|window| window.grids) {
        if let Some(item) = carried.find(window.item) {
            seen.extend(item.grids.iter().flatten().map(|(_, child)| child));
        }
    }
    if carried.loot_open {
        for (grid, rows) in carried.loot.grids.iter().zip(&in_sight.0) {
            for (place, item) in grid {
                let height = if place.turned { item.cells()[0] } else { item.cells()[1] };
                if place.y < rows[1] && place.y + height > rows[0] {
                    seen.push(item);
                }
            }
        }
    }
    let seen: Vec<(String, [u32; 2])> = seen.into_iter().map(|item| (item.kind.template.clone(), item.cells())).collect();
    for wanted in seen {
        if wanted.1[0] == 0 || wanted.1[1] == 0 || !icons.known.insert(wanted.clone()) {
            continue;
        }
        let (template, cells) = wanted;
        let size = [cells[0] * CELL_PIXELS + 1, cells[1] * CELL_PIXELS + 1];
        let Some(path) = icons.path(&template, size) else {
            icons.queue.push_back((template, cells));
            continue;
        };
        icons.arriving.push(AsyncComputeTaskPool::get().spawn(async move {
            let made = trp_data::icon::load_png(&path).and_then(|(width, height, rgba)| {
                anyhow::ensure!([width, height] == size, "the cache's picture is {width} by {height}");
                let turned = trp_data::icon::turned(&rgba, width as usize, height as usize);
                Ok((rgba, turned))
            });
            Arrived { template, cells, size, drawn: false, made }
        }));
    }
}

/// A line in the log each time what was asked is all made: how many
/// pictures were read, drawn, and could not be made.
fn report_icons(mut icons: ResMut<Icons>) {
    let _clock = crate::render_timing::Clock::new("report_icons");
    let counts = (icons.cached, icons.drawn, icons.failed);
    if icons.busy() || counts == icons.reported {
        return;
    }
    icons.reported = counts;
    info!("the inventory's pictures: {} read from the cache, {} drawn, {} not made", counts.0, counts.1, counts.2);
}

/// The icon thread: one model at a time, in the order asked.
fn prepare_icons(mut icons: ResMut<Icons>, mut carried: ResMut<Carried>, mut ready: ResMut<crate::inventory_view::PicturesArrived>) {
    let _clock = crate::render_timing::Clock::new("prepare_icons");
    let icons = &mut *icons;
    if let Some(task) = icons.task.as_mut() {
        let Some((worker, (template, _), made)) = block_on(poll_once(task)) else { return };
        icons.task = None;
        icons.worker = worker;
        match made {
            Ok(prepared) => icons.ready.push_back(prepared),
            Err(e) => {
                info!("the picture of {template} is not made: {e:#}");
                icons.failed += 1;
                arrived(carried.bypass_change_detection(), &mut ready, template, None);
            }
        }
    }
    let Some(wanted) = icons.queue.pop_front() else {
        // Nothing left to read: the thread's reader and its bundles go,
        // let go of away from the frame, as the rigs' is (`weapon::
        // prepare_rigs`: freeing one in the frame had cost it 40 ms).
        if let Some(worker) = icons.worker.take() {
            AsyncComputeTaskPool::get().spawn(async move { drop(worker) }).detach();
        }
        return;
    };
    let (worker, install) = (icons.worker.take(), icons.install.clone());
    icons.task = Some(AsyncComputeTaskPool::get().spawn(async move {
        match worker.map_or_else(|| Worker::new(&install), Ok) {
            Ok(mut worker) => {
                let made = worker.prepare(&wanted.0, wanted.1);
                (Some(worker), wanted, made)
            }
            Err(e) => (None, wanted, Err(e)),
        }
    }));
}

/// The studio: a model made ready is put in it and the camera aimed at it;
/// after a few frames the picture is read back (`icon_captured`).
#[allow(clippy::too_many_arguments)]
fn draw_icons(
    mut commands: Commands,
    mut icons: ResMut<Icons>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut cameras: Query<(&mut Camera, &mut Projection)>,
    waiting: Res<crate::PipelinesWaiting>,
    mut carried: ResMut<Carried>,
    mut ready: ResMut<crate::inventory_view::PicturesArrived>,
    uploads: Res<crate::weapon::UploadWatch>,
) {
    let _clock = crate::render_timing::Clock::new("draw_icons");
    let icons = &mut *icons;
    // `TRP_ICON_TRACE` in the environment: where the pictures stand, every
    // 60 frames (with `main::count_waiting`'s pipelines still being built,
    // which a picture waits on).
    if std::env::var_os("TRP_ICON_TRACE").is_some() {
        static FRAME: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let frame = FRAME.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if frame % 60 == 0 {
            let drawing = icons.drawing.as_ref().map(|d| format!("{} frames {} wait {} asked {} tries {}", d.template, d.frames, d.wait, d.asked.is_some(), d.tries));
            info!("icon trace {frame}: queue {} task {} ready {} arriving {} drawing {:?} pipelines waiting {}", icons.queue.len(), icons.task.is_some(), icons.ready.len(), icons.arriving.len(), drawing, waiting.0.load(std::sync::atomic::Ordering::Relaxed));
        }
    }
    // The camera and its lights, once the icon camera is known.
    if icons.camera.is_none() {
        let Some(studio) = icons.ready.front_mut().and_then(|first| first.studio.take()).or_else(|| icons.studio.take()) else { return };
        let layer = RenderLayers::layer(LAYER);
        // The game's camera stands at (0, 0, -1) of its space looking
        // along z; in the engine's axes that is +1 looking along -z. It is
        // put further back: it is orthographic, and nothing of a large
        // thing is then cut off.
        let place = convert::position(studio.position).normalize_or(Vec3::Z) * 10.0;
        let camera = commands
            .spawn((
                Name::new("Icon camera"),
                Camera3d::default(),
                Camera { is_active: false, order: -5, hdr: false, clear_color: ClearColorConfig::Custom(Color::NONE), ..default() },
                Projection::Orthographic(OrthographicProjection { near: 0.0, far: 40.0, ..OrthographicProjection::default_3d() }),
                Transform { translation: place, rotation: convert::rotation(studio.rotation), ..default() },
                Msaa::Sample4,
                bevy::core_pipeline::tonemapping::Tonemapping::None,
                // No light but its own (`IconRenderSettings`: ambient light
                // black), and an exposure at which a light's intensity is
                // the game's.
                // (Bevy 0.16 copies this into the render world, where it
                // asks for a camera of its own and warns, once, of an
                // entity with a camera and no render graph: that entity is
                // the render world's, and nothing is wrong with this one.)
                AmbientLight { color: Color::WHITE, brightness: 0.0, affects_lightmapped_meshes: false },
                Exposure { ev100: 0.0 },
                layer.clone(),
            ))
            .id();
        for light in &studio.lights {
            // Bevy's diffuse light has a 1/pi the game's has not, and the
            // exposure of ev100 0 takes 1/1.2 of it: so many lux give the
            // game's intensity. Each light's turn is in the camera's space.
            commands.spawn((
                Name::new("Icon light"),
                DirectionalLight { color: Color::srgb(light.color[0], light.color[1], light.color[2]), illuminance: light.intensity * std::f32::consts::PI * 1.2, shadows_enabled: false, ..default() },
                Transform::from_rotation(convert::rotation(studio.rotation) * convert::rotation(light.rotation)),
                layer.clone(),
            ));
        }
        info!("the icon studio: its camera {camera} and {} lights", studio.lights.len());
        icons.camera = Some(camera);
        icons.studio = Some(studio);
        // The camera is there from the frame's end: aimed in this frame it
        // would not be, and the first picture would be read from a target
        // nothing drew to.
        return;
    }
    let Some(camera) = icons.camera else { return };
    // A model in the studio: once it has stood long enough, its picture is
    // read back.
    if let Some(drawing) = icons.drawing.as_mut() {
        drawing.frames += 1;
        // Not until every mesh and picture of it is on the card: a part
        // whose own are not is left out of the drawing, and the picture
        // kept without it.
        if !drawing.on_card {
            let name = format!("icon {}", drawing.model);
            if uploads.take(|waited| waited == name).is_empty() {
                return;
            }
            if drawing.frames > drawing.old_rule {
                info!("the picture of {}: its model all on the graphics card {} frames after it was put in the studio, where {} had been waited", drawing.template, drawing.frames, drawing.old_rule);
            }
            (drawing.on_card, drawing.frames) = (true, 0);
        }
        if drawing.asked.is_none() && drawing.frames >= drawing.wait && waiting.0.load(std::sync::atomic::Ordering::Relaxed) == 0 {
            drawing.asked = Some(commands.spawn(Screenshot::image(drawing.target.clone())).observe(icon_captured).id());
            drawing.frames = 0;
        } else if drawing.asked.is_some() && drawing.frames > ANSWER_FRAMES_AT_MOST {
            if drawing.tries < 3 {
                warn!("the picture of {} was not read back in {ANSWER_FRAMES_AT_MOST} frames: asked again", drawing.template);
                (drawing.asked, drawing.frames, drawing.tries) = (None, 0, drawing.tries + 1);
            } else {
                warn!("the picture of {} was not read back three times: not made", drawing.template);
                give_up(&mut commands, icons, &mut images, carried.bypass_change_detection(), &mut ready);
                if let Ok((mut camera, _)) = cameras.get_mut(camera) {
                    camera.is_active = false;
                }
            }
        }
        return;
    }
    let Some(prepared) = icons.ready.pop_front() else { return };
    let PreparedIcon { template, size, hierarchy, keeps_own, looks, parts, extra, turn, place, half_height, picture_bytes, .. } = prepared;
    let (mesh_handles, material_handles, pictures) = looks.add_with_pictures(&mut meshes, &mut images, &mut materials);
    let layer = RenderLayers::layer(LAYER);
    let model = commands.spawn((Name::new(format!("Icon of {template}")), Transform { translation: place, rotation: turn, ..default() }, Visibility::Inherited)).id();
    uploads.wait(format!("icon {model}"), pictures.iter().map(Handle::id).collect(), mesh_handles.iter().map(Handle::id).collect());
    let mut entities: Vec<Entity> = Vec::with_capacity(hierarchy.nodes.len());
    for (index, node) in hierarchy.nodes.iter().enumerate() {
        let own = if index == 0 && !keeps_own { Transform::from_scale(Vec3::from(node.scale)) } else { convert::transform(node.translation, node.rotation, node.scale) };
        let visibility = if node.active { Visibility::Inherited } else { Visibility::Hidden };
        let entity = commands.spawn((own, visibility)).id();
        commands.entity(node.parent.map_or(model, |parent| entities[parent])).add_child(entity);
        entities.push(entity);
        for (mesh, material) in &parts[index] {
            let drawn = commands.spawn((Mesh3d(mesh_handles[*mesh].clone()), MeshMaterial3d(material_handles[*material].clone()), Transform::default(), layer.clone())).id();
            commands.entity(entity).add_child(drawn);
        }
    }
    for (node, mesh, material) in &extra {
        let drawn = commands.spawn((Mesh3d(mesh_handles[*mesh].clone()), MeshMaterial3d(material_handles[*material].clone()), Transform::default(), layer.clone())).id();
        commands.entity(entities[*node]).add_child(drawn);
    }
    // Drawn at twice its size (`IconCreatorBase`: a texture of double
    // size), onto a picture of its own.
    let (width, height) = (size[0] * 2, size[1] * 2);
    // With nothing in it to send: a picture with something to send waits
    // its turn behind every other's (`main::UPLOAD_BUDGET`), and one not on
    // the card when it is read back is not read at all. The camera clears
    // it before drawing.
    let mut target = Image::new_uninit(Extent3d { width, height, depth_or_array_layers: 1 }, TextureDimension::D2, TextureFormat::Rgba8UnormSrgb, bevy::asset::RenderAssetUsages::default());
    target.texture_descriptor.usage = TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_SRC | TextureUsages::COPY_DST | TextureUsages::RENDER_ATTACHMENT;
    let target = images.add(target);
    if let Ok((mut aim, mut projection)) = cameras.get_mut(camera) {
        aim.target = RenderTarget::Image(target.clone().into());
        aim.is_active = true;
        if let Projection::Orthographic(ortho) = &mut *projection {
            ortho.scaling_mode = ScalingMode::Fixed { width: 2.0 * half_height * size[0] as f32 / size[1] as f32, height: 2.0 * half_height };
        }
    }
    let old_rule = SETTLE_FRAMES + (picture_bytes / OLD_RULE_BYTES_A_FRAME) as u32;
    icons.drawing = Some(Drawing { template, size, model, target, frames: 0, wait: SETTLE_FRAMES, asked: None, tries: 0, on_card: false, old_rule });
}

/// A picture that cannot be made: its model and its target go, and its
/// box keeps the plain shape of its sort.
fn give_up(commands: &mut Commands, icons: &mut Icons, images: &mut Assets<Image>, carried: &mut Carried, ready: &mut crate::inventory_view::PicturesArrived) {
    let Some(drawing) = icons.drawing.take() else { return };
    commands.entity(drawing.model).despawn();
    images.remove(&drawing.target);
    icons.failed += 1;
    arrived(carried, ready, drawing.template, None);
}

/// The picture read back: finished as the game finishes it, put on the
/// screen, written to the cache; the model goes.
fn icon_captured(
    trigger: Trigger<ScreenshotCaptured>,
    mut commands: Commands,
    mut icons: ResMut<Icons>,
    mut carried: ResMut<Carried>,
    mut ready: ResMut<crate::inventory_view::PicturesArrived>,
    mut images: ResMut<Assets<Image>>,
    mut cameras: Query<&mut Camera>,
) {
    let icons = &mut *icons;
    // (An answer to a reading back asked before, and asked again since, is
    // not this picture's.)
    let Some(drawing) = icons.drawing.as_mut().filter(|drawing| drawing.asked == Some(trigger.target())) else { return };
    let captured = &trigger.event().0;
    let Some(data) = captured.data.as_ref() else { return };
    let [width, height] = drawing.size;
    // Nothing drawn yet (a picture of the model's not yet on the card):
    // tried again a few frames on, three times at the most, and then given
    // up, the box keeping the plain shape of its sort. An empty picture is
    // never kept.
    let empty = !data.chunks_exact(4).any(|pixel| pixel[3] > 0);
    if empty && drawing.tries < 3 {
        (drawing.tries, drawing.frames, drawing.asked, drawing.wait) = (drawing.tries + 1, 0, None, SETTLE_FRAMES * 2);
        return;
    }
    if empty {
        warn!("the picture of {} came out empty three times: not made", drawing.template);
        give_up(&mut commands, icons, &mut images, carried.bypass_change_detection(), &mut ready);
        if let Some(mut camera) = icons.camera.and_then(|camera| cameras.get_mut(camera).ok()) {
            camera.is_active = false;
        }
        return;
    }
    // Finished, kept and turned away from the frame: the shadow's blur and
    // the PNG had cost the frame they were made in.
    let shadow = icons.studio.as_ref().map_or_else(|| Studio::as_read(), Clone::clone);
    let (template, model, target) = (drawing.template.clone(), drawing.model, drawing.target.clone());
    let (path, data, cells) = (icons.path(&template, [width, height]), data.clone(), [(width - 1) / CELL_PIXELS, (height - 1) / CELL_PIXELS]);
    icons.arriving.push(AsyncComputeTaskPool::get().spawn(async move {
        let rgba = trp_data::icon::finish(&data, (width * 2) as usize, (height * 2) as usize, shadow.shadow, shadow.shadow_offset);
        if let Some(path) = path {
            if let Err(e) = trp_data::icon::save_png(&path, width, height, &rgba) {
                warn!("the picture of {template} is not kept: {e:#}");
            }
        }
        let turned = trp_data::icon::turned(&rgba, width as usize, height as usize);
        Arrived { template, cells, size: [width, height], drawn: true, made: Ok((rgba, turned)) }
    }));
    commands.entity(model).despawn();
    images.remove(&target);
    icons.drawing = None;
    if let Some(mut camera) = icons.camera.and_then(|camera| cameras.get_mut(camera).ok()) {
        camera.is_active = false;
    }
}
