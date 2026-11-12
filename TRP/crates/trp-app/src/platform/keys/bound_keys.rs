//! The bindings, the keys heard each frame and the scripted mouse.

use super::*;

/// The bindings, and what the keys came to on this frame.
#[derive(Resource)]
pub struct Bindings {
    /// As they stand.
    pub controls: Controls,
    /// As the install has them, to go back to.
    pub install: Controls,
    preset: Preset,
    pub(super) keys: HashMap<Code, Key>,
    /// Every key that can be chosen for a binding, lowest number first.
    pub valid: Vec<Code>,
    heard: Heard,
    /// What the mouse's travel on this frame was given to, in its counts:
    /// the view's turn, or the head's free look.
    pub(super) mouse: MouseHeard,
    index: HashMap<String, usize>,
    captions: HashMap<String, String>,
    /// Nothing is heard: a key is being chosen for a binding.
    pub muted: bool,
    /// Nothing is heard: a text field has the keys (a tag being named).
    pub typing: bool,
    /// Nothing is heard: the MASTERING page's search field has the keys
    /// (`mastery_view`).
    pub searching: bool,
    /// How many times the bindings have been changed: what is drawn of
    /// them is drawn afresh when this moves.
    pub version: u32,
    /// The wheel's input to a variable scope's zoom on this frame
    /// (`trp_sim::sights::zoom_input`).
    zoom: f32,
    /// While a screen takes every command but these (the gestures' menu:
    /// `GesturesMenu.TranslateCommand`), only these are given, and no axis
    /// is pushed and the mouse turns nothing (`TranslateAxes`).
    pub only: Option<&'static [&'static str]>,
    /// The wheel is a list's (the gestures' held list), not the bindings'.
    pub wheel_taken: bool,
}

impl Bindings {
    pub fn of(install: &trp_data::Install) -> Self {
        let controls = install_controls(install);
        let keys = engine_keys();
        let mut bindings = Self {
            install: controls.clone(),
            preset: Preset::new(&controls),
            controls,
            valid: keys.iter().map(|(code, _)| *code).collect(),
            keys: keys.into_iter().collect(),
            heard: Heard::default(),
            mouse: MouseHeard::default(),
            index: HashMap::new(),
            captions: HashMap::new(),
            muted: false,
            typing: false,
            searching: false,
            version: 0,
            zoom: 0.0,
            only: None,
            wheel_taken: false,
        };
        bindings.changed();
        bindings
    }

    /// The bindings have been changed: the combinations are made afresh,
    /// as the game makes them when its settings are saved.
    pub fn changed(&mut self) {
        self.version += 1;
        // The bindings the game makes of others, afresh (the throws').
        self.controls.generate();
        self.preset = Preset::new(&self.controls);
        self.heard = Heard::default();
        self.index = self.controls.keys.iter().enumerate().map(|(at, group)| (group.name.clone(), at)).collect();
        let first = |group: &KeyGroup| group.variants.iter().find(|variant| !variant.is_empty()).map(words).unwrap_or_default();
        self.captions = self.controls.keys.iter().map(|group| (group.name.clone(), first(group))).collect();
    }

    /// The words for a binding's first key (`2`, `TAB`).
    pub fn caption(&self, name: &str) -> &str {
        self.captions.get(name).map_or("", String::as_str)
    }

    /// A binding's first key as the game names it in a notice
    /// (`ControlSettingsGroup.GetKeyName`: `InputSource.ToString` without
    /// its quotes, its keys in the order kept, each by
    /// `KeyTools.GetKeyNameAlias`, " + " between): `T + LShift`. None where
    /// nothing is bound (the notice then says `Settings/NotSet`). A wheel's
    /// axis is left out (the game names it first; none of the notices'
    /// keys is one).
    pub fn notice_name(&self, name: &str) -> Option<String> {
        let group = self.controls.keys.get(*self.index.get(name)?)?;
        let source = group.variants.iter().find(|variant| !variant.is_empty())?;
        Some(source.keys.iter().map(|key| key_alias(&trp_sim::base::bindings::name(*key))).collect::<Vec<_>>().join(" + "))
    }

    /// Whether the binding is given on this frame: the game's command
    /// for it comes.
    pub fn fired(&self, name: &str) -> bool {
        self.lets(name) && self.index.get(name).is_some_and(|group| self.heard.given.contains(group))
    }

    /// Whether a binding is let through to whatever listens to it (`only`).
    fn lets(&self, name: &str) -> bool {
        self.only.is_none_or(|only| only.contains(&name))
    }

    /// Whether a binding that holds is taken back on this frame: its keys
    /// came up, where it is bound to be held.
    pub fn ended(&self, name: &str) -> bool {
        self.lets(name) && self.index.get(name).is_some_and(|group| self.heard.taken_back.contains(group))
    }

    /// The mouse's travel on this frame as the manager gave it out, in
    /// counts across and down: to the turn, or to the head's free look.
    pub fn mouse(&self) -> MouseHeard {
        if self.only.is_some() {
            return MouseHeard::default();
        }
        self.mouse
    }

    /// The wheel's input to a variable scope's zoom on this frame: its turn
    /// times the options' `OpticSensitivity`, with the zoom's keys held
    /// (optics spec, "Zoom and modes").
    pub fn zoom(&self) -> f32 {
        if self.only.is_some() {
            return 0.0;
        }
        self.zoom
    }

    /// An axis's push on this frame, -1 to 1.
    pub fn axis(&self, name: &str) -> f32 {
        if self.only.is_some() {
            return 0.0;
        }
        self.controls.axes.iter().position(|axis| axis.name == name).and_then(|at| self.heard.axes.get(at)).copied().unwrap_or(0.0)
    }

    /// Whether a key is down, by the game engine's number for it.
    pub fn held(&self, code: Code, keys: &ButtonInput<KeyCode>, buttons: &ButtonInput<MouseButton>) -> bool {
        match self.keys.get(&code) {
            Some(Key::Board(key)) => keys.pressed(*key),
            Some(Key::Mouse(button)) => buttons.pressed(*button),
            None => false,
        }
    }
}

/// Runs the game's input manager for this frame: which bindings the keys
/// and the wheel give, and which they take back. First in the frame, as
/// the game's own runs before what listens to it.
pub fn hear(
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    scroll: Res<AccumulatedMouseScroll>,
    motion: Res<AccumulatedMouseMotion>,
    scripted: Res<ScriptedMouse>,
    options: Res<crate::Options>,
    time: Res<Time<Real>>,
    mut bindings: ResMut<Bindings>,
    (panel, controller): (Option<Res<crate::panels::hud::GuideShown>>, Res<crate::player::controller::Controller>),
) {
    let bindings = &mut *bindings;
    let map = &bindings.keys;
    // While a key is being chosen every key counts as up, so that what
    // was held is let go of and nothing new is heard.
    let muted = bindings.muted || bindings.typing || bindings.searching;
    // With the debug panel open the mouse is its buttons': its travel and
    // its buttons are not the character's (`debug_menu`).
    let mouse_muted = muted || (!options.unattended() && crate::panels::hud::panel_open(panel.as_deref(), &controller));
    let held = |code: Code| {
        !muted
            && match map.get(&code) {
                Some(Key::Board(key)) => keys.pressed(*key),
                Some(Key::Mouse(button)) => !mouse_muted && buttons.pressed(*button),
                None => false,
            }
    };
    // The game takes the wheel for pressed on a frame whose axis reads
    // exactly one either way (`InputAxis.Update`), and the install's
    // input settings have that axis read one for a notch (`Mouse Scroll`,
    // one a count): one notch in a frame is a press, and two in the same
    // frame are none.
    let wheel = if mouse_muted || bindings.wheel_taken { 0 } else { notch(scroll.delta.y) };
    bindings.heard = bindings.preset.update(&held, wheel, time.delta_secs());
    // A variable scope's zoom reads the wheel itself, by its turn, past the
    // commands (`ScopeZoomHandler.GetZoomInput`).
    let turned = if mouse_muted || bindings.wheel_taken { 0.0 } else { scroll.delta.y };
    bindings.zoom = trp_sim::weapons::sights::zoom_input(&bindings.controls, &held, turned, options.optic_sensitivity, options.invert[1]);
    // The mouse's travel, which is the turn's unless the free look's keys
    // give it to the head (`Preset::mouse`). A run nobody is at the keys
    // of has the travel it was told (`--mouse`) in place of the mouse's.
    let travel = if options.unattended() { scripted.0.unwrap_or_default() } else { motion.delta };
    bindings.mouse = if mouse_muted { MouseHeard::default() } else { bindings.preset.mouse([travel.x, travel.y]) };
}

/// The mouse's travel on this frame in a run nobody is at the keys of:
/// what `--mouse` told, and nothing on the frames it did not name. A run
/// someone plays never looks at it: its mouse is the real one.
#[derive(Resource, Default)]
pub struct ScriptedMouse(pub Option<Vec2>);

/// The wheel as the game's input manager takes it: a notch one way or the
/// other on a frame whose turn is exactly one notch, and nothing on a
/// frame with more or less (settings spec, "How a key becomes a command").
pub fn notch(turned: f32) -> i32 {
    if turned == 1.0 {
        1
    } else if turned == -1.0 {
        -1
    } else {
        0
    }
}
