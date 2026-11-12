//! The clothing suits of the install's customization list.
//! After the inventory spec, "The OVERALL tab": each an upper body (a body and the hands that go
//! with it) or a lower body (the legs), with the bundles of its parts and
//! its name as the game's locale words it (`<id> Name`). The game offers a
//! character the suits of its own side; this work lists every side's.

use anyhow::{Context, Result};
use serde_json::Value;

use crate::setup::install::Install;

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
    /// An upper suit's body by its customization id (`_props.Body`): the
    /// body's `BodyCustomizationId`, which an armband's meshes go by.
    pub body_id: Option<String>,
}

/// Every suit of the install, the upper ones then the lower, each list by
/// its name, worded in the run's texts (`Locale`; read once, where they
/// could not be read its own names).
pub fn suits(install: &Install, locale: &crate::character::inventory::Locale) -> Result<Vec<Suit>> {
    let path = install.database_dir().join("templates/customization.json");
    let text = std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let all: serde_json::Map<String, Value> = serde_json::from_str(&text).with_context(|| path.display().to_string())?;
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
            let worded = locale.get(&format!("{id} Name")).map(str::trim).filter(|s| !s.is_empty());
            let name = worded.map(str::to_owned).or_else(|| entry.get("_name").and_then(Value::as_str).map(str::to_owned)).unwrap_or_else(|| id.clone());
            Some(Suit {
                id: id.clone(),
                name,
                upper,
                sides: props.get("Side").and_then(Value::as_array).map(|sides| sides.iter().filter_map(|s| s.as_str().map(str::to_owned)).collect()).unwrap_or_default(),
                body: bundle(field("Body")),
                hands: bundle(field("Hands")),
                feet: bundle(field("Feet")),
                body_id: field("Body").map(str::to_owned),
            })
        })
        .filter(|suit| if suit.upper { suit.body.is_some() } else { suit.feet.is_some() })
        .collect();
    suits.sort_by(|a, b| b.upper.cmp(&a.upper).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())).then_with(|| a.id.cmp(&b.id)));
    Ok(suits)
}

/// What a bot of a type may look like, as SPT's server dresses one
/// (`bots/types/<type>.json`, its `appearance`; `BotGenerator.
/// SetBotAppearance`): for its head, its body and its legs, each
/// customization's id with its weight, as the file lists them; and its
/// voices, drawn the same way (`BotGenerator.GenerateBot`, before the
/// look: `appearance.voice`).
#[derive(Debug, Clone, PartialEq)]
pub struct BotLooks {
    pub head: Vec<(String, f64)>,
    pub body: Vec<(String, f64)>,
    pub feet: Vec<(String, f64)>,
    pub voice: Vec<(String, f64)>,
}

/// The looks of a bot type of the install by its name as SPT keys it, the
/// role in lower case (`BotHelper.GetBotTemplate`: a PMC's is its side's,
/// `usec` or `bear`).
pub fn bot_looks(install: &Install, bot_type: &str) -> Result<BotLooks> {
    let path = install.database_dir().join(format!("bots/types/{bot_type}.json"));
    let text = std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let all: Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).with_context(|| path.display().to_string())?;
    let list = |name: &str| -> Vec<(String, f64)> {
        all.pointer(&format!("/appearance/{name}")).and_then(Value::as_object).into_iter().flatten().filter_map(|(id, weight)| Some((id.clone(), weight.as_f64()?))).collect()
    };
    Ok(BotLooks { head: list("head"), body: list("body"), feet: list("feet"), voice: list("voice") })
}

/// The install's customizations by id (`templates/customization.json`):
/// what a part of a look is drawn from.
pub struct Customizations(serde_json::Map<String, Value>);

impl Customizations {
    pub fn load(install: &Install) -> Result<Self> {
        let path = install.database_dir().join("templates/customization.json");
        let text = std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        Ok(Self(serde_json::from_str(&text).with_context(|| path.display().to_string())?))
    }

    /// A part's bundle (`_props.Prefab.path`), where it names one.
    pub fn bundle(&self, id: &str) -> Option<String> {
        let path = self.0.get(id)?.pointer("/_props/Prefab/path")?.as_str()?;
        (!path.is_empty()).then(|| path.to_owned())
    }

    /// Every upper body's customization id by its bundle (the parts whose
    /// `BodyPart` is `Body`), each bundle's ids sorted: what an
    /// armband's meshes are chosen by (`PlayerBody.BodyCustomizationId`,
    /// `trp_data::dress::ArmBand`), for a body the app knows by its bundle.
    pub fn body_ids(&self) -> std::collections::HashMap<String, Vec<String>> {
        let mut ids: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
        for (id, entry) in &self.0 {
            if entry.pointer("/_props/BodyPart").and_then(Value::as_str) != Some("Body") {
                continue;
            }
            if let Some(bundle) = self.bundle(id) {
                ids.entry(bundle).or_default().push(id.clone());
            }
        }
        ids.values_mut().for_each(|of| of.sort());
        ids
    }

    /// A voice's name as its phrases are found by (`Usec_1`): the
    /// template's own `_name`. It is its `_props.Name` too but for two,
    /// whose `_props.Name` is what the game shows (`Rob`, `Vitaly`) and
    /// whose `_name` (`Usec_7`, `Bear_4`) is the name the client's table
    /// of phrase bundles keys (`InGameBundles.TakePhrasePath`); a bot
    /// given either speaks in the game (`courses3\3-bot-blast`'s
    /// `Rob_hit_07`, `Vitaly_hit_15`).
    pub fn voice_name(&self, id: &str) -> Option<String> {
        let template = self.0.get(id)?;
        template.get("_name").and_then(Value::as_str).or_else(|| template.pointer("/_props/Name")?.as_str()).map(str::to_owned)
    }
}
