//! Weapon mastering: the weapons' families, their points and the level the game makes of them.
//! After `docs/spec/skills.md`, "Weapon mastering": the install's families of weapons (`config.Mastering`), each family's points
//! and the level the game makes of them (`Mastering`, a `BaseSkill`), and
//! which family a weapon's level is read from (`SkillManager.GetMastering`).
//! The points are set by a host (the app's options tab, or a host game in
//! play), and earned by hits (`crate::earning::mastering_points`).
//!
//! The level is 0, 1 or 2, as the code counts it; every screen of the game
//! shows it and one ("Level: 1" to "Level: 3").

use serde_json::Value as Json;

/// The most points a family holds (`BaseSkill.ClampCurrent`, as a skill).
pub const MOST_POINTS: f32 = 5100.0;
/// The highest level (`Mastering.GetLevelForValue`).
pub const TOP_LEVEL: u32 = 2;
/// What the inspect window's row is out of for a weapon whose family is
/// not begun (`MasteringCharacteristicPanel.MaxVal`).
pub const UNBEGUN_TAKES: u32 = 1500;
/// What a level adds to the walk's cap with the sights up
/// (`SkillManager.GetWeaponInfo`: `AimMovementSpeed`, the level times it;
/// `SetAimingSlowdown(IsAiming, 0.33 + it)`).
pub const AIM_WALK_PER_LEVEL: f32 = 0.05;

/// A family of weapons (`GlobalConfiguration.MasteringGroup`): its name,
/// its weapons by template, and the points to the second level shown
/// (`Level2`, the code's level 1) and then to the third (`Level3`).
#[derive(Debug, Clone, PartialEq)]
pub struct Family {
    pub name: String,
    pub weapons: Vec<String>,
    pub level2: u32,
    pub level3: u32,
}

impl Family {
    /// The level so many points make (`GetLevelForValue`): 1 from
    /// `Level2`, 2 from `Level2 + Level3`.
    pub fn level(&self, points: f32) -> u32 {
        let points = clamp_points(points);
        if points < self.level2 as f32 {
            0
        } else if points < (self.level2 + self.level3) as f32 {
            1
        } else {
            TOP_LEVEL
        }
    }

    /// The share into the level (`LevelProgress`): below `Level2` the
    /// points over it; from it, the points past it over `Level3`, held at
    /// 1, so the top level is full.
    pub fn share(&self, points: f32) -> f32 {
        if points < self.level2 as f32 {
            points / (self.level2.max(1) as f32)
        } else {
            ((points - self.level2 as f32) / self.level3.max(1) as f32).clamp(0.0, 1.0)
        }
    }

    /// What the level takes (`LevelingThreshold`): `Level2` at level 0,
    /// `Level3` above.
    pub fn takes(&self, level: u32) -> u32 {
        if level == 0 {
            self.level2
        } else {
            self.level3
        }
    }

    /// A weapon of this family as the screens show it at so many points.
    pub fn shown(&self, points: f32) -> Shown {
        let level = self.level(points);
        let takes = self.takes(level);
        let share = self.share(clamp_points(points));
        Shown { level, into: share * takes as f32, takes, share, begun: true }
    }

    /// The points a level begins at: 0, `Level2`, `Level2 + Level3`.
    pub fn points_at(&self, level: u32) -> f32 {
        match level.min(TOP_LEVEL) {
            0 => 0.0,
            1 => self.level2 as f32,
            _ => (self.level2 + self.level3) as f32,
        }
    }
}

/// Points held between 0 and 5,100; a number that is none, 0.
fn clamp_points(points: f32) -> f32 {
    if points.is_finite() {
        points.clamp(0.0, MOST_POINTS)
    } else {
        0.0
    }
}

/// Every family of the install, in its order.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Families {
    list: Vec<Family>,
}

impl Families {
    /// The families of the install's `config` (`globals.json`'s
    /// `config.Mastering`); none where it has none.
    pub fn of_config(config: &Json) -> Self {
        let list = config
            .get("Mastering")
            .and_then(Json::as_array)
            .into_iter()
            .flatten()
            .filter_map(|group| {
                let number = |key: &str| group.get(key).and_then(Json::as_u64).map(|n| n as u32);
                Some(Family {
                    name: group.get("Name")?.as_str()?.to_owned(),
                    weapons: group.get("Templates")?.as_array()?.iter().filter_map(|t| t.as_str().map(str::to_owned)).collect(),
                    level2: number("Level2")?,
                    level3: number("Level3")?,
                })
            })
            .collect();
        Self { list }
    }

    pub fn all(&self) -> &[Family] {
        &self.list
    }

    pub fn by_name(&self, name: &str) -> Option<&Family> {
        self.list.iter().find(|family| family.name == name)
    }

    /// A weapon's family: the first that has it (`Associations`, which
    /// keeps the first where two share a weapon); none for a weapon no
    /// family has.
    pub fn of_weapon(&self, template: &str) -> Option<&Family> {
        self.list.iter().find(|family| family.weapons.iter().any(|weapon| weapon == template))
    }
}

/// The character's mastering: each begun family's points, in the order
/// begun (`SkillManager.Mastering`; the profile's `Skills.Mastering`). A
/// family is begun at 0 when a weapon of it is first drawn
/// (`OnWeaponDraw`) or with points when they are set
/// (`ChangeMasteringLevel`), and stays begun.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MasteringLevels {
    begun: Vec<(String, f32)>,
}

/// A weapon's mastering as the screens show it: its level (0 to 2), the
/// points into the level and what it takes (`LevelProgress` times
/// `LevelingThreshold`), the share, and whether its family is begun.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shown {
    pub level: u32,
    pub into: f32,
    pub takes: u32,
    pub share: f32,
    pub begun: bool,
}

impl MasteringLevels {
    /// Each begun family's name and points, in the order begun.
    pub fn begun(&self) -> &[(String, f32)] {
        &self.begun
    }

    /// A family's points, none where it is not begun.
    pub fn points(&self, family: &str) -> Option<f32> {
        self.begun.iter().find(|(name, _)| name == family).map(|(_, points)| *points)
    }

    /// Begins a family at 0 points if it is not (`MasterNewWeapon`, as a
    /// weapon of it is first drawn); whether it was begun now.
    pub fn begin(&mut self, family: &str) -> bool {
        if self.points(family).is_some() {
            return false;
        }
        self.begun.push((family.to_owned(), 0.0));
        true
    }

    /// Sets a family's points, held between 0 and 5,100, beginning it
    /// where it is not (`ChangeMasteringLevel`).
    pub fn set_points(&mut self, family: &str, points: f32) {
        let points = clamp_points(points);
        match self.begun.iter_mut().find(|(name, _)| name == family) {
            Some((_, kept)) => *kept = points,
            None => self.begun.push((family.to_owned(), points)),
        }
    }

    /// Sets a family to the start of a level, 0 to 2.
    pub fn set_level(&mut self, family: &Family, level: u32) {
        self.set_points(&family.name, family.points_at(level));
    }

    /// A family's level, 0 where it is not begun.
    pub fn level(&self, family: &Family) -> u32 {
        self.points(&family.name).map_or(0, |points| family.level(points))
    }

    /// Every begun family back to 0 points; they stay begun.
    pub fn all_to_nought(&mut self) {
        for (_, points) in &mut self.begun {
            *points = 0.0;
        }
    }

    /// Whether every begun family is at 0.
    pub fn untrained(&self) -> bool {
        self.begun.iter().all(|(_, points)| *points == 0.0)
    }

    /// A weapon's begun family and its points (`GetMastering`: the first
    /// begun family whose weapons have it).
    pub fn of_weapon<'a>(&self, families: &'a Families, template: &str) -> Option<(&'a Family, f32)> {
        self.begun.iter().find_map(|(name, points)| {
            let family = families.by_name(name)?;
            family.weapons.iter().any(|weapon| weapon == template).then_some((family, *points))
        })
    }

    /// A weapon's level (`CurrentMasteringLevel`): its begun family's, 0
    /// where none is.
    pub fn level_of(&self, families: &Families, template: &str) -> u32 {
        self.of_weapon(families, template).map_or(0, |(family, points)| family.level(points))
    }

    /// A weapon's mastering as the screens show it; a weapon whose family
    /// is not begun at level 0, nothing into it, out of 1,500.
    pub fn shown(&self, families: &Families, template: &str) -> Shown {
        match self.of_weapon(families, template) {
            Some((family, points)) => family.shown(points),
            None => Shown { level: 0, into: 0.0, takes: UNBEGUN_TAKES, share: 0.0, begun: false },
        }
    }
}

/// What the walk's cap with the sights up gains at a level (0.05 a level).
pub fn aim_walk_bonus(level: u32) -> f32 {
    level.min(TOP_LEVEL) as f32 * AIM_WALK_PER_LEVEL
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m4() -> Family {
        Family { name: "M4".to_owned(), weapons: vec!["m4a1".to_owned(), "adar".to_owned()], level2: 1600, level3: 2000 }
    }

    #[test]
    fn levels_and_shares_as_the_code_makes_them() {
        let m4 = m4();
        assert_eq!((m4.level(0.0), m4.level(1599.9), m4.level(1600.0), m4.level(3599.0), m4.level(3600.0), m4.level(9000.0)), (0, 0, 1, 1, 2, 2));
        assert_eq!(m4.share(800.0), 0.5);
        assert_eq!(m4.share(2600.0), 0.5);
        // The top level is full however far past it.
        assert_eq!(m4.share(3600.0), 1.0);
        assert_eq!(m4.share(5100.0), 1.0);
        assert_eq!((m4.takes(0), m4.takes(1), m4.takes(2)), (1600, 2000, 2000));
        assert_eq!((m4.points_at(0), m4.points_at(1), m4.points_at(2), m4.points_at(7)), (0.0, 1600.0, 3600.0, 3600.0));
    }

    #[test]
    fn a_weapon_reads_its_begun_family() {
        let families = Families { list: vec![m4(), Family { name: "AKM".to_owned(), weapons: vec!["akm".to_owned()], level2: 500, level3: 600 }] };
        let mut levels = MasteringLevels::default();
        // Not begun: level 0, out of 1,500.
        assert_eq!(levels.level_of(&families, "adar"), 0);
        assert_eq!(levels.shown(&families, "adar"), Shown { level: 0, into: 0.0, takes: 1500, share: 0.0, begun: false });
        assert!(levels.begin("M4"));
        assert!(!levels.begin("M4"));
        assert_eq!(levels.shown(&families, "adar"), Shown { level: 0, into: 0.0, takes: 1600, share: 0.0, begun: true });
        levels.set_level(families.by_name("AKM").unwrap(), 2);
        levels.set_points("M4", 2100.0);
        assert_eq!((levels.level_of(&families, "m4a1"), levels.level_of(&families, "akm"), levels.level_of(&families, "nothing")), (1, 2, 0));
        assert_eq!(levels.shown(&families, "m4a1").into, 500.0);
        // In the order begun; set back to nought, they stay begun.
        assert_eq!(levels.begun().iter().map(|(name, _)| name.as_str()).collect::<Vec<_>>(), ["M4", "AKM"]);
        levels.all_to_nought();
        assert!(levels.untrained());
        assert_eq!(levels.begun().len(), 2);
        levels.set_points("AKM", f32::NAN);
        assert_eq!(levels.points("AKM"), Some(0.0));
        levels.set_points("AKM", 99999.0);
        assert_eq!(levels.points("AKM"), Some(MOST_POINTS));
    }
}
