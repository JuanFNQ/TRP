//! The inventory's own panels read: the HEALTH panel's parts, the containers, the crate, the quick bar and a thing's views.

use super::*;

/// The HEALTH panel's parts, the containers' column, the panel of a container opened, the quick-use panel,
/// a thing's views, a magazine's load component and the examination's view, with their pictures.
pub(super) fn read_inventory_panels(look: &mut HealthLook, install: &trp_data::Install, file: &str, layouts: &[String], panel: &trp_data::interface::screen::HealthPanel, read: &mut ReadPanel) {
    look.health.pictures = read(&panel.panel, &look.health.shown, &mut look.textures);
    if let Some(strip) = &panel.strip {
        look.strip.pictures = read(strip, &look.strip.shown, &mut look.textures);
    }
    if let Some(foot) = &panel.foot {
        look.foot.pictures = read(foot, &look.foot.shown, &mut look.textures);
    }
    if let Some(border) = &panel.border {
        look.border.pictures = read(border, &look.border.shown, &mut look.textures);
    }
    if let Some(gear) = &panel.gear_panel {
        look.gear.pictures = read(gear, &look.gear.shown, &mut look.textures);
    }
    // The containers' column: its panel with its row template,
    // its grid view, the slots' empty pictures, the layouts.
    match trp_data::interface::screen::ContainerViews::load(install, file) {
        Ok(views) => {
            let template = &views.panel.nodes[views.template].path;
            let by_slot: Vec<usize> = ["Full Border", "Empty Border", "Background", "BackImage"].iter().filter_map(|name| views.panel.find(&format!("{template}/Slot Panel/{name}"))).collect();
            // The row template, and its special slots' panel
            // and view, which a container with special slots
            // turns on (`SearchableSlotView.CreateSlots`).
            let mut turned = vec![views.template];
            turned.extend(["Slot Template/Special Slots Panel Template", "Slot Template/Special Slot Template"].iter().filter_map(|end| (0..views.panel.nodes.len()).find(|&index| views.panel.nodes[index].path.ends_with(end))));
            // A row's search parts (`draw_row_search`), and a
            // body's dog tag view's slot (`draw_dogtag_slot`), on
            // and off as they are drawn.
            let search_parts: Vec<usize> = ROW_SEARCH_PARTS.iter().filter_map(|part| views.panel.find(&format!("{template}/{part}"))).collect();
            let dogtag: Vec<usize> = ["Scrollview Parent/Containers Scrollview/Content/DogtagTemplate", "Scrollview Parent/Containers Scrollview/Content/DogtagTemplate/Panel", DOGTAG_SLOT].iter().filter_map(|path| views.panel.find(path)).collect();
            let dogtag_parts: Vec<usize> = dogtag.last().map_or_else(Vec::new, |&slot| ["Full Border", "Empty Border", "Background", "BackImage"].iter().filter_map(|name| views.panel.find(&format!("{}/{name}", views.panel.nodes[slot].path))).collect());
            // A special slot's full border, saved off, on where
            // the slot holds a thing (`draw_special_slots`).
            let special_full = (0..views.panel.nodes.len()).find(|&index| views.panel.nodes[index].path.ends_with("Special Slot Template/Full Border"));
            turned.extend(&search_parts);
            turned.extend(&dogtag);
            let by_slot: Vec<usize> = by_slot.into_iter().chain(search_parts).chain(dogtag_parts).chain(special_full).collect();
            look.containers_drawn.shown = shown_of(&views.panel.nodes, &turned, &[], &by_slot);
            look.containers_drawn.pictures = read(&views.panel, &look.containers_drawn.shown, &mut look.textures);
            if let Some(grid) = &views.grid {
                look.grid_drawn.shown = shown_of(&grid.nodes, &[], &[], &[]);
                look.grid_drawn.pictures = read(grid, &look.grid_drawn.shown, &mut look.textures);
            }
            look.containers = Some(views);
        }
        Err(e) => eprintln!("the game's containers' column could not be read; it is drawn the app's way: {e:#}"),
    }

    look.layouts = trp_data::interface::screen::rig_layouts(install, layouts);
    // The panel for a container opened, as a raid shows it: the
    // panel and its header turned on; the money, what is only in
    // the lobby, the sorting table's and the search's tabs and
    // the mark of what is had from outside a raid off
    // (`SimpleStashPanel.Show`); its scroll bars drawn with the
    // scrolling (`scroll_views`); its filters' versions as the
    // filter chosen (`FilterPanel.Show`, all).
    match trp_data::interface::screen::StashViews::load(install, file) {
        Ok(mut stash) => {
            let turned: Vec<usize> = [Some(stash.simple), stash.header].into_iter().flatten().collect();
            let bars: Vec<usize> = (0..stash.panel.nodes.len()).filter(|&index| stash.panel.nodes[index].scripts.iter().any(|(class, _)| class == "Scrollbar")).collect();
            let never: Vec<usize> = [stash.money, stash.sorting_table, stash.search, stash.external].into_iter().flatten().chain(stash.lobby_only.iter().copied()).chain(bars).collect();

            // On and off as the script leaves them, which is what
            // its rows lay out (the header's row is sized to it).
            for &index in &turned {
                stash.panel.nodes[index].active = true;
            }
            for &index in &never {
                stash.panel.nodes[index].active = false;
            }
            let mut scripted: Vec<usize> = stash.filter_tabs.iter().flat_map(|tab| [tab.normal, tab.selected]).flatten().collect();
            // The search's parts (`SearchableView`, `SearchButton`):
            // on and off by the search as it is drawn
            // (`draw_stash`), what lies under them shown with them.
            // The panel for a corpse's gear (`ComplexStashPanel`)
            // likewise, shown in place of the simple panel while a
            // body is searched (`draw_complex`).
            let searching: Vec<usize> = SEARCH_PARTS.iter().filter_map(|path| stash.panel.find(path)).chain(stash.complex.map(|complex| complex.panel)).collect();
            scripted.extend(&searching);
            let turned_on: Vec<usize> = turned.iter().chain(&searching).copied().collect();
            look.stash_drawn.shown = shown_of(&stash.panel.nodes, &turned_on, &never, &scripted);
            look.stash_drawn.pictures = read(&stash.panel, &look.stash_drawn.shown, &mut look.textures);
            look.name_font = stash.name.and_then(|name| stash.panel.nodes[name].text.as_ref()).and_then(|text| {
                let file = stash.panel.file_of(text.font.0)?;
                trp_data::interface::screen::TmpFont::read(install, &file, text.font.1).ok()
            });
            look.stash = Some(stash);
        }
        Err(e) => eprintln!("the game's panel for a container opened could not be read; the app's is drawn: {e:#}"),
    }
    // The quick-use panel, every view on as its script shows
    // them (`InventoryScreenQuickAccessPanel.Show`).
    match trp_data::interface::screen::QuickViews::load(install, file) {
        Ok(mut quick) => {
            for slot in quick.slots.clone() {
                quick.panel.nodes[slot.object].active = true;
            }
            look.quick_drawn.shown = shown_of(&quick.panel.nodes, &[], &[], &[]);
            look.quick_drawn.pictures = read(&quick.panel, &look.quick_drawn.shown, &mut look.textures);
            look.quick = Some(quick);
        }
        Err(e) => eprintln!("the game's quick-use panel could not be read; the app's is drawn: {e:#}"),
    }
    // The views of a thing, in a slot and in a grid.
    match trp_data::interface::screen::ItemViews::load(install) {
        Ok(items) => {
            for (view, drawn) in [(&items.slot, &mut look.slot_item), (&items.grid, &mut look.grid_item)] {
                if let Some(view) = view {
                    // What the views' scripts turn on for a
                    // weapon (`ValidateMastering`) and a part
                    // (`ItemViewStats`): their pictures read.
                    let named = |name: &str| (0..view.nodes.len()).filter(|&index| view.nodes[index].name == name).collect::<Vec<_>>();
                    let mut turned: Vec<usize> =
                        ["MasteringContainer", "MasteringIcon", "Level", "ModIconContainer", "WishlistGlow", "WishlistBorder", "WishlistBorders", "WishlistIcon", "Loader", "Small Loader", "NotExaminedBackground", "UnsearchedBackground"].iter().flat_map(|name| named(name)).collect();
                    turned.extend(named("Image").into_iter().filter(|&index| view.nodes[index].parent.is_some_and(|parent| view.nodes[parent].name == "MasteringIcon")));
                    drawn.shown = shown_of(&view.nodes, &turned, &[], &[]);
                    drawn.pictures = read(view, &drawn.shown, &mut look.textures);
                }
            }
            look.items = Some(items);
        }
        Err(e) => eprintln!("the game's views of a thing could not be read; things are drawn the app's way: {e:#}"),
    }
    // The load component over a magazine's view, all of it.
    match trp_data::interface::screen::load_ammo_component(install) {
        Ok(panel) => {
            let all: Vec<usize> = (0..panel.nodes.len()).collect();
            look.spin_drawn.shown = shown_of(&panel.nodes, &all, &[], &[]);
            look.spin_drawn.pictures = read(&panel, &look.spin_drawn.shown, &mut look.textures);
            look.spin = Some(panel);
        }
        Err(e) => eprintln!("the game's load component of a magazine could not be read; the app's is drawn: {e:#}"),
    }
    // The examination's view, all of it.
    match trp_data::interface::screen::examine_component(install) {
        Ok(panel) => {
            let all: Vec<usize> = (0..panel.nodes.len()).collect();
            look.examine_drawn.shown = shown_of(&panel.nodes, &all, &[], &[]);
            look.examine_drawn.pictures = read(&panel, &look.examine_drawn.shown, &mut look.textures);
            look.examine = Some(panel);
        }
        Err(e) => eprintln!("the game's examination view could not be read; the app's is drawn: {e:#}"),
    }
}
