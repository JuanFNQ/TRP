//! The skill levels held and the covert movement they give.

use super::*;

/// The character's skills: each one's progress, 0 to 5,100.
#[derive(Debug, Clone, PartialEq)]
pub struct SkillLevels {
    pub(super) progress: [f32; 43],
}

impl Default for SkillLevels {
    fn default() -> Self {
        Self { progress: [0.0; 43] }
    }
}

impl SkillLevels {
    pub fn progress(&self, skill: Skill) -> f32 {
        self.progress[skill.index()]
    }

    /// A skill's progress, held between 0 and 5,100 (`ClampCurrent`). A
    /// fraction of a level is kept: it is what was earned toward the next.
    pub fn set_progress(&mut self, skill: Skill, progress: f32) {
        self.progress[skill.index()] = if progress.is_finite() { progress.clamp(0.0, MOST_PROGRESS) } else { 0.0 };
    }

    /// The level, whole (`BaseSkill.Level`), 0 to 51.
    pub fn level(&self, skill: Skill) -> u32 {
        ((self.progress(skill) / POINTS_PER_LEVEL) as u32).min(ELITE)
    }

    /// Sets a skill to a whole level, 0 to 51 (`Skill.SetLevel`).
    pub fn set_level(&mut self, skill: Skill, level: u32) {
        self.set_progress(skill, level.min(ELITE) as f32 * POINTS_PER_LEVEL);
    }

    /// A level by the game's name of the skill, 0 for a name it has not.
    pub fn level_of(&self, name: &str) -> u32 {
        Skill::of_name(name).map_or(0, |skill| self.level(skill))
    }

    /// Every skill at the one whole level (the dummies' level 1: body hits
    /// spec, "The dummy on the range").
    pub fn all_at(level: u32) -> Self {
        let mut levels = Self::default();
        for skill in Skill::ALL {
            levels.set_level(skill, level);
        }
        levels
    }

    /// Every skill at level 0.
    pub fn untrained(&self) -> bool {
        self.progress.iter().all(|p| *p == 0.0)
    }

    /// What the levels make of every buff.
    pub fn buffs(&self) -> Buffs {
        Buffs::of(self)
    }

    /// A skill's summary level with stimulators' buffs at work on it
    /// (`BaseSkill.SummaryLevel`, `Skill.Buff`; health spec, "Buffs"): its
    /// whole level and each buff's value, each held to what takes the level
    /// to between nought and 60 (the level rounded up) and rounded, at most
    /// 60 where they add up above nought and 51 otherwise. A buff can so make
    /// a skill elite while it lasts.
    pub fn summary(&self, skill: Skill, boosts: &[(Skill, f32)]) -> u32 {
        let level = self.level(skill) as i32;
        let up = (self.progress(skill) / POINTS_PER_LEVEL).ceil() as i32;
        let buff: i32 = boosts.iter().filter(|(s, _)| *s == skill).map(|(_, v)| v.clamp(-up as f32, (60 - up) as f32).round() as i32).sum();
        let cap = if buff > 0 { 60 } else { ELITE as i32 };
        (level + buff).clamp(0, cap) as u32
    }

    /// What the summary levels make of every buff.
    pub fn buffs_with(&self, boosts: &[(Skill, f32)]) -> Buffs {
        Buffs::of_fn(|skill| self.summary(skill, boosts))
    }
}

/// Covert Movement's buffs (`MovementContext.CovertMovementVolume`,
/// `CovertMovementVolumeBySpeed`, `CovertEquipmentNoise`,
/// `UpdateCovertEfficiency`; footsteps spec).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Covert {
    /// A step on a soft surface quieter by it, times how quietly one walks
    /// (`CovertMovementSoundVolume`, 0.012 a level).
    pub soft: f32,
    /// The same on any other (`CovertMovementLoud`, 0.006 a level, 0.6 at
    /// elite).
    pub loud: f32,
    /// The kit's noise less it (`CovertMovementEquipment`, 0.012 a level).
    pub equipment: f32,
    /// How slowly one must walk to walk quietly (`CovertMovementSpeed`,
    /// 1 + 0.01 a level).
    pub speed: f32,
}

impl Default for Covert {
    fn default() -> Self {
        Self { soft: 0.0, loud: 0.0, equipment: 0.0, speed: 1.0 }
    }
}

impl Covert {
    /// How quietly one walks, 0 to 1 (`UpdateCovertEfficiency`): in full
    /// below `0.4 × speed − 0.2` of the top speed, not at all above
    /// `0.4 × speed`; none with a leg damaged, less by the walk's
    /// overweight.
    pub fn efficiency(&self, speed_share: f32, leg_damaged: bool, walk_overweight: f32) -> f32 {
        if leg_damaged {
            return 0.0;
        }
        let top = 0.4 * self.speed;
        crate::base::math::inverse_lerp(top, top - 0.2, speed_share) * (1.0 - walk_overweight)
    }

    /// A step's factor (`CovertMovementVolumeBySpeed`): 1 untrained.
    pub fn step_volume(&self, soft_surface: bool, efficiency: f32) -> f32 {
        1.0 - if soft_surface { self.soft } else { self.loud } * efficiency
    }

    /// The other sounds' factor (`CovertMovementVolume`): 1 untrained.
    pub fn volume(&self, soft_surface: bool) -> f32 {
        1.0 - if soft_surface { self.soft } else { self.loud }
    }
}
