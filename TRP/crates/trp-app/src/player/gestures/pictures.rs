//! The menu's pictures read, the situations and a phrase played.

use super::*;

/// The menu's pictures once read: the sprites, then the icons.
#[derive(Resource, Default)]
pub(super) struct Pictures {
    pub(super) sprites: Vec<Option<(Handle<Image>, Rect)>>,
    pub(super) icons: Vec<Option<(Handle<Image>, Rect)>>,
    coming: Option<std::sync::Mutex<std::sync::mpsc::Receiver<Read>>>,
}

pub(super) type Picture = (trp_data::interface::screen::SpriteCut, trp_data::unity::extract::TextureData);

pub(super) struct Read {
    pub(super) sprites: Vec<Option<Picture>>,
    pub(super) icons: Vec<Option<Picture>>,
}

/// Reads the menu's sprites and the hand signs' icons away from the frame.
pub(super) fn start_reading(game: Res<crate::GameData>, mut pictures: ResMut<Pictures>, features: Res<crate::screens::options_view::Features>) {
    if !features.gestures {
        return;
    }
    let install = game.unity.install.clone();
    let (send, receive) = std::sync::mpsc::channel();
    pictures.coming = Some(std::sync::Mutex::new(receive));
    std::thread::spawn(move || {
        let sprite_ids = trp_data::interface::screen::sprites_named(&install, "sharedassets44.assets", SPRITES);
        let icon_ids = trp_data::interface::screen::sprites_named(&install, "resources.assets", ICONS);
        let Ok(mut pictures) = trp_data::interface::screen::ScenePictures::new(&install) else { return };
        let mut picture = |found: &Option<(String, i64)>| -> Option<Picture> {
            let (file, id) = found.as_ref()?;
            let cut = pictures.sprite(file, *id).ok()?;
            let texture = pictures.texture(&cut.texture.0, cut.texture.1).ok()?;
            Some((cut, texture))
        };
        let sprites = sprite_ids.iter().map(&mut picture).collect();
        let icons = icon_ids.iter().map(&mut picture).collect();
        let _ = send.send(Read { sprites, icons });
    });
}

/// The pictures made images when they come.
pub(super) fn take_pictures(mut pictures: ResMut<Pictures>, mut images: ResMut<Assets<Image>>, mut gestures: ResMut<Gestures>) {
    let Some(coming) = pictures.coming.take() else { return };
    let read = match coming.lock().ok().map(|receive| receive.try_recv()) {
        Some(Ok(read)) => read,
        Some(Err(std::sync::mpsc::TryRecvError::Empty)) => {
            pictures.coming = Some(coming);
            return;
        }
        _ => return,
    };
    let mut made = |picture: Option<Picture>| -> Option<(Handle<Image>, Rect)> {
        let (cut, texture) = picture?;
        let handle = image_of(&texture, &mut images)?;
        let [x, y, w, h] = cut.rect;
        Some((handle, Rect::new(x, y, x + w, y + h)))
    };
    pictures.sprites = read.sprites.into_iter().map(&mut made).collect();
    pictures.icons = read.icons.into_iter().map(&mut made).collect();
    info!(
        "the gestures' pictures: {} of {} sprites, {} of {} icons",
        pictures.sprites.iter().flatten().count(),
        SPRITES.len(),
        pictures.icons.iter().flatten().count(),
        ICONS.len()
    );
    // Drawn again with them.
    (gestures.menu_drawn, gestures.prompt_drawn) = (None, None);
}

/// A picture made an image the screens sample, clamped at its edges.
fn image_of(texture: &trp_data::unity::extract::TextureData, images: &mut Assets<Image>) -> Option<Handle<Image>> {
    use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
    let mut image = crate::assets::render_data::build_image(texture, crate::assets::render_data::Role::Color).map_err(|e| warn!("a picture of the gestures': {e:#}")).ok()?;
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

/// The situational phrases (`GesturesQuickPanel`'s handlers): by the whole
/// body's health, the effects, what is looked at, and the situations the
/// hands raised (5 s each). A lasting situation newly holding (or its
/// source changed: another thing looked at, another effect begun) adds its
/// phrase or restores its priority; one no longer holding takes it away.
/// `PROVISIONAL`: the game hears each change as it is raised; the app
/// looks once a frame; a residual effect counts as holding.
#[allow(clippy::too_many_arguments)]
pub(super) fn situations(
    mut gestures: ResMut<Gestures>,
    mut controller: ResMut<Controller>,
    reach: Res<crate::screens::inventory_view::Reach>,
    carried: Res<crate::screens::inventory_view::Carried>,
    range: Option<Res<crate::world::range::Range>>,
    voiced: Res<Voiced>,
    time: Res<Time<Real>>,
) {
    use phrase::*;
    use trp_sim::condition::health::{Kind, Part};
    let now = time.elapsed_secs_f64();
    let gestures = &mut *gestures;
    let mut holding: HashSet<i32> = HashSet::new();
    let mut effects = [0usize; 5];
    if let Some(health) = controller.health.as_ref().filter(|health| health.alive) {
        let [current, maximum] = health.common();
        let whole = 100.0 * current / maximum.max(1e-6);
        let hurt = match whole {
            w if (0.0..=15.0).contains(&w) => Some(HURT_NEAR_DEATH),
            w if w > 15.0 && w <= 40.0 => Some(HURT_HEAVY),
            w if w > 40.0 && w <= 70.0 => Some(HURT_MEDIUM),
            w if w > 70.0 && w < 90.0 => Some(HURT_LIGHT),
            _ => None,
        };
        holding.extend(hurt);
        let count = |kinds: &[Kind], parts: &[Part]| parts.iter().map(|part| kinds.iter().filter(|kind| health.active(**kind, Some(*part)).is_some()).count()).sum::<usize>();
        effects = [
            count(&[Kind::LightBleeding, Kind::HeavyBleeding], &Part::ALL),
            count(&[Kind::Fracture], &[Part::LeftLeg, Part::RightLeg]),
            count(&[Kind::Fracture], &[Part::LeftArm, Part::RightArm]),
            usize::from(health.active(Kind::Dehydration, None).is_some()),
            usize::from(health.active(Kind::Exhaustion, None).is_some()),
        ];
        for (n, trigger) in [BLEEDING, LEG_BROKEN, HAND_BROKEN, DEHYDRATED, EXHAUSTED].into_iter().enumerate() {
            if effects[n] > 0 {
                holding.insert(trigger);
            }
        }
    }
    // What is looked at: a loose thing by its kind, a body, a container, a
    // door (locked or shut as well).
    let mut looked = None;
    if let Some(id) = reach.loose {
        if let Some((item, _)) = carried.ground.iter().find(|(item, _)| item.id == id) {
            let is = |class: &str| trp_sim::items::inventory::is(item, class);
            let trigger = if is("Key") {
                LOOT_KEY
            } else if is("Money") {
                LOOT_MONEY
            } else if is("Weapon") || is("Knife") {
                LOOT_WEAPON
            } else {
                LOOT_GENERIC
            };
            holding.insert(trigger);
            looked = Some((0, id as u64));
        }
    } else if let Some(body) = reach.body {
        holding.extend([LOOT_BODY, CHECK_HIM]);
        looked = Some((1, body as u64));
    } else if let (Some(at), Some(range)) = (reach.thing, range.as_deref()) {
        if let Some(placed) = range.things.get(at) {
            use trp_sim::held::things::{Kind as ThingKind, ThingState};
            match placed.thing.kind {
                ThingKind::Door => {
                    holding.insert(OPEN_DOOR);
                    if matches!(placed.thing.state(), ThingState::Locked | ThingState::Shut) {
                        holding.insert(LOCKED_DOOR);
                    }
                }
                // A trunk is no `LootableContainer` in the game.
                ThingKind::Container => {
                    holding.insert(LOOT_CONTAINER);
                }
                ThingKind::Trunk => {}
            }
            looked = Some((2, at as u64));
        }
    }
    let fresh_look = looked != gestures.looked;
    let fresh_effects: Vec<bool> = effects.iter().zip(gestures.effects).map(|(now, was)| *now > was).collect();
    let voiced = &voiced.0;
    const BY_EFFECT: [i32; 5] = [BLEEDING, LEG_BROKEN, HAND_BROKEN, DEHYDRATED, EXHAUSTED];
    const BY_LOOK: [i32; 9] = [LOOT_KEY, LOOT_MONEY, LOOT_WEAPON, LOOT_GENERIC, LOOT_BODY, CHECK_HIM, LOOT_CONTAINER, OPEN_DOOR, LOCKED_DOOR];
    let lasting = [HURT_NEAR_DEATH, HURT_HEAVY, HURT_MEDIUM, HURT_LIGHT].into_iter().chain(BY_EFFECT).chain(BY_LOOK);
    for trigger in lasting {
        let now_holds = holding.contains(&trigger);
        let looked_kind = BY_LOOK.contains(&trigger);
        let effect_at = BY_EFFECT.iter().position(|t| *t == trigger);
        let fresh = !gestures.held.contains(&trigger) || (looked_kind && fresh_look) || effect_at.is_some_and(|at| fresh_effects[at]);
        // A timed situation of the same phrase keeps it.
        let timed = gestures.timed.iter().any(|(t, _)| *t == trigger);
        if now_holds && fresh {
            gestures.activate(trigger, voiced);
        } else if !now_holds && gestures.held.contains(&trigger) && !timed {
            gestures.deactivate(trigger);
        }
    }
    gestures.held = holding;
    gestures.looked = looked;
    gestures.effects = effects;
    // The hands' situations, 5 s each (`PlayerPhraseSituation`).
    for trigger in std::mem::take(&mut controller.phrase_situations) {
        gestures.raise(trigger, now, voiced);
    }
    gestures.expire(now);
}

/// Plays what was chosen (`GamePlayerOwner.PlayPhraseOrGesture`): a hand
/// sign shown by the body's gestures, a phrase asked of the voice if it may
/// be said now.
pub(super) fn play(number: i32, aggressive: bool, gestures: &Gestures, controller: &mut Controller, asked: &mut MessageWriter<PhraseAsked>) {
    if number == 0 {
        return;
    }
    if let Some(sign) = is_sign(number) {
        let shown = controller.movement.set_interaction(sign);
        info!("gestures: {sign:?}{}", if shown { "" } else { " refused" });
    } else if gestures.available(number) {
        asked.write(PhraseAsked { trigger: number, aggressive });
        info!("gestures: the phrase {} ({number}){}", phrase::name(number), if aggressive { ", aggressive" } else { "" });
    } else {
        info!("gestures: the phrase {} is not available now", phrase::name(number));
    }
}
