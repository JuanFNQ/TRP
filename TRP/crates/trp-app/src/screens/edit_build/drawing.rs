//! Drawing the screen, and the app's stats panel.

use super::*;

/// The screen drawn: its layers made once, each drawn again only as what
/// it shows changes; the slots' nodes placed every frame they move; the
/// loader shown while the preview's model is made.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn draw(
    mut commands: Commands,
    mut state: ResMut<EditBuild>,
    carried: Res<Carried>,
    look: Res<HealthLook>,
    said: Res<Words>,
    game: Res<crate::GameData>,
    mut icons: ResMut<crate::screens::icons::Icons>,
    window: Query<&Window, With<PrimaryWindow>>,
    time: Res<Time>,
    hovers: Query<(&SlotHover, &RelativeCursorPosition)>,
    mut placed: Query<(&SlotPiece, &mut Node, &mut UiTransform, &mut Visibility), (Without<Piece>, Without<AppLoader>)>,
    mut pieces: Query<(&Piece, &mut UiTransform, &mut Visibility), (Without<SlotPiece>, Without<AppLoader>)>,
    mut app_loader: Query<(&mut UiTransform, &mut Visibility), (With<AppLoader>, Without<SlotPiece>, Without<Piece>)>,
    mut picture: Query<&mut ImageNode, With<PreviewPicture>>,
    mut stats_images: Query<(&Piece, &mut ImageNode), Without<PreviewPicture>>,
) {
    let state = &mut *state;
    if !state.open {
        return;
    }
    let Ok(window) = window.single() else { return };
    let s = window.height() / HIGH;
    let wide = window.width() / s;
    let Some(build) = state.build.as_ref() else { return };
    let eb = look.edit_build();
    if state.prepared.is_none() {
        if let Some(eb) = eb {
            state.prepared = Some(prepare(eb, &look, &said));
        }
    }
    let game_look = eb.zip(state.prepared.as_ref());
    if state.notice.as_ref().is_some_and(|(_, until)| time.elapsed_secs() > *until) {
        state.notice = None;
    }
    // The layers, made once; all drawn again for another size.
    if state.drawn.root.is_none() {
        let root = commands.spawn((ScreenRoot, layer(), BackgroundColor(BEHIND.with_alpha(BEHIND_ALPHA)), GlobalZIndex(40))).id();
        let picture = commands.spawn((PreviewPicture, Button, ImageNode::default(), layer(), ChildOf(root))).id();
        let mut made = || commands.spawn((layer(), ChildOf(root))).id();
        let (under, lines, slots, stats, dropdown, over, notice) = (made(), made(), made(), made(), made(), made(), made());
        state.drawn = Drawn { root: Some(root), picture: Some(picture), under: Some(under), lines: Some(lines), slots: Some(slots), stats: Some(stats), dropdown: Some(dropdown), over: Some(over), notice: Some(notice), ..Drawn::default() };
    }
    if state.drawn.scale != Some((s, wide)) {
        state.drawn.scale = Some((s, wide));
        for layer in [state.drawn.under, state.drawn.lines, state.drawn.slots, state.drawn.stats, state.drawn.dropdown, state.drawn.over, state.drawn.notice].into_iter().flatten() {
            commands.entity(layer).despawn_related::<Children>();
        }
        (state.drawn.statics, state.drawn.statics_for, state.drawn.slots_for, state.drawn.dropdown_for, state.drawn.notice_for, state.drawn.stats_for) = (None, None, None, None, None, None);
        state.drawn.boxes.clear();
    }
    if let (Some(image), Some(entity)) = (state.preview.image.clone(), state.drawn.picture) {
        if let Ok(mut node) = picture.get_mut(entity) {
            if node.image != image {
                node.image = image;
            }
        }
    }
    // The slot the mouse is on: the one drawn last of those under it.
    let hovered = state.drawn.boxes.iter().rev().find_map(|nodes| hovers.iter().find(|(hover, at)| hover.0 == nodes.id && at.cursor_over()).map(|(hover, _)| hover.0)).filter(|_| !state.mouse_taken);
    state.hovered = hovered;
    // The screen's own objects, worked out again as the build, the weapon
    // or the toggles change.
    let statics_for = (state.made, state.toggles);
    let statics = (state.drawn.statics_for != Some(statics_for)).then(|| {
        let name = carried.texts.names.get(&build.kind.template).cloned().unwrap_or_else(|| build.kind.name.clone());
        let ready = state.weapon.and_then(|id| carried.equipment.item.find(id)).is_some_and(|weapon| !built(weapon, build, state.gear)) && !vital_missing(build);
        Statics { gear: state.gear, name, toggles: state.toggles, ready }
    });
    state.drawn.statics_for = Some(statics_for);
    if let Some(statics) = statics.filter(|statics| state.drawn.statics.as_ref() != Some(statics)) {
        let (Some(under), Some(over)) = (state.drawn.under, state.drawn.over) else { return };
        commands.entity(under).despawn_related::<Children>();
        commands.entity(over).despawn_related::<Children>();
        match game_look {
            Some((eb, prepared)) => {
                commands.entity(under).with_children(|under| statics_under(under, &look, eb, prepared, &statics, &said, s, wide));
                commands.entity(over).with_children(|over| statics_over(over, &look, eb, prepared, &statics, &said, s, wide));
            }
            None => {
                commands.entity(over).with_children(|over| statics_app(over, &statics, s, wide));
            }
        }
        state.drawn.statics = Some(statics);
    }
    // The loader while the preview's model is made (`RefreshingWeapon`),
    // turning.
    let refreshing = state.preview.key.as_deref() != Some(state.build_key.as_str());
    let shown = if refreshing { Visibility::Inherited } else { Visibility::Hidden };
    let spin = Rot2::radians((time.elapsed_secs() * LOADER_TURNS).fract() * std::f32::consts::TAU);
    // The loader's picture is drawn where its cut lies in its rectangle (a
    // sprite packed trimmed, the game's `spinner_big`), and turned about
    // that rectangle's middle as the game turns it: moved by as much as
    // turning its cut's middle about the rectangle's moves it.
    let off_middle = game_look
        .and_then(|(eb, prepared)| {
            let loader = prepared.loader?;
            let c = eb.screen.pictures.get(loader).copied().flatten()?.covers;
            let [_, _, w, h] = prepared.screen.rect(loader, [wide, HIGH]);
            Some(Vec2::new(w * ((c[0] + c[2]) / 2.0 - 0.5), h * (0.5 - (c[1] + c[3]) / 2.0)) * s)
        })
        .unwrap_or(Vec2::ZERO);
    if let Some(loader) = game_look.and_then(|(_, prepared)| prepared.loader) {
        for (piece, mut turn, mut visibility) in &mut pieces {
            if piece.0 != EB_SCREEN + loader {
                continue;
            }
            if *visibility != shown {
                *visibility = shown;
            }
            if refreshing {
                let moved = spin * off_middle - off_middle;
                turn.rotation = spin;
                turn.translation = Val2::px(moved.x, moved.y);
            }
        }
    }
    for (mut turn, mut visibility) in &mut app_loader {
        if *visibility != shown {
            *visibility = shown;
        }
        if refreshing {
            turn.rotation = spin;
        }
    }
    // The slots: all made again when the slots shown change, each at its
    // place from the first frame; a view drawn again as the mouse comes
    // onto it or its dropdown opens.
    if state.drawn.slots_for != Some(state.slots_made) {
        state.drawn.slots_for = Some(state.slots_made);
        let (Some(lines), Some(slots)) = (state.drawn.lines, state.drawn.slots) else { return };
        commands.entity(lines).despawn_related::<Children>();
        commands.entity(slots).despawn_related::<Children>();
        let mut boxes = Vec::with_capacity(state.slots.len());
        for (at, slot) in state.slots.iter().enumerate() {
            let place = state.placed.get(at).copied().flatten();
            let visibility = if place.is_some() { Visibility::Inherited } else { Visibility::Hidden };
            let (bone_at, view_at) = place.unwrap_or(([0.0, 0.0], [0.0, 0.0]));
            let (line_node, line_turn) = line_place(bone_at, view_at, s);
            commands.spawn((SlotPiece::Line(at), line_node, line_turn, UNROUNDED, BackgroundColor(class_color(slot)), visibility, ChildOf(lines)));
            let own = commands.spawn((layer(), ChildOf(slots))).id();
            let bone = commands.spawn((SlotPiece::Bone(at), point_node(bone_at, s), visibility, ChildOf(own))).id();
            let view = commands.spawn((SlotPiece::View(at), point_node(view_at, s), visibility, ChildOf(own))).id();
            let id = (slot.owner, slot.index);
            let shown = BoxShown { button: false, open: !state.menu_seen };
            let marks = slot_marks(&game, build, slot, &mut state.filters);
            commands.entity(bone).with_children(|bone| match game_look {
                Some((eb, prepared)) => bone_game(bone, &look, eb, prepared, slot, &state.texts, &said, s),
                None => bone_app(bone, slot, &state.texts, s),
            });
            commands.entity(view).with_children(|view| match game_look {
                Some((eb, prepared)) => view_game(view, &look, eb, prepared, &carried, &mut icons, state.shown.as_ref(), slot, shown, &marks, &said, s),
                None => view_app(view, &carried, &mut icons, state.shown.as_ref(), slot, s),
            });
            boxes.push(SlotNodes { id, own, view, shown });
        }
        state.drawn.boxes = boxes;
    } else {
        for at in 0..state.drawn.boxes.len() {
            let Some(slot) = state.slots.get(at) else { continue };
            let id = state.drawn.boxes[at].id;
            let open = state.menu == Some(id) || !state.menu_seen;
            // (A locked slot shows no dropdown's button: `HoverStartHandler`.)
            let wanted = BoxShown { button: (hovered == Some(id) || state.menu == Some(id)) && !slot.locked, open };
            if state.drawn.boxes[at].shown == wanted {
                continue;
            }
            let nodes = &mut state.drawn.boxes[at];
            nodes.shown = wanted;
            let (view, own) = (nodes.view, nodes.own);
            // (Raised over the others while the mouse is on it.)
            commands.entity(own).insert(ZIndex(if wanted.button { 1 } else { 0 }));
            commands.entity(view).despawn_related::<Children>();
            let marks = slot_marks(&game, build, slot, &mut state.filters);
            commands.entity(view).with_children(|view| match game_look {
                Some((eb, prepared)) => view_game(view, &look, eb, prepared, &carried, &mut icons, state.shown.as_ref(), slot, wanted, &marks, &said, s),
                None => view_app(view, &carried, &mut icons, state.shown.as_ref(), slot, s),
            });
        }
    }
    // Every frame: the slots' nodes where they lie, written only where they
    // have moved.
    for (piece, mut node, mut turn, mut visibility) in &mut placed {
        let at = match *piece {
            SlotPiece::Line(at) | SlotPiece::Bone(at) | SlotPiece::View(at) => at,
        };
        let Some(Some((bone_at, view_at))) = state.placed.get(at).copied() else {
            if *visibility != Visibility::Hidden {
                *visibility = Visibility::Hidden;
            }
            continue;
        };
        if *visibility != Visibility::Inherited {
            *visibility = Visibility::Inherited;
        }
        let (wanted, rotation) = match *piece {
            SlotPiece::Line(_) => {
                let (node, turned) = line_place(bone_at, view_at, s);
                (node, turned.rotation)
            }
            SlotPiece::Bone(_) => (point_node(bone_at, s), Rot2::IDENTITY),
            SlotPiece::View(_) => (point_node(view_at, s), Rot2::IDENTITY),
        };
        if node.left != wanted.left || node.top != wanted.top || node.width != wanted.width || node.height != wanted.height {
            (node.left, node.top, node.width, node.height) = (wanted.left, wanted.top, wanted.width, wanted.height);
        }
        if turn.rotation != rotation {
            turn.rotation = rotation;
        }
    }
    // The dropdown of a slot: every part of the game it takes but the one
    // in it, and the view that empties it (`DropDownMenu.Show`).
    if state.drawn.dropdown_for != Some(state.menu) {
        state.drawn.dropdown_for = Some(state.menu);
        let Some(layer_entity) = state.drawn.dropdown else { return };
        commands.entity(layer_entity).despawn_related::<Children>();
        let found = state.menu.and_then(|menu| state.slots.iter().position(|slot| (slot.owner, slot.index) == menu));
        if let (Some(at), Some(parts)) = (found, if state.gear { state.gear_parts.clone() } else { state.parts.clone() }) {
            let slot = state.slots[at].clone();
            let current = slot.part.and_then(|id| build.find(id)).map(|part| part.kind.template.clone());
            // (`GetItemCollections`: by the slot's filter; each then asked
            // whether it would go in, `SelectCheck`, and shown dim with the
            // reason where not.)
            let offered: Vec<(Arc<Kind>, Option<String>)> = parts
                .iter()
                .filter(|kind| Some(&kind.template) != current.as_ref() && rules::takes(build, slot.owner, slot.index, kind))
                .map(|kind| (kind.clone(), rules::refusal(build, slot.owner, slot.index, kind).map(|refusal| crate::screens::inventory_view::why(&refusal, &carried.texts))))
                .collect();
            let view_at = state.placed.get(at).copied().flatten().map_or([wide / 2.0, HIGH / 2.0], |(_, view)| view);
            state.scroll = 0.0;
            commands.entity(layer_entity).with_children(|layer| match game_look {
                Some((eb, prepared)) => dropdown_game(layer, &look, eb, prepared, &carried, &mut icons, (slot.owner, slot.index), &offered, view_at, &said, s, wide),
                None => dropdown_app(layer, &carried, &mut icons, (slot.owner, slot.index), &offered, view_at, s, wide),
            });
        }
    }
    // The characteristics panel at the bottom left (`CharacteristicsPanel.
    // Show(Item, _compareItem)`, at every change of the build and of what
    // it is compared with), its bars' comparison pulsing.
    let stats_for = (state.made, state.compare_made, state.stats_folded);
    if state.drawn.stats_for != Some(stats_for) {
        state.drawn.stats_for = Some(stats_for);
        if let Some(layer_entity) = state.drawn.stats {
            commands.entity(layer_entity).despawn_related::<Children>();
            state.drawn.pulse.clear();
            let say = |key: &str| carried.texts.say(key);
            let figures = trp_sim::items::item_attributes::figures(build, &game.db, &say);
            let hovered = compared_build(build, state.compare.as_ref());
            let compare_item = hovered.as_ref().unwrap_or(build);
            let compared = trp_sim::items::item_attributes::figures(compare_item, &game.db, &say);
            let weight_name = say("Weight").unwrap_or_else(|| "Weight".to_owned()).to_uppercase();
            let weights = (build.weight(), compare_item.weight());
            let built = game_look.and_then(|(_, prepared)| crate::screens::health_view::eb_stats_built(&look, &figures, &compared, weights, &weight_name, !state.stats_folded).zip(prepared.stats));
            match (built, game_look) {
                (Some((built, top)), Some((_, prepared))) => {
                    // Where the panel lies on the screen: its own anchors
                    // and pivot, at the size its rows make it.
                    let mut screen = prepared.screen.clone();
                    screen.nodes[top].layout.size_delta = built.size;
                    let [x, y, ..] = screen.rect(top, [wide, HIGH]);
                    commands.entity(layer_entity).with_children(|layer| {
                        crate::screens::health_view::draw_eb_stats(layer, &look, &said, &built, [x, y], s);
                        for [sx, sy, sw, sh] in &built.switches {
                            layer.spawn((Button, Press::Info, rect_node([x + sx, y + sy, *sw, *sh], s)));
                        }
                    });
                    state.drawn.pulse = built.pulse.clone();
                }
                _ => {
                    commands.entity(layer_entity).with_children(|layer| stats_app(layer, &figures, &compared, weights, &weight_name, state.stats_folded, s));
                }
            }
        }
    }
    if !state.drawn.pulse.is_empty() {
        let alpha = crate::screens::health_view::compare_alpha(time.elapsed_secs());
        for (piece, mut image) in &mut stats_images {
            if let Some((_, color)) = state.drawn.pulse.iter().find(|(index, _)| crate::screens::health_view::EB_STATS + index == piece.0) {
                let wanted = color.with_alpha(alpha);
                if image.color != wanted {
                    image.color = wanted;
                }
            }
        }
    }
    // The notice (`NotificationManager`), over the bottom's middle.
    let notice = state.notice.as_ref().map(|(said, _)| said.clone());
    if state.drawn.notice_for.as_ref() != Some(&notice) {
        let Some(layer_entity) = state.drawn.notice else { return };
        commands.entity(layer_entity).despawn_related::<Children>();
        if let Some(said) = &notice {
            let row = Node { position_type: PositionType::Absolute, left: Val::Px(0.0), width: Val::Percent(100.0), top: Val::Px(1000.0 * s), justify_content: JustifyContent::Center, ..default() };
            commands.entity(layer_entity).with_children(|layer| {
                layer.spawn(row).with_children(|row| {
                    row.spawn((Node { padding: UiRect::axes(Val::Px(20.0 * s), Val::Px(8.0 * s)), ..default() }, BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.8)))).with_children(|plate| {
                        plate.spawn(words(said.clone(), 18.0 * s, INK));
                    });
                });
            });
        }
        state.drawn.notice_for = Some(notice);
    }
}

/// The characteristics in the app's own look: at the bottom left, a line
/// each (the durability, the weight, then the figures' rows), a difference
/// from the build compared with in brackets after a bar's figure; folded to
/// its heading, which folds and unfolds it.
fn stats_app(layer: &mut ChildSpawnerCommands, figures: &trp_sim::items::item_attributes::Figures, compared: &trp_sim::items::item_attributes::Figures, (weight, compare_weight): (f32, f32), weight_name: &str, folded: bool, s: f32) {
    use trp_sim::items::item_attributes::Show;
    let size = 13.0 * s;
    let column = Node { position_type: PositionType::Absolute, left: Val::Px(30.0 * s), bottom: Val::Px(40.0 * s), flex_direction: FlexDirection::Column, row_gap: Val::Px(3.0 * s), padding: UiRect::all(Val::Px(8.0 * s)), ..default() };
    layer.spawn((column, BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)))).with_children(|column| {
        let heading = if folded { "INFO  >" } else { "INFO  <" };
        column.spawn((Button, Press::Info, Node::default())).with_children(|button| {
            button.spawn(words(heading.to_owned(), size, INK));
        });
        if folded {
            return;
        }
        let mut lines = Vec::new();
        if let Some(durability) = figures.durability {
            lines.push(format!("{}   {:.0}/{:.0} ({:.0})", figures.durability_name, durability.now, durability.most, durability.peak));
        }
        let difference = compare_weight - weight;
        lines.push(if difference.abs() > 1e-4 { format!("{weight_name}   {weight:.3} ({}{difference:.3})", if difference > 0.0 { "+" } else { "" }) } else { format!("{weight_name}   {weight:.3}") });
        for row in &figures.rows {
            let mut value = row.value.clone().unwrap_or_default();
            if let Show::Bar { base, delta, .. } = row.show {
                let other = compared.rows.iter().find(|other| other.id == row.id).and_then(|other| match other.show {
                    Show::Bar { base, delta, .. } => Some(base * delta),
                    _ => None,
                });
                if let Some(more) = other.map(|change| change - base * delta).filter(|more| more.abs() > f32::EPSILON) {
                    value.push_str(&format!(" ({}{})", if more > 0.0 { "+" } else { "" }, more.round() as i64));
                }
            }
            lines.push(if value.is_empty() { row.name.clone() } else { format!("{}   {value}", row.name) });
        }
        for line in lines {
            column.spawn(words(line, size, INK));
        }
    });
}
