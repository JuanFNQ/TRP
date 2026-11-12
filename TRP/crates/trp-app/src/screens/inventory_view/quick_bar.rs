//! The quick bar and the pictures and values of its slots.

use super::*;

/// What the mouse and the keys do in the screen, in the app's own words.
pub(super) const HELP: &str = "Hold a thing with the left button and let it go on a cell, a slot or another thing; R turns it.\nCtrl and a click sends it to the other side. Alt and a click puts it on. A double click uses medicine, food or drink, opens a container, or shows what a thing is. The right button on a thing opens its menu.\nCtrl held as a stack is let go asks how many.\nA quick-use key let go over a thing in the vest or the pockets binds it.\nThe discard key puts the thing under the mouse down.";

/// The quick-use keys in the bar's order: the game's name for each
/// binding and the word for it where the install's bindings have none.
pub(super) const QUICK_KEYS: [(&str, &str); 7] = [("Slot4", "4"), ("Slot5", "5"), ("Slot6", "6"), ("Slot7", "7"), ("Slot8", "8"), ("Slot9", "9"), ("Slot0", "0")];

/// The quick-use panel where the install's scene could not be read: its
/// width and height, how far its lower edge is over the screen's and its
/// middle to the right of the screen's, and the side and step of its
/// views, as the install's scene had them when first read.
pub(super) const QUICK_PANEL: [f32; 4] = [854.2, 115.0, 30.0, 25.6];

const QUICK_SIDE: f32 = 64.0;

const QUICK_STEP: f32 = 70.0;

/// The quick-use bar (`InventoryScreenQuickAccessPanel`): a view for the
/// grenade key, one for each of the four slots that have a key of their
/// own (the sheath, the holster, the sling, the back) showing what is in
/// the slot, and one for each of the seven keys things are bound to.
/// Where each sits along the row is the install's scene's; it is shown
/// and nothing is done to it with the mouse.
/// What a quick-use view is for, by the game's name of its binding
/// (`EBoundItem`, or the view's own key): a slot with a key of its own (the
/// sheath, the holster, the sling, the back) with what is in it, or one of
/// the seven keys with what is bound to it.
pub(crate) fn quick_thing<'a>(carried: &'a Carried, key: &str) -> (&'static str, Option<&'a Item>) {
    let worn = &carried.equipment.item;
    match key {
        "Grenade" | "G" => ("ThrowGrenade", carried.top_grenade.and_then(|id| worn.find(id))),
        "ItemV" | "V" => ("Knife", worn.in_slot("Scabbard")),
        "Item1" | "1" => ("SecondaryWeapon", worn.in_slot("Holster")),
        "Item2" | "2" => ("PrimaryWeaponFirst", worn.in_slot("FirstPrimaryWeapon")),
        "Item3" | "3" => ("PrimaryWeaponSecond", worn.in_slot("SecondPrimaryWeapon")),
        other => match other.trim_start_matches("Item").parse::<usize>().ok().and_then(|number| number.checked_sub(4)).filter(|at| *at < QUICK_KEYS.len()) {
            Some(at) => (QUICK_KEYS[at].0, carried.bound.items[at].and_then(|id| worn.find(id))),
            None => ("", None),
        },
    }
}

/// The words of a binding's keys, or the app's where the install has none.
pub(crate) fn quick_key_words(bindings: &Bindings, binding: &str, own: &str) -> String {
    match bindings.caption(binding) {
        "" => QUICK_KEYS.iter().find(|(name, _)| *name == binding).map_or(own, |(_, word)| word).to_owned(),
        word => word.to_owned(),
    }
}

pub(super) fn quick_bar(commands: &mut Commands, carried: &Carried, bindings: &Bindings, units: Units, window: Vec2, look: &crate::screens::health_view::HealthLook, said: &crate::screens::health_view::Words) {
    let s = units.0;
    let thing = |key: &str| quick_thing(carried, key);
    let key_words = |binding: &str, own: &str| quick_key_words(bindings, binding, own);
    // With `TARKOV UI`, the game's own panel where the game's screen has it
    // (`health_view::draw_quick`), each view's thing in its place
    // (`QuickSlotItemView`: its picture turned and fitted, and no ground of
    // its kind's colour, `courses10`'s `hover-tooltips` at frame 3000: its
    // `Background` is not drawn), the views' labels and borders over them.
    if let (true, Some(quick)) = (look.tarkov_ui, look.quick.as_ref()) {
        let screen = [window.x / s.max(1e-3), SCREEN_HEIGHT];
        let [x, y, width, height] = quick.quick_in(screen);
        let words: HashMap<String, (String, String)> = quick
            .slots
            .iter()
            .map(|slot| {
                let (binding, item) = thing(&slot.key);
                let own = slot.key.trim_start_matches("Item").replace("10", "0");
                (slot.key.clone(), (key_words(binding, &own), item.map_or(String::new(), |item| carried.name(item))))
            })
            .collect();
        // A weapon's view red with a malfunction the shooter knows of
        // (`QuickSlotView.method_0`).
        let marks: HashMap<String, crate::screens::health_view::SlotMark> = quick.slots.iter().filter_map(|slot| Some((slot.key.clone(), crate::screens::health_view::SlotMark::of(thing(&slot.key).1?)?))).collect();
        let size = [width, height];
        // (On the screen's group, as the game's `Items Panel` is.)
        let on = crate::ui::screen_fade::OnScreen::Inventory;
        commands.spawn((Overlay, Cover(BAR_LAYER), RelativeCursorPosition::default(), placed(x * s, y * s, width * s, height * s), GlobalZIndex(12), on)).with_children(|bar| {
            crate::screens::health_view::draw_quick(bar, look, said, size, &words, &marks, 0, s);
            for (key, [ix, iy, iw, ih]) in crate::screens::health_view::quick_places(look, size) {
                let (_, Some(item)) = thing(&key) else { continue };
                let place = Node { overflow: Overflow::clip(), ..placed(ix * s, iy * s, iw * s, ih * s) };
                bar.spawn(place).with_children(|place| {
                    quick_picture(place, carried, item, [iw, ih], s);
                    quick_value(place, look, item, [iw, ih], s);
                });
            }
            crate::screens::health_view::draw_quick(bar, look, said, size, &words, &marks, 1, s);
        });
        return;
    }
    let [width, height, up, right] = if carried.panel.quick[0] > 0.0 { carried.panel.quick } else { QUICK_PANEL };
    // The views by the scene's own key for each, left to right.
    let read = &carried.panel.quick_slots;
    let fallback: Vec<(String, f32, f32)> = ["G", "V", "1", "2", "3", "4", "5", "6", "7", "8", "9", "10"]
        .iter()
        .enumerate()
        .map(|(at, key)| (key.to_string(), at as f32 * QUICK_STEP + if at > 4 { 10.0 } else { 0.0 }, QUICK_SIDE))
        .collect();
    let views = if read.is_empty() { &fallback } else { read };
    let (first, last) = (views.iter().map(|(_, middle, side)| middle - side / 2.0).fold(f32::MAX, f32::min), views.iter().map(|(_, middle, side)| middle + side / 2.0).fold(f32::MIN, f32::max));
    let margin = (width - (last - first)) / 2.0 - first;
    let node = placed(window.x / 2.0 + (right - width / 2.0) * s, window.y - (up + height) * s, width * s, height * s);
    commands.spawn((Overlay, Cover(BAR_LAYER), RelativeCursorPosition::default(), node, BackgroundColor(PANEL.with_alpha(0.96)), GlobalZIndex(12), crate::ui::screen_fade::OnScreen::Inventory)).with_children(|bar| {
        bar.spawn((placed(8.0 * s, 4.0 * s, 300.0 * s, 18.0 * s), words("QUICK USE", 10.0, FAINT)));
        for (key, middle, side) in views {
            // What the view is for: a slot with a key of its own, or one
            // of the seven keys.
            let (binding, item) = thing(key);
            if binding.is_empty() {
                continue;
            }
            let caption = key_words(binding, key);
            let size = Vec2::splat(side * s);
            let view = placed((middle - side / 2.0 + margin) * s, (height - side - 10.0) * s, size.x, size.y);
            bar.spawn((Node { border: UiRect::all(Val::Px(1.0)), overflow: Overflow::clip(), ..view }, BorderColor::all(LINE), BackgroundColor(SLOT))).with_children(|view| {
                if let Some(item) = item {
                    picture_in(view, carried, item, within_line(size), false, None);
                    view.spawn((placed(2.0, size.y - 16.0, size.x - 4.0, 14.0), words(carried.name(item), 9.0, WORDS)));
                }
                view.spawn((placed(3.0, 1.0, size.x - 6.0, 14.0), words(caption, 10.0, FAINT)));
            });
        }
    });
}

/// A thing's picture as an image of the screen, as it lies, at so much of
/// its alpha.
pub(crate) fn picture_image(picture: &Picture, alpha: f32) -> ImageNode {
    ImageNode::new(picture.upright.clone()).with_color(Color::WHITE.with_alpha(alpha))
}

/// A thing's picture in a quick-use view (`QuickSlotItemView.UpdateScale`):
/// turned 45 degrees and made as large as fits the view (`room`, units):
/// the picture's own size (its cells, 63 units to a cell) turned, each of
/// its two sides brought within the view's, the smaller share taken (a
/// square picture by its height alone). While it is to come, or where it
/// cannot be made, the view's loader in the middle (`_iconLoader`, the
/// quick view's `Small Loader`: the same picture and size as a slot's
/// view's, which is drawn), its picture put in its place as it comes
/// (`fill_pictures`): a weapon given a magazine of another kind shows its
/// new build's picture once made. The plain shape of its sort
/// only where no picture is made (`ICON GENERATION` off).
pub(crate) fn quick_picture(place: &mut ChildSpawnerCommands, carried: &Carried, item: &Item, room: [f32; 2], s: f32) {
    let size = quick_fit(item.cells(), room);
    let spot = Rect::from_corners(Vec2::new((room[0] - size.x) / 2.0, (room[1] - size.y) / 2.0) * s, Vec2::new((room[0] + size.x) / 2.0, (room[1] + size.y) / 2.0) * s);
    // Red for a weapon with a malfunction the shooter knows of
    // (`QuickSlotItemView.UpdateInfo`).
    let tint = crate::screens::health_view::SlotMark::picture(crate::screens::health_view::SlotMark::of(item));
    let key = shown_key(carried, item);
    if let Some(picture) = carried.pictures.get(&key) {
        place.spawn(quick_picture_node(picture, spot, tint));
        return;
    }
    let [across, down] = item.cells();
    if carried.pictures_on && across > 0 && down > 0 {
        let loader = carried.item_looks[0].as_ref().and_then(|look| look.loader.as_ref()).map(|(sprite, side, covers)| (sprite, Vec2::from(*side) * s, *covers));
        let room = Rect::from_corners(Vec2::ZERO, Vec2::new(room[0], room[1]) * s);
        place.spawn(spinner_node(carried, Loading { key, room, turned: false, quick: Some((spot, tint)) }, loader));
    } else if let Some(picture) = carried.icons.get(icon_of(item)) {
        place.spawn(quick_picture_node(picture, spot, tint));
    }
}

/// A quick-use view's picture in its place (pixels), turned 45 degrees, in
/// its colour.
pub(super) fn quick_picture_node(picture: &Picture, spot: Rect, tint: Color) -> impl Bundle {
    (ImageNode::new(picture.upright.clone()).with_color(tint), placed(spot.min.x, spot.min.y, spot.width(), spot.height()), UiTransform { rotation: Rot2::degrees(-45.0), ..default() })
}

/// What a quick-use view writes under its thing (`QuickSlotItemView.
/// TryGetResourceText`, its components asked in its order), and the
/// words' colour where it is not the text's own: a knife's poison's doses
/// left over its most in green (`SideEffectComponent`, `<color=#009225>`,
/// to two places, none kept where nought: `0.##`; spent by its blows on
/// the dummies); a medkit's resource over its most
/// (`MedKitComponent`, on meds whose most is above nought) and food's or
/// drink's where its most is above one (`FoodDrinkComponent`), each
/// rounded. (A barter thing's `ResourceComponent` comes second, but none
/// can be bound: `IsAtBindablePlace`.) Nothing for anything else.
pub(crate) fn quick_resource(item: &Item) -> Option<(String, Option<Color>)> {
    let is = |class: &str| item.kind.class_names.iter().any(|name| name == class);
    let hundredths = |value: f32| {
        let said = format!("{:.2}", (value as f64 * 100.0).round() / 100.0);
        said.trim_end_matches('0').trim_end_matches('.').to_owned()
    };
    let whole = |value: f32| format!("{}", value.round() as i64);
    if is("Knife") && item.kind.side_effect {
        // Its poison's doses left (`SideEffectComponent.Value`: spent by a
        // blow that puts it on, `dummy::blow_dummies`).
        let most = item.kind.max_resource;
        return Some((format!("{}/{}", hundredths(item.resource.unwrap_or(most)), hundredths(most)), Some(Color::srgb_u8(0x00, 0x92, 0x25))));
    }
    if is("Meds") && item.kind.max_hp_resource > 0.0 {
        let most = item.kind.max_hp_resource;
        return Some((format!("{}/{}", whole(item.resource.unwrap_or(most)), whole(most)), None));
    }
    if is("FoodDrink") && item.kind.max_resource > 1.0 {
        let most = item.kind.max_resource;
        return Some((format!("{}/{}", whole(item.resource.unwrap_or(most)), whole(most)), None));
    }
    None
}

/// A quick-use view's figure (`TARKOV UI`; `QuickSlotItemView.UpdateInfo`),
/// over its thing's picture in a view so large (`room`, units), where the
/// thing has one (`quick_resource`) and the prefab was read.
pub(crate) fn quick_value(place: &mut ChildSpawnerCommands, look: &crate::screens::health_view::HealthLook, item: &Item, room: [f32; 2], s: f32) {
    let (Some(value), Some((said, color))) = (look.quick_value.as_ref(), quick_resource(item)) else { return };
    let Some(text) = value.panel.nodes[value.value].text.as_ref() else { return };
    let Some([x, y, w, h]) = value.place(look, &said, room) else { return };
    let color = color.unwrap_or_else(|| crate::screens::health_view::rgba(text.color));
    let justify = match text.horizontal {
        1 => JustifyContent::FlexStart,
        2 => JustifyContent::Center,
        _ => JustifyContent::FlexEnd,
    };
    let frame = Node { justify_content: justify, align_items: AlignItems::Center, ..placed(x * s, y * s, w * s, h * s) };
    place.spawn(frame).with_children(|frame| match &value.tmp {
        Some((asset, material)) => {
            let run = trp_data::interface::tmp_text::TmpRun { text: said, size: text.size * s, color: crate::screens::health_view::screen_numbers(color), bold: text.style & 1 != 0 };
            let layout = trp_data::interface::tmp_text::TmpLayout { em: text.size * s, spacing: text.spacing, kerning: text.kerning, wrap: None, align: text.horizontal & 7, lead: 0.0 };
            crate::screens::health_view::tmp_words(frame, Node::default(), crate::screens::health_view::TmpText { asset: asset.clone(), material: material.clone(), runs: vec![run], layout }, &mut |_| {});
        }
        None => {
            let font = TextFont { font: value.font.clone(), font_size: text.size * s, ..default() };
            crate::screens::health_view::effect_words(frame, Node::default(), &[(said, font, color)], value.effect, text.size * s, &mut |_, _, _| {});
        }
    });
}

/// How large a thing's picture of so many cells is drawn in a quick-use
/// view so large (units), before it is turned (`QuickSlotItemView.
/// UpdateScale`: its size turned 45 degrees, `(w - h, w + h)` over the
/// square root of two, each part within the view's, the smaller share).
pub(crate) fn quick_fit(cells: [u32; 2], room: [f32; 2]) -> Vec2 {
    let own = Vec2::new(cells[0] as f32 * CELL, cells[1] as f32 * CELL);
    let turned = Vec2::new((own.x - own.y).abs(), own.x + own.y) * std::f32::consts::FRAC_1_SQRT_2;
    let share = [room[0] / turned.x, room[1] / turned.y].into_iter().filter(|share| share.is_finite()).fold(f32::MAX, f32::min);
    own * share
}
