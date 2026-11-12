//! The hearing parameters and choosing a template by what is worn.

use super::*;

/// The mixer's values the default configurator sets from a template, in
/// its order, by the names it sets them by (`Param::name`). It first sets
/// `Tinnitus1` and `Tinnitus2` again to what they hold, left out here: the
/// tinnitus' sends are not built.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Param {
    GunsSend,
    ClientPlayerSend,
    ObservedPlayerSend,
    NpcSend,
    EnvTechnicalSend,
    EnvNatureSend,
    EnvCommonSend,
    AmbientSend,
    EffectsReturnsSend,
    HeadphonesVolume,
    OcclusionVolume,
    EnvironmentVolume,
    AmbientVolume,
    AmbientOccluded,
    EffectsReturnsGroupVolume,
    GunsVolume,
    OutEnvironmentVolume,
}

impl Param {
    pub const ALL: [Self; 17] = [
        Self::GunsSend,
        Self::ClientPlayerSend,
        Self::ObservedPlayerSend,
        Self::NpcSend,
        Self::EnvTechnicalSend,
        Self::EnvNatureSend,
        Self::EnvCommonSend,
        Self::AmbientSend,
        Self::EffectsReturnsSend,
        Self::HeadphonesVolume,
        Self::OcclusionVolume,
        Self::EnvironmentVolume,
        Self::AmbientVolume,
        Self::AmbientOccluded,
        Self::EffectsReturnsGroupVolume,
        Self::GunsVolume,
        Self::OutEnvironmentVolume,
    ];

    /// The name the game sets it by: an exposed parameter of the mixer
    /// (`trp_data::sound::Mixer::exposed`).
    pub fn name(self) -> &'static str {
        match self {
            Self::GunsSend => "GunsCompressorSendLevel",
            Self::ClientPlayerSend => "ClientPlayerCompressorSendLevel",
            Self::ObservedPlayerSend => "ObservedPlayerCompressorSendLevel",
            Self::NpcSend => "NpcCompressorSendLevel",
            Self::EnvTechnicalSend => "EnvTechnicalCompressorSendLevel",
            Self::EnvNatureSend => "EnvNatureCompressorSendLevel",
            Self::EnvCommonSend => "EnvCommonCompressorSendLevel",
            Self::AmbientSend => "AmbientCompressorSendLevel",
            Self::EffectsReturnsSend => "EffectsReturnsCompressorSendLevel",
            Self::HeadphonesVolume => "HeadphonesMixerVolume",
            Self::OcclusionVolume => "OcclusionVolume",
            Self::EnvironmentVolume => "EnvironmentVolume",
            Self::AmbientVolume => "AmbientVolume",
            Self::AmbientOccluded => "AmbientOccluded",
            Self::EffectsReturnsGroupVolume => "EffectsReturnsGroupVolume",
            Self::GunsVolume => "GunsVolume",
            Self::OutEnvironmentVolume => "OutEnvironmentVolume",
        }
    }
}

/// What chose the template (`UpdatePhonesReally`, in its order: the first
/// that is not the default's wins).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    /// A headset: in the earpiece slot, or among the headwear's parts. Its
    /// own template is worn over any cut.
    Headset,
    /// The headwear's cut, else the face cover's (none: the default).
    Deaf(Deaf),
}

impl Choice {
    pub fn template(self) -> &'static Template {
        match self {
            // PROVISIONAL: a headset's own template (its compressor,
            // distortion, filters and rolloff, from the item database) is
            // not built: today's sound.
            Self::Headset => &DEFAULT,
            Self::Deaf(deaf) => Template::by_deaf(deaf),
        }
    }
}

fn is_a(item: &Item, class: &str) -> bool {
    item.kind.class_names.iter().any(|name| name == class)
}

/// Whether a thing has an armour component (`ArmoredEquipment`'s
/// constructor: its template's greatest durability and armour class both
/// above nought): only then does a part's or a face cover's cut count.
fn armoured(item: &Item) -> bool {
    item.kind.durability[1] > 0.0 && item.kind.armor_class > 0
}

/// The template's choice by what is worn (`UpdatePhonesReally`): a headset
/// in the earpiece slot or fitted to the headwear; else, for headwear that
/// is armoured equipment, the strongest of its own cut and its armoured
/// parts' (`GetItemComponentsInChildren<CompositeArmorComponent>`); else
/// the strongest of the face cover's armoured self and parts. No switch's
/// state is read: a face shield up or down changes nothing.
pub fn choose(equipment: &Equipment) -> Choice {
    let headwear = equipment.worn("Headwear");
    let earpiece = equipment.worn("Earpiece").is_some_and(|item| is_a(item, "Headphones"));
    if earpiece || headwear.is_some_and(|headwear| with_parts(headwear).any(|part| is_a(part, "Headphones"))) {
        return Choice::Headset;
    }
    if let Some(headwear) = headwear.filter(|headwear| is_a(headwear, "ArmoredEquipment")) {
        let deaf = with_parts(headwear).filter(|part| armoured(part)).map(|part| part.kind.deaf).fold(headwear.kind.deaf, Ord::max);
        if deaf != Deaf::None {
            return Choice::Deaf(deaf);
        }
    }
    let cover = equipment.worn("FaceCover").map_or(Deaf::None, |cover| with_parts(cover).filter(|part| armoured(part)).map(|part| part.kind.deaf).max().unwrap_or_default());
    Choice::Deaf(cover)
}

/// The slots the choice looks in.
pub(super) const SLOTS: [&str; 3] = ["Earpiece", "Headwear", "FaceCover"];

/// What the choice goes by, as one number: the things in the earpiece,
/// headwear and face cover slots and everything fitted to them. A host
/// applies the template again when it changes (`PROVISIONAL`: the game
/// does on any thing moved to or from a slot anywhere, and after a round
/// is chambered, which tells only while a fade runs).
pub fn worn_key(equipment: &Equipment) -> u64 {
    let mix = |key: u64, value: u64| (key ^ value).wrapping_mul(0x0000_0100_0000_01b3);
    let mut key = 0xcbf2_9ce4_8422_2325;
    for slot in SLOTS {
        key = mix(key, u64::MAX);
        for part in equipment.worn(slot).into_iter().flat_map(with_parts) {
            key = mix(key, part.id);
        }
    }
    key
}
