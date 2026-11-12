//! Examining things: what the character knows, an examine begun and ended with the game's notice.
//!
//! Examining in the app (inventory spec, "Examining"; `trp_sim::examine`):
//! the list of what the character knows, set as the run begins and kept in
//! the options' file; an examine asked from a thing's menu, the middle
//! button or its inspect window, begun with Attention's buff and ended
//! with the game's notice; and the world's, of a thing where it lies,
//! ended saying nothing.

use bevy::prelude::*;

use crate::screens::inventory_view::Carried;

/// Where what was learned is kept in the options' file (`options_view`'s,
/// which keeps it as it is when it writes the rest).
pub const KEPT_AS: &str = "examined";

/// What was learned, as the options' file keeps it, in the order learned.
pub fn kept(file: &std::path::Path) -> Vec<String> {
    let all = crate::platform::options_store::read(file);
    let list = all.as_ref().and_then(|all| all.get(KEPT_AS)?.as_array().cloned()).unwrap_or_default();
    list.iter().filter_map(|template| template.as_str().map(str::to_owned)).collect()
}

/// Keeps what is learned now in the options' file, all else in it as it
/// was.
pub fn keep(file: &std::path::Path) {
    crate::platform::options_store::keep(file, KEPT_AS, Some(serde_json::json!(trp_sim::items::examine::learned())), "what was examined");
}

/// Sets the list as the run begins (`EXAMINING` on): the new character's
/// of that edition and side, with what was
/// learned before; off, or where the install has no list, everything is
/// known.
pub fn begin_run(install: &trp_data::Install, edition: &str, side: &str, on: bool) {
    if !on {
        trp_sim::items::examine::know_everything();
        return;
    }
    match trp_data::character::inventory::starting_encyclopedia(install, edition, side) {
        Ok(start) => {
            let learned = kept(&crate::screens::options_view::options_file());
            println!("examining: the {edition} {side}'s {} kinds known at the start, {} learned since", start.len(), learned.len());
            trp_sim::items::examine::set_list(start, learned);
        }
        Err(e) => {
            eprintln!("examining: the starting character's list could not be read, so everything is known: {e:#}");
            trp_sim::items::examine::know_everything();
        }
    }
}

/// What the character carries in a slot as the run begins is known, at any
/// depth (a slot of one's own refuses a thing not known, rounds aside:
/// `Slot.CheckConditions`, out of a raid too; so the game's character can
/// carry none there). What lies in a grid is as the list has it.
pub fn know_carried(equipment: &trp_sim::items::inventory::Item) {
    fn walk(item: &trp_sim::items::inventory::Item, out: &mut Vec<String>) {
        for part in item.slots.iter().flatten() {
            if !trp_sim::items::inventory::is(part, "Ammo") {
                out.push(part.kind.template.clone());
            }
        }
        for inside in item.contents() {
            walk(inside, out);
        }
    }
    let mut templates = Vec::new();
    walk(equipment, &mut templates);
    trp_sim::items::examine::know_from_start(templates);
}

pub struct ExaminePlugin;

impl Plugin for ExaminePlugin {
    fn build(&self, app: &mut App) {
        // After the click that asks for one and before the page is drawn
        // afresh: the examination's view, the caption hidden and the blink
        // all on the release's frame, as the game's (`courses10`'s
        // `examine-multiple`, frame 150; a frame late before).
        app.add_systems(Update, (begin_asked, finish, warn_reload).chain().after(crate::screens::inventory_view::ScreenClicks).before(crate::screens::inventory_view::PageDrawn).run_if(crate::running));
        // Before the step, so that the pick-up's state is left and the
        // thing looked at again on the frame the examine ends.
        app.init_resource::<WorldExamine>().add_systems(Update, finish_world.before(crate::player::controller::step).run_if(crate::running));
    }
}

/// The thing being examined where it lies (the world's EXAMINE, begun by
/// `inventory_view::toggle` with the pick-up's action), whose end is
/// `finish_world`'s.
#[derive(Resource, Default)]
pub struct WorldExamine(pub Option<trp_sim::items::inventory::Id>);

/// Whether the examine under way ended on this frame: its time out on the
/// frame before (`out` keeps that frame's examine).
fn ends_now(examiner: &trp_sim::items::examine::Examiner, now: f64, out: &mut Option<(trp_sim::items::inventory::Id, u64)>) -> bool {
    let Some(under_way) = examiner.under_way.as_ref() else { return false };
    if now < under_way.began + under_way.time as f64 {
        return false;
    }
    let this = (under_way.item, under_way.began.to_bits());
    if *out != Some(this) {
        *out = Some(this);
        return false;
    }
    true
}

/// The world's examine ended, on the frame after its time ran out, as the
/// inventory's (`courses10`'s `examine-world-loot`: the panel gone on 464,
/// the state `Idle` and the prompt back on 465): its kind learned and kept,
/// the pick-up's state told (`PickUpState.Examine`: it is left), and
/// nothing said and nothing heard (`InventoryController.Examine` straight,
/// not `ItemUiContext.Examine`'s notice: the take has neither).
fn finish_world(mut carried: ResMut<Carried>, mut world: ResMut<WorldExamine>, mut controller: ResMut<crate::player::controller::Controller>, time: Res<Time<Real>>, mut out: Local<Option<(trp_sim::items::inventory::Id, u64)>>, mut statistics: ResMut<crate::player::statistics::Statistics>) {
    let Some(id) = world.0 else { return };
    let now = time.elapsed_secs_f64();
    if !carried.examiner.examining(id) {
        world.0 = None;
        return;
    }
    if !ends_now(&carried.examiner, now, &mut out) {
        return;
    }
    let Some(done) = carried.examiner.finish(now) else { return };
    world.0 = None;
    keep(&crate::screens::options_view::options_file());
    controller.movement.examined();
    // Its experience to the character at once (`ExamineOperation`).
    statistics.heard.push(crate::player::experience::Happened::Experience(done.kind.examine_experience as i32));
    let short = carried.texts.say(&format!("{} ShortName", done.kind.template)).unwrap_or_else(|| done.kind.name.clone());
    info!("examined where it lies: {short}");
    carried.note = format!("{short}: examined");
}

/// A reload refused for the weapon's own magazine not known: the game's
/// warning, its words the key itself (the install has none for it).
fn warn_reload(mut controller: ResMut<crate::player::controller::Controller>, mut feedback: ResMut<crate::weapons::wear::Feedback>) {
    if controller.reload_refused_unknown {
        controller.reload_refused_unknown = false;
        feedback.warning("Attached magazine is not examined.".to_owned());
    }
}

/// An examine asked (`InventoryController.Examine`): begun with
/// Attention's buff in force (stimulators' too), refused with the game's
/// notice while another is under way; nothing for a thing known.
fn begin_asked(mut carried: ResMut<Carried>, time: Res<Time<Real>>, skills: Option<Res<crate::player::skills::Skills>>, mut feedback: ResMut<crate::weapons::wear::Feedback>) {
    if carried.examine_asked.is_none() {
        return;
    }
    let Some(id) = carried.examine_asked.take() else { return };
    let Some(item) = carried.find(id).cloned() else { return };
    let attention = skills.map_or(0.0, |skills| skills.buffs().attention_examine);
    let now = time.elapsed_secs_f64();
    match carried.examiner.begin(&item, attention, now) {
        Ok(examination) => {
            let time = examination.time;
            carried.note = format!("examining, {time:.2} s");
            info!("examining {} for {time:.3} s (Attention's buff {attention:.2})", item.kind.template);
        }
        Err(trp_sim::items::examine::Refused::Busy) => {
            let words = carried.texts.say("You can't examine two items at the same time").unwrap_or_else(|| "You can't examine two items at the same time".to_owned());
            feedback.warning(words);
        }
        Err(trp_sim::items::examine::Refused::Known) => {}
    }
}

/// The examine under way ended on the frame after the one its time ran out
/// on: its kind learned and kept, every view drawn anew, and the game's
/// notice ("Item examined: ", its short name, its experience:
/// `ItemUiContext.Examine`). The battle screen's panel goes on the frame
/// its own clock runs out (`UsingPanel.Progress`, `using_view`), the thing
/// is known on the next (`courses10`'s `dead-scav-examine` 747 and 748,
/// `examine-looted-item` 1605 and 1606, `examine-multiple`'s four alike:
/// the operation's end is heard a frame after its delay is out).
/// The notice not shown where the options' HUD page has EXAMINE XP off.
pub(crate) fn finish(mut carried: ResMut<Carried>, world: Res<WorldExamine>, time: Res<Time<Real>>, options: Option<Res<crate::screens::options_view::OptionsState>>, mut feedback: ResMut<crate::weapons::wear::Feedback>, mut out: Local<Option<(trp_sim::items::inventory::Id, u64)>>, mut statistics: ResMut<crate::player::statistics::Statistics>) {
    let now = time.elapsed_secs_f64();
    // (The world's is `finish_world`'s.)
    if world.0.is_some_and(|id| carried.examiner.examining(id)) {
        return;
    }
    // Its time out on this frame: ended on the next.
    if !ends_now(&carried.examiner, now, &mut out) {
        return;
    }
    let Some(done) = carried.examiner.finish(now) else { return };
    keep(&crate::screens::options_view::options_file());
    let template = &done.kind.template;
    let said = carried.texts.say("Item examined: ").unwrap_or_else(|| "Item examined: ".to_owned());
    let short = carried.texts.say(&format!("{template} ShortName")).unwrap_or_else(|| done.kind.name.clone());
    if options.is_none_or(|options| options.notices.on(crate::screens::options_view::Notices::EXAMINING)) {
        feedback.message(format!("{said}{short} (+{} exp)", done.kind.examine_experience));
    }
    statistics.heard.push(crate::player::experience::Happened::Experience(done.kind.examine_experience as i32));
    carried.note = format!("{short}: examined");
}
