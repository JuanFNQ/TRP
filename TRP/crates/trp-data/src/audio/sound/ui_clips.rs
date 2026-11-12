//! The interface's and items' sounds.

use super::*;

/// The interface's sounds: those of the kinds asked, by kind, and the two
/// lists a magazine's rounds are loaded and unloaded to (inventory spec,
/// "Loading and unloading a magazine").
pub struct UiSounds {
    pub by_kind: HashMap<i32, Clip>,
    /// `_loadSounds`, `_unloadSounds`, in their order.
    pub load: Vec<Clip>,
    pub unload: Vec<Clip>,
    /// A notice's sound (`GUISounds.PlayNotificationSound`: the Resource
    /// `Audio/Interface Sounds/notification_exp`), where it is had.
    pub notification: Option<Clip>,
}

/// The interface's sounds of the kinds asked (`EUISoundType`'s numbers), by
/// kind: the Resource `audio/uisoundswrapper` (`UISoundsWrapper`), whose
/// fields after a script's own begin with its list of a kind and a clip's
/// pointer each (`_UIAudioClips`), the clips read from `resources.assets`
/// by an audio clip's type tree borrowed from the things' bundle, their
/// banks from the file their resource names beside it. After that list
/// come two more of the same shape (the social network's, the game's end's)
/// and then the load's and the unload's, each a list of clips' pointers.
pub fn ui_sounds(install: &Install, kinds: &[i32]) -> Result<UiSounds> {
    let mut setups = VorbisSetups::new(install);
    let paths = crate::interface::screen::resource_paths(install)?;
    let (_, file, id) = paths.iter().find(|(path, ..)| path == "audio/uisoundswrapper").context("no resource is at audio/uisoundswrapper")?;
    let (_, bytes) = crate::interface::screen::object_bytes(install, file, *id)?;
    let int = |at: usize| -> Result<i64> { Ok(i32::from_le_bytes(bytes.get(at..at + 4).context("the sounds' list is short")?.try_into()?) as i64) };
    // The object, its switch, its script, its name (four-aligned).
    let mut at = 12 + 4 + 12;
    at += 4 + int(at)? as usize;
    at = (at + 3) & !3;
    let count = int(at)? as usize;
    at += 4;
    let tree = crate::interface::screen::tree_of(install, ITEM_SOUNDS, AUDIO_CLIP)?;
    let mut pictures = crate::interface::screen::ScenePictures::new(install)?;
    let folder = install.root.join("EscapeFromTarkov_Data");
    let bank = |source: &str, offset: usize, size: usize| -> Result<Vec<u8>> {
        use std::io::{Read, Seek, SeekFrom};
        let path = folder.join(source.rsplit('/').next().unwrap_or(source));
        let mut file = std::fs::File::open(&path).with_context(|| path.display().to_string())?;
        file.seek(SeekFrom::Start(offset as u64))?;
        let mut data = vec![0; size];
        file.read_exact(&mut data)?;
        Ok(data)
    };
    let mut out = HashMap::new();
    for n in 0..count {
        let entry = at + 16 * n;
        let kind = int(entry)? as i32;
        if !kinds.contains(&kind) || int(entry + 4)? != 0 {
            continue;
        }
        let clip_id = i64::from_le_bytes(bytes.get(entry + 8..entry + 16).context("the sounds' list is short")?.try_into()?);
        let value = pictures.read_by(file, clip_id, AUDIO_CLIP, &tree)?;
        match clip_of(&value, &bank, &mut setups) {
            Ok(clip) => {
                out.insert(kind, clip);
            }
            Err(e) => eprintln!("the interface's sound {kind} could not be read: {e:#}"),
        }
    }
    // Past the two lists of a kind and a clip each, the two of clips.
    let mut lists = at + 16 * count;
    for _ in 0..2 {
        lists += 4 + 16 * int(lists)? as usize;
    }
    let mut clips_of = |at: &mut usize| -> Result<Vec<Clip>> {
        let count = int(*at)? as usize;
        let mut clips = Vec::new();
        for n in 0..count {
            let entry = *at + 4 + 12 * n;
            if int(entry)? != 0 {
                continue;
            }
            let clip_id = i64::from_le_bytes(bytes.get(entry + 4..entry + 12).context("the sounds' list is short")?.try_into()?);
            let value = pictures.read_by(file, clip_id, AUDIO_CLIP, &tree)?;
            clips.push(clip_of(&value, &bank, &mut setups)?);
        }
        *at += 4 + 12 * count;
        Ok(clips)
    };
    let load = clips_of(&mut lists)?;
    let unload = clips_of(&mut lists)?;
    let notification = paths.iter().find(|(path, ..)| path == "audio/interface sounds/notification_exp").and_then(|(_, file, id)| {
        let value = pictures.read_by(file, *id, AUDIO_CLIP, &tree).ok()?;
        clip_of(&value, &bank, &mut setups).map_err(|e| eprintln!("the notices' sound could not be read: {e:#}")).ok()
    });
    Ok(UiSounds { by_kind: out, load, unload, notification })
}

/// The things' sounds (`ItemSounds`), every one, by its clip's name
/// (`<group>_<way>`: `weap_rifle_drop`; the ways `drop`, `pickup`, `use`,
/// `open`, `other`, `offline_use`, `EInventorySoundType`).
pub fn item_sounds(install: &Install) -> Result<HashMap<String, Clip>> {
    let mut setups = VorbisSetups::new(install);
    let mut assets = crate::unity::Assets::new(install.clone())?;
    let files = assets.load(ITEM_SOUNDS)?;
    let mut out = HashMap::new();
    // Those that could not be read, said once together.
    let mut unread = Vec::new();
    for file in files {
        let ids: Vec<i64> = assets.file(file).meta.objects.iter().filter(|object| object.class_id == AUDIO_CLIP).map(|object| object.path_id).collect();
        for id in ids {
            let value = assets.read(crate::unity::ObjRef { file, path_id: id })?;
            let bank = |source: &str, offset: usize, size: usize| -> Result<Vec<u8>> { Ok(assets.resource(source, offset, size)?.to_vec()) };
            match clip_of(&value, &bank, &mut setups) {
                Ok(clip) => {
                    out.insert(clip.name.clone(), clip);
                }
                Err(e) => unread.push(format!("{e:#}")),
            }
        }
    }
    if !unread.is_empty() {
        eprintln!("{} of the things' sounds are not read and are silent: {}", unread.len(), unread.join("; "));
    }
    Ok(out)
}
