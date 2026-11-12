//! The step settings, surface checks and the kit's figures.

use super::*;

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
    /// How far from the listener a character seen from outside has the
    /// ground under it looked up, by `SURFACE_CHECKS`' order (a step's, a
    /// sprint's, a landing's, a crawl's): the class's 40, 60, 40 and 30 m,
    /// widened to the longest of every surface's bank of the kind
    /// (`_runSurfaceCheck` and the rest, `widen_surface_checks`); beyond it
    /// and a metre (`CHECK_RANGE_BUFF`) no ground is looked up and the
    /// sound is not played (`CheckSurface`, `IsOutOfProtagonistsRange`).
    pub surface_checks: [f32; 4],
}

/// The banks whose rolloffs widen the surface checks, in their order: a
/// step's, a sprint's, a landing's and a crawl's (`SURFACE_BANKS`' places).
pub const SURFACE_CHECKS: [usize; 4] = [RUN, SPRINT, LANDING, PRONE];

/// The class's own surface checks, before any bank widens them.
const SURFACE_CHECKS_LEAST: [f32; 4] = [40.0, 60.0, 40.0, 30.0];

/// How far past a surface check the listener may be (`CHECK_RANGE_BUFF`).
pub(super) const CHECK_BUFFER: f32 = 1.0;

impl Default for StepSettings {
    /// The game's own figures where the install gives none (the hard
    /// settings' as read, `globals.json`'s sound radius, and
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
            surface_checks: SURFACE_CHECKS_LEAST,
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
            surface_checks: SURFACE_CHECKS_LEAST,
        })
    }

    /// The surface checks widened by the banks' rolloffs: for each kind, the
    /// longest of that bank over every surface (`Player.Init`, over the
    /// surface sets).
    pub fn widen_surface_checks(&mut self, rolloffs: impl IntoIterator<Item = (usize, f32)>) {
        for (bank, rolloff) in rolloffs {
            if let Some(at) = SURFACE_CHECKS.iter().position(|&b| b == bank) {
                self.surface_checks[at] = self.surface_checks[at].max(rolloff);
            }
        }
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
