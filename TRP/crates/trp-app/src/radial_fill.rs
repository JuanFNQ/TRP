//! A picture filled radially (`Image.Type.Filled`, `Radial360`), from the
//! bottom, clockwise, as the game's progress spinner is (`ProgressSpinner`;
//! inventory spec, "The load component"): a UI material that keeps of a
//! sprite's cut what lies within the share filled about the node's middle.
//! Its colours are worked as the game's screens are where those are
//! composed in gamma space (`gamma_ui`).

use bevy::asset::{load_internal_asset, uuid_handle};
use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;
use bevy::ui_render::prelude::{UiMaterial, UiMaterialPlugin};

const RADIAL_FILL_SHADER: Handle<Shader> = uuid_handle!("8f3c1a27-5d64-4b9e-a0f2-6e1d9c4b7a35");

/// A sprite's cut drawn filled so far round.
#[derive(Asset, AsBindGroup, TypePath, Debug, Clone)]
pub struct RadialFill {
    /// Its colour.
    #[uniform(0)]
    pub color: Vec4,
    /// The share filled (0 to 1), 1 where the screens are composed in
    /// gamma space, and the picture's turn clockwise about the node's
    /// middle as a share of a whole one (a spinner's; 0 for none).
    #[uniform(1)]
    pub fill: Vec4,
    /// The cut, in the texture's pixels from its first row, which is its
    /// lowest: left, lowest, right, highest.
    #[uniform(2)]
    pub cut: Vec4,
    #[texture(3)]
    #[sampler(4)]
    pub texture: Handle<Image>,
}

impl RadialFill {
    pub fn new(texture: Handle<Image>, cut: Rect, color: Color, gamma: bool) -> Self {
        let c = color.to_linear();
        Self {
            color: Vec4::new(c.red, c.green, c.blue, c.alpha),
            fill: Vec4::new(0.0, if gamma { 1.0 } else { 0.0 }, 0.0, 0.0),
            cut: Vec4::new(cut.min.x, cut.min.y, cut.max.x, cut.max.y),
            texture,
        }
    }

    /// Its colour now.
    pub fn set_color(&mut self, color: Color) {
        let c = color.to_linear();
        self.color = Vec4::new(c.red, c.green, c.blue, c.alpha);
    }

    /// Its picture whole, turned clockwise by `turn` of a whole turn.
    pub fn set_turn(&mut self, turn: f32) {
        self.fill.x = 1.0;
        self.fill.z = turn;
    }
}

impl UiMaterial for RadialFill {
    fn fragment_shader() -> ShaderRef {
        RADIAL_FILL_SHADER.into()
    }
}

pub struct RadialFillPlugin;

impl Plugin for RadialFillPlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(app, RADIAL_FILL_SHADER, "radial_fill.wgsl", Shader::from_wgsl);
        app.add_plugins(UiMaterialPlugin::<RadialFill>::default());
    }
}
