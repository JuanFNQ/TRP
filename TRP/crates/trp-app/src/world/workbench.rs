//! The benches on the range, two tables of the hideout's level 3 bench.
//! The gun bench's with its vise and the armor bench's plain one (`range::BenchKind`), read from
//! the install at once on another thread as the run begins
//! (`trp_data::scene_props`) and put into the world where the range stands
//! each (`range::Workbench`) once ready; their solids are the range's. A
//! character standing in a bench's zone before it has it in sight, and the
//! interaction menu offers its lines: the gun bench's open the edit build
//! screen for the weapon in hand, the armor bench's for the helmet or the
//! armour worn (`edit_build`).

use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task};

use crate::assets::render_data::PreparedLooks;

/// The benches' scene, and their object there (whose axes the range places
/// each by); what of it each draws, `BenchKind::drawn`.
const SCENE: &str = "bunker_2.unity";
const BENCH: &str = "10_workbench/level3";
/// Left out of it: decals (`StaticDeferredDecal`, projected on the
/// hideout's surfaces, which are not here).
const SKIP_SCRIPTS: [&str; 1] = ["StaticDeferredDecal"];

/// The bench's model: being read, then in the world.
#[derive(Resource, Default)]
pub struct WorkbenchModel {
    task: Option<Task<anyhow::Result<Prepared>>>,
    began: Option<std::time::Instant>,
    pub spawned: bool,
}

/// The benches read and made ready away from the frame: the objects (their
/// places under their parents), what each draws (a mesh and a material, by
/// their places in `looks`), which bench each object is of (or on the way
/// to: `of[kind][node]`), and what could not be read.
struct Prepared {
    nodes: Vec<(Option<usize>, Transform, bool, String)>,
    drawn: Vec<Vec<(usize, usize)>>,
    of: Vec<(crate::world::range::BenchKind, Vec<bool>, Vec<bool>)>,
    looks: PreparedLooks,
    skipped: Vec<String>,
    counts: [usize; 3],
    /// With the world's things' levels of detail on (`lod_view::world_on`):
    /// each node's group and levels, and the groups by the node each is on.
    lod: Vec<Option<trp_data::unity::extract::NodeLod>>,
    lods: Vec<(usize, std::sync::Arc<crate::assets::lod_view::LodSpec>)>,
}

/// What the edit build screen is asked to open for, by a bench's line
/// (`edit_build`): the weapon in hand (MODIFY WEAPON), the helmet worn
/// (MODIFY HELMET) or the armour worn (MODIFY ARMOR).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BuildFor {
    Weapon,
    Helmet,
    Armor,
}

/// A bench's line for the edit build screen was chosen.
#[derive(Resource, Default)]
pub struct WorkbenchAsked(pub Option<BuildFor>);

fn prepare(install: trp_data::Install) -> anyhow::Result<Prepared> {
    use crate::world::range::BenchKind;
    let kinds: Vec<BenchKind> = BenchKind::ALL.into_iter().filter(|kind| kind.on()).collect();
    let keep: Vec<&str> = kinds.iter().flat_map(|kind| kind.drawn().0.iter().copied()).collect();
    let left_out: Vec<&str> = kinds.iter().flat_map(|kind| kind.drawn().1.iter().copied()).collect();
    let props = trp_data::level::scene_props::read(&install, SCENE, BENCH, &SKIP_SCRIPTS, &left_out, &keep, crate::assets::lod_view::world_on())?;
    // Each object's path under the one read, and so the bench whose kept
    // objects it is under; the objects on the way to those, drawing
    // nothing, made for each.
    let mut paths: Vec<String> = Vec::with_capacity(props.nodes.len());
    for node in &props.nodes {
        let path = match node.parent {
            None => String::new(),
            Some(parent) if paths[parent].is_empty() => node.name.clone(),
            Some(parent) => format!("{}/{}", paths[parent], node.name),
        };
        paths.push(path);
    }
    let of = kinds
        .iter()
        .map(|&kind| {
            let own: Vec<bool> = paths.iter().map(|path| kind.drawn().0.iter().any(|kept| path == kept || path.starts_with(&format!("{kept}/")))).collect();
            let mut made = own.clone();
            // (A group's object is made too, where it places a drawn
            // object's levels of detail.)
            for node in (0..props.nodes.len()).filter(|&node| own[node]) {
                if let Some(group) = props.nodes[node].lod.and_then(|lod| props.lods.get(lod.group)) {
                    made[group.node] = true;
                    let mut at = props.nodes[group.node].parent;
                    while let Some(parent) = at {
                        made[parent] = true;
                        at = props.nodes[parent].parent;
                    }
                }
                let mut at = props.nodes[node].parent;
                while let Some(parent) = at {
                    made[parent] = true;
                    at = props.nodes[parent].parent;
                }
            }
            (kind, own, made)
        })
        .collect();
    let mut looks = PreparedLooks::default();
    let mut made = std::collections::HashMap::new();
    let mut materials = std::collections::HashMap::new();
    let mut drawn = Vec::with_capacity(props.nodes.len());
    for node in &props.nodes {
        let mut own = Vec::new();
        for &(mesh, material) in &node.drawn {
            let material = *materials.entry(material).or_insert_with(|| looks.prop_material(&props.materials[material], &props.pictures, &mut made));
            for part in looks.custom_mesh(&props.meshes[mesh]) {
                own.push((part, material));
            }
        }
        drawn.push(own);
    }
    let nodes = props.nodes.iter().map(|node| (node.parent, crate::platform::convert::transform(node.translation, node.rotation, node.scale), node.in_root, node.name.clone())).collect();
    let counts = [props.meshes.len(), props.materials.len(), props.pictures.len()];
    let lod = props.nodes.iter().map(|node| node.lod).collect();
    let lods = crate::assets::lod_view::LodSpec::of_props(&props.lods);
    Ok(Prepared { nodes, drawn, of, looks, skipped: props.skipped, counts, lod, lods })
}

/// As the run begins, with a bench on the range: they are asked of another
/// thread.
pub fn read_workbench(mut model: ResMut<WorkbenchModel>, range: Res<crate::world::range::Range>, game: Res<crate::GameData>) {
    if model.task.is_some() || model.spawned || range.benches.is_empty() {
        return;
    }
    let install = game.unity.install.clone();
    model.began = Some(std::time::Instant::now());
    model.task = Some(AsyncComputeTaskPool::get().spawn(async move { prepare(install) }));
}

/// The benches read: each put into the world where the range stands it,
/// every object of it under its top as it was under the level's, what it
/// draws on it (a mesh taken out of a joined one, on the top).
pub fn place_workbench(
    mut commands: Commands,
    mut model: ResMut<WorkbenchModel>,
    range: Res<crate::world::range::Range>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some(task) = model.task.as_mut() else { return };
    let Some(done) = bevy::tasks::block_on(bevy::tasks::futures_lite::future::poll_once(task)) else { return };
    model.task = None;
    model.spawned = true;
    let took = model.began.map_or(0.0, |began| began.elapsed().as_secs_f32());
    let prepared = match done {
        Ok(prepared) => prepared,
        Err(e) => {
            warn!("the benches could not be read: {e:#}");
            return;
        }
    };
    let Prepared { nodes, drawn, of, looks, skipped, counts, lod, lods } = prepared;
    for skipped in &skipped {
        info!("the benches: not drawn, {skipped}");
    }
    let (mesh_handles, material_handles, _) = looks.add_with_pictures(&mut meshes, &mut images, &mut materials);
    for (kind, own, made) in &of {
        let Some(bench) = range.benches.iter().find(|bench| bench.kind == *kind) else { continue };
        let top = commands.spawn((Name::new(kind.name()), bench.place, Visibility::Inherited)).id();
        let mut entities: Vec<Option<Entity>> = Vec::with_capacity(nodes.len());
        for (index, (parent, place, _, name)) in nodes.iter().enumerate() {
            if !made[index] {
                entities.push(None);
                continue;
            }
            let under = parent.and_then(|parent| entities.get(parent).copied().flatten()).unwrap_or(top);
            let at = if index == 0 { Transform::IDENTITY } else { *place };
            entities.push(Some(commands.spawn((Name::new(name.clone()), at, Visibility::Inherited, ChildOf(under))).id()));
        }
        for (index, drawn) in drawn.iter().enumerate().filter(|(index, _)| own[*index]) {
            let Some(entity) = entities[index] else { continue };
            let on = if nodes[index].2 { top } else { entity };
            // Its levels of detail, placed by its group's object (`lod_view`).
            let tag = crate::assets::lod_view::LodSpec::prop_piece(lod[index], &lods, &|node| entities.get(node).copied().flatten());
            for &(mesh, material) in drawn {
                let piece = commands.spawn((Mesh3d(mesh_handles[mesh].clone()), MeshMaterial3d(material_handles[material].clone()), Transform::IDENTITY, ChildOf(on))).id();
                if let Some(tag) = &tag {
                    commands.entity(piece).insert(tag.clone());
                }
            }
        }
    }
    info!(
        "the benches: {} objects, {} meshes, {} materials, {} pictures, read in {took:.2} s away from the frame; {}",
        nodes.len(),
        counts[0],
        counts[1],
        counts[2],
        crate::assets::lod_view::LodSpec::props_said(&lods, &lod)
    );
}
