//! The right-click menu's lines, clicks and move sounds.

use super::*;

/// What a line of the menu does, of those a raid shows (inventory spec,
/// "The menu of a right click"): Inspect opens the window that shows the
/// thing, Open its own window, Equip puts it on (as Alt and a click),
/// Unequip takes it off to the first place found for it (`Uninstall`:
/// `QuickFindAppropriatePlace`, as Ctrl and a click), Use and Use All use
/// it, Discard puts it down; the rest are not built, and say so.
#[allow(clippy::too_many_arguments)]
pub(super) fn do_line(
    carried: &mut Carried,
    id: Id,
    line: trp_sim::items::item_menu::Line,
    caption: &str,
    cursor: Vec2,
    screen: Option<(Vec2, f32)>,
    health: Option<&trp_sim::condition::health::Health>,
    db: &trp_data::Database,
    asked: &mut AskedFor,
    eye: Option<&GlobalTransform>,
    floor: f32,
    in_hands: Option<Id>,
) -> Option<Sound> {
    use trp_sim::items::item_menu::Line;
    match line {
        Line::Inspect => open_view(carried, id, false, cursor, screen),
        // An examine asked (`ItemUiContext.Examine`; `examine_view`).
        Line::Examine => {
            carried.examine_asked = Some(id);
            None
        }
        Line::Open => open_view(carried, id, true, cursor, screen),
        Line::Equip => click(carried, id, false, [false, false, true], cursor, screen),
        Line::Unequip => click(carried, id, false, [true, false, false], cursor, screen),
        Line::Use | Line::UseAll => {
            use_thing(carried, id, health, db, cursor, asked, if line == Line::Use { Using::Use } else { Using::All });
            None
        }
        Line::Discard => {
            put_down(carried, id, eye, floor);
            None
        }
        // A weapon's magazine out, in or swapped from the menu (firing
        // spec, "A magazine taken out or put in from the inventory"): a move
        // of one thing on the weapon in hand is its hands' (`settle`); a
        // swap of two is made at once.
        Line::Unload => unload_weapon(carried, id),
        Line::Load => {
            let next = carried.find(id).and_then(|weapon| trp_sim::items::inventory::magazine_for(&carried.equipment.item, weapon));
            match next {
                Some(next) => fit_magazine(carried, next, id),
                None => {
                    let name = carried.find(id).map(|item| carried.name(item)).unwrap_or_default();
                    carried.note = format!("{name}: can't find any non-empty magazine");
                    None
                }
            }
        }
        Line::Reload => {
            let name = carried.find(id).map(|item| carried.name(item)).unwrap_or_default();
            match trp_sim::items::inventory::menu_reload(&mut carried.equipment.item, id) {
                Ok(trp_sim::items::inventory::MenuReload::Swapped(_, new)) => {
                    carried.note = format!("{name}: magazines swapped");
                    carried.tidy();
                    let kind = carried.find(new).map(|magazine| magazine.kind.clone())?;
                    let address = carried.lies(new);
                    Some(move_sound(carried, &kind, address))
                }
                Ok(trp_sim::items::inventory::MenuReload::Fit(next)) => fit_magazine(carried, next, id),
                Err(Refusal::NoAction) => {
                    carried.note = format!("{name}: can't find any non-empty magazine");
                    None
                }
                Err(Refusal::NoRoom) => {
                    carried.note = format!("{name}: no place for the old magazine (the game throws it down, which is not built)");
                    None
                }
                Err(refusal) => {
                    carried.note = format!("{name}: {}", why(&refusal, &carried.texts));
                    None
                }
            }
        }
        Line::Install => {
            let name = carried.find(id).map(|item| carried.name(item)).unwrap_or_default();
            match trp_sim::items::inventory::menu_install(&mut carried.equipment.item, id, in_hands) {
                Ok(trp_sim::items::inventory::MenuInstall::Swapped(..)) => {
                    carried.note = format!("{name}: put in, the other magazine swapped out");
                    carried.tidy();
                    let kind = carried.find(id).map(|magazine| magazine.kind.clone())?;
                    let address = carried.lies(id);
                    Some(move_sound(carried, &kind, address))
                }
                Ok(trp_sim::items::inventory::MenuInstall::Fit(weapon, _)) => fit_magazine(carried, id, weapon),
                Err(Refusal::NoAction) => {
                    carried.note = format!("{name}: can't find appropriate slot");
                    None
                }
                Err(refusal) => {
                    carried.note = format!("{name}: {}", why(&refusal, &carried.texts));
                    None
                }
            }
        }
        Line::Uninstall => click(carried, id, false, [true, false, false], cursor, screen),
        // A switch flipped (`ItemUiContext.ToggleItem`): at once; the gesture
        // and the sound, for one worn on the head, are `settle`'s.
        Line::TurnOn | Line::TurnOff => {
            let name = carried.find(id).map(|item| carried.name(item)).unwrap_or_default();
            let switched = trp_sim::items::inventory::toggle(&mut carried.equipment.item, id).or_else(|| trp_sim::items::inventory::toggle(&mut carried.loot, id));
            if let Some((part, on)) = switched {
                carried.switched.push((part, on));
                carried.note = format!("{name}: {}", if on { "on" } else { "off" });
            }
            None
        }
        // A magazine checked (`ItemUiContext.CheckMagazine`; inventory spec,
        // "Checking a magazine"): one wait, as a load's process is kept and
        // stopped; nothing if it is known already.
        Line::CheckMagazine => {
            let (name, modifier) = carried.find(id).map(|item| (carried.name(item), item.kind.check_modifier)).unwrap_or_default();
            if !carried.checked.contains(&id) {
                let time = carried.round_times.check(modifier);
                carried.magazine_work = Some(MagazineWork::new(trp_sim::MagazineProcess::check(id, time), 1));
                carried.note = format!("{name}: checking, {time:.2} s");
            }
            None
        }
        // A fold turned (`ItemUiContext.FoldItem`; inventory spec, "A fold
        // from the menu"): the stock's sound as the line is done; on the
        // weapon in hand the hands are asked (`settle`), anywhere else it
        // is turned at once, refused where a weapon in a grid has no room
        // for its new size.
        Line::Fold | Line::Unfold => {
            let name = carried.find(id).map(|item| carried.name(item)).unwrap_or_default();
            let on_hands = in_hands.is_some_and(|weapon| carried.equipment.item.find(weapon).is_some_and(|weapon| weapon.find(id).is_some()));
            if on_hands {
                carried.fold_asked = Some(id);
            } else {
                let roots = &mut [&mut carried.equipment.item, &mut carried.loot];
                carried.note = match trp_sim::items::inventory::fold(roots, id) {
                    Ok((_, folded)) => format!("{name}: {}", if folded { "folded" } else { "unfolded" }),
                    Err(_) => format!("{name}: can't fold the stock now"),
                };
                carried.tidy();
            }
            Some(Sound::Ui(Ui::MenuStock))
        }
        // A tag edited in its window (`ItemUiContext.EditTag`; inventory
        // spec, "A tag"), or reset at once (`ResetTag`: no name, colour 0).
        Line::Tag => {
            open_tag(carried, id, cursor, screen);
            None
        }
        Line::ResetTag => {
            let name = carried.find(id).map(|item| carried.name(item)).unwrap_or_default();
            if trp_sim::items::inventory::set_tag(&mut carried.equipment.item, id, "", 0) || trp_sim::items::inventory::set_tag(&mut carried.loot, id, "", 0) {
                carried.note = format!("{name}: tag reset");
            }
            None
        }
        // A weapon taken apart (`ItemUiContext.Disassemble`; inventory
        // spec, "Disassemble"): its sound, then each of its own parts sent
        // where Ctrl and a click sends it, those that may not come off here
        // staying.
        Line::Disassemble => {
            let name = carried.find(id).map(|item| carried.name(item)).unwrap_or_default();
            let targets = carried.other_side(id);
            let roots = &mut [&mut carried.equipment.item, &mut carried.loot];
            let came = trp_sim::items::inventory::disassemble(roots, id, &targets, Order::OTHER_SIDE, &carried.rules);
            let off = came.iter().filter(|(_, done)| done.is_ok()).count();
            carried.note = format!("{name}: {off} of its {} parts off", came.len());
            carried.tidy();
            Some(Sound::Ui(Ui::MenuWeaponDisassemble))
        }
        // A magazine unloaded a round at a time (`ItemUiContext.UnloadAmmo`;
        // inventory spec, "Loading and unloading a magazine").
        Line::UnloadAmmo if carried.find(id).is_some_and(|item| is(item, "Magazine") && trp_sim::items::inventory::magazine_rounds(item) > 0) => {
            let modifier = carried.find(id).map_or(0.0, |item| item.kind.load_unload_modifier);
            let time = carried.round_times.unload(modifier);
            let rounds = carried.find(id).map_or(0, trp_sim::items::inventory::magazine_rounds);
            carried.magazine_work = Some(MagazineWork::new(trp_sim::MagazineProcess::unload(id, time, None), rounds));
            let name = carried.find(id).map(|item| carried.name(item)).unwrap_or_default();
            carried.note = format!("{name}: unloading, {time:.3} s a round");
            None
        }
        _ => {
            let name = carried.find(id).map(|item| carried.name(item)).unwrap_or_default();
            carried.note = format!("{name}: {} is not built yet", caption.to_uppercase());
            None
        }
    }
}

/// A click on a thing (`ItemView.OnClick`, the left button): a double
/// click with neither Ctrl nor Shift held opens a container in a window
/// of its own and shows what anything else is (medicine, food and drink
/// are used before this is asked: `use_by_double_click`); else Ctrl
/// sends the thing to the other side, Shift to the sorting table (a raid
/// has none), and Alt puts it on.
pub(super) fn click(carried: &mut Carried, id: Id, double: bool, [ctrl, shift, alt]: [bool; 3], cursor: Vec2, screen: Option<(Vec2, f32)>) -> Option<Sound> {
    let item = carried.find(id)?;
    let (name, kind) = (carried.name(item), item.kind.clone());
    if double && !ctrl && !shift {
        let grids = Carried::opens(item);
        return open_view(carried, id, grids, cursor, screen);
    }
    let targets = carried.other_side(id);
    let worn = carried.equipment.item.id;
    let roots = &mut [&mut carried.equipment.item, &mut carried.loot];
    // What it came to, and where it went for its sound.
    let (note, went) = if ctrl {
        match quick_find(roots, id, &targets, Order::OTHER_SIDE, &carried.rules) {
            Ok((into, Applied::Merged)) => ("merged".to_owned(), Some(Went::Into(into))),
            Ok((into, Applied::Topped(count))) => (format!("{count} handed over"), Some(Went::Into(into))),
            Ok((into, Applied::Put(spot))) => ("sent over".to_owned(), Some(Went::To(into, spot))),
            Err(refusal) => {
                // A quick move refused is a warning among the notices,
                // in the error's own words (`ItemUiContext.
                // QuickFindAppropriatePlace` with its warnings shown):
                // no room (`NoFreeSpaceError`), nothing to look in
                // (`NoPossibleActionsError`), else the refusal's.
                let said = |key: &str, or: &str| carried.texts.say(key).unwrap_or_else(|| or.to_owned());
                carried.warning = Some(match refusal {
                    Refusal::NoRoom => said("No free room for that item", "No space for this item"),
                    Refusal::NoAction => said("InventoryError/NoPossibleActions", "No actions available"),
                    _ => why(&refusal, &carried.texts),
                });
                (why(&refusal, &carried.texts), None)
            }
        }
    } else if shift {
        ("there is no sorting table in a raid".to_owned(), None)
    } else if alt {
        match equip(roots, id, &carried.rules) {
            Ok(slot) => ("put on".to_owned(), Some(Went::To(worn, Spot::Slot(slot)))),
            Err(Refusal::AlreadyThere) => ("it is on already".to_owned(), None),
            Err(Refusal::NoRoom) => ("no free slot for it".to_owned(), None),
            Err(Refusal::NoAction) => ("it is not a thing to put on".to_owned(), None),
            Err(refusal) => (why(&refusal, &carried.texts), None),
        }
    } else if carried.magazine_work.as_ref().is_some_and(|work| work.process.magazine == id) {
        // A plain click on a magazine being worked stops it (`ItemView`:
        // `StopProcesses`).
        carried.magazine_work = None;
        ("stopped".to_owned(), None)
    } else {
        return None;
    };
    carried.note = format!("{name}: {note}");
    carried.tidy();
    went.map(|went| move_sound(carried, &kind, went.address(carried)))
}

/// Where a thing went: into a place of a holder, or into another thing
/// (a stack it merged with: where that lies).
#[derive(Clone, Copy)]
pub(super) enum Went {
    To(Id, Spot),
    Into(Id),
}

impl Went {
    pub(super) fn address(self, carried: &Carried) -> Option<(Id, Spot)> {
        match self {
            Went::To(holder, spot) => Some((holder, spot)),
            Went::Into(other) => carried.lies(other),
        }
    }

    /// Where a drop on that target puts a thing.
    pub(super) fn of(target: Target) -> Self {
        match target {
            Target::Item(onto) => Went::Into(onto),
            Target::Cell(holder, grid, place) => Went::To(holder, Spot::Cell(grid, place)),
            Target::Slot(holder, slot) => Went::To(holder, Spot::Slot(slot)),
        }
    }
}

/// The sound of a thing of that kind gone to a place
/// (`ItemUiContext.PlayOperationSound`): a part into a slot of a weapon
/// sounds the interface's installing (a magazine's `MenuInstallMag`, else
/// its class's: `ItemViewFactory.GetModClass`, `GUISounds.PlayItemSound`, a
/// vital part's, a gear part's, any other part's the functional one's);
/// anything else its group's `use` into a slot of the equipment or into a
/// magazine, its `drop` anywhere else (a grid, a slot of another thing).
pub(super) fn move_sound(carried: &Carried, kind: &trp_sim::items::inventory::Kind, address: Option<(Id, Spot)>) -> Sound {
    let of = |class: &str| kind.class_names.iter().any(|name| name == class);
    let parent = address.filter(|(_, spot)| !matches!(spot, Spot::Cell(..))).and_then(|(holder, _)| carried.find(holder));
    if parent.is_some_and(|parent| is(parent, "Weapon")) && of("Mod") {
        let ui = if of("Magazine") {
            Some(Ui::InstallMag)
        } else if of("FunctionalMod") || of("AuxiliaryMod") || of("ArmorPlate") {
            Some(Ui::InstallModFunc)
        } else if of("GearMod") {
            Some(Ui::InstallModGear)
        } else if of("MasterMod") {
            Some(Ui::InstallModVital)
        } else {
            None
        };
        if let Some(ui) = ui {
            return Sound::Ui(ui);
        }
    }
    let used = parent.is_some_and(|parent| parent.id == carried.equipment.item.id || is(parent, "Magazine"));
    Sound::Item(kind.sound.clone(), if used { Way::Use } else { Way::Drop })
}

/// Where the game opens a window (`ItemUiContext.OpenWindow`), so large in
/// units, on a screen so large in pixels, `s` to a unit: 40 units right
/// of and below the top left corner of the window opened last of those
/// open, or with none open in the middle of the screen; moved back onto
/// the screen where it would leave it (`CorrectPositionResolution`).
pub(super) fn game_window_at(carried: &Carried, size: Vec2, screen: Vec2, s: f32) -> Vec2 {
    let size = size * s;
    let at = match carried.windows.iter().max_by_key(|window| window.opened) {
        Some(last) => last.at + Vec2::new(40.0, 40.0) * s,
        None => (screen - size) / 2.0,
    };
    at.min(screen - size).max(Vec2::ZERO)
}
