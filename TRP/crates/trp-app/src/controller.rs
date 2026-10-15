//! Drives the simulation from the keyboard and mouse and keeps what is to
//! be drawn of it: the player's place, the frame the view hangs in, the
//! weapon's moved bone and the camera.
//!
//! The simulation runs on frames of its own length (`trp_sim::Pacer`), so
//! nothing in it depends on the rate of drawing. What is drawn is the
//! simulation as it stood one physics step before the host's time: what
//! the springs move (the weapon, the camera) blended between the last two
//! of its frames that had a physics step, and what the movement and the
//! body's animation move (the feet's place, the ribcage's, the skeleton)
//! between the two of its frames either side of that moment. The view's
//! own turning is shown at once.

use std::collections::VecDeque;


use bevy::prelude::*;
use trp_sim::math::{delta_angle, quat_axis, quat_lerp, quat_mul, quat_rotate};
use trp_sim::{
    AimPoint, ArmPose, CameraPose, FireMode, FirearmEvent, HandsInput, Input as SimInput, Movement, Pacer, PointOfView, State,
};

use crate::range::Range;
use crate::weapon::{HandsRig, Player, RigPose, ViewPivot, WeaponRig};
use crate::{convert, Options, View};


const NO_TURN: [f32; 4] = [0.0, 0.0, 0.0, 1.0];

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
    fn of(m: &Movement) -> Option<Self> {
        let bones = m.body.drawn_bones();
        if bones.is_empty() {
            return None;
        }
        let at = m.drawn_position();
        let turn = quat_mul(quat_axis(1, m.drawn_body_yaw()), m.body_tilt);
        Some(Self { at: Vec3::new(at[0], at[1] + m.body_height_offset, -at[2]), turn: convert::rotation(turn), bones })
    }

    fn blend(&self, to: &Self, t: f32) -> Self {
        let lerp = |a: [f32; 3], b: [f32; 3]| -> [f32; 3] { std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t) };
        Self {
            at: self.at.lerp(to.at, t),
            turn: self.turn.slerp(to.turn, t),
            bones: self.bones.iter().zip(&to.bones).map(|(a, b)| (lerp(a.0, b.0), quat_lerp(a.1, b.1, t))).collect(),
        }
    }
}

/// An arm between two of the simulation's frames.
fn blend_arm(a: ArmPose, b: ArmPose, t: f32) -> ArmPose {
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
struct Stepped {
    position: Vec3,
    /// The ribcage from the feet, in the world's frame and the game's
    /// axes, and the view (yaw and pitch) the body's pose was made for.
    ribcage: [f32; 3],
    posed_for: [f32; 2],
    arms: [ArmPose; 2],
    /// The body's skeleton, where the body is drawn.
    body: Option<BodyPose>,
    /// Every bone of the rig in the hands as its animator left it on this
    /// frame, each in its parent and the game's axes.
    rig: Option<std::sync::Arc<Vec<([f32; 3], [f32; 4])>>>,
}

impl Stepped {
    fn of(m: &Movement) -> Self {
        // The body's animation gives the ribcage in the body's axes; the
        // body may face a little away from the view.
        // The body faces as it is to be drawn: in a jump the game turns it
        // after the view on a clock that makes the turn uneven.
        let in_view = m.drawn_body_in_view();
        // Drawn where the frames' own time puts it: the game's clock for
        // the player makes a jump's travel uneven from frame to frame.
        let at = m.drawn_position();
        Self {
            // The simulation's forward is +z; Bevy's is -z.
            position: Vec3::new(at[0], at[1], -at[2]),
            ribcage: quat_rotate(quat_axis(1, m.drawn_body_yaw()), m.ribcage()),
            posed_for: m.posed_for(),
            arms: [m.body.arm_pose(true, in_view), m.body.arm_pose(false, in_view)],
            body: BodyPose::of(m),
            rig: None,
        }
    }
}

/// What the weapon's springs leave of a frame that had a physics step: the
/// weapon's moved bone, the camera, and the elbows' shifts.
#[derive(Clone, Copy)]
struct Sprung {
    weapon: ([f32; 3], [f32; 4]),
    camera: CameraPose,
    elbows: [[f32; 3]; 2],
}

impl Sprung {
    fn blend(self, to: Self, t: f32) -> Self {
        let lerp = |a: [f32; 3], b: [f32; 3]| -> [f32; 3] { std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t) };
        Self {
            weapon: (lerp(self.weapon.0, to.weapon.0), quat_lerp(self.weapon.1, to.weapon.1, t)),
            camera: CameraPose {
                position: lerp(self.camera.position, to.camera.position),
                rotation: quat_lerp(self.camera.rotation, to.camera.rotation, t),
                field_of_view: self.camera.field_of_view + (to.camera.field_of_view - self.camera.field_of_view) * t,
            },
            elbows: [lerp(self.elbows[0], to.elbows[0]), lerp(self.elbows[1], to.elbows[1])],
        }
    }
}

/// Input that arrives between frames of the simulation.
#[derive(Default)]
struct Pending {
    look: Vec2,
    /// What the mouse gave the head's free look, in degrees across and
    /// down, and whether the look's key came up.
    head: Vec2,
    head_reset: bool,
    jump: bool,
    crouch: bool,
    prone: bool,
    stance_delta: f32,
    speed_delta: f32,
    reload: bool,
    /// The reload key's double click (`QuickReloadWeapon`).
    quick_reload: bool,
    fire_mode: bool,
    check_magazine: bool,
    check_chamber: bool,
    breath: bool,
    fold_stock: bool,
    /// The weapon's three further keys: its fullest way of firing, the
    /// look at its selector, and the look all over.
    force_auto: bool,
    check_fire_mode: bool,
    examine: bool,
    /// The walk key given, and taken back where it is bound to be held.
    walk_toggle: bool,
    walk_end: bool,
    /// The game's commands for the bindings that hold, as the keys gave
    /// them since the simulation's last frame (`trp_sim::Input`): the
    /// breath and the crouch key coming up, the sprint, the lean and the
    /// step aside given and taken back (left then right).
    breath_end: bool,
    restore_pose: bool,
    sprint_toggle: bool,
    sprint_end: bool,
    lean_toggle: [bool; 2],
    lean_end: [bool; 2],
    step_toggle: [bool; 2],
    step_end: [bool; 2],
    /// The aim's binding, in the order it came: given (true), which puts
    /// the sights the other way, or taken back, which puts them down.
    aim: Vec<bool>,
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

#[derive(Resource)]
pub struct Controller {
    pub movement: Movement,
    /// What the last drawn frame ran of the simulation.
    pub paced: Paced,
    pending: Pending,
    pacer: Pacer,
    /// What the springs left of the last two frames of the simulation
    /// that had a physics step.
    before: Sprung,
    now: Sprung,
    /// What the movement and the body left of its last few frames, the
    /// newest last: enough to reach back past the moment drawn.
    stepped: VecDeque<Stepped>,
    /// The view as shown on the last drawn frames, each with the host's
    /// time: yaw and pitch. And where the ribcage is drawn for the newest
    /// (`Shown::ribcage`).
    looks: VecDeque<(f32, [f32; 2])>,
    ribcage_shown: Vec3,
    /// Whether the aim key was down when last looked at, in a run that
    /// plays a script.
    aim_key: bool,
    /// How the trigger stands: the fire binding given turns it over and
    /// taken back lets it go (`FirearmHandsInputTranslator`).
    trigger: bool,
    /// How the vault key stands: its binding given or taken back turns
    /// it over, which is all the game makes of either (`Player.Vaulting`).
    vault_key: bool,
    /// What the keys push the two movement axes to, right and forward.
    move_keys: [f32; 2],
    /// The slow lean's axis as the keys push it, kept as the walk's are.
    slow_lean: f32,
    /// How deep the weapon would stick into something, in metres, as the
    /// last frame measured it.
    pub overlap: f32,
    /// The box that keeps the camera out of the world, from the player
    /// prefab, if it could be read.
    pub camera_box: Option<trp_data::player::CameraBoxData>,
    /// The inventory screen is open: the mouse is the screen's, so the
    /// view does not turn and the mouse's buttons do nothing.
    pub inventory_open: bool,
    /// Whether there is a weapon in the hands: the one on the sling slot
    /// of the equipment (inventory spec, "The weapon in hand"). With none
    /// the weapon's keys do nothing and its rig is not drawn.
    pub armed: bool,
    /// What the simulation takes from the rig in the hands, as its
    /// animator left it on the last frame.
    rig_pose: RigPose,
    /// What the thing in the hands said of itself since it was last asked:
    /// that it has appeared, that it has been put away.
    hands_events: Vec<FirearmEvent>,
    /// How many rounds the character has within reach for the weapon in
    /// hand, as the inventory last counted them: what a tube is filled
    /// from. And how many the weapon has taken since the inventory last
    /// took them out of the pockets and the vest.
    pub rounds_in_reach: u32,
    pub rounds_taken: u32,
    /// How many chambered rounds the weapon has thrown out, unfired, since
    /// the inventory last put them on the ground.
    pub rounds_ejected: u32,
    /// How many live rounds a single barrel's reload has taken out since
    /// the inventory last put them away or down.
    pub rounds_unloaded: u32,
    /// The magazine within reach that a reload would take for the weapon
    /// in hand, with its rounds, as the inventory last found it; the one
    /// the last reload's key chose; and what the weapon has said of its
    /// magazines since the inventory last heard: each exchange begun (the
    /// rounds left in the old one, and whether it was the quick way), and
    /// how many times an old one has left the hand (firing spec, "The
    /// quick reload").
    pub next_magazine: Option<(trp_sim::inventory::Id, u32)>,
    pub magazine_asked: Option<trp_sim::inventory::Id>,
    pub swaps: Vec<(Option<u32>, bool)>,
    pub magazines_hidden: u32,
    /// A menu is shown for what is looked at: the wheel is the menu's and
    /// does not set the walking speed (`ActionPanel.TranslateCommand`).
    /// And the notches it has turned since the menu last took them, up
    /// the list counted as positive.
    pub menu_shown: bool,
    pub menu_notches: i32,
}

impl Controller {
    pub fn new(movement: Movement, pacer: Pacer) -> Self {
        let camera = CameraPose { position: [0.0; 3], rotation: NO_TURN, field_of_view: 50.0 };
        let sprung = Sprung { weapon: ([0.0; 3], NO_TURN), camera, elbows: [[0.0; 3]; 2] };
        let stepped = VecDeque::from([Stepped::of(&movement)]);
        let mut controller = Self {
            movement,
            paced: Paced::default(),
            pending: Pending::default(),
            pacer,
            before: sprung,
            now: sprung,
            stepped,
            looks: VecDeque::new(),
            ribcage_shown: Vec3::ZERO,
            aim_key: false,
            trigger: false,
            vault_key: false,
            move_keys: [0.0; 2],
            slow_lean: 0.0,
            overlap: 0.0,
            camera_box: None,
            inventory_open: false,
            armed: false,
            rig_pose: RigPose::default(),
            hands_events: Vec::new(),
            rounds_in_reach: 0,
            rounds_taken: 0,
            rounds_ejected: 0,
            rounds_unloaded: 0,
            next_magazine: None,
            magazine_asked: None,
            swaps: Vec::new(),
            magazines_hidden: 0,
            menu_shown: false,
            menu_notches: 0,
        };
        controller.settle_shown();
        controller
    }

    /// The view as it was shown at a moment of the host's time.
    fn look_at(&self, time: f32) -> [f32; 2] {
        let Some(&(first, mut look)) = self.looks.front() else { return self.look().into() };
        let mut before = first;
        for &(at, next) in &self.looks {
            if at >= time {
                let t = if at > before { ((time - before) / (at - before)).clamp(0.0, 1.0) } else { 1.0 };
                return [look[0] + (next[0] - look[0]) * t, look[1] + (next[1] - look[1]) * t];
            }
            (before, look) = (at, next);
        }
        look
    }

    /// Works out where the ribcage is drawn for the view as shown now.
    /// Once every drawn frame, after the simulation's frames.
    ///
    /// The frames the body is drawn from are a physics step old and more,
    /// and each was posed for the view as it stood a frame before it. The
    /// view is shown as it stands now. Drawn as those frames have it, the
    /// ribcage trailed the view by some 35 ms where the game's trails it
    /// by one frame: the eye, which hangs on it, kept moving that long
    /// after the view had stopped, by 2 cm after a turn at 300 degrees a
    /// second with the view 80 degrees down, and everything below the eye
    /// slid across the picture with it. So the ribcage is drawn where
    /// those frames have it, moved by what the simulation's body makes of
    /// the difference between the view they were posed for and the view
    /// shown one of the simulation's frames ago: a frame behind the view,
    /// as in the game, and at rest a frame after the view is.
    fn settle_shown(&mut self) {
        let now = self.pacer.host_time();
        let look: [f32; 2] = self.look().into();
        self.looks.push_back((now, look));
        while self.looks.len() > 2 && self.looks[1].0 < now - 0.1 {
            self.looks.pop_front();
        }
        let trailing = self.look_at(now - self.pacer.frame_length());

        let (older, newer, t) = self.around_drawn();
        let mut ribcage: [f32; 3] = std::array::from_fn(|k| older.ribcage[k] + (newer.ribcage[k] - older.ribcage[k]) * t);
        let posed = [
            older.posed_for[0] + delta_angle(older.posed_for[0], newer.posed_for[0]) * t,
            older.posed_for[1] + (newer.posed_for[1] - older.posed_for[1]) * t,
        ];
        // A view that has stood still that long changes nothing, and the
        // pose need not be worked out again.
        const SAME: f32 = 1e-3;
        if delta_angle(posed[0], trailing[0]).abs() > SAME || (trailing[1] - posed[1]).abs() > SAME {
            let m = &self.movement;
            let (from, to) = (m.ribcage_posed_for(posed), m.ribcage_posed_for(trailing));
            let change = quat_rotate(quat_axis(1, m.drawn_body_yaw()), std::array::from_fn(|k| to[k] - from[k]));
            ribcage = std::array::from_fn(|k| ribcage[k] + change[k]);
        }
        // Into the frame turned with the view as shown; the simulation's
        // forward is +z, Bevy's -z.
        let seen = quat_rotate(quat_axis(1, -look[0]), ribcage);
        self.ribcage_shown = Vec3::new(seen[0], seen[1], -seen[2]);
    }

    /// The two frames of the simulation either side of the moment drawn,
    /// the older first, and how far from the one to the other it lies.
    fn around_drawn(&self) -> (&Stepped, &Stepped, f32) {
        let behind = self.pacer.frames_behind();
        let last = self.stepped.len() - 1;
        let newer = last.saturating_sub(behind.floor() as usize);
        let older = last.saturating_sub(behind.ceil() as usize);
        (&self.stepped[older], &self.stepped[newer], 1.0 - behind.fract())
    }

    /// What to draw now.
    pub fn shown(&self) -> Shown {
        let sprung = self.before.blend(self.now, self.pacer.blend());
        let (older, newer, t) = self.around_drawn();
        Shown {
            position: older.position.lerp(newer.position, t),
            ribcage: self.ribcage_shown,
            weapon: sprung.weapon,
            camera: sprung.camera,
            arms: [blend_arm(older.arms[0], newer.arms[0], t), blend_arm(older.arms[1], newer.arms[1], t)],
            elbows: sprung.elbows,
        }
    }

    /// The body's skeleton to draw now, if the body is drawn.
    pub fn body_shown(&self) -> Option<BodyPose> {
        let (older, newer, t) = self.around_drawn();
        let newer = newer.body.as_ref()?;
        Some(older.body.as_ref().unwrap_or(newer).blend(newer, t))
    }

    /// The rig in the hands as its animator has it at the moment drawn:
    /// every bone in its parent, between the two frames of the simulation
    /// either side of that moment. The animator's results are untouched;
    /// only what is drawn of them lies between frames.
    pub fn rig_shown(&self) -> Option<Vec<([f32; 3], [f32; 4])>> {
        let (older, newer, t) = self.around_drawn();
        let newer = newer.rig.as_ref()?;
        // Two frames of one rig are blended; across a change of what is in
        // the hands the newer stands alone.
        let Some(older) = older.rig.as_ref().filter(|older| older.len() == newer.len()) else { return Some(newer.to_vec()) };
        let lerp = |a: [f32; 3], b: [f32; 3]| -> [f32; 3] { std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t) };
        Some(older.iter().zip(newer.iter()).map(|(a, b)| (lerp(a.0, b.0), quat_lerp(a.1, b.1, t))).collect())
    }

    /// What is in the hands has been changed for something else: its rig
    /// is drawn from this pose on, and the simulation's next frame reads
    /// this of it.
    pub fn rig_changed(&mut self, pose: Vec<([f32; 3], [f32; 4])>, read: RigPose) {
        let pose = std::sync::Arc::new(pose);
        for stepped in &mut self.stepped {
            stepped.rig = Some(pose.clone());
        }
        self.rig_pose = read;
    }

    /// What the thing in the hands has said of itself since last asked.
    pub fn take_hands_events(&mut self) -> Vec<FirearmEvent> {
        std::mem::take(&mut self.hands_events)
    }

    /// The moment drawn now, in the simulation's own seconds.
    pub fn drawn_time(&self) -> f32 {
        self.pacer.drawn_time()
    }

    /// Where the player looks now: the simulation's view and whatever the
    /// mouse has added since its last frame, as the next frame will let
    /// it through (a sprint, a jump and lying down slow the view's turning).
    pub fn look(&self) -> (f32, f32) {
        let [yaw, pitch] = self.movement.look_ahead([self.pending.look.x, self.pending.look.y]);
        (yaw, pitch)
    }
}

/// Collects mouse look and what the keys came to every frame: the game's
/// bindings as its input manager gives them out (`crate::keys::hear`),
/// each by the game's name for it.
pub fn gather_input(
    view: Res<View>,
    options: Res<Options>,
    hands: Option<Res<HandsRig>>,
    frame_time: Res<Time<Virtual>>,
    bindings: Res<crate::keys::Bindings>,
    mut controller: ResMut<Controller>,
) {
    // An unattended capture must not pick up the mouse happening to move
    // over its window. It can turn the view at a steady rate instead
    // (`--turn`), from a second in: what a mouse moved evenly would add in
    // the time each drawn frame took.
    // A run that plays a script (`--scenario`) takes its turning from that,
    // in `step`, and leaves the mouse alone likewise.
    if options.screenshot.is_some() || options.scenario.is_some() {
        if options.turn != 0.0 && frame_time.elapsed_secs() > 1.0 {
            controller.pending.look.x += options.turn * frame_time.delta_secs();
        }
        // A capture given presses of its own (`--tap`) and no keys to
        // hold hears them as a played run hears a hand's, through the
        // bindings: how the bindings themselves are tried unattended.
        if (options.taps.is_empty() && options.mouse.is_empty()) || options.hold.is_some() || options.scenario.is_some() {
            return;
        }
    }
    // How the trigger, the vault key and the movement keys stand is kept
    // whatever has the keys, as a key's own state is: let go under an
    // open screen, they are let go.
    if bindings.fired("Shoot") {
        controller.trigger = !controller.trigger;
    }
    if bindings.ended("Shoot") {
        controller.trigger = false;
    }
    if bindings.fired("Vaulting") || bindings.ended("Vaulting") {
        controller.vault_key = !controller.vault_key;
    }
    controller.move_keys = [bindings.axis("MoveX"), bindings.axis("MoveY")];
    controller.slow_lean = bindings.axis("LeanX");
    // With the inventory open the mouse and every key are the screen's
    // (`R` turns what the mouse holds): nothing is gathered, and a key
    // pressed meanwhile is lost, as in the game (inventory spec, "What an
    // open screen does to the keys").
    if !view.played() || controller.inventory_open {
        return;
    }
    let (delta_level, delta_speed) = {
        let m = &controller.movement;
        (m.settings.delta_level, m.settings.delta_speed)
    };
    // While aiming, the weapon's factor and the aiming setting replace the
    // ordinary sensitivity.
    let sensitivity = match hands.as_deref() {
        Some(rig) if rig.aiming => rig.aim_sensitivity * options.aiming_sensitivity,
        _ => options.sensitivity,
    };
    let menu_shown = controller.menu_shown;
    let mut menu_notches = 0;
    let pending = &mut controller.pending;
    // The mouse's travel as the manager gave it out: to the turn, or with
    // the free look's key held to the head (`Preset::mouse`). A count is
    // so many degrees before the sensitivity (settings spec, "The
    // mouse"), which the head's look does not have, and either axis may
    // be turned about for both (the game's two switches).
    let mouse = bindings.mouse();
    let way = Vec2::new(if options.invert[0] { -1.0 } else { 1.0 }, if options.invert[1] { -1.0 } else { 1.0 });
    pending.look += Vec2::from(mouse.turn) * way * options.degrees_a_count * sensitivity;
    pending.head += Vec2::from(mouse.look) * way * options.look_degrees_a_count;
    // The look's key is bound to its release: coming up, it asks for the
    // head's way back.
    pending.head_reset |= bindings.fired(trp_sim::bindings::LOOK_BINDING);
    // Each binding by the game's name for it, pressed as the install has
    // it: the jump when its key is let go, where the same key held vaults
    // (vaulting spec, "Two moves, one key"); the reload when its key is
    // let go, and late by the double click's time where a double click
    // is bound to the same key (settings spec).
    pending.fold_stock |= bindings.fired("FoldStock");
    pending.jump |= bindings.fired("Jump");
    pending.reload |= bindings.fired("ReloadWeapon");
    pending.quick_reload |= bindings.fired("QuickReloadWeapon");
    pending.fire_mode |= bindings.fired("ShootingMode");
    pending.check_magazine |= bindings.fired("CheckAmmo");
    pending.check_chamber |= bindings.fired("CheckChamber");
    pending.force_auto |= bindings.fired("ForceAutoWeaponMode");
    pending.check_fire_mode |= bindings.fired("CheckFireMode");
    pending.examine |= bindings.fired("ExamineWeapon");
    pending.walk_toggle |= bindings.fired("Walk");
    pending.walk_end |= bindings.ended("Walk");
    pending.breath |= bindings.fired("Breath");
    pending.breath_end |= bindings.ended("Breath");
    pending.crouch |= bindings.fired("Duck");
    pending.restore_pose |= bindings.ended("Duck");
    pending.prone |= bindings.fired("Prone");
    pending.sprint_toggle |= bindings.fired("Sprint");
    pending.sprint_end |= bindings.ended("Sprint");
    for (side, (lean, step)) in [("LeanLockLeft", "StepLeft"), ("LeanLockRight", "StepRight")].into_iter().enumerate() {
        pending.lean_toggle[side] |= bindings.fired(lean);
        pending.lean_end[side] |= bindings.ended(lean);
        pending.step_toggle[side] |= bindings.fired(step);
        pending.step_end[side] |= bindings.ended(step);
    }
    if bindings.fired("Aim") {
        pending.aim.push(true);
    }
    if bindings.ended("Aim") {
        pending.aim.push(false);
    }
    // The wheel's two bindings set the speed, and with the stance's key
    // held the stance's two set that. While a menu is shown for what is
    // looked at, the speed's two are the menu's instead: up the list for
    // a notch up.
    let notches = |up: &str, down: &str| bindings.fired(up) as i32 - bindings.fired(down) as i32;
    pending.stance_delta += notches("NextWalkPose", "PreviousWalkPose") as f32 * delta_level;
    if menu_shown {
        menu_notches = notches("Next", "Previous");
    } else {
        pending.speed_delta += notches("Next", "Previous") as f32 * delta_speed;
    }
    controller.menu_notches += menu_notches;
}

/// Runs the frames of the simulation that have come due.
///
/// Each is one frame of the game: the player is moved and the weapon's
/// logic run once, by the frame's own length, and the weapon's springs
/// stepped once for each physics step that falls in it, in the game's
/// order: movement, the springs, the weapon's logic (whose shots the
/// springs feel on their next step), then the camera.
pub fn step(
    view: Res<View>,
    options: Res<Options>,
    mut controller: ResMut<Controller>,
    hands: Option<ResMut<HandsRig>>,
    frame_time: Res<Time<Virtual>>,
    mut range: ResMut<Range>,
    features: Res<crate::options_view::Features>,
) {
    let controller = &mut *controller;
    let inventory_open = controller.inventory_open;
    // Whose eyes the character is seen with: its own while the picture is
    // its own, and anyone else's from a camera outside it, as the game has
    // every character but the one playing (third-person spec).
    let point_of_view = options.point_of_view(*view);
    let outside = point_of_view == PointOfView::ThirdPerson;
    if controller.movement.body.point_of_view() != point_of_view {
        controller.movement.body.set_point_of_view(point_of_view);
    }
    let began = std::time::Instant::now();
    let host_before = controller.pacer.host_time();
    let due = controller.pacer.pass(frame_time.delta_secs());
    controller.paced =
        Paced { host_dt: frame_time.delta_secs(), blend: controller.pacer.blend(), ..Paced::default() };
    // One of the recorder's scripts as the input. Its turns come as a
    // mouse's would: what the script asks over each drawn frame, through
    // the sensitivity in force (the game takes the recorder's turns
    // through its mouse sensitivity too), gathered for the simulation's
    // frames to share out.
    let scenario = options.scenario.as_ref();
    if let Some(scenario) = scenario {
        let at = options.scenario_at;
        let [yaw, pitch] = scenario.turned(host_before - at, controller.pacer.host_time() - at);
        let sensitivity = match hands.as_deref() {
            Some(rig) if rig.aiming => rig.aim_sensitivity * options.aiming_sensitivity,
            _ => options.sensitivity,
        };
        controller.pending.look += Vec2::new(yaw, pitch) * sensitivity;
    }
    if due == 0 {
        controller.settle_shown();
        controller.paced.cost = began.elapsed().as_secs_f32();
        return;
    }
    let mut hands = hands;

    let scripted = options.hold.as_deref();
    let has = |name: &str| scripted.is_some_and(|script| script.split('+').any(|k| k == name));
    let axis = |pos: bool, neg: bool| pos as i32 as f32 - neg as i32 as f32;
    let (delta_level, delta_speed) = (controller.movement.settings.delta_level, controller.movement.settings.delta_speed);

    // The mouse's travel goes to the frames by their own time, as a game
    // running at their rate would have had it: each frame that is due
    // takes the share that falls in it, and what is left belongs to a
    // frame still to come and is shown ahead of it (`Controller::look`).
    // Given whole to whichever frame came due, the view would turn by
    // uneven amounts wherever the two rates do not divide, and the body
    // that follows it in a jump, with the weapon it carries, would shake.
    let mut travel = controller.pending.look;
    let mut head_travel = controller.pending.head;
    for _ in 0..due {
        let before = controller.pacer.simulated();
        let frame = controller.pacer.frame();
        let now = controller.pacer.simulated();
        let look = travel * frame.input_share;
        travel -= look;
        let head = head_travel * frame.input_share;
        head_travel -= head;
        // A scripted run's presses: held from a moment on, or made on the
        // frame that passes it.
        let since = |seconds: f32| now > seconds;
        let passing = |seconds: f32| before < seconds && now >= seconds;
        // A script's keys and presses go by the simulation's own time.
        let (script_before, script_now) = (before - options.scenario_at, now - options.scenario_at);
        let script_held = |name: &str| scenario.is_some_and(|s| s.held(name, script_now));
        let script_pressed =
            |name: &str| scenario.is_some_and(|s| s.pressed(script_before, script_now).any(|key| key == name));
        // A key of a run that plays a script, by the script's name for
        // it. A played run's keys come as the game's bindings instead.
        let played = scenario.is_none() && scripted.is_none();
        let held = |name: &str| match (scenario, scripted) {
            (Some(_), _) => script_held(name),
            (None, Some(_)) => has(name),
            (None, None) => false,
        };
        // Presses go to the first frame that comes.
        let pending = std::mem::take(&mut controller.pending);

        // The aim key asks; the weapon's logic and the movement state decide.
        // The same goes for a press of the trigger: a sprint and a crawl
        // take none.
        let (can_aim, can_fire) = (controller.movement.can_aim(), controller.movement.can_press_trigger());
        let armed = controller.armed;
        // With the inventory screen open the weapon hears none of its
        // keys either. Where each stands is still noted, so that a key
        // pressed under the screen does nothing when it shuts.
        let heard = !inventory_open;
        // With the weapon's handling switched off none of its keys but the
        // aim's is heard (settings spec, "Features").
        let handled = heard && features.weapon_handling;
        let aim_key = armed
            && match (scenario, scripted) {
            (Some(_), _) => script_held("aim"),
            // A scripted run with `aim` presses it half a second in.
            (None, Some(_)) => has("aim") && since(0.5),
            (None, None) => false,
        };
        if let Some(firearm) = hands.as_deref_mut().and_then(|h| h.firearm.as_mut()) {
            if played {
                // The aim's binding given puts the sights the other way
                // (`ToggleAim`); taken back, where it is bound to be
                // held, it puts them down.
                for given in pending.aim.iter().filter(|_| armed && heard) {
                    firearm.set_aim(*given && !firearm.aiming(), can_aim);
                }
            } else if std::mem::replace(&mut controller.aim_key, aim_key) != aim_key && heard {
                firearm.set_aim(aim_key, can_aim);
            }
            // The reload key goes to the weapon before the body moves:
            // the body's animator is told of a reload on the frame it
            // starts. A scripted run with `reload` starts one two seconds
            // in, or three with `prone`, when the character is down.
            let at = if has("prone") { 3.0 } else { 2.0 };
            let plain = pending.reload || (has("reload") && passing(at)) || script_pressed("reload");
            let quick = pending.quick_reload && !plain;
            if armed && handled && (plain || quick) {
                if firearm.settings.internal_magazine {
                    // A tube is filled from the rounds within reach, and
                    // barrels are loaded, by either key; a cylinder's
                    // quick way is not built, and its key does nothing.
                    if plain || firearm.settings.cylinder.is_none() {
                        firearm.reload_shells(controller.rounds_in_reach);
                    }
                } else if scenario.is_some() {
                    // A recorded scenario changes the magazine for a full
                    // one, as its recording does.
                    firearm.reload();
                } else if let Some((magazine, rounds)) = controller.next_magazine {
                    // The fullest magazine within reach goes in; with
                    // none, nothing is done (`ReloadMagNotFound`). The
                    // inventory makes the exchange as the reload begins.
                    controller.magazine_asked = Some(magazine);
                    if quick {
                        firearm.quick_reload_with(rounds);
                    } else {
                        firearm.reload_with(rounds);
                    }
                }
            }
        }
        let firearm = hands.as_deref().and_then(|h| h.firearm.as_ref());
        let (aiming, reloading) = (firearm.is_some_and(|f| f.aiming()), firearm.is_some_and(|f| f.reloading()));
        let empty_hands = hands.as_deref().and_then(|h| h.empty.as_ref());
        let hands_busy = firearm.is_some_and(|f| f.hands_busy()) || empty_hands.is_some_and(|empty| empty.hands_busy());
        // Whether the animator of the thing in hand is at rest, which a
        // gesture of the hands waits for, and whether it is in its states
        // for an open inventory, which the body's own pose for one waits
        // for (empty hands have none, so their body never takes it).
        let hands_idle = firearm.is_some_and(|f| f.animator_idling()) || empty_hands.is_some_and(|empty| empty.animator_idling());
        let hands_in_inventory = firearm.is_some_and(|f| f.in_inventory());
        let weapon_occupied = firearm.is_some_and(|f| f.occupied());
        let notch = |up: &str, down: &str| axis(script_pressed(up), script_pressed(down));
        // What a played run's keys push the movement axes to; none while
        // the keys fly the camera.
        let move_keys = if view.played() { controller.move_keys } else { [0.0; 2] };

        let input = SimInput {
            aiming,
            reloading,
            hands_busy,
            hands_idle,
            hands_in_inventory,
            weapon_occupied,
            free_look: false,
            // The inventory screen has the keys while it is open: the
            // character is handed no movement, no turn and no key.
            screen: inventory_open,
            inventory: inventory_open,
            // The head's free look: its share of the mouse's travel, its
            // key come up, and how far down the camera last pointed.
            head_look: [head.x, head.y],
            head_reset: pending.head_reset,
            camera_pitch: {
                let ahead = quat_rotate(quat_mul(quat_axis(0, controller.movement.shown_pitch()), controller.now.camera.rotation), [0.0, 0.0, 1.0]);
                (-ahead[1]).clamp(-1.0, 1.0).asin().to_degrees()
            },
            // The jump key held vaults or climbs what stands ahead.
            vault: if played { view.played() && controller.vault_key } else { held("vault") },
            move_dir: if played { move_keys } else { [axis(held("d"), held("a")), axis(held("w"), held("s"))] },
            look_delta: [look.x, look.y],
            // Sprint only starts from a walk, so a scripted run presses it
            // a second after the movement keys.
            sprint: held("shift") && (scenario.is_some() || since(1.0)),
            // A played run's sprint, lean, step aside, breath and crouch
            // come as the game's commands for their bindings.
            sprint_toggle: pending.sprint_toggle,
            sprint_end: pending.sprint_end,
            lean_toggle: pending.lean_toggle,
            lean_end: pending.lean_end,
            step_toggle: pending.step_toggle,
            step_end: pending.step_end,
            breath_end: pending.breath_end,
            restore_pose: pending.restore_pose,
            walk_toggle: pending.walk_toggle,
            walk_end: pending.walk_end,
            // The slow lean's axis, as its keys push it.
            slow_lean: if played && view.played() { controller.slow_lean } else { 0.0 },
            // A scripted run with `jump` jumps two seconds in.
            jump: pending.jump || (has("jump") && passing(2.0)) || script_pressed("jump"),
            crouch: pending.crouch || script_pressed("crouch"),
            // A scripted run with `prone` lies down a second in.
            prone: pending.prone || (has("prone") && passing(1.0)) || script_pressed("prone"),
            // A scripted run with `breath` holds it three seconds in.
            hold_breath: pending.breath || (has("breath") && passing(3.0)) || script_pressed("breath"),
            pose_delta: pending.stance_delta + notch("stance_up", "stance_down") * delta_level,
            speed_delta: pending.speed_delta + notch("wheel_up", "wheel_down") * delta_speed,
            // A scripted run with `stepr` or `stepl` steps a second in,
            // or three seconds in with `prone`, when the character is
            // down: lying down the same key rolls.
            lean: 0.0,
            lean_keys: [held("q"), held("e")],
            step: match (scenario, scripted) {
                (Some(_), _) => axis(script_held("step_right"), script_held("step_left")) as i32,
                (None, Some(_)) => {
                    let at = if has("prone") { 3.0 } else { 1.0 };
                    axis(has("stepr") && since(at), has("stepl") && since(at)) as i32
                }
                (None, None) => 0,
            },
        };
        // The pacer's frames are the game's drawn frames: the player's
        // own whole-millisecond clock runs on them.
        controller.movement.frame(frame.dt, &input, &*range);
        // The doors, containers and trunk move after the character, as the
        // game runs their routines after its update.
        range.step_things(frame.dt);

        let (mut weapon, mut camera) = (controller.now.weapon, controller.now.camera);
        let mut elbows = controller.now.elbows;
        let mut rig_now = None;
        if let Some(hands) = hands.as_deref_mut() {
            // The sprint switching on puts the sights down and lets go of
            // the trigger; so does entering a state that does not allow
            // aiming, for the sights.
            let (sprint_started, aim_lowered) =
                (controller.movement.take_sprint_started(), controller.movement.take_aim_lowered());
            // A state the body's animator entered may carry a pose for the
            // first-person weapon (the roll lying down does).
            hands.hands.weapon_flip_speed = controller.movement.settings.weapon_flip_speed;
            // The weapon's motion and its recoil as the options' switches
            // have them (the rig in hand may be new).
            hands.hands.set_systems(&controller.movement.systems);
            // The weapon's motion is told whose eyes see it; the rig in
            // the hands may have been changed for another since.
            if hands.hands.point_of_view() != point_of_view {
                hands.hands.set_point_of_view(point_of_view);
            }
            for pose in controller.movement.take_poses() {
                hands.hands.assign_pose(pose.as_ref());
            }
            if let Some(firearm) = &mut hands.firearm {
                if sprint_started {
                    firearm.sprint_started();
                }
                if aim_lowered {
                    firearm.lower_aim();
                }
                // More than half a second off the ground puts the sights
                // down, on every frame of it.
                if controller.movement.long_fall() {
                    firearm.set_aim(false, controller.movement.can_aim());
                }
                // So does a vault or a climb of something high enough to
                // take a hand off the weapon.
                if controller.movement.take_aim_taken() {
                    firearm.set_aim(false, controller.movement.can_aim());
                }
            }
            // The camera takes the head's turn, and the aimed field of
            // view gives way while the mouse has the head.
            let head = &controller.movement.head_look;
            hands.hands.set_head_rotation(head.rotation(), head.control(), aiming);
            let aiming = hands.firearm.as_ref().is_some_and(|f| f.aiming());
            hands.aiming = aiming;
            // The bone the weapon's moved bone hangs from, as its animator
            // left it on the frame before: the springs run ahead of the
            // frame's own update of the animator.
            hands.hands.weapon_root = controller.rig_pose.parent;

            // The springs, once for each physics step in the frame.
            for _ in 0..frame.physics_steps {
                let footstep = controller.movement.take_footstep();
                let m = &controller.movement;
                hands.hands.advance(
                    controller.pacer.physics_step(),
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
                        frame_dt: frame.dt,
                        wind: m.wind(),
                    },
                );
            }
            // The rig is turned to the view after the steps, as the game
            // does it: the sway sees this frame's turn a frame on.
            hands.hands.turn_rig(controller.movement.yaw, controller.movement.shown_pitch());

            // The weapon's own logic.
            if let Some(firearm) = &mut hands.firearm {
                let trigger = match (scenario, scripted) {
                    // A script names the fire mode it wants.
                    (Some(_), _) => {
                        let wanted = [("auto", FireMode::FullAuto), ("single", FireMode::Single), ("semiauto", FireMode::SemiAuto)];
                        for (_, mode) in wanted.into_iter().filter(|(name, _)| script_pressed(name)) {
                            firearm.set_fire_mode(mode);
                        }
                        if script_pressed("check_chamber") && armed && handled {
                            firearm.check_chamber();
                        }
                        script_held("fire")
                    }
                    // A scripted run with `auto` first switches the fire
                    // mode, then presses the trigger two seconds in.
                    (None, Some(_)) => {
                        if has("auto") && before == 0.0 {
                            firearm.next_fire_mode();
                        }
                        has("fire") && (!has("auto") || since(2.0))
                    }
                    (None, None) => view.played() && controller.trigger,
                };
                if handled {
                    firearm.set_trigger(trigger && armed, can_fire);
                } else {
                    firearm.note_trigger(trigger && armed);
                }
                if armed && handled && pending.fire_mode {
                    firearm.next_fire_mode();
                }
                if armed && handled && (pending.check_magazine || script_pressed("check_magazine")) {
                    firearm.check_magazine();
                }
                if armed && handled && pending.check_chamber {
                    firearm.check_chamber();
                }
                // The fullest way of firing, the look at the selector and
                // the look all over, each the idling operation's to take.
                if armed && handled && pending.force_auto {
                    firearm.force_auto_fire_mode();
                }
                if armed && handled && pending.check_fire_mode {
                    firearm.check_fire_mode();
                }
                if armed && handled && pending.examine {
                    firearm.examine();
                }
                // A scripted run with `fold` folds the stock a second and a
                // half in.
                if armed && handled && (pending.fold_stock || (has("fold") && passing(1.5))) {
                    firearm.fold_stock();
                }
                firearm.set_aim_angle(controller.movement.pitch);
                // How its animations are drawn: the options' switch.
                firearm.set_drawn_smooth(features.smooth_animations);
                for event in firearm.step(frame.dt) {
                    match event {
                        FirearmEvent::Shot { strength } => hands.hands.shoot(strength),
                        FirearmEvent::AutoFire(on) => hands.hands.set_auto_fire(on),
                        FirearmEvent::Appeared | FirearmEvent::Hidden => controller.hands_events.push(event),
                        // The body's own part in a weapon drawn or put
                        // away: its animator is told whoever is looking.
                        FirearmEvent::BodyAction(action) => controller.movement.body.set_weapon_action(action),
                        // The rounds a tube took: the inventory takes
                        // them out of the pockets and the vest.
                        FirearmEvent::RoundsTaken(count) => controller.rounds_taken += count,
                        // A chambered round thrown out by an opened bolt:
                        // the inventory puts it on the ground.
                        FirearmEvent::RoundEjected => controller.rounds_ejected += 1,
                        FirearmEvent::RoundUnloaded => controller.rounds_unloaded += 1,
                        // A magazine's exchange begun, and the old one out
                        // of the hand: the inventory makes the one and
                        // lets the magazine fall at the other.
                        FirearmEvent::MagazineSwap { left, quick } => controller.swaps.push((left, quick)),
                        FirearmEvent::MagazineHidden => controller.magazines_hidden += 1,
                        // The stock folded or unfolded: the recoil and
                        // the hold go by the build as it now stands, and
                        // the body's animator by its new width.
                        FirearmEvent::Folded(folded) => {
                            hands.hands.set_folded(folded, aiming);
                            let width = if folded { hands.folded_width.unwrap_or(hands.width) } else { hands.width };
                            controller.movement.body.set_weapon_width(width);
                        }
                        // Sound, shells and the magazine's model are not in yet.
                        _ => {}
                    }
                }
            }
            // Empty hands have a controller and an animator of their own.
            if let Some(empty) = &mut hands.empty {
                empty.set_drawn_smooth(features.smooth_animations);
                let said = empty.step(frame.dt, controller.movement.pitch);
                for event in &said {
                    if let FirearmEvent::BodyAction(action) = event {
                        controller.movement.body.set_weapon_action(*action);
                    }
                }
                controller.hands_events.extend(said.into_iter().filter(|event| matches!(event, FirearmEvent::Appeared | FirearmEvent::Hidden)));
            }
            // The rig as its animator has it now, which is what the rest
            // of the frame goes by, as the game's late update does.
            // What is drawn of it is the animator's pose for drawing,
            // which is that pose taken on curves without corners while
            // the options' switch says so (`Animator::set_drawn_smooth`).
            // The simulation reads the pose itself.
            let posed = hands.animator().map(|animator| (animator.pose().to_vec(), animator.drawn_pose()));
            if let Some((posed, drawn)) = posed {
                controller.rig_pose = hands.frame.read(&posed);
                rig_now = Some(std::sync::Arc::new(drawn));
            }
            let pose = controller.rig_pose;
            hands.hands.weapon_root = pose.parent;
            // Seen from outside, the body reaches for the vest as the left
            // hand's marker nears the weapon's marker for it, while a
            // reload is in hand or the inventory is open, and not in a
            // sprint (`Player.AdjustUtilityLayerWeight`).
            if outside {
                let reaching = (hands.firearm.as_ref().is_some_and(|f| f.reloading()) || inventory_open) && !controller.movement.sprinting;
                controller.movement.body.weigh_reach_for_vest(pose.vest_reach.is_some(), pose.vest_reach.filter(|_| reaching), frame.dt);
            }

            // Where that leaves the weapon's moved bone, and the camera:
            // the body's weapon bone takes the weapon over in a sprint and
            // its right hand in a crawl, and its camera bone lends its turn.
            let m = &controller.movement;
            // The iron sights' line and the muzzle as the rig has them now.
            hands.hands.note_sights(pose.sight_line, pose.muzzle);
            hands.hands.blend_poses(frame.dt);
            // What is drawn of the body's hold takes the body as drawn.
            let in_view = m.drawn_body_in_view();
            weapon = hands.hands.weapon_transform_carried(
                pose.parent,
                m.body.carried(in_view),
                m.body.held(in_view),
                pose.marker,
            );
            // The camera takes the sight's place off the weapon where the
            // procedural motion put it, before the body takes the weapon.
            let placed = hands.hands.weapon_transform();
            let aim = pose.aim_bone.map(|bone| {
                let hanging = quat_rotate(pose.parent.1, placed.0);
                let origin: [f32; 3] = std::array::from_fn(|i| pose.parent.0[i] + hanging[i]);
                let turn = quat_mul(pose.parent.1, placed.1);
                let reach = quat_rotate(turn, bone);
                AimPoint {
                    bone: std::array::from_fn(|i| origin[i] + reach[i]),
                    origin,
                    // The barrel runs along the moved bone's -y.
                    barrel: quat_rotate(turn, [0.0, -1.0, 0.0]),
                }
            });
            hands.hands.body_camera = Some(m.body.camera_bone());
            // The camera is moved for the character's own eyes only: seen
            // from outside it is left where it last stood
            // (`ThirdPersonStrategy.ApplyCameraTransformations`).
            if !outside {
                camera = hands.hands.frame(frame.dt, pose.animated, aim);
                // The camera's box is pushed out of whatever it has got into,
                // and the camera with it: the next frame's easing starts there.
                if let Some(shape) = &controller.camera_box {
                    let push = m.camera_push(&*range, camera.position, camera.rotation, shape.size, shape.centre);
                    camera.position = hands.hands.push_camera(push);
                }
            } else {
                hands.hands.frame_outside();
            }
            elbows = [hands.hands.elbow_shift(true), hands.hands.elbow_shift(false)];

            // Last in the frame, as in the game: how deep the weapon would
            // stick into something. The next physics steps turn it away by
            // that, and past a limit the sights come down until it is clear.
            let depth = m.weapon_overlap(&*range, hands.hands.weapon_line());
            hands.hands.set_overlap(depth);
            let run_down = hands.hands.turn_away_value();
            if let Some(firearm) = &mut hands.firearm {
                firearm.overlap(depth, run_down, m.can_aim());
                // Sights the obstacle put down or gave back are so from
                // here on, not from the next physics step.
                let aiming = firearm.aiming();
                hands.hands.aim_changed_after_frame(aiming, frame.dt);
            }
            controller.overlap = depth;
        }

        // What the springs moved is drawn from the frames with a physics
        // step; what the movement and the body moved, from every frame.
        if frame.physics_steps > 0 {
            controller.before = controller.now;
            controller.now = Sprung { weapon, camera, elbows };
        }
        controller.stepped.push_back(Stepped { rig: rig_now, ..Stepped::of(&controller.movement) });
        let kept = (controller.pacer.physics_step() / controller.pacer.frame_length()).ceil() as usize + 2;
        while controller.stepped.len() > kept {
            controller.stepped.pop_front();
        }
        controller.paced.frames += 1;
        controller.paced.physics_steps += frame.physics_steps;
    }
    // The travel no frame has had yet.
    controller.pending.look = travel;
    controller.pending.head = head_travel;
    controller.settle_shown();
    controller.paced.cost = began.elapsed().as_secs_f32();
    controller.paced.blend = controller.pacer.blend();
}

/// Places the player and the frame the view hangs in.
pub fn apply(
    controller: Res<Controller>,
    rig: Option<Res<WeaponRig>>,
    mut player: Query<&mut Transform, (With<Player>, Without<ViewPivot>)>,
    mut pivot: Query<&mut Transform, (With<ViewPivot>, Without<Player>)>,
) {
    if rig.is_none() {
        return;
    }
    let shown = controller.shown();
    let (yaw, pitch) = controller.look();
    if let Ok(mut transform) = player.single_mut() {
        transform.translation = shown.position;
        // The simulation's yaw turns right when positive; Bevy's turns left.
        transform.rotation = Quat::from_rotation_y(-yaw.to_radians());
    }
    if let Ok(mut transform) = pivot.single_mut() {
        transform.translation = shown.ribcage;
        transform.rotation = Quat::from_rotation_x(-pitch.to_radians());
    }
}
