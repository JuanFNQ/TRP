//! Reads made away from the frame: taken on the frame they come, or waited
//! for where they are first wanted.
//!
//! What nothing on the first frame shows is read on a thread of the async
//! compute pool, with a reader of its own and the run's database
//! (`GameData::db`), and put into the engine's stores on the frame it comes
//! (`worn_reads`, `clothing`, `weapon::Preparing`, the bullets' holes in
//! `decal_view`). Where it is wanted before it has come (a shot in the
//! first frames, a thing put on in a capture) the frame waits for it there
//! (`take` with `wait`), so that what is shown is what a run that read it at
//! the start shows: only when it is read changes. An unattended run (a
//! capture, a scripted replay) waits for a thing at the first frame that
//! may show it, so that what it shows never hangs on how fast a thread was.

use std::time::Instant;

use bevy::prelude::*;
use bevy::tasks::{block_on, futures_lite::future::poll_once, Task};

/// Where a picture made is put: the engine's store, on the frame's thread;
/// or away from the frame a handle reserved from that store, the picture
/// kept to be put into it on the frame the read is taken (`PendingImages`),
/// so that a reader made to give handles runs on a thread unchanged.
pub trait AddImage {
    fn add_image(&mut self, image: Image) -> Handle<Image>;
}

impl AddImage for Assets<Image> {
    fn add_image(&mut self, image: Image) -> Handle<Image> {
        self.add(image)
    }
}

impl AddImage for ResMut<'_, Assets<Image>> {
    fn add_image(&mut self, image: Image) -> Handle<Image> {
        self.add(image)
    }
}

/// Pictures made away from the frame, each with its handle reserved from
/// the engine's store (`Assets::get_handle_provider`), put into the store
/// together (`put`). Until then a handle draws nothing, as one loading.
pub struct PendingImages {
    provider: bevy::asset::AssetHandleProvider,
    made: Vec<(Handle<Image>, Image)>,
}

impl PendingImages {
    pub fn new(images: &Assets<Image>) -> Self {
        Self { provider: images.get_handle_provider(), made: Vec::new() }
    }

    /// Into the store, every one; the bytes of them.
    pub fn put(self, images: &mut Assets<Image>) -> usize {
        let mut bytes = 0;
        for (handle, image) in self.made {
            bytes += image.data.as_ref().map_or(0, Vec::len);
            if let Err(e) = images.insert(handle.id(), image) {
                warn!("a picture made away from the frame could not be put in its place: {e}");
            }
        }
        bytes
    }
}

impl AddImage for PendingImages {
    fn add_image(&mut self, image: Image) -> Handle<Image> {
        let handle = self.provider.reserve_handle().typed::<Image>();
        self.made.push((handle.clone(), image));
        handle
    }
}

/// A thread's result: taken if it has come, or, with `wait`, waited for on
/// this frame (the wait is logged under `what` when it held the frame).
pub fn take<T>(task: &mut Task<T>, wait: bool, what: &str) -> Option<T> {
    if !wait {
        return block_on(poll_once(task));
    }
    let began = Instant::now();
    let made = block_on(task);
    let waited = began.elapsed().as_secs_f32() * 1000.0;
    if waited >= 1.0 {
        info!("{what}: wanted before it had come, waited for on the frame for {waited:.1} ms");
    }
    Some(made)
}
