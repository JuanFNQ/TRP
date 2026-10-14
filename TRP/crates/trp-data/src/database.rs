//! SPT's copy of the server database: item templates and weapon presets.

use std::collections::HashMap;

use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use serde_json::Value as Json;

use crate::install::Install;

/// One entry of `templates/items.json`.
#[derive(Debug, Clone, Deserialize)]
pub struct ItemTemplate {
    #[serde(rename = "_id")]
    pub id: String,
    #[serde(rename = "_name", default)]
    pub name: String,
    /// The class of item this is one of: another entry's id.
    #[serde(rename = "_parent", default)]
    pub parent: String,
    #[serde(rename = "_props", default)]
    pub props: Json,
}

impl ItemTemplate {
    /// Manifest key of the bundle holding this item's prefab.
    pub fn prefab_key(&self) -> Option<&str> {
        self.props.get("Prefab")?.get("path")?.as_str().filter(|p| !p.is_empty())
    }

    pub fn prop_f32(&self, name: &str) -> Option<f32> {
        self.props.get(name)?.as_f64().map(|v| v as f32)
    }
}

/// One part of an assembled weapon.
#[derive(Debug, Clone, Deserialize)]
pub struct PresetItem {
    #[serde(rename = "_id")]
    pub id: String,
    #[serde(rename = "_tpl")]
    pub template: String,
    #[serde(rename = "parentId", default)]
    pub parent: Option<String>,
    /// Name of the slot on the parent, which is also the name of the bone
    /// the part attaches to.
    #[serde(rename = "slotId", default)]
    pub slot: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Preset {
    #[serde(rename = "_id")]
    pub id: String,
    #[serde(rename = "_name", default)]
    pub name: String,
    /// Id of the root item in `items`.
    #[serde(rename = "_parent")]
    pub root: String,
    #[serde(rename = "_items")]
    pub items: Vec<PresetItem>,
    /// Template id of the weapon this is the stock build of, if any.
    #[serde(rename = "_encyclopedia", default)]
    pub encyclopedia: Option<String>,
}

pub struct Database {
    pub items: HashMap<String, ItemTemplate>,
    pub presets: Vec<Preset>,
    /// `globals.json` `config` section: movement, stamina, aiming and inertia settings.
    pub config: Json,
    /// The weapons that are not the install's but one's own, by their
    /// item templates (`crate::custom`): each is in `items` and `presets`
    /// like any other, and this says how it is put together.
    pub custom: HashMap<String, std::sync::Arc<crate::custom::Definition>>,
    /// The outfits that are not the install's (`crate::outfit`), by their
    /// item templates: each is in `items` like any other.
    pub outfits: HashMap<String, std::sync::Arc<crate::outfit::Definition>>,
    /// Why a weapon or an outfit of one's own was left out, if one was.
    pub custom_notes: Vec<String>,
}

impl Database {
    pub fn load(install: &Install) -> Result<Self> {
        let dir = install.database_dir();
        let read = |rel: &str| -> Result<String> {
            let path = dir.join(rel);
            std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))
        };
        let items: HashMap<String, ItemTemplate> =
            serde_json::from_str(&read("templates/items.json")?).context("items.json")?;
        let mut globals: Json = serde_json::from_str(&read("globals.json")?).context("globals.json")?;
        let presets = match globals.get_mut("ItemPresets").map(Json::take) {
            Some(Json::Object(map)) => map
                .into_iter()
                .filter_map(|(_, v)| serde_json::from_value(v).ok())
                .collect(),
            _ => Vec::new(),
        };
        let config = globals.get_mut("config").map(Json::take).unwrap_or(Json::Null);
        let mut db = Self { items, presets, config, custom: HashMap::new(), outfits: HashMap::new(), custom_notes: Vec::new() };
        // And the weapons and the outfits of one's own, which are no part
        // of the install: none of the install's own entries is changed by
        // them.
        crate::custom::register_all(&mut db);
        crate::outfit::register_all(&mut db);
        Ok(db)
    }

    pub fn item(&self, id: &str) -> Result<&ItemTemplate> {
        self.items.get(id).ok_or_else(|| anyhow!("no item template {id}"))
    }

    /// Whether an item is of a class, by the class's name in the database
    /// (`Stock`, `Pistol`, `Weapon`): itself or any class above it.
    pub fn is_a(&self, item: &ItemTemplate, class: &str) -> bool {
        let mut at = Some(item);
        while let Some(template) = at {
            if template.name == class {
                return true;
            }
            at = self.items.get(&template.parent);
        }
        false
    }

    /// The stock build of a weapon.
    pub fn default_preset(&self, weapon_template: &str) -> Result<&Preset> {
        self.presets
            .iter()
            .find(|p| p.encyclopedia.as_deref() == Some(weapon_template))
            .ok_or_else(|| anyhow!("no default preset for {weapon_template}"))
    }
}
