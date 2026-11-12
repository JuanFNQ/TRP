//! The inspect window's parts, its tips and lock tips.

use super::*;

/// What can never be under the mouse's reach (a locked slot's part): a
/// layer no cover is.
pub(super) const OUT_OF_REACH: u8 = 254;

/// A button of the window that shows a thing: the thing, its line, the
/// window's layer among the covers, its box in the window's pixels (left,
/// top, width, height) and its ground's piece where it is lit.
#[derive(Component, Clone, Copy)]
pub(crate) struct InfoButtonHit(pub(super) Id, pub(super) trp_sim::items::item_menu::Line, pub(super) u8, pub(super) [f32; 4], pub(super) Option<usize>);

/// A stretching place of a window that shows a thing: the window, and its
/// sides (`StretchArea.EStretchAreaType`).
#[derive(Component)]
pub(super) struct WinStretch(pub(super) usize, pub(super) u8);

/// A dropdown row's arrow in a window that shows a thing: the thing, the
/// row's place among its figures' rows, and the window's layer.
#[derive(Component)]
pub(super) struct InfoTip(pub(super) Id, pub(super) usize, pub(super) u8);

/// A dropdown row's tooltip while the mouse rests on its arrow, on top
/// where the mouse is (`HoverTrigger`: shown as the mouse comes onto it,
/// shut as it leaves; `CompactCharacteristicDropdownPanel.ShowTooltip`): a
/// list of lines in the tooltip of lines (`MultiLineInfo`: an armour's
/// plates, a round's chances through each class), a list of words in the
/// simple tooltip, a line each (`Tooltip.Show(_availableList)`: an armour's
/// zones, `string.Join("\n")`), at once (`crate::tooltip`).
pub(super) fn info_tips(
    controller: Res<Controller>,
    carried: Res<Carried>,
    game: Res<crate::GameData>,
    (tips, covers): (Query<(&InfoTip, &RelativeCursorPosition)>, Query<(&Cover, &RelativeCursorPosition)>),
    (mut simple, mut multi): (ResMut<crate::ui::tooltip::SimpleTip>, ResMut<crate::ui::tooltip::LinesTip>),
    mut kept: Local<Option<(Id, usize, Vec<trp_sim::items::item_attributes::TipLine>)>>,
) {
    let top = cover_under(covers.iter());
    let under = if windows_up(&controller) { tips.iter().find(|(tip, at)| at.cursor_over() && tip.2 == top).map(|(tip, _)| (tip.0, tip.1)) } else { None };
    let Some((id, row)) = under else {
        *kept = None;
        return;
    };
    // Its lines made as the mouse comes onto it.
    if kept.as_ref().is_none_or(|(was, at, _)| (*was, *at) != (id, row)) {
        let Some(item) = carried.find(id) else { return };
        let figures = trp_sim::items::item_attributes::figures(item, &game.db, &|key| carried.texts.say(key));
        *kept = Some((id, row, figures.rows.get(row).map(|row| row.tip.clone()).unwrap_or_default()));
    }
    let Some((.., lines)) = kept.as_ref().filter(|(.., lines)| !lines.is_empty()) else { return };
    if lines.iter().any(|line| line.divider || line.class.is_some()) {
        multi.show(lines.clone());
    } else {
        let words: Vec<&str> = lines.iter().map(|line| line.text.as_str()).collect();
        simple.show(("info", id ^ ((row as u64) << 48)), words.join("\n"), 0.0);
    }
}

/// A button of a window that shows a thing that cannot be done says why
/// while the mouse is on it, on top there: in the simple tooltip, at once,
/// by the mouse, shut as it leaves (`SimpleContextMenuButton.
/// OnPointerEnter`: `Tooltip.Show(_lockReason)`, `OnPointerExit`: `Close`),
/// as the menu's dimmed line does (`item_menu_view::hover`). The reason
/// read as the mouse comes onto it (`SetButtonInteraction`, as the buttons
/// are made). `courses10`'s `weapon-inspect-menu`, frame 1252: "You can't
/// disassemble an equipped item" on the frame the mouse is on DISASSEMBLE.
#[allow(clippy::too_many_arguments)]
pub(super) fn info_button_tips(
    controller: Res<Controller>,
    carried: Res<Carried>,
    game: Res<crate::GameData>,
    (held, said): (Res<HandsHeld>, Res<crate::screens::health_view::Words>),
    (hits, covers): (Query<(&InfoButtonHit, &RelativeCursorPosition)>, Query<(&Cover, &RelativeCursorPosition)>),
    menu: Res<crate::screens::item_menu_view::ItemMenu>,
    mut simple: ResMut<crate::ui::tooltip::SimpleTip>,
    mut kept: Local<Option<(Id, trp_sim::items::item_menu::Line, String)>>,
) {
    let top = cover_under(covers.iter());
    let under = if windows_up(&controller) && menu.0.as_ref().is_none_or(|open| open.button.is_some()) { hits.iter().find(|(hit, at)| at.cursor_over() && hit.2 == top).map(|(hit, _)| (hit.0, hit.1)) } else { None };
    let Some((id, line)) = under else {
        *kept = None;
        return;
    };
    if kept.as_ref().is_none_or(|(was, was_line, _)| (*was, *was_line) != (id, line)) {
        let lines = menu_lines(&carried, id, &held, controller.health.as_ref(), &game.db, &said);
        let reason = lines.into_iter().find(|(own, ..)| *own == line).and_then(|(_, can, _)| can.err()).filter(|reason| !reason.is_empty()).map(|reason| said.word(reason, reason)).unwrap_or_default();
        *kept = Some((id, line, reason));
    }
    if let Some((.., reason)) = kept.as_ref().filter(|(.., reason)| !reason.is_empty()) {
        let number = trp_sim::items::item_menu::Line::ALL.iter().position(|own| *own == line).unwrap_or(0) as u64;
        simple.show(("info button", id ^ (number << 48)), reason.clone(), 0.0);
    }
}

/// A thing not known's figures (`CompactCharacteristicPanel.Show`,
/// `CharacteristicPanel`, `CompactCharacteristicDropdownPanel` with
/// `examined` off): "?" for each, a bar at nothing, a list "???" with no
/// tooltip, nothing coloured; no durability; a repair buff's row hidden
/// and nothing enhanced (`RepairBuffAttribute`, `Enhancement`: shown only
/// examined).
pub(super) fn unknown_figures(figures: &mut trp_sim::items::item_attributes::Figures) {
    use trp_sim::items::item_attributes::Show;
    figures.durability = None;
    figures.rows.retain(|row| !(row.enhanced && row.id.ends_with("Buff")));
    for row in &mut figures.rows {
        row.colored = None;
        row.enhanced = false;
        row.tip.clear();
        match &mut row.show {
            Show::Bar { base, delta, .. } => {
                (*base, *delta) = (0.0, 0.0);
                row.value = Some("?".to_owned());
            }
            Show::Compact => row.value = Some("?".to_owned()),
            Show::Dropdown => row.value = Some("???".to_owned()),
        }
    }
}

/// What the window that shows a thing holds besides its figures
/// (inspect-window spec), and where each of its slots is in the things:
/// its parts' panel's slots in the game's order (`CreateModSlots`:
/// `AllSlots`, the thing's own slots, then each part's after them, parts
/// of parts in turn, a weapon's chambers last; none for a rocket launcher),
/// each named in the install's words, with its picture's name, locked as a
/// raid locks it (`GetModLockedState`: a slot that is locked, an armour's
/// plate in armour worn, a part not to be changed in a raid, a vital
/// part's slot) and, empty, with its filters' marks; its buttons, the
/// thing's lines but Inspect (`InitInteractionButtonsPanel`); a weapon's
/// mastering, its family's level, the points into it and what the level
/// takes (`skills::Mastery::shown`: a family not begun 0 out of 1,500);
/// the size it is stretched to.
#[allow(clippy::type_complexity)]
pub(crate) fn info_parts(carried: &Carried, item: &Item, held: &HandsHeld, health: Option<&trp_sim::condition::health::Health>, game: &crate::GameData, said: &crate::screens::health_view::Words, stretched: Option<[f32; 2]>) -> (crate::screens::health_view::InfoParts, Vec<(Id, usize)>) {
    use crate::screens::health_view::{InfoButtonLook, InfoMastering, InfoParts, InfoSlot};
    let db = &game.db;
    let is = |thing: &Item, class: &str| thing.kind.class_names.iter().any(|name| name == class);
    let chamber = |holder: &Item, index: usize| holder.kind.slots[index].name.starts_with("patron_in_weapon");
    fn flatten<'a>(slots: Vec<(&'a Item, usize)>, out: &mut Vec<(&'a Item, usize)>) {
        out.extend(slots.iter().copied());
        for (holder, index) in slots {
            if let Some(part) = holder.slots.get(index).and_then(Option::as_ref) {
                flatten((0..part.kind.slots.len()).map(|at| (part, at)).collect(), out);
            }
        }
    }
    let mut all = Vec::new();
    if !item.kind.slots.is_empty() && !is(item, "RocketLauncher") {
        flatten((0..item.kind.slots.len()).filter(|&at| !chamber(item, at)).map(|at| (item, at)).collect(), &mut all);
        all.extend((0..item.kind.slots.len()).filter(|&at| chamber(item, at)).map(|at| (item, at)));
    }
    let worn = matches!(carried.lies(item.id), Some((holder, Spot::Slot(_))) if holder == carried.equipment.item.id);
    let launcher = is(item, "Launcher");
    let unknown = is(item, "Weapon") && !carried.checked_chambers.contains(&item.id);
    let cylinder = fitted_magazine(item).and_then(|magazine| item.find(magazine)).is_some_and(trp_sim::items::inventory::is_cylinder);
    let slots = all
        .iter()
        .map(|(holder, number)| {
            let holder_slot = &holder.kind.slots[*number];
            let part = holder.slots.get(*number).and_then(Option::as_ref);
            let template = db.items.get(&holder.kind.template);
            let armour = template.and_then(|template| trp_sim::condition::armor::ArmorSlot::of(template, &holder_slot.name));
            let locked_slot = armour.as_ref().is_some_and(|slot| slot.locked);
            let plate = part.is_some_and(|part| is(part, "ArmorPlate"));
            let fixed = part.is_some_and(|part| is(part, "Mod") && part.kind.not_raid_moddable);
            let key = slot_name(&holder_slot.name);
            let name = carried.texts.say(key).unwrap_or_else(|| key.to_owned()).to_uppercase();
            // `ModSlotView.Show`: `Slots/` and the slot's `Name` (the first of
            // the weapon's mod kinds its id has: `mod_mount_004` is
            // `mod_mount`), as the caption.
            let picture = if launcher { "grenade_patron_in_weapon".to_owned() } else { key.to_lowercase() };
            // A raid's lock first; else a chamber not checked, in a raid
            // (`IsChamberLocked`): the weapon's own chamber, or a chamber of
            // its cylinder fitted, until the character knows it.
            let raid_locked = (plate && worn && !locked_slot) || locked_slot || fixed || holder_slot.required;
            let unchecked = !raid_locked && unknown && unchecked_slot(&holder_slot.name, cylinder);
            // What it says under the mouse while locked (`GetModLockedState`:
            // a lock's reason, and the part's name for a raid's lock; a
            // chamber not checked's, `unchecked_words`), shown by
            // `ModSlotView.OnPointerEnter`: an armour's slot with a plate in
            // it by the plate's words and the reason (`TryGetArmorSlot`),
            // any other the reason, two line breaks and the name.
            let say = |key: &str| carried.texts.say(key).unwrap_or_else(|| key.to_owned());
            let (reason, named) = if plate && worn && !locked_slot {
                (say("Equipped locked slot"), false)
            } else if locked_slot {
                (say("Locked slot"), false)
            } else if raid_locked {
                (format!("<color=red>{}</color>", say("Raid lock")), true)
            } else {
                (String::new(), false)
            };
            let tip = match part {
                _ if unchecked => unchecked_words(carried, &holder_slot.name),
                _ if !raid_locked => String::new(),
                Some(part) if holder_slot.armour && is(part, "ArmorPlate") => carried.plate_words(part, &holder_slot.name, &reason),
                _ => format!("{reason}\n\n{}", part.filter(|_| named).map(|part| carried.full_name(part)).unwrap_or_default()),
            };
            InfoSlot {
                name,
                picture,
                armour: holder_slot.armour,
                full: part.is_some(),
                locked: raid_locked || unchecked,
                unchecked,
                marks: if part.is_none() { crate::screens::edit_build::filter_marks(db, holder_slot) } else { Vec::new() },
                tip,
            }
        })
        .collect();
    let at = all.iter().map(|(holder, number)| (holder.id, *number)).collect();
    let buttons = menu_lines(carried, item.id, held, health, db, said)
        .into_iter()
        .filter(|(line, ..)| *line != trp_sim::items::item_menu::Line::Inspect)
        .map(|(line, can, caption)| InfoButtonLook { name: line.name(), caption, enabled: can.is_ok(), arrow: line.has_lines() })
        .collect();
    let mastering = is(item, "Weapon").then(|| {
        let shown = carried.mastery.shown(&item.kind.template);
        InfoMastering { level: shown.level, points: shown.into, most: shown.takes as f32 }
    });
    (InfoParts { slots, buttons, mastering, stretched }, at)
}

/// Whether a slot of a weapon is one whose lock a check of its chamber
/// lifts (`ItemSpecificationPanel.IsChamberLocked`): a chamber's
/// (`chamber…`, `patron_in_weapon…`), or one of its cylinder's
/// (`camora…`) where the weapon's magazine is a cylinder.
pub(super) fn unchecked_slot(name: &str, cylinder: bool) -> bool {
    name.starts_with("chamber") || name.starts_with("patron_in_weapon") || (cylinder && name.starts_with("camora"))
}

/// The words of a chamber not checked's tooltip (`GetModLockedState`;
/// `TooltipData.ToString`: its error, two line breaks, and no name): the
/// drum's for a chamber of a cylinder, else the chamber's, in orange.
fn unchecked_words(carried: &Carried, slot: &str) -> String {
    let key = if slot.starts_with("camora") { "You need to check the revolver drum" } else { "You need to check chamber in weapon" };
    format!("<color=#d77400>{}</color>\n\n", carried.texts.say(key).unwrap_or_else(|| key.to_owned()))
}

/// A locked slot in a window that shows a thing, for its tooltip: the
/// thing, the slot's place among the window's, its words and the window's
/// layer.
#[derive(Component)]
pub(super) struct InfoLockTip(pub(super) Id, pub(super) usize, pub(super) String, pub(super) u8);

/// A locked slot says why 0.6 s after the mouse comes onto it, on top
/// there, by the mouse, shut as it leaves (`ModSlotView.OnPointerEnter`: a
/// locked slot's `Tooltip.Show(text, null, 0.6)`, an armour's plate's
/// words with the reason, `OnPointerExit`: `Close`; its root takes the
/// mouse where its locked pieces do not): a chamber not checked; a
/// built-in insert ("Built-in armor components cannot be changed"), a
/// plate in armour worn, a part not to be changed in a raid or a vital
/// part, such as aramid slots or parts that cannot be taken off
/// (`InfoSlot::tip`). Not in `courses10`'s take (the mouse never stayed so
/// long on one): read from the code alone.
pub(super) fn info_lock_tips(controller: Res<Controller>, tips: Query<(&InfoLockTip, &RelativeCursorPosition)>, covers: Query<(&Cover, &RelativeCursorPosition)>, menu: Res<crate::screens::item_menu_view::ItemMenu>, mut simple: ResMut<crate::ui::tooltip::SimpleTip>) {
    if !windows_up(&controller) || menu.0.is_some() {
        return;
    }
    let top = cover_under(covers.iter());
    if let Some((tip, _)) = tips.iter().find(|(tip, at)| at.cursor_over() && tip.3 == top) {
        simple.show(("info lock", tip.0 ^ ((tip.1 as u64) << 48)), tip.2.clone(), ITEM_TIP_DELAY);
    }
}

/// A slot's name as the game names it (`Slot.Name`): the first of the
/// kinds of weapon part (`EWeaponModType`, in its order) its own name
/// holds, so `mod_mount_004` is `mod_mount` ("MOUNT"); else its own name.
pub(crate) fn slot_name(id: &str) -> &str {
    const KINDS: [&str; 29] = [
        "mod_mount", "mod_scope", "mod_tactical", "mod_stock", "mod_magazine", "mod_barrel", "mod_handguard", "mod_muzzle", "mod_sight_front", "mod_sight_rear", "mod_foregrip", "mod_reciever", "mod_charge", "mod_pistol_grip", "mod_launcher",
        "mod_bipod", "mod_mag_shaft", "mod_silencer", "mod_tactical_2", "chamber0", "chamber1", "patron_in_weapon", "mod_gas_block", "mod_equipment", "mod_equipment_000", "mod_equipment_001", "mod_nvg", "mod_flashlight", "mod_muzzle_001",
    ];
    KINDS.iter().find(|kind| id.contains(**kind)).copied().unwrap_or(id)
}

/// A container's slots shown with it as its special slots: every one but
/// an armour's (`SearchableSlotView.CreateSlots` leaves out each
/// `ArmorSlot`: an armoured rig's plates and inserts show only where the
/// thing is looked at or modded).
/// The game's window of a container's grids laid out for a container,
/// where the game's window was read (`TARKOV UI`): its caption the
/// container's short name, in the locale's case.
pub(super) fn grid_built(look: &crate::screens::health_view::HealthLook, carried: &Carried, item: &Item, picked: bool, said: &crate::screens::health_view::Words) -> Option<crate::screens::health_view::GridWindowBuilt> {
    let cells: Vec<[u32; 2]> = item.kind.grids.iter().map(|grid| grid.cells).collect();
    // A searchable thing's window has its search (`GridWindow`'s
    // `SearchableView`): a bag found elsewhere is opened by it.
    let search = trp_sim::items::search::searchable(item).then(|| search_shown(carried, item));
    crate::screens::health_view::grid_window_built(look, &carried.name(item), &item.kind.layout, &cells, picked, search.as_ref(), said)
}

pub(super) fn special_slots(item: &Item) -> Vec<usize> {
    (0..item.kind.slots.len()).filter(|&index| !item.kind.slots[index].armour).collect()
}
