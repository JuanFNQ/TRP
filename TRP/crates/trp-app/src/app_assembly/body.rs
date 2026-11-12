//! The body's parts, its animator and standing pose, the movement made from them, and the hands' data.

use super::*;

pub(super) struct BodyInputs<'a> {
    pub(super) unity: &'a mut UnityAssets,
    pub(super) db: &'a std::sync::Arc<Database>,
    pub(super) options: &'a Options,
    pub(super) present: &'a std::collections::HashSet<String>,
    pub(super) options_state: &'a screens::options_view::OptionsState,
    pub(super) slung_bones: &'a Vec<String>,
    pub(super) dummy_looks: &'a Vec<[String; 3]>,
    pub(super) with_dummy: bool,
    pub(super) movement_settings: trp_sim::MovementSettings,
    pub(super) buffs: &'a trp_sim::condition::skills::Buffs,
    pub(super) skills: &'a player::skills::Skills,
    pub(super) hard: &'a HardSettings,
}

pub(super) struct BodyAndMovement {
    pub(super) clothing: player::body_view::BodyParts,
    pub(super) with_poppers: bool,
    pub(super) scope_pictures: bool,
    pub(super) boot_shadow: bool,
    pub(super) body_data: trp_sim::locomotion::body::BodyData,
    pub(super) movement: trp_sim::Movement,
    pub(super) hands_data: Option<HandsData>,
}

/// Reads the body's parts and animator with the poses the screens stand it in, makes the movement and reads the hands' data.
pub(super) fn load(inputs: BodyInputs) -> anyhow::Result<BodyAndMovement> {
    let BodyInputs { mut unity, db, options, present, options_state, slung_bones, dummy_looks, with_dummy, movement_settings, buffs, skills, hard } = inputs;
    let mut clothing = player::body_view::BodyParts::load(&mut unity, &db, &[&options.body, &options.legs, &options.head], &options.wear, &present, options_state.in_force.custom_hands, &slung_bones, &dummy_looks, options_state.in_force.head_fitting, options_state.in_force.armband);
    // Every upper body's customization ids, which an armband's meshes go by
    // (`body_view::fit_armbands`), with `ARMBAND` on.
    if options_state.in_force.armband {
        match trp_data::character::suits::Customizations::load(&unity.install) {
            Ok(customizations) => clothing.body_ids = customizations.body_ids(),
            Err(e) => eprintln!("the install's customizations could not be read; an armband takes its first meshes on every body: {e:#}"),
        }
    }
    // And, with the options' `CHARACTER MODEL` on, the pose the game's
    // screens stand the character in, for the model of it on the gear
    // page and on the OVERALL tab; with `TARKOV UI` on the gear page has
    // the game's flat outline of the character in its place, as the game's
    // does, and the OVERALL tab alone, as the game's, has
    // the model.
    platform::run_log::phase(&format!("the body's parts ({} meshes, {} models on the head, {} worn things)", clothing.parts.len(), clothing.on_head.len(), options.wear.len()));
    let model = options_state.in_force.character_model;
    // The dummy stands so too (`dummy`, `with_dummy` above).
    let with_poppers = options_state.in_force.reactive_targets;
    // The scopes' second picture (`optic_view`), with `ATTACHMENTS`.
    let scope_pictures = options_state.in_force.attachments;
    let standing: &[String] = if model || with_dummy { &clothing.standing } else { &[] };
    // The shadow at the OVERALL figure's feet, wanted with its figure.
    let mut boot_shadow = false;
    let body_data = match trp_sim::locomotion::body::BodyData::load_standing(&mut unity, &clothing.bones, standing) {
        Ok((data, standing)) => {
            match standing {
                Some(Ok(pose)) => {
                    if with_dummy {
                        clothing.dummy_pose = Some(pose.clone());
                    }
                    if model {
                        clothing.standing_pose = Some(pose);
                    }
                    // With `OVERALL FIGURE` the OVERALL tab's own: the game's
                    // menu figure on the menus' player's skeleton, the body's
                    // meshes' bones among it (`figure_view::Overall`).
                    if model && options_state.in_force.overall_figure {
                        let began = std::time::Instant::now();
                        match trp_sim::characters::figure::FigureData::load(&mut unity, &clothing.standing) {
                            Ok(data) => {
                                println!("the OVERALL tab's own figure: the menu figure's body read in {:.0} ms, {} bones", began.elapsed().as_secs_f32() * 1000.0, data.paths().len());
                                clothing.overall = Some(data);
                                // And the shadow at its feet, read on a
                                // worker (`boot_shadow`).
                                boot_shadow = true;
                            }
                            Err(e) => eprintln!("the OVERALL tab's own figure could not be read; the OVERALL tab draws the gear page's still figure: {e:#}"),
                        }
                    }
                }
                Some(Err(e)) => eprintln!("the character model is not drawn: {e:#}"),
                None => {}
            }
            data
        }
        Err(e) => {
            eprintln!("the body is not drawn: {e:#}");
            clothing = player::body_view::BodyParts::default();
            trp_sim::locomotion::body::BodyData::load(&mut unity)?
        }
    };
    let body = trp_sim::Body::new(&body_data, 1.0);
    let mut movement = trp_sim::Movement::new(movement_settings, body);
    // What the skills make of the movement, the weight's limits, stamina
    // and the steps.
    player::skills::put_on_movement(&mut movement, &buffs, skills.carry(&buffs), (&hard, &db.config));
    movement.stamina.resettle();
    movement.pitch = options.pitch;
    (movement.yaw, movement.body_yaw) = (options.yaw, options.yaw);
    movement.note_rotation();
    platform::run_log::phase("the body's animator");
    let hands_data = match HandsData::load(&mut unity) {
        Ok(data) => Some(data),
        Err(e) => {
            eprintln!("weapon motion data could not be read: {e:#}");
            None
        }
    };
    Ok(BodyAndMovement { clothing, with_poppers, scope_pictures, boot_shadow, body_data, movement, hands_data })
}
