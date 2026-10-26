//! The sights fitted to an assembled weapon (optics spec, "The sights a
//! weapon has", "A scope's parts"): each part whose item is a sight
//! (`SightComponent`), with its item's figures and what its prefab says of
//! its modes (`ScopePrefabCache`, `OpticSight`, `ScopeCameraData`,
//! `ScopeSmoothCameraData`), and the backup sights that fold
//! (`AutoFoldableSight`).

use std::collections::HashMap;

use anyhow::Result;
use serde_json::Value as Json;

use crate::curve::Curve;
use crate::database::{Database, Preset, PresetItem};
use crate::unity::extract::Hierarchy;
use crate::unity::{class, Assets, ObjRef, Value};

/// One scope of a sight item (the template's lists, a scope each): how
/// many modes it has, and for each mode its magnification (`Zooms`, which
/// only the HUD shows; a variable scope's one mode lists its range), its
/// aiming sensitivity (`AimSensitivity`) and its zeroing distances
/// (`CalibrationDistances`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ScopeFigures {
    pub modes: usize,
    pub zooms: Vec<f32>,
    pub sensitivity: Vec<f32>,
    pub calibration: Vec<i32>,
}

/// A variable scope's figures (`IsAdjustableOptic`): its fields of view
/// widest and narrowest and the zoom's speed (`MinMaxFov`, which the game
/// takes over its prefab's), how much a turn of the wheel adds to the
/// zoom's velocity (`ZoomSensitivity`), and the aiming sensitivity at the
/// widest and at the narrowest (`AdjustableOpticSensitivity`, `..Max`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AdjustableOptic {
    pub fov: [f32; 2],
    pub speed: f32,
    pub zoom_sensitivity: f32,
    pub sensitivity: [f32; 2],
}

/// One mode of a sight's prefab (`ScopePrefabCache.ScopeModeInfo`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PrefabMode {
    /// The mode's own object, on while the mode is.
    pub node: Option<usize>,
    pub optic: Option<OpticMode>,
    /// The object of its `CollimatorSight`, a red dot's or a holographic's.
    pub collimator: Option<usize>,
    pub ignore_optics_for_camera_plane: bool,
}

/// A mode's `OpticSight`: its object; the aim bone it names
/// (`ScopeTransform`); how far behind that bone the eye sits
/// (`DistanceToCamera`); a fixed scope's field of view for the mode (the
/// `ScopeCameraData` below it), and the object that carries that data,
/// which is where the optic camera stands.
#[derive(Debug, Clone, PartialEq)]
pub struct OpticMode {
    pub node: usize,
    pub bone: Option<usize>,
    pub distance_to_camera: f32,
    pub field_of_view: Option<f32>,
    pub camera: Option<usize>,
    /// The optic camera's near plane for the mode (its camera data's;
    /// a variable scope's, its root's), where it gives one.
    pub near: Option<f32>,
    /// The object of its lens (`LensRenderer`), whose material is dimmed
    /// while the mode is not live.
    pub lens: Option<usize>,
    /// Its picture's chromatic aberration (`ScopeEffectsData`:
    /// `ChromaticAberrationShift` where `ChromaticAberration` is on).
    pub aberration: f32,
    /// A variable scope's reticle mesh, drawn into the picture
    /// (`ScopeData.Reticle`, a `ScopeReticle`).
    pub reticle: Option<std::sync::Arc<ScopeReticle>>,
    /// The thermal scope whose camera loses its roll as it is turned to
    /// the zeroing point (`IsThermalSightAvailableAt45Degrees`: the T12's).
    pub thermal_at_45: bool,
}

/// A part its prefab turns by the zeroing distance (`ScopePrefabCache.
/// CanChangeAngleByDistance`): the object turned (`WeaponScopeAxis`) and its
/// table of distances and angles about x, degrees (`AngleByRange`).
#[derive(Debug, Clone, PartialEq)]
pub struct AngleByRange {
    pub axis: usize,
    pub table: Vec<(f32, f32)>,
}

/// A variable scope's ring turned by its zoom (`ScopeZoomHandler.method_11`):
/// the ring (`ScopeSwitcher`), its own turns at the widest and the narrowest
/// view (`StartPivot`, `EndPivot`, its siblings, so their local turns stand
/// for the world ones the game blends), and whether it turns the long way
/// where that is the way the pivots stand (`InverseRotation`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ZoomRing {
    pub node: usize,
    pub start: [f32; 4],
    pub end: [f32; 4],
    pub inverse: bool,
}

/// A reticle drawn into the optic camera's picture (`ScopeReticle`,
/// `OpticRetrice`; optics spec, "The lens"): its mesh's first part (the
/// vertices, their colours, the triangles, the first blend shape's full
/// move), its place and turn (Euler degrees) about the optic camera, its
/// stored scale, and its material's `_Color`.
#[derive(Debug, Clone, PartialEq)]
pub struct ScopeReticle {
    pub positions: Vec<[f32; 3]>,
    pub colors: Vec<[f32; 4]>,
    pub triangles: Vec<u32>,
    pub shape: Option<Vec<[f32; 3]>>,
    pub position: [f32; 3],
    pub rotation: [f32; 3],
    pub scale: f32,
    pub color: [f32; 4],
}

/// A piece of a sight part drawn by one of the sights' own shaders, by its
/// material's shader (optics spec, "The lens"): its object, the material
/// slot (the submesh: the Tango's glass is its body's second), its
/// material, and what kind it is.
#[derive(Debug, Clone)]
pub struct LensNode {
    pub node: usize,
    pub slot: usize,
    pub material: ObjRef,
    pub kind: LensKind,
    /// Its material's pictures that its shader reads (`_MarkTex`,
    /// `_MaskTex`, `_MaskTex2`; a collimator's `_MarkTex`, `_NoiseTex`,
    /// `_FadeTex`), read with the build: a host draws from them.
    pub pictures: Vec<(String, std::sync::Arc<crate::unity::extract::TextureData>)>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LensKind {
    /// A scope's lens (`CW FX/OpticSight`).
    Optic(OpticLens),
    /// A red dot's or a holographic sight's (`CW FX/Collimator`).
    Collimator(CollimatorLens),
    /// The front glass: a scope's (`Custom/OpticGlass`), or a red dot's
    /// window (`EFT/CollimatorGlass`, transparent, its queue 3000: the
    /// PK-06's).
    Glass,
    /// The back of the lens (`CW FX/BackLens`).
    Back,
}

/// A scope's lens: its material's figures (`_Scales`, `_Shifts`,
/// `_ShiftDirection`, `_NormalHideness`, `_MarkLightness`, read as the
/// game's Y-up left-handed vectors) and its mesh's two rings' radii (the
/// inner ring's, whose uv radius is 0.4725, and the rim's), metres.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OpticLens {
    pub scales: [f32; 2],
    pub shifts: [f32; 2],
    pub shift_direction: [f32; 3],
    pub normal_hideness: f32,
    pub mark_lightness: f32,
    pub inner_radius: f32,
    pub outer_radius: f32,
}

/// A collimator's lens: its mark's colour, `_HDR`, `_MarkScale` and
/// `_MarkShift` (the game's axes).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CollimatorLens {
    pub color: [f32; 4],
    pub hdr: f32,
    pub mark_scale: f32,
    pub mark_shift: [f32; 3],
}

/// One part of the build that is a sight.
#[derive(Debug, Clone)]
pub struct SightPart {
    /// The preset's id of the item, and its template's.
    pub item: String,
    pub template: String,
    /// The part's own root, and the bone of the slot it hangs in.
    pub root: usize,
    pub slot_bone: usize,
    /// An iron sight (`IronSight`), and whether it hangs on a part that is
    /// itself a sight (a backup sight on a scope's rail).
    pub iron: bool,
    pub on_sight: bool,
    pub scopes: Vec<ScopeFigures>,
    pub adjustable: Option<AdjustableOptic>,
    /// The aim plane of the weapon's prefab the sight names, if any (the
    /// T12 thermal's).
    pub custom_aim_plane: String,
    /// Its prefab's modes; none where its root has no `ScopePrefabCache`.
    pub modes: Vec<PrefabMode>,
    pub has_prefab_cache: bool,
    /// A variable scope's `ScopeZoomHandler`, on its root: its curve of the
    /// wheel's target (`ScopeSmoothCameraData.FieldOfViewCurve`). (Every
    /// sight's root is given the `SightModVisualControllers` that switch
    /// its modes when it is made: `WeaponModPoolObject`.)
    pub zoom_curve: Option<Curve>,
    /// And its reticle's blend by the zoom (`ReticleBlendCurve`).
    pub reticle_curve: Option<Curve>,
    /// How far an eyecup on it moves the eye back (`EyeGuardComponent.
    /// ShiftsAimCamera` of a part below it), in metres.
    pub eye_guard_shift: f32,
    /// Its pieces drawn by the sights' own shaders: lenses, collimators,
    /// the glass and the back of the lens (not those of a part hung on it).
    pub lenses: Vec<LensNode>,
    /// The part its prefab turns by the zeroing distance, where it does.
    pub angle_by_range: Option<AngleByRange>,
    /// A variable scope's ring its zoom turns, where its prefab names one.
    pub zoom_ring: Option<ZoomRing>,
}

/// A sight that folds out of the way (`AutoFoldableSight`): its object,
/// and whether it is shown when no sight part looks straight ahead (its
/// `Mode` `On`; `Off` shows it only beside one that does).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FoldingSight {
    pub node: usize,
    pub mode_on: bool,
}

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
        for optic in modes.iter_mut().filter_map(|mode| mode.optic.as_mut()) {
            optic.near = optic.near.or(smooth_near);
        }
        // Its own nodes, not into another sight part hung on it.
        let others: Vec<usize> = roots.values().copied().filter(|&r| r != root).collect();
        let own = own_nodes(hierarchy, root, &others);
        let lenses = lens_nodes(assets, hierarchy, &own);
        let handled = hierarchy.nodes[root].scripts.iter().any(|s| s == "ScopeZoomHandler");
        let smooth = root_scripts("ScopeSmoothCameraData").filter(|_| handled);
        let zoom_curve = smooth.as_ref().and_then(|data| data.get("FieldOfViewCurve").and_then(|c| crate::player::curve(c).ok()));
        let reticle_curve = smooth.as_ref().and_then(|data| data.get("ReticleBlendCurve").and_then(|c| crate::player::curve(c).ok()));
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
                node.map(|node| OpticMode { node, bone, distance_to_camera, field_of_view, camera: stands, near, lens, aberration, reticle, thermal_at_45 })
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

/// What a part's `ScopePrefabCache` turns by the zeroing distance, where its
/// `CanChangeAngleByDistance` is on and it names its object.
fn angle_by_range(assets: &Assets, hierarchy: &Hierarchy, root: usize, cache: ObjRef) -> Option<AngleByRange> {
    let value = assets.read(cache).ok()?;
    if !value.get("CanChangeAngleByDistance").and_then(Value::as_bool).unwrap_or(false) {
        return None;
    }
    let transform = value.get("WeaponScopeAxis").and_then(Value::as_pptr).filter(|p| !p.is_null()).and_then(|p| assets.resolve_loaded(cache.file, p))?;
    let axis = part_nodes(hierarchy, root).into_iter().find(|&i| hierarchy.nodes[i].transform == transform)?;
    let table = value
        .get("AngleByRange")
        .and_then(Value::as_array)
        .unwrap_or(&[])
        .iter()
        .filter_map(|entry| Some((entry.get("Distance")?.as_f32()?, entry.get("Angle")?.as_f32()?)))
        .collect();
    Some(AngleByRange { axis, table })
}

/// What a part's `ScopeZoomHandler` turns, where it names its ring and both
/// pivots and they hang from one object (the Nightforce NXS's names none).
fn zoom_ring(assets: &Assets, hierarchy: &Hierarchy, root: usize) -> Option<ZoomRing> {
    let n = &hierarchy.nodes[root];
    let obj = n.scripts.iter().zip(&n.script_objects).find(|(name, _)| name.as_str() == "ScopeZoomHandler").map(|(_, &obj)| obj)?;
    let value = assets.read(obj).ok()?;
    let own = part_nodes(hierarchy, root);
    let find = |name: &str| -> Option<usize> {
        let transform = value.get(name).and_then(Value::as_pptr).filter(|p| !p.is_null()).and_then(|p| assets.resolve_loaded(obj.file, p))?;
        own.iter().copied().find(|&i| hierarchy.nodes[i].transform == transform)
    };
    let (ring, start, end) = (find("ScopeSwitcher")?, find("StartPivot")?, find("EndPivot")?);
    let parent = hierarchy.nodes[ring].parent;
    if hierarchy.nodes[start].parent != parent || hierarchy.nodes[end].parent != parent {
        return None;
    }
    let inverse = value.get("InverseRotation").and_then(Value::as_bool).unwrap_or(false);
    Some(ZoomRing { node: ring, start: hierarchy.nodes[start].rotation, end: hierarchy.nodes[end].rotation, inverse })
}

/// A `ScopeReticle` and what it names.
fn read_reticle(assets: &Assets, obj: ObjRef) -> Result<Option<ScopeReticle>> {
    let value = assets.read(obj)?;
    let resolve = |v: Option<&Value>| v.and_then(Value::as_pptr).filter(|p| !p.is_null()).and_then(|p| assets.resolve_loaded(obj.file, p));
    let Some(mesh_obj) = resolve(value.get("Mesh")) else { return Ok(None) };
    let mesh = crate::unity::extract::read_mesh(assets, mesh_obj)?;
    let color = resolve(value.get("Material")).and_then(|m| crate::unity::extract::read_material(assets, m).ok()).and_then(|m| m.colors.get("_Color").copied()).unwrap_or([1.0; 4]);
    let vec3 = |name: &str| value.get(name).and_then(Value::as_vec3).unwrap_or([0.0; 3]);
    Ok(Some(ScopeReticle {
        colors: if mesh.colors.len() == mesh.positions.len() { mesh.colors.clone() } else { vec![[1.0; 4]; mesh.positions.len()] },
        triangles: mesh.submeshes.first().cloned().unwrap_or_default(),
        shape: mesh.shape.clone(),
        positions: mesh.positions,
        position: vec3("Position"),
        rotation: vec3("Rotation"),
        scale: value.get("Scale").and_then(Value::as_f32).unwrap_or(1.0),
        color,
    }))
}

/// A part's own nodes: its root and all below it, depth first, but not into
/// another part's root.
fn own_nodes(hierarchy: &Hierarchy, root: usize, others: &[usize]) -> Vec<usize> {
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(at) = stack.pop() {
        out.push(at);
        let kids: Vec<usize> = (0..hierarchy.nodes.len()).filter(|&i| hierarchy.nodes[i].parent == Some(at) && !others.contains(&i)).collect();
        stack.extend(kids.into_iter().rev());
    }
    out
}

/// What a material's saved properties say it is drawn by: the shaders'
/// own properties, each kept by one of the four (`PROVISIONAL`: by its
/// properties, not by its shader's name, which is in the 378 MB shaders
/// bundle the loader opens only when it must; a material keeps the
/// properties of a shader it once had, so a lens's are looked for first).
pub fn lens_shader(data: &crate::unity::extract::MaterialData) -> Option<&'static str> {
    if data.textures.contains_key("_MaskTex2") && data.colors.contains_key("_Scales") && data.colors.contains_key("_Shifts") {
        Some("CW FX/OpticSight")
    } else if data.colors.contains_key("_MarkShift") && data.floats.contains_key("_HDR") {
        Some("CW FX/Collimator")
    } else if data.floats.contains_key("_SwitchToSightMultiplier") {
        Some("Custom/OpticGlass")
    } else if data.colors.contains_key("_Offset") && !data.textures.contains_key("_MainTex") {
        Some("CW FX/BackLens")
    } else if ["_GlobalReflectionStrength", "_FresPow", "_FresAdd", "_Specular"].iter().all(|name| data.floats.contains_key(*name)) && !data.textures.contains_key("_MainTex") {
        // A red dot's window: the reflective glass's figures and no picture
        // of its own (`EFT/Glass` keeps the same with a `_MainTex`).
        Some("EFT/CollimatorGlass")
    } else {
        None
    }
}

/// The nodes among `nodes` drawn by the sights' own shaders (`lens_shader`),
/// with what their material and mesh give.
fn lens_nodes(assets: &Assets, hierarchy: &Hierarchy, nodes: &[usize]) -> Vec<LensNode> {
    let mut out = Vec::new();
    let slots = nodes.iter().filter_map(|&node| Some((node, hierarchy.nodes[node].renderer.as_ref()?))).flat_map(|(node, renderer)| renderer.materials.iter().enumerate().filter_map(move |(slot, m)| Some((node, renderer, slot, (*m)?))));
    for (node, renderer, slot, material) in slots {
        let Ok(data) = crate::unity::extract::read_material(assets, material) else { continue };
        let Some(name) = lens_shader(&data) else { continue };
        let color = |name: &str| data.colors.get(name).copied().unwrap_or([0.0; 4]);
        let float = |name: &str, or: f32| data.floats.get(name).copied().unwrap_or(or);
        let kind = match name {
            "CW FX/OpticSight" => {
                // The two rings' radii from the mesh: the rim is the
                // farthest vertex, the inner ring the farthest whose uv lies
                // within the rim's.
                let Ok(mesh) = crate::unity::extract::read_mesh(assets, renderer.mesh) else { continue };
                let radius = |p: &[f32; 3]| (p[0] * p[0] + p[2] * p[2]).sqrt();
                let outer = mesh.positions.iter().map(radius).fold(0.0f32, f32::max);
                let inner = mesh.positions.iter().zip(&mesh.uv0).filter(|(_, uv)| ((uv[0] - 0.5).powi(2) + (uv[1] - 0.5).powi(2)).sqrt() < 0.49).map(|(p, _)| radius(p)).fold(0.0f32, f32::max);
                let (scales, shifts, way) = (color("_Scales"), color("_Shifts"), color("_ShiftDirection"));
                LensKind::Optic(OpticLens {
                    scales: [scales[0], scales[1]],
                    shifts: [shifts[0], shifts[1]],
                    shift_direction: [way[0], way[1], way[2]],
                    normal_hideness: float("_NormalHideness", 1.0),
                    mark_lightness: float("_MarkLightness", 0.0),
                    inner_radius: inner,
                    outer_radius: outer,
                })
            }
            "CW FX/Collimator" => {
                let shift = color("_MarkShift");
                LensKind::Collimator(CollimatorLens { color: data.colors.get("_Color").copied().unwrap_or([1.0; 4]), hdr: float("_HDR", 1.0), mark_scale: float("_MarkScale", 1.0), mark_shift: [shift[0], shift[1], shift[2]] })
            }
            "Custom/OpticGlass" | "EFT/CollimatorGlass" => LensKind::Glass,
            "CW FX/BackLens" => LensKind::Back,
            _ => continue,
        };
        let pictures = ["_MarkTex", "_MaskTex", "_MaskTex2", "_NoiseTex", "_FadeTex"]
            .into_iter()
            .filter_map(|name| {
                let texture = crate::unity::extract::read_texture(assets, *data.textures.get(name)?).ok()?;
                Some((name.to_owned(), std::sync::Arc::new(texture)))
            })
            .collect();
        out.push(LensNode { node, slot, material, kind, pictures });
    }
    out
}

/// A node and every node below it, depth first in file order.
fn part_nodes(hierarchy: &Hierarchy, root: usize) -> Vec<usize> {
    let mut out = vec![root];
    let mut at = 0;
    while at < out.len() {
        let here = out[at];
        let kids: Vec<usize> = (0..hierarchy.nodes.len()).filter(|&i| hierarchy.nodes[i].parent == Some(here)).collect();
        // Depth first: the children go right after their parent.
        for (k, kid) in kids.into_iter().enumerate() {
            out.insert(at + 1 + k, kid);
        }
        at += 1;
    }
    out
}
