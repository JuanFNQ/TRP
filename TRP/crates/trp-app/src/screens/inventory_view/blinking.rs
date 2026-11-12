//! Pictures filled, blinking, taken things dimmed, and slot and grid boxes drawn.

use super::*;

/// Turns the spinners, and puts in each one's place the picture that has
/// come for it, or the plain shape of its sort where none can be made:
/// the box and everything else on the screen stay as they are.
#[allow(clippy::too_many_arguments)]
pub(super) fn fill_pictures(
    mut commands: Commands,
    time: Res<Time<Real>>,
    carried: Res<Carried>,
    features: Res<crate::screens::options_view::Features>,
    mut arrived: ResMut<PicturesArrived>,
    mut materials: ResMut<Assets<crate::ui::radial_fill::RadialFill>>,
    mut shared: Local<Vec<(SpinnerPicture, Handle<crate::ui::radial_fill::RadialFill>)>>,
    fresh: Query<(Entity, &SpinnerPicture), (Without<MaterialNode<crate::ui::radial_fill::RadialFill>>, Without<SearchSpinner>)>,
    spinners: Query<(Entity, &Loading, &ChildOf)>,
    drains: Query<(), With<DrainSpinner>>,
    kids: Query<&Children>,
) {
    // A spinner is turned inside its upright node, by its picture's
    // material, never by turning the node: Bevy cuts a turned node at a
    // clip's edge as if it were not turned, pulling it out of shape (a box
    // half out of a scrolled panel), and a
    // box cuts what is in it, so a loader whose corners reached past its
    // box's edges could not turn at all (a medical case's small things,
    // 0.104.2). Upright, it is cut as any picture is.
    for (entity, picture) in &fresh {
        let handle = match shared.iter().find(|(had, _)| had == picture) {
            Some((_, handle)) => handle.clone(),
            None => {
                let handle = materials.add(crate::ui::radial_fill::RadialFill::new(picture.0.clone(), picture.1, picture.2, features.tarkov_ui, picture.3));
                shared.push((picture.clone(), handle.clone()));
                handle
            }
        };
        // (A spinner whose view is drawn afresh on this frame by another
        // system, the battle screen's quick-use bar as its picture comes,
        // is gone by the time this is done: nothing is done to it.)
        commands.entity(entity).try_insert(MaterialNode(handle));
    }
    if !spinners.is_empty() || !drains.is_empty() {
        let turn = (time.elapsed_secs() * SPINNER_TURNS).fract();
        for (_, handle) in shared.iter() {
            if let Some(material) = materials.get_mut(handle) {
                material.set_turn(turn);
            }
        }
    }
    let come = std::mem::take(&mut arrived.0);
    for (entity, loading, parent) in &spinners {
        if !come.contains(&loading.key) {
            continue;
        }
        // (One that cannot be made keeps its spinner.)
        let Some(picture) = carried.pictures.get(&loading.key) else { continue };
        // In its loader's own place among the box's children, as the view
        // was built (`picture_in`): its words after it, drawn over it; a
        // thing not known's noise and colour panel before it, under it.
        // Put first, a picture drawn afresh would lie under
        // the noise and the panel, and a thing not known showed a light
        // grey until a move built its view anew.
        // (Put in within its box, so that a box drawn afresh on this frame
        // by another system takes nothing.)
        let box_ = parent.parent();
        let at = kids.get(box_).ok().and_then(|children| children.iter().position(|child| child == entity)).unwrap_or(0);
        match loading.quick {
            Some((place, tint)) => put_in(&mut commands, box_, at, quick_picture_node(picture, place, tint)),
            None => put_in(&mut commands, box_, at, picture_node(picture, loading.room, loading.turned)),
        }
        commands.entity(entity).try_despawn();
    }
}

/// A node made and put among a box's children at that place, where the box
/// is still there when the commands are carried out.
pub(super) fn put_in(commands: &mut Commands, box_: Entity, at: usize, bundle: impl Bundle) {
    commands.entity(box_).queue_silenced(move |mut held: EntityWorldMut| {
        let shown = held.world_scope(|world| world.spawn(bundle).id());
        held.insert_children(at, &[shown]);
    });
}

/// How a blinking box's picture shows, `since` seconds after it began, as
/// the game's (`ItemViewAnimation.CG_StartBlinking`): faded out to nothing
/// over half a second and back over the next, round and round, each half a
/// tween with the tweening library's default easing (out, quadratic).
fn blink_alpha(since: f32) -> f32 {
    let out_quad = |x: f32| 1.0 - (1.0 - x) * (1.0 - x);
    let into = since.rem_euclid(1.0);
    if into < 0.5 { 1.0 - out_quad(into / 0.5) } else { out_quad((into - 0.5) / 0.5) }
}

/// Half a blink, seconds (`CG_StartBlinking`: each `DOColor` 0.5 s).
const BLINK_HALF: f32 = 0.5;

/// Where a thing being examined begins its blink from: the colour
/// `GridItemView.UpdateInfo` gives a thing not known, (0, 0, 0, 0.75), set
/// again by `OnExamine` after the blink began and before its tween's first
/// step (`UpdateMainImageAlpha`'s 0.7 not set while it blinks):
/// `courses10`'s `examine-multiple`, the fade fitted from 0.751.
pub(super) const EXAMINED_BLINK_FROM: f32 = 0.75;

/// One half of a box's blink under way (`ItemViewAnimation.
/// CG_StartBlinking`): its picture's alpha from and to, out and quadratic
/// (the tweening library's default), and when its clock began.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct BlinkTween {
    pub(super) from: f32,
    pub(super) to: f32,
    pub(super) began: f64,
}

impl BlinkTween {
    /// The alpha by then. Done, it is at its end on that frame, and the
    /// next half is begun with its clock from there: the next tween is made
    /// once this one is heard done, its first step a frame on (`courses10`'s
    /// `dead-scav-examine`: 0 on 739 and 740, 0.0331 on 741, 0.008 s on).
    pub(super) fn step(&mut self, now: f64) -> f32 {
        let gone = (now - self.began) as f32;
        if gone >= BLINK_HALF {
            let reached = self.to;
            *self = BlinkTween { from: reached, to: if reached > 0.5 { 0.0 } else { 1.0 }, began: now };
            return reached;
        }
        let out_quad = |x: f32| 1.0 - (1.0 - x) * (1.0 - x);
        self.from + (self.to - self.from) * out_quad(gone / BLINK_HALF)
    }
}

/// Fades the pictures of the boxes that blink (`Blinking`), each from when
/// it began; a box that has stopped is whole again. Only a picture whose
/// fading changes is written; only the thing's own picture blinks
/// (`ItemViewAnimation._mainImage`: not the examination's loader and bar,
/// a part's marks or the wish list's glow, which faded with it here).
/// The fade begins from the picture's alpha (`DOColor` from its colour):
/// a thing examined from 0.75, its tween stepped on its first frame by
/// that frame's time (the click is heard before the tweens step:
/// `courses10`'s `dead-scav-examine`, 0.722 on the release's frame 681,
/// 0.0093 s into its fade); else from its alpha at rest, from its frame.
/// A magazine being loaded or unloaded blinks too (`ItemViewAnimation.
/// StartLoading`), and takes its clicks.
/// The worked magazine's ground blinks with it (`StartBackgroundBlinking`):
/// to an alpha of 0.6 and back, half a second each way (the tween's
/// default easing, out and quadratic), whatever lights it meanwhile; the
/// page drawn afresh as the work stops gives it its colour again.
#[allow(clippy::type_complexity)]
pub(super) fn blink(
    time: Res<Time<Real>>,
    blinking: Res<Blinking>,
    carried: Res<Carried>,
    views: Query<&ItemView>,
    insides: Query<&ViewInside>,
    mut pictures: Query<(&mut ImageNode, &ChildOf, Has<UnknownPicture>), (With<ItemPicture>, Without<Loading>, Without<SpinPiece>, Without<DrainSpinner>)>,
    mut grounds: Query<(&ItemView, &Ground, &mut BackgroundColor)>,
    mut began: Local<HashMap<Id, f64>>,
    mut tweens: Local<HashMap<Id, BlinkTween>>,
    mut blinked: Local<bool>,
) {
    let now = time.elapsed_secs_f64();
    let worked = carried.magazine_work.as_ref().map(|work| work.process.magazine);
    // A thing examined blinks as one moved (`ItemView.Blinking`).
    let examined = carried.examiner.under_way.as_ref().map(|under_way| under_way.item);
    let blinks = || blinking.0.iter().copied().chain(worked).chain(examined);
    // With nothing blinking now or on the frame before (when a picture
    // that stopped was made whole again), nothing is looked at.
    let any = blinks().next().is_some();
    if !any && !*blinked {
        return;
    }
    *blinked = any;
    began.retain(|id, _| blinks().any(|blinking| blinking == *id));
    tweens.retain(|id, _| blinks().any(|blinking| blinking == *id));
    for id in blinks() {
        began.entry(id).or_insert(now);
        tweens.entry(id).or_insert_with(|| {
            if Some(id) == examined {
                BlinkTween { from: EXAMINED_BLINK_FROM, to: 0.0, began: now - time.delta_secs_f64() }
            } else {
                let rest = if carried.find(id).is_some_and(|item| !trp_sim::items::examine::known(item)) { UNKNOWN_ALPHA } else { 1.0 };
                BlinkTween { from: rest, to: 0.0, began: now }
            }
        });
    }
    // Each thing's tween stepped once, whatever views it has.
    let alphas: HashMap<Id, f32> = tweens.iter_mut().map(|(id, tween)| (*id, tween.step(now))).collect();
    for (mut image, parent, unknown) in &mut pictures {
        // In its view, or in the node a framed view's parts lie in.
        let Some(id) = views.get(parent.parent()).map(|view| view.0).or_else(|_| insides.get(parent.parent()).map(|inside| inside.0)).ok() else { continue };
        let rest = if unknown { UNKNOWN_ALPHA } else { 1.0 };
        let alpha = alphas.get(&id).copied().unwrap_or(rest);
        if image.color.alpha() != alpha {
            image.color.set_alpha(alpha);
        }
    }
    // The ground blinks with the picture for a magazine worked and for a
    // thing being used (the app's, as a magazine's).
    for (view, ground, mut back) in &mut grounds {
        if Some(view.0) != worked && Some(view.0) != carried.drained {
            continue;
        }
        let Some(at) = began.get(&view.0) else { continue };
        let since = (now - at) as f32;
        let base = ground.0.alpha();
        let lit = 1.0 - blink_alpha(since);
        let alpha = base + (0.6 - base) * lit;
        if back.0 != ground.0.with_alpha(alpha) {
            back.0 = ground.0.with_alpha(alpha);
        }
    }
}

/// A thing shown in the slot it left (`Carried::leaving`) is gone from
/// there once its move is done, as it stops blinking (the page drawn
/// afresh).
pub(super) fn keep_leaving(blinking: Res<Blinking>, mut carried: ResMut<Carried>) {
    if carried.leaving.iter().any(|(id, ..)| !blinking.0.contains(id)) {
        carried.leaving.retain(|(id, ..)| blinking.0.contains(id));
    }
}

/// The view a thing was taken from, while it is carried (`ItemView.
/// IsBeingDragged`; `courses10`'s `drag-and-drop`, every drag): it keeps
/// its place, its picture at 0.85 unless it blinks (`ItemViewAnimation.
/// SetDragState`, `UpdateMainImageAlpha`), and its other parts hidden
/// (`InfoVisibility`, `UpdateInfoVisibility`: its border, its words' panel,
/// the panel of a thing not examined; its ground is `light_related`'s); as
/// it was once let go. Its spinners are left as they are.
#[allow(clippy::type_complexity)]
pub(super) fn dim_taken(
    drag: Res<Drag>,
    blinking: Res<Blinking>,
    views: Query<(&ItemView, &Children)>,
    insides: Query<&Children, With<ViewInside>>,
    mut parts: Query<(&mut Visibility, Option<&mut ImageNode>, Has<ItemPicture>, Has<UnknownPicture>), (Without<Loading>, Without<SpinPiece>, Without<DrainSpinner>, Without<SpinnerPicture>)>,
    mut kept: Local<Vec<Entity>>,
) {
    let Some((id, layer)) = drag.0.as_ref().map(|held| (held.id, held.layer)) else {
        for entity in kept.drain(..) {
            let Ok((mut shown, image, picture, unknown)) = parts.get_mut(entity) else { continue };
            match (picture, image) {
                (true, Some(mut image)) => image.color.set_alpha(if unknown { UNKNOWN_ALPHA } else { 1.0 }),
                _ => *shown = Visibility::Inherited,
            }
        }
        return;
    };
    let blinks = blinking.0.contains(&id);
    for (view, children) in &views {
        if (view.0, view.1) != (id, layer) {
            continue;
        }
        // Framed as the game's, its parts lie in a node of their own.
        let list = children.iter().find_map(|child| insides.get(child).ok()).unwrap_or(children);
        for child in list.iter() {
            let Ok((mut shown, image, picture, _)) = parts.get_mut(child) else { continue };
            match (picture, image) {
                (true, Some(mut image)) if !blinks && image.color.alpha() != 0.85 => image.color.set_alpha(0.85),
                (true, _) => continue,
                (false, _) if *shown != Visibility::Hidden => *shown = Visibility::Hidden,
                (false, _) => continue,
            }
            if !kept.contains(&child) {
                kept.push(child);
            }
        }
    }
}

/// A slot as drawn: whatever is in it, filling it. `size` is the slot's,
/// in pixels.
#[allow(clippy::too_many_arguments)]
pub(super) fn slot_box(parent: &mut ChildSpawnerCommands, carried: &Carried, holder: &Item, slot: usize, node: Node, size: Vec2, layer: u8, s: f32) {
    slot_box_drawn(parent, carried, holder, slot, node, size, layer, SlotLook::Own, true, s);
}

/// Where the game's view of a slot of `holder` has its border line as the
/// slot stands (`SlotLook::Game`): an empty one's, a unit outside; a full
/// one's is the shaded frame, which has none.
pub(super) fn empty_line(holder: &Item, slot: usize) -> SlotLook {
    SlotLook::Game(holder.slots.get(slot).is_some_and(Option::is_none).then_some(EMPTY_LINE_OUT))
}

/// A slot's box, with its own border and ground (`SlotLook::Own`) or with
/// none, where the game's own slot view is drawn under it (`TARKOV UI`),
/// and then the line over the view's border that a tint shows on
/// (`AcceptLine`); what is in it in a slot's view (`wide`: the
/// equipment's, `SlotItemView`) or a grid's (a part in the inspect window's
/// slot: `ModSlotView` makes a `GridItemView`, `ItemUiContext.CreateItemView`).
#[allow(clippy::too_many_arguments)]
pub(super) fn slot_box_drawn(parent: &mut ChildSpawnerCommands, carried: &Carried, holder: &Item, slot: usize, node: Node, size: Vec2, layer: u8, look: SlotLook, wide: bool, s: f32) {
    // The game's slot holds no line of the app's: the thing's view is
    // stretched over the slot's place (`SlotView.CreateItemView`, its
    // `_slotPlace`: the slot's whole panel), its ground to the slot's
    // edges, the slot's frame outside them (`slot_border_full`, 6 pixels
    // out: its light pixel the second out, its dark the first). Kept a
    // pixel inside, a pixel of the panel would show between the
    // frame and the ground.
    let edge = if matches!(look, SlotLook::Game(_)) { 0.0 } else { 1.0 };
    let node = Node { border: UiRect::all(Val::Px(edge)), ..node };
    // What is in it, or what is leaving it while the hands finish its move
    // (`Carried::leaving`): shown as full.
    let shown = holder.slots.get(slot).and_then(Option::as_ref).or_else(|| carried.leaving_from(holder.id, slot));
    let look = match look {
        SlotLook::Game(Some(_)) if shown.is_some() => SlotLook::Game(None),
        look => look,
    };
    let ((border, ground), line) = match look {
        SlotLook::Own => ((LINE, SLOT), None),
        SlotLook::Game(line) => ((Color::NONE, Color::NONE), line),
    };
    parent.spawn((SlotBox { holder: holder.id, slot, layer, border, lined: line.is_some() }, RelativeCursorPosition::default(), node, BorderColor::all(border), BackgroundColor(ground))).with_children(|view| {
        if let Some(item) = shown {
            let inside = (size - Vec2::splat(2.0 * edge)).max(Vec2::ONE);
            item_view(view, carried, item, Node { width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() }, wide, inside, false, layer, s);
        }
        // The line a pixel wide on the border of the view's rectangle so
        // far out (placed from inside the box's own border, where it has
        // one).
        if let Some(out) = line {
            let out = out * s;
            let at = Node {
                position_type: PositionType::Absolute,
                left: Val::Px(-edge - out),
                top: Val::Px(-edge - out),
                width: Val::Px(size.x + 2.0 * out),
                height: Val::Px(size.y + 2.0 * out),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            };
            view.spawn((AcceptLine { holder: holder.id, slot }, at, BorderColor::all(Color::NONE), Visibility::Hidden, bevy::ui::FocusPolicy::Pass));
        }
    });
}

/// A grid as drawn: its cells, and each thing in it where it lies.
pub(super) fn grid_box(parent: &mut ChildSpawnerCommands, carried: &Carried, holder: &Item, grid: usize, cell: f32, layer: u8) {
    grid_box_drawn(parent, carried, holder, grid, cell, layer, true);
}

/// A grid's box, with its own ground and cell lines (`drawn`) or with
/// none, where the game's own grid view is drawn under it (`TARKOV UI`).
pub(super) fn grid_box_drawn(parent: &mut ChildSpawnerCommands, carried: &Carried, holder: &Item, grid: usize, cell: f32, layer: u8, drawn: bool) {
    let cells = holder.kind.grids[grid].cells;
    let (width, height) = (cells[0] as f32 * cell, cells[1] as f32 * cell);
    let node = Node { width: Val::Px(width), height: Val::Px(height), flex_shrink: 0.0, ..default() };
    let outline = Outline::new(Val::Px(if drawn { 1.0 } else { 0.0 }), Val::ZERO, LINE);
    let ground = if drawn { BACK } else { Color::NONE };
    parent.spawn((GridBox { holder: holder.id, grid, cells, layer }, RelativeCursorPosition::default(), node, outline, BackgroundColor(ground))).with_children(|inside| {
        for column in (1..cells[0]).filter(|_| drawn) {
            inside.spawn((placed(column as f32 * cell, 0.0, 1.0, height), BackgroundColor(PANEL)));
        }
        for row in (1..cells[1]).filter(|_| drawn) {
            inside.spawn((placed(0.0, row as f32 * cell, width, 1.0), BackgroundColor(PANEL)));
        }
        // Over the game's grid view, a thing's view framed as the game's
        // lies at its first cell's corner, as large as its cells and a
        // unit more, over the lines about them (`health_view::ViewFrame`);
        // the app's own lies a pixel inside them.
        let s = cell / CELL;
        let framed = !drawn && carried.item_looks[1].as_ref().is_some_and(|look| look.frame.is_some());
        for (place, item) in &holder.grids[grid] {
            let [w, h] = item.cells();
            let (w, h) = if place.turned { (h, w) } else { (w, h) };
            let (size, node) = match framed {
                true => {
                    let size = Vec2::new(w as f32 * cell + s, h as f32 * cell + s);
                    (size, placed(place.x as f32 * cell, place.y as f32 * cell, size.x, size.y))
                }
                false => {
                    let size = Vec2::new(w as f32 * cell - 2.0, h as f32 * cell - 2.0);
                    (size, placed(place.x as f32 * cell + 1.0, place.y as f32 * cell + 1.0, size.x, size.y))
                }
            };
            item_view(inside, carried, item, node, false, size, place.turned, layer, s);
        }
    });
}
