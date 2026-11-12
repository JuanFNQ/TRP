//! What SPT's server fills a loot container with.
//! After the inventory spec, "Searching a container", "The range's crates": a location's table of a
//! container's loot (`database/locations/<location>/staticLoot.json`, by the
//! container's template: how many things, `itemcountDistribution`, and
//! which, `itemDistribution`, each by its `relativeProbability`), the things
//! never put in loot (`configs/item.json`, `lootableItemBlacklist`) and the
//! most of a kind a location holds (`configs/location.json`,
//! `lootMaxSpawnLimits`). Read from the player's install; the server's own
//! rules for using them (`LocationLootGenerator`) are SPT's published
//! source, read for the spec.

use std::collections::HashMap;

use anyhow::{Context, Result};
use serde_json::Value as Json;

use crate::setup::install::Install;

/// A container's loot table: how many things, and which, each with its
/// relative weight.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LootTable {
    pub counts: Vec<(u32, f64)>,
    pub items: Vec<(String, f64)>,
}

/// The tables of a location (`bigmap` for Customs), by container template.
pub fn static_loot(install: &Install, location: &str) -> Result<HashMap<String, LootTable>> {
    let path = install.database_dir().join("locations").join(location).join("staticLoot.json");
    let text = std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let all: Json = serde_json::from_str(&text).with_context(|| format!("{}", path.display()))?;
    let Json::Object(all) = all else { anyhow::bail!("{} is no object", path.display()) };
    let weight = |entry: &Json| entry.get("relativeProbability").and_then(Json::as_f64).unwrap_or(0.0);
    let list = |table: &Json, name: &str| table.get(name).and_then(Json::as_array).cloned().unwrap_or_default();
    Ok(all
        .into_iter()
        .map(|(template, table)| {
            let counts = list(&table, "itemcountDistribution").iter().filter_map(|entry| Some((entry.get("count")?.as_u64()? as u32, weight(entry)))).collect();
            let items = list(&table, "itemDistribution").iter().filter_map(|entry| Some((entry.get("tpl")?.as_str()?.to_owned(), weight(entry)))).collect();
            (template, LootTable { counts, items })
        })
        .collect())
}

/// SPT's configuration file of that name (`SPT_Data/configs`), for what
/// else its server decides (`repair.json`'s buffs: `trp_sim::repair`).
pub fn spt_config(install: &Install, name: &str) -> Result<Json> {
    config(install, name)
}

/// SPT's configuration file of that name (`SPT_Data/configs`).
fn config(install: &Install, name: &str) -> Result<Json> {
    let path = install.database_dir().with_file_name("configs").join(name);
    let text = std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("{}", path.display()))
}

/// The things never put in loot (`item.json`, `lootableItemBlacklist`:
/// `ItemFilterService.IsLootableItemBlacklisted`).
pub fn lootable_blacklist(install: &Install) -> Result<Vec<String>> {
    let item = config(install, "item.json")?;
    Ok(item.get("lootableItemBlacklist").and_then(Json::as_array).map(|list| list.iter().filter_map(|t| t.as_str().map(str::to_owned)).collect()).unwrap_or_default())
}

/// The things SPT never hands out (`item.json`, `blacklist`:
/// `ItemFilterService.IsItemBlacklisted`, which its traders, flea market,
/// bots and rewards go by): the game's unfinished items (the GK-01, the
/// Sparrow 22, the NXS 2.5, the infected's arms), its shrapnel, its test
/// items and the like.
pub fn item_blacklist(install: &Install) -> Result<Vec<String>> {
    let item = config(install, "item.json")?;
    Ok(item.get("blacklist").and_then(Json::as_array).map(|list| list.iter().filter_map(|t| t.as_str().map(str::to_owned)).collect()).unwrap_or_default())
}

/// The most of a kind a location holds (`location.json`,
/// `lootMaxSpawnLimits`), by template.
pub fn spawn_limits(install: &Install, location: &str) -> Result<HashMap<String, u32>> {
    let config = config(install, "location.json")?;
    let limits = config.get("lootMaxSpawnLimits").and_then(|all| all.get(location)).and_then(Json::as_object);
    Ok(limits.map(|limits| limits.iter().filter_map(|(template, most)| Some((template.clone(), most.as_u64()? as u32))).collect()).unwrap_or_default())
}

/// A location's multiplier of the count drawn (`location.json`,
/// `staticLootMultiplier`, the location's or else `default`'s; 1 where
/// neither is had).
pub fn static_multiplier(install: &Install, location: &str) -> Result<f64> {
    let config = config(install, "location.json")?;
    let all = config.get("staticLootMultiplier");
    Ok(all.and_then(|all| all.get(location).or_else(|| all.get("default"))).and_then(Json::as_f64).unwrap_or(1.0))
}

/// How many times a thing may fail to fit before a container is given no
/// more (`location.json`, `fitLootIntoContainerAttempts`; 3 in the
/// install).
pub fn fit_attempts(install: &Install) -> Result<u32> {
    let config = config(install, "location.json")?;
    Ok(config.get("fitLootIntoContainerAttempts").and_then(Json::as_u64).unwrap_or(3) as u32)
}

/// The volume of the player's own search sound in first person
/// (`ClientAudioSettings.PlayerAudioSettings.SearchSoundVolume.FpVolume`,
/// the backend's `settings.json` `config.AudioSettings.PlayerSettings`;
/// 0.6 in the install), from the database's `client`; the class's own 1
/// where it is not had.
pub fn search_sound_volume(client: &Json) -> f32 {
    client.pointer("/AudioSettings/PlayerSettings/SearchSoundVolume/FpVolume").and_then(Json::as_f64).unwrap_or(1.0) as f32
}
