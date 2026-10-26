//! The character's skills (`docs/spec/skills.md`): the levels as the
//! options tab sets them (the user, 2026-10-22: "Set in the options tab"),
//! or a host game at run time (2026-10-23: "leave it for host games to
//! alter levels on runtime"), and their buffs put wherever they act. Nothing
//! here earns points ("never earning"). The `SKILLS` switch holds for the
//! run (restart required): off, every buff is an untrained character's
//! whatever the levels say.

use bevy::prelude::*;
use trp_sim::health::HealthSkills;
use trp_sim::settings::WeightLimits;
use trp_sim::skills::{Buffs, SkillLevels};

use crate::controller::Controller;
use crate::inventory_view::Carried;
use crate::weapon::{HandsRig, Stowed};
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
    boosts: Vec<(trp_sim::skills::Skill, f32)>,
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
    pub fn level(&self, skill: trp_sim::skills::Skill) -> u32 {
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
            HealthSkills::of_levels(|name| trp_sim::skills::Skill::of_name(name).map_or(0, |skill| self.levels.summary(skill, &self.boosts)))
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
pub fn put_on_movement(movement: &mut trp_sim::Movement, buffs: &Buffs, carry: f32, game: (&trp_data::hard_settings::HardSettings, &serde_json::Value)) {
    movement.settings.set_strength(buffs.strength_level as f32);
    if let Err(e) = movement.set_weight_limits(game.0, game.1, WeightLimits { carry, ..WeightLimits::of(buffs) }) {
        warn!("the weight's limits for the skills: {e:#}");
    }
    movement.stamina.settings.set_skill_buffs(buffs);
    movement.settings.stamina = movement.stamina.settings.clone();
    movement.sounds.set_covert(buffs.covert);
}

impl HandsRig {
    /// The skills' share in the handling of what is in these hands, as
    /// the game takes it when a weapon is drawn (`SyncWithCharacterSkills`,
    /// `UpdateWeaponVariables`): its class's reload and draw speeds and
    /// ergonomics, Aim Drills' aiming speed. Nothing for hands that hold no
    /// firearm.
    pub fn take_skills(&mut self, buffs: &Buffs, aim_speed: f32) {
        let Some(firearm) = self.firearm.as_mut() else { return };
        let weapon = buffs.weapon(self.class);
        firearm.set_skill_speeds(weapon.reload_speed, weapon.swap_speed);
        self.hands.set_skills(aim_speed, weapon.ergonomics, weapon.recoil);
        self.mount_ergonomics = weapon.mount_ergonomics;
        self.aim_mass = self.hands.settings.ergonomic_weight();
        if weapon != trp_sim::skills::WeaponBuffs::default() || aim_speed != 0.0 {
            info!(
                "the skills on the weapon drawn ({:?}): reload x{:.3}, draw x{:.3}, recoil less {:.3}, ergonomics x{:.3}, time to aim {:.3} s",
                self.class,
                weapon.reload_speed,
                weapon.swap_speed,
                weapon.recoil,
                1.0 + weapon.ergonomics,
                1.0 / self.hands.aiming_speed
            );
        }
    }
}

/// Levels set in play on a drawn frame (`--skill-at`): the app standing in
/// for a host game that levels a skill at run time.
pub fn host_levels(options: Res<crate::Options>, mut skills: ResMut<Skills>, mut frame: Local<u32>) {
    *frame += 1;
    for (_, name, level) in options.skill_at.iter().filter(|(at, ..)| *at == *frame) {
        match trp_sim::skills::Skill::of_name(name) {
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
    mut again: MessageReader<crate::death::BeginAgain>,
    mut started: Local<bool>,
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
        if (carried.round_times.drills, carried.strength_elite, carried.carry) != (buffs.mag_drills, buffs.strength_elite, carry) {
            carried.round_times.drills = buffs.mag_drills;
            (carried.strength_elite, carried.carry) = (buffs.strength_elite, carry);
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
