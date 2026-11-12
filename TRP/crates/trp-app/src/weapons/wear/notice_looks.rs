//! The notices' timing and rows, the glow material and the looks read.

use super::*;

/// How a notice comes and goes (`NotifierView`'s `Notification`, its
/// animator's controller `Item`, `sharedassets49` 113, at `Speed` 1 for a
/// notice of `ENotificationDurationType.Default`): its show (`ShowNotification`,
/// clip 100, 2.6667 s: its icon's alpha from 0 to 1 over the first third of
/// a second, flat at both ends), then at once (`AutoComplete`) its hide
/// (`HideNotification`, clip 99: its group's alpha from 1 to 0 over 0.25 s,
/// flat at both ends; its icon's own from 1 with a slope of -8.87 to 0),
/// whose event at 0.0833 s (`OnAnimationDone`) folds its row away over
/// 0.3 s, after which it is gone. Read from the install
/// (`local\notification_clip_100_s134.txt`, `_99_`; malfunctions spec,
/// "What the shooter is told").
pub(super) const NOTICE_SHOW: f64 = 2.666_667;

pub(super) const NOTICE_ICON_IN: f64 = 1.0 / 3.0;

pub(super) const NOTICE_HIDE: f64 = 0.25;

pub(super) const NOTICE_DONE: f64 = 0.083_333;

pub(super) const NOTICE_FOLD: f64 = 0.3;

/// How long a notice's row takes to open, as long as its fold
/// (`BaseNotificationView.ANIMATION_PREPARE_TIME`: `DOPreferredSize` over
/// 0.3 s, the tweening library's default easing, out and quadratic).
const NOTICE_OPEN: f64 = 0.3;

/// A notice on the screen (`BaseNotificationView`): its words, whether an
/// alert, when its clock began, when its row began to fold, and its number
/// (what it is drawn by). Its clock begins with the frame before the one
/// it is shown on: its animator and its row's tween take their first step
/// on its own frame (`courses10`'s `examine-multiple`: the first warning
/// shown on 178, its row 1.74 pixels open on 179, 0.017 s after 177's
/// time). Its row folds from the frame its hide's event comes on
/// (`OnAnimationDone`, `StopAnimation`), its tween's first step a frame on
/// (folding on 500, 0.008 s after 499's time), and it is gone on the frame
/// the fold is done (523, its row shut from 518).
#[derive(Clone, Debug, PartialEq)]
pub struct Notice {
    pub words: String,
    pub alert: bool,
    pub(super) began: f64,
    pub(super) fold: Option<f64>,
    pub(super) number: u64,
}

/// A notice's row's height, so long after its clock began and after its
/// fold began where it has, in units: as high as its box when open and the
/// row's padding below it (`LayoutElement.preferredHeight`, the
/// `Notification`'s own height, `DOPreferredSize` to it and back to
/// nothing).
pub(super) fn notice_row(full: f32, since: f64, folding: Option<f64>) -> f32 {
    let out_quad = |x: f64| 1.0 - (1.0 - x) * (1.0 - x);
    let open = out_quad((since / NOTICE_OPEN).clamp(0.0, 1.0));
    let open = match folding {
        Some(gone) => 1.0 - out_quad((gone / NOTICE_FOLD).clamp(0.0, 1.0)),
        None => open,
    };
    full * open as f32
}

/// A notice's box's height in a row so high, in units: the row less its
/// padding below, none in a row lower than that (the row's
/// `VerticalLayoutGroup` lerps its box between its least, nothing, and
/// what it asks).
pub(super) fn notice_box(row: f32, padding: f32) -> f32 {
    (row - padding).max(0.0)
}

/// The notice's ground and words as the notifier's script colours them
/// (`NotificationView._defaultBackgroundColor`, `_defaultTextColor`).
pub(super) const NOTICE_GROUND: [f32; 4] = [0.0, 0.0, 0.0, 200.0 / 255.0];

pub(super) const NOTICE_WORDS: [f32; 4] = [1.0, 1.0, 1.0, 1.0];

/// The alphas of a notice's group and its icon, so long after it was shown.
pub(super) fn notice_alphas(t: f64) -> (f32, f32) {
    let smooth = |x: f64| x * x * (3.0 - 2.0 * x);
    if t < NOTICE_SHOW {
        let icon = if t < NOTICE_ICON_IN { smooth(t / NOTICE_ICON_IN) } else { 1.0 };
        return (1.0, icon as f32);
    }
    let u = t - NOTICE_SHOW;
    if u >= NOTICE_HIDE {
        return (0.0, 0.0);
    }
    let group = 1.0 - smooth(u / NOTICE_HIDE);
    let icon = (-13.917 * u * u * u + 22.959 * u * u - 8.870 * u + 1.0).clamp(0.0, 1.0);
    (group as f32, icon as f32)
}

pub(super) const GLOW_SHADER: Handle<Shader> = uuid_handle!("3b7e2d14-9c58-4f61-a2d0-7e5c1b9f4a86");

/// The glow's picture added onto the screen as the game's `UI/Additive`
/// adds it (its material `UI Additive`: `Blend SrcAlpha One`, its colour
/// twice the vertex colour times its `_TintColor` of a half, times the
/// picture): the picture's colour times its alpha times the glow's alpha.
#[derive(Asset, AsBindGroup, TypePath, Debug, Clone)]
pub struct GlowMaterial {
    /// The glow's alpha now; and 1 where the screens are composed in gamma
    /// space (`gamma_ui`).
    #[uniform(0)]
    pub alpha: Vec4,
    /// The sprite's cut, in its texture's pixels from its lowest row: left,
    /// lowest, right, highest.
    #[uniform(1)]
    pub cut: Vec4,
    #[texture(2)]
    #[sampler(3)]
    pub texture: Handle<Image>,
}

impl UiMaterial for GlowMaterial {
    fn fragment_shader() -> ShaderRef {
        GLOW_SHADER.into()
    }

    fn specialize(descriptor: &mut RenderPipelineDescriptor, _key: bevy::ui_render::ui_material::UiMaterialKey<Self>) {
        let add = BlendComponent { src_factor: BlendFactor::One, dst_factor: BlendFactor::One, operation: BlendOperation::Add };
        let keep = BlendComponent { src_factor: BlendFactor::Zero, dst_factor: BlendFactor::One, operation: BlendOperation::Add };
        if let Some(target) = descriptor.fragment.as_mut().and_then(|fragment| fragment.targets.first_mut()).and_then(Option::as_mut) {
            target.blend = Some(BlendState { color: add, alpha: keep });
        }
    }
}

/// What is read of the install for them: the notice's template and its
/// alert icon, the glow's place and each kind's picture and curve.
#[derive(Resource, Default)]
pub(super) struct Looks {
    pub(super) notice: Option<trp_data::interface::screen::NoticeLook>,
    /// The alert's icon and the plain notice's.
    pub(super) icons: [Option<(Handle<Image>, Rect, [f32; 2])>; 2],
    pub(super) glow_panel: Option<trp_data::interface::screen::ScenePanel>,
    pub(super) glows: Vec<(i32, Handle<Image>, Rect, Curve)>,
    coming: Option<Mutex<Receiver<Read>>>,
}

pub(super) type Picture = (trp_data::interface::screen::SpriteCut, trp_data::unity::extract::TextureData);

pub(super) enum Read {
    Notice(trp_data::interface::screen::NoticeLook, [Option<Picture>; 2]),
    Glow(trp_data::interface::screen::GlowLook, Vec<(i32, Picture, Curve)>),
}

/// Reads them away from the frame.
pub(super) fn start_reading(game: Res<crate::GameData>, mut looks: ResMut<Looks>) {
    let install = game.unity.install.clone();
    let (send, receive) = channel();
    looks.coming = Some(Mutex::new(receive));
    std::thread::spawn(move || {
        let Ok(mut pictures) = trp_data::interface::screen::ScenePictures::new(&install) else { return };
        let mut picture = |(file, id): &(String, i64)| -> Option<Picture> {
            let cut = pictures.sprite(file, *id).ok()?;
            let texture = pictures.texture(&cut.texture.0, cut.texture.1).ok()?;
            Some((cut, texture))
        };
        match trp_data::interface::screen::notice(&install) {
            Ok(notice) => {
                let icons = [notice.alert.clone(), notice.plain.clone()].map(|sprite| sprite.as_ref().and_then(&mut picture));
                let _ = send.send(Read::Notice(notice, icons));
            }
            Err(e) => eprintln!("the game's notices could not be read; none are drawn: {e:#}"),
        }
        match trp_data::interface::screen::malfunction_glow(&install) {
            Ok(glow) => {
                let kinds = glow.glows.iter().filter_map(|(kind, sprite, curve)| Some((*kind, picture(sprite)?, curve.clone()))).collect();
                let _ = send.send(Read::Glow(glow, kinds));
            }
            Err(e) => eprintln!("the game's malfunction glow could not be read; none is drawn: {e:#}"),
        }
    });
}

/// A picture made an image the screens sample, clamped at its edges.
fn image_of(texture: &trp_data::unity::extract::TextureData, images: &mut Assets<Image>) -> Option<Handle<Image>> {
    use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
    let mut image = crate::assets::render_data::build_image(texture, crate::assets::render_data::Role::Color).map_err(|e| warn!("a picture of the notices': {e:#}")).ok()?;
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::ClampToEdge,
        address_mode_v: ImageAddressMode::ClampToEdge,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Linear,
        ..default()
    });
    Some(images.add(image))
}

fn cut_of(cut: &trp_data::interface::screen::SpriteCut) -> Rect {
    let [x, y, w, h] = cut.rect;
    Rect::new(x, y, x + w, y + h)
}

pub(super) fn take_looks(mut looks: ResMut<Looks>, mut images: ResMut<Assets<Image>>) {
    let Some(coming) = looks.coming.take() else { return };
    let mut done = false;
    if let Ok(receive) = coming.lock() {
        loop {
            match receive.try_recv() {
                Ok(Read::Notice(notice, icons)) => {
                    looks.icons = icons.map(|icon| icon.and_then(|(cut, texture)| Some((image_of(&texture, &mut images)?, cut_of(&cut), [cut.whole[2], cut.whole[3]]))));
                    looks.notice = Some(notice);
                }
                Ok(Read::Glow(glow, kinds)) => {
                    looks.glows = kinds.into_iter().filter_map(|(kind, (cut, texture), curve)| Some((kind, image_of(&texture, &mut images)?, cut_of(&cut), curve))).collect();
                    looks.glow_panel = Some(glow.panel);
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    done = true;
                    break;
                }
            }
        }
    }
    if !done {
        looks.coming = Some(coming);
    }
}

/// A notice drawn: its number, and the window's height it was laid out
/// for; its box, its icon, its words and the frame of its words, by its
/// number.
#[derive(Component, PartialEq)]
pub(super) struct NoticeRoot(pub(super) u64, pub(super) u32);

#[derive(Component)]
pub(super) struct NoticeBox(pub(super) u64);

#[derive(Component)]
pub(super) struct NoticeIcon(pub(super) u64);

#[derive(Component)]
pub(super) struct NoticeWords(pub(super) u64);

#[derive(Component)]
pub(super) struct NoticeFrame(pub(super) u64);
