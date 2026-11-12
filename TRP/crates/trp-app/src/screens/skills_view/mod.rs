//! The inventory's SKILLS tab, showing which skills take effect here.
//!
//! By the skills spec, "The SKILLS tab": the game's own page with `TARKOV UI`,
//! a skill whose effects the app does not have drawn as the game draws a locked one.
//!
//! With `TARKOV UI` it is the game's page (`SkillsAndMasteringScreen`,
//! `SkillsScreen`), drawn from the install's scene: every skill of the
//! game's list but the locked ones (`shown`) as its list's view
//! (`SkillPanel`) or its grid's (`SkillIcon`), at its level as set (the
//! options' SKILLS page, or a host game in play), with its fill, its buffs'
//! icons (`BuffIcon`) and, with the mouse on its icon, the game's skill
//! tooltip (`SkillTooltip`) with a line for each buff; a buff's icon under
//! the mouse says its buff in the simple tooltip. The sort and the filter
//! (`DropDownBox`), the list's and the grid's tabs, the scroll view and its
//! bar work as the game's. A skill whose effects the app does not have is
//! drawn as the game draws a locked one (`CanvasGroup.SetUnlockStatus`,
//! 0.3, on the view and on its icon) with its tooltip, the reason at the
//! tooltip's end; with the options' SKILLS off, every one. The caption's MASTERING tab shows the
//! game's MASTERING page (`mastery_view`), drawn here with the page's
//! frame; the experience at the top is the character's (`experience`;
//! `PlayerExperiencePanel.Set`), each skill's leveling speed
//! its own session's. Without `TARKOV UI` it is the app's own list, the
//! weapons' mastering under the skills.

use std::collections::HashMap;

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use trp_data::interface::screen::{ScenePanel, SkillsScreen, SKILL_DROPDOWNS, SKILL_GRID, SKILL_LIST, SKILL_SCROLL};
use trp_sim::condition::skills::{buffs_of, BuffDef, BuffValue, Class, Skill, SkillLevels};

use crate::screens::health_view::{copy_within, draw_nodes, inset_by_margins, prefer_size, rgba, rich_spans, screen_numbers, shown_of, tmp_words, Drawn, Frame, HealthLook, Marks, Picture, PictureReader, Piece, TmpText, Words, EXACT};
use crate::screens::inventory_view::{words, FAINT, PANEL, WORDS};
use crate::screens::overall_view::{bar_handle, dropdown_places, inside, screen_of, Places};
use crate::audio::ui_sound::{Sound, Ui};

mod look;
mod skill_words;
mod mouse;
mod drawing;
mod page_drawn;
mod tips;

pub use look::*;
pub use skill_words::*;
use skill_words::{ELITE_BORDER, LevelWords, buff_words, dimmed_why, level_words, listed_by, rank_sprites, set_bar, skill_name};
#[cfg(test)]
use skill_words::{dotnet_figure, fill_hole, level_exp};
#[cfg(test)]
use skill_words::listed;
use mouse::{Held, Hover, PageTab, Scrolled, SkillsState, View, fit, hear, lines_of, panel_rect, places, scrolled, scrolling_bar};
#[cfg(test)]
use mouse::{Fill};
use drawing::{BodyShows, OwnList, draw, make_layers, page_texts};
use page_drawn::{arrows_of, child, draw_mastering, draw_page, rate_line};
#[cfg(test)]
use page_drawn::{percent_of};
use tips::{built_tip, draw_own, draw_skill_tip, simple_tip};

pub struct SkillsPlugin;

impl Plugin for SkillsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SkillsState>().add_systems(Update, (hear, make_layers, draw).chain().after(crate::screens::inventory_view::ScreenDrawn).run_if(crate::running));
    }
}

/// The page's node (as large as the window), drawn into afresh as what it
/// shows changes.
#[derive(Component)]
pub struct SkillsPage;

/// The page's own drawing, and the tooltip's layer over it, moved with the
/// mouse.
#[derive(Component)]
struct SkillsBody;
#[derive(Component)]
struct SkillsTip;

/// Where the page's objects are numbered among the drawn pieces: the
/// page's, each skill's view's (each one's past the one before it), the
/// tooltip's and the dropdowns' lines'.
pub(crate) const PAGE: usize = 25 << 20;
const VIEWS: usize = 26 << 20;
const TIP: usize = 27 << 20;
const LINES: usize = 28 << 20;

/// What the game draws a locked skill's view and icon at (`MyExtensions.
/// SetUnlockStatus`: its `CanvasGroup`'s alpha), and so the app's skills
/// that change nothing here (the app's choice).
const DIM: f32 = 0.3;
/// The level's words at elite, a key to the install's words.
const ELITE_LEVEL: &str = "<color=#c78b2c>ELITE Level</color>";
/// The sort's lines (`SkillsScreen.Show`) and the filter's after "All
/// types": the classes' names in `ESkillClass`'s order.
const SORTS: [&str; 3] = ["Default", "Most leveled", "Most recent"];
const CLASSES: [&str; 5] = ["Physical", "Combat", "Special", "Practical", "Mental"];
/// The two tabs of the list's and the grid's views, below the page's top.
const VIEW_TABS: [&str; 2] = ["BottomPanel/SkillsPanel/Options/Tabs/List Tab", "BottomPanel/SkillsPanel/Options/Tabs/Grid Tab"];
/// A skill's fill's arrows, up and down (`_effectivenessUp`,
/// `_effectivenessDown`), below the list's view's top.
const ARROWS: [&str; 2] = ["Fill Bar/Container/EffectivenessUp", "Fill Bar/Container/EffectivenessDown"];
const CAPTION_SKILLS: &str = "Caption Bar/Tabs/Skills";
const CAPTION_MASTERING: &str = "Caption Bar/Tabs/Mastering";
/// The two pages' panels below the page's top.
const SKILLS_PANEL: &str = "BottomPanel/SkillsPanel";
const EXPERIENCE: &str = "TopPanel/Progress Panel";
/// How far right and down of the mouse the skill tooltip's corner is
/// (`SkillTooltip.Show`: `(25, -25)`, the engine's y up), and the simple
/// tooltip's lower left corner up and right of it (`Tooltip.SetPosition`).
const TIP_OFFSET: f32 = 25.0;
const SIMPLE_OFFSET: f32 = 10.0;
/// How far down the app's own list starts, in pixels: under its row of
/// tabs. `ESTIMATE`: the app's own figure, with no game figure behind it
/// (the game's look has none: it is the game's page).
const OWN_TOP: f32 = 110.0;

#[cfg(test)]
mod tests;
