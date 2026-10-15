//! Turning extracted Unity objects into Bevy meshes, images and materials.
//!
//! The materials are an approximation: Tarkov's weapon shader is a
//! specular/gloss model and Bevy's is metallic/roughness.

use std::collections::HashMap;

use anyhow::Result;
use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology, VertexAttributeValues};
use bevy::render::render_resource::{Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages};
use trp_data::unity::extract::{read_material, read_mesh, read_texture, MeshData, TextureData};
use trp_data::unity::texture::{self, format};
use trp_data::unity::{Assets as UnityAssets, ObjRef};

use crate::convert;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Role {
    Color,
    Normal,
    Gloss,
}

#[derive(Clone)]
pub struct MeshEntry {
    /// One mesh per submesh.
    pub parts: Vec<Handle<Mesh>>,
    pub bind_poses: Vec<Mat4>,
}

/// Bevy assets already built from Unity objects.
#[derive(Resource, Default)]
pub struct Converted {
    meshes: HashMap<ObjRef, MeshEntry>,
    materials: HashMap<ObjRef, Handle<StandardMaterial>>,
    images: HashMap<(ObjRef, Role), Option<Handle<Image>>>,
    fallback: Option<Handle<StandardMaterial>>,
}

impl Converted {
    pub fn mesh(&mut self, unity: &UnityAssets, obj: ObjRef, meshes: &mut Assets<Mesh>) -> Result<MeshEntry> {
        if let Some(entry) = self.meshes.get(&obj) {
            return Ok(entry.clone());
        }
        let data = read_mesh(unity, obj)?;
        let entry = MeshEntry {
            parts: data.submeshes.iter().map(|indices| meshes.add(build_mesh(&data, indices))).collect(),
            bind_poses: data.bind_poses.iter().map(|m| convert::matrix(*m)).collect(),
        };
        self.meshes.insert(obj, entry.clone());
        Ok(entry)
    }

    pub fn material(
        &mut self,
        unity: &UnityAssets,
        obj: Option<ObjRef>,
        images: &mut Assets<Image>,
        materials: &mut Assets<StandardMaterial>,
    ) -> Handle<StandardMaterial> {
        let Some(obj) = obj else {
            return self.fallback(materials);
        };
        if let Some(handle) = self.materials.get(&obj) {
            return handle.clone();
        }
        let Ok(data) = read_material(unity, obj) else {
            return self.fallback(materials);
        };
        let mut texture = |slot: &str, role: Role| -> Option<Handle<Image>> {
            let tex = *data.textures.get(slot)?;
            self.images
                .entry((tex, role))
                .or_insert_with(|| match read_texture(unity, tex).and_then(|t| build_image(&t, role)) {
                    Ok(image) => Some(images.add(image)),
                    Err(e) => {
                        warn!("texture for {slot} of {}: {e:#}", data.name);
                        None
                    }
                })
                .clone()
        };
        let base_color_texture = texture("_MainTex", Role::Color);
        let normal_map_texture = texture("_BumpMap", Role::Normal);
        let metallic_roughness_texture = texture("_SpecMap", Role::Gloss);
        let tint = data.colors.get("_Color").copied().unwrap_or([1.0; 4]);
        let handle = materials.add(StandardMaterial {
            base_color: Color::srgb(tint[0], tint[1], tint[2]),
            base_color_texture,
            normal_map_texture,
            // The texture's green channel carries roughness; this scales it.
            perceptual_roughness: if metallic_roughness_texture.is_some() { 1.0 } else { 0.6 },
            metallic: 0.0,
            metallic_roughness_texture,
            ..default()
        });
        self.materials.insert(obj, handle.clone());
        handle
    }

    /// The meshes of a part of a weapon that is not the install's
    /// (`trp_data::custom`), one for each of its submeshes: built as the
    /// install's own are, from a mesh in the same manner.
    pub fn custom_mesh(&mut self, data: &MeshData, meshes: &mut Assets<Mesh>) -> Vec<Handle<Mesh>> {
        data.submeshes.iter().map(|indices| meshes.add(build_mesh(data, indices))).collect()
    }

    /// The materials of such a weapon, from its own pictures: colour,
    /// normals, and one picture with occlusion, roughness and metal in
    /// its three channels, which is how the standard shader reads them.
    /// Unlike the install's (whose shader has gloss and no metal, and is
    /// approximated) these are what the shader is made for, and are used
    /// as they are. (Tried both ways in captures on 2026-10-13: with the
    /// metal the rifle is dark steel with bright worn edges, much as the
    /// install's AKM is drawn; without it, a flat grey-brown.)
    pub fn custom_materials(
        &mut self,
        list: &[trp_data::custom::Material],
        images: &mut Assets<Image>,
        materials: &mut Assets<StandardMaterial>,
    ) -> Vec<Handle<StandardMaterial>> {
        let read = trp_data::custom::pictures_of(list);
        list.iter()
            .zip(read)
            .map(|(material, pictures)| {
                let pictures = pictures.unwrap_or_else(|e| {
                    warn!("the pictures of {}: {e:#}", material.name);
                    Default::default()
                });
                let mut add = |picture: Option<trp_data::custom::Picture>, format: TextureFormat| picture.map(|picture| images.add(picture_image(picture, format)));
                let surface = add(pictures.surface, TextureFormat::Rgba8Unorm);
                materials.add(StandardMaterial {
                    base_color_texture: add(pictures.colour, TextureFormat::Rgba8UnormSrgb),
                    normal_map_texture: add(pictures.normal, TextureFormat::Rg8Unorm),
                    // The picture's own roughness and metal, at full scale.
                    perceptual_roughness: if surface.is_some() { 1.0 } else { 0.6 },
                    metallic: if surface.is_some() { 1.0 } else { 0.0 },
                    occlusion_texture: surface.clone(),
                    metallic_roughness_texture: surface,
                    ..default()
                })
            })
            .collect()
    }

    /// The material of a mesh of an outfit that is not the install's
    /// (`trp_data::outfit`): its own picture for its colour, as the file
    /// has it packed, and nothing else of that game's shading. It is
    /// drawn as cloth by the standard shader (rough, no metal), which is
    /// an approximation as the install's own materials are.
    pub fn outfit_material(&mut self, picture: &trp_data::xray::Picture, images: &mut Assets<Image>, materials: &mut Assets<StandardMaterial>) -> Handle<StandardMaterial> {
        let (format, size) = stalker_picture(picture);
        let colour = images.add(image(&size, format, picture.levels as usize, picture.data.clone()));
        materials.add(StandardMaterial { base_color_texture: Some(colour), perceptual_roughness: CLOTH, metallic: 0.0, ..default() })
    }

    fn fallback(&mut self, materials: &mut Assets<StandardMaterial>) -> Handle<StandardMaterial> {
        self.fallback
            .get_or_insert_with(|| materials.add(StandardMaterial::from_color(Color::srgb(0.8, 0.1, 0.8))))
            .clone()
    }
}

/// How rough the other game's cloth is drawn (`Converted::outfit_material`).
const CLOTH: f32 = 0.9;

/// A picture of the other game's: the card's format for its packing, and
/// its size as the install's pictures give it.
fn stalker_picture(picture: &trp_data::xray::Picture) -> (TextureFormat, TextureData) {
    use trp_data::xray::Packing;
    let format = match picture.packing {
        Packing::Bc1 => TextureFormat::Bc1RgbaUnormSrgb,
        Packing::Bc2 => TextureFormat::Bc2RgbaUnormSrgb,
        Packing::Bc3 => TextureFormat::Bc3RgbaUnormSrgb,
        Packing::Bc7 => TextureFormat::Bc7RgbaUnormSrgb,
        Packing::Rgba => TextureFormat::Rgba8UnormSrgb,
    };
    (format, TextureData { name: String::new(), width: picture.width, height: picture.height, format: 0, mip_count: picture.levels, srgb: true, data: Vec::new() })
}

/// A material made ready away from the engine's own thread: what a
/// `StandardMaterial` is made of, its pictures by their places in
/// `PreparedLooks::images`.
pub struct PreparedMaterial {
    base_color: Color,
    base: Option<usize>,
    normal: Option<usize>,
    gloss: Option<usize>,
    /// One picture with occlusion, roughness and metal in its three
    /// channels (a weapon of one's own's).
    surface: Option<usize>,
    /// It is the stand-in for a material that could not be read.
    fallback: bool,
    /// How rough it is, where that is not what its pictures say.
    roughness: Option<f32>,
}

/// Meshes, pictures and materials made ready on another thread, as the
/// engine's own values and not yet in its stores: what `Converted` makes,
/// for a rig that is built away from the frame (`weapon::Preparing`).
/// `add_with_pictures` puts them all into the stores at once, which is quick.
#[derive(Default)]
pub struct PreparedLooks {
    pub meshes: Vec<Mesh>,
    images: Vec<Image>,
    /// Each picture's mark, where they have been marked (`mark_pictures`).
    marks: Vec<u64>,
    materials: Vec<PreparedMaterial>,
    by_material: HashMap<Option<ObjRef>, usize>,
    by_image: HashMap<(ObjRef, Role), Option<usize>>,
}

impl PreparedLooks {
    /// A mesh of the install's, one for each submesh: their places in
    /// `meshes`.
    pub fn mesh(&mut self, unity: &UnityAssets, obj: ObjRef) -> Result<Vec<usize>> {
        let data = read_mesh(unity, obj)?;
        Ok(self.custom_mesh(&data))
    }

    /// The same, with the mesh's bind poses, for a mesh to be drawn on
    /// bones (one for each bone, in its order).
    pub fn mesh_with_poses(&mut self, unity: &UnityAssets, obj: ObjRef) -> Result<(Vec<usize>, Vec<Mat4>)> {
        let data = read_mesh(unity, obj)?;
        let poses = data.bind_poses.iter().map(|m| convert::matrix(*m)).collect();
        Ok((self.custom_mesh(&data), poses))
    }

    /// One mesh without the bones it would be skinned to (`unskinned`).
    pub fn unskin(&mut self, mesh: usize) {
        if let Some(mesh) = self.meshes.get_mut(mesh) {
            unskin(mesh);
        }
    }

    /// A mesh made as the install's are (a weapon of one's own's part).
    pub fn custom_mesh(&mut self, data: &MeshData) -> Vec<usize> {
        data.submeshes
            .iter()
            .map(|indices| {
                self.meshes.push(build_mesh(data, indices));
                self.meshes.len() - 1
            })
            .collect()
    }

    /// A material of the install's, as `Converted::material` makes it:
    /// its place in the list. None, or one that cannot be read, is the
    /// fallback.
    pub fn material(&mut self, unity: &UnityAssets, obj: Option<ObjRef>) -> usize {
        if let Some(at) = self.by_material.get(&obj) {
            return *at;
        }
        let material = match obj.map(|obj| read_material(unity, obj)) {
            Some(Ok(data)) => {
                let mut texture = |slot: &str, role: Role| -> Option<usize> {
                    let tex = *data.textures.get(slot)?;
                    *self.by_image.entry((tex, role)).or_insert_with(|| match read_texture(unity, tex).and_then(|t| build_image(&t, role)) {
                        Ok(image) => {
                            self.images.push(image);
                            Some(self.images.len() - 1)
                        }
                        Err(e) => {
                            warn!("texture for {slot} of {}: {e:#}", data.name);
                            None
                        }
                    })
                };
                let (base, normal, gloss) = (texture("_MainTex", Role::Color), texture("_BumpMap", Role::Normal), texture("_SpecMap", Role::Gloss));
                let tint = data.colors.get("_Color").copied().unwrap_or([1.0; 4]);
                PreparedMaterial { base_color: Color::srgb(tint[0], tint[1], tint[2]), base, normal, gloss, surface: None, fallback: false, roughness: None }
            }
            _ => PreparedMaterial { base_color: Color::srgb(0.8, 0.1, 0.8), base: None, normal: None, gloss: None, surface: None, fallback: true, roughness: None },
        };
        self.materials.push(material);
        self.by_material.insert(obj, self.materials.len() - 1);
        self.materials.len() - 1
    }

    /// The materials of a weapon of one's own, as `Converted::
    /// custom_materials` makes them: their places in the list.
    pub fn custom_materials(&mut self, list: &[trp_data::custom::Material]) -> Vec<usize> {
        let read = trp_data::custom::pictures_of(list);
        list.iter()
            .zip(read)
            .map(|(material, pictures)| {
                let pictures = pictures.unwrap_or_else(|e| {
                    warn!("the pictures of {}: {e:#}", material.name);
                    Default::default()
                });
                let mut add = |picture: Option<trp_data::custom::Picture>, format: TextureFormat| {
                    picture.map(|picture| {
                        self.images.push(picture_image(picture, format));
                        self.images.len() - 1
                    })
                };
                let surface = add(pictures.surface, TextureFormat::Rgba8Unorm);
                let (base, normal) = (add(pictures.colour, TextureFormat::Rgba8UnormSrgb), add(pictures.normal, TextureFormat::Rg8Unorm));
                self.materials.push(PreparedMaterial { base_color: Color::WHITE, base, normal, gloss: None, surface, fallback: false, roughness: None });
                self.materials.len() - 1
            })
            .collect()
    }

    /// The material of a mesh of the other game's, as `Converted::
    /// outfit_material` makes it: its place in the list.
    pub fn stalker_material(&mut self, picture: &trp_data::xray::Picture) -> usize {
        let (format, size) = stalker_picture(picture);
        self.images.push(image(&size, format, picture.levels as usize, picture.data.clone()));
        self.materials.push(PreparedMaterial { base_color: Color::WHITE, base: Some(self.images.len() - 1), normal: None, gloss: None, surface: None, fallback: false, roughness: Some(CLOTH) });
        self.materials.len() - 1
    }

    /// Every mesh without the bones it would be skinned to: drawn as it
    /// lies in its own rest pose (an item's model for its picture, drawn
    /// with no skeleton). A mesh with bones and nothing to move them would
    /// ask the engine for a skinned drawing it cannot give.
    pub fn unskinned(&mut self) {
        self.meshes.iter_mut().for_each(unskin);
    }

    /// Whether a material is the stand-in for one that could not be read.
    pub fn is_fallback(&self, material: usize) -> bool {
        self.materials.get(material).is_some_and(|m| m.fallback)
    }

    /// How many pictures it has.
    pub fn picture_count(&self) -> usize {
        self.images.len()
    }

    /// How many bytes its pictures have.
    pub fn picture_bytes(&self) -> usize {
        self.images.iter().map(|image| image.data.as_ref().map_or(0, Vec::len)).sum()
    }

    /// Marks every picture by all that it is: its size, its format, its
    /// levels, how it is sampled and every byte of it. Two pictures with
    /// the same mark are drawn alike, and a rig's picture with the mark of
    /// one already on the graphics card is drawn with that one
    /// (`SharedPictures`). Done away from the frame, with the rest of a
    /// rig's making.
    pub fn mark_pictures(&mut self) {
        use std::hash::{BuildHasher, Hash, Hasher};
        self.marks = self
            .images
            .iter()
            .map(|image| {
                let mut hasher = bevy::platform::hash::FixedHasher.build_hasher();
                let descriptor = &image.texture_descriptor;
                (descriptor.size, descriptor.mip_level_count, descriptor.format, format!("{:?}", image.sampler)).hash(&mut hasher);
                image.data.as_deref().unwrap_or_default().hash(&mut hasher);
                hasher.finish()
            })
            .collect();
    }

    /// Into the engine's stores: the handles of the meshes, of the
    /// materials and of the pictures, each in its place.
    pub fn add_with_pictures(self, meshes: &mut Assets<Mesh>, images: &mut Assets<Image>, materials: &mut Assets<StandardMaterial>) -> (Vec<Handle<Mesh>>, Vec<Handle<StandardMaterial>>, Vec<Handle<Image>>) {
        let (meshes, materials, pictures, _) = self.add_sharing(meshes, images, materials, None);
        (meshes, materials, pictures)
    }

    /// The same, a picture marked as one already there (`mark_pictures`)
    /// taken from `shared` in place of its own, and those put there for
    /// any that come after; with the marks of all it draws with, to let go
    /// of when it goes (`SharedPictures::release`), and how many of its
    /// pictures and their bytes were already on the card.
    pub fn add_sharing(
        self,
        meshes: &mut Assets<Mesh>,
        images: &mut Assets<Image>,
        materials: &mut Assets<StandardMaterial>,
        mut shared: Option<&mut SharedPictures>,
    ) -> (Vec<Handle<Mesh>>, Vec<Handle<StandardMaterial>>, Vec<Handle<Image>>, Shared) {
        // For the renderer's world alone: handed over to it they are moved
        // there, where what the main world keeps as well is copied, on the
        // main thread and all in the frame they come (Bevy's
        // `extract_render_asset`): 36 ms of the frame the StG 44's rig came
        // in. Nothing reads them in the main world after this.
        let marked = self.marks.len() == self.images.len();
        let mut kept = Shared::default();
        let pictures: Vec<Handle<Image>> = self
            .images
            .into_iter()
            .enumerate()
            .map(|(index, mut image)| {
                let mark = self.marks.get(index).copied().filter(|_| marked);
                if let (Some(mark), Some(shared)) = (mark, shared.as_deref_mut()) {
                    kept.marks.push(mark);
                    if let Some((handle, holders)) = shared.0.get_mut(&mark) {
                        *holders += 1;
                        kept.already += 1;
                        kept.bytes_spared += image.data.as_ref().map_or(0, Vec::len);
                        return handle.clone();
                    }
                    image.asset_usage = RenderAssetUsages::RENDER_WORLD;
                    let handle = images.add(image);
                    shared.0.insert(mark, (handle.clone(), 1));
                    return handle;
                }
                image.asset_usage = RenderAssetUsages::RENDER_WORLD;
                images.add(image)
            })
            .collect();
        let picture = |at: Option<usize>| at.map(|at| pictures[at].clone());
        let made = self
            .materials
            .iter()
            .map(|m| {
                materials.add(StandardMaterial {
                    base_color: m.base_color,
                    base_color_texture: picture(m.base),
                    normal_map_texture: picture(m.normal),
                    // As `Converted::material` and `custom_materials` have
                    // them: the gloss picture's roughness at full scale and
                    // no metal; a surface picture's roughness and metal both.
                    perceptual_roughness: m.roughness.unwrap_or(if m.gloss.is_some() || m.surface.is_some() { 1.0 } else { 0.6 }),
                    metallic: if m.surface.is_some() { 1.0 } else { 0.0 },
                    occlusion_texture: picture(m.surface),
                    metallic_roughness_texture: picture(m.gloss.or(m.surface)),
                    ..default()
                })
            })
            .collect();
        let meshes = self
            .meshes
            .into_iter()
            .map(|mut mesh| {
                mesh.asset_usage = RenderAssetUsages::RENDER_WORLD;
                meshes.add(mesh)
            })
            .collect();
        (meshes, made, pictures, kept)
    }
}

/// The pictures of the weapons' rigs on the graphics card, each once, by
/// its mark (`PreparedLooks::mark_pictures`), with how many rigs draw with
/// it: a rig whose picture is the same as one already there draws with
/// that one (the user, 2026-10-14, night: "The pictures should apply to
/// weapons that look the exact same"), and a picture goes when the last
/// rig that draws with it goes (`release`).
#[derive(Resource, Default)]
pub struct SharedPictures(HashMap<u64, (Handle<Image>, u32)>);

impl SharedPictures {
    /// A rig that drew with these has gone.
    pub fn release(&mut self, marks: &[u64]) {
        for mark in marks {
            if let Some((_, holders)) = self.0.get_mut(mark) {
                *holders -= 1;
                if *holders == 0 {
                    self.0.remove(mark);
                }
            }
        }
    }

    /// How many pictures are kept.
    pub fn len(&self) -> usize {
        self.0.len()
    }
}

/// What a rig took of `SharedPictures`: the marks of all it draws with, and
/// how many of its pictures (and their bytes) were already there.
#[derive(Default)]
pub struct Shared {
    pub marks: Vec<u64>,
    pub already: usize,
    pub bytes_spared: usize,
}

/// A mesh's bones taken out: it is drawn as it lies. A mesh that keeps its
/// joints is drawn by the engine only on bones (a `SkinnedMesh`); with
/// none, the engine's skinned pipeline meets a bind group without them and
/// the frame fails (the AS VAL's and the VSS's hammer spring, 2026-10-14).
pub fn unskin(mesh: &mut Mesh) {
    mesh.remove_attribute(Mesh::ATTRIBUTE_JOINT_INDEX);
    mesh.remove_attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT);
}

/// Meshes in the store drawn as they lie (`unskin`); one with no bones is
/// left untouched, so as not to be sent to the card again.
pub fn unskin_all(meshes: &mut Assets<Mesh>, handles: &[Handle<Mesh>]) {
    for handle in handles {
        if meshes.get(handle).is_some_and(|mesh| mesh.attribute(Mesh::ATTRIBUTE_JOINT_INDEX).is_some()) {
            if let Some(mesh) = meshes.get_mut(handle) {
                unskin(mesh);
            }
        }
    }
}

/// Builds the mesh for one submesh, keeping only the vertices it uses.
fn build_mesh(data: &MeshData, indices: &[u32]) -> Mesh {
    let vertex_count = data.positions.len();
    let mut remap = vec![u32::MAX; vertex_count];
    let mut used: Vec<usize> = Vec::new();
    let mut out_indices = Vec::with_capacity(indices.len());
    for tri in indices.chunks_exact(3) {
        if tri.iter().any(|&i| i as usize >= vertex_count) {
            continue;
        }
        // Mirroring Z turns Unity's clockwise front faces counter-clockwise
        // only if two corners are swapped.
        for i in [tri[0], tri[2], tri[1]] {
            let slot = &mut remap[i as usize];
            if *slot == u32::MAX {
                *slot = used.len() as u32;
                used.push(i as usize);
            }
            out_indices.push(*slot);
        }
    }

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    let positions: Vec<[f32; 3]> = used.iter().map(|&i| convert::position(data.positions[i]).to_array()).collect();
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    let has_normals = data.normals.len() == vertex_count;
    if has_normals {
        let normals: Vec<[f32; 3]> = used
            .iter()
            .map(|&i| convert::position(data.normals[i]).normalize_or_zero().to_array())
            .collect();
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    }
    let has_uvs = data.uv0.len() == vertex_count;
    if has_uvs {
        // Texture rows are uploaded in Unity's bottom-up order, so the
        // coordinates need no flip either.
        let uvs: Vec<[f32; 2]> = used.iter().map(|&i| data.uv0[i]).collect();
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    }
    if has_normals && has_uvs && data.tangents.len() == vertex_count {
        let tangents: Vec<[f32; 4]> = used
            .iter()
            .map(|&i| {
                let t = data.tangents[i];
                let v = convert::position([t[0], t[1], t[2]]).normalize_or_zero();
                // The mirror reverses the cross product, so the sign flips.
                [v.x, v.y, v.z, -t[3]]
            })
            .collect();
        mesh.insert_attribute(Mesh::ATTRIBUTE_TANGENT, tangents);
    }
    if data.weights.len() == vertex_count && data.joints.len() == vertex_count {
        let joints: Vec<[u16; 4]> = used.iter().map(|&i| data.joints[i]).collect();
        let weights: Vec<[f32; 4]> = used.iter().map(|&i| data.weights[i]).collect();
        mesh.insert_attribute(Mesh::ATTRIBUTE_JOINT_INDEX, VertexAttributeValues::Uint16x4(joints));
        mesh.insert_attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT, weights);
    }
    mesh.insert_indices(Indices::U32(out_indices));
    mesh
}

fn build_image(tex: &TextureData, role: Role) -> Result<Image> {
    let mips = texture::mips(tex)?;
    let block_aligned = tex.width % 4 == 0 && tex.height % 4 == 0;
    match role {
        Role::Color => {
            let compressed = match (tex.format, tex.srgb) {
                (format::DXT1, true) => Some(TextureFormat::Bc1RgbaUnormSrgb),
                (format::DXT1, false) => Some(TextureFormat::Bc1RgbaUnorm),
                (format::DXT5, true) => Some(TextureFormat::Bc3RgbaUnormSrgb),
                (format::DXT5, false) => Some(TextureFormat::Bc3RgbaUnorm),
                (format::BC7, true) => Some(TextureFormat::Bc7RgbaUnormSrgb),
                (format::BC7, false) => Some(TextureFormat::Bc7RgbaUnorm),
                _ => None,
            };
            if let Some(fmt) = compressed.filter(|_| block_aligned) {
                // The GPU reads these as they are; no decoding needed.
                let end = mips.last().map_or(0, |m| m.offset + m.size);
                return Ok(image(tex, fmt, mips.len(), tex.data[..end].to_vec()));
            }
            let mut data = Vec::new();
            for mip in &mips {
                data.extend(texture::decode_rgba(tex, *mip)?);
            }
            let fmt = if tex.srgb { TextureFormat::Rgba8UnormSrgb } else { TextureFormat::Rgba8Unorm };
            Ok(image(tex, fmt, mips.len(), data))
        }
        Role::Normal => {
            // DXT5 normal maps keep X in alpha and Y in green. Two channels
            // are enough: the shader rebuilds Z. Re-packed block by block
            // as BC5 where the picture is DXT1 or DXT5 (the GPU reads two
            // channels of that as a normal map), which costs a few
            // operations a block where decoding to pixels and repacking
            // them cost most of a weapon's build (`texture::two_channels`).
            let swizzled = tex.format == format::DXT5;
            let channels = if swizzled { [texture::Channel::Alpha, texture::Channel::Green] } else { [texture::Channel::Red, texture::Channel::Green] };
            if let Some(data) = texture::two_channels(tex, channels)?.filter(|_| block_aligned) {
                return Ok(image(tex, TextureFormat::Bc5RgUnorm, mips.len(), data));
            }
            let mut data = Vec::new();
            for mip in &mips {
                for px in texture::decode_rgba(tex, *mip)?.chunks_exact(4) {
                    data.extend_from_slice(&[if swizzled { px[3] } else { px[0] }, px[1]]);
                }
            }
            Ok(image(tex, TextureFormat::Rg8Unorm, mips.len(), data))
        }
        Role::Gloss => {
            // Greyscale gloss becomes roughness in green, the channel Bevy
            // reads it from. Blue is metallic and stays at full so the
            // material's own metallic value decides. Re-packed block by
            // block where it can be: roughness in green, and no blue,
            // which the material's metallic of none makes the same.
            if let Some(data) = texture::two_channels(tex, [texture::Channel::Full, texture::Channel::RedInverted])?.filter(|_| block_aligned) {
                return Ok(image(tex, TextureFormat::Bc5RgUnorm, mips.len(), data));
            }
            let mut data = Vec::new();
            for mip in &mips {
                for px in texture::decode_rgba(tex, *mip)?.chunks_exact(4) {
                    data.extend_from_slice(&[255, 255 - px[0], 255, 255]);
                }
            }
            Ok(image(tex, TextureFormat::Rgba8Unorm, mips.len(), data))
        }
    }
}

/// A picture of a weapon that is not the install's, as an image: in the
/// compressed format its blocks are (`custom::Blocks`), the colour space
/// the one asked for.
fn picture_image(picture: trp_data::custom::Picture, format: TextureFormat) -> Image {
    use trp_data::custom::Blocks;
    let format = match (picture.blocks, format) {
        (Some(Blocks::Bc1), TextureFormat::Rgba8UnormSrgb) => TextureFormat::Bc1RgbaUnormSrgb,
        (Some(Blocks::Bc1), _) => TextureFormat::Bc1RgbaUnorm,
        (Some(Blocks::Bc5), _) => TextureFormat::Bc5RgUnorm,
        (None, format) => format,
    };
    let size = TextureData { name: String::new(), width: picture.width, height: picture.height, format: 0, mip_count: picture.mips, srgb: false, data: Vec::new() };
    image(&size, format, picture.mips as usize, picture.data)
}

fn image(tex: &TextureData, format: TextureFormat, mip_count: usize, data: Vec<u8>) -> Image {
    let mut image = Image::default();
    image.texture_descriptor = TextureDescriptor {
        label: None,
        size: Extent3d { width: tex.width, height: tex.height, depth_or_array_layers: 1 },
        mip_level_count: mip_count as u32,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format,
        usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
        view_formats: &[],
    };
    image.data = Some(data);
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Linear,
        anisotropy_clamp: 8,
        ..default()
    });
    image
}
