//! The tip's corner, runs, marks, measures, ground and words.

use super::*;

/// Where a tooltip's box lies, its top left in pixels, y down
/// (`Tooltip.SetPosition`, `CorrectPositionResolution`): its lower left
/// `offset` units right of and above the mouse, moved back onto the screen
/// from the edge it crosses (the left's checked before the right's, the
/// bottom's before the top's), then its top left corner put on a whole
/// pixel (`Math.Round`: to the even one at a half). It is placed by its
/// size as the layout last left it (`laid`), and the layout then gives it
/// its own size keeping its lower left: on the frame its words change, a
/// pixel's fraction off where the next frame puts it (`courses10`'s
/// `hover-tooltips`, frames 214 and 215).
pub(crate) fn corner(cursor: Vec2, offset: Vec2, laid: Vec2, size: Vec2, screen: Vec2, s: f32) -> Vec2 {
    // Its lower left, y up.
    let mut x = cursor.x + offset.x * s;
    let mut y = screen.y - cursor.y + offset.y * s;
    if x < 0.0 {
        x = 0.0;
    } else if x + laid.x > screen.x {
        x = screen.x - laid.x;
    }
    if y < 0.0 {
        y = 0.0;
    } else if y + laid.y > screen.y {
        y = screen.y - laid.y;
    }
    let (left, top) = (x.round_ties_even(), (y + laid.y).round_ties_even());
    Vec2::new(left, screen.y - (top - laid.y + size.y))
}

/// Words with their colour and bold tags as TextMeshPro's runs, each in its
/// colour or the text's own.
fn runs(words: &TipWords, said: &str, s: f32) -> Vec<trp_data::interface::tmp_text::TmpRun> {
    let base = crate::screens::health_view::screen_numbers(words.color);
    let spans = crate::screens::health_view::rich_runs(said);
    spans.into_iter().map(|(text, tint, bold)| trp_data::interface::tmp_text::TmpRun { text, size: words.size * s, color: tint.unwrap_or(base), bold }).collect()
}

/// The words' layout, the first line's first letter `lead` pixels in.
fn layout(words: &TipWords, s: f32, wrap: Option<f32>, lead: f32) -> trp_data::interface::tmp_text::TmpLayout {
    trp_data::interface::tmp_text::TmpLayout { em: words.size * s, spacing: words.spacing, kerning: words.kerning, wrap, align: 1, lead }
}

/// The armour class's mark words begin with (`<sprite
/// name="armor_classes_N">`: `ArmorPlatesFormatter.FormatArmorClassIcon`),
/// and the words after it.
pub(super) fn leading_mark(said: &str) -> Option<(u32, &str)> {
    let rest = said.strip_prefix("<sprite name=\"armor_classes_")?;
    let (class, rest) = rest.split_once("\">")?;
    Some((class.parse().ok()?, rest))
}

/// A tag-free copy of the words in runs of their colours, for the font
/// where the game's face is not had (bold not drawn).
pub(super) fn plain_spans(said: &str) -> Vec<(String, Option<[f32; 4]>)> {
    crate::screens::health_view::rich_runs(said).into_iter().map(|(text, tint, _)| (text, tint)).collect()
}

/// The game's face of the words, where it is had.
fn face<'a>(look: &'a HealthLook, words: &TipWords) -> Option<(&'a trp_data::interface::tmp_text::TmpFace, (String, i64), (String, i64))> {
    let (asset, material) = words.tmp.clone()?;
    Some((look.tmp_faces.get(&asset)?.as_ref(), asset, material))
}

/// The words' width and height as TextMeshPro lays them out, in pixels,
/// the first line `lead` pixels in.
pub(crate) fn measure(look: &HealthLook, words: &TipWords, said: &str, s: f32, wrap: Option<f32>, lead: f32) -> Option<Vec2> {
    let (face, ..) = face(look, words)?;
    Some(Vec2::from(trp_data::interface::tmp_text::measure(face, &runs(words, said, s), &layout(words, s, wrap, lead))))
}

/// A tooltip's ground (`Background`, black), at `at` and that size, with its
/// frame's shadow and its frame over it; or the app's frame, a pixel wide in
/// the frame's colour, where the game's pictures are not had. Its root and
/// the box of its pictures (`frame`).
pub(super) fn ground(commands: &mut Commands, look: &TipLook, name: &'static str, at: Vec2, size: Vec2) -> (Entity, Option<FramePieces>) {
    let node = Node { position_type: PositionType::Absolute, left: Val::Px(at.x), top: Val::Px(at.y), width: Val::Px(size.x), height: Val::Px(size.y), ..default() };
    let mut root = commands.spawn((TipRoot, Name::new(name), node, EXACT, BackgroundColor(Color::BLACK), GlobalZIndex(TIP_Z)));
    if look.border.is_none() {
        root.insert((Node { border: UiRect::all(Val::Px(1.0)), ..root_node(at, size) }, BorderColor::all(look.frame)));
    }
    let root = root.id();
    let pictures = frame(commands, look, size);
    if let Some(pictures) = &pictures {
        commands.entity(root).add_child(pictures.0);
    }
    (root, pictures)
}

/// A tooltip's frame as drawn: the box holding its pictures, and each
/// picture's box with its pieces.
pub(super) type FramePieces = (Entity, Vec<(Entity, Vec<Entity>)>);

/// A tooltip's frame's pictures for a ground of that size: its shadow and
/// its frame over it, each a box where it lies and its pieces in it.
fn frame_layout(look: &TipLook, size: Vec2) -> Vec<(&'static str, Node, Vec<(Node, ImageNode)>)> {
    let mut pictures = Vec::new();
    if let Some((shadow, [l, t, r, b])) = &look.shadow {
        let whole = Vec2::new(size.x + l + r, size.y + t + b);
        pictures.push(("Border Shadow", root_node(Vec2::new(-l, -t), whole), crate::screens::health_view::sprite_pixel_pieces(shadow, whole.to_array())));
    }
    if let Some(border) = &look.border {
        pictures.push(("Border", root_node(Vec2::ZERO, size), crate::screens::health_view::sprite_pixel_pieces(border, size.to_array())));
    }
    pictures
}

/// A tooltip's frame's shadow and its frame over it, as large as its ground
/// is, in a box of their own at the ground's top left (its first child,
/// under the words), kept as drawn so that new words of another size size
/// them again (`resize_frame`); none where neither picture is had.
pub(super) fn frame(commands: &mut Commands, look: &TipLook, size: Vec2) -> Option<FramePieces> {
    let layout = frame_layout(look, size);
    if layout.is_empty() {
        return None;
    }
    let holder = commands.spawn((Name::new("Frame"), root_node(Vec2::ZERO, size), EXACT)).id();
    let mut boxes = Vec::new();
    for (name, node, pieces) in layout {
        let picture = commands.spawn((Name::new(name), node, EXACT)).id();
        let pieces: Vec<Entity> = pieces.into_iter().map(|(node, image)| commands.spawn((node, image, EXACT)).id()).collect();
        commands.entity(picture).add_children(&pieces);
        commands.entity(holder).add_child(picture);
        boxes.push((picture, pieces));
    }
    Some((holder, boxes))
}

/// A tooltip's frame sized again to its ground's new size, its pieces kept
/// and moved (a piece made anew is drawn a frame after it is made, and the
/// frame then blinked as the words changed); made afresh only where the new
/// size has other pieces. The frame as it is after.
pub(super) fn resize_frame(commands: &mut Commands, look: &TipLook, root: Entity, drawn: Option<FramePieces>, size: Vec2) -> Option<FramePieces> {
    let layout = frame_layout(look, size);
    let alike = drawn.as_ref().is_some_and(|(_, boxes)| boxes.len() == layout.len() && boxes.iter().zip(&layout).all(|((_, pieces), (.., laid))| pieces.len() == laid.len()));
    match drawn {
        Some((holder, boxes)) if alike => {
            commands.entity(holder).insert(root_node(Vec2::ZERO, size));
            for ((picture, pieces), (_, node, laid)) in boxes.iter().zip(layout) {
                commands.entity(*picture).insert(node);
                for (piece, (node, _)) in pieces.iter().zip(laid) {
                    commands.entity(*piece).insert(node);
                }
            }
            Some((holder, boxes))
        }
        drawn => {
            if let Some((holder, _)) = drawn {
                commands.entity(holder).despawn();
            }
            let pictures = frame(commands, look, size);
            if let Some((holder, _)) = &pictures {
                commands.entity(root).insert_children(0, &[*holder]);
            }
            pictures
        }
    }
}

pub(super) fn root_node(at: Vec2, size: Vec2) -> Node {
    Node { position_type: PositionType::Absolute, left: Val::Px(at.x), top: Val::Px(at.y), width: Val::Px(size.x), height: Val::Px(size.y), ..default() }
}

/// The words as the game draws them, where its face is had.
pub(super) fn tmp_text(look: &HealthLook, words: &TipWords, said: &str, s: f32, wrap: Option<f32>, lead: f32) -> Option<TmpText> {
    let (_, asset, material) = face(look, words)?;
    Some(TmpText { asset, material, runs: runs(words, said, s), layout: layout(words, s, wrap, lead) })
}

/// The box that words drawn at a place in a tooltip are laid in.
pub(super) fn words_node(at: Vec2, width: Option<f32>) -> Node {
    Node { position_type: PositionType::Absolute, left: Val::Px(at.x), top: Val::Px(at.y), width: width.map_or(Val::Auto, Val::Px), ..default() }
}

/// Words drawn at a place in a tooltip, as the game draws them where its
/// face is had (then their box's entity and their text's), else in the font.
#[allow(clippy::too_many_arguments)]
pub(super) fn words_at(parent: &mut ChildSpawnerCommands, look: &HealthLook, words: &TipWords, said: &str, at: Vec2, width: Option<f32>, wrap: Option<f32>, lead: f32, s: f32) -> Option<(Entity, Entity)> {
    let node = words_node(at, width);
    match tmp_text(look, words, said, s, wrap, lead) {
        Some(text) => {
            let mut drawn = None;
            let laid = crate::screens::health_view::tmp_words(parent, node, text, &mut |text| {
                drawn = Some(text.insert(Name::new("Text")).id());
            });
            drawn.map(|drawn| (laid, drawn))
        }
        None => {
            let font = TextFont { font: words.font.clone(), font_size: words.size * s, ..default() };
            let line = if wrap.is_some() { TextLayout::new_with_linebreak(bevy::text::LineBreak::WordBoundary) } else { TextLayout::new_with_no_wrap() };
            let spans = plain_spans(said);
            parent.spawn(node).with_children(|frame| {
                frame.spawn((Name::new("Text"), Text::default(), font.clone(), TextColor(words.color), line)).with_children(|text| {
                    for (said, tint) in spans {
                        let color = tint.map_or(words.color, |c| Color::srgba(c[0], c[1], c[2], c[3]));
                        text.spawn((TextSpan::new(said), font.clone(), TextColor(color)));
                    }
                });
            });
            None
        }
    }
}
