//! The keys, the options tab's state and the features' switches as the run begins.

use super::*;

pub(super) struct Switches {
    pub(super) bindings: platform::keys::Bindings,
    pub(super) families: trp_sim::condition::mastering::Families,
    pub(super) options_state: screens::options_view::OptionsState,
    pub(super) unfinished: std::sync::Arc<std::collections::HashSet<String>>,
}

/// Reads the keys, the options tab's state and the features' switches for the run, and what is hidden as unfinished.
pub(super) fn read(install: &Install, mut options: &mut Options, words: &std::sync::Arc<std::collections::HashMap<String, String>>, db: &std::sync::Arc<Database>) -> Switches {
    // The keys as the install binds them, and what the options tab has
    // kept over them and for the display (for someone at the keys only);
    // the features' switches as the run begins, which hold until it ends
    // (each is "restart required": `options_view::Features`). Read before
    // the range's things, of which `HIDE UNFINISHED ITEMS` lays out fewer.
    let mut bindings = platform::keys::Bindings::of(&install);
    let window = options.window;
    let families = trp_sim::condition::mastering::Families::of_config(&db.config);
    let options_state = screens::options_view::OptionsState::new(&mut options, &install, &words, &mut bindings, window, families.clone());
    // The levels of detail by distance (the characters' `distance_detail`,
    // the world's things' `world_distance_detail`), known to every reader
    // of models from here on (`lod_view`).
    assets::lod_view::set_on(options_state.in_force.distance_detail);
    assets::lod_view::set_world_on(options_state.in_force.world_distance_detail);
    let unfinished = std::sync::Arc::new(unfinished_items(&install, options_state.in_force.hide_unfinished));
    Switches { bindings, families, options_state, unfinished }
}
