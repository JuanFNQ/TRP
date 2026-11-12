//! The master mixer, its groups and effects, aiming banks and headphones.

use super::*;

/// The bundle of the game's audio mixer (`MasterMixer`, Unity's
/// `AudioMixerController` as built: its groups, effects and snapshots
/// compiled into one `AudioMixerConstant`).
pub const MASTER_MIXER: &str = "assets/content/audio/mastermixer.bundle";

/// A group of the game's mixer: its name, its parent's place, its volume
/// in the start snapshot (dB), whether a script may set that volume (an
/// exposed parameter), and its effects.
#[derive(Debug, Clone)]
pub struct MixerGroup {
    pub name: String,
    pub parent: Option<usize>,
    pub volume_db: f32,
    pub exposed: bool,
    pub effects: Vec<MixerEffect>,
    /// Where its volume is among a snapshot's values.
    pub volume_index: Option<usize>,
}

/// An effect of a group: its kind as the build numbers it (negative for the
/// mixer's own: -2 the group's attenuation, -3 a send, -4 a return; else
/// as read, FMOD's own numbering of its effects, 16 its compressor,
/// 17 its reverb, 3 and 5 its low and high pass, 10 its equaliser) and its
/// parameters' values in the start snapshot.
#[derive(Debug, Clone)]
pub struct MixerEffect {
    pub kind: i64,
    pub parameters: Vec<f32>,
    /// Where each parameter is among a snapshot's values.
    pub indices: Vec<usize>,
    /// For a send (-3), the group it sends to (`sendTargetEffectIndex`'s
    /// effect's group), where it names one.
    pub target: Option<usize>,
    /// For a send, its level (`wetMixLevelIndex`): where it is among a
    /// snapshot's values, and the start snapshot's (dB). A send keeps its
    /// level there, not among its parameters (it has none).
    pub level: Option<(usize, f32)>,
}

/// What an exposed parameter of the mixer sets: a group's volume, a send's
/// level, or a parameter of another effect of a group (a filter's
/// frequency, a compressor's threshold).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Exposed {
    Volume { group: usize },
    Send { group: usize, effect: usize },
    Effect { group: usize, effect: usize, parameter: usize },
}

/// The bundle of the weapons' other sounds (`WeaponSounds`: the shells',
/// the grenades', the mount's, and the aiming's by the weapon's kind).
pub const WEAPON_SOUNDS: &str = "assets/content/audio/prefabs/shells/weaponsounds.bundle";

/// A weapon's kind of sound (`ESoundWeaponType`, `AudioWeaponUtils.
/// GetWeaponType`) by its template's classes: a grenade launcher
/// `Launcher`; an assault rifle or carbine, a marksman or sniper rifle
/// `Riffle` (the game's spelling); `Shotgun`; `MachineGun`; a pistol or a
/// revolver `Pistol`; `SMG`; any other weapon `SpecialWeapon`.
pub fn sound_weapon_kind(db: &crate::Database, template: &crate::setup::database::ItemTemplate) -> &'static str {
    let is = |class: &str| db.is_a(template, class);
    if is("GrenadeLauncher") {
        "Launcher"
    } else if is("AssaultRifle") || is("AssaultCarbine") || is("MarksmanRifle") || is("SniperRifle") {
        "Riffle"
    } else if is("Shotgun") {
        "Shotgun"
    } else if is("MachineGun") {
        "MachineGun"
    } else if is("Pistol") || is("Revolver") {
        "Pistol"
    } else if is("Smg") {
        "SMG"
    } else {
        "SpecialWeapon"
    }
}

/// A bank of the aiming's sounds (`SoundBankWithSettings`): its clips,
/// volume and rolloff.
#[derive(Debug, Clone)]
pub struct AimingBank {
    pub name: String,
    pub volume: f32,
    pub rolloff: f32,
    pub clips: Vec<Clip>,
}

/// The aiming's sounds (`WeaponSounds.AimingSounds`, a
/// `WeaponAimingSoundsSO` in a bundle `WEAPON_SOUNDS` depends on): each
/// kind of weapon's bank by the kind's name (`_keys`, `_values`), kinds
/// that share a bank sharing its clips' reading.
pub fn aiming_sounds(install: &Install) -> Result<Vec<(String, std::sync::Arc<AimingBank>)>> {
    use crate::unity::{class, extract};
    let mut setups = VorbisSetups::new(install);
    let mut assets = crate::unity::Assets::new(install.clone())?;
    let files = assets.load(WEAPON_SOUNDS)?;
    let sounds = files
        .iter()
        .flat_map(|&file| assets.objects_of(file, class::MONO_BEHAVIOUR).collect::<Vec<_>>())
        .find(|&obj| extract::script_class(&assets, obj).as_deref() == Some("WeaponSounds"))
        .context("no WeaponSounds in its bundle")?;
    let value = assets.read(sounds)?;
    let so = value.get("AimingSounds").and_then(Value::as_pptr).and_then(|p| assets.resolve_loaded(sounds.file, p)).context("the aiming's sounds lead nowhere loaded")?;
    let so_value = assets.read(so)?;
    let table = so_value.get("_aimingSounds").context("no _aimingSounds")?;
    let keys = table.get("_keys").and_then(Value::as_array).unwrap_or(&[]);
    let values = table.get("_values").and_then(Value::as_array).unwrap_or(&[]);
    let mut read: HashMap<crate::unity::ObjRef, std::sync::Arc<AimingBank>> = HashMap::new();
    let mut out = Vec::new();
    for (key, pointer) in keys.iter().zip(values) {
        let (Some(kind), Some(at)) = (key.as_str(), pointer.as_pptr().and_then(|p| assets.resolve_loaded(so.file, p))) else { continue };
        if let Some(bank) = read.get(&at) {
            out.push((kind.to_owned(), bank.clone()));
            continue;
        }
        let bank = assets.read(at)?;
        let mut clips = Vec::new();
        for pointer in bank.get("Clips").and_then(Value::as_array).unwrap_or(&[]) {
            let Some(clip) = pointer.as_pptr().and_then(|p| assets.resolve_loaded(at.file, p)) else { continue };
            let value = assets.read(clip)?;
            let resource = |source: &str, offset: usize, size: usize| -> Result<Vec<u8>> { Ok(assets.resource(source, offset, size)?.to_vec()) };
            match clip_of(&value, &resource, &mut setups) {
                Ok(clip) => clips.push(clip),
                Err(e) => eprintln!("an aiming sound could not be read: {e:#}"),
            }
        }
        let made = std::sync::Arc::new(AimingBank {
            name: bank.get("m_Name").and_then(Value::as_str).unwrap_or_default().to_owned(),
            volume: bank.get("BaseVolume").and_then(Value::as_f32).unwrap_or(1.0),
            rolloff: bank.get("Rolloff").and_then(Value::as_f32).unwrap_or(100.0),
            clips,
        });
        read.insert(at, made.clone());
        out.push((kind.to_owned(), made));
    }
    Ok(out)
}

/// The game's mixer as it starts (`m_StartSnapshot`).
#[derive(Debug, Clone, Default)]
pub struct Mixer {
    /// The exposed parameters (`exposedParameterNames`, each a CRC32 of
    /// the name scripts set it by, and `exposedParameterIndices`, where its
    /// value is).
    pub exposed: Vec<(u32, usize)>,
    pub start_snapshot: String,
    pub groups: Vec<MixerGroup>,
}

/// The effect kind read as FMOD's compressor (`FMOD_DSP_TYPE_COMPRESSOR`):
/// its parameters a threshold (dB), an attack and a release (ms) and a
/// make-up gain (dB).
pub const COMPRESSOR: i64 = 16;

impl Mixer {
    pub fn group(&self, name: &str) -> Option<usize> {
        self.groups.iter().position(|group| group.name == name)
    }

    /// What the exposed parameter of that name sets (`AudioMixer.SetFloat`'s
    /// name, found by its CRC32), where the mixer has it.
    pub fn exposed(&self, name: &str) -> Option<Exposed> {
        let hash = crc32fast::hash(name.as_bytes());
        let index = self.exposed.iter().find(|(named, _)| *named == hash)?.1;
        for (g, group) in self.groups.iter().enumerate() {
            if group.volume_index == Some(index) {
                return Some(Exposed::Volume { group: g });
            }
            for (e, effect) in group.effects.iter().enumerate() {
                if effect.level.is_some_and(|(at, _)| at == index) {
                    return Some(Exposed::Send { group: g, effect: e });
                }
                if let Some(parameter) = effect.indices.iter().position(|&i| i == index) {
                    return Some(Exposed::Effect { group: g, effect: e, parameter });
                }
            }
        }
        None
    }

    /// The start snapshot's value of the exposed parameter of that name
    /// (what `AudioMixer.GetFloat` gives before any script has set it): a
    /// group's volume or a send's level in dB, or an effect's parameter.
    pub fn value(&self, name: &str) -> Option<f32> {
        Some(match self.exposed(name)? {
            Exposed::Volume { group } => self.groups[group].volume_db,
            Exposed::Send { group, effect } => self.groups[group].effects[effect].level?.1,
            Exposed::Effect { group, effect, parameter } => *self.groups[group].effects[effect].parameters.get(parameter)?,
        })
    }

    /// A group's volume with every group above it, in dB, as the start
    /// snapshot has them.
    pub fn path_db(&self, group: usize) -> f32 {
        let mut at = Some(group);
        let mut db = 0.0;
        while let Some(index) = at {
            db += self.groups[index].volume_db;
            at = self.groups[index].parent;
        }
        db
    }

    /// The master group's compressor, if it has one: threshold (dB),
    /// attack and release (s), make-up gain (dB).
    pub fn master_compressor(&self) -> Option<[f32; 4]> {
        let master = self.groups.iter().find(|group| group.parent.is_none() && group.name == "Master")?;
        let effect = master.effects.iter().find(|effect| effect.kind == COMPRESSOR && effect.parameters.len() >= 4)?;
        let p = &effect.parameters;
        Some([p[0], p[1] / 1000.0, p[2] / 1000.0, p[3]])
    }
}

/// The game's mixer (`MASTER_MIXER`): each group named by its GUID among
/// the bundle's group objects, its volume and its effects' parameters by
/// their places in the start snapshot's values.
pub fn master_mixer(install: &Install) -> Result<Mixer> {
    use crate::unity::ObjRef;
    let mut assets = crate::unity::Assets::new(install.clone())?;
    let file = *assets.load(MASTER_MIXER)?.first().context("the mixer's bundle is empty")?;
    let guid = |v: &Value| -> Option<[i64; 4]> { Some([v.get("data[0]")?.as_i64()?, v.get("data[1]")?.as_i64()?, v.get("data[2]")?.as_i64()?, v.get("data[3]")?.as_i64()?]) };
    let mut names: HashMap<[i64; 4], String> = HashMap::new();
    let mut mixer = None;
    let objects: Vec<(i64, i32)> = assets.file(file).meta.objects.iter().map(|o| (o.path_id, o.class_id)).collect();
    for (path_id, class_id) in objects {
        let object = ObjRef { file, path_id };
        match class_id {
            243 | 245 => {
                let value = assets.read(object)?;
                let id = value.get(if class_id == 243 { "m_GroupID" } else { "m_SnapshotID" }).and_then(guid);
                if let Some(id) = id {
                    names.insert(id, value.get("m_Name").and_then(Value::as_str).unwrap_or_default().to_owned());
                }
            }
            241 => mixer = Some((object, assets.read(object)?)),
            _ => {}
        }
    }
    let (object, mixer) = mixer.context("no mixer in its bundle")?;
    let constant = mixer.get("m_MixerConstant").context("the mixer has no constant")?;
    let list = |name: &str| constant.get(name).and_then(Value::as_array).unwrap_or(&[]);
    // The start snapshot's place among the snapshots, by its GUID.
    let start = mixer
        .get("m_StartSnapshot")
        .and_then(Value::as_pptr)
        .and_then(|p| assets.resolve_loaded(object.file, p))
        .and_then(|snapshot| assets.read(snapshot).ok())
        .and_then(|snapshot| snapshot.get("m_SnapshotID").and_then(guid));
    let snapshot_ids: Vec<Option<[i64; 4]>> = list("snapshotGUIDs").iter().map(guid).collect();
    let at = snapshot_ids.iter().position(|id| id.is_some() && *id == start).context("the start snapshot is not among the snapshots")?;
    let values = list("snapshots").get(at).and_then(|s| s.get("values")).and_then(Value::as_floats).context("the start snapshot has no values")?;
    let exposed: Vec<u32> = match constant.get("exposedParameterIndices") {
        Some(Value::UInts(indices)) => indices.clone(),
        _ => Vec::new(),
    };
    let int = |v: &Value, name: &str| v.get(name).and_then(Value::as_i64).unwrap_or(-1);
    let group_ids: Vec<Option<[i64; 4]>> = list("groupGUIDs").iter().map(guid).collect();
    let mut groups: Vec<MixerGroup> = list("groups")
        .iter()
        .enumerate()
        .map(|(n, group)| {
            let volume = int(group, "volumeIndex");
            MixerGroup {
                name: group_ids.get(n).copied().flatten().and_then(|id| names.get(&id).cloned()).unwrap_or_default(),
                parent: usize::try_from(int(group, "parentConstantIndex")).ok(),
                volume_db: usize::try_from(volume).ok().and_then(|i| values.get(i)).copied().unwrap_or(0.0),
                exposed: exposed.contains(&(volume as u32)),
                effects: Vec::new(),
                volume_index: usize::try_from(volume).ok(),
            }
        })
        .collect();
    let effects = list("effects");
    for effect in effects {
        let group_of = |effect: &Value| usize::try_from(int(effect, "groupConstantIndex")).ok();
        let Some(at) = group_of(effect) else { continue };
        let indices: Vec<usize> = match effect.get("parameterIndices") {
            Some(Value::UInts(indices)) => indices.iter().map(|&i| i as usize).collect(),
            _ => Vec::new(),
        };
        let parameters = indices.iter().map(|&i| values.get(i).copied().unwrap_or(0.0)).collect();
        let target = usize::try_from(int(effect, "sendTargetEffectIndex")).ok().and_then(|e| effects.get(e)).and_then(group_of);
        // A send's level, where it has one (none is 0xFFFFFFFF, past the values).
        let level = usize::try_from(int(effect, "wetMixLevelIndex")).ok().and_then(|i| Some((i, *values.get(i)?)));
        let Some(group) = groups.get_mut(at) else { continue };
        group.effects.push(MixerEffect { kind: int(effect, "type"), parameters, indices, target, level });
    }
    let names_hashed: Vec<u32> = match constant.get("exposedParameterNames") {
        Some(Value::UInts(hashes)) => hashes.clone(),
        _ => Vec::new(),
    };
    let exposed = names_hashed.into_iter().zip(exposed.iter().map(|&i| i as usize)).collect();
    let start_snapshot = start.and_then(|id| names.get(&id).cloned()).unwrap_or_default();
    Ok(Mixer { exposed, start_snapshot, groups })
}

/// The game's fades of a sound or a mixer's value (`EAudioFadeType`, in its
/// order; `AudioUtils.GetEasingFunction`): each takes the share of a fade's
/// time gone, 0 to 1, to the share of the way gone (sound spec, "Hearing
/// under headgear").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FadeType {
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    Exponential,
    InverseExponential,
    Logarithmic,
    SmoothStep,
    Logistic,
}

impl FadeType {
    pub const ALL: [Self; 9] = [Self::Linear, Self::EaseIn, Self::EaseOut, Self::EaseInOut, Self::Exponential, Self::InverseExponential, Self::Logarithmic, Self::SmoothStep, Self::Logistic];

    /// By its name as the settings write it (`"EaseIn"`), or its number.
    pub fn of(value: &serde_json::Value) -> Option<Self> {
        match value {
            serde_json::Value::String(name) => Self::ALL.into_iter().find(|fade| format!("{fade:?}") == *name),
            other => Self::ALL.get(usize::try_from(other.as_u64()?).ok()?).copied(),
        }
    }

    /// The share of the way gone at `t`, as the game's own reckons it in
    /// single precision (its powers in double, as `Mathf.Pow`).
    pub fn ease(self, t: f32) -> f32 {
        let pow = |base: f32, power: f32| (base as f64).powf(power as f64) as f32;
        match self {
            Self::Linear => t,
            Self::EaseIn => t * t,
            Self::EaseOut => 1.0 - (1.0 - t) * (1.0 - t),
            Self::EaseInOut => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    1.0 - pow(-2.0 * t + 2.0, 2.0) / 2.0
                }
            }
            Self::Exponential => pow(2.0, 10.0 * (t - 1.0)),
            Self::InverseExponential => 1.0 - pow(2.0, -10.0 * t),
            Self::Logarithmic => (t * 9.0 + 1.0).log10(),
            Self::SmoothStep => t * t * t * (t * (t * 6.0 - 15.0) + 10.0),
            Self::Logistic => 1.0 / (1.0 + (-10.0 * (t - 0.5)).exp()),
        }
    }
}

/// How the headphones' templates fade the mixer
/// (`ClientAudioSettings.ActiveHeadphonesSettings`): over how long, by
/// which fade where a value rises above the one last asked of it, and by
/// which otherwise (and the first time it is asked).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeadphonesSettings {
    pub fade_duration: f32,
    pub fade_in: FadeType,
    pub fade_out: FadeType,
}

impl Default for HeadphonesSettings {
    /// The class's own figures (0.7 s, `InverseExponential`, `EaseIn`):
    /// what a field the backend leaves out keeps.
    fn default() -> Self {
        Self { fade_duration: 0.7, fade_in: FadeType::InverseExponential, fade_out: FadeType::EaseIn }
    }
}

/// The headphones' fades from the client settings (`settings.json`'s
/// `config`, `Database::client`: `AudioSettings.HeadphonesSettings`), each
/// field the backend leaves out at the class's own.
pub fn headphones_settings(client: &serde_json::Value) -> HeadphonesSettings {
    let mut settings = HeadphonesSettings::default();
    if let Some(given) = client.pointer("/AudioSettings/HeadphonesSettings") {
        if let Some(duration) = given.get("FadeDuration").and_then(serde_json::Value::as_f64) {
            settings.fade_duration = duration as f32;
        }
        if let Some(fade) = given.get("FadeIn").and_then(FadeType::of) {
            settings.fade_in = fade;
        }
        if let Some(fade) = given.get("FadeOut").and_then(FadeType::of) {
            settings.fade_out = fade;
        }
    }
    settings
}
