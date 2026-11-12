//! The hands' settings, weapon handling, the sight line and the aim point.

use super::*;

/// `globals.json` `config.Aiming`: how weight and ergonomics set the time to aim.
#[derive(Debug, Clone, Copy)]
pub struct AimingConfig {
    pub light_weight: f32,
    pub heavy_weight: f32,
    pub max_time_light: f32,
    pub max_time_heavy: f32,
    pub min_time_light: f32,
    pub min_time_heavy: f32,
}

pub struct HandsSettings {
    pub data: HandsData,
    /// Seconds per physics step.
    pub fixed_timestep: f32,
    /// Angular frequency and damping ratio of the sway spring for a
    /// healthy, unburdened character. The settings object supplies them and
    /// overrides the prefab's.
    pub sway_frequency: f32,
    pub sway_damping: f32,
    /// The same for both arms damaged (`SWAY_FREQ_NORMAL_DAMAGED.y`,
    /// `SWAY_DAMPING_NORMAL_DAMAGED.y`); one arm takes half the way.
    pub sway_damaged: [f32; 2],
    pub sway_strength_per_kg: Curve,
    pub displacement_strength_per_kg: Curve,
    pub aiming: AimingConfig,
    /// Weight of the weapon in kilograms and the sum of its ergonomics.
    pub weapon_weight: f32,
    pub weapon_ergonomics: f32,
    /// The point the weapon swings about, in its own bone space, and the
    /// point recoil turns it about.
    pub rotation_center: [f32; 3],
    pub recoil_pivot: [f32; 3],
    /// How far the tactical reload's blend holds the `weapon` bone toward its
    /// rest (`Hands::stiffen_weapon_bone`): its turn about x, y and z, and its
    /// place (the template's `TacticalReloadStiffnes`, `TacticalReloadFixation`;
    /// every firearm of the install 0.95, 0.33, 0.95 and 0.95). Nought where the
    /// template names none, as the game's template then has.
    pub tactical_stiffness: [f32; 3],
    pub tactical_fixation: f32,
    /// Damping of the weapon's position spring while a weapon is in hand.
    pub hand_damping: f32,
    /// Scale of the procedural motion while aiming.
    pub aim_intensity: f32,
    /// The camera's aiming rate right after a shot.
    pub camera_snap: f32,
    /// Slowest and fastest blend per frame with which the camera follows
    /// the weapon's recoil, and how fast that blend changes per second.
    pub camera_follow_range: [f32; 2],
    pub camera_follow_step: f32,
    pub recoil: RecoilSettings,
    /// Scale of breathing by stance: prone, crouched, standing.
    pub intensity_by_pose: [f32; 3],
    /// What a shot from the hip adds to the muzzle's wander, how long that
    /// holds in seconds, and how fast it fades per second.
    pub breath_gain: [f32; 3],
    /// Seed of the recoil's random numbers.
    pub seed: u64,
    /// What a hit's push is scaled by (`AimPunchMagnitude` of the globals,
    /// the force reaction's `WiggleMagnitude`).
    pub aim_punch: f32,
    /// The player's field-of-view setting: vertical degrees, 50 to 75.
    pub field_of_view: f32,
    /// The player's head-bobbing setting, 0.2 to 1: how much of the
    /// animated camera rotation reaches the camera.
    pub head_bobbing: f32,
    /// What the character's skills add to the aiming speed, as a fraction
    /// (Aim Drills' `DrawSpeed`).
    pub aim_speed_bonus: f32,
    /// What the weapon's class skill adds to the share its ergonomics is
    /// multiplied by (`WeaponErgonomicsBuff`; `TotalErgonomicsFunc`), 0
    /// untrained.
    pub ergonomics_buff: f32,
    /// How far ahead of the weapon's moving bone the eye sits when aiming,
    /// along the barrel, in metres: the nearer of the weapon's two aim planes.
    pub aim_plane_depth: f32,
    /// The weapon's sights as the game picks them (`crate::sights`), for a
    /// weapon that has any: which one is aimed by, and what an optic does
    /// to the main camera's field of view and to the eye's place. Without,
    /// the eye goes to `aim_plane_depth` as over iron sights.
    pub sights: Option<crate::weapons::sights::Sights>,
    /// From the bone the procedural motion moves to the weapon's far end,
    /// in metres: the length of the line that feels for obstacles.
    pub weapon_length: f32,
    /// The bone that moved bone hangs from, in the frame the rig hangs
    /// in, with the weapon at rest: its place and its turn.
    pub weapon_root: ([f32; 3], [f32; 4]),
    /// The weapon is a pistol: it turns away from obstacles by a pose of
    /// its own.
    pub pistol: bool,
    /// The weapon is held closer to the body as built: one of compact
    /// handling with nothing on it that reaches the shoulder
    /// (`CheckShouldMoveWeaponCloser`). And the point it then swings
    /// about (`RotationCenterNoStock`).
    pub held_closer: bool,
    pub rotation_center_no_stock: [f32; 3],
    /// The weapon with its stock folded, if it has one that folds.
    pub folded: Option<FoldedHold>,
    /// What the recoil's strengths are made of (none for hands that hold
    /// no firearm), and the class skill's suppression they were last
    /// worked out with (0 untrained, as made).
    pub recoil_force: Option<RecoilForce>,
    pub recoil_suppression: f32,
    /// The weapon's mount figures (`crate::mounting::WeaponMount`) and the
    /// mount's own (its camera's speed, the sway on the bipod): what the
    /// motion goes by while mounted (mounting spec, "The weapon's motion
    /// while mounted").
    pub mount: crate::weapons::mounting::WeaponMount,
    pub mount_settings: crate::weapons::mounting::Settings,
    /// What a mount adds to the ergonomics' share just now (the class skill's
    /// gain while mounted: `Hands::set_mount_ergonomics`), 0 off a mount.
    pub mount_ergonomics: f32,
    /// The character's load (`Physical.Overweight`, 0 to 1) and the gear's
    /// share of the ergonomics (the armour's and backpack's
    /// `WeaponErgonomicPenalty` over 100, at most 0) as last handed over
    /// (`Hands::set_load`; weapon handling spec, "The load", "The gear's share
    /// of the ergonomics").
    pub overweight: f32,
    pub gear_ergonomics: f32,
    /// The arms damaged, left and right (`EPhysicalCondition.LeftArmDamaged`,
    /// `RightArmDamaged`: broken or destroyed, no painkiller at work), as last
    /// handed over (`Hands::set_health`): with either the ergonomics are 0
    /// (`TotalErgonomicsFunc`; weapon handling spec, "The gear's share of the
    /// ergonomics").
    pub arms_damaged: [bool; 2],
    /// What the load takes the aiming's pace to at its top, and the walk's
    /// share of it at its top (`globals.json` `Stamina.AimingSpeedMultiplier`,
    /// `WalkVisualEffectMultiplier`).
    pub load_aiming: f32,
    pub load_walk: f32,
}

/// What a weapon's two recoil strengths are made of (`NewRecoilShotEffect.
/// RecalculateRecoilParamsOnChangeWeapon`): the template's forces and the
/// aiming settings' bonuses, the share the build's parts make of the forces
/// (`1 + modsFactorRecoil`, at least 0) as built and with the stock folded,
/// and the prefab's ranges and multiplier. The class skill's suppression
/// (`RecoilSupression.x`, 0.003 a level) takes its share off the forces:
/// `max(0, parts × (1 − x))`; Recoil Control's (`.y`) is always 0 here, its
/// skill being out of the game's list.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RecoilForce {
    pub up: f32,
    pub back: f32,
    pub vert_bonus: f32,
    pub back_bonus: f32,
    pub multiplier: f32,
    pub rotation_range: [f32; 2],
    pub position_range: [f32; 2],
    pub parts: f32,
    pub folded_parts: Option<f32>,
}

impl RecoilForce {
    /// The two strengths for a share of the parts and a suppression.
    pub fn strengths(&self, parts: f32, suppression: f32) -> ([f32; 2], [f32; 2]) {
        let parts = (parts * (1.0 - suppression)).max(0.0);
        let scale = |force: f32, bonus: f32| (force * parts + bonus) * self.multiplier;
        let (rotation, position) = (scale(self.up, self.vert_bonus), scale(self.back, self.back_bonus));
        (
            [self.rotation_range[0] * rotation, self.rotation_range[1] * rotation],
            [self.position_range[0] * position, self.position_range[1] * position],
        )
    }
}

/// What a folded stock changes of the weapon's motion: the recoil's two
/// strengths, which lose the stock's share, and whether the weapon is then
/// held closer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FoldedHold {
    pub rotation_strength: [f32; 2],
    pub position_strength: [f32; 2],
    pub held_closer: bool,
}

/// What a weapon's own template gives its motion that an underbarrel
/// launcher's ghost weapon takes over while the launcher is in use
/// (underbarrel-launcher spec, "While the launcher is in use";
/// `ProceduralWeaponAnimation.InitWeaponData` with the launcher and no
/// prefab, `ShotEffector.SubscribeToWeapon`, `NewRecoilShotEffect.
/// OnWeaponParametersChanged`): the recoil's figures and curves, its
/// strengths and their parts, the folded stock's hold, the camera's snap
/// after a shot, hip shots' widening of the wander, the tactical reload's
/// stiffness, and the mount's recoil, snap, return and breath figures. The
/// rest (the ergonomics and the sway, the pivots, the aim planes, the
/// bipod's, the turn-away) stays the weapon's.
#[derive(Debug, Clone)]
pub struct WeaponHandling {
    pub recoil: RecoilSettings,
    pub recoil_force: Option<RecoilForce>,
    pub folded: Option<FoldedHold>,
    pub camera_snap: f32,
    pub breath_gain: [f32; 3],
    pub tactical_stiffness: [f32; 3],
    pub tactical_fixation: f32,
    /// `MountVerticalRecoilMultiplier`, `MountHorizontalRecoilMultiplier`,
    /// `MountCameraSnapMultiplier`, `MountReturnSpeedHandMultiplier`,
    /// `MountingVerticalOutOfBreathMultiplier`, `...HorizontalOutOfBreathMultiplier`.
    pub mount: [f32; 6],
}

impl WeaponHandling {
    /// A weapon's own, as its settings were built.
    pub fn of(settings: &HandsSettings) -> Self {
        let m = &settings.mount;
        Self {
            recoil: settings.recoil.clone(),
            recoil_force: settings.recoil_force,
            folded: settings.folded,
            camera_snap: settings.camera_snap,
            breath_gain: settings.breath_gain,
            tactical_stiffness: settings.tactical_stiffness,
            tactical_fixation: settings.tactical_fixation,
            mount: [m.recoil_side, m.recoil_forward, m.camera_snap, m.return_speed, m.breath_side, m.breath_forward],
        }
    }
}

/// How far a weapon held closer is drawn back, in metres
/// (`ProceduralWeaponAnimation.ZeroAdjustments`).
pub(super) const HELD_CLOSER: f32 = 0.05;

/// The line through a weapon's two iron sights, as measured on its rig
/// (`rig::SightNodes::line`): how far it is pitched from the weapon's own
/// way, in degrees (the front sight higher is positive), and where it
/// passes the depth of the sight's aim bone, as a shift of the eye from
/// that bone: along the barrel, and up.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SightLine {
    pub angle: f32,
    pub shift: [f32; 2],
}

/// How fast the weapon is turned to level its sights' line and back, as a
/// share of what is left a second (`ApplyAimingAlignment`), and the angle
/// under which it is not turned at all, in degrees.
pub(super) const SIGHT_LINE_RATE: f32 = 5.0;

pub(super) const SIGHT_LINE_LEAST: f32 = 0.002;

/// Where the sight in use is, for placing the eye while aiming. Positions
/// are beside the weapon root, in the game's axes.
#[derive(Debug, Clone, Copy)]
pub struct AimPoint {
    /// The sight's aim bone.
    pub bone: [f32; 3],
    /// Origin of the bone the procedural motion moves, and the direction
    /// its barrel points.
    pub origin: [f32; 3],
    pub barrel: [f32; 3],
    /// The way the eye looks along the aim bone
    /// (`crate::rig::SightNodes::line_in_moved`), which the eye's target
    /// slides along in automatic fire.
    pub line: [f32; 3],
}

impl HandsSettings {
    /// The weapon's weight as its ergonomics lighten it: what the sway
    /// scales with, and what aiming costs the arms by.
    /// The aim sway's strength (`UpdateWeaponVariables`): how far the
    /// weapon's weight, less its ergonomics' share, is from where the sway
    /// starts to where it is whole.
    pub fn aim_sway_strength(&self) -> f32 {
        let a = &self.data.aim_sway;
        let e = (self.total_ergonomics() / 100.0).clamp(0.0, 1.0);
        inverse_lerp(a.starts, a.full, self.weapon_weight * (1.0 - e))
    }

    pub fn ergonomic_weight(&self) -> f32 {
        self.weapon_weight * (1.0 - self.total_ergonomics().max(0.0).sqrt() / 25.0)
    }

    /// The ergonomics the character handles the weapon with
    /// (`TotalErgonomics`): its own, times one and the class skill's buff,
    /// the mount's bonus and the gear's share; 0 with an arm damaged, before
    /// any of them.
    pub fn total_ergonomics(&self) -> f32 {
        if self.arms_damaged.contains(&true) {
            return 0.0;
        }
        (self.weapon_ergonomics * (1.0 + self.ergonomics_buff + self.mount_ergonomics + self.gear_ergonomics)).max(0.0)
    }

    /// The weight the sway's and the displacement's strengths are read at:
    /// the ergonomic weight, made heavier by the load (`UpdateSwayFactors`).
    fn loaded_weight(&self) -> f32 {
        self.ergonomic_weight() * (1.0 + self.overweight)
    }

    /// What the load leaves of the aiming's pace (`_overweightAimingMultiplier`).
    pub fn load_aiming_share(&self) -> f32 {
        lerp(1.0, self.load_aiming, self.overweight)
    }

    /// The walk's share of the load (`WalkEffector.Overweight`).
    pub fn load_walk_share(&self) -> f32 {
        lerp(0.0, self.load_walk, self.overweight)
    }

    /// What goes by the weapon's weight and ergonomics
    /// (`UpdateWeaponVariables`): the sway's and the displacement's
    /// strengths and the aiming speed.
    pub(super) fn by_weight(&self) -> (f32, f32, f32) {
        // The weight that sway scales with shrinks as ergonomics rise, and
        // grows with the load.
        let ergonomic_weight = self.loaded_weight();
        let a = self.aiming;
        let t = inverse_lerp(a.light_weight, a.heavy_weight, self.weapon_weight);
        let (slowest, fastest) = (lerp(a.max_time_light, a.max_time_heavy, t), lerp(a.min_time_light, a.min_time_heavy, t));
        let e = (self.total_ergonomics() / 100.0).clamp(0.0, 1.0);
        let k = if e < 0.25 { 0.25 + 3.0 * e * e } else { 2.0 * e - e * e };
        (
            self.sway_strength_per_kg.evaluate(ergonomic_weight),
            self.displacement_strength_per_kg.evaluate(ergonomic_weight),
            1.0 / lerp(slowest, fastest, k) * (1.0 + self.aim_speed_bonus),
        )
    }
}
