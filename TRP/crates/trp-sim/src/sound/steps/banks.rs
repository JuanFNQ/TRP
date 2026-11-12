//! Step sound kinds, the banks they come from and a sound to play.


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

    pub(super) fn name(self) -> &'static str {
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
