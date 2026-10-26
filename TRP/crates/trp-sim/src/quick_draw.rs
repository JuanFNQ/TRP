//! The quick draw of the holster's weapon (stamina spec, "Wounds and the
//! fast weapon switch"): the holster's key pressed twice
//! (`QuickSelectSecondaryWeapon`, `FastSlotSelection`) puts the weapon in
//! hand away and draws the pistol faster, each costing the arms' stamina
//! (`Physical.OnWeaponSwitchFast`). This holds the speeds
//! (`WeaponFastDrawSettings`, `FirearmController.GetWeaponDrawSpeedMultiplier`);
//! the costs are `Stamina::fast_switch`.

use serde_json::Value as Json;

/// A drawn weapon's speeds, the slow end untrained and the fast at its
/// class skill's level 50 (the level counts by whole fifties), for a pistol
/// and for anything else.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QuickDraw {
    pub weapon: [f32; 2],
    pub pistol: [f32; 2],
}

/// Putting a weapon away goes at the draw's speed over this
/// (`useFastDropAnimationSpeed`).
const PUT_AWAY_SHARE: f32 = 1.5;

/// How long after the weapon is put away the pistol's draw still counts as
/// quick, in seconds (`FirearmController.SetAnimatorAndProceduralValues`).
pub const QUICK_WITHIN: f32 = 1.0;

impl QuickDraw {
    /// From `globals.json`'s `config` (`WeaponFastDrawSettings`).
    pub fn read(config: &Json) -> Option<Self> {
        let s = config.get("WeaponFastDrawSettings")?;
        let n = |name: &str| s.get(name).and_then(Json::as_f64).map(|x| x as f32);
        Some(Self {
            weapon: [n("WeaponFastSwitchMinSpeedMult")?, n("WeaponFastSwitchMaxSpeedMult")?],
            pistol: [n("WeaponPistolFastSwitchMinSpeedMult")?, n("WeaponPistolFastSwitchMaxSpeedMult")?],
        })
    }

    /// The animator's draw speed (`GetWeaponDrawSpeedMultiplier`) for a
    /// weapon of its class skill's `level`, a pistol or not, put away or
    /// drawn: the slow end plus a whole step at level 50, a third less
    /// putting away.
    pub fn speed(&self, pistol: bool, level: u32, putting_away: bool) -> f32 {
        let [slow, fast] = if pistol { self.pistol } else { self.weapon };
        let speed = slow + (level / 50) as f32 * (fast - slow);
        if putting_away { speed / PUT_AWAY_SHARE } else { speed }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_speed_steps_at_level_fifty_and_putting_away_is_a_third_less() {
        let q = QuickDraw { weapon: [1.5, 2.0], pistol: [1.5, 2.0] };
        assert_eq!(q.speed(true, 0, false), 1.5);
        assert_eq!(q.speed(true, 49, false), 1.5);
        assert_eq!(q.speed(true, 50, false), 2.0);
        assert!((q.speed(false, 10, true) - 1.0).abs() < 1e-6);
    }
}
