//! Item views, their icons, values and menu texts.

use super::*;

/// The game's own words for the menu's lines, from the run's texts.
pub(super) fn menu_texts(game: Res<GameData>, mut texts: ResMut<MenuTexts>) {
    let wanted = [Action::Open, Action::Close, Action::Search, Action::Unlock, Action::Breach, Action::Take, Action::Examine];
    let mut names: Vec<&str> = wanted.iter().map(|action| action.name()).collect();
    // The panel's error. (The benches' names and lines are the app's own
    // words, none of the game's.)
    names.push(NO_SPACE);
    texts.0 = trp_data::character::inventory::words_of(&game.words, &names);
}

/// The sort of thing an item is, for its picture: the nearest of its
/// classes that has one.
pub(super) fn icon_of(item: &Item) -> &'static str {
    const SORTS: [(&str, &str); 16] = [
        ("Weapon", "weapon"),
        ("Magazine", "magazine"),
        ("Ammo", "ammo"),
        ("AmmoBox", "ammo"),
        ("Headwear", "headwear"),
        ("Vest", "vest"),
        ("Armor", "vest"),
        ("Backpack", "backpack"),
        ("Meds", "meds"),
        ("FoodDrink", "food"),
        ("Food", "food"),
        ("Drink", "food"),
        ("Headphones", "headset"),
        ("Knife", "knife"),
        ("Mod", "part"),
        ("FaceCover", "headwear"),
    ];
    let named = |class: &String| SORTS.iter().find(|(name, _)| name == class).map(|(_, sort)| *sort);
    item.kind.class_names.iter().find_map(named).unwrap_or("thing")
}

/// Makes the pictures: a plain shape for each sort of thing, drawn here
/// out of a few boxes and rounds. They stand in for the game's own
/// pictures of its items, which are not read; nothing of them is the
/// game's.
pub(super) fn make_icons(mut images: ResMut<Assets<Image>>, mut carried: ResMut<Carried>) {
    // Each shape: whether it is round, then left, top, right and bottom
    // as shares of the picture.
    type Shape = (bool, [f32; 4]);
    let sorts: [(&'static str, &[Shape]); 11] = [
        ("weapon", &[(false, [0.04, 0.44, 0.62, 0.50]), (false, [0.30, 0.40, 0.72, 0.58]), (false, [0.70, 0.44, 0.97, 0.60]), (false, [0.44, 0.58, 0.54, 0.88]), (false, [0.62, 0.58, 0.68, 0.78])]),
        ("magazine", &[(false, [0.36, 0.14, 0.64, 0.90]), (false, [0.30, 0.10, 0.70, 0.22])]),
        ("ammo", &[(true, [0.16, 0.14, 0.32, 0.56]), (false, [0.16, 0.36, 0.32, 0.88]), (true, [0.42, 0.14, 0.58, 0.56]), (false, [0.42, 0.36, 0.58, 0.88]), (true, [0.68, 0.14, 0.84, 0.56]), (false, [0.68, 0.36, 0.84, 0.88])]),
        ("headwear", &[(true, [0.14, 0.16, 0.86, 1.30]), (false, [0.08, 0.72, 0.92, 0.82])]),
        ("vest", &[(false, [0.26, 0.28, 0.74, 0.92]), (false, [0.26, 0.08, 0.38, 0.30]), (false, [0.62, 0.08, 0.74, 0.30])]),
        ("backpack", &[(true, [0.22, 0.08, 0.78, 0.52]), (false, [0.22, 0.30, 0.78, 0.92]), (false, [0.12, 0.34, 0.18, 0.82]), (false, [0.82, 0.34, 0.88, 0.82])]),
        ("meds", &[(false, [0.40, 0.14, 0.60, 0.86]), (false, [0.14, 0.40, 0.86, 0.60])]),
        ("food", &[(false, [0.32, 0.22, 0.68, 0.88]), (true, [0.32, 0.12, 0.68, 0.32])]),
        ("headset", &[(false, [0.16, 0.14, 0.84, 0.22]), (false, [0.16, 0.14, 0.24, 0.50]), (false, [0.76, 0.14, 0.84, 0.50]), (false, [0.10, 0.46, 0.30, 0.86]), (false, [0.70, 0.46, 0.90, 0.86])]),
        ("knife", &[(false, [0.08, 0.45, 0.62, 0.54]), (false, [0.62, 0.40, 0.92, 0.60]), (false, [0.58, 0.34, 0.63, 0.66])]),
        ("part", &[(false, [0.18, 0.40, 0.82, 0.60]), (true, [0.38, 0.26, 0.62, 0.74])]),
    ];
    // The longer side of a picture, in pixels.
    const SIDE: f32 = 128.0;
    let mut paint = |shapes: &[Shape]| {
        // The picture is as large as its shapes reach and no larger, so
        // that it has the thing's own proportions: a rifle is long, a
        // magazine tall.
        let reach = |pick: fn(&[f32; 4]) -> f32, most: bool| {
            let all = shapes.iter().map(|(_, sides)| pick(sides));
            if most { all.fold(0.0, f32::max) } else { all.fold(1.0, f32::min) }
        };
        let (left, top) = (reach(|s| s[0], false), reach(|s| s[1], false).max(0.0));
        let (right, bottom) = (reach(|s| s[2], true), reach(|s| s[3], true).min(1.0));
        let (across, down) = ((right - left).max(0.01), (bottom - top).max(0.01));
        let scale = SIDE / across.max(down);
        let (width, height) = (((across * scale).round() as usize).max(1), ((down * scale).round() as usize).max(1));
        let inside = |x: usize, y: usize| {
            let (u, v) = (left + (x as f32 + 0.5) / width as f32 * across, top + (y as f32 + 0.5) / height as f32 * down);
            shapes.iter().any(|(round, [left, top, right, bottom])| match round {
                false => u >= *left && u <= *right && v >= *top && v <= *bottom,
                true => (((u - (left + right) / 2.0) / ((right - left) / 2.0)).powi(2) + ((v - (top + bottom) / 2.0) / ((bottom - top) / 2.0)).powi(2)) <= 1.0,
            })
        };
        // As it lies, and turned a quarter to the right for a thing that
        // lies turned in its grid.
        let mut image = |width: usize, height: usize, at: &dyn Fn(usize, usize) -> bool| {
            let mut data = vec![0u8; width * height * 4];
            for (index, pixel) in data.chunks_exact_mut(4).enumerate() {
                if at(index % width, index / width) {
                    pixel.copy_from_slice(&[196, 200, 184, 120]);
                }
            }
            let size = Extent3d { width: width as u32, height: height as u32, depth_or_array_layers: 1 };
            images.add(Image::new(size, TextureDimension::D2, data, TextureFormat::Rgba8UnormSrgb, RenderAssetUsages::default()))
        };
        Picture {
            upright: image(width, height, &inside),
            turned: image(height, width, &|x, y| inside(y, height - 1 - x)),
            shape: width as f32 / height as f32,
            fills: false,
        }
    };
    for (sort, shapes) in sorts {
        let picture = paint(shapes);
        carried.icons.insert(sort, picture);
    }
    let plain = paint(&[(false, [0.22, 0.22, 0.78, 0.78])]);
    carried.icons.insert("thing", plain);
    carried.spinner = images.add(spinner_image());
}

/// The spinner: a ring with a quarter of it open, fading from its head to
/// its tail, in the screen's light colour on nothing. The app's (the
/// game's loader's own picture is not read).
fn spinner_image() -> Image {
    const SIDE: usize = SPINNER_PIXELS;
    let middle = SIDE as f32 / 2.0;
    let (outer, inner) = (middle - 1.5, middle - 8.5);
    let mut data = vec![0u8; SIDE * SIDE * 4];
    for (index, pixel) in data.chunks_exact_mut(4).enumerate() {
        let (dx, dy) = ((index % SIDE) as f32 + 0.5 - middle, (index / SIDE) as f32 + 0.5 - middle);
        let reach = (dx * dx + dy * dy).sqrt();
        // A pixel's worth of soft edge on either side of the ring.
        let edge = ((outer - reach).min(reach - inner) + 0.5).clamp(0.0, 1.0);
        // Round from the top the way it turns, 0 to 1: the head at the
        // end, the tail a quarter after the start, open before that.
        let round = (dx.atan2(-dy) / std::f32::consts::TAU).rem_euclid(1.0);
        let along = ((round - 0.25) / 0.75).clamp(0.0, 1.0);
        let alpha = edge * along * along;
        pixel.copy_from_slice(&[226, 228, 214, (alpha * 235.0) as u8]);
    }
    let size = Extent3d { width: SIDE as u32, height: SIDE as u32, depth_or_array_layers: 1 };
    Image::new(size, TextureDimension::D2, data, TextureFormat::Rgba8UnormSrgb, RenderAssetUsages::default())
}

/// How much of an item's box its picture may take, each way.
pub(super) const PICTURE_SHARE: f32 = 0.86;

/// An item as drawn: a box with its picture, its name, the stack and,
/// where there is room for it, its weight. `size` is the box's, in pixels,
/// and `turned` whether the thing lies turned a quarter. The picture keeps
/// its own proportions: it is drawn in the middle of the box at the
/// largest size that fits, so a larger box has a larger picture and never
/// a stretched one.
#[allow(clippy::too_many_arguments)]
pub(crate) fn item_view(parent: &mut ChildSpawnerCommands, carried: &Carried, item: &Item, node: Node, wide: bool, size: Vec2, turned: bool, layer: u8, s: f32) {
    // A thing not yet found in a search (inventory spec, "Searching a
    // container"; `GridItemView.UpdateInfo`): every part of its view off
    // but its unsearched ground, and nothing to take a click, a drag, a
    // drop or the mouse's hover (`ItemView.IsSearched`); the view's group
    // at `UNTAKEN_ALPHA`, as it cannot be taken out (`ItemView.
    // Transparency`: the remove's `UnknownItemError`). Under the mouse it
    // keeps the place it takes, so that a thing found there shows no
    // tooltip (`item_tooltip`).
    if !carried.searcher.known(item.id) {
        let look = carried.item_looks[if wide { 0 } else { 1 }].as_ref().and_then(|look| look.unsearched.as_ref().map(|ground| (look, ground)));
        let mut view = parent.spawn((UnsearchedView(item.id), node, RelativeCursorPosition::default()));
        match look {
            Some((look, (index, ground))) => {
                let units = [size.x / s, size.y / s];
                let [x, y, w, h] = look.panel.placed(*index, units).rect;
                let color = ground.color.with_alpha(ground.color.alpha() * UNTAKEN_ALPHA);
                view.with_children(|thing| crate::screens::health_view::draw_sprite_tiled(thing, ground, [x * s, y * s, w * s, h * s], s, color));
            }
            // The app's own look: the same grey, flat.
            None => {
                view.insert(BackgroundColor(Color::srgba(0.502, 0.502, 0.502, 0.69 * UNTAKEN_ALPHA)));
            }
        }
        return;
    }
    // With `TARKOV UI` on, the game's own view of a thing in a slot or in
    // a grid.
    if let Some(look) = carried.item_looks[if wide { 0 } else { 1 }].clone() {
        game_item_view(parent, carried, item, node, size, turned, layer, s, &look);
        return;
    }
    let stack = if item.count > 1 { format!(" x{}", item.count) } else { String::new() };
    let weight = if wide { format!("   {:.2} kg", item.weight()) } else { String::new() };
    let (unknown, examining) = (!trp_sim::items::examine::known(item), carried.examiner.examining(item.id));
    // The key it is bound to, as the game marks a bound thing.
    let key = carried.bound.key_of(item.id).map(|key| format!("   [{}]", QUICK_KEYS[key].1)).unwrap_or_default();
    let node = Node { border: UiRect::all(Val::Px(1.0)), padding: UiRect::all(Val::Px(2.0)), overflow: Overflow::clip(), ..node };
    // On the wish list: its border blue, its group named at the bottom.
    let wish = carried.wishlist.get(&item.kind.template);
    let frame = if wish.is_some() { WISHED } else { LINE };
    let mut view = parent.spawn((ItemView(item.id, layer), RelativeCursorPosition::default(), node, BorderColor::all(frame), BackgroundColor(if unknown { Color::srgba_u8(0, 0, 0, 100) } else { THING })));
    if unknown {
        view.insert(UnknownView);
    }
    view.with_children(|thing| {
        picture_in(thing, carried, item, within_line(size), turned, None);
        // Examined: the app's bar along its foot, filled by the share done.
        if examining {
            let track = Node { position_type: PositionType::Absolute, left: Val::Px(0.0), right: Val::Px(0.0), bottom: Val::Px(0.0), height: Val::Px(3.0), ..default() };
            thing.spawn((track, BackgroundColor(Color::BLACK.with_alpha(0.6)))).with_children(|track| {
                track.spawn((ExamineFill(None), Node { width: Val::Percent(0.0), height: Val::Percent(100.0), ..default() }, BackgroundColor(Color::srgb(0.0, 0.627, 1.0))));
            });
        }
        // A magazine worked: the app's look of the game's load component, a
        // bar along its foot filled by the work's share done.
        if carried.magazine_work.as_ref().is_some_and(|work| work.process.magazine == item.id) {
            let track = Node { position_type: PositionType::Absolute, left: Val::Px(0.0), right: Val::Px(0.0), bottom: Val::Px(0.0), height: Val::Px(3.0), ..default() };
            thing.spawn((track, BackgroundColor(Color::BLACK.with_alpha(0.6)))).with_children(|track| {
                track.spawn((AppSpinBar, Node { width: Val::Percent(0.0), height: Val::Percent(100.0), ..default() }, BackgroundColor(WORDS)));
            });
        }
        if let Some(group) = wish {
            let mark = Node { position_type: PositionType::Absolute, left: Val::Px(2.0), bottom: Val::Px(1.0), ..default() };
            thing.spawn(mark).with_children(|mark| {
                mark.spawn(words(group.name().to_owned(), 9.0, WISHED));
            });
        }
        // Its tag, at the top left in its colour (the game's place for it).
        if !item.tag.name.trim().is_empty() {
            let plate = Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), padding: UiRect::horizontal(Val::Px(3.0)), ..default() };
            thing.spawn((plate, BackgroundColor(crate::screens::health_view::tag_colour(item.tag.color)))).with_children(|plate| {
                plate.spawn(words(item.tag.name.clone(), 10.0, WORDS));
            });
        }
        let name = if unknown { "???".to_owned() } else { carried.name(item) };
        thing.spawn(words(format!("{name}{stack}{weight}{key}"), 10.0, WORDS));
    });
}

/// The app's colour of a thing on the wish list.
const WISHED: Color = Color::srgb(0.35, 0.62, 0.86);

/// The repair buff a thing's view shows (`GridItemView.TryGetBuff`): of
/// the thing's and everything's in it, a rare one first, else the first.
pub(super) fn shown_buff(item: &Item) -> Option<trp_sim::items::repair::RepairBuff> {
    fn every(item: &Item, out: &mut Vec<trp_sim::items::repair::RepairBuff>) {
        out.extend(item.buff);
        for inside in item.contents() {
            every(inside, out);
        }
    }
    let mut found = Vec::new();
    every(item, &mut found);
    found.iter().find(|buff| buff.rarity == trp_sim::items::repair::Rarity::Rare).or(found.first()).copied()
}

/// A special slot's view's side (`Special Slot Template`), in units.
pub(super) const SPECIAL_SIDE: f32 = 64.0;

/// The durability's bar's width at a slot's view's right, which its value
/// is put left of (`DurabilitySlider`'s `LayoutElement`, 12 units in the
/// prefab).
pub(super) const VALUE_BAR: f32 = 12.0;

/// Which view a thing's value is written in (`item_value`): a grid's, a
/// slot's, or a special slot's (`SlotItemView._isSpecSlotLayout`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum ValueLayout {
    Grid,
    Slot,
    Special,
}

/// Whether a thing lies in a special slot (`SlotItemAddress.
/// IsSpecialSlotAddress`): a slot whose name holds "SpecialSlot", or any
/// slot of a thing not shown in its slot (the pockets).
pub(super) fn in_special_slot(carried: &Carried, item: &Item) -> bool {
    let Some((holder, Spot::Slot(slot))) = carried.lies(item.id) else { return false };
    let Some(holder) = carried.equipment.item.find(holder).or_else(|| carried.loot.find(holder)) else { return false };
    holder.kind.slots.get(slot).is_some_and(|place| place.name.contains("SpecialSlot")) || holder.kind.not_shown_in_slot
}

/// What a thing's view writes at its bottom right with `TARKOV UI`
/// (`GridItemView.UpdateInfo`, `SetItemValue`, `SetCountValue`), as runs
/// of words each with its colour (the text's own where none) and its size
/// (the text's own where none); nothing for nothing. In a slot a weapon's
/// rounds are written large, in a special slot both figures small
/// (`SlotItemView.ValueFormat`, by its `_isSpecSlotLayout`).
pub(super) fn item_value(carried: &Carried, item: &Item, layout: ValueLayout) -> Vec<(String, Option<Color>, Option<f32>)> {
    let is = |class: &str| item.kind.class_names.iter().any(|name| name == class);
    let hex = |hex: u32| Some(Color::srgb_u8((hex >> 16) as u8, (hex >> 8) as u8, hex as u8));
    let (cream, red_orange, red) = (hex(0xdadabc), hex(0xff5335), hex(0xff0000));
    // A thing not known has "?" for each figure (`SetItemValue` with
    // `display` off); a stack's count is shown all the same.
    let unknown = !trp_sim::items::examine::known(item);
    let hidden = |said: String| if unknown { "?".to_owned() } else { said };
    // Two figures in one colour (`EItemValueFormat.TwoValues`).
    let two = |a: String, b: String, color: Option<Color>| vec![(hidden(a), color, None), (format!("/{}", hidden(b)), color, None)];
    // The view's own format: the first figure in its colour, large in a
    // slot, the second plain; in a special slot both at 11 (`ValueFormat`).
    let (first, second) = match layout {
        ValueLayout::Grid => (None, None),
        ValueLayout::Slot => (Some(30.0), None),
        ValueLayout::Special => (Some(11.0), Some(11.0)),
    };
    let format = |a: String, b: u32, color: Option<Color>| vec![(hidden(a), color, first), (format!("/{}", hidden(b.to_string())), None, second)];
    // A magazine's count as the character knows it (`Magazine.
    // GetAmmoCountByLevel`, with no skill of magazine drills: the app has
    // none): exact when full (one short in a weapon worn, its chamber's),
    // words when part full, `?` unchecked.
    let known = |magazine: &Item, count: u32, most: u32, color: Option<Color>, worn: bool| -> Vec<(String, Option<Color>, Option<f32>)> {
        if unknown {
            return format(String::new(), most, color);
        }
        let said = if count == 0 {
            "0".to_owned()
        } else if !carried.checked.contains(&magazine.id) {
            "?".to_owned()
        } else if count >= if worn { most.saturating_sub(1) } else { most } {
            count.to_string()
        } else {
            const SHARES: [&str; 4] = ["~ Empty", "< 1/2", "~ 1/2", "~ Full"];
            let share = SHARES[((3.33 * count as f32 / most.max(1) as f32 + 0.5) as usize).min(3)];
            return vec![(share.to_owned(), color, None)];
        };
        format(said, most, color)
    };
    let worn = carried.equipment.item.slots.iter().flatten().any(|slotted| slotted.id == item.id);
    if is("Weapon") {
        let magazine = item.slots.iter().zip(&item.kind.slots).find(|(_, holder)| holder.name == trp_sim::items::inventory::MAGAZINE_SLOT).and_then(|(part, _)| part.as_ref());
        let (count, most) = match magazine {
            Some(magazine) => (trp_sim::items::inventory::magazine_rounds(magazine), magazine.kind.capacity),
            // With no magazine only a weapon loaded by its barrels counts
            // its chambers (`GridItemView.UpdateInfo`, `OnlyBarrel`); any
            // other shows nothing.
            None if item.kind.reload_mode == "OnlyBarrel" => {
                let chambers: Vec<&Option<Item>> = item.slots.iter().zip(&item.kind.slots).filter(|(_, holder)| holder.name.starts_with("patron_in_weapon")).map(|(part, _)| part).collect();
                (chambers.iter().filter(|part| part.is_some()).count() as u32, chambers.len() as u32)
            }
            None => (0, 0),
        };
        if most == 0 {
            return Vec::new();
        }
        let color = if (count as f32) / (most as f32) < 0.15 { red } else { cream };
        return match magazine {
            Some(magazine) => known(magazine, count, most, color, worn),
            None => format(count.to_string(), most, color),
        };
    }
    // Armour, a plate or an insert, and a rig that holds plates: its own
    // and its parts' durability summed, over their greatest, white, red at
    // none (`GridItemView.UpdateInfo`: every `RepairableComponent` in it,
    // where it has an `ArmorComponent` or an `ArmorHolderComponent`; each
    // sum rounded). Each at the wear its thing keeps: a blast's on the
    // character's own (`blast_view`), a hit's or a blast's on a dummy's
    // body (`dummy::keep_dummy_wear`).
    if item.kind.armor_class > 0 || is("ArmorPlate") || item.kind.slots.iter().any(|slot| slot.armour) {
        fn summed(item: &Item, sums: &mut [f32; 2]) {
            sums[0] += item.durability()[0];
            sums[1] += item.durability()[1];
            for part in item.slots.iter().flatten() {
                summed(part, sums);
            }
        }
        let mut sums = [0.0, 0.0];
        summed(item, &mut sums);
        if sums[1] > f32::EPSILON {
            let colour = if sums[0] <= 0.0 { hex(0xff0000) } else { hex(0xffffff) };
            return two(sums[0].round().to_string(), sums[1].round().to_string(), colour);
        }
    }
    if is("Magazine") {
        return known(item, trp_sim::items::inventory::magazine_rounds(item), item.kind.capacity, cream, worn);
    }
    if is("AmmoBox") {
        return two(item.contents().map(|round| round.count).sum::<u32>().to_string(), item.kind.capacity.to_string(), cream);
    }
    if is("Meds") && item.kind.max_hp_resource > 0.0 {
        let left = item.resource.unwrap_or(item.kind.max_hp_resource);
        return two(format!("{}", left.round() as i32), format!("{}", item.kind.max_hp_resource.round() as i32), red_orange);
    }
    if is("FoodDrink") && item.kind.max_resource > 1.0 {
        let left = item.resource.unwrap_or(item.kind.max_resource);
        return two(format!("{}", left.round() as i32), format!("{}", item.kind.max_resource.round() as i32), red_orange);
    }
    if is("BarterItem") && item.kind.max_resource > 0.0 {
        let left = item.resource.unwrap_or(item.kind.max_resource);
        return two(format!("{}", left as i32), format!("{}", item.kind.max_resource), cream);
    }
    // A repair kit's points left over its greatest, white, red at none:
    // floored above half, ceiled below (`GridItemView.UpdateInfo`).
    if is("RepairKits") && item.kind.repair_resource > 0.0 {
        let (left, most) = (crate::screens::repair_view::kit_resource(item), item.kind.repair_resource);
        let shown = if left > most / 2.0 { left.floor() } else { left.ceil() };
        let colour = if left.ceil() <= 0.0 { red } else { hex(0xffffff) };
        return two(format!("{}", shown as i32), format!("{}", most as i32), colour);
    }
    // A key's uses left (the app keeps no count of a key's uses: as new).
    if is("Key") && item.kind.key_uses > 0 {
        return two(item.kind.key_uses.to_string(), item.kind.key_uses.to_string(), red_orange);
    }
    // A stack's count, in its colour (`SetCountValue`, `GetStackColor`).
    if item.count != 1 {
        return vec![(item.count.to_string(), Some(crate::screens::health_view::stack_color(&item.kind.class_names)), None)];
    }
    Vec::new()
}
