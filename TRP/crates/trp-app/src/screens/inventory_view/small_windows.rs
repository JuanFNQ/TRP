//! The repair and tag windows, the asking box and the stash's fills.

use super::*;

/// The repair window (`RepairWindow`): the game's own (`TARKOV UI`), its
/// kit's picture as its repairer's avatar, the thing's model in its
/// preview, a place over its caption to move it by, its cross, its slider,
/// its top's button and its REPAIR (`repair_view::spots`); else the app's
/// look of the same figures, in words, with its REPAIR.
#[allow(clippy::too_many_arguments)]
pub(super) fn repair_window_box(commands: &mut Commands, carried: &Carried, window: Win, index: usize, units: Units, look: &crate::screens::health_view::HealthLook, said: &crate::screens::health_view::Words, (game, previews): (&crate::GameData, &crate::screens::info_preview::InfoPreviews)) {
    let Some(item) = carried.find(window.item) else { return };
    let Some(repair) = carried.repairs.iter().find(|repair| repair.thing == window.item) else { return };
    let (layer, s) = (WINDOW_LAYER + index as u8, units.0);
    let model = previews.picture(window.item);
    let Some((shown, _)) = crate::screens::repair_view::repair_words(carried, item, repair, &game.db, said, model.is_none()) else { return };
    if let Some(built) = crate::screens::health_view::repair_built(look, &shown) {
        let [w, h] = built.size;
        commands
            .spawn((Overlay, WinRoot(index), Cover(layer), RelativeCursorPosition::default(), placed(window.at.x, window.at.y, w * s, h * s), window_z(index)))
            .with_children(|inside| {
                crate::screens::health_view::draw_repair_window(inside, look, said, &built, index, s);
                if let Some(kit) = carried.find(repair.kit) {
                    let [x, y, w, h] = built.avatar;
                    inside.spawn(placed(x * s, y * s, w * s, h * s)).with_children(|room| picture_in(room, carried, kit, within_line(Vec2::new(w * s, h * s)), false, None));
                }
                let [x, y, w, h] = built.preview;
                let room = placed(x * s, y * s, w * s, h * s);
                match model {
                    Some(image) => inside.spawn((crate::screens::info_preview::InfoPicture(window.item, layer), RelativeCursorPosition::default(), ImageNode::new(image), room)),
                    None => inside.spawn((crate::screens::info_preview::InfoPicture(window.item, layer), RelativeCursorPosition::default(), room)),
                };
                crate::screens::repair_view::spots(inside, &built, index, s);
                let [x, y, w, h] = built.caption;
                inside.spawn((WinCaption(index), RelativeCursorPosition::default(), placed(x * s, y * s, w * s, h * s)));
                let [x, y, w, h] = built.close;
                inside.spawn((WinClose(index), Button, placed(x * s, y * s, w * s, h * s)));
            });
        return;
    }
    // The app's own look: the same figures in words.
    let root = Node { position_type: PositionType::Absolute, left: Val::Px(window.at.x), top: Val::Px(window.at.y), flex_direction: FlexDirection::Column, border: UiRect::all(Val::Px(1.0)), ..default() };
    let plain = |said: &str| crate::screens::health_view::rich_spans(said).map_or_else(|| said.to_owned(), |spans| spans.into_iter().map(|(words, _)| words).collect::<String>());
    commands
        .spawn((Overlay, WinRoot(index), Cover(layer), RelativeCursorPosition::default(), root, BorderColor::all(LINE), BackgroundColor(PANEL), window_z(index)))
        .with_children(|inside| {
            let caption = Node { justify_content: JustifyContent::SpaceBetween, align_items: AlignItems::Center, column_gap: Val::Px(16.0), padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)), ..default() };
            inside.spawn((WinCaption(index), RelativeCursorPosition::default(), caption, BackgroundColor(LINE))).with_children(|caption| {
                caption.spawn(words(words_caption(&shown.caption), 11.0, WORDS));
                caption.spawn((WinClose(index), Button, Node { padding: UiRect::horizontal(Val::Px(6.0)), ..default() }, BackgroundColor(PANEL))).with_children(|close| {
                    close.spawn(words("X", 11.0, WORDS));
                });
            });
            let body = Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(4.0), padding: UiRect::all(Val::Px(6.0)), ..default() };
            inside.spawn(body).with_children(|body| {
                let lines = [shown.dropdown.clone(), plain(&shown.charge)].into_iter().chain(shown.lines.iter().map(|line| plain(line))).chain(shown.warnings.iter().map(|(_, said)| said.clone())).chain([plain(&shown.durability), shown.repair_dp.clone(), format!("{} / {}", shown.in_stash, shown.to_pay)]);
                for line in lines.filter(|line| !line.is_empty()) {
                    body.spawn(words(line, 12.0, WORDS));
                }
                let ground = if shown.interactable { FAINT } else { LINE };
                body.spawn((crate::screens::repair_view::RepairSpot::Press(index), RelativeCursorPosition::default(), Node { padding: UiRect::axes(Val::Px(10.0), Val::Px(4.0)), ..default() }, BackgroundColor(ground))).with_children(|button| {
                    button.spawn(words("REPAIR", 12.0, WORDS));
                });
            });
        });
}

/// The caption's words in the app's look: upper case, as its windows'.
fn words_caption(caption: &str) -> String {
    caption.to_uppercase()
}

/// Opens the window that edits a thing's tag (`ItemUiContext.EditTag`,
/// `OpenWindow`), with its name and colour as they stand, where the game
/// opens a window; or brings its own to the top.
pub(super) fn open_tag(carried: &mut Carried, id: Id, cursor: Vec2, screen: Option<(Vec2, f32)>) {
    let Some(tag) = carried.find(id).filter(|item| trp_sim::items::inventory::takes_tag(item)).map(|item| item.tag.clone()) else { return };
    let at = match (carried.tag_size, screen) {
        (Some(size), Some((screen, s))) => game_window_at(carried, size, screen, s),
        _ => cursor + Vec2::new(14.0, 14.0),
    };
    if !carried.tag_edits.iter().any(|edit| edit.item == id) {
        carried.tag_edits.push(TagEdit { item: id, field: crate::screens::text_field::TextField::saying(tag.name), color: tag.color });
    }
    carried.open_window(id, WinKind::Tag, at);
}

/// A place of the window that edits a tag, by the window's number: its
/// field, a colour cell, its button.
#[derive(Component, Clone, Copy, PartialEq, Debug)]
pub(super) struct TagPlace(usize, TagSpot);

#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum TagSpot {
    Field,
    Color(u32),
    Save,
}

/// The window that edits a tag: the game's own (`TARKOV UI`), else the
/// app's look of the same: its caption and cross, the seven colours with
/// the chosen one's border, the count of letters, the field (its name or
/// its placeholder; while it has the keys a bar where its caret stands,
/// or what is selected between brackets) and its button.
#[allow(clippy::too_many_arguments)]
pub(super) fn tag_window_box(commands: &mut Commands, carried: &Carried, window: Win, index: usize, units: Units, look: &crate::screens::health_view::HealthLook, said: &crate::screens::health_view::Words) {
    let Some(edit) = carried.tag_edits.iter().find(|edit| edit.item == window.item) else { return };
    let (layer, s) = (WINDOW_LAYER + index as u8, units.0);
    if let Some(places) = crate::screens::health_view::tag_places(look) {
        let [w, h] = places.size;
        commands
            .spawn((Overlay, WinRoot(index), Cover(layer), RelativeCursorPosition::default(), placed(window.at.x, window.at.y, w * s, h * s), window_z(index)))
            .with_children(|inside| {
                let words = crate::screens::health_view::TagWords { field: &edit.field, color: edit.color };
                crate::screens::health_view::draw_tag_window(inside, look, said, &words, index, s);
                let at = |[x, y, w, h]: [f32; 4]| placed(x * s, y * s, w * s, h * s);
                inside.spawn((WinCaption(index), RelativeCursorPosition::default(), at(places.caption)));
                inside.spawn((TagPlace(index, TagSpot::Field), Button, at(places.input)));
                for (n, cell) in places.cells.iter().enumerate() {
                    inside.spawn((TagPlace(index, TagSpot::Color(n as u32)), Button, at(*cell)));
                }
                inside.spawn((TagPlace(index, TagSpot::Save), Button, at(places.save)));
                inside.spawn((WinClose(index), Button, at(places.close)));
            });
        return;
    }
    let root = Node { position_type: PositionType::Absolute, left: Val::Px(window.at.x), top: Val::Px(window.at.y), flex_direction: FlexDirection::Column, border: UiRect::all(Val::Px(1.0)), ..default() };
    commands
        .spawn((Overlay, WinRoot(index), Cover(layer), RelativeCursorPosition::default(), root, BorderColor::all(LINE), BackgroundColor(PANEL), window_z(index)))
        .with_children(|inside| {
            let caption = Node { justify_content: JustifyContent::SpaceBetween, align_items: AlignItems::Center, column_gap: Val::Px(16.0), padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)), ..default() };
            inside.spawn((WinCaption(index), RelativeCursorPosition::default(), caption, BackgroundColor(LINE))).with_children(|caption| {
                caption.spawn(words(said.word("ADD/EDIT CONTAINER TAG", "ADD/EDIT CONTAINER TAG"), 11.0, WORDS));
                caption.spawn((WinClose(index), Button, Node { padding: UiRect::horizontal(Val::Px(6.0)), ..default() }, BackgroundColor(PANEL))).with_children(|close| {
                    close.spawn(words("X", 11.0, WORDS));
                });
            });
            let body = Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(6.0), padding: UiRect::all(Val::Px(8.0)), ..default() };
            inside.spawn(body).with_children(|body| {
                body.spawn(Node { column_gap: Val::Px(10.0), align_items: AlignItems::Center, ..default() }).with_children(|row| {
                    for n in 0..crate::screens::health_view::TAG_COLOURS.len() as u32 {
                        let chosen = n == edit.color;
                        let cell = Node { width: Val::Px(22.0), height: Val::Px(22.0), border: UiRect::all(Val::Px(if chosen { 2.0 } else { 0.0 })), ..default() };
                        row.spawn((TagPlace(index, TagSpot::Color(n)), Button, cell, BackgroundColor(crate::screens::health_view::tag_colour(n)), BorderColor::all(WORDS)));
                    }
                    row.spawn(words(format!("{}/{}", edit.field.letters(), carried.tag_limit), 14.0, FAINT));
                });
                let field = Node { width: Val::Px(320.0), height: Val::Px(30.0), align_items: AlignItems::Center, padding: UiRect::horizontal(Val::Px(6.0)), border: UiRect::all(Val::Px(1.0)), ..default() };
                let (name, focused) = (&edit.field.text, edit.field.focused);
                body.spawn((TagPlace(index, TagSpot::Field), Button, field, BorderColor::all(if focused { WORDS } else { LINE }))).with_children(|field| {
                    let letters: Vec<char> = name.chars().collect();
                    let part = |from: usize, to: usize| letters[from.min(letters.len())..to.min(letters.len())].iter().collect::<String>();
                    let shown = match (name.is_empty(), focused, edit.field.selection()) {
                        (true, _, _) => format!("{}{}", said.word("container tag", "container tag"), if focused { "|" } else { "" }),
                        (_, true, Some((from, to))) => format!("{}[{}]{}", part(0, from), part(from, to), part(to, letters.len())),
                        (_, true, None) => format!("{}|{}", part(0, edit.field.caret), part(edit.field.caret, letters.len())),
                        _ => name.clone(),
                    };
                    field.spawn(words(shown, 16.0, if name.is_empty() { FAINT } else { WORDS }));
                });
                body.spawn((TagPlace(index, TagSpot::Save), Button, Node { align_self: AlignSelf::Center, padding: UiRect::axes(Val::Px(24.0), Val::Px(4.0)), ..default() }, BackgroundColor(LINE))).with_children(|save| {
                    save.spawn(words(said.word("Save", "Save").to_uppercase(), 14.0, WORDS));
                });
            });
        });
}

/// The windows that edit a tag (`EditTagWindow`; inventory spec, "A tag"):
/// its field is the game's text field (`text_field`; "A text field"): a
/// press on it gives it the keys with all it says selected, a press on it
/// after that puts its caret, a drag selects, and a press anywhere else
/// takes the keys away; while it has them the game's bindings hear
/// nothing and the keys edit its name at its caret, by its own most
/// letters and its validation's letters, Enter letting the keys go. A
/// colour cell chooses its colour; the button saves the tag (an empty name
/// with colour 0) where it changed, and shuts the window
/// (`EditTagWindow.Save`). Escape and the cross shut it as any window
/// (`windows`). An edit goes with its window.
#[allow(clippy::too_many_arguments)]
pub(super) fn tag_windows(
    buttons: Res<ButtonInput<MouseButton>>,
    mut typed: MessageReader<bevy::input::keyboard::KeyboardInput>,
    controller: Res<Controller>,
    mut carried: ResMut<Carried>,
    places: Query<(&Interaction, &TagPlace)>,
    mut bindings: ResMut<Bindings>,
    (keys, clock, look, units, window): (Res<ButtonInput<KeyCode>>, Res<Time<Real>>, Res<crate::screens::health_view::HealthLook>, Res<Units>, Query<&Window, With<PrimaryWindow>>),
) {
    use crate::screens::text_field::{field_key, Rules, Spot};
    let open = |carried: &Carried, item: Id| carried.windows.iter().any(|window| window.kind == WinKind::Tag && window.item == item);
    if carried.tag_edits.iter().any(|edit| !open(&carried, edit.item)) {
        let carried = &mut *carried;
        let windows = carried.windows.clone();
        carried.tag_edits.retain(|edit| windows.iter().any(|window| window.kind == WinKind::Tag && window.item == edit.item));
    }
    let typing = controller.inventory_open && carried.tag_edits.iter().any(|edit| edit.field.focused);
    if bindings.typing != typing {
        bindings.typing = typing;
    }
    if carried.tag_edits.is_empty() {
        typed.clear();
        return;
    }
    let item_of = |carried: &Carried, window: usize| carried.windows.get(window).filter(|window| window.kind == WinKind::Tag).map(|window| window.item);
    // The field's own rules: its most letters, its validation's letters,
    // and all selected as it takes the keys (the install's field does:
    // `probe tagwindow`; so where it was not read).
    let rules = Rules { limit: carried.tag_limit as usize, keeps: tag_letter, select_all: look.tag_field.is_none_or(|field| field.select_all) };
    let (mods, now) = (crate::screens::text_field::mods(&keys), clock.elapsed_secs());
    // Where the mouse is on the screen, and in a thing's field's words
    // (the game's window: `tag_spot`; in the app's own look, the end).
    let cursor = window.single().ok().and_then(Window::cursor_position);
    let spot = |carried: &Carried, item: Id, said: &str| {
        let own = carried.windows.iter().find(|window| window.kind == WinKind::Tag && window.item == item).map(|window| window.at);
        let end = said.chars().count();
        let in_window = cursor.zip(own).map(|(cursor, own)| (cursor - own) / units.0.max(1e-3));
        in_window.and_then(|at| crate::screens::health_view::tag_spot(&look, said, at.into())).unwrap_or(Spot { gap: end, letter: end, word: None })
    };
    let cursor_px = cursor.map_or([0.0, 0.0], Into::into);
    // The button held from a press on a field: its selection's end follows
    // the mouse; let go, the field holds it no longer.
    if let Some(at) = carried.tag_edits.iter().position(|edit| edit.field.is_held()).filter(|_| !buttons.just_pressed(MouseButton::Left)) {
        let mut field = carried.tag_edits[at].field.clone();
        if buttons.pressed(MouseButton::Left) {
            field.drag(spot(&carried, carried.tag_edits[at].item, &field.text), cursor_px, DRAG_THRESHOLD);
        } else {
            field.release();
        }
        if field != carried.tag_edits[at].field {
            carried.tag_edits[at].field = field;
        }
    }
    // A press: on a field it takes the keys or puts its caret, anywhere
    // else none has the keys.
    if buttons.just_pressed(MouseButton::Left) {
        let pressed: Vec<TagPlace> = places.iter().filter(|(interaction, _)| **interaction == Interaction::Pressed).map(|(_, place)| *place).collect();
        let on = pressed.iter().find(|place| place.1 == TagSpot::Field).and_then(|place| item_of(&carried, place.0));
        for at in 0..carried.tag_edits.len() {
            let mut field = carried.tag_edits[at].field.clone();
            if Some(carried.tag_edits[at].item) == on {
                field.press(spot(&carried, carried.tag_edits[at].item, &field.text), cursor_px, mods, &rules, now);
            } else {
                field.blur();
            }
            if field != carried.tag_edits[at].field {
                carried.tag_edits[at].field = field;
            }
        }
        for place in pressed {
            let Some(item) = item_of(&carried, place.0) else { continue };
            match place.1 {
                TagSpot::Color(n) => {
                    if let Some(edit) = carried.tag_edits.iter_mut().find(|edit| edit.item == item && edit.color != n) {
                        edit.color = n;
                    }
                }
                TagSpot::Save => {
                    let Some((said, color)) = carried.tag_edits.iter().find(|edit| edit.item == item).map(|edit| (edit.field.text.clone(), edit.color)) else { continue };
                    let color = if said.is_empty() { 0 } else { color };
                    let carried = &mut *carried;
                    let changed = carried.find(item).is_some_and(|thing| thing.tag.name != said || thing.tag.color != color);
                    let name = carried.find(item).map(|thing| carried.name(thing)).unwrap_or_default();
                    if changed && (trp_sim::items::inventory::set_tag(&mut carried.equipment.item, item, &said, color) || trp_sim::items::inventory::set_tag(&mut carried.loot, item, &said, color)) {
                        carried.note = if said.is_empty() { format!("{name}: tag reset") } else { format!("{name}: tagged {said}") };
                    }
                    carried.windows.retain(|window| !(window.kind == WinKind::Tag && window.item == item));
                    carried.tag_edits.retain(|edit| edit.item != item);
                }
                TagSpot::Field => {}
            }
        }
    }
    // The keys, to the field that has them (`TextField::key`); a key
    // after Enter goes nowhere.
    let Some(at) = carried.tag_edits.iter().position(|edit| edit.field.focused) else {
        typed.clear();
        return;
    };
    let mut field = carried.tag_edits[at].field.clone();
    for key in typed.read().filter(|key| key.state.is_pressed()) {
        if let (true, Some(key)) = (field.focused, field_key(key, mods)) {
            field.key(&key, mods, &rules, now);
        }
    }
    if field != carried.tag_edits[at].field {
        carried.tag_edits[at].field = field;
    }
}

/// The window that asks how many of a stack (`SplitDialog`, with its
/// stepped slider): the figure, the slider with a stop at each of the
/// game's notches, a button to go through with it and one to drop it.
///
/// With `TARKOV UI`, the game's (`split_view`): its middle at the mouse,
/// kept on the screen, its corner on a whole pixel of the screen's
/// (`pixels` of them to one of the window's), over the screen darkened by
/// its ground, which takes every press that is not on the window; the
/// places of its parts kept on it for `ask`. Else the app's own.
pub(super) fn asking_box(commands: &mut Commands, asking: &Asking, window: Vec2, (s, pixels): (f32, f32), look: &crate::screens::health_view::HealthLook, said: &crate::screens::health_view::Words) {
    use crate::screens::split_view::{corner, draw_split_window, split_built, split_size, SplitShown, SplitTints};
    if let (true, Some(size)) = (look.tarkov_ui, split_size(look)) {
        let shown = SplitShown {
            title: asking.caption(said),
            int: matches!(asking.asked, Asked::Use { .. }),
            notches: asking.notches(),
            share: asking.shares().0,
            field: asking.field(),
            focused: asking.focused,
            selected: asking.selected,
            corner: corner(asking.at, size * s, window, pixels),
        };
        if let Some(built) = split_built(look, &shown, window, s) {
            let mut tints = SplitTints::default();
            let mut root = commands.spawn((Overlay, Cover(ASKING_LAYER), RelativeCursorPosition::default(), placed(0.0, 0.0, window.x, window.y), GlobalZIndex(30), AskPlaces(built.places)));
            root.with_children(|inside| tints = draw_split_window(inside, look, said, &built, &shown, s));
            root.insert(tints);
            return;
        }
    }
    const WIDTH: f32 = 260.0;
    let (share, gone) = asking.shares();
    let at = asking.at.min(window - Vec2::new(WIDTH + 8.0, 120.0)).max(Vec2::ZERO);
    let root = Node {
        position_type: PositionType::Absolute,
        left: Val::Px(at.x),
        top: Val::Px(at.y),
        width: Val::Px(WIDTH),
        flex_direction: FlexDirection::Column,
        row_gap: Val::Px(8.0),
        padding: UiRect::all(Val::Px(8.0)),
        border: UiRect::all(Val::Px(1.0)),
        ..default()
    };
    commands.spawn((Overlay, Cover(ASKING_LAYER), RelativeCursorPosition::default(), root, BorderColor::all(WORDS), BackgroundColor(PANEL), GlobalZIndex(30))).with_children(|inside| {
        inside.spawn(Node { justify_content: JustifyContent::SpaceBetween, ..default() }).with_children(|top| {
            top.spawn(words(asking.caption(said), 12.0, WORDS));
            top.spawn((AskClose, Button, Node { padding: UiRect::horizontal(Val::Px(6.0)), ..default() }, BackgroundColor(LINE))).with_children(|close| {
                close.spawn(words("X", 11.0, WORDS));
            });
        });
        inside.spawn(words(format!("{}   of {}", asking.field(), asking.most), 16.0, WORDS));
        let bar = Node { width: Val::Percent(100.0), height: Val::Px(14.0), ..default() };
        inside.spawn((AskBar, RelativeCursorPosition::default(), bar, BackgroundColor(BACK))).with_children(|bar| {
            // What is gone of food or drink lies before the knob's first
            // place, darker (the game's notches not available).
            if gone > 0.0 {
                bar.spawn((Node { width: Val::Percent(gone * 100.0), height: Val::Percent(100.0), ..default() }, BackgroundColor(LINE)));
            }
            bar.spawn((Node { width: Val::Percent((share - gone).max(0.0) * 100.0), height: Val::Percent(100.0), ..default() }, BackgroundColor(FAINT)));
        });
        inside.spawn((AskAccept, Button, Node { padding: UiRect::axes(Val::Px(14.0), Val::Px(5.0)), align_self: AlignSelf::Center, ..default() }, BackgroundColor(LINE))).with_children(|accept| {
            accept.spawn(words("ACCEPT", 12.0, WORDS));
        });
        inside.spawn(words("Slide, or type a figure.   Enter goes through with it, Escape drops it.", 10.0, FAINT));
    });
}

/// The Gear page: the game's three parts side by side (`ItemsPanel`). On
/// the left the equipment's slot views where the game's own panel has
/// them, over a figure; in the middle each container in its slot with its
/// grids beside it; on the right what is open, in the width of the game's
/// stash panel.
///
/// Behind the slots, the character's own model where the options have it
/// (`CHARACTER MODEL`, `figure_view`), over the whole panel; else the
/// app's paper figure.
/// What the page's left panel shows: the tab it is drawn for (GEAR or
/// HEALTH), the game's health panel with the words and the health it
/// shows, and the screen's size in its units, which the left panel's
/// width goes by.
pub struct LeftPanel<'a> {
    pub tab: usize,
    pub look: &'a crate::screens::health_view::HealthLook,
    pub said: &'a crate::screens::health_view::Words,
    pub health: Option<&'a trp_sim::condition::health::Health>,
    pub screen: [f32; 2],
    /// The crate panel's stripes cut to their masks (`StashFills`).
    pub stash_fills: &'a HashMap<usize, Handle<Image>>,
    /// Whether the panel of what is looted is shown yet (`Opening`).
    pub loot_shown: bool,
}

/// The crate panel's stripes cut to their masks (`TARKOV UI`), made at the
/// window's scale (`make_stash_fills`), by the mask's object.
#[derive(Resource, Default)]
pub struct StashFills {
    pub(super) scale: u32,
    pub(super) images: HashMap<usize, Handle<Image>>,
}

/// Makes the crate panel's stripes (`StashFills`) afresh as the window's
/// scale or shape changes, before the page is drawn with them.
pub(super) fn make_stash_fills(look: Res<crate::screens::health_view::HealthLook>, window: Query<&Window, With<PrimaryWindow>>, mut images: ResMut<Assets<Image>>, mut fills: ResMut<StashFills>) {
    let (Some(game), Ok(window)) = (look.panel.as_ref().filter(|_| look.tarkov_ui && look.stash.is_some()), window.single()) else { return };
    if window.height() <= 0.0 {
        return;
    }
    let s = window.height() / SCREEN_HEIGHT;
    let key = (s * 997.0 + window.width()).to_bits();
    if fills.scale == key {
        return;
    }
    let Some([_, _, width, height]) = game.stash_in([window.width() / s, SCREEN_HEIGHT]) else { return };
    for (_, old) in fills.images.drain() {
        images.remove(&old);
    }
    fills.images = crate::screens::health_view::stash_fill_images(&look, [width, height], s, &mut images);
    fills.scale = key;
}
