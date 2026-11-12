//! Acting on things, refusals and spawning the screen's nodes.

use super::*;

/// Carries a drop out on the things given: on another item, on a cell or
/// on a slot (a slot that holds something stands for what it holds, as in
/// the game). Says what it came to.
pub(super) fn act(roots: &mut [&mut Item], id: Id, target: Target, rules: &Rules, part: Option<u32>) -> Result<&'static str, Refusal> {
    // A thing not known goes to no slot's place, rounds too, whoever owns
    // it (`ExecutePossibleAction` to a slot's address, before all else).
    if let Target::Slot(holder, slot) = target {
        let unknown = roots.iter().find_map(|root| root.find(id)).is_some_and(|item| !trp_sim::items::examine::known(item));
        if unknown && in_slot(roots, holder, slot).is_none() {
            return Err(Refusal::NotExamined { equipment: false });
        }
    }
    // With Ctrl held only part of a stack goes (the game's "partial
    // transfer only"): so many split off into a place, or handed to a
    // stack of the kind; a thing that is no stack goes nowhere.
    if let Some(count) = part {
        return part_of(roots, id, target, count, rules);
    }
    let applied = |done: Applied| match done {
        Applied::Merged => "merged",
        Applied::Put(_) => "put in",
        Applied::Topped(_) => "filled up",
    };
    match target {
        Target::Item(onto) => apply_item(roots, id, onto, rules).map(applied),
        Target::Cell(holder, grid, place) => move_item(roots, id, holder, Spot::Cell(grid, place), rules).map(|()| "moved"),
        Target::Slot(holder, slot) => {
            let held = roots.iter().find_map(|root| root.find(holder)).and_then(|holder| holder.slots.get(slot)?.as_ref().map(|item| item.id));
            match held {
                Some(onto) if onto == id => Err(Refusal::AlreadyThere),
                Some(onto) => apply_item(roots, id, onto, rules).map(applied),
                None => move_item(roots, id, holder, Spot::Slot(slot), rules).map(|()| "moved"),
            }
        }
    }
}

/// What a slot's own target stands for: the thing in it, if there is one.
pub(super) fn in_slot(roots: &[&mut Item], holder: Id, slot: usize) -> Option<Id> {
    roots.iter().find_map(|root| root.find(holder)).and_then(|holder| holder.slots.get(slot)?.as_ref().map(|item| item.id))
}

/// How many of a stack may go to a target with Ctrl held: onto a stack of
/// its kind as many as that has room for, into a place all but one.
pub(super) fn most_of(roots: &[&mut Item], id: Id, target: Target) -> u32 {
    match target {
        Target::Item(onto) => transfer_most(roots, id, onto),
        Target::Cell(_, grid, place) => split_most(roots, id, Spot::Cell(grid, place)),
        Target::Slot(holder, slot) => match in_slot(roots, holder, slot) {
            Some(onto) => transfer_most(roots, id, onto),
            None => split_most(roots, id, Spot::Slot(slot)),
        },
    }
}

/// So many of a stack to a target.
fn part_of(roots: &mut [&mut Item], id: Id, target: Target, count: u32, rules: &Rules) -> Result<&'static str, Refusal> {
    if count == 0 || count > most_of(roots, id, target) {
        return Err(Refusal::NoAction);
    }
    let onto = match target {
        Target::Item(onto) => Some(onto),
        Target::Slot(holder, slot) => in_slot(roots, holder, slot),
        Target::Cell(..) => None,
    };
    match (onto, target) {
        (Some(onto), _) => transfer_item(roots, id, onto, count, rules).map(|()| "handed over"),
        (None, Target::Cell(holder, grid, place)) => split_item(roots, id, count, holder, Spot::Cell(grid, place), rules).map(|_| "split"),
        (None, Target::Slot(holder, slot)) => split_item(roots, id, count, holder, Spot::Slot(slot), rules).map(|_| "split"),
        (None, Target::Item(_)) => Err(Refusal::NoAction),
    }
}

/// A refusal in words: the app's, or the game's own line where it is the
/// install's (the conflicting parts, a locked slot, a worn armour's
/// plates, the raid's limits: `InventoryError.GetLocalizedDescription`).
pub(crate) fn why(refusal: &Refusal, texts: &trp_data::character::inventory::ItemTexts) -> String {
    let said = |key: &str, or: &str| texts.say(key).unwrap_or_else(|| or.to_owned());
    let short = |template: &str| texts.say(&format!("{template} ShortName")).unwrap_or_else(|| template.to_owned());
    match refusal {
        Refusal::Locked => said("Inventory Errors/Slot is locked", "the slot is locked"),
        // (`ConflictingItemError`: the one there first, then the one coming.)
        Refusal::Conflicting { existing, new } => said("Inventory Errors/Conflicting Items", "{0} and {1} cannot be on it together").replace("{0}", &short(existing)).replace("{1}", &short(new)),
        Refusal::PlateWorn => said("Equipped locked slot", "not while the armour is worn"),
        // (`CountLimitError`: the thing's whole name, then the limit.)
        Refusal::OverLimit { template, limit } => {
            let name = texts.names.get(template).cloned().unwrap_or_else(|| template.clone());
            said("Inventory Errors/Exceeded the maximum limit of item {0} in the inventory ({1} max)", "no more than {1} of {0} in a raid").replace("{0}", &name).replace("{1}", &limit.to_string())
        }
        Refusal::NoSuchPlace => "there is no such place".into(),
        Refusal::Taken => "the place is taken".into(),
        Refusal::Filter => "it does not go there".into(),
        Refusal::Blocked(by) => format!("what is in {by} is in the way"),
        Refusal::WouldBlock(slot) => format!("it would be in the way of what is in {slot}"),
        Refusal::Stack => "a slot takes one thing, not a stack".into(),
        Refusal::OutsideGrid => "it does not fit there".into(),
        Refusal::NoRoom => "no room".into(),
        Refusal::AlreadyThere => "it is there already".into(),
        Refusal::Cycle => "it cannot go into itself".into(),
        Refusal::TooDeep => "backpacks do not go that deep into one another".into(),
        Refusal::Vital(slot) => format!("the part in {slot} stays on in a raid"),
        Refusal::NotInRaid => "that part is not changed in a raid".into(),
        Refusal::MissingParts => "it lacks a part it cannot do without".into(),
        Refusal::Dogtag => "the dogtag's slot takes nothing".into(),
        Refusal::KeptOn(slot) => format!("it stays in {slot} through a raid"),
        Refusal::NoAction => "nothing to be done with the one on the other".into(),
        Refusal::HandsBusy => "the hands are busy with the weapon".into(),
        Refusal::Malfunction => "you must fix malfunctions first".into(),
        // (`InstallNotExaminedError`, `ItemNotExaminedError`, `TakeNotExaminedError`.)
        Refusal::NotExamined { equipment: true } => said("Inventory Errors/Not examined equip", "Cannot equip unexamined item"),
        Refusal::NotExamined { equipment: false } => said("Inventory Errors/Not examined install", "Cannot install unexamined item"),
        Refusal::TakeNotExamined { equipment: true } => said("Inventory Errors/Take not examined equip", "You can't take off unexamined equipment"),
        Refusal::TakeNotExamined { equipment: false } => said("Inventory Errors/Take not examined install", "Item is not examined"),
        Refusal::MagazineFull => said("Inventory Errors/Magazine is full", "Magazine is full"),
        Refusal::WrongAmmo => said("Inventory Errors/Ammo is incompatible", "Wrong ammo type"),
    }
}

/// Where a thing carried is held: over a slot, or over a grid or a thing
/// in one. Each of the game's two views shows a refusal's words by a list
/// of its own.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Over {
    Slot,
    Grid,
}

/// What the game says at the mouse over a slot that refuses the thing
/// carried (`SlotView.HighlightItemViewPosition`: `SimpleTooltip.
/// ShowInventoryError`, the error's own words, `GetLocalizedDescription`,
/// in `<color=red>`; a key the install lacks said as it is, as `Localized`
/// does): over the holster, the M4A1's "Item is incompatible"
/// (`Slot.ItemFiltersWontAllowError`, `courses10`'s `drag-and-drop`).
/// None for a thing already there or nothing to be done (`ItemManipulator.
/// ItemAlreadyThereError`, `NoPossibleActionsError`, which close it).
/// `PROVISIONAL`: none either where the game's error has words the app's
/// refusal lacks (`DeepMoveError`'s limit, `MissingVitalPartsError`'s
/// slots, `TooLargeStackError`, `PlayerIsBusyError`), nor for the app's own.
///
/// Over a grid (`GridView.HighlightItemViewPosition`) none either for a
/// place taken, a thing that does not fit or no room, which the red cells
/// alone say (`Grid.PlaceTakenByAnotherItemError`, `Grid.WontFitToGridError`,
/// `NoFreeSpaceError`); a grid's own filter is said there
/// (`Grid.ItemFiltersWontAllowError`), and a magazine's refusal of rounds
/// (`Magazine.FullMagazineError`, `Magazine.ItemFiltersWontAllowError`).
pub(super) fn refusal_tip(refusal: &Refusal, carried: &Carried, over: Over) -> Option<String> {
    let said = |key: &str| carried.texts.say(key).unwrap_or_else(|| key.to_owned());
    if over == Over::Grid && matches!(refusal, Refusal::Taken | Refusal::OutsideGrid | Refusal::NoRoom) {
        return None;
    }
    let words = match refusal {
        Refusal::MagazineFull => said("Inventory Errors/Magazine is full"),
        Refusal::WrongAmmo => said("Inventory Errors/Ammo is incompatible"),
        Refusal::Filter => said("Inventory Errors/Item is incompatible"),
        Refusal::Locked => said("Inventory Errors/Slot is locked"),
        Refusal::Taken => said("Inventory Errors/Slot not empty"),
        Refusal::Cycle => said("Inventory Errors/Infinite item cycle"),
        Refusal::Vital(_) | Refusal::NotInRaid => said("Inventory Errors/Not moddable in raid"),
        Refusal::KeptOn(_) => said("Inventory Errors/Cannot move item during the raid"),
        Refusal::Malfunction => said("InventoryErrors/MalfunctionError"),
        // (`SlotBlockedError`: the names of the things in the slots that
        // block it; `ConflictingSlotBlockedError`: the slot's name.)
        Refusal::Blocked(slot) => {
            let name = carried.equipment.worn(slot).map_or_else(|| slot.clone(), |item| carried.texts.names.get(&item.kind.template).cloned().unwrap_or_else(|| item.kind.name.clone()));
            said("Inventory Errors/Slot is blocked").replace("{0}", &name)
        }
        Refusal::WouldBlock(slot) => said("Inventory Errors/Conflicting slot").replace("{0}", &said(slot)),
        // (Already the game's words.)
        Refusal::Conflicting { .. } | Refusal::PlateWorn | Refusal::OverLimit { .. } | Refusal::NotExamined { .. } | Refusal::TakeNotExamined { .. } => why(refusal, &carried.texts),
        _ => return None,
    };
    Some(format!("<color=red>{words}</color>"))
}

pub(super) fn spawn(mut commands: Commands, options: Res<Options>, features: Res<crate::screens::options_view::Features>, look: Res<crate::screens::health_view::HealthLook>) {
    let open = options.inventory;
    // With `TARKOV UI` the screen begins at the window's top, where the
    // game's has its bar of tabs (the debug figures are hidden while it
    // is open: `hud::hide_over_screen`); the game's own bar is drawn there
    // where it was read (`game_tabs`).
    let tarkov = features.tarkov_ui;
    let game_bar = tarkov && look.tab_bar.is_some();
    let root = Node {
        position_type: PositionType::Absolute,
        left: Val::Px(0.0),
        top: Val::Px(0.0),
        width: Val::Percent(100.0),
        height: Val::Percent(100.0),
        flex_direction: FlexDirection::Column,
        padding: UiRect { left: Val::Px(18.0), right: Val::Px(18.0), top: Val::Px(if tarkov { 0.0 } else { 64.0 }), bottom: Val::Px(18.0) },
        row_gap: Val::Px(12.0),
        ..default()
    };
    let shown = if open { Visibility::Inherited } else { Visibility::Hidden };
    // The app's own ground over the world; the game's screen has none, the
    // world blurred behind it (`screen_blur`).
    let ground = if tarkov { Color::NONE } else { BACK };
    commands.spawn((Screen, root, BackgroundColor(ground), GlobalZIndex(10), shown, crate::ui::screen_fade::OnScreen::Inventory)).with_children(|screen| {
        // The row of tabs; with `TARKOV UI` as high as the game's bar of
        // tabs (`Tab Bar`, 43 units) and over the page, which is as large
        // as the window.
        let row = match tarkov {
            true => Node { column_gap: Val::Px(6.0), align_items: AlignItems::Center, height: Val::Vh(TAB_BAR / SCREEN_HEIGHT * 100.0), flex_shrink: 0.0, ..default() },
            false => Node { column_gap: Val::Px(6.0), align_items: AlignItems::Center, ..default() },
        };
        screen.spawn((row, ZIndex(1))).with_children(|row| {
            // With the game's bar, the row only keeps its place.
            if game_bar {
                return;
            }
            for (index, (caption, _)) in TABS.into_iter().enumerate() {
                let color = if !tab_open(index, &features) { FAINT } else { WORDS };
                let back = if index == GEAR { LINE } else { PANEL };
                row.spawn((TabButton(index), Button, Node { padding: UiRect::axes(Val::Px(14.0), Val::Px(6.0)), ..default() }, BackgroundColor(back)))
                    .with_children(|tab| {
                        tab.spawn(words(caption, 14.0, color));
                    });
            }
        });
        if game_bar {
            let whole = Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() };
            screen.spawn((GameTabBar, whole, ZIndex(2)));
        }
        let page = |index: usize| {
            let node = Node { flex_grow: 1.0, column_gap: Val::Px(14.0), display: if index == GEAR { Display::Flex } else { Display::None }, ..default() };
            (Page(index), node)
        };
        // The Gear page is drawn afresh whenever something has moved
        // (`rebuild`); the HEALTH tab shows it too, with the health panel
        // where the equipment panel was. With `TARKOV UI` it is the whole
        // window, its panels where the game's screen has them.
        match tarkov {
            true => {
                let (index, node) = page(GEAR);
                let whole = Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), width: Val::Percent(100.0), height: Val::Percent(100.0), ..node };
                screen.spawn((index, whole, GearPage));
            }
            false => {
                screen.spawn((page(GEAR), GearPage));
            }
        }
        // The options' page is drawn by its own module, with no ground of
        // its own: the world blurred behind it, as behind the game's
        // screens.
        let options_page = Node { flex_grow: 1.0, flex_direction: FlexDirection::Column, min_height: Val::Px(0.0), display: Display::None, ..default() };
        screen.spawn((Page(OPTIONS), crate::screens::options_view::OptionsPage, options_page));
        // The OVERALL tab's page, as large as the window, its things where
        // the game's screen has them (`overall_view`).
        let overall = Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), width: Val::Percent(100.0), height: Val::Percent(100.0), display: Display::None, ..default() };
        screen.spawn((Page(OVERALL), crate::screens::overall_view::OverallPage, overall.clone()));
        // The SKILLS tab's page, as large as the window (`skills_view`).
        screen.spawn((Page(SKILLS), crate::screens::skills_view::SkillsPage, overall));
    });
    // The item the mouse holds, over everything: its picture where it has
    // one (`DraggedItemView`), else the app's box with its name; under it
    // the cells it would take in a grid (`GridView._highlightPanel`).
    let ghost = Node { position_type: PositionType::Absolute, border: UiRect::all(Val::Px(1.0)), padding: UiRect::all(Val::Px(2.0)), overflow: Overflow::clip(), ..default() };
    commands.spawn((Ghost, ghost, BorderColor::all(WORDS), BackgroundColor(THING.with_alpha(0.8)), GlobalZIndex(21), Visibility::Hidden)).with_children(|held| {
        let whole = Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() };
        held.spawn((GhostPicture, whole, ImageNode::default(), Visibility::Hidden));
        held.spawn((GhostWords, words("", 10.0, WORDS)));
    });
    commands.spawn((DropMark, Node { position_type: PositionType::Absolute, ..default() }, BackgroundColor(Color::NONE), GlobalZIndex(20), Visibility::Hidden));
    // What the game shows when something that can be worked is looked at
    // from within reach: its menu, a line for each thing it offers.
    let prompt = Node {
        position_type: PositionType::Absolute,
        left: Val::Percent(50.0),
        top: Val::Percent(58.0),
        flex_direction: FlexDirection::Column,
        row_gap: Val::Px(2.0),
        ..default()
    };
    commands.spawn((Prompt, prompt, Visibility::Hidden));
}
