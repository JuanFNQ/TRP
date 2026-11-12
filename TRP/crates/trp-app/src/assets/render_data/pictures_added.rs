//! The prepared looks' pictures counted, marked and added, and shared pictures.

use super::*;

impl PreparedLooks {
    /// An install's colour pictures read as stored, as the game's shader in
    /// its gamma project samples them (`Role::RawColor`): for a model drawn
    /// with the game's own surface alone (an item's picture). Read as sRGB,
    /// the graphics card filters the texels in linear light, which a
    /// picture much smaller than its texture lightens (the Iskra's 4 to 6
    /// in 255 over the game's).
    pub fn raw_colours(mut self) -> Self {
        self.raw = true;
        self
    }

    /// Whether a material is the stand-in for one that could not be read.
    pub fn is_fallback(&self, material: usize) -> bool {
        self.materials.get(material).is_some_and(|m| m.fallback)
    }

    /// The sights' own shader a material is drawn with in the game, where
    /// it is one (`PreparedMaterial::lens`).
    pub fn lens(&self, material: usize) -> Option<&'static str> {
        self.materials.get(material).and_then(|m| m.lens)
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
    /// materials and of the pictures, each in its place; the pictures
    /// whole, whatever the settings of their levels (`streaming`), as the
    /// game draws an item for its picture (`ItemIconCreator`:
    /// `requestedMipmapLevel` 0).
    pub fn add_with_pictures(self, meshes: &mut Assets<Mesh>, images: &mut Assets<Image>, materials: &mut Assets<StandardMaterial>) -> (Vec<Handle<Mesh>>, Vec<Handle<StandardMaterial>>, Vec<Handle<Image>>) {
        let (meshes, materials, pictures, _) = self.add_looks(meshes, images, materials, None, false);
        (meshes, materials, pictures)
    }

    /// The same, a picture marked as one already there (`mark_pictures`)
    /// taken from `shared` in place of its own, and those put there for
    /// any that come after; with the marks of all it draws with, to let go
    /// of when it goes (`SharedPictures::release`), and how many of its
    /// pictures and their bytes were already on the card. The pictures by
    /// the settings of their levels (`streaming`).
    pub fn add_sharing(
        self,
        meshes: &mut Assets<Mesh>,
        images: &mut Assets<Image>,
        materials: &mut Assets<StandardMaterial>,
        shared: Option<&mut SharedPictures>,
    ) -> (Vec<Handle<Mesh>>, Vec<Handle<StandardMaterial>>, Vec<Handle<Image>>, Shared) {
        self.add_looks(meshes, images, materials, shared, true)
    }

    fn add_looks(
        self,
        meshes: &mut Assets<Mesh>,
        images: &mut Assets<Image>,
        materials: &mut Assets<StandardMaterial>,
        mut shared: Option<&mut SharedPictures>,
        streamed: bool,
    ) -> (Vec<Handle<Mesh>>, Vec<Handle<StandardMaterial>>, Vec<Handle<Image>>, Shared) {
        // For the renderer's world alone: handed over to it they are moved
        // there, where what the main world keeps as well is copied, on the
        // main thread and all in the frame they come (Bevy's
        // `extract_render_asset`): 36 ms of the frame the StG 44's rig came
        // in. Nothing reads them in the main world after this.
        let mut kept = Shared::default();
        let pictures = add_pictures(self.images, self.again, &self.marks, images, shared.as_deref_mut(), &mut kept, streamed);
        let picture = |at: Option<usize>| at.map(|at| pictures[at].clone());
        let made = self
            .materials
            .iter()
            .map(|m| {
                let handle = materials.add(StandardMaterial {
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
                    alpha_mode: m.alpha,
                    ..default()
                });
                if m.stencil == 2 {
                    crate::rendering::highlight_view::note_hands(&handle);
                }
                crate::effects::decal_view::note_stencil(&handle, m.stencil);
                if let Some((figures, spec)) = m.glass {
                    crate::effects::glass_view::note(&handle, crate::effects::glass_view::GlassNote { figures, spec: picture(spec), cube: picture(m.cube) });
                }
                if let (Some(game), true) = (m.game, m.figure) {
                    crate::screens::figure_surface::note(&handle, game, picture(m.cube));
                }
                handle
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

/// Pictures into the engine's store, for the renderer's world alone: one
/// marked as one already there (`PreparedLooks::mark_pictures`) taken from
/// `shared` in place of its own, and those put there for any that come
/// after; what was taken noted in `kept`. `streamed`: by the settings of
/// their levels (`streaming::add`); else whole.
fn add_pictures(
    own: Vec<Image>,
    again: Vec<crate::assets::streaming::Source>,
    marks: &[u64],
    images: &mut Assets<Image>,
    mut shared: Option<&mut SharedPictures>,
    kept: &mut Shared,
    streamed: bool,
) -> Vec<Handle<Image>> {
    let marked = marks.len() == own.len();
    let mut add = |image: Image, again: crate::assets::streaming::Source| if streamed { crate::assets::streaming::add(images, image, again) } else { images.add(image) };
    let again = again.into_iter().chain(std::iter::repeat_with(|| crate::assets::streaming::Source::Kept));
    own.into_iter()
        .zip(again)
        .enumerate()
        .map(|(index, (mut image, again))| {
            let mark = marks.get(index).copied().filter(|_| marked);
            if let (Some(mark), Some(shared)) = (mark, shared.as_deref_mut()) {
                kept.marks.push(mark);
                if let Some((handle, holders)) = shared.0.get_mut(&mark) {
                    *holders += 1;
                    kept.already += 1;
                    kept.bytes_spared += image.data.as_ref().map_or(0, Vec::len);
                    return handle.clone();
                }
                image.asset_usage = RenderAssetUsages::RENDER_WORLD;
                let handle = add(image, again);
                shared.0.insert(mark, (handle.clone(), 1));
                return handle;
            }
            image.asset_usage = RenderAssetUsages::RENDER_WORLD;
            add(image, again)
        })
        .collect()
}

/// The pictures of the weapons' rigs on the graphics card, each once, by
/// its mark (`PreparedLooks::mark_pictures`), with how many rigs draw with
/// it: a rig whose picture is the same as one already there draws with
/// that one, and a picture goes when the last
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
