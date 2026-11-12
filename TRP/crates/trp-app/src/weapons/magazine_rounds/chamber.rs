//! The chamber's kinds kept, its point found, the chambered round shown and rounds spawned.

use super::*;

/// The kinds of the rounds in the weapon in hand's chambers of its own (a
/// cylinder's, barrels'), and of the cases there, by the chambers' numbers:
/// the logic's chambers hold neither kind, which the item keeps
/// (`inventory_view::settle`). A chamber keeps its round's kind after the
/// shot, and its case takes it as the round goes to the port
/// (`FirearmEvent::CaseLeft`). With them, the rounds in reach in the order
/// a reload takes them, as they were when the reload began (the game's
/// `AmmoPack`).
#[derive(Resource, Default)]
pub struct ChamberKinds {
    pub(super) weapon: Option<(Id, usize)>,
    kinds: Vec<Option<String>>,
    cases: Vec<Option<String>>,
    pack: Vec<String>,
    /// The stack the pack was taken from, where the magazine picker chose
    /// one (`HandsHeld::pack`).
    pack_of: Option<Id>,
}

impl ChamberKinds {
    /// The kind of a chamber's round, or of the round last fired from it.
    pub fn kind(&self, chamber: usize) -> Option<&str> {
        self.kinds.get(chamber).and_then(Option::as_deref)
    }

    /// The kind of a chamber's case, where one is known: the item's, or
    /// the round's that left it.
    pub fn case(&self, chamber: usize) -> Option<&str> {
        self.cases.get(chamber).and_then(Option::as_deref)
    }

    /// The rounds and cases the item holds, by the chambers' numbers, taken
    /// as the logic takes them (the weapon newly in hand, its rounds moved
    /// by the inventory): a chamber with a round has its kind, one without
    /// keeps the kind it had; the cases are the item's.
    pub fn take_item(&mut self, weapon: Id, rounds: &[Option<String>], cases: &[Option<String>]) {
        if self.weapon != Some((weapon, rounds.len())) {
            (self.weapon, self.kinds) = (Some((weapon, rounds.len())), vec![None; rounds.len()]);
        }
        for (kept, round) in self.kinds.iter_mut().zip(rounds) {
            if round.is_some() {
                kept.clone_from(round);
            }
        }
        self.cases = cases.to_vec();
    }
}

/// The round the item holds in a chamber of its own, where it holds one: a
/// barrel's (`patron_in_weapon_000` on), a cylinder's (`camora_000` on).
fn round_in_item(weapon: &Item, chamber: usize) -> Option<String> {
    let in_slots = |holder: &Item, named: &str| {
        let mut rounds = holder.slots.iter().zip(&holder.kind.slots).filter(|(_, slot)| slot.name.starts_with(named));
        rounds.nth(chamber).and_then(|(round, _)| round.as_ref()).map(|round| round.kind.template.clone())
    };
    in_slots(weapon, "patron_in_weapon").or_else(|| weapon.in_slot(trp_sim::items::inventory::MAGAZINE_SLOT).and_then(|cylinder| in_slots(cylinder, "camora_")))
}

/// The kinds kept beside the logic's chambers, after its step: made afresh
/// for a weapon newly in hand (the item's round in each chamber, else the
/// magazine's top one, else the weapon's own kind: `cases::fired_round`;
/// the item's cases); the rounds in reach read again while no reload is in
/// hand; a round a reload shows (`FirearmEvent::RoundShown`), the one it
/// takes in its turn; a round gone to its port as its case
/// (`FirearmEvent::CaseLeft`), the case's kind from now.
pub fn keep_chamber_kinds(mut kinds: ResMut<ChamberKinds>, mut controller: ResMut<Controller>, hands: Option<Res<HandsRig>>, carried: Res<Carried>, held: Res<HandsHeld>, game: Res<crate::GameData>) {
    let shown = std::mem::take(&mut controller.rounds_shown);
    let left = std::mem::take(&mut controller.cases_left);
    let firearm = hands.as_deref().and_then(|hands| hands.firearm.as_ref()).filter(|firearm| !firearm.single_chamber());
    let (Some(firearm), Some(weapon)) = (firearm, held.item) else {
        if kinds.weapon.is_some() {
            *kinds = ChamberKinds::default();
        }
        return;
    };
    let count = firearm.chambers_shown().len();
    if kinds.weapon != Some((weapon, count)) {
        let own = crate::weapons::cases::fired_round(&carried, &held, &game.db);
        let item = carried.find(weapon);
        kinds.kinds = (0..count).map(|chamber| item.and_then(|item| round_in_item(item, chamber)).or_else(|| own.clone())).collect();
        kinds.cases = item.map(trp_sim::items::inventory::chamber_cases).unwrap_or_default().into_iter().map(|case| case.map(|kind| kind.template.clone())).collect();
        kinds.weapon = Some((weapon, count));
    }
    for chamber in left {
        let round = kinds.kind(chamber).map(str::to_owned).or_else(|| crate::weapons::cases::fired_round(&carried, &held, &game.db));
        if kinds.cases.len() <= chamber {
            kinds.cases.resize(chamber + 1, None);
        }
        kinds.cases[chamber] = round;
    }
    if !firearm.reloading() || kinds.pack_of != held.pack {
        kinds.pack_of = held.pack;
        let pack: Vec<String> = trp_sim::items::inventory::rounds_in_reach_kinds(&carried.equipment.item, weapon, count, held.pack).iter().map(|kind| kind.template.clone()).collect();
        if kinds.pack != pack {
            kinds.pack = pack;
        }
    }
    for (chamber, nth) in shown {
        let kind = kinds.pack.get(nth).cloned().or_else(|| crate::weapons::cases::fired_round(&carried, &held, &game.db));
        if let Some(kept) = kinds.kinds.get_mut(chamber) {
            *kept = kind;
        }
    }
}

/// A chamber's point in the weapon in hand (`patron_in_weapon` under its
/// weapon bone: `Firearms.SetupPatronInWeaponTransform`, every one whose
/// name has it, breadth first, numbered so; a weapon of one chamber uses
/// the first, which its clips move with the bolt): the rig it is of, its
/// number, the round drawn there with its template, and whether it shows.
#[derive(Component)]
pub struct ChamberPoint {
    pub(super) rig: Entity,
    index: usize,
    pub(super) round: Option<(Entity, String)>,
    shown: Option<bool>,
}

/// The weapon in hand's chamber points found, once a rig. What the
/// weapon's own model holds under a point, its first object, is taken away
/// as the point is found (`Firearms`, as the hands are made: `ReturnToPool`
/// of each point's first child): the RShG-2's model has its rocket there,
/// which would otherwise be drawn loaded or not. The chamber's round is
/// put there by `show_chambered`.
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
    let mut index = 0;
    while let Some(entity) = queue.pop_front() {
        if names.get(entity).is_ok_and(|name| name.as_str().contains("patron_in_weapon")) {
            if !marked.contains(entity) {
                commands.entity(entity).insert(ChamberPoint { rig: from, index, round: None, shown: None });
                if let Some(under) = children.get(entity).ok().filter(|under| !under.is_empty()) {
                    commands.entity(under[0]).despawn();
                    queue.extend(under.iter().skip(1));
                    index += 1;
                    continue;
                }
            }
            index += 1;
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
/// action opens, in a look into the chamber. A cylinder's or barrels'
/// rounds likewise at each chamber's point while the model holds one there
/// (`Firearm::chambers_shown`), of the kind kept for it (`ChamberKinds`).
/// An underbarrel launcher's round at its own chamber's point while its
/// model holds one (`Firearm::launcher_models`; underbarrel-launcher spec,
/// "Reloading and unloading": `UnderbarrelContainer.SetRoundIntoWeapon`),
/// its model the round its item holds there (else the one drawn before).
#[allow(clippy::too_many_arguments)]
pub fn show_chambered(
    mut commands: Commands,
    mut points: Query<(Entity, &mut ChamberPoint)>,
    carried: Res<Carried>,
    held: Res<HandsHeld>,
    (hands, rig, kinds): (Option<Res<HandsRig>>, Option<Res<WeaponRig>>, Res<ChamberKinds>),
    game: Res<crate::GameData>,
    mut loose: ResMut<Loose>,
) {
    let firearm = hands.as_deref().and_then(|hands| hands.firearm.as_ref());
    let current = rig.as_deref().and_then(|rig| rig.weapon_root);
    let launcher_chamber = rig.as_deref().and_then(|rig| rig.launcher_chamber);
    for (entity, mut point) in &mut points {
        // Another rig's points are left as they are, under its hidden root.
        if Some(point.rig) != current {
            continue;
        }
        let index = point.index;
        let wanted = firearm.and_then(|firearm| {
            if Some(entity) == launcher_chamber {
                firearm.launcher_models().filter(|(round, _)| *round)?;
                launcher_round(&carried, &held).or_else(|| point.round.as_ref().map(|(_, kind)| kind.clone()))
            } else if firearm.single_chamber() {
                (index == 0 && firearm.round_drawn_in_chamber()).then(|| crate::weapons::cases::fired_round(&carried, &held, &game.db)).flatten()
            } else {
                firearm.chambers_shown().get(index).filter(|shown| shown.round)?;
                kinds.kind(index).map(str::to_owned).or_else(|| crate::weapons::cases::fired_round(&carried, &held, &game.db))
            }
        });
        // A round of another kind: made afresh.
        if let (Some(want), Some((round, have))) = (&wanted, &point.round) {
            if want != have {
                commands.entity(*round).try_despawn();
                (point.round, point.shown) = (None, None);
            }
        }
        if let (Some(want), None) = (&wanted, &point.round) {
            if let Some(ready) = loose.model_for(&format!("{ROUND_MODEL}{want}")) {
                let round = spawn_round(&mut commands, &ready, false);
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

/// The round in the weapon in hand's underbarrel launcher's chamber, as its
/// item holds it.
pub(crate) fn launcher_round(carried: &Carried, held: &HandsHeld) -> Option<String> {
    let weapon = held.item.and_then(|id| carried.find(id))?;
    trp_sim::items::inventory::launcher_part(weapon).and_then(trp_sim::items::inventory::chambered).map(|round| round.kind.template.clone())
}

/// A round's model under its point: at the point's origin, a quarter turn
/// about x (`FillAmmoObjectsOnStart`), its objects on or off as its model
/// has them (`loose::ROUND_MODEL`).
pub(crate) fn spawn_round(commands: &mut Commands, ready: &Ready, levels: bool) -> Entity {
    spawn_round_on(commands, ready, None, levels)
}

/// The same, its pieces drawn on `layer` where one is given; with `levels`,
/// every level of detail of the round's own group spawned and tagged for
/// the picker (`loose::Ready::spawn_levels`).
pub(crate) fn spawn_round_on(commands: &mut Commands, ready: &Ready, layer: Option<&RenderLayers>, levels: bool) -> Entity {
    let turn = convert::rotation(trp_sim::base::math::quat_euler([90.0, 0.0, 0.0]));
    let root = commands.spawn((Name::new("Round in a magazine"), Transform::from_rotation(turn), Visibility::Inherited)).id();
    let mut entities = Vec::with_capacity(ready.nodes.len());
    for (parent, own, on) in &ready.nodes {
        let entity = commands.spawn((*own, if *on { Visibility::Inherited } else { Visibility::Hidden })).id();
        commands.entity(parent.map_or(root, |p| entities[p])).add_child(entity);
        entities.push(entity);
    }
    let mut pieces = Vec::with_capacity(ready.drawn.len());
    for (k, (node, mesh, material)) in ready.drawn.iter().enumerate() {
        let piece = commands.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(material.clone()), Transform::default())).id();
        if let Some(layer) = layer {
            commands.entity(piece).insert(layer.clone());
        }
        commands.entity(entities[*node]).add_child(piece);
        pieces.push((k, piece));
    }
    if levels {
        ready.spawn_levels(commands, &entities, &pieces, layer);
    }
    root
}
