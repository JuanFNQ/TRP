//! Simple and lined tips made, placed, hidden and drawn.

use super::*;

/// The simple tooltip made (`SimpleTooltip` as its prefab lays it out):
/// its ground as wide as its words and its padding, at most `widest`
/// units, the words broken into lines within it where they are wider
/// (its `HorizontalLayoutGroup` giving the ground no more than the
/// `LayoutElement`'s width), as high as its words and its padding. Its size
/// in pixels, none where the game's face is not had (then as the app's
/// layout makes it, the next frame). Words that begin with an armour
/// class's mark (an armour plate's: `ArmorPlatesFormatter.
/// FormatArmorPlateTooltip`) have it drawn apart, where the tooltip of lines
/// draws its marks in a row, and their first line put on by its room
/// (`MARK_EMS`). `PROVISIONAL`: the simple tooltip's mark is not in a take.
fn make_simple(commands: &mut Commands, look: &HealthLook, tip: &TipLook, said: &str, s: f32, place: &dyn Fn(Vec2) -> Vec2) -> (Entity, Vec2, Option<Kept>) {
    let [pl, pr, pt, pb] = tip.padding.map(|units| units * s);
    let widest = tip.widest * s;
    let SimpleLaid { mark, said, lead, size } = lay_simple(look, tip, said, s);
    match size {
        Some((size, wrap)) => {
            let (root, pictures) = ground(commands, tip, "SimpleTooltip/Background", place(size), size);
            let mut words = None;
            commands.entity(root).with_children(|ground| {
                if let Some(sprite) = mark.and_then(|class| look.class_mark(class)) {
                    let [w, h] = [sprite.size[0] * MARK_PER_PIXEL * s, sprite.size[1] * MARK_PER_PIXEL * s];
                    let node = Node { position_type: PositionType::Absolute, left: Val::Px(pl + MARK_IN * s), top: Val::Px(pt - MARK_UP * s), width: Val::Px(w), height: Val::Px(h), ..default() };
                    ground.spawn((Name::new("Mark"), node, ImageNode { image: sprite.image.clone(), rect: Some(sprite.cut), flip_y: true, ..default() }));
                }
                words = words_at(ground, look, &tip.words, said, Vec2::new(pl, pt), Some(size.x - pl - pr), wrap, lead, s);
            });
            let kept = words.zip(tip_shape(s, mark, true)).map(|((words, text), shape)| Kept { shape, frame: pictures, words, text });
            (root, size, kept)
        }
        None => {
            let node = Node {
                position_type: PositionType::Absolute,
                max_width: Val::Px(widest),
                padding: UiRect { left: Val::Px(pl), right: Val::Px(pr), top: Val::Px(pt), bottom: Val::Px(pb) },
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            };
            let root = commands.spawn((TipRoot, Name::new("SimpleTooltip/Background"), node, BackgroundColor(Color::BLACK), BorderColor::all(tip.frame), GlobalZIndex(TIP_Z), Visibility::Hidden)).id();
            let font = TextFont { font: tip.words.font.clone(), font_size: tip.words.size * s, ..default() };
            let spans = plain_spans(said);
            commands.entity(root).with_children(|ground| {
                ground.spawn((Name::new("Text"), Text::default(), font.clone(), TextColor(tip.words.color))).with_children(|text| {
                    for (said, tint) in spans {
                        let color = tint.map_or(tip.words.color, |c| Color::srgba(c[0], c[1], c[2], c[3]));
                        text.spawn((TextSpan::new(said), font.clone(), TextColor(color)));
                    }
                });
            });
            (root, Vec2::ZERO, None)
        }
    }
}

/// How a simple tooltip lays out its words: the armour class's mark they
/// begin with and the words after it, the room the mark takes before the
/// first line, and, where the game's face is had, the tooltip's size in
/// pixels and the width its words break at where they are wider than it
/// takes.
struct SimpleLaid<'a> {
    mark: Option<u32>,
    said: &'a str,
    lead: f32,
    size: Option<(Vec2, Option<f32>)>,
}

fn lay_simple<'a>(look: &HealthLook, tip: &TipLook, said: &'a str, s: f32) -> SimpleLaid<'a> {
    let [pl, pr, pt, pb] = tip.padding.map(|units| units * s);
    let widest = tip.widest * s;
    let (mark, said) = leading_mark(said).map_or((None, said), |(class, rest)| (Some(class), rest));
    let lead = if mark.is_some() { MARK_EMS * tip.words.size * s } else { 0.0 };
    let size = measure(look, &tip.words, said, s, None, lead).map(|one| {
        if one.x + pl + pr <= widest {
            (Vec2::new(one.x + pl + pr, one.y + pt + pb), None)
        } else {
            let inside = widest - pl - pr;
            let high = measure(look, &tip.words, said, s, Some(inside), lead).map_or(one.y, |wrapped| wrapped.y);
            (Vec2::new(widest, high + pt + pb), Some(inside))
        }
    });
    SimpleLaid { mark, said, lead, size }
}

/// New words taken by the simple tooltip shown, where they have its shape
/// (`tip_shape`), as the game's one tooltip takes them (`SimpleTooltip.
/// SetText`, then its layout sizing the ground to them): its entities kept,
/// the new text written into its words' entity and, where their size is not
/// the last words', its ground, its words' box and its frame's pictures
/// sized to them (`resize_frame`). False where they have not its
/// shape (it is then made afresh).
fn swap_simple(commands: &mut Commands, look: &HealthLook, tip: &TipLook, said: &str, s: f32, shown: &mut Shown, roots: &mut Roots) -> bool {
    let laid = lay_simple(look, tip, said, s);
    let (Some(kept), Some(root), Some((size, wrap))) = (shown.kept.as_mut(), shown.root, laid.size) else { return false };
    if tip_shape(s, laid.mark, true).as_ref() != Some(&kept.shape) {
        return false;
    }
    let (Some(text), Ok((mut node, ..))) = (tmp_text(look, &tip.words, laid.said, s, wrap, laid.lead), roots.get_mut(root)) else { return false };
    commands.entity(kept.text).insert(text);
    if size != shown.size {
        let [pl, pr, pt, _] = tip.padding.map(|units| units * s);
        (node.width, node.height) = (Val::Px(size.x), Val::Px(size.y));
        commands.entity(kept.words).insert(words_node(Vec2::new(pl, pt), Some(size.x - pl - pr)));
        kept.frame = resize_frame(commands, tip, root, kept.frame.take(), size);
        shown.size = size;
    }
    true
}

/// The tooltip of lines made (`MultiLineTooltip`, `MultiLineRow`, as their
/// prefab lays them out): its header, empty (a line's words alone), then
/// each line its padding's spacing below the one before: its words as high
/// as their line, after its armour class's mark or the room for one (with
/// a space between, as `MultiLineInfo` writes them); a break as its line
/// across the whole tooltip. As wide as its widest line and its padding.
fn make_lines(commands: &mut Commands, look: &HealthLook, tip: &TipLook, lines: &[TipLine], s: f32, place: &dyn Fn(Vec2) -> Vec2) -> (Entity, Vec2, Option<Kept>) {
    let [pl, pr, pt, pb] = tip.padding.map(|units| units * s);
    let gap = tip.spacing * s;
    let em = tip.words.size * s;
    let line_high = 100.0 / SEPARATOR_PER_UNIT * s;
    // Each line: its words (none for a break), the room before them, its
    // size.
    let laid: Vec<(Option<String>, f32, Vec2)> = lines
        .iter()
        .map(|line| {
            if line.divider {
                return (None, 0.0, Vec2::new(0.0, line_high));
            }
            let (before, said) = match (line.class, line.indent) {
                (Some(_), _) => (MARK_EMS * em, format!(" {}", line.text)),
                (None, true) => (INDENT_EMS * em, format!(" {}", line.text)),
                (None, false) => (0.0, line.text.clone()),
            };
            let size = measure(look, &tip.words, &said, s, None, 0.0).unwrap_or(Vec2::new(said.chars().count() as f32 * em * 0.5, em * 1.1));
            (Some(said), before, Vec2::new(before + size.x, size.y))
        })
        .collect();
    let inside = laid.iter().map(|(.., size)| size.x).fold(0.0f32, f32::max);
    let high = pt + pb + laid.iter().map(|(.., size)| gap + size.y).sum::<f32>();
    let size = Vec2::new(inside + pl + pr, high);
    let (root, _) = ground(commands, tip, "MultiLineTooltip/Background", place(size), size);
    commands.entity(root).with_children(|ground| {
        let mut y = pt;
        for ((said, before, row), line) in laid.iter().zip(lines) {
            y += gap;
            match said {
                Some(said) => {
                    ground.spawn((Name::new("MultiLineRow(Clone)"), root_node(Vec2::new(pl, y), *row), EXACT)).with_children(|row_node| {
                        // The mark where the take's picture has it.
                        if let Some(mark) = line.class.and_then(|class| look.class_mark(class)) {
                            let [w, h] = [mark.size[0] * MARK_PER_PIXEL * s, mark.size[1] * MARK_PER_PIXEL * s];
                            let node = Node { position_type: PositionType::Absolute, left: Val::Px(MARK_IN * s), top: Val::Px(-MARK_UP * s), width: Val::Px(w), height: Val::Px(h), ..default() };
                            row_node.spawn((Name::new("Mark"), node, ImageNode { image: mark.image.clone(), rect: Some(mark.cut), flip_y: true, ..default() }));
                        }
                        words_at(row_node, look, &tip.words, said, Vec2::new(*before, 0.0), None, None, 0.0, s);
                    });
                }
                None => {
                    let rect = [pl, y, inside, row.y];
                    match &tip.separator {
                        Some(separator) => {
                            ground.spawn((Name::new("Line"), root_node(Vec2::new(rect[0], rect[1]), Vec2::new(rect[2], rect[3])), EXACT)).with_children(|frame| {
                                crate::screens::health_view::draw_sprite(frame, separator, [0.0, 0.0, rect[2], rect[3]], s * 100.0 / SEPARATOR_PER_UNIT);
                            });
                        }
                        None => {
                            ground.spawn((Name::new("Line"), root_node(Vec2::new(rect[0], rect[1]), Vec2::new(rect[2], rect[3])), EXACT, BackgroundColor(tip.frame)));
                        }
                    }
                }
            }
            y += row.y;
        }
    });
    (root, size, None)
}

/// The tooltips' roots: where each lies, whether it is shown, and its size
/// as the layout left it.
pub(super) type Roots<'w, 's> = Query<'w, 's, (&'static mut Node, &'static mut Visibility, &'static ComputedNode), With<TipRoot>>;

/// Makes a tooltip: its root, its size in pixels (none where the layout
/// sizes it) and what of it is kept for new words.
pub(super) type Make<'a> = dyn FnMut(&mut Commands, &dyn Fn(Vec2) -> Vec2) -> (Entity, Vec2, Option<Kept>) + 'a;

/// Takes new words into the tooltip shown, true where it could.
pub(super) type Swap<'a> = dyn FnMut(&mut Commands, &mut Shown, &mut Roots) -> bool + 'a;

/// Puts a tooltip by the mouse; where what it shows has changed (`made`),
/// its new words taken in place (`swap`) or else made afresh; its root
/// shown.
pub(super) fn place_tip(commands: &mut Commands, shown: &mut Shown, roots: &mut Roots, made: String, (cursor, offset, screen, s): (Vec2, Vec2, Vec2, f32), make: &mut Make, swap: &mut Swap) {
    let laid = shown.laid;
    // (Made afresh too where its root has gone: it is never asked for on
    // the frame it is made.)
    let gone = shown.root.is_none_or(|root| roots.get(root).is_err());
    let mut placed_by = None;
    if shown.made.as_ref() != Some(&made) || gone {
        if !gone && swap(commands, shown, roots) {
            shown.made = Some(made);
            // Placed by the size the layout last left it, as made afresh.
            placed_by = Some(laid);
        } else {
            if let Some(root) = shown.root.take() {
                commands.entity(root).despawn();
            }
            // Placed by the size the layout last left it (the last words'),
            // where it was shown before.
            let place = |size: Vec2| corner(cursor, offset, if laid == Vec2::ZERO { size } else { laid }, size, screen, s);
            let (root, size, kept) = make(commands, &place);
            shown.root = Some(root);
            shown.made = Some(made);
            shown.size = size;
            shown.kept = kept;
            if size != Vec2::ZERO {
                shown.laid = size;
            }
            return;
        }
    }
    let Some(root) = shown.root else { return };
    let Ok((mut node, mut visibility, computed)) = roots.get_mut(root) else { return };
    let size = if shown.size != Vec2::ZERO { shown.size } else { computed.size() * computed.inverse_scale_factor() };
    let at = corner(cursor, offset, placed_by.unwrap_or(size), size, screen, s);
    let (left, top) = (Val::Px(at.x), Val::Px(at.y));
    if node.left != left || node.top != top {
        (node.left, node.top) = (left, top);
    }
    if *visibility != Visibility::Inherited {
        *visibility = Visibility::Inherited;
    }
    shown.laid = size;
}

/// Hides a tooltip (`Tooltip.Close`), kept for its next showing.
fn hide(shown: &Shown, roots: &mut Roots) {
    if let Some(Ok((_, mut visibility, _))) = shown.root.map(|root| roots.get_mut(root)) {
        if *visibility != Visibility::Hidden {
            *visibility = Visibility::Hidden;
        }
    }
}

/// The simple tooltip shown as it was asked for on this frame: once its
/// key has been asked for its delay running (a frame with none asked, or
/// another key, starts it again: `Tooltip.Close` kills the wait), with its
/// words; else hidden.
pub(super) fn draw_simple_tip(
    mut commands: Commands,
    mut tip: ResMut<SimpleTip>,
    look: Res<HealthLook>,
    time: Res<Time>,
    window: Query<&Window, With<PrimaryWindow>>,
    mut roots: Roots,
) {
    let tip = &mut *tip;
    let want = tip.want.take();
    let anew = std::mem::take(&mut tip.anew);
    let now = time.elapsed_secs();
    let since = match (&want, tip.since) {
        (Some((key, ..)), Some((was, since))) if *key == was && !anew => since,
        (Some((key, ..)), _) => {
            tip.since = Some((*key, now));
            now
        }
        (None, _) => {
            tip.since = None;
            now
        }
    };
    let mouse = window.single().ok().and_then(|window| Some((window.cursor_position()?, Vec2::new(window.width(), window.height()))));
    let Some(((_, said, _, offset), (cursor, screen))) = want.filter(|(_, _, delay, _)| now - since >= *delay).zip(mouse) else {
        hide(&tip.shown, &mut roots);
        return;
    };
    let s = screen.y / HIGH;
    let simple = look.simple_tip_look();
    place_tip(
        &mut commands,
        &mut tip.shown,
        &mut roots,
        format!("{s}|{said}"),
        (cursor, offset, screen, s),
        &mut |commands, place| make_simple(commands, &look, &simple, &said, s, place),
        &mut |commands, shown, roots| swap_simple(commands, &look, &simple, &said, s, shown, roots),
    );
}

/// The tooltip of lines shown as it was asked for on this frame, else
/// hidden.
pub(super) fn draw_lines_tip(
    mut commands: Commands,
    mut tip: ResMut<LinesTip>,
    look: Res<HealthLook>,
    window: Query<&Window, With<PrimaryWindow>>,
    mut roots: Roots,
) {
    let tip = &mut *tip;
    let mouse = window.single().ok().and_then(|window| Some((window.cursor_position()?, Vec2::new(window.width(), window.height()))));
    let Some((lines, (cursor, screen))) = tip.want.take().zip(mouse) else {
        hide(&tip.shown, &mut roots);
        return;
    };
    let s = screen.y / HIGH;
    let multi = look.lines_tip_look();
    // Made afresh as its lines change (no screen changes them while shown).
    place_tip(&mut commands, &mut tip.shown, &mut roots, format!("{s}|{lines:?}"), (cursor, Vec2::ZERO, screen, s), &mut |commands, place| make_lines(commands, &look, &multi, &lines, s, place), &mut |_, _, _| false);
}
