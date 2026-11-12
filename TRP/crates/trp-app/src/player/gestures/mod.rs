//! The hand signs and the voice's phrases.
//! After `docs/spec/gestures.md`; the options' `GESTURES`: the menu Y twice opens (`GesturesMenu`),
//! the quick prompt a tap of Y says ("BROKEN LEG [Y]", `GesturesQuickPanel`), the list Y held opens
//! (`GesturesDropdownPanel`), and the function keys' binds (`GesturesBindPanel`), F1 saying the
//! mumble until changed.
//!
//! A hand sign goes to the body's gestures (`Movement::set_interaction`,
//! `Player.ShowGesture`); a phrase to the voice (`voice_sound::PhraseAsked`,
//! `Player.Say`). The menu takes every command but its own key and the
//! inventory's while it is open (`Bindings::only`), frees the cursor and
//! blurs the world (`Controller::gestures_open`). The binds are kept in the
//! options' file (`KEPT_AS`). TRP's choices: all eleven
//! hand signs shown, not only the profile's seven; the menu drawn from the
//! install's layout, sprites and colours.

use std::collections::{HashMap, HashSet};

use bevy::input::mouse::AccumulatedMouseScroll;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use trp_sim::held::interactions::Interaction as Sign;

use crate::player::controller::Controller;
use crate::audio::voice_sound::{PhraseAsked, PhraseTold, Voiced};

mod binds;
mod pictures;
mod listen_layout;
mod menu_drawn;

pub use binds::*;
use binds::{BindMenu, Pick, is_sign};
use pictures::{Pictures, play, situations, start_reading, take_pictures};
use listen_layout::{Area, Frame, bind_lines, layout, listen};
use menu_drawn::{draw_menu, draw_prompt};
#[cfg(test)]
use menu_drawn::{blink_alpha};

/// The voice's phrases by the game's numbers and names (`EPhraseTrigger`),
/// the names the locale keys their words by.
pub mod phrase;

/// The menu's groups of phrases, in its order (`GesturesMenu.
/// InitPhraseGroups`): the group's name (its own word in the locale), whether
/// it is a situational one (always open, unnamed), and its phrases.
const GROUPS: [(&str, bool, &[i32]); 9] = [
    ("COMMAND", false, &[41, 42, 43, 44, 45, 46, 47, 36, 35, 34, 33, 32, 31, 30, 37, 38, 39, 40]),
    ("REACTION", false, &[64, 65, 66, 67, 68, 63, 59, 62, 61, 60, 58, 57]),
    ("HEALTH STATUS", false, &[51, 52, 53, 54, 55, 56, 50, 49, 48]),
    ("CONTACT", false, &[72, 73, 74, 71, 70, 69]),
    ("ENEMY", false, &[78, 79, 80, 77, 76, 75]),
    ("HELP", false, &[85, 86, 87, 88, 89, 90]),
    ("TEAM STATUS", false, &[83, 84, 82, 81]),
    ("MUMBLE", true, &[phrase::MUMBLE]),
    ("SITUATIONAL", true, &[92, 97, 98, 96, 94, 95, 93, 103, 102]),
];

/// Where each group stands, from the screen's centre, y up (`AudioGroupPanel`).
const GROUP_AT: [[f32; 2]; 9] = [[-500.0, 0.0], [0.0, 315.0], [-380.0, 210.0], [380.0, 210.0], [500.0, 0.0], [-380.0, -210.0], [380.0, -210.0], [0.0, 150.0], [0.0, -100.0]];

/// Where a common group's shown phrases stand, from its centre (`SubItems`).
const PHRASE_AT: [[f32; 2]; 24] = [
    [-70.0, 9.0],
    [70.0, 9.0],
    [-70.0, -9.0],
    [70.0, -9.0],
    [-70.0, 27.0],
    [70.0, 27.0],
    [-70.0, -27.0],
    [70.0, -27.0],
    [-60.0, 45.0],
    [60.0, 45.0],
    [-60.0, -45.0],
    [60.0, -45.0],
    [-35.0, 63.0],
    [35.0, 63.0],
    [-35.0, -63.0],
    [35.0, -63.0],
    [-10.0, 81.0],
    [10.0, 81.0],
    [-10.0, -81.0],
    [10.0, -81.0],
    [0.0, 99.0],
    [0.0, 117.0],
    [0.0, -99.0],
    [0.0, -117.0],
];

/// Where each hand sign's hexagon's top centre stands (`GesturesGroupPanel`).
const SIGN_AT: [[f32; 2]; 11] = [[-42.0, 138.0], [42.0, 138.0], [0.0, 66.0], [-83.0, 66.0], [83.0, 66.0], [-125.0, -5.0], [125.0, -5.0], [-82.0, -80.0], [0.0, -80.0], [82.0, -80.0], [0.0, 210.0]];

/// Each hand sign's icon (`StaticIcons.GestureSprites`), in `Sign::GESTURES`' order.
const ICONS: [&str; 11] = [
    "gesture_that_way",
    "gesture_stop",
    "gesture_hi",
    "gesture_fuckoff",
    "gesture_ok",
    "gesture_bad",
    "gesture_follow",
    "gesture_PlayStone",
    "gesture_PlayScissors",
    "gesture_PlayPaper",
    "gesture_Okay",
];

/// The menu's own sprites (`sharedassets44.assets`).
const SPRITES: [&str; 9] = ["small_group_back", "small_group_back_selected", "subgroup_back", "subgroup_back_selected2", "group_back", "group_selected", "bind_label_botleft", "bind_label_botright", "gesture_down_icon"];
const SMALL_GROUP: usize = 0;
const SMALL_GROUP_SELECTED: usize = 1;
const SUBGROUP: usize = 2;
const SUBGROUP_SELECTED: usize = 3;
const GROUP_BACK: usize = 4;
const GROUP_GLOW: usize = 5;
const TAB_LEFT: usize = 6;
const TAB_RIGHT: usize = 7;
const DOWN_ICON: usize = 8;

/// The menu's colours (`GesturesMenu._colorMap`).
const AUDIO: Color = Color::srgba(0.0, 0.0, 0.0, 1.0);
const AUDIO_SELECTED: Color = Color::srgba(0.694, 0.737, 0.753, 1.0);
const GESTURE: Color = Color::srgba(0.694, 0.624, 0.404, 0.784);
const GESTURE_SELECTED: Color = Color::srgba(0.860, 0.810, 0.652, 0.784);
const TEXT_AUDIO: Color = Color::srgba(0.714, 0.757, 0.780, 1.0);
const TEXT_AUDIO_SELECTED: Color = Color::srgba(0.0, 0.0, 0.0, 1.0);
const TEXT_GESTURE: Color = Color::srgba(0.839, 0.890, 0.914, 1.0);
const TEXT_GESTURE_SELECTED: Color = Color::srgba(0.0, 0.0, 0.0, 1.0);
/// A group's name (`CommonGroupTemplate/Background/Text`).
const GROUP_NAME: Color = Color::srgba(0.796, 0.796, 0.757, 1.0);
/// A key tab's words.
const TAB_WORDS: Color = Color::srgba(0.196, 0.196, 0.196, 1.0);
/// The prompt's ground and the list's rows (`BindPanel`, `GesturesQuickPanelItem`).
const PROMPT_GROUND: Color = Color::srgba(0.0, 0.0, 0.0, 0.941);
const ROW_CHOSEN: Color = Color::srgba(0.714, 0.757, 0.780, 0.784);
const ROW_WORDS_CHOSEN: Color = Color::srgba(0.0, 0.0, 0.0, 0.961);
/// The binding menu (`GesturesContextMenu`).
const MENU_BUTTON: Color = Color::srgba(0.169, 0.176, 0.180, 1.0);
const MENU_GROUND: Color = Color::srgba(0.0, 0.0, 0.0, 0.791);

/// A dimmed item's alpha (`SetUnlockStatus`).
const DIMMED: f32 = 0.3;
/// The wait before the held list opens (`MumbleDropdown`), and after it
/// shuts before a tap says the prompt's phrase again.
const LIST_WAIT: f64 = 0.2;
const QUICK_BAR: f64 = 0.3;
/// A situation's life (`PhraseSituation(p, 5)`).
const SITUATION_LIFE: f64 = 5.0;

/// Where the binds are kept in the options' file.
pub const KEPT_AS: &str = "gesture_binds";

pub struct GesturesPlugin;

impl Plugin for GesturesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Gestures>().init_resource::<Pictures>().add_systems(Startup, start_reading).add_systems(
            Update,
            (take_pictures, situations, listen, draw_prompt, draw_menu)
                .chain()
                .after(crate::platform::keys::hear)
                .before(crate::player::controller::gather_input)
                .run_if(crate::running)
                .run_if(|features: Res<crate::screens::options_view::Features>| features.gestures),
        );
    }
}

#[cfg(test)]
mod tests;
