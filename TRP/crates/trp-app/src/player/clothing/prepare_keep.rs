//! Clothing's parts prepared and loaded, the choice kept, and the joints found.

use super::*;

/// The place of a suit's role among `worn` and `wanted`.
pub(super) fn slot(role: SuitRole) -> usize {
    match role {
        SuitRole::Upper => 0,
        SuitRole::Lower => 1,
    }
}

/// A part of a suit made ready away from the frame: its bones, its
/// meshes (each with its places among the looks' meshes, one for each
/// submesh, and its bind poses; a torso of the newer kind one for each
/// case of what is worn), its materials by submesh.
pub(super) struct PreparedPart {
    pub(super) bone_paths: Vec<String>,
    pub(super) meshes: Vec<(Option<Torso>, Vec<usize>, Vec<Mat4>)>,
    /// A body's hooded form (`trp_data::dress::hood`), as `meshes`, read
    /// with `fitting`.
    pub(super) hood: Option<Vec<(Option<Torso>, Vec<usize>, Vec<Mat4>)>>,
    pub(super) materials: Vec<usize>,
    /// Beside `materials`: a watch's glass, not drawn on the hands
    /// (`trp_data::arms::is_glass`).
    pub(super) glass: Vec<bool>,
    /// Its levels of detail, where read (`lod_view`): its group placed under
    /// the player's mesh transform, and its levels.
    pub(super) lod: Option<(std::sync::Arc<crate::assets::lod_view::LodSpec>, u8)>,
}

pub(super) struct Prepared {
    /// The bits its body's or legs' changer adds (`Clothing::bits`).
    pub(super) bits: u8,
    /// Its legs' holster (`Clothing::holster`).
    pub(super) holster: Option<trp_data::character::arms::LegsHolster>,
    /// The body's and the legs' levels of detail, the fullest first (it
    /// alone, where they are not read).
    pub(super) body: Vec<PreparedPart>,
    /// The hands, and what their bundle draws beside them (a watch's case
    /// and glass: `trp_data::arms::load`), each a skin of its own.
    pub(super) hands: Vec<PreparedPart>,
    pub(super) feet: Vec<PreparedPart>,
    looks: PreparedLooks,
}

/// A part of a suit in the engine's stores.
pub(super) struct LoadedPart {
    pub(super) bone_paths: Vec<String>,
    pub(super) meshes: Vec<(Option<Torso>, Vec<Handle<Mesh>>, Handle<SkinnedMeshInverseBindposes>)>,
    pub(super) hood: Option<Vec<(Option<Torso>, Vec<Handle<Mesh>>, Handle<SkinnedMeshInverseBindposes>)>>,
    pub(super) materials: Vec<Handle<StandardMaterial>>,
    pub(super) glass: Vec<bool>,
    pub(super) lod: Option<crate::assets::lod_view::LodPiece>,
}

pub(super) struct Loaded {
    pub(super) bits: u8,
    pub(super) holster: Option<trp_data::character::arms::LegsHolster>,
    pub(super) body: Vec<LoadedPart>,
    pub(super) hands: Vec<LoadedPart>,
    pub(super) feet: Vec<LoadedPart>,
}

impl Clothing {
    /// The suits of the install, those worn found by the bundles the body
    /// is drawn in (the upper by its body's, the lower by its legs').
    pub fn new(install: trp_data::Install, suits: Vec<Suit>, body: &str, legs: &str, file: Option<std::path::PathBuf>) -> Self {
        let upper = suits.iter().position(|suit| suit.upper && suit.body.as_deref() == Some(body));
        let lower = suits.iter().position(|suit| !suit.upper && suit.feet.as_deref() == Some(legs));
        Self { suits, worn: [upper, lower], wanted: [upper, lower], hands: None, bits: [None; 2], holster: None, fitting: false, loaded: HashMap::new(), on_card: HashSet::new(), failed: Vec::new(), task: None, worker: None, install, file }
    }

    /// With the options' `HEAD FITTING` as the run began: on, a suit's
    /// changer is read with it.
    pub fn fitted(self, fitting: bool) -> Self {
        Self { fitting, ..self }
    }

    /// The suits of a role, by their places in `suits`, in the order they
    /// are listed.
    pub fn of(&self, role: SuitRole) -> Vec<usize> {
        (0..self.suits.len()).filter(|&at| self.suits[at].upper == (role == SuitRole::Upper)).collect()
    }

    /// Asks for a suit to be worn.
    pub fn choose(&mut self, at: usize) {
        let Some(suit) = self.suits.get(at) else { return };
        let role = if suit.upper { SuitRole::Upper } else { SuitRole::Lower };
        self.wanted[slot(role)] = Some(at);
    }

    /// Whether a suit asked for is still being read.
    pub fn reading(&self) -> bool {
        self.task.is_some()
    }
}

/// The choice kept in the options' file: the upper suit's id and the lower's.
pub fn kept(file: &std::path::Path) -> [Option<String>; 2] {
    let all = crate::platform::options_store::read(file);
    let id = |name: &str| all.as_ref().and_then(|all| all.get(KEPT_AS)?.get(name)?.as_str().map(str::to_owned));
    [id("upper"), id("lower")]
}

/// Keeps the choice in the options' file, all else in it as it was.
pub(super) fn keep(file: &std::path::Path, ids: [Option<&str>; 2]) {
    crate::platform::options_store::keep(file, KEPT_AS, Some(serde_json::json!({ "upper": ids[0], "lower": ids[1] })), "the clothing chosen");
}

/// Reads a suit's parts and makes them ready, away from the frame; with
/// `fitting`, the bits its body's or legs' changer adds and its body's
/// hooded form; and its legs' holster.
pub(super) fn prepare(unity: &mut UnityAssets, suit: &Suit, fitting: bool) -> anyhow::Result<Prepared> {
    // (Its cubes for the OVERALL figure's own surface: `figure_surface`.)
    let mut looks = PreparedLooks::default().with_figure_cubes();
    let bits = if fitting { [&suit.body, &suit.feet].into_iter().flatten().map(|key| trp_data::character::dress::part_clipping(unity, key).unwrap_or(0)).fold(0, |all, bits| all | bits) } else { 0 };
    // The levels of detail, where read (`lod_view`): by the player's mesh
    // transform, one spec a group.
    let mesh_transform = if crate::assets::lod_view::on() { trp_data::character::player::mesh_transform(unity).ok() } else { None };
    let mut specs: Vec<(usize, std::sync::Arc<crate::assets::lod_view::LodSpec>)> = Vec::new();
    let mut spec_of = |arms: &trp_data::character::arms::Arms| -> Option<(std::sync::Arc<crate::assets::lod_view::LodSpec>, u8)> {
        let (lod, (place, turn)) = arms.lod.as_ref().zip(mesh_transform)?;
        let key = std::sync::Arc::as_ptr(&lod.group) as usize;
        let spec = match specs.iter().find(|(made, _)| *made == key) {
            Some((_, spec)) => spec.clone(),
            None => {
                let spec = crate::assets::lod_view::LodSpec::through(&lod.group, place, turn);
                specs.push((key, spec.clone()));
                spec
            }
        };
        Some((spec, lod.levels))
    };
    // A part's skins: its own, then those its bundle draws beside it (none
    // on a body or legs, whose list is levels of detail: with `levels`,
    // each level, the fullest first).
    let mut skins = |key: &Option<String>, levels: bool| -> anyhow::Result<Vec<PreparedPart>> {
        let Some(key) = key else { return Ok(Vec::new()) };
        let mut all = if levels && crate::assets::lod_view::on() { trp_data::character::arms::load_levels(unity, key)? } else { vec![trp_data::character::arms::load(unity, key)?] };
        let beside = std::mem::take(&mut all[0].beside);
        let mut made = Vec::new();
        for arms in all.into_iter().chain(beside) {
            let cases: Vec<(Option<Torso>, trp_data::unity::ObjRef)> = match &arms.torso {
                Some(torso) => Torso::ALL.iter().filter_map(|&case| Some((Some(case), torso.mesh(case)?))).collect(),
                None => vec![(None, arms.mesh)],
            };
            let mut meshes = Vec::new();
            for (case, mesh) in cases {
                let (made, poses) = looks.mesh_with_poses(unity, mesh, false)?;
                meshes.push((case, made, poses));
            }
            let submeshes = meshes.iter().map(|(_, made, _)| made.len()).max().unwrap_or(0);
            let materials = (0..submeshes).map(|slot| looks.material(unity, arms.materials.get(slot).copied().flatten())).collect();
            let lod = spec_of(&arms);
            made.push(PreparedPart { bone_paths: arms.bone_paths, meshes, materials, glass: arms.glass, hood: None, lod });
        }
        Ok(made)
    };
    let (mut body, hands, feet) = (skins(&suit.body, true)?, skins(&suit.hands, false)?, skins(&suit.feet, true)?);
    // Its body's hooded form, each case's mesh beside the bare one's: the
    // fullest's, and each coarser level's by the levels it shares.
    if let (Some(key), true) = (&suit.body, fitting) {
        let fullest_hood = trp_data::character::dress::hood(unity, key).unwrap_or_else(|e| {
            warn!("the suit {}: its hood could not be read: {e:#}", suit.name);
            None
        });
        let hooded = if fullest_hood.is_some() && crate::assets::lod_view::on() { trp_data::character::dress::hood_levels(unity, key).unwrap_or_default() } else { Vec::new() };
        for (at, level) in body.iter_mut().enumerate() {
            let levels = level.lod.as_ref().map(|(_, levels)| *levels);
            let hood = match at {
                0 => fullest_hood.clone(),
                _ => hooded.iter().find(|form| form.lod.as_ref().map(|lod| lod.levels) == levels && levels.is_some()).cloned(),
            };
            let Some(hood) = hood else { continue };
            let cases: Vec<(Option<Torso>, trp_data::unity::ObjRef)> = match &hood.torso {
                Some(torso) => Torso::ALL.iter().filter_map(|&case| Some((Some(case), torso.mesh(case)?))).collect(),
                None => vec![(None, hood.mesh)],
            };
            let mut meshes = Vec::new();
            for (case, mesh) in cases {
                let (made, poses) = looks.mesh_with_poses(unity, mesh, false)?;
                meshes.push((case, made, poses));
            }
            level.hood = Some(meshes);
        }
    }
    let holster = match &suit.feet {
        Some(key) => trp_data::character::arms::legs_holster(unity, key).unwrap_or_else(|e| {
            warn!("the suit {}: its legs' holster could not be read; no pistol is hung on them: {e:#}", suit.name);
            None
        }),
        None => None,
    };
    Ok(Prepared { bits, holster, body, hands, feet, looks })
}

/// Into the engine's stores (the pictures by the settings of their levels,
/// as the body's own are).
pub(super) fn into_stores(prepared: Prepared, meshes: &mut Assets<Mesh>, images: &mut Assets<Image>, materials: &mut Assets<StandardMaterial>, bindposes: &mut Assets<SkinnedMeshInverseBindposes>) -> Loaded {
    let Prepared { bits, holster, body, hands, feet, looks } = prepared;
    let (made_meshes, made_materials, _, _) = looks.add_sharing(meshes, images, materials, None);
    let mut part = |part: PreparedPart| LoadedPart {
        bone_paths: part.bone_paths,
        meshes: part.meshes.into_iter().map(|(case, at, poses)| (case, at.into_iter().map(|at| made_meshes[at].clone()).collect(), bindposes.add(SkinnedMeshInverseBindposes::from(poses)))).collect(),
        hood: part.hood.map(|hood| hood.into_iter().map(|(case, at, poses)| (case, at.into_iter().map(|at| made_meshes[at].clone()).collect(), bindposes.add(SkinnedMeshInverseBindposes::from(poses)))).collect()),
        materials: part.materials.into_iter().map(|at| made_materials[at].clone()).collect(),
        glass: part.glass,
        lod: part.lod.map(|(group, levels)| crate::assets::lod_view::LodPiece { group, anchor: None, levels }),
    };
    Loaded { bits, holster, body: body.into_iter().map(&mut part).collect(), hands: hands.into_iter().map(&mut part).collect(), feet: feet.into_iter().map(&mut part).collect() }
}

/// The entity a bone of a suit's part is tied to on the body: as the body's
/// own parts are tied (`BodyRig::drawn`), or the nearest bone above it the
/// body has; where no body is drawn, the hands' bones on the weapon's rig
/// by their paths from the collarbones.
pub(crate) fn joint_of(rig: Option<&BodyRig>, parts: &BodyParts, weapon: &WeaponRig, path: &str) -> Entity {
    if let Some(rig) = rig {
        let mut at = path;
        loop {
            if let Some(entity) = rig.drawn(parts, weapon, at) {
                return entity;
            }
            match at.rfind('/') {
                Some(cut) => at = &at[..cut],
                None => return rig.root(),
            }
        }
    }
    const TORSO: &str = "Base HumanRibcage/";
    let mut on_rig = path.find(TORSO).map_or(path, |at| &path[at + TORSO.len()..]);
    loop {
        if let Some(entity) = weapon.by_hash.get(&path_hash(on_rig)) {
            return *entity;
        }
        match on_rig.rfind('/') {
            Some(cut) => on_rig = &on_rig[..cut],
            None => return weapon.root,
        }
    }
}
