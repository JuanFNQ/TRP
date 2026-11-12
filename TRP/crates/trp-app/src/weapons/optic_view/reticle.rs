//! The reticle's mesh and its showing.

use super::*;

/// The reticle mesh shown: whose (`ScopeReticle`, by its place in memory),
/// its entity, its mesh and the blend weight it was last made at.
pub(super) struct Shown {
    pub(super) key: usize,
    pub(super) entity: Entity,
    pub(super) mesh: Handle<Mesh>,
    weight: f32,
}

/// A reticle's mesh at a weight of its blend shape, in the app's axes.
fn reticle_mesh(reticle: &trp_data::weapons::sight::ScopeReticle, weight: f32) -> Mesh {
    let moved: Vec<[f32; 3]> = reticle
        .positions
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let d = reticle.shape.as_ref().and_then(|s| s.get(i)).copied().unwrap_or([0.0; 3]);
            crate::platform::convert::position([p[0] + weight * d[0], p[1] + weight * d[1], p[2] + weight * d[2]]).to_array()
        })
        .collect();
    // Turned round with z, each triangle keeps its order (they are drawn
    // one over another in it) and its corners are taken the other way.
    let mut triangles = reticle.triangles.clone();
    for t in triangles.chunks_exact_mut(3) {
        t.swap(1, 2);
    }
    Mesh::new(bevy::mesh::PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, moved)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, reticle.colors.clone())
        .with_inserted_indices(bevy::mesh::Indices::U32(triangles))
}

/// The live optic's reticle mesh, where its scope has one (a variable
/// scope's, `OpticRetrice`): a child of the optic camera at its place and
/// turn, on the camera's own layer, its blend shape weighted by the zoom.
#[allow(clippy::too_many_arguments)]
pub(super) fn show_reticle(
    mut commands: Commands,
    hands: Option<Res<HandsRig>>,
    light: Option<Res<crate::rendering::lighting::LevelLighting>>,
    camera: Query<Entity, With<OpticCamera>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ReticleMaterial>>,
    mut shown: Local<Option<Shown>>,
) {
    let Ok(camera) = camera.single() else { return };
    let wanted = hands.as_ref().and_then(|hands| {
        let sights = hands.hands.sights()?;
        let (part, mode) = sights.live_optic()?;
        let reticle = sights.parts().get(part)?.modes.get(mode)?.optic.as_ref()?.reticle.clone()?;
        Some((reticle, sights.reticle_weight()))
    });
    let Some((reticle, weight)) = wanted else {
        if let Some(old) = shown.take() {
            commands.entity(old.entity).despawn();
        }
        return;
    };
    let key = Arc::as_ptr(&reticle) as usize;
    if let Some(now) = shown.as_mut().filter(|now| now.key == key) {
        if (now.weight - weight).abs() > 1e-4 {
            now.weight = weight;
            if let Some(mesh) = meshes.get_mut(&now.mesh) {
                *mesh = reticle_mesh(&reticle, weight);
            }
        }
        return;
    }
    if let Some(old) = shown.take() {
        commands.entity(old.entity).despawn();
    }
    let mesh = meshes.add(reticle_mesh(&reticle, weight));
    let power = if light.is_some() { 2.2 } else { 1.0 };
    let material = materials.add(ReticleMaterial { reticle: ReticleUniform { color: Vec4::from_array(reticle.color), figures: Vec4::new(reticle.scale * 0.1, power, 0.0, 0.0) } });
    let turn = trp_sim::base::math::quat_euler(reticle.rotation);
    let entity = commands
        .spawn((
            Name::new("Scope reticle"),
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material),
            Transform { translation: crate::platform::convert::position(reticle.position), rotation: crate::platform::convert::rotation(turn), scale: Vec3::ONE },
            RenderLayers::layer(RETICLE_LAYER),
            NotShadowCaster,
            bevy::camera::visibility::NoFrustumCulling,
        ))
        .id();
    commands.entity(camera).add_child(entity);
    *shown = Some(Shown { key, entity, mesh, weight });
}
