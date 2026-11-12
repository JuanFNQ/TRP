//! Slot keys, named builds, round times and the magazines' loading work.

use super::*;

/// The four slots with a key of their own and their bindings, as the
/// quick-use bar has them.
const SLOT_BINDINGS: [(&str, &str); 4] = [("Scabbard", "Knife"), ("Holster", "SecondaryWeapon"), ("FirstPrimaryWeapon", "PrimaryWeaponFirst"), ("SecondPrimaryWeapon", "PrimaryWeaponSecond")];

/// Keeps the words of the four slots' keys as the bindings have them.
pub(super) fn name_slot_keys(bindings: Res<Bindings>, mut carried: ResMut<Carried>) {
    if !bindings.is_changed() && !carried.slot_keys.is_empty() {
        return;
    }
    let named: Vec<(&'static str, String)> = SLOT_BINDINGS.iter().map(|(slot, binding)| (*slot, bindings.caption(binding).to_owned())).collect();
    if carried.slot_keys != named {
        carried.slot_keys = named;
    }
}

/// The builds a weapon's name takes the name of (`ItemFactory.
/// SetItemPresets`: each preset of no weapon's stock, or one that renames,
/// `_changeWeaponName`), in the install's order: each one's name (its id in
/// the install's words) and the templates of what it is made of but its
/// magazines and rounds (`ItemPreset.Children`).
pub fn named_builds(db: &trp_data::Database, texts: &trp_data::character::inventory::ItemTexts) -> Vec<(String, Vec<String>)> {
    db.presets
        .iter()
        .filter(|preset| preset.encyclopedia.is_none() || db.renaming.contains(&preset.id))
        .map(|preset| {
            let parts = preset
                .items
                .iter()
                .filter(|part| !db.class_names(&part.template).iter().any(|class| class == "Magazine" || class == "Ammo"))
                .map(|part| part.template.clone())
                .collect();
            (texts.say(&preset.id).unwrap_or_else(|| preset.id.clone()), parts)
        })
        .collect()
}

/// A name as the game's views give a thing, by the named builds
/// (`Carried::brief_name`).
pub(super) fn brief_name(named: &[(String, Vec<String>)], item: &Item, name: String) -> String {
    if !is(item, "Weapon") {
        return name;
    }
    fn gather<'a>(item: &'a Item, into: &mut std::collections::HashSet<&'a str>) {
        into.insert(item.kind.template.as_str());
        for part in item.slots.iter().flatten() {
            gather(part, into);
        }
    }
    let mut held = std::collections::HashSet::new();
    gather(item, &mut held);
    match named.iter().find(|(_, parts)| parts.iter().all(|part| held.contains(part.as_str()))) {
        Some((found, _)) if !found.is_empty() => {
            let cut = if found.chars().count() <= 30 { found.clone() } else { found.chars().take(28).chain("...".chars()).collect() };
            format!("{name} {cut}")
        }
        _ => name,
    }
}

/// Every magazine in or under an item (`EPlayerItems.NonQuestItems`'
/// magazines, for those carried as the run begins).
pub fn magazines_in(item: &Item) -> std::collections::HashSet<Id> {
    let mut found = std::collections::HashSet::new();
    let mut stack = vec![item];
    while let Some(item) = stack.pop() {
        if item.kind.class_names.iter().any(|class| class == "Magazine") {
            found.insert(item.id);
        }
        stack.extend(item.contents());
    }
    found
}

/// The pictures that have come since the screen last took them, or that
/// cannot be made, by template (`icons`): the boxes still showing a
/// spinner for one have it put in its place (`fill_pictures`).
#[derive(Resource, Default)]
pub struct PicturesArrived(pub Vec<String>);

/// The things whose box blinks (inventory spec, "A box that blinks"): a
/// move that waits on the hands, as the game's does (`ItemView.Blinking`,
/// a view being added or removed until its operation succeeds, which for
/// the character's own hands is when the weapon put in the slot they
/// last drew from has appeared in them, or the one taken out of its slot
/// has been put away and the next is in hand). Kept by `settle`, drawn by
/// `blink`; a blinking box takes no clicks (`ItemView.IsInteractive`).
#[derive(Resource, Default)]
pub struct Blinking(pub Vec<Id>);

/// The base times a magazine's round takes to load and to unload, from the
/// install's globals (`BaseLoadTime`, `BaseUnloadTime`), the elite level's
/// progress (`LoadTimeSpeedProgress`), and the time a magazine takes to
/// check (`BaseCheckTime`).
#[derive(Clone, Copy, Debug)]
pub struct RoundTimes {
    pub load: f32,
    pub unload: f32,
    pub progress: f32,
    pub check: f32,
    /// Whether a shot unknows the magazine it leaves neither empty nor full
    /// (`UncheckOnShot`).
    pub uncheck_on_shot: bool,
    /// Mag Drills' buffs as the skill's level gives them (`skills`): the
    /// percentages off a round's and a check's time, and the two elites.
    pub drills: trp_sim::condition::skills::MagDrills,
}

impl Default for RoundTimes {
    /// As the install first read has them.
    fn default() -> Self {
        Self { load: 0.85, unload: 0.3, progress: 1.0, check: 3.0, uncheck_on_shot: true, drills: trp_sim::condition::skills::MagDrills::default() }
    }
}

impl RoundTimes {
    pub fn of(db: &trp_data::Database) -> Self {
        let read = |name: &str, otherwise: f32| db.config.get(name).and_then(|value| value.as_f64()).map_or(otherwise, |value| value as f32);
        let base = Self::default();
        Self {
            load: read("BaseLoadTime", base.load),
            unload: read("BaseUnloadTime", base.unload),
            progress: read("LoadTimeSpeedProgress", base.progress),
            check: read("BaseCheckTime", base.check),
            uncheck_on_shot: db.config.get("UncheckOnShot").and_then(|value| value.as_bool()).unwrap_or(base.uncheck_on_shot),
            drills: base.drills,
        }
    }

    /// A check's time for a magazine of that modifier (`CheckTimeModifier`),
    /// less Mag Drills' check buff (0.8 a level).
    pub fn check(&self, modifier: f32) -> f32 {
        trp_sim::MagazineProcess::round_time(self.check, self.drills.check, modifier)
    }

    /// A round's time for a magazine of that modifier, less Mag Drills'
    /// load buff (0.6 a level).
    pub fn load(&self, modifier: f32) -> f32 {
        trp_sim::MagazineProcess::round_time(self.load, self.drills.load, modifier)
    }

    pub fn unload(&self, modifier: f32) -> f32 {
        trp_sim::MagazineProcess::round_time(self.unload, self.drills.unload, modifier)
    }

    /// The share of the first round's time each later one is quicker by, at
    /// Mag Drills' elite level (`MagDrillsLoadProgression`); a load only
    /// (an unload is never given it: `UnloadMagazineProcess(elite: false)`).
    pub fn load_elite(&self) -> Option<f32> {
        self.drills.load_progression.then_some(self.progress)
    }
}

/// A magazine at work, and what it stops on: the tab it began under, and
/// the medical items started then (`Controller::meds_started`).
pub struct MagazineWork {
    pub process: trp_sim::MagazineProcess,
    pub(super) began: Option<(usize, u32)>,
    /// What its load component shows (inventory spec, "The load
    /// component"): its figures as it began, and the time since.
    pub shown: Shown,
}

/// A magazine's work as its load component shows it: a round's time as
/// begun with, the rounds to go and those done then, and the seconds since.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Shown {
    pub round: f32,
    pub to_go: u32,
    pub done: u32,
    pub since: f32,
}

impl Shown {
    /// The share of the work done (the spinner's fill) and the share of the
    /// present round (the loader's turn).
    pub fn shares(&self) -> (f32, f32) {
        let before = self.round * self.done as f32;
        let at = self.since + before;
        let whole = self.round * self.to_go as f32 + before;
        let done = if whole > 0.0 { (at / whole).clamp(0.0, 1.0) } else { 1.0 };
        let round = if self.round > 0.0 { (at % self.round) / self.round } else { 0.0 };
        (done, round)
    }
}

impl MagazineWork {
    /// A work begun with so many rounds to go (one for a check).
    pub fn new(process: trp_sim::MagazineProcess, to_go: u32) -> Self {
        let round = process.round_wait();
        Self { process, began: None, shown: Shown { round, to_go, done: 0, since: 0.0 } }
    }
}

/// A magazine loaded or unloaded a round at a time (inventory spec,
/// "Loading and unloading a magazine"), or checked (inventory spec,
/// "Checking a magazine"): each round when its wait is over, with its
/// sound; a check when its one wait is, the magazine's count known from
/// then on and its drop sound played. Each wait counts from the frame the
/// work began on (its timer starts in the click's frame, after that
/// frame's time). Stopped where it is by the inventory shut, a tab
/// changed, a medical item starting its work, or a round that cannot move.
pub(super) fn work_magazine(time: Res<Time>, controller: Res<Controller>, selected: Res<Selected>, mut carried: ResMut<Carried>, mut sounds: MessageWriter<Sound>, mut statistics: ResMut<crate::player::statistics::Statistics>) {
    // Looked at first without a write, which would have the page drawn
    // afresh on every frame.
    if carried.magazine_work.is_none() {
        return;
    }
    // Its picture's key as it begins: held while it works.
    if let Some(magazine) = carried.magazine_work.as_ref().map(|work| work.process.magazine).filter(|magazine| !carried.frozen_keys.contains_key(magazine)) {
        if let Some(key) = carried.find(magazine).map(crate::screens::icons::picture_key) {
            let carried = carried.bypass_change_detection();
            carried.frozen_keys.retain(|id, _| *id == magazine);
            carried.frozen_keys.insert(magazine, key);
        }
    }
    let Some(work) = carried.bypass_change_detection().magazine_work.as_mut() else { return };
    let first = work.began.is_none();
    if first {
        info!("a magazine's {:?} begun, a wait of {:.3} s", work.process.work, work.process.round_wait());
    }
    if !first {
        work.shown.since += time.delta_secs();
    }
    let (tab, meds) = *work.began.get_or_insert((selected.0, controller.meds_started));
    if !controller.inventory_open || selected.0 != tab || controller.meds_started != meds {
        info!("a magazine's work stopped: the inventory shut, a tab changed or a medical item begun");
        carried.magazine_work = None;
        return;
    }
    if first || !work.process.step(time.delta_secs()) {
        return;
    }
    let (magazine, source, check) = (work.process.magazine, work.process.source(), work.process.work == trp_sim::items::magazine_process::Work::Check);
    let carried = &mut *carried;
    if check {
        carried.magazine_work = None;
        let Some(found) = carried.find(magazine) else { return };
        let (name, group) = (carried.name(found), found.kind.sound.clone());
        info!("a magazine checked: {name}");
        // Its first check: Mag Drills' (`MagazineCheckAction`).
        if carried.checked.insert(magazine) {
            statistics.heard.push(crate::player::experience::Happened::Act(trp_sim::condition::earning::Act::MagazineChecked));
        }
        // An event of the inventory, known before or not (the check's
        // refresh): the weight read again (`weigh_carried`).
        carried.weighed.forget();
        carried.note = format!("Magazine checked: {name}");
        sounds.write(Sound::Item(group, Way::Drop));
        return;
    }
    let worn = carried.equipment.item.id;
    let stacks_of = |carried: &Carried| carried.find(magazine).map_or(0, |found| found.loose.len());
    let stacks_before = stacks_of(carried);
    let roots = &mut [&mut carried.equipment.item, &mut carried.loot];
    let (moved, sound) = match source {
        Some(source) => (trp_sim::items::inventory::load_round(roots, source, magazine).is_ok(), Sound::Load),
        None => (trp_sim::items::inventory::unload_round(roots, magazine, worn, &carried.rules).is_ok(), Sound::Unload),
    };
    let left = roots.iter().find_map(|root| root.find(magazine)).map_or(0, trp_sim::items::inventory::magazine_rounds);
    // A magazine left empty or full by a round in or out is known
    // (`CheckMagazineAmmoDepend`, as each add or removal raises it).
    let capacity = roots.iter().find_map(|root| root.find(magazine)).map_or(0, |found| found.kind.capacity);
    if moved && trp_sim::items::inventory::known_after(left, capacity, carried.checked.contains(&magazine), false) {
        carried.checked.insert(magazine);
    }
    let done = match carried.magazine_work.as_mut() {
        Some(work) if moved => work.process.moved() || (source.is_none() && left == 0),
        _ => true,
    };
    if moved {
        sounds.write(sound);
        // A round loaded or unloaded: Mag Drills' (`RaidLoadedAmmoAction`,
        // `RaidUnloadedAmmoAction`).
        let act = if source.is_some() { trp_sim::condition::earning::Act::RoundLoaded } else { trp_sim::condition::earning::Act::RoundUnloaded };
        statistics.heard.push(crate::player::experience::Happened::Act(act));
    }
    // Its picture asked again where the game asks: a stack added or taken
    // away; at the end (the load's last round, an unload left empty, or a
    // round that could not move) its own key again.
    if done {
        carried.frozen_keys.remove(&magazine);
    } else if moved && stacks_of(carried) != stacks_before {
        if let Some(key) = carried.find(magazine).map(crate::screens::icons::picture_key) {
            carried.frozen_keys.insert(magazine, key);
        }
    }
    if done {
        info!("a magazine's work over: {left} rounds in it{}", if moved { "" } else { " (a round could not move)" });
        carried.magazine_work = None;
    }
}
