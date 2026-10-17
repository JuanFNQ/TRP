//! The character's own model on the inventory's gear page, behind the
//! equipment's slots where the app had a paper figure (`CHARACTER MODEL`;
//! the user, 2026-10-14, night: "a still model of the 3d model of the
//! current playermodel ... standing straight like the image provided this
//! stance comes from the pre-raid screen").
//!
//! It is the body as drawn (`body_view`): **the same meshes, pictures,
//! materials and bind poses, by their handles**, on a skeleton of its own,
//! so that nothing of it is read or sent to the graphics card a second
//! time. Its skeleton stands as the game's own screens stand the character
//! with nothing in its hands (`trp_sim::body::BodyData::load_standing`: the
//! body animator's `GUI` layer, `inv_idle_hands_down`), turned to face its
//! camera as the game turns it, and its camera stands where the game's
//! screens put theirs (`PlayerModelView.Show`: the model 5 m ahead and 1 m
//! below). Each piece is shown while the body's own piece is: what is worn
//! comes and goes on it as it does on the body.
//!
//! The camera draws it onto a picture the page shows, on a render layer
//! nothing else is on, and **only when something has changed** (what it
//! wears, the window's size): a few frames and it is off again. The game's
//! screens let the animator run on; the user asked for it still.

use std::collections::HashMap;

use bevy::asset::RenderAssetUsages;
use bevy::math::Affine3A;
use bevy::prelude::*;
use bevy::camera::RenderTarget;
use bevy::mesh::skinning::{SkinnedMesh, SkinnedMeshInverseBindposes};
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};
use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::window::PrimaryWindow;

use crate::convert;
use crate::inventory_view::Carried;
use crate::weapon::UploadWatch;

/// The figure's own render layer: its camera sees this alone, and its light
/// lights this alone.
pub const LAYER: usize = 22;
/// Where it stands: anywhere, nothing else being on its layer.
const PLACE: Vec3 = Vec3::new(0.0, -50.0, 0.0);
/// The game's turn of the model, half round the vertical, in the install's
/// axes (`PlayerModelLoader.CreatePlayerBody`).
const TURN: [f32; 4] = [0.0, 1.0, 0.0, 0.0];
/// Its camera, from the model's feet: 5 m before it and 1 m up, looking
/// level (`PlayerModelView.Show`: the model at (0, -1, 5) of the view).
const AHEAD: f32 = 5.0;
const UP: f32 = 1.0;
/// Room above the head's bone for the head and what is on it, and about
/// the whole, in metres. `ESTIMATE`: the app's framing in the panel.
const HEAD_ROOM: f32 = 0.28;
const MARGIN: f32 = 0.04;
/// The figure's light: the range's sun, from the same way and as bright,
/// without shadows. `ESTIMATE`: the game's screens light the model in a
/// scene of their own, not read.
const LIGHT_LUX: f32 = 12_000.0;
const LIGHT_FROM: Vec3 = Vec3::new(3.0, 8.0, 4.0);
/// The camera stays on so many frames at the least for a drawing, and
/// until the renderer has nothing left to build; at most so many.
const DRAW_FRAMES: u32 = 2;
const DRAW_FRAMES_AT_MOST: u32 = 240;
/// The name it waits for its meshes and pictures under (`UploadWatch`).
const ON_CARD: &str = "the character model";

/// A piece of the figure drawn as a piece of the body is: shown while that
/// one is.
#[derive(Component)]
pub struct Twin(pub Entity);

/// The picture the figure is drawn onto, for the gear page.
#[derive(Resource, Default)]
pub struct FigureImage(pub Option<Handle<Image>>);

/// The figure as built: its camera, what it is drawn with (to wait for it
/// on the graphics card), and where its drawing stands.
#[derive(Resource)]
pub struct Figure {
    camera: Entity,
    meshes: Vec<Handle<Mesh>>,
    materials: Vec<Handle<StandardMaterial>>,
    /// Asked to wait for its meshes and pictures, and they are there.
    waiting: bool,
    on_card: bool,
    /// Something changed since it was last drawn; the frames the camera has
    /// been on for this drawing; the picture's size in pixels.
    wanted: bool,
    drawing: Option<u32>,
    size: UVec2,
}

/// Builds the figure beside the body, piece by piece as `body_view::
/// spawn_body` makes the body's own. The corpse the character becomes
/// (`death`) is built the same way: the same meshes on a skeleton of its
/// own, hidden until it is wanted.
pub struct Builder {
    made: Made,
    root: Entity,
    by_path: HashMap<String, usize>,
    paths: Vec<String>,
    bones: Vec<Entity>,
    /// Each bone in the root's axes, in the engine's.
    world: Vec<Affine3A>,
    meshes: Vec<Handle<Mesh>>,
    materials: Vec<Handle<StandardMaterial>>,
    pieces: usize,
    /// The corpse's outfits, each to be posed on its skeleton as it falls.
    outfits: Vec<crate::death::CorpseOutfit>,
}

/// What a builder makes.
#[derive(Clone, Copy, PartialEq)]
enum Made {
    /// The gear page's character model, on a layer of its own.
    Figure,
    /// The corpse, in the world with the body.
    Corpse,
}

impl Builder {
    /// The skeleton, standing: an entity for each bone of `paths` (a
    /// parent before what hangs from it), put where `pose` has it.
    pub fn new(commands: &mut Commands, paths: &[String], pose: &trp_sim::body::Pose) -> Self {
        let root = commands.spawn((Name::new("Character model"), Transform { translation: PLACE, rotation: convert::rotation(TURN), ..default() }, Visibility::default())).id();
        Self::on(Made::Figure, commands, root, paths, |index| convert::transform(pose[index].0, pose[index].1, [1.0; 3]))
    }

    /// The corpse's skeleton, at the world's origin and hidden: its bones
    /// are given the body's pose when the character dies.
    pub fn corpse(commands: &mut Commands, paths: &[String]) -> Self {
        let root = commands.spawn((Name::new("Corpse"), Transform::default(), Visibility::Hidden)).id();
        Self::on(Made::Corpse, commands, root, paths, |_| Transform::default())
    }

    fn on(made: Made, commands: &mut Commands, root: Entity, paths: &[String], local_of: impl Fn(usize) -> Transform) -> Self {
        let mut by_path = HashMap::with_capacity(paths.len());
        let (mut bones, mut world) = (Vec::with_capacity(paths.len()), Vec::with_capacity(paths.len()));
        let whose = if made == Made::Corpse { "the corpse's" } else { "the model's" };
        for (index, path) in paths.iter().enumerate() {
            let local = local_of(index);
            let parent = path.rfind('/').and_then(|at| by_path.get(&path[..at]).copied());
            let name = path.rsplit('/').next().unwrap_or(path);
            let entity = commands.spawn((Name::new(format!("{name}, {whose}")), local, Visibility::default())).id();
            commands.entity(parent.map_or(root, |parent| bones[parent])).add_child(entity);
            world.push(parent.map_or(Affine3A::IDENTITY, |parent: usize| world[parent]) * local.compute_affine());
            bones.push(entity);
            by_path.insert(path.clone(), index);
        }
        Self { made, root, by_path, paths: paths.to_vec(), bones, world, meshes: Vec::new(), materials: Vec::new(), pieces: 0, outfits: Vec::new() }
    }

    /// The render layer its pieces are drawn on.
    fn layer(&self) -> RenderLayers {
        match self.made {
            Made::Figure => RenderLayers::layer(LAYER),
            Made::Corpse => RenderLayers::layer(crate::body_view::SEEN),
        }
    }

    /// A piece of it marked as the twin of a piece of the body.
    fn twin(&self, commands: &mut Commands, piece: Entity, original: Entity) {
        match self.made {
            Made::Figure => commands.entity(piece).insert(Twin(original)),
            Made::Corpse => commands.entity(piece).insert(crate::death::CorpseTwin(original)),
        };
    }

    /// The bone a mesh's bone is drawn on: that one, or the nearest above it
    /// the figure has.
    fn at(&self, path: &str) -> Option<usize> {
        let mut path = path;
        loop {
            if let Some(index) = self.by_path.get(path) {
                return Some(*index);
            }
            path = &path[..path.rfind('/')?];
        }
    }

    fn joint(&self, path: &str) -> Entity {
        self.at(path).map_or(self.root, |index| self.bones[index])
    }

    /// The bone whose path ends so, if the figure has it.
    pub fn bone(&self, end: &str) -> Option<Entity> {
        self.paths.iter().position(|path| path.ends_with(end)).map(|at| self.bones[at])
    }

    /// A skinned piece of the body, on the bones it names.
    pub fn skinned(&mut self, commands: &mut Commands, original: Entity, drawn: (&Handle<Mesh>, &Handle<StandardMaterial>), poses: &Handle<SkinnedMeshInverseBindposes>, bone_paths: &[String], hidden: bool) -> Entity {
        let joints = bone_paths.iter().map(|path| self.joint(path)).collect();
        self.skinned_on(commands, original, drawn, poses, joints, hidden)
    }

    /// A skinned piece on joints of its own (an outfit's, `outfit`).
    pub fn skinned_on(&mut self, commands: &mut Commands, original: Entity, (mesh, material): (&Handle<Mesh>, &Handle<StandardMaterial>), poses: &Handle<SkinnedMeshInverseBindposes>, joints: Vec<Entity>, hidden: bool) -> Entity {
        let visibility = if hidden { Visibility::Hidden } else { Visibility::Inherited };
        let name = if self.made == Made::Corpse { "Corpse, a piece" } else { "Character model, a piece" };
        let piece = commands
            .spawn((
                Name::new(name),
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material.clone()),
                SkinnedMesh { inverse_bindposes: poses.clone(), joints },
                NoFrustumCulling,
                self.layer(),
                visibility,
            ))
            .id();
        self.twin(commands, piece, original);
        commands.entity(self.root).add_child(piece);
        self.keep(mesh, material);
        piece
    }

    fn keep(&mut self, mesh: &Handle<Mesh>, material: &Handle<StandardMaterial>) {
        if !self.meshes.contains(mesh) {
            self.meshes.push(mesh.clone());
        }
        if !self.materials.contains(material) {
            self.materials.push(material.clone());
        }
        self.pieces += 1;
    }

    /// An outfit's skeleton for the figure: its rig (in the engine's axes)
    /// given the figure's bones as they stand, once, as `body_view::
    /// see_outfit` gives it the body's every frame. `rest` is each rig
    /// bone's own matrix for a point of a mesh. The entities its meshes
    /// hang on. The corpse's is kept to be posed as the corpse falls
    /// (`death`).
    pub fn outfit(&mut self, commands: &mut Commands, item: &str, rig: &trp_data::outfit::Rig, rest: &[Affine3A]) -> Vec<Entity> {
        use trp_data::outfit::Move;
        if self.made == Made::Corpse {
            let ours: Vec<Entity> = rig
                .bones
                .iter()
                .map(|bone| {
                    let name = bone.path.rsplit('/').next().unwrap_or(&bone.path);
                    let entity = commands.spawn((Name::new(format!("{name}, the corpse's outfit's")), Transform::default(), Visibility::default())).id();
                    commands.entity(self.root).add_child(entity);
                    entity
                })
                .collect();
            let theirs = rig.bones.iter().map(|bone| self.at(&bone.path)).collect();
            self.outfits.push(crate::death::CorpseOutfit { item: item.to_owned(), rig: rig.clone(), theirs, rest: rest.to_vec(), ours: ours.clone() });
            return ours;
        }
        let theirs: Vec<Move> = rig
            .bones
            .iter()
            .zip(rest)
            .map(|(bone, rest)| {
                let moved = self.at(&bone.path).map_or(Affine3A::IDENTITY, |index| self.world[index]) * *rest;
                Move { axes: [moved.matrix3.x_axis.to_array(), moved.matrix3.y_axis.to_array(), moved.matrix3.z_axis.to_array()], shift: moved.translation.to_array() }
            })
            .collect();
        rig.pose(&theirs)
            .into_iter()
            .zip(&rig.bones)
            .map(|(moved, bone)| {
                let turn = Quat::from_mat3(&Mat3::from_cols(moved.axes[0].into(), moved.axes[1].into(), moved.axes[2].into())).normalize();
                let name = bone.path.rsplit('/').next().unwrap_or(&bone.path);
                let entity = commands.spawn((Name::new(format!("{name}, the model's outfit's")), Transform { translation: moved.shift.into(), rotation: turn, scale: Vec3::ONE }, Visibility::default())).id();
                commands.entity(self.root).add_child(entity);
                entity
            })
            .collect()
    }

    /// An object of a model hung on a bone (what rides on the head), with
    /// its own place: an entity of the figure's, shown as `original` is
    /// where it has one.
    pub fn hung(&mut self, commands: &mut Commands, parent: Option<Entity>, transform: Transform, visibility: Visibility, original: Option<Entity>) -> Entity {
        let entity = commands.spawn((transform, visibility)).id();
        if let Some(original) = original {
            self.twin(commands, entity, original);
        }
        commands.entity(parent.unwrap_or(self.root)).add_child(entity);
        entity
    }

    /// A mesh drawn as it lies, on such an object.
    pub fn rigid(&mut self, commands: &mut Commands, on: Entity, mesh: &Handle<Mesh>, material: &Handle<StandardMaterial>) -> Entity {
        let piece = commands.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(material.clone()), Transform::default(), self.layer())).id();
        commands.entity(on).add_child(piece);
        self.keep(mesh, material);
        piece
    }

    /// The corpse as built, for `death`: its skeleton by path, and its
    /// outfits.
    pub fn finish_corpse(self, commands: &mut Commands) {
        let parents = self.paths.iter().map(|path| path.rfind('/').and_then(|at| self.by_path.get(&path[..at]).copied())).collect();
        info!("the corpse: {} bones, {} pieces, {} outfits", self.bones.len(), self.pieces, self.outfits.len());
        commands.insert_resource(crate::death::Corpse { root: self.root, paths: self.paths, bones: self.bones, parents, outfits: self.outfits });
    }

    /// Its camera and light, and the resource that draws it.
    pub fn finish(self, commands: &mut Commands) {
        let layer = RenderLayers::layer(LAYER);
        // The view takes in the whole of it, the camera level with a point
        // a metre up: as far above that as the head and what is on it, or
        // as far below as the feet, whichever is more.
        let heights = self.world.iter().map(|bone| bone.translation.y);
        let (low, high) = heights.fold((f32::MAX, f32::MIN), |(low, high), y| (low.min(y), high.max(y)));
        let (low, high) = if low <= high { (low, high + HEAD_ROOM) } else { (0.0, 1.9) };
        let half_height = (high - UP).max(UP - low) + MARGIN;
        let turn = convert::rotation(TURN);
        let eye = PLACE + turn * Vec3::new(0.0, UP, -AHEAD);
        let camera = commands
            .spawn((
                Name::new("Character model's camera"),
                Camera3d::default(),
                // As the world's own camera draws but for its picture: the
                // same kinds of pipelines, built once for both.
                Camera { is_active: false, order: -4, clear_color: ClearColorConfig::Custom(Color::NONE), ..default() },
                // Bevy's own clipping plane at the near plane too (it is
                // 0.1 m unless told).
                Projection::Perspective(PerspectiveProjection {
                    fov: 2.0 * (half_height / AHEAD).atan(),
                    near: 0.5,
                    near_clip_plane: Vec4::new(0.0, 0.0, -1.0, -0.5),
                    far: AHEAD * 3.0,
                    ..default()
                }),
                Transform::from_translation(eye).looking_at(PLACE + Vec3::Y * UP, Vec3::Y),
                layer.clone(),
            ))
            .id();
        commands.spawn((
            Name::new("Character model's light"),
            DirectionalLight { illuminance: LIGHT_LUX, shadows_enabled: false, ..default() },
            Transform::from_translation(LIGHT_FROM).looking_at(Vec3::ZERO, Vec3::Y),
            layer,
        ));
        info!("the character model: {} bones, {} pieces, {} meshes and {} materials of the body's own", self.bones.len(), self.pieces, self.meshes.len(), self.materials.len());
        commands.insert_resource(Figure {
            camera,
            meshes: self.meshes,
            materials: self.materials,
            waiting: false,
            on_card: false,
            wanted: true,
            drawing: None,
            size: UVec2::ZERO,
        });
    }
}

pub struct FigurePlugin;

impl Plugin for FigurePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FigureImage>().add_systems(Update, (mirror, draw).chain().after(crate::body_view::show_outfit).after(crate::body_view::show_worn));
    }
}

/// Shows each piece of the figure as its piece of the body is shown, and
/// asks for a drawing when that changes.
fn mirror(figure: Option<ResMut<Figure>>, originals: Query<&Visibility, Without<Twin>>, mut twins: Query<(&Twin, &mut Visibility)>) {
    let Some(mut figure) = figure else { return };
    for (twin, mut visibility) in &mut twins {
        let Ok(original) = originals.get(twin.0) else { continue };
        if *visibility != *original {
            *visibility = *original;
            figure.wanted = true;
        }
    }
}

/// The picture the size of the gear page's panel in the window's pixels;
/// and a drawing, when one is wanted and the figure is all on the graphics
/// card: the camera on for a few frames, and then off.
#[allow(clippy::too_many_arguments)]
fn draw(
    figure: Option<ResMut<Figure>>,
    mut image: ResMut<FigureImage>,
    mut images: ResMut<Assets<Image>>,
    materials: Res<Assets<StandardMaterial>>,
    uploads: Res<UploadWatch>,
    waiting: Res<crate::PipelinesWaiting>,
    carried: Res<Carried>,
    window: Query<&Window, With<PrimaryWindow>>,
    mut cameras: Query<(&mut Camera, &mut RenderTarget)>,
) {
    let Some(mut figure) = figure else { return };
    let figure = &mut *figure;
    if !figure.waiting {
        figure.waiting = true;
        let pictures = figure.materials.iter().filter_map(|material| materials.get(material)).flat_map(pictures_of).map(|picture| picture.id()).collect();
        uploads.wait(ON_CARD.to_owned(), pictures, figure.meshes.iter().map(Handle::id).collect());
    }
    if !figure.on_card && !uploads.take(|name| name == ON_CARD).is_empty() {
        figure.on_card = true;
    }
    let Ok((mut camera, mut render_target)) = cameras.get_mut(figure.camera) else { return };
    // A picture of the panel's size in the window's own pixels: made again
    // when that changes, and drawn on.
    if let Ok(window) = window.single() {
        let panel = crate::inventory_view::gear_panel(&carried);
        let scale = window.height() / crate::inventory_view::SCREEN_HEIGHT * window.scale_factor();
        let size = UVec2::new((panel[0] * scale).round().max(1.0) as u32, (panel[1] * scale).round().max(1.0) as u32);
        if size != figure.size {
            figure.size = size;
            // With nothing in it to send, so that it is on the card at once
            // (`icons.rs`, `ANSWER_FRAMES_AT_MOST`); the camera clears it.
            let mut target = Image::new_uninit(Extent3d { width: size.x, height: size.y, depth_or_array_layers: 1 }, TextureDimension::D2, TextureFormat::Rgba8UnormSrgb, RenderAssetUsages::default());
            target.texture_descriptor.usage = TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::RENDER_ATTACHMENT;
            let target = images.add(target);
            if let Some(old) = image.0.replace(target.clone()) {
                images.remove(&old);
            }
            *render_target = RenderTarget::Image(target.into());
            figure.wanted = true;
        }
    }
    if figure.wanted && figure.on_card && image.0.is_some() {
        figure.wanted = false;
        figure.drawing = Some(0);
    }
    let Some(frames) = figure.drawing.as_mut() else { return };
    *frames += 1;
    let done = (*frames > DRAW_FRAMES && waiting.0.load(std::sync::atomic::Ordering::Relaxed) == 0) || *frames > DRAW_FRAMES_AT_MOST;
    if done {
        figure.drawing = None;
    }
    if camera.is_active == done {
        camera.is_active = !done;
    }
}

/// Every picture a material draws with.
fn pictures_of(material: &StandardMaterial) -> Vec<Handle<Image>> {
    [&material.base_color_texture, &material.normal_map_texture, &material.metallic_roughness_texture, &material.occlusion_texture, &material.emissive_texture].into_iter().flatten().cloned().collect()
}
