//! The preview's zoom, its model put in place and its lights.

use super::*;

/// The preview's entities: its turning parent, the model shown (its holder
/// and its key), one made and not yet shown, each slot's bone in the shown
/// model in the turning parent's frame; the camera, its picture and its
/// lights; how many more frames the camera draws.
#[derive(Default)]
pub(super) struct Preview {
    pub(super) rotator: Option<Entity>,
    pub(super) holder: Option<Entity>,
    pub(super) key: Option<String>,
    pub(super) model: u64,
    pub(super) pending: Option<Model>,
    /// The model last asked for (`Loose::model_for`).
    pub(super) asked: Option<String>,
    pub(super) bones: HashMap<SlotId, Affine3A>,
    pub(super) camera: Option<Entity>,
    pub(super) image: Option<Handle<Image>>,
    pub(super) size: UVec2,
    pub(super) lights: Vec<Entity>,
    pub(super) frames: u32,
    /// The camera's place along its axis (the engine's z under the
    /// turning parent's parent: −1 as an item is first shown,
    /// `ResetRotator(-1)`), and the shown model's bounds as it was put in
    /// (Bevy's axes), which the zoom keeps in sight.
    pub(super) camera_z: f32,
    pub(super) bounds: Option<(Vec3, Vec3)>,
    /// The shown model's objects, what hangs from what, its meshes by
    /// object and each slot's bone among them (the outline's part).
    pub(super) nodes: Vec<Entity>,
    pub(super) parents: Vec<Option<usize>>,
    pub(super) drawn: Vec<(usize, Handle<Mesh>)>,
    pub(super) bone_nodes: HashMap<SlotId, usize>,
    /// The outline (`HighLightMesh`): the mask's camera (under the
    /// preview's) and picture, the white its meshes are drawn in, what it
    /// was last put for (the slot outlined, if any, the model and the
    /// picture's size), and the slot's meshes drawn again into the mask.
    pub(super) mask_camera: Option<Entity>,
    pub(super) mask_image: Option<Handle<Image>>,
    pub(super) mask_material: Option<Handle<StandardMaterial>>,
    pub(super) outlined: Option<(Option<SlotId>, u64, UVec2)>,
    pub(super) copies: Vec<Entity>,
}

/// The zoom: a notch of the wheel
/// moves the camera 0.12 m along its axis, toward the model for a notch up;
/// moved in, it backs off a centimetre at a time until the whole of the
/// model's bounds as put in is in sight again; it is held between 2.7 m and
/// 0.45 m in front of the turning parent.
pub(crate) const ZOOM_PER_NOTCH: f32 = 0.12;

const ZOOM_BACK: f32 = 0.01;

pub(crate) const ZOOM_NEAREST: f32 = -0.45;

pub(crate) const ZOOM_FARTHEST: f32 = -2.7;

/// The camera's place as an item is first shown (`ResetRotator(-1)`).
pub(crate) const CAMERA_START: f32 = -1.0;

/// A wheel's travel given in pixels (a touchpad's) taken as notches: the
/// app's.
pub(crate) const PIXELS_A_NOTCH: f32 = 100.0;

/// Whether every corner of a box (Bevy's axes) lies in the preview camera's
/// sight with the camera at that place (`IsFullyVisibleByCamera`: inside
/// all six planes of its frustum): the camera looks along Bevy's −z from
/// the turning parent's height.
pub(crate) fn in_sight(scene: &trp_data::interface::edit_build::EditBuildScene, camera_z: f32, aspect: f32, (low, high): (Vec3, Vec3)) -> bool {
    let eye = convert::position([0.0, scene.height, camera_z]);
    let tan = (scene.fov.to_radians() / 2.0).tan();
    (0..8).all(|corner| {
        let p = Vec3::new(if corner & 1 == 0 { low.x } else { high.x }, if corner & 2 == 0 { low.y } else { high.y }, if corner & 4 == 0 { low.z } else { high.z });
        let ahead = eye.z - p.z;
        (scene.near..=scene.far).contains(&ahead) && (p.y - eye.y).abs() <= ahead * tan && (p.x - eye.x).abs() <= ahead * tan * aspect
    })
}

/// The camera's place after a zoom by that much (`WeaponPreview.Zoom`).
pub(crate) fn zoomed(scene: &trp_data::interface::edit_build::EditBuildScene, camera_z: f32, zoom: f32, aspect: f32, bounds: (Vec3, Vec3)) -> f32 {
    let mut z = camera_z + zoom;
    if zoom > 0.0 && !in_sight(scene, z, aspect, bounds) {
        // (No further back than its farthest, where nothing is ever
        // backed off past.)
        while !in_sight(scene, z, aspect, bounds) && z > ZOOM_FARTHEST {
            z -= ZOOM_BACK;
        }
    }
    z.clamp(ZOOM_FARTHEST, ZOOM_NEAREST)
}

/// A model put into the preview hidden, shown on the next frame once its
/// objects' places are worked out: what was put in, its key and the build
/// it is of.
pub(super) struct Model {
    pub(super) put: PutModel,
    pub(super) key: String,
    pub(super) build: Item,
}

/// A thing's model put into a preview (`WeaponPreview.PositionGameObject`),
/// hidden: its holder, its objects with their names and parents, and what
/// it draws that its bounds are of (`GetBounds`: the meshes on objects
/// that are on, but a lens's and the muzzle's jet), by object.
pub(crate) struct PutModel {
    pub(crate) holder: Entity,
    pub(crate) nodes: Vec<Entity>,
    pub(crate) names: Vec<String>,
    pub(crate) parents: Vec<Option<usize>>,
    pub(crate) bounded: Vec<(usize, Handle<Mesh>)>,
    /// Each mesh drawn, by its object.
    pub(crate) drawn: Vec<(usize, Handle<Mesh>)>,
}

/// A thing's model put under a preview's turning parent, hidden, on its
/// layer: its pivot turned by `turn` (Bevy's) and scaled by the
/// `PreviewPivot`, its container at minus its pivot, the weapon's object
/// at its place in the container (`PositionGameObject`); with no
/// `PreviewPivot`, about the model's own first object (`PROVISIONAL`: the
/// game's bounds' middle).
/// With `surface`, each piece of an install's material is drawn with the
/// game's own surface under that view's lights (`game_surface`: the inspect
/// window's preview), a dithered glass with that surface's glass (marked
/// `Dithered`, casting no shadow); else with its standard material.
pub(crate) fn put_model(
    commands: &mut Commands,
    ready: &crate::world::loose::Ready,
    rotator: Entity,
    layer: &RenderLayers,
    turn: Quat,
    place: Option<trp_data::interface::icon::PreviewPlace>,
    mut surface: Option<(&mut Assets<crate::rendering::game_surface::GameSurfaceMaterial>, &Assets<StandardMaterial>, &crate::rendering::game_surface::SurfaceView)>,
) -> PutModel {
    let (scale, pivot, object) = match place {
        Some(place) => (Vec3::from(place.scale), place.pivot, convert::transform(place.object.0, place.object.1, place.object.2)),
        None => (Vec3::ONE, [0.0; 3], Transform::IDENTITY),
    };
    let holder = commands.spawn((Transform { translation: Vec3::ZERO, rotation: turn, scale }, Visibility::Hidden, layer.clone(), ChildOf(rotator))).id();
    let container = commands.spawn((Transform::from_translation(convert::position([-pivot[0], -pivot[1], -pivot[2]])), Visibility::Inherited, layer.clone(), ChildOf(holder))).id();
    let top = commands.spawn((object, Visibility::Inherited, layer.clone(), ChildOf(container))).id();
    let mut nodes = Vec::with_capacity(ready.nodes.len());
    for (parent, own, on) in &ready.nodes {
        let under = parent.and_then(|parent| nodes.get(parent).copied()).unwrap_or(top);
        let shown = if *on { Visibility::Inherited } else { Visibility::Hidden };
        nodes.push(commands.spawn((*own, shown, layer.clone(), ChildOf(under))).id());
    }
    let mut drawn = Vec::with_capacity(ready.drawn.len());
    for (k, (node, mesh, material)) in ready.drawn.iter().enumerate() {
        if crate::world::loose::left_out(ready.lens.get(k).copied().flatten(), true) {
            continue;
        }
        let piece = commands.spawn((Mesh3d(mesh.clone()), Transform::IDENTITY, layer.clone(), ChildOf(nodes[*node]))).id();
        let game = ready.games.get(k).cloned().flatten();
        let glass = surface.as_ref().and_then(|_| crate::effects::glass_view::glass_note(material));
        let made = match (game, surface.as_mut()) {
            (Some((figures, cube)), Some((surfaces, standards, view))) => standards.get(material).map(|standard| {
                surfaces.add(match &glass {
                    Some(note) => crate::rendering::game_surface::GameSurface::glass(standard, &figures, note, view),
                    None => crate::rendering::game_surface::GameSurface::material(standard, &figures, cube, view),
                })
            }),
            _ => None,
        };
        match made {
            Some(game) if glass.is_some() => commands.entity(piece).insert((MeshMaterial3d(game), crate::effects::glass_view::Dithered, bevy::light::NotShadowCaster)),
            Some(game) => commands.entity(piece).insert(MeshMaterial3d(game)),
            None => commands.entity(piece).insert(MeshMaterial3d(material.clone())),
        };
        drawn.push((*node, mesh.clone()));
    }
    let parents: Vec<Option<usize>> = ready.nodes.iter().map(|(parent, ..)| *parent).collect();
    // (An object is on where it and all it hangs from are.)
    let mut active = vec![true; ready.nodes.len()];
    for (index, (parent, _, on)) in ready.nodes.iter().enumerate() {
        active[index] = *on && parent.and_then(|parent| active.get(parent).copied()).unwrap_or(true);
    }
    let bounded = ready
        .drawn
        .iter()
        .filter(|(node, ..)| active.get(*node).copied().unwrap_or(false) && ready.names.get(*node).is_none_or(|name| !name.contains("linza") && name != "MuzzleJetCombinedMesh"))
        .map(|(node, mesh, _)| (*node, mesh.clone()))
        .collect();
    PutModel { holder, nodes, names: ready.names.clone(), parents, bounded, drawn }
}

/// A model's bounds in the world as it was put in (`GetBounds`): each
/// mesh's box through its object's place, those of no size left out; none
/// before its objects' places are had.
pub(crate) fn model_bounds(put: &PutModel, meshes: &Assets<Mesh>, globals: &Query<&GlobalTransform>) -> Option<(Vec3, Vec3)> {
    let mut bounds: Option<(Vec3, Vec3)> = None;
    for (node, mesh) in &put.bounded {
        let (Some(aabb), Ok(global)) = (meshes.get(mesh).and_then(MeshAabb::compute_aabb), globals.get(put.nodes[*node])) else { continue };
        if aabb.half_extents == Vec3A::ZERO {
            continue;
        }
        let affine = global.affine();
        for corner in 0..8 {
            let sign = Vec3A::new(if corner & 1 == 0 { -1.0 } else { 1.0 }, if corner & 2 == 0 { -1.0 } else { 1.0 }, if corner & 4 == 0 { -1.0 } else { 1.0 });
            let p = Vec3::from(affine.transform_point3a(aabb.center + aabb.half_extents * sign));
            bounds = Some(bounds.map_or((p, p), |(low, high)| (low.min(p), high.max(p))));
        }
    }
    bounds
}

/// A preview's lights, as the edit build screen's scene has them, on those
/// layers (`WeaponPreview._weaponPreviewLights`): as the game moves their
/// object to the world's middle (`DisplaySettingsChangedHandler`), a point
/// light's intensity by Bevy's fall-off where the engine's has halved it
/// at a fifth of its range, as the explosions' (`ESTIMATE`).
pub(crate) fn preview_lights(commands: &mut Commands, scene: &trp_data::interface::edit_build::EditBuildScene, layer: &RenderLayers) -> Vec<Entity> {
    scene.lights.iter().filter(|light| light.on).map(|light| preview_light(commands, light, convert::transform(light.position, light.rotation, [1.0; 3]), layer, false)).collect()
}

/// One light of a screen's preview, placed so, on those layers, no
/// shadows. The one maker of a screen's lights: the edit build screen's,
/// and the OVERALL tab's eight (`figure_view`). For Bevy's light: its
/// colour made linear, a directional at its intensity times π and 1.2, a
/// point light by Bevy's fall-off where the engine's has halved it at a
/// fifth of its range (`ESTIMATE`, as `preview_lights`). For the game's
/// light rule (`game`, a view that asks for it: `shading`): in the game's
/// own numbers, as the world's lights are handed (`lighting`): its colour
/// as stored, a directional at its intensity, a point light at its
/// intensity times 4π (which Bevy divides by) and its range, by the
/// engine's fall-off at every distance.
pub(crate) fn preview_light(commands: &mut Commands, light: &trp_data::interface::edit_build::PreviewLight, place: Transform, layer: &RenderLayers, game: bool) -> Entity {
    let pi = std::f32::consts::PI;
    if game {
        let color = Color::linear_rgb(light.color[0], light.color[1], light.color[2]);
        return match light.kind {
            1 => commands.spawn((Name::new(light.name.clone()), DirectionalLight { color, illuminance: light.intensity, shadows_enabled: false, ..default() }, place, layer.clone())).id(),
            _ => commands.spawn((Name::new(light.name.clone()), PointLight { color, intensity: 4.0 * pi * light.intensity, range: light.range, radius: 0.0, shadows_enabled: false, ..default() }, place, layer.clone())).id(),
        };
    }
    let color = Color::srgb(light.color[0], light.color[1], light.color[2]);
    match light.kind {
        1 => commands.spawn((Name::new(light.name.clone()), DirectionalLight { color, illuminance: light.intensity * pi * 1.2, shadows_enabled: false, ..default() }, place, layer.clone())).id(),
        _ => {
            let reach = 0.2 * light.range;
            let lumens = 4.0 * pi * reach * reach * light.intensity / (1.0 + 25.0 * 0.2 * 0.2) * pi * 1.2;
            commands.spawn((Name::new(light.name.clone()), PointLight { color, intensity: lumens, range: light.range, radius: 0.0, shadows_enabled: false, ..default() }, place, layer.clone())).id()
        }
    }
}
