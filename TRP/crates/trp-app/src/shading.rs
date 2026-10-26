//! Tarkov's surface shading, with the options' `TARKOV SHADING` (and
//! `CUSTOMS LIGHT`; `docs/spec/lighting.md`, "Tarkov's surface shading").
//!
//! The game lights its surfaces by its own copy of the engine's deferred
//! light (`Hidden/Internal-DeferredShadingEFT`) and its ambient pass after
//! it, every figure its gamma project's number. Bevy's PBR shader is
//! patched as it loads (as `gamma_ui` patches the screens'): before its
//! `apply_pbr_lighting` goes `game_shading.wgsl`, and a fragment of the
//! view that asks for it is lit there instead. The main camera asks by its
//! own ambient light, whose alpha is below nought (no ambient light's is)
//! and whose red carries the ambient pass's brightness term. Its lights are
//! handed as the game's: the sun's colour times intensity as Bevy's colour
//! at an illuminance of one, a lamp's colour with its intensity times 4π
//! (which Bevy divides by) and its range (`lighting`, `explosion_view`).
//! The icon studio and the figures' views keep Bevy's light.
//!
//! `PROVISIONAL` (the lighting spec, "Tarkov's surface shading"): the surfaces'
//! figures are Bevy's materials' (the game's own specular colour, rim, gloss
//! scale and reflections are the second part); a material's colour factor is
//! taken with its picture in linear light; the shadows are Bevy's cascades;
//! the ambient's normal is made of length one; see-through materials and the
//! highlight's twins are added in linear light. The spot's cone for the
//! engine's cookie is marked in `game_shading.wgsl`.

use std::sync::atomic::{AtomicBool, Ordering};

use bevy::asset::AssetEvent;
use bevy::prelude::*;
use bevy::shader::{Shader, Source};

static ON: AtomicBool = AtomicBool::new(false);

/// Whether the scene is lit by the game's shading this run.
pub fn on() -> bool {
    ON.load(Ordering::Relaxed)
}

/// Bevy's PBR functions, where the lighting is put together.
const PBR_FUNCTIONS: &str = "embedded://bevy_pbr/render/pbr_functions.wgsl";
const LIGHTING: &str = "fn apply_pbr_lighting(";
const GAME_SHADING: &str = include_str!("game_shading.wgsl");
/// What `apply_pbr_lighting` begins with once patched.
const ASKED: &str = "
    // (trp: the game's shading in the view that asks for it, `shading.rs`.)
    if view_bindings::lights.ambient_color.a < 0.0 {
        return trp_game_lighting(in);
    }
";

/// The main camera's ambient light that asks for the game's shading,
/// carrying the ambient pass's brightness term.
pub fn asking(brightness_term: f32) -> AmbientLight {
    AmbientLight { color: Color::linear_rgba(brightness_term, 0.0, 0.0, -1.0), brightness: 1.0, affects_lightmapped_meshes: true }
}

pub struct ShadingPlugin {
    pub on: bool,
}

impl Plugin for ShadingPlugin {
    fn build(&self, app: &mut App) {
        ON.store(self.on, Ordering::Relaxed);
        if self.on {
            app.add_systems(Startup, hold).add_systems(Update, patch);
        }
    }
}

#[derive(Resource)]
struct Held(Handle<Shader>);

fn hold(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(Held(assets.load(PBR_FUNCTIONS)));
}

/// Bevy's PBR functions, as they are loaded (and again if ever loaded
/// afresh), put back patched; if their lighting function is not found
/// (another Bevy), they are left as they are, with a line saying so.
fn patch(held: Option<Res<Held>>, mut events: MessageReader<AssetEvent<Shader>>, mut shaders: ResMut<Assets<Shader>>) {
    let Some(held) = held else { return };
    for event in events.read() {
        let (AssetEvent::Added { id } | AssetEvent::Modified { id } | AssetEvent::LoadedWithDependencies { id }) = *event else { continue };
        if id != held.0.id() {
            continue;
        }
        let Some(shader) = shaders.get(id) else { continue };
        let Source::Wgsl(source) = &shader.source else { continue };
        if source.contains("fn trp_game_lighting(") {
            continue;
        }
        let Some(function) = source.find(LIGHTING) else {
            warn!("{PBR_FUNCTIONS}: its lighting function was not found; the scene is lit by Bevy's shading");
            continue;
        };
        let Some(body) = source[function..].find('{').map(|at| function + at + 1) else { continue };
        // Before the function's own `#ifndef PREPASS_FRAGMENT`, as ours has
        // its own.
        let before = source[..function].rfind("#ifndef PREPASS_FRAGMENT").unwrap_or(function);
        let patched = format!("{}{GAME_SHADING}\n{}{ASKED}{}", &source[..before], &source[before..body], &source[body..]);
        // Its imports as the patched source uses them (naga_oil takes them
        // from what the code names), the rest of it as it was.
        let fresh = Shader::from_wgsl(patched, shader.path.clone());
        let known = shader.imports.clone();
        let replaced = Shader { source: fresh.source, imports: fresh.imports, import_path: fresh.import_path, ..shader.clone() };
        if let Some(new) = replaced.imports.iter().find(|import| !known.contains(import)) {
            warn!("{PBR_FUNCTIONS}: patched, it would import {new:?}, which it did not; the scene is lit by Bevy's shading");
            continue;
        }
        let _ = shaders.insert(id, replaced);
        info!("Tarkov's surface shading: Bevy's PBR lighting patched");
    }
}

