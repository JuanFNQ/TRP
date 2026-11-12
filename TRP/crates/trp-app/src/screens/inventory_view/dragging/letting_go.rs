//! A thing let go: used on a part of the HEALTH tab, asked how many of, loaded, mended or moved.

use super::*;

/// What letting go changes and reads.
pub(super) struct Landing<'a, 'w> {
    pub(super) carried: Mut<'a, Carried>,
    pub(super) asked: Mut<'a, AskedFor>,
    pub(super) sounds: &'a mut MessageWriter<'w, Sound>,
    pub(super) controller: &'a Controller,
    pub(super) game: &'a crate::GameData,
    pub(super) in_hands: &'a HandsHeld,
}

/// The thing `id` let go over `target` (none: perhaps a part of the HEALTH tab), with Ctrl held or not.
pub(super) fn let_go(
    (id, name): (Id, String),
    target: Option<Target>,
    ctrl: bool,
    cursor: Vec2,
    drops: &Query<(&crate::screens::health_view::HealthDrop, &RelativeCursorPosition, &InheritedVisibility)>,
    landing: Landing,
) {
    let Landing { mut carried, mut asked, sounds, controller, game, in_hands } = landing;
    // Let go on a part of the HEALTH tab (`HealthBarButton.OnDrop`):
    // medicine, food or drink the character has is used on that part
    // (`HealthController.ApplyItem`: taken in hand as by a quick-use
    // key, the part its list), if it can do anything to it
    // (`CanApplyItem`); else nothing.
    let part = drops.iter().find(|(_, at, shown)| shown.get() && at.cursor_over()).map(|(drop, ..)| trp_sim::condition::health::Part::ALL[drop.0]);
    if let (None, Some(part)) = (target, part) {
        let carried = &mut *carried;
        let medicine = carried.equipment.item.find(id).and_then(|item| Some((item, trp_sim::condition::health::Medicine::of(&game.db, &item.kind.template)?)));
        carried.note = match (medicine, controller.health.as_ref()) {
            (Some((item, medicine)), Some(health)) => {
                let resource = item.resource.unwrap_or_else(|| medicine.full());
                if health.part_to_apply(&medicine, resource, Some(part)).is_some() {
                    asked.1 = Some((id, Some(part), None));
                    String::new()
                } else {
                    format!("{name}: it can do nothing there now")
                }
            }
            (Some(_), None) => format!("{name}: with health switched off nothing is used"),
            (None, _) => format!("{name}: only medicine, food and drink the character has are used on a part"),
        };
        return;
    }
    let Some(target) = target else { return };
    let from = carried.lies(id);
    let carried = &mut *carried;
    let roots = &mut [&mut carried.equipment.item, &mut carried.loot];
    // With Ctrl held: how many may go. More than one, and the window
    // asks how many, beginning at half and one (`SplitDialog.Show`).
    let part = ctrl.then(|| most_of(roots, id, target));
    if let Some(most) = part.filter(|most| *most > 1) {
        let onto = match target {
            Target::Item(_) => true,
            Target::Slot(holder, slot) => in_slot(roots, holder, slot).is_some(),
            Target::Cell(..) => false,
        };
        let kind = roots.iter().find_map(|root| root.find(id)).map(|item| item.kind.clone());
        carried.asking = Some(Asking::of_stack(id, target, onto, most, cursor));
        // Its sound as the window opens: the game plays it as the drag
        // ends, with no place yet, so a drop (`GridView.AcceptItem`,
        // `SlotView.AcceptItem`: `PlayOperationSound`), and nothing when
        // the window goes through or is dropped (`courses10`'s
        // `how-many`: frames 238 and 1171).
        if let Some(kind) = kind {
            sounds.write(move_sound(carried, &kind, None));
        }
        return;
    }
    // Rounds let go on a magazine load it, a round at a time, as many as
    // it has room for (inventory spec, "Loading and unloading a
    // magazine").
    let onto = match target {
        Target::Item(onto) => Some(onto),
        Target::Slot(holder, slot) => in_slot(roots, holder, slot),
        Target::Cell(..) => None,
    };
    // In the screen opened from the workbench's REPAIR GEAR, a repair
    // kit let go on what it mends asks for its repair window
    // (`Carried::repair_asked`), and nothing is moved (the app's).
    if carried.repairing {
        let mends = onto.and_then(|onto| Some((roots.iter().find_map(|root| root.find(id))?, roots.iter().find_map(|root| root.find(onto))?))).filter(|(kit, thing)| kit_mends(kit, thing));
        if let (Some((_, thing)), Some(onto)) = (mends, onto) {
            carried.note = format!("{name}: repairing {}", thing.kind.name);
            carried.open_repair(id, onto);
            return;
        }
    }
    let loading = onto.and_then(|onto| {
        let (rounds, magazine) = (roots.iter().find_map(|root| root.find(id))?, roots.iter().find_map(|root| root.find(onto))?);
        Some((onto, trp_sim::items::inventory::rounds_onto_magazine(magazine, rounds)?, magazine.kind.load_unload_modifier))
    });
    if let Some((magazine, loads, modifier)) = loading {
        carried.note = match loads {
            Ok(count) if count > 0 => {
                let time = carried.round_times.load(modifier);
                carried.magazine_work = Some(MagazineWork::new(trp_sim::MagazineProcess::load(magazine, id, count, time, carried.round_times.load_elite()), count));
                format!("{name}: {count} to load, {time:.3} s a round")
            }
            Ok(_) => format!("{name}: {}", why(&Refusal::MagazineFull, &carried.texts)),
            Err(refusal) => format!("{name}: {}", why(&refusal, &carried.texts)),
        };
        return;
    }
    let kind = roots.iter().find_map(|root| root.find(id)).map(|item| item.kind.clone());
    let done = act(roots, id, target, &carried.rules, part);
    if done.is_ok() {
        carried.known_on_arrival(id);
        // Out of a worn slot, a move the hands must finish (the weapon
        // in hand put elsewhere, `TryRemoveFromHands`; a weapon put
        // into the slot the empty hands draw from, `TrySetInHands`):
        // shown in the slot it left as well until they have (`settle`
        // keeps it while it blinks).
        let worn = carried.equipment.item.id;
        if let Some((holder, Spot::Slot(slot))) = from.filter(|(holder, _)| *holder == worn) {
            let now = carried.lies(id);
            let into = match now {
                Some((at, Spot::Slot(index))) if at == worn => carried.equipment.item.kind.slots.get(index).map(|slot| slot.name.as_str()),
                _ => None,
            };
            let waits = (in_hands.item == Some(id) && now != from) || (in_hands.item.is_none() && into.is_some() && into == in_hands.takes_from);
            if waits {
                carried.leaving.push((id, holder, slot));
            }
        }
    }
    if let (Ok(_), Some(kind)) = (&done, kind) {
        // Where it lies now; a stack merged whole, or a part of one,
        // where the target says.
        let address = part.is_none().then(|| carried.lies(id)).flatten().or_else(|| Went::of(target).address(carried));
        sounds.write(move_sound(carried, &kind, address));
    }
    carried.note = match done {
        Ok(what) => format!("{name}: {what}"),
        Err(refusal) => format!("{name}: {}", why(&refusal, &carried.texts)),
    };
    carried.tidy();
}
