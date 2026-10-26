//! Room to move (movement spec, "Room to move"): the checks the game makes
//! before a step aside, a roll, lying down and standing up, and while
//! crawling, going down or turning a lying body: a box where the body would
//! go, rays down for steady ground to that side, and spheres for the length
//! a lying body needs (`MovementContext.OverlapOrHasNoGround`, `HasGround`,
//! `LegacyTwoPointSteadyGroundCheck`, `CanRoll`, `CanProne`,
//! `CanProneCheck`, `CanStandAt`, `LimitProneMotion`, `IsAbleToRotateProne`,
//! `GetPenetrationDepth`; `Transit2ProneState.ProcessAnimatorMovement`). Each
//! is asked of the host's world (`World::box_overlaps`, `sphere_cast`,
//! `ray`). Read from the game's code; no recording has a refusal
//! (`PROVISIONAL`, in the spec).

use super::slope::{cross, normalized};
use super::{Movement, World};
use crate::math::{lerp, quat_axis, quat_mul, quat_rotate};
use crate::mounting::Layer;

/// The capsule's radius (`CharacterControllerSpawner`; movement spec,
/// "What is asked of a host's world").
const RADIUS: f32 = 0.366;

/// Ground counts as steady where its upward part is at least this (about
/// 45 degrees); where the first ray finds less, a second one 0.1 m along
/// the body's forward is asked (`LegacyTwoPointSteadyGroundCheck`).
const STEADY: f32 = 0.707;
const SECOND_RAY: f32 = 0.1;

/// How much further than the capsule's half height the rays for steady
/// ground reach under the capsule's middle, less what a slope adds
/// (`OverlapOrHasNoGround`, `HasGround`: `height / 2 + 0.3 + (1 - normal.y)`).
const UNDER_MIDDLE: f32 = 0.3;

/// A step aside: a box 0.3 m deep beside the capsule, 0.2 m across either
/// way of its middle and a third of the capsule's height either way of
/// its middle (`IdlePlayerState.SetStep`).
const STEP_DEPTH: f32 = 0.3;
const STEP_WIDTH: f32 = 0.2;
const STEP_HEIGHT_SHARE: f32 = 3.0;

/// Lying down (`CanProne`): ground no steeper than 30 degrees; spheres of
/// 0.25 m from 0.4 m over the feet swept 1.3 m ahead and behind, a way
/// with nothing in it counting as 1.55 m; at least 0.5 m ahead and three
/// times the frame's motion ahead, at least 1.7 m in all; and ground within
/// 0.6 m under two points ahead and behind, placed by what the spheres found.
const PRONE_STEEPEST: f32 = 30.0;
const PRONE_FROM: f32 = 0.4;
const PRONE_SPHERE: f32 = 0.25;
const PRONE_SWEEP: f32 = 1.3;
const PRONE_CLEAR: f32 = 1.55;
const PRONE_AHEAD: f32 = 0.5;
const PRONE_MOTION_AHEAD: f32 = 3.0;
const PRONE_LENGTH: f32 = 1.7;
const PRONE_BEHIND_POINT: f32 = 0.6;
const PRONE_AHEAD_POINT: f32 = 0.4;
const PRONE_POINTS_SHIFT: f32 = 1.1;
const PRONE_GROUND: f32 = 0.6;

/// Standing at a stance (`CanStandAt`): a box over the feet 0.21 m out to
/// each side, for a height running from 0.875 (crouched) to 1.15
/// (standing), its middle at three quarters of that height and its half
/// height five eighths of it.
const STAND_HEIGHTS: [f32; 2] = [0.875, 1.15];
const STAND_MIDDLE: f32 = 0.75;
const STAND_HALF_HEIGHT: f32 = 1.25 / 2.0;
const STAND_HALF_WIDTH: f32 = 0.21;

/// Crawling and lying still (`LimitProneMotion`): steady ground 0.1 m out
/// to the side moved toward and 0.6 m out along the body, each ray 0.15 m
/// longer than the plain reach; motion toward ground that is not there is
/// turned back.
const CRAWL_SIDE: f32 = 0.1;
const CRAWL_ALONG: f32 = 0.6;
const CRAWL_EXTRA: f32 = 0.15;
/// Motion shorter than this along an axis is not looked at.
const CRAWL_LEAST: f32 = 0.0001;

/// Turning a lying body (`IsAbleToRotateProne`): the body as twice the turn
/// would leave it needs steady ground 0.55 m ahead and 0.75 m behind, each
/// ray 0.15 m longer.
const TURN_AHEAD: f32 = 0.55;
const TURN_BEHIND: f32 = 0.75;
const TURN_EXTRA: f32 = 0.15;
const TURN_PREDICTED: f32 = 2.0;

/// Going down (`Transit2ProneState`): a box 0.75 m deep ahead (a quarter
/// of the capsule's height either way, half the radius across, rays 0.2 m
/// longer) and one behind (0.25 m across, rays 0.15 m longer); against
/// either the character is pushed the other way by how far the line from
/// its middle along that way goes into something within the radius and
/// 0.35 m (0.4 behind), never less than a centimetre, eased at 10 (7)
/// a second.
const DOWN_DEPTH: f32 = 0.75;
const DOWN_HEIGHT_SHARE: f32 = 4.0;
const DOWN_AHEAD_EXTRA: f32 = 0.2;
const DOWN_BEHIND_WIDTH: f32 = 0.25;
const DOWN_BEHIND_EXTRA: f32 = 0.15;
const DOWN_AHEAD_REACH: f32 = 0.35;
const DOWN_BEHIND_REACH: f32 = 0.4;
const DOWN_LEAST: f32 = 0.01;
const DOWN_AHEAD_EASE: f32 = 10.0;
const DOWN_BEHIND_EASE: f32 = 7.0;

fn add(a: [f32; 3], b: [f32; 3], scale: f32) -> [f32; 3] {
    std::array::from_fn(|k| a[k] + b[k] * scale)
}

impl Movement {
    /// The capsule's middle in the world (`PlayerColliderCenter`).
    pub(super) fn collider_centre(&self) -> [f32; 3] {
        let [_, middle] = self.capsule_figures();
        [self.position[0], self.position[1] + middle, self.position[2]]
    }

    /// The way the body faces, level (`PlayerTransform.forward`).
    fn body_forward(&self) -> [f32; 3] {
        let (sin, cos) = self.body_yaw.to_radians().sin_cos();
        [sin, 0.0, cos]
    }

    /// The body's forward laid along the ground under it
    /// (`ForwardProjectionOnSurface`), for a forward of `forward`.
    fn on_surface(&self, forward: [f32; 3]) -> [f32; 3] {
        normalized(cross([forward[2], 0.0, -forward[0]], self.surface))
    }

    /// The body as the game turns its boxes (`AnimatedTransform.rotation`):
    /// its yaw and, lying down, its lay on the ground.
    fn body_turn(&self) -> [f32; 4] {
        quat_mul(quat_axis(1, self.body_yaw), self.body_tilt)
    }

    /// Whether there is steady ground under `origin` within `reach`
    /// (`LegacyTwoPointSteadyGroundCheck`, which answers the other way).
    fn steady_ground(&self, world: &dyn World, origin: [f32; 3], reach: f32) -> bool {
        let down = [0.0, -1.0, 0.0];
        let upward = |from| world.ray(from, down, reach, Layer::Static).map_or(0.0, |hit| hit.normal[1]);
        let mut found = upward(origin);
        if found < STEADY {
            found = upward(add(origin, self.body_forward(), SECOND_RAY));
        }
        found >= STEADY
    }

    /// How far the rays for steady ground reach under the capsule's middle.
    fn under_middle(&self) -> f32 {
        let [height, _] = self.capsule_figures();
        height / 2.0 + UNDER_MIDDLE + (1.0 - self.surface[1])
    }

    /// Whether something is in a box beside the capsule toward `axis`, or
    /// there is no steady ground under it (`OverlapOrHasNoGround`): the box
    /// `depth` deep out from the capsule's side, `width` across either way
    /// of its middle (half the radius at none), the capsule's height over
    /// `height_share` either way, turned as the body is.
    fn overlap_or_no_ground(&self, world: &dyn World, depth: f32, axis: [f32; 3], width: f32, height_share: f32, extra: f32) -> bool {
        let middle = add(self.collider_centre(), axis, RADIUS + depth / 2.0);
        let width = if width > 0.0 { width } else { RADIUS / 2.0 };
        let [height, _] = self.capsule_figures();
        world.box_overlaps(middle, [width, height / height_share, depth / 2.0], self.body_turn())
            || !self.steady_ground(world, middle, self.under_middle() + extra)
    }

    /// Whether there is steady ground `depth` out from the capsule's side
    /// toward `axis` (`HasGround`).
    fn has_ground(&self, world: &dyn World, depth: f32, axis: [f32; 3], extra: f32) -> bool {
        let origin = add(self.collider_centre(), axis, RADIUS + depth / 2.0);
        self.steady_ground(world, origin, self.under_middle() + extra)
    }

    /// Whether a step aside to `side` (1 right, -1 left) has room
    /// (`IdlePlayerState.SetStep`): nothing in the way and steady ground there.
    pub(super) fn room_to_step(&self, side: i32, world: &dyn World) -> bool {
        let forward = self.body_forward();
        let right = [forward[2], 0.0, -forward[0]];
        let sign = if side > 0 { 1.0 } else { -1.0 };
        !self.overlap_or_no_ground(world, STEP_DEPTH, right.map(|k| k * sign), STEP_WIDTH, STEP_HEIGHT_SHARE, 0.0)
    }

    /// Whether a roll to `side` has room (`CanRoll`): steady ground under a
    /// point beside the ribcage, and nothing in the box there, turned to
    /// the view's yaw and the ground's pitch.
    pub(super) fn room_to_roll(&self, side: i32, world: &dyn World) -> bool {
        let s = &self.settings;
        let mut shift = s.roll_check_shift;
        shift[0] = if side > 0 { shift[0].abs() } else { -shift[0].abs() };
        let turn = quat_mul(quat_axis(1, self.yaw), quat_axis(0, self.surface_pitch));
        let ribcage = self.position[1] + self.ribcage()[1];
        let middle = add([self.position[0], ribcage, self.position[2]], quat_rotate(turn, shift), 1.0);
        self.steady_ground(world, middle, s.roll_check_cast) && !world.box_overlaps(middle, s.roll_check_size.map(|k| k / 2.0), turn)
    }

    /// The room a body lying down needs (`CanProne`, less health's part,
    /// which `Conditions::can_prone` has): ground no steeper than 30
    /// degrees, length enough ahead and behind, and ground under both ends.
    pub(super) fn room_to_lie(&self, world: &dyn World) -> bool {
        if self.surface[1].clamp(-1.0, 1.0).acos().to_degrees() > PRONE_STEEPEST {
            return false;
        }
        let forward = self.on_surface(self.body_forward());
        let back = forward.map(|k| -k);
        let from = [self.position[0], self.position[1] + PRONE_FROM, self.position[2]];
        let ahead = world.sphere_cast(from, PRONE_SPHERE, forward, PRONE_SWEEP).map_or(PRONE_CLEAR, |d| d.min(PRONE_CLEAR));
        // The frame's motion along the body, as the animator gives it.
        let motion = self.animated_motion;
        let along = motion[0] * self.body_forward()[0] + motion[2] * self.body_forward()[2];
        if ahead < PRONE_AHEAD + along * PRONE_MOTION_AHEAD {
            return false;
        }
        let behind = world.sphere_cast(from, PRONE_SPHERE, back, PRONE_SWEEP).map_or(PRONE_CLEAR, |d| d.min(PRONE_CLEAR));
        if ahead + behind < PRONE_LENGTH {
            return false;
        }
        let shift = (PRONE_POINTS_SHIFT - behind).clamp(0.0, 1.0);
        let tail = add(from, forward, -(behind.clamp(0.0, PRONE_BEHIND_POINT) - shift));
        let head = add(from, forward, ahead.clamp(0.0, PRONE_AHEAD_POINT) + shift);
        let down = [0.0, -1.0, 0.0];
        [tail, head].iter().all(|point| world.ray(*point, down, PRONE_GROUND, Layer::Static).is_some())
    }

    /// Whether the character may stand at stance `level` (`CanStandAt`):
    /// lower than it stands now (and not lying) always; else a box over
    /// the feet for that stance's height has to be clear. Lying with
    /// medicine at work, never.
    pub(super) fn can_stand_at(&self, level: f32, world: &dyn World) -> bool {
        if self.prone && self.conditions.using_meds {
            return false;
        }
        if level < self.pose_level && !self.prone {
            return true;
        }
        let height = lerp(STAND_HEIGHTS[0], STAND_HEIGHTS[1], level);
        let middle = [self.position[0], self.position[1] + height * STAND_MIDDLE, self.position[2]];
        let half = [STAND_HALF_WIDTH, height * STAND_HALF_HEIGHT, STAND_HALF_WIDTH];
        !world.box_overlaps(middle, half, quat_axis(1, self.body_yaw))
    }

    /// A lying body's motion with what goes toward ground that is not there
    /// turned back (`LimitProneMotion`): across the body by steady ground
    /// 0.1 m out to that side, along it by steady ground 0.6 m out that way.
    pub(super) fn limit_prone_motion(&self, motion: [f32; 3], world: &dyn World) -> [f32; 3] {
        if (motion[0] * motion[0] + motion[1] * motion[1] + motion[2] * motion[2]).sqrt() <= f32::EPSILON {
            return motion;
        }
        let forward = self.body_forward();
        let right = [forward[2], 0.0, -forward[0]];
        // In the body's own axes: x across, z along.
        let (mut x, y, mut z) = (motion[0] * right[0] + motion[2] * right[2], motion[1], motion[0] * forward[0] + motion[2] * forward[2]);
        let mut turned = false;
        if x.abs() > CRAWL_LEAST && !self.has_ground(world, CRAWL_SIDE, right.map(|k| k * x.signum()), CRAWL_EXTRA) {
            x = -x;
            turned = true;
        }
        if z.abs() > CRAWL_LEAST {
            let along = normalized(cross(right, self.surface));
            if !self.has_ground(world, CRAWL_ALONG, along.map(|k| k * z.signum()), CRAWL_EXTRA) {
                z = -z;
                turned = true;
            }
        }
        if !turned {
            return motion;
        }
        [right[0] * x + forward[0] * z, y, right[2] * x + forward[2] * z]
    }

    /// Whether a lying body may turn by `yaw` degrees
    /// (`IsAbleToRotateProne`, the ground's half): the body as twice the
    /// turn would leave it needs steady ground ahead and behind. No turn
    /// at all is refused, as the game's answer is (nothing is lost by it).
    pub(super) fn able_to_turn_prone(&self, yaw: f32, world: &dyn World) -> bool {
        if yaw.abs() <= f32::EPSILON {
            return false;
        }
        let (sin, cos) = (self.body_yaw + TURN_PREDICTED * yaw).to_radians().sin_cos();
        let forward = self.on_surface([sin, 0.0, cos]);
        self.has_ground(world, TURN_AHEAD, forward, TURN_EXTRA) && self.has_ground(world, TURN_BEHIND, forward.map(|k| -k), TURN_EXTRA)
    }

    /// How far the line from the capsule's middle along `axis` goes into
    /// something within the radius and `extra` (`GetPenetrationDepth`).
    fn penetration_depth(&self, world: &dyn World, axis: [f32; 3], extra: f32) -> f32 {
        let reach = RADIUS + extra;
        world.ray(self.collider_centre(), axis, reach, Layer::Static).map_or(0.0, |hit| reach - hit.distance)
    }

    /// Going down, the motion with the push back from something ahead or
    /// behind (`Transit2ProneState.ProcessAnimatorMovement`): the motion
    /// along the body is replaced by the eased push, and the view's yaw is
    /// put back as the frame found it. `None` where nothing is in the way.
    pub(super) fn going_down_push(&mut self, motion: [f32; 3], dt: f32, world: &dyn World) -> Option<[f32; 3]> {
        let forward = self.on_surface(self.body_forward());
        let back = forward.map(|k| -k);
        let along = if self.overlap_or_no_ground(world, DOWN_DEPTH, forward, 0.0, DOWN_HEIGHT_SHARE, DOWN_AHEAD_EXTRA) {
            let depth = self.penetration_depth(world, forward, DOWN_AHEAD_REACH).max(DOWN_LEAST);
            self.down_push = lerp(self.down_push, depth, dt * DOWN_AHEAD_EASE);
            -self.down_push
        } else if self.overlap_or_no_ground(world, DOWN_DEPTH, back, DOWN_BEHIND_WIDTH, DOWN_HEIGHT_SHARE, DOWN_BEHIND_EXTRA) {
            let depth = self.penetration_depth(world, back, DOWN_BEHIND_REACH).max(DOWN_LEAST);
            self.down_push = lerp(self.down_push, depth, dt * DOWN_BEHIND_EASE);
            self.down_push
        } else {
            return None;
        };
        self.yaw = self.yaw_before_turn;
        let body = self.body_forward();
        let right = [body[2], 0.0, -body[0]];
        let x = motion[0] * right[0] + motion[2] * right[2];
        Some([right[0] * x + body[0] * along, motion[1], right[2] * x + body[2] * along])
    }
}
