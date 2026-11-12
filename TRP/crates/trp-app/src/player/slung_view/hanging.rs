//! Keeping slung things hung, letting go, and hanging the character's and others' things.

use super::*;

/// Where a body's models go: its bones by path, the layer its pieces are
/// drawn on (the figure's own; else the world's), whether its models'
/// heat shimmers only while they are shown (the character's own), and
/// whether a model waits for all its rounds' models before it is hung (a
/// figure drawn once, not again as they come).
pub(super) struct On<'a> {
    pub(super) body: Body,
    pub(super) bone: &'a dyn Fn(&str) -> Option<Entity>,
    layer: Option<RenderLayers>,
    seen_only: bool,
    whole: bool,
}

/// What a body's hanging did this frame: the body, whether a model was
/// hung or taken down, the meshes and materials hung, and the keys taken
/// down. (The keys it wants are `Slung::wants`'.)
pub(super) struct Done {
    body: Body,
    changed: bool,
    meshes: Vec<Handle<Mesh>>,
    materials: Vec<Handle<StandardMaterial>>,
    gone: Vec<String>,
}

/// Keeps a body's slots' models on its bones (`SlotView`'s binding): a
/// thing in a slot and not in hand has its model, made afresh when its
/// build, its bone or its rounds change; one taken in hand or gone leaves.
/// A model not yet read is asked for every frame it is wanted, and hung
/// when it comes.
#[allow(clippy::too_many_arguments)]
fn keep_hung(slung: &mut Slung, on: &On, gear: Option<&Item>, stamp: u32, in_hand: Option<Id>, holster: Option<LegsHolster>, commands: &mut Commands, loose: &mut Loose) -> Done {
    // What its slots hang, worked out when the carried things, the thing
    // in hand, the holster or the rounds' switch changed (`Wants`).
    let mut wants = slung.take_wants(on.body, gear, stamp, in_hand, holster, loose.rounds);
    // The rounds each shows (none without `MAGAZINE ROUNDS`), by its model
    // once read: what it was hung with stays while they are the same.
    for (want, rounds) in wants.wanted.iter().zip(&mut wants.rounds) {
        if rounds.is_none() {
            *rounds = if loose.rounds {
                loose.model_for(&want.key).map(|ready| gear.and_then(|gear| gear.find(want.item)).map(|item| crate::world::loose::shown_rounds(item, &ready)).unwrap_or_default())
            } else {
                Some(Vec::new())
            };
        }
    }
    let none = Vec::new();
    let mut done = Done { body: on.body, changed: false, meshes: Vec::new(), materials: Vec::new(), gone: Vec::new() };
    // What is hung and no longer wanted so goes.
    slung.hung.retain(|hung| {
        if hung.body != on.body {
            return true;
        }
        let keep = wants.wanted.iter().zip(&wants.rounds).any(|(want, rounds)| want.slot == hung.slot && want.item == hung.item && want.key == hung.key && want.bone == hung.bone && *rounds.as_ref().unwrap_or(&none) == hung.rounds);
        if !keep {
            info!("{} taken down from {}{}", hung.template, hung.slot, on.body.said());
            commands.entity(hung.root).despawn();
            done.gone.push(hung.key.clone());
            done.changed = true;
        }
        keep
    });
    for (want, rounds) in wants.wanted.iter().zip(&wants.rounds) {
        // (Asked for every frame it is wanted: kept while it is.)
        let ready = loose.model_for(&want.key);
        if slung.hung.iter().any(|hung| hung.body == on.body && hung.slot == want.slot) {
            continue;
        }
        let rounds = rounds.as_ref().unwrap_or(&none);
        let (Some(ready), Some(bone_entity)) = (ready, (on.bone)(&want.bone)) else { continue };
        // Its rounds' models had first, as the game's model is made whole
        // (each asked for, and kept, every frame it waits): its chambered
        // rounds', and on a figure every one's.
        let round_models: Vec<(bool, Option<std::sync::Arc<crate::world::loose::Ready>>)> =
            rounds.iter().filter(|(_, _, chamber)| *chamber || on.whole).map(|(_, round, chamber)| (*chamber, loose.model_for(&format!("{}{round}", crate::world::loose::ROUND_MODEL)))).collect();
        if round_models.iter().any(|(_, model)| model.is_none()) {
            continue;
        }
        // The model's own `Weapon_root` as the model stands (its first
        // object where it is placed: `CreateLootPrefab`), and the model
        // turned and moved so that it lies on the bone.
        let Some(weapon_root) = ready.names.iter().position(|name| name == "Weapon_root") else {
            warn!("{} in {}{}: its model has no Weapon_root to hang it by", want.template, want.slot, on.body.said());
            continue;
        };
        let mut chain = vec![weapon_root];
        while let Some(parent) = ready.nodes[*chain.last().expect("one")].0 {
            chain.push(parent);
        }
        let in_model = chain.iter().rev().fold(GlobalTransform::IDENTITY, |acc, node| acc.mul_transform(ready.nodes[*node].1));
        let mut placed = Transform::from_matrix(in_model.to_matrix().inverse());
        // A weapon hung by its alternative point (`UseAltMountBone`): the
        // point's own place and turn added on.
        if slung.alternative_mount.contains(&want.template) {
            if let Some(alternative) = ready.names.iter().position(|name| name == "Weapon_root_alt") {
                let own = ready.nodes[alternative].1;
                placed.translation += own.translation;
                placed.rotation *= own.rotation;
            }
        }
        // The character's own hidden until `show` says it is seen.
        let shown = if on.body == Body::Character { Visibility::Hidden } else { Visibility::Inherited };
        let model = commands.spawn((Name::new(format!("Slung {} {}{}", want.slot, want.template, on.body.said())), placed, shown)).id();
        if on.body == Body::Character {
            commands.entity(model).insert(SlungModel);
            // Hidden in first person, it is parked (`platform::park`), but for one that carries
            // a tactical device: a light left on lights the ground from the body there too.
            if ready.devices.is_empty() {
                commands.entity(model).insert(crate::platform::park::Park);
            }
        }
        // A figure's is parked while no picture of its figure is drawn
        // (`figure_view::show_hung`). Hung, it asks for one, so it begins
        // shown.
        if matches!(on.body, Body::Figure | Body::Overall) {
            commands.entity(model).insert((FigureHung(on.body), crate::platform::park::Park));
        }
        commands.entity(bone_entity).add_child(model);
        let mut entities = Vec::with_capacity(ready.nodes.len());
        for (index, (parent, own, on_at_start)) in ready.nodes.iter().enumerate() {
            // (The first object's own place is the hanging's.)
            let own = if index == 0 { Transform::IDENTITY } else { *own };
            let entity = commands.spawn((own, if *on_at_start { Visibility::Inherited } else { Visibility::Hidden })).id();
            commands.entity(parent.map_or(model, |p| entities[p])).add_child(entity);
            entities.push(entity);
        }
        let mut pieces = Vec::with_capacity(ready.drawn.len());
        for (k, (node, mesh, material)) in ready.drawn.iter().enumerate() {
            if crate::world::loose::left_out(ready.lens.get(k).copied().flatten(), false) {
                continue;
            }
            let part = commands.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(material.clone()), Transform::default())).id();
            if let Some(layer) = &on.layer {
                commands.entity(part).insert(layer.clone());
            }
            commands.entity(entities[*node]).add_child(part);
            // Its levels of detail (`lod_view`), with the characters' on (the
            // model carries them with either: `lod_view::any_on`).
            if let Some(lod) = ready.lod_piece(*node, &entities).filter(|_| crate::assets::lod_view::on()) {
                commands.entity(part).insert(lod);
            }
            pieces.push((k, part));
            done.meshes.push(mesh.clone());
            done.materials.push(material.clone());
        }
        // And what it draws at a coarser level only, by the same rule.
        for (node, mesh, material, lens) in &ready.lower {
            let Some(lod) = ready.lod_piece(*node, &entities).filter(|_| crate::assets::lod_view::on() && !crate::world::loose::left_out(*lens, false)) else { continue };
            let part = commands.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(material.clone()), Transform::default(), lod)).id();
            if let Some(layer) = &on.layer {
                commands.entity(part).insert(layer.clone());
            }
            commands.entity(entities[*node]).add_child(part);
            done.meshes.push(mesh.clone());
            done.materials.push(material.clone());
        }
        for (node, mesh, material) in &ready.shadows {
            let part = commands.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(material.clone()), Transform::default(), crate::rendering::shadow_only::ShadowOnly(true))).id();
            if let Some(layer) = &on.layer {
                commands.entity(part).insert(layer.clone());
            }
            commands.entity(entities[*node]).add_child(part);
        }
        // Its heat its item's (`WeaponPrefab.ManualUpdate` runs for every
        // weapon object enabled, its renderers on or not); its glows take
        // their pieces' layer (`heat_view`).
        ready.spawn_glows(commands, model, want.item, &pieces, on.seen_only);
        // Its chambered round on its chamber's object, and its magazine's
        // points for its rounds (`magazine_rounds`), as on the ground.
        let chambered = round_models.iter().filter(|(chamber, _)| *chamber).filter_map(|(_, model)| model.as_ref());
        for ((node, _, _), round_model) in rounds.iter().filter(|(_, _, chamber)| *chamber).zip(chambered) {
            let drawn = crate::weapons::magazine_rounds::spawn_round_on(commands, round_model, on.layer.as_ref(), false);
            commands.entity(entities[*node]).add_child(drawn);
        }
        for (_, round_model) in round_models.iter().filter_map(|(chamber, model)| Some((chamber, model.as_ref()?))) {
            done.meshes.extend(round_model.drawn.iter().map(|(_, mesh, _)| mesh.clone()));
            done.materials.extend(round_model.drawn.iter().map(|(_, _, material)| material.clone()));
        }
        if loose.rounds && !ready.points.is_empty() {
            let points = ready.points.iter().map(|&(node, number)| (entities[node], number)).collect();
            let mut magazine = crate::weapons::magazine_rounds::MagazinePoints::new(crate::weapons::magazine_rounds::Owner::Slung(want.item), points);
            if let Some(layer) = &on.layer {
                magazine = magazine.on_layer(layer.clone());
            }
            commands.entity(model).insert(magazine);
        }
        info!("{} hung in {}{} on {}{}", want.template, want.slot, on.body.said(), want.bone, if rounds.is_empty() { String::new() } else { format!(", {} rounds shown", rounds.len()) });
        done.changed = true;
        slung.hung.push(Hung { body: on.body, template: want.template.clone(), slot: want.slot, root: model, item: want.item, key: want.key.clone(), bone: want.bone.clone(), rounds: rounds.clone(), objects: entities, devices: ready.devices.clone() });
    }
    slung.wants.push(wants);
    done
}

/// Models taken down and wanted by no body now are let go of (the cache
/// keeps them while anything else of the world draws with them): a key
/// still hung on any body, or wanted by one this frame, is kept.
fn let_go(slung: &Slung, done: &[&Done], loose: &mut Loose) {
    for key in done.iter().flat_map(|done| &done.gone) {
        let wanted = |body: Body| slung.wants.iter().any(|wants| wants.body == body && wants.wanted.iter().any(|want| &want.key == key));
        if !slung.hung.iter().any(|hung| &hung.key == key) && !done.iter().any(|done| wanted(done.body)) {
            loose.let_go(key);
        }
    }
}

/// The holster the character's legs give now: a lower suit's chosen on
/// OVERALL, else the legs begun in.
fn character_holster(slung: &Slung, clothing: Option<&crate::player::clothing::Clothing>) -> Option<LegsHolster> {
    clothing.and_then(|clothing| clothing.holster).unwrap_or(slung.holster)
}

/// The character's hung weapons: its own equipment, the thing in its hands
/// left out (`PlayerBody._itemInHands`). The pistol's bone takes the legs'
/// holster's place and turn (`PlayerBones.AdjustPistolHolster`): the legs'
/// the character wears now, a lower suit put on bringing its own
/// (`PlayerBody.Init` with the new legs); on legs without one no pistol is
/// hung. With `MAGAZINE ROUNDS`, a hung weapon's model has its magazine's
/// rounds and its chambered round (`magazine_rounds`, `Owner::Slung`), as
/// the game's whole model of it has them; made afresh when they change.
/// `PROVISIONAL`: the game makes the model afresh on its item's
/// `ChildrenChanged`, which a round put into or taken from the magazine
/// was not read to raise.
#[allow(clippy::too_many_arguments)]
pub fn hang(
    mut commands: Commands,
    mut slung: ResMut<Slung>,
    carried: Res<Carried>,
    held: Res<HandsHeld>,
    parts: Res<BodyParts>,
    rig: Option<Res<BodyRig>>,
    mut loose: ResMut<Loose>,
    mut transforms: Query<&mut Transform>,
    clothing: Option<Res<crate::player::clothing::Clothing>>,
) {
    let Some(rig) = rig else { return };
    let holster = character_holster(&slung, clothing.as_deref());
    if slung.holster_now != Some(holster) {
        if slung.holster_now.is_some() {
            info!(
                "the legs' holster: {}",
                match holster {
                    Some(h) if h.right => "the right leg's",
                    Some(_) => "the left leg's",
                    None => "none, and no pistol is hung",
                }
            );
        }
        slung.holster_now = Some(holster);
    }
    // The holster on its leg's bone, after the body is posed.
    if let Some((path, holster)) = slung.holster_bone(holster) {
        if let Some(mut transform) = rig.bone(&parts, path).and_then(|bone| transforms.get_mut(bone).ok()) {
            *transform = convert::transform(holster.position, holster.rotation, [1.0; 3]);
        }
    }
    let on = On { body: Body::Character, bone: &|path| rig.bone(&parts, path), layer: None, seen_only: true, whole: false };
    let done = keep_hung(&mut slung, &on, Some(&carried.equipment.item), carried.last_changed().get(), held.item, holster, &mut commands, &mut loose);
    let_go(&slung, &[&done], &mut loose);
}

/// The other bodies' hung weapons (`Body`): the figure's by the
/// character's equipment and legs, the weapon in hand left off (the gear
/// page's stays still with its hands down; the OVERALL tab's own, with
/// `OVERALL FIGURE`, leaves off the thing it was made holding at its
/// showing, `figure_view::Overall::in_hand`), drawn afresh when they change and once their meshes are
/// on the graphics card; each dummy's by its own body's gear and legs, its
/// thing in hand left off (`dummy::Dummies::in_hand`), its holster bone
/// placed on every frame after the dummy is posed or falls.
#[allow(clippy::too_many_arguments)]
pub fn hang_others(
    mut commands: Commands,
    mut slung: ResMut<Slung>,
    carried: Res<Carried>,
    held: Res<HandsHeld>,
    mut loose: ResMut<Loose>,
    // (The gear figure's holster bone is placed while the figure is parked.)
    mut transforms: Query<&mut Transform, crate::platform::park::EvenParked>,
    clothing: Option<Res<crate::player::clothing::Clothing>>,
    figure: Option<ResMut<crate::screens::figure_view::Figure>>,
    (skeletons, dummies, corpse): (Option<Res<crate::world::dummy::DummySkeletons>>, Option<Res<crate::world::dummy::Dummies>>, Option<Res<crate::screens::figure_view::CorpseSkeleton>>),
) {
    let mut dones = Vec::new();
    // The character's corpse: nothing (`Body::hangs`), whatever it has on.
    if let Some(corpse) = corpse {
        let on = On { body: Body::Corpse, bone: &|path| corpse.0.bone(path), layer: None, seen_only: false, whole: false };
        dones.push(keep_hung(&mut slung, &on, Some(&carried.equipment.item), carried.last_changed().get(), held.item, None, &mut commands, &mut loose));
    }
    if let Some(mut figure) = figure {
        let holster = character_holster(&slung, clothing.as_deref());
        if slung.figure_holster != Some(holster) {
            if let Some((path, holster)) = slung.holster_bone(holster) {
                let placed = convert::transform(holster.position, holster.rotation, [1.0; 3]);
                if let Some(mut transform) = figure.skeleton.bone(path).and_then(|bone| transforms.get_mut(bone).ok()) {
                    *transform = placed;
                }
                // The OVERALL tab's own figure's, kept there as it moves.
                if let Some(overall) = figure.overall.as_mut() {
                    overall.fix_bone(path, placed);
                }
            }
            slung.figure_holster = Some(holster);
            figure.ask();
        }
        let done = {
            let skeleton = &figure.skeleton;
            let on = On { body: Body::Figure, bone: &|path| skeleton.bone(path), layer: Some(RenderLayers::layer(crate::screens::figure_view::LAYER)), seen_only: false, whole: true };
            keep_hung(&mut slung, &on, Some(&carried.equipment.item), carried.last_changed().get(), held.item, holster, &mut commands, &mut loose)
        };
        if done.changed {
            figure.wait_for(&done.meshes, &done.materials);
        }
        dones.push(done);
        // The OVERALL tab's own figure's: the thing it was made holding at
        // its showing left off (`figure_view::Overall::in_hand`), so that it
        // never hangs the weapon it holds.
        let done = figure.overall.as_ref().map(|overall| {
            let skeleton = &overall.skeleton;
            let on = On { body: Body::Overall, bone: &|path| skeleton.bone(path), layer: Some(RenderLayers::layer(crate::screens::figure_view::OVERALL_LAYER)), seen_only: false, whole: true };
            keep_hung(&mut slung, &on, Some(&carried.equipment.item), carried.last_changed().get(), overall.in_hand(held.item), holster, &mut commands, &mut loose)
        });
        if let Some(done) = done {
            if done.changed {
                figure.wait_for(&done.meshes, &done.materials);
            }
            dones.push(done);
        }
    }
    if let (Some(skeletons), Some(dummies)) = (skeletons, dummies) {
        for (which, skeleton) in skeletons.0.iter().enumerate() {
            let holster = slung.dummy_holsters.get(which).copied().flatten();
            if let Some((path, holster)) = slung.holster_bone(holster) {
                if let Some(mut transform) = skeleton.bone(path).and_then(|bone| transforms.get_mut(bone).ok()) {
                    *transform = convert::transform(holster.position, holster.rotation, [1.0; 3]);
                }
            }
            let on = On { body: Body::Dummy(which), bone: &|path| skeleton.bone(path), layer: None, seen_only: false, whole: false };
            dones.push(keep_hung(&mut slung, &on, carried.body(which), carried.last_changed().get(), dummies.in_hand(which), holster, &mut commands, &mut loose));
        }
    }
    let_go(&slung, &dones.iter().collect::<Vec<_>>(), &mut loose);
}

/// Shows the character's hung models from outside alone: in first person
/// the game's own character has their renderers off (`CreateAndParent`,
/// `_isYourPlayer`), and so casts no shadow of them either.
pub fn show(view: Res<crate::View>, mut models: Query<&mut Visibility, (With<SlungModel>, crate::platform::park::EvenParked)>) {
    let wish = if *view == crate::View::FirstPerson { Visibility::Hidden } else { Visibility::Inherited };
    for mut visibility in &mut models {
        if *visibility != wish {
            *visibility = wish;
        }
    }
}
