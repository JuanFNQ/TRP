//! The controller made, and what it shows of the rig and the body.

use super::*;

impl Controller {
    pub fn new(movement: Movement, pacer: Pacer) -> Self {
        let camera = CameraPose { position: [0.0; 3], rotation: NO_TURN, field_of_view: 50.0 };
        let sprung = Sprung { weapon: ([0.0; 3], NO_TURN), camera, elbows: [[0.0; 3]; 2], shift: None };
        let stepped = VecDeque::from([Stepped::of(&movement)]);
        let mut controller = Self {
            feet_before_move: movement.position,
            movement,
            health: None,
            paced: Paced::default(),
            pending: Pending::default(),
            pacer,
            before: sprung,
            now: sprung,
            stepped,
            cut: false,
            looks: VecDeque::new(),
            ribcage_shown: Vec3::ZERO,
            ribcage_drawn: ([0.0; 3], [0.0; 2], [0.0; 2]),
            aim_key: false,
            trigger: false,
            vault_key: false,
            move_keys: [0.0; 2],
            slow_lean: 0.0,
            overlap: 0.0,
            overlap_with_character: false,
            camera_box: None,
            gear_ergonomics: 0.0,
            inventory_open: false,
            edit_build: false,
            on_debug_button: false,
            gestures_open: false,
            gestures_list: false,
            phrase_situations: Vec::new(),
            armed: false,
            rig_pose: RigPose::default(),
            launcher_bones: false,
            hands_events: Vec::new(),
            wear_events: Vec::new(),
            hands_sounds: Vec::new(),
            shot_calls: Vec::new(),
            case_calls: Vec::new(),
            rounds_shown: Vec::new(),
            cases_left: Vec::new(),
            bullet_calls: Vec::new(),
            aims_up: 0,
            mount_sounds: 0,
            mount_refusals: 0,
            mount_sound_in: None,
            bipod_grounds: 0,
            bipod_marks: Vec::new(),
            mount_forward: false,
            items_said: Vec::new(),
            hands_change: None,
            bled: Vec::new(),
            bleeding_drops: Vec::new(),
            voice_heard: None,
            rounds_in_reach: 0,
            rounds_taken: 0,
            launcher_rounds_in_reach: 0,
            launcher_round_taken: false,
            launcher_fired: false,
            launcher_recoil: 1.0,
            launcher_shot_calls: Vec::new(),
            launcher_bullets: Vec::new(),
            launcher_sounds: Vec::new(),
            magazine_unknown: false,
            reload_refused_unknown: false,
            rounds_ejected: 0,
            rounds_racked: 0,
            rounds_unloaded: 0,
            rounds_dropped: Vec::new(),
            next_magazine: None,
            picked: None,
            picker_took_reload: false,
            magazine_asked: None,
            swaps: Vec::new(),
            magazines_hidden: 0,
            magazine_drawn: None,
            happened: Vec::new(),
            magazine_checked: false,
            chamber_checked: false,
            magazine_looked: false,
            panel_calls: Vec::new(),
            grenade_thrown: false,
            meds_started: 0,
            magazines_moved: 0,
            chamber_by_hand: false,
            folds_begun: 0,
            shots_made: 0,
            shots_fired: 0,
            launcher_shots_fired: 0,
            chamber_given_up: false,
            toggle_sounds: Vec::new(),
            switch_sounds: Default::default(),
            throws: Vec::new(),
            grenade_release: None,
            blade_left: None,
            stroke_world: Default::default(),
            blows: Vec::new(),
            knife_hits: Vec::new(),
            health_pulls: [0, 0],
            fast_slot: false,
            aim_kept: false,
            menu_shown: false,
            menu_notches: 0,
            zoom_notches: 0,
        };
        controller.settle_shown();
        controller
    }

    /// The view as it was shown at a moment of the host's time.
    fn look_at(&self, time: f32) -> [f32; 2] {
        let Some(&(first, mut look)) = self.looks.front() else { return self.look().into() };
        let mut before = first;
        for &(at, next) in &self.looks {
            if at >= time {
                let t = if at > before { ((time - before) / (at - before)).clamp(0.0, 1.0) } else { 1.0 };
                return [look[0] + (next[0] - look[0]) * t, look[1] + (next[1] - look[1]) * t];
            }
            (before, look) = (at, next);
        }
        look
    }

    /// Works out where the ribcage is drawn for the view as shown now.
    /// Once every drawn frame, after the simulation's frames.
    ///
    /// The frames the body is drawn from are a physics step old and more,
    /// and each was posed for the view as it stood a frame before it. The
    /// view is shown as it stands now. Drawn as those frames have it, the
    /// ribcage trailed the view by some 35 ms where the game's trails it
    /// by one frame: the eye, which hangs on it, kept moving that long
    /// after the view had stopped, by 2 cm after a turn at 300 degrees a
    /// second with the view 80 degrees down, and everything below the eye
    /// slid across the picture with it. So the ribcage is drawn where
    /// those frames have it, moved by what the simulation's body makes of
    /// the difference between the view they were posed for and the view
    /// shown one of the simulation's frames ago: a frame behind the view,
    /// as in the game, and at rest a frame after the view is.
    pub(super) fn settle_shown(&mut self) {
        let now = self.pacer.host_time();
        let look: [f32; 2] = self.look().into();
        self.looks.push_back((now, look));
        while self.looks.len() > 2 && self.looks[1].0 < now - 0.1 {
            self.looks.pop_front();
        }
        let trailing = self.look_at(now - self.pacer.frame_length());

        let (older, newer, t) = self.around_drawn();
        let mut ribcage: [f32; 3] = std::array::from_fn(|k| older.ribcage[k] + (newer.ribcage[k] - older.ribcage[k]) * t);
        let posed = [
            older.posed_for[0] + delta_angle(older.posed_for[0], newer.posed_for[0]) * t,
            older.posed_for[1] + (newer.posed_for[1] - older.posed_for[1]) * t,
        ];
        // A view that has stood still that long changes nothing, and the
        // pose need not be worked out again.
        const SAME: f32 = 1e-3;
        if delta_angle(posed[0], trailing[0]).abs() > SAME || (trailing[1] - posed[1]).abs() > SAME {
            let m = &self.movement;
            let (from, to) = (m.ribcage_posed_for(posed), m.ribcage_posed_for(trailing));
            let change = quat_rotate(quat_axis(1, m.drawn_body_yaw()), std::array::from_fn(|k| to[k] - from[k]));
            ribcage = std::array::from_fn(|k| ribcage[k] + change[k]);
        }
        // Into the frame turned with the view as shown; the simulation's
        // forward is +z, Bevy's -z.
        let seen = quat_rotate(quat_axis(1, -look[0]), ribcage);
        self.ribcage_shown = Vec3::new(seen[0], seen[1], -seen[2]);
        self.ribcage_drawn = (ribcage, look, trailing);
    }

    /// The rig's origin from the ribcage for the frame drawn, on a mount:
    /// worked out again (`Movement::mounted_rig_apart`) for the view and
    /// the ribcage as drawn, which are a little ahead of the simulation's
    /// last frames while the view turns; the pivot turned by the view one
    /// of the simulation's frames before, as the simulation turns it by the
    /// frame before's. Taken between those frames' own
    /// figures, the pivot came off its point by the view's turn since them
    /// (some 7 mm a degree at half a metre), and the camera, which the
    /// mount's way moves with the rig on frames with a physics step, came
    /// off the weapon by as much on the frames between: the weapon shook
    /// against the view as it turned. Only the drawing: the simulation's
    /// figures are its own.
    fn rig_apart_drawn(&self, older: &Stepped, newer: &Stepped, t: f32, position: Vec3) -> Option<[f32; 3]> {
        let (a, b) = (older.mount.or(newer.mount)?, newer.mount?);
        let lerp = |x: [f32; 3], y: [f32; 3]| -> [f32; 3] { std::array::from_fn(|i| x[i] + (y[i] - x[i]) * t) };
        let (pivot, progress) = (lerp(a.pivot, b.pivot), a.progress + (b.progress - a.progress) * t);
        let (ribcage, [yaw, pitch], [yaw_before, pitch_before]) = self.ribcage_drawn;
        // The simulation's forward is +z; Bevy's is -z.
        let origin = [position.x + ribcage[0], position.y + ribcage[1], -position.z + ribcage[2]];
        let turn = quat_mul(quat_axis(1, yaw), quat_axis(0, pitch));
        let before = quat_mul(quat_axis(1, yaw_before), quat_axis(0, pitch_before));
        Some(trp_sim::locomotion::movement::mounted_apart(b.point, origin, turn, before, pivot, progress))
    }

    /// The two frames of the simulation either side of the moment drawn,
    /// the older first, and how far from the one to the other it lies.
    fn around_drawn(&self) -> (&Stepped, &Stepped, f32) {
        let last = self.stepped.len() - 1;
        if self.cut {
            return (&self.stepped[last], &self.stepped[last], 1.0);
        }
        let behind = self.pacer.frames_behind();
        let newer = last.saturating_sub(behind.floor() as usize);
        let older = last.saturating_sub(behind.ceil() as usize);
        (&self.stepped[older], &self.stepped[newer], 1.0 - behind.fract())
    }

    /// What to draw now.
    pub fn shown(&self) -> Shown {
        let sprung = if self.cut { self.now } else { self.before.blend(self.now, self.pacer.blend()) };
        let (older, newer, t) = self.around_drawn();
        let position = older.position.lerp(newer.position, t);
        let camera_apart: [f32; 3] = std::array::from_fn(|k| older.camera_apart[k] + (newer.camera_apart[k] - older.camera_apart[k]) * t);
        let rig_apart = self
            .rig_apart_drawn(older, newer, t, position)
            .unwrap_or_else(|| std::array::from_fn(|k| older.rig_apart[k] + (newer.rig_apart[k] - older.rig_apart[k]) * t));
        // The mount's camera goes with the rig as drawn, not as it stood on
        // the frames it was worked out on.
        let mut camera = sprung.camera;
        if let Some(shift) = sprung.shift {
            camera.position = std::array::from_fn(|k| camera.position[k] - shift[k] + rig_apart[k] - camera_apart[k]);
        }
        Shown {
            position,
            ribcage: self.ribcage_shown,
            weapon: sprung.weapon,
            camera,
            camera_apart,
            rig_apart,
            arms: [blend_arm(older.arms[0], newer.arms[0], t), blend_arm(older.arms[1], newer.arms[1], t)],
            elbows: sprung.elbows,
        }
    }

    /// The body's skeleton to draw now, if the body is drawn.
    pub fn body_shown(&self) -> Option<BodyPose> {
        let (older, newer, t) = self.around_drawn();
        let newer = newer.body.as_ref()?;
        Some(older.body.as_ref().unwrap_or(newer).blend(newer, t))
    }

    /// The rig in the hands as its animator has it at the moment drawn:
    /// every bone in its parent, between the two frames of the simulation
    /// either side of that moment. The animator's results are untouched;
    /// only what is drawn of them lies between frames.
    ///
    /// `snap`, a cylinder's bone and half its chamber's angle in degrees:
    /// where that bone turns so much or more from one frame to the next, it
    /// is drawn at the nearer frame's pose, not between. The game snaps a
    /// cylinder by whole chambers within a frame at a click (its `FIRE` and
    /// `DRY FIRE` hold the bone at the first chamber's angle, the next state
    /// back at the chamber's: more-weapons spec, "A cylinder drawn between
    /// frames"), which, drawn between frames, was a spin of 60 to 180
    /// degrees there and back. No clip turns it that fast (25 degrees a
    /// frame at most).
    pub fn rig_shown(&self, snap: Option<(usize, f32)>) -> Option<Vec<([f32; 3], [f32; 4])>> {
        let (older, newer, t) = self.around_drawn();
        // Dead: the newest frame's rig as the animator's straight pose has it.
        if self.cut {
            return newer.rig_straight.as_ref().or(newer.rig.as_ref()).map(|rig| rig.to_vec());
        }
        let newer = newer.rig.as_ref()?;
        // Two frames of one rig are blended; across a change of what is in
        // the hands the newer stands alone.
        let Some(older) = older.rig.as_ref().filter(|older| older.len() == newer.len()) else { return Some(newer.to_vec()) };
        Some(blend_rig(older, newer, t, snap))
    }

}

/// Two frames of a rig's bones blended at `t`, the bone `snap` names (with
/// half its chamber's angle) drawn at the nearer frame where it turns that
/// much or more between them (`Controller::rig_shown`).
pub(crate) fn blend_rig(older: &[([f32; 3], [f32; 4])], newer: &[([f32; 3], [f32; 4])], t: f32, snap: Option<(usize, f32)>) -> Vec<([f32; 3], [f32; 4])> {
    let lerp = |a: [f32; 3], b: [f32; 3]| -> [f32; 3] { std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t) };
    let snapped = |bone: usize, a: [f32; 4], b: [f32; 4]| {
        snap.is_some_and(|(at, half)| {
            let dot = (0..4).map(|i| a[i] * b[i]).sum::<f32>().abs().min(1.0);
            at == bone && 2.0 * dot.acos().to_degrees() >= half
        })
    };
    older
        .iter()
        .zip(newer.iter())
        .enumerate()
        .map(|(bone, (a, b))| match snapped(bone, a.1, b.1) {
            true if t < 0.5 => *a,
            true => *b,
            false => (lerp(a.0, b.0), quat_lerp(a.1, b.1, t)),
        })
        .collect()
}

impl Controller {
    /// What is in the hands has been changed for something else: its rig
    /// is drawn from this pose on, and the simulation's next frame reads
    /// this of it.
    /// What the simulation takes from the rig in the hands, as its animator
    /// left it on the last frame.
    pub fn rig_pose(&self) -> &RigPose {
        &self.rig_pose
    }

    /// The rig in the hands changed for the same weapon built anew while
    /// its animation goes on (`Firearm::carry_on_from`): the frames already
    /// stepped and still to be drawn keep their poses, each bone taken
    /// from the old rig's same bone (`map`, by its path; none where the old
    /// rig has none, which takes `pose`'s), so that the drawing goes on
    /// without a stop or a jump.
    pub fn rig_carried_over(&mut self, map: &[Option<usize>], pose: Vec<([f32; 3], [f32; 4])>, read: RigPose) {
        let now = std::sync::Arc::new(pose);
        for stepped in &mut self.stepped {
            let remapped = stepped.rig.as_ref().map(|old| std::sync::Arc::new(map.iter().zip(now.iter()).map(|(from, own)| from.and_then(|i| old.get(i).copied()).unwrap_or(*own)).collect::<Vec<_>>()));
            stepped.rig = Some(remapped.unwrap_or_else(|| now.clone()));
        }
        self.rig_pose = read;
    }

    pub fn rig_changed(&mut self, pose: Vec<([f32; 3], [f32; 4])>, read: RigPose) {
        let pose = std::sync::Arc::new(pose);
        for stepped in &mut self.stepped {
            stepped.rig = Some(pose.clone());
        }
        self.rig_pose = read;
    }

    /// What the thing in the hands has said of itself since last asked.
    pub fn take_hands_events(&mut self) -> Vec<FirearmEvent> {
        std::mem::take(&mut self.hands_events)
    }

    /// The raid's clock (`GameHelper.PastTime`): the simulation's own
    /// seconds since the run began, by which a weapon's heat cools.
    pub fn raid_clock(&self) -> f64 {
        f64::from(self.pacer.simulated())
    }

    /// The moment drawn now, in the simulation's own seconds.
    pub fn drawn_time(&self) -> f32 {
        self.pacer.drawn_time()
    }

    /// Where the player looks now: the simulation's view and whatever the
    /// mouse has added since its last frame, as the next frame will let
    /// it through (a sprint, a jump and lying down slow the view's turning).
    pub fn look(&self) -> (f32, f32) {
        let [yaw, pitch] = self.movement.look_ahead([self.pending.look.x, self.pending.look.y], self.overlap);
        (yaw, pitch)
    }
}
