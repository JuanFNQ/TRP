//! A weapon heard as it lands.
//! After the sound spec, "A weapon landing" (`LootItem.OnCollisionEnter`, `CheckSurface`, `CalculateEnergy`): every
//! thing a pool makes carries the loot part (`ObservedLootItem`, added by
//! `AssetsPoolStaticConfig`), and a thing whose template has a landing
//! sound (`DropSoundType`: a pistol, a submachine gun or a rifle) plays one
//! as it begins to touch something with a surface. At most one look at a
//! surface each half second: a touch within half a second of the last
//! look is not heard, and a look at what has no surface (a collider
//! without `BaseBallistic`) still counts as one. The sound's strength is
//! the fall's energy: the mass times gravity times the height between
//! where it set off (or last sounded) and where it is now, plus the mass
//! times the square of its speed now (after the touch's own push), as the
//! game writes it (not half of it). The host gives the touch, the surface,
//! the height, the speed and the time; the volume is the install's curve
//! of that energy (`ItemDropSounds.EnergyToVolumeCurve`), rounded to two
//! places.

/// How long after a look at a surface the next may be (`CheckSurface`'s
/// `delayToNextCheck` from `OnCollisionEnter`).
pub const CHECK_EVERY: f32 = 0.5;

/// A thing that may sound as it lands.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Landing {
    /// Its template's landing sound (`EItemDropSoundType`: none 0, pistol
    /// 1, submachine gun 2, rifle 3).
    pub kind: u8,
    /// Its rigid body's mass.
    pub mass: f32,
    /// The height it set off from, or last sounded at.
    from: f32,
    /// The time before which no surface is looked at.
    next_look: f32,
}

impl Landing {
    /// A thing setting off at a height: a thing thrown from where it was
    /// thrown (`ThrowItem`'s `CacheParameters`); a weapon a death lets go
    /// from the world's nought, as its loot part never had a place kept
    /// (`DropItemDead` keeps none; `PROVISIONAL`: a pooled weapon that was
    /// loot before keeps its last place in the game, and is taken here as
    /// never thrown).
    pub fn new(kind: u8, mass: f32, from: f32) -> Self {
        Self { kind, mass, from, next_look: 0.0 }
    }

    /// A touch begun at `now` (the game's time, seconds): what it met's
    /// surface (`ESurfaceSound`), if it has one, the thing's height and
    /// speed now, and gravity's strength. The energy to sound by, if the
    /// touch is heard.
    pub fn touched(&mut self, now: f32, surface: Option<u8>, height: f32, speed: f32, gravity: f32) -> Option<f32> {
        if self.kind == 0 || self.next_look > now {
            return None;
        }
        self.next_look = now + CHECK_EVERY;
        surface?;
        let energy = self.mass * gravity * (self.from - height).abs() + self.mass * speed * speed;
        self.from = height;
        Some(energy)
    }
}

/// The volume a landing plays at: the curve's value of its energy, rounded
/// to two places (`Math.Round(..., 2)`, to even at a half).
pub fn volume(curve_value: f32) -> f32 {
    ((curve_value as f64 * 100.0).round_ties_even() / 100.0) as f32
}

// ==== A corpse's fall ====
//
// A body heard as it falls, at any character's death, the player's and a
// bot's alike (health spec, "Death"; sound spec, "A body's fall";
// `Player.OnDead` → `CreateCorpse` → `Corpse.Init` → `TryToCreateRagdoll` →
// `PlayCorpseDropSound`): on the frame the corpse is made, a line straight
// down from the corpse's root (the character's feet as it died) finds the
// surface under it; that surface's set of the sounds of moving gives its
// prone drop's bank (concrete's set where the surface has none), played
// once where the root is as a thing let fall at a fixed energy is
// (`BetterAudio.PlayDropItem` with a bank: the bank's own source kind, clips
// by the distance, its own draw of volume and pitch, unheard past its
// rolloff).

/// The energy a corpse's fall is heard at (`PlayCorpseDropSound` hands
/// `PlayDropItem` a 50, which is the energy its volume's curve is read at,
/// not a reach).
pub const BODY_FALL_ENERGY: f32 = 50.0;

/// How far down from the root the surface is looked for
/// (`LootItem.CheckSurfaceImmediately`: a ray of 10 m on the steps' layers;
/// the 0.5 its caller passes is the time before the next look, in seconds,
/// which a corpse made anew never waits on). Nothing met within it, nothing
/// plays.
pub const BODY_FALL_LOOK: f32 = 10.0;

/// How far over the root the line down begins: the game's character stands
/// over its ground by its controller's (recorded at five deaths: 1.0 to 7.5
/// cm, `ground_below`), and a host's box is not entered by a line that
/// begins on its face, where its character may stand. `PROVISIONAL`: the
/// game's line begins at the root itself.
pub const BODY_FALL_LIFT: f32 = 0.01;

/// The line a corpse's surface is looked for along, from its root (the
/// game's axes): the start and the end.
pub fn body_fall_line(root: [f32; 3]) -> ([f32; 3], [f32; 3]) {
    ([root[0], root[1] + BODY_FALL_LIFT, root[2]], [root[0], root[1] - BODY_FALL_LOOK, root[2]])
}

/// The bank a corpse's fall plays, as a place in the sounds' banks, from
/// what the line down met: nothing (`None`), no sound; a surface (a thing
/// of none is concrete: `CheckSurfaceImmediately`), its set's
/// `ProneDropSoundBank`, or concrete's set's where the surface has no set
/// (`Sounds.GetSurfaceSet`'s null); nothing where that set has no such bank
/// (`PlayDropItem`'s null bank).
pub fn body_fall_bank(sounds: &trp_data::audio::sound::MovementSounds, met: Option<u8>) -> Option<usize> {
    let surface = met?;
    let set = sounds.surface(surface).or_else(|| sounds.surface(crate::sound::steps::CONCRETE))?;
    set.banks.get(crate::sound::steps::PRONE_DROP).copied().flatten()
}

/// The volume a corpse's fall is asked at, before its bank's own draw: the
/// drop sounds' curve (`ItemDropSounds.EnergyToVolumeCurve`) at
/// `BODY_FALL_ENERGY`, rounded to hundredths (the install's: 0.59).
pub fn body_fall_volume(curve: &trp_data::formats::curve::Curve) -> f32 {
    volume(curve.evaluate(BODY_FALL_ENERGY))
}

/// What a bank played at a point may come out at, before its falloff: the
/// volume asked times the bank's draw, its base less up to its delta
/// (`SoundBank.RandomVolume`), the least and the most; and its pitch, its
/// base within its delta either way (`RandomPitch`).
pub fn drawn_ranges(bank: &trp_data::audio::sound::Bank, asked: f32) -> ([f32; 2], [f32; 2]) {
    let least = if bank.delta_volume > 0.0 { bank.base_volume - bank.delta_volume } else { bank.base_volume };
    ([asked * least, asked * bank.base_volume], [bank.base_pitch - bank.delta_pitch, bank.base_pitch + bank.delta_pitch])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rifle_sounds_by_its_fall_and_speed() {
        let mut rifle = Landing::new(3, 3.5, 1.2);
        let energy = rifle.touched(1.0, Some(0), 0.05, 2.0, 9.81).expect("heard");
        assert!((energy - (3.5 * 9.81 * 1.15 + 3.5 * 4.0)).abs() < 1e-3, "{energy}");
        // Within half a second nothing is looked at; after it, the fall is
        // counted from where it last sounded.
        assert_eq!(rifle.touched(1.3, Some(0), 0.0, 1.0, 9.81), None);
        let energy = rifle.touched(1.6, Some(0), 0.05, 0.0, 9.81).expect("heard again");
        assert!(energy.abs() < 1e-4, "{energy}");
    }

    #[test]
    fn what_has_no_surface_is_silent_and_still_counts() {
        let mut rifle = Landing::new(3, 1.0, 0.0);
        assert_eq!(rifle.touched(0.0, None, 1.0, 3.0, 9.81), None);
        assert_eq!(rifle.touched(0.2, Some(0), 0.0, 3.0, 9.81), None);
        assert!(rifle.touched(0.6, Some(0), 0.0, 3.0, 9.81).is_some());
    }

    #[test]
    fn a_thing_with_no_landing_sound_is_never_heard() {
        let mut medkit = Landing::new(0, 0.3, 1.0);
        assert_eq!(medkit.touched(0.0, Some(0), 0.0, 5.0, 9.81), None);
    }

    #[test]
    fn the_volume_is_rounded_to_hundredths() {
        assert_eq!(volume(0.4882688), 0.49);
        assert_eq!(volume(0.125), 0.12);
    }

    fn bank(name: &str, base_volume: f32, delta_volume: f32) -> trp_data::audio::sound::Bank {
        trp_data::audio::sound::Bank {
            name: name.to_owned(),
            base_volume,
            base_pitch: 1.0,
            delta_volume,
            delta_pitch: 0.15,
            rolloff: 66.5,
            blend: None,
            lists: [vec![0], Vec::new(), Vec::new()],
            source_type: 3,
            physical: false,
            limited: false,
            clip_length: 1.0,
        }
    }

    /// Concrete (0) with a prone drop, soil (3) with its own, wood thin (18)
    /// with a set but no prone drop; no set for metal (1).
    fn sounds() -> trp_data::audio::sound::MovementSounds {
        let set = |surface: u8, drop: Option<usize>| {
            let mut banks = vec![None; 9];
            banks[crate::sound::steps::PRONE_DROP] = drop;
            trp_data::audio::sound::SurfaceSounds { surface, banks }
        };
        trp_data::audio::sound::MovementSounds {
            banks: vec![bank("ProneDropSoundBank", 0.8, 0.1), bank("ProneDropSoilSoundBank", 0.6, 0.1)],
            surfaces: vec![set(0, Some(0)), set(3, Some(1)), set(18, None)],
            ..Default::default()
        }
    }

    #[test]
    fn a_corpse_falls_on_its_surface_or_on_concrete() {
        let sounds = sounds();
        assert_eq!(body_fall_bank(&sounds, Some(3)), Some(1));
        assert_eq!(body_fall_bank(&sounds, Some(0)), Some(0));
        // A surface with no set: concrete's set.
        assert_eq!(body_fall_bank(&sounds, Some(1)), Some(0));
        // A set with no prone drop: nothing, not concrete's.
        assert_eq!(body_fall_bank(&sounds, Some(18)), None);
        // Nothing met within the line's reach: nothing.
        assert_eq!(body_fall_bank(&sounds, None), None);
    }

    #[test]
    fn the_line_down_reaches_ten_metres() {
        let (from, to) = body_fall_line([1.0, 2.0, 3.0]);
        assert_eq!(from, [1.0, 2.0 + BODY_FALL_LIFT, 3.0]);
        assert_eq!(to, [1.0, 2.0 - 10.0, 3.0]);
    }

    #[test]
    fn a_fall_comes_out_within_its_banks_draw() {
        // The install's curve at 50 rounds to 0.59; concrete's bank draws 0.7
        // to 0.8 of it (recorded: 0.414 to 0.468), soil's 0.5 to 0.6
        // (recorded: 0.303 to 0.351).
        let (volume, pitch) = drawn_ranges(&bank("concrete", 0.8, 0.1), 0.59);
        assert!((volume[0] - 0.413).abs() < 1e-5 && (volume[1] - 0.472).abs() < 1e-5, "{volume:?}");
        assert!((pitch[0] - 0.85).abs() < 1e-6 && (pitch[1] - 1.15).abs() < 1e-6, "{pitch:?}");
        let (volume, _) = drawn_ranges(&bank("snow", 0.6, 0.0), 0.59);
        assert!((volume[0] - volume[1]).abs() < 1e-6);
    }
}
