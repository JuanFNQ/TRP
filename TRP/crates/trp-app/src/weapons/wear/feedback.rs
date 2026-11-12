//! Notices and malfunctions shown, the feedback given, and the wear kept.

use super::*;

/// Where the notices asked are shown: a notice asked before it is shown on
/// the frame.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct NoticesShown;

/// A malfunction made known at the shot, 150 ms on (`BattleUIScreen.
/// MalfunctionExamined`, on `ExamineMalfunctionEvent`, which only a
/// malfunction not known before raises: `ProcessMalfunctionExamined(150)`):
/// the battle screen's quick bar raised and its pistols' slots glowing for
/// `glow` seconds (`TimeToQuickdrawPistol` less the 150 ms; `battle_view`).
#[derive(Message, Clone, Copy, Debug)]
pub struct MalfunctionShown {
    pub glow: f32,
}

/// What the battle screen waits after the shot (`ProcessMalfunctionExamined(150)`),
/// as the glow and its sound wait.
const EXAMINED_DELAY: f64 = 0.15;

/// How long after a malfunction a pistol's slot glows, the wait included
/// (`Malfunction.TimeToQuickdrawPistol` of the globals), else the install's
/// 5 as read.
fn quickdraw_time(config: &serde_json::Value) -> f32 {
    config.get("Malfunction").and_then(|malf| malf.get("TimeToQuickdrawPistol")).and_then(serde_json::Value::as_f64).map_or(5.0, |time| time as f32)
}

/// The install's words the notices are made of (`Notification/...`, the
/// malfunctions' names, `Settings/NotSet` for a key not bound).
pub fn word_keys() -> Vec<String> {
    let mut keys: Vec<String> = ["Notification/MalfunctionOccurred", "Notification/MalfunctionRepairRequired", "Notification/MalfunctionExaminationRequired", "Settings/NotSet", INCOMPATIBLE_AMMO].map(str::to_owned).into();
    keys.extend([Malf::Misfire, Malf::Jam, Malf::Feed, Malf::SoftSlide, Malf::HardSlide].map(|malf| malf.text_key().to_owned()));
    keys
}

/// The install's line for a magazine whose top round the chamber refuses
/// (`Idling.ShowIncompatibleNotification`; the key is the English words).
pub(crate) const INCOMPATIBLE_AMMO: &str = "Ammo ({0}) is not compatible. Need: {1}";

/// The glow's three kinds (`BattleUIMalfunctionGlow.EGlowType`): a
/// malfunction made known or reminded of, its kind looked at, put right.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Glow {
    Examined,
    TypeExamined,
    Repaired,
}

/// What the screen shows of the weapon's malfunctions and the game's other
/// notices: those asked for since the last frame, those on the screen,
/// and the glow last shown, its kind, when, and its alpha's factor
/// (`GetGlowAlphaMultiplier`).
///
/// Every notice asked is shown at once, the newest at the foot of the
/// column, those before it pushed up (`NotifierView.NotificationReceivedHandler`
/// with `ShowImmediately`; its queue, one at a time, is for the notices
/// that are not: `courses10`'s `examine-multiple`, three at once by frame
/// 230, asked on 178, 202 and 215. The decompiled `Notification` leaves
/// `ShowImmediately` false for the custom notices; the take is the game's).
#[derive(Resource, Default)]
pub struct Feedback {
    /// Each its words and whether it is an alert (`ENotificationIconType.
    /// Alert`, the malfunctions') or plain (`Default`, a repair's message).
    pub waiting: VecDeque<(String, bool)>,
    pub shown: Vec<Notice>,
    pub(super) shown_count: u64,
    pub glow: Option<(Glow, f64, f32)>,
    /// The glow's asks in their window (`ShowGlow`): its start and the
    /// count, which a glow shown puts far below.
    pub(super) window: (f64, i32),
    /// A glow and its sound waiting their 150 ms (`ProcessMalfunctionExamined`),
    /// and the weapon's item.
    examined_at: Option<(f64, Id)>,
}

impl Feedback {
    /// A glow asked for (`BattleUIMalfunctionGlow.ShowGlow`): a forced one
    /// shows at once; any other only at the `attempts`-th ask within 2 s
    /// of the first, a window opened again at an ask more than 2 s after
    /// its start. Whether it shows.
    pub(super) fn ask_glow(&mut self, glow: Glow, forced: bool, now: f64, attempts: u32, alpha: f32) -> bool {
        if now - self.window.0 > 2.0 {
            self.window = (now, 0);
        }
        self.window.1 += 1;
        if self.window.1 < attempts as i32 && !forced {
            return false;
        }
        self.window.1 = -1000;
        self.glow = Some((glow, now, alpha));
        true
    }

    pub(super) fn notice(&mut self, words: String) {
        info!("notice: {words}");
        self.waiting.push_back((words, true));
    }

    /// A warning (`NotificationManager.DisplayWarningNotification`: the
    /// alert's icon).
    pub fn warning(&mut self, words: String) {
        self.notice(words);
    }

    /// The same, but not while an alert of those words is shown and not yet
    /// folding, or waiting (`DisplaySingletonWarningNotification`: a line's
    /// reason; `NotifierView.ShowNotification`, `SingletonNotification.
    /// Equals`: the same icon and words, whatever their case; a notice
    /// folding `IsHiding`).
    pub fn single_warning(&mut self, words: String) {
        let same = |said: &str| said.to_lowercase() == words.to_lowercase();
        let there = self.shown.iter().any(|notice| notice.alert && notice.fold.is_none() && same(&notice.words)) || self.waiting.iter().any(|(said, alert)| *alert && same(said));
        if !there {
            self.notice(words);
        }
    }

    /// A plain notice (`NotificationManager.DisplayMessageNotification`).
    pub fn message(&mut self, words: String) {
        info!("notice: {words}");
        self.waiting.push_back((words, false));
    }
}

/// Which weapon's wear the logic in hand holds, by its item and its build,
/// and the seed the next is given.
#[derive(Default)]
pub(super) struct Given {
    item: Option<(Id, String)>,
    seeds: i32,
    settings: Option<Settings>,
    /// The command line's malfunction has come.
    forced: bool,
    /// The durability and greatest last kept back on the item: one that
    /// differs from them was changed elsewhere (a repair), and goes to the
    /// logic.
    written: Option<[f32; 2]>,
    /// The repair buff last kept back on the item, the same way (a repair
    /// drew one).
    buff_written: Option<trp_sim::items::repair::RepairBuff>,
}

/// Every part in a weapon's slots, depth first, but the rounds in its
/// chambers (`Weapon.Mods`: every mod in its slots, the magazine with them).
fn parts(item: &Item, out: &mut Vec<String>) {
    for (slot, kind) in item.slots.iter().zip(&item.kind.slots) {
        let Some(part) = slot else { continue };
        if kind.name.starts_with("patron_in_weapon") {
            continue;
        }
        out.push(part.kind.template.clone());
        parts(part, out);
    }
}

/// A weapon item's figures as built: its template's and every part's in
/// its slots (`WeaponFigures::of`), the logic in hand's or a model's not
/// in hand (`heat_view`).
pub(crate) fn figures_of(db: &trp_data::Database, item: &Item) -> WeaponFigures {
    let mut list = Vec::new();
    parts(item, &mut list);
    let refs: Vec<&str> = list.iter().map(String::as_str).collect();
    WeaponFigures::of(db, &item.kind.template, &refs)
}

/// The globals' malfunction and overheat settings, else the game's own as
/// built in.
pub(crate) fn settings_of(config: &serde_json::Value) -> Settings {
    Settings::from_config(config).unwrap_or_else(|e| {
        warn!("the globals' malfunction settings: {e:#}; the game's own as built in");
        Settings::default()
    })
}

/// The weapon in hand given its wear, told what it needs, and its wear
/// kept back on its item; what it said of its malfunctions shown.
#[allow(clippy::too_many_arguments)]
pub(super) fn keep_wear(
    mut carried: ResMut<Carried>,
    held: Res<HandsHeld>,
    game: Res<crate::GameData>,
    mut weapon: Option<ResMut<HandsRig>>,
    mut controller: ResMut<Controller>,
    features: Res<crate::screens::options_view::Features>,
    skills: Option<Res<crate::player::skills::Skills>>,
    (words, bindings): (Res<crate::screens::health_view::Words>, Res<crate::platform::keys::Bindings>),
    time: Res<Time<Real>>,
    (mut sounds, mut malfunction_shown): (MessageWriter<crate::audio::ui_sound::Sound>, MessageWriter<MalfunctionShown>),
    mut feedback: ResMut<Feedback>,
    mut given: Local<Given>,
    (options, shown): (Res<crate::Options>, Option<Res<crate::screens::options_view::OptionsState>>),
    mut again: MessageReader<crate::player::death::BeginAgain>,
) {
    // The run begun again after a death: the weapon in hand's logic is
    // given its item's wear afresh, the item being as the run began (it
    // kept the old run's malfunction, durability and heat: a jam was still
    // there after the respawn).
    if again.read().count() > 0 {
        (given.item, given.written) = (None, None);
    }
    let now = time.elapsed_secs_f64();
    let events = std::mem::take(&mut controller.wear_events);
    let settings = given.settings.get_or_insert_with(|| settings_of(&game.db.config));
    let attempts = settings.glow_attempts;
    // The glow's sound and its forced glow, 150 ms after a malfunction
    // was made known (`ProcessMalfunctionExamined`).
    let alpha = glow_alpha(&controller);
    if let Some((_, gun)) = feedback.examined_at.filter(|(at, _)| now >= *at) {
        feedback.examined_at = None;
        feedback.ask_glow(Glow::Examined, true, now, attempts, alpha);
        sounds.write(crate::audio::ui_sound::Sound::Ui(crate::audio::ui_sound::Ui::MalfunctionExamined));
        // The quick prompt's situation raised again by the weapon's state
        // now (so it holds 5.15 s from the shot), and the battle screen's
        // quick bar told.
        if let Some(state) = carried.find(gun).map(|item| item.malfunction) {
            controller.phrase_situations.push(crate::player::gestures::phrase::of_malfunction(state.malf, state.type_known));
        }
        malfunction_shown.write(MalfunctionShown { glow: quickdraw_time(&game.db.config) - EXAMINED_DELAY as f32 });
    }
    let Some(firearm) = weapon.as_deref_mut().and_then(|rig| rig.firearm.as_mut()) else {
        given.item = None;
        return;
    };
    let Some(item) = held.item.and_then(|id| carried.equipment.item.find(id)).filter(|item| trp_sim::items::inventory::is(item, "Weapon")) else {
        given.item = None;
        return;
    };
    let id = item.id;
    // Taken in hand (or its build changed): the logic has its item's wear.
    let build = trp_sim::items::inventory::rig_key(item);
    if given.item.as_ref() != Some(&(id, build.clone())) || firearm.wear().is_none() {
        let figures = figures_of(&game.db, item);
        let [durability, most] = item.durability();
        given.seeds = given.seeds.wrapping_add(1);
        let seed = given.seeds;
        let settings = given.settings.clone().unwrap_or_default();
        // The command line's check: the first weapon taken in hand that
        // hot, as if just fired, and malfunctioning at its next shot.
        // (The heat once, kept on the item from then; the malfunction
        // asked again of a logic made afresh until it has come.)
        let mut state = item.malfunction;
        if let Some(heat) = options.heat.filter(|_| seed == 1) {
            (state.last_shot_heat, state.last_shot_time) = (heat, controller.raid_clock());
        }
        let mut wear = Wear::new(figures, settings, durability, most, state, seed);
        wear.buff = item.buff;
        firearm.set_wear(wear);
        (given.written, given.buff_written) = (Some([durability, most]), item.buff);
        if let Some(malf) = options.malfunction.filter(|_| !given.forced) {
            if !firearm.force_malfunction(malf) {
                warn!("--malfunction: {} cannot have {malf:?} in the game (its template does not allow it)", item.kind.name);
            }
        }
        info!("{}: its wear {durability:.2} of {most:.2}, its malfunction {:?}", item.kind.name, item.malfunction.malf);
        given.item = Some((id, build));
    }
    // What it would fire next: the chambered round, else the magazine's
    // top; and the magazine's own chance (none with none fitted).
    let magazine = item.in_slot(trp_sim::items::inventory::MAGAZINE_SLOT);
    let round = trp_sim::items::inventory::chambered(item).map(|round| round.kind.template.clone()).or_else(|| magazine.and_then(|magazine| trp_sim::items::inventory::magazine_round_at(magazine, 0)).map(|kind| kind.template.clone()));
    let round = round.map_or_else(RoundFigures::default, |template| RoundFigures::of(&game.db, &template));
    let chance = magazine.map(|magazine| malfunction::magazine_chance(&game.db, &magazine.kind.template));
    firearm.set_wear_inputs(controller.raid_clock(), round, chance);
    let level = |skill: Skill| skills.as_deref().map_or(0, |skills| skills.level(skill));
    let troubleshooting = level(Skill::TroubleShooting);
    let skills = Skills { maintenance: level(Skill::WeaponTreatment) as f32, troubleshooting: troubleshooting as f32, troubleshooting_elite: troubleshooting >= ELITE };
    let conditions = controller.movement.conditions;
    let arms = u32::from(conditions.left_arm) + u32::from(conditions.right_arm);
    firearm.set_wear_skills(skills, arms, features.wear());
    // Its durability changed on the item elsewhere (a repair) goes to the
    // logic first.
    let on_item = item.repairable.map(|kept| [kept.durability, kept.max_durability]);
    if let (Some(now), Some(was)) = (on_item, given.written) {
        if now != was {
            firearm.set_durability(now[0], now[1]);
            given.written = Some(now);
        }
    }
    if item.buff != given.buff_written {
        firearm.set_buff(item.buff);
        given.buff_written = item.buff;
    }
    // Its wear, state and buff back on the item, where they changed.
    if let Some(wear) = firearm.wear() {
        let (durability, most, state, buff) = (wear.durability, wear.max_durability, wear.state, wear.buff);
        let kept = item.repairable.is_none_or(|kept| kept.durability == durability && kept.max_durability == most) && item.malfunction == state && item.buff == buff;
        if !kept {
            if let Some(item) = carried.equipment.item.find_mut(id) {
                if item.repairable.is_some() {
                    item.repairable = Some(Repairable { durability, max_durability: most });
                    given.written = Some([durability, most]);
                }
                item.malfunction = state;
                (item.buff, given.buff_written) = (buff, buff);
            }
        }
    }
    // What it said, as the shooter is told.
    let name = |malf: Malf| words.word(malf.text_key(), "");
    let occurred = |malf: Malf| words.word("Notification/MalfunctionOccurred", "Malfunction: {0}").replace("{0}", &name(malf));
    let key = |binding: &str| bindings.notice_name(binding).unwrap_or_else(|| words.word("Settings/NotSet", "Not set"));
    // The notices shown or not (the HUD page's NOTIFICATIONS, the app's
    // own); the glow and its sound either way.
    let told = shown.as_ref().is_none_or(|shown| shown.notices.on(crate::screens::options_view::Notices::MALFUNCTIONS));
    let notice = |feedback: &mut Feedback, said: String| {
        if told {
            feedback.notice(said);
        }
    };
    for event in events {
        match event {
            // Made known as it happens (`ExamineMalfunction`, not known
            // before): the glow and its sound 150 ms on.
            FirearmEvent::Malfunctioned(malf) => {
                info!("the weapon malfunctioned: {malf:?}");
                given.forced = true;
                feedback.examined_at = Some((now + EXAMINED_DELAY, id));
            }
            // Known: the glow asked for, and only where it shows, the
            // notice: fix it, or look at it first.
            FirearmEvent::MalfunctionReminder { malf, type_known } => {
                if feedback.ask_glow(Glow::Examined, false, now, attempts, alpha) {
                    let said = if type_known {
                        format!("{} - {}", words.word("Notification/MalfunctionRepairRequired", "Fix malfunction: {0}").replace("{0}", &name(malf)), key("CheckChamber"))
                    } else {
                        format!("{} - {}", words.word("Notification/MalfunctionExaminationRequired", "Malfunction! Examine your weapon"), key("ExamineWeapon"))
                    };
                    notice(&mut feedback, said);
                }
            }
            // Its kind looked at (`ExamineMalfunctionType`): the first time
            // its glow too.
            FirearmEvent::MalfunctionExamined { malf, first } => {
                if first {
                    feedback.ask_glow(Glow::TypeExamined, true, now, attempts, alpha);
                }
                notice(&mut feedback, occurred(malf));
            }
            // A misfire put right by a reload, a draw or the chamber
            // unloaded (`CallUnknownMalfunctionStartRepair`).
            FirearmEvent::MalfunctionCleared(malf) => {
                feedback.ask_glow(Glow::Repaired, true, now, attempts, alpha);
                notice(&mut feedback, occurred(malf));
            }
            // The repair (`CallMalfunctionRepaired`).
            FirearmEvent::MalfunctionRepaired(_) => {
                feedback.ask_glow(Glow::Repaired, true, now, attempts, alpha);
            }
            // A failure to feed's round thrown out by its repair: on the
            // ground before the character, as the chamber's thrown round.
            FirearmEvent::FedRoundThrown => {
                let round = carried.equipment.item.find_mut(id).and_then(|item| item.fed_round.take());
                if let Some(kind) = round {
                    let m = &controller.movement;
                    let at = crate::screens::inventory_view::beside(m.position, m.yaw, [crate::screens::inventory_view::DROP_AHEAD, 0.2]);
                    carried.ground.push((Item::new(kind), at));
                    carried.note = "the round that failed to feed was thrown out: it lies on the ground".to_owned();
                }
            }
            FirearmEvent::Autoshot => info!("the hot weapon fired a round by itself"),
            // A press with a magazine whose top round the chamber refuses
            // (firing spec, "Rounds the chamber does not take"): a single
            // warning, the round by its whole name, the weapon's caliber as
            // the database writes it (`Weapon.AmmoCaliber`). Always shown,
            // as the game shows it: its switch among the HUD page's
            // notices is gone.
            FirearmEvent::IncompatibleAmmo => {
                let gun = carried.equipment.item.find(id);
                // The logic's top round: the item keeps those it has fed
                // until it rests.
                let fed = |magazine: &Item| trp_sim::items::inventory::magazine_rounds(magazine).saturating_sub(firearm.magazine.unwrap_or(0));
                let top = gun.and_then(|gun| gun.in_slot(trp_sim::items::inventory::MAGAZINE_SLOT)).and_then(|magazine| trp_sim::items::inventory::magazine_round_at(magazine, fed(magazine)));
                let round = top.map(|kind| carried.full_name(&Item::new(kind))).unwrap_or_default();
                let caliber = gun.and_then(|gun| game.db.items.get(&gun.kind.template)).and_then(|template| template.props.get("ammoCaliber")).and_then(|caliber| caliber.as_str()).unwrap_or_default();
                let said = words.word(INCOMPATIBLE_AMMO, INCOMPATIBLE_AMMO).replace("{0}", &round).replace("{1}", caliber);
                feedback.single_warning(said);
            }
            _ => {}
        }
    }
}

/// The glow's alpha (`GetGlowAlphaMultiplier`): half, or whole with the
/// low health's effect on.
fn glow_alpha(controller: &Controller) -> f32 {
    let low = controller.health.as_ref().is_some_and(|health| health.active(trp_sim::condition::health::Kind::LowHealth, None).is_some());
    if low { 1.0 } else { 0.5 }
}
