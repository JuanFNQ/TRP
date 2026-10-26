//! Blood on the screen (health spec, "Blood on the screen"; `BloodOnScreen`,
//! `BloodDropsController`, `BloodDropPlacer`): where the drops go for a hit
//! and for a bleeding, which are drawn each frame and with what life, how
//! much the distortion map fades, and when the blood and a heavy bleeding's
//! pulse show. The host draws them.

use std::collections::VecDeque;

use trp_data::camera_effects::BloodData;

use crate::recoil::Rng;

/// How many drops there are to place (`BloodDropsController.MaxDrops`).
pub const MAX_DROPS: usize = 64;
/// How long a heavy bleeding's pulse shows after its last blood
/// (`ApplyVignette`'s 6 s).
pub const HEAVY_TIME: f32 = 6.0;
/// A heavy bleeding's drops are so many times a light one's
/// (`ARTERIAL_BLEEDING_FACTOR`).
pub const ARTERIAL_FACTOR: f32 = 2.0;
/// How far the drops' picture is blurred without glasses, in its pixels
/// (`OnBloodImageRendered`'s `MakeBlur(…, 34)`).
pub const DROP_BLUR_PIXELS: f32 = 34.0;
/// `Mathf.Cos(45f)`: of 45 radians, as the placer has it.
const COS_45: f32 = 0.525_321_96;
/// From the middle to an edge's middle (`_cathetus`).
const CATHETUS: f32 = 0.5;

/// The figures as `Start` leaves them.
#[derive(Debug, Clone)]
pub struct BloodFigures {
    pub data: BloodData,
    pub start_scale: [f32; 2],
    pub end_scale: [f32; 2],
    pub lifetime: [f32; 2],
    pub off_glass_scale: f32,
    pub initial_drops: usize,
}

impl BloodFigures {
    /// `BloodOnScreen.Start`'s changes: the life's ends 3 and 1.2 times;
    /// the scales' firsts both `min(StartXdistribution.x,
    /// EndScaleDimension.x)` and their seconds both
    /// `max(StartXdistribution.y, EndScaleDimension.y)`, as the game has it;
    /// off glasses' scale 1.8; twice the first drops, at most 64.
    pub fn new(data: BloodData) -> Self {
        let lifetime = [data.lifetime[0] * 3.0, data.lifetime[1] * 1.2];
        let first = data.start_x[0].min(data.end_scale[0]);
        let second = data.start_x[1].max(data.end_scale[1]);
        let initial_drops = ((2 * data.initial_drops) as usize).min(MAX_DROPS);
        Self { start_scale: [first, second], end_scale: [first, second], lifetime, off_glass_scale: 1.8, initial_drops, data }
    }
}

/// A drop placed and waiting to be drawn: its place (−1 to 1 each way,
/// as the placer leaves it), its scale, and its number in its shot.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Waiting {
    place: [f32; 2],
    scale: f32,
    number: usize,
}

/// A drop drawn this frame: its place and scale, and its life
/// (`_Lifetime`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DrawnDrop {
    pub place: [f32; 2],
    pub scale: f32,
    pub life: f32,
}

/// What the blood does this frame.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct BloodFrame {
    /// The blood drawn, and its maps worked (the last blood within its time).
    pub on: bool,
    /// The drops drawn into the distortion map this frame.
    pub drops: Vec<DrawnDrop>,
    /// Whether the drops' picture is blurred (without glasses).
    pub blurred: bool,
    /// The fade's share this frame (`dt / MaxBloodTime`; the shader takes
    /// at least 1/256).
    pub fade: f32,
    /// A heavy bleeding's pulse: the vignette's darkness.
    pub heavy: Option<f32>,
}

/// The blood's state.
#[derive(Debug, Clone)]
pub struct Blood {
    figures: BloodFigures,
    waiting: VecDeque<Waiting>,
    locked: bool,
    last_hit: f32,
    last_heavy: f32,
    glasses: bool,
    rng: Rng,
}

impl Blood {
    /// Started at `now` (`Start`): no glasses, and the pre-warm: the last
    /// hit 0.99 of the blood's time ago and a drop placed for a direction
    /// of nought.
    pub fn new(data: BloodData, now: f32, seed: u64) -> Self {
        let max_time = data.max_time;
        // The last heavy bleeding's time starts at the game's nought, which
        // its clock has long passed in the menus before a raid: here, never.
        let mut blood = Self { figures: BloodFigures::new(data), waiting: VecDeque::new(), locked: false, last_hit: f32::MIN, last_heavy: f32::NEG_INFINITY, glasses: false, rng: Rng::new(seed) };
        blood.last_hit = now - max_time * 0.99;
        let drops = blood.place([0.0, 0.0, 0.0], None);
        blood.waiting.extend(drops);
        blood
    }

    pub fn figures(&self) -> &BloodFigures {
        &self.figures
    }

    /// `Random.insideUnitSphere`.
    pub fn ball(&mut self) -> [f32; 3] {
        loop {
            let v = [self.rng.range(-1.0, 1.0), self.rng.range(-1.0, 1.0), self.rng.range(-1.0, 1.0)];
            if v[0] * v[0] + v[1] * v[1] + v[2] * v[2] <= 1.0 {
                return v;
            }
        }
    }

    /// `Random.Range` for whole numbers: from `low` to below `high`.
    fn whole(&mut self, low: i32, high: i32) -> i32 {
        if high <= low {
            return low;
        }
        (low + (self.rng.unit() * (high - low) as f32) as i32).min(high - 1)
    }

    /// A hit (`Hit`): one time in three, while not locked, the last hit
    /// now and one drop placed for the direction (the camera's own axes).
    /// Whether it was.
    pub fn hit(&mut self, direction: [f32; 3], now: f32) -> bool {
        if self.locked || self.whole(0, 3) != 0 {
            return false;
        }
        self.last_hit = now;
        let drops = self.place(direction, None);
        self.waiting.extend(drops);
        self.locked = true;
        true
    }

    /// A centre with a draw in a ball of that radius, laid flat.
    pub fn about(&mut self, centre: [f32; 3], radius: f32) -> [f32; 3] {
        let ball = self.ball();
        [centre[0] + ball[0] * radius, 0.0, centre[2] + ball[2] * radius]
    }

    /// A bleeding's blood (`HitBleeding`): while not locked, the last hit
    /// now (a heavy one's too), its drops placed, locked.
    pub fn bleed(&mut self, direction: [f32; 3], heavy: bool, now: f32) -> bool {
        if self.locked {
            return false;
        }
        self.last_hit = now;
        if heavy {
            self.last_heavy = now;
        }
        let drops = self.place(direction, Some(heavy));
        self.waiting.extend(drops);
        self.locked = true;
        true
    }

    /// The glasses put on or off (`ChangeGlassesState`).
    pub fn set_glasses(&mut self, glasses: bool) {
        self.glasses = glasses;
    }

    /// `GetDropsPositions` (one drop) and `GetDropsPositionsBleeding` (a
    /// bleeding's count, heavy or not): along a ray in from the edge the
    /// direction points to, each with its spread and scale; none past the
    /// drops free.
    fn place(&mut self, direction: [f32; 3], bleeding: Option<bool>) -> Vec<Waiting> {
        let direction = if direction[0] * direction[0] + direction[1] * direction[1] + direction[2] * direction[2] < f32::EPSILON { [0.0, 0.0, -1.0] } else { direction };
        // `GetViewportIntersectionPoint`: the flat part made of length 1;
        // its up part if that is at least cos 45 (radians), else its across.
        let flat = [direction[0].abs(), direction[2].abs()];
        let length = (flat[0] * flat[0] + flat[1] * flat[1]).sqrt();
        if length <= 1e-5 {
            // Straight up or down: the game's point is not a number, and no
            // drop shows.
            return Vec::new();
        }
        let flat = [flat[0] / length, flat[1] / length];
        let along = if flat[1] < COS_45 { flat[0] } else { flat[1] };
        let hypotenuse = CATHETUS / along;
        let edge = [0.5 + hypotenuse * direction[0], 0.5 + hypotenuse * direction[2]];
        let f = self.figures.clone();
        let ray = hypotenuse * self.rng.range(f.data.max_ray[0], f.data.max_ray[1]);
        let count = match bleeding {
            None => 1,
            Some(heavy) => {
                let k = if heavy { ARTERIAL_FACTOR } else { 1.0 };
                self.whole((k * f.data.count_range[0]).floor() as i32, (k * f.data.count_range[1]).ceil() as i32).max(0) as usize
            }
        };
        let way = [-direction[0], -direction[2]];
        let scale_by = if self.glasses { 1.0 } else { f.off_glass_scale };
        let mut drops = Vec::new();
        for i in 0..count {
            if self.waiting.len() + drops.len() >= MAX_DROPS {
                break;
            }
            let share = (i as f32 + 1.0) / count as f32;
            let reach = ray / count as f32 * (i as f32 + 1.0);
            let x = self.spread(f.data.start_x, f.data.end_x, share, true);
            let y = self.spread(f.data.start_y, f.data.end_y, share, true);
            let scale = self.spread(f.start_scale, f.end_scale, share, false);
            let place = [edge[0] + way[0] * reach + x, edge[1] + way[1] * reach + y];
            let place = [place[0].clamp(0.0, 1.0) * 2.0 - 1.0, place[1].clamp(0.0, 1.0) * 2.0 - 1.0];
            let scale = (scale * 0.05 * scale_by).max(0.15);
            drops.push(Waiting { place, scale, number: i });
        }
        drops
    }

    /// `GetRandomValue`: the range at that share of the way from the
    /// start's to the end's, its sign a draw where signed, and a draw in it.
    fn spread(&mut self, start: [f32; 2], end: [f32; 2], share: f32, signed: bool) -> f32 {
        let mut range = [start[0] + share * (end[0] - start[0]), start[1] + share * (end[1] - start[1])];
        if signed && self.rng.range(-1.0, 1.0) < 0.0 {
            range = [-range[0], -range[1]];
        }
        self.rng.range(range[0], range[1])
    }

    /// A frame (`BloodOnScreen.Update` and what it draws): while the last
    /// blood is within its time, the waiting drops drawn (one, or for a
    /// shot's first drop with as many waiting as the first drops, a draw
    /// of one to one fewer than those), each with its life, unlocking; the
    /// fade; the heavy bleeding's pulse within 6 s of its last blood.
    pub fn frame(&mut self, dt: f32, now: f32) -> BloodFrame {
        let max_time = self.figures.data.max_time;
        let on = !(now - self.last_hit > max_time);
        if !on {
            return BloodFrame::default();
        }
        let mut drops = Vec::new();
        if let Some(first) = self.waiting.front().copied() {
            let initial = self.figures.initial_drops;
            let count = if first.number == 0 && self.waiting.len() >= initial { self.whole(1, initial as i32).max(1) as usize } else { 1 };
            let life_by = if self.glasses { 1.0 } else { self.figures.data.off_glass_lifetime };
            for _ in 0..count {
                let Some(drop) = self.waiting.pop_front() else { break };
                let life = self.rng.range(self.figures.lifetime[0], self.figures.lifetime[1]) * life_by;
                drops.push(DrawnDrop { place: drop.place, scale: drop.scale, life });
            }
            self.locked = false;
        }
        let heavy = (!(now - self.last_heavy > HEAVY_TIME)).then(|| 15.0 * (now.sin() + 1.0));
        BloodFrame { on, drops, blurred: !self.glasses, fade: dt / max_time, heavy }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The figures as the install has them (`the_camera_effects_are_read`).
    fn data() -> BloodData {
        BloodData {
            refraction: 0.046,
            downsampling: 3,
            max_time: 8.0,
            initial_drops: 4,
            color: [0.4, 0.0, 0.0, 1.0],
            dimension: 11,
            start_scale: [1.1622465, 2.385023],
            end_scale: [0.27263156, 1.3563193],
            count_range: [3.1818182, 7.2918644],
            max_ray: [0.4526316, 0.89551413],
            start_x: [0.0, 0.075],
            start_y: [0.0, 0.075],
            end_x: [0.05, 0.3],
            end_y: [0.05, 0.3],
            lifetime: [0.24736843, 1.0],
            off_glass_scale: 2.5,
            off_glass_lifetime: 2.0,
            levels: [[48.0 / 255.0; 4], [1.0; 4], [1.0; 4], [0.0; 4], [254.0 / 255.0; 4]],
            center: [0.5, 0.5],
            sharpness: 10.0,
        }
    }

    #[test]
    fn start_changes_the_figures_and_every_drop_is_the_least() {
        let f = BloodFigures::new(data());
        assert_eq!((f.start_scale, f.end_scale), ([0.0, 1.3563193], [0.0, 1.3563193]));
        assert!((f.lifetime[0] - 0.7421053).abs() < 1e-6 && (f.lifetime[1] - 1.2).abs() < 1e-6);
        assert_eq!(f.initial_drops, 8);
        let mut blood = Blood::new(data(), 100.0, 7);
        for _ in 0..50 {
            blood.locked = false;
            blood.bleed([1.0, 0.0, -1.0], true, 100.0);
            assert!(blood.waiting.iter().all(|d| (d.scale - 0.15).abs() < 1e-6));
            blood.waiting.clear();
        }
    }

    #[test]
    fn the_pre_warm_drop_is_drawn_at_the_foot_and_the_blood_goes_off() {
        let mut blood = Blood::new(data(), 100.0, 3);
        // From the foot's middle (a direction of nought: (0, 0, −1)), up a
        // ray of 0.23 to 0.45, spread up to 0.3 each way.
        let frame = blood.frame(0.016, 100.0);
        assert!(frame.on && frame.blurred && frame.heavy.is_none());
        assert_eq!(frame.drops.len(), 1);
        let drop = frame.drops[0];
        assert!(drop.place[1] <= 0.5 + 1e-5 && drop.place[0].abs() <= 0.6 + 1e-5, "{drop:?}");
        assert!(drop.life >= 0.742 * 2.0 - 1e-4 && drop.life <= 1.2 * 2.0 + 1e-4);
        assert!((frame.fade - 0.002).abs() < 1e-6);
        // 0.08 s later the blood is off, its map left as it is.
        assert!(!blood.frame(0.016, 100.09).on);
    }

    #[test]
    fn a_bleeding_s_drops_come_a_draw_at_once_then_one_a_frame() {
        let mut blood = Blood::new(data(), 0.0, 11);
        blood.frame(0.016, 0.0);
        let mut counts = Vec::new();
        for n in 0..40 {
            let now = 100.0 + n as f32 * 20.0;
            assert!(blood.bleed([-1.0, 0.0, -1.0], n % 2 == 1, now));
            assert!(!blood.bleed([-1.0, 0.0, -1.0], false, now), "locked until drawn");
            let waiting = blood.waiting.len();
            counts.push((n % 2 == 1, waiting));
            let first = blood.frame(0.016, now).drops.len();
            if waiting >= 8 {
                assert!((1..8).contains(&first), "{first} of {waiting}");
            } else {
                assert_eq!(first, 1);
            }
            let mut drawn = first;
            while !blood.waiting.is_empty() {
                assert_eq!(blood.frame(0.016, now).drops.len(), 1);
                drawn += 1;
            }
            assert_eq!(drawn, waiting);
            // A heavy bleeding pulses for 6 s.
            assert_eq!(blood.frame(0.016, now + 5.0).heavy.is_some(), n % 2 == 1);
            assert!(blood.frame(0.016, now + 6.5).heavy.is_none());
        }
        assert!(counts.iter().all(|&(heavy, n)| if heavy { (6..=14).contains(&n) } else { (3..=7).contains(&n) }), "{counts:?}");
    }

    /// A head's bleeding comes from below: its drops start at the foot's
    /// edge and run up, and the drawing's height times the screen's shape
    /// leaves many of them below the picture (the game's sums).
    #[test]
    fn where_a_head_bleeding_s_drops_land() {
        let mut blood = Blood::new(data(), 0.0, 9);
        blood.frame(0.016, 0.0);
        let (mut all, mut inside, mut middle) = (0, 0, 0);
        for n in 0..500 {
            let now = 100.0 + n as f32 * 20.0;
            let direction = blood.about([0.0, 0.0, -1.0], 1.0 / 3.0);
            blood.bleed(direction, true, now);
            while !blood.waiting.is_empty() {
                for drop in blood.frame(0.016, now).drops {
                    all += 1;
                    // The square's middle in the picture's clip space.
                    let y = -drop.place[1] * 16.0 / 9.0;
                    if y.abs() - drop.scale * 16.0 / 9.0 < 1.0 {
                        inside += 1;
                    }
                    if y.abs() < 1.0 {
                        middle += 1;
                    }
                }
            }
        }
        println!("{inside} of {all} drops touch the picture, {middle} with their middle in it");
        let share = middle as f32 / all as f32;
        assert!(share > 0.2 && share < 0.6 && inside > middle, "{share}");
    }

    #[test]
    fn a_hit_bleeds_one_time_in_three() {
        let mut blood = Blood::new(data(), 0.0, 5);
        let (mut bled, mut sum) = (0, [0.0f32; 2]);
        for n in 0..3000 {
            let now = 50.0 + n as f32;
            if blood.hit([0.6, 0.0, 0.8], now) {
                bled += 1;
                let frame = blood.frame(0.016, now);
                assert_eq!(frame.drops.len(), 1);
                let place = frame.drops[0].place;
                assert!(place[0] > -0.6 && place[1] > -0.6, "{place:?}");
                sum = [sum[0] + place[0], sum[1] + place[1]];
            }
        }
        assert!((850..1150).contains(&bled), "{bled}");
        // From the right and ahead: from the top right edge (0.875, 1), in
        // toward the middle, on the whole at the right and above it.
        let mean = [sum[0] / bled as f32, sum[1] / bled as f32];
        assert!(mean[0] > 0.2 && mean[1] > 0.2, "{mean:?}");
    }
}
