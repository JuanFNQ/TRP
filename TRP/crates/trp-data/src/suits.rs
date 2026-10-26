//! The clothing suits of the install's customization list (inventory spec,
//! "The OVERALL tab"): each an upper body (a body and the hands that go
//! with it) or a lower body (the legs), with the bundles of its parts and
//! its name as the game's locale words it (`<id> Name`). The game offers a
//! character the suits of its own side; this work lists every side's, by
//! the user's word.

use anyhow::{Context, Result};
use serde_json::Value;

use crate::install::Install;

/// One suit.
#[derive(Debug, Clone, PartialEq)]
pub struct Suit {
    pub id: String,
    /// As the locale words it, or the suit's own name where the locale has
    /// none.
    pub name: String,
    /// An upper body's (`Body`, `Hands`), or a lower body's (`Feet`).
    pub upper: bool,
    /// The sides it is the game's for (`Usec`, `Bear`, `Savage`).
    pub sides: Vec<String>,
    /// The bundles of its parts: the body's and the hands' for an upper
    /// suit, the legs' for a lower one.
    pub body: Option<String>,
    pub hands: Option<String>,
    pub feet: Option<String>,
}

/// Every suit of the install, the upper ones then the lower, each list by
/// its name.
pub fn suits(install: &Install, language: &str) -> Result<Vec<Suit>> {
    let path = install.database_dir().join("templates/customization.json");
    let text = std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let all: serde_json::Map<String, Value> = serde_json::from_str(&text).with_context(|| path.display().to_string())?;
    let locale_path = install.database_dir().join(format!("locales/global/{language}.json"));
    let locale: serde_json::Map<String, Value> = std::fs::read_to_string(&locale_path).ok().and_then(|text| serde_json::from_str(&text).ok()).unwrap_or_default();
    // A part's bundle, by the part's id.
    let bundle = |id: Option<&str>| -> Option<String> {
        let part = all.get(id?)?;
        let path = part.get("_props")?.get("Prefab")?.get("path")?.as_str()?;
        (!path.is_empty()).then(|| path.to_owned())
    };
    let mut suits: Vec<Suit> = all
        .iter()
        .filter(|(_, entry)| entry.get("_type").and_then(Value::as_str) == Some("Item"))
        .filter_map(|(id, entry)| {
            let props = entry.get("_props")?;
            let field = |name: &str| props.get(name).and_then(Value::as_str).filter(|s| !s.is_empty());
            let upper = field("Body").is_some();
            if !upper && field("Feet").is_none() {
                return None;
            }
            let worded = locale.get(&format!("{id} Name")).and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty());
            let name = worded.map(str::to_owned).or_else(|| entry.get("_name").and_then(Value::as_str).map(str::to_owned)).unwrap_or_else(|| id.clone());
            Some(Suit {
                id: id.clone(),
                name,
                upper,
                sides: props.get("Side").and_then(Value::as_array).map(|sides| sides.iter().filter_map(|s| s.as_str().map(str::to_owned)).collect()).unwrap_or_default(),
                body: bundle(field("Body")),
                hands: bundle(field("Hands")),
                feet: bundle(field("Feet")),
            })
        })
        .filter(|suit| if suit.upper { suit.body.is_some() } else { suit.feet.is_some() })
        .collect();
    suits.sort_by(|a, b| b.upper.cmp(&a.upper).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())).then_with(|| a.id.cmp(&b.id)));
    Ok(suits)
}
