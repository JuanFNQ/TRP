//! Tests of a helmet's cut of what is heard, by the install's items.
//! After the sound spec, "Hearing under headgear", through `trp_sim::hearing`.

use std::sync::Arc;

use trp_sim::sound::hearing::{choose, Choice, Deaf};
use trp_sim::items::inventory::{Equipment, Item, Kind};

/// The thing of that template as the game makes one, its locked inserts in
/// it, with each part given put in the first free slot that takes it, its
/// switch on where `on`.
fn made(db: &trp_data::Database, template: &str, parts: &[&str], on: bool) -> Item {
    let mut item = Item::new(Arc::new(Kind::of(db, template).unwrap_or_else(|e| panic!("{template}: {e}"))));
    item.fill_locked(db).expect("its inserts");
    for part in parts {
        let mut part = Item::new(Arc::new(Kind::of(db, part).expect("the part")));
        part.on = on;
        let at = (0..item.slots.len()).find(|&at| item.slots[at].is_none() && item.slot_takes(at, &part).is_ok()).unwrap_or_else(|| panic!("no slot of {template} takes {}", part.kind.template));
        item.slots[at] = Some(part);
    }
    item
}

/// Equipment wearing each thing in its slot.
fn wearing(db: &trp_data::Database, worn: Vec<(&str, Item)>) -> Equipment {
    let mut equipment = Equipment::empty(db).expect("the equipment");
    for (slot, item) in worn {
        let at = equipment.item.slot_index(slot).expect("the slot");
        equipment.item.slots[at] = Some(item);
    }
    equipment
}

/// The ZSh-1-2M (`5aa7e454e5b5b0214e506fa2`, in the recording) is High
/// with its face shield (`5aa7e3abe5b5b000171d064d`) up or down; the FAST
/// MT none alone and Low with its side armour, a headset's with the RAC
/// fitted; the 6B47 Low with no armour of its own; the welding mask Low on
/// the face; a headset in the ear over the ZSh-1-2M wins, the default for
/// now. Every headwear of the database chooses by its own cut, alone.
#[test]
fn the_install_s_headgear_chooses_its_template() {
    let Ok(_install) = crate::fixture::install().ok_or(()) else { return };
    let db = crate::fixture::database().expect("database");
    let zsh = "5aa7e454e5b5b0214e506fa2";
    let fast = "5a154d5cfcdbcb001a3b00da";
    for on in [false, true] {
        let equipment = wearing(db, vec![("Headwear", made(db, zsh, &["5aa7e3abe5b5b000171d064d"], on))]);
        assert_eq!(choose(&equipment), Choice::Deaf(Deaf::High), "the ZSh-1-2M, its shield {}", if on { "down" } else { "up" });
        assert_eq!(choose(&equipment).template().name, "StrongMute");
    }
    assert_eq!(choose(&wearing(db, vec![("Headwear", made(db, fast, &[], false))])), Choice::Deaf(Deaf::None));
    assert_eq!(choose(&wearing(db, vec![("Headwear", made(db, fast, &["5a16badafcdbcb001865f72d"], false))])), Choice::Deaf(Deaf::Low), "the FAST's side armour");
    assert_eq!(choose(&wearing(db, vec![("Headwear", made(db, fast, &["5a16b9fffcdbcb0176308b34"], false))])), Choice::Headset, "the RAC headset on the FAST");
    assert_eq!(choose(&wearing(db, vec![("Headwear", made(db, "5a7c4850e899ef00150be885", &[], false))])), Choice::Deaf(Deaf::Low), "the 6B47");
    assert_eq!(choose(&wearing(db, vec![("FaceCover", made(db, "60a7ad3a0c5cb24b0134664a", &[], false))])), Choice::Deaf(Deaf::Low), "the welding mask");
    let headset = wearing(db, vec![("Earpiece", made(db, "5b432b965acfc47a8774094e", &[], false)), ("Headwear", made(db, zsh, &[], false))]);
    assert_eq!(choose(&headset), Choice::Headset);
    assert!(choose(&headset).template().is_default(), "a headset: the default template until headsets are built");
    // Every headwear alone: its own cut. And the database's count.
    let mut counts = [0; 3];
    for item in db.items.values().filter(|item| item.node_type == "Item") {
        let deaf = item.props.get("DeafStrength").and_then(|word| word.as_str()).map_or(Deaf::None, Deaf::of_word);
        counts[deaf as usize] += 1;
        if db.is_a(item, "Headwear") && item.prefab_key().is_some() {
            let kind = Kind::of(db, &item.id).expect("the kind");
            assert_eq!(kind.deaf, deaf);
            let equipment = wearing(db, vec![("Headwear", Item::new(Arc::new(kind)))]);
            assert_eq!(choose(&equipment), Choice::Deaf(deaf), "{} {}", item.id, item.name);
        }
    }
    println!("the database's cuts: {} none, {} Low, {} High", counts[0], counts[1], counts[2]);
    assert_eq!((counts[1], counts[2]), (23, 21));
}
