//! Tactical devices in the app: their keys, their lights and their lasers.
//!
//! Tactical devices in the app (tactical-devices spec): the game's keys for
//! them in either activation mode, put to the weapon's firearm and the
//! head's devices (`trp_sim::tactical`); each device's model shown as its
//! state has it (`TacticalComboVisualController`); its lights as the
//! engine's, placed where the game's are each frame (the weapon's on its
//! model, the head's by the camera: `HeadLightsValidator`); and what the
//! HUD's panel of the activation mode is told (`sight_panels`). The lasers
//! are drawn by `muzzle_view`'s pass; the flares are `flare_view`'s (one's
//! own shows only from outside: in first person the camera is behind it);
//! the light shafts are not built (the recorded game had them off in its settings). The
//! dummies' weapons' devices are shown and lit here too
//! (`show_dummy_devices`), and so are those of the weapons hung on the body
//! (`show_slung_devices`: a weapon put away keeps its light, the game
//! switching only its model's renderers off for its own character), and
//! those of the weapon in the OVERALL tab's own figure's hands
//! (`show_figure_devices`: its light seen by the figure's camera alone). Seen
//! from outside, the head's lights and lasers are on their objects on the
//! model of what is worn, as another player's are: the game moves them to
//! the camera for its own player's eyes alone (`GamePlayerOwner.
//! HeadLightsValidator`).

use std::collections::HashMap;
use std::sync::Arc;

use bevy::asset::RenderAssetUsages;
use bevy::ecs::query::QueryItem;
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::light::{NotShadowCaster, SpotLightTexture};
use bevy::prelude::*;
use bevy::render::extract_component::{ExtractComponent, ExtractComponentPlugin};
use bevy::render::render_resource::{Extent3d, ShaderType, TextureDimension, TextureFormat};
use bevy::transform::TransformSystems;
use trp_data::weapons::tactical::{DeviceLaser, Volume};
use trp_data::weapons::tactical::{Device, DeviceLight};
use trp_sim::items::inventory::{Id, Item, Light};
use trp_sim::weapons::tactical::{self as tac, InputMode};

use crate::player::controller::Controller;
use crate::screens::inventory_view::{Carried, HandsHeld};
use crate::weapons::weapon::{HandsRig, WeaponRig};

mod laser_lamps;
mod devices_shown;
mod aiming;

pub use laser_lamps::*;
pub(crate) use aiming::*;
use laser_lamps::{LaserKey, lit, show_modes, spawn_lamp, spawn_lamp_for, tactical_keys};
use devices_shown::{show_devices, show_dummy_devices, show_figure_devices, show_slung_devices};
use aiming::{aim_lasers, place_lamps};

/// The devices' systems; `on` is the options' `TACTICAL DEVICES` as the run
/// began. Off, the keys do nothing and no light is made, but every device's
/// model still shows its off object, as an item made with its device off.
pub struct TacticalViewPlugin {
    pub on: bool,
    /// The range lit by its own light (`CUSTOMS LIGHT` off): its camera at
    /// Bevy's own exposure.
    pub own_light: bool,
}

/// What a light worked out for the game's light's exposure (EV100 0,
/// `lighting::EXPOSURE_EV100`) is multiplied by for the camera it is drawn
/// by: 1 with Customs' light; with the range's own, whose camera keeps
/// Bevy's own exposure (EV100 9.7), 2 to the difference, so that a lit
/// surface takes what the game's would.
pub(crate) fn exposure_gain(own_light: bool) -> f32 {
    if own_light {
        2f32.powf(bevy::camera::Exposure::EV100_BLENDER - crate::rendering::lighting::EXPOSURE_EV100)
    } else {
        1.0
    }
}

impl Plugin for TacticalViewPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Tactical { on: self.on, exposure_gain: exposure_gain(self.own_light), ..default() })
            .add_systems(Update, tactical_keys.after(crate::platform::keys::hear).before(crate::player::controller::step))
            .add_systems(Update, (show_devices, show_dummy_devices, show_slung_devices, show_figure_devices).chain().after(tactical_keys))
            .add_systems(PostUpdate, (place_lamps, aim_lasers).chain().after(TransformSystems::Propagate))

            .add_plugins(ExtractComponentPlugin::<LaserScreen>::default());
    }
}

/// A device's light as the engine draws it, which the frame places.
#[derive(Component)]
struct TacticalLamp;

/// A light of a device as the engine draws it: its entity, and which
/// device's which light it is (the weapon's by the rig's node, the head's
/// by the side of the camera it was found on).
struct Lamp {
    entity: Entity,
    device: usize,
    light: usize,
    /// The weapon's: the light's object on the rig.
    node: Option<Entity>,
    /// The head's: on the camera's right (`HeadLightsValidator.IsOnRight`),
    /// and its turn as last placed (the game moves it along its turn of the
    /// frame before).
    right: bool,
    turned: [f32; 4],
}

/// The devices' state in the app.
#[derive(Resource, Default)]
pub struct Tactical {
    pub on: bool,
    /// The keys, Hold mode's let-go and the head light's coroutine and lock.
    devices: tac::Devices,
    /// The changes of the hands begun as last counted.
    changes: u32,
    /// The camera (place and turn in the game's axes, field of view) as the
    /// last frame left it, and the one the head's devices go by this frame.
    head_camera: Option<([f32; 3], [f32; 4], f32)>,
    head_camera_used: Option<([f32; 3], [f32; 4], f32)>,
    /// The head's devices as their models and lights show them: switched
    /// only at the coroutine's 0.8 s (their items' state is written at the
    /// press), at once when what is worn changes.
    head_shown: HashMap<Id, Light>,
    /// The head's devices as last found (item and template), to tell a
    /// change of what is worn.
    head_found: Vec<(Id, String)>,
    /// Each head device's figures, read from its own prefab when first
    /// worn, by template (none where it could not be read).
    head_data: HashMap<String, Option<Arc<Device>>>,
    /// Each head device's side of the camera, as first found.
    head_side: HashMap<Id, bool>,
    /// The activation mode switched this frame by T+B, for the HUD's panel.
    pub mode_switched: Option<InputMode>,
    /// The lights of the rig in hand (its root), and the head's.
    weapon_lamps: Option<((Entity, usize), Vec<Lamp>)>,
    head_lamps: Vec<(Id, Lamp)>,
    /// The dummies' weapons' lights, by each one's model (its weapon's first
    /// entity).
    dummy_lamps: Vec<(Entity, Vec<Lamp>)>,
    /// The lights of the weapons hung on the body, by each one's model (its
    /// root as hung: `slung_view`).
    slung_lamps: Vec<(Entity, Vec<Lamp>)>,
    /// The lights of the weapon in the OVERALL figure's hands, by its model
    /// (its root as made at a showing: `figure_view`).
    figure_lamps: Option<(Entity, Vec<Lamp>)>,
    /// The lights' cookies, by the picture's name.
    cookies: HashMap<String, Option<Handle<Image>>>,
    /// The lights' share for the camera's exposure (`exposure_gain`).
    exposure_gain: f32,
    /// The lasers' pictures and noises, by their names.
    pictures: HashMap<String, Option<Handle<Image>>>,
    /// Each laser's dot's light (`LaserBeam`'s own: "laserBeamLight"), by the
    /// laser: the weapon's by device and laser, the head's by item.
    dot_lights: HashMap<LaserKey, Entity>,
    /// The head's lasers' turns as last placed (as its lights').
    head_laser_turns: HashMap<LaserKey, [f32; 4]>,
}

impl Tactical {
    /// The head's devices as their models and lights show them: each one's
    /// template, its state as shown, and its figures (those read).
    pub(crate) fn head_devices_shown(&self) -> impl Iterator<Item = (&str, Light, &Device)> + '_ {
        self.head_found.iter().filter_map(|(id, template)| {
            let device = self.head_data.get(template)?.as_deref()?;
            Some((template.as_str(), self.head_shown.get(id).copied().unwrap_or_default(), device))
        })
    }
}

use trp_sim::weapons::tactical::{head_pose, turn};


