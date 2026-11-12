//! The movement sounds: surfaces, gear, vaults, toggles and mounts.

use super::*;

/// The bundle of the sounds of moving (`Sounds`, read by the game's
/// `Player.InitAudioController`): for each surface a set of banks
/// (`SurfaceSet`), and the equipment's banks.
pub const MOVEMENT_SOUNDS: &str = "assets/content/audio/prefabs/movement/sounds.bundle";

/// The surfaces a step can be on (`BaseBallistic.ESurfaceSound`), by number.
pub const SURFACES: [&str; 20] = [
    "Concrete", "Metal", "Wood", "Soil", "Grass", "Asphalt", "Glass", "Gravel", "MetalThin", "Puddle", "Slate", "Tile", "WoodThick", "Swamp", "Garbage", "GarbageMetal", "Plastic", "WholeGlass", "WoodThin", "Snow",
];

/// A bank of the game's (`SoundBank`): its figures, and the clips of the
/// one environment the steps are played in (`EnvironmentType.Outdoor`, the
/// first of `Environments`: the player passes it on every call), in its
/// three lists (`DistanceVarity`), each clip by its place in
/// `MovementSounds::clips`.
#[derive(Debug, Clone)]
pub struct Bank {
    pub name: String,
    pub base_volume: f32,
    pub base_pitch: f32,
    pub delta_volume: f32,
    pub delta_pitch: f32,
    pub rolloff: f32,
    /// Its `DistanceBlendOptions.Values`, if it has them: where the blend
    /// value moves from one list to the next.
    pub blend: Option<Vec<f32>>,
    pub lists: [Vec<usize>; 3],
    /// The kind of source it plays from (`SourceType`, BetterAudio's
    /// `AudioSourceGroupType`: 4, `Environment`, if the bank does not say).
    pub source_type: i64,
    /// Whether an effect plays it from afar (`Physical`:
    /// `PlayAtPointDistant`), or limited near others of its kind
    /// (`AllowLimitedPlay`).
    pub physical: bool,
    pub limited: bool,
    /// Its clips' length as the bank keeps it (`ClipLength`: `CustomLength`
    /// where it is not nought, else `_clipLiength`), in seconds.
    pub clip_length: f32,
}

/// The banks of a surface (`SurfaceSet`), in its order.
pub const SURFACE_BANKS: [&str; 9] = ["RunSoundBank", "SprintSoundBank", "StopSoundBank", "LandingSoundBank", "TurnSoundBank", "DuckSoundBank", "ProneSoundBank", "ProneDropSoundBank", "JumpSoundBank"];

/// The equipment's banks (`Sounds`), in that order.
pub const GEAR_BANKS: [&str; 4] = ["Gear", "GearMedium", "GearFast", "BackpackDrop"];

/// A surface's sound sets for the three moves over an obstacle
/// (`SurfaceSet`; footsteps spec, "The vault's sounds"), in the order of
/// the game's `EVaultingSoundType` less its none: a vault, a sprint's
/// vault, a climb.
pub const VAULT_SETS: [&str; 3] = ["VaultSoundSet", "SprintVaultSoundSet", "ClimbSoundSet"];

/// The kit's bank for each of the three moves (`Sounds`), in the same order.
pub const VAULT_GEAR_BANKS: [&str; 3] = ["GearVault", "GearSprintVault", "GearClimb"];

/// The marks of a move at which a set's elements play
/// (`EVaultingSoundState`, by number; its `Gear` is never looked up).
pub const VAULT_MARKS: [&str; 4] = ["Start", "Grab", "Slide", "End"];

/// One mark's sound of a move on a surface (`VaultingSoundElement`): its
/// bank, if it names one (a place in `MovementSounds::banks`), and the
/// range the obstacle's height places its volume in (`VolumeRange`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VaultElement {
    pub bank: Option<usize>,
    pub min_volume: f32,
    pub max_volume: f32,
}

/// A surface's vault sounds: by move (`VAULT_SETS`' order) and mark
/// (`VAULT_MARKS`' order); a mark a set does not list is `None`.
#[derive(Debug, Clone)]
pub struct VaultSounds {
    pub surface: u8,
    pub elements: [[Option<VaultElement>; 4]; 3],
}

/// A surface's banks, by `SURFACE_BANKS`' order, each by its place in
/// `MovementSounds::banks` (a bank missing is `None`; one bank may serve
/// two ways or two surfaces, as the swamp's run serves its crouch too).
#[derive(Debug, Clone)]
pub struct SurfaceSounds {
    pub surface: u8,
    pub banks: Vec<Option<usize>>,
}

/// The sounds of moving as the install keeps them.
#[derive(Debug, Clone, Default)]
pub struct MovementSounds {
    /// Every bank once.
    pub banks: Vec<Bank>,
    pub surfaces: Vec<SurfaceSounds>,
    /// By `GEAR_BANKS`' order, as places in `banks`.
    pub gear: Vec<Option<usize>>,
    /// Each surface's sounds for the moves over an obstacle.
    pub vault: Vec<VaultSounds>,
    /// The kit's banks for those moves, by `VAULT_GEAR_BANKS`' order.
    pub vault_gear: Vec<Option<usize>>,
    /// Every clip a bank names, read or not (one kept in a way not read, or
    /// left unread when only the figures were asked for, is `None`), with
    /// its name.
    pub clips: Vec<(String, Option<Clip>)>,
    /// The switches on the head's clips, by `TOGGLE_SOUNDS`' order (its name
    /// in the `Sounds`, the clip's own name, the clip if read).
    pub toggles: Vec<(&'static str, String, Option<Clip>)>,
}

/// The `Sounds`' clips for a switch on the head turned on and off
/// (`Player.PlayFaceShieldSound` and the rest; inventory spec, "A switch on
/// the head"), and the player's tactical sound (`SwitchHeadlights`: Hold
/// mode's toggles and the head light; tactical-devices spec, "The
/// animation and the sound"), the last.
pub const TOGGLE_SOUNDS: [&str; 7] = ["FaceShieldOn", "FaceShieldOff", "NightVisionOn", "NightVisionOff", "ThermalVisionOn", "ThermalVisionOff", "SwitchHeadlights"];

/// The player's tactical sound's place in `TOGGLE_SOUNDS`.
pub const TACTICAL_SOUND: usize = 6;

impl MovementSounds {
    /// A surface's set, by its number.
    pub fn surface(&self, surface: u8) -> Option<&SurfaceSounds> {
        self.surfaces.iter().find(|set| set.surface == surface)
    }

    /// A surface's bank of the way given (`SURFACE_BANKS`' place).
    pub fn bank(&self, surface: u8, way: usize) -> Option<&Bank> {
        self.surface(surface)?.banks.get(way).copied().flatten().map(|at| &self.banks[at])
    }

    /// A surface's element for a move (`VAULT_SETS`' place) at a mark
    /// (`VAULT_MARKS`' place), if its set lists one.
    pub fn vault_element(&self, surface: u8, kind: usize, mark: usize) -> Option<VaultElement> {
        self.vault.iter().find(|set| set.surface == surface)?.elements.get(kind)?.get(mark).copied().flatten()
    }
}

/// The sounds of moving (`MOVEMENT_SOUNDS`' `Sounds`): every surface's
/// banks and the equipment's, with their clips' samples if `samples`
/// (else only their names), only of the surfaces listed if any are.
pub fn movement_sounds(install: &Install, only: Option<&[u8]>, samples: bool) -> Result<MovementSounds> {
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
    let mut out = MovementSounds::default();
    let mut reader = BankReader::new(samples);
    for set in value.get("Sets").and_then(Value::as_array).unwrap_or(&[]) {
        let surface = set.get("Surface").and_then(Value::as_i64).unwrap_or(0) as u8;
        if only.is_some_and(|only| !only.contains(&surface)) {
            continue;
        }
        let mut banks = Vec::new();
        for name in SURFACE_BANKS {
            banks.push(reader.bank_at(&assets, &mut setups, sounds.file, set.get(name), &mut out.banks, &mut out.clips)?);
        }
        out.surfaces.push(SurfaceSounds { surface, banks });
        // The three moves' sets: each a list of elements, a mark's number
        // each, with its bank and its volume's range.
        let mut elements: [[Option<VaultElement>; 4]; 3] = Default::default();
        for (kind, name) in VAULT_SETS.iter().enumerate() {
            for element in set.get(name).and_then(|set| set.get("VaultingBanks")).and_then(Value::as_array).unwrap_or(&[]) {
                let mark = element.get("SoundState").and_then(Value::as_i64).unwrap_or(-1);
                let Some(slot) = usize::try_from(mark).ok().and_then(|mark| elements[kind].get_mut(mark)) else { continue };
                let bank = reader.bank_at(&assets, &mut setups, sounds.file, element.get("SoundBank"), &mut out.banks, &mut out.clips)?;
                let range = |name: &str, or: f32| element.get("VolumeRange").and_then(|range| range.get(name)).and_then(Value::as_f32).unwrap_or(or);
                *slot = Some(VaultElement { bank, min_volume: range("MinVolume", 0.1), max_volume: range("MaxVolume", 0.7) });
            }
        }
        out.vault.push(VaultSounds { surface, elements });
    }
    for name in GEAR_BANKS {
        let bank = reader.bank_at(&assets, &mut setups, sounds.file, value.get(name), &mut out.banks, &mut out.clips)?;
        out.gear.push(bank);
    }
    for name in VAULT_GEAR_BANKS {
        let bank = reader.bank_at(&assets, &mut setups, sounds.file, value.get(name), &mut out.banks, &mut out.clips)?;
        out.vault_gear.push(bank);
    }
    let mut unread = std::mem::take(&mut reader.unread);
    for name in TOGGLE_SOUNDS {
        let Some(at) = value.get(name).and_then(Value::as_pptr).and_then(|p| assets.resolve_loaded(sounds.file, p)) else { continue };
        let clip = assets.read(at)?;
        let clip_name = clip.get("m_Name").and_then(Value::as_str).unwrap_or_default().to_owned();
        let read = if samples {
            let resource = |source: &str, offset: usize, size: usize| -> Result<Vec<u8>> { Ok(assets.resource(source, offset, size)?.to_vec()) };
            match clip_of(&clip, &resource, &mut setups) {
                Ok(clip) => Some(clip),
                Err(e) => {
                    unread.push(format!("{e:#}"));
                    None
                }
            }
        } else {
            None
        };
        out.toggles.push((name, clip_name, read));
    }
    if !unread.is_empty() {
        eprintln!("{} of the sounds of moving are not read and are silent: {}", unread.len(), unread.join("; "));
    }
    Ok(out)
}

/// Banks read from loaded files (`SoundBank`), each once, and their clips
/// each once, with their samples if asked for: what could not be read kept
/// to be told.
pub(super) struct BankReader {
    pub(super) samples: bool,
    pub(super) clip_places: HashMap<crate::unity::ObjRef, usize>,
    bank_places: HashMap<crate::unity::ObjRef, usize>,
    pub(super) unread: Vec<String>,
}

impl BankReader {
    pub(super) fn new(samples: bool) -> Self {
        Self { samples, clip_places: HashMap::new(), bank_places: HashMap::new(), unread: Vec::new() }
    }

    /// A bank by its pointer from `from`'s file, put in `banks` with its
    /// clips (in `clips`) if it is not there yet: its place there. Only its
    /// first environment's clips (`EnvironmentType.Outdoor`).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn bank_at(&mut self, assets: &crate::unity::Assets, setups: &mut VorbisSetups, from: usize, pointer: Option<&Value>, banks: &mut Vec<Bank>, clips: &mut Vec<(String, Option<Clip>)>) -> Result<Option<usize>> {
        let Some(at) = pointer.and_then(Value::as_pptr).and_then(|p| assets.resolve_loaded(from, p)) else { return Ok(None) };
        if let Some(&place) = self.bank_places.get(&at) {
            return Ok(Some(place));
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
                let place = match self.clip_places.get(&clip) {
                    Some(&place) => place,
                    None => {
                        let value = assets.read(clip)?;
                        let name = value.get("m_Name").and_then(Value::as_str).unwrap_or_default().to_owned();
                        let read = if self.samples {
                            let resource = |source: &str, offset: usize, size: usize| -> Result<Vec<u8>> { Ok(assets.resource(source, offset, size)?.to_vec()) };
                            match clip_of(&value, &resource, setups) {
                                Ok(clip) => Some(clip),
                                Err(e) => {
                                    self.unread.push(format!("{e:#}"));
                                    None
                                }
                            }
                        } else {
                            None
                        };
                        clips.push((name, read));
                        self.clip_places.insert(clip, clips.len() - 1);
                        clips.len() - 1
                    }
                };
                list.push(place);
            }
        }
        banks.push(Bank {
            name: bank.get("m_Name").and_then(Value::as_str).unwrap_or_default().to_owned(),
            base_volume: number("BaseVolume", 1.0),
            base_pitch: number("BasePitch", 1.0),
            delta_volume: number("DeltaVolume", 0.0),
            delta_pitch: number("DeltaPitch", 0.0),
            rolloff: number("Rolloff", 100.0),
            blend,
            lists,
            source_type: bank.get("SourceType").and_then(Value::as_i64).unwrap_or(4),
            physical: bank.get("Physical").and_then(Value::as_bool).unwrap_or(false),
            limited: bank.get("AllowLimitedPlay").and_then(Value::as_bool).unwrap_or(false),
            clip_length: match number("CustomLength", 0.0) {
                custom if custom != 0.0 => custom,
                _ => number("_clipLiength", 0.0),
            },
        });
        self.bank_places.insert(at, banks.len() - 1);
        Ok(Some(banks.len() - 1))
    }
}

/// The weapon's mount sound (`WeaponSounds.VerticalMountSounds`, a `SoundBank`
/// in `WEAPON_SOUNDS`: `WeaponSoundPlayer.PlayMountSound`, the approach's time
/// after any mount; mounting spec, "Sounds"): the bank and its clips, read
/// whole.
pub fn mount_sounds(install: &Install) -> Result<(Bank, Vec<(String, Clip)>)> {
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
    let mut reader = BankReader::new(true);
    let (mut banks, mut clips) = (Vec::new(), Vec::new());
    let at = reader.bank_at(&assets, &mut setups, sounds.file, value.get("VerticalMountSounds"), &mut banks, &mut clips)?.context("no VerticalMountSounds")?;
    let bank = banks.swap_remove(at);
    let read = bank.lists[0].iter().filter_map(|&place| clips.get(place).and_then(|(name, clip)| Some((name.clone(), clip.clone()?)))).collect();
    Ok((bank, read))
}
