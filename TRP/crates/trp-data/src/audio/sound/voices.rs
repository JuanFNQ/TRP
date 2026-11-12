//! The voices' phrase banks and their reader.

use super::*;

/// The folder of the voices' phrase bundles.
pub const PHRASES: &str = "assets/content/audio/phrases";

/// One clip of a voice's bank (`TaggedClip`): the tags it answers to
/// (`ETagStatus` bits; nought answers to any), its volume, its falloff in
/// metres, its length as the bank keeps it, and the sound itself.
#[derive(Debug, Clone)]
pub struct PhraseClip {
    pub name: String,
    pub mask: u32,
    pub volume: f32,
    pub falloff: f32,
    pub length: f32,
    pub clip: Option<Clip>,
}

/// One bank of a voice (`TagBank`): what it is said for (`EPhraseTrigger`),
/// how much it matters against what is being said, whether its clips'
/// tags count, its clips (the spread groups' together, as the bank keeps
/// them), and the phrase it may chain to (`ChainEvent`).
#[derive(Debug, Clone)]
pub struct PhraseBank {
    pub trigger: i32,
    pub importance: i32,
    pub ignore_tags: bool,
    pub clips: Vec<PhraseClip>,
    pub chain: PhraseChain,
}

/// What a bank's phrase may be followed by (`Chain`): who says it (0 the
/// speaker itself, 1 its group, 2 another), the phrase (`EPhraseTrigger`;
/// `PhraseNone` for none) and the chance in a hundred.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PhraseChain {
    pub who: i32,
    pub event: i32,
    pub probability: i32,
}

/// A voice's banks for the triggers asked for (`Voice.Banks`), in the
/// voice's order, and the bundle it was read from.
#[derive(Debug, Clone, Default)]
pub struct VoiceBanks {
    pub name: String,
    pub bundle: String,
    pub banks: Vec<PhraseBank>,
}

/// A voice opened in the install by its name (`CustomizationSolver.
/// GetVoice(...).Name`, `Bear_2_Eng`), its bundle kept loaded so that its
/// banks can be read as they are wanted (the gestures' phrases, each bank's
/// sounds read the first time it is said). Its phrase bundle is the name in
/// small letters and `_voice.bundle` for most (`InGameBundles.
/// TakePhrasePath`); where that bundle is not there or holds another voice,
/// every phrase bundle is looked through for the `Voice` of that name. Two
/// names the client's table sends to a bundle whose `Voice` has another
/// name (`ELSEWHERE`): that bundle's voice, whatever its name.
pub struct VoiceReader {
    pub(super) assets: crate::unity::Assets,
    pub(super) setups: VorbisSetups,
    voice: crate::unity::assets::ObjRef,
    pub name: String,
    pub bundle: String,
}

/// The voices the client's table of phrase bundles (`InGameBundles.
/// TakePhrasePath`) sends to a bundle of another name, whose `Voice` is
/// named otherwise too (`Usec_Rob`): the USEC voice shown as Rob and the
/// BEAR voice shown as Vitaly (`probe voice Usec_7`).
const ELSEWHERE: [(&str, &str); 2] = [("Usec_7", "usec_rob_voice"), ("Bear_4", "bear_vitaly_voice")];

impl VoiceReader {
    pub fn open(install: &Install, name: &str) -> Result<Self> {
        use crate::unity::{class, extract};
        if let Some((_, bundle)) = ELSEWHERE.iter().find(|(voice, _)| *voice == name) {
            let key = format!("{PHRASES}/{bundle}.bundle");
            let mut assets = crate::unity::Assets::new(install.clone())?;
            let files = assets.load(&key)?;
            let voice = files.iter().flat_map(|&file| assets.objects_of(file, class::MONO_BEHAVIOUR).collect::<Vec<_>>()).find(|&obj| extract::script_class(&assets, obj).as_deref() == Some("Voice"));
            let voice = voice.with_context(|| format!("{key} holds no voice"))?;
            return Ok(Self { assets, setups: VorbisSetups::new(install), voice, name: name.to_owned(), bundle: key });
        }
        let guess = format!("{PHRASES}/{}_voice.bundle", name.to_lowercase());
        let mut keys = vec![guess.clone()];
        if let Ok(dir) = std::fs::read_dir(install.bundle_path(PHRASES)) {
            let mut others: Vec<String> = dir.filter_map(|e| e.ok()).filter_map(|e| e.file_name().into_string().ok()).filter(|f| f.ends_with(".bundle")).map(|f| format!("{PHRASES}/{f}")).filter(|k| *k != guess).collect();
            others.sort();
            keys.extend(others);
        }
        for key in keys {
            if !install.bundle_path(&key).exists() {
                continue;
            }
            let mut assets = crate::unity::Assets::new(install.clone())?;
            let Ok(files) = assets.load(&key) else { continue };
            let voice = files.iter().flat_map(|&file| assets.objects_of(file, class::MONO_BEHAVIOUR).collect::<Vec<_>>()).find(|&obj| {
                extract::script_class(&assets, obj).as_deref() == Some("Voice") && assets.read(obj).ok().and_then(|v| v.get("Name").and_then(Value::as_str).map(|n| n == name)).unwrap_or(false)
            });
            let Some(voice) = voice else { continue };
            return Ok(Self { assets, setups: VorbisSetups::new(install), voice, name: name.to_owned(), bundle: key });
        }
        bail!("no phrase bundle holds the voice {name}")
    }

    /// The voice's banks of `triggers` (`Voice.Banks`), in the voice's
    /// order; their clips' sounds read for the triggers `sounds` takes.
    pub fn banks(&mut self, triggers: &dyn Fn(i32) -> bool, sounds: &dyn Fn(i32) -> bool) -> Result<Vec<PhraseBank>> {
        let (assets, setups, name) = (&self.assets, &mut self.setups, &self.name);
        let value = assets.read(self.voice)?;
        let mut out = Vec::new();
        for pptr in value.get("Banks").and_then(Value::as_array).unwrap_or(&[]).iter().filter_map(Value::as_pptr) {
            let Some(at) = assets.resolve_loaded(self.voice.file, pptr) else { continue };
            let bank = assets.read(at)?;
            let trigger = bank.get("Trigger").and_then(Value::as_i64).unwrap_or(-1) as i32;
            if !triggers(trigger) {
                continue;
            }
            let samples = sounds(trigger);
            let mut clips = Vec::new();
            for tagged in bank.get("Clips").and_then(Value::as_array).unwrap_or(&[]) {
                let f = |k: &str| tagged.get(k).and_then(Value::as_f32).unwrap_or(0.0);
                let found = tagged.get("Clip").and_then(Value::as_pptr).and_then(|p| assets.resolve_loaded(at.file, p));
                let (clip_name, clip) = match found {
                    Some(obj) => {
                        let read = assets.read(obj)?;
                        let clip_name = read.get("m_Name").and_then(Value::as_str).unwrap_or_default().to_owned();
                        let clip = if samples {
                            let resource = |source: &str, offset: usize, size: usize| -> Result<Vec<u8>> { Ok(assets.resource(source, offset, size)?.to_vec()) };
                            match clip_of(&read, &resource, setups) {
                                Ok(clip) => Some(clip),
                                Err(e) => {
                                    eprintln!("the voice {name}'s {clip_name} could not be read; it is silent: {e:#}");
                                    None
                                }
                            }
                        } else {
                            None
                        };
                        (clip_name, clip)
                    }
                    None => (String::new(), None),
                };
                clips.push(PhraseClip {
                    name: clip_name,
                    mask: tagged.get("Mask").and_then(Value::as_i64).unwrap_or(0) as u32,
                    volume: f("Volume"),
                    falloff: f("Falloff"),
                    length: f("Length"),
                    clip,
                });
            }
            out.push(PhraseBank {
                trigger,
                importance: bank.get("Importance").and_then(Value::as_i64).unwrap_or(0) as i32,
                ignore_tags: bank.get("IgnoreTags").and_then(|v| v.as_bool().or_else(|| v.as_i64().map(|i| i != 0))).unwrap_or(false),
                clips,
                chain: bank.get("ChainEvent").map_or(PhraseChain::default(), |chain| {
                    let int = |name: &str| chain.get(name).and_then(Value::as_i64).unwrap_or(0) as i32;
                    PhraseChain { who: int("Who"), event: int("Event"), probability: int("Probability") }
                }),
            });
        }
        Ok(out)
    }
}

/// A voice's banks for the triggers asked for, their clips' sounds with
/// `samples` (`VoiceReader`, opened and read once).
pub fn voice_banks(install: &Install, name: &str, triggers: &[i32], samples: bool) -> Result<VoiceBanks> {
    let mut reader = VoiceReader::open(install, name)?;
    let banks = reader.banks(&|trigger| triggers.contains(&trigger), &|_| samples)?;
    Ok(VoiceBanks { name: reader.name, bundle: reader.bundle, banks })
}
