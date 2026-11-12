//! A custom part's definition, its kept nodes, marks and mesh picks.

use super::*;

/// A part of one's own: an item made from one of the install's parts, as
/// a weapon of one's own is made from one of the install's weapons. Its
/// template is its donor's with what its file changes, and its own slots;
/// on a weapon, the donor's prefab is hung as the game hangs a part, for
/// what the game goes by on such a part (a scope's camera, its lens and
/// the eye's bone), with nothing of it drawn but the objects its file
/// keeps, and the model's pieces drawn in its place.
#[derive(Debug, Clone, Deserialize)]
pub struct PartDefinition {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub short_name: String,
    /// The install's part its template and prefab are its donor's.
    pub donor: String,
    /// What of the model it draws.
    pub meshes: Vec<MeshPick>,
    /// The model's point that lies at the bone of the slot it is in, in
    /// the model's units and axes (x to the right, y up, z back).
    pub at_slot: [f32; 3],
    /// Its size as a share of the model's, about that point: its slots'
    /// points with it, so that parts of one model fitted to each other at
    /// one share still fit.
    #[serde(default = "full_size")]
    pub scale: f32,
    /// A scope's reticle of its own, in place of its donor's lens's
    /// (`_MarkTex`): drawn black on nothing, as the install's are.
    #[serde(default)]
    pub mark: Option<MarkDef>,
    /// Its own slots, each with its bone's point on the model: the donor's
    /// slots of the same names, taking what these say. It has no others.
    #[serde(default)]
    pub slots: Vec<SlotDef>,
    /// The donor's objects that are drawn still, by name: a scope's lens.
    #[serde(default)]
    pub kept: Vec<KeptNode>,
    #[serde(default)]
    pub template: Changes,
    /// Its file's model, and the folder the file's paths are from.
    #[serde(skip)]
    pub model: Model,
    #[serde(skip)]
    pub root: PathBuf,
}

/// An object of a part's donor that is drawn on the part: its name, and
/// its size as a share of its own (a lens fitted to the model's eyepiece).
#[derive(Debug, Clone, Deserialize)]
pub struct KeptNode {
    pub name: String,
    #[serde(default = "full_size")]
    pub scale: f32,
}

fn full_size() -> f32 {
    1.0
}

/// A reticle's picture: a square of `size` pixels, its rows from the top as
/// the eye sees it through the scope and its middle where the scope looks,
/// as the install's marks are laid out (the PU's: its post's tip at the
/// middle of 2048 pixels); each rectangle of `black` (from the left and
/// the top, to the right and the bottom, in pixels) drawn opaque black, the
/// rest clear.
#[derive(Debug, Clone, Deserialize)]
pub struct MarkDef {
    pub size: u32,
    pub black: Vec<[u32; 4]>,
}

impl PartDefinition {
    /// Its reticle's picture, as a picture of the install's is read
    /// (RGBA32, its rows from the bottom up), made once a run: the app keeps
    /// a lens's picture by the picture.
    pub fn mark_picture(&self) -> Option<Arc<crate::unity::extract::TextureData>> {
        static MADE: std::sync::OnceLock<std::sync::Mutex<HashMap<String, Arc<crate::unity::extract::TextureData>>>> = std::sync::OnceLock::new();
        let mark = self.mark.as_ref()?;
        let made = MADE.get_or_init(Default::default);
        if let Some(picture) = made.lock().ok().and_then(|kept| kept.get(&self.id).cloned()) {
            return Some(picture);
        }
        let size = mark.size.max(1) as usize;
        let mut data = vec![0u8; size * size * 4];
        for &[left, top, right, bottom] in &mark.black {
            for row in (top as usize).min(size)..(bottom as usize).min(size) {
                // Kept from the bottom up.
                let stored = size - 1 - row;
                for column in (left as usize).min(size)..(right as usize).min(size) {
                    data[(stored * size + column) * 4 + 3] = 255;
                }
            }
        }
        let picture = Arc::new(crate::unity::extract::TextureData {
            name: format!("{} (reticle)", self.name),
            width: size as u32,
            height: size as u32,
            format: crate::unity::texture::format::RGBA32,
            mip_count: 1,
            srgb: true,
            data,
            streaming_mipmaps: false,
            point: false,
            clamp: [true, true],
            faces: 1,
        });
        if let Ok(mut kept) = made.lock() {
            kept.insert(self.id.clone(), picture.clone());
        }
        Some(picture)
    }
}

/// What of one of the model's meshes a part draws: its connected pieces
/// (`fbx::Mesh::piece`), by where they lie and by number. A piece is
/// taken where it lies wholly inside one of the boxes `inside`, if any
/// are given, and wholly inside none of the boxes `outside`; those named
/// in `except` are left out and those in `also` taken whatever the boxes
/// say. Boxes are their least and most corners, in the model's units and
/// axes.
#[derive(Debug, Clone, Deserialize)]
pub struct MeshPick {
    pub model: String,
    #[serde(default)]
    pub inside: Vec<[[f32; 3]; 2]>,
    #[serde(default)]
    pub outside: Vec<[[f32; 3]; 2]>,
    #[serde(default)]
    pub except: Vec<u32>,
    #[serde(default)]
    pub also: Vec<u32>,
}

impl MeshPick {
    /// Whether each of a mesh's pieces is taken.
    pub(super) fn pieces(&self, mesh: &fbx::Mesh) -> Vec<bool> {
        let count = mesh.pieces as usize;
        let (mut low, mut high) = (vec![[f32::MAX; 3]; count], vec![[f32::MIN; 3]; count]);
        for (corner, p) in mesh.positions.iter().enumerate() {
            let piece = mesh.piece[corner / 3] as usize;
            (0..3).for_each(|k| (low[piece][k], high[piece][k]) = (low[piece][k].min(p[k]), high[piece][k].max(p[k])));
        }
        let within = |piece: usize, b: &[[f32; 3]; 2]| (0..3).all(|k| low[piece][k] >= b[0][k] && high[piece][k] <= b[1][k]);
        (0..count)
            .map(|piece| {
                let number = piece as u32;
                if self.also.contains(&number) {
                    return true;
                }
                !self.except.contains(&number) && (self.inside.is_empty() || self.inside.iter().any(|b| within(piece, b))) && !self.outside.iter().any(|b| within(piece, b))
            })
            .collect()
    }
}

/// A mesh with only its pieces taken.
pub(super) fn picked(mesh: &fbx::Mesh, taken: &[bool]) -> fbx::Mesh {
    let mut out = fbx::Mesh { pieces: mesh.pieces, ..fbx::Mesh::default() };
    for triangle in (0..mesh.piece.len()).filter(|&t| taken[mesh.piece[t] as usize]) {
        let corners = triangle * 3..triangle * 3 + 3;
        out.positions.extend_from_slice(&mesh.positions[corners.clone()]);
        out.normals.extend_from_slice(&mesh.normals[corners.clone()]);
        out.uvs.extend_from_slice(&mesh.uvs[corners]);
        out.materials.push(mesh.materials[triangle]);
        out.piece.push(mesh.piece[triangle]);
    }
    out
}

impl PartDefinition {
    /// A point of the model, in its own units and axes, on the bone of the
    /// slot the part is in: metres to the right, back and up, as the
    /// install's slots' bones have them on a weapon. `unit` is how many
    /// centimetres one of the model's units is.
    pub(super) fn on_slot(&self, p: [f32; 3], unit: f32) -> [f32; 3] {
        let (a, k) = (self.at_slot, self.scale * unit * 0.01);
        [(p[0] - a[0]) * k, (p[2] - a[2]) * k, (p[1] - a[1]) * k]
    }

    /// What it is called where a short name is shown.
    pub fn shown_name(&self) -> &str {
        if self.short_name.is_empty() {
            &self.name
        } else {
            &self.short_name
        }
    }
}

/// A model file's scene as a file reads it: in its frame, turned, with y
/// up. Read once a run for each file, frame and turn: a part's model is
/// hung on every build that has it, and a file exported from a viewer can
/// be large (the StG 44's second is 99 MB).
pub(super) fn scene_of(root: &Path, model: &Model) -> Result<Arc<fbx::Scene>> {
    static SCENES: std::sync::OnceLock<std::sync::Mutex<HashMap<String, Arc<fbx::Scene>>>> = std::sync::OnceLock::new();
    let file = root.join(&model.file);
    let key = format!("{}|{:?}|{:?}", file.display(), model.frame, model.turn);
    let scenes = SCENES.get_or_init(Default::default);
    if let Some(scene) = scenes.lock().ok().and_then(|kept| kept.get(&key).cloned()) {
        return Ok(scene);
    }
    let data = std::fs::read(&file).with_context(|| format!("reading {}", file.display()))?;
    let mut scene = fbx::read(&data)?;
    ensure!(scene.up == (1, 1.0), "the model's up is not +y");
    if let Some(frame) = &model.frame {
        scene.seen_from(frame)?;
    }
    if let Some(turn) = model.turn {
        scene.turned(turn);
    }
    let scene = Arc::new(scene);
    if let Ok(mut kept) = scenes.lock() {
        kept.insert(key, scene.clone());
    }
    Ok(scene)
}
