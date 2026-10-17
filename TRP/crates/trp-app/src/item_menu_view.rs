//! The menu of a right click on a thing in the inventory
//! (`docs/spec/inventory.md`, "The menu of a right click"): what it holds
//! while it is open, its drawing (as the game's with `TARKOV UI`, else the
//! app's way) and the line under the mouse. Opening it, doing a line and
//! shutting it are the inventory's (`inventory_view::drag`), where what a
//! line does is.

use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;
use trp_sim::inventory::Id;
use trp_sim::item_menu::{Can, Line};

use crate::health_view::{HealthLook, Words};

/// The menu while it is open.
#[derive(Resource, Default)]
pub struct ItemMenu(pub Option<OpenMenu>);

pub struct OpenMenu {
    pub id: Id,
    /// Each line, whether it can be done now, and its caption.
    pub lines: Vec<(Line, Can, String)>,
    /// Where the right button was let go, in the window's pixels: the
    /// menu's top left (`SimpleContextMenu.ShowMenu`: `transform.position`),
    /// moved in to keep it on the screen.
    pub cursor: Vec2,
    /// The line under the mouse.
    pub hover: Option<usize>,
}

/// The drawn menu, all of it.
#[derive(Component)]
struct MenuRoot;

/// A line's place, for the mouse: the line's number.
#[derive(Component)]
struct MenuHit(usize);

pub struct ItemMenuPlugin;

impl Plugin for ItemMenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ItemMenu>().add_systems(Update, (hover, draw).chain());
    }
}

/// The line under the mouse; lit as it comes onto it (`OnPointerEnter`).
fn hover(mut menu: ResMut<ItemMenu>, hits: Query<(&MenuHit, &RelativeCursorPosition)>) {
    let Some(open) = menu.0.as_ref() else { return };
    let over = hits.iter().find(|(_, at)| at.cursor_over()).map(|(hit, _)| hit.0);
    if over != open.hover {
        if let Some(open) = menu.0.as_mut() {
            open.hover = over;
        }
    }
}

/// The app's colours for its own look of the menu.
const PLATE: Color = Color::srgb(0.12, 0.13, 0.13);
const LIT: Color = Color::srgb(0.25, 0.27, 0.27);
const WORDS: Color = Color::srgb(0.85, 0.86, 0.84);
const FRAME: Color = Color::srgb(0.32, 0.35, 0.35);

/// Draws the menu afresh whenever it changes: opened, lit, shut.
fn draw(
    mut commands: Commands,
    menu: Res<ItemMenu>,
    look: Res<HealthLook>,
    said: Res<Words>,
    window: Query<&Window>,
    drawn: Query<Entity, With<MenuRoot>>,
) {
    if !menu.is_changed() {
        return;
    }
    for entity in &drawn {
        commands.entity(entity).despawn();
    }
    let (Some(open), Ok(window)) = (menu.0.as_ref(), window.single()) else { return };
    let s = window.height() / 1080.0;
    let screen = Vec2::new(window.width(), window.height());
    let captions: Vec<&str> = open.lines.iter().map(|(_, _, caption)| caption.as_str()).collect();
    let looks: Vec<crate::health_view::MenuLineLook> = open
        .lines
        .iter()
        .enumerate()
        .map(|(index, (line, can, _))| crate::health_view::MenuLineLook { name: line.name(), caption: captions[index], enabled: can.is_ok(), hovered: open.hover == Some(index), arrow: line.has_lines() })
        .collect();
    let root = Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() };
    // The reason a line under the mouse cannot be done, for the tooltip.
    let reason = open.hover.and_then(|index| open.lines[index].1.err()).filter(|key| !key.is_empty()).map(|key| said.word(key, key));
    let built = look.tarkov_ui.then(|| crate::health_view::menu_layout(&look, &looks)).flatten();
    commands.spawn((MenuRoot, root, GlobalZIndex(40))).with_children(|layer| {
        let tooltip_at: Vec2;
        match built {
            // The game's: laid out by its layout, its top left at the
            // mouse, moved in to stay on the screen (`CorrectPosition`).
            Some(built) => {
                let size = Vec2::new(built.size[0], built.size[1]) * s;
                let corner = open.cursor.min(screen - size).max(Vec2::ZERO);
                let origin = [corner.x / s, corner.y / s];
                crate::health_view::draw_menu(layer, &look, &said, &built, origin, s);
                for (index, [x, y, w, h]) in built.lines.iter().enumerate() {
                    let hit = Node { position_type: PositionType::Absolute, left: Val::Px((origin[0] + x) * s), top: Val::Px((origin[1] + y) * s), width: Val::Px(w * s), height: Val::Px(h * s), ..default() };
                    layer.spawn((MenuHit(index), hit, RelativeCursorPosition::default()));
                }
                let hovered = open.hover.and_then(|index| built.lines.get(index));
                tooltip_at = hovered.map_or(corner, |[x, y, w, _]| Vec2::new((origin[0] + x + w + 4.0) * s, (origin[1] + y) * s));
            }
            // The app's: the same lines in its own colours.
            None => {
                let row_height = 20.0 * s;
                let width = 190.0 * s;
                let size = Vec2::new(width + 2.0 * s, open.lines.len() as f32 * (row_height + 2.0 * s) + 6.0 * s);
                let corner = open.cursor.min(screen - size).max(Vec2::ZERO);
                let column = Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(corner.x),
                    top: Val::Px(corner.y),
                    width: Val::Px(size.x),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(2.0 * s),
                    padding: UiRect::all(Val::Px(3.0 * s)),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                };
                layer.spawn((column, BackgroundColor(Color::BLACK.with_alpha(0.85)), BorderColor::all(FRAME))).with_children(|lines| {
                    for (index, (line, can, caption)) in open.lines.iter().enumerate() {
                        let alpha = if can.is_ok() { 1.0 } else { 0.3 };
                        let ground = if open.hover == Some(index) { LIT } else { PLATE };
                        let row = Node { height: Val::Px(row_height), padding: UiRect::horizontal(Val::Px(8.0 * s)), align_items: AlignItems::Center, justify_content: JustifyContent::SpaceBetween, ..default() };
                        lines.spawn((MenuHit(index), RelativeCursorPosition::default(), row, BackgroundColor(ground.with_alpha(alpha)))).with_children(|row| {
                            row.spawn((Text::new(caption.to_uppercase()), TextFont { font_size: 12.0 * s, ..default() }, TextColor(WORDS.with_alpha(alpha))));
                            if line.has_lines() {
                                row.spawn((Text::new(">"), TextFont { font_size: 12.0 * s, ..default() }, TextColor(WORDS.with_alpha(alpha))));
                            }
                        });
                    }
                });
                tooltip_at = Vec2::new(corner.x + size.x + 4.0 * s, corner.y + open.hover.map_or(0.0, |index| index as f32 * (row_height + 2.0 * s)));
            }
        }
        // Why the line under the mouse cannot be done (`SimpleTooltip`,
        // `SetButtonInteraction`): beside the line, in the tooltip's look.
        if let Some(reason) = reason {
            let node = Node {
                position_type: PositionType::Absolute,
                left: Val::Px(tooltip_at.x.min(screen.x - 200.0 * s).max(0.0)),
                top: Val::Px(tooltip_at.y),
                padding: UiRect { left: Val::Px(6.0 * s), right: Val::Px(6.0 * s), top: Val::Px(2.0 * s), bottom: Val::Px(5.0 * s) },
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            };
            let (font, size, color, frame) = look.tooltip_words();
            layer.spawn((node, BackgroundColor(Color::BLACK), BorderColor::all(frame))).with_children(|tip| {
                tip.spawn((Text::new(reason), TextFont { font, font_size: size * s, ..default() }, TextColor(color), TextLayout::new_with_no_wrap()));
            });
        }
    });
}
