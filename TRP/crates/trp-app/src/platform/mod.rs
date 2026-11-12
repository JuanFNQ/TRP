//! The app's own plumbing: window, graphics backend, frame limit, logs, first-start setup and key bindings.

pub(crate) mod options_store;
pub(crate) mod memory;
pub(crate) mod frame_limit;
pub(crate) mod debug_menu;
pub(crate) mod park;
pub(crate) mod unready;
pub(crate) mod convert;
pub(crate) mod cursor;
pub(crate) mod setup;
pub(crate) mod run_log;
pub(crate) mod keys;
pub(crate) mod backend;
pub(crate) mod render_timing;
pub(crate) mod window_icon;
