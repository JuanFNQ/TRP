//! The battle view drawn: the quick bar and picker, health followed, fading.

use super::*;

/// The game's colour of a part on the battle screen
/// (`EHealthColorScheme.GetBodyPartColor`, then `Saturation(0.5)`): its
/// health's share squared; white to red, red brought up to white by it
/// (`Color.Gamma`); green to red, its hue at 120 degrees times it; black
/// destroyed; every channel then halved and the alpha whole.
fn part_color(share: f32, destroyed: bool, green_to_red: bool) -> Color {
    if destroyed {
        return Color::BLACK;
    }
    let h = share.clamp(0.0, 1.0).powi(2);
    let [r, g, b] = if green_to_red {
        let [r, g, b] = crate::screens::health_view::hsv(h * 120.0, 1.0, 1.0);
        [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0]
    } else {
        [1.0, h, h]
    };
    Color::srgb(r * 0.5, g * 0.5, b * 0.5)
}

/// Draws both panels (`TARKOV UI`) as the scene has them, each whole in a
/// window's size, hidden until faded in; again as the window's size
/// changes.
pub(super) fn draw_battle(mut commands: Commands, look: Res<HealthLook>, said: Res<Words>, window: Query<&Window, With<PrimaryWindow>>, roots: Query<(Entity, &BattleRoot)>, mut state: ResMut<BattleState>) {
    let Some(views) = look.battle.as_ref().filter(|_| look.tarkov_ui) else { return };
    let Ok(window) = window.single() else { return };
    let size = [window.width().round() as u32, window.height().round() as u32];
    if size[1] == 0 || look.images.is_empty() || roots.iter().any(|(_, root)| root.panel < 2 && root.size == size) {
        return;
    }
    for (entity, _) in roots.iter().filter(|(_, root)| root.panel < 2) {
        commands.entity(entity).despawn();
    }
    state.effects.clear();
    let s = size[1] as f32 / 1080.0;
    let screen = [size[0] as f32 / s, 1080.0];
    let whole = Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() };
    let on = crate::ui::screen_fade::OnScreen::Battle;
    commands.spawn((BattleRoot { panel: 0, size }, whole.clone(), Visibility::Hidden, on)).with_children(|root| {
        draw_nodes(root, &look, &views.stance, &look.stance_drawn, STANCE, Marks::default(), &said, Frame::In(screen), [0.0, 0.0], s);
        // The twenty notches the script makes in the speed's row
        // (`BattleStanceNotch.Init`): the row a horizontal group that
        // shares itself among them, each notch's picture where the
        // template keeps it in its share.
        let (Some(row), Some(template)) = (views.notches_place, views.notch_template) else { return };
        let Some(image) = views.stance.find(&format!("{}/Image", views.stance.nodes[template].path)) else { return };
        let [x, y, w, h] = views.stance.placed_in(row, screen).rect;
        let cell = trp_data::interface::screen::Rect { at: [0.0, 0.0], size: [w / 20.0, h] };
        let at = trp_data::interface::screen::place(views.stance.nodes[image].layout, cell);
        let color = views.stance.nodes[image].image.map_or(Color::WHITE, |image| crate::screens::health_view::rgba(image.color));
        for index in 0..20 {
            let node = Node {
                position_type: PositionType::Absolute,
                left: Val::Px((x + index as f32 * w / 20.0 + at.at[0]) * s),
                top: Val::Px((y + h - at.at[1] - at.size[1]) * s),
                width: Val::Px(at.size[0] * s),
                height: Val::Px(at.size[1] * s),
                ..default()
            };
            root.spawn((SpeedNotch(index), node, BackgroundColor(color), Own { image: None, ground: Some(color), text: None }));
        }
    });
    commands.spawn((BattleRoot { panel: 1, size }, whole, Visibility::Hidden, on)).with_children(|root| {
        draw_nodes(root, &look, &views.health, &look.body_drawn, BODY, Marks::default(), &said, Frame::In(screen), [0.0, 0.0], s);
    });
}

/// Draws the battle screen's quick bar (`TARKOV UI`) where the game's
/// screen has it (`QuickViews::quick_in`: its middle 57.5 right of the
/// screen's, 10 down, as wide as its views ask), as the inventory's
/// quick-use panel is drawn (`health_view::draw_quick_views`): every view
/// with its key's words, its thing's ground and picture turned and fitted
/// (`QuickSlotItemView`), its caption the thing's short name; the view
/// whose thing is in the hands lit (`RefreshSelection`: its ground white,
/// its caption black), the rest with their ground black at 0.6. Drawn
/// afresh as any of that changes.
///
/// Faded out with nothing to come, the bar is not kept: Bevy lays out
/// every node there is, shown or not, each frame. It is
/// drawn afresh when it is raised.
#[allow(clippy::too_many_arguments)]
pub(super) fn draw_quick_bar(
    mut commands: Commands,
    look: Res<HealthLook>,
    said: Res<Words>,
    carried: Res<Carried>,
    bindings: Res<crate::platform::keys::Bindings>,
    held: Res<HandsHeld>,
    window: Query<&Window, With<PrimaryWindow>>,
    roots: Query<(Entity, &BattleRoot, Option<&QuickDrawn>)>,
    state: Res<BattleState>,
    picker: Res<crate::screens::ammo_picker::Picker>,
    // The window's size the bar was last looked at for; none once what it
    // is drawn from has changed.
    mut dirty: Local<Option<[u32; 2]>>,
) {
    // What the bar is drawn from, changed since it was last looked at (kept
    // until a frame goes through to the drawing, so that a change on a frame
    // that stops early is still seen): with nothing changed, the window the
    // same and the bar there, the bar would be found the same as drawn.
    let changed = carried.is_changed() || held.is_changed() || bindings.is_changed() || look.is_changed() || said.is_changed() || picker.is_changed();
    if changed {
        *dirty = None;
    }
    let Some(quick) = look.battle_quick.as_ref().filter(|_| look.tarkov_ui) else { return };
    if state.quick.alpha <= 0.0 && state.quick.steps.is_empty() {
        for (entity, ..) in roots.iter().filter(|(_, root, _)| root.panel == 2) {
            commands.entity(entity).despawn();
        }
        return;
    }
    let Ok(window) = window.single() else { return };
    let size = [window.width().round() as u32, window.height().round() as u32];
    if size[1] == 0 || look.images.is_empty() {
        return;
    }
    if *dirty == Some(size) && roots.iter().any(|(_, root, was)| root.panel == 2 && was.is_some()) {
        return;
    }
    *dirty = Some(size);
    let mut drawn = vec![format!("{size:?} {:?}", held.item)];
    let mut words: HashMap<String, (String, String)> = HashMap::new();
    let mut selected = Vec::new();
    let mut marks = HashMap::new();
    for slot in &quick.slots {
        let (binding, item) = crate::screens::inventory_view::quick_thing(&carried, &slot.key);
        // (A weapon's view red with a malfunction the shooter knows of:
        // `QuickSlotView.method_0`, on its state's change.)
        let mark = item.and_then(SlotMark::of);
        if let Some(mark) = mark {
            marks.insert(slot.key.clone(), mark);
        }
        let own = slot.key.trim_start_matches("Item").replace("10", "0");
        let key = crate::screens::inventory_view::quick_key_words(&bindings, binding, &own);
        let caption = item.map_or(String::new(), |item| carried.name(item));
        if item.is_some_and(|item| Some(item.id) == held.item) {
            selected.push(slot.key.clone());
        }
        // (Its picture's key with it, and whether that has come: a weapon
        // given a magazine of another kind is a new build, its picture
        // another, the bar drawn again with its loader, and again as it
        // comes.)
        let picture = item.map(|item| crate::screens::inventory_view::shown_key(&carried, item));
        let pictured = picture.as_ref().is_some_and(|key| carried.pictures.contains_key(key));
        // (Its figure with it: a medkit's resource spent, the bar is drawn
        // again.)
        let figure = item.and_then(crate::screens::inventory_view::quick_resource).map(|(said, _)| said);
        drawn.push(format!("{} {key} {caption} {:?} {picture:?} {pictured} {figure:?} {mark:?}", slot.key, item.map(|item| item.id)));
        words.insert(slot.key.clone(), (key, caption));
    }
    // (The magazine picker, and whether its things' pictures have come.)
    if let Some(shown) = picker.shown.as_ref() {
        let pictured: Vec<bool> = shown.choices.iter().map(|id| carried.find(*id).is_some_and(|item| carried.pictures.contains_key(&crate::screens::icons::picture_key(item)))).collect();
        drawn.push(format!("picker {shown:?} {pictured:?}"));
    }
    if roots.iter().any(|(_, root, was)| root.panel == 2 && was.is_some_and(|was| was.0 == drawn)) {
        return;
    }
    for (entity, ..) in roots.iter().filter(|(_, root, _)| root.panel == 2) {
        commands.entity(entity).despawn();
    }
    let s = size[1] as f32 / 1080.0;
    let screen = [size[0] as f32 / s, 1080.0];
    let [x, y, width, height] = quick.quick_in(screen);
    let bar = Node { position_type: PositionType::Absolute, left: Val::Px(x * s), top: Val::Px(y * s), width: Val::Px(width * s), height: Val::Px(height * s), ..default() };
    commands.spawn((BattleRoot { panel: 2, size }, QuickDrawn(drawn), bar, Visibility::Hidden, crate::ui::screen_fade::OnScreen::Battle)).with_children(|bar| {
        let views = [width, height];
        crate::screens::health_view::draw_quick_views(bar, &look, quick, &look.battle_quick_drawn, QUICK_BAR, &said, views, &words, Some(&selected), &marks, 0, s);
        for slot in &quick.slots {
            let (Some(install), (_, Some(item))) = (slot.install, crate::screens::inventory_view::quick_thing(&carried, &slot.key)) else { continue };
            let [ix, iy, iw, ih] = quick.panel.rect(install, views);
            // No ground of its kind's colour: the quick slot's view draws
            // its picture and value alone (`QuickSlotItemView`, the same
            // prefab as the inventory's bar: `courses10`'s `hover-tooltips`).
            let place = Node { position_type: PositionType::Absolute, left: Val::Px(ix * s), top: Val::Px(iy * s), width: Val::Px(iw * s), height: Val::Px(ih * s), overflow: Overflow::clip(), ..default() };
            bar.spawn(place).with_children(|place| {
                crate::screens::inventory_view::quick_picture(place, &carried, item, [iw, ih], s);
                crate::screens::inventory_view::quick_value(place, &look, item, [iw, ih], s);
            });
        }
        crate::screens::health_view::draw_quick_views(bar, &look, quick, &look.battle_quick_drawn, QUICK_BAR, &said, views, &words, Some(&selected), &marks, 1, s);
        if let Some(shown) = picker.shown.as_ref() {
            draw_picker(bar, &look, quick, &carried, &said, shown, views, s);
        }
    });
}

/// The magazine picker on the bar (`InventoryScreenQuickAccessPanel.
/// ShowAmmoSelector`, `AmmoSelector.ShowAcceptableMags`): the weapon's
/// view's arrow (`ShowArrow`), and the picker's top left put by its offset
/// from that view's middle (`transform.position = screenPosition +
/// _arrowOffset`: screen pixels, y up), its things' views as the
/// inventory's grids draw them, each as large as its cells and not turned
/// (`GridItemView.Create`, `ItemRotation.Horizontal`), in its flow with the
/// cancel last (`SetAsLastSibling`), the cancel's ground in its chosen
/// colour when it is the choice (`SetSelectedState`); a thing's view lit
/// is `light_picker`'s. A thing no longer carried is left out.
#[allow(clippy::too_many_arguments)]
fn draw_picker(bar: &mut ChildSpawnerCommands, look: &HealthLook, quick: &trp_data::interface::screen::QuickViews, carried: &Carried, said: &Words, shown: &crate::screens::ammo_picker::Shown, views: [f32; 2], s: f32) {
    let Some(picker) = quick.ammo_selector.as_ref() else { return };
    let Some(slot) = quick.slots.iter().find(|slot| crate::screens::inventory_view::quick_thing(carried, &slot.key).1.is_some_and(|item| item.id == shown.weapon)) else { return };
    if let Some(arrow) = slot.arrow {
        let set = HashMap::from([(arrow, true)]);
        draw_nodes(bar, look, &quick.panel, &look.battle_quick_drawn, QUICK_BAR + (2 << 16), Marks { within: &[arrow], set: Some(&set), ..Marks::default() }, said, Frame::Is(views), [0.0, 0.0], s);
    }
    let [x, y, w, h] = quick.panel.rect(slot.object, views);
    let corner = [x + w / 2.0 + picker.arrow_offset[0] / s, y + h / 2.0 - picker.arrow_offset[1] / s];
    let things: Vec<(usize, &trp_sim::items::inventory::Item)> = shown.choices.iter().enumerate().filter_map(|(at, id)| Some((at, carried.find(*id)?))).collect();
    let cell = crate::screens::inventory_view::CELL;
    let mut sizes: Vec<[f32; 2]> = things.iter().map(|(_, item)| item.cells()).map(|[w, h]| [w as f32 * cell + 1.0, h as f32 * cell + 1.0]).collect();
    sizes.push(picker.cancel_size);
    let (places, _) = picker.flow(&sizes);
    let placed = |[px, py]: [f32; 2], [pw, ph]: [f32; 2]| Node {
        position_type: PositionType::Absolute,
        left: Val::Px((corner[0] + px) * s),
        top: Val::Px((corner[1] + py) * s),
        width: Val::Px(pw * s),
        height: Val::Px(ph * s),
        ..default()
    };
    for ((at, item), (&place, &size)) in things.iter().zip(places.iter().zip(&sizes)) {
        let room = Vec2::new(size[0] * s, size[1] * s);
        bar.spawn((PickerCell(*at), placed(place, size))).with_children(|view| {
            crate::screens::inventory_view::item_view(view, carried, item, Node { width: Val::Px(room.x), height: Val::Px(room.y), ..default() }, false, room, false, PICKER_LAYER, s);
        });
    }
    // The cancel, its own objects moved to its place in the flow.
    let Some(&to) = places.last() else { return };
    let [cx, cy, ..] = quick.panel.rect(picker.cancel, views);
    let inside: Vec<usize> = (0..quick.panel.nodes.len())
        .filter(|&index| {
            let mut above = Some(index);
            while let Some(at) = above {
                if at == picker.cancel {
                    return true;
                }
                above = quick.panel.nodes[at].parent;
            }
            false
        })
        .collect();
    let set: HashMap<usize, bool> = inside.iter().map(|&index| (index, true)).collect();
    let [r, g, b, a] = if shown.selected >= shown.choices.len() { picker.selected } else { picker.normal };
    let colors = HashMap::from([(picker.cancel_ground, Color::srgba(r, g, b, a))]);
    let origin = [corner[0] + to[0] - cx, corner[1] + to[1] - cy];
    draw_nodes(bar, look, &quick.panel, &look.battle_quick_drawn, QUICK_BAR + (3 << 16), Marks { within: &[picker.cancel], set: Some(&set), colors: Some(&colors), ..Marks::default() }, said, Frame::Is(views), origin, s);
}

/// The magazine picker's choice lit (`ItemView.Highlight`: its colour
/// panel the selected colour, `UpdateColor`), every other view of it its
/// own ground; set in the colours the bar's fade draws from (`Own`).
pub(super) fn light_picker(picker: Res<crate::screens::ammo_picker::Picker>, cells: Query<(&PickerCell, &Children)>, mut views: Query<(&crate::screens::inventory_view::Ground, &mut Own)>) {
    let selected = picker.shown.as_ref().map(|shown| shown.selected);
    for (cell, children) in &cells {
        for child in children.iter() {
            if let Ok((ground, mut own)) = views.get_mut(child) {
                let want = Some(if Some(cell.0) == selected { crate::screens::health_view::ITEM_SELECTED } else { ground.0 });
                if own.ground != want {
                    own.ground = want;
                }
            }
        }
    }
}

/// The pistols' glow drawn (`QuickSlotView.ShowQuickdrawGlow`): each
/// glowing view's `InstallPlace` (84, 193, 255) at the glow's alpha in place
/// of its own colour; the glow over or killed, each back at rest
/// (`method_0`: as `SlotMark` has it, else the scene's). Set in the colours
/// the bar's fade draws from (`Own`), over what `draw_quick_bar` drew.
pub(super) fn glow_quick(state: Res<BattleState>, look: Res<HealthLook>, carried: Res<Carried>, mut was: Local<Vec<String>>, mut pieces: Query<(&Piece, &mut Own)>) {
    let Some(quick) = look.battle_quick.as_ref().filter(|_| look.tarkov_ui) else { return };
    let glow = state.quickdraw.as_ref().and_then(|glow| Some((glow.slots.as_slice(), glow_alpha(glow.at, glow.legs)?)));
    let glowing = glow.map_or(&[][..], |(slots, _)| slots);
    if glowing.is_empty() && was.is_empty() {
        return;
    }
    let mut want = HashMap::new();
    for slot in quick.slots.iter().filter(|slot| glowing.contains(&slot.key) || was.contains(&slot.key)) {
        let Some(install) = slot.install else { continue };
        let colour = match glow {
            Some((slots, alpha)) if slots.contains(&slot.key) => Color::srgba_u8(GLOW_RGB[0], GLOW_RGB[1], GLOW_RGB[2], 0).with_alpha(alpha),
            _ => match quick_thing(&carried, &slot.key).1.and_then(SlotMark::of) {
                Some(mark) => mark.install(),
                None => match quick.panel.nodes[install].image.as_ref() {
                    Some(image) => crate::screens::health_view::rgba(image.color),
                    None => continue,
                },
            },
        };
        want.insert(QUICK_BAR + install, colour);
    }
    *was = glowing.to_vec();
    for (piece, mut own) in &mut pieces {
        if let Some(&colour) = want.get(&piece.0) {
            if own.ground != Some(colour) {
                own.ground = Some(colour);
            }
        }
    }
}

/// Keeps each newly drawn object's own colours (`Own`), which its panel's
/// fade draws it at a share of.
/// Only what was put in place since it last ran is looked at (everything
/// under a root was put there, and was seen then), not every object of
/// every panel on every frame.
#[allow(clippy::type_complexity)]
pub(super) fn keep_own(mut commands: Commands, roots: Query<(), With<BattleRoot>>, parents: Query<&ChildOf>, fresh: Query<(Entity, Option<&ImageNode>, Option<&BackgroundColor>, Option<&TextColor>), (Without<Own>, Changed<ChildOf>)>) {
    for (entity, image, ground, text) in &fresh {
        if parents.iter_ancestors(entity).any(|above| roots.contains(above)) {
            commands.entity(entity).insert(Own { image: image.map(|image| image.color), ground: ground.map(|ground| ground.0), text: text.map(|text| text.0) });
        }
    }
}

/// Follows health as the health panel's script does: each part's colour,
/// the ground's alpha by the colours' scheme, and the effects' icons (the
/// effects at work, those the game has an icon for, one icon of a kind
/// with how many there are where more than one and they are not of a kind
/// that stacks, pain hidden by a painkiller: `EffectDescriptionFilter`).
#[allow(clippy::type_complexity)]
pub(super) fn follow_health(
    mut commands: Commands,
    controller: Res<Controller>,
    look: Res<HealthLook>,
    mut state: ResMut<BattleState>,
    window: Query<&Window, With<PrimaryWindow>>,
    roots: Query<(Entity, &BattleRoot)>,
    shown: Query<Entity, With<BattleEffect>>,
    mut pieces: Query<(&Piece, &mut Own)>,
) {
    let Some(views) = look.battle.as_ref().filter(|_| look.tarkov_ui) else { return };
    let Some(health) = controller.health.as_ref() else { return };
    let modes = state.modes;
    const NAMES: [&str; 7] = ["Head", "Chest", "Stomach", "LeftArm", "RightArm", "LeftLeg", "RightLeg"];
    for (piece, mut own) in &mut pieces {
        let Some(index) = piece.0.checked_sub(BODY).filter(|&index| index < SPAN) else { continue };
        if Some(index) == views.background {
            own.image = Some(Color::WHITE.with_alpha(if modes.green_to_red { 0.2 } else { 1.0 }));
        } else if let Some((name, _)) = views.parts.iter().find(|(_, image)| *image == index) {
            let Some(at) = NAMES.iter().position(|n| n == name) else { continue };
            let part = health.part(Part::ALL[at]);
            own.image = Some(part_color(part.current / part.maximum.max(1e-6), part.destroyed, modes.green_to_red));
        }
    }
    // The effects' icons.
    use trp_sim::condition::health::Kind;
    let at_work: Vec<Kind> = health.effects.iter().filter(|effect| effect.active()).map(|effect| effect.kind).collect();
    let painkiller = at_work.contains(&Kind::PainKiller);
    let mut wanted: Vec<(&'static str, usize)> = Vec::new();
    let stimulators = crate::screens::health_view::stimulator_icons(health);
    for kind in at_work {
        if kind == Kind::Pain && painkiller {
            continue;
        }
        // A stimulator's two icons, every stimulator's merged into the
        // first's and never counted (`UpdateActiveEffects`: a `Stimulant`'s
        // buffs added to the first of its type).
        if kind == Kind::Stimulator {
            for name in &stimulators {
                if !wanted.iter().any(|(had, _)| had == name) {
                    wanted.push((name, 1));
                }
            }
            continue;
        }
        let Some(name) = effect_icon(kind) else { continue };
        match wanted.iter_mut().find(|(had, _)| *had == name) {
            Some((_, count)) => *count += 1,
            None => wanted.push((name, 1)),
        }
    }
    if wanted == state.effects {
        return;
    }
    let Some((root, _)) = roots.iter().find(|(_, root)| root.panel == 1) else { return };
    state.effects = wanted.clone();
    for entity in &shown {
        commands.entity(entity).despawn();
    }
    let Ok(window) = window.single() else { return };
    let s = window.height() / 1080.0;
    let screen = [window.width() / s, 1080.0];
    let (Some(effects), Some(grid)) = (views.effects, views.effects_grid) else { return };
    let [ex, ey, _, eh] = views.health.placed_in(effects, screen).rect;
    let rows = (((eh - grid.padding[2] - grid.padding[3] + grid.spacing[1] + 0.001) / (grid.cell[1] + grid.spacing[1])).floor() as usize).max(1);
    let template = views.effect_template.map(|icon| views.health.placed_in(icon, screen).rect);
    let count_text = views.stack_count.and_then(|text| Some((views.health.placed_in(text, screen).rect, views.health.nodes[text].text.clone()?)));
    commands.entity(root).with_children(|root| {
        for (n, (name, count)) in wanted.into_iter().enumerate() {
            let Some(picture) = look.effect_pictures.get(name) else { continue };
            let Some(image) = look.images.get(picture.texture).cloned().flatten() else { continue };
            // Down a column first (the grid's start axis, vertical), then
            // the next column.
            let (column, row) = if grid.start_axis == 1 { (n / rows, n % rows) } else { (n % 4, n / 4) };
            let x = ex + grid.padding[0] + column as f32 * (grid.cell[0] + grid.spacing[0]);
            let y = ey + grid.padding[2] + row as f32 * (grid.cell[1] + grid.spacing[1]);
            let node = Node { position_type: PositionType::Absolute, left: Val::Px(x * s), top: Val::Px(y * s), width: Val::Px(grid.cell[0] * s), height: Val::Px(grid.cell[1] * s), ..default() };
            root.spawn((BattleEffect, node, ImageNode { image, rect: Some(picture.cut), flip_y: true, ..default() }, Own { image: Some(Color::WHITE), ground: None, text: None }));
            // Its count where more than one, but of a kind that stacks
            // (`IStackable`: tremor, of the app's).
            if count > 1 && name != "Tremor" {
                let (Some([tx, ty, ..]), Some(([cx, cy, cw, ch], text))) = (template, count_text.as_ref()) else { continue };
                let font = TextFont { font: look.styled_font(&views.health, text.font, text.style), font_size: text.size * s, weight: crate::screens::health_view::weight_of(text.style), ..default() };
                let color = crate::screens::health_view::rgba(text.color);
                let at = Node { position_type: PositionType::Absolute, left: Val::Px((x + cx - tx) * s), top: Val::Px((y + cy - ty) * s), min_width: Val::Px(cw * s), height: Val::Px(ch * s), ..default() };
                let effect = look.text_effect(&views.health, text.font);
                let tmp = look.tmp_of(&views.health, text);
                root.spawn((BattleEffect, at)).with_children(|frame| match tmp {
                    // As the game draws it.
                    Some((asset, material)) => {
                        let run = trp_data::interface::tmp_text::TmpRun { text: count.to_string(), size: text.size * s, color: crate::screens::health_view::screen_numbers(color), bold: text.style & 1 != 0 };
                        let layout = trp_data::interface::tmp_text::TmpLayout { em: text.size * s, spacing: text.spacing, kerning: text.kerning, wrap: None, align: text.horizontal & 7, lead: 0.0 };
                        crate::screens::health_view::tmp_words(frame, Node::default(), crate::screens::health_view::TmpText { asset, material, runs: vec![run], layout }, &mut |_| {});
                    }
                    None => crate::screens::health_view::effect_words(frame, Node::default(), &[(count.to_string(), font, color)], effect, text.size * s, &mut |_, _, _| {}),
                });
            }
        }
    });
}

/// Draws each panel at its fade (`CanvasGroup.alpha`): every object under
/// it at that share of its own colours, the panel hidden while out.
pub(super) fn fade_battle(state: Res<BattleState>, mut roots: Query<(Entity, &BattleRoot, &mut Visibility)>, children: Query<&Children>, mut colours: Query<(&Own, Option<&mut ImageNode>, Option<&mut BackgroundColor>, Option<&mut TextColor>)>) {
    for (root, panel, mut visibility) in &mut roots {
        let alpha = match panel.panel {
            0 => state.stance.alpha,
            1 => state.health.alpha,
            _ => state.quick.alpha,
        };
        let want = if alpha > 0.0 { Visibility::Inherited } else { Visibility::Hidden };
        if *visibility != want {
            *visibility = want;
        }
        if alpha <= 0.0 {
            continue;
        }
        for entity in children.iter_descendants(root) {
            let Ok((own, image, ground, text)) = colours.get_mut(entity) else { continue };
            let faded = |color: Color| color.with_alpha(color.alpha() * alpha);
            if let (Some(mut image), Some(color)) = (image, own.image) {
                let color = faded(color);
                if image.color != color {
                    image.color = color;
                }
            }
            if let (Some(mut ground), Some(color)) = (ground, own.ground) {
                let color = faded(color);
                if ground.0 != color {
                    ground.0 = color;
                }
            }
            if let (Some(mut text), Some(color)) = (text, own.text) {
                let color = faded(color);
                if text.0 != color {
                    text.0 = color;
                }
            }
        }
    }
}
