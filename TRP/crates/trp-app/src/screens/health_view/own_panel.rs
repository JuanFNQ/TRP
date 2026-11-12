//! The app's own health panel and its figures.

use super::*;

/// The figures of the app's own panel: each one's field on the game's
/// panel, the key of its words in the install's texts and the app's word
/// where the install has none. Blood pressure, which the game never
/// writes (its `120/80` is the scene's own), is left out.
pub(super) const FIGURES: [(&str, &str, &str); 6] = [
    ("OverallHealth", "Health", "HEALTH"),
    ("Hydration", "HYDRATION", "HYDRATION"),
    ("Energy", "ENERGY", "ENERGY"),
    ("Temperature", "Temperature", "TEMPERATURE"),
    ("Poisoning", "Poisoning", "POISONING"),
    ("Radiation", "Radiation", "RADIATION"),
];

/// The game's view of a part and its figures, in units: a view's bar
/// (its header is above it, its effects under it) and a figure's panel
/// (`probe healthpanel`).
const VIEW: [f32; 2] = [130.0, 19.0];

const HEADER: f32 = 21.0;

const FIGURE: [f32; 2] = [180.0, 46.0];

/// Where the app's own panel puts each part's view (in `Part::ALL`'s
/// order) and each figure (in `FIGURES`' order) where the game's panel
/// could not be read: the game's places on a 16:9 screen (read by
/// `probe healthpanel`), from the equipment panel's
/// top left corner, in units.
const OWN_PARTS: [[f32; 2]; 7] = [[325.1, 65.5], [196.1, 164.1], [196.1, 243.8], [362.2, 243.8], [29.8, 243.8], [315.1, 405.8], [75.1, 405.8]];

const OWN_FIGURES: [[f32; 2]; 6] = [[161.0, 612.0], [177.5, 729.0], [366.9, 729.0], [-14.6, 681.0], [177.5, 681.0], [366.9, 681.0]];

/// The app's own panel's parts: a part's figures and its bar's fill, by
/// the part's place in `Part::ALL`; a figure's value and its rate, by its
/// place in `FIGURES`.
#[derive(Component)]
pub(super) struct OwnFigures(usize);

#[derive(Component)]
pub(super) struct OwnFill(usize);

#[derive(Component)]
pub(super) struct OwnValue(usize);

#[derive(Component)]
pub(super) struct OwnRate(usize);

/// The app's colours for a part's bar: whole, hurt, badly hurt,
/// destroyed (the app's own).
const GOOD: Color = Color::srgb(0.42, 0.60, 0.30);

const HURT: Color = Color::srgb(0.78, 0.60, 0.22);

const BAD: Color = Color::srgb(0.74, 0.22, 0.18);

const GONE: Color = Color::srgb(0.10, 0.10, 0.10);

fn shade(share: f32, destroyed: bool) -> Color {
    if destroyed {
        GONE
    } else if share > 0.6 {
        GOOD
    } else if share > 0.3 {
        HURT
    } else {
        BAD
    }
}

/// The app's own HEALTH panel (`TARKOV UI` off): a box for each part (its
/// name, its figures, a bar and the effects on it) and the figures, in the
/// app's style, each where the game's panel has it in a left panel of
/// that size, the panel's top left at `origin` (units, in what it is drawn
/// into); over the character's model, which the page draws where the gear
/// tab has it. Where the game's panel could not be read, at its places on
/// a 16:9 screen from the equipment panel's top left, `gear`.
#[allow(clippy::too_many_arguments)]
pub fn draw_own_panel(column: &mut ChildSpawnerCommands, look: &HealthLook, said: &Words, health: Option<&Health>, left: Option<[f32; 2]>, origin: [f32; 2], gear: [f32; 2], s: f32) {
    if health.is_none() {
        column.spawn(words("The options' HEALTH SYSTEM is off: the character cannot be hurt, and is taken as fed and watered.", 13.0, WORDS));
        return;
    }
    let game = look.panel.as_ref().zip(left);
    let place = |index: usize, fallback: [f32; 2]| match game {
        Some((game, left)) => {
            let [x, y, ..] = game.panel.rect(index, left);
            [origin[0] + x, origin[1] + y]
        }
        None => [gear[0] + fallback[0], gear[1] + fallback[1]],
    };
    let at = |[x, y]: [f32; 2], width: f32| Node { position_type: PositionType::Absolute, left: Val::Px(x * s), top: Val::Px(y * s), width: Val::Px(width * s), ..default() };
    for (n, part) in Part::ALL.iter().enumerate() {
        let view = game.and_then(|(game, _)| game.views.parts.get(n)).map(|view| view.view);
        let [x, y] = view.map_or([gear[0] + OWN_PARTS[n][0], gear[1] + OWN_PARTS[n][1]], |view| place(view, OWN_PARTS[n]));
        let node = Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), padding: UiRect::all(Val::Px(4.0)), border: UiRect::all(Val::Px(1.0)), ..at([x, y - HEADER], VIEW[0]) };
        // (A thing let go on the box is used on the part: `HealthDrop`.)
        column.spawn((HealthDrop(n), RelativeCursorPosition::default(), node, BorderColor::all(crate::screens::inventory_view::LINE), BackgroundColor(crate::screens::inventory_view::PANEL.with_alpha(0.85)))).with_children(|cell| {
            cell.spawn(words(said.part(*part), 10.0, FAINT));
            cell.spawn((OwnFigures(n), words("", 12.0, WORDS)));
            let track = Node { width: Val::Percent(100.0), height: Val::Px(5.0), ..default() };
            cell.spawn((track, BackgroundColor(crate::screens::inventory_view::LINE))).with_children(|track| {
                track.spawn((OwnFill(n), Node { width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() }, BackgroundColor(GOOD)));
            });
            cell.spawn((EffectWords(n), words("", 10.0, FAINT)));
        });
    }
    for (n, (name, key, otherwise)) in FIGURES.iter().enumerate() {
        let figure = game.and_then(|(game, _)| game.views.parameter(name)).map(|figure| figure.panel);
        let [x, y] = figure.map_or([gear[0] + OWN_FIGURES[n][0], gear[1] + OWN_FIGURES[n][1]], |figure| place(figure, OWN_FIGURES[n]));
        // A box as a part's: its name and its rate over its figures.
        let node = Node {
            height: Val::Px(FIGURE[1] * s),
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::Center,
            row_gap: Val::Px(2.0),
            padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)),
            border: UiRect::all(Val::Px(1.0)),
            ..at([x, y], FIGURE[0])
        };
        column.spawn((node, BorderColor::all(crate::screens::inventory_view::LINE), BackgroundColor(crate::screens::inventory_view::PANEL.with_alpha(0.85)))).with_children(|cell| {
            cell.spawn(Node { justify_content: JustifyContent::SpaceBetween, ..default() }).with_children(|row| {
                row.spawn((words(said.say(key, otherwise).to_uppercase(), 10.0, FAINT), TextLayout::new_with_no_wrap()));
                row.spawn((OwnRate(n), words("", 10.0, FAINT), TextLayout::new_with_no_wrap()));
            });
            cell.spawn((OwnValue(n), words("", 15.0, WORDS), TextLayout::new_with_no_wrap()));
        });
    }
}

/// Keeps the app's own panel, every frame, and each part's effects in
/// words on either panel: the same figures as the game's panel shows.
#[allow(clippy::type_complexity)]
pub(super) fn fill_own(
    controller: Res<Controller>,
    said: Res<Words>,
    mut texts: Query<(&mut Text, &mut TextColor, Option<&OwnFigures>, Option<&OwnValue>, Option<&OwnRate>, Option<&EffectWords>), Or<(With<OwnFigures>, With<OwnValue>, With<OwnRate>, With<EffectWords>)>>,
    mut fills: Query<(&OwnFill, &mut Node, &mut BackgroundColor)>,
) {
    let Some(health) = controller.health.as_ref() else { return };
    if texts.is_empty() {
        return;
    }
    let rows = figure_rows(health);
    for (mut text, mut color, figures, value, rate, effects) in &mut texts {
        let (now, tint) = if let Some(OwnFigures(n)) = figures {
            let state = health.part(Part::ALL[*n]);
            let shown = if state.maximum / state.current < 2.0 { state.current.floor() } else { state.current.ceil() };
            let words = if state.destroyed { format!("{shown} / {}   DESTROYED", state.maximum) } else { format!("{shown} / {}", state.maximum) };
            (words, WORDS)
        } else if let Some(OwnValue(n)) = value {
            let Some(&(_, [current, maximum], _, _, places, _)) = rows.iter().find(|row| row.0 == FIGURES[*n].0) else { continue };
            let (_, written) = figure(current, maximum, places, -1);
            // Temperature has no maximum written, as on the game's panel.
            (if places > 0 { written } else { format!("{written} / {maximum:.0}") }, WORDS)
        } else if let Some(OwnRate(n)) = rate {
            match rows.iter().find(|row| row.0 == FIGURES[*n].0).and_then(|row| row.5) {
                Some((value, _, rise_good)) if value != 0.0 => (format!("{value:+.2}/min"), if (value > 0.0) == rise_good { GOOD } else { BAD }),
                _ => (String::new(), FAINT),
            }
        } else if let Some(EffectWords(n)) = effects {
            let part = Part::ALL[*n];
            let mut names: Vec<String> = health
                .effects
                .iter()
                .filter(|e| e.part == part && e.existing() && !matches!(e.kind, Kind::Encumbered | Kind::OverEncumbered))
                .filter_map(|e| said.effect(e.kind))
                .collect();
            names.dedup();
            (names.join(", "), color.0)
        } else {
            continue;
        };
        if text.0 != now {
            text.0 = now;
        }
        if color.0 != tint {
            color.0 = tint;
        }
    }
    for (fill, mut node, mut back) in &mut fills {
        let state = health.part(Part::ALL[fill.0]);
        let share = if state.maximum > 0.0 { (state.current / state.maximum).clamp(0.0, 1.0) } else { 0.0 };
        let width = Val::Percent((share * 100.0).round());
        if node.width != width {
            node.width = width;
        }
        let tint = shade(share, state.destroyed);
        if back.0 != tint {
            back.0 = tint;
        }
    }
}

/// What a frame wants of a drawn object: shown or not, its colour, how
/// much of it is filled, its words and what goes before them.
#[derive(Default, Clone)]
pub(super) struct Want {
    pub(super) visible: Option<bool>,
    pub(super) color: Option<Color>,
    pub(super) fill: Option<f32>,
    pub(super) before: Option<(String, Color)>,
    pub(super) words: Option<(String, Color)>,
    /// An arrow: whether its figure moves (`SetBuffValue`'s `active`, its
    /// animator's `Active`), and how far up it is drawn, in units.
    pub(super) moving: Option<bool>,
    pub(super) raised: f32,
}

/// `ColorSchemeExtension.GetBodyPartColor` with the green-to-red scheme:
/// the share squared as a hue from red to green, full; black destroyed.
pub(super) fn part_color(share: f32, destroyed: bool) -> Color {
    if destroyed {
        return Color::BLACK;
    }
    let share = share.clamp(0.0, 1.0);
    let [r, g, b] = hsv(share * share * 120.0, 1.0, 1.0);
    Color::srgb_u8(r, g, b)
}

/// `ColorUtilities.ColorFromHSV`: a hue in degrees, each channel a byte
/// rounded to the nearest (`Convert.ToByte`, a half to even).
pub(crate) fn hsv(hue: f32, saturation: f32, value: f32) -> [u8; 3] {
    let sector = (hue / 60.0).floor() as i32 % 6;
    let f = (hue / 60.0) as f64 - (hue / 60.0).floor() as f64;
    let value = value * 255.0;
    let byte = |x: f64| x.round_ties_even().clamp(0.0, 255.0) as u8;
    let v = byte(value as f64);
    let p = byte((value * (1.0 - saturation)) as f64);
    let q = byte(value as f64 * (1.0 - f * saturation as f64));
    let t = byte(value as f64 * (1.0 - (1.0 - f) * saturation as f64));
    match sector {
        0 => [v, t, p],
        1 => [q, v, p],
        2 => [p, v, t],
        3 => [p, q, v],
        4 => [t, p, v],
        _ => [v, p, q],
    }
}

/// A figure as `HealthParameterPanel.SetParameterValue` writes it: whole,
/// down where more than half is left and up where less (`F0`), or to so
/// many places (a half to even); and the noughts that make up the fewest
/// digits it is written with.
pub(super) fn figure(current: f32, maximum: f32, places: i32, least: i32) -> (String, String) {
    let text = if places <= 0 {
        let whole = if current / maximum > 0.5 { current.floor() } else { current.ceil() };
        format!("{whole:.0}")
    } else {
        let scale = 10f64.powi(places);
        format!("{}", (current as f64 * scale).round_ties_even() / scale)
    };
    let short = (least - text.chars().count() as i32).max(0) as usize;
    ("0".repeat(short), text)
}

/// One of the figures the panel writes (`HealthParametersPanel.
/// UpdateMainHealthAndStats`): its field's name, its current and maximum,
/// its warning and whether it is reached from above, its places, and its
/// rate a minute with whether it shows it moving (short of its maximum)
/// and whether a rise is good.
pub(super) type FigureRow = (&'static str, [f32; 2], f32, bool, i32, Option<(f32, bool, bool)>);

pub(super) fn figure_rows(health: &Health) -> [FigureRow; 6] {
    let rates = health.rates();
    let settings = &health.settings;
    let common = health.common();
    [
        ("OverallHealth", [common[0].round_ties_even(), common[1]], 10.0, false, 0, Some((rates[0] * 60.0, common[0] < common[1], true))),
        ("Hydration", [health.hydration[0], health.hydration[2]], 10.0, false, 0, Some((rates[2] * 60.0, health.hydration[0] < health.hydration[2], true))),
        ("Energy", [health.energy[0], health.energy[2]], 10.0, false, 0, Some((rates[1] * 60.0, health.energy[0] < health.energy[2], true))),
        ("Temperature", [settings.temperature[2], settings.temperature[1]], 10.0, false, 1, Some((0.0, settings.temperature[2] < settings.temperature[1], false))),
        ("Poisoning", [settings.poisoning[2], settings.poisoning[1]], 100.0, true, 0, None),
        ("Radiation", [0.0, 100.0], -1.0, false, 0, None),
    ]
}

/// The rate's colours (`BuffableHealthParameterPanel`).
pub(super) const RISING: Color = Color::srgb(0.349, 0.667, 0.835);

pub(super) const FALLING: Color = Color::srgb(0.729, 0.0, 0.0);
