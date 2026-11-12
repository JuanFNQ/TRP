//! The dummies' colliders, armour, health and characters, and the character's own armour against a blast.

use super::*;

pub(super) struct DummySourceInputs<'a> {
    pub(super) options_state: &'a screens::options_view::OptionsState,
    pub(super) bullets: &'a Option<weapons::bullets::Bullets>,
    pub(super) unity: &'a mut UnityAssets,
    pub(super) db: &'a std::sync::Arc<Database>,
    pub(super) hard: &'a HardSettings,
    pub(super) engine: &'a EngineSettings,
    pub(super) movement_as_read: &'a trp_sim::MovementSettings,
    pub(super) body_data: &'a trp_sim::locomotion::body::BodyData,
    pub(super) options: &'a Options,
    pub(super) dummy_kits: Vec<trp_sim::items::inventory::Item>,
    pub(super) grenades: bool,
}

pub(super) struct DummiesMade {
    pub(super) dummy_source: Option<world::dummy::DummySource>,
    pub(super) player_armour: Option<effects::blast_view::PlayerArmour>,
}

/// Makes the dummies' source with their characters' maker, and what the character's own armour takes of a blast.
pub(super) fn make(inputs: DummySourceInputs) -> DummiesMade {
    let DummySourceInputs { options_state, bullets, mut unity, db, hard, engine, movement_as_read, body_data, options, dummy_kits, grenades } = inputs;
    // The dummy on the range (`DUMMY`, with the bullets): the player prefab's
    // hit colliders, the armour's and health's figures, the killing hit's
    // force.
    let dummy_source = if options_state.in_force.dummy && bullets.is_some() {
        // In the order the game gathers them (`PlayerBones`), which a blast
        // walks and gives its parts' damage in.
        let colliders = trp_data::character::ragdoll::RagdollData::hit_colliders(&mut unity).map(|mut colliders| {
            match trp_data::character::ragdoll::RagdollData::hit_collider_order(&mut unity) {
                Ok(order) => colliders.sort_by_key(|c| order.iter().position(|name| *name == c.object).unwrap_or(usize::MAX)),
                Err(e) => warn!("the hit colliders' order was not read ({e:#}): a blast's parts go by name"),
            }
            colliders
        });
        let rules = trp_sim::condition::armor::ArmorRules::new(&db.config);
        let health = trp_sim::condition::health::HealthSettings::new(&db.config);
        // Each dummy's character (`trp_sim::puppet`): the AKM in its hands,
        // assembled as the run's own, its animator and logic's settings and
        // its class, the weapon's motion's data, the movement's settings as
        // read (each dummy's made from them for its load and skills as it is
        // made), the player's body animator, the arms' solver, the player's
        // frame rate.
        let maker = (|| -> anyhow::Result<world::dummy::PuppetMaker> {
            // With its light (`dummy::build`).
            let weapon = trp_data::weapons::weapon::assemble_preset(&mut unity, &db, &world::dummy::build(&db)?)?;
            let animator = trp_sim::weapons::weapon_animator::WeaponAnimatorData::load(&unity, &weapon)?;
            let firearm = trp_sim::firearm_settings(&unity, &db, &hard, &weapon)?;
            let hands_data = HandsData::load(&mut unity)?;
            let movement = trp_sim::movement_settings_as_read(&movement_as_read, &db, &hard, &engine, 0.0)?;
            let limbs = trp_data::character::player::LimbData::load(&mut unity)?;
            // With the posing economy, each animator split by the bones a
            // puppet reads of its rig (`puppet::followed_rig_bones`, by the
            // sight its motion picks), so that a character posed when asked
            // works out only those at each update; posed in full, the same.
            let economy = options_state.in_force.posing_economy;
            let split = |animator: trp_sim::weapons::weapon_animator::WeaponAnimatorData, rig: &trp_data::weapons::weapon::Weapon, hands: &trp_sim::Hands| -> anyhow::Result<trp_sim::weapons::weapon_animator::WeaponAnimatorData> {
                if !economy {
                    return Ok(animator);
                }
                animator.following(&trp_sim::characters::puppet::followed_rig_bones(&rig.hierarchy, trp_sim::characters::puppet::sights_for(hands, &rig.hierarchy).as_ref()))
            };
            let animator = split(animator, &weapon, &trp_sim::Hands::new(trp_sim::hands_settings(hands_data.clone(), &db, &hard, &engine, &weapon, trp_sim::ViewSettings::default())?))?;
            // The empty hands, which a dummy stood up with its AKM gone holds
            // (`Puppet::empty_handed`).
            let empty = (|| -> anyhow::Result<world::dummy::EmptyMaker> {
                let rig = trp_data::weapons::weapon::assemble_prefab(&mut unity, trp_data::weapons::weapon::EMPTY_HANDS)?;
                let animator = trp_sim::weapons::weapon_animator::WeaponAnimatorData::load(&unity, &rig)?;
                let animator = split(animator, &rig, &trp_sim::Hands::new(trp_sim::empty_hands_settings(hands_data.clone(), &db, &hard, &engine, &rig, trp_sim::ViewSettings::default())?))?;
                Ok(world::dummy::EmptyMaker { rig: std::sync::Arc::new(rig), animator })
            })()
            .map_err(|e| warn!("the dummies' empty hands could not be read; one stood up without its AKM stands still: {e:#}"))
            .ok();
            Ok(world::dummy::PuppetMaker {
                class: trp_sim::condition::skills::Skill::of_weapon(&db.class_names(&weapon.template)),
                weight_on: options_state.in_force.weight,
                weapon: std::sync::Arc::new(weapon),
                animator,
                firearm,
                hands_data,
                view: trp_sim::ViewSettings::default(),
                movement,
                body: body_data.clone(),
                limbs,
                frame_rate: options.frame_rate,
                physics_step: engine.fixed_timestep,
                longest_frame: engine.maximum_timestep,
                smooth: options_state.in_force.smooth_animations,
                empty,
            })
        })();
        let maker = match maker {
            Ok(maker) => Some(maker),
            Err(e) => {
                warn!("the dummies' characters could not be made: {e:#}");
                None
            }
        };
        match (colliders, rules, health) {
            (Ok(colliders), Ok(rules), Ok(health)) => Some(world::dummy::DummySource {
                colliders,
                rules: std::sync::Arc::new(rules),
                health: std::sync::Arc::new(health),
                hit_force: hard.float("HIT_FORCE").unwrap_or(150.0),
                maker,
                kits: dummy_kits,
                // Their skills at level 1 while `SKILLS` is on (`dummy::
                // SKILL_LEVEL`).
                skills: if options_state.in_force.skills { trp_sim::condition::skills::SkillLevels::all_at(world::dummy::SKILL_LEVEL) } else { trp_sim::condition::skills::SkillLevels::default() },
            }),
            (colliders, rules, health) => {
                warn!(
                    "the dummy could not be set up: {}",
                    [colliders.err(), rules.err(), health.err()].into_iter().flatten().map(|e| format!("{e:#}")).collect::<Vec<_>>().join("; ")
                );
                None
            }
        }
    } else {
        None
    };
    // What the character's own armour takes of a blast needs (`GRENADES AND
    // KNIFE`; `blast_view::PlayerArmour`): the player prefab's hit colliders
    // and the armour's figures, the dummies' where they were read.
    let player_armour = if grenades {
        match dummy_source.as_ref() {
            Some(source) => Some(effects::blast_view::PlayerArmour { colliders: source.colliders.clone(), rules: source.rules.clone() }),
            None => match (trp_data::character::ragdoll::RagdollData::hit_colliders(&mut unity), trp_sim::condition::armor::ArmorRules::new(&db.config)) {
                (Ok(colliders), Ok(rules)) => Some(effects::blast_view::PlayerArmour { colliders, rules: std::sync::Arc::new(rules) }),
                (colliders, rules) => {
                    warn!("the character's armour takes nothing of a blast: {}", [colliders.err(), rules.err()].into_iter().flatten().map(|e| format!("{e:#}")).collect::<Vec<_>>().join("; "));
                    None
                }
            },
        }
    } else {
        None
    };
    DummiesMade { dummy_source, player_armour }
}
