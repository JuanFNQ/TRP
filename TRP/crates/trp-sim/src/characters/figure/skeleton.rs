//! Small vector helpers, the blender and the named skeleton.

use super::*;

/// A bone's place and turning from a frame's: `a` then `b` within it.
pub(super) fn then(a: Placed, b: Placed) -> Placed {
    let at = quat_rotate(a.1, b.0);
    (std::array::from_fn(|k| a.0[k] + at[k]), quat_mul(a.1, b.1))
}

/// `b` in `a`'s axes.
pub(super) fn within(a: Placed, b: Placed) -> Placed {
    let back = [-a.1[0], -a.1[1], -a.1[2], a.1[3]];
    (quat_rotate(back, std::array::from_fn(|k| b.0[k] - a.0[k])), quat_mul(back, b.1))
}

pub(super) fn inverse(q: [f32; 4]) -> [f32; 4] {
    [-q[0], -q[1], -q[2], q[3]]
}

/// `Vector3.Lerp`, its share clamped.
pub(super) fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    let t = t.clamp(0.0, 1.0);
    std::array::from_fn(|k| a[k] + (b[k] - a[k]) * t)
}

pub(super) fn distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    (0..3).map(|k| (a[k] - b[k]).powi(2)).sum::<f32>().sqrt()
}

/// The game's `Player.ValueBlender`: a value that runs toward 0 or 1 at a
/// speed from when its target last changed, read off the clock.
#[derive(Debug, Clone, Copy)]
pub(super) struct Blender {
    pub(super) target: f32,
    pub(super) start_value: f32,
    pub(super) start_time: f32,
    pub(super) speed: f32,
}

impl Blender {
    pub(super) fn new(value: f32, speed: f32, now: f32) -> Self {
        Self { target: value, start_value: value, start_time: now, speed }
    }

    pub(super) fn value(&self, now: f32) -> f32 {
        // (`Mathf.Sign` is 1 at nought.)
        let sign = if self.target - 0.5 >= 0.0 { 1.0 } else { -1.0 };
        (self.start_value + (now - self.start_time) * self.speed * sign).clamp(0.0, 1.0)
    }

    /// A new target (`Target`'s setter): from the value now, from now; the
    /// same target again changes nothing.
    pub(super) fn aim(&mut self, target: f32, now: f32) {
        if self.target != target {
            self.start_value = self.value(now);
            self.start_time = now;
            self.target = target;
        }
    }
}

/// A list of bone paths, each after its parent.
#[derive(Debug, Clone, Default)]
pub(super) struct Skeleton {
    pub(super) paths: Vec<String>,
    pub(super) parents: Vec<Option<usize>>,
}

impl Skeleton {
    /// The bone at `path`, added with every bone above it not yet there.
    pub(super) fn add(&mut self, path: &str) -> usize {
        let mut parent = None;
        for (end, _) in path.match_indices('/').chain(std::iter::once((path.len(), ""))) {
            let prefix = &path[..end];
            parent = Some(match self.paths.iter().position(|known| known == prefix) {
                Some(at) => at,
                None => {
                    self.paths.push(prefix.to_owned());
                    self.parents.push(parent);
                    self.paths.len() - 1
                }
            });
        }
        parent.expect("a bone's path")
    }
}

/// The bones the poser reads and moves, by their places in the figure's
/// list; each pair right then left.
#[derive(Debug, Clone)]
pub(super) struct Named {
    pub(super) root: usize,
    pub(super) ribcage: usize,
    pub(super) neck: usize,
    pub(super) head: usize,
    pub(super) weapon_3rd: usize,
    pub(super) goals: [usize; 2],
    /// The collarbone, the upper arm, the forearm's three, the palm.
    pub(super) arms: [[usize; 6]; 2],
    /// Below the palm, the thumb first, each finger from the knuckle out:
    /// the hand poser's `Children` after the palm.
    pub(super) fingers: [[usize; 15]; 2],
    pub(super) feet: [usize; 2],
}

impl Named {
    pub(super) fn add(skeleton: &mut Skeleton) -> Self {
        let ribcage = format!("{SPINE3}/Base HumanRibcage");
        let side = |right: bool| if right { 'R' } else { 'L' };
        let arm = |skeleton: &mut Skeleton, right: bool| -> [usize; 6] {
            let s = side(right);
            let mut path = format!("{ribcage}/Base Human{s}Collarbone");
            let mut out = [0; 6];
            out[0] = skeleton.add(&path);
            for (k, name) in ["Upperarm", "Forearm1", "Forearm2", "Forearm3", "Palm"].iter().enumerate() {
                path.push_str(&format!("/Base Human{s}{name}"));
                out[k + 1] = skeleton.add(&path);
            }
            out
        };
        let fingers = |skeleton: &mut Skeleton, right: bool| -> [usize; 15] {
            let s = side(right);
            let palm = format!("{ribcage}/Base Human{s}Collarbone/Base Human{s}Upperarm/Base Human{s}Forearm1/Base Human{s}Forearm2/Base Human{s}Forearm3/Base Human{s}Palm");
            let mut out = [0; 15];
            for finger in 0..5 {
                let mut path = palm.clone();
                for joint in 0..3 {
                    path.push_str(&format!("/Base Human{s}Digit{}{}", finger + 1, joint + 1));
                    out[finger * 3 + joint] = skeleton.add(&path);
                }
            }
            out
        };
        let foot = |skeleton: &mut Skeleton, right: bool| {
            let s = side(right);
            skeleton.add(&format!("Root_Joint/Base HumanPelvis/Base Human{s}Thigh1/Base Human{s}Thigh2/Base Human{s}Calf/Base Human{s}Foot"))
        };
        Self {
            root: skeleton.add("Root_Joint"),
            ribcage: skeleton.add(&ribcage),
            neck: skeleton.add(&format!("{SPINE3}/Base HumanNeck")),
            head: skeleton.add(&format!("{SPINE3}/Base HumanNeck/Base HumanHead")),
            weapon_3rd: skeleton.add(&format!("{SPINE3}/Weapon_root_3rd_anim")),
            goals: [skeleton.add(&format!("{SPINE3}/Bend_Goal_Right")), skeleton.add(&format!("{SPINE3}/Bend_Goal_Left"))],
            arms: [arm(skeleton, true), arm(skeleton, false)],
            fingers: [fingers(skeleton, true), fingers(skeleton, false)],
            feet: [foot(skeleton, true), foot(skeleton, false)],
        }
    }
}
