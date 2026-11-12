//! The range's dummies: their kits, their looks and their voices.

use super::*;

pub(super) struct Dummies {
    pub(super) with_dummy: bool,
    pub(super) dummy_kits: Vec<trp_sim::items::inventory::Item>,
    pub(super) dummy_looks: Vec<[String; 3]>,
    pub(super) dummy_voice_seed: u64,
    pub(super) dummy_voices: Vec<Option<String>>,
}

/// Makes each dummy's kit and picks its look and its voice.
pub(super) fn pick(options_state: &screens::options_view::OptionsState, options: &Options, unity: &UnityAssets, db: &std::sync::Arc<Database>) -> Dummies {
    // The clothing the body is drawn in names the bones the animator has
    // to pose for it.

    // The dummies on the range (`DUMMY`, with the ballistics: nothing would
    // hit them without), each with a kit and a look of its own, apart from
    // the character's (`dummy::SIDES`): its kit a new PMC's, its look picked
    // as SPT's server picks a PMC bot's, anew each run.
    platform::run_log::phase("the graphics' settings");
    let with_dummy = options_state.in_force.dummy && options_state.in_force.ballistics;
    // The range's crowd (`--start crowd-<n>`: the posing economy's scene,
    // `dummy::crowd`): its dummies past the two each a copy of its side's
    // kit with numbers of its own, the character's look, and no voice.
    if let Some(n) = options.start.as_deref().and_then(world::dummy::crowd_of).filter(|_| with_dummy) {
        world::dummy::set_crowd(n);
        println!("the range's crowd: {} dummies (`--start crowd-{n}`)", world::dummy::places().len());
    }
    let dummy_kits: Vec<trp_sim::items::inventory::Item> = if with_dummy {
        let sides: Vec<trp_sim::items::inventory::Item> = world::dummy::SIDES
            .iter()
            .map(|side| {
                dummy_kit(&unity.install, &db, side).unwrap_or_else(|e| {
                    eprintln!("the dummy's kit of a new {side} could not be made, and it wears nothing: {e:#}");
                    trp_sim::items::inventory::Equipment::empty(&db).map(|equipment| equipment.item).expect("an empty equipment")
                })
            })
            .collect();
        let crowd = (sides.len()..world::dummy::places().len()).map(|which| sides[which % sides.len()].renumbered()).collect::<Vec<_>>();
        sides.into_iter().chain(crowd).collect()
    } else {
        Vec::new()
    };
    let dummy_looks: Vec<[String; 3]> = if with_dummy {
        let seed = fixed_seed(0x100c).unwrap_or_else(|| std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |d| d.as_nanos() as u64));
        let mut dice = trp_sim::items::search::Dice::new(seed);
        match trp_data::character::suits::Customizations::load(&unity.install) {
            Ok(customizations) => world::dummy::SIDES
                .iter()
                .enumerate()
                .map(|(which, side)| match dummy_look(&unity.install, &customizations, side, &mut dice) {
                    Ok(look) => {
                        let name = |bundle: &str| bundle.rsplit('/').next().unwrap_or(bundle).trim_end_matches(".bundle").to_owned();
                        println!("dummy {which}'s look, a {side} PMC's: {}", look.iter().map(|bundle| name(bundle)).collect::<Vec<_>>().join(", "));
                        look
                    }
                    Err(e) => {
                        eprintln!("dummy {which}'s look could not be picked, and it has the character's: {e:#}");
                        Default::default()
                    }
                })
                .chain((world::dummy::SIDES.len()..world::dummy::places().len()).map(|_| Default::default()))
                .collect(),
            Err(e) => {
                eprintln!("the install's customizations could not be read, and the dummies look as the character: {e:#}");
                Vec::new()
            }
        }
    } else {
        Vec::new()
    };
    // Each dummy's voice, drawn as SPT's server draws a PMC bot's, anew each
    // run, from dice of their own (the looks' draws stay as they were), and
    // the seed of their speakers' draws (`dummy_voice`).
    let clock = || std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |d| d.as_nanos() as u64);
    let dummy_voice_seed = fixed_seed(0x100e).unwrap_or_else(|| clock() ^ 0x5bd1_e995);
    let dummy_voices: Vec<Option<String>> = if with_dummy {
        let mut dice = trp_sim::items::search::Dice::new(fixed_seed(0x100d).unwrap_or_else(clock));
        match trp_data::character::suits::Customizations::load(&unity.install) {
            Ok(customizations) => world::dummy::SIDES
                .iter()
                .enumerate()
                .map(|(which, side)| match dummy_voice(&unity.install, &customizations, side, &mut dice) {
                    Ok(Some(name)) => {
                        println!("dummy {which}'s voice, a {side} PMC's: {name}");
                        Some(name)
                    }
                    Ok(None) => {
                        eprintln!("dummy {which}'s side lists no voice, and it says nothing");
                        None
                    }
                    Err(e) => {
                        eprintln!("dummy {which}'s voice could not be drawn, and it says nothing: {e:#}");
                        None
                    }
                })
                .chain((world::dummy::SIDES.len()..world::dummy::places().len()).map(|_| None))
                .collect(),
            Err(e) => {
                eprintln!("the install's customizations could not be read, and the dummies say nothing: {e:#}");
                Vec::new()
            }
        }
    } else {
        Vec::new()
    };
    Dummies { with_dummy, dummy_kits, dummy_looks, dummy_voice_seed, dummy_voices }
}
