//! The sounds of the player's own moving (`docs/spec/footsteps.md`; the
//! user, 2026-10-16: "Lets add the sounds for moving in the world. May want
//! to add different materials on the ground to walk on. Look how tarkov
//! handles footsteps"): when the game's player plays a footstep, a
//! sprint's, a stop, a jump, a landing, a crawl, a turn or its kit, from
//! which bank of which surface, how loud and at what blend value
//! (`Player`'s `Run`, `Sprint`, `Idle`, `StateChangedHandler`,
//! `PlayStepSound`, `PlayGroundedSound`, `PlaySoundBank`, `PlayTurnSound`,
//! `PlayGearSound`, `DefaultPlay`); and how a bank chooses its clips and
//! pitch (`SoundBank`). The movement runs the rules on every step
//! (`Movement::hear`) and the host plays what they ask for.

use anyhow::{Context, Result};
use serde_json::Value as Json;
use trp_data::hard_settings::HardSettings;
use trp_data::sound::Bank;

use crate::movement::State;
use crate::recoil::Rng;

/// The kinds of movement the loudness goes by (`EAudioMovementState`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    None,
    Run,
    Sprint,
    Stop,
    Land,
    Turn,
    Duck,
    Prone,
    Drop,
    Jump,
    Gear,
    VaultLand,
    VaultGeneric,
    Search,
}

impl Kind {
    pub const ALL: [Kind; 14] = [
        Kind::None,
        Kind::Run,
        Kind::Sprint,
        Kind::Stop,
        Kind::Land,
        Kind::Turn,
        Kind::Duck,
        Kind::Prone,
        Kind::Drop,
        Kind::Jump,
        Kind::Gear,
        Kind::VaultLand,
        Kind::VaultGeneric,
        Kind::Search,
    ];

    fn name(self) -> &'static str {
        match self {
            Kind::None => "None",
            Kind::Run => "Run",
            Kind::Sprint => "Sprint",
            Kind::Stop => "Stop",
            Kind::Land => "Land",
            Kind::Turn => "Turn",
            Kind::Duck => "Duck",
            Kind::Prone => "Prone",
            Kind::Drop => "Drop",
            Kind::Jump => "Jump",
            Kind::Gear => "Gear",
            Kind::VaultLand => "VaultLand",
            Kind::VaultGeneric => "VaultGeneric",
            Kind::Search => "Search",
        }
    }
}

/// A surface's banks by their place in `trp_data::sound::SURFACE_BANKS`.
pub const RUN: usize = 0;
pub const SPRINT: usize = 1;
pub const STOP: usize = 2;
pub const LANDING: usize = 3;
pub const TURN: usize = 4;
pub const DUCK: usize = 5;
pub const PRONE: usize = 6;
pub const PRONE_DROP: usize = 7;
pub const JUMP: usize = 8;
/// The kit's banks by their place in `trp_data::sound::GEAR_BANKS`.
pub const GEAR: usize = 0;
pub const GEAR_MEDIUM: usize = 1;
pub const GEAR_FAST: usize = 2;

/// Concrete: where the player begins, and what a thing without a surface
/// of its own is (`BaseBallistic.ESurfaceSound.Concrete`).
pub const CONCRETE: u8 = 0;

/// Which bank a sound is from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BankOf {
    /// A surface's (`SurfaceSet`), by the surface's number and the bank's
    /// place (`RUN` and the rest).
    Surface(u8, usize),
    /// The kit's (`GEAR`, `GEAR_FAST`).
    Kit(usize),
    /// A surface's element for a move over an obstacle at one of its
    /// marks (`SurfaceSet.VaultSoundSet` and the other two; footsteps
    /// spec, "The vault's sounds"): the move (`trp_data::sound::VAULT_SETS`'
    /// place), the mark (`VAULT_MARKS`' place) and the surface. The play's
    /// `volume` is then the obstacle's height, which the element's volume
    /// range is read at.
    Vault { kind: usize, mark: usize, surface: u8 },
    /// The kit's bank for a move over an obstacle (`GearVault` and the
    /// other two, `trp_data::sound::VAULT_GEAR_BANKS`' place). The play's
    /// `volume` is the height's share of the bank's own range.
    VaultGear(usize),
}

/// A sound the rules ask for: its bank, whether it is the kit's source
/// (`_gearSource`) or the steps' (`NestedStepSoundSource`), its volume
/// (before the bank clamps it to 0 to 1; a vault's is the obstacle's
/// height instead, `BankOf::Vault`) and the value the bank chooses its
/// list by.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Play {
    pub bank: BankOf,
    pub kit: bool,
    pub volume: f32,
    pub blend: f32,
    /// Whether the bank chooses by the listener's distance instead of
    /// `blend` (every sound but a footstep passes the distance, which is
    /// nought in first person).
    pub by_distance: bool,
    /// The kind's multiplier its source's farthest was just set by
    /// (`UpdateStepSourceRolloff`, `CalculateMovementRolloff` over its
    /// base): the source is heard no farther than the base rolloff times
    /// this times the environment's share (`StepSettings`). None for a
    /// vault's, which a bank plays at a point by its own rolloff.
    pub reach: Option<f32>,
}

/// The figures the rules go by.
#[derive(Clone, Debug)]
pub struct StepSettings {
    /// `Stamina.SoundRadius` (`globals.json`): from the first to the
    /// second by the overweight.
    pub sound_radius: [f32; 2],
    /// Each kind's multiplier, by `Kind::ALL`'s order
    /// (`PlayerSettings.MovementRolloffMultipliers` in SPT's
    /// `settings.json`; 1 where it names none, as the game's class has it).
    pub multipliers: [f32; 14],
    /// The least a footstep's speed share of volume is, and its share of
    /// the rolloff at a standstill (`MinSpeedVolumeMult`,
    /// `MinSpeedRolloffMult`: the class's own 0.15 and 0.8, as the install's
    /// file names them otherwise and the game reads neither).
    pub min_speed_volume: f32,
    pub min_speed_rolloff: f32,
    /// How long after a footstep the kit is heard (`GEAR_SOUND_DELAY`), the
    /// angle of the hands from the body past which standing turns are
    /// heard (`TURN_ANGLE`), and how long the check then waits
    /// (`TURN_SOUND_DELAY`): the game's hard settings.
    pub gear_delay: f32,
    pub turn_angle: f32,
    pub turn_delay: f32,
    /// How far a character seen from outside is heard at a kind's
    /// multiplier of 1 (`BaseMaxMovementRolloff`; first person's is 70,
    /// a constant), and the share of it outdoors and indoors
    /// (`OutdoorRolloffMult`, `IndoorRolloffMult`): SPT's `settings.json`.
    pub third_person_rolloff: f32,
    pub outdoor_rolloff: f32,
    pub indoor_rolloff: f32,
}

impl Default for StepSettings {
    /// The game's own figures where the install gives none (the hard
    /// settings' as read on 2026-10-16, `globals.json`'s sound radius, and
    /// SPT's multipliers).
    fn default() -> Self {
        let mut settings = Self {
            sound_radius: [1.0, 1.3],
            multipliers: [1.0; 14],
            min_speed_volume: 0.15,
            min_speed_rolloff: 0.8,
            gear_delay: 0.1,
            turn_angle: 45.0,
            turn_delay: 0.5,
            third_person_rolloff: 90.0,
            outdoor_rolloff: 0.98,
            indoor_rolloff: 1.0,
        };
        for (kind, by) in [
            (Kind::Run, 0.85),
            (Kind::Sprint, 0.95),
            (Kind::Stop, 0.85),
            (Kind::Turn, 0.8),
            (Kind::Duck, 0.4),
            (Kind::Prone, 0.5),
            (Kind::Drop, 0.95),
            (Kind::Jump, 0.5),
            (Kind::Gear, 0.5),
            (Kind::VaultLand, 0.85),
            (Kind::VaultGeneric, 0.5),
            (Kind::Search, 0.2),
        ] {
            settings.multipliers[kind as usize] = by;
        }
        settings
    }
}

impl StepSettings {
    /// From `globals.json`'s `config` and the hard settings; the kinds'
    /// multipliers are the class's 1 until `read_client`.
    pub fn new(config: &Json, hard: &HardSettings) -> Result<Self> {
        let radius = config.get("Stamina").and_then(|s| s.get("SoundRadius")).context("globals have no Stamina.SoundRadius")?;
        let part = |k: &str| radius.get(k).and_then(Json::as_f64).map(|v| v as f32).context("Stamina.SoundRadius");
        Ok(Self {
            sound_radius: [part("x")?, part("y")?],
            multipliers: [1.0; 14],
            min_speed_volume: 0.15,
            min_speed_rolloff: 0.8,
            gear_delay: hard.float("GEAR_SOUND_DELAY")?,
            turn_angle: hard.float("TURN_ANGLE")?,
            turn_delay: hard.float("TURN_SOUND_DELAY")?,
            third_person_rolloff: 90.0,
            outdoor_rolloff: 0.98,
            indoor_rolloff: 1.0,
        })
    }

    /// The kinds' multipliers from SPT's `settings.json` (its `config`,
    /// which the game's `ClientSettingsConfig` is read from):
    /// `AudioSettings.PlayerSettings.MovementRolloffMultipliers`, a kind
    /// by its name and its multiplier.
    pub fn read_client(&mut self, client: &Json) {
        let listed = client.pointer("/AudioSettings/PlayerSettings/MovementRolloffMultipliers").and_then(Json::as_array);
        for entry in listed.into_iter().flatten() {
            let name = entry.get("MovementState").and_then(Json::as_str);
            let by = entry.get("RolloffMultiplier").and_then(Json::as_f64);
            if let (Some(kind), Some(by)) = (Kind::ALL.iter().find(|kind| Some(kind.name()) == name), by) {
                self.multipliers[*kind as usize] = by as f32;
            }
        }
        let figure = |name: &str| client.pointer(&format!("/AudioSettings/PlayerSettings/{name}")).and_then(Json::as_f64).map(|v| v as f32);
        if let Some(rolloff) = figure("BaseMaxMovementRolloff") {
            self.third_person_rolloff = rolloff;
        }
        if let Some(by) = figure("OutdoorRolloffMult") {
            self.outdoor_rolloff = by;
        }
        if let Some(by) = figure("IndoorRolloffMult") {
            self.indoor_rolloff = by;
        }
    }
}

/// A bank's own figures as the rules use them: its base volume and how
/// much it takes off at random (`BaseVolume`, `DeltaVolume`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Figures {
    pub base_volume: f32,
    pub delta_volume: f32,
}

impl Figures {
    pub fn of(bank: &Bank) -> Self {
        Self { base_volume: bank.base_volume, delta_volume: bank.delta_volume }
    }
}

/// What the movement tells the rules on a step.
pub struct Moment {
    /// The movement's clock and its step.
    pub clock: f32,
    pub dt: f32,
    pub state: State,
    /// A footstep was heard on this step (`Movement::watch_footsteps`).
    pub footstep: bool,
    /// The crawl's marks passed on this step (`PlaySoundBank("Prone")`).
    pub crawl_marks: usize,
    /// The ground found on this step after being off it: how far below the
    /// highest point and how far above the take-off the feet are.
    pub landed: Option<(f32, f32)>,
    pub prone: bool,
    /// The stance and its smoothed value, 0 to 1.
    pub pose_level: f32,
    pub smoothed_pose_level: f32,
    /// The speed setting after its limits (`ClampedSpeed`), before them
    /// (`CharacterMovementSpeed`), and the top one (`MaxSpeed`).
    pub clamped_speed: f32,
    pub speed: f32,
    pub max_speed: f32,
    /// 0 to 1 by carried weight (`Physical.Overweight`), and for an
    /// upright walk (`WalkOverweight`), which cuts how quietly one walks.
    pub overweight: f32,
    pub walk_overweight: f32,
    pub leg_damaged: bool,
    /// The view's yaw, and the hands' angle from the body, in degrees.
    pub yaw: f32,
    pub hands_to_body: f32,
}

/// The rules' memory between steps.
#[derive(Clone, Debug)]
pub struct StepSounds {
    pub settings: StepSettings,
    /// The banks' figures by surface and bank, once the host has read
    /// them (`set_figures`): until then a bank's are 1 and 0.
    figures: Vec<(u8, [Option<Figures>; 9])>,
    /// The kit's banks' figures, by `trp_data::sound::GEAR_BANKS`' order
    /// (`set_kit_figures`).
    kit_figures: [Option<Figures>; 4],
    /// The surface the feet were last found on (`CurrentSurface`).
    pub surface: u8,
    /// The movement state as last seen, and the one before it
    /// (`MovementContext.PreviousState`).
    state: State,
    previous: State,
    /// A footstep heard since `Run` or `Sprint` began (`_playedAtLeastOneStep`).
    played: bool,
    /// Each kind's multiplier as last worked out (`_cachedMovementRolloff`,
    /// over the 70 m of a first-person rolloff).
    kept: [Option<f32>; 14],
    /// The kit's sounds waiting for their time (`Gear`): when, and how loud.
    gear_due: Vec<(f32, f32)>,
    /// The last turn sound's time (`_lastTimeTurnSound`), and the idle
    /// check's wait: until when, and whether the frame after it is skipped
    /// too (the `yield return null` after the wait).
    last_turn: f32,
    turn_wait: Option<(f32, bool)>,
    /// The yaw's turn on each of the last ten steps, in degrees a second
    /// (`AverageRotationSpeed`, a `RollingMedian(10)`), where the next goes,
    /// how many there are, and the last yaw.
    turns: [f32; 10],
    turn_at: usize,
    turn_count: usize,
    last_yaw: Option<f32>,
    /// No landing sound until then (`_nextJumpAfter`).
    next_landing: f32,
    rng: Rng,
    /// What the rules have asked for and the host has not taken.
    asked: Vec<Play>,
    /// Covert Movement's buffs (`skills::Covert`: every factor 1
    /// untrained), how quietly one walks (`CovertEfficiency`), and the
    /// speed it was last worked out at (`_lastSpeedCovertCheck`), or that it
    /// is to be worked out afresh.
    covert: crate::skills::Covert,
    efficiency: f32,
    efficiency_speed: f32,
    efficiency_due: bool,
    /// A character seen from outside (`FirstPersonPointOfView` false): a
    /// footstep, a sprint's step and the stop take no volume of their
    /// bank's (`PlayStepSound`, `Sprint`, `StateChangedHandler`).
    pub third_person: bool,
}

/// The surfaces the game counts as soft (`MovementContext.SoftSurface`:
/// asphalt, concrete, gravel, soil, wood, thick wood and puddle, by their
/// numbers in `BaseBallistic.ESurfaceSound`), which only Covert Movement's
/// figures go by.
fn soft(surface: u8) -> bool {
    matches!(surface, 0 | 2 | 3 | 5 | 7 | 9 | 12)
}

impl Default for StepSounds {
    fn default() -> Self {
        Self::new(StepSettings::default())
    }
}

/// The game's falloff of a turn's share with the stance: a full turn a
/// second standing, two crouched fully.
const TURN_FULL: f32 = 360.0;
/// The turn sound's own wait (`maxLengthTurnSound`).
const TURN_AGAIN: f32 = 0.6;
/// A drop the landing is heard from (`LandingThreshold`), and the share of
/// it that is heard whole (`LANDING_VOLUME_MULT`).
const LANDING_THRESHOLD: f32 = 0.3;
const LANDING_WHOLE: f32 = 2.5;
/// No footstep heard in a `Run` this long after the last, and one is
/// played as it is left.
const RUN_LEFT_QUIET: f32 = 0.66;
/// A crawl's mark is heard this long after the last footstep or mark.
const CRAWL_GAP: f32 = 0.5;
/// The smoothed stance under which a step is crouched low (`Player.Pose`).
const DUCK_BELOW: f32 = 0.11;
/// The slowest footstep's blend value at full overweight or with a leg
/// damaged (`PhysicalBase.MinStepSound`).
const MIN_STEP: f32 = 0.6;

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
            covert: crate::skills::Covert::default(),
            efficiency: 0.0,
            efficiency_speed: 0.0,
            efficiency_due: false,
            third_person: false,
        };
        // The steps' source is made set for running (`CreateNestedSource`).
        sounds.work_out(Kind::Run, None, 0.0);
        sounds
    }

    /// Covert Movement's buffs, as the skill's level gives them: how
    /// quietly one walks is worked out afresh on the next step, as the game
    /// does when the level changes (`UpdateCovertEfficiency(force)`).
    pub fn set_covert(&mut self, covert: crate::skills::Covert) {
        if covert != self.covert {
            self.covert = covert;
            self.efficiency_due = true;
        }
    }

    /// Covert Movement's buffs as they stand.
    pub fn covert(&self) -> crate::skills::Covert {
        self.covert
    }

    /// `CovertMovementVolume`: 1 untrained.
    fn covert_volume(&self) -> f32 {
        self.covert.volume(soft(self.surface))
    }

    /// `CovertEquipmentNoise`: 1 and the overweight, less the buff.
    fn equipment_noise(&self, overweight: f32) -> f32 {
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

    fn figures(&self, bank: usize) -> Figures {
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
    fn work_out(&mut self, kind: Kind, speed_share: Option<f32>, overweight: f32) -> f32 {
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
    fn kept_or_worked(&mut self, kind: Kind, overweight: f32) -> f32 {
        match self.kept[kind as usize] {
            Some(by) => by,
            None => self.work_out(kind, None, overweight),
        }
    }

    /// A sound asked for whose bank chooses by the listener's distance.
    fn ask(&mut self, bank: BankOf, kit: bool, volume: f32, reach: Option<f32>) {
        self.asked.push(Play { bank, kit, volume, blend: 0.0, by_distance: true, reach });
    }

    /// `DefaultPlay`: the kind's multiplier worked out afresh, the volume
    /// times it, the bank at the listener's distance.
    fn default_play(&mut self, bank: usize, volume: f32, kind: Kind, overweight: f32) {
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
    fn gear(&mut self, volume: f32, fast: bool, overweight: f32) {
        let by = self.work_out(Kind::Gear, None, overweight);
        let noise = self.equipment_noise(overweight);
        self.ask(BankOf::Kit(if fast { GEAR_FAST } else { GEAR }), true, volume * by * noise, Some(by));
    }

    /// `PlayStepSound`: a footstep of `Run`, crouched low or not.
    fn footstep(&mut self, m: &Moment) {
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
    fn sprint_step(&mut self, m: &Moment) {
        let base = if self.third_person { 1.0 } else { self.figures(SPRINT).base_volume };
        let heavy = 0.5 + 3.0 * m.overweight;
        let volume = self.kept_or_worked(Kind::Sprint, m.overweight) * base * heavy;
        let reach = self.work_out(Kind::Sprint, None, m.overweight);
        let surface = self.surface;
        self.ask(BankOf::Surface(surface, SPRINT), false, volume, Some(reach));
        self.gear(1.0, false, m.overweight);
    }

    /// `CheckSurface`: the line down asked of the world; a miss leaves the
    /// surface as it was.
    fn check_surface(&mut self, surface: &mut dyn FnMut() -> Option<u8>) {
        if let Some(found) = surface() {
            self.surface = found;
        }
    }

    /// The rules for one step of the movement. `last_step` is the time of
    /// the last footstep (`_lastStepTime`), which a footstep played on
    /// leaving `Run` and a crawl's mark set too; `surface` casts the line
    /// down.
    pub fn update(&mut self, m: &Moment, last_step: &mut f32, surface: &mut dyn FnMut() -> Option<u8>) {
        // How quietly one walks, worked out again when the speed has
        // changed by a hundredth (`UpdateCovertEfficiency`).
        if self.efficiency_due || (m.clamped_speed - self.efficiency_speed).abs() >= 0.01 {
            let share = if m.max_speed > 0.0 { m.clamped_speed / m.max_speed } else { 0.0 };
            self.efficiency = self.covert.efficiency(share, m.leg_damaged, m.walk_overweight);
            (self.efficiency_speed, self.efficiency_due) = (m.clamped_speed, false);
        }
        // The view's turn on this step (`UpdateRotationSpeed`).
        if m.dt > 0.0 {
            if let Some(last) = self.last_yaw {
                self.turn_at = (self.turn_at + 1) % self.turns.len();
                self.turns[self.turn_at] = delta_angle(last, m.yaw) / m.dt;
                self.turn_count = (self.turn_count + 1).min(self.turns.len());
            }
            self.last_yaw = Some(m.yaw);
        }
        // The ground found again (`PlayGroundedSound`).
        if let Some((fall, rise)) = m.landed {
            self.landing(m, fall, rise, surface);
        }
        if m.state != self.state {
            self.state_changed(self.state, m.state, m, last_step, surface);
            self.previous = self.state;
            self.state = m.state;
        }
        if m.footstep {
            match m.state {
                State::Move => {
                    self.played = true;
                    self.check_surface(surface);
                    self.footstep(m);
                }
                State::Sprint => {
                    self.played = true;
                    self.check_surface(surface);
                    self.sprint_step(m);
                }
                _ => {}
            }
        }
        for _ in 0..m.crawl_marks {
            if m.clock - *last_step >= CRAWL_GAP {
                self.check_surface(surface);
                let base = self.figures(PRONE).base_volume;
                let volume = (self.covert_volume() * m.clamped_speed * base).max(0.4) * self.kept_or_worked(Kind::Prone, m.overweight);
                let reach = self.work_out(Kind::Prone, None, m.overweight);
                let at = self.surface;
                self.ask(BankOf::Surface(at, PRONE), false, volume, Some(reach));
                *last_step = m.clock;
            }
        }
        if m.state == State::Idle {
            self.idle(m);
        }
        let mut due = std::mem::take(&mut self.gear_due);
        due.retain(|&(when, volume)| {
            if m.clock + 1e-6 >= when {
                self.gear(volume, false, m.overweight);
                false
            } else {
                true
            }
        });
        self.gear_due.extend(due);
    }

    /// `StateChangedHandler`: what leaving one state and entering the next
    /// plays.
    fn state_changed(&mut self, from: State, to: State, m: &Moment, last_step: &mut f32, surface: &mut dyn FnMut() -> Option<u8>) {
        match from {
            State::Idle => self.turn_wait = None,
            State::Move => {
                if !self.played && m.clock - *last_step > RUN_LEFT_QUIET {
                    self.check_surface(surface);
                    self.footstep(m);
                    *last_step = m.clock;
                }
            }
            State::Sprint => {
                if !self.played {
                    self.check_surface(surface);
                    self.default_play(SPRINT, 1.0, Kind::Sprint, m.overweight);
                }
                if matches!(to, State::SprintStop | State::Idle) {
                    let base = if self.third_person { 1.0 } else { self.figures(STOP).base_volume };
                    let covert = self.covert_volume();
                    self.default_play(STOP, base * covert, Kind::Stop, m.overweight);
                }
            }
            _ => {}
        }
        match to {
            State::FromProne => self.gear(0.7, true, m.overweight),
            State::Sprint | State::Move => self.played = false,
            State::Jump => {
                self.default_play(JUMP, 1.0, Kind::Jump, m.overweight);
                let noise = self.equipment_noise(m.overweight);
                self.gear(noise, true, m.overweight);
            }
            State::ToProne => {
                let volume = 0.7 * self.covert_volume();
                if from == State::Sprint {
                    self.default_play(PRONE_DROP, volume, Kind::Drop, m.overweight);
                } else {
                    self.gear(volume, true, m.overweight);
                }
            }
            _ => {}
        }
    }

    /// `PlayGroundedSound`: the larger of the drop and the rise (the drop
    /// alone after a climb up), heard over 0.3 m, not within 0.5 s of the
    /// last, nor out of a climb or a vault's landing. The ground is found
    /// before the animator changes the state on the step (`CheckFlying` in
    /// `ManualUpdate`), so the states are those seen before it.
    fn landing(&mut self, m: &Moment, fall: f32, rise: f32, surface: &mut dyn FnMut() -> Option<u8>) {
        if m.clock < self.next_landing {
            return;
        }
        let (now, before) = (self.state, self.previous);
        let quiet = !matches!(now, State::Move | State::Sprint) && (before == State::Climb || now == State::Climb || before == State::VaultLanding);
        if quiet {
            return;
        }
        let height = if before == State::Climb { fall } else { fall.max(rise) };
        if height > LANDING_THRESHOLD {
            self.check_surface(surface);
            let volume = inverse_lerp(0.1, LANDING_THRESHOLD * LANDING_WHOLE, height);
            self.default_play(LANDING, volume, Kind::Land, m.overweight);
            self.next_landing = m.clock + 0.5;
        }
    }

    /// `Idle`: a turn of the hands past the angle is heard, and the check
    /// waits.
    fn idle(&mut self, m: &Moment) {
        if let Some((until, skip)) = self.turn_wait {
            if m.clock + 1e-6 < until {
                return;
            }
            if skip {
                self.turn_wait = Some((until, false));
                return;
            }
            self.turn_wait = None;
        }
        if m.hands_to_body.abs() > self.settings.turn_angle {
            self.turn(m);
            self.turn_wait = Some((m.clock + self.settings.turn_delay, true));
        }
    }

    /// `PlayTurnSound`.
    fn turn(&mut self, m: &Moment) {
        if m.clock - self.last_turn < TURN_AGAIN {
            return;
        }
        // The average of the last turns, signed (`RollingMedian.Avarage`):
        // a turn to the left is below 0 and silent.
        let average = if self.turn_count == 0 { 0.0 } else { self.turns.iter().sum::<f32>() / self.turn_count as f32 };
        let share = inverse_lerp(1.0, TURN_FULL + (1.0 - m.pose_level) * TURN_FULL, average.max(0.0));
        let volume = share * self.covert_volume() * self.figures(TURN).base_volume;
        self.default_play(TURN, volume, Kind::Turn, m.overweight);
        self.last_turn = m.clock;
        if share > 0.4 {
            self.gear(volume, false, m.overweight);
        }
    }
}

/// How a bank goes through its clips (`SoundBank._shuffle`,
/// `_shuffleIndex`): one for each bank, kept by the host with the bank.
#[derive(Clone, Debug, Default)]
pub struct Shuffle {
    order: Vec<u8>,
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
fn next_index(shuffle: &mut Shuffle, count: usize, rng: &mut Rng) -> usize {
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
fn inverse_lerp(a: f32, b: f32, v: f32) -> f32 {
    if (b - a).abs() < f32::EPSILON {
        return 0.0;
    }
    ((v - a) / (b - a)).clamp(0.0, 1.0)
}

/// `Mathf.DeltaAngle`: the shortest turn from `from` to `to`, in degrees.
fn delta_angle(from: f32, to: f32) -> f32 {
    let mut d = (to - from).rem_euclid(360.0);
    if d > 180.0 {
        d -= 360.0;
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bank(blend: Option<Vec<f32>>, lists: [Vec<usize>; 3]) -> Bank {
        Bank { name: "b".into(), base_volume: 0.68, base_pitch: 1.0, delta_volume: 0.05, delta_pitch: 0.15, rolloff: 52.5, blend, lists, source_type: 4, physical: false, limited: false, clip_length: 0.0 }
    }

    /// Concrete's run bank: no clip in its first list, the walk in its
    /// second, the run in its third, blended from 0.6 to 1.01.
    fn run_bank() -> Bank {
        bank(Some(vec![-0.2, 0.4, 0.6, 1.01]), [vec![], (0..6).collect(), (6..12).collect()])
    }

    #[test]
    fn a_footstep_chooses_its_list_by_its_blend_value() {
        let (bank, mut shuffle, mut rng) = (run_bank(), Shuffle::default(), Rng::new(1));
        // Slow: the first list is empty and the second's clip plays whole.
        let slow = choose(&bank, &mut shuffle, 0.3, &mut rng);
        assert!(slow.clips[0].is_none() && slow.clips[1].is_some_and(|c| c < 6));
        assert!((slow.share - inverse_lerp(0.4, -0.2, 0.3)).abs() < 1e-6);
        // Between 0.4 and 0.6, a walking clip alone.
        let walk = choose(&bank, &mut shuffle, 0.5, &mut rng);
        assert!(walk.clips[0].is_some_and(|c| c < 6) && walk.clips[1].is_none() && walk.share == 1.0);
        // Between 0.6 and 1.01, a walking and a running clip, the walk's
        // share falling.
        let both = choose(&bank, &mut shuffle, 0.9, &mut rng);
        assert!(both.clips[0].is_some_and(|c| c < 6) && both.clips[1].is_some_and(|c| c >= 6));
        assert!((both.share - (1.01 - 0.9) / 0.41).abs() < 1e-5);
        // The pitch within its spread.
        assert!((both.pitch - 1.0).abs() <= 0.15);
    }

    /// The shuffle gives every clip of a list once before any again, and
    /// two clips simply alternate (its swap never draws the last place).
    #[test]
    fn the_shuffle_goes_through_a_list_and_two_alternate() {
        let (mut shuffle, mut rng) = (Shuffle::default(), Rng::new(7));
        let mut seen: Vec<usize> = (0..6).map(|_| next_index(&mut shuffle, 6, &mut rng)).collect();
        seen.sort();
        assert_eq!(seen, (0..6).collect::<Vec<_>>());
        let mut two = Shuffle::default();
        let order: Vec<usize> = (0..6).map(|_| next_index(&mut two, 2, &mut rng)).collect();
        assert_eq!(order, [0, 1, 0, 1, 0, 1]);
    }

    fn moment(clock: f32, state: State) -> Moment {
        Moment {
            clock,
            dt: 1.0 / 120.0,
            state,
            footstep: false,
            crawl_marks: 0,
            landed: None,
            prone: false,
            pose_level: 1.0,
            smoothed_pose_level: 1.0,
            clamped_speed: 1.0,
            speed: 1.0,
            max_speed: 1.0,
            overweight: 0.0,
            walk_overweight: 0.0,
            leg_damaged: false,
            yaw: 0.0,
            hands_to_body: 0.0,
        }
    }

    /// A footstep at the top speed, unburdened: the run's 0.85 with the
    /// speed share of 1 (the first footstep: as the source was made), the
    /// bank's 0.68 less up to 0.05; and the kit 0.1 s later at 0.5 times
    /// the gear's 0.5.
    #[test]
    fn a_footstep_and_its_kit() {
        let mut sounds = StepSounds::default();
        sounds.set_figures(vec![(CONCRETE, std::array::from_fn(|_| Some(Figures { base_volume: 0.68, delta_volume: 0.05 })))]);
        let mut last = 0.0;
        sounds.update(&moment(1.0, State::Move), &mut last, &mut || Some(CONCRETE));
        let mut walking = moment(1.2, State::Move);
        walking.footstep = true;
        sounds.update(&walking, &mut last, &mut || Some(CONCRETE));
        let asked = sounds.take();
        assert_eq!(asked.len(), 1);
        let step = asked[0];
        assert_eq!(step.bank, BankOf::Surface(CONCRETE, RUN));
        assert!(step.volume <= 0.85 * 0.68 + 1e-6 && step.volume >= 0.85 * 0.63 - 1e-6, "{}", step.volume);
        assert_eq!(step.blend, 1.0);
        sounds.update(&moment(1.25, State::Move), &mut last, &mut || None);
        assert!(sounds.take().is_empty());
        sounds.update(&moment(1.31, State::Move), &mut last, &mut || None);
        let kit = sounds.take();
        assert_eq!(kit.len(), 1);
        assert!(kit[0].kit && kit[0].bank == BankOf::Kit(GEAR));
        assert!((kit[0].volume - 0.5 * 0.5).abs() < 1e-6, "{}", kit[0].volume);
    }

    /// Covert Movement at level 40, walking slowly enough to walk quietly
    /// in full: a step on a soft surface (concrete) 48% quieter and its
    /// blend with it, on a loud one (metal) 24% (`CovertMovementVolumeBy
    /// Speed`).
    #[test]
    fn covert_movement_quiets_a_quiet_step() {
        let mut levels = crate::skills::SkillLevels::default();
        levels.set_level(crate::skills::Skill::CovertMovement, 40);
        let covert = levels.buffs().covert;
        let step = |covert: Option<crate::skills::Covert>, surface: u8| {
            let mut sounds = StepSounds::default();
            if let Some(covert) = covert {
                sounds.set_covert(covert);
            }
            sounds.set_figures(vec![(surface, std::array::from_fn(|_| Some(Figures { base_volume: 0.68, delta_volume: 0.0 })))]);
            let mut last = 0.0;
            let slow = |clock: f32, footstep: bool| Moment { clamped_speed: 0.15, speed: 0.15, footstep, ..moment(clock, State::Move) };
            sounds.update(&slow(1.0, false), &mut last, &mut || Some(surface));
            sounds.update(&slow(1.2, true), &mut last, &mut || Some(surface));
            let asked = sounds.take();
            (asked[0].volume, asked[0].blend)
        };
        let ((plain, plain_blend), (quiet, quiet_blend)) = (step(None, CONCRETE), step(Some(covert), CONCRETE));
        assert!((quiet / plain - 0.52).abs() < 1e-5, "{quiet} against {plain}");
        assert!((quiet_blend / plain_blend - 0.52).abs() < 1e-5, "{quiet_blend} against {plain_blend}");
        let (loud_plain, loud) = (step(None, 1).0, step(Some(covert), 1).0);
        assert!((loud / loud_plain - 0.76).abs() < 1e-5, "{loud} against {loud_plain}");
    }

    /// A walk left with no footstep heard in it plays one and counts it;
    /// a sprint left for its stop plays the stop.
    #[test]
    fn leaving_a_walk_and_a_sprint() {
        let mut sounds = StepSounds::default();
        let mut last = 0.0;
        sounds.update(&moment(1.0, State::Move), &mut last, &mut || None);
        sounds.update(&moment(1.3, State::Idle), &mut last, &mut || None);
        let asked = sounds.take();
        assert!(asked.iter().any(|play| play.bank == BankOf::Surface(CONCRETE, RUN)));
        assert_eq!(last, 1.3);
        sounds.update(&moment(2.0, State::Sprint), &mut last, &mut || None);
        let mut step = moment(2.3, State::Sprint);
        step.footstep = true;
        sounds.update(&step, &mut last, &mut || None);
        sounds.take();
        sounds.update(&moment(2.6, State::SprintStop), &mut last, &mut || None);
        let asked = sounds.take();
        assert_eq!(asked.len(), 1);
        assert_eq!(asked[0].bank, BankOf::Surface(CONCRETE, STOP));
    }

    /// The landing: heard over 0.3 m, whole from 0.75 m; a turn to the
    /// right standing is heard, one to the left is not.
    #[test]
    fn a_landing_and_the_turns() {
        let mut sounds = StepSounds::default();
        let mut last = 0.0;
        sounds.update(&moment(0.5, State::Jump), &mut last, &mut || None);
        let mut landed = moment(1.0, State::Jump);
        landed.landed = Some((0.2, 0.0));
        sounds.update(&landed, &mut last, &mut || None);
        assert!(sounds.take().iter().all(|play| play.bank != BankOf::Surface(CONCRETE, LANDING)));
        landed.landed = Some((1.0, -0.5));
        landed.clock = 2.0;
        sounds.update(&landed, &mut last, &mut || None);
        let asked = sounds.take();
        let land = asked.iter().find(|play| play.bank == BankOf::Surface(CONCRETE, LANDING)).expect("a landing");
        assert!((land.volume - 1.0).abs() < 1e-6);
        for (sign, heard) in [(1.0, true), (-1.0, false)] {
            let mut sounds = StepSounds::default();
            for frame in 0..20 {
                let mut idle = moment(10.0 + frame as f32 / 120.0, State::Idle);
                idle.yaw = sign * frame as f32 * 3.0;
                idle.hands_to_body = if frame >= 10 { sign * 50.0 } else { 0.0 };
                sounds.update(&idle, &mut last, &mut || None);
            }
            let turn = sounds.take().into_iter().find(|play| play.bank == BankOf::Surface(CONCRETE, TURN)).expect("a turn tried");
            assert_eq!(turn.volume > 0.0, heard, "{sign}: {}", turn.volume);
        }
    }
}
