//! Locating the install and its bundle manifest.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::Deserialize;

mod profiles;
mod game_settings;

pub use profiles::*;
pub use game_settings::*;

/// What a file of `local` that names a folder says (`spt_dir.txt`,
/// `gamma_dir.txt`, `anomaly_dir.txt`): its first line that is not empty,
/// as `clean_path` has it, whichever way the file was saved: plain, with
/// the mark UTF-8 may begin with, or as UTF-16 (which Windows PowerShell's
/// `>` writes). None where it cannot be read or names nothing.
pub fn read_hint(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    let wide = |rest: &[u8], big: bool| {
        let units: Vec<u16> = rest.chunks_exact(2).map(|pair| if big { u16::from_be_bytes([pair[0], pair[1]]) } else { u16::from_le_bytes([pair[0], pair[1]]) }).collect();
        String::from_utf16_lossy(&units)
    };
    let text = match bytes.as_slice() {
        [0xFF, 0xFE, rest @ ..] => wide(rest, false),
        [0xFE, 0xFF, rest @ ..] => wide(rest, true),
        [0xEF, 0xBB, 0xBF, rest @ ..] => String::from_utf8_lossy(rest).into_owned(),
        all => String::from_utf8_lossy(all).into_owned(),
    };
    let named = clean_path(&text);
    (!named.is_empty()).then_some(named)
}

/// A folder as a person types or pastes it: the first line that is not
/// empty, trimmed, without the quotes Windows' "Copy as path" puts about
/// it, and without the mark PowerShell puts before what it pipes in.
pub fn clean_path(text: &str) -> String {
    let blank = |c: char| c.is_whitespace() || c == '\u{feff}';
    let line = text.lines().map(|line| line.trim_matches(blank)).find(|line| !line.is_empty()).unwrap_or("");
    line.trim_matches('"').trim_matches(blank).to_owned()
}

/// TRP's `local` folder as seen from a place: the one beside the nearest
/// `Cargo.toml` with a `crates` folder above it (the workspace: where
/// `cargo run` is, and above `target\debug` for the program started by
/// itself), made or not; with no workspace above, the first `local`
/// folder found going up, else the place's own. One rule for what is
/// written there and read back: the folders the first start asks for, the
/// options, the logs.
pub fn local_folder_from(here: &Path) -> PathBuf {
    if let Some(workspace) = here.ancestors().find(|dir| dir.join("Cargo.toml").is_file() && dir.join("crates").is_dir()) {
        return workspace.join("local");
    }
    here.ancestors().map(|dir| dir.join("local")).find(|local| local.is_dir()).unwrap_or_else(|| here.join("local"))
}

/// TRP's `local` folder from where the app runs (`local_folder_from`).
pub fn local_folder() -> PathBuf {
    local_folder_from(&std::env::current_dir().unwrap_or_default())
}

/// An install of Single Player Tushonka (SPT): the game's client with SPT
/// over it. Read-only.
#[derive(Debug, Clone)]
pub struct Install {
    pub root: PathBuf,
}

impl Install {
    /// Finds the install from `SPT_DIR`, or from `local/spt_dir.txt` in the
    /// current directory or any parent.
    pub fn locate() -> Result<Self> {
        if let Ok(dir) = std::env::var("SPT_DIR") {
            return Self::at(dir);
        }
        let mut dir = std::env::current_dir()?;
        loop {
            let hint = dir.join("local").join("spt_dir.txt");
            if hint.is_file() {
                let named = read_hint(&hint).with_context(|| format!("{} names no folder", hint.display()))?;
                return Self::at(named);
            }
            if !dir.pop() {
                break;
            }
        }
        bail!("no SPT install configured: run the app in a console to be asked for it (or with --setup), set SPT_DIR, or write the path to local/spt_dir.txt")
    }

    /// An install this can read: the game's bundles (`at`), and SPT's copy
    /// of the database beside them, which the items are read from.
    pub fn check(root: impl Into<PathBuf>) -> Result<Self> {
        let install = Self::at(root)?;
        if !install.database_dir().is_dir() {
            bail!("{} has the game but not SPT's database ({}): is SPT installed there?", install.root.display(), install.database_dir().display());
        }
        Ok(install)
    }

    pub fn at(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        let install = Self { root };
        if !install.bundles_dir().is_dir() {
            bail!("{} does not look like an SPT install (it has no EscapeFromTarkov_Data\\StreamingAssets\\Windows)", install.root.display());
        }
        Ok(install)
    }

    /// Directory the manifest keys are relative to.
    pub fn bundles_dir(&self) -> PathBuf {
        self.root
            .join("EscapeFromTarkov_Data")
            .join("StreamingAssets")
            .join("Windows")
    }

    /// SPT's copy of the server database (item templates, globals).
    pub fn database_dir(&self) -> PathBuf {
        self.root.join("SPT_Runtime").join("SPT_Data").join("database")
    }

    pub fn bundle_path(&self, key: &str) -> PathBuf {
        self.bundles_dir().join(Path::new(key))
    }

    /// Where SPT keeps the player's own settings for the game: one file
    /// of JSON per page of the game's settings menu.
    pub fn settings_dir(&self) -> PathBuf {
        self.root.join("SPT_Runtime").join("user").join("sptSettings")
    }
}

#[cfg(test)]
mod tests;
