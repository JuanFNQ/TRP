//! The mouse and the window. The app's own, not the game's: how a window
//! holds the pointer is the host's business.
//!
//! While the character plays the pointer is hidden and held where it is;
//! while a screen is open it is shown and free to leave the window, for
//! another window or another screen (the user, 2026-10-14: "make it
//! completely free to move if we press tab"); with the free camera, or
//! with the window not the one in front, it is free.
//! The engine puts a hold on the pointer only when what is asked of it
//! changes, and the system lets go of one whenever another window comes to
//! the front, so the hold is asked for afresh from what stands every frame:
//! coming back to the window takes the pointer again.
//!
//! On Windows a key pressed with Alt held is first the window's own: the
//! system takes it for a letter of the window's menu, and for the moment
//! it looks for one the pointer is shown and let go. `Alt` is a key of the
//! game's (the breath, the step aside, the slow lean, the looks at the
//! weapon), so the window is kept from going to look (`keep_alt`): the
//! command the system sends for it is answered, and nothing else is. The
//! key itself, and the letter the system makes of it, have to reach the
//! window untouched: the engine's own account of a key going down waits
//! for that letter, and a key whose letter never comes is never pressed
//! (2026-10-13: with the letter answered here too, nothing was heard of
//! any key pressed with Alt held, the walk's among them).

use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;
use bevy::window::{CursorGrabMode, CursorIcon, CursorOptions, CustomCursor, CustomCursorImage, PrimaryWindow};

use crate::controller::Controller;
use crate::health_view::Piece;
use crate::{Options, View};

/// The game's own mouse pointers (`EFT.UI.CursorSwitcher`; inventory spec,
/// "The mouse pointer"), with `TARKOV UI`: read as the run begins
/// (`trp_data::screen::cursor_pictures`), made into pictures when first
/// shown, and the one shown now. Without them (the switch off, or none
/// read) the system's own arrow stays.
#[derive(Resource, Default)]
pub struct Pointers {
    pictures: Vec<trp_data::screen::CursorPicture>,
    made: Vec<(String, Handle<Image>, [u32; 2])>,
    shown: Option<&'static str>,
}

impl Pointers {
    pub fn of(pictures: Vec<trp_data::screen::CursorPicture>) -> Self {
        Self { pictures, made: Vec::new(), shown: None }
    }
}

/// The game's pointer at rest (`ECursorType.Idle`) and over a button
/// (`Hover`).
const IDLE: &str = "cursor_idle";
const HOVER: &str = "cursor_over";

/// The game's pointers over a window's stretching places
/// (`StretchArea.Init`): the top and bottom edges, the left and right ones,
/// the top left and bottom right corners (`\`), the top right and bottom
/// left ones (`/`).
const STRETCH_VERTICAL: &str = "cursor_stretch_vertical";
const STRETCH_HORIZONTAL: &str = "cursor_stretch_horizontal";
const STRETCH_CORNER: &str = "cursor_stretch_corner";
const STRETCH_CORNER_2: &str = "cursor_stretch_corner_2";

/// The pointer of a stretching place by its sides (`EStretchAreaType`: top
/// 1, bottom 2, left 4, right 8).
fn stretch_pointer(sides: u8) -> &'static str {
    match sides {
        1 | 2 => STRETCH_VERTICAL,
        4 | 8 => STRETCH_HORIZONTAL,
        5 | 10 => STRETCH_CORNER,
        _ => STRETCH_CORNER_2,
    }
}

/// A window's stretching place under the pointer: its window's place among
/// the windows, and its sides (`inventory_view` makes them).
#[derive(Component)]
pub struct StretchPointer(pub usize, pub u8);

/// The game's scripts that turn the pointer to its hover look while it is
/// over their object (`CursorSwitcher.SetCursor(ECursorType.Hover)` on the
/// pointer's coming in, `Idle` on its going out): `ButtonFeedback` and
/// every class built on it, which also put it to rest on a click, and
/// `AnimatedToggle`, the last, which does not. (`ClickableUIText` asks for
/// the hand over a link of its words: no text the app draws has one.)
pub const HOVER_SCRIPTS: [&str; 13] = [
    "ButtonFeedback",
    "AreaPanel",
    "AreaRequirementPanel",
    "AreaWorldPanel",
    "ChangeGameModeButton",
    "DefaultUIButton",
    "EmptyMemberView",
    "GroupMemberView",
    "MarathonButton",
    "PartyPlayerItem",
    "SlotViewHeader",
    "UISpawnableToggle",
    "AnimatedToggle",
];

/// Where a drawn object of the game's screens turns the pointer to its
/// hover look (`HOVER_SCRIPTS`): an empty node over the object's own
/// rectangle, by the number of its piece (`Piece`), which says whether the
/// object is shown, and whether a click puts the pointer to rest.
#[derive(Component)]
pub struct HoverPointer(pub usize, pub bool);

/// Whether a drawn object of the game's screens turns the pointer to its
/// hover look (one of `HOVER_SCRIPTS` on it), and if so whether a click on
/// it puts the pointer to rest (one of the `ButtonFeedback` kind on it).
pub fn hover_of(scripts: &[(String, Vec<u8>)]) -> Option<bool> {
    let on = |class: &str| scripts.iter().any(|(own, _)| own == class);
    let (feedback, toggle) = HOVER_SCRIPTS.split_at(HOVER_SCRIPTS.len() - 1);
    let resets = feedback.iter().any(|class| on(class));
    (resets || toggle.iter().any(|class| on(class))).then_some(resets)
}

/// The game's pointer as things stand. Over a window's stretching place,
/// its stretching pointer; once pressed there, that pointer kept until the
/// button is let go, wherever the pointer goes (`StretchArea`: forced).
/// Else its hover look while the pointer is over a shown object whose
/// script asks for it, unless a click on that object put it to rest
/// (`ButtonFeedback.PointerClickHandler`), until the pointer leaves it;
/// its rest otherwise. The system draws it (a hardware pointer at the
/// picture's own size, as the game's), only while it is shown at all
/// (`hold`).
#[allow(clippy::too_many_arguments)]
pub fn point(
    mut commands: Commands,
    mut pointers: ResMut<Pointers>,
    mut images: ResMut<Assets<Image>>,
    buttons: Res<ButtonInput<MouseButton>>,
    window: Query<Entity, With<PrimaryWindow>>,
    hovers: Query<(&HoverPointer, &RelativeCursorPosition, &InheritedVisibility)>,
    stretches: Query<(&StretchPointer, &RelativeCursorPosition)>,
    pieces: Query<(&Piece, &InheritedVisibility)>,
    (mut forced, mut rested): (Local<Option<u8>>, Local<Option<usize>>),
) {
    if pointers.pictures.is_empty() && pointers.made.is_empty() {
        return;
    }
    let stretch = stretches.iter().filter(|(_, at)| at.cursor_over()).max_by_key(|(place, _)| place.0).map(|(place, _)| place.1);
    if buttons.just_pressed(MouseButton::Left) {
        *forced = stretch;
    }
    if !buttons.pressed(MouseButton::Left) {
        *forced = None;
    }
    let shown_pieces: std::collections::HashSet<usize> = if hovers.is_empty() {
        Default::default()
    } else {
        pieces.iter().filter(|(_, seen)| seen.get()).map(|(piece, _)| piece.0).collect()
    };
    let over = hovers.iter().find(|(hover, at, seen)| seen.get() && at.cursor_over() && shown_pieces.contains(&hover.0)).map(|(hover, ..)| (hover.0, hover.1));
    // A click on what turns it to rest; left, it is forgotten.
    if rested.is_some() && over.map(|(piece, _)| piece) != *rested {
        *rested = None;
    }
    if let Some((piece, true)) = over.filter(|_| buttons.just_released(MouseButton::Left)) {
        *rested = Some(piece);
    }
    let wanted = match (*forced, stretch, over) {
        (Some(sides), ..) | (None, Some(sides), _) => stretch_pointer(sides),
        (None, None, Some((piece, _))) if *rested != Some(piece) => HOVER,
        _ => IDLE,
    };
    if pointers.shown == Some(wanted) {
        return;
    }
    let Ok(window) = window.single() else { return };
    if pointers.made.is_empty() {
        let pictures = std::mem::take(&mut pointers.pictures);
        for picture in &pictures {
            let image = Image::new(
                bevy::render::render_resource::Extent3d { width: picture.size[0], height: picture.size[1], depth_or_array_layers: 1 },
                bevy::render::render_resource::TextureDimension::D2,
                picture.rgba.clone(),
                bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
                bevy::asset::RenderAssetUsages::MAIN_WORLD | bevy::asset::RenderAssetUsages::RENDER_WORLD,
            );
            pointers.made.push((picture.path.clone(), images.add(image), picture.hotspot));
        }
        pointers.pictures = pictures;
    }
    let Some((_, handle, hotspot)) = pointers.made.iter().find(|(path, _, _)| path == wanted) else { return };
    let image = CustomCursorImage { handle: handle.clone(), hotspot: (hotspot[0] as u16, hotspot[1] as u16), ..default() };
    commands.entity(window).insert(CursorIcon::Custom(CustomCursor::Image(image)));
    pointers.shown = Some(wanted);
}

/// Holds the pointer as the play stands. A run nobody is at the keys of
/// leaves it alone.
pub fn hold(options: Res<Options>, view: Res<View>, controller: Res<Controller>, panel: Option<Res<crate::hud::GuideShown>>, mut window: Query<(&mut Window, &mut CursorOptions), With<PrimaryWindow>>) {
    if options.unattended() {
        return;
    }
    let Ok((mut window, mut cursor)) = window.single_mut() else { return };
    let (grab, visible) = if controller.edit_build && crate::edit_build::turning() {
        // The edit build screen's preview turned by a drag: the mouse
        // hidden and held (`ItemObserveScreen.ShouldLockCursor`).
        (CursorGrabMode::Locked, false)
    } else if !window.focused || controller.inventory_open || controller.edit_build || crate::hud::panel_open(panel.as_deref(), &controller) {
        // (The debug panel open: its buttons are the mouse's, `debug_menu`.)
        (CursorGrabMode::None, true)
    } else if view.played() {
        (CursorGrabMode::Locked, false)
    } else {
        (CursorGrabMode::None, true)
    };
    // Written only when it differs: the window is otherwise seen as
    // changed on every frame.
    if cursor.grab_mode != grab {
        // The pointer is held where it is, so it is first put in the
        // middle of the window: one that was outside it (the window come
        // to the front by the keyboard) would be held out there, in sight.
        if grab == CursorGrabMode::Locked {
            let middle = Vec2::new(window.width(), window.height()) / 2.0;
            window.set_cursor_position(Some(middle));
        }
        cursor.grab_mode = grab;
    }
    if cursor.visible != visible {
        cursor.visible = visible;
    }
}

/// Keeps the window from taking a key pressed with Alt for its own menu,
/// once, when the window is first the one in front. On the main thread,
/// which is the window's own.
pub fn keep_alt(options: Res<Options>, window: Query<&Window, With<PrimaryWindow>>, _main: bevy::ecs::system::NonSendMarker, mut kept: Local<bool>) {
    if *kept || options.alt_menu || options.unattended() || !window.single().is_ok_and(|window| window.focused) {
        return;
    }
    *kept = alt::keep();
}

#[cfg(windows)]
#[allow(unsafe_code)]
mod alt {
    //! The window's own procedure is put behind one that answers the
    //! command by which the system goes to look for a letter of the
    //! window's menu, and hands every other message on.

    use std::sync::atomic::{AtomicIsize, Ordering};

    const GWLP_WNDPROC: i32 = -4;
    const WM_SYSCOMMAND: u32 = 0x0112;
    const SC_KEYMENU: usize = 0xF100;

    #[link(name = "user32")]
    extern "system" {
        fn GetForegroundWindow() -> isize;
        fn GetWindowThreadProcessId(window: isize, process: *mut u32) -> u32;
        fn SetWindowLongPtrW(window: isize, index: i32, value: isize) -> isize;
        fn CallWindowProcW(procedure: isize, window: isize, message: u32, wparam: usize, lparam: isize) -> isize;
        fn DefWindowProcW(window: isize, message: u32, wparam: usize, lparam: isize) -> isize;
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn GetCurrentProcessId() -> u32;
    }

    /// The procedure the window had before.
    static BEFORE: AtomicIsize = AtomicIsize::new(0);

    unsafe extern "system" fn procedure(window: isize, message: u32, wparam: usize, lparam: isize) -> isize {
        // Alt with a key, as the command it becomes once the window's own
        // procedure has heard the key and its letter: answered here, so
        // that the system never looks for a menu.
        if message == WM_SYSCOMMAND && wparam & 0xFFF0 == SC_KEYMENU {
            return 0;
        }
        match BEFORE.load(Ordering::Relaxed) {
            0 => DefWindowProcW(window, message, wparam, lparam),
            before => CallWindowProcW(before, window, message, wparam, lparam),
        }
    }

    /// Whether the window in front is this run's and now has the
    /// procedure above.
    pub fn keep() -> bool {
        // SAFETY: plain calls into the system with a window handle it has
        // just given out, on the thread that made the window; the
        // procedure put in place is a function of this program's that
        // lives as long as it does, and it hands on to the one it took
        // the place of.
        unsafe {
            let window = GetForegroundWindow();
            let mut process = 0;
            if window == 0 || GetWindowThreadProcessId(window, &mut process) == 0 || process != GetCurrentProcessId() {
                return false;
            }
            let before = SetWindowLongPtrW(window, GWLP_WNDPROC, procedure as *const () as isize);
            if before == 0 {
                return false;
            }
            BEFORE.store(before, Ordering::Relaxed);
            true
        }
    }
}

#[cfg(not(windows))]
mod alt {
    /// Nothing to keep the key from on other systems.
    pub fn keep() -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each stretching place's pointer by its sides, as `StretchArea.Init`
    /// gives them.
    #[test]
    fn a_stretching_place_has_the_game_s_pointer() {
        let pointers: Vec<&str> = [1, 2, 4, 8, 5, 10, 9, 6].into_iter().map(stretch_pointer).collect();
        assert_eq!(pointers, [STRETCH_VERTICAL, STRETCH_VERTICAL, STRETCH_HORIZONTAL, STRETCH_HORIZONTAL, STRETCH_CORNER, STRETCH_CORNER, STRETCH_CORNER_2, STRETCH_CORNER_2]);
    }

    /// A button of the `ButtonFeedback` kind puts the pointer to rest on a
    /// click; a toggle that is only an `AnimatedToggle` does not; a plain
    /// object turns it to nothing.
    #[test]
    fn which_objects_turn_the_pointer() {
        let of = |classes: &[&str]| hover_of(&classes.iter().map(|class| (class.to_string(), Vec::new())).collect::<Vec<_>>());
        assert_eq!((of(&["DefaultUIButton"]), of(&["AnimatedToggle"]), of(&["AnimatedToggle", "UISpawnableToggle"]), of(&["Image"])), (Some(true), Some(false), Some(true), None));
    }
}
