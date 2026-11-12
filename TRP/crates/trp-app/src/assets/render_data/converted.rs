//! Converted meshes and materials, and STALKER pictures.

use super::*;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Role {
    Color,
    /// A colour picture read as stored, not as sRGB: as a gamma project's
    /// shader samples it, its texels filtered in its own figures (the
    /// game's own surface in a picture: `PreparedLooks::raw_colours`).
    RawColor,
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
    /// By the mesh and whether the node drawing it is mirrored.
    pub(super) meshes: HashMap<(ObjRef, bool), MeshEntry>,
    pub(super) materials: HashMap<ObjRef, Handle<StandardMaterial>>,
    pub(super) images: HashMap<(ObjRef, Role), Option<Handle<Image>>>,
    pub(super) fallback: Option<Handle<StandardMaterial>>,
    /// The materials' cubemaps, each read once, for the OVERALL figure's
    /// own surface (`figure_surface`).
    pub(super) cubes: HashMap<ObjRef, Option<Handle<Image>>>,
}

impl Converted {
    /// A mesh of the install's, one for each submesh. `mirrored`: the node
    /// drawing it is (`Hierarchy::mirrored`), and its faces are turned
    /// round to be culled as the engine culls them.
    pub fn mesh(&mut self, unity: &UnityAssets, obj: ObjRef, mirrored: bool, meshes: &mut Assets<Mesh>) -> Result<MeshEntry> {
        if let Some(entry) = self.meshes.get(&(obj, mirrored)) {
            return Ok(entry.clone());
        }
        let data = read_mesh(unity, obj)?;
        let entry = MeshEntry {
            parts: data.submeshes.iter().map(|indices| meshes.add(build_mesh(&data, indices, mirrored))).collect(),
            bind_poses: data.bind_poses.iter().map(|m| convert::matrix(*m)).collect(),
        };
        self.meshes.insert((obj, mirrored), entry.clone());
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
                .or_insert_with(|| match picture_of(unity, tex, role) {
                    Ok((image, source)) => Some(crate::assets::streaming::add(images, image, source)),
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
        let has_gloss = metallic_roughness_texture.is_some();
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
        // The game's stencil type for hands (`_StencilType` 2), which the
        // ambient highlight lights again (`highlight_view`).
        if data.floats.get("_StencilType") == Some(&2.0) {
            crate::rendering::highlight_view::note_hands(&handle);
        }
        // And every stencil type but the world's, which decals keep off
        // (`decal_view`).
        crate::effects::decal_view::note_stencil(&handle, data.floats.get("_StencilType").map_or(0, |s| *s as u8));
        // The dithered glass, with its specular picture and its cube, each
        // whole, where it is drawn see-through (`glass_view`).
        if let Some(figures) = trp_data::drawing::glass::GlassFigures::of(&data).filter(|_| crate::effects::glass_view::on()) {
            let spec = data.textures.get("_SpecTex").and_then(|&tex| match picture_of(unity, tex, Role::RawColor) {
                Ok((image, _)) => Some(crate::assets::streaming::add(images, image, crate::assets::streaming::Source::Whole)),
                Err(e) => {
                    warn!("the specular picture of {}: {e:#}", data.name);
                    None
                }
            });
            let cube = data.textures.get("_Cube").and_then(|&tex| match read_texture_shared(unity, tex).and_then(|(texture, _)| cube_image(&texture)) {
                Ok(image) => Some(images.add(image)),
                Err(e) => {
                    warn!("the cubemap of {}: {e:#}", data.name);
                    None
                }
            });
            crate::effects::glass_view::note(&handle, crate::effects::glass_view::GlassNote { figures, spec, cube });
        }
        // The game's own surface's figures, and its cube whole, for the
        // OVERALL figure (`figure_surface`): not the glass's, nor the
        // sights' own shaders'.
        if crate::screens::figure_surface::on() && trp_data::drawing::glass::GlassFigures::of(&data).is_none() && trp_data::weapons::sight::lens_shader(&data).is_none() {
            let cube = data.textures.get("_Cube").and_then(|&tex| {
                self.cubes
                    .entry(tex)
                    .or_insert_with(|| match read_texture_shared(unity, tex).and_then(|(texture, _)| cube_image(&texture)) {
                        Ok(image) => Some(images.add(image)),
                        Err(e) => {
                            warn!("the cubemap of {}: {e:#}", data.name);
                            None
                        }
                    })
                    .clone()
            });
            crate::screens::figure_surface::note(&handle, GameFigures::of(&data, has_gloss), cube);
        }
        self.materials.insert(obj, handle.clone());
        handle
    }

    /// The meshes of a part of a weapon that is not the install's
    /// (`trp_data::custom`), one for each of its submeshes: built as the
    /// install's own are, from a mesh in the same manner.
    pub fn custom_mesh(&mut self, data: &MeshData, meshes: &mut Assets<Mesh>) -> Vec<Handle<Mesh>> {
        data.submeshes.iter().map(|indices| meshes.add(build_mesh(data, indices, false))).collect()
    }

    /// The materials of such a weapon, from its own pictures: colour,
    /// normals, and one picture with occlusion, roughness and metal in
    /// its three channels, which is how the standard shader reads them.
    /// Unlike the install's (whose shader has gloss and no metal, and is
    /// approximated) these are what the shader is made for, and are used
    /// as they are. (Tried both ways in captures: with the
    /// metal the rifle is dark steel with bright worn edges, much as the
    /// install's AKM is drawn; without it, a flat grey-brown.)
    pub fn custom_materials(
        &mut self,
        list: &[trp_data::weapons::custom::Material],
        images: &mut Assets<Image>,
        materials: &mut Assets<StandardMaterial>,
    ) -> Vec<Handle<StandardMaterial>> {
        let read = trp_data::weapons::custom::pictures_of(list);
        list.iter()
            .zip(read)
            .map(|(material, pictures)| {
                let pictures = pictures.unwrap_or_else(|e| {
                    warn!("the pictures of {}: {e:#}", material.name);
                    Default::default()
                });
                let mut add = |picture: Option<trp_data::weapons::custom::Picture>, format: TextureFormat| picture.map(|picture| crate::assets::streaming::add(images, picture_image(picture, format), crate::assets::streaming::Source::Kept));
                let surface = add(pictures.surface, TextureFormat::Rgba8Unorm);
                let handle = materials.add(StandardMaterial {
                    base_color_texture: add(pictures.colour, TextureFormat::Rgba8UnormSrgb),
                    normal_map_texture: add(pictures.normal, TextureFormat::Rg8Unorm),
                    // The picture's own roughness and metal, at full scale.
                    perceptual_roughness: if surface.is_some() { 1.0 } else { 0.6 },
                    metallic: if surface.is_some() { 1.0 } else { 0.0 },
                    occlusion_texture: surface.clone(),
                    metallic_roughness_texture: surface,
                    ..default()
                });
                // A weapon of one's own, of the hands' stencil type as the
                // game's weapons are (`PROVISIONAL`: its materials are not
                // the game's).
                crate::rendering::highlight_view::note_hands(&handle);
                crate::effects::decal_view::note_stencil(&handle, 2);
                handle
            })
            .collect()
    }

    /// The material of a mesh of an outfit that is not the install's
    /// (`trp_data::outfit`): its own picture for its colour, as the file
    /// has it packed, and nothing else of that game's shading. It is
    /// drawn as cloth by the standard shader (rough, no metal), which is
    /// an approximation as the install's own materials are.
    pub fn outfit_material(&mut self, picture: &trp_data::formats::xray::Picture, images: &mut Assets<Image>, materials: &mut Assets<StandardMaterial>) -> Handle<StandardMaterial> {
        let (format, size) = stalker_picture(picture);
        let colour = crate::assets::streaming::add(images, image(&size, format, picture.levels as usize, picture.data.clone()), crate::assets::streaming::Source::Kept);
        materials.add(StandardMaterial { base_color_texture: Some(colour), perceptual_roughness: CLOTH, metallic: 0.0, ..default() })
    }

    pub(super) fn fallback(&mut self, materials: &mut Assets<StandardMaterial>) -> Handle<StandardMaterial> {
        self.fallback
            .get_or_insert_with(|| materials.add(StandardMaterial::from_color(Color::srgb(0.8, 0.1, 0.8))))
            .clone()
    }
}

/// How rough the other game's cloth is drawn (`Converted::outfit_material`).
pub(super) const CLOTH: f32 = 0.9;

/// A picture of the other game's: the card's format for its packing, and
/// its size as the install's pictures give it.
pub(super) fn stalker_picture(picture: &trp_data::formats::xray::Picture) -> (TextureFormat, TextureData) {
    use trp_data::formats::xray::Packing;
    let format = match picture.packing {
        Packing::Bc1 => TextureFormat::Bc1RgbaUnormSrgb,
        Packing::Bc2 => TextureFormat::Bc2RgbaUnormSrgb,
        Packing::Bc3 => TextureFormat::Bc3RgbaUnormSrgb,
        Packing::Bc7 => TextureFormat::Bc7RgbaUnormSrgb,
        Packing::Rgba => TextureFormat::Rgba8UnormSrgb,
    };
    (format, TextureData { name: String::new(), width: picture.width, height: picture.height, format: 0, mip_count: picture.levels, srgb: true, data: Vec::new(), streaming_mipmaps: false, point: false, clamp: [false; 2], faces: 1 })
}
