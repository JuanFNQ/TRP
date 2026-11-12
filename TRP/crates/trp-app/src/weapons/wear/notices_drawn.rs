//! Notices shown and spawned, and the glow shown.

use super::*;

/// How high a notice's box is when open, in units: its words as
/// TextMeshPro lays them out, broken within its text's width, and the
/// text's margins above and below (`LayoutRebuilder.ForceRebuildLayoutImmediate`
/// in `BaseNotificationView.Init`: a line, 26.61 units, `courses10`'s
/// 35.48 pixels at 1440 lines), where the game's face of them is had.
fn notice_full(notice: &trp_data::interface::screen::NoticeLook, health_look: &crate::screens::health_view::HealthLook, words: &str) -> Option<f32> {
    let text = notice.panel.nodes[notice.text].text.as_ref()?;
    let (asset, _) = health_look.tmp_of(&notice.panel, text)?;
    let face = health_look.tmp_faces.get(&asset)?;
    let width = notice.panel.nodes[notice.text].layout.size_delta[0] - text.margin[0] - text.margin[2];
    let runs = [trp_data::interface::tmp_text::TmpRun { text: words.to_owned(), size: text.size, color: [1.0; 4], bold: text.style & 1 != 0 }];
    let layout = trp_data::interface::tmp_text::TmpLayout { em: text.size, spacing: text.spacing, kerning: text.kerning, wrap: Some(width), align: 1, lead: 0.0 };
    let [_, high] = trp_data::interface::tmp_text::measure(face, &runs, &layout);
    Some(high + text.margin[1] + text.margin[3])
}

/// Shows every notice asked for at once, each with the notices' sound, as
/// the notifier lays out its template at the screen's lower right (its box
/// as wide as the scene's, the words wrapped within it after its left
/// padding, the icon in that padding), the newest at the foot of the
/// column and those before it pushed up, each row's padding below it; each
/// row opening as it comes and folding as it goes, its box cut to it (its
/// `RectMask2D`: the words, top left in their margins, and the icon, in
/// its middle, cut as it opens), and faded in and out.
#[allow(clippy::too_many_arguments)]
pub(super) fn show_notices(
    mut commands: Commands,
    mut feedback: ResMut<Feedback>,
    looks: Res<Looks>,
    health_look: Res<crate::screens::health_view::HealthLook>,
    window: Query<&Window, With<bevy::window::PrimaryWindow>>,
    time: Res<Time<Real>>,
    drawn: Query<(Entity, &NoticeRoot)>,
    mut boxes: Query<(&NoticeBox, &mut Node, &mut BackgroundColor)>,
    frames: Query<(&NoticeFrame, &ComputedNode)>,
    mut icons: Query<(&NoticeIcon, &mut ImageNode)>,
    mut words: Query<(&NoticeWords, Option<&mut TextColor>, Option<&mut ImageNode>), Without<NoticeIcon>>,
    mut sounds: MessageWriter<crate::audio::ui_sound::Sound>,
    mut fulls: Local<std::collections::HashMap<u64, f32>>,
) {
    let now = time.elapsed_secs_f64();
    let feedback = &mut *feedback;
    while let Some((said, alert)) = feedback.waiting.pop_front() {
        feedback.shown_count += 1;
        let number = feedback.shown_count;
        if let Some(full) = looks.notice.as_ref().and_then(|notice| notice_full(notice, &health_look, &said)) {
            fulls.insert(number, full);
        }
        info!("a notice shown: {said}");
        feedback.shown.push(Notice { words: said, alert, began: now - time.delta_secs_f64(), fold: None, number });
        sounds.write(crate::audio::ui_sound::Sound::Notification);
    }
    for notice in &mut feedback.shown {
        if notice.fold.is_none() && now - notice.began >= NOTICE_SHOW + NOTICE_DONE {
            notice.fold = Some(now);
        }
    }
    feedback.shown.retain(|notice| notice.fold.is_none_or(|at| now - at < NOTICE_FOLD));
    fulls.retain(|number, _| feedback.shown.iter().any(|notice| notice.number == *number));
    let height = window.single().map_or(0, |window| window.height().round() as u32);
    for (entity, root) in &drawn {
        if root.1 != height || !feedback.shown.iter().any(|notice| notice.number == root.0) {
            commands.entity(entity).despawn();
        }
    }
    let Some(look) = looks.notice.as_ref() else { return };
    if height == 0 {
        return;
    }
    let s = height as f32 / 1080.0;
    // From the column's foot up (the bottom panel's padding under it), the
    // newest first: each row its box and its padding below it. A box's
    // height when open is its words' as the game lays them out, else as
    // the app's laid them out on the frame before.
    let padding = look.panel.nodes[0].group.map_or(2.0, |group| group.padding[3]);
    let laid: std::collections::HashMap<u64, f32> = frames.iter().map(|(frame, computed)| (frame.0, computed.size().y * computed.inverse_scale_factor() / s)).collect();
    let mut foot = look.below - padding;
    let mut places = std::collections::HashMap::new();
    for notice in feedback.shown.iter().rev() {
        let full = fulls.get(&notice.number).or_else(|| laid.get(&notice.number)).copied().unwrap_or(0.0) + padding;
        let row = notice_row(full, now - notice.began, notice.fold.map(|at| now - at));
        places.insert(notice.number, (foot + padding, notice_box(row, padding)));
        foot += row;
    }
    for notice in &feedback.shown {
        if !drawn.iter().any(|(_, root)| *root == NoticeRoot(notice.number, height)) {
            info!("a notice drawn: {}", notice.words);
            let icon = looks.icons[usize::from(!notice.alert)].as_ref();
            let place = places.get(&notice.number).copied().unwrap_or((look.below, 0.0));
            spawn_notice(&mut commands, notice, NoticeRoot(notice.number, height), place, look, icon, &health_look);
        }
    }
    let alphas: std::collections::HashMap<u64, (f32, f32)> = feedback.shown.iter().map(|notice| (notice.number, notice_alphas(now - notice.began))).collect();
    for (owner, mut node, mut ground) in &mut boxes {
        let (Some((bottom, high)), Some((group, _))) = (places.get(&owner.0), alphas.get(&owner.0)) else { continue };
        let (bottom, high) = (Val::Px(bottom * s), Val::Px(high * s));
        if node.bottom != bottom || node.height != high {
            (node.bottom, node.height) = (bottom, high);
        }
        ground.0 = Color::srgba(NOTICE_GROUND[0], NOTICE_GROUND[1], NOTICE_GROUND[2], NOTICE_GROUND[3] * group);
    }
    for (owner, mut image) in &mut icons {
        if let Some((group, icon)) = alphas.get(&owner.0) {
            image.color = Color::srgba(1.0, 1.0, 1.0, icon * group);
        }
    }
    for (owner, color, image) in &mut words {
        let Some((group, _)) = alphas.get(&owner.0) else { continue };
        if let Some(mut color) = color {
            color.0 = Color::srgba(NOTICE_WORDS[0], NOTICE_WORDS[1], NOTICE_WORDS[2], NOTICE_WORDS[3] * group);
        }
        if let Some(mut image) = image {
            image.color = Color::srgba(1.0, 1.0, 1.0, *group);
        }
    }
}

/// A notice's box at its place (its foot over the screen's and its height,
/// in units), its parts laid out in it as the template has them.
fn spawn_notice(commands: &mut Commands, notice: &Notice, root: NoticeRoot, (bottom, high): (f32, f32), look: &trp_data::interface::screen::NoticeLook, icon: Option<&(Handle<Image>, Rect, [f32; 2])>, health_look: &crate::screens::health_view::HealthLook) {
    let s = root.1 as f32 / 1080.0;
    let number = notice.number;
    let said = notice.words.clone();
    let content = &look.panel.nodes[look.content].layout;
    let icon_layout = &look.panel.nodes[look.icon].layout;
    let text = look.panel.nodes[look.text].text.as_ref();
    let text_layout = &look.panel.nodes[look.text].layout;
    let [left, right, top, below] = look.padding;
    let whole = Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() };
    // Its words from its top (their margin above them), not in its middle:
    // the text is as high as the box and lays its words out from its top
    // left, so that they come up with the box's top as it opens.
    let plate = Node {
        position_type: PositionType::Absolute,
        right: Val::Px(0.0),
        bottom: Val::Px(bottom * s),
        width: Val::Px(content.size_delta[0] * s),
        height: Val::Px(high * s),
        padding: UiRect { left: Val::Px(left * s), right: Val::Px(right * s), top: Val::Px(top * s), bottom: Val::Px(below * s) },
        flex_direction: FlexDirection::Row,
        align_items: AlignItems::FlexStart,
        overflow: Overflow::clip(),
        ..default()
    };
    let ground = Color::srgba(NOTICE_GROUND[0], NOTICE_GROUND[1], NOTICE_GROUND[2], NOTICE_GROUND[3]);
    commands.spawn((root, whole, GlobalZIndex(45))).with_children(|layer| {
        layer.spawn((NoticeBox(number), plate, BackgroundColor(ground))).with_children(|plate| {
            // The icon, hung in the left padding, its right edge so far in,
            // kept in its shape within its square (`preserveAspect`), in
            // the box's middle.
            if let Some((image, cut, [w, h])) = icon {
                let side = icon_layout.size_delta;
                let fit = (side[0] / w.max(1.0)).min(side[1] / h.max(1.0));
                let [w, h] = [w * fit, h * fit];
                let right_edge = icon_layout.offset[0] + side[0] * (1.0 - icon_layout.pivot[0]);
                let node = Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px((right_edge - side[0] + (side[0] - w) / 2.0) * s),
                    top: Val::Percent(50.0),
                    margin: UiRect { top: Val::Px(-h / 2.0 * s), ..default() },
                    width: Val::Px(w * s),
                    height: Val::Px(h * s),
                    ..default()
                };
                plate.spawn((NoticeIcon(number), node, ImageNode { image: image.clone(), color: Color::srgba(1.0, 1.0, 1.0, 0.0), rect: Some(*cut), flip_y: true, ..default() }));
            }
            let (size, margin, font) = match text {
                Some(text) => (text.size, text.margin, health_look.styled_font(&look.panel, text.font, text.style)),
                None => (16.0, [0.0, 4.0, 0.0, 5.0], Handle::default()),
            };
            let frame = Node {
                width: Val::Px(text_layout.size_delta[0] * s),
                flex_shrink: 0.0,
                padding: UiRect { left: Val::Px(margin[0] * s), top: Val::Px(margin[1] * s), right: Val::Px(margin[2] * s), bottom: Val::Px(margin[3] * s) },
                ..default()
            };
            let color = Color::srgba(NOTICE_WORDS[0], NOTICE_WORDS[1], NOTICE_WORDS[2], NOTICE_WORDS[3]);
            // As the game draws them where its face is had (its first
            // line's top at the margin: the app's own text lay 7.3 pixels
            // lower, `courses10`'s `examine-multiple`, frame 230), faded by
            // their picture's alpha.
            let tmp = text.and_then(|text| Some((text, health_look.tmp_of(&look.panel, text)?)));
            plate.spawn((NoticeFrame(number), frame)).with_children(|frame| match tmp {
                Some((text, (asset, material))) => {
                    let run = trp_data::interface::tmp_text::TmpRun { text: said, size: size * s, color: crate::screens::health_view::screen_numbers(color), bold: text.style & 1 != 0 };
                    let wrap = (text_layout.size_delta[0] - margin[0] - margin[2]) * s;
                    let layout = trp_data::interface::tmp_text::TmpLayout { em: size * s, spacing: text.spacing, kerning: text.kerning, wrap: Some(wrap), align: 1, lead: 0.0 };
                    crate::screens::health_view::tmp_words(frame, Node::default(), crate::screens::health_view::TmpText { asset, material, runs: vec![run], layout }, &mut |image| {
                        image.insert(NoticeWords(number));
                    });
                }
                None => {
                    frame.spawn((NoticeWords(number), Text::new(said), TextFont { font, font_size: size * s, ..default() }, TextColor(color), TextLayout::new_with_linebreak(LineBreak::WordBoundary)));
                }
            });
        });
    });
}

/// The glow drawn: its kind's number, when it began, the window's height.
#[derive(Component, PartialEq)]
pub(super) struct GlowRoot(i32, u64, u32);

/// The glow (`ProcessGlow`): its kind's picture where the scene places the
/// glow (the screen's lower right corner, past its edges), its alpha the
/// kind's curve by the wall clock's seconds since it began times its
/// factor, until the curve's last key.
pub(super) fn show_glow(
    mut commands: Commands,
    feedback: Res<Feedback>,
    looks: Res<Looks>,
    health_look: Res<crate::screens::health_view::HealthLook>,
    window: Query<&Window, With<bevy::window::PrimaryWindow>>,
    time: Res<Time<Real>>,
    drawn: Query<(Entity, &GlowRoot, &MaterialNode<GlowMaterial>)>,
    mut materials: ResMut<Assets<GlowMaterial>>,
) {
    let now = time.elapsed_secs_f64();
    let number = |glow: Glow| match glow {
        Glow::Examined => 0,
        Glow::Repaired => 1,
        Glow::TypeExamined => 2,
    };
    let shown = feedback.glow.and_then(|(glow, at, factor)| {
        let (_, image, cut, curve) = looks.glows.iter().find(|(kind, ..)| *kind == number(glow))?;
        let t = (now - at) as f32;
        (t <= curve.keys.last().map_or(0.0, |key| key.time)).then(|| (number(glow), at, image, cut, factor * curve.evaluate(t)))
    });
    let Ok(window) = window.single() else { return };
    let height = window.height().round() as u32;
    let wanted = shown.as_ref().map(|(kind, at, ..)| GlowRoot(*kind, at.to_bits(), height));
    let gamma = if health_look.tarkov_ui { 1.0 } else { 0.0 };
    if drawn.iter().map(|(_, root, _)| root).next() != wanted.as_ref() {
        for (entity, ..) in &drawn {
            commands.entity(entity).despawn();
        }
        let (Some(root), Some((_, _, image, cut, alpha)), Some(panel)) = (wanted, shown.as_ref(), looks.glow_panel.as_ref()) else { return };
        let s = height as f32 / 1080.0;
        let [x, y, w, h] = panel.placed_in(0, [window.width() / s, 1080.0]).rect;
        let material = materials.add(GlowMaterial { alpha: Vec4::new(*alpha, gamma, 0.0, 0.0), cut: Vec4::new(cut.min.x, cut.min.y, cut.max.x, cut.max.y), texture: (*image).clone() });
        let node = Node { position_type: PositionType::Absolute, left: Val::Px(x * s), top: Val::Px(y * s), width: Val::Px(w * s), height: Val::Px(h * s), ..default() };
        info!("the malfunction glow drawn: kind {}", root.0);
        commands.spawn((root, node, MaterialNode(material), GlobalZIndex(44)));
        return;
    }
    if let (Some((.., alpha)), Some((_, _, handle))) = (shown, drawn.iter().next()) {
        if let Some(material) = materials.get_mut(&handle.0) {
            material.alpha.x = alpha;
        }
    }
}
