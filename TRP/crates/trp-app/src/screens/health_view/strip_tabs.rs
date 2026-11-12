//! The strip, the gear, the foot, the border and the tab bar.

use super::*;

/// Draws the GEAR tab's strip of figures as the game's (`TARKOV UI`),
/// where the game lays it out in the left panel, as `draw_panel` draws.
pub fn draw_strip(column: &mut ChildSpawnerCommands, look: &HealthLook, said: &Words, left: [f32; 2], origin: [f32; 2], s: f32) {
    let Some((game, strip)) = look.panel.as_ref().and_then(|game| Some((game, game.strip.as_ref()?))) else { return };
    let arrows: Vec<usize> = game.strip_figures.iter().filter_map(|(_, f)| f.as_ref()).flat_map(|f| [f.up, f.down]).flatten().collect();
    draw_nodes(column, look, strip, &look.strip, STRIP, Marks { arrows: &arrows, ..Marks::default() }, said, Frame::In(left), origin, s);
}

/// Draws the game's equipment panel (`TARKOV UI`) as a panel of that size
/// with its top left at `origin`: its outline of the character and the
/// slot view of each slot the page has (`present`), full or empty as the
/// slot is (`full`), its header in the install's words for the slot
/// (`SlotViewHeader.Show`: the slot's name in capitals). The page's own
/// boxes for the slots, with what is in them, lie over these.
#[allow(clippy::too_many_arguments)]
pub fn draw_gear(column: &mut ChildSpawnerCommands, look: &HealthLook, said: &Words, size: [f32; 2], origin: [f32; 2], s: f32, present: &dyn Fn(&str) -> bool, full: &dyn Fn(&str) -> bool) {
    let Some((game, gear)) = look.panel.as_ref().and_then(|game| Some((game, game.gear_panel.as_ref()?))) else { return };
    let (mut set, mut skip, mut words) = (HashMap::new(), Vec::new(), HashMap::new());
    for (slot, parts) in &game.slot_views {
        if !present(slot) {
            skip.push(parts.view);
            continue;
        }
        let full = full(slot);
        for (part, on) in [(parts.full_border, full), (parts.back_image, !full), (parts.background, !full), (parts.empty_border, !full)] {
            if let Some(part) = part {
                set.insert(part, on);
            }
        }
        let text = parts.header.and_then(|header| (header..gear.nodes.len()).find(|&below| gear.nodes[below].text.is_some() && gear.nodes[below].path.starts_with(&gear.nodes[header].path)));
        if let Some(text) = text {
            words.insert(text, said.say(slot, slot).trim().to_uppercase());
        }
    }
    let marks = Marks { set: Some(&set), skip: &skip, words: Some(&words), ..Marks::default() };
    draw_nodes(column, look, gear, &look.gear, GEAR, marks, said, Frame::Is(size), origin, s);
}

/// Draws the left panel's foot (`Bottom Bar`), which lies under both tabs'
/// panels, as the game's (`TARKOV UI`).
pub fn draw_foot(column: &mut ChildSpawnerCommands, look: &HealthLook, said: &Words, left: [f32; 2], origin: [f32; 2], s: f32) {
    let Some(foot) = look.panel.as_ref().and_then(|game| game.foot.as_ref()) else { return };
    draw_nodes(column, look, foot, &look.foot, FOOT, Marks::default(), said, Frame::In(left), origin, s);
}

/// The left panel's own ground (`TARKOV UI`): its picture's colour.
pub fn left_ground(look: &HealthLook) -> Option<Color> {
    look.panel.as_ref()?.left_ground.map(rgba)
}

/// Draws the left side's frame round its two panels (`Border`), over
/// them, as the game's (`TARKOV UI`): `side` is the left side's size and
/// `origin` its corner, in units, in what it is drawn into.
pub fn draw_border(column: &mut ChildSpawnerCommands, look: &HealthLook, said: &Words, side: [f32; 2], origin: [f32; 2], s: f32) {
    let Some(border) = look.panel.as_ref().and_then(|game| game.border.as_ref()) else { return };
    draw_nodes(column, look, border, &look.border, BORDER, Marks::default(), said, Frame::In(side), origin, s);
}

/// Draws the game's bar of tabs (`TARKOV UI`) into a node as large as the
/// window (`screen`, units): its ground, then each tab over the one before
/// it, as its chosen version or its idle one (`Tab.UpdateVersion`) and,
/// idle, its graphic under the mouse or its own (`Tab.Hover`), with its
/// words and its fill cut to its mask (`fills`, by the mask's object); and
/// the app's own tab (its words `own`) drawn as the first tab is, with the
/// game's settings cog for its own, where the row would lay a ninth.
/// `chosen` and `hovered` are tabs' places in the row, the app's own after
/// the game's. Only the game's first `shown` tabs are drawn, the app's
/// after the last of them (the HUD page's HIDE UNUSED TABS removes Map,
/// Tasks, Prestige and Achievements, the game's last four).
#[allow(clippy::too_many_arguments)]
pub fn draw_tab_bar(column: &mut ChildSpawnerCommands, look: &HealthLook, said: &Words, screen: [f32; 2], chosen: Option<usize>, hovered: Option<usize>, own: &str, (dimmed, shown): (&[bool], usize), fills: &HashMap<usize, Handle<Image>>, s: f32) {
    let Some(bar) = &look.tab_bar else { return };
    let tabs: Vec<usize> = bar.tabs.iter().map(|tab| tab.object).collect();
    let masks: Vec<usize> = look.tab_fills.keys().copied().collect();
    draw_nodes(column, look, &bar.panel, &look.tabs_drawn, TABS, Marks { skip: &tabs, ..Marks::default() }, said, Frame::In(screen), [0.0, 0.0], s);
    // `icon`: the tab's own icons, or others put in its icons' objects (the
    // app's own tab: `Image.sprite`, at rest and chosen), an object hidden
    // where there is none for it.
    let draw = |column: &mut ChildSpawnerCommands, place: usize, tab: &trp_data::interface::screen::TabParts, words: &str, icon: Option<[Option<Picture>; 2]>, origin: [f32; 2]| {
        let (is_chosen, is_hovered) = (chosen == Some(place), hovered == Some(place));
        let mut set = HashMap::new();
        for (part, on) in [(tab.normal, !is_chosen), (tab.selected, is_chosen), (tab.hover, is_hovered), (tab.idle, !is_hovered)] {
            if let Some(part) = part {
                set.insert(part, on);
            }
        }
        let mut sprites = HashMap::new();
        if let Some(own) = icon {
            let path = &bar.panel.nodes[tab.object].path;
            let chosen_path = tab.selected.map(|selected| format!("{}/", bar.panel.nodes[selected].path));
            for index in (0..bar.panel.nodes.len()).filter(|&index| bar.panel.nodes[index].name == "Icon" && bar.panel.nodes[index].path.starts_with(path.as_str())) {
                let under_chosen = chosen_path.as_ref().is_some_and(|chosen| bar.panel.nodes[index].path.starts_with(chosen.as_str()));
                match own[usize::from(under_chosen)] {
                    Some(picture) => {
                        sprites.insert(index, picture);
                    }
                    None => {
                        set.insert(index, false);
                    }
                }
            }
        }
        let words: HashMap<usize, String> = tab.labels.iter().map(|&label| (label, words.to_owned())).collect();
        // The prestige tab does not work in a raid (`InventoryScreen.Show`:
        // `SetInteractable(... && !InRaid)`): the game draws its group at
        // 0.3 (`SetUnlockStatus`), the whole tab see-through. The tabs the
        // app has not built are dimmed until they are, like the prestige
        // one, and, the app's look for all five, by their words and icon at
        // 0.3 alone, the tab solid, so that the tabs do not show through one
        // another.
        let mut groups: HashMap<usize, f32> = HashMap::new();
        if dimmed.get(place).copied().unwrap_or(false) {
            let path = &bar.panel.nodes[tab.object].path;
            let icons = (0..bar.panel.nodes.len()).filter(|&index| bar.panel.nodes[index].name == "Icon" && bar.panel.nodes[index].path.starts_with(path.as_str()));
            for part in tab.labels.iter().copied().chain(icons) {
                groups.insert(part, UNLOCKED_NOT);
            }
        }
        let marks = Marks { set: Some(&set), words: Some(&words), within: &[tab.object], skip_below: &masks, fills: Some(fills), groups: Some(&groups), sprites: Some(&sprites), ..Marks::default() };
        draw_nodes(column, look, &bar.panel, &look.tabs_drawn, TABS + ((place + 1) << 16), marks, said, Frame::In(screen), origin, s);
    };
    // The app's own tab, the options, as a ninth after the game's last, as
    // the others lie, with the game's single settings cog beside its words,
    // as the other tabs have their icons.
    let options = |column: &mut ChildSpawnerCommands| {
        if let Some(first) = bar.tabs.first() {
            let [x, ..] = bar.panel.placed_in(first.object, screen).rect;
            let origin = [options_tab_left(look, screen, shown) - x, 0.0];
            draw(column, bar.tabs.len(), first, own, Some(look.options_icons), origin);
        }
    };
    // In the order the game's tab group leaves them (`TabGroup.SelectTab`:
    // every tab put last in turn from the last to the first, then the one
    // chosen): each tab over the one after it, its slanted end on the
    // next, and the one chosen over all. The app's, as a ninth, under all.
    if chosen != Some(bar.tabs.len()) {
        options(column);
    }
    for (place, tab) in bar.tabs.iter().enumerate().take(shown).rev().filter(|(place, _)| chosen != Some(*place)) {
        draw(column, place, tab, look.tab_words.get(place).map_or("", String::as_str), None, [0.0, 0.0]);
    }
    match chosen {
        Some(place) if place == bar.tabs.len() => options(column),
        Some(place) => {
            if let Some(tab) = bar.tabs.get(place).filter(|_| place < shown) {
                draw(column, place, tab, look.tab_words.get(place).map_or("", String::as_str), None, [0.0, 0.0]);
            }
        }
        None => {}
    }
    if let Some(back) = &bar.back {
        draw_nodes(column, look, back, &look.back_drawn, BACK_BUTTON, Marks::default(), said, Frame::In(screen), [0.0, 0.0], s);
    }
}

/// The alpha of a tab that does not work (`CanvasGroup.SetUnlockStatus`).
const UNLOCKED_NOT: f32 = 0.3;

/// The left of the app's options tab, in units: where the row would lay a
/// tab after the game's first `shown` (`TabBar::lay_out`: each `width +
/// gap` on from the one before; a ninth with all eight), its left under
/// the last tab's slanted end as each tab's is under the one before.
pub(super) fn options_tab_left(look: &HealthLook, screen: [f32; 2], shown: usize) -> f32 {
    let Some(bar) = &look.tab_bar else { return 0.0 };
    let Some(first) = bar.tabs.first() else { return 0.0 };
    let [x, ..] = bar.panel.placed_in(first.object, screen).rect;
    x + (look.tab_width + bar.gap) * shown.min(bar.tabs.len()) as f32
}

/// Where the screen's back button takes the mouse (`TARKOV UI`), in units
/// from the screen's top left corner.
pub fn back_place(look: &HealthLook, screen: [f32; 2]) -> Option<[f32; 4]> {
    let back = look.tab_bar.as_ref()?.back.as_ref()?;
    Some(back.placed_in(0, screen).rect)
}

/// The game's filters of a container's panel, in their tabs' order
/// (`FilterPanel._filterRules`): the game's item classes each stands for,
/// by the install's database's nodes of the same names; all for the
/// first, none for the last (what was found in the raid, which the app
/// does not keep).
pub const FILTERS: [&[&str]; 18] = [
    &[],
    &["Weapon"],
    &["Magazine"],
    &["Ammo", "AmmoBox"],
    &["Meds"],
    &["FoodAndDrink", "Food", "Drink"],
    &["Knife"],
    &["Mod"],
    &["ThrowWeap"],
    &["BarterItem"],
    &["Vest"],
    &["Visors"],
    &["SearchableItem", "SimpleContainer"],
    &["Armor"],
    &["Info"],
    &["Key"],
    &["SpecItem"],
    &[],
];
