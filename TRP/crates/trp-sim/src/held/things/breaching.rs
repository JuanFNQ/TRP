//! Breaching, breaking in, kicking open, unlocking and the gestures at a door.

use super::*;

impl Thing {
    /// The door's line shut and open, from where it stands now, and the
    /// way from the character to the point a kick is aimed at, all flat on
    /// the ground.
    fn breach_lines(&self, from: [f32; 3]) -> Option<([f32; 2], [f32; 2], [f32; 2])> {
        let hinge = self.hinge?;
        let own = turn(hinge.forward, hinge.axis, self.angle);
        let (shut, open) = (turn(own, hinge.axis, self.shut_angle), turn(own, hinge.axis, self.open_angle));
        // The point is the door's own, and turns with it.
        let target: [f32; 3] = std::array::from_fn(|k| hinge.at[k] + own[k] * self.view_target[0] + hinge.axis[k].abs() * self.view_target[1]);
        let way = flat_unit(sub(target, from));
        Some((flat_unit(shut), flat_unit([shut[0] + open[0], 0.0, shut[2] + open[2]]), way))
    }

    /// Whether the character stands where a kick is offered
    /// (`Door.IsBreachAngle`): not too far along the door's own line.
    pub fn breach_angle(&self, from: [f32; 3], threshold: f32) -> bool {
        self.breach_lines(from).is_some_and(|(shut, _, way)| (way[0] * shut[0] + way[1] * shut[1]).abs() < threshold)
    }

    /// Whether a kick from there throws it open (`Door.BreachSuccessRoll`,
    /// which rolls nothing): only one that can be breached, and only from
    /// the side it swings away from.
    pub fn breach_succeeds(&self, from: [f32; 3]) -> bool {
        self.can_be_breached
            && self.operatable
            && self.breach_lines(from).is_some_and(|(_, both, way)| way[0] * both[0] + way[1] * both[1] > 0.0)
    }

    /// Where a door is kicked from and what the kick is aimed at
    /// (`Door.GetBreakInPosition`, `GetBreakInParameters`), for a character
    /// whose feet are at `from`: the place 1.3 m square off the leaf, half
    /// a metre along it from the hinge, on the character's side of it and
    /// at the height of its feet; and the point of the leaf half a metre
    /// along it, at the height of the door's own aiming point.
    pub fn break_in(&self, from: [f32; 3]) -> Option<([f32; 3], [f32; 3])> {
        let hinge = self.hinge?;
        let along = turn(hinge.forward, hinge.axis, self.angle);
        let off = cross(along, [0.0, 1.0, 0.0]);
        let place = |side: f32| -> [f32; 3] {
            let at: [f32; 3] = std::array::from_fn(|k| hinge.at[k] + side * BREAK_IN_OFF * off[k] + BREAK_IN_ALONG * along[k]);
            [at[0], from[1], at[2]]
        };
        let (one, other) = (place(1.0), place(-1.0));
        let far = |at: [f32; 3]| dot(sub(at, from), sub(at, from));
        let position = if far(one) < far(other) { one } else { other };
        let target = [hinge.at[0] + BREAK_IN_ALONG * along[0], hinge.at[1] + self.view_target[1], hinge.at[2] + BREAK_IN_ALONG * along[2]];
        Some((position, target))
    }

    /// A kick is begun at it by a character whose feet are at `from`
    /// (`MovementContext.StartDoorBreak`): where it is kicked from and
    /// aimed at, whether the door puts the character there first, and
    /// whether the kick will throw it open, which goes by the place it is
    /// kicked from. The door is held for it from now.
    pub fn start_breach(&mut self, from: [f32; 3]) -> Option<Breach> {
        let (position, view_target) = self.break_in(from)?;
        let snaps = match self.state {
            ThingState::Shut => self.snaps_shut,
            ThingState::Locked => self.snaps_locked,
            _ => false,
        };
        let opens = self.breach_succeeds(position);
        self.hold();
        Some(Breach { position, view_target, snaps, opens })
    }

    /// Held for the work (`LockForInteraction`): being worked, with what
    /// it was noted. A kick holds the door from the key on.
    pub fn hold(&mut self) {
        if self.state != ThingState::Interacting {
            self.fallback = self.state;
            self.state = ThingState::Interacting;
        }
    }

    /// The kick lands and throws the door open (`Door.KickOpen`): from its
    /// shut angle to its open one in half a second, with no wait, and open
    /// when that is over. Not while one kick's swing is already the order
    /// in hand.
    pub fn kick_open(&mut self) {
        if self.breaching {
            return;
        }
        (self.breaching, self.result, self.broken) = (true, None, true);
        self.record = [0.0, KICK_LASTS];
        // The breach clip, and the swing's own opening at once: the
        // routine runs to its first yield as it is started, and a kick's
        // swing has no wait.
        self.sounds.push(ThingSound::Breach);
        self.sounds.push(ThingSound::Swing { opens: false, squeaks: self.fallback != ThingState::Locked });
        self.swings.push(Swing {
            wait: None,
            from: self.shut_angle,
            turn: delta_angle(self.shut_angle, self.open_angle),
            opening: true,
            progress: 0.0,
            pace: 1.0,
            to: ThingState::Open,
            kicked: true,
            begun: true,
            shut_played: false,
        });
    }

    /// The kick lands and the door holds (`Door.FailBreach`): three tenths
    /// of a second on it is what it was again, unless it is swinging.
    pub fn fail_breach(&mut self) {
        self.sounds.push(ThingSound::Hit);
        self.failed_kick = Some((0.0, true));
    }

    /// Unlocked with its key (`Unlock`): shut, at once.
    pub fn unlock(&mut self) {
        (self.state, self.fallback) = (ThingState::Shut, ThingState::Shut);
        self.angle = self.shut_angle;
    }

    /// The gesture it asks of a character whose feet are at `from`
    /// (`GetInteractionParameters`): none for a container or a trunk being
    /// worked, whose number is then nothing.
    pub fn gesture(&self, from: [f32; 3]) -> Option<Interaction> {
        match self.kind {
            Kind::Door => Interaction::of_game_number(self.door_gesture(from)),
            Kind::Container | Kind::Trunk => match self.state {
                ThingState::Shut => Some(self.open_gesture.unwrap_or(Interaction::ContainerOpenDefault)),
                ThingState::Open => Some(self.close_gesture.unwrap_or(Interaction::ContainerCloseDefault)),
                ThingState::Interacting | ThingState::Locked => None,
            },
        }
    }

    /// A door's gesture by the game's own number (`Door.CalculateInteractionIndex`).
    /// Of the line the door lies along now and the line it will lie along
    /// when it has swung, it asks on which side of the first the character
    /// stands against the side the door will swing to (towards the
    /// character or away), and on which hand the hinge is (the upward part
    /// of a cross product). Both lines are the door's own axis turned by its
    /// angles from where the door stands now.
    pub fn door_gesture(&self, from: [f32; 3]) -> u8 {
        let Some(hinge) = self.hinge else { return 0 };
        // The door's own forward axis turns with it.
        let own = turn(hinge.forward, hinge.axis, self.angle);
        let (now, then) = match self.state {
            ThingState::Shut => (turn(own, hinge.axis, self.shut_angle), turn(own, hinge.axis, self.open_angle)),
            ThingState::Open => (turn(own, hinge.axis, -self.shut_angle), turn(own, hinge.axis, -self.open_angle)),
            ThingState::Interacting | ThingState::Locked => ([0.0; 3], [0.0; 3]),
        };
        let to_character = sub(from, hinge.at);
        let foot = sub(project(to_character, now), to_character);
        let side = sub(then, project(then, now));
        let away = dot(foot, side) > 0.0;
        let right = cross(now, foot)[1] > 0.0;
        match (self.state, right, away) {
            (ThingState::Shut, _, true) => 27,
            (ThingState::Shut, true, false) => 26,
            (ThingState::Shut, false, false) => 24,
            (ThingState::Open, true, true) => 25,
            (ThingState::Open, false, true) => 23,
            _ => 28,
        }
    }
}
