//! A weapon's bipod and mount figures.

use super::*;

/// A bipod's figures (`BipodTemplate`, the M60E6's: recoil 0.2, camera
/// snap 1, breath 0.1, return 1, prone pitch -4 to 6, yaw ±15), and
/// whether it folds forward.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BipodFigures {
    pub recoil: f32,
    pub camera_snap: f32,
    pub breath: f32,
    pub return_speed: f32,
    pub pitch_prone: [f32; 2],
    pub yaw_prone: [f32; 2],
    pub forward: bool,
}

/// A weapon's mount figures (its template's; mounting spec, "The mount's
/// data", "The weapon's motion while mounted"), with its pivots in the
/// axes of the weapon's `Weapon_root`, as the game uses them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeaponMount {
    /// `IsMountable`.
    pub mountable: bool,
    /// `MountingRotationCenter`: the template's `MountingPosition`, or the
    /// left hand's marker plus the moved bone's place plus (0.05, -0.18,
    /// 0.025) where that is nought.
    pub pivot: [f32; 3],
    /// `MountingRotationCenterBipods`: the bipod's ground bone (nought for a
    /// bipod without one; none without a bipod).
    pub bipod_pivot: Option<[f32; 3]>,
    /// `MountVerticalRecoilMultiplier`, `MountHorizontalRecoilMultiplier`.
    pub recoil_side: f32,
    pub recoil_forward: f32,
    /// `MountCameraSnapMultiplier`, `MountReturnSpeedHandMultiplier`.
    pub camera_snap: f32,
    pub return_speed: f32,
    /// `MountingVerticalOutOfBreathMultiplier`, `MountingHorizontalOutOfBreathMultiplier`.
    pub breath_side: f32,
    pub breath_forward: f32,
    pub bipod: Option<BipodFigures>,
}

impl Default for WeaponMount {
    fn default() -> Self {
        Self {
            mountable: false,
            pivot: [0.0; 3],
            bipod_pivot: None,
            recoil_side: 0.75,
            recoil_forward: 0.6,
            camera_snap: 1.0,
            return_speed: 1.0,
            breath_side: 0.8,
            breath_forward: 0.4,
            bipod: None,
        }
    }
}

/// The name of the left hand's marker on a weapon.
const LEFT_MARKER: &str = "weapon_L_hand_marker";

impl WeaponMount {
    /// A weapon's figures from its template and its assembled model.
    pub fn of_weapon(db: &trp_data::Database, weapon: &trp_data::weapons::weapon::Weapon) -> Result<Self> {
        let template = &db.item(&weapon.template)?.props;
        let number = |v: &Json, name: &str, or: f32| v.get(name).and_then(Json::as_f64).map_or(or, |n| n as f32);
        let triple = |v: &Json, name: &str| -> [f32; 3] {
            let p = v.get(name);
            let f = |k: &str| p.and_then(|p| p.get(k)).and_then(Json::as_f64).map_or(0.0, |n| n as f32);
            [f("x"), f("y"), f("z")]
        };
        let hierarchy = &weapon.hierarchy;
        let moved = crate::base::rig::WeaponRig::new(hierarchy).moved_bone();
        let moved_at = moved.map_or([0.0; 3], |m| hierarchy.nodes[m].translation);
        let mut pivot = triple(template, "MountingPosition");
        if pivot == [0.0; 3] {
            pivot = match hierarchy.find_below(0, LEFT_MARKER) {
                Some(marker) => add(add(hierarchy.nodes[marker].translation, moved_at), MARKER_TO_PIVOT),
                None => [0.0; 3],
            };
        }
        let bipod = weapon.bipod.as_ref();
        let bipod_pivot = bipod.map(|part| {
            // The ground bone in the axes of the moved bone's child it
            // hangs under (`FindBipodMountingPoint`: up to, not through, it).
            let Some(ground) = part.ground else { return [0.0; 3] };
            let node = &hierarchy.nodes[ground];
            let mut at = node.translation;
            let mut parent = node.parent;
            while let Some(p) = parent {
                let above = hierarchy.nodes[p].parent;
                if above.is_none() || above == moved {
                    break;
                }
                at = add(quat_rotate(hierarchy.nodes[p].rotation, at), hierarchy.nodes[p].translation);
                parent = above;
            }
            at
        });
        let bipod_figures = match bipod {
            Some(part) => {
                let props = &db.item(&part.template)?.props;
                let pair = |name: &str, or: [f32; 2]| -> [f32; 2] {
                    let p = props.get(name);
                    let f = |k: &str, o: f32| p.and_then(|p| p.get(k)).and_then(Json::as_f64).map_or(o, |n| n as f32);
                    [f("x", or[0]), f("y", or[1])]
                };
                Some(BipodFigures {
                    recoil: number(props, "BipodRecoilMultiplier", 0.2),
                    camera_snap: number(props, "BipodCameraSnapMultiplier", 1.0),
                    breath: number(props, "BipodOutOfStaminaBreathMultiplier", 0.1),
                    return_speed: number(props, "BipodReturnHandSpeedMultiplier", 1.0),
                    pitch_prone: pair("PitchLimitProneBipod", [-4.0, 6.0]),
                    yaw_prone: pair("YawLimitProneBipod", [-15.0, 15.0]),
                    forward: part.forward,
                })
            }
            None => None,
        };
        Ok(Self {
            mountable: template.get("IsMountable").and_then(Json::as_bool).unwrap_or(false),
            pivot,
            bipod_pivot,
            recoil_side: number(template, "MountVerticalRecoilMultiplier", 0.75),
            recoil_forward: number(template, "MountHorizontalRecoilMultiplier", 0.6),
            camera_snap: number(template, "MountCameraSnapMultiplier", 1.0),
            return_speed: number(template, "MountReturnSpeedHandMultiplier", 1.0),
            breath_side: number(template, "MountingVerticalOutOfBreathMultiplier", 0.8),
            breath_forward: number(template, "MountingHorizontalOutOfBreathMultiplier", 0.4),
            bipod: bipod_figures,
        })
    }
}
