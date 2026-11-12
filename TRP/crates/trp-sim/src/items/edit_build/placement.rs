//! Placing the slots' labels around an ellipse and spacing them out.


/// An ellipse's arc lengths by whole degrees of its first quarter, as the
/// game's calculator keeps them (`EllipseCalculator.Ellipse`): made with
/// its `width` and `height` as the screen hands them over (the vertical
/// half first), each degree's arc summed over forty steps along it.
#[derive(Debug, Clone)]
pub struct Ellipse {
    width: f64,
    height: f64,
    eccentricity: f64,
    lengths: [f64; 90],
    pub length: f64,
}

const STEPS: usize = 40;

const DEGREE: f64 = 0.01745329;

impl Ellipse {
    pub fn new(width: f64, height: f64) -> Self {
        let mut ellipse = Self { width, height, eccentricity: 1.0 - height * height / (width * width), lengths: [0.0; 90], length: 0.0 };
        for i in 0..90 {
            let arc = ellipse.curve(DEGREE * i as f64, DEGREE * (i + 1) as f64);
            ellipse.lengths[i] = if i > 0 { ellipse.lengths[i - 1] } else { 0.0 } + arc;
        }
        ellipse.length = ellipse.lengths[89] * 4.0;
        ellipse
    }

    /// The arc from one angle to another (`GetCurveLength`).
    fn curve(&self, start: f64, angle: f64) -> f64 {
        if (start - angle).abs() < 1e-10 {
            return 0.0;
        }
        let (c, c0) = (angle.cos(), start.cos());
        let r = self.height / (1.0 - self.eccentricity * c * c).sqrt();
        let r0 = self.height / (1.0 - self.eccentricity * c0 * c0).sqrt();
        let (x, x0) = (r * c, r0 * c0);
        let step = (x - x0).abs() / STEPS as f64;
        let mut at_x = x0;
        let mut at_y = (self.height * self.height * (1.0 - at_x * at_x / (self.width * self.width))).sqrt();
        if at_y.is_nan() {
            at_y = 0.0;
        }
        let mut sum = 0.0;
        let way = if angle > std::f64::consts::PI { -1.0 } else { 1.0 };
        for _ in 0..STEPS {
            let next_x = at_x - step * way;
            let next_y = (self.height * self.height * (1.0 - next_x * next_x / (self.width * self.width))).sqrt();
            sum += ((at_x - next_x).powi(2) + (at_y - next_y).powi(2)).sqrt();
            (at_x, at_y) = (next_x, next_y);
        }
        sum
    }

    /// The length along the ellipse to a whole degree (`GetCashLength`).
    fn length_to(&self, angle: i32) -> f64 {
        if angle <= 0 {
            return 0.0;
        }
        let quarter = self.lengths[89];
        let share = angle as f32 / 90.0;
        if share < 1.0 {
            self.lengths[(angle - 1) as usize]
        } else if share > 1.0 && share < 2.0 {
            quarter + (quarter - self.lengths[(179 - angle) as usize])
        } else if share > 2.0 && share < 3.0 {
            quarter * 2.0 + self.lengths[(angle - 180) as usize]
        } else if share > 3.0 && share < 4.0 {
            quarter * 3.0 + (quarter - self.lengths[(359 - angle) as usize])
        } else {
            share as f64 * quarter
        }
    }

    /// The length along it to an angle in degrees (`LengthFromAngle`).
    pub fn length_at(&self, mut angle: f32) -> f32 {
        if angle < 0.0 {
            angle += 360.0;
        } else if angle > 360.0 {
            angle -= 360.0;
        }
        let whole = angle.floor() as i32;
        (self.length_to(whole) + self.curve(DEGREE * whole as f64, DEGREE * angle as f64)) as f32
    }

    /// The angle in degrees at a length along it (`AngleFromLength`,
    /// `GetAngle`, `GetNearestAngle`).
    pub fn angle_at(&self, mut length: f32) -> f32 {
        if length < 0.0 {
            length += self.length as f32;
        } else if length as f64 > self.length {
            length -= self.length as f32;
        }
        if length.abs() < 0.001 {
            return 0.0;
        }
        let quarter = self.lengths[89] as f32;
        let share = length / quarter;
        if share < 1.0 {
            self.nearest(length)
        } else if share > 1.0 && share < 2.0 {
            90.0 + (90.0 - self.nearest(2.0 * quarter - length))
        } else if share > 2.0 && share < 3.0 {
            180.0 + self.nearest(length - 2.0 * quarter)
        } else if share > 3.0 && share < 4.0 {
            270.0 + (90.0 - self.nearest(4.0 * quarter - length))
        } else {
            share * 90.0
        }
    }

    fn nearest(&self, length: f32) -> f32 {
        let at = self.lengths.iter().position(|&l| length - (l as f32) < 0.0).unwrap_or(0);
        let before = if at == 0 { 0.0 } else { self.lengths[at - 1] };
        let span = self.lengths[at] - before;
        (at as f64 + (length as f64 - before) / span) as f32
    }
}

/// A point put on the ellipse (half axes `a` across and `b` down, about
/// `centre`) along the line from its middle through it
/// (`CastPointsOnEllipse`).
pub fn cast_on_ellipse(point: [f32; 2], centre: [f32; 2], a: f32, b: f32) -> [f32; 2] {
    let angle = (point[1] - centre[1]).atan2(point[0] - centre[0]);
    let (s, c) = angle.sin_cos();
    let r = 1.0 / (b * b * c * c + a * a * s * s).sqrt() * (a * b);
    [r * c + centre[0], r * s + centre[1]]
}

/// Spreads values along a line that comes round on itself after `period`
/// so that no two are nearer than `distance` (`Spacer.SpaceOut`): each a
/// group of its own, groups that come within the distance of each other
/// merged about their mean (the later's values moved a period where they
/// lie across the line's ends), until none meet; then each group's values
/// laid out from its mean, in order, the distance apart. The values in
/// their own order.
pub fn space_out(values: &mut [f32], distance: f32, period: f32) {
    struct Group {
        average: f32,
        half: f32,
        items: Vec<usize>,
    }
    impl Group {
        fn left(&self) -> f32 {
            self.average - self.half
        }
        fn right(&self) -> f32 {
            self.average + self.half
        }
    }
    let settle = |group: &mut Group, values: &[f32]| {
        if !group.items.is_empty() {
            group.average = group.items.iter().map(|&i| values[i]).sum::<f32>() / group.items.len() as f32;
            group.half = (group.items.len() - 1) as f32 * distance * 0.5;
        }
    };
    let meet = |a: &Group, b: &Group| {
        let (mut l1, mut r1, mut l2, mut r2) = (a.left(), a.right(), b.left(), b.right());
        let gap = b.average - a.average;
        if gap.abs() >= period * 0.5 {
            if gap > 0.0 {
                l1 += period;
                r1 += period;
            } else {
                l2 += period;
                r2 += period;
            }
        }
        if l1 >= l2 { l1 < r2 + distance } else { r1 + distance > l2 }
    };
    let mut groups: Vec<Group> = (0..values.len()).map(|i| Group { average: 0.0, half: 0.0, items: vec![i] }).collect();
    loop {
        let mut merged = false;
        for group in &mut groups {
            settle(group, values);
        }
        if groups.len() > 1 {
            let mut first = 0;
            let mut n = 1;
            while n <= groups.len() && groups.len() > 1 {
                let index = if n < groups.len() { n } else { 0 };
                if meet(&groups[first], &groups[index]) {
                    merged = true;
                    // `Group.Merge`: the other's values moved a period
                    // where the two lie across the line's ends.
                    let other = groups.remove(index);
                    let into = if index < first { first - 1 } else { first };
                    let mut sum = groups[into].average * groups[into].items.len() as f32 + other.items.iter().map(|&i| values[i]).sum::<f32>();
                    let gap = other.average - groups[into].average;
                    if gap.abs() >= period * 0.5 {
                        let shift = if gap > 0.0 { -period } else { period };
                        sum += other.items.len() as f32 * shift;
                        for &i in &other.items {
                            values[i] += shift;
                        }
                    }
                    groups[into].items.extend(other.items);
                    groups[into].average = sum / groups[into].items.len() as f32;
                    groups[into].half = (groups[into].items.len() - 1) as f32 * distance * 0.5;
                    first = into;
                    if index == 0 {
                        break;
                    }
                } else {
                    first = index;
                    n += 1;
                }
            }
        }
        if !(merged && groups.len() > 1) {
            break;
        }
    }
    for group in &mut groups {
        let mut at = group.left();
        group.items.sort_by(|&l, &r| values[l].total_cmp(&values[r]));
        for &i in &group.items {
            values[i] = at;
            at += distance;
        }
    }
}

/// Where each slot's view lies (`UpdatePositions`), its bone's point on the
/// screen given (the panel's units, from its bottom left): on the ellipse
/// of half axes `a` across and `b` down about `centre`, by the length along
/// it (the calculator made with `b` and `a`, as the screen hands them) of
/// the bone's point cast onto it, spaced 110 apart along its whole length.
pub fn place_slots(bones: &[[f32; 2]], centre: [f32; 2], a: f32, b: f32) -> Vec<[f32; 2]> {
    const APART: f32 = 110.0;
    if bones.is_empty() || a <= 0.0 || b <= 0.0 {
        return Vec::new();
    }
    let ellipse = Ellipse::new(b as f64, a as f64);
    let mut lengths: Vec<f32> = bones
        .iter()
        .map(|&bone| {
            let on = cast_on_ellipse(bone, centre, a, b);
            let (x, y) = (on[0] - centre[0], on[1] - centre[1]);
            let angle = (y * a).atan2(x * b);
            ellipse.length_at(angle.to_degrees())
        })
        .collect();
    // Sorted along the ellipse before they are spaced (`list.Sort`), each
    // back to its own slot after.
    let mut order: Vec<usize> = (0..lengths.len()).collect();
    order.sort_by(|&l, &r| lengths[l].total_cmp(&lengths[r]));
    let mut sorted: Vec<f32> = order.iter().map(|&i| lengths[i]).collect();
    space_out(&mut sorted, APART, ellipse.length as f32);
    for (at, &i) in order.iter().enumerate() {
        lengths[i] = sorted[at];
    }
    lengths
        .iter()
        .map(|&length| {
            let angle = ellipse.angle_at(length).to_radians();
            [angle.cos() * a + centre[0], angle.sin() * b + centre[1]]
        })
        .collect()
}
