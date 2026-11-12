//! A weapon's settings kept on its items: its fire mode, the sight it aims by and each sight's scope, mode, zeroing and zoom.

use bevy::prelude::*;
use trp_sim::items::inventory::{self as inv, Id, Item};
use trp_sim::weapons::sights::{SightKept, Sights};

use crate::screens::inventory_view::{Carried, HandsHeld};
use crate::weapons::weapon::{HandsRig, Stowed};

pub struct KeptSettingsPlugin;

impl Plugin for KeptSettingsPlugin {
    fn build(&self, app: &mut App) {
        // After the hands have settled what is in them, before the weapon's
        // clips play, as the weapon's wear is kept (`wear::keep_wear`).
        app.add_systems(Update, keep_settings.after(crate::screens::inventory_view::ScreenDrawn).before(crate::player::animation::play_clips));
    }
}

/// The weapon whose items' settings the hands were last given, by its
/// number and its build, and the item each of its sight parts is.
#[derive(Default)]
struct Given {
    weapon: Option<(Id, String)>,
    sights: Vec<Option<Id>>,
}

/// The item each sight part of a weapon's rig is, in the parts' order. The
/// rig is made from the build's key and knows no item's number, so each
/// part is the next item of its template in the game's walk of the
/// weapon's items (`CompoundItem.AllSlots`), which is the parts' order.
fn sight_items(db: &trp_data::Database, weapon: &Item, sights: &Sights) -> Vec<Option<Id>> {
    let build = inv::build_of(weapon);
    let walked = trp_data::weapons::sight::all_slots_order(db, &build);
    let mut taken = vec![false; walked.len()];
    let parts = sights.parts().iter();
    parts
        .map(|part| {
            let Some(at) = (0..walked.len()).find(|&i| !taken[i] && walked[i].template == part.template) else {
                // The weapon itself, where its own template is a sight's.
                return (weapon.kind.template == part.template).then_some(weapon.id);
            };
            taken[at] = true;
            walked[at].id.parse().ok()
        })
        .collect()
}

/// The weapon in hand takes its settings from its items as it is taken,
/// and its items keep them as they change (weapon-handling spec, "What
/// the items keep"). The game's are the items' own components, so a
/// weapon put away and drawn again, or another of the same build, has its
/// own; here the hands' logic and sights are a rig's, which weapons of
/// one build share.
fn keep_settings(mut carried: ResMut<Carried>, held: Res<HandsHeld>, game: Res<crate::GameData>, mut weapon: Option<ResMut<HandsRig>>, stowed: Option<Res<Stowed>>, mut given: Local<Given>) {
    let Some(rig) = weapon.as_deref().filter(|rig| rig.firearm.is_some()) else {
        given.weapon = None;
        return;
    };
    let Some(item) = held.item.and_then(|id| carried.equipment.item.find(id)).filter(|item| inv::is(item, "Weapon")) else {
        given.weapon = None;
        return;
    };
    // A weapon on its way into the hands is named before its rig is
    // there: until then the rig in the hands is another weapon's.
    let build = inv::rig_key(item);
    if stowed.as_deref().and_then(|stowed| stowed.held()).is_some_and(|held| held != build) {
        return;
    }
    let id = item.id;
    if given.weapon.as_ref() != Some(&(id, build.clone())) {
        // Taken in hand, or built anew: the hands have its items' settings.
        given.sights = rig.hands.sights().map_or_else(Vec::new, |sights| sight_items(&game.db, item, sights));
        let kept: Vec<SightKept> = given.sights.iter().map(|part| part.and_then(|part| item.find(part)).map_or_else(SightKept::default, |part| part.sight.clone())).collect();
        let (mode, aim_index) = (item.fire_mode, item.aim_index);
        if let Some(rig) = weapon.as_deref_mut() {
            if let Some(firearm) = rig.firearm.as_mut() {
                firearm.start_fire_mode(mode);
            }
            if let Some(sights) = rig.hands.settings.sights.as_mut() {
                sights.take_kept(aim_index, &kept);
            }
            debug!("weapon {id} in hand as its items keep it: its fire mode {:?}, its sight {aim_index}, {} sight parts", rig.firearm.as_ref().map(|firearm| firearm.mode()), kept.len());
        }
        given.weapon = Some((id, build));
        return;
    }
    // What changed in the hands goes back onto the items. A launcher's
    // sight in use is not the weapon's choice of sight.
    let Some(firearm) = rig.firearm.as_ref() else { return };
    let mode = firearm.mode();
    let sights = rig.hands.sights();
    let aim_index = sights.filter(|sights| !sights.grenade()).map_or(item.aim_index, Sights::aim_index);
    let changed: Vec<(Id, SightKept)> = sights.map_or_else(Vec::new, |sights| {
        let differs = |p: usize, part: Id| item.find(part).is_some_and(|found| !sights.keeps(p, &found.sight));
        let parts = given.sights.iter().enumerate().filter_map(|(p, part)| part.map(|part| (p, part)));
        parts.filter(|(p, part)| differs(*p, *part)).map(|(p, part)| (part, sights.kept(p))).collect()
    });
    if item.fire_mode == Some(mode) && item.aim_index == aim_index && changed.is_empty() {
        return;
    }
    // Nothing drawn goes by these, so what watches the inventory is not
    // woken for them.
    let Some(item) = carried.bypass_change_detection().equipment.item.find_mut(id) else { return };
    (item.fire_mode, item.aim_index) = (Some(mode), aim_index);
    for (part, kept) in changed {
        if let Some(part) = item.find_mut(part) {
            part.sight = kept;
        }
    }
}
