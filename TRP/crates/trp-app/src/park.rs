//! What is hidden kept out of the engine's work each frame: an entity
//! marked `Park` and hidden by its own visibility is disabled with
//! everything under it (Bevy's `Disabled`), and enabled again the frame it
//! is shown. The performance pass of 0.80.0 (`docs/findings.md`, "What a
//! frame costs, again"): the body, its copy on the gear page and the
//! corpse each carry a mesh for every piece of clothing and every thing
//! the run could wear, some 2,500 hidden meshes that the engine's transform,
//! visibility and shadow systems went over every frame for nothing.
//!
//! What is drawn does not change. A disabled entity is in no view's list of
//! what to draw, as a hidden one is not; it is enabled in `PostUpdate`
//! before the engine propagates visibility and transforms, so a piece shown
//! in `Update` is drawn that frame, as before. As it comes back, everything
//! the engine takes from it is marked changed (`Touched`), as if it were
//! new: placed afresh, its mesh readied, its material and layers taken. The systems that keep such a piece's state (its
//! visibility, its material, its layers, a hinge, the copies that follow
//! it) see it while disabled (`Allow<Disabled>` in their queries), so it
//! comes back as it would have been.

use bevy::camera::visibility::VisibilitySystems;
use bevy::ecs::entity_disabling::Disabled;
use bevy::prelude::*;
use bevy::transform::TransformSystems;

/// A query's filter that lets it see parked entities as well: for the
/// systems that keep what is parked as it would be.
pub type EvenParked = Allow<Disabled>;

/// Hidden by its own visibility, this entity and everything under it are
/// disabled.
#[derive(Component, Clone, Copy, Default)]
pub struct Park;

pub struct ParkPlugin;

impl Plugin for ParkPlugin {
    fn build(&self, app: &mut App) {
        // `TRP_NO_PARK` in the environment leaves everything as before, to
        // set the two beside each other.
        if std::env::var_os("TRP_NO_PARK").is_some() {
            info!("park: off (TRP_NO_PARK)");
            return;
        }
        app.add_systems(PostUpdate, park.before(VisibilitySystems::VisibilityPropagate).before(TransformSystems::Propagate));
    }
}

/// What the engine takes from an entity when it changes, and so must be
/// marked changed as the entity comes back: whatever changed while it was
/// disabled was seen by none of the engine's systems (a material swapped,
/// its layers), and a mesh disabled before the engine first saw it was
/// never readied for drawing.
type Touched = (
    Option<&'static mut Transform>,
    Option<&'static mut Visibility>,
    Option<&'static mut Mesh3d>,
    Option<&'static mut MeshMaterial3d<StandardMaterial>>,
    Option<&'static mut MeshMaterial3d<crate::shadow_only::ShadowOnlyMaterial>>,
    Option<&'static mut bevy::camera::visibility::RenderLayers>,
    Option<&'static mut bevy::mesh::skinning::SkinnedMesh>,
);

fn touch((transform, visibility, mesh, standard, shadow_only, layers, skin): bevy::ecs::query::QueryItem<Touched>) {
    if let Some(mut c) = transform {
        c.set_changed();
    }
    if let Some(mut c) = visibility {
        c.set_changed();
    }
    if let Some(mut c) = mesh {
        c.set_changed();
    }
    if let Some(mut c) = standard {
        c.set_changed();
    }
    if let Some(mut c) = shadow_only {
        c.set_changed();
    }
    if let Some(mut c) = layers {
        c.set_changed();
    }
    if let Some(mut c) = skin {
        c.set_changed();
    }
}

/// Whether something above `entity` (not itself) parks it.
fn parked_above(entity: Entity, parents: &Query<&ChildOf, Allow<Disabled>>, parks: &Query<&Visibility, (With<Park>, Allow<Disabled>)>) -> bool {
    let mut at = entity;
    while let Ok(parent) = parents.get(at) {
        at = parent.parent();
        if parks.get(at).is_ok_and(|visibility| *visibility == Visibility::Hidden) {
            return true;
        }
    }
    false
}

/// Each `Park` whose visibility changed, or that is new, and each entity
/// newly hung under something: its tree disabled or enabled as what parks
/// it now says.
#[allow(clippy::type_complexity)]
fn park(
    mut commands: Commands,
    joined: Query<Entity, (Added<ChildOf>, Allow<Disabled>)>,
    parents: Query<&ChildOf, Allow<Disabled>>,
    nodes: Query<(Option<&Children>, Has<Disabled>), Allow<Disabled>>,
    // What changed and what parks are read, then what comes back is
    // touched: the three in turn.
    mut set: ParamSet<(
        Query<Entity, (With<Park>, Or<(Changed<Visibility>, Added<Park>)>, Allow<Disabled>)>,
        Query<&Visibility, (With<Park>, Allow<Disabled>)>,
        Query<Touched, Allow<Disabled>>,
    )>,
) {
    let starts: Vec<Entity> = set.p0().iter().chain(joined.iter()).collect();
    if starts.is_empty() {
        return;
    }
    let mut back: Vec<Entity> = Vec::new();
    {
        let parks = set.p1();
        let mut stack: Vec<(Entity, bool)> = Vec::new();
        for entity in starts {
            stack.push((entity, parked_above(entity, &parents, &parks)));
            while let Some((at, above)) = stack.pop() {
                let parked = above || parks.get(at).is_ok_and(|visibility| *visibility == Visibility::Hidden);
                let Ok((children, disabled)) = nodes.get(at) else { continue };
                if parked && !disabled {
                    commands.entity(at).try_insert(Disabled);
                } else if !parked && disabled {
                    commands.entity(at).try_remove::<Disabled>();
                    back.push(at);
                }
                for &child in children.into_iter().flatten() {
                    stack.push((child, parked));
                }
            }
        }
    }
    let mut touched = set.p2();
    for entity in back {
        if let Ok(item) = touched.get_mut(entity) {
            touch(item);
        }
    }
}
