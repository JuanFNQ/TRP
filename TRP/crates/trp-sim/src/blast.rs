//! What a grenade's blast does to the character (grenade spec, "The
//! blast"; `ExplosionSharedMethods.AffectPlayersShared`): the damage to
//! each part through the lines from the blast to the body's hit colliders,
//! the contusion, and how hard the view is shaken. The host tells where the
//! colliders stand and what in its world a line meets; health takes the
//! damage (`Health::apply_damage`, `GrenadeFragment`) and the contusion
//! (`Health::do_contusion`).

use crate::health::Part;

/// A grenade's figures for its blast (its item's): the greatest and least
/// distances (`MaxExplosionDistance`, `MinExplosionDistance`), its
/// strength, and its contusion's least and greatest reach and its strength
/// (`Contusion`: x, y, z).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Explosive {
    pub max: f32,
    pub min: f32,
    pub strength: f32,
    pub contusion: [f32; 3],
}

/// One of the body's hit colliders as the body stands (`BodyPartCollider`):
/// its part and its object's place.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HitPoint {
    pub part: Part,
    pub at: [f32; 3],
}

/// What a blast does to the character.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Blast {
    /// Each part a line reached and its damage, the largest of its
    /// colliders'.
    pub damage: Vec<(Part, f32)>,
    /// A contusion's seconds, where one is made.
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
    [grenade[0], grenade[1] + 0.08, grenade[2]]
}

/// A blast at `at` on a character standing at `feet` with its hit
/// colliders at `points` (`AffectPlayersShared`). `entered(from, to)` is how
/// far along the line from one place to the other the first solid of the
/// world is met, as a share of it, if any: the line to a collider stopped so
/// does it no damage (the host's world has nothing a fragment pierces).
/// `wall` is the wall's absorption of a contusion (`WallContusionAbsorption`:
/// thickness taking it all, thickness taking none).
pub fn blast(explosive: &Explosive, at: [f32; 3], feet: [f32; 3], points: &[HitPoint], entered: &dyn Fn([f32; 3], [f32; 3]) -> Option<f32>, wall: [f32; 2]) -> Blast {
    let far = distance(feet, at);
    let shake = inverse_lerp(explosive.max * 3.0, explosive.min / 2.0, far);
    let mut out = Blast { shake, ..Blast::default() };
    if far > explosive.max && far > explosive.contusion[1] {
        return out;
    }
    let mut reached = false;
    // The wall the chest's line was stopped by, as thick as from where the
    // line enters it to where the line back from the chest does (the last
    // chest collider's so stopped).
    let mut wall_thickness = None;
    for point in points {
        let length = distance(point.at, at);
        if let Some(share) = entered(at, point.at) {
            if point.part == Part::Chest {
                let forward = along(at, point.at, share);
                // (A line back that meets nothing leaves the game's hit at
                // the world's origin.)
                let back = entered(point.at, at).map_or([0.0; 3], |share| along(point.at, at, share));
                wall_thickness = Some(distance(forward, back));
            }
            continue;
        }
        reached = true;
        if explosive.strength <= 0.0 {
            continue;
        }
        let damage = inverse_lerp(explosive.max, explosive.min, length) * explosive.strength;
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
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The F-1's figures (its item's).
    const F1: Explosive = Explosive { max: 7.0, min: 3.0, strength: 80.0, contusion: [1.5, 4.0, 15.0] };
    const WALL: [f32; 2] = [0.4, 0.01];

    fn body(feet: [f32; 3]) -> Vec<HitPoint> {
        let up = |part, height| HitPoint { part, at: [feet[0], feet[1] + height, feet[2]] };
        vec![up(Part::Head, 1.6), up(Part::Chest, 1.3), up(Part::Stomach, 1.0), up(Part::LeftLeg, 0.5), up(Part::RightLeg, 0.5)]
    }

    #[test]
    fn near_a_blast_every_part_takes_its_strength_and_a_contusion() {
        let clear = |_: [f32; 3], _: [f32; 3]| None;
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
        let clear = |_: [f32; 3], _: [f32; 3]| None;
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
        let wall = |from: [f32; 3], to: [f32; 3]| -> Option<f32> {
            let (low, high) = (0.5f32, 0.55f32);
            let (a, b) = (from[0], to[0]);
            if (a - b).abs() < 1e-9 {
                return None;
            }
            let enter = if a < b { (low - a) / (b - a) } else { (high - a) / (b - a) };
            (enter > 0.0 && enter < 1.0).then_some(enter)
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
}
