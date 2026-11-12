//! What is carried, footsteps, hearing and the body's input.

use super::*;

impl Movement {
    /// Where the body's animation holds the weapon, where it does (a
    /// sprint, a crawl), for `Hands::weapon_transform_carried`.
    pub fn carried(&self) -> Option<Carried> {
        self.body.carried(self.body_in_view())
    }

    /// The body's right palm, where its animation takes the weapon in that
    /// hand (a crawl), for the same.
    pub fn held(&self) -> Option<Held> {
        self.body.held(self.body_in_view())
    }

    /// An arm as the body's animation has it, while it takes any of the
    /// arm from the weapon (a crawl), for a host that draws arms.
    pub fn arm(&self, right: bool) -> Option<ArmPose> {
        self.body.arm(right, self.body_in_view())
    }

    /// An arm as the body's animation has it, whatever share of it that
    /// animation claims: what the arm's solver starts from (`arms::hold`).
    pub fn arm_pose(&self, right: bool) -> ArmPose {
        self.body.arm_pose(right, self.body_in_view())
    }

    /// A footstep that has happened since this was last asked: the seconds
    /// between it and the footstep before. The weapon's bob is tuned by it.
    pub fn take_footstep(&mut self) -> Option<f32> {
        self.footstep.take()
    }

    /// Whether the state has changed into one other than the walk since
    /// this was last asked (`HandsInput::walk_stop`): the game turns the
    /// weapon's bob off on every change of the movement state but into
    /// `Run`, and turning it off rewinds its curves (`Player`'s handler of
    /// `MovementContext.OnStateChanged`, `WalkEffectorEnabled`), whether
    /// the state left was the walk or not.
    pub fn take_walk_stop(&mut self) -> bool {
        std::mem::take(&mut self.walk_stop)
    }

    /// A footstep taken on this moment, the next one's gap measured from
    /// it: for a host that begins with the character already walking and
    /// the last step's time not known (the replay of a recording begun in a
    /// walk or a sprint).
    pub fn note_step_now(&mut self) {
        self.last_step = self.clock;
    }

    /// Footsteps come from the body's animation: a curve in its clips
    /// changes sign each time a foot comes down. One counts if the last
    /// was more than 0.2 s ago and the character has not been falling long.
    pub(super) fn watch_footsteps(&mut self, dt: f32) {
        const SHORTEST_GAP: f32 = 0.2;
        self.clock += dt;
        let longest_fall = match self.state {
            State::Move => 1.0,
            State::Sprint => 0.6,
            _ => return,
        };
        let side = sign(self.body.foot_step());
        if side == self.step_side {
            return;
        }
        self.step_side = side;
        let gap = self.clock - self.last_step;
        if gap > SHORTEST_GAP && self.freefall_time < longest_fall {
            self.last_step = self.clock;
            self.footstep = Some(gap);
            self.stepped = true;
        }
    }

    /// The sounds of moving's rules on this step (`steps.rs`): told the
    /// footstep, the crawl's marks and the landing of the step, the state,
    /// the stance, the speeds and the load, and asking the world what the
    /// line down from the collider's middle meets (`CalculateMovementSurface`:
    /// as long as the collider's height and half a metre).
    pub(super) fn hear(&mut self, dt: f32, world: &dyn World) {
        let capsule = self.capsule();
        let middle = self.position[1] + (capsule.underside + capsule.top) / 2.0;
        let reach = capsule.top - capsule.underside + 0.5;
        let from = [self.position[0], middle, self.position[2]];
        let to = [self.position[0], middle - reach, self.position[2]];
        let moment = crate::sound::steps::Moment {
            clock: self.clock,
            dt,
            state: self.state,
            footstep: std::mem::take(&mut self.stepped),
            crawl_marks: std::mem::take(&mut self.crawl_marks),
            landed: self.landed.take(),
            prone: self.prone,
            pose_level: self.pose_level,
            smoothed_pose_level: self.smoothed_pose_level,
            clamped_speed: self.clamped_speed(),
            speed: self.character_speed,
            max_speed: self.settings.max_speed,
            speed_set: std::mem::take(&mut self.speed_set),
            overweight: self.stamina.settings.overweight,
            walk_overweight: self.stamina.settings.walk_overweight,
            leg_damaged: self.conditions.leg_damaged(),
            yaw: self.yaw,
            hands_to_body: self.hands_to_body,
        };
        self.sounds.update(&moment, &mut self.last_step, &mut || world.surface(from, to));
    }

    /// What the body's animator is told this step.
    pub(super) fn body_input(&self) -> BodyInput {
        BodyInput {
            speed: self.smoothed_speed,
            direction: self.movement_direction,
            discrete_direction: self.discrete_direction,
            level: self.smoothed_pose_level,
            tilt: self.mounted_tilt_shown().unwrap_or(self.smoothed_tilt),
            inert: self.inert,
            sprint: self.sprint_flag,
            sprint_speed: self.sprint_speed,
            sprint_inertia: self.sprint_inertia,
            jumping: self.jump_flag,
            landing: self.landing_flag,
            // Falls are not simulated yet, so the animator is never told of one.
            falling: false,
            grounded: self.grounded,
            prone: self.prone,
            transition_speed: self.transition_speed(),
            aim_angle: self.aim_angle,
            delta_rotation: self.hands_to_body,
            aiming: self.aiming && !self.aim_kept_from_body,
            reloading: self.prone_reload,
            hands_busy: self.hands_busy,
            step: self.step,
            sidestep: self.sidestep,
            sideback_speed: self.sideback_speed,
            do_vault: self.do_vault,
            do_climb: self.do_climb,
            vaulting: self.is_vaulting,
            vault_figures: self.vault_figures,
            vaulting_speed: self.vaulting_speed,
            interaction: self.shown_interaction,
            inventory: self.body_inventory[0],
            inventory_operation: self.body_inventory[1],
            kick: self.kick.is_some(),
            kick_succeeds: self.kick.is_some_and(|kick| kick.opens),
            approached: self.kick.is_some_and(|kick| kick.approached),
            blind_fire: self.blind_shown,
            blind_fire_float: self.blind_float,
            prone_bipods: self.mount.is_some() && self.prone,
            mounted: self.mount.is_some(),
            lying: self.prone,
            left_stance: self.left_stance.animator,
            force_left_stance: self.left_stance.force_one_frame,
        }
    }
}
