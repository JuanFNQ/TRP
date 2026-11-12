//! Hip inaccuracy, sights, zoom, skills and the field of view.

use super::*;

impl Hands {
    /// The hip wander's scale as the firearm's controller works it out
    /// (`UpdateHipInaccuracy`: as the weapon is drawn or changed, and as a
    /// tactical device's request is taken): by the weapon's total
    /// ergonomics, and lower with a combo unit on (`tactical::hip_penalty`;
    /// tactical-devices spec, "What it does to the weapon").
    pub fn update_hip_inaccuracy(&mut self, combo_on: bool) {
        self.combo_on = combo_on;
        self.breath.set_hip_penalty(crate::weapons::tactical::hip_penalty(self.settings.total_ergonomics(), combo_on));
    }

    /// The hip penalty in force (`Breath.HipPenalty`).
    pub fn hip_penalty(&self) -> f32 {
        self.breath.hip_penalty()
    }

    /// Whether a combo unit was on when the hip penalty was last worked out.
    pub fn combo_on(&self) -> bool {
        self.combo_on
    }

    /// The weapon's sights, where it has any (`crate::sights`).
    pub fn sights(&self) -> Option<&crate::weapons::sights::Sights> {
        self.settings.sights.as_ref()
    }

    /// The bones the rig is read at for the sight aimed by, where the
    /// weapon has sights (`crate::sights::Sights::nodes`).
    pub fn sight_nodes(&self) -> Option<crate::base::rig::SightNodes> {
        self.sights().map(crate::weapons::sights::Sights::nodes)
    }

    /// The main camera's field of view with the sights up: 35 degrees
    /// through an optic, the setting less 15 over anything else.
    pub(super) fn aimed_fov(&self) -> f32 {
        let fov = self.settings.field_of_view;
        // A controller with no weapon's data leaves it as set
        // (`OnAimOrPoseChanged` narrows it only with a firearm's: the
        // rangefinder's, rangefinder spec).
        if self.fov_kept_aimed {
            return fov;
        }
        self.sights().map_or(fov - AIM_FOV_DROP, |sights| sights.aimed_field_of_view(fov))
    }

    /// The field of view kept as set while aiming (`keep_fov_aimed`).
    pub fn keep_fov_aimed(&mut self) {
        self.fov_kept_aimed = true;
    }

    /// The key to the next sight (`ChangeAimScope`; optics spec, "The
    /// sights a weapon has"): refused by the caller in blind fire and with
    /// the left hand busy, and here, mounted, past any sight turned aside.
    /// Whether the sight changed: the eye then goes to the new one at the
    /// aiming rate, its line is measured again, and the field of view
    /// starts toward the new sight's (`CycleScopes`, `OnAimOrPoseChanged`).
    pub fn change_aim_scope(&mut self, mounted: bool) -> bool {
        let Some(sights) = self.settings.sights.as_mut() else { return false };
        let changed = sights.change_aim_scope(mounted);
        if changed {
            self.sight_changed();
        }
        changed
    }

    /// An underbarrel launcher's sight made the only one (its aim bone), or
    /// the weapon's own given back (`IsGrenadeLauncher`: the aim transforms
    /// found again, then `OnAimOrPoseChanged`; `crate::sights::Sights::
    /// set_grenade`): the eye goes to it at the aiming rate, its line is
    /// measured again, and the field of view starts toward its.
    pub fn set_grenade_sight(&mut self, bone: Option<usize>) {
        let Some(sights) = self.settings.sights.as_mut() else { return };
        if sights.set_grenade(bone) {
            self.sight_changed();
        }
    }

    /// A mount found: a canted sight in use gives way to the first straight
    /// one (`OnFoundPoint`), and with none, whether the sight in use is still
    /// canted (the mount's start then puts the sights down:
    /// `ToggleFirearmAimByBodyAction`).
    pub fn straighten_for_mount(&mut self) -> bool {
        let Some(sights) = self.settings.sights.as_mut() else { return false };
        if sights.first_straight() {
            self.sight_changed();
        }
        self.settings.sights.as_ref().is_some_and(|sights| sights.scope_rotation() != 0.0)
    }

    /// The magnification key (`ChangeAimScopeMagnification`; optics spec,
    /// "Zoom and modes"): the scope aimed by goes to its next mode and
    /// every variable scope snaps to the far end of its range. Whether it
    /// was done; the host then gives the hands' animator its `ModSwitch`.
    pub fn change_magnification(&mut self) -> bool {
        let Some(sights) = self.settings.sights.as_mut() else { return false };
        let changed = sights.change_magnification();
        if changed {
            self.sight_changed();
        }
        changed
    }

    /// One frame of the zoom's wheel (`ScopeZoomHandler`): `input` is the
    /// wheel's turn on the frame with the zoom's keys held, times the
    /// options' `OpticSensitivity` (wheel up positive, which narrows), `dt`
    /// the frame's time.
    pub fn zoom_frame(&mut self, input: f32, dt: f32) {
        let aiming = self.aiming;
        if let Some(sights) = self.settings.sights.as_mut() {
            sights.zoom_frame(input, dt, aiming);
        }
    }

    /// What the mouse's aiming sensitivity is multiplied by while aiming:
    /// the sight's figure (`crate::sights::Sights::aim_sensitivity`), or the
    /// weapon's own where it has no sights.
    pub fn aim_sensitivity(&self, weapon: f32) -> f32 {
        self.sights().map_or(weapon, |sights| sights.aim_sensitivity(weapon))
    }

    /// Another sight or mode is in force (`OnAimOrPoseChanged`): the field
    /// of view starts again toward the figure for the sights as they are,
    /// unless the mouse has the head; the line of the iron sights is taken
    /// again with the sights up.
    fn sight_changed(&mut self) {
        if self.aiming {
            self.retake_line = true;
        }
        let in_range = self.sights().is_some_and(crate::weapons::sights::Sights::in_range);
        if in_range && !self.head_control {
            let target = if self.aiming { self.aimed_fov() } else { self.settings.field_of_view };
            self.set_fov(target, 1.0);
        }
    }

    /// The player has changed the game's field-of-view setting (50 to 75)
    /// while playing: the view eases to the new figure, less the aim's
    /// drop with the sights up, and the weapon's squeeze goes by it from
    /// here on. PROVISIONAL: the game takes a changed setting when its
    /// settings screen is saved, and how its camera gets there was not
    /// read; the ease is the one a change of aim uses.
    /// The skills' share in the handling (Aim Drills' aiming speed, the
    /// weapon class's ergonomics), as the game takes them when the weapon
    /// is drawn or changed (`UpdateWeaponVariables`): what goes by the
    /// weapon's weight and ergonomics is worked out again.
    pub fn set_skills(&mut self, aim_speed_bonus: f32, ergonomics_buff: f32, recoil_suppression: f32) {
        if (aim_speed_bonus, ergonomics_buff) != (self.settings.aim_speed_bonus, self.settings.ergonomics_buff) {
            (self.settings.aim_speed_bonus, self.settings.ergonomics_buff) = (aim_speed_bonus, ergonomics_buff);
            (self.sway_strength, self.displacement_strength, self.aiming_speed) = self.settings.by_weight();
            self.aim_sway_strength = self.settings.aim_sway_strength();
            self.keep_manual();
            self.update_hip_inaccuracy(self.combo_on);
        }
        // The recoil's strengths with the class skill's suppression
        // (`RecalculateRecoilParamsOnChangeWeapon`), as built and folded,
        // and those of the stock as it stands put on the recoil.
        if recoil_suppression != self.settings.recoil_suppression {
            let Some(force) = self.settings.recoil_force else { return };
            self.settings.recoil_suppression = recoil_suppression;
            (self.settings.recoil.rotation_strength, self.settings.recoil.position_strength) = force.strengths(force.parts, recoil_suppression);
            if let (Some(hold), Some(parts)) = (self.settings.folded.as_mut(), force.folded_parts) {
                (hold.rotation_strength, hold.position_strength) = force.strengths(parts, recoil_suppression);
            }
            let (rotation, position) = match self.settings.folded.filter(|_| self.folded) {
                Some(hold) => (hold.rotation_strength, hold.position_strength),
                None => (self.settings.recoil.rotation_strength, self.settings.recoil.position_strength),
            };
            self.recoil.set_strengths(rotation, position);
        }
    }

    /// Whether the sights brought up hold the breath stiff for 3 s (skills
    /// spec, "The stiff draw"): the firearm just drawn's (`GetWeaponInfo`'s
    /// `StiffDraw`, taken as its controller is made; a skills' change while
    /// it is held, `SyncWithCharacterSkills`, leaves it). Hands that hold
    /// no firearm keep the last one's, as the game's weapon motion does.
    pub fn set_stiff_draw(&mut self, stiff: bool) {
        self.breath.set_stiff_draw(stiff);
    }

    /// The aiming speed and the aim sway's strength set by hand
    /// (`ManualSetVariables`; the overweight and the ergonomic weight it
    /// sets as well are not kept here: `PROVISIONAL`, the sway's strength
    /// stays the rig's).
    pub fn manual_set_variables(&mut self, aiming_speed: f32, aim_sway_strength: f32) {
        self.manual = Some((aiming_speed, aim_sway_strength));
        self.keep_manual();
    }

    pub(super) fn keep_manual(&mut self) {
        if let Some((speed, sway)) = self.manual {
            (self.aiming_speed, self.aim_sway_strength) = (speed, sway);
        }
    }

    /// What a mount adds to the ergonomics' share (`TotalErgonomicsFunc`'s
    /// mounted bonus), worked in as the skills' are (`RecalculateErgonomic`).
    pub fn set_mount_ergonomics(&mut self, bonus: f32) {
        if bonus != self.settings.mount_ergonomics {
            self.settings.mount_ergonomics = bonus;
            (self.sway_strength, self.displacement_strength, self.aiming_speed) = self.settings.by_weight();
            self.aim_sway_strength = self.settings.aim_sway_strength();
            self.keep_manual();
            self.update_hip_inaccuracy(self.combo_on);
        }
    }

    pub fn set_field_of_view(&mut self, degrees: f32) {
        self.settings.field_of_view = degrees;
        self.set_fov(if self.aiming { self.aimed_fov() } else { degrees }, 1.0);
        // The weapon's squeeze goes by the setting, and the sights' eye by
        // the squeeze (`SetCompensationScale`, `SetFovParams`).
        let scale = self.fov_scale();
        if let Some(sights) = self.settings.sights.as_mut() {
            sights.set_scale(scale);
        }
    }

    /// Starts the field of view moving to `target`. `time` is the game's
    /// figure for the move; larger is slower.
    pub(super) fn set_fov(&mut self, target: f32, time: f32) {
        self.fov_target = target;
        self.fov_time = time;
        self.fov_blend = 1.0;
    }

    /// The camera's vertical field of view in degrees, advanced by one
    /// rendered frame. The game's easing is tied to the frame rate.
    pub(super) fn field_of_view(&mut self, frame_dt: f32) -> f32 {
        if self.fov_blend > 0.0 {
            self.fov = lerp(self.fov_target, self.fov, self.fov_blend);
            self.fov_blend -= frame_dt / self.fov_time;
        } else {
            self.fov = self.fov_target;
        }
        self.fov
    }

    /// The camera's field of view as it last stood, degrees.
    pub fn field_of_view_now(&self) -> f32 {
        self.fov
    }

    /// Advances the field of view's easing by one rendered frame alone, for
    /// a host that does not move the camera (`frame` does it with the
    /// rest): what a comparison of the sights against a recording drives.
    pub fn step_field_of_view(&mut self, frame_dt: f32) -> f32 {
        self.field_of_view(frame_dt)
    }

    /// How much the weapon is squeezed along the view at a wide field of
    /// view, so that it fills the same part of the screen.
    pub fn fov_scale(&self) -> f32 {
        lerp(1.0, 0.65, inverse_lerp(50.0, 75.0, self.settings.field_of_view))
    }
}
