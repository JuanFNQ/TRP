//! The weapon on the figure read, and the figure kept live and posed.

use super::*;

/// The OVERALL tab's own figure's weapon in hand, read away from the frame
/// (`background`): its animated model as assembled for the item's build (a
/// firearm's; its backup sights as any model of it is made) or its own
/// prefab (a grenade's, a melee weapon's), what the figure holds it by
/// (`trp_sim::figure::FigureWeaponData`), and its looks. The last one read
/// is kept for the next showing (its model is made anew at each, from
/// this), its looks in the engine's stores, its pictures shared with the
/// hands' rig of the same where that has them (`render_data::
/// SharedPictures`), let go of when another is read. One that could not be
/// read is not asked for again: the figure holds nothing then.
#[derive(Resource, Default)]
pub struct FigureWeapon {
    task: Option<(String, bevy::tasks::Task<anyhow::Result<WeaponRead>>)>,
    had: Option<WeaponHad>,
    failed: Vec<String>,
}

pub(super) struct WeaponRead {
    pub(super) weapon: trp_data::weapons::weapon::Weapon,
    pub(super) held: trp_sim::characters::figure::FigureWeaponData,
    looks: crate::weapons::weapon::Looks,
}

pub(super) struct WeaponHad {
    pub(super) key: String,
    pub(super) weapon: trp_data::weapons::weapon::Weapon,
    pub(super) held: trp_sim::characters::figure::FigureWeaponData,
    pub(super) drawn: crate::weapons::weapon::Drawn,
    marks: Vec<u64>,
    pub(super) on_card: bool,
}

/// What it waits for on the graphics card under (`UploadWatch`).
fn weapon_on_card(key: &str) -> String {
    format!("the OVERALL figure's {key}")
}

/// The weapon read, on a thread with a reader of its own.
fn read_weapon(install: trp_data::Install, db: std::sync::Arc<trp_data::Database>, held: &Held) -> anyhow::Result<WeaponRead> {
    let mut unity = trp_data::unity::Assets::new(install)?;
    // A window magazine's glass's cube, as the hands' rigs' (`glass_view`).
    crate::effects::glass_view::open_cubes(&mut unity);
    let weapon = match &held.prefab {
        Some(prefab) => trp_data::weapons::weapon::assemble_prefab(&mut unity, prefab)?,
        None => {
            let mut weapon = trp_data::weapons::weapon::assemble_key(&mut unity, &db, &held.key)?;
            trp_sim::weapons::sights::fold_sights_as_made(&mut weapon);
            weapon
        }
    };
    let figure = trp_sim::characters::figure::FigureWeaponData::load(&unity, &weapon)?;
    // (Its cubes for the figure's own surface: `figure_surface`.)
    let looks = crate::weapons::weapon::looks_into(&unity, &weapon, crate::assets::render_data::PreparedLooks::default().with_figure_cubes());
    Ok(WeaponRead { weapon, held: figure, looks })
}

/// What the figure holds at a showing (`PlayerModelLoader.GetItemInHands`,
/// in a raid the inventory controller's item in hand): the item in hand's
/// rig by its key, the own prefab of a grenade, a melee weapon, the
/// rangefinder or a radio transmitter, a firearm's build; the kind the body
/// is told (`trp_sim::figure::kind_of`); its rounds in its own chambers
/// (`Weapon.ChamberAmmoCount`: a revolver's cylinder none) and its stock's
/// fold. Anything else in hand (medicine, food, drink) is told its kind
/// with no model (`Held::model`). None for the empty hands.
pub(super) fn held_now(gear: &trp_sim::items::inventory::Item, in_hand: Option<trp_sim::items::inventory::Id>, db: &trp_data::Database) -> Option<Held> {
    use trp_sim::characters::figure::kind;
    use trp_sim::items::inventory as inv;
    let item = gear.find(in_hand?)?;
    let template = item.kind.template.as_str();
    let kind = trp_sim::characters::figure::kind_of(db, template);
    let own_prefab = kind == kind::THROWN || kind == kind::KNIFE || ["PortableRangeFinder", "RadioTransmitter"].into_iter().any(|class| inv::is(item, class));
    let (prefab, model) = if own_prefab {
        (Some(db.items.get(template)?.props["Prefab"]["path"].as_str().filter(|path| !path.is_empty())?.to_owned()), true)
    } else {
        (None, inv::is(item, "Weapon"))
    };
    let cylinder = item.in_slot(inv::MAGAZINE_SLOT).is_some_and(inv::is_cylinder);
    let chamber = if cylinder || !model { 0 } else { inv::chamber_rounds(item).iter().flatten().count() as u32 };
    let folded = inv::folding_part(item).is_some_and(|part| part.folded);
    Some(Held { item: item.id, key: inv::rig_key(item), prefab, kind, chamber, folded, model })
}

/// The magazine's points for its rounds in a weapon's model, by its
/// objects: the first object under its magazine's slot (`mod_magazine`,
/// as `magazine_rounds` finds the hands' and `loose` a thing's), then every
/// object below it whose name numbers a point, breadth first, as the game
/// finds them (`FindAllTransformsWithNameContains`); none without a
/// magazine or points.
fn magazine_points(hierarchy: &trp_data::unity::extract::Hierarchy, entities: &[Entity]) -> Option<(Entity, Vec<(Entity, u32)>)> {
    let nodes = &hierarchy.nodes;
    let below = |parent: usize| (0..nodes.len()).filter(move |&at| nodes[at].parent == Some(parent));
    let slot = nodes.iter().position(|node| node.name == "mod_magazine")?;
    let magazine = below(slot).next()?;
    let mut points = Vec::new();
    let mut queue = std::collections::VecDeque::from([magazine]);
    while let Some(at) = queue.pop_front() {
        if let Some(number) = trp_sim::items::magazine_view::point_number(&nodes[at].name) {
            points.push((*entities.get(at)?, number));
        }
        queue.extend(below(at));
    }
    (!points.is_empty()).then(|| Some((*entities.get(magazine)?, points))).flatten()
}

/// A weapon's chambered rounds on its model (`AttachMods`: each chamber's
/// round on its slot's object, the shallowest of the slot's name, as
/// `loose::shown_rounds` puts a thing's): each object and the round's
/// template. A cylinder's are not built (as a hung weapon's).
fn chambered_rounds(item: &trp_sim::items::inventory::Item, hierarchy: &trp_data::unity::extract::Hierarchy, entities: &[Entity]) -> Vec<(Entity, String)> {
    if !trp_sim::items::inventory::is(item, "Weapon") {
        return Vec::new();
    }
    item.slots
        .iter()
        .zip(&item.kind.slots)
        .filter(|(_, kind)| kind.name.starts_with("patron_in_weapon"))
        .filter_map(|(slot, kind)| {
            let round = slot.as_ref()?;
            let node = hierarchy.nodes.iter().position(|node| node.name.eq_ignore_ascii_case(&kind.name)).and_then(|first| hierarchy.find_below(0, &hierarchy.nodes[first].name))?;
            Some((*entities.get(node)?, round.kind.template.clone()))
        })
        .collect()
}

/// The layers a light of the OVERALL figure's own is on (its weapon's
/// devices': `tactical_view`): its pieces' and its lights', so that it is
/// seen by the figure's camera alone and lights the figure, whose pieces
/// cast its shadows.
pub fn lamp_layers() -> RenderLayers {
    RenderLayers::from_layers(&[OVERALL_LAYER, OVERALL_LIGHTS])
}

/// The OVERALL tab's own figure (`OVERALL FIGURE`), every frame. As its
/// page shows (`OverallFigure::remake`) the figure is made anew
/// (`PlayerModelView.Show`): told what is in hand, its weapon's model made
/// on its ribcage from the one read (read first, away from the frame, the
/// spinner showing meanwhile, as the game's while its model loads), its
/// picture drawn; its clock from nought as the spinner goes, the figure
/// stepped by the frame's time and posed (its bones, its outfits, the
/// weapon's rig), its camera on every frame while the page is shown (the
/// game's animator runs on), turned by the page's drag. Hidden, its camera
/// is off and its weapon's model goes. Once as the run begins it is drawn
/// a few frames into the small picture, which builds its pipelines.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn live(
    mut commands: Commands,
    figure: Option<ResMut<Figure>>,
    mut overall: ResMut<OverallFigure>,
    mut reads: ResMut<FigureWeapon>,
    (controller, selected, carried, held): (Res<crate::player::controller::Controller>, Res<crate::screens::inventory_view::Selected>, Res<Carried>, Res<crate::screens::inventory_view::HandsHeld>),
    (game, options, time, hold, picks): (Res<crate::GameData>, Res<crate::Options>, Res<Time>, Option<Res<FigureHold>>, Option<Res<FigurePicks>>),
    (uploads, waiting, mut loose): (Res<UploadWatch>, Res<crate::PipelinesWaiting>, ResMut<crate::world::loose::Loose>),
    (mut meshes, mut images, mut materials, mut bindposes, mut shared): (
        ResMut<Assets<Mesh>>,
        ResMut<Assets<Image>>,
        ResMut<Assets<StandardMaterial>>,
        ResMut<Assets<SkinnedMeshInverseBindposes>>,
        ResMut<crate::assets::render_data::SharedPictures>,
    ),
    mut cameras: Query<&mut Camera>,
    // (Even parked: the figure is posed on the frame its camera comes on,
    // before it is back, `camera_draw::show_hung`.)
    mut transforms: Query<&mut Transform, crate::platform::park::EvenParked>,
    globals: Query<&GlobalTransform>,
) {
    let Some(mut figure) = figure else { return };
    let figure = &mut *figure;
    let hold = hold.map(|hold| *hold);
    let (on_card, camera, gear_drawing) = (figure.on_card, figure.shots[OVERALL].camera, figure.drawing.is_some());
    let Some(live) = figure.overall.as_mut() else { return };
    let shown = controller.inventory_open && selected.0 == crate::screens::inventory_view::OVERALL;
    let mut on = false;
    // The weapon read: into the stores, the one before let go of.
    if let Some((key, task)) = reads.task.as_mut() {
        if let Some(read) = crate::assets::background::take(task, options.unattended(), "the OVERALL figure's weapon") {
            let key = key.clone();
            reads.task = None;
            match read {
                Ok(read) => {
                    let (drawn, pictures, made, kept, count) = crate::weapons::weapon::looks_into_world(read.looks, &mut meshes, &mut images, &mut materials, &mut bindposes, &mut shared);
                    uploads.wait(weapon_on_card(&key), pictures.iter().map(Handle::id).collect(), made.iter().map(Handle::id).collect());
                    info!("the OVERALL figure's weapon in hand, {}: read away from the frame, {} nodes; {} of its {count} pictures already on the card", read.weapon.name, read.weapon.hierarchy.nodes.len(), kept.already);
                    if let Some(old) = reads.had.replace(WeaponHad { key, weapon: read.weapon, held: read.held, drawn, marks: kept.marks, on_card: false }) {
                        shared.release(&old.marks);
                    }
                }
                Err(e) => {
                    warn!("the OVERALL figure's weapon in hand, {key}, could not be read; the figure holds nothing: {e:#}");
                    reads.failed.push(key);
                }
            }
        }
    }
    if let Some(had) = reads.had.as_mut().filter(|had| !had.on_card) {
        let name = weapon_on_card(&had.key);
        had.on_card = !uploads.take(|taken| taken == name).is_empty();
    }
    if !shown {
        // Gone with its page: made anew at the next showing.
        if live.made || live.weapon.is_some() {
            if let Some(tree) = live.weapon.take() {
                commands.entity(tree[0]).despawn();
            }
            (live.devices, live.chambered) = (Vec::new(), Vec::new());
            (live.made, live.running, live.clock, live.frames) = (false, None, false, 0);
        }
        // Its first drawing as the run begins, into the small picture,
        // after the gear page's (the two drawn one after the other, as
        // before: the pictures' levels go by the cameras drawing them,
        // `streaming`).
        if !live.warmed && on_card && camera.is_some() && (!gear_drawing || live.frames > 0) {
            live.frames += 1;
            on = true;
            if live.frames > DRAW_FRAMES && waiting.0.load(std::sync::atomic::Ordering::Relaxed) == 0 {
                (live.warmed, live.frames, on) = (true, 0, false);
            }
        }
    } else {
        // Shown anew: what it holds now.
        if overall.remake {
            overall.remake = false;
            overall.making = true;
            if let Some(tree) = live.weapon.take() {
                commands.entity(tree[0]).despawn();
            }
            (live.devices, live.chambered) = (Vec::new(), Vec::new());
            (live.made, live.running, live.clock, live.frames, live.held_frames) = (false, None, false, 0, 0);
            live.wanted = held_now(&carried.equipment.item, held.item, &game.db);
            live.held_item = held.item;
            live.showings += 1;
        }
        // Its weapon read if it is not: asked for (an unattended run waits
        // for it on this frame, `background`).
        if let Some(wanted) = live.wanted.as_ref().filter(|wanted| wanted.model && !live.made) {
            let have = reads.had.as_ref().is_some_and(|had| had.key == wanted.key);
            let asked = reads.task.as_ref().is_some_and(|(key, _)| *key == wanted.key);
            if !have && !asked && !reads.failed.contains(&wanted.key) && reads.task.is_none() {
                let (install, db, wanted) = (game.unity.install.clone(), game.db.clone(), wanted.clone());
                let key = wanted.key.clone();
                let task = bevy::tasks::AsyncComputeTaskPool::get().spawn(async move { read_weapon(install, db, &wanted) });
                reads.task = Some((key, task));
            }
        }
        // Made once what it holds is had (or cannot be).
        if !live.made {
            let weapon = live.wanted.as_ref().filter(|wanted| wanted.model).map(|wanted| (wanted, reads.had.as_ref().filter(|had| had.key == wanted.key)));
            let ready = match weapon {
                None => true,
                Some((wanted, had)) => had.is_some() || reads.failed.contains(&wanted.key),
            };
            if ready {
                let seed = crate::fixed_seed(0x0f16_u64).unwrap_or_else(|| std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |d| d.subsec_nanos() as u64));
                let seed = (seed as i32).wrapping_add(live.showings as i32);
                let holding = weapon.and_then(|(wanted, had)| had.map(|had| (wanted, had)));
                let (kind, with) = match holding {
                    Some((wanted, had)) => {
                        let loaded = trp_sim::characters::figure::Loaded::of(&game.db, &had.weapon, wanted.chamber, wanted.folded);
                        (wanted.kind, Some((&had.held, loaded)))
                    }
                    // Nothing made for its hands: told the item's kind all
                    // the same where it is one the game makes no model for
                    // (one that could not be read: nothing).
                    None => (live.wanted.as_ref().filter(|wanted| !wanted.model).map_or(trp_sim::characters::figure::kind::NOTHING, |wanted| wanted.kind), None),
                };
                let mut running = trp_sim::characters::figure::Figure::new(&live.data, kind, with, seed);
                for &pick in picks.iter().flat_map(|picks| &picks.0) {
                    running.steer(pick);
                }
                live.running = Some(running);
                live.landmarks = ["Base HumanHead", "Base HumanRPalm", "Base HumanLPalm"].into_iter().zip(["head", "right palm", "left palm"]).filter_map(|(bone, name)| Some((name, live.skeleton.bone(bone)?))).collect();
                // The weapon's model on the ribcage, on the figure's layer.
                if let (Some((wanted, had)), Some(ribcage)) = (holding, live.ribcage) {
                    let (parts, skins, extra) = &had.drawn;
                    let tree = crate::weapons::weapon::spawn_nodes(&mut commands, &had.weapon.hierarchy, ribcage, None, parts, skins, extra);
                    for (_, _, piece) in &tree.drawn {
                        commands.entity(*piece).insert(RenderLayers::layer(OVERALL_LAYER));
                    }
                    if let Some(muzzle) = had.weapon.hierarchy.nodes.iter().position(|node| node.name == "fireport") {
                        live.landmarks.push(("muzzle", tree.entities[muzzle]));
                    }
                    // Its devices (`tactical_view`), and with `MAGAZINE
                    // ROUNDS` its magazine's rounds and its chambered ones,
                    // as a hung weapon's whole model has them (the game's
                    // figure's weapon is made by the same factory).
                    live.devices = had.weapon.devices.clone();
                    let item = carried.equipment.item.find(wanted.item);
                    if loose.rounds {
                        let found = magazine_points(&had.weapon.hierarchy, &tree.entities);
                        let count = found.as_ref().map_or(0, |(_, points)| points.len());
                        if let Some((magazine, points)) = found {
                            let drawn = crate::weapons::magazine_rounds::MagazinePoints::new(crate::weapons::magazine_rounds::Owner::Figure(wanted.item), points).on_layer(RenderLayers::layer(OVERALL_LAYER));
                            commands.entity(magazine).insert(drawn);
                        }
                        live.chambered = item.map(|item| chambered_rounds(item, &had.weapon.hierarchy, &tree.entities)).unwrap_or_default();
                        info!("the OVERALL figure's weapon: its magazine's {count} points for rounds, {} chambered rounds, {} tactical devices", live.chambered.len(), live.devices.len());
                    }
                    live.weapon = Some(tree.entities);
                }
                live.made = true;
                info!(
                    "the OVERALL figure made (showing {}, its view {:?} in the screen's units): {}, kind {kind}{}",
                    live.showings,
                    overall.rect,
                    holding.map_or("nothing in hand".to_owned(), |(wanted, had)| format!("{} in hand ({}, {} chambered{})", had.weapon.name, wanted.key, wanted.chamber, if wanted.folded { ", folded" } else { "" })),
                    live.wanted.as_ref().filter(|_| holding.is_none()).map_or(String::new(), |wanted| if wanted.model {
                        format!(" ({} could not be read)", wanted.key)
                    } else {
                        format!(" ({}: the game makes no model of it for the hands)", wanted.key)
                    })
                );
            }
        }
        // The chambered rounds whose models have come, on the figure's layer.
        if !live.chambered.is_empty() {
            let waiting = std::mem::take(&mut live.chambered);
            for (point, template) in waiting {
                match loose.model_for(&format!("{}{template}", crate::world::loose::ROUND_MODEL)) {
                    Some(ready) => {
                        let round = crate::weapons::magazine_rounds::spawn_round_on(&mut commands, &ready, Some(&RenderLayers::layer(OVERALL_LAYER)), false);
                        commands.entity(point).add_child(round);
                    }
                    None => live.chambered.push((point, template)),
                }
            }
        }
        // Drawn once it and its weapon are on the graphics card; the
        // spinner goes after its first drawing, and its clock runs from
        // then (the game's figure is made as its spinner goes).
        let weapon_ready = live.weapon.is_none() || reads.had.as_ref().is_some_and(|had| had.on_card);
        if live.made && on_card && weapon_ready && camera.is_some() {
            on = true;
            live.frames += 1;
            if overall.making && live.frames > DRAW_FRAMES && waiting.0.load(std::sync::atomic::Ordering::Relaxed) == 0 {
                overall.making = false;
                live.clock = true;
                info!("the OVERALL figure in its view: its clock from nought");
            }
        }
        if let Some(running) = live.running.as_mut().filter(|_| live.clock) {
            let dt = time.delta_secs();
            let dt = match hold {
                Some(hold) => (hold.0 - running.age()).clamp(0.0, dt),
                None => dt,
            };
            if dt > 0.0 {
                running.step(dt);
            }
        }
        pose(live, overall.yaw, &mut transforms);
        // A capture's aid (`--figure-hold`): once held a few frames (its
        // places through the engine's), where its head, its palms and its
        // weapon's muzzle are in its view, in the view's pixels, and its
        // turn.
        if let (Some(hold), Some(running), Some(camera)) = (hold, live.running.as_ref(), camera) {
            if running.age() >= hold.0 - 1e-6 {
                live.held_frames += 1;
            }
            if live.held_frames == 4 {
                let in_view = |entity: Entity| -> Option<Vec2> {
                    let (eye, at) = (globals.get(camera).ok()?, globals.get(entity).ok()?);
                    Some(cameras.get(camera).ok()?.world_to_viewport(eye, at.translation()).ok()? / 2.0)
                };
                let said: Vec<String> = live.landmarks.iter().map(|(name, entity)| format!("{name} {:?}", in_view(*entity).map(|p| [p.x.round(), p.y.round()]))).collect();
                info!("the OVERALL figure held at {:.3} s, turned {:.1} degrees from its making: {}", running.age(), overall.yaw - YAW, said.join(", "));
            }
        }
    }
    if let Some(Ok(mut camera)) = camera.map(|camera| cameras.get_mut(camera)) {
        if camera.is_active != on {
            camera.is_active = on;
        }
    }
}

/// The figure as its running figure leaves it: its root turned as its page
/// has it, every bone, its outfits' skeletons, the weapon's rig (its root
/// on the ribcage at nought, as its prefab is hung).
pub(super) fn pose(live: &Overall, yaw: f32, transforms: &mut Query<&mut Transform, crate::platform::park::EvenParked>) {
    let Some(running) = live.running.as_ref() else { return };
    if let Ok(mut root) = transforms.get_mut(live.skeleton.root) {
        let rotation = turned(yaw);
        if root.rotation != rotation {
            root.rotation = rotation;
        }
    }
    let pose = running.pose();
    let parents = live.data.parents();
    let mut world: Vec<Affine3A> = Vec::with_capacity(pose.len());
    for (index, (bone, (at, turn))) in live.bones.iter().zip(pose).enumerate() {
        let fixed = live.fixed.iter().find(|(at, _)| *at == index).map(|(_, place)| *place);
        let local = fixed.unwrap_or(Transform { translation: convert::position(*at), rotation: convert::rotation(*turn), scale: Vec3::ONE });
        if let Ok(mut transform) = transforms.get_mut(*bone) {
            if *transform != local {
                *transform = local;
            }
        }
        world.push(parents[index].map_or(Affine3A::IDENTITY, |parent| world[parent]) * local.compute_affine());
    }
    for outfit in &live.outfits {
        for (entity, placed) in outfit.placed(&world) {
            if let Ok(mut transform) = transforms.get_mut(entity) {
                *transform = placed;
            }
        }
    }
    if let (Some(tree), Some(rig)) = (live.weapon.as_ref(), running.weapon_pose()) {
        for (index, (entity, (at, turn))) in tree.iter().zip(&rig).enumerate() {
            if let Ok(mut transform) = transforms.get_mut(*entity) {
                // The prefab's root on the ribcage at nought, whatever its
                // own place in the prefab; below it the rig as posed.
                let (at, turn) = if index == 0 { (Vec3::ZERO, Quat::IDENTITY) } else { (convert::position(*at), convert::rotation(*turn)) };
                if transform.translation != at || transform.rotation != turn {
                    (transform.translation, transform.rotation) = (at, turn);
                }
            }
        }
    }
}

/// Every picture a material draws with.
pub(super) fn pictures_of(material: &StandardMaterial) -> Vec<Handle<Image>> {
    [&material.base_color_texture, &material.normal_map_texture, &material.metallic_roughness_texture, &material.occlusion_texture, &material.emissive_texture].into_iter().flatten().cloned().collect()
}
