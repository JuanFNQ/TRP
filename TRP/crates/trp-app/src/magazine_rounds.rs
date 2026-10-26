//! A magazine's rounds drawn in its model (inventory spec, "A magazine's
//! rounds drawn"; the options' `MAGAZINE ROUNDS`): its `patron_` points
//! (`trp_sim::magazine_view`) filled as the game fills them when its
//! object is made (`SpringMagazineVisual.FillAmmoObjectsOnStart`), each
//! with its round's own model, and on every other frame after, a point
//! numbered past the count switched off (`UpdateCartridgesActiveState`).
//! For the weapon in hand's magazine (its rig's, filled again when the
//! magazine in the weapon changes, its count the logic's: the game's item
//! loses a round at the shot, the app's at rest) and for a magazine on
//! the ground, or a weapon's there (`loose`). A magazine's picture has its
//! rounds too (`icons::picture_key`).

use bevy::diagnostic::FrameCount;
use bevy::prelude::*;
use trp_sim::inventory::{Id, Item};
use trp_sim::magazine_view::{round_at, stays};

use crate::convert;
use crate::inventory_view::{Carried, HandsHeld};
use crate::loose::{Loose, Ready, ROUND_MODEL};
use crate::weapon::{HandsRig, WeaponRig};

/// Whose magazine a model's points are: the weapon in hand's, or a thing's
/// on the ground (a magazine, or a weapon with its magazine).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Owner {
    Hands,
    Ground(Id),
}

/// A magazine drawn: its points (breadth first, as the game finds them)
/// with their numbers and whether each is on, the magazine it was filled
/// for (`None` inside: a weapon with none), the rounds still to be made
/// (a point's place and the round's template), the rounds made, and the
/// frames it updates on (`_evenOrNotEvenUpdate`).
#[derive(Component)]
pub struct MagazinePoints {
    owner: Owner,
    points: Vec<(Entity, u32)>,
    on: Vec<Option<bool>>,
    filled: Option<Option<Id>>,
    waiting: Vec<(usize, String)>,
    rounds: Vec<Entity>,
    parity: u32,
}

impl MagazinePoints {
    pub fn new(owner: Owner, points: Vec<(Entity, u32)>) -> Self {
        let on = vec![None; points.len()];
        Self { owner, points, on, filled: None, waiting: Vec::new(), rounds: Vec::new(), parity: 0 }
    }

    /// A point switched on or off, where it is not so already.
    fn set(&mut self, commands: &mut Commands, place: usize, on: bool) {
        if self.on[place] != Some(on) {
            self.on[place] = Some(on);
            commands.entity(self.points[place].0).insert(if on { Visibility::Inherited } else { Visibility::Hidden });
        }
    }
}

/// The parity the next magazine made updates on (`_evenOrNotEvenUpdateLastValue`:
/// each takes the other one from the one before).
#[derive(Resource, Default)]
pub struct NextParity(u32);

/// The weapon in hand's magazine's object (the first under its magazine's
/// slot, as `loose` finds it) given its points, once, as it is first seen.
pub fn find_hands_magazine(
    mut commands: Commands,
    weapon: Option<Res<WeaponRig>>,
    names: Query<&Name>,
    children: Query<&Children>,
    marked: Query<(), With<MagazinePoints>>,
    mut searched: Local<bool>,
) {
    let Some(rig) = weapon else { return };
    // Looked for once a rig (one with none, as empty hands, not again).
    if !rig.is_changed() && *searched {
        return;
    }
    *searched = true;
    let Some(from) = rig.weapon_root else { return };
    let mut stack = vec![from];
    let mut magazine = None;
    while let Some(entity) = stack.pop() {
        if names.get(entity).is_ok_and(|name| name.as_str() == "mod_magazine") {
            magazine = children.get(entity).ok().and_then(|below| below.first().copied());
            break;
        }
        stack.extend(children.get(entity).into_iter().flatten());
    }
    let Some(magazine) = magazine.filter(|&entity| !marked.contains(entity)) else { return };
    // Breadth first through its own objects (`FindAllTransformsWithNameContains`).
    let mut points = Vec::new();
    let mut queue = std::collections::VecDeque::from([magazine]);
    while let Some(entity) = queue.pop_front() {
        if let Some(number) = names.get(entity).ok().and_then(|name| trp_sim::magazine_view::point_number(name.as_str())) {
            points.push((entity, number));
        }
        queue.extend(children.get(entity).into_iter().flatten());
    }
    info!("the weapon in hand's magazine: {} points for rounds", points.len());
    commands.entity(magazine).insert(MagazinePoints::new(Owner::Hands, points));
}

/// The magazine a model's points show, and its count: the weapon in hand's
/// (by its logic's count, where it has a chamber of its own), or the thing
/// on the ground's own, or its magazine's. `None` where the thing is
/// neither a magazine whose rounds are drawn nor a weapon, or its magazine
/// is a belt's or a stationary weapon's (another visual: left as it is);
/// `Some(None)` for a weapon with no magazine.
fn magazine_of<'a>(owner: Owner, carried: &'a Carried, held: &HandsHeld, hands: Option<&HandsRig>) -> Option<Option<(&'a Item, u32)>> {
    let in_slot = |weapon: &'a Item| weapon.slots.iter().zip(&weapon.kind.slots).find(|(_, slot)| slot.name == trp_sim::inventory::MAGAZINE_SLOT).and_then(|(part, _)| part.as_ref());
    match owner {
        Owner::Hands => {
            let weapon = held.item.and_then(|id| carried.find(id))?;
            let Some(magazine) = in_slot(weapon) else { return Some(None) };
            if magazine.kind.visible_rounds.is_empty() {
                return None;
            }
            // (Through a reload the item's is the new magazine's own count,
            // as the game's new object is filled from it; the logic's is the
            // old one's until `MagOut`.)
            let logic = hands.and_then(|hands| hands.firearm.as_ref()).filter(|firearm| firearm.single_chamber() && !firearm.reloading()).and_then(|firearm| firearm.magazine);
            Some(Some((magazine, logic.unwrap_or_else(|| trp_sim::inventory::magazine_rounds(magazine)))))
        }
        Owner::Ground(id) => {
            let (item, _) = carried.ground.iter().find(|(item, _)| item.id == id)?;
            if !item.kind.visible_rounds.is_empty() {
                return Some(Some((item, trp_sim::inventory::magazine_rounds(item))));
            }
            if !item.kind.slots.iter().any(|slot| slot.name == trp_sim::inventory::MAGAZINE_SLOT) {
                return None;
            }
            match in_slot(item) {
                None => Some(None),
                Some(magazine) if magazine.kind.visible_rounds.is_empty() => None,
                Some(magazine) => Some(Some((magazine, trp_sim::inventory::magazine_rounds(magazine)))),
            }
        }
    }
}

/// Every magazine drawn: filled when first seen or when its magazine
/// changes, its rounds made as their models come, and its points past the
/// count switched off on its frames.
#[allow(clippy::too_many_arguments)]
pub fn show_rounds(
    mut commands: Commands,
    mut magazines: Query<&mut MagazinePoints>,
    carried: Res<Carried>,
    held: Res<HandsHeld>,
    hands: Option<Res<HandsRig>>,
    mut loose: ResMut<Loose>,
    frames: Res<FrameCount>,
    mut next: ResMut<NextParity>,
) {
    // A reload's old magazine still in the hand keeps its rounds as drawn
    // (the item's is the new one's from the reload's start: the game's new
    // object comes at `MagShow`).
    let old_in_hand = hands.as_deref().and_then(|hands| hands.firearm.as_ref()).is_some_and(|firearm| firearm.old_magazine_in_hand());
    for mut drawn in &mut magazines {
        if drawn.owner == Owner::Hands && old_in_hand {
            continue;
        }
        let Some(magazine) = magazine_of(drawn.owner, &carried, &held, hands.as_deref()) else { continue };
        let (id, count) = magazine.map_or((None, 0), |(item, count)| (Some(item.id), count));
        // Made afresh: filled as the game fills a magazine's object.
        if drawn.filled != Some(id) {
            for round in std::mem::take(&mut drawn.rounds) {
                commands.entity(round).try_despawn();
            }
            drawn.waiting.clear();
            drawn.filled = Some(id);
            drawn.parity = next.0;
            next.0 ^= 1;
            for place in 0..drawn.points.len() {
                let number = drawn.points[place].1;
                let round = round_at(place, number, count).and_then(|from_top| trp_sim::inventory::magazine_round_at(magazine?.0, from_top));
                drawn.set(&mut commands, place, round.is_some());
                if let Some(kind) = round {
                    drawn.waiting.push((place, kind.template.clone()));
                }
            }
        }
        // The rounds whose models have come.
        if !drawn.waiting.is_empty() {
            let waiting = std::mem::take(&mut drawn.waiting);
            for (place, template) in waiting {
                match loose.model_for(&format!("{ROUND_MODEL}{template}")) {
                    Some(ready) => {
                        let round = spawn_round(&mut commands, &ready);
                        commands.entity(drawn.points[place].0).add_child(round);
                        drawn.rounds.push(round);
                    }
                    None => drawn.waiting.push((place, template)),
                }
            }
        }
        // On its frames, a point numbered past the count goes off.
        if frames.0 % 2 == drawn.parity {
            for place in 0..drawn.points.len() {
                if drawn.on[place] == Some(true) && !stays(drawn.points[place].1, count) {
                    drawn.set(&mut commands, place, false);
                }
            }
        }
    }
}

/// The weapon in hand's chamber (`patron_in_weapon` under its weapon bone,
/// which the clips move with the bolt: `Firearms.
/// SetupPatronInWeaponTransform`, the first whose name has it, breadth
/// first): the round drawn there, with its template, and whether it shows.
#[derive(Component, Default)]
pub struct ChamberPoint {
    round: Option<(Entity, String)>,
    shown: Option<bool>,
}

/// The weapon in hand's chamber point found, once a rig.
pub fn find_hands_chamber(
    mut commands: Commands,
    weapon: Option<Res<WeaponRig>>,
    names: Query<&Name>,
    children: Query<&Children>,
    marked: Query<(), With<ChamberPoint>>,
    mut searched: Local<bool>,
) {
    let Some(rig) = weapon else { return };
    if !rig.is_changed() && *searched {
        return;
    }
    *searched = true;
    let Some(from) = rig.weapon_root else { return };
    let mut queue = std::collections::VecDeque::from([from]);
    while let Some(entity) = queue.pop_front() {
        if names.get(entity).is_ok_and(|name| name.as_str().contains("patron_in_weapon")) {
            if !marked.contains(entity) {
                commands.entity(entity).insert(ChamberPoint::default());
            }
            return;
        }
        queue.extend(children.get(entity).into_iter().flatten());
    }
}

/// The round in the weapon in hand's chamber (firing spec, "The round in
/// the chamber"): drawn at its point while the logic has one there to be
/// seen (`Firearm::round_drawn_in_chamber`: a weapon of one chamber of its
/// own, not between a shot and the next round fed), the whole cartridge,
/// its model the chambered round's (else its magazine's top one, else the
/// weapon's own kind: `cases::fired_round`), a quarter turn about x as a
/// magazine's (`ParentShellToTransform`). The game never hides it
/// (`SetPatronInWeaponVisible` is called by nothing): it is seen as the
/// action opens, in a look into the chamber.
pub fn show_chambered(
    mut commands: Commands,
    mut points: Query<(Entity, &mut ChamberPoint)>,
    carried: Res<Carried>,
    held: Res<HandsHeld>,
    hands: Option<Res<HandsRig>>,
    game: Res<crate::GameData>,
    mut loose: ResMut<Loose>,
) {
    let firearm = hands.as_deref().and_then(|hands| hands.firearm.as_ref());
    for (entity, mut point) in &mut points {
        let wanted = firearm.filter(|firearm| firearm.round_drawn_in_chamber()).and_then(|_| crate::cases::fired_round(&carried, &held, &game.db));
        // A round of another kind: made afresh.
        if let (Some(want), Some((round, have))) = (&wanted, &point.round) {
            if want != have {
                commands.entity(*round).try_despawn();
                (point.round, point.shown) = (None, None);
            }
        }
        if let (Some(want), None) = (&wanted, &point.round) {
            if let Some(ready) = loose.model_for(&format!("{ROUND_MODEL}{want}")) {
                let round = spawn_round(&mut commands, &ready);
                commands.entity(entity).add_child(round);
                (point.round, point.shown) = (Some((round, want.clone())), Some(true));
            }
        }
        let show = wanted.is_some();
        if let Some(round) = point.round.as_ref().map(|(round, _)| *round) {
            if point.shown != Some(show) {
                point.shown = Some(show);
                commands.entity(round).insert(if show { Visibility::Inherited } else { Visibility::Hidden });
            }
        }
    }
}

/// A round's model under its point: at the point's origin, a quarter turn
/// about x (`FillAmmoObjectsOnStart`), its objects on or off as its model
/// has them (`loose::ROUND_MODEL`).
fn spawn_round(commands: &mut Commands, ready: &Ready) -> Entity {
    let turn = convert::rotation(trp_sim::math::quat_euler([90.0, 0.0, 0.0]));
    let root = commands.spawn((Name::new("Round in a magazine"), Transform::from_rotation(turn), Visibility::Inherited)).id();
    let mut entities = Vec::with_capacity(ready.nodes.len());
    for (parent, own, on) in &ready.nodes {
        let entity = commands.spawn((*own, if *on { Visibility::Inherited } else { Visibility::Hidden })).id();
        commands.entity(parent.map_or(root, |p| entities[p])).add_child(entity);
        entities.push(entity);
    }
    for (node, mesh, material) in &ready.drawn {
        let piece = commands.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(material.clone()), Transform::default())).id();
        commands.entity(entities[*node]).add_child(piece);
    }
    root
}
