//! The level-of-detail specs, pieces and bands, and the levels picked for each view.

use super::*;

/// A group of levels of detail as the app draws by it: its size (the
/// group's, before its object's scale), its reference point in its
/// anchor's own axes (Bevy's), each level's least share of the view's
/// height (the game's rule's, kept for the log), and TRP's measure: each
/// level's deviation from the fullest in its object's own axes (empty:
/// unmeasured, the fullest at any distance).
#[derive(Debug, Clone, PartialEq)]
pub struct LodSpec {
    pub size: f32,
    pub reference: Vec3,
    pub heights: Vec<f32>,
    pub deviation: Vec<f32>,
}

impl LodSpec {
    /// A model's groups, which switch together (a weapon's parts with their
    /// weapon, a thing's with the thing: `trp_sim::lod::together`), each
    /// placed by its own object (its reference point in that object's
    /// axes).
    pub fn bundle<'a>(groups: impl Iterator<Item = &'a trp_data::drawing::lod::LodGroup>) -> Vec<Arc<Self>> {
        let groups: Vec<&trp_data::drawing::lod::LodGroup> = groups.collect();
        let together = trp_sim::characters::lod::together(&groups.iter().map(|group| group.deviation.clone()).collect::<Vec<_>>());
        groups.iter().zip(together).map(|(group, deviation)| Arc::new(Self { size: group.size, reference: convert::position(group.reference), heights: group.heights(), deviation })).collect()
    }

    /// A scene's props' groups (`trp_data::scene_props::Props::lods`), each
    /// by the prop's node it is on, placed by that node's own object; a
    /// prop's groups switch together.
    pub fn of_props(lods: &[trp_data::unity::extract::HierarchyLod]) -> Vec<(usize, Arc<Self>)> {
        lods.iter().map(|lod| lod.node).zip(Self::bundle(lods.iter().map(|lod| &*lod.group))).collect()
    }

    /// A prop's piece's tag (`LodPiece`), from its node's group and levels
    /// among `lods` (`of_props`), its group placed by the entity made for
    /// the group's node (`entity_of`); none where it has no group, or its
    /// group's object was not made (then drawn at any distance).
    pub fn prop_piece(lod: Option<trp_data::unity::extract::NodeLod>, lods: &[(usize, Arc<Self>)], entity_of: &dyn Fn(usize) -> Option<Entity>) -> Option<LodPiece> {
        let lod = lod?;
        let (node, group) = lods.get(lod.group)?;
        Some(LodPiece { group: group.clone(), anchor: Some(entity_of(*node)?), levels: lod.levels })
    }

    /// A prop's groups for the log: how many, each one's levels and
    /// deviations, and how many of its objects are tagged.
    pub fn props_said(lods: &[(usize, Arc<Self>)], tagged: &[Option<trp_data::unity::extract::NodeLod>]) -> String {
        if lods.is_empty() {
            return "no levels of detail".to_owned();
        }
        let levels: Vec<String> = lods.iter().map(|(_, group)| format!("{} m: {:?}, deviations {:?} m", group.size, group.heights, group.deviation)).collect();
        format!("{} groups of levels of detail ({}), {} objects tagged", lods.len(), levels.join("; "), tagged.iter().flatten().count())
    }

    /// A group whose object is hung under another with the place and turn
    /// given (the game's axes): a part of the body's, under the player's
    /// mesh transform (`trp_data::player::mesh_transform`), so that its
    /// reference point is in the character's own axes, where the app's
    /// skinned pieces are.
    pub fn through(group: &trp_data::drawing::lod::LodGroup, place: [f32; 3], turn: [f32; 4]) -> Arc<Self> {
        let turned = convert::rotation(turn) * convert::position(group.reference);
        Arc::new(Self { size: group.size, reference: convert::position(place) + turned, heights: group.heights(), deviation: group.deviation.clone() })
    }
}

/// A mesh drawn at some levels of detail of a group: drawn by a view only
/// where its band (`LodBand`) holds that view's pixel at it. The group is
/// placed by the anchor's transform (its object's: a rigid model's object),
/// or, with none, by the mesh's own (a skinned piece's: the character's
/// root, where it hangs with no place of its own).
#[derive(Component, Clone)]
pub struct LodPiece {
    pub group: Arc<LodSpec>,
    pub anchor: Option<Entity>,
    pub levels: u8,
}

/// A piece's band as the shader has it (`trp_sim::lod::untag` of its tag)
/// and the top object it is drawn under (its whole thing); made as it is
/// first seen, again when the tolerance changes.
#[derive(Component, Clone, Copy, Debug)]
pub struct LodBand {
    pub band: (f32, f32),
    pub root: Entity,
    tolerance: f32,
}

/// On the main camera: the view whose levels a light's own shadow views
/// take, and whose pick a body's part is said in the log by.
#[derive(Component, Clone, Copy)]
pub struct LodMainView;

/// The bias as the options have it (`OBJECT LOD QUALITY`), else the
/// game's own unless set.
fn bias_now(state: Option<&crate::screens::options_view::OptionsState>) -> f32 {
    state.map_or(trp_data::setup::install::LOD_BIAS_DEFAULT, |state| state.graphics.lod_bias)
}

/// The pixels of deviation allowed: `--lod-pixels`'s, else the options'
/// `OBJECT LOD QUALITY`'s (`trp_sim::lod::tolerance`).
fn tolerance_now(state: Option<&crate::screens::options_view::OptionsState>, options: &crate::Options) -> f32 {
    options.lod_pixels.unwrap_or_else(|| trp_sim::characters::lod::tolerance(bias_now(state)))
}

/// The visibility range that switches Bevy's dithered pipeline on for a
/// tagged mesh and never culls it (its fade's end past any distance): the
/// shader takes the mesh's tag in its place (`patch`).
pub(super) fn carrier() -> VisibilityRange {
    VisibilityRange { start_margin: 0.0..0.0, end_margin: 1.0e30..f32::MAX, use_aabb: false }
}

/// Each whole thing's largest group's world size, and the piece of it
/// that is measured from (the one with that group): by its top object.
#[derive(Resource, Default)]
pub(crate) struct Wholes(pub(super) HashMap<Entity, (f32, Entity)>);

impl Wholes {
    /// A whole thing's largest group's world size, by its top object, once
    /// a piece of it has been seen (the dummies' posing goes by it:
    /// `dummy::run_dummies`).
    pub(crate) fn size_of(&self, root: Entity) -> Option<f32> {
        self.0.get(&root).map(|&(size, _)| size)
    }
}

/// The pixels of deviation allowed now (`tolerance_now`), for what else
/// goes by `OBJECT LOD QUALITY` as the levels do (the dummies' posing).
pub(crate) fn tolerance(state: Option<&crate::screens::options_view::OptionsState>, options: &crate::Options) -> f32 {
    tolerance_now(state, options)
}

/// The largest absolute scale of a transform.
fn largest_scale(transform: &GlobalTransform) -> f32 {
    let m = transform.affine().matrix3;
    m.x_axis.length().max(m.y_axis.length()).max(m.z_axis.length())
}

/// Each piece's band, its tag and its carrier: as it is first seen, and
/// all again when the tolerance changes. Its whole thing is its top object.
#[allow(clippy::type_complexity)]
pub(super) fn band_pieces(
    mut commands: Commands,
    state: Option<Res<crate::screens::options_view::OptionsState>>,
    options: Res<crate::Options>,
    fresh: Query<(Entity, &LodPiece, &GlobalTransform, Option<&LodBand>)>,
    tags: Query<&MeshTag>,
    added: Query<Entity, Added<LodPiece>>,
    anchors: Query<&GlobalTransform>,
    parents: Query<&ChildOf>,
    mut wholes: ResMut<Wholes>,
    mut last: Local<Option<f32>>,
    mut frames: Local<u32>,
    mut origins: Local<HashMap<(usize, Entity), (Vec3, u8, bool)>>,
) {
    let tolerance = tolerance_now(state.as_deref(), &options);
    let all = *last != Some(tolerance);
    *last = Some(tolerance);
    let mut band_of = |entity: Entity, piece: &LodPiece, own: &GlobalTransform| {
        let anchor = piece.anchor.and_then(|anchor| anchors.get(anchor).ok()).unwrap_or(own);
        let scale = largest_scale(anchor);
        let switches = trp_sim::characters::lod::switches(&piece.group.deviation, piece.group.heights.len(), scale, tolerance);
        let tag = trp_sim::characters::lod::tag(trp_sim::characters::lod::band(&switches, piece.levels));
        let mut root = entity;
        while let Ok(parent) = parents.get(root) {
            root = parent.parent();
        }
        let size = piece.group.size * scale;
        let whole = wholes.0.entry(root).or_insert((size, entity));
        if size > whole.0 {
            *whole = (size, entity);
        }
        // The decals' bits of the tag (its material's stencil type:
        // `decal_view::tag_by_stencil`) kept as they are.
        let kept = tags.get(entity).map_or(0, |found| found.0 & !trp_sim::characters::lod::BAND_BITS);
        commands.entity(entity).insert((LodBand { band: trp_sim::characters::lod::untag(tag), root, tolerance }, MeshTag(kept | tag), carrier()));
    };
    if all {
        for (entity, piece, own, _) in &fresh {
            band_of(entity, piece, own);
        }
    } else {
        for entity in &added {
            if let Ok((entity, piece, own, band)) = fresh.get(entity) {
                if band.is_none_or(|band| band.tolerance != tolerance) {
                    band_of(entity, piece, own);
                }
            }
        }
    }
    // Where a group's levels do not share their origin (the shader fades
    // each piece by its own place, so two levels apart there fade not quite
    // through each other's pixels): said once a group, by its first level's
    // piece's place against another level's.
    for entity in &added {
        let Ok((_, piece, own, _)) = fresh.get(entity) else { continue };
        let Some(anchor) = piece.anchor else { continue };
        let key = (Arc::as_ptr(&piece.group) as usize, anchor);
        let first = *origins.entry(key).or_insert((own.translation(), piece.levels, false));
        if first.1 != piece.levels && !first.2 {
            let apart = first.0.distance(own.translation());
            if apart > 0.01 {
                debug!("levels of detail: a group's levels {:#b} and {:#b} {apart:.3} m apart at their origins (of {:.2} m)", first.1, piece.levels, piece.group.size);
            }
            origins.insert(key, (first.0, first.1, true));
        }
    }
    if origins.len() > 20_000 {
        origins.clear();
    }
    // Wholes gone with their pieces, now and then.
    *frames += 1;
    if *frames % 600 == 0 {
        wholes.0.retain(|_, (_, piece)| fresh.contains(*piece));
    }
}

/// A view's pixel at a place: its height in pixels, its projection's
/// [1][1], whether it is orthographic, its place.
#[derive(Clone, Copy)]
pub(super) struct ViewPixel {
    pixels: f32,
    clip_y: f32,
    orthographic: bool,
    eye: Vec3,
}

impl ViewPixel {
    fn at(&self, place: Vec3) -> f32 {
        trp_sim::characters::lod::pixel(place.distance(self.eye), self.pixels, self.clip_y, self.orthographic)
    }
}

/// What the log last said, so that it speaks only as it changes.
#[derive(Default)]
pub(super) struct Said {
    counts: HashMap<Entity, (usize, usize, usize, usize, i32)>,
    parts: HashMap<(usize, Entity), usize>,
    cameras: usize,
}

/// Each view's levels: what of the tagged it does not draw, taken off its
/// list of what it sees; the sun's shadows cast for it by the coarser of
/// its own and the cascade's texel; the lights' own views by the main
/// camera's.
#[allow(clippy::type_complexity)]
pub(super) fn pick_levels(
    state: Option<Res<crate::screens::options_view::OptionsState>>,
    options: Res<crate::Options>,
    pieces: Query<(&LodPiece, &GlobalTransform, Option<&LodBand>)>,
    anchors: Query<&GlobalTransform>,
    mut cameras: Query<(Entity, &Camera, &GlobalTransform, &Projection, &mut VisibleEntities, Has<LodMainView>)>,
    all_cameras: Query<(), With<Camera>>,
    mut suns: Query<(&mut CascadesVisibleEntities, &Cascades)>,
    mut cubes: Query<&mut CubemapVisibleEntities>,
    mut spots: Query<&mut VisibleMeshEntities>,
    wholes: Res<Wholes>,
    mut said: Local<Said>,
    names: Query<&Name>,
) {
    if pieces.is_empty() {
        return;
    }
    let tolerance = tolerance_now(state.as_deref(), &options);
    let debug = bevy::log::tracing::enabled!(bevy::log::Level::DEBUG);
    let camera_count = all_cameras.iter().count();
    if camera_count > 32 && said.cameras <= 32 {
        // Bevy's range check counts 32 views; a tagged mesh is out of range
        // of the rest.
        warn!("levels of detail: {camera_count} cameras, more than the 32 Bevy's visibility ranges serve");
    }
    said.cameras = camera_count;
    // A piece's band: made, or made here the frame it first shows.
    let band_now = |piece: &LodPiece, own: &GlobalTransform, band: Option<&LodBand>| -> (f32, f32) {
        if let Some(band) = band.filter(|band| band.tolerance == tolerance) {
            return band.band;
        }
        let anchor = piece.anchor.and_then(|anchor| anchors.get(anchor).ok()).unwrap_or(own);
        let switches = trp_sim::characters::lod::switches(&piece.group.deviation, piece.group.heights.len(), largest_scale(anchor), tolerance);
        trp_sim::characters::lod::untag(trp_sim::characters::lod::tag(trp_sim::characters::lod::band(&switches, piece.levels)))
    };
    let mut views: HashMap<Entity, ViewPixel> = HashMap::new();
    let mut main: Option<ViewPixel> = None;
    for (camera_entity, camera, place, projection, mut seen, is_main) in &mut cameras {
        if !camera.is_active {
            continue;
        }
        let Some(size) = camera.physical_viewport_size() else { continue };
        let clip = projection.get_clip_from_view();
        let view = ViewPixel { pixels: size.y.max(1) as f32, clip_y: clip.y_axis.y, orthographic: clip.w_axis.w == 1.0, eye: place.translation() };
        views.insert(camera_entity, view);
        if is_main {
            main = Some(view);
        }
        // Each whole thing gone or not in this view, as asked.
        let mut gone: HashMap<Entity, bool> = HashMap::new();
        let (mut tagged, mut drawn, mut fading, mut culled, mut game) = (0, 0, 0, 0, 0);
        for list in seen.entities.values_mut() {
            list.retain(|&entity| {
                let Ok((piece, own, band)) = pieces.get(entity) else { return true };
                tagged += 1;
                let root = band.map(|band| band.root);
                if let Some(root) = root {
                    let whole_gone = *gone.entry(root).or_insert_with(|| {
                        wholes.0.get(&root).is_some_and(|&(size, probe)| {
                            let at = pieces.get(probe).map_or(own.translation(), |(_, probe, _)| probe.translation());
                            trp_sim::characters::lod::culled_whole(size, view.at(at))
                        })
                    });
                    if whole_gone {
                        culled += 1;
                        return false;
                    }
                }
                let band = band_now(piece, own, band);
                let pixel = view.at(own.translation());
                let keep = trp_sim::characters::lod::drawn(band, pixel);

                if keep {
                    drawn += 1;
                    if trp_sim::characters::lod::dither(band, pixel) != 0 {
                        fading += 1;
                    }
                    // A body's part's level said as the main view's changes
                    // (the parts are the groups of 1.5 m and more), for a
                    // run's log to be checked against the rule (with
                    // `RUST_LOG=info,trp::lod_view=debug`).
                    if debug && is_main && piece.group.size >= 1.5 && trp_sim::characters::lod::dither(band, pixel) == 0 {
                        let key = (Arc::as_ptr(&piece.group) as usize, piece.anchor.unwrap_or(entity));
                        let level = piece.levels.trailing_zeros() as usize;
                        if said.parts.insert(key, level) != Some(level) {
                            debug!("levels of detail: a part of {:.1} m (deviations {:?} m) at {:.2} m, {:.0} pixels, a pixel {:.2} mm: level {level}", piece.group.size, piece.group.deviation, own.translation().distance(view.eye), view.pixels, pixel * 1e3);
                        }
                    }
                }
                // The game's rule's count beside it, for the log alone.
                if debug {
                    let anchor = piece.anchor.and_then(|anchor| anchors.get(anchor).ok()).unwrap_or(own);
                    let distance = anchor.transform_point(piece.group.reference).distance(view.eye);
                    let fov = if is_main { options.field_of_view } else { 2.0 * (1.0 / view.clip_y).atan().to_degrees() };
                    let pick = trp_sim::characters::lod::pick(piece.group.size, largest_scale(anchor), distance, fov, bias_now(state.as_deref()), &piece.group.heights);
                    if pick.is_some_and(|level| level < 8 && piece.levels & (1 << level) != 0) {
                        game += 1;
                    }
                }
                keep
            });
        }
        // How many of the tagged each view draws, said as it changes, with
        // the game's rule's count beside it.
        if debug {
            let count = (tagged, drawn, fading, culled, (view.pixels / view.clip_y * 10.0) as i32);
            if said.counts.insert(camera_entity, count) != Some(count) {
                let name = names.get(camera_entity).map_or(if is_main { "the main camera" } else { "a camera" }, |name| name.as_str());
                debug!("levels of detail: {name} ({:.0} pixels, {:.1} degrees, {tolerance:.2} pixels allowed) draws {drawn} of {tagged} tagged pieces in view ({fading} fading, {culled} of whole things gone); the game's rule would draw {game}", view.pixels, 2.0 * (1.0 / view.clip_y).atan().to_degrees());
            }
        }
    }
    // A piece's steady level's band holds a pixel (no fade: Bevy's shadow
    // pass draws no dither).
    let steady = |band: (f32, f32), pixel: f32| pixel >= band.0 && pixel < band.1;
    // The sun's shadows, cast for each view: the coarser of the view's pixel
    // at the piece and the cascade's texel.
    for (mut sun, cascades) in &mut suns {
        for (view_entity, lists) in sun.entities.iter_mut() {
            let Some(view) = views.get(view_entity).copied() else { continue };
            let texels: Vec<f32> = cascades.cascades.get(view_entity).map(|cascades| cascades.iter().map(|cascade| cascade.texel_size).collect()).unwrap_or_default();
            for (n, cascade) in lists.iter_mut().enumerate() {
                let texel = texels.get(n).copied().unwrap_or(0.0);
                cascade.entities.retain(|&entity| {
                    let Ok((piece, own, band)) = pieces.get(entity) else { return true };
                    let pixel = view.at(own.translation());
                    if band.and_then(|band| wholes.0.get(&band.root)).is_some_and(|&(size, _)| trp_sim::characters::lod::culled_whole(size, pixel)) {
                        return false;
                    }
                    steady(band_now(piece, own, band), pixel.max(texel))
                });
            }
        }
    }
    // A light's own views: the main camera's steady levels.
    let Some(main) = main else { return };
    let keep = |entity: &Entity| pieces.get(*entity).map_or(true, |(piece, own, band)| steady(band_now(piece, own, band), main.at(own.translation())));
    for mut cube in &mut cubes {
        for face in cube.iter_mut() {
            face.entities.retain(keep);
        }
    }
    for mut spot in &mut spots {
        spot.entities.retain(keep);
    }
}
