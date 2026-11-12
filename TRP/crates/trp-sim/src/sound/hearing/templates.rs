//! Deafness and the hearing templates with their presets.

use super::*;

/// How much a thing worn cuts what is heard (`EDeafStrength`), in its
/// order: the strongest of several counts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Deaf {
    #[default]
    None,
    Low,
    High,
}

impl Deaf {
    /// By the item database's word (`DeafStrength`); any other none.
    pub fn of_word(word: &str) -> Self {
        match word {
            "Low" => Self::Low,
            "High" => Self::High,
            _ => Self::None,
        }
    }
}

/// A headphones template's figures (`HeadphonesTemplate`): the levels (dB)
/// the default configurator sets by name, then what is not applied
/// (`PROVISIONAL`, above).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Template {
    /// Its short name: `Default` is the template of nothing worn
    /// (`Extensions.IsDefault`).
    pub name: &'static str,
    /// The sends into the bus's compressors, by the group they leave
    /// (`GunsCompressorSendLevel` and on, `Param`'s first nine).
    pub guns_send: f32,
    pub client_player_send: f32,
    pub observed_player_send: f32,
    pub npc_send: f32,
    pub env_technical_send: f32,
    pub env_nature_send: f32,
    pub env_common_send: f32,
    pub ambient_send: f32,
    pub effects_returns_send: f32,
    /// The bus's own volume (`HeadphonesMixerVolume`).
    pub headphones_volume: f32,
    /// The dry level: the guns', the environment's, the occluded sounds'
    /// and the outdoor environment's groups' volumes.
    pub dry_volume: f32,
    /// The ambience's group's volume; its occluded send 20 dB under it (5
    /// for the default template).
    pub ambient_volume: f32,
    /// The reverb and delay returns' group's volume.
    pub effects_returns_group_volume: f32,
    /// Not applied: the compressors' attack, gain, release and threshold,
    /// their distortion, the bus's high pass (Hz, resonance) and low pass
    /// (Hz), and its three bands' gains.
    pub compressor: [f32; 4],
    pub distortion: f32,
    pub high_pass: [f32; 2],
    pub low_pass: f32,
    pub bands: [f32; 3],
}

/// Nothing worn (`HeadphonesLocalTemplatesContainer.Default`, read from the
/// game's code: the spec): the dry way whole, every send and the bus at
/// the mixer's least.
pub const DEFAULT: Template = Template {
    name: "Default",
    guns_send: -80.0,
    client_player_send: -80.0,
    observed_player_send: -80.0,
    npc_send: -80.0,
    env_technical_send: -80.0,
    env_nature_send: -80.0,
    env_common_send: -80.0,
    ambient_send: -80.0,
    effects_returns_send: -80.0,
    headphones_volume: -80.0,
    dry_volume: 0.0,
    ambient_volume: 0.0,
    effects_returns_group_volume: 0.0,
    compressor: [0.0; 4],
    distortion: 0.0,
    high_pass: [60.0, 1.0],
    low_pass: 22000.0,
    bands: [1.0; 3],
};

/// A `Low` cut (`LowMute`, the same source).
pub const LOW_MUTE: Template = Template {
    name: "LowMute",
    guns_send: 0.0,
    client_player_send: -1.0,
    observed_player_send: 0.0,
    npc_send: 0.0,
    env_technical_send: -2.0,
    env_nature_send: -2.0,
    env_common_send: 0.0,
    ambient_send: -2.0,
    effects_returns_send: -80.0,
    headphones_volume: -3.0,
    dry_volume: -50.0,
    ambient_volume: -50.0,
    effects_returns_group_volume: -5.0,
    compressor: [0.0; 4],
    distortion: 0.0,
    high_pass: [60.0, 1.0],
    low_pass: 21000.0,
    bands: [1.0; 3],
};

/// A `High` cut (`StrongMute`, the same source).
pub const STRONG_MUTE: Template = Template {
    name: "StrongMute",
    guns_send: 0.0,
    client_player_send: -1.0,
    observed_player_send: 0.0,
    npc_send: 0.0,
    env_technical_send: -2.0,
    env_nature_send: -2.0,
    env_common_send: 0.0,
    ambient_send: -5.0,
    effects_returns_send: -80.0,
    headphones_volume: -5.0,
    dry_volume: -50.0,
    ambient_volume: -50.0,
    effects_returns_group_volume: -7.0,
    compressor: [0.0; 4],
    distortion: 0.0,
    high_pass: [60.0, 1.1],
    low_pass: 18000.0,
    bands: [1.0; 3],
};

impl Template {
    /// Whether it is the template of nothing worn (`IsDefault`: by name).
    pub fn is_default(&self) -> bool {
        self.name == "Default"
    }

    /// By a thing's cut (`GetHeadphonesTemplateByDeaf`).
    pub fn by_deaf(deaf: Deaf) -> &'static Self {
        match deaf {
            Deaf::None => &DEFAULT,
            Deaf::Low => &LOW_MUTE,
            Deaf::High => &STRONG_MUTE,
        }
    }

    /// What it sets a value of the mixer to
    /// (`ActiveHeadphonesDefaultConfigurator.Configure`).
    pub fn value(&self, param: Param) -> f32 {
        match param {
            Param::GunsSend => self.guns_send,
            Param::ClientPlayerSend => self.client_player_send,
            Param::ObservedPlayerSend => self.observed_player_send,
            Param::NpcSend => self.npc_send,
            Param::EnvTechnicalSend => self.env_technical_send,
            Param::EnvNatureSend => self.env_nature_send,
            Param::EnvCommonSend => self.env_common_send,
            Param::AmbientSend => self.ambient_send,
            Param::EffectsReturnsSend => self.effects_returns_send,
            Param::HeadphonesVolume => self.headphones_volume,
            Param::OcclusionVolume | Param::EnvironmentVolume | Param::GunsVolume | Param::OutEnvironmentVolume => self.dry_volume,
            Param::AmbientVolume => self.ambient_volume,
            Param::AmbientOccluded => self.ambient_volume - if self.is_default() { 5.0 } else { 20.0 },
            Param::EffectsReturnsGroupVolume => self.effects_returns_group_volume,
        }
    }
}
