//! What lies under the mouse, foresight, related things lit and item tooltips.

use super::*;

/// What is drawn on top where the mouse is: the page (0), a window, the
/// bar or the window that asks how many.
pub(crate) fn cover_under<'a>(covers: impl Iterator<Item = (&'a Cover, &'a RelativeCursorPosition)>) -> u8 {
    covers.filter(|(_, at)| at.cursor_over()).map(|(cover, _)| cover.0).max().unwrap_or(0)
}

/// The thing under the mouse on what is drawn on top there: the smallest
/// is the one meant. With it, where in it the mouse is, as a share of it.
/// A box that blinks is not there to the mouse (`ItemView.IsInteractive`).
pub(super) fn item_under<'a>(items: impl Iterator<Item = (&'a ItemView, &'a RelativeCursorPosition, &'a ComputedNode)>, cover: u8, but: Option<Id>, blinking: &Blinking) -> Option<(Id, Vec2)> {
    let over = items.filter(|(view, at, _)| view.1 == cover && Some(view.0) != but && at.cursor_over() && !blinking.0.contains(&view.0));
    over.min_by(|a, b| a.2.size().length_squared().total_cmp(&b.2.size().length_squared())).map(|(view, at, _)| (view.0, share_of(at).unwrap_or_default()))
}

/// How long the mouse rests on a weapon before its rig is made ready
/// ahead of its being hung, in seconds. `ESTIMATE`: the app's choice.
const RESTS_ON: f32 = 0.25;

/// A weapon the mouse rests on, or one taken up, has its rig made ready
/// before it is hung, so that the hands do not wait for it then: once no
/// weapon hung waits for one, and one at a time (`weapon::Preparing::
/// ask_ahead`). One that is not hung within a while is let go of again
/// (`weapon::free_rigs`); it is asked for once a rest, so that a mouse
/// left on it does not have it made again and again.
#[allow(clippy::too_many_arguments)]
pub(super) fn foresee(
    time: Res<Time>,
    controller: Res<Controller>,
    carried: Res<Carried>,
    drag: Res<Drag>,
    blinking: Res<Blinking>,
    covers: Query<(&Cover, &RelativeCursorPosition)>,
    views: Query<(&ItemView, &RelativeCursorPosition, &ComputedNode)>,
    preparing: Option<ResMut<crate::weapons::weapon::Preparing>>,
    stowed: Option<Res<Stowed>>,
    mut resting: Local<Option<(Id, f32, bool)>>,
) {
    let (Some(mut preparing), Some(stowed)) = (preparing, stowed) else { return };
    if !controller.inventory_open {
        *resting = None;
        return;
    }
    let (pointed, taken) = match &drag.0 {
        Some(held) => (Some(held.id), true),
        None => (item_under(views.iter(), cover_under(covers.iter()), None, &blinking).map(|(id, _)| id), false),
    };
    let now = time.elapsed_secs();
    let Some(pointed) = pointed else {
        *resting = None;
        return;
    };
    let (since, asked) = match *resting {
        Some((id, since, asked)) if id == pointed => (since, asked),
        _ => {
            *resting = Some((pointed, now, false));
            (now, false)
        }
    };
    if asked || (!taken && now - since < RESTS_ON) {
        return;
    }
    *resting = Some((pointed, since, true));
    let Some(item) = carried.find(pointed).filter(|item| trp_sim::items::inventory::is(item, "Weapon")) else { return };
    let template = &trp_sim::items::inventory::rig_key(item);
    if !stowed.has_rig(template) && stowed.awaited(template) {
        preparing.ask_ahead(template);
    }
}

/// The thing the mouse is on in the screen, or carries, as `light_related`
/// last found it (stale while the screen is shut): what the key for it would
/// put down (`loose::Foreseen`).
#[derive(Resource, Default)]
pub struct Pointed(pub Option<Id>);

/// The box under the mouse in the game's own colour for it, and the boxes
/// of the things that go with it in orange (inventory spec, "The box under
/// the mouse, and what goes with it"); while a thing is carried, what goes
/// with that. The orange only with `COMPATIBLE ITEMS HIGHLIGHT` on (settings
/// spec, "Features"). Worked out again only when what is pointed at
/// changes or the screen has been drawn afresh, and only a box whose
/// colour changes is written, so that nothing else is drawn anew.
pub(super) fn light_related(
    controller: Res<Controller>,
    features: Res<crate::screens::options_view::Features>,
    carried: Res<Carried>,
    drag: Res<Drag>,
    blinking: Res<Blinking>,
    covers: Query<(&Cover, &RelativeCursorPosition)>,
    mut views: Query<(&ItemView, &RelativeCursorPosition, &ComputedNode, &mut BackgroundColor, &mut BorderColor, Option<&Ground>, Option<&GroundApart>), Without<UnknownGround>>,
    mut apart: Query<&mut BackgroundColor, (With<UnknownGround>, Without<ItemView>)>,
    fresh: Query<(), Added<ItemView>>,
    mut lit_for: Local<Option<(Option<Id>, bool)>>,
    mut lay: Local<Option<(Id, Spot)>>,
    mut told: ResMut<Pointed>,
) {
    let _clock = crate::platform::render_timing::Clock::new("inventory light_related");
    if !controller.inventory_open {
        *lit_for = None;
        return;
    }
    let pointed = match &drag.0 {
        Some(held) => Some(held.id),
        // On the frame the page is drawn afresh its boxes do not yet know
        // where the mouse is: what was pointed at stays lit, or every box
        // the mouse lights goes dark for that frame (a blink); but not a
        // thing that has moved (put on with Alt, sent with Ctrl), whose new
        // box is not under the mouse: lit there, it flashed white for that
        // frame).
        None if !fresh.is_empty() => lit_for.and_then(|(pointed, _)| pointed).filter(|id| carried.lies(*id) == *lay),
        None => item_under(views.iter().map(|(view, at, node, ..)| (view, at, node)), cover_under(covers.iter()), None, &blinking).map(|(id, _)| id),
    };
    // (A thing pointed at and then taken is the same thing: its own box
    // goes back to its colour.)
    let now = (pointed, drag.0.is_some());
    if *lit_for == Some(now) && fresh.is_empty() {
        return;
    }
    *lit_for = Some(now);
    if told.0 != pointed {
        told.0 = pointed;
    }
    *lay = pointed.and_then(|id| carried.lies(id));
    // Every thing on the screen by its number, once.
    let mut things = HashMap::new();
    fn index<'a>(item: &'a Item, into: &mut HashMap<Id, &'a Item>) {
        into.insert(item.id, item);
        item.contents().for_each(|inside| index(inside, into));
    }
    index(&carried.equipment.item, &mut things);
    index(&carried.loot, &mut things);
    let source = pointed.and_then(|id| things.get(&id).copied());
    let over = |(tint, share): (Srgba, f32)| Color::Srgba(THING.to_srgba().mix(&tint, share));
    let taken = drag.0.as_ref().map(|held| (held.id, held.layer));
    // While a thing is carried, the views of the things that would take it
    // (`ItemUiContext.CheckCompatibleItems`; `GridItemView.
    // CheckAcceptHandler`, `ShowAcceptHighlight`, the game's highlight
    // scope `All`, `courses10`'s): where a drop on the thing would do
    // something (`Container.CanAccept`, the drop's own try), the engine's
    // yellow for a thing with grids or a stack, its green otherwise; a
    // repair kit's green on what it mends where it may (`CanInteract`);
    // each at 10/51 in place of its ground (`UpdateColor`). Not the thing
    // itself nor what is in it. `PROVISIONAL`: tried once as it is taken,
    // not again as Ctrl goes down or up.
    let accepts = |thing: &Item| -> Option<Color> {
        let source = source.filter(|_| taken.is_some())?;
        if thing.id == source.id || source.find(thing.id).is_some() {
            return None;
        }
        let could = !thing.kind.grids.is_empty() || !thing.kind.slots.is_empty() || !thing.kind.cartridges.is_empty() || thing.kind.template == source.kind.template;
        let takes = could && {
            let (mut worn, mut loot) = (carried.equipment.item.clone(), carried.loot.clone());
            act(&mut [&mut worn, &mut loot], source.id, Target::Item(thing.id), &carried.rules, None).is_ok()
        };
        let color = if takes && (!thing.kind.grids.is_empty() || thing.kind.stack_max > 1) {
            Color::srgb(1.0, 0.92156863, 0.015686275)
        } else if takes || (carried.repairing && kit_mends(source, thing)) {
            Color::srgb(0.0, 1.0, 0.0)
        } else {
            return None;
        };
        Some(color.with_alpha(10.0 / 51.0))
    };
    let mut accepted = HashMap::new();
    for (view, _, _, back, mut border, ground, ground_apart) in &mut views {
        // The view it was taken from: its ground and border gone while it
        // is carried (`ItemView.UpdateInfoVisibility`: `ColorPanel`,
        // `_border`); its other parts, `dim_taken`.
        let from = taken == Some((view.0, view.1));
        let mut lit = || *accepted.entry(view.0).or_insert_with(|| things.get(&view.0).and_then(|thing| accepts(thing)));
        let (want_back, want_border) = match (source, ground) {
            _ if from => (Color::NONE, Color::NONE),
            // The game's own view: its ground, white under the mouse
            // (`ItemView.UpdateColor`), as the game's own colours go.
            (Some(source), Some(ground)) if source.id == view.0 => (if drag.0.is_some() { ground.0 } else { crate::screens::health_view::ITEM_SELECTED }, ground.1),
            (Some(source), Some(_)) if features.compatible_highlight && things.get(&view.0).is_some_and(|thing| go_together(source, thing)) => (Color::Srgba(GOES_WITH.0.with_alpha(GOES_WITH.1)), Color::Srgba(GOES_WITH.0)),
            (_, Some(ground)) => (lit().unwrap_or(ground.0), ground.1),
            (Some(source), None) if source.id == view.0 => (if drag.0.is_some() { THING } else { over(POINTED) }, LINE),
            (Some(source), None) if features.compatible_highlight && things.get(&view.0).is_some_and(|thing| go_together(source, thing)) => (over(GOES_WITH), Color::Srgba(GOES_WITH.0)),
            _ => (lit().unwrap_or(THING), LINE),
        };
        // (A thing not known's ground lies over its noise, apart.)
        let mut back = match ground_apart.and_then(|apart_of| apart.get_mut(apart_of.0).ok()) {
            Some(ground) => ground,
            None => back,
        };
        if back.0 != want_back {
            back.0 = want_back;
        }
        if *border != BorderColor::all(want_border) {
            *border = BorderColor::all(want_border);
        }
    }
}

/// How long the mouse rests on a thing's view before its name shows
/// (`GridItemView.ShowTooltip`: `SimpleTooltip.Show(name, null, 0.6f)`).
pub(super) const ITEM_TIP_DELAY: f32 = 0.6;

/// What the things' tooltips last saw: the thing pointed at and where it
/// lay, whether the mouse was on its value, whether its name was closed by
/// that, and its points' lines as last made.
#[derive(Default)]
pub(super) struct ItemTipKept {
    pointed: Option<Id>,
    pub(super) lay: Option<(Id, Spot)>,
    on_value: bool,
    pub(super) closed: bool,
    plates: Option<(Id, Vec<trp_sim::items::item_attributes::TipLine>)>,
    /// The words last asked for and their wait, the thing pointed at if it
    /// was being examined, and whether the hover is renewed on this frame.
    pub(super) said: Option<(String, f32)>,
    pub(super) examined: Option<Id>,
    renew: bool,
    /// A thing not yet found under the mouse, as last seen.
    pub(super) hidden: Option<Id>,
}

/// A thing's name while the mouse rests on its view (inventory spec,
/// "Tooltips"; `GridItemView.OnPointerEnter`: `ShowTooltip`, the simple
/// tooltip after `ITEM_TIP_DELAY`), on what is drawn on top where the mouse
/// is, while nothing is carried and no menu is open (a drag's start ends
/// the wait: `courses10`'s `hover-tooltips`, frame 3653); gone as the mouse
/// leaves the view (`OnPointerExit`: `HideTooltip`). A thing in a quick
/// slot has none (`QuickSlotItemView`), nor one not yet found (its view
/// shows nothing: `IsSearched`), nor one found under the mouse until the
/// mouse comes onto it anew (its view shown in place, `GridItemView.
/// UpdateInfo`, not entered: `courses10`'s `airdrop-crate-search`, frame
/// 1368, no tooltip in the 2.7 s left). On an armour holder's value, its points
/// at once in place of the name (`ValueProxyPointerEnterHandler`: the
/// simple tooltip closed, the tooltip of lines shown,
/// `MultiLineInfo(item, false)`), gone as the mouse leaves the value
/// (`ValueProxyPointerExitHandler`), the name not shown again before the
/// mouse comes onto the view anew.
#[allow(clippy::too_many_arguments)]
pub(super) fn item_tooltip(
    controller: Res<Controller>,
    carried: Res<Carried>,
    drag: Res<Drag>,
    blinking: Res<Blinking>,
    menu: Res<crate::screens::item_menu_view::ItemMenu>,
    game: Res<GameData>,
    (covers, views, values, hidden): (Query<(&Cover, &RelativeCursorPosition)>, Query<(&ItemView, &RelativeCursorPosition, &ComputedNode)>, Query<(&ValueProxy, &RelativeCursorPosition)>, Query<(&UnsearchedView, &RelativeCursorPosition)>),
    fresh: Query<(), Or<(Added<ItemView>, Added<ValueProxy>)>>,
    (mut simple, mut lines): (ResMut<crate::ui::tooltip::SimpleTip>, ResMut<crate::ui::tooltip::LinesTip>),
    mut kept: Local<ItemTipKept>,
) {
    if !controller.inventory_open || drag.0.is_some() || menu.0.is_some() {
        *kept = ItemTipKept::default();
        return;
    }
    let cover = cover_under(covers.iter());
    // On the frame the page is drawn afresh its boxes do not yet know where
    // the mouse is: what was pointed at stays so, if it has not moved.
    let (pointed, on_value, hidden) = if fresh.is_empty() {
        let pointed = item_under(views.iter(), cover, None, &blinking).map(|(id, _)| id);
        let hidden = hidden.iter().find(|(_, at)| at.cursor_over()).map(|(view, _)| view.0);
        (pointed, values.iter().any(|(value, at)| value.1 == cover && Some(value.0) == pointed && at.cursor_over()), hidden)
    } else {
        let pointed = kept.pointed.filter(|id| carried.lies(*id) == kept.lay);
        (pointed, kept.on_value && pointed.is_some(), kept.hidden)
    };
    // A thing found where the mouse was on it hidden is not come onto: no
    // tooltip until the mouse leaves it and comes back.
    if pointed != kept.pointed {
        kept.closed = pointed.is_some() && pointed == kept.hidden;
    }
    kept.hidden = hidden;
    (kept.pointed, kept.lay, kept.on_value) = (pointed, pointed.and_then(|id| carried.lies(id)), on_value);
    let Some(item) = pointed.and_then(|id| carried.find(id)) else { return };
    if on_value {
        kept.closed = true;
        if kept.plates.as_ref().is_none_or(|(id, _)| *id != item.id) || carried.is_changed() {
            let figures = trp_sim::items::item_attributes::figures(item, &game.db, &|key| carried.texts.say(key));
            let plates = figures.rows.into_iter().find(|row| row.id == "ArmorClass").map(|row| row.tip).unwrap_or_default();
            kept.plates = Some((item.id, plates));
        }
        if let Some((_, plates)) = kept.plates.as_ref().filter(|(_, plates)| !plates.is_empty()) {
            lines.show(plates.clone());
        }
        return;
    }
    if kept.closed {
        return;
    }
    // Its examine ended under the mouse: its view's examination (its
    // label, what the mouse was on) gone, the words as they were on that
    // frame (they are set as the tooltip shows); on the next the hover is
    // renewed, the tooltip closed and its wait begun again
    // (`GridItemView.OnPointerExit`, `OnPointerEnter`; `courses10`'s
    // `dead-scav-examine`: "Unknown item" on the known frame 748, closed on
    // 749, the name on 821, 0.601 s on).
    let examining = carried.examiner.examining(item.id);
    let ended = kept.examined == Some(item.id) && !examining;
    kept.examined = examining.then_some(item.id);
    if ended {
        kept.renew = true;
        let (said, wait) = kept.said.clone().unwrap_or_else(|| carried.tooltip_words(item));
        simple.show(("item", item.id), said, wait);
        return;
    }
    let (said, wait) = carried.tooltip_words(item);
    kept.said = Some((said.clone(), wait));
    if std::mem::take(&mut kept.renew) {
        simple.show_anew(("item", item.id), said, wait);
    } else {
        simple.show(("item", item.id), said, wait);
    }
}
