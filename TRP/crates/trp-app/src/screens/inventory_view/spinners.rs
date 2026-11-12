//! Unknown and unsearched things, the examine loader and the spinners.

use super::*;

/// A game's view of a thing (`TARKOV UI`): its own ground and border,
/// which it goes back to when it is no longer lit.
#[derive(Component, Clone, Copy)]
pub(crate) struct Ground(pub(crate) Color, pub(crate) Color);

/// The colour panel of a thing not known, drawn over its noise
/// (`game_item_view`), and the view's own pointer to it.
#[derive(Component)]
pub(super) struct UnknownGround;

#[derive(Component, Clone, Copy)]
pub(super) struct GroundApart(pub(super) Entity);

/// A view of a thing not known, and the node its parts lie in (inventory
/// spec, "Examining"); and its picture, black, at 0.7 when not blinking.
#[derive(Component)]
pub(super) struct UnknownView;

#[derive(Component)]
pub(super) struct UnknownPicture;

/// A view of a thing not yet found in a search (inventory spec, "Searching
/// a container"): its unsearched ground and nothing else; the thing.
#[derive(Component)]
pub(crate) struct UnsearchedView(pub(super) Id);

/// The alpha of a thing's view that cannot be dragged or taken out
/// (`ItemView.Transparency`: its `minAlpha`, 0.6, on the view's
/// `CanvasGroup`), as a thing not yet found is: `courses10`'s searches draw
/// its ground's 0.69 at 0.414.
pub(super) const UNTAKEN_ALPHA: f32 = 0.6;

/// A part of a container's search that takes a click: the container, and
/// whether it begins or stops its search (`search_view::clicks`).
#[derive(Component, Clone, Copy)]
pub(crate) struct SearchClick(pub Id, pub crate::screens::health_view::SearchHit);

/// How the search of a container stands, as its view draws it
/// (`SearchableView.UpdateSearchState`): not yet opened; being searched,
/// with its timer from its start (`TimerText`: whole minutes, then the
/// seconds in two places), on the clock the screen was last drawn at; its
/// button shown while it is not searched, being searched, or holds what is
/// still to be found. A search may begin while fewer are under way than
/// the character may make at once (`CanStartNewSearchOperation`: one, two
/// at Search's elite).
pub(crate) fn search_shown(carried: &Carried, container: &Item) -> crate::screens::health_view::StashSearch {
    let searcher = &carried.searcher;
    let under_way = searcher.searching(container.id);
    let unsearched = !searcher.searched(container.id);
    let timer = under_way.map_or_else(String::new, |search| {
        let gone = (carried.search_clock - search.began).max(0.0);
        format!("{}:{:02}", (gone / 60.0) as u32, gone as u32 % 60)
    });
    crate::screens::health_view::StashSearch {
        unsearched,
        searching: under_way.is_some(),
        button: unsearched || under_way.is_some() || searcher.holds_unknown(container),
        can_start: searcher.under_way.len() < if carried.search_double { 2 } else { 1 },
        timer,
    }
}

/// The examination's bar's fill: as wide as the share done of its whole
/// width in pixels, or of its holder (the app's look).
#[derive(Component)]
pub(super) struct ExamineFill(pub(super) Option<f32>);

/// The examination's percent done.
#[derive(Component)]
pub(super) struct ExamineLabel;

/// The examination's loader (`ExamineLoader`).
#[derive(Component)]
pub(super) struct ExamineLoader;

/// How fast the examination's loader turns, degrees a second, clockwise:
/// measured, `courses10`'s `examine-multiple` (each of its four examines
/// from 0 on its first frame: -4.37 a frame on, -43.99 at 0.122 s, -202.13
/// at 0.561 s) and `examine-looted-item` (360.2). What turns it was not
/// found: the prefab's loader has no script but its picture
/// (`probe prefab .../itemviewexaminationcomponent`).
const EXAMINE_LOADER_TURNS: f32 = 360.0;

/// The examination's view kept to the share done, each frame
/// (`ItemViewExamineComponent.Show`: the wall clock's time gone over the
/// whole; its label the percent rounded, with no sign); its loader turned
/// clockwise from where it began, as the examine began.
pub(super) fn turn_examination(carried: Res<Carried>, time: Res<Time<Real>>, mut fills: Query<(&ExamineFill, &mut Node)>, mut labels: Query<&mut Text, With<ExamineLabel>>, mut loaders: Query<&mut UiTransform, With<ExamineLoader>>) {
    let Some(under_way) = carried.examiner.under_way.as_ref() else { return };
    let now = time.elapsed_secs_f64();
    // (The engine's clockwise is less than none; Bevy's, more.)
    let turn = Rot2::degrees(EXAMINE_LOADER_TURNS * (now - under_way.began).max(0.0) as f32);
    for mut loader in &mut loaders {
        if loader.rotation != turn {
            loader.rotation = turn;
        }
    }
    let done = under_way.done(now);
    for (fill, mut node) in &mut fills {
        let width = match fill.0 {
            Some(whole) => Val::Px(whole * done),
            None => Val::Percent(done * 100.0),
        };
        if node.width != width {
            node.width = width;
        }
    }
    let said = (done * 100.0).round().to_string();
    for mut text in &mut labels {
        if **text != said {
            **text = said.clone();
        }
    }
}

/// The picture of a thing not known's view black, at 0.7 (`GridItemView.
/// UpdateInfo`'s `(0, 0, 0, 0.75)`, its alpha then set to 0.7 by
/// `UpdateMainImageAlpha`), as it comes.
pub(super) fn tint_unknown(mut commands: Commands, mut pictures: Query<(Entity, &mut ImageNode, &ChildOf), Added<ItemPicture>>, unknown: Query<(), With<UnknownView>>) {
    for (entity, mut image, parent) in &mut pictures {
        if unknown.contains(parent.parent()) {
            image.color = Color::srgba(0.0, 0.0, 0.0, UNKNOWN_ALPHA);
            commands.entity(entity).insert(UnknownPicture);
        }
    }
}

/// A thing not known's picture's alpha at rest.
pub(super) const UNKNOWN_ALPHA: f32 = 0.7;

/// A piece of the load component over a worked magazine's view.
#[derive(Component, Clone, Copy, PartialEq)]
pub(super) struct SpinPiece(pub(super) SpinRole);

#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum SpinRole {
    Back,
    Fill,
    Loader,
    Stop,
}

/// The spinner's fill, its picture and cut, until its material is made.
#[derive(Component)]
pub(super) struct SpinFill(pub(super) Handle<Image>, pub(super) Rect);

/// The app's bar of a worked magazine's share done.
#[derive(Component)]
pub(super) struct AppSpinBar;

/// The load component turned each frame (inventory spec, "The load
/// component"): the spinner filled by the work's share done, the loader
/// turned by the present round's (shown where more than one round is to
/// go), both red and the stop cross on with the mouse on the magazine's
/// view (`SetStopButtonStatus`); the app's bar filled the same.
#[allow(clippy::too_many_arguments)]
pub(super) fn turn_spinners(
    mut commands: Commands,
    carried: Res<Carried>,
    views: Query<(&ItemView, &RelativeCursorPosition)>,
    mut materials: ResMut<Assets<crate::ui::radial_fill::RadialFill>>,
    fresh: Query<(Entity, &SpinFill), Without<MaterialNode<crate::ui::radial_fill::RadialFill>>>,
    mut pieces: Query<(&SpinPiece, Option<&mut UiTransform>, Option<&mut ImageNode>, &mut Visibility, Option<&MaterialNode<crate::ui::radial_fill::RadialFill>>)>,
    mut bars: Query<&mut Node, With<AppSpinBar>>,
    mut was_hovered: Local<bool>,
) {
    // (The page is drawn afresh as each round moves: a view just made does
    // not yet know where the mouse is, and keeps the one before's; its
    // fill's material is made at the work's share and colour, so that
    // nothing flickers between the two.)
    let work = carried.magazine_work.as_ref();
    let (done, round) = work.map_or((0.0, 0.0), |work| work.shown.shares());
    let hovered = work.is_some_and(|work| views.iter().filter(|(view, _)| view.0 == work.process.magazine).any(|(_, at)| if at.normalized.is_none() { *was_hovered } else { at.cursor_over() }));
    *was_hovered = hovered;
    let color = if hovered { Color::srgb(1.0, 0.0, 0.0) } else { Color::WHITE };
    for (entity, fill) in &fresh {
        let mut made = crate::ui::radial_fill::RadialFill::new(fill.0.clone(), fill.1, color, true, [0.0, 0.0, 1.0, 1.0]);
        made.fill.x = done;
        let material = materials.add(made);
        commands.entity(entity).insert((MaterialNode(material), UiTransform::IDENTITY));
    }
    let Some(work) = work else { return };
    for (piece, turn, image, mut visibility, material) in &mut pieces {
        let wanted = match piece.0 {
            SpinRole::Stop => hovered,
            SpinRole::Loader => work.shown.to_go > 1,
            _ => true,
        };
        let wanted = if wanted { Visibility::Inherited } else { Visibility::Hidden };
        if *visibility != wanted {
            *visibility = wanted;
        }
        if let (Some(mut image), true) = (image, matches!(piece.0, SpinRole::Back | SpinRole::Loader)) {
            if image.color != color {
                image.color = color;
            }
        }
        // A whole turn clockwise each round (the engine's -360 degrees about
        // its forward axis; Bevy's clockwise is the other sign).
        if let (SpinRole::Loader, Some(mut turn)) = (piece.0, turn) {
            turn.rotation = Rot2::degrees(360.0 * round);
        }
        if let Some(fill) = material.and_then(|node| materials.get_mut(&node.0)) {
            fill.fill.x = done;
            fill.set_color(color);
        }
    }
    for mut bar in &mut bars {
        bar.width = Val::Percent(done * 100.0);
    }
}

/// An item's picture, whole in the middle of a box of that size: the
/// thing's own where it has been made; while it is still to come a
/// spinner, as the game's view shows its loader until its picture is read
/// or drawn (`ItemView._iconLoader`; icons spec, "When a picture is asked
/// for"): the game's own (`loader`, its picture and its size in pixels)
/// where the view is the game's, else the app's; and on where the picture
/// cannot be made, as the game's view keeps its loader while it has none.
/// The plain shape of its sort only where no picture is
/// made at all (`ICON GENERATION` off, or a thing of no cells).
pub(super) fn picture_in(thing: &mut ChildSpawnerCommands, carried: &Carried, item: &Item, room: Rect, turned: bool, loader: Option<(&crate::screens::health_view::Sprite, Vec2, [f32; 4])>) {
    let key = shown_key(carried, item);
    if let Some(picture) = carried.pictures.get(&key) {
        thing.spawn(picture_node(picture, room, turned));
        return;
    }
    let [across, down] = item.cells();
    if carried.pictures_on && across > 0 && down > 0 {
        thing.spawn(spinner_node(carried, Loading { key, room, turned, quick: None }, loader));
    } else if let Some(picture) = carried.icons.get(icon_of(item)) {
        thing.spawn(picture_node(picture, room, turned));
    }
}

/// The inside of a box of that size within the app's line, a pixel wide
/// all round: the room its picture fills, from its holder's corner.
pub(super) fn within_line(size: Vec2) -> Rect {
    Rect::from_corners(Vec2::ZERO, (size - Vec2::splat(2.0)).max(Vec2::ONE))
}

/// A picture placed whole in the middle of a room, the room's place in
/// its holder.
pub(super) fn picture_node(picture: &Picture, room: Rect, turned: bool) -> impl Bundle {
    let shape = if turned { 1.0 / picture.shape } else { picture.shape };
    let share = if picture.fills { 1.0 } else { PICTURE_SHARE };
    let size = room.size();
    let height = (size.y * share).min(size.x * share / shape);
    let width = height * shape;
    let over = placed(room.min.x + (size.x - width) / 2.0, room.min.y + (size.y - height) / 2.0, width, height);
    let image = if turned { &picture.turned } else { &picture.upright };
    (ImageNode::new(image.clone()), over, ItemPicture)
}

/// An item's own picture in its view (`GridItemView.MainImage`).
#[derive(Component)]
pub(super) struct ItemPicture;

/// An item's view drawn as unusable by a malfunction (`game_item_view`),
/// and the node its parts lie in where it is framed as the game's.
#[derive(Component)]
pub(super) struct Unusable;

/// The node over the whole of a framed view that its parts lie in
/// (`game_item_view`), by the thing's number.
#[derive(Component)]
pub(super) struct ViewInside(pub(super) Id);

/// The picture of an unusable item's view, red (`MainImage.color = Color.red`),
/// as it comes (made with the view, or put in for its spinner).
pub(super) fn tint_unusable(mut pictures: Query<(&mut ImageNode, &ChildOf), (Added<ItemPicture>, Without<UnknownPicture>)>, unusable: Query<(), With<Unusable>>) {
    for (mut image, parent) in &mut pictures {
        if unusable.contains(parent.parent()) {
            let alpha = image.color.alpha();
            image.color = Color::srgba(1.0, 0.0, 0.0, alpha);
        }
    }
}

/// How large a spinner is, as a share of the box's shorter side, and at
/// most and least in pixels; and how many turns it makes a second. The
/// app's.
const SPINNER_SHARE: f32 = 0.42;

const SPINNER_SIDE: [f32; 2] = [10.0, 28.0];

pub(super) const SPINNER_TURNS: f32 = 1.1;

/// The app's spinner picture's side, in pixels.
pub(super) const SPINNER_PIXELS: usize = 64;

/// The loader over a thing being used (`ItemView._drainLoader`).
#[derive(Component)]
pub(super) struct DrainSpinner;

/// A search's loader (the unsearched panel's, the search button's), and
/// the container searched: turned on its own (`turn_search_loaders`).
#[derive(Component)]
pub(super) struct SearchSpinner(pub(super) Id);

/// How fast a search's loaders turn, in turns a second, clockwise, each
/// from upright as its search began: measured, `courses10`'s searches
/// (`dead-scav-search-b`: both loaders from 0 on the search's first frame,
/// 3.00 degrees a frame at 8.3 ms, together; `dead-scav-search-a`,
/// `weapon-box-search-a`, `airdrop-crate-search` about 360 a second). What
/// turns them was not read.
const SEARCH_LOADER_TURNS: f64 = 1.0;

/// A spinner's picture: its texture, its cut as `RadialFill` takes it
/// (left, lowest, right, highest row), its colour, and the share of the
/// spinner's square its cut covers (a sprite packed trimmed lies in its
/// place, and the square turns about its middle). Every spinner of one
/// picture is drawn by one material, turned each frame (`fill_pictures`).
#[derive(Component, Clone, PartialEq)]
pub(super) struct SpinnerPicture(pub(super) Handle<Image>, pub(super) Rect, pub(super) Color, pub(super) [f32; 4]);

/// A spinner in the middle of a box, turned each frame (`fill_pictures`):
/// the game's loader at its own size where it is had (a view of the
/// game's), else the app's.
pub(super) fn spinner_node(carried: &Carried, loading: Loading, loader: Option<(&crate::screens::health_view::Sprite, Vec2, [f32; 4])>) -> impl Bundle {
    let (middle, room) = (loading.room.center(), loading.room.size());
    if let Some((sprite, side, covers)) = loader {
        let over = placed(middle.x - side.x / 2.0, middle.y - side.y / 2.0, side.x, side.y);
        return (SpinnerPicture(sprite.image.clone(), sprite.cut, sprite.color, covers), over, loading);
    }
    let side = (room.min_element() * SPINNER_SHARE).clamp(SPINNER_SIDE[0], SPINNER_SIDE[1]).min(room.min_element());
    let over = placed(middle.x - side / 2.0, middle.y - side / 2.0, side, side);
    // The app's picture has its first row at the top, where the game's
    // textures have theirs lowest: its cut taken upside down.
    let pixels = SPINNER_PIXELS as f32;
    let cut = Rect { min: Vec2::new(0.0, pixels), max: Vec2::new(pixels, 0.0) };
    (SpinnerPicture(carried.spinner.clone(), cut, Color::WHITE, [0.0, 0.0, 1.0, 1.0]), over, loading)
}

/// A search's loaders turned, each container's by a material of its own
/// from when its search began (`SEARCH_LOADER_TURNS`): a search begun anew
/// turns them from upright again.
pub(super) fn turn_search_loaders(
    mut commands: Commands,
    time: Res<Time<Real>>,
    carried: Res<Carried>,
    features: Res<crate::screens::options_view::Features>,
    mut materials: ResMut<Assets<crate::ui::radial_fill::RadialFill>>,
    mut kept: Local<Vec<(Id, SpinnerPicture, Handle<crate::ui::radial_fill::RadialFill>)>>,
    fresh: Query<(Entity, &SpinnerPicture, &SearchSpinner), Without<MaterialNode<crate::ui::radial_fill::RadialFill>>>,
) {
    for (entity, picture, loader) in &fresh {
        let handle = match kept.iter().find(|(id, had, _)| *id == loader.0 && had == picture) {
            Some((.., handle)) => handle.clone(),
            None => {
                let handle = materials.add(crate::ui::radial_fill::RadialFill::new(picture.0.clone(), picture.1, picture.2, features.tarkov_ui, picture.3));
                kept.push((loader.0, picture.clone(), handle.clone()));
                handle
            }
        };
        commands.entity(entity).insert(MaterialNode(handle));
    }
    let now = time.elapsed_secs_f64();
    kept.retain(|(id, ..)| carried.searcher.searching(*id).is_some());
    for (id, _, handle) in kept.iter() {
        let Some(search) = carried.searcher.searching(*id) else { continue };
        if let Some(material) = materials.get_mut(handle) {
            material.set_turn(((now - search.began).max(0.0) * SEARCH_LOADER_TURNS).fract() as f32);
        }
    }
}
