//! A weapon heard as it lands (sound spec, "A weapon landing";
//! `LootItem.OnCollisionEnter`, `CheckSurface`, `CalculateEnergy`): every
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
}
