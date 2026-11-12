//! Blow damage, the swing and the blade's sweep.


/// A blow's damage (`BaseKnifeController.ProcessHit`): the stroke's own, a
/// whole number, cut with the hands exhausted to the whole number under it
/// times the stamina settings' figure (`ExhaustedMeleeDamageMultiplier`,
/// 0.33: `exhausted`), then times one and Strength's melee buff
/// (`StrengthBuffMeleePowerInc`).
pub fn blow_damage(stroke: f32, exhausted: Option<f32>, strength: f32) -> f32 {
    let whole = stroke as i32;
    let whole = exhausted.map_or(whole, |factor| (whole as f32 * factor) as i32);
    whole as f32 * (1.0 + strength)
}

/// Unity's `Vector3.normalized`: nought under 1e-5 long.
fn unit(v: [f32; 3]) -> [f32; 3] {
    let length = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if length > 1e-5 { v.map(|x| x / length) } else { [0.0; 3] }
}

/// Unity's `Vector3.Lerp`.
fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    std::array::from_fn(|k| a[k] + (b[k] - a[k]) * t)
}

/// The swing's way (`GlobalSwingDirection`) and where the blade was when it
/// was last eased (`PreviousColliderPosition`), a melee controller's own:
/// nought as it is made.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Swing {
    pub way: [f32; 3],
    pub before: [f32; 3],
}

impl Swing {
    /// Eased by 0.9 toward the blade's step from where it was last (`at`
    /// the blade's object now), that place kept.
    pub fn follow(&mut self, at: [f32; 3]) {
        self.way = self.eased(at);
        self.before = at;
    }

    /// A hit's direction (`ProcessHit`): eased once more by the blade's
    /// step, and kept as the way; the place is not.
    pub fn at_hit(&mut self, at: [f32; 3]) -> [f32; 3] {
        self.way = self.eased(at);
        self.way
    }

    fn eased(&self, at: [f32; 3]) -> [f32; 3] {
        lerp3(self.way, unit(std::array::from_fn(|k| at[k] - self.before[k])), 0.9)
    }
}

/// The box a live blade is swept as (`KnifeCollider.ManualUpdateClient`;
/// the spec, "The knife's hands"), for the host's physics engine to sweep
/// (PhysX's own sweep, Unity's `Physics.BoxCast`): from the blade's box's
/// centre (`BoxCollider.center`, turned with the blade's object and not
/// scaled) a blade's length back along the look, half the box's width and
/// depth and a quarter of its length (its size not scaled), turned as the
/// blade's object, along the look by the stroke's reach.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BladeSweep {
    pub from: [f32; 3],
    pub half: [f32; 3],
    pub turn: [f32; 4],
    pub way: [f32; 3],
    pub reach: f32,
}

impl BladeSweep {
    /// The sweep of a blade's object at a place and turn in the world, its
    /// box's centre and size in its own axes, along the look's way.
    /// **Which place**: the game sweeps on a frame before the hands' clips
    /// move on, so from the blade where the frame before left it, with the
    /// first person's squeeze off: so the recorded hits, every one met by
    /// the first live sweep at the recorded point within 0.6 mm
    /// (`bin/grenade_knife`).
    pub fn of(at: [f32; 3], turn: [f32; 4], centre: [f32; 3], size: [f32; 3], look: [f32; 3], reach: f32) -> Self {
        let c = crate::base::math::quat_rotate(turn, centre);
        Self {
            from: std::array::from_fn(|k| at[k] + c[k] - look[k] * size[1]),
            half: [size[0] / 2.0, size[1] / 4.0, size[2] / 2.0],
            turn,
            way: look,
            reach,
        }
    }
}

/// The look's way in the world (`MovementContext.LookDirection`:
/// `Euler(pitch, yaw, 0)` times forward), the yaw to the right and the
/// pitch down, in degrees.
pub fn look_way(yaw: f32, pitch: f32) -> [f32; 3] {
    let (yaw, pitch) = (yaw.to_radians(), pitch.to_radians());
    [yaw.sin() * pitch.cos(), -pitch.sin(), yaw.cos() * pitch.cos()]
}
