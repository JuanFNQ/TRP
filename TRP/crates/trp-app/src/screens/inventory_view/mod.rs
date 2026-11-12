//! The inventory screen: its tabs, the gear's slots, the containers' grids and what is open beside them.
//!
//! The inventory screen (`docs/spec/inventory.md`, "The screen and its
//! tabs"): the game's row of tabs in the game's order, and under Gear the
//! equipment's slots laid over a figure of the character, each container's
//! grids, and whatever is open beside them: the crate on the range.
//!
//! Things are moved with the mouse, by the model's own moves
//! (`trp_sim::inventory::move_item`, `apply_item`): held and let go on a
//! cell, on a slot, or on another item. What the equipment then holds is
//! what the character carries, wears and has in hand (`settle`).
//!
//! And without dragging, as the game's item view takes a click
//! (`docs/spec/inventory.md`, "Clicks, keys and windows"): Ctrl and a
//! click sends a thing to the other side, Alt and a click puts it on, a
//! double click opens a container in a window of its own or shows what a
//! thing is; Ctrl held as a stack is let go asks how many in a window;
//! a quick-use key let go over a thing binds it to that key, and the bar
//! under the screen shows what each key has.
//!
//! It is drawn plainly: boxes and words. The figure and where the slots
//! sit on it are the app's own drawing in the order of the game's panel;
//! the game's prefab for the screen is not read, and none of its art is
//! used.

use std::collections::HashMap;

use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;
use bevy::window::PrimaryWindow;
use bevy::asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use trp_sim::held::things::{Action, Menu};
use trp_sim::{Acted, FirearmEvent, Interaction as Gesture, State, WorldInteraction};
use trp_sim::items::inventory::{
    apply_item, can_pick_up, equip, go_together, is, move_item, pick_up, quick_find, split_item, split_most, take_out, transfer_item, transfer_most, Applied, Bound,
    Equipment, Id, Item, Order, Place, Refusal, Rules, Spot, HAND_SLOTS,
};

use crate::player::controller::Controller;
use crate::audio::ui_sound::{Sound, Ui, Way};
use crate::platform::keys::Bindings;
use crate::world::range::Range;
use crate::weapons::weapon::{HandsRig, Stowed, WeaponRig};
use crate::{GameData, Options, View};

mod carried;
mod magazine_work;
mod windows_open;
mod asking;
mod loot_panel;
mod layout;
mod components;
mod acting;
mod item_view;
mod game_item_view;
mod spinners;
mod blinking;
mod quick_bar;
mod window_frames;
mod info_parts;
mod small_windows;
mod gear;
mod tabs;
mod scrolling;
mod rebuilding;
mod pointing;
mod using;
mod context_menu;
mod dragging;
mod window_moves;
mod hands;
mod hand_keys;
mod hand_takes;
mod settling;
mod reach;
mod toggle;

pub use carried::*;
pub use magazine_work::*;
pub use windows_open::*;
pub use asking::*;
pub use loot_panel::*;
pub use layout::*;
pub use components::*;
pub(crate) use acting::*;
pub(crate) use item_view::*;
pub(crate) use game_item_view::*;
pub(crate) use spinners::*;
pub(crate) use quick_bar::*;
pub(crate) use info_parts::*;
pub use small_windows::*;
pub(crate) use tabs::*;
pub use scrolling::*;
pub(crate) use rebuilding::*;
pub(crate) use pointing::*;
pub use window_moves::*;
pub use hand_keys::*;
pub use reach::*;
use magazine_work::{brief_name, name_slot_keys, work_magazine};
use windows_open::{Loading, WINDOWS_OVER_EDIT_BUILD, tag_letter, window_z};
use asking::{Asked, ask, tint_asking};
use loot_panel::{GRID_GAP, LOOT_PANEL_TOP, LooseItem, LootArea, LootThumb, REPAIR_KITS_CLASS, SCROLL_BAR, body_list, body_panel, carries, carries_class, row_search, row_unsearched_size, scroll_loot, search_body_at_start, search_words};
use layout::{CAPTION, CONTAINERS, EQUIPMENT, FIGURE, GOES_WITH, LOOSE_REACH, MAGAZINE_DROP, POINTED, SHAPE, SLOT, SLOT_SIDE, STASH_WIDTH, TAB_BAR, THING, Units, hide_plain_tabs, tab_open, tabs_hidden};
use components::{ASKING_LAYER, AcceptLine, Accepting, AskAccept, AskBar, AskClose, AskPlaces, AskPress, BAR_LAYER, Clicked, Clicks, DOUBLE_CLICK, Drag, DropMark, DropShown, EMPTY_LINE_OUT, GameBack, GameTab, GameTabBar, GameTabLook, GearPage, Ghost, GhostPicture, GhostWords, GridBox, Held, ItemView, Overlay, PART_LINE_OUT, Page, Pressed, PromptLine, Screen, SlotBox, SlotLook, TabButton, Target, ValueProxy, WINDOW_LAYER, WinCaption, WinClose, WindowPress, check_accept, drop_colour, grid_location, show_drop, slot_colour};
#[cfg(test)]
use components::{accepting_slots};
use acting::{act, in_slot, most_of, refusal_tip, spawn};
use item_view::{PICTURE_SHARE, SPECIAL_SIDE, VALUE_BAR, ValueLayout, icon_of, in_special_slot, item_value, make_icons, menu_texts, shown_buff};
use spinners::{AppSpinBar, DrainSpinner, ExamineFill, ExamineLabel, ExamineLoader, GroundApart, ItemPicture, SPINNER_PIXELS, SPINNER_TURNS, SearchSpinner, SpinFill, SpinPiece, SpinRole, SpinnerPicture, UNKNOWN_ALPHA, UNTAKEN_ALPHA, UnknownGround, UnknownPicture, UnknownView, Unusable, ViewInside, picture_in, picture_node, spinner_node, tint_unknown, tint_unusable, turn_examination, turn_search_loaders, turn_spinners, within_line};
use blinking::{blink, dim_taken, empty_line, fill_pictures, grid_box, grid_box_drawn, keep_leaving, slot_box, slot_box_drawn};
#[cfg(test)]
use blinking::{BlinkTween, EXAMINED_BLINK_FROM};
use quick_bar::{HELP, QUICK_KEYS, QUICK_PANEL, quick_bar, quick_picture_node};
use window_frames::{fit_windows, stretch_windows, window_box};
#[cfg(test)]
use window_frames::{regrown};
use info_parts::{InfoLockTip, InfoTip, OUT_OF_REACH, WinStretch, grid_built, info_button_tips, info_lock_tips, info_tips, special_slots, unknown_figures};
#[cfg(test)]
use info_parts::{unchecked_slot};
use small_windows::{asking_box, make_stash_fills, open_tag, repair_window_box, tag_window_box, tag_windows};
use gear::{gear_page};
use tabs::{choose_tab, follow_opening, game_tabs, screen_sounds, tab_sounds};
use scrolling::{COLUMN, CRATE, InfoScrollArea, InfoScrollBar, ScrollBarNode, ScrollContent, ScrollHandle, ScrollWheel, scroll_info_texts, scroll_views};
#[cfg(test)]
use scrolling::{magnified_rows};
use rebuilding::{dragged_to, inside, rebuild, scripted_drags, stepped};
use pointing::{ITEM_TIP_DELAY, foresee, item_tooltip, item_under, light_related};
use using::{Using, fit_magazine, menu_lines, open_view, put_down, unload_weapon, use_thing};
use context_menu::{Went, click, do_line, game_window_at, move_sound};
use dragging::{drag};
use window_moves::{on_screen, publish_drag, top_grenade, windows};
use hands::{ChambersHeld, Hands, SLOT_KEYS, chamber_after_shots, first_pose, fitted_magazine, fold_as_item, item_chambers, quick_keys, quick_use_keys, round_over_chamber};
use hand_keys::{chamber_by_hand, switch_followed};
use hand_takes::{HandSlots, MagazineHeld, change_hands, empty_hands_in, hands_asked, hands_first, item_into_hands, take_into_empty, weapon_into_hands};
use settling::{settle};
use reach::{screen_shut, weigh_carried};
use toggle::{MENU_DARK, MENU_DIM, MENU_LIGHT, NEVER_SHOWN, NO_SPACE, show_ground, toggle};

// ==== The plugin, the screen drawn and the carried model ====

pub struct InventoryPlugin;

impl Plugin for InventoryPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Selected(GEAR))
            .insert_resource(Units(0.0))
            .init_resource::<Drag>()
            .init_resource::<Clicks>()
            .init_resource::<AskedFor>()
            .init_resource::<Hands>()
            .init_resource::<Reach>()
            .init_resource::<LootScroll>()
            .init_resource::<GameScrolls>()
            .init_resource::<StashFills>()
            .init_resource::<LootInSight>()
            .init_resource::<MenuTexts>()
            .init_resource::<PicturesArrived>()
            .init_resource::<Blinking>()
            .init_resource::<HandsHeld>()
            .init_resource::<Dragging>()
            .init_resource::<Pointed>()
            .init_resource::<DropShown>()
            .init_resource::<Accepting>()
            .init_resource::<Opening>()
            .add_systems(Update, publish_drag.after(ScreenClicks))
            .add_systems(Update, (check_accept, show_drop).chain().after(ScreenClicks).run_if(crate::running))
            .add_systems(Update, (info_tips, info_button_tips, info_lock_tips).after(rebuild).run_if(crate::running))
            .add_systems(Update, tint_unusable.after(rebuild))
            .add_systems(Update, (tint_unknown.after(rebuild).after(tint_unusable), turn_examination.after(rebuild)))
            .add_systems(Update, work_magazine.after(ScreenClicks).before(rebuild).run_if(crate::running))
            .add_systems(Update, tag_windows.after(ScreenClicks).before(rebuild).run_if(crate::running))
            .add_systems(Update, (screen_sounds, tab_sounds).after(toggle).run_if(crate::running))
            .add_systems(Update, search_body_at_start.after(toggle).before(rebuild).run_if(crate::running))
            .add_systems(Update, goggles_key.after(crate::platform::keys::hear).before(crate::player::controller::step).run_if(crate::running))
            .add_systems(Update, unload_keys.after(crate::platform::keys::hear).before(crate::player::controller::step).run_if(crate::running))
            .add_systems(Update, launcher_keys.after(crate::platform::keys::hear).before(crate::player::controller::step).run_if(crate::running))
            // The screen's clicks and the quick-use keys before the
            // simulation's frames, and from the empty hands what they ask
            // taken in hand there (`hands_first`): the game makes those
            // hands within the click's or the key's own call, before that
            // frame's health and hands (health spec, "The hands"). The
            // screen is opened and shut after the frames (`toggle`), as
            // before; what its clicks were at is let go there on the frame
            // it shuts (`screen_shut`).
            .add_systems(
                Update,
                (scripted_drags, ask, (stretch_windows, windows).chain(), drag.in_set(ClicksHeard), quick_use_keys, hands_first)
                    .chain()
                    .after(crate::player::controller::gather_input)
                    .after(goggles_key)
                    .after(unload_keys)
                    .after(launcher_keys)
                    .before(crate::player::controller::step)
                    .run_if(crate::running),
            )
            .add_systems(Startup, (spawn, make_icons, menu_texts))
            // After the simulation's frames and before the weapon's clips:
            // a screen opened in one frame has the keys from the next, as
            // the game's has, and what the weapon is told here plays then.
            // None of it before the run has begun (`crate::Warmup`): until
            // then the weapon is in the picture whether the character
            // begins with it or not. What is worn is shown from the first
            // frame all the same, to be built with the rest.
            .add_systems(
                Update,
                (
                    // (The panel's scrolling goes by where the mouse is
                    // over the page as it stood, so before the page is
                    // drawn afresh.)
                    // (What is looked at is cast after the keys have done
                    // the last cast's chosen line, as the game's cast in
                    // `LateUpdate` after its keys in `Update`: the prompt
                    // gone on the key's frame, a thing taken up gone from it
                    // on its own: `courses10`'s `examine-world-loot`, 326.)
                    ((toggle.in_set(ScreenKeys), follow_opening).chain(), reach, choose_tab, screen_shut.in_set(ScreenClicks), quick_keys, scroll_loot, (scroll_views, scroll_info_texts), make_stash_fills, (name_slot_keys, fit_windows.in_set(WindowsFitted)).chain(), (rebuild.in_set(PageDrawn), tint_asking, crate::screens::split_view::blink_caret).chain(), (game_tabs, hide_plain_tabs), (fill_pictures, turn_search_loaders), (light_related, item_tooltip).chain(), foresee, (settle.in_set(Settled), keep_leaving, weigh_carried).chain(), (blink, turn_spinners, dim_taken).chain(), show_ground).chain().run_if(crate::running),
                    crate::player::body_view::show_worn,
                    crate::player::body_view::fit_head_parts,
                    crate::player::body_view::show_head_parts,
                    crate::player::body_view::fit_head_things,
                    crate::player::body_view::fit_armbands,
                    crate::player::body_view::turn_hinges,
                )
                    .chain()
                    .in_set(ScreenDrawn)
                    .after(crate::player::controller::step)
                    .before(crate::player::animation::play_clips),
            );
    }
}

/// Where the screen is drawn afresh: what fills its game's panels each
/// frame comes after, so that a page built on a frame is filled on it
/// (else it shows the scene's own colours for a frame: the parts' shapes
/// red).
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ScreenDrawn;

/// Where what the hands and the gear have is settled after the screen's
/// moves (`settle`): a thing first worn is read after it (`worn_reads`).
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct Settled;

/// Where the screen's clicks are heard (`drag`), before the controller's
/// step; what lights a line of the menu of a right click comes before it
/// (`item_menu_view::hover`). What goes by the clicks' outcome goes after
/// `ScreenClicks`, in the screen's own place after the step.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ClicksHeard;

/// Where the windows over the screen are placed by their sizes
/// (`fit_windows`): a search that shows a window's grids comes before, so
/// that the window grown keeps its middle on the frame it grows.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct WindowsFitted;

/// Where the screen's own keys are heard (the inventory's, Escape): what
/// else answers to one of them on the screen comes after, so that a key
/// it is about to take is seen here as still its own.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ScreenKeys;

/// The screen's clicks heard (`drag`), and the page drawn afresh
/// (`rebuild`): what a click asks of the inventory's model comes between,
/// so that the page shows it on the click's frame (an examine begun).
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ScreenClicks;
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct PageDrawn;

pub fn words(text: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (Text::new(text), TextFont { font_size: size, ..default() }, TextColor(color))
}

fn placed(left: f32, top: f32, width: f32, height: f32) -> Node {
    Node { position_type: PositionType::Absolute, left: Val::Px(left), top: Val::Px(top), width: Val::Px(width), height: Val::Px(height), ..default() }
}

// ==== Tests ====

#[cfg(test)]
mod spin_tests {
    use super::Shown;

    /// The load component's shares (`ItemViewLoadAmmoComponent.
    /// StartAnimation`): the work's done by the time since it began over
    /// its rounds' time, the present round's by its remainder; rounds done
    /// before it began count to both.
    #[test]
    fn the_spinner_fills_by_the_work_and_the_loader_turns_by_the_round() {
        let shown = Shown { round: 0.3, to_go: 30, done: 0, since: 4.5 };
        let (done, round) = shown.shares();
        assert!((done - 0.5).abs() < 1e-6);
        assert!(round.abs() < 1e-4 || (round - 1.0).abs() < 1e-4, "{round}");
        let (_, round) = Shown { since: 0.15, ..shown }.shares();
        assert!((round - 0.5).abs() < 1e-5);
        // An unload begun with 10 out: 10 of 40 rounds' time already.
        let (done, _) = Shown { round: 0.3, to_go: 30, done: 10, since: 0.0 }.shares();
        assert!((done - 0.25).abs() < 1e-6);
        // A check: one round of its whole time.
        let (done, _) = Shown { round: 3.0, to_go: 1, done: 0, since: 4.0 }.shares();
        assert_eq!(done, 1.0);
    }
}

#[cfg(test)]
pub(crate) mod tests;

/// Where the mouse is over a box, as a share of it from its top left
/// corner (0 to 1 each way): Bevy 0.17 measures it from the box's middle
/// (-0.5 to 0.5).
pub fn share_of(at: &RelativeCursorPosition) -> Option<Vec2> {
    at.normalized.map(|middle| middle + Vec2::splat(0.5))
}
