//! The sun's shadows as the game's own quality presets set them
//! (`docs/spec/settings.md`, "The shadows"): how many cascades its shadow
//! map is cut into, where each ends, and how far shadows reach. The drawing
//! alone: nothing of the simulation changes.

use bevy::light::CascadeShadowConfig;

/// The game's shadow quality where the install says none: 0, low
/// (`GraphicsSettingsGroup`: `ShadowsQuality` registered with 0).
pub const DEFAULT_QUALITY: u8 = 0;

/// The qualities' names, 0 to 3, as the game's menu has them.
pub const NAMES: [&str; 4] = ["LOW", "MEDIUM", "HIGH", "ULTRA"];

/// How far shadows reach at each quality, in metres
/// (`GraphicsSettingsGroup.GetMaxShadowVisibility`). A map may ask for more
/// (`EnvironmentManager.OutdoorShadowDistance`: the larger of the two); the
/// range is no map of the game's.
const REACH: [f32; 4] = [30.0, 40.0, 60.0, 80.0];

/// Where each cascade ends, as a share of the reach (`QualityLevelPreset`:
/// low one cascade; medium two, split at 0.33; high and ultra four, split
/// at 0.05, 0.14 and 0.33).
fn shares(quality: u8) -> &'static [f32] {
    match quality {
        0 => &[1.0],
        1 => &[0.33, 1.0],
        _ => &[0.05, 0.14, 0.33, 1.0],
    }
}

/// The sun's cascades at a quality, from the camera's near plane. They meet
/// without blending into one another, as the cascades of Unity's built-in
/// renderer do (`overlap_proportion` 0).
pub fn cascades(quality: u8, near: f32) -> CascadeShadowConfig {
    let quality = quality.min(3);
    let reach = REACH[quality as usize];
    CascadeShadowConfig { bounds: shares(quality).iter().map(|share| share * reach).collect(), overlap_proportion: 0.0, minimum_distance: near }
}

/// What a quality comes to, for the run's first lines.
pub fn describe(quality: u8) -> String {
    let quality = quality.min(3);
    let count = shares(quality).len();
    format!("{} ({} cascade{} to {} m)", NAMES[quality as usize].to_lowercase(), count, if count == 1 { "" } else { "s" }, REACH[quality as usize])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cascades_are_the_game_s_presets() {
        let bounds = |quality| cascades(quality, 0.03).bounds;
        assert_eq!(bounds(0), vec![30.0]);
        assert_eq!(bounds(1), vec![0.33 * 40.0, 40.0]);
        assert_eq!(bounds(2), vec![0.05 * 60.0, 0.14 * 60.0, 0.33 * 60.0, 60.0]);
        assert_eq!(bounds(3), vec![0.05 * 80.0, 0.14 * 80.0, 0.33 * 80.0, 80.0]);
        assert_eq!(bounds(9), bounds(3));
        assert_eq!(cascades(1, 0.03).minimum_distance, 0.03);
    }
}
