//! Scorches prepared, decals laid and tagged by stencil, and the decal shader patched.

use super::*;

pub(crate) fn prepare_scorches(unity: &mut trp_data::unity::Assets, db: &trp_data::Database, images: &mut impl crate::assets::background::AddImage) -> Option<Scorches> {
    let decal = match trp_data::drawing::particles::deferred_grenade_decal(unity) {
        Ok(decal) => decal,
        Err(e) => {
            warn!("a grenade's scorch is not laid: {e:#}");
            return None;
        }
    };
    if decal.color != [1.0; 4] {
        info!("the grenade decal's colour {:?} is not built: taken as white", decal.color);
    }
    let Some(picture) = crate::effects::particles_view::picture(&decal.picture, images) else { return None };
    // The blasts' effects that lay one (the grenades' and the explosive
    // rounds': `loose::blast_effects`).
    let mut laying = crate::world::loose::blast_effects(db);
    laying.retain(|name| trp_data::drawing::particles::named_effect(unity, name).is_ok_and(|effect| effect.deferred_decal));
    info!("a grenade's scorch: {} by {}, {:?} m, {} m deep, at most {}; laid by {laying:?}", decal.picture.name, decal.shader, decal.sizes, decal.projector_height, decal.max_decals);
    Some(Scorches { decal, picture, laying, laid: VecDeque::new(), rng: Rng::new(0x5c0c) })
}

/// At each blast whose effect lays one, its scorch: the box
/// (`trp_sim::decals::static_box`, its normal as `EmitGrenade` gives it: a
/// thrown grenade's up, a round's hit's surface's;
/// its size and turn drawn as the game draws them) as a clustered decal,
/// its unit box's x the box's first way across, its y the second's
/// opposite and its z the normal (Bevy's picture's place `(x, −y) + ½`
/// being then the game's), in the app's axes.
pub(super) fn lay(mut commands: Commands, mut blasts: MessageReader<crate::world::loose::Blasted>, mut scorches: ResMut<Scorches>) {
    for blasted in blasts.read() {
        let Some((name, _)) = blasted.effect.as_ref() else { continue };
        if !scorches.laying.iter().any(|n| n == name) {
            continue;
        }
        let size_share = scorches.rng.unit();
        let turn = scorches.decal.randomize_rotation.then(|| ((scorches.rng.unit() * 359.0) as u32).min(358) as f32);
        let b = trp_sim::effects::decals::static_box(blasted.at, blasted.normal, scorches.decal.sizes, size_share, scorches.decal.projector_height, turn);
        let axis = |v: [f32; 3], length: f32| crate::platform::convert::position(v) * length;
        let matrix = Mat4::from_cols(
            axis(b.right, 2.0 * b.half[0]).extend(0.0),
            axis(b.forward.map(|c| -c), 2.0 * b.half[2]).extend(0.0),
            axis(b.normal, 2.0 * b.half[1]).extend(0.0),
            crate::platform::convert::position(b.center).extend(1.0),
        );
        let entity = commands
            .spawn((
                ClusteredDecal { base_color_texture: Some(scorches.picture.clone()), normal_map_texture: None, metallic_roughness_texture: None, emissive_texture: None, tag: 0 },
                Transform::from_matrix(matrix),
            ))
            .id();
        scorches.laid.push_back(entity);
        while scorches.laid.len() > scorches.decal.max_decals as usize {
            if let Some(old) = scorches.laid.pop_front() {
                commands.entity(old).despawn();
            }
        }
        info!("a scorch laid at {:?}, {} m across, turned {turn:?}", blasted.at, 2.0 * b.half[0]);
    }
}

/// Each mesh tagged with its material's stencil type in the tag's top two
/// bits (nought where it is nought), as it is given a material; the rest of
/// the tag is the levels of detail's band (`lod_view`), kept
/// as it is found.
pub(super) fn tag_by_stencil(mut commands: Commands, meshes: Query<(Entity, &MeshMaterial3d<StandardMaterial>, Option<&MeshTag>), Changed<MeshMaterial3d<StandardMaterial>>>) {
    for (entity, material, tag) in &meshes {
        let stencil = u32::from(stencil_of(&material.0)).min(3) << 30;
        let band = tag.map_or(0, |tag| tag.0 & trp_sim::characters::lod::BAND_BITS);
        let wanted = stencil | band;
        match tag {
            Some(t) if t.0 == wanted => {}
            None if wanted == 0 => {}
            Some(_) if wanted == 0 => {
                commands.entity(entity).remove::<MeshTag>();
            }
            _ => {
                commands.entity(entity).insert(MeshTag(wanted));
            }
        }
    }
}

/// Bevy's decal step and the fragment that calls it.
const DECALS: &str = "embedded://bevy_pbr/decal/clustered.wgsl";

const FRAGMENT: &str = "embedded://bevy_pbr/render/pbr.wgsl";

#[derive(Resource)]
pub(super) struct Held(Handle<Shader>, Handle<Shader>);

pub(super) fn hold(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(Held(assets.load(DECALS), assets.load(FRAGMENT)));
}

/// The game's decal step, put after Bevy's (renamed).
const GAME_DECALS: &str = "
// (trp: the game's deferred decal, `decal_view`; grenades spec, \"The
// scorch\".) Linear light to the GPU's sRGB numbers and back.
fn trp_encode(c: vec3<f32>) -> vec3<f32> {
    let k = max(c, vec3<f32>(0.0));
    return select(1.055 * pow(k, vec3<f32>(1.0 / 2.4)) - vec3<f32>(0.055), k * 12.92, k <= vec3<f32>(0.0031308));
}

fn trp_decode(c: vec3<f32>) -> vec3<f32> {
    let k = max(c, vec3<f32>(0.0));
    return select(pow((k + vec3<f32>(0.055)) / 1.055, vec3<f32>(2.4)), k / 12.92, k <= vec3<f32>(0.04045));
}

// Bevy's own name, which its fragments and the range's (`grid.wgsl`) call:
// the game's step, on a surface of the world's stencil type.
fn apply_decals(pbr_input: ptr<function, PbrInput>) {
    apply_decals_tagged(pbr_input, 0u);
}

// Each decal whose box holds the point and whose normal the surface faces
// within 0.8 blends the surface's colour, in the game's numbers, toward its
// picture's by the picture's alpha, and its emission toward nothing; the
// picture's level by its change across the pixel. A mesh tagged (the game's
// stencil type not nought, in its tag's top two bits; the rest is the levels
// of detail's band, `lod_view`) takes none.
fn apply_decals_tagged(pbr_input: ptr<function, PbrInput>, tag: u32) {
#ifdef CLUSTERED_DECALS_ARE_USABLE
    let world_position = (*pbr_input).world_position.xyz;
    let dx = dpdx(world_position);
    let dy = dpdy(world_position);
    if (tag >> 30u) != 0u {
        return;
    }
    let surface_normal = normalize((*pbr_input).N);
    let frag_coord = (*pbr_input).frag_coord.xy;
    var colour = trp_encode((*pbr_input).material.base_color.rgb);
    var emissive = (*pbr_input).material.emissive;
    let view_z = get_view_z(world_position);
    let cluster_index = clustered_forward::fragment_cluster_index(frag_coord, view_z, view_is_orthographic());
    var ranges = clustered_forward::unpack_clusterable_object_index_ranges(cluster_index);
    var iterator = clustered_decal_iterator_new(world_position, &ranges);
    var laid = false;
    while (clustered_decal_iterator_next(&iterator)) {
        if iterator.base_color_texture_index < 0 {
            continue;
        }
        let index = i32(clustered_forward::get_clusterable_object_id(u32(iterator.decal_index_offset)));
        let m = mesh_view_bindings::clustered_decals.decals[index].local_from_world;
        let row_x = vec3<f32>(m[0].x, m[1].x, m[2].x);
        let row_y = vec3<f32>(m[0].y, m[1].y, m[2].y);
        let row_z = vec3<f32>(m[0].z, m[1].z, m[2].z);
        if dot(surface_normal, normalize(row_z)) < 0.8 {
            continue;
        }
        var ddx = vec2<f32>(dot(row_x, dx), -dot(row_y, dx));
        var ddy = vec2<f32>(dot(row_x, dy), -dot(row_y, dy));
        // A bullet's hole: its place in the atlas, its tile and its sheet's
        // cells in the tag (`decal_view::lay_holes`).
        var uv = iterator.uv;
        let t = iterator.tag;
        if t != 0u {
            let rect = vec4<f32>(f32(t & 63u), f32((t >> 6u) & 63u), f32(((t >> 12u) & 63u) + 1u), f32(((t >> 18u) & 63u) + 1u)) / 64.0;
            let tile = vec2<f32>(f32((t >> 24u) & 3u), f32((t >> 26u) & 3u));
            let cells = vec2<f32>(f32(((t >> 28u) & 3u) + 1u), f32(((t >> 30u) & 3u) + 1u));
            let scale = rect.zw / cells;
            uv = rect.xy + (tile + uv) * scale;
            ddx = ddx * scale;
            ddy = ddy * scale;
        }
        let c = textureSampleGrad(mesh_view_bindings::clustered_decal_textures[iterator.base_color_texture_index], mesh_view_bindings::clustered_decal_sampler, uv, ddx, ddy);
        colour = c.rgb * c.a + colour * (1.0 - c.a);
        emissive = vec4<f32>(emissive.rgb * (1.0 - c.a), emissive.a);
        laid = true;
    }
    if laid {
        (*pbr_input).material.base_color = vec4<f32>(trp_decode(colour), (*pbr_input).material.base_color.a);
        (*pbr_input).material.emissive = emissive;
    }
#endif
}
";

/// Bevy's decal step as it begins.
const BEVY_DECALS: &str = "fn apply_decals(pbr_input: ptr<function, PbrInput>) {";

/// The fragment's call, with its mesh's tag.
const CALL: &str = "    apply_decals(&pbr_input);";

const TAGGED_CALL: &str = "
    // (trp: the game's decal step with the mesh's tag, `decal_view`.)
#ifdef VERTEX_OUTPUT_INSTANCE_INDEX
    apply_decals_tagged(&pbr_input, mesh_functions::get_tag(in.instance_index));
#else
    apply_decals_tagged(&pbr_input, 0u);
#endif
";

const IMPORTS: &str = "#import bevy_pbr::{\n    pbr_types,";

const MORE_IMPORTS: &str = "#import bevy_pbr::decal::clustered::apply_decals_tagged\n#import bevy_pbr::mesh_functions\n#import bevy_pbr::{\n    pbr_types,";

/// Bevy's two shaders, as they are loaded, put back patched; a part not
/// found (another Bevy) leaves them as they are, with a line saying so.
pub(super) fn patch(held: Option<Res<Held>>, mut events: MessageReader<AssetEvent<Shader>>, mut shaders: ResMut<Assets<Shader>>) {
    let Some(held) = held else { return };
    for event in events.read() {
        let (AssetEvent::Added { id } | AssetEvent::Modified { id } | AssetEvent::LoadedWithDependencies { id }) = *event else { continue };
        let decals = id == held.0.id();
        if !decals && id != held.1.id() {
            continue;
        }
        let Some(shader) = shaders.get(id) else { continue };
        let Source::Wgsl(source) = &shader.source else { continue };
        let text = source.to_string();
        let patched = if decals {
            if text.contains("fn apply_decals_tagged(") {
                continue;
            }
            // Bevy's own step kept aside under another name.
            text.contains(BEVY_DECALS).then(|| format!("{}\n{GAME_DECALS}", text.replacen(BEVY_DECALS, "fn bevy_apply_decals(pbr_input: ptr<function, PbrInput>) {", 1)))
        } else {
            if text.contains("apply_decals_tagged(") {
                continue;
            }
            (text.contains(CALL) && text.contains(IMPORTS)).then(|| text.replacen(CALL, TAGGED_CALL, 1).replacen(IMPORTS, MORE_IMPORTS, 1))
        };
        let Some(patched) = patched else {
            warn!("Bevy's {} is not as expected: the game's decal step is not put in (the scorch is Bevy's)", if decals { DECALS } else { FRAGMENT });
            continue;
        };
        let fresh = Shader::from_wgsl(patched, shader.path.clone());
        let replaced = Shader { source: fresh.source, imports: fresh.imports, import_path: fresh.import_path, ..shader.clone() };
        let _ = shaders.insert(id, replaced);
        info!("the game's decal step: Bevy's {} patched", if decals { DECALS } else { FRAGMENT });
    }
}
