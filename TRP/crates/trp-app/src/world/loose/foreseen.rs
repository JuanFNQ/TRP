//! What the inventory screen may put down next: the models' reader kept and one model asked ahead.

use super::*;

/// How long the mouse rests on a weapon in the inventory screen before its
/// model for the world is asked for, in seconds. `ESTIMATE`: the app's
/// choice (the hands' rig waits a quarter of a second on the same sign,
/// `inventory_view::foresee`; a model is the lighter of the two, and a
/// mouse on its way over a box is on it for less).
pub(super) const RESTS_ON: f32 = 0.1;

/// What the inventory screen shows of what may be put down next (`GameWorld.
/// ThrowItem` from the screen: its key puts down the thing the mouse carries
/// or is on, the menu's `DISCARD` the menu's thing).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(super) struct Signs {
    /// The screen is open.
    pub(super) open: bool,
    /// The thing the mouse is on (`inventory_view::Pointed`).
    pub(super) pointed: Option<Id>,
    /// The thing the mouse carries (`inventory_view::Dragging`).
    pub(super) taken: Option<Id>,
    /// The thing whose menu is open (`item_menu_view::ItemMenu`).
    pub(super) menu: Option<Id>,
}

/// What is had ahead of a throw from the inventory screen, the game making
/// a thrown thing's object in the throw itself (`docs/spec/physics.md`,
/// "Loose things"): the models' reader, made as the screen opens and kept
/// while it is open (`read_models`), and the model of the one thing the
/// signs say may be put down next, asked for at the sign and kept while it
/// lasts, as the grenade in hand's is (`Loose::held`). The sign over or the
/// screen shut, the model is let go of as any nothing draws with
/// (`uses::unused`), unless the thing was thrown: its flight then has it.
#[derive(Default)]
pub(super) struct Foreseen {
    /// The screen is open: the reader is made and kept.
    pub(super) open: bool,
    /// The reader has been asked for since the screen opened (once an
    /// opening: one that cannot be made is not tried again every frame).
    pub(super) reader_asked: bool,
    /// The thing the mouse is on, and since when (the app's clock, s).
    resting: Option<(Id, f32)>,
    /// The thing that may be put down next, the carried things' change
    /// number its key was made at, and its model's key (none for a thing
    /// whose model is not asked ahead).
    thing: Option<(Id, u32, Option<String>)>,
}

impl Foreseen {
    /// The model held ahead.
    pub(super) fn key(&self) -> Option<&str> {
        self.thing.as_ref().and_then(|(.., key)| key.as_deref())
    }

    /// A frame's signs taken: the thing that may be put down next is the
    /// one the mouse carries, else the one whose menu is open, else the one
    /// the mouse has rested on for `RESTS_ON`; none with the screen shut.
    /// Its model's key (`key_of`: none for a thing not asked ahead) is made
    /// when the thing changes or the carried things have (`stamp`, their
    /// change number: a build's key changes with its parts). Says whether
    /// the key held ahead has changed: the new one is then to be asked for,
    /// and what uses the models looked through for the old.
    pub(super) fn frame(&mut self, now: f32, signs: Signs, stamp: u32, key_of: impl FnOnce(Id) -> Option<String>) -> bool {
        if self.open != signs.open {
            (self.open, self.reader_asked) = (signs.open, false);
        }
        match signs.pointed.filter(|_| signs.open) {
            Some(id) if self.resting.is_some_and(|(on, _)| on == id) => {}
            Some(id) => self.resting = Some((id, now)),
            None => self.resting = None,
        }
        let rested = self.resting.filter(|(_, since)| now - since >= RESTS_ON).map(|(id, _)| id);
        let thing = if signs.open { signs.taken.or(signs.menu).or(rested) } else { None };
        if self.thing.as_ref().map(|(id, at, _)| (*id, *at)) == thing.map(|id| (id, stamp)) {
            return false;
        }
        let before = self.thing.take().and_then(|(.., key)| key);
        self.thing = thing.map(|id| (id, stamp, key_of(id)));
        before.as_deref() != self.key()
    }
}

impl Loose {
    /// The inventory screen's signs of a throw taken this frame
    /// (`Foreseen::frame`): a weapon's model asked for ahead, by the key
    /// its throw would ask it by (`model_key`). Only a weapon's: any other
    /// thing's is read within the time the game hides a thrown thing for
    /// (`ThrowLootMakeVisibleDelay`) once the reader is there. Only a
    /// thing the screen can put down: one of the character's, or of the
    /// container open beside it.
    pub(super) fn foresee(&mut self, now: f32, signs: Signs, carried: &Carried, stamp: u32) {
        let key_of = |id| {
            let item = carried.equipment.item.find(id).or_else(|| carried.loot.find(id))?;
            trp_sim::items::inventory::is(item, "Weapon").then(|| model_key(item))
        };
        if self.foreseen.frame(now, signs, stamp, key_of) {
            self.uses.stale = true;
            if let Some(key) = self.foreseen.key().map(str::to_owned) {
                debug!("the model of {key} asked for ahead: the inventory may put it down");
                self.ask(&key);
            }
        }
    }
}
