//! The glass drawn: its twins readied, swapped and followed, and the cameras marked.

use super::*;

/// On a mesh drawn with the glass's twin: the standard material it is drawn
/// with otherwise.
#[derive(Component)]
pub struct GlassDrawn(Handle<StandardMaterial>);

impl GlassDrawn {
    pub fn material(&self) -> &Handle<StandardMaterial> {
        &self.0
    }
}

/// The twin of each glass's standard material, made once and shared.
#[derive(Resource, Default)]
pub(super) struct Twins(pub(super) HashMap<AssetId<StandardMaterial>, Handle<GlassMaterial>>);

/// Before the shadow-only wish is applied (`shadow_only::apply`): a glass
/// mesh, as it comes, casts no shadow; one drawn with its twin whose wish
/// is now its shadow alone goes back to its standard material, which the
/// wish then turns to the shadow's twin in the same frame (so it draws
/// nothing at all).
#[allow(clippy::type_complexity)]
pub(super) fn ready(
    mut commands: Commands,
    added: Query<(Entity, &MeshMaterial3d<StandardMaterial>), (Added<MeshMaterial3d<StandardMaterial>>, Without<NotShadowCaster>, EvenParked)>,
    shown: Query<(Entity, &ShadowOnly, &GlassDrawn), EvenParked>,
) {
    for (entity, material) in &added {
        if is_glass(&material.0) {
            commands.entity(entity).insert(NotShadowCaster);
        }
    }
    for (entity, wish, drawn) in &shown {
        if wish.0 {
            commands.entity(entity).remove::<(MeshMaterial3d<GlassMaterial>, GlassDrawn, Dithered)>().insert(MeshMaterial3d(drawn.0.clone()));
        }
    }
}

/// After it: each glass mesh drawn with its standard material and not
/// wished to be its shadow alone is drawn with the twin.
#[allow(clippy::type_complexity)]
pub(super) fn swap(
    mut commands: Commands,
    meshes: Query<(Entity, &MeshMaterial3d<StandardMaterial>, Option<&ShadowOnly>), (Changed<MeshMaterial3d<StandardMaterial>>, EvenParked)>,
    standard: Res<Assets<StandardMaterial>>,
    mut glasses: ResMut<Assets<GlassMaterial>>,
    mut twins: ResMut<Twins>,
    level: Option<Res<crate::rendering::lighting::LevelLighting>>,
) {
    let top_ambient = level.map_or([0.0; 3], |level| level.top_ambient);
    for (entity, material, wish) in &meshes {
        if wish.is_some_and(|wish| wish.0) {
            continue;
        }
        let Some(note) = note_of(&material.0) else { continue };
        let Some(base) = standard.get(&material.0) else { continue };
        let twin = twins.0.entry(material.0.id()).or_insert_with(|| glasses.add(Glass::twin(base, &note, top_ambient))).clone();
        commands.entity(entity).remove::<MeshMaterial3d<StandardMaterial>>().insert((MeshMaterial3d(twin), GlassDrawn(material.0.clone()), Dithered, NotShadowCaster));
    }
}

/// A glass's standard material pointed at other pictures (a level of its
/// colour picture raised or brought down: `streaming`): its twin likewise.
pub(super) fn follow(mut events: MessageReader<AssetEvent<StandardMaterial>>, standard: Res<Assets<StandardMaterial>>, twins: Res<Twins>, mut glasses: ResMut<Assets<GlassMaterial>>) {
    for event in events.read() {
        let AssetEvent::Modified { id } = *event else { continue };
        let (Some(twin), Some(base)) = (twins.0.get(&id), standard.get(id)) else { continue };
        if let Some(glass) = glasses.get_mut(twin) {
            glass.base.base_color_texture = base.base_color_texture.clone();
            glass.base.normal_map_texture = base.normal_map_texture.clone();
        }
    }
}

/// Whether the glass may be drawn yet: where the PBR functions are to be
/// patched (`TARKOV SHADING`, `OVERALL FIGURE`), once they are, whose light
/// its shader calls (`shading`).
pub(super) fn ready_to_draw() -> bool {
    !crate::rendering::shading::installed() || crate::rendering::shading::patched()
}

/// On a camera that draws a glass this frame (one shown on its layers):
/// the undithering runs there. Only a camera whose depth can be read (the
/// world's, the optic's, the gear page's figures', the edit build screen's,
/// the inspect window's preview's): the scene's depth is copied for it
/// (`muzzle_view`).
#[derive(Component, Clone, Copy, Default, ExtractComponent)]
pub struct UnditherScreen;

pub(super) fn mark_cameras(
    mut commands: Commands,
    cameras: Query<(Entity, &Camera, &Camera3d, Option<&RenderLayers>, Has<UnditherScreen>, Option<&Name>)>,
    glasses: Query<(&ViewVisibility, Option<&RenderLayers>), With<Dithered>>,
) {
    let none = RenderLayers::default();
    for (entity, camera, camera_3d, layers, marked, name) in &cameras {
        let layers = layers.unwrap_or(&none);
        let readable = TextureUsages::from(camera_3d.depth_texture_usages).contains(TextureUsages::TEXTURE_BINDING);
        let seen = camera.is_active && readable && glasses.iter().any(|(visible, on)| visible.get() && on.unwrap_or(&none).intersects(layers));
        match (seen, marked) {
            (true, false) => {
                debug!("the undithering on {}", name.map_or("a camera", |name| name.as_str()));
                commands.entity(entity).insert(UnditherScreen);
            }
            (false, true) => {
                commands.entity(entity).remove::<UnditherScreen>();
            }
            _ => {}
        }
    }
}
