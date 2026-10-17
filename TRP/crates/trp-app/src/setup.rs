//! The first start's questions, asked in the console the app runs in:
//! where the user's games are (`docs/manual.md`, "The first start"). Two at
//! most, each offering for Enter the folder kept from before or found in
//! the usual places: SPT, which is needed, and S.T.A.L.K.E.R. (GAMMA's Mod
//! Organizer folder or Anomaly's own), which is not. A third comes only
//! where GAMMA's Mod Organizer names no Anomaly folder. The answers are
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

/// S.T.A.L.K.E.R.'s folders as one answer gives them.
#[derive(Debug, Clone, Default, PartialEq)]
struct Stalker {
    /// GAMMA's Mod Organizer folder.
    gamma: Option<PathBuf>,
    /// Anomaly's own folder: the one GAMMA's Mod Organizer names, or the
    /// one given.
    anomaly: Option<PathBuf>,
}

/// What a folder given for S.T.A.L.K.E.R. is: GAMMA's Mod Organizer folder
/// (its `ModOrganizer.ini`), with the Anomaly it names where that one has
/// its `fsgame.ltx`; or Anomaly's own folder.
fn stalker_at(folder: &Path) -> std::result::Result<Stalker, String> {
    if folder.join("ModOrganizer.ini").is_file() {
        let anomaly = trp_data::xray::Stalker::organizer_game(folder).filter(|game| game.join("fsgame.ltx").is_file());
        return Ok(Stalker { gamma: Some(folder.to_path_buf()), anomaly });
    }
    if folder.join("fsgame.ltx").is_file() {
        return Ok(Stalker { gamma: None, anomaly: Some(folder.to_path_buf()) });
    }
    match folder.is_dir() {
        false => Err(format!("{} is not a folder.", folder.display())),
        true => Err(format!("{} is neither GAMMA's folder (it has no ModOrganizer.ini) nor Anomaly's (no fsgame.ltx).", folder.display())),
    }
}

/// The drives there are, `C:\` to `Z:\`.
fn drives() -> impl Iterator<Item = PathBuf> {
    (b'C'..=b'Z').map(|letter| PathBuf::from(format!("{}:\\", letter as char))).filter(|root| root.is_dir())
}

/// The first of the usual folders, on any drive, that `fits`.
fn found(names: &[&str], fits: impl Fn(&Path) -> bool) -> Option<PathBuf> {
    drives().flat_map(|drive| names.iter().map(move |name| drive.join(name)).collect::<Vec<_>>()).find(|folder| fits(folder))
}

/// Asks where the games are and writes the answers to `local`: Single
/// Player Tushonka, which is needed, and S.T.A.L.K.E.R., which is not.
/// What `local` holds already, else what is found in the usual folders,
/// is each question's answer for Enter.
pub fn run(input: &mut dyn BufRead) -> Result<()> {
    let local = local_folder();
    let kept = |file: &str| read_hint(&local.join(file)).map(PathBuf::from);
    println!();
    println!("Where are your games?");
    println!();
    let spt_offered = kept("eft_dir.txt").or_else(|| found(&["SPT", "SPTarkov", "Games\\SPT", "Games\\SPTarkov"], |folder| trp_data::Install::check(folder).is_ok()));
    let spt = ask(input, "1. Single Player Tushonka (SPT), needed: the folder with EscapeFromTarkov.exe and SPT_Runtime.", spt_offered.as_deref(), false, &|folder| {
        trp_data::Install::check(folder).map(|_| folder.to_path_buf()).map_err(|e| format!("{e:#}"))
    })
    .ok_or_else(|| anyhow!("no folder was given for Single Player Tushonka"))?;
    println!();
    let stalker_offered = kept("gamma_dir.txt")
        .or_else(|| kept("anomaly_dir.txt"))
        .or_else(|| found(&["GAMMA", "Games\\GAMMA", "Anomaly", "Games\\Anomaly"], |folder| stalker_at(folder).is_ok()));
    let mut stalker = ask(input, "2. S.T.A.L.K.E.R. Anomaly or GAMMA, optional (for the S.T.A.L.K.E.R. suits): GAMMA's folder, with ModOrganizer.exe, or Anomaly's.", stalker_offered.as_deref(), true, &stalker_at)
        .unwrap_or_default();
    if stalker.gamma.is_some() && stalker.anomaly.is_none() {
        println!();
        println!("  GAMMA's Mod Organizer names no Anomaly folder with fsgame.ltx in it.");
        stalker.anomaly = ask(input, "   Anomaly's own folder, the one with fsgame.ltx.", kept("anomaly_dir.txt").as_deref(), true, &has("fsgame.ltx", "Anomaly's folder"));
    }
    std::fs::create_dir_all(&local).with_context(|| local.display().to_string())?;
    for (file, folder) in [("eft_dir.txt", Some(spt)), ("gamma_dir.txt", stalker.gamma), ("anomaly_dir.txt", stalker.anomaly)] {
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

    /// One answer for S.T.A.L.K.E.R.: GAMMA's folder brings the Anomaly its
    /// Mod Organizer names (every `\` doubled, as Mod Organizer writes it),
    /// where that has its `fsgame.ltx`; Anomaly's folder is Anomaly alone;
    /// any other folder is neither.
    #[test]
    fn gamma_or_anomaly_is_one_answer() {
        let anomaly = folder_with("anomaly", &[("fsgame.ltx", "")]);
        let named = anomaly.display().to_string().replace('\\', "\\\\");
        let gamma = folder_with("gamma", &[("ModOrganizer.ini", &format!("[General]\ngamePath=@ByteArray({named})\n"))]);
        let lost = folder_with("gamma-lost", &[("ModOrganizer.ini", "[General]\ngamePath=@ByteArray(C:\\\\not\\\\there)\n")]);
        let neither = folder_with("neither", &[]);
        assert_eq!(stalker_at(&gamma), Ok(Stalker { gamma: Some(gamma.clone()), anomaly: Some(anomaly.clone()) }));
        assert_eq!(stalker_at(&anomaly), Ok(Stalker { gamma: None, anomaly: Some(anomaly.clone()) }));
        assert_eq!(stalker_at(&lost), Ok(Stalker { gamma: Some(lost.clone()), anomaly: None }));
        assert!(stalker_at(&neither).is_err());
        for dir in [anomaly, gamma, lost, neither] {
            let _ = std::fs::remove_dir_all(&dir);
        }
    }
}
