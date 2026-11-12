//! Tests of the head's things fitted to one another, by the install's items.
//! After the body animator spec, "The head's things fitted to one another", through
//! `trp_sim::inventory::fitting_bits`, the game's `PlayerClippingRules.CalculateRules`.

use std::sync::Arc;

use trp_data::character::dress::fit;
use trp_sim::items::inventory::{fitting_bits_of, Equipment, Item, Kind};

/// The USEC cap (headwear of `BodyArmor`), the ComTac II, the Buffalo half
/// mask: the range's dummy 0's (the third-person brief's set), the cap and
/// headset the capture's `--wear`.
const USEC_CAP: &str = "5aa2a7e8e5b5b00016327c16";
const COMTAC_II: &str = "5645bcc04bdc2d363b8b4572";
const BUFFALO: &str = "572b7fa524597762b747ce82";
/// The Grom glasses (eyewear) and the ZSh-1-2M (headwear of `Helmet`).
const GROM: &str = "59e770b986f7742cbd762754";
const ZSH: &str = "5aa7e454e5b5b0214e506fa2";

fn kind(db: &trp_data::Database, template: &str) -> Arc<Kind> {
    Arc::new(Kind::of(db, template).unwrap_or_else(|e| panic!("{template}: {e}")))
}

/// Equipment wearing each thing in its slot.
fn wearing(db: &trp_data::Database, worn: &[(&str, &str)]) -> Equipment {
    let mut equipment = Equipment::empty(db).expect("the equipment");
    for (slot, template) in worn {
        let at = equipment.item.slot_index(slot).expect("the slot");
        equipment.item.slots[at] = Some(Item::new(kind(db, template)));
    }
    equipment
}

/// The game's table: a face cover gives the hat's and the helmet's bits and
/// the visor's (no face cover of the install is a half mask), eyewear the
/// face cover's and the hat's, headwear that is not a helmet the
/// headphones', an earpiece the hat's. Dummy 0's set puts the cap and the
/// headset on their second meshes and leaves the mask on its first, until
/// a top whose changer gives the face cover's bit is worn with it.
#[test]
fn the_head_things_bits_follow_the_game_s_table() {
    let Some(db) = super::fixture::database() else { return };
    let bits = |worn: &[(&str, &str)]| fitting_bits_of(&wearing(db, worn).item, &[]);
    assert_eq!(bits(&[]), 0);
    assert_eq!(bits(&[("Headwear", USEC_CAP)]), fit::HEADPHONES, "a cap is not a helmet");
    assert_eq!(bits(&[("Headwear", ZSH)]), 0, "a helmet gives nothing");
    assert_eq!(bits(&[("Earpiece", COMTAC_II)]), fit::HAT);
    assert_eq!(bits(&[("FaceCover", BUFFALO)]), fit::HAT | fit::HELMET | fit::VISOR);
    assert_eq!(bits(&[("Eyewear", GROM)]), fit::FACE_COVER | fit::HAT);

    let dummy = bits(&[("Headwear", USEC_CAP), ("Earpiece", COMTAC_II), ("FaceCover", BUFFALO)]);
    assert_eq!(dummy, fit::HAT | fit::HELMET | fit::VISOR | fit::HEADPHONES);
    let fitted = |kind: u8, bits: u8| bits & kind == kind;
    assert!(fitted(fit::HAT, dummy), "the cap on its second mesh");
    assert!(fitted(fit::HEADPHONES, dummy), "the headset on its second mesh");
    assert!(!fitted(fit::FACE_COVER, dummy), "the mask on its first");
    // A top with a changer of the face cover's bit (84 of the install's 88).
    assert!(fitted(fit::FACE_COVER, dummy | fit::FACE_COVER));

    // Things drawn whatever the slots hold (`--wear`), each in its kind's slot.
    let (cap, headset) = (kind(db, USEC_CAP), kind(db, COMTAC_II));
    assert_eq!(fitting_bits_of(&Equipment::empty(db).unwrap().item, &[&cap, &headset]), fit::HAT | fit::HEADPHONES);

    // No face cover of the install is a half mask (`IsHalfMask` is named by
    // no template), and every helmet's material is told.
    let face_covers: Vec<Arc<Kind>> = db.items.values().filter(|item| item.node_type == "Item" && db.class_names(&item.id).iter().any(|class| class == "FaceCover")).map(|item| kind(db, &item.id)).collect();
    assert!(face_covers.len() > 40, "{} face covers", face_covers.len());
    assert!(face_covers.iter().all(|kind| !kind.half_mask));
    assert!(kind(db, ZSH).helmet_material && !kind(db, USEC_CAP).helmet_material);
}
