//! A weapon's figures for spread and deviation, and the iron shot direction.

use super::*;

/// A weapon's figures for its shots, from its build (ballistics spec, "A
/// shot leaves the weapon"): its centre of impact (its barrel's, else its
/// own, times one less its parts' accuracy over 100), its speed factor (one
/// plus its own and its parts' velocity over 100), its shotgun dispersion
/// (its barrel's or its own, times the same accuracy), its iron sight range,
/// its default round and a revolver's double action penalty.
#[derive(Debug, Clone, PartialEq)]
pub struct WeaponFigures {
    pub centre_of_impact: f32,
    pub speed_factor: f32,
    pub shotgun_dispersion: f32,
    pub iron_sight_range: i32,
    pub default_round: Option<String>,
    /// The barrel's deviation by durability (`DeviationCurve`,
    /// `DeviationMax`): its barrel part's, else the weapon's own; a maximum
    /// of nought is 100.
    pub deviation_curve: f32,
    pub deviation_max: f32,
    /// The penalty a shot from a hammer left down takes
    /// (`DoubleActionAccuracyPenalty`, the weapon's own; nought where it has
    /// none), and its first stock's multiplier of it
    /// (`StockDoubleActionAccuracyPenaltyMult`: the first part of the class
    /// `Stock` in `Weapon.Mods`, 1 without one).
    pub double_action_penalty: f32,
    pub stock_double_action: f32,
}

impl WeaponFigures {
    /// From the weapon's template and its parts', every part in its build
    /// in the game's order (`Weapon.Mods`: every slot of the weapon itself,
    /// then each of those parts' own in the same way, a part's before the
    /// next's), without the rounds in its chambers.
    pub fn of(db: &trp_data::Database, weapon: &str, parts: &[&str]) -> Option<Self> {
        let template = db.items.get(weapon)?;
        let part_templates: Vec<&trp_data::setup::database::ItemTemplate> = parts.iter().filter_map(|p| db.items.get(*p)).collect();
        let barrel = part_templates.iter().find(|t| t.props.get("CenterOfImpact").is_some_and(|v| v.is_number()));
        let base = barrel.and_then(|b| b.prop_f32("CenterOfImpact")).or_else(|| template.prop_f32("CenterOfImpact")).unwrap_or(0.0);
        let accuracy: f32 = part_templates.iter().map(|t| t.prop_f32("Accuracy").unwrap_or(0.0)).sum();
        let delta = -accuracy / 100.0;
        let velocity: f32 = part_templates.iter().map(|t| t.prop_f32("Velocity").unwrap_or(0.0)).sum::<f32>() + template.prop_f32("Velocity").unwrap_or(0.0);
        let dispersion = barrel.and_then(|b| b.prop_f32("ShotgunDispersion")).or_else(|| template.prop_f32("ShotgunDispersion")).unwrap_or(0.0);
        let of_barrel = |name: &str| barrel.and_then(|b| b.prop_f32(name)).or_else(|| template.prop_f32(name));
        let deviation_max = of_barrel("DeviationMax").filter(|max| *max != 0.0).unwrap_or(100.0);
        let stock = part_templates.iter().find(|t| db.is_a(t, "Stock"));
        Some(Self {
            centre_of_impact: base * (1.0 + delta),
            speed_factor: 1.0 + velocity / 100.0,
            shotgun_dispersion: dispersion * (1.0 + delta),
            iron_sight_range: template.props.get("IronSightRange").and_then(|v| v.as_f64()).unwrap_or(0.0) as i32,
            default_round: template.props.get("defAmmo").and_then(|v| v.as_str()).map(str::to_owned),
            deviation_curve: of_barrel("DeviationCurve").unwrap_or(1.0),
            deviation_max,
            double_action_penalty: template.prop_f32("DoubleActionAccuracyPenalty").unwrap_or(0.0),
            stock_double_action: stock.and_then(|t| t.prop_f32("DoubleActionAccuracyPenaltyMult")).unwrap_or(1.0),
        })
    }

    /// An underbarrel launcher's, as its shot takes them (`LauncherShot`,
    /// `Launcher`): its ghost weapon's (`LinkedWeapon`) centre of impact alone,
    /// no part's accuracy in it; its speed by its own parts' and the ghost's
    /// `Velocity`; the barrel's deviation the ghost's curve with its own barrel
    /// part's or the ghost's greatest (a launcher has no durability: taken at
    /// 100); its shotgun spread by its own parts as a weapon's. None where its
    /// template names no ghost.
    pub fn of_launcher(db: &trp_data::Database, launcher: &str, parts: &[&str]) -> Option<Self> {
        let ghost_id = db.items.get(launcher)?.props.get("LinkedWeapon")?.as_str()?;
        let ghost = db.items.get(ghost_id)?;
        let mut figures = Self::of(db, ghost_id, parts)?;
        figures.centre_of_impact = ghost.prop_f32("CenterOfImpact").unwrap_or(0.0);
        figures.deviation_curve = ghost.prop_f32("DeviationCurve").unwrap_or(1.0);
        figures.double_action_penalty = 0.0;
        Some(figures)
    }

    /// The double action's penalty on a shot (`Shot`): with the hammer down
    /// as it is made, the weapon's penalty times one less the skill's share
    /// (`DoubleActionRecoilReduce`: its class's
    /// `WeaponDoubleActionRecoilReduceBuff`, 0.01 a level) times its stock's
    /// multiplier, in that order; nought with the hammer up.
    pub fn double_action(&self, hammer_down: bool, reduce: f32) -> f32 {
        if hammer_down {
            self.double_action_penalty * (1.0 - reduce) * self.stock_double_action
        } else {
            0.0
        }
    }

    /// The barrel's deviation at a durability (`Weapon.GetBarrelDeviation`):
    /// a quadratic Bézier from `DeviationMax` at nought through
    /// `DeviationCurve` to 1 at 100, found for its parameter by the
    /// durability (`GetBarrelDeviation(float)`; the weapon's own, without
    /// an argument, times the repair buff's `WeaponSpread`: `spread_worn`).
    pub fn barrel_deviation(&self, durability: f32) -> f32 {
        let curve = self.deviation_curve;
        let twice = 2.0 * curve;
        let t = if 100.0 - twice == 0.0 { durability / twice } else { (-curve + ((100.0 - twice) * durability + curve * curve).sqrt()) / (100.0 - twice) };
        let u = 1.0 - t;
        u * u * self.deviation_max + 2.0 * t * u * curve + t * t
    }

    /// The spread a shot is given (`Shot`): the centre of impact times the
    /// round's factor, the overheat's multiplier, the barrel's deviation
    /// and the repair buff's spread (each 1 for a new weapon without one).
    pub fn spread(&self, round: &Round) -> f32 {
        self.centre_of_impact * round.ammo_factor()
    }

    /// The same with the weapon's wear and heat and the double action's
    /// penalty (`double_action`, added to the centre of impact): its
    /// durability's barrel deviation and the overheat's multiplier (`heat`:
    /// 1 below `OverheatProblemsStart`; `crate::malfunction::spread_factor`),
    /// and a repair buff's spread (`buff`, 1 without one), multiplied in the
    /// game's order. The buff counts twice, as the game's `Shot` has it: in
    /// the weapon's barrel deviation (`Weapon.GetBarrelDeviation()`) and
    /// again on its own (`num4`), so the cone shrinks by its square.
    pub fn spread_worn(&self, round: &Round, durability: f32, heat: f32, double_action: f32, buff: f32) -> f32 {
        (self.centre_of_impact + double_action) * round.ammo_factor() * heat * (self.barrel_deviation(durability) * buff) * buff
    }
}

/// The local shot direction for iron sights (`CalculateShotDirectionForIron`):
/// `point` the place on the sights' line `range` out, less the muzzle, in
/// the weapon's own bone (`method_14`); the default round flown along x at
/// its speed times the weapon's, and its height where its distance, then
/// its x, first passes the range (the second search over a table that, the
/// flight restarted, holds each step one on: the game's).
pub fn iron_shot_direction(point: V3, round: &Round, speed_factor: f32, range: i32, gravity: V3) -> V3 {
    let speed = round.initial_speed * speed_factor;
    let mut flight = Trajectory::new([0.0; 3], [speed, 0.0, 0.0], round.mass_g, round.diameter_mm, round.ballistic_coefficient, gravity);
    let range = range as f32;
    let search = |flight: &mut Trajectory, shift: usize, by: &dyn Fn(V3) -> f32| -> Option<f32> {
        let (mut low, mut high) = (0i64, STEPS as i64 - 1);
        while low <= high {
            let mid = (low + high) / 2;
            if mid as usize + 1 + shift > STEPS {
                return None;
            }
            let a = flight.step(mid as usize + shift).position;
            let b = flight.step(mid as usize + 1 + shift).position;
            if by(a) < range && by(b) > range {
                return Some(a[1]);
            }
            if by(a) > range && by(b) > range {
                high = mid - 1;
            } else {
                low = mid + 1;
            }
        }
        None
    };
    let mut drop = search(&mut flight, 0, &|p| magnitude(p)).unwrap_or(0.0);
    if let Some(again) = search(&mut flight, 1, &|p| p[0]) {
        drop = again;
    }
    [0.0, -1.0, (point[1] - drop) / point[2]]
}

/// The shot's start and way moved for the wide field of view's squeeze
/// (`AdjustShotVectors`): each put into the hands' root's axes (`at`,
/// `turn`), its forward figure times the squeeze, and put back; the way made
/// a unit again.
pub fn squeezed(position: V3, direction: V3, at: V3, turn: [f32; 4], squeeze: f32) -> (V3, V3) {
    let back = [-turn[0], -turn[1], -turn[2], turn[3]];
    let mut p = crate::base::math::quat_rotate(back, sub(position, at));
    let mut d = crate::base::math::quat_rotate(back, direction);
    p[2] *= squeeze;
    d[2] *= squeeze;
    (add(at, crate::base::math::quat_rotate(turn, p)), normalized(crate::base::math::quat_rotate(turn, d)))
}

/// A shot's way with the weapon's spread (`Shot`): the way times 100 plus a
/// point within a ball (Unity's own generator) times the spread, made a unit.
pub fn spread(direction: V3, amount: f32, random: &mut UnityRandom) -> V3 {
    let ball = random.inside_unit_sphere();
    normalized(add(scale(direction, 100.0), scale(ball, amount)))
}
