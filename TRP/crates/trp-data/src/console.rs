//! What the crates print, sent to the run's log as well as to the console.
//! Every line `println!` and `eprintln!` write in
//! `trp-data`, `trp-sim` and the app goes to the console as before and, once the app has set
//! one (`also_to`), to its run's log as well (`trp-app`'s `run_log`). Each crate's root defines
//! the two macros over `line`, so that a warning a library prints (a sound not read, say) is in
//! the log beside the app's own. Without a sink, as in the tests and the tools, they print alone.

use std::fmt;
use std::sync::OnceLock;

/// Where each printed line goes besides the console: set once, by the app.
static SINK: OnceLock<fn(&str)> = OnceLock::new();

/// Sends every line printed from now on to `sink` too. Only the first call counts.
pub fn also_to(sink: fn(&str)) {
    let _ = SINK.set(sink);
}

/// One line, printed to the standard output or the error stream, and given to the sink.
pub fn line(to_stderr: bool, args: fmt::Arguments) {
    let Some(sink) = SINK.get() else {
        if to_stderr {
            std::eprintln!("{args}");
        } else {
            std::println!("{args}");
        }
        return;
    };
    let text = fmt::format(args);
    if to_stderr {
        std::eprintln!("{text}");
    } else {
        std::println!("{text}");
    }
    sink(&text);
}

/// `println!` within the crate: the standard one, and the sink (`line`).
macro_rules! println {
    () => {
        $crate::console::line(false, ::std::format_args!(""))
    };
    ($($arg:tt)*) => {
        $crate::console::line(false, ::std::format_args!($($arg)*))
    };
}

/// `eprintln!` within the crate: the standard one, and the sink (`line`).
macro_rules! eprintln {
    () => {
        $crate::console::line(true, ::std::format_args!(""))
    };
    ($($arg:tt)*) => {
        $crate::console::line(true, ::std::format_args!($($arg)*))
    };
}
