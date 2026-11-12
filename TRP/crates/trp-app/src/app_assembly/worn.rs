//! What is worn as the run begins, the suits, the slung weapons' bones and the wrist watch.

use super::*;

pub(super) struct Worn {
    pub(super) dummy_kits: Vec<trp_sim::items::inventory::Item>,
    pub(super) present: std::collections::HashSet<String>,
    pub(super) wardrobe: Option<player::clothing::Clothing>,
    pub(super) slung: Option<player::slung_view::Slung>,
    pub(super) slung_bones: Vec<String>,
    pub(super) wrist_watch: Option<player::watch_view::WristWatch>,
}

/// Gathers what the character and the dummies wear, reads the suits, the weapons' bones on the body and the watch; the dummies' kits handed back.
#[allow(clippy::too_many_arguments)]
pub(super) fn read(equipment: &trp_sim::items::inventory::Equipment, dummy_kits: Vec<trp_sim::items::inventory::Item>, options: &mut Options, options_state: &screens::options_view::OptionsState, mut unity: &mut UnityAssets, locale: trp_data::character::inventory::Locale, db: &std::sync::Arc<Database>, dummy_looks: &[[String; 3]]) -> Worn {
    // What is worn as the run begins, in the slots the game draws, by the
    // character and by each dummy: read for the body with what `--wear`
    // named; and every part fitted to it, of which a model on the head
    // reads those it may have fitted. Nothing else: a thing put on later is
    // read when it is (`worn_reads`), so that only what is seen at start-up
    // is loaded, where every thing the run had
    // anywhere was read with its meshes for every case (431 things, 405
    // meshes and 203 models on the head, two seconds and more of the start).
    platform::run_log::phase("the dummies' kits and looks");
    let present: std::collections::HashSet<String> = {
        let mut found = std::collections::HashSet::new();
        let worn = std::iter::once(&equipment.item).chain(&dummy_kits).flat_map(|gear| trp_sim::items::inventory::WORN.iter().filter_map(|slot| gear.in_slot(slot)));
        for item in worn {
            if !options.wear.contains(&item.kind.template) {
                options.wear.push(item.kind.template.clone());
            }
            let mut stack: Vec<&trp_sim::items::inventory::Item> = vec![item];
            while let Some(item) = stack.pop() {
                found.insert(item.kind.template.clone());
                stack.extend(item.slots.iter().flatten());
            }
        }
        found
    };
    // The suits of the install (`CLOTHING`), and those to begin in: as
    // `--suit` says, else as the options' file keeps them (for someone at
    // the keys), each where the run was not told its bundles.
    platform::run_log::phase("every thing the run has");
    let wardrobe = if options_state.in_force.clothing {
        let suits = trp_data::character::suits::suits(&unity.install, &locale).unwrap_or_else(|e| {
            eprintln!("the install's suits could not be read; none is offered: {e:#}");
            Vec::new()
        });
        let file = (!options.unattended()).then(screens::options_view::options_file);
        let kept: Vec<String> = file.as_deref().map(player::clothing::kept).into_iter().flatten().flatten().collect();
        for id in kept.iter().filter(|_| options.suits.is_empty()).chain(&options.suits) {
            let Some(suit) = suits.iter().find(|suit| &suit.id == id) else {
                eprintln!("no suit {id} in the install's customization list");
                continue;
            };
            match (suit.upper, &suit.body, &suit.hands, &suit.feet) {
                (true, Some(body), hands, _) if options.body == DEFAULT_BODY => {
                    options.body = body.clone();
                    if let Some(hands) = hands.as_ref().filter(|_| options.arms == DEFAULT_ARMS) {
                        options.arms = hands.clone();
                    }
                }
                (false, _, _, Some(feet)) if options.legs == DEFAULT_LEGS => options.legs = feet.clone(),
                _ => {}
            }
            println!("begun in the suit {}", suit.name);
        }
        Some(player::clothing::Clothing::new(unity.install.clone(), suits, &options.body, &options.legs, file).fitted(options_state.in_force.head_fitting))
    } else {
        None
    };
    // The texts as the file has them, wanted no more (their words kept).
    drop(locale);
    // The weapons on the body (`WEAPONS ON THE BODY`): the slot bones the
    // skeleton is to have for them, and the legs' holster.
    let slung = if options_state.in_force.weapons_on_body {
        match trp_data::character::body::slot_bones(&mut unity) {
            Ok(bones) => {
                let holster = trp_data::character::arms::legs_holster(&mut unity, &options.legs).unwrap_or_else(|e| {
                    eprintln!("the legs' holster could not be read; no pistol is hung: {e:#}");
                    None
                });
                // Each dummy's by its own legs, or the character's it wears
                // where it has none of its own (for the weapons on its body).
                let dummy_holsters = dummy_looks
                    .iter()
                    .map(|look| match look[player::body_view::LOOK_LOWER].as_str() {
                        "" => holster,
                        legs => trp_data::character::arms::legs_holster(&mut unity, legs).unwrap_or_else(|e| {
                            eprintln!("a dummy's legs' holster could not be read; none: {e:#}");
                            None
                        }),
                    })
                    .collect();
                Some(player::slung_view::Slung::new(&db, bones, holster, dummy_holsters))
            }
            Err(e) => {
                eprintln!("the weapons' bones on the body could not be read; none is hung: {e:#}");
                None
            }
        }
    } else {
        None
    };
    let slung_bones = slung.as_ref().map(player::slung_view::Slung::wanted_bones).unwrap_or_default();
    // The watch on the wrist, with its switch on (`watch_view`): the
    // install's hands entries' watches.
    let wrist_watch = if options_state.in_force.wrist_watch {
        player::watch_view::WristWatch::new(&unity.install, options.clock)
            .map_err(|e| eprintln!("the hands' watches could not be read; none is worn: {e:#}"))
            .ok()
    } else {
        None
    };
    platform::run_log::phase("the suits, the slung weapons' bones");
    Worn { dummy_kits, present, wardrobe, slung, slung_bones, wrist_watch }
}
