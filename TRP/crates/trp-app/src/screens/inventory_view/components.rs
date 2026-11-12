//! The screen's components, drop marks, the held thing and click state.

use super::*;

#[derive(Component)]
pub(super) struct Screen;

#[derive(Component)]
pub(super) struct TabButton(pub(super) usize);

#[derive(Component)]
pub(super) struct Page(pub(super) usize);

#[derive(Component)]
pub(super) struct GearPage;

/// The game's bar of tabs (`TARKOV UI`): the node it is drawn into, as
/// large as the window; what of it is drawn afresh as the tabs change; and
/// a tab's button over it.
#[derive(Component)]
pub(super) struct GameTabBar;

#[derive(Component)]
pub(super) struct GameTabLook;

#[derive(Component)]
pub(super) struct GameTab;

/// The game's back button over the bar (`TARKOV UI`): a click shuts the
/// screen, as the game's does.
#[derive(Component)]
pub(super) struct GameBack;

/// An item as drawn, by its number, and what it is drawn on: the page
/// (0) or a window over it (its place among the windows, from 1).
#[derive(Component)]
pub(super) struct ItemView(pub(super) Id, pub(super) u8);

/// The value of an armour holder with plates in it, which shows its points
/// as the mouse comes onto it (`GridItemView.ValuePointerEventsProxy`): the
/// item, and what it is drawn on (as `ItemView`'s).
#[derive(Component)]
pub(super) struct ValueProxy(pub(super) Id, pub(super) u8);

/// A slot of an item as drawn.
#[derive(Component)]
pub(super) struct SlotBox {
    pub(super) holder: Id,
    pub(super) slot: usize,
    pub(super) layer: u8,
    /// Its own border's colour, which a held thing over it tints.
    pub(super) border: Color,
    /// The game's slot view under it has a border line a tint recolours
    /// (`AcceptLine`), which is tinted in place of its own border.
    pub(super) lined: bool,
}

/// The line of the game's border of a slot's view (`TARKOV UI`), drawn over
/// it in a tint while one is wanted, hidden otherwise: the view's empty
/// border (`SlotView._emptyBorder`, the sprite `border_generic`, a line of
/// one pixel round its rectangle, `PixelPerfectSpriteScaler`), which the
/// game recolours (`HighlightItemViewPosition`). The same line in another
/// colour over it is the game's recoloured line, pixel for pixel.
#[derive(Component)]
pub(super) struct AcceptLine {
    pub(super) holder: Id,
    pub(super) slot: usize,
}

/// How a slot's box is drawn: with its own border and ground (the app's
/// look), or over the game's own slot view, which draws the slot
/// (`TARKOV UI`); then where that view's border line lies as the slot
/// stands, in units outside the box, if it has one (`AcceptLine`).
#[derive(Clone, Copy)]
pub(super) enum SlotLook {
    Own,
    Game(Option<f32>),
}

/// Where an empty equipment slot's, a container's row's or a special slot's
/// border line lies (their `Empty Border`: 127 units square about the
/// view's 125, 66 about the dogtag's 64; a special slot's a unit outside
/// too); a part's slot's in the inspect window lies on its own edge, full
/// or empty (`ModSlotView`'s `Empty Border` and `Full Border`, both
/// `border_generic` on the view's 64 square). Read from the install
/// (`probe scenepanel`, `probe infowindow`).
pub(super) const EMPTY_LINE_OUT: f32 = 1.0;

pub(super) const PART_LINE_OUT: f32 = 0.0;

/// A grid of an item as drawn.
#[derive(Component)]
pub(super) struct GridBox {
    pub(super) holder: Id,
    pub(super) grid: usize,
    pub(super) cells: [u32; 2],
    pub(super) layer: u8,
}

/// Something drawn over the page that the mouse does not reach through:
/// the quick-use bar, a window (above the bar, as it is drawn: its place
/// among the windows from `WINDOW_LAYER`), the window that asks how many,
/// the menu of a right click (over every window; never up with the window
/// that asks).
/// The mouse goes to the highest under it.
#[derive(Component)]
pub(crate) struct Cover(pub(crate) u8);

pub(super) const BAR_LAYER: u8 = 200;

pub(super) const WINDOW_LAYER: u8 = BAR_LAYER + 1;

pub(super) const ASKING_LAYER: u8 = 255;

pub(crate) const MENU_LAYER: u8 = 255;

/// Everything drawn over the page, made afresh with it.
#[derive(Component)]
pub(super) struct Overlay;

/// A window's caption, by which it is moved, and its root.
#[derive(Component)]
pub(super) struct WinCaption(pub(super) usize);

#[derive(Component)]
pub(crate) struct WinRoot(pub(super) usize);

#[derive(Component)]
pub(super) struct WinClose(pub(super) usize);

/// The parts of the window that asks how many.
#[derive(Component)]
pub(super) struct AskBar;

/// The window that asks how many as the game draws it (`TARKOV UI`): where
/// its parts lie, on its root.
#[derive(Component)]
pub(super) struct AskPlaces(pub(super) crate::screens::split_view::SplitPlaces);

/// What a press on that window went to: its cross, its button, its
/// slider (held where the press was on the handle, from its middle, or
/// from the middle where the press was on the track), or its caption (the
/// press's place, and once the window moves, the mouse's from the window's
/// middle).
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum AskPress {
    Close,
    Accept,
    Slider(f32),
    Caption(Vec2, Option<Vec2>),
}

#[derive(Component)]
pub(super) struct AskAccept;

#[derive(Component)]
pub(super) struct AskClose;

/// The item being moved, as it follows the mouse.
#[derive(Component)]
pub(super) struct Ghost;

#[derive(Component)]
pub(super) struct GhostWords;

/// The held thing's own picture in it (`DraggedItemView._mainImage`).
#[derive(Component)]
pub(super) struct GhostPicture;

/// The cells of a grid the held thing would take, in the colour of what a
/// drop there would do (`GridView._highlightPanel`).
#[derive(Component)]
pub(super) struct DropMark;

#[derive(Component)]
pub(crate) struct Prompt;

/// A line of the menu for what is looked at.
#[derive(Component)]
pub(super) struct PromptLine;

/// The tab that is shown.
#[derive(Resource)]
pub struct Selected(pub usize);

/// An item held by the mouse: which, whether it has been turned, where in
/// it the mouse took hold (in pixels from its top left), and its name; the
/// layer its view was taken from; its picture's lower left corner as the
/// mouse last moved it (pixels, y down) and the mouse then.
pub(super) struct Held {
    pub(super) id: Id,
    pub(super) turned: bool,
    pub(super) grab: Vec2,
    pub(super) cells: [u32; 2],
    pub(super) name: String,
    pub(super) layer: u8,
    pub(super) corner: Vec2,
    pub(super) moved: Option<Vec2>,
}

/// The cell a thing held would go to in a grid (`GridView.
/// CalculateItemLocation`): from its picture's lower left corner as last
/// kept (`corner`, pixels, y down) and the grid's top left (`origin`), its
/// left over a cell, and its bottom over a cell less its height as it is
/// now turned (`high`), each rounded (`Mathf.RoundToInt`: a half to the
/// even one) and kept between 0 and the grid's width or height.
pub(super) fn grid_location(corner: Vec2, origin: Vec2, cell: f32, high: u32, cells: [u32; 2]) -> (u32, u32) {
    let x = ((corner.x - origin.x) / cell).round_ties_even().clamp(0.0, cells[0] as f32);
    let y = ((corner.y - origin.y) / cell - high as f32).round_ties_even().clamp(0.0, cells[1] as f32);
    (x as u32, y as u32)
}

#[derive(Resource, Default)]
pub(super) struct Drag(pub(super) Option<Held>);

/// What a drop where the mouse is would mark this frame (`drag`), drawn by
/// `show_drop`: the cells of a grid it would take (left, top, width,
/// height in pixels) and the slot under it, each in the colour of what the
/// drop would do.
#[derive(Resource, Default, PartialEq)]
pub(super) struct DropShown {
    pub(super) cells: Option<([f32; 4], Color)>,
    pub(super) slot: Option<(Id, usize, Color)>,
}

/// What a drop would do, in the game's colours for it (`GridView.
/// GetHighlightColor`: a move green, a merge, split or hand-over yellow,
/// anything else red; each at 0.57).
pub(super) fn drop_colour(would: Option<&Result<&'static str, Refusal>>) -> Color {
    match would {
        Some(Ok("moved" | "put in")) => Color::srgba(0.06, 0.38, 0.06, 0.57),
        Some(Ok(_)) => Color::srgba(0.69, 0.66, 0.0, 0.57),
        _ => Color::srgba(0.68, 0.0, 0.0, 0.57),
    }
}

/// A slot's own colours for its borders (`SlotView`: `_validMoveColor`,
/// the engine's `Color.green`; `GetHighlightColor` of a refused move,
/// `_invalidOperationColor`, `Color.red`): what it would take, green, by
/// whatever the drop would do; what it would not, red
/// (`HighlightItemViewPosition`: a move it can make is green whatever it
/// is, merge or move).
const SLOT_TAKES: Color = Color::srgb(0.0, 1.0, 0.0);

const SLOT_REFUSES: Color = Color::srgb(1.0, 0.0, 0.0);

pub(super) fn slot_colour(would: Option<&Result<&'static str, Refusal>>) -> Color {
    match would {
        Some(Ok(_)) => SLOT_TAKES,
        _ => SLOT_REFUSES,
    }
}

/// The empty slots drawn that would take the thing carried, lit green
/// while it is (`show_drop`).
#[derive(Resource, Default, PartialEq)]
pub(super) struct Accepting {
    pub(super) held: Option<Id>,
    pub(super) slots: Vec<(Id, usize)>,
}

/// While a thing is carried, every empty slot drawn that a drop would put
/// it in has its borders green (`ItemUiContext.CheckCompatibleItems`, to
/// every view; `SlotView.CheckAcceptHandler`, an empty slot's alone;
/// `HighlightItemViewPosition` with `preview`: lit only where it would go,
/// `DragItemContext.CanAccept`, which is the drop's own try, with Ctrl a
/// part of a stack). The game asks as the thing is taken, as a view is
/// made while it is held, and as Ctrl, Shift or Alt goes down or up
/// (`ItemUiContext.Update`), and not otherwise; so here. A slot that would
/// not take it (another kind, one a thing worn blocks or that would block
/// one in use, a part not to be changed in a raid) is not lit. Not built:
/// the game lights a slot refused only because the character is busy with
/// a thing (`PlayerIsBusyError`), which the app's moves have no refusal
/// for.
pub(super) fn check_accept(drag: Res<Drag>, carried: Res<Carried>, keys: Res<ButtonInput<KeyCode>>, boxes: Query<&SlotBox>, made: Query<(), Added<SlotBox>>, mut accepting: ResMut<Accepting>) {
    const MODIFIERS: [KeyCode; 6] = [KeyCode::ControlLeft, KeyCode::ControlRight, KeyCode::ShiftLeft, KeyCode::ShiftRight, KeyCode::AltLeft, KeyCode::AltRight];
    let held = drag.0.as_ref().map(|held| held.id);
    let asked_again = held.is_some() && (!made.is_empty() || keys.any_just_pressed(MODIFIERS) || keys.any_just_released(MODIFIERS));
    if held == accepting.held && !asked_again {
        return;
    }
    let part = keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]);
    let drawn = boxes.iter().filter(|slot| slot.layer != OUT_OF_REACH).map(|slot| (slot.holder, slot.slot));
    let slots = held.map_or_else(Vec::new, |id| accepting_slots(&carried, id, drawn, part));
    accepting.set_if_neq(Accepting { held, slots });
}

/// Of the slots drawn, the empty ones a drop of the thing would go into,
/// each tried on copies (`act`), as the drop itself is (`drag`).
pub(super) fn accepting_slots(carried: &Carried, id: Id, drawn: impl Iterator<Item = (Id, usize)>, part: bool) -> Vec<(Id, usize)> {
    let copies = || (carried.equipment.item.clone(), carried.loot.clone());
    let (mut worn, mut loot) = copies();
    let mut lit = Vec::new();
    for (holder, slot) in drawn {
        let empty = carried.find(holder).is_some_and(|holder| holder.slots.get(slot).is_some_and(Option::is_none));
        if !empty || lit.contains(&(holder, slot)) {
            continue;
        }
        if act(&mut [&mut worn, &mut loot], id, Target::Slot(holder, slot), &carried.rules, part.then_some(1)).is_ok() {
            lit.push((holder, slot));
            // (The copies took it: fresh ones for the next.)
            (worn, loot) = copies();
        }
    }
    lit
}

/// Draws what `drag` says a drop would mark: the grid's cells over the
/// grid, under the held picture; the slot under it in the slot's own
/// colours (`SlotView.HighlightItemViewPosition`), the empty slots that
/// would take the thing green (`check_accept`), every other slot's border
/// its own. With `TARKOV UI` the tint is on the game's view's border line
/// where it has one (`AcceptLine`); on a full equipment or container slot,
/// whose full border is the game's shaded frame (`slot_border_full`, six
/// units out), the box's own line of a pixel stands for it (`PROVISIONAL`).
#[allow(clippy::type_complexity)]
pub(super) fn show_drop(
    (shown, accepting): (Res<DropShown>, Res<Accepting>),
    mut mark: Query<(&mut Node, &mut Visibility, &mut BackgroundColor), (With<DropMark>, Without<AcceptLine>)>,
    mut slots: Query<(&SlotBox, &mut BorderColor), Without<AcceptLine>>,
    mut lines: Query<(&AcceptLine, &mut BorderColor, &mut Visibility), Without<DropMark>>,
) {
    let tint = |holder: Id, at: usize| match shown.slot {
        Some((over, over_at, colour)) if over == holder && over_at == at => Some(colour),
        _ => accepting.slots.contains(&(holder, at)).then_some(SLOT_TAKES),
    };
    for (line, mut border, mut visibility) in &mut lines {
        let want = tint(line.holder, line.slot);
        let seen = if want.is_some() { Visibility::Inherited } else { Visibility::Hidden };
        if *visibility != seen {
            *visibility = seen;
        }
        if let Some(colour) = want.filter(|colour| *border != BorderColor::all(*colour)) {
            *border = BorderColor::all(colour);
        }
    }
    if let Ok((mut node, mut visibility, mut ground)) = mark.single_mut() {
        match shown.cells {
            Some(([x, y, w, h], colour)) => {
                let place = (Val::Px(x), Val::Px(y), Val::Px(w), Val::Px(h));
                if (node.left, node.top, node.width, node.height) != place {
                    (node.left, node.top, node.width, node.height) = place;
                }
                if ground.0 != colour {
                    ground.0 = colour;
                }
                if *visibility != Visibility::Inherited {
                    *visibility = Visibility::Inherited;
                }
            }
            None => {
                if *visibility != Visibility::Hidden {
                    *visibility = Visibility::Hidden;
                }
            }
        }
    }
    for (slot, mut border) in &mut slots {
        let want = if slot.lined { slot.border } else { tint(slot.holder, slot.slot).unwrap_or(slot.border) };
        if *border != BorderColor::all(want) {
            *border = BorderColor::all(want);
        }
    }
}

/// The mouse's button down on an item and not yet let go or moved far
/// enough to carry it: a click in the making. Which item, where the mouse
/// was, where in the item it took hold (as a share of it), and when.
#[derive(Clone, Copy)]
pub(super) struct Pressed {
    pub(super) id: Id,
    pub(super) cursor: Vec2,
    pub(super) at: Vec2,
}

/// The click before this one, for telling a double click.
#[derive(Clone, Copy)]
pub(super) struct Clicked {
    pub(super) id: Id,
    pub(super) at: f64,
    pub(super) count: u32,
}

#[derive(Resource, Default)]
pub(super) struct Clicks {
    pub(super) pressed: Option<Pressed>,
    pub(super) last: Option<Clicked>,
    /// A window being moved by its caption: which, and where in it the
    /// mouse holds it.
    pub(super) moving: Option<(usize, Vec2)>,
    /// A press on a window (`Window`'s pointer handlers): which, where the
    /// mouse went down, whether on its caption's bar, whether a thing in it
    /// took the press (an `ItemView` handles its own click and drag), and
    /// whether a drag of it has begun.
    pub(super) window_press: Option<WindowPress>,
    /// Whether the slider of the window that asks how many is held.
    pub(super) sliding: bool,
    /// A press on the window that asks how many as the game draws it, and
    /// what it went to (`ask`).
    pub(super) ask_press: Option<AskPress>,
    /// A window that shows a thing being stretched: which, by which sides
    /// (`StretchArea.EStretchAreaType`), the mouse's last place as the
    /// stretch has taken it (`_lastPosition`: the press until the drag has
    /// begun), and whether it has.
    pub(super) stretching: Option<(usize, u8, Vec2, bool)>,
    /// The button of a window that shows a thing the mouse was on, on top
    /// there: its thing and line.
    pub(super) info_hover: Option<(Id, trp_sim::items::item_menu::Line)>,
    /// Escape taken this frame by a window over the screen or the window
    /// that asks how many, heard before the step (`windows`, `ask`), so
    /// that the screen, which hears it after (`toggle`), keeps it from
    /// itself and the menu of a right click as it did when it heard first.
    pub(super) escape_taken: bool,
}

#[derive(Clone, Copy)]
pub(super) struct WindowPress {
    pub(super) index: usize,
    pub(super) from: Vec2,
    pub(super) by_caption: bool,
    pub(super) thing: bool,
    pub(super) dragged: bool,
}

/// How far the mouse has to move with the button down before a drag
/// begins, in the screen's pixels: the game's event system's
/// (`EventSystem.m_DragThreshold`, 5 on `Application (Main Client)/
/// EventSystem`, `probe fields level210 "Application (Main Client)/
/// EventSystem"`; the engine's default is 10), reached at it or past it
/// (`PointerInputModule.ShouldStartDrag`: the squared distance at least
/// its square). `courses10`'s `drag-and-drop`: 8 drags of 8 began on the
/// first frame 5 pixels or more from the press.
pub(crate) const DRAG_THRESHOLD: f32 = 5.0;

/// Two clicks on one thing within this many seconds are a double click:
/// the engine's own figure for its screens (`StandaloneInputModule`, in
/// its published source; a third click starts the count again at odd).
pub(super) const DOUBLE_CLICK: f64 = 0.3;

/// Where an item held by the mouse would go.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Target {
    Item(Id),
    Cell(Id, usize, Place),
    Slot(Id, usize),
}
