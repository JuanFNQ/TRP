//! A folding stock's bones and the folded view.

use super::*;

/// A build with its stock folded (`FoldableComponent`, `Weapon.RecoilDelta`,
/// `CompoundItem.CalculateExtraSize`, `ProceduralWeaponAnimation.
/// CheckShouldMoveWeaponCloser`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fold {
    /// The parts' share of the recoil without the stock's.
    pub recoil_delta: f32,
    /// Cells wide.
    pub cell_width: u32,
    /// Whether it is then held closer to the body.
    pub held_closer: bool,
}

/// A bone a fold turns (`FoldableStockView.BonePosition`): the node, and
/// its own turn folded and unfolded (`xyzw`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FoldBone {
    pub node: usize,
    pub folded: [f32; 4],
    pub unfolded: [f32; 4],
}

impl Weapon {
    /// The tree as a model not animated shows it with its fold folded or
    /// not: each bone the fold turns given that turn
    /// (`FoldableStockView.Init`, at once as it is set up).
    pub fn fold_tree(&mut self, folded: bool) {
        for bone in &self.fold_bones {
            if let Some(node) = self.hierarchy.nodes.get_mut(bone.node) {
                node.rotation = if folded { bone.folded } else { bone.unfolded };
            }
        }
    }
}

/// A bundle's fold view (`FoldableStockView._bonePositions`): each bone's
/// name and its turn folded and unfolded. None where it has no such view.
pub fn fold_view(assets: &Assets, files: &[usize]) -> Result<Vec<(String, [f32; 4], [f32; 4])>> {
    let Some(view) = files.iter().flat_map(|&file| assets.objects_of(file, class::MONO_BEHAVIOUR)).find(|&obj| script_class(assets, obj).as_deref() == Some("FoldableStockView")) else {
        return Ok(Vec::new());
    };
    let value = assets.read(view)?;
    // A turn of all noughts, as the 9A-91's view has for its unfolded
    // stock, is taken as no turn at all (`PROVISIONAL`: what the engine
    // makes of it is not read; the model lies so as made).
    let quat = |v: Option<&Value>| -> [f32; 4] {
        let f = |name: &str| v.and_then(|v| v.get(name)).and_then(Value::as_f32).unwrap_or(0.0);
        let q = [f("x"), f("y"), f("z"), f("w")];
        let length = q.iter().map(|k| k * k).sum::<f32>().sqrt();
        if length < 1e-6 { [0.0, 0.0, 0.0, 1.0] } else { q.map(|k| k / length) }
    };
    Ok(value
        .req("_bonePositions")?
        .as_array()
        .unwrap_or(&[])
        .iter()
        .filter_map(|bone| Some((bone.get("BoneName")?.as_str()?.to_owned(), quat(bone.get("FoldedRotation")), quat(bone.get("UnfoldedRotation")))))
        .collect())
}
