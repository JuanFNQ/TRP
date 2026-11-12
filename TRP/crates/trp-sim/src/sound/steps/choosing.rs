//! Making the step sounds, covert walking, kit noise and choosing a sound from a bank.

use super::*;

impl StepSounds {
    pub fn new(settings: StepSettings) -> Self {
        let mut sounds = Self {
            settings,
            figures: Vec::new(),
            kit_figures: [None; 4],
            surface: CONCRETE,
            state: State::Idle,
            previous: State::Idle,
            played: false,
            kept: [None; 14],
            gear_due: Vec::new(),
            last_turn: 0.0,
            turn_wait: None,
            turns: [0.0; 10],
            turn_at: 0,
            turn_count: 0,
            last_yaw: None,
            next_landing: 0.0,
            rng: Rng::new(0x5745_5053),
            asked: Vec::new(),
            covert: crate::condition::skills::Covert::default(),
            efficiency: 0.0,
            efficiency_speed: 0.0,
            efficiency_due: false,
            third_person: false,
            listener: None,
        };
        // The steps' source is made set for running (`CreateNestedSource`).
        sounds.work_out(Kind::Run, None, 0.0);
        sounds
    }

    /// Covert Movement's buffs, as the skill's level gives them: how
    /// quietly one walks is worked out afresh on the next step, as the game
    /// does when the level changes (`UpdateCovertEfficiency(force)`).
    pub fn set_covert(&mut self, covert: crate::condition::skills::Covert) {
        if covert != self.covert {
            self.covert = covert;
            self.efficiency_due = true;
        }
    }

    /// Covert Movement's buffs as they stand.
    pub fn covert(&self) -> crate::condition::skills::Covert {
        self.covert
    }

    /// How quietly the character walks now (`MovementContext.
    /// CovertEfficiency`), as last worked out: a walk ended above 0.9 is
    /// a quiet one (`CovertNoiseLevel` 0), which Covert Movement earns by.
    pub fn covert_efficiency(&self) -> f32 {
        self.efficiency
    }

    /// `CovertMovementVolume`: 1 untrained.
    pub(super) fn covert_volume(&self) -> f32 {
        self.covert.volume(soft(self.surface))
    }

    /// `CovertEquipmentNoise`: 1 and the overweight, less the buff.
    pub(super) fn equipment_noise(&self, overweight: f32) -> f32 {
        1.0 + overweight - self.covert.equipment
    }

    /// The banks' own figures, by surface, in `SURFACE_BANKS`' order.
    pub fn set_figures(&mut self, figures: Vec<(u8, [Option<Figures>; 9])>) {
        self.figures = figures;
    }

    /// Whether the banks' figures have been given.
    pub fn has_figures(&self) -> bool {
        !self.figures.is_empty()
    }

    /// The kit's banks' own figures, by `GEAR_BANKS`' order.
    pub fn set_kit_figures(&mut self, figures: [Option<Figures>; 4]) {
        self.kit_figures = figures;
    }

    /// The kit's medium rattle a mount sounds (`Player.EnterMountedStateHandler`,
    /// `ExitMountedStateHandler`: `PlayGearSound(_gearMediumSoundBank,
    /// RandomVolume)`; mounting spec, "Sounds"): the bank at its own random
    /// volume, then as `PlayGearSound` has any.
    pub fn kit_medium(&mut self, overweight: f32) {
        let f = self.kit_figures[GEAR_MEDIUM].unwrap_or(Figures { base_volume: 1.0, delta_volume: 0.0 });
        let volume = if f.delta_volume > 0.0 { f.base_volume + self.rng.range(-f.delta_volume, 0.0) } else { f.base_volume };
        let by = self.work_out(Kind::Gear, None, overweight);
        let noise = self.equipment_noise(overweight);
        self.ask(BankOf::Kit(GEAR_MEDIUM), true, volume * by * noise, Some(by));
    }

    /// What the rules have asked for since this was last called.
    pub fn take(&mut self) -> Vec<Play> {
        std::mem::take(&mut self.asked)
    }

    pub(super) fn figures(&self, bank: usize) -> Figures {
        self.figures
            .iter()
            .find(|(surface, _)| *surface == self.surface)
            .and_then(|(_, banks)| banks[bank])
            .unwrap_or(Figures { base_volume: 1.0, delta_volume: 0.0 })
    }

    /// `SoundBank.RandomVolume`: the base volume less up to its delta.
    fn random_volume(&mut self, bank: usize) -> f32 {
        let figures = self.figures(bank);
        if figures.delta_volume > 0.0 {
            figures.base_volume + self.rng.range(-figures.delta_volume, 0.0)
        } else {
            figures.base_volume
        }
    }

    /// A kind's multiplier worked out and kept (`CalculateMovementRolloff`
    /// over 70): the sound radius at the overweight, the kind's, and for a
    /// footstep a share from `min_speed_rolloff` at a standstill to 1 at
    /// the top speed.
    pub(super) fn work_out(&mut self, kind: Kind, speed_share: Option<f32>, overweight: f32) -> f32 {
        let [low, high] = self.settings.sound_radius;
        let radius = low + (high - low) * overweight.clamp(0.0, 1.0);
        let speed = speed_share.map_or(1.0, |t| lerp(self.settings.min_speed_rolloff, 1.0, t));
        let by = radius * self.settings.multipliers[kind as usize] * speed;
        self.kept[kind as usize] = Some(by);
        by
    }

    /// The kind's multiplier as last kept, worked out if it never was
    /// (`CalculateMovementVolumeDefaultMultiplier`), times one less the Bot
    /// Sound skill's buff (0 untrained).
    pub(super) fn kept_or_worked(&mut self, kind: Kind, overweight: f32) -> f32 {
        match self.kept[kind as usize] {
            Some(by) => by,
            None => self.work_out(kind, None, overweight),
        }
    }

    /// A sound asked for whose bank chooses by the listener's distance.
    pub(super) fn ask(&mut self, bank: BankOf, kit: bool, volume: f32, reach: Option<f32>) {
        self.asked.push(Play { bank, kit, volume, blend: 0.0, by_distance: true, reach });
    }

    /// `DefaultPlay`: the kind's multiplier worked out afresh, the volume
    /// times it, the bank at the listener's distance.
    pub(super) fn default_play(&mut self, bank: usize, volume: f32, kind: Kind, overweight: f32) {
        let by = self.work_out(kind, None, overweight);
        let surface = self.surface;
        self.ask(BankOf::Surface(surface, bank), false, volume * by, Some(by));
    }

    /// The kit's rattle the host asks for (`Player.PlayGearSound`, not
    /// fast): the sights brought up or down sound it at 0.2
    /// (`FirearmController.SetAim`).
    pub fn kit(&mut self, volume: f32, overweight: f32) {
        self.gear(volume, false, overweight);
    }

    /// A mark of a move over an obstacle plays a surface's element for it
    /// (`BaseVaultingAudioController.PlaySound`): the element's bank at
    /// `lerp(min, max, height)` times the bank's base volume, then as any
    /// bank at a point, with a blend value of 0 (the distance to the
    /// camera in first person). The host reads the range and the bank;
    /// the height goes as the play's volume.
    pub fn vault_element(&mut self, kind: usize, mark: usize, surface: u8, height: f32) {
        self.ask(BankOf::Vault { kind, mark, surface }, false, height, None);
    }

    /// The kit at a grab and at an end of a move over an obstacle
    /// (`PlayGearSound`): the move's bank at `lerp(0.1, 1, height)` times
    /// its base volume, then as any bank at a point.
    pub fn vault_gear(&mut self, kind: usize, height: f32) {
        self.ask(BankOf::VaultGear(kind), false, lerp(0.1, 1.0, height.clamp(0.0, 1.0)), None);
    }

    /// `PlayGearSound`: the kit's bank, the volume times the gear's
    /// multiplier and the equipment's noise (1 and the overweight, less the
    /// Covert Movement buff, 0 untrained).
    pub(super) fn gear(&mut self, volume: f32, fast: bool, overweight: f32) {
        let by = self.work_out(Kind::Gear, None, overweight);
        let noise = self.equipment_noise(overweight);
        self.ask(BankOf::Kit(if fast { GEAR_FAST } else { GEAR }), true, volume * by * noise, Some(by));
    }

    /// `PlayStepSound`: a footstep of `Run`, crouched low or not.
    pub(super) fn footstep(&mut self, m: &Moment) {
        let low = !m.prone && m.smoothed_pose_level < DUCK_BELOW;
        let (bank, kind) = if low { (DUCK, Kind::Duck) } else { (RUN, Kind::Run) };
        let slowest = if m.leg_damaged { MIN_STEP } else { MIN_STEP * m.overweight };
        // Covert Movement's factor (`CovertMovementVolumeBySpeed`, 1
        // untrained), on the blend and the volume alike.
        let by_speed = self.covert.step_volume(soft(self.surface), self.efficiency);
        let blend = m.clamped_speed.max(slowest) * by_speed;
        let speed_share = inverse_lerp(0.0, m.max_speed, m.speed);
        let share = speed_share.clamp(self.settings.min_speed_volume, 1.0);
        let by = self.kept_or_worked(kind, m.overweight);
        let random = if self.third_person { 1.0 } else { self.random_volume(bank) };
        let volume = by_speed * share * by * random;
        let reach = self.work_out(kind, Some(speed_share), m.overweight);
        let surface = self.surface;
        self.asked.push(Play { bank: BankOf::Surface(surface, bank), kit: false, volume, blend, by_distance: false, reach: Some(reach) });
        // The kit after it (`CalculateWalkGearVolume`).
        let kit = if low { blend.clamp(0.0, 0.3) } else { (blend * 0.75 * m.pose_level.max(0.0).sqrt()).clamp(0.1, 0.5) };
        if kit > 0.0 {
            self.gear_due.push((m.clock + self.settings.gear_delay, kit));
        }
    }

    /// A sprint's footstep (`Sprint`), with the kit at once.
    pub(super) fn sprint_step(&mut self, m: &Moment) {
        let base = if self.third_person { 1.0 } else { self.figures(SPRINT).base_volume };
        let heavy = 0.5 + 3.0 * m.overweight;
        let volume = self.kept_or_worked(Kind::Sprint, m.overweight) * base * heavy;
        let reach = self.work_out(Kind::Sprint, None, m.overweight);
        let surface = self.surface;
        self.ask(BankOf::Surface(surface, SPRINT), false, volume, Some(reach));
        self.gear(1.0, false, m.overweight);
    }

    /// `IsOutOfProtagonistsRange` turned round: whether the listener is
    /// within a surface check (by its bank) and a metre; one's own always.
    pub(super) fn in_range(&self, bank: usize) -> bool {
        let at = SURFACE_CHECKS.iter().position(|&b| b == bank).unwrap_or(0);
        self.listener.is_none_or(|distance| distance < self.settings.surface_checks[at] + CHECK_BUFFER)
    }

    /// `CheckSurface`: the line down asked of the world; a miss leaves the
    /// surface as it was.
    pub(super) fn check_surface(&mut self, surface: &mut dyn FnMut() -> Option<u8>) {
        if let Some(found) = surface() {
            self.surface = found;
        }
    }
}

/// How a bank goes through its clips (`SoundBank._shuffle`,
/// `_shuffleIndex`): one for each bank, kept by the host with the bank.
#[derive(Clone, Debug, Default)]
pub struct Shuffle {
    pub(super) order: Vec<u8>,
    left: u8,
}

/// What a bank plays: up to two clips, by their places in the host's list
/// (`trp_data::sound::MovementSounds::clips`), the first clip's share of
/// the volume when there are two, and the pitch.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Choice {
    pub clips: [Option<usize>; 2],
    pub share: f32,
    pub pitch: f32,
}

/// `SoundBank.PickClipsByDistance` and `RandomPitch`: the list or the two
/// lists the blend value falls in, a clip of each by the bank's shuffle,
/// the share between them, and a pitch drawn about the base.
pub fn choose(bank: &Bank, shuffle: &mut Shuffle, blend: f32, rng: &mut Rng) -> Choice {
    let mut clips = [None, None];
    let mut share = 1.0;
    let pick = |list: usize, shuffle: &mut Shuffle, rng: &mut Rng| -> Option<usize> {
        let clips = &bank.lists[list];
        if clips.is_empty() {
            return None;
        }
        Some(clips[next_index(shuffle, clips.len(), rng) % clips.len()])
    };
    match &bank.blend {
        None => clips[0] = pick(0, shuffle, rng),
        Some(points) => {
            let i = points.iter().position(|&point| blend < point).unwrap_or(points.len());
            match i {
                0 => clips[0] = pick(0, shuffle, rng),
                1 => {
                    clips[0] = pick(0, shuffle, rng);
                    clips[1] = pick(1, shuffle, rng);
                }
                2 => clips[0] = pick(1, shuffle, rng),
                3 => {
                    clips[0] = pick(1, shuffle, rng);
                    clips[1] = pick(2, shuffle, rng);
                }
                _ => clips[0] = pick(2, shuffle, rng),
            }
            if i % 2 == 1 {
                share = inverse_lerp(points[i], points[i - 1], blend);
            }
        }
    }
    let pitch = bank.base_pitch + rng.range(-bank.delta_pitch, bank.delta_pitch);
    Choice { clips, share, pitch }
}

/// `SoundBank.GetRandomClipIndex`: the next place of the bank's shuffled
/// order, shuffled again for `count` places when it has run out.
pub(super) fn next_index(shuffle: &mut Shuffle, count: usize, rng: &mut Rng) -> usize {
    if count <= 1 {
        return 0;
    }
    if shuffle.left < 1 {
        let count = count.min(u8::MAX as usize) as u8;
        if shuffle.order.len() < count as usize {
            shuffle.order.resize(count as usize, 0);
        }
        for (i, place) in shuffle.order.iter_mut().enumerate().take(count as usize) {
            *place = i as u8;
        }
        for i in 0..count as usize {
            // `Random.Range(0, n - 1)` for whole numbers leaves its top out.
            let other = random_whole(rng, count as usize - 1);
            shuffle.order.swap(other, i);
        }
        shuffle.left = count;
    }
    shuffle.left -= 1;
    shuffle.order[shuffle.left as usize] as usize
}

/// A whole number from 0 up to but not `below` (0 when it is 0).
fn random_whole(rng: &mut Rng, below: usize) -> usize {
    if below == 0 {
        return 0;
    }
    ((rng.unit() * below as f32) as usize).min(below - 1)
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

/// `Mathf.InverseLerp`: where `v` lies from `a` to `b`, 0 to 1.
pub(super) fn inverse_lerp(a: f32, b: f32, v: f32) -> f32 {
    if (b - a).abs() < f32::EPSILON {
        return 0.0;
    }
    ((v - a) / (b - a)).clamp(0.0, 1.0)
}

/// `Mathf.DeltaAngle`: the shortest turn from `from` to `to`, in degrees.
pub(super) fn delta_angle(from: f32, to: f32) -> f32 {
    let mut d = (to - from).rem_euclid(360.0);
    if d > 180.0 {
        d -= 360.0;
    }
    d
}
