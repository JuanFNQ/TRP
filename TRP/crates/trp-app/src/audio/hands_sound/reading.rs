//! The rig's sounds read, and the aiming banks read and chosen from.

use super::*;

/// A rig's sound player read, if there is one and its sounds are wanted.
pub fn read(unity: &trp_data::unity::Assets, weapon: &trp_data::weapons::weapon::Weapon, setups: &mut trp_data::audio::sound::VorbisSetups, wanted: Wanted) -> Option<Arc<RigSounds>> {
    if !wanted.hands && !wanted.shots {
        return None;
    }
    match trp_data::audio::sound::hands_sounds(unity, weapon, setups, wanted.shots) {
        Ok(Some(sounds)) => {
            let player = sounds.player.clone();
            let mut rig = RigSounds::of(sounds, wanted, weapon.silenced);
            // A weapon's player plays its aiming's sound (`WeaponSoundPlayer.Init`).
            if player == "WeaponSoundPlayer" && wanted.hands {
                rig.aiming = weapon.sound_kind.map(|kind| (kind, weapon.total_ergonomics));
            }
            // Its bipod's own player (mounting spec, "Sounds").
            if let (Some(part), true) = (weapon.bipod.as_ref(), wanted.hands) {
                if let Some(bundle) = &part.bundle {
                    match trp_data::audio::sound::bipod_sounds(unity, bundle, setups) {
                        Ok(Some(read)) => {
                            let surfaces: Vec<(u8, ClipSet)> = read
                                .surfaces
                                .into_iter()
                                .filter_map(|(name, set)| trp_data::audio::sound::SURFACES.iter().position(|s| *s == name).map(|n| (n as u8, ClipSet::of(set))))
                                .collect();
                            info!("{}: its bipod's sound player ({} surfaces)", weapon.name, surfaces.len());
                            rig.bipod = Some(BipodSet { node: part.node, open: ClipSet::of(read.open), close: ClipSet::of(read.close), surfaces });
                        }
                        Ok(None) => info!("{}: its bipod has no sound player in {bundle}", weapon.name),
                        Err(e) => warn!("the bipod's sounds of {}: {e:#}", weapon.name),
                    }
                }
            }
            // Its underbarrel launcher's own player, in its bundle
            // (underbarrel-launcher spec, "Two animators").
            if let Some(bundle) = weapon.launcher.as_ref().and_then(|part| part.bundle.as_ref()) {
                match trp_data::audio::sound::bundle_sounds(unity, bundle, setups, wanted.shots) {
                    Ok(Some(read)) => {
                        let launcher = RigSounds::of(read, wanted, false);
                        info!("{}: its launcher's sound player, {} sounds{}", weapon.name, launcher.len(), if launcher.shots.is_some() { ", and its shots" } else { "" });
                        rig.launcher = Some(Arc::new(launcher));
                    }
                    Ok(None) => info!("{}: its launcher has no sound player in {bundle}", weapon.name),
                    Err(e) => warn!("the launcher's sounds of {}: {e:#}", weapon.name),
                }
            }
            info!("{}: its {player}, {} sounds{}", weapon.name, rig.len(), if rig.shots.is_some() { ", and its shots" } else { "" });
            Some(Arc::new(rig))
        }
        Ok(None) => None,
        Err(e) => {
            warn!("the sounds of {}: {e:#}", weapon.name);
            None
        }
    }
}

/// Where the falloff seen from outside begins, in metres.
pub(super) const FULL_WITHIN: f32 = 1.0;

/// How far the bipod's line toward the mount's point looks for the
/// surface (`BipodAudioController.CHECK_SURFACE_RAYCAST_LENGTH`).
pub(super) const BIPOD_SURFACE_RAY: f32 = 1.0;

/// How high over the feet the hands' sounds are taken to come from, seen
/// from outside (`PROVISIONAL`).
pub(super) const HANDS_HIGH: f32 = 1.4;

/// The random draws of which clip plays.
#[derive(Resource)]
pub(super) struct Draws(pub(super) Rng);

/// The aiming's sounds (`trp_data::sound::aiming_sounds`), by the weapon's
/// kind, read away from the frame when `HANDS SOUNDS` is on; and each
/// bank's clips not yet played since all last were (`GetRandomClipWithExclusion`).
#[derive(Resource, Default)]
pub struct AimingBanks {
    pub(super) banks: HashMap<String, Arc<AimingBank>>,
    pub(super) left: HashMap<String, Vec<usize>>,
    /// The weapon's mount sound (`WeaponSounds.VerticalMountSounds`), with
    /// its random share of the volume (`DeltaVolume`).
    pub(super) mount: Option<(Arc<AimingBank>, f32)>,
    coming: Option<Mutex<Receiver<(ByKind, Option<(Arc<AimingBank>, f32)>)>>>,
}

/// The banks by the weapons' kinds, as read.
pub(super) type ByKind = Vec<(String, Arc<AimingBank>)>;

pub struct AimingBank {
    pub(super) name: String,
    pub(super) volume: f32,
    pub(super) rolloff: f32,
    pub(super) clips: Vec<(String, Samples)>,
}

/// Starts reading the aiming's sounds, if the hands' sounds are on.
pub fn start_reading(app: &mut App, install: trp_data::Install, on: bool) {
    let mut banks = AimingBanks::default();
    if on {
        let (send, receive) = channel();
        std::thread::spawn(move || match trp_data::audio::sound::aiming_sounds(&install) {
            Ok(read) => {
                // The mount's bank beside them (mounting spec, "Sounds").
                let mount = match trp_data::audio::sound::mount_sounds(&install) {
                    Ok((bank, clips)) => Some((
                        Arc::new(AimingBank { name: bank.name.clone(), volume: bank.base_volume, rolloff: bank.rolloff, clips: clips.into_iter().map(|(name, clip)| (name, Samples::of_owned(clip))).collect() }),
                        bank.delta_volume,
                    )),
                    Err(e) => {
                        eprintln!("the weapon's mount sound could not be read; it is silent: {e:#}");
                        None
                    }
                };
                // Each bank made ready once, however many kinds share it.
                let mut made: HashMap<String, Arc<AimingBank>> = HashMap::new();
                let banks = read
                    .into_iter()
                    .map(|(kind, bank)| {
                        let ready = made
                            .entry(bank.name.clone())
                            .or_insert_with(|| Arc::new(AimingBank { name: bank.name.clone(), volume: bank.volume, rolloff: bank.rolloff, clips: bank.clips.iter().map(|clip| (clip.name.clone(), Samples::of_owned(clip.clone()))).collect() }))
                            .clone();
                        (kind, ready)
                    })
                    .collect();
                let _ = send.send((banks, mount));
            }
            Err(e) => eprintln!("the aiming's sounds could not be read; they are silent: {e:#}"),
        });
        banks.coming = Some(Mutex::new(receive));
    }
    app.insert_resource(banks);
}

pub(super) fn take_aiming(mut banks: ResMut<AimingBanks>) {
    let Some(coming) = banks.coming.take() else { return };
    let got = coming.lock().ok().and_then(|receive| match receive.try_recv() {
        Ok(read) => Some(Some(read)),
        Err(std::sync::mpsc::TryRecvError::Empty) => None,
        Err(std::sync::mpsc::TryRecvError::Disconnected) => Some(None),
    });
    match got {
        None => banks.coming = Some(coming),
        Some(Some((read, mount))) => {
            info!("the aiming's sounds: {} kinds of weapon", read.len());
            banks.banks = read.into_iter().collect();
            banks.mount = mount;
        }
        Some(None) => {}
    }
}

/// The aiming's sound's volume (`CalculateAimingSoundVolume`): the square
/// of how far the weapon's ergonomics fall short of 100, as a share,
/// within 0.1 and 0.2; times the equipment's noise (1 and the overweight,
/// less the Covert Movement buff); the draw's skill and the bots' sound
/// coefficient untrained (0).
pub(super) fn aiming_volume(ergonomics: f32, overweight: f32) -> f32 {
    let short = ergonomics / 100.0 - 1.0;
    (short * short).clamp(0.1, 0.2) * (1.0 + overweight)
}

/// The clip of a bank to play next (`GetRandomClipWithExclusion`): one of
/// those not played since all last were, drawn by an integer
/// `Random.Range(0, n - 1)` among them (so never the last of them while
/// another is left), and set aside.
pub(super) fn next_clip(left: &mut Vec<usize>, count: usize, rng: &mut Rng) -> Option<usize> {
    if count == 0 {
        return None;
    }
    if left.is_empty() {
        left.extend(0..count);
    }
    let top = left.len().saturating_sub(1);
    let at = if top == 0 { 0 } else { (rng.range(0.0, top as f32) as usize).min(top - 1) };
    Some(left.remove(at))
}
