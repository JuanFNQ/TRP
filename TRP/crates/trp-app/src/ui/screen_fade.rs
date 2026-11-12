//! The game's screens faded in and out softly as they are shown and shut.
//!
//! The game's screens shown softly (`UIScreen.ShowGameObject`, not asked to
//! be instant: `SoftShow`, `VisualExtensions.SoftChange` at its speed of 4,
//! `Co_ChangeCanvasGroupAlpha`): as a screen is shown its whole group is
//! set to 0 and gains four times the frame's time, on that frame and on each
//! after it, until it is within 0.05 of whole, when the next frame makes it
//! whole (about a quarter of a second). Shut (`UIScreen.Close`), the fade is
//! dropped and the screen is gone at once; shown again, it begins from 0.
//! The app's two game screens have it: the inventory's and the battle
//! screen's (`courses10`'s `inventory-open-close`: every opening, on the
//! key's frame, 4 x dt a frame to 1 past 0.95).
//!
//! The engine has no alpha for a group of nodes: each colour drawn under a
//! screen's roots (`OnScreen`) is multiplied by its screen's alpha after the
//! frame's own systems have set it, written past the engine's change
//! detection (what is drawn is read afresh each frame, and nothing is laid
//! out again for it), and put back as the next frame begins, so that what
//! sets a colour finds its own. Nothing is done while no screen fades.

use bevy::prelude::*;

pub struct ScreenFadePlugin;

impl Plugin for ScreenFadePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ScreenFades>()
            .init_resource::<Faded>()
            .add_systems(First, put_back)
            .add_systems(PostUpdate, fade.in_set(ScreensFaded).after(bevy::ui::UiSystems::PostLayout).after(bevy::ui::UiSystems::Stack));
    }
}

/// Where the screens' colours are faded for the frame: what reads them as
/// drawn comes after (`ui_dump`).
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ScreensFaded;

/// The game's screen a root of the interface is drawn on.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub enum OnScreen {
    Inventory,
    Battle,
}

/// Each screen's soft show.
#[derive(Resource, Default)]
pub struct ScreenFades {
    pub inventory: SoftShow,
    pub battle: SoftShow,
}

/// `SoftChange`'s speed, and how near whole the fade stops
/// (`Co_ChangeCanvasGroupAlpha`).
const SPEED: f32 = 4.0;
const NEAR: f32 = 0.05;

/// A screen's group alpha as `SoftShow` steps it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SoftShow {
    pub alpha: f32,
    fading: bool,
    up: bool,
}

impl Default for SoftShow {
    fn default() -> Self {
        Self { alpha: 1.0, fading: false, up: false }
    }
}

impl SoftShow {
    /// Follows the screen, once a frame: shown as it comes up, the group at
    /// 0 and its first step on this frame; shut as it goes, whole (it is not
    /// drawn shut); else a step of the fade.
    pub fn follow(&mut self, up: bool, dt: f32) {
        if up != self.up {
            self.up = up;
            (self.alpha, self.fading) = if up { (0.0, true) } else { (1.0, false) };
        }
        if !self.fading {
            return;
        }
        if (self.alpha - 1.0).abs() > NEAR {
            self.alpha = (self.alpha + dt * SPEED).min(1.0);
        } else {
            // The coroutine's end: whole (`SoftChange`'s callback).
            self.alpha = 1.0;
            self.fading = false;
        }
    }
}

/// The colours a fade changed this frame, to be put back.
#[derive(Resource, Default)]
struct Faded(Vec<(Entity, Kept)>);

#[derive(Clone, Copy, Default)]
struct Kept {
    ground: Option<Color>,
    border: Option<BorderColor>,
    image: Option<Color>,
    text: Option<Color>,
}

type Colours = (Option<&'static mut BackgroundColor>, Option<&'static mut BorderColor>, Option<&'static mut ImageNode>, Option<&'static mut TextColor>);

fn times(color: Color, alpha: f32) -> Color {
    color.with_alpha(color.alpha() * alpha)
}

/// Each colour under a fading screen's roots drawn at its screen's alpha.
fn fade(fades: Res<ScreenFades>, roots: Query<(Entity, &OnScreen)>, children: Query<&Children>, mut colours: Query<Colours>, mut faded: ResMut<Faded>) {
    for (root, screen) in &roots {
        let alpha = match screen {
            OnScreen::Inventory => fades.inventory.alpha,
            OnScreen::Battle => fades.battle.alpha,
        };
        if alpha >= 1.0 {
            continue;
        }
        for entity in std::iter::once(root).chain(children.iter_descendants(root)) {
            let Ok((ground, border, image, text)) = colours.get_mut(entity) else { continue };
            let mut kept = Kept::default();
            if let Some(mut ground) = ground {
                let ground = ground.bypass_change_detection();
                kept.ground = Some(ground.0);
                ground.0 = times(ground.0, alpha);
            }
            if let Some(mut border) = border {
                let border = border.bypass_change_detection();
                kept.border = Some(*border);
                for side in [&mut border.top, &mut border.right, &mut border.bottom, &mut border.left] {
                    *side = times(*side, alpha);
                }
            }
            if let Some(mut image) = image {
                let image = image.bypass_change_detection();
                kept.image = Some(image.color);
                image.color = times(image.color, alpha);
            }
            if let Some(mut text) = text {
                let text = text.bypass_change_detection();
                kept.text = Some(text.0);
                text.0 = times(text.0, alpha);
            }
            faded.0.push((entity, kept));
        }
    }
}

/// The colours as their own systems left them, before the frame begins.
fn put_back(mut faded: ResMut<Faded>, mut colours: Query<Colours>) {
    if faded.0.is_empty() {
        return;
    }
    for (entity, kept) in faded.0.drain(..) {
        let Ok((ground, border, image, text)) = colours.get_mut(entity) else { continue };
        if let (Some(mut ground), Some(color)) = (ground, kept.ground) {
            ground.bypass_change_detection().0 = color;
        }
        if let (Some(mut border), Some(color)) = (border, kept.border) {
            *border.bypass_change_detection() = color;
        }
        if let (Some(mut image), Some(color)) = (image, kept.image) {
            image.bypass_change_detection().color = color;
        }
        if let (Some(mut text), Some(color)) = (text, kept.text) {
            text.bypass_change_detection().0 = color;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `courses10`'s `inventory-open-close`, the tab bar's group alpha: the
    /// double tap (opened at 1268, shut at 1272 at 0.498) and the opening at
    /// 1279, from 0 again, each frame's figure from the frame's own time.
    #[test]
    fn a_screen_is_shown_softly_as_the_game_shows_it() {
        let mut fade = SoftShow::default();
        fade.follow(false, 0.0083);
        assert_eq!(fade.alpha, 1.0);
        let opened = [(0.0083337, 0.0333), (0.060877, 0.2768), (0.0138531, 0.3323), (0.0415278, 0.4984)];
        for (dt, alpha) in opened {
            fade.follow(true, dt);
            assert!((fade.alpha - alpha).abs() < 5e-4, "{} against the game's {alpha}", fade.alpha);
        }
        for dt in [0.0193513, 0.026838, 0.0212816, 0.012146, 0.010851, 0.0110405, 0.0108184] {
            fade.follow(false, dt);
        }
        let again = [
            (0.0106614, 0.0426),
            (0.0441164, 0.2191),
            (0.011709, 0.2659),
            (0.037586, 0.4163),
            (0.0164599, 0.4821),
            (0.0126245, 0.5326),
            (0.0123679, 0.5821),
            (0.0119765, 0.63),
            (0.012586, 0.6804),
            (0.0121218, 0.7288),
            (0.0139031, 0.7844),
            (0.0141337, 0.841),
            (0.0135587, 0.8952),
            (0.0136878, 0.95),
            (0.0132794, 1.0),
            (0.0127768, 1.0),
        ];
        for (dt, alpha) in again {
            fade.follow(true, dt);
            assert!((fade.alpha - alpha).abs() < 5e-4, "{} against the game's {alpha}", fade.alpha);
        }
    }
}
