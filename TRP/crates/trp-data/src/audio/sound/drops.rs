//! The sounds of items dropped and of spent shells.

use super::*;

/// The bundle of the weapons' sounds as they land (`ItemDropSounds`, which
/// the game's `BetterAudio` loads as it begins).
pub const ITEM_DROP_SOUNDS: &str = "assets/content/audio/prefabs/items/itemdropsounds.bundle";

/// Each surface's banks for the kinds of weapon that sound as they land
/// (`ItemDropSurfaceSet`), in the order of the game's `EItemDropSoundType`
/// less its none: a pistol, a submachine gun, a rifle.
pub const DROP_KINDS: [&str; 3] = ["PistolDropSoundBank", "SubGunDropSoundBank", "RiffleDropSoundBank"];

/// A template's `DropSoundType` as the game numbers it (`EItemDropSoundType`):
/// none 0, a pistol 1, a submachine gun 2, a rifle 3.
pub fn drop_kind(name: &str) -> u8 {
    match name {
        "Pistol" => 1,
        "SubMachineGun" => 2,
        "Rifle" => 3,
        _ => 0,
    }
}

/// The weapons' landing sounds (`ITEM_DROP_SOUNDS`' `ItemDropSounds`).
#[derive(Debug, Clone, Default)]
pub struct DropSounds {
    pub banks: Vec<Bank>,
    /// The volume by the landing's energy (`EnergyToVolumeCurve`).
    pub volume: crate::formats::curve::Curve,
    /// The surface taken for one with no set (`DefaultSurfaceSound`).
    pub default_surface: u8,
    /// Each surface's banks by `DROP_KINDS`' order, as places in `banks`;
    /// a surface listed twice keeps its first set, as the game's map does.
    pub surfaces: Vec<(u8, [Option<usize>; 3])>,
    pub clips: Vec<(String, Option<Clip>)>,
}

impl DropSounds {
    /// The bank a weapon of a kind (`drop_kind`'s number) plays landing on
    /// a surface (`ItemDropSounds.GetSoundBank`): its surface's set, or the
    /// default surface's where it has none; nothing for a kind of none, a
    /// set without that bank, or no set for the default surface either
    /// (the game's lookup fails there).
    pub fn bank(&self, surface: u8, kind: u8) -> Option<usize> {
        let set = |surface: u8| self.surfaces.iter().find(|(s, _)| *s == surface);
        let (_, banks) = set(surface).or_else(|| set(self.default_surface))?;
        banks.get(usize::from(kind).checked_sub(1)?).copied().flatten()
    }
}

/// The `ItemDropSounds` of its bundle: the loaded assets, the object and
/// its fields.
fn item_drop_sounds(install: &Install) -> Result<(crate::unity::Assets, crate::unity::ObjRef, Value)> {
    use crate::unity::{class, extract};
    let mut assets = crate::unity::Assets::new(install.clone())?;
    let files = assets.load(ITEM_DROP_SOUNDS)?;
    let sounds = files
        .iter()
        .flat_map(|&file| assets.objects_of(file, class::MONO_BEHAVIOUR).collect::<Vec<_>>())
        .find(|&obj| extract::script_class(&assets, obj).as_deref() == Some("ItemDropSounds"))
        .context("no ItemDropSounds in its bundle")?;
    let value = assets.read(sounds)?;
    Ok((assets, sounds, value))
}

/// The volume a thing let fall plays at by its energy
/// (`ItemDropSounds.EnergyToVolumeCurve`), alone: what a corpse's fall goes
/// by (`BetterAudio.PlayDropItem` with a bank of its own), whose banks are
/// the movement's.
pub fn drop_volume_curve(install: &Install) -> Result<crate::formats::curve::Curve> {
    let (_, _, value) = item_drop_sounds(install)?;
    crate::character::player::curve(value.req("EnergyToVolumeCurve")?)
}

/// The weapons' landing sounds, with their clips' samples if `samples`.
pub fn drop_sounds(install: &Install, samples: bool) -> Result<DropSounds> {
    let mut setups = VorbisSetups::new(install);
    let (assets, sounds, value) = item_drop_sounds(install)?;
    let mut out = DropSounds {
        volume: crate::character::player::curve(value.req("EnergyToVolumeCurve")?)?,
        default_surface: value.get("DefaultSurfaceSound").and_then(Value::as_i64).unwrap_or(0) as u8,
        ..DropSounds::default()
    };
    let mut reader = BankReader::new(samples);
    for set in value.get("SurfaceSets").and_then(Value::as_array).unwrap_or(&[]) {
        let surface = set.get("Surface").and_then(Value::as_i64).unwrap_or(0) as u8;
        let mut banks = [None; 3];
        for (bank, name) in banks.iter_mut().zip(DROP_KINDS) {
            *bank = reader.bank_at(&assets, &mut setups, sounds.file, set.get(name), &mut out.banks, &mut out.clips)?;
        }
        if !out.surfaces.iter().any(|(s, _)| *s == surface) {
            out.surfaces.push((surface, banks));
        }
    }
    if !reader.unread.is_empty() {
        eprintln!("{} of the weapons' landing sounds are not read and are silent: {}", reader.unread.len(), reader.unread.join("; "));
    }
    Ok(out)
}

/// The calibres' classes of a spent case's sounds (`ECaliber`, a round's
/// `AmmoPoolObject._caliber`), by number, as `WeaponSounds` names them.
pub const SHELL_CALIBERS: [&str; 4] = ["9mm", "12cal", "556mm", "Heavy"];

/// The surfaces a spent case has sounds for, as `WeaponSounds` names them
/// (which surface plays which: `trp_sim::shells::sound_surface`).
pub const SHELL_SURFACES: [&str; 5] = ["Concrete", "Metal", "Soil", "Wood", "Plastic"];

/// The spent cases' sounds (`WEAPON_SOUNDS`' `Shell<calibre><surface>`):
/// every bank once, by calibre and surface (`SHELL_CALIBERS`,
/// `SHELL_SURFACES`), with their clips.
#[derive(Debug, Clone, Default)]
pub struct ShellSounds {
    pub banks: Vec<Bank>,
    pub by: [[Option<usize>; 5]; 4],
    pub clips: Vec<(String, Option<Clip>)>,
}

impl ShellSounds {
    /// The bank of a calibre's class and a surface of `SHELL_SURFACES`.
    pub fn bank(&self, caliber: u8, surface: usize) -> Option<usize> {
        *self.by.get(caliber as usize)?.get(surface)?
    }
}

/// The spent cases' sounds (`WeaponSounds`, `Firearms.PlayShellSound`),
/// with their clips' samples if `samples`.
pub fn shell_sounds(install: &Install, samples: bool) -> Result<ShellSounds> {
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
    let mut out = ShellSounds::default();
    let mut reader = BankReader::new(samples);
    for (c, caliber) in SHELL_CALIBERS.iter().enumerate() {
        for (s, surface) in SHELL_SURFACES.iter().enumerate() {
            out.by[c][s] = reader.bank_at(&assets, &mut setups, sounds.file, value.get(&format!("Shell{caliber}{surface}")), &mut out.banks, &mut out.clips)?;
        }
    }
    if !reader.unread.is_empty() {
        eprintln!("{} of the spent cases' sounds are not read and are silent: {}", reader.unread.len(), reader.unread.join("; "));
    }
    Ok(out)
}
