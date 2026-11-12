//! The weapon's rig with no engine: its bones, each in its parent, as the
//! weapon's animator has them. For a host that draws nothing and still needs
//! what the clips move: the rig's shoulders, the hand markers, the elbows'
//! goals. A host that draws keeps the same bones in its own scene.

use std::collections::HashMap;

use trp_data::unity::extract::Hierarchy;

use crate::weapons::arms::{Placed, RigArm};
use crate::base::math::{quat_mul, quat_rotate};

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
/// of the iron sights' line, rear and front, where the game takes one for
/// that sight; and the muzzle, about which the weapon is turned to level
/// that line. Which sight is in use is `crate::sights`'. `own` says the aim
/// bone is the weapon's own `aim_camera` (`crate::sights::Sight::own`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SightNodes {
    pub weapon: usize,
    pub bone: usize,
    pub line: Option<[usize; 2]>,
    pub muzzle: Option<usize>,
    pub own: bool,
}

impl SightNodes {
    /// A weapon's for the sight it is first aimed by
    /// (`crate::sights::Sights`): the build's sight parts and the rule the
    /// game picks by.
    pub fn of_weapon(weapon: &trp_data::weapons::weapon::Weapon) -> Option<Self> {
        crate::weapons::sights::Sights::of_weapon(weapon).map(|sights| sights.nodes())
    }

    /// A rig's with no sight parts known (the empty hands', an item's): the
    /// first sight's aim bone on it that is not turned too far aside, else
    /// its own `aim_camera`; and the iron sights' line where it has one.
    pub fn find(hierarchy: &Hierarchy) -> Option<Self> {
        crate::weapons::sights::Sights::new(hierarchy, Vec::new(), &[], crate::weapons::sights::AimPlanes::default()).map(|sights| sights.nodes())
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
    pub fn line(&self, rig: &WeaponRig) -> Option<crate::weapons::hands::SightLine> {
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
        Some(crate::weapons::hands::SightLine {
            angle: (rise / run).atan().to_degrees(),
            shift: [rear[0] + back[0] * behind - bone[0], rear[1] + back[1] * behind - bone[1]],
        })
    }

    /// The point the iron sights aim at for the shot's direction
    /// (`ProceduralWeaponAnimation.method_14`), as the rig is posed: in the
    /// weapon's own bone, the front sight plus `range` (at least 25 m) of a
    /// way made of the line's rise and run (`(0, rise, run)` made a unit, as
    /// the game makes it), less the muzzle. None without both sights or a
    /// muzzle.
    pub fn iron_point(&self, rig: &WeaponRig, range: i32) -> Option<[f32; 3]> {
        iron_point(self.iron_points(rig)?, range)
    }

    /// The rear sight, the front sight and the muzzle in the weapon's own
    /// bone, as the rig is posed.
    pub fn iron_points(&self, rig: &WeaponRig) -> Option<[[f32; 3]; 3]> {
        let [rear, front] = self.line?;
        Some([self.on_weapon(rig, rear), self.on_weapon(rig, front), self.on_weapon(rig, self.muzzle?)])
    }
}

/// The iron sights' aim point from their rear, front and the muzzle in the
/// weapon's own bone (`SightNodes::iron_point`).
pub fn iron_point([rear, front, muzzle]: [[f32; 3]; 3], range: i32) -> Option<[f32; 3]> {
    {
        let (rise, run) = (front[2] - rear[2], -front[1] + rear[1]);
        let size = (rise * rise + run * run).sqrt();
        if size < 1e-5 {
            return None;
        }
        let reach = range.max(25) as f32;
        let way = [0.0, rise / size, run / size];
        Some(std::array::from_fn(|k| front[k] + way[k] * reach - muzzle[k]))
    }
}

impl SightNodes {
    /// The way the eye looks along the aim bone, in the axes of the bone
    /// the procedural motion moves, as the rig is posed: the bone's -z
    /// (`-forward`), or its -y (`-up`) where it is the weapon's own
    /// `aim_camera` (`ProceduralWeaponAnimation`, the camera's target in
    /// automatic fire; weapon-handling spec, "Camera"). Its turn alone, as
    /// the game takes a direction.
    pub fn line_in_moved(&self, rig: &WeaponRig) -> Option<[f32; 3]> {
        let turn = rig.placed(rig.moved_bone()?).1;
        let back = [-turn[0], -turn[1], -turn[2], turn[3]];
        let way = if self.own { [0.0, -1.0, 0.0] } else { [0.0, 0.0, -1.0] };
        Some(quat_rotate(quat_mul(back, rig.placed(self.bone).1), way))
    }

    /// The same in the weapon's own bone.
    pub fn line_on_weapon(&self, rig: &WeaponRig) -> [f32; 3] {
        let turn = rig.placed(self.weapon).1;
        let back = [-turn[0], -turn[1], -turn[2], turn[3]];
        let way = if self.own { [0.0, -1.0, 0.0] } else { [0.0, 0.0, -1.0] };
        quat_rotate(quat_mul(back, rig.placed(self.bone).1), way)
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

/// A rig's bones, each in its parent. Its tree and its names are shared
/// between copies: a copy to pose (`Puppet`'s, a pose made when
/// asked) costs its bones' places alone.
#[derive(Clone, Debug)]
pub struct WeaponRig {
    parents: std::sync::Arc<[Option<usize>]>,
    local: Vec<Local>,
    by_path: std::sync::Arc<HashMap<String, usize>>,
}

impl WeaponRig {
    /// The rig at rest.
    pub fn new(hierarchy: &Hierarchy) -> Self {
        let by_path: HashMap<String, usize> = (0..hierarchy.nodes.len()).map(|i| (hierarchy.path(i), i)).collect();
        Self {
            parents: hierarchy.nodes.iter().map(|n| n.parent).collect::<Vec<_>>().into(),
            local: hierarchy.nodes.iter().map(|n| Local { at: n.translation, turn: n.rotation, scale: n.scale }).collect(),
            by_path: std::sync::Arc::new(by_path),
        }
    }

    /// How many bones it has.
    pub fn len(&self) -> usize {
        self.local.len()
    }

    pub fn is_empty(&self) -> bool {
        self.local.is_empty()
    }

    /// A bone by its path below the rig's root.
    pub fn bone(&self, path: &str) -> Option<usize> {
        self.by_path.get(path).copied()
    }

    /// The bone the procedural motion moves.
    pub fn moved_bone(&self) -> Option<usize> {
        self.bone(MOVED_BONE)
    }

    /// A bone's parent, none for the root.
    pub fn parent(&self, bone: usize) -> Option<usize> {
        self.parents.get(bone).copied().flatten()
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

    /// Puts a bone at `placed` in the frame the rig hangs in (as `placed`
    /// gives it back), by its place and turn in its parent, its parent's
    /// scales undone; whatever hangs from it goes with it.
    pub fn set_placed(&mut self, bone: usize, placed: Placed) {
        let Some(parent) = self.parent(bone) else { return };
        let (at, turn) = self.placed(parent);
        // The scales `placed` multiplies the bone's own place by: those of
        // the bones above it, the root's left out.
        let mut scale = [1.0f32; 3];
        let mut index = parent;
        while let Some(above) = self.parents[index] {
            scale = std::array::from_fn(|k| scale[k] * self.local[index].scale[k]);
            index = above;
        }
        let back = [-turn[0], -turn[1], -turn[2], turn[3]];
        let local = quat_rotate(back, std::array::from_fn(|k| placed.0[k] - at[k]));
        let local: [f32; 3] = std::array::from_fn(|k| if scale[k].abs() > f32::EPSILON { local[k] / scale[k] } else { local[k] });
        (self.local[bone].at, self.local[bone].turn) = (local, quat_mul(back, placed.1));
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

    /// The weapon's bone put where a bone below it stands, in a pose of this
    /// rig (`ProceduralWeaponAnimation.UpdateWeaponBoneByLauncherWeaponBone`:
    /// the hands' `Weapon` given the launcher's prop bone's place and turn,
    /// once a frame, after the weapon's motion has placed its moved bone). The
    /// bone below is carried along with everything else on the weapon, so the
    /// weapon's bone in its parent becomes itself followed by the way down to
    /// that bone, whatever the moved bone does. A bone not below the weapon's
    /// leaves the pose as it is.
    pub fn carry_by(&self, pose: &mut [Placed], weapon: usize, prop: usize) {
        let mut chain = Vec::new();
        let mut index = prop;
        while index != weapon {
            chain.push(index);
            match self.parents.get(index).copied().flatten() {
                Some(parent) => index = parent,
                None => return,
            }
        }
        let local = |bone: usize| pose.get(bone).copied().unwrap_or((self.local[bone].at, self.local[bone].turn));
        // The way down in the weapon's bone's axes, scaled as `placed` scales
        // (the bones above the weapon's are at their own size).
        let (mut at, mut turn, mut scale) = ([0.0f32; 3], [0.0f32, 0.0, 0.0, 1.0], self.local[weapon].scale);
        for &bone in chain.iter().rev() {
            let (bone_at, bone_turn) = local(bone);
            let hanging = quat_rotate(turn, std::array::from_fn(|k| bone_at[k] * scale[k]));
            at = std::array::from_fn(|k| at[k] + hanging[k]);
            turn = quat_mul(turn, bone_turn);
            scale = std::array::from_fn(|k| scale[k] * self.local[bone].scale[k]);
        }
        let (weapon_at, weapon_turn) = local(weapon);
        let moved = quat_rotate(weapon_turn, at);
        if let Some(bone) = pose.get_mut(weapon) {
            *bone = (std::array::from_fn(|k| weapon_at[k] + moved[k]), quat_mul(weapon_turn, turn));
        }
    }
}
