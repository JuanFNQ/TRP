//! The hideout shooting range's popper, read from the install to be put on
//! the app's range (`docs/spec/range-targets.md`): one popper whole from the
//! hideout's scene (`scene_props`, as the workbench is read), which of its
//! objects are the plate and the roller, and its four sounds, found by
//! name in the scene's shared file (which keeps no type trees: an
//! `AudioClip` is read by the tree of the same class in a bundle the same
//! engine wrote, `screen::tree_of`), their data from the file beside it.

use anyhow::{Context, Result};

use crate::install::Install;
use crate::scene_props::Props;
use crate::screen::{tree_of, ScenePictures};
use crate::sound::Clip;
use crate::unity::{class, Value};

/// The hideout's scene, and one popper in it (level 3's first row's first;
/// every popper is the same but for its row).
pub const SCENE: &str = "bunker_2.unity";
pub const POPPER: &str = "12_shooting_range/level3/highlight_transform/Popper_targets/01_row_targets/Shooting_range_popper_changed (1)";
/// Its plate and its roller, the two parts turned at their hinges
/// (`FoldingPopperTarget._target`, `_roller`).
pub const PLATE: &str = "Shooting_range_popper_part_03";
pub const ROLLER: &str = "Shooting_range_popper_part_02";
/// The scene's shared file its sounds are in.
const SOUNDS_FILE: &str = "sharedassets161.assets";
/// Its sounds' clips (`_foldAudios`' first two, `_unfoldAudio`,
/// `_rockingAudio`; the third fold's is never played).
pub const CLIPS: [&str; 4] = ["shooting_range_popper_fold", "shooting_range_popper_fold2", "shooting_range_popper_unfold", "shooting_range_popper_rocking"];

/// A popper as read: its objects and what they draw, and the places in
/// `props.nodes` of its plate and its roller.
pub struct PopperModel {
    pub props: Props,
    pub plate: usize,
    pub roller: usize,
}

/// Reads one popper whole (its sounds' objects and decals left out).
pub fn popper(install: &Install) -> Result<PopperModel> {
    let props = crate::scene_props::read(install, SCENE, POPPER, &["DecalSystem"], &[], &[])?;
    let find = |name: &str| props.nodes.iter().position(|node| node.name == name).with_context(|| format!("the popper has no {name}"));
    let (plate, roller) = (find(PLATE)?, find(ROLLER)?);
    Ok(PopperModel { props, plate, roller })
}

/// The popper's sounds, by their names in `CLIPS`' order.
pub fn popper_clips(install: &Install) -> Result<Vec<(String, Clip)>> {
    let tree = tree_of(install, crate::sound::EFFECTS, class::AUDIO_CLIP).context("an audio clip's tree")?;
    let mut scene = ScenePictures::new(install)?;
    let mut setups = crate::sound::VorbisSetups::new(install);
    let folder = install.root.join("EscapeFromTarkov_Data");
    let resource = |source: &str, offset: usize, size: usize| -> Result<Vec<u8>> {
        let name = source.rsplit(['/', '\\']).next().unwrap_or(source);
        let data = std::fs::read(folder.join(name)).with_context(|| format!("the clip's data in {name}"))?;
        data.get(offset..offset + size).map(<[u8]>::to_vec).with_context(|| format!("{name} is short of {offset} + {size}"))
    };
    let mut found: Vec<Option<(String, Clip)>> = vec![None; CLIPS.len()];
    for id in scene.objects(SOUNDS_FILE, class::AUDIO_CLIP)? {
        let Ok(value) = scene.read_by(SOUNDS_FILE, id, class::AUDIO_CLIP, &tree) else { continue };
        let Some(name) = value.get("m_Name").and_then(Value::as_str) else { continue };
        let Some(at) = CLIPS.iter().position(|wanted| *wanted == name) else { continue };
        if found[at].is_none() {
            found[at] = Some((name.to_owned(), crate::sound::clip_of(&value, &resource, &mut setups)?));
        }
    }
    found.into_iter().zip(CLIPS).map(|(clip, name)| clip.with_context(|| format!("no clip {name} in {SOUNDS_FILE}"))).collect()
}
