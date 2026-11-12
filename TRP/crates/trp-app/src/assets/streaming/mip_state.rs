//! Mip streaming's state, new pictures taken, and the card watched.

use super::*;

/// The pictures of the models as they stand.
#[derive(Resource)]
pub struct MipStreaming {
    pub settings: TextureSettings,
    /// By the first copy's id.
    pub(super) pictures: HashMap<AssetId<Image>, Streamed>,
    /// Each copy's picture, by the copy's id (the first's own among them).
    pub(super) family: HashMap<AssetId<Image>, AssetId<Image>>,
    /// The materials drawing with each picture, and the pictures each
    /// material draws with.
    pub(super) users: HashMap<AssetId<Image>, HashSet<AssetId<StandardMaterial>>>,
    drawn_with: HashMap<AssetId<StandardMaterial>, Vec<AssetId<Image>>>,
    /// Each mesh's texel density: its surface over its pictures' (world
    /// area over UV area, the mesh's own size).
    pub(super) density: HashMap<AssetId<Mesh>, f32>,
    pub(super) since_decided: f32,
    pub(super) clock: f32,
    pub(super) reported: (usize, usize, usize),
    /// How many times a picture's materials were pointed at another copy:
    /// a held view (`HeldView`) draws afresh when it changes, else it
    /// would go on showing the copy it drew with.
    pub swaps: u64,
}

/// A camera whose picture stays on the screen while it does not draw (a
/// preview's, drawn afresh only for a few frames after a change: the edit
/// build screen's, the inspect window's). Its sight counts for the
/// pictures of what is on its own layers as a drawing camera's does,
/// drawing or not. Counted only while drawing, the armor
/// bench's carrier went down to its lowest level two seconds after each
/// redraw and was raised by the next, too late to be drawn with: shown
/// sharp or blurred by turns.
#[derive(Component)]
pub struct HeldView;

impl MipStreaming {
    pub fn new(settings: TextureSettings) -> Self {
        set(settings);
        Self {
            settings,
            pictures: HashMap::new(),
            family: HashMap::new(),
            users: HashMap::new(),
            drawn_with: HashMap::new(),
            density: HashMap::new(),
            since_decided: 0.0,
            clock: 0.0,
            reported: (0, 0, 0),
            swaps: 0,
        }
    }

    /// The copy to draw with for a picture named by any of its copies, if
    /// it is not that one.
    fn now_for(&self, named: AssetId<Image>) -> Option<Handle<Image>> {
        let picture = self.family.get(&named)?;
        let current = self.pictures.get(picture)?.current.as_ref()?;
        (current.id() != named).then(|| current.clone())
    }

    /// The materials drawing with a picture pointed at its copy from
    /// `level` (the first, where none), the copy before let go.
    pub(super) fn point_to(&mut self, materials: &mut Assets<StandardMaterial>, id: AssetId<Image>, level: u32, copy: Option<Handle<Image>>) {
        let Some(streamed) = self.pictures.get_mut(&id) else { return };
        streamed.loaded = level;
        self.swaps += 1;
        let before = std::mem::replace(&mut streamed.current, copy.clone()).map(|before| before.id());
        let to = copy.unwrap_or_else(|| streamed.first.clone());
        // The copies a material may name now: the first and the one before.
        let was = [Some(id), before];
        if let Some(before) = before {
            self.family.remove(&before);
        }
        self.family.insert(to.id(), id);
        for material in self.users.get(&id).into_iter().flatten() {
            let Some(material) = materials.get_mut(*material) else { continue };
            for slot in pictures_of(material) {
                if slot.as_ref().is_some_and(|named| named.id() != to.id() && was.contains(&Some(named.id()))) {
                    *slot = Some(to.clone());
                }
            }
        }
    }

    /// A picture nothing holds and nothing draws with any more, let go.
    pub(super) fn forget(&mut self, picture: AssetId<Image>) {
        if let Some(streamed) = self.pictures.remove(&picture) {
            self.family.remove(&picture);
            if let (Some((_, copy)), Ok(mut lists)) = (&streamed.arriving, ON_CARD.lock()) {
                lists.0.retain(|id| *id != copy.id());
            }
            if let Some(current) = &streamed.current {
                self.family.remove(&current.id());
            }
        }
        self.users.remove(&picture);
    }
}

/// The picture fields of a material.
fn pictures_of(material: &mut StandardMaterial) -> [&mut Option<Handle<Image>>; 5] {
    [&mut material.base_color_texture, &mut material.normal_map_texture, &mut material.metallic_roughness_texture, &mut material.occlusion_texture, &mut material.emissive_texture]
}

/// The pictures added, taken in; new meshes' texel densities; each new
/// material noted with the pictures it draws with, and pointed at their
/// copies of the moment where it was made with older ones (a rig's picture
/// shared with a thing on the ground, `SharedPictures`); a material gone,
/// let go of. In `Last`, after the stores' events of the frame.
pub fn take_new(
    mut streaming: ResMut<MipStreaming>,
    meshes: Res<Assets<Mesh>>,
    mut mesh_events: MessageReader<AssetEvent<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut material_events: MessageReader<AssetEvent<StandardMaterial>>,
) {
    let streaming = &mut *streaming;
    if let Ok(mut added) = ADDED.lock() {
        for streamed in added.drain(..) {
            let id = streamed.first.id();
            streaming.family.insert(id, id);
            streaming.pictures.insert(id, streamed);
        }
    }
    for event in mesh_events.read() {
        match *event {
            AssetEvent::Added { id } => {
                if let Some(density) = meshes.get(id).and_then(texel_density) {
                    streaming.density.insert(id, density);
                }
            }
            AssetEvent::Removed { id } => {
                streaming.density.remove(&id);
            }
            _ => {}
        }
    }
    for event in material_events.read() {
        match *event {
            AssetEvent::Added { id } => {
                // (Taken to be changed only where a picture is: a material
                // changed is prepared again.)
                let Some(material) = materials.get(id) else { continue };
                let mut firsts = Vec::new();
                let mut stale = false;
                for slot in [&material.base_color_texture, &material.normal_map_texture, &material.metallic_roughness_texture, &material.occlusion_texture, &material.emissive_texture].into_iter().flatten() {
                    let Some(first) = streaming.family.get(&slot.id()).copied() else { continue };
                    firsts.push(first);
                    stale |= streaming.now_for(slot.id()).is_some();
                }
                if firsts.is_empty() {
                    continue;
                }
                for first in &firsts {
                    streaming.users.entry(*first).or_default().insert(id);
                }
                streaming.drawn_with.insert(id, firsts);
                if stale {
                    if let Some(material) = materials.get_mut(id) {
                        for slot in pictures_of(material) {
                            if let Some(now) = slot.as_ref().and_then(|named| streaming.now_for(named.id())) {
                                *slot = Some(now);
                            }
                        }
                    }
                }
            }
            AssetEvent::Removed { id } => {
                for picture in streaming.drawn_with.remove(&id).into_iter().flatten() {
                    let Some(users) = streaming.users.get_mut(&picture) else { continue };
                    users.remove(&id);
                    if users.is_empty() && !streaming.pictures.get(&picture).is_some_and(Streamed::held) {
                        streaming.forget(picture);
                    }
                }
            }
            _ => {}
        }
    }
}

/// A mesh's surface over its pictures' surface (Unity's
/// `Mesh.GetUVDistributionMetric`, taken here as the whole surface over the
/// whole of it in the pictures: `PROVISIONAL`, the engine's own sum is not
/// published), in its own size.
fn texel_density(mesh: &Mesh) -> Option<f32> {
    use bevy::mesh::VertexAttributeValues;
    let Some(VertexAttributeValues::Float32x3(positions)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION) else { return None };
    let Some(VertexAttributeValues::Float32x2(uvs)) = mesh.attribute(Mesh::ATTRIBUTE_UV_0) else { return None };
    let indices: Vec<usize> = mesh.indices()?.iter().collect();
    let (mut surface, mut picture) = (0.0f64, 0.0f64);
    for triangle in indices.chunks_exact(3) {
        let [a, b, c] = [triangle[0], triangle[1], triangle[2]];
        let (pa, pb, pc) = (Vec3::from(*positions.get(a)?), Vec3::from(*positions.get(b)?), Vec3::from(*positions.get(c)?));
        surface += ((pb - pa).cross(pc - pa).length() * 0.5) as f64;
        let (ua, ub, uc) = (Vec2::from(*uvs.get(a)?), Vec2::from(*uvs.get(b)?), Vec2::from(*uvs.get(c)?));
        picture += ((ub - ua).perp_dot(uc - ua).abs() * 0.5) as f64;
    }
    (picture > 1e-12 && surface > 0.0).then(|| (surface / picture) as f32)
}

/// The copies added and not yet seen on the card, and those the renderer
/// has seen there since (`see_on_card`).
pub(super) static ON_CARD: Mutex<(Vec<AssetId<Image>>, Vec<AssetId<Image>>)> = Mutex::new((Vec::new(), Vec::new()));

/// In the renderer, after its pictures are readied: each copy waited for
/// that is on the card now, said to be.
pub fn see_on_card(images: Res<bevy::render::render_asset::RenderAssets<bevy::render::texture::GpuImage>>) {
    let Ok(mut lists) = ON_CARD.lock() else { return };
    if lists.0.is_empty() {
        return;
    }
    let (asked, there) = &mut *lists;
    asked.retain(|id| {
        let on = images.get(*id).is_some();
        if on {
            there.push(*id);
        }
        !on
    });
}

/// The renderer's side of the streaming: `see_on_card` after its pictures
/// are readied.
pub fn watch_card(app: &mut App) {
    use bevy::render::{Render, RenderApp, RenderSystems};
    if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
        render_app.add_systems(Render, see_on_card.after(RenderSystems::PrepareAssets).before(RenderSystems::Render));
    }
}
