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
    /// `Item` for an item the game makes, `Node` for a class of them.
    #[serde(rename = "_type", default)]
    pub node_type: String,
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
    /// `settings.json` `config` section: what the game's client settings
    /// are read from (`ClientSettingsConfig`; the sounds of moving's
    /// multipliers). Null where the install has no such file.
    pub client: Json,
    /// The weapons that are not the install's but one's own, by their
    /// item templates (`crate::custom`): each is in `items` and `presets`
    /// like any other, and this says how it is put together.
    pub custom: HashMap<String, std::sync::Arc<crate::custom::Definition>>,
    /// The outfits that are not the install's (`crate::outfit`), by their
    /// item templates: each is in `items` like any other.
    pub outfits: HashMap<String, std::sync::Arc<crate::outfit::Definition>>,
    /// Why a weapon or an outfit of one's own was left out, if one was.
    pub custom_notes: Vec<String>,
    /// The weapons whose stock presets the app put a magazine into, by
    /// their item templates (`fit_magazines`).
    pub given_magazines: Vec<String>,
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
        let client = read("settings.json")
            .ok()
            .and_then(|text| serde_json::from_str::<Json>(&text).ok())
            .and_then(|mut settings| settings.get_mut("config").map(Json::take))
            .unwrap_or(Json::Null);
        let mut db = Self { items, presets, config, client, custom: HashMap::new(), outfits: HashMap::new(), custom_notes: Vec::new(), given_magazines: Vec::new() };
        db.fit_magazines();
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

    /// Puts a magazine into each stock preset of a weapon fed from
    /// magazines (`ReloadMode` `ExternalMagazine`) that has nothing in its
    /// magazine slot: the one its template names as its default
    /// (`defMagType`), if the slot takes it. Of SPT 4.1's presets only the
    /// RPK-16's is such ("RPK16 nomag"). **The app's**, so that every weapon
    /// in a crate has a magazine to fill; the game's client never reads
    /// `defMagType`, and SPT's server takes it as the magazine of a weapon
    /// whose build has none (`GetWeaponsDefaultMagazineTpl`, for the spare
    /// magazines it gives a bot). Inventory spec, "Built": the tables of
    /// weapons.
    fn fit_magazines(&mut self) {
        const SLOT: &str = "mod_magazine";
        for preset in &mut self.presets {
            let Some(weapon) = preset.encyclopedia.as_deref().and_then(|id| self.items.get(id)) else { continue };
            if weapon.props.get("ReloadMode").and_then(Json::as_str) != Some("ExternalMagazine") || preset.items.iter().any(|item| item.slot.as_deref() == Some(SLOT)) {
                continue;
            }
            let Some(magazine) = weapon.props.get("defMagType").and_then(Json::as_str).filter(|id| self.items.contains_key(*id)) else { continue };
            let takes = weapon.props.get("Slots").and_then(Json::as_array).into_iter().flatten().filter(|slot| slot.get("_name").and_then(Json::as_str) == Some(SLOT));
            let filters = takes.filter_map(|slot| slot.pointer("/_props/filters")?.as_array()).flatten();
            if !filters.filter_map(|filter| filter.get("Filter")?.as_array()).flatten().any(|id| id.as_str() == Some(magazine)) {
                continue;
            }
            preset.items.push(PresetItem { id: format!("{}-magazine", preset.root), template: magazine.to_owned(), parent: Some(preset.root.clone()), slot: Some(SLOT.into()) });
            self.given_magazines.push(weapon.id.clone());
        }
    }

    /// The stock build of a weapon: its stock preset as the install has it,
    /// with a magazine where the app puts one (`fit_magazines`).
    pub fn default_preset(&self, weapon_template: &str) -> Result<&Preset> {
        self.presets
            .iter()
            .find(|p| p.encyclopedia.as_deref() == Some(weapon_template))
            .ok_or_else(|| anyhow!("no default preset for {weapon_template}"))
    }
}
