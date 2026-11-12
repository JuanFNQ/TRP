//! The options' state and how it starts.

use super::*;

#[derive(Resource)]
pub struct OptionsState {
    /// Which page is shown (`PAGES`).
    pub(super) page: usize,
    pub(super) listening: Option<Listen>,
    /// The binding whose way of being pressed is being chosen.
    pub(super) asking: Option<usize>,
    pub(super) note: String,
    /// How far down the page is scrolled, in pixels, and where its bar's
    /// handle is held from its top while it is dragged.
    pub(super) page_scroll: f32,
    pub(super) page_held: Option<f32>,
    /// The CONTROLS page's category (`CATEGORIES`), what is typed in its
    /// search field, and whether the field has the keys.
    pub(super) category: usize,
    pub(super) search: String,
    pub(super) search_focused: bool,
    /// The character's name, which the OVERALL tab shows (set on the
    /// FEATURES page and changed at any time; kept as `"character_name"`, the newest
    /// profile's nickname until one is typed), and whether its field has
    /// the keys.
    pub character_name: String,
    pub(super) name_focused: bool,
    /// The cell of the bindings last clicked, which CLEAR KEY clears.
    pub(super) selected_cell: Option<Cell>,
    /// The sections folded shut, by their page and their place on it:
    /// none as the run begins, every section open.
    pub(super) folded: std::collections::HashSet<(usize, usize)>,
    /// Which of the battle screen's notices show (the HUD page's
    /// NOTIFICATIONS).
    pub notices: Notices,
    /// The inventory's MAP, TASKS, ACHIEVEMENTS and PRESTIGE tabs taken off
    /// its bar (the HUD page's HIDE UNUSED TABS, the app's own; on as it
    /// comes), at once.
    pub hide_tabs: bool,
    /// What F1 to F12 say or show in play, bound from the gestures' menu
    /// alone, not the options tab (`gestures`; kept as `"gesture_binds"`),
    /// 0 for nothing.
    pub gesture_binds: [i32; 12],
    /// The character's experience (the SKILLS page's LEVEL; the app's file
    /// keeps it as `"experience"`), the install's table of the levels it
    /// makes (set by `main`), and a save of what play changed waiting for
    /// its time (`keep_in_play`: at most every two seconds).
    pub experience: i64,
    pub level_table: trp_sim::condition::experience::LevelTable,
    pub(super) keep_due: bool,
    pub(super) kept_at: f64,
    pub graphics: Graphics,
    /// The FEATURES page's switches as they are kept for the next run, and
    /// as this run began (a switch thrown away from it says so).
    pub switches: Switches,
    pub switches_started: Switches,
    /// The fields as they hold for this run: the switches' as it began,
    /// with the command line's over them, and each part of a group that is
    /// off held off (`Features::effective`). What the app goes by.
    pub in_force: Features,
    /// The mouse's settings, and what the install's own game has for them.
    pub mouse: Mouse,
    pub(super) install_mouse: Mouse,
    /// The display's settings came from the app's file and are to be
    /// put on the window as the run begins.
    pub(super) apply_at_start: bool,
    /// What the install's own game has for them.
    pub(super) install_graphics: Graphics,
    /// The settings of the game's own GAME tab the battle screen's panels
    /// go by (`battle_view::BattleModes`), as they stand and as the
    /// install has them.
    pub game: crate::panels::battle_view::BattleModes,
    pub(super) install_game: crate::panels::battle_view::BattleModes,
    /// The range's time of day (the FEATURES page's TIME OF DAY), in hours,
    /// and as the run began: the range's own light
    /// follows it at once (`range_day`), Customs' is set by it at the start
    /// (`CUSTOMS LIGHT`). Not kept: every run begins at
    /// `lighting::RANGE_MOMENT`'s hour, 09:30, or `--hour`'s.
    pub hour: f32,
    pub hour_started: f32,
    /// The game's volumes of its sound settings the app's sounds go by
    /// (`ui_sound::Levels`), as they stand and as the install has them.
    pub sound: crate::audio::ui_sound::Levels,
    pub(super) install_sound: crate::audio::ui_sound::Levels,
    /// The SOUNDS page's PLAYER VOICE: the PMC voices the profile has, of both sides
    /// (`Install::profile_voices`), each by its name (`Bear_1`) with the
    /// game's words for it (`ALEX`) and whether it is a USEC's; the one
    /// spoken with (kept as `"voice"` by `voice_sound`, where the profile
    /// still has it), the profile's own, and whether the list is open.
    pub voices: Vec<(String, String, bool)>,
    pub voice: Option<String>,
    pub(super) profile_voice: Option<String>,
    pub(super) voice_open: bool,
    /// The pictures' quality and streaming as this run began, which it
    /// holds.
    pub textures_started: (u8, bool),
    /// The shadows' quality as this run began, which it holds.
    pub shadows_started: u8,
    /// The ambient occlusion's choice as this run began, which it holds.
    pub ssao_started: trp_sim::lighting::hbao::SsaoMode,
    /// The clouds' quality as this run began, which it holds.
    pub clouds_started: trp_sim::lighting::clouds::CloudsQuality,
    /// Whether this run renders on a thread of its own, which it holds.
    pub render_thread_started: bool,
    /// The backend as this run began, which it holds.
    pub renderer_started: crate::platform::backend::Renderer,
    /// The page is to be drawn afresh.
    pub(super) dirty: bool,
    /// Where what is changed is kept; none in a run nobody is at the
    /// keys of, which neither reads nor writes it.
    pub(crate) file: Option<PathBuf>,
    /// The game's own words for the bindings, by their names in small
    /// letters (the install's English texts).
    pub(super) names: HashMap<String, String>,
    /// The bar held by the mouse.
    pub(super) sliding: Option<Slide>,
    /// The character's skills as the SKILLS page sets them, and as a host
    /// game changes them in play (`skills::Skills`): each one's progress,
    /// kept in the app's file whole, fractions of a level and all.
    pub skills: trp_sim::condition::skills::SkillLevels,
    /// The install's weapon families, and each begun one's points as the
    /// SKILLS page sets them and as a host game changes them in play
    /// (`skills::Mastery`), kept in the app's file in the order begun.
    pub families: std::sync::Arc<trp_sim::condition::mastering::Families>,
    pub mastering: trp_sim::condition::mastering::MasteringLevels,
}

impl OptionsState {
    /// The options as the run begins: the display as the app starts it
    /// (a window of `size`, the frames waiting for the screen unless told
    /// otherwise), and, for someone at the keys, what the app's own file
    /// has over that and over the install's bindings. The field of view
    /// given on the command line stays.
    pub fn new(options: &mut Options, install: &trp_data::Install, words: &HashMap<String, String>, bindings: &mut Bindings, size: [u32; 2], families: trp_sim::condition::mastering::Families) -> Self {
        let read = install.display_settings();
        // The install's own choice of ambient occlusion, or the game's
        // default (`GraphicsSettingsGroup`: off).
        let install_ssao = read.ssao.as_deref().and_then(trp_sim::lighting::hbao::SsaoMode::from_key).unwrap_or(trp_sim::lighting::hbao::SsaoMode::Off);
        // The install's clouds' quality, or the game's default
        // (`GraphicsSettingsGroup`: low).
        let install_clouds = read.clouds_quality.and_then(trp_sim::lighting::clouds::CloudsQuality::from_key).unwrap_or(trp_sim::lighting::clouds::CloudsQuality::Low);
        // The game renders a frame behind its main thread where its build
        // has the engine's graphics jobs on (this install's has).
        let install_render_thread = install.graphics_jobs().unwrap_or(true);
        let install_fps = read.game_framerate.unwrap_or(GAME_FPS_DEFAULT).clamp(GAME_FPS[0], GAME_FPS[1]);
        let install_fps_off = read.disable_game_framerate_limit.unwrap_or(false);
        // The install's levels of detail's bias, or the game's own unless
        // set (`GraphicsSettingsGroup.LodBias`: 2), clamped as the game's.
        let install_lod_bias = read.lod_bias.unwrap_or(trp_data::setup::install::LOD_BIAS_DEFAULT);
        // The pictures and the shadows at HIGH, the pictures streamed (the
        // install's own game has it off).
        let mut graphics = Graphics {
            vsync: options.present == "vsync",
            field_of_view: options.field_of_view,
            mode: Mode::Windowed,
            resolution: size,
            texture_quality: crate::assets::streaming::START_QUALITY,
            mip_streaming: true,
            shadow_quality: crate::rendering::shadows::START_QUALITY,
            ssao: install_ssao,
            clouds_quality: install_clouds,
            render_thread: install_render_thread,
            game_fps: install_fps,
            fps_limit_off: install_fps_off,
            msaa: 4,
            dust_resolution: crate::effects::offscreen_dust::DustResolution::DEFAULT,
            renderer: crate::platform::backend::Renderer::DEFAULT,
            lod_bias: install_lod_bias,
        };
        let install_graphics = Graphics {
            vsync: read.vsync.unwrap_or(false),
            field_of_view: install.player_settings().field_of_view.unwrap_or(FOV[0]).clamp(FOV[0], FOV[1]),
            mode: read.mode.map_or(Mode::Borderless, Mode::of_game),
            resolution: read.resolution.unwrap_or(size),
            texture_quality: read.texture_quality.unwrap_or(2),
            mip_streaming: read.mip_streaming.unwrap_or(false),
            shadow_quality: read.shadows_quality.unwrap_or(crate::rendering::shadows::DEFAULT_QUALITY),
            ssao: install_ssao,
            clouds_quality: install_clouds,
            render_thread: install_render_thread,
            game_fps: install_fps,
            fps_limit_off: install_fps_off,
            // (The game multisamples nothing; the app's own is kept.)
            msaa: 4,
            // (The game draws the dust whole.)
            dust_resolution: crate::effects::offscreen_dust::DustResolution::Full,
            // (The game draws through Direct3D 11, which is neither.)
            renderer: crate::platform::backend::Renderer::DEFAULT,
            lod_bias: install_lod_bias,
        };
        // The mouse as the run begins with it: the install's, or what the
        // command line said; and what the install's own game has.
        let mut mouse = Mouse {
            sensitivity: options.sensitivity,
            aiming: options.aiming_sensitivity,
            optic: install.control_setting("OpticSensitivity").unwrap_or(1.0).clamp(1.0, 5.0),
            invert: options.invert,
        };
        let theirs = install.player_settings();
        let install_mouse = Mouse {
            sensitivity: theirs.mouse_sensitivity.unwrap_or(0.7).clamp(0.1, 5.0),
            aiming: theirs.aiming_sensitivity.unwrap_or(0.5).clamp(0.1, 5.0),
            optic: mouse.optic,
            invert: options.invert,
        };
        let file = (!options.unattended()).then(options_file);
        let mut apply_at_start = false;
        let mut switches = Switches::default();
        // The battle screen's panels as the install's own game has them,
        // unless kept otherwise here.
        let install_game = crate::panels::battle_view::BattleModes::read(install);
        let mut game = install_game;
        let install_sound = crate::audio::ui_sound::Levels::read(install);
        let mut sound = install_sound;
        // The skills untrained until the file says otherwise (never the
        // profile's: they are set in the options tab).
        let mut skills = trp_sim::condition::skills::SkillLevels::default();
        // No family begun until the file says otherwise (nor read from the
        // profile, as the skills).
        let mut mastering = trp_sim::condition::mastering::MasteringLevels::default();
        let mut hour = crate::rendering::lighting::RANGE_MOMENT.hour;
        let mut notices = Notices::default();
        // The unused tabs hidden as the app comes.
        let mut hide_tabs = true;
        // A new character's experience: nought, level 1.
        let mut experience = 0;
        if let Some(path) = &file {
            let asked = graphics.field_of_view;
            let asked_mouse = mouse.clone();
            let [controls, display] = load(path, &mut bindings.controls, &mut graphics, &mut mouse, &mut switches, (&mut game, &mut notices, &mut hide_tabs, &mut experience), &mut sound, &mut skills, &mut mastering);
            if controls {
                bindings.changed();
            }
            if options.fov_given {
                graphics.field_of_view = asked;
            }
            if options.sensitivity_given[0] {
                mouse.sensitivity = asked_mouse.sensitivity;
            }
            if options.sensitivity_given[1] {
                mouse.aiming = asked_mouse.aiming;
            }
            (options.sensitivity, options.aiming_sensitivity, options.invert) = (mouse.sensitivity, mouse.aiming, mouse.invert);
            options.optic_sensitivity = mouse.optic;
            if controls || display {
                println!("options: {} read ({}{})", path.display(), if controls { "the keys" } else { "" }, if display { " the display" } else { "" });
            }
            options.field_of_view = graphics.field_of_view;
            apply_at_start = display;
        }
        // The game's own words for its bindings, for the ways of being
        // pressed and for what else its controls tab says.
        let mut wanted: Vec<&str> = bindings.controls.keys.iter().map(|group| group.name.as_str()).collect();
        wanted.extend(SAID.iter().map(|(name, _)| *name));
        wanted.extend(trp_sim::condition::skills::Skill::ALL.map(trp_sim::condition::skills::Skill::name));
        // The families' weapons' short names, which the SKILLS page lists
        // under each family's bar.
        let short: Vec<String> = families.all().iter().flat_map(|family| family.weapons.iter().map(|weapon| format!("{weapon} ShortName"))).collect();
        wanted.extend(short.iter().map(String::as_str));

        let names = trp_data::character::inventory::words_of(words, &wanted);
        // The command line has the last word on the battle screen's panels
        // for this run (`--hud`), as on a switch.
        if let Some(shows) = options.hud {
            (game.quick, game.stamina, game.health) = (shows, shows, shows);
        }
        // The command line has the last word on the hour (`--hour`).
        if let Some(asked) = options.hour {
            hour = asked;
        }
        // The command line has the last word on a switch, as on the rest:
        // a switch of the page by its key, as thrown on the page (on only
        // where what it needs is on; off, what needs it goes with it), and
        // then a field alone by its own key, for this run.
        let mut fields: Vec<(Part, bool)> = Vec::new();
        for (name, on) in &options.feature_switches {
            if let Some(switch) = Switch::ALL.into_iter().find(|switch| switch.key() == name) {
                if !switch.built() && *on {
                    eprintln!("--feature {name}=on: not taken, it is not built");
                } else if !switches.set(switch, *on) {
                    eprintln!("--feature {name}=on: not taken, it needs {} on", switch.requires().iter().map(|need| need.words().0).collect::<Vec<_>>().join(" and "));
                }
            } else if let Some(part) = Part::ALL.into_iter().find(|part| part.key() == name) {
                fields.push((part, *on));
            } else {
                eprintln!("no switch of the features is called {name}: {}; or one part alone: {}", Switch::ALL.map(Switch::key).join(", "), Part::ALL.map(Part::key).join(", "));
            }
        }
        let mut features = switches.features();
        if let Some(on) = options.custom_hands {
            features.custom_hands = on;
        }
        if let Some(on) = options.smooth_animations {
            features.smooth_animations = on;
        }
        if let Some(on) = options.icon_generation {
            features.icon_generation = on;
        }
        for (part, on) in fields {
            *part.slot(&mut features) = on;
        }
        // The command line's levels for this run, over the file's.
        use trp_sim::condition::skills::Skill;
        for class in Skill::WEAPON_CLASSES.into_iter().filter(|_| options.weapon_skill.is_some()) {
            skills.set_level(class, options.weapon_skill.unwrap_or(0));
        }
        if let Some(level) = options.strength {
            skills.set_level(Skill::Strength, level);
        }
        if let Some(level) = options.endurance {
            skills.set_level(Skill::Endurance, level);
        }
        for (name, level) in &options.skill_levels {
            match Skill::of_name(name) {
                Some(skill) => skills.set_level(skill, *level),
                None => eprintln!("no skill is called {name}: {}", Skill::ALL.map(Skill::name).join(", ")),
            }
        }
        // The families the file names that the install has, in their
        // order begun; then the command line's for this run
        // (`--mastering-family`), over the file's.
        let mut known = trp_sim::condition::mastering::MasteringLevels::default();
        for (name, points) in mastering.begun() {
            match families.by_name(name) {
                Some(_) => known.set_points(name, *points),
                None => eprintln!("options: the install has no weapon family {name}"),
            }
        }
        let mut mastering = known;
        for (name, level) in &options.mastering_families {
            match families.by_name(name) {
                Some(family) => mastering.set_level(family, *level),
                None => eprintln!("no weapon family is called {name}: {}", families.all().iter().map(|family| family.name.as_str()).collect::<Vec<_>>().join(", ")),
            }
        }
        let families = std::sync::Arc::new(families);
        let started = features.clone();
        let in_force = started.effective();
        let textures_started = (graphics.texture_quality, graphics.mip_streaming);
        let shadows_started = graphics.shadow_quality;
        let ssao_started = graphics.ssao;
        let clouds_started = graphics.clouds_quality;
        let render_thread_started = options.render_thread.unwrap_or(if options.unattended() { false } else { graphics.render_thread });
        let renderer_started = graphics.renderer;
        // The voices offered, each by the game's words for it (`"<id>
        // Name"`, its own name where the install has none), and the one
        // spoken with.
        let offered = install.profile_voices();
        let profile_voice = install.profile_voice();
        let voice = crate::audio::voice_sound::chosen(&offered, file.as_deref(), profile_voice.clone());
        let voices = offered.iter().map(|v| (v.name.clone(), words.get(&format!("{} name", v.id)).filter(|w| !w.is_empty()).unwrap_or(&v.name).to_uppercase(), v.usec)).collect();
        Self {
            page: 0,
            listening: None,
            asking: None,
            note: String::new(),
            page_scroll: 0.0,
            page_held: None,
            category: 0,
            search: String::new(),
            search_focused: false,
            character_name: file.as_deref().and_then(kept_name).or_else(|| install.profile_name().map(|(nickname, _)| nickname)).unwrap_or_default(),
            name_focused: false,
            selected_cell: None,
            folded: Default::default(),
            notices,
            hide_tabs,
            gesture_binds: file.as_deref().and_then(crate::player::gestures::kept).unwrap_or_else(crate::player::gestures::default_binds),
            experience,
            level_table: trp_sim::condition::experience::LevelTable::default(),
            keep_due: false,
            kept_at: 0.0,
            graphics,
            switches,
            switches_started: switches,
            in_force,
            mouse,
            install_mouse,
            apply_at_start,
            install_graphics,
            game,
            install_game,
            sound,
            install_sound,
            voices,
            voice,
            profile_voice,
            voice_open: false,
            hour,
            hour_started: hour,
            textures_started,
            shadows_started,
            ssao_started,
            clouds_started,
            render_thread_started,
            renderer_started,
            dirty: true,
            file,
            names,
            sliding: None,
            skills,
            families,
            mastering,
        }
    }
}
