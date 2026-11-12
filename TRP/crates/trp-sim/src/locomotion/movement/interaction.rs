//! Interactions: working, searching, unlocking, kicking doors and picking up.

use super::*;

impl Movement {
    /// Asks for a gesture of the hands (`MovementContext.SetInteractInHands`).
    /// It is refused in a sprint, and while the animator of the thing in
    /// hand is anywhere but at rest (a backpack's dropping excepted); a
    /// hand sign also in a jump and its landing. (A stationary weapon in
    /// hand would refuse it too; there is none here.)
    pub fn set_interaction(&mut self, interaction: Interaction) -> bool {
        // (Nothing while mounted: `SetInteractInHands`.)
        if self.mount.is_some() || self.sprinting || (!self.hands_idle && interaction != Interaction::DropBackpack) {
            return false;
        }
        if interaction.is_gesture() && matches!(self.state, State::Jump | State::Landing) {
            return false;
        }
        let shows = self.interactions.set(interaction);
        self.show(shows);
        true
    }

    /// The gesture playing on the body's animator, if one is.
    pub fn interaction(&self) -> Option<Interaction> {
        self.interactions.current()
    }

    /// Whether a gesture is playing or waiting to
    /// (`AnimatedInteractions.IsInteractionPlaying`).
    pub fn interaction_playing(&self) -> bool {
        self.interactions.is_playing()
    }

    /// The body's animator's two flags for an open inventory, as given
    /// (`Inventory`, `InventoryOperation`).
    pub fn body_inventory(&self) -> [bool; 2] {
        self.body_inventory
    }

    /// Whether the body may take a gesture for the bipod now
    /// (`FirearmController.ToggleBipod`'s part of the movement's: the
    /// gestures' queue takes one and nothing plays).
    pub fn gesture_free(&self) -> bool {
        !self.interactions.schedule_denied() && self.interactions.can_interact()
    }

    /// The bipod's gesture (`ToggleBipodOperation.Start`'s `SetInteractInHands`):
    /// out or in, by the way it folds. `Acted::BipodToggled` comes when its
    /// state passes the toggle's mark. Whether it plays.
    pub fn bipod_gesture(&mut self, out: bool, forward: bool) -> bool {
        let gesture = Interaction::bipod(out, forward);
        let plays = self.set_interaction(gesture);
        if plays {
            self.toggling_bipod = Some(gesture);
        }
        plays
    }

    /// Whether the movement lets a backpack be dropped (`Player.DropBackpack`,
    /// of what it asks of the movement): not in a sprint, not through a
    /// kick, and only with no gesture of the hands playing.
    pub fn can_drop_backpack(&self) -> bool {
        !self.sprinting && self.state != State::Breach && self.interactions.can_interact()
    }

    /// The backpack is dropped (the hands' `DropBackpackOperation`, which
    /// asks the body for the gesture): `Acted::BackpackDropped` comes when
    /// the gesture's state passes its mark for the drop. The gesture is
    /// asked for whatever the thing in hand is doing; in a sprint it is
    /// refused, and then nothing is dropped. Returns whether it plays.
    pub fn drop_backpack(&mut self) -> bool {
        // Dropping the backpack leaves a mount at once (`Player.DropBackpack`).
        self.leave_mount_now();
        let plays = self.set_interaction(Interaction::DropBackpack);
        self.dropping_backpack |= plays;
        plays
    }

    /// The character works a thing of the world: a door pushed or pulled,
    /// a container opened or shut (`Player.ExecuteInteraction`,
    /// `MovementState.ExecuteDoorInteraction`; interactions spec, "Working
    /// a door or a container"). **The thing itself begins to move half a
    /// second later or a little more** (`things::Thing` does it). Here a
    /// character lying down is no longer lying, and the
    /// thing's gesture is asked of the left hand: it is refused in a
    /// sprint and while the thing in hand is busy (`set_interaction`), and
    /// the thing works all the same; a thing worked without a gesture
    /// (`interactWithoutAnimation`) asks for none. No movement state is
    /// taken: the character walks, turns and does as it likes meanwhile.
    /// Returns whether a gesture plays.
    pub fn work(&mut self, gesture: impl Into<Option<Interaction>>) -> bool {
        if self.prone {
            self.toggle_prone_for_interaction();
        }
        gesture.into().is_some_and(|gesture| self.set_interaction(gesture))
    }

    /// The character searches a container standing at `at`: it is worked
    /// as above, and when the game's wait for it is over
    /// (`MovementSettings::thing_answers_after`, the install's
    /// `DelayToOpenContainer`) `Acted::Searched` says it is to be shown,
    /// or `Acted::LeftUnsearched` that it is not: the character is in a
    /// jump, or more than a quarter of a metre further from it than it
    /// was at the key.
    pub fn search(&mut self, gesture: impl Into<Option<Interaction>>, at: [f32; 3]) -> bool {
        let played = self.work(gesture);
        self.searching = Some((at, self.distance_to(at), 0.0, true));
        played
    }

    pub(super) fn distance_to(&self, at: [f32; 3]) -> f32 {
        let d = [at[0] - self.position[0], at[1] - self.position[1], at[2] - self.position[2]];
        (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
    }

    /// The character works a thing that is unlocked with a key or a card
    /// in the hand, or kicks a door in: the game's other way to a thing
    /// (`MovementContext.StartInteraction`, `ApproachState`,
    /// `DoorInteractState`; interactions spec, "Unlocking and kicking").
    /// Lying down it gets up first. The lean is let go and a low thing
    /// crouched to (`approach`); the gesture begins; `Acted::Worked` comes
    /// when the hand has been laid on the thing, and `Acted::Done` when
    /// the state is over. Nothing is started while something else is
    /// being worked. PROVISIONAL: read from the game's code and compared
    /// with nothing; no recording has a lock or a kick.
    pub fn unlock(&mut self, with: WorldInteraction) -> bool {
        if self.acting.is_some() {
            return false;
        }
        if self.prone {
            // The game takes the character out of its lying pose here.
            self.toggle_prone_for_interaction();
        }
        self.approach(with.height);
        self.tilt = 0.0;
        self.set_interaction(with.interaction);
        self.acting = Some(Acting::Working { time: 0.0, worked: false, snap: with.snap });
        true
    }

    /// The character kicks a door (`MovementContext.StartDoorBreak`;
    /// interactions spec, "A kick"). `opens` is whether the kick will throw
    /// the door open, which the door says for where the character stands
    /// (`Thing::breach_succeeds`), and which picks the body's clip. The
    /// body's animator takes it from here: out of standing or a walk into
    /// its approach, and from there into the kick, and
    /// `Acted::Kicked` says when it lands. Nothing is started while
    /// another kick, an unlocking or a pick-up is in hand.
    ///
    /// `snap` is for a door that puts the character at a place of its own
    /// first: that place and the point it has the character look at
    /// (`Thing::start_breach`). The approach then walks the character
    /// there and turns its view to the point, levelled, over as long as
    /// the longest of the three takes, and the kick waits for it. A door
    /// that does not leaves the character where it stands, looking as it
    /// looks.
    pub fn kick(&mut self, opens: bool, snap: Option<([f32; 3], [f32; 3])>) -> bool {
        if self.kick.is_some() || self.acting.is_some() {
            return false;
        }
        // Where the door puts the character nowhere, it counts as
        // standing where it is to from the key on.
        self.kick = Some(Kick { opens, landed: false, approached: snap.is_none(), snap, approach: None });
        true
    }

    /// The approach begins (`ApproachState.Enter`), for a thing that puts
    /// the character at a place: how long it takes is the longest of the
    /// walk there, the turn of the view and its levelling, and no more
    /// than a second and a half.
    pub(super) fn begin_approach(&mut self) {
        let Some(Kick { snap: Some((to, target)), .. }) = self.kick else { return };
        let from = self.position;
        // The view is to look from that place, at the ribcage's height,
        // to the point, and level (`ViewTargetWithZeroPitch`).
        let eye = [to[0], self.position[1] + self.ribcage()[1], to[2]];
        let yaw = (target[0] - eye[0]).atan2(target[2] - eye[2]).to_degrees();
        let look_to = [yaw, 0.0];
        let far = (0..3).map(|k| (to[k] - from[k]).powi(2)).sum::<f32>().sqrt();
        let speed = lerp(APPROACH_SPEED[0], APPROACH_SPEED[1], self.stamina.settings.overweight.clamp(0.0, 1.0));
        let turns = [delta_angle(look_to[0], self.yaw).abs(), delta_angle(look_to[1], self.pitch).abs()];
        let length = (far / speed).max(turns[0] / APPROACH_TURN).max(turns[1] / APPROACH_TURN).min(APPROACH_LONGEST);
        let approach = Approaching {
            time: 0.0,
            length,
            from,
            to,
            look_from: [self.yaw, self.pitch],
            look_to,
            body_from: self.body_yaw,
            direction: [0.0; 2],
            fresh: true,
        };
        if let Some(kick) = &mut self.kick {
            kick.approach = Some(approach);
        }
    }

    /// The approach's own handling of a frame (`ApproachState.
    /// ManualAnimatorMoveUpdate`, `Approach`): the character is where the
    /// approach's curve has it between where it stood and its place, its
    /// view and its body turned by the same share; when the approach's
    /// time is out it counts as standing there. The frame the state is
    /// entered on has run none of it.
    pub(super) fn update_approach(&mut self, dt: f32, world: &dyn World) {
        let Some(Kick { approach: Some(mut approach), .. }) = self.kick else { return };
        if !std::mem::take(&mut approach.fresh) {
            approach.time += self.frame_dt;
        }
        // The curve eases in and out: flat at both ends.
        let share = if approach.length > 0.0 {
            let s = (approach.time / approach.length).clamp(0.0, 1.0);
            (3.0 * s * s - 2.0 * s * s * s).clamp(0.0, 1.0)
        } else {
            1.0
        };
        self.body_yaw = lerp_angle(approach.body_from, approach.look_to[0], share);
        self.yaw = lerp_angle(approach.look_from[0], approach.look_to[0], share);
        self.pitch = lerp_angle(approach.look_from[1], approach.look_to[1], share);
        let motion: [f32; 3] = std::array::from_fn(|k| lerp(approach.from[k], approach.to[k], share) - self.position[k]);
        let size = (motion[0] * motion[0] + motion[1] * motion[1] + motion[2] * motion[2]).sqrt();
        if size > 0.0 {
            self.travel(motion, dt, world, true);
        }
        // The body's animator is told which way it is walked, a tenth of
        // the way there each frame.
        let heading = if size > 0.0 { [motion[0] / size, motion[2] / size] } else { [0.0; 2] };
        let local = self.to_local(heading);
        approach.direction = std::array::from_fn(|k| lerp(approach.direction[k], local[k], 0.1));
        self.movement_direction = approach.direction;
        self.note_rotation();
        let there = approach.time > approach.length;
        if let Some(kick) = &mut self.kick {
            kick.approach = Some(approach);
            kick.approached |= there;
        }
    }

    /// Whether a door is being kicked: from the key until the kick's
    /// state is left.
    pub fn kicking(&self) -> bool {
        self.kick.is_some()
    }

    /// Whether the movement state keeps the inventory screen shut (the
    /// approach to a thing does: `StateLocksInventory`).
    pub fn locks_inventory(&self) -> bool {
        self.state == State::Approach
    }

    /// The kick's own handling of a frame (`BreachDoorState.
    /// ManualAnimatorMoveUpdate`): the body does not turn after the view;
    /// from 0.15 of the way through the state it moves by the clip's own
    /// motion and turn; and 0.45 of the way through, once, the kick lands,
    /// which costs the legs what a jump does (`Physical.OnBreach`).
    pub(super) fn update_breach(&mut self, dt: f32, world: &dyn World) {
        let through = self.body.time_in(movement_state::BREACH_DOOR).unwrap_or(0.0);
        if through > KICK_MOVES_FROM {
            let motion = self.animated_motion;
            self.apply_motion(motion, dt, world);
            self.body_yaw += self.body.turn();
            self.note_rotation();
        }
        if through >= KICK_LANDS_AT {
            if let Some(kick) = self.kick.as_mut().filter(|kick| !kick.landed) {
                kick.landed = true;
                self.acted.push(Acted::Kicked { opens: kick.opens });
                self.stamina.jump(self.frame_clock);
            }
        }
    }

    /// The character takes up a thing it looks at (`IdlePlayerState.Pickup`,
    /// `PickUpState`). Only standing still: in any other movement state
    /// the game's call does nothing. The view is held, the lean let go,
    /// and with the view more than 60 degrees down the character goes
    /// down to a third of its stance; `Acted::PickedUp` comes 0.6 s in,
    /// and with it the state is over and the stance taken again.
    pub fn pick_up(&mut self) -> bool {
        self.reach_for(false)
    }

    /// The character reaches for a thing it looks at to examine it where
    /// it lies (the world's EXAMINE: `Pickup` with the examine as its
    /// action, `InteractionContextHelper.GetAvailableInteractionState`): the
    /// same state as `pick_up`, `Acted::PickedUp` 0.6 s in being the
    /// examine's beginning; but the state lasts until `examined` says the
    /// examine is over (`PickUpState.Examine`), or 5 s.
    pub fn pick_up_to_examine(&mut self) -> bool {
        self.reach_for(true)
    }

    /// The examine a pick-up's state waits on is over: the state ends with
    /// the next step, the stance it began with taken again.
    pub fn examined(&mut self) {
        if let Some(Acting::PickingUp { waits: true, told, .. }) = &mut self.acting {
            *told = true;
        }
    }

    fn reach_for(&mut self, waits: bool) -> bool {
        if self.acting.is_some() || self.state != State::Idle {
            return false;
        }
        self.tilt = 0.0;
        self.set_interaction(Interaction::TakeItem);
        let level = self.pose_level;
        if self.pitch > PICK_UP_CROUCH_PITCH {
            // Lower only, standing: the room to stand is never asked.
            self.change_pose(level / 3.0 - level, &Flat);
        }
        self.acting = Some(Acting::PickingUp { time: 0.0, level, taken: false, waits, told: false });
        true
    }

    /// Whether the movement state lets the player look at a thing to work
    /// it (`MovementContext.CanInteract`, as the interaction's line cast
    /// asks it; interactions spec, "What is looked at"): standing, walking,
    /// lying still or crawling and blind fire do; a sprint, a jump and its
    /// landing, a sidestep, going down or getting up, a vault, a reload or a
    /// turn lying down, and the states for the hands on the world do not.
    /// The thing in hand has its own say (`Firearm::can_interact`).
    pub fn can_interact(&self) -> bool {
        self.can_interact
    }

    /// Whether the character is unlocking a thing of the world or taking
    /// one up: the walk is then refused, and for a pick-up the view too.
    pub fn acting(&self) -> bool {
        self.acting.is_some()
    }

    /// Whether a container has been searched and is not shown yet.
    pub fn searching(&self) -> bool {
        self.searching.is_some()
    }

    /// What came of it since this was last asked.
    pub fn take_acted(&mut self) -> Vec<Acted> {
        std::mem::take(&mut self.acted)
    }

    /// Moves the state for the hands on the world on by a frame. The
    /// game's two states time by the real clock (`DoorInteractState`,
    /// `PickUpState`: `Time.realtimeSinceStartup` since they were entered),
    /// which the player's whole-millisecond stopwatch is read from: so it
    /// is given the player's frame time (recorded: a lock opened with its
    /// key shut on the game's frame with it, a frame late with the
    /// engine's).
    pub(super) fn act(&mut self, dt: f32, world: &dyn World) {
        match &mut self.acting {
            Some(Acting::Working { time, worked, snap }) => {
                *time += dt;
                if !*worked {
                    if *time > HAND_LAID_AFTER {
                        *worked = true;
                        self.acted.push(Acted::Worked);
                    }
                } else if !*snap || *time > WORKING_ENDS_AFTER {
                    // The frame after the thing has done its part, or the
                    // state's full length.
                    self.acting = None;
                    self.acted.push(Acted::Done);
                }
            }
            Some(Acting::PickingUp { time, level, taken, waits, told }) => {
                *time += dt;
                let level = *level;
                let now = *time > PICKED_UP_AFTER && !*taken;
                if now {
                    *taken = true;
                }
                // The thing taken ends the state; an examine's state waits
                // on the examine's end, or its 5 s. Ended, the stance it
                // began with is taken again.
                let over = if *waits { *told || *time > PICK_UP_LASTS } else { now };
                if now {
                    self.acted.push(Acted::PickedUp);
                }
                if over {
                    self.acting = None;
                    if !self.prone {
                        self.change_pose(level - self.pose_level, world);
                    }
                }
            }
            None => {}
        }
    }

    /// A character lying down that is to work a thing of the world is no
    /// longer lying, and its stance is put at its lowest
    /// (`StartInteraction`).
    fn toggle_prone_for_interaction(&mut self) {
        self.prone = false;
        self.pose_level = 0.0;
    }
}
