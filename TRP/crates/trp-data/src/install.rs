//! Locating the install and its bundle manifest.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::Deserialize;

/// What a file of `local` that names a folder says (`eft_dir.txt`,
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

/// An install of Single Player Tushonka (SPT): the game's client with SPT
/// over it. Read-only.
#[derive(Debug, Clone)]
pub struct Install {
    pub root: PathBuf,
}

impl Install {
    /// Finds the install from `EFT_DIR`, or from `local/eft_dir.txt` in the
    /// current directory or any parent.
    pub fn locate() -> Result<Self> {
        if let Ok(dir) = std::env::var("EFT_DIR") {
            return Self::at(dir);
        }
        let mut dir = std::env::current_dir()?;
        loop {
            let hint = dir.join("local").join("eft_dir.txt");
            if hint.is_file() {
                let named = read_hint(&hint).with_context(|| format!("{} names no folder", hint.display()))?;
                return Self::at(named);
            }
            if !dir.pop() {
                break;
            }
        }
        bail!("no SPT install configured: run the app in a console to be asked for it (or with --setup), set EFT_DIR, or write the path to local/eft_dir.txt")
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

    /// One page of the game's settings as SPT keeps it (`Game.ini`,
    /// `Control.ini`, `Graphics.ini`): JSON, whatever the name says.
    /// The skills' levels of the newest profile in the install
    /// (`SPT_Runtime/user/profiles`, its PMC's `Skills.Common`), by the
    /// game's names: progress over 100, at most 51 (`BaseSkill.Level`).
    /// Read, never written; none where there is no profile.
    pub fn profile_skill_levels(&self) -> Option<std::collections::HashMap<String, u32>> {
        let profile = self.newest_profile()?;
        let skills = profile["characters"]["pmc"]["Skills"]["Common"].as_array()?;
        Some(
            skills
                .iter()
                .filter_map(|skill| {
                    let progress = skill["Progress"].as_f64()?.clamp(0.0, 5100.0);
                    Some((skill["Id"].as_str()?.to_owned(), (progress / 100.0) as u32))
                })
                .collect(),
        )
    }

    /// The newest profile of the install (`SPT_Runtime/user/profiles`):
    /// read, never written.
    fn newest_profile(&self) -> Option<serde_json::Value> {
        let dir = self.root.join("SPT_Runtime").join("user").join("profiles");
        let newest = std::fs::read_dir(dir)
            .ok()?
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.path().extension().is_some_and(|e| e == "json"))
            .max_by_key(|entry| entry.metadata().and_then(|m| m.modified()).ok())?;
        let text = std::fs::read_to_string(newest.path()).ok()?;
        serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()
    }

    /// The voice of the newest profile's PMC by its name (`Bear_2_Eng`):
    /// its `Customization.Voice`, an id of the database's
    /// `customization.json`, whose `_props.Name` it is
    /// (`CustomizationSolver.GetVoice`). None where either is missing.
    pub fn profile_voice(&self) -> Option<String> {
        let profile = self.newest_profile()?;
        let id = profile["characters"]["pmc"]["Customization"]["Voice"].as_str()?.to_owned();
        let text = std::fs::read_to_string(self.database_dir().join("templates").join("customization.json")).ok()?;
        let all: serde_json::Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()?;
        all.get(&id)?["_props"]["Name"].as_str().map(str::to_owned)
    }

    fn settings_page(&self, name: &str) -> Option<serde_json::Value> {
        let text = std::fs::read_to_string(self.settings_dir().join(name)).ok()?;
        serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()
    }

    /// The settings of the game's own menu that change what the
    /// controller shows or how it takes the mouse, as this install has
    /// them. Each is `None` where its file or its entry is missing.
    pub fn player_settings(&self) -> PlayerSettings {
        let number = |page: &Option<serde_json::Value>, key: &str| page.as_ref()?.get(key)?.as_f64().map(|v| v as f32);
        let (game, control) = (self.settings_page("Game.ini"), self.settings_page("Control.ini"));
        PlayerSettings {
            field_of_view: number(&game, "FieldOfView"),
            head_bobbing: number(&game, "HeadBobbing"),
            mouse_sensitivity: number(&control, "MouseSensitivity"),
            aiming_sensitivity: number(&control, "MouseAimingSensitivity"),
        }
    }

    /// The game's key bindings as this install has them (`Control.ini`,
    /// `keyBindings`); none if the file is missing. Each keeps both of
    /// its variants, an empty one included, in the file's order.
    pub fn key_bindings(&self) -> Vec<KeyBinding> {
        let Some(page) = self.settings_page("Control.ini") else { return Vec::new() };
        let Some(list) = page.get("keyBindings").and_then(|list| list.as_array()) else { return Vec::new() };
        let binding = |entry: &serde_json::Value| {
            let variants = entry.get("variants")?.as_array()?.iter().map(KeySource::of).collect();
            let press = entry.get("pressType").and_then(|press| press.as_str()).unwrap_or_default().to_owned();
            Some(KeyBinding { name: entry.get("keyName")?.as_str()?.to_owned(), variants, press })
        };
        list.iter().filter_map(binding).collect()
    }

    /// The game's axes as this install binds them (`Control.ini`,
    /// `axisBindings`): for each, its pairs of keys for the positive way
    /// and the negative.
    pub fn axis_bindings(&self) -> Vec<AxisBinding> {
        let Some(page) = self.settings_page("Control.ini") else { return Vec::new() };
        let Some(list) = page.get("axisBindings").and_then(|list| list.as_array()) else { return Vec::new() };
        let binding = |entry: &serde_json::Value| {
            let pair = |pair: &serde_json::Value| Some([KeySource::of(pair.get("positive")?), KeySource::of(pair.get("negative")?)]);
            let pairs = entry.get("pairs")?.as_array()?.iter().filter_map(pair).collect();
            Some(AxisBinding { name: entry.get("axisName")?.as_str()?.to_owned(), pairs })
        };
        list.iter().filter_map(binding).collect()
    }

    /// One of the game's control settings that is a figure
    /// (`Control.ini`: `DoubleClickTimeout`).
    pub fn control_setting(&self, name: &str) -> Option<f32> {
        self.settings_page("Control.ini")?.get(name)?.as_f64().map(|v| v as f32)
    }

    /// One of the game's control settings that is a switch
    /// (`Control.ini`: `InvertedXAxis`, `InvertedYAxis`).
    pub fn control_flag(&self, name: &str) -> Option<bool> {
        self.settings_page("Control.ini")?.get(name)?.as_bool()
    }

    /// What the first variant of an axis's binding multiplies its engine
    /// axis by (`InputSource.sensitivity`, of `TurnX` and its like in
    /// `Control.ini`; 5 for the mouse's four in the install read).
    pub fn axis_sensitivity(&self, axis: &str) -> Option<f32> {
        let page = self.settings_page("Control.ini")?;
        let entry = page.get("axisBindings")?.as_array()?.iter().find(|entry| entry.get("axisName").and_then(|name| name.as_str()) == Some(axis))?;
        entry.get("pairs")?.get(0)?.get("positive")?.get("sensitivity")?.as_f64().map(|v| v as f32)
    }

    /// The engine's own input axes as the build has them (the input
    /// settings in `globalgamemanagers`): what the game's bindings name
    /// when they name an axis (`Mouse X`, `Mouse Scroll`). The file has no
    /// type trees; an axis is read by the engine's fixed layout for one.
    pub fn engine_axes(&self) -> Result<Vec<EngineAxis>> {
        use crate::unity::reader::Reader;
        let path = self.root.join("EscapeFromTarkov_Data").join("globalgamemanagers");
        let data = std::fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
        let meta = crate::unity::SerializedFile::parse(&data).with_context(|| path.display().to_string())?;
        let settings = meta.objects.iter().find(|object| object.class_id == INPUT_SETTINGS).context("the build has no input settings")?;
        let mut r = Reader::new(data.get(settings.offset..settings.offset + settings.size).context("the input settings lie outside their file")?);
        let string = |r: &mut Reader| -> Result<String> {
            let len = r.u32()? as usize;
            let text = String::from_utf8_lossy(r.bytes(len)?).into_owned();
            r.align(4);
            Ok(text)
        };
        let count = r.u32()? as usize;
        let mut axes = Vec::new();
        for _ in 0..count {
            let name = string(&mut r)?;
            // Two descriptions and four buttons.
            for _ in 0..6 {
                string(&mut r)?;
            }
            let (_gravity, dead, sensitivity) = (r.f32()?, r.f32()?, r.f32()?);
            r.skip(4)?; // snap and invert, padded
            let (kind, axis, _joystick) = (r.i32()?, r.i32()?, r.i32()?);
            axes.push(EngineAxis { name, dead, sensitivity, kind, axis });
        }
        Ok(axes)
    }

    /// Whether the build draws with the engine's graphics jobs, which run
    /// its rendering on threads of its own a frame behind the main thread
    /// (`boot.config`: `gfx-enable-gfx-jobs=1`, as this install has it).
    /// `None` where the file or the line is missing.
    pub fn graphics_jobs(&self) -> Option<bool> {
        let text = std::fs::read_to_string(self.root.join("EscapeFromTarkov_Data").join("boot.config")).ok()?;
        text.lines().find_map(|line| line.trim().strip_prefix("gfx-enable-gfx-jobs=")).map(|value| value.trim() == "1")
    }

    /// The display settings of the game's graphics page as this install
    /// has them (`Graphics.ini`).
    pub fn display_settings(&self) -> DisplaySettings {
        let Some(page) = self.settings_page("Graphics.ini") else { return DisplaySettings::default() };
        let display = page.get("DisplaySettings");
        let side = |name: &str| display?.get("Resolution")?.get(name)?.as_u64().map(|v| v as u32);
        DisplaySettings {
            vsync: page.get("VSync").and_then(|v| v.as_bool()),
            mode: display.and_then(|display| display.get("FullScreenMode")?.as_i64()),
            resolution: side("Width").zip(side("Height")).map(|(width, height)| [width, height]),
            texture_quality: page.get("TextureQuality").and_then(|v| v.as_u64()).map(|v| v.min(2) as u8),
            mip_streaming: page.get("MipStreaming").and_then(|v| v.as_bool()),
            shadows_quality: page.get("ShadowsQuality").and_then(|v| v.as_u64()).map(|v| v.min(3) as u8),
            ssao: page.get("Ssao").and_then(|v| v.as_str()).and_then(|name| SSAO_CHOICES.into_iter().find(|choice| choice.eq_ignore_ascii_case(name))),
            clouds_quality: page.get("CloudsQuality").and_then(|v| v.as_str()).and_then(|name| CLOUDS_CHOICES.into_iter().find(|choice| choice.eq_ignore_ascii_case(name))),
            game_framerate: page.get("GameFramerate").and_then(|v| v.as_u64()).map(|v| v.min(u32::MAX as u64) as u32),
            disable_game_framerate_limit: page.get("DisableGameFramerateLimit").and_then(|v| v.as_bool()),
            sharpen: page.get("Sharpen").and_then(|v| v.as_f64()).map(|v| v as f32),
        }
    }

    /// One of the game's sound settings by its name (`Sound.ini`:
    /// `OverallVolume`, `InterfaceVolume`), as a number.
    pub fn sound_setting(&self, name: &str) -> Option<f32> {
        self.settings_page("Sound.ini")?.get(name)?.as_f64().map(|v| v as f32)
    }

    /// One of the game's own settings by its name, as text
    /// (`Game.ini`: `PriorityWindowMode`, `ItemQuickUseMode`).
    pub fn game_setting(&self, name: &str) -> Option<String> {
        match self.settings_page("Game.ini")?.get(name)? {
            serde_json::Value::String(text) => Some(text.clone()),
            other => Some(other.to_string()),
        }
    }

    pub fn manifest(&self) -> Result<Manifest> {
        let path = self.bundles_dir().join("Windows.json");
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        let entries: HashMap<String, ManifestEntry> = serde_json::from_str(&text)?;
        Ok(Manifest { entries })
    }
}

/// What the player has set in the game's own settings menu, of the
/// settings the controller answers to: the field of view in vertical
/// degrees (50 to 75), head bobbing (0.2 to 1: how much of the animated
/// camera bone's turning the camera takes), and the mouse's sensitivity
/// from the hip and with the sights up.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct PlayerSettings {
    pub field_of_view: Option<f32>,
    pub head_bobbing: Option<f32>,
    pub mouse_sensitivity: Option<f32>,
    pub aiming_sensitivity: Option<f32>,
}

/// The engine's class number for a build's input settings.
const INPUT_SETTINGS: i32 = 13;

/// One of the engine's input axes (`InputManager`): its name, its dead
/// zone, what a count of the device is multiplied by, its kind (0 a key or
/// a button, 1 the mouse's movement, 2 a joystick) and which of the
/// device's axes it is (for the mouse: 0 across, 1 up and down, 2 the
/// wheel).
#[derive(Debug, Clone, PartialEq)]
pub struct EngineAxis {
    pub name: String,
    pub dead: f32,
    pub sensitivity: f32,
    pub kind: i32,
    pub axis: i32,
}

#[derive(Debug, Deserialize)]
pub struct ManifestEntry {
    #[serde(rename = "Dependencies", default)]
    pub dependencies: Vec<String>,
}

/// `Windows.json`: every bundle key and the bundles it depends on.
#[derive(Debug)]
pub struct Manifest {
    pub entries: HashMap<String, ManifestEntry>,
}

impl Manifest {
    pub fn dependencies(&self, key: &str) -> &[String] {
        self.entries
            .get(key)
            .map(|e| e.dependencies.as_slice())
            .unwrap_or(&[])
    }

    pub fn contains(&self, key: &str) -> bool {
        self.entries.contains_key(key)
    }
}

/// What the game's graphics page says of the display, as the install has
/// it: whether frames wait for the screen, how the window is shown (the
/// game's own figure: 0 the screen taken over, 1 a borderless window over
/// all of it, 2 a window), and the resolution.
/// The ambient occlusion's choices by the names `Graphics.ini` writes them
/// in (`ESSAOMode`, in the game's order).
/// The clouds' quality's choices by the names `Graphics.ini` writes them
/// (`CloudsMode`).
pub const CLOUDS_CHOICES: [&str; 3] = ["Low", "Medium", "High"];

pub const SSAO_CHOICES: [&str; 6] = ["Off", "FastestPerformance", "FastPerformance", "HighQuality", "HighestQuality", "ColoredHighestQuality"];

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct DisplaySettings {
    pub vsync: Option<bool>,
    pub mode: Option<i64>,
    pub resolution: Option<[u32; 2]>,
    /// The pictures' quality, 0 (low) to 2 (high), and whether they are
    /// streamed by their levels of detail (`TextureQuality`, `MipStreaming`).
    pub texture_quality: Option<u8>,
    pub mip_streaming: Option<bool>,
    /// The shadows' quality, 0 (low) to 3 (ultra) (`ShadowsQuality`).
    pub shadows_quality: Option<u8>,
    /// The ambient occlusion's choice by its name (`Ssao`:
    /// `FastestPerformance` and the like; `trp_sim::hbao::SsaoMode`).
    pub ssao: Option<&'static str>,
    /// The clouds' quality by its name (`CloudsQuality`: `Low`, `Medium`,
    /// `High`; `trp_sim::clouds::CloudsQuality`).
    pub clouds_quality: Option<&'static str>,
    /// The game's frame rate limit in a raid (`GameFramerate`), and whether
    /// it is turned off (`DisableGameFramerateLimit`).
    pub game_framerate: Option<u32>,
    pub disable_game_framerate_limit: Option<bool>,
    /// The camera's sharpening (`Sharpen`, `CameraManager.SetSharpen`).
    pub sharpen: Option<f32>,
}

/// One variant of a binding as the install keeps it (`InputSource`): the
/// keys held together, by the engine's names for them (`Tab`, `Alpha2`,
/// `LeftAlt`, `Mouse0`), and the axis it ends in if it has one, with its
/// way (`Mouse Scroll`, up when true).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct KeySource {
    pub keys: Vec<String>,
    pub axis: Option<(String, bool)>,
}

impl KeySource {
    fn of(source: &serde_json::Value) -> Self {
        let codes = source.get("keyCode").and_then(|codes| codes.as_array());
        let keys = codes.map(|codes| codes.iter().filter_map(|code| code.as_str().map(str::to_owned)).collect()).unwrap_or_default();
        let positive = source.get("positiveAxis").and_then(|way| way.as_bool()).unwrap_or(false);
        let axis = source.get("axisName").and_then(|name| name.as_str()).filter(|name| !name.is_empty()).map(|name| (name.to_owned(), positive));
        Self { keys, axis }
    }

    pub fn is_empty(&self) -> bool {
        self.keys.is_empty() && self.axis.is_none()
    }
}

/// One of the game's key bindings as the install keeps it: the game's own
/// name for what the key does (`EGameKey`: `Inventory`,
/// `PrimaryWeaponFirst`, `Slot4`), its variants, and how it is pressed
/// (`Press`, `Continuous`, `Release`, `DoubleClick`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyBinding {
    pub name: String,
    pub variants: Vec<KeySource>,
    pub press: String,
}

/// One of the game's axes as the install binds it (`AxisGroup`: `MoveX`,
/// `MoveY`, `LeanX` and the mouse's four): its pairs, each the positive
/// way's keys and the negative's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AxisBinding {
    pub name: String,
    pub pairs: Vec<[KeySource; 2]>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A file naming a folder is read however it was saved: by an editor,
    /// with the mark UTF-8 may begin with, by Windows PowerShell's `>` (UTF-16
    /// with its mark), with the quotes of "Copy as path", with lines after.
    #[test]
    fn a_hint_is_read_however_it_was_saved() {
        let dir = std::env::temp_dir().join(format!("trp-hint-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("eft_dir.txt");
        let wide = |text: &str, big: bool| -> Vec<u8> {
            let mut bytes = if big { vec![0xFE, 0xFF] } else { vec![0xFF, 0xFE] };
            for unit in text.encode_utf16() {
                bytes.extend(if big { unit.to_be_bytes() } else { unit.to_le_bytes() });
            }
            bytes
        };
        let saved: Vec<(&str, Vec<u8>)> = vec![
            ("plain", b"D:\\SPT".to_vec()),
            ("a line's end and spaces", b"  D:\\SPT  \r\n".to_vec()),
            ("UTF-8 with its mark", [&[0xEF, 0xBB, 0xBF][..], b"D:\\SPT\r\n"].concat()),
            ("UTF-16, as PowerShell 5.1's >", wide("D:\\SPT\r\n", false)),
            ("UTF-16 the other way about", wide("D:\\SPT", true)),
            ("quoted", b"\"D:\\SPT\"".to_vec()),
            ("an empty line first", b"\r\n\r\nD:\\SPT\r\nsomething else".to_vec()),
        ];
        for (how, bytes) in saved {
            std::fs::write(&file, bytes).unwrap();
            assert_eq!(read_hint(&file).as_deref(), Some("D:\\SPT"), "{how}");
        }
        std::fs::write(&file, b"  \r\n").unwrap();
        assert_eq!(read_hint(&file), None, "a file naming nothing");
        assert_eq!(read_hint(&dir.join("not there.txt")), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_pasted_folder_is_cleaned() {
        assert_eq!(clean_path("  \"E:\\Games\\SPT\"  \n"), "E:\\Games\\SPT");
        assert_eq!(clean_path("E:\\Games\\S P T"), "E:\\Games\\S P T");
        assert_eq!(clean_path("\u{feff}E:\\SPT\r\n"), "E:\\SPT");
        assert_eq!(clean_path(""), "");
    }
}
