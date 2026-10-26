//! A character no one is at the keys of, seen from outside with a firearm
//! in its hands (`docs/spec/body-hits.md`, "The dummy on the range";
//! `docs/spec/third-person.md`): the movement with the body's animator, the
//! weapon's motion in its third-person strategy, the firearm with its own
//! animator, the weapon's rig posed by that animator, and both arms solved
//! onto it; every frame as the player's own is run (the app's controller,
//! and the replay that sets a character seen from outside beside the
//! game's), on a pacer of its own. A host draws what it leaves
//! ([`PuppetPose`]): the body's skeleton, the rig's bones and the arms.

use anyhow::{Context, Result};
use trp_data::player::{effector, LimbData};
use trp_data::unity::extract::Hierarchy;

use crate::arms::{self, Placed};
use crate::math::{quat_axis, quat_mul, quat_rotate};
use crate::rig::{ArmNodes, SightNodes, WeaponRig};
use crate::{Firearm, FirearmEvent, Hands, HandsInput, Input, Movement, Pacer, PointOfView, State, World};

/// The bone the procedural motion moves, and the one it hangs from.
const MOVED: &str = "Weapon_root/Weapon_root_anim";
const PARENT: &str = "Weapon_root";
const RIGHT_MARKER: &str = "Weapon_root/Weapon_root_anim/weapon/weapon_R_IK_marker";
const LEFT_MARKER: &str = "Weapon_root/Weapon_root_anim/weapon/weapon_L_IK_marker";
const VEST_MARKER: &str = "Weapon_root/Weapon_root_anim/weapon/weapon_vest_IK_marker";

/// One arm of the rig: what its solver reads, the palm's bone, the bones
/// below the palm in the rig's order, and the finger pose of the grip it
/// takes, if any (the grip's own turn first).
struct RigHand {
    nodes: ArmNodes,
    fingers: Vec<usize>,
    grip_pose: Vec<[f32; 4]>,
}

/// What the character leaves of its last frame, for drawing, in the game's
/// axes.
#[derive(Debug, Clone)]
pub struct PuppetPose {
    /// The feet as drawn, and the view's yaw and pitch.
    pub position: [f32; 3],
    pub yaw: f32,
    pub pitch: f32,
    /// The body: where its skeleton hangs (the feet lifted by the lag in
    /// the body's height) and its turn, and each bone the animator gives in
    /// its parent (`Body::drawn_bones`).
    pub body_at: [f32; 3],
    pub body_turn: [f32; 4],
    pub bones: Vec<Placed>,
    /// The ribcage from the feet, in the frame turned with the view: where
    /// the rig hangs, turned by the view's pitch.
    pub ribcage: [f32; 3],
    /// Every bone of the rig in its parent, the moved bone where the
    /// weapon's motion and the body put it, the fingers on the grip.
    pub rig: Vec<Placed>,
    /// Each arm as its solver leaves it, right then left: the upper arm,
    /// the three forearm bones and the palm, in the rig's frame.
    pub arms: [[Placed; 5]; 2],
}

/// The character.
pub struct Puppet {
    pub movement: Movement,
    pub hands: Hands,
    pub firearm: Firearm,
    pacer: Pacer,
    /// The rig as the simulation reads it (the moved bone at rest), and as
    /// it is drawn.
    read: WeaponRig,
    drawn: WeaponRig,
    moved: usize,
    parent: usize,
    marker: Option<usize>,
    vest: Option<[usize; 2]>,
    sights: Option<SightNodes>,
    hands_rig: [RigHand; 2],
    limbs: [LimbData; 2],
    /// The animator's poses are drawn on curves without corners.
    smooth: bool,
    last: Option<PuppetPose>,
}

impl Puppet {
    /// A character with the firearm `firearm` (its motion `hands`, the
    /// weapon's assembled `hierarchy`), the arms' solver's settings, the
    /// player's frame rate and the engine's physics step and longest frame.
    /// It is seen from outside, with a bot's mask (the turn-away alone).
    #[allow(clippy::too_many_arguments)]
    pub fn new(mut movement: Movement, mut hands: Hands, mut firearm: Firearm, hierarchy: &Hierarchy, limbs: [LimbData; 2], cell_width: f32, pacer: Pacer, smooth: bool) -> Result<Self> {
        // The weapon in hand from the start, as the player's run begins.
        firearm.start_drawn();
        movement.body.set_point_of_view(PointOfView::ThirdPerson);
        movement.sounds.third_person = true;
        movement.body.set_weapon_width(cell_width);
        hands.set_point_of_view(PointOfView::ThirdPerson);
        hands.mask = effector::DRAW_DOWN;
        let rig = WeaponRig::new(hierarchy);
        let moved = rig.bone(MOVED).context("the weapon has no bone for the procedural motion")?;
        let parent = rig.bone(PARENT).context("the weapon has no root")?;
        let hand = |right: bool| -> Result<RigHand> {
            let nodes = ArmNodes::find(hierarchy, right).context("the weapon's rig lacks an arm's bones")?;
            let side = if right { 'R' } else { 'L' };
            let mut path = format!("Base Human{side}Collarbone/Base Human{side}Upperarm");
            for name in ["Forearm1", "Forearm2", "Forearm3", "Palm"] {
                path.push_str(&format!("/Base Human{side}{name}"));
            }
            let palm = rig.bone(&path).context("the weapon's rig lacks a palm")?;
            let below = |index: usize, ancestor: usize| {
                let mut at = index;
                while let Some(p) = hierarchy.nodes[at].parent {
                    if p == ancestor {
                        return true;
                    }
                    at = p;
                }
                false
            };
            let fingers = (palm + 1..hierarchy.nodes.len()).filter(|&i| below(i, palm)).collect();
            // An uncached grip keeps its finger pose as bones below it.
            let grip_pose = nodes
                .grip
                .map(|g| {
                    let stored = &hierarchy.nodes[g].grip.as_ref().expect("a grip").fingers;
                    if stored.is_empty() {
                        std::iter::once(g).chain((g + 1..hierarchy.nodes.len()).filter(|&i| below(i, g))).map(|i| hierarchy.nodes[i].rotation).collect()
                    } else {
                        stored.clone()
                    }
                })
                .unwrap_or_default();
            Ok(RigHand { nodes, fingers, grip_pose })
        };
        let hands_rig = [hand(true)?, hand(false)?];
        // The sight the weapon is aimed by, as its sights pick it.
        let sights = hands.sight_nodes().or_else(|| SightNodes::find(hierarchy));
        Ok(Self {
            movement,
            hands,
            firearm,
            pacer,
            marker: rig.bone(RIGHT_MARKER),
            vest: rig.bone(LEFT_MARKER).zip(rig.bone(VEST_MARKER)).map(|(left, vest)| [left, vest]),
            sights,
            read: rig.clone(),
            drawn: rig,
            moved,
            parent,
            hands_rig,
            limbs,
            smooth,
            last: None,
        })
    }

    /// The host has drawn a frame `dt` seconds after the last: the frames
    /// of the character's own that are due, the walk's keys held as
    /// `move_dir` (x right, y forward).
    pub fn step(&mut self, dt: f32, move_dir: [f32; 2], world: &dyn World) {
        for _ in 0..self.pacer.pass(dt) {
            let frame = self.pacer.frame();
            self.frame(frame.dt, frame.physics_steps, move_dir, world);
        }
    }

    /// What its last frame left, for drawing.
    pub fn pose(&self) -> Option<&PuppetPose> {
        self.last.as_ref()
    }

    /// One frame, as the app's controller runs the player's
    /// (`controller::step`) seen from outside.
    fn frame(&mut self, dt: f32, physics_steps: u32, move_dir: [f32; 2], world: &dyn World) {
        let firearm = &mut self.firearm;
        let input = Input {
            move_dir,
            aiming: firearm.aiming(),
            reloading: firearm.reloading(),
            hands_busy: firearm.hands_busy(),
            hands_idle: firearm.animator_idling(),
            hands_in_inventory: firearm.in_inventory(),
            weapon_occupied: firearm.occupied(),
            ..Input::default()
        };
        self.movement.frame(dt, &input, world);
        let m = &mut self.movement;
        let hands = &mut self.hands;
        let (sprint_started, aim_lowered) = (m.take_sprint_started(), m.take_aim_lowered());
        hands.weapon_flip_speed = m.settings.weapon_flip_speed;
        hands.set_systems(&m.systems);
        hands.set_health(m.conditions);
        for pose in m.take_poses() {
            hands.assign_pose(pose.as_ref());
        }
        if sprint_started {
            firearm.sprint_started();
        }
        if aim_lowered {
            firearm.lower_aim();
        }
        if m.long_fall() || m.take_aim_taken() {
            firearm.set_aim(false, m.can_aim());
        }
        let aiming = firearm.aiming();
        // The weapon's motion takes the aim on its frame (`Hands::aim_changed`).
        hands.aim_changed(aiming);
        hands.set_head_rotation(m.head_look.rotation(), m.head_look.control(), aiming);
        // The bone the moved bone hangs from, as the animator left it on
        // the frame before.
        hands.weapon_root = self.read.local(self.parent);
        for _ in 0..physics_steps {
            let footstep = m.take_footstep();
            hands.advance(
                self.pacer.physics_step(),
                &HandsInput {
                    yaw: m.yaw,
                    pitch: m.shown_pitch(),
                    motion: m.input_velocity(),
                    velocity: m.velocity,
                    speed: m.smoothed_speed,
                    walking: m.state == State::Move,
                    sprinting: m.state == State::Sprint,
                    idle: m.state == State::Idle,
                    footstep,
                    tilt: m.smoothed_tilt,
                    tilt_target: m.tilt,
                    pose_level: m.pose_level,
                    smoothed_pose_level: m.smoothed_pose_level,
                    aiming,
                    prone: m.prone,
                    crawling: m.state == State::ProneMove,
                    frame_dt: dt,
                    wind: m.wind(),
                },
            );
        }
        hands.turn_rig(m.yaw, m.shown_pitch());
        // The weapon's logic: no trigger, its animator on the engine's clock.
        firearm.set_aim_angle(m.pitch);
        firearm.set_drawn_smooth(self.smooth);
        for event in firearm.step_on_clock(dt, m.player_dt()) {
            match event {
                FirearmEvent::Shot { strength } => hands.shoot(strength),
                FirearmEvent::AutoFire(on) => hands.set_auto_fire(on),
                FirearmEvent::BodyAction(action) => m.body.set_weapon_action(action),
                _ => {}
            }
        }
        // The rig as the animator has it: read with the moved bone at rest,
        // drawn from the animator's pose for drawing.
        self.read.pose(firearm.animator().pose(), Some(self.moved));
        self.drawn.pose(&firearm.animator().drawn_pose(), Some(self.moved));
        let parent = self.read.local(self.parent);
        hands.weapon_root = parent;
        let (moved_at, moved_turn) = self.read.placed(self.moved);
        let back = [-moved_turn[0], -moved_turn[1], -moved_turn[2], moved_turn[3]];
        let within = |rig: &WeaponRig, bone: usize| {
            let (at, turn) = rig.placed(bone);
            (quat_rotate(back, std::array::from_fn(|k| at[k] - moved_at[k])), quat_mul(back, turn))
        };
        let marker = self.marker.map_or(([0.0; 3], [0.0, 0.0, 0.0, 1.0]), |marker| within(&self.read, marker));
        // Seen from outside the body reaches for the vest during a reload
        // (none here) as the left hand's marker nears the weapon's.
        let reach = self.vest.map(|[left, vest]| {
            let (left, vest) = (self.read.placed(left).0, self.read.placed(vest).0);
            (0..3).map(|k| (left[k] - vest[k]).powi(2)).sum::<f32>().sqrt()
        });
        let reaching = firearm.reloading() && !m.sprinting;
        m.body.weigh_reach_for_vest(reach.is_some(), reach.filter(|_| reaching), dt);
        if let Some(sights) = &self.sights {
            let muzzle = sights.in_moved(&self.read).and_then(|(_, muzzle)| muzzle);
            hands.note_sights(sights.line(&self.read), muzzle);
        }
        // On the player's clock, as the game's weapon motion runs.
        hands.blend_poses(m.player_dt());
        let in_view = m.drawn_body_in_view();
        let weapon = hands.weapon_transform_carried(parent, m.body.carried(in_view), m.body.held(in_view), marker);
        hands.body_camera = Some(m.body.camera_bone());
        hands.frame_outside(dt);
        let elbows = [hands.elbow_shift(true), hands.elbow_shift(false)];
        // Last, how deep the weapon would stick into something.
        let depth = m.weapon_overlap(world, hands.weapon_line());
        hands.set_overlap(depth);
        let run_down = hands.turn_away_value();
        firearm.overlap(depth, run_down, m.can_aim());
        hands.aim_changed_after_frame(firearm.aiming(), dt);

        // What is drawn: the moved bone where the weapon's motion and the
        // body put it, the arms solved onto the rig so placed, the fingers
        // closed on the grip as the hand comes onto it.
        self.drawn.set_local(self.moved, weapon);
        let mut arms_out = [[([0.0; 3], [0.0, 0.0, 0.0, 1.0]); 5]; 2];
        let mut grips = [0.0f32; 2];
        for (side, right) in [true, false].into_iter().enumerate() {
            let body = m.body.arm_pose(right, in_view);
            let mut on_rig = self.drawn.arm(&self.hands_rig[side].nodes);
            on_rig.goal = std::array::from_fn(|k| on_rig.goal[k] + elbows[side][k]);
            let (bones, grip) = arms::hold(&body, &on_rig, &self.limbs[side]);
            arms_out[side] = bones;
            grips[side] = grip;
        }
        let mut rig: Vec<Placed> = (0..self.drawn.len()).map(|bone| self.drawn.local(bone)).collect();
        for (side, hand) in self.hands_rig.iter().enumerate() {
            if grips[side] <= 0.0 {
                continue;
            }
            for (finger, held) in hand.fingers.iter().zip(hand.grip_pose.iter().skip(1)) {
                if let Some(local) = rig.get_mut(*finger) {
                    local.1 = nlerp(local.1, *held, grips[side]);
                }
            }
        }
        let at = m.drawn_position();
        let turn = quat_mul(quat_axis(1, m.drawn_body_yaw()), m.body_tilt);
        let ribcage = quat_rotate(quat_axis(1, m.drawn_body_yaw()), m.ribcage());
        self.last = Some(PuppetPose {
            position: at,
            yaw: m.yaw,
            pitch: m.shown_pitch(),
            body_at: [at[0], at[1] + m.body_height_offset, at[2]],
            body_turn: turn,
            bones: m.body.drawn_bones(),
            ribcage: quat_rotate(quat_axis(1, -m.yaw), ribcage),
            rig,
            arms: arms_out,
        });
    }

}

/// Unity's `Quaternion.Lerp` (normalised, the shorter way).
fn nlerp(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    let dot = a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3];
    let s = if dot < 0.0 { -1.0 } else { 1.0 };
    let q: [f32; 4] = std::array::from_fn(|k| a[k] * (1.0 - t) + b[k] * s * t);
    let n = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
    if n > 0.0 { q.map(|x| x / n) } else { a }
}
