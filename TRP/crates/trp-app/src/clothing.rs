//! The clothing chosen on the OVERALL tab (`CLOTHING`; the user, 2026-10-18:
//! "I want to add the clothing drop downs so we can change our appearance.
//! Usually it's limited to only USEC or BEAR, but we should mix them all.
//! They only load as we click on them"; inventory spec, "The OVERALL tab").
//!
//! Every suit of the install's customization list is offered, upper and
//! lower, whatever its side (`trp_data::suits`). **A suit is read when it is
//! first chosen** (lazy loading): its parts' bundles, meshes and pictures
//! made ready on another thread (`PreparedLooks`, as a weapon's rig is), put
//! into the engine's stores on the frame they come, and kept for a later
//! choice of the same suit. Then it is put on as the game puts a suit on
//! (`CustomizationSuite.SetClothingsToProfile`, the model drawn again): an
//! upper suit's body in place of the body's own top, on the body, the gear
//! page's and the OVERALL tab's model and the corpse, and its hands in
//! place of the hands the character's own eyes see; a lower suit's legs in
//! place of the legs. A torso of the newer kind has its three meshes, of
//! which the one for what is worn is drawn (`body_view::show_torso`). The
//! bones every suit hangs on are in the skeleton from the start
//! (`body_view::SUIT_BONES`). The choice is kept in the options' file for the
//! next run, which puts it on as it begins.

use std::collections::{HashMap, HashSet};

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::mesh::skinning::{SkinnedMesh, SkinnedMeshInverseBindposes};
use bevy::prelude::*;
use bevy::tasks::{block_on, futures_lite::future::poll_once, AsyncComputeTaskPool, Task};
use trp_data::arms::Torso;
use trp_data::suits::Suit;
use trp_data::unity::anim::path_hash;
use trp_data::unity::Assets as UnityAssets;

use crate::body_view::{BodyParts, BodyRig, LowerBody, OwnBody, SuitPiece, SuitRole, TorsoPiece, UpperBody, SEEN};
use crate::render_data::PreparedLooks;
use crate::weapon::{FirstPersonHands, WeaponRig};

/// Where the choice is kept in the options' file (`options_view`'s, which
/// keeps it as it is when it writes the rest).
pub const KEPT_AS: &str = "clothing";

/// The name a suit is waited for under on the graphics card, before its
/// place in the list (`weapon::UploadWatch`).
const SUIT_WAITED: &str = "a suit, the list's ";

pub struct ClothingPlugin;

impl Plugin for ClothingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, wear.before(crate::body_view::show_outfit));
    }
}

/// The suits, which are worn, which are asked for, and those read.
#[derive(Resource)]
pub struct Clothing {
    pub suits: Vec<Suit>,
    /// The upper suit and the lower worn, by their places in `suits`.
    pub worn: [Option<usize>; 2],
    /// Those asked for on the OVERALL tab, put on once read.
    pub wanted: [Option<usize>; 2],
    loaded: HashMap<usize, Loaded>,
    /// Those read whose meshes and pictures are all on the graphics card:
    /// a suit is put on only then, the one it takes the place of drawn
    /// until it is (`weapon::UploadWatch`).
    on_card: HashSet<usize>,
    /// Those that could not be read, not asked for again.
    pub failed: Vec<usize>,
    task: Option<(usize, Task<(Option<UnityAssets>, anyhow::Result<Prepared>)>)>,
    /// The thread's reader of the install, kept while suits are asked for.
    worker: Option<UnityAssets>,
    install: trp_data::Install,
    /// The options' file, where the choice is kept; none in a run nobody
    /// is at the keys of.
    file: Option<std::path::PathBuf>,
}

/// The place of a suit's role among `worn` and `wanted`.
fn slot(role: SuitRole) -> usize {
    match role {
        SuitRole::Upper => 0,
        SuitRole::Lower => 1,
    }
}

/// A part of a suit made ready away from the frame: its bones, its
/// meshes (each with its places among the looks' meshes, one for each
/// submesh, and its bind poses; a torso of the newer kind one for each
/// case of what is worn), its materials by submesh.
struct PreparedPart {
    bone_paths: Vec<String>,
    meshes: Vec<(Option<Torso>, Vec<usize>, Vec<Mat4>)>,
    materials: Vec<usize>,
}

struct Prepared {
    body: Option<PreparedPart>,
    hands: Option<PreparedPart>,
    feet: Option<PreparedPart>,
    looks: PreparedLooks,
}

/// A part of a suit in the engine's stores.
struct LoadedPart {
    bone_paths: Vec<String>,
    meshes: Vec<(Option<Torso>, Vec<Handle<Mesh>>, Handle<SkinnedMeshInverseBindposes>)>,
    materials: Vec<Handle<StandardMaterial>>,
}

struct Loaded {
    body: Option<LoadedPart>,
    hands: Option<LoadedPart>,
    feet: Option<LoadedPart>,
}

impl Clothing {
    /// The suits of the install, those worn found by the bundles the body
    /// is drawn in (the upper by its body's, the lower by its legs').
    pub fn new(install: trp_data::Install, suits: Vec<Suit>, body: &str, legs: &str, file: Option<std::path::PathBuf>) -> Self {
        let upper = suits.iter().position(|suit| suit.upper && suit.body.as_deref() == Some(body));
        let lower = suits.iter().position(|suit| !suit.upper && suit.feet.as_deref() == Some(legs));
        Self { suits, worn: [upper, lower], wanted: [upper, lower], loaded: HashMap::new(), on_card: HashSet::new(), failed: Vec::new(), task: None, worker: None, install, file }
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
    let all = std::fs::read_to_string(file).ok().and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok());
    let id = |name: &str| all.as_ref().and_then(|all| all.get(KEPT_AS)?.get(name)?.as_str().map(str::to_owned));
    [id("upper"), id("lower")]
}

/// Keeps the choice in the options' file, all else in it as it was.
fn keep(file: &std::path::Path, ids: [Option<&str>; 2]) {
    let mut all = std::fs::read_to_string(file).ok().and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok()).unwrap_or_else(|| serde_json::json!({}));
    if !all.is_object() {
        return;
    }
    all[KEPT_AS] = serde_json::json!({ "upper": ids[0], "lower": ids[1] });
    if let Some(folder) = file.parent() {
        let _ = std::fs::create_dir_all(folder);
    }
    if let Err(e) = std::fs::write(file, serde_json::to_string_pretty(&all).unwrap_or_default()) {
        warn!("the clothing chosen could not be kept: {e}");
    }
}

/// Reads a suit's parts and makes them ready, away from the frame.
fn prepare(unity: &mut UnityAssets, suit: &Suit) -> anyhow::Result<Prepared> {
    let mut looks = PreparedLooks::default();
    let mut part = |key: &Option<String>| -> anyhow::Result<Option<PreparedPart>> {
        let Some(key) = key else { return Ok(None) };
        let arms = trp_data::arms::load(unity, key)?;
        let cases: Vec<(Option<Torso>, trp_data::unity::ObjRef)> = match &arms.torso {
            Some(torso) => Torso::ALL.iter().filter_map(|&case| Some((Some(case), torso.mesh(case)?))).collect(),
            None => vec![(None, arms.mesh)],
        };
        let mut meshes = Vec::new();
        for (case, mesh) in cases {
            let (made, poses) = looks.mesh_with_poses(unity, mesh)?;
            meshes.push((case, made, poses));
        }
        let submeshes = meshes.iter().map(|(_, made, _)| made.len()).max().unwrap_or(0);
        let materials = (0..submeshes).map(|slot| looks.material(unity, arms.materials.get(slot).copied().flatten())).collect();
        Ok(Some(PreparedPart { bone_paths: arms.bone_paths, meshes, materials }))
    };
    let (body, hands, feet) = (part(&suit.body)?, part(&suit.hands)?, part(&suit.feet)?);
    Ok(Prepared { body, hands, feet, looks })
}

/// Into the engine's stores (the pictures by the settings of their levels,
/// as the body's own are).
fn into_stores(prepared: Prepared, meshes: &mut Assets<Mesh>, images: &mut Assets<Image>, materials: &mut Assets<StandardMaterial>, bindposes: &mut Assets<SkinnedMeshInverseBindposes>) -> Loaded {
    let Prepared { body, hands, feet, looks } = prepared;
    let (made_meshes, made_materials, _, _) = looks.add_sharing(meshes, images, materials, None);
    let mut part = |part: Option<PreparedPart>| {
        part.map(|part| LoadedPart {
            bone_paths: part.bone_paths,
            meshes: part.meshes.into_iter().map(|(case, at, poses)| (case, at.into_iter().map(|at| made_meshes[at].clone()).collect(), bindposes.add(SkinnedMeshInverseBindposes::from(poses)))).collect(),
            materials: part.materials.into_iter().map(|at| made_materials[at].clone()).collect(),
        })
    };
    Loaded { body: part(body), hands: part(hands), feet: part(feet) }
}

/// The entity a bone of a suit's part is tied to on the body: as the body's
/// own parts are tied (`BodyRig::drawn`), or the nearest bone above it the
/// body has; where no body is drawn, the hands' bones on the weapon's rig
/// by their paths from the collarbones.
fn joint_of(rig: Option<&BodyRig>, parts: &BodyParts, weapon: &WeaponRig, path: &str) -> Entity {
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

/// Reads the suits asked for, one at a time, and puts on each as it is had.
#[allow(clippy::too_many_arguments)]
fn wear(
    mut commands: Commands,
    clothing: Option<ResMut<Clothing>>,
    parts: Res<BodyParts>,
    rig: Option<Res<BodyRig>>,
    weapon: Option<Res<WeaponRig>>,
    mut figure: Option<ResMut<crate::figure_view::Figure>>,
    (corpse, dummies, uploads): (Option<Res<crate::figure_view::CorpseSkeleton>>, Option<Res<crate::dummy::DummySkeletons>>, Res<crate::weapon::UploadWatch>),
    (mut meshes, mut images, mut materials, mut bindposes): (ResMut<Assets<Mesh>>, ResMut<Assets<Image>>, ResMut<Assets<StandardMaterial>>, ResMut<Assets<SkinnedMeshInverseBindposes>>),
    pieces: Query<(Entity, &SuitPiece), crate::park::EvenParked>,
    twins: Query<(Entity, &crate::figure_view::Twin), crate::park::EvenParked>,
    (corpse_twins, dummy_twins): (Query<(Entity, &crate::death::CorpseTwin), crate::park::EvenParked>, Query<(Entity, &crate::dummy::DummyTwin), crate::park::EvenParked>),
    hands: Query<(Entity, &ChildOf), With<FirstPersonHands>>,
) {
    let (Some(mut clothing), Some(weapon)) = (clothing, weapon) else { return };
    let clothing = &mut *clothing;
    // A suit made ready: into the stores.
    let done = clothing.task.as_mut().and_then(|(_, task)| block_on(poll_once(task)));
    if let Some((worker, made)) = done {
        let (at, _) = clothing.task.take().expect("a task was polled");
        clothing.worker = worker;
        match made {
            Ok(prepared) => {
                let loaded = into_stores(prepared, &mut meshes, &mut images, &mut materials, &mut bindposes);
                info!("the suit {}: read", clothing.suits[at].name);
                // Waited for on the graphics card before it is put on: the
                // pictures go up a few a frame (`UPLOAD_BUDGET`), and a piece
                // whose pictures are not there is not drawn, which left the
                // body, its copies and the dummies without the suit for those
                // frames (the user's report of 2026-10-26).
                let parts = [&loaded.body, &loaded.hands, &loaded.feet];
                let shapes: Vec<AssetId<Mesh>> = parts.iter().copied().flatten().flat_map(|part| &part.meshes).flat_map(|(_, submeshes, _)| submeshes.iter().map(Handle::id)).collect();
                let pictures: Vec<AssetId<Image>> = parts
                    .iter()
                    .copied()
                    .flatten()
                    .flat_map(|part| &part.materials)
                    .filter_map(|material| materials.get(material))
                    .flat_map(|m| [&m.base_color_texture, &m.normal_map_texture, &m.metallic_roughness_texture, &m.occlusion_texture, &m.emissive_texture].into_iter().flatten().map(Handle::id).collect::<Vec<_>>())
                    .collect();
                uploads.wait(format!("{SUIT_WAITED}{at}"), pictures, shapes);
                clothing.loaded.insert(at, loaded);
            }
            Err(e) => {
                warn!("the suit {} could not be read, and is not worn: {e:#}", clothing.suits[at].name);
                clothing.failed.push(at);
                for role in [SuitRole::Upper, SuitRole::Lower] {
                    if clothing.wanted[slot(role)] == Some(at) {
                        clothing.wanted[slot(role)] = clothing.worn[slot(role)];
                    }
                }
            }
        }
    }
    for name in uploads.take(|name| name.starts_with(SUIT_WAITED)) {
        if let Some(at) = name.strip_prefix(SUIT_WAITED).and_then(|at| at.parse().ok()) {
            clothing.on_card.insert(at);
        }
    }
    // Each suit asked for, had and on the card: put on.
    let mut changed = false;
    for role in [SuitRole::Upper, SuitRole::Lower] {
        let Some(at) = clothing.wanted[slot(role)].filter(|at| clothing.worn[slot(role)] != Some(*at)) else { continue };
        let Some(loaded) = clothing.loaded.get(&at).filter(|_| clothing.on_card.contains(&at)) else { continue };
        // What it takes the place of goes, with its copies on the models
        // and the corpse.
        let gone: Vec<Entity> = pieces.iter().filter(|(_, piece)| piece.0 == role).map(|(entity, _)| entity).collect();
        for (entity, twin) in &twins {
            if gone.contains(&twin.0) {
                commands.entity(entity).despawn();
            }
        }
        for (entity, twin) in &corpse_twins {
            if gone.contains(&twin.0) {
                commands.entity(entity).despawn();
            }
        }
        for (entity, twin) in &dummy_twins {
            if gone.contains(&twin.original) {
                commands.entity(entity).despawn();
            }
        }
        for entity in gone {
            commands.entity(entity).despawn();
        }
        let part = match role {
            SuitRole::Upper => loaded.body.as_ref(),
            SuitRole::Lower => loaded.feet.as_ref(),
        };
        let root = rig.as_ref().map(|rig| rig.root());
        let mut made_meshes: Vec<Handle<Mesh>> = Vec::new();
        let mut made_materials: Vec<Handle<StandardMaterial>> = Vec::new();
        if let (Some(part), Some(root)) = (part, root) {
            let joints: Vec<Entity> = part.bone_paths.iter().map(|path| joint_of(rig.as_deref(), &parts, &weapon, path)).collect();
            for (case, submeshes, poses) in &part.meshes {
                for (slot, mesh) in submeshes.iter().enumerate() {
                    let Some(material) = part.materials.get(slot) else { continue };
                    let piece = commands
                        .spawn((
                            Name::new("Clothing, a suit chosen"),
                            Mesh3d(mesh.clone()),
                            MeshMaterial3d(material.clone()),
                            SkinnedMesh { inverse_bindposes: poses.clone(), joints: joints.clone() },
                            NoFrustumCulling,
                            RenderLayers::layer(SEEN),
                            OwnBody { top: role == SuitRole::Upper },
                            SuitPiece(role),
                            crate::park::Park,
                        ))
                        .id();
                    match role {
                        SuitRole::Upper => commands.entity(piece).insert(UpperBody),
                        SuitRole::Lower => commands.entity(piece).insert(LowerBody),
                    };
                    if let Some(case) = case {
                        commands.entity(piece).insert((TorsoPiece(*case), Visibility::Hidden));
                    }
                    commands.entity(root).add_child(piece);
                    if let Some(figure) = figure.as_ref() {
                        figure.skeleton.skinned(&mut commands, piece, (mesh, material), poses, &part.bone_paths, true);
                    }
                    if let Some(corpse) = corpse.as_ref() {
                        corpse.0.skinned(&mut commands, piece, (mesh, material), poses, &part.bone_paths, true);
                    }
                    for dummy in dummies.iter().flat_map(|dummies| &dummies.0) {
                        dummy.skinned(&mut commands, piece, (mesh, material), poses, &part.bone_paths, true);
                    }
                    made_meshes.push(mesh.clone());
                    made_materials.push(material.clone());
                }
            }
        }
        // An upper suit's hands, for the character's own eyes, in place of
        // those it had (where it names any).
        if let (SuitRole::Upper, Some(new)) = (role, loaded.hands.as_ref()) {
            let parent = hands.iter().next().map(|(_, child_of)| child_of.parent()).unwrap_or(weapon.root);
            for (entity, _) in &hands {
                commands.entity(entity).despawn();
            }
            let joints: Vec<Entity> = new.bone_paths.iter().map(|path| joint_of(rig.as_deref(), &parts, &weapon, path)).collect();
            for (_, submeshes, poses) in &new.meshes {
                for (slot, mesh) in submeshes.iter().enumerate() {
                    let Some(material) = new.materials.get(slot) else { continue };
                    let piece = commands
                        .spawn((
                            Name::new("Arms, a suit chosen"),
                            FirstPersonHands,
                            Mesh3d(mesh.clone()),
                            MeshMaterial3d(material.clone()),
                            SkinnedMesh { inverse_bindposes: poses.clone(), joints: joints.clone() },
                            NoFrustumCulling,
                            // As the game's hands: no shadow (`weapon::spawn_weapon`).
                            bevy::light::NotShadowCaster,
                        ))
                        .id();
                    commands.entity(parent).add_child(piece);
                }
            }
        }
        if let Some(figure) = figure.as_mut() {
            figure.wait_for(&made_meshes, &made_materials);
        }
        info!("the suit {} put on", clothing.suits[at].name);
        clothing.worn[slot(role)] = Some(at);
        changed = true;
    }
    if changed {
        if let Some(file) = &clothing.file {
            let id = |role: SuitRole| clothing.worn[slot(role)].map(|at| clothing.suits[at].id.as_str());
            keep(file, [id(SuitRole::Upper), id(SuitRole::Lower)]);
        }
    }
    // The next asked for that is not had: read, if nothing is being read.
    if clothing.task.is_none() {
        let next = [SuitRole::Upper, SuitRole::Lower]
            .into_iter()
            .filter_map(|role| clothing.wanted[slot(role)].filter(|at| clothing.worn[slot(role)] != Some(*at)))
            .find(|at| !clothing.loaded.contains_key(at) && !clothing.failed.contains(at));
        match next {
            Some(at) => {
                let (worker, install, suit) = (clothing.worker.take(), clothing.install.clone(), clothing.suits[at].clone());
                let task = AsyncComputeTaskPool::get().spawn(async move {
                    match worker.map_or_else(|| UnityAssets::new(install), Ok) {
                        Ok(mut unity) => {
                            let made = prepare(&mut unity, &suit);
                            (Some(unity), made)
                        }
                        Err(e) => (None, Err(e)),
                    }
                });
                clothing.task = Some((at, task));
            }
            // Nothing left to read: the thread's reader goes, away from the
            // frame.
            None => {
                if let Some(worker) = clothing.worker.take() {
                    AsyncComputeTaskPool::get().spawn(async move { drop(worker) }).detach();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{keep, kept, KEPT_AS};

    /// The choice kept in the options' file and read back, the rest of the
    /// file as it was.
    #[test]
    fn the_choice_is_kept_beside_the_options() {
        let file = std::env::temp_dir().join(format!("trp_clothing_{}.json", std::process::id()));
        std::fs::write(&file, r#"{ "graphics": { "vsync": true } }"#).expect("written");
        keep(&file, [Some("5d1f609d86f7744bca3f1689"), None]);
        assert_eq!(kept(&file), [Some("5d1f609d86f7744bca3f1689".to_owned()), None]);
        let all: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&file).expect("read")).expect("json");
        assert_eq!(all["graphics"]["vsync"], serde_json::Value::Bool(true));
        assert!(all.get(KEPT_AS).is_some());
        let _ = std::fs::remove_file(&file);
        assert_eq!(kept(&file), [None, None]);
    }
}
