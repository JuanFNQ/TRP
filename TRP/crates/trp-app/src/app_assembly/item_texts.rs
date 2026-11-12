//! The install's texts and names, the equipment panel, the inventory's rules and the airdrop crate.

use super::*;

pub(super) struct Texts {
    pub(super) locale: trp_data::character::inventory::Locale,
    pub(super) words: std::sync::Arc<std::collections::HashMap<String, String>>,
    pub(super) texts: trp_data::character::inventory::ItemTexts,
    pub(super) named_builds: Vec<(String, Vec<String>)>,
    pub(super) names: std::collections::HashMap<String, String>,
    pub(super) panel: trp_data::interface::screen::GearPanel,
    pub(super) rules: trp_sim::items::inventory::Rules,
    pub(super) loot: trp_sim::items::inventory::Item,
}

/// Reads the install's texts and the things' names, the equipment panel, the inventory's rules and the empty airdrop crate.
pub(super) fn read(install: &Install, db: &std::sync::Arc<Database>) -> anyhow::Result<Texts> {
    // The install's texts, read once for the run (`Locale`; findings, "The
    // game's texts read once"): kept as every key's words in small letters
    // (`words`), which the screens, the options, the menus and the edit
    // build screen take theirs from; the whole file's as it has them only
    // until the suits are named (below).
    platform::run_log::phase("the character's kit");
    let locale = trp_data::character::inventory::Locale::read(&install, "en").unwrap_or_else(|e| {
        eprintln!("the install's texts could not be read; the app's own words stand in: {e:#}");
        Default::default()
    });
    let words = locale.words();
    // What the window that shows a thing says of it.
    let mut texts = locale.item_texts(&install, &words).unwrap_or_else(|e| {
        eprintln!("the install's item texts could not be read; a thing's window says less: {e:#}");
        Default::default()
    });
    // A weapon or a part of one's own has no words in the install's texts:
    // its window is captioned with its file's name for it, not its
    // template's.
    for definition in db.custom.values() {
        texts.names.extend(definition.names().map(|(id, name)| (id.to_owned(), name.to_owned())));
    }
    for part in db.custom_parts.values() {
        texts.names.insert(part.id.clone(), part.shown_name().to_owned());
    }
    let named_builds = screens::inventory_view::named_builds(&db, &texts);
    let mut names = locale.short_names();
    // A weapon of one's own has no name in the install's texts: its
    // file's.
    for definition in db.custom.values() {
        names.extend(definition.names().map(|(id, name)| (id.to_owned(), name.to_owned())));
    }
    // Nor has a part of one's own.
    for part in db.custom_parts.values() {
        names.insert(part.id.clone(), part.shown_name().to_owned());
    }
    // Nor has an outfit of one's own.
    for definition in db.outfits.values() {
        names.insert(definition.id.clone(), definition.shown_name().to_owned());
    }
    // The crate on the range (inventory spec, "The crate"): the install's
    // airdrop crate, holding the app's weapon as its stock preset
    // has it and the vest and the backpack a BEAR of the Standard edition
    // starts with, each with what the database has in it, but for the
    // magazines and the rounds: those are the weapon's own
    // (`Loadout::fit_to`). The two are made for the body at the
    // start and drawn once they are put on.
    // Where the game's own screen has the equipment's slot views.
    platform::run_log::phase("the items' texts and names");
    let panel = trp_data::interface::screen::GearPanel::load(&install).unwrap_or_else(|e| {
        eprintln!("the install's equipment panel could not be read, the app's own arrangement is used: {e:#}");
        Default::default()
    });
    let rules = trp_sim::items::inventory::Rules::of(&db);
    let mut crate_kind = trp_sim::items::inventory::Kind::of(&db, CRATE)?;
    if let Some(grid) = crate_kind.grids.first_mut() {
        grid.cells = CRATE_CELLS;
    }
    // (The airdrop crate, empty: the screen's first container
    // to bring, which nothing on the range opens.)
    let loot = trp_sim::items::inventory::Item::new(std::sync::Arc::new(crate_kind));
    Ok(Texts { locale, words, texts, named_builds, names, panel, rules, loot })
}
