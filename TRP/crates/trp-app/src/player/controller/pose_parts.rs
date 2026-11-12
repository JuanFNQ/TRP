//! The shown pose, body pose, springs, mounts and the pending changes the controller keeps.

use super::*;

pub(super) const NO_TURN: [f32; 4] = [0.0, 0.0, 0.0, 1.0];

/// The kit's rattle the sights brought up or down ask for
/// (`FirearmController.SetAim`: `PlayGearSound(0.2f)`).
pub(super) const AIM_KIT: f32 = 0.2;

/// What is drawn of the simulation now.
#[derive(Clone, Copy)]
pub struct Shown {
    /// The player's feet, in Bevy's axes.
    pub position: Vec3,
    /// Where the body's ribcage is from the feet, in the frame turned
    /// with the view as shown. The first-person rig and the camera hang
    /// there, so the eye's height, its bob with each stride, its drop
    /// when crouching and its shift when leaning all come from the body's
    /// animation. That animation also turns and bends the upper body to
    /// the view, legs standing still, so the ribcage goes round the feet
    /// with the view's yaw and forward and down with its pitch; in the
    /// game it does so a frame behind the view. It is drawn so here: where
    /// the frames drawn from have it, and moved by what the view's turning
    /// since those frames makes of it (`Controller::settle_shown`).
    pub ribcage: Vec3,
    /// The weapon's moved bone in its parent, in the game's axes.
    pub weapon: ([f32; 3], [f32; 4]),
    /// The camera beside the weapon's root, in the game's axes.
    pub camera: CameraPose,
    /// How far the camera's own object is from the ribcage, in the frame
    /// the rig hangs in and the game's axes (`Movement::camera_container`):
    /// nothing but where a clip shifts the ribcage on its spine, as blind
    /// fire's do. The camera's place is from there.
    pub camera_apart: [f32; 3],
    /// How far the rig's origin is from the ribcage, in the same frame and
    /// axes: nothing but on a mount, which moves the weapon onto its point
    /// (`Movement::mounted_rig_apart`; mounting spec, "The weapon's place").
    pub rig_apart: [f32; 3],
    /// Each arm, right then left, as the body's animation has it, with
    /// how much of it that animation claims (a crawl claims both whole),
    /// in the frame the rig hangs in and the game's axes. The arm's
    /// solver starts from it.
    pub arms: [ArmPose; 2],
    /// What the turn-away from an obstacle adds to the weapon's goal for
    /// each elbow, right then left, in the same frame and axes.
    pub elbows: [[f32; 3]; 2],
}

/// The body's skeleton on one frame of the simulation, for a body that is
/// drawn: where it hangs (the feet, lifted by the lag in the body's
/// height, and turned as the body faces and lies), in Bevy's axes, and
/// each bone asked of the animator in its parent, in the game's.
#[derive(Clone)]
pub struct BodyPose {
    pub at: Vec3,
    pub turn: Quat,
    pub bones: Vec<([f32; 3], [f32; 4])>,
}

impl BodyPose {
    pub(super) fn of(m: &Movement) -> Option<Self> {
        let bones = m.body.drawn_bones();
        if bones.is_empty() {
            return None;
        }
        let at = m.drawn_position();
        let turn = quat_mul(quat_axis(1, m.drawn_body_yaw()), m.body_tilt);
        Some(Self { at: Vec3::new(at[0], at[1] + m.body_height_offset, -at[2]), turn: convert::rotation(turn), bones })
    }

    pub(super) fn blend(&self, to: &Self, t: f32) -> Self {
        let lerp = |a: [f32; 3], b: [f32; 3]| -> [f32; 3] { std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t) };
        Self {
            at: self.at.lerp(to.at, t),
            turn: self.turn.slerp(to.turn, t),
            bones: self.bones.iter().zip(&to.bones).map(|(a, b)| (lerp(a.0, b.0), quat_lerp(a.1, b.1, t))).collect(),
        }
    }
}

/// A look into the chamber asked of the weapon in hand; whether it began
/// with no malfunction (`Idling.CheckChamber`: with none, the look and the
/// chamber known, `InventoryController.CheckChamber(weapon, true)`; with
/// one, its repair or its reminder, the chamber as it was).
pub(super) fn look_into_chamber(firearm: &mut trp_sim::weapons::firearm::Firearm) -> bool {
    let clear = firearm.is_idling() && firearm.malfunction() == trp_sim::weapons::malfunction::Malf::None;
    firearm.check_chamber();
    clear && !firearm.is_idling()
}

/// An arm between two of the simulation's frames.
pub(super) fn blend_arm(a: ArmPose, b: ArmPose, t: f32) -> ArmPose {
    let lerp = |a: f32, b: f32| a + (b - a) * t;
    let lerp3 = |a: [f32; 3], b: [f32; 3]| -> [f32; 3] { std::array::from_fn(|i| lerp(a[i], b[i])) };
    ArmPose {
        hand: lerp(a.hand, b.hand),
        shoulder: lerp(a.shoulder, b.shoulder),
        elbow: lerp(a.elbow, b.elbow),
        bend_goal: lerp3(a.bend_goal, b.bend_goal),
        upper: (lerp3(a.upper.0, b.upper.0), quat_lerp(a.upper.1, b.upper.1, t)),
        below: std::array::from_fn(|i| (lerp3(a.below[i].0, b.below[i].0), quat_lerp(a.below[i].1, b.below[i].1, t))),
        fingers: a.fingers.zip(b.fingers).map(|(a, b)| std::array::from_fn(|i| quat_lerp(a[i], b[i], t))),
    }
}

/// What the movement and the body's animation leave of one frame of the
/// simulation. They change on every frame, so what is drawn of them is
/// taken between two frames next to each other: a leg in a sprint is up to
/// 25 mm from its path, on every other frame, if it is taken between
/// frames a physics step apart.
#[derive(Clone)]
pub(super) struct Stepped {
    pub(super) position: Vec3,
    /// The ribcage from the feet, in the world's frame and the game's
    /// axes, and the view (yaw and pitch) the body's pose was made for.
    pub(super) ribcage: [f32; 3],
    /// The camera's own object from the ribcage, in the rig's frame
    /// (`Shown::camera_apart`).
    pub(super) camera_apart: [f32; 3],
    /// The rig's origin from the ribcage (`Shown::rig_apart`).
    pub(super) rig_apart: [f32; 3],
    /// The mount, while there is one.
    pub(super) mount: Option<MountDrawn>,
    pub(super) posed_for: [f32; 2],
    pub(super) arms: [ArmPose; 2],
    /// The body's skeleton, where the body is drawn.
    pub(super) body: Option<BodyPose>,
    /// Every bone of the rig in the hands as its animator left it on this
    /// frame, each in its parent and the game's axes.
    pub(super) rig: Option<std::sync::Arc<Vec<([f32; 3], [f32; 4])>>>,
    /// The same rig as the animator's straight pose has it, where it is
    /// drawn smooth (`SMOOTH ANIMATIONS`; none where the two are the same):
    /// what a death is drawn by (`Controller::rig_shown`).
    pub(super) rig_straight: Option<std::sync::Arc<Vec<([f32; 3], [f32; 4])>>>,
}

impl Stepped {
    pub(super) fn of(m: &Movement) -> Self {
        // The body's animation gives the ribcage in the body's axes; the
        // body may face a little away from the view.
        // The body faces as it is to be drawn: in a jump the game turns it
        // after the view on a clock that makes the turn uneven.
        let in_view = m.drawn_body_in_view();
        // Drawn where the frames' own time puts it: the game's clock for
        // the player makes a jump's travel uneven from frame to frame.
        let at = m.drawn_position();
        // The camera's own object beside the ribcage, into the frame the rig
        // hangs in (turned from the body to the view).
        let camera_apart = {
            let (container, ribcage) = (m.camera_container(), m.ribcage());
            let look = quat_mul(quat_axis(1, m.yaw - m.drawn_body_yaw()), quat_axis(0, m.pitch));
            quat_rotate([-look[0], -look[1], -look[2], look[3]], std::array::from_fn(|k| container[k] - ribcage[k]))
        };
        Self {
            // The simulation's forward is +z; Bevy's is -z.
            position: Vec3::new(at[0], at[1], -at[2]),
            ribcage: quat_rotate(quat_axis(1, m.drawn_body_yaw()), m.ribcage()),
            camera_apart,
            rig_apart: [0.0; 3],
            mount: None,
            posed_for: m.posed_for(),
            arms: [m.body.arm_pose(true, in_view), m.body.arm_pose(false, in_view)],
            body: BodyPose::of(m),
            rig: None,
            rig_straight: None,
        }
    }
}

/// Where a thrown grenade leaves the hand and how it is turned
/// (`trp_sim::grenade_hands::release`), in the world and the game's axes,
/// and the look on the frame it was thrown on: the throw's way is the
/// body's `WeaponRoot`'s down, which is the look (`loose::look_root`;
/// recorded, within 0.02 degrees in six throws).
#[derive(Debug, Clone, Copy)]
pub struct GrenadeRelease {
    pub at: [f32; 3],
    pub turn: [f32; 4],
    pub yaw: f32,
    pub pitch: f32,
    /// The eye on that frame, in the world (for the log).
    pub eye: [f32; 3],
}

/// A place and turn in the frame the rig in hand hangs in (its root's, the
/// game's axes), in the world as this frame of the simulation hangs the
/// rig: its root `rig_apart` from the ribcage in the frame turned with the
/// look, as `apply` and `hands_view::apply_hands` draw it, from this
/// frame's own figures.
pub(super) fn rig_in_world(m: &Movement, rig_apart: [f32; 3], (at, turn): ([f32; 3], [f32; 4])) -> ([f32; 3], [f32; 4]) {
    let feet = m.drawn_position();
    let ribcage = quat_rotate(quat_axis(1, m.drawn_body_yaw()), m.ribcage());
    let look = quat_mul(quat_axis(1, m.yaw), quat_axis(0, m.pitch));
    let hung = quat_rotate(look, std::array::from_fn(|k| rig_apart[k] + at[k]));
    (std::array::from_fn(|k| feet[k] + ribcage[k] + hung[k]), quat_mul(look, turn))
}

/// What the weapon's springs leave of a frame that had a physics step: the
/// weapon's moved bone, the camera, and the elbows' shifts.
#[derive(Clone, Copy)]
pub(super) struct Sprung {
    pub(super) weapon: ([f32; 3], [f32; 4]),
    pub(super) camera: CameraPose,
    pub(super) elbows: [[f32; 3]; 2],
    /// On a mount, how far the camera's place on that frame was moved with
    /// the rig (`Hands::set_rig_shift`: the mount's camera follows the rig
    /// across and up at once), so that it can be moved with the rig as
    /// drawn instead (`Controller::shown`).
    pub(super) shift: Option<[f32; 3]>,
}

impl Sprung {
    pub(super) fn blend(self, to: Self, t: f32) -> Self {
        let lerp = |a: [f32; 3], b: [f32; 3]| -> [f32; 3] { std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t) };
        Self {
            weapon: (lerp(self.weapon.0, to.weapon.0), quat_lerp(self.weapon.1, to.weapon.1, t)),
            camera: CameraPose {
                position: lerp(self.camera.position, to.camera.position),
                rotation: quat_lerp(self.camera.rotation, to.camera.rotation, t),
                field_of_view: self.camera.field_of_view + (to.camera.field_of_view - self.camera.field_of_view) * t,
            },
            elbows: [lerp(self.elbows[0], to.elbows[0]), lerp(self.elbows[1], to.elbows[1])],
            shift: self.shift.zip(to.shift).map(|(a, b)| lerp(a, b)),
        }
    }
}

/// A frame's mount, for drawing the rig onto its point
/// (`Movement::mounted_rig_apart`): the point in the world's frame and the
/// game's axes, the pivot in the rig's frame as the hands' animator had it,
/// and the transition's share.
#[derive(Clone, Copy)]
pub(super) struct MountDrawn {
    pub(super) point: [f32; 3],
    pub(super) pivot: [f32; 3],
    pub(super) progress: f32,
}

/// Input that arrives between frames of the simulation.
#[derive(Default, Clone)]
pub(super) struct Pending {
    pub(super) look: Vec2,
    /// What the mouse gave the head's free look, in degrees across and
    /// down, and whether the look's key came up.
    pub(super) head: Vec2,
    pub(super) head_reset: bool,
    pub(super) jump: bool,
    pub(super) crouch: bool,
    pub(super) prone: bool,
    pub(super) stance_delta: f32,
    pub(super) speed_delta: f32,
    pub(super) reload: bool,
    /// The reload key's double click (`QuickReloadWeapon`).
    pub(super) quick_reload: bool,
    pub(super) fire_mode: bool,
    pub(super) check_magazine: bool,
    pub(super) check_chamber: bool,
    pub(super) breath: bool,
    pub(super) fold_stock: bool,
    /// The weapon's three further keys: its fullest way of firing, the
    /// look at its selector, and the look all over.
    pub(super) force_auto: bool,
    pub(super) check_fire_mode: bool,
    pub(super) examine: bool,
    /// The sights' two keys (`ChangeAimScope`, `ChangeAimScopeMagnification`)
    /// and the wheel's turns for a variable scope's zoom since the
    /// simulation's last frame (optics spec, "The sights a weapon has",
    /// "Zoom and modes").
    pub(super) change_scope: bool,
    pub(super) magnification: bool,
    pub(super) zoom: f32,
    /// The zeroing keys (`OpticCalibrationSwitchUp`, `Down`; optics spec,
    /// "Zeroing").
    pub(super) zeroing_up: bool,
    pub(super) zeroing_down: bool,
    /// An item in use asked to stop (`MedsInputTranslator`): the shoot
    /// key, the aim key or the scope's key pressed.
    pub(super) use_cancel: bool,
    /// The walk key given, and taken back where it is bound to be held.
    pub(super) walk_toggle: bool,
    pub(super) walk_end: bool,
    /// The game's commands for the bindings that hold, as the keys gave
    /// them since the simulation's last frame (`trp_sim::Input`): the
    /// breath and the crouch key coming up, the sprint, the lean and the
    /// step aside given and taken back (left then right).
    pub(super) breath_end: bool,
    pub(super) restore_pose: bool,
    pub(super) sprint_toggle: bool,
    pub(super) sprint_end: bool,
    pub(super) lean_toggle: [bool; 2],
    pub(super) lean_end: [bool; 2],
    pub(super) step_toggle: [bool; 2],
    pub(super) step_end: [bool; 2],
    /// Blind fire's commands (`docs/spec/blind-fire.md`, "The keys"): the
    /// side given last (`ToggleBlindAbove` 1, `ToggleBlindRight` -1, 0
    /// none) and its keys come up (`BlindShootEnd`).
    pub(super) blind_toggle: i32,
    pub(super) blind_end: bool,
    /// The mount's key and the bipod's (`WeaponMounting`, `ToggleBipods`;
    /// mounting spec, "The keys").
    pub(super) mount: bool,
    pub(super) bipod: bool,
    /// The left shoulder's key (`LeftStance`, `Mouse4`; left-shoulder spec).
    pub(super) left_stance: bool,
    /// The aim's binding, in the order it came: given (true), which puts
    /// the sights the other way, or taken back, which puts them down.
    pub(super) aim: Vec<bool>,
    /// For a grenade in hand, the throws' two keys as they went down and
    /// came up, in order (`HighThrow` and `LowThrow`: the trigger's keys
    /// and the aim's, held; `TryHighThrow`, `FinishHighThrow` and the
    /// like): from below or not, and down or up. For a knife, the strokes
    /// asked for (`ToggleShooting` a slash, `ToggleAlternativeShooting` a
    /// stab).
    pub(super) throw_keys: Vec<(bool, bool)>,
    pub(super) strokes: Vec<trp_sim::Stroke>,
}

/// What the simulation did in the last drawn frame and what that cost, for
/// `--log-pacing`.
#[derive(Default, Clone, Copy)]
pub struct Paced {
    /// The host's frame time as the pacer was given it, in seconds.
    pub host_dt: f32,
    /// Frames of the simulation run, and the physics steps in them.
    pub frames: u32,
    pub physics_steps: u32,
    /// Seconds they took to work out.
    pub cost: f32,
    /// How far between the two frames drawn from.
    pub blend: f32,
}
