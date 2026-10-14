//! Grid lines on the test range's ground and boxes, so that distance, speed
//! and height can be read off the picture: a faint line every half metre, a
//! clearer one every metre and a strong one every five, on whole steps of
//! the world's own axes. The range's posts stand every five metres along
//! the lane, on the strong lines; a wall's lines are heights over the
//! ground.
//!
//! This is the app's own drawing of its stand-in world and nothing of the
//! game's: the standard material with a few lines of shader over its base
//! colour (`grid.wgsl`), compiled into the app.

use bevy::asset::{load_internal_asset, weak_handle};
use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderRef, ShaderType};

const GRID_SHADER: Handle<Shader> = weak_handle!("6f0c6f0e-6a1d-4b53-9d0a-5c1f2b9e7a41");

/// The standard material with the grid's lines over its base colour.
pub type GridMaterial = ExtendedMaterial<StandardMaterial, GridLines>;

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct GridLines {
    #[uniform(100)]
    pub lines: Lines,
}

pub use uniform::Lines;

// The derive that lays the figures out for the shader writes helpers this
// compiler counts as unused.
#[allow(dead_code)]
mod uniform {
    use super::*;

    /// Three levels of line, finest first.
    #[derive(ShaderType, Reflect, Debug, Clone, Copy)]
    pub struct Lines {
        /// The colour the lines are drawn toward (linear; alpha unused).
        pub tone: Vec4,
        /// Metres between the lines of each level (the fourth is unused).
        pub spacing: Vec4,
        /// Metres each level's lines are wide.
        pub width: Vec4,
        /// How far toward the tone each level is drawn, 0 to 1.
        pub strength: Vec4,
    }
}

impl Lines {
    const SPACING: Vec4 = Vec4::new(0.5, 1.0, 5.0, 1.0);
    const WIDTH: Vec4 = Vec4::new(0.010, 0.016, 0.032, 0.0);

    /// Lines for the ground: lighter than it, which is dark.
    pub fn ground() -> Self {
        Self {
            tone: LinearRgba::from(Color::srgb(0.66, 0.69, 0.62)).to_vec4(),
            spacing: Self::SPACING,
            width: Self::WIDTH,
            strength: Vec4::new(0.07, 0.17, 0.36, 0.0),
        }
    }

    /// Lines for a box: darker than its own colour, like a seam.
    pub fn solid() -> Self {
        Self {
            tone: LinearRgba::from(Color::srgb(0.08, 0.08, 0.09)).to_vec4(),
            spacing: Self::SPACING,
            width: Self::WIDTH,
            strength: Vec4::new(0.10, 0.24, 0.24, 0.0),
        }
    }

    /// No lines at all: the plain surface, as the range was drawn before.
    pub fn none(self) -> Self {
        Self { strength: Vec4::ZERO, ..self }
    }
}

impl MaterialExtension for GridLines {
    fn fragment_shader() -> ShaderRef {
        GRID_SHADER.into()
    }
}

pub struct GridPlugin;

impl Plugin for GridPlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(app, GRID_SHADER, "grid.wgsl", Shader::from_wgsl);
        app.add_plugins(MaterialPlugin::<GridMaterial>::default());
    }
}
