//! Repairing at the workbench with a repair kit, in the game's repair window.
//!
//! By `docs/spec/workbench.md`, "The repair window", the app's own design:
//! in the screen opened from the bench's `REPAIR GEAR`, a kit let go on a
//! thing it mends opens the game's repair window for that thing with that
//! kit (`ItemView`'s drop, `RepairItemContext`), its kits the ones of the
//! same kind carried (the game's are the stash's: here as in a raid), and
//! its figures, its warnings, its slider and its button as
//! the game's scripts make them (`trp_sim::repair`; `health_view::
//! repair_built`). REPAIR makes the repair as the server of the player's
//! install does, with the game's sound and notice.

use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;
use trp_sim::items::inventory::{is, Id, Item, Repairable};
use trp_sim::items::repair::{self, Plan, Repairer};
use trp_sim::condition::skills::Skill;

use crate::screens::health_view::{RepairCondition, RepairWords, Words};
use crate::screens::inventory_view::{Carried, WinKind};

pub struct RepairViewPlugin;

impl Plugin for RepairViewPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (check_open, check_buffed, keep_repairers, clicks).chain().run_if(crate::running));
    }
}

/// A repair window's own state: its thing, the kit let go on it, the
/// slider's value (a fraction of the template durability; none until it is
/// first laid out, when it goes to its top: `UpdateDegradationPrediction`
/// with `setMaxValue`), and the character as the repair goes by them.
#[derive(Clone, Debug, PartialEq)]
pub struct Repair {
    pub thing: Id,
    pub kit: Id,
    pub selected: Option<f32>,
    pub who: Option<Repairer>,
}

/// The install's words the window is made of.
pub fn word_keys() -> Vec<String> {
    let mut keys: Vec<String> = [
        "REPAIR: ",
        "Unknown item",
        " kg",
        "Instant",
        "repair speed",
        "repair quality",
        "repairability",
        "Repair/EnhancementChance",
        "REPAIR POINTS IN STASH: ",
        "REPAIR POINTS TO PAY: ",
        "SELECT REPAIRER",
        "SELECT DESIRED CONDITION",
        "TECHNICAL CONDITION",
        "REPAIR TIME: ",
        "DP TO REPAIR: ",
        "Repair will take out {0} to {1} points of maximum durability",
        "Repair/RepairKit/CanEnhance",
        "Repair/HasCommonBuff",
        "Repair/HasRareBuff",
        "Item successfully repaired to",
        "BrokenItem",
        "NoCorrespondingArea",
        "very bad",
        "bad",
        "fine",
        "good",
        "very good",
    ]
    .map(str::to_owned)
    .into();
    for quality in ["Terrible", "Subpar", "Average", "Fine", "Excellent"] {
        keys.push(format!("rQuality{quality}"));
    }
    for text in ["Very good", "Good", "Average", "Bad", "Very bad", "Terrible", "Unpredictable"] {
        keys.push(format!("repairability/{text}"));
    }
    for chance in ["very low", "low", "average", "high"] {
        keys.push(format!("enhancementChance/{chance}"));
    }
    keys
}

/// The labels the install's words fill (`LocalizedText`), by their paths:
/// a kit's in place of the cash's (`TraderSelected`).
const LABELS: [(&str, &str); 7] = [
    ("Inner/Contents/RepairerPanel/RepairerLabelPanel/RepairerLabel", "SELECT REPAIRER"),
    ("Inner/Contents/RepairerPanel/ExceptionItemGroup/ConditionPanel/RepairerLabel", "SELECT DESIRED CONDITION"),
    ("Inner/Contents/RepairerPanel/ExceptionItemGroup/ConditionPanel/ConditionSlider/Name", "TECHNICAL CONDITION"),
    ("Inner/Contents/RepairerPanel/ExceptionItemGroup/RepairSumPanel/RepairTime/Label", "REPAIR TIME: "),
    ("Inner/Contents/RepairerPanel/ExceptionItemGroup/RepairSumPanel/RepairDP/Label", "DP TO REPAIR: "),
    ("Inner/Contents/RepairerPanel/ExceptionItemGroup/RepairSumPanel/CashInStash/Label", "REPAIR POINTS IN STASH: "),
    ("Inner/Contents/RepairerPanel/ExceptionItemGroup/RepairSumPanel/TotalSum/Label", "REPAIR POINTS TO PAY: "),
];

/// A kit's resource now (`RepairKitComponent.Resource`: its greatest as made).
pub fn kit_resource(kit: &Item) -> f32 {
    kit.resource.unwrap_or(kit.kind.repair_resource)
}

/// The kits a repair window offers (`FormRepairKitsCollection`, one
/// collection of the kit let go's kind): those of its kind carried with
/// some resource left, the one let go first and the rest least first
/// (`RepairKitsCollection.RepairItems`).
pub fn kits_of(carried: &Carried, repair: &Repair) -> Vec<(Id, f32)> {
    let Some(dragged) = carried.find(repair.kit).filter(|kit| kit_resource(kit) > 0.0) else { return Vec::new() };
    let mut found = Vec::new();
    fn walk(item: &Item, template: &str, out: &mut Vec<(Id, f32)>) {
        if item.kind.template == template && kit_resource(item) > 0.0 {
            out.push((item.id, kit_resource(item)));
        }
        for inside in item.contents() {
            walk(inside, template, out);
        }
    }
    walk(&carried.equipment.item, &dragged.kind.template, &mut found);
    if !found.iter().any(|(id, _)| *id == dragged.id) {
        found.push((dragged.id, kit_resource(dragged)));
    }
    found.sort_by(|a, b| a.1.total_cmp(&b.1));
    if let Some(at) = found.iter().position(|(id, _)| *id == dragged.id) {
        let first = found.remove(at);
        found.insert(0, first);
    }
    found
}

/// A figure as C# writes a rounded double: no decimals where whole.
fn figure(value: f64, places: i32) -> String {
    let scale = 10f64.powi(places);
    crate::screens::health_view::trimmed_figure(((value * scale).round() / scale) as f32)
}

/// Everything the window shows for a thing, worked out as its scripts do;
/// and the slider's top (a fraction), for its clicks.
#[allow(clippy::too_many_arguments)]
pub fn repair_words(carried: &Carried, item: &Item, repair: &Repair, db: &trp_data::Database, said: &Words, loading: bool) -> Option<(RepairWords, f32)> {
    let settings = carried.repair_settings.as_ref()?;
    let plan = repair::plan_of(item, db, settings)?;
    let who = repair.who.unwrap_or_default();
    let word = |key: &str| said.word(key, key);
    let template = plan.template_durability();
    let accept = plan.can_accept(settings, &who);
    let top = accept / template.max(1.0);
    let selected = repair.selected.unwrap_or(top).clamp(0.0, top.max(0.0));
    let amount = selected * template;
    let range = plan.degradation(amount, settings, &who);
    let kits = kits_of(carried, repair);
    let points: f32 = kits.iter().map(|(_, resource)| resource).sum();
    let price = plan.price(amount, settings, &who);
    let kit = carried.find(repair.kit);
    let first = kits.first().and_then(|(id, _)| carried.find(*id));
    let charge = first.map_or(String::new(), |kit| {
        let (resource, most) = (kit_resource(kit), kit.kind.repair_resource);
        let shown = if resource / most.max(1.0) > 0.5 { resource.floor() } else { resource.ceil() };
        format!("<color=#ffffff>{shown}</color><color=#ffffff>/{most}</color>")
    });
    let line = |value: &str, label: &str| format!("<color=#c5c3b4>{}</color> {}", word(value), word(label));
    let (_, display) = plan.quality(settings, &who);
    let enhancement = plan.enhancement_chance(amount, settings, &who);
    let lines = [
        line("Instant", "repair speed"),
        line(&format!("rQuality{}", repair::quality_words(display)), "repair quality"),
        line(&format!("repairability/{}", plan.repairability()), "repairability"),
        enhancement.map_or(String::new(), |chance| line(repair::enhancement_words(chance), "Repair/EnhancementChance")),
    ];
    // The warnings (`UpdateRepairKitLabels`): a kit's points short and
    // nothing to repair kept but not shown; a broken item shown; with
    // neither, the greatest durability the repair takes, and whether an
    // enhancement may come. (The hideout's zone is the bench here: the
    // user's design; never missing.)
    let short = price > points as f64;
    let nothing = selected <= 0.0;
    let broken = plan.broken();
    let mut warnings = Vec::new();
    if broken {
        warnings.push((0, word("BrokenItem")));
    }
    if !short && !nothing {
        warnings.push((1, word("Repair will take out {0} to {1} points of maximum durability").replacen("{0}", &figure(range[0] as f64, 1), 1).replacen("{1}", &figure(range[1] as f64, 1), 1)));
        if enhancement.is_some() {
            warnings.push((2, word("Repair/RepairKit/CanEnhance")));
        }
    }
    // The thing's own buff (`UpdateBuffState`: `GetBuffState`, the thing's,
    // an armour's own not its plates'): "Enhanced" on the common's panel,
    // "Rare enhancement" on the rare's, after the others (made after them
    // as the window opens).
    match item.buff.map(|buff| buff.rarity) {
        Some(repair::Rarity::Common) => warnings.push((3, word("Repair/HasCommonBuff"))),
        Some(repair::Rarity::Rare) => warnings.push((4, word("Repair/HasRareBuff"))),
        None => {}
    }
    let current = plan.durability();
    let most = plan.max_durability();
    let durability = format!("{}/{}<color=white> ({})</color>", figure(current as f64, 2), figure(most as f64, 2), template.round() as i32);
    let repair_dp = format!("{} + {} \u{2248} {}", figure(current as f64, 1), figure(amount as f64, 1), figure((current as f64 * 10.0).round() / 10.0 + (amount as f64 * 10.0).round() / 10.0, 1));
    let name = carried.texts.names.get(&item.kind.template).cloned().unwrap_or_else(|| carried.name(item));
    let words = RepairWords {
        caption: format!("{}{}", word("REPAIR: "), name),
        kind: carried.texts.kinds.get(&item.kind.template).cloned().unwrap_or_default(),
        weight: format!("{:.3}{}", item.weight(), carried.texts.kg),
        mastering: is(item, "Weapon").then(|| format!("0{}", carried.mastery.shown(&item.kind.template).level + 1)),
        loading,
        repairing: false,
        warnings,
        dropdown: kit.map(|kit| carried.view_name(kit).trim().to_owned()).unwrap_or_default(),
        charge,
        price_rate: word(repair::price_rate_words(plan.price_rate(settings, &who))),
        lines,
        labels: LABELS.iter().map(|(path, key)| ((*path).to_owned(), word(key))).collect(),
        condition: RepairCondition { current: current / template.max(1.0), max: most / template.max(1.0), top, selected, range, template },
        durability,
        repair_time: "0".to_owned(),
        repair_dp,
        in_stash: crate::screens::health_view::trimmed_figure(points),
        to_pay: figure(price, 2),
        to_pay_critical: short,
        broken,
        interactable: !short && !nothing && !kits.is_empty(),
    };
    Some((words, top))
}

/// The character as a repair goes by them, put on each window that has not
/// got it yet: the governing skills', Intellect's and Charisma's levels;
/// no hideout's bonus (`PROVISIONAL`: the range has no hideout, the bench
/// standing for its zone only); for the server's buff the character's
/// level (the options' SKILLS page's LEVEL) and the three skills' progress
/// (nought with `SKILLS` off). A window whose thing or kit has gone, or
/// that has shut, is let go.
fn keep_repairers(mut carried: ResMut<Carried>, skills: Option<Res<crate::player::skills::Skills>>, experience: Option<Res<crate::player::experience::Experience>>) {
    let open: Vec<Id> = carried.windows.iter().filter(|window| window.kind == WinKind::Repair).map(|window| window.item).collect();
    if carried.repairs.iter().any(|repair| !open.contains(&repair.thing)) {
        carried.repairs.retain(|repair| open.contains(&repair.thing));
    }
    if carried.repairs.iter().all(|repair| repair.who.is_some()) {
        return;
    }
    let level = |skill: Skill| skills.as_deref().map_or(0, |skills| skills.level(skill));
    let progress = |skill: Skill| skills.as_deref().filter(|skills| skills.on()).map_or(0.0, |skills| skills.levels.progress(skill));
    let who = Repairer {
        levels: [level(Skill::WeaponTreatment), level(Skill::LightVests), level(Skill::HeavyVests), level(Skill::Intellect)],
        intellect: level(Skill::Intellect),
        charisma: level(Skill::Charisma),
        bonus: 0.0,
        profile_level: experience.as_deref().map_or(0, crate::player::experience::Experience::level),
        progress: [progress(Skill::WeaponTreatment), progress(Skill::LightVests), progress(Skill::HeavyVests)],
    };
    for repair in carried.repairs.iter_mut().filter(|repair| repair.who.is_none()) {
        repair.who = Some(who);
    }
}

/// A repair window's places that take the mouse: its slider's track, the
/// button to its top, its REPAIR; by the window's number.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub enum RepairSpot {
    Track(usize),
    Top(usize),
    Press(usize),
}

/// The window's clicks: a press on the slider's track moves its handle
/// there and a drag on, as the engine's slider does; its top's button
/// puts it at its top; REPAIR, where it can be pressed, makes the repair
/// (`RepairWindow.RepairItem`: the server's result, its buff drawn after
/// it, `repair::buffs`; the sound `RepairComplete` and the notice "Item
/// successfully repaired to" its durability).
#[allow(clippy::too_many_arguments)]
fn clicks(
    buttons: Res<ButtonInput<MouseButton>>,
    spots: Query<(&RepairSpot, &RelativeCursorPosition)>,
    mut carried: ResMut<Carried>,
    game: Res<crate::GameData>,
    said: Res<Words>,
    previews: Res<crate::screens::info_preview::InfoPreviews>,
    mut held: Local<Option<usize>>,
    mut sounds: MessageWriter<crate::audio::ui_sound::Sound>,
    mut feedback: ResMut<crate::weapons::wear::Feedback>,
    mut seed: Local<u32>,
) {
    if !buttons.pressed(MouseButton::Left) {
        *held = None;
    }
    let over = |wanted: fn(&RepairSpot) -> Option<usize>| spots.iter().find(|(spot, at)| wanted(spot).is_some() && at.cursor_over()).map(|(spot, at)| (wanted(spot).unwrap_or(0), at.normalized));
    if buttons.just_pressed(MouseButton::Left) {
        if let Some((index, _)) = over(|spot| if let RepairSpot::Track(i) = spot { Some(*i) } else { None }) {
            *held = Some(index);
        }
    }
    let window_of = |carried: &Carried, index: usize| carried.windows.get(index).filter(|window| window.kind == WinKind::Repair).map(|window| window.item);
    // The handle where the mouse is along the track, while held.
    if let Some(index) = *held {
        let along = spots.iter().find(|(spot, _)| **spot == RepairSpot::Track(index)).and_then(|(_, at)| at.normalized).map(|n| (n.x + 0.5).clamp(0.0, 1.0));
        if let (Some(along), Some(thing)) = (along, window_of(&carried, index)) {
            if let Some(top) = top_of(&carried, thing, &game, &said, &previews) {
                let value = if top >= 0.001 { 0.001 + along * (top - 0.001) } else { 0.0 };
                // (The page drawn afresh only as the value moves.)
                let at = carried.repairs.iter().position(|repair| repair.thing == thing && repair.selected != Some(value));
                if let Some(at) = at {
                    carried.repairs[at].selected = Some(value);
                }
            }
        }
    }
    if !buttons.just_pressed(MouseButton::Left) {
        return;
    }
    if let Some((index, _)) = over(|spot| if let RepairSpot::Top(i) = spot { Some(*i) } else { None }) {
        if let Some(thing) = window_of(&carried, index) {
            let top = top_of(&carried, thing, &game, &said, &previews);
            if let Some(repair) = carried.repairs.iter_mut().find(|repair| repair.thing == thing) {
                repair.selected = top;
            }
        }
        return;
    }
    let Some((index, _)) = over(|spot| if let RepairSpot::Press(i) = spot { Some(*i) } else { None }) else { return };
    let Some(thing) = window_of(&carried, index) else { return };
    let Some(repair) = carried.repairs.iter().find(|repair| repair.thing == thing).cloned() else { return };
    let Some(item) = carried.find(thing).cloned() else { return };
    let Some((words, _)) = repair_words(&carried, &item, &repair, &game.db, &said, false) else { return };
    if !words.interactable {
        return;
    }
    let Some(settings) = carried.repair_settings.clone() else { return };
    let Some(plan) = repair::plan_of(&item, &game.db, &settings) else { return };
    let who = repair.who.unwrap_or_default();
    let amount = words.condition.selected * plan.template_durability();
    let kits = kits_of(&carried, &repair);
    // The server's draws, the repair's and its buff's: the app's own
    // generator (`PROVISIONAL`: SPT's `System.Random` for the buff's chance,
    // its `RandomUtil` for the rest).
    *seed = seed.wrapping_add(1);
    let mut random = trp_sim::base::unity_random::UnityRandom::new(0x5245_5041 ^ *seed as i32);
    let mut draw = || random.value();
    let outcome = repair::repair(&plan, amount, &kits, &settings, &who, &mut draw);
    let won = repair::buffs(&plan, &outcome, &settings, &who, &mut || random.value() as f64);
    apply(&mut carried, &plan, &outcome);
    put_buffs(&mut carried, &won);
    info!("repaired {} by {amount:.1} with {} kit(s): {:?}; buffs drawn {won:?}", item.kind.name, kits.len(), outcome);
    sounds.write(crate::audio::ui_sound::Sound::Ui(crate::audio::ui_sound::Ui::RepairComplete));
    feedback.message(format!("{} {:.1}", said.word("Item successfully repaired to", "Item successfully repaired to"), outcome.repaired_to));
}

/// The slider's top for a window's thing, as it is laid out now.
fn top_of(carried: &Carried, thing: Id, game: &crate::GameData, said: &Words, previews: &crate::screens::info_preview::InfoPreviews) -> Option<f32> {
    let repair = carried.repairs.iter().find(|repair| repair.thing == thing)?;
    let item = carried.find(thing)?;
    repair_words(carried, item, repair, &game.db, said, previews.picture(thing).is_none()).map(|(_, top)| top)
}

/// A repair's result on the things: each part's durability and greatest,
/// each kit's resource, a kit used up gone from where it lay.
fn apply(carried: &mut Carried, plan: &Plan, outcome: &repair::Outcome) {
    let _ = plan;
    for (id, durability, max) in &outcome.parts {
        let root = if carried.equipment.item.find(*id).is_some() { &mut carried.equipment.item } else { &mut carried.loot };
        if let Some(part) = root.find_mut(*id) {
            part.repairable = Some(Repairable { durability: *durability, max_durability: *max });
        }
    }
    for (id, left) in &outcome.kits {
        let root = if carried.equipment.item.find(*id).is_some() { &mut carried.equipment.item } else { &mut carried.loot };
        match left {
            Some(left) => {
                if let Some(kit) = root.find_mut(*id) {
                    kit.resource = Some(*left);
                }
            }
            None => {
                trp_sim::items::inventory::use_up(root, *id);
            }
        }
    }
    carried.tidy();
}

/// The buffs a repair brought, each on its part (in place of any it had).
fn put_buffs(carried: &mut Carried, won: &[(Id, repair::RepairBuff)]) {
    for (id, buff) in won {
        let root = if carried.equipment.item.find(*id).is_some() { &mut carried.equipment.item } else { &mut carried.loot };
        if let Some(part) = root.find_mut(*id) {
            part.buff = Some(*buff);
        }
    }
}

/// The command line's check (`--buffed <kind>[:rare]`): once the screen is
/// open, the first weapon in a slot given that buff as the server would
/// draw it at the middle of its ranges (a weapon's kinds; an armour's
/// `reduction` goes to the armour worn), and its inspect window opened
/// (not with `--repair`, whose window shows the buff alone).
fn check_buffed(options: Res<crate::Options>, controller: Res<crate::player::controller::Controller>, mut carried: ResMut<Carried>, mut done: Local<bool>) {
    let Some((kind, rarity)) = options.buffed.filter(|_| !*done && controller.inventory_open) else { return };
    *done = true;
    let Some(settings) = carried.repair_settings.clone() else { return };
    let armour = kind == repair::BuffKind::DamageReduction;
    let root = &carried.equipment.item;
    let wanted = |item: &&Item| if armour { item.kind.armor_class > 0 && item.repairable.is_some() } else { is(item, "Weapon") };
    let Some(thing) = root.slots.iter().flatten().find(wanted).map(|item| (item.id, item.durability()[0])) else {
        warn!("--buffed: nothing in a slot takes a {kind:?} buff");
        return;
    };
    let mut table = if armour { settings.kit_buffs.armour.clone() } else { settings.kit_buffs.weapon.clone() };
    table.rarities = vec![(rarity, 1.0)];
    table.kinds = vec![(kind, 1.0)];
    let Some(buff) = repair::roll_buff(&table, thing.1, &mut || 0.5) else {
        warn!("--buffed: SPT's repair.json gives no {rarity:?} {kind:?} buff");
        return;
    };
    put_buffs(&mut carried, &[(thing.0, buff)]);
    info!("--buffed: {buff:?}");
    // (With `--repair` its repair window shows it, alone.)
    if options.repair.is_none() {
        carried.open_info(thing.0);
    }
}

/// The command line's check (`--repair <durability>`): once the screen is
/// open, the first weapon in a slot worn to that durability and its repair
/// window opened with the first kit carried.
fn check_open(options: Res<crate::Options>, controller: Res<crate::player::controller::Controller>, mut carried: ResMut<Carried>, game: Res<crate::GameData>, mut done: Local<bool>) {
    let Some(durability) = options.repair.filter(|_| !*done && controller.inventory_open) else { return };
    *done = true;
    fn first(item: &Item, wanted: &dyn Fn(&Item) -> bool) -> Option<Id> {
        if wanted(item) {
            return Some(item.id);
        }
        item.contents().find_map(|inside| first(inside, wanted))
    }
    let root = &carried.equipment.item;
    let weapon = root.slots.iter().flatten().find(|item| is(item, "Weapon")).map(|item| item.id);
    // A kit carried, else one of the bench's boxes' (the kit let go counts
    // from wherever it lies: `RepairerParametersPanel.Show`).
    let wanted = |item: &Item| item.kind.template == "5910968f86f77425cf569c32";
    let mut kit = first(root, &wanted).or_else(|| first(&carried.loot, &wanted));
    if kit.is_none() {
        // (None at hand: one made, lying with the loot.)
        if let Ok(kind) = trp_sim::items::inventory::Kind::of(&game.db, "5910968f86f77425cf569c32") {
            let made = Item::new(std::sync::Arc::new(kind));
            kit = Some(made.id);
            carried.loot.loose.push(made);
        }
    }
    let root = &carried.equipment.item;
    let (Some(weapon), Some(kit)) = (weapon, kit) else {
        let classes: Vec<String> = root.contents().map(|item| format!("{} {:?}", item.kind.name, item.kind.class_names)).collect();
        warn!("--repair: no weapon in a slot ({weapon:?}), or no repair kit carried ({kit:?}; --carry 5910968f86f77425cf569c32): {classes:?}");
        return;
    };
    if let Some(item) = carried.equipment.item.find_mut(weapon) {
        if let Some(wear) = item.repairable.as_mut() {
            wear.durability = durability.min(wear.max_durability);
        }
    }
    carried.repairing = true;
    carried.open_repair(kit, weapon);
}

/// The places a repair window takes the mouse, put over it as drawn.
pub fn spots(inside: &mut ChildSpawnerCommands, built: &crate::screens::health_view::RepairBuilt, index: usize, s: f32) {
    let place = |[x, y, w, h]: [f32; 4]| Node { position_type: PositionType::Absolute, left: Val::Px(x * s), top: Val::Px(y * s), width: Val::Px(w * s), height: Val::Px(h * s), ..default() };
    if let Some(track) = built.slider {
        inside.spawn((RepairSpot::Track(index), RelativeCursorPosition::default(), place(track)));
    }
    if let Some(top) = built.top_button {
        inside.spawn((RepairSpot::Top(index), RelativeCursorPosition::default(), place(top)));
    }
    inside.spawn((RepairSpot::Press(index), RelativeCursorPosition::default(), place(built.button)));
}
