//! The battle screen's stance panel, health panel and quick bar, drawn and faded as the game's.
//!
//! The battle screen's panels of the character, as the game's with
//! `TARKOV UI` on: the stance panel in the screen's lower left corner
//! (`BattleStancePanel`: the stance's figure and its slider, the speed's
//! slider and its notches, the legs' and the arms' stamina, the noise) and
//! the health panel in its upper left (`CharacterHealthPanel`: the body's
//! parts coloured by their health, its effects' icons), each fading in as
//! the game's screen raises it and out again (`BattleUIComponentAnimation`),
//! and the quick bar at the top (`InventoryScreenQuickAccessPanel`: each
//! view's thing, the one in the hands lit), by the game's own settings of
//! when they show (`BattleModes`: the install's `Game.ini`, or the options'
//! HUD page). Drawn from the scene as the inventory's panels are
//! (`health_view::draw_nodes`); with `TARKOV UI` off the app's plain meters
//! stay (`hud`). `docs/spec/inventory.md`, "The battle screen's panels".

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use std::collections::HashMap;

use crate::player::controller::Controller;
use crate::screens::health_view::{draw_nodes, effect_icon, FullWidth, HealthLook, Marks, Frame, Piece, PieceFill, SlotMark, Words};
use crate::screens::inventory_view::{quick_thing, Carried, HandsHeld};
use crate::platform::keys::Bindings;
use trp_data::interface::screen::SliderParts;
use trp_sim::condition::health::Part;
use trp_sim::items::inventory::Item;

mod modes_fader;
mod battle_state;
mod drawn_parts;
mod stance;

pub use modes_fader::*;
use modes_fader::{BODY, BattleEffect, BattleRoot, Ladder, Own, QUICK_BAR, QUICK_PRESSES, QuickDrawn, SPAN, STANCE, Seen, SpeedNotch, VISIBILITY_DURATION};
use battle_state::{BattleState, GLOW_RGB, PICKER_LAYER, PickerCell, QuickdrawGlow, glow_alpha, glow_legs, glowing_slots, is_pistol, mount_flash_alpha, noise_level};
use drawn_parts::{draw_battle, draw_quick_bar, fade_battle, follow_health, glow_quick, keep_own, light_picker};
use stance::{follow_stance};

pub struct BattleViewPlugin;

impl Plugin for BattleViewPlugin {
    fn build(&self, app: &mut App) {
        // The quick bar is drawn after the panels' fades have stepped, so
        // that a bar raised on a frame is there on it (it is not kept while
        // faded out: `draw_quick_bar`), and its colours kept before the
        // fade draws it.
        app.init_resource::<BattleState>().add_systems(
            Update,
            (draw_battle, keep_own, follow_stance, follow_health, draw_quick_bar, keep_own, light_picker, glow_quick, fade_battle).chain().after(crate::player::controller::step),
        );
        // The screen's own fade, from the frame the inventory's key shuts
        // that screen.
        app.add_systems(Update, follow_screen.after(crate::player::controller::step).after(crate::screens::inventory_view::ScreenKeys));
    }
}

/// The battle screen shown softly as it comes up (`BattleUIScreen.Show`:
/// `ShowGameObject`, `screen_fade`), the whole screen's group a quarter of
/// a second from 0 to whole, each panel's own fade under it; gone at once
/// as it shuts (`courses10`'s `inventory-open-close`: from 0.0337 on the
/// inventory's shutting key to whole 0.25 s on, seen on its `ActionPanel`).
/// Up as the panels have it (`follow_stance`).
fn follow_screen(controller: Res<Controller>, time: Res<Time>, mut fades: ResMut<crate::ui::screen_fade::ScreenFades>) {
    let alive = controller.health.as_ref().is_none_or(|health| health.alive);
    let up = !controller.inventory_open && !controller.edit_build && alive;
    fades.battle.follow(up, time.delta_secs());
}

#[cfg(test)]
mod tests;
