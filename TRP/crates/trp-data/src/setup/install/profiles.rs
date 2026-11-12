//! The profile's skill levels, name and voices.

use super::*;

impl Install {
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

    /// The newest profile's PMC as the OVERALL tab names it
    /// (`InventoryPlayerModelWithStatsWindow.Show`): its nickname
    /// (`Info.Nickname`) and the member category its mark and the name's
    /// colour go by (`Info.SelectedMemberCategory`, none for a Scav). None
    /// where there is no profile; read, never written.
    pub fn profile_name(&self) -> Option<(String, u32)> {
        let profile = self.newest_profile()?;
        let info = &profile["characters"]["pmc"]["Info"];
        let nickname = info["Nickname"].as_str()?.to_owned();
        let category = if info["Side"].as_str() == Some("Savage") { 0 } else { info["SelectedMemberCategory"].as_u64().unwrap_or(0) as u32 };
        Some((nickname, category))
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

    /// The PMC voices the newest profile has: of the customisations it has
    /// (the server's own for every profile, `templates/customisationStorage.json`,
    /// and the profile's `customisationUnlocks`: SPT's
    /// `GetCustomisationStorage`), those of the type `voice` whose voice
    /// (`customization.json`) is a BEAR's or a USEC's (its `Side`), ordered
    /// by `_props.Name` as the game's list orders them (`VoiceSelector.
    /// Initialize`). The game's list holds the profile's side's alone
    /// (`CustomizationSolver.GetAvailableVoices`); the app's options both,
    /// each marked with its side, so that each one's faction shows.
    /// Each its id (the game shows the words of `"<id> Name"`), its name
    /// (`Bear_1`), by which its phrases are found, and its side. Empty
    /// where any of it is missing.
    pub fn profile_voices(&self) -> Vec<VoiceChoice> {
        let read = |path: PathBuf| -> Option<serde_json::Value> {
            let text = std::fs::read_to_string(path).ok()?;
            serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()
        };
        let templates = self.database_dir().join("templates");
        let (Some(profile), Some(all), Some(everyone)) = (self.newest_profile(), read(templates.join("customization.json")), read(templates.join("customisationStorage.json"))) else { return Vec::new() };
        let own = profile["customisationUnlocks"].as_array().cloned().unwrap_or_default();
        let had = everyone.as_array().into_iter().flatten().chain(&own).filter(|entry| entry["type"].as_str() == Some("voice")).filter_map(|entry| entry["id"].as_str());
        let mut voices: Vec<VoiceChoice> = Vec::new();
        for id in had {
            let props = &all[id]["_props"];
            let side = props["Side"].as_array().into_iter().flatten().filter_map(|s| s.as_str()).find(|s| matches!(*s, "Bear" | "Usec"));
            if let (Some(side), Some(name)) = (side, props["Name"].as_str()) {
                if !voices.iter().any(|voice| voice.id == id) {
                    voices.push(VoiceChoice { id: id.to_owned(), name: name.to_owned(), usec: side == "Usec" });
                }
            }
        }
        voices.sort_by(|a, b| a.name.cmp(&b.name));
        voices
    }
}

/// A voice the character may be given (`CustomizationPlayerVoice`): its id
/// in `customization.json`, its name (`Bear_1`) and whether it is a USEC's
/// (else a BEAR's).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoiceChoice {
    pub id: String,
    pub name: String,
    pub usec: bool,
}
