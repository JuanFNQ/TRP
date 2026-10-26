//! What a bullet's hit plays (ballistics spec, "What a hit shows and
//! sounds"; `EffectsCommutator.PlayHitEffect`, `Effects.Emit`,
//! `Effects.Update`): which hits play at all, where, how loud, whether a
//! ricochet's own sound comes with it, whether its picture is seen, and how
//! many a frame. In the game's axes; the host plays them.

use crate::ballistics::{Hit, State};

/// Two hits closer than this on every axis in one frame play once.
pub const SAME_PLACE: f32 = 0.025;
/// The effects played a frame (`MAX_EFFECTS_PER_FRAME`); the rest of the
/// frame's are dropped.
pub const MAX_EFFECTS_PER_FRAME: usize = 10;

/// A hit to be played: what it met, where (the hit point moved along the
/// normal by the hard settings' `DECAL_SHIFT`), the normal, how loud (0 to
/// 1), whether its picture is seen (particles and light; the decal and the
/// sound are played either way), whether the ricochet's sound plays at it,
/// and the thing it met (a decal on a thing that moves follows it).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Impact {
    pub material: i32,
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub volume: f32,
    pub seen: bool,
    pub ricochet_sound: bool,
    pub thing: u64,
}

/// The places played this frame (`_hitPointsOfFrame`).
#[derive(Debug, Clone, Default)]
pub struct Commutator {
    played: Vec<[f32; 3]>,
}

impl Commutator {
    /// A new frame (the late update empties the list).
    pub fn new_frame(&mut self) {
        self.played.clear();
    }

    /// A hit as the shot delegate hands it on, if it plays: nothing past
    /// its life (the delegate does nothing with it); nothing within
    /// `SAME_PLACE` of a hit played this frame on every axis (its place
    /// kept all the same); its volume `InverseLerp(64, 256, speed²)` for a
    /// forward hit, halved for each fragment number past the first
    /// (`0.5 / n`), nought for a backward hit; a ricochet's sound with a
    /// ricochet, but for a hit a living character took (`taken`, the
    /// material that hit was: `PlayerHitInfo.Material`, which it plays
    /// instead of the collider's; body hits spec, "What a body's hit
    /// shows"); seen when it is the shot's own first bullet (`own`, a
    /// fragment number of nought), else when `in_view` says the hit point
    /// is within the camera's picture.
    pub fn hit(&mut self, hit: &Hit, decal_shift: f32, own: bool, taken: Option<i32>, in_view: impl Fn([f32; 3]) -> bool) -> Option<Impact> {
        if hit.out_of_life {
            return None;
        }
        let p = hit.point;
        if self.played.iter().any(|q| (p[0] - q[0]).abs() < SAME_PLACE && (p[1] - q[1]).abs() < SAME_PLACE && !((p[2] - q[2]).abs() >= SAME_PLACE)) {
            return None;
        }
        self.played.push(p);
        let share = if hit.fragment > 0 { 0.5 / hit.fragment as f32 } else { 1.0 };
        let position = std::array::from_fn(|k| p[k] + hit.normal[k] * decal_shift);
        let v = hit.velocity;
        let loud = inverse_lerp(64.0, 256.0, v[0] * v[0] + v[1] * v[1] + v[2] * v[2]);
        let seen = (hit.fragment == 0 && own) || in_view(hit.point);
        Some(Impact {
            material: taken.unwrap_or(hit.surface.material),
            position,
            normal: hit.normal,
            volume: if hit.forward { share * loud } else { 0.0 },
            seen,
            ricochet_sound: taken.is_none() && hit.state == State::RicochetHit,
            thing: hit.thing,
        })
    }
}

/// Unity's `Mathf.InverseLerp`.
fn inverse_lerp(a: f32, b: f32, value: f32) -> f32 {
    if a != b { ((value - a) / (b - a)).clamp(0.0, 1.0) } else { 0.0 }
}

/// A frame's queued effects as the effects' update plays them: the first
/// `MAX_EFFECTS_PER_FRAME`, the rest dropped.
pub fn played_this_frame(mut queue: Vec<Impact>) -> Vec<Impact> {
    queue.truncate(MAX_EFFECTS_PER_FRAME);
    queue
}

/// An effects' list entry's emitter plays only within its distances from
/// the camera (`ParticleSys.Emit`: not below the least, not past the most).
pub fn within(distance: f32, range: [f32; 2]) -> bool {
    !(distance < range[0]) && distance <= range[1]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ballistics::{Round, Surface};

    fn hit(point: [f32; 3], speed: f32, fragment: i32, forward: bool, state: State) -> Hit {
        Hit {
            shot: 1,
            fragment,
            seed: 0,
            depth: 0,
            point,
            normal: [0.0, 0.0, -1.0],
            forward,
            thing: 1,
            collider: 1,
            surface: Surface { material: 5, figures: [0.0; 6] },
            state,
            speed,
            velocity: [0.0, 0.0, speed],
            damage: 0.0,
            penetration: 0.0,
            armor_damage: 0.0,
            cos: -1.0,
            age: 0.0,
            fragments: 0,
            out_of_life: false,
            first_hit: true,
            body: None,
            deflected_by: None,
            blocked_by: None,
            no_damage: false,
            direction: [0.0, 0.0, 1.0],
            round: std::sync::Arc::new(Round {
                initial_speed: speed,
                mass_g: 1.0,
                diameter_mm: 1.0,
                ballistic_coefficient: 1.0,
                damage: 1.0,
                penetration_power: 1.0,
                penetration_deviation: 0.0,
                penetration_chance: 0.0,
                ricochet_chance: 0.0,
                fragmentation_chance: 0.0,
                min_fragments: 0,
                max_fragments: 0,
                armor_damage: 0.0,
                life: 5.0,
                accuracy: 0,
                projectiles: 1,
                penetration_damage_mod: 0.0,
                explosive: false,
                stamina_burn: 0.0,
                heavy_bleeding: 0.0,
                light_bleeding: 0.0,
            }),
        }
    }

    #[test]
    fn a_hit_plays_once_a_place_a_frame() {
        let mut c = Commutator::default();
        let first = c.hit(&hit([1.0, 1.0, 10.0], 700.0, 0, true, State::StopHit), 0.015, true, None, |_| false).expect("played");
        assert_eq!(first.position, [1.0, 1.0, 10.0 - 0.015]);
        assert_eq!(first.volume, 1.0);
        assert!(first.seen && !first.ricochet_sound);
        // Two centimetres off on every axis: the same place.
        assert!(c.hit(&hit([1.02, 1.02, 10.02], 700.0, 0, true, State::StopHit), 0.015, true, None, |_| true).is_none());
        // Three off on one: another.
        assert!(c.hit(&hit([1.03, 1.0, 10.0], 700.0, 0, true, State::StopHit), 0.015, true, None, |_| true).is_some());
        c.new_frame();
        assert!(c.hit(&hit([1.0, 1.0, 10.0], 700.0, 0, true, State::StopHit), 0.015, true, None, |_| true).is_some());
    }

    #[test]
    fn a_hit_s_volume_and_sight() {
        let mut c = Commutator::default();
        // A fragment: half, a third out of view.
        let f = c.hit(&hit([0.0, 0.0, 1.0], 700.0, 1, true, State::RicochetHit), 0.0, true, None, |_| false).expect("played");
        assert_eq!(f.volume, 0.5);
        assert!(!f.seen && f.ricochet_sound);
        let g = c.hit(&hit([0.0, 0.0, 2.0], 700.0, 2, true, State::StopHit), 0.0, true, None, |_| true).expect("played");
        assert_eq!(g.volume, 0.25);
        assert!(g.seen);
        // A backward hit is silent; a slow one quieter (12 m/s: 144 of 64 to 256).
        assert_eq!(c.hit(&hit([0.0, 0.0, 3.0], 700.0, 0, false, State::StopHit), 0.0, true, None, |_| true).expect("played").volume, 0.0);
        let slow = c.hit(&hit([0.0, 0.0, 4.0], 12.0, 0, true, State::StopHit), 0.0, true, None, |_| true).expect("played");
        assert!((slow.volume - 80.0 / 192.0).abs() < 1e-6);
        // A hit a living character took plays the material it was, and no
        // ricochet's whine.
        let body = c.hit(&hit([0.0, 0.0, 5.0], 700.0, 0, true, State::RicochetHit), 0.0, true, Some(36), |_| true).expect("played");
        assert_eq!((body.material, body.ricochet_sound), (36, false));
    }

    #[test]
    fn ten_effects_a_frame_and_the_emitters_distances() {
        let impacts: Vec<Impact> = (0..14).map(|i| Impact { material: 5, position: [i as f32, 0.0, 0.0], normal: [0.0, 1.0, 0.0], volume: 1.0, seen: true, ricochet_sound: false, thing: 1 }).collect();
        let played = played_this_frame(impacts);
        assert_eq!(played.len(), 10);
        assert_eq!(played[9].position[0], 9.0);
        assert!(within(0.0, [0.0, 50.0]) && within(50.0, [0.0, 50.0]) && !within(50.1, [0.0, 50.0]) && !within(49.0, [50.0, 500.0]));
    }
}
