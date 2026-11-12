//! The interface frame by frame, as the recorder writes it beside a trace and as the app writes it.
//! The recorder's `<trace>.ui.jsonl` (recorder 0.16.0, `hosts/spt-oracle/Interface.cs`), the app's
//! `--dump-ui`: a JSON object a line, for each frame on which anything of the interface changed. A line holds
//! `frame` and `t` and only what changed: the screen shown, the window's size, the cursor, the
//! mouse and its buttons, what the pointer is over, pressing and dragging, the canvases, the
//! elements added (whole), set (the changed fields alone, `null` for a field gone) and gone, and
//! the interface's sounds. Boxes are `[left, top, right, bottom]` in the window's pixels from its
//! top left. The format in full: `docs/manual.md`, recorder 0.16.0.
//!
//! [`Trace`] reads a file; [`State`] is the interface as it stands after a frame, replayed from
//! the first line; [`beside`] pairs the elements drawn in two interfaces (the game's and the
//! app's). The reader is lenient: a key it does not know is passed over, and a line it cannot
//! read is kept aside with its number and skipped. `uitrace` (`src/bin/uitrace.rs`) prints them.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};
use serde_json::{Map, Value};

mod lines;
mod frame_state;
mod trace_file;
mod side_by_side;

pub use lines::*;
pub use frame_state::*;
pub use trace_file::*;
pub use side_by_side::*;
use lines::{four, whole, words_of};

#[cfg(test)]
mod tests;
