//! The menu of a right click on a thing in the inventory
//! (`docs/spec/inventory.md`, "The menu of a right click"): what it holds
//! while it is open, its drawing (as the game's with `TARKOV UI`, else the
//! app's way), the line under the mouse, and the menu of a line's own
//! lines (the wish list's: "The wish list"), opened and shut by the mouse.
//! Opening it, doing a line and shutting it are the inventory's
//! (`inventory_view::drag`), where what a line does is.

use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;
use trp_sim::inventory::Id;
use trp_sim::item_menu::{Can, Line, WishLine};

use crate::health_view::{HealthLook, MenuLineLook, Words};
use crate::inventory_view::Carried;

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
    /// The menu of a line's own lines, while it is open.
    pub sub: Option<SubMenu>,
}

/// The menu of a line's own lines (`InteractionButtonsContainer.
/// SetSubInteractions`): the line it is of, its lines, each with whether
/// it can be done now and its caption, and the line under the mouse.
#[derive(Clone, PartialEq)]
pub struct SubMenu {
    pub parent: usize,
    pub lines: Vec<(WishLine, Can, String)>,
    pub hover: Option<usize>,
}

/// The drawn menu, all of it.
#[derive(Component)]
struct MenuRoot;

/// A line's place, for the mouse: the line's number.
#[derive(Component)]
struct MenuHit(usize);

/// A line of the menu of lines, for the mouse.
#[derive(Component)]
struct SubHit(usize);

/// A menu's whole place, for the mouse: the first's, or (true) the menu of
/// lines'.
#[derive(Component)]
struct MenuArea(bool);

pub struct ItemMenuPlugin;

impl Plugin for ItemMenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ItemMenu>().add_systems(Update, (hover, draw).chain());
    }
}

/// The line under the mouse; lit as it comes onto it (`OnPointerEnter`).
/// A line the mouse comes onto shuts the menu of lines and opens its own,
/// where it has lines it can open (`CreateInternalContextButton`'s hover:
/// the wish list's for the thing's kind); the mouse out of both menus shuts
/// it (`OnPointerExit`, unless into the other).
fn hover(
    mut menu: ResMut<ItemMenu>,
    hits: Query<(&MenuHit, &RelativeCursorPosition)>,
    sub_hits: Query<(&SubHit, &RelativeCursorPosition)>,
    areas: Query<(&MenuArea, &RelativeCursorPosition)>,
    carried: Res<Carried>,
    said: Res<Words>,
) {
    let Some(open) = menu.0.as_ref() else { return };
    let over = hits.iter().find(|(_, at)| at.cursor_over()).map(|(hit, _)| hit.0);
    let over_sub = sub_hits.iter().find(|(_, at)| at.cursor_over()).map(|(hit, _)| hit.0);
    let in_area = |sub: bool| areas.iter().any(|(area, at)| area.0 == sub && at.cursor_over());
    let mut sub = open.sub.clone();
    if over != open.hover {
        if let Some(index) = over {
            let template = carried.find(open.id).map(|item| item.kind.template.clone());
            sub = open.lines.get(index).filter(|(line, can, _)| line.has_lines() && can.is_ok()).zip(template).map(|(_, template)| SubMenu {
                parent: index,
                lines: trp_sim::item_menu::wish_lines(&template, &carried.wishlist)
                    .into_iter()
                    .map(|(line, can)| (line, can, said.word(line.caption_key(), line.name())))
                    .collect(),
                hover: None,
            });
        }
    }
    if sub.is_some() && over.is_none() && !in_area(false) && !in_area(true) {
        sub = None;
    }
    if let Some(sub) = sub.as_mut() {
        sub.hover = over_sub;
    }
    if over != open.hover || sub != open.sub {
        if let Some(open) = menu.0.as_mut() {
            open.hover = over;
            open.sub = sub;
        }
    }
}

/// Where a menu of lines goes (`SetSubInteractions`): its top left at its
/// line's top right, moved in to stay on the screen (`CorrectPosition`);
/// where that moved it sideways, at the line's left instead (its right
/// edge there), at the height it was moved to. In the window's pixels.
fn sub_corner(line_right_top: Vec2, line_left: f32, size: Vec2, screen: Vec2) -> Vec2 {
    let moved = line_right_top.min(screen - size).max(Vec2::ZERO);
    if (moved.x - line_right_top.x).abs() > 1e-3 {
        Vec2::new(line_left - size.x, moved.y)
    } else {
        moved
    }
}

/// The app's colours for its own look of the menu.
const PLATE: Color = Color::srgb(0.12, 0.13, 0.13);
const LIT: Color = Color::srgb(0.25, 0.27, 0.27);
const WORDS: Color = Color::srgb(0.85, 0.86, 0.84);
const FRAME: Color = Color::srgb(0.32, 0.35, 0.35);

/// A node at a place of the window, in pixels.
fn at(x: f32, y: f32, w: f32, h: f32) -> Node {
    Node { position_type: PositionType::Absolute, left: Val::Px(x), top: Val::Px(y), width: Val::Px(w), height: Val::Px(h), ..default() }
}

/// The app's look of a menu: its lines in a column at that corner, each
/// lit, dimmed or with its arrow as it says, numbered for the mouse in the
/// first menu or the menu of lines (`sub`). Its size in pixels.
fn app_menu(layer: &mut ChildSpawnerCommands, lines: &[MenuLineLook], corner: Vec2, s: f32, sub: bool) -> Vec2 {
    let row_height = 20.0 * s;
    let size = app_menu_size(lines.len(), s);
    let column = Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0 * s), padding: UiRect::all(Val::Px(3.0 * s)), border: UiRect::all(Val::Px(1.0)), ..at(corner.x, corner.y, size.x, size.y) };
    layer.spawn((MenuArea(sub), RelativeCursorPosition::default(), column, BackgroundColor(Color::BLACK.with_alpha(0.85)), BorderColor::all(FRAME))).with_children(|column| {
        for (index, line) in lines.iter().enumerate() {
            let alpha = if line.enabled { 1.0 } else { 0.3 };
            let ground = if line.hovered { LIT } else { PLATE };
            let row = Node { height: Val::Px(row_height), padding: UiRect::horizontal(Val::Px(8.0 * s)), align_items: AlignItems::Center, justify_content: JustifyContent::SpaceBetween, ..default() };
            let mut spawned = column.spawn((RelativeCursorPosition::default(), row, BackgroundColor(ground.with_alpha(alpha))));
            if sub {
                spawned.insert(SubHit(index));
            } else {
                spawned.insert(MenuHit(index));
            }
            spawned.with_children(|row| {
                row.spawn((Text::new(line.caption.to_uppercase()), TextFont { font_size: 12.0 * s, ..default() }, TextColor(WORDS.with_alpha(alpha))));
                if line.arrow {
                    row.spawn((Text::new(">"), TextFont { font_size: 12.0 * s, ..default() }, TextColor(WORDS.with_alpha(alpha))));
                }
            });
        }
    });
    size
}

/// The app's menu's size in pixels for so many lines.
fn app_menu_size(lines: usize, s: f32) -> Vec2 {
    Vec2::new(192.0 * s, lines as f32 * 22.0 * s + 6.0 * s)
}

/// Draws the menu afresh whenever it changes: opened, lit, shut, its menu
/// of lines opened or shut.
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
    // A line with its menu of lines open stays lit (`Blocked`).
    let looks: Vec<MenuLineLook> = open
        .lines
        .iter()
        .enumerate()
        .map(|(index, (line, can, caption))| MenuLineLook {
            name: line.name(),
            caption,
            enabled: can.is_ok(),
            hovered: open.hover == Some(index) || open.sub.as_ref().is_some_and(|sub| sub.parent == index),
            arrow: line.has_lines(),
        })
        .collect();
    let sub_looks: Vec<MenuLineLook> = open.sub.as_ref().map_or_else(Vec::new, |sub| {
        sub.lines.iter().enumerate().map(|(index, (line, can, caption))| MenuLineLook { name: line.name(), caption, enabled: can.is_ok(), hovered: sub.hover == Some(index), arrow: false }).collect()
    });
    let root = Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() };
    // The reason a line under the mouse cannot be done, for the tooltip:
    // of the menu of lines' where the mouse is on it.
    let sub_reason = open.sub.as_ref().and_then(|sub| sub.hover.and_then(|index| sub.lines[index].1.err()));
    let reason = sub_reason.or_else(|| open.hover.and_then(|index| open.lines[index].1.err())).filter(|key| !key.is_empty()).map(|key| said.word(key, key));
    let built = look.tarkov_ui.then(|| crate::health_view::menu_layout(&look, &looks)).flatten();
    let sub_built = (look.tarkov_ui && !sub_looks.is_empty()).then(|| crate::health_view::submenu_layout(&look, &sub_looks)).flatten();
    commands.spawn((MenuRoot, root, GlobalZIndex(40))).with_children(|layer| {
        let mut tooltip_at: Vec2;
        match built {
            // The game's: laid out by its layout, its top left at the
            // mouse, moved in to stay on the screen (`CorrectPosition`).
            Some(built) => {
                let size = Vec2::new(built.size[0], built.size[1]) * s;
                let corner = open.cursor.min(screen - size).max(Vec2::ZERO);
                let origin = [corner.x / s, corner.y / s];
                crate::health_view::draw_menu(layer, &look, &said, &built, origin, s, false);
                layer.spawn((MenuArea(false), RelativeCursorPosition::default(), at(corner.x, corner.y, size.x, size.y)));
                for (index, [x, y, w, h]) in built.lines.iter().enumerate() {
                    layer.spawn((MenuHit(index), at((origin[0] + x) * s, (origin[1] + y) * s, w * s, h * s), RelativeCursorPosition::default()));
                }
                let hovered = open.hover.and_then(|index| built.lines.get(index));
                tooltip_at = hovered.map_or(corner, |[x, y, w, _]| Vec2::new((origin[0] + x + w + 4.0) * s, (origin[1] + y) * s));
                // Its menu of lines, by the line it is of.
                if let (Some(sub), Some(sub_built)) = (open.sub.as_ref(), sub_built) {
                    if let Some([x, y, w, _]) = built.lines.get(sub.parent) {
                        let sub_size = Vec2::new(sub_built.size[0], sub_built.size[1]) * s;
                        let (left, top) = ((origin[0] + x) * s, (origin[1] + y) * s);
                        let sub_at = sub_corner(Vec2::new(left + w * s, top), left, sub_size, screen);
                        let sub_origin = [sub_at.x / s, sub_at.y / s];
                        crate::health_view::draw_menu(layer, &look, &said, &sub_built, sub_origin, s, true);
                        layer.spawn((MenuArea(true), RelativeCursorPosition::default(), at(sub_at.x, sub_at.y, sub_size.x, sub_size.y)));
                        for (index, [x, y, w, h]) in sub_built.lines.iter().enumerate() {
                            layer.spawn((SubHit(index), at((sub_origin[0] + x) * s, (sub_origin[1] + y) * s, w * s, h * s), RelativeCursorPosition::default()));
                        }
                        if let Some([x, y, w, _]) = sub.hover.and_then(|index| sub_built.lines.get(index)) {
                            tooltip_at = Vec2::new((sub_origin[0] + x + w + 4.0) * s, (sub_origin[1] + y) * s);
                        }
                    }
                }
            }
            // The app's: the same lines in its own colours.
            None => {
                let size = app_menu_size(looks.len(), s);
                let corner = open.cursor.min(screen - size).max(Vec2::ZERO);
                app_menu(layer, &looks, corner, s, false);
                let row = |index: usize| corner.y + 3.0 * s + index as f32 * 22.0 * s;
                tooltip_at = Vec2::new(corner.x + size.x + 4.0 * s, open.hover.map_or(corner.y, row));
                if let Some(sub) = open.sub.as_ref() {
                    let sub_size = app_menu_size(sub_looks.len(), s);
                    let sub_at = sub_corner(Vec2::new(corner.x + size.x, row(sub.parent)), corner.x, sub_size, screen);
                    app_menu(layer, &sub_looks, sub_at, s, true);
                    if let Some(index) = sub.hover {
                        tooltip_at = Vec2::new(sub_at.x + sub_size.x + 4.0 * s, sub_at.y + 3.0 * s + index as f32 * 22.0 * s);
                    }
                }
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A menu of lines at its line's top right, kept on the screen; moved
    /// sideways by that, at the line's left at the height it was kept to.
    #[test]
    fn a_menu_of_lines_goes_right_of_its_line_or_left() {
        let screen = Vec2::new(1920.0, 1080.0);
        let size = Vec2::new(150.0, 120.0);
        assert_eq!(sub_corner(Vec2::new(800.0, 300.0), 650.0, size, screen), Vec2::new(800.0, 300.0));
        assert_eq!(sub_corner(Vec2::new(800.0, 1000.0), 650.0, size, screen), Vec2::new(800.0, 960.0));
        assert_eq!(sub_corner(Vec2::new(1850.0, 1000.0), 1700.0, size, screen), Vec2::new(1550.0, 960.0));
    }
}
