//! The body's update and what it reports: motion, turns, states and progress.

use super::*;

impl Body {
    /// Advances by `dt` seconds. Returns the states the animator entered
    /// that name a movement state, in the order it entered them.
    pub fn update(&mut self, dt: f32, input: &BodyInput) -> Vec<Entered> {
        let ids = self.ids;
        self.mounted = [input.mounted, input.lying];
        self.kind_now = if (self.kind_now - self.kind).abs() > hands_kind::NEAR {
            self.kind_now + (self.kind - self.kind_now) * (dt * hands_kind::RATE).clamp(0.0, 1.0)
        } else {
            self.kind
        };
        if let Some(id) = ids.weapon_type_float {
            self.animator.set(id, self.kind_now);
        }
        let a = &mut self.animator;
        let mut set = |id: Option<usize>, value: f32| {
            if let Some(id) = id {
                a.set(id, value);
            }
        };
        let flag = |b: bool| b as u8 as f32;
        set(ids.speed, input.speed);
        set(ids.direct_x, input.direction[0]);
        set(ids.direct_y, input.direction[1]);
        set(ids.direct, input.discrete_direction);
        set(ids.level, input.level);
        set(ids.tilt, input.tilt);
        set(ids.inert, flag(input.inert));
        set(ids.inert_float, flag(input.inert));
        set(ids.sprint, flag(input.sprint));
        set(ids.sprint_speed, input.sprint_speed);
        set(ids.sprint_inertia, input.sprint_inertia);
        set(ids.jumping, flag(input.jumping));
        set(ids.landing, flag(input.landing));
        set(ids.falling, flag(input.falling));
        set(ids.grounded, flag(input.grounded));
        set(ids.prone, flag(input.prone));
        set(ids.transition_speed, input.transition_speed);
        set(ids.aim_angle, input.aim_angle);
        set(ids.delta_rotation, input.delta_rotation);
        set(ids.is_aiming, flag(input.aiming));
        set(ids.prone_reload, flag(input.reloading));
        set(ids.reload_float, flag(input.hands_busy));
        set(ids.step, input.step as f32);
        set(ids.sidestep, input.sidestep);
        set(ids.sideback_speed, input.sideback_speed);
        set(ids.do_vault, flag(input.do_vault));
        set(ids.do_climb, flag(input.do_climb));
        set(ids.is_vaulting, flag(input.vaulting));
        set(ids.vaulting_height, input.vault_figures[0]);
        set(ids.vaulting_length, input.vault_figures[1]);
        set(ids.over_obstacle, input.vault_figures[2]);
        set(ids.forward_velocity, input.vault_figures[3]);
        set(ids.vaulting_speed, input.vaulting_speed);
        set(ids.is_interaction, flag(input.interaction.is_some()));
        if let Some(index) = input.interaction {
            set(ids.interaction_index, index as f32);
        }
        set(ids.inventory, flag(input.inventory));
        set(ids.inventory_operation, flag(input.inventory_operation));
        set(ids.kick, flag(input.kick));
        set(ids.kick_succeed, flag(input.kick_succeeds));
        set(ids.approached, flag(input.approached));
        set(ids.blind_fire, input.blind_fire);
        set(ids.blind_fire_float, input.blind_fire_float);
        set(ids.prone_bipods, flag(input.prone_bipods));
        set(ids.left_stance, flag(input.left_stance));
        set(ids.force_left_stance, flag(input.force_left_stance));
        // The game fades one layer by a curve of the clips playing.
        let fade = ids.aiming_layer.map_or(0.0, |id| a.get(id));
        a.set_layer_weight(self.faded_layer, 1.0 - fade);

        let events = self.animator.update(dt);
        self.crawl_marks += self.animator.passed_events().filter(|event| event.function == "PlaySoundBank" && event.data == "Prone").count();
        // A gesture is over when its state has been left: the state's own
        // script says so in the game. The state's name begins with the
        // number its way in asks for.
        if let Some((_, interactions)) = self.interactions {
            self.finished.extend(events.iter().filter_map(|event| match event {
                Event::Left { machine, state } if *machine == interactions => {
                    let name = self.animator.state_def(*machine, *state).name.as_str();
                    name.rsplit('.').next()?.split(' ').next()?.parse::<u8>().ok()
                }
                _ => None,
            }));
        }
        // The backpack's drop has a mark of its own in its state, which
        // comes once as the state plays past it, entering or entered.
        if let (Some((layer, _)), Some(at)) = (self.interactions, self.backpack_drop) {
            let (current, next) = self.animator.state(layer);
            let time = if current.name == BACKPACK_DROP {
                Some(self.animator.progress(layer).0)
            } else if next.is_some_and(|next| next.name == BACKPACK_DROP) {
                self.animator.next_progress(layer)
            } else {
                None
            };
            match time {
                Some(time) if time >= at && !self.backpack_passed => (self.backpack_passed, self.backpack_dropped) = (true, true),
                Some(_) => {}
                None => self.backpack_passed = false,
            }
        }
        // So does each of the bipod's gestures its toggle's mark.
        if let Some((layer, _)) = self.interactions {
            let (current, next) = self.animator.state(layer);
            let found = BIPOD_STATES.iter().zip(self.bipod_marks).find_map(|(name, at)| {
                let at = at?;
                if current.name == *name {
                    Some((self.animator.progress(layer).0, at))
                } else if next.is_some_and(|next| next.name == *name) {
                    self.animator.next_progress(layer).map(|time| (time, at))
                } else {
                    None
                }
            });
            match found {
                Some((time, at)) if time >= at && !self.bipod_passed => (self.bipod_passed, self.bipod_toggled) = (true, true),
                Some(_) => {}
                None => self.bipod_passed = false,
            }
        }
        // The marks of a move over an obstacle come as its state plays
        // past each, entering or entered, once each while it plays.
        {
            let (current, next) = self.animator.state(0);
            for (kind, marks) in self.vault_marks.iter().enumerate() {
                let time = if current.name == VAULT_STATES[kind] {
                    Some(self.animator.progress(0).0)
                } else if next.is_some_and(|next| next.name == VAULT_STATES[kind]) {
                    self.animator.next_progress(0)
                } else {
                    None
                };
                let Some(time) = time else {
                    self.vault_passed[kind] = [false; 4];
                    continue;
                };
                for (mark, at) in marks.iter().enumerate() {
                    if at.is_some_and(|at| time >= at) && !self.vault_passed[kind][mark] {
                        self.vault_passed[kind][mark] = true;
                        self.vault_due.push((kind, mark));
                    }
                }
            }
        }
        let named = |machine: &usize, state: &usize| {
            (*machine == self.base_machine)
                .then(|| self.animator.state_def(*machine, *state).movement.as_ref().map(|m| m.state))
                .flatten()
        };
        self.left = events
            .iter()
            .filter_map(|event| match event {
                Event::Left { machine, state } => named(machine, state),
                _ => None,
            })
            .collect();
        let mut entered = Vec::new();
        for event in &events {
            let Event::Entered { machine, state } = event else { continue };
            if *machine != self.base_machine {
                continue;
            }
            let Some(script) = self.animator.state_def(*machine, *state).movement.as_ref() else { continue };
            // The game numbers the directions; only the two sides are used
            // by what is built.
            let side = match script.direction {
                1 => 1.0,
                5 => -1.0,
                _ => 0.0,
            };
            entered.push(Entered {
                state: script.state,
                part: script.kind,
                side,
                turn_factor: script.sensitivity,
                turn_limit: script.rotation_speed_clamp,
                can_interact: script.can_interact,
            });
            // Every such state says what first-person pose it has, or
            // that it has none.
            self.pose_changes.push(script.first_person_pose.clone());
        }
        entered
    }

    /// The first-person pose of each scripted state entered since this was
    /// last asked, in order: `None` for a state that has none. For
    /// `Hands::assign_pose`.
    pub fn take_poses(&mut self) -> Vec<Option<FirstPersonPose>> {
        std::mem::take(&mut self.pose_changes)
    }

    /// The length the game's sidestep state runs its parameter over, in
    /// seconds: the one its ways in and out are given (movement spec, "The
    /// sidestep").
    pub fn sidestep_length(&self) -> f32 {
        self.sidestep_length
    }

    /// How many of the base layer's states that name this movement state
    /// had a part in the last update: the current one, one being blended
    /// into, and any the update left. The game's animator calls on the
    /// movement state once for each, so in a frame where the body goes
    /// from standing into a turn on the spot the standing state runs twice.
    pub fn calls(&self, movement: i32) -> usize {
        let (current, next) = self.animator.state(0);
        let playing = [Some(current), next].into_iter().flatten().filter_map(|s| s.movement.as_ref().map(|m| m.state));
        playing.chain(self.left.iter().copied()).filter(|named| *named == movement).count()
    }

    /// How far the animation moved the body in the last update, in the
    /// body's own axes (x right, y up, z ahead).
    pub fn motion(&self) -> [f32; 3] {
        self.animator.root_motion()
    }

    /// How far the animation turned the body in the last update, in
    /// degrees to the right. Only the turns on the spot do.
    pub fn turn(&self) -> f32 {
        self.animator.root_turn()
    }

    /// The root bone's own rotation in the pose.
    pub fn root_rotation(&self) -> [f32; 4] {
        self.animator.bone(0).1
    }

    /// The clips' `Mouse_look` curve: how far a vault or a climb has
    /// taken the view to its own limit, 0 to 1.
    pub fn mouse_look(&self) -> f32 {
        self.ids.mouse_look.map_or(0.0, |id| self.animator.get(id))
    }

    /// Whether the base layer is between two states.
    pub fn in_transition(&self) -> bool {
        self.animator.state(0).1.is_some()
    }

    /// The footstep curve: its sign flips at each foot plant.
    pub fn foot_step(&self) -> f32 {
        self.ids.foot_step.map_or(0.0, |id| self.animator.get(id))
    }

    /// The base layer's state, and the one it is blending into if any.
    pub fn state_names(&self) -> (&str, Option<&str>) {
        let (current, next) = self.animator.state(0);
        (current.name.as_str(), next.map(|s| s.name.as_str()))
    }

    /// How far the base layer's current state has played, in cycles, and
    /// how far its transition has run, 0 to 1.
    pub fn progress(&self) -> (f32, f32) {
        self.animator.progress(0)
    }

    /// How far the base layer's state that names a movement state has
    /// played, in cycles: the one being entered if that names it, else the
    /// current one.
    pub fn time_in(&self, movement: i32) -> Option<f32> {
        let (current, next) = self.animator.state(0);
        let names = |state: &crate::base::animator::StateDef| state.movement.as_ref().is_some_and(|m| m.state == movement);
        if next.is_some_and(names) {
            self.animator.next_progress(0)
        } else {
            names(current).then(|| self.animator.progress(0).0)
        }
    }

    /// The animator's layers as they stand, one line each, and the curves
    /// that decide who carries the weapon.
    pub fn describe(&self) -> Vec<String> {
        let curve = |id: Option<usize>| id.map_or(f32::NAN, |id| self.animator.get(id));
        let mut lines = self.animator.describe();
        lines.push(format!(
            "Weapon_3rd {:.3}, First_Person_Curve_Weight {:.3}, Hand_Right {:.3}, Aiming_Layer {:.3}",
            curve(self.ids.weapon_3rd),
            curve(self.ids.first_person),
            curve(self.ids.hand_right),
            curve(self.ids.aiming_layer)
        ));
        lines
    }

    /// Starts each layer but the base one where a recording has the
    /// game's (recorder 0.3.0 on, `layers`: four numbers a layer, the hash
    /// of its state's own name, how far through it, its weight, and the
    /// state it is blending into or -1): for a run that takes up where the
    /// recording's began. A layer is taken up only if it was in no
    /// transition on the first frame and the state is one of its own; it
    /// is put a frame back by what it played over the next frame, so that
    /// the first update lands it on the recorded figure. A layer that
    /// shares another's machine is left to that one. Returns how many
    /// were taken up.
    pub fn start_layers_as(&mut self, first: &[f32], second: Option<&[f32]>) -> usize {
        let layers = (first.len() / 4).min(self.animator.layer_count());
        let mut taken = 0;
        for layer in 1..layers {
            if !self.animator.owns_machine(layer) {
                continue;
            }
            let [hash, time, _, next] = [0, 1, 2, 3].map(|k| first[layer * 4 + k]);
            if next >= 0.0 {
                continue;
            }
            let Some(state) = self.animator.state_by_short_hash(layer, hash as u32) else { continue };
            let played = second
                .filter(|later| later.len() >= layer * 4 + 4 && later[layer * 4] == hash && later[layer * 4 + 3] < 0.0)
                .map_or(0.0, |later| later[layer * 4 + 1] - time);
            self.animator.start_in(layer, state, time - played);
            taken += 1;
        }
        taken
    }

    /// Puts the base layer's current state at a point in its play, in
    /// cycles: a body that has stood a while is somewhere in its idle.
    pub fn set_progress(&mut self, cycles: f32) {
        self.animator.set_progress(0, cycles);
    }
}
