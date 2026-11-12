//! The run's log, the check of the install's versions and a crash's report.
//! The app's own, for the people running it, not a system of the controller: no switch among the features.
//! `docs/manual.md`, "A run's log and a crash's report".
//!
//! Each run writes `local\logs\run_<date>_<time>.txt` (UTC): TRP's version, the command line,
//! the paths, the install's versions, the graphics card and backend, and every line the run
//! prints (the app's, the libraries' through `trp_data::console`, and the engine's own log),
//! as it goes, so that a run that dies without a panic still leaves what it said. A panic writes
//! `local\logs\crash_<date>_<time>.txt`: its message, where it was, the thread and the
//! backtrace, then the run's log as far as it went; the console and, when someone is at it, a
//! message box say where it is (the window is gone by then). A start that fails says where the
//! run's log is in the same way. Both files hold the folder names of the machine they ran on,
//! which is said wherever they are offered for sharing. The newest `KEPT` of each are kept.
//!
//! The install's versions are checked as soon as the install is found, before anything is read
//! from it: a client or an SPT not of the releases TRP was built for is said plainly, naming
//! both, and the run goes on.

use std::fs::{File, OpenOptions};
use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard, OnceLock, PoisonError};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use bevy::prelude::*;
use trp_data::setup::versions::same_release;
use trp_data::Install;

/// The releases TRP was built for, the game's client and SPT: Tarkov 0.16.9 and SPT 4.1, the
/// install every reading was made from being the client 0.16.9.5-40743
/// with SPT 4.1.6. A later build of either (0.16.9.6, 4.1.7)
/// is of the same release.
pub const BUILT_FOR_GAME: &str = "0.16.9";
pub const BUILT_FOR_SPT: &str = "4.1";

/// How many of each kind of file `local\logs` keeps: the newest.
const KEPT: usize = 20;

/// The most a run's log takes; past it, the console alone has the rest.
const MOST: u64 = 4 << 20;

struct Log {
    file: File,
    path: PathBuf,
    written: u64,
    full: bool,
}

static LOG: Mutex<Option<Log>> = Mutex::new(None);
static REPORT: Mutex<Option<PathBuf>> = Mutex::new(None);
static STARTED: OnceLock<Instant> = OnceLock::new();
static ATTENDED: AtomicBool = AtomicBool::new(false);
static PHASE: Mutex<Option<Instant>> = Mutex::new(None);

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

fn seconds() -> f64 {
    STARTED.get().map_or(0.0, |started| started.elapsed().as_secs_f64())
}

/// Whether someone is at the run to read a message box: a console to type in, and not a
/// capture or a scripted scenario (as the first start's questions, `setup::wanted`).
pub fn attended() -> bool {
    ATTENDED.load(Ordering::Relaxed)
}

/// Begins the run's log and sets the panic's report, first thing.
pub fn start(args: &[String]) {
    STARTED.get_or_init(Instant::now);
    let capture = args.iter().any(|arg| arg == "--screenshot" || arg == "--scenario");
    ATTENDED.store(std::io::stdin().is_terminal() && !capture, Ordering::Relaxed);
    on_panic();
    let folder = crate::platform::setup::local_folder().join("logs");
    let (file, path) = match open(&folder, "run") {
        Ok(opened) => opened,
        Err(e) => {
            eprintln!("this run has no log: {e:#}");
            return;
        }
    };
    *lock(&LOG) = Some(Log { file, path: path.clone(), written: 0, full: false });
    trp_data::console::also_to(note);
    note(&format!("TRP {}, started {}", env!("TRP_VERSION"), stamp().1));
    note(SHARING);
    note(&format!("the command line: {}", args.join(" ")));
    let program = std::env::current_exe().map_or_else(|e| format!("not known ({e})"), |path| path.display().to_string());
    let here = std::env::current_dir().map_or_else(|e| format!("not known ({e})"), |path| path.display().to_string());
    note(&format!("the program: {program}"));
    note(&format!("the current folder: {here}"));
    note(&format!("TRP's local folder: {}", crate::platform::setup::local_folder().display()));
    println!("this run's log: {}", path.display());
}

/// What is said wherever the log or a report is offered for sharing.
const SHARING: &str = "This file holds the folder names of the machine it ran on (the paths in it): look it over before you share it.";

/// One line into the run's log, after the seconds since the run began; the lines after the
/// first of a text of several are set under it.
pub fn note(line: &str) {
    let mut log = lock(&LOG);
    let Some(log) = log.as_mut().filter(|log| !log.full) else { return };
    let mut text = String::new();
    for (index, part) in line.trim_end_matches(['\n', '\r']).split('\n').enumerate() {
        let part = part.trim_end_matches('\r');
        if index == 0 {
            text.push_str(&format!("[{:9.3}] {part}\n", seconds()));
        } else {
            text.push_str(&format!("            {part}\n"));
        }
    }
    if log.written + text.len() as u64 > MOST {
        text = format!("[{:9.3}] the log has its {} MB: the rest of the run is on the console alone\n", seconds(), MOST >> 20);
        log.full = true;
    }
    let _ = log.file.write_all(text.as_bytes());
    log.written += text.len() as u64;
}

/// A mark of the start-up's parts in the run's log: the milliseconds since
/// the last mark (the first, since the run began), so that what the loading
/// costs is read part by part.
pub fn phase(name: &str) {
    let now = Instant::now();
    let since = lock(&PHASE).replace(now).or_else(|| STARTED.get().copied()).map_or(0.0, |then| (now - then).as_secs_f64() * 1000.0);
    note(&format!("start-up: {name}, {since:.0} ms"));
}

/// The run's log, where there is one.
pub fn log_path() -> Option<PathBuf> {
    lock(&LOG).as_ref().map(|log| log.path.clone())
}

/// A new file of `kind` (`run`, `crash`) in `folder`, named by the time; the oldest beyond
/// the newest `KEPT` removed first.
fn open(folder: &Path, kind: &str) -> anyhow::Result<(File, PathBuf)> {
    std::fs::create_dir_all(folder).map_err(|e| anyhow::anyhow!("{}: {e}", folder.display()))?;
    prune(folder, kind, KEPT - 1);
    let named = stamp().0;
    let mut last = None;
    for attempt in 1..100 {
        let name = if attempt == 1 { format!("{kind}_{named}.txt") } else { format!("{kind}_{named}_{attempt}.txt") };
        let path = folder.join(name);
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => return Ok((file, path)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => last = Some(e),
            Err(e) => anyhow::bail!("{}: {e}", path.display()),
        }
    }
    anyhow::bail!("{}: no free name ({last:?})", folder.display())
}

/// Leaves the newest `keep` files of `kind` in `folder` (their names sort by their time).
fn prune(folder: &Path, kind: &str, keep: usize) {
    let Ok(entries) = std::fs::read_dir(folder) else { return };
    let prefix = format!("{kind}_");
    let mut named: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.file_name().and_then(|name| name.to_str()).is_some_and(|name| name.starts_with(&prefix) && name.ends_with(".txt")))
        .collect();
    named.sort();
    let surplus = named.len().saturating_sub(keep);
    for path in &named[..surplus] {
        let _ = std::fs::remove_file(path);
    }
}

// ==== The install's versions ====

/// The install's paths and versions into the log, and a warning where the game's client or SPT
/// is not of the release TRP was built for (or its version cannot be read): on the console and,
/// with someone at the run, in a message box, naming both. The run goes on either way.
pub fn check_install(install: &Install) {
    let named_by = match std::env::var_os("SPT_DIR") {
        Some(_) => "SPT_DIR".to_owned(),
        None => std::env::current_dir()
            .ok()
            .and_then(|here| here.ancestors().map(|dir| dir.join("local").join("spt_dir.txt")).find(|hint| hint.is_file()))
            .map_or_else(|| "nothing found".to_owned(), |hint| hint.display().to_string()),
    };
    note(&format!("the install: {} (named by {named_by})", install.root.display()));
    let local = crate::platform::setup::local_folder();
    for (game, variable, file) in [("GAMMA", "GAMMA_DIR", "gamma_dir.txt"), ("Anomaly", "ANOMALY_DIR", "anomaly_dir.txt")] {
        let named = std::env::var(variable).ok().or_else(|| trp_data::setup::install::read_hint(&local.join(file)));
        note(&format!("{game}'s folder: {}", named.as_deref().unwrap_or("none named")));
    }
    // In the log, each version with the file it was read from, or why it could not be read
    // (which names the file); in what the console and the message box say, the version alone.
    let read = |version: &anyhow::Result<String>, from: &Path| match version {
        Ok(version) => format!("{version} ({})", from.display()),
        Err(e) => format!("not known: {e:#}"),
    };
    let said = |version: &anyhow::Result<String>| match version {
        Ok(version) => version.clone(),
        Err(_) => "not known (its version could not be read)".to_owned(),
    };
    let game = install.game_version();
    let server = install.spt_version();
    let client = install.spt_client_version();
    note(&format!("the game's client: {}", read(&game, &install.game_program())));
    note(&format!("SPT's server: {}", read(&server, &install.spt_server_program())));
    note(&format!("SPT's client plugin: {}", read(&client, &install.spt_client_program())));
    match install.spt_made_for() {
        Ok(version) => note(&format!("SPT made for the game's client: {version}")),
        Err(e) => note(&format!("SPT made for the game's client: not known: {e:#}")),
    }
    // SPT's version is its server's (whose database is read); its client plugin's where the
    // server's cannot be read.
    let spt = match server {
        Ok(version) => Ok(version),
        Err(e) => client.map_err(|_| e),
    };
    let game_ok = game.as_ref().is_ok_and(|version| same_release(version, BUILT_FOR_GAME));
    let spt_ok = spt.as_ref().is_ok_and(|version| same_release(version, BUILT_FOR_SPT));
    if game_ok && spt_ok {
        println!("the install: the game's client {}, SPT {}, of the releases TRP was built for", said(&game), said(&spt));
        return;
    }
    let message = mismatch(&install.root.display().to_string(), &said(&game), &said(&spt));
    eprintln!("warning: {}", message.replace("\n\n", " ").replace('\n', " "));
    if attended() {
        dialog("TRP: another version of the game", &message, false);
    }
}

/// What a mismatch says: the releases TRP was built for and those found, both named.
fn mismatch(install: &str, game: &str, spt: &str) -> String {
    format!(
        "TRP was built for the game's client {BUILT_FOR_GAME} with SPT {BUILT_FOR_SPT}.\nThis install ({install}) has the game's client {game} with SPT {spt}.\n\nTRP will try to start anyway, but what it reads from the install may have changed: if something fails, this is the likely reason."
    )
}

// ==== The start-up's systems and the long frames ====

/// Marks the engine's start-up schedules in the run's log (`phase`).
pub fn startup_begun() {
    phase("the engine's plugins finished");
}

pub fn startup_ended() {
    phase("the start-up systems");
}

/// The longest a frame is let take before the run's log notes it.
const LONG_FRAME: f64 = 0.1;
/// The most long frames noted in a run.
const LONG_FRAMES_NOTED: u32 = 200;

/// Notes each frame that took longer than `LONG_FRAME` by the wall (a load
/// or a stutter), with its number, so that a stall is found in the log by
/// what was said about it.
pub fn note_long_frames(frames: Res<bevy::diagnostic::FrameCount>, mut last: Local<Option<Instant>>, mut noted: Local<u32>) {
    let now = Instant::now();
    if let Some(then) = last.replace(now) {
        let took = (now - then).as_secs_f64();
        if took > LONG_FRAME && *noted < LONG_FRAMES_NOTED {
            *noted += 1;
            note(&format!("a long frame: {:.0} ms, frame {}", took * 1000.0, frames.0));
        }
    }
}

// ==== The graphics card, the engine's log, the end ====

/// The graphics card and the backend the engine drew with, as it found them.
pub fn graphics_card(adapter: Option<Res<bevy::render::renderer::RenderAdapterInfo>>) {
    match adapter {
        Some(info) => println!("graphics card: {} ({:?}) through {:?}, driver {} {}", info.name, info.device_type, info.backend, info.driver, info.driver_info),
        None => println!("graphics card: none reported by the engine"),
    }
}

/// The engine's own log (Bevy's and the graphics library's warnings and errors, and what they
/// say as they start) into the run's log as well as the console (`LogPlugin::custom_layer`).
pub fn engine_log(_app: &mut App) -> Option<bevy::log::BoxedLayer> {
    use bevy::log::tracing_subscriber::fmt;
    Some(Box::new(fmt::layer().with_ansi(false).without_time().with_writer(|| ToLog)))
}

struct ToLog;

impl Write for ToLog {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let text = String::from_utf8_lossy(bytes);
        if !text.trim().is_empty() {
            note(&text);
        }
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// The run's end into its log.
pub fn ended() {
    note(&format!("ended after {:.1} s", seconds()));
}

/// A start that failed (`main`'s error): into the log, and, with someone at the run, where the
/// log is in a message box (the console a double-click opened goes with the run).
pub fn failed(error: &anyhow::Error) {
    note(&format!("what failed: {error:#}"));
    let Some(path) = log_path() else { return };
    eprintln!("this run's log: {} ({})", path.display(), SHARING_SHORT);
    if attended() {
        let text = format!("TRP stopped:\n{error:#}\n\nThis run's log is in:\n{}\n\n{SHARING}", path.display());
        dialog("TRP stopped", &text, true);
    }
}

const SHARING_SHORT: &str = "it holds your own folder names: look it over before you share it";

// ==== A crash's report ====

/// Each panic, after the standard report on the console (the hook before), is written to a
/// report: the first makes it, any after are added to it.
fn on_panic() {
    let earlier = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        earlier(info);
        crashed(info);
    }));
}

fn crashed(info: &std::panic::PanicHookInfo) {
    let message = match (info.payload().downcast_ref::<&str>(), info.payload().downcast_ref::<String>()) {
        (Some(text), _) => (*text).to_owned(),
        (None, Some(text)) => text.clone(),
        (None, None) => "(the panic says nothing in words)".to_owned(),
    };
    let place = info.location().map_or_else(|| "not said".to_owned(), |at| format!("{}:{}:{}", at.file(), at.line(), at.column()));
    let thread = std::thread::current();
    let thread = thread.name().unwrap_or("(unnamed)").to_owned();
    let backtrace = std::backtrace::Backtrace::force_capture();
    note(&format!("crashed, on the thread {thread}: {message} (at {place})"));
    let panic = format!("The panic: {message}\nWhere: {place}\nThe thread: {thread}\n\nThe backtrace:\n{backtrace}");
    let mut report = lock(&REPORT);
    let first = report.is_none();
    let path = match report.as_ref() {
        Some(path) => {
            if let Ok(mut file) = OpenOptions::new().append(true).open(path) {
                let _ = write!(file, "\n\nThen another panic, {:.1} s into the run:\n{panic}", seconds());
            }
            Some(path.clone())
        }
        None => {
            let folder = crate::platform::setup::local_folder().join("logs");
            match open(&folder, "crash") {
                Ok((mut file, path)) => {
                    let (log, so_far) = match log_path() {
                        Some(log) => (log.display().to_string(), std::fs::read_to_string(&log).unwrap_or_else(|e| format!("(not read: {e})"))),
                        None => ("none".to_owned(), "(this run had no log)".to_owned()),
                    };
                    let _ = write!(
                        file,
                        "TRP {} crashed: {}, {:.1} s into the run.\n\n{SHARING}\n\n{panic}\n\nThe run's log as far as it went ({log}):\n{so_far}",
                        env!("TRP_VERSION"),
                        stamp().1,
                        seconds()
                    );
                    *report = Some(path.clone());
                    Some(path)
                }
                Err(e) => {
                    eprintln!("the crash's report could not be written: {e:#}");
                    None
                }
            }
        }
    };
    drop(report);
    let Some(path) = path else { return };
    eprintln!("TRP crashed: {message}. Its report: {} ({SHARING_SHORT}).", path.display());
    if first && attended() {
        let text = format!("TRP crashed:\n{message}\n\nIts report, with what led to it, is in:\n{}\n\n{SHARING}", path.display());
        dialog("TRP crashed", &text, true);
    }
}

/// A panic asked for (`--crash`), on the first frame, to see the report and its message box.
pub fn crash_on_purpose() {
    panic!("a crash asked for (--crash)");
}

// ==== Times and the message box ====

/// The time now (UTC): as a file's name has it (`2026-10-31_18-04-12`), and as read.
fn stamp() -> (String, String) {
    let seconds = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| since.as_secs());
    let (year, month, day) = civil((seconds / 86_400) as i64);
    let (hour, minute, second) = (seconds % 86_400 / 3600, seconds % 3600 / 60, seconds % 60);
    (
        format!("{year:04}-{month:02}-{day:02}_{hour:02}-{minute:02}-{second:02}"),
        format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02} UTC"),
    )
}

/// The date of a count of days since 1970-01-01 in the Gregorian calendar (Howard Hinnant's
/// `civil_from_days`, published into the public domain).
fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era = (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_from_march = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * month_from_march + 2) / 5 + 1) as u32;
    let month = (if month_from_march < 10 { month_from_march + 3 } else { month_from_march - 9 }) as u32;
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

/// A message box over everything, until it is answered: on a thread of its own, so that the
/// window's own messages are not handled under it while this one waits.
fn dialog(title: &str, text: &str, error: bool) {
    platform::message_box(title, text, error);
}

#[cfg(windows)]
#[allow(unsafe_code)]
mod platform {
    const MB_ICONERROR: u32 = 0x10;
    const MB_ICONWARNING: u32 = 0x30;
    const MB_SETFOREGROUND: u32 = 0x1_0000;
    const MB_TOPMOST: u32 = 0x4_0000;

    #[link(name = "user32")]
    extern "system" {
        fn MessageBoxW(owner: *mut core::ffi::c_void, text: *const u16, caption: *const u16, kind: u32) -> i32;
    }

    pub fn message_box(title: &str, text: &str, error: bool) {
        let wide = |text: &str| text.encode_utf16().chain([0]).collect::<Vec<u16>>();
        let (title, text) = (wide(title), wide(text));
        let kind = if error { MB_ICONERROR } else { MB_ICONWARNING } | MB_SETFOREGROUND | MB_TOPMOST;
        let _ = std::thread::spawn(move || {
            // SAFETY: two strings ended by zeros, alive through the call, and no owner window.
            unsafe { MessageBoxW(std::ptr::null_mut(), text.as_ptr(), title.as_ptr(), kind) };
        })
        .join();
    }
}

#[cfg(not(windows))]
mod platform {
    pub fn message_box(_title: &str, _text: &str, _error: bool) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn days_to_dates() {
        assert_eq!(civil(0), (1970, 1, 1));
        assert_eq!(civil(-1), (1969, 12, 31));
        assert_eq!(civil(19_723), (2024, 1, 1));
        assert_eq!(civil(19_782), (2024, 2, 29));
        assert_eq!(civil(19_783), (2024, 3, 1));
    }

    /// The newest kept, by name; files of another kind left alone.
    #[test]
    fn the_oldest_logs_removed() {
        let folder = std::env::temp_dir().join(format!("trp-run-log-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).unwrap();
        for day in 10..30 {
            std::fs::write(folder.join(format!("run_2026-10-{day}_00-00-00.txt")), "").unwrap();
        }
        std::fs::write(folder.join("crash_2026-10-01_00-00-00.txt"), "").unwrap();
        prune(&folder, "run", 5);
        let mut left: Vec<String> = std::fs::read_dir(&folder).unwrap().map(|entry| entry.unwrap().file_name().into_string().unwrap()).collect();
        left.sort();
        assert_eq!(left.len(), 6);
        assert_eq!(left[0], "crash_2026-10-01_00-00-00.txt");
        assert_eq!(left[1], "run_2026-10-25_00-00-00.txt");
        let (_, path) = open(&folder, "run").unwrap();
        assert!(path.file_name().unwrap().to_str().unwrap().starts_with("run_"));
        let _ = std::fs::remove_dir_all(&folder);
    }

    #[test]
    fn a_mismatch_names_both() {
        let said = mismatch("D:\\SPT", "0.16.10.1-41000", "4.2.0");
        assert!(said.contains("0.16.9") && said.contains("4.1") && said.contains("0.16.10.1-41000") && said.contains("4.2.0") && said.contains("D:\\SPT"));
    }
}
