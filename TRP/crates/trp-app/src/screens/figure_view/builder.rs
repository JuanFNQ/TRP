//! The figure's skeleton, the Hangs trait and the builder that makes figures.

use super::*;

/// A skeleton of the figure's or the corpse's: its root, its bones by path,
/// and what its pieces are drawn as. Kept once built, for pieces put on
/// later (`clothing`).
pub struct Skeleton {
    pub(super) made: Made,
    pub(super) root: Entity,
    by_path: HashMap<String, usize>,
    pub(super) bones: Vec<Entity>,
}

impl Skeleton {
    /// The render layer its pieces are drawn on.
    pub(super) fn layer(&self) -> RenderLayers {
        match self.made {
            Made::Figure => RenderLayers::layer(LAYER),
            Made::Overall => RenderLayers::layer(OVERALL_LAYER),
            Made::Corpse | Made::Dummy(_) => RenderLayers::layer(crate::player::body_view::SEEN),
        }
    }

    /// A piece of it marked as the twin of a piece of the body, out of the
    /// engine's work while hidden as that one is (`park`).
    pub(super) fn twin(&self, commands: &mut Commands, piece: Entity, original: Entity) {
        match self.made {
            Made::Figure | Made::Overall => commands.entity(piece).insert((Twin(original), crate::platform::park::Park)),
            Made::Corpse => commands.entity(piece).insert((crate::player::death::CorpseTwin(original), crate::platform::park::Park)),
            Made::Dummy(which) => commands.entity(piece).insert((crate::world::dummy::DummyTwin { which }, crate::platform::park::Park)),
        };
    }

    /// The bone a mesh's bone is drawn on: that one, or the nearest above it
    /// the skeleton has.
    pub(super) fn at(&self, path: &str) -> Option<usize> {
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

    /// The bone whose path ends so, if the skeleton has it (as
    /// `Builder::bone`, for a model on the head hung later).
    pub fn bone(&self, end: &str) -> Option<Entity> {
        self.by_path.iter().find(|(path, _)| path.ends_with(end)).map(|(_, &index)| self.bones[index])
    }

    /// An object of a model hung on a bone (what rides on the head), or on
    /// such an object, with its own place, shown as `original` is where it
    /// has one.
    pub fn hung(&self, commands: &mut Commands, parent: Option<Entity>, transform: Transform, visibility: Visibility, original: Option<Entity>) -> Entity {
        let entity = commands.spawn((transform, visibility)).id();
        if let Some(original) = original {
            self.twin(commands, entity, original);
        }
        commands.entity(parent.unwrap_or(self.root)).add_child(entity);
        entity
    }

    /// A mesh drawn as it lies, on such an object.
    pub fn rigid(&self, commands: &mut Commands, on: Entity, mesh: &Handle<Mesh>, material: &Handle<StandardMaterial>) -> Entity {
        let piece = commands.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(material.clone()), Transform::default(), self.layer())).id();
        commands.entity(on).add_child(piece);
        piece
    }

    /// A skinned piece of the body, on the bones it names.
    pub fn skinned(&self, commands: &mut Commands, original: Entity, drawn: (&Handle<Mesh>, &Handle<StandardMaterial>), poses: &Handle<SkinnedMeshInverseBindposes>, bone_paths: &[String], hidden: bool) -> Entity {
        let joints = bone_paths.iter().map(|path| self.joint(path)).collect();
        self.skinned_on(commands, original, drawn, poses, joints, hidden)
    }

    /// A skinned piece on joints of its own (an outfit's, `Builder::outfit`).
    pub fn skinned_on(&self, commands: &mut Commands, original: Entity, (mesh, material): (&Handle<Mesh>, &Handle<StandardMaterial>), poses: &Handle<SkinnedMeshInverseBindposes>, joints: Vec<Entity>, hidden: bool) -> Entity {
        let visibility = if hidden { Visibility::Hidden } else { Visibility::Inherited };
        let name = match self.made {
            Made::Corpse => "Corpse, a piece",
            Made::Dummy(_) => "Dummy, a piece",
            Made::Figure => "Character model, a piece",
            Made::Overall => "Character model, the OVERALL tab's, a piece",
        };
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
        piece
    }
}

/// What a model on the head is hung on beside the body: a copy as it is
/// built with the body (`Builder`), or kept for a part fitted later
/// (`Skeleton`; `body_view::fit_head_parts`).
pub trait Hangs {
    fn hang(&mut self, commands: &mut Commands, parent: Option<Entity>, transform: Transform, visibility: Visibility, original: Option<Entity>) -> Entity;
    fn lay(&mut self, commands: &mut Commands, on: Entity, mesh: &Handle<Mesh>, material: &Handle<StandardMaterial>) -> Entity;
    /// A mesh it may be drawn with later (a piece's second form,
    /// `body_view::Fitted`): the figure built waits for it on the graphics
    /// card with the rest. (A copy kept is told by `Figure::wait_for`.)
    fn keep_mesh(&mut self, _mesh: &Handle<Mesh>) {}
}

impl Hangs for Builder {
    fn hang(&mut self, commands: &mut Commands, parent: Option<Entity>, transform: Transform, visibility: Visibility, original: Option<Entity>) -> Entity {
        self.hung(commands, parent, transform, visibility, original)
    }
    fn lay(&mut self, commands: &mut Commands, on: Entity, mesh: &Handle<Mesh>, material: &Handle<StandardMaterial>) -> Entity {
        self.rigid(commands, on, mesh, material)
    }
    fn keep_mesh(&mut self, mesh: &Handle<Mesh>) {
        Builder::keep_mesh(self, mesh);
    }
}

impl Hangs for Skeleton {
    fn hang(&mut self, commands: &mut Commands, parent: Option<Entity>, transform: Transform, visibility: Visibility, original: Option<Entity>) -> Entity {
        self.hung(commands, parent, transform, visibility, original)
    }
    fn lay(&mut self, commands: &mut Commands, on: Entity, mesh: &Handle<Mesh>, material: &Handle<StandardMaterial>) -> Entity {
        self.rigid(commands, on, mesh, material)
    }
}

/// The corpse's skeleton, kept once built (`Skeleton`).
#[derive(Resource)]
pub struct CorpseSkeleton(pub Skeleton);

/// Builds the figure beside the body, piece by piece as `body_view::
/// spawn_body` makes the body's own. The corpse the character becomes
/// (`death`) is built the same way: the same meshes on a skeleton of its
/// own, hidden until it is wanted.
pub struct Builder {
    pub(super) skeleton: Skeleton,
    pub(super) paths: Vec<String>,
    /// Each bone in the root's axes, in the engine's.
    pub(super) world: Vec<Affine3A>,
    pub(super) meshes: Vec<Handle<Mesh>>,
    pub(super) materials: Vec<Handle<StandardMaterial>>,
    pieces: usize,
    /// The corpse's outfits, each to be posed on its skeleton as it falls.
    pub(super) outfits: Vec<crate::player::death::CorpseOutfit>,
}

/// What a builder makes.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Made {
    /// The gear page's character model, on a layer of its own.
    Figure,
    /// The OVERALL tab's own (`OVERALL FIGURE`), on another.
    Overall,
    /// The corpse, in the world with the body.
    Corpse,
    /// A dummy on the range (`dummy`), in the world: which of them.
    Dummy(usize),
}

impl Builder {
    /// The skeleton, standing: an entity for each bone of `paths` (a
    /// parent before what hangs from it), put where `pose` has it.
    pub fn new(commands: &mut Commands, paths: &[String], pose: &trp_sim::locomotion::body::Pose) -> Self {
        // Parked while no picture of it is drawn or about to be
        // (`camera_draw::show_hung`): with `TARKOV UI` and the OVERALL
        // tab's own figure, never.
        let root = commands.spawn((Name::new("Character model"), Transform { translation: PLACE, rotation: convert::rotation(TURN), ..default() }, Visibility::default(), crate::platform::park::Park)).id();
        Self::on(Made::Figure, commands, root, paths, |index| convert::transform(pose[index].0, pose[index].1, [1.0; 3]))
    }

    /// The OVERALL tab's own figure's skeleton (`OVERALL FIGURE`): the menu
    /// player's bones (`FigureData::paths`, every bone after its parent),
    /// standing as its figure is made with nothing in hand, where the gear
    /// page's figure stands, on a layer of its own.
    pub fn overall(commands: &mut Commands, data: &trp_sim::characters::figure::FigureData) -> Self {
        let made = trp_sim::characters::figure::Figure::new(data, trp_sim::characters::figure::kind::NOTHING, None, 0);
        let pose = made.pose();
        // Parked while its camera is off (`camera_draw::show_hung`), with
        // all on it: its skeleton, its clothing, its shadow, what hangs
        // from it and what it holds.
        let root = commands.spawn((Name::new("Character model, the OVERALL tab's"), Transform { translation: PLACE, rotation: convert::rotation(TURN), ..default() }, Visibility::default(), crate::platform::park::Park)).id();
        Self::on(Made::Overall, commands, root, data.paths(), |index| convert::transform(pose[index].0, pose[index].1, [1.0; 3]))
    }

    /// The corpse's skeleton, at the world's origin and hidden: its bones
    /// are given the body's pose when the character dies.
    pub fn corpse(commands: &mut Commands, paths: &[String]) -> Self {
        // Hidden until a death, and out of the engine's work till then
        // (`park`).
        let root = commands.spawn((Name::new("Corpse"), Transform::default(), Visibility::Hidden, crate::platform::park::Park)).id();
        Self::on(Made::Corpse, commands, root, paths, |_| Transform::default())
    }

    /// A dummy's skeleton (`dummy`, the `which`th), standing as the figure
    /// stands, at `place` in the world, until its character poses it.
    pub fn dummy(commands: &mut Commands, paths: &[String], pose: &trp_sim::locomotion::body::Pose, place: Transform, which: usize) -> Self {
        let root = commands.spawn((Name::new(format!("Dummy {which}")), place, Visibility::default())).id();
        Self::on(Made::Dummy(which), commands, root, paths, |index| convert::transform(pose[index].0, pose[index].1, [1.0; 3]))
    }

    pub(super) fn on(made: Made, commands: &mut Commands, root: Entity, paths: &[String], local_of: impl Fn(usize) -> Transform) -> Self {
        let mut by_path = HashMap::with_capacity(paths.len());
        let (mut bones, mut world) = (Vec::with_capacity(paths.len()), Vec::with_capacity(paths.len()));
        let whose = match made {
            Made::Corpse => "the corpse's",
            Made::Dummy(_) => "the dummy's",
            Made::Figure => "the model's",
            Made::Overall => "the OVERALL model's",
        };
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
        let skeleton = Skeleton { made, root, by_path, bones };
        Self { skeleton, paths: paths.to_vec(), world, meshes: Vec::new(), materials: Vec::new(), pieces: 0, outfits: Vec::new() }
    }

    /// The bone whose path ends so, if the figure has it.
    pub fn bone(&self, end: &str) -> Option<Entity> {
        self.paths.iter().position(|path| path.ends_with(end)).map(|at| self.skeleton.bones[at])
    }

    /// A skinned piece of the body, on the bones it names.
    pub fn skinned(&mut self, commands: &mut Commands, original: Entity, drawn: (&Handle<Mesh>, &Handle<StandardMaterial>), poses: &Handle<SkinnedMeshInverseBindposes>, bone_paths: &[String], hidden: bool) -> Entity {
        let piece = self.skeleton.skinned(commands, original, drawn, poses, bone_paths, hidden);
        self.keep(drawn.0, drawn.1);
        piece
    }

    /// A skinned piece on joints of its own (an outfit's, `outfit`).
    pub fn skinned_on(&mut self, commands: &mut Commands, original: Entity, drawn: (&Handle<Mesh>, &Handle<StandardMaterial>), poses: &Handle<SkinnedMeshInverseBindposes>, joints: Vec<Entity>, hidden: bool) -> Entity {
        let piece = self.skeleton.skinned_on(commands, original, drawn, poses, joints, hidden);
        self.keep(drawn.0, drawn.1);
        piece
    }

    /// A mesh a piece of it may be drawn with later (`Hangs::keep_mesh`),
    /// waited for with the rest.
    pub fn keep_mesh(&mut self, mesh: &Handle<Mesh>) {
        if !self.meshes.contains(mesh) {
            self.meshes.push(mesh.clone());
        }
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
    pub fn outfit(&mut self, commands: &mut Commands, item: &str, rig: &trp_data::character::outfit::Rig, rest: &[Affine3A]) -> Vec<Entity> {
        use trp_data::character::outfit::Move;
        let root = self.skeleton.root;
        if self.skeleton.made == Made::Corpse {
            let ours: Vec<Entity> = rig
                .bones
                .iter()
                .map(|bone| {
                    let name = bone.path.rsplit('/').next().unwrap_or(&bone.path);
                    let entity = commands.spawn((Name::new(format!("{name}, the corpse's outfit's")), Transform::default(), Visibility::default())).id();
                    commands.entity(root).add_child(entity);
                    entity
                })
                .collect();
            let theirs = rig.bones.iter().map(|bone| self.skeleton.at(&bone.path)).collect();
            self.outfits.push(crate::player::death::CorpseOutfit { item: item.to_owned(), rig: rig.clone(), theirs, rest: rest.to_vec(), ours: ours.clone() });
            return ours;
        }
        let theirs: Vec<Move> = rig
            .bones
            .iter()
            .zip(rest)
            .map(|(bone, rest)| {
                let moved = self.skeleton.at(&bone.path).map_or(Affine3A::IDENTITY, |index| self.world[index]) * *rest;
                Move { axes: [moved.matrix3.x_axis.to_array(), moved.matrix3.y_axis.to_array(), moved.matrix3.z_axis.to_array()], shift: moved.translation.to_array() }
            })
            .collect();
        let ours: Vec<Entity> = rig
            .pose(&theirs)
            .into_iter()
            .zip(&rig.bones)
            .map(|(moved, bone)| {
                let turn = Quat::from_mat3(&Mat3::from_cols(moved.axes[0].into(), moved.axes[1].into(), moved.axes[2].into())).normalize();
                let name = bone.path.rsplit('/').next().unwrap_or(&bone.path);
                let entity = commands.spawn((Name::new(format!("{name}, the model's outfit's")), Transform { translation: moved.shift.into(), rotation: turn, scale: Vec3::ONE }, Visibility::default())).id();
                commands.entity(root).add_child(entity);
                entity
            })
            .collect();
        // The dummy's stood once, and kept to be posed as it falls; the
        // OVERALL tab's own figure's, to be posed as it moves.
        if matches!(self.skeleton.made, Made::Dummy(_) | Made::Overall) {
            let theirs = rig.bones.iter().map(|bone| self.skeleton.at(&bone.path)).collect();
            self.outfits.push(crate::player::death::CorpseOutfit { item: item.to_owned(), rig: rig.clone(), theirs, rest: rest.to_vec(), ours: ours.clone() });
        }
        ours
    }

    /// An object of a model hung on a bone (what rides on the head), with
    /// its own place: an entity of the figure's, shown as `original` is
    /// where it has one.
    pub fn hung(&mut self, commands: &mut Commands, parent: Option<Entity>, transform: Transform, visibility: Visibility, original: Option<Entity>) -> Entity {
        self.skeleton.hung(commands, parent, transform, visibility, original)
    }

    /// A mesh drawn as it lies, on such an object.
    pub fn rigid(&mut self, commands: &mut Commands, on: Entity, mesh: &Handle<Mesh>, material: &Handle<StandardMaterial>) -> Entity {
        self.keep(mesh, material);
        self.skeleton.rigid(commands, on, mesh, material)
    }

    /// The corpse as built, for `death`: its skeleton by path, and its
    /// outfits; and the skeleton kept for pieces put on later.
    pub fn finish_corpse(self, commands: &mut Commands) {
        let skeleton = self.skeleton;
        let parents = self.paths.iter().map(|path| path.rfind('/').and_then(|at| skeleton.by_path.get(&path[..at]).copied())).collect();
        info!("the corpse: {} bones, {} pieces, {} outfits", skeleton.bones.len(), self.pieces, self.outfits.len());
        commands.insert_resource(crate::player::death::Corpse { root: skeleton.root, paths: self.paths, bones: skeleton.bones.clone(), parents, outfits: self.outfits });
        commands.insert_resource(CorpseSkeleton(skeleton));
    }

    /// A dummy as built, for `dummy`: its skeleton by path with each bone's
    /// place in its parent as it stands, and its outfits; and the skeleton,
    /// kept for pieces put on later.
    pub fn finish_dummy(self) -> (crate::world::dummy::DummyRig, Skeleton) {
        let skeleton = self.skeleton;
        let parents: Vec<Option<usize>> = self.paths.iter().map(|path| path.rfind('/').and_then(|at| skeleton.by_path.get(&path[..at]).copied())).collect();
        let rest = parents.iter().zip(&self.world).map(|(parent, world)| Transform::from_matrix(Mat4::from(parent.map_or(Affine3A::IDENTITY, |p| self.world[p]).inverse() * *world))).collect();
        info!("a dummy: {} bones, {} pieces, {} outfits", skeleton.bones.len(), self.pieces, self.outfits.len());
        (crate::world::dummy::DummyRig { root: skeleton.root, paths: self.paths, bones: skeleton.bones.clone(), parents, rest, outfits: self.outfits }, skeleton)
    }

    /// Its camera for the gear page and its light, and the resource that
    /// draws it (the OVERALL tab's camera is made when first wanted); with
    /// `OVERALL FIGURE`, the OVERALL tab's own figure as built beside it
    /// (`overall`) and what runs it.
    pub fn finish(self, commands: &mut Commands, overall: Option<(Builder, trp_sim::characters::figure::FigureData)>) {
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
                // Its depth read where the glass is undithered (`glass_view`).
                Camera3d { depth_texture_usages: crate::effects::glass_view::depth_usages(), ..default() },
                // As the world's own camera draws but for its picture: the
                // same kinds of pipelines, built once for both.
                Camera { is_active: false, order: -4, clear_color: ClearColorConfig::Custom(Color::NONE), ..default() },
                // The range's own ambient (`FIGURE_AMBIENT`), whatever lights
                // the range.
                AmbientLight { color: Color::WHITE, brightness: FIGURE_AMBIENT, ..default() },
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
                RenderLayers::from_layers(&[LAYER, GEAR_LIGHTS]),
            ))
            .id();
        let _ = layer;
        commands.spawn((
            Name::new("Character model's light"),
            DirectionalLight { illuminance: LIGHT_LUX, shadows_enabled: false, ..default() },
            Transform::from_translation(LIGHT_FROM).looking_at(Vec3::ZERO, Vec3::Y),
            RenderLayers::layer(GEAR_LIGHTS),
        ));
        info!("the character model: {} bones, {} pieces, {} meshes and {} materials of the body's own", self.skeleton.bones.len(), self.pieces, self.meshes.len(), self.materials.len());
        let (mut meshes, mut materials) = (self.meshes, self.materials);
        let overall = overall.map(|(built, data)| {
            info!("the OVERALL tab's own figure: {} bones, {} pieces, {} outfits", built.skeleton.bones.len(), built.pieces, built.outfits.len());
            // (Its pieces draw with the gear page's meshes and materials,
            // and those its own outfits and later pieces bring.)
            for mesh in built.meshes {
                if !meshes.contains(&mesh) {
                    meshes.push(mesh);
                }
            }
            for material in built.materials {
                if !materials.contains(&material) {
                    materials.push(material);
                }
            }
            let ribcage = built.paths.iter().position(|path| path.ends_with("Base HumanRibcage")).map(|at| built.skeleton.bones[at]);
            Overall {
                bones: built.skeleton.bones.clone(),
                skeleton: built.skeleton,
                data,
                ribcage,
                outfits: built.outfits,
                wanted: None,
                made: false,
                running: None,
                clock: false,
                weapon: None,
                devices: Vec::new(),
                chambered: Vec::new(),
                frames: 0,
                warmed: false,
                showings: 0,
                held_item: None,
                fixed: Vec::new(),
                landmarks: Vec::new(),
                held_frames: 0,
            }
        });
        let shot = |camera: Option<Entity>| Shot { camera, wanted: true, size: UVec2::ZERO, yaw: YAW, shown: false };
        let (meshes, materials) = (meshes.iter().map(Handle::id).collect(), materials.iter().map(Handle::id).collect());
        commands.insert_resource(Figure { shots: [shot(Some(camera)), shot(None)], meshes, materials, waiting: false, on_card: false, drawing: None, skeleton: self.skeleton, overall });
    }
}
