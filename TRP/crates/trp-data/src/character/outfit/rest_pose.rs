//! Vector helpers, the rest place of bones and the moves that bend a limb.


/// A point or a direction.
pub type V3 = [f32; 3];

pub(super) fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

pub(super) fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub(super) fn scaled(a: V3, by: f32) -> V3 {
    [a[0] * by, a[1] * by, a[2] * by]
}

pub(super) fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub(super) fn cross(a: V3, b: V3) -> V3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

pub(super) fn length(a: V3) -> f32 {
    dot(a, a).sqrt()
}

pub(super) fn unit(a: V3) -> Option<V3> {
    let long = length(a);
    (long > 1e-6).then(|| scaled(a, 1.0 / long))
}

/// Where a bone of the install's skeleton is in its mesh's space at rest.
/// A mesh keeps, for each bone, the matrix that takes a point of the mesh
/// into the bone's own axes (rows `e00..e33`); the bone's own origin is
/// what that matrix takes to nothing.
pub fn rest_place(bind: &[f32; 16]) -> Option<V3> {
    let (a, b, c) = ([bind[0], bind[1], bind[2]], [bind[4], bind[5], bind[6]], [bind[8], bind[9], bind[10]]);
    let t = [bind[3], bind[7], bind[11]];
    let det = dot(a, cross(b, c));
    if det.abs() < 1e-12 {
        return None;
    }
    // The columns of the inverse of the matrix whose rows are a, b and c
    // are those rows' cross products, over the determinant.
    let inverse = [scaled(cross(b, c), 1.0 / det), scaled(cross(c, a), 1.0 / det), scaled(cross(a, b), 1.0 / det)];
    let at = |row: usize| -(inverse[0][row] * t[0] + inverse[1][row] * t[1] + inverse[2][row] * t[2]);
    Some([at(0), at(1), at(2)])
}

/// A bone of the install's skeleton as the clothing's meshes have it: its
/// path on the skeleton and the matrix (rows `e00..e33`) that takes a
/// point of the mesh into the bone's own axes.
#[derive(Debug, Clone)]
pub struct RestBone {
    pub path: String,
    pub bind: [f32; 16],
}

/// The three lists of triangles a fitted mesh has, by who sees them: the
/// legs, which the character's own eyes see; the arms, which they see in
/// place of the install's own arms; and the rest, which in first person
/// is a shadow and nothing more, as the install's upper body is.
pub const LEGS: usize = 0;

pub const ARMS: usize = 1;

pub const UPPER: usize = 2;

/// A move of something rigid: three axes and a shift. A point `p` goes to
/// `p.x * axes[0] + p.y * axes[1] + p.z * axes[2] + shift`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Move {
    pub axes: [V3; 3],
    pub shift: V3,
}

pub(super) const NO_TURN: [V3; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

impl Move {
    /// The move that leaves everything where it is.
    pub const STILL: Move = Move { axes: NO_TURN, shift: [0.0; 3] };

    /// A direction, turned.
    pub fn turned(&self, v: V3) -> V3 {
        add(add(scaled(self.axes[0], v[0]), scaled(self.axes[1], v[1])), scaled(self.axes[2], v[2]))
    }

    /// A point, moved.
    pub fn of(&self, p: V3) -> V3 {
        add(self.turned(p), self.shift)
    }

    /// The move that turns so and leaves one point at a place.
    pub(super) fn about(axes: [V3; 3], joint: V3, at: V3) -> Self {
        let turn = Move { axes, shift: [0.0; 3] };
        Move { axes, shift: sub(at, turn.turned(joint)) }
    }
}

/// One turn after another: `first`, then `then`.
pub(super) fn turns(then: [V3; 3], first: [V3; 3]) -> [V3; 3] {
    let then = Move { axes: then, shift: [0.0; 3] };
    first.map(|axis| then.turned(axis))
}

/// The shortest turn that takes one direction to another.
pub(super) fn arc(from: V3, to: V3) -> [V3; 3] {
    let (Some(u), Some(v)) = (unit(from), unit(to)) else { return NO_TURN };
    let (k, c) = (cross(u, v), dot(u, v));
    if c < -0.9999 {
        // Half a turn, about anything across the direction.
        let across = unit(cross(u, [1.0, 0.0, 0.0])).or_else(|| unit(cross(u, [0.0, 1.0, 0.0]))).unwrap_or([0.0, 0.0, 1.0]);
        return NO_TURN.map(|x| sub(scaled(across, 2.0 * dot(across, x)), x));
    }
    NO_TURN.map(|x| add(add(x, cross(k, x)), scaled(cross(k, cross(k, x)), 1.0 / (1.0 + c))))
}

/// How far a limb's middle joint is off the line from where the limb
/// hangs to where it ends, and which way: the side the limb is bent to.
pub(super) fn off_line(hangs: V3, middle: V3, ends: V3) -> V3 {
    let to = sub(middle, hangs);
    unit(sub(ends, hangs)).map_or(to, |along| sub(to, scaled(along, dot(to, along))))
}

/// Where a limb of two bones is bent: its middle joint, for a limb that
/// hangs at one place and reaches for another with two lengths. It bends
/// to the side `towards` points (what of that lies across the limb's own
/// line), or `else_towards` where that says nothing. Out of reach, the
/// limb is straight and its end short of the place.
pub(super) fn bend(hangs: V3, reaches: V3, first: f32, second: f32, towards: V3, else_towards: V3) -> V3 {
    let line = sub(reaches, hangs);
    let Some(along) = unit(line) else { return add(hangs, scaled(unit(towards).unwrap_or([0.0, 1.0, 0.0]), first)) };
    let far = length(line).clamp((first - second).abs() + 1e-5, first + second);
    let before = (first * first - second * second + far * far) / (2.0 * far);
    let off = (first * first - before * before).max(0.0).sqrt();
    let across = |v: V3| unit(sub(v, scaled(along, dot(v, along)))).filter(|_| length(sub(v, scaled(along, dot(v, along)))) > 1e-4);
    let side = across(towards).or_else(|| across(else_towards)).or_else(|| across([1.0, 0.0, 0.0])).or_else(|| across([0.0, 1.0, 0.0])).unwrap_or([0.0, 0.0, 1.0]);
    add(add(hangs, scaled(along, before)), scaled(side, off))
}

/// How a bone of an outfit's rig follows the install's skeleton, frame by
/// frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Follows {
    /// As the install's own bone: what is on it is carried exactly as that
    /// bone carries its own (the hips; a hand, which is on the weapon; a
    /// foot, which is on the ground).
    Theirs,
    /// Hung from another bone of the rig at this one's joint, and turned
    /// as the install's bone is turned.
    Hung { from: usize },
    /// The first bone of a limb of two (an upper arm, a thigh): hung from
    /// one bone and reaching for the joint of `end`, which is the
    /// install's own, with the outfit's own two lengths; bent at `lower`'s
    /// joint, to the side the install's own limb is bent.
    Limb { from: usize, lower: usize, end: usize },
    /// The second bone of such a limb: placed with the first.
    Lower,
    /// A bone along a limb's bone (the install's skeleton has a forearm in
    /// three and a thigh in two): hung from it, turned as the install's is
    /// and then as the limb's bone was turned to reach.
    Along { of: usize },
    /// Held by another bone of the rig at that bone's (or a third's)
    /// joint, and turned as the install's bone is turned: an arm seen
    /// from its own eyes goes back from the hand so, the forearm held at
    /// the wrist and the upper arm at the elbow, each lying exactly as the
    /// install's lies and as long as the outfit's own.
    Held { from: usize, at: usize },
}
