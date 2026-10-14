//! The weapon's rig with no engine: its bones, each in its parent, as the
//! weapon's animator has them. For a host that draws nothing and still needs
//! what the clips move: the rig's shoulders, the hand markers, the elbows'
//! goals. A host that draws keeps the same bones in its own scene.

use std::collections::HashMap;

use trp_data::unity::extract::Hierarchy;

use crate::arms::{Placed, RigArm};
use crate::math::{quat_mul, quat_rotate};

/// The bone the procedural motion moves.
const MOVED_BONE: &str = "Weapon_root/Weapon_root_anim";

#[derive(Debug, Clone, Copy)]
struct Local {
    at: [f32; 3],
    turn: [f32; 4],
    scale: [f32; 3],
}

/// Which of the rig's bones one arm reads.
#[derive(Debug, Clone, Copy)]
pub struct ArmNodes {
    pub upper: usize,
    pub marker: usize,
    pub reference: usize,
    pub goal: usize,
    pub grip: Option<usize>,
}

impl ArmNodes {
    /// Finds them in an assembled weapon.
    pub fn find(hierarchy: &Hierarchy, right: bool) -> Option<Self> {
        let side = if right { 'R' } else { 'L' };
        let node = |path: &str| (0..hierarchy.nodes.len()).find(|&i| hierarchy.path(i) == path);
        let on_weapon = |name: &str| node(&format!("{MOVED_BONE}/weapon/{name}"));
        let depth = |mut index: usize| {
            let mut depth = 0;
            while let Some(parent) = hierarchy.nodes[index].parent {
                depth += 1;
                index = parent;
            }
            depth
        };
        // The game takes the ordinary grip for this hand that sits deepest
        // in the weapon, with an underbarrel weapon's grip ahead of others.
        let grip = (0..hierarchy.nodes.len())
            .filter(|&i| hierarchy.nodes[i].grip.as_ref().is_some_and(|g| g.left_hand != right && g.kind != 1))
            .max_by_key(|&i| (hierarchy.nodes[i].grip.as_ref().map(|g| g.kind == 2), depth(i)));
        Some(Self {
            upper: node(&format!("Base Human{side}Collarbone/Base Human{side}Upperarm"))?,
            marker: on_weapon(&format!("weapon_{side}_IK_marker"))?,
            reference: on_weapon(&format!("weapon_{side}_hand_marker"))?,
            goal: on_weapon(if right { "Bend_Goal_Right" } else { "Bend_Goal_Left" })?,
            grip,
        })
    }
}

/// The bones of a weapon the eye is lined up with while the sights are up
/// (`ProceduralWeaponAnimation.FindAimTransforms`, `method_15`;
/// weapon-handling spec, "The sights"): the weapon's own bone, in which
/// the rest are measured; the aim bone of the sight in use; the two points
/// of the iron sights' line, rear and front, where the weapon has both;
/// and the muzzle, about which the weapon is turned to level that line.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SightNodes {
    pub weapon: usize,
    pub bone: usize,
    pub line: Option<[usize; 2]>,
    pub muzzle: Option<usize>,
}

/// The name of a sight's own aim bone, and of the weapon's own, which is
/// used where the build has no sight with one.
const SIGHT_AIM_BONE: &str = "mod_aim_camera";
const WEAPON_AIM_BONE: &str = "aim_camera";
/// How far from straight back a sight's aim bone may be turned and still
/// be used, in degrees (`ScopeRotationWithinLimits`).
const SIGHT_TURN_LIMIT: f32 = 55.0;

impl SightNodes {
    /// Finds them in an assembled weapon. For a build whose sights are
    /// iron sights, or none: the first sight's aim bone that is not turned
    /// too far aside, else the weapon's own; and for the line the deepest
    /// bone named for the rear sight's point and the deepest named for the
    /// front's. (A build with an optic or a collimator has an aim bone for
    /// each, chosen between by a key: not read here.)
    pub fn find(hierarchy: &Hierarchy) -> Option<Self> {
        let nodes = &hierarchy.nodes;
        let weapon = (0..nodes.len()).find(|&i| hierarchy.path(i) == format!("{MOVED_BONE}/weapon"))?;
        let rig = WeaponRig::new(hierarchy);
        let below = |mut index: usize| {
            let mut depth = 0;
            while let Some(parent) = nodes[index].parent {
                if parent == weapon {
                    return Some(depth);
                }
                (depth, index) = (depth + 1, parent);
            }
            None
        };
        // Turned about the weapon's upright axis by no more than the limit
        // from a half turn, as the weapon has it.
        let straight = |index: usize| {
            let (_, weapon_turn) = rig.placed(weapon);
            let (_, turn) = rig.placed(index);
            let back = [-weapon_turn[0], -weapon_turn[1], -weapon_turn[2], weapon_turn[3]];
            let about_y = crate::math::quat_to_euler(quat_mul(back, turn))[1].rem_euclid(360.0);
            (180.0 - about_y).abs() < SIGHT_TURN_LIMIT
        };
        let named = |name: &str| -> Vec<usize> { (0..nodes.len()).filter(|&i| nodes[i].name == name && below(i).is_some()).collect() };
        let bone = named(SIGHT_AIM_BONE).into_iter().find(|&i| straight(i)).or_else(|| named(WEAPON_AIM_BONE).into_iter().next())?;
        let deepest = |part: &str| {
            let mut found: Option<(usize, usize)> = None;
            for index in (0..nodes.len()).filter(|&i| nodes[i].name.contains(part)) {
                let Some(depth) = below(index) else { continue };
                if found.is_none_or(|(_, best)| depth > best) {
                    found = Some((index, depth));
                }
            }
            found.map(|(index, _)| index)
        };
        let line = deepest("mod_align_rear").zip(deepest("mod_align_front")).map(|(rear, front)| [rear, front]);
        let muzzle = (0..nodes.len()).find(|&i| nodes[i].name == "fireport");
        Some(Self { weapon, bone, line, muzzle })
    }

    /// A bone's place in the weapon's own bone, as the rig is posed.
    fn on_weapon(&self, rig: &WeaponRig, bone: usize) -> [f32; 3] {
        let (at, turn) = rig.placed(self.weapon);
        let back = [-turn[0], -turn[1], -turn[2], turn[3]];
        let there = rig.placed(bone).0;
        quat_rotate(back, std::array::from_fn(|k| there[k] - at[k]))
    }

    /// The line through the two iron sights as the rig is posed
    /// (`ProceduralWeaponAnimation.method_14`), if the weapon has both
    /// points: how far it is pitched from the weapon's own way, and where
    /// it passes the aim bone's depth, as a shift of the eye from that
    /// bone along the barrel and up. In the weapon's own bone the barrel
    /// runs along -y and up is z.
    pub fn line(&self, rig: &WeaponRig) -> Option<crate::hands::SightLine> {
        let [rear, front] = self.line?;
        let flat = |at: [f32; 3]| [-at[1], at[2]];
        let (rear, front, bone) = (flat(self.on_weapon(rig, rear)), flat(self.on_weapon(rig, front)), flat(self.on_weapon(rig, self.bone)));
        let (rise, run) = (front[1] - rear[1], front[0] - rear[0]);
        let size = (rise * rise + run * run).sqrt();
        if size < f32::EPSILON || run.abs() < f32::EPSILON {
            return None;
        }
        // Back along the line from the rear sight as far as the aim bone
        // lies behind it.
        let behind = rear[0] - bone[0];
        let back = [-run / size, -rise / size];
        Some(crate::hands::SightLine {
            angle: (rise / run).atan().to_degrees(),
            shift: [rear[0] + back[0] * behind - bone[0], rear[1] + back[1] * behind - bone[1]],
        })
    }

    /// The aim bone and the muzzle in the axes of the bone the procedural
    /// motion moves, as the rig is posed.
    pub fn in_moved(&self, rig: &WeaponRig) -> Option<([f32; 3], Option<[f32; 3]>)> {
        let (at, turn) = rig.placed(rig.moved_bone()?);
        let back = [-turn[0], -turn[1], -turn[2], turn[3]];
        let within = |bone: usize| {
            let there = rig.placed(bone).0;
            quat_rotate(back, std::array::from_fn(|k| there[k] - at[k]))
        };
        Some((within(self.bone), self.muzzle.map(within)))
    }

    /// The aim bone in the weapon's own bone, as the rig is posed.
    pub fn bone_on_weapon(&self, rig: &WeaponRig) -> [f32; 3] {
        self.on_weapon(rig, self.bone)
    }

    /// The muzzle in the weapon's own bone, as the rig is posed.
    pub fn muzzle_on_weapon(&self, rig: &WeaponRig) -> Option<[f32; 3]> {
        self.muzzle.map(|muzzle| self.on_weapon(rig, muzzle))
    }
}

pub struct WeaponRig {
    parents: Vec<Option<usize>>,
    local: Vec<Local>,
    by_path: HashMap<String, usize>,
}

impl WeaponRig {
    /// The rig at rest.
    pub fn new(hierarchy: &Hierarchy) -> Self {
        let by_path: HashMap<String, usize> = (0..hierarchy.nodes.len()).map(|i| (hierarchy.path(i), i)).collect();
        Self {
            parents: hierarchy.nodes.iter().map(|n| n.parent).collect(),
            local: hierarchy.nodes.iter().map(|n| Local { at: n.translation, turn: n.rotation, scale: n.scale }).collect(),
            by_path,
        }
    }

    /// A bone by its path below the rig's root.
    pub fn bone(&self, path: &str) -> Option<usize> {
        self.by_path.get(path).copied()
    }

    /// The bone the procedural motion moves.
    pub fn moved_bone(&self) -> Option<usize> {
        self.bone(MOVED_BONE)
    }

    /// A bone in its parent.
    pub fn local(&self, bone: usize) -> Placed {
        (self.local[bone].at, self.local[bone].turn)
    }

    pub fn set_local(&mut self, bone: usize, placed: Placed) {
        (self.local[bone].at, self.local[bone].turn) = placed;
    }

    /// A bone in the frame the rig hangs in: from the rig's root, without
    /// that root's own place.
    pub fn placed(&self, bone: usize) -> Placed {
        let mut chain = Vec::new();
        let mut index = bone;
        while let Some(parent) = self.parents[index] {
            chain.push(index);
            index = parent;
        }
        let (mut at, mut turn, mut scale) = ([0.0f32; 3], [0.0f32, 0.0, 0.0, 1.0], [1.0f32; 3]);
        for &i in chain.iter().rev() {
            let l = &self.local[i];
            let hanging = quat_rotate(turn, std::array::from_fn(|k| l.at[k] * scale[k]));
            at = std::array::from_fn(|k| at[k] + hanging[k]);
            turn = quat_mul(turn, l.turn);
            scale = std::array::from_fn(|k| scale[k] * l.scale[k]);
        }
        (at, turn)
    }

    /// What the rig gives one arm's solver.
    pub fn arm(&self, nodes: &ArmNodes) -> RigArm {
        RigArm {
            upper: self.placed(nodes.upper),
            marker: self.placed(nodes.marker),
            reference: self.placed(nodes.reference).0,
            grip: nodes.grip.map(|grip| self.placed(grip)),
            goal: self.placed(nodes.goal).0,
        }
    }

    /// Puts every bone as an animator of this rig has it after its last
    /// update (`WeaponAnimator::pose`), each in its parent. `but` is a
    /// bone to leave as it is: the one the procedural motion moves, which
    /// none of a weapon's clips places and a host places itself.
    pub fn pose(&mut self, pose: &[Placed], but: Option<usize>) {
        for (bone, placed) in pose.iter().enumerate().filter(|(bone, _)| Some(*bone) != but) {
            if let Some(local) = self.local.get_mut(bone) {
                (local.at, local.turn) = *placed;
            }
        }
    }
}
