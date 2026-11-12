//! Making one frame of the puppet and the rig bones it follows.

use super::*;

impl Puppet {
    /// One frame, as the app's controller runs the player's
    /// (`controller::step`) seen from outside: with empty hands, as its
    /// frame runs them (their controller stepped on the frame's time with
    /// the view's pitch, nothing aimed, reloaded or fired). The simulation's
    /// half, then the recipe kept and, posed in full, the pose made from it.
    pub(super) fn frame(&mut self, dt: f32, physics_steps: u32, asked: &Input, world: &dyn World, health: Option<&mut Health>) {
        // Posed when asked, the animators leave the bones nothing here goes
        // by to the recipe's mixes (put on each frame, as smoothing is).
        let deferred = self.posing == Posing::Asked;
        self.movement.body.set_deferred_posing(deferred);
        let held = &mut self.held;
        match held {
            Held::Firearm(firearm) => firearm.set_deferred_posing(deferred),
            Held::Empty(empty) => empty.set_deferred_posing(deferred),
        }
        let input = match held {
            Held::Firearm(firearm) => Input {
                aiming: firearm.aiming(),
                aim_kept_from_body: !firearm.tells_body_of_aim(),
                reloading: firearm.reloading(),
                hands_busy: firearm.hands_busy(),
                hands_idle: firearm.animator_idling(),
                hands_in_inventory: firearm.in_inventory(),
                weapon_occupied: firearm.occupied(),
                ..*asked
            },
            Held::Empty(empty) => Input {
                aiming: false,
                aim_kept_from_body: false,
                reloading: false,
                hands_busy: empty.hands_busy(),
                hands_idle: empty.animator_idling(),
                hands_in_inventory: false,
                weapon_occupied: false,
                ..*asked
            },
        };
        self.movement.frame(dt, &input, world);
        // Then health, which takes what the movement did and tells it what
        // the character may do, on its whole-millisecond clock, as the
        // controller runs the player's (`Health::follow`).
        if let Some(health) = health {
            let player_dt = self.movement.player_dt();
            health.follow(player_dt, &mut self.movement);
        }
        let m = &mut self.movement;
        let hands = &mut self.hands;
        let (sprint_started, aim_lowered) = (m.take_sprint_started(), m.take_aim_lowered());
        hands.weapon_flip_speed = m.settings.weapon_flip_speed;
        hands.set_systems(&m.systems);
        // The load and the gear's share of the ergonomics, then what health
        // does to the weapon and its motion: an arm's damage works the
        // weapon out again (and aiming's stamina weight with it) and puts
        // the animator's skill speeds to 1, as the player's.
        hands.set_load(m.stamina.settings.overweight, self.gear_ergonomics);
        if hands.set_health(m.conditions) {
            m.stamina.aim_mass = hands.settings.ergonomic_weight();
        }
        if let Held::Firearm(firearm) = held {
            firearm.set_arms_damaged(m.conditions.left_arm, m.conditions.right_arm);
        }
        for pose in m.take_poses() {
            hands.assign_pose(pose.as_ref());
        }
        // (The aim taken asked only where the fall did not say so.)
        let aim_dropped = m.long_fall() || m.take_aim_taken();
        if let Held::Firearm(firearm) = held {
            if sprint_started {
                firearm.sprint_started();
            }
            if aim_lowered {
                firearm.lower_aim();
            }
            if aim_dropped {
                firearm.set_aim(false, m.can_aim());
            }
        }
        let aiming = held.aiming();
        // The weapon's motion takes the aim on its frame (`Hands::aim_changed`).
        hands.aim_changed(aiming);
        hands.set_head_rotation(m.head_look.rotation(), m.head_look.control(), aiming);
        // The bone the moved bone hangs from, as the animator left it on
        // the frame before.
        hands.weapon_root = self.read.local(self.parent);
        for _ in 0..physics_steps {
            let footstep = m.take_footstep();
            let walk_stop = m.take_walk_stop();
            hands.advance(
                self.pacer.physics_step(),
                &HandsInput {
                    yaw: m.yaw,
                    pitch: m.shown_pitch(),
                    motion: m.input_velocity(),
                    velocity: m.velocity,
                    speed: m.smoothed_speed,
                    walking: m.state == State::Move,
                    walk_stop,
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
                    frame_dt: dt,
                    wind: m.wind(),
                },
            );
        }
        hands.turn_rig(m.yaw, m.shown_pitch());
        match held {
            // The weapon's logic: no trigger, its animator on the engine's
            // clock.
            Held::Firearm(firearm) => {
                firearm.set_aim_angle(m.pitch);
                firearm.set_drawn_smooth(self.drawing.smooth);
                for event in firearm.step_on_clock(dt, m.player_dt()) {
                    match event {
                        FirearmEvent::Shot { strength, .. } => hands.shoot(strength),
                        FirearmEvent::AutoFire(on) => hands.set_auto_fire(on),
                        FirearmEvent::BodyAction(action) => m.body.set_weapon_action(action),
                        FirearmEvent::BodyDrawSpeed(speed) => m.body.set_draw_speed(speed),
                        _ => {}
                    }
                }
            }
            // The empty hands' own controller, on the frame's time, told the
            // view's pitch (as the controller steps the player's).
            Held::Empty(empty) => {
                empty.set_drawn_smooth(self.drawing.smooth);
                for event in empty.step(dt, m.pitch) {
                    if let FirearmEvent::BodyAction(action) = event {
                        m.body.set_weapon_action(action);
                    }
                }
            }
        }
        // The rig as the animator has it, read with the moved bone at rest:
        // the bones it is read by are worked out at each update, posed in
        // full or not (`followed_rig_bones`).
        self.read.pose(held.animator().pose(), Some(self.drawing.moved));
        let parent = self.read.local(self.parent);
        hands.weapon_root = parent;
        let (moved_at, moved_turn) = self.read.placed(self.drawing.moved);
        let back = [-moved_turn[0], -moved_turn[1], -moved_turn[2], moved_turn[3]];
        let within = |rig: &WeaponRig, bone: usize| {
            let (at, turn) = rig.placed(bone);
            (quat_rotate(back, std::array::from_fn(|k| at[k] - moved_at[k])), quat_mul(back, turn))
        };
        let marker = self.marker.map_or(([0.0; 3], [0.0, 0.0, 0.0, 1.0]), |marker| within(&self.read, marker));
        // Seen from outside the body reaches for the vest during a reload
        // (none here) as the left hand's marker nears the weapon's.
        let reach = self.vest.map(|[left, vest]| {
            let (left, vest) = (self.read.placed(left).0, self.read.placed(vest).0);
            (0..3).map(|k| (left[k] - vest[k]).powi(2)).sum::<f32>().sqrt()
        });
        let reaching = matches!(held, Held::Firearm(firearm) if firearm.reloading()) && !m.sprinting;
        m.body.weigh_reach_for_vest(reach.is_some(), reach.filter(|_| reaching), dt);
        if let Some(sights) = &self.sights {
            let muzzle = sights.in_moved(&self.read).and_then(|(_, muzzle)| muzzle);
            hands.note_sights(sights.line(&self.read), muzzle);
        }
        // On the player's clock, as the game's weapon motion runs.
        hands.blend_poses(m.player_dt());
        let in_view = m.drawn_body_in_view();
        let weapon = hands.weapon_transform_carried(parent, m.body.carried(in_view), m.body.held(in_view), marker);
        hands.body_camera = Some(m.body.camera_bone());
        hands.frame_outside(dt);
        let elbows = [hands.elbow_shift(true), hands.elbow_shift(false)];
        // Last, how deep the weapon would stick into something.
        // Another character's capsule among what it meets (`OverlapLn`).
        let (depth, with_character) = m.placed_weapon_overlap_met(world, hands.weapon_line(), [0.0; 3]);
        hands.set_overlap_met(depth, with_character);
        let run_down = hands.turn_away_value();
        if let Held::Firearm(firearm) = held {
            firearm.overlap(depth, run_down, m.can_aim());
        }
        hands.aim_changed_after_frame(held.aiming(), dt);

        // What the pose is made from: the moved bone where the weapon's
        // motion and the body put it, the body's arms and the elbows' shift
        // for the arms' solver, where the body and the view stand, the bound.
        let at = m.drawn_position();
        let body_at = [at[0], at[1] + m.body_height_offset, at[2]];
        let body_turn = quat_mul(quat_axis(1, m.drawn_body_yaw()), m.body_tilt);
        let ribcage = quat_rotate(quat_axis(1, m.drawn_body_yaw()), m.ribcage());
        let in_world = |p: [f32; 3]| {
            let turned = quat_rotate(body_turn, p);
            std::array::from_fn(|k| body_at[k] + turned[k])
        };
        let bound = Bound { ends: [in_world(m.body.pelvis()), in_world(m.body.ribcage())], radius: m.body.reach().into_iter().fold(0.0, f32::max) + BOUND_MARGIN };
        let recipe = Recipe {
            frame: self.made,
            position: at,
            yaw: m.yaw,
            pitch: m.shown_pitch(),
            body_at,
            body_turn,
            ribcage: quat_rotate(quat_axis(1, -m.yaw), ribcage),
            bound,
            mixes: deferred.then(|| (m.body.posing_mix().clone(), held.animator().posing())),
            head_turn: m.body.head_turn(),
            weapon,
            arms: [m.body.arm_pose(true, in_view), m.body.arm_pose(false, in_view)],
            elbows,
        };
        self.made += 1;
        // Posed in full, the pose at once from the animators as their update
        // left them.
        let pose = OnceLock::new();
        if !deferred {
            let animator = held.animator();
            let _ = pose.set(self.drawing.make(&recipe, animator.pose(), &animator.drawn_pose(), m.body.drawn_bones()));
        }
        self.frames.push_back(Kept { recipe, pose });
        let kept = (self.pacer.physics_step() / self.pacer.frame_length()).ceil() as usize + 2;
        while self.frames.len() > kept && self.frames.front().is_some_and(|oldest| self.hold.is_none_or(|hold| oldest.recipe.frame < hold)) {
            self.frames.pop_front();
        }
    }
}

/// The bones of a rig in its hands that the character's simulation reads
/// of its animator's pose (`frame`, the rig as read): the weapon's root
/// that the moved bone hangs from, the moved bone, the right hand's marker,
/// the left hand's and the vest's, and the sight it is aimed by (`sights`:
/// the weapon's own bone, the aim bone, the iron sights' two points and the
/// muzzle; `sights_for`), each with every bone above it. The arms, the
/// fingers and the rest of the rig are drawing alone
/// (`WeaponAnimatorData::following`).
pub fn followed_rig_bones(hierarchy: &Hierarchy, sights: Option<&SightNodes>) -> Vec<bool> {
    let rig = WeaponRig::new(hierarchy);
    let mut followed = vec![false; hierarchy.nodes.len()];
    let named = [PARENT, MOVED, RIGHT_MARKER, LEFT_MARKER, VEST_MARKER].into_iter().filter_map(|path| rig.bone(path));
    let seen = sights.into_iter().flat_map(|s| [Some(s.weapon), Some(s.bone), s.line.map(|l| l[0]), s.line.map(|l| l[1]), s.muzzle]).flatten();
    for bone in named.chain(seen) {
        let mut at = Some(bone);
        while let Some(index) = at.filter(|index| *index < followed.len()) {
            followed[index] = true;
            at = hierarchy.nodes[index].parent;
        }
    }
    followed
}

/// Unity's `Quaternion.Lerp` (normalised, the shorter way).
pub(super) fn nlerp(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    let dot = a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3];
    let s = if dot < 0.0 { -1.0 } else { 1.0 };
    let q: [f32; 4] = std::array::from_fn(|k| a[k] * (1.0 - t) + b[k] * s * t);
    let n = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
    if n > 0.0 { q.map(|x| x / n) } else { a }
}
