//! The battle screen's panels of the character, as the game's with
//! `TARKOV UI` on (the user, 2026-10-16: "Give the TARKOV UI character
//! speed/stance makeover as well as displaying the health shilloute
//! in-game"): the stance panel in the screen's lower left corner
//! (`BattleStancePanel`: the stance's figure and its slider, the speed's
//! slider and its notches, the legs' and the arms' stamina, the noise) and
//! the health panel in its upper left (`CharacterHealthPanel`: the body's
//! parts coloured by their health, its effects' icons), each fading in as
//! the game's screen raises it and out again (`BattleUIComponentAnimation`),
//! and the quick bar at the top (`InventoryScreenQuickAccessPanel`: each
//! view's thing, the one in the hands lit), by the game's own settings of
//! when they show (`BattleModes`: the install's `Game.ini`, or the options'
//! GAME page). Drawn from the scene as the inventory's panels are
//! (`health_view::draw_nodes`); with `TARKOV UI` off the app's plain meters
//! stay (`hud`). `docs/spec/inventory.md`, "The battle screen's panels".

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use std::collections::HashMap;

use crate::controller::Controller;
use crate::health_view::{draw_nodes, effect_icon, FullWidth, HealthLook, Marks, Frame, Piece, PieceFill, Words};
use crate::inventory_view::{Carried, HandsHeld};
use trp_data::screen::SliderParts;
use trp_sim::health::Part;

pub struct BattleViewPlugin;

impl Plugin for BattleViewPlugin {
    fn build(&self, app: &mut App) {
        // The quick bar is drawn after the panels' fades have stepped, so
        // that a bar raised on a frame is there on it (it is not kept while
        // faded out: `draw_quick_bar`), and its colours kept before the
        // fade draws it.
        app.init_resource::<BattleState>().add_systems(
            Update,
            (draw_battle, keep_own, follow_stance, follow_health, draw_quick_bar, keep_own, fade_battle).chain().after(crate::controller::step),
        );
    }
}

/// Where the two panels' objects are numbered among the drawn pieces (past
/// the inventory's, `health_view`).
const STANCE: usize = 11 << 20;
const BODY: usize = 12 << 20;
const QUICK_BAR: usize = 13 << 20;
const SPAN: usize = 1 << 20;

/// When one of the panels shows (`EVisibilityMode`): always, a while after
/// what raises it (the game's default), or never.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Shows {
    Always,
    #[default]
    Autohide,
    Never,
}

impl Shows {
    /// In the order of the game's own list of them (autohide, always
    /// shown, always hidden).
    pub const ALL: [Shows; 3] = [Shows::Autohide, Shows::Always, Shows::Never];

    /// Its name as the game keeps it (`Game.ini`), and the key of its words
    /// in the install's texts.
    pub fn name(self) -> &'static str {
        match self {
            Shows::Always => "Always",
            Shows::Autohide => "Autohide",
            Shows::Never => "Never",
        }
    }

    pub fn of(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|shows| shows.name() == name)
    }
}

/// The settings the battle screen's panels go by (the game's own: its
/// settings' GAME tab, `GameSettingsGroup`): when the quick bar, the stance
/// panel and the health panel show, and how the parts are coloured. As
/// the run begins, the install's (`Game.ini`; where it says nothing, the
/// game's defaults, autohide, autohide, autohide and white to red), or
/// what the options' GAME page has kept over them (`options_view`); a
/// change there is taken as the battle screen is next shown (the
/// inventory shut), as the game's panels read them as they are shown.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct BattleModes {
    pub quick: Shows,
    pub stamina: Shows,
    pub health: Shows,
    /// The parts coloured green to red (`EHealthColorScheme.GreenToRed`,
    /// "Polychrome") rather than white to red ("Monochrome").
    pub green_to_red: bool,
}

impl BattleModes {
    pub fn read(install: &trp_data::Install) -> Self {
        let shows = |name: &str| install.game_setting(name).as_deref().and_then(Shows::of).unwrap_or_default();
        Self { quick: shows("QuickSlotsVisibility"), stamina: shows("StaminaVisibility"), health: shows("HealthVisibility"), green_to_red: install.game_setting("HealthColor").as_deref() == Some("GreenToRed") }
    }

    /// The scheme's name as the game keeps it.
    pub fn color_name(&self) -> &'static str {
        if self.green_to_red {
            "GreenToRed"
        } else {
            "WhiteToRed"
        }
    }
}

/// One of the panels as drawn, made afresh as the window's size
/// changes: which (0 the stance's, 1 the health's, 2 the quick bar) and
/// for what size.
#[derive(Component)]
struct BattleRoot {
    panel: usize,
    size: [u32; 2],
}

/// One of the twenty notches of the speed the stance panel's script makes
/// (`BattleStanceNotch`, at a twentieth of the top speed each).
#[derive(Component)]
struct SpeedNotch(usize);

/// An effect's icon on the health panel (`StackedEffectIcon`).
#[derive(Component)]
struct BattleEffect;

/// What the quick bar was drawn for: the window, the thing in the hands,
/// and each view's key, words and thing; drawn afresh when any changes.
#[derive(Component, PartialEq)]
struct QuickDrawn(Vec<String>);

/// The colours a drawn object has of its own, before its panel's fade
/// (`CanvasGroup.alpha`, which every picture and text under the panel is
/// drawn at besides): its picture's, its ground's and its words'.
#[derive(Component, Clone, Copy)]
struct Own {
    image: Option<Color>,
    ground: Option<Color>,
    text: Option<Color>,
}

/// A panel's fade (`BattleUIComponentAnimation`): its alpha, and what it is
/// doing, one step after another: waiting so long, or fading to an alpha
/// in half a second from where it stands (`DOFade`, DOTween's default
/// ease, out quad).
#[derive(Default)]
struct Fader {
    alpha: f32,
    steps: Vec<Step>,
    at: f32,
    from: f32,
}

#[derive(Clone, Copy)]
enum Step {
    Wait(f32),
    Fade(f32),
}

/// `BattleUIComponentAnimation`'s half a second's fade, and its three
/// seconds' stay before a panel raised a while fades out.
const FADE_TIME: f32 = 0.5;
const VISIBILITY_DURATION: f32 = 3.0;

impl Fader {
    /// `Show`: to whole, then, raised a while, three seconds and out.
    fn show(&mut self, autohide: bool) {
        self.steps = vec![Step::Fade(1.0)];
        if autohide {
            self.steps.extend([Step::Wait(VISIBILITY_DURATION), Step::Fade(0.0)]);
        }
        self.begin();
    }

    /// `Hide`: after the delay, out.
    fn hide(&mut self, delay: f32) {
        self.steps = if delay > 0.0 { vec![Step::Wait(delay), Step::Fade(0.0)] } else { vec![Step::Fade(0.0)] };
        self.begin();
    }

    /// `Close`: gone at once.
    fn close(&mut self) {
        self.steps.clear();
        self.alpha = 0.0;
    }

    fn begin(&mut self) {
        self.at = 0.0;
        self.from = self.alpha;
    }

    fn advance(&mut self, mut dt: f32) {
        while let Some(&step) = self.steps.first() {
            let length = match step {
                Step::Wait(time) => time,
                Step::Fade(_) => FADE_TIME,
            };
            let left = length - self.at;
            if dt < left {
                self.at += dt;
                if let Step::Fade(to) = step {
                    let x = self.at / FADE_TIME;
                    self.alpha = self.from + (to - self.from) * (1.0 - (1.0 - x) * (1.0 - x));
                }
                return;
            }
            dt -= left;
            if let Step::Fade(to) = step {
                self.alpha = to;
            }
            self.steps.remove(0);
            self.begin();
        }
    }
}

/// What the panels' scripts follow of the character, as last seen: the
/// stance's target and whether lying, the top speed and the limit on it,
/// the speed the panel shows, each stamina pool's changes, the sprint, and
/// health's raising of its panel.
#[derive(Clone, Copy, PartialEq)]
struct Seen {
    pose_level: f32,
    prone: bool,
    max_speed: f32,
    limit: f32,
    speed: f32,
    legs: u32,
    arms: u32,
    sprinting: bool,
    pulls: [u32; 2],
}

/// The stance's figures stepped through one at a time (`LadderSetActive`):
/// the one the last ladder reached (`_currentStanceIndex`), the one shown,
/// the one it goes to, and the frames until the next step (two a step).
#[derive(Default)]
struct Ladder {
    reached: usize,
    shown: usize,
    to: usize,
    wait: u32,
}

impl Ladder {
    fn step(&mut self) {
        if self.shown != self.to {
            self.shown = if self.to > self.shown { self.shown + 1 } else { self.shown - 1 };
            self.wait = 2;
        } else {
            self.reached = self.to;
            self.wait = 0;
        }
    }
}

/// The panels' state as their scripts keep it.
#[derive(Resource, Default)]
struct BattleState {
    /// Whether the battle screen is up (the inventory shut and the
    /// character alive); each time it comes up its panels are shown afresh
    /// (`BattleUIScreen.Show`), by the settings as they then stand.
    up: bool,
    modes: BattleModes,
    stance: Fader,
    health: Fader,
    quick: Fader,
    /// The changes of the hands begun, as last seen (`HandsHeld`).
    hands_seen: u32,
    seen: Option<Seen>,
    ladder: Ladder,
    /// The stance the figure was last set for (`_currentStanceImageType`)
    /// and the slider's value; the speed slider's value and most; the
    /// noise's level; each notch's being had.
    stance_type: f32,
    stance_value: f32,
    speed: [f32; 2],
    noise: usize,
    notches: [bool; 21],
    /// The sprint holds the stance panel up (`_sprintDisplayBattleStance`);
    /// the health panel was last raised a while (`AutoHide`).
    sprint_holds: bool,
    health_autohide: bool,
    /// The effects' icons shown: each one's effect, by the game's name, and
    /// how many of it.
    effects: Vec<(&'static str, usize)>,
}

impl BattleState {
    /// `BattleStancePanel.ChangeStanceImage`: the figure nearest the stance
    /// (lying, -0.35), stepped to from the last one reached; the slider at
    /// its stance.
    fn stance_image(&mut self, figures: &[(usize, f32)], kind: f32) {
        if (self.stance_type - kind).abs() < f32::EPSILON || figures.is_empty() {
            return;
        }
        self.stance_type = kind;
        let mut closest = 0;
        for (index, (_, factor)) in figures.iter().enumerate() {
            if (kind - factor).abs() < (kind - figures[closest].1).abs() {
                closest = index;
            }
        }
        self.ladder.to = closest;
        self.ladder.shown = self.ladder.reached;
        self.ladder.step();
        self.stance_value = figures[closest].1;
    }

    /// `ChangeMaxSpeed`: each notch had where it is no higher than the
    /// limit's share of the top speed (rounded to two places, as the
    /// game's double rounds, a half to even); the speed set to the clamped
    /// one.
    fn max_speed(&mut self, limit: f32, clamped: f32, max: f32) {
        for (index, had) in self.notches.iter_mut().enumerate() {
            let at = if index == 20 { 1.0 } else { 0.05 * index as f32 } as f64;
            let over = ((at - (limit / max) as f64) * 100.0).round_ties_even() / 100.0;
            *had = over <= f32::EPSILON as f64;
        }
        self.speed = [clamped, max];
    }

    /// `BattleUIScreen.PullUpBattleStancePanel`.
    fn pull_up(&mut self, modes: BattleModes) {
        if !self.sprint_holds && modes.stamina != Shows::Never {
            self.stance.show(modes.stamina == Shows::Autohide);
        }
    }

    /// `BattleUIScreen.DecideHealthVisibility` (no effect of the app's is
    /// critical: only lethal intoxication is).
    fn decide_health(&mut self, initial: bool, modes: BattleModes) {
        match modes.health {
            Shows::Never => self.health.hide(0.0),
            Shows::Always => {
                self.health_autohide = false;
                self.health.show(false);
            }
            Shows::Autohide if initial => {
                self.health_autohide = true;
                self.health.show(true);
            }
            Shows::Autohide => self.health.hide(0.0),
        }
    }
}

/// `MovementContext.CovertNoiseLevel`: 1 lying, 2 sprinting; else 0 where
/// the covert movement is nearly whole (`CovertEfficiency`, over 0.9: none
/// with a leg damaged, else the speed's share of the top under 0.4 to 0.2
/// times the Covert Movement skill's speed, less the walk's overweight; the
/// skill at level 0, its speed 1, skills not being a system of the app's),
/// 2 up off the ground faster than 0.8 of the top, and 1 otherwise.
fn noise_level(controller: &Controller) -> usize {
    let m = &controller.movement;
    if m.prone {
        return 1;
    }
    if m.sprinting {
        return 2;
    }
    let ratio = m.clamped_speed() / m.settings.max_speed.max(1e-6);
    let covert = 0.4 * 1.0;
    let efficiency = if m.conditions.leg_damaged() { 0.0 } else { ((ratio - covert) / -0.2).clamp(0.0, 1.0) * (1.0 - m.stamina.settings.walk_overweight) };
    if efficiency > 0.9 {
        0
    } else if m.pose_level > 0.01 && ratio > 0.8 {
        2
    } else {
        1
    }
}

/// The game's colour of a part on the battle screen
/// (`EHealthColorScheme.GetBodyPartColor`, then `Saturation(0.5)`): its
/// health's share squared; white to red, red brought up to white by it
/// (`Color.Gamma`); green to red, its hue at 120 degrees times it; black
/// destroyed; every channel then halved and the alpha whole.
fn part_color(share: f32, destroyed: bool, green_to_red: bool) -> Color {
    if destroyed {
        return Color::BLACK;
    }
    let h = share.clamp(0.0, 1.0).powi(2);
    let [r, g, b] = if green_to_red {
        let [r, g, b] = crate::health_view::hsv(h * 120.0, 1.0, 1.0);
        [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0]
    } else {
        [1.0, h, h]
    };
    Color::srgb(r * 0.5, g * 0.5, b * 0.5)
}

/// Draws both panels (`TARKOV UI`) as the scene has them, each whole in a
/// window's size, hidden until faded in; again as the window's size
/// changes.
fn draw_battle(mut commands: Commands, look: Res<HealthLook>, said: Res<Words>, window: Query<&Window, With<PrimaryWindow>>, roots: Query<(Entity, &BattleRoot)>, mut state: ResMut<BattleState>) {
    let Some(views) = look.battle.as_ref().filter(|_| look.tarkov_ui) else { return };
    let Ok(window) = window.single() else { return };
    let size = [window.width().round() as u32, window.height().round() as u32];
    if size[1] == 0 || look.images.is_empty() || roots.iter().any(|(_, root)| root.panel < 2 && root.size == size) {
        return;
    }
    for (entity, _) in roots.iter().filter(|(_, root)| root.panel < 2) {
        commands.entity(entity).despawn();
    }
    state.effects.clear();
    let s = size[1] as f32 / 1080.0;
    let screen = [size[0] as f32 / s, 1080.0];
    let whole = Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() };
    commands.spawn((BattleRoot { panel: 0, size }, whole.clone(), Visibility::Hidden)).with_children(|root| {
        draw_nodes(root, &look, &views.stance, &look.stance_drawn, STANCE, Marks::default(), &said, Frame::In(screen), [0.0, 0.0], s);
        // The twenty notches the script makes in the speed's row
        // (`BattleStanceNotch.Init`): the row a horizontal group that
        // shares itself among them, each notch's picture where the
        // template keeps it in its share.
        let (Some(row), Some(template)) = (views.notches_place, views.notch_template) else { return };
        let Some(image) = views.stance.find(&format!("{}/Image", views.stance.nodes[template].path)) else { return };
        let [x, y, w, h] = views.stance.placed_in(row, screen).rect;
        let cell = trp_data::screen::Rect { at: [0.0, 0.0], size: [w / 20.0, h] };
        let at = trp_data::screen::place(views.stance.nodes[image].layout, cell);
        let color = views.stance.nodes[image].image.map_or(Color::WHITE, |image| crate::health_view::rgba(image.color));
        for index in 0..20 {
            let node = Node {
                position_type: PositionType::Absolute,
                left: Val::Px((x + index as f32 * w / 20.0 + at.at[0]) * s),
                top: Val::Px((y + h - at.at[1] - at.size[1]) * s),
                width: Val::Px(at.size[0] * s),
                height: Val::Px(at.size[1] * s),
                ..default()
            };
            root.spawn((SpeedNotch(index), node, BackgroundColor(color), Own { image: None, ground: Some(color), text: None }));
        }
    });
    commands.spawn((BattleRoot { panel: 1, size }, whole, Visibility::Hidden)).with_children(|root| {
        draw_nodes(root, &look, &views.health, &look.body_drawn, BODY, Marks::default(), &said, Frame::In(screen), [0.0, 0.0], s);
    });
}

/// Draws the battle screen's quick bar (`TARKOV UI`) where the game's
/// screen has it (`QuickViews::quick_in`: its middle 57.5 right of the
/// screen's, 10 down, as wide as its views ask), as the inventory's
/// quick-use panel is drawn (`health_view::draw_quick_views`): every view
/// with its key's words, its thing's ground and picture turned and fitted
/// (`QuickSlotItemView`), its caption the thing's short name; the view
/// whose thing is in the hands lit (`RefreshSelection`: its ground white,
/// its caption black), the rest with their ground black at 0.6. Drawn
/// afresh as any of that changes.
///
/// Faded out with nothing to come, the bar is not kept: Bevy lays out
/// every node there is, shown or not, each frame (`findings.md`). It is
/// drawn afresh when it is raised.
#[allow(clippy::too_many_arguments)]
fn draw_quick_bar(
    mut commands: Commands,
    look: Res<HealthLook>,
    said: Res<Words>,
    carried: Res<Carried>,
    bindings: Res<crate::keys::Bindings>,
    held: Res<HandsHeld>,
    window: Query<&Window, With<PrimaryWindow>>,
    roots: Query<(Entity, &BattleRoot, Option<&QuickDrawn>)>,
    state: Res<BattleState>,
) {
    let Some(quick) = look.battle_quick.as_ref().filter(|_| look.tarkov_ui) else { return };
    if state.quick.alpha <= 0.0 && state.quick.steps.is_empty() {
        for (entity, ..) in roots.iter().filter(|(_, root, _)| root.panel == 2) {
            commands.entity(entity).despawn();
        }
        return;
    }
    let Ok(window) = window.single() else { return };
    let size = [window.width().round() as u32, window.height().round() as u32];
    if size[1] == 0 || look.images.is_empty() {
        return;
    }
    let mut drawn = vec![format!("{size:?} {:?}", held.item)];
    let mut words: HashMap<String, (String, String)> = HashMap::new();
    let mut selected = Vec::new();
    for slot in &quick.slots {
        let (binding, item) = crate::inventory_view::quick_thing(&carried, &slot.key);
        let own = slot.key.trim_start_matches("Item").replace("10", "0");
        let key = crate::inventory_view::quick_key_words(&bindings, binding, &own);
        let caption = item.map_or(String::new(), |item| carried.name(item));
        if item.is_some_and(|item| Some(item.id) == held.item) {
            selected.push(slot.key.clone());
        }
        let pictured = item.is_some_and(|item| carried.pictures.contains_key(&item.kind.template));
        drawn.push(format!("{} {key} {caption} {:?} {pictured}", slot.key, item.map(|item| item.id)));
        words.insert(slot.key.clone(), (key, caption));
    }
    if roots.iter().any(|(_, root, was)| root.panel == 2 && was.is_some_and(|was| was.0 == drawn)) {
        return;
    }
    for (entity, ..) in roots.iter().filter(|(_, root, _)| root.panel == 2) {
        commands.entity(entity).despawn();
    }
    let s = size[1] as f32 / 1080.0;
    let screen = [size[0] as f32 / s, 1080.0];
    let [x, y, width, height] = quick.quick_in(screen);
    let bar = Node { position_type: PositionType::Absolute, left: Val::Px(x * s), top: Val::Px(y * s), width: Val::Px(width * s), height: Val::Px(height * s), ..default() };
    commands.spawn((BattleRoot { panel: 2, size }, QuickDrawn(drawn), bar, Visibility::Hidden)).with_children(|bar| {
        let views = [width, height];
        crate::health_view::draw_quick_views(bar, &look, quick, &look.battle_quick_drawn, QUICK_BAR, &said, views, &words, Some(&selected), 0, s);
        for slot in &quick.slots {
            let (Some(install), (_, Some(item))) = (slot.install, crate::inventory_view::quick_thing(&carried, &slot.key)) else { continue };
            let [ix, iy, iw, ih] = quick.panel.rect(install, views);
            let place = Node { position_type: PositionType::Absolute, left: Val::Px(ix * s), top: Val::Px(iy * s), width: Val::Px(iw * s), height: Val::Px(ih * s), overflow: Overflow::clip(), ..default() };
            let ground = crate::health_view::item_ground(&item.kind.color);
            bar.spawn((place, BackgroundColor(ground))).with_children(|place| crate::inventory_view::quick_picture(place, &carried, item, [iw, ih], s));
        }
        crate::health_view::draw_quick_views(bar, &look, quick, &look.battle_quick_drawn, QUICK_BAR, &said, views, &words, Some(&selected), 1, s);
    });
}

/// Keeps each newly drawn object's own colours (`Own`), which its panel's
/// fade draws it at a share of.
fn keep_own(mut commands: Commands, roots: Query<Entity, With<BattleRoot>>, children: Query<&Children>, fresh: Query<(Option<&ImageNode>, Option<&BackgroundColor>, Option<&TextColor>), Without<Own>>) {
    for root in &roots {
        for entity in children.iter_descendants(root) {
            if let Ok((image, ground, text)) = fresh.get(entity) {
                commands.entity(entity).insert(Own { image: image.map(|image| image.color), ground: ground.map(|ground| ground.0), text: text.map(|text| text.0) });
            }
        }
    }
}

/// Follows the character as the stance panel's script and the battle
/// screen do: what raises the panel, the figure, the sliders, the notches,
/// the fills and the noise.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn follow_stance(
    controller: Res<Controller>,
    look: Res<HealthLook>,
    time: Res<Time>,
    mut state: ResMut<BattleState>,
    (settings, held): (Res<BattleModes>, Res<HandsHeld>),
    window: Query<&Window, With<PrimaryWindow>>,
    mut pieces: Query<(&Piece, &mut Visibility, &mut UiTransform, &mut Node, Option<&FullWidth>, Option<&mut ImageNode>, Option<&mut Own>)>,
    mut fills: Query<(&PieceFill, &mut ImageNode), Without<Piece>>,
    mut notches: Query<(&SpeedNotch, &mut Own), Without<Piece>>,
) {
    let Some(views) = look.battle.as_ref().filter(|_| look.tarkov_ui) else { return };
    let m = &controller.movement;
    let alive = controller.health.as_ref().is_none_or(|health| health.alive);
    let up = !controller.inventory_open && alive;
    let state = &mut *state;
    // The screen shut (the inventory open, the character dead): every
    // panel gone at once (`BattleUIScreen.Close`).
    if !up {
        if state.up {
            state.up = false;
            state.stance.close();
            state.health.close();
            state.quick.close();
        }
        return;
    }
    let pulls = controller.health_pulls;
    let now = Seen {
        pose_level: m.pose_level,
        prone: m.prone,
        max_speed: m.settings.max_speed,
        limit: m.speed_limit(),
        speed: if m.sprinting { m.character_speed } else { m.clamped_speed() },
        legs: m.stamina.legs.changes(),
        arms: m.stamina.arms.changes(),
        sprinting: m.sprinting,
        pulls,
    };
    if !state.up {
        // The screen shown, by the settings as they stand: each panel shut,
        // then as the settings have it with nothing raising it
        // (`UpdatePanelsVisibility(false)`): always shown, it fades in.
        state.up = true;
        state.modes = *settings;
        let modes = state.modes;
        state.stance.close();
        state.health.close();
        state.quick.close();
        state.sprint_holds = false;
        state.seen = None;
        state.hands_seen = held.changes;
        match modes.stamina {
            Shows::Always => state.stance.show(false),
            _ => state.stance.hide(0.0),
        }
        // (No health panel at all with `HEALTH SYSTEM` off.)
        if controller.health.is_some() {
            state.decide_health(false, modes);
        }
        match modes.quick {
            Shows::Always => state.quick.show(false),
            _ => state.quick.hide(0.0),
        }
    }
    let modes = state.modes;
    // The hands begin to change (`HandsChangingEvent`): the quick bar
    // raised (`method_14`).
    if held.changes != state.hands_seen {
        state.hands_seen = held.changes;
        if modes.quick != Shows::Never {
            state.quick.show(modes.quick == Shows::Autohide);
        }
    }
    let figures = &views.figures;
    match state.seen {
        // The panel shown (`BattleStancePanel.Show`): set as the character
        // stands, nothing raised.
        None => {
            state.stance_image(figures, if m.prone { -0.35 } else { m.pose_level });
            state.max_speed(now.limit, m.clamped_speed(), now.max_speed);
            state.noise = noise_level(&controller);
        }
        Some(before) => {
            let mut pull = false;
            if now.legs != before.legs || now.arms != before.arms {
                pull = true;
            }
            if now.prone != before.prone {
                state.stance_image(figures, if m.prone { -0.35 } else { m.smoothed_pose_level });
                state.noise = noise_level(&controller);
                pull = true;
            } else if (now.pose_level - before.pose_level).abs() > 0.0 {
                state.stance_image(figures, if m.prone { -0.35 } else { m.pose_level });
                state.noise = noise_level(&controller);
                pull = true;
            }
            if now.max_speed != before.max_speed || now.limit != before.limit {
                state.max_speed(now.limit, m.clamped_speed(), now.max_speed);
                state.noise = noise_level(&controller);
                pull = true;
            }
            if now.speed != before.speed {
                state.speed = [now.speed, now.max_speed];
                state.noise = noise_level(&controller);
                pull = true;
            }
            if now.sprinting != before.sprinting {
                // `PullUpBattleStancePanelBySprint`: held up while the
                // sprint lasts, out three seconds after it.
                state.noise = noise_level(&controller);
                state.sprint_holds = now.sprinting;
                if modes.stamina != Shows::Never {
                    if now.sprinting {
                        state.stance.show(false);
                    } else if modes.stamina == Shows::Autohide {
                        state.stance.hide(VISIBILITY_DURATION);
                    }
                }
            }
            if pull {
                state.pull_up(modes);
            }
            if controller.health.is_some() {
                if now.pulls[0] != before.pulls[0] {
                    state.decide_health(true, modes);
                } else if now.pulls[1] != before.pulls[1] && !state.health_autohide {
                    state.decide_health(true, modes);
                }
            }
        }
    }
    state.seen = Some(now);
    if state.ladder.wait > 0 {
        state.ladder.wait -= 1;
        if state.ladder.wait == 0 {
            state.ladder.step();
        }
    }
    let dt = time.delta_secs();
    state.stance.advance(dt);
    state.health.advance(dt);
    state.quick.advance(dt);

    // The pieces as the script leaves them.
    let Ok(window) = window.single() else { return };
    let s = window.height() / 1080.0;
    let stance = &views.stance;
    let own_size = stance.nodes[0].layout.size_delta;
    let under = |index: usize, top: usize| {
        let mut above = Some(index);
        while let Some(at) = above {
            if at == top {
                return true;
            }
            above = stance.nodes[at].parent;
        }
        false
    };
    // How far each slider's handle is moved from where the scene has it.
    let moved = |slider: Option<SliderParts>, share: f32| -> Option<(usize, [f32; 2])> {
        let slider = slider?;
        let handle = slider.handle?;
        let [x0, y0, ..] = stance.rect(handle, own_size);
        let [x1, y1, ..] = slider.handle_at(stance, own_size, share)?;
        Some((handle, [x1 - x0, y1 - y0]))
    };
    let stance_share = views.stance_slider.map_or(0.0, |slider| SliderParts::share(state.stance_value, slider.min, slider.max));
    let speed_share = SliderParts::share(state.speed[0].min(state.speed[1]), 0.0, state.speed[1]);
    let handles: Vec<(usize, [f32; 2])> = [moved(views.stance_slider, stance_share), moved(views.speed_slider, speed_share)].into_iter().flatten().collect();
    let final_image = views.final_notch.and_then(|notch| stance.find(&format!("{}/Image", stance.nodes[notch].path)));
    let notch_color = |had: bool| crate::health_view::rgba(views.notch_colors[if had { 0 } else { 1 }]);
    let picture = |key: Option<(i32, i64)>| key.and_then(|key| look.battle_sprites.get(&key)).and_then(|picture| Some((look.images.get(picture.texture)?.clone()?, picture.cut)));
    let level = [m.stamina.legs.level(), m.stamina.arms.level()];
    let spent = [m.stamina.legs.exhausted(), m.stamina.arms.exhausted()];
    for (piece, mut visibility, mut transform, mut node, full, image, own) in &mut pieces {
        let Some(index) = piece.0.checked_sub(STANCE).filter(|&index| index < SPAN) else { continue };
        if let Some(n) = figures.iter().position(|(object, _)| *object == index) {
            let want = if n == state.ladder.shown { Visibility::Inherited } else { Visibility::Hidden };
            if *visibility != want {
                *visibility = want;
            }
        }
        if let Some((_, [dx, dy])) = handles.iter().find(|(handle, _)| under(index, *handle)) {
            let want = Val2::px(dx * s, dy * s);
            if transform.translation != want {
                transform.translation = want;
            }
        }
        for (fill, n) in [(views.legs_fill, 0), (views.arms_fill, 1)] {
            if fill == Some(index) {
                if let Some(full) = full {
                    let want = Val::Px(full.0 * level[n].clamp(0.0, 1.0));
                    if node.width != want {
                        node.width = want;
                    }
                }
            }
        }
        if Some(index) == final_image {
            if let Some(mut own) = own {
                own.ground = Some(notch_color(state.notches[20]));
            }
        } else if Some(index) == views.noise {
            if let (Some(mut image), Some((handle, cut))) = (image, picture(views.noise_sprites.get(state.noise).copied())) {
                if image.image != handle || image.rect != Some(cut) {
                    image.image = handle;
                    image.rect = Some(cut);
                }
            }
        }
    }
    // The fills' sprites: each its own, or the spent one under 15.
    for (fill, mut image) in &mut fills {
        let Some(index) = fill.0.checked_sub(STANCE).filter(|&index| index < SPAN) else { continue };
        for (object, n) in [(views.legs_fill, 0), (views.arms_fill, 1)] {
            if object == Some(index) {
                let key = if spent[n] { views.fill_sprites[2] } else { views.fill_sprites[n] };
                if let Some((handle, cut)) = picture(key) {
                    if image.image != handle || image.rect != Some(cut) {
                        image.image = handle;
                        image.rect = Some(cut);
                    }
                }
            }
        }
    }
    for (notch, mut own) in &mut notches {
        own.ground = Some(notch_color(state.notches[notch.0]));
    }
}

/// Follows health as the health panel's script does: each part's colour,
/// the ground's alpha by the colours' scheme, and the effects' icons (the
/// effects at work, those the game has an icon for, one icon of a kind
/// with how many there are where more than one and they are not of a kind
/// that stacks, pain hidden by a painkiller: `EffectDescriptionFilter`).
#[allow(clippy::type_complexity)]
fn follow_health(
    mut commands: Commands,
    controller: Res<Controller>,
    look: Res<HealthLook>,
    mut state: ResMut<BattleState>,
    window: Query<&Window, With<PrimaryWindow>>,
    roots: Query<(Entity, &BattleRoot)>,
    shown: Query<Entity, With<BattleEffect>>,
    mut pieces: Query<(&Piece, &mut Own)>,
) {
    let Some(views) = look.battle.as_ref().filter(|_| look.tarkov_ui) else { return };
    let Some(health) = controller.health.as_ref() else { return };
    let modes = state.modes;
    const NAMES: [&str; 7] = ["Head", "Chest", "Stomach", "LeftArm", "RightArm", "LeftLeg", "RightLeg"];
    for (piece, mut own) in &mut pieces {
        let Some(index) = piece.0.checked_sub(BODY).filter(|&index| index < SPAN) else { continue };
        if Some(index) == views.background {
            own.image = Some(Color::WHITE.with_alpha(if modes.green_to_red { 0.2 } else { 1.0 }));
        } else if let Some((name, _)) = views.parts.iter().find(|(_, image)| *image == index) {
            let Some(at) = NAMES.iter().position(|n| n == name) else { continue };
            let part = health.part(Part::ALL[at]);
            own.image = Some(part_color(part.current / part.maximum.max(1e-6), part.destroyed, modes.green_to_red));
        }
    }
    // The effects' icons.
    use trp_sim::health::Kind;
    let at_work: Vec<Kind> = health.effects.iter().filter(|effect| effect.active()).map(|effect| effect.kind).collect();
    let painkiller = at_work.contains(&Kind::PainKiller);
    let mut wanted: Vec<(&'static str, usize)> = Vec::new();
    for kind in at_work {
        if kind == Kind::Pain && painkiller {
            continue;
        }
        let Some(name) = effect_icon(kind) else { continue };
        match wanted.iter_mut().find(|(had, _)| *had == name) {
            Some((_, count)) => *count += 1,
            None => wanted.push((name, 1)),
        }
    }
    if wanted == state.effects {
        return;
    }
    let Some((root, _)) = roots.iter().find(|(_, root)| root.panel == 1) else { return };
    state.effects = wanted.clone();
    for entity in &shown {
        commands.entity(entity).despawn();
    }
    let Ok(window) = window.single() else { return };
    let s = window.height() / 1080.0;
    let screen = [window.width() / s, 1080.0];
    let (Some(effects), Some(grid)) = (views.effects, views.effects_grid) else { return };
    let [ex, ey, _, eh] = views.health.placed_in(effects, screen).rect;
    let rows = (((eh - grid.padding[2] - grid.padding[3] + grid.spacing[1] + 0.001) / (grid.cell[1] + grid.spacing[1])).floor() as usize).max(1);
    let template = views.effect_template.map(|icon| views.health.placed_in(icon, screen).rect);
    let count_text = views.stack_count.and_then(|text| Some((views.health.placed_in(text, screen).rect, views.health.nodes[text].text.clone()?)));
    commands.entity(root).with_children(|root| {
        for (n, (name, count)) in wanted.into_iter().enumerate() {
            let Some(picture) = look.effect_pictures.get(name) else { continue };
            let Some(image) = look.images.get(picture.texture).cloned().flatten() else { continue };
            // Down a column first (the grid's start axis, vertical), then
            // the next column.
            let (column, row) = if grid.start_axis == 1 { (n / rows, n % rows) } else { (n % 4, n / 4) };
            let x = ex + grid.padding[0] + column as f32 * (grid.cell[0] + grid.spacing[0]);
            let y = ey + grid.padding[2] + row as f32 * (grid.cell[1] + grid.spacing[1]);
            let node = Node { position_type: PositionType::Absolute, left: Val::Px(x * s), top: Val::Px(y * s), width: Val::Px(grid.cell[0] * s), height: Val::Px(grid.cell[1] * s), ..default() };
            root.spawn((BattleEffect, node, ImageNode { image, rect: Some(picture.cut), flip_y: true, ..default() }, Own { image: Some(Color::WHITE), ground: None, text: None }));
            // Its count where more than one, but of a kind that stacks
            // (`IStackable`: tremor, of the app's).
            if count > 1 && name != "Tremor" {
                let (Some([tx, ty, ..]), Some(([cx, cy, cw, ch], text))) = (template, count_text.as_ref()) else { continue };
                let font = TextFont { font: look.styled_font(&views.health, text.font, text.style), font_size: text.size * s, weight: crate::health_view::weight_of(text.style), ..default() };
                let color = crate::health_view::rgba(text.color);
                let at = Node { position_type: PositionType::Absolute, left: Val::Px((x + cx - tx) * s), top: Val::Px((y + cy - ty) * s), min_width: Val::Px(cw * s), height: Val::Px(ch * s), ..default() };
                let effect = look.text_effect(&views.health, text.font);
                let tmp = look.tmp_of(&views.health, text);
                root.spawn((BattleEffect, at)).with_children(|frame| match tmp {
                    // As the game draws it.
                    Some((asset, material)) => {
                        let run = trp_data::tmp_text::TmpRun { text: count.to_string(), size: text.size * s, color: crate::health_view::screen_numbers(color), bold: text.style & 1 != 0 };
                        let layout = trp_data::tmp_text::TmpLayout { em: text.size * s, spacing: text.spacing, kerning: text.kerning, wrap: None, align: text.horizontal & 7 };
                        crate::health_view::tmp_words(frame, Node::default(), crate::health_view::TmpText { asset, material, runs: vec![run], layout }, &mut |_| {});
                    }
                    None => crate::health_view::effect_words(frame, Node::default(), &[(count.to_string(), font, color)], effect, text.size * s, &mut |_, _, _| {}),
                });
            }
        }
    });
}

/// Draws each panel at its fade (`CanvasGroup.alpha`): every object under
/// it at that share of its own colours, the panel hidden while out.
fn fade_battle(state: Res<BattleState>, mut roots: Query<(Entity, &BattleRoot, &mut Visibility)>, children: Query<&Children>, mut colours: Query<(&Own, Option<&mut ImageNode>, Option<&mut BackgroundColor>, Option<&mut TextColor>)>) {
    for (root, panel, mut visibility) in &mut roots {
        let alpha = match panel.panel {
            0 => state.stance.alpha,
            1 => state.health.alpha,
            _ => state.quick.alpha,
        };
        let want = if alpha > 0.0 { Visibility::Inherited } else { Visibility::Hidden };
        if *visibility != want {
            *visibility = want;
        }
        if alpha <= 0.0 {
            continue;
        }
        for entity in children.iter_descendants(root) {
            let Ok((own, image, ground, text)) = colours.get_mut(entity) else { continue };
            let faded = |color: Color| color.with_alpha(color.alpha() * alpha);
            if let (Some(mut image), Some(color)) = (image, own.image) {
                let color = faded(color);
                if image.color != color {
                    image.color = color;
                }
            }
            if let (Some(mut ground), Some(color)) = (ground, own.ground) {
                let color = faded(color);
                if ground.0 != color {
                    ground.0 = color;
                }
            }
            if let (Some(mut text), Some(color)) = (text, own.text) {
                let color = faded(color);
                if text.0 != color {
                    text.0 = color;
                }
            }
        }
    }
}
