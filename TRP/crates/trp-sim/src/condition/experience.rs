//! The character's experience and level, and what the game counts as experience in a raid.
//! After `docs/spec/skills.md`, "The character's experience": the install's table of what each level takes
//! (`exp.level.exp_table`, `ProfileInfo.GetLevel`, `GetExperience`), and
//! what the game counts as experience in a raid, by the install's figures
//! (`exp`: `heal`, `kill`, the locked doors'; `BaseStatisticsManager`,
//! `HealthStatisticsManager`). What is counted when, and the notices told
//! of it, are the app's to follow (`trp-app`'s `experience`).

use serde_json::Value;

/// What each level takes over the one before (`exp_table`): the first
/// entry is level 1's (nought), the last the top level's.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LevelTable {
    steps: Vec<i64>,
}

impl LevelTable {
    /// The install's table (`exp.level.exp_table`, each `{ "exp": n }`).
    pub fn read(config: &Value) -> Self {
        let steps = config["exp"]["level"]["exp_table"].as_array().into_iter().flatten().map(|step| step["exp"].as_i64().unwrap_or(0)).collect();
        Self { steps }
    }

    pub fn of(steps: Vec<i64>) -> Self {
        Self { steps }
    }

    /// The top level: as many as the table has (`GetLevel`'s last).
    pub fn top(&self) -> u32 {
        self.steps.len().max(1) as u32
    }

    /// The level so much experience makes (`ProfileInfo.GetLevel`): the
    /// table summed entry by entry until the sum passes the experience, the
    /// level the number of entries summed before; the top level once all
    /// are summed.
    pub fn level(&self, experience: i64) -> u32 {
        let mut sum = 0;
        for (at, step) in self.steps.iter().enumerate() {
            sum += step;
            if experience < sum {
                return at as u32;
            }
        }
        self.steps.len() as u32
    }

    /// The experience a level begins at (`ProfileInfo.GetExperience`): the
    /// table summed up to the level's own entry (level 1 at nought).
    pub fn experience_at(&self, level: u32) -> i64 {
        let mut sum = 0;
        for (at, step) in self.steps.iter().enumerate() {
            sum += step;
            if at + 1 == level as usize {
                break;
            }
        }
        sum
    }
}

/// The install's experience figures that a raid's counting reads
/// (`GlobalConfiguration.Experience`).
#[derive(Debug, Clone, PartialEq)]
pub struct ExpFigures {
    /// A point of health restored (`heal.expForHeal`), of energy and of
    /// hydration gained (`expForEnergy`, `expForHydration`).
    pub heal: f32,
    pub energy: f32,
    pub hydration: f32,
    /// A bot killed whose own reward is none (`kill.victimBotLevelExp`),
    /// the share more for a bot's head (`botHeadShotMult`), the streak's
    /// percent by the kills of the raid (`combo`), what a heavy hit on a
    /// bot gives for its whole health (`botExpOnDamageAllHealth`).
    pub victim_bot: i32,
    pub bot_head: f32,
    pub combo: Vec<i32>,
    pub bot_damage_all_health: f32,
    /// A PMC killed (`victimLevelExp`), its head's share more
    /// (`pmcHeadShotMult`), a heavy hit's for its whole health
    /// (`pmcExpOnDamageAllHealth`).
    pub victim_pmc: i32,
    pub pmc_head: f32,
    pub pmc_damage_all_health: f32,
    /// A locked door opened with its key (`expForLockedDoorOpen`) and
    /// breached (`expForLockedDoorBreach`).
    pub door_open: i32,
    pub door_breach: i32,
}

impl ExpFigures {
    pub fn read(config: &Value) -> Self {
        let exp = &config["exp"];
        let f = |v: &Value| v.as_f64().unwrap_or(0.0) as f32;
        let i = |v: &Value| v.as_i64().unwrap_or(0) as i32;
        Self {
            heal: f(&exp["heal"]["expForHeal"]),
            energy: f(&exp["heal"]["expForEnergy"]),
            hydration: f(&exp["heal"]["expForHydration"]),
            victim_bot: i(&exp["kill"]["victimBotLevelExp"]),
            bot_head: f(&exp["kill"]["botHeadShotMult"]),
            combo: exp["kill"]["combo"].as_array().into_iter().flatten().map(|step| i(&step["percent"])).collect(),
            bot_damage_all_health: f(&exp["kill"]["botExpOnDamageAllHealth"]),
            victim_pmc: i(&exp["kill"]["victimLevelExp"]),
            pmc_head: f(&exp["kill"]["pmcHeadShotMult"]),
            pmc_damage_all_health: f(&exp["kill"]["pmcExpOnDamageAllHealth"]),
            door_open: i(&exp["expForLockedDoorOpen"]),
            door_breach: i(&exp["expForLockedDoorBreach"]),
        }
    }

    /// A bot killed (`BaseStatisticsManager.OnEnemyKill`, a Scav's side):
    /// its own reward, or `victimBotLevelExp` where it has none (below
    /// nought, as the install's Scavs: -1); the head's share of that more,
    /// cut to a whole (`(int)(exp × mult)`); the streak's percent of it by
    /// how many the raid has killed with this one (`GetKillingBonusPercent`:
    /// the `killed - 1`th of `combo`, held to its last), cut. Base, head,
    /// streak.
    pub fn bot_kill(&self, reward: i32, head: bool, killed: u32) -> (i32, i32, i32) {
        let base = if reward < 0 { self.victim_bot } else { reward };
        self.kill_parts(base, if head { self.bot_head } else { 0.0 }, killed)
    }

    /// A PMC killed (`OnEnemyKill`, a USEC's or a BEAR's side): always
    /// `victimLevelExp`, whatever its level; the head's and the streak's
    /// as a bot's, with `pmcHeadShotMult`.
    pub fn pmc_kill(&self, head: bool, killed: u32) -> (i32, i32, i32) {
        self.kill_parts(self.victim_pmc, if head { self.pmc_head } else { 0.0 }, killed)
    }

    fn kill_parts(&self, base: i32, head_mult: f32, killed: u32) -> (i32, i32, i32) {
        let head = (base as f32 * head_mult) as i32;
        let at = (killed.max(1) as usize - 1).min(self.combo.len().saturating_sub(1));
        let percent = self.combo.get(at).copied().unwrap_or(0) as f32 / 100.0;
        (base, head, (base as f32 * percent) as i32)
    }

    /// A heavy hit on a bot (`OnEnemyDamage`): `botExpOnDamageAllHealth`
    /// times the damage over its whole health, cut. A hit is heavy where
    /// its body damage is at least 0.6 of the part's most and the part
    /// bleeds (`Player.ApplyDamageInfo`'s `isHeavyDamage`).
    pub fn bot_damage(&self, damage: f32, whole_health: f32) -> i32 {
        if whole_health <= 0.0 {
            return 0;
        }
        (self.bot_damage_all_health * damage / whole_health) as i32
    }

    /// The same for a PMC (`pmcExpOnDamageAllHealth`).
    pub fn pmc_damage(&self, damage: f32, whole_health: f32) -> i32 {
        if whole_health <= 0.0 {
            return 0;
        }
        (self.pmc_damage_all_health * damage / whole_health) as i32
    }
}

/// Whether a hit on a bot is heavy (`Player.ApplyDamageInfo`): its body
/// damage at least 0.6 of the part's most, and the part bleeding.
pub fn heavy_hit(body_damage: f32, part_most: f32, bleeding: bool) -> bool {
    part_most > 0.0 && body_damage / part_most >= 0.6 && bleeding
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table() -> LevelTable {
        LevelTable::of(vec![0, 1000, 3017, 4415])
    }

    /// As the game's: level 1 from nought, 2 from 1,000, 3 from 4,017, the
    /// top (4, the table's length) from 8,432 on.
    #[test]
    fn a_level_is_the_table_summed() {
        let t = table();
        assert_eq!((t.level(0), t.level(999), t.level(1000), t.level(4016), t.level(4017), t.level(8431), t.level(8432), t.level(1_000_000)), (1, 1, 2, 2, 3, 3, 4, 4));
        assert_eq!((t.experience_at(1), t.experience_at(2), t.experience_at(3), t.experience_at(4)), (0, 1000, 4017, 8432));
        assert_eq!(t.top(), 4);
        for level in 1..=4 {
            assert_eq!(t.level(t.experience_at(level)), level);
        }
    }

    /// The install's Scav: 80 a kill, 88 more for the head, the streak's
    /// percent from the second kill.
    #[test]
    fn a_bot_kill_counts_its_parts() {
        let figures = ExpFigures { heal: 1.0, energy: 1.0, hydration: 1.0, victim_bot: 80, bot_head: 1.1, combo: vec![0, 20, 30], bot_damage_all_health: 30.0, victim_pmc: 175, pmc_head: 1.2, pmc_damage_all_health: 50.0, door_open: 30, door_breach: 20 };
        assert_eq!(figures.bot_kill(-1, false, 1), (80, 0, 0));
        assert_eq!(figures.bot_kill(-1, true, 2), (80, 88, 16));
        assert_eq!(figures.bot_kill(-1, false, 9), (80, 0, 24));
        assert_eq!(figures.bot_damage(60.0, 440.0), 4);
        assert!(heavy_hit(30.0, 40.0, true) && !heavy_hit(30.0, 40.0, false) && !heavy_hit(20.0, 40.0, true));
        // The range's dummies are PMCs: 175, 210 for the head, the streak.
        assert_eq!(figures.pmc_kill(true, 1), (175, 210, 0));
        assert_eq!(figures.pmc_kill(false, 3), (175, 0, 52));
        assert_eq!(figures.pmc_damage(88.0, 440.0), 10);
    }
}
