//! Meshes unskinned, posed and built, and images built from textures.

use super::*;

/// A mesh's bones taken out: it is drawn as it lies. A mesh that keeps its
/// joints is drawn by the engine only on bones (a `SkinnedMesh`); with
/// none, the engine's skinned pipeline meets a bind group without them and
/// the frame fails (the AS VAL's and the VSS's hammer spring).
pub fn unskin(mesh: &mut Mesh) {
    mesh.remove_attribute(Mesh::ATTRIBUTE_JOINT_INDEX);
    mesh.remove_attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT);
}

/// A mesh carried by its bones as the engine skins it, then without them:
/// each point, normal and tangent by its bones' matrices (each a bone's
/// place in the mesh's own object's axes, times the bone's inverse bind
/// pose) by their weights. For a model drawn still (a picture, a thing on
/// the ground), where its bones lie in the prefab: the F-1's tripwire kit
/// hangs on bones the prefab shrinks to a hundredth, out of sight. A mesh
/// with no joints is left as it is.
pub fn pose_on_bones(mesh: &mut Mesh, matrices: &[Mat4]) {
    let (Some(VertexAttributeValues::Uint16x4(joints)), Some(VertexAttributeValues::Float32x4(weights))) = (mesh.attribute(Mesh::ATTRIBUTE_JOINT_INDEX), mesh.attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT)) else { return };
    let blended: Vec<Mat4> = joints
        .iter()
        .zip(weights)
        .map(|(joint, weight)| (0..4).filter(|k| weight[*k] != 0.0).filter_map(|k| matrices.get(joint[k] as usize).map(|bone| *bone * weight[k])).fold(Mat4::ZERO, |sum, part| sum + part))
        .collect();
    if let Some(VertexAttributeValues::Float32x3(points)) = mesh.attribute_mut(Mesh::ATTRIBUTE_POSITION) {
        for (point, by) in points.iter_mut().zip(&blended) {
            *point = by.transform_point3(Vec3::from(*point)).to_array();
        }
    }
    if let Some(VertexAttributeValues::Float32x3(normals)) = mesh.attribute_mut(Mesh::ATTRIBUTE_NORMAL) {
        for (normal, by) in normals.iter_mut().zip(&blended) {
            *normal = by.transform_vector3(Vec3::from(*normal)).normalize_or_zero().to_array();
        }
    }
    if let Some(VertexAttributeValues::Float32x4(tangents)) = mesh.attribute_mut(Mesh::ATTRIBUTE_TANGENT) {
        for (tangent, by) in tangents.iter_mut().zip(&blended) {
            let turned = by.transform_vector3(Vec3::new(tangent[0], tangent[1], tangent[2])).normalize_or_zero();
            *tangent = [turned.x, turned.y, turned.z, tangent[3]];
        }
    }
    unskin(mesh);
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

/// A mesh of the install's read away from `Converted` (one of Unity's own:
/// the boot shadow's plane, `boot_shadow`), one for each submesh, drawn by
/// a node that does not mirror it.
pub(crate) fn meshes_of(data: &MeshData) -> Vec<Mesh> {
    data.submeshes.iter().map(|indices| build_mesh(data, indices, false)).collect()
}

/// Builds the mesh for one submesh, keeping only the vertices it uses.
/// A mesh for Bevy from one of the install's submeshes. `mirrored`: drawn
/// by a node that mirrors it (`Hierarchy::mirrored`). The engine culls such
/// a renderer's other faces (Unity flips the culling where a renderer's
/// world matrix mirrors), which Bevy does not: its corners are left in
/// Unity's order instead, which the axes' own mirror (below) turns round.
/// Bevy already turns the tangents' handedness by the world matrix's sign,
/// as the engine does (`unity_WorldTransformParams.w`).
pub(super) fn build_mesh(data: &MeshData, indices: &[u32], mirrored: bool) -> Mesh {
    let vertex_count = data.positions.len();
    let mut remap = vec![u32::MAX; vertex_count];
    let mut used: Vec<usize> = Vec::new();
    let mut out_indices = Vec::with_capacity(indices.len());
    for tri in indices.chunks_exact(3) {
        if tri.iter().any(|&i| i as usize >= vertex_count) {
            continue;
        }
        // Mirroring Z turns Unity's clockwise front faces counter-clockwise
        // only if two corners are swapped; a node that mirrors them again
        // keeps them.
        let corners = if mirrored { [tri[0], tri[1], tri[2]] } else { [tri[0], tri[2], tri[1]] };
        for i in corners {
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

/// A picture of the install's as the engine takes it, and where its levels
/// are had again when they are streamed (`streaming::Source`): not at all
/// where the game's engine does not stream it; read again from its bundle's
/// file, made again as it was made, where they are kept beside its
/// serialized file; else kept.
pub(super) fn picture_of(unity: &UnityAssets, tex: ObjRef, role: Role) -> Result<(Image, crate::assets::streaming::Source)> {
    use crate::assets::streaming::Source;
    let (texture, place) = read_texture_shared(unity, tex)?;
    let image = build_image(&texture, role)?;
    let source = match place {
        _ if !texture.streaming_mipmaps => Source::Whole,
        None => Source::Kept,
        Some(place) => {
            let mut header = texture;
            header.data = Vec::new();
            Source::Bundle(std::sync::Arc::new(move || {
                let mut texture = header.clone();
                texture.data = place.read().ok()?;
                build_image(&texture, role).ok()
            }))
        }
    };
    Ok((image, source))
}

pub(crate) fn build_image(tex: &TextureData, role: Role) -> Result<Image> {
    let mips = texture::mips(tex)?;
    let block_aligned = tex.width % 4 == 0 && tex.height % 4 == 0;
    match role {
        Role::Color | Role::RawColor => {
            let srgb = tex.srgb && role == Role::Color;
            let compressed = match (tex.format, srgb) {
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
            let fmt = if srgb { TextureFormat::Rgba8UnormSrgb } else { TextureFormat::Rgba8Unorm };
            Ok(image(tex, fmt, mips.len(), data))
        }
        Role::Normal => {
            // DXT5 normal maps keep X in alpha and Y in green. Two channels
            // are enough: the shader rebuilds Z. Re-packed block by block
            // as BC5 where the picture is DXT1 or DXT5 (the GPU reads two
            // channels of that as a normal map), which costs a few
            // operations a block where decoding to pixels and repacking
            // them cost most of a weapon's build (`texture::two_channels`).
            // A BC7 one is laid out as a DXT5's (red full, X in alpha, Y in
            // green and blue: `probe texstats`, the M4A1's and the MPX's).
            if tex.format == format::BC7 {
                return bc7_as_bc5(tex, &mips, block_aligned, crate::assets::bc7_maps::Taken::Normals);
            }
            let swizzled = tex.format == format::DXT5;
            let channels = if swizzled { [texture::Channel::Alpha, texture::Channel::Green] } else { [texture::Channel::Red, texture::Channel::Green] };
            if let Some(data) = texture::two_channels(tex, channels)?.filter(|_| block_aligned) {
                return Ok(image(tex, TextureFormat::Bc5RgUnorm, mips.len(), data));
            }
            // Any other, pixel by pixel, as the game's surface shader reads
            // every normal map (`p0/Reflective/Bumped Specular SMap`: X red
            // times alpha, Y green), which takes the install's layout (red
            // full, X in alpha, as the Glock 17's slide's RGBA32 has it) and
            // one of red and green alike.
            debug!("{}: a normal map of format {} read pixel by pixel", tex.name, tex.format);
            let mut data = Vec::new();
            for mip in &mips {
                for px in texture::decode_rgba(tex, *mip)?.chunks_exact(4) {
                    data.extend_from_slice(&crate::assets::bc7_maps::Taken::Normals.of([px[0], px[1], px[2], px[3]]));
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
            // A BC7 one is grey as the others (red, green and blue alike).
            if tex.format == format::BC7 {
                return bc7_as_bc5(tex, &mips, block_aligned, crate::assets::bc7_maps::Taken::Roughness);
            }
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

/// A normal or gloss picture packed as BC7: the two channels the material
/// reads (`taken`), as BC5, a byte a pixel as the BC7 was (`bc7_maps`); two
/// bytes a pixel unpacked where its size is not of whole blocks.
fn bc7_as_bc5(tex: &TextureData, mips: &[texture::Mip], block_aligned: bool, taken: crate::assets::bc7_maps::Taken) -> Result<Image> {
    if !block_aligned {
        let mut data = Vec::new();
        for mip in mips {
            data.extend(texture::decode_rgba(tex, *mip)?.chunks_exact(4).flat_map(|px| taken.of([px[0], px[1], px[2], px[3]])));
        }
        return Ok(image(tex, TextureFormat::Rg8Unorm, mips.len(), data));
    }
    let end = mips.last().map_or(0, |m| m.offset + m.size);
    let data = crate::assets::bc7_maps::to_bc5(&tex.data[..end], taken, &tex.name);
    Ok(image(tex, TextureFormat::Bc5RgUnorm, mips.len(), data))
}

/// A picture of a weapon that is not the install's, as an image: in the
/// compressed format its blocks are (`custom::Blocks`), the colour space
/// the one asked for.
pub(super) fn picture_image(picture: trp_data::weapons::custom::Picture, format: TextureFormat) -> Image {
    use trp_data::weapons::custom::Blocks;
    let format = match (picture.blocks, format) {
        (Some(Blocks::Bc1), TextureFormat::Rgba8UnormSrgb) => TextureFormat::Bc1RgbaUnormSrgb,
        (Some(Blocks::Bc1), _) => TextureFormat::Bc1RgbaUnorm,
        (Some(Blocks::Bc5), _) => TextureFormat::Bc5RgUnorm,
        (None, format) => format,
    };
    let size = TextureData { name: String::new(), width: picture.width, height: picture.height, format: 0, mip_count: picture.mips, srgb: false, data: Vec::new(), streaming_mipmaps: false, point: false, clamp: [false; 2], faces: 1 };
    image(&size, format, picture.mips as usize, picture.data)
}

/// A cubemap as the engine's cube picture: its six faces in the install's
/// order (+X, -X, +Y, -Y, +Z, -Z, as the graphics card's), each with its
/// levels after it, as the install keeps them; DXT1 and DXT5 as they are,
/// any other decoded. Read as stored (not as sRGB): the game's project is
/// in gamma, and its shader adds the cube's figures as they are.
pub(crate) fn cube_image(tex: &TextureData) -> Result<Image> {
    anyhow::ensure!(tex.faces == 6, "{} is not a cubemap ({} faces)", tex.name, tex.faces);
    let face_bytes = tex.data.len() / 6;
    let one = |face: usize| TextureData { data: tex.data[face * face_bytes..(face + 1) * face_bytes].to_vec(), faces: 1, ..tex.clone() };
    let mips = texture::mips(&one(0))?;
    let compressed = match tex.format {
        format::DXT1 => Some(TextureFormat::Bc1RgbaUnorm),
        format::DXT5 => Some(TextureFormat::Bc3RgbaUnorm),
        _ => None,
    };
    let block_aligned = tex.width % 4 == 0 && tex.height % 4 == 0;
    let mut data = Vec::new();
    let format = match compressed.filter(|_| block_aligned) {
        Some(format) => {
            let end = mips.last().map_or(0, |m| m.offset + m.size);
            for face in 0..6 {
                data.extend_from_slice(&tex.data[face * face_bytes..face * face_bytes + end]);
            }
            format
        }
        None => {
            for face in 0..6 {
                let face = one(face);
                for mip in &mips {
                    data.extend(texture::decode_rgba(&face, *mip)?);
                }
            }
            TextureFormat::Rgba8Unorm
        }
    };
    let mut image = image(tex, format, mips.len(), data);
    image.texture_descriptor.size.depth_or_array_layers = 6;
    image.texture_view_descriptor = Some(bevy::render::render_resource::TextureViewDescriptor { dimension: Some(bevy::render::render_resource::TextureViewDimension::Cube), ..default() });
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::ClampToEdge,
        address_mode_v: ImageAddressMode::ClampToEdge,
        address_mode_w: ImageAddressMode::ClampToEdge,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Linear,
        ..default()
    });
    Ok(image)
}

pub(super) fn image(tex: &TextureData, format: TextureFormat, mip_count: usize, data: Vec<u8>) -> Image {
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
