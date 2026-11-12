//! The run's start: its version and log, the install found and the options read.

use super::*;

/// Says the version, starts the log, finds the install and reads the options; `None` where the run ends here.
pub(super) fn open() -> anyhow::Result<Option<(Install, Options)>> {
    // The version before anything else, so that whatever a run prints says
    // which build it came from (`docs\manual.md`, "Versions"); `--version`
    // prints it alone.
    println!("TRP {}", env!("TRP_VERSION"));
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--version") {
        return Ok(None);
    }
    // The run's log and a crash's report, before anything can fail.
    platform::run_log::start(&args);
    // Where the games are: asked in the console on a first start, or with
    // `--setup` (`setup`); `--setup-only` asks and ends (`Install.bat`).
    if platform::setup::wanted(&args) {
        platform::setup::run(&mut std::io::stdin().lock())?;
        if args.iter().any(|arg| arg == "--setup-only") {
            return Ok(None);
        }
    }
    let install = Install::locate()?;
    // The install's versions, before anything is read from it: another
    // release of the client or of SPT is warned of, and the run goes on.
    platform::run_log::check_install(&install);
    let settings = install.player_settings();
    let mut options = parse_options(&settings);
    let _ = FIXED_SEED.set(options.unattended().then_some(options.seed));
    options.degrees_a_count = platform::keys::degrees_a_count(&install);
    options.look_degrees_a_count = platform::keys::look_degrees_a_count(&install);
    options.invert = ["InvertedXAxis", "InvertedYAxis"].map(|name| install.control_flag(name).unwrap_or(false));
    options.optic_sensitivity = install.control_setting("OpticSensitivity").unwrap_or(1.0).clamp(1.0, 5.0);
    println!(
        "view settings: field of view {}, head bobbing {}, mouse sensitivity {} and {} with the sights up, a count of the mouse {} degrees before either{}",
        options.field_of_view,
        options.head_bobbing,
        options.sensitivity,
        options.aiming_sensitivity,
        options.degrees_a_count,
        if settings == trp_data::PlayerSettings::default() { " (the install has none of its own)" } else { "" }
    );
    platform::run_log::phase("the install found, the options read");
    Ok(Some((install, options)))
}
