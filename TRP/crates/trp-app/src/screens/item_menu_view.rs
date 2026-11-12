//! The menu of a right click on a thing in the inventory, with its lines and their own lines.
//!
//! The menu of a right click on a thing in the inventory
//! (`docs/spec/inventory.md`, "The menu of a right click"): what it holds
//! while it is open, its drawing (as the game's with `TARKOV UI`, else the
//! app's way), the line under the mouse, and the menu of a line's own
//! lines (the wish list's: "The wish list"), opened and shut by the mouse.
//! Opening it, doing a line and shutting it are the inventory's
//! (`inventory_view::drag`), where what a line does is.

use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;
use trp_sim::items::inventory::Id;
use trp_sim::items::item_menu::{Can, Line, WishLine};

use crate::screens::health_view::{HealthLook, MenuLineLook, Words};
use crate::screens::inventory_view::Carried;

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
    /// Where the menu is a window's button's (`InteractionButtonsContainer`
    /// of the window that shows a thing): the button, which stands for its
    /// one line, and the menu of lines alone is drawn.
    pub button: Option<WindowButton>,
}

/// A window's button whose menu of lines is open: its box in the window's
/// pixels (left, top, width, height) and its ground's piece, kept lit while
/// its menu of lines is open (`SimpleContextMenuButton.Blocked`).
#[derive(Clone, Copy, PartialEq)]
pub struct WindowButton {
    pub rect: [f32; 4],
    pub piece: Option<usize>,
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
        // Drawn after the screen's systems have opened or shut it, so that
        // it shows on the frame its button is let go (`courses10`'s
        // `inspect-window`, frame 1722) and goes on Escape's. The line
        // under the mouse lit before the clicks are heard (before the
        // controller's step: `ClicksHeard`).
        use crate::screens::inventory_view::{ClicksHeard, ScreenDrawn};
        app.init_resource::<ItemMenu>().add_systems(Update, (hover.before(ClicksHeard).before(ScreenDrawn), draw.after(ScreenDrawn)));
    }
}

/// The line under the mouse; lit as it comes onto it (`OnPointerEnter`).
/// A line the mouse comes onto shuts the menu of lines and opens its own,
/// where it has lines it can open (`CreateInternalContextButton`'s hover:
/// the wish list's for the thing's kind); the mouse out of both menus and
/// their shadows shuts it (`OnPointerExit`, unless into the other). A line
/// that cannot be done says why in the simple tooltip while the mouse is
/// on it (`SimpleContextMenuButton.OnPointerEnter`: `SimpleTooltip.
/// Show(_lockReason)`, no delay, by the mouse; `OnPointerExit` closes it).
fn hover(
    mut menu: ResMut<ItemMenu>,
    hits: Query<(&MenuHit, &RelativeCursorPosition)>,
    sub_hits: Query<(&SubHit, &RelativeCursorPosition)>,
    areas: Query<(&MenuArea, &RelativeCursorPosition)>,
    carried: Res<Carried>,
    said: Res<Words>,
    mut tooltip: ResMut<crate::ui::tooltip::SimpleTip>,
) {
    let Some(open) = menu.0.as_ref() else { return };
    let in_area = |sub: bool| areas.iter().any(|(area, at)| area.0 == sub && at.cursor_over());
    // The menu of lines is over the first, its shadow over the left ends
    // of the first's lines: a line there is not under the mouse
    // (`courses10`'s `inspect-window`, frame 432).
    let over = hits.iter().find(|(_, at)| at.cursor_over()).map(|(hit, _)| hit.0).filter(|_| !in_area(true));
    let over_sub = sub_hits.iter().find(|(_, at)| at.cursor_over()).map(|(hit, _)| hit.0);
    let mut sub = open.sub.clone();
    if over != open.hover {
        if let Some(index) = over {
            let template = carried.find(open.id).map(|item| item.kind.template.clone());
            sub = open.lines.get(index).filter(|(line, can, _)| line.has_lines() && can.is_ok()).zip(template).map(|(_, template)| SubMenu {
                parent: index,
                lines: trp_sim::items::item_menu::wish_lines(&template, &carried.wishlist)
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
    // A window's button's menu goes with its menu of lines: the mouse off
    // the button and its menu of lines (`InteractionButtonsContainer.
    // OnPointerExit`, `CloseSubMenu`): `courses10`'s `weapon-inspect-menu`,
    // frame 1179.
    if open.button.is_some() && sub.is_none() {
        menu.0 = None;
        return;
    }
    if let Some(sub) = sub.as_mut() {
        sub.hover = over_sub;
    }
    let reason = match &sub {
        Some(sub) if sub.hover.is_some() => sub.hover.and_then(|index| Some((("menu sub line", index as u64), sub.lines.get(index)?.1.err()?))),
        _ => over.and_then(|index| Some((("menu line", index as u64), open.lines.get(index)?.1.err()?))),
    };
    if let Some((key, reason)) = reason.filter(|(_, reason)| !reason.is_empty()) {
        tooltip.show(key, said.word(reason, reason), 0.0);
    }
    if over != open.hover || sub != open.sub {
        if let Some(open) = menu.0.as_mut() {
            open.hover = over;
            open.sub = sub;
        }
    }
}

/// Where a menu goes from the top left it is shown at, as `CorrectPosition`
/// puts it (`RectTransformExtensions.CorrectPositionResolution`): moved
/// back onto the screen from the edge it crosses (the left's checked
/// before the right's, the bottom's before the top's), then its top left
/// put on a whole pixel (`Math.Round`, to the even one at a half). In the
/// window's pixels, y down.
fn correct(corner: Vec2, size: Vec2, screen: Vec2) -> Vec2 {
    // Its left and top, y up.
    let (mut x, mut top) = (corner.x, screen.y - corner.y);
    if x < 0.0 {
        x = 0.0;
    } else if x + size.x > screen.x {
        x = screen.x - size.x;
    }
    if top - size.y < 0.0 {
        top = size.y;
    } else if top > screen.y {
        top = screen.y;
    }
    Vec2::new(x.round_ties_even(), screen.y - top.round_ties_even())
}

/// Where a menu of lines goes (`InteractionButtonsContainer.
/// SetSubInteractions`): shown with its top left at its line's top right
/// and put as `correct` puts it; where that moved it sideways at all
/// (`IsZero`: less than `float.Epsilon`), at the line's left less its
/// width instead, that not rounded, at the height it was put at. A line's
/// right edge is seldom on a whole pixel (the lines are 5 units in and
/// 160.17 or so wide), so it is nearly always on the left: `courses10`'s
/// `inspect-window`, four times of four with room on the right. In the
/// window's pixels.
fn sub_corner(line_right_top: Vec2, line_left: f32, size: Vec2, screen: Vec2) -> Vec2 {
    let put = correct(line_right_top, size, screen);
    if put.x != line_right_top.x {
        Vec2::new(line_left - size.x, put.y)
    } else {
        put
    }
}

/// What of a menu takes the mouse (its rectangle and its shadow), from
/// its top left, in units: a place of the menu's for its hover, and over
/// whatever lies under it (`inventory_view::Cover`), as the game's menu
/// and its shadow are what the mouse is on (`courses10`'s
/// `inspect-window`: from frame 205 the thing under the menu not lit).
fn catch(layer: &mut ChildSpawnerCommands, [x, y, w, h]: [f32; 4], origin: Vec2, s: f32, sub: bool) {
    let node = at(origin.x + x * s, origin.y + y * s, w * s, h * s);
    layer.spawn((MenuArea(sub), crate::screens::inventory_view::Cover(crate::screens::inventory_view::MENU_LAYER), RelativeCursorPosition::default(), node));
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
    let cover = crate::screens::inventory_view::Cover(crate::screens::inventory_view::MENU_LAYER);
    layer.spawn((MenuArea(sub), cover, RelativeCursorPosition::default(), column, BackgroundColor(Color::BLACK.with_alpha(0.85)), BorderColor::all(FRAME))).with_children(|column| {
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
    let built = look.tarkov_ui.then(|| crate::screens::health_view::menu_layout(&look, &looks)).flatten();
    let sub_built = (look.tarkov_ui && !sub_looks.is_empty()).then(|| crate::screens::health_view::submenu_layout(&look, &sub_looks)).flatten();
    // (Over every screen, the edit build screen's too, at 40.)
    commands.spawn((MenuRoot, root, GlobalZIndex(55))).with_children(|layer| {
        // A window's button's: the button stands for the first menu, its
        // place the first's for the mouse (not over the window: the mouse is
        // on the button), and its menu of lines is shown with its top left
        // at the button's top right, as a line's is (`SetSubInteractions`,
        // `sub_corner`): `courses10`'s `weapon-inspect-menu`, frame 1084, on
        // the button's left, its top at the button's.
        if let Some(button) = open.button {
            let [left, top, w, h] = button.rect;
            layer.spawn((MenuArea(false), RelativeCursorPosition::default(), at(left, top, w, h), bevy::ui::FocusPolicy::Pass));
            let Some(sub) = open.sub.as_ref() else { return };
            match sub_built {
                Some(sub_built) => {
                    let sub_size = Vec2::new(sub_built.size[0], sub_built.size[1]) * s;
                    let sub_at = sub_corner(Vec2::new(left + w, top), left, sub_size, screen);
                    let sub_origin = [sub_at.x / s, sub_at.y / s];
                    crate::screens::health_view::draw_menu(layer, &look, &said, &sub_built, sub_origin, s, true);
                    catch(layer, sub_built.catch(), sub_at, s, true);
                    for (index, [x, y, w, h]) in sub_built.lines.iter().enumerate() {
                        layer.spawn((SubHit(index), at((sub_origin[0] + x) * s, (sub_origin[1] + y) * s, w * s, h * s), RelativeCursorPosition::default()));
                    }
                }
                None => {
                    let sub_size = app_menu_size(sub.lines.len(), s);
                    let sub_at = sub_corner(Vec2::new(left + w, top), left, sub_size, screen);
                    app_menu(layer, &sub_looks, sub_at, s, true);
                }
            }
            return;
        }
        match built {
            // The game's: laid out by its layout, its top left at the
            // mouse, put as `CorrectPosition` puts it (`SimpleContextMenu.
            // ShowMenu`).
            Some(built) => {
                let size = Vec2::new(built.size[0], built.size[1]) * s;
                let corner = correct(open.cursor, size, screen);
                let origin = [corner.x / s, corner.y / s];
                crate::screens::health_view::draw_menu(layer, &look, &said, &built, origin, s, false);
                catch(layer, built.catch(), corner, s, false);
                for (index, [x, y, w, h]) in built.lines.iter().enumerate() {
                    layer.spawn((MenuHit(index), at((origin[0] + x) * s, (origin[1] + y) * s, w * s, h * s), RelativeCursorPosition::default()));
                }
                // Its menu of lines, by the line it is of.
                if let (Some(sub), Some(sub_built)) = (open.sub.as_ref(), sub_built) {
                    if let Some([x, y, w, _]) = built.lines.get(sub.parent) {
                        let sub_size = Vec2::new(sub_built.size[0], sub_built.size[1]) * s;
                        let (left, top) = ((origin[0] + x) * s, (origin[1] + y) * s);
                        let sub_at = sub_corner(Vec2::new(left + w * s, top), left, sub_size, screen);
                        let sub_origin = [sub_at.x / s, sub_at.y / s];
                        crate::screens::health_view::draw_menu(layer, &look, &said, &sub_built, sub_origin, s, true);
                        catch(layer, sub_built.catch(), sub_at, s, true);
                        for (index, [x, y, w, h]) in sub_built.lines.iter().enumerate() {
                            layer.spawn((SubHit(index), at((sub_origin[0] + x) * s, (sub_origin[1] + y) * s, w * s, h * s), RelativeCursorPosition::default()));
                        }
                    }
                }
            }
            // The app's: the same lines in its own colours.
            None => {
                let size = app_menu_size(looks.len(), s);
                let corner = correct(open.cursor, size, screen);
                app_menu(layer, &looks, corner, s, false);
                let row = |index: usize| corner.y + 3.0 * s + index as f32 * 22.0 * s;
                if let Some(sub) = open.sub.as_ref() {
                    let sub_size = app_menu_size(sub_looks.len(), s);
                    let sub_at = sub_corner(Vec2::new(corner.x + size.x, row(sub.parent)), corner.x, sub_size, screen);
                    app_menu(layer, &sub_looks, sub_at, s, true);
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A menu of lines at its line's top right where that is on a whole
    /// pixel, kept on the screen; moved sideways by that, at the line's
    /// left at the height it was kept to.
    #[test]
    fn a_menu_of_lines_goes_right_of_its_line_or_left() {
        let screen = Vec2::new(1920.0, 1080.0);
        let size = Vec2::new(150.0, 120.0);
        assert_eq!(sub_corner(Vec2::new(800.0, 300.0), 650.0, size, screen), Vec2::new(800.0, 300.0));
        assert_eq!(sub_corner(Vec2::new(800.0, 1000.0), 650.0, size, screen), Vec2::new(800.0, 960.0));
        assert_eq!(sub_corner(Vec2::new(1850.0, 1000.0), 1700.0, size, screen), Vec2::new(1550.0, 960.0));
    }

    /// `courses10`'s `inspect-window`, frame 2163: the M4A1's menu, let go
    /// at 368,743 (2560 by 1440, 4/3 pixels a unit), its wish list's line
    /// from 374.67 to 588.23 across, its top at 883. Its menu of lines,
    /// shown at the line's top right, is put on a whole pixel, 0.23 to the
    /// left, so it goes on the left: at 374.67 less its width (260.21: its
    /// ground then at 117.12, the game's), at 883. A line's right on a
    /// whole pixel would keep it on the right.
    #[test]
    fn a_menu_of_lines_goes_left_where_its_corner_is_rounded() {
        let (screen, s) = (Vec2::new(2560.0, 1440.0), 1440.0 / 1080.0);
        let left = 368.0 + 5.0 * s;
        let top = 743.0 + 5.0 * s + 5.0 * 20.0 * s;
        let size = Vec2::new(260.21, 176.0);
        let at = sub_corner(Vec2::new(left + 160.17 * s, top), left, size, screen);
        assert!((at - Vec2::new(left - 260.21, 883.0)).length() < 0.01, "{at}");
        assert_eq!(sub_corner(Vec2::new(588.0, 883.0), left, size, screen), Vec2::new(588.0, 883.0));
    }

    /// `courses10`'s `weapon-inspect-menu`, frame 1084: the window's ADD TO
    /// WISHLIST button at 1468, 576, 233.33 wide (its right off a whole
    /// pixel); its menu of lines (260.21 wide) at its left, 1207.79 (its
    /// lines from 1211.79, the game's), its top the button's.
    #[test]
    fn a_window_button_s_menu_of_lines_goes_on_its_left_at_its_top() {
        let screen = Vec2::new(2560.0, 1440.0);
        let at = sub_corner(Vec2::new(1468.0 + 233.33, 576.0), 1468.0, Vec2::new(260.21, 170.66), screen);
        assert!((at - Vec2::new(1207.79, 576.0)).length() < 0.01, "{at}");
    }

    /// A menu kept on the screen has its top left put on a whole pixel:
    /// let go 30 pixels above the screen's foot, a menu 197.34 high is put
    /// with its top at 1243 (1242.66 unrounded).
    #[test]
    fn a_menu_kept_on_the_screen_is_on_a_whole_pixel() {
        let screen = Vec2::new(2560.0, 1440.0);
        assert_eq!(correct(Vec2::new(368.0, 1410.0), Vec2::new(216.23, 197.34), screen), Vec2::new(368.0, 1243.0));
        assert_eq!(correct(Vec2::new(368.0, 743.0), Vec2::new(216.23, 197.34), screen), Vec2::new(368.0, 743.0));
    }
}
