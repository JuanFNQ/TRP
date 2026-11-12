//! The page's pages, colours, measures, fonts and components.

use super::*;

/// The pages, in the order of the column at the left.
pub(super) const PAGES: [&str; 6] = ["GRAPHICS", "SOUNDS", "HUD", "CONTROLS", "FEATURES", "SKILLS"];

pub(super) const GRAPHICS_PAGE: usize = 0;

pub(super) const SOUNDS_PAGE: usize = 1;

pub(super) const HUD_PAGE: usize = 2;

pub(super) const CONTROLS_PAGE: usize = 3;

pub(super) const FEATURES_PAGE: usize = 4;

pub(super) const SKILLS_PAGE: usize = 5;

/// The CONTROLS page's categories: every binding, then the groups the list
/// keeps (the first the axes' and what follows them, then one from each of
/// `GROUP_STARTS`), and the mouse's settings.
pub(super) const CATEGORIES: [&str; 9] = ["ALL", "MOVEMENT", "COMBAT", "WEAPONS", "GEAR", "ACTIONS", "QUICK USE", "INTERFACE", "MOUSE"];

pub(super) const MOUSE_CATEGORY: usize = 8;

/// The bindings that begin a group of their own (the app's grouping, over
/// the game's own order, which is kept): the
/// weapon's handling, the weapons taken in hand, what is worn and switched,
/// the other actions, the quick-use keys, and the screen's own.
pub(super) const GROUP_STARTS: [&str; 6] = ["WeaponMounting", "PrimaryWeaponFirst", "ToggleGoggles", "Nidnod", "Slot4", "MakeScreenshot"];

/// The shares of the list's width the game's four columns have: what the
/// binding does, its two keys, and how it is pressed.
pub(super) const COLUMNS: [f32; 4] = [40.0, 20.0, 20.0, 20.0];

/// The page's colours: the app's own, after the game's screens, with no
/// solid ground, see-through as the rest of the app's screens, in their
/// font and colours: its words a warm white over the blurred
/// world, its grounds black and see-through, a chosen thing light with
/// dark words as the game's chosen tab, a change that waits for a start in
/// the game's amber. Its grounds a shade darker and some of
/// its words coloured: what is on in a soft green, a bar's fill in khaki,
/// an elite in the orange of the game's elite skill icon's border
/// (`skills_view::ELITE_BORDER`) made lighter to be read, and the SKILLS
/// page's classes in the game's colours for them (`skills_view::
/// class_look`).
pub(super) const INK: Color = Color::srgb(0.84, 0.83, 0.78);

pub(super) const INK_DIM: Color = Color::srgb(0.66, 0.67, 0.63);

pub(super) const INK_FAINT: Color = Color::srgba(0.60, 0.61, 0.58, 0.45);

pub(super) const INK_DARK: Color = Color::srgb(0.09, 0.09, 0.08);

pub(super) const CHOSEN: Color = Color::srgb(0.78, 0.77, 0.71);

pub(super) const CHOSEN_LIT: Color = Color::srgb(0.86, 0.85, 0.79);

pub(super) const GROUND: Color = Color::srgba(0.0, 0.0, 0.0, 0.58);

pub(super) const GROUND_LIT: Color = Color::srgba(0.30, 0.31, 0.30, 0.66);

pub(super) const ROW: Color = Color::srgba(0.0, 0.0, 0.0, 0.55);

pub(super) const ROW_ALT: Color = Color::srgba(0.0, 0.0, 0.0, 0.45);

pub(super) const ROW_LIT: Color = Color::srgba(0.26, 0.27, 0.26, 0.60);

pub(super) const HEAD: Color = Color::srgba(0.0, 0.0, 0.0, 0.72);

pub(super) const HEAD_LIT: Color = Color::srgba(0.12, 0.12, 0.12, 0.80);

pub(super) const WELL: Color = Color::srgba(0.0, 0.0, 0.0, 0.66);

pub(super) const EDGE: Color = Color::srgba(1.0, 1.0, 1.0, 0.13);

pub(super) const AMBER: Color = Color::srgb(0.88, 0.70, 0.36);

pub(super) const RUST: Color = Color::srgb(0.82, 0.47, 0.34);

const SHADOW: Color = Color::srgba(0.0, 0.0, 0.0, 0.7);

/// What is on: a box's tick, a section with all of its switches on.
pub(super) const ON: Color = Color::srgb(0.62, 0.78, 0.45);

/// A bar's fill, and TRP's version under its name.
pub(super) const KHAKI: Color = Color::srgb(0.71, 0.67, 0.52);

/// An elite: its figure, its bar and its name lit.
pub(super) const ELITE: Color = Color::srgb(0.95, 0.64, 0.30);

/// How far a notch of the wheel scrolls a page, in pixels.
pub(super) const NOTCH: f32 = 78.0;

/// The bar's handle at the least, in pixels, however long the page.
pub(super) const HANDLE_LEAST: f32 = 24.0;

/// The page's measure and fonts as it is drawn: a unit is one of the game's
/// screen's 1080 lines, in the window's pixels (`inventory_view::
/// SCREEN_HEIGHT`); the game's own Bender where the install's fonts are
/// read (`TARKOV UI`), else the app's.
#[derive(Clone)]
pub(super) struct Look {
    pub(super) s: f32,
    pub(super) font: Handle<Font>,
    pub(super) bold: Handle<Font>,
}

impl Look {
    pub(super) fn new(height: f32, fonts: Option<&crate::screens::health_view::HealthLook>) -> Self {
        let s = (height / crate::screens::inventory_view::SCREEN_HEIGHT).max(0.3);
        let named = |names: &[&str]| fonts.and_then(|look| names.iter().find_map(|name| look.font_named(name)));
        let font = named(&["Bender Normal SDF", "Bender Shadowed SDF"]).unwrap_or_default();
        // The bold as the game's own Bender Normal draws it, its weight
        // table's typeface; else an asset named for it.
        let bold = fonts.and_then(|look| ["Bender Normal SDF", "Bender Shadowed SDF"].iter().find_map(|name| look.bold_named(name))).or_else(|| named(&["Bender Bold SDF"])).unwrap_or_else(|| font.clone());
        Self { s, font, bold }
    }

    pub(super) fn u(&self, units: f32) -> Val {
        Val::Px(units * self.s)
    }

    pub(super) fn words(&self, text: impl Into<String>, size: f32, color: Color, bold: bool) -> impl Bundle {
        // Light words over the world have the game's dark rim under them;
        // dark words on a light ground none.
        let shadow = if color == INK_DARK { Color::NONE } else { SHADOW };
        (Text::new(text), self.font_of(size, bold), TextColor(color), TextShadow { offset: Vec2::splat((1.2 * self.s).max(1.0)), color: shadow })
    }

    /// The font a run of words is drawn in. Bevy finds a face by its
    /// family and the weight asked, and Bender Bold is of Bender Normal's
    /// family: its handle asked at the normal weight draws Bender Normal,
    /// so the bold is asked bold too (`health_view::weight_of`).
    fn font_of(&self, size: f32, bold: bool) -> TextFont {
        let (font, style) = if bold { (self.bold.clone(), 1) } else { (self.font.clone(), 0) };
        TextFont { font, font_size: size * self.s, weight: crate::screens::health_view::weight_of(style), ..default() }
    }

    pub(super) fn text(&self, text: impl Into<String>, size: f32, color: Color) -> impl Bundle {
        self.words(text, size, color, false)
    }

    pub(super) fn strong(&self, text: impl Into<String>, size: f32, color: Color) -> impl Bundle {
        self.words(text, size, color, true)
    }

    /// A run of words after a text's first, in a colour of its own (the
    /// text's rim under it).
    pub(super) fn span(&self, text: impl Into<String>, size: f32, color: Color, bold: bool) -> impl Bundle {
        (TextSpan::new(text), self.font_of(size, bold), TextColor(color))
    }
}

#[derive(Component)]
pub struct OptionsPage;

/// What a click on a thing of the page does.
#[derive(Component, Clone, Copy, PartialEq)]
pub(super) enum Click {
    Page(usize),
    Quit,
    Reset,
    Apply,
    Step(Step, i8),
    Toggle(Toggle),
    Feature(Switch),
    Fold(usize),
    Category(usize),
    Search,
    /// The FEATURES page's CHARACTER NAME field.
    Name,
    ClearKey,
    Key(Cell),
    Press(usize),
    PressChoice(PressType),
    HourDefault,
    SkillsUntrained,
    MasteryUntrained,
    /// The PLAYER VOICE's box (its list opened or shut), a voice of the
    /// list chosen, and the speaker.
    VoiceList,
    VoiceChoice(usize),
    VoicePreview,
}

impl Click {
    /// A button of the game's kind (`ButtonFeedback`: the settings' tabs,
    /// `UISpawnableToggle`, and its buttons, `DefaultUIButton`), which
    /// sounds `ButtonOver` as the mouse comes onto it and `ButtonClick` at
    /// a click while it works. A box, an arrow, a section's head, a key's
    /// cell, a way of pressing and the search field are not: each sounds
    /// as what it stands for does in the game, or not at all (`clicks`).
    pub(super) fn feedback(self) -> bool {
        matches!(self, Click::Page(_) | Click::Quit | Click::Reset | Click::Apply | Click::Category(_) | Click::ClearKey | Click::HourDefault | Click::SkillsUntrained | Click::MasteryUntrained)
    }
}

/// A button that works and sounds as the game's do (`Click::feedback`).
#[derive(Component)]
pub(super) struct Feedback;

/// A setting chosen among several, stepped through by its arrows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Step {
    Quick,
    Stamina,
    Health,
    Colors,
    Mode,
    Resolution,
    Texture,
    Shadow,
    Ssao,
    Clouds,
    Msaa,
    Dust,
    Renderer,
    /// `OBJECT LOD QUALITY` (`trp_data::install::LOD_BIAS_CHOICES`).
    Lod,
}

/// A setting on or off, ticked in its box.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Toggle {
    Vsync,
    /// The game's frame rate limit, ticked where it holds (the game's
    /// `DisableGameFramerateLimit` the other way).
    FpsLimit,
    MipStreaming,
    RenderThread,
    Invert(usize),
    /// One of the HUD page's notices (`Notices::ALL`).
    Notice(usize),
    /// The HUD page's HIDE UNUSED TABS.
    HideTabs,
}


/// A row lit while the mouse is on it: its ground at rest.
#[derive(Component)]
pub(super) struct Lit(pub(super) Color);

/// A button's grounds at rest and under the mouse.
#[derive(Component)]
pub(super) struct Tint(pub(super) Color, pub(super) Color);

/// The foot's words: the page's last note (`OptionsState::note`).
#[derive(Component)]
pub(super) struct Footer;

/// The search field of the CONTROLS page.
#[derive(Component)]
pub(super) struct SearchField;

/// The CHARACTER NAME field of the FEATURES page.
#[derive(Component)]
pub(super) struct NameField;

/// A bar that is slid, the part of it that is filled, and its figure.
#[derive(Component)]
pub(super) struct SlideBar(pub(super) Slide);

#[derive(Component)]
pub(super) struct SlideFill(pub(super) Slide);

#[derive(Component)]
pub(super) struct SlideValue(pub(super) Slide);

/// The page's scrolled area, its bar and the bar's handle.
#[derive(Component)]
pub(super) struct PageArea;

#[derive(Component)]
pub(super) struct PageBar;

#[derive(Component)]
pub(super) struct PageHandle;
