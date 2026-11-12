//! Updating the preset: key states, commands and axes.

use super::*;

impl Preset {
    /// What the mouse's travel on this frame is given to, across and up
    /// and down: after `update`, with the keys as that left them.
    ///
    /// The turn's two axes and the look's two are all bound to the mouse,
    /// and the look's have the free look's keys as their modifier
    /// (`InputAxisCombination`): an axis of the look that is idle takes
    /// the mouse's axis on a frame the mouse moves along it with those
    /// keys held, and gives nothing on that frame; from the next it gives
    /// the travel, until a frame on which the mouse does not move along
    /// that axis or the keys are up, which lets the axis go. The turn has
    /// of an axis only what the look has not taken on the frame: so the
    /// first frame of each run of travel is given to neither. Each of the
    /// two axes goes by itself.
    pub fn mouse(&mut self, moved: [f32; 2]) -> MouseHeard {
        let mut heard = MouseHeard::default();
        let mut taken = [false; 2];
        for at in 0..self.look.len() {
            let status = self.look_status(at);
            for axis in 0..2 {
                let has = self.look[at].has[axis];
                let held = match has {
                    true => !matches!(status, KeyPress::Up | KeyPress::None),
                    false => matches!(status, KeyPress::Down | KeyPress::Hold),
                };
                let keeps = moved[axis] != 0.0 && held;
                if keeps && has && !taken[axis] {
                    heard.look[axis] = moved[axis];
                }
                taken[axis] |= keeps;
                self.look[at].has[axis] = keeps;
            }
        }
        for axis in 0..2 {
            if !taken[axis] {
                heard.turn[axis] = moved[axis];
            }
        }
        heard
    }

    /// What the free look's keys are together for one variant, and none
    /// if a combination the look's axes give way to has one of them: any
    /// of as many inputs or more, but never the look's own binding nor
    /// blind fire's (`InputAxisCombination.IsLowerPriority`).
    fn look_status(&self, at: usize) -> KeyPress {
        let look = &self.look[at];
        // The mouse's axis, moving, counts as held.
        let mut status = KeyPress::Hold;
        for &key in &look.modifiers {
            let state = &self.keys[key];
            let gives_way = state.in_use_by.is_some_and(|user| {
                let theirs = &self.combinations[user];
                match &theirs.kind {
                    Kind::Key { group, blind, .. } if *group == look.group || *blind => false,
                    _ => theirs.important || theirs.count >= look.count,
                }
            });
            if gives_way {
                return KeyPress::None;
            }
            let press = match state.id {
                KeyId::Wheel(_) if matches!(state.press, KeyPress::Down | KeyPress::Hold) => KeyPress::Hold,
                KeyId::Wheel(_) => KeyPress::None,
                KeyId::Key(_) => state.press,
            };
            status = MERGE[status as usize][press as usize];
        }
        status
    }

    /// One frame (`InputPreset.UpdateInput`): `held` says which keys are
    /// down, `wheel` which way the wheel turned on this frame if it did,
    /// and `dt` is the frame's length in seconds.
    pub fn update(&mut self, held: &dyn Fn(Code) -> bool, wheel: i32, dt: f32) -> Heard {
        for key in &mut self.keys {
            let down = match key.id {
                KeyId::Key(code) => held(code),
                KeyId::Wheel(up) => wheel != 0 && (wheel > 0) == up,
            };
            key.press = UPDATE[down as usize][key.press as usize];
        }
        let mut heard = Heard { axes: vec![0.0; self.axes], ..Heard::default() };
        for index in 0..self.combinations.len() {
            let Kind::Key { group, holds, .. } = self.combinations[index].kind else {
                self.update_axis(index);
                continue;
            };
            match self.command(index, holds, dt) {
                Some(true) => heard.given.push(group),
                Some(false) => heard.taken_back.push(group),
                None => {}
            }
        }
        // An axis takes the push of the first of its pairs that has one.
        for &index in &self.axis_order {
            if let Kind::Axis { axis, .. } = self.combinations[index].kind {
                if heard.axes[axis].abs() < 0.0001 {
                    heard.axes[axis] = self.progress[index].value;
                }
            }
        }
        heard
    }

    /// Whether a combination gives way to the one that has a key
    /// (`IsLowerPriority`).
    fn lower(&self, this: usize, other: usize) -> bool {
        if this == other {
            return false;
        }
        let (mine, theirs) = (&self.combinations[this], &self.combinations[other]);
        if theirs.important {
            return true;
        }
        match (&mine.kind, &theirs.kind) {
            (Kind::Key { check_hash, priority, .. }, Kind::Key { check_hash: other_hash, priority: other_priority, press, .. }) => {
                if check_hash == other_hash {
                    // The same keys: only a double click is waited for.
                    return *press == PressType::DoubleClick;
                }
                if *priority == NEVER_YIELDS {
                    return false;
                }
                if *priority > UNRANKED && *other_priority > UNRANKED {
                    return priority >= other_priority;
                }
                theirs.count >= mine.count
            }
            // An axis keeps its keys from blind fire's.
            (Kind::Axis { .. }, Kind::Key { blind: true, .. }) => false,
            _ => theirs.count >= mine.count,
        }
    }

    /// What a combination's keys are together, and whether none of them
    /// is in the hands of a combination it gives way to (`GetKeysStatus`).
    fn status(&mut self, index: usize) -> (bool, KeyPress) {
        let (mut free, mut status) = (true, KeyPress::Hold);
        for at in 0..self.combinations[index].keys.len() {
            let key = self.combinations[index].keys[at];
            if self.keys[key].release_next_frame && self.keys[key].in_use_by.is_none_or(|user| user >= index) {
                self.keys[key].in_use_by = None;
                self.keys[key].release_next_frame = false;
            }
            if free && self.keys[key].in_use_by.is_some_and(|user| self.lower(index, user)) {
                free = false;
            }
            status = MERGE[status as usize][self.keys[key].press as usize];
        }
        (free, status)
    }

    /// Takes the combination's keys, for good or for this frame.
    fn handle(&mut self, index: usize, this_frame_only: bool) {
        for at in 0..self.combinations[index].keys.len() {
            let key = self.combinations[index].keys[at];
            self.keys[key].in_use_by = Some(index);
            if this_frame_only {
                self.keys[key].release_next_frame = true;
            }
        }
    }

    /// Lets go of them; a double click that did not come gives them back
    /// as just pressed, so that a plain press of the same key is heard.
    fn unhandle(&mut self, index: usize, restore: Option<KeyPress>) {
        for at in 0..self.combinations[index].keys.len() {
            let key = self.combinations[index].keys[at];
            if self.keys[key].in_use_by == Some(index) {
                self.keys[key].in_use_by = None;
            }
            if let Some(press) = restore {
                self.keys[key].press = press;
            }
        }
    }

    fn change(&mut self, index: usize, state: State) {
        if self.progress[index].state != state {
            self.progress[index].state = state;
            if matches!(state, State::LongTapPressed | State::WaitingForFirstRelease) {
                self.progress[index].time_left = self.double_click;
            }
        }
    }

    fn timed_out(&mut self, index: usize, dt: f32) -> bool {
        self.progress[index].time_left -= dt;
        self.progress[index].time_left <= 0.0
    }

    /// A combination's frame: whether its binding is given (`true`) or
    /// taken back (`false`) on it.
    fn command(&mut self, index: usize, holds: bool, dt: f32) -> Option<bool> {
        let state = self.progress[index].state;
        // A double click that takes too long, or whose key something else
        // has taken, is off.
        if matches!(state, State::WaitingForFirstRelease | State::WaitingForSecondPress | State::WaitingForSecondRelease) {
            if self.timed_out(index, dt) {
                self.unhandle(index, Some(KeyPress::Down));
                self.change(index, State::DoubleClickIdling);
                return None;
            }
            let (free, status) = self.status(index);
            if !free {
                self.unhandle(index, None);
                self.change(index, State::DoubleClickIdling);
                return None;
            }
            if state == State::WaitingForSecondRelease && status == KeyPress::Up {
                self.handle(index, true);
                self.change(index, State::DoubleClickIdling);
                return Some(true);
            }
            self.handle(index, false);
            match (state, status) {
                (State::WaitingForFirstRelease, KeyPress::Up) => self.change(index, State::WaitingForSecondPress),
                (State::WaitingForSecondPress, KeyPress::Down) => self.change(index, State::WaitingForSecondRelease),
                _ => {}
            }
            return None;
        }
        let (free, status) = self.status(index);
        match state {
            State::ClickIdling => {
                if free && status == KeyPress::Down {
                    self.handle(index, false);
                    self.change(index, State::ClickWaitForRelease);
                    return Some(true);
                }
            }
            State::ClickWaitForRelease | State::LongTapWaitForRelease => {
                if status != KeyPress::Hold {
                    if free {
                        self.handle(index, true);
                    } else {
                        self.unhandle(index, None);
                    }
                    self.change(index, if state == State::ClickWaitForRelease { State::ClickIdling } else { State::ContinuousIdling });
                } else if free {
                    self.handle(index, false);
                }
            }
            State::ContinuousIdling => {
                if free && matches!(status, KeyPress::Down | KeyPress::Hold) {
                    self.handle(index, false);
                    // A binding that holds is given at once; any other
                    // only when it has been held long enough.
                    if holds {
                        self.change(index, State::ContinuousPressed);
                        return Some(true);
                    }
                    self.change(index, State::LongTapPressed);
                }
            }
            State::ContinuousPressed => {
                if free && status == KeyPress::Hold {
                    self.handle(index, false);
                } else {
                    self.unhandle(index, None);
                    self.change(index, State::ContinuousIdling);
                    return Some(!holds);
                }
            }
            State::LongTapPressed => {
                if free && status == KeyPress::Hold {
                    self.handle(index, false);
                    if self.timed_out(index, dt) {
                        self.change(index, State::LongTapWaitForRelease);
                        return Some(true);
                    }
                } else {
                    self.unhandle(index, None);
                    self.change(index, State::ContinuousIdling);
                }
            }
            State::DoubleClickIdling => {
                if free && status == KeyPress::Down {
                    self.handle(index, false);
                    self.change(index, State::WaitingForFirstRelease);
                }
            }
            State::ReleaseIdling => {
                if matches!(status, KeyPress::Down | KeyPress::Hold) {
                    if free {
                        self.handle(index, false);
                    }
                    self.change(index, State::ReleasePressed);
                }
            }
            State::ReleasePressed => match status {
                KeyPress::Up => {
                    self.change(index, State::ReleaseIdling);
                    if free {
                        self.handle(index, true);
                        return Some(true);
                    }
                }
                KeyPress::Hold if free => self.handle(index, false),
                KeyPress::Hold => {
                    // A key taken by a combination of other keys: the
                    // release is not heard this time.
                    let mine = match self.combinations[index].kind {
                        Kind::Key { check_hash, .. } => check_hash,
                        Kind::Axis { .. } => 0,
                    };
                    let taken = self.combinations[index].keys.iter().any(|key| {
                        self.keys[*key].in_use_by.is_some_and(|user| !matches!(self.combinations[user].kind, Kind::Key { check_hash, .. } if check_hash == mine))
                    });
                    self.unhandle(index, None);
                    if taken {
                        self.change(index, State::ReleaseStuck);
                    }
                }
                _ => {}
            },
            State::ReleaseStuck => {
                if status == KeyPress::Up {
                    self.change(index, State::ReleaseIdling);
                }
            }
            _ => {}
        }
        None
    }

    /// Which way an axis's keys push, and what its keys are together
    /// (`AxisCombinationState.GetKeysStatus`): the first way whose keys
    /// are all the axis's to use and none of them idle.
    fn axis_status(&self, index: usize, with_common: bool) -> (Option<usize>, KeyPress) {
        let Kind::Axis { ways, common, .. } = &self.combinations[index].kind else { return (None, KeyPress::None) };
        let mut found = (None, KeyPress::Hold);
        for (way, (_, keys)) in ways.iter().enumerate() {
            let mut status = KeyPress::Hold;
            if keys.is_empty() {
                found.1 = status;
                continue;
            }
            for &key in keys {
                let state = &self.keys[key];
                if status == KeyPress::None || state.in_use_by.is_some_and(|user| self.lower(index, user)) {
                    status = KeyPress::None;
                    break;
                }
                let press = match state.id {
                    KeyId::Wheel(_) if matches!(state.press, KeyPress::Down | KeyPress::Hold) => KeyPress::Hold,
                    KeyId::Wheel(_) => KeyPress::None,
                    KeyId::Key(_) => state.press,
                };
                status = MERGE[press as usize][status as usize];
            }
            found.1 = status;
            if status != KeyPress::None {
                found.0 = Some(way);
                break;
            }
        }
        let (Some(way), true) = (found.0, with_common) else { return found };
        let mut status = found.1;
        for &key in common {
            if self.keys[key].in_use_by.is_some_and(|user| self.lower(index, user)) {
                return (None, KeyPress::None);
            }
            status = MERGE[status as usize][self.keys[key].press as usize];
        }
        (Some(way), status)
    }

    /// An axis's push one way: one if every key of that way is down, and
    /// every key the two ways share.
    fn axis_value(&self, index: usize, way: usize) -> f32 {
        let Kind::Axis { ways, common, .. } = &self.combinations[index].kind else { return 0.0 };
        let down = |keys: &[usize]| keys.iter().all(|key| matches!(self.keys[*key].press, KeyPress::Down | KeyPress::Hold));
        let (positive, keys) = &ways[way];
        if !down(keys) || !down(common) {
            return 0.0;
        }
        if *positive { 1.0 } else { -1.0 }
    }

    fn update_axis(&mut self, index: usize) {
        let state = self.progress[index].state;
        let (way, status) = self.axis_status(index, state != State::JustAxis);
        let value = match (state, way) {
            (State::JustAxis, Some(way)) => self.axis_value(index, way),
            (State::AxisIdling, Some(way)) if matches!(status, KeyPress::Down | KeyPress::Hold) => {
                self.take_axis(index, Some(way));
                self.progress[index].state = State::AxisPressed;
                0.0
            }
            (State::AxisPressed, Some(way)) if !matches!(status, KeyPress::Up | KeyPress::None) => {
                self.take_axis(index, Some(way));
                self.axis_value(index, way)
            }
            (State::AxisPressed, _) => {
                self.take_axis(index, None);
                self.progress[index].state = State::AxisIdling;
                0.0
            }
            _ => 0.0,
        };
        self.progress[index].value = value;
    }

    /// An axis whose ways share a key takes that key and the keys of the
    /// way pushed, or lets go of all it has.
    fn take_axis(&mut self, index: usize, way: Option<usize>) {
        let Kind::Axis { ways, common, .. } = &self.combinations[index].kind else { return };
        match way {
            Some(way) => {
                for &key in common.iter().chain(&ways[way].1) {
                    self.keys[key].in_use_by = Some(index);
                }
            }
            None => {
                for &key in common.iter().chain(ways.iter().flat_map(|(_, keys)| keys)) {
                    if self.keys[key].in_use_by == Some(index) {
                        self.keys[key].in_use_by = None;
                    }
                }
            }
        }
    }
}
