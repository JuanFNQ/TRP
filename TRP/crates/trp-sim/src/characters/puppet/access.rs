//! Making a puppet, stepping it and reading its poses, recipes and bound.

use super::*;

impl Puppet {
    /// A character with the firearm `firearm` (its motion `hands`, the
    /// weapon's assembled `hierarchy`), the arms' solver's settings, the
    /// player's frame rate and the engine's physics step and longest frame.
    /// It is seen from outside, with a bot's mask (the turn-away alone).
    #[allow(clippy::too_many_arguments)]
    pub fn new(movement: Movement, hands: Hands, mut firearm: Firearm, hierarchy: &Hierarchy, limbs: [LimbData; 2], cell_width: f32, pacer: Pacer, smooth: bool) -> Result<Self> {
        // The weapon in hand from the start, as the player's run begins.
        firearm.start_drawn();
        Self::holding(movement, hands, Held::Firearm(firearm), hierarchy, limbs, cell_width, pacer, smooth)
    }

    /// A character holding nothing: the empty hands `empty` (their motion
    /// `hands`, their rig's `hierarchy`), there and at rest from the start
    /// as a character has them that has held nothing for a while
    /// (`EmptyHands::start_drawn`), the body told so at once
    /// (`Body::start_hands_kind`); the rest as `new`.
    #[allow(clippy::too_many_arguments)]
    pub fn empty_handed(mut movement: Movement, hands: Hands, mut empty: EmptyHands, hierarchy: &Hierarchy, limbs: [LimbData; 2], cell_width: f32, pacer: Pacer, smooth: bool) -> Result<Self> {
        empty.start_drawn();
        movement.body.start_hands_kind(crate::locomotion::body::hands_kind::EMPTY_HANDS);
        Self::holding(movement, hands, Held::Empty(empty), hierarchy, limbs, cell_width, pacer, smooth)
    }

    #[allow(clippy::too_many_arguments)]
    fn holding(mut movement: Movement, mut hands: Hands, held: Held, hierarchy: &Hierarchy, limbs: [LimbData; 2], cell_width: f32, pacer: Pacer, smooth: bool) -> Result<Self> {
        movement.body.set_point_of_view(PointOfView::ThirdPerson);
        movement.sounds.third_person = true;
        movement.body.set_weapon_width(cell_width);
        hands.set_point_of_view(PointOfView::ThirdPerson);
        hands.mask = effector::DRAW_DOWN;
        let rig = WeaponRig::new(hierarchy);
        let moved = rig.bone(MOVED).context("the weapon has no bone for the procedural motion")?;
        let parent = rig.bone(PARENT).context("the weapon has no root")?;
        let hand = |right: bool| -> Result<RigHand> {
            let nodes = ArmNodes::find(hierarchy, right).context("the weapon's rig lacks an arm's bones")?;
            let side = if right { 'R' } else { 'L' };
            let mut path = format!("Base Human{side}Collarbone/Base Human{side}Upperarm");
            for name in ["Forearm1", "Forearm2", "Forearm3", "Palm"] {
                path.push_str(&format!("/Base Human{side}{name}"));
            }
            let palm = rig.bone(&path).context("the weapon's rig lacks a palm")?;
            let below = |index: usize, ancestor: usize| {
                let mut at = index;
                while let Some(p) = hierarchy.nodes[at].parent {
                    if p == ancestor {
                        return true;
                    }
                    at = p;
                }
                false
            };
            let fingers = (palm + 1..hierarchy.nodes.len()).filter(|&i| below(i, palm)).collect();
            // An uncached grip keeps its finger pose as bones below it.
            let grip_pose = nodes
                .grip
                .map(|g| {
                    let stored = &hierarchy.nodes[g].grip.as_ref().expect("a grip").fingers;
                    if stored.is_empty() {
                        std::iter::once(g).chain((g + 1..hierarchy.nodes.len()).filter(|&i| below(i, g))).map(|i| hierarchy.nodes[i].rotation).collect()
                    } else {
                        stored.clone()
                    }
                })
                .unwrap_or_default();
            Ok(RigHand { nodes, fingers, grip_pose })
        };
        let hands_rig = [hand(true)?, hand(false)?];
        // The sight the weapon is aimed by, as its sights pick it.
        let sights = sights_for(&hands, hierarchy);
        Ok(Self {
            movement,
            hands,
            held,
            gear_ergonomics: 0.0,
            pacer,
            marker: rig.bone(RIGHT_MARKER),
            vest: rig.bone(LEFT_MARKER).zip(rig.bone(VEST_MARKER)).map(|(left, vest)| [left, vest]),
            sights,
            read: rig.clone(),
            parent,
            drawing: Drawing { rest: rig, moved, hands_rig, limbs, smooth },
            posing: Posing::Full,
            made: 0,
            frames: VecDeque::new(),
            hold: None,
        })
    }

    /// The host has drawn a frame `dt` seconds after the last: the frames
    /// of the character's own that are due, the walk's keys held as
    /// `move_dir` (x right, y forward), its `health` (where it has one)
    /// followed after each frame's movement.
    pub fn step(&mut self, dt: f32, move_dir: [f32; 2], world: &dyn World, health: Option<&mut Health>) {
        self.step_with(dt, &Input { move_dir, ..Input::default() }, world, health);
    }

    /// As `step`, with the movement's keys as a host's `input` has them
    /// (the stance, the lean, the sprint, the vault: a bot's or a test's):
    /// the first of the step's frames takes it whole, the rest the keys
    /// held alone (the walk, the sprint, the lean, the step aside, the
    /// vault), so a press is heard once. What the hands hold says the rest
    /// (the sights, a reload, the hands busy), as `step`.
    pub fn step_with(&mut self, dt: f32, input: &Input, world: &dyn World, mut health: Option<&mut Health>) {
        let mut asked = *input;
        for _ in 0..self.pacer.pass(dt) {
            let frame = self.pacer.frame();
            self.frame(frame.dt, frame.physics_steps, &asked, world, health.as_deref_mut());
            asked = Input { move_dir: asked.move_dir, sprint: asked.sprint, lean: asked.lean, lean_keys: asked.lean_keys, step: asked.step, vault: asked.vault, ..Input::default() };
        }
    }

    /// How its frames are posed from its next step on (`Posing`): the
    /// host's, told before each step (the app's by the character's size on
    /// screen and what may meet it). Either way every result is the same,
    /// and so is every pose asked for; from asked to full the first frame
    /// is posed at once, the frames before it from their recipes.
    pub fn posing(&mut self, posing: Posing) {
        self.posing = posing;
    }

    /// How its frames are posed (`posing`).
    pub fn how_posed(&self) -> Posing {
        self.posing
    }

    /// The skills' buffs put on the character, as the player's are as it
    /// is made (skills spec; the app's `skills::put_on_movement` and
    /// `HandsRig::take_skills`): on its movement (`Movement::take_skills`,
    /// `carry` the carrying factor), the stamina's pools settled to them,
    /// and on the weapon in its hands by its class `class` (`Buffs::
    /// weapon`): the animator's reload and draw speeds, the ergonomics, the
    /// recoil's suppression, Aim Drills' aiming speed, the stiff draw; the
    /// aiming's stamina weight from the weapon so worked out. A host puts
    /// the health's and the armour's on them itself.
    pub fn take_skills(
        &mut self,
        buffs: &Buffs,
        class: Option<Skill>,
        carry: f32,
        hard: &trp_data::setup::hard_settings::HardSettings,
        config: &serde_json::Value,
    ) -> Result<()> {
        let limited = self.movement.take_skills(buffs, carry, hard, config);
        self.movement.stamina.resettle();
        let weapon = buffs.weapon(class);
        if let Held::Firearm(firearm) = &mut self.held {
            firearm.set_skill_speeds(weapon.reload_speed, weapon.swap_speed);
        }
        self.hands.set_skills(buffs.aim_speed, weapon.ergonomics, weapon.recoil);
        self.hands.set_stiff_draw(weapon.stiff_draw);
        self.movement.stamina.aim_mass = self.hands.settings.ergonomic_weight();
        limited
    }

    /// What its gear does to it, as the player's (`Player.
    /// RecalculateEquipmentParams`, called as it is set up and whenever what
    /// it wears changes): the armour's and the backpack's speed penalty, in
    /// percent (`armor::speed_penalty`, its vest skills' shares taken off),
    /// and their share of the weapon's ergonomics.
    pub fn carry_gear(&mut self, speed_penalty: f32, gear_ergonomics: f32) {
        self.movement.set_armour_penalty(speed_penalty);
        self.gear_ergonomics = gear_ergonomics;
        self.hands.set_load(self.movement.stamina.settings.overweight, gear_ergonomics);
    }

    /// What its last frame left: its newest pose, which whatever goes by
    /// where it is (its hit colliders, a ragdoll taking over) goes by;
    /// made now if it was not.
    pub fn pose(&self) -> Option<&PuppetPose> {
        self.frames.back().map(|kept| self.made(kept))
    }

    /// The pose of its frame numbered `frame` (`Recipe::frame`), while that
    /// frame is kept; made now if it was not, and kept.
    pub fn pose_of_frame(&self, frame: u64) -> Option<&PuppetPose> {
        self.kept(frame).map(|kept| self.made(kept))
    }

    /// Whether the pose of its frame `frame` has been made (at once, or
    /// asked for since).
    pub fn is_posed(&self, frame: u64) -> bool {
        self.kept(frame).is_some_and(|kept| kept.pose.get().is_some())
    }

    /// The number of its newest frame.
    pub fn newest_frame(&self) -> Option<u64> {
        self.frames.back().map(|kept| kept.recipe.frame)
    }

    /// What its newest frame left for its pose.
    pub fn recipe(&self) -> Option<&Recipe> {
        self.frames.back().map(|kept| &kept.recipe)
    }

    /// What its frame `frame` left for its pose, while it is kept.
    pub fn recipe_of_frame(&self, frame: u64) -> Option<&Recipe> {
        self.kept(frame).map(|kept| &kept.recipe)
    }

    /// The capsule that holds its hit colliders on its newest frame
    /// (`Bound`), had without a pose.
    pub fn bound(&self) -> Option<Bound> {
        self.recipe().map(|recipe| recipe.bound)
    }

    pub(super) fn kept(&self, frame: u64) -> Option<&Kept> {
        let oldest = self.frames.front()?.recipe.frame;
        self.frames.get(usize::try_from(frame.checked_sub(oldest)?).ok()?)
    }

    /// A kept frame's pose: as made, or made now from its recipe.
    pub(super) fn made<'a>(&'a self, kept: &'a Kept) -> &'a PuppetPose {
        kept.pose.get_or_init(|| {
            let (body, weapon) = kept.recipe.mixes.as_ref().expect("a frame left unposed keeps its mixes");
            let animator = self.held.animator();
            let bones = self.movement.body.drawn_bones_turned(body, kept.recipe.head_turn);
            self.drawing.make(&kept.recipe, &animator.pose_of(weapon), &animator.drawn_of(weapon), bones)
        })
    }

    /// What is drawn of it now: its pose at the moment drawn, one physics
    /// step behind the host's time, between the two of its frames either
    /// side of that moment (`Pacer::frames_behind`), as the app draws the
    /// player's body, rig and arms (`Controller::around_drawn`; rule 8's
    /// one liberty). Its frames are untouched: only what is drawn lies
    /// between them. Where the moment drawn lies on a frame, or before the
    /// oldest kept (a character just made, or after a `cut`), that frame
    /// as it was left. The two frames' poses are made now where they were
    /// not.
    pub fn drawn(&self) -> Option<PuppetPose> {
        let last = self.frames.len().checked_sub(1)?;
        let behind = self.pacer.frames_behind();
        let newer = last.saturating_sub(behind.floor() as usize);
        let older = last.saturating_sub(behind.ceil() as usize);
        let newest = self.made(&self.frames[newer]);
        if older == newer {
            return Some(newest.clone());
        }
        Some(self.made(&self.frames[older]).blend(newest, 1.0 - behind.fract()))
    }

    /// The oldest of its frames the host may still ask for (a host that
    /// reads the frame before, as the app's knife and blasts do: `pose_of_
    /// frame`, `recipe_of_frame`), kept however many frames its next steps
    /// make (a slow draw makes many a step), until the host says another or
    /// none. The frames kept for drawing between frames are kept either way.
    pub fn keep_from(&mut self, frame: Option<u64>) {
        self.hold = frame;
    }

    /// Its newest frame's pose as the animator's straight pose has it,
    /// whatever is drawn: the rig as given out, the arms solved on it and the
    /// fingers closed on the grip by them, the body's bones as `pose` has
    /// them (never smoothed). What a ragdoll taking over starts from (the
    /// user's decision, the 14e point: a drawing option moves no result);
    /// `pose` itself where the poses are not drawn smooth.
    pub fn straight_pose(&self) -> Option<PuppetPose> {
        let kept = self.frames.back()?;
        let pose = self.made(kept);
        if !self.drawing.smooth {
            return Some(pose.clone());
        }
        let animator = self.held.animator();
        // A frame posed when asked keeps its mixes; one posed in full was
        // the animator's last update, which nothing has moved since.
        let straight = match &kept.recipe.mixes {
            Some((_, weapon)) => animator.pose_of(weapon),
            None => animator.pose().to_vec(),
        };
        Some(self.drawing.make(&kept.recipe, &straight, &straight, pose.bones.clone()))
    }

    /// A cut: every frame before its newest forgotten, so that nothing is
    /// drawn between them and what comes after (its death, where the host's
    /// ragdoll takes over from its newest frame; a move to another place).
    /// What is drawn is then its newest frame until the moment drawn passes
    /// it.
    pub fn cut(&mut self) {
        while self.frames.len() > 1 {
            self.frames.pop_front();
        }
    }

    /// The firearm in its hands, where it holds one.
    pub fn firearm(&self) -> Option<&Firearm> {
        match &self.held {
            Held::Firearm(firearm) => Some(firearm),
            Held::Empty(_) => None,
        }
    }

    /// The firearm in its hands, to work it (a bot's reload, a test's).
    pub fn firearm_mut(&mut self) -> Option<&mut Firearm> {
        match &mut self.held {
            Held::Firearm(firearm) => Some(firearm),
            Held::Empty(_) => None,
        }
    }
}
