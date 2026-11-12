//! Tests of the inventory's OVERALL figure against the game's, from the install and three recorded takes.
//! After the inventory spec, "The model's view", and the body-animator spec, "The stance of
//! the game's screens", through `trp_sim::figure`: frame by frame beside the three takes of
//! recorder 0.17.0 (`local/traces/overall2_s252`).
//! These pass trivially, with a note, where no install or no recording is
//! there.

use std::collections::BTreeMap;
use std::io::BufRead;

use serde_json::Value as Json;
use trp_data::unity::anim::path_hash;
use trp_data::unity::Assets;
use trp_data::Database;
use trp_sim::weapons::arms::Placed;
use trp_sim::characters::figure::{kind, kind_of, Figure, FigureBones, FigureData, FigureEvent, FigureWeaponData, Loaded, Pick};

mod idle;
mod takes;
mod held;

use takes::{said};

const AKM: &str = "59d6088586f774275f37482f";
const DT: f32 = 1.0 / 60.0;

fn data() -> Option<(FigureData, Assets, &'static Database)> {
    let install = crate::fixture::install()?;
    let db = crate::fixture::database().expect("database");
    let mut assets = Assets::new(install).expect("manifest");
    let started = std::time::Instant::now();
    let data = FigureData::load(&mut assets, &[]).expect("the figure");
    println!("the figure's body read in {:.0} ms", started.elapsed().as_secs_f32() * 1000.0);
    Some((data, assets, db))
}

fn distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    (0..3).map(|k| (a[k] - b[k]).powi(2)).sum::<f32>().sqrt()
}

/// The angle between two turns, in degrees.
fn apart(a: [f32; 4], b: [f32; 4]) -> f32 {
    let dot: f32 = (0..4).map(|k| a[k] * b[k]).sum();
    2.0 * dot.abs().min(1.0).acos().to_degrees()
}

/// A layer's state by its own name (after the last dot), how far through it
/// in cycles, and the state it is entering.
fn layer(figure: &Figure, index: usize) -> (String, f32, Option<String>) {
    let animator = figure.animator();
    let (state, next) = animator.state(index);
    let own = |name: &str| name.rsplit('.').next().unwrap_or(name).to_owned();
    (own(&state.name), animator.progress(index).0, next.map(|next| own(&next.name)))
}

/// The engine's short name hash as the recorder writes it (its low 24 bits).
fn hash(name: &str) -> i64 {
    i64::from(path_hash(name) & 0x00ff_ffff)
}
