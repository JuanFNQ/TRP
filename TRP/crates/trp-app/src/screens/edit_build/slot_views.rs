//! The slots' lines, bones, views and dropdowns, in the game's look and the app's.

use super::*;

/// A slot's line from its bone to its view (`LineRenderer`): a node as long
/// as the line, laid along it, at least a pixel thick and not rounded to
/// the screen's pixels (rounded, a line thinner than a pixel came out none
/// at some places).
pub(super) fn line_place(from: [f32; 2], to: [f32; 2], s: f32) -> (Node, UiTransform) {
    let (from, to) = (Vec2::from(from), Vec2::from(to));
    let length = from.distance(to);
    let middle = (from + to) / 2.0;
    let thick = (LINE * s).max(1.0);
    let node = Node { position_type: PositionType::Absolute, left: Val::Px((middle.x - length / 2.0) * s), top: Val::Px(middle.y * s - thick / 2.0), width: Val::Px(length * s), height: Val::Px(thick), ..default() };
    let angle = (to.y - from.y).atan2(to.x - from.x);
    (node, UiTransform { rotation: Rot2::radians(angle), ..UiTransform::IDENTITY })
}

/// The marks of what a shown slot takes, by its item's template and its
/// place, worked out once.
pub(super) fn slot_marks(game: &crate::GameData, build: &Item, slot: &BuildSlot, filters: &mut HashMap<(String, usize), Vec<&'static str>>) -> Vec<&'static str> {
    let Some(owner) = build.find(slot.owner) else { return Vec::new() };
    let key = (owner.kind.template.clone(), slot.index);
    if let Some(marks) = filters.get(&key) {
        return marks.clone();
    }
    let marks = owner.kind.slots.get(slot.index).map(|holder| filter_marks(&game.db, holder)).unwrap_or_default();
    filters.insert(key, marks.clone());
    marks
}

/// The game's caption and the weapon's name under it (`UpdateNameLabel`:
/// the item's name in the install's words).
#[allow(clippy::too_many_arguments)]
pub(super) fn statics_under(layer: &mut ChildSpawnerCommands, look: &HealthLook, eb: &EditBuildLook, prepared: &Prepared, statics: &Statics, said: &Words, s: f32, wide: f32) {
    let mut words = prepared.words.clone();
    if let Some(sub) = prepared.sub_caption {
        words.insert(sub, statics.name.clone());
    }
    if let Some(&caption) = prepared.caption.iter().find(|&&at| Some(at) != prepared.sub_caption).filter(|_| statics.gear) {
        words.insert(caption, ITEM_OVERLOOK.to_owned());
    }
    let marks = Marks { within: &prepared.caption, words: Some(&words), ..Default::default() };
    draw_nodes(layer, look, &prepared.screen, &eb.screen, EB_SCREEN, marks, said, Frame::Is([wide, HIGH]), [0.0, 0.0], s);
}

/// The rest of the game's screen: the panel of a build ready to assemble
/// (`SetItemReadyToAssembleActive`: a build not yet the weapon's, its
/// parts all had, which here they always are), the build's name, the
/// buttons (the saved builds' not built: dimmed as the game dims what does
/// not work, `SetUnlockStatus`; ASSEMBLE so while there is nothing to
/// build), the toggles (ONLY AVAILABLE dimmed: every part is had here), the
/// back button and the loader; and what takes their clicks.
#[allow(clippy::too_many_arguments)]
pub(super) fn statics_over(layer: &mut ChildSpawnerCommands, look: &HealthLook, eb: &EditBuildLook, prepared: &Prepared, statics: &Statics, said: &Words, s: f32, wide: f32) {
    const DIM: f32 = 0.3;
    let mut set: HashMap<usize, bool> = HashMap::new();
    for (at, mark) in prepared.marks.iter().enumerate() {
        if let Some(mark) = mark {
            set.insert(*mark, at < 3 && statics.toggles[at]);
        }
    }
    if let Some(ready) = prepared.ready {
        set.insert(ready, statics.ready);
    }
    if let Some(loader) = prepared.loader {
        set.insert(loader, false);
    }
    let mut groups: HashMap<usize, f32> = prepared.buttons.iter().map(|&button| (button, DIM)).collect();
    groups.extend(prepared.save.map(|save| (save, DIM)));
    groups.extend(prepared.assemble.map(|assemble| (assemble, if statics.ready { 1.0 } else { DIM })));
    groups.extend(prepared.toggle[3].map(|only| (only, DIM)));
    let mut skip = prepared.caption.clone();
    skip.extend(prepared.preview_panel);
    let marks = Marks { skip: &skip, set: Some(&set), groups: Some(&groups), words: Some(&prepared.words), ..Default::default() };
    draw_nodes(layer, look, &prepared.screen, &eb.screen, EB_SCREEN, marks, said, Frame::Is([wide, HIGH]), [0.0, 0.0], s);
    // What takes the clicks, over what is drawn.
    let rect = |index: usize| prepared.screen.rect(index, [wide, HIGH]);
    for (at, toggle) in prepared.toggle.iter().take(3).enumerate() {
        if let Some(toggle) = toggle {
            layer.spawn((Button, Press::Toggle(at), rect_node(rect(*toggle), s)));
        }
    }
    if statics.ready {
        for button in [prepared.assemble_button, prepared.ready_button].into_iter().flatten() {
            layer.spawn((Button, Press::Assemble, rect_node(rect(button), s)));
        }
    }
    if let Some(back) = prepared.back {
        layer.spawn((Button, Press::Back, rect_node(rect(back), s)));
    }
}

/// A slot's bone as the game draws it (`ModdingScreenSlotView`): its icon
/// by its class and whether it holds a part, and a vital or gear slot's
/// name beside it, black on red where a vital part is missing.
#[allow(clippy::too_many_arguments)]
pub(super) fn bone_game(bone: &mut ChildSpawnerCommands, look: &HealthLook, eb: &EditBuildLook, prepared: &Prepared, slot: &BuildSlot, texts: &HashMap<String, String>, said: &Words, s: f32) {
    let Some(icon) = prepared.bone_icon else { return };
    let named = matches!(slot.class, ModClass::Vital | ModClass::Gear);
    let mut panel = eb.slot.clone();
    let mut words = HashMap::new();
    if let (Some(name), true) = (prepared.bone_name, named) {
        words.insert(name, slot_said(texts, slot));
    }
    look.ask_by_content(&mut panel, &eb.slot_drawn, &words);
    let mut set = HashMap::new();
    set.extend(prepared.bone_panel.map(|panel| (panel, named)));
    let mut sprites = HashMap::new();
    if let Some(picture) = eb.bones.get(bone_icon(slot)) {
        sprites.insert(icon, *picture);
    }
    let mut colors = HashMap::new();
    colors.extend(prepared.bone_back.map(|back| (back, if slot.missing { MISSING_GROUND } else { Color::BLACK })));
    colors.extend(prepared.bone_name.map(|name| (name, if slot.missing { Color::BLACK } else { Color::WHITE })));
    let within = [icon];
    let marks = Marks { within: &within, set: Some(&set), sprites: Some(&sprites), colors: Some(&colors), words: Some(&words), ..Default::default() };
    draw_nodes(bone, look, &panel, &eb.slot_drawn, EB_SLOT, marks, said, Frame::Is([0.0, 0.0]), [0.0, 0.0], s);
}

/// A slot's view as the game draws it, about its place on the ellipse (its
/// pivot, the middle of its foot): its circle, the marks of what it takes
/// over it, its dropdown's button while the mouse is on it or its dropdown
/// is open (open or shut, `SetMenuOpen`), and the part in it as a part's
/// view (`CreateModdingItemView`, a part's cells, from its lower left) or
/// the empty view; what the mouse is found on.
#[allow(clippy::too_many_arguments)]
pub(super) fn view_game(view: &mut ChildSpawnerCommands, look: &HealthLook, eb: &EditBuildLook, prepared: &Prepared, carried: &Carried, icons: &mut crate::screens::icons::Icons, shown: Option<&Item>, slot: &BuildSlot, boxed: BoxShown, marks: &[&'static str], said: &Words, s: f32) {
    let Some(item) = prepared.menu_item else { return };
    let [x, y, w, h] = prepared.item_rect;
    let id = (slot.owner, slot.index);
    view.spawn((SlotHover(id), Button, RelativeCursorPosition::default(), rect_node([x, y, w, h], s)));
    // The prefab's own pieces, about the pivot.
    let pivot_in = |panel: &ScenePanel| {
        let rect = panel.rect(item, [0.0, 0.0]);
        let pivot = panel.nodes[item].layout.pivot;
        [-(rect[0] + pivot[0] * rect[2]), -(rect[1] + (1.0 - pivot[1]) * rect[3])]
    };
    let origin = pivot_in(&eb.slot);
    // (What draws nothing is not made: the dropdown's button while it is
    // hidden, the empty text a debug build of the game writes to; the fade
    // of a locked slot is drawn after the part, below.)
    let mut set = HashMap::new();
    set.extend(prepared.slot_button.map(|button| (button, true)));
    let mut skip: Vec<usize> = [prepared.slot_fade, prepared.slot_debug].into_iter().flatten().collect();
    skip.extend(prepared.slot_button.filter(|_| !boxed.button));
    let mut sprites = HashMap::new();
    if let (Some(button), Some(picture)) = (prepared.slot_button, eb.dropdown[if boxed.open { 0 } else { 1 }]) {
        sprites.insert(button, picture);
    }
    let within = [item];
    let pieces = Marks { within: &within, skip: &skip, set: Some(&set), sprites: Some(&sprites), ..Default::default() };
    draw_nodes(view, look, &eb.slot, &eb.slot_drawn, EB_SLOT, pieces, said, Frame::Is([0.0, 0.0]), origin, s);
    // The marks of what it takes, in a row over its top left
    // (`_filtersContainer`: from its left, their feet two over its top).
    let mut left = x;
    for kind in marks {
        let Some(sprite) = look.mod_mark(kind).and_then(|picture| look.sprite_of(picture, Color::WHITE)) else { continue };
        let [mw, mh] = sprite.size;
        crate::screens::health_view::draw_sprite(view, &sprite, [left * s, (y - 2.0 - mh) * s, mw * s, mh * s], s);
        left += mw;
    }
    // Its dropdown's button's click.
    if let (Some(button), true) = (prepared.slot_button, boxed.button) {
        let rect = eb.slot.rect(button, [0.0, 0.0]);
        view.spawn((Button, Press::Open(id), rect_node([rect[0] + origin[0], rect[1] + origin[1], rect[2], rect[3]], s)));
    }
    // The part, or the empty view: a part of any cells in a view of the
    // part view's own size (`ItemViewContainer`, a row that sizes it by
    // its `LayoutElement`), as the dropdown's are.
    match slot.part.and_then(|part| shown?.find(part)) {
        Some(part) => {
            let size = eb.part_side();
            want_picture(carried, icons, part);
            let node = rect_node([x, y + h - size[1], size[0], size[1]], s);
            match eb.part_look.as_ref() {
                Some(part_look) => crate::screens::inventory_view::game_item_view(view, carried, part, node.clone(), Vec2::new(size[0], size[1]) * s, false, VIEW_LAYER, s, part_look),
                None => crate::screens::inventory_view::item_view(view, carried, part, node.clone(), false, Vec2::new(size[0], size[1]) * s, false, VIEW_LAYER, s),
            }
            let mut part_view = view.spawn((PartView(part.clone()), RelativeCursorPosition::default(), node));
            if slot.locked {
                part_view.insert(PartLocked);
            }
        }
        None => {
            let side = eb.empty.nodes.first().map_or([64.0, 64.0], |top| top.layout.size_delta);
            let marks = Marks { words: Some(&prepared.empty_words), ..Default::default() };
            draw_nodes(view, look, &eb.empty, &eb.empty_drawn, EB_EMPTY, marks, said, Frame::Is(side), [x, y + h - side[1]], s);
        }
    }
    // The fade over a locked slot's part (`_lockedFade`, on with
    // `Slot.Locked`: the prefab's black at 0.4, its square over the item's
    // container, a sibling after it), the mouse passing through to the part.
    if let (Some(fade), true) = (prepared.slot_fade, slot.locked) {
        let rect = eb.slot.rect(fade, [0.0, 0.0]);
        view.spawn((rect_node([rect[0] + origin[0], rect[1] + origin[1], rect[2], rect[3]], s), BackgroundColor(Color::srgba(0.0, 0.0, 0.0, LOCKED_FADE)), bevy::ui::FocusPolicy::Pass));
    }
}

/// The locked slot's fade's alpha (`modding_screen_slot`'s `LockedFade`
/// image, (0, 0, 0, 0.4)).
const LOCKED_FADE: f32 = 0.4;

/// A part's picture asked for where it has none yet.
fn want_picture(carried: &Carried, icons: &mut crate::screens::icons::Icons, item: &Item) {
    let key = crate::screens::icons::picture_key(item);
    if !carried.pictures.contains_key(&key) {
        icons.want(key, item.cells());
    }
}

/// The game's dropdown of a slot (`DropDownMenu`): its top left at the
/// slot's view's menu anchor, kept on the screen and off its foot
/// (`CorrectPositionResolution`, rounded); the empty view over the parts'
/// views in a grid as square as their count (`GridLayoutGroup`,
/// `Flexible`: as many columns as the square root, rounded up), each in a
/// cell of its own; a click on one puts it in the slot.
#[allow(clippy::too_many_arguments)]
pub(super) fn dropdown_game(layer: &mut ChildSpawnerCommands, look: &HealthLook, eb: &EditBuildLook, prepared: &Prepared, carried: &Carried, icons: &mut crate::screens::icons::Icons, id: SlotId, offered: &[(Arc<Kind>, Option<String>)], view_at: [f32; 2], said: &Words, s: f32, wide: f32) {
    let Some(menu) = prepared.dropdown else { return };
    // Clicked anywhere else, it shuts (the app's).
    layer.spawn((Button, Press::Shut, self::layer()));
    let count = offered.len();
    let columns = (count as f32).sqrt().ceil() as usize;
    let rows = if columns == 0 { 0 } else { count.div_ceil(columns) };
    let span = |cells: usize| if cells == 0 { 0.0 } else { cells as f32 * GRID_CELL + (cells - 1) as f32 * GRID_SPACING };
    let mut panel = prepared.screen.clone();
    let side = eb.empty.nodes.first().map_or([64.0, 64.0], |top| top.layout.size_delta);
    let element = |preferred: [f32; 2]| trp_data::interface::screen::Element { ignore: false, min: [-1.0, -1.0], preferred, flexible: [-1.0, -1.0] };
    if let Some(holder) = prepared.none_holder {
        panel.nodes[holder].element = Some(element(side));
    }
    if let Some(grid) = prepared.grid {
        panel.nodes[grid].element = Some(element([span(columns), span(rows)]));
    }
    let size = [wide, HIGH];
    let [mx, my, mw, mh] = panel.rect(menu, size);
    // Its pivot (its top left) at the anchor, then kept on the screen.
    let mut at = [view_at[0] + prepared.menu_anchor[0], view_at[1] + prepared.menu_anchor[1]];
    let (left, right, foot, top) = (at[0], wide - (at[0] + mw), HIGH - (at[1] + mh) - MENU_FOOT, at[1]);
    if left < 0.0 {
        at[0] -= left;
    } else if right < 0.0 {
        at[0] += right;
    }
    if foot < 0.0 {
        at[1] += foot;
    } else if top < 0.0 {
        at[1] -= top;
    }
    at = [at[0].round(), at[1].round()];
    let origin = [at[0] - mx, at[1] - my];
    let within = [menu];
    let set: HashMap<usize, bool> = [(menu, true)].into_iter().collect();
    let marks = Marks { within: &within, set: Some(&set), ..Default::default() };
    draw_nodes(layer, look, &panel, &eb.screen, EB_SCREEN, marks, said, Frame::Is(size), origin, s);
    // The empty view, which empties the slot.
    if let Some(holder) = prepared.none_holder {
        let [hx, hy, _, _] = panel.rect(holder, size);
        let place = [hx + origin[0], hy + origin[1]];
        layer.spawn((Button, Press::Pick(id, None), rect_node([place[0], place[1], side[0], side[1]], s))).with_children(|empty| {
            let marks = Marks { words: Some(&prepared.empty_words), ..Default::default() };
            draw_nodes(empty, look, &eb.empty, &eb.empty_drawn, EB_EMPTY, marks, said, Frame::Is(side), [0.0, 0.0], s);
        });
    }
    // The parts.
    let Some(grid) = prepared.grid else { return };
    let [gx, gy, _, _] = panel.rect(grid, size);
    for (n, (kind, refused)) in offered.iter().enumerate() {
        let (column, row) = (n % columns.max(1), n / columns.max(1));
        let cell = [gx + origin[0] + column as f32 * (GRID_CELL + GRID_SPACING), gy + origin[1] + row as f32 * (GRID_CELL + GRID_SPACING), GRID_CELL, GRID_CELL];
        let item = Item::new(kind.clone());
        want_picture(carried, icons, &item);
        let cell_size = Vec2::splat(GRID_CELL * s);
        let inside = Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), width: Val::Px(cell_size.x), height: Val::Px(cell_size.y), ..default() };
        let mut view = layer.spawn((Button, Press::Pick(id, Some(kind.clone())), PartView(item.clone()), RelativeCursorPosition::default(), rect_node(cell, s)));
        view.with_children(|cell| {
            match eb.part_look.as_ref() {
                Some(part_look) => crate::screens::inventory_view::game_item_view(cell, carried, &item, inside.clone(), cell_size, false, VIEW_LAYER, s, part_look),
                None => crate::screens::inventory_view::item_view(cell, carried, &item, inside.clone(), false, cell_size, false, VIEW_LAYER, s),
            }
            if refused.is_some() {
                refused_marks(cell, inside.clone());
            }
        });
        if let Some(reason) = refused {
            view.insert(PartRefused(reason.clone()));
        }
    }
}

/// Over a part's view the slot would refuse: the view dimmed and its red
/// frame (`REFUSED_DIM`, `REFUSED_BORDER`).
fn refused_marks(cell: &mut ChildSpawnerCommands, inside: Node) {
    let node = Node { border: UiRect::all(Val::Px(1.0)), ..inside };
    cell.spawn((node, BackgroundColor(Color::BLACK.with_alpha(REFUSED_DIM)), BorderColor::all(REFUSED_BORDER.with_alpha(1.0 - REFUSED_DIM)), bevy::ui::FocusPolicy::Pass));
}

/// The app's look of the screen's own objects (without `TARKOV UI`): the
/// caption, the weapon's name, the toggles, the buttons' column (ASSEMBLE
/// the one that works; the saved builds' dim), BACK, and the loader.
pub(super) fn statics_app(layer: &mut ChildSpawnerCommands, statics: &Statics, s: f32, wide: f32) {
    let px = |v: f32| Val::Px(v * s);
    let at = |x: f32, y: f32| Node { position_type: PositionType::Absolute, left: px(x), top: px(y), ..default() };
    let centred = |top: f32| Node { position_type: PositionType::Absolute, left: px(0.0), width: Val::Percent(100.0), top: px(top), justify_content: JustifyContent::Center, column_gap: px(20.0), ..default() };
    layer.spawn(centred(14.0)).with_children(|row| {
        row.spawn(words(if statics.gear { ITEM_OVERLOOK } else { "WEAPON BUILDS" }, 41.0 * s, INK));
    });
    layer.spawn(centred(55.0)).with_children(|row| {
        row.spawn(words(statics.name.clone(), 18.0 * s, INK));
    });
    let labels = ["Vital parts", "Functional mods", "Equipment parts", "Use only available parts"];
    layer.spawn(centred(98.0)).with_children(|row| {
        for (index, label) in labels.iter().enumerate() {
            let (on, live) = if index < 3 { (statics.toggles[index], true) } else { (false, false) };
            let ink = if live { INK } else { INK.with_alpha(0.3) };
            let mut toggle = row.spawn(Node { column_gap: px(5.0), align_items: AlignItems::Center, ..default() });
            if live {
                toggle.insert((Button, Press::Toggle(index)));
            }
            toggle.with_children(|toggle| {
                toggle.spawn((Node { width: px(16.0), height: px(16.0), border: UiRect::all(px(2.0)), ..default() }, BorderColor::all(ink), BackgroundColor(if on { ink } else { Color::NONE })));
                toggle.spawn(words(*label, 18.0 * s, ink));
            });
        }
    });
    let column = ["NEW BUILD", "OPEN ...", "SAVE AS ...", "SELECT WEAPON", "FIND PARTS", "PUBLISH ...", "DELETE BUILD"];
    let mut y = 66.0;
    for (index, label) in column.iter().enumerate() {
        if index == 6 {
            y += 5.0 + 3.0;
        }
        let ground = if index == 6 { Color::srgb(0.231, 0.0, 0.0) } else { Color::srgb(0.169, 0.176, 0.180) };
        layer.spawn((Node { width: px(150.0), height: px(18.0), padding: UiRect::left(px(35.0)), align_items: AlignItems::Center, ..at(45.0, y) }, BackgroundColor(ground.with_alpha(0.3)))).with_children(|button| {
            button.spawn(words(*label, 11.0 * s, Color::WHITE.with_alpha(0.3)));
        });
        y += 18.0 + 3.0;
    }
    y += 15.0;
    let ink = if statics.ready { Color::WHITE } else { Color::WHITE.with_alpha(0.3) };
    let mut assemble = layer.spawn((Node { width: px(51.0), height: px(50.0), justify_content: JustifyContent::Center, align_items: AlignItems::Center, border: UiRect::all(px(1.0)), ..at(45.0 + 51.0 + 20.0, y) }, BorderColor::all(ink), BackgroundColor(Color::srgb(0.2, 0.2, 0.2))));
    if statics.ready {
        assemble.insert((Button, Press::Assemble));
    }
    assemble.with_children(|button| {
        button.spawn(words("ASSEMBLE", 10.0 * s, ink));
    });
    layer.spawn((Button, Press::Back, Node { width: px(250.0), height: px(55.6), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..at(wide - 275.0, HIGH - 50.0 - 55.6) })).with_children(|back| {
        back.spawn(words("BACK", 36.0 * s, Color::srgb(0.906, 0.898, 0.831)));
    });
    layer.spawn((
        AppLoader,
        Node { width: px(88.0), height: px(88.0), border: UiRect::all(px(3.0)), border_radius: BorderRadius::all(px(44.0)), ..at(wide / 2.0 - 44.0, HIGH / 2.0 - 44.0) },
        BorderColor { top: INK, left: INK.with_alpha(0.0), right: INK.with_alpha(0.5), bottom: INK.with_alpha(0.25) },
        UiTransform::IDENTITY,
        Visibility::Hidden,
    ));
}

/// The app's look of a slot's bone: a square of its class's colour, full
/// where it holds a part, and a vital or gear slot's name beside it.
pub(super) fn bone_app(bone: &mut ChildSpawnerCommands, slot: &BuildSlot, texts: &HashMap<String, String>, s: f32) {
    let px = |v: f32| Val::Px(v * s);
    let color = class_color(slot);
    bone.spawn((Node { position_type: PositionType::Absolute, left: px(-4.5), top: px(-4.5), width: px(9.0), height: px(9.0), border: UiRect::all(px(2.0)), ..default() }, BorderColor::all(color), BackgroundColor(if slot.part.is_some() { color } else { Color::NONE })));
    if matches!(slot.class, ModClass::Vital | ModClass::Gear) {
        let (ground, ink) = if slot.missing { (MISSING_GROUND, Color::BLACK) } else { (Color::BLACK, Color::WHITE) };
        bone.spawn((Node { position_type: PositionType::Absolute, left: px(12.5), top: px(-8.0), padding: UiRect::axes(px(2.0), px(1.0)), ..default() }, BackgroundColor(ground))).with_children(|plate| {
            plate.spawn(words(slot_said(texts, slot), 11.0 * s, ink));
        });
    }
}

/// The app's look of a slot's view: a box with the part's view in it, or
/// EMPTY; a click on it opens its dropdown, but a locked slot's, faded.
pub(super) fn view_app(view: &mut ChildSpawnerCommands, carried: &Carried, icons: &mut crate::screens::icons::Icons, shown: Option<&Item>, slot: &BuildSlot, s: f32) {
    let id = (slot.owner, slot.index);
    let color = class_color(slot);
    let rect = [-32.0, -64.0, 64.0, 64.0];
    let mut boxed = view.spawn((SlotHover(id), Button, RelativeCursorPosition::default(), Node { border: UiRect::all(Val::Px(s)), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..rect_node(rect, s) }, BorderColor::all(color), BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6))));
    if !slot.locked {
        boxed.insert(Press::Open(id));
    }
    boxed.with_children(|boxed| {
        match slot.part.and_then(|part| shown?.find(part)) {
            Some(part) => {
                want_picture(carried, icons, part);
                let size = Vec2::splat(62.0 * s);
                crate::screens::inventory_view::item_view(boxed, carried, part, Node { width: Val::Px(size.x), height: Val::Px(size.y), ..default() }, false, size, false, VIEW_LAYER, s);
                let mut part_view = boxed.spawn((PartView(part.clone()), RelativeCursorPosition::default(), Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() }));
                if slot.locked {
                    part_view.insert(PartLocked);
                }
            }
            None => {
                boxed.spawn(words("EMPTY", 9.0 * s, color));
            }
        }
        if slot.locked {
            boxed.spawn((Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() }, BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)), bevy::ui::FocusPolicy::Pass));
        }
    });
}

/// The app's look of a slot's dropdown: a list of the parts it takes, the
/// line that empties it first, scrolled by the wheel.
#[allow(clippy::too_many_arguments)]
pub(super) fn dropdown_app(layer: &mut ChildSpawnerCommands, carried: &Carried, icons: &mut crate::screens::icons::Icons, id: SlotId, offered: &[(Arc<Kind>, Option<String>)], view_at: [f32; 2], s: f32, wide: f32) {
    let px = |v: f32| Val::Px(v * s);
    let columns = 6usize;
    let width = columns as f32 * (CELL + 2.0) + 10.0;
    let left = (view_at[0] + 32.0 + 30.0).min(wide - width - 10.0);
    let top = (view_at[1] - 64.0).clamp(10.0, HIGH - 120.0);
    layer.spawn((Button, Press::Shut, self::layer()));
    layer
        .spawn((DropdownList, ScrollPosition(Vec2::ZERO), Node { position_type: PositionType::Absolute, left: px(left), top: px(top), width: px(width), max_height: px(HIGH - top - 120.0), flex_wrap: FlexWrap::Wrap, padding: UiRect::all(px(5.0)), row_gap: px(2.0), column_gap: px(2.0), overflow: Overflow::scroll_y(), ..default() }, BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.922))))
        .with_children(|list| {
            list.spawn((Button, Press::Pick(id, None), Node { width: px(CELL), height: px(CELL), border: UiRect::all(px(1.0)), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() }, BorderColor::all(INK.with_alpha(0.4)))).with_children(|none| {
                none.spawn(words("NONE", 10.0 * s, INK));
            });
            for (kind, refused) in offered {
                let item = Item::new(kind.clone());
                let [w, h] = item.cells();
                let size = Vec2::new(w as f32 * CELL + (w as f32 - 1.0) * 2.0, h as f32 * CELL + (h as f32 - 1.0) * 2.0) * s;
                want_picture(carried, icons, &item);
                let mut view = list.spawn((Button, Press::Pick(id, Some(kind.clone())), PartView(item.clone()), RelativeCursorPosition::default(), Node { width: Val::Px(size.x), height: Val::Px(size.y), ..default() }));
                view.with_children(|cell| {
                    crate::screens::inventory_view::item_view(cell, carried, &item, Node { width: Val::Px(size.x), height: Val::Px(size.y), ..default() }, false, size, false, VIEW_LAYER, s);
                    if refused.is_some() {
                        refused_marks(cell, Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), width: Val::Px(size.x), height: Val::Px(size.y), ..default() });
                    }
                });
                if let Some(reason) = refused {
                    view.insert(PartRefused(reason.clone()));
                }
            }
        });
}
