//! Reading a sight part from its prefab: slots, modes and figures.

use super::*;

/// The items fitted to a weapon in the order the game walks them
/// (`CompoundItem.AllSlots`: an item's slots in its template's order, then
/// each of them in turn the same way).
pub fn all_slots_order<'a>(db: &Database, preset: &'a Preset) -> Vec<&'a PresetItem> {
    fn below<'a>(db: &Database, preset: &'a Preset, parent: &PresetItem, out: &mut Vec<&'a PresetItem>) {
        let slots: Vec<&str> = db
            .items
            .get(&parent.template)
            .and_then(|t| t.props.get("Slots"))
            .and_then(Json::as_array)
            .map(|slots| slots.iter().filter_map(|s| s.get("_name")?.as_str()).collect())
            .unwrap_or_default();
        let kids: Vec<&'a PresetItem> = slots
            .iter()
            .filter_map(|slot| preset.items.iter().find(|i| i.parent.as_deref() == Some(parent.id.as_str()) && i.slot.as_deref() == Some(*slot)))
            .collect();
        out.extend(kids.iter().copied());
        for kid in kids {
            below(db, preset, kid, out);
        }
    }
    let mut out = Vec::new();
    if let Some(root) = preset.items.iter().find(|i| i.id == preset.root) {
        below(db, preset, root, &mut out);
    }
    out
}

/// A prefab's own scope where one of its objects carries one (a
/// `ScopePrefabCache`, as the handheld rangefinder's `item` does:
/// rangefinder spec, "The optic"), as a sight part of the template: its
/// modes, its lenses, one scope of one mode with no zoom, sensitivity or
/// zeroing. Not a sight to aim by (no `mod_aim_camera`): its optic is
/// switched by its controller (`trp_sim::sights::Sights::force_optic`).
pub fn prefab_part(assets: &Assets, hierarchy: &Hierarchy, template: &str) -> Result<Option<SightPart>> {
    let found = hierarchy.nodes.iter().enumerate().find_map(|(node, n)| n.scripts.iter().zip(&n.script_objects).find(|(name, _)| name.as_str() == "ScopePrefabCache").map(|(_, &obj)| (node, obj)));
    let Some((root, cache)) = found else { return Ok(None) };
    let Some(slot_bone) = hierarchy.nodes[root].parent else { return Ok(None) };
    let modes = prefab_modes(assets, hierarchy, root, cache)?;
    let lenses = lens_nodes(assets, hierarchy, &own_nodes(hierarchy, root, &[]));
    Ok(Some(SightPart {
        item: String::new(),
        template: template.to_owned(),
        root,
        slot_bone,
        iron: false,
        on_sight: false,
        scopes: vec![ScopeFigures { modes: modes.len().max(1), zooms: Vec::new(), sensitivity: Vec::new(), calibration: Vec::new() }],
        adjustable: None,
        custom_aim_plane: String::new(),
        modes,
        has_prefab_cache: true,
        zoom_curve: None,
        reticle_curve: None,
        eye_guard_shift: 0.0,
        lenses,
        angle_by_range: None,
        zoom_ring: None,
    }))
}

/// The sight parts of a build, in the order the game walks its items, and
/// the sights that fold. `roots` gives each placed item's root in the
/// assembled tree.
pub fn read(assets: &Assets, db: &Database, preset: &Preset, roots: &HashMap<&str, usize>, hierarchy: &Hierarchy) -> Result<(Vec<SightPart>, Vec<FoldingSight>)> {
    let mut parts = Vec::new();
    let is_sight = |template: &str| db.items.get(template).is_some_and(|t| db.is_a(t, "Sights"));
    for item in all_slots_order(db, preset) {
        let Some(template) = db.items.get(&item.template).filter(|t| db.is_a(t, "Sights")) else { continue };
        let Some(&root) = roots.get(item.id.as_str()) else { continue };
        let Some(slot_bone) = hierarchy.nodes[root].parent else { continue };
        let parent_template = item.parent.as_deref().and_then(|p| preset.items.iter().find(|i| i.id == p)).map(|p| p.template.as_str());
        // The eyecups below it (`GetItemComponentsInChildren<EyeGuardComponent>`):
        // the first one's shift.
        let mut below = vec![item.id.as_str()];
        let mut eye_guard_shift = 0.0;
        while let Some(at) = below.pop() {
            for kid in preset.items.iter().filter(|i| i.parent.as_deref() == Some(at)) {
                if let Some(shift) = db.items.get(&kid.template).and_then(|t| t.prop_f32("ShiftsAimCamera")) {
                    if eye_guard_shift == 0.0 {
                        eye_guard_shift = shift;
                    }
                }
                below.push(kid.id.as_str());
            }
        }
        let props = &template.props;
        let nested = |name: &str| -> Vec<Vec<f32>> {
            props
                .get(name)
                .and_then(Json::as_array)
                .map(|scopes| scopes.iter().map(|s| s.as_array().map(|m| m.iter().filter_map(Json::as_f64).map(|v| v as f32).collect()).unwrap_or_default()).collect())
                .unwrap_or_default()
        };
        let (zooms, sensitivity, calibration) = (nested("Zooms"), nested("AimSensitivity"), nested("CalibrationDistances"));
        let counts: Vec<usize> = props.get("ModesCount").and_then(Json::as_array).map(|c| c.iter().filter_map(Json::as_u64).map(|n| n as usize).collect()).unwrap_or_default();
        let scopes_count = props.get("ScopesCount").and_then(Json::as_u64).unwrap_or(counts.len() as u64) as usize;
        let scopes = (0..scopes_count)
            .map(|s| ScopeFigures {
                modes: counts.get(s).copied().unwrap_or(1),
                zooms: zooms.get(s).cloned().unwrap_or_default(),
                sensitivity: sensitivity.get(s).cloned().unwrap_or_default(),
                calibration: calibration.get(s).map(|c| c.iter().map(|&d| d as i32).collect()).unwrap_or_default(),
            })
            .collect();
        let flag = |name: &str| props.get(name).and_then(Json::as_bool).unwrap_or(false);
        let adjustable = flag("IsAdjustableOptic").then(|| {
            let fov = props.get("MinMaxFov");
            let axis = |a: &str| fov.and_then(|f| f.get(a)).and_then(Json::as_f64).unwrap_or(0.0) as f32;
            AdjustableOptic {
                fov: [axis("x"), axis("y")],
                speed: axis("z"),
                zoom_sensitivity: template.prop_f32("ZoomSensitivity").unwrap_or(0.0),
                sensitivity: [template.prop_f32("AdjustableOpticSensitivity").unwrap_or(1.0), template.prop_f32("AdjustableOpticSensitivityMax").unwrap_or(1.0)],
            }
        });
        let root_scripts = |class: &str| -> Option<Value> {
            let n = &hierarchy.nodes[root];
            n.scripts.iter().zip(&n.script_objects).find(|(name, _)| name.as_str() == class).and_then(|(_, &obj)| assets.read(obj).ok())
        };
        let cache = hierarchy.nodes[root].scripts.iter().zip(&hierarchy.nodes[root].script_objects).find(|(name, _)| name.as_str() == "ScopePrefabCache").map(|(_, &obj)| obj);
        let mut modes = match cache {
            Some(obj) => prefab_modes(assets, hierarchy, root, obj)?,
            None => Vec::new(),
        };
        let angle_by_range = cache.and_then(|obj| angle_by_range(assets, hierarchy, root, obj));
        // A variable scope's camera data is its root's.
        let smooth_near = root_scripts("ScopeSmoothCameraData").and_then(|data| data.get("NearClipPlane").and_then(Value::as_f32));
        let smooth_factor = root_scripts("ScopeSmoothCameraData").and_then(|data| lod_bias_factor(&data));
        for optic in modes.iter_mut().filter_map(|mode| mode.optic.as_mut()) {
            optic.near = optic.near.or(smooth_near);
            optic.lod_bias_factor = optic.lod_bias_factor.or(smooth_factor);
        }
        // Its own nodes, not into another sight part hung on it.
        let others: Vec<usize> = roots.values().copied().filter(|&r| r != root).collect();
        let own = own_nodes(hierarchy, root, &others);
        let lenses = lens_nodes(assets, hierarchy, &own);
        let handled = hierarchy.nodes[root].scripts.iter().any(|s| s == "ScopeZoomHandler");
        let smooth = root_scripts("ScopeSmoothCameraData").filter(|_| handled);
        let zoom_curve = smooth.as_ref().and_then(|data| data.get("FieldOfViewCurve").and_then(|c| crate::character::player::curve(c).ok()));
        let reticle_curve = smooth.as_ref().and_then(|data| data.get("ReticleBlendCurve").and_then(|c| crate::character::player::curve(c).ok()));
        let zoom_ring = zoom_ring(assets, hierarchy, root);
        parts.push(SightPart {
            item: item.id.clone(),
            template: item.template.clone(),
            root,
            slot_bone,
            iron: db.is_a(template, "IronSight"),
            on_sight: parent_template.is_some_and(is_sight),
            scopes,
            adjustable,
            custom_aim_plane: props.get("CustomAimPlane").and_then(Json::as_str).unwrap_or("").to_owned(),
            modes,
            has_prefab_cache: cache.is_some(),
            zoom_curve,
            reticle_curve,
            eye_guard_shift,
            lenses,
            angle_by_range,
            zoom_ring,
        });
    }
    let mut folding = Vec::new();
    for (node, n) in hierarchy.nodes.iter().enumerate() {
        for (name, &obj) in n.scripts.iter().zip(&n.script_objects) {
            if name == "AutoFoldableSight" {
                let mode = assets.read(obj).ok().and_then(|v| v.get("Mode").and_then(Value::as_i64)).unwrap_or(0);
                folding.push(FoldingSight { node, mode_on: mode == 0 });
            }
        }
    }
    Ok((parts, folding))
}

/// The modes a part's `ScopePrefabCache` lists, each resolved to the part's
/// objects.
fn prefab_modes(assets: &Assets, hierarchy: &Hierarchy, root: usize, cache: ObjRef) -> Result<Vec<PrefabMode>> {
    let value = assets.read(cache)?;
    let in_part = part_nodes(hierarchy, root);
    let resolve = |v: Option<&Value>| v.and_then(Value::as_pptr).filter(|p| !p.is_null()).and_then(|p| assets.resolve_loaded(cache.file, p));
    // A script's object, a transform's, or a game object's, as the node of
    // the part that is it or carries it.
    let by_script = |obj: ObjRef| in_part.iter().copied().find(|&i| hierarchy.nodes[i].script_objects.contains(&obj));
    let by_transform = |obj: ObjRef| in_part.iter().copied().find(|&i| hierarchy.nodes[i].transform == obj);
    let by_game_object = |obj: ObjRef| {
        let go = assets.read(obj).ok()?;
        crate::unity::extract::components(assets, obj, &go).into_iter().find(|&c| assets.class_id(c) == Some(class::TRANSFORM)).and_then(by_transform)
    };
    let mut modes = Vec::new();
    for info in value.get("_scopeModeInfos").and_then(Value::as_array).unwrap_or(&[]) {
        let optic = match resolve(info.get("OpticSight")) {
            Some(obj) => {
                let sight = assets.read(obj)?;
                let node = by_script(obj);
                let bone = resolve(sight.get("ScopeTransform")).and_then(by_transform);
                let distance_to_camera = sight.get("DistanceToCamera").and_then(Value::as_f32).unwrap_or(0.0);
                // The camera's data: the first `ScopeCameraData` below the
                // sight's object (`GetComponentInChildren`, the inactive
                // too), depth first.
                let camera = node.and_then(|node| {
                    let below = part_nodes(hierarchy, node);
                    below.into_iter().find_map(|i| {
                        let n = &hierarchy.nodes[i];
                        n.scripts.iter().zip(&n.script_objects).find(|(name, _)| name.as_str() == "ScopeCameraData").map(|(_, &obj)| (i, obj))
                    })
                });
                let camera_data = camera.and_then(|(_, obj)| assets.read(obj).ok());
                let field_of_view = camera_data.as_ref().and_then(|data| data.get("FieldOfView").and_then(Value::as_f32));
                let near = camera_data.as_ref().and_then(|data| data.get("NearClipPlane").and_then(Value::as_f32));
                let lod_bias_factor = camera_data.as_ref().and_then(lod_bias_factor);
                // Its lens: the renderer's object.
                let lens = resolve(sight.get("LensRenderer")).and_then(|renderer| assets.read(renderer).ok().and_then(|r| r.get("m_GameObject").and_then(Value::as_pptr)).and_then(|p| assets.resolve_loaded(renderer.file, p))).and_then(by_game_object);
                // Where the optic camera stands: the object of the sight's
                // `ScopeData`, which the camera data sits beside.
                let scope_data_obj = resolve(sight.get("ScopeData"));
                let stands = scope_data_obj.and_then(by_script).or(camera.map(|(i, _)| i));
                let scope_data = scope_data_obj.and_then(|obj| assets.read(obj).ok());
                // Its effects: the chromatic aberration's shift, where on.
                let effects = scope_data.as_ref().and_then(|data| resolve(data.get("PostEffectsData"))).and_then(|obj| assets.read(obj).ok());
                let aberration = effects
                    .filter(|e| e.get("ChromaticAberration").and_then(Value::as_bool).unwrap_or(false))
                    .and_then(|e| e.get("ChromaticAberrationShift").and_then(Value::as_f32))
                    .unwrap_or(0.0);
                let reticle = scope_data.as_ref().and_then(|data| resolve(data.get("Reticle"))).and_then(|obj| read_reticle(assets, obj).ok().flatten()).map(std::sync::Arc::new);
                let thermal_at_45 = sight.get("IsThermalSightAvailableAt45Degrees").and_then(Value::as_bool).unwrap_or(false);
                node.map(|node| OpticMode { node, bone, distance_to_camera, field_of_view, camera: stands, near, lod_bias_factor, lens, aberration, reticle, thermal_at_45 })
            }
            None => None,
        };
        modes.push(PrefabMode {
            node: resolve(info.get("ModeGameObject")).and_then(by_game_object),
            optic,
            collimator: resolve(info.get("CollimatorSight")).and_then(by_script),
            ignore_optics_for_camera_plane: info.get("IgnoreOpticsForCameraPlane").and_then(Value::as_bool).unwrap_or(false),
        });
    }
    Ok(modes)
}
