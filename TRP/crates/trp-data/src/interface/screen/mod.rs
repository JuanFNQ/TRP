//! The game's own screens as its scenes hold them: the objects of one of the install's scene files.
//! Each with its name, what it hangs from and the rectangle its layout gives it
//! (`docs/spec/inventory.md`, "Where the views sit").
//!
//! A scene file (`EscapeFromTarkov_Data/level<N>`) is a serialized file
//! with no type trees, so the three kinds of object wanted are read by
//! the engine's own fixed layouts for them: an object's name and
//! components, a transform's place in the hierarchy, and a screen
//! transform's anchors, offset, size and pivot.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{bail, Context, Result};

use crate::setup::install::Install;
use crate::unity::reader::Reader;
use crate::unity::{class, SerializedFile};

mod skills;
pub use skills::*;

mod scene_read;
mod notices;
mod context_menus;
mod windows;
mod icons;
mod gear;
mod looks;
mod panel;
mod panel_layout;
mod health;
mod overall;
mod pictures;
mod item_views;
mod tabs;
mod tmp_font;
mod quick;
mod battle;
mod sights;
mod stash;
mod rigs;
mod materials;
mod prefab_reader;

pub use notices::*;
pub use context_menus::*;
pub use windows::*;
pub use icons::*;
pub use gear::*;
pub use looks::*;
pub use panel::*;
pub use health::*;
pub use overall::*;
pub use pictures::*;
pub use item_views::*;
pub use tabs::*;
pub use tmp_font::*;
pub use quick::*;
pub use battle::*;
pub use sights::*;
pub use stash::*;
pub use rigs::*;
pub use materials::*;
pub use prefab_reader::*;
use windows::{context_window};
use gear::{GEAR_VIEWS};
use looks::{color, flag};
use panel::row_group;
use health::{pointers_from};
use pictures::{SPRITE};
use item_views::{selectable, skip_odin};
use sights::{UI_ELEMENT};
use prefab_reader::{PrefabReader};

/// The engine's class number for a transform laid out on a screen.
const RECT_TRANSFORM: i32 = 224;

// ==== A scene's objects: layouts, nodes, rects, behaviours, pointers ====

/// How a screen transform is laid out in what it hangs from, in the
/// engine's terms: the two anchors as shares of the parent's rectangle,
/// the pivot's offset from the anchors, the size added to the anchors'
/// spread, and the pivot as a share of its own rectangle.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Layout {
    pub anchor_min: [f32; 2],
    pub anchor_max: [f32; 2],
    pub offset: [f32; 2],
    pub size_delta: [f32; 2],
    pub pivot: [f32; 2],
    pub scale: [f32; 2],
    /// Its turn about the screen's own axis, in degrees, counter-clockwise
    /// as the screen is seen (its transform's rotation about z).
    pub turn: f32,
}

/// A rotation's turn about z, in degrees, from its quaternion (x, y, z, w):
/// what a screen's objects are turned by (none is turned about another axis).
fn z_turn([x, y, z, w]: [f32; 4]) -> f32 {
    let _ = (x, y);
    (2.0 * z.atan2(w)).to_degrees()
}

/// An object's transform against its parent's, in the engine's axes (z
/// forward, the turn's quaternion x, y, z, w).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Local {
    pub rotation: [f32; 4],
    pub position: [f32; 3],
    pub scale: [f32; 3],
}

/// The turn `a` then `b` (`a * b`), quaternions x, y, z, w.
pub fn quaternion_product(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    let [ax, ay, az, aw] = a;
    let [bx, by, bz, bw] = b;
    [
        aw * bx + ax * bw + ay * bz - az * by,
        aw * by - ax * bz + ay * bw + az * bx,
        aw * bz + ax * by - ay * bx + az * bw,
        aw * bw - ax * bx - ay * by - az * bz,
    ]
}

/// One object of a scene.
#[derive(Debug, Clone)]
pub struct Node {
    pub name: String,
    pub active: bool,
    pub parent: Option<usize>,
    pub children: Vec<usize>,
    /// None for an object that is not laid out on a screen.
    pub layout: Option<Layout>,
    /// The classes of its scripts, where they were asked for.
    pub scripts: Vec<String>,
}

/// A rectangle on a screen: its lower left corner and its size, in the
/// screen's own units, y up.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub at: [f32; 2],
    pub size: [f32; 2],
}

/// A script on an object of a scene: the object it is on, and where its
/// own fields lie in the file.
#[derive(Debug, Clone, Copy)]
pub struct Behaviour {
    /// Its own number in the file, which is what points to it.
    pub id: i64,
    pub node: usize,
    /// Whether it is switched on (`Behaviour.enabled`): an image or a text
    /// switched off draws nothing.
    pub enabled: bool,
    fields: (usize, usize),
}

pub struct Scene {
    pub nodes: Vec<Node>,
    pub behaviours: Vec<Behaviour>,
    /// Each script's class by its number, where they were asked for.
    classes: HashMap<i64, String>,
    /// Each object by its own number, which is what points to it.
    objects: HashMap<i64, usize>,
    /// Each object by its transform's number, which is what a screen's
    /// scripts point to for a place on it (`RectTransform`).
    transforms: HashMap<i64, usize>,
    /// Each object by the numbers of its engine's own parts (a
    /// `CanvasGroup` and the rest), which scripts point to as well.
    components: HashMap<i64, usize>,
    /// Each of those parts' engine class (`Canvas` 223, `CanvasGroup` 225
    /// and the rest), by its number.
    component_classes: HashMap<i64, i32>,
    /// Each object's own transform as the file has it (turn, place and
    /// scale in the engine's axes, against its parent), where it was read
    /// from a scene file.
    locals: HashMap<usize, Local>,
    /// Where each object of the file lies in it, by its number.
    spans: HashMap<i64, (usize, usize)>,
    /// Each object's layer (`GameObject.m_Layer`), where it was read from
    /// a file.
    layers: HashMap<usize, u32>,
    /// The files a pointer's first number counts in, from 1.
    pub externals: Vec<String>,
    data: Vec<u8>,
}

struct Pointer {
    file: i32,
    path: i64,
}

fn pointer(r: &mut Reader) -> Result<Pointer> {
    Ok(Pointer { file: r.i32()?, path: r.i64()? })
}

fn pair(r: &mut Reader) -> Result<[f32; 2]> {
    Ok([r.f32()?, r.f32()?])
}

fn string(r: &mut Reader) -> Result<String> {
    let len = r.u32()? as usize;
    let text = String::from_utf8_lossy(r.bytes(len)?).into_owned();
    r.align(4);
    Ok(text)
}

/// The engine's class number for the build's own settings, which begin
/// with the list of its scenes.
const BUILD_SETTINGS: i32 = 141;

/// The engine's class number of a cubemap.
const CUBEMAP: i32 = 89;
