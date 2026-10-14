//! The head's free look (`docs/spec/free-look.md`): the camera's own turn
//! while its key is held, apart from the body's and the weapon's, and its
//! way back when the key comes up. This is the game's `Player.Look`, with
//! its state.
//!
//! Angles are degrees. The head's turn across is negative to the right
//! and its turn up and down positive downward, as the game keeps them.

/// The limits of the head's turn (`MOUSE_LOOK_HORIZONTAL_LIMIT`,
/// `MOUSE_LOOK_VERTICAL_LIMIT`, `MOUSE_LOOK_LIMIT_IN_AIMING_COEF` of the
/// game's settings object).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeadLookSettings {
    /// The least and the most of the turn across.
    pub across: [f32; 2],
    /// The least (up) and the most (down) of the turn up and down.
    pub down: [f32; 2],
    /// What the limits across are multiplied by with the sights up.
    pub aiming: f32,
}

impl Default for HeadLookSettings {
    /// The figures the game's code has, which stand where the install's
    /// settings object cannot be read.
    fn default() -> Self {
        Self { across: [-40.0, 40.0], down: [-50.0, 20.0], aiming: 0.7 }
    }
}

/// The share of what is left of the turn that goes each second on the
/// way back, and the turn under which it is put to nothing.
const RETURN_RATE: f32 = 15.0;
const RETURN_END: f32 = 0.01;
/// The camera's pitch below level, in degrees, from which a sprinting
/// character cannot look further down.
const SPRINT_DOWN: [f32; 2] = [50.0, 90.0];

/// What a frame's free look is told beside the mouse's travel.
#[derive(Debug, Clone, Copy, Default)]
pub struct HeadLookInput {
    /// The mouse's travel given to the look on this frame, in degrees:
    /// across (to the right positive) and down.
    pub travel: [f32; 2],
    /// The frame's length in seconds, by the engine's clock.
    pub dt: f32,
    /// The sights are up (a character an AI plays is never taken so).
    pub aiming: bool,
    /// The camera's pitch below level as last drawn, in degrees, and
    /// whether the sprint is on.
    pub camera_pitch: f32,
    pub sprinting: bool,
}

#[derive(Debug, Clone)]
pub struct HeadLook {
    pub settings: HeadLookSettings,
    across: f32,
    down: f32,
    /// The mouse has the head (`_mouseLookControl`): from the first frame
    /// that turns it both ways at once until its key comes up.
    control: bool,
    /// The key has come up and the way back is to begin, is under way,
    /// and ends with the next frame.
    reset_asked: bool,
    resetting: bool,
    reset_ends: bool,
    looking: bool,
    rotation: [f32; 3],
}

impl HeadLook {
    pub fn new(settings: HeadLookSettings) -> Self {
        Self {
            settings,
            across: 0.0,
            down: 0.0,
            control: false,
            reset_asked: false,
            resetting: false,
            reset_ends: false,
            looking: false,
            rotation: [0.0; 3],
        }
    }

    /// The key has come up (`ResetLookDirection`): the next frame takes
    /// the head from the mouse and starts it back.
    pub fn reset(&mut self) {
        self.reset_asked = true;
    }

    /// One frame (`Player.Look`). The game calls it every frame its keys
    /// reach the player, with whatever the look's two axes gave, nothing
    /// included.
    pub fn frame(&mut self, input: &HeadLookInput) {
        let [mut travel_across, mut travel_down] = input.travel;
        let (mut across, mut down) = (self.settings.across, self.settings.down);
        if input.aiming {
            across = [across[0] * self.settings.aiming, across[1] * self.settings.aiming];
        }
        if (SPRINT_DOWN[0]..=SPRINT_DOWN[1]).contains(&input.camera_pitch) && input.sprinting {
            down[1] = 0.0;
        }
        // The engine's clamp: the lower bound first.
        let clamp = |value: f32, [least, most]: [f32; 2]| if value < least { least } else { value.min(most) };
        self.across = clamp(self.across - travel_across, across);
        self.down = clamp(self.down + travel_down, down);
        // Looking down is cut back as the head turns aside: all of it
        // straight ahead, none at the turn's limit. It is worked out
        // before this frame's share of the way back.
        let aside = self.across / across[1];
        let down_shown = if self.down > 0.0 { self.down * (1.0 - aside * aside) } else { self.down };
        if std::mem::take(&mut self.reset_ends) {
            self.resetting = false;
        }
        if std::mem::take(&mut self.reset_asked) {
            (self.control, self.resetting) = (false, true);
            (travel_across, travel_down) = (0.0, 0.0);
        }
        // The mouse has the head from a frame that turns it both ways.
        if travel_across.abs() >= f32::EPSILON && travel_down.abs() >= f32::EPSILON {
            self.control = true;
        }
        if !self.control {
            let back = |turn: f32| if turn.abs() > RETURN_END { turn + (0.0 - turn) * (input.dt * RETURN_RATE).clamp(0.0, 1.0) } else { 0.0 };
            (self.across, self.down) = (back(self.across), back(self.down));
        }
        self.looking = !self.resetting && self.across != 0.0 && self.down != 0.0;
        if self.across == 0.0 && self.down == 0.0 {
            self.reset_ends = true;
        }
        self.rotation = [down_shown, self.across, 0.0];
    }

    /// The head's turn as the camera takes it (`Player.HeadRotation`):
    /// added to the camera's rotation spring's figure, about its x for
    /// down and its y for across.
    pub fn rotation(&self) -> [f32; 3] {
        self.rotation
    }

    /// The same as the neck's and the head's bones take it
    /// (`ProceduralWeaponAnimation.GetHeadRotation`): each is turned by
    /// half of it in its own axes (`Body::set_head_turn`).
    pub fn bones_turn(&self) -> [f32; 3] {
        [self.rotation[1], 0.0, -self.rotation[0]]
    }

    /// Whether the mouse has the head (`MouseLookControl`): the aimed
    /// field of view gives way to the hip's while it has.
    pub fn control(&self) -> bool {
        self.control
    }

    /// Whether the head is turned both ways and not on its way back, and
    /// whether it is on its way back (`IsLooking`, `IsResettingLook`).
    pub fn looking(&self) -> bool {
        self.looking
    }

    pub fn resetting(&self) -> bool {
        self.resetting
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 120.0;

    fn frame(head: &mut HeadLook, travel: [f32; 2]) {
        head.frame(&HeadLookInput { travel, dt: DT, ..HeadLookInput::default() });
    }

    #[test]
    fn the_head_turns_with_the_mouse_and_stays_until_the_key_comes_up() {
        let mut head = HeadLook::new(HeadLookSettings::default());
        // To the right and up, a degree and half a degree a frame.
        for _ in 0..20 {
            frame(&mut head, [1.0, -0.5]);
        }
        assert!(head.control() && head.looking());
        assert_eq!(head.rotation(), [-10.0, -20.0, 0.0]);
        // The mouse still: the head stays where it is.
        for _ in 0..60 {
            frame(&mut head, [0.0, 0.0]);
        }
        assert_eq!(head.rotation(), [-10.0, -20.0, 0.0]);
        // The key up: back by 15 a second of what is left, to nothing.
        head.reset();
        frame(&mut head, [0.0, 0.0]);
        assert!(!head.control() && head.resetting() && !head.looking());
        assert!((head.rotation()[1] + 20.0 * (1.0 - DT * 15.0)).abs() < 1e-4, "{:?}", head.rotation());
        // What is shown of the turn down is a frame behind the turn itself.
        assert_eq!(head.rotation()[0], -10.0);
        let mut frames = 1;
        while head.rotation() != [0.0; 3] {
            frame(&mut head, [0.0, 0.0]);
            frames += 1;
            assert!(frames < 400);
        }
        println!("back to nothing {:.2} s after the key came up", frames as f32 * DT);
        assert!(frames as f32 * DT > 0.3 && frames as f32 * DT < 0.8);
        frame(&mut head, [0.0, 0.0]);
        frame(&mut head, [0.0, 0.0]);
        assert!(!head.resetting());
    }

    #[test]
    fn a_turn_one_way_only_never_takes_the_head() {
        let mut head = HeadLook::new(HeadLookSettings::default());
        // Across alone: the way back runs against it on every frame.
        for _ in 0..240 {
            frame(&mut head, [1.0, 0.0]);
        }
        assert!(!head.control() && !head.looking());
        // It settles where a frame's travel is what a frame gives back.
        let rests = head.rotation()[1];
        println!("a degree a frame across alone holds the head at {rests:.2} degrees");
        assert!(rests < -6.0 && rests > -8.0, "{rests}");
        for _ in 0..240 {
            frame(&mut head, [0.0, 0.0]);
        }
        assert_eq!(head.rotation(), [0.0; 3]);
    }

    #[test]
    fn the_turn_is_held_within_its_limits() {
        let mut head = HeadLook::new(HeadLookSettings::default());
        for _ in 0..100 {
            frame(&mut head, [-1.0, 1.0]);
        }
        // To the left as far as it goes, where no look down is left.
        assert_eq!(head.rotation(), [0.0, 40.0, 0.0]);
        let mut head = HeadLook::new(HeadLookSettings::default());
        for _ in 0..100 {
            frame(&mut head, [0.001, 1.0]);
        }
        assert!((head.rotation()[0] - 20.0).abs() < 0.01, "{:?}", head.rotation());
        for _ in 0..100 {
            frame(&mut head, [0.001, -1.0]);
        }
        assert_eq!(head.rotation()[0], -50.0);
        // With the sights up the head turns seven tenths as far aside.
        let mut head = HeadLook::new(HeadLookSettings::default());
        for _ in 0..100 {
            head.frame(&HeadLookInput { travel: [1.0, -0.001], dt: DT, aiming: true, ..HeadLookInput::default() });
        }
        assert!((head.rotation()[1] + 28.0).abs() < 1e-4, "{:?}", head.rotation());
        // Sprinting with the camera far down, nothing further down.
        let mut head = HeadLook::new(HeadLookSettings::default());
        for _ in 0..10 {
            head.frame(&HeadLookInput { travel: [0.5, 1.0], dt: DT, camera_pitch: 60.0, sprinting: true, ..HeadLookInput::default() });
        }
        assert_eq!(head.rotation()[0], 0.0);
    }
}
