//! The options' values: mode, graphics, mouse, and the features' defaults.

use super::*;

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
    pub(super) const ALL: [Self; 3] = [Self::Fullscreen, Self::Borderless, Self::Windowed];

    pub(super) fn word(self) -> &'static str {
        match self {
            Self::Fullscreen => "FULLSCREEN",
            Self::Borderless => "BORDERLESS",
            Self::Windowed => "WINDOWED",
        }
    }

    /// The game's own figure for a mode in its settings file.
    pub(super) fn of_game(figure: i64) -> Self {
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
    pub ssao: trp_sim::lighting::hbao::SsaoMode,
    /// The game's clouds' quality (`CloudsQuality`; `clouds_view`), taken
    /// at the next start.
    pub clouds_quality: trp_sim::lighting::clouds::CloudsQuality,
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
    /// The dust's and smoke's resolution (`offscreen_dust`): the app's own,
    /// as the game draws them whole (FULL). Taken at once.
    pub dust_resolution: crate::effects::offscreen_dust::DustResolution,
    /// The graphics backend (`backend`), taken at the next start.
    pub renderer: crate::platform::backend::Renderer,
    /// The levels of detail's bias (`OBJECT LOD QUALITY`, the game's
    /// `LodBias`: 2 to 4 by halves, the install's unless chosen), taken at
    /// once; TRP's own rule's tolerance, `2 / bias` pixels of
    /// a level's deviation (`trp_sim::lod::tolerance`, `lod_view`).
    pub lod_bias: f32,
}

/// The game's limits for its field-of-view setting (`GameSettingsGroup`).
pub(super) const FOV: [f32; 2] = [50.0, 75.0];

/// The game's limits for its frame rate limit in a raid
/// (`GraphicsSettingsController.MinFramerateLimit`, `MaxFramerateGameLimit`;
/// this install's server settings, `SPT_Data/database/settings.json`'s
/// `FramerateLimit`, have the same), and the setting's default
/// (`GraphicsSettingsGroup`: 120).
pub const GAME_FPS: [u32; 2] = [30, 144];

pub(super) const GAME_FPS_DEFAULT: u32 = 120;

/// The choices of the app's multisampling, by their samples.
pub(super) const MSAA: [(u8, &str); 3] = [(1, "OFF"), (2, "2X"), (4, "4X")];

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

impl Default for Features {
    /// The fields as the FEATURES page's recommended switches set them.
    fn default() -> Self {
        Switches::default().features()
    }
}

impl Features {
    /// Every field on: what the systems were before any switch, for a
    /// test that wants all of them.
    #[cfg(test)]
    pub(super) fn all_on() -> Self {
        Self {
            custom_hands: true,
            smooth_animations: true,
            grounded_feet: true,
            visual_effects: true,
            spent_cases: true,
            magazine_rounds: true,
            face_shield_mask: true,
            face_shield_cracks: true,
            muzzle_flash: true,
            particles: true,
            tracers: true,
            // Off unless switched.
            customs_light: true,
            camera_color_filter: true,
            camera_sharpening: true,
            sky_scattering: true,
            customs_clouds: true,
            ambient_highlight: true,
            tarkov_shading: true,
            ambient_occlusion: true,
            hall_lamps: true,
            icon_generation: true,
            compatible_highlight: true,
            character_model: true,
            // On unless switched.
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
            weapons_on_body: true,
            wrist_watch: true,
            head_fitting: true,
            armband: true,
            overall_figure: true,
            distance_detail: true,
            inventory: true,
            world_interactions: true,
            health: true,
            skills: true,
            weapon_mastering: true,
            experience: true,
            simple_xp_gain: true,
            gestures: true,
            item_physics: true,
            world_distance_detail: true,
            sounds: true,
            movement_sounds: true,
            hands_sounds: true,
            shot_sounds: true,
            world_sounds: true,
            headgear_hearing: true,
            grenades_and_knife: true,
            ballistics: true,
            dummy: true,
            reactive_targets: true,
            posing_economy: true,
            attachments: true,
            workbench: true,
            armor_bench: true,
            raid_limits: true,
            examining: true,
            searching: true,
            hide_unfinished: true,
            clothing: true,
            tactical_devices: true,
            blind_fire: true,
            mounting: true,
            left_shoulder: true,
            underbarrel_launcher: true,
            durability: true,
            malfunctions: true,
            overheating: true,
            see_through_glass: true,
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
    pub fn wear(&self) -> trp_sim::weapons::firearm::WearSwitches {
        trp_sim::weapons::firearm::WearSwitches { durability: self.durability, malfunctions: self.malfunctions, overheating: self.overheating }
    }

    /// The switches as they hold for a run: a part of a `SIMPLE` group is
    /// on only while its group is too; its own switch is kept as it is
    /// for when the group is on again (settings spec, "Features").
    pub fn effective(&self) -> Self {
        let mut held = self.clone();
        for part in Part::ALL {
            if part.group().is_some_and(|group| !group.get(self)) {
                *part.slot(&mut held) = false;
            }
        }
        held
    }
}
