//! What the character earns by what it does, from every source of experience the game has.
//! After `docs/spec/skills.md`, "Earning", with the game's notices, the skill list following
//! each award and the overall level: every skill's points by the game's actions
//! (`trp_sim::earning`), the weapon's mastering by its hits, and the
//! character's experience by what the game's statistics count in a raid
//! (`BaseStatisticsManager`, `HealthStatisticsManager`), with the game's
//! notices.
//!
//! **The experience counts as it is earned**: the game keeps a raid's in its
//! session's counters and adds them to the character at the raid's end
//! (with the exit's multiplier); the range has no end, so each is added at
//! once, a fraction kept for the next, as the game's own statistics out of
//! a raid do (`OfflineStatisticManager.ExperienceGained`).
//!
//! What raises what: the controller's queue (`Controller::happened`: the
//! weapon's reloads, chamberings, sights and malfunctions put right, a
//! throw, the health's events), the run's statistics' (`Statistics::heard`:
//! the inventory's rounds, checks, finds and searches, examining, the doors;
//! its experience counted), the dummies' (`Dummies::experience`: the
//! player's hits and kills), and the movement read here (the pedometer).
//! With `EXPERIENCE` off nothing is earned.

use std::collections::HashMap;

use bevy::prelude::*;
use trp_sim::condition::earning::{Act, EarnRules, LevelUp, Session};
use trp_sim::condition::experience::{ExpFigures, LevelTable};
use trp_sim::condition::health::{Event, Kind};
use trp_sim::locomotion::movement::State;
use trp_sim::condition::skills::{Skill, ELITE};

use crate::player::controller::Controller;

pub struct ExperiencePlugin;

impl Plugin for ExperiencePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, earn.after(crate::screens::inventory_view::ScreenDrawn).after(crate::player::statistics::Told).before(crate::weapons::wear::NoticesShown).run_if(crate::running));
    }
}

/// Something the character did that earns, as the system that saw it
/// tells it.
#[derive(Debug, Clone, PartialEq)]
pub enum Happened {
    /// A skill's action whole as told.
    Act(Act),
    /// The weapon in hand reloaded, a round chambered by its hands: its
    /// class's action, the class read here.
    Reload,
    Chamber,
    /// The sights up or down (the fast-aim timer's target).
    Aimed(bool),
    /// What the health did.
    Health(Event),
    /// Experience the statistics counted (looting, searching, examining).
    Experience(i32),
    /// A locked door opened with its key, or breached
    /// (`MovementContext.LockedDoorOpened`).
    Door { breached: bool },
}

impl Happened {
    /// A skill's action the hands raised (`FirearmEvent::Earn`): the class's
    /// reload or chamber action, or Mag Drills' round loaded.
    pub fn of_earn(earn: trp_sim::weapons::firearm::Earn) -> Self {
        use trp_sim::weapons::firearm::Earn;
        match earn {
            Earn::Reload => Self::Reload,
            Earn::Chamber => Self::Chamber,
            Earn::RoundLoaded => Self::Act(Act::RoundLoaded),
        }
    }
}

/// The pedometer (`Pedometer`): once a second the character's level
/// distance since the last sample (the first from the world's origin),
/// credited to the state it is in at the sample; a walk's or a sprint's
/// metres are what was credited to it since its mark, made as it began.
#[derive(Debug, Clone, Default)]
struct Pedometer {
    walked: f32,
    sprinted: f32,
    marks: [f32; 2],
    last: [f32; 3],
    next: Option<f64>,
}

impl Pedometer {
    fn sample(&mut self, now: f64, at: [f32; 3], state: State) {
        if self.next.is_some_and(|next| now < next) {
            return;
        }
        self.next = Some(self.next.map_or(now, |next| next) + 1.0);
        if at != self.last {
            let far = ((self.last[0] - at[0]).powi(2) + (self.last[2] - at[2]).powi(2)).sqrt();
            match state {
                State::Move => self.walked += far,
                State::Sprint => self.sprinted += far,
                _ => {}
            }
            self.last = at;
        }
    }

    fn mark(&mut self, state: State) {
        match state {
            State::Move => self.marks[0] = self.walked,
            State::Sprint => self.marks[1] = self.sprinted,
            _ => {}
        }
    }

    fn from_mark(&self, sprint: bool) -> f32 {
        if sprint { self.sprinted - self.marks[1] } else { self.walked - self.marks[0] }
    }
}

/// The fast-aim timer (`SkillManager.FastAimTimer`, a `BetterValueBlender`
/// of speed 1): its value goes toward its target at 1 a second, the target
/// 0 while the sights are up and 2 while they are down; both begin at 0,
/// so the run's first aim is no fast one. A hit gives Aim Drills while the
/// value is over 0 and the target 0.
#[derive(Debug, Clone, Copy, Default)]
struct FastAim {
    start: f32,
    target: f32,
    since: f64,
}

impl FastAim {
    fn value(&self, now: f64) -> f32 {
        let went = (now - self.since).max(0.0) as f32;
        let toward = (self.target - self.start).signum();
        let value = self.start + went * toward;
        if self.start > self.target { value.clamp(self.target, self.start) } else { value.clamp(self.start, self.target) }
    }

    fn set_target(&mut self, target: f32, now: f64) {
        if self.target != target {
            self.start = self.value(now);
            self.since = now;
            self.target = target;
        }
    }

    fn fast(&self, now: f64) -> bool {
        self.value(now) > 0.0 && self.target == 0.0
    }
}

/// The character's earning.
#[derive(Resource)]
pub struct Experience {
    /// `EXPERIENCE` and `SIMPLE XP GAIN` as the run began.
    on: bool,
    simple: bool,
    rules: EarnRules,
    session: Session,
    pub table: LevelTable,
    figures: ExpFigures,
    /// What medicine putting an effect right gives (`HealExperience`), by
    /// the effect's name.
    treatment: HashMap<String, i32>,
    /// The character's experience: the options' file keeps it
    /// (`"experience"`), the options' SKILLS page sets it by its level.
    pub total: i64,
    /// What the run has earned of it.
    pub this_run: i64,
    /// What of a fraction of experience is not yet a whole one.
    fraction: f32,
    /// Healing's experience not yet told (`HealingExp`), and when it is
    /// next told (once a second, from ten: `Co_CalculateExperience`).
    healing: f32,
    next_told: Option<f64>,
    /// How many the run has killed (`Kills`), for the streak's percent.
    kills: u32,
    pedometer: Pedometer,
    state: Option<State>,
    fast_aim: FastAim,
    /// When low health and a stimulant's ill effect began.
    low_health: Option<f64>,
    stimulant_ill: Option<f64>,
}

impl Experience {
    pub fn new(config: &serde_json::Value, total: i64, on: bool, simple: bool) -> Self {
        let rules = EarnRules::read(config);
        let session = Session::new(&rules);
        let treatment = config["Health"]["Effects"].as_object().into_iter().flatten().filter_map(|(name, effect)| Some((name.clone(), effect["HealExperience"].as_i64()? as i32))).collect();
        Self {
            on,
            simple,
            rules,
            session,
            table: LevelTable::read(config),
            figures: ExpFigures::read(config),
            treatment,
            total: total.max(0),
            this_run: 0,
            fraction: 0.0,
            healing: 0.0,
            next_told: None,
            kills: 0,
            pedometer: Pedometer::default(),
            state: None,
            fast_aim: FastAim::default(),
            low_health: None,
            stimulant_ill: None,
        }
    }

    /// Whether anything is earned this run.
    pub fn on(&self) -> bool {
        self.on
    }

    /// The character's level (`ProfileInfo.GetLevel`).
    pub fn level(&self) -> u32 {
        self.table.level(self.total)
    }


    /// A skill's session as the SKILLS tab shows it: its effectiveness
    /// (the arrow and the tooltip's leveling speed), and when it last
    /// earned (the sort "Most recent").
    pub fn session(&self, skill: Skill) -> &trp_sim::condition::earning::SkillSession {
        self.session.of(skill)
    }

    /// Whether the session's fatigue applies (not with `SIMPLE XP GAIN`).
    pub fn simple(&self) -> bool {
        self.simple
    }

    /// Experience added, a fraction kept (`ExperienceGained`).
    fn gain(&mut self, experience: f32) {
        if experience <= 0.0 {
            return;
        }
        let whole = experience + self.fraction;
        let add = whole.floor();
        self.fraction = whole - add;
        self.total += add as i64;
        self.this_run += add as i64;
    }
}

/// What the notices say, in the install's words.
struct Said<'a> {
    texts: &'a trp_data::character::inventory::ItemTexts,
}

impl Said<'_> {
    fn word(&self, key: &str, otherwise: &str) -> String {
        self.texts.say(key).unwrap_or_else(|| otherwise.to_owned())
    }

    /// `StatisticNotification`: "<first> - <second> (<value>)".
    fn statistic(&self, first: (&str, &str), second: (&str, &str), value: i32) -> String {
        format!("{} - {} ({value})", self.word(first.0, first.1), self.word(second.0, second.1))
    }

    /// `SkillLevelUpNotification`: "{0} skill leveled up to {1}".
    fn skill_up(&self, skill: Skill, level: u32) -> String {
        let name = self.word(skill.name(), skill.name());
        self.word("SkillLevelUpMessage", "{0} skill leveled up to {1}").replace("{0}", &name).replace("{1}", &level.to_string())
    }

    /// `MasteringLevelUpMessage`: "{0} mastering leveled up to {1}".
    fn mastering_up(&self, family: &str, level: u32) -> String {
        let name = self.word(family, family);
        self.word("MasteringLevelUpMessage", "{0} mastering leveled up to {1}").replace("{0}", &name).replace("{1}", &level.to_string())
    }
}

/// Everything told since the last frame earned: skills' points (their
/// level-ups told), the weapon's mastering by its hits, the character's
/// experience (healing's told once a second from ten, a treatment's and a
/// door's at once); the movement's walks and sprints by the pedometer, the
/// fast-aim timer, the time at low health and under a stimulant's ill
/// effect.
#[allow(clippy::too_many_arguments)]
fn earn(
    time: Res<Time>,
    mut experience: ResMut<Experience>,
    mut controller: ResMut<Controller>,
    mut statistics: ResMut<crate::player::statistics::Statistics>,
    dummies: Option<ResMut<crate::world::dummy::Dummies>>,
    mut skills: ResMut<crate::player::skills::Skills>,
    mut mastery: ResMut<crate::player::skills::Mastery>,
    (hands, stowed): (Option<Res<crate::weapons::weapon::HandsRig>>, Option<Res<crate::weapons::weapon::Stowed>>),
    carried: Res<crate::screens::inventory_view::Carried>,
    options: Option<Res<crate::screens::options_view::OptionsState>>,
    mut feedback: ResMut<crate::weapons::wear::Feedback>,
) {
    let now = time.elapsed_secs_f64();
    let mut happened = std::mem::take(&mut controller.bypass_change_detection().happened);
    happened.append(&mut statistics.bypass_change_detection().heard);
    let hits = dummies.map(|mut dummies| std::mem::take(&mut dummies.bypass_change_detection().experience)).unwrap_or_default();
    let xp = &mut *experience;
    // The looting's share whole with `SIMPLE XP GAIN` (`statistics`).
    let simple = xp.on && xp.simple;
    if statistics.simple != simple {
        statistics.bypass_change_detection().simple = simple;
    }
    if !xp.on {
        return;
    }
    use crate::screens::options_view::Notices;
    let shown = |which: usize| options.as_ref().is_none_or(|options| options.notices.on(which));
    let said = Said { texts: &carried.texts };
    let mut acts: Vec<Act> = Vec::new();
    let class = hands.as_deref().and_then(|hands| hands.class);
    let health = controller.health.as_ref();
    // The movement: the pedometer's sample, then a walk's or a sprint's end
    // on the ground (`MovementContextOnStateChanged`).
    let m = &controller.movement;
    let state = m.state;
    xp.pedometer.sample(now, m.position, state);
    if xp.state != Some(state) {
        debug!("the movement's state {:?} to {state:?}, grounded {}, walked {:.2}, sprinted {:.2}", xp.state, m.grounded, xp.pedometer.from_mark(false), xp.pedometer.from_mark(true));
        if let Some(was) = xp.state.filter(|was| matches!(was, State::Move | State::Sprint)) {
            let sprint = was == State::Sprint;
            let metres = xp.pedometer.from_mark(sprint);
            if m.grounded && metres > 0.0 {
                let prone = matches!(state, State::ToProne | State::ProneIdle | State::ProneMove);
                let quiet = !sprint && !prone && state != State::Sprint && m.sounds.covert_efficiency() > 0.9;
                let fatigue = health.and_then(|health| health.active(Kind::ChronicFatigue, Some(trp_sim::condition::health::Part::Chest))).map_or(0.0, |effect| effect.strength);
                acts.push(Act::Moved { metres, sprint, overweight: m.stamina.settings.overweight, fatigue, quiet });
            }
        }
        xp.pedometer.mark(state);
        xp.state = Some(state);
    }
    // A stimulant's ill effect: its seconds as the last of them ends.
    let ill = health.is_some_and(|health| health.stimulator_listing(true).iter().any(|list| !list.is_empty()));
    match (ill, xp.stimulant_ill) {
        (true, None) => xp.stimulant_ill = Some(now),
        (false, Some(since)) => {
            xp.stimulant_ill = None;
            acts.push(Act::StimulantIll((now - since) as f32));
        }
        _ => {}
    }
    let on_painkillers = health.is_some_and(|health| health.conditions().on_painkillers);
    let mut gained = 0.0f32;
    for event in happened {
        match event {
            Happened::Act(act) => acts.push(act),
            Happened::Reload => acts.extend(class.map(Act::Reload)),
            Happened::Chamber => acts.extend(class.map(Act::Chamber)),
            Happened::Aimed(up) => xp.fast_aim.set_target(if up { 0.0 } else { 2.0 }, now),
            Happened::Experience(amount) => gained += amount as f32,
            Happened::Door { breached } => {
                let amount = if breached { xp.figures.door_breach } else { xp.figures.door_open };
                gained += amount as f32;
                if amount > 0 && shown(Notices::DOORS) {
                    let second = if breached { ("StatsDoorsBreached", "Doors breached") } else { ("StatsDoorsUnlocked", "Doors unlocked") };
                    feedback.message(said.statistic(("StatsLooting", "Looting experience"), second, amount));
                }
            }
            Happened::Health(event) => {
                // Metabolism's: by the point by the game's rule, or once an
                // item used with simple XP gain (`earning::metabolism`).
                acts.extend(trp_sim::condition::earning::metabolism(&event, xp.simple));
                match event {
                    Event::Damaged(_, amount, kind) if !kind.self_inflicted() => acts.push(Act::DamageTaken(amount)),
                    Event::Started(Kind::LightBleeding | Kind::HeavyBleeding, _) => acts.push(Act::Bleeding),
                    Event::Started(Kind::Intoxication, _) => acts.push(Act::Intoxication),
                    Event::Started(Kind::Pain, _) if !on_painkillers => acts.push(Act::Pain),
                    Event::Started(Kind::LowHealth, _) => xp.low_health = Some(now),
                    Event::Residual(Kind::LowHealth, _) | Event::Removed(Kind::LowHealth, ..) => {
                        if let Some(since) = xp.low_health.take() {
                            acts.push(Act::LowHealth((now - since) as f32));
                        }
                    }
                    // Energy and hydration gained: their experience
                    // (`expForEnergy`, `expForHydration`).
                    Event::EnergyGained(amount) => gained += xp.figures.energy * amount,
                    Event::HydrationGained(amount) => gained += xp.figures.hydration * amount,
                    // Health restored: its experience, told once a second from
                    // ten (`OnHealthChanged`, `ShowHealingNotification`).
                    Event::Healed(_, amount) => {
                        let figure = xp.figures.heal * amount;
                        gained += figure;
                        xp.healing += figure;
                    }
                    // An effect a medicine put right: its treatment's
                    // experience, told at once (`OnHealerDone`).
                    Event::Cured(kind, _) => {
                        let amount = xp.treatment.get(kind.name()).copied().unwrap_or(0);
                        if amount > 0 {
                            gained += amount as f32;
                            if shown(Notices::TREATMENT) {
                                feedback.message(said.statistic(("StatsTreatment", "Treatment experience"), ("StatsHealed", "Healing"), amount));
                            }
                        }
                    }
                    Event::Restored(_) => acts.push(Act::Surgery),
                    _ => {}
                }
            }
        }
    }
    // The player's hits on living dummies (`ManageAggressor`): the weapon
    // in hand's class, Aim Drills within the fast aim, a mastering point;
    // a heavy hit's and a kill's experience (`OnEnemyDamage`,
    // `OnEnemyKill`: the range's dummies are PMCs). A knife's blow is a
    // hit by its item too (`ExecuteShotSkill`): its class Melee, which the
    // knife's rig does not name.
    let held = stowed.as_deref().and_then(|stowed| stowed.held()).map(|key| trp_sim::items::inventory::key_template(key).to_owned());
    let mut mastered: Vec<(String, u32)> = Vec::new();
    for hit in hits {
        if hit.bullet {
            let class = if hit.melee { Some(Skill::Melee) } else { class };
            if let Some(class) = class {
                acts.push(Act::Hit { class, fast_aim: xp.fast_aim.fast(now) });
                // The class elite as the buffs have it, a stimulator's boost
                // with (`WeaponDoubleMastering` by `SummaryLevel`).
                let elite = skills.level(class) >= ELITE;
                if let (true, Some(template)) = (mastery.on(), held.as_deref()) {
                    let families = mastery.families.clone();
                    if let Some(family) = families.of_weapon(template) {
                        let mastery = mastery.bypass_change_detection();
                        let before = mastery.levels.level(family);
                        let points = mastery.levels.points(&family.name).unwrap_or(0.0) + trp_sim::condition::earning::mastering_points(&xp.rules, elite);
                        mastery.levels.set_points(&family.name, points);
                        let after = mastery.levels.level(family);
                        if after != before {
                            mastered.push((family.name.clone(), after));
                        }
                    }
                }
            }
            if hit.heavy {
                gained += xp.figures.pmc_damage(hit.damage, hit.whole) as f32;
            }
        }
        if hit.killed {
            xp.kills += 1;
            let (base, head, streak) = xp.figures.pmc_kill(hit.head, xp.kills);
            gained += (base + head + streak) as f32;
        }
    }
    // The skills' awards.
    let mut ups: Vec<LevelUp> = Vec::new();
    if !acts.is_empty() {
        let (rules, simple) = (&xp.rules, xp.simple);
        // The classes elite as the buffs have them (`SummaryLevel`).
        let elite: Vec<Skill> = Skill::ALL.iter().copied().filter(|skill| skills.level(*skill) >= ELITE).collect();
        let levels = &mut skills.bypass_change_detection().levels;
        for act in acts {
            ups.extend(trp_sim::condition::earning::earn(levels, &mut xp.session, rules, act, now, simple, |class| elite.contains(&class)));
            debug!("earned by {act:?}: {}", trp_sim::condition::earning::awards(act, rules, |_| false).iter().map(|(skill, _)| format!("{} {:.3}", skill.name(), levels.progress(*skill))).collect::<Vec<_>>().join(", "));
        }
    }
    let before = xp.total;
    xp.gain(gained);
    if xp.total != before {
        info!("experience +{}: {} in all, level {} ({} this run)", xp.total - before, xp.total, xp.level(), xp.this_run);
    }
    for up in &ups {
        info!("{} earned to level {}", up.skill.name(), up.level);
        if shown(Notices::SKILL_LEVEL) {
            feedback.message(said.skill_up(up.skill, up.level));
        }
    }
    // A level reached puts the buffs on again (`Skill.LevelChanged`).
    if !ups.is_empty() {
        skills.set_changed();
    }
    for (family, level) in mastered {
        info!("the {family} family's mastering earned to level {level}");
        if shown(Notices::MASTERING_LEVEL) {
            feedback.message(said.mastering_up(&family, level));
        }
        mastery.set_changed();
    }
    // Healing's experience told, once a second from ten.
    if xp.next_told.is_none_or(|next| now >= next) {
        xp.next_told = Some(now + 1.0);
        if xp.healing >= 10.0 {
            if shown(Notices::TREATMENT) {
                feedback.message(said.statistic(("StatsTreatment", "Treatment experience"), ("StatsHealed", "Healing"), xp.healing as i32));
            }
            xp.healing = 0.0;
        }
    }
}

#[cfg(test)]
mod tests;
