//! The weapon in hand's wear, malfunctions and heat in the app.
//! After `docs/spec/malfunctions.md`, "In the app": the weapon's logic is given its item's
//! durability and malfunction state as it is taken in hand (`Firearm::set_wear`), is told each
//! frame the round it would fire next, its magazine, the skills and the arms, and its wear and
//! state are kept back on the item; what it says of its malfunctions is told to the shooter as the
//! game tells it (`PlayerOwnerInventoryController`'s `ExamineMalfunction` and the rest): the
//! notices (`NotificationManager`) and the glow (`BattleUIMalfunctionGlow`), with the interface's
//! sound; a failure to feed's round is thrown onto the ground by its repair.
//!
//! The three switches (`DURABILITY`, `MALFUNCTIONS`, `OVERHEATING`) are the
//! logic's (`WearSwitches`): off, the weapon does not wear, never
//! malfunctions, stays cold.

use std::collections::VecDeque;
use std::sync::mpsc::{channel, Receiver};
use std::sync::Mutex;

use bevy::asset::{load_internal_asset, uuid_handle};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, BlendComponent, BlendFactor, BlendOperation, BlendState, RenderPipelineDescriptor};
use bevy::shader::ShaderRef;
use bevy::ui_render::prelude::{UiMaterial, UiMaterialPlugin};
use trp_data::formats::curve::Curve;
use trp_sim::weapons::firearm::Wear;
use trp_sim::items::inventory::{Id, Item, Repairable};
use trp_sim::weapons::malfunction::{self, Malf, RoundFigures, Settings, Skills, WeaponFigures};
use trp_sim::condition::skills::{Skill, ELITE};
use trp_sim::FirearmEvent;

use crate::player::controller::Controller;
use crate::screens::inventory_view::{Carried, HandsHeld};
use crate::weapons::weapon::HandsRig;

mod feedback;
mod notice_looks;
mod notices_drawn;

pub use feedback::*;
pub use notice_looks::*;
use feedback::{keep_wear};
use notice_looks::{GLOW_SHADER, Looks, NOTICE_DONE, NOTICE_FOLD, NOTICE_GROUND, NOTICE_SHOW, NOTICE_WORDS, NoticeBox, NoticeFrame, NoticeIcon, NoticeRoot, NoticeWords, notice_alphas, notice_box, notice_row, start_reading, take_looks};
#[cfg(test)]
use notice_looks::{NOTICE_HIDE, NOTICE_ICON_IN};
use notices_drawn::{show_glow, show_notices};

pub struct WearPlugin;

impl Plugin for WearPlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(app, GLOW_SHADER, "../../shaders/malfunction_glow.wgsl", Shader::from_wgsl);
        app.add_plugins(UiMaterialPlugin::<GlowMaterial>::default())
            .init_resource::<Feedback>()
            .init_resource::<Looks>()
            .add_message::<MalfunctionShown>()
            .add_systems(Startup, start_reading)
            .add_systems(
                Update,
                // After the hands have settled what is in them, before the
                // weapon's clips play (what it is told here it acts on in the
                // next frame, as the magazine's count is).
                keep_wear.after(crate::screens::inventory_view::ScreenDrawn).before(crate::player::animation::play_clips).run_if(crate::running),
            )
            .add_systems(Update, (take_looks, show_notices, show_glow).chain().in_set(NoticesShown).after(keep_wear));
    }
}

#[cfg(test)]
mod tests;
