//! Searching containers and bodies, as the game's searchable views do it.
//!
//! Searching in the app (inventory spec, "Searching a container",
//! "Searching a body"; `trp_sim::search`): every searchable thing whose
//! search the screen shows (each a `SearchableView` in the game: the loot
//! panel's container, a body's vest, pockets and backpack, the character's
//! own containers' rows, a container's window) searched as the game's view
//! of it searches: the loot panel's container begun as it is shown
//! (`SimpleStashPanel.Show`, a body's never), any of them by a click on its
//! SEARCH button or its unsearched panel, stopped by its stop button or as
//! its view goes (`SearchableView.Close`); one at a time, two at Search's
//! elite; moved on by the real clock with Attention's and Search's buffs
//! (stimulators' with them; Search's only where the container lies on a
//! body, the character's own included), each thing found with its own drop
//! sound, a lucky search with the game's luck, and the last searched
//! container's search sound looping while any search is under way
//! (`Player.InventoryControllerOnSearchEvent`).

use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;
use trp_sim::items::inventory::Id;
use trp_sim::items::search::Event;

use crate::screens::inventory_view::{Carried, SearchClick};
use crate::audio::ui_sound::{Cut, Level, Sound, Way};

/// How long the search sound plays at its volume twice over before it is
/// at its volume once (`Player.SupportAudioSourceCoroutine`: the source's
/// base volume set to `SearchSoundVolume` and then played at that volume
/// again, `SimpleSource.Play`, 0.6 x 0.6 = 0.36, as `courses10`'s searches
/// record it; the routine's next turn, after `WaitForSeconds(0.5)`, sets
/// the base volume alone, 0.6: read from the code, not heard in the takes).
const LOUDER_AFTER: f64 = 0.5;

pub struct SearchPlugin;

impl Plugin for SearchPlugin {
    fn build(&self, app: &mut App) {
        // Before the page is drawn: a thing found shows on the frame its
        // sound plays (`courses10`'s searches: the view, its kind's filter
        // and its drop on one frame), and before the windows are placed (a
        // window grown by its search's end keeps its middle on that frame).
        app.add_systems(
            Update,
            (clicks, follow).chain().after(crate::screens::inventory_view::ScreenClicks).before(crate::screens::inventory_view::WindowsFitted).before(crate::screens::inventory_view::PageDrawn).run_if(crate::running),
        );
    }
}

/// A click let go on a part of a search (`SearchableView`: the unsearched
/// panel and the button begin one, the stop button stops it).
fn clicks(buttons: Res<ButtonInput<MouseButton>>, hits: Query<(&SearchClick, &RelativeCursorPosition)>, mut carried: ResMut<Carried>) {
    if !buttons.just_released(MouseButton::Left) {
        return;
    }
    if let Some((hit, _)) = hits.iter().find(|(_, at)| at.cursor_over()) {
        carried.search_asked = Some((hit.0, hit.1 == crate::screens::health_view::SearchHit::Begin));
    }
}

/// What the searching keeps from frame to frame: the searches' views shown
/// last frame, the loot panel's container then (searched as it is shown),
/// and the search sound playing, if one is, with its own level until it is
/// raised (`LOUDER_AFTER`) and when it began.
#[derive(Default)]
struct Following {
    shown: Vec<Id>,
    simple: Option<Id>,
    sound: Option<Cut>,
    quieter: Option<(Level, f64)>,
    /// The screen's tab last frame.
    tab: usize,
}

/// The skills' share in a search, as the character has them now.
fn skills_now(skills: Option<&crate::player::skills::Skills>) -> trp_sim::items::search::Skills {
    skills.map_or_else(Default::default, |skills| {
        let buffs = skills.buffs();
        trp_sim::items::search::Skills { loot_speed: buffs.attention_search, body_speed: buffs.search_speed, lucky: buffs.attention_lucky, double: buffs.search_double }
    })
}

/// The searchable things whose search the screen shows now: with the loot
/// panel open its container, or a body's containers that the looter sees;
/// the character's own containers' rows; each container's window.
fn views_shown(carried: &Carried) -> Vec<Id> {
    let mut shown = Vec::new();
    let searchable = |item: &trp_sim::items::inventory::Item| trp_sim::items::search::searchable(item).then_some(item.id);
    if carried.loot_open {
        if carried.shows_body() {
            let body = &carried.loot;
            for slot in trp_sim::items::search::BODY_CONTAINERS.into_iter().filter(|slot| trp_sim::items::search::seen_on_body(slot, true)) {
                shown.extend(body.in_slot(slot).and_then(searchable));
            }
        } else {
            shown.push(carried.loot.id);
        }
    }
    for slot in ["TacticalVest", "Pockets", "Backpack", "SecuredContainer"] {
        shown.extend(carried.equipment.item.in_slot(slot).and_then(searchable));
    }
    for window in carried.windows.iter().filter(|window| window.kind == crate::screens::inventory_view::WinKind::Grids) {
        shown.extend(carried.find(window.item).and_then(searchable));
    }
    shown
}

#[allow(clippy::too_many_arguments)]
fn follow(
    mut carried: ResMut<Carried>,
    controller: Res<crate::player::controller::Controller>,
    time: Res<Time<Real>>,
    skills: Option<Res<crate::player::skills::Skills>>,
    game: Res<crate::GameData>,
    mixer: Option<Res<crate::audio::ui_sound::Mixer>>,
    selected: Res<crate::screens::inventory_view::Selected>,
    mut sounds: MessageWriter<Sound>,
    mut tint: ResMut<crate::screens::health_view::SearchTint>,
    opening: Res<crate::screens::inventory_view::Opening>,
    (mut statistics, features): (ResMut<crate::player::statistics::Statistics>, Res<crate::screens::options_view::Features>),
    mut state: Local<Following>,
) {
    let now = time.elapsed_secs_f64();
    // The views are on the gear tab and the health tab, each its own
    // showing of the items panel (`ItemsTabController`: a tab left hides
    // the panel, `TryHide`, and closes its views).
    let tab = selected.0;
    let open = controller.inventory_open && (tab == crate::screens::inventory_view::GEAR || tab == crate::screens::inventory_view::HEALTH);
    let quiet = carried.searcher.under_way.is_empty() && carried.search_asked.is_none() && (!open && state.shown.is_empty());
    if quiet && state.sound.is_none() {
        return;
    }
    let shown = if open { views_shown(&carried) } else { Vec::new() };
    // (The loot panel shown a frame after the page: `Opening`.)
    let simple = (open && carried.loot_open && !carried.shows_body() && opening.stage() >= 2).then_some(carried.loot.id);
    let elite = crate::player::statistics::elite(skills.as_deref());
    let skills = skills_now(skills.as_deref());
    // (Found things counted only with the inventory's experience kept.)
    let mut counting = features.inventory.then(|| Counting { statistics: &mut statistics, db: &game.db, elite });
    let mut changed = false;
    // Written to without telling the screen, which is told below only if
    // something it shows has changed.
    let carried_now = carried.bypass_change_detection();
    // Another tab: the items panel hidden and, on the other of its two
    // tabs, shown afresh: every search stops with its view, and the loot
    // panel's container is shown anew.
    if tab != state.tab {
        if !carried_now.searcher.under_way.is_empty() {
            carried_now.searcher.stop_all();
            changed = true;
        }
        state.shown.clear();
        state.simple = None;
        state.tab = tab;
    }
    // A search whose view is no longer shown stops (`SearchableView.Close`).
    if shown != state.shown {
        let gone: Vec<Id> = carried_now.searcher.under_way.iter().map(|search| search.container).filter(|id| !shown.contains(id)).collect();
        for id in gone {
            changed |= carried_now.searcher.stop(id);
        }
        state.shown = shown.clone();
    }
    // The loot panel's container, shown and not yet searched, is searched
    // (`SimpleStashPanel.Show`).
    if simple != state.simple {
        if let Some(id) = simple.filter(|&id| !carried_now.searcher.searched(id)) {
            changed |= begin(carried_now, id, &skills, now, &mut sounds, &mut counting);
        }
        state.simple = simple;
    }
    // A click on a search's parts.
    if let Some((id, start)) = carried_now.search_asked.take() {
        if !start {
            changed |= carried_now.searcher.stop(id);
        } else if shown.contains(&id) && carried_now.searcher.searching(id).is_none() {
            changed |= begin(carried_now, id, &skills, now, &mut sounds, &mut counting);
        }
    }
    // The searches moved on to now.
    if !carried_now.searcher.under_way.is_empty() {
        let mut searcher = std::mem::take(&mut carried_now.searcher);
        let events = searcher.advance(now, |id| carried_now.find(id));
        carried_now.searcher = searcher;
        changed |= !events.is_empty();
        // A container opened while searched: its view brought up to date
        // (`SearchableView.ItemSearchedHandler`), its button's tint with it.
        tint.renewed.extend(events.iter().filter_map(|event| if let Event::Opened(id) = event { Some(*id) } else { None }));
        tell(carried_now, &events, &mut sounds, &mut counting);
        // A timer's second moved on, or a timer's first frame past (it
        // shows its words from the next).
        let was = carried_now.search_clock;
        if carried_now.searcher.under_way.iter().any(|search| (now - search.began).floor() != (was - search.began).floor() || (was <= search.began && now > search.began)) {
            changed = true;
        }
    }
    if changed {
        carried_now.search_clock = now;
        carried.set_changed();
    }
    // The words the loot panel's timer is left with, as it was last
    // updated (`TimerText.Update`).
    let carried_now = carried.bypass_change_detection();
    if let Some(search) = simple.and_then(|id| carried_now.searcher.searching(id)).filter(|search| now > search.began) {
        let gone = (now - search.began).max(0.0);
        let said = format!("{}:{:02}", (gone / 60.0) as u32, gone as u32 % 60);
        if carried_now.search_timer_said.as_deref() != Some(said.as_str()) {
            carried_now.search_timer_said = Some(said);
        }
    }
    // The search sound: the last container searched's, looping from the
    // first search begun until none is under way (`SearchSoundVolume`'s
    // first-person volume; the source's own group, the player's movement),
    // at that volume twice over for its first `LOUDER_AFTER`.
    let searching = carried.searcher.under_way.last().map(|search| search.container);
    match (searching, state.sound.is_some()) {
        (Some(id), false) => {
            let clip = carried.find(id).map(|container| container.kind.search_sound.clone()).filter(|clip| !clip.is_empty());
            if let Some(clip) = clip {
                let cut = crate::audio::ui_sound::no_cut();
                let volume = trp_data::level::loot::search_sound_volume(&game.db.client);
                let level: Level = Default::default();
                crate::audio::ui_sound::set_level(&level, 20.0 * volume.max(1e-6).log10());
                sounds.write(Sound::Looping(clip, volume, cut.clone(), level.clone()));
                state.sound = Some(cut);
                state.quieter = Some((level, now));
            }
        }
        (None, true) => {
            if let (Some(cut), Some(mixer)) = (state.sound.take(), mixer) {
                cut.store(mixer.0.clock(), std::sync::atomic::Ordering::Release);
            }
            state.quieter = None;
        }
        _ => {}
    }
    if state.quieter.as_ref().is_some_and(|(_, began)| now - began >= LOUDER_AFTER) {
        if let Some((level, _)) = state.quieter.take() {
            crate::audio::ui_sound::set_level(&level, 0.0);
        }
    }
}

/// Begins a search of a container shown; whether it began (not where as
/// many are under way as the character may make at once). Its pace has
/// Search's speed where the container lies on a body (`trp_sim::search::
/// on_body`: what holds it at the top is an equipment, the character's
/// own or a body's). It runs from the next frame on (`trp_sim::search`:
/// the game's `Task.Yield`), a search over at once finding everything then.
fn begin(carried: &mut Carried, id: Id, skills: &trp_sim::items::search::Skills, now: f64, sounds: &mut MessageWriter<Sound>, counting: &mut Option<Counting>) -> bool {
    let (container, on_body) = if let Some(container) = carried.equipment.item.find(id) {
        (container.clone(), trp_sim::items::search::on_body(&carried.equipment.item))
    } else if let Some(container) = carried.loot.find(id) {
        (container.clone(), trp_sim::items::search::on_body(&carried.loot))
    } else {
        return false;
    };
    let mut searcher = std::mem::take(&mut carried.searcher);
    let begun = searcher.begin(&container, on_body, skills, now);
    // The searches moved on to now, the new one with them (every one under
    // way, its container found wherever it lies).
    let found = if begun.is_ok() { searcher.advance(now, |at| carried.find(at)) } else { Vec::new() };
    carried.searcher = searcher;
    let Ok(events) = begun else { return false };
    let lucky = carried.searcher.searching(id).is_some_and(|search| search.instant);
    info!(
        "searching the {}: speed {:.2}{}{}",
        container.kind.name,
        skills.speed(on_body),
        if on_body { " (on a body)" } else { "" },
        if lucky { ", over at once" } else { "" }
    );
    tell(carried, &events, sounds, counting);
    tell(carried, &found, sounds, counting);
    true
}

/// Where a thing found is counted for its experience (`statistics`).
struct Counting<'a> {
    statistics: &'a mut crate::player::statistics::Statistics,
    db: &'a trp_data::Database,
    elite: bool,
}

/// What a search did, heard and counted: the luck (`looting_luck2`, as a
/// thing's other sound), each thing found by its own drop, and each found
/// as the statistics' (`BaseStatisticsManager.OnItemFound`, on the search
/// controller's `OnItemFound`: a lucky search's finds as well).
fn tell(carried: &Carried, events: &[Event], sounds: &mut MessageWriter<Sound>, counting: &mut Option<Counting>) {
    for event in events {
        match event {
            Event::Lucky(_) => {
                sounds.write(Sound::Item("looting_luck2".to_owned(), Way::Other));
            }
            Event::Found { item, sounded, .. } => {
                let Some(found) = carried.find(*item) else { continue };
                if *sounded {
                    sounds.write(Sound::Item(found.kind.sound.clone(), Way::Drop));
                }
                if let Some(counting) = counting.as_mut() {
                    // On a body where what holds it at the top is an
                    // equipment (Attention's `FindActionTrue`, Search's find).
                    let root = if carried.equipment.item.find(*item).is_some() { &carried.equipment.item } else { &carried.loot };
                    counting.statistics.found(found, counting.db, counting.elite, trp_sim::items::search::on_body(root));
                }
            }
            // A search run to its end: Search's (`SearchAction`); not one
            // whose container went.
            Event::Ended(container) if carried.find(*container).is_some() => {
                if let Some(counting) = counting.as_mut() {
                    counting.statistics.heard.push(crate::player::experience::Happened::Act(trp_sim::condition::earning::Act::Searched));
                }
            }
            _ => {}
        }
    }
}
