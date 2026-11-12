//! Nine-slices, marks, frames and the nodes drawn.

use super::*;

/// A sliced picture: its texture and cut, its border (left, bottom,
/// right, top) in the sprite's pixels, the screen's pixels to one of
/// them, whether its middle is drawn, and its colour.
pub(super) struct Slices {
    pub(super) image: Handle<Image>,
    pub(super) cut: Rect,
    pub(super) border: [f32; 4],
    pub(super) per: f32,
    pub(super) fill_center: bool,
    pub(super) color: Color,
    /// Mirrored across its frame's middle: each piece where its mirror
    /// image lies, its picture flipped.
    pub(super) flip_x: bool,
    /// Its edges and middle repeated at the sprite's size, not stretched
    /// (`Image.Type.Tiled`).
    pub(super) tiled: bool,
}

/// One piece of a sliced picture, by the drawn object it is of.
#[derive(Component)]
pub(super) struct SliceOf(pub(super) usize);

/// A sliced picture in a frame of that size, in pixels, as nine pieces:
/// the corners whole, the edges and the middle stretched (the texture
/// kept as the engine has it, its lowest row first, and drawn turned up).
pub(super) fn nine_slice(frame: &mut ChildSpawnerCommands, slices: &Slices, size: [f32; 2], index: usize) {
    for (row, column, node, piece) in slice_pieces(slices, size) {
        if let (true, Some(rect)) = (slices.tiled && (row == 1 || column == 1), piece.rect) {
            let tile = [rect.width() * slices.per, rect.height() * slices.per];
            if let Some(inner) = tiles_grown(&node, tile, [column == 1, row == 1], slices.flip_x) {
                frame.spawn(Node { overflow: Overflow::clip(), ..node }).with_children(|clip| {
                    clip.spawn((SliceOf(index), inner, piece));
                });
                continue;
            }
        }
        frame.spawn((SliceOf(index), node, piece));
    }
}

/// The pieces `nine_slice` draws, each by its row and column (the top's
/// and the left's first), where it lies in the frame and its picture.
pub(super) fn slice_pieces(slices: &Slices, size: [f32; 2]) -> Vec<(usize, usize, Node, ImageNode)> {
    let mut pieces = Vec::new();
    let [l, b, r, t] = slices.border;
    let (x0, y0, x1, y1) = (slices.cut.min.x, slices.cut.min.y, slices.cut.max.x, slices.cut.max.y);
    let (mut left, mut right, mut top, mut bottom) = (l * slices.per, r * slices.per, t * slices.per, b * slices.per);
    if left + right > size[0] && left + right > 0.0 {
        let k = size[0] / (left + right);
        (left, right) = (left * k, right * k);
    }
    if top + bottom > size[1] && top + bottom > 0.0 {
        let k = size[1] / (top + bottom);
        (top, bottom) = (top * k, bottom * k);
    }
    let columns = [(x0, x0 + l, 0.0, left), (x0 + l, x1 - r, left, size[0] - left - right), (x1 - r, x1, size[0] - right, right)];
    let rows = [(y1 - t, y1, 0.0, top), (y0 + b, y1 - t, top, size[1] - top - bottom), (y0, y0 + b, size[1] - bottom, bottom)];
    // A middle with no pixels of its own (a sprite all border, as the
    // skills' bars' `level_fill_bar`, 8 by 8 with 4 all round) is the
    // texture at the line between its borders, stretched: the engine's
    // sliced mesh gives its middle that line's coordinates. A pixel's
    // width about the line stands for it.
    let widened = |a: f32, b: f32| if b - a < 1e-3 { ((a + b) / 2.0 - 0.5, (a + b) / 2.0 + 0.5) } else { (a, b) };
    for (row, (ya, yb, at_y, h)) in rows.into_iter().enumerate() {
        for (column, (xa, xb, at_x, w)) in columns.into_iter().enumerate() {
            if (row == 1 && column == 1 && !slices.fill_center) || w <= 0.0 || h <= 0.0 || xb < xa || yb < ya {
                continue;
            }
            let ((xa, xb), (ya, yb)) = (widened(xa, xb), widened(ya, yb));
            let at_x = if slices.flip_x { size[0] - at_x - w } else { at_x };
            let node = Node { position_type: PositionType::Absolute, left: Val::Px(at_x), top: Val::Px(at_y), width: Val::Px(w), height: Val::Px(h), ..default() };
            let mut piece = ImageNode { image: slices.image.clone(), color: slices.color, rect: Some(Rect::new(xa, ya, xb, yb)), flip_x: slices.flip_x, flip_y: true, ..default() };
            if slices.tiled && (row == 1 || column == 1) {
                piece.image_mode = NodeImageMode::Tiled { tile_x: column == 1, tile_y: row == 1, stretch_value: slices.per };
            }
            pieces.push((row, column, node, piece));
        }
    }
    pieces
}

/// A picture repeated at its own size over a rectangle, laid from the
/// rectangle's lower left as the engine lays it (`Image.GenerateTiledSprite`:
/// whole tiles up and right from the corner, the last row and column cut),
/// where Bevy lays its tiles from the top left. Where the rectangle is not
/// a whole number of tiles high (a grid of 63-unit cells is one unit more,
/// for its last line), the caller draws the picture in a clip of the
/// rectangle (its `Node` with `Overflow::clip()`) at the place returned:
/// grown upward to whole tiles, so that the cut row is the top one; a
/// mirrored one likewise grown to the left. `None` where the tiles fit as
/// Bevy lays them.
fn tiles_grown(node: &Node, tile: [f32; 2], tiles: [bool; 2], flip_x: bool) -> Option<Node> {
    let (Val::Px(w), Val::Px(h)) = (node.width, node.height) else { return None };
    let whole = |extent: f32, tile: f32, on: bool| if on && tile > 0.5 { ((extent / tile) - 1e-3).ceil().max(1.0) * tile } else { extent };
    let (grown_w, grown_h) = (if flip_x { whole(w, tile[0], tiles[0]) } else { w }, whole(h, tile[1], tiles[1]));
    if (grown_w - w).abs() < 0.01 && (grown_h - h).abs() < 0.01 {
        return None;
    }
    Some(Node { position_type: PositionType::Absolute, left: Val::Px(w - grown_w), top: Val::Px(h - grown_h), width: Val::Px(grown_w), height: Val::Px(grown_h), ..default() })
}

/// What of a panel is drawn as its scripts have it: its rates' arrows,
/// the views' glows, the headers whose texts are keys to the install's
/// words; objects shown or hidden as the caller has them (`set`), those
/// not drawn with all under them (`skip`), and texts the caller writes
/// (`words`).
#[derive(Default)]
pub(crate) struct Marks<'a> {
    pub(crate) arrows: &'a [usize],
    pub(crate) glows: &'a [usize],
    pub(crate) headers: &'a [usize],
    pub(crate) set: Option<&'a HashMap<usize, bool>>,
    pub(crate) skip: &'a [usize],
    pub(crate) words: Option<&'a HashMap<usize, String>>,
    /// Only what lies in these objects (themselves among it), where any
    /// are named; and none of what lies under these.
    pub(crate) within: &'a [usize],
    pub(crate) skip_below: &'a [usize],
    /// Pictures the caller puts on objects (a slot's empty picture), drawn
    /// at their own size about the object's middle (`Image.SetNativeSize`).
    pub(crate) pictures: Option<&'a HashMap<usize, Picture>>,
    /// Sprites the caller swaps into objects (`Image.sprite` and no more: a
    /// menu line's icon), drawn in the object's own rectangle, kept to the
    /// sprite's shape where the image keeps its shape.
    pub(crate) sprites: Option<&'a HashMap<usize, Picture>>,
    /// Images the caller has made for objects, drawn over the whole of
    /// each in place of what the object draws (a tab's fill cut to its
    /// mask, `MaskedFill`).
    pub(crate) fills: Option<&'a HashMap<usize, Handle<Image>>>,
    /// The alpha a script puts on objects' pictures (`ChangeImageAlpha`: a
    /// filter that does not work at 0.15).
    pub(crate) alphas: Option<&'a HashMap<usize, f32>>,
    /// The alpha a script puts on objects' groups (`CanvasGroup.alpha`: a
    /// tab that does not work at 0.3, `SetUnlockStatus`), which every
    /// picture and text in the object or under it is drawn at besides.
    pub(crate) groups: Option<&'a HashMap<usize, f32>>,
    /// The colour a script puts on objects' pictures or texts (a quick
    /// view's ground and caption for the thing in the hands,
    /// `QuickSlotView.SwitchVisualSelection`), and texts it sets to the
    /// normal style (`fontStyle = Normal`).
    pub(crate) colors: Option<&'a HashMap<usize, Color>>,
    pub(crate) unbold: &'a [usize],
}

/// How a panel's top lies: by its own anchors in what it hangs from, so
/// large (the left panel), or as a rectangle of that size.
#[derive(Clone, Copy)]
pub(crate) enum Frame {
    In([f32; 2]),
    Is([f32; 2]),
}

/// Each object of one of the game's panels that shows, where the game lays
/// it out in a left panel of that size, its picture as the engine draws it
/// and its text, numbered from `base` among the drawn pieces.
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_nodes(column: &mut ChildSpawnerCommands, look: &HealthLook, panel: &trp_data::interface::screen::ScenePanel, drawn: &Drawn, base: usize, marks: Marks, said: &Words, frame: Frame, origin: [f32; 2], s: f32) {
    let at = |x: f32, y: f32, w: f32, h: f32| Node {
        position_type: PositionType::Absolute,
        left: Val::Px((origin[0] + x) * s),
        top: Val::Px((origin[1] + y) * s),
        width: Val::Px(w * s),
        height: Val::Px(h * s),
        ..default()
    };
    for (index, node) in panel.nodes.iter().enumerate() {
        let mut visibility = match drawn.shown.get(index) {
            None | Some(Shown::Never) => continue,
            Some(Shown::Always) => Visibility::Inherited,
            Some(Shown::ByScript) => Visibility::Hidden,
        };
        // Left out with all under it, or shown as the caller has it; hidden
        // with all under it where the caller hides something above it (an
        // object off hides what hangs from it).
        let mut above = Some(index);
        let (mut skipped, mut inside, mut hidden) = (false, marks.within.is_empty(), false);
        while let Some(at) = above {
            skipped |= marks.skip.contains(&at) || (at != index && marks.skip_below.contains(&at));
            inside |= marks.within.contains(&at);
            hidden |= at != index && marks.set.and_then(|set| set.get(&at)) == Some(&false);
            above = panel.nodes[at].parent;
        }
        if skipped || !inside {
            continue;
        }
        let mut group = 1.0;
        let mut above = Some(index);
        while let Some(at) = above {
            group *= marks.groups.and_then(|groups| groups.get(&at)).copied().unwrap_or(1.0);
            above = panel.nodes[at].parent;
        }
        if let Some(&on) = marks.set.and_then(|set| set.get(&index)) {
            visibility = if on { Visibility::Inherited } else { Visibility::Hidden };
        }
        if hidden {
            visibility = Visibility::Hidden;
        }
        let piece = Piece(base + index);
        let mut placed = match frame {
            Frame::In(outer) => panel.placed_in(index, outer),
            Frame::Is(size) => panel.placed(index, size),
        };
        // A picture whose border is kept a pixel to a pixel
        // (`PixelPerfectSpriteScaler.UpdateScale`): its offsets from the
        // edges of what it hangs from, on the sides it scales
        // (`_sidesToScale`, all unless set), divided by the screen's scale,
        // so it lies that many pixels out, not units (a quick-use view's
        // shadowed frame, five pixels about it).
        if let (Some((_, fields)), Some(parent)) = (node.scripts.iter().find(|(class, _)| class == "PixelPerfectSpriteScaler"), node.parent) {
            let sides = fields.get(..4).and_then(|b| b.try_into().ok()).map_or(0xF, i32::from_le_bytes);
            let [px, py, pw, ph] = match frame {
                Frame::In(outer) => panel.placed_in(parent, outer).rect,
                Frame::Is(size) => panel.placed(parent, size).rect,
            };
            let [x, y, w, h] = placed.rect;
            let (mut left, mut top, mut right, mut bottom) = (x, y, x + w, y + h);
            let k = 1.0 / s.max(1e-3);
            if sides & 1 != 0 {
                top = py - (py - top) * k;
            }
            if sides & 2 != 0 {
                left = px - (px - left) * k;
            }
            if sides & 4 != 0 {
                bottom = py + ph + (bottom - (py + ph)) * k;
            }
            if sides & 8 != 0 {
                right = px + pw + (right - (px + pw)) * k;
            }
            placed.rect = [left, top, right - left, bottom - top];
        }
        if let Some(own) = marks.pictures.and_then(|pictures| pictures.get(&index)) {
            let [x, y, w, h] = placed.rect;
            placed.rect = [x + (w - own.size[0]) / 2.0, y + (h - own.size[1]) / 2.0, own.size[0], own.size[1]];
        }
        // A simple picture that keeps its sprite's shape
        // (`Image.preserveAspect`): narrowed or lowered to it, moved by the
        // object's pivot (`PreserveSpriteAspectRatio`). The sprite is the one
        // swapped in, where one is.
        let sprite = marks.sprites.and_then(|sprites| sprites.get(&index).copied()).or_else(|| drawn.pictures.get(index).copied().flatten());
        if let Some(picture) = node.image.filter(|image| image.preserve_aspect && image.kind == 0).and(sprite) {
            let [x, y, w, h] = placed.rect;
            let (sprite, rect) = (picture.size[0] / picture.size[1].max(1e-3), w / h.max(1e-3));
            let pivot = node.layout.pivot;
            placed.rect = if sprite > rect {
                let height = w / sprite;
                [x, y + (h - height) * (1.0 - pivot[1]), w, height]
            } else {
                let width = h * sprite;
                [x + (w - width) * pivot[0], y, width, h]
            };
        }
        // Turned with what it hangs from (a slider's handle, turned to
        // point at its bar, and the arrow under it): its middle turned
        // about the pivot of each turned object above it, nearest first,
        // and it turned as much besides.
        let mut turn = node.layout.turn;
        let mut above = node.parent;
        while let Some(at) = above {
            let layout = panel.nodes[at].layout;
            if layout.turn.abs() > 1e-3 {
                let [ax, ay, aw, ah] = match frame {
                    Frame::In(outer) => panel.placed_in(at, outer).rect,
                    Frame::Is(size) => panel.placed(at, size).rect,
                };
                let pivot = [ax + layout.pivot[0] * aw, ay + (1.0 - layout.pivot[1]) * ah];
                let [x, y, w, h] = placed.rect;
                let (dx, dy) = (x + w / 2.0 - pivot[0], y + h / 2.0 - pivot[1]);
                let (sin, cos) = layout.turn.to_radians().sin_cos();
                placed.rect = [pivot[0] + dx * cos + dy * sin - w / 2.0, pivot[1] - dx * sin + dy * cos - h / 2.0, w, h];
                turn += layout.turn;
            }
            above = panel.nodes[at].parent;
        }
        let [x, y, w, h] = placed.rect;
        // Over a button of the game's the pointer takes its hover look
        // (`cursor::point`).
        if let Some(resets) = crate::platform::cursor::hover_of(&node.scripts) {
            column.spawn((crate::platform::cursor::HoverPointer(base + index, resets), at(x, y, w, h), bevy::ui::RelativeCursorPosition::default(), visibility));
        }
        if let Some(handle) = marks.fills.and_then(|fills| fills.get(&index)) {
            let fill = ImageNode { flip_x: placed.flip[0], ..ImageNode::new(handle.clone()).with_color(Color::WHITE.with_alpha(group)) };
            column.spawn((piece, at(x, y, w, h), EXACT, fill, visibility));
            continue;
        }
        // A mask that does not show its own picture (`Mask.showMaskGraphic`
        // off) draws nothing; what hangs from it is cut to it.
        let hidden_mask = node.scripts.iter().any(|(class, fields)| class == "Mask" && fields.first() == Some(&0));
        if let Some(image) = node.image.as_ref().filter(|_| !hidden_mask) {
            let mut color = marks.colors.and_then(|colors| colors.get(&index)).copied().unwrap_or_else(|| rgba(image.color));
            if let Some(&alpha) = marks.alphas.and_then(|alphas| alphas.get(&index)) {
                color = color.with_alpha(alpha);
            }
            if let (true, Some(alpha)) = (marks.glows.contains(&index), look.glow_alpha) {
                color = color.with_alpha(alpha);
            }
            color = color.with_alpha(color.alpha() * group);
            let own = marks.pictures.and_then(|pictures| pictures.get(&index).copied()).or_else(|| marks.sprites.and_then(|sprites| sprites.get(&index).copied()));
            let picture = own.or_else(|| drawn.pictures.get(index).copied().flatten()).and_then(|picture| Some((picture, look.images.get(picture.texture)?.clone()?)));
            // (A sprite a script puts on it counts as its own: a slot's
            // picture on an image kept with none.)
            match (image.sprite.or(own.map(|_| (0, 0))), picture) {
                // No sprite: the engine's plain white, coloured.
                (None, _) => {
                    column.spawn((piece, at(x, y, w, h), EXACT, BackgroundColor(color), visibility));
                }
                (Some(_), Some((picture, handle))) => {
                    // The cut's share of the rectangle, mirrored with it.
                    let mut c = picture.covers;
                    if placed.flip[0] {
                        c = [1.0 - c[2], c[1], 1.0 - c[0], c[3]];
                    }
                    if placed.flip[1] {
                        c = [c[0], 1.0 - c[3], c[2], 1.0 - c[1]];
                    }
                    let (dx, dy, dw, dh) = (x + w * c[0], y + h * (1.0 - c[3]), w * (c[2] - c[0]), h * (c[3] - c[1]));
                    let mut drawn = ImageNode { image: handle.clone(), color, rect: Some(picture.cut), flip_x: placed.flip[0], flip_y: !placed.flip[1], ..default() };
                    // Sliced: nine pieces, the border kept whole and its
                    // middle left out unless the image fills it
                    // (`Image.fillCenter`); the border a pixel to a pixel
                    // of the screen where the object keeps it so
                    // (`PixelPerfectSpriteScaler`), else a unit to a pixel
                    // of the sprite over its multiplier.
                    let [l, b, r, t] = picture.border;
                    // Tiled with a border (`Image.GenerateTiledSprite`):
                    // as sliced, the border whole, but its edges and its
                    // middle repeated at their own size, not stretched (the
                    // quick-use panel's header, its slanted end in its
                    // right border).
                    if (image.kind == 1 || image.kind == 2) && (l > 0.0 || b > 0.0 || r > 0.0 || t > 0.0) {
                        let pixel_perfect = node.scripts.iter().any(|(class, _)| class == "PixelPerfectSpriteScaler");
                        let per = if pixel_perfect { 1.0 } else { s / image.pixels_per_unit.max(1e-3) };
                        let slices = Slices { image: handle, cut: picture.cut, border: picture.border, per, fill_center: image.fill_center, color, flip_x: placed.flip[0], tiled: image.kind == 2 };
                        column.spawn((piece, at(dx, dy, dw, dh), EXACT, visibility)).with_children(|frame| nine_slice(frame, &slices, [dw * s, dh * s], base + index));
                        continue;
                    }
                    // Tiled: the sprite repeated at its own size, a unit
                    // to a pixel.
                    if image.kind == 2 {
                        drawn.image_mode = NodeImageMode::Tiled { tile_x: true, tile_y: true, stretch_value: s };
                    }
                    // Filled across from the left (`Image.Type.Filled`,
                    // horizontal, from the left): a clip as wide as the
                    // share filled, the picture whole inside it.
                    let grown = match drawn.image_mode {
                        NodeImageMode::Tiled { .. } if turn.abs() <= 1e-3 && !marks.arrows.contains(&index) => {
                            let tile = [picture.cut.width() * s, picture.cut.height() * s];
                            tiles_grown(&at(dx, dy, dw, dh), tile, [true, true], placed.flip[0])
                        }
                        _ => None,
                    };
                    if let Some(inner) = grown {
                        let clip = Node { overflow: Overflow::clip(), ..at(dx, dy, dw, dh) };
                        column.spawn((piece, clip, EXACT, visibility)).with_children(|clip| {
                            clip.spawn((SliceOf(base + index), inner, EXACT, drawn));
                        });
                    } else if image.kind == 3 && image.fill_method == 0 {
                        let clip = Node { overflow: Overflow::clip(), ..at(dx, dy, dw * image.fill_amount, dh) };
                        column.spawn((piece, FullWidth(dw * s), clip, EXACT, visibility)).with_children(|clip| {
                            let whole = Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), width: Val::Px(dw * s), height: Val::Px(dh * s), ..default() };
                            clip.spawn((PieceFill(base + index), whole, drawn));
                        });
                    } else if marks.arrows.contains(&index) {
                        let unit = if placed.flip[1] { -s } else { s };
                        column.spawn((piece, Bob((origin[1] + dy) * s, unit), at(dx, dy, dw, dh), EXACT, drawn, visibility));
                    } else {
                        // Turned about its middle where the scene turns it
                        // (the sheath's picture, 45 degrees): the engine's
                        // counter-clockwise, Bevy's clockwise.
                        let mut piece = column.spawn((piece, at(dx, dy, dw, dh), EXACT, drawn, visibility));
                        if turn.abs() > 1e-3 {
                            piece.insert(UiTransform { rotation: Rot2::degrees(-turn), ..UiTransform::IDENTITY });
                        }
                    }
                }
                // A sprite that could not be read: left out.
                (Some(_), None) => {}
            }
        }
        if let Some(text) = &node.text {
            let justify = match text.horizontal {
                2 => JustifyContent::Center,
                4 => JustifyContent::FlexEnd,
                _ => JustifyContent::FlexStart,
            };
            let align = match text.vertical {
                256 => AlignItems::FlexStart,
                1024 => AlignItems::FlexEnd,
                _ => AlignItems::Center,
            };
            // A view's header says what the install's words have for its
            // own text (`LocalizedText`); every other text is as the scene
            // has it until a script writes to it.
            let mut said_now = match marks.words.and_then(|words| words.get(&index)) {
                Some(words) => words.clone(),
                None if marks.headers.contains(&index) => said.say(&text.text, &text.text),
                None => text.text.clone(),
            };
            if text.style & 16 != 0 {
                said_now = said_now.to_uppercase();
            }
            // In the game's own font where it is had.
            let style = if marks.unbold.contains(&index) { text.style & !1 } else { text.style };
            let font = TextFont { font: look.styled_font(panel, text.font, style), font_size: text.size * s, weight: weight_of(style), ..default() };
            let color = marks.colors.and_then(|colors| colors.get(&index)).copied().unwrap_or_else(|| rgba(text.color));
            let color = TextColor(color.with_alpha(color.alpha() * group));
            let frame = Node { justify_content: justify, align_items: align, ..at(x, y, w, h) };
            // (Its rim or shadow at the group's alpha as well.)
            let effect = look.text_effect(panel, text.font).map(|effect| effect.faded(group));
            let size = text.size * s;
            // As the game draws it, where its font asset is had: what a
            // script puts before the words (a figure's noughts), then the
            // words.
            // Words with TextMeshPro's colour tags in them (`<color=#...>`,
            // a localized format's figure) as runs of their colours.
            let spans = rich_spans(&said_now);
            if let Some((asset, material)) = look.tmp_of(panel, text) {
                let bold = style & 1 != 0;
                let run = |said: String| trp_data::interface::tmp_text::TmpRun { text: said, size, color: screen_numbers(color.0), bold };
                let layout = trp_data::interface::tmp_text::TmpLayout { em: text.size * s, spacing: text.spacing, kerning: text.kerning, wrap: None, align: text.horizontal & 7, lead: 0.0 };
                let runs = match spans {
                    Some(spans) => std::iter::once(run(String::new()))
                        .chain(spans.into_iter().map(|(said, tint)| {
                            let tinted = tint.map_or(color.0, |c| Color::srgba(c[0], c[1], c[2], c[3] * group));
                            trp_data::interface::tmp_text::TmpRun { text: said, size, color: screen_numbers(tinted), bold }
                        }))
                        .collect(),
                    None => vec![run(String::new()), run(said_now)],
                };
                let words = TmpText { asset, material, runs, layout };
                column.spawn((piece, frame, visibility)).with_children(|frame| {
                    tmp_words(frame, Node::default(), words, &mut |entity| {
                        entity.insert(PieceText(base + index));
                    });
                });
            } else {
                let runs: Vec<(String, TextFont, Color)> = match spans {
                    Some(spans) => spans.into_iter().map(|(said, tint)| (said, font.clone(), tint.map_or(color.0, |c| Color::srgba(c[0], c[1], c[2], c[3] * group)))).collect(),
                    None => vec![(said_now, font, color.0)],
                };
                column.spawn((piece, frame, visibility)).with_children(|frame| {
                    effect_words(frame, Node::default(), &runs, effect, size, &mut |entity, run, _| {
                        match run {
                            None => entity.insert(PieceText(base + index)),
                            Some(_) => entity.insert(PieceSpan(base + index)),
                        };
                    });
                });
            }
        }
    }
}
