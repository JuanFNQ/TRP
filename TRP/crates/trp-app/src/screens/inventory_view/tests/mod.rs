//! Tests of the inventory screen, with the carried things they share.

use std::sync::Arc;

use trp_data::character::inventory::Holder;
use trp_sim::items::inventory::{Kind, EQUIPMENT_TEMPLATE};

use super::*;

mod naming_looks;
mod windows_scroll;
mod moves_hands;
mod sliders;

use naming_looks::{kind};
use windows_scroll::{holder};

/// A bare character beside an open crate that holds a backpack, a
/// rifle, sixty rounds and a small case (the dummies' tests search a
/// body beside it: `dummy::tests`).
pub(crate) fn carried() -> (Carried, [Id; 4]) {
    let slots = vec![holder("FirstPrimaryWeapon", [0, 0], "Weapon"), holder("Backpack", [0, 0], "Backpack"), holder("TacticalVest", [0, 0], "Vest")];
    let worn = Item::new(Arc::new(Kind { template: EQUIPMENT_TEMPLATE.into(), slots, ..kind("equipment", &[], [1, 1]) }));
    let grid = |cells| vec![holder("main", cells, "Item")];
    let mut loot = Item::new(Arc::new(Kind { grids: grid([8, 8]), ..kind("crate", &[], [1, 1]) }));
    let bag = Item::new(Arc::new(Kind { grids: grid([4, 4]), ..kind("bag", &["Backpack", "SearchableItem", "Equipment"], [3, 3]) }));
    let rifle = Item::new(Arc::new(kind("rifle", &["Weapon"], [4, 1])));
    let mut rounds = Item::new(Arc::new(Kind { stack_max: 60, ..kind("round", &["Ammo"], [1, 1]) }));
    rounds.count = 60;
    let case = Item::new(Arc::new(Kind { grids: grid([2, 2]), ..kind("case", &["SimpleContainer"], [1, 1]) }));
    let ids = [bag.id, rifle.id, rounds.id, case.id];
    for item in [bag, rifle, rounds, case] {
        loot.stow(item).unwrap();
    }
    let carried = Carried {
        equipment: Equipment { item: worn },
        names: HashMap::new(),
        loot,
        loot_open: true,
        away: Vec::new(),
        loot_of: 0,
        searched_ground: None,
        repairing: false,
        repairs: Vec::new(),
        repair_settings: None,
        rules: Rules { in_raid: true, backpacks_deep: 0, busy_weapon: None, chamber_closed: None, chamber_held: None, limits: None, own: None },
        fixed_wear: Vec::new(),
        note: String::new(),
        panel: Default::default(),
        ground: Vec::new(),
        thrown: Vec::new(),
        item_looks: Default::default(),
        icons: HashMap::new(),
        pictures: HashMap::new(),
        pictures_on: false,
        picture_failed: Default::default(),
        spinner: Handle::default(),
        bound: Default::default(),
        top_grenade: None,
        magazine_work: None,
        drained: None,
        leaving: Vec::new(),
        frozen_keys: HashMap::new(),
        reload_key: None,
        warning: None,
        switched: Vec::new(),
        fold_asked: None,
        round_times: RoundTimes::default(),
        strength_elite: false,
        carry: 1.0,
        weighed: Default::default(),
        told_weight: 0.0,
        windows: Vec::new(),
        priority: None,
        priority_auto: true,
        asking: None,
        pack_asked: false,
        falling: Vec::new(),
        checked: Default::default(),
        checked_chambers: Default::default(),
        mastery: Default::default(),
        slot_keys: Vec::new(),
        texts: Default::default(),
        named_builds: Vec::new(),
        info_size: None,
        game_grid_window: false,
        tag_size: None,
        tag_limit: crate::screens::health_view::TAG_LIMIT,
        tag_edits: Vec::new(),
        observed: Vec::new(),
        wishlist: Default::default(),
        examiner: Default::default(),
        examine_asked: None,
        searcher: Default::default(),
        search_asked: None,
        search_clock: 0.0,
        search_timer_said: None,
        bodies: Vec::new(),
        searched_body: None,
        search_double: false,
    };
    (carried, ids)
}
