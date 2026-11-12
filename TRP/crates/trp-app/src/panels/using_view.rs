//! The panel shown while the character examines a thing, with its name and a filling bar.
//!
//! The game scene's panel of an examine (`UsingPanel`, in the game's own
//! interface scene; inventory spec, "Examining", "The panel of an
//! examine"): while the character examines a thing, "EXAMINING" over the
//! thing's full name (shown though it is not known; broken into lines, the
//! panel growing upward), and a bar filled from the left by the wall clock
//! over the examine's time, at the screen's bottom middle, over the battle
//! screen and the open inventory alike (its canvas, the game scene's, is
//! sorted over the inventory's). Shown on the examine's first frame and
//! gone, all of it at once, on the frame its own clock runs out
//! (`UsingPanel.OnPlayerExamining`, `ShowPanel`, `Progress`); no fade, no
//! sound. Drawn from the scene as the battle screen's panels are, with
//! `TARKOV UI` (the app's own look has none); with `EXAMINING` off nothing
//! is ever examined.
//!
//! It follows the examine under way (`Carried::examiner`), however it was
//! begun: an examine begun before `show_using` runs is shown on its frame.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::screens::health_view::{draw_nodes, Frame, FullWidth, HealthLook, Marks, Piece, Words};
use crate::screens::inventory_view::Carried;

pub struct UsingPlugin;

impl Plugin for UsingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, show_using.after(crate::screens::examine_view::finish).run_if(crate::running));
    }
}

/// Its place among the screen's layers: over the inventory's page and its
/// quick-use bar (12), under its windows (from 14), what the mouse carries
/// and the preloader's notices, menus and tooltips (the game scene's
/// canvas is sorted 1, over the inventory's 0 and under the preloader's
/// 100: `courses10`'s takes' canvases).
pub(crate) const USING_Z: i32 = 13;

/// Where its objects are numbered among the drawn pieces.
const USING: usize = 34 << 20;

/// The panel drawn: for which examine (its thing and when it began), its
/// name and the window's size.
#[derive(Component, PartialEq)]
pub(crate) struct UsingRoot(String);

/// The thing's full name as the panel writes it (`Item.Name.Localized()`):
/// the install's words, else the database's.
fn full_name(carried: &Carried, kind: &trp_sim::items::inventory::Kind) -> String {
    carried.texts.names.get(&kind.template).filter(|name| !name.is_empty()).cloned().unwrap_or_else(|| kind.name.clone())
}

/// How high the name's text asks to be, in units: its words as TextMeshPro
/// lays them out in its width (a line 15.4, two 31.36: `courses10`'s
/// `examine-multiple`, "Health Resort west wing room 207 key" on two), its
/// margins above and below; a line of the font's height (1.1 of its size,
/// as the game's measures it) where the game's face is not had.
fn name_height(look: &HealthLook, panel: &trp_data::interface::screen::ScenePanel, index: usize, name: &str) -> f32 {
    let Some(text) = panel.nodes[index].text.as_ref() else { return 0.0 };
    let width = panel.nodes[index].layout.size_delta[0] - text.margin[0] - text.margin[2];
    let measured = look.tmp_of(panel, text).and_then(|(asset, _)| look.tmp_faces.get(&asset)).map(|face| {
        let runs = [trp_data::interface::tmp_text::TmpRun { text: name.to_owned(), size: text.size, color: [1.0; 4], bold: text.style & 1 != 0 }];
        let layout = trp_data::interface::tmp_text::TmpLayout { em: text.size, spacing: text.spacing, kerning: text.kerning, wrap: Some(width), align: text.horizontal & 7, lead: 0.0 };
        trp_data::interface::tmp_text::measure(face, &runs, &layout)[1]
    });
    measured.unwrap_or(text.size * 1.1) + text.margin[1] + text.margin[3]
}

/// Shows the panel while an examine is under way and its own clock has
/// not run out, drawn afresh for each examine; its bar's fill the share of
/// the time gone (`fillAmount`: 0 on its first frame).
#[allow(clippy::too_many_arguments)]
pub(crate) fn show_using(
    mut commands: Commands,
    carried: Res<Carried>,
    look: Res<HealthLook>,
    said: Res<Words>,
    window: Query<&Window, With<PrimaryWindow>>,
    time: Res<Time<Real>>,
    roots: Query<(Entity, &UsingRoot)>,
    mut fills: Query<(&Piece, &FullWidth, &mut Node, Option<&bevy::ui::ComputedNode>)>,
) {
    let now = time.elapsed_secs_f64();
    let under_way = carried.examiner.under_way.as_ref().filter(|under_way| now < under_way.began + under_way.time as f64);
    let using = look.using.as_ref().filter(|_| look.tarkov_ui);
    let window = window.single().ok().filter(|window| window.height() > 0.0);
    let (Some(examination), Some(using), Some(window)) = (under_way, using, window) else {
        for (entity, _) in &roots {
            commands.entity(entity).despawn();
        }
        return;
    };
    let share = examination.done(now);
    let name = full_name(&carried, &examination.kind);
    let size = [window.width().round() as u32, window.height().round() as u32];
    let key = UsingRoot(format!("{:?} {} {size:?} {name}", examination.item, examination.began.to_bits()));
    if roots.iter().any(|(_, root)| *root == key) {
        // Its fill, as wide as the share of its whole.
        let Some(fill) = using.fill else { return };
        for (piece, full, mut node, laid) in &mut fills {
            if piece.0 == USING + fill {
                crate::screens::health_view::set_fill_width(&mut node, laid, full.0 * share);
            }
        }
        return;
    }
    for (entity, _) in &roots {
        commands.entity(entity).despawn();
    }
    let s = size[1] as f32 / 1080.0;
    let screen = [size[0] as f32 / s, 1080.0];
    // Its name's text as high as its words ask, which its groups lay the
    // panel out by (the name's plate and the panel's description fitted to
    // what they hold: the caption raised over a name of two lines); the
    // fill as full as the share.
    let mut panel = using.panel.clone();
    if let Some(index) = using.name {
        let high = name_height(&look, &panel, index, &name);
        let wide = panel.nodes[index].layout.size_delta[0];
        crate::screens::health_view::prefer_size(&mut panel, index, [wide, high]);
    }
    if let Some(image) = using.fill.and_then(|fill| panel.nodes[fill].image.as_mut()) {
        image.fill_amount = share;
    }
    let caption = carried.texts.say("EXAMINING").unwrap_or_else(|| "EXAMINING".to_owned());
    let words: std::collections::HashMap<usize, String> = using.caption.map(|index| (index, caption)).into_iter().collect();
    let [r, g, b, a] = using.examine_color;
    let colors: std::collections::HashMap<usize, Color> = using.fill.map(|index| (index, Color::srgba(r, g, b, a))).into_iter().collect();
    let skip: Vec<usize> = using.name.into_iter().collect();
    let whole = Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() };
    info!("the panel of an examine shown: {name}");
    commands.spawn((key, Name::new("UsingPanel"), whole, GlobalZIndex(USING_Z))).with_children(|root| {
        let marks = Marks { words: Some(&words), colors: Some(&colors), skip: &skip, ..default() };
        draw_nodes(root, &look, &panel, &look.using_drawn, USING, marks, &said, Frame::In(screen), [0.0, 0.0], s);
        // The name, broken into lines within its text's width, each line in
        // its middle.
        let Some((index, text)) = using.name.and_then(|index| Some((index, panel.nodes[index].text.as_ref()?))) else { return };
        let [x, y, w, h] = panel.placed_in(index, screen).rect;
        let color = crate::screens::health_view::rgba(text.color);
        let frame = Node {
            position_type: PositionType::Absolute,
            left: Val::Px(x * s),
            top: Val::Px(y * s),
            width: Val::Px(w * s),
            height: Val::Px(h * s),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        };
        root.spawn((Name::new("UsingItem/Text"), frame)).with_children(|frame| match look.tmp_of(&panel, text) {
            Some((asset, material)) => {
                let run = trp_data::interface::tmp_text::TmpRun { text: name.clone(), size: text.size * s, color: crate::screens::health_view::screen_numbers(color), bold: text.style & 1 != 0 };
                let layout = trp_data::interface::tmp_text::TmpLayout { em: text.size * s, spacing: text.spacing, kerning: text.kerning, wrap: Some((w - text.margin[0] - text.margin[2]) * s), align: text.horizontal & 7, lead: 0.0 };
                crate::screens::health_view::tmp_words(frame, Node::default(), crate::screens::health_view::TmpText { asset, material, runs: vec![run], layout }, &mut |_| {});
            }
            None => {
                let font = TextFont { font: look.styled_font(&panel, text.font, text.style), font_size: text.size * s, ..default() };
                frame.spawn((Node { width: Val::Px(w * s), ..default() }, Text::new(name.clone()), font, TextColor(color), TextLayout::new(Justify::Center, LineBreak::WordBoundary)));
            }
        });
    });
}
