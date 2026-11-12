//! The texture settings, sources added, and streamed textures' levels.

use super::*;

/// The game's two settings of its pictures (`Graphics.ini`: `TextureQuality`
/// 0 low to 2 high, `MipStreaming`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextureSettings {
    pub quality: u8,
    pub streaming: bool,
}

impl TextureSettings {
    /// The levels every picture loses with streaming off
    /// (`TextureQualityToMipBias`: `clamp(2 - quality, 0, 2)`).
    pub fn limit(&self) -> u32 {
        if self.streaming { 0 } else { 2u32.saturating_sub(self.quality as u32) }
    }

    /// The streaming's bias, in levels (the same figure).
    pub fn bias(&self) -> f32 {
        2.0 - self.quality.min(2) as f32
    }

    /// The most levels a picture may be brought down
    /// (`streamingMipmapsMaxLevelReduction = 4 - quality`).
    pub fn most_reduction(&self) -> u32 {
        4u32.saturating_sub(self.quality as u32)
    }
}

/// The pictures' quality the app starts at until the options change it:
/// HIGH, streamed. The
/// install's own is what `AS THE INSTALL HAS THEM` takes.
pub const START_QUALITY: u8 = 2;

/// The game's budget for streamed pictures: 1,024 MB at every quality
/// (`RefreshStreamingMemoryBudget(1024f * Vector4.one, ...)`).
pub const BUDGET: u64 = 1024 << 20;

/// How much of the pictures may go up to the card in a frame: the game's
/// upload buffer (`Graphics.ini`: `MipStreamingBufferSize`, 32 MB in the
/// install read). `ESTIMATE`: Unity's buffer is a ring the
/// uploads pass through, not a limit a frame.
pub(super) const UPLOAD_A_FRAME: usize = 32 << 20;

/// How much of the copies made may be on their way to the card at once:
/// taken in to the engine and not yet seen there. A copy made waits for a
/// frame where it fits under this with those on their way (one larger goes
/// alone when none is), so that the copies go over to the renderer, up to
/// the card and into their materials a few a frame. `ESTIMATE`: one frame's
/// upload to the card (`warmup_log::UPLOAD_BUDGET`, 8 MB); taken in all at
/// once, a walk's raise of some 68 MB made 30 to 45 frames of 5 to 11 ms
/// (session 287). Unity time-slices its uploads (`asyncUploadTimeSlice`),
/// not read from the install.
pub(super) const ON_THEIR_WAY: usize = 8 << 20;

/// A picture brought down in detail only once it has been wanted lower for
/// so long, so that a thing passing in and out of sight does not send its
/// pictures up again and again. `ESTIMATE`: Unity's own delay is not
/// published.
pub(super) const LOWER_AFTER: f32 = 2.0;

/// The levels are worked out so often. `ESTIMATE`.
pub(super) const DECIDE_EVERY: f32 = 0.1;

/// The settings the run's pictures are made by, once, before any is.
static SETTINGS: OnceLock<TextureSettings> = OnceLock::new();

/// Pictures added since `take_new` last took them in; and how many have
/// been added whole, the game not streaming them.
pub(super) static ADDED: Mutex<Vec<Streamed>> = Mutex::new(Vec::new());

pub(super) static ADDED_WHOLE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// The settings the run's pictures are made by: once, before any is.
pub fn set(settings: TextureSettings) {
    let _ = SETTINGS.set(settings);
}

/// A picture made whole again from where it was read, as it was made the
/// first time: from its bundle's file (`trp_data::unity::ResourcePlace`),
/// so that nothing of it is kept in memory.
pub type ReadAgain = Arc<dyn Fn() -> Option<Image> + Send + Sync>;

/// Where a picture's levels are had again when it is streamed, or that it
/// is not.
pub enum Source {
    /// From its bundle's file.
    Bundle(ReadAgain),
    /// Kept here: a picture that has no bundle (a weapon of one's own's,
    /// the other game's), or whose levels are in its serialized file.
    Kept,
    /// Not streamed: the game's engine does not stream it
    /// (`m_StreamingMipmaps` off), and draws it whole while streaming is on.
    Whole,
}

/// Where a streamed picture's levels are had again.
#[derive(Clone)]
pub(super) enum Whole {
    /// From its bundle.
    Read(ReadAgain),
    /// Kept here: a picture that has no bundle (a weapon of one's own's,
    /// the other game's).
    Kept(Arc<Vec<u8>>),
}

/// A model's picture into the engine's store as the settings have it: with
/// streaming off without the levels the quality leaves off (every picture,
/// as Unity's `globalTextureMipmapLimit` takes them all); with it on, one
/// the game streams from the most it may be brought down, to be raised as
/// it is needed, its levels had again from `source`. A picture of one
/// level, or before the settings are known, goes in as it is.
pub fn add(images: &mut Assets<Image>, mut image: Image, source: Source) -> Handle<Image> {
    let Some(settings) = SETTINGS.get().copied() else { return images.add(image) };
    // A cube (a material's `_Cube`, six faces each with its levels): as it
    // is, whole; the cutting below takes one face's levels.
    if image.texture_descriptor.size.depth_or_array_layers > 1 {
        return images.add(image);
    }
    if settings.streaming && matches!(source, Source::Whole) {
        ADDED_WHOLE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        return images.add(image);
    }
    let Some((levels, deepest)) = levels_of(&image) else { return images.add(image) };
    let Some(data) = image.data.take() else { return images.add(image) };
    let start = if settings.streaming { settings.most_reduction() } else { settings.limit() }.min(deepest);
    let first = images.add(cut(&image, &levels, &data, start));
    if settings.streaming {
        let whole = match source {
            Source::Bundle(again) => Whole::Read(again),
            Source::Kept | Source::Whole => Whole::Kept(Arc::new(data)),
        };
        let streamed = Streamed { template: image, whole, levels, deepest, loaded: start, lower_since: None, first: first.clone(), current: None, pending: None, arriving: None, stuck: false, raised_at: None };
        if let Ok(mut added) = ADDED.lock() {
            added.push(streamed);
        }
    }
    first
}

/// A picture from `level` on, from its data whole.
pub(super) fn cut(template: &Image, levels: &[(std::ops::Range<usize>, UVec2)], data: &[u8], level: u32) -> Image {
    let mut image = template.clone();
    let (start, size) = (levels[level as usize].0.start, levels[level as usize].1);
    image.texture_descriptor.size.width = size.x;
    image.texture_descriptor.size.height = size.y;
    image.texture_descriptor.mip_level_count = levels.len() as u32 - level;
    image.data = Some(data[start..levels.last().map_or(start, |last| last.0.end)].to_vec());
    image
}

/// A picture streamed, and what of it is on the card.
pub(super) struct Streamed {
    /// The picture as made, without its data, and where its data is had
    /// again.
    template: Image,
    pub(super) whole: Whole,
    /// Each level's place in the data and its size in pixels.
    pub(super) levels: Vec<(std::ops::Range<usize>, UVec2)>,
    /// The deepest level a copy may start from (a compressed picture's
    /// first level whole blocks).
    pub(super) deepest: u32,
    /// The level the card's copy starts from, and since when it has been
    /// wanted lower.
    pub(super) loaded: u32,
    pub(super) lower_since: Option<f32>,
    /// The copy first added, which the models were made with and which is
    /// kept as long as the picture is (a rig waits for it to be on the
    /// card); and the copy the materials draw with now, where it is
    /// another.
    pub(super) first: Handle<Image>,
    pub(super) current: Option<Handle<Image>>,
    /// A copy being made away from the frame, from its level; and whether
    /// one could not be (left as it is from then).
    pub(super) pending: Option<(u32, Task<Option<Image>>)>,
    /// A copy made and added, from its level, on its way to the card: the
    /// materials are pointed at it once it is there (`take_made`).
    pub(super) arriving: Option<(u32, Handle<Image>)>,
    pub(super) stuck: bool,
    /// When the raise now on its way was decided, on the streaming's
    /// clock: for the log of how long a raise takes to reach the card.
    pub(super) raised_at: Option<f32>,
}

impl Streamed {
    pub(super) fn bytes_from(&self, level: u32) -> usize {
        self.levels[level as usize..].iter().map(|(range, _)| range.len()).sum()
    }

    /// Its copy from `level` on, made away from the frame. The copy's
    /// bytes are the renderer's alone (`RenderAssetUsages::RENDER_WORLD`):
    /// only materials name a copy, and nothing reads its bytes here, so
    /// they move to the renderer as it is taken there and are not kept
    /// here as well. Kept in both worlds, the engine copied each copy's
    /// bytes whole on the frame it was taken to the renderer (0.7 to 3.9
    /// ms a frame of a walk's raises, the profile of session 287).
    pub(super) fn make(&self, level: u32) -> Task<Option<Image>> {
        let (template, levels, whole) = (self.template.clone(), self.levels.clone(), self.whole.clone());
        AsyncComputeTaskPool::get().spawn(async move {
            let mut copy = match whole {
                Whole::Kept(data) => cut(&template, &levels, &data, level),
                Whole::Read(again) => {
                    let data = again()?.data?;
                    if data.len() < levels.last()?.0.end {
                        return None;
                    }
                    cut(&template, &levels, &data, level)
                }
            };
            copy.asset_usage = bevy::asset::RenderAssetUsages::RENDER_WORLD;
            Some(copy)
        })
    }

    /// Whether anything but this holds the first copy.
    pub(super) fn held(&self) -> bool {
        match &self.first {
            Handle::Strong(strong) => Arc::strong_count(strong) > 1,
            Handle::Uuid(..) => false,
        }
    }
}

/// The places of a picture's levels in its data, and their sizes; and the
/// deepest level a copy may start from.
pub(super) fn levels_of(image: &Image) -> Option<(Vec<(std::ops::Range<usize>, UVec2)>, u32)> {
    let descriptor = &image.texture_descriptor;
    if descriptor.dimension != TextureDimension::D2 || descriptor.size.depth_or_array_layers != 1 || descriptor.mip_level_count < 2 {
        return None;
    }
    let format: TextureFormat = descriptor.format;
    let (block_w, block_h) = format.block_dimensions();
    let block_bytes = format.block_copy_size(None)? as usize;
    let (width, height) = (descriptor.size.width, descriptor.size.height);
    let mut at = 0;
    let mut levels = Vec::new();
    let mut deepest = 0;
    for level in 0..descriptor.mip_level_count {
        let (w, h) = ((width >> level).max(1), (height >> level).max(1));
        let bytes = w.div_ceil(block_w) as usize * h.div_ceil(block_h) as usize * block_bytes;
        // A compressed picture's first level is whole blocks.
        if w % block_w == 0 && h % block_h == 0 {
            deepest = level;
        }
        levels.push((at..at + bytes, UVec2::new(w, h)));
        at += bytes;
    }
    (image.data.as_ref()?.len() >= at).then_some((levels, deepest))
}
