//! The contusion and eye-burn screens, the hits received, and their update each frame.

use super::*;

pub(super) const CONTUSION_SHADER: Handle<Shader> = uuid_handle!("3b8d5f21-9c47-4e6a-b0d2-7a1e5c9f4b68");

pub(super) const EYE_BURN_SHADER: Handle<Shader> = uuid_handle!("8e2c4a71-3f95-4d0b-a6c8-1b7e9d5f2a43");

/// What the camera's picture shows this frame of a contusion (each `*_on`
/// 1 where that effect is on).
#[derive(Component, Clone, Copy, Default, ShaderType)]
pub struct ContusionScreen {
    pub displace: Vec2,
    pub amount: f32,
    pub timer: f32,
    pub scale: f32,
    pub strength: f32,
    pub keep: f32,
    pub double_on: f32,
    pub wiggle_on: f32,
    pub blur_on: f32,
    pub fresh: f32,
    /// The flash's white (`_Coef`), and 1 while it is on.
    pub flash: f32,
    pub flash_on: f32,
    /// 1 while the eyes' burn is on (`EyeBurnScreen`).
    pub burn_on: f32,
    /// Pain's blur's amount, 1 while it draws, and its centre.
    pub radial: f32,
    pub radial_on: f32,
    pub radial_center: Vec2,
    /// 1 while exhaustion's vignette draws; near death's amount.
    pub vignette_on: f32,
    pub near_death: f32,
    /// The vignette's `_Data`: its centre, its sharpness × 0.01 and its
    /// darkness × 0.02 (`CC_FastVignette`).
    pub vignette: Vec4,
    /// 1 while near death's blend draws.
    pub near_death_on: f32,
}

impl ContusionScreen {
    fn any(&self) -> bool {
        self.double_on > 0.0 || self.wiggle_on > 0.0 || self.blur_on > 0.0 || self.flash_on > 0.0 || self.burn_on > 0.0 || self.radial_on > 0.0 || self.vignette_on > 0.0 || self.near_death_on > 0.0
    }
}

/// A hit as the player is told of it (`Player.ReceiveDamage`: a shot, or
/// damage given to the player that no weapon did, as barbed wire's and a
/// grenade's), with what armour took off it.
#[derive(Message, Clone, Copy)]
pub struct Received {
    pub part: Part,
    pub damage: f32,
    pub absorbed: f32,
    pub kind: DamageKind,
    /// The stamina the hit burns (the round's figure a point times the
    /// damage, `DamageInfo.StaminaBurnRate * Damage`): what a weapon's hit
    /// slows the character by (`Movement::hit`). Nought for what no round did.
    pub burn: f32,
}

/// The most spots the burn draws at once (the oldest let go past them).
const BURN_SPOTS: usize = 32;

/// The eyes' burn's spots this frame, two rows each: its middle and half
/// size (the screen's terms, y up), then its turn, its life left and its
/// picture; how many there are.
#[derive(Component, Clone, Copy, ShaderType)]
pub struct EyeBurnScreen {
    pub spots: [Vec4; 2 * BURN_SPOTS],
    pub count: f32,
    pub _pad: Vec3,
}

impl Default for EyeBurnScreen {
    fn default() -> Self {
        Self { spots: [Vec4::ZERO; 2 * BURN_SPOTS], count: 0.0, _pad: Vec3::ZERO }
    }
}

impl ExtractComponent for EyeBurnScreen {
    type QueryData = &'static EyeBurnScreen;
    type QueryFilter = ();
    type Out = EyeBurnScreen;

    fn extract_component(burn: QueryItem<'_, '_, Self::QueryData>) -> Option<Self::Out> {
        (burn.count > 0.0).then_some(*burn)
    }
}

/// The camera's pictures once read: the burn's (`trp_data::eye_burn`: the
/// spots', then the trail's), and near death's (`screen_neardeath_MULT`).
#[derive(Resource, Clone, Default, ExtractResource)]
pub struct ScreenImages {
    pub burn: Vec<Handle<Image>>,
    pub near_death: Option<Handle<Image>>,
    /// Blood's drop (`BloodDrop`'s `_MainTex`).
    pub blood_drop: Option<Handle<Image>>,
}

/// The eyes' burned (`DoBurnEyes`'s `BurnEyesEvent`): where the blast is,
/// in the game's axes, and the distance's share it burns with.
#[derive(Message, Clone, Copy)]
pub struct BurnedEyes {
    pub at: [f32; 3],
    pub strength: f32,
}

/// The burn: its spots once its figures have come, what brings them, the
/// view's turn as the frame before left it, and the draws.
#[derive(Resource)]
pub struct EyeBurnState {
    pub(super) burn: Option<EyeBurn>,
    coming: Option<Mutex<Receiver<trp_data::drawing::camera_effects::CameraEffects>>>,
    pub(super) view: Option<[f32; 2]>,
    seed: u32,
}

/// Starts reading the camera's effects (the burn's figures and pictures,
/// health's, near death's picture) away from the frame.
pub fn start_reading(app: &mut App, install: trp_data::Install) {
    let (send, receive) = channel();
    std::thread::spawn(move || match trp_data::drawing::camera_effects::read(&install) {
        Ok(read) => {
            let _ = send.send(read);
        }
        Err(e) => eprintln!("the camera's effects could not be read; a flash leaves no burn, and hits, pain, exhaustion and near death show nothing on the screen: {e:#}"),
    });
    app.insert_resource(EyeBurnState { burn: None, coming: Some(Mutex::new(receive)), view: None, seed: 0x4255_524e });
    app.init_resource::<ScreenImages>();
}

/// Each hit of the frame (`EffectsController.OnPlayerDamaged`): the view
/// pushed (`Hands::add_force`), while alive the hit's blur begun, and blood
/// on the screen (`BloodOnScreen.Hit`: a draw in the unit ball laid flat,
/// turned into the camera's axes; for the head a second at its bleeding
/// centre).
pub fn hits(
    mut received: MessageReader<Received>,
    controller: Res<crate::player::controller::Controller>,
    mut state: ResMut<ContusionState>,
    mut hands: Option<ResMut<crate::weapons::weapon::HandsRig>>,
    mut blood: Option<ResMut<crate::effects::blood_view::BloodState>>,
    camera: Query<&GlobalTransform, With<crate::MainCamera>>,
    time: Res<Time>,
) {
    let now = time.elapsed_secs();
    let turn = camera.single().map_or(Quat::IDENTITY, |place| place.compute_transform().rotation);
    for hit in received.read() {
        let painkillers = controller.health.as_ref().is_some_and(|health| health.conditions().on_painkillers);
        let Some(reaction) = hit_reaction(hit.damage, hit.absorbed, hit.part, hit.kind, painkillers) else { continue };
        if let Some(hands) = hands.as_mut() {
            hands.hands.add_force(reaction.strength, reaction.hands, reaction.camera);
        }
        if controller.health.as_ref().is_some_and(|health| health.alive) {
            state.0.hit(reaction.blur_power);
        }
        if !reaction.blood {
            continue;
        }
        let Some(state) = blood.as_mut() else { continue };
        let head = state.centre(Part::Head);
        let Some(blood) = state.blood.as_mut() else { continue };
        // In the game's axes (z ahead), turned by the camera's own.
        let ball = blood.ball();
        let local = turn.inverse() * Vec3::new(ball[0], 0.0, -ball[2]);
        let local = Vec3::new(local.x, local.y, -local.z).normalize_or_zero();
        let drawn = blood.hit(local.to_array(), now);
        info!("blood on the screen: a hit to the {}: {}", hit.part.name(), if drawn { "a drop placed" } else { "none this time" });
        if hit.part == Part::Head {
            let direction = blood.about(head, 1.0 / 1.5);
            blood.hit(direction, now);
        }
    }
}

/// The burn's frame: its figures and pictures taken in as they come, each
/// burn of the frame laid where its blast is on the screen (`EyeBurn.Burn`,
/// with the view's turn since the frame before), and its spots handed to
/// the camera.
#[allow(clippy::too_many_arguments)]
pub fn eye_burn(
    mut burned: MessageReader<BurnedEyes>,
    mut state: ResMut<EyeBurnState>,
    mut images: ResMut<ScreenImages>,
    mut assets: ResMut<Assets<Image>>,
    mut effects: ResMut<ContusionState>,
    mut blood: Option<ResMut<crate::effects::blood_view::BloodState>>,
    game: Res<crate::GameData>,
    controller: Res<crate::player::controller::Controller>,
    time: Res<Time>,
    camera: Query<(&Camera, &GlobalTransform), With<crate::MainCamera>>,
    mut screens: Query<(&mut EyeBurnScreen, &mut ContusionScreen)>,
) {
    let state = &mut *state;
    if let Some(coming) = state.coming.take() {
        match coming.lock().ok().map(|receive| receive.try_recv()) {
            Some(Ok(camera)) => {
                // Read as stored, as the game's gamma-space project reads
                // them (`gamma_ui`): no sRGB curve undone.
                let raw = |picture: &trp_data::unity::extract::TextureData| crate::assets::render_data::build_image(&trp_data::unity::extract::TextureData { srgb: false, ..picture.clone() }, crate::assets::render_data::Role::Color);
                let read = camera.eye_burn;
                let made: Vec<Handle<Image>> = read
                    .spots
                    .iter()
                    .chain(&read.trail)
                    .filter_map(|picture| raw(picture).map_err(|e| warn!("a picture of the eyes' burn: {e:#}")).ok())
                    .map(|image| assets.add(image))
                    .collect();
                info!("the eyes' burn: {} spot pictures and {} of its trail", read.spots.len(), read.trail.len());
                images.burn = made;
                images.near_death = raw(&camera.near_death).map_err(|e| warn!("near death's picture: {e:#}")).ok().map(|image| assets.add(image));
                let h = &camera.health;
                info!(
                    "health's screen effects: a hit's blur over {} s, pain's blur to {}, exhaustion's vignette to {}, near death's picture {} by {}",
                    h.hit_time, h.radial_most, h.vignette_darkness, camera.near_death.width, camera.near_death.height
                );
                effects.0.set_figures(camera.health);
                // Blood on the screen, started now (`BloodOnScreen.Start`,
                // its pre-warm), with the hard settings' bleeding centres.
                images.blood_drop = raw(&camera.blood_drop).map_err(|e| warn!("blood's drop: {e:#}")).ok().map(|image| assets.add(image));
                if let Some(blood) = blood.as_mut() {
                    blood.centres = game.hard.vec3s("BleedingCenters").map(<[[f32; 3]]>::to_vec).unwrap_or_else(|e| {
                        warn!("{e:#}: blood from a bleeding comes from below");
                        Vec::new()
                    });
                    blood.blood = Some(trp_sim::effects::blood::Blood::new(camera.blood, time.elapsed_secs(), 0x424c_4f4f));
                }
                state.burn = Some(EyeBurn::new(EyeBurnFigures {
                    scale: read.scale,
                    max_time: read.max_time,
                    trail_every: read.trail_every,
                    trail_shift: read.trail_shift,
                    max_trail: read.max_trail,
                    trail_curve: read.trail_curve,
                    scale_curve: read.scale_curve,
                    spots: read.spots.len(),
                    trails: read.trail.len(),
                }));
            }
            Some(Err(std::sync::mpsc::TryRecvError::Empty)) => state.coming = Some(coming),
            _ => {}
        }
    }
    // The view's turn this frame (`MovementContext.PreviousRotation −
    // Rotation`, degrees across and up).
    let view = [controller.movement.yaw, controller.movement.pitch];
    let turn = state.view.map_or([0.0, 0.0], |before| [before[0] - view[0], before[1] - view[1]]);
    state.view = Some(view);
    let now = time.elapsed_secs();
    let Some(burn) = state.burn.as_mut() else {
        burned.clear();
        return;
    };
    let Ok((camera, place)) = camera.single() else { return };
    for eyes in burned.read() {
        let at = Vec3::new(eyes.at[0], eyes.at[1], -eyes.at[2]);
        let ahead = (at - place.translation()).dot(place.forward().as_vec3());
        let Some(ndc) = camera.world_to_ndc(place, at) else { continue };
        let screen = [(ndc.x + 1.0) / 2.0, (ndc.y + 1.0) / 2.0, ahead];
        let aspect = camera.logical_viewport_size().map_or(16.0 / 9.0, |size| size.x / size.y.max(1.0));
        let seed = &mut state.seed;
        let mut random = || {
            *seed ^= *seed << 13;
            *seed ^= *seed >> 17;
            *seed ^= *seed << 5;
            (*seed >> 8) as f32 / (1u32 << 24) as f32
        };
        let laid = burn.burn(screen, eyes.strength, turn, aspect, now, &mut random);
        info!("the eyes burnt at {:.2} {:.2} of the screen by {:.2}: {}", screen[0], screen[1], eyes.strength, if laid { "a spot and its trail" } else { "out of sight, none" });
    }
    let spots = burn.frame(time.delta_secs(), now);
    let skip = spots.len().saturating_sub(BURN_SPOTS);
    let mut shown = EyeBurnScreen::default();
    for (n, spot) in spots.iter().skip(skip).enumerate() {
        shown.spots[2 * n] = Vec4::new(spot.middle[0], spot.middle[1], spot.half[0], spot.half[1]);
        shown.spots[2 * n + 1] = Vec4::new(spot.turn, spot.coef, spot.picture as f32, 0.0);
    }
    shown.count = (spots.len() - skip) as f32;
    let on = burn.on();
    for (mut burn_screen, mut screen) in &mut screens {
        *burn_screen = shown;
        screen.burn_on = if on && shown.count > 0.0 { 1.0 } else { 0.0 };
    }
}

impl ExtractComponent for ContusionScreen {
    type QueryData = &'static ContusionScreen;
    type QueryFilter = ();
    type Out = ContusionScreen;

    /// Only while one is on: with none the picture goes through untouched.
    fn extract_component(screen: QueryItem<'_, '_, Self::QueryData>) -> Option<Self::Out> {
        screen.any().then_some(*screen)
    }
}

/// The accumulators' state (`trp_sim::screen_effects`).
#[derive(Resource, Default)]
pub struct ContusionState(ScreenEffects);

impl ContusionState {
    /// The desaturation's health share, mask and radius (`sharpen_view`).
    pub fn desaturation(&self) -> [f32; 3] {
        self.0.desaturation()
    }

    /// The painkiller's ring's radii as the level's camera has them.
    pub fn set_radii(&mut self, radii: [f32; 2]) {
        self.0.set_desaturation_radii(radii);
    }

    /// The camera's sharpening from the player's own, a berserk's share
    /// with it (`sharpen_view`).
    pub fn sharpen(&self, player: f32) -> f32 {
        self.0.sharpen(player)
    }
}

/// The frame's screen effects, from the effects listed on the character
/// (added and not yet removed) with their current strengths: contusions,
/// low health, stuns, pain, painkillers, dehydration, exhaustion and a
/// berserk; and a hit's blur.
pub fn update(
    time: Res<Time>,
    controller: Res<crate::player::controller::Controller>,
    mut state: ResMut<ContusionState>,
    mut cameras: Query<(&mut ContusionScreen, Option<&mut crate::ui::screen_blur::ScreenBlur>)>,
    blindness: Option<ResMut<crate::effects::visor_view::Blindness>>,
) {
    let listed: Vec<Listed> = controller
        .health
        .as_ref()
        .map(|health| {
            health
                .effects
                .iter()
                .filter(|effect| matches!(effect.stage, Stage::Added | Stage::Started | Stage::Residual))
                .filter_map(|effect| {
                    let source = match effect.kind {
                        Kind::Contusion => Source::Contusion,
                        Kind::LowHealth => Source::LowHealth,
                        Kind::Stun => Source::Stun,
                        Kind::Pain => Source::Pain,
                        Kind::PainKiller => Source::PainKiller,
                        Kind::Dehydration => Source::Dehydration,
                        Kind::Exhaustion => Source::Exhaustion,
                        Kind::TunnelVision => Source::TunnelVision,
                        Kind::Berserk => Source::Berserk,
                        Kind::MedEffect if effect.med.as_ref().is_some_and(|m| m.medicine.class == trp_sim::condition::health::MedClass::MedKit) => Source::MedKit,
                        _ => return None,
                    };
                    let started = matches!(effect.stage, Stage::Started | Stage::Residual);
                    Some(Listed { id: effect.id, source, current: effect.current_strength, strength: effect.strength, residual: effect.stage == Stage::Residual, started })
                })
                .collect()
        })
        .unwrap_or_default();
    let shown = state.0.frame(time.delta_secs(), &listed);
    // The flash's strength while it runs, kept after (`_BlindnessCoef`:
    // the face shield's mask, `visor_view`).
    if let (Some(mut blindness), Some([_, strength])) = (blindness, shown.flash) {
        blindness.0 = strength;
    }
    let on = |on: bool| if on { 1.0 } else { 0.0 };
    let [timer, scale, strength] = shown.wiggle.unwrap_or([0.0, 0.0, 0.0]);
    let (blur, fresh) = shown.motion_blur.unwrap_or((0.0, false));
    let screen = ContusionScreen {
        displace: Vec2::from(DOUBLE_VISION_DISPLACE),
        amount: shown.double_vision.unwrap_or(0.0),
        timer,
        scale,
        strength,
        keep: 1.0 - blur,
        double_on: on(shown.double_vision.is_some_and(|amount| amount != 0.0)),
        wiggle_on: on(shown.wiggle.is_some()),
        blur_on: on(shown.motion_blur.is_some()),
        fresh: on(fresh),
        flash: shown.flash.map_or(0.0, |[white, _]| white),
        flash_on: on(shown.flash.is_some()),
        burn_on: 0.0,
        // `CC_RadialBlur`: an amount of nought copies the picture.
        radial: shown.radial_blur.unwrap_or(0.0),
        radial_on: on(shown.radial_blur.is_some_and(|amount| amount != 0.0)),
        radial_center: Vec2::from(state.0.radial_center()),
        vignette_on: on(shown.vignette.is_some()),
        vignette: {
            let ([x, y], sharpness) = state.0.vignette_shape();
            Vec4::new(x, y, sharpness * 0.01, shown.vignette.unwrap_or(0.0) * 0.02)
        },
        // `CC_Blend`: an amount of nought copies the picture too.
        near_death: shown.near_death.unwrap_or(0.0),
        near_death_on: on(shown.near_death.is_some_and(|amount| amount != 0.0)),
    };
    for (mut camera, blur) in &mut cameras {
        // The burn's own flag is the burn's to set (`eye_burn`).
        *camera = ContusionScreen { burn_on: camera.burn_on, ..screen };
        if let Some(mut blur) = blur {
            let hit = shown.hit_blur.unwrap_or(0.0);
            if blur.hit != hit {
                blur.hit = hit;
            }
        }
    }
}
