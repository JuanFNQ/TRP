//! The watch on the first-person hands' wrist.
//! After the body animator spec, "The watch on the wrist". A hands entry of the install's customization list
//! names a prefab of the watch's three hands (`WatchPrefab`) and where it
//! goes on the hands' skeleton (`WatchPosition`, `WatchRotation`); the
//! prefab names the bone (`PluggableBone.BonePath`), its hands and the
//! second hand's tick (`Watch`). The watch's case and face are the hands'
//! own mesh.

use std::collections::HashMap;

use anyhow::{anyhow, Context, Result};
use serde_json::Value as Json;

use crate::formats::curve::Curve;
use crate::setup::install::Install;
use crate::unity::extract::{exported_game_objects, read_hierarchy, Hierarchy};
use crate::unity::Assets;

/// Where a hands entry puts its watch: the prefab's bundle, and its place
/// and turn on its bone (Unity's axes; the turn as Euler angles in degrees,
/// `Quaternion.Euler`).
#[derive(Debug, Clone, PartialEq)]
pub struct WatchPlace {
    pub prefab: String,
    pub position: [f32; 3],
    pub rotation: [f32; 3],
}

/// Each hands bundle's watch, from the install's customization list: the
/// entries that are items for the hands (`BodyPart` `Hands`) and name a
/// watch (`CustomizationSolver.GetWatchBundle`; none where the path is
/// empty, `WatchBundleInfo.HasValidPath`). The game goes by the entry; the
/// app goes by the bundle it draws, and in the install each hands bundle
/// is one entry's (one is two entries', with the same watch).
pub fn places(install: &Install) -> Result<HashMap<String, WatchPlace>> {
    let path = install.database_dir().join("templates/customization.json");
    let text = std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let all: serde_json::Map<String, Json> = serde_json::from_str(&text).with_context(|| path.display().to_string())?;
    let vector = |v: Option<&Json>| -> [f32; 3] { ["x", "y", "z"].map(|axis| v.and_then(|v| v.get(axis)).and_then(Json::as_f64).unwrap_or(0.0) as f32) };
    let mut places = HashMap::new();
    for entry in all.values().filter(|entry| entry.get("_type").and_then(Json::as_str) == Some("Item")) {
        let Some(props) = entry.get("_props").filter(|props| props.get("BodyPart").and_then(Json::as_str) == Some("Hands")) else { continue };
        let path_of = |name: &str| props.pointer(&format!("/{name}/path")).and_then(Json::as_str).filter(|path| !path.is_empty());
        let (Some(hands), Some(prefab)) = (path_of("Prefab"), path_of("WatchPrefab")) else { continue };
        places.insert(hands.to_owned(), WatchPlace { prefab: prefab.to_owned(), position: vector(props.get("WatchPosition")), rotation: vector(props.get("WatchRotation")) });
    }
    Ok(places)
}

/// A watch prefab as read: its objects, the bone it goes on, its three
/// hands and the second hand's tick.
pub struct WatchModel {
    pub hierarchy: Hierarchy,
    /// The bone's path on the hands' skeleton, from the pelvis
    /// (`PluggableBone.BonePath`).
    pub bone_path: String,
    /// The hour, minute and second hands' objects, by their places in
    /// `hierarchy` (`Watch._hourHand`, `_minuteHand`, `_secondHand`).
    pub hands: [usize; 3],
    /// The second hand's place between two seconds by the share of the
    /// second gone (`Watch._curve`).
    pub tick: Curve,
}

/// Reads a watch prefab from its bundle.
pub fn load(assets: &mut Assets, bundle_key: &str) -> Result<WatchModel> {
    let files = assets.load(bundle_key)?;
    let (_, root) = files
        .iter()
        .find_map(|&file| exported_game_objects(assets, file).ok()?.into_iter().next())
        .ok_or_else(|| anyhow!("{bundle_key} exports no object"))?;
    let hierarchy = read_hierarchy(assets, root)?;
    let script = |class: &str| {
        hierarchy
            .nodes
            .iter()
            .flat_map(|node| node.scripts.iter().zip(&node.script_objects))
            .find(|(name, _)| name.as_str() == class)
            .map(|(_, obj)| *obj)
            .ok_or_else(|| anyhow!("{bundle_key} has no {class}"))
    };
    let plug = assets.read(script("PluggableBone")?)?;
    let bone_path = plug.req("BonePath")?.as_str().ok_or_else(|| anyhow!("{bundle_key}: its bone's path is not text"))?.to_owned();
    let watch_obj = script("Watch")?;
    let watch = assets.read(watch_obj)?;
    let hand = |field: &str| -> Result<usize> {
        let transform = watch.req(field)?.as_pptr().and_then(|pptr| assets.resolve_loaded(watch_obj.file, pptr)).ok_or_else(|| anyhow!("{bundle_key}: {field} names nothing"))?;
        hierarchy.nodes.iter().position(|node| node.transform == transform).ok_or_else(|| anyhow!("{bundle_key}: {field} is not one of its objects"))
    };
    let hands = [hand("_hourHand")?, hand("_minuteHand")?, hand("_secondHand")?];
    let tick = crate::character::player::curve(watch.req("_curve")?)?;
    Ok(WatchModel { hierarchy, bone_path, hands, tick })
}
