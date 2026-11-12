//! Muzzle parts, shell ports, and the rig's frame and pose.

use super::*;

/// A weapon's muzzle flash as its rig was made (`MuzzleManager.
/// UpdateJetsAndFumes`): the entity its jets' mesh hangs from, the flash's
/// figures, and the mesh's corners and indices.
#[derive(Clone)]
pub struct MuzzleParts {
    pub parent: Entity,
    /// Each gun smoke's object (`MuzzleSmoke`), in the data's order.
    pub smokes: Vec<Entity>,
    /// Each fume's object (`MuzzleFume`), in the data's order.
    pub fumes: Vec<Option<Entity>>,
    /// Each sparks' object (`MuzzleSparks`), in the data's order.
    pub sparks: Vec<Option<Entity>>,
    /// Each heat haze's object (`HeatHazeEmitter`), in the data's order.
    pub hazes: Vec<Option<Entity>>,
    /// Each particle pivot's object and kind (`MuzzleParticlePivot`).
    pub pivots: Vec<(Option<Entity>, i64)>,
    pub data: std::sync::Arc<trp_data::weapons::muzzle::MuzzleData>,
    pub vertices: std::sync::Arc<Vec<trp_sim::weapons::muzzle::JetVertex>>,
    pub indices: std::sync::Arc<Vec<u32>>,
}

/// A weapon's muzzle flash among its rig's entities, its jets' mesh built
/// (`muzzle_view`'s draws; the game's `Random` for each rig).
pub(super) fn muzzle_of(weapon: &trp_data::weapons::weapon::Weapon, entities: &[Entity]) -> Option<MuzzleParts> {
    muzzle_parts(weapon, weapon.muzzle.clone()?, entities, 0)
}

/// Its underbarrel launcher's fireport and own muzzle flash among its rig's
/// entities (underbarrel-launcher spec, "The grenade").
pub(super) fn launcher_of(weapon: &trp_data::weapons::weapon::Weapon, entities: &[Entity]) -> (Option<Entity>, Option<MuzzleParts>) {
    let fireport = weapon.launcher.as_ref().and_then(|part| part.fireport).and_then(|node| entities.get(node).copied());
    let muzzle = weapon.launcher_muzzle.clone().and_then(|data| muzzle_parts(weapon, data, entities, 0x4c41));
    (fireport, muzzle)
}

/// Its underbarrel launcher's chamber point and shell port among its rig's
/// entities (`UnderbarrelContainer.InitTransformsForAmmo`; underbarrel-launcher
/// spec, "Reloading and unloading"): the port with its place as the weapon was
/// made (`_initialShellPortPosition`) and the launcher's own throw.
pub(super) fn launcher_ports_of(weapon: &trp_data::weapons::weapon::Weapon, entities: &[Entity]) -> (Option<Entity>, Option<ShellPort>) {
    let Some(part) = weapon.launcher.as_ref() else { return (None, None) };
    let chamber = part.chamber.and_then(|node| entities.get(node).copied());
    let port = part.shell_port.zip(part.extraction).and_then(|(index, extraction)| {
        Some(ShellPort { entity: *entities.get(index)?, rest: weapon.hierarchy.nodes.get(index)?.translation, extraction, patron: Default::default() })
    });
    (chamber, port)
}

fn muzzle_parts(weapon: &trp_data::weapons::weapon::Weapon, data: trp_data::weapons::muzzle::MuzzleData, entities: &[Entity], salt: u64) -> Option<MuzzleParts> {
    let parent = *entities.get(data.mesh_parent)?;
    let smokes = data.smokes.iter().filter_map(|smoke| entities.get(smoke.node).copied()).collect();
    let fumes = data.fumes.iter().map(|fume| entities.get(fume.node).copied()).collect();
    let sparks = data.sparks.iter().map(|sparks| entities.get(sparks.node).copied()).collect();
    let hazes = data.hazes.iter().map(|haze| entities.get(haze.node).copied()).collect();
    let pivots = data.pivots.iter().map(|&(node, kind)| (entities.get(node).copied(), kind)).collect();
    let seed = weapon.template.bytes().fold(0x4d55_5a5au64 ^ salt, |seed, b| seed.rotate_left(5) ^ u64::from(b));
    let (vertices, indices) = trp_sim::weapons::muzzle::jet_mesh(&data, &weapon.hierarchy, &mut trp_sim::weapons::recoil::Rng::new(seed));
    Some(MuzzleParts { parent, smokes, fumes, sparks, hazes, pivots, data: std::sync::Arc::new(data), vertices: std::sync::Arc::new(vertices), indices: std::sync::Arc::new(indices) })
}

/// A weapon's shell port and how it throws a spent case (`Firearms.
/// SetupShellPortTransform`, `ShellExtractionData`): the port's entity, its
/// place in its parent as the weapon was made (`_initialShellPortPosition`,
/// the game's axes), and the throw's settings.
#[derive(Clone, Copy, Debug)]
pub struct ShellPort {
    pub entity: Entity,
    pub rest: [f32; 3],
    pub extraction: trp_data::weapons::weapon::Extraction,
    /// The throw of a live round out of it (`PatronExtractionSettings`).
    pub patron: trp_data::weapons::weapon::Extraction,
}

/// A weapon's shell port among its rig's entities (its tree's own): of a
/// weapon of several chambers, the first of its ports, whose move and
/// forward every case thrown from them goes by (`_shellPortsTransforms[0]`,
/// `_initialShellPortPosition`).
pub(super) fn shell_port_of(weapon: &trp_data::weapons::weapon::Weapon, entities: &[Entity]) -> Option<ShellPort> {
    let (index, extraction) = weapon.shell_ports.first().copied().or(weapon.shell_port).zip(weapon.extraction)?;
    Some(ShellPort { entity: *entities.get(index)?, rest: weapon.hierarchy.nodes.get(index)?.translation, extraction, patron: weapon.patron_extraction.unwrap_or_default() })
}

/// A weapon's shell ports by its chambers' numbers, among its rig's
/// entities.
pub(super) fn shell_ports_of(weapon: &trp_data::weapons::weapon::Weapon, entities: &[Entity]) -> Vec<Entity> {
    let ports = if weapon.shell_ports.is_empty() { weapon.shell_port.into_iter().collect() } else { weapon.shell_ports.clone() };
    ports.into_iter().filter_map(|index| entities.get(index).copied()).collect()
}

/// The rig with no engine, for what the simulation takes from it as its
/// animator poses it: the bone the weapon hangs from, the marker for the
/// right hand and the sight's aim bone (both in the moved bone's axes),
/// and the turn of the animated camera bone.
pub struct RigFrame {
    pub(super) rig: trp_sim::base::rig::WeaponRig,
    pub(super) parent: Option<usize>,
    moved: Option<usize>,
    /// The weapon's own bone, which the tactical reload's blend holds
    /// toward its rest (`stiffen`).
    pub(super) weapon: Option<usize>,
    marker: Option<usize>,
    /// The bones the eye is lined up with while the sights are up.
    pub(super) sights: Option<trp_sim::base::rig::SightNodes>,
    pub(super) eye: Option<usize>,
    /// The left hand's marker and the weapon's marker for the vest: how
    /// far apart they are weighs the body's reach for the vest, for a
    /// character seen from outside.
    left_marker: Option<usize>,
    vest_marker: Option<usize>,
}

/// What the simulation takes from the rig, in the game's axes.
#[derive(Clone, Copy)]
pub struct RigPose {
    pub parent: ([f32; 3], [f32; 4]),
    pub marker: ([f32; 3], [f32; 4]),
    pub aim_bone: Option<[f32; 3]>,
    /// The way the eye looks along the aim bone, in the moved bone's axes
    /// (`trp_sim::rig::SightNodes::line_in_moved`).
    pub aim_line: [f32; 3],
    pub animated: [f32; 4],
    /// The muzzle in the moved bone's axes, and the line through the two
    /// iron sights, where the weapon has both.
    pub muzzle: Option<[f32; 3]>,
    pub sight_line: Option<trp_sim::SightLine>,
    /// The rear sight, the front sight and the muzzle in the weapon's own
    /// bone, for the shot's direction while the iron sights are up
    /// (`trp_sim::rig::iron_point`).
    pub sight_points: Option<[[f32; 3]; 3]>,
    /// How far the left hand's marker is from the weapon's marker for the
    /// vest, in metres, where the rig has both.
    pub vest_reach: Option<f32>,
}

impl Default for RigPose {
    fn default() -> Self {
        const NO_TURN: [f32; 4] = [0.0, 0.0, 0.0, 1.0];
        Self {
            parent: ([0.0; 3], NO_TURN),
            marker: ([0.0; 3], NO_TURN),
            aim_bone: None,
            aim_line: [0.0; 3],
            animated: trp_sim::base::math::quat_euler([-90.0, 0.0, 0.0]),
            muzzle: None,
            sight_line: None,
            sight_points: None,
            vest_reach: None,
        }
    }
}

impl RigFrame {
    pub(super) fn new(built: &trp_data::weapons::weapon::Weapon) -> Self {
        let hierarchy = &built.hierarchy;
        let rig = trp_sim::base::rig::WeaponRig::new(hierarchy);
        let named = |name: &str| hierarchy.nodes.iter().position(|n| n.name == name);
        Self {
            parent: rig.bone("Weapon_root"),
            moved: rig.moved_bone(),
            weapon: rig.bone("Weapon_root/Weapon_root_anim/weapon"),
            marker: rig.bone("Weapon_root/Weapon_root_anim/weapon/weapon_R_IK_marker"),
            // The aim bone of the sight the game aims by, and the two
            // points of the iron sights' line (weapon-handling spec, "The
            // sights"). The empty hands have none.
            sights: trp_sim::base::rig::SightNodes::find(hierarchy),
            eye: named("Camera_animated"),
            left_marker: rig.bone("Weapon_root/Weapon_root_anim/weapon/weapon_L_IK_marker"),
            // Only where the rig links it for the player (`Weapon::vest_linked`).
            vest_marker: rig.bone("Weapon_root/Weapon_root_anim/weapon/weapon_vest_IK_marker").filter(|_| built.vest_linked),
            rig,
        }
    }

    /// The bones of the sight now aimed by (`trp_sim::Hands::sight_nodes`):
    /// the eye's aim bone and the iron sights' line are read at them.
    pub fn set_sights(&mut self, nodes: trp_sim::base::rig::SightNodes) {
        self.sights = Some(nodes);
    }

    /// The weapon's own bone in an animator's pose of this rig held toward
    /// its rest by the tactical reload's blend (`Hands::stiffen_weapon_bone`),
    /// before anything reads or draws the pose, as the game's
    /// `ApplyTacticalReloadTransformations` follows the animator.
    /// An underbarrel launcher's part of an animator's pose of this rig
    /// (underbarrel-launcher spec, "While the launcher is in use"): its own
    /// animator's pose over its bones, and while its bones are in use the
    /// weapon's bone carried by its prop bone (`trp_sim::rig::WeaponRig::
    /// carry_by`), before anything reads or draws the pose.
    pub fn launcher_pose(&self, firearm: &trp_sim::Firearm, pose: &mut [([f32; 3], [f32; 4])], drawn: bool) {
        firearm.put_launcher_pose(pose, drawn);
        let prop = firearm.launcher_bones().and_then(|bones| bones.prop).filter(|_| firearm.launcher_bones_in_use());
        if let (Some(weapon), Some(prop)) = (self.weapon, prop) {
            self.rig.carry_by(pose, weapon, prop);
        }
    }

    pub fn stiffen(&self, hands: &trp_sim::Hands, pose: &mut [([f32; 3], [f32; 4])]) {
        if let Some(bone) = self.weapon.and_then(|weapon| pose.get_mut(weapon)) {
            *bone = hands.stiffen_weapon_bone(*bone);
        }
    }

    /// A bone in the frame the rig hangs in (its root's, the game's axes,
    /// with no squeeze), the rig as last read (`read`) and its moved bone
    /// where the procedural motion puts it (`moved`, in its parent).
    ///
    /// The turn is kept at unit length, as an engine keeps a transform's.
    /// The moved bone stays where this leaves it, the next `read` takes
    /// the right hand's marker off it, and while the body has that hand
    /// the next place is worked out from that marker (`Hands::held_onto`).
    /// A turn a hair off unit length came back squared each frame: a
    /// knife's place was not a number 25 frames into a sprint.
    pub fn in_root(&mut self, bone: usize, moved: ([f32; 3], [f32; 4])) -> ([f32; 3], [f32; 4]) {
        if let Some(m) = self.moved {
            self.rig.set_local(m, (moved.0, trp_sim::base::math::quat_normalize(moved.1)));
        }
        self.rig.placed(bone)
    }

    /// Reads them off the rig as an animator of it has just posed it.
    pub fn read(&mut self, pose: &[([f32; 3], [f32; 4])]) -> RigPose {
        use trp_sim::base::math::{quat_mul, quat_rotate};
        self.rig.pose(pose, self.moved);
        let mut out = RigPose::default();
        if let Some(parent) = self.parent {
            out.parent = self.rig.local(parent);
        }
        if let Some(eye) = self.eye {
            out.animated = self.rig.local(eye).1;
        }
        let Some(moved) = self.moved else { return out };
        // A bone in the moved bone's own axes.
        let (at, turn) = self.rig.placed(moved);
        let back = [-turn[0], -turn[1], -turn[2], turn[3]];
        let within = |bone: usize| {
            let (bone_at, bone_turn) = self.rig.placed(bone);
            (quat_rotate(back, std::array::from_fn(|k| bone_at[k] - at[k])), quat_mul(back, bone_turn))
        };
        if let Some(marker) = self.marker {
            out.marker = within(marker);
        }
        if let (Some(left), Some(vest)) = (self.left_marker, self.vest_marker) {
            let (left, vest) = (self.rig.placed(left).0, self.rig.placed(vest).0);
            out.vest_reach = Some((0..3).map(|k| (left[k] - vest[k]).powi(2)).sum::<f32>().sqrt());
        }
        if let Some(sights) = &self.sights {
            if let Some((bone, muzzle)) = sights.in_moved(&self.rig) {
                (out.aim_bone, out.muzzle) = (Some(bone), muzzle);
            }
            out.aim_line = sights.line_in_moved(&self.rig).unwrap_or_default();
            out.sight_line = sights.line(&self.rig);
            out.sight_points = sights.iron_points(&self.rig);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use trp_sim::base::math::{quat_mul, quat_normalize, quat_rotate};

    /// While the body has the right hand the weapon goes where the palm
    /// is, by the marker the frame before left on the rig
    /// (weapon-handling spec, "In the body's hand"). Its place holds
    /// through a long sprint.
    #[test]
    fn a_rig_put_in_the_body_s_hand_keeps_its_place() {
        let Ok(install) = trp_data::Install::locate() else {
            eprintln!("skipped: no install configured (set SPT_DIR or local/spt_dir.txt)");
            return;
        };
        let db = trp_data::Database::load(&install).expect("database");
        let mut unity = trp_data::unity::Assets::new(install).expect("manifest");
        // The ice pick's own prefab.
        let item = db.items.get("5c0126f40db834002a125382").expect("the ice pick");
        let prefab = item.props["Prefab"]["path"].as_str().expect("its prefab").to_owned();
        let weapon = trp_data::weapons::weapon::assemble_prefab(&mut unity, &prefab).expect("its rig");
        let mut frame = RigFrame::new(&weapon);
        let marker = frame.marker.expect("its marker for the right hand");
        let rest: Vec<_> = (0..weapon.hierarchy.nodes.len()).map(|bone| frame.rig.local(bone)).collect();
        let palm = ([0.36, -0.32, -0.33], quat_normalize([-0.52, -0.85, 0.0, -0.1]));
        for step in 0..400 {
            let pose = frame.read(&rest);
            // The marker put on the palm, the hand all the body's
            // (`Hands::held_onto` with the rig's weight at nought).
            let (marker_at, marker_turn) = pose.marker;
            let turn = quat_mul(palm.1, [-marker_turn[0], -marker_turn[1], -marker_turn[2], marker_turn[3]]);
            let reach = quat_rotate(turn, marker_at);
            let at: [f32; 3] = std::array::from_fn(|k| palm.0[k] - reach[k]);
            let (parent_at, parent_turn) = pose.parent;
            let back = [-parent_turn[0], -parent_turn[1], -parent_turn[2], parent_turn[3]];
            let moved = (quat_rotate(back, std::array::from_fn(|k| at[k] - parent_at[k])), quat_mul(back, turn));
            let (marked, _) = frame.in_root(marker, moved);
            let off = (0..3).map(|k| (marked[k] - palm.0[k]).powi(2)).sum::<f32>().sqrt();
            assert!(off < 1e-3, "step {step}: the marker is {off} m off the palm");
        }
    }
}
