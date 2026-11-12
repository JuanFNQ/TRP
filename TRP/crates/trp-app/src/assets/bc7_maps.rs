//! The install's normal and gloss pictures packed as BC7, made BC5 of the two channels the material reads.
//! After `trp_data::unity::bc7::to_bc5`, a byte a pixel as the BC7 was: each once.
//!
//! - **Shared while it is made**: a picture asked for again while it is being
//!   made (a weapon's rigs, its hands', its body's, its picture's and its
//!   model on the ground, are made at once, each on a thread of its own) waits
//!   for that one and takes it.
//! - **Kept on the disk**: made once, it is written to `local\bc7_cache`
//!   (or the folder `TRP_BC7_CACHE` names, to try a run with a cache of its
//!   own), named by a hash of the BC7's own bytes and the channels taken, and
//!   read from there from then on: by every run after, and by a picture's
//!   levels read again when they are streamed (`streaming`). A picture of the
//!   install changed is another name, made again. Nothing of it is kept in
//!   memory once made.
//! - **Made on threads of its own**, half the processor's shared by all that
//!   make them at once, so that the frame's own threads keep theirs.

use std::collections::HashMap;
use std::hash::{BuildHasher, Hash, Hasher};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

/// The cache's version: raised when what is written changes (`v2` since
/// 0.187.0: a normal map's X red times alpha, as the game's shader reads it,
/// where it had been alpha alone).
const CACHE: &str = "v2";

/// Which two channels are taken.
#[derive(Clone, Copy, Hash)]
pub enum Taken {
    /// A normal map's X and Y as the game's surface shader reads them
    /// (`p0/Reflective/Bumped Specular SMap`, Unity's `UnpackNormalmapRGorAG`):
    /// red times alpha, and green. The install packs X in alpha with red
    /// full (its BC7 ones' red 239 to 255), or X in red with alpha full.
    Normals,
    /// A gloss map's grey made roughness: full, then the red inverted.
    Roughness,
}

impl Taken {
    pub fn of(self, pixel: [u8; 4]) -> [u8; 2] {
        match self {
            Taken::Normals => [((pixel[0] as u32 * pixel[3] as u32 + 127) / 255) as u8, pixel[1]],
            Taken::Roughness => [255, 255 - pixel[0]],
        }
    }
}

type Making = Arc<OnceLock<Arc<Vec<u8>>>>;

/// Pictures being made now, by their names.
fn making() -> &'static Mutex<HashMap<u64, Making>> {
    static MAKING: OnceLock<Mutex<HashMap<u64, Making>>> = OnceLock::new();
    MAKING.get_or_init(Default::default)
}

/// The folder kept on the disk, made where it is not; none where there is no
/// `local` beside the workspace and no `TRP_BC7_CACHE`.
fn folder() -> Option<&'static PathBuf> {
    static FOLDER: OnceLock<Option<PathBuf>> = OnceLock::new();
    FOLDER
        .get_or_init(|| {
            let folder = match std::env::var_os("TRP_BC7_CACHE") {
                Some(folder) => PathBuf::from(folder).join(CACHE),
                None => std::env::current_dir().ok()?.ancestors().map(|dir| dir.join("local")).find(|local| local.is_dir())?.join("bc7_cache").join(CACHE),
            };
            std::fs::create_dir_all(&folder).ok().map(|_| folder)
        })
        .as_ref()
}

/// The blocks of a BC7 picture's levels (whole blocks) as BC5 of the two
/// channels `taken`, from the disk where they were made before, else made;
/// `picture` its name, for the log.
pub fn to_bc5(blocks: &[u8], taken: Taken, picture: &str) -> Vec<u8> {
    let started = std::time::Instant::now();
    let name = {
        let mut hasher = bevy::platform::hash::FixedHasher.build_hasher();
        (taken, blocks.len(), blocks).hash(&mut hasher);
        hasher.finish()
    };
    let path = folder().map(|folder| folder.join(format!("{name:016x}.bc5")));
    let cell: Making = making().lock().map(|mut making| making.entry(name).or_default().clone()).unwrap_or_default();
    let mut how = "shared";
    let made = cell
        .get_or_init(|| {
            if let Some(read) = path.as_ref().and_then(|path| std::fs::read(path).ok()).filter(|read| read.len() == blocks.len()) {
                how = "read";
                return Arc::new(read);
            }
            how = "made";
            let made = with_threads(|threads| trp_data::unity::bc7::to_bc5(blocks, |pixel| taken.of(pixel), threads));
            // Written whole under another name, then named: a run stopped
            // in the middle leaves no part of one to be read.
            if let Some(path) = &path {
                let part = path.with_extension(format!("part{:?}", std::thread::current().id()).replace(['(', ')'], ""));
                if std::fs::write(&part, &made).is_ok() && std::fs::rename(&part, path).is_err() {
                    let _ = std::fs::remove_file(&part);
                }
            }
            Arc::new(made)
        })
        .clone();
    // Made, it is on the disk: kept in memory no longer.
    if let Ok(mut making) = making().lock() {
        making.remove(&name);
    }
    bevy::log::debug!("BC7 {picture} ({name:016x}, {} KB) {how} in {:.1} ms", blocks.len() / 1024, started.elapsed().as_secs_f64() * 1000.0);
    Vec::clone(&made)
}

/// Work given threads of its own from those set aside for making BC7
/// pictures BC5: half the processor's, shared by all that make them at once,
/// each taking half of those free (one alone has most of them, two or three
/// at once each a share). Work that finds none free is done on its own
/// thread.
fn with_threads<T>(work: impl FnOnce(usize) -> T) -> T {
    static FREE: OnceLock<AtomicUsize> = OnceLock::new();
    let free = FREE.get_or_init(|| AtomicUsize::new((std::thread::available_parallelism().map_or(2, usize::from) / 2).max(1)));
    let mut taken = 0;
    let _ = free.try_update(Ordering::AcqRel, Ordering::Acquire, |left| {
        taken = left.div_ceil(2);
        Some(left - taken)
    });
    let out = work(taken.max(1));
    free.fetch_add(taken, Ordering::AcqRel);
    out
}
