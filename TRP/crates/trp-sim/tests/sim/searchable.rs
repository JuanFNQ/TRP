//! Tests of which of the install's things hide what they hold until searched.
//! After the inventory spec, "Searching a body": the game's `SearchableItem`, that is
//! `Backpack`, `Vest`, `Pockets`, `LootContainer` and `MobContainer`, by the class the
//! database gives each. These pass trivially, with a note, where no install
//! is configured.

use std::sync::Arc;

use trp_sim::items::inventory::{is, Item, Kind};
use trp_sim::items::search::{scav_body, searchable};

use crate::fixture::database;

/// A pack, a rig, the pockets, a weapon box (a loot container) and the
/// dead Scav (a container of the world's): all searchable. A weapon case,
/// a helmet and a round: none.
#[test]
fn bags_vests_pockets_and_crates_are_searchable() {
    let Some(db) = database() else { return };
    let of = |template: &str| Item::new(Arc::new(Kind::of(db, template).expect(template)));
    for (template, name) in [
        ("5ab8ebf186f7742d8b372e80", "SSO Attack 2 raid backpack"),
        ("5c0e746986f7741453628fe5", "WARTECH TV-110 plate carrier rig"),
        ("627a4e6b255f7527fb05a0f6", "pockets"),
        ("5909d5ef86f77467974efbd8", "the weapon box"),
        ("5909e4b686f7747f5b744fa4", "the dead Scav"),
    ] {
        assert!(searchable(&of(template)), "{name} should be searchable");
    }
    for (template, name) in [("59fb023c86f7746d0d4b423c", "the weapon case"), ("5aa7d03ae5b5b00016327db5", "a helmet"), ("54527a984bdc2d4e668b4567", "a round")] {
        assert!(!searchable(&of(template)), "{name} should not be searchable");
    }
}

/// The dummies' bodies: a Scav's copy of an equipment keeps its slots but
/// empties the dog tag's and the armband's.
#[test]
fn a_scav_copy_of_the_equipment_keeps_its_slots() {
    let Some(db) = database() else { return };
    let mut equipment = Item::new(Arc::new(Kind::of(db, trp_sim::items::inventory::EQUIPMENT_TEMPLATE).expect("equipment")));
    let tag = Item::new(Arc::new(Kind::of(db, "59f32c3b86f77472a31742f0").expect("a dog tag")));
    if let Some(index) = equipment.slot_index("Dogtag") {
        equipment.slots[index] = Some(tag);
    }
    let body = scav_body(&equipment);
    assert_eq!(body.slots.len(), equipment.slots.len());
    assert!(body.in_slot("Dogtag").is_none());
    assert!(is(&body, "Equipment") || body.kind.template == trp_sim::items::inventory::EQUIPMENT_TEMPLATE);
}
