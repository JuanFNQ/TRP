//! The locale's words, items' short names and texts.

use super::*;

/// The install's texts in one of the languages the database has them in
/// (`locales/global/<language>.json`: 3 MB, some 15 ms a parse), read once
/// as a run begins and asked of by whatever wants its words, which had
/// each parsed the file again: every key's words as the file has the key, a key whose value
/// is not words left out.
#[derive(Debug, Clone, Default)]
pub struct Locale {
    pub(super) texts: std::collections::HashMap<String, String>,
}

impl Locale {
    pub fn read(install: &Install, language: &str) -> Result<Self> {
        let path = install.database_dir().join(format!("locales/global/{language}.json"));
        let text = std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        let texts: std::collections::HashMap<String, Json> = serde_json::from_str(&text).with_context(|| path.display().to_string())?;
        let words = |(key, value): (String, Json)| match value {
            Json::String(words) => Some((key, words)),
            _ => None,
        };
        Ok(Self { texts: texts.into_iter().filter_map(words).collect() })
    }

    /// A key's words, the key as the file has it.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.texts.get(key).map(String::as_str)
    }

    /// The game's short name for every item, by template.
    pub fn short_names(&self) -> std::collections::HashMap<String, String> {
        self.texts.iter().filter_map(|(key, words)| Some((key.strip_suffix(" ShortName")?.to_owned(), words.clone()))).collect()
    }

    /// Every key's words by the key in small letters, as the game looks a
    /// key up whatever its case (`LocalizationManager`): what the run keeps
    /// of the texts, shared by whatever asks for words later (`words_of`).
    pub fn words(&self) -> std::sync::Arc<std::collections::HashMap<String, String>> {
        std::sync::Arc::new(self.texts.iter().map(|(key, words)| (key.to_lowercase(), words.clone())).collect())
    }

    /// The window that shows a thing's words (`ItemTexts`), keeping those
    /// words (`words`) as every key's.
    pub fn item_texts(&self, install: &Install, words: &std::sync::Arc<std::collections::HashMap<String, String>>) -> Result<ItemTexts> {
        let said = |key: &str| self.get(key).map(str::to_owned);
        let mut out = ItemTexts { kg: said(" kg").unwrap_or_else(|| " kg".to_owned()), words: words.clone(), ..ItemTexts::default() };
        for (key, words) in &self.texts {
            if let Some(id) = key.strip_suffix(" Name") {
                out.names.insert(id.to_owned(), words.to_owned());
            } else if let Some(id) = key.strip_suffix(" Description") {
                out.descriptions.insert(id.to_owned(), words.to_owned());
            }
        }
        let path = install.database_dir().join("templates/handbook.json");
        let handbook: Json = serde_json::from_str(&std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?)?;
        let parents: std::collections::HashMap<String, String> = handbook["Categories"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|category| Some((category["Id"].as_str()?.to_owned(), category["ParentId"].as_str().unwrap_or_default().to_owned())))
            .collect();
        for item in handbook["Items"].as_array().into_iter().flatten() {
            let (Some(id), Some(mut at)) = (item["Id"].as_str(), item["ParentId"].as_str()) else { continue };
            let mut chain = Vec::new();
            while !at.is_empty() && chain.len() < 16 {
                chain.push(said(at).unwrap_or_else(|| at.to_owned()));
                at = parents.get(at).map_or("", String::as_str);
            }
            chain.reverse();
            out.kinds.insert(id.to_owned(), chain.join(" > "));
        }
        Ok(out)
    }
}

/// The words of the names asked for, from every key's words by the key in
/// small letters (`Locale::words`): found whatever their case, keyed by
/// the name in small letters, a name the install has no words for left
/// out; as `texts` reads them from the file.
pub fn words_of(words: &std::collections::HashMap<String, String>, names: &[&str]) -> std::collections::HashMap<String, String> {
    names
        .iter()
        .filter_map(|name| {
            let key = name.to_lowercase();
            let said = words.get(&key)?.clone();
            Some((key, said))
        })
        .collect()
}

/// The game's short name for every item, by template, in one of the
/// languages the database has texts for (`locales/global/en.json`).
pub fn short_names(install: &Install, language: &str) -> Result<std::collections::HashMap<String, String>> {
    Ok(Locale::read(install, language)?.short_names())
}

/// What the game's window that shows a thing says of it, by template, in
/// one of the languages the database has texts for: its name
/// (`<id> Name`), its description (`<id> Description`), and its kind as
/// the handbook files it (`templates/handbook.json`: the item's category
/// and its parents', from the first, each in the language's words, joined
/// by " > ", as `ItemSpecificationPanel` writes its type); and the words
/// for a weight's unit (" kg").
#[derive(Debug, Clone, Default)]
pub struct ItemTexts {
    pub names: std::collections::HashMap<String, String>,
    pub descriptions: std::collections::HashMap<String, String>,
    pub kinds: std::collections::HashMap<String, String>,
    pub kg: String,
    /// Every key's words, by the key in lower case (the game's locale
    /// is looked up whatever the case: `LocalizationManager`); the run's
    /// one copy (`Locale::words`).
    pub words: std::sync::Arc<std::collections::HashMap<String, String>>,
}

impl ItemTexts {
    /// The install's words for a key, whatever its case.
    pub fn say(&self, key: &str) -> Option<String> {
        self.words.get(&key.to_lowercase()).cloned()
    }
}

pub fn item_texts(install: &Install, language: &str) -> Result<ItemTexts> {
    let locale = Locale::read(install, language)?;
    locale.item_texts(install, &locale.words())
}
