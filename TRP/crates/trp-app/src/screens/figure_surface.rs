//! The OVERALL tab's own figure's surface, lit as the game's menu figure is.
//! It is the `OVERALL FIGURE` switch's, by the inventory spec, "The character's surface".
//!
//! The game draws its menu figure with each material's own shader, the
//! characters' and their gear's `p0/Reflective/Bumped Specular SMap(_Decal)`,
//! deferred, under the view's eight lights: its G-buffer's colour is the
//! texel times `_Color` times kd, kd = `_DefVals`.x + `_DefVals`.y·½(1 −
//! n·v)², so a face toward the camera takes only `_DefVals`.x of its colour
//! (the vest 0.55, the head 0.8); its specular the texel's alpha times
//! `_Glossness`, ks and `_SpecColor`; its emission the material's cube at
//! the reflection (inspect-window spec, "The surface in the preview"). The
//! figure shared the body's standard materials, so it showed its colour
//! whole and Bevy's gloss.
//!
//! Here: each install material's figures are noted as it is made (`note`:
//! `render_data`'s `Converted::material` and `PreparedLooks`, with its cube
//! where it names one), and each piece on the OVERALL figure's layer drawn
//! with a noted material is drawn with a twin of it (`swap`), made once and
//! shared: the game's surface in the preview's way, lit by the view's own
//! lights through the game's light rule (`game_surface::GameSurface::
//! lit_by_view`, `game_shading.wgsl`'s `trp_game_light`). Its pictures follow
//! the standard material's as the streaming changes their levels
//! (`follow`); `SurfaceDrawn` keeps the standard material, whose pictures
//! the streaming weighs by this piece too.
//!
//! Only with `OVERALL FIGURE`, once its camera is lit by the game's eight
//! (`set_lit`) and Bevy's PBR functions are patched (`shading::patched`);
//! the dithered glass keeps its own twin (`glass_view`), the sights' lenses
//! theirs, a piece wished to be its shadow alone (`shadow_only`) its
//! standard material. The world's bodies, the gear page's figure and the
//! inspect window are not touched. `PROVISIONAL`: every install material
//! but the glass and the lenses is drawn by the `p0` shader's sums (the
//! shaders of the others the figure wears are not read), as the inspect
//! window's are.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{LazyLock, Mutex};

use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;

use crate::rendering::game_surface::{GameSurface, GameSurfaceMaterial};
use crate::assets::render_data::GameFigures;

static ON: AtomicBool = AtomicBool::new(false);
static LIT: AtomicBool = AtomicBool::new(false);

/// The switch for the run (`OVERALL FIGURE`, with the character's model),
/// set before any material is made.
pub fn set_on(on: bool) {
    ON.store(on, Ordering::Relaxed);
}

/// Whether the OVERALL figure is drawn with the game's surface this run:
/// its materials' figures and cubes are noted as they are made.
pub fn on() -> bool {
    ON.load(Ordering::Relaxed)
}

/// The OVERALL figure's camera is lit by the game's eight in the game's
/// units and asks for the game's light (`figure_view`).
pub fn set_lit() {
    LIT.store(true, Ordering::Relaxed);
}

/// Whether the figure's pieces are drawn with the twins now: on, its camera
/// lit so, and the light the twins' shader calls there.
pub fn ready() -> bool {
    on() && LIT.load(Ordering::Relaxed) && crate::rendering::shading::patched()
}

/// What a material is drawn with on the figure: the game's figures, and its
/// cube (none: the engine's grey).
#[derive(Clone, Debug)]
struct Note {
    figures: GameFigures,
    cube: Option<Handle<Image>>,
}

/// The install's materials, as they are made (`render_data`).
static NOTED: LazyLock<Mutex<HashMap<AssetId<StandardMaterial>, Note>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// A material of the install's drawn by the game's own surface, noted as it
/// is made, with its cube where it names one (this run's, with `on`).
pub fn note(material: &Handle<StandardMaterial>, figures: GameFigures, cube: Option<Handle<Image>>) {
    if on() {
        keep(material.id(), Note { figures, cube });
    }
}

fn keep(material: AssetId<StandardMaterial>, note: Note) {
    if let Ok(mut map) = NOTED.lock() {
        map.insert(material, note);
    }
}

fn note_of(material: AssetId<StandardMaterial>) -> Option<Note> {
    NOTED.lock().ok().and_then(|map| map.get(&material).cloned())
}

/// On a piece drawn with its material's twin: that material, whose pictures
/// the twin draws with (`streaming` weighs them by this piece).
#[derive(Component, Clone, Debug)]
pub struct SurfaceDrawn(Handle<StandardMaterial>);

impl SurfaceDrawn {
    pub fn material(&self) -> &Handle<StandardMaterial> {
        &self.0
    }
}

/// The twin of each noted material, made once and shared.
#[derive(Resource, Default)]
pub struct Twins(HashMap<AssetId<StandardMaterial>, Handle<GameSurfaceMaterial>>);

/// Each piece on the OVERALL figure's layer drawn with a noted material is
/// drawn with its twin: every such piece at the first run (`ready`), then
/// each given a material or a layer since. Run after the figure is made and
/// posed for the frame (`figure_view`). A piece wished to be its shadow
/// alone (a weapon's renderer that casts and draws nothing: `shadow_only`)
/// is left to that.
#[allow(clippy::type_complexity)]
pub fn swap(
    mut commands: Commands,
    pieces: Query<(Entity, Ref<MeshMaterial3d<StandardMaterial>>, Ref<RenderLayers>, Option<&crate::rendering::shadow_only::ShadowOnly>), crate::platform::park::EvenParked>,
    standards: Res<Assets<StandardMaterial>>,
    mut surfaces: ResMut<Assets<GameSurfaceMaterial>>,
    mut twins: ResMut<Twins>,
    mut swept: Local<bool>,
) {
    let figure = RenderLayers::layer(crate::screens::figure_view::OVERALL_LAYER);
    let sweep = !std::mem::replace(&mut *swept, true);
    let (mut made, mut cubed) = (0, 0);
    for (entity, material, layers, wish) in &pieces {
        if !(sweep || material.is_changed() || layers.is_changed()) || !layers.intersects(&figure) || wish.is_some_and(|wish| wish.0) {
            continue;
        }
        let Some(note) = note_of(material.0.id()) else { continue };
        let twin = match twins.0.get(&material.0.id()) {
            Some(twin) => twin.clone(),
            None => {
                let Some(standard) = standards.get(&material.0) else { continue };
                let twin = surfaces.add(GameSurface::lit_by_view(standard, &note.figures, note.cube.clone()));
                twins.0.insert(material.0.id(), twin.clone());
                made += 1;
                cubed += usize::from(note.cube.is_some());
                twin
            }
        };
        commands.entity(entity).remove::<MeshMaterial3d<StandardMaterial>>().insert((MeshMaterial3d(twin), SurfaceDrawn(material.0.clone())));
    }
    if made > 0 {
        info!("the OVERALL figure drawn with the game's own surface: {made} more materials of its own ({cubed} with a cube of their own), {} in all", twins.0.len());
    }
}

/// A noted material pointed at other pictures (a level of one raised or
/// brought down: `streaming`): its twin likewise. One gone: its note and
/// its twin let go of.
fn follow(mut events: MessageReader<AssetEvent<StandardMaterial>>, standards: Res<Assets<StandardMaterial>>, mut twins: ResMut<Twins>, mut surfaces: ResMut<Assets<GameSurfaceMaterial>>) {
    for event in events.read() {
        match *event {
            AssetEvent::Modified { id } => {
                let (Some(twin), Some(base)) = (twins.0.get(&id), standards.get(id)) else { continue };
                if let Some(surface) = surfaces.get_mut(twin) {
                    surface.base.base_color_texture = base.base_color_texture.clone();
                    surface.base.normal_map_texture = base.normal_map_texture.clone();
                    surface.base.metallic_roughness_texture = base.metallic_roughness_texture.clone();
                    surface.base.occlusion_texture = base.occlusion_texture.clone();
                    surface.base.emissive_texture = base.emissive_texture.clone();
                }
            }
            AssetEvent::Removed { id } => {
                twins.0.remove(&id);
                if let Ok(mut map) = NOTED.lock() {
                    map.remove(&id);
                }
            }
            _ => {}
        }
    }
}

pub struct FigureSurfacePlugin;

impl Plugin for FigureSurfacePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Twins>();
        if on() {
            app.add_systems(PostUpdate, follow);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    /// A piece on the OVERALL figure's layer drawn with a noted material
    /// is drawn with its twin (the preview's way lit by the view's lights,
    /// one twin for every piece of that material), keeping the material
    /// for the streaming; a piece of a material not noted, one on another
    /// layer and one wished to be its shadow alone keep theirs.
    #[test]
    fn the_overall_figure_s_pieces_take_the_game_s_surface() {
        let mut world = World::new();
        world.init_resource::<Assets<StandardMaterial>>();
        world.init_resource::<Assets<GameSurfaceMaterial>>();
        world.init_resource::<Twins>();
        let (noted, other) = {
            let mut standards = world.resource_mut::<Assets<StandardMaterial>>();
            (standards.add(StandardMaterial::default()), standards.add(StandardMaterial::default()))
        };
        let figures = GameFigures {
            color: [1.0; 4],
            spec_color: [1.0; 4],
            reflect_color: [0.4434, 0.4434, 0.4434, 1.0],
            def_vals: [0.55, 0.7],
            spec_vals: [1.1, 2.0],
            specularness: 1.0,
            glossness: 1.0,
            bump_tiling: 1.0,
            has_gloss: true,
            raw_colour: false,
        };
        keep(noted.id(), Note { figures, cube: None });
        let figure = RenderLayers::layer(crate::screens::figure_view::OVERALL_LAYER);
        let a = world.spawn((MeshMaterial3d(noted.clone()), figure.clone())).id();
        let b = world.spawn((MeshMaterial3d(noted.clone()), figure.clone())).id();
        let not_noted = world.spawn((MeshMaterial3d(other.clone()), figure.clone())).id();
        let elsewhere = world.spawn((MeshMaterial3d(noted.clone()), RenderLayers::layer(crate::screens::figure_view::LAYER))).id();
        let shadow = world.spawn((MeshMaterial3d(noted.clone()), figure.clone(), crate::rendering::shadow_only::ShadowOnly(true))).id();
        // (A weapon's piece drawn as such: its wish there, false.)
        let drawn = world.spawn((MeshMaterial3d(noted.clone()), figure, crate::rendering::shadow_only::ShadowOnly(false))).id();
        world.run_system_once(swap).expect("the system ran");
        let twin = |entity: Entity| world.get::<MeshMaterial3d<GameSurfaceMaterial>>(entity).map(|m| m.0.clone());
        let (twin_a, twin_b) = (twin(a).expect("its twin"), twin(b).expect("its twin"));
        assert_eq!(twin_a, twin_b, "one twin for the material");
        assert_eq!(twin(drawn), Some(twin_a.clone()), "a weapon's piece drawn as such");
        assert!(world.get::<MeshMaterial3d<StandardMaterial>>(a).is_none());
        assert_eq!(world.get::<SurfaceDrawn>(a).map(|drawn| drawn.material().clone()), Some(noted.clone()));
        for kept in [not_noted, elsewhere, shadow] {
            assert!(twin(kept).is_none() && world.get::<MeshMaterial3d<StandardMaterial>>(kept).is_some());
        }
        let surfaces = world.resource::<Assets<GameSurfaceMaterial>>();
        let made = surfaces.get(&twin_a).expect("made");
        assert_eq!(made.extension.figures.way.x, 6.0, "the preview's way, lit by the view's lights");
        assert!(made.extension.view_lights);
        assert_eq!(made.extension.figures.way.y, 0.0, "no lights in its figures");
        assert_eq!(made.extension.figures.vals, Vec4::new(0.55, 0.7, 1.1, 2.0));
        assert_eq!(made.base.base_color, Color::WHITE, "the shader takes `_Color` itself");
    }
}
