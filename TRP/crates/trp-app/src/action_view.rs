//! The battle screen's interaction panel as the game's with `TARKOV UI`
//! (`ActionPanel`; interactions spec, "The panel as the game draws it"): the
//! lines of what can be done to what is looked at, in a black box centred
//! 30.5 units over the screen's middle; a loose thing's name, or the error
//! where it offers nothing, 37 under it; and the pointer at the middle, a
//! hand over something to do or the sense's dot near a loose thing, fading
//! in and out at five a second (`ActionPanel.Update`). All of it 200 units
//! lower while aiming (`_aimingShift`). What is looked at and its lines
//! are `inventory_view::reach`'s; without the panel read, or without
//! `TARKOV UI`, the app's own prompt is drawn there.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::controller::Controller;
use crate::health_view::{rgba, screen_numbers, text_size, tmp_words, HealthLook, TmpText};
use crate::inventory_view::{MenuTexts, Reach};
use crate::weapon::HandsRig;
use crate::View;

pub struct ActionViewPlugin;

impl Plugin for ActionViewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Pointer>().add_systems(Update, (draw_action, fade_pointer).chain().after(crate::inventory_view::reach));
    }
}

/// The panel drawn: what it was drawn for, so that it is drawn afresh only
/// when that changes.
#[derive(Component)]
struct ActionRoot(Drawn);

#[derive(Clone, PartialEq)]
struct Drawn {
    size: [u32; 2],
    reach: Reach,
    aiming: bool,
}

/// The pointer's image.
#[derive(Component)]
struct PointerImage;

/// The pointer's alpha as it eases to its target, and which picture it
/// shows: the hand once something to do is looked at, the dot while the
/// sense alone is on; the picture is kept as it fades out
/// (`ShowHandCursor`, `ShowPointer`, `HideCursor`).
#[derive(Resource, Default)]
struct Pointer {
    alpha: f32,
    hand: bool,
}

/// The game's scale of its screens: the smaller of the window's width over
/// 1920 and its height over 1080 (`UICanvasScalerController`).
fn scale(size: [u32; 2]) -> f32 {
    (size[0] as f32 / 1920.0).min(size[1] as f32 / 1080.0)
}

#[allow(clippy::too_many_arguments)]
fn draw_action(
    mut commands: Commands,
    look: Res<HealthLook>,
    reach: Res<Reach>,
    texts: Res<MenuTexts>,
    view: Res<View>,
    controller: Res<Controller>,
    hands: Option<Res<HandsRig>>,
    window: Query<&Window, With<PrimaryWindow>>,
    roots: Query<(Entity, &ActionRoot)>,
) {
    let Some(parts) = look.battle.as_ref().and_then(|views| views.action.as_ref()).filter(|_| look.tarkov_ui) else { return };
    let Ok(window) = window.single() else { return };
    let size = [window.width().round() as u32, window.height().round() as u32];
    let playing = view.played() && !controller.inventory_open && !controller.edit_build;
    let aiming = hands.as_deref().and_then(|hands| hands.firearm.as_ref()).is_some_and(|firearm| firearm.aiming());
    let drawn = Drawn { size, reach: if playing { reach.clone() } else { Reach::default() }, aiming };
    if roots.iter().any(|(_, root)| root.0 == drawn) {
        return;
    }
    for (entity, _) in &roots {
        commands.entity(entity).despawn();
    }
    if size[1] == 0 {
        return;
    }
    let s = scale(size);
    let panel = &parts.panel;
    let image_color = |node: usize| panel.nodes[node].image.as_ref().map_or(Color::BLACK, |image| rgba(image.color));
    let text_color = |node: usize| panel.nodes[node].text.as_ref().map_or(Color::WHITE, |text| rgba(text.color));
    // The panel's middle is the screen's; while aiming it is 200 units
    // lower (Unity's y is up).
    let shift = if aiming { -parts.aiming_shift[1] } else { 0.0 };
    let whole = |down: f32, right: f32| Node {
        position_type: PositionType::Absolute,
        left: Val::Px(0.0),
        top: Val::Px(0.0),
        width: Val::Percent(100.0),
        height: Val::Percent(100.0),
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        // A centred child moved: by twice the shift in margin on one side.
        padding: UiRect {
            top: Val::Px((2.0 * down).max(0.0) * s),
            bottom: Val::Px((-2.0 * down).max(0.0) * s),
            left: Val::Px((2.0 * right).max(0.0) * s),
            right: Val::Px((-2.0 * right).max(0.0) * s),
        },
        ..default()
    };
    let root = Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() };
    commands.spawn((ActionRoot(drawn.clone()), root, GlobalZIndex(5))).with_children(|root| {
        let reach = &drawn.reach;
        // The pointer, at the middle, under the lines (the panel's `Cursor`
        // comes before them): its picture and alpha are `fade_pointer`'s.
        // (The texture is kept as the engine has it, its lowest row first:
        // drawn turned up.)
        root.spawn(whole(shift, 0.0)).with_children(|middle| {
            middle.spawn((PointerImage, Node { width: Val::Px(9.0 * s), height: Val::Px(9.0 * s), ..default() }, ImageNode { color: Color::NONE, flip_y: true, ..default() }));
        });
        // The lines (`Interaction Buttons Container`): a black box, three
        // units round them and two between, each line as wide as the
        // widest, its words centred in it less four left and five right
        // and one under; the chosen one grey with black words, the rest
        // black with grey words, one that cannot be chosen darker
        // (`InteractionButton`).
        if let Some(menu) = reach.menu.as_ref().filter(|menu| !menu.lines.is_empty()) {
            let light = image_color(parts.line_ground);
            let dark = image_color(parts.list_ground);
            let chosen_words = text_color(parts.line_text);
            root.spawn(whole(-30.5 + shift, -0.5)).with_children(|middle| {
                let list = Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Stretch,
                    padding: UiRect::all(Val::Px(3.0 * s)),
                    row_gap: Val::Px(2.0 * s),
                    ..default()
                };
                middle.spawn((list, BackgroundColor(dark))).with_children(|list| {
                    for (index, (action, disabled)) in menu.lines.iter().enumerate() {
                        let chosen = menu.selected == Some(index);
                        let (ground, ink) = match (chosen, *disabled) {
                            (true, _) => (light, chosen_words),
                            (false, true) => (dark, DISABLED),
                            (false, false) => (dark, light),
                        };
                        let line = Node {
                            justify_content: JustifyContent::Center,
                            padding: UiRect { left: Val::Px(4.0 * s), right: Val::Px(5.0 * s), top: Val::Px(0.0), bottom: Val::Px(1.0 * s) },
                            ..default()
                        };
                        list.spawn((line, BackgroundColor(ground))).with_children(|line| {
                            words(line, &look, panel, parts.line_text, texts.of(*action), ink, s);
                        });
                    }
                });
            });
        }
        // A loose thing's name, or the error where it offers no line, 37
        // under the middle: its words in capitals on its ground, which
        // reaches four left, five right and one under them.
        let named = match (&reach.error, &reach.name) {
            (Some(error), _) => Some((error.to_uppercase(), parts.error_ground, parts.error_text)),
            (None, Some(name)) if reach.menu.as_ref().is_some_and(|menu| menu.selected.is_some()) => Some((name.to_uppercase(), parts.name_ground, parts.name_text)),
            _ => None,
        };
        if let Some((said, ground, text)) = named {
            // (The words' panel is what is centred there: its ground
            // reaches 4 left of it and 5 right, and 1 under.)
            root.spawn(whole(37.5 + shift, 0.5)).with_children(|middle| {
                let panel_node = Node { padding: UiRect { left: Val::Px(4.0 * s), right: Val::Px(5.0 * s), top: Val::Px(0.0), bottom: Val::Px(1.0 * s) }, ..default() };
                middle.spawn((panel_node, BackgroundColor(image_color(ground)))).with_children(|panel_node| {
                    words(panel_node, &look, panel, text, said, text_color(text), s);
                });
            });
        }
    });
}

/// Words of the panel in the font, size and style of one of its texts, as
/// TextMeshPro draws them where its font asset was read, in a box of the
/// size they ask of the panel's layout (TextMeshPro's preferred size: the
/// advances with the asset's spacing, the font's ascender to its
/// descender), the words centred in it; else in the engine's text.
fn words(parent: &mut ChildSpawnerCommands, look: &HealthLook, panel: &trp_data::screen::ScenePanel, node: usize, said: String, color: Color, s: f32) {
    let Some(text) = panel.nodes[node].text.as_ref() else { return };
    // (`FontStyles.UpperCase`.)
    let said = if text.style & 16 != 0 { said.to_uppercase() } else { said };
    match look.tmp_of(panel, text) {
        Some((asset, material)) => {
            let room = match text_size(look, panel, node, &said) {
                Some([w, h]) => Node { width: Val::Px(w * s), height: Val::Px(h * s), align_items: AlignItems::Center, flex_shrink: 0.0, ..default() },
                None => Node::default(),
            };
            let run = trp_data::tmp_text::TmpRun { text: said, size: text.size * s, color: screen_numbers(color), bold: text.style & 1 != 0 };
            let layout = trp_data::tmp_text::TmpLayout { em: text.size * s, spacing: text.spacing, kerning: text.kerning, wrap: None, align: text.horizontal & 7 };
            tmp_words(parent, room, TmpText { asset, material, runs: vec![run], layout }, &mut |_| {});
        }
        None => {
            let font = TextFont { font: look.styled_font(panel, text.font, text.style), font_size: text.size * s, ..default() };
            parent.spawn((Text::new(said), font, TextColor(color), TextLayout::new_with_no_wrap()));
        }
    }
}

/// A line that cannot be chosen: its words' grey (`InteractionButton`).
const DISABLED: Color = Color::srgb(0.25, 0.25, 0.25);

/// The pointer each frame (`ActionPanel`): with lines to choose, the hand
/// at full; else, with the sense on and the sights down, the dot at full;
/// else nothing, the picture kept as it fades out. The alpha eases at five
/// a second (`Mathf.Lerp(alpha, target, Time.deltaTime × 5)`); the picture
/// at its own size (`SetNativeSize`).
fn fade_pointer(
    look: Res<HealthLook>,
    reach: Res<Reach>,
    view: Res<View>,
    controller: Res<Controller>,
    time: Res<Time>,
    window: Query<&Window, With<PrimaryWindow>>,
    mut pointer: ResMut<Pointer>,
    mut images: Query<(&mut ImageNode, &mut Node), With<PointerImage>>,
) {
    let Some(parts) = look.battle.as_ref().and_then(|views| views.action.as_ref()).filter(|_| look.tarkov_ui) else { return };
    let playing = view.played() && !controller.inventory_open && !controller.edit_build;
    let lines = playing && reach.menu.as_ref().is_some_and(|menu| !menu.lines.is_empty());
    let sense = playing && reach.sense;
    if lines {
        pointer.hand = true;
    } else if sense {
        pointer.hand = false;
    }
    let target = if lines || sense { 1.0 } else { 0.0 };
    if (pointer.alpha - target).abs() > 1e-4 {
        pointer.alpha += (target - pointer.alpha) * (time.delta_secs() * 5.0).min(1.0);
    }
    let sprite = if pointer.hand { parts.hover_sprite } else { parts.sense_sprite };
    let picture = sprite.and_then(|key| look.battle_sprites.get(&key)).and_then(|picture| Some((look.images.get(picture.texture)?.clone()?, picture.cut)));
    let Ok(window) = window.single() else { return };
    let s = scale([window.width().round() as u32, window.height().round() as u32]);
    for (mut image, mut node) in &mut images {
        let Some((handle, cut)) = picture.clone() else {
            image.color = Color::NONE;
            continue;
        };
        if image.image != handle || image.rect != Some(cut) {
            image.image = handle;
            image.rect = Some(cut);
        }
        let (width, height) = (Val::Px(cut.width() * s), Val::Px(cut.height() * s));
        if node.width != width || node.height != height {
            (node.width, node.height) = (width, height);
        }
        let color = Color::WHITE.with_alpha(pointer.alpha);
        if image.color != color {
            image.color = color;
        }
    }
}
