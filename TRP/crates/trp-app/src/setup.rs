//! The first start's questions, asked in the console the app runs in:
//! where the user's games are (`docs/manual.md`, "The first start"). The
//! answers are written to the workspace's `local` folder as a person would
//! write them by hand, one plain file a folder: `eft_dir.txt` for Single
//! Player Tushonka (SPT), `gamma_dir.txt` and `anomaly_dir.txt` for S.T.A.L.K.E.R. Of
//! the games, nothing is read here but enough to tell that each folder is
//! the one asked for.

use std::io::{BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use trp_data::install::{clean_path, read_hint};

/// Whether to ask: always with `--setup`; otherwise when no install is
/// named anywhere (or the one named is not there), someone is at the
/// console to answer, and the run is not a capture. A folder given by
/// `EFT_DIR` is the user's own word, and is not asked over.
pub fn wanted(args: &[String]) -> bool {
    if args.iter().any(|arg| arg == "--setup") {
        return true;
    }
    let capture = args.iter().any(|arg| arg == "--screenshot" || arg == "--scenario");
    std::env::var_os("EFT_DIR").is_none() && !capture && std::io::stdin().is_terminal() && trp_data::Install::locate().is_err()
}

/// The workspace's `local` folder: beside the nearest `Cargo.toml` with a
/// `crates` folder, from where the app runs upwards (where `cargo run` is,
/// and above `target\debug` for the program started by itself); else the
/// current folder's. The files there are found from either.
pub fn local_folder() -> PathBuf {
    let here = std::env::current_dir().unwrap_or_default();
    here.ancestors().find(|dir| dir.join("Cargo.toml").is_file() && dir.join("crates").is_dir()).unwrap_or(&here).join("local")
}

/// One question, asked until it is answered with a folder `check` takes.
/// Enter takes `default` where there is one, and is no folder at all for
/// a question that may go unanswered (`optional`), as `-` is; the end of
/// the input takes the default too.
fn ask(input: &mut dyn BufRead, question: &str, default: Option<&Path>, optional: bool, check: &dyn Fn(&Path) -> std::result::Result<(), String>) -> Option<PathBuf> {
    loop {
        let how = match (default, optional) {
            (Some(default), true) => format!(" [Enter: {}; -: none]", default.display()),
            (Some(default), false) => format!(" [Enter: {}]", default.display()),
            (None, true) => " [Enter: none]".to_owned(),
            (None, false) => String::new(),
        };
        print!("{question}{how}\n> ");
        let _ = std::io::stdout().flush();
        let mut line = String::new();
        if input.read_line(&mut line).unwrap_or(0) == 0 {
            println!();
            return default.map(Path::to_path_buf);
        }
        let answer = clean_path(&line);
        let chosen = match (answer.as_str(), default) {
            ("-", _) if optional => return None,
            ("", Some(default)) => default.to_path_buf(),
            ("", None) if optional => return None,
            ("", None) => {
                println!("  A folder is needed here.");
                continue;
            }
            (folder, _) => PathBuf::from(folder),
        };
        match check(&chosen) {
            Ok(()) => return Some(chosen),
            Err(why) => println!("  {why}"),
        }
    }
}

/// A folder that has a file of that name in it, or a word on why not.
fn has(file: &'static str, what: &'static str) -> impl Fn(&Path) -> std::result::Result<(), String> {
    move |folder: &Path| match (folder.is_dir(), folder.join(file).is_file()) {
        (false, _) => Err(format!("{} is not a folder.", folder.display())),
        (true, false) => Err(format!("{} has no {file}: not {what}.", folder.display())),
        (true, true) => Ok(()),
    }
}

/// Asks where the games are and writes the answers to `local`: Single
/// Player Tushonka, which is needed, and S.T.A.L.K.E.R., which is not. What `local`
/// holds already is each question's answer for Enter.
pub fn run(input: &mut dyn BufRead) -> Result<()> {
    let local = local_folder();
    let kept = |file: &str| read_hint(&local.join(file)).map(PathBuf::from);
    println!();
    println!("Where are your games? (The answers are kept in {}; start with --setup to change them.)", local.display());
    println!();
    let spt = ask(input, "Single Player Tushonka (SPT): its folder, the one with SPT_Runtime and the game's EscapeFromTarkov.exe", kept("eft_dir.txt").as_deref(), false, &|folder| {
        trp_data::Install::check(folder).map(|_| ()).map_err(|e| format!("{e:#}"))
    })
    .ok_or_else(|| anyhow!("no folder was given for Single Player Tushonka"))?;
    println!();
    println!("Another game to take things from, if you have one. For now that is S.T.A.L.K.E.R. Anomaly, alone or under GAMMA (its suits).");
    let gamma = ask(input, "GAMMA: its Mod Organizer folder (the one with ModOrganizer.ini)", kept("gamma_dir.txt").as_deref(), true, &has("ModOrganizer.ini", "a Mod Organizer folder"));
    // Mod Organizer names the game it lies over.
    let anomaly_known = kept("anomaly_dir.txt").or_else(|| gamma.as_deref().and_then(trp_data::xray::Stalker::organizer_game).filter(|folder| folder.join("fsgame.ltx").is_file()));
    let anomaly = ask(input, "Anomaly: its own folder (the one with fsgame.ltx)", anomaly_known.as_deref(), true, &has("fsgame.ltx", "Anomaly's folder"));
    std::fs::create_dir_all(&local).with_context(|| local.display().to_string())?;
    let mut kept_now = Vec::new();
    for (file, folder) in [("eft_dir.txt", Some(spt)), ("gamma_dir.txt", gamma), ("anomaly_dir.txt", anomaly)] {
        let path = local.join(file);
        match folder {
            Some(folder) => {
                std::fs::write(&path, folder.display().to_string()).with_context(|| path.display().to_string())?;
                kept_now.push(format!("{file}: {}", folder.display()));
            }
            None if path.is_file() => std::fs::remove_file(&path).with_context(|| path.display().to_string())?,
            None => {}
        }
    }
    println!();
    println!("Kept in {}: {}.", local.display(), kept_now.join("; "));
    println!();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder_with(name: &str, files: &[&str]) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("trp-setup-test-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for file in files {
            std::fs::write(dir.join(file), b"").unwrap();
        }
        dir
    }

    /// Asked again after a folder that is not one; a pasted folder in
    /// quotes taken; Enter on an optional question with nothing kept is no
    /// folder, `-` is none even where one is kept.
    #[test]
    fn a_question_is_asked_until_it_has_a_folder() {
        let good = folder_with("good", &["ModOrganizer.ini"]);
        let check = has("ModOrganizer.ini", "a Mod Organizer folder");
        let typed = format!("C:\\not\\there\n\"{}\"\n", good.display());
        assert_eq!(ask(&mut typed.as_bytes(), "?", None, true, &check), Some(good.clone()));
        assert_eq!(ask(&mut "\n".as_bytes(), "?", None, true, &check), None);
        assert_eq!(ask(&mut "\n".as_bytes(), "?", Some(&good), true, &check), Some(good.clone()));
        assert_eq!(ask(&mut "-\n".as_bytes(), "?", Some(&good), true, &check), None);
        // A question that needs an answer is asked again for an empty one.
        let typed = format!("\n{}\n", good.display());
        assert_eq!(ask(&mut typed.as_bytes(), "?", None, false, &check), Some(good.clone()));
        // The input ended: what was kept stays.
        assert_eq!(ask(&mut "".as_bytes(), "?", Some(&good), true, &check), Some(good.clone()));
        let _ = std::fs::remove_dir_all(&good);
    }
}
