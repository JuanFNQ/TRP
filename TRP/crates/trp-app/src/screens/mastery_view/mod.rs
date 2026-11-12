//! The SKILLS tab's MASTERING page, built as the game's.
//! By the skills spec, "Weapon mastering".
//!
//! With `TARKOV UI` it is the game's page (`MasteringScreen`), drawn from
//! the install's scene as the SKILLS page is (`skills_view`, which draws
//! the page's frame, its caption and its experience, and hands this page
//! its part): a row for each weapon of each begun family, in the list
//! (`MasteringPanel` with its `MasteringIcon`) or the grid (`MasteringIcon`),
//! each with the weapon's picture as the inventory has it, turned and
//! fitted as the game's `ItemIconView` does; the search field, the filter
//! by weapon type and the sort (`MasteringContainer`); the mastering
//! tooltip over a grid's icon (`MasteringTooltip`). With the options'
//! `WEAPON MASTERING` off the rows are dimmed (they change nothing). Without
//! `TARKOV UI` the rows are listed under the app's own list of skills.

use std::collections::HashMap;

use bevy::prelude::*;
use trp_data::interface::screen::{MasteringScreen, ScenePanel, MASTERING_DROPDOWNS, MASTERING_GRID, MASTERING_LIST, MASTERING_SCROLL, MASTERING_SEARCH, MASTERING_TABS};
use trp_sim::condition::mastering::Shown;

use crate::screens::health_view::{draw_nodes, inset_by_margins, shown_of, Drawn, Frame, HealthLook, Marks, Picture, PictureReader, Words, EXACT};
use crate::screens::overall_view::{bar_handle, dropdown_places, inside, Places};
use crate::screens::text_field::{draw_field_marks, field_key, field_place, Rules, Spot, TextField};
use crate::audio::ui_sound::{Sound, Ui};

mod look_rows;
mod mouse;
mod rows_drawn;

pub use look_rows::*;
pub(crate) use mouse::*;
pub(crate) use rows_drawn::*;
use look_rows::{lines_of, listed, row_words};
use mouse::{Held, rect_of, scrolled, scrolling_bar};

/// Where the page's objects are numbered among the drawn pieces: a row's
/// view (each past the one before), the tooltip, the dropdowns' lines.
const VIEWS: usize = 29 << 20;
const TIP: usize = 30 << 20;
const LINES: usize = 31 << 20;

/// The sort's lines (`MasteringScreen.Show`).
const SORTS: [&str; 3] = ["Default", "Most leveled", "Most recent"];
/// What the rows are drawn at with the options' `WEAPON MASTERING` off
/// (the SKILLS page's dimming of what changes nothing, `SetUnlockStatus`'s
/// 0.3).
const DIM: f32 = 0.3;
/// A weapon's picture's place in its view, and its loader's
/// (`ItemIconView._mainImage`, `_iconLoader`), below the list's view's top
/// (the grid's: the same less `Skill Icon/`); the frame that cuts it
/// (`Level Up Button`'s `RectMask2D`).
const PICTURE: &str = "Level Up Button/Skill Image";
const LOADER: &str = "Level Up Button/Small Loader";
const FRAME: &str = "Level Up Button";
/// How far the weapon's picture is turned (`ItemIconView._iconRotation`,
/// the scene's 0, 0, 45; the engine's turn the other way about, as the
/// quick-use bar's).
const TURN_DEGREES: f32 = -45.0;
/// How far right and down of the mouse the tooltip's corner is
/// (`MasteringTooltip.Show`: `(25, -25)`, the engine's y up).
const TIP_OFFSET: f32 = 25.0;

#[cfg(test)]
mod tests;
