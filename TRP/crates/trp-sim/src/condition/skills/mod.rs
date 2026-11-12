//! The character's skills: each skill's progress and what its level makes of the buffs.
//! After `docs/spec/skills.md`. A level is a hundred points, 0 to 51, the
//! last elite; the buffs are those the rest of the simulation reads, by the
//! game's own rules (`SkillManager`, `Skill.UpdateRules`). The levels are set by a
//! host (the app's options tab, or a host game at run time), and earned by
//! play (`crate::earning`).
//!
//! Every figure is 0, or what leaves its system as it was, at level 0: a
//! character with no levels is the untrained one the simulation always had.

mod catalogue;
mod levels;
mod buffs;
mod buff_table;

pub use catalogue::*;
pub use levels::*;
pub use buffs::*;
pub use buff_table::*;
use catalogue::{base, elite, or_elite, per_level, to_top};

/// The progress a level takes (`SkillExpPerLevel`), the most a skill holds
/// (`BaseSkill.ClampCurrent`), and the elite level.
pub const POINTS_PER_LEVEL: f32 = 100.0;
pub const MOST_PROGRESS: f32 = 5100.0;
pub const ELITE: u32 = 51;

/// The game's four kinds of skill, as its skill page groups them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Class {
    Physical,
    Mental,
    Combat,
    Practical,
}

impl Class {
    pub const ALL: [Self; 4] = [Self::Physical, Self::Mental, Self::Combat, Self::Practical];

    /// The game's name for it (`ESkillClass`).
    pub fn name(self) -> &'static str {
        match self {
            Self::Physical => "Physical",
            Self::Mental => "Mental",
            Self::Combat => "Combat",
            Self::Practical => "Practical",
        }
    }
}

#[cfg(test)]
mod tests;
