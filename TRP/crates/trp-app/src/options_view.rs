//! The options tab, the last of the inventory screen's tabs
//! (`docs/spec/settings.md`): the keys, the display, and a list of the
//! systems a host may come to switch on and off.
//!
//! The game keeps its settings on a screen of its menu; the user asked
//! for them here (2026-10-12). What the tab holds is the game's where the
//! game has it: the bindings are the install's, shown with the game's own
//! words for them and changed by the game's own rules
//! (`trp_sim::bindings`), and the display's settings are the four of the
//! game's the user named. How the tab looks is the app's.
//!
//! What is changed here is kept in the app's own file
//! (`local\options.json`), never in the install.

use std::collections::HashMap;
use std::path::PathBuf;

use bevy::app::AppExit;
use bevy::input::mouse::AccumulatedMouseScroll;
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;
use bevy::window::{Monitor, MonitorSelection, PresentMode, PrimaryWindow, VideoModeSelection, WindowMode};
use trp_sim::bindings::{code, name as key_name, Chosen, Controls, Listening, PressType, Source};

use crate::controller::Controller;
use crate::inventory_view::{words, Selected, BACK, FAINT, LINE, OPTIONS, PANEL, WORDS};
use crate::keys::{self, Bindings};
use crate::weapon::{HandsRig, Stowed};
use crate::Options;

// ==== The options' values: mode, graphics, mouse, the features' switches ====

/// How the window is shown, as the game's display setting has it
/// (`FullScreenMode`): the screen taken over, a borderless window over
/// all of it, or a window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Fullscreen,
    Borderless,
    Windowed,
}

impl Mode {
    const ALL: [Self; 3] = [Self::Fullscreen, Self::Borderless, Self::Windowed];

    fn word(self) -> &'static str {
        match self {
            Self::Fullscreen => "FULLSCREEN",
            Self::Borderless => "BORDERLESS",
            Self::Windowed => "WINDOWED",
        }
    }

    /// The game's own figure for a mode in its settings file.
    fn of_game(figure: i64) -> Self {
        match figure {
            0 => Self::Fullscreen,
            2 => Self::Windowed,
            _ => Self::Borderless,
        }
    }
}

/// The display's settings as they stand: whether frames wait for the
/// screen, the game's field of view (vertical degrees, 50 to 75), how the
/// window is shown and how many pixels it has.
#[derive(Debug, Clone, PartialEq)]
pub struct Graphics {
    pub vsync: bool,
    pub field_of_view: f32,
    pub mode: Mode,
    pub resolution: [u32; 2],
    /// The game's pictures' quality, 0 (low) to 2 (high), and whether they
    /// are streamed by their levels of detail (`streaming`): each taken at
    /// the next start.
    pub texture_quality: u8,
    pub mip_streaming: bool,
    /// The game's shadows' quality, 0 (low) to 3 (ultra): how many
    /// cascades the sun's shadows have and how far they reach (`shadows`),
    /// taken at the next start.
    pub shadow_quality: u8,
    /// The game's ambient occlusion's choice (`Ssao`; `hbao_view`), taken
    /// at the next start.
    pub ssao: trp_sim::hbao::SsaoMode,
    /// The game's clouds' quality (`CloudsQuality`; `clouds_view`), taken
    /// at the next start.
    pub clouds_quality: trp_sim::clouds::CloudsQuality,
    /// Whether frames are rendered on a thread of their own while the next
    /// is worked out (Bevy's pipelined rendering), as the game's engine
    /// renders with its graphics jobs: taken at the next start.
    pub render_thread: bool,
    /// The game's frame rate limit in a raid (`GameFramerate`, within
    /// `GAME_FPS`) and its switch (`DisableGameFramerateLimit`), held with
    /// VSync off as the game holds it (`frame_limit`): taken at once.
    pub game_fps: u32,
    pub fps_limit_off: bool,
    /// The samples of the app's multisampling of the scene, 1 (off), 2 or
    /// 4: the app's own, as the game's deferred camera has none (its
    /// anti-aliasing is FXAA or TAA, not built). Taken at once.
    pub msaa: u8,
    /// The graphics backend (`backend`), taken at the next start.
    pub renderer: crate::backend::Renderer,
}

/// The game's limits for its field-of-view setting (`GameSettingsGroup`).
const FOV: [f32; 2] = [50.0, 75.0];

/// The game's limits for its frame rate limit in a raid
/// (`GraphicsSettingsController.MinFramerateLimit`, `MaxFramerateGameLimit`;
/// this install's server settings, `SPT_Data/database/settings.json`'s
/// `FramerateLimit`, have the same), and the setting's default
/// (`GraphicsSettingsGroup`: 120).
pub const GAME_FPS: [u32; 2] = [30, 144];
const GAME_FPS_DEFAULT: u32 = 120;

/// The choices of the app's multisampling, by their samples.
const MSAA: [(u8, &str); 3] = [(1, "OFF"), (2, "2X"), (4, "4X")];

/// The game's settings of the mouse on its controls tab, as they stand
/// (`ControlSettingsGroup`): the sensitivity from the hip and with the
/// sights up, the sensitivity of a scope's zoom being adjusted (how far a
/// turn of the wheel zooms a variable scope), and the two switches
/// that turn an axis of the mouse about, across and up and down. The
/// double click's time, the fourth of the tab's bars, is the bindings'.
#[derive(Debug, Clone, PartialEq)]
pub struct Mouse {
    pub sensitivity: f32,
    pub aiming: f32,
    pub optic: f32,
    pub invert: [bool; 2],
}

/// What of the features' list is switched (settings spec, "Features"):
/// the app's own switches, none of them the game's, in three lists since
/// 0.101.0 (the user, 2026-10-21: "I want to overhaul the feature list"):
/// `SIMPLE` (what a player sees or feels, each Tarkov's own: a few of them
/// groups of the parts listed under `ADVANCED`), `EXPERIMENTAL` (what
/// departs from Tarkov on purpose) and `ADVANCED` (the systems underneath,
/// one by one, for a host that does the job itself). A part of a group is
/// on only while its group is (`effective`). **Each is "restart
/// required"** (the user, 2026-10-14): as a resource, the switches as the
/// run began, which hold until it ends, so that nothing is made or
/// switched in play; one thrown on the tab is kept for the next run
/// (`OptionsState::features`), and the tab says so.
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct Features {
    /// `CUSTOM FIRST PERSON HANDS` (the user, 2026-10-13): an outfit's
    /// own arms are drawn for the character's own eyes. Off, the hands do
    /// not change with what is worn: they are the install's default arms.
    pub custom_hands: bool,
    /// `SMOOTH FIRST PERSON ANIMATIONS` (the user, 2026-10-14): every
    /// animation of the thing in hand (any weapon, empty hands) is drawn
    /// on curves without corners through its clips' samples. Off, all of
    /// them are drawn as the engine has them, the look all over a weapon
    /// with them. The drawing alone: the simulation is the same either
    /// way.
    pub smooth_animations: bool,
    /// `GROUNDED FEET` (the user, 2026-10-16: "Can i see the float fix
    /// first?"; foot IK tried and taken back): the body is drawn down onto
    /// the ground where the character stands over it within Tarkov's ground
    /// check (after a short drop it rests up to 15 cm over the ground, as
    /// the game's capsule does; `feet_down`). Off, the body is drawn where
    /// the game's capsule holds it, feet and all. The drawing alone: the
    /// simulation, the eye and the first-person arms are the same either
    /// way. A departure from Tarkov's look.
    pub grounded_feet: bool,
    /// `VISUAL EFFECTS` on the `SIMPLE` list (0.101.0): the group of
    /// `MUZZLE FLASH`, `PARTICLES`, `SPENT CASES`, `MAGAZINE ROUNDS` and
    /// `FACE SHIELD MASK`. Off, all five are off.
    pub visual_effects: bool,
    /// `FACE SHIELD MASK` (2026-10-24, 0.118.0; the user put it under
    /// `VISUAL EFFECTS`): a face shield worn down drawn over the first
    /// person's view as the game's camera draws it (`visor_view`; inventory
    /// spec, "The face shield's mask"). Off, nothing of it is read or drawn.
    pub face_shield_mask: bool,
    /// `SPENT CASES` (2026-10-17, night: the user, "Ca we also add bullet
    /// casings while we're at it"): a fired round's case thrown from the
    /// weapon's shell port at its mark, flying and bouncing as the game's
    /// do, gone a second later (`cases`; firing spec, "Spent cases"). Off,
    /// nothing of it is read or drawn.
    pub spent_cases: bool,
    /// `MAGAZINE ROUNDS` (2026-10-17, night: the user, "And also the
    /// bullets on the magazines models"): a magazine's rounds drawn in its
    /// model's points, in the hands, on the ground and in its picture, as
    /// the game draws them (`magazine_rounds`; inventory spec, "A
    /// magazine's rounds drawn"). Off, none is drawn.
    pub magazine_rounds: bool,
    /// `MUZZLE FLASH` (2026-10-18: the user, "Add the particle system so we
    /// can add muzzle flash, and gun smoke"): a shot's flash of the
    /// weapon's jets and the gun smoke's trail, as the game draws them
    /// (`muzzle_view`; firing spec, "The muzzle flash", "The gun smoke").
    /// Off, nothing of them is read or drawn.
    pub muzzle_flash: bool,
    /// `PARTICLES` (2026-10-18: the user, "Go ahead with the particle
    /// system"): Unity's particle systems run as the game's effects use
    /// them: the muzzles' fumes (`particles_view`; particles spec; firing
    /// spec, "The muzzle's fumes"), a launcher's blasts, a grenade's
    /// explosion with its scorch (`explosion_view`, `decal_view`). Off,
    /// nothing of them is read or drawn.
    pub particles: bool,
    /// `CUSTOMS LIGHT` (2026-10-19: the user, "The range I guess give it
    /// factory's daytime", then "Port exposure + tonemap"; of Factory's,
    /// "It's far too dark. Can we do customs instead?", then "Customs' time
    /// of day"): the range lit by Customs' sun, ambient and sky in the
    /// recorded morning, by the game's time-of-day rules, the effects by its
    /// figures, and the camera's exposure, vignette and filmic tonemap
    /// (PRISM), as the game's (`lighting`, `tod_sky_view`, `prism_view`;
    /// lighting spec). Off, the range's own light and Bevy's tonemapper.
    pub customs_light: bool,
    /// `CAMERA COLOR FILTER` (2026-10-19; the user, of the light: "Can we
    /// make the colors a little cooler/brighter?", and of "Jason": "I dont
    /// want to force the user onto an optional setting"): the camera's own
    /// colour filter (`CC_Vintage`) as every level's camera ships it, after
    /// PRISM, with `CUSTOMS LIGHT` (`color_filter_view`; lighting spec, "The
    /// camera's colour filter"). Not the player's optional Post-FX filter.
    /// Off, nothing of it is read or drawn.
    pub camera_color_filter: bool,
    /// `CAMERA SHARPENING` (2026-10-26; the user: "Whole pass, own
    /// switch"): the camera's own sharpening and desaturation (`CC_Sharpen`
    /// with its `DesaturateEffect`) as Customs' camera runs it, with
    /// `CUSTOMS LIGHT` (`sharpen_view`; health spec, "Desaturation"): the
    /// player's `Sharpen`, the weather's grey, and the health's share and a
    /// painkiller's ring where `HEALTH SYSTEM` runs. Off, nothing of it is
    /// read or drawn.
    pub camera_sharpening: bool,
    /// `SKY SCATTERING` (2026-10-19, with the filter, of the user's "a
    /// little cooler/brighter"): the camera's `TOD_Scattering` as Customs'
    /// ran it, the sky's light scattered over the scene by its distance and
    /// height, with `CUSTOMS LIGHT` (`scattering_view`; lighting spec, "The
    /// sky's scattering over the scene"). Off, nothing of it is read or
    /// drawn.
    pub sky_scattering: bool,
    /// `CUSTOMS CLOUDS` (2026-10-19; the user's "Customs' clouds and their
    /// shadows", then "1, 2, then 3 easy peasy. Goa head"): Customs' cloud
    /// layer as its weather's cloud controller draws it over the sky, and
    /// its shadows baked into the sun's cookie, at the GRAPHICS page's
    /// CLOUDS QUALITY, with `CUSTOMS LIGHT` (`clouds_view`; lighting spec,
    /// "Customs' clouds"). Off, nothing of it is read or drawn.
    pub customs_clouds: bool,
    /// `AMBIENT HIGHLIGHT` (2026-10-19; the user, of the character inside
    /// being dark: "Does tarkov do anything to lighten people up?", then
    /// "do both"): the game's `AmbientHighlight` as Customs has it, the
    /// ambient drawn again over the first-person arms and weapon outdoors,
    /// with `CUSTOMS LIGHT` (`highlight_view`; lighting spec, "The ambient
    /// highlight"). Off, nothing of it is drawn.
    pub ambient_highlight: bool,
    /// `TARKOV SHADING` (2026-10-19; the user, of the next step put: "Port
    /// Tarkov's shading (Recommended)"): the scene's surfaces lit as the
    /// game's deferred light and ambient pass light them, in its numbers,
    /// with `CUSTOMS LIGHT` (`shading`; lighting spec, "Tarkov's surface
    /// shading"). Off, Bevy's own shading, as before.
    pub tarkov_shading: bool,
    /// `AMBIENT OCCLUSION` (2026-10-19; the user, of SSAO, DLSS and FSR:
    /// "Can we just do ssao and then just do performance passes?"): the
    /// game's ambient occlusion, its camera's `HBAO`, at the choice of the
    /// game's SSAO setting on the GRAPHICS page (the install's own until
    /// changed), drawn as the game draws it (`hbao_view`; the spec
    /// `ambient-occlusion.md`). Off, nothing of it is drawn and the camera
    /// has no normal prepass.
    pub ambient_occlusion: bool,
    /// `HALL LAMPS` (2026-10-19, with the range hall): the hall's ceiling
    /// lamps, Factory's industrial lamp's figures, as Bevy's spot lights
    /// (`lighting::hall_lamp`), with `CUSTOMS LIGHT`. The user's fallback
    /// for lights on surfaces before Tarkov's surface shading is ported
    /// (`decisions.md`, 2026-10-18). Off, the hall is lit by its windows
    /// alone.
    pub hall_lamps: bool,
    /// `ICON GENERATION` (the user, 2026-10-14): the inventory's pictures
    /// are drawn from the items' models as the game draws them, and kept
    /// (`icons`). Off, none is drawn or read: each box has the plain shape
    /// of its sort.
    pub icon_generation: bool,
    /// `COMPATIBLE ITEMS HIGHLIGHT` (the user, 2026-10-14, night: "Add the
    /// hover machanic to the feature switches"): what goes with the thing
    /// under the mouse, or carried, is lit orange (`inventory_view::
    /// light_related`), as S.T.A.L.K.E.R. lights it. Off, only the box under
    /// the mouse is lit, as the game lights it.
    pub compatible_highlight: bool,
    /// `CHARACTER MODEL` (the user, 2026-10-14, night: "Lets change the
    /// Paper player model to be a still model of the 3d model of the
    /// current playermodel ... standing straight like the image provided
    /// this stance comes from the pre-raid screen. Make it a switchable
    /// mechanic feature as well."): the gear page has the character's own
    /// model behind its slots, as the game's screens stand it, drawn by the
    /// body's own meshes and pictures (`figure_view`). Off, the app's paper
    /// figure of plain shapes.
    pub character_model: bool,
    /// `TARKOV UI` (the user, 2026-10-15, at night: "right now, we have a
    /// mixed ui id like a switch that changes to 1;1 tarkov ui or our own.
    /// for examjple1 i want our 3d inventory model in our health tab
    /// instead of a mix of tarkov and ours"): the inventory's screens drawn
    /// as the game's own wherever they are built so (the HEALTH tab's
    /// panel, `health_view`), their pictures read from the install. Off,
    /// the app's own look throughout: the HEALTH tab has the character's
    /// model as the gear tab has it, and its boxes in the app's style at
    /// the game's places.
    pub tarkov_ui: bool,
    /// The systems of the list that are built (the user, 2026-10-14:
    /// "make the switches work for mechanics that are fully done. For
    /// now, when set to off, lets make it just stop"): those of the
    /// simulation (`trp_sim::Systems`, `systems`), and the app's own
    /// three: the weapon's handling (its keys are not heard), the
    /// first-person body (nothing of it is drawn for the character's own
    /// eyes), the inventory (its screen opens on the options alone, and
    /// nothing is taken, bound or moved) and what is worked in the world
    /// (nothing is looked at to be worked).
    pub movement: bool,
    pub vaulting: bool,
    /// `STAMINA` on the `SIMPLE` list (0.101.0): the group of the two
    /// parts under `ADVANCED`, `STAMINA DRAIN` and `EXHAUSTION`. Off, both
    /// are off whatever their own switches say (`effective`). (Its key in
    /// the options' file, `"stamina"`, was the drain's until 0.101.0: a
    /// file that had it off now has the group off, which is at least that.)
    pub stamina: bool,
    /// `STAMINA DRAIN` (`STAMINA` until 0.101.0; `trp_sim::Systems::stamina`):
    /// the pools drain and refill. Off, nothing is paid for.
    pub stamina_drain: bool,
    pub weight: bool,
    pub exhaustion: bool,
    pub weapon_motion: bool,
    pub recoil: bool,
    pub weapon_handling: bool,
    pub first_person_body: bool,
    pub inventory: bool,
    pub world_interactions: bool,
    /// `HEALTH SYSTEM` (2026-10-14, night: the user, "we are ready to
    /// start the health system"): the body's parts and what harms and
    /// heals them (`docs/spec/health.md`). Off, nothing of it is made: the
    /// character cannot be hurt, and is taken as fed and watered.
    pub health: bool,
    /// `SKILLS` (2026-10-23: the user, "Build the skills system: a page in
    /// the options tab where I set each skill's level 0 to 51 ... a switch
    /// for the system like the others"): each skill's level, set on the
    /// SKILLS page or by a host game, changes what the game's code says it
    /// changes (`skills`; skills spec). Off, every skill counts as
    /// untrained whatever its level.
    pub skills: bool,
    /// `ITEM PHYSICS` (2026-10-15: the user, "Notice weapons when doing a
    /// quick reload drop the magazine, and it is affected by gravity. So
    /// are dropped items, projected from the player... We need 1:1 physics
    /// parity"): things put down, dropped and let fall are thrown and fall
    /// in the game's engine, each with its own model (`loose`). Off,
    /// nothing of it is made: a thing lies as a plain box where it is put.
    pub item_physics: bool,
    /// `SOUNDS` on the `SIMPLE` list (0.101.0): the group of `MOVEMENT
    /// SOUNDS`, `HANDS SOUNDS` and `SHOT SOUNDS`. Off, all three are off.
    pub sounds: bool,
    /// `MOVEMENT SOUNDS` (2026-10-16: the user, "Lets add the sounds for
    /// moving in the world"): the footsteps by the surface underfoot, the
    /// sprint's, the stop, the jump, the landing, the crawl, the turn and
    /// the kit, as the game plays them for the player (`step_sound`). Off,
    /// the banks are not read and nothing plays.
    pub movement_sounds: bool,
    /// `HANDS SOUNDS` (2026-10-16: the user, "Let's do the sounds"): what
    /// a weapon, an item used or the empty hands sound as their animation
    /// marks it (drawn, put away, a magazine out and in, the bolt, an AI-2
    /// opened and injected), from each one's own sound player
    /// (`hands_sound`). Off, the players are not read and nothing plays.
    pub hands_sounds: bool,
    /// `SHOT SOUNDS` (2026-10-16: the plan's next step, the shots' sounds):
    /// a weapon's shots from its own banks, an automatic weapon's fire as
    /// the game's loop ended on a beat by its tail (`shot_sound`). Off, the
    /// banks are not read and the shots are silent.
    pub shot_sounds: bool,
    /// `WORLD SOUNDS` (2026-10-23: the user, "Give the crates and tables
    /// the game's loot-opening sound ... the door breaching ... sounds are
    /// missing"): what a door, a container or a trunk of the range sounds
    /// as it swings, shuts or is kicked, from the clips its Customs prefab
    /// names (`thing_sound`). Off, the scenes are not read and nothing
    /// plays.
    pub world_sounds: bool,
    /// `GRENADES AND KNIFE` (2026-10-17: the plan's next step, "A grenade
    /// and a knife by their keys"): a grenade taken in hand by its keys,
    /// its pin pulled and held, thrown from above or below and flying in
    /// the game's engine to its fuse; the knife drawn from its sheath, its
    /// slash and stab with their stamina and their blade swept for a hit,
    /// and the quick stroke (grenades-and-knife spec). Off, their keys do
    /// nothing and their rigs are not made.
    pub grenades_and_knife: bool,
    /// `BALLISTICS` (2026-10-18: the user, "ballistics and a dummy on the
    /// range"; 2026-10-20, "Go ahead and start"): each shot's bullets flown
    /// as the game flies them, meeting the range's surfaces and going
    /// through, turning, breaking up, glancing off or stopping by the
    /// game's rules and its chances, each hit playing its material's hole,
    /// particles, light and sound (`bullets`, `impact_sound`; ballistics
    /// spec). Off, a shot makes no bullet.
    pub ballistics: bool,
    /// `DUMMY` (2026-10-18: the user, "a dummy on the range"; "Let's do the
    /// PMC dummy idea"; 2026-10-20, "Copy your gear", "Falls as a corpse"):
    /// a PMC standing down the lanes with the game's hit colliders, health
    /// and the armour of a copy of the player's gear, the debug panel dressing it
    /// afresh and making it whole; dead, it falls as the game's ragdoll
    /// (`dummy`; body hits spec, "The dummy on the range"). Off, it is not
    /// built.
    pub dummy: bool,
    /// `REACTIVE TARGETS` (2026-10-26: the user, "can we put the range
    /// finder somewhere along with some reactive targets far away?"): the
    /// hideout range's steel poppers out along the open start's way, folded
    /// by a hit on the face and stood up again as the game's (`poppers`;
    /// range targets spec). Off, none is made and nothing of them is read.
    pub reactive_targets: bool,
    /// `ADVANCED ATTACHMENT SYSTEM` (2026-10-20: the user's workbench,
    /// "opens the edit build menu for the current gun in-hand"): every
    /// weapon is made from the parts it has, in the hands, its picture and
    /// its model on the ground, and handles by them (`trp_sim::inventory::
    /// rig_key`; inventory spec, "A weapon's build"). Off, every weapon is
    /// its stock build whatever is put on it.
    pub attachments: bool,
    /// `WORKBENCH` (2026-10-20: the user, "Let's add a workbench using the
    /// workbench model, that when interacted with, opens the edit build
    /// menu for the current gun in-hand"; "Replace the old clothes crate
    /// with the workbench"): the hideout's level 3 bench read from the
    /// install where the airdrop crate stood, and the interaction menu's
    /// EDIT BUILD in its triggers (`workbench`; workbench spec). Off, it is
    /// not there.
    pub workbench: bool,
    /// `CLOTHING` (2026-10-18: the user, "I want to add the clothing drop
    /// downs so we can change our appearance ... we should mix them all"):
    /// the OVERALL tab's two dropdowns of the install's suits, every side's,
    /// each read when first chosen and put on the body, the model, the
    /// corpse and the hands for one's own eyes (`clothing`; inventory spec,
    /// "The OVERALL tab"). Off, nothing of it is read: the tab shows no
    /// clothing panel, as the game's does in a raid, and the character
    /// wears the default suits.
    pub clothing: bool,
    /// `PIP` (2026-10-23: the user, "I want PIP to be under the "Simple"
    /// Category, just because it is a core feature"): a scope's lens shows
    /// the game's own second picture, drawn by its optic camera from inside
    /// the scope, with its reticle and its shadow (`optic_view`; optics
    /// spec, "The optic camera", "The lens"). Off, no optic camera is made:
    /// the world zoomed by the scope's magnification instead, the weapon at
    /// its size, the lens clear with its reticle (the user's; optics spec,
    /// "With `PIP` off").
    pub pip: bool,
    /// `TACTICAL DEVICES` (2026-10-25: the user, "Tactical devices and blind
    /// fire are much more important"; on `SIMPLE`, their choice): a
    /// weapon's lights and lasers and the head light, switched by the
    /// game's keys in either activation mode, the hands' switch and its
    /// sound, the head light's gesture, each device's model as its mode
    /// shows it, its lights, laser, light shaft and flare, the HUD's panel
    /// of the mode, and a combo unit's hip penalty (`tactical_view`;
    /// tactical-devices spec). Off, the keys do nothing and nothing of them
    /// is read or drawn: every device shows its off object.
    pub tactical_devices: bool,
    /// `BLIND FIRE` (2026-10-25: the user, "Tactical devices and blind fire
    /// are much more important"; on `SIMPLE` by their choice, asked by
    /// multiple choice): the weapon held over cover or out to the right by
    /// `Alt`+`W` and `Alt`+`S`, as the game's state, keys, refusals,
    /// body, camera and overlap have it (`trp_sim::blind_fire`; blind-fire
    /// spec). Off, its keys are not heard and it never starts
    /// (`trp_sim::Systems::blind_fire`).
    pub blind_fire: bool,
    /// `MOUNTING` (2026-10-25: on `SIMPLE` after `BLIND FIRE`, the user's
    /// choice by multiple choice): the weapon rested on cover by `V` and the
    /// bipod put out and folded by `Ctrl`+`V`, as the game's search, mount,
    /// refusals, ends, body, camera and recoil have it (`trp_sim::mounting`;
    /// mounting spec). Off, both keys do nothing
    /// (`trp_sim::Systems::mounting`).
    pub mounting: bool,
    /// `LEFT SHOULDER` (2026-10-25, the user's order after mounting; on
    /// `SIMPLE` after `MOUNTING`): the weapon carried on the left shoulder by
    /// `Mouse4`, as the game's key, refusals, drops and returns, body and
    /// weapon placement have it (`trp_sim::left_stance`; left-shoulder spec).
    /// Off, its key is not heard (`trp_sim::Systems::left_shoulder`).
    pub left_shoulder: bool,
    /// `DURABILITY`, `MALFUNCTIONS`, `OVERHEATING` (2026-10-25, the user's
    /// order after the left shoulder: "all of the durability, and weapon
    /// malfunction systems in, including the red barrel"; on `SIMPLE` after
    /// `LEFT SHOULDER`): a shot's wear of the weapon, the roll for a
    /// malfunction at each shot with its states, look and repair, and the
    /// heat each shot leaves, with what it does and the barrel's glow
    /// (`trp_sim::malfunction`; malfunctions spec). Off, each stops: the
    /// weapon does not wear, never malfunctions, stays cold
    /// (`trp_sim::firearm::WearSwitches`).
    pub durability: bool,
    pub malfunctions: bool,
    pub overheating: bool,
}

impl Default for Features {
    fn default() -> Self {
        Self {
            custom_hands: true,
            smooth_animations: true,
            grounded_feet: true,
            visual_effects: true,
            spent_cases: true,
            magazine_rounds: true,
            face_shield_mask: true,
            muzzle_flash: true,
            particles: true,
            // Off unless switched (the user, 2026-10-21: "Disable tarkov
            // lighting, customs light, camera color, sky scattering, clouds,
            // ambient highlight, tarkov shading, and hall lamps by
            // default").
            customs_light: false,
            camera_color_filter: false,
            camera_sharpening: true,
            sky_scattering: false,
            customs_clouds: false,
            ambient_highlight: false,
            tarkov_shading: false,
            ambient_occlusion: true,
            hall_lamps: false,
            icon_generation: true,
            compatible_highlight: true,
            character_model: true,
            // On unless switched (the user, 2026-10-17: "is tarkov ui on by
            // default? it should be."; off until 0.42.4).
            tarkov_ui: true,
            movement: true,
            vaulting: true,
            stamina: true,
            stamina_drain: true,
            weight: true,
            exhaustion: true,
            weapon_motion: true,
            recoil: true,
            weapon_handling: true,
            first_person_body: true,
            inventory: true,
            world_interactions: true,
            health: true,
            skills: true,
            item_physics: true,
            sounds: true,
            movement_sounds: true,
            hands_sounds: true,
            shot_sounds: true,
            world_sounds: true,
            grenades_and_knife: true,
            ballistics: true,
            dummy: true,
            reactive_targets: true,
            attachments: true,
            workbench: true,
            clothing: true,
            pip: true,
            tactical_devices: true,
            blind_fire: true,
            mounting: true,
            left_shoulder: true,
            durability: true,
            malfunctions: true,
            overheating: true,
        }
    }
}

impl Features {
    /// The simulation's own systems as switched.
    pub fn systems(&self) -> trp_sim::Systems {
        trp_sim::Systems {
            movement: self.movement,
            vaulting: self.vaulting,
            stamina: self.stamina_drain,
            weight: self.weight,
            exhaustion: self.exhaustion,
            weapon_motion: self.weapon_motion,
            recoil: self.recoil,
            blind_fire: self.blind_fire,
            mounting: self.mounting,
            left_shoulder: self.left_shoulder,
        }
    }

    /// The weapon's wear as switched.
    pub fn wear(&self) -> trp_sim::firearm::WearSwitches {
        trp_sim::firearm::WearSwitches { durability: self.durability, malfunctions: self.malfunctions, overheating: self.overheating }
    }

    /// The switches as they hold for a run: a part of a `SIMPLE` group is
    /// on only while its group is too; its own switch is kept as it is
    /// for when the group is on again (settings spec, "Features").
    pub fn effective(&self) -> Self {
        let mut held = self.clone();
        for part in FeatureSwitch::ALL {
            if part.group().is_some_and(|group| !group.get(self)) {
                *part.slot(&mut held) = false;
            }
        }
        held
    }
}

// ==== Slides, cells, listening; the options' state, its file, saving and loading ====

/// A bar that is slid: the field of view, and the four of the game's
/// controls tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Slide {
    Fov,
    GameFps,
    Sensitivity,
    Aiming,
    DoubleClick,
    Optic,
    /// A skill's level on the SKILLS page.
    Skill(trp_sim::skills::Skill),
    /// The range's time of day on the FEATURES page (`range_day`).
    Hour,
}

impl Slide {
    /// The game's own limits for each (`ControlSettingsGroup`,
    /// `GameSettingsGroup`).
    fn range(self) -> [f32; 2] {
        match self {
            Self::Fov => FOV,
            Self::GameFps => [GAME_FPS[0] as f32, GAME_FPS[1] as f32],
            Self::Sensitivity | Self::Aiming => [0.1, 5.0],
            Self::DoubleClick => [0.3, 1.0],
            Self::Optic => [1.0, 5.0],
            Self::Skill(_) => [0.0, trp_sim::skills::ELITE as f32],
            Self::Hour => [0.0, 24.0],
        }
    }

    /// What a bar is slid by: whole degrees, as the game's field of view
    /// is; hundredths for the rest (the app's: the game's bars are not
    /// stepped).
    fn step(self) -> f32 {
        match self {
            Self::Fov | Self::GameFps | Self::Skill(_) => 1.0,
            // Five minutes.
            Self::Hour => 1.0 / 12.0,
            _ => 0.01,
        }
    }

    /// The figure as the game's own bar writes it (three places for the
    /// two sensitivities, two for the others).
    fn shown(self, value: f32) -> String {
        match self {
            Self::Fov | Self::GameFps => format!("{value:.0}"),
            Self::Skill(_) if value >= trp_sim::skills::ELITE as f32 => "ELITE".to_owned(),
            Self::Skill(_) => format!("{value:.0}"),
            Self::Sensitivity | Self::Aiming => format!("{value:.3}"),
            Self::DoubleClick | Self::Optic => format!("{value:.2}"),
            Self::Hour => {
                let minutes = (value * 60.0).round() as u32;
                format!("{:02}:{:02}", minutes / 60, minutes % 60)
            }
        }
    }

    /// The game's own words for it, by the name its texts have them
    /// under, and the app's where the install has none.
    fn caption(self) -> (&'static str, &'static str) {
        match self {
            Self::Fov => ("", "FIELD OF VIEW"),
            Self::GameFps => ("Game FPS Limit", "GAME FPS LIMIT"),
            Self::Sensitivity => ("Mouse sensitivity", "MOUSE SENSITIVITY"),
            Self::Aiming => ("Mouse sensitivity (aiming)", "MOUSE SENSITIVITY (AIMING)"),
            Self::DoubleClick => ("Settings/DoubleClickTimeout", "DOUBLE CLICK TIMEOUT"),
            Self::Optic => ("Settings/OpticSensitivity", "SCOPE ZOOM ADJUSTMENT SENSITIVITY"),
            Self::Skill(skill) => (skill.name(), skill.name()),
            Self::Hour => ("", "TIME OF DAY"),
        }
    }
}

/// A cell of the list of bindings that takes keys: a variant of a
/// binding, or one way of one of an axis's pairs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Cell {
    Key(usize, usize),
    Axis(usize, usize, usize),
}

/// A key being chosen for a cell.
struct Listen {
    cell: Cell,
    heard: Listening,
    /// The click that began it is over: no key or button is down.
    armed: bool,
    /// The choice is made; it waits for every key to be let go.
    done: bool,
}

#[derive(Resource)]
pub struct OptionsState {
    /// Which of the tab's three parts is shown.
    sub: usize,
    listening: Option<Listen>,
    /// The binding whose way of being pressed is being chosen.
    asking: Option<usize>,
    note: String,
    /// How far down the list of bindings is scrolled, in pixels.
    scroll: f32,
    pub graphics: Graphics,
    /// What is switched of the features' list, as it is kept for the next
    /// run.
    pub features: Features,
    /// The switches as this run began, which it holds (`Features`): a
    /// switch thrown away from its own says so on the tab.
    pub started: Features,
    /// The switches as they hold for this run: `started` with each part
    /// of a `SIMPLE` group that is off held off (`Features::effective`).
    /// What the app goes by.
    pub in_force: Features,
    /// The mouse's settings, and what the install's own game has for them.
    pub mouse: Mouse,
    install_mouse: Mouse,
    /// The display's settings came from the app's file and are to be
    /// put on the window as the run begins.
    apply_at_start: bool,
    /// What the install's own game has for them.
    install_graphics: Graphics,
    /// The settings of the game's own GAME tab the battle screen's panels
    /// go by (`battle_view::BattleModes`), as they stand and as the
    /// install has them.
    pub game: crate::battle_view::BattleModes,
    install_game: crate::battle_view::BattleModes,
    /// The range's time of day (the FEATURES page's TIME OF DAY: the user,
    /// 2026-10-25), in hours, and as the run began: the range's own light
    /// follows it at once (`range_day`), Customs' is set by it at the start
    /// (`CUSTOMS LIGHT`). Not kept: every run begins at
    /// `lighting::RANGE_MOMENT`'s hour, 09:30 (the user: "make 09:30 the
    /// default so the app always starts at that time"), or `--hour`'s.
    pub hour: f32,
    pub hour_started: f32,
    /// The game's volumes of its sound settings the app's sounds go by
    /// (`ui_sound::Levels`), as they stand and as the install has them.
    pub sound: crate::ui_sound::Levels,
    install_sound: crate::ui_sound::Levels,
    /// The pictures' quality and streaming as this run began, which it
    /// holds.
    pub textures_started: (u8, bool),
    /// The shadows' quality as this run began, which it holds.
    pub shadows_started: u8,
    /// The ambient occlusion's choice as this run began, which it holds.
    pub ssao_started: trp_sim::hbao::SsaoMode,
    /// The clouds' quality as this run began, which it holds.
    pub clouds_started: trp_sim::clouds::CloudsQuality,
    /// Whether this run renders on a thread of its own, which it holds.
    pub render_thread_started: bool,
    /// The backend as this run began, which it holds.
    pub renderer_started: crate::backend::Renderer,
    /// The page is to be drawn afresh.
    dirty: bool,
    /// Where what is changed is kept; none in a run nobody is at the
    /// keys of, which neither reads nor writes it.
    file: Option<PathBuf>,
    /// The game's own words for the bindings, by their names in small
    /// letters (the install's English texts).
    names: HashMap<String, String>,
    /// The bar held by the mouse.
    sliding: Option<Slide>,
    /// The character's skills as the SKILLS page sets them, and as a host
    /// game changes them in play (`skills::Skills`): each one's progress,
    /// kept in the app's file whole, fractions of a level and all.
    pub skills: trp_sim::skills::SkillLevels,
}

/// The app's own file for what is changed here: `options.json` in the
/// `local` folder beside the workspace (the nearest above where the app
/// is run from, as the install's path is looked for).
pub(crate) fn options_file() -> PathBuf {
    let here = std::env::current_dir().unwrap_or_default();
    let found = here.ancestors().map(|dir| dir.join("local")).find(|local| local.is_dir());
    found.unwrap_or_else(|| here.join("local")).join("options.json")
}

fn source_json(source: &Source) -> serde_json::Value {
    let keys: Vec<String> = source.keys.iter().map(|key| key_name(*key)).collect();
    match source.wheel {
        Some(up) => serde_json::json!({ "keys": keys, "wheel": up }),
        None => serde_json::json!({ "keys": keys }),
    }
}

fn source_of_json(value: &serde_json::Value) -> Source {
    let names = value.get("keys").and_then(|keys| keys.as_array());
    let keys = names.map(|names| names.iter().filter_map(|name| code(name.as_str()?)).collect()).unwrap_or_default();
    Source { keys, wheel: value.get("wheel").and_then(|up| up.as_bool()) }
}

/// The skills as the app's file keeps them: each skill not at 0 by the
/// game's name, with its progress (a hundred a level).
fn skills_json(skills: &trp_sim::skills::SkillLevels) -> serde_json::Value {
    let kept = trp_sim::skills::Skill::ALL.iter().filter(|skill| skills.progress(**skill) > 0.0).map(|skill| (skill.name().to_owned(), serde_json::json!(skills.progress(*skill))));
    serde_json::Value::Object(kept.collect())
}

#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_arguments)]
fn save(path: &std::path::Path, controls: &Controls, graphics: &Graphics, mouse: &Mouse, features: &Features, game: &crate::battle_view::BattleModes, sound: &crate::ui_sound::Levels, skills: &trp_sim::skills::SkillLevels) -> std::io::Result<()> {
    let keys: Vec<serde_json::Value> = controls
        .keys
        .iter()
        .map(|group| serde_json::json!({ "name": group.name, "press": group.press.name(), "variants": group.variants.iter().map(source_json).collect::<Vec<_>>() }))
        .collect();
    let axes: Vec<serde_json::Value> = controls
        .axes
        .iter()
        .map(|axis| serde_json::json!({ "name": axis.name, "pairs": axis.pairs.iter().map(|pair| pair.iter().map(source_json).collect::<Vec<_>>()).collect::<Vec<_>>() }))
        .collect();
    let mode = match graphics.mode {
        Mode::Fullscreen => "fullscreen",
        Mode::Borderless => "borderless",
        Mode::Windowed => "windowed",
    };
    let all = serde_json::json!({
        "controls": { "double_click": controls.double_click, "keys": keys, "axes": axes },
        "mouse": { "sensitivity": mouse.sensitivity, "aiming_sensitivity": mouse.aiming, "optic_sensitivity": mouse.optic, "inverted_x": mouse.invert[0], "inverted_y": mouse.invert[1] },
        "graphics": { "vsync": graphics.vsync, "field_of_view": graphics.field_of_view, "mode": mode, "resolution": graphics.resolution, "texture_quality": graphics.texture_quality, "mip_streaming": graphics.mip_streaming, "shadow_quality": graphics.shadow_quality, "ssao": graphics.ssao.key(), "clouds_quality": graphics.clouds_quality.key(), "render_thread": graphics.render_thread, "game_fps": graphics.game_fps, "fps_limit_off": graphics.fps_limit_off, "msaa": graphics.msaa, "renderer": graphics.renderer.key() },
        "features": FeatureSwitch::ALL.iter().map(|switch| (switch.key().to_owned(), serde_json::Value::Bool(switch.get(features)))).collect::<serde_json::Map<_, _>>(),
        "game": { "quick_slots": game.quick.name(), "stamina": game.stamina.name(), "health": game.health.name(), "health_color": game.color_name(), "tactical_input": game.tactical.word() },
        "sound": { "overall": sound.overall, "interface": sound.interface },
        "skills": skills_json(skills),
    });
    // The suits chosen on the OVERALL tab are kept in the same file by
    // their own module (`clothing`), and kept as they are.
    let mut all = all;
    if let Some(kept) = std::fs::read_to_string(path).ok().and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok()).and_then(|old| old.get(crate::clothing::KEPT_AS).cloned()) {
        all[crate::clothing::KEPT_AS] = kept;
    }
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder)?;
    }
    std::fs::write(path, serde_json::to_string_pretty(&all).unwrap_or_default())
}

/// Reads the app's file over the install's bindings and the display's
/// and the mouse's settings as the run began with them: a binding or an
/// axis the file names takes the file's keys, and the rest stay the
/// install's; a switch of the features' list the file has is as the file
/// has it. Says which of the first two parts the file had.
#[allow(clippy::too_many_arguments)]
fn load(path: &std::path::Path, controls: &mut Controls, graphics: &mut Graphics, mouse: &mut Mouse, features: &mut Features, game: &mut crate::battle_view::BattleModes, sound: &mut crate::ui_sound::Levels, skills: &mut trp_sim::skills::SkillLevels) -> [bool; 2] {
    let Some(all) = std::fs::read_to_string(path).ok().and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok()) else { return [false; 2] };
    if let Some(kept) = all.get("skills").and_then(|kept| kept.as_object()) {
        for (name, progress) in kept {
            match (trp_sim::skills::Skill::of_name(name), progress.as_f64()) {
                (Some(skill), Some(progress)) => skills.set_progress(skill, progress as f32),
                _ => eprintln!("options: no skill {name} to keep"),
            }
        }
    }
    if let Some(kept) = all.get("sound") {
        let level = |name: &str, now: u8| kept.get(name).and_then(|v| v.as_u64()).map_or(now, |v| v.min(10) as u8);
        (sound.overall, sound.interface) = (level("overall", sound.overall), level("interface", sound.interface));
    }
    if let Some(kept) = all.get("game") {
        use crate::battle_view::Shows;
        let shows = |name: &str, now: Shows| kept.get(name).and_then(|v| Shows::of(v.as_str()?)).unwrap_or(now);
        game.quick = shows("quick_slots", game.quick);
        game.stamina = shows("stamina", game.stamina);
        game.health = shows("health", game.health);
        if let Some(scheme) = kept.get("health_color").and_then(|v| v.as_str()) {
            game.green_to_red = scheme == "GreenToRed";
        }
        if let Some(mode) = kept.get("tactical_input").and_then(|v| v.as_str()).and_then(trp_sim::tactical::InputMode::of_word) {
            game.tactical = mode;
        }
    }
    let switch = |name: &str| all.get("features").and_then(|kept| kept.get(name)?.as_bool());
    for kept in FeatureSwitch::ALL {
        if let Some(on) = switch(kept.key()) {
            *kept.slot(features) = on;
        }
    }
    if let Some(kept) = all.get("mouse") {
        let figure = |name: &str, kind: Slide, now: f32| kept.get(name).and_then(|v| v.as_f64()).map_or(now, |v| (v as f32).clamp(kind.range()[0], kind.range()[1]));
        mouse.sensitivity = figure("sensitivity", Slide::Sensitivity, mouse.sensitivity);
        mouse.aiming = figure("aiming_sensitivity", Slide::Aiming, mouse.aiming);
        mouse.optic = figure("optic_sensitivity", Slide::Optic, mouse.optic);
        for (at, name) in ["inverted_x", "inverted_y"].into_iter().enumerate() {
            mouse.invert[at] = kept.get(name).and_then(|v| v.as_bool()).unwrap_or(mouse.invert[at]);
        }
    }
    let list = |part: &str, list: &str| all.get(part).and_then(|part| part.get(list)?.as_array()).cloned().unwrap_or_default();
    let named = |entry: &serde_json::Value| entry.get("name").and_then(|name| name.as_str()).map(str::to_owned);
    for entry in list("controls", "keys") {
        let Some(group) = named(&entry).and_then(|name| controls.keys.iter_mut().find(|group| group.name == name)) else { continue };
        let variants = entry.get("variants").and_then(|variants| variants.as_array()).cloned().unwrap_or_default();
        group.variants = std::array::from_fn(|at| variants.get(at).map(source_of_json).unwrap_or_default());
        group.press = entry.get("press").and_then(|press| PressType::of(press.as_str()?)).unwrap_or(group.press);
    }
    for entry in list("controls", "axes") {
        let Some(axis) = named(&entry).and_then(|name| controls.axes.iter_mut().find(|axis| axis.name == name)) else { continue };
        let pairs = entry.get("pairs").and_then(|pairs| pairs.as_array()).cloned().unwrap_or_default();
        axis.pairs = std::array::from_fn(|pair| std::array::from_fn(|way| pairs.get(pair).and_then(|pair| pair.get(way)).map(source_of_json).unwrap_or_default()));
    }
    if let Some(seconds) = all.get("controls").and_then(|controls| controls.get("double_click")?.as_f64()) {
        controls.double_click = (seconds as f32).clamp(0.3, 1.0);
    }
    let Some(display) = all.get("graphics") else { return [all.get("controls").is_some(), false] };
    graphics.vsync = display.get("vsync").and_then(|vsync| vsync.as_bool()).unwrap_or(graphics.vsync);
    graphics.field_of_view = display.get("field_of_view").and_then(|fov| fov.as_f64()).map_or(graphics.field_of_view, |fov| (fov as f32).clamp(FOV[0], FOV[1]));
    graphics.mode = match display.get("mode").and_then(|mode| mode.as_str()) {
        Some("fullscreen") => Mode::Fullscreen,
        Some("borderless") => Mode::Borderless,
        Some("windowed") => Mode::Windowed,
        _ => graphics.mode,
    };
    graphics.texture_quality = display.get("texture_quality").and_then(|quality| quality.as_u64()).map_or(graphics.texture_quality, |quality| quality.min(2) as u8);
    graphics.mip_streaming = display.get("mip_streaming").and_then(|on| on.as_bool()).unwrap_or(graphics.mip_streaming);
    graphics.shadow_quality = display.get("shadow_quality").and_then(|quality| quality.as_u64()).map_or(graphics.shadow_quality, |quality| quality.min(3) as u8);
    graphics.ssao = display.get("ssao").and_then(|key| trp_sim::hbao::SsaoMode::from_key(key.as_str()?)).unwrap_or(graphics.ssao);
    graphics.clouds_quality = display.get("clouds_quality").and_then(|key| trp_sim::clouds::CloudsQuality::from_key(key.as_str()?)).unwrap_or(graphics.clouds_quality);
    graphics.render_thread = display.get("render_thread").and_then(|on| on.as_bool()).unwrap_or(graphics.render_thread);
    graphics.game_fps = display.get("game_fps").and_then(|fps| fps.as_u64()).map_or(graphics.game_fps, |fps| (fps.min(1000) as u32).clamp(GAME_FPS[0], GAME_FPS[1]));
    graphics.fps_limit_off = display.get("fps_limit_off").and_then(|off| off.as_bool()).unwrap_or(graphics.fps_limit_off);
    graphics.msaa = display.get("msaa").and_then(|samples| samples.as_u64()).and_then(|samples| MSAA.iter().find(|(s, _)| *s as u64 == samples).map(|(s, _)| *s)).unwrap_or(graphics.msaa);
    graphics.renderer = display.get("renderer").and_then(|key| crate::backend::Renderer::from_key(key.as_str()?)).unwrap_or(graphics.renderer);
    let side = |at: usize| display.get("resolution")?.get(at)?.as_u64().map(|side| side as u32);
    if let (Some(width), Some(height)) = (side(0), side(1)) {
        graphics.resolution = [width.max(640), height.max(360)];
    }
    [all.get("controls").is_some(), true]
}

impl OptionsState {
    /// The options as the run begins: the display as the app starts it
    /// (a window of `size`, the frames waiting for the screen unless told
    /// otherwise), and, for someone at the keys, what the app's own file
    /// has over that and over the install's bindings. The field of view
    /// given on the command line stays.
    pub fn new(options: &mut Options, install: &trp_data::Install, bindings: &mut Bindings, size: [u32; 2]) -> Self {
        let read = install.display_settings();
        // The install's own choice of ambient occlusion, or the game's
        // default (`GraphicsSettingsGroup`: off).
        let install_ssao = read.ssao.as_deref().and_then(trp_sim::hbao::SsaoMode::from_key).unwrap_or(trp_sim::hbao::SsaoMode::Off);
        // The install's clouds' quality, or the game's default
        // (`GraphicsSettingsGroup`: low).
        let install_clouds = read.clouds_quality.and_then(trp_sim::clouds::CloudsQuality::from_key).unwrap_or(trp_sim::clouds::CloudsQuality::Low);
        // The game renders a frame behind its main thread where its build
        // has the engine's graphics jobs on (this install's has).
        let install_render_thread = install.graphics_jobs().unwrap_or(true);
        let install_fps = read.game_framerate.unwrap_or(GAME_FPS_DEFAULT).clamp(GAME_FPS[0], GAME_FPS[1]);
        let install_fps_off = read.disable_game_framerate_limit.unwrap_or(false);
        // The pictures and the shadows at HIGH (the user, 2026-10-22:
        // "Start at HIGH instead"), the pictures streamed (the user,
        // 2026-10-15: "can we enable mip streaming?"; the install's own
        // game has it off).
        let mut graphics = Graphics {
            vsync: options.present == "vsync",
            field_of_view: options.field_of_view,
            mode: Mode::Windowed,
            resolution: size,
            texture_quality: crate::streaming::START_QUALITY,
            mip_streaming: true,
            shadow_quality: crate::shadows::START_QUALITY,
            ssao: install_ssao,
            clouds_quality: install_clouds,
            render_thread: install_render_thread,
            game_fps: install_fps,
            fps_limit_off: install_fps_off,
            msaa: 4,
            renderer: crate::backend::Renderer::DEFAULT,
        };
        let install_graphics = Graphics {
            vsync: read.vsync.unwrap_or(false),
            field_of_view: install.player_settings().field_of_view.unwrap_or(FOV[0]).clamp(FOV[0], FOV[1]),
            mode: read.mode.map_or(Mode::Borderless, Mode::of_game),
            resolution: read.resolution.unwrap_or(size),
            texture_quality: read.texture_quality.unwrap_or(2),
            mip_streaming: read.mip_streaming.unwrap_or(false),
            shadow_quality: read.shadows_quality.unwrap_or(crate::shadows::DEFAULT_QUALITY),
            ssao: install_ssao,
            clouds_quality: install_clouds,
            render_thread: install_render_thread,
            game_fps: install_fps,
            fps_limit_off: install_fps_off,
            // (The game multisamples nothing; the app's own is kept.)
            msaa: 4,
            // (The game draws through Direct3D 11, which is neither.)
            renderer: crate::backend::Renderer::DEFAULT,
        };
        // The mouse as the run begins with it: the install's, or what the
        // command line said; and what the install's own game has.
        let mut mouse = Mouse {
            sensitivity: options.sensitivity,
            aiming: options.aiming_sensitivity,
            optic: install.control_setting("OpticSensitivity").unwrap_or(1.0).clamp(1.0, 5.0),
            invert: options.invert,
        };
        let theirs = install.player_settings();
        let install_mouse = Mouse {
            sensitivity: theirs.mouse_sensitivity.unwrap_or(0.7).clamp(0.1, 5.0),
            aiming: theirs.aiming_sensitivity.unwrap_or(0.5).clamp(0.1, 5.0),
            optic: mouse.optic,
            invert: options.invert,
        };
        let file = (!options.unattended()).then(options_file);
        let mut apply_at_start = false;
        let mut features = Features::default();
        // The battle screen's panels as the install's own game has them,
        // unless kept otherwise here.
        let install_game = crate::battle_view::BattleModes::read(install);
        let mut game = install_game;
        let install_sound = crate::ui_sound::Levels::read(install);
        let mut sound = install_sound;
        // The skills untrained until the file says otherwise (never the
        // profile's: the user, 2026-10-22, "Set in the options tab").
        let mut skills = trp_sim::skills::SkillLevels::default();
        let mut hour = crate::lighting::RANGE_MOMENT.hour;
        if let Some(path) = &file {
            let asked = graphics.field_of_view;
            let asked_mouse = mouse.clone();
            let [controls, display] = load(path, &mut bindings.controls, &mut graphics, &mut mouse, &mut features, &mut game, &mut sound, &mut skills);
            if controls {
                bindings.changed();
            }
            if options.fov_given {
                graphics.field_of_view = asked;
            }
            if options.sensitivity_given[0] {
                mouse.sensitivity = asked_mouse.sensitivity;
            }
            if options.sensitivity_given[1] {
                mouse.aiming = asked_mouse.aiming;
            }
            (options.sensitivity, options.aiming_sensitivity, options.invert) = (mouse.sensitivity, mouse.aiming, mouse.invert);
            options.optic_sensitivity = mouse.optic;
            if controls || display {
                println!("options: {} read ({}{})", path.display(), if controls { "the keys" } else { "" }, if display { " the display" } else { "" });
            }
            options.field_of_view = graphics.field_of_view;
            apply_at_start = display;
        }
        // The game's own words for its bindings, for the ways of being
        // pressed and for what else its controls tab says.
        let mut wanted: Vec<&str> = bindings.controls.keys.iter().map(|group| group.name.as_str()).collect();
        wanted.extend(SAID.iter().map(|(name, _)| *name));
        wanted.extend(trp_sim::skills::Skill::ALL.map(trp_sim::skills::Skill::name));
        let names = trp_data::inventory::texts(install, "en", &wanted).unwrap_or_else(|e| {
            eprintln!("the game's words for its bindings could not be read: {e:#}");
            HashMap::new()
        });
        // The command line has the last word on the battle screen's panels
        // for this run (`--hud`), as on a switch.
        if let Some(shows) = options.hud {
            (game.quick, game.stamina, game.health) = (shows, shows, shows);
        }
        // The command line has the last word on the hour (`--hour`).
        if let Some(asked) = options.hour {
            hour = asked;
        }
        // The command line has the last word on a switch, as on the rest.
        if let Some(on) = options.custom_hands {
            features.custom_hands = on;
        }
        if let Some(on) = options.smooth_animations {
            features.smooth_animations = on;
        }
        if let Some(on) = options.icon_generation {
            features.icon_generation = on;
        }
        for (name, on) in &options.feature_switches {
            match FeatureSwitch::ALL.into_iter().find(|switch| switch.key() == name) {
                Some(switch) => *switch.slot(&mut features) = *on,
                None => eprintln!("no switch of the features is called {name}: {}", FeatureSwitch::ALL.map(FeatureSwitch::key).join(", ")),
            }
        }
        // The command line's levels for this run, over the file's.
        use trp_sim::skills::Skill;
        for class in Skill::WEAPON_CLASSES.into_iter().filter(|_| options.weapon_skill.is_some()) {
            skills.set_level(class, options.weapon_skill.unwrap_or(0));
        }
        if let Some(level) = options.strength {
            skills.set_level(Skill::Strength, level);
        }
        if let Some(level) = options.endurance {
            skills.set_level(Skill::Endurance, level);
        }
        for (name, level) in &options.skill_levels {
            match Skill::of_name(name) {
                Some(skill) => skills.set_level(skill, *level),
                None => eprintln!("no skill is called {name}: {}", Skill::ALL.map(Skill::name).join(", ")),
            }
        }
        let started = features.clone();
        let in_force = started.effective();
        let textures_started = (graphics.texture_quality, graphics.mip_streaming);
        let shadows_started = graphics.shadow_quality;
        let ssao_started = graphics.ssao;
        let clouds_started = graphics.clouds_quality;
        let render_thread_started = options.render_thread.unwrap_or(if options.unattended() { false } else { graphics.render_thread });
        let renderer_started = graphics.renderer;
        Self { sub: 0, listening: None, asking: None, note: String::new(), scroll: 0.0, graphics, features, started, in_force, mouse, install_mouse, apply_at_start, install_graphics, game, install_game, sound, install_sound, hour, hour_started: hour, textures_started, shadows_started, ssao_started, clouds_started, render_thread_started, renderer_started, dirty: true, file, names, sliding: None, skills }
    }

    /// Whether Escape is the tab's own just now: a key is being chosen,
    /// which Escape drops, or a way of being pressed is asked for, which
    /// Escape leaves as it was. The inventory screen then does not shut
    /// for it.
    pub fn takes_escape(&self) -> bool {
        self.listening.is_some() || self.asking.is_some()
    }

    /// The game's own words for a line of its controls tab, in capitals,
    /// or the app's where the install has none.
    fn said(&self, name: &str) -> String {
        let ours = SAID.iter().find(|(known, _)| *known == name).map_or(name, |(_, ours)| *ours);
        self.names.get(&name.to_lowercase()).map_or_else(|| ours.to_owned(), |words| words.to_uppercase())
    }

    /// The game's word for a way of being pressed.
    fn press_word(&self, press: PressType) -> String {
        self.said(&format!("EPressType/{}", press.name()))
    }

    /// A bar's figure as it stands. The double click's time is the
    /// bindings' own.
    fn slider(&self, kind: Slide, controls: &Controls) -> f32 {
        match kind {
            Slide::Fov => self.graphics.field_of_view,
            Slide::GameFps => self.graphics.game_fps as f32,
            Slide::Sensitivity => self.mouse.sensitivity,
            Slide::Aiming => self.mouse.aiming,
            Slide::DoubleClick => controls.double_click,
            Slide::Optic => self.mouse.optic,
            Slide::Skill(skill) => self.skills.level(skill) as f32,
            Slide::Hour => self.hour,
        }
    }

    fn set_slider(&mut self, kind: Slide, value: f32, controls: &mut Controls) {
        match kind {
            Slide::Fov => self.graphics.field_of_view = value,
            Slide::GameFps => self.graphics.game_fps = (value.round() as u32).clamp(GAME_FPS[0], GAME_FPS[1]),
            Slide::Sensitivity => self.mouse.sensitivity = value,
            Slide::Aiming => self.mouse.aiming = value,
            Slide::DoubleClick => controls.double_click = value,
            Slide::Optic => self.mouse.optic = value,
            Slide::Skill(skill) => self.skills.set_level(skill, value.round().max(0.0) as u32),
            Slide::Hour => self.hour = value.clamp(0.0, 24.0),
        }
    }

    /// The game's words for a binding, in capitals, or its own name
    /// spaced out where the install has none.
    fn name_of(&self, name: &str) -> String {
        match self.names.get(&name.to_lowercase()) {
            Some(words) => words.to_uppercase(),
            None => {
                let mut spaced = String::new();
                for letter in name.chars() {
                    if letter.is_uppercase() && !spaced.is_empty() {
                        spaced.push(' ');
                    }
                    spaced.extend(letter.to_uppercase());
                }
                spaced
            }
        }
    }

    /// The tactical key's activation mode changed in play (T+B: the game
    /// flips it and saves its settings at once), and kept.
    pub fn set_tactical_input(&mut self, mode: trp_sim::tactical::InputMode, controls: &Controls) {
        self.game.tactical = mode;
        self.keep(controls);
    }

    fn keep(&mut self, controls: &Controls) {
        if let Some(path) = &self.file {
            if let Err(e) = save(path, controls, &self.graphics, &self.mouse, &self.features, &self.game, &self.sound, &self.skills) {
                self.note = format!("not kept: {} could not be written ({e})", path.display());
            }
        }
    }
}

/// The app's words for an axis's two ways.
const AXIS_WORDS: [(&str, [&str; 2]); 3] =
    [("MoveX", ["MOVE RIGHT", "MOVE LEFT"]), ("MoveY", ["MOVE FORWARD", "MOVE BACKWARD"]), ("LeanX", ["LEAN RIGHT SLOWLY", "LEAN LEFT SLOWLY"])];

/// What the game's controls tab says besides its bindings, each by the
/// name the install's texts have it under, with the app's words for an
/// install that has none.
const SAID: [(&str, &str); 33] = [
    ("Settings/Graphics/CloudsQuality", "CLOUDS QUALITY:"),
    ("CloudsMode/Low", "LOW"),
    ("CloudsMode/Medium", "MEDIUM"),
    ("CloudsMode/High", "HIGH"),
    ("Settings/Sound/OverallVolume", "OVERALL VOLUME:"),
    ("Game FPS Limit", "GAME FPS LIMIT:"),
    ("DisableGameFramerateLimit", "DISABLE GAME FPS LIMIT"),
    ("ESSAOMode/Off", "OFF"),
    ("ESSAOMode/FastestPerformance", "MAX PERFORMANCE"),
    ("ESSAOMode/FastPerformance", "HIGH PERFORMANCE"),
    ("ESSAOMode/HighQuality", "HIGH"),
    ("ESSAOMode/HighestQuality", "ULTRA"),
    ("ESSAOMode/ColoredHighestQuality", "COLORED ULTRA"),
    ("Settings/Sound/InterfaceVolume", "INTERFACE VOLUME:"),
    ("Ui/Settings/QuickSlotsVisibility", "QUICK SLOTS"),
    ("Ui/Settings/StanimaVisibility", "STAMINA AND STANCE"),
    ("Ui/Settings/HeathVisibility", "HEALTH CONDITION"),
    ("Ui/Settings/HealthColorScheme", "HEALTH COLOR SCHEME"),
    ("Ui/Settings/Visibility/Autohide", "AUTOHIDE"),
    ("Ui/Settings/Visibility/Always", "ALWAYS SHOWN"),
    ("Ui/Settings/Visibility/Never", "ALWAYS HIDDEN"),
    ("Ui/Settings/ColorScheme/WhiteToRed", "MONOCHROME"),
    ("Ui/Settings/ColorScheme/GreenToRed", "POLYCHROME"),
    ("EPressType/Release", "RELEASE"),
    ("EPressType/Continuous", "CONTINUOUS"),
    ("EPressType/Press", "PRESS"),
    ("EPressType/DoubleClick", "DOUBLE CLICK"),
    ("Mouse sensitivity", "MOUSE SENSITIVITY"),
    ("Mouse sensitivity (aiming)", "MOUSE SENSITIVITY (AIMING)"),
    ("Settings/DoubleClickTimeout", "DOUBLE CLICK TIMEOUT"),
    ("Settings/OpticSensitivity", "SCOPE ZOOM ADJUSTMENT SENSITIVITY"),
    ("Inverted X axis", "INVERTED X AXIS"),
    ("Inverted Y axis", "INVERTED Y AXIS"),
];

/// The one switch of the lists that has nothing behind it yet: the user's
/// own design for an inventory in the S.T.A.L.K.E.R. series' way
/// (2026-10-21; `docs/backlog.md`, "Lite Inventory"), not to be built until
/// told. Shown under `EXPERIMENTAL` with no switch, so that the list says
/// where it will go. (The older list of what was not built is gone with
/// 0.101.0: `docs/backlog.md` holds what is missing.)
const LITE_INVENTORY: (&str, &str) = (
    "LITE INVENTORY",
    "An inventory like the S.T.A.L.K.E.R. series': no storage slots of its own on each item, the right side one list that grows as things are put in, the left side whatever is being looted (a crate, a body); a dropped backpack or rig holds nothing and can only be picked up; a rig adds quick slots (1 to 3 without one, more by how large it was), a backpack adds carrying weight. Designed, not built.",
);

// ==== The page's components ====

#[derive(Component)]
pub struct OptionsPage;
#[derive(Component)]
struct SubTab(usize);
#[derive(Component)]
struct KeyCell(Cell);
#[derive(Component)]
struct PressCell(usize);
#[derive(Component)]
struct PressChoice(PressType);
#[derive(Component)]
struct InstallKeys;
#[derive(Component)]
struct InstallDisplay;
#[derive(Component)]
struct VsyncChoice(bool);
#[derive(Component)]
struct TextureChoice(u8);
#[derive(Component)]
struct StreamingChoice(bool);
#[derive(Component)]
struct ShadowChoice(u8);
#[derive(Component)]
struct SsaoChoice(trp_sim::hbao::SsaoMode);
#[derive(Component)]
struct CloudsChoice(trp_sim::clouds::CloudsQuality);
#[derive(Component)]
struct RenderThreadChoice(bool);
#[derive(Component)]
struct FpsLimitOffChoice(bool);
#[derive(Component)]
struct MsaaChoice(u8);
#[derive(Component)]
struct RendererChoice(crate::backend::Renderer);
/// A choice of the GAME page: which setting (0 the quick bar, 1 the stamina
/// and stance, 2 the health condition) and when it shows; or the health's
/// colours (green to red or not).
#[derive(Component)]
enum GameChoice {
    Shows(u8, crate::battle_view::Shows),
    Colors(bool),
}
#[derive(Component)]
struct InstallGame;
/// A choice of the SOUND page: which volume (0 the overall, 1 the
/// interface's) and its step, 0 to 10.
#[derive(Component)]
struct SoundChoice(u8, u8);
#[derive(Component)]
struct InstallSound;
#[derive(Component)]
struct ModeChoice(Mode);
#[derive(Component)]
struct ResolutionChoice([u32; 2]);
/// A bar that is slid, the part of it that is filled, and its figure.
#[derive(Component)]
struct SlideBar(Slide);
#[derive(Component)]
struct SlideFill(Slide);
#[derive(Component)]
struct SlideValue(Slide);
/// One of the two switches that turn an axis of the mouse about.
#[derive(Component)]
struct InvertChoice(usize);

// ==== The feature switches: the three lists ====

/// A switch of `Features`.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum FeatureSwitch {
    Movement,
    Vaulting,
    Stamina,
    StaminaDrain,
    Weight,
    Exhaustion,
    WeaponMotion,
    Recoil,
    WeaponHandling,
    FirstPersonBody,
    Inventory,
    WorldInteractions,
    Health,
    Skills,
    ItemPhysics,
    Sounds,
    MovementSounds,
    HandsSounds,
    ShotSounds,
    WorldSounds,
    GrenadesAndKnife,
    Ballistics,
    Dummy,
    ReactiveTargets,
    Attachments,
    Workbench,
    Clothing,
    Pip,
    TacticalDevices,
    BlindFire,
    Mounting,
    LeftShoulder,
    Durability,
    Malfunctions,
    Overheating,
    CustomHands,
    SmoothAnimations,
    GroundedFeet,
    VisualEffects,
    SpentCases,
    MagazineRounds,
    FaceShieldMask,
    MuzzleFlash,
    Particles,
    CustomsLight,
    CameraColorFilter,
    CameraSharpening,
    SkyScattering,
    CustomsClouds,
    AmbientHighlight,
    TarkovShading,
    AmbientOcclusion,
    HallLamps,
    IconGeneration,
    CompatibleHighlight,
    CharacterModel,
    TarkovUi,
}

impl FeatureSwitch {
    /// Every switch, in the order of the options' file: the built systems
    /// of the older list in its order, then the app's own, the three
    /// groups of `SIMPLE` (0.101.0) each before its parts.
    const ALL: [Self; 57] = [
        Self::Movement,
        Self::Vaulting,
        Self::Stamina,
        Self::StaminaDrain,
        Self::Weight,
        Self::Exhaustion,
        Self::WeaponMotion,
        Self::Recoil,
        Self::WeaponHandling,
        Self::FirstPersonBody,
        Self::Inventory,
        Self::WorldInteractions,
        Self::Health,
        Self::Skills,
        Self::ItemPhysics,
        Self::Sounds,
        Self::MovementSounds,
        Self::HandsSounds,
        Self::ShotSounds,
        Self::WorldSounds,
        Self::GrenadesAndKnife,
        Self::Ballistics,
        Self::Dummy,
        Self::ReactiveTargets,
        Self::Attachments,
        Self::Workbench,
        Self::Clothing,
        Self::Pip,
        Self::TacticalDevices,
        Self::BlindFire,
        Self::Mounting,
        Self::LeftShoulder,
        Self::Durability,
        Self::Malfunctions,
        Self::Overheating,
        Self::CustomHands,
        Self::SmoothAnimations,
        Self::GroundedFeet,
        Self::VisualEffects,
        Self::SpentCases,
        Self::MagazineRounds,
        Self::FaceShieldMask,
        Self::MuzzleFlash,
        Self::Particles,
        Self::CustomsLight,
        Self::CameraColorFilter,
        Self::CameraSharpening,
        Self::SkyScattering,
        Self::CustomsClouds,
        Self::AmbientHighlight,
        Self::TarkovShading,
        Self::AmbientOcclusion,
        Self::HallLamps,
        Self::IconGeneration,
        Self::CompatibleHighlight,
        Self::CharacterModel,
        Self::TarkovUi,
    ];

    /// Its name in the options file.
    fn key(self) -> &'static str {
        match self {
            Self::Movement => "movement",
            Self::Vaulting => "vaulting_and_climbing",
            Self::Stamina => "stamina",
            Self::StaminaDrain => "stamina_drain",
            Self::Weight => "weight",
            Self::Exhaustion => "exhaustion",
            Self::WeaponMotion => "weapon_motion",
            Self::Recoil => "recoil",
            Self::WeaponHandling => "weapon_handling",
            Self::FirstPersonBody => "first_person_body",
            Self::Inventory => "inventory",
            Self::WorldInteractions => "world_interactions",
            Self::Health => "health",
            Self::Skills => "skills",
            Self::ItemPhysics => "item_physics",
            Self::Sounds => "sounds",
            Self::MovementSounds => "movement_sounds",
            Self::HandsSounds => "hands_sounds",
            Self::ShotSounds => "shot_sounds",
            Self::WorldSounds => "world_sounds",
            Self::GrenadesAndKnife => "grenades_and_knife",
            Self::Ballistics => "ballistics",
            Self::Dummy => "dummy",
            Self::ReactiveTargets => "reactive_targets",
            Self::Attachments => "advanced_attachment_system",
            Self::Workbench => "workbench",
            Self::Clothing => "clothing",
            Self::Pip => "pip",
            Self::TacticalDevices => "tactical_devices",
            Self::BlindFire => "blind_fire",
            Self::Mounting => "mounting",
            Self::LeftShoulder => "left_shoulder",
            Self::Durability => "durability",
            Self::Malfunctions => "malfunctions",
            Self::Overheating => "overheating",
            Self::CustomHands => "custom_first_person_hands",
            Self::SmoothAnimations => "smooth_first_person_animations",
            Self::GroundedFeet => "grounded_feet",
            Self::VisualEffects => "visual_effects",
            Self::SpentCases => "spent_cases",
            Self::MagazineRounds => "magazine_rounds",
            Self::FaceShieldMask => "face_shield_mask",
            Self::MuzzleFlash => "muzzle_flash",
            Self::Particles => "particles",
            Self::CustomsLight => "customs_light",
            Self::CameraColorFilter => "camera_color_filter",
            Self::CameraSharpening => "camera_sharpening",
            Self::SkyScattering => "sky_scattering",
            Self::CustomsClouds => "customs_clouds",
            Self::AmbientHighlight => "ambient_highlight",
            Self::TarkovShading => "tarkov_shading",
            Self::AmbientOcclusion => "ambient_occlusion",
            Self::HallLamps => "hall_lamps",
            Self::IconGeneration => "icon_generation",
            Self::CompatibleHighlight => "compatible_items_highlight",
            Self::CharacterModel => "character_model",
            Self::TarkovUi => "tarkov_ui",
        }
    }

    fn slot(self, features: &mut Features) -> &mut bool {
        match self {
            Self::Movement => &mut features.movement,
            Self::Vaulting => &mut features.vaulting,
            Self::Stamina => &mut features.stamina,
            Self::StaminaDrain => &mut features.stamina_drain,
            Self::Weight => &mut features.weight,
            Self::Exhaustion => &mut features.exhaustion,
            Self::WeaponMotion => &mut features.weapon_motion,
            Self::Recoil => &mut features.recoil,
            Self::WeaponHandling => &mut features.weapon_handling,
            Self::FirstPersonBody => &mut features.first_person_body,
            Self::Inventory => &mut features.inventory,
            Self::WorldInteractions => &mut features.world_interactions,
            Self::Health => &mut features.health,
            Self::Skills => &mut features.skills,
            Self::ItemPhysics => &mut features.item_physics,
            Self::Sounds => &mut features.sounds,
            Self::MovementSounds => &mut features.movement_sounds,
            Self::HandsSounds => &mut features.hands_sounds,
            Self::ShotSounds => &mut features.shot_sounds,
            Self::WorldSounds => &mut features.world_sounds,
            Self::GrenadesAndKnife => &mut features.grenades_and_knife,
            Self::Ballistics => &mut features.ballistics,
            Self::Dummy => &mut features.dummy,
            Self::ReactiveTargets => &mut features.reactive_targets,
            Self::Attachments => &mut features.attachments,
            Self::Workbench => &mut features.workbench,
            Self::Clothing => &mut features.clothing,
            Self::Pip => &mut features.pip,
            Self::TacticalDevices => &mut features.tactical_devices,
            Self::BlindFire => &mut features.blind_fire,
            Self::Mounting => &mut features.mounting,
            Self::LeftShoulder => &mut features.left_shoulder,
            Self::Durability => &mut features.durability,
            Self::Malfunctions => &mut features.malfunctions,
            Self::Overheating => &mut features.overheating,
            Self::CustomHands => &mut features.custom_hands,
            Self::SmoothAnimations => &mut features.smooth_animations,
            Self::GroundedFeet => &mut features.grounded_feet,
            Self::VisualEffects => &mut features.visual_effects,
            Self::SpentCases => &mut features.spent_cases,
            Self::MagazineRounds => &mut features.magazine_rounds,
            Self::FaceShieldMask => &mut features.face_shield_mask,
            Self::MuzzleFlash => &mut features.muzzle_flash,
            Self::Particles => &mut features.particles,
            Self::CustomsLight => &mut features.customs_light,
            Self::CameraColorFilter => &mut features.camera_color_filter,
            Self::CameraSharpening => &mut features.camera_sharpening,
            Self::SkyScattering => &mut features.sky_scattering,
            Self::CustomsClouds => &mut features.customs_clouds,
            Self::AmbientHighlight => &mut features.ambient_highlight,
            Self::TarkovShading => &mut features.tarkov_shading,
            Self::AmbientOcclusion => &mut features.ambient_occlusion,
            Self::HallLamps => &mut features.hall_lamps,
            Self::IconGeneration => &mut features.icon_generation,
            Self::CompatibleHighlight => &mut features.compatible_highlight,
            Self::CharacterModel => &mut features.character_model,
            Self::TarkovUi => &mut features.tarkov_ui,
        }
    }

    /// Its name on the tab, and what it does.
    fn words(self) -> (&'static str, &'static str) {
        match self {
            Self::Movement => (
                "MOVEMENT SYSTEM",
                "Tarkov's walk, inertia, sprint, stance, lean, step aside, jump and crawl. Off, none of their keys is heard and the character stays where it stands; the view still turns.",
            ),
            Self::Vaulting => ("VAULTING AND CLIMBING", "The jump key held takes the character over or onto what stands ahead. Off, it only jumps."),
            Self::Stamina => (
                "STAMINA",
                "The legs' and the arms' stamina and the breath, as Tarkov has them: what drains and refills them (STAMINA DRAIN) and what spent stamina does (EXHAUSTION), together. Off, both are off: nothing is paid for, and nothing holds the character back. Each alone: under ADVANCED.",
            ),
            Self::StaminaDrain => ("STAMINA DRAIN", "The legs' and the arms' stamina and the breath, what drains and refills them. Off, nothing is paid for and they stay full. Part of STAMINA under SIMPLE."),
            Self::Weight => ("WEIGHT", "Carried weight slows the character, adds inertia and costs stamina. Off, what is carried weighs nothing to them."),
            Self::Exhaustion => (
                "EXHAUSTION",
                "What spent stamina does: the sway, no sprint and no jump on spent legs, the breath let go. Off, the pools still drain, and running out holds nothing back. Part of STAMINA under SIMPLE.",
            ),
            Self::WeaponMotion => (
                "WEAPON MOTION",
                "The springs, sway, breathing, the walk's bob, the lean's roll and the turn-away from walls. Off, the weapon moves only by its animation and its recoil.",
            ),
            Self::Recoil => ("RECOIL", "The climb, the kick back and the camera's kick of a shot. Off, a shot moves neither the weapon nor the camera."),
            Self::WeaponHandling => (
                "WEAPON HANDLING",
                "Firing, reloading, the checks, the fire mode, the stock, drawing and putting away. Off, none of the weapon's keys is heard; the sights still come up.",
            ),
            Self::FirstPersonBody => ("FIRST-PERSON BODY", "The legs and the arms drawn, the upper body as a shadow. Off, none of it is drawn for the character's own eyes."),
            Self::Inventory => (
                "INVENTORY SYSTEM",
                "The equipment's slots, containers and their grids, moving things, the quick-use bar. Off, the screen opens on these options alone, and nothing is taken up, bound or moved.",
            ),
            Self::WorldInteractions => ("WORLD INTERACTIONS", "Doors, containers, a trunk, a kick, a lock and its key, things on the ground. Off, nothing is looked at to be worked."),
            Self::Health => (
                "HEALTH SYSTEM",
                "The body's seven parts and their hit points; bleeding, fractures, pain and tremor; hunger and thirst; falls, barbed wire and death; and what each does to moving, stamina and the weapon. Off, the character cannot be hurt and is taken as fed and watered.",
            ),
            Self::Skills => (
                "SKILLS",
                "Tarkov's character skills: the level you set for each on the SKILLS page (0 to 51, the last elite) changes what Tarkov's own code says it changes, where the app has it: stamina and the breath (Endurance), the top speeds, the jump, the weight's limits, aiming's cost, a throw and a knife's blow (Strength), bleeding, fractures, pain, hunger, thirst, food and painkillers (the health skills), the steps' and the kit's noise (Covert Movement), a weapon's reload, draw, ergonomics and aiming speed (its class, Aim Drills), magazines (Mag Drills). No points are earned here: the levels are yours, or a host game's. Off, every skill counts as untrained whatever its level.",
            ),
            Self::ItemPhysics => (
                "ITEM PHYSICS",
                "Things put down from the inventory and backpacks dropped are thrown as Tarkov throws them, and a quick reload's magazine falls from the weapon, each with its own model and colliders in Tarkov's own physics engine (PhysX, set up as the game's). Off, a thing lies as a plain box where it is put.",
            ),
            Self::Sounds => (
                "SOUNDS",
                "What the character hears of itself, from Tarkov's own banks: the steps and the kit (MOVEMENT SOUNDS), the hands' work on the weapon and on items (HANDS SOUNDS) and the shots (SHOT SOUNDS), together. Off, all three are off and none of their banks is read. Each alone: under ADVANCED.",
            ),
            Self::MovementSounds => (
                "MOVEMENT SOUNDS",
                "Footsteps by what is underfoot (twenty surfaces, from Tarkov's own banks: a walking and a running clip blended by speed), the sprint's steps, the stop, the jump, the landing, the crawl, a turn on the spot and the kit's rattle, when and as loud as Tarkov plays them for the player. Off, they are not read and nothing plays. Part of SOUNDS under SIMPLE.",
            ),
            Self::HandsSounds => (
                "HANDS SOUNDS",
                "What is in the hands sounds as its animation marks it, from its own sounds in Tarkov's files: a weapon drawn and put away, a magazine out and in, the bolt, the selector, a hand on the weapon; an AI-2 opened, its syringe taken, injected and thrown away. Off, they are not read and nothing plays. Part of SOUNDS under SIMPLE.",
            ),
            Self::ShotSounds => (
                "SHOT SOUNDS",
                "A weapon's shots from its own banks in Tarkov's files, as Tarkov plays them: an automatic weapon's fire as one recorded loop at its rate, ended on the next beat by its tail when the trigger is let go; a bolt action's, a shotgun's or a pistol's each shot whole; the silenced banks with a silencer on; both barrels' own sound when they fire together. Off, they are not read and the shots are silent. Part of SOUNDS under SIMPLE.",
            ),
            Self::WorldSounds => (
                "WORLD SOUNDS",
                "The doors', crates', cases', boxes' and drawers' own sounds as their Tarkov prefabs name them, read from Tarkov's Customs scenes: the opening (one of its open clips and a squeak, at the prefab's volume) as the swing begins, the shutting as the lid comes down, a kick's breach and a failed kick's thud; placed where the thing stands, plain stereo within three metres. Off, the scenes are not read and the things are silent.",
            ),
            Self::GrenadesAndKnife => (
                "GRENADES AND KNIFE",
                "A grenade taken in hand by the grenade's key or a quick-use key, its pin pulled and held on either button and thrown from above or below as the button comes up, flying in Tarkov's own physics engine and going off at its fuse; the knife drawn from its sheath, a slash and a stab with their stamina, and the quick stroke on its key twice. Off, their keys do nothing.",
            ),
            Self::Ballistics => (
                "BALLISTICS",
                "Each shot's bullets flown as Tarkov flies them, slowed by the air and pulled down, meeting the range's walls, crates, doors and ground, each made of one of Tarkov's materials, and going through, turning, breaking up, glancing off or stopping by Tarkov's own rules and chances, the pieces flying on in turn; and each hit plays what Tarkov plays for its material: its hole, its dust, chips, splinters or sparks, its flash of light and its sound, and a ricochet its own whine. Off, a shot makes no bullet.",
            ),
            Self::Dummy => (
                "TARGET DUMMIES",
                "Two PMCs down the lanes to shoot at, each holding an AKM: one standing in Tarkov's idle, the other walking to its right and its left. Tarkov's own hit zones, armour plates and health, and their deaths as Tarkov's ragdoll, the rifle let go. They wear a fresh copy of what you wear; put on armour and a helmet from the cabinets and press DRESS AS YOU on the F1 panel to dress them again, which also makes them whole and stands them up. Every hit they take is written to the log. Off, they are not there.",
            ),
            Self::ReactiveTargets => (
                "REACTIVE TARGETS",
                "Steel poppers from Tarkov's hideout shooting range, in rows of three down the lanes at 25, 50, 100, 200 and 300 metres past the knee-high wall, each row with a sign saying how far it is. A hit on the round face knocks the plate down and it stands up again 3 seconds later; a hit on its neck or edge only rocks it, with Tarkov's own timing and sounds. Off, there are none.",
            ),
            Self::Attachments => (
                "ATTACHMENTS",
                "Tarkov's attachment system: every weapon is made from the parts it has: a sight, a grip or a magazine put on or taken off shows on the gun in your hands, in its picture and on the ground, and the gun handles by its parts (its weight, ergonomics and recoil). Tarkov's own rules for what goes where. Off, every weapon is its stock build whatever is put on it.",
            ),
            Self::Workbench => (
                "WORKBENCH",
                "Tarkov's own workbench from the hideout (its third level, read from your install) where the clothing crate stood, against the knee-high wall. Stand before it and its menu offers EDIT BUILD for the gun in your hands: Tarkov's screen for changing every part of it, which builds the gun at once (no trading, no stash). Tarkov lets no gun be changed so in a raid; this is the range's. Off, it is not there.",
            ),
            Self::Pip => (
                "PIP",
                "Picture-in-picture scopes, as Tarkov draws them: a second camera inside the scope draws the world at the scope's magnification, and the lens shows that picture with its reticle and its dark rim, which slides as your eye moves off the lens's axis. Red dots and holographic sights draw their dot either way. Off, no second camera is made: the whole view zooms by the scope's magnification instead, as in STALKER's 3D scopes, while the weapon keeps its size and the lens is clear glass with its reticle.",
            ),
            Self::TacticalDevices => (
                "TACTICAL DEVICES",
                "Tarkov's weapon lights, lasers and head lights. T switches every device on the weapon, Ctrl+T steps their modes, H and Ctrl+H do the same for a light on the helmet (the hand goes up to it, and it switches 0.8 s later), and T+B changes T between Press and Hold, as Tarkov's keys do. Each device lights the range as Tarkov's light does, with its cone, its picture and its shadows, its light shaft and its flare; a laser draws its beam and its dot; infrared ones show nothing without night vision. A combo unit switched on steadies the gun's wander from the hip, as in Tarkov. Off, the keys do nothing and nothing of them is read or drawn.",
            ),
            Self::BlindFire => (
                "BLIND FIRE",
                "Tarkov's blind fire: hold Alt+W to hold the gun up over cover, Alt+S to hold it out to the right, and fire without the sights; letting go brings it back. As in Tarkov it starts only standing (or crouched) still with the gun at rest, not while walking, in a shot, a reload or a jump; it drops the sights and holds back the aim, the sight and fire mode keys, the looks at the gun and every reload, and you cannot walk or vault until you let go. The body plays Tarkov's own animations, the camera tilts up or to the side as Tarkov's does, and a wall just in front is cleared by the gun held over it. Off, the keys do nothing.",
            ),
            Self::Mounting => (
                "MOUNTING",
                "Tarkov's weapon mounting and bipod: V rests the gun on a sill, a wall's top or the edge of a doorway in front of you (Tarkov looks for one ahead within 1.2 m, between hip and chest height, or at a wall's corner beside you); the character steps up to it, the gun settles on it, and looking around then leans the body and moves its height with the look, as far as the cover allows. Recoil and sway are lower on it, more so on a bipod. Moving, leaning, jumping, crouching, reloading or V again takes the gun off; lying down, vaulting or opening the inventory at once. Ctrl+V puts a bipod out or folds it (on a gun that has one); lying down with it out, V rests the gun on it. Off, both keys do nothing.",
            ),
            Self::LeftShoulder => (
                "LEFT SHOULDER",
                "Tarkov's left shoulder: the side mouse button (Mouse4) carries the gun on your left shoulder, and again on the right, as in Tarkov. The body plays Tarkov's own move, the gun follows it over to the left side, and aiming brings the sights in front of your left eye. As in Tarkov it switches only with the gun at rest, not lying down, in blind fire or mounted; sprinting, jumping, lying down, vaulting something high, a reload or the inventory take the gun back to the right for as long as they last, and it comes back after; a mount turns it off. Off, the key does nothing.",
            ),
            Self::Durability => (
                "DURABILITY",
                "Tarkov's weapon wear: every shot wears the gun by its round, more as the gun runs hot, and a gun run very hot loses some of its greatest durability for good; your Weapon Maintenance skill lowers it. A worn gun malfunctions more often. Its durability shows in the inventory, as in Tarkov, and a repair kit at the workbench mends it. Off, guns do not wear.",
            ),
            Self::Malfunctions => (
                "MALFUNCTIONS",
                "Tarkov's weapon malfunctions: every shot has Tarkov's chance of a misfire, a failure to eject, a failure to feed or a jammed bolt, from the gun's durability, its round, its magazine and its heat. The gun stops as in Tarkov; the trigger then only clicks, L looks at the gun to learn what went wrong, and Shift+T clears it with Tarkov's own animation, quicker with your Troubleshooting skill. Off, guns never malfunction.",
            ),
            Self::Overheating => (
                "OVERHEATING",
                "Tarkov's overheating: every shot heats the gun by its parts and its round, and it cools from the last shot. Hot, it wears faster, spreads its shots wider, malfunctions more, can change its rate of fire, holds its bolt back, and may fire a round by itself; its barrel glows red, as in Tarkov. Off, guns stay cold.",
            ),
            Self::Clothing => (
                "CLOTHING",
                "The OVERALL tab's two dropdowns of Tarkov's suits, upper and lower, every side's mixed in one list each (Tarkov offers only your side's, and only out of a raid). A suit is read from your install when you first choose it, then put on your body, your model, your corpse and your hands in first person; your choice is kept for the next run. Off, nothing of it is read: the tab has no clothing panel, as in a raid, and you wear the default suits.",
            ),
            Self::CustomHands => (
                "CUSTOM FIRST PERSON HANDS",
                "An outfit that changes the body brings its own arms into first person. Off, the hands do not change with what is worn: they are Tarkov's default arms. Whatever an outfit has not got is Tarkov's default either way.",
            ),
            Self::SmoothAnimations => (
                "SMOOTH FIRST PERSON ANIMATIONS",
                "Every animation of what is in hand, weapons and empty hands, drawn on curves through its clips' samples, where Tarkov's engine joins them by straight lines (the stutter in a look over a weapon). Off, all are drawn as the engine has them, and each pose is worked out once a step instead of twice. Only the drawing changes.",
            ),
            Self::GroundedFeet => (
                "GROUNDED FEET",
                "After a short drop Tarkov's character can come to rest up to 15 cm over the ground, as its capsule does, and its feet with it. On, the body is drawn down onto the ground there; the view and the hands are where Tarkov has them either way. Off, the feet float as in Tarkov. Only the drawing changes.",
            ),
            Self::VisualEffects => (
                "VISUAL EFFECTS",
                "What a shot and a blast show, as Tarkov draws them: the flash at the muzzle and the gun smoke (MUZZLE FLASH), the fumes, sparks and heat shimmer, a launcher's blasts and a grenade's explosion (PARTICLES), the cases thrown out of the weapon (SPENT CASES), a magazine's rounds in its model and picture (MAGAZINE ROUNDS) and a face shield's glass over your view (FACE SHIELD MASK), together. Off, all five are off and nothing of them is read. Each alone: under ADVANCED.",
            ),
            Self::FaceShieldMask => (
                "FACE SHIELD MASK",
                "A face shield worn down, seen from behind it as Tarkov draws it over your view: its frame and the scratches and blur at its edges, its hits' cracks, fading in as it comes down and out as it goes up. Off, nothing of it is read or drawn. Part of VISUAL EFFECTS under SIMPLE.",
            ),
            Self::SpentCases => (
                "SPENT CASES",
                "A fired round's case thrown out of the weapon at its mark in the animation, from its shell port, flying and bouncing as Tarkov's do, gone a second later, with its sound where it strikes. Off, nothing of it is read or drawn. Part of VISUAL EFFECTS under SIMPLE.",
            ),
            Self::MagazineRounds => (
                "MAGAZINE ROUNDS",
                "A magazine's rounds drawn in its model, as Tarkov draws them: at the top of the magazine in your weapon, going as you fire them, in a magazine on the ground, and in a loaded magazine's inventory picture. Off, none is drawn. Part of VISUAL EFFECTS under SIMPLE.",
            ),
            Self::MuzzleFlash => (
                "MUZZLE FLASH",
                "A shot's flash at the muzzle, drawn from the weapon's own flash and its picture as Tarkov draws it, and the gun smoke that trails from the muzzle after a burst. Off, nothing of either is read or drawn. Part of VISUAL EFFECTS under SIMPLE.",
            ),
            Self::Particles => (
                "PARTICLES",
                "Tarkov's particle effects, run as the game runs them: the puffs of smoke each shot throws out of the muzzle and the ejection port, lit, drifting and fading, the glowing sparks a shot throws now and then, the shimmer of hot air at the muzzle, the RShG-2's front and back blasts, and a grenade going off: its fireball, smoke, sparks, dust and bouncing chips of concrete, its flash of light, its shock wave and heat shimmer, the scorch it leaves on the ground, and a flashbang's sparks, smoke and light. Off, nothing of them is read or drawn. Part of VISUAL EFFECTS under SIMPLE.",
            ),
            Self::CustomsLight => (
                "CUSTOMS LIGHT",
                "The range lit as Tarkov's Customs on a clear morning at 09:30 (TIME OF DAY, under this panel): its sun, sky and ambient light worked out by the game's own time of day, with smoke, dust and fire lit by the same, and Tarkov's own camera, which adjusts to how bright the view is, darkens the corners and brings the light to the screen through its film-like curve. Off, the range's own bright light and the app's own camera.",
            ),
            Self::CameraColorFilter => (
                "CAMERA COLOR FILTER",
                "The colour filter Tarkov's camera has on every map as the game ships it (with CUSTOMS LIGHT): it lifts the middle tones and the sky and deepens the darkest a little. This is the camera's own, not the optional colour filter of Tarkov's Post-FX settings, which is off unless you turn it on there. Off, nothing of it is read or drawn.",
            ),
            Self::CameraSharpening => (
                "CAMERA SHARPENING",
                "Tarkov's camera sharpening and colour fade as Customs' camera runs them (with CUSTOMS LIGHT): the picture sharpened at your Tarkov's own Sharpen setting, its colours faded a little toward grey by the weather, more when exhausted, and on painkillers grey closing in from the edges of the view. Off, nothing of it is read or drawn.",
            ),
            Self::SkyScattering => (
                "SKY SCATTERING",
                "The air between you and what you see, as Tarkov's camera draws it on Customs (with CUSTOMS LIGHT): the sky's light scattered over the scene, more the farther away and the lower down, which turns distant things toward the sky's blue. Off, nothing of it is read or drawn.",
            ),
            Self::CustomsClouds => (
                "CUSTOMS CLOUDS",
                "Customs' clouds as Tarkov's weather draws them on that morning (with CUSTOMS LIGHT): the cloud layer over the sky, lit by the sun and the sky, drifting with the wind, and their shadows passing over the ground, at the CLOUDS QUALITY on the GRAPHICS page, which starts as your Tarkov's own. Off, nothing of it is read or drawn.",
            ),
            Self::AmbientHighlight => (
                "AMBIENT HIGHLIGHT",
                "Tarkov's extra light on your own arms and weapon (with CUSTOMS LIGHT): outdoors the game lights them a second time with the sky's light from around and below, up to twice over by how high the sun is, so they do not go dark against the scene. Indoors it adds nothing, as in the game. Off, nothing of it is drawn.",
            ),
            Self::TarkovShading => (
                "TARKOV SHADING",
                "Surfaces lit the way Tarkov's renderer lights them (with CUSTOMS LIGHT): the sun, the sky's light, the hall's lamps and an explosion's flash each worked out by the game's own light, its numbers added as the game adds them. The sun's side comes out brighter and the slopes away from it falling off faster than Bevy's own light has them. Off, Bevy's own shading lights everything, as before.",
            ),
            Self::AmbientOcclusion => (
                "AMBIENT OCCLUSION",
                "Tarkov's own ambient occlusion (its SSAO setting): creases, corners and the ground under things darkened as the game's camera darkens them, at the choice on the GRAPHICS page, which starts as your Tarkov's own. At ULTRA and the like Tarkov adds a second, finer effect, not drawn here yet. Off, nothing of it is drawn, and the scene is not drawn an extra time for it.",
            ),
            Self::HallLamps => (
                "LEVEL LAMPS",
                "The ceiling lamps a level file places (a level made in Blender and run with --level: its lamp objects), white, with Tarkov's industrial lamp's brightness (with CUSTOMS LIGHT), and the light they bounce off the floor and walls inside a level's hall, standing in for the baked light Tarkov's buildings have; how far their light reaches is the app's. The range has no hall and no lamps since 0.100.0, so on it this does nothing. Off, no lamp is made.",
            ),
            Self::IconGeneration => (
                "ICON GENERATION",
                "The inventory's pictures drawn from each item's own model, as Tarkov draws them, once, and kept for later runs (the first run starts a few seconds later while it draws them). Off, none is drawn or read: each box shows the plain shape of its sort.",
            ),
            Self::CompatibleHighlight => (
                "COMPATIBLE ITEMS HIGHLIGHT",
                "With the mouse on a thing in the inventory, or a thing carried, what goes with it is lit orange: a weapon's magazines, rounds and parts, a magazine's weapons and rounds, the weapons and magazines rounds go in. S.T.A.L.K.E.R.'s way; Tarkov has nothing like it. Off, only the thing under the mouse is lit, as in Tarkov.",
            ),
            Self::CharacterModel => (
                "CHARACTER MODEL",
                "The gear page has the character's own model behind its slots, in what it wears, standing as Tarkov's pre-raid screen stands it, hands at its sides, and still. It is drawn from the body already loaded, nothing read twice, and drawn again only when what is worn changes. Off, a paper figure of plain shapes.",
            ),
            Self::TarkovUi => (
                "TARKOV UI",
                "The screens drawn 1:1 as Tarkov's own, from the game's own layout, pictures, fonts and animations read from the install: the inventory (its bar of tabs, the GEAR and HEALTH tabs, the containers, the things, the quick-use panel, the blur behind it) and, in a raid, the stance panel (stance, speed, stamina, noise) and the health silhouette with its effects, fading in and out as your game's own settings say. Off, the app's own look: the character model on the HEALTH tab, the plain meters in play. The figures are the same either way.",
            ),
        }
    }

    /// The `SIMPLE` switch this one is a part of, if any: while that is
    /// off, so is this, whatever its own switch says (`Features::effective`).
    fn group(self) -> Option<Self> {
        match self {
            Self::StaminaDrain | Self::Exhaustion => Some(Self::Stamina),
            Self::MovementSounds | Self::HandsSounds | Self::ShotSounds | Self::WorldSounds => Some(Self::Sounds),
            Self::MuzzleFlash | Self::Particles | Self::SpentCases | Self::MagazineRounds | Self::FaceShieldMask => Some(Self::VisualEffects),
            _ => None,
        }
    }

    fn get(self, features: &Features) -> bool {
        *self.slot(&mut features.clone())
    }

    fn flip(self, features: &mut Features) {
        let on = self.slot(features);
        *on = !*on;
    }
}

// ==== The plugin and the page's parts: controls, game, sound, graphics ====

/// The button that ends the run (`QUIT`, the user's word for it).
#[derive(Component)]
struct ExitButton;
/// The scrolled list of bindings: how high all of it is, in pixels.
#[derive(Component)]
struct ListArea {
    whole: f32,
}

/// The height of a row of the list of bindings, in pixels: the game's 28
/// units on a screen 1080 high, at the size the app's words are.
const ROW: f32 = 26.0;
/// The second of the two shades the list's rows take by turns, a little
/// lighter than a cell's own.
const STRIPE: Color = Color::srgb(0.155, 0.165, 0.165);
/// The bindings that begin a group of their own in the list, which leaves
/// a little room before each: the app's grouping (the user, 2026-10-13),
/// over the game's own order, which is kept. The first group is the
/// axes' and what follows them down to the speed's two; then the weapon's
/// handling, the weapons taken in hand, what is worn and switched, the
/// other actions, the quick-use keys, and the screen's own.
const GROUP_STARTS: [&str; 6] = ["WeaponMounting", "PrimaryWeaponFirst", "ToggleGoggles", "Nidnod", "Slot4", "MakeScreenshot"];
const GROUP_GAP: f32 = 8.0;
/// The shares of the list's width the game's four columns have: what the
/// binding does, its two keys, and how it is pressed.
const COLUMNS: [f32; 4] = [40.0, 20.0, 20.0, 20.0];
const SUBS: [&str; 6] = ["GAME", "CONTROLS", "GRAPHICS", "SOUND", "SKILLS", "FEATURES"];

pub struct OptionsPlugin;

impl Plugin for OptionsPlugin {
    fn build(&self, app: &mut App) {
        // With the inventory's own systems: after the simulation's frames.
        app.add_systems(
            Update,
            (clicks, skill_clicks, hour_clicks, listen, slide, sync_skills, scroll_list, apply, rebuild, describe_features)
                .chain()
                .run_if(crate::running)
                .after(crate::controller::step)
                // After the screen's own keys: Escape that the tab is about
                // to take is seen there as taken.
                .after(crate::inventory_view::ScreenKeys)
                .before(crate::animation::play_clips),
        );
    }
}

fn button(parent: &mut ChildSpawnerCommands, marker: impl Bundle, text: &str, lit: bool, width: Val) {
    let node = Node { width, padding: UiRect::axes(Val::Px(10.0), Val::Px(4.0)), justify_content: JustifyContent::Center, flex_shrink: 0.0, ..default() };
    parent.spawn((marker, Button, node, BackgroundColor(if lit { LINE } else { PANEL }))).with_children(|inside| {
        inside.spawn(words(text, 12.0, WORDS));
    });
}

/// A cell of the list of bindings, so many hundredths of its width wide.
fn cell(share: f32) -> Node {
    Node { width: Val::Percent(share), height: Val::Px(ROW - 3.0), padding: UiRect::horizontal(Val::Px(8.0)), align_items: AlignItems::Center, overflow: Overflow::clip(), ..default() }
}

/// A bar to slide with its figure beside it, as the game's own bar has
/// its figure to its right.
fn slider(row: &mut ChildSpawnerCommands, kind: Slide, value: f32, width: f32) {
    let [low, high] = kind.range();
    let share = ((value - low) / (high - low)).clamp(0.0, 1.0);
    row.spawn((SlideBar(kind), RelativeCursorPosition::default(), Node { width: Val::Px(width), height: Val::Px(16.0), flex_shrink: 0.0, ..default() }, BackgroundColor(LINE))).with_children(|bar| {
        bar.spawn((SlideFill(kind), Node { width: Val::Percent(share * 100.0), height: Val::Percent(100.0), ..default() }, BackgroundColor(WORDS)));
    });
    row.spawn(Node { width: Val::Px(52.0), flex_shrink: 0.0, ..default() }).with_children(|figure| {
        figure.spawn((SlideValue(kind), words(kind.shown(value), 13.0, WORDS)));
    });
}

/// The game's controls tab as its own screen lays it out
/// (`ControlSettingsTab`; settings spec, "The controls tab"): one large
/// box that scrolls, with the axes' two ways each and then every binding
/// the game's own screen shows, in four columns (what it does, its two
/// keys, how it is pressed); and under it the mouse's panel, a row of
/// four bars and a row of two switches.
fn controls_part(page: &mut ChildSpawnerCommands, state: &OptionsState, bindings: &Bindings) {
    let controls = &bindings.controls;
    page.spawn(Node { column_gap: Val::Px(12.0), align_items: AlignItems::Center, flex_shrink: 0.0, ..default() }).with_children(|top| {
        button(top, InstallKeys, "AS THE INSTALL HAS THEM", false, Val::Auto);
        let (said, color) = if state.note.is_empty() {
            ("Click a key's cell, hold the keys wanted and let one go (three at the most; a notch of the wheel may end it; Escape keeps what stood there). The dim commands do nothing here yet.".to_owned(), FAINT)
        } else {
            (state.note.clone(), WORDS)
        };
        top.spawn((Node { flex_shrink: 1.0, ..default() }, words(said, 12.0, color)));
    });
    let boxed = |grows: bool| {
        let node = Node {
            flex_direction: FlexDirection::Column,
            width: Val::Percent(100.0),
            padding: UiRect::all(Val::Px(6.0)),
            border: UiRect::all(Val::Px(1.0)),
            row_gap: Val::Px(3.0),
            flex_grow: if grows { 1.0 } else { 0.0 },
            flex_shrink: if grows { 1.0 } else { 0.0 },
            flex_basis: if grows { Val::Px(0.0) } else { Val::Auto },
            min_height: Val::Px(0.0),
            ..default()
        };
        (node, BackgroundColor(BACK), BorderColor::all(LINE))
    };
    let line = || Node { column_gap: Val::Px(3.0), height: Val::Px(ROW), width: Val::Percent(100.0), flex_shrink: 0.0, ..default() };
    let listened = state.listening.as_ref();
    // A row's cells are all of the row's shade, and the rows take the two
    // shades by turns: the eye keeps to a row across the list's width.
    let key_cell = |row: &mut ChildSpawnerCommands, at: Cell, source: &Source, shade: Color| {
        let text = match listened.filter(|listening| listening.cell == at && !listening.done) {
            Some(listening) if listening.heard.source.is_empty() => "PRESS ANY KEY...".to_owned(),
            Some(listening) => format!("{} + ...", keys::words(&listening.heard.source)),
            None if source.is_empty() => "NOT SET".to_owned(),
            None => keys::words(source),
        };
        let lit = listened.is_some_and(|listening| listening.cell == at);
        let color = if source.is_empty() && !lit { FAINT } else { WORDS };
        row.spawn((KeyCell(at), Button, cell(COLUMNS[1]), BackgroundColor(if lit { LINE } else { shade }))).with_children(|inside| {
            inside.spawn((words(text, 12.0, color), TextLayout::new_with_no_wrap()));
        });
    };
    let named = |row: &mut ChildSpawnerCommands, text: String, built: bool, shade: Color| {
        row.spawn((cell(COLUMNS[0]), BackgroundColor(shade))).with_children(|inside| {
            inside.spawn((words(text, 12.0, if built { WORDS } else { FAINT }), TextLayout::new_with_no_wrap()));
        });
    };
    let shown: Vec<usize> = controls.keys.iter().enumerate().filter(|(_, group)| !group.hidden()).map(|(index, _)| index).collect();
    // A little room before each binding that begins a group of its own.
    let begins = |index: usize| GROUP_STARTS.contains(&controls.keys[index].name.as_str());
    let rows = controls.axes.len() * 2 + shown.len();
    let whole = rows as f32 * ROW + shown.iter().filter(|index| begins(**index)).count() as f32 * GROUP_GAP;
    page.spawn(boxed(true)).with_children(|panel| {
        // The columns' heads stand over the list and do not scroll.
        panel.spawn(line()).with_children(|row| {
            for (share, text) in COLUMNS.into_iter().zip(["ACTION", "KEY", "KEY", "PRESS TYPE"]) {
                row.spawn(cell(share)).with_children(|inside| {
                    inside.spawn(words(text, 11.0, FAINT));
                });
            }
        });
        let area = Node { flex_direction: FlexDirection::Column, flex_grow: 1.0, flex_basis: Val::Px(0.0), min_height: Val::Px(0.0), width: Val::Percent(100.0), overflow: Overflow::scroll_y(), ..default() };
        panel.spawn((ListArea { whole }, area, ScrollPosition(Vec2::new(0.0, state.scroll.max(0.0))), RelativeCursorPosition::default())).with_children(|list| {
            // How many rows the group in hand has so far: its first is of
            // the darker shade.
            let mut in_group = 0;
            let mut shade = || {
                in_group += 1;
                if in_group % 2 == 1 { PANEL } else { STRIPE }
            };
            for (index, axis) in controls.axes.iter().enumerate() {
                let ways = AXIS_WORDS.iter().find(|(name, _)| *name == axis.name).map_or(["+", "-"], |(_, ways)| *ways);
                for (way, word) in ways.into_iter().enumerate() {
                    let shade = shade();
                    list.spawn(line()).with_children(|row| {
                        named(row, word.to_owned(), keys::BUILT.contains(&axis.name.as_str()), shade);
                        key_cell(row, Cell::Axis(index, 0, way), &axis.pairs[0][way], shade);
                        key_cell(row, Cell::Axis(index, 1, way), &axis.pairs[1][way], shade);
                        row.spawn((cell(COLUMNS[3]), BackgroundColor(shade)));
                    });
                }
            }
            let mut in_group = 2 * controls.axes.len();
            for index in shown {
                let group = &controls.keys[index];
                if begins(index) {
                    list.spawn(Node { height: Val::Px(GROUP_GAP), flex_shrink: 0.0, ..default() });
                    in_group = 0;
                }
                in_group += 1;
                let shade = if in_group % 2 == 1 { PANEL } else { STRIPE };
                list.spawn(line()).with_children(|row| {
                    let built = keys::BUILT.contains(&group.name.as_str());
                    named(row, state.name_of(&group.name), built, shade);
                    key_cell(row, Cell::Key(index, 0), &group.variants[0], shade);
                    // How it is pressed: chosen from the four where the
                    // game lets it be chosen (its list of them, laid over
                    // the second key's cell while it is open), and not
                    // shown for a binding with no keys.
                    if state.asking == Some(index) {
                        let room = Node { width: Val::Percent(COLUMNS[2] + COLUMNS[3]), column_gap: Val::Px(3.0), justify_content: JustifyContent::FlexEnd, ..default() };
                        row.spawn(room).with_children(|choices| {
                            for press in PressType::ALL {
                                let node = Node { padding: UiRect::horizontal(Val::Px(7.0)), height: Val::Px(ROW - 3.0), align_items: AlignItems::Center, ..default() };
                                choices.spawn((PressChoice(press), Button, node, BackgroundColor(if press == group.press { FAINT } else { LINE }))).with_children(|inside| {
                                    inside.spawn((words(state.press_word(press), 11.0, WORDS), TextLayout::new_with_no_wrap()));
                                });
                            }
                        });
                        return;
                    }
                    key_cell(row, Cell::Key(index, 1), &group.variants[1], shade);
                    if group.is_empty() {
                        row.spawn((cell(COLUMNS[3]), BackgroundColor(shade)));
                    } else if group.press_shown() {
                        row.spawn((PressCell(index), Button, cell(COLUMNS[3]), BackgroundColor(shade))).with_children(|inside| {
                            inside.spawn(words(state.press_word(group.press), 12.0, WORDS));
                        });
                    } else {
                        row.spawn((cell(COLUMNS[3]), BackgroundColor(shade))).with_children(|inside| {
                            inside.spawn(words(state.press_word(group.press), 12.0, FAINT));
                        });
                    }
                });
            }
        });
    });
    // The mouse's panel: the four bars in a row, each under its name, and
    // the two switches under them.
    page.spawn(boxed(false)).with_children(|panel| {
        panel.spawn(Node { justify_content: JustifyContent::SpaceAround, width: Val::Percent(100.0), flex_wrap: FlexWrap::Wrap, row_gap: Val::Px(6.0), ..default() }).with_children(|bars| {
            for kind in [Slide::Sensitivity, Slide::Aiming, Slide::DoubleClick, Slide::Optic] {
                bars.spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(4.0), align_items: AlignItems::FlexStart, ..default() }).with_children(|bar| {
                    bar.spawn(words(state.said(kind.caption().0), 12.0, WORDS));
                    bar.spawn(Node { column_gap: Val::Px(10.0), align_items: AlignItems::Center, ..default() }).with_children(|row| {
                        slider(row, kind, state.slider(kind, controls), 240.0);
                    });
                });
            }
        });
        panel.spawn(Node { justify_content: JustifyContent::Center, column_gap: Val::Px(48.0), width: Val::Percent(100.0), margin: UiRect::top(Val::Px(6.0)), ..default() }).with_children(|switches| {
            for (axis, name) in ["Inverted X axis", "Inverted Y axis"].into_iter().enumerate() {
                let on = state.mouse.invert[axis];
                let node = Node { column_gap: Val::Px(8.0), align_items: AlignItems::Center, padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)), ..default() };
                switches.spawn((InvertChoice(axis), Button, node, BackgroundColor(BACK))).with_children(|switch| {
                    let tick = Node { width: Val::Px(18.0), height: Val::Px(18.0), border: UiRect::all(Val::Px(1.0)), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() };
                    switch.spawn((tick, BorderColor::all(LINE), BackgroundColor(if on { FAINT } else { PANEL }))).with_children(|mark| {
                        mark.spawn(words(if on { "X" } else { "" }, 12.0, WORDS));
                    });
                    switch.spawn(words(state.said(name), 12.0, WORDS));
                });
            }
        });
    });
}

/// The display: the four of the game's settings the user named.
/// The game's own GAME tab, of it what the app has (the user, 2026-10-16:
/// "We need to add a setting for auto-hiding, always showing, or never
/// showing the in-raid HUD (HP doll, Stance doll, Quick slots) like tarkov
/// does. We may need a "Game" cateogry in options just like tarkov has
/// it"): when the battle screen's quick bar, stance panel and health panel
/// show (`QuickSlotsVisibility`, `StaminaVisibility`, `HealthVisibility`),
/// and the health's colours (`HealthColor`), in the game's words, laid out
/// as the app's other pages are (the user: "We dont need to copy tarkov
/// 1:1 exactly here").
fn game_part(page: &mut ChildSpawnerCommands, state: &OptionsState) {
    use crate::battle_view::Shows;
    let game = &state.game;
    let line = |page: &mut ChildSpawnerCommands, caption: &str, inside: &mut dyn FnMut(&mut ChildSpawnerCommands)| {
        page.spawn(Node { column_gap: Val::Px(6.0), align_items: AlignItems::Center, flex_wrap: FlexWrap::Wrap, row_gap: Val::Px(4.0), ..default() }).with_children(|row| {
            row.spawn(Node { width: Val::Px(240.0), flex_shrink: 0.0, ..default() }).with_children(|name| {
                name.spawn(words(caption, 12.0, FAINT));
            });
            inside(row);
        });
    };
    for (which, (key, now)) in [("Ui/Settings/QuickSlotsVisibility", game.quick), ("Ui/Settings/StanimaVisibility", game.stamina), ("Ui/Settings/HeathVisibility", game.health)].into_iter().enumerate() {
        line(page, &state.said(key), &mut |row| {
            for shows in Shows::ALL {
                button(row, GameChoice::Shows(which as u8, shows), &state.said(&format!("Ui/Settings/Visibility/{}", shows.name())), now == shows, Val::Px(150.0));
            }
        });
    }
    line(page, &state.said("Ui/Settings/HealthColorScheme"), &mut |row| {
        for green in [false, true] {
            let name = if green { "GreenToRed" } else { "WhiteToRed" };
            button(row, GameChoice::Colors(green), &state.said(&format!("Ui/Settings/ColorScheme/{name}")), game.green_to_red == green, Val::Px(150.0));
        }
    });
    page.spawn(words("The panels of the screen in a raid with TARKOV UI on: the quick slots at the top, the stance and stamina at the lower left, the health at the upper left. Each change is taken when the inventory is shut, as the game's panels take theirs when the screen is next shown.", 11.0, FAINT));
    let theirs = &state.install_game;
    page.spawn(Node { column_gap: Val::Px(12.0), align_items: AlignItems::Center, margin: UiRect::top(Val::Px(10.0)), ..default() }).with_children(|row| {
        button(row, InstallGame, "AS THE INSTALL HAS THEM", false, Val::Auto);
        let said = format!(
            "the install's own game: quick slots {}, stamina and stance {}, health {}, {}",
            state.said(&format!("Ui/Settings/Visibility/{}", theirs.quick.name())).to_lowercase(),
            state.said(&format!("Ui/Settings/Visibility/{}", theirs.stamina.name())).to_lowercase(),
            state.said(&format!("Ui/Settings/Visibility/{}", theirs.health.name())).to_lowercase(),
            state.said(&format!("Ui/Settings/ColorScheme/{}", theirs.color_name())).to_lowercase(),
        );
        row.spawn(words(said, 11.0, FAINT));
    });
}

/// The game's own SOUND tab, of it what the app's sounds go by: the
/// overall volume and the interface's, in the game's eleven steps
/// (`SoundSettingsGroup`: 0 to 10, -80 to 0 decibels), as the install has
/// them until changed, each taken at once (`ui_sound`).
fn sound_part(page: &mut ChildSpawnerCommands, state: &OptionsState) {
    for (which, (key, now)) in [("Settings/Sound/OverallVolume", state.sound.overall), ("Settings/Sound/InterfaceVolume", state.sound.interface)].into_iter().enumerate() {
        page.spawn(Node { column_gap: Val::Px(6.0), align_items: AlignItems::Center, flex_wrap: FlexWrap::Wrap, row_gap: Val::Px(4.0), ..default() }).with_children(|row| {
            row.spawn(Node { width: Val::Px(240.0), flex_shrink: 0.0, ..default() }).with_children(|name| {
                name.spawn(words(state.said(key).trim_end_matches(':').to_owned(), 12.0, FAINT));
            });
            for step in 0..=10u8 {
                button(row, SoundChoice(which as u8, step), &step.to_string(), now == step, Val::Px(44.0));
            }
            row.spawn(words(format!("{:.0} dB", crate::ui_sound::Levels::DECIBELS[now.min(10) as usize]), 11.0, FAINT));
        });
    }
    page.spawn(words("The interface's sounds play at both volumes; a thing's (taken, put down, put on) at the overall volume and at 0.15 of it in a raid, as the game's. Each change is heard at once.", 11.0, FAINT));
    let theirs = &state.install_sound;
    page.spawn(Node { column_gap: Val::Px(12.0), align_items: AlignItems::Center, margin: UiRect::top(Val::Px(10.0)), ..default() }).with_children(|row| {
        button(row, InstallSound, "AS THE INSTALL HAS THEM", false, Val::Auto);
        row.spawn(words(format!("the install's own game: overall {}, interface {}", theirs.overall, theirs.interface), 11.0, FAINT));
    });
}

/// The SKILLS page's button that puts every skill back to 0.
#[derive(Component)]
struct SkillsUntrained;

/// What a skill changes in the app as it stands, or why nothing
/// (`skills_view::acts_here`, which the SKILLS tab dims by).
fn acts_here(skill: trp_sim::skills::Skill) -> &'static str {
    match crate::skills_view::acts_here(skill) {
        Ok(what) | Err(what) => what,
    }
}

/// A skill's name in the install's words, in capitals, or the app's.
fn skill_word(state: &OptionsState, skill: trp_sim::skills::Skill) -> String {
    match skill {
        trp_sim::skills::Skill::TroubleShooting if !state.names.contains_key("troubleshooting") => "TROUBLESHOOTING".to_owned(),
        _ => state.name_of(skill.name()),
    }
}

/// The game's skill page, of it what the app has (the user, 2026-10-23:
/// "a page in the options tab where I set each skill's level 0 to 51"):
/// each skill of the game's list by its class, its level on a bar (51,
/// elite), and faint, what it changes here. A level is taken at once (a
/// weapon takes its class's and Aim Drills' as it is next drawn, as the
/// game's); one a host game sets in play shows here and is kept the same
/// way. No points are earned. The locked skills, which have nothing to
/// change, are named at the end.
fn skills_part(page: &mut ChildSpawnerCommands, state: &OptionsState) {
    use trp_sim::skills::{Class, Skill};
    page.spawn(Node { column_gap: Val::Px(12.0), align_items: AlignItems::Center, flex_wrap: FlexWrap::Wrap, ..default() }).with_children(|top| {
        button(top, SkillsUntrained, "ALL TO 0", false, Val::Auto);
        let said = if !state.in_force.skills {
            "SKILLS is off under FEATURES: every skill counts as untrained this run, whatever its level here.".to_owned()
        } else {
            "Each level is taken at once; a weapon takes its class's and Aim Drills' as it is next drawn, as in Tarkov. Nothing is earned: a host game may set levels in play, and they show and are kept here.".to_owned()
        };
        top.spawn((Node { flex_shrink: 1.0, ..default() }, words(said, 12.0, FAINT)));
        if state.features.skills != state.started.skills {
            top.spawn(words("RESTART REQUIRED", 10.0, WORDS));
        }
    });
    let shown = |class: Class| Skill::ALL.into_iter().filter(move |skill| skill.class() == class && !skill.locked());
    page.spawn(Node { column_gap: Val::Px(24.0), row_gap: Val::Px(10.0), flex_wrap: FlexWrap::Wrap, margin: UiRect::top(Val::Px(8.0)), ..default() }).with_children(|columns| {
        let column = Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(1.0), width: Val::Px(560.0), flex_shrink: 0.0, ..default() };
        for classes in [&[Class::Physical, Class::Mental][..], &[Class::Combat], &[Class::Practical]] {
            columns.spawn(column.clone()).with_children(|column| {
                for (n, class) in classes.iter().enumerate() {
                    if n > 0 {
                        column.spawn(Node { height: Val::Px(10.0), ..default() });
                    }
                    section(column, &class.name().to_uppercase());
                    for skill in shown(*class) {
                        column.spawn(Node { column_gap: Val::Px(8.0), align_items: AlignItems::Center, ..default() }).with_children(|row| {
                            row.spawn(Node { width: Val::Px(190.0), flex_shrink: 0.0, ..default() }).with_children(|name| {
                                name.spawn((words(skill_word(state, skill), 12.0, WORDS), TextLayout::new_with_no_wrap()));
                            });
                            slider(row, Slide::Skill(skill), state.skills.level(skill) as f32, 280.0);
                        });
                        column.spawn((Node { padding: UiRect::left(Val::Px(4.0)), margin: UiRect::bottom(Val::Px(3.0)), max_width: Val::Px(540.0), ..default() }, words(acts_here(skill), 10.0, FAINT)));
                    }
                }
                if classes == [Class::Practical] {
                    let locked: Vec<String> = Skill::ALL.into_iter().filter(|skill| skill.locked()).map(|skill| skill_word(state, skill).to_lowercase()).collect();
                    column.spawn((Node { margin: UiRect::top(Val::Px(10.0)), max_width: Val::Px(540.0), ..default() }, words(format!("Locked in Tarkov, with nothing to change: {}.", locked.join(", ")), 10.0, FAINT)));
                }
            });
        }
    });
}

/// The SKILLS page's button.
fn skill_clicks(mut state: ResMut<OptionsState>, bindings: Res<Bindings>, untrained: Query<&Interaction, (With<SkillsUntrained>, Changed<Interaction>)>) {
    if untrained.iter().any(|interaction| *interaction == Interaction::Pressed) && !state.skills.untrained() {
        state.skills = trp_sim::skills::SkillLevels::default();
        state.keep(&bindings.controls);
        state.dirty = true;
    }
}

/// The TIME OF DAY's button: the hour every run begins at.
#[derive(Component)]
struct HourDefault;

/// The FEATURES page's TIME OF DAY put back at the hour a run begins at.
fn hour_clicks(mut state: ResMut<OptionsState>, default: Query<&Interaction, (With<HourDefault>, Changed<Interaction>)>) {
    if default.iter().any(|interaction| *interaction == Interaction::Pressed) && state.hour != crate::lighting::RANGE_MOMENT.hour {
        state.hour = crate::lighting::RANGE_MOMENT.hour;
        state.dirty = true;
    }
}

/// The tab and the run's skills kept as one (`skills::Skills`): a level
/// changed on the tab is the run's at once (and kept when the bar is let
/// go); one a host game changed in play shows on the tab and is kept in
/// the app's file, fractions of a level and all (the user, 2026-10-23:
/// "Earned points should be saved. The options tab should update when a
/// skill is leveled").
fn sync_skills(mut state: ResMut<OptionsState>, skills: Option<ResMut<crate::skills::Skills>>, bindings: Res<Bindings>, mut agreed: Local<Option<trp_sim::skills::SkillLevels>>) {
    let Some(mut skills) = skills else { return };
    if state.skills != skills.levels {
        if agreed.as_ref().is_some_and(|was| *was != skills.levels) {
            state.skills = skills.levels.clone();
            state.keep(&bindings.controls);
            state.dirty = true;
        } else {
            skills.levels = state.skills.clone();
        }
    }
    if agreed.as_ref() != Some(&skills.levels) {
        *agreed = Some(skills.levels.clone());
    }
}

fn graphics_part(page: &mut ChildSpawnerCommands, state: &OptionsState, resolutions: &[[u32; 2]]) {
    let graphics = &state.graphics;
    let line = |page: &mut ChildSpawnerCommands, caption: &str, inside: &mut dyn FnMut(&mut ChildSpawnerCommands)| {
        page.spawn(Node { column_gap: Val::Px(6.0), align_items: AlignItems::Center, flex_wrap: FlexWrap::Wrap, row_gap: Val::Px(4.0), ..default() }).with_children(|row| {
            row.spawn(Node { width: Val::Px(200.0), flex_shrink: 0.0, ..default() }).with_children(|name| {
                name.spawn(words(caption, 12.0, FAINT));
            });
            inside(row);
        });
    };
    line(page, "VSYNC", &mut |row| {
        button(row, VsyncChoice(true), "ON", graphics.vsync, Val::Px(110.0));
        button(row, VsyncChoice(false), "OFF", !graphics.vsync, Val::Px(110.0));
    });
    // The game's frame rate limit in a raid, held with VSync off as the
    // game's is (`frame_limit`), in the game's words.
    let fps_caption = state.said("Game FPS Limit").trim_end_matches(':').to_owned();
    let fps_note = if graphics.fps_limit_off {
        "the game's, 30 to 144: turned off"
    } else if graphics.vsync {
        "the game's, 30 to 144: held with VSYNC off; with it on the screen paces the frames, as in the game's raids"
    } else {
        "the game's, 30 to 144: frames held to it"
    };
    line(page, &fps_caption, &mut |row| {
        slider(row, Slide::GameFps, graphics.game_fps as f32, 400.0);
        row.spawn(words(fps_note, 11.0, FAINT));
    });
    line(page, &state.said("DisableGameFramerateLimit"), &mut |row| {
        button(row, FpsLimitOffChoice(true), "ON", graphics.fps_limit_off, Val::Px(110.0));
        button(row, FpsLimitOffChoice(false), "OFF", !graphics.fps_limit_off, Val::Px(110.0));
    });
    line(page, "FIELD OF VIEW", &mut |row| {
        slider(row, Slide::Fov, graphics.field_of_view, 400.0);
        row.spawn(words("the game's setting, 50 to 75; aimed, 15 less", 11.0, FAINT));
    });
    line(page, "DISPLAY MODE", &mut |row| {
        for mode in Mode::ALL {
            button(row, ModeChoice(mode), mode.word(), graphics.mode == mode, Val::Px(130.0));
        }
    });
    line(page, "RESOLUTION", &mut |row| {
        for size in resolutions {
            button(row, ResolutionChoice(*size), &format!("{} x {}", size[0], size[1]), graphics.resolution == *size, Val::Px(130.0));
        }
    });
    // The pictures', taken at the next start (`streaming`).
    let textures_changed = graphics.texture_quality != state.textures_started.0 || graphics.mip_streaming != state.textures_started.1;
    line(page, "TEXTURE QUALITY", &mut |row| {
        for (quality, word) in [(0, "LOW"), (1, "MEDIUM"), (2, "HIGH")] {
            button(row, TextureChoice(quality), word, graphics.texture_quality == quality, Val::Px(110.0));
        }
    });
    line(page, "MIP STREAMING", &mut |row| {
        button(row, StreamingChoice(true), "ON", graphics.mip_streaming, Val::Px(110.0));
        button(row, StreamingChoice(false), "OFF", !graphics.mip_streaming, Val::Px(110.0));
        row.spawn(words("RESTART REQUIRED", 10.0, if textures_changed { WORDS } else { FAINT }));
    });
    // The sun's, taken at the next start (`shadows`).
    line(page, "SHADOW QUALITY", &mut |row| {
        for (quality, word) in crate::shadows::NAMES.iter().enumerate() {
            button(row, ShadowChoice(quality as u8), word, graphics.shadow_quality == quality as u8, Val::Px(110.0));
        }
        row.spawn(words("RESTART REQUIRED", 10.0, if graphics.shadow_quality != state.shadows_started { WORDS } else { FAINT }));
    });
    // The ambient occlusion's, taken at the next start (`hbao_view`), in
    // the game's words for its choices.
    line(page, "SSAO", &mut |row| {
        for mode in trp_sim::hbao::SsaoMode::ALL {
            button(row, SsaoChoice(mode), &state.said(&format!("ESSAOMode/{}", mode.key())), graphics.ssao == mode, Val::Px(140.0));
        }
        row.spawn(words("RESTART REQUIRED", 10.0, if graphics.ssao != state.ssao_started { WORDS } else { FAINT }));
    });
    // The clouds', taken at the next start (`clouds_view`), in the game's
    // words.
    let clouds_caption = state.said("Settings/Graphics/CloudsQuality").trim_end_matches(':').to_owned();
    line(page, &clouds_caption, &mut |row| {
        for quality in trp_sim::clouds::CloudsQuality::ALL {
            button(row, CloudsChoice(quality), &state.said(&format!("CloudsMode/{}", quality.key())), graphics.clouds_quality == quality, Val::Px(110.0));
        }
        row.spawn(words("RESTART REQUIRED", 10.0, if graphics.clouds_quality != state.clouds_started { WORDS } else { FAINT }));
    });
    // Rendering a frame behind on a thread of its own, as the game's engine
    // does with its graphics jobs, taken at the next start.
    line(page, "RENDER THREAD", &mut |row| {
        button(row, RenderThreadChoice(true), "ON", graphics.render_thread, Val::Px(110.0));
        button(row, RenderThreadChoice(false), "OFF", !graphics.render_thread, Val::Px(110.0));
        row.spawn(words("RESTART REQUIRED", 10.0, if graphics.render_thread != state.render_thread_started { WORDS } else { FAINT }));
    });
    // The app's own multisampling of the scene: the game's camera has none.
    line(page, "MSAA", &mut |row| {
        for (samples, word) in MSAA {
            button(row, MsaaChoice(samples), word, graphics.msaa == samples, Val::Px(110.0));
        }
        row.spawn(words("the app's own: Tarkov's camera multisamples nothing", 11.0, FAINT));
    });
    // The backend, taken at the next start (`backend`).
    line(page, "RENDERER", &mut |row| {
        for renderer in [crate::backend::Renderer::Vulkan, crate::backend::Renderer::Dx12] {
            button(row, RendererChoice(renderer), renderer.word(), graphics.renderer == renderer, Val::Px(110.0));
        }
        row.spawn(words("RESTART REQUIRED", 10.0, if graphics.renderer != state.renderer_started { WORDS } else { FAINT }));
    });
    page.spawn(words(
        "Each is put on the window as it is chosen. A borderless window is as large as the screen whatever the resolution says; the screen taken over is put into the nearest mode it has.",
        11.0,
        FAINT,
    ));
    let theirs = &state.install_graphics;
    page.spawn(Node { column_gap: Val::Px(12.0), align_items: AlignItems::Center, margin: UiRect::top(Val::Px(10.0)), ..default() }).with_children(|row| {
        button(row, InstallDisplay, "AS THE INSTALL HAS THEM", false, Val::Auto);
        let said = format!(
            "the install's own game: vsync {}, field of view {:.0}, game fps limit {}, {}, {} x {}, textures {}, mip streaming {}, shadows {}, ssao {}, clouds {}, render thread {}",
            if theirs.vsync { "on" } else { "off" },
            theirs.field_of_view,
            if theirs.fps_limit_off { "off".to_owned() } else { theirs.game_fps.to_string() },
            theirs.mode.word().to_lowercase(),
            theirs.resolution[0],
            theirs.resolution[1],
            ["low", "medium", "high"][theirs.texture_quality.min(2) as usize],
            if theirs.mip_streaming { "on" } else { "off" },
            crate::shadows::NAMES[theirs.shadow_quality.min(3) as usize].to_lowercase(),
            state.said(&format!("ESSAOMode/{}", theirs.ssao.key())).to_lowercase(),
            state.said(&format!("CloudsMode/{}", theirs.clouds_quality.key())).to_lowercase(),
            if theirs.render_thread { "on (graphics jobs)" } else { "off" }
        );
        row.spawn(words(said, 11.0, FAINT));
    });
    page.spawn(words(state.note.clone(), 12.0, WORDS));
}

/// The switches in three lists (0.101.0; the user, 2026-10-21: "I want to
/// overhaul the feature list. My vision is to have 2 Main categories",
/// three named: `SIMPLE`, "very basic and easy to understand features";
/// `EXPERIMENTAL`, "features that were not originally in Tarkov but may
/// help with passthrough or QOL"; `ADVANCED`, "whatever systems we have in
/// place that are not so accessible, but may be important in order to
/// make passthrough play easier. Like our light system"; "we should
/// definitely make them from scratch"). The placement is mine, by the
/// rule each list's header says; layered (a `SIMPLE` group's parts listed
/// one by one under `ADVANCED`) rather than flat, so that a host can turn
/// one part off and keep the rest, which the passthrough wants. Before
/// 0.101.0 they were `GAMEPLAY` and `VISUALS` (the user, 2026-10-16).
///
/// `SIMPLE`: what a player sees or feels, in plain words, every one of
/// them Tarkov's own; three are groups whose parts are under `ADVANCED`.
const SIMPLE: [FeatureSwitch; 21] = [
    FeatureSwitch::TarkovUi,
    FeatureSwitch::Health,
    FeatureSwitch::Skills,
    FeatureSwitch::Stamina,
    FeatureSwitch::Weight,
    FeatureSwitch::Ballistics,
    FeatureSwitch::GrenadesAndKnife,
    FeatureSwitch::TacticalDevices,
    FeatureSwitch::BlindFire,
    FeatureSwitch::Mounting,
    FeatureSwitch::LeftShoulder,
    FeatureSwitch::Durability,
    FeatureSwitch::Malfunctions,
    FeatureSwitch::Overheating,
    FeatureSwitch::Attachments,
    FeatureSwitch::Pip,
    FeatureSwitch::Sounds,
    FeatureSwitch::VisualEffects,
    FeatureSwitch::Dummy,
    FeatureSwitch::ReactiveTargets,
    FeatureSwitch::CharacterModel,
];
/// `EXPERIMENTAL`: what departs from Tarkov on purpose, for comfort or for
/// play through another game; each says what Tarkov does instead. (The
/// workbench: Tarkov edits a build only out of a raid. The clothing:
/// every side's suits, in a raid.) `LITE INVENTORY` follows, unbuilt.
const EXPERIMENTAL: [FeatureSwitch; 6] = [
    FeatureSwitch::CompatibleHighlight,
    FeatureSwitch::Clothing,
    FeatureSwitch::Workbench,
    FeatureSwitch::CustomHands,
    FeatureSwitch::SmoothAnimations,
    FeatureSwitch::GroundedFeet,
];
/// `ADVANCED`: the systems underneath, one switch each, for a host game
/// that does the job itself (it turns ours off) or for tracking a fault
/// down: the controller's and the world's, then the parts of each `SIMPLE`
/// group (the stamina's, the sounds', the effects'), then the lighting
/// (off unless switched, the user's), then the pictures. A little room
/// between the groups.
const ADVANCED: [&[FeatureSwitch]; 6] = [
    &[
        FeatureSwitch::Movement,
        FeatureSwitch::Vaulting,
        FeatureSwitch::WeaponMotion,
        FeatureSwitch::Recoil,
        FeatureSwitch::WeaponHandling,
        FeatureSwitch::FirstPersonBody,
        FeatureSwitch::Inventory,
        FeatureSwitch::WorldInteractions,
        FeatureSwitch::ItemPhysics,
    ],
    &[FeatureSwitch::StaminaDrain, FeatureSwitch::Exhaustion],
    &[FeatureSwitch::MovementSounds, FeatureSwitch::HandsSounds, FeatureSwitch::ShotSounds, FeatureSwitch::WorldSounds],
    &[FeatureSwitch::MuzzleFlash, FeatureSwitch::Particles, FeatureSwitch::SpentCases, FeatureSwitch::MagazineRounds, FeatureSwitch::FaceShieldMask],
    &[
        FeatureSwitch::CustomsLight,
        FeatureSwitch::CameraColorFilter,
        FeatureSwitch::CameraSharpening,
        FeatureSwitch::SkyScattering,
        FeatureSwitch::CustomsClouds,
        FeatureSwitch::AmbientHighlight,
        FeatureSwitch::TarkovShading,
        FeatureSwitch::AmbientOcclusion,
        FeatureSwitch::HallLamps,
    ],
    &[FeatureSwitch::IconGeneration],
];
/// What each list is for, under its header.
const LIST_WORDS: [(&str, &str); 3] = [
    ("SIMPLE", "What you see and feel, each Tarkov's own. Three are groups: their parts are under ADVANCED."),
    ("EXPERIMENTAL", "What departs from Tarkov on purpose, for comfort or for play through another game. Off, Tarkov's way."),
    ("ADVANCED", "The systems underneath, one by one, for a host game that does the job itself, or to track a fault down. A part of a SIMPLE group runs only while its group is on."),
];

// ==== The features' part: rows, headers, descriptions ====

/// A row of the features: what the panel beside them says with the mouse
/// on it (its name, what it does, and its state's note).
#[derive(Component)]
struct FeatureRow {
    name: &'static str,
    what: &'static str,
    note: String,
}
/// The panel that says what the row under the mouse does.
#[derive(Component)]
struct FeatureAbout(u8);

/// A row lit with the mouse on it.
const ROW_LIT: Color = Color::srgba(1.0, 1.0, 1.0, 0.08);

fn features_part(page: &mut ChildSpawnerCommands, state: &OptionsState) {
    page.spawn(Node { column_gap: Val::Px(28.0), margin: UiRect::top(Val::Px(8.0)), ..default() }).with_children(|columns| {
        // The first two lists in one column, the long third in its own.
        columns.spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), width: Val::Px(330.0), flex_shrink: 0.0, ..default() }).with_children(|column| {
            list_header(column, LIST_WORDS[0]);
            for switch in SIMPLE {
                switch_row(column, state, switch);
            }
            column.spawn(Node { height: Val::Px(14.0), ..default() });
            list_header(column, LIST_WORDS[1]);
            for switch in EXPERIMENTAL {
                switch_row(column, state, switch);
            }
            unbuilt_row(column, LITE_INVENTORY);
        });
        columns.spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), width: Val::Px(360.0), flex_shrink: 0.0, ..default() }).with_children(|column| {
            list_header(column, LIST_WORDS[2]);
            for (n, group) in ADVANCED.iter().enumerate() {
                if n > 0 {
                    column.spawn(Node { height: Val::Px(6.0), ..default() });
                }
                for switch in *group {
                    switch_row(column, state, *switch);
                }
            }
        });
        // What the row under the mouse does, and under it the range's time
        // of day (the user, 2026-10-25), taken at once by its own light.
        columns.spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(18.0), width: Val::Px(440.0), flex_shrink: 0.0, ..default() }).with_children(|right| {
            let panel = Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(8.0), width: Val::Px(440.0), min_height: Val::Px(160.0), padding: UiRect::all(Val::Px(12.0)), border: UiRect::all(Val::Px(1.0)), flex_shrink: 0.0, align_self: AlignSelf::FlexStart, ..default() };
            right.spawn((panel, BorderColor::all(LINE), BackgroundColor(PANEL))).with_children(|about| {
                about.spawn((FeatureAbout(0), words("", 14.0, WORDS)));
                about.spawn((FeatureAbout(1), Node { max_width: Val::Px(414.0), ..default() }, words("Point at a switch to read what it does. Each takes effect when the app is next started. SIMPLE is what you see and feel; EXPERIMENTAL what departs from Tarkov on purpose; ADVANCED the systems underneath, one by one. What is not built yet is listed in docs/backlog.md.", 12.0, FAINT)));
                about.spawn((FeatureAbout(2), words("", 11.0, WORDS)));
            });
            right.spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(4.0), ..default() }).with_children(|column| {
                section(column, Slide::Hour.caption().1);
                // Customs' light is worked out once, as the run begins: no
                // bar for it, only the hour it was set at.
                let starts = Slide::Hour.shown(crate::lighting::RANGE_MOMENT.hour);
                let note = if state.in_force.customs_light {
                    format!("With CUSTOMS LIGHT on, the range is lit as Customs at the hour the app started at ({}; every run starts at {starts}, or at --hour's). The bar is there with CUSTOMS LIGHT off.", Slide::Hour.shown(state.hour_started))
                } else {
                    column.spawn(Node { column_gap: Val::Px(8.0), align_items: AlignItems::Center, padding: UiRect::axes(Val::Px(4.0), Val::Px(2.0)), ..default() }).with_children(|row| {
                        slider(row, Slide::Hour, state.hour, 260.0);
                        button(row, HourDefault, "DEFAULT", false, Val::Auto);
                    });
                    format!("The range's hour, taken at once: the range's own sun, ambient and sky follow Customs' day and night, darker at night so that a weapon's light shows. The drawing alone. Every run starts at {starts}; DEFAULT puts it back there.")
                };
                column.spawn((Node { max_width: Val::Px(430.0), padding: UiRect::axes(Val::Px(4.0), Val::Px(0.0)), ..default() }, words(note, 11.0, FAINT)));
            });
        });
    });
}

/// A section's header over its rows.
fn section(column: &mut ChildSpawnerCommands, name: &str) {
    column.spawn((Node { padding: UiRect::axes(Val::Px(4.0), Val::Px(3.0)), margin: UiRect::bottom(Val::Px(4.0)), border: UiRect::bottom(Val::Px(1.0)), ..default() }, BorderColor::all(LINE))).with_children(|header| {
        header.spawn(words(name, 13.0, WORDS));
    });
}

/// A list's header, and under it, faint, what the list is for.
fn list_header(column: &mut ChildSpawnerCommands, (name, what): (&str, &str)) {
    section(column, name);
    column.spawn((Node { max_width: Val::Px(330.0), padding: UiRect::axes(Val::Px(4.0), Val::Px(0.0)), margin: UiRect::bottom(Val::Px(4.0)), ..default() }, words(what, 10.0, FAINT)));
}

/// A row for what has no switch yet: a dash where the switch would be,
/// its name faint, and in the panel what it will be.
fn unbuilt_row(column: &mut ChildSpawnerCommands, (name, what): (&'static str, &'static str)) {
    let row = Node { column_gap: Val::Px(8.0), align_items: AlignItems::Center, padding: UiRect::axes(Val::Px(4.0), Val::Px(2.0)), ..default() };
    let note = "Not built yet: designed, to be built when the user says so (docs/backlog.md).".to_owned();
    column.spawn((FeatureRow { name, what, note }, RelativeCursorPosition::default(), row, BackgroundColor(Color::NONE))).with_children(|row| {
        row.spawn((Node { width: Val::Px(46.0), padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)), border: UiRect::all(Val::Px(1.0)), justify_content: JustifyContent::Center, flex_shrink: 0.0, ..default() }, BorderColor::all(LINE))).with_children(|switch| {
            switch.spawn(words("-", 11.0, FAINT));
        });
        row.spawn(words(name, 12.0, FAINT));
    });
}

/// A switch's row: its switch and its name, and, once it is thrown away
/// from what the run holds, the note that it takes effect at the next
/// start ("restart required"). A part of a `SIMPLE` group whose group is
/// off is drawn faint, its own switch shown as it is kept, and the panel
/// says what holds it off.
fn switch_row(column: &mut ChildSpawnerCommands, state: &OptionsState, switch: FeatureSwitch) {
    let (name, what) = switch.words();
    let on = switch.get(&state.features);
    let pending = on != switch.get(&state.started);
    let held_off_by = switch.group().filter(|group| !group.get(&state.features));
    let note = match (held_off_by, pending) {
        (Some(group), _) => format!("Held off by {} under SIMPLE while that is off; its own switch is kept as it stands. RESTART REQUIRED to take effect.", group.words().0),
        (None, true) => format!("RESTART REQUIRED: {} until the app is started again.", if on { "off" } else { "on" }),
        (None, false) => "RESTART REQUIRED to take effect.".to_owned(),
    };
    let shade = if held_off_by.is_some() { FAINT } else { WORDS };
    let row = Node { column_gap: Val::Px(8.0), align_items: AlignItems::Center, padding: UiRect::axes(Val::Px(4.0), Val::Px(2.0)), ..default() };
    column.spawn((FeatureRow { name, what, note }, RelativeCursorPosition::default(), row, BackgroundColor(Color::NONE))).with_children(|row| {
        let node = Node { width: Val::Px(46.0), padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)), border: UiRect::all(Val::Px(1.0)), justify_content: JustifyContent::Center, flex_shrink: 0.0, ..default() };
        row.spawn((switch, Button, node, BorderColor::all(if on { shade } else { LINE }), BackgroundColor(if on { LINE } else { PANEL }))).with_children(|inside| {
            inside.spawn(words(if on { "ON" } else { "OFF" }, 11.0, shade));
        });
        row.spawn(words(name, 12.0, shade));
        if pending {
            row.spawn(words("RESTART", 10.0, WORDS));
        }
    });
}

/// The row under the mouse lit, and what it does said in the panel beside.
fn describe_features(mut rows: Query<(&FeatureRow, &RelativeCursorPosition, &mut BackgroundColor)>, mut about: Query<(&FeatureAbout, &mut Text)>, mut said: Local<Option<&'static str>>) {
    let mut over = None;
    for (row, at, mut ground) in &mut rows {
        let lit = at.cursor_over();
        let want = if lit { ROW_LIT } else { Color::NONE };
        if ground.0 != want {
            ground.0 = want;
        }
        if lit {
            over = Some(row);
        }
    }
    let Some(row) = over else { return };
    if *said == Some(row.name) && about.iter().any(|(part, text)| part.0 == 2 && text.0 == row.note) {
        return;
    }
    *said = Some(row.name);
    for (part, mut text) in &mut about {
        let now = match part.0 {
            0 => row.name,
            1 => row.what,
            _ => row.note.as_str(),
        };
        if text.0 != now {
            text.0 = now.to_owned();
        }
    }
}

// ==== Rebuilding the page, clicks, listening, slides, scrolling, applying ====

/// The sizes the window may be given: those the screens have a mode for,
/// the largest first, with the size the app starts at and the one in
/// force among them.
fn resolutions(monitors: &Query<&Monitor>, graphics: &Graphics) -> Vec<[u32; 2]> {
    let mut sizes: Vec<[u32; 2]> = monitors.iter().flat_map(|monitor| monitor.video_modes.iter().map(|mode| [mode.physical_size.x, mode.physical_size.y])).collect();
    sizes.retain(|size| size[0] >= 1024 && size[1] >= 720);
    sizes.extend([crate::WINDOW_SIZE, graphics.resolution]);
    sizes.sort_by(|a, b| (b[0], b[1]).cmp(&(a[0], a[1])));
    sizes.dedup();
    sizes.truncate(14);
    if !sizes.contains(&graphics.resolution) {
        sizes.push(graphics.resolution);
    }
    sizes
}

/// Draws the tab afresh when something on it has changed.
fn rebuild(
    mut commands: Commands,
    mut state: ResMut<OptionsState>,
    controller: Res<Controller>,
    selected: Res<Selected>,
    bindings: Res<Bindings>,
    window: Query<&Window, With<PrimaryWindow>>,
    monitors: Query<&Monitor>,
    page: Query<Entity, With<OptionsPage>>,
    mut was: Local<(bool, Vec2)>,
) {
    let _clock = crate::render_timing::Clock::new("options rebuild");
    let size = window.single().map_or(Vec2::ZERO, |window| Vec2::new(window.width(), window.height()));
    let shows = controller.inventory_open && selected.0 == OPTIONS;
    let changed = state.dirty || *was != (shows, size);
    *was = (shows, size);
    if !shows || !changed {
        return;
    }
    state.dirty = false;
    let state = &*state;
    let sizes = resolutions(&monitors, &state.graphics);
    for page in &page {
        commands.entity(page).despawn_related::<Children>().with_children(|page| {
            // The tab's parts to the left, and at the far right the
            // button that ends the run.
            page.spawn(Node { column_gap: Val::Px(6.0), width: Val::Percent(100.0), flex_shrink: 0.0, ..default() }).with_children(|row| {
                for (index, caption) in SUBS.into_iter().enumerate() {
                    button(row, SubTab(index), caption, index == state.sub, Val::Px(150.0));
                }
                row.spawn(Node { flex_grow: 1.0, ..default() });
                button(row, ExitButton, "QUIT", false, Val::Px(150.0));
            });
            match state.sub {
                0 => game_part(page, state),
                1 => controls_part(page, state, &bindings),
                2 => graphics_part(page, state, &sizes),
                3 => sound_part(page, state),
                4 => skills_part(page, state),
                _ => features_part(page, state),
            }
        });
    }
}

/// What a click on the tab does.
fn clicks(
    mut state: ResMut<OptionsState>,
    mut bindings: ResMut<Bindings>,
    controller: Res<Controller>,
    keys: Res<ButtonInput<KeyCode>>,
    subs: Query<(&Interaction, &SubTab), Changed<Interaction>>,
    cells: Query<(&Interaction, &KeyCell), Changed<Interaction>>,
    press: Query<(&Interaction, &PressCell), Changed<Interaction>>,
    choices: Query<(&Interaction, &PressChoice), Changed<Interaction>>,
    install_keys: Query<&Interaction, (With<InstallKeys>, Changed<Interaction>)>,
    install_display: Query<&Interaction, (With<InstallDisplay>, Changed<Interaction>)>,
    (vsync, modes, sizes, custom_hands, fps_off, msaa_choices): (
        Query<(&Interaction, &VsyncChoice), Changed<Interaction>>,
        Query<(&Interaction, &ModeChoice), Changed<Interaction>>,
        Query<(&Interaction, &ResolutionChoice), Changed<Interaction>>,
        Query<(&Interaction, &FeatureSwitch), Changed<Interaction>>,
        Query<(&Interaction, &FpsLimitOffChoice), Changed<Interaction>>,
        Query<(&Interaction, &MsaaChoice), Changed<Interaction>>,
    ),
    inverts: Query<(&Interaction, &InvertChoice), Changed<Interaction>>,
    (textures, streamings, shadow_choices, ssao_choices, clouds_choices, threads, renderers): (
        Query<(&Interaction, &TextureChoice), Changed<Interaction>>,
        Query<(&Interaction, &StreamingChoice), Changed<Interaction>>,
        Query<(&Interaction, &ShadowChoice), Changed<Interaction>>,
        Query<(&Interaction, &SsaoChoice), Changed<Interaction>>,
        Query<(&Interaction, &CloudsChoice), Changed<Interaction>>,
        Query<(&Interaction, &RenderThreadChoice), Changed<Interaction>>,
        Query<(&Interaction, &RendererChoice), Changed<Interaction>>,
    ),
    exit: Query<&Interaction, (With<ExitButton>, Changed<Interaction>)>,
    mut quit: MessageWriter<AppExit>,
    (game_choices, install_game, mut battle_modes, sound_choices, install_sound, mut levels): (
        Query<(&Interaction, &GameChoice), Changed<Interaction>>,
        Query<&Interaction, (With<InstallGame>, Changed<Interaction>)>,
        ResMut<crate::battle_view::BattleModes>,
        Query<(&Interaction, &SoundChoice), Changed<Interaction>>,
        Query<&Interaction, (With<InstallSound>, Changed<Interaction>)>,
        ResMut<crate::ui_sound::Levels>,
    ),
) {
    let pressed = |interaction: &Interaction| *interaction == Interaction::Pressed;
    // The screen shut: a key being chosen is dropped with it.
    if !controller.inventory_open {
        if state.listening.is_some() || state.asking.is_some() {
            (state.listening, state.asking, state.dirty) = (None, None, true);
            bindings.muted = false;
        }
        return;
    }
    if state.listening.is_some() {
        return;
    }
    let state = &mut *state;
    // How a binding is pressed, asked for: one of the four, or Escape,
    // which leaves it as it is (the game's list shut with nothing chosen).
    if let Some(group) = state.asking {
        let chosen = choices.iter().find(|(interaction, _)| pressed(interaction)).map(|(_, choice)| choice.0);
        let kept = keys.just_pressed(KeyCode::Escape).then(|| bindings.controls.keys[group].press);
        if let Some(press) = chosen.or(kept) {
            let before = bindings.controls.clone();
            bindings.controls.press_chosen(group, press);
            state.note = format!("{}: {}{}", state.name_of(&before.keys[group].name), state.press_word(bindings.controls.keys[group].press).to_lowercase(), lost(state, &before, &bindings.controls, group));
            bindings.changed();
            state.keep(&bindings.controls);
            (state.asking, state.dirty) = (None, true);
        }
        return;
    }
    // The run is ended: what was changed is already kept.
    if exit.iter().any(pressed) {
        quit.write(AppExit::Success);
        return;
    }
    if let Some((_, tab)) = subs.iter().find(|(interaction, _)| pressed(interaction)) {
        (state.sub, state.note, state.dirty) = (tab.0, String::new(), true);
        return;
    }
    // A setting of the GAME page: kept, and handed to the battle screen,
    // which takes it as it is next shown.
    let game_before = state.game;
    if let Some((_, choice)) = game_choices.iter().find(|(interaction, _)| pressed(interaction)) {
        match *choice {
            GameChoice::Shows(0, shows) => state.game.quick = shows,
            GameChoice::Shows(1, shows) => state.game.stamina = shows,
            GameChoice::Shows(_, shows) => state.game.health = shows,
            GameChoice::Colors(green) => state.game.green_to_red = green,
        }
    }
    if install_game.iter().any(pressed) {
        state.game = state.install_game;
    }
    if state.game != game_before {
        *battle_modes = state.game;
        state.keep(&bindings.controls);
        state.dirty = true;
        return;
    }
    // A volume of the SOUND page: kept, and heard at once.
    let sound_before = state.sound;
    if let Some((_, choice)) = sound_choices.iter().find(|(interaction, _)| pressed(interaction)) {
        match choice.0 {
            0 => state.sound.overall = choice.1,
            _ => state.sound.interface = choice.1,
        }
    }
    if install_sound.iter().any(pressed) {
        state.sound = state.install_sound;
    }
    if state.sound != sound_before {
        *levels = state.sound;
        state.keep(&bindings.controls);
        state.dirty = true;
        return;
    }
    // A switch of the features' list: thrown and kept for the next run.
    // What goes by it holds the switches as this run began (restart
    // required): nothing is handed on now.
    if let Some((_, switch)) = custom_hands.iter().find(|(interaction, _)| pressed(interaction)) {
        switch.flip(&mut state.features);
        state.note.clear();
        state.keep(&bindings.controls);
        state.dirty = true;
        return;
    }
    if let Some((_, choice)) = inverts.iter().find(|(interaction, _)| pressed(interaction)) {
        state.mouse.invert[choice.0] = !state.mouse.invert[choice.0];
        state.keep(&bindings.controls);
        state.dirty = true;
        return;
    }
    if let Some((_, cell)) = cells.iter().find(|(interaction, _)| pressed(interaction)) {
        state.listening = Some(Listen { cell: cell.0, heard: Listening::default(), armed: false, done: false });
        bindings.muted = true;
        state.dirty = true;
        return;
    }
    if let Some((_, cell)) = press.iter().find(|(interaction, _)| pressed(interaction)) {
        (state.asking, state.dirty) = (Some(cell.0), true);
        return;
    }
    if install_keys.iter().any(pressed) {
        bindings.controls = bindings.install.clone();
        bindings.changed();
        state.mouse = state.install_mouse.clone();
        state.note = "the keys and the mouse are the install's again".to_owned();
        state.keep(&bindings.controls);
        state.dirty = true;
        return;
    }
    let before = state.graphics.clone();
    if let Some((_, choice)) = vsync.iter().find(|(interaction, _)| pressed(interaction)) {
        state.graphics.vsync = choice.0;
    }
    if let Some((_, choice)) = modes.iter().find(|(interaction, _)| pressed(interaction)) {
        state.graphics.mode = choice.0;
    }
    if let Some((_, choice)) = sizes.iter().find(|(interaction, _)| pressed(interaction)) {
        state.graphics.resolution = choice.0;
    }
    if let Some((_, choice)) = textures.iter().find(|(interaction, _)| pressed(interaction)) {
        state.graphics.texture_quality = choice.0;
    }
    if let Some((_, choice)) = streamings.iter().find(|(interaction, _)| pressed(interaction)) {
        state.graphics.mip_streaming = choice.0;
    }
    if let Some((_, choice)) = shadow_choices.iter().find(|(interaction, _)| pressed(interaction)) {
        state.graphics.shadow_quality = choice.0;
    }
    if let Some((_, choice)) = ssao_choices.iter().find(|(interaction, _)| pressed(interaction)) {
        state.graphics.ssao = choice.0;
    }
    if let Some((_, choice)) = clouds_choices.iter().find(|(interaction, _)| pressed(interaction)) {
        state.graphics.clouds_quality = choice.0;
    }
    if let Some((_, choice)) = threads.iter().find(|(interaction, _)| pressed(interaction)) {
        state.graphics.render_thread = choice.0;
    }
    if let Some((_, choice)) = fps_off.iter().find(|(interaction, _)| pressed(interaction)) {
        state.graphics.fps_limit_off = choice.0;
    }
    if let Some((_, choice)) = msaa_choices.iter().find(|(interaction, _)| pressed(interaction)) {
        state.graphics.msaa = choice.0;
    }
    if let Some((_, choice)) = renderers.iter().find(|(interaction, _)| pressed(interaction)) {
        state.graphics.renderer = choice.0;
    }
    if install_display.iter().any(pressed) {
        // (The backend stays: the game has none of the two.)
        state.graphics = Graphics { renderer: state.graphics.renderer, msaa: state.graphics.msaa, ..state.install_graphics.clone() };
    }
    if state.graphics != before {
        state.note.clear();
        state.keep(&bindings.controls);
        state.dirty = true;
    }
}

/// Which other bindings lost keys to a change, in words.
fn lost(state: &OptionsState, before: &Controls, after: &Controls, but: usize) -> String {
    let keys = before.keys.iter().zip(&after.keys).enumerate().filter(|(at, (was, is))| *at != but && was.variants != is.variants).map(|(_, (was, _))| state.name_of(&was.name));
    let axes = before.axes.iter().zip(&after.axes).filter(|(was, is)| was.pairs != is.pairs).map(|(was, _)| was.name.to_uppercase());
    let names: Vec<String> = keys.chain(axes).collect();
    if names.is_empty() { String::new() } else { format!("   (taken from {})", names.join(", ")) }
}

/// A key being chosen (`CommandKeyPair.ListenForKey`): once the click
/// that began it is over, the keys held are gathered, and the first of
/// them let go, a third going down or a notch of the wheel is the choice;
/// Escape among them drops it. Nothing else hears a key meanwhile, nor
/// until every key is up again.
fn listen(
    mut state: ResMut<OptionsState>,
    mut bindings: ResMut<Bindings>,
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    scroll: Res<AccumulatedMouseScroll>,
) {
    let state = &mut *state;
    let Some(listening) = &mut state.listening else { return };
    let any_down = keys.get_pressed().next().is_some() || buttons.get_pressed().next().is_some();
    if !listening.armed {
        listening.armed = !any_down;
        return;
    }
    if listening.done {
        if !any_down {
            (state.listening, state.dirty) = (None, true);
            bindings.muted = false;
        }
        return;
    }
    let cell = listening.cell;
    // The wheel may end any binding's keys but the trigger's
    // (`ForbiddenForAxisWithDefaultKey`); an axis's keys are keys.
    let takes_wheel = matches!(cell, Cell::Key(group, _) if bindings.controls.keys[group].name != "Shoot");
    let wheel = if scroll.delta.y == 0.0 { 0 } else { scroll.delta.y.signum() as i32 };
    let had = listening.heard.source.keys.len();
    let chosen = listening.heard.frame(&bindings.valid, &|code| bindings.held(code, &keys, &buttons), wheel, takes_wheel);
    match chosen {
        Chosen::Going => state.dirty |= listening.heard.source.keys.len() != had,
        Chosen::Dropped => {
            listening.done = true;
            state.note = "kept as it was".to_owned();
            state.dirty = true;
        }
        Chosen::Done(source) => {
            listening.done = true;
            let before = bindings.controls.clone();
            let said = keys::words(&source);
            match cell {
                Cell::Key(group, variant) => {
                    let asks = bindings.controls.key_chosen(group, variant, source);
                    let name = state.name_of(&before.keys[group].name);
                    if asks {
                        state.asking = Some(group);
                        state.note = format!("{name}: {said}. Another binding has these keys pressed the same way, or this one had none: choose how it is pressed.");
                    } else {
                        state.note = format!("{name}: {said}{}", lost(state, &before, &bindings.controls, group));
                    }
                }
                Cell::Axis(axis, pair, way) => {
                    bindings.controls.axis_chosen(axis, pair, way, source);
                    let word = AXIS_WORDS.iter().find(|(name, _)| *name == before.axes[axis].name).map_or("", |(_, ways)| ways[way]);
                    state.note = format!("{word}: {said}{}", lost(state, &before, &bindings.controls, usize::MAX));
                }
            }
            bindings.changed();
            state.keep(&bindings.controls);
            state.dirty = true;
        }
    }
}

/// A bar held and slid: its figure follows the mouse along it, and what
/// is drawn of it is changed where it stands. It is kept once the bar is
/// let go; the double click's time is then put to the bindings, whose
/// combinations are made afresh as the game makes them when it is saved.
fn slide(
    mut state: ResMut<OptionsState>,
    mut bindings: ResMut<Bindings>,
    buttons: Res<ButtonInput<MouseButton>>,
    bars: Query<(&SlideBar, &RelativeCursorPosition)>,
    mut fills: Query<(&SlideFill, &mut Node)>,
    mut figures: Query<(&SlideValue, &mut Text)>,
) {
    if state.sliding.is_none() && !buttons.just_pressed(MouseButton::Left) {
        return;
    }
    let state = &mut *state;
    if buttons.just_pressed(MouseButton::Left) && state.listening.is_none() {
        state.sliding = bars.iter().find(|(_, over)| over.cursor_over()).map(|(bar, _)| bar.0);
    }
    let Some(kind) = state.sliding else { return };
    if !buttons.pressed(MouseButton::Left) {
        state.sliding = None;
        if kind == Slide::DoubleClick {
            bindings.changed();
        }
        state.keep(&bindings.controls);
        return;
    }
    let Some(at) = bars.iter().find(|(bar, _)| bar.0 == kind).and_then(|(_, over)| crate::inventory_view::share_of(over)) else { return };
    let ([low, high], step) = (kind.range(), kind.step());
    let value = (((low + at.x.clamp(0.0, 1.0) * (high - low)) / step).round() * step).clamp(low, high);
    if value == state.slider(kind, &bindings.controls) {
        return;
    }
    // The bindings are not marked as changed for a bar that is not theirs.
    if kind == Slide::DoubleClick {
        bindings.controls.double_click = value;
    } else {
        state.set_slider(kind, value, &mut bindings.bypass_change_detection().controls);
    }
    for (_, mut node) in fills.iter_mut().filter(|(fill, _)| fill.0 == kind) {
        node.width = Val::Percent((value - low) / (high - low) * 100.0);
    }
    for (_, mut text) in figures.iter_mut().filter(|(figure, _)| figure.0 == kind) {
        text.0 = kind.shown(value);
    }
}

/// The list of bindings is scrolled by the wheel, three rows a notch,
/// unless the wheel is being listened to.
fn scroll_list(
    mut state: ResMut<OptionsState>,
    options: Res<Options>,
    wheel: Res<AccumulatedMouseScroll>,
    mut area: Query<(&ListArea, &mut ScrollPosition, &RelativeCursorPosition, &ComputedNode)>,
    mut frame: Local<u32>,
) {
    // A run nobody is at the keys of turns the wheel by its own count of
    // drawn frames (`--wheel`), as the inventory's does.
    *frame += 1;
    let scripted: i32 = options.wheel.iter().filter(|(at, _)| options.unattended() && *frame == *at).map(|(_, notches)| *notches).sum();
    let Ok((sizes, mut position, over, laid)) = area.single_mut() else { return };
    // How high the part of the list shown is, as the screen's layout has
    // it; nothing before it has been laid out.
    let shown = laid.size().y * laid.inverse_scale_factor();
    if shown <= 0.0 {
        return;
    }
    let mut to = state.scroll - scripted as f32 * ROW * 3.0;
    if state.listening.is_none() && over.cursor_over() && wheel.delta.y != 0.0 {
        to -= wheel.delta.y.signum() * ROW * 3.0;
    }
    let to = to.clamp(0.0, (sizes.whole - shown).max(0.0));
    if to != state.scroll {
        state.bypass_change_detection().scroll = to;
    }
    if position.y != to {
        position.y = to;
    }
}

/// Bevy's multisampling for a count of samples.
pub fn msaa_of(samples: u8) -> bevy::render::view::Msaa {
    match samples {
        1 => bevy::render::view::Msaa::Off,
        2 => bevy::render::view::Msaa::Sample2,
        _ => bevy::render::view::Msaa::Sample4,
    }
}

/// Puts the display's settings on the window and the view when they have
/// changed. Each is the game's setting of that name; the game puts them
/// on when its settings screen is saved, the app as each is chosen.
fn apply(
    state: Res<OptionsState>,
    mut options: ResMut<Options>,
    mut window: Query<&mut Window, With<PrimaryWindow>>,
    monitors: Query<&Monitor>,
    hands: Option<ResMut<HandsRig>>,
    stowed: Option<ResMut<Stowed>>,
    mut cameras: Query<&mut bevy::render::view::Msaa, With<crate::MainCamera>>,
    mut applied: Local<Option<Graphics>>,
) {
    // The mouse's settings are what the view is turned by from now on.
    let mouse = &state.mouse;
    if (options.sensitivity, options.aiming_sensitivity, options.optic_sensitivity, options.invert) != (mouse.sensitivity, mouse.aiming, mouse.optic, mouse.invert) {
        (options.sensitivity, options.aiming_sensitivity, options.optic_sensitivity, options.invert) = (mouse.sensitivity, mouse.aiming, mouse.optic, mouse.invert);
    }
    if applied.as_ref() == Some(&state.graphics) {
        return;
    }
    let now = state.graphics.clone();
    // As the run begins there is nothing to do, unless the app's file
    // said how the display is to be.
    let was = applied.replace(now.clone());
    let Some(was) = was.or_else(|| state.apply_at_start.then(|| Graphics { vsync: !now.vsync, field_of_view: f32::NAN, mode: Mode::Windowed, resolution: [0, 0], ..now.clone() })) else { return };
    let Ok(mut window) = window.single_mut() else { return };
    if now.vsync != was.vsync {
        window.present_mode = if now.vsync { PresentMode::AutoVsync } else { PresentMode::AutoNoVsync };
    }
    if now.msaa != was.msaa {
        for mut msaa in &mut cameras {
            *msaa = msaa_of(now.msaa);
        }
    }
    if now.field_of_view != was.field_of_view {
        options.field_of_view = now.field_of_view;
        if let Some(mut hands) = hands {
            hands.hands.set_field_of_view(now.field_of_view);
        }
        if let Some(mut stowed) = stowed {
            stowed.set_field_of_view(now.field_of_view);
        }
    }
    if now.mode != was.mode || now.resolution != was.resolution {
        let [width, height] = now.resolution;
        match now.mode {
            Mode::Windowed => {
                window.mode = WindowMode::Windowed;
                window.resolution.set_physical_resolution(width, height);
            }
            Mode::Borderless => window.mode = WindowMode::BorderlessFullscreen(MonitorSelection::Current),
            Mode::Fullscreen => {
                // The screen's mode of that size with the most frames a
                // second, or the mode it is in if it has none.
                let modes = monitors.iter().flat_map(|monitor| monitor.video_modes.iter().copied());
                let wanted = modes.filter(|mode| mode.physical_size == UVec2::new(width, height)).max_by_key(|mode| (mode.refresh_rate_millihertz, mode.bit_depth));
                window.mode = WindowMode::Fullscreen(MonitorSelection::Current, wanted.map_or(VideoModeSelection::Current, VideoModeSelection::Specific));
            }
        }
    }
}

// ==== Tests ====

#[cfg(test)]
mod tests {
    use super::*;
    use trp_sim::bindings::{AxisGroup, KeyGroup};

    fn source(keys: &[&str], wheel: Option<bool>) -> Source {
        Source { keys: keys.iter().map(|key| code(key).unwrap()).collect(), wheel }
    }

    #[test]
    fn what_is_changed_is_kept_and_read_back_over_the_install_s() {
        let group = |name: &str, press: PressType, keys: &[&str]| KeyGroup { name: name.to_owned(), variants: [source(keys, None), Source::default()], press };
        let install = Controls {
            keys: vec![group("ReloadWeapon", PressType::Release, &["R"]), group("Prone", PressType::Press, &["X"]), group("NextWalkPose", PressType::Press, &["C"])],
            axes: vec![AxisGroup { name: "MoveX".into(), pairs: [[source(&["D"], None), source(&["A"], None)], Default::default()] }],
            double_click: 0.3,
        };
        let mut changed = install.clone();
        changed.keys[0] = group("ReloadWeapon", PressType::Press, &["T", "LeftAlt"]);
        changed.keys[2].variants[1] = source(&["Z"], Some(false));
        changed.axes[0].pairs[1] = [source(&["RightArrow"], None), source(&["LeftArrow"], None)];
        changed.double_click = 0.45;
        let graphics = Graphics { vsync: false, field_of_view: 68.0, mode: Mode::Borderless, resolution: [2560, 1440], texture_quality: 0, mip_streaming: false, shadow_quality: 3, ssao: trp_sim::hbao::SsaoMode::HighestQuality, clouds_quality: trp_sim::clouds::CloudsQuality::High, render_thread: false, game_fps: 90, fps_limit_off: true, msaa: 2, renderer: crate::backend::Renderer::Dx12 };
        let mouse = Mouse { sensitivity: 0.42, aiming: 0.31, optic: 2.5, invert: [false, true] };
        let path = std::env::temp_dir().join(format!("trp-options-test-{}.json", std::process::id()));
        let kept = Features { custom_hands: false, smooth_animations: false, icon_generation: false, compatible_highlight: false, character_model: false, tarkov_ui: false, stamina: false, world_interactions: false, ..Features::default() };
        use crate::battle_view::{BattleModes, Shows};
        let game = BattleModes { quick: Shows::Never, stamina: Shows::Always, health: Shows::Autohide, green_to_red: true, tactical: trp_sim::tactical::InputMode::Hold };
        use crate::ui_sound::Levels;
        let sound = Levels { overall: 7, interface: 4 };
        // Skills, a fraction of a level among them (what a host's level up
        // left toward the next).
        use trp_sim::skills::{Skill, SkillLevels};
        let mut skills = SkillLevels::default();
        skills.set_progress(Skill::CovertMovement, 4000.5);
        skills.set_level(Skill::Strength, 51);
        save(&path, &changed, &graphics, &mouse, &kept, &game, &sound, &skills).unwrap();
        // Read over an install that has since gained a binding: the new
        // one stays, the rest are as kept.
        let mut read = install.clone();
        read.keys.push(group("Jump", PressType::Release, &["Space"]));
        let mut display = Graphics { vsync: true, field_of_view: 50.0, mode: Mode::Windowed, resolution: [1600, 900], texture_quality: 2, mip_streaming: true, shadow_quality: 1, ssao: trp_sim::hbao::SsaoMode::Off, clouds_quality: trp_sim::clouds::CloudsQuality::Low, render_thread: true, game_fps: 120, fps_limit_off: false, msaa: 4, renderer: crate::backend::Renderer::Vulkan };
        let mut pointer = Mouse { sensitivity: 0.7, aiming: 0.5, optic: 1.0, invert: [false; 2] };
        // A switch of the features' list is on until a file says it is not.
        let mut switched = Features::default();
        assert!(switched.custom_hands && switched.tarkov_ui);
        // The GAME page's settings over the install's.
        let mut hud = BattleModes { quick: Shows::Always, stamina: Shows::Autohide, health: Shows::Never, green_to_red: false, tactical: trp_sim::tactical::InputMode::Press };
        let mut volumes = Levels { overall: 1, interface: 1 };
        let mut levels = SkillLevels::default();
        assert_eq!(load(&path, &mut read, &mut display, &mut pointer, &mut switched, &mut hud, &mut volumes, &mut levels), [true, true]);
        assert_eq!(levels, skills, "the skills are kept, the fraction of a level too");
        assert_eq!((levels.level(Skill::CovertMovement), levels.level(Skill::Strength), levels.level(Skill::Endurance)), (40, 51, 0));
        assert_eq!(hud, game, "the battle screen's settings are kept");
        assert_eq!(volumes, sound, "the SOUND page's volumes are kept");
        assert!(!switched.custom_hands && !switched.smooth_animations && !switched.icon_generation && !switched.compatible_highlight && !switched.character_model);
        assert!(!switched.tarkov_ui, "TARKOV UI, on unless switched, is kept off");
        assert_eq!(switched, kept, "every switch is kept, the systems' among them");
        // A file kept before there was such a switch leaves it as it is.
        let older = path.with_extension("older.json");
        std::fs::write(&older, r#"{ "graphics": { "vsync": true } }"#).unwrap();
        let mut untold = Features::default();
        let mut install_hud = BattleModes::default();
        let mut install_volumes = Levels { overall: 1, interface: 1 };
        let mut untrained = SkillLevels::default();
        assert_eq!(load(&older, &mut read.clone(), &mut display.clone(), &mut pointer.clone(), &mut untold, &mut install_hud, &mut install_volumes, &mut untrained), [false, true]);
        assert!(untrained.untrained(), "a file without them leaves every skill at 0");
        assert_eq!(install_volumes, Levels { overall: 1, interface: 1 }, "a file without them leaves the install's volumes");
        assert_eq!(untold, Features::default());
        assert_eq!(install_hud, BattleModes::default(), "a file without them leaves the install's");
        let _ = std::fs::remove_file(&older);
        let _ = std::fs::remove_file(&path);
        assert_eq!(read.keys[..3], changed.keys[..]);
        assert_eq!(read.keys[3], group("Jump", PressType::Release, &["Space"]));
        assert_eq!(read.axes, changed.axes);
        assert_eq!(read.double_click, 0.45);
        assert_eq!(display, graphics);
        assert_eq!(pointer, mouse);
        // No file: nothing read, nothing changed.
        assert_eq!(load(&path, &mut read, &mut display, &mut pointer, &mut switched, &mut hud, &mut volumes, &mut levels), [false, false]);
        assert_eq!(pointer, mouse);
        assert!(!switched.custom_hands);
    }

    #[test]
    fn a_simple_group_off_holds_its_parts_off_and_keeps_their_switches() {
        let mut kept = Features::default();
        kept.sounds = false;
        kept.shot_sounds = false;
        kept.visual_effects = false;
        kept.stamina = false;
        let held = kept.effective();
        // The group's parts are off whatever their own switches say, and
        // the file keeps each part's own switch for when the group is on.
        assert!(!held.movement_sounds && !held.hands_sounds && !held.shot_sounds, "the sounds' three");
        assert!(kept.movement_sounds && kept.hands_sounds && !kept.shot_sounds, "their own switches are kept");
        assert!(!held.muzzle_flash && !held.particles && !held.spent_cases && !held.magazine_rounds && !held.face_shield_mask, "the effects' five");
        assert!(!held.stamina_drain && !held.exhaustion, "the stamina's two");
        assert!(!held.systems().stamina && !held.systems().exhaustion);
        assert!(held.health && held.ballistics && held.movement && held.tarkov_ui, "the rest as they are");
        // With every group on, each part is its own switch.
        let mut own = Features::default();
        own.shot_sounds = false;
        assert_eq!(own.effective(), own);
        assert!(FeatureSwitch::ALL.iter().all(|switch| switch.group().is_none_or(|group| SIMPLE.contains(&group))), "a group is a SIMPLE switch");
        // Every switch is on exactly one of the three lists, but the groups'
        // parts, which are under ADVANCED alone.
        let listed: Vec<FeatureSwitch> = SIMPLE.iter().chain(EXPERIMENTAL.iter()).chain(ADVANCED.iter().flat_map(|group| group.iter())).copied().collect();
        assert_eq!(listed.len(), FeatureSwitch::ALL.len());
        for switch in FeatureSwitch::ALL {
            assert_eq!(listed.iter().filter(|on| **on == switch).count(), 1, "{}", switch.key());
        }
    }

    #[test]
    fn a_bar_s_figure_is_written_as_the_game_writes_it() {
        assert_eq!(Slide::Sensitivity.shown(0.7), "0.700");
        assert_eq!(Slide::DoubleClick.shown(0.3), "0.30");
        assert_eq!(Slide::Fov.shown(74.0), "74");
        assert_eq!(Slide::GameFps.shown(120.0), "120");
        for kind in [Slide::Fov, Slide::GameFps, Slide::Sensitivity, Slide::Aiming, Slide::DoubleClick, Slide::Optic] {
            let [low, high] = kind.range();
            assert!(low < high && kind.step() > 0.0, "{kind:?}");
        }
    }
}
