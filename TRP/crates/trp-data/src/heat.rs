//! The parts of a weapon that glow with its heat (`docs/spec/malfunctions.md`,
//! "The barrel's glow"): each object with a `HotObject`, whose renderer the
//! weapon's heat is given to each frame (`WeaponPrefab.ManualUpdate`), and
//! which of that renderer's materials draw the glow (the weapons' surface
//! shader with its `USEHEAT` keyword on: `HotObject` does not switch it).

use crate::unity::extract::Hierarchy;
use crate::unity::{Assets, Value};

/// A part that glows with the weapon's heat, as its `HotObject` has it: the
/// node whose renderer it heats; the box its glow fills, in the node's own
/// axes (`HeatBounds`: its centre, and its whole size, twice the extent
/// stored); how visible the glow is (`VisibleHeatAlpha`: 0 on a part that
/// only warms in thermal vision); and the material slots it heats whose
/// material glows.
#[derive(Debug, Clone, PartialEq)]
pub struct HotPart {
    pub node: usize,
    pub center: [f32; 3],
    pub size: [f32; 3],
    pub visible: f32,
    pub slots: Vec<HotSlot>,
}

/// A material slot that glows: its place among the renderer's, its glow's
/// two colours (`_HeatColor1`, `_HeatColor2`) and its reflection's figures
/// (`_SpecVals`), which weigh the glow.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HotSlot {
    pub slot: usize,
    pub colors: [[f32; 4]; 2],
    pub spec: [f32; 4],
}

/// The shader's defaults where a material leaves its colours unset (`p0/
/// Reflective/Bumped Specular SMap`'s properties).
const COLOR_1: [f32; 4] = [1.0, 0.0, 0.0, 1.0];
const COLOR_2: [f32; 4] = [1.0, 0.34, 0.0, 1.0];

/// The hot parts of an assembled tree, in its order.
pub fn read(assets: &Assets, hierarchy: &Hierarchy) -> Vec<HotPart> {
    let mut parts = Vec::new();
    for (index, node) in hierarchy.nodes.iter().enumerate() {
        let Some(renderer) = node.renderer.as_ref() else { continue };
        for (_, &obj) in node.scripts.iter().zip(&node.script_objects).filter(|(name, _)| name.as_str() == "HotObject") {
            let Ok(value) = assets.read(obj) else { continue };
            let bounds = value.get("HeatBounds");
            let center = bounds.and_then(|b| b.get("m_Center")).and_then(Value::as_vec3).unwrap_or([0.0; 3]);
            // The class's own default where none is stored.
            let extent = bounds.and_then(|b| b.get("m_Extent")).and_then(Value::as_vec3).unwrap_or([0.015, 0.04, 0.015]);
            let visible = value.get("VisibleHeatAlpha").and_then(Value::as_f32).unwrap_or(1.0);
            let all = value.get("IsApplyAllMaterials").and_then(Value::as_bool).unwrap_or(true);
            let listed: Vec<usize> = value.get("materialsId").and_then(Value::as_array).unwrap_or(&[]).iter().filter_map(|id| id.as_i64()).map(|id| id as usize).collect();
            let mut slots = Vec::new();
            for (slot, material) in renderer.materials.iter().enumerate() {
                if !all && !listed.contains(&slot) {
                    continue;
                }
                let Some(material) = material.and_then(|material| assets.read(material).ok()) else { continue };
                if let Some(glows) = glowing(&material) {
                    slots.push(HotSlot { slot, ..glows });
                }
            }
            parts.push(HotPart { node: index, center, size: extent.map(|e| 2.0 * e), visible, slots });
        }
    }
    parts
}

/// A material's glow, where its `USEHEAT` keyword is on (in its valid
/// keywords, or the older string of them).
fn glowing(material: &Value) -> Option<HotSlot> {
    let listed = material.get("m_ValidKeywords").and_then(Value::as_array).is_some_and(|keywords| keywords.iter().any(|k| k.as_str() == Some("USEHEAT")));
    let spelled = material.get("m_ShaderKeywords").and_then(Value::as_str).is_some_and(|keywords| keywords.split(' ').any(|k| k == "USEHEAT"));
    if !listed && !spelled {
        return None;
    }
    let colors = material.get("m_SavedProperties").and_then(|p| p.get("m_Colors")).and_then(Value::as_array).unwrap_or(&[]);
    let color = |name: &str| {
        colors
            .iter()
            .find(|pair| pair.get("first").and_then(Value::as_str) == Some(name))
            .and_then(|pair| pair.get("second"))
            .and_then(|c| Some([c.get("r")?.as_f32()?, c.get("g")?.as_f32()?, c.get("b")?.as_f32()?, c.get("a")?.as_f32()?]))
    };
    Some(HotSlot { slot: 0, colors: [color("_HeatColor1").unwrap_or(COLOR_1), color("_HeatColor2").unwrap_or(COLOR_2)], spec: color("_SpecVals").unwrap_or([1.0, 1.0, 0.0, 0.0]) })
}
