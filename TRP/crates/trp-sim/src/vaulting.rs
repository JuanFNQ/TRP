//! Vaulting and climbing: measuring what stands ahead and choosing the
//! move. See `docs/spec/vaulting.md`. The moves themselves are movement
//! states (`movement.rs`), played by the body's animator.

use anyhow::{Context, Result, anyhow};
use serde_json::Value as Json;

use crate::math::lerp;
use crate::movement::World;

/// How far the grid's rays reach, in metres.
const RAY: f32 = 10.0;
/// How far up a roof is felt for.
const ROOF_RAY: f32 = 1.6;
/// Top points this close in height to the heaviest one are the same top.
const SAME_TOP: f32 = 0.2;
/// The player's height as the checks for room take it.
const PLAYER_HEIGHT: f32 = 1.8;
/// The shares of that height a vault and a climb need under a roof.
const VAULT_ROOM: f32 = 0.5;
const CLIMB_ROOM: f32 = 0.95;
/// The drop behind is measured this far above the top's last point, and
/// this far past it; in a sprint, this far ahead of the grid's root.
const BEHIND_UP: f32 = 0.5;
const BEHIND_ON: f32 = 0.8;
const BEHIND_SPRINT: f32 = 2.6;
/// A vault needs the drop behind under the first; a sprinting character
/// vaults only with it at or under the second.
const VAULT_DROP: f32 = -0.3;
const SPRINT_DROP: f32 = -0.5;
/// How far past the obstacle there has to be room for a vault.
const LANDING_ROOM: f32 = 1.6;
/// The stairs test: where its rays start (behind the root, and above it),
/// how far apart they are, and the slopes that count as stairs.
const STAIRS_BACK: f32 = 0.366;
const STAIRS_UP: f32 = 0.9;
const STAIRS_STEP: f32 = 0.1;
const STAIRS_SLOPES: [f32; 2] = [0.45, 1.1917];

/// The limits a move is held to (`MoveRestrictions`).
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    /// The obstacle may be no further from the grid's root than this.
    pub distance: f32,
    pub height: [f32; 2],
    pub length: [f32; 2],
}

/// One move's settings.
#[derive(Debug, Clone, Copy)]
pub struct MoveSettings {
    pub active: bool,
    /// Up to this height the hands stay on the weapon.
    pub max_without_hand: f32,
    /// A climb above this takes both hands. A vault has no such figure.
    pub max_one_hand: f32,
    /// The clips' pace, from an unburdened character to a fully overweight
    /// one.
    pub speed: [f32; 2],
    pub limits: Limits,
    /// The limits of the automatic move, which nothing ever makes: two of
    /// their figures are read all the same.
    pub auto_limits: Limits,
}

/// `globals.json`, `config.VaultingSettings`.
#[derive(Debug, Clone, Copy)]
pub struct VaultingSettings {
    pub active: bool,
    /// The jump key held longer than this no longer jumps when let go.
    pub input_time: f32,
    /// The grid: its size, the step between rays and its offset, each
    /// across, up and ahead; and how far ahead of the feet it is put for
    /// each metre a second of forward speed.
    pub grid_size: [f32; 3],
    pub grid_step: [f32; 3],
    pub grid_offset: [f32; 3],
    pub offset_factor: f32,
    pub vault: MoveSettings,
    pub climb: MoveSettings,
}

impl VaultingSettings {
    /// `config` is the `config` section of `globals.json`.
    pub fn new(config: &Json) -> Result<Self> {
        let v = config.get("VaultingSettings").ok_or_else(|| anyhow!("globals have no VaultingSettings"))?;
        let number = |from: &Json, name: &str| -> Result<f32> {
            from.get(name).and_then(Json::as_f64).map(|f| f as f32).ok_or_else(|| anyhow!("VaultingSettings have no {name}"))
        };
        let flag = |from: &Json, name: &str| -> Result<bool> {
            from.get(name).and_then(Json::as_bool).ok_or_else(|| anyhow!("VaultingSettings have no {name}"))
        };
        fn part<'a>(from: &'a Json, name: &str) -> Result<&'a Json> {
            from.get(name).ok_or_else(|| anyhow!("VaultingSettings have no {name}"))
        }
        let limits = |from: &Json, name: &str| -> Result<Limits> {
            let l = part(from, name)?;
            Ok(Limits {
                distance: number(l, "MinDistantToInteract")?,
                height: [number(l, "MinHeight")?, number(l, "MaxHeight")?],
                length: [number(l, "MinLength")?, number(l, "MaxLength")?],
            })
        };
        let one = |name: &str, climb: bool| -> Result<MoveSettings> {
            let m = part(part(v, "MovesSettings")?, name)?;
            let speed = part(m, "SpeedRange")?;
            Ok(MoveSettings {
                active: flag(m, "IsActive")?,
                max_without_hand: number(m, "MaxWithoutHandHeight")?,
                max_one_hand: if climb { number(m, "MaxOneHandHeight")? } else { f32::INFINITY },
                speed: [number(speed, "x").context("SpeedRange")?, number(speed, "y").context("SpeedRange")?],
                limits: limits(m, "MoveRestrictions")?,
                auto_limits: limits(m, "AutoMoveRestrictions")?,
            })
        };
        let grid = part(v, "GridSettings")?;
        let three = |stem: &str| -> Result<[f32; 3]> {
            Ok([number(grid, &format!("{stem}X"))?, number(grid, &format!("{stem}Y"))?, number(grid, &format!("{stem}Z"))?])
        };
        Ok(Self {
            active: flag(v, "IsActive")?,
            input_time: number(v, "VaultingInputTime")?,
            grid_size: three("GridSize")?,
            grid_step: three("SteppingLength")?,
            grid_offset: three("GridOffset")?,
            offset_factor: number(grid, "OffsetFactor")?,
            vault: one("VaultSettings", false)?,
            climb: one("ClimbSettings", true)?,
        })
    }

    /// The heights between which a top point can be an obstacle's at all:
    /// the widest any of the four sets of limits allows.
    fn heights(&self) -> [f32; 2] {
        let sets = [self.vault.limits, self.vault.auto_limits, self.climb.limits, self.climb.auto_limits];
        [
            sets.iter().map(|l| l.height[0]).fold(f32::INFINITY, f32::min),
            sets.iter().map(|l| l.height[1]).fold(f32::NEG_INFINITY, f32::max),
        ]
    }

    /// The pace a move's clips play at for a character that overweight
    /// (0 to 1): the animator's `Vaulting_speed`.
    pub fn pace(&self, kind: Move, overweight: f32) -> f32 {
        let range = self.of(kind).speed;
        lerp(range[0], range[1], overweight.clamp(0.0, 1.0))
    }

    pub fn of(&self, kind: Move) -> &MoveSettings {
        match kind {
            Move::Vault => &self.vault,
            Move::Climb => &self.climb,
        }
    }
}

/// Over the obstacle and down the other side, or up onto it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Move {
    Vault,
    Climb,
}

/// What stands ahead, as last measured. Heights are over the grid's root
/// (the level of the feet), distances ahead of it along the body's facing.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Obstacle {
    pub height: f32,
    /// How deep the top is.
    pub length: f32,
    /// How far ahead of the grid's root its near edge is.
    pub distance: f32,
    /// The drop behind (the animator's `OverObstacleHeight`): -1 for
    /// ground at the feet's level beyond it, rising with the ground there.
    pub behind: f32,
    /// How far over the top the lowest thing above it is, if anything is
    /// within reach.
    pub roof: Option<f32>,
    /// The top is the terrain itself, which is never vaulted.
    pub terrain: bool,
}

/// A point a ray hit: in the grid root's axes (x across, y up, z ahead)
/// and in the world's.
#[derive(Debug, Clone, Copy)]
struct Point {
    local: [f32; 3],
    world: [f32; 3],
}

/// What the choice of a move goes by besides the obstacle.
#[derive(Debug, Clone, Copy)]
pub struct Standing {
    /// A vault or a climb is under way (the animator's `IsVaulting`).
    pub vaulting: bool,
    /// The movement keys ask for backward.
    pub backing: bool,
    pub grounded: bool,
    /// The body animator's first layer is between two states.
    pub in_transition: bool,
    /// A jump has been asked of the animator.
    pub jumping: bool,
    /// The sprint's own flag.
    pub sprinting: bool,
    /// Whether the legs, and the arms where the obstacle's height takes
    /// them, can pay for each move.
    pub can_vault: bool,
    pub can_climb: bool,
}

/// The game's `VaultingComponent`: the grid and what it last found.
#[derive(Debug, Clone)]
pub struct Vaulting {
    /// Where the grid's root stands in the body's axes. Each measurement
    /// sets it for the next.
    root: [f32; 3],
    /// The forward speed the root's place was last set with.
    pub forward_speed: f32,
    pub obstacle: Obstacle,
    /// The forward rays' hits, from the lowest ray up, and the downward
    /// rays', from the nearest out.
    forward: Vec<Point>,
    tops: Vec<Point>,
    /// The top point the obstacle was measured from.
    heaviest: Option<Point>,
    stairs: bool,
}

impl Default for Vaulting {
    fn default() -> Self {
        Self { root: [0.0; 3], forward_speed: 0.0, obstacle: Obstacle::default(), forward: Vec::new(), tops: Vec::new(), heaviest: None, stairs: false }
    }
}

impl Vaulting {
    /// One measurement (`VaultingController.Tick`): the rays, the heaviest
    /// point, the root's place for next time, the obstacle, and whether
    /// the character stands on stairs. `feet` is the character's place,
    /// `body_yaw` the way its body faces, `forward_speed` how fast it goes
    /// along the view's heading (nothing if it goes against it).
    pub fn measure(&mut self, s: &VaultingSettings, world: &dyn World, feet: [f32; 3], body_yaw: f32, forward_speed: f32, sprinting: bool) {
        let (sin, cos) = body_yaw.to_radians().sin_cos();
        // The root's axes to the world's and back, for a root standing at
        // `root` in the body's.
        let out_from = |root: [f32; 3], local: [f32; 3]| -> [f32; 3] {
            let (x, z) = (root[0] + local[0], root[2] + local[2]);
            [feet[0] + x * cos + z * sin, feet[1] + root[1] + local[1], feet[2] - x * sin + z * cos]
        };
        let back_to = |root: [f32; 3], at: [f32; 3]| -> [f32; 3] {
            let (x, z) = (at[0] - feet[0], at[2] - feet[2]);
            [x * cos - z * sin - root[0], at[1] - feet[1] - root[1], x * sin + z * cos - root[2]]
        };
        let cast_from = |root: [f32; 3], from: [f32; 3], to: [f32; 3]| -> Option<Point> {
            let length = (0..3).map(|k| (to[k] - from[k]).powi(2)).sum::<f32>().sqrt();
            let hit = world.line(from, to)?;
            let at: [f32; 3] = std::array::from_fn(|k| from[k] + (to[k] - from[k]) * hit / length);
            Some(Point { local: back_to(root, at), world: at })
        };
        // The grid's rays go by where the last measurement left the root.
        let root = self.root;
        let out = |local: [f32; 3]| out_from(root, local);
        let cast = |from: [f32; 3], to: [f32; 3]| cast_from(root, from, to);

        // The grid. A size over a step, rounded up, and one more.
        let count = |k: usize| (s.grid_size[k] / s.grid_step[k].max(0.01)).ceil() as usize + 1;
        let across = |j: usize| s.grid_offset[0] - s.grid_size[0] / 2.0 + s.grid_step[0] * j as f32;
        self.forward.clear();
        self.tops.clear();
        for i in 0..count(1) {
            for j in 0..count(0) {
                let from = out([across(j), s.grid_offset[1] + s.grid_step[1] * i as f32, s.grid_offset[2]]);
                self.forward.extend(cast(from, [from[0] + sin * RAY, from[1], from[2] + cos * RAY]));
            }
        }
        for i in 0..count(2) {
            for j in 0..count(0) {
                let from = out([across(j), s.grid_offset[1] + s.grid_size[1], s.grid_offset[2] + s.grid_step[2] * i as f32]);
                self.tops.extend(cast(from, [from[0], from[1] - RAY, from[2]]));
            }
        }

        // The heaviest top point: high and near. Of equals the first.
        let [lowest, highest] = s.heights();
        let weight = |p: &Point| p.local[1] * (-p.local[2]).exp();
        let mut heaviest: Option<usize> = None;
        for (i, p) in self.tops.iter().enumerate() {
            let within = p.local[1] >= lowest && p.local[1] <= highest;
            if within && heaviest.is_none_or(|best| weight(p) > weight(&self.tops[best])) {
                heaviest = Some(i);
            }
        }

        // The root's place for the next measurement. What is cast from
        // here on in this one goes by it already, though the points above
        // were placed by the old: the game moves the root between the two.
        self.forward_speed = forward_speed.max(0.0);
        self.root = [s.grid_offset[0], s.grid_offset[1], s.grid_offset[2] + self.forward_speed * s.offset_factor];
        let root = self.root;
        let out = |local: [f32; 3]| out_from(root, local);
        let down = |from: [f32; 3]| cast_from(root, from, [from[0], from[1] - RAY, from[2]]);

        // The obstacle. With no heaviest point the game works from a blank
        // one at the root, which never comes to a height any move takes.
        self.obstacle = Obstacle::default();
        self.heaviest = heaviest.map(|i| self.tops[i]);
        if let Some(index) = heaviest {
            let p = self.tops[index];
            let mut o = Obstacle { terrain: world.terrain(p.world), ..Obstacle::default() };
            let roof = |o: &mut Obstacle, at: [f32; 3]| {
                if let Some(up) = world.line(at, [at[0], at[1] + ROOF_RAY, at[2]]) {
                    o.roof = Some(o.roof.map_or(up, |least| least.min(up)));
                }
            };
            // How far the ground behind a point is under it, as a figure.
            let behind = |at: [f32; 3]| -> f32 {
                let ahead = if sprinting { BEHIND_SPRINT } else { at[2] + BEHIND_ON };
                let from = [at[0], at[1] + BEHIND_UP, ahead];
                down(out(from)).map_or(0.0, |hit| hit.local[1] / (from[1] - hit.local[1]) - 1.0)
            };
            roof(&mut o, p.world);
            let same = |q: &Point| (q.local[1] - p.local[1]).abs() <= SAME_TOP;
            let walk = |o: &mut Obstacle, q: &Point| -> f32 {
                o.height = o.height.max(q.local[1]).max(p.local[1]);
                o.distance = p.local[2];
                roof(o, q.world);
                o.behind = behind(q.local);
                (q.local[2] - p.local[2]).abs()
            };
            // Onward, then back toward the character: the two lengths add.
            for q in self.tops[index + 1..].iter().take_while(|q| same(q)) {
                o.length = walk(&mut o, q);
            }
            let mut near = 0.0;
            for q in self.tops[..index].iter().rev().take_while(|q| same(q)) {
                near = walk(&mut o, q);
            }
            o.length += near;
            if o.length == 0.0 {
                // A point alone is an obstacle one step deep.
                let step = s.grid_step[2];
                o.length = step;
                o.height = o.height.max(p.local[1]);
                o.distance = p.local[2];
                roof(&mut o, p.world);
                o.behind = behind([p.local[0], p.local[1], p.local[2] + step]);
            }
            self.obstacle = o;
        }

        // Stairs: the average rise between neighbours of a row of rays down.
        let rays = (STAIRS_BACK / STAIRS_STEP * 1.8).round() as usize;
        let ground = |k: usize| down(out([0.0, STAIRS_UP, -STAIRS_BACK + STAIRS_STEP * k as f32])).map(|p| p.world[1]);
        self.stairs = rays >= 2
            && (1..rays)
                .map(|k| Some((ground(k)? - ground(k - 1)?) / STAIRS_STEP))
                .sum::<Option<f32>>()
                .is_some_and(|rise| (rise / rays as f32) > STAIRS_SLOPES[0] && (rise / rays as f32) < STAIRS_SLOPES[1]);
    }

    /// Takes a measurement made elsewhere in place of one of its own: the
    /// obstacle, and the forward speed the grid's root was placed with. A
    /// replay gives the game's own this way (the recorder writes what the
    /// game's grid found), since a trace does not hold the map the rays
    /// were cast into. The checks that go by the rays' own points (what
    /// stands before and behind the obstacle, stairs) have nothing to go
    /// by then and pass.
    pub fn take(&mut self, s: &VaultingSettings, obstacle: Obstacle, forward_speed: f32) {
        self.forward.clear();
        self.tops.clear();
        self.heaviest = None;
        self.stairs = false;
        self.forward_speed = forward_speed.max(0.0);
        self.root = [s.grid_offset[0], s.grid_offset[1], s.grid_offset[2] + self.forward_speed * s.offset_factor];
        self.obstacle = obstacle;
    }

    /// Where the heaviest top point of the last measurement lies, in the
    /// world's axes (`MaxWeightPointPosition`): what the vault's sounds
    /// cast down onto for the obstacle's surface. None after a measurement
    /// handed in from a recording.
    pub fn heaviest_point(&self) -> Option<[f32; 3]> {
        self.heaviest.map(|point| point.world)
    }

    /// Whether the character stood on stairs at the last measurement.
    pub fn on_stairs(&self) -> bool {
        self.stairs
    }

    /// Whether anything can be tried at all (`CanVaulting`).
    fn open(&self, s: &VaultingSettings, at: &Standing) -> bool {
        s.active && !at.vaulting && !at.backing && at.grounded && !self.obstacle.terrain
    }

    /// The move the key gets, if any (`GetVaultingStrategy`), from the
    /// obstacle as last measured.
    pub fn choose(&self, s: &VaultingSettings, at: &Standing) -> Option<Move> {
        if !self.open(s, at) || at.in_transition || at.jumping {
            return None;
        }
        let stairs = self.stairs;
        // A sprinting character vaults only where the ground beyond is
        // clearly lower, and never climbs.
        let shallow = at.sprinting && self.obstacle.behind > SPRINT_DROP;
        if at.can_vault && self.can_vault(s) && !shallow && !stairs {
            Some(Move::Vault)
        } else if at.can_climb && self.can_climb(s) && !at.sprinting && !stairs {
            Some(Move::Climb)
        } else {
            None
        }
    }

    fn within(&self, limits: &Limits) -> bool {
        let o = &self.obstacle;
        o.height >= limits.height[0]
            && o.height <= limits.height[1]
            && o.length >= limits.length[0]
            && o.length <= limits.length[1]
            && limits.distance >= o.distance
    }

    /// `VaultMoveModel.CanMove`.
    fn can_vault(&self, s: &VaultingSettings) -> bool {
        let room = PLAYER_HEIGHT * VAULT_ROOM;
        s.vault.active
            && self.within(&s.vault.limits)
            && self.obstacle.behind < VAULT_DROP
            && self.clear_behind(s, room)
            && self.roof_allows(room)
            && self.clear_before()
    }

    /// `ClimbMoveModel.CanMove`.
    fn can_climb(&self, s: &VaultingSettings) -> bool {
        s.climb.active && self.within(&s.climb.limits) && self.roof_allows(PLAYER_HEIGHT * CLIMB_ROOM) && self.clear_before()
    }

    fn roof_allows(&self, room: f32) -> bool {
        self.obstacle.roof.is_none_or(|roof| roof >= room)
    }

    /// Nothing stands before the obstacle: of the forward points, from the
    /// lowest ray up, the first at least as high as the heaviest point is
    /// no nearer than it.
    fn clear_before(&self) -> bool {
        let p = self.heaviest.map_or([0.0; 3], |p| p.local);
        self.forward.iter().find(|q| q.local[1] >= p[1]).is_none_or(|q| q.local[2] >= p[2])
    }

    /// Nothing stands behind it where a body would come down: no forward
    /// point just above the obstacle within reach beyond it.
    fn clear_behind(&self, s: &VaultingSettings, room: f32) -> bool {
        let o = &self.obstacle;
        let p = self.heaviest.map_or([0.0; 3], |p| p.local);
        // The gap looked through is wider over a low obstacle.
        let gap = lerp(room * 2.0, room, (o.height / s.vault.auto_limits.height[1]).clamp(0.0, 1.0));
        !self.forward.iter().any(|q| {
            let [_, y, z] = q.local;
            y > o.height * 1.1 && y < o.height + gap && z <= p[2] + o.length + LANDING_ROOM
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// This install's settings.
    pub(crate) fn settings() -> VaultingSettings {
        let limits = |distance, height, length| Limits { distance, height, length };
        VaultingSettings {
            active: true,
            input_time: 0.6,
            grid_size: [0.0, 1.8, 2.0],
            grid_step: [0.1, 0.1, 0.1],
            grid_offset: [0.0; 3],
            offset_factor: 0.1,
            vault: MoveSettings {
                active: true,
                max_without_hand: 0.65,
                max_one_hand: f32::INFINITY,
                speed: [1.25, 0.9],
                limits: limits(0.5, [0.25, 1.1], [0.0, 0.41]),
                auto_limits: limits(0.41, [0.25, 0.41], [0.0, 0.41]),
            },
            climb: MoveSettings {
                active: true,
                max_without_hand: 0.65,
                max_one_hand: 1.2,
                speed: [1.1, 0.8],
                limits: limits(0.5, [0.25, 1.31], [0.0, 10.0]),
                auto_limits: limits(0.41, [0.25, 0.41], [0.41, 10.0]),
            },
        }
    }

    /// Level ground with boxes on it, each from `near` to `far` ahead
    /// (along z), endlessly wide, from `low` to `high`.
    struct Boxes(Vec<[f32; 4]>);

    impl World for Boxes {
        fn ground_height(&self, _x: f32, _z: f32) -> f32 {
            0.0
        }

        fn line(&self, from: [f32; 3], to: [f32; 3]) -> Option<f32> {
            let length = (0..3).map(|k| (to[k] - from[k]).powi(2)).sum::<f32>().sqrt();
            let mut nearest: Option<f32> = (from[1] >= 0.0 && to[1] < 0.0).then(|| from[1] / (from[1] - to[1]));
            for [near, far, low, high] in &self.0 {
                let (mut enter, mut leave) = (0.0f32, 1.0f32);
                let mut misses = false;
                for (k, (a, b)) in [(2, (*near, *far)), (1, (*low, *high))] {
                    let (start, travel) = (from[k], to[k] - from[k]);
                    if travel.abs() < 1e-9 {
                        // A line along a face does not enter the box.
                        misses |= start <= a || start >= b;
                        continue;
                    }
                    let (t0, t1) = ((a - start) / travel, (b - start) / travel);
                    enter = enter.max(t0.min(t1));
                    leave = leave.min(t0.max(t1));
                }
                if !misses && enter <= leave && enter > 0.0 {
                    nearest = Some(nearest.map_or(enter, |n| n.min(enter)));
                }
            }
            nearest.map(|share| share * length)
        }

        fn terrain(&self, at: [f32; 3]) -> bool {
            at[1] < 1e-4
        }
    }

    const READY: Standing =
        Standing { vaulting: false, backing: false, grounded: true, in_transition: false, jumping: false, sprinting: false, can_vault: true, can_climb: true };

    fn measured(boxes: &[[f32; 4]], sprinting: bool) -> Vaulting {
        let mut v = Vaulting::default();
        v.measure(&settings(), &Boxes(boxes.to_vec()), [0.0; 3], 0.0, 0.0, sprinting);
        v
    }

    #[test]
    fn a_thin_wall_is_vaulted_and_a_deep_box_climbed() {
        let s = settings();
        // A wall 0.35 m thick and a metre high, 0.42 m ahead.
        let wall = measured(&[[0.42, 0.77, 0.0, 1.0]], false);
        let o = wall.obstacle;
        println!("wall: height {:.3}, length {:.3}, distance {:.3}, behind {:.3}", o.height, o.length, o.distance, o.behind);
        assert!((o.height - 1.0).abs() < 1e-4 && (o.distance - 0.5).abs() < 1e-4, "{o:?}");
        assert!((o.length - 0.2).abs() < 1e-4, "the rays at 0.5, 0.6 and 0.7 m stand on it: {o:?}");
        assert!((o.behind + 1.0).abs() < 1e-4, "level ground beyond: {o:?}");
        assert_eq!(wall.choose(&s, &READY), Some(Move::Vault));

        // A box a metre deep: too deep to vault, so it is climbed.
        let deep = measured(&[[0.42, 1.42, 0.0, 1.0]], false);
        let o = deep.obstacle;
        println!("box: height {:.3}, length {:.3}, distance {:.3}, behind {:.3}", o.height, o.length, o.distance, o.behind);
        assert!((o.length - 0.9).abs() < 1e-4, "{o:?}");
        assert_eq!(deep.choose(&s, &READY), Some(Move::Climb));

        // A ledge: the ground beyond is the top itself.
        let ledge = measured(&[[0.42, 9.0, 0.0, 1.0]], false);
        assert!((ledge.obstacle.behind - 1.0).abs() < 1e-4, "twice the height less one: {:?}", ledge.obstacle);
        assert_eq!(ledge.choose(&s, &READY), Some(Move::Climb));
    }

    #[test]
    fn what_refuses_a_move() {
        let s = settings();
        let wall = [0.42, 0.77, 0.0, 1.0];
        let stands = |change: fn(&mut Standing)| {
            let mut at = READY;
            change(&mut at);
            at
        };
        let v = measured(&[wall], false);
        assert_eq!(v.choose(&s, &stands(|at| at.vaulting = true)), None);
        assert_eq!(v.choose(&s, &stands(|at| at.backing = true)), None);
        assert_eq!(v.choose(&s, &stands(|at| at.grounded = false)), None);
        assert_eq!(v.choose(&s, &stands(|at| at.in_transition = true)), None);
        assert_eq!(v.choose(&s, &stands(|at| at.jumping = true)), None);
        assert_eq!(v.choose(&s, &stands(|at| at.can_vault = false)), Some(Move::Climb), "legs that cannot pay a vault may pay a climb");
        // Too far, too high, too low.
        assert_eq!(measured(&[[0.52, 0.87, 0.0, 1.0]], false).choose(&s, &READY), None, "0.6 m to the first ray on it");
        assert_eq!(measured(&[[0.42, 0.77, 0.0, 1.2]], false).choose(&s, &READY), Some(Move::Climb), "over a vault's height");
        assert_eq!(measured(&[[0.42, 0.77, 0.0, 1.4]], false).choose(&s, &READY), None, "over a climb's");
        assert_eq!(measured(&[[0.42, 0.77, 0.0, 0.2]], false).choose(&s, &READY), None, "a kerb");
        // Open ground has nothing of a height to take.
        assert_eq!(measured(&[], false).choose(&s, &READY), None);
        // And the same wall as part of the terrain is left alone.
        let mut hill = measured(&[wall], false);
        hill.obstacle.terrain = true;
        assert_eq!(hill.choose(&s, &READY), None);
        // A roof a metre and a half over the top lets a vault under and not a climb.
        let roofed = measured(&[[0.42, 1.42, 0.0, 1.0], [0.0, 3.0, 2.5, 2.6]], false);
        assert_eq!(roofed.obstacle.roof.map(|r| (r * 100.0).round()), Some(150.0));
        assert_eq!(roofed.choose(&s, &READY), None, "too deep to vault, too low a roof to climb");
        // Something standing just beyond the wall, above it: no vault.
        let blocked = measured(&[wall, [1.5, 1.7, 0.0, 1.8]], false);
        assert_eq!(blocked.choose(&s, &READY), Some(Move::Climb));
        // Something before the wall, higher than it.
        let behind_a_post = measured(&[wall, [0.2, 0.3, 0.0, 1.5]], false);
        assert_eq!(behind_a_post.choose(&s, &READY), None);
    }

    #[test]
    fn a_sprint_vaults_only_onto_lower_ground_and_never_climbs() {
        let s = settings();
        let sprinting = Standing { sprinting: true, ..READY };
        let wall = measured(&[[0.42, 0.77, 0.0, 1.0]], true);
        assert_eq!(wall.choose(&s, &sprinting), Some(Move::Vault), "level ground beyond is -1: {:?}", wall.obstacle);
        // Ground 0.6 m up beyond the wall, 2.6 m ahead: -0.33, a vault
        // walking and none sprinting.
        let step = [0.77, 9.0, 0.0, 0.6];
        let raised = measured(&[[0.42, 0.77, 0.0, 1.0], step], true);
        println!("behind in a sprint: {:.3}", raised.obstacle.behind);
        assert_eq!(raised.choose(&s, &sprinting), None);
        assert_eq!(measured(&[[0.42, 1.42, 0.0, 1.0]], true).choose(&s, &sprinting), None, "a sprint does not climb");
    }

    #[test]
    fn the_grid_is_put_ahead_by_the_speed() {
        let s = settings();
        let world = Boxes(vec![[0.92, 1.27, 0.0, 1.0]]);
        let mut v = Vaulting::default();
        // The first measurement is made from the feet and finds the wall
        // too far; it leaves the root 0.5 m ahead for a sprint's 5 m/s.
        v.measure(&s, &world, [0.0; 3], 0.0, 5.0, true);
        assert!((v.obstacle.distance - 1.0).abs() < 1e-4, "{:?}", v.obstacle);
        v.measure(&s, &world, [0.0; 3], 0.0, 5.0, true);
        assert!((v.obstacle.distance - 0.5).abs() < 1e-4, "{:?}", v.obstacle);
        assert_eq!(v.choose(&s, &Standing { sprinting: true, ..READY }), Some(Move::Vault));
    }

    #[test]
    fn stairs_are_not_climbed() {
        // Treads 0.2 m deep rising 0.15 m each, from 0.3 m behind the feet.
        let stairs: Vec<[f32; 4]> = (0..12).map(|i| [-0.3 + 0.2 * i as f32, 9.0, 0.0, 0.15 * (i + 1) as f32]).collect();
        let v = measured(&stairs, false);
        assert!(v.on_stairs());
        assert_eq!(v.choose(&settings(), &READY), None);
        assert!(!measured(&[[0.42, 0.77, 0.0, 1.0]], false).on_stairs());
    }
}
