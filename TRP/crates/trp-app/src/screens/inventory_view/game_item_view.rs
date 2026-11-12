//! The game's own item view drawn for a thing.

use super::*;

/// A thing's view drawn as the game's own (`TARKOV UI`; `health_view::
/// ItemViewLook`): its ground in its kind's colour (`ItemView.
/// UpdateStaticInfo`), its border where its view has one on, its picture,
/// its caption at the top right (the short name between two spaces in
/// #b6c1c7: `GridItemView.GetItemName`), a stack's count at the bottom
/// right (`SetCountValue`, in its colour: `GetStackColor`), and the label
/// of the key it is bound to at the top left (`BindPanel`); `s` pixels to
/// a unit. A view's top and bottom rows are laid out by groups that fit
/// their contents while the game runs: the label is put at the top left
/// at its picture's own size, the count against the bottom row's right
/// and bottom edges, as those groups put them.
#[allow(clippy::too_many_arguments)]
pub(crate) fn game_item_view(parent: &mut ChildSpawnerCommands, carried: &Carried, item: &Item, node: Node, size: Vec2, turned: bool, layer: u8, s: f32, look: &crate::screens::health_view::ItemViewLook) {
    // Unusable by a malfunction the shooter knows of, or a weapon whose
    // magazine has on its top a round the chamber does not take: its picture
    // red on a dark red ground (`GridItemView.UpdateInfo`: `UnusableDueToMalfunction`,
    // `Weapon.CompatibleAmmo`; firing spec, "Rounds the chamber does not take").
    let root = if carried.equipment.item.find(item.id).is_some() { &carried.equipment.item } else { &carried.loot };
    // A thing not known: its ground black at 100 of 255, before all else
    // (`GridItemView.UpdateInfo`); and one being examined.
    let (unknown, examining) = (!trp_sim::items::examine::known(item), carried.examiner.examining(item.id));
    let refused = is(item, "Weapon") && trp_sim::items::inventory::ammo_incompatible(item);
    let unusable = !unknown && (trp_sim::items::inventory::unusable_by_malfunction(root, item) || refused);
    // A repair buff on it or on anything in it, a rare one first
    // (`GridItemView.TryGetBuff`): its ground the buff's (209, 153, 45 at
    // 32 of 255) where nothing else colours it, and its mark over its
    // value, known (`ChangeRepairBuffStatus`).
    let buffed = if unknown { None } else { shown_buff(item) };
    let ground = if unknown {
        Color::srgba_u8(0, 0, 0, 100)
    } else if unusable {
        Color::srgba_u8(83, 0, 0, 63)
    } else if buffed.is_some() {
        Color::srgba_u8(209, 153, 45, 32)
    } else {
        crate::screens::health_view::item_ground(&item.kind.color)
    };
    // Framed as the game's (`health_view::ViewFrame`): its ground inside a
    // border of its own that draws nothing (Bevy fills a node's ground
    // inside its border: the game's `Color Panel`, a unit in), its line
    // drawn by the game's picture over the whole view. Else the app's
    // line, a pixel wide, where the view has a border.
    let frame = look.frame.as_ref();
    let border = if frame.is_some() { Color::NONE } else { look.border.unwrap_or(Color::NONE) };
    let edges = match frame {
        Some(frame) => {
            let [left, top, right, bottom] = frame.inset.map(|units| Val::Px(units * s));
            UiRect { left, top, right, bottom }
        }
        None => UiRect::all(Val::Px(if look.border.is_some() { 1.0 } else { 0.0 })),
    };
    // Cut at its outer edge, not inside its border: the game's border is a
    // picture over the view that takes no room, and cut inside it, a
    // caption's last letter and its dark rim at the right were lost.
    // On the wish list (`WishlistGridView`), its marks; a slot's view's
    // border lies outside it, and is not cut away.
    let wish = carried.wishlist.get(&item.kind.template).zip(look.wish.as_ref()).filter(|_| !unknown);
    let outside = if wish.is_some() { 3.0 * s } else { 0.0 };
    let node = Node {
        border: edges,
        overflow: Overflow::clip(),
        overflow_clip_margin: OverflowClipMargin::border_box().with_margin(outside),
        ..node
    };
    let units = [size.x / s, size.y / s];
    let panel = &look.panel;
    // A thing not known, framed as the game's: its noise drawn first and
    // its colour panel over it, as the game draws them
    // (`NotExaminedBackground`, then `Color Panel`), so that the panel lit
    // under the mouse (white at 0.302) lies over the noise: the view's
    // ground is a node of its own after the noise (`UnknownGround`), which
    // the light goes to (`light_related`).
    let noise = look.unexamined.as_ref().filter(|_| unknown);
    let ground_apart = noise.is_some() && frame.is_some();
    let own_ground = if ground_apart { Color::NONE } else { ground };
    let mut view = parent.spawn((ItemView(item.id, layer), Ground(ground, border), RelativeCursorPosition::default(), node, BorderColor::all(border), BackgroundColor(own_ground)));
    if unusable {
        view.insert(Unusable);
    }
    if unknown {
        view.insert(UnknownView);
    }
    // Framed, its parts laid out from the view's own corner, as the game
    // lays them, in a node over the whole view (what lies in a node is
    // laid out from inside its border).
    let parts = match frame {
        Some(frame) => {
            let whole = Node { position_type: PositionType::Absolute, left: Val::Px(-frame.inset[0] * s), top: Val::Px(-frame.inset[1] * s), width: Val::Px(size.x), height: Val::Px(size.y), ..default() };
            let inside = view.commands().spawn((ViewInside(item.id), whole)).id();
            if unusable {
                view.commands().entity(inside).insert(Unusable);
            }
            if unknown {
                view.commands().entity(inside).insert(UnknownView);
            }
            view.add_child(inside);
            inside
        }
        None => view.id(),
    };
    // The room its picture fills: its icon, made as large as its cells and
    // a unit more, at its own size in the view's middle (`SetNativeSize`;
    // the whole view in a grid), shrunk to fit where it is larger than the
    // view either way, never grown (`ItemView.UpdateScale` with the slot's
    // place as `_iconScale`: a rig or a backpack in its slot, which the app
    // had drawn too small).
    let native = {
        let [w, h] = item.cells();
        let (w, h) = if turned { (h, w) } else { (w, h) };
        Vec2::new(w as f32 * CELL + 1.0, h as f32 * CELL + 1.0) * s
    };
    let room = if native.x <= size.x + 0.01 && native.y <= size.y + 0.01 { Rect::from_center_size(size / 2.0, native) } else { Rect::from_corners(Vec2::ZERO, size) };
    let mut ground_node = None;
    view.commands().entity(parts).with_children(|thing| {
        // A thing not known: its ground's noise over the whole view, tiled
        // (`NotExaminedBackground`), then its colour panel over it, a unit
        // in (the frame's inset). Not framed, the noise is drawn over the
        // view's own ground, its colour darkened by the black's share
        // (1 - 100/255), which comes to the same but under the mouse.
        if let Some((index, fill)) = noise {
            let [x, y, w, h] = panel.placed(*index, units).rect;
            let keep = if ground_apart { 1.0 } else { 1.0 - 100.0 / 255.0 };
            let color = fill.color.to_srgba();
            let tint = Color::srgba(color.red * keep, color.green * keep, color.blue * keep, color.alpha);
            crate::screens::health_view::draw_sprite_tiled(thing, fill, [x * s, y * s, w * s, h * s], s, tint);
            if let (true, Some(frame)) = (ground_apart, frame) {
                let [left, top, right, bottom] = frame.inset.map(|units| units * s);
                let inside = placed(left, top, size.x - left - right, size.y - top - bottom);
                ground_node = Some(thing.spawn((UnknownGround, inside, crate::screens::health_view::EXACT, BackgroundColor(ground))).id());
            }
        }
        // Its glow, under its picture.
        if let Some((_, (index, glow))) = wish.and_then(|(group, wish)| Some((group, wish.glow.as_ref()?))) {
            let [x, y, w, h] = panel.placed(*index, units).rect;
            crate::screens::health_view::draw_sprite(thing, glow, [x * s, y * s, w * s, h * s], s);
        }
        picture_in(thing, carried, item, room, turned, look.loader.as_ref().map(|(sprite, side, covers)| (sprite, Vec2::from(*side) * s, *covers)));
        // Being used: its drain loader at its own size in the view's
        // middle, turned as a loader is (`fill_pictures`), not faded with
        // the blinking picture.
        if let (true, Some((sprite, side, covers))) = (carried.drained == Some(item.id), look.drain.as_ref()) {
            let side = Vec2::from(*side) * s;
            thing.spawn((SpinnerPicture(sprite.image.clone(), sprite.cut, sprite.color, *covers), placed((size.x - side.x) / 2.0, (size.y - side.y) / 2.0, side.x, side.y), DrainSpinner));
        }
        // Its tag (`GridItemView.UpdateTag`, `ResizeTag`): the plate at the
        // view's top left in the tag's colour, the name on it where the
        // caption leaves room.
        if let Some(tag) = look.tag.as_ref().filter(|_| !item.tag.name.trim().is_empty()) {
            let [x, y, _, h] = panel.placed(tag.plate, units).rect;
            let caption = format!(" {} ", carried.view_name(item));
            let (width, named) = tag.width(panel, look.caption, &caption, &item.tag.name, units[0]);
            thing.spawn((placed(x * s, y * s, width * s, h * s), BackgroundColor(crate::screens::health_view::tag_colour(item.tag.color)))).with_children(|plate| {
                let Some(text) = panel.nodes[tag.name].text.as_ref().filter(|_| named) else { return };
                let color = Color::srgba(text.color[0], text.color[1], text.color[2], text.color[3]);
                let frame = Node { width: Val::Percent(100.0), height: Val::Percent(100.0), align_items: AlignItems::Center, ..default() };
                plate.spawn(frame).with_children(|frame| match &tag.name_tmp {
                    Some((asset, material)) => {
                        let run = trp_data::interface::tmp_text::TmpRun { text: item.tag.name.clone(), size: text.size * s, color: crate::screens::health_view::screen_numbers(color), bold: text.style & 1 != 0 };
                        let layout = trp_data::interface::tmp_text::TmpLayout { em: text.size * s, spacing: text.spacing, kerning: text.kerning, wrap: None, align: text.horizontal & 7, lead: 0.0 };
                        crate::screens::health_view::tmp_words(frame, Node::default(), crate::screens::health_view::TmpText { asset: asset.clone(), material: material.clone(), runs: vec![run], layout }, &mut |_| {});
                    }
                    None => {
                        let font = TextFont { font: tag.name_font.clone(), font_size: text.size * s, ..default() };
                        crate::screens::health_view::effect_words(frame, Node::default(), &[(item.tag.name.clone(), font, color)], None, text.size * s, &mut |_, _, _| {});
                    }
                });
            });
        }
        // Its border over its picture and tag, under its words.
        if let Some(frame) = frame {
            crate::screens::health_view::draw_sprite_pixels(thing, &frame.border, [0.0, 0.0, size.x, size.y]);
        }
        // The caption, as its text lies in its rectangle; hidden while the
        // thing is examined (`OnBeingExaminedChanged`). Its figures and its
        // round are hidden there too, but shown again at once by the view's
        // `UpdateInfo` that `GridItemView.OnExamine` calls next: drawn
        // (`courses10`'s `examine-multiple`, a key's "?/?" through each
        // examine; the round read from the code only).
        if let Some((index, text)) = look.caption.filter(|_| !examining).and_then(|index| Some((index, panel.nodes[index].text.as_ref()?))) {
            let [x, y, w, h] = panel.placed(index, units).rect;
            let justify = match text.horizontal {
                1 => JustifyContent::FlexStart,
                2 => JustifyContent::Center,
                _ => JustifyContent::FlexEnd,
            };
            let align = match text.vertical {
                256 => AlignItems::FlexStart,
                1024 => AlignItems::FlexEnd,
                _ => AlignItems::Center,
            };
            let frame = Node { justify_content: justify, align_items: align, ..placed(x * s, y * s, w * s, h * s) };
            let font = TextFont { font: look.caption_font.clone(), font_size: text.size * s, weight: crate::screens::health_view::weight_of(text.style), ..default() };
            let size = font.font_size;
            // What of it fits its box, as its text's overflow cuts it (the
            // caption's: truncated, so "FRONT PL" of a long name, the
            // user's capture).
            let said = match look.caption_face.as_ref() {
                Some(face) => crate::screens::health_view::fitted_words(face, text, &format!(" {} ", carried.view_name(item)), w),
                None => format!(" {} ", carried.view_name(item)),
            };
            let color = Color::srgb_u8(0xb6, 0xc1, 0xc7);
            // A caption that wraps (the edit build screen's parts',
            // `modding_selectable_layout`'s `Name`: `m_enableWordWrapping`
            // on, its overflow masked) breaks at its rectangle's width
            // between its margins, its lines going on down past its height:
            // "Mk47" over "254mm" in a part's box.
            let wrap = text.wraps.then_some((w - text.margin[0] - text.margin[2]) * s);
            thing.spawn(frame).with_children(|frame| match &look.caption_tmp {
                // As the game draws it.
                Some((asset, material)) => {
                    let run = trp_data::interface::tmp_text::TmpRun { text: said, size, color: crate::screens::health_view::screen_numbers(color), bold: text.style & 1 != 0 };
                    let layout = trp_data::interface::tmp_text::TmpLayout { em: text.size * s, spacing: text.spacing, kerning: text.kerning, wrap, align: text.horizontal & 7, lead: 0.0 };
                    crate::screens::health_view::tmp_words(frame, Node::default(), crate::screens::health_view::TmpText { asset: asset.clone(), material: material.clone(), runs: vec![run], layout }, &mut |_| {});
                }
                None => crate::screens::health_view::effect_words(frame, Node::default(), &[(said, font, color)], look.caption_effect, size, &mut |_, _, _| {}),
            });
        }
        let is = |class: &str| item.kind.class_names.iter().any(|name| name == class);
        // A slot's view of a weapon or a knife has its durability's bar at
        // its right (`ValidateDurability`), and of a weapon its mastering
        // (`ValidateMastering`); neither in a special slot (`_isSpecSlotLayout`:
        // a flare in the pockets' slots shows no mastery, durability or the
        // like).
        let slotted = look.mastering.is_some();
        let special = slotted && in_special_slot(carried, item);
        let bar = slotted && !special && (is("Weapon") || is("Knife")) && look.durability.is_some() && !examining;
        // The weapon's round: in a slot under the caption, in a grid at the
        // bottom left (`SetInscription`, `Weapon.AmmoCaliber`).
        if let Some((index, text)) = look.inscription.filter(|_| is("Weapon") && !item.kind.caliber.is_empty()).and_then(|index| Some((index, panel.nodes[index].text.as_ref()?))) {
            let [x, y, w, h] = panel.placed(index, units).rect;
            let frame = match text.horizontal {
                1 => Node { position_type: PositionType::Absolute, left: Val::Px(3.0 * s), bottom: Val::Px(3.0 * s), ..default() },
                _ => Node { justify_content: JustifyContent::FlexEnd, align_items: AlignItems::Center, ..placed(x * s, y * s, w * s, h * s) },
            };
            let font = TextFont { font: look.inscription_font.clone(), font_size: text.size * s, ..default() };
            let size = font.font_size;
            let said = if unknown { "?".to_owned() } else { item.kind.caliber.replace("Caliber", "") };
            let color = crate::screens::health_view::rgba(text.color);
            thing.spawn(frame).with_children(|frame| match &look.inscription_tmp {
                Some((asset, material)) => {
                    let run = trp_data::interface::tmp_text::TmpRun { text: said, size, color: crate::screens::health_view::screen_numbers(color), bold: text.style & 1 != 0 };
                    let layout = trp_data::interface::tmp_text::TmpLayout { em: text.size * s, spacing: text.spacing, kerning: text.kerning, wrap: None, align: text.horizontal & 7, lead: 0.0 };
                    crate::screens::health_view::tmp_words(frame, Node::default(), crate::screens::health_view::TmpText { asset: asset.clone(), material: material.clone(), runs: vec![run], layout }, &mut |_| {});
                }
                None => crate::screens::health_view::effect_words(frame, Node::default(), &[(said, font, color)], look.inscription_effect, size, &mut |_, _, _| {}),
            });
        }
        // What it holds or has left, against the bottom row's right and
        // bottom (left of the durability's bar where it has one).
        let runs = item_value(carried, item, if special { ValueLayout::Special } else if slotted { ValueLayout::Slot } else { ValueLayout::Grid });
        let mark = buffed.and_then(|buff| Some((look.buff_icons.get(buff.attribute())?, look.buff_spacing?)));
        let has_runs = !runs.is_empty();
        if let (true, Some((index, text))) = (has_runs || mark.is_some(), look.value.and_then(|index| Some((index, panel.nodes[index].text.as_ref()?)))) {
            let row = panel.nodes[index].parent.and_then(|group| panel.nodes[group].parent).unwrap_or(index);
            let [x, y, w, h] = panel.placed(row, units).rect;
            let slider = if bar { VALUE_BAR } else { 0.0 };
            let (right, bottom) = ((units[0] - x - w).max(0.0) + slider, (units[1] - y - h).max(0.0));
            let at = Node { position_type: PositionType::Absolute, right: Val::Px(right * s), bottom: Val::Px(bottom * s), ..default() };
            let base = crate::screens::health_view::rgba(text.color);
            let font = |size: Option<f32>| TextFont { font: look.value_font.clone(), font_size: size.unwrap_or(text.size) * s, ..default() };
            // Drawn with its font's rim (Bender Outline: the dark edge round
            // a medkit's red figures), as wide as the largest run asks.
            let largest = runs.iter().map(|(_, _, size)| size.unwrap_or(text.size)).fold(text.size, f32::max) * s;
            // An armour holder's with plates in it, known, takes the mouse
            // for the tooltip of its points (`GridItemView.
            // ValuePointerEventsProxy`, `IsValueProxyActive`): its words'
            // box, as its group fits them.
            let plated = !unknown && item.kind.slots.iter().zip(&item.slots).any(|(holder, part)| holder.armour && part.as_ref().is_some_and(|part| trp_sim::items::inventory::is(part, "ArmorPlate")));
            let draw =|thing: &mut ChildSpawnerCommands, at: Node| {
                if let Some((asset, material)) = &look.value_tmp {
                    // As the game draws it: each run at its own size and colour.
                    let runs = runs
                        .into_iter()
                        .map(|(said, color, size)| trp_data::interface::tmp_text::TmpRun { text: said, size: size.unwrap_or(text.size) * s, color: crate::screens::health_view::screen_numbers(color.unwrap_or(base)), bold: text.style & 1 != 0 })
                        .collect();
                    let layout = trp_data::interface::tmp_text::TmpLayout { em: text.size * s, spacing: text.spacing, kerning: text.kerning, wrap: None, align: text.horizontal & 7, lead: 0.0 };
                    crate::screens::health_view::tmp_words(thing, at, crate::screens::health_view::TmpText { asset: asset.clone(), material: material.clone(), runs, layout }, &mut |_| {});
                } else {
                    let words: Vec<(String, TextFont, Color)> = runs.into_iter().map(|(said, color, size)| (said, font(size), color.unwrap_or(base))).collect();
                    crate::screens::health_view::effect_words(thing, at, &words, look.value_effect, largest, &mut |_, _, _| {});
                }
            };
            match mark {
                // The buff's mark over the value, the column's spacing
                // between, both against its right (the bottom right
                // column: lower right, the icon at its own size).
                Some((icon, spacing)) => {
                    let column = Node { flex_direction: FlexDirection::Column, align_items: AlignItems::FlexEnd, row_gap: Val::Px(spacing * s), ..at };
                    thing.spawn(column).with_children(|column| {
                        let size = Node { width: Val::Px(icon.size[0] * s), height: Val::Px(icon.size[1] * s), ..default() };
                        column.spawn((size, crate::screens::health_view::EXACT, ImageNode { image: icon.image.clone(), color: icon.color, rect: Some(icon.cut), flip_y: true, ..default() }));
                        if has_runs && plated {
                            column.spawn((ValueProxy(item.id, layer), RelativeCursorPosition::default(), Node::default())).with_children(|proxy| draw(proxy, Node::default()));
                        } else if has_runs {
                            draw(column, Node::default());
                        }
                    });
                }
                None if plated => {
                    thing.spawn((ValueProxy(item.id, layer), RelativeCursorPosition::default(), at)).with_children(|proxy| draw(proxy, Node::default()));
                }
                None => draw(thing, at),
            }
        }
        // The durability's bar: its track and its fill, full for a weapon
        // as made (`DurabilitySlider.Init`: the template's durability).
        if let (true, Some((track, fill))) = (bar, look.durability.as_ref()) {
            let [w, h] = units;
            crate::screens::health_view::draw_sprite(thing, track, [(w - 15.0) * s, 26.0 * s, 12.0 * s, (h - 29.0) * s], s);
            let [now, most] = item.durability();
            let share = if most > 0.0 { (now / most).clamp(0.0, 1.0) } else { 1.0 };
            let (top, bottom) = (32.5, h - 9.0);
            let high = (bottom - top) * share;
            crate::screens::health_view::draw_sprite(thing, fill, [(w - 10.0) * s, (bottom - high) * s, 2.0 * s, high * s], s);
        }
        // The weapon's mastering (`SlotItemView.UpdateInfo`): its slider
        // at the view's left, filled to the points into the level (the
        // slider's value the share times 100, cut to a whole number: 0 for
        // a family not begun), and under it the mark at its own size with
        // the level, the game's level and one, written with a nought
        // before it, at its right; as the view's layout lays them out
        // (`MasteringLook`).
        if let (true, Some(mastering)) = (is("Weapon") && !examining && !special, look.mastering.as_ref()) {
            let [_, h] = units;
            let shown = carried.mastery.shown(&item.kind.template);
            if let Some(slider) = mastering.slider.as_ref() {
                let [x, y, w, high] = slider.track_at.at(h);
                crate::screens::health_view::draw_sprite(thing, &slider.track, [x * s, y * s, w * s, high * s], s);
                let share = slider.range.share((shown.share * 100.0).floor());
                let [x, y, w, high] = slider.fill_at(h, share);
                crate::screens::health_view::draw_sprite(thing, &slider.fill, [x * s, y * s, w * s, high * s], s);
            }
            let (mark, [left, bottom]) = (&mastering.mark, mastering.mark_at);
            let [wide, high] = mark.size;
            crate::screens::health_view::draw_sprite(thing, mark, [left * s, (h - bottom - high) * s, wide * s, high * s], s);
            let at = Node { position_type: PositionType::Absolute, left: Val::Px((left + wide) * s), bottom: Val::Px(bottom * s), ..default() };
            let level = format!("0{}", shown.level + 1);
            thing.spawn(at).with_children(|at| {
                at.spawn((Text::new(level), TextFont { font: mastering.font.clone(), font_size: mastering.size * s, ..default() }, TextColor(mastering.color), TextLayout::new_with_no_wrap()));
            });
        }
        // A part's marks at the bottom left: its class's ground and its
        // kind's mark over it (`ItemViewStats.SetItemIcon`).
        let class = crate::screens::health_view::MOD_CLASSES.iter().find(|(class, _)| is(class)).map(|(_, name)| *name);
        let kind = crate::screens::health_view::MOD_KINDS.iter().find(|(class, _)| is(class)).map(|(_, name)| *name);
        // A plate that may be taken out shows its armour class's mark in
        // their place (`ItemViewStats.SetStaticInfo`, `IsMoveAblePlate`: one
        // not in a slot, or of a class in a slot not locked); a built-in
        // insert, in its locked slot, the part's marks. `PROVISIONAL`: the
        // mark at the marks' place, its own object (`_armorClassIcon`) not
        // read.
        // (None of them for a thing not known: `ItemViewStats.SetItemIcon`,
        // `SetArmorPlateIcon`.)
        let (class, kind) = if unknown { (None, None) } else { (class, kind) };
        let movable_plate = !unknown
            && is("ArmorPlate")
            && match carried.lies(item.id) {
                Some((holder, Spot::Slot(at))) => item.kind.armor_class > 0 && !carried.find(holder).is_some_and(|holder| holder.kind.slots.get(at).is_some_and(|slot| slot.locked)),
                _ => true,
            };
        if movable_plate {
            if let Some(mark) = look.mods.get(&format!("plate_{}", item.kind.armor_class)) {
                let ([_, h], [mw, mh]) = (units, mark.size);
                crate::screens::health_view::draw_sprite(thing, mark, [3.0 * s, (h - 3.0 - mh) * s, mw * s, mh * s], s);
            }
        } else if let (Some(class), Some(kind)) = (class, kind) {
            let [_, h] = units;
            if let Some(ground) = look.mods.get(&format!("type_{class}")) {
                let [gw, gh] = ground.size;
                crate::screens::health_view::draw_sprite(thing, ground, [3.0 * s, (h - 3.0 - gh) * s, gw * s, gh * s], s);
                if let Some(mark) = look.mods.get(&format!("mod_{kind}")) {
                    let [mw, mh] = mark.size;
                    crate::screens::health_view::draw_sprite(thing, mark, [(3.0 + (gw - mw) / 2.0) * s, (h - 3.0 - (gh + mh) / 2.0) * s, mw * s, mh * s], s);
                }
            }
        }
        // Its border's edges over it, and its group's outlined icon at the
        // top of the bottom left column (above a part's marks and the
        // weapon's round, where those are in it).
        if let Some((group, wish)) = wish {
            for (index, edge) in &wish.edges {
                let [x, y, w, h] = panel.placed(*index, units).rect;
                crate::screens::health_view::draw_sprite(thing, edge, [x * s, y * s, w * s, h * s], s);
            }
            if let Some(icon) = wish.icons.get(group.name()) {
                let [cx, cy, _, ch] = panel.placed(wish.column, units).rect;
                let marks = class.is_some() && kind.is_some();
                let round = is("Weapon") && !item.kind.caliber.is_empty();
                let below = if marks { wish.mod_height } else { 0.0 } + if round { wish.inscription_height.unwrap_or(0.0) } else { 0.0 };
                let [w, h] = [panel.asks(wish.icon, 0).1, panel.asks(wish.icon, 1).1];
                crate::screens::health_view::draw_sprite(thing, icon, [cx * s, (cy + ch - below - h) * s, w * s, h * s], s);
            }
        }
        // A magazine worked: the game's load component over its view
        // (`ItemViewAnimation.StartLoading`), turned each frame
        // (`turn_spinners`).
        if let (Some(spin), true) = (look.spin.as_ref(), carried.magazine_work.as_ref().is_some_and(|work| work.process.magazine == item.id)) {
            let pieces = [(SpinRole::Back, &spin.back), (SpinRole::Fill, &spin.fill), (SpinRole::Loader, &spin.loader), (SpinRole::Stop, &spin.stop)];
            for (role, piece) in pieces {
                let Some((index, picture)) = piece else { continue };
                let [x, y, w, h] = spin.rect(*index, picture, units);
                let node = placed(x * s, y * s, w * s, h * s);
                match role {
                    SpinRole::Fill => {
                        thing.spawn((node, SpinPiece(role), SpinFill(picture.image.clone(), picture.cut)));
                    }
                    _ => {
                        let shown = if role == SpinRole::Stop { Visibility::Hidden } else { Visibility::Inherited };
                        thing.spawn((node, SpinPiece(role), ImageNode { image: picture.image.clone(), rect: Some(picture.cut), flip_y: true, ..default() }, shown));
                    }
                }
            }
        }
        // Being examined: the game's examination view over the thing's
        // (`ItemViewAnimation.StartExamination`): the bar's ground, its fill
        // by the share done, the loader, the percent done (each frame:
        // `turn_examination`).
        if let (Some(examine), true) = (look.examine.as_ref(), examining) {
            if let Some((index, back)) = &examine.back {
                let [x, y, w, h] = examine.panel.placed(*index, units).rect;
                crate::screens::health_view::draw_sprite(thing, back, [x * s, y * s, w * s, h * s], s);
            }
            if let Some((index, fill)) = &examine.fill {
                let [x, y, w, h] = examine.panel.placed(*index, units).rect;
                let image = ImageNode { image: fill.image.clone(), color: fill.color, rect: Some(fill.cut), flip_y: true, ..default() };
                thing.spawn((placed(x * s, y * s, 0.0, h * s), image, ExamineFill(Some(w * s))));
            }
            // The loader, turned about its middle (`turn_examination`).
            if let Some((index, loader)) = &examine.loader {
                let [x, y, w, h] = examine.panel.placed(*index, units).rect;
                let image = ImageNode { image: loader.image.clone(), color: loader.color, rect: Some(loader.cut), flip_y: true, ..default() };
                thing.spawn((placed(x * s, y * s, w * s, h * s), crate::screens::health_view::EXACT, image, UiTransform::IDENTITY, ExamineLoader));
            }
            if let Some((index, size, color, font)) = &examine.label {
                let [x, y, w, h] = examine.panel.placed(*index, units).rect;
                let frame = Node { justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..placed(x * s, y * s, w * s, h * s) };
                thing.spawn(frame).with_children(|frame| {
                    frame.spawn((Text::new("0"), TextFont { font: font.clone(), font_size: size * s, ..default() }, TextColor(*color), TextLayout::new_with_no_wrap(), ExamineLabel));
                });
            }
        }
        // The key it is bound to, or of the slot it is in where that slot
        // has a key of its own.
        let worn_in = carried.equipment.item.slots.iter().zip(&carried.equipment.item.kind.slots).find(|(part, _)| part.as_ref().is_some_and(|part| part.id == item.id)).map(|(_, slot)| slot.name.as_str());
        let slot_key = worn_in.and_then(|slot| carried.slot_keys.iter().find(|(name, _)| *name == slot)).map(|(_, words)| words.as_str()).filter(|words| !words.is_empty());
        let key = carried.bound.key_of(item.id).map(|key| QUICK_KEYS[key].1).or(slot_key);
        if let (Some(key), Some(bind)) = (key, look.bind.as_ref()) {
            let [w, h] = bind.size;
            let label = ImageNode { image: bind.image.clone(), rect: Some(bind.cut), flip_y: true, ..default() };
            let frame = Node { justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..placed(0.0, 0.0, w * s, h * s) };
            let font = TextFont { font: bind.font.clone(), font_size: bind.text_size * s, ..default() };
            thing.spawn((frame, label)).with_children(|label| {
                label.spawn((Text::new(key), font, TextColor(bind.color), TextLayout::new_with_no_wrap()));
            });
        }
    });
    if let Some(child) = ground_node {
        view.insert(GroundApart(child));
    }
}
