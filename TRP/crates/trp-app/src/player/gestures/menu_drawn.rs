//! The prompt and the menu drawn.

use super::*;

/// The quick prompt and the held list at the bottom left.
#[derive(Component)]
pub(super) struct PromptRoot;

/// The prompt, whose alpha blinks.
#[derive(Component)]
pub(super) struct PromptBox;

/// The fonts the menu draws in.
fn fonts(look: Option<&crate::screens::health_view::HealthLook>) -> (Handle<Font>, Handle<Font>) {
    let named = |name: &str| look.and_then(|look| look.font_named(name));
    let font = named("Bender Normal SDF").or_else(|| named("Bender Shadowed SDF")).unwrap_or_default();
    let bold = look.and_then(|look| look.bold_named("Bender Normal SDF")).unwrap_or_else(|| font.clone());
    (font, bold)
}

/// A phrase's or a hand sign's words, upper case (their TMP styles).
fn words_of(texts: &trp_data::character::inventory::ItemTexts, number: i32) -> String {
    let key = match is_sign(number) {
        Some(sign) => sign.name(),
        None => phrase::name(number).to_owned(),
    };
    texts.say(&key).unwrap_or(key).to_uppercase()
}

/// A run of words in a box, aligned.
fn text_in(parent: &mut ChildSpawnerCommands, area: Area, words: String, font: &Handle<Font>, size: f32, color: Color, justify: JustifyContent, inset: f32, bold: bool) {
    let mut node = area.node();
    node.justify_content = justify;
    node.align_items = AlignItems::Center;
    node.padding = UiRect::horizontal(Val::Px(inset));
    node.overflow = Overflow::clip();
    parent.spawn(node).with_children(|cell| {
        cell.spawn((Text::new(words), TextFont { font: font.clone(), font_size: size, weight: crate::screens::health_view::weight_of(u32::from(bold)), ..default() }, TextColor(color), TextLayout::new_with_no_wrap()));
    });
}

/// A sprite in a box, tinted; a plain box of the tint where it is not read.
fn sprite_in(parent: &mut ChildSpawnerCommands, area: Area, sprite: Option<&(Handle<Image>, Rect)>, tint: Color) {
    match sprite {
        Some((image, rect)) => {
            parent.spawn((area.node(), ImageNode { image: image.clone(), color: tint, rect: Some(*rect), flip_y: true, ..default() }));
        }
        None => {
            parent.spawn((area.node(), BackgroundColor(tint)));
        }
    }
}

/// The blink's alpha at a time into it (`PhraseAlphaAnim`: 1, 0, 1, 0, 1
/// at sixths of a second with flat tangents, held to the second, looped).
pub(super) fn blink_alpha(t: f64) -> f32 {
    let t = t.rem_euclid(1.0) as f32;
    if t >= 2.0 / 3.0 {
        return 1.0;
    }
    let segment = (t * 6.0).floor();
    let u = t * 6.0 - segment;
    let eased = u * u * (3.0 - 2.0 * u);
    let (from, to) = if segment as i32 % 2 == 0 { (1.0, 0.0) } else { (0.0, 1.0) };
    from + (to - from) * eased
}

/// The prompt and the held list (`GesturesQuickPanel`): drawn afresh when
/// what they show changes; the prompt's alpha every frame while it blinks.
#[allow(clippy::too_many_arguments)]
pub(super) fn draw_prompt(
    mut commands: Commands,
    mut gestures: ResMut<Gestures>,
    bindings: Res<crate::platform::keys::Bindings>,
    carried: Res<crate::screens::inventory_view::Carried>,
    look: Option<Res<crate::screens::health_view::HealthLook>>,
    pictures: Res<Pictures>,
    controller: Res<Controller>,
    window: Query<&Window, With<PrimaryWindow>>,
    roots: Query<Entity, With<PromptRoot>>,
    mut boxes: Query<&mut BackgroundColor, With<PromptBox>>,
    mut box_images: Query<&mut ImageNode, With<PromptBox>>,
    time: Res<Time<Real>>,
) {
    let Ok(window) = window.single() else { return };
    let frame = Frame::of(window);
    let shown = !controller.inventory_open && !controller.edit_build && !gestures.open;
    let top = gestures.top().filter(|_| shown);
    let rows = if gestures.list.is_some() && shown { gestures.rows() } else { Vec::new() };
    let key = bindings.caption("MumbleQuick").to_owned();
    let drawn = {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        (top, &rows, gestures.list, &key, frame.w as u32, frame.h as u32).hash(&mut hasher);
        hasher.finish()
    };
    let now = time.elapsed_secs_f64();
    if gestures.prompt_drawn != Some(drawn) {
        gestures.prompt_drawn = Some(drawn);
        for root in &roots {
            commands.entity(root).despawn();
        }
        if top.is_none() && rows.is_empty() {
            return;
        }
        let (font, _) = fonts(look.as_deref());
        let s = frame.s;
        let sprite = |n: usize| pictures.sprites.get(n).and_then(Option::as_ref);
        let root = Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), width: Val::Px(frame.w), height: Val::Px(frame.h), ..default() };
        commands.spawn((PromptRoot, root, GlobalZIndex(6))).with_children(|root| {
            // The prompt: its box 210 right of and 37.7 up from the bottom
            // left, the key's tab outside its right, the arrow under it.
            if let Some(top) = top {
                let area = Area { x: 210.0 * s, y: frame.h - (24.4 + 13.3 + 15.0) * s, w: 94.0 * s, h: 15.0 * s };
                match sprite(SUBGROUP) {
                    Some((image, rect)) => root.spawn((PromptBox, area.node(), ImageNode { image: image.clone(), color: PROMPT_GROUND, rect: Some(*rect), flip_y: true, ..default() })),
                    None => root.spawn((PromptBox, area.node(), BackgroundColor(PROMPT_GROUND))),
                };
                text_in(root, area, words_of(&carried.texts, top), &font, 12.0 * s, TEXT_GESTURE, JustifyContent::FlexStart, 2.5 * s, false);
                let tab = Area { x: area.x + area.w, y: area.y, w: 22.0 * s, h: 15.0 * s };
                sprite_in(root, tab, sprite(TAB_RIGHT), Color::WHITE);
                text_in(root, tab, key.clone(), &font, 12.0 * s, TAB_WORDS, JustifyContent::Center, 0.0, false);
                let arrow = Area { x: area.x, y: area.y + area.h, w: 94.0 * s, h: 9.0 * s };
                if let Some(arrow_sprite) = sprite(DOWN_ICON) {
                    sprite_in(root, arrow, Some(arrow_sprite), Color::WHITE);
                }
            }
            // The held list: rows growing up from 345 right and 37.7 up.
            for (n, trigger) in rows.iter().enumerate() {
                let chosen = gestures.list == Some(n);
                let area = Area { x: 345.0 * s, y: frame.h - 37.7 * s - (n as f32 + 1.0) * 15.0 * s, w: 95.0 * s, h: 15.0 * s };
                root.spawn((area.node(), BackgroundColor(if chosen { ROW_CHOSEN } else { PROMPT_GROUND })));
                text_in(root, area, words_of(&carried.texts, *trigger), &font, 12.0 * s, if chosen { ROW_WORDS_CHOSEN } else { TEXT_AUDIO }, JustifyContent::FlexStart, 2.5 * s, false);
                if chosen {
                    let tab = Area { x: area.x + area.w, y: area.y, w: 22.0 * s, h: 15.0 * s };
                    sprite_in(root, tab, sprite(TAB_RIGHT), Color::WHITE);
                    text_in(root, tab, key.clone(), &font, 12.0 * s, TAB_WORDS, JustifyContent::Center, 0.0, false);
                }
            }
        });
    }
    // The blink, on the prompt's ground (its words keep their own).
    let alpha = match gestures.blink {
        Some((began, until)) if now < until => blink_alpha(now - began),
        Some(_) => {
            gestures.blink = None;
            1.0
        }
        None => 1.0,
    };
    for mut ground in &mut boxes {
        ground.0 = PROMPT_GROUND.with_alpha(PROMPT_GROUND.alpha() * alpha);
    }
    for mut image in &mut box_images {
        image.color = PROMPT_GROUND.with_alpha(PROMPT_GROUND.alpha() * alpha);
    }
}

/// The menu, its root over the whole screen.
#[derive(Component)]
pub(super) struct MenuRoot;

/// The menu (`GesturesMenu`): drawn afresh whenever what it shows changes
/// (the pointer's pick, the group open, the binds, the active phrases, the
/// binding menu, the window).
#[allow(clippy::too_many_arguments)]
pub(super) fn draw_menu(
    mut commands: Commands,
    mut gestures: ResMut<Gestures>,
    carried: Res<crate::screens::inventory_view::Carried>,
    look: Option<Res<crate::screens::health_view::HealthLook>>,
    pictures: Res<Pictures>,
    voiced: Res<Voiced>,
    window: Query<&Window, With<PrimaryWindow>>,
    roots: Query<Entity, With<MenuRoot>>,
) {
    let Ok(window) = window.single() else { return };
    let frame = Frame::of(window);
    let drawn = gestures.open.then(|| {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        let mut quick: Vec<i32> = gestures.quick.keys().copied().collect();
        quick.sort_unstable();
        (gestures.pick, gestures.group_open, gestures.binds, quick, frame.w as u32, frame.h as u32, voiced.0.len(), pictures.sprites.len()).hash(&mut hasher);
        gestures.bind_menu.map(|menu| (menu.number, menu.hovered, menu.at.x as i32, menu.at.y as i32)).hash(&mut hasher);
        hasher.finish()
    });
    if gestures.menu_drawn == drawn {
        return;
    }
    gestures.menu_drawn = drawn;
    for root in &roots {
        commands.entity(root).despawn();
    }
    if !gestures.open {
        return;
    }
    let (font, bold) = fonts(look.as_deref());
    let s = frame.s;
    let lay = layout(&frame, &gestures, &voiced.0, gestures.group_open);
    let sprite = |n: usize| pictures.sprites.get(n).and_then(Option::as_ref);
    let bound_to = |number: i32| gestures.binds.iter().position(|b| *b == number && number != 0);
    let texts = &carried.texts;
    let root = Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), width: Val::Px(frame.w), height: Val::Px(frame.h), ..default() };
    commands.spawn((MenuRoot, root, GlobalZIndex(30))).with_children(|root| {
        // The groups, under the hand signs.
        for (g, (_, ground, placed)) in lay.groups.iter().enumerate() {
            let (name, situational, phrases) = GROUPS[g];
            let any = phrases.iter().any(|p| voiced.0.contains(p) || *p == phrase::MUMBLE);
            let dim = if any { 1.0 } else { DIMMED };
            if !situational {
                if gestures.group_open == Some(g) {
                    let centre = frame.at(GROUP_AT[g]);
                    let glow = Area { x: centre.x - 170.0 * s, y: centre.y - 115.0 * s, w: 340.0 * s, h: 230.0 * s };
                    sprite_in(root, glow, sprite(GROUP_GLOW), Color::WHITE);
                }
                sprite_in(root, *ground, sprite(GROUP_BACK), Color::WHITE.with_alpha(dim));
                let band = Area { x: ground.x, y: ground.y + ground.h / 2.0 - 28.7 * s, w: ground.w, h: 57.4 * s };
                text_in(root, band, texts.say(name).unwrap_or_else(|| name.to_owned()).to_uppercase(), &font, 23.0 * s, GROUP_NAME.with_alpha(dim), JustifyContent::Center, 0.0, false);
            }
            for (trigger, area, justify, tab_left) in placed {
                let over = gestures.pick == Some(Pick::Phrase(g, *trigger));
                sprite_in(root, *area, sprite(if over { SUBGROUP_SELECTED } else { SUBGROUP }), Color::WHITE);
                let mut words = words_of(texts, *trigger);
                if phrase::situational(*trigger) {
                    words = format!("! {words}");
                }
                text_in(root, *area, words, &bold, 12.0 * s, if over { TEXT_AUDIO_SELECTED } else { TEXT_AUDIO }, *justify, 11.0 * s, true);
                if let Some(key) = bound_to(*trigger) {
                    tab(root, &frame, *area, *tab_left, key, &font, &sprite);
                }
            }
        }
        // The hand signs.
        for (n, (hexagon, label, left)) in lay.signs.iter().enumerate() {
            let over = gestures.pick == Some(Pick::Sign(n));
            sprite_in(root, *hexagon, sprite(if over { SMALL_GROUP_SELECTED } else { SMALL_GROUP }), Color::WHITE);
            if let Some((image, rect)) = pictures.icons.get(n).and_then(Option::as_ref) {
                // Kept to its shape in a 56 by 53 box.
                let (w, h) = (rect.width().max(1.0), rect.height().max(1.0));
                let k = (56.0 / w).min(53.0 / h);
                let (iw, ih) = (w * k * s, h * k * s);
                let icon = Area { x: hexagon.x + hexagon.w / 2.0 - iw / 2.0, y: hexagon.y + hexagon.h / 2.0 - ih / 2.0, w: iw, h: ih };
                root.spawn((icon.node(), ImageNode { image: image.clone(), rect: Some(*rect), flip_y: true, ..default() }));
            }
            root.spawn((label.node(), BackgroundColor(if over { GESTURE_SELECTED } else { GESTURE })));
            let number = Sign::GESTURES[n].game_number() as i32;
            text_in(root, *label, words_of(texts, number), &font, 12.0 * s, if over { TEXT_GESTURE_SELECTED } else { TEXT_GESTURE }, JustifyContent::Center, 5.0 * s, false);
            if let Some(key) = bound_to(number) {
                tab(root, &frame, *label, *left, key, &font, &sprite);
            }
        }
        // The bind panel: each slot by what it holds.
        for (key, (area, right)) in lay.slots.iter().enumerate() {
            let number = gestures.binds[key];
            let sign = is_sign(number).is_some();
            let over = gestures.pick == Some(Pick::Slot(key));
            let usable = number != 0 && (sign || ((voiced.0.contains(&number) || number == phrase::MUMBLE) && gestures.available(number)));
            let alpha = if usable { 1.0 } else { DIMMED };
            let (ground, ink) = match (sign, over) {
                (true, false) => (GESTURE, TEXT_GESTURE),
                (true, true) => (GESTURE_SELECTED, TEXT_GESTURE_SELECTED),
                (false, false) => (AUDIO, TEXT_AUDIO),
                (false, true) => (AUDIO_SELECTED, TEXT_AUDIO_SELECTED),
            };
            root.spawn((area.node(), BackgroundColor(ground.with_alpha(ground.alpha() * alpha))));
            let words = if number == 0 { String::new() } else { words_of(texts, number) };
            let justify = if *right { JustifyContent::FlexStart } else { JustifyContent::FlexEnd };
            text_in(root, *area, words, &font, 12.0 * s, ink.with_alpha(alpha), justify, 5.0 * s, false);
            // The key's tab between the columns.
            let tab_area = if *right { Area { x: area.x - 22.0 * s, y: area.y, w: 22.0 * s, h: area.h } } else { Area { x: area.x + area.w, y: area.y, w: 22.0 * s, h: area.h } };
            sprite_in(root, tab_area, sprite(if *right { TAB_LEFT } else { TAB_RIGHT }), Color::WHITE.with_alpha(alpha));
            text_in(root, tab_area, format!("F{}", key + 1), &font, 12.0 * s, TAB_WORDS, JustifyContent::Center, 0.0, false);
        }
        // The binding menu over all.
        if let Some(menu) = gestures.bind_menu {
            let lines = bind_lines(&frame, &menu);
            if let (Some(first), Some(last)) = (lines.first(), lines.last()) {
                let ground = Area { x: first.x - 2.0 * s, y: first.y - 2.0 * s, w: first.w + 4.0 * s, h: last.y + last.h - first.y + 4.0 * s };
                root.spawn((ground.node(), BackgroundColor(MENU_GROUND)));
            }
            for (key, line) in lines.iter().enumerate() {
                let over = menu.hovered == Some(key);
                root.spawn((line.node(), BackgroundColor(if over { AUDIO_SELECTED } else { MENU_BUTTON })));
                let holds = gestures.binds[key];
                let words = if holds == 0 { format!("F{}", key + 1) } else { words_of(texts, holds) };
                let tab_area = Area { x: line.x, y: line.y, w: 22.0 * s, h: line.h };
                text_in(root, tab_area, format!("F{}", key + 1), &font, 11.0 * s, if over { Color::BLACK } else { Color::WHITE }, JustifyContent::Center, 0.0, false);
                let rest = Area { x: line.x + 24.0 * s, y: line.y, w: line.w - 24.0 * s, h: line.h };
                text_in(root, rest, words, &font, 11.0 * s, if over { Color::BLACK } else { Color::WHITE }, JustifyContent::FlexStart, 2.0 * s, false);
            }
        }
    });
}

/// A key's tab beside a thing bound to it: outside its left or right edge.
fn tab<'a>(root: &mut ChildSpawnerCommands, frame: &Frame, area: Area, left: bool, key: usize, font: &Handle<Font>, sprite: &dyn Fn(usize) -> Option<&'a (Handle<Image>, Rect)>) {
    let s = frame.s;
    let (w, h) = (22.0 * s, 15.0 * s);
    let at = if left { Area { x: area.x - w, y: area.y, w, h } } else { Area { x: area.x + area.w, y: area.y, w, h } };
    sprite_in(root, at, sprite(if left { TAB_LEFT } else { TAB_RIGHT }), Color::WHITE);
    text_in(root, at, format!("F{}", key + 1), font, 12.0 * s, TAB_WORDS, JustifyContent::Center, 0.0, false);
}
