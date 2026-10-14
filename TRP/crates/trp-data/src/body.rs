//! The player's body: its animator controller and the bones the
//! first-person view depends on. See `docs/spec/body-animator.md`.

use std::collections::HashMap;

use anyhow::{anyhow, Result};

use crate::states::StateTable;
use crate::unity::extract::{exported_game_objects, read_hierarchy};
use crate::unity::{class, Assets};

const PLAYER_BUNDLE: &str = "assets/content/commonprefabs/playersuperior.bundle";
const CONTROLLER_BUNDLE: &str = "assets/content/characters/controllers/player_anim_controller.bundle";
/// The controller the game asks that bundle for. The bundle also holds a
/// reworked copy and a backup under other names.
const CONTROLLER: &str = "Player_anim_controller";
/// The object the body's animator sits on; its clips name bones from there.
const ANIMATED_OBJECT: &str = "Player";

/// A bone of the player's skeleton at rest.
#[derive(Debug, Clone)]
pub struct Bone {
    /// Path below the animated object, as clips name it.
    pub path: String,
    pub position: [f32; 3],
    pub rotation: [f32; 4],
}

/// The body animator's controller, read in full.
pub fn controller(assets: &mut Assets) -> Result<StateTable> {
    let files = assets.load(CONTROLLER_BUNDLE)?;
    let controller = files
        .iter()
        .flat_map(|&file| assets.objects_of(file, class::ANIMATOR_CONTROLLER).collect::<Vec<_>>())
        .find(|obj| assets.name_of(*obj).as_deref() == Some(CONTROLLER))
        .ok_or_else(|| anyhow!("no controller named {CONTROLLER}"))?;
    StateTable::from_controller(assets, controller, &HashMap::new())
}

/// The rest pose of the named bones, each given by its path below the
/// animated object.
pub fn bones(assets: &mut Assets, paths: &[&str]) -> Result<Vec<Bone>> {
    for file in assets.load(PLAYER_BUNDLE)? {
        for (_, root) in exported_game_objects(assets, file)? {
            let hierarchy = read_hierarchy(assets, root)?;
            if hierarchy.find_below(0, ANIMATED_OBJECT).is_none() {
                continue;
            }
            let prefix = format!("{ANIMATED_OBJECT}/");
            let by_path: HashMap<String, usize> = (0..hierarchy.nodes.len())
                .filter_map(|i| Some((hierarchy.path(i).strip_prefix(&prefix)?.to_owned(), i)))
                .collect();
            return paths
                .iter()
                .map(|path| {
                    let node = &hierarchy.nodes[*by_path.get(*path).ok_or_else(|| anyhow!("the player has no bone {path}"))?];
                    Ok(Bone { path: (*path).to_owned(), position: node.translation, rotation: node.rotation })
                })
                .collect();
        }
    }
    Err(anyhow!("the player prefab was not found"))
}
