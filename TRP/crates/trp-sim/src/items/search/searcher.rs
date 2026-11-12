//! The searcher: what is hidden, beginning and stopping a search, and its advance.

use super::*;

/// A search under way (`ActiveSearchContentOperation`).
#[derive(Debug, Clone, PartialEq)]
pub struct Search {
    pub container: Id,
    /// When it began, on the clock the searcher is given
    /// (`SearchContentOperation.StartTime`: its timer counts from it).
    pub began: f64,
    /// A first search over at once (`Instant`).
    pub instant: bool,
    /// Its pace.
    pub speed: f32,
    /// Whether the container had never been searched as it began
    /// (`_firstSearch`).
    first: bool,
    /// Made and not yet run: the player's inventory controller runs an
    /// operation a frame after it is made (`ExecuteAsync`: `Task.Yield`),
    /// so its waits, and a lucky search's finds, begin on the next frame
    /// (`courses10`'s searches: each open 2 s from the frame after the
    /// panel's, its timer from the panel's).
    pending: bool,
    /// When a container never searched opens; none once open.
    opens: Option<f64>,
    /// When the next thing is found, once open.
    next: Option<f64>,
}

/// What searching did.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// A search begun that is over at once: the game's luck sound
    /// (`looting_luck2`, `PlayInstantSearchSound`).
    Lucky(Id),
    /// A container opened (`SetItemAsSearched`): its grids shown.
    Opened(Id),
    /// A thing found (`DiscoverItem`), with its sound unless the search was
    /// over at once (`PlayDiscoverSound`: the thing's own drop).
    Found { container: Id, item: Id, sounded: bool },
    /// A search ended: all found, or nothing left to find.
    Ended(Id),
}

/// Why a search was not begun.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    /// That container is being searched already.
    Searching,
    /// As many as the character may make at once are under way
    /// (`CanStartNewSearchOperation`).
    Busy,
}

/// The character's searching (`ActiveSearchController` and its
/// operations).
#[derive(Debug, Clone, PartialEq)]
pub struct Searcher {
    unsearched: HashSet<Id>,
    unknown: HashSet<Id>,
    pub under_way: Vec<Search>,
    pub(super) dice: Dice,
}

impl Default for Searcher {
    fn default() -> Self {
        Self::new(1)
    }
}

/// The things in a container's grids, in the order they lie in each grid
/// (`Item.GetFirstLevelItems` over its searchable grids): the order they are
/// found in.
fn first_level(container: &Item) -> impl Iterator<Item = &Item> {
    container.grids.iter().flatten().map(|(_, item)| item)
}

impl Searcher {
    pub fn new(seed: u64) -> Self {
        Self { unsearched: HashSet::new(), unknown: HashSet::new(), under_way: Vec::new(), dice: Dice::new(seed) }
    }

    /// Makes a container of the world unsearched and what lies in its grids
    /// unknown, as a raid begins, and every searchable thing in it as well
    /// (`hide_within`): a bag or a vest that lies in it is not the
    /// character's own, so is not searched either.
    pub fn hide(&mut self, container: &Item) {
        self.unsearched.insert(container.id);
        self.unknown.extend(first_level(container).map(|item| item.id));
        for item in container.contents() {
            self.hide_within(item);
        }
    }

    /// Hides a body's gear from the character (`ActiveSearchController`:
    /// only its own equipment is uncovered as it is made, so a searchable
    /// thing of anyone else's is unsearched until searched, and a thing in
    /// its grids unknown until found): every searchable thing the body
    /// wears or carries, at any depth, is unsearched, and what lies in its
    /// grids unknown. What is in a slot is known (`IsItemKnown`: only a
    /// searchable grid hides), so the gear worn shows; the body itself is
    /// not a container that is searched.
    pub fn hide_body(&mut self, equipment: &Item) {
        for item in equipment.contents() {
            self.hide_within(item);
        }
    }

    /// Hides a thing of the world's lying loose, not in a container: if it
    /// is searchable, it is unsearched with what lies in its grids unknown,
    /// and so on down (`hide_within`); else what it holds is hidden as far
    /// as it is searchable.
    pub fn hide_thing(&mut self, item: &Item) {
        self.hide_within(item);
    }

    /// That thing, if searchable, unsearched with what lies in its grids
    /// unknown; and so on down whatever it holds.
    fn hide_within(&mut self, item: &Item) {
        if searchable(item) {
            self.unsearched.insert(item.id);
            self.unknown.extend(first_level(item).map(|inside| inside.id));
        }
        for inside in item.contents() {
            self.hide_within(inside);
        }
    }

    /// Forgets whatever was hidden of a thing and what it holds, and stops
    /// its searches: a body dressed afresh, its old gear gone.
    pub fn forget(&mut self, root: &Item) {
        fn ids(item: &Item, into: &mut Vec<Id>) {
            into.push(item.id);
            for inside in item.contents() {
                ids(inside, into);
            }
        }
        let mut gone = Vec::new();
        ids(root, &mut gone);
        for id in &gone {
            self.unsearched.remove(id);
            self.unknown.remove(id);
        }
        self.under_way.retain(|search| !gone.contains(&search.container));
    }

    /// Whether a container has been searched (`IsSearched`): every
    /// container not hidden has.
    pub fn searched(&self, container: Id) -> bool {
        !self.unsearched.contains(&container)
    }

    /// Whether a thing is known where it lies (`IsItemKnown`).
    pub fn known(&self, item: Id) -> bool {
        !self.unknown.contains(&item)
    }

    /// Whether anything at all is still hidden.
    pub fn hides_anything(&self) -> bool {
        !self.unsearched.is_empty() || !self.unknown.is_empty()
    }

    /// Whether a container holds anything not yet found
    /// (`ContainsUnknownItems`).
    pub fn holds_unknown(&self, container: &Item) -> bool {
        first_level(container).any(|item| !self.known(item.id))
    }

    /// The search of that container, if one is under way.
    pub fn searching(&self, container: Id) -> Option<&Search> {
        self.under_way.iter().find(|search| search.container == container)
    }

    /// Whether another search may begin (`CanStartNewSearchOperation`):
    /// one at a time, two at Search's elite.
    pub fn can_start(&self, skills: &Skills) -> bool {
        self.under_way.len() < if skills.double { 2 } else { 1 }
    }

    /// Begins a search of a container (`SearchContents`): made now, its
    /// luck rolled (Attention's elite, a first search only), and run on the
    /// next frame (`advance`): then a first one waits to open unless it is
    /// over at once; one of a container opened before goes straight to
    /// what is left to find. Nothing is done as it begins: the events
    /// (the luck's among them) come as it runs.
    pub fn begin(&mut self, container: &Item, on_body: bool, skills: &Skills, now: f64) -> Result<Vec<Event>, Refused> {
        if self.searching(container.id).is_some() {
            return Err(Refused::Searching);
        }
        if !self.can_start(skills) {
            return Err(Refused::Busy);
        }
        let first = !self.searched(container.id);
        let instant = first && self.dice.unit() < skills.lucky as f64;
        let speed = skills.speed(on_body);
        self.under_way.push(Search { container: container.id, began: now, instant, speed, first, pending: true, opens: None, next: None });
        Ok(Vec::new())
    }

    /// Stops a search (`StopSearching`: the stop button, or the view
    /// shut): a container not yet open stays unsearched, and what was
    /// found stays found.
    pub fn stop(&mut self, container: Id) -> bool {
        let before = self.under_way.len();
        self.under_way.retain(|search| search.container != container);
        self.under_way.len() != before
    }

    /// Stops every search.
    pub fn stop_all(&mut self) {
        self.under_way.clear();
    }

    /// Moves the searches on to `now`, the frame's time: a container
    /// opened, a thing found, a search ended (`Open`, `SearchContent`), at
    /// most one step a search on a frame but where it waits nothing (a
    /// search over at once), each next wait counted from `now`. `find`
    /// gives a container by its number; a search whose container is gone
    /// ends.
    pub fn advance<'a>(&mut self, now: f64, find: impl Fn(Id) -> Option<&'a Item>) -> Vec<Event> {
        let mut events = Vec::new();
        let mut index = 0;
        while index < self.under_way.len() {
            let Some(container) = find(self.under_way[index].container) else {
                let gone = self.under_way.remove(index);
                events.push(Event::Ended(gone.container));
                continue;
            };
            let mut ended = false;
            // Run on the first frame after the one it was made on
            // (`ExecuteInternal`): the luck's sound, then the wait to open,
            // or for the first thing (`Open`, `SearchContent`).
            if self.under_way[index].pending {
                if now <= self.under_way[index].began {
                    index += 1;
                    continue;
                }
                let (first, instant, speed) = {
                    let search = &mut self.under_way[index];
                    search.pending = false;
                    (search.first, search.instant, search.speed)
                };
                if instant {
                    events.push(Event::Lucky(container.id));
                }
                if first && !instant {
                    self.under_way[index].opens = Some(now + OPEN_TIME);
                } else {
                    if first {
                        self.unsearched.remove(&container.id);
                        events.push(Event::Opened(container.id));
                    }
                    let wait = if self.holds_unknown(container) { find_time(&mut self.dice, speed, instant) } else { 0.0 };
                    self.under_way[index].next = Some(now + wait);
                }
            }
            loop {
                let (opens, next, speed, instant) = {
                    let search = &self.under_way[index];
                    (search.opens, search.next, search.speed, search.instant)
                };
                if let Some(at) = opens {
                    if now < at {
                        break;
                    }
                    self.under_way[index].opens = None;
                    self.unsearched.remove(&container.id);
                    events.push(Event::Opened(container.id));
                    // `SearchContent` from the open's end: nothing to
                    // find, it ends.
                    if !self.holds_unknown(container) {
                        ended = true;
                        break;
                    }
                    // Each wait begins as the last step lands, on the
                    // frame it is seen (`SearchContent` awaits each
                    // `Task.Delay` after the one before has resumed on a
                    // frame), not when it was due: `courses10`'s searches
                    // run 2 to 10 ms late a step, never early.
                    let wait = find_time(&mut self.dice, speed, instant);
                    self.under_way[index].next = Some(now + wait);
                    continue;
                }
                let Some(at) = next else {
                    ended = true;
                    break;
                };
                if !self.holds_unknown(container) {
                    ended = true;
                    break;
                }
                if now < at {
                    break;
                }
                let Some(found) = first_level(container).find(|item| !self.known(item.id)).map(|item| item.id) else {
                    ended = true;
                    break;
                };
                self.unknown.remove(&found);
                events.push(Event::Found { container: container.id, item: found, sounded: !instant });
                if !self.holds_unknown(container) {
                    ended = true;
                    break;
                }
                let wait = find_time(&mut self.dice, speed, instant);
                self.under_way[index].next = Some(now + wait);
            }
            if ended {
                let done = self.under_way.remove(index);
                events.push(Event::Ended(done.container));
            } else {
                index += 1;
            }
        }
        events
    }

    /// The dice, for what else a run rolls with them (the crates' fill).
    pub fn dice(&mut self) -> &mut Dice {
        &mut self.dice
    }
}
