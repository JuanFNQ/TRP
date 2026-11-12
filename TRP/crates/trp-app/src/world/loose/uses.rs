//! What the loose models are used by, kept between frames, and the models nothing uses.

use super::*;

/// What draws with the loose models, as last worked out (`read_models`):
/// each thing on the ground's model key (`model_key`: a weapon's is its
/// build's, a string made from its parts), made again only when what is
/// carried or lies about has changed (the carried things' change number, as
/// `slung_view::Wants` keeps its keys by); and whether anything that uses a
/// model has changed since the models nothing uses were last looked for
/// (`stale`): a flight begun or ended, a place given or taken away, the
/// grenade in hand's, a model kept or let go of (`model_for`, `let_go`),
/// one asked ahead (`Foreseen`), one come from the thread, the keys made
/// again. While none has, nothing is looked for.
#[derive(Default)]
pub(super) struct Uses {
    /// The carried things' change number the keys were made at.
    stamp: Option<u32>,
    /// Each thing on the ground with its model's key.
    keys: Vec<(Id, String)>,
    /// Something that uses a model has changed (set where it changes).
    pub(super) stale: bool,
}

impl Uses {
    /// The ground things' keys kept for the carried things' change number:
    /// made again from what lies on the ground (`ground`, looked at only
    /// then) where the number has moved, and what uses the models then to
    /// be looked through. Says whether they were made.
    pub(super) fn keep_keys<'a>(&mut self, stamp: u32, ground: impl IntoIterator<Item = &'a trp_sim::items::inventory::Item>) -> bool {
        if self.stamp == Some(stamp) {
            return false;
        }
        self.stamp = Some(stamp);
        self.keys.clear();
        self.keys.extend(ground.into_iter().map(|item| (item.id, model_key(item))));
        self.stale = true;
        true
    }

    /// The keys of the things on the ground that have a place (`placed`):
    /// those whose models are drawn where they lie.
    pub(super) fn lying<'a>(&'a self, placed: impl Fn(Id) -> bool + 'a) -> impl Iterator<Item = &'a str> + 'a {
        self.keys.iter().filter(move |(id, _)| placed(*id)).map(|(_, key)| key.as_str())
    }
}

/// The models nothing draws with: of those ready (`ready`), the ones that
/// are none of the keys in use (`in_use`: the flights', the lying things',
/// the grenade in hand's, the one asked ahead, the kept ones).
pub(super) fn unused<'a>(ready: impl Iterator<Item = &'a str>, in_use: impl Iterator<Item = &'a str>) -> Vec<String> {
    let in_use: std::collections::HashSet<&str> = in_use.collect();
    ready.filter(|key| !in_use.contains(key)).map(str::to_owned).collect()
}
