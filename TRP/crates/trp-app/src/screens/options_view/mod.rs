//! The options tab, the last of the inventory screen's tabs: the keys, the display and the systems' switches.
//!
//! By `docs/spec/settings.md`. The game keeps its settings on a screen of
//! its menu; the app has them here. What the tab holds is the game's where the
//! game has it: the bindings are the install's, shown with the game's own
//! words for them and changed by the game's own rules
//! (`trp_sim::bindings`), and the display's settings are the four of the
//! game's the app takes. How the tab looks is the app's.
//!
//! What is changed here is kept in the app's own file
//! (`local\options.json`), never in the install.

use std::collections::HashMap;
use std::path::PathBuf;

use bevy::app::AppExit;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::mouse::AccumulatedMouseScroll;
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;
use bevy::window::{Monitor, MonitorSelection, PresentMode, PrimaryWindow, VideoModeSelection, WindowMode};
use trp_sim::base::bindings::{code, name as key_name, Chosen, Controls, Listening, PressType, Source};

use crate::player::controller::Controller;
use crate::screens::inventory_view::{Selected, OPTIONS};
use crate::platform::keys::{self, Bindings};
use crate::audio::ui_sound::{Sound, Ui};
use crate::weapons::weapon::{HandsRig, Stowed};
use crate::Options;

mod values;
mod features;
mod slides;
mod options_state;
mod saving;
mod switches;
mod fields;
mod look;
mod widgets;
mod page_frame;
mod page_features;
mod page_graphics;
mod page_skills;
mod page_controls;
mod rebuild_clicks;
mod typing;
mod applying;

pub use values::*;
pub use features::*;
pub use slides::*;
pub use options_state::*;
pub use saving::*;
pub use switches::*;
pub use look::*;
pub use applying::*;
use values::{FOV, GAME_FPS_DEFAULT, MSAA};
use slides::{AXIS_WORDS, Cell, Listen, SAID, Slide};
use saving::{NAME_MOST, keep_name, kept_name, load, save};
use switches::{CHARACTER_SECTION, HUD_SWITCHES, SECTIONS};
use fields::{Part};
use look::{AMBER, CATEGORIES, CHOSEN, CHOSEN_LIT, COLUMNS, CONTROLS_PAGE, Click, EDGE, ELITE, FEATURES_PAGE, Feedback, Footer, GRAPHICS_PAGE, GROUND, GROUND_LIT, GROUP_STARTS, HANDLE_LEAST, HEAD, HEAD_LIT, HUD_PAGE, INK, INK_DARK, INK_DIM, INK_FAINT, KHAKI, Lit, Look, MOUSE_CATEGORY, NOTCH, NameField, ON, PAGES, PageArea, PageBar, PageHandle, ROW, ROW_ALT, ROW_LIT, RUST, SKILLS_PAGE, SOUNDS_PAGE, SearchField, SlideBar, SlideFill, SlideValue, Step, Tint, Toggle, WELL};
use widgets::{Row, button, chevron, note_row, row, section_head, section_head_as, setting_row, slide_inks, slider, slider_as, small_button, stepper, stepper_as, tag, tick_box, toggle};
use page_frame::{footer, left_column, page_head};
use page_features::{features_page, hud_page};
use page_graphics::{graphics_page, sound_page, step_choices, step_set};
#[cfg(test)]
use page_graphics::{lod_bias_word};
use page_skills::{skills_page};
use page_controls::{axis_word, cell_name, cell_source, controls_page};
#[cfg(test)]
use page_controls::{entries};
use rebuild_clicks::{clicks, rebuild};
use typing::{clear_cell, listen, lost, reset_page, restart, type_name, type_search};
use applying::{apply, button_over, hover, keep_in_play, live_switches, scroll_page, slide, sync_experience, sync_mastery, sync_skills};

pub struct OptionsPlugin;

impl Plugin for OptionsPlugin {
    fn build(&self, app: &mut App) {
        // With the inventory's own systems: after the simulation's frames.
        app.add_systems(
            Update,
            (clicks, live_switches, type_search, type_name, listen, slide, sync_skills, sync_mastery, sync_experience, keep_in_play, scroll_page, apply, rebuild, hover, button_over)
                .chain()
                .run_if(crate::running)
                .after(crate::player::controller::step)
                // After the screen's own keys: Escape that the tab is about
                // to take is seen there as taken.
                .after(crate::screens::inventory_view::ScreenKeys)
                .before(crate::player::animation::play_clips),
        );
    }
}

// ==== Tests ====

#[cfg(test)]
mod tests;
