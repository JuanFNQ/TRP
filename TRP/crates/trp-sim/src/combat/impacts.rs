//! What a bullet's hit plays: which hits play, where, how loud, and whether seen.
//! After the ballistics spec, "What a hit shows and sounds"
//! (`EffectsCommutator.PlayHitEffect`, `Effects.Emit`, `Effects.Update`): which hits play at all, where, how loud, whether a
//! ricochet's own sound comes with it, whether its picture is seen, and how
//! many a frame. In the game's axes; the host plays them.

use crate::combat::ballistics::{Hit, State};

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
    /// A living character's hit that went through
    /// (`CheckEnvironmentHitAfterBodyHit`): the line the host casts for a
    /// wall behind, from the hit point (not moved) along the bullet's way,
    /// `DRAW_BLOOD_ON_WALLS_MAX_DISTANCE` long on the effects' layers, and
    /// lays the blood on what it meets first if that is a ballistic thing
    /// and not a body (`EmitBloodOnEnvironment`), at once: before the
    /// frame's effects play.
    pub blood_behind: Option<BloodLine>,
    /// A blade's hit (`Effects.Emit`'s `isKnife`): its material's particles,
    /// light and sound, and no decal of either kind.
    pub knife: bool,
    /// A sound of the blade's own played at the place in place of the
    /// material's: its place among the effects' other banks
    /// (`AdditionalSoundEffects`).
    pub own_sound: Option<usize>,
}

/// The effects' other banks a melee weapon shown on the character's model
/// plays at its hit (`EffectsCommutator.PlayKnifeHitEffect`: the fourth on
/// a body or its armour, the fifth on anything else).
pub const SHOWN_BLADE_ON_BODY: usize = 3;
pub const SHOWN_BLADE_ON_THING: usize = 4;

impl Impact {
    /// What a blade's hit plays (`EffectsCommutator.PlayKnifeHitEffect`): the
    /// material of what it struck, at the hit point moved along the normal by
    /// `DECAL_SHIFT`, at full volume. A melee weapon shown on the
    /// character's model (`shown_on_model`, its template's `DisplayOnModel`)
    /// plays the material's effect at no volume, so without its sound, and
    /// a sound of its own at the same place.
    pub fn knife(material: i32, point: [f32; 3], normal: [f32; 3], decal_shift: f32, seen: bool, shown_on_model: bool) -> Self {
        use crate::combat::ballistics::material::{BODY, BODY_ARMOR};
        let position = std::array::from_fn(|k| point[k] + normal[k] * decal_shift);
        let own_sound = shown_on_model.then_some(if material == BODY || material == BODY_ARMOR { SHOWN_BLADE_ON_BODY } else { SHOWN_BLADE_ON_THING });
        Self { material, position, normal, volume: if shown_on_model { 0.0 } else { 1.0 }, seen, ricochet_sound: false, thing: 0, blood_behind: None, knife: true, own_sound }
    }
}

/// Where to look for a wall behind a body's hit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BloodLine {
    pub from: [f32; 3],
    pub way: [f32; 3],
}

/// Whether a hit a living character took went through
/// (`DamageInfo.Penetrated`, read in its IL: not blocked, or blocked and not
/// deflected), so that only a bullet both blocked and deflected by its
/// armour leaves no blood behind: a plate that stopped it does not.
pub fn penetrated(hit: &Hit) -> bool {
    hit.blocked_by.is_none() || hit.deflected_by.is_none()
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
    /// is within the camera's picture. A living character's hit that went
    /// through (`penetrated`) gives the line to look along for a wall
    /// behind (`Impact::blood_behind`).
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
            blood_behind: (taken.is_some() && penetrated(hit)).then_some(BloodLine { from: hit.point, way: hit.direction }),
            knife: false,
            own_sound: None,
        })
    }
}

/// What a character's health event does to its drops on the ground
/// (`Player.OnHealthEffectVisualAdded`, `…Removed`, `OnPlayerVisualDied`):
/// a heavy bleeding's start puts it in (true), a heavy bleeding going
/// residual or its death takes it out (false); nothing else does.
pub fn bleeding_change(event: &crate::condition::health::Event) -> Option<bool> {
    use crate::condition::health::{Event, Kind};
    match event {
        Event::Started(Kind::HeavyBleeding, _) => Some(true),
        Event::Residual(Kind::HeavyBleeding, _) | Event::Died(_) => Some(false),
        _ => None,
    }
}

/// The characters bleeding heavily, each with the time its next drop of
/// blood on the ground is due (`EffectsCommutator`'s
/// `_playerBleedingDecalTimes`; body hits spec, "What a body's hit shows").
/// A heavy bleeding's start puts its character in (`StartBleedingForPlayer`,
/// on `EffectStartedEvent`: the bleeding past its delay), due at once; a
/// heavy bleeding going residual (healed, or run out) takes it out, even
/// with another heavy bleeding still on, and so does its death
/// (`StopBleedingForPlayer`).
#[derive(Debug, Clone, Default)]
pub struct Bleedings {
    list: Vec<(u32, f32)>,
}

impl Bleedings {
    /// A character's heavy bleeding started: in, due at once, if it is not
    /// in already.
    pub fn start(&mut self, character: u32) {
        if !self.list.iter().any(|(c, _)| *c == character) {
            self.list.push((character, 0.0));
        }
    }

    /// A character's heavy bleeding ended, or it died.
    pub fn stop(&mut self, character: u32) {
        if let Some(at) = self.list.iter().position(|(c, _)| *c == character) {
            self.list.remove(at);
        }
    }

    pub fn is_bleeding(&self, character: u32) -> bool {
        self.list.iter().any(|(c, _)| *c == character)
    }

    /// The commutator's update (`UpdatePlayersBleedings`) at the game's time
    /// `now`: each character in turn whose drop is due (`now` past its time)
    /// is handed to `drop`, which casts down from its feet
    /// (`DRAW_BLEEDING_MAX_DISTANCE` on the effects' layers) and lays the
    /// drop where that meets something (`EmitBleeding`), saying whether it
    /// did; then, if it did, its next is due a draw between `spawn_delta`'s
    /// two later (`Random.Range`, after the drop's own draws). One that met
    /// nothing is tried again on the next update.
    pub fn update(&mut self, now: f32, spawn_delta: [f32; 2], random: &mut crate::base::unity_random::UnityRandom, mut drop: impl FnMut(u32, &mut crate::base::unity_random::UnityRandom) -> bool) {
        for (character, due) in &mut self.list {
            if now > *due && drop(*character, random) {
                *due = now + random.range(spawn_delta[0], spawn_delta[1]);
            }
        }
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
    use crate::combat::ballistics::{Round, Surface};

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
                fuse: None,
                stamina_burn: 0.0,
                heavy_bleeding: 0.0,
                light_bleeding: 0.0,
                tracer: None,
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

    /// A blade's hit: its material at the point moved along the normal, at
    /// full volume, and marked so that no decal is laid for it.
    #[test]
    fn a_blade_s_hit_is_marked_and_moved_off_the_surface() {
        let impact = Impact::knife(5, [1.0, 2.0, 3.0], [0.0, 1.0, 0.0], 0.05, true, false);
        assert!(impact.knife && impact.seen && !impact.ricochet_sound && impact.blood_behind.is_none());
        assert_eq!((impact.material, impact.volume, impact.own_sound), (5, 1.0, None));
        assert!((impact.position[1] - 2.05).abs() < 1e-6 && impact.position[0] == 1.0 && impact.position[2] == 3.0);
    }

    /// A melee weapon shown on the character's model: the material's
    /// effect silent, and its own sound, one on a body or its armour and
    /// another on anything else.
    #[test]
    fn a_blade_shown_on_the_model_has_its_own_hit_sound() {
        use crate::combat::ballistics::material::{BODY, BODY_ARMOR, CONCRETE};
        for (material, wanted) in [(BODY, SHOWN_BLADE_ON_BODY), (BODY_ARMOR, SHOWN_BLADE_ON_BODY), (CONCRETE, SHOWN_BLADE_ON_THING)] {
            let impact = Impact::knife(material, [1.0, 2.0, 3.0], [0.0, 1.0, 0.0], 0.05, true, true);
            assert!(impact.knife && impact.seen);
            assert_eq!((impact.material, impact.volume, impact.own_sound), (material, 0.0, Some(wanted)));
        }
    }

    #[test]
    fn ten_effects_a_frame_and_the_emitters_distances() {
        let impacts: Vec<Impact> = (0..14).map(|i| Impact { material: 5, position: [i as f32, 0.0, 0.0], normal: [0.0, 1.0, 0.0], volume: 1.0, seen: true, ricochet_sound: false, thing: 1, blood_behind: None, knife: false, own_sound: None }).collect();
        let played = played_this_frame(impacts);
        assert_eq!(played.len(), 10);
        assert_eq!(played[9].position[0], 9.0);
        assert!(within(0.0, [0.0, 50.0]) && within(50.0, [0.0, 50.0]) && !within(50.1, [0.0, 50.0]) && !within(49.0, [50.0, 500.0]));
    }

    /// A living character's hit looks for a wall behind along the bullet's
    /// way from the hit point unless its armour both blocked and deflected
    /// the bullet: a plate that stopped it alone does not stop the blood.
    /// A hit on anything else, or on the dead, looks for none.
    #[test]
    fn blood_behind_a_body_s_hit_unless_blocked_and_deflected() {
        let mut c = Commutator::default();
        let mut through = hit([0.0, 1.0, 5.0], 700.0, 0, true, State::Flying);
        through.direction = [0.0, 0.0, 1.0];
        let line = c.hit(&through, 0.015, true, Some(36), |_| true).expect("played").blood_behind.expect("a line");
        assert_eq!((line.from, line.way), ([0.0, 1.0, 5.0], [0.0, 0.0, 1.0]));
        let mut stopped = hit([0.0, 1.0, 6.0], 700.0, 0, true, State::StopHit);
        stopped.blocked_by = Some(1);
        assert!(c.hit(&stopped, 0.015, true, Some(37), |_| true).expect("played").blood_behind.is_some());
        let mut both = hit([0.0, 1.0, 7.0], 700.0, 0, true, State::RicochetHit);
        both.blocked_by = Some(1);
        both.deflected_by = Some(2);
        assert!(!penetrated(&both));
        assert!(c.hit(&both, 0.015, true, Some(37), |_| true).expect("played").blood_behind.is_none());
        let mut deflected = hit([0.0, 1.0, 8.0], 700.0, 0, true, State::RicochetHit);
        deflected.deflected_by = Some(2);
        assert!(penetrated(&deflected));
        assert!(c.hit(&hit([0.0, 1.0, 9.0], 700.0, 0, true, State::Flying), 0.015, true, None, |_| true).expect("played").blood_behind.is_none());
    }

    /// A heavy bleeding's drops: due at once when it starts, then every
    /// draw between the two after a drop laid; one that met nothing tried
    /// again; out when a heavy bleeding ends, though another goes on.
    #[test]
    fn a_heavy_bleeding_s_drops() {
        let mut b = Bleedings::default();
        let mut random = crate::base::unity_random::UnityRandom::new(7);
        b.start(1);
        b.start(1);
        b.start(2);
        let mut laid = Vec::new();
        b.update(0.01, [1.0, 3.0], &mut random, |who, _| {
            laid.push(who);
            who == 1
        });
        assert_eq!(laid, vec![1, 2]);
        // The first's next is 1 to 3 s on; the second, which met nothing,
        // goes again at once.
        laid.clear();
        b.update(0.99, [1.0, 3.0], &mut random, |who, _| {
            laid.push(who);
            true
        });
        assert_eq!(laid, vec![2]);
        laid.clear();
        b.update(3.02, [1.0, 3.0], &mut random, |who, _| {
            laid.push(who);
            true
        });
        assert_eq!(laid.first(), Some(&1));
        b.stop(1);
        assert!(!b.is_bleeding(1) && b.is_bleeding(2));
    }
}
