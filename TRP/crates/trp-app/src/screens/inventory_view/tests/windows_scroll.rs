//! Tests of windows grown, kept and stretched, the loot's rows, cells and scroll bars.

use super::*;

/// `courses10`'s `backpack-search`, frame 562: the Terraframe's window,
/// its search over, grown from 545.34 by 206.66 to 534.67 by 464 about
/// its middle, its corner from 1007.33,616.67 to 1013,488 (1012.665
/// rounded); one grown past the screen's foot is moved up onto it.
#[test]
fn a_window_grown_keeps_its_middle_at_whole_pixels() {
    let room = Vec2::new(2560.0, 1440.0);
    let at = regrown(Vec2::new(1007.33, 616.67), Vec2::new(545.34, 206.66), Vec2::new(534.67, 464.0), room);
    assert_eq!(at, Vec2::new(1013.0, 488.0));
    assert_eq!(regrown(Vec2::new(1000.0, 1200.0), Vec2::new(400.0, 200.0), Vec2::new(400.0, 600.0), room), Vec2::new(1000.0, 840.0));
}

/// The loot panel's first grid has views for the rows its scroll view
/// shows, rounded to whole cells as the game's (`GridViewMagnifier`):
/// a view 600 units high scrolled 100 down shows rows 1 to 10 (its
/// foot at 11.1 cells, its height 9.5 rounded to 10), so a thing in
/// row 0, more than half out, has none; scrolled 20 down (a third of a
/// cell), row 0 is still in.
#[test]
fn the_loot_s_rows_in_sight_are_rounded_to_cells() {
    assert_eq!(magnified_rows(700.0, 600.0, 40), [1, 11]);
    assert_eq!(magnified_rows(620.0, 600.0, 40), [0, 10]);
    assert_eq!(magnified_rows(600.0, 600.0, 5), [0, 5]);
    assert_eq!(magnified_rows(2600.0, 600.0, 40), [31, 40]);
}

/// The things in a grid lie on the game's grid view's own lines at
/// every row, in a window whose unit is no whole share of a pixel (2560
/// by 1366 inside): a cell is 63 units at the
/// screen's scale, a fraction included, as the view tiles them, so the
/// fiftieth row's line is where the view's tile puts it.
#[test]
fn a_cell_is_the_grid_view_s_tile() {
    let units = Units(1366.0 / SCREEN_HEIGHT);
    let tile = CELL * units.0;
    assert_eq!(units.cell(), tile);
    let row = 50.0;
    assert!((row * units.cell() - row * 63.0 * units.0).abs() < 1e-3, "{} against {}", row * units.cell(), row * 63.0 * units.0);
}

pub(super) fn holder(name: &str, cells: [u32; 2], takes: &str) -> Holder {
    Holder { name: name.into(), cells, takes: vec![takes.into()], ..Holder::default() }
}

/// A thing's picture in a quick-use view as the game sizes it before
/// turning it: a square one by its turned height alone (so that its
/// corners reach the view's), a long one by its turned length.
#[test]
fn a_quick_use_picture_is_fitted_as_the_game_s() {
    let square = quick_fit([1, 1], [64.0, 64.0]);
    assert!((square.x - 64.0 / std::f32::consts::SQRT_2).abs() < 1e-3, "{square}");
    // A rifle four cells by two: (252, 126) turned is (89.1, 267.3);
    // its length bounds it, 64 / 267.3.
    let rifle = quick_fit([4, 2], [64.0, 64.0]);
    assert!((rifle.x - 252.0 * 64.0 / (378.0 / std::f32::consts::SQRT_2)).abs() < 1e-2, "{rifle}");
    assert!((rifle.x / rifle.y - 2.0).abs() < 1e-4);
}

/// Unity's `SmoothDamp`, which an elastic scroll view springs back by:
/// with no time (its least, a ten-thousandth of a second) it lands within
/// a hundredth of a unit on the first frame; with some it comes most of
/// the way in that time and never passes the target.
#[test]
fn an_elastic_view_springs_back_as_unity_s() {
    let mut speed = 0.0;
    assert!(smooth_damp(-63.0, 0.0, &mut speed, 0.0, 1.0 / 120.0).abs() < 0.01);
    let (mut at, mut speed) = (-63.0f32, 0.0);
    for _ in 0..12 {
        at = smooth_damp(at, 0.0, &mut speed, 0.1, 1.0 / 120.0);
        assert!(at <= 0.0);
    }
    assert!(at > -40.0 && at < -5.0, "{at}");
    for _ in 0..240 {
        at = smooth_damp(at, 0.0, &mut speed, 0.1, 1.0 / 120.0);
    }
    assert!(at.abs() < 0.01, "{at}");
}

/// The containers' column's bar as the game's `Scrollbar`: a handle
/// dragged follows the mouse over its share of the sliding area, from
/// where it was taken; the bar held below its handle steps down a
/// handle's share at a time, to four places, until the handle is under
/// the press.
#[test]
fn the_column_s_bar_is_worked_as_the_game_s() {
    // A sliding area 10 down and 100 high, the handle half of it.
    let sliding = [10.0, 100.0];
    // Taken by its middle at the top (value 1: its middle 35 down).
    assert_eq!(dragged_to(sliding, 0.5, 0.0, 35.0, true), Some(1.0));
    // Moved 25 down of the 50 it can go: half way.
    assert_eq!(dragged_to(sliding, 0.5, 0.0, 60.0, true), Some(0.5));
    // Taken 5 above its middle, the same place of the mouse is 5 less.
    assert_eq!(dragged_to(sliding, 0.5, 5.0, 60.0, true), Some(0.4));
    // Past the ends, at the ends; a handle that fills the area, none.
    assert_eq!(dragged_to(sliding, 0.5, 0.0, 500.0, true), Some(0.0));
    assert_eq!(dragged_to(sliding, 1.0, 0.0, 50.0, true), None);
    // The handle at the top (2 to 62 down) and the bar pressed at 90.
    let handle = [0.0, 2.0, 6.0, 60.0];
    assert_eq!(stepped(1.0, 0.3, handle, [3.0, 90.0], true), Some(0.7));
    assert_eq!(stepped(0.2, 0.3, handle, [3.0, 90.0], true), Some(0.0));
    assert_eq!(stepped(0.123456, 0.1, [0.0, 50.0, 6.0, 10.0], [3.0, 10.0], true), Some(0.2235));
    assert_eq!(stepped(1.0, 0.3, handle, [3.0, 30.0], true), None);
}

/// While a thing is carried, the empty slots a drop would put it in are
/// lit (`SlotView.CheckAcceptHandler`): a backpack its own slot alone, a
/// rifle the weapon's, a stack none; none where something worn blocks
/// the slot, and not the slot it is in.
#[test]
fn a_thing_carried_lights_the_slots_that_would_take_it() {
    let (mut carried, [bag, rifle, rounds, _]) = carried();
    let worn = carried.equipment.item.id;
    let drawn = move || (0..3).map(move |slot| (worn, slot));
    assert_eq!(accepting_slots(&carried, bag, drawn(), false), vec![(worn, 1)]);
    assert_eq!(accepting_slots(&carried, rifle, drawn(), false), vec![(worn, 0)]);
    assert!(accepting_slots(&carried, rounds, drawn(), false).is_empty());
    // A vest that blocks the backpack's slot, worn.
    let vest = Item::new(Arc::new(Kind { blocks: vec!["Backpack".into()], ..kind("vest", &["Vest"], [2, 2]) }));
    carried.equipment.item.slots[2] = Some(vest);
    assert!(accepting_slots(&carried, bag, drawn(), false).is_empty());
    assert_eq!(accepting_slots(&carried, rifle, drawn(), false), vec![(worn, 0)]);
    // The rifle in its slot: that slot is full, and lit for nothing.
    let mut roots = [&mut carried.equipment.item, &mut carried.loot];
    assert_eq!(act(&mut roots, rifle, Target::Slot(worn, 0), &carried.rules, None), Ok("moved"));
    assert!(accepting_slots(&carried, rifle, drawn(), false).is_empty());
}

/// A container's window as the game's (`ItemUiContext.OpenWindow`, with
/// `PriorityWindowMode` Auto): opened again it is brought to the top
/// but not picked; as any window shuts, the top-most container's window
/// left is picked; and it shuts when its container is moved, even within
/// the inventory.
#[test]
fn container_windows_pick_and_shut_as_the_game() {
    let (mut carried, [bag, rifle, _, case]) = carried();
    let plain = [false; 3];
    click(&mut carried, case, true, plain, Vec2::ZERO, None);
    click(&mut carried, bag, true, plain, Vec2::ZERO, None);
    assert_eq!(carried.priority, Some(bag));
    click(&mut carried, case, true, plain, Vec2::ZERO, None);
    assert_eq!((carried.windows.last().map(|window| window.item), carried.priority), (Some(case), Some(bag)));
    // A window that shows a thing shut: the case's, on top, is picked.
    click(&mut carried, rifle, true, plain, Vec2::ZERO, None);
    carried.shut_window(carried.windows.len() - 1);
    assert_eq!(carried.priority, Some(case));
    // The case's shut: the bag's.
    let at = carried.windows.iter().position(|window| window.item == case).expect("the case's window");
    carried.shut_window(at);
    assert_eq!((carried.windows.len(), carried.priority), (1, Some(bag)));
    // The bag put on, so moved: its window shuts.
    click(&mut carried, bag, false, [false, false, true], Vec2::ZERO, None);
    assert!(carried.windows.is_empty() && carried.priority.is_none());
}

/// A window is kept on the screen as the game keeps it: its left and
/// bottom edges first, at whole pixels.
#[test]
fn a_window_is_kept_on_the_screen() {
    let room = Vec2::new(1920.0, 1080.0);
    assert_eq!(on_screen(Vec2::new(-30.0, 100.4), Vec2::new(261.0, 347.0), room), Vec2::new(0.0, 100.0));
    assert_eq!(on_screen(Vec2::new(1800.0, 900.0), Vec2::new(261.0, 347.0), room), Vec2::new(1659.0, 733.0));
    // Higher than the screen: its bottom kept on it.
    assert_eq!(on_screen(Vec2::new(10.0, 0.0), Vec2::new(261.0, 1200.0), room), Vec2::new(10.0, -120.0));
}

/// `courses10`'s `weapon-inspect-menu`, frame 501: the M4A1's window,
/// opened at 833.33, 68.67 (2560 by 1440), stretched by its corner to
/// 900.33 by 1302.67, has its top left on whole pixels, 833, 69; a half
/// goes to the even pixel (`Math.Round`).
#[test]
fn a_window_stretched_is_put_on_whole_pixels() {
    let room = Vec2::new(2560.0, 1440.0);
    assert_eq!(on_screen(Vec2::new(833.33, 68.67), Vec2::new(900.33, 1302.67), room), Vec2::new(833.0, 69.0));
    assert_eq!(on_screen(Vec2::new(832.5, 69.5), Vec2::new(900.0, 1300.0), room), Vec2::new(832.0, 70.0));
}
