//! The experience a thing taken from the world gives, as the battle screen tells it.
//! After `BaseStatisticsManager`; interactions spec, "Taking a thing up": the thing and everything
//! in it counted as looted (`OnGrabLoot`, from the inventory's `InProcess`), the thing itself as
//! found where it lands in a searchable grid (the search controller's `SetItemAsKnown` for a thing
//! it had never discovered, `OnItemFound`), each id once either way, each kind's share falling with
//! each time it is counted in the run (`GetMultForLootingAttempts`); what was gathered told every
//! second as one notice of each ("Looting experience - Items (10)", "Searching experience - Items
//! (4)": `Co_CalculateExperience`, `StatisticNotification`). What it counts is the character's
//! experience too (`heard`, read by `crate::experience`). With the `INVENTORY SYSTEM` (nothing is
//! taken without it).
//!
//! Counted: a thing taken up from the ground (`taken`), a thing a search
//! uncovers (`found`), a body's gear as its loot screen shows, any thing
//! that comes into the equipment by a move of the screen (a Ctrl click, a
//! drag, the menu's lines, a part of a stack split off), and a thing never
//! found come to lie in a searchable grid of the equipment (`count_moves`).
//! A thing put down and taken up again gives nothing more.

use std::collections::{HashMap, HashSet};

use bevy::prelude::*;
use trp_sim::items::inventory::{Id, Item};

use crate::screens::inventory_view::Carried;

pub struct StatisticsPlugin;

impl Plugin for StatisticsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Statistics>()
            .add_systems(Update, (count_moves, tell).chain().in_set(Told).after(crate::screens::inventory_view::ScreenDrawn).before(crate::weapons::wear::NoticesShown).run_if(crate::running));
    }
}

/// The statistics counted and told for the frame: what the experience
/// reads after (`crate::experience`).
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct Told;

/// Whether the character has Attention's elite, which doubles what a
/// count gives (`AttentionEliteExtraLootExp`).
pub fn elite(skills: Option<&crate::player::skills::Skills>) -> bool {
    let attention = skills.map_or(0, |skills| skills.level(trp_sim::condition::skills::Skill::Attention));
    trp_sim::condition::skills::buffs_of(trp_sim::condition::skills::Skill::Attention)
        .iter()
        .find(|buff| buff.id == "AttentionEliteExtraLootExp")
        .is_some_and(|buff| matches!(buff.value(attention), trp_sim::condition::skills::BuffValue::Flag(true)))
}

/// The run's statistics as far as they are told.
#[derive(Resource, Default)]
pub struct Statistics {
    /// How many times each kind has been counted this run, by its
    /// template (`CounterTag.LootItem`, looted and found alike).
    counts: HashMap<String, u32>,
    /// What has been counted as looted, and as found (`_lootedItemsIds`,
    /// `_foundItemsIds`): everything the character had as the run began
    /// among both (`MarkItemsAsFoundedAndLooted`).
    looted: HashSet<Id>,
    found: HashSet<Id>,
    /// The experience gathered and not yet told (`_lootingExp`,
    /// `_searchingExp`).
    looting: i32,
    searching: i32,
    /// When it is next told, the run's own having been counted.
    next: Option<f64>,
    /// Whether the run's own has been counted.
    begun: bool,
    /// What the loot beside the equipment held when it was last looked
    /// at: a thing come into the equipment from among it was moved there
    /// (`count_moves`).
    beside: HashSet<Id>,
    /// What the inventory and the world did that earns, for the experience
    /// to read (`crate::experience`; the game's statistics manager is where
    /// it all goes): the experience counted here, a thing looted the first
    /// time (Perception), found by a search (Attention, Search), a search's
    /// end, a magazine's rounds and checks, a thing examined, a door.
    pub heard: Vec<crate::player::experience::Happened>,
    /// `SIMPLE XP GAIN` in force (set by the experience): each count at its
    /// whole share, none falling with the kind's count.
    pub simple: bool,
}

/// Every item in a thing, the thing first (`Item.GetAllItems`).
fn every<'a>(item: &'a Item, out: &mut Vec<&'a Item>) {
    out.push(item);
    for inside in item.contents() {
        every(inside, out);
    }
}

/// A kind's experience the `count`-th time it is counted in the run: its
/// `LootExperience` times the share for that time (`exp.loot_attempts`,
/// nothing past them), rounded up as the game's Mono runtime computes the
/// product, in double precision (20 at the second time, 0.15: 3.0000001,
/// so 4: `courses10`'s `examine-looted-item`); twice that with Attention's
/// elite (`AttentionEliteExtraLootExp`).
fn experience(loot: i32, count: u32, attempts: &[f32], elite: bool, simple: bool) -> i32 {
    let share = if simple { 1.0 } else { count.checked_sub(1).and_then(|at| attempts.get(at as usize)).copied().unwrap_or(0.0) };
    let figure = (loot as f32 as f64 * share as f64).ceil() as i32;
    if figure > 0 && elite { figure * 2 } else { figure }
}

/// The install's share for each time a kind is counted (`exp.
/// loot_attempts`).
fn attempts(db: &trp_data::Database) -> Vec<f32> {
    db.config["exp"]["loot_attempts"].as_array().into_iter().flatten().map(|at| at["k_exp"].as_f64().unwrap_or(0.0) as f32).collect()
}

/// A kind's `LootExperience`.
fn loot_of(db: &trp_data::Database, item: &Item) -> i32 {
    db.item(&item.kind.template).ok().and_then(|template| template.prop_f32("LootExperience")).unwrap_or(0.0) as i32
}

impl Statistics {
    /// A thing taken from the world into the inventory: each item in it
    /// looted. Whether the thing is found as well goes by where it went
    /// (`look`: only into a searchable grid, and only the first time).
    pub fn taken(&mut self, item: &Item, db: &trp_data::Database, elite: bool) {
        let attempts = attempts(db);
        let mut all = Vec::new();
        every(item, &mut all);
        for each in &all {
            self.count(each, true, loot_of(db, each), &attempts, elite);
        }
    }

    /// A thing a search uncovered (`ActiveSearchContentOperation.
    /// DiscoverItem`, the search controller's `SetItemAsKnown` for a thing
    /// it had never discovered, `OnItemFound`): the thing alone found, not
    /// what it holds. A lucky search's finds as well (each discovered with
    /// its event).
    pub fn found(&mut self, item: &Item, db: &trp_data::Database, elite: bool, on_body: bool) {
        self.discover(item, loot_of(db, item), &attempts(db), elite, on_body);
    }

    /// A thing discovered with its event (`OnItemFound`): counted found,
    /// and, new to the count and not of a stack, Attention's find, and on
    /// a body Search's (`InventoryControllerOnItemFound`).
    fn discover(&mut self, item: &Item, loot: i32, attempts: &[f32], elite: bool, on_body: bool) {
        let new = self.count(item, false, loot, attempts, elite);
        if new && item.kind.stack_max <= 1 {
            self.heard.push(crate::player::experience::Happened::Act(trp_sim::condition::earning::Act::Found { on_body }));
        }
    }

    /// One item counted (`OnLootItem`): once as looted and once as found.
    /// Whether it was new to the count. A thing looted the first time, not
    /// ammunition or money, is Perception's unique loot (`OnUniqueLoot`).
    fn count(&mut self, item: &Item, grab: bool, loot: i32, attempts: &[f32], elite: bool) -> bool {
        let seen = if grab { &mut self.looted } else { &mut self.found };
        if !seen.insert(item.id) {
            return false;
        }
        let count = self.counts.entry(item.kind.template.clone()).or_default();
        *count += 1;
        let figure = experience(loot, *count, attempts, elite, self.simple);
        if figure > 0 {
            if grab {
                self.looting += figure;
            } else {
                self.searching += figure;
            }
            self.heard.push(crate::player::experience::Happened::Experience(figure));
        }
        if grab && !item.kind.class_names.iter().any(|class| class == "Ammo" || class == "Money") {
            self.heard.push(crate::player::experience::Happened::Act(trp_sim::condition::earning::Act::UniqueLoot));
        }
        true
    }
}

/// Where a thing lies in what holds it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Lies {
    Slot,
    Cell,
    /// Among a magazine's rounds.
    Loose,
}

/// The things in an item not yet counted as looted, each with where it
/// lies, outermost first (as `every`).
fn not_looted<'a>(item: &'a Item, looted: &HashSet<Id>, out: &mut Vec<(&'a Item, Lies)>) {
    let slots = item.slots.iter().flatten().map(|inside| (inside, Lies::Slot));
    let cells = item.grids.iter().flatten().map(|(_, inside)| (inside, Lies::Cell));
    let loose = item.loose.iter().map(|inside| (inside, Lies::Loose));
    for (inside, lies) in slots.chain(cells).chain(loose) {
        if !looted.contains(&inside.id) {
            out.push((inside, lies));
        }
        not_looted(inside, looted, out);
    }
}

/// The things lying in the grids of the searchable things in an item
/// (`SearchableGrid`: a vest's, a backpack's, the pockets', the secure
/// container's), at any depth.
fn in_searchable_grids<'a>(item: &'a Item, out: &mut Vec<&'a Item>) {
    if trp_sim::items::search::searchable(item) {
        out.extend(item.grids.iter().flatten().map(|(_, inside)| inside));
    }
    for inside in item.contents() {
        in_searchable_grids(inside, out);
    }
}

/// What a thing new to the equipment came to be there by.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Came {
    /// Moved from the loot beside it: looted (`OnGrabLoot` from the
    /// inventory's `InProcess`), and known there, so found already (a
    /// container's things by its search, a body's as its screen showed,
    /// the armory's as known: below), never again.
    Moved,
    /// A part of a stack split off into a grid or into a magazine's
    /// rounds: a new thing, looted (`SplitOperation`'s `InProcess`); found
    /// where it lands in a searchable grid (its `AddResult`, below).
    Split,
    /// Made there by the app (a round chambered, a part from the edit
    /// build screen): counted as the character's own, for nothing.
    Made,
}

fn came(item: &Item, lies: Lies, beside: &HashSet<Id>) -> Came {
    if beside.contains(&item.id) {
        Came::Moved
    } else if item.kind.stack_max > 1 && lies != Lies::Slot {
        Came::Split
    } else {
        Came::Made
    }
}

/// Counts the run's own as it begins (`MarkItemsAsFoundedAndLooted`:
/// looted and found, no kind's count raised), a body's gear as its loot
/// screen shows (`OnInteractWithLootContainer`: every item in it found,
/// `InteractionContextHelper`'s loot actions, as the screen is shown), and
/// what has come into the equipment since it was last looked at, by how
/// it came (`Came`), and what has come to lie in a searchable grid of the
/// equipment never found (`look`). The game counts each move as it is made
/// (any move whose place is the character's: `PlayerInventoryController.
/// InProcess`); the app looks at what the equipment holds once a frame
/// after the screen's moves. `PROVISIONAL`: the armory's things (known,
/// never searched: "Armory known") are discovered as they show, without
/// the event, and give looting alone, as a searched container's; a split
/// part told apart from a round the app makes by where it lies.
fn count_moves(mut statistics: ResMut<Statistics>, carried: Res<Carried>, game: Res<crate::GameData>, skills: Option<Res<crate::player::skills::Skills>>, features: Res<crate::screens::options_view::Features>) {
    if statistics.begun && !carried.is_changed() {
        return;
    }
    let db = &game.db;
    let loot = |item: &Item| loot_of(db, item);
    let shares = Shares { attempts: attempts(db), elite: elite(skills.as_deref()), loot: &loot };
    let body = carried.loot_open && carried.shows_body();
    if let Some((items, gave)) = statistics.look(&carried.equipment.item, &carried.loot, body, features.inventory, &shares, &carried.searcher) {
        info!("a body's gear shown: its {items} items found, {gave} experience");
    }
}

/// What a count goes by: the install's shares, Attention's elite, and
/// each kind's `LootExperience`.
struct Shares<'a> {
    attempts: Vec<f32>,
    elite: bool,
    loot: &'a dyn Fn(&Item) -> i32,
}

impl Statistics {
    /// One look at the equipment and the loot beside it (`count_moves`),
    /// the run's own counted at the first; with a body's gear shown, how
    /// many of its items were counted found as it showed, and what they
    /// gave.
    fn look(&mut self, equipment: &Item, loot: &Item, body: bool, counting: bool, shares: &Shares, searcher: &trp_sim::items::search::Searcher) -> Option<(usize, i32)> {
        if !self.begun {
            self.begun = true;
            let mut own = Vec::new();
            every(equipment, &mut own);
            for item in own {
                self.looted.insert(item.id);
                self.found.insert(item.id);
            }
        }
        let count = |statistics: &mut Self, item: &Item, grab: bool| {
            statistics.count(item, grab, (shares.loot)(item), &shares.attempts, shares.elite);
        };
        // A body's gear, as its loot screen shows.
        let mut shown = None;
        if counting && body && !self.found.contains(&loot.id) {
            let mut all = Vec::new();
            every(loot, &mut all);
            let was = self.searching;
            for item in &all {
                count(self, item, false);
            }
            shown = Some((all.len(), self.searching - was));
        }
        // What has come into the equipment.
        let mut new = Vec::new();
        not_looted(equipment, &self.looted, &mut new);
        for (item, lies) in new {
            let came = came(item, lies, &self.beside);
            if counting && came != Came::Made {
                count(self, item, true);
            }
            self.looted.insert(item.id);
            // Known where it lay beside (a bag's things still hidden in it
            // are not), or the character's own: never found again.
            if came == Came::Made || (came == Came::Moved && searcher.known(item.id)) {
                self.found.insert(item.id);
            }
        }
        // What lies in a searchable grid of the equipment (a vest's, a
        // backpack's, the pockets', the secure container's), known and
        // never found, is found now (`AddResult.RaiseEvents`: a move's or
        // an add's place is a searchable grid where the thing is not known,
        // `SetItemAsKnown`, and its first discovery raises `OnItemFound`):
        // a thing picked up or split off into one, or moved there later
        // from a slot. Not into a slot, a magazine or a case's grid
        // (`IsItemKnown`: any other place is known). The character's own
        // equipment is a body to the find's skills
        // (`InventoryControllerOnItemFound`: its owner's root).
        if counting {
            let mut landed = Vec::new();
            in_searchable_grids(equipment, &mut landed);
            let on_body = trp_sim::items::search::on_body(equipment);
            for item in landed {
                if searcher.known(item.id) && !self.found.contains(&item.id) {
                    self.discover(item, (shares.loot)(item), &shares.attempts, shares.elite, on_body);
                }
            }
        }
        // What the loot beside it holds now, for the next look.
        self.beside.clear();
        ids_in(loot, &mut self.beside);
        shown
    }
}

/// The numbers of an item and of everything in it.
fn ids_in(item: &Item, out: &mut HashSet<Id>) {
    out.insert(item.id);
    for inside in item.contents() {
        ids_in(inside, out);
    }
}

/// Every second, what was gathered told, looting before searching
/// (`Co_CalculateExperience`: from the session's start, each wait a second
/// of the game's clock from the frame the last ended). `PROVISIONAL`: the
/// app's run begins the session; the game's begins with the raid.
/// A notice the options' HUD page has turned off is not shown, and what it
/// would have told is let go all the same.
fn tell(mut statistics: ResMut<Statistics>, carried: Res<Carried>, time: Res<Time>, features: Res<crate::screens::options_view::Features>, options: Option<Res<crate::screens::options_view::OptionsState>>, mut feedback: ResMut<crate::weapons::wear::Feedback>) {
    let now = time.elapsed_secs_f64();
    if statistics.next.is_some_and(|next| now < next) {
        return;
    }
    statistics.next = Some(now + 1.0);
    if !features.inventory {
        return;
    }
    let word = |key: &str, otherwise: &str| carried.texts.say(key).unwrap_or_else(|| otherwise.to_owned());
    let items = word("StatsItems", "Items");
    use crate::screens::options_view::Notices;
    let shown = |which: usize| options.as_ref().is_none_or(|options| options.notices.on(which));
    if statistics.looting > 0 {
        if shown(Notices::LOOTING) {
            let said = format!("{} - {items} ({})", word("StatsLooting", "Looting experience"), statistics.looting);
            feedback.message(said);
        }
        statistics.looting = 0;
    }
    if statistics.searching > 0 {
        if shown(Notices::SEARCHING) {
            let said = format!("{} - {items} ({})", word("StatsSearching", "Searching experience"), statistics.searching);
            feedback.message(said);
        }
        statistics.searching = 0;
    }
}

#[cfg(test)]
mod tests;
