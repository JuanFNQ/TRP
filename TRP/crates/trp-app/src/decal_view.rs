//! The game's deferred decals as the app lays them (grenades spec, "The
//! scorch"; `trp_sim::decals`): a grenade's scorch, where its effect lays
//! one (`Effect.Emit`, `DeferredDecalRenderer.DrawDecal`), as one of Bevy's
//! clustered decals over the decal's box, at most the renderer's number at
//! once (the oldest taken away). The game draws the box into its G-buffer
//! before its light; Bevy's clustered decals change a surface's own values
//! before its light, and their step is patched as Bevy's shaders load (as
//! `shading` patches its light) to the game's shader's rule: the surface's
//! colour blended toward the picture's by its alpha in the game's gamma
//! numbers, only on a surface facing the decal's normal within 0.8 (its dot),
//! its emission toward nothing by the same; the picture's level by its size
//! on the screen (`sample` in the game's). The game's stencil test (the
//! decal's stencil type, 0, against the surface's: characters 1, hands 2)
//! is kept by each mesh's tag: a mesh of a material whose `_StencilType` is
//! not nought is tagged with it (`note_stencil`), and takes no decal.
//!
//! `PROVISIONAL`: Bevy's decals share one sampler, the first image's (the
//! clouds' cookie's with `CUSTOMS CLOUDS`); overlapping decals are blended
//! in Bevy's order, the game's in the order laid. Not built: a decal's
//! specular and normal parts (the grenade's material has neither: its
//! weights nought), its colour other than white, the occlusion it writes
//! (the game's ambient takes none on its diffuse part: lighting spec).
//!
//! A bullet's hole (ballistics spec, "What a hit shows and sounds"): the
//! renderer's decal for the struck material (`_decals` by
//! `DecalMaterialType`, else `_genericDecal`), where its effect lays one,
//! by the same box (its own sizes and the renderer's projector height); its
//! tile of the sheet drawn as the game draws it (`GenerateTangents`). Bevy's
//! decal step takes at most eight pictures at once (the clouds' cookie and
//! the scorch among them), so the bullets' sheets are packed into one atlas
//! and each hole's place in it, its tile and its sheet's cells ride in the
//! decal's tag, which the game's step reads. One ring of 200 places runs
//! through every sheet's static holes (`_currentDecalIndex`); a hole on a
//! thing that moves (a door's leaf) follows it, from a ring of its own
//! (`_maxDynamicDecals`). `PROVISIONAL`: the atlas's own levels, made from
//! it (the pictures' own are not used), so a hole far off may take a
//! little of its neighbours; the moving holes drawn with the static
//! material's picture (`DynamicDecalMaterial` not read); a cutout sheet
//! (soil's) blended as the rest. Not built: their normals and specular.

use std::collections::{HashMap, VecDeque};
use std::sync::{LazyLock, Mutex};

use bevy::asset::AssetEvent;
use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::light::ClusteredDecal;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::mesh::MeshTag;
use bevy::prelude::*;
use bevy::shader::{Shader, Source};
use trp_sim::recoil::Rng;

/// The game's stencil type of each material made with one not nought
/// (`_StencilType`: 1 characters, 2 hands), as they are made
/// (`render_data`).
static STENCIL_MATERIALS: LazyLock<Mutex<HashMap<AssetId<StandardMaterial>, u8>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// A material noted with its stencil type as made.
pub fn note_stencil(material: &Handle<StandardMaterial>, stencil: u8) {
    if stencil == 0 {
        return;
    }
    if let Ok(mut map) = STENCIL_MATERIALS.lock() {
        map.insert(material.id(), stencil);
    }
}

fn stencil_of(material: &Handle<StandardMaterial>) -> u8 {
    STENCIL_MATERIALS.lock().ok().and_then(|map| map.get(&material.id()).copied()).unwrap_or(0)
}

/// The grenade decal read, its picture, the decals laid (oldest first).
#[derive(Resource)]
struct Scorches {
    decal: trp_data::particles::DeferredDecal,
    picture: Handle<Image>,
    /// The effects that lay a deferred decal, by name.
    laying: Vec<String>,
    laid: VecDeque<Entity>,
    rng: Rng,
}

pub struct DecalPlugin {
    /// The explosions drawn (`PARTICLES`) and the hard settings'
    /// `DEFERRED_DECALS_ENABLED`.
    pub on: bool,
}

impl Plugin for DecalPlugin {
    fn build(&self, app: &mut App) {
        if !self.on {
            return;
        }
        app.add_systems(Startup, (read_decal, read_holes, hold))
            .add_systems(Update, (patch, tag_by_stencil, lay.run_if(resource_exists::<Scorches>)));
    }
}

/// The bullets' decals read, their atlas, each decal's tag of its place in
/// it, the static ring (each sheet's places by the shared index) and the
/// moving holes' ring, and Unity's generator for the draws.
#[derive(Resource)]
pub struct BulletHoles {
    decals: Vec<(Vec<i32>, trp_data::particles::DeferredDecal, u32)>,
    generic: Option<(trp_data::particles::DeferredDecal, u32)>,
    atlas: Handle<Image>,
    laid: Vec<Vec<Option<Entity>>>,
    index: usize,
    moving: Vec<Option<Entity>>,
    moving_index: usize,
    random: trp_sim::unity_random::UnityRandom,
}

/// The atlas's sides are cut into 64 parts each, a picture's place and
/// size given in them (and so a multiple of a 64th of the atlas).
const ATLAS_PARTS: u32 = 64;
const ATLAS_WIDTH: u32 = 1024;
/// The most moving holes at once (`_maxDynamicDecals`, as the install has it).
const MOVING_HOLES: usize = 200;

/// A picture's place in the atlas as a tag's bits: its corner and size in
/// 64ths of the atlas (the size less one), each six bits.
fn place_bits(x: u32, y: u32, w: u32, h: u32, height: u32) -> u32 {
    let (ux, uy) = (ATLAS_WIDTH / ATLAS_PARTS, height / ATLAS_PARTS);
    (x / ux) | ((y / uy) << 6) | ((w / ux - 1) << 12) | ((h / uy - 1) << 18)
}

/// The bullets' decals, their pictures packed into one atlas (on shelves,
/// tallest first; each picture's rows in their stored order, as every
/// picture of the game's is laid), and its levels made by halving.
fn read_holes(mut commands: Commands, mut game: ResMut<crate::GameData>, mut images: ResMut<Assets<Image>>) {
    let read = match trp_data::particles::deferred_bullet_decals(&mut game.unity) {
        Ok(read) => read,
        Err(e) => {
            warn!("the bullets' holes are not laid: {e:#}");
            return;
        }
    };
    let mut pictures: Vec<(usize, u32, u32, Vec<u8>)> = Vec::new();
    let all: Vec<&trp_data::particles::DeferredDecal> = read.by_material.iter().map(|(_, d)| d).chain([&read.generic]).collect();
    for (at, decal) in all.iter().enumerate() {
        let decoded = trp_data::unity::texture::mips(&decal.picture).ok().and_then(|levels| levels.first().copied()).and_then(|level| trp_data::unity::texture::decode_rgba(&decal.picture, level).ok().map(|px| (level.width, level.height, px)));
        match decoded {
            Some((w, h, px)) => pictures.push((at, w, h, px)),
            None => warn!("a bullet hole's picture {} is not read: left out", decal.picture.name),
        }
    }
    // On shelves of the atlas's width, the tallest first; each place a
    // multiple of a 64th of the side (the generic decal's 30 by 29 is laid
    // in a 32 by 32 place).
    let unit = ATLAS_WIDTH / ATLAS_PARTS;
    let fit = |n: u32| n.div_ceil(unit) * unit;
    let mut order: Vec<usize> = (0..pictures.len()).collect();
    order.sort_by_key(|&i| std::cmp::Reverse(pictures[i].2));
    let (mut x, mut y, mut shelf) = (0u32, 0u32, 0u32);
    let mut places = vec![(0u32, 0u32, 0u32, 0u32); pictures.len()];
    for &i in &order {
        let (w, h) = (fit(pictures[i].1), fit(pictures[i].2));
        if x + w > ATLAS_WIDTH {
            (x, y, shelf) = (0, y + shelf, 0);
        }
        places[i] = (x, y, w, h);
        x += w;
        shelf = shelf.max(h);
    }
    let height = (y + shelf).next_power_of_two().max(ATLAS_PARTS);
    let mut atlas = vec![0u8; (ATLAS_WIDTH * height * 4) as usize];
    for (i, (_, w, h, px)) in pictures.iter().enumerate() {
        let (px0, py0, _, _) = places[i];
        for row in 0..*h {
            let from = (row * w * 4) as usize;
            let to = (((py0 + row) * ATLAS_WIDTH + px0) * 4) as usize;
            atlas[to..to + (w * 4) as usize].copy_from_slice(&px[from..from + (w * 4) as usize]);
        }
    }
    // Its levels, each the one above halved by the mean of four.
    let mut data = atlas.clone();
    let (mut lw, mut lh, mut level) = (ATLAS_WIDTH, height, atlas);
    let mut levels = 1;
    while lw > 1 || lh > 1 {
        let (nw, nh) = ((lw / 2).max(1), (lh / 2).max(1));
        let mut next = vec![0u8; (nw * nh * 4) as usize];
        for j in 0..nh {
            for i in 0..nw {
                for c in 0..4 {
                    let at = |x: u32, y: u32| level[((y.min(lh - 1) * lw + x.min(lw - 1)) * 4 + c) as usize] as u32;
                    let sum = at(2 * i, 2 * j) + at(2 * i + 1, 2 * j) + at(2 * i, 2 * j + 1) + at(2 * i + 1, 2 * j + 1);
                    next[((j * nw + i) * 4 + c) as usize] = ((sum + 2) / 4) as u8;
                }
            }
        }
        data.extend_from_slice(&next);
        (lw, lh, level) = (nw, nh, next);
        levels += 1;
    }
    let mut image = Image::new(Extent3d { width: ATLAS_WIDTH, height, depth_or_array_layers: 1 }, TextureDimension::D2, data, TextureFormat::Rgba8Unorm, RenderAssetUsages::RENDER_WORLD);
    image.texture_descriptor.mip_level_count = levels;
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::ClampToEdge,
        address_mode_v: ImageAddressMode::ClampToEdge,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Linear,
        ..default()
    });
    let atlas = images.add(image);
    let tag_of = |at: usize| -> Option<u32> {
        let i = pictures.iter().position(|p| p.0 == at)?;
        let (x, y, _, _) = places[i];
        Some(place_bits(x, y, fit(pictures[i].1), fit(pictures[i].2), height))
    };
    let count = read.by_material.len();
    let decals: Vec<(Vec<i32>, trp_data::particles::DeferredDecal, u32)> = read.by_material.into_iter().enumerate().filter_map(|(at, (m, d))| Some((m, d, tag_of(at)?))).collect();
    let generic = tag_of(count).map(|tag| (read.generic, tag));
    let most = decals.first().map_or(200, |d| d.1.max_decals as usize);
    for (materials, d, _) in &decals {
        info!("a bullet's hole on {materials:?}: {} ({}x{}, {}x{} cells, {:?} m, by {})", d.picture.name, d.picture.width, d.picture.height, d.columns, d.rows, d.sizes, d.shader);
    }
    info!("the bullets' holes: {} sheets in an atlas of {ATLAS_WIDTH} by {height}, at most {most} still and {MOVING_HOLES} moving", decals.len());
    let laid = vec![vec![None; most]; decals.len() + 1];
    let random = trp_sim::unity_random::UnityRandom::new(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |d| d.subsec_nanos() as i32));
    commands.insert_resource(BulletHoles { decals, generic, atlas, laid, index: 0, moving: vec![None; MOVING_HOLES], moving_index: 0, random });
}

/// The frame's bullets' holes (`Effect.Emit` → `DeferredDecalRenderer.
/// DrawDecal`, for each effect played whose entry lays a deferred decal):
/// the struck material's decal, its size and turn drawn as the box's
/// (`AddCubeToMesh`), then its tile (`GenerateTangents`: the column's draw,
/// then the row's), as a clustered decal over the atlas; on a door's leaf,
/// a child of the leaf, so that it swings with it.
pub fn lay_holes(
    mut commands: Commands,
    bullets: Res<crate::bullets::Bullets>,
    mut holes: ResMut<BulletHoles>,
    parts: Query<(Entity, &crate::ThingPart, &GlobalTransform)>,
) {
    let holes = &mut *holes;
    for (impact, entry) in &bullets.effects {
        if !bullets.entries.get(*entry).is_some_and(|e| e.deferred_decal) {
            continue;
        }
        let which = holes.decals.iter().position(|(m, _, _)| m.contains(&impact.material));
        let (decal, base) = match which {
            Some(i) => (holes.decals[i].1.clone(), holes.decals[i].2),
            None => match &holes.generic {
                Some((d, tag)) => (d.clone(), *tag),
                None => continue,
            },
        };
        let size = holes.random.range(decal.sizes[0], decal.sizes[1]);
        let share = if decal.sizes[1] != decal.sizes[0] { (size - decal.sizes[0]) / (decal.sizes[1] - decal.sizes[0]) } else { 0.0 };
        let turn = decal.randomize_rotation.then(|| holes.random.range_int(0, 359) as f32);
        let column = holes.random.range_int(0, decal.columns as i32) as u32;
        let row = holes.random.range_int(0, decal.rows as i32) as u32;
        // The game puts the row's draw on the picture's first axis and the
        // column's on the second (`GenerateTangents`).
        let tag = base | ((row.min(3)) << 24) | ((column.min(3)) << 26) | ((decal.columns.clamp(1, 4) - 1) << 28) | ((decal.rows.clamp(1, 4) - 1) << 30);
        let b = trp_sim::decals::static_box(impact.position, impact.normal, decal.sizes, share, decal.projector_height, turn);
        let axis = |v: [f32; 3], length: f32| crate::convert::position(v) * length;
        let matrix = Mat4::from_cols(
            axis(b.right, 2.0 * b.half[0]).extend(0.0),
            axis(b.forward.map(|c| -c), 2.0 * b.half[2]).extend(0.0),
            axis(b.normal, 2.0 * b.half[1]).extend(0.0),
            crate::convert::position(b.center).extend(1.0),
        );
        let made = ClusteredDecal { base_color_texture: Some(holes.atlas.clone()), normal_map_texture: None, metallic_roughness_texture: None, emissive_texture: None, tag };
        // On a door's leaf: a child of the leaf as it stands.
        let leaf = crate::range::bullet_thing_part(impact.thing).and_then(|thing| parts.iter().find(|(_, part, _)| part.0 == thing));
        if let Some((parent, _, at)) = leaf {
            let local = at.affine().inverse() * bevy::math::Affine3A::from_mat4(matrix);
            let entity = commands.spawn((made, Transform::from_matrix(Mat4::from(local)))).id();
            commands.entity(parent).add_child(entity);
            let slot = holes.moving_index;
            if let Some(old) = holes.moving[slot].replace(entity) {
                commands.entity(old).try_despawn();
            }
            holes.moving_index = (slot + 1) % MOVING_HOLES;
        } else {
            let entity = commands.spawn((made, Transform::from_matrix(matrix))).id();
            let sheet = which.unwrap_or(holes.decals.len());
            let slot = holes.index;
            if let Some(old) = holes.laid[sheet][slot].replace(entity) {
                commands.entity(old).try_despawn();
            }
            holes.index = (slot + 1) % holes.laid[sheet].len();
        }
    }
}

fn read_decal(mut commands: Commands, mut game: ResMut<crate::GameData>, mut images: ResMut<Assets<Image>>) {
    let decal = match trp_data::particles::deferred_grenade_decal(&mut game.unity) {
        Ok(decal) => decal,
        Err(e) => {
            warn!("a grenade's scorch is not laid: {e:#}");
            return;
        }
    };
    if decal.color != [1.0; 4] {
        info!("the grenade decal's colour {:?} is not built: taken as white", decal.color);
    }
    let Some(picture) = crate::particles_view::picture(&decal.picture, &mut images) else { return };
    // The grenades' effects that lay one (each of the database's grenades'
    // `ExplosionEffectType`).
    let mut laying: Vec<String> = game.db.items.values().filter_map(|item| item.props.get("ExplosionEffectType")?.as_str().map(str::to_owned)).filter(|name| !name.is_empty()).collect();
    laying.sort();
    laying.dedup();
    laying.retain(|name| trp_data::particles::named_effect(&mut game.unity, name).is_ok_and(|effect| effect.deferred_decal));
    info!("a grenade's scorch: {} by {}, {:?} m, {} m deep, at most {}; laid by {laying:?}", decal.picture.name, decal.shader, decal.sizes, decal.projector_height, decal.max_decals);
    commands.insert_resource(Scorches { decal, picture, laying, laid: VecDeque::new(), rng: Rng::new(0x5c0c) });
}

/// At each grenade's blast whose effect lays one, its scorch: the box
/// (`trp_sim::decals::static_box`, its normal up as `EmitGrenade` gives it,
/// its size and turn drawn as the game draws them) as a clustered decal,
/// its unit box's x the box's first way across, its y the second's
/// opposite and its z the normal (Bevy's picture's place `(x, −y) + ½`
/// being then the game's), in the app's axes.
fn lay(mut commands: Commands, mut blasts: MessageReader<crate::loose::Blasted>, game: Res<crate::GameData>, mut scorches: ResMut<Scorches>) {
    for blasted in blasts.read() {
        let Some(name) = game.db.items.get(&blasted.template).and_then(|item| item.props.get("ExplosionEffectType")?.as_str()) else { continue };
        if !scorches.laying.iter().any(|n| n == name) {
            continue;
        }
        let size_share = scorches.rng.unit();
        let turn = scorches.decal.randomize_rotation.then(|| ((scorches.rng.unit() * 359.0) as u32).min(358) as f32);
        let b = trp_sim::decals::static_box(blasted.at, [0.0, 1.0, 0.0], scorches.decal.sizes, size_share, scorches.decal.projector_height, turn);
        let axis = |v: [f32; 3], length: f32| crate::convert::position(v) * length;
        let matrix = Mat4::from_cols(
            axis(b.right, 2.0 * b.half[0]).extend(0.0),
            axis(b.forward.map(|c| -c), 2.0 * b.half[2]).extend(0.0),
            axis(b.normal, 2.0 * b.half[1]).extend(0.0),
            crate::convert::position(b.center).extend(1.0),
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

/// Each mesh tagged with its material's stencil type (none where it is
/// nought), as it is given a material.
fn tag_by_stencil(mut commands: Commands, meshes: Query<(Entity, &MeshMaterial3d<StandardMaterial>, Option<&MeshTag>), Changed<MeshMaterial3d<StandardMaterial>>>) {
    for (entity, material, tag) in &meshes {
        let stencil = u32::from(stencil_of(&material.0));
        match (stencil, tag) {
            (0, Some(_)) => {
                commands.entity(entity).remove::<MeshTag>();
            }
            (0, None) => {}
            (s, Some(t)) if t.0 == s => {}
            (s, _) => {
                commands.entity(entity).insert(MeshTag(s));
            }
        }
    }
}

/// Bevy's decal step and the fragment that calls it.
const DECALS: &str = "embedded://bevy_pbr/decal/clustered.wgsl";
const FRAGMENT: &str = "embedded://bevy_pbr/render/pbr.wgsl";

#[derive(Resource)]
struct Held(Handle<Shader>, Handle<Shader>);

fn hold(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(Held(assets.load(DECALS), assets.load(FRAGMENT)));
}

/// The game's decal step, put after Bevy's (renamed).
const GAME_DECALS: &str = "
// (trp: the game's deferred decal, `decal_view.rs`; grenades spec, \"The
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
// stencil type not nought) takes none.
fn apply_decals_tagged(pbr_input: ptr<function, PbrInput>, tag: u32) {
#ifdef CLUSTERED_DECALS_ARE_USABLE
    let world_position = (*pbr_input).world_position.xyz;
    let dx = dpdx(world_position);
    let dy = dpdy(world_position);
    if tag != 0u {
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
    // (trp: the game's decal step with the mesh's tag, `decal_view.rs`.)
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
fn patch(held: Option<Res<Held>>, mut events: MessageReader<AssetEvent<Shader>>, mut shaders: ResMut<Assets<Shader>>) {
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
