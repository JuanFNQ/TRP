//! Icons received, asked for and reported.

use super::*;

/// A picture come back from away from the frame: its item's template, the
/// cells it was asked for, its size in pixels, whether it was drawn (else
/// read from the cache), and as it lies and turned a quarter.
pub(super) struct Arrived {
    pub(super) template: String,
    pub(super) cells: [u32; 2],
    pub(super) size: [u32; 2],
    pub(super) drawn: bool,
    pub(super) made: anyhow::Result<(Vec<u8>, Vec<u8>)>,
}

/// A picture as the screen draws it, from RGBA with its colours not
/// multiplied, as it lies and turned a quarter.
pub(super) fn picture_of(images: &mut Assets<Image>, [width, height]: [u32; 2], upright: Vec<u8>, turned: Vec<u8>) -> Picture {
    let make = |w: u32, h: u32, data: Vec<u8>| Image::new(Extent3d { width: w, height: h, depth_or_array_layers: 1 }, TextureDimension::D2, data, TextureFormat::Rgba8UnormSrgb, bevy::asset::RenderAssetUsages::default());
    Picture::rendered(images.add(make(width, height, upright)), images.add(make(height, width, turned)), width as f32 / height as f32)
}

/// A picture the screen waits for has come, or cannot: the screen puts it
/// in its spinner's place (`inventory_view::PicturesArrived`), and a
/// change to what is carried is not made of it, which would draw the whole
/// screen afresh.
pub(super) fn arrived(carried: &mut Carried, ready: &mut crate::screens::inventory_view::PicturesArrived, template: String, picture: Option<Picture>) {
    match picture {
        Some(picture) => {
            carried.pictures.insert(template.clone(), picture);
        }
        None => {
            carried.picture_failed.insert(template.clone());
        }
    }
    ready.0.push(template);
}

/// The pictures read or finished away from the frame, put on the screen
/// as they come; one the cache had none of, or none of its size, is asked
/// of the icon thread to be drawn.
pub(super) fn receive_icons(mut icons: ResMut<Icons>, mut carried: ResMut<Carried>, mut ready: ResMut<crate::screens::inventory_view::PicturesArrived>, mut images: ResMut<Assets<Image>>) {
    let _clock = crate::platform::render_timing::Clock::new("receive_icons");
    if icons.arriving.is_empty() {
        return;
    }
    let mut done = Vec::new();
    icons.arriving.retain_mut(|task| match block_on(poll_once(task)) {
        Some(come) => {
            done.push(come);
            false
        }
        None => true,
    });
    for come in done {
        match (come.made, come.drawn) {
            (Ok((upright, turned)), drawn) => {
                let picture = picture_of(&mut images, come.size, upright, turned);
                arrived(carried.bypass_change_detection(), &mut ready, come.template, Some(picture));
                if drawn {
                    icons.drawn += 1;
                } else {
                    icons.cached += 1;
                }
            }
            (Err(_), false) => icons.queue.push_back((come.template, come.cells)),
            (Err(e), true) => {
                warn!("the picture of {} is not made: {e:#}", come.template);
                icons.failed += 1;
                arrived(carried.bypass_change_detection(), &mut ready, come.template, None);
            }
        }
    }
}

/// The equipment's slots whose things the gear page shows in rows of their
/// own, their grids and their special slots beside them (`inventory_view::
/// CONTAINERS`).
const CONTAINER_SLOTS: [&str; 4] = ["TacticalVest", "Pockets", "Backpack", "SecuredContainer"];

/// What a container shows in its special slots: what is in each of its
/// slots but an armour's (`inventory_view::special_slots`).
fn in_special_slots(item: &trp_sim::items::inventory::Item) -> impl Iterator<Item = &trp_sim::items::inventory::Item> {
    item.slots.iter().zip(&item.kind.slots).filter(|(_, slot)| !slot.armour).filter_map(|(part, _)| part.as_ref())
}

/// Every item the screen shows, by its template and its size, asked for
/// when the screen first shows it, as the game asks for an item view's
/// picture when the view is made (`ItemView.RefreshIcon`,
/// `ItemViewFactory.LoadItemIcon`, `ItemIconCreator.GetItemIcon`; icons
/// spec, "When a picture is asked for"): what is worn and carried, and
/// the container open beside it (not the container itself: it has no
/// model of an item). What lies in a container not yet opened, and on the
/// ground, is not asked for. Its picture is read from the cache away from
/// the frame, as the game reads it (`LoadFromUserCacheAsync`); one the
/// cache has not is asked of the icon thread to be drawn. Meanwhile its
/// box shows a spinner, where the game's shows its loader.
pub(super) fn ask_icons(carried: Res<Carried>, in_sight: Res<crate::screens::inventory_view::LootInSight>, mut icons: ResMut<Icons>) {
    let _clock = crate::platform::render_timing::Clock::new("ask_icons");
    if !icons.on || !(carried.is_changed() || in_sight.is_changed()) {
        return;
    }
    // What the screen shows in boxes of their own, as the game's views
    // are made: what is in the equipment's slots, and what lies in their
    // grids (the panel of the four containers); what lies in the grids of
    // a container opened in a window; and of the open container what its
    // panel has in sight (`LootInSight`), as the game's panel makes views
    // only for what is in sight of its scroll area (`GridViewMagnifier`).
    // Not a weapon's parts or a magazine's rounds, nor what is in a
    // container lying in another: those have no box of their own until it
    // is opened.
    let mut seen: Vec<&trp_sim::items::inventory::Item> = Vec::new();
    for item in carried.equipment.item.contents() {
        seen.push(item);
        seen.extend(item.grids.iter().flatten().map(|(_, child)| child));
    }
    // What is in the four containers' special slots, shown beside their
    // grids (the pockets' three: a view each, `SearchableSlotView.
    // CreateSlots`). Not asked for, what lay there would keep its spinner.
    for slot in CONTAINER_SLOTS {
        seen.extend(carried.equipment.item.in_slot(slot).into_iter().flat_map(in_special_slots));
    }
    for window in carried.windows.iter().filter(|window| window.kind == crate::screens::inventory_view::WinKind::Grids) {
        if let Some(item) = carried.find(window.item) {
            seen.extend(item.grids.iter().flatten().map(|(_, child)| child));
            seen.extend(in_special_slots(item));
        }
    }
    // What a window that shows a thing has in its parts' panel: every
    // part in its slots, at any depth (`CreateModSlots`, a view each).
    for window in carried.windows.iter().filter(|window| window.kind == crate::screens::inventory_view::WinKind::Info) {
        let mut holders: Vec<&trp_sim::items::inventory::Item> = carried.find(window.item).into_iter().collect();
        while let Some(holder) = holders.pop() {
            for part in holder.slots.iter().flatten() {
                seen.push(part);
                holders.push(part);
            }
        }
    }
    // A body's gear shown as the equipment is: what is in its slots and
    // in their grids (its panel makes a view of each, `ComplexStashPanel`).
    if carried.loot_open && carried.shows_body() {
        for item in carried.loot.contents() {
            seen.push(item);
            seen.extend(item.grids.iter().flatten().map(|(_, child)| child).filter(|child| carried.searcher.known(child.id)));
        }
        for slot in CONTAINER_SLOTS {
            seen.extend(carried.loot.in_slot(slot).into_iter().flat_map(in_special_slots));
        }
    }
    if carried.loot_open {
        for (grid, rows) in carried.loot.grids.iter().zip(&in_sight.0) {
            for (place, item) in grid {
                let height = if place.turned { item.cells()[0] } else { item.cells()[1] };
                if place.y < rows[1] && place.y + height > rows[0] {
                    seen.push(item);
                }
            }
        }
    }
    let seen: Vec<(String, [u32; 2])> = seen.into_iter().map(|item| (crate::screens::inventory_view::shown_key(&carried, item), item.cells())).collect();
    for (key, cells) in seen {
        icons.want(key, cells);
    }
}

impl Icons {
    /// A thing's picture asked for, by its key (`picture_key`) and its
    /// cells, if it has not been: read from the cache away from the frame,
    /// or drawn (the screens that show things: the inventory's,
    /// `ask_icons`; the edit build screen's, `edit_build`).
    pub(crate) fn want(&mut self, key: String, cells: [u32; 2]) {
        let icons = self;
        let wanted = (key, cells);
        if !icons.on || wanted.1[0] == 0 || wanted.1[1] == 0 || !icons.known.insert(wanted.clone()) {
            return;
        }
        let (template, cells) = wanted;
        let size = [cells[0] * CELL_PIXELS + 1, cells[1] * CELL_PIXELS + 1];
        let Some(path) = icons.path(&template, size) else {
            icons.queue.push_back((template, cells));
            return;
        };
        icons.arriving.push(AsyncComputeTaskPool::get().spawn(async move {
            let made = trp_data::interface::icon::load_png(&path).and_then(|(width, height, rgba)| {
                anyhow::ensure!([width, height] == size, "the cache's picture is {width} by {height}");
                let turned = trp_data::interface::icon::turned(&rgba, width as usize, height as usize);
                Ok((rgba, turned))
            });
            Arrived { template, cells, size, drawn: false, made }
        }));
    }
}

/// A line in the log each time what was asked is all made: how many
/// pictures were read, drawn, and could not be made.
pub(super) fn report_icons(mut icons: ResMut<Icons>) {
    let _clock = crate::platform::render_timing::Clock::new("report_icons");
    let counts = (icons.cached, icons.drawn, icons.failed);
    if icons.busy() || counts == icons.reported {
        return;
    }
    icons.reported = counts;
    info!("the inventory's pictures: {} read from the cache, {} drawn, {} not made", counts.0, counts.1, counts.2);
}
