//! Making a figure, stepping it and reading its pose and bones.

use super::*;

impl Figure {
    /// The figure as the screen makes it (`PlayerModelLoader.CreateWeapon`,
    /// `MenuPlayerPoser.ChangeWeapon`): the body told the kind of thing in
    /// hand (`kind`) and updated by nothing, the weapon's animator (where
    /// it holds one) asked for `PATROL`, told the item's state, updated by
    /// 3 s, told it is on and on patrol and asked for `PATROL` again; then
    /// held once. `seed` starts the picks' draws.
    pub fn new(data: &FigureData, kind: i32, weapon: Option<(&FigureWeaponData, Loaded)>, seed: i32) -> Self {
        let mut body = Animator::new(data.controller.clone());
        body.set_layer_weight(data.layer, 1.0);
        for name in KIND {
            body.set_float(name, kind as f32);
        }
        body.update(0.0);
        let random_count = body.float(RANDOM_COUNT) as i32;
        let held = weapon.map(|(weapon, loaded)| {
            let mut animator = WeaponAnimator::new(&weapon.animator);
            animator.play(&weapon.patrol, 0.0);
            if loaded.firearm {
                animator.set_layer_weight("Stock", if loaded.folded { 1.0 } else { 0.0 });
                animator.set_flag("StockFolded", loaded.folded);
                animator.set("AmmoInMag", loaded.magazine.unwrap_or(0) as f32);
                animator.set_flag("MagInWeapon", loaded.magazine.is_some());
                animator.set("AmmoInChamber", loaded.chamber as f32);
                if loaded.magazine.is_some() {
                    animator.set("RelTypeOld", loaded.magazine_animation as f32);
                    animator.set("RelTypeNew", loaded.magazine_animation as f32);
                }
            }
            animator.update(MAKING_UPDATE);
            // (The marks of that update reach a poser that does nothing with
            // any of them while it holds still.)
            animator.deliver();
            animator.set_flag("Active", true);
            animator.set_flag("Patrol", true);
            animator.play(&weapon.patrol, 0.0);
            animator.set_flag("CanReload", loaded.can_reload);
            let blender = matches!(weapon.grips[1], Some(GripChoice::Blend { .. })).then(|| Blender::new(0.0, BLEND_SPEED, 0.0));
            Held {
                rig: weapon.rig.clone(),
                data: weapon.clone(),
                animator,
                grip_weight: [0.0; 2],
                crutch: Blender { target: 1.0, start_value: 1.0, start_time: 0.0, speed: BLEND_SPEED },
                blender,
                blend: 0.0,
                moved: ([0.0; 3], [0.0, 0.0, 0.0, 1.0]),
                markers: [None; 2],
                targets: [None; 2],
            }
        });
        let moves = weapon.map_or_else(|| vec!["Look"], |(_, loaded)| loaded.moves());
        let mut figure = Self {
            data: data.clone(),
            pose: body.pose().to_vec(),
            body,
            kind,
            held,
            moves,
            random_count,
            dice: UnityRandom::new(seed),
            steered: VecDeque::new(),
            picks: Vec::new(),
            move_picked: None,
            patrol_delayed: false,
            ammo_to_load: -1,
            can_reload: weapon.is_some_and(|(_, loaded)| loaded.can_reload),
            heard: Vec::new(),
            age: 0.0,
        };
        figure.hold();
        figure
    }

    /// One frame `dt` seconds after the last: both animators, the body's
    /// clip events as they passed (the idle's picks, a move's start and
    /// end), the weapon's marks handed to the poser, then the hold.
    pub fn step(&mut self, dt: f32) {
        self.age += dt;
        self.heard.clear();
        self.body.update(dt);
        let events: Vec<String> = self
            .body
            .passed_events()
            .map(|event| event.function.clone())
            .filter(|function| [RANDOMIZE, OPERATION_START, OPERATION_END].contains(&function.as_str()))
            .collect();
        if let Some(held) = &mut self.held {
            held.animator.update(dt);
        }
        for event in events {
            self.hear(false, &event, "");
            match event.as_str() {
                RANDOMIZE => self.randomize(),
                OPERATION_START => self.operation_start(),
                _ => self.operation_end(),
            }
        }
        // The weapon's marks, one by one, each if its conditions hold as
        // the ones before it left them (`AnimationEventsEmitter.EmitEvents`).
        while let Some(mark) = self.held.as_mut().and_then(|held| held.animator.deliver_next()) {
            self.hear(true, &mark.name, &mark.text);
            match mark.name.as_str() {
                IDLE_START => self.idle_start(),
                name if ADD_AMMO.contains(&name) => self.add_ammo(),
                _ => {}
            }
        }
        self.hold();
    }

    pub(super) fn hear(&mut self, weapon: bool, name: &str, text: &str) {
        self.heard.push(FigureEvent { weapon, name: name.to_owned(), text: text.to_owned() });
    }

    /// What reached the poser on the last frame, in its order.
    pub fn heard(&self) -> &[FigureEvent] {
        &self.heard
    }

    /// The next picks, instead of the draws: for following a recording.
    pub fn steer(&mut self, pick: Pick) {
        self.steered.push_back(pick);
    }

    /// Every pick made so far, each with the figure's time.
    pub fn picks(&self) -> &[(f32, Pick)] {
        &self.picks
    }

    /// The weapon moves a pick chooses among, by their triggers.
    pub fn weapon_moves(&self) -> &[&'static str] {
        &self.moves
    }

    /// The kind of thing in hand it was told.
    pub fn kind(&self) -> i32 {
        self.kind
    }

    /// Seconds since it was made.
    pub fn age(&self) -> f32 {
        self.age
    }

    /// The body's animator, for looking at.
    pub fn animator(&self) -> &Animator {
        &self.body
    }

    /// The weapon's hands' state by its full name, how far through it (in
    /// cycles), and the state it is entering; none without a weapon.
    pub fn weapon_state(&self) -> Option<(&str, f32, Option<&str>)> {
        self.held.as_ref().map(|held| held.animator.state())
    }

    /// Every bone of the figure in its parent, in `FigureData::paths`'
    /// order, as the last frame left it.
    pub fn pose(&self) -> &[Placed] {
        &self.pose
    }

    /// Every bone of the weapon's rig in its parent, as the last frame left
    /// it, the rig's own root hung on the ribcage at nought; none without a
    /// weapon.
    pub fn weapon_pose(&self) -> Option<Vec<Placed>> {
        self.held.as_ref().map(|held| (0..held.rig.len()).map(|bone| held.rig.local(bone)).collect())
    }

    /// Every bone of the figure in its root's axes.
    pub fn in_root(&self) -> Vec<Placed> {
        compose(&self.data.skeleton.parents, &self.pose)
    }

    /// The bones the recorder writes, in the root's axes.
    pub fn bones(&self) -> FigureBones {
        let world = self.in_root();
        let n = &self.data.named;
        FigureBones {
            head: world[n.head],
            neck: world[n.neck],
            ribcage: world[n.ribcage],
            palms: [world[n.arms[0][5]], world[n.arms[1][5]]],
            feet: [world[n.feet[0]], world[n.feet[1]]],
            goals: [world[n.goals[0]], world[n.goals[1]]],
            weapon_3rd: world[n.weapon_3rd],
            markers: self.held.as_ref().map_or([None; 2], |held| held.markers),
            targets: self.held.as_ref().map_or([None; 2], |held| held.targets),
            weapon_root_anim: self.held.as_ref().map(|held| held.moved),
            weapon: self.held.as_ref().map(|_| world[n.ribcage]),
        }
    }
}
