//! The Dummy and Dummies types, their placement, hits and numbering.

use super::*;

/// One dummy's character.
pub struct Dummy {
    pub(super) which: usize,
    pub(super) colliders: Vec<Collider>,
    pub(super) placed: Vec<Placed>,
    /// Each collider's object's place as last posed (the game's axes): what
    /// a blast's lines go to (`BodyPartCollider.transform.position`).
    pub(super) objects: Vec<[f32; 3]>,
    pub wearer: Wearer,
    /// Its armour's wear changed by a hit or a blast since it was last
    /// written onto its things (`keep_dummy_wear`).
    pub(super) worn: bool,
    pub health: Health,
    /// Its health followed by its character this frame (`run_dummies`),
    /// not to be stepped again (`place_dummies`).
    pub(super) followed: bool,
    /// Its body's gear as its character last read it: the weight, and what
    /// it wears.
    pub(super) load: (f32, Vec<Worn>),
    /// Given its kit.
    pub(super) dressed: bool,
    /// This frame's hits it took: the bullet (shot, fragment, seed) and the
    /// material it was, for its effect.
    pub(super) taken: Vec<((u32, i32, i32), i32)>,
    pub(super) killing: Option<Killing>,
    pub(super) fall: Option<Fall>,
    pub(super) hits: u32,
    /// Its character, where it could be made, and the way the walker heads
    /// along the lanes (+1 toward the world's +x).
    pub(super) puppet: Option<Puppet>,
    pub(super) heading: f32,
    pub(super) model: Option<Model>,
    /// The empty hands' grip, for a character holding nothing.
    pub(super) empty_grip: Option<Grip>,
    /// Its own AKM (its kit's, in its rifle's slot as it was made), and
    /// where on its body its hands hold it from (`held_of`), none while
    /// they are empty (`Player.SetControllerInsteadRemovedOne`: no weapon
    /// to take) or it has no character.
    pub(super) own: Option<trp_sim::items::inventory::Id>,
    pub(super) holds: Option<(usize, trp_sim::items::inventory::Id)>,
    /// The skeleton's bones the body's animator gives, in its order; and
    /// each arm's (the collarbone, the upper arm, the three forearm bones,
    /// the palm), right then left.
    pub(super) drawn: Vec<Option<usize>>,
    pub(super) arm_bones: [[Option<usize>; 6]; 2],
    /// Each skeleton bone in its parent as last posed (as drawn).
    pub(super) locals: Vec<Transform>,
    /// Its skeleton's parents (the rig's), by which its bones are put
    /// together for its colliders wherever they are asked for.
    pub(super) parents: Arc<[Option<usize>]>,
    /// The frame of its character its hit colliders are of while it stands,
    /// as `place_dummies` last said (its newest then), and the one it said
    /// the frame before: what the bullets meet, and what the knife and a
    /// blast meet (the frame before's: `blast_on_dummies`). None before its
    /// character's first frame, and from its fall on (its colliders then
    /// on its skeleton as it stands: `placed`).
    pub(super) colliders_frame: Option<u64>,
    pub(super) before_frame: Option<u64>,
    /// Its colliders placed on the frames asked for (the last few), each
    /// made once, when first asked (`placement`), not every frame.
    pub(super) placements: Mutex<Vec<Arc<Placement>>>,
    /// How it is drawn by its size on screen (`run_dummies`), and the frame
    /// last drawn while held.
    pub(super) drawn_by: PosingDrawn,
    pub(super) held_frame: Option<u64>,
    /// What its voice is to be told, in the order it happened, while one
    /// listens (`Dummies::listened`; `dummy_voice`).
    pub(super) heard: Vec<Heard>,
}

/// A standing dummy's hit colliders placed on one of its character's frames
/// (`Dummy::placement`): its root and skeleton as that frame's pose puts
/// them (the arms on the animator's straight pose: `posed`), each collider
/// placed by its bone, and each collider's object's place (the game's
/// axes), which a blast's lines go to.
pub(super) struct Placement {
    pub(super) frame: u64,
    pub(super) root: Transform,
    pub(super) locals: Vec<Transform>,
    pub(super) placed: Vec<Placed>,
    pub(super) objects: Vec<[f32; 3]>,
}

/// How many placements were made, for the posing's log line.
pub(super) static PLACEMENTS_MADE: AtomicU32 = AtomicU32::new(0);

/// A dummy's hit colliders as a reader meets them: on a frame of its
/// character's (`Placement`), or as its skeleton stands (fallen, or before
/// its character's first frame: `Dummy::placed`).
pub(super) enum Colliders<'a> {
    Posed(Arc<Placement>),
    Laid(&'a [Placed], &'a [[f32; 3]]),
}

impl Colliders<'_> {
    pub(super) fn placed(&self) -> &[Placed] {
        match self {
            Colliders::Posed(placement) => &placement.placed,
            Colliders::Laid(placed, _) => placed,
        }
    }

    pub(super) fn objects(&self) -> &[[f32; 3]] {
        match self {
            Colliders::Posed(placement) => &placement.objects,
            Colliders::Laid(_, objects) => objects,
        }
    }
}

/// What happened to a dummy that its voice goes by (`dummy_voice`): a hit
/// reaching a part before its damage is taken (`ApplyDamageInfo`: the part,
/// the damage, its kind), its health's events, or its character made anew
/// (the debug panel's HEAL AND STAND UP).
#[derive(Debug, Clone, PartialEq)]
pub enum Heard {
    Struck(Part, f32, trp_sim::condition::health::DamageKind),
    Health(trp_sim::condition::health::Event),
    Anew,
}

impl Dummy {
    pub(super) fn character(&self) -> u32 {
        self.which as u32 + 1
    }

    pub(super) fn base(&self) -> u64 {
        THINGS + self.which as u64 * THINGS_EACH
    }

    /// Its number.
    pub fn number(&self) -> usize {
        self.which
    }

    /// Its character's movement, where it has one (dead, as it last was).
    pub fn movement(&self) -> Option<&trp_sim::Movement> {
        self.puppet.as_ref().map(|puppet| &puppet.movement)
    }

    /// What its voice is to be told since last asked.
    pub fn take_heard(&mut self) -> Vec<Heard> {
        std::mem::take(&mut self.heard)
    }

    /// The thing its body's slots leave out of their views (`PlayerBody.
    /// _itemInHands`): what its hands hold standing; dead, what they held
    /// as it died, even once that thing has left its slot (the corpse keeps
    /// the living body's binding, `Player.CreateCorpse`, `reinitBody:
    /// false`; put back, it is drawn nowhere).
    pub fn in_hand(&self) -> Option<trp_sim::items::inventory::Id> {
        self.fall.as_ref().map_or(self.holds, |fall| fall.in_hands).map(|(_, id)| id)
    }

    /// Where a bone of its skeleton is (Bevy's axes) by what goes by the
    /// simulation, not by what is drawn (its hit colliders, its voice's
    /// place): standing, as its character's newest frame put it
    /// (`placement`, made if it was not); fallen, or before its character's
    /// first frame, as its skeleton stands (its ragdoll's bones).
    pub fn bone_now(&self, rig: &DummyRig, bone: usize, helper: &TransformHelper) -> Option<Affine3A> {
        match self.newest_standing().and_then(|frame| self.placement(frame)) {
            Some(placement) => Some(in_world(&self.parents, &placement.root, &placement.locals, bone)),
            None => Some(helper.compute_global_transform(*rig.bones.get(bone)?).ok()?.affine()),
        }
    }

    /// Its character's newest frame while it stands (not fallen), none
    /// before its first.
    pub(super) fn newest_standing(&self) -> Option<u64> {
        self.fall.is_none().then(|| self.puppet.as_ref()?.newest_frame()).flatten()
    }

    /// Its hit colliders on its character's frame `frame` (`Placement`):
    /// that frame's pose (`Puppet::pose_of_frame`, made if it was not) and
    /// the colliders placed on it, kept with the frame, so that a frame's
    /// are made once however many readers ask; none where the frame is no
    /// longer kept.
    pub(super) fn placement(&self, frame: u64) -> Option<Arc<Placement>> {
        let puppet = self.puppet.as_ref()?;
        let mut kept = self.placements.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(found) = kept.iter().find(|placement| placement.frame == frame) {
            return Some(found.clone());
        }
        let pose = puppet.pose_of_frame(frame)?;
        let Posed { root, locals, .. } = posed(self, pose, puppet.firearm().is_some(), true);
        let (placed_now, objects) = self.placed_by(&root, &locals);
        let made = Arc::new(Placement { frame, root, locals, placed: placed_now, objects });
        kept.push(made.clone());
        if kept.len() > 3 {
            kept.remove(0);
        }
        PLACEMENTS_MADE.fetch_add(1, Ordering::Relaxed);
        Some(made)
    }

    /// Its hit colliders, and their objects' places (the game's axes), with
    /// its root at `root` and each bone at `locals` in its parent, put
    /// together as the engine puts the bones' own entities together
    /// (`in_world`).
    pub(super) fn placed_by(&self, root: &Transform, locals: &[Transform]) -> (Vec<Placed>, Vec<[f32; 3]>) {
        let bones: HashMap<usize, Affine3A> = self.colliders.iter().map(|c| (c.bone, in_world(&self.parents, root, locals, c.bone))).collect();
        let placed_now = self.colliders.iter().map(|c| placed(c, bones.get(&c.bone).copied().unwrap_or(Affine3A::IDENTITY))).collect();
        let objects = self
            .colliders
            .iter()
            .map(|c| {
                let at = (bones.get(&c.bone).copied().unwrap_or(Affine3A::IDENTITY) * c.local).translation;
                [at.x, at.y, -at.z]
            })
            .collect();
        (placed_now, objects)
    }

    /// Its hit colliders as a reader of `frame` meets them: its character's
    /// frame's while it stands (`placement`), else as `place_dummies` laid
    /// them on its skeleton.
    pub(super) fn colliders_on(&self, frame: Option<u64>) -> Colliders<'_> {
        match frame.and_then(|frame| self.placement(frame)) {
            Some(placement) => Colliders::Posed(placement),
            None => Colliders::Laid(&self.placed, &self.objects),
        }
    }

    /// Whether its hit colliders on its character's frame `frame` may lie
    /// within `reach` of the segment from `from` to `to` (the game's axes):
    /// the frame's bound (`Puppet::bound`, a capsule that holds every one
    /// in any pose) widened by `reach`, had without a pose. True where the
    /// frame's recipe is not kept (the colliders are then asked for).
    pub(super) fn bound_meets(&self, frame: u64, from: V3, to: V3, reach: f32) -> bool {
        self.puppet.as_ref().and_then(|puppet| puppet.recipe_of_frame(frame)).is_none_or(|recipe| {
            let bound = trp_sim::characters::puppet::Bound { radius: recipe.bound.radius + reach, ..recipe.bound };
            bound.meets(from, to)
        })
    }

    /// Its kept placements let go (a death, a character made anew: its
    /// frames' numbers start again).
    pub(super) fn forget_placements(&mut self) {
        self.placements.get_mut().unwrap_or_else(std::sync::PoisonError::into_inner).clear();
        self.colliders_frame = None;
        self.before_frame = None;
        self.held_frame = None;
    }

    /// Whether its AKM's model is shown: in its hands while they hold it,
    /// on the strap dead until its thing leaves its slot.
    pub(super) fn weapon_shown(&self) -> bool {
        self.fall.as_ref().map_or(self.holds.is_some(), |fall| fall.hung)
    }

    /// Its health's events taken: a heavy bleeding's start and end and its
    /// death for the drops on the ground, and each kept for its voice while
    /// one listens.
    pub(super) fn drain(&mut self, listened: bool, drops: &mut Vec<(u32, bool)>) {
        let character = self.character();
        let events = self.health.take_events();
        drops.extend(events.iter().filter_map(trp_sim::combat::impacts::bleeding_change).map(|on| (character, on)));
        if listened {
            self.heard.extend(events.into_iter().map(Heard::Health));
        }
    }
}

/// Every dummy, by its number.
#[derive(Resource)]
pub struct Dummies {
    pub list: Vec<Dummy>,
    /// Each heavy bleeding's start (true) and end or death (false) since the
    /// drops on the ground last asked, by character (`decal_view`); a
    /// dummy made whole again is taken out.
    pub bleeding_drops: Vec<(u32, bool)>,
    /// The player's hits on living dummies and the dummies killed since the
    /// experience last read them (`Player.ManageAggressor`,
    /// `OnEnemyDamage`, `OnEnemyKill`; `crate::experience`). Every bullet on
    /// the range is the player's: the dummies' own weapons never fire.
    pub experience: Vec<DummyHit>,
    /// The face shields the player's hits and blasts marked on each dummy
    /// (its number), in the order marked, for the player to make
    /// (`shield_marks`: found alive, within its limit).
    pub marks: Vec<(usize, trp_sim::condition::armor::Mark)>,
    /// Whether their voices listen (`dummy_voice`): what happens to each is
    /// kept for its voice only then.
    pub listened: bool,
    /// TRP's own posing economy (the field `posing_economy` of `TARGETS`,
    /// on; third-person spec, "Posing by distance and off screen: the
    /// game's, and TRP's own"): each drawn by its size on screen, its
    /// colliders placed only where a bullet, a blade or a blast may meet
    /// them (their bound first). Off, every one posed in full and drawn
    /// between frames every frame, and its colliders placed every frame.
    /// Every result the same either way.
    pub economy: bool,
    /// What the posing cost, for its log line (`run_dummies`).
    pub(super) posing_log: PosingLog,
}

/// The dummies' posing as the log says it, now and then: the frames
/// counted, the time their steps and their drawing took, how many were
/// drawn each way, the colliders placed.
#[derive(Default)]
pub(super) struct PosingLog {
    pub(super) frames: u32,
    pub(super) stepped: f64,
    pub(super) drawing: f64,
    pub(super) placing: f64,
    pub(super) every_frame: u32,
    pub(super) held: u32,
    pub(super) not: u32,
    pub(super) placed_before: u32,
    pub(super) since: f32,
}

/// A hit on a living dummy, or its death by a blast, as the player's
/// statistics take it: whether it killed, on the head; a bullet's damage
/// through the armour (`DamageInfo.Damage`, as the body took it:
/// `PROVISIONAL`, the game's figure after the armour not read further) and
/// whether it was heavy (`isHeavyDamage`: at least 0.6 of the part's most,
/// the part bleeding); the dummy's whole health (`EBodyPart.Common`'s
/// most); whether a weapon in hand made it, a bullet or a blow (a blast's
/// is a thrown weapon's: no hit, a kill alone), and whether a melee
/// weapon's blow did (its class's skill is Melee, `ExecuteShotSkill` by
/// the knife's item).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DummyHit {
    pub killed: bool,
    pub head: bool,
    pub damage: f32,
    pub heavy: bool,
    pub whole: f32,
    pub bullet: bool,
    pub melee: bool,
}

/// Each number of a thing and of all in it against its copy's, the copy of
/// the same shape (`Item::renumbered`: slot by slot, cell by cell, the loose
/// in order).
pub(crate) fn pair_numbers(old: &trp_sim::items::inventory::Item, new: &trp_sim::items::inventory::Item, map: &mut HashMap<trp_sim::items::inventory::Id, trp_sim::items::inventory::Id>) {
    map.insert(old.id, new.id);
    for (a, b) in old.slots.iter().zip(&new.slots) {
        if let (Some(a), Some(b)) = (a, b) {
            pair_numbers(a, b, map);
        }
    }
    for (a, b) in old.grids.iter().flatten().zip(new.grids.iter().flatten()) {
        pair_numbers(&a.1, &b.1, map);
    }
    for (a, b) in old.loose.iter().zip(&new.loose) {
        pair_numbers(a, b, map);
    }
}

/// The dummies' bodies given new numbers as the run begins again (`death::
/// begin_again`; inventory spec, "Searching a body"), and each dummy told
/// its things' new numbers (`Dummy::renumber`), in the one step, so that no
/// frame sees a body by numbers its dummy no longer knows.
pub(crate) fn renumber_bodies(carried: &mut crate::screens::inventory_view::Carried, mut dummies: Option<&mut Dummies>) {
    for (which, gear) in carried.bodies.iter_mut().enumerate() {
        let Some(gear) = gear else { continue };
        let renewed = gear.renumbered();
        if let Some(dummies) = dummies.as_deref_mut() {
            let mut map = HashMap::new();
            pair_numbers(gear, &renewed, &mut map);
            if let Some(dummy) = dummies.list.iter_mut().find(|dummy| dummy.which == which) {
                dummy.renumber(&map);
            }
        }
        *gear = renewed;
    }
}

impl Dummy {
    /// What it holds, held as it died and wears, by its things' new numbers
    /// (a number not in the map is a thing no longer on the body, and is
    /// left as it was). Until 0.248.0 they kept the old numbers: after a
    /// respawn its rifle was taken for one not in its hands (hung on its
    /// back while it stood, left in the air as it died).
    fn renumber(&mut self, map: &HashMap<trp_sim::items::inventory::Id, trp_sim::items::inventory::Id>) {
        let renew = |id: &mut trp_sim::items::inventory::Id| {
            if let Some(new) = map.get(id) {
                *id = *new;
            }
        };
        if let Some(own) = self.own.as_mut() {
            renew(own);
        }
        if let Some((_, held)) = self.holds.as_mut() {
            renew(held);
        }
        if let Some((_, held)) = self.fall.as_mut().and_then(|fall| fall.in_hands.as_mut()) {
            renew(held);
        }
        for piece in &mut self.wearer.pieces {
            if let Some(item) = piece.item.as_mut() {
                renew(item);
            }
        }
    }
}
