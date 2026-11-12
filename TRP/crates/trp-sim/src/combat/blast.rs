//! What a grenade's blast does to a character, the player's or any other's.
//! After the grenade spec, "The blast" (`ExplosionSharedMethods.AffectPlayersShared`):
//! the damage to each part through the lines from the blast to the
//! body's hit colliders, what its armour takes, the contusion, and how hard
//! the view is shaken. The host tells where the colliders stand and what in
//! its world a line meets; health takes the damage (`Health::apply_damage`,
//! `GrenadeFragment`) and the contusion (`Health::do_contusion`, the
//! player's alone: a bot is never contused), the armour its share
//! (`armor::Wearer::take_blast`). Its fragments: `fragments`.

use crate::condition::health::Part;

/// A grenade's figures for its blast (its item's): the greatest and least
/// distances (`MaxExplosionDistance`, `MinExplosionDistance`), its
/// strength, its contusion's least and greatest reach and its strength
/// (`Contusion`: x, y, z), its armour damage's nearest and farthest
/// distances and its most (`ArmorDistanceDistanceDamage`: x, y, z), and its
/// fragment round's penetration power (`CreateFragment`'s
/// `PenetrationPower`), which what a line goes through takes from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Explosive {
    pub max: f32,
    pub min: f32,
    pub strength: f32,
    pub contusion: [f32; 3],
    pub armor: [f32; 3],
    pub power: f32,
}

/// One of the body's hit colliders as the body stands (`BodyPartCollider`,
/// and an `ArmorPlateCollider` while its plate is in): its part, its
/// object's place, and its collider type and plate (0 for a body part's
/// own), which name the armour it damages.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HitPoint {
    pub part: Part,
    pub at: [f32; 3],
    pub collider: u8,
    pub plate: u16,
}

/// What a blast's line from the blast to a collider meets (`AffectPlayersShared`'s
/// ray through `HighPolyWithTerrainMask`, the world's solids and terrain,
/// never a body): stopped at a share of it by a thing the fragment round
/// does not go through, or through things whose levels, each plus 2, it
/// takes from the round's penetration (nought for a clear line).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Line {
    Stopped(f32),
    Through(f32),
}

impl Line {
    /// The rule for what a line meets, by the fragment round's penetration
    /// power: each ballistic thing met (its share along the line, its
    /// penetration chance and level) stops it where it has no chance or a
    /// level above the power; else one with a level adds that level plus 2.
    /// A stop is taken at the nearest stopping thing (`PROVISIONAL`: the
    /// game takes the first in its ray's buffer, whose order it does not
    /// fix; it changes only the wall's thickness for a contusion).
    pub fn of(met: impl IntoIterator<Item = (f32, f32, f32)>, power: f32) -> Line {
        let mut levels = 0.0;
        let mut stop: Option<f32> = None;
        for (share, chance, level) in met {
            if chance < f32::EPSILON || power < level {
                stop = Some(stop.map_or(share, |s| s.min(share)));
            } else if level >= f32::EPSILON {
                levels += level + 2.0;
            }
        }
        stop.map_or(Line::Through(levels), Line::Stopped)
    }
}

/// `CircularEaseIn`: `1 − √(1 − p²)`.
fn circular_ease_in(p: f32) -> f32 {
    1.0 - (1.0 - p * p).sqrt()
}

/// What a blast does to a character.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Blast {
    /// Each part a line reached and its damage, the largest of its
    /// colliders'.
    pub damage: Vec<(Part, f32)>,
    /// What its armour takes: for each collider type and plate a line
    /// reached within the armour curve's reach, the curve's damage at that
    /// collider's distance (a later collider of the same type and plate
    /// writing over an earlier's), given to every piece covering it
    /// (`ApplyExplosionDamageToArmor`).
    pub armor: Vec<(u8, u16, f32)>,
    /// A contusion's seconds, where one is made (the player's alone).
    pub contusion: Option<f32>,
    /// How hard the view is shaken (`GrenadeShake_CO`'s strength), 0 for
    /// not at all.
    pub shake: f32,
}

/// The engine's `Mathf.InverseLerp`.
fn inverse_lerp(a: f32, b: f32, value: f32) -> f32 {
    if a == b {
        0.0
    } else {
        ((value - a) / (b - a)).clamp(0.0, 1.0)
    }
}

/// A flash on a player (`ApplyLightAndSoundHealthEffects`; grenade spec, "A
/// flash"), the line from the blast to the head clear: its figures
/// (`Blindness`: whole within x, nothing past y, z the most seconds), the
/// player's distance from the blast (from where it stands), the blast's
/// place and the head's, and where it looks (a unit way). How hard it
/// strikes (`DoStun`'s and `DoBurnEyes`'s strength: the facing times the
/// distance's share), and the distance's share for the eyes' burn (whole
/// at the blast, nothing past y); none where it strikes with nothing.
pub fn flash(blindness: [f32; 3], far: f32, blast: [f32; 3], head: [f32; 3], look: [f32; 3]) -> Option<(f32, f32)> {
    let toward: [f32; 3] = std::array::from_fn(|k| blast[k] - head[k]);
    let length = distance(blast, head);
    let facing = if length > 1e-6 { ((toward[0] * look[0] + toward[1] * look[1] + toward[2] * look[2]) / length).clamp(0.0, 1.0) } else { 0.0 };
    let strength = inverse_lerp(blindness[1], blindness[0], far) * facing;
    (strength > 0.0).then(|| (strength, inverse_lerp(blindness[1], 0.0, far)))
}

fn distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

fn along(from: [f32; 3], to: [f32; 3], share: f32) -> [f32; 3] {
    std::array::from_fn(|k| from[k] + (to[k] - from[k]) * share)
}

/// The blast's place from the grenade's (`Grenade._defaultShift`: 8 cm up).
pub fn blast_point(grenade: [f32; 3]) -> [f32; 3] {
    blast_point_along(grenade, [0.0, 1.0, 0.0])
}

/// The blast's place from where it went off, 8 cm along a normal
/// (`Grenade.Explosion`'s shift): a thrown grenade's up, an explosive round's
/// the hit's normal (`ShotDelegate`: `HitNormal * 0.08`).
pub fn blast_point_along(at: [f32; 3], normal: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|k| at[k] + normal[k] * 0.08)
}

/// Whether a blast at `at` reaches a character standing at `feet` at all:
/// its feet within the explosive's farthest damage or its contusion's
/// (`blast` does nothing past both, whatever its colliders: a host may ask
/// this before it places them).
pub fn reaches(explosive: &Explosive, at: [f32; 3], feet: [f32; 3]) -> bool {
    let far = distance(feet, at);
    !(far > explosive.max && far > explosive.contusion[1])
}

/// A blast at `at` on a character standing at `feet` with its hit
/// colliders at `points` (`AffectPlayersShared`). `line(from, to)` is what
/// the line from one place to the other meets (`Line::of`): a line to a
/// collider stopped does it no damage, one through things does it the
/// share a circular ease-in of the penetration left gives
/// (`CircularEaseIn(InverseLerp(0, power, power − levels))`). `wall` is the
/// wall's absorption of a contusion (`WallContusionAbsorption`: thickness
/// taking it all, thickness taking none).
pub fn blast(explosive: &Explosive, at: [f32; 3], feet: [f32; 3], points: &[HitPoint], line: &dyn Fn([f32; 3], [f32; 3]) -> Line, wall: [f32; 2]) -> Blast {
    let far = distance(feet, at);
    let shake = inverse_lerp(explosive.max * 3.0, explosive.min / 2.0, far);
    let mut out = Blast { shake, ..Blast::default() };
    if !reaches(explosive, at, feet) {
        return out;
    }
    let mut reached = false;
    // The wall the chest's line was stopped by, as thick as from where the
    // line enters it to where the line back from the chest does (the last
    // chest collider's so stopped).
    let mut wall_thickness = None;
    let power = explosive.power;
    for point in points {
        let length = distance(point.at, at);
        let levels = match line(at, point.at) {
            Line::Stopped(share) => {
                if point.part == Part::Chest {
                    let forward = along(at, point.at, share);
                    // (A line back that meets nothing leaves the game's hit
                    // at the world's origin.)
                    let back = match line(point.at, at) {
                        Line::Stopped(share) => along(point.at, at, share),
                        Line::Through(_) => [0.0; 3],
                    };
                    wall_thickness = Some(distance(forward, back));
                }
                continue;
            }
            Line::Through(levels) => levels,
        };
        reached = true;
        if explosive.strength <= 0.0 {
            continue;
        }
        let left = (power - levels).clamp(0.0, power.max(0.0));
        let ease = circular_ease_in(inverse_lerp(0.0, power, left));
        // The armour's share by the collider's distance, apart from the
        // ease (`ArmorDistanceDistanceDamage`).
        let [near, far_armor, most] = explosive.armor;
        let armor = (1.0 - inverse_lerp(near, far_armor, length)) * most;
        if armor > 0.0 {
            match out.armor.iter_mut().find(|(collider, plate, _)| *collider == point.collider && *plate == point.plate) {
                Some((.., value)) => *value = armor,
                None => out.armor.push((point.collider, point.plate, armor)),
            }
        }
        let damage = inverse_lerp(explosive.max, explosive.min, length) * explosive.strength * ease;
        match out.damage.iter_mut().find(|(part, _)| *part == point.part) {
            Some((_, most)) => *most = most.max(damage),
            None => out.damage.push((point.part, damage)),
        }
    }
    if reached || wall_thickness.is_some() {
        let [near, reach, strength] = explosive.contusion;
        let mut time = inverse_lerp(reach, near, far) * strength;
        if time > 2.0 {
            if !reached {
                time *= inverse_lerp(wall[0], wall[1], wall_thickness.unwrap_or(0.0));
            }
            if time > 2.0 {
                out.contusion = Some(time);
            }
        }
    }
    out
}

#[cfg(test)]
mod flash_tests {
    use super::*;

    #[test]
    fn a_flash_strikes_by_facing_and_distance() {
        // The Zarya's 10, 20, 40: looking straight at it 5 m off, whole;
        // 15 m off, half, its share of the burn a quarter; facing away or
        // past 20 m, nothing.
        let zarya = [10.0, 20.0, 40.0];
        let head = [0.0, 1.6, 0.0];
        let at = |far: f32| [0.0, 1.6, far];
        assert_eq!(flash(zarya, 5.0, at(5.0), head, [0.0, 0.0, 1.0]), Some((1.0, 0.75)));
        let (strength, share) = flash(zarya, 15.0, at(15.0), head, [0.0, 0.0, 1.0]).expect("struck");
        assert!((strength - 0.5).abs() < 1e-6 && (share - 0.25).abs() < 1e-6);
        assert_eq!(flash(zarya, 5.0, at(5.0), head, [0.0, 0.0, -1.0]), None);
        assert_eq!(flash(zarya, 21.0, at(21.0), head, [0.0, 0.0, 1.0]), None);
        // Side on (90 degrees): nothing; 60 degrees off: half.
        assert_eq!(flash(zarya, 5.0, at(5.0), head, [1.0, 0.0, 0.0]), None);
        let off = 60f32.to_radians();
        let (strength, _) = flash(zarya, 5.0, at(5.0), head, [off.sin(), 0.0, off.cos()]).expect("struck");
        assert!((strength - 0.5).abs() < 1e-5);
    }

    /// Recorder 0.11.0's `courses3`, `1-flash` (Reserve): two Zaryas, each
    /// measured from the frame before the blast as recorded (the feet, the
    /// head's bone in the player's axes, its yaw, the movement's look), the
    /// blast 8 cm over the grenade's place; the game's stun's strength and
    /// the stun's and the flash's times as its health then held them.
    #[test]
    fn a_flash_strikes_as_the_recorded_zaryas_did() {
        let zarya = [10.0, 20.0, 40.0];
        // (grenade, feet, body yaw, head bone, look's yaw and pitch;
        // the game's strength, stun's and flash's seconds)
        let takes: [([f32; 3], [f32; 3], f32, [f32; 3], [f32; 2], [f32; 3]); 2] = [
            ([-8.900558, 0.024_138_928, -26.278_917], [-3.207_519_5, 0.0, -9.068_712], 198.394_78, [-0.037_695_84, 1.386_097_9, 0.262_259_36], [198.403_6, 5.993_741_5], [0.187_130_39, 7.685_215_5, 3.944_644_2]),
            ([-11.670_921, 0.024_137_02, -40.600_063], [-11.044_731, 0.062_436_58, -38.65408], 196.021_42, [0.071_404_25, 1.429_083_3, 0.378_970_53], [195.953_81, 36.362_457], [0.998_181_94, 40.127_277, 36.110_645]),
        ];
        for (grenade, feet, body, bone, [yaw, pitch], [strength, stun, burn]) in takes {
            let at = blast_point(grenade);
            let (s, c) = body.to_radians().sin_cos();
            let head = [feet[0] + bone[0] * c + bone[2] * s, feet[1] + bone[1], feet[2] - bone[0] * s + bone[2] * c];
            let (yaw, pitch) = (yaw.to_radians(), pitch.to_radians());
            let look = [yaw.sin() * pitch.cos(), -pitch.sin(), yaw.cos() * pitch.cos()];
            let (ours, share) = flash(zarya, distance(feet, at), at, head, look).expect("struck");
            assert!((ours - strength).abs() < 2e-6, "strength {ours}, the game's {strength}");
            assert!((0.2 + zarya[2] * ours - stun).abs() < 1e-4, "the stun's {} s, the game's {stun}", 0.2 + zarya[2] * ours);
            assert!((0.2 + zarya[2] * share - burn).abs() < 1e-4, "the flash's {} s, the game's {burn}", 0.2 + zarya[2] * share);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The F-1's figures (its item's; its fragment's power the F-1's
    /// round's 10, as the RGD-5's).
    const F1: Explosive = Explosive { max: 7.0, min: 3.0, strength: 80.0, contusion: [1.5, 4.0, 15.0], armor: [1.0, 4.0, 25.0], power: 10.0 };
    const WALL: [f32; 2] = [0.4, 0.01];

    fn body(feet: [f32; 3]) -> Vec<HitPoint> {
        let up = |part, height, collider| HitPoint { part, at: [feet[0], feet[1] + height, feet[2]], collider, plate: 0 };
        vec![up(Part::Head, 1.6, 0), up(Part::Chest, 1.3, 1), up(Part::Stomach, 1.0, 2), up(Part::LeftLeg, 0.5, 3), up(Part::RightLeg, 0.5, 4)]
    }

    fn clear(_: [f32; 3], _: [f32; 3]) -> Line {
        Line::Through(0.0)
    }

    #[test]
    fn near_a_blast_every_part_takes_its_strength_and_a_contusion() {
        let blast = blast(&F1, [0.0, 0.08, 0.0], [1.0, 0.0, 0.0], &body([1.0, 0.0, 0.0]), &clear, WALL);
        // Every collider within 3 m: the whole strength.
        assert_eq!(blast.damage.len(), 5);
        assert!(blast.damage.iter().all(|(_, damage)| (*damage - 80.0).abs() < 1e-4));
        // 1 m from the feet, inside the contusion's least reach: 15 s.
        assert_eq!(blast.contusion, Some(15.0));
        assert!((blast.shake - 1.0).abs() < 1e-6);
    }

    #[test]
    fn far_off_only_the_shake_and_past_the_greatest_distance_nothing() {
        let middle = blast(&F1, [0.0; 3], [5.0, 0.0, 0.0], &body([5.0, 0.0, 0.0]), &clear, WALL);
        // 5 m: the legs (5.02 m) a little over half way down from 3 to 7 m.
        let legs = middle.damage.iter().find(|(part, _)| *part == Part::LeftLeg).map(|(_, damage)| *damage).unwrap();
        assert!((legs - 80.0 * (7.0 - (25.0f32 + 0.25).sqrt()) / 4.0).abs() < 1e-3, "{legs}");
        assert_eq!(middle.contusion, None);
        let gone = blast(&F1, [0.0; 3], [10.0, 0.0, 0.0], &body([10.0, 0.0, 0.0]), &clear, WALL);
        assert!(gone.damage.is_empty() && gone.contusion.is_none());
        assert!((gone.shake - (21.0 - 10.0) / 19.5).abs() < 1e-5);
    }

    #[test]
    fn a_wall_stops_the_damage_and_takes_the_contusion_by_its_thickness() {
        // A wall across x from 0.5 to 0.55 m, as high as the body.
        let wall = |from: [f32; 3], to: [f32; 3]| -> Line {
            let (low, high) = (0.5f32, 0.55f32);
            let (a, b) = (from[0], to[0]);
            if (a - b).abs() < 1e-9 {
                return Line::Through(0.0);
            }
            let enter = if a < b { (low - a) / (b - a) } else { (high - a) / (b - a) };
            if enter > 0.0 && enter < 1.0 {
                Line::Stopped(enter)
            } else {
                Line::Through(0.0)
            }
        };
        let blast = blast(&F1, [0.0, 0.08, 0.0], [1.0, 0.0, 0.0], &body([1.0, 0.0, 0.0]), &wall, WALL);
        assert!(blast.damage.is_empty());
        // 5 cm of wall crossed slantwise by the chest's line (1 m across,
        // 1.22 m up): 15 s times inverseLerp(0.4, 0.01, its length in it).
        let thickness = 0.05 * (1.0f32 + 1.22 * 1.22).sqrt();
        let kept = (0.4 - thickness) / (0.4 - 0.01);
        let time = blast.contusion.expect("a contusion through the wall");
        assert!((time - 15.0 * kept).abs() < 1e-3, "{time}");
    }

    #[test]
    fn what_a_line_goes_through_eases_the_damage_and_the_armour_takes_its_curve() {
        // A thing of level 3 met by every line (the round's power 10): 5 of
        // it taken, half the power left, `1 − √(1 − 0.25)` of the damage.
        let thin = |_: [f32; 3], _: [f32; 3]| Line::of([(0.4, 1.0, 3.0)], 10.0);
        let blast = blast(&F1, [0.0, 0.08, 0.0], [1.0, 0.0, 0.0], &body([1.0, 0.0, 0.0]), &thin, WALL);
        let ease = 1.0 - (1.0f32 - 0.25).sqrt();
        assert!(blast.damage.iter().all(|(_, damage)| (*damage - 80.0 * ease).abs() < 1e-3), "{:?}", blast.damage);
        // The armour's curve by each collider's distance: whole within 1 m,
        // nothing past 4 m; the legs' 1.04 m from the blast.
        let legs = blast.armor.iter().find(|(collider, ..)| *collider == 3).map(|(.., value)| *value).unwrap();
        let far = ((1.0f32).powi(2) + 0.42f32.powi(2)).sqrt();
        assert!((legs - (1.0 - (far - 1.0) / 3.0) * 25.0).abs() < 1e-3, "{legs}");
        // Something above the power, or with no chance, stops the line.
        assert_eq!(Line::of([(0.7, 1.0, 12.0), (0.2, 1.0, 3.0)], 10.0), Line::Stopped(0.7));
        assert_eq!(Line::of([(0.5, 0.0, 1.0)], 10.0), Line::Stopped(0.5));
        // A level of nought goes through and takes nothing.
        assert_eq!(Line::of([(0.5, 1.0, 0.0)], 10.0), Line::Through(0.0));
    }
}
