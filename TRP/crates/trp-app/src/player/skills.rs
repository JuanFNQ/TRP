//! The character's skills.
//! After `docs/spec/skills.md`: the levels as the options tab sets them, or a host game at run
//! time, and their buffs put wherever they act. What play earns is `experience`'s. The `SKILLS`
//! switch holds for the run (restart required): off, every buff is an untrained character's
//! whatever the levels say.

use bevy::prelude::*;
use trp_sim::condition::health::HealthSkills;
use trp_sim::condition::skills::{Buffs, SkillLevels};

use crate::player::controller::Controller;
use crate::screens::inventory_view::Carried;
use crate::weapons::weapon::{HandsRig, Stowed};
use crate::GameData;

/// The character's skills. A host game changes `levels` (any time: a
/// level up of its own); the buffs follow from the next frame, and the
/// options tab shows the change and keeps it in the app's file.
#[derive(Resource, Debug, Clone)]
pub struct Skills {
    pub levels: SkillLevels,
    /// The `SKILLS` switch as the run began.
    on: bool,
    /// Aim Drills' share of the aiming speed as `--aim-speed-bonus` gave it
    /// for the run, over the level's.
    aim_override: Option<f32>,
    /// What stimulators' buffs at work add to skills (`Skill.Buff`; health
    /// spec, "Buffs"), as the health last told them (`stimulated`).
    boosts: Vec<(trp_sim::condition::skills::Skill, f32)>,
    /// The carrying limits times this from the health's weight buffs
    /// (`HealthController.CarryingWeightRelativeModifier`).
    carry_health: f32,
}

impl Skills {
    pub fn new(levels: SkillLevels, on: bool, aim_override: Option<f32>) -> Self {
        Self { levels, on, aim_override, boosts: Vec::new(), carry_health: 1.0 }
    }

    /// A skill's level in force, a stimulator's buffs on it with
    /// (`SummaryLevel`): nought with `SKILLS` off.
    pub fn level(&self, skill: trp_sim::condition::skills::Skill) -> u32 {
        if self.on {
            self.levels.summary(skill, &self.boosts)
        } else {
            0
        }
    }

    /// Whether the system runs this run.
    pub fn on(&self) -> bool {
        self.on
    }

    /// The buffs in force: the summary levels', or none with the system off
    /// (a stimulator's skill buffs then do nothing either).
    pub fn buffs(&self) -> Buffs {
        if self.on { self.levels.buffs_with(&self.boosts) } else { Buffs::default() }
    }

    /// The health's skills in force.
    pub fn health(&self) -> HealthSkills {
        if self.on {
            HealthSkills::of_levels(|name| trp_sim::condition::skills::Skill::of_name(name).map_or(0, |skill| self.levels.summary(skill, &self.boosts)))
        } else {
            HealthSkills::default()
        }
    }

    /// The carrying limits' factor: Strength's lift times the health's
    /// weight buffs (`Player.CarryingWeightRelativeModifier`).
    pub fn carry(&self, buffs: &Buffs) -> f32 {
        (1.0 + buffs.strength_lift) * self.carry_health
    }

    /// The aiming speed's share from the skills (Aim Drills), or the run's
    /// own.
    pub fn aim_speed(&self, buffs: &Buffs) -> f32 {
        self.aim_override.unwrap_or(buffs.aim_speed)
    }
}

/// Puts the buffs on the movement: Strength's top speeds and jump, the
/// weight's limits (Strength's lift and its inertia step), stamina
/// (Endurance and Strength), and the steps (Covert Movement). The health's
/// are put on it where it is made (`Skills::health`).
pub fn put_on_movement(movement: &mut trp_sim::Movement, buffs: &Buffs, carry: f32, game: (&trp_data::setup::hard_settings::HardSettings, &serde_json::Value)) {
    if let Err(e) = movement.take_skills(buffs, carry, game.0, game.1) {
        warn!("the weight's limits for the skills: {e:#}");
    }
}

impl HandsRig {
    /// The skills' share in the handling of what is in these hands, as
    /// the game takes it when a weapon is drawn (`SyncWithCharacterSkills`,
    /// `UpdateWeaponVariables`): its class's reload and draw speeds,
    /// ergonomics and share of a double action's penalty, Aim Drills'
    /// aiming speed. Nothing for hands that hold no firearm.
    pub fn take_skills(&mut self, buffs: &Buffs, aim_speed: f32) {
        let Some(firearm) = self.firearm.as_mut() else { return };
        let weapon = buffs.weapon(self.class);
        firearm.set_skill_speeds(weapon.reload_speed, weapon.swap_speed);
        self.hands.set_skills(aim_speed, weapon.ergonomics, weapon.recoil);
        self.hands.set_stiff_draw(weapon.stiff_draw);
        self.mount_ergonomics = weapon.mount_ergonomics;
        self.double_action_reduce = weapon.double_action_reduce;
        self.aim_mass = self.hands.settings.ergonomic_weight();
        if weapon != trp_sim::condition::skills::WeaponBuffs::default() || aim_speed != 0.0 {
            info!(
                "the skills on the weapon drawn ({:?}): reload x{:.3}, draw x{:.3}, recoil less {:.3}, ergonomics x{:.3}, time to aim {:.3} s, stiff draw {}, double action's penalty less {:.2}",
                self.class,
                weapon.reload_speed,
                weapon.swap_speed,
                weapon.recoil,
                1.0 + weapon.ergonomics,
                1.0 / self.hands.aiming_speed,
                weapon.stiff_draw,
                weapon.double_action_reduce
            );
        }
    }
}

/// The character's weapon mastering in force (skills spec, "Weapon
/// mastering"): the install's families, each begun family's points as the
/// options tab sets them or a host game in
/// play, the `WEAPON MASTERING` switch and the run's `--mastering`. A
/// resource of its own (a change of it is not a skills' change, which puts
/// the stamina's buffs on again); its hits' points are `experience`'s.
#[derive(Resource, Debug, Clone, Default)]
pub struct Mastery {
    pub families: std::sync::Arc<trp_sim::condition::mastering::Families>,
    /// Each begun family's points, in the order begun: a host game
    /// changes them (`set_points`); the options tab shows the change and
    /// the app's file keeps it.
    pub levels: trp_sim::condition::mastering::MasteringLevels,
    /// The `WEAPON MASTERING` switch as the run began.
    on: bool,
    /// `--mastering`: one level for every weapon this run (0 to 2), over
    /// the families' and the switch.
    run: Option<u32>,
}

impl Mastery {
    pub fn new(families: trp_sim::condition::mastering::Families, levels: trp_sim::condition::mastering::MasteringLevels, on: bool, run: Option<u32>) -> Self {
        Self { families: std::sync::Arc::new(families), levels, on, run: run.map(|level| level.min(trp_sim::condition::mastering::TOP_LEVEL)) }
    }

    /// Whether the system runs this run.
    pub fn on(&self) -> bool {
        self.on
    }

    /// The run's one level for every weapon (`--mastering`), if given.
    pub fn run_level(&self) -> Option<u32> {
        self.run
    }

    /// A weapon's level (`CurrentMasteringLevel`): the run's where
    /// `--mastering` gives one, else its begun family's with the system on,
    /// else 0.
    pub fn level_of(&self, template: &str) -> u32 {
        match self.run {
            Some(level) => level,
            None if self.on => self.levels.level_of(&self.families, template),
            None => 0,
        }
    }

    /// A weapon's mastering as the screens show it; with `--mastering`, its
    /// family at the start of that level.
    pub fn shown(&self, template: &str) -> trp_sim::condition::mastering::Shown {
        use trp_sim::condition::mastering::{Shown, UNBEGUN_TAKES};
        match self.run {
            Some(level) => match self.families.of_weapon(template) {
                Some(family) => {
                    let mut at = trp_sim::condition::mastering::MasteringLevels::default();
                    at.set_level(family, level);
                    at.shown(&self.families, template)
                }
                None => Shown { level, into: 0.0, takes: UNBEGUN_TAKES, share: 0.0, begun: false },
            },
            None if self.on => self.levels.shown(&self.families, template),
            None => Shown { level: 0, into: 0.0, takes: UNBEGUN_TAKES, share: 0.0, begun: false },
        }
    }

    /// A weapon drawn: its family begun at 0 if it is not (`OnWeaponDraw`,
    /// `MasterNewWeapon`), with the system on. Whether it was begun now.
    pub fn drawn(&mut self, template: &str) -> bool {
        if !self.on {
            return false;
        }
        match self.families.of_weapon(template) {
            Some(family) => {
                let name = family.name.clone();
                self.levels.begin(&name)
            }
            None => false,
        }
    }
}

/// The weapon in the hands as it is drawn: its family begun (the game's
/// `OnWeaponDraw`), so that the MASTERING page lists it and the app's file
/// keeps it, as the game's profile does.
pub fn mastery_drawn(stowed: Option<Res<Stowed>>, mut mastery: ResMut<Mastery>, mut was: Local<Option<String>>) {
    let held = stowed.as_deref().and_then(|stowed| stowed.held()).map(str::to_owned);
    if *was == held {
        return;
    }
    if let Some(key) = held.as_deref() {
        let template = trp_sim::items::inventory::key_template(key).to_owned();
        if mastery.bypass_change_detection().drawn(&template) {
            mastery.set_changed();
            info!("the mastering of {template}'s family begun as it is drawn");
        }
    }
    *was = held;
}

/// The mastering as it stands, put where it is read: the weapons drawn
/// from now on, the inventory's views, and the weapon in the hands at once
/// where its level changed (`OnCurrentWeaponBeingMastered`,
/// `SyncWithCharacterSkills`: the animator's `WeaponLevel`).
pub fn mastery_apply(mastery: Res<Mastery>, stowed: Option<ResMut<Stowed>>, hands: Option<ResMut<HandsRig>>, carried: Option<ResMut<Carried>>) {
    let Some(mut stowed) = stowed else { return };
    if mastery.is_changed() {
        stowed.set_mastery(mastery.clone());
        if let Some(mut carried) = carried {
            carried.mastery = mastery.clone();
        }
    }
    let Some(template) = stowed.held().map(|key| trp_sim::items::inventory::key_template(key).to_owned()) else { return };
    let level = mastery.level_of(&template);
    if let Some(mut hands) = hands {
        if hands.firearm.as_ref().is_some_and(|firearm| firearm.mastering() != level) {
            hands.firearm.as_mut().expect("a firearm").set_mastering(level);
            info!("the mastering of {template} in the hands: level {level}");
        }
    }
}

/// Levels set in play on a drawn frame (`--skill-at`): the app standing in
/// for a host game that levels a skill at run time.
pub fn host_levels(options: Res<crate::Options>, mut skills: ResMut<Skills>, mut frame: Local<u32>) {
    *frame += 1;
    for (_, name, level) in options.skill_at.iter().filter(|(at, ..)| *at == *frame) {
        match trp_sim::condition::skills::Skill::of_name(name) {
            Some(skill) => {
                skills.levels.set_level(skill, *level);
                info!("{name} set to level {level} in play");
            }
            None => warn!("no skill is called {name}"),
        }
    }
}

/// The stimulators' buffs at work, as the health has them after its frame:
/// a change in what they add to skills or to carrying is a change of the
/// skills in force, put on as a level's is (`apply`; `Skill.Buff`,
/// `Endurance.Started`'s `UpdateWeightModifier`).
pub fn stimulated(controller: Res<Controller>, mut skills: ResMut<Skills>) {
    let (boosts, carry) = controller.health.as_ref().map_or((Vec::new(), 1.0), |health| (health.skill_boosts(), health.carry_modifier()));
    if skills.boosts != boosts || skills.carry_health != carry {
        (skills.boosts, skills.carry_health) = (boosts, carry);
    }
}

/// A level changed (a host's, or the tab's): the buffs are put on again
/// (`Skill.LevelChanged`, `UpdateRules`), at once on what reads them as it
/// goes, and on a weapon as it is next drawn, as the game's. The run's
/// first are put on as it is made (`main`), and the weapon in the hands
/// then takes them on the first frame.
#[allow(clippy::too_many_arguments)]
pub fn apply(
    skills: Res<Skills>,
    game: Res<GameData>,
    mut controller: ResMut<Controller>,
    stowed: Option<ResMut<Stowed>>,
    hands: Option<ResMut<HandsRig>>,
    carried: Option<ResMut<Carried>>,
    mut again: MessageReader<crate::player::death::BeginAgain>,
    mut started: Local<bool>,
    mut put_on: Local<Option<(Buffs, HealthSkills, f32)>>,
) {
    let first = !*started;
    *started = true;
    // A run begun again after a death has the controller as the run began:
    // the levels as they now stand are put on it again.
    let begun_again = again.read().count() > 0;
    if !skills.is_changed() && !begun_again {
        return;
    }
    let buffs = skills.buffs();
    // A change that leaves every buff as it was (points earned within a
    // level; a level of a skill whose buffs it does not move) puts nothing
    // on again: the stamina's pools are not settled anew for it.
    let now = (buffs, skills.health(), skills.carry(&buffs));
    if !first && !begun_again && put_on.as_ref() == Some(&now) {
        return;
    }
    *put_on = Some(now);
    let controller = &mut *controller;
    if !first {
        put_on_movement(&mut controller.movement, &buffs, skills.carry(&buffs), (&game.hard, &game.db.config));
        controller.movement.stamina.resettle();
        if let Some(health) = controller.health.as_mut() {
            health.skills = skills.health();
        }
    }
    // Mag Drills' times and elites, and what Strength makes of carrying
    // (the weight worked out again where its elite changes).
    if let Some(mut carried) = carried {
        let carry = skills.carry(&buffs);
        if (carried.round_times.drills, carried.strength_elite, carried.carry, carried.search_double) != (buffs.mag_drills, buffs.strength_elite, carry, buffs.search_double) {
            carried.round_times.drills = buffs.mag_drills;
            (carried.strength_elite, carried.carry, carried.search_double) = (buffs.strength_elite, carry, buffs.search_double);
        }
    }
    let aim_speed = skills.aim_speed(&buffs);
    if let Some(mut stowed) = stowed {
        stowed.set_skills(buffs, aim_speed);
    }
    if first {
        if let Some(mut hands) = hands {
            hands.take_skills(&buffs, aim_speed);
            if hands.firearm.is_some() {
                controller.movement.stamina.aim_mass = hands.aim_mass;
            }
        }
    }
}
