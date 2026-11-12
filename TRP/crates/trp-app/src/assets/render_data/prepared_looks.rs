//! Prepared materials, the game's figures, and the prepared looks' meshes and materials.

use super::*;

/// A material made ready away from the engine's own thread: what a
/// `StandardMaterial` is made of, its pictures by their places in
/// `PreparedLooks::images`.
pub struct PreparedMaterial {
    pub(super) base_color: Color,
    pub(super) base: Option<usize>,
    pub(super) normal: Option<usize>,
    pub(super) gloss: Option<usize>,
    /// One picture with occlusion, roughness and metal in its three
    /// channels (a weapon of one's own's).
    pub(super) surface: Option<usize>,
    /// It is the stand-in for a material that could not be read.
    pub(super) fallback: bool,
    /// How rough it is, where that is not what its pictures say.
    pub(super) roughness: Option<f32>,
    /// The game's stencil type (`_StencilType`): 2 the hands' (the ambient
    /// highlight's, `highlight_view`), 1 characters', 0 the world's (which
    /// alone takes decals, `decal_view`).
    pub(super) stencil: u8,
    /// How it is blended: opaque but for a level's things drawn after the
    /// opaque ones (`scene_props`).
    pub(super) alpha: AlphaMode,
    /// The sights' own shader it is drawn with in the game, where it is one
    /// (`trp_data::sight::lens_shader`): what a model's lens, glass or
    /// collimator piece is, to leave out where the game draws nothing.
    pub(super) lens: Option<&'static str>,
    /// The figures the game's own surface shader reads (`GameFigures`),
    /// for an install's material; none for the app's own (a weapon of
    /// one's own's, the other game's suits, a level's things).
    pub(super) game: Option<GameFigures>,
    /// Its cubemap (`_Cube`) by its place in `images`, where cubes are
    /// read (`PreparedLooks::with_cubes`, or it is the glass) and it names
    /// one.
    pub(super) cube: Option<usize>,
    /// The dithered glass's figures and its specular picture by its place
    /// in `images`, where it is that and drawn see-through (`glass_view`).
    pub(super) glass: Option<(trp_data::drawing::glass::GlassFigures, Option<usize>)>,
    /// Drawn with the game's own surface on the OVERALL figure
    /// (`figure_surface`): an install's material, neither the glass's nor
    /// a sight's own shader's.
    pub(super) figure: bool,
}

/// What `p0/Reflective/Bumped Specular SMap` and its picture's twin read
/// of a material (icons spec, "The game's surface in a picture"), each as
/// the material saves it or, where it saves none, the shader's own default
/// (its parsed form): figures as stored, a gamma project's, none made
/// linear.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GameFigures {
    /// `_Color`, default white.
    pub color: [f32; 4],
    /// `_SpecColor`, default grey 0.5.
    pub spec_color: [f32; 4],
    /// `_ReflectColor`, default white (alpha 0.5).
    pub reflect_color: [f32; 4],
    /// `_DefVals`: the colour's scale and its rim's (default 0.5, 0.7).
    pub def_vals: [f32; 2],
    /// `_SpecVals`: the specular's scale and its rim's (default 1.1, 2).
    pub spec_vals: [f32; 2],
    /// `_Specularness`: the gloss picture's red's scale to the exponent's
    /// 128 (default 0.078125; the shader's own words call it "Glossness").
    pub specularness: f32,
    /// `_Glossness`: the colour picture's alpha's scale to the specular
    /// and the reflection (default 1; its words "Specularness").
    pub glossness: f32,
    /// `_BumpTiling`, the normal map's tiling (default 1).
    pub bump_tiling: f32,
    /// Whether it has a gloss picture (`_SpecMap`, default white: a red
    /// of one).
    pub has_gloss: bool,
    /// Whether its colour picture is read as stored (`Role::RawColor`),
    /// not decoded as sRGB.
    pub raw_colour: bool,
}

impl GameFigures {
    /// A material's figures as it saves them, the shader's defaults else.
    pub fn of(data: &trp_data::unity::extract::MaterialData, has_gloss: bool) -> Self {
        let color = |name: &str, default: [f32; 4]| data.colors.get(name).copied().unwrap_or(default);
        let float = |name: &str, default: f32| data.floats.get(name).copied().unwrap_or(default);
        let def_vals = color("_DefVals", [0.5, 0.7, 0.0, 0.0]);
        let spec_vals = color("_SpecVals", [1.1, 2.0, 0.0, 0.0]);
        Self {
            color: color("_Color", [1.0; 4]),
            spec_color: color("_SpecColor", [0.5, 0.5, 0.5, 1.0]),
            reflect_color: color("_ReflectColor", [1.0, 1.0, 1.0, 0.5]),
            def_vals: [def_vals[0], def_vals[1]],
            spec_vals: [spec_vals[0], spec_vals[1]],
            specularness: float("_Specularness", 0.078125),
            glossness: float("_Glossness", 1.0),
            bump_tiling: float("_BumpTiling", 1.0),
            has_gloss,
            raw_colour: false,
        }
    }
}

/// Meshes, pictures and materials made ready on another thread, as the
/// engine's own values and not yet in its stores: what `Converted` makes,
/// for a rig that is built away from the frame (`weapon::Preparing`).
/// `add_with_pictures` puts them all into the stores at once, which is quick.
#[derive(Default)]
pub struct PreparedLooks {
    pub meshes: Vec<Mesh>,
    pub(super) images: Vec<Image>,
    /// Where each picture's levels are had again when they are streamed
    /// (`streaming::Source`).
    pub(super) again: Vec<crate::assets::streaming::Source>,
    /// Each picture's mark, where they have been marked (`mark_pictures`).
    pub(super) marks: Vec<u64>,
    pub(super) materials: Vec<PreparedMaterial>,
    by_material: HashMap<Option<ObjRef>, usize>,
    by_image: HashMap<(ObjRef, Role), Option<usize>>,
    /// Whether a material's cubemap is read (`with_cubes`): only what the
    /// game's own surface is drawn for wants it (an item's picture, the
    /// inspect window's, the OVERALL figure's: `figure_surface`).
    pub(super) cubes: bool,
    by_cube: HashMap<ObjRef, Option<usize>>,
    /// Whether an install's colour pictures are read as stored
    /// (`raw_colours`).
    pub(super) raw: bool,
}

impl PreparedLooks {
    /// A mesh of the install's, one for each submesh (their places in
    /// `meshes`), with its bind poses, for a mesh drawn on bones (one for
    /// each bone, in its order).
    /// `mirrored` as for `Converted::mesh`.
    pub fn mesh_with_poses(&mut self, unity: &UnityAssets, obj: ObjRef, mirrored: bool) -> Result<(Vec<usize>, Vec<Mat4>)> {
        let data = read_mesh(unity, obj)?;
        let poses = data.bind_poses.iter().map(|m| convert::matrix(*m)).collect();
        Ok((self.made_mesh(&data, mirrored), poses))
    }

    /// One mesh posed on its bones where they lie, then drawn without them
    /// (`pose_on_bones`).
    pub fn pose_on_bones(&mut self, mesh: usize, matrices: &[Mat4]) {
        if let Some(mesh) = self.meshes.get_mut(mesh) {
            pose_on_bones(mesh, matrices);
        }
    }

    /// One mesh without the bones it would be skinned to (`unskinned`).
    pub fn unskin(&mut self, mesh: usize) {
        if let Some(mesh) = self.meshes.get_mut(mesh) {
            unskin(mesh);
        }
    }

    /// A mesh made as the install's are (a weapon of one's own's part).
    pub fn custom_mesh(&mut self, data: &MeshData) -> Vec<usize> {
        self.made_mesh(data, false)
    }

    fn made_mesh(&mut self, data: &MeshData, mirrored: bool) -> Vec<usize> {
        data.submeshes
            .iter()
            .map(|indices| {
                self.meshes.push(build_mesh(data, indices, mirrored));
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
                    *self.by_image.entry((tex, role)).or_insert_with(|| match picture_of(unity, tex, role) {
                        Ok((image, source)) => {
                            self.images.push(image);
                            self.again.push(source);
                            Some(self.images.len() - 1)
                        }
                        Err(e) => {
                            warn!("texture for {slot} of {}: {e:#}", data.name);
                            None
                        }
                    })
                };
                let colour = if self.raw { Role::RawColor } else { Role::Color };
                let (base, normal, gloss) = (texture("_MainTex", colour), texture("_BumpMap", Role::Normal), texture("_SpecMap", Role::Gloss));
                // The dithered glass's specular picture, read as stored and
                // kept whole (no material's picture of Bevy's, it is not
                // among what the streaming raises: `streaming::stream`).
                let glass = trp_data::drawing::glass::GlassFigures::of(&data).filter(|_| crate::effects::glass_view::on()).map(|figures| {
                    let spec = data.textures.get("_SpecTex").and_then(|&tex| match picture_of(unity, tex, Role::RawColor) {
                        Ok((image, _)) => {
                            self.images.push(image);
                            self.again.push(crate::assets::streaming::Source::Whole);
                            Some(self.images.len() - 1)
                        }
                        Err(e) => {
                            warn!("the specular picture of {}: {e:#}", data.name);
                            None
                        }
                    });
                    (figures, spec)
                });
                let tint = data.colors.get("_Color").copied().unwrap_or([1.0; 4]);
                let stencil = data.floats.get("_StencilType").map_or(0, |s| *s as u8);
                let lens = trp_data::weapons::sight::lens_shader(&data);
                // Its cubemap, whole (a few levels of a small, blurred
                // studio: 256 square and DXT1 for the Salewa's).
                let cube = match (self.cubes || glass.is_some(), data.textures.get("_Cube")) {
                    (true, Some(&tex)) => *self.by_cube.entry(tex).or_insert_with(|| match read_texture_shared(unity, tex).and_then(|(texture, _)| cube_image(&texture)) {
                        Ok(image) => {
                            self.images.push(image);
                            self.again.push(crate::assets::streaming::Source::Whole);
                            Some(self.images.len() - 1)
                        }
                        Err(e) => {
                            warn!("the cubemap of {}: {e:#}", data.name);
                            None
                        }
                    }),
                    _ => None,
                };
                let game = Some(GameFigures { raw_colour: self.raw, ..GameFigures::of(&data, gloss.is_some()) });
                let figure = lens.is_none() && trp_data::drawing::glass::GlassFigures::of(&data).is_none();
                PreparedMaterial { base_color: Color::srgb(tint[0], tint[1], tint[2]), base, normal, gloss, surface: None, fallback: false, roughness: None, stencil, alpha: AlphaMode::Opaque, lens, game, cube, glass, figure }
            }
            _ => PreparedMaterial { base_color: Color::srgb(0.8, 0.1, 0.8), base: None, normal: None, gloss: None, surface: None, fallback: true, roughness: None, stencil: 0, alpha: AlphaMode::Opaque, lens: None, game: None, cube: None, glass: None, figure: false },
        };
        self.materials.push(material);
        self.by_material.insert(obj, self.materials.len() - 1);
        self.materials.len() - 1
    }

    /// A material of a level's thing (`trp_data::scene_props`), as
    /// `material` makes one of the install's bundles: its colour, normal
    /// and gloss pictures (each made once, by its place in `pictures`, kept
    /// in `made`), its tint, and the world's stencil; drawn after the opaque
    /// ones (its queue past 2450: `AlphaTest` and beyond) see-through by
    /// its colour's alpha, cut at a half (`ESTIMATE`: the shaders' own
    /// cut-offs are not read) under 3000, blended from 3000 (`Transparent`).
    /// Its place in the list.
    pub fn prop_material(&mut self, material: &trp_data::level::scene_props::PropMaterial, pictures: &[TextureData], made: &mut HashMap<(usize, Role), Option<usize>>) -> usize {
        let mut picture = |at: Option<usize>, role: Role| -> Option<usize> {
            let at = at?;
            *made.entry((at, role)).or_insert_with(|| match build_image(&pictures[at], role) {
                Ok(image) => {
                    self.images.push(image);
                    self.again.push(crate::assets::streaming::Source::Kept);
                    Some(self.images.len() - 1)
                }
                Err(e) => {
                    warn!("a picture of {}: {e:#}", material.name);
                    None
                }
            })
        };
        let (base, normal, gloss) = (picture(material.base, Role::Color), picture(material.normal, Role::Normal), picture(material.gloss, Role::Gloss));
        let tint = material.color;
        let alpha = match material.queue {
            queue if queue >= 3000 => AlphaMode::Blend,
            queue if queue > 2450 => AlphaMode::Mask(0.5),
            _ => AlphaMode::Opaque,
        };
        self.materials.push(PreparedMaterial { base_color: Color::srgba(tint[0], tint[1], tint[2], tint[3]), base, normal, gloss, surface: None, fallback: false, roughness: None, stencil: 0, alpha, lens: None, game: None, cube: None, glass: None, figure: false });
        self.materials.len() - 1
    }

    /// The materials of a weapon of one's own, as `Converted::
    /// custom_materials` makes them: their places in the list.
    pub fn custom_materials(&mut self, list: &[trp_data::weapons::custom::Material]) -> Vec<usize> {
        let read = trp_data::weapons::custom::pictures_of(list);
        list.iter()
            .zip(read)
            .map(|(material, pictures)| {
                let pictures = pictures.unwrap_or_else(|e| {
                    warn!("the pictures of {}: {e:#}", material.name);
                    Default::default()
                });
                let mut add = |picture: Option<trp_data::weapons::custom::Picture>, format: TextureFormat| {
                    picture.map(|picture| {
                        self.images.push(picture_image(picture, format));
                        self.again.push(crate::assets::streaming::Source::Kept);
                        self.images.len() - 1
                    })
                };
                let surface = add(pictures.surface, TextureFormat::Rgba8Unorm);
                let (base, normal) = (add(pictures.colour, TextureFormat::Rgba8UnormSrgb), add(pictures.normal, TextureFormat::Rg8Unorm));
                self.materials.push(PreparedMaterial { base_color: Color::WHITE, base, normal, gloss: None, surface, fallback: false, roughness: None, stencil: 2, alpha: AlphaMode::Opaque, lens: None, game: None, cube: None, glass: None, figure: false });
                self.materials.len() - 1
            })
            .collect()
    }

    /// The material of a mesh of the other game's, as `Converted::
    /// outfit_material` makes it: its place in the list.
    pub fn stalker_material(&mut self, picture: &trp_data::formats::xray::Picture) -> usize {
        let (format, size) = stalker_picture(picture);
        self.images.push(image(&size, format, picture.levels as usize, picture.data.clone()));
        self.again.push(crate::assets::streaming::Source::Kept);
        // The other game's outfit, worn by a character (`PROVISIONAL`: a
        // character's stencil type, as the game's characters).
        self.materials.push(PreparedMaterial { base_color: Color::WHITE, base: Some(self.images.len() - 1), normal: None, gloss: None, surface: None, fallback: false, roughness: Some(CLOTH), stencil: 1, alpha: AlphaMode::Opaque, lens: None, game: None, cube: None, glass: None, figure: false });
        self.materials.len() - 1
    }

    /// Every mesh without the bones it would be skinned to: drawn as it
    /// lies in its own rest pose (an item's model for its picture, drawn
    /// with no skeleton). A mesh with bones and nothing to move them would
    /// ask the engine for a skinned drawing it cannot give.
    pub fn unskinned(&mut self) {
        self.meshes.iter_mut().for_each(unskin);
    }

    /// Each material's figures for the game's own surface and its cube by
    /// its place among the pictures, where it is an install's
    /// (`game_surface`); by the materials' places.
    pub fn game_figures(&self) -> Vec<Option<(GameFigures, Option<usize>)>> {
        self.materials.iter().map(|m| m.game.map(|game| (game, m.cube))).collect()
    }

    /// Its materials' cubemaps read as they are made (`material`), for
    /// the game's own surface (`game_surface`).
    pub fn with_cubes(mut self) -> Self {
        self.cubes = true;
        self
    }

    /// The same where the OVERALL figure is drawn with the game's own
    /// surface this run (`figure_surface`): for what it may wear or hold.
    pub fn with_figure_cubes(self) -> Self {
        if crate::screens::figure_surface::on() { self.with_cubes() } else { self }
    }

}
