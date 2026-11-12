//! Using a thing, putting it down, unloading and fitting magazines.

use super::*;

/// How a thing is used: by a double click ("Use", where that cannot be
/// "Use All"), or by the menu's "Use" or "Use All".
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Using {
    DoubleClick,
    Use,
    All,
}

/// Medicine, food or drink used from the screen (`ItemView.OnClick`, the
/// install's `ItemQuickUseMode` letting a double click; the menu's lines):
/// a double click is "Use", and where that cannot be, "Use All"
/// (`ItemContextInteractionsSwitcher`). "Use" is medicine on the part the
/// inventory names (`Health::use_part`), and for food or drink whose whole
/// is above 1 the window that asks how much (`ShowSplitMenuDialog`); "Use
/// All" is food or drink whole. Either only if it can do anything now
/// (`HasPartsToApply`), else the refusal is said. Whether the thing was
/// one of those (a double click on one is never taken as anything else).
pub(super) fn use_thing(carried: &mut Carried, id: Id, health: Option<&trp_sim::condition::health::Health>, db: &trp_data::Database, cursor: Vec2, asked: &mut AskedFor, using: Using) -> bool {
    use trp_sim::condition::health::Refused;
    let Some(item) = carried.find(id) else { return false };
    if !(is(item, "FoodDrink") || is(item, "Meds")) {
        return false;
    }
    let (name, template, held) = (carried.name(item), item.kind.template.clone(), item.resource);
    let owned = carried.equipment.item.find(id).is_some();
    let medicine = trp_sim::condition::health::Medicine::of(db, &template);
    carried.note = match (medicine, health) {
        (_, None) => format!("{name}: with health switched off nothing is used"),
        (None, _) => format!("{name}: its figures were not read"),
        (Some(_), _) if !owned => format!("{name}: only what the character has is used here"),
        (Some(medicine), Some(health)) => {
            let resource = held.unwrap_or_else(|| medicine.full());
            let share = medicine.food() && medicine.max_resource > 1.0 && using != Using::All;
            match health.has_parts_to_apply(&medicine, resource) {
                Err(Refused::Dead) => format!("{name}: the dead use nothing"),
                Err(Refused::Incompatible) => format!("{name}: it can do nothing now"),
                Err(Refused::Depleted) => format!("{name}: nothing is left in it"),
                Ok(()) if share => {
                    carried.asking = Asking::of_use(id, medicine.max_resource, resource, cursor);
                    String::new()
                }
                Ok(()) if medicine.food() => {
                    asked.1 = Some((id, None, None));
                    String::new()
                }
                Ok(()) => {
                    asked.1 = Some((id, health.use_part(&medicine, resource), None));
                    String::new()
                }
            }
        }
    };
    true
}

/// The thing put down on the ground a step ahead of where the eyes are
/// (the game's `ThrowItem`, which a raid carries out without asking; the
/// game throws the thing and it falls where it falls). A backpack on the
/// back is not put down at once: its slot is one the game animates, and
/// it goes by the body's gesture for it, as it does by its own key
/// (interactions spec, "A backpack dropped").
pub(super) fn put_down(carried: &mut Carried, id: Id, eye: Option<&GlobalTransform>, floor: f32) {
    if carried.equipment.worn("Backpack").is_some_and(|pack| pack.id == id) {
        carried.pack_asked = true;
        return;
    }
    let Some(eye) = eye else { return };
    let ahead = eye.forward().as_vec3().with_y(0.0).normalize_or_zero();
    let at = Vec3::new(eye.translation().x, floor, eye.translation().z) + ahead * DROP_AHEAD;
    let name = carried.find(id).map(|item| carried.name(item)).unwrap_or_default();
    match take_out(&mut [&mut carried.equipment.item, &mut carried.loot], id, &carried.rules) {
        Ok(item) => {
            // A weapon thrown has its chamber unknown (`ThrowItem`:
            // `CheckChamber(weapon, false)`).
            if is(&item, "Weapon") {
                carried.know_chamber(id, false);
            }
            carried.ground.push((item, at));
            carried.thrown.push((id, crate::world::loose::How::Thrown));
            carried.note = format!("{name}: put down");
        }
        Err(refusal) => carried.note = format!("{name}: {}", why(&refusal, &carried.texts)),
    }
    carried.tidy();
}

/// A thing's menu (`trp_sim::item_menu`): its lines in a raid by what the
/// thing is, where it lies, what the hands hold and what health says, each
/// with its caption in the install's words.
pub(super) fn menu_lines(carried: &Carried, id: Id, held: &HandsHeld, health: Option<&trp_sim::condition::health::Health>, db: &trp_data::Database, said: &crate::screens::health_view::Words) -> Vec<(trp_sim::items::item_menu::Line, trp_sim::items::item_menu::Can, String)> {
    let Some(item) = carried.find(id) else { return Vec::new() };
    // A part inspected from the edit build screen: its menu's one line
    // there (the app's; `edit_build::part_clicks`), which its window does
    // not show as a button.
    if carried.equipment.item.find(id).is_none() && carried.loot.find(id).is_none() {
        let line = trp_sim::items::item_menu::Line::Inspect;
        return vec![(line, Ok(()), said.word(line.name(), line.name()))];
    }
    let equipment = carried.equipment.item.id;
    let place = carried.lies(id);
    let in_hands = held.item.and_then(|weapon| carried.find(weapon));
    let magazine_in_hands = in_hands.and_then(|weapon| weapon.kind.slots.iter().zip(&weapon.slots).find(|(slot, _)| slot.name == "mod_magazine").and_then(|(_, held)| held.as_ref()).map(|magazine| magazine.id));
    let health = health.map(|health| match trp_sim::condition::health::Medicine::of(db, &item.kind.template) {
        Some(medicine) => health.has_parts_to_apply(&medicine, item.resource.unwrap_or_else(|| medicine.full())).map_err(trp_sim::items::item_menu::refusal_key),
        None => Err(trp_sim::items::item_menu::refusal_key(trp_sim::condition::health::Refused::Incompatible)),
    });
    let facts = trp_sim::items::item_menu::Facts {
        worn: matches!(place, Some((holder, Spot::Slot(_))) if holder == equipment),
        fitted: matches!(place, Some((holder, Spot::Slot(_))) if holder != equipment),
        own: carried.equipment.item.find(id).is_some(),
        something_in_hands: held.item.is_some(),
        magazine_in_hands,
        checked: carried.checked.contains(&id),
        // Whether a fold would be refused where it lies, tried on a copy.
        fold_refused: trp_sim::items::inventory::can_fold(item) && {
            let (mut worn, mut loot) = (carried.equipment.item.clone(), carried.loot.clone());
            trp_sim::items::inventory::fold(&mut [&mut worn, &mut loot], id).is_err()
        },
        health,
        unknown: !trp_sim::items::examine::known(item),
        examining: carried.examiner.examining(id),
    };
    trp_sim::items::item_menu::lines(item, &facts).into_iter().map(|(line, can)| (line, can, said.word(line.name(), line.name()))).collect()
}

/// A weapon's magazine taken out by its menu's Unload (`ItemUiContext.
/// UnloadWeapon`): to the first place found for it, the equipment's and,
/// with something open beside it, that's, the side it is from first.
pub(super) fn unload_weapon(carried: &mut Carried, weapon: Id) -> Option<Sound> {
    let magazine = carried.find(weapon).and_then(|gun| gun.slot_index(trp_sim::items::inventory::MAGAZINE_SLOT).and_then(|slot| gun.slots[slot].as_ref()).map(|magazine| magazine.id))?;
    let (worn, loot) = (carried.equipment.item.id, carried.loot.id);
    let targets = match (carried.loot_open, carried.equipment.item.find(magazine).is_some()) {
        (false, _) => vec![worn],
        (true, true) => vec![worn, loot],
        (true, false) => vec![loot, worn],
    };
    let (name, kind) = carried.find(magazine).map(|item| (carried.name(item), item.kind.clone()))?;
    let roots = &mut [&mut carried.equipment.item, &mut carried.loot];
    let done = quick_find(roots, magazine, &targets, Order::UNLOAD_WEAPON, &carried.rules);
    carried.note = match &done {
        Ok(_) => format!("{name}: out of the weapon"),
        Err(refusal) => format!("{name}: {}", why(refusal, &carried.texts)),
    };
    let went = done.ok().map(|(into, applied)| match applied {
        Applied::Put(spot) => Went::To(into, spot),
        _ => Went::Into(into),
    });
    carried.tidy();
    went.map(|went| move_sound(carried, &kind, went.address(carried)))
}

/// A magazine moved into a weapon's empty magazine slot (the menu's Load,
/// Reload with none fitted, Install): a move of one thing.
pub(super) fn fit_magazine(carried: &mut Carried, magazine: Id, weapon: Id) -> Option<Sound> {
    let slot = carried.find(weapon)?.slot_index(trp_sim::items::inventory::MAGAZINE_SLOT)?;
    let (name, kind) = carried.find(magazine).map(|item| (carried.name(item), item.kind.clone()))?;
    let roots = &mut [&mut carried.equipment.item, &mut carried.loot];
    let done = move_item(roots, magazine, weapon, Spot::Slot(slot), &carried.rules);
    carried.note = match &done {
        Ok(()) => format!("{name}: into the weapon"),
        Err(refusal) => format!("{name}: {}", why(refusal, &carried.texts)),
    };
    carried.tidy();
    done.ok().map(|()| move_sound(carried, &kind, Some((weapon, Spot::Slot(slot)))))
}

/// A thing's window opened (`ItemUiContext.OpenItem` for one that holds
/// things, `Inspect` for anything): a container sounds its group's own
/// opening, or the interface's; a window that shows a thing, the
/// inspector's, even where it is open.
pub(super) fn open_view(carried: &mut Carried, id: Id, grids: bool, cursor: Vec2, screen: Option<(Vec2, f32)>) -> Option<Sound> {
    let kind = carried.find(id)?.kind.clone();
    let sound = if grids { Sound::Item(kind.sound.clone(), Way::Open) } else { Sound::Ui(Ui::InspectorOpen) };
    // Where the game opens a window (a container's placed by its own size
    // once it is laid out: `fit_windows`); the app's own look's a little
    // below and right of the mouse.
    let at = match (grids, carried.info_size, screen) {
        (false, Some(size), Some((screen, s))) => game_window_at(carried, size, screen, s),
        (true, _, Some((screen, s))) if carried.game_grid_window => game_window_at(carried, Vec2::ZERO, screen, s),
        _ => cursor + Vec2::new(14.0, 14.0),
    };
    carried.open_window(id, if grids { WinKind::Grids } else { WinKind::Info }, at);
    Some(sound)
}
