//! The magazine's points found and its rounds shown.

use super::*;

/// Whose magazine a model's points are: the weapon in hand's, a thing's
/// on the ground (a magazine, or a weapon with its magazine), a weapon's
/// hung on the body (`slung_view`, by its item), or the weapon in the
/// OVERALL tab's own figure's hands (`figure_view`, by its item: the item's
/// own count, as a hung weapon's).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Owner {
    Hands,
    Ground(Id),
    Slung(Id),
    Figure(Id),
}

/// A magazine drawn: its points (breadth first, as the game finds them)
/// with their numbers and whether each is on, the magazine it was filled
/// for (`None` inside: a weapon with none), the rounds still to be made
/// (a point's place and the round's template), the rounds made, and the
/// frames it updates on (`_evenOrNotEvenUpdate`).
#[derive(Component)]
pub struct MagazinePoints {
    owner: Owner,
    pub(super) points: Vec<(Entity, u32)>,
    pub(super) on: Vec<Option<bool>>,
    filled: Option<Option<Id>>,
    waiting: Vec<(usize, String)>,
    pub(super) rounds: Vec<Entity>,
    parity: u32,
    /// The layer its rounds are drawn on, where its model's is not the
    /// world's (a weapon hung on the gear page's figure: `slung_view`).
    pub(super) layer: Option<RenderLayers>,
}

impl MagazinePoints {
    pub fn new(owner: Owner, points: Vec<(Entity, u32)>) -> Self {
        let on = vec![None; points.len()];
        Self { owner, points, on, filled: None, waiting: Vec::new(), rounds: Vec::new(), parity: 0, layer: None }
    }

    /// Its rounds drawn on that layer.
    pub fn on_layer(mut self, layer: RenderLayers) -> Self {
        self.layer = Some(layer);
        self
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
        if let Some(number) = names.get(entity).ok().and_then(|name| trp_sim::items::magazine_view::point_number(name.as_str())) {
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
    let in_slot = |weapon: &'a Item| weapon.slots.iter().zip(&weapon.kind.slots).find(|(_, slot)| slot.name == trp_sim::items::inventory::MAGAZINE_SLOT).and_then(|(part, _)| part.as_ref());
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
            Some(Some((magazine, logic.unwrap_or_else(|| trp_sim::items::inventory::magazine_rounds(magazine)))))
        }
        Owner::Ground(_) | Owner::Slung(_) | Owner::Figure(_) => {
            let item = match owner {
                Owner::Ground(id) => &carried.ground.iter().find(|(item, _)| item.id == id)?.0,
                Owner::Slung(id) => carried.equipment.item.find(id).or_else(|| carried.find_on_bodies(id))?,
                Owner::Figure(id) => carried.equipment.item.find(id)?,
                Owner::Hands => return None,
            };
            if !item.kind.visible_rounds.is_empty() {
                return Some(Some((item, trp_sim::items::inventory::magazine_rounds(item))));
            }
            if !item.kind.slots.iter().any(|slot| slot.name == trp_sim::items::inventory::MAGAZINE_SLOT) {
                return None;
            }
            match in_slot(item) {
                None => Some(None),
                Some(magazine) if magazine.kind.visible_rounds.is_empty() => None,
                Some(magazine) => Some(Some((magazine, trp_sim::items::inventory::magazine_rounds(magazine)))),
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
    mut magazines: Query<&mut MagazinePoints, crate::platform::park::EvenParked>,
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
                let round = round_at(place, number, count).and_then(|from_top| trp_sim::items::inventory::magazine_round_at(magazine?.0, from_top));
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
                        // A round in a thing on the ground by its own levels
                        // of detail (`lod_view`, the world's things'), as the
                        // game's round object made in it keeps its group;
                        // the hands', a body's and a figure's at their
                        // fullest, as before.
                        let levels = matches!(drawn.owner, Owner::Ground(_)) && crate::assets::lod_view::world_on();
                        let round = spawn_round_on(&mut commands, &ready, drawn.layer.as_ref(), levels);
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
