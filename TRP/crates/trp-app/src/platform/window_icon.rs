//! The window's icon, the one the build puts into `trp.exe`.
//! It is read from the install by `build.rs` and kept as the icon resource numbered 1; none where the build found no install. The exe's own icon is what Explorer shows; the window's title bar and
//! taskbar button take theirs from the window, which winit leaves at the system's default, so the
//! same resource is given to the window once it exists.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

/// Gives the primary window the exe's icon, once. On the main thread, which holds the windows.
pub fn set(window: Query<Entity, With<PrimaryWindow>>, _main: bevy::ecs::system::NonSendMarker, mut done: Local<bool>) {
    if *done {
        return;
    }
    let Ok(entity) = window.single() else { return };
    bevy::winit::WINIT_WINDOWS.with_borrow(|windows| {
        if let Some(window) = windows.get_window(entity) {
            platform::set(window);
            *done = true;
        }
    });
}

#[cfg(windows)]
#[allow(unsafe_code)]
mod platform {
    use winit::dpi::PhysicalSize;
    use winit::platform::windows::{IconExtWindows, WindowExtWindows};
    use winit::window::{Icon, Window};

    /// The icon resource's number (`build.rs`).
    const RESOURCE: u16 = 1;
    const SM_CXICON: i32 = 11;
    const SM_CXSMICON: i32 = 49;

    #[link(name = "user32")]
    extern "system" {
        fn GetSystemMetrics(index: i32) -> i32;
    }

    fn icon(metric: i32) -> Option<Icon> {
        // SAFETY: a plain query of the system's settings.
        let side = unsafe { GetSystemMetrics(metric) }.max(16) as u32;
        Icon::from_resource(RESOURCE, Some(PhysicalSize::new(side, side))).ok()
    }

    /// The small icon for the title bar, the large one for the taskbar and Alt+Tab, each at the
    /// size the system draws it, so that it picks the nearest of the file's sizes.
    pub fn set(window: &Window) {
        window.set_window_icon(icon(SM_CXSMICON));
        window.set_taskbar_icon(icon(SM_CXICON));
    }
}

#[cfg(not(windows))]
mod platform {
    pub fn set(_window: &winit::window::Window) {}
}
