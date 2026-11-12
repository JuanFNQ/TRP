//! Head models kept, hung and fitted, and read models hung on the body.

use super::*;

/// A model on the head as hung (`spawn_body`), kept for the parts fitted
/// to it later (`fit_head_parts`): the worn thing's template, its tree as
/// it now stands, its parts, the ways down asked for that hang nothing,
/// and each of its objects' entities on the body, the gear page's figure,
/// the corpse and each dummy.
pub struct HeadKept {
    pub(super) item: String,
    pub(super) model: Hierarchy,
    pub(super) parts: Vec<trp_data::character::dress::WornPart>,
    pub(super) fittings: Vec<(usize, trp_data::character::dress::Fitting)>,
    pub(super) none: HashSet<Vec<(String, String)>>,
    pub(super) body: Vec<Entity>,
    pub(super) figure: Vec<Entity>,
    /// The OVERALL tab's own figure's (`OVERALL FIGURE`).
    pub(super) overall: Vec<Entity>,
    pub(super) corpse: Vec<Entity>,
    pub(super) dummies: Vec<Vec<Entity>>,
}

/// Every model on the head, as hung.
#[derive(Resource, Default)]
pub struct HeadModels(pub(super) Vec<HeadKept>);

impl HeadModels {
    /// The things worn whose models are hung, by template.
    pub(crate) fn items(&self) -> impl Iterator<Item = &str> {
        self.0.iter().map(|kept| kept.item.as_str())
    }

    /// A head device's objects on the drawn body (tactical-devices spec,
    /// "The look"): the device by its template, the thing worn itself or a
    /// part fitted to it. Its own model's objects are numbered as
    /// `trp_data::tactical::read_alone` numbers them, from its root: a part
    /// is hung as its prefab saves its tree (`Hierarchy::attach`), so its
    /// object `n` is the hung tree's `root + n`.
    pub(crate) fn device_on_body(&self, template: &str) -> Option<DeviceOnBody<'_>> {
        self.0.iter().find_map(|kept| {
            let root = if kept.item == template { Some(0) } else { kept.parts.iter().find(|part| part.path.last().is_some_and(|(_, of)| of == template)).map(|part| part.root) };
            root.map(|root| DeviceOnBody { kept, root })
        })
    }
}

/// A head device as hung on the body's model of what it is worn on or
/// fitted to (`HeadModels::device_on_body`).
pub(crate) struct DeviceOnBody<'a> {
    pub(super) kept: &'a HeadKept,
    pub(super) root: usize,
}

impl<'a> DeviceOnBody<'a> {
    /// The entity on the body of the device's object `node` (as its own
    /// model numbers it), and that object's name.
    pub(crate) fn object(&self, node: usize) -> Option<(Entity, &'a str)> {
        let kept: &'a HeadKept = self.kept;
        let at = self.root + node;
        Some((*kept.body.get(at)?, kept.model.nodes.get(at)?.name.as_str()))
    }
}

/// The head bone of the body and of each copy, which a model's first
/// object hangs on.
#[derive(Default)]
pub(super) struct Heads {
    pub(super) body: Option<Entity>,
    pub(super) figure: Option<Entity>,
    pub(super) overall: Option<Entity>,
    pub(super) corpse: Option<Entity>,
    pub(super) dummies: Vec<Option<Entity>>,
}

/// The copies of the body a model on the head is hung on beside it: the
/// gear page's figure, the corpse and the dummies, as they are built
/// (`figure_view::Builder`) or as kept (`figure_view::Skeleton`).
pub(super) struct Copies<'a, T: crate::screens::figure_view::Hangs> {
    pub(super) figure: Option<&'a mut T>,
    /// The OVERALL tab's own figure (`OVERALL FIGURE`), as the gear page's.
    pub(super) overall: Option<&'a mut T>,
    pub(super) corpse: Option<&'a mut T>,
    pub(super) dummies: &'a mut [T],
}

/// Hangs a model on the head's objects from `from` on (the whole of it as
/// the body is made; later, the objects a part fitted added): each on the
/// body under its parent (the first on the head bone, with the one fixed
/// turn the game gives it), shown as the model has it; the model's first
/// object marked to show and hide it whole (`show_worn`), a part's root
/// to show and hide the part (`show_head_parts`), a hinge's object given
/// its hinge (`hinges`, by node); and each again on every copy, shown as
/// the body's is. Its meshes drawn as they lie, on the body and on each
/// copy, as `looks` gives them for each object (by its index); a mesh with
/// a second form (`kept.fittings`) given both on each, by whose things it
/// goes (`Fitted`, `fit_head_things`). Each mesh and material drawn on the
/// body, the second forms' with them.
pub(super) fn hang_head_model<T: crate::screens::figure_view::Hangs>(
    commands: &mut Commands,
    looks: &mut NodeLooks,
    kept: &mut HeadKept,
    from: usize,
    hinges: &[trp_data::character::dress::Hinge],
    heads: &Heads,
    mut copies: Copies<T>,
) -> Vec<(Handle<Mesh>, Handle<StandardMaterial>)> {
    let mut made = Vec::new();
    let item = kept.item.clone();
    // Its groups of levels of detail, each placed by its own object, which
    // switch together (`lod_view`), where they are drawn by.
    let specs: Vec<std::sync::Arc<crate::assets::lod_view::LodSpec>> = if crate::assets::lod_view::on() { crate::assets::lod_view::LodSpec::bundle(kept.model.lods.iter().map(|lod| &*lod.group)) } else { Vec::new() };
    for index in from..kept.model.nodes.len() {
        let node = &kept.model.nodes[index];
        let transform = match index {
            0 => convert::transform([0.0; 3], [0.5, -0.5, -0.5, -0.5], [1.0; 3]),
            _ => convert::transform(node.translation, node.rotation, node.scale),
        };
        let visibility = if node.active { Visibility::Inherited } else { Visibility::Hidden };
        let entity = commands.spawn((Name::new(node.name.clone()), transform, visibility)).id();
        // The whole model is shown and hidden by its first object, a
        // part it may have fitted by the part's.
        if index == 0 {
            commands.entity(entity).insert((WornPiece { item: item.clone(), over: Vec::new() }, Park));
        }
        let part = kept.parts.iter().find(|part| part.root == index);
        if let Some(part) = part {
            commands.entity(entity).insert((HeadPart { item: item.clone(), path: part.path.clone() }, Park));
        }
        let shown_by = (index == 0 || part.is_some()).then_some(entity);
        // What a dummy's copy is shown by, on the dummy's own gear
        // (`dummy::show_dummies`): the whole by the thing worn, a part by
        // its being fitted.
        let dummy_shows = match (index, part) {
            (0, _) => Some(crate::world::dummy::Shows::Worn { item: item.clone(), over: Vec::new() }),
            (_, Some(part)) => Some(crate::world::dummy::Shows::Head { item: item.clone(), path: part.path.clone() }),
            _ => None,
        };
        // Its hinge, on each copy of it.
        let hinged = hinges.iter().find(|hinge| hinge.node == index).map(|hinge| HelmetHinge {
            item: item.clone(),
            owner: hinge.owner.clone(),
            off: hinge.off,
            on: hinge.on,
            hinge: trp_sim::effects::visor::ShieldHinge::new(hinge.forward.clone(), hinge.backward.clone()),
            figure: false,
        });
        if let Some(hinged) = &hinged {
            commands.entity(entity).insert(hinged.clone());
        }
        if let Some(parent) = node.parent.map(|p| kept.body[p]).or(heads.body) {
            commands.entity(parent).add_child(entity);
        }
        kept.body.push(entity);
        if let Some(figure) = copies.figure.as_deref_mut() {
            let on = node.parent.map(|p| kept.figure[p]).or(heads.figure);
            let copy = figure.hang(commands, on, transform, visibility, shown_by);
            if let Some(hinged) = &hinged {
                commands.entity(copy).insert(HelmetHinge { figure: true, ..hinged.clone() });
            }
            kept.figure.push(copy);
        }
        if let Some(overall) = copies.overall.as_deref_mut() {
            let on = node.parent.map(|p| kept.overall[p]).or(heads.overall);
            let copy = overall.hang(commands, on, transform, visibility, shown_by);
            if let Some(hinged) = &hinged {
                commands.entity(copy).insert(HelmetHinge { figure: true, ..hinged.clone() });
            }
            kept.overall.push(copy);
        }
        for (which, dummy) in copies.dummies.iter_mut().enumerate() {
            let on = node.parent.map(|p| kept.dummies[which][p]).or(heads.dummies.get(which).copied().flatten());
            let copy = dummy.hang(commands, on, transform, visibility, shown_by);
            if let Some(shows) = &dummy_shows {
                commands.entity(copy).insert(crate::world::dummy::DummyShows(shows.clone()));
            }
            kept.dummies[which].push(copy);
        }
        if let Some(corpse) = copies.corpse.as_deref_mut() {
            let on = node.parent.map(|p| kept.corpse[p]).or(heads.corpse);
            let copy = corpse.hang(commands, on, transform, visibility, shown_by);
            if let Some(hinged) = hinged {
                commands.entity(copy).insert(hinged);
            }
            kept.corpse.push(copy);
        }
        let (node, mirrored) = (&kept.model.nodes[index], kept.model.mirrored(index));
        let drawn = looks(index, node, mirrored, None);
        // Its levels of detail, on the body and on each copy: placed by
        // that copy's object of its group.
        let lod_on = |anchors: &[Entity]| -> Option<crate::assets::lod_view::LodPiece> {
            let lod = node.lod?;
            let group = specs.get(lod.group)?.clone();
            let anchor = *anchors.get(kept.model.lods.get(lod.group)?.node)?;
            Some(crate::assets::lod_view::LodPiece { group, anchor: Some(anchor), levels: lod.levels })
        };
        let (body_lod, figure_lod, overall_lod, corpse_lod) = (lod_on(&kept.body), lod_on(&kept.figure), lod_on(&kept.overall), lod_on(&kept.corpse));
        let dummy_lods: Vec<Option<crate::assets::lod_view::LodPiece>> = kept.dummies.iter().map(|copies| lod_on(copies)).collect();
        // Its second form, where it has one.
        let fitting = kept.fittings.iter().find(|(at, _)| *at == index).map(|(_, fitting)| *fitting).filter(|_| !drawn.is_empty());
        let seconds = match fitting {
            Some(fitting) => second_form(drawn.len(), &looks(index, node, mirrored, Some(fitting.custom)).into_iter().map(|(mesh, _)| mesh).collect::<Vec<_>>()),
            None => Vec::new(),
        };
        for (slot, (mesh, material)) in drawn.into_iter().enumerate() {
            let fitted = fitting.zip(seconds.get(slot)).map(|(fitting, second)| Fitted { wearer: Wearer::Character, kind: fitting.kind, meshes: [mesh.clone(), second.clone()], poses: None });
            let piece = commands.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(material.clone()), Transform::default(), UpperBody, RenderLayers::layer(SEEN))).id();
            commands.entity(entity).add_child(piece);
            if let Some(fitted) = &fitted {
                commands.entity(piece).insert(fitted.clone());
            }
            if let Some(lod) = &body_lod {
                commands.entity(piece).insert(lod.clone());
            }
            if let (Some(figure), Some(on)) = (copies.figure.as_deref_mut(), kept.figure.last()) {
                let copy = figure.lay(commands, *on, &mesh, &material);
                if let Some(fitted) = &fitted {
                    figure.keep_mesh(&fitted.meshes[1]);
                    commands.entity(copy).insert(fitted.of(Wearer::Figure));
                }
                if let Some(lod) = &figure_lod {
                    commands.entity(copy).insert(lod.clone());
                }
            }
            if let (Some(overall), Some(on)) = (copies.overall.as_deref_mut(), kept.overall.last()) {
                let copy = overall.lay(commands, *on, &mesh, &material);
                if let Some(fitted) = &fitted {
                    overall.keep_mesh(&fitted.meshes[1]);
                    commands.entity(copy).insert(fitted.of(Wearer::Figure));
                }
                if let Some(lod) = &overall_lod {
                    commands.entity(copy).insert(lod.clone());
                }
            }
            if let (Some(corpse), Some(on)) = (copies.corpse.as_deref_mut(), kept.corpse.last()) {
                let corpse_piece = corpse.lay(commands, *on, &mesh, &material);
                commands.entity(corpse_piece).insert(crate::player::death::CorpseHead);
                if let Some(fitted) = &fitted {
                    commands.entity(corpse_piece).insert(fitted.of(Wearer::Corpse));
                }
                if let Some(lod) = &corpse_lod {
                    commands.entity(corpse_piece).insert(lod.clone());
                }
            }
            for (which, dummy) in copies.dummies.iter_mut().enumerate() {
                if let Some(on) = kept.dummies[which].last() {
                    let copy = dummy.lay(commands, *on, &mesh, &material);
                    if let Some(fitted) = &fitted {
                        commands.entity(copy).insert(fitted.of(Wearer::Dummy(which)));
                    }
                    if let Some(lod) = dummy_lods.get(which).cloned().flatten() {
                        commands.entity(copy).insert(lod);
                    }
                }
            }
            if let Some(fitted) = &fitted {
                made.push((fitted.meshes[1].clone(), material.clone()));
            }
            made.push((mesh, material));
        }
    }
    made
}

/// The meshes and materials of a model's object, by its index, drawn as
/// it lies (`hang_head_model`).
/// The flag: the object is mirrored where it stands (`Hierarchy::mirrored`).
/// The mesh: its renderer's second form's (`trp_data::dress::Fitting`), in
/// place of its own, where one is asked for (only its meshes are taken).
pub(crate) type NodeLooks<'a> = dyn FnMut(usize, &trp_data::unity::extract::Node, bool, Option<trp_data::unity::ObjRef>) -> Vec<(Handle<Mesh>, Handle<StandardMaterial>)> + 'a;

/// A model's objects' looks made from the install on the frame
/// (`Converted`): hung on the head and drawn as it lies, a mesh with bones
/// and none to move them stops the renderer, so each is unskinned.
pub(super) fn converted_looks<'a>(
    unity: &'a UnityAssets,
    converted: &'a mut Converted,
    meshes: &'a mut Assets<Mesh>,
    images: &'a mut Assets<Image>,
    materials: &'a mut Assets<StandardMaterial>,
) -> impl FnMut(usize, &trp_data::unity::extract::Node, bool, Option<trp_data::unity::ObjRef>) -> Vec<(Handle<Mesh>, Handle<StandardMaterial>)> + 'a {
    move |_, node, mirrored, second| {
        // A coarser level's too, where the levels are drawn by (`lod_view`).
        let Some(renderer) = node.renderer.as_ref().or(node.lower.as_ref().filter(|_| crate::assets::lod_view::on())) else { return Vec::new() };
        let entry = match converted.mesh(unity, second.unwrap_or(renderer.mesh), mirrored, meshes) {
            Ok(entry) => entry,
            Err(e) => {
                warn!("mesh on {}: {e:#}", node.name);
                return Vec::new();
            }
        };
        crate::assets::render_data::unskin_all(meshes, &entry.parts);
        entry.parts.iter().enumerate().map(|(slot, mesh)| (mesh.clone(), converted.material(unity, renderer.materials.get(slot).copied().flatten(), images, materials))).collect()
    }
}

/// A model on the head read after the body was made (`worn_reads`), hung
/// as `spawn_body` hangs those read with it, on the body (its head bone
/// `head`) and on every copy kept, with its objects' looks made ready away
/// from the frame (each object's by its index, and its second form's
/// meshes where it has one), and kept with the rest. The meshes and
/// materials it drew with.
#[allow(clippy::too_many_arguments)]
pub(crate) fn hang_read_model(
    commands: &mut Commands,
    models: &mut HeadModels,
    model: HeadModel,
    mut looks: Vec<Vec<(Handle<Mesh>, Handle<StandardMaterial>)>>,
    mut seconds: Vec<Vec<Handle<Mesh>>>,
    head: Option<Entity>,
    (figure, overall, corpse, dummies): (Option<&mut crate::screens::figure_view::Skeleton>, Option<&mut crate::screens::figure_view::Skeleton>, Option<&mut crate::screens::figure_view::Skeleton>, &mut [crate::screens::figure_view::Skeleton]),
) -> Vec<(Handle<Mesh>, Handle<StandardMaterial>)> {
    let heads = Heads {
        body: head,
        figure: figure.as_deref().and_then(|skeleton| skeleton.bone(HEAD)),
        overall: overall.as_deref().and_then(|skeleton| skeleton.bone(HEAD)),
        corpse: corpse.as_deref().and_then(|skeleton| skeleton.bone(HEAD)),
        dummies: dummies.iter().map(|skeleton| skeleton.bone(HEAD)).collect(),
    };
    let HeadModel { item, model, parts, hinges, fittings } = model;
    let mut kept = HeadKept { item, model, parts, fittings, none: HashSet::new(), body: Vec::new(), figure: Vec::new(), overall: Vec::new(), corpse: Vec::new(), dummies: vec![Vec::new(); dummies.len()] };
    // Made already, mirrored where the object is (`worn_reads`).
    let mut take = |index: usize, _: &trp_data::unity::extract::Node, _: bool, second: Option<trp_data::unity::ObjRef>| match second {
        Some(_) => seconds.get_mut(index).map(std::mem::take).unwrap_or_default().into_iter().map(|mesh| (mesh, Handle::default())).collect(),
        None => looks.get_mut(index).map(std::mem::take).unwrap_or_default(),
    };
    let made = hang_head_model(commands, &mut take, &mut kept, 0, &hinges, &heads, Copies { figure, overall, corpse, dummies });
    models.0.push(kept);
    made
}

/// Every way down from a thing to a part fitted to it, by slot and
/// template, each before the ways to the parts fitted to that part.
fn ways_down(item: &trp_sim::items::inventory::Item, above: &mut Vec<(String, String)>, found: &mut Vec<Vec<(String, String)>>) {
    for (holder, held) in item.kind.slots.iter().zip(&item.slots) {
        let Some(held) = held else { continue };
        above.push((holder.name.clone(), held.kind.template.clone()));
        found.push(above.clone());
        ways_down(held, above, found);
        above.pop();
    }
}

/// Hangs on a model on the head each part fitted to the thing worn that
/// the model was not made with: the
/// model was made with the parts the run had as it began (`BodyParts::
/// load`), and the armor bench offers every part of the game. Read when
/// the thing is first worn so, by the character or by a dummy (whose
/// helmet searching may fit a part to), as the game makes a worn thing's model with
/// what is fitted to it (`ObjectsFactory.AttachMods`: body animator spec,
/// "A worn thing's parts and a helmet's hinge"), each hung by that rule
/// on the object of its slot's name below what it is fitted to, with its
/// own hinge if it carries one, on the body, the gear page's figure, the
/// corpse and the dummies; shown from then on as the rest are
/// (`show_head_parts`). A way down that hangs nothing (no object of its
/// slot's name, an armour plate, no model) is not asked for again.
#[allow(clippy::too_many_arguments)]
pub fn fit_head_parts(
    mut commands: Commands,
    (carried, parts): (Res<Carried>, Res<BodyParts>),
    mut game: ResMut<GameData>,
    models: Option<ResMut<HeadModels>>,
    mut converted: ResMut<Converted>,
    (mut meshes, mut images, mut materials): (ResMut<Assets<Mesh>>, ResMut<Assets<Image>>, ResMut<Assets<StandardMaterial>>),
    (mut figure, mut corpse, mut dummies): (Option<ResMut<crate::screens::figure_view::Figure>>, Option<ResMut<crate::screens::figure_view::CorpseSkeleton>>, Option<ResMut<crate::world::dummy::DummySkeletons>>),
    mut seen: Local<usize>,
) {
    let Some(mut models) = models else { return };
    // (A model read later, `worn_reads`, is asked of too.)
    if !carried.is_changed() && models.0.len() == *seen {
        return;
    }
    *seen = models.0.len();
    let game = &mut *game;
    for kept in &mut models.0 {
        let mut fitted = Vec::new();
        for worn in worn_everywhere(&carried, &kept.item) {
            ways_down(worn, &mut Vec::new(), &mut fitted);
        }
        for path in fitted {
            if kept.none.contains(&path) || kept.parts.iter().any(|part| part.path == path) {
                continue;
            }
            // What it is fitted to: the worn thing's model, or a part's
            // (fitted before it here: a way down comes after the way to
            // what it is on).
            let above = &path[..path.len() - 1];
            let parent = if above.is_empty() { Some(0) } else { kept.parts.iter().find(|part| part.path == above).map(|part| part.root) };
            let from = kept.model.nodes.len();
            let Some((part, hinge)) = parent.and_then(|parent| trp_data::character::dress::fit_part(&mut game.unity, &game.db, &mut kept.model, parent, &path)) else {
                kept.none.insert(path);
                continue;
            };
            kept.parts.push(part);
            // Its meshes' second forms, read with the reader that read it.
            if parts.fitting {
                let found = trp_data::character::dress::fittings(&game.unity, &kept.model);
                kept.fittings.extend(found.into_iter().filter(|(node, _)| *node >= from));
            }
            let dummy_skeletons: &mut [crate::screens::figure_view::Skeleton] = match dummies.as_mut() {
                Some(dummies) => &mut dummies.0,
                None => &mut [],
            };
            let (figure_skeleton, overall_skeleton) = match figure.as_mut() {
                Some(figure) => {
                    let (gear, overall) = figure.skeletons();
                    (Some(gear), overall)
                }
                None => (None, None),
            };
            let copies = Copies { figure: figure_skeleton, overall: overall_skeleton, corpse: corpse.as_mut().map(|corpse| &mut corpse.0), dummies: dummy_skeletons };
            let mut looks = converted_looks(&game.unity, &mut converted, &mut meshes, &mut images, &mut materials);
            let made = hang_head_model(&mut commands, &mut looks, kept, from, hinge.as_slice(), &Heads::default(), copies);
            info!("worn on the head, {}: its part {} fitted, its model read ({} objects, {} meshes)", kept.item, path.last().map_or("", |(_, template)| template.as_str()), kept.model.nodes.len() - from, made.len());
            if let Some(figure) = figure.as_mut() {
                let (meshes, materials): (Vec<_>, Vec<_>) = made.into_iter().unzip();
                figure.wait_for(&meshes, &materials);
            }
        }
    }
}
