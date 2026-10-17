//! The first start's questions, asked in the console the app runs in:
//! where the user's games are (`docs/manual.md`, "The first start"). One
//! question for each folder, each offering for Enter only the folder kept
//! from before: none is looked for on the drives or worked out from another
//! (the user, 2026-10-17: "no dont do automatic search!", and "make sure the
//! installer asks where each of the folders are"). SPT's is needed; GAMMA's
//! Mod Organizer folder and Anomaly's own are not. The answers are
//! written to the workspace's `local` folder as a person would write them
//! by hand, one plain file a folder: `eft_dir.txt` for Single Player
//! Tushonka (SPT), `gamma_dir.txt` and `anomaly_dir.txt` for S.T.A.L.K.E.R.
//! Of the games, nothing is read here but enough to tell that each folder
//! is the one asked for.

use std::io::{BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use trp_data::install::{clean_path, read_hint};

/// Whether to ask: always with `--setup` or `--setup-only`; otherwise when
/// no install is named anywhere (or the one named is not there), someone
/// is at the console to answer, and the run is not a capture. A folder
/// given by `EFT_DIR` is the user's own word, and is not asked over.
pub fn wanted(args: &[String]) -> bool {
    if args.iter().any(|arg| arg == "--setup" || arg == "--setup-only") {
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
/// Enter takes `offered` where there is one, and is no folder at all for
/// a question that may go unanswered (`optional`), as `-` is; the end of
/// the input takes what is offered too.
fn ask<T>(input: &mut dyn BufRead, question: &str, offered: Option<&Path>, optional: bool, check: &dyn Fn(&Path) -> std::result::Result<T, String>) -> Option<T> {
    println!("{question}");
    let how = match (offered, optional) {
        (Some(offered), true) => format!("Press Enter for {}, type - to skip, or paste another folder.", offered.display()),
        (Some(offered), false) => format!("Press Enter for {}, or paste another folder.", offered.display()),
        (None, true) => "Paste its folder, or press Enter to skip.".to_owned(),
        (None, false) => "Paste its folder here.".to_owned(),
    };
    println!("  {how}");
    loop {
        print!("> ");
        let _ = std::io::stdout().flush();
        let mut line = String::new();
        if input.read_line(&mut line).unwrap_or(0) == 0 {
            println!();
            return offered.and_then(|offered| check(offered).ok());
        }
        let answer = clean_path(&line);
        let chosen = match (answer.as_str(), offered) {
            ("-", _) if optional => return None,
            ("", Some(offered)) => offered.to_path_buf(),
            ("", None) if optional => return None,
            ("", None) => {
                println!("  A folder is needed here.");
                continue;
            }
            (folder, _) => PathBuf::from(folder),
        };
        match check(&chosen) {
            Ok(taken) => return Some(taken),
            Err(why) => println!("  {why}"),
        }
    }
}

/// A folder that has a file of that name in it, or a word on why not.
fn has(file: &'static str, what: &'static str) -> impl Fn(&Path) -> std::result::Result<PathBuf, String> {
    move |folder: &Path| match (folder.is_dir(), folder.join(file).is_file()) {
        (false, _) => Err(format!("{} is not a folder.", folder.display())),
        (true, false) => Err(format!("{} has no {file}: not {what}.", folder.display())),
        (true, true) => Ok(folder.to_path_buf()),
    }
}

/// Asks where each game's folder is and writes the answers to `local`:
/// Single Player Tushonka's, which is needed, then GAMMA's and Anomaly's,
/// which are not. What `local` holds already is each question's answer for
/// Enter.
pub fn run(input: &mut dyn BufRead) -> Result<()> {
    let local = local_folder();
    let kept = |file: &str| read_hint(&local.join(file)).map(PathBuf::from);
    println!();
    println!("Where are your games?");
    println!();
    let spt = ask(input, "1. Single Player Tushonka (SPT), needed: the folder with EscapeFromTarkov.exe and SPT_Runtime.", kept("eft_dir.txt").as_deref(), false, &|folder| {
        trp_data::Install::check(folder).map(|_| folder.to_path_buf()).map_err(|e| format!("{e:#}"))
    })
    .ok_or_else(|| anyhow!("no folder was given for Single Player Tushonka"))?;
    println!();
    println!("S.T.A.L.K.E.R., optional (for its suits): either may be skipped.");
    println!();
    let gamma = ask(input, "2. GAMMA: the folder with ModOrganizer.exe and ModOrganizer.ini.", kept("gamma_dir.txt").as_deref(), true, &has("ModOrganizer.ini", "GAMMA's folder"));
    println!();
    let anomaly = ask(input, "3. S.T.A.L.K.E.R. Anomaly: the folder with fsgame.ltx.", kept("anomaly_dir.txt").as_deref(), true, &has("fsgame.ltx", "Anomaly's folder"));
    std::fs::create_dir_all(&local).with_context(|| local.display().to_string())?;
    for (file, folder) in [("eft_dir.txt", Some(spt)), ("gamma_dir.txt", gamma), ("anomaly_dir.txt", anomaly)] {
        let path = local.join(file);
        match folder {
            Some(folder) => std::fs::write(&path, folder.display().to_string()).with_context(|| path.display().to_string())?,
            None if path.is_file() => std::fs::remove_file(&path).with_context(|| path.display().to_string())?,
            None => {}
        }
    }
    println!();
    println!("Saved in {}. To change them: run Install.bat again, or start with --setup.", local.display());
    println!();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder_with(name: &str, files: &[(&str, &str)]) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("trp-setup-test-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for (file, text) in files {
            std::fs::write(dir.join(file), text).unwrap();
        }
        dir
    }

    /// Asked again after a folder that is not one; a pasted folder in
    /// quotes taken; Enter on an optional question with nothing offered is
    /// no folder, `-` is none even where one is offered.
    #[test]
    fn a_question_is_asked_until_it_has_a_folder() {
        let good = folder_with("good", &[("ModOrganizer.ini", "")]);
        let check = has("ModOrganizer.ini", "a Mod Organizer folder");
        let typed = format!("C:\\not\\there\n\"{}\"\n", good.display());
        assert_eq!(ask(&mut typed.as_bytes(), "?", None, true, &check), Some(good.clone()));
        assert_eq!(ask(&mut "\n".as_bytes(), "?", None, true, &check), None);
        assert_eq!(ask(&mut "\n".as_bytes(), "?", Some(&good), true, &check), Some(good.clone()));
        assert_eq!(ask(&mut "-\n".as_bytes(), "?", Some(&good), true, &check), None);
        // A question that needs an answer is asked again for an empty one.
        let typed = format!("\n{}\n", good.display());
        assert_eq!(ask(&mut typed.as_bytes(), "?", None, false, &check), Some(good.clone()));
        // The input ended: what was offered stays.
        assert_eq!(ask(&mut "".as_bytes(), "?", Some(&good), true, &check), Some(good.clone()));
        let _ = std::fs::remove_dir_all(&good);
    }

    /// Each game's question takes that game's folder alone: GAMMA's names
    /// an Anomaly (every `\` doubled, as Mod Organizer writes it), and still
    /// Anomaly's question is asked and answered for itself; a GAMMA folder
    /// given for Anomaly is refused, and asked again.
    #[test]
    fn each_folder_is_asked_for_itself() {
        let anomaly = folder_with("anomaly", &[("fsgame.ltx", "")]);
        let named = anomaly.display().to_string().replace('\\', "\\\\");
        let gamma = folder_with("gamma", &[("ModOrganizer.ini", &format!("[General]\ngamePath=@ByteArray({named})\n"))]);
        let gamma_check = has("ModOrganizer.ini", "GAMMA's folder");
        let anomaly_check = has("fsgame.ltx", "Anomaly's folder");
        assert_eq!(ask(&mut format!("{}\n", gamma.display()).as_bytes(), "?", None, true, &gamma_check), Some(gamma.clone()));
        // Nothing offered for Anomaly though GAMMA names one: Enter skips it.
        assert_eq!(ask(&mut "\n".as_bytes(), "?", None, true, &anomaly_check), None);
        let typed = format!("{}\n{}\n", gamma.display(), anomaly.display());
        assert_eq!(ask(&mut typed.as_bytes(), "?", None, true, &anomaly_check), Some(anomaly.clone()));
        for dir in [anomaly, gamma] {
            let _ = std::fs::remove_dir_all(&dir);
        }
    }
}
