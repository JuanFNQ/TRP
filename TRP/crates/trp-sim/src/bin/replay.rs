//! Replays the recording plugin's scenarios through the simulation and,
//! given traces from the running game, reports how far apart the two are.
//!
//!   replay [--scenarios DIR] [--traces DIR] [--out DIR] [--weapon ID] [--weight KG] [name ...]
//!
//! The scenario files are the ones the plugin plays (`hosts/spt-oracle/scenarios`).
//! A game trace is looked for as `<traces>/<scenario>.jsonl`. Where one
//! exists the replay follows its frames: the same frame times on both of
//! the game's clocks, the same look direction and bearing, and the weapon,
//! round and settings it recorded.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use trp_data::engine::EngineSettings;
use trp_data::hard_settings::HardSettings;
use trp_data::player::{HandsData, LimbData};
use trp_data::unity::anim::path_hash;

use trp_data::unity::Assets;
use trp_data::{Database, Install};
use trp_sim::arms;
use trp_sim::body::BodyData;
use trp_sim::collision::Course;
use trp_sim::math::{delta_angle, quat_axis, quat_euler, quat_mul, quat_rotate};
use trp_sim::movement::{Capsule, GROUND_CHECK, GROUND_CHECK_JUMP, GROUND_CHECK_MOVING, GROUND_CHECK_SPRINT};
use trp_sim::rig::{ArmNodes, WeaponRig};
use trp_sim::script::Scenario;
use trp_sim::vaulting::Obstacle;
use trp_sim::{
    AimPoint, Body, Firearm, FirearmEvent, FireMode, Hands, HandsInput, Input, Movement, MovementSettings, PointOfView, State,
    ViewSettings, World,
};
use serde_json::Value as Json;

const AKM: &str = "59d6088586f774275f37482f";
const DT: f32 = 1.0 / 60.0;
/// The state the weapon's animator rests in.
const WEAPON_AT_REST: &str = "IDLE";

/// One line of a trace: named values, each one or more numbers.
type Sample = BTreeMap<String, Vec<f32>>;

/// How far the weapon and the arms are drawn in along the view: the wide
/// field of view's squeeze for the character's own eyes, none for
/// another's (third-person spec, "What the switch does").
fn squeeze(hands: &Hands) -> f32 {
    if hands.point_of_view() == PointOfView::ThirdPerson {
        1.0
    } else {
        hands.fov_scale()
    }
}

/// A rotation as its axis times its angle in degrees, which compares
/// sensibly component by component.
fn rotation_vector(q: &[f32]) -> Vec<f32> {
    let sign = if q[3] < 0.0 { -1.0 } else { 1.0 };
    let (x, y, z, w) = (q[0] * sign, q[1] * sign, q[2] * sign, (q[3] * sign).min(1.0));
    let sine = (x * x + y * y + z * z).sqrt();
    if sine < 1e-7 {
        return vec![0.0; 3];
    }
    let angle = 2.0 * sine.atan2(w).to_degrees();
    vec![x / sine * angle, y / sine * angle, z / sine * angle]
}

/// The movement state as one number, the same for a recorded state name
/// and a simulated state, so that the two can be set side by side.
fn state_number(state: State) -> f32 {
    match state {
        State::Idle => 0.0,
        State::Move => 1.0,
        State::Sprint => 2.0,
        State::Jump => 3.0,
        State::SprintStop => 4.0,
        State::Landing => 5.0,
        State::ToProne => 6.0,
        State::ProneIdle => 7.0,
        State::ProneMove => 8.0,
        State::FromProne => 9.0,
        State::Sidestep => 10.0,
        State::Roll => 11.0,
        State::Vault => 12.0,
        State::Climb => 13.0,
        State::VaultFall => 14.0,
        State::VaultLanding => 15.0,
        State::Approach => 16.0,
        State::Breach => 17.0,
        State::BlindFire => 18.0,
    }
}

/// The number given to a recorded movement state the simulation has no
/// state for, so that every frame of it counts as a difference.
const UNKNOWN_STATE: f32 = 99.0;
/// What the weapon's logic has in hand, as `in_hand` in both traces.
const RELOAD_IN_HAND: f32 = 1.0;
const LOOK_IN_HAND: f32 = 2.0;

/// An animator state as the recorder writes it: the low 24 bits of the
/// hash of the state's own name, which a single float holds exactly.
fn animator_state(name: &str) -> f32 {
    let own = name.rsplit('.').next().unwrap_or(name);
    (path_hash(own) & 0x00ff_ffff) as f32
}

/// The same number for the game's own name of a movement state.
fn recorded_state_number(name: &str) -> Option<f32> {
    Some(match name {
        "Idle" => 0.0,
        "Run" => 1.0,
        "Sprint" => 2.0,
        "Jump" => 3.0,
        "Transition" => 4.0,
        "JumpLanding" => 5.0,
        "Transit2Prone" => 6.0,
        "ProneIdle" => 7.0,
        "ProneMove" => 8.0,
        "Prone2Stand" => 9.0,
        "Sidestep" => 10.0,
        "Roll" => 11.0,
        "ClimbOver" => 12.0,
        "ClimbUp" => 13.0,
        "VaultingFallDown" => 14.0,
        "VaultingLanding" => 15.0,
        "Approach" => 16.0,
        "BreachDoor" => 17.0,
        "BlindFire" => 18.0,
        _ => return None,
    })
}

/// Where the weapon's parts rest, for working out where the sight is.
struct Geometry {
    weapon_root: ([f32; 3], [f32; 4]),
    /// The weapon's own bone, at rest in the bone the procedural motion
    /// moves, and the aim bone on it. The weapon's clips move the first.
    weapon_bone: ([f32; 3], [f32; 4]),
    aim_bone: [f32; 3],
    /// The way the eye looks along the aim bone, on the weapon's own bone.
    aim_line: [f32; 3],
    camera_bone: [f32; 4],
    /// The marker for the right hand, in that bone's axes.
    right_hand: ([f32; 3], [f32; 4]),
    /// The box that keeps the camera out of the world.
    camera_box: trp_data::player::CameraBoxData,
}

impl Geometry {
    fn of(
        built: &trp_data::weapon::Weapon,
        right_hand: ([f32; 3], [f32; 4]),
        camera_box: trp_data::player::CameraBoxData,
    ) -> Result<Self> {
        let hierarchy = &built.hierarchy;
        let node = |path: &str| {
            (0..hierarchy.nodes.len())
                .find(|&i| hierarchy.path(i) == path)
                .map(|i| &hierarchy.nodes[i])
                .with_context(|| format!("the weapon has no {path}"))
        };
        let root = node("Weapon_root")?;
        let weapon = node("Weapon_root/Weapon_root_anim/weapon")?;
        // The aim bone of the sight the game aims by first (optics spec,
        // "The sights a weapon has"; weapon-handling spec, "The sights").
        let sights = trp_sim::rig::SightNodes::of_weapon(built).context("the weapon has no aim bone")?;
        let aim = sights.bone_on_weapon(&WeaponRig::new(hierarchy));
        Ok(Self {
            weapon_root: (root.translation, root.rotation),
            weapon_bone: (weapon.translation, weapon.rotation),
            aim_bone: aim,
            aim_line: sights.line_on_weapon(&WeaponRig::new(hierarchy)),
            camera_bone: node("Camera_animated").map_or(quat_euler([-90.0, 0.0, 0.0]), |n| n.rotation),
            right_hand,
            camera_box,
        })
    }

    /// The sight for a weapon bone moved by `position` and `rotation`.
    /// `animated` is where the weapon's clip has its own bone on this
    /// frame, if a recording says; the replay plays no clips of the
    /// weapon's and takes the bone at rest otherwise.
    fn aim_point(&self, position: [f32; 3], rotation: [f32; 4], animated: Option<([f32; 3], [f32; 4])>) -> AimPoint {
        let (root_position, root_rotation) = self.weapon_root;
        let moved = quat_rotate(root_rotation, position);
        let origin: [f32; 3] = std::array::from_fn(|i| root_position[i] + moved[i]);
        let turn = quat_mul(root_rotation, rotation);
        let (weapon_position, weapon_rotation) = animated.unwrap_or(self.weapon_bone);
        let on_weapon = quat_rotate(weapon_rotation, self.aim_bone);
        let bone = quat_rotate(turn, std::array::from_fn(|i| weapon_position[i] + on_weapon[i]));
        AimPoint {
            bone: std::array::from_fn(|i| origin[i] + bone[i]),
            origin,
            barrel: quat_rotate(turn, [0.0, -1.0, 0.0]),
            line: quat_rotate(turn, quat_rotate(weapon_rotation, self.aim_line)),
        }
    }
}

struct Rig {
    movement: Movement,
    hands: Hands,
    firearm: Firearm,
    /// The rounds of the magazine a reload takes, as the recorded
    /// equipment has it, for a reload whose frames give none.
    next_rounds: Option<u32>,
    /// The recorded equipment was read and has no magazine within reach
    /// for the weapon: the reload key does nothing (`ReloadMagNotFound`),
    /// unless the frames show one fitted (course 8's `03`: the MDR's key at
    /// 17.0 s found none, and blind fire was then taken at 17.5 s).
    no_magazine: bool,
}

/// The weapon's own rig as its animator poses it, for the arms' hold on
/// it: the replay draws nothing, but the arms are solved onto bones the
/// weapon's animator moves.
struct Holding {
    rig: WeaponRig,
    /// The bone the procedural motion moves, and the weapon's own below it.
    moved: usize,
    weapon: usize,
    /// The weapon's points its length is measured to.
    ends: Vec<usize>,
    /// The bones the eye is lined up with while the sights are up.
    sights: Option<trp_sim::rig::SightNodes>,
    /// Right, then left: the rig's bones the arm reads, and its solver's
    /// settings from the player prefab.
    arms: [ArmNodes; 2],
    limbs: [LimbData; 2],
    /// The left hand's marker and the weapon's marker for the vest, which
    /// a character seen from outside reaches toward (third-person spec).
    vest: Option<[usize; 2]>,
}

impl Holding {
    fn new(weapon: &trp_data::weapon::Weapon, limbs: [LimbData; 2]) -> Result<Self> {
        let rig = WeaponRig::new(&weapon.hierarchy);
        let arm = |right: bool| ArmNodes::find(&weapon.hierarchy, right).context("the weapon's rig lacks an arm's bones");
        Ok(Self {
            moved: rig.moved_bone().context("the weapon has no bone for the procedural motion")?,
            weapon: rig.bone("Weapon_root/Weapon_root_anim/weapon").context("the weapon has no bone of its own")?,
            ends: trp_sim::weapon_line_ends(weapon)?,
            sights: trp_sim::rig::SightNodes::of_weapon(weapon),
            arms: [arm(true)?, arm(false)?],
            limbs,
            vest: rig
                .bone("Weapon_root/Weapon_root_anim/weapon/weapon_L_IK_marker")
                .zip(rig.bone("Weapon_root/Weapon_root_anim/weapon/weapon_vest_IK_marker"))
                .map(|(left, vest)| [left, vest]),
            rig,
        })
    }
}

/// One rendered frame to replay: its time, its length, and where the game
/// was looking if that is known.
struct Frame {
    time: f32,
    /// The script's own time on this frame: the frame's, but behind it by
    /// as long as the script's gates were shut (recorder 0.4.9 on).
    script: f32,
    /// The gate whose key the recorder pressed on this frame, if one did.
    gate_press: Option<usize>,
    dt: f32,
    /// How many physics steps fall in it.
    steps: u32,
    /// The frame's length by the player's own clock.
    player_dt: f32,
    look: Option<[f32; 2]>,
    /// What the recorder's script asked the view to turn in this frame
    /// (recorder 0.4.0 on), times the game's own multiplier for it: the
    /// turn before the movement state has its say.
    asked: Option<[f32; 2]>,
    /// The rounds in a magazine the recording shows being fitted on this
    /// frame.
    fitted: Option<u32>,
    /// The aim sway's direction as the game drew it (recorder 0.12.0 on).
    aim_sway: Option<[f32; 3]>,
    /// How many rounds of the weapon's kind the character had outside the
    /// weapon, as the recording last listed what was carried (recorder
    /// 0.6.2 on): what a tube is filled from.
    reach: Option<u32>,
    /// Where the weapon's clip had the weapon's own bone (recorder 0.3.0
    /// on): the sight rides on it.
    weapon_bone: Option<([f32; 3], [f32; 4])>,
    /// And where it had the marker for the right hand, by which the
    /// body's hand holds the weapon in a crawl.
    right_marker: Option<([f32; 3], [f32; 4])>,
    /// What the game's own solver for each arm was given, right then
    /// left, where a recording has it (recorder 0.4.0 on).
    given: [Option<Given>; 2],
    /// Whose inertia the raid's one curve under the walk's floor was keyed
    /// with, on a frame a recorded walk begins and shows it.
    floor_key: Option<f32>,
    /// How well fed and how well watered the character was on this frame,
    /// 1 for full, where a recording has them (recorder 0.4.7 on).
    condition: Option<[f32; 2]>,
    /// The recoil's three processes as the recording has them on this
    /// frame (the weapon's turn, its push, the camera's kick), with
    /// `--recorded-recoil`: put on the simulation's before the frame's
    /// late update, so that what follows a shot compares frame for frame
    /// apart from the game's random draws.
    recoil: Option<[[f32; 3]; 3]>,
    /// What the game's own grid measured ahead of the character, where a
    /// recording has it (recorder 0.4.8 on): on the frame the game raised
    /// the flag for a vault or a climb, the obstacle it chose by and the
    /// forward speed, within reach; on every other frame nothing within
    /// reach. A trace does not hold the map, so the replay's own grid has
    /// nothing to measure: it takes the game's.
    vault: Option<(Obstacle, f32)>,
}

/// An obstacle no move reaches: what the replay's grid is given on the
/// frames the game's found nothing to do.
const OUT_OF_REACH: Obstacle = Obstacle { height: 0.0, length: 0.0, distance: f32::MAX, behind: 0.0, roof: None, terrain: false };

/// The game's measurement for each frame of a recording that has it
/// (`Frame::vault`).
fn recorded_obstacles(frames: &[(f32, Sample)]) -> Vec<Option<(Obstacle, f32)>> {
    let flag = |i: usize, name: &str| frames[i].1.get(name).and_then(|v| v.first().copied()) == Some(1.0);
    let asked = |i: usize| flag(i, "p_DoVault") || flag(i, "p_DoClimb");
    (0..frames.len())
        .map(|i| {
            let measured = frames[i].1.get("vault_obstacle").filter(|m| m.len() == 4)?;
            let chosen = asked(i) && (i == 0 || !asked(i - 1));
            Some(if chosen {
                // How far off it was is not on record; that it was within
                // reach is what the game's choice says.
                (Obstacle { height: measured[0], length: measured[1], behind: measured[2], distance: 0.0, roof: None, terrain: false }, measured[3])
            } else {
                (OUT_OF_REACH, measured[3])
            })
        })
        .collect()
}

/// One arm's solver as the game ran it in a frame: where the palm ended
/// and where the elbow's goal was, in the frame the rig hangs in (at the
/// game's own ribcage, turned to the view, at true size), and that
/// ribcage in the player's axes.
#[derive(Clone, Copy)]
struct Given {
    palm: ([f32; 3], [f32; 4]),
    goal: [f32; 3],
    ribcage: [f32; 3],
}

impl Given {
    /// `squeeze` is how far the recording's bones below the ribcage are
    /// drawn in along the view.
    fn of(sample: &Sample, side: &str, squeeze: f32) -> Option<Self> {
        let (palm_at, palm_turn) = recorded_bone(sample, &format!("{side}_palm"))?;
        let goal = sample.get(&format!("{side}_bend_goal_p")).filter(|g| g.len() == 3)?;
        let ribcage = sample.get("ribcage_p").filter(|r| r.len() == 3)?;
        let ribcage = [ribcage[0], ribcage[1], ribcage[2]];
        let (look, body) = (sample.get("look").filter(|l| l.len() == 2)?, sample.get("body_yaw")?.first()?);
        let view = quat_mul(quat_axis(1, look[0] - body), quat_axis(0, look[1]));
        let back = [-view[0], -view[1], -view[2], view[3]];
        let into_rig = |at: [f32; 3], squeezed: bool| {
            let mut from = quat_rotate(back, std::array::from_fn(|k| at[k] - ribcage[k]));
            if squeezed {
                from[2] /= squeeze;
            }
            from
        };
        Some(Self {
            palm: (into_rig(palm_at, true), quat_mul(back, palm_turn)),
            goal: into_rig([goal[0], goal[1], goal[2]], false),
            ribcage,
        })
    }
}

/// A bone the recorder wrote as `<name>_p` and `<name>_q`: where it is
/// and how it is turned.
fn recorded_bone(sample: &Sample, name: &str) -> Option<([f32; 3], [f32; 4])> {
    match (sample.get(&format!("{name}_p")), sample.get(&format!("{name}_q"))) {
        (Some(p), Some(q)) if p.len() == 3 && q.len() == 4 => Some(([p[0], p[1], p[2]], [q[0], q[1], q[2], q[3]])),
        _ => None,
    }
}

/// The rounds in the magazine a recorded reload fitted, on the frame the
/// recording's count jumps: what is then in the magazine, and the round
/// that went from it into an empty chamber.
fn fitted_magazine(frames: &[(f32, Sample)], i: usize) -> Option<u32> {
    let value = |i: usize, name: &str| frames[i].1.get(name).and_then(|v| v.first().copied());
    let before = i.checked_sub(1)?;
    let (was, now) = (value(before, "magazine")?, value(i, "magazine")?);
    let chambered = (value(i, "chamber")? - value(before, "chamber")?).max(0.0);
    (now > was).then_some((now + chambered) as u32)
}

/// The player's own frame time on each recorded frame, where it shows.
///
/// The game runs the player on a stopwatch read in whole milliseconds. A
/// recording made with the recorder of 0.2.0 or later states the step
/// (`clock_dt`). An earlier one does not, but the game's velocity figure
/// is the frame's motion over that time, so on a frame in which the player
/// moved the motion over the velocity gives it.
fn player_clock(frames: &[(f32, Sample)]) -> Vec<Option<f32>> {
    let vector = |i: usize, name: &str| frames[i].1.get(name).filter(|v| v.len() == 3);
    (0..frames.len())
        .map(|i| {
            if let Some(stated) = frames[i].1.get("clock_dt").and_then(|v| v.first().copied()).filter(|dt| *dt > 0.0) {
                return Some(stated);
            }
            let (here, before, velocity) = (vector(i, "pos")?, vector(i.checked_sub(1)?, "pos")?, vector(i, "vel")?);
            let moved = (0..3).map(|k| (here[k] - before[k]).powi(2)).sum::<f32>().sqrt();
            let speed = velocity.iter().map(|v| v * v).sum::<f32>().sqrt();
            if moved < 2e-4 || speed < 1e-2 {
                return None;
            }
            let milliseconds = moved / speed * 1000.0;
            let whole = milliseconds.round();
            (whole >= 1.0 && (milliseconds - whole).abs() < 0.1).then_some(whole / 1000.0)
        })
        .collect()
}

/// Whose inertia the curve under the walk's floor was keyed with as each
/// recorded walk began: the frame's number and that inertia.
///
/// The game keeps one such curve for the whole raid, and every character
/// in it, a bot too, re-keys it with its own inertia whenever its carried
/// weight changes (movement spec, "The floor's curve is shared"). The
/// walking state reads it as the walk begins. A walk from standing begins
/// with its speed under the floor, so on its first frame the speed
/// parameter is drawn toward the floor itself, by a known share of the
/// way: how far it moved says where the floor was. Only a recording that
/// states the player's own frame time says so closely enough.
fn floor_keys(recorded: &[(f32, Sample)], settings: &MovementSettings) -> Vec<(usize, f32)> {
    let value = |i: usize, name: &str| recorded[i].1.get(name).and_then(|v| v.first().copied());
    let mut keys = Vec::new();
    for i in 1..recorded.len() {
        if value(i - 1, "movement_state") != Some(0.0) || value(i, "movement_state") != Some(1.0) {
            continue;
        }
        // From recorder 0.4.5 on the recording has the key itself.
        if let Some(key) = recorded[i].1.get("floor_key").filter(|key| key.len() == 2) {
            let [none, full] = settings.acceleration_range_right;
            if (full[0] - none[0]).abs() > 1e-6 {
                keys.push((i, ((key[0] - none[0]) / (full[0] - none[0])).clamp(0.0, 1.0)));
                continue;
            }
        }
        let (Some(before), Some(now), Some(clamped), Some(dt)) =
            (value(i - 1, "smoothed_speed"), value(i, "smoothed_speed"), value(i, "clamped_speed"), value(i, "clock_dt"))
        else {
            continue;
        };
        let share = settings.sudden_changes_smoothness * dt;
        // Within a thousandth of the floor the parameter is put on it at
        // once; further off it goes that share of the way. A move too
        // small for the second and too large to be none is either, and a
        // floor at nothing says nothing of the curve.
        let floor = match (now - before).abs() {
            moved if moved < 0.5e-3 * share => now,
            moved if moved < 1e-3 => continue,
            _ => before + (now - before) / share,
        };
        if floor < 1e-3 {
            continue;
        }
        if let Some(inertia) = settings.acceleration_key_giving(clamped - floor, 3e-4) {
            keys.push((i, inertia));
        }
    }
    keys
}

/// The first frame of a recording's first walk from standing: its number,
/// the keys held, and how far the direction then stands from where a walk
/// starts it.
///
/// The game's walking state keeps the rate its direction smoothing ended
/// the last walk on, and on a walk's first frame that rate is all that
/// moves the direction. The last walk was in the scenario before, or the
/// player's own on the way to the spot, so this is the only place it shows.
struct FirstWalk {
    frame: usize,
    keys: [f32; 2],
    shift: [f32; 2],
}

impl FirstWalk {
    fn of(frames: &[(f32, Sample)]) -> Option<Self> {
        let state = |i: usize| frames[i].1.get("movement_state").and_then(|s| s.first().copied());
        let frame = (1..frames.len()).find(|&i| state(i) == Some(1.0))?;
        if state(frame - 1) != Some(0.0) {
            return None;
        }
        let pair = |name: &str| frames[frame].1.get(name).filter(|v| v.len() == 2).map(|v| [v[0], v[1]]);
        let (keys, direction) = (pair("move")?, pair("dir")?);
        // Standing, a key sets the direction to a hundredth its way.
        let start = keys.map(|k| if k == 0.0 { 0.0 } else { 0.01 * k.signum() });
        Some(Self { frame, keys, shift: [direction[0] - start[0], direction[1] - start[1]] })
    }
}

/// The most a recording shows in the legs: their capacity, if the
/// character was rested at any moment of it. Read as text, since the
/// traces are large and only this one figure is wanted of each frame.
fn recorded_legs_capacity(traces: Option<&Path>) -> Option<f32> {
    const NAME: &str = "\"stamina\":";
    let mut most: Option<f32> = None;
    for entry in std::fs::read_dir(traces?).ok()?.flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|e| e != "jsonl") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else { continue };
        for (at, _) in text.match_indices(NAME) {
            let rest = &text[at + NAME.len()..];
            let end = rest.find([',', '}']).unwrap_or(rest.len());
            if let Ok(value) = rest[..end].parse::<f32>() {
                most = Some(most.map_or(value, |m| m.max(value)));
            }
        }
    }
    most
}

/// Where a recording's stamina stood as a scenario began, as far as its
/// frames show: what was in the legs and the arms, when each next began
/// to refill, what the character's food made of the refill (the game's
/// energy is not recorded and falls all through a raid), and how much of
/// a rise of the stance was already in the tally that charges for it.
struct StaminaStart {
    pools: [f32; 2],
    refill_from: [Option<f32>; 2],
    thirst_factor: Option<f32>,
    energy_factor: Option<f32>,
    pose_rise: Option<f32>,
}

impl StaminaStart {
    fn of(
        frames: &[(f32, Sample)],
        settings: &trp_sim::StaminaSettings,
        capacities: [f32; 2],
        aim_mass: f32,
        thirst_before: Option<f32>,
    ) -> Option<Self> {
        const NAMES: [&str; 2] = ["stamina", "hands_stamina"];
        let at = |i: usize, name: &str| frames.get(i).and_then(|f| f.1.get(name)).and_then(|v| v.first().copied());
        let first = [at(0, NAMES[0])?, at(0, NAMES[1])?];
        // The simulation's first frame works the pools once more: begun a
        // frame's refill or drain back, it stands where the recording does
        // after it. A charge on the second frame is no rate.
        let step = |i: usize| at(i, "clock_dt").or(at(i, "dt")).unwrap_or(DT);
        let pools = [0, 1].map(|p| match at(1, NAMES[p]).map(|next| next - first[p]) {
            Some(change) if change.abs() < 0.2 && step(1) > 0.0 => first[p] - change * step(0) / step(1),
            _ => first[p],
        });
        // The engine's clock as the simulation counts it: each frame's
        // length added before the frame is worked.
        let mut clock = 0.0;
        let clocks: Vec<f32> = (0..frames.len())
            .map(|i| {
                clock += at(i, "dt").unwrap_or(DT);
                clock
            })
            .collect();
        let pose = |i: usize| match (at(i, "prone"), at(i, "pose")) {
            (Some(lying), _) if lying > 0.0 => 0,
            (_, Some(level)) if level < 0.11 => 1,
            _ => 2,
        };
        // A pool that starts short and still: it refills from the first
        // frame it rises on, if nothing is charged to it before.
        let refill_from = [0, 1].map(|p| {
            let still = pools[p] < capacities[p] - 1e-3 && at(1, NAMES[p]) == Some(first[p]);
            let rises = (1..frames.len()).find(|&i| at(i, NAMES[p]) > at(i - 1, NAMES[p]))?;
            still.then(|| (clocks[rises - 1] + clocks[rises]) / 2.0)
        });
        // What thirst makes of the drains: a sprint's and aiming's cost a
        // second on every frame of either, over what the settings give for
        // a character that has drunk its fill.
        let on = |i: usize, name: &str| at(i, name) == Some(1.0) && at(i - 1, name) == Some(1.0);
        let mut thirsts: Vec<f32> = Vec::new();
        for i in 1..frames.len() {
            let spent = |p: usize| at(i - 1, NAMES[p]).zip(at(i, NAMES[p])).map_or(0.0, |(last, now)| last - now) / step(i).max(1e-6);
            if on(i, "sprinting") && spent(0) > 0.0 && spent(0) < 12.0 {
                thirsts.push(spent(0) / settings.sprint_drain - settings.overweight);
            }
            let aiming = aim_mass.max(0.0).sqrt() * settings.aim_drain * settings.aim_by_pose[pose(i)] * (1.0 - settings.strength_aim);
            if on(i, "firearm_aiming") && spent(1) > 0.0 && spent(1) < 12.0 && aiming > 0.0 {
                thirsts.push(spent(1) / aiming);
            }
        }
        thirsts.sort_by(f32::total_cmp);
        let thirst_factor = (thirsts.len() >= 8).then(|| thirsts[thirsts.len() / 2]);
        let thirst = thirst_factor.or(thirst_before).unwrap_or(1.0).max(1.0);
        // The refill a second on every frame a pool rises without reaching
        // its top, over what the settings give before food has its say.
        let mut factors: Vec<f32> = Vec::new();
        for i in 1..frames.len() {
            let before = settings.refill_before_energy(pose(i)).map(|rate| rate / thirst);
            for p in 0..2 {
                let (Some(now), Some(last)) = (at(i, NAMES[p]), at(i - 1, NAMES[p])) else { continue };
                if now > last && now < capacities[p] - 1e-3 && step(i) > 0.0 {
                    factors.push((now - last) / step(i) / before[p]);
                }
            }
        }
        factors.sort_by(f32::total_cmp);
        let energy_factor = (factors.len() >= 8).then(|| factors[factors.len() / 2]);
        // The stance's tally: where the first charge for a rise falls says
        // how much was in it; with no charge, it was short of charging by
        // more than the stance rose.
        // Every frame bounds it: on a frame with a charge the tally had
        // reached three notches, and after every frame it was short of
        // them. The slow end of a rise bounds it closest.
        let (third, charged_at, taken) = (settings.stance_third(), 3.0 * settings.delta_level, settings.delta_level / 3.0);
        let rise = |i: usize| match (at(i, "smoothed_pose"), at(i - 1, "smoothed_pose"), at(i, "pose")) {
            // Within a thousandth of its target the stance is put there,
            // and that last step is not counted.
            (Some(now), Some(last), Some(target)) if (target - last).abs() > 0.001 => (now - last).max(0.0),
            _ => 0.0,
        };
        let (mut risen, mut paid) = (0.0, 0.0);
        let (mut least, mut most) = (f32::NEG_INFINITY, f32::INFINITY);
        for i in 1..frames.len() {
            risen += rise(i);
            // Getting up is charged on the frame of the key, which may be
            // a frame the stance is charged on too.
            let stood = at(i - 1, "prone") == Some(1.0) && at(i, "prone") == Some(0.0);
            let known = if stood { settings.standup[0] + (settings.standup[1] - settings.standup[0]) * settings.overweight } else { 0.0 };
            let dropped = (at(i - 1, NAMES[0]).zip(at(i, NAMES[0])).map_or(0.0, |(last, now)| last - now) - known) / third;
            if rise(i) > 0.0 && (1.0..=6.5).contains(&dropped) && (dropped - dropped.round()).abs() < 0.02 {
                least = least.max(charged_at * dropped.round() - risen + paid * taken);
                paid += dropped.round();
            }
            most = most.min(charged_at - risen + paid * taken);
        }
        let pose_rise = if least > most {
            least
        } else if least.is_finite() {
            (least + most) / 2.0
        } else {
            // No charge at all: anywhere short of one, as a tally stands
            // between charges.
            (charged_at - settings.delta_level / 6.0).min(most - 1e-4)
        };
        Some(Self { pools, refill_from, thirst_factor, energy_factor, pose_rise: Some(pose_rise.max(0.0)) })
    }
}

/// Even ground: at some height where the scenario began, rising by `slope`
/// metres a metre along the world's x and z.
struct Ground {
    height: f32,
    slope: [f32; 2],
    /// How far under this the terrain itself lies. The character rides
    /// over the ground by up to its ground check and the replay's ground
    /// is what it rides on; what is pushed out of the world or cast into
    /// it (the camera's box, the weapon's line) meets the terrain.
    below: f32,
    /// The ground as the recording's own path shows it, in place of the
    /// plane above, for a recording made over something: a wall to vault,
    /// a ledge to fall from.
    profile: Option<Profile>,
    /// A sill the recording was made at (`mount.txt`, `recorded_mount`):
    /// what a mount's search meets besides the terrain.
    sill: Option<trp_sim::collision::Block>,
    /// The floor a mount's target place stands on, where the folder gives it
    /// (`mount.txt`, `floor`): the static layer's rays down meet it there.
    floor: Option<f32>,
}

/// What a recording's folder says of a mount the recorder could not see
/// the world of (`mount.txt`): a sill, `sill <x> <z> <yaw> <top> [<width>
/// <depth>]`, the middle of its face (x and z from where the character stood at
/// the trace's start), the way the character looked at it (degrees), its top's
/// height over the feet at the start, and its size along the face and
/// back from it (2 and 0.4 m), from the terrain up; and `blocked`, the walk's
/// lock on at the start (movement spec, "Stopped by what stands in the way":
/// a character that has walked into the sill and stands, which the trace
/// begins after). Read off what the game's mount found, not measured.
fn recorded_mount(traces: Option<&Path>) -> (Option<([f32; 2], f32, f32, [f32; 2])>, bool, Option<f32>) {
    let text = traces.and_then(|dir| std::fs::read_to_string(dir.join("mount.txt")).ok()).unwrap_or_default();
    let mut sill = None;
    let mut blocked = false;
    let mut floor = None;
    for line in text.lines().filter(|line| !line.trim_start().starts_with('#')) {
        let words: Vec<&str> = line.split_whitespace().collect();
        let numbers: Vec<f32> = words.iter().skip(1).filter_map(|word| word.parse().ok()).collect();
        match words.first() {
            Some(&"sill") if numbers.len() >= 4 => {
                let size = if numbers.len() >= 6 { [numbers[4], numbers[5]] } else { [2.0, 0.4] };
                sill = Some(([numbers[0], numbers[1]], numbers[2], numbers[3], size));
            }
            Some(&"blocked") => blocked = true,
            // The floor under the target place, from the feet at the start.
            Some(&"floor") if !numbers.is_empty() => floor = Some(numbers[0]),
            _ => {}
        }
    }
    (sill, blocked, floor)
}

/// What a recorded path shows of the ground under it (recorder 0.4.3 on,
/// which writes on every frame how far under the character something
/// solid lies): the height of whatever was under the character's axis at
/// each place it passed, by how far out that place is along the way the
/// character first faced. A wall climbed shows as a step up where the
/// axis came over its face, and the far side of it as a step down.
///
/// It is the ground along one line only, and only where the character
/// went: enough to walk the same line again, which is what a replay does.
struct Profile {
    /// The sine and cosine of the first heading.
    heading: [f32; 2],
    /// The ground along that line, cut into pieces at its steps, for the
    /// character's capsule to be kept out of as the game's own controller
    /// keeps it (`trp_sim::collision`).
    course: Course,
    /// The capsule's radius, from the player prefab.
    radius: f32,
}

impl Profile {
    /// Places nearer one another than this are one place: the later
    /// frame's word on it stands.
    const GRAIN: f32 = 0.004;

    fn of(frames: &[(f32, Sample)], radius: f32) -> Option<Self> {
        let yaw = frames.first()?.1.get("look")?.first()?.to_radians();
        let heading = [yaw.sin(), yaw.cos()];
        let mut points: Vec<[f32; 2]> = Vec::new();
        for (_, sample) in frames {
            let (Some(at), Some(below)) = (sample.get("pos").filter(|p| p.len() == 3), sample.get("ground_below").and_then(|b| b.first())) else {
                continue;
            };
            let point = [at[0] * heading[0] + at[2] * heading[1], at[1] - below];
            match points.binary_search_by(|p| p[0].total_cmp(&point[0])) {
                Ok(index) => points[index] = point,
                Err(index) => {
                    let near = |i: usize| points.get(i).is_some_and(|p: &[f32; 2]| (p[0] - point[0]).abs() < Self::GRAIN);
                    if index > 0 && near(index - 1) {
                        points[index - 1] = point;
                    } else if near(index) {
                        points[index] = point;
                    } else {
                        points.insert(index, point);
                    }
                }
            }
        }
        (points.len() > 1).then(|| Self { heading, course: Course::from_path(&points), radius })
    }

    fn along(&self, x: f32, z: f32) -> f32 {
        x * self.heading[0] + z * self.heading[1]
    }

    /// The ground's height at a place along the line.
    fn height(&self, along: f32) -> f32 {
        self.course.height(along)
    }

    /// A place in the world as the course has it: how far along, how high.
    fn on_course(&self, at: [f32; 3]) -> [f32; 2] {
        [self.along(at[0], at[2]), at[1]]
    }
}

impl World for Ground {
    fn ground_height(&self, x: f32, z: f32) -> f32 {
        match &self.profile {
            Some(profile) => profile.height(profile.along(x, z)),
            None => self.height + self.slope[0] * x + self.slope[1] * z,
        }
    }

    fn ground_normal(&self, _x: f32, _z: f32) -> [f32; 3] {
        let [x, z] = self.slope;
        let len = (x * x + 1.0 + z * z).sqrt();
        [-x / len, 1.0 / len, -z / len]
    }

    /// On a recorded path's ground the character is kept out of what
    /// stands on it as the game's own controller keeps it, along the
    /// path's line; what the move does across that line is left alone.
    fn obstructed(&self, from: [f32; 3], to: [f32; 3], capsule: Capsule) -> [f32; 3] {
        let Some(profile) = &self.profile else { return to };
        let (before, wanted) = (profile.on_course(from), profile.on_course(to));
        let settled = profile.course.settle(before, wanted, capsule, profile.radius);
        let onward = settled[0] - wanted[0];
        [to[0] + onward * profile.heading[0], settled[1], to[2] + onward * profile.heading[1]]
    }

    /// A recorded path's course puts the feet on its ground itself.
    fn sets_feet(&self) -> bool {
        self.profile.is_some()
    }

    fn held(&self, at: [f32; 3], capsule: Capsule) -> bool {
        self.profile.as_ref().is_some_and(|profile| profile.course.held(profile.on_course(at), capsule, profile.radius))
    }

    fn line(&self, from: [f32; 3], to: [f32; 3]) -> Option<f32> {
        Terrain(self).line(from, to)
    }

    /// A mount's rays meet the sill where there is one, and the terrain.
    fn ray(&self, from: [f32; 3], direction: [f32; 3], length: f32, layer: trp_sim::mounting::Layer) -> Option<trp_sim::mounting::Hit> {
        let mut terrain = Terrain(self).ray(from, direction, length, layer);
        // The target place's floor, where the folder gives it.
        if let (Some(floor), trp_sim::mounting::Layer::Static) = (self.floor.map(|f| f + self.height), layer) {
            if direction[1] < 0.0 && from[1] >= floor && from[1] + direction[1] * length < floor {
                let distance = (from[1] - floor) / -direction[1];
                terrain = Some(trp_sim::mounting::Hit { distance, point: std::array::from_fn(|k| from[k] + direction[k] * distance), normal: [0.0, 1.0, 0.0] });
            }
        }
        let to: [f32; 3] = std::array::from_fn(|k| from[k] + direction[k] * length);
        let sill = self.sill.as_ref().and_then(|block| block.entered_face(from, to)).map(|(share, normal)| {
            let distance = share * length;
            trp_sim::mounting::Hit { distance, point: std::array::from_fn(|k| from[k] + direction[k] * distance), normal }
        });
        match (sill, terrain) {
            (Some(s), Some(t)) => Some(if s.distance <= t.distance { s } else { t }),
            (s, t) => s.or(t),
        }
    }

    fn sphere_clear(&self, centre: [f32; 3], radius: f32, layer: trp_sim::mounting::Layer) -> bool {
        Terrain(self).sphere_clear(centre, radius, layer) && !self.sill.as_ref().is_some_and(|block| block.sphere_overlaps(centre, radius))
    }

    /// The way to a mount's place against the sill, the box tried every 5 cm.
    fn box_sweep(&self, from: [f32; 3], half: [f32; 3], yaw: f32, direction: [f32; 3], length: f32, _layer: trp_sim::mounting::Layer) -> bool {
        let Some(block) = &self.sill else { return false };
        let tries = (length / 0.05).ceil().max(1.0) as usize;
        (0..=tries).any(|i| {
            let along = length * i as f32 / tries as f32;
            block.upright_box_overlaps(std::array::from_fn(|k| from[k] + direction[k] * along), half, yaw)
        })
    }

    fn push_out(&self, centre: [f32; 3], half: [f32; 3], turn: [f32; 4], margin: f32) -> [f32; 3] {
        Terrain(self).push_out(centre, half, turn, margin)
    }

    /// The head's capsule: out of the terrain, and out of the sill where
    /// the folder gives one, each lengthened by a millimetre.
    fn capsule_push(&self, ends: [[f32; 3]; 2], radius: f32) -> [f32; 3] {
        let mut push = Terrain(self).capsule_push(ends, radius);
        if let Some((way, depth)) = self.sill.as_ref().and_then(|block| block.capsule_push(ends, radius)) {
            for k in 0..3 {
                push[k] += way[k] * (depth + trp_sim::movement::pom::MARGIN);
            }
        }
        push
    }
}

/// A recording's kit (`equipment`: each thing with its parent's id and the
/// slot it is in, under the one with no parent) as a tree for the armour.
fn recorded_kit(equipment: &[Json]) -> Vec<trp_sim::armor::Worn> {
    fn under(equipment: &[Json], parent: &str) -> Vec<trp_sim::armor::Worn> {
        equipment
            .iter()
            .filter(|e| e.get("in").and_then(Json::as_str) == Some(parent))
            .map(|e| trp_sim::armor::Worn {
                template: e.get("tpl").and_then(Json::as_str).unwrap_or("").to_owned(),
                slot: e.get("at").and_then(Json::as_str).unwrap_or("").to_owned(),
                children: under(equipment, e.get("id").and_then(Json::as_str).unwrap_or("")),
            })
            .collect()
    }
    let root = equipment.iter().find(|e| e.get("in").is_none()).and_then(|e| e.get("id")).and_then(Json::as_str).unwrap_or("");
    under(equipment, root)
}

/// The terrain under the ground the character rides on.
struct Terrain<'a>(&'a Ground);

impl World for Terrain<'_> {
    fn ground_height(&self, x: f32, z: f32) -> f32 {
        self.0.ground_height(x, z) - self.0.below
    }

    fn ground_normal(&self, x: f32, z: f32) -> [f32; 3] {
        self.0.ground_normal(x, z)
    }
}

/// The ground a recording was made on, from `ground.txt` in its folder if
/// there is one: the rise in metres per metre along the world's x and
/// along its z, and after them, if it is known, how far over the ground
/// the character stood on the spot every scenario starts from (a standing
/// jump from there lands that much lower). Level, and stood on, without
/// the file.
fn recorded_slope(traces: Option<&Path>) -> ([f32; 2], f32) {
    let numbers: Vec<f32> = ground_words(traces).iter().filter_map(|word| word.parse().ok()).collect();
    match numbers[..] {
        [x, z] => ([x, z], 0.0),
        [x, z, over] => ([x, z], over.max(0.0)),
        _ => ([0.0; 2], 0.0),
    }
}

/// The words of a recording's `ground.txt`, its notes left out.
fn ground_words(traces: Option<&Path>) -> Vec<String> {
    let text = traces.and_then(|dir| std::fs::read_to_string(dir.join("ground.txt")).ok()).unwrap_or_default();
    text.lines().filter(|line| !line.trim_start().starts_with('#')).flat_map(str::split_whitespace).map(str::to_owned).collect()
}

/// Whether a recording's `ground.txt` says its ground is to be taken from
/// each trace's own path (the word `profile`): one made at an obstacle or
/// a ledge, where no plane will do.
fn recorded_profile(traces: Option<&Path>) -> bool {
    ground_words(traces).iter().any(|word| word == "profile")
}

/// The lower body's bones the replay has the animator pose: the name each
/// goes by in a trace (none for the root), the bone's own name, and which
/// of these it hangs from (itself for the root).
const LEGS: [(&str, &str, usize); 13] = [
    ("", "Root_Joint", 0),
    ("pelvis", "Base HumanPelvis", 0),
    ("spine1", "Base HumanSpine1", 1),
    ("left_thigh1", "Base HumanLThigh1", 1),
    ("left_thigh2", "Base HumanLThigh2", 3),
    ("left_calf", "Base HumanLCalf", 4),
    ("left_foot", "Base HumanLFoot", 5),
    ("left_toe", "Base HumanLToe", 6),
    ("right_thigh1", "Base HumanRThigh1", 1),
    ("right_thigh2", "Base HumanRThigh2", 8),
    ("right_calf", "Base HumanRCalf", 9),
    ("right_foot", "Base HumanRFoot", 10),
    ("right_toe", "Base HumanRToe", 11),
];

/// Their paths below the animated object.
fn leg_paths() -> Vec<String> {
    let mut paths: Vec<String> = Vec::with_capacity(LEGS.len());
    for (index, (_, bone, above)) in LEGS.iter().enumerate() {
        paths.push(if *above == index { (*bone).to_owned() } else { format!("{}/{bone}", paths[*above]) });
    }
    paths
}

/// The length of the line the game cast along the weapon in a recording
/// whose traces do not say (before recorder 0.4.3), if the recording's
/// folder does: one number, in metres, in `weapon_length.txt`.
/// The weapon a folder of traces was recorded with: what the first line
/// of its first trace names.
fn recorded_weapon(traces: Option<&Path>) -> Option<String> {
    use std::io::BufRead;
    let mut files: Vec<PathBuf> = std::fs::read_dir(traces?).ok()?.filter_map(|e| Some(e.ok()?.path())).filter(|p| p.extension().is_some_and(|e| e == "jsonl")).collect();
    files.sort();
    let mut line = String::new();
    std::io::BufReader::new(std::fs::File::open(files.first()?).ok()?).read_line(&mut line).ok()?;
    let header: Json = serde_json::from_str(&line).ok()?;
    header.get("weapon").and_then(Json::as_str).filter(|id| !id.is_empty()).map(str::to_owned)
}

/// The weapon in hand as the recording's equipment has it (recorder 0.11.0
/// on: every item with its parent and slot), as a build: the first item of
/// the weapon's template in a weapon slot, and every part below it but the
/// rounds. `None` where the first trace's header lists no such item.
fn recorded_build(traces: Option<&Path>, weapon: &str) -> Option<trp_data::database::Preset> {
    use std::io::BufRead;
    let mut files: Vec<PathBuf> = std::fs::read_dir(traces?).ok()?.filter_map(|e| Some(e.ok()?.path())).filter(|p| p.extension().is_some_and(|e| e == "jsonl")).collect();
    files.sort();
    let mut line = String::new();
    std::io::BufReader::new(std::fs::File::open(files.first()?).ok()?).read_line(&mut line).ok()?;
    let header: Json = serde_json::from_str(&line).ok()?;
    let items = header.get("equipment")?.as_array()?;
    let text = |item: &Json, key: &str| item.get(key).and_then(Json::as_str).map(str::to_owned);
    let root = items.iter().find(|item| {
        text(item, "tpl").as_deref() == Some(weapon) && matches!(text(item, "at").as_deref(), Some("FirstPrimaryWeapon" | "SecondPrimaryWeapon" | "Holster"))
    })?;
    let root_id = text(root, "id")?;
    let mut build = vec![trp_data::database::PresetItem { id: root_id.clone(), template: weapon.to_owned(), parent: None, slot: None }];
    let mut wanted = vec![root_id.clone()];
    while let Some(parent) = wanted.pop() {
        for item in items.iter().filter(|item| text(item, "in").as_deref() == Some(parent.as_str())) {
            let slot = text(item, "at")?;
            if slot == "cartridges" || slot == "patron_in_weapon" {
                continue;
            }
            let id = text(item, "id")?;
            build.push(trp_data::database::PresetItem { id: id.clone(), template: text(item, "tpl")?, parent: Some(parent.clone()), slot: Some(slot) });
            wanted.push(id);
        }
    }
    Some(trp_data::database::Preset { id: "recorded".into(), name: "recorded".into(), root: root_id, items: build, encyclopedia: None })
}

fn recorded_line(traces: Option<&Path>) -> Option<f32> {
    let text = std::fs::read_to_string(traces?.join("weapon_length.txt")).ok()?;
    let mut numbers = text.lines().filter(|line| !line.trim_start().starts_with('#')).flat_map(str::split_whitespace);
    numbers.find_map(|word| word.parse().ok())
}

/// How far into a physics step the game's physics clock stood when a
/// recording began, in seconds. The game's springs move in physics steps
/// only, sixty a second, so at 120 frames a second every other frame has
/// one; which frames those are shows in the recording as the frames on
/// which the weapon's rotation spring changed (breathing keeps it moving).
/// The camera's code takes the physics time on such a frame and the
/// frame's own otherwise, so a replay that steps on the other frames has
/// the camera a frame's share ahead or behind all through a move.
fn physics_phase(frames: &[(f32, Sample)]) -> f32 {
    const TRIES: usize = 16;
    let spring = |index: usize| frames[index].1.get("hands_rot");
    let moved: Vec<Option<bool>> = (1..frames.len()).map(|i| Some(spring(i)? != spring(i - 1)?)).collect();
    let mut best = (0usize, 0.0f32);
    for phase in (0..TRIES).map(|k| k as f32 / TRIES as f32 * DT) {
        let (mut due, mut agree) = (0u32, 0usize);
        for (index, (time, _)) in frames.iter().enumerate() {
            let before = due;
            while (due + 1) as f32 * DT <= time + phase + 1e-4 {
                due += 1;
            }
            agree += usize::from(index > 0 && moved[index - 1] == Some(due > before));
        }
        if agree > best.0 {
            best = (agree, phase);
        }
    }
    best.1
}

/// How many physics steps each frame of a recording had. The clock says
/// how many were due (`physics_phase`), and the recording says which
/// frames had any: those on which the weapon's rotation spring changed.
/// The two differ where the game's frame times wander and a step falls a
/// frame early or late, which the clock by one fitted phase does not
/// follow; the spring is taken at its word, and the clock for how many.
fn physics_steps(frames: &[(f32, Sample)]) -> Vec<u32> {
    let phase = physics_phase(frames);
    let spring = |index: usize| frames[index].1.get("hands_rot");
    let mut worked = 0.0f32;
    frames
        .iter()
        .enumerate()
        .map(|(index, (time, _))| {
            let due = ((time + phase + 1e-4 - worked) / DT).floor().max(0.0) as u32;
            let moved = if index > 0 { spring(index).zip(spring(index - 1)).map(|(now, before)| now != before) } else { None };
            let steps = match moved {
                Some(false) => 0,
                Some(true) => due.max(1),
                None => due,
            };
            worked += steps as f32 * DT;
            steps
        })
        .collect()
}

/// How far from its starting height the recorded character found the
/// ground, where a jump shows it.
///
/// The game pulls nothing down that is within its ground check, so a
/// character stands as much as 8 cm over ground it walked down onto, and
/// only comes to rest on it out of a jump. The height the jump ends at is
/// the ground's there, and on ground of the given slope that gives its
/// height where the scenario began.
fn recorded_ground(frames: &[(f32, Sample)], slope: [f32; 2]) -> Option<f32> {
    let value = |i: usize, name: &str| frames[i].1.get(name).and_then(|v| v.first().copied());
    let jumping = |i: usize| value(i, "movement_state") == Some(3.0);
    let first = (0..frames.len()).find(|&i| jumping(i))?;
    let last = (first..frames.len()).take_while(|&i| jumping(i)).last()?;
    if value(last, "grounded") != Some(1.0) {
        return None;
    }
    let at = frames[last].1.get("pos").filter(|p| p.len() == 3)?;
    Some(at[1] - slope[0] * at[0] - slope[1] * at[2])
}

fn replay(
    scenario: &Scenario,
    rig: &mut Rig,
    frames: &[Frame],
    first_walk: Option<FirstWalk>,
    geometry: &Geometry,
    ground: &Ground,
    show_layers: Option<usize>,
    own_view: bool,
    holding: &mut Holding,
) -> (Vec<(f32, Sample)>, Option<[f32; 2]>) {
    let Rig { movement, hands, firearm, next_rounds, no_magazine } = rig;
    let outside = hands.point_of_view() == PointOfView::ThirdPerson;
    let mut out = Vec::new();
    let mut leftover = None;
    let mut pressed = vec![false; scenario.presses.len()];
    let mut aim_key = false;
    // How deep the weapon last stuck into something, and what the mount did.
    let mut last_overlap = 0.0f32;
    let mut mount_events: Vec<(f32, trp_sim::mounting::MountEvent)> = Vec::new();
    // The replay faces the way the recording did. The bearing matters:
    // coming out of a sprint the game starts the walk's speed vector
    // pointing along the world's z axis, whichever way the player faces.
    let first_look = frames.first().and_then(|f| f.look);
    if let Some(look) = first_look {
        [movement.yaw, movement.pitch] = look;
    }
    movement.note_rotation();
    // The recorder hands the game its movement keys as they stood a frame
    // before: the game reads them ahead of the recorder's own clock moving
    // on. Its other commands go to the player directly and are not late. A
    // recorded walk starts and stops one frame after the script says.
    let mut keys_time = frames.first().map_or(0.0, |f| f.script - f.dt);
    for (index, frame) in frames.iter().enumerate() {
        // The game moves the player once per drawn frame, by that frame's
        // own length, with the keys as they stand at that moment by the
        // script's own clock.
        let time = frame.script;
        if let Some(walk) = first_walk.as_ref().filter(|w| w.frame == index) {
            let rate = movement.walk_leftover_for(walk.shift, walk.keys, frame.player_dt);
            movement.set_walk_leftover(rate);
            leftover = Some(rate);
        }
        if let Some(inertia) = frame.floor_key {
            movement.settings.key_acceleration_range(inertia);
        }
        // Hunger and thirst as the recording has them, which stamina's
        // refill and drains go by.
        if let Some([energy, hydration]) = frame.condition {
            (movement.stamina.settings.energy, movement.stamina.settings.hydration) = (energy, hydration);
        }
        let held_at = std::mem::replace(&mut keys_time, time);
        let key = |name: &str, time: f32| scenario.held(name, time) as i32 as f32;
        let mut input = Input {
            move_dir: [key("d", held_at) - key("a", held_at), key("w", held_at) - key("s", held_at)],
            sprint: scenario.held("shift", time),
            lean_keys: [scenario.held("q", time), scenario.held("e", time)],
            step: (key("step_right", time) - key("step_left", time)) as i32,
            vault: scenario.held("vault", time),
            aiming: false,
            // The recorded view has been through the game's own limits on
            // turning; a scripted turn with no recording has not.
            free_look: frame.look.is_some(),
            ..Input::default()
        };
        match (frame.asked.filter(|_| own_view), frame.look) {
            // Turn the view as the recorder's script asked the game to,
            // and let the movement state slow and limit it.
            (Some(asked), _) => {
                input.look_delta = asked;
                input.free_look = false;
            }
            // Follow where the game looked.
            (None, Some(look)) => {
                input.look_delta = [delta_angle(movement.yaw, look[0]), look[1] - movement.pitch];
            }
            (None, None) => {
                for (_, _, yaw, pitch) in scenario.turns.iter().filter(|t| time >= t.0 && time < t.1) {
                    input.look_delta[0] += yaw * frame.dt;
                    input.look_delta[1] += pitch * frame.dt;
                }
            }
        }
        // The aim sway's direction as the game drew it, where recorded.
        if let Some(direction) = frame.aim_sway {
            hands.aim_sway_override = Some(direction);
        }
        let mut keys: Vec<&str> = Vec::new();
        for (press, (key, at)) in scenario.presses.iter().enumerate() {
            if time >= *at && !std::mem::replace(&mut pressed[press], true) {
                keys.push(key);
            }
        }
        // And the key of a shut gate, on the frames the recorder pressed it.
        let gate = frame.gate_press.and_then(|gate| scenario.gates.get(gate));
        keys.extend(gate.and_then(|gate| gate.press.as_ref()).map(|(key, _)| key.as_str()));
        for key in keys {
            let s = &movement.settings;
            match key {
                "jump" => input.jump = true,
                "crouch" => input.crouch = true,
                "prone" => input.prone = true,
                // The magazine going in holds what the recording says it
                // did: the game counts it from the reload's first frame.
                // A tube is filled from what the recording says was carried.
                "reload" if firearm.settings.internal_magazine => firearm.reload_shells(frame.reach.unwrap_or(u32::MAX / 2)),
                "reload" => {
                    let near = &frames[index.saturating_sub(1)..(index + 4).min(frames.len())];
                    match near.iter().find_map(|f| f.fitted).or(*next_rounds) {
                        Some(rounds) => firearm.reload_with(rounds),
                        None if *no_magazine => {}
                        None => firearm.reload(),
                    }
                }
                "wheel_up" => input.speed_delta += s.delta_speed,
                "wheel_down" => input.speed_delta -= s.delta_speed,
                "stance_up" => input.pose_delta += s.delta_level,
                "stance_down" => input.pose_delta -= s.delta_level,
                "auto" => _ = firearm.set_fire_mode(FireMode::FullAuto),
                "single" => _ = firearm.set_fire_mode(FireMode::Single),
                "semiauto" => _ = firearm.set_fire_mode(FireMode::SemiAuto),
                "check_magazine" => _ = firearm.check_magazine(),
                "check_chamber" => firearm.check_chamber(),
                // Recorder 0.12.0: the selector's look and the fire mode key,
                // the sights' keys and zeroing, as the app plays them.
                "check_fire_mode" => _ = firearm.check_fire_mode(),
                "fire_mode_next" => firearm.next_fire_mode(),
                "sight_next" => _ = hands.change_aim_scope(movement.mounted()),
                "magnification" => {
                    if hands.change_magnification() {
                        firearm.mod_switch();
                    }
                }
                "zero_up" => _ = hands.step_zeroing(true),
                "zero_down" => _ = hands.step_zeroing(false),
                // Recorder 0.13.0: blind fire's commands as the game's input
                // root gives them (`cmd:`; blind-fire spec, "The keys").
                "cmd:ToggleBlindAbove" => input.blind_toggle = 1,
                "cmd:ToggleBlindRight" => input.blind_toggle = -1,
                "cmd:BlindShootEnd" => input.blind_end = true,
                // The mount's and the bipod's (mounting spec, "The keys").
                "cmd:WeaponMounting" => input.mount = true,
                // The left shoulder's (left-shoulder spec, "The key").
                "cmd:LeftStanceToggle" => input.left_stance = true,
                "cmd:ToggleBipods" => {

                    if firearm.toggle_bipod(movement.mounted(), movement.sprinting, movement.gesture_free()) {
                        if let (Some(out), Some(forward)) = (firearm.bipod(), firearm.settings.bipod) {
                            movement.bipod_gesture(out, forward);
                            hands.set_bipod(out);
                        }
                    }
                }
                _ => {}
            }
        }
        // The weapon's mount figures and its bipod, for the movement's mount.
        movement.set_mount_weapon(Some(hands.settings.mount), firearm.bipod().unwrap_or(false));
        (input.mount_may, input.overlap_depth) = (firearm.may_mount(), last_overlap);
        // What the weapon in hand says to blind fire's gate.
        (input.blind_firearm, input.blind_may_start) = (true, firearm.can_set_blind_fire());
        input.left_stance_may = firearm.may_toggle_left_stance();
        // The aim key asks; the weapon's logic and the movement state
        // decide. The sprint switching on puts the sights down, and so
        // does entering a state that does not allow aiming.
        let (wants_aim, can_aim) = (scenario.held("aim", time), movement.can_aim());
        if std::mem::replace(&mut aim_key, wants_aim) != wants_aim {
            firearm.set_aim(wants_aim, can_aim);
        }
        // Whether the hands take a press of the trigger is asked of the
        // movement state the frame began in, like the aim key.
        let can_fire = movement.can_press_trigger();
        input.aiming = firearm.aiming();
        // The weapon's motion takes the aim as the key changed it, on this
        // frame, whether a physics step comes in it or not.
        hands.aim_changed(firearm.aiming());
        // A reload begun this frame is one the body's animator sees this
        // frame; one that ends in the weapon's step below, the next.
        input.reloading = firearm.reloading();
        input.hands_busy = firearm.hands_busy();
        // Whether the hands' animator is at rest, which a gesture of the
        // hands waits for (the bipod's).
        input.hands_idle = firearm.animator_idling();
        input.weapon_occupied = firearm.occupied();
        // What stands ahead is as the game's own grid measured it.
        if frame.vault.is_some() {
            movement.vault_given = frame.vault;
        }
        movement.step_on_clock(frame.dt, frame.player_dt, &input, ground);
        // What blind fire's gate took goes to the weapon and its motion.
        trp_sim::blind_fire::tell(movement, Some(&mut *firearm), Some(&mut *hands));
        // And what the mount did (`trp_sim::mounting::tell`): untrained, no
        // skill's gain.
        for event in trp_sim::mounting::tell(movement, Some(&mut *firearm), Some(&mut *hands), [0.0; 2]) {
            if let (trp_sim::mounting::MountEvent::Entered { .. }, Some(data)) = (event, movement.last_mount_data()) {
                println!("  mount data at {time:.3} s: point {:?} target {:?} body yaw {:.4} pose limits {:?} pitch limits {:?} yaw limits {:?} approach {:.5}; stance {:.6}", data.point.at, data.target, data.body_yaw, data.pose_limit, data.pitch_limit, data.yaw_limit, data.approach, movement.pose_level);
            }
            mount_events.push((time, event));
        }
        // The bipod's gesture past its mark ends the weapon's toggle.
        for acted in movement.take_acted() {
            if acted == trp_sim::Acted::BipodToggled {
                firearm.bipod_toggled();
            }
        }
        if movement.take_sprint_started() {
            firearm.sprint_started();
        }
        if movement.take_aim_lowered() {
            firearm.lower_aim();
        }
        // More than half a second off the ground puts the sights down, on
        // every frame of it.
        if movement.long_fall() {
            firearm.set_aim(false, movement.can_aim());
        }
        // A state the body's animator entered may carry a pose for the
        // first-person weapon; the others go out.
        for pose in movement.take_poses() {
            hands.assign_pose(pose.as_ref());
        }
        if show_layers == Some(index) {
            println!("  the body's animator on frame {index}:");
            for line in movement.body.describe() {
                println!("    {line}");
            }
        }

        // The bone the weapon's moved bone hangs from, as the rig has it.
        hands.weapon_root = geometry.weapon_root;
        // The springs run on physics steps: those that are due by this
        // frame.
        for _ in 0..frame.steps {
            let footstep = movement.take_footstep();
            let m = &*movement;
            hands.advance(
                DT,
                &HandsInput {
                    yaw: m.yaw,
                    pitch: m.pitch,
                    motion: m.input_velocity(),
                    velocity: m.velocity,
                    speed: m.smoothed_speed,
                    walking: m.state == State::Move,
                    sprinting: m.state == State::Sprint,
                    idle: m.state == State::Idle,
                    footstep,
                    tilt: if m.mounted() { m.weapon_tilt() } else { m.smoothed_tilt },
                    tilt_target: m.tilt,
                    pose_level: m.pose_level,
                    smoothed_pose_level: m.smoothed_pose_level,
                    aiming: firearm.aiming(),
                    prone: m.prone,
                    crawling: m.state == State::ProneMove,
                    frame_dt: frame.dt,
                    wind: m.wind(),
                },
            );
        }
        // The rig is turned to the view after the steps, every frame.
        hands.turn_rig(movement.yaw, movement.pitch);
        // The weapon's logic runs once a drawn frame, after the physics
        // steps, as the game's does: a trigger pulled in a frame fires in
        // that frame, and the springs feel the shot on their next step.
        firearm.set_trigger(scenario.held("fire", time), can_fire);
        let was_reloading = firearm.reloading();
        for event in firearm.step_on_clock(frame.dt, frame.player_dt) {
            match event {
                FirearmEvent::Shot { strength } => hands.shoot(strength),
                FirearmEvent::AutoFire(on) => hands.set_auto_fire(on),
                // The body's part in the weapon leaving and coming to the
                // hands, told whoever is looking (third-person spec).
                FirearmEvent::BodyAction(action) => movement.body.set_weapon_action(action),
                // An operation that ends blind fire has begun.
                FirearmEvent::BlindFireEnded => trp_sim::blind_fire::ended(movement, Some(&mut *hands)),
                // And one that ends a mount.
                FirearmEvent::LeaveMount { at_once } => movement.leave_mount(at_once),
                // And what the left shoulder hears of the hands.
                FirearmEvent::LeftStance(call) => movement.left_stance_call(call),
                _ => {}
            }
        }
        // With `--recorded-recoil`, the recoil as the game had it on this
        // frame, before anything is placed by it.
        if let Some([turn, push, kick]) = frame.recoil {
            hands.recoil.hand_rotation.current = turn;
            hands.recoil.hand_position.current = push;
            hands.recoil.camera_rotation.current = kick;
        }
        // A reload handing over takes stock of the weapon: its length
        // for the line that feels for obstacles is measured afresh, in the
        // pose the reload's clip has the weapon in, and with the wide
        // field of view's squeeze still on the rig from the frame before's
        // drawing (weapon-handling spec, "Turn-away"). The weapon's own
        // bone is taken as the recording has it on this frame, where it
        // has it; the bones above it, which the procedural motion and the
        // body place, are as the frame before left them, which is why this
        // comes before the animator's pose of this frame is put on the rig.
        if was_reloading && !firearm.reloading() {
            let kept = holding.rig.local(holding.weapon);
            if let Some(bone) = frame.weapon_bone {
                holding.rig.set_local(holding.weapon, bone);
            }
            if let Some(length) = trp_sim::weapon_length_squeezed(&holding.rig, &holding.ends, squeeze(hands)) {
                hands.settings.weapon_length = length;
            }
            holding.rig.set_local(holding.weapon, kept);
        }
        // The weapon's animator as this frame left it, for what the arms
        // take from its rig.
        holding.rig.pose(firearm.animator().pose(), Some(holding.moved));
        // Seen from outside, the body reaches for the vest as the left
        // hand's marker nears the weapon's marker for it, while a reload is
        // in hand and not in a sprint (`Player.AdjustUtilityLayerWeight`).
        if outside {
            let reach = holding.vest.map(|[left, vest]| {
                let (left, vest) = (holding.rig.placed(left).0, holding.rig.placed(vest).0);
                (0..3).map(|k| (left[k] - vest[k]).powi(2)).sum::<f32>().sqrt()
            });
            let reaching = firearm.reloading() && !movement.sprinting;
            movement.body.weigh_reach_for_vest(reach.is_some(), reach.filter(|_| reaching), frame.dt);
        }

        let m = &*movement;
        // The iron sights' line and the muzzle as the rig has them now.
        if let Some(sights) = &holding.sights {
            hands.note_sights(sights.line(&holding.rig), sights.in_moved(&holding.rig).and_then(|(_, muzzle)| muzzle));
        }
        // The frame's share of any first-person pose, before the weapon
        // is placed, on the player's clock.
        hands.blend_poses(frame.player_dt);
        // Where the body carries the weapon, if it does, and its right
        // hand where that has the weapon.
        let marker = frame.right_marker.unwrap_or(geometry.right_hand);
        // With the left shoulder's part (`PlayerBones.ShiftWeaponRoot`).
        let left = trp_sim::hands::LeftShoulder {
            curve: m.body.left_stance_curve(),
            sprinting: m.state == State::Sprint,
            anim_value: m.left_stance().last_anim,
            aiming: hands.is_aiming(),
            forward: m.forward_input(),
            dt: frame.player_dt,
            ribcage_local: m.body.ribcage_local(),
        };
        let shifted = hands.weapon_transform_shifted(geometry.weapon_root, m.body.weapon_root_third(m.body_in_view()), m.held(), marker, left);
        let (weapon_position, weapon_rotation) = shifted.held;
        // The camera takes the sight's place off the weapon where the
        // procedural motion put it: the game works the camera out before
        // the body takes the weapon; with the left shoulder's curve up, late
        // in the frame, from where the shoulder put it (`LateTransformations`).
        let (placed_position, placed_rotation) = if hands.left_shoulder_curve() > 0.0 { shifted.held } else { hands.weapon_transform() };
        let aim = geometry.aim_point(placed_position, placed_rotation, frame.weapon_bone);
        hands.body_camera = Some(m.body.camera_bone());
        // On a mount the rig is moved onto its point, and the mount's camera
        // follows it from the camera's own object (`MountingRotationFunction`).
        let rig_apart = match m.mount_view() {
            Some(view) => {
                let pivot = hands.settings.mount.pivot_for(view.side != trp_sim::mounting::Side::Forward, view.on_bipod);
                let (root_at, root_turn) = geometry.weapon_root;
                let turned = quat_rotate(root_turn, pivot);
                m.mounted_rig_apart(std::array::from_fn(|k| root_at[k] + turned[k]))
            }
            None => [0.0; 3],
        };
        let camera_apart = {
            let (container, ribcage) = (m.camera_container(), m.ribcage());
            let look = quat_mul(quat_axis(1, m.yaw - m.body_yaw), quat_axis(0, m.pitch));
            quat_rotate([-look[0], -look[1], -look[2], look[3]], std::array::from_fn(|i| container[i] - ribcage[i]))
        };
        hands.set_rig_shift(std::array::from_fn(|k| rig_apart[k] - camera_apart[k]));
        // The camera is moved for the character's own eyes only: seen from
        // outside it is left where it stood
        // (`ThirdPersonStrategy.ApplyCameraTransformations`), and what the
        // replay says of it is not the game's.
        let mut camera = trp_sim::CameraPose { position: [0.0; 3], rotation: [0.0, 0.0, 0.0, 1.0], field_of_view: hands.settings.field_of_view };
        let mut push = [0.0; 3];
        if outside {
            hands.frame_outside(frame.dt);
        } else {
            camera = hands.frame_on_clock(frame.dt, frame.player_dt, geometry.camera_bone, Some(aim));
            // Last of all in the game's frame, the camera's box is pushed
            // out of the world, and the camera with it.
            let shape = &geometry.camera_box;
            push = m.camera_push(ground, camera.position, camera.rotation, shape.size, shape.centre);
            camera.position = hands.push_camera(push);
        }
        let springs = hands.spring_values();
        let mut sample = Sample::new();
        let mut put = |name: &str, values: &[f32]| {
            sample.insert(name.to_owned(), values.to_vec());
        };
        put("distance", &[(m.position[0].powi(2) + m.position[2].powi(2)).sqrt()]);
        put("weapon_length_now", &[hands.settings.weapon_length]);
        put("height", &[m.position[1]]);
        put("ground_speed", &[m.ground_speed()]);
        put("speed", &[m.character_speed]);
        put("clamped_speed", &[m.clamped_speed()]);
        put("dir", &m.movement_direction);
        put("smoothed_speed", &[m.smoothed_speed]);
        put("pose", &[m.pose_level]);
        put("smoothed_pose", &[m.smoothed_pose_level]);
        put("smoothed_tilt", &[m.smoothed_tilt]);
        put("sprint_speed", &[m.sprint_speed]);
        put("hands_pos", &hands.hands_position.current);
        put("hands_rot", &hands.hands_rotation.current);
        put("cam_rot", &hands.camera_rotation.current);
        put("cam_pos", &springs[3]);
        put("sway", &hands.sway.position);
        put("step_frequency", &[hands.step_frequency]);
        put("possible_tilt", &[hands.possible_tilt]);
        put("recoil_rot", &hands.recoil.hand_rotation.current);
        put("recoil_pos", &hands.recoil.hand_position.current);
        put("recoil_cam", &hands.recoil.camera_rotation.current);
        // The weapon's moved bone as the camera sees it: through the rig,
        // which is squeezed along the view, from a camera that is not. The
        // camera hangs in its own object, which a clip may leave apart from
        // the ribcage the rig hangs from (`Movement::camera_container`).
        let apart = {
            let (container, ribcage) = (m.camera_container(), m.ribcage());
            let look = quat_mul(quat_axis(1, m.yaw - m.body_yaw), quat_axis(0, m.pitch));
            let back = [-look[0], -look[1], -look[2], look[3]];
            quat_rotate(back, std::array::from_fn(|i| container[i] - ribcage[i]))
        };
        let seen = |position: [f32; 3]| {
            let (root_position, root_rotation) = geometry.weapon_root;
            let hanging = quat_rotate(root_rotation, position);
            let mut at: [f32; 3] = std::array::from_fn(|i| root_position[i] + hanging[i]);
            at[2] *= squeeze(hands);
            // (A mount's move of the rig is not squeezed: it is the root's own place.)
            at = std::array::from_fn(|i| at[i] + rig_apart[i]);
            let back = [-camera.rotation[0], -camera.rotation[1], -camera.rotation[2], camera.rotation[3]];
            quat_rotate(back, std::array::from_fn(|i| at[i] - camera.position[i] - apart[i]))
        };
        put("weapon_pos", &weapon_position);
        put("weapon_rot", &rotation_vector(&weapon_rotation));
        if !outside {
            put("weapon_seen", &seen(weapon_position));
        }
        // The eye from the feet, in the body's axes: the ribcage, and the
        // camera's place beside it turned to where the player looks.
        let look = quat_mul(quat_axis(1, m.yaw - m.body_yaw), quat_axis(0, m.pitch));
        let beside = quat_rotate(look, camera.position);
        let ribcage = m.ribcage();
        // The camera hangs in its own object beside the ribcage
        // (`Movement::camera_container`).
        let container = m.camera_container();
        if !outside {
            put("eye", &[container[0] + beside[0], container[1] + beside[1], container[2] + beside[2]]);
        }
        put("ribcage", &ribcage);
        // The body's own bones, where a recording has them (recorder 0.3.0
        // on): its weapon bone, and its arms.
        put("ribcage_p", &ribcage);
        // The pelvis, the first spine bone and each leg down to the toe,
        // each put together from the bones above it.
        let posed = m.body.drawn_bones();
        let mut legs: Vec<([f32; 3], [f32; 4])> = Vec::with_capacity(posed.len());
        for (index, (position, rotation)) in posed.iter().enumerate() {
            let (name, _, above) = LEGS[index];
            let (at, turn) = if above == index { ([0.0; 3], [0.0, 0.0, 0.0, 1.0]) } else { legs[above] };
            let shifted = quat_rotate(turn, *position);
            legs.push((std::array::from_fn(|k| at[k] + shifted[k]), quat_mul(turn, *rotation)));
            if !name.is_empty() {
                let (at, turn) = m.in_player(legs[index]);
                put(&format!("{name}_p"), &at);
                put(&format!("{name}_q"), &turn);
            }
        }
        let (at, turn) = m.in_player(m.body.weapon_bone());
        put("weapon_3rd_p", &at);
        put("weapon_3rd_q", &turn);
        // How far the first-person weapon follows that bone, and how much
        // of each hand is still the weapon's.
        put("weapon_3rd_authority", &[m.carried().map_or(0.0, |c| c.authority)]);
        put("reach_for_vest", &[m.body.reach_for_vest()]);
        put("right_hand_first_person", &[arms::hand_weight(&m.arm_pose(true))]);
        put("left_hand_first_person", &[arms::hand_weight(&m.arm_pose(false))]);
        // The arms as the game's solver leaves them: on the weapon, or the
        // body's own by as much as its animation claims them. The weapon's
        // bone is where the body's carrying left it; the hand-over to the
        // body's palm comes after the arms are solved.
        let carried_only = shifted.carried;
        holding.rig.set_local(holding.moved, carried_only);
        // From the frame the rig hangs in to the player's axes. The
        // recorder reads as the frame is drawn, when the game has turned
        // the ribcage to the view and squeezed it along the view for a
        // wide field of view: what hangs from it is drawn in.
        let from_rig = |at: [f32; 3], squeezed: bool| {
            let scale = if squeezed { squeeze(hands) } else { 1.0 };
            let turned = quat_rotate(look, [at[0], at[1], at[2] * scale]);
            [ribcage[0] + turned[0], ribcage[1] + turned[1], ribcage[2] + turned[2]]
        };
        for (index, (right, side)) in [(true, "right"), (false, "left")].into_iter().enumerate() {
            let body = m.arm_pose(right);
            let mut on_rig = holding.rig.arm(&holding.arms[index]);
            // An obstacle that pushes the weapon back draws the elbows aside.
            let shift = hands.elbow_shift(right);
            on_rig.goal = std::array::from_fn(|k| on_rig.goal[k] + shift[k]);
            let (bones, _) = arms::hold(&body, &on_rig, &holding.limbs[index]);
            for (bone, name) in [(0, "upperarm"), (1, "forearm"), (4, "palm")] {
                let (at, turn) = bones[bone];
                put(&format!("{side}_{name}_p"), &from_rig(at, true));
                put(&format!("{side}_{name}_q"), &quat_mul(look, turn));
            }
            // The elbow's goal: the body's bone as it is left for the
            // solver. It hangs from the spine and is not squeezed.
            put(&format!("{side}_bend_goal_p"), &from_rig(arms::bend_goal(&body, &on_rig), false));
            // The solver alone: given the palm and the elbow's goal the
            // game recorded, where it turns the upper arm and the forearm.
            // What is left of a gap then is not the weapon's place. Only
            // with the hand all the weapon's: at a part weight the palm
            // the game recorded is not where its solver was reaching.
            let weight = arms::hand_weight(&body);
            if let (Some(given), true) = (frame.given[index], weight >= 1.0) {
                let bones = arms::solve(&arms::found(&body, &on_rig), given.palm, given.goal, weight, &holding.limbs[index]);
                put(&format!("{side}_upperarm_given_q"), &quat_mul(look, bones[0].1));
                put(&format!("{side}_forearm_given_q"), &quat_mul(look, bones[1].1));
                let elbow = quat_rotate(look, [bones[1].0[0], bones[1].0[1], bones[1].0[2] * squeeze(hands)]);
                put(&format!("{side}_forearm_given_p"), &std::array::from_fn::<f32, 3, _>(|k| given.ribcage[k] + elbow[k]));
            }
        }
        // The weapon's own goals for the elbows, where the weapon ends the
        // frame: in the body's hand, where the body has taken it.
        holding.rig.set_local(holding.moved, (weapon_position, weapon_rotation));
        for (index, side) in ["right", "left"].into_iter().enumerate() {
            let goal = holding.rig.placed(holding.arms[index].goal).0;
            put(&format!("weapon_{side}_bend_goal_p"), &from_rig(goal, true));
        }
        // What the weapon's clips are doing, where a recording has it: the
        // weapon's own bone in its parent, and the two hand markers in the
        // axes of the bone the procedural motion moves.
        let (weapon_at, weapon_turn) = holding.rig.local(holding.weapon);
        put("weapon_bone_p", &weapon_at);
        put("weapon_bone_q", &weapon_turn);
        let (moved_at, moved_turn) = holding.rig.placed(holding.moved);
        // And the bone the procedural motion moves, in the player's axes.
        put("weapon_root_anim_p", &from_rig(moved_at, true));
        put("weapon_root_anim_q", &quat_mul(look, moved_turn));
        let unmoved = [-moved_turn[0], -moved_turn[1], -moved_turn[2], moved_turn[3]];
        for (index, side) in ["right", "left"].into_iter().enumerate() {
            let (at, turn) = holding.rig.placed(holding.arms[index].marker);
            put(&format!("{side}_marker_p"), &quat_rotate(unmoved, std::array::from_fn(|k| at[k] - moved_at[k])));
            put(&format!("{side}_marker_q"), &quat_mul(unmoved, turn));
        }
        // Last in the frame, as in the game: how deep the weapon would
        // stick into the ground. The next physics steps turn it away by
        // that, and past a limit the sights come down until it is clear.
        let left_wanted = m.left_stance().wanted;
        let (depth, measured) = hands.measure_overlap_shoulder(left_wanted, |line| m.weapon_overlap(ground, line));
        let clear_left = !left_wanted || m.weapon_overlap(ground, hands.shoulder_line(0.2)) <= 0.0;
        hands.set_overlap(depth);
        last_overlap = depth;
        movement.left_stance_overlap(depth, measured, || clear_left, frame.dt);
        let m = &*movement;
        firearm.overlap(depth, hands.turn_away_value(), m.can_aim());
        // Sights the obstacle put down or gave back are so from here on,
        // not from the next physics step.
        hands.aim_changed_after_frame(firearm.aiming(), frame.dt);
        put("overlap_depth", &[depth]);
        // How far over the ground the far end of the weapon's line is: what
        // a depth of nothing leaves unsaid.
        let (line_from, line_to) = hands.weapon_line();
        let (rig_at, rig_turn) = m.rig_frame();
        let over_ground = |at: [f32; 3]| {
            let turned = quat_rotate(rig_turn, at);
            let [x, y, z]: [f32; 3] = std::array::from_fn(|k| rig_at[k] + turned[k]);
            y - Terrain(ground).ground_height(x, z)
        };
        if !outside {
            put("camera_push", &push);
        }
        put("line_clearance", &[over_ground(line_from), over_ground(line_to)]);
        put("turn_away", &[hands.turn_away_value()]);
        let (away_position, away_rotation) = hands.turn_away_offsets();
        put("turn_away_pos", &away_position);
        put("turn_away_rot", &away_rotation);
        put("height_lag", &[m.body_height_offset]);
        put("foot_step", &[m.body.foot_step()]);
        // How far the view is turned from the body.
        put("body_lag", &[delta_angle(m.body_yaw, m.yaw)]);
        // Where the view points, on the recording's side of a full turn,
        // and how freely the movement state lets it turn.
        let yaw = frame.look.map_or(m.yaw, |look| look[0] + delta_angle(look[0], m.yaw));
        put("look", &[yaw, m.pitch]);
        put("turn_factor", &[m.turn_factor]);
        put("turn_limit", &[m.turn_limit]);
        // Where the two animators are: the body's state and how far through
        // it, and the state each is in or on its way into.
        let (playing, next) = m.body.state_names();
        put("body_anim", &[animator_state(playing)]);
        put("body_anim_time", &[m.body.progress().0]);
        put("body_entering", &[animator_state(next.unwrap_or(playing))]);
        // The weapon's own animator, which its logic goes by: the state
        // it is in, how far through, whether it is in a transition, and
        // the state it is in or on its way into.
        let (state, time, next) = firearm.animator().state();
        put("hands_entering", &[animator_state(next.unwrap_or(state))]);
        put("hands_anim", &[animator_state(state)]);
        put("hands_anim_time", &[time]);
        put("hands_blending", &[next.is_some() as u8 as f32]);
        put("machine_entering", &[animator_state(next.unwrap_or(state))]);
        put("prone", &[m.prone as u8 as f32]);
        put("sprinting", &[m.sprinting as u8 as f32]);
        // What is left in the legs and in the arms.
        put("stamina", &[m.stamina.legs.current]);
        put("hands_stamina", &[m.stamina.arms.current]);
        // The step aside, under the names the recorder gives the
        // animator's parameters.
        put("p_Step", &[m.step as f32]);
        put("p_SidestepFloat", &[m.sidestep]);
        // The angle from the body to the view as the animator has it,
        // which starts a turn on the spot when it is more than the limit.
        put("p_DeltaRotation", &[m.noted_angles()[0]]);
        // The reload in hand, which the weapon's logic sets on the body's
        // animator.
        put("p_ProneReload", &[firearm.reloading() as u8 as f32]);
        put("p_ReloadFloat", &[firearm.hands_busy() as u8 as f32]);
        let in_hand = match (firearm.reloading(), firearm.checking()) {
            (true, _) => RELOAD_IN_HAND,
            (_, true) => LOOK_IN_HAND,
            _ => 0.0,
        };
        put("in_hand", &[in_hand]);
        // A vault or a climb, under the names the recorder gives the
        // animator's parameters, and the character's capsule as the game's
        // code sizes it (its radius is the prefab's and never changes).
        let (figures, pace, [do_vault, do_climb]) = m.vault_parameters();
        put("p_VaultingHeight", &[figures[0]]);
        put("p_VaultingLength", &[figures[1]]);
        put("p_OverObstacleHeight", &[figures[2]]);
        put("p_Vaulting_speed", &[pace]);
        put("p_DoVault", &[do_vault as u8 as f32]);
        put("p_DoClimb", &[do_climb as u8 as f32]);
        put("p_IsVaulting", &[m.is_vaulting as u8 as f32]);
        put("capsule", &m.capsule_figures());
        // The head's push as the last limit found it (`head_push`, the
        // recorder's `POM.GetOffset`).
        put("head_push", &m.head_push());
        put("grounded", &[m.grounded as u8 as f32]);
        put("pos", &m.position);
        put("movement_state", &[state_number(m.state)]);
        // Blind fire as the recorder has it (`blind`: the movement's figure
        // and the blender's value), the body's two parameters, and the
        // camera rotation spring's rest.
        let (blind_value, _) = hands.blind_fire();
        put("blind", &[m.blind_fire() as f32, blind_value]);
        let (blind_shown, blind_float) = m.blind_fire_animator();
        put("p_BlindFire", &[blind_shown]);
        put("p_BlindFireFloat", &[blind_float]);
        // Mounting as the recorder has it (`mount`: mounted, the weapon's
        // motion told it, at a side, the bipod used, a bipod at all), the
        // mount's data and figures, the lean, the body's `Tilt` and
        // `ProneBipodsOn`, the gesture shown, the rig moved onto the point.
        let (hands_mount, _) = hands.mounting();
        let side = m.mount_view().is_some_and(|view| view.side != trp_sim::mounting::Side::Forward);
        put("mount", &[m.mounted() as u8 as f32, hands_mount.is_some() as u8 as f32, side as u8 as f32, firearm.bipod().unwrap_or(false) as u8 as f32, firearm.bipod().is_some() as u8 as f32]);
        if let (Some((data, [offset, mounted_tilt, camera_tilt, timer])), Some(view)) = (m.mount_figures(), m.mount_view()) {
            put("mount_target", &data.target);
            put("mount_point", &view.point);
            put("mount_limits", &[data.pose_limit[0], data.pose_limit[1], data.pitch_limit[0], data.pitch_limit[1], data.yaw_limit[0], data.yaw_limit[1]]);
            put("mount_approach", &[data.approach]);
            put("mount_transition", &[view.moving as u8 as f32, view.progress]);
            put("mount_figures", &[offset, mounted_tilt, camera_tilt, timer]);
        }
        put("tilt", &[m.tilt]);
        put("p_Tilt", &[m.body_tilt_shown()]);
        put("p_ProneBipodsOn", &[(m.mounted() && m.prone) as u8 as f32]);
        // The left shoulder (left-shoulder spec): the animator's flags and
        // the curve its clips give.
        put("p_LeftStance", &[m.left_stance().animator as u8 as f32]);
        put("p_ForceLeftStanceInOneFrame", &[m.left_stance().force_one_frame as u8 as f32]);
        put("p_Leftstance_curve", &[m.body.left_stance_curve()]);
        // (Not under the animator's name: its parameter keeps the last
        // gesture's number, where this is the one shown now.)
        put("interaction_shown", &[m.shown_interaction().map_or(0.0, f32::from)]);
        put("rig_apart", &rig_apart);
        put("bipod_operation", &[firearm.toggling_bipod() as u8 as f32]);
        put("cam_rot_zero", &hands.camera_rotation.zero);
        if !outside {
            put("camera_pos", &camera.position);
            put("camera_rot", &rotation_vector(&camera.rotation));
            put("fov", &[camera.field_of_view]);
        }
        // The weapon animator's parameters that its logic sets, under the
        // recorder's names for them (0.6.2 on), and the fire mode by the
        // game's number.
        for name in WEAPON_PARAMETERS {
            put(&format!("hp_{name}"), &[firearm.animator().get(name)]);
        }
        put("fire_mode", &[trp_sim::firearm::mode_number(firearm.mode())]);
        put("chamber", &[firearm.chamber as u8 as f32]);
        put("aiming", &[firearm.aiming() as u8 as f32]);
        put("tactical_reload", &[firearm.tactical_reload()]);
        put("magazine", &[firearm.magazine.unwrap_or(0) as f32]);
        out.push((frame.time, sample));
    }
    for (at, event) in &mount_events {
        println!("  mount at {at:.3} s: {event:?}");
    }
    (out, leftover)
}

/// The weapon animator's parameters the replay sets beside a recording's
/// (`hp_<name>`, recorder 0.6.2 on): those the weapon's logic sets.
const WEAPON_PARAMETERS: [&str; 10] =
    ["Active", "Inventory", "Fire", "AmmoInMag", "AmmoInChamber", "CanReload", "BoltCatch", "Armed", "FireMode", "IdlePosition"];

/// The rounds the weapon's chamber takes (the game's own test of what a
/// tube may be filled with: `Chambers[0].CanAccept`), set once the
/// database is read.
static CHAMBER_TAKES: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();

/// How many rounds the weapon takes a recorded list of what is carried
/// has outside a weapon: not in a magazine, not in a chamber.
fn rounds_in_reach(list: &Json) -> u32 {
    let entries = list.as_array().map(Vec::as_slice).unwrap_or_default();
    let taken = CHAMBER_TAKES.get().map(Vec::as_slice).unwrap_or_default();
    let loose = entries.iter().filter(|entry| {
        let at = entry.get("at").and_then(Json::as_str).unwrap_or("");
        let kind = entry.get("tpl").and_then(Json::as_str).unwrap_or("");
        taken.iter().any(|id| id == kind) && at != "cartridges" && at != "patron_in_weapon"
    });
    loose.filter_map(|entry| entry.get("n").and_then(Json::as_u64)).sum::<u64>() as u32
}

/// The game's number for a fire mode by the name a recording writes.
fn fire_mode_number(name: &str) -> Option<f32> {
    Some(match name {
        "fullauto" => 0.0,
        "single" => 1.0,
        "doublet" => 2.0,
        "burst" => 3.0,
        "doubleaction" => 4.0,
        "semiauto" => 5.0,
        _ => return None,
    })
}

/// Reads a game trace. Its first line describes the run; the rest are frames.
fn read_trace(path: &Path) -> Result<(Json, Vec<(f32, Sample)>)> {
    let text = std::fs::read_to_string(path)?;
    let mut lines = text.lines().filter(|l| !l.trim().is_empty());
    let header: Json = serde_json::from_str(lines.next().context("empty trace")?)?;
    // What a recording writes only as it changes (recorder 0.6.2: the
    // weapon animator's parameters, the fire mode, what is carried) is
    // carried on from frame to frame here.
    let mut standing: Sample = Sample::new();
    // Seen from outside the share of the first-person rig is 1 whatever
    // the curve says (third-person spec, "Whose arms, whose weapon").
    let outside = header.get("point_of_view").and_then(Json::as_str) == Some("ThirdPerson");
    if let Some(list) = header.get("equipment") {
        standing.insert("rounds_in_reach".into(), vec![rounds_in_reach(list) as f32]);
    }
    let mut frames = Vec::new();
    for line in lines {
        let v: Json = serde_json::from_str(line)?;
        let Some(object) = v.as_object() else { continue };
        let mut sample = Sample::new();
        for (name, value) in object {
            // Recorder 0.12.0: the aim sway's direction, drawn at random as
            // the sights came up (`sight.aim_sway`: strength, direction, left).
            if name == "sight" {
                if let Some(direction) = value.get("aim_sway").and_then(|a| a.get(1)).and_then(Json::as_array) {
                    sample.insert("aim_sway_direction".into(), direction.iter().filter_map(|x| x.as_f64().map(|x| x as f32)).collect());
                }
                continue;
            }
            // Recorder 0.13.0: the body's parameters as they change (`bp`),
            // of which the replay sets blind fire's beside its own
            // (`BlindFire`, carried from frame to frame).
            if name == "bp" {
                if let Some(blind) = value.get("BlindFire").and_then(Json::as_f64) {
                    standing.insert("p_BlindFire".into(), vec![blind as f32]);
                }
                continue;
            }
            let numbers: Vec<f32> = match value {
                Json::Array(_) if name == "equipment" => {
                    standing.insert("rounds_in_reach".into(), vec![rounds_in_reach(value) as f32]);
                    continue;
                }
                Json::String(s) if name == "fire_mode" => match fire_mode_number(s) {
                    Some(number) => vec![number],
                    None => continue,
                },
                Json::Number(n) => vec![n.as_f64().unwrap_or(0.0) as f32],
                Json::Bool(b) => vec![*b as u8 as f32],
                Json::Array(a) => a.iter().filter_map(|x| x.as_f64().map(|x| x as f32)).collect(),
                Json::String(s) if name == "state" => {
                    sample.insert("movement_state".into(), vec![recorded_state_number(s).unwrap_or(UNKNOWN_STATE)]);
                    continue;
                }
                // Which operation the weapon's logic has in hand (recorder
                // 0.4.0 on), as far as the simulation tells them apart: a
                // reload, a look at the weapon, or neither.
                Json::String(s) if name == "operation" => {
                    let in_hand = match s.as_str() {
                        "ReloadExternalMagOperation" | "ReloadInternalMagOperation" => RELOAD_IN_HAND,
                        "UtilityOperation" => LOOK_IN_HAND,
                        _ => 0.0,
                    };
                    sample.insert("in_hand".into(), vec![in_hand]);
                    continue;
                }
                _ => continue,
            };
            sample.insert(name.clone(), numbers);
        }
        // Turn what the game reports into the measures the replay writes.
        if let Some(p) = sample.get("pos").cloned().filter(|p| p.len() == 3) {
            sample.insert("distance".into(), vec![(p[0] * p[0] + p[2] * p[2]).sqrt()]);
            sample.insert("height".into(), vec![p[1]]);
        }
        if let Some(v) = sample.get("vel").cloned().filter(|v| v.len() == 3) {
            sample.insert("ground_speed".into(), vec![(v[0] * v[0] + v[2] * v[2]).sqrt()]);
        }
        // The capsule's height and its centre's; its radius never changes.
        if let Some(capsule) = sample.get_mut("capsule").filter(|c| c.len() == 3) {
            capsule.truncate(2);
        }
        // How far the first-person weapon follows the body's weapon bone:
        // the product of two of the animator's curves, each 1 at most.
        // (The recorder's own figure for it is not in the traces.)
        if let (Some(carry), Some(share)) = (sample.get("p_Weapon_3rd"), sample.get("p_First_Person_Curve_Weight")) {
            if let (Some(carry), Some(share), false) = (carry.first(), share.first(), sample.contains_key("weapon_3rd_authority")) {
                let share = if outside { 1.0 } else { share.min(1.0) };
                sample.insert("weapon_3rd_authority".into(), vec![(carry.min(1.0) * share).clamp(0.0, 1.0)]);
            }
        }
        // The weight of the layer that reaches for the vest
        // (`UtilityOperation`, the body animator's third), which the game
        // sets for a character seen from outside.
        if let Some(weight) = sample.get("layers").and_then(|layers| layers.get(2 * 4 + 2)).copied() {
            sample.insert("reach_for_vest".into(), vec![weight]);
        }
        // The arm's bones a second time, to set beside what the solver
        // makes of the game's own palm and goal.
        for side in ["right", "left"] {
            for (bone, given) in [("upperarm_q", "upperarm_given_q"), ("forearm_q", "forearm_given_q"), ("forearm_p", "forearm_given_p")] {
                if let Some(value) = sample.get(&format!("{side}_{bone}")).cloned() {
                    sample.insert(format!("{side}_{given}"), value);
                }
            }
        }
        // The state an animator is in, or blending into.
        for animator in ["body", "hands"] {
            let entering = sample.get(&format!("{animator}_next")).or_else(|| sample.get(&format!("{animator}_anim"))).cloned();
            if let Some(entering) = entering {
                // The weapon's is set beside two things: the logic's own
                // state and the state machine's.
                if animator == "hands" {
                    sample.insert("machine_entering".to_owned(), entering.clone());
                }
                sample.insert(format!("{animator}_entering"), entering);
            }
        }
        if let (Some(look), Some(body)) = (sample.get("look").cloned(), sample.get("body_yaw").cloned()) {
            if let (Some(&look), Some(&body)) = (look.first(), body.first()) {
                sample.insert("body_lag".into(), vec![delta_angle(body, look)]);
            }
        }
        for name in ["weapon_rot", "camera_rot"] {
            if let Some(q) = sample.get(name).cloned().filter(|q| q.len() == 4) {
                sample.insert(name.into(), rotation_vector(&q));
            }
        }
        for (name, value) in &sample {
            if name.starts_with("hp_") || name == "fire_mode" {
                standing.insert(name.clone(), value.clone());
            }
        }
        for (name, value) in &standing {
            sample.entry(name.clone()).or_insert_with(|| value.clone());
        }
        let time = sample.get("t").and_then(|t| t.first().copied()).unwrap_or(0.0);
        frames.push((time, sample));
    }
    Ok((header, frames))
}

/// How fast the game's camera closed on the sight line, from a trace with
/// aiming in it: the game's aiming speed, per second.
///
/// On a frame with no physics step the camera covers `frame time * speed *
/// rate` of what is left sideways; the smallest share seen gives the speed.
fn recorded_aiming_speed(frames: &[(f32, Sample)]) -> Option<f32> {
    let value = |s: &Sample, name: &str, i: usize| s.get(name).and_then(|v| v.get(i).copied());
    let aimed_x = frames
        .iter()
        .filter(|f| value(&f.1, "aiming", 0) == Some(1.0))
        .filter_map(|f| value(&f.1, "camera_pos", 0))
        .fold(f32::MIN, f32::max);
    let mut slowest: Option<f32> = None;
    for pair in frames.windows(2) {
        let (a, b) = (&pair[0].1, &pair[1].1);
        if value(a, "aiming", 0) != Some(1.0) || value(b, "aiming", 0) != Some(1.0) {
            continue;
        }
        let (Some(x0), Some(x1), Some(dt), Some(rate)) =
            (value(a, "camera_pos", 0), value(b, "camera_pos", 0), value(b, "dt", 0), value(b, "camera_smooth", 0))
        else {
            continue;
        };
        if aimed_x - x0 < 0.02 || dt <= 0.0 || rate <= 0.0 {
            continue;
        }
        let speed = (x1 - x0) / (aimed_x - x0) / dt / rate;
        slowest = Some(slowest.map_or(speed, |s: f32| s.min(speed)));
    }
    slowest
}

/// For every value both sides have: the largest difference, when it
/// happened, and the root mean square difference. The two are frame for
/// frame when the replay followed the trace.
fn compare(sim: &[(f32, Sample)], game: &[(f32, Sample)]) -> String {
    let mut worst: BTreeMap<&str, (f32, f32, f64, usize)> = BTreeMap::new();
    let mut cursor = 0;
    for (time, frame) in game {
        while cursor + 1 < sim.len() && (sim[cursor + 1].0 - time).abs() <= (sim[cursor].0 - time).abs() {
            cursor += 1;
        }
        for (name, values) in &sim[cursor].1 {
            let Some(theirs) = frame.get(name) else { continue };
            if theirs.len() != values.len() {
                continue;
            }
            let mut difference = values.iter().zip(theirs).map(|(a, b)| (a - b).abs()).fold(0.0f32, f32::max);
            // Two turnings are apart by the angle from one to the other,
            // in degrees.
            if name.ends_with("_q") && values.len() == 4 {
                let along: f32 = values.iter().zip(theirs).map(|(a, b)| a * b).sum();
                difference = 2.0 * along.abs().min(1.0).acos().to_degrees();
            }
            // A state is a name, not a size: the same or not.
            if name.ends_with("_anim") || name.ends_with("_entering") || *name == "movement_state" {
                difference = difference.min(1.0).ceil();
            }
            let entry = worst.entry(name.as_str()).or_insert((0.0, 0.0, 0.0, 0));
            if difference > entry.0 {
                (entry.0, entry.1) = (difference, *time);
            }
            entry.2 += (difference as f64).powi(2);
            entry.3 += 1;
        }
    }
    let mut out = String::new();
    let _ = writeln!(out, "  {:<16} {:>12} {:>8} {:>12}", "value", "largest gap", "at (s)", "rms gap");
    for (name, (largest, at, squares, count)) in worst {
        let _ = writeln!(out, "  {name:<16} {largest:>12.4} {at:>8.2} {:>12.4}", (squares / count.max(1) as f64).sqrt());
    }
    out
}

fn write_trace(path: &Path, samples: &[(f32, Sample)]) -> Result<()> {
    let mut text = String::new();
    for (time, sample) in samples {
        let mut object = serde_json::Map::new();
        object.insert("t".into(), Json::from(*time));
        for (name, values) in sample {
            let value = match values.as_slice() {
                [one] => Json::from(*one),
                many => Json::from(many.to_vec()),
            };
            object.insert(name.clone(), value);
        }
        text.push_str(&Json::Object(object).to_string());
        text.push('\n');
    }
    std::fs::write(path, text).with_context(|| path.display().to_string())
}

fn main() -> Result<()> {
    let mut scenarios_dir = PathBuf::from("hosts/spt-oracle/scenarios");
    let mut traces_dir: Option<PathBuf> = None;
    let mut out_dir = PathBuf::from("local/oracle/sim");
    let (mut weapon_id, mut weight) = (String::new(), 0.0f32);
    let mut wanted: Vec<String> = Vec::new();
    let mut show_layers: Option<usize> = None;
    let mut own_view = false;
    let mut line: Option<f32> = None;
    let mut seed: Option<u64> = None;
    let mut endurance: Option<f32> = None;
    let mut recorded = false;
    let mut recorded_recoil = false;
    let mut aim_sway: Option<[f32; 3]> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--scenarios" => scenarios_dir = args.next().map(PathBuf::from).context("--scenarios needs a folder")?,
            "--traces" => traces_dir = args.next().map(PathBuf::from),
            "--out" => out_dir = args.next().map(PathBuf::from).context("--out needs a folder")?,
            "--weapon" => weapon_id = args.next().context("--weapon needs an item id")?,
            "--weight" => weight = args.next().and_then(|v| v.parse().ok()).context("--weight needs kilograms")?,
            "--layers" => show_layers = Some(args.next().and_then(|v| v.parse().ok()).context("--layers needs a frame")?),
            // Turn the view from what the recorder's script asked rather
            // than follow where the game looked (recorder 0.4.0 on).
            "--own-view" => own_view = true,
            // The length of the line that feels for obstacles, where it is
            // not to be the weapon's at rest: the game measures it as the
            // weapon is drawn, and a recording from 0.4.3 on says what it
            // came to (`weapon_length` in the header, which goes first).
            "--line" => line = Some(args.next().and_then(|v| v.parse().ok()).context("--line needs metres")?),
            // The seed of the recoil's and the breath's random numbers:
            // the game's own draws are not known, so a shot compares with
            // the spread of many runs, not with any one.
            "--seed" => seed = Some(args.next().and_then(|v| v.parse().ok()).context("--seed needs a number")?),
            "--endurance" => endurance = Some(args.next().and_then(|v| v.parse().ok()).context("--endurance needs a level")?),
            // The weapon built as the recording's equipment has it, its
            // parts as they were (a user's own build), not its stock one.
            "--recorded-build" => recorded = true,
            // The recoil's processes taken from the recording each frame
            // (`recoil_rot`, `recoil_pos`, `recoil_cam`), so that the
            // camera's follow and the weapon's pose under fire compare
            // with the game's apart from its random draws.
            "--recorded-recoil" => recorded_recoil = true,
            // A heavy weapon's aim sway in this direction each time the
            // sights come up (`_aimSwayDirection`, as the game draws it at
            // random): a recording before recorder 0.12.0 does not hold it.
            "--aim-sway" => {
                let parts: Vec<f32> = args.next().context("--aim-sway needs x,y,z")?.split(',').filter_map(|v| v.parse().ok()).collect();
                aim_sway = Some(parts.try_into().map_err(|_| anyhow::anyhow!("--aim-sway needs three numbers"))?);
            }
            name => wanted.push(name.to_owned()),
        }
    }

    // The weapon is the one asked for, else the one the recording was made
    // with (its traces' first line names it), else the first one built.
    if weapon_id.is_empty() {
        weapon_id = recorded_weapon(traces_dir.as_deref()).unwrap_or_else(|| AKM.to_owned());
    }
    let install = Install::locate()?;
    let db = Database::load(&install)?;
    let hard = HardSettings::load(&install)?;
    let engine = EngineSettings::load(&install)?;
    let mut assets = Assets::new(install)?;
    let build = if recorded { Some(recorded_build(traces_dir.as_deref(), &weapon_id).context("--recorded-build: the first trace's equipment has no such weapon in a weapon slot")?) } else { None };
    let weapon = match &build {
        Some(preset) => {
            let weapon = trp_data::weapon::assemble_preset(&mut assets, &db, preset)?;
            println!("the weapon as recorded: {} parts", preset.items.len());
            if let Some(bipod) = &weapon.bipod {
                println!("  its bipod: {} (folds {})", bipod.template, if bipod.forward { "forward" } else { "backward" });
            }
            for skipped in &weapon.skipped {
                println!("  part not placed: {skipped}");
            }
            weapon
        }
        None => trp_data::weapon::assemble(&mut assets, &db, &weapon_id)?,
    };
    println!("the weapon: {} ({weapon_id})", weapon.name);
    // Every round its chamber takes, by the chamber's own filter.
    let chamber = trp_sim::inventory::Kind::of(&db, &weapon_id)?.slots.into_iter().find(|slot| slot.name == "patron_in_weapon");
    let takes: Vec<String> = chamber.map_or_else(Vec::new, |slot| db.items.keys().filter(|id| db.classes(id).iter().any(|class| slot.takes.contains(class))).cloned().collect());
    let _ = CHAMBER_TAKES.set(takes);
    let camera_box = trp_data::player::CameraBoxData::load(&mut assets)?;
    let geometry = Geometry::of(&weapon, trp_sim::right_hand_marker(&assets, &weapon)?, camera_box)?;
    let (slope, stood_over) = recorded_slope(traces_dir.as_deref());
    let mount_note = recorded_mount(traces_dir.as_deref());
    let profiled = recorded_profile(traces_dir.as_deref());
    // The character's capsule, for a ground with something standing on it.
    let capsule_radius = trp_data::player::CapsuleData::load(&mut assets)?.radius;
    let line = line.or_else(|| recorded_line(traces_dir.as_deref()));
    let hands_data = HandsData::load(&mut assets)?;
    let started = std::time::Instant::now();
    // The lower body's bones are posed too, to set beside a recording
    // that has them (recorder 0.4.4 on).
    let body_data = BodyData::load_with(&mut assets, &leg_paths())?;
    println!("the body's animator: {} clips read in {:.1} s", body_data.clip_count(), started.elapsed().as_secs_f32());
    // The arms' solver as the player prefab sets it up.
    let limbs = LimbData::load(&mut assets)?;
    for limb in &limbs {
        arms::check(limb)?;
    }
    let firearm_settings = trp_sim::firearm_settings(&assets, &db, &hard, &weapon)?;
    // The weapon's own controller, to run beside the logic.
    let weapon_animator = trp_sim::weapon_animator::WeaponAnimatorData::load(&assets, &weapon)?;
    println!("the weapon's animator: {} clips read in {:.1} s", weapon_animator.clip_count(), started.elapsed().as_secs_f32());
    let untrained = Hands::new(trp_sim::hands_settings(
        hands_data.clone(),
        &db,
        &hard,
        &engine,
        &weapon,
        ViewSettings::default(),
    )?);
    // What the game's own way of measuring makes of the weapon at rest,
    // at this install's widest squeeze and with none.
    let at_rest = |fov: f32| {
        let squeeze = trp_sim::math::lerp(1.0, 0.65, trp_sim::math::inverse_lerp(50.0, 75.0, fov));
        let rig = WeaponRig::new(&weapon.hierarchy);
        trp_sim::weapon_line_ends(&weapon).ok().and_then(|ends| trp_sim::weapon_length_squeezed(&rig, &ends, squeeze))
    };
    println!(
        "measured the game's way at rest: {:.4} m at a field of view of 50, {:.4} m at 74",
        at_rest(50.0).unwrap_or(f32::NAN),
        at_rest(74.0).unwrap_or(f32::NAN)
    );
    println!(
        "the line that feels for obstacles is {:.3} m long; the sights come down {:.2} m into one",
        untrained.settings.weapon_length, firearm_settings.stop_aiming_at
    );
    println!(
        "{}: {:.3} kg with its default rounds, ergonomics {}, carried weight {weight} kg; sway spring {} rad/s, damping {} (the prefab says {} and {})",
        weapon.name,
        weapon.total_weight,
        weapon.total_ergonomics,
        untrained.settings.sway_frequency,
        untrained.settings.sway_damping,
        hands_data.sway_frequency,
        hands_data.sway_damping
    );

    // The character's aiming speed shows in any trace that aims; one
    // figure serves every scenario of the same recording.
    let recorded_speed = traces_dir
        .as_ref()
        .and_then(|d| std::fs::read_dir(d).ok())
        .into_iter()
        .flatten()
        .filter_map(|e| read_trace(&e.ok()?.path()).ok())
        .find_map(|(header, frames)| {
            header.get("aiming_speed").and_then(Json::as_f64).map(|v| v as f32).or_else(|| recorded_aiming_speed(&frames))
        });
    if let Some(speed) = recorded_speed {
        println!(
            "the recorded character brings the sights up in {:.2} s; an untrained one would take {:.2} s with the default rounds loaded",
            1.0 / speed,
            1.0 / untrained.aiming_speed
        );
    }

    // Carried weight shows in a recording as inertia, and it changes within
    // one: a magazine dropped in a reload is weight gone. Unless a weight
    // was given, each scenario gets the one that produces its own figure.
    let mut weights: Vec<(f32, f32)> = Vec::new();
    let mut carried_seen: Vec<f32> = Vec::new();

    // The legs' capacity says how far the character's Endurance has come,
    // and one figure serves a whole recording. The refill says how well fed
    // the character was, which changes through one: each scenario reads
    // its own, or keeps the last one read.
    let legs_capacity = recorded_legs_capacity(traces_dir.as_deref());
    let mut energy_factor: Option<f32> = None;
    let mut thirst_factor: Option<f32> = None;

    let mut files: Vec<PathBuf> = std::fs::read_dir(&scenarios_dir)
        .with_context(|| scenarios_dir.display().to_string())?
        .filter_map(|e| Some(e.ok()?.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .collect();
    files.sort();
    std::fs::create_dir_all(&out_dir)?;
    let (mut compared, mut missing) = (0, 0);
    for file in files {
        // A recording made before a script was rewritten keeps the script
        // it was made with, in a `scenarios` folder of its own.
        let kept = traces_dir.as_ref().zip(file.file_name()).map(|(traces, name)| traces.join("scenarios").join(name));
        let scenario = Scenario::load(kept.as_deref().filter(|kept| kept.exists()).unwrap_or(&file))?;
        if !wanted.is_empty() && !wanted.iter().any(|w| scenario.name.contains(w.as_str())) {
            continue;
        }
        // Every scenario starts from a fresh, standing player. Where a game
        // trace exists, what it says about the player is taken over so that
        // the two start alike: the top walking speed (which the Strength
        // skill sets), the view settings, the weapon's loaded weight, the
        // round's recoil, the aiming speed and the rounds loaded.
        let trace = traces_dir.as_ref().map(|d| d.join(format!("{}.jsonl", scenario.name))).filter(|t| t.exists());
        let recorded = trace.as_deref().map(read_trace).transpose()?;
        let header = |name: &str| recorded.as_ref().and_then(|r| r.0.get(name));
        let header_number = |name: &str| header(name).and_then(Json::as_f64).map(|v| v as f32);
        let first = |name: &str| -> Option<f32> { recorded.as_ref()?.1.first()?.1.get(name)?.first().copied() };

        // What the character carried, where the recording says (recorder
        // 0.5.0 on): the game's own sum beside the model's, and every
        // item's cells and weight. Said once for each weight.
        if let Some(list) = header("equipment") {
            let game = header_number("inventory_weight").unwrap_or(0.0);
            if !carried_seen.iter().any(|seen: &f32| (seen - game).abs() < 1e-4) {
                carried_seen.push(game);
                match trp_sim::inventory::Equipment::of_recording(&db, list) {
                    Ok((equipment, compared, left_out)) => {
                        let off: Vec<&trp_sim::inventory::Compared> = compared.iter().filter(|item| !item.agrees()).collect();
                        println!(
                            "the recorded equipment: {} items; carried {game:.3} kg in the game, {:.3} here ({:.3} and {:.3} at elite Strength); {} items differ in cells or weight, {} found no place",
                            compared.len(),
                            equipment.carried_weight(false),
                            header_number("inventory_weight_elite").unwrap_or(0.0),
                            equipment.carried_weight(true),
                            off.len(),
                            left_out.len()
                        );
                        for item in off.iter().take(12) {
                            println!(
                                "  {}: {} x {} cells and {:.3} kg in the game, {} x {} and {:.3} here",
                                item.name, item.cells[0][0], item.cells[0][1], item.weight[0], item.cells[1][0], item.cells[1][1], item.weight[1]
                            );
                        }
                        for entry in left_out.iter().take(6) {
                            println!("  no place for {entry}");
                        }
                    }
                    Err(e) => println!("the recorded equipment could not be put together: {e:#}"),
                }
            }
        }

        // (The replay still goes by the weight that gives the recorded
        // inertia: that one holds for the character's Strength too.)
        let mut recorded_weight = None;
        if let (Some(inertia), true) = (header_number("inertia"), weight == 0.0) {
            let known = weights.iter().find(|(known, _)| *known == inertia).map(|(_, weight)| *weight);
            recorded_weight = Some(match known {
                Some(weight) => weight,
                None => {
                    let (mut light, mut heavy) = (0.0f32, 120.0f32);
                    for _ in 0..24 {
                        let middle = (light + heavy) / 2.0;
                        if trp_sim::load_movement_settings(&mut assets, &db, &hard, &engine, middle)?.inertia < inertia {
                            light = middle;
                        } else {
                            heavy = middle;
                        }
                    }
                    println!(
                        "a recorded inertia of {inertia:.4} is that of {heavy:.2} kg carried, for an untrained character"
                    );
                    weights.push((inertia, heavy));
                    heavy
                }
            });
        }
        let mut movement_settings =
            trp_sim::load_movement_settings(&mut assets, &db, &hard, &engine, recorded_weight.unwrap_or(weight))?;
        if let Some(max_speed) = header_number("max_speed") {
            let level = movement_settings.strength_for(max_speed);
            movement_settings.set_strength(level);
        }
        let defaults = ViewSettings::default();
        let view = ViewSettings {
            field_of_view: header_number("fov").unwrap_or(defaults.field_of_view),
            head_bobbing: header_number("far_aim_plane").unwrap_or(defaults.head_bobbing),
            seed: seed.unwrap_or(defaults.seed),
            ..defaults
        };
        let mut hands_settings = trp_sim::hands_settings(hands_data.clone(), &db, &hard, &engine, &weapon, view)?;
        if hands_settings.mount.mountable {
            println!("  its mount: pivot {:?}, the bipod's {:?}", hands_settings.mount.pivot, hands_settings.mount.bipod_pivot);
        }
        if let Some(loaded) = header_number("weapon_weight") {
            hands_settings.weapon_weight = loaded;
        }
        // The weapon's motion goes by the weight the weapon had when the
        // game last took stock of it (drawn, or at a reload's end), not by
        // what the rounds fired since have left: the recorder's ergonomic
        // weight is that one, and gives the weight back.
        let stock = 1.0 - hands_settings.weapon_ergonomics.max(0.0).sqrt() / 25.0;
        if let Some(weight) = header_number("ergonomic_weight").filter(|_| stock > 0.0) {
            hands_settings.weapon_weight = weight / stock;
        }
        if let Some(length) = header_number("weapon_length").filter(|length| *length > 0.0).or(line) {
            hands_settings.weapon_length = length;
        }
        // The bonus is whatever makes the weapon as loaded come up as fast
        // as the recording shows.
        if let Some(speed) = recorded_speed {
            let base = Hands::new(trp_sim::hands_settings(hands_data.clone(), &db, &hard, &engine, &weapon, view).map(
                |mut s| {
                    s.weapon_weight = hands_settings.weapon_weight;
                    s
                },
            )?)
            .aiming_speed;
            hands_settings.aim_speed_bonus = speed / base - 1.0;
        }
        let mut firearm_settings = firearm_settings.clone();
        // A weapon with no chamber records no round (the recorder reads
        // the first chamber's): its fitted magazine's top round, as the
        // recorded equipment has it (the last stack's).
        let round = header("ammo")
            .and_then(Json::as_str)
            .or_else(|| {
                let equipment = header("equipment")?.as_array()?;
                let under = |item: &Json, slot: &str, parent: &str| item["at"].as_str() == Some(slot) && item["in"].as_str() == Some(parent);
                let gun = equipment.iter().find(|item| item["tpl"].as_str() == Some(weapon_id.as_str()))?["id"].as_str()?;
                let magazine = equipment.iter().find(|item| under(item, "mod_magazine", gun))?["id"].as_str()?;
                equipment.iter().filter(|item| under(item, "cartridges", magazine)).last()?["tpl"].as_str()
            })
            .and_then(|id| db.items.get(id));
        if let Some(round) = round {
            firearm_settings.shot_strength = 1.0 + round.prop_f32("ammoRec").unwrap_or(0.0) / 100.0;
        }
        // How well the recorded character knows the weapon, where the
        // recording says: from level 2 a reload keeps the sights up.
        if let Some(level) = header_number("mastering") {
            firearm_settings.mastering = level as u32;
        }
        let mut firearm = Firearm::new(firearm_settings, &weapon_animator);
        if let (Some(magazine), Some(chamber)) = (first("magazine"), first("chamber")) {
            firearm.magazine = Some(magazine as u32);
            firearm.chamber = chamber > 0.0;
        }
        // The magazines' clips (`magAnimationIndex`) as the recorded
        // equipment has them: the weapon's fitted one's, and the one a
        // reload takes (`FindSuitableMagazine`: the fullest within reach),
        // with its rounds.
        let mut next_rounds = None;
        let mut no_magazine = false;
        if let Some(list) = header("equipment") {
            if let Ok((equipment, ..)) = trp_sim::inventory::Equipment::of_recording(&db, list) {
                let gun = ["FirstPrimaryWeapon", "SecondPrimaryWeapon", "Holster"].into_iter().filter_map(|slot| equipment.item.in_slot(slot)).find(|gun| gun.kind.template == weapon_id);
                if let Some(gun) = gun {
                    if let Some(fitted) = gun.in_slot(trp_sim::inventory::MAGAZINE_SLOT) {
                        firearm.set_magazine_variant(fitted.kind.magazine_animation);
                    }
                    if let Some(next) = trp_sim::inventory::magazine_for(&equipment.item, gun).and_then(|id| equipment.item.find(id)) {
                        firearm.next_magazine_variant(next.kind.magazine_animation);
                        next_rounds = Some(trp_sim::inventory::magazine_rounds(next));
                    } else {
                        no_magazine = true;
                    }
                }
            }
        }
        // The fire mode the last scenario left it in (recorder 0.6.2 on).
        let mode = first("fire_mode").and_then(|number| {
            [FireMode::Single, FireMode::FullAuto, FireMode::Burst, FireMode::SemiAuto].into_iter().find(|mode| trp_sim::firearm::mode_number(*mode) == number)
        });
        if let Some(mode) = mode {
            firearm.start_in_mode(mode);
        }
        // Its animator starts where the recording's stood, if that was at
        // rest.
        firearm.start_drawn();
        // And with the hammer and the bolt as the last scenario left them.
        firearm.start_with(first("hp_Armed").map(|armed| armed != 0.0), first("hp_BoltCatch").map(|caught| caught != 0.0));
        if let (Some(state), Some(time)) = (first("hands_anim"), first("hands_anim_time")) {
            if state == animator_state(WEAPON_AT_REST) {
                firearm.rest_at(time, first("dt").unwrap_or(0.0));
            }
        }
        let mut movement = Movement::new(movement_settings, Body::new(&body_data, weapon.cell_width as f32));
        // The armour and backpack the recording's kit had slow the
        // character as the game's did (`Player.UpdatePenalties`; movement
        // spec, "Speed limits"), by the recorded vest skills.
        if let Some(equipment) = header("equipment").and_then(Json::as_array) {
            let level = |name: &str| -> u32 {
                header("skills").and_then(Json::as_array).and_then(|all| {
                    all.iter().find(|s| s.get(0).and_then(Json::as_str) == Some(name)).and_then(|s| s.get(1)).and_then(Json::as_f64)
                }).unwrap_or(0.0) as u32
            };
            let penalty = trp_sim::armor::speed_penalty(&db, &recorded_kit(equipment), &trp_sim::armor::VestSkills::of_levels(level));
            movement.set_armour_penalty(penalty);
        }
        // The recording runs its scenarios back to back, so each starts
        // with the wheel and stance the last one left.
        if let (Some(speed), Some(smoothed)) = (first("speed"), first("smoothed_speed")) {
            movement.select_speed(speed);
            movement.smoothed_speed = smoothed;
        }
        if let (Some(pose), Some(smoothed)) = (first("pose"), first("smoothed_pose")) {
            movement.pose_level = pose;
            movement.pose_memo = pose;
            movement.smoothed_pose_level = smoothed;
        }
        // The sprint's pace too is where the last sprint left it: 1 for a
        // character that has not sprinted yet, the starting figure after.
        if let Some(pace) = first("sprint_speed") {
            movement.sprint_speed = pace;
        }
        // The animator still holds what the last vault or climb gave it.
        if let (Some(height), Some(length), Some(behind), Some(pace)) =
            (first("p_VaultingHeight"), first("p_VaultingLength"), first("p_OverObstacleHeight"), first("p_Vaulting_speed"))
        {
            let speed = first("p_AbsoluteForwardVelocity").unwrap_or(0.0);
            movement.set_vault_parameters([height, length, behind, speed], pace);
        }
        // Nor does the body always face where the player looks: standing,
        // it stays where the last turn left it, within its limit.
        if let Some(body) = first("body_yaw") {
            movement.body_yaw = body;
        }
        // And it has been standing for some time: its idle is wherever the
        // recording's first two frames put it.
        if let Some((_, frames)) = recorded.as_ref().filter(|(_, frames)| frames.len() > 1) {
            let at = |i: usize, name: &str| frames[i].1.get(name).and_then(|v| v.first().copied());
            let same = at(0, "body_anim") == Some(animator_state(movement.body.state_names().0))
                && at(1, "body_anim") == at(0, "body_anim")
                && at(0, "body_blending") == Some(0.0);
            if let (true, Some(now), Some(next)) = (same, at(0, "body_anim_time"), at(1, "body_anim_time")) {
                movement.body.set_progress(now - (next - now));
            }
            // And every other layer where the recording has it (recorder
            // 0.3.0 on): a breath, a lean into a turn, are somewhere in
            // their play as a scenario begins.
            if let Some(layers) = frames[0].1.get("layers") {
                movement.body.start_layers_as(layers, frames[1].1.get("layers").map(Vec::as_slice));
            }
        }
        // Stamina, as the recording's character has it and where the
        // recording has it.
        {
            let stamina = &mut movement.stamina;
            stamina.aim_mass = header_number("ergonomic_weight").unwrap_or(hands_settings.ergonomic_weight());
            // The skills as the recorder gives them (0.4.7 on), or as the
            // legs' capacity and the top walking speed show them.
            let level = endurance.or(first("endurance")).or(legs_capacity.map(|most| stamina.settings.endurance_for(most))).unwrap_or(0.0);
            let strength = first("strength").or(header_number("max_speed").map(|top| movement.settings.strength_for(top))).unwrap_or(0.0);
            stamina.settings.set_skills(level, strength);
            stamina.resettle();
            let capacities = [stamina.legs.capacity, stamina.arms.capacity];
            let mass = stamina.aim_mass;
            let start = recorded.as_ref().and_then(|(_, frames)| StaminaStart::of(frames, &stamina.settings, capacities, mass, thirst_factor));
            if let Some(start) = start {
                thirst_factor = start.thirst_factor.or(thirst_factor);
                energy_factor = start.energy_factor.or(energy_factor);
                if let Some(factor) = thirst_factor {
                    stamina.settings.set_hydration_for(factor);
                }
                if let Some(factor) = energy_factor {
                    stamina.settings.set_energy_for(factor);
                }
                stamina.resettle();
                (stamina.legs.current, stamina.arms.current) = (start.pools[0], start.pools[1]);
                for (pool, from) in [&mut stamina.legs, &mut stamina.arms].into_iter().zip(start.refill_from) {
                    pool.hold_refill_until(from.unwrap_or(0.0));
                }
                stamina.pose_rise = start.pose_rise.unwrap_or(0.0);
            }
        }
        let mut hands = Hands::new(hands_settings);
        hands.weapon_flip_speed = movement.settings.weapon_flip_speed;
        hands.aim_sway_override = aim_sway;
        // The bob keeps the tempo of the last footstep, which in a
        // recording was taken in the scenario before.
        if let Some(tempo) = first("step_frequency") {
            hands.step_frequency = tempo;
        }
        // And the character has been breathing all along: the breath is
        // wherever the weapon's rise and fall over the scenario's quiet
        // start puts it (up to half a second, before the script's first
        // key), one figure for each physics step.
        if let Some((_, frames)) = recorded.as_ref().filter(|_| first("aiming") == Some(0.0)) {
            let quiet = scenario.quiet().clamp(0.1, 0.5);
            let lift = |at: f32| {
                let (_, sample) = frames.iter().find(|(time, _)| *time >= at)?;
                sample.get("hands_pos")?.get(2).copied()
            };
            let lifted: Vec<f32> = (0..).map(|k| k as f32 * DT).take_while(|at| *at < quiet).map_while(lift).collect();
            let pose = match (first("prone"), first("pose")) {
                (Some(lying), _) if lying > 0.0 => 0,
                (_, Some(level)) if level < 0.11 => 1,
                _ => 2,
            };
            hands.start_breath(pose, &lifted);
        }
        // How far a lean reaches the weapon is only worked out while one is
        // asked for, so a scenario begins with what the last lean left.
        if let Some(reach) = first("possible_tilt") {
            hands.possible_tilt = reach;
        }
        // The walk's lock on at the start, where the folder says so.
        let mut movement = movement;
        if mount_note.1 {
            movement.set_blocked([0.0, 1.0]);
        }
        let mut rig = Rig { movement, hands, firearm, next_rounds, no_magazine };
        // A recording made from outside (recorder 0.7.0 on) is set beside
        // the character as others see it (third-person spec).
        if header("point_of_view").and_then(Json::as_str) == Some("ThirdPerson") {
            println!("  seen from outside: the character as others see it");
            rig.movement.body.set_point_of_view(PointOfView::ThirdPerson);
            rig.hands.set_point_of_view(PointOfView::ThirdPerson);
        }
        let squeeze = squeeze(&rig.hands);

        // The frames to play: the recording's own, or sixty a second.
        if recorded.is_none() && !scenario.gates.is_empty() {
            println!("  (the script has gates, which a replay follows from a recording: without one it is played straight through)");
        }
        let obstacles = recorded.as_ref().map(|(_, frames)| recorded_obstacles(frames)).unwrap_or_default();
        let mut frames: Vec<Frame> = match &recorded {
            Some((_, frames)) => frames
                .iter()
                .zip(player_clock(frames))
                .enumerate()
                .map(|(index, ((time, sample), clock))| {
                    let dt = sample.get("dt").and_then(|d| d.first().copied()).unwrap_or(DT);
                    let number = |name: &str| sample.get(name).and_then(|v| v.first().copied());
                    Frame {
                        time: *time,
                        script: number("script_t").unwrap_or(*time),
                        gate_press: number("gate_press").filter(|pressed| *pressed > 0.0).and(number("gate")).map(|gate| gate as usize),
                        dt,
                        steps: 0,
                        // Standing still the player's clock does not show;
                        // the engine's time stands in for it.
                        player_dt: clock.unwrap_or(dt),
                        look: sample.get("look").filter(|l| l.len() == 2).map(|l| [l[0], l[1]]),
                        asked: match (sample.get("turn_asked"), sample.get("rotation_multiplier")) {
                            (Some(turn), Some(by)) if turn.len() == 2 && by.len() == 1 => Some([turn[0] * by[0], turn[1] * by[0]]),
                            _ => None,
                        },
                        fitted: fitted_magazine(frames, index),
                        aim_sway: sample.get("aim_sway_direction").and_then(|d| <[f32; 3]>::try_from(d.as_slice()).ok()),
                        reach: number("rounds_in_reach").map(|rounds| rounds as u32),
                        weapon_bone: recorded_bone(sample, "weapon_bone"),
                        right_marker: recorded_bone(sample, "right_marker"),
                        given: [Given::of(sample, "right", squeeze), Given::of(sample, "left", squeeze)],
                        floor_key: None,
                        condition: sample.get("energy").zip(sample.get("hydration")).and_then(|(e, h)| Some([*e.first()?, *h.first()?])),
                        vault: obstacles[index],
                        recoil: ["recoil_rot", "recoil_pos", "recoil_cam"]
                            .map(|name| sample.get(name).and_then(|v| <[f32; 3]>::try_from(v.as_slice()).ok()))
                            .into_iter()
                            .collect::<Option<Vec<_>>>()
                            .filter(|_| recorded_recoil)
                            .map(|v| [v[0], v[1], v[2]]),
                    }
                })
                .collect(),
            None => (1..=(scenario.duration / DT).ceil() as usize)
                .map(|i| Frame {
                    time: i as f32 * DT,
                    script: i as f32 * DT,
                    gate_press: None,
                    dt: DT,
                    steps: 0,
                    player_dt: DT,
                    look: None,
                    asked: None,
                    fitted: None,
                    aim_sway: None,
                    reach: None,
                    weapon_bone: None,
                    right_marker: None,
                    given: [None; 2],
                    floor_key: None,
                    condition: None,
                    vault: None,
                    recoil: None,
                })
                .collect(),
        };
        // The springs run on physics steps: those that are due by each
        // frame, with the physics clock as far into a step at the start
        // as the recording shows it was.
        match &recorded {
            Some((_, recorded)) => {
                for (frame, steps) in frames.iter_mut().zip(physics_steps(recorded)) {
                    frame.steps = steps;
                }
            }
            None => {
                let mut due = 0u32;
                for frame in &mut frames {
                    let before = due;
                    while (due + 1) as f32 * DT <= frame.time + 1e-4 {
                        due += 1;
                    }
                    frame.steps = due - before;
                }
            }
        }
        let first_walk = recorded.as_ref().and_then(|(_, frames)| FirstWalk::of(frames));
        // The floor under each recorded walk, as whichever character's
        // weight changed last in the raid had left it.
        if let Some((_, recorded)) = &recorded {
            let keys = floor_keys(recorded, &rig.movement.settings);
            if !keys.is_empty() {
                let shown: Vec<String> = keys.iter().map(|(_, inertia)| format!("{inertia:.3}")).collect();
                println!(
                    "  the walk's floor as a character of inertia {} left its curve (this one's inertia is {:.3})",
                    shown.join(", "),
                    rig.movement.settings.inertia
                );
            }
            for (index, inertia) in keys {
                frames[index].floor_key = Some(inertia);
            }
        }
        // The ground is level, and where the recording's jump found it. The
        // character counted as standing on it at the start, so whichever
        // state set the ground check last had set it at least that wide.
        // Or the ground is what the recording's own path shows of it, for
        // a recording that says so: the character then starts as far over
        // it as it stood.
        let profile = recorded.as_ref().filter(|_| profiled).and_then(|(_, frames)| Profile::of(frames, capsule_radius));
        let found = match &profile {
            Some(profile) => Some(profile.height(0.0)),
            None => recorded.as_ref().and_then(|(_, frames)| recorded_ground(frames, slope)),
        };
        let ground = found.unwrap_or(0.0).min(0.0);
        rig.movement.ground_check = [GROUND_CHECK_JUMP, GROUND_CHECK, GROUND_CHECK_MOVING, GROUND_CHECK_SPRINT]
            .into_iter()
            .find(|within| -ground <= *within)
            .unwrap_or(GROUND_CHECK_SPRINT);
        // Without a jump of its own to find the ground by, the terrain is
        // as far under the character as it was on the spot every scenario
        // starts from, if the recording's folder says how far that is. A
        // character put back there with a ground check too short to reach
        // the ground has fallen onto it before the scenario began.
        let reach = recorded
            .as_ref()
            .and_then(|(_, frames)| frames.first()?.1.get("ground_check")?.first().copied())
            .unwrap_or(0.0);
        // From recorder 0.4.3 on the recording says itself how far under
        // the feet the ground is.
        let below = match first("ground_below") {
            Some(measured) if found.is_none() => measured.max(0.0),
            Some(_) => 0.0,
            None if found.is_none() && reach >= stood_over => stood_over,
            None => 0.0,
        };
        // A sill set down from the recording's mount, from the terrain up.
        let sill = mount_note.0.map(|([x, z], yaw, top, [width, depth])| {
            let (s, c) = yaw.to_radians().sin_cos();
            let floor = ground - below;
            let foot = [x + s * depth / 2.0, floor, z + c * depth / 2.0];
            trp_sim::collision::Block::turned(foot, [width, ground + top - floor, depth], [c, 0.0, -s])
        });
        let ground = Ground { height: ground, slope, below, profile, sill, floor: mount_note.2 };
        let mut holding = Holding::new(&weapon, limbs)?;
        let (samples, leftover) =
            replay(&scenario, &mut rig, &frames, first_walk, &geometry, &ground, show_layers, own_view, &mut holding);
        write_trace(&out_dir.join(format!("{}.jsonl", scenario.name)), &samples)?;
        let last = &samples.last().context("empty scenario")?.1;
        println!(
            "{}: {} frames; ends {:.2} m out at {:.2} m/s, {} rounds in the magazine",
            scenario.name,
            samples.len(),
            last["distance"][0],
            last["ground_speed"][0],
            last["magazine"][0]
        );
        if own_view {
            let turning = |f: &&Frame| f.asked.is_some_and(|turn| turn != [0.0; 2]);
            println!(
                "  the view turned from what the script asked, through the simulation's own limits: {} of {} frames carry the asking, {} of them a turn",
                frames.iter().filter(|f| f.asked.is_some()).count(),
                frames.len(),
                frames.iter().filter(turning).count()
            );
        }
        // The second pair of one scenario should be the first pair of the
        // next: the recording ran them back to back.
        if let Some(taken) = leftover {
            let left = rig.movement.walk_leftover();
            println!(
                "  the walking state's leftover direction rate, sideways and ahead per second: ({:.2}, {:.2}) read off the recording's first walking frame, ({:.2}, {:.2}) left by the replay",
                taken[0], taken[1], left[0], left[1]
            );
        }
        let Some((header, frames)) = recorded else {
            missing += traces_dir.is_some() as usize;
            continue;
        };
        let from_game = |name: &str| header.get(name).map(Json::to_string).unwrap_or_else(|| "?".into());
        println!(
            "  game trace: weapon {} kg, round {}{}, ergonomics {}, overweight {}, inertia {}, top speed setting {}, field of view {}, head bobbing {}",
            from_game("weapon_weight"),
            from_game("ammo"),
            round.map_or(String::new(), |r| format!(" ({}, recoil {:+}%)", r.name, r.prop_f32("ammoRec").unwrap_or(0.0))),
            from_game("ergonomics"),
            from_game("overweight"),
            from_game("inertia"),
            from_game("max_speed"),
            from_game("fov"),
            from_game("far_aim_plane"),
        );
        print!("{}", compare(&samples, &frames));
        compared += 1;
    }
    match &traces_dir {
        Some(dir) => println!("compared {compared} scenarios with traces in {}; {missing} had no trace", dir.display()),
        None => println!("no --traces folder given: replayed only. Simulation traces are in {}", out_dir.display()),
    }
    Ok(())
}
