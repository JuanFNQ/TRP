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
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

use crate::controller::Controller;
use crate::{Options, View};

/// Holds the pointer as the play stands. A run nobody is at the keys of
/// leaves it alone.
pub fn hold(options: Res<Options>, view: Res<View>, controller: Res<Controller>, mut window: Query<(&mut Window, &mut CursorOptions), With<PrimaryWindow>>) {
    if options.unattended() {
        return;
    }
    let Ok((mut window, mut cursor)) = window.single_mut() else { return };
    let (grab, visible) = if !window.focused || controller.inventory_open {
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
