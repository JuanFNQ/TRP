//! Making the dummies and keeping their worn things and armour.

use super::*;

/// The dummies made once their skeletons are (`body_view::spawn_body`):
/// each its colliders, its character, the AKM's model in its hands, and its
/// own kit, its body's gear from the start, hidden from the player as a
/// body's gear is (`ActiveSearchController`; inventory spec, "Searching a
/// body"), its armour of it.
#[allow(clippy::too_many_arguments)]
pub fn make_dummies(
    mut commands: Commands,
    source: Option<Res<DummySource>>,
    rigs: Option<Res<DummyRigs>>,
    dummies: Option<Res<Dummies>>,
    parts: Res<crate::player::body_view::BodyParts>,
    (mut carried, features): (ResMut<crate::screens::inventory_view::Carried>, Res<crate::screens::options_view::Features>),
    mut game: ResMut<crate::GameData>,
    mut converted: ResMut<crate::assets::render_data::Converted>,
    (mut meshes, mut images, mut materials, mut bindposes): (ResMut<Assets<Mesh>>, ResMut<Assets<Image>>, ResMut<Assets<StandardMaterial>>, ResMut<Assets<SkinnedMeshInverseBindposes>>),
) {
    let (Some(source), Some(rigs), None) = (source, rigs, dummies) else { return };
    let seed = crate::fixed_seed(0xd0d0).unwrap_or_else(|| std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |d| d.subsec_nanos() as u64));
    let mut list = Vec::new();
    for (which, rig) in rigs.0.iter().enumerate() {
        let colliders = colliders_of(&source, rig, which);
        let kit = source.kits.get(which);
        let wearer = match kit {
            Some(kit) => wearer_of(&game.db, source.rules.clone(), kit, VestSkills::of(&source.skills)),
            None => Wearer::dress(&game.db, source.rules.clone(), &[], VestSkills::of(&source.skills)),
        };
        let mut health = Health::new(source.health.clone(), seed + which as u64);
        health.skills = health_skills(&source.skills);
        let load = source.maker.as_ref().map(|maker| maker.load_of(kit, &source.skills)).unwrap_or_default();
        // The skeleton's bones the body's animator gives, and its arms.
        let drawn: Vec<Option<usize>> = parts.bones.iter().map(|path| rig.paths.iter().position(|p| p == path).or_else(|| rig.bone(path.rsplit('/').next().unwrap_or(path)))).collect();
        let arm_bones = ['R', 'L'].map(|side| ["Collarbone", "Upperarm", "Forearm1", "Forearm2", "Forearm3", "Palm"].map(|name| rig.bone(&format!("Base Human{side}{name}"))));
        // Its own AKM, in its rifle's slot, which its hands hold.
        let own = kit.and_then(|kit| in_slot(kit, kit.slot_index(IN_HAND_SLOT)?));
        let holds = source.maker.as_ref().and(held_of(kit, own));
        // Its character, and the AKM's model in its hands, hung as the
        // player's rig is (`weapon::spawn_weapon`): from the feet turned
        // with the view, at the ribcage turned by the view's pitch.
        let puppet = source.maker.as_ref().and_then(|maker| match maker.make(&game, places()[which].at, kit, &source.skills, holds.is_some()) {
            Ok(puppet) => Some(puppet),
            Err(e) => {
                warn!("dummy {which}'s character could not be made: {e:#}");
                None
            }
        });
        let holds = holds.filter(|_| puppet.is_some());
        let model = match (&puppet, source.maker.as_ref()) {
            (Some(_), Some(maker)) => {
                let shown = if holds.is_some() { Visibility::Inherited } else { Visibility::Hidden };
                let player = commands.spawn((Name::new(format!("Dummy {which}'s hands")), Transform::from_translation(places()[which].at), shown)).id();
                let pivot = commands.spawn((Name::new(format!("Dummy {which}'s view")), Transform::default(), Visibility::default())).id();
                commands.entity(player).add_child(pivot);
                let hierarchy = &maker.weapon.hierarchy;
                let tree = crate::weapons::weapon::spawn_tree(&mut commands, &mut game.unity, &mut converted, &mut meshes, &mut images, &mut materials, &mut bindposes, hierarchy, pivot, None, None);
                let weapon_root = tree.by_path.get("Weapon_root").copied();
                Some(Model { player, pivot, tree: tree.entities, weapon_root, grip: Grip::of(hierarchy, rig) })
            }
            _ => None,
        };
        let empty_grip = source.maker.as_ref().and_then(|maker| maker.empty.as_ref()).map(|empty| Grip::of(&empty.rig.hierarchy, rig));
        info!(
            "dummy {which}: {} hit colliders, {} of the body animator's {} bones on its skeleton, {} fingers, {}; its own kit, {}: {}; plates on {:#x}; {}",
            colliders.len(),
            drawn.iter().flatten().count(),
            drawn.len(),
            model.as_ref().map_or(0, |model| model.grip.fingers.len()),
            match (&puppet, holds) {
                (Some(_), Some(_)) => "an AKM in its hands",
                (Some(_), None) => "its hands empty",
                (None, _) => "standing as the screens stand a character",
            },
            kit.map_or("none".to_owned(), |kit| trp_sim::items::inventory::WORN.iter().filter_map(|slot| kit.in_slot(slot)).map(|item| item.kind.name.clone()).collect::<Vec<_>>().join(", ")),
            if wearer.pieces.is_empty() {
                "no armour".to_owned()
            } else {
                wearer.pieces.iter().map(|p| format!("{} (class {}, {:.0})", game.db.items.get(&p.template).map_or(p.template.as_str(), |t| t.name.as_str()), p.class, p.durability)).collect::<Vec<_>>().join(", ")
            },
            wearer.plates_on(),
            puppet.as_ref().map_or(String::new(), |puppet| load_said(puppet, &load, &game.db, &source.skills))
        );
        if let Some(kit) = kit {
            if carried.bodies.len() <= which {
                carried.bodies.resize(which + 1, None);
            }
            if features.searching {
                carried.searcher.hide_body(kit);
            }
            carried.bodies[which] = Some(kit.clone());
        }
        list.push(Dummy {
            which,
            colliders,
            placed: Vec::new(),
            objects: Vec::new(),
            wearer,
            worn: false,
            health,
            followed: false,
            load,
            dressed: true,
            taken: Vec::new(),
            killing: None,
            fall: None,
            hits: 0,
            puppet,
            heading: 1.0,
            model,
            empty_grip,
            own,
            holds,
            drawn,
            arm_bones,
            locals: rig.rest.clone(),
            parents: rig.parents.clone().into(),
            colliders_frame: None,
            before_frame: None,
            placements: Mutex::new(Vec::new()),
            drawn_by: PosingDrawn::EveryFrame,
            held_frame: None,
            heard: Vec::new(),
        });
    }
    info!("{} dummies made; the posing economy {}", list.len(), if features.posing_economy { "on" } else { "off: every one posed in full" });
    commands.insert_resource(Dummies { list, bleeding_drops: Vec::new(), experience: Vec::new(), marks: Vec::new(), listened: false, economy: features.posing_economy, posing_log: PosingLog::default() });
}

/// A dummy's load and skills for the run's log.
pub(super) fn load_said(puppet: &Puppet, (weight, worn): &(f32, Vec<Worn>), db: &trp_data::Database, skills: &SkillLevels) -> String {
    let s = &puppet.movement.settings;
    let (penalty, ergonomics) = gear_of(db, worn, skills);
    format!(
        "carrying {weight:.2} kg as a bot (inertia {:.4}, overweight {:.1}), its gear's speed {penalty:+.2}% (of its top {:.4}) and ergonomics {:+.1}%, skills at level {}",
        s.inertia,
        s.overweight,
        s.max_speed,
        ergonomics * 100.0,
        skills.level(Skill::Strength)
    )
}

/// What a character wears, as a tree of things for the armour: each with
/// its number, which a mark goes to, and whether its hinge is up (its
/// switch off: a raised face shield is no armour, `CompositeArmorComponent.
/// ShotMatches`).
pub(crate) fn worn_of(item: &trp_sim::items::inventory::Item) -> Vec<Worn> {
    item.slots
        .iter()
        .zip(&item.kind.slots)
        .filter_map(|(content, slot)| {
            let content = content.as_ref()?;
            Some(Worn { template: content.kind.template.clone(), slot: slot.name.clone(), children: worn_of(content), item: Some(content.id), up: content.kind.hinge && !content.on })
        })
        .collect()
}

/// A character's armour as its things stand (body hits spec, "The armour
/// worn"; the dummy's, "Its armour's wear kept on its things"): dressed
/// from what `gear` has on, each piece at its thing's wear (`ArmorComponent`
/// reads its item's `RepairableComponent`: the durability and the greatest
/// durability now) and its repair buff.
pub(crate) fn wearer_of(db: &trp_data::Database, rules: Arc<ArmorRules>, gear: &trp_sim::items::inventory::Item, skills: VestSkills) -> Wearer {
    let mut wearer = Wearer::dress(db, rules, &worn_of(gear), skills);
    for piece in &mut wearer.pieces {
        if let Some(item) = piece.item.and_then(|id| gear.find(id)) {
            [piece.durability, piece.max_durability] = item.durability();
            piece.buff = item.buff;
        }
    }
    wearer
}

/// Whether a piece's wear or repair buff differs from its thing's in
/// `gear` (`keep_wear` would change it).
pub(crate) fn wear_differs(wearer: &Wearer, gear: &trp_sim::items::inventory::Item) -> bool {
    wearer.pieces.iter().any(|piece| {
        piece.item.and_then(|id| gear.find(id)).is_some_and(|item| item.repairable.is_some_and(|wear| wear.durability != piece.durability) || item.buff != piece.buff)
    })
}

/// Each piece's wear and repair buff written back onto its thing in `gear`
/// (a hit's or a blast's; the game's armour is its item's own, so its wear
/// stays on the thing wherever it goes). Its greatest durability is the
/// thing's (a repair's, not a hit's).
pub(crate) fn keep_wear(wearer: &Wearer, gear: &mut trp_sim::items::inventory::Item) {
    for piece in &wearer.pieces {
        let Some(item) = piece.item.and_then(|id| gear.find_mut(id)) else { continue };
        if let Some(wear) = item.repairable.as_mut() {
            wear.durability = piece.durability;
        }
        item.buff = piece.buff;
    }
}

/// HEAL AND STAND UP's armour (TRP's default: it still renews the armour):
/// every piece `gear` has on made new,
/// its thing's durability and greatest durability its template's again and
/// its repair buff gone, and the armour dressed from it. A face shield's
/// marks stay (its glass's, not its wear).
pub(crate) fn renew_armour(db: &trp_data::Database, rules: Arc<ArmorRules>, gear: &mut trp_sim::items::inventory::Item, skills: VestSkills) -> Wearer {
    let wearer = Wearer::dress(db, rules, &worn_of(gear), skills);
    for piece in &wearer.pieces {
        let Some(item) = piece.item.and_then(|id| gear.find_mut(id)) else { continue };
        if let Some(wear) = item.repairable.as_mut() {
            wear.durability = piece.durability;
            wear.max_durability = piece.max_durability;
        }
        item.buff = None;
    }
    wearer
}

/// The slot its weapon in hand is given in (`main::dummy_kit`: the AKM in
/// its rifle's slot), which its hands hold.
pub const IN_HAND_SLOT: &str = "FirstPrimaryWeapon";

/// The thing in a body's slot, by the slot's place: its number, none where
/// the slot holds nothing.
pub(super) fn in_slot(gear: &trp_sim::items::inventory::Item, slot: usize) -> Option<trp_sim::items::inventory::Id> {
    gear.slots.get(slot)?.as_ref().map(|item| item.id)
}

/// Whether the thing in hand at the death is still in its slot on the body
/// (`Corpse.RemoveLootItem`: any move of it off its place, by a search or on
/// the body itself, takes the weapon away; a part taken off it does not).
pub(super) fn still_in_hands(gear: Option<&trp_sim::items::inventory::Item>, (slot, id): (usize, trp_sim::items::inventory::Id)) -> bool {
    gear.and_then(|gear| in_slot(gear, slot)) == Some(id)
}
