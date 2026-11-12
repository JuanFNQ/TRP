//! Eyes burnt by a bright light and the spots they leave.

use super::*;

/// The eyes' burn's figures (`EyeBurn` on `Cam2`; `trp_data::eye_burn`).
#[derive(Debug, Clone)]
pub struct EyeBurnFigures {
    pub scale: [f32; 2],
    pub max_time: f32,
    pub trail_every: f32,
    pub trail_shift: f32,
    pub max_trail: u32,
    pub trail_curve: Curve,
    pub scale_curve: Curve,
    /// How many spot materials and trail materials there are.
    pub spots: usize,
    pub trails: usize,
}

/// A spot of the burn as drawn this frame: its middle and half size in the
/// screen's terms (−1 to 1 each way, y up), its turn (`_Rotation`), its
/// life left (`_Coef`), and its picture (the spots' by their order, then
/// the trail's).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BurnSpot {
    pub middle: [f32; 2],
    pub half: [f32; 2],
    pub turn: f32,
    pub coef: f32,
    pub picture: usize,
}

/// A spot kept (`EyeBurnSpot`).
#[derive(Debug, Clone, Copy)]
pub(super) struct Spot {
    pub(super) middle: [f32; 2],
    pub(super) picture: usize,
    turn: f32,
    pub(super) scale: [f32; 2],
    start: f32,
    lasts: f32,
}

/// The eyes' burn (`EyeBurn`): its spots, the material last used, and the
/// time left before it goes off.
#[derive(Debug, Clone)]
pub struct EyeBurn {
    pub(super) figures: EyeBurnFigures,
    pub(super) spots: Vec<Spot>,
    pub(super) current: usize,
    off_in: Option<f32>,
}

impl EyeBurn {
    pub fn new(figures: EyeBurnFigures) -> Self {
        Self { figures, spots: Vec::new(), current: 0, off_in: None }
    }

    /// A burn (`EyeBurn.Burn`): where the blast is on the screen (0 to 1
    /// from the lower left, and its depth), the distance's share, the
    /// view's turn this frame (degrees, across and up), the screen's width
    /// over its height, the time, and draws from 0 to 1. Nothing where the
    /// blast is not in sight; else a spot there and a trail along the turn,
    /// and the burn on for its longest time.
    pub fn burn(&mut self, screen: [f32; 3], strength: f32, turn: [f32; 2], aspect: f32, now: f32, random: &mut dyn FnMut() -> f32) -> bool {
        let size = self.figures.scale[0] + (self.figures.scale[1] - self.figures.scale[0]) * strength;
        let scale = [size, size * aspect];
        let lasts = self.figures.max_time * strength;
        if !(screen[0] > 0.0 && screen[0] < 1.0 && screen[1] > 0.0 && screen[1] < 1.0 && screen[2] > 0.0) {
            return false;
        }
        // `AddEyeBurn`: the next spot material, a whole number of degrees.
        self.current += 1;
        let picture = self.current % self.figures.spots.max(1);
        let middle = [screen[0] * 2.0 - 1.0, screen[1] * 2.0 - 1.0];
        let degrees = (random() * 360.0).floor().min(359.0);
        self.spots.push(Spot { middle, picture, turn: degrees, scale, start: now, lasts });
        // `AddTrailEyeburn`: along the turn (across, and up turned down).
        let way = [turn[0], -turn[1]];
        let length = (way[0] * way[0] + way[1] * way[1]).sqrt();
        let count = (self.figures.max_trail as f32).min((length / self.figures.trail_every).floor()) as u32;
        if count >= 1 {
            let unit = [way[0] / length, way[1] / length];
            let mut at = [middle[0] + unit[0] * self.figures.trail_shift * scale[0], middle[1] + unit[1] * self.figures.trail_shift * scale[0]];
            for i in 0..count {
                let trail = (random() * self.figures.trails.max(1) as f32).floor().min(self.figures.trails.max(1) as f32 - 1.0) as usize;
                let degrees = (random() * 360.0).floor().min(359.0);
                let shrink = self.figures.trail_curve.evaluate(i as f32);
                let scale = [scale[0] * shrink, scale[1] * shrink];
                self.spots.push(Spot { middle: at, picture: self.figures.spots + trail, turn: degrees, scale, start: now, lasts });
                at = [at[0] + unit[0] * self.figures.trail_shift * scale[0], at[1] + unit[1] * self.figures.trail_shift * scale[0]];
            }
        }
        self.off_in = Some(self.figures.max_time);
        true
    }

    /// The frame's spots, each with its life left and its size by it; none
    /// once the burn has gone off (`TimeDisable`: its longest time after
    /// the last burn, the spots let go).
    pub fn frame(&mut self, dt: f32, now: f32) -> Vec<BurnSpot> {
        let Some(left) = self.off_in.as_mut() else { return Vec::new() };
        *left -= dt;
        if *left < 0.0 {
            self.off_in = None;
            self.spots.clear();
            return Vec::new();
        }
        self.spots
            .iter()
            .map(|spot| {
                let coef = if spot.lasts > 0.0 { (1.0 - (now - spot.start) / spot.lasts).clamp(0.0, 1.0) } else { 0.0 };
                let size = self.figures.scale_curve.evaluate(coef);
                BurnSpot { middle: spot.middle, half: [spot.scale[0] * size, spot.scale[1] * size], turn: spot.turn, coef, picture: spot.picture }
            })
            .collect()
    }

    /// Whether the burn is on (its picture laid over the frame).
    pub fn on(&self) -> bool {
        self.off_in.is_some()
    }
}
