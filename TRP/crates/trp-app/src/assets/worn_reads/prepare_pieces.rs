//! Worn pieces prepared from the install on a worker.

use super::*;

/// Reads a thing worn and makes it ready, away from the frame: as
/// `BodyParts::load` reads one, its model on the head with the parts in
/// `fitted` (those fitted to it on some body); with `fitting`, its meshes'
/// second forms; an armband with `armband` alone, each skin's mesh for
/// every upper body.
pub(super) fn prepare(unity: &mut UnityAssets, db: &trp_data::Database, template: &str, fitted: &HashSet<String>, fitting: bool, armband: bool) -> anyhow::Result<Prepared> {
    let bundle = db.items.get(template).and_then(|item| item.prefab_key()).unwrap_or(template).to_owned();
    let vestlike = db.items.get(template).is_some_and(|item| item.parent == crate::player::body_view::ARMOUR) && trp_data::character::dress::vestlike(unity, &bundle).unwrap_or(false);
    // Its levels of detail where they are read (`lod_view`), and each
    // level's second form.
    let levelled = crate::assets::lod_view::on();
    let mesh_transform = if levelled { trp_data::character::player::mesh_transform(unity).ok() } else { None };
    let skin_fittings: Vec<trp_data::character::dress::Fitting> = match (fitting, levelled) {
        (false, _) => Vec::new(),
        (true, false) => trp_data::character::dress::skin_fitting(unity, &bundle).ok().flatten().into_iter().collect(),
        (true, true) => trp_data::character::dress::skin_fittings(unity, &bundle).unwrap_or_default(),
    };
    let mut specs: Vec<(usize, Arc<crate::assets::lod_view::LodSpec>)> = Vec::new();
    // (Its cubes for the OVERALL figure's own surface: `figure_surface`.)
    let mut looks = PreparedLooks::default().with_figure_cubes();
    let mut pieces: Vec<(trp_data::unity::ObjRef, PreparedPiece)> = Vec::new();
    for under in crate::player::body_view::ALL_UNDER {
        match trp_data::character::dress::load(unity, &bundle, under)? {
            Worn::Skinned(fullest) => {
                let levels = match levelled {
                    true => trp_data::character::dress::load_levels(unity, &bundle, under)?.unwrap_or_else(|| vec![fullest]),
                    false => vec![fullest],
                };
                for part in levels {
                    // The same mesh as for a case before: that piece's too.
                    if let Some((_, piece)) = pieces.iter_mut().find(|(mesh, _)| *mesh == part.mesh) {
                        piece.over.push(under);
                        continue;
                    }
                    let (meshes, poses) = looks.mesh_with_poses(unity, part.mesh, false)?;
                    let materials = (0..meshes.len()).map(|slot| looks.material(unity, part.materials.get(slot).copied().flatten())).collect();
                    let second = match skin_fittings.iter().find(|fitting| fitting.base == part.mesh) {
                        Some(fitting) => {
                            let (custom, custom_poses) = looks.mesh_with_poses(unity, fitting.custom, false)?;
                            // Each first submesh's second, as `body_view::second_form` pairs them.
                            let seconds = (0..meshes.len()).filter_map(|slot| custom.get(slot.min(custom.len().saturating_sub(1))).copied()).collect();
                            Some((fitting.kind, seconds, custom_poses))
                        }
                        None => None,
                    };
                    let lod = part.lod.as_ref().zip(mesh_transform).map(|(lod, (place, turn))| {
                        let key = Arc::as_ptr(&lod.group) as usize;
                        let spec = match specs.iter().find(|(made, _)| *made == key) {
                            Some((_, spec)) => spec.clone(),
                            None => {
                                let spec = crate::assets::lod_view::LodSpec::through(&lod.group, place, turn);
                                specs.push((key, spec.clone()));
                                spec
                            }
                        };
                        (spec, lod.levels)
                    });
                    pieces.push((part.mesh, PreparedPiece { bone_paths: part.bone_paths, meshes, materials, poses, over: vec![under], second, band: None, lod }));
                }
            }
            Worn::Band(band) => {
                anyhow::ensure!(armband, "an armband (ARMBAND is off)");
                for (at, skin) in band.skins.into_iter().enumerate() {
                    // Each mesh of its sets once, the first set's first.
                    let mut known: Vec<trp_data::unity::ObjRef> = Vec::new();
                    let (mut sets, mut forms) = (Vec::new(), Vec::new());
                    for (id, meshes) in &band.sets {
                        let Some(&mesh) = meshes.get(at) else { continue };
                        let place = match known.iter().position(|of| *of == mesh) {
                            Some(place) => place,
                            None => {
                                forms.push(looks.mesh_with_poses(unity, mesh, false)?);
                                known.push(mesh);
                                known.len() - 1
                            }
                        };
                        sets.push((id.clone(), place));
                    }
                    let Some((meshes, poses)) = forms.first().cloned() else { continue };
                    let materials = (0..meshes.len()).map(|slot| looks.material(unity, skin.materials.get(slot).copied().flatten())).collect();
                    pieces.push((skin.mesh, PreparedPiece { bone_paths: skin.bone_paths, meshes, materials, poses, over: Vec::new(), second: None, band: Some((sets, forms)), lod: None }));
                }
                break;
            }
            Worn::Rigid(model) => {
                let (model, parts, hinges) = trp_data::character::dress::head_model(unity, db, template, model, &|part| fitted.contains(part));
                let fittings = if fitting { trp_data::character::dress::fittings(unity, &model) } else { Vec::new() };
                // Hung on the head and drawn as it lies: a mesh with bones
                // and none to move them stops the renderer.
                let mut nodes = Vec::with_capacity(model.nodes.len());
                let mut seconds = vec![Vec::new(); model.nodes.len()];
                for (index, node) in model.nodes.iter().enumerate() {
                    // A coarser level's too, where the levels are drawn by.
                    let Some(renderer) = node.renderer.as_ref().or(node.lower.as_ref().filter(|_| levelled)) else {
                        nodes.push(Vec::new());
                        continue;
                    };
                    // Its second form's meshes, where it has one, unskinned
                    // as the first's.
                    if let Some((_, fitting)) = fittings.iter().find(|(at, _)| *at == index) {
                        match looks.mesh_with_poses(unity, fitting.custom, model.mirrored(index)) {
                            Ok((custom, _)) => {
                                custom.iter().for_each(|mesh| looks.unskin(*mesh));
                                seconds[index] = custom;
                            }
                            Err(e) => warn!("second mesh on {}: {e:#}", node.name),
                        }
                    }
                    let drawn = match looks.mesh_with_poses(unity, renderer.mesh, model.mirrored(index)) {
                        Ok((meshes, _)) => meshes
                            .into_iter()
                            .enumerate()
                            .map(|(slot, mesh)| {
                                looks.unskin(mesh);
                                (mesh, looks.material(unity, renderer.materials.get(slot).copied().flatten()))
                            })
                            .collect(),
                        Err(e) => {
                            warn!("mesh on {}: {e:#}", node.name);
                            Vec::new()
                        }
                    };
                    nodes.push(drawn);
                }
                let model = HeadModel { item: template.to_owned(), model, parts, hinges, fittings };
                return Ok(Prepared { shape: PreparedShape::Rigid(model, nodes, seconds), vestlike: false, looks });
            }
        }
    }
    Ok(Prepared { shape: PreparedShape::Skinned(pieces.into_iter().map(|(_, piece)| piece).collect()), vestlike, looks })
}
