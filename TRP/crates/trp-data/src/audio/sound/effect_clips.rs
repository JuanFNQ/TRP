//! The bipod's, tinnitus, fracture and effects' sounds.

use super::*;

/// One of a part's sets of clips (`AudioMultipleClipContainer`): the clips
/// one is drawn from, the volume and the farthest it is heard
/// (`_maxDistance`).
#[derive(Debug, Clone, Default)]
pub struct ClipSet {
    pub clips: Vec<Clip>,
    pub volume: f32,
    pub max_distance: f32,
}

/// A bipod's own sounds (`BipodAudioController`, on its prefab; mounting
/// spec, "Sounds"): its opening and folding (the hands' `BipodOpen` and
/// `BipodClose` marks) and, by the surface's name (`ESurfaceSound`, as
/// `SURFACES`), what it sounds on the ground at a forward mount.
#[derive(Debug, Clone, Default)]
pub struct BipodSounds {
    pub open: ClipSet,
    pub close: ClipSet,
    pub surfaces: Vec<(String, ClipSet)>,
}

/// The bipod's sounds, from its prefab's bundle as the build loaded it
/// (`BipodPart::bundle`); none where it has no `BipodAudioController`.
pub fn bipod_sounds(assets: &crate::unity::Assets, bundle: &str, setups: &mut VorbisSetups) -> Result<Option<BipodSounds>> {
    use crate::unity::{class, extract};
    let files: Vec<usize> = assets.files().iter().enumerate().filter(|(_, f)| f.bundle_key == bundle).map(|(i, _)| i).collect();
    let Some(behaviour) = files
        .iter()
        .flat_map(|&file| assets.objects_of(file, class::MONO_BEHAVIOUR).collect::<Vec<_>>())
        .find(|&obj| extract::script_class(assets, obj).as_deref() == Some("BipodAudioController"))
    else {
        return Ok(None);
    };
    let fields = assets.read(behaviour)?;
    let mut read_clips: HashMap<crate::unity::ObjRef, Clip> = HashMap::new();
    let mut set = |value: Option<&Value>| -> Result<ClipSet> {
        let Some(value) = value else { return Ok(ClipSet::default()) };
        let mut clips = Vec::new();
        for clip in value.get("_clips").and_then(Value::as_array).unwrap_or(&[]) {
            let Some(at) = clip.as_pptr().and_then(|p| assets.resolve_loaded(behaviour.file, p)) else { continue };
            if !read_clips.contains_key(&at) {
                let value = assets.read(at)?;
                let bank = |source: &str, offset: usize, size: usize| -> Result<Vec<u8>> { Ok(assets.resource(source, offset, size)?.to_vec()) };
                match clip_of(&value, &bank, setups) {
                    Ok(read) => {
                        read_clips.insert(at, read);
                    }
                    Err(e) => eprintln!("a bipod's sound could not be read: {e:#}"),
                }
            }
            if let Some(read) = read_clips.get(&at) {
                clips.push(read.clone());
            }
        }
        Ok(ClipSet {
            clips,
            volume: value.get("_volume").and_then(Value::as_f32).unwrap_or(1.0),
            max_distance: value.get("_maxDistance").and_then(Value::as_f32).unwrap_or(0.0),
        })
    };
    let open = set(fields.get("_bipodOpenSounds"))?;
    let close = set(fields.get("_bipodCloseSounds"))?;
    let table = fields.get("_bipodMountSurfaceSounds");
    let keys = table.and_then(|t| t.get("_keys")).and_then(Value::as_array).unwrap_or(&[]);
    let values = table.and_then(|t| t.get("_values")).and_then(Value::as_array).unwrap_or(&[]);
    let mut surfaces = Vec::new();
    for (key, value) in keys.iter().zip(values) {
        // The keys are written as the enum's names, or its numbers.
        let name = key.as_str().map(str::to_owned).or_else(|| key.as_i64().and_then(|n| SURFACES.get(n as usize)).map(|s| (*s).to_owned()));
        if let Some(name) = name {
            surfaces.push((name, set(Some(value))?));
        }
    }
    Ok(Some(BipodSounds { open, close, surfaces }))
}

/// The ringing of a contusion (`Sounds.TinnitusSound`, in `MOVEMENT_SOUNDS`:
/// `Player.TryStartContusion`'s clip), read whole.
pub fn tinnitus_sound(install: &Install) -> Result<Clip> {
    movement_clip(install, "TinnitusSound")
}

/// A clip the movement's `Sounds` names (`TinnitusSound`, `FractureSound`).
fn movement_clip(install: &Install, field: &str) -> Result<Clip> {
    use crate::unity::{class, extract};
    let mut setups = VorbisSetups::new(install);
    let mut assets = crate::unity::Assets::new(install.clone())?;
    let files = assets.load(MOVEMENT_SOUNDS)?;
    let sounds = files
        .iter()
        .flat_map(|&file| assets.objects_of(file, class::MONO_BEHAVIOUR).collect::<Vec<_>>())
        .find(|&obj| extract::script_class(&assets, obj).as_deref() == Some("Sounds"))
        .context("no Sounds in the movement's bundle")?;
    let value = assets.read(sounds)?;
    let at = value.get(field).and_then(Value::as_pptr).and_then(|p| assets.resolve_loaded(sounds.file, p)).with_context(|| format!("the Sounds name no {field}"))?;
    let clip = assets.read(at)?;
    let resource = |source: &str, offset: usize, size: usize| -> Result<Vec<u8>> { Ok(assets.resource(source, offset, size)?.to_vec()) };
    clip_of(&clip, &resource, &mut setups)
}

/// A bone's breaking (`Sounds.FractureSound`, `bone_break`), which a
/// fracture plays as it starts (`Player.OnHealthEffectAdded`).
pub fn fracture_sound(install: &Install) -> Result<Clip> {
    movement_clip(install, "FractureSound")
}

/// The bundle of the world's effects (`Effects`: each named effect's
/// particles, decals and sounds, `EffectsArray`).
pub const EFFECTS: &str = "assets/systems/effects/particlesystems/effects.bundle";

/// Effects' sounds (`Effects.Effect.Sound`, the bank an effect plays where
/// it is not seen in first person): each effect asked for by its name with
/// its bank's place in `banks` (none where it has no sound or is not
/// there), and the banks' clips.
#[derive(Debug, Clone, Default)]
pub struct EffectSounds {
    pub banks: Vec<Bank>,
    pub effects: Vec<(String, Option<usize>)>,
    pub clips: Vec<(String, Option<Clip>)>,
    /// The effects' other banks (`AdditionalSoundEffects`, in order: the
    /// first a ricochet's), each by its place in `banks`.
    pub additional: Vec<Option<usize>>,
}

impl EffectSounds {
    /// An effect's bank by its name.
    pub fn bank(&self, name: &str) -> Option<&Bank> {
        self.effects.iter().find(|(effect, _)| effect == name).and_then(|(_, at)| *at).map(|at| &self.banks[at])
    }
}

/// The sounds of the effects named (`EFFECTS`), with their clips' samples
/// if `samples`.
pub fn effect_sounds(install: &Install, names: &[String], samples: bool) -> Result<EffectSounds> {
    use crate::unity::{class, extract};
    let mut setups = VorbisSetups::new(install);
    let mut assets = crate::unity::Assets::new(install.clone())?;
    let files = assets.load(EFFECTS)?;
    let effects = files
        .iter()
        .flat_map(|&file| assets.objects_of(file, class::MONO_BEHAVIOUR).collect::<Vec<_>>())
        .find(|&obj| extract::script_class(&assets, obj).as_deref() == Some("Effects"))
        .context("no Effects in its bundle")?;
    let value = assets.read(effects)?;
    let all = value.get("EffectsArray").and_then(Value::as_array).context("the effects have no EffectsArray")?;
    let mut out = EffectSounds::default();
    let mut reader = BankReader::new(samples);
    for name in names {
        let entry = all.iter().find(|effect| effect.get("Name").and_then(Value::as_str) == Some(name.as_str()));
        let bank = match entry {
            Some(effect) => reader.bank_at(&assets, &mut setups, effects.file, effect.get("Sound"), &mut out.banks, &mut out.clips)?,
            None => None,
        };
        out.effects.push((name.clone(), bank));
    }
    for extra in value.get("AdditionalSoundEffects").and_then(Value::as_array).unwrap_or(&[]) {
        let bank = reader.bank_at(&assets, &mut setups, effects.file, Some(extra), &mut out.banks, &mut out.clips)?;
        out.additional.push(bank);
    }
    if !reader.unread.is_empty() {
        eprintln!("{} of the effects' sounds are not read and are silent: {}", reader.unread.len(), reader.unread.join("; "));
    }
    Ok(out)
}
