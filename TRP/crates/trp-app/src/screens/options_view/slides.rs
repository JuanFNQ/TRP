//! Slides, cells, notices, and the state's slider, waiting and naming.

use super::*;

/// A bar that is slid: the field of view, and the four of the game's
/// controls tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Slide {
    Fov,
    GameFps,
    Sensitivity,
    Aiming,
    DoubleClick,
    Optic,
    /// A skill's level on the SKILLS page.
    Skill(trp_sim::condition::skills::Skill),
    /// A weapon family's mastering on the SKILLS page, by its place among
    /// the install's families, as the game shows it (Level 1 to 3).
    Mastering(u16),
    /// The character's level on the SKILLS page, 1 to the install's top
    /// (the table's length, carried with it).
    Level(u16),
    /// The range's time of day under the FEATURES page's THE RANGE
    /// (`range_day`).
    Hour,
    /// A volume of the SOUNDS page, the game's eleven steps (0 the overall,
    /// 1 the interface's).
    Volume(u8),
}

impl Slide {
    /// The game's own limits for each (`ControlSettingsGroup`,
    /// `GameSettingsGroup`).
    pub(super) fn range(self) -> [f32; 2] {
        match self {
            Self::Fov => FOV,
            Self::GameFps => [GAME_FPS[0] as f32, GAME_FPS[1] as f32],
            Self::Sensitivity | Self::Aiming => [0.1, 5.0],
            Self::DoubleClick => [0.3, 1.0],
            Self::Optic => [1.0, 5.0],
            Self::Skill(_) => [0.0, trp_sim::condition::skills::ELITE as f32],
            Self::Mastering(_) => [1.0, trp_sim::condition::mastering::TOP_LEVEL as f32 + 1.0],
            Self::Level(top) => [1.0, top.max(1) as f32],
            Self::Hour => [0.0, 24.0],
            Self::Volume(_) => [0.0, 10.0],
        }
    }

    /// What a bar is slid by: whole degrees, as the game's field of view
    /// is; hundredths for the rest (the app's: the game's bars are not
    /// stepped).
    pub(super) fn step(self) -> f32 {
        match self {
            Self::Fov | Self::GameFps | Self::Skill(_) | Self::Mastering(_) | Self::Level(_) | Self::Volume(_) => 1.0,
            // Five minutes.
            Self::Hour => 1.0 / 12.0,
            _ => 0.01,
        }
    }

    /// The figure as the game's own bar writes it (three places for the
    /// two sensitivities, two for the others).
    pub(super) fn shown(self, value: f32) -> String {
        match self {
            Self::Fov | Self::GameFps => format!("{value:.0}"),
            Self::Skill(_) if value >= trp_sim::condition::skills::ELITE as f32 => "ELITE".to_owned(),
            Self::Skill(_) | Self::Mastering(_) | Self::Level(_) => format!("{value:.0}"),
            Self::Sensitivity | Self::Aiming => format!("{value:.3}"),
            Self::DoubleClick | Self::Optic => format!("{value:.2}"),
            Self::Hour => {
                let minutes = (value * 60.0).round() as u32;
                format!("{:02}:{:02}", minutes / 60, minutes % 60)
            }
            // Its step and its level on the game's mixer (`sound.md`, "How
            // loud").
            Self::Volume(_) => format!("{value:.0}  ({:.0} dB)", crate::audio::ui_sound::Levels::DECIBELS[(value.round().max(0.0) as usize).min(10)]),
        }
    }

    /// The game's own words for it, by the name its texts have them
    /// under, and the app's where the install has none.
    pub(super) fn caption(self) -> (&'static str, &'static str) {
        match self {
            Self::Fov => ("", "FIELD OF VIEW"),
            Self::GameFps => ("Game FPS Limit", "GAME FPS LIMIT"),
            Self::Sensitivity => ("Mouse sensitivity", "MOUSE SENSITIVITY"),
            Self::Aiming => ("Mouse sensitivity (aiming)", "MOUSE SENSITIVITY (AIMING)"),
            Self::DoubleClick => ("Settings/DoubleClickTimeout", "DOUBLE CLICK TIMEOUT"),
            Self::Optic => ("Settings/OpticSensitivity", "SCOPE ZOOM ADJUSTMENT SENSITIVITY"),
            Self::Skill(skill) => (skill.name(), skill.name()),
            Self::Mastering(_) => ("", "WEAPON MASTERING"),
            Self::Level(_) => ("", "LEVEL"),
            Self::Hour => ("", "TIME OF DAY"),
            Self::Volume(0) => ("Settings/Sound/OverallVolume", "OVERALL VOLUME"),
            Self::Volume(_) => ("Settings/Sound/InterfaceVolume", "INTERFACE VOLUME"),
        }
    }
}

/// A cell of the list of bindings that takes keys: a variant of a
/// binding, or one way of one of an axis's pairs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Cell {
    Key(usize, usize),
    Axis(usize, usize, usize),
}

/// A key being chosen for a cell.
pub(super) struct Listen {
    pub(super) cell: Cell,
    pub(super) heard: Listening,
    /// The click that began it is over: no key or button is down.
    pub(super) armed: bool,
    /// The choice is made; it waits for every key to be let go.
    pub(super) done: bool,
}

/// Which of the battle screen's notices show: of experience, a search's
/// finds, a thing taken, a thing examined (`statistics::tell`,
/// `examine_view::finish`); and the weapon's malfunctions', made known,
/// looked at or put right (`wear::keep_wear`; their glow stays). The
/// app's own: the game has no such setting and shows them all, as
/// they begin. Taken at once; the experience is counted and the weapon
/// malfunctions the same either way. The warning of a round the chamber
/// does not take had a switch here from 0.207.0 to 0.261.0; it is always
/// shown, as the game shows it (`wear::feedback`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Notices(pub(super) [bool; 8]);

impl Default for Notices {
    fn default() -> Self {
        Self([true; 8])
    }
}

impl Notices {
    pub const SEARCHING: usize = 0;
    pub const LOOTING: usize = 1;
    pub const EXAMINING: usize = 2;
    pub const MALFUNCTIONS: usize = 3;
    /// Since 0.201.0, with the experience earned: healing's and a
    /// treatment's experience, a locked door's, a skill's level reached, a
    /// weapon family's mastering level reached.
    pub const TREATMENT: usize = 4;
    pub const DOORS: usize = 5;
    pub const SKILL_LEVEL: usize = 6;
    pub const MASTERING_LEVEL: usize = 7;
    /// Each, with its row's name and what it says.
    pub(super) const ALL: [(usize, &'static str, &'static str); 8] = [
        (Self::SEARCHING, "SEARCHING XP", "\"Searching experience\": the things a search uncovers."),
        (Self::LOOTING, "LOOTING XP", "\"Looting experience\": the things taken into your gear."),
        (Self::EXAMINING, "EXAMINE XP", "\"Item examined\": a thing learned, with its experience."),
        (Self::TREATMENT, "TREATMENT XP", "\"Treatment experience - Healing\": health restored and wounds treated."),
        (Self::DOORS, "DOORS XP", "\"Doors unlocked\", \"Doors breached\": a locked door opened."),
        (Self::SKILL_LEVEL, "SKILL LEVEL UP", "\"... skill leveled up to ...\": a skill's new level."),
        (Self::MASTERING_LEVEL, "MASTERING LEVEL UP", "\"... mastering leveled up to ...\": a weapon family's new level."),
        (Self::MALFUNCTIONS, "MALFUNCTIONS", "\"Malfunction\", \"Fix malfunction\": your weapon's malfunctions (the glow stays)."),
    ];
    /// Their names in the app's file (under `"notices"`; the first three
    /// once under `"xp_notices"`, still read). A file's `"incompatible_ammo"`
    /// of before 0.262.0 is not read.
    pub(super) const KEYS: [&'static str; 8] = ["searching", "looting", "examining", "malfunctions", "treatment", "doors", "skill_level", "mastering_level"];

    pub fn on(&self, which: usize) -> bool {
        self.0.get(which).copied().unwrap_or(true)
    }

    pub(super) fn set(&mut self, which: usize, on: bool) {
        if let Some(slot) = self.0.get_mut(which) {
            *slot = on;
        }
    }
}

impl OptionsState {
    /// Whether Escape is the tab's own just now: a key is being chosen,
    /// which Escape drops, a way of being pressed is asked for, which
    /// Escape leaves as it was, the search field has the keys, which
    /// Escape takes from it, or the list of voices is open, which Escape
    /// shuts. The inventory screen then does not shut for it.
    pub fn takes_escape(&self) -> bool {
        self.listening.is_some() || self.asking.is_some() || self.search_focused || self.name_focused || self.voice_open
    }

    /// Whether a page's section is shut: none is until its head is
    /// clicked.
    pub(super) fn shut(&self, page: usize, at: usize) -> bool {
        self.folded.contains(&(page, at))
    }

    /// What waits for the next start, by name: each switch thrown since
    /// the run began, and each display setting taken only at a start.
    pub(super) fn waiting(&self) -> Vec<&'static str> {
        let mut names: Vec<&'static str> = Switch::ALL.into_iter().filter(|switch| !switch.at_once() && self.switches.on(*switch) != self.switches_started.on(*switch)).map(|switch| switch.words().0).collect();
        let graphics = &self.graphics;
        for (changed, name) in [
            ((graphics.texture_quality, graphics.mip_streaming) != self.textures_started, "TEXTURES"),
            (graphics.shadow_quality != self.shadows_started, "SHADOWS"),
            (graphics.ssao != self.ssao_started, "SSAO"),
            (graphics.clouds_quality != self.clouds_started, "CLOUDS"),
            (graphics.render_thread != self.render_thread_started, "RENDER THREAD"),
            (graphics.renderer != self.renderer_started, "RENDERER"),
        ] {
            if changed {
                names.push(name);
            }
        }
        names
    }

    /// The game's own words for a line of its controls tab, in capitals,
    /// or the app's where the install has none.
    pub(super) fn said(&self, name: &str) -> String {
        let ours = SAID.iter().find(|(known, _)| *known == name).map_or(name, |(_, ours)| *ours);
        self.names.get(&name.to_lowercase()).map_or_else(|| ours.to_owned(), |words| words.to_uppercase())
    }

    /// The game's word for a way of being pressed.
    pub(super) fn press_word(&self, press: PressType) -> String {
        self.said(&format!("EPressType/{}", press.name()))
    }

    /// A bar's figure as it stands. The double click's time is the
    /// bindings' own.
    pub(super) fn slider(&self, kind: Slide, controls: &Controls) -> f32 {
        match kind {
            Slide::Fov => self.graphics.field_of_view,
            Slide::GameFps => self.graphics.game_fps as f32,
            Slide::Sensitivity => self.mouse.sensitivity,
            Slide::Aiming => self.mouse.aiming,
            Slide::DoubleClick => controls.double_click,
            Slide::Optic => self.mouse.optic,
            Slide::Skill(skill) => self.skills.level(skill) as f32,
            Slide::Mastering(at) => self.families.all().get(at as usize).map_or(0, |family| self.mastering.level(family)) as f32 + 1.0,
            Slide::Level(_) => self.level_table.level(self.experience) as f32,
            Slide::Hour => self.hour,
            Slide::Volume(0) => self.sound.overall as f32,
            Slide::Volume(_) => self.sound.interface as f32,
        }
    }

    pub(super) fn set_slider(&mut self, kind: Slide, value: f32, controls: &mut Controls) {
        match kind {
            Slide::Fov => self.graphics.field_of_view = value,
            Slide::GameFps => self.graphics.game_fps = (value.round() as u32).clamp(GAME_FPS[0], GAME_FPS[1]),
            Slide::Sensitivity => self.mouse.sensitivity = value,
            Slide::Aiming => self.mouse.aiming = value,
            Slide::DoubleClick => controls.double_click = value,
            Slide::Optic => self.mouse.optic = value,
            Slide::Skill(skill) => self.skills.set_level(skill, value.round().max(0.0) as u32),
            Slide::Mastering(at) => {
                // Set to the start of the level shown; a family not begun
                // left so at Level 1 (set, it is begun: `ChangeMasteringLevel`).
                let level = (value.round().max(1.0) as u32 - 1).min(trp_sim::condition::mastering::TOP_LEVEL);
                if let Some(family) = self.families.all().get(at as usize).cloned() {
                    if self.mastering.level(&family) != level && (level > 0 || self.mastering.points(&family.name).is_some()) {
                        self.mastering.set_level(&family, level);
                    }
                }
            }
            // The start of the level chosen (`ProfileInfo.GetExperience`).
            Slide::Level(_) => self.experience = self.level_table.experience_at(value.round().max(1.0) as u32),
            Slide::Hour => self.hour = value.clamp(0.0, 24.0),
            Slide::Volume(which) => {
                let step = value.round().clamp(0.0, 10.0) as u8;
                match which {
                    0 => self.sound.overall = step,
                    _ => self.sound.interface = step,
                }
            }
        }
    }

    /// The game's words for a binding, in capitals, or its own name
    /// spaced out where the install has none.
    pub(super) fn name_of(&self, name: &str) -> String {
        match self.names.get(&name.to_lowercase()) {
            Some(words) => words.to_uppercase(),
            None => {
                let mut spaced = String::new();
                for letter in name.chars() {
                    if letter.is_uppercase() && !spaced.is_empty() {
                        spaced.push(' ');
                    }
                    spaced.extend(letter.to_uppercase());
                }
                spaced
            }
        }
    }

    /// The tactical key's activation mode changed in play (T+B: the game
    /// flips it and saves its settings at once), and kept.
    pub fn set_tactical_input(&mut self, mode: trp_sim::weapons::tactical::InputMode, controls: &Controls) {
        self.game.tactical = mode;
        self.keep(controls);
    }

    pub(super) fn keep(&mut self, controls: &Controls) {
        if let Some(path) = &self.file {
            if let Err(e) = save(path, controls, &self.graphics, &self.mouse, &self.switches, (&self.game, &self.notices, self.hide_tabs, self.experience), &self.sound, &self.skills, &self.mastering) {
                self.note = format!("not kept: {} could not be written ({e})", path.display());
            }
        }
    }
}

/// The app's words for an axis's two ways.
pub(super) const AXIS_WORDS: [(&str, [&str; 2]); 3] =
    [("MoveX", ["MOVE RIGHT", "MOVE LEFT"]), ("MoveY", ["MOVE FORWARD", "MOVE BACKWARD"]), ("LeanX", ["LEAN RIGHT SLOWLY", "LEAN LEFT SLOWLY"])];

/// What the game's controls tab says besides its bindings, each by the
/// name the install's texts have it under, with the app's words for an
/// install that has none.
pub(super) const SAID: [(&str, &str); 33] = [
    ("Settings/Graphics/CloudsQuality", "CLOUDS QUALITY:"),
    ("Object LOD quality:", "OBJECT LOD QUALITY:"),
    ("CloudsMode/Low", "LOW"),
    ("CloudsMode/Medium", "MEDIUM"),
    ("CloudsMode/High", "HIGH"),
    ("Settings/Sound/OverallVolume", "OVERALL VOLUME:"),
    ("Game FPS Limit", "GAME FPS LIMIT:"),
    ("ESSAOMode/Off", "OFF"),
    ("ESSAOMode/FastestPerformance", "MAX PERFORMANCE"),
    ("ESSAOMode/FastPerformance", "HIGH PERFORMANCE"),
    ("ESSAOMode/HighQuality", "HIGH"),
    ("ESSAOMode/HighestQuality", "ULTRA"),
    ("ESSAOMode/ColoredHighestQuality", "COLORED ULTRA"),
    ("Settings/Sound/InterfaceVolume", "INTERFACE VOLUME:"),
    ("Ui/Settings/QuickSlotsVisibility", "QUICK SLOTS"),
    ("Ui/Settings/StanimaVisibility", "STAMINA AND STANCE"),
    ("Ui/Settings/HeathVisibility", "HEALTH CONDITION"),
    ("Ui/Settings/HealthColorScheme", "HEALTH COLOR SCHEME"),
    ("Ui/Settings/Visibility/Autohide", "AUTOHIDE"),
    ("Ui/Settings/Visibility/Always", "ALWAYS SHOWN"),
    ("Ui/Settings/Visibility/Never", "ALWAYS HIDDEN"),
    ("Ui/Settings/ColorScheme/WhiteToRed", "MONOCHROME"),
    ("Ui/Settings/ColorScheme/GreenToRed", "POLYCHROME"),
    ("EPressType/Release", "RELEASE"),
    ("EPressType/Continuous", "CONTINUOUS"),
    ("EPressType/Press", "PRESS"),
    ("EPressType/DoubleClick", "DOUBLE CLICK"),
    ("Mouse sensitivity", "MOUSE SENSITIVITY"),
    ("Mouse sensitivity (aiming)", "MOUSE SENSITIVITY (AIMING)"),
    ("Settings/DoubleClickTimeout", "DOUBLE CLICK TIMEOUT"),
    ("Settings/OpticSensitivity", "SCOPE ZOOM ADJUSTMENT SENSITIVITY"),
    ("Inverted X axis", "INVERTED X AXIS"),
    ("Inverted Y axis", "INVERTED Y AXIS"),
];
