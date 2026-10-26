//! The workbench on the range (the user, 2026-10-20; workbench spec): the
//! hideout's level 3 bench's main metal table alone (the user: "just the
//! main metal table without all the extra stuff"), read from the install on
//! another thread as the run begins (`trp_data::scene_props`) and put into
//! the world where the range stands it (`range::Workbench`) once it is
//! ready; its solid is the range's. A character standing in one of
//! the game's two triggers before it has it in sight, and the interaction
//! menu offers the line that opens the edit build screen for the weapon in
//! hand (`edit_build`).

use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task};

use crate::render_data::PreparedLooks;

/// The bench's scene, its object there (whose axes the range places it
/// by), and of it the main metal table alone (the long one against the
/// hideout's wall; the user's).
const SCENE: &str = "bunker_2.unity";
const BENCH: &str = "10_workbench/level3";
const TABLE: [&str; 1] = ["highlight_transform/metal_table (2)"];
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

/// The bench read and made ready away from the frame: its objects (their
/// places under their parents), what each draws (a mesh and a material, by
/// their places in `looks`), and what could not be read.
struct Prepared {
    nodes: Vec<(Option<usize>, Transform, bool, String)>,
    drawn: Vec<Vec<(usize, usize)>>,
    looks: PreparedLooks,
    skipped: Vec<String>,
    counts: [usize; 3],
}

/// The interaction menu's line for the bench was chosen: the edit build
/// screen is to open (`edit_build`).
#[derive(Resource, Default)]
pub struct WorkbenchAsked(pub bool);

fn prepare(install: trp_data::Install) -> anyhow::Result<Prepared> {
    let props = trp_data::scene_props::read(&install, SCENE, BENCH, &SKIP_SCRIPTS, &[], &TABLE)?;
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
    let nodes = props.nodes.iter().map(|node| (node.parent, crate::convert::transform(node.translation, node.rotation, node.scale), node.in_root, node.name.clone())).collect();
    let counts = [props.meshes.len(), props.materials.len(), props.pictures.len()];
    Ok(Prepared { nodes, drawn, looks, skipped: props.skipped, counts })
}

/// As the run begins, with the bench on the range: it is asked of another
/// thread.
pub fn read_workbench(mut model: ResMut<WorkbenchModel>, range: Res<crate::range::Range>, game: Res<crate::GameData>) {
    if model.task.is_some() || model.spawned || range.workbench.is_none() {
        return;
    }
    let install = game.unity.install.clone();
    model.began = Some(std::time::Instant::now());
    model.task = Some(AsyncComputeTaskPool::get().spawn(async move { prepare(install) }));
}

/// The bench read: put into the world where the range stands it, every
/// object under its top as it was under the bench's, what it draws on it
/// (a mesh taken out of a joined one, on the top).
pub fn place_workbench(
    mut commands: Commands,
    mut model: ResMut<WorkbenchModel>,
    range: Res<crate::range::Range>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some(task) = model.task.as_mut() else { return };
    let Some(done) = bevy::tasks::block_on(bevy::tasks::futures_lite::future::poll_once(task)) else { return };
    model.task = None;
    model.spawned = true;
    let took = model.began.map_or(0.0, |began| began.elapsed().as_secs_f32());
    let (prepared, Some(bench)) = (match done {
        Ok(prepared) => prepared,
        Err(e) => {
            warn!("the workbench could not be read: {e:#}");
            return;
        }
    }, range.workbench) else {
        return;
    };
    let Prepared { nodes, drawn, looks, skipped, counts } = prepared;
    for skipped in &skipped {
        info!("the workbench: not drawn, {skipped}");
    }
    let (mesh_handles, material_handles, _) = looks.add_with_pictures(&mut meshes, &mut images, &mut materials);
    let top = commands.spawn((Name::new("workbench"), bench.place, Visibility::Inherited)).id();
    let mut entities: Vec<Entity> = Vec::with_capacity(nodes.len());
    for (index, (parent, place, _, name)) in nodes.iter().enumerate() {
        let under = parent.and_then(|parent| entities.get(parent).copied()).unwrap_or(top);
        let at = if index == 0 { Transform::IDENTITY } else { *place };
        entities.push(commands.spawn((Name::new(name.clone()), at, Visibility::Inherited, ChildOf(under))).id());
    }
    for (index, own) in drawn.iter().enumerate() {
        let on = if nodes[index].2 { top } else { entities[index] };
        for &(mesh, material) in own {
            commands.spawn((Mesh3d(mesh_handles[mesh].clone()), MeshMaterial3d(material_handles[material].clone()), Transform::IDENTITY, ChildOf(on)));
        }
    }
    info!(
        "the workbench: {} objects, {} meshes, {} materials, {} pictures, read in {took:.2} s away from the frame",
        nodes.len(),
        counts[0],
        counts[1],
        counts[2]
    );
}
