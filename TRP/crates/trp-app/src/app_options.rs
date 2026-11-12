//! The run's data, the view and the app's options.

use super::*;

#[derive(Resource)]
pub struct GameData {
    pub unity: UnityAssets,
    /// Shared with the threads that read away from the frame (the weapons'
    /// rigs, the worn things), which once read a database of their own.
    pub db: std::sync::Arc<Database>,
    pub hard: HardSettings,
    pub engine: EngineSettings,
    /// Tuning of the weapon motion, taken by the weapon when it is spawned.
    pub hands_data: Option<HandsData>,
    /// The install's words, every key's by the key in small letters
    /// (`trp_data::inventory::Locale::words`), read once as the run began:
    /// what the menus and the edit build screen ask theirs of
    /// (`trp_data::inventory::words_of`).
    pub words: std::sync::Arc<std::collections::HashMap<String, String>>,
    /// The things the range lays out none of and the benches do not list
    /// (`HIDE UNFINISHED ITEMS`: SPT's `item.json` `blacklist`,
    /// `unfinished_items`), none with the switch off; read once as the run
    /// began.
    pub unfinished: std::sync::Arc<std::collections::HashSet<String>>,
}

#[derive(Resource, Clone, Copy, PartialEq, Eq)]
pub enum View {
    /// Playing: the camera rides the weapon's animated camera bone.
    FirstPerson,
    /// Free camera for looking at the rig from outside.
    Free,
    /// Playing, watched from a camera that stands outside the character
    /// and keeps its place beside it: the keys and the mouse are the
    /// character's, and the picture is what someone else would see of it.
    Watched,
}

impl View {
    /// Whether the keys and the mouse are the character's, and not the
    /// free camera's.
    pub fn played(self) -> bool {
        self != View::Free
    }
}

/// Where the watching camera stands from the character's feet (`View::Watched`).
#[derive(Resource, Default)]
pub(super) struct Watching(pub(super) Option<Vec3>);

#[derive(Resource)]
pub struct Options {
    /// Item template id of the weapon.
    pub weapon: String,
    /// What the airdrop crate holds besides the run's own weapon: those
    /// asked for (`--add-weapon`). Every weapon whose logic is built lies
    /// in the crates of weapons along the knee-high wall (`weapon_crates`).
    pub weapons: Vec<String>,
    /// `--measure-rigs <crate|all>`: the crate of weapons (by its label, or
    /// all of them) whose every weapon's rig is made ready and measured,
    /// not put into the world, before the run ends (`weapon::Measured`);
    /// then their templates.
    pub(super) measure_crate: Option<String>,
    pub measure_rigs: Vec<String>,
    /// Manifest key of the arms bundle.
    pub arms: String,
    /// The bundles of the clothing the body is drawn in: its upper body and
    /// its legs. `none` leaves one out.
    pub body: String,
    pub legs: String,
    pub head: String,
    /// Suits of the install's customization list to begin in, by their ids
    /// (`--suit`, as often as wanted: an upper one, a lower one), over the
    /// choice the options' file keeps.
    pub suits: Vec<String>,
    /// What the character wears over its clothing: item template ids or
    /// bundles, any number.
    pub wear: Vec<String>,
    /// Things put in the vest or the pockets as the run begins, by their
    /// templates (`--carry`, as often as wanted: a testing aid, as
    /// `--damage` is).
    pub carry: Vec<String>,
    /// Things put on as the run begins (`--put-on`, as often as wanted: a
    /// testing aid, as `--carry` is): a template, made as its default preset
    /// has it, or a file holding a build in the database's form (as
    /// `--preset` reads one: a helmet with a light on its rail), worn in the
    /// first slot of the equipment that takes it.
    pub put_on: Vec<String>,
    /// `--shield-hits <n>`: the face shields worn on the head and the face
    /// begin with that many hits (`FaceShieldComponent.Hits`), as if marked;
    /// in a run with a screenshot, each with the seed `--seed` gives, so a
    /// capture repeats. A testing aid, as `--put-on` is: no rule.
    pub shield_hits: Option<u8>,
    /// Things laid on the floor ahead of where the run starts, by their
    /// templates (`--lay`, as often as wanted: a testing aid, as `--carry`
    /// is).
    pub lay: Vec<String>,
    /// How far ahead of the start `--lay` lays them (`--lay-ahead <metres>`,
    /// 0.8 unless told: to see a thing's levels of detail
    /// near and far).
    pub(super) lay_ahead: f32,
    /// The pixels of deviation the levels of detail allow, over the options'
    /// `OBJECT LOD QUALITY` (`--lod-pixels <n>`: a capture's aid since
    /// 0.244.0, to see the switch and its fade near; `lod_view`).
    pub lod_pixels: Option<f32>,
    /// Weapons asked into the crate beside the app's own (`--add-weapon`),
    /// as given: a template, or a part of a name.
    pub(super) added: Vec<String>,
    /// `--preset <id>`: the run's own weapon as another of the database's
    /// builds of it (`ItemPresets`, by its id), in its gun case and in hand
    /// with `--equip`, where the cases hold the stock builds (a testing
    /// aid: the M4A1 with a canted sight is `5a8ae36686f774377d6ce226`).
    pub(super) preset: Option<String>,
    /// Carried weight in kilograms.
    pub(super) weight: f32,
    pub(super) view: View,
    /// Whose eyes the character is posed for, where the run says
    /// (`--pov first|third`); otherwise its own while the picture is its
    /// own, and another's from any camera outside it.
    pub(super) pov: Option<trp_sim::PointOfView>,
    /// Clips to play at start, in order.
    pub(super) play: Vec<String>,
    /// Freeze the first clip at this many seconds.
    pub(super) hold_at: Option<f32>,
    /// Keys held for the whole run, joined by `+`: w a s d shift q e jump aim fire.
    pub hold: Option<String>,
    /// The game's field-of-view setting: vertical degrees, 50 to 75.
    pub field_of_view: f32,
    /// Whether `--fov` gave it, which then has the say over what the
    /// options tab last kept.
    pub(super) fov_given: bool,
    /// The game's head-bobbing setting, 0.2 to 1.
    pub head_bobbing: f32,
    /// The game's mouse sensitivity setting.
    pub sensitivity: f32,
    /// The game's setting for the mouse's sensitivity with the sights up.
    /// The weapon's own factor multiplies it.
    pub aiming_sensitivity: f32,
    /// The game's setting for how far a turn of the wheel zooms a variable
    /// scope (`OpticSensitivity`, 1 to 5).
    pub optic_sensitivity: f32,
    /// Whether the command line gave either, which then has the say over
    /// what the options tab last kept.
    pub(super) sensitivity_given: [bool; 2],
    /// Degrees a count of the mouse turns the view before the
    /// sensitivity (`keys::degrees_a_count`), and the game's two switches
    /// that turn an axis of the mouse about, across and up and down.
    pub degrees_a_count: f32,
    pub invert: [bool; 2],
    /// Degrees a count of the mouse turns the head's free look
    /// (`keys::look_degrees_a_count`): no sensitivity comes after it.
    pub look_degrees_a_count: f32,
    /// Seed of the recoil's random numbers.
    pub seed: u64,
    /// What the character's skills add to the aiming speed, as a fraction,
    /// for this run over Aim Drills' level (`--aim-speed-bonus`).
    pub aim_speed_bonus: Option<f32>,
    /// One mastering level for every weapon this run (`--mastering`, 0 to
    /// 2, the game's levels; its screens show the level and one), over each
    /// weapon's family's (`skills::Mastery`). From 2 a reload keeps the
    /// sights up.
    pub mastering: Option<u32>,
    /// Weapon families' levels for this run over the options tab's
    /// (`--mastering-family Name=level`, the install's name, 0 to 2).
    pub mastering_families: Vec<(String, u32)>,
    /// Skill levels for this run over the options tab's (`skills`): every
    /// weapon class's (`--weapon-skill`), Strength's, Endurance's, and any
    /// by the game's name (`--skill Name=level`), each 0 to 51.
    pub weapon_skill: Option<u32>,
    pub skill_levels: Vec<(String, u32)>,
    /// The character's experience for this run over the options file's
    /// (`--experience <n>`; `experience`).
    pub experience: Option<i64>,
    /// Levels set in play on a drawn frame (`--skill-at frame:Name=level`),
    /// as a host game would set them: what the options tab does then.
    pub skill_at: Vec<(u32, String, u32)>,
    /// Frames a second the simulation runs at, whatever the rate of drawing.
    pub frame_rate: f32,
    /// Milliseconds the clock moves on each drawn frame, whatever the wall's
    /// clock did (`--fixed-dt`): the engine's real and virtual times both
    /// (`TimeUpdateStrategy::ManualDuration`), so that a run made twice draws
    /// the same frames (a capture set beside another build's). None: the
    /// wall's clock, as played (and as the frame's cost is measured).
    pub fixed_dt: Option<f32>,
    /// The character's Strength and Endurance levels for this run, 0 to 51.
    pub strength: Option<u32>,
    pub endurance: Option<u32>,
    /// Where the view starts, in degrees below level: a capture run
    /// ignores the mouse, so this is how it looks down or up.
    pub(super) pitch: f32,
    /// Which way the character starts facing, in degrees to the right of
    /// the lane: a capture run walks up to the range's walls with it.
    pub(super) yaw: f32,
    /// Whether `--yaw` or `--pitch` was given: a `--start` before one of
    /// the range's things otherwise turns the view to it.
    pub(super) view_given: bool,
    /// The moment `CUSTOMS LIGHT` lights the range at, where `--hour` or
    /// `--cloudiness` says another than the app's (`lighting::RANGE_MOMENT`).
    pub(super) hour: Option<f32>,
    pub(super) cloudiness: Option<f32>,
    /// `--malfunction <kind>`: the weapon first taken in hand malfunctions
    /// so at its next shot (`misfire`, `jam`, `feed`, `soft`, `hard`); and
    /// `--heat <n>`: it is taken in hand that hot (0 to 200), as if just
    /// fired. For a check (`crate::wear`).
    pub malfunction: Option<trp_sim::weapons::malfunction::Malf>,
    pub heat: Option<f32>,
    /// `--repair <durability>`: the first weapon in a slot worn down to that
    /// durability, and its repair window opened with the first repair kit
    /// carried, the inventory open for the repair. For a check
    /// (`repair_view`).
    pub repair: Option<f32>,
    /// `--buffed <spread|protection|reduction>[:rare]`: the first weapon
    /// (or, for `reduction`, armour) in a slot given that repair buff, at
    /// the middle of SPT's ranges, and its inspect window opened, the
    /// inventory open. For a check (`repair_view`).
    pub buffed: Option<(trp_sim::items::repair::BuffKind, trp_sim::items::repair::Rarity)>,
    /// Save one frame here and exit. Used to check the app unattended.
    pub(super) screenshot: Option<PathBuf>,
    /// With `--screenshot`, the region of the frame kept, in the window's
    /// pixels (`--screenshot-crop x,y,w,h`): a reader of a capture pays for
    /// its pixels, and a screen's corner is a fraction of the whole.
    pub(super) screenshot_crop: Option<[u32; 4]>,
    /// Everything the app's sounds mixed, from the start to the run's end,
    /// written here as a WAVE file (`--sound-out`): to hear, and set beside
    /// a recording of the game, what a run played.
    pub(super) sound_out: Option<PathBuf>,
    /// A line for each sound of moving played, written here (`--log-sounds`).
    pub(super) log_sounds: Option<PathBuf>,
    /// The interface written here frame by frame in the recorder's own
    /// lines (`--dump-ui`), from the frame `--dump-ui-from` gives on, as
    /// the screenshot's frames count (`ui_dump`).
    pub(super) dump_ui: Option<PathBuf>,
    pub(super) dump_ui_from: u32,
    /// Frame on which that capture is taken.
    pub(super) screenshot_frame: u32,
    /// `--figure-hold <seconds>`: the OVERALL tab's own figure's clock
    /// stopped there at each showing (`figure_view::FigureHold`), for a
    /// capture beside a picture of the game's at that time.
    pub(super) figure_hold: Option<f32>,
    /// `--figure-pick <pick>` (as often as wanted): what the OVERALL tab's
    /// own figure's idle picks at its first events, in place of its draws
    /// (`figure_view::FigurePicks`), for a capture of a move.
    pub(super) figure_picks: Vec<trp_sim::characters::figure::Pick>,
    /// How many frames in a row are captured from it (`--screenshot-frames`).
    pub(super) screenshot_count: u32,
    /// The window's size as the run begins (`--resolution`; the options'
    /// own resolution has the last word for someone at the keys).
    pub(super) window: [u32; 2],
    /// Write one line per rendered frame here: the clip on screen and
    /// where the weapon and the hands are as the camera sees them.
    pub(super) log_frames: Option<PathBuf>,
    /// A level file to read for the run (`--level`; `level`).
    pub(super) level: Option<PathBuf>,
    /// Write the range as a level file there and end (`--export-level`).
    pub(super) export_level: Option<PathBuf>,
    /// Write one line per rendered frame here: how long the frame took,
    /// what the simulation ran and cost in it, and where the camera is.
    pub(super) log_pacing: Option<PathBuf>,
    /// Degrees a second a capture run turns the view to the right, from a
    /// second in.
    pub turn: f32,
    /// One of the recorder's scripts to play as the input, in place of
    /// the keyboard and mouse (`--scenario <file>`), and how many seconds
    /// into the run it begins (`--scenario-at`).
    pub scenario: Option<trp_sim::recordings::script::Scenario>,
    pub scenario_at: f32,
    /// Whether a view from outside takes in the whole figure.
    pub(super) whole: bool,
    /// How far round the character the watching camera stands from its
    /// usual place, degrees (`--view watch:<degrees>`).
    pub(super) watch_turn: f32,
    /// A capture from outside sees the range from overhead, from this many
    /// metres over where the character starts (`--view map[:<metres>]`).
    pub(super) map: Option<f32>,
    /// Where in the range the character starts, if not in its middle:
    /// `platform`, `shelf` or `stairs`.
    pub(super) start: Option<String>,
    /// A starting character of the install's database whose equipment the
    /// character carries and wears, as `Standard/usec`; nothing if not
    /// given.
    pub(super) loadout: Option<String>,
    /// Whether `--weight` was given, which then has the say over the
    /// equipment's own weight.
    pub(super) weight_given: bool,
    /// Start with the inventory screen open (for a capture).
    pub(super) inventory: bool,
    /// Start with the crate open in that screen (`--loot`).
    pub(super) loot: bool,
    /// Start with the first dummy's body searched in that screen, as soon
    /// as it has gear, dead or not (`--search-body`, a testing aid).
    pub search_body: bool,
    /// Start with what is in the crate put on by the inventory's own
    /// moves: the weapon on the sling, the vest and the backpack worn
    /// (`--equip`). A run nobody is at the keys of does so unless told
    /// `--bare`, so that it has its weapon in hand as before.
    pub(super) equip: bool,
    pub(super) bare: bool,
    /// The dummies' weapon lights on from the start (`--dummy-lights`; a
    /// testing aid, as the debug panel's `LIGHTS`).
    pub(super) dummy_lights: bool,
    /// The magazines the kit put on taken off again, the weapon's own left
    /// in it: what the reload key does with no magazine to find
    /// (`--no-spare-magazines`, a test's).
    pub(super) no_spare_magazines: bool,
    /// A panic on the first frame, asked for to see a crash's report (`--crash`, `run_log`).
    pub(super) crash: bool,
    /// Whether a weapon hangs where the hands take one from as the run
    /// begins (`--equip`, a run nobody is at the keys of, a loadout): the
    /// run begins with it in hand. Otherwise it begins with empty hands and
    /// every weapon's rig is made when it is first wanted (`weapon::
    /// spawn_weapon`).
    pub starts_armed: bool,
    /// Drags of the mouse to make in a run nobody is at the keys of, each
    /// from one place in the window to another, in pixels (`--drag
    /// x:y:x:y`, as often as wanted): how the inventory screen's moving
    /// of things is tried without a hand on the mouse.
    pub(super) drags: Vec<[f32; 4]>,
    /// The drawn frame the first of them begins on (`--drag-at`; 90 unless
    /// told), for a screen that opens later than that.
    pub(super) drag_at: u32,
    /// Moves of the mouse with no button, each on a drawn frame to a place
    /// in the window, in pixels (`--move <frame>:<x>:<y>`): onto a line of
    /// a menu, to open its own lines.
    pub(super) moves: Vec<(u32, f32, f32)>,
    /// Keys to press in such a run: the drawn frame, the key, and for
    /// how many frames it is held (`--tap <frame>:<key>[:<frames>]` with
    /// a letter, a digit, `tab`, `delete`, `space`, `ctrl`, `alt`,
    /// `shift`, `lmb` and so on). With any given, the keys are live in
    /// the run: the inventory's, and, in a run that holds no keys of its
    /// own (`--hold`), the character's, heard through the bindings as a
    /// hand's are.
    pub(super) taps: Vec<(u32, platform::keys::Key, u32)>,
    /// The mouse's travel in such a run: from a drawn frame, for so many
    /// frames, so many counts a frame across and down (`--mouse
    /// <frame>:<frames>:<across>:<down>`). It goes through the bindings
    /// as a hand's does: the turn's, or with the free look's key held
    /// (`--tap <frame>:mmb:<frames>`) the head's.
    pub mouse: Vec<(u32, u32, Vec2)>,
    /// Notches of the wheel to turn in such a run: the drawn frame and
    /// how many, up counted as positive (`--wheel <frame>:<notches>`).
    pub wheel: Vec<(u32, i32)>,
    /// Notches of the mouse's own wheel in such a run, heard by the
    /// bindings as a hand's turn is, a notch a frame from the frame given
    /// (`--scroll <frame>:<notches>`: R held and a notch, the magazine
    /// picker).
    pub scroll: Vec<(u32, i32)>,
    /// Notches of the wheel turned with the scope zoom's keys held in such
    /// a run (`--zoom <frame>:<notches>`, up counted as positive, which
    /// narrows): a variable scope's zoom (`optic_view`).
    pub zoom: Vec<(u32, i32)>,
    /// Damage done in such a run, to look at what health makes of it: on a
    /// drawn frame, to a part, so much, as a bullet's (`--damage
    /// <frame>:<part>:<amount>`, the part `head`, `chest`, `stomach`,
    /// `left-arm`, `right-arm`, `left-leg` or `right-leg`; and after them,
    /// if given, the stamina burnt a point as a round's `StaminaBurnPerDamage`,
    /// which slows the character for a while: `Movement::hit`). A testing
    /// aid, in any run.
    pub damage: Vec<(u32, trp_sim::condition::health::Part, f32, f32)>,
    /// Blows by the install's poisoned knife (the cultists' damask), to
    /// look at a poisoning (`--poison <frame>:<part>:<amount>`, the parts as
    /// `--damage` names them): the damage a knife's, then its poison as the
    /// game puts it on (`health_view::damage_asked`). A testing aid.
    pub poison: Vec<(u32, trp_sim::condition::health::Part, f32)>,
    /// Whether the range's ground and boxes carry grid lines (`--grid off`
    /// leaves them plain, as the earliest captures have them).
    pub(super) grid: bool,
    /// Whether the window is left to answer a key pressed with Alt in the
    /// system's own way (`--alt-menu`), which shows the pointer for a
    /// moment at each such key: for setting beside what the app does.
    pub alt_menu: bool,
    /// Whether the app's panel at the top right (the frame rate, the debug
    /// figures and the keys) starts shown (`--fps on`); `` ` `` switches it
    /// while running.
    pub show_fps: bool,
    /// When the battle screen's three panels show in this run, over the
    /// install's and the options' (`--hud always|autohide|never`).
    pub hud: Option<crate::panels::battle_view::Shows>,
    /// The options' switch `CUSTOM FIRST PERSON HANDS` as the run begins,
    /// where the command line says (`--custom-hands on|off`): otherwise
    /// it is as the options' file has it, and on without one.
    pub custom_hands: Option<bool>,
    /// The time of day the wrist watch begins at, in seconds from midnight
    /// (`--clock HH:MM[:SS]`): for a capture to show a time known before.
    /// Otherwise the computer's local time as the run begins (`watch_view`).
    pub clock: Option<f64>,
    /// The switch `SMOOTH FIRST PERSON ANIMATIONS` as the run begins, the
    /// same way (`--smooth-animations on|off`).
    pub smooth_animations: Option<bool>,
    /// The switch `ICON GENERATION` as the run begins, the same way
    /// (`--icons on|off`).
    pub icon_generation: Option<bool>,
    /// The pictures' quality (0 to 2) and their streaming as the run
    /// begins, over the options' (`--texture-quality <n>`, `--mip-streaming
    /// on|off`).
    pub texture_quality: Option<u8>,
    pub mip_streaming: Option<bool>,
    /// The shadows' quality (0 to 3) as the run begins, over the options'
    /// (`--shadow-quality <n>`); what the run took, once it has begun.
    pub shadow_quality: Option<u8>,
    /// The ambient occlusion's choice as the run begins, over the options'
    /// (`--ssao <choice>`, as `Graphics.ini` names it); what the run took,
    /// once it has begun.
    pub ssao: Option<trp_sim::lighting::hbao::SsaoMode>,
    /// The occlusion shown alone, as the game's `HBAO` can show it
    /// (`--ssao-display ao`): for looking at it.
    pub ssao_ao_only: bool,
    /// The game's frame rate limit for this run (`--fps-limit <30 to
    /// 144|off>`), over the options' (`frame_limit`): a run nobody is at the
    /// keys of holds none unless told.
    pub fps_limit: Option<Option<u32>>,
    /// The samples of the scene's multisampling (`--msaa off|2|4`), over the
    /// options'; what the run took, once it has begun.
    pub msaa: Option<u8>,
    /// The dust's and smoke's resolution (`--dust-resolution
    /// full|half|quarter`, `offscreen_dust`), over the options'.
    pub dust_resolution: Option<effects::offscreen_dust::DustResolution>,
    /// Any switch of the options' features as the run begins, by its name
    /// in the options' file (`--feature stamina=off`, as often as wanted).
    pub feature_switches: Vec<(String, bool)>,
    /// How the window presents its frames: `vsync` (the default), `off`,
    /// or `mailbox`.
    pub(super) present: String,
    /// The graphics backend, `vulkan` or `dx12`, over the options' (`backend`).
    pub(super) backend: Option<String>,
    /// Whether frames are rendered on a thread of their own while the next
    /// is worked out (Bevy's pipelined rendering, as the game's engine
    /// renders with its graphics jobs: `boot.config`), over the options'
    /// `RENDER THREAD` (`--render-thread on|off`). On for a run at the keys
    /// unless the options say otherwise; off for a capture run unless asked
    /// for, so that a capture's frame shows what it always has. On Bevy 0.16
    /// one update in eight ran a quarter of a
    /// frame early or late with it (the mouse read unevenly); on Bevy 0.18
    /// two in 1,110 do, on or off (`--log-pacing` at 165 a second). Without it a frame
    /// shows input one frame sooner; with it the frame costs 38% less.
    pub(super) render_thread: Option<bool>,
}

impl Options {
    /// Whether nobody is at the keys: a run that saves a frame and exits,
    /// or one that plays a script and logs how it was drawn.
    /// Whether the inventory's keys are to be listened to: someone is at
    /// them, or the run was given presses of its own.
    pub(crate) fn keys_live(&self) -> bool {
        !self.unattended() || !self.taps.is_empty() || !self.mouse.is_empty()
    }

    pub fn unattended(&self) -> bool {
        self.screenshot.is_some() || (self.scenario.is_some() && self.log_pacing.is_some())
    }

    /// Whose eyes the character is seen with in a view: the game has a
    /// character seen by anyone but itself in third person, with a pose
    /// and a weapon motion of its own (`docs/spec/third-person.md`).
    pub fn point_of_view(&self, view: View) -> trp_sim::PointOfView {
        self.pov.unwrap_or(match view {
            View::FirstPerson => trp_sim::PointOfView::FirstPerson,
            View::Free | View::Watched => trp_sim::PointOfView::ThirdPerson,
        })
    }
}
