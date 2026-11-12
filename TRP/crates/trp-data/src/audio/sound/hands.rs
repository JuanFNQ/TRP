//! The hands' sound players and the shot banks.

use super::*;

/// A sound a hands rig plays for a mark of its animator
/// (`BaseSoundPlayer.SoundElement`): the mark's name, how far off it is
/// heard (its rolloff, in metres), its volume, and its clips, one of which
/// plays at random each time.
#[derive(Debug, Clone)]
pub struct SoundElement {
    pub event: String,
    pub rolloff: i32,
    pub volume: f32,
    pub clips: Vec<Clip>,
}

/// A hands rig's sound player (`BaseSoundPlayer` or a kind of it, on a
/// weapon's or an item's own prefab): its script's class and its two lists,
/// `MainSounds` (by the name of an event of the weapon's own) and
/// `AdditionalSounds` (by a mark's string: the `Sound` and `SoundAtPoint`
/// marks).
#[derive(Debug, Clone, Default)]
pub struct HandsSounds {
    pub player: String,
    pub main: Vec<SoundElement>,
    pub additional: Vec<SoundElement>,
    /// A weapon's shots, where its player is a `WeaponSoundPlayer` with
    /// banks of them.
    pub shots: Option<ShotSounds>,
}

/// A weapon's sound player's banks of shots (`WeaponSoundPlayer`), in this
/// order: `Body` (a shot, or for an automatic weapon its fire as a loop),
/// `Tail` (what an automatic weapon's fire ends with), `Doublet` (two
/// chambers fired at once), and the three with a silencer.
pub const SHOT_BANKS: [&str; 6] = ["Body", "Tail", "Doublet", "BodySilenced", "TailSilenced", "DoubletSilenced"];

/// A bank of a weapon's shots (`SoundBank`): its figures, and the clips of
/// the outdoor environment (`EnvironmentType.Outdoor`, the first of its
/// `Environments`; the app's range has no indoors) in its three lists by
/// distance (`DistanceVarity`), each clip by its place in
/// `ShotSounds::clips`.
#[derive(Debug, Clone)]
pub struct ShotBank {
    pub name: String,
    pub base_volume: f32,
    pub rolloff: f32,
    /// `ClipLength`: its `CustomLength` if that is not 0, else the length
    /// kept with it (`_clipLiength`). An automatic weapon's loop is 16 beats
    /// of it.
    pub clip_length: f32,
    /// Its `DistanceBlendOptions.Values`, if it has them.
    pub blend: Option<Vec<f32>>,
    pub lists: [Vec<usize>; 3],
}

/// A weapon's shots as its sound player keeps them.
#[derive(Debug, Clone, Default)]
pub struct ShotSounds {
    /// `Non_auto`: each shot plays whole, not as a loop.
    pub non_auto: bool,
    /// By `SHOT_BANKS`' order; a bank the player has not is `None`.
    pub banks: Vec<Option<ShotBank>>,
    /// Every clip the banks name, read once (one not read is `None`), with
    /// its name.
    pub clips: Vec<(String, Option<Clip>)>,
}

impl ShotSounds {
    /// A bank by its name in `SHOT_BANKS`.
    pub fn bank(&self, name: &str) -> Option<&ShotBank> {
        SHOT_BANKS.iter().position(|n| *n == name).and_then(|at| self.banks.get(at)).and_then(Option::as_ref)
    }
}

/// A sound player's banks of shots, with their clips (each read once).
fn shot_sounds(assets: &crate::unity::Assets, behaviour: crate::unity::ObjRef, fields: &Value, setups: &mut VorbisSetups) -> Result<Option<ShotSounds>> {
    use crate::unity::ObjRef;
    let mut out = ShotSounds { non_auto: fields.get("Non_auto").and_then(Value::as_bool).unwrap_or(false), ..Default::default() };
    let mut clip_places: HashMap<ObjRef, usize> = HashMap::new();
    let mut bank_read: HashMap<ObjRef, ShotBank> = HashMap::new();
    let mut unread = Vec::new();
    for name in SHOT_BANKS {
        let Some(at) = fields.get(name).and_then(Value::as_pptr).and_then(|p| assets.resolve_loaded(behaviour.file, p)) else {
            out.banks.push(None);
            continue;
        };
        if let Some(bank) = bank_read.get(&at) {
            out.banks.push(Some(bank.clone()));
            continue;
        }
        let bank = assets.read(at)?;
        let number = |name: &str, or: f32| bank.get(name).and_then(Value::as_f32).unwrap_or(or);
        let blend = bank
            .get("BlendOptions")
            .and_then(Value::as_pptr)
            .and_then(|p| assets.resolve_loaded(at.file, p))
            .and_then(|options| assets.read(options).ok())
            .and_then(|options| options.get("Values").and_then(Value::as_floats).map(<[f32]>::to_vec));
        let mut lists: [Vec<usize>; 3] = Default::default();
        let first = bank.get("Environments").and_then(Value::as_array).and_then(|all| all.first());
        let varieties = first.and_then(|e| e.get("Clips")).and_then(Value::as_array).unwrap_or(&[]);
        for (list, variety) in lists.iter_mut().zip(varieties) {
            for pointer in variety.get("Clips").and_then(Value::as_array).unwrap_or(&[]) {
                let Some(clip) = pointer.as_pptr().and_then(|p| assets.resolve_loaded(at.file, p)) else { continue };
                let place = match clip_places.get(&clip) {
                    Some(&place) => place,
                    None => {
                        let value = assets.read(clip)?;
                        let name = value.get("m_Name").and_then(Value::as_str).unwrap_or_default().to_owned();
                        let resource = |source: &str, offset: usize, size: usize| -> Result<Vec<u8>> { Ok(assets.resource(source, offset, size)?.to_vec()) };
                        let read = match clip_of(&value, &resource, setups) {
                            Ok(clip) => Some(clip),
                            Err(e) => {
                                unread.push(format!("{e:#}"));
                                None
                            }
                        };
                        out.clips.push((name, read));
                        clip_places.insert(clip, out.clips.len() - 1);
                        out.clips.len() - 1
                    }
                };
                list.push(place);
            }
        }
        let custom = number("CustomLength", 0.0);
        let read = ShotBank {
            name: bank.get("m_Name").and_then(Value::as_str).unwrap_or_default().to_owned(),
            base_volume: number("BaseVolume", 1.0),
            rolloff: number("Rolloff", 100.0),
            clip_length: if custom != 0.0 { custom } else { number("_clipLiength", 0.0) },
            blend,
            lists,
        };
        bank_read.insert(at, read.clone());
        out.banks.push(Some(read));
    }
    if !unread.is_empty() {
        eprintln!("{} of the shots' sounds are not read and are silent: {}", unread.len(), unread.join("; "));
    }
    Ok(out.banks.iter().any(Option::is_some).then_some(out))
}

impl HandsSounds {
    /// The sound for a mark's string (`SoundEventHandler`): the element of
    /// `AdditionalSounds` named so, or so with `Snd` before it.
    pub fn for_mark(&self, name: &str) -> Option<&SoundElement> {
        self.additional.iter().find(|element| element.event == name || element.event.strip_prefix("Snd") == Some(name))
    }
}

/// The script on an object whose class's name ends in `SoundPlayer`, with
/// its class.
fn sound_player_on(assets: &crate::unity::Assets, object: crate::unity::ObjRef) -> Option<(String, crate::unity::ObjRef)> {
    let value = assets.read(object).ok()?;
    let components: Vec<crate::unity::ObjRef> =
        value.get("m_Component").and_then(Value::as_array).unwrap_or(&[]).iter().filter_map(|c| c.get("component")?.as_pptr()).filter_map(|p| assets.resolve_loaded(object.file, p)).collect();
    components.into_iter().find_map(|component| crate::unity::extract::script_class(assets, component).filter(|class| class.ends_with("SoundPlayer")).map(|class| (class, component)))
}

/// A rig's sound player: on the prefab's own object (beside its
/// `WeaponPrefab` or `UsableHandsPrefab`, above the animated rig), else on
/// one of the rig's objects; its lists read with their clips (each clip
/// read once), and its banks of shots if `shots` and it has them.
pub fn hands_sounds(assets: &crate::unity::Assets, weapon: &crate::weapons::weapon::Weapon, setups: &mut VorbisSetups, shots: bool) -> Result<Option<HandsSounds>> {
    let root = weapon.prefab.get("m_GameObject").and_then(Value::as_pptr).and_then(|p| assets.resolve_loaded(weapon.prefab_file, p));
    let nodes = weapon.hierarchy.nodes.iter().filter(|node| node.scripts.iter().any(|class| class.ends_with("SoundPlayer"))).filter_map(|node| {
        let transform = assets.read(node.transform).ok()?;
        transform.get("m_GameObject").and_then(Value::as_pptr).and_then(|p| assets.resolve_loaded(node.transform.file, p))
    });
    let found = root.into_iter().chain(nodes).find_map(|object| sound_player_on(assets, object));
    let Some((player, behaviour)) = found else { return Ok(None) };
    read_sound_player(assets, player, behaviour, setups, shots).map(Some)
}

/// An underbarrel launcher's own sound player (`UnderbarrelContainer.Init`:
/// its `WeaponPrefab`'s `WeaponSoundPlayer`, on its container, which is not
/// hung on the rifle; underbarrel-launcher spec, "Two animators"): the first
/// of its bundle's behaviours that is a sound player, read as a rig's.
pub fn bundle_sounds(assets: &crate::unity::Assets, bundle: &str, setups: &mut VorbisSetups, shots: bool) -> Result<Option<HandsSounds>> {
    use crate::unity::{class, extract};
    let files: Vec<usize> = assets.files().iter().enumerate().filter(|(_, f)| f.bundle_key == bundle).map(|(i, _)| i).collect();
    let found = files
        .iter()
        .flat_map(|&file| assets.objects_of(file, class::MONO_BEHAVIOUR).collect::<Vec<_>>())
        .find_map(|obj| extract::script_class(assets, obj).filter(|class| class.ends_with("SoundPlayer")).map(|class| (class, obj)));
    let Some((player, behaviour)) = found else { return Ok(None) };
    read_sound_player(assets, player, behaviour, setups, shots).map(Some)
}

/// A sound player's lists read with their clips (each clip read once), and
/// its banks of shots if `shots` and it has them.
fn read_sound_player(assets: &crate::unity::Assets, player: String, behaviour: crate::unity::ObjRef, setups: &mut VorbisSetups, shots: bool) -> Result<HandsSounds> {
    {
        let fields = assets.read(behaviour)?;
        let mut read_clips: HashMap<crate::unity::ObjRef, Clip> = HashMap::new();
        let mut list = |name: &str| -> Result<Vec<SoundElement>> {
            let mut out = Vec::new();
            for element in fields.get(name).and_then(Value::as_array).unwrap_or(&[]) {
                let mut clips = Vec::new();
                for clip in element.get("SoundClips").and_then(Value::as_array).unwrap_or(&[]) {
                    let Some(at) = clip.as_pptr().and_then(|p| assets.resolve_loaded(behaviour.file, p)) else { continue };
                    if !read_clips.contains_key(&at) {
                        let value = assets.read(at)?;
                        let bank = |source: &str, offset: usize, size: usize| -> Result<Vec<u8>> { Ok(assets.resource(source, offset, size)?.to_vec()) };
                        match clip_of(&value, &bank, setups) {
                            Ok(read) => {
                                read_clips.insert(at, read);
                            }
                            Err(e) => eprintln!("a hands sound could not be read: {e:#}"),
                        }
                    }
                    if let Some(read) = read_clips.get(&at) {
                        clips.push(read.clone());
                    }
                }
                out.push(SoundElement {
                    event: element.get("EventName").and_then(Value::as_str).unwrap_or_default().to_owned(),
                    rolloff: element.get("RollOff").and_then(Value::as_i64).unwrap_or(30) as i32,
                    volume: element.get("Volume").and_then(Value::as_f32).unwrap_or(1.0),
                    clips,
                });
            }
            Ok(out)
        };
        let main = list("MainSounds")?;
        let additional = list("AdditionalSounds")?;
        let shots = if shots && player == "WeaponSoundPlayer" { shot_sounds(assets, behaviour, &fields, setups)? } else { None };
        Ok(HandsSounds { player, main, additional, shots })
    }
}
