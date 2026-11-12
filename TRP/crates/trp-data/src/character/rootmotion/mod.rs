//! The player's root motion table.
//!
//! The game precomputes, for every state of the body animator, how fast the
//! character moves as a function of the animator's parameters. A state is a
//! tree: clips carry a velocity, and blend trees mix their children by
//! parameters. This is what the game uses for characters that do not run a
//! full animator, and it is where walking and sprinting speeds come from.
//!
//! Format and evaluation are described in `docs/spec/movement.md`.

use std::collections::HashMap;

use anyhow::{bail, Context, Result};

use crate::unity::anim::path_hash;
use crate::unity::extract::script_class;
use crate::unity::{class, Assets, Value};

mod table_queries;
mod blend_weights;
mod node_read;

pub use blend_weights::*;
use node_read::{Bin, read_node, text_asset_bytes};

pub const TABLE_BUNDLE: &str = "assets/content/characters/rootmotiontable/playerrootmotiontable.asset.bundle";

/// Weights below this are treated as zero, as the game does.
const SKIP_WEIGHT: f32 = 0.01;

/// Range and grid spacing of one animator parameter.
#[derive(Debug, Clone)]
pub struct Parameter {
    pub name: String,
    pub min: f32,
    pub max: f32,
    pub step: f32,
}

impl Parameter {
    fn steps(&self) -> usize {
        ((self.max - self.min).abs() / self.step).ceil() as usize + 1
    }

    /// Grid cell and position within it for a value.
    fn locate(&self, value: f32) -> (usize, usize, f32) {
        let value = value.clamp(self.min, self.max);
        let total = self.steps();
        if total == 1 {
            return (0, 0, 0.0);
        }
        let mut a = ((value - self.min) / self.step).floor() as usize;
        if a >= total {
            a = total - 1;
        }
        let t = (value - (self.min + a as f32 * self.step)) / self.step;
        let b = if a + 1 < total { a + 1 } else { a };
        (a, b, t.clamp(0.0, 1.0))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlendType {
    Simple1D,
    FreeformDirectional2D,
    FreeformCartesian2D,
    SimpleDirectional2D,
    Direct,
}

#[derive(Debug, Clone)]
pub enum Node {
    Clip {
        name: String,
        looping: bool,
        /// Seconds.
        length: f32,
        time_scale: f32,
        /// Metres per second in the character's frame: x right, y up, z forward.
        velocity: [f32; 3],
        /// The game reads this clip's motion from curves instead of the
        /// velocity. Used for one-off moves such as vaults and kicks.
        uses_curves: bool,
    },
    Tree {
        name: String,
        blend: BlendType,
        param_x: u32,
        param_y: u32,
        thresholds: Vec<f32>,
        /// Where each child sits in a two-dimensional blend.
        positions: Vec<[f32; 2]>,
        direct_params: Vec<u32>,
        /// Precomputed child weights over the grid of both parameters,
        /// indexed `[(x * steps_y + y) * children + child]`.
        steps_x: usize,
        steps_y: usize,
        weights: Vec<f32>,
        children: Vec<Node>,
    },
}

impl Node {
    pub fn name(&self) -> &str {
        match self {
            Node::Clip { name, .. } | Node::Tree { name, .. } => name,
        }
    }
}

/// Values of the animator's float parameters, by name hash.
pub trait Parameters {
    fn get(&self, hash: u32) -> f32;
}

impl Parameters for HashMap<u32, f32> {
    fn get(&self, hash: u32) -> f32 {
        self.get(&hash).copied().unwrap_or(0.0)
    }
}

#[derive(Clone)]
pub struct RootMotionTable {
    pub parameters: Vec<Parameter>,
    /// States by the hash of their full path, such as `Base Layer.Idle`.
    pub states: HashMap<u32, (String, Node)>,
    by_hash: HashMap<u32, usize>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_dimensional_weights_form_a_partition() {
        let thresholds = [0.0, 0.5, 1.0];
        for blend in [-1.0, 0.0, 0.2, 0.5, 0.9, 1.0, 2.0] {
            let w: Vec<f32> = (0..3).map(|i| weight_1d(&thresholds, i, blend)).collect();
            assert!((w.iter().sum::<f32>() - 1.0).abs() < 1e-5, "{blend}: {w:?}");
        }
        assert_eq!(weight_1d(&thresholds, 1, 0.25), 0.5);
        assert_eq!(weight_1d(&thresholds, 0, 0.25), 0.5);
        assert_eq!(weight_1d(&thresholds, 2, 0.25), 0.0);
    }

    #[test]
    fn parameter_grid_lookup() {
        let p = Parameter { name: "Speed".into(), min: 0.0, max: 1.0, step: 0.1 };
        assert_eq!(p.steps(), 11);
        let (a, b, t) = p.locate(0.25);
        assert_eq!((a, b), (2, 3));
        assert!((t - 0.5).abs() < 1e-4);
        assert_eq!(p.locate(5.0).0, 10);
    }
}
