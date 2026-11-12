//! The bundles' index, the movement's settings, the experience, the skills and the switches read from them.

use super::*;

pub(super) struct BundlesAndSkills {
    pub(super) unity: UnityAssets,
    pub(super) hard: HardSettings,
    pub(super) engine: EngineSettings,
    pub(super) movement_settings: trp_sim::MovementSettings,
    pub(super) movement_as_read: trp_sim::MovementSettings,
    pub(super) earning: player::experience::Experience,
    pub(super) skills: player::skills::Skills,
    pub(super) mastery: player::skills::Mastery,
    pub(super) buffs: trp_sim::condition::skills::Buffs,
    pub(super) rounds_drawn: bool,
    pub(super) rules: trp_sim::items::inventory::Rules,
    pub(super) muzzle_drawn: bool,
    pub(super) particles_drawn: bool,
    pub(super) tracers_drawn: bool,
}

/// Opens the bundles, reads the movement's settings, the experience, the skills and the mastering, and the switches that follow from them.
pub(super) fn load(install: Install, db: &std::sync::Arc<Database>, options: &mut Options, options_state: &mut screens::options_view::OptionsState, equipment: &trp_sim::items::inventory::Equipment, families: trp_sim::condition::mastering::Families, rules: trp_sim::items::inventory::Rules) -> anyhow::Result<BundlesAndSkills> {
    platform::run_log::phase("the range's things");
    // The see-through glass (`SEE-THROUGH GLASS`), known as each material
    // is made from here on, its cubes in their bundle (`glass_view`).
    effects::glass_view::set_on(options_state.in_force.see_through_glass);
    // The OVERALL tab's own figure drawn with the game's own surface
    // (`OVERALL FIGURE`, with the character's model: `figure_surface`), its
    // materials' figures and cubes noted as they are made from here on.
    screens::figure_surface::set_on(options_state.in_force.character_model && options_state.in_force.overall_figure);
    let mut unity = UnityAssets::new(install)?;
    effects::glass_view::open_cubes(&mut unity);
    let hard = HardSettings::load(&unity.install)?;
    let engine = EngineSettings::load(&unity.install)?;
    let mut movement_settings = trp_sim::load_movement_settings(&mut unity, &db, &hard, &engine, options.weight)?;
    // As read, before the character's own changes: the dummies' are made
    // from it (below), the install not read again.
    let movement_as_read = movement_settings.clone();
    // The body's animator decides the movement states and supplies the
    // motion. The weapon's width, which it also takes, is set once the
    // weapon is assembled.
    // (The keys and the options were read before the range's things:
    // `options_state`.)
    platform::run_log::phase("the bundles' index, the movement's settings");
    // The character's level, by the install's table, and what the run
    // earns (`experience`; `EXPERIENCE`, `SIMPLE XP GAIN`).
    options_state.level_table = trp_sim::condition::experience::LevelTable::read(&db.config);
    if let Some(experience) = options.experience {
        options_state.experience = experience;
    }
    let earning = player::experience::Experience::new(&db.config, options_state.experience, options_state.in_force.experience, options_state.in_force.simple_xp_gain);
    println!(
        "experience: {} (level {}){}",
        earning.total,
        earning.level(),
        match (earning.on(), earning.simple()) {
            (false, _) => ", nothing earned (EXPERIENCE off)",
            (true, true) => ", earned at its whole worth (SIMPLE XP GAIN)",
            (true, false) => ", earned by Tarkov's rules",
        }
    );
    // The character's skills: the tab's levels (the run's flags over them),
    // in force while `SKILLS` is on (`skills`). With Strength at its elite
    // level the weapons weigh nothing (inventory spec, "Weight").
    let skills = player::skills::Skills::new(options_state.skills.clone(), options_state.in_force.skills, options.aim_speed_bonus);
    // The weapon mastering: the tab's families (the run's flags over
    // them), in force while `WEAPON MASTERING` is on; `--mastering` over
    // every weapon whatever the switch.
    let mastery = player::skills::Mastery::new(families, options_state.mastering.clone(), options_state.in_force.weapon_mastering, options.mastering);
    let buffs = skills.buffs();
    if buffs.strength_elite && !options.weight_given {
        options.weight = equipment.carried_weight(true);
        movement_settings = movement_settings.reweighed(&hard, &db.config, options.weight)?;
        println!("carried at elite Strength: {:.2} kg", options.weight);
    }
    if !skills.levels.untrained() {
        let set: Vec<String> = trp_sim::condition::skills::Skill::ALL.iter().filter(|s| skills.levels.level(**s) > 0).map(|s| format!("{} {}", s.name(), skills.levels.level(*s))).collect();
        println!("skills{}: {}", if skills.on() { "" } else { " (SKILLS off: no buff)" }, set.join(", "));
    }
    // A magazine's rounds in its model and its picture (`MAGAZINE ROUNDS`).
    let rounds_drawn = options_state.in_force.magazine_rounds;
    screens::icons::draw_rounds(rounds_drawn);
    // A weapon made from its own parts (`ADVANCED ATTACHMENT SYSTEM`).
    trp_sim::items::inventory::set_builds(options_state.in_force.attachments);
    // How many of a kind may be carried in a raid (`RAID LIMITS`, off
    // unless chosen): the install's list, made once for the run.
    let mut rules = rules;
    if options_state.in_force.raid_limits {
        rules.limits = Some(Box::leak(Box::new(trp_sim::items::inventory::raid_limits(&db))));
    }
    // And the discard limit a thing's figures list (`CANNOT BE DISCARDED
    // IN RAID`): none with the switch off.
    trp_sim::items::item_attributes::set_discard_limits(options_state.in_force.raid_limits);
    // A shot's flash (`MUZZLE FLASH`), where the hard settings have shot
    // effects at all (`SHOT_EFFECTS_ENABLED`).
    let muzzle_drawn = options_state.in_force.muzzle_flash && hard.float("SHOT_EFFECTS_ENABLED").is_ok_and(|on| on != 0.0);
    // Its fumes (`PARTICLES`), from the same shot.
    let particles_drawn = options_state.in_force.particles && hard.float("SHOT_EFFECTS_ENABLED").is_ok_and(|on| on != 0.0);
    // A tracer's trace (`TRACERS`, with `BALLISTICS`: `tracer_view`).
    let tracers_drawn = options_state.in_force.tracers && options_state.in_force.ballistics;
    Ok(BundlesAndSkills { unity, hard, engine, movement_settings, movement_as_read, earning, skills, mastery, buffs, rounds_drawn, rules, muzzle_drawn, particles_drawn, tracers_drawn })
}
