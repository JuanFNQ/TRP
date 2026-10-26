//! The weapon in hand's wear, malfunctions and heat in the app
//! (`docs/spec/malfunctions.md`, "In the app"): the weapon's logic is given
//! its item's durability and malfunction state as it is taken in hand
//! (`Firearm::set_wear`), is told each frame the round it would fire next,
//! its magazine, the skills and the arms, and its wear and state are kept
//! back on the item; what it says of its malfunctions is told to the
//! shooter as the game tells it (`PlayerOwnerInventoryController`'s
//! `ExamineMalfunction` and the rest): the notices (`NotificationManager`)
//! and the glow (`BattleUIMalfunctionGlow`), with the interface's sound; a
//! failure to feed's round is thrown onto the ground by its repair.
//!
//! The three switches (`DURABILITY`, `MALFUNCTIONS`, `OVERHEATING`) are the
//! logic's (`WearSwitches`): off, the weapon does not wear, never
//! malfunctions, stays cold.

use std::collections::VecDeque;
use std::sync::mpsc::{channel, Receiver};
use std::sync::Mutex;

use bevy::asset::{load_internal_asset, uuid_handle};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, BlendComponent, BlendFactor, BlendOperation, BlendState, RenderPipelineDescriptor};
use bevy::shader::ShaderRef;
use bevy::ui_render::prelude::{UiMaterial, UiMaterialPlugin};
use trp_data::curve::Curve;
use trp_sim::firearm::Wear;
use trp_sim::inventory::{Id, Item, Repairable};
use trp_sim::malfunction::{self, Malf, RoundFigures, Settings, Skills, WeaponFigures};
use trp_sim::skills::{Skill, ELITE};
use trp_sim::FirearmEvent;

use crate::controller::Controller;
use crate::inventory_view::{Carried, HandsHeld};
use crate::weapon::HandsRig;

pub struct WearPlugin;

impl Plugin for WearPlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(app, GLOW_SHADER, "malfunction_glow.wgsl", Shader::from_wgsl);
        app.add_plugins(UiMaterialPlugin::<GlowMaterial>::default())
            .init_resource::<Feedback>()
            .init_resource::<Looks>()
            .add_systems(Startup, start_reading)
            .add_systems(
                Update,
                // After the hands have settled what is in them, before the
                // weapon's clips play (what it is told here it acts on in the
                // next frame, as the magazine's count is).
                keep_wear.after(crate::inventory_view::ScreenDrawn).before(crate::animation::play_clips).run_if(crate::running),
            )
            .add_systems(Update, (take_looks, show_notices, show_glow).chain().after(keep_wear));
    }
}

/// The install's words the notices are made of (`Notification/...`, the
/// malfunctions' names, `Settings/NotSet` for a key not bound).
pub fn word_keys() -> Vec<String> {
    let mut keys: Vec<String> = ["Notification/MalfunctionOccurred", "Notification/MalfunctionRepairRequired", "Notification/MalfunctionExaminationRequired", "Settings/NotSet"].map(str::to_owned).into();
    keys.extend([Malf::Misfire, Malf::Jam, Malf::Feed, Malf::SoftSlide, Malf::HardSlide].map(|malf| malf.text_key().to_owned()));
    keys
}

/// The glow's three kinds (`BattleUIMalfunctionGlow.EGlowType`): a
/// malfunction made known or reminded of, its kind looked at, put right.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Glow {
    Examined,
    TypeExamined,
    Repaired,
}

/// What the screen shows of the weapon's malfunctions: the notices waiting
/// their turn (`NotifierView`'s queue: one shows at a time, the next once
/// the last has gone), the one showing and since when, and the glow last
/// shown, its kind, when, and its alpha's factor (`GetGlowAlphaMultiplier`).
#[derive(Resource, Default)]
pub struct Feedback {
    /// Each its words and whether it is an alert (`ENotificationIconType.
    /// Alert`, the malfunctions') or plain (`Default`, a repair's message).
    pub waiting: VecDeque<(String, bool)>,
    pub showing: Option<(String, bool, f64)>,
    pub glow: Option<(Glow, f64, f32)>,
    /// The glow's asks in their window (`ShowGlow`): its start and the
    /// count, which a glow shown puts far below.
    window: (f64, i32),
    /// A glow and its sound waiting their 150 ms (`ProcessMalfunctionExamined`).
    examined_at: Option<f64>,
}

impl Feedback {
    /// A glow asked for (`BattleUIMalfunctionGlow.ShowGlow`): a forced one
    /// shows at once; any other only at the `attempts`-th ask within 2 s
    /// of the first, a window opened again at an ask more than 2 s after
    /// its start. Whether it shows.
    fn ask_glow(&mut self, glow: Glow, forced: bool, now: f64, attempts: u32, alpha: f32) -> bool {
        if now - self.window.0 > 2.0 {
            self.window = (now, 0);
        }
        self.window.1 += 1;
        if self.window.1 < attempts as i32 && !forced {
            return false;
        }
        self.window.1 = -1000;
        self.glow = Some((glow, now, alpha));
        true
    }

    fn notice(&mut self, words: String) {
        info!("notice: {words}");
        self.waiting.push_back((words, true));
    }

    /// A plain notice (`NotificationManager.DisplayMessageNotification`).
    pub fn message(&mut self, words: String) {
        info!("notice: {words}");
        self.waiting.push_back((words, false));
    }
}

/// Which weapon's wear the logic in hand holds, by its item and its build,
/// and the seed the next is given.
#[derive(Default)]
struct Given {
    item: Option<(Id, String)>,
    seeds: i32,
    settings: Option<Settings>,
    /// The command line's malfunction has come.
    forced: bool,
    /// The durability and greatest last kept back on the item: one that
    /// differs from them was changed elsewhere (a repair), and goes to the
    /// logic.
    written: Option<[f32; 2]>,
}

/// Every part in a weapon's slots, depth first, but the rounds in its
/// chambers (`Weapon.Mods`: every mod in its slots, the magazine with them).
fn parts(item: &Item, out: &mut Vec<String>) {
    for (slot, kind) in item.slots.iter().zip(&item.kind.slots) {
        let Some(part) = slot else { continue };
        if kind.name.starts_with("patron_in_weapon") {
            continue;
        }
        out.push(part.kind.template.clone());
        parts(part, out);
    }
}

/// The weapon in hand given its wear, told what it needs, and its wear
/// kept back on its item; what it said of its malfunctions shown.
#[allow(clippy::too_many_arguments)]
fn keep_wear(
    mut carried: ResMut<Carried>,
    held: Res<HandsHeld>,
    game: Res<crate::GameData>,
    mut weapon: Option<ResMut<HandsRig>>,
    mut controller: ResMut<Controller>,
    features: Res<crate::options_view::Features>,
    skills: Option<Res<crate::skills::Skills>>,
    (words, bindings): (Res<crate::health_view::Words>, Res<crate::keys::Bindings>),
    time: Res<Time<Real>>,
    mut sounds: MessageWriter<crate::ui_sound::Sound>,
    mut feedback: ResMut<Feedback>,
    mut given: Local<Given>,
    options: Res<crate::Options>,
) {
    let now = time.elapsed_secs_f64();
    let events = std::mem::take(&mut controller.wear_events);
    let settings = given.settings.get_or_insert_with(|| {
        Settings::from_config(&game.db.config).unwrap_or_else(|e| {
            warn!("the globals' malfunction settings: {e:#}; the game's own as built in");
            Settings::default()
        })
    });
    let attempts = settings.glow_attempts;
    // The glow's sound and its forced glow, 150 ms after a malfunction
    // was made known (`ProcessMalfunctionExamined`).
    let alpha = glow_alpha(&controller);
    if feedback.examined_at.is_some_and(|at| now >= at) {
        feedback.examined_at = None;
        feedback.ask_glow(Glow::Examined, true, now, attempts, alpha);
        sounds.write(crate::ui_sound::Sound::Ui(crate::ui_sound::Ui::MalfunctionExamined));
    }
    let Some(firearm) = weapon.as_deref_mut().and_then(|rig| rig.firearm.as_mut()) else {
        given.item = None;
        return;
    };
    let Some(item) = held.item.and_then(|id| carried.equipment.item.find(id)).filter(|item| trp_sim::inventory::is(item, "Weapon")) else {
        given.item = None;
        return;
    };
    let id = item.id;
    // Taken in hand (or its build changed): the logic has its item's wear.
    let build = trp_sim::inventory::rig_key(item);
    if given.item.as_ref() != Some(&(id, build.clone())) || firearm.wear().is_none() {
        let mut list = Vec::new();
        parts(item, &mut list);
        let refs: Vec<&str> = list.iter().map(String::as_str).collect();
        let figures = WeaponFigures::of(&game.db, &item.kind.template, &refs);
        let [durability, most] = item.durability();
        given.seeds = given.seeds.wrapping_add(1);
        let seed = given.seeds;
        let settings = given.settings.clone().unwrap_or_default();
        // The command line's check: the first weapon taken in hand that
        // hot, as if just fired, and malfunctioning at its next shot.
        // (The heat once, kept on the item from then; the malfunction
        // asked again of a logic made afresh until it has come.)
        let mut state = item.malfunction;
        if let Some(heat) = options.heat.filter(|_| seed == 1) {
            (state.last_shot_heat, state.last_shot_time) = (heat, controller.raid_clock());
        }
        firearm.set_wear(Wear::new(figures, settings, durability, most, state, seed));
        given.written = Some([durability, most]);
        if let Some(malf) = options.malfunction.filter(|_| !given.forced) {
            firearm.force_malfunction(malf);
        }
        info!("{}: its wear {durability:.2} of {most:.2}, its malfunction {:?}", item.kind.name, item.malfunction.malf);
        given.item = Some((id, build));
    }
    // What it would fire next: the chambered round, else the magazine's
    // top; and the magazine's own chance (none with none fitted).
    let magazine = item.in_slot(trp_sim::inventory::MAGAZINE_SLOT);
    let round = trp_sim::inventory::chambered(item).map(|round| round.kind.template.clone()).or_else(|| magazine.and_then(|magazine| trp_sim::inventory::magazine_round_at(magazine, 0)).map(|kind| kind.template.clone()));
    let round = round.map_or_else(RoundFigures::default, |template| RoundFigures::of(&game.db, &template));
    let chance = magazine.map(|magazine| malfunction::magazine_chance(&game.db, &magazine.kind.template));
    firearm.set_wear_inputs(controller.raid_clock(), round, chance);
    let level = |skill: Skill| skills.as_deref().map_or(0, |skills| skills.level(skill));
    let troubleshooting = level(Skill::TroubleShooting);
    let skills = Skills { maintenance: level(Skill::WeaponTreatment) as f32, troubleshooting: troubleshooting as f32, troubleshooting_elite: troubleshooting >= ELITE };
    let conditions = controller.movement.conditions;
    let arms = u32::from(conditions.left_arm) + u32::from(conditions.right_arm);
    firearm.set_wear_skills(skills, arms, features.wear());
    // Its durability changed on the item elsewhere (a repair) goes to the
    // logic first.
    let on_item = item.repairable.map(|kept| [kept.durability, kept.max_durability]);
    if let (Some(now), Some(was)) = (on_item, given.written) {
        if now != was {
            firearm.set_durability(now[0], now[1]);
            given.written = Some(now);
        }
    }
    // Its wear and state back on the item, where they changed.
    if let Some(wear) = firearm.wear() {
        let (durability, most, state) = (wear.durability, wear.max_durability, wear.state);
        let kept = item.repairable.is_none_or(|kept| kept.durability == durability && kept.max_durability == most) && item.malfunction == state;
        if !kept {
            if let Some(item) = carried.equipment.item.find_mut(id) {
                if item.repairable.is_some() {
                    item.repairable = Some(Repairable { durability, max_durability: most });
                    given.written = Some([durability, most]);
                }
                item.malfunction = state;
            }
        }
    }
    // What it said, as the shooter is told.
    let name = |malf: Malf| words.word(malf.text_key(), "");
    let occurred = |malf: Malf| words.word("Notification/MalfunctionOccurred", "Malfunction: {0}").replace("{0}", &name(malf));
    let key = |binding: &str| bindings.notice_name(binding).unwrap_or_else(|| words.word("Settings/NotSet", "Not set"));
    for event in events {
        match event {
            // Made known as it happens (`ExamineMalfunction`, not known
            // before): the glow and its sound 150 ms on.
            FirearmEvent::Malfunctioned(malf) => {
                info!("the weapon malfunctioned: {malf:?}");
                given.forced = true;
                feedback.examined_at = Some(now + 0.15);
            }
            // Known: the glow asked for, and only where it shows, the
            // notice: fix it, or look at it first.
            FirearmEvent::MalfunctionReminder { malf, type_known } => {
                if feedback.ask_glow(Glow::Examined, false, now, attempts, alpha) {
                    let said = if type_known {
                        format!("{} - {}", words.word("Notification/MalfunctionRepairRequired", "Fix malfunction: {0}").replace("{0}", &name(malf)), key("CheckChamber"))
                    } else {
                        format!("{} - {}", words.word("Notification/MalfunctionExaminationRequired", "Malfunction! Examine your weapon"), key("ExamineWeapon"))
                    };
                    feedback.notice(said);
                }
            }
            // Its kind looked at (`ExamineMalfunctionType`): the first time
            // its glow too.
            FirearmEvent::MalfunctionExamined { malf, first } => {
                if first {
                    feedback.ask_glow(Glow::TypeExamined, true, now, attempts, alpha);
                }
                feedback.notice(occurred(malf));
            }
            // A misfire put right by a reload, a draw or the chamber
            // unloaded (`CallUnknownMalfunctionStartRepair`).
            FirearmEvent::MalfunctionCleared(malf) => {
                feedback.ask_glow(Glow::Repaired, true, now, attempts, alpha);
                feedback.notice(occurred(malf));
            }
            // The repair (`CallMalfunctionRepaired`).
            FirearmEvent::MalfunctionRepaired(_) => {
                feedback.ask_glow(Glow::Repaired, true, now, attempts, alpha);
            }
            // A failure to feed's round thrown out by its repair: on the
            // ground before the character, as the chamber's thrown round.
            FirearmEvent::FedRoundThrown => {
                let round = carried.equipment.item.find_mut(id).and_then(|item| item.fed_round.take());
                if let Some(kind) = round {
                    let m = &controller.movement;
                    let at = crate::inventory_view::beside(m.position, m.yaw, [crate::inventory_view::DROP_AHEAD, 0.2]);
                    carried.ground.push((Item::new(kind), at));
                    carried.note = "the round that failed to feed was thrown out: it lies on the ground".to_owned();
                }
            }
            FirearmEvent::Autoshot => info!("the hot weapon fired a round by itself"),
            _ => {}
        }
    }
}

/// The glow's alpha (`GetGlowAlphaMultiplier`): half, or whole with the
/// low health's effect on.
fn glow_alpha(controller: &Controller) -> f32 {
    let low = controller.health.as_ref().is_some_and(|health| health.active(trp_sim::health::Kind::LowHealth, None).is_some());
    if low { 1.0 } else { 0.5 }
}

// ==== The notices and the glow, drawn as the game's preloader draws them ====

/// How a notice comes and goes (`NotifierView`'s `Notification`, its
/// animator's controller `Item`, `sharedassets49` 113, at `Speed` 1 for a
/// notice of `ENotificationDurationType.Default`): its show (`ShowNotification`,
/// clip 100, 2.6667 s: its icon's alpha from 0 to 1 over the first third of
/// a second, flat at both ends), then at once (`AutoComplete`) its hide
/// (`HideNotification`, clip 99: its group's alpha from 1 to 0 over 0.25 s,
/// flat at both ends; its icon's own from 1 with a slope of -8.87 to 0),
/// whose event at 0.0833 s (`OnAnimationDone`) folds its row away over
/// 0.3 s, after which the next waiting is shown. Read from the install
/// (`local\notification_clip_100_s134.txt`, `_99_`; malfunctions spec,
/// "What the shooter is told").
const NOTICE_SHOW: f64 = 2.666_667;
const NOTICE_ICON_IN: f64 = 1.0 / 3.0;
const NOTICE_HIDE: f64 = 0.25;
const NOTICE_DONE: f64 = 0.083_333;
const NOTICE_FOLD: f64 = 0.3;

/// The notice's ground and words as the notifier's script colours them
/// (`NotificationView._defaultBackgroundColor`, `_defaultTextColor`).
const NOTICE_GROUND: [f32; 4] = [0.0, 0.0, 0.0, 200.0 / 255.0];
const NOTICE_WORDS: [f32; 4] = [1.0, 1.0, 1.0, 1.0];

/// The alphas of a notice's group and its icon, so long after it was shown.
fn notice_alphas(t: f64) -> (f32, f32) {
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

const GLOW_SHADER: Handle<Shader> = uuid_handle!("3b7e2d14-9c58-4f61-a2d0-7e5c1b9f4a86");

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
struct Looks {
    notice: Option<trp_data::screen::NoticeLook>,
    /// The alert's icon and the plain notice's.
    icons: [Option<(Handle<Image>, Rect, [f32; 2])>; 2],
    glow_panel: Option<trp_data::screen::ScenePanel>,
    glows: Vec<(i32, Handle<Image>, Rect, Curve)>,
    coming: Option<Mutex<Receiver<Read>>>,
}

type Picture = (trp_data::screen::SpriteCut, trp_data::unity::extract::TextureData);

enum Read {
    Notice(trp_data::screen::NoticeLook, [Option<Picture>; 2]),
    Glow(trp_data::screen::GlowLook, Vec<(i32, Picture, Curve)>),
}

/// Reads them away from the frame.
fn start_reading(game: Res<crate::GameData>, mut looks: ResMut<Looks>) {
    let install = game.unity.install.clone();
    let (send, receive) = channel();
    looks.coming = Some(Mutex::new(receive));
    std::thread::spawn(move || {
        let Ok(mut pictures) = trp_data::screen::ScenePictures::new(&install) else { return };
        let mut picture = |(file, id): &(String, i64)| -> Option<Picture> {
            let cut = pictures.sprite(file, *id).ok()?;
            let texture = pictures.texture(&cut.texture.0, cut.texture.1).ok()?;
            Some((cut, texture))
        };
        match trp_data::screen::notice(&install) {
            Ok(notice) => {
                let icons = [notice.alert.clone(), notice.plain.clone()].map(|sprite| sprite.as_ref().and_then(&mut picture));
                let _ = send.send(Read::Notice(notice, icons));
            }
            Err(e) => eprintln!("the game's notices could not be read; none are drawn: {e:#}"),
        }
        match trp_data::screen::malfunction_glow(&install) {
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
    let mut image = crate::render_data::build_image(texture, crate::render_data::Role::Color).map_err(|e| warn!("a picture of the notices': {e:#}")).ok()?;
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

fn cut_of(cut: &trp_data::screen::SpriteCut) -> Rect {
    let [x, y, w, h] = cut.rect;
    Rect::new(x, y, x + w, y + h)
}

fn take_looks(mut looks: ResMut<Looks>, mut images: ResMut<Assets<Image>>) {
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

/// The notice drawn: its words, whether an alert, when it was shown, and
/// the window's height it was laid out for.
#[derive(Component, PartialEq)]
struct NoticeRoot(String, bool, u64, u32);
#[derive(Component)]
struct NoticeBox;
#[derive(Component)]
struct NoticeIcon;
#[derive(Component)]
struct NoticeWords;

/// Shows the notices one at a time, each as the notifier lays out its
/// template at the screen's lower right (its box as wide as the scene's,
/// the words wrapped within it after its left padding, the icon in that
/// padding, the box over the screen's foot by the column's padding), and
/// fades it in and out; each with the notices' sound as it comes.
#[allow(clippy::too_many_arguments)]
fn show_notices(
    mut commands: Commands,
    mut feedback: ResMut<Feedback>,
    looks: Res<Looks>,
    health_look: Res<crate::health_view::HealthLook>,
    window: Query<&Window, With<bevy::window::PrimaryWindow>>,
    time: Res<Time<Real>>,
    drawn: Query<(Entity, &NoticeRoot)>,
    mut boxes: Query<&mut BackgroundColor, With<NoticeBox>>,
    mut icons: Query<&mut ImageNode, With<NoticeIcon>>,
    mut words: Query<&mut TextColor, With<NoticeWords>>,
    mut sounds: MessageWriter<crate::ui_sound::Sound>,
) {
    let now = time.elapsed_secs_f64();
    if feedback.showing.as_ref().is_some_and(|(_, _, at)| now - at >= NOTICE_SHOW + NOTICE_DONE + NOTICE_FOLD) {
        feedback.showing = None;
    }
    if feedback.showing.is_none() {
        if let Some((next, alert)) = feedback.waiting.pop_front() {
            feedback.showing = Some((next, alert, now));
            sounds.write(crate::ui_sound::Sound::Notification);
        }
    }
    let height = window.single().map_or(0, |window| window.height().round() as u32);
    let wanted = feedback.showing.as_ref().map(|(said, alert, at)| NoticeRoot(said.clone(), *alert, at.to_bits(), height));
    if drawn.iter().map(|(_, root)| root).next() != wanted.as_ref() {
        for (entity, _) in &drawn {
            commands.entity(entity).despawn();
        }
        if let (Some(root), Some(notice)) = (wanted, looks.notice.as_ref()) {
            info!("a notice drawn: {}", root.0);
            let icon = looks.icons[usize::from(!root.1)].as_ref();
            spawn_notice(&mut commands, root, notice, icon, &health_look);
        }
        return;
    }
    let Some((_, _, at)) = feedback.showing.as_ref() else { return };
    let (group, icon) = notice_alphas(now - at);
    for mut ground in &mut boxes {
        ground.0 = Color::srgba(NOTICE_GROUND[0], NOTICE_GROUND[1], NOTICE_GROUND[2], NOTICE_GROUND[3] * group);
    }
    for mut image in &mut icons {
        image.color = Color::srgba(1.0, 1.0, 1.0, icon * group);
    }
    for mut color in &mut words {
        color.0 = Color::srgba(NOTICE_WORDS[0], NOTICE_WORDS[1], NOTICE_WORDS[2], NOTICE_WORDS[3] * group);
    }
}

fn spawn_notice(commands: &mut Commands, root: NoticeRoot, notice: &trp_data::screen::NoticeLook, icon: Option<&(Handle<Image>, Rect, [f32; 2])>, health_look: &crate::health_view::HealthLook) {
    let s = root.3 as f32 / 1080.0;
    let said = root.0.clone();
    let content = &notice.panel.nodes[notice.content].layout;
    let icon_layout = &notice.panel.nodes[notice.icon].layout;
    let text = notice.panel.nodes[notice.text].text.as_ref();
    let text_layout = &notice.panel.nodes[notice.text].layout;
    let [left, right, top, bottom] = notice.padding;
    let whole = Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() };
    let plate = Node {
        position_type: PositionType::Absolute,
        right: Val::Px(0.0),
        bottom: Val::Px(notice.below * s),
        width: Val::Px(content.size_delta[0] * s),
        padding: UiRect { left: Val::Px(left * s), right: Val::Px(right * s), top: Val::Px(top * s), bottom: Val::Px(bottom * s) },
        flex_direction: FlexDirection::Row,
        align_items: AlignItems::Center,
        ..default()
    };
    let ground = Color::srgba(NOTICE_GROUND[0], NOTICE_GROUND[1], NOTICE_GROUND[2], NOTICE_GROUND[3]);
    commands.spawn((root, whole, GlobalZIndex(45))).with_children(|layer| {
        layer.spawn((NoticeBox, plate, BackgroundColor(ground))).with_children(|plate| {
            // The icon, hung in the left padding, its right edge so far in,
            // kept in its shape within its square (`preserveAspect`).
            if let Some((image, cut, [w, h])) = icon {
                let side = icon_layout.size_delta;
                let fit = (side[0] / w.max(1.0)).min(side[1] / h.max(1.0));
                let [w, h] = [w * fit, h * fit];
                let right_edge = icon_layout.offset[0] + side[0] * (1.0 - icon_layout.pivot[0]);
                let node = Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px((right_edge - side[0] + (side[0] - w) / 2.0) * s),
                    top: Val::Percent(50.0),
                    margin: UiRect { top: Val::Px(-h / 2.0 * s), ..default() },
                    width: Val::Px(w * s),
                    height: Val::Px(h * s),
                    ..default()
                };
                plate.spawn((NoticeIcon, node, ImageNode { image: image.clone(), color: Color::srgba(1.0, 1.0, 1.0, 0.0), rect: Some(*cut), flip_y: true, ..default() }));
            }
            let (size, margin, font) = match text {
                Some(text) => (text.size, text.margin, health_look.styled_font(&notice.panel, text.font, text.style)),
                None => (16.0, [0.0, 4.0, 0.0, 5.0], Handle::default()),
            };
            let frame = Node {
                width: Val::Px(text_layout.size_delta[0] * s),
                padding: UiRect { left: Val::Px(margin[0] * s), top: Val::Px(margin[1] * s), right: Val::Px(margin[2] * s), bottom: Val::Px(margin[3] * s) },
                ..default()
            };
            let color = Color::srgba(NOTICE_WORDS[0], NOTICE_WORDS[1], NOTICE_WORDS[2], NOTICE_WORDS[3]);
            plate.spawn(frame).with_children(|frame| {
                frame.spawn((NoticeWords, Text::new(said), TextFont { font, font_size: size * s, ..default() }, TextColor(color), TextLayout::new_with_linebreak(LineBreak::WordBoundary)));
            });
        });
    });
}

/// The glow drawn: its kind's number, when it began, the window's height.
#[derive(Component, PartialEq)]
struct GlowRoot(i32, u64, u32);

/// The glow (`ProcessGlow`): its kind's picture where the scene places the
/// glow (the screen's lower right corner, past its edges), its alpha the
/// kind's curve by the wall clock's seconds since it began times its
/// factor, until the curve's last key.
fn show_glow(
    mut commands: Commands,
    feedback: Res<Feedback>,
    looks: Res<Looks>,
    health_look: Res<crate::health_view::HealthLook>,
    window: Query<&Window, With<bevy::window::PrimaryWindow>>,
    time: Res<Time<Real>>,
    drawn: Query<(Entity, &GlowRoot, &MaterialNode<GlowMaterial>)>,
    mut materials: ResMut<Assets<GlowMaterial>>,
) {
    let now = time.elapsed_secs_f64();
    let number = |glow: Glow| match glow {
        Glow::Examined => 0,
        Glow::Repaired => 1,
        Glow::TypeExamined => 2,
    };
    let shown = feedback.glow.and_then(|(glow, at, factor)| {
        let (_, image, cut, curve) = looks.glows.iter().find(|(kind, ..)| *kind == number(glow))?;
        let t = (now - at) as f32;
        (t <= curve.keys.last().map_or(0.0, |key| key.time)).then(|| (number(glow), at, image, cut, factor * curve.evaluate(t)))
    });
    let Ok(window) = window.single() else { return };
    let height = window.height().round() as u32;
    let wanted = shown.as_ref().map(|(kind, at, ..)| GlowRoot(*kind, at.to_bits(), height));
    let gamma = if health_look.tarkov_ui { 1.0 } else { 0.0 };
    if drawn.iter().map(|(_, root, _)| root).next() != wanted.as_ref() {
        for (entity, ..) in &drawn {
            commands.entity(entity).despawn();
        }
        let (Some(root), Some((_, _, image, cut, alpha)), Some(panel)) = (wanted, shown.as_ref(), looks.glow_panel.as_ref()) else { return };
        let s = height as f32 / 1080.0;
        let [x, y, w, h] = panel.placed_in(0, [window.width() / s, 1080.0]).rect;
        let material = materials.add(GlowMaterial { alpha: Vec4::new(*alpha, gamma, 0.0, 0.0), cut: Vec4::new(cut.min.x, cut.min.y, cut.max.x, cut.max.y), texture: (*image).clone() });
        let node = Node { position_type: PositionType::Absolute, left: Val::Px(x * s), top: Val::Px(y * s), width: Val::Px(w * s), height: Val::Px(h * s), ..default() };
        info!("the malfunction glow drawn: kind {}", root.0);
        commands.spawn((root, node, MaterialNode(material), GlobalZIndex(44)));
        return;
    }
    if let (Some((.., alpha)), Some((_, _, handle))) = (shown, drawn.iter().next()) {
        if let Some(material) = materials.get_mut(&handle.0) {
            material.alpha.x = alpha;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A reminder's glow shows at the third ask within 2 s of the first,
    /// then no more in that window; a forced one shows at once
    /// (`BattleUIMalfunctionGlow.ShowGlow`).
    #[test]
    fn the_glow_shows_at_the_third_ask_in_its_window() {
        let mut feedback = Feedback::default();
        let ask = |feedback: &mut Feedback, at: f64| feedback.ask_glow(Glow::Examined, false, at, 3, 0.5);
        assert!(!ask(&mut feedback, 10.0) && !ask(&mut feedback, 10.5));
        assert!(ask(&mut feedback, 11.0), "the third within 2 s");
        assert!(!ask(&mut feedback, 11.5) && !ask(&mut feedback, 11.9), "none more in the window");
        assert!(!ask(&mut feedback, 12.5), "a new window from here");
        assert!(feedback.ask_glow(Glow::Repaired, true, 12.6, 3, 0.5), "forced");
        assert_eq!(feedback.glow.map(|(glow, ..)| glow), Some(Glow::Repaired));
    }

    /// A notice's alphas: its icon in over a third of a second, all of it
    /// out over a quarter after 2.667 s.
    #[test]
    fn a_notice_comes_and_goes() {
        assert_eq!(notice_alphas(0.0), (1.0, 0.0));
        assert!((notice_alphas(NOTICE_ICON_IN / 2.0).1 - 0.5).abs() < 1e-5);
        assert_eq!(notice_alphas(1.0), (1.0, 1.0));
        let (group, _) = notice_alphas(NOTICE_SHOW + NOTICE_HIDE / 2.0);
        assert!((group - 0.5).abs() < 1e-5);
        assert_eq!(notice_alphas(NOTICE_SHOW + NOTICE_HIDE), (0.0, 0.0));
    }
}
