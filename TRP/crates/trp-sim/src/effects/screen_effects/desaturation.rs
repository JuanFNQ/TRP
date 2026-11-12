//! The health desaturation of the screen.

use super::*;

/// The desaturation's two accumulators (`DesaturateAccumulator`,
/// `DesaturateMaskAccumulator`): the effects each holds, the figures they
/// move, and the mask's state.
#[derive(Debug, Clone)]
pub struct Desaturation {
    health_listed: Vec<(u32, Source)>,
    pub(super) health: f32,
    mask_listed: Vec<u32>,
    pub(super) mask_on: bool,
    restore: bool,
    pub(super) mask: f32,
    pub(super) radius: f32,
    /// The ring's least and most radius (`MinMaxRadius`).
    pub(super) radii: [f32; 2],
}

impl Default for Desaturation {
    fn default() -> Self {
        Self { health_listed: Vec::new(), health: 0.0, mask_listed: Vec::new(), mask_on: false, restore: false, mask: 0.0, radius: 1.0, radii: [0.325, 1.0] }
    }
}

/// The health's share each effect drives it toward (`_effectMaxValues`):
/// a medkit's use brightens, exhaustion greys.
fn health_desaturation(source: Source) -> f32 {
    if source == Source::MedKit {
        -0.15
    } else {
        0.15
    }
}

/// `Mathf.MoveTowards`.
fn move_towards(current: f32, target: f32, step: f32) -> f32 {
    if (target - current).abs() <= step {
        target
    } else {
        current + (target - current).signum() * step
    }
}

impl Desaturation {
    /// As a recording's first frame has it: the figures, the ring's
    /// radii, and the effects already started taken in (the mask on where
    /// a painkiller is), nothing moved.
    pub fn resume(figures: [f32; 3], radii: [f32; 2], listed: &[Listed]) -> Self {
        let mut d = Self { health: figures[0], mask: figures[1], radius: figures[2], radii, ..Self::default() };
        d.health_listed = listed.iter().filter(|e| e.started && matches!(e.source, Source::MedKit | Source::Exhaustion)).map(|e| (e.id, e.source)).collect();
        d.mask_listed = listed.iter().filter(|e| e.started && e.source == Source::PainKiller).map(|e| e.id).collect();
        d.mask_on = !d.mask_listed.is_empty() || figures[1] > 0.0;
        d
    }

    /// A frame (`AddEffect`, `DeleteEffect`, `UpdateAmount` of each): the
    /// health's toward the most of its effects at 0.1 a second, every
    /// frame; the mask, while on, its radius toward the least with a
    /// painkiller started (the most without) at 0.2 a second and its
    /// desaturation toward one (nought) at 0.5; a painkiller gone, the
    /// radius opens first, then the desaturation fades and the mask is off.
    pub fn frame(&mut self, dt: f32, listed: &[Listed]) -> [f32; 3] {
        for e in listed.iter().filter(|e| e.started && matches!(e.source, Source::MedKit | Source::Exhaustion)) {
            if !self.health_listed.iter().any(|(id, _)| *id == e.id) {
                self.health_listed.push((e.id, e.source));
            }
        }
        self.health_listed.retain(|(id, _)| listed.iter().any(|e| e.id == *id));
        let most = self.health_listed.iter().map(|(_, s)| health_desaturation(*s)).fold(None, |m: Option<f32>, v| Some(m.map_or(v, |m| m.max(v)))).unwrap_or(0.0);
        self.health = move_towards(self.health, most, dt * 0.1);
        for e in listed.iter().filter(|e| e.started && e.source == Source::PainKiller) {
            if !self.mask_listed.contains(&e.id) {
                self.mask_listed.push(e.id);
                self.mask_on = true;
            }
        }
        let before = self.mask_listed.len();
        self.mask_listed.retain(|id| listed.iter().any(|e| e.id == *id));
        if self.mask_listed.len() < before {
            self.restore = true;
        }
        if self.mask_on {
            let any = !self.mask_listed.is_empty();
            let (goal, radius) = if any { (1.0, self.radii[0]) } else { (0.0, self.radii[1]) };
            // The radius as the frame began: the fading waits for it open.
            let was = self.radius;
            self.radius = move_towards(self.radius, radius, dt * 0.2);
            if !self.restore {
                self.mask = move_towards(self.mask, goal, dt * 0.5);
            } else if was >= self.radii[1] {
                self.mask = move_towards(self.mask, goal, dt * 0.5);
                if self.mask <= 0.0 {
                    self.radius = self.radii[1];
                    self.restore = false;
                    self.mask_on = false;
                }
            }
        }
        [self.health, self.mask, self.radius]
    }
}
