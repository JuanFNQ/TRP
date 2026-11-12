//! The visor and crack screens, their images and state, and the visor's frame.

use super::*;

/// What the mask draws this frame (`on` 1 while it shows).
#[derive(Component, Clone, Copy, Default, ShaderType)]
pub struct VisorScreen {
    pub swing: Vec2,
    pub intensity: f32,
    pub edge_fade: f32,
    pub rotation: f32,
    pub hit_zoom: f32,
    pub scratches_intensity: f32,
    pub light: f32,
    pub distort_intensity: f32,
    pub blindness: f32,
    pub textured: f32,
    pub framed: f32,
    pub blur_steps: Vec4,
    pub rounds: f32,
    pub on: f32,
    /// 1 where the cracks' picture is read (`FACE SHIELD CRACKS`).
    pub cracks: f32,
}

impl ExtractComponent for VisorScreen {
    type QueryData = &'static VisorScreen;
    type QueryFilter = ();
    type Out = VisorScreen;

    /// Only while it shows: below an intensity of 0.01 the game's effect
    /// copies the frame as it is.
    fn extract_component(screen: QueryItem<'_, '_, Self::QueryData>) -> Option<Self::Out> {
        (screen.on > 0.0).then_some(*screen)
    }
}

/// The flash's strength as it last ran (`_BlindnessCoef`, set by
/// `GrenadeFlashScreenEffect.Update` while it runs, and kept after: its
/// `OnDisable` does not put it back), which takes the scratches away.
#[derive(Resource, Default)]
pub struct Blindness(pub f32);

/// The mask's pictures on the card: the mask the shield's kind is drawn
/// with (none: no frame), the scratches, the blur's mask and the
/// distortion's.
#[derive(Resource, Clone, Default, ExtractResource)]
pub struct VisorImages {
    pub mask: Option<Handle<Image>>,
    pub scratches: Option<Handle<Image>>,
    pub blur_mask: Option<Handle<Image>>,
    pub distort_mask: Option<Handle<Image>>,
    /// The crack's picture (`GlassDamageMaterial`'s `_MainTex`).
    pub crack: Option<Handle<Image>>,
}

/// The cracks as the camera last drew them (`DrawScratches`): its drawing's
/// number (`trp_sim::visor::Cracks::drawings`; nought, none yet) and each
/// crack's quad at the camera's aspect as it was drawn. The cracks' picture
/// is drawn anew whenever the number moves, and whenever the view's size
/// does, from the same quads (the game's picture is the display's size,
/// made once).
#[derive(Resource, Clone, Default, ExtractResource)]
pub struct CrackScreen {
    pub drawings: u64,
    pub quads: Vec<CrackQuad>,
}

/// The mask once its figures have come: the camera's state, the figures,
/// each kind's mask picture, and what brings them.
#[derive(Resource, Default)]
pub struct VisorState {
    pub(super) visor: Option<Visor>,
    data: Option<VisorData>,
    masks: Vec<(MaskKind, Option<Handle<Image>>)>,
    pub(super) mask: Option<MaskKind>,
    coming: Option<Mutex<Receiver<VisorData>>>,
    /// `FACE SHIELD CRACKS`.
    pub(super) cracks: bool,
}

/// Starts reading the mask's figures and pictures away from the frame, the
/// cracks' with `cracks`.
pub fn start_reading(app: &mut App, install: trp_data::Install, cracks: bool) {
    let (send, receive) = channel();
    std::thread::spawn(move || match trp_data::character::visor::read(&install, cracks) {
        Ok(read) => {
            let _ = send.send(read);
        }
        Err(e) => eprintln!("the face shield's mask could not be read; a face shield down shows nothing over the view: {e:#}"),
    });
    app.insert_resource(VisorState { coming: Some(Mutex::new(receive)), cracks, ..default() });
}

/// The mask's frame: its figures and pictures taken in as they come; the
/// shields of the head's and the face's slots heard (`trp_sim::visor`);
/// the filter and the swing moved on (the walk's step frequency, the
/// body's speed across the ground); and what the main camera draws, in
/// first person only.
#[allow(clippy::too_many_arguments)]
pub fn visor_frame(
    mut commands: Commands,
    mut state: ResMut<VisorState>,
    mut images: ResMut<VisorImages>,
    mut assets: ResMut<Assets<Image>>,
    carried: Res<crate::screens::inventory_view::Carried>,
    controller: Res<crate::player::controller::Controller>,
    hands: Option<Res<crate::weapons::weapon::HandsRig>>,
    blindness: Res<Blindness>,
    view: Res<crate::View>,
    time: Res<Time>,
    cameras: Query<(Entity, Option<&VisorScreen>), With<crate::MainCamera>>,
    (mut crack_screen, projections): (ResMut<CrackScreen>, Query<&Projection, With<crate::MainCamera>>),
) {
    let state = &mut *state;
    if let Some(coming) = state.coming.take() {
        match coming.lock().ok().map(|receive| receive.try_recv()) {
            Some(Ok(data)) => {
                // Read as stored, as the game's gamma-space project reads
                // them (`gamma_ui`): no sRGB curve undone.
                let mut raw = |picture: &trp_data::unity::extract::TextureData, name: &str| {
                    crate::assets::render_data::build_image(&trp_data::unity::extract::TextureData { srgb: false, ..picture.clone() }, crate::assets::render_data::Role::Color)
                        .map_err(|e| warn!("the face shield's {name}: {e:#}"))
                        .ok()
                        .map(|image| assets.add(image))
                };
                images.scratches = raw(&data.scratches, "scratches");
                images.blur_mask = raw(&data.blur_mask, "blur mask");
                images.distort_mask = raw(&data.distort_mask, "distortion mask");
                images.crack = data.crack.as_ref().and_then(|(_, picture)| raw(picture, "crack picture"));
                state.masks = [MaskKind::None, MaskKind::Narrow, MaskKind::Wide].into_iter().map(|kind| (kind, data.mask_of(kind).and_then(|picture| raw(picture, "mask")))).collect();
                info!(
                    "the face shield's mask: scratches at {}, distortion {}, a blur of {} rounds; masks {:?}",
                    data.scratches_intensity,
                    data.distort_intensity,
                    data.blur_iterations,
                    state.masks.iter().map(|(kind, picture)| (*kind, picture.is_some())).collect::<Vec<_>>()
                );
                state.visor = Some(Visor::new(VisorFigures {
                    forward: data.forward.clone(),
                    backward: data.backward.clone(),
                    edge_fade: data.edge_fade.clone(),
                    dx: data.dx.clone(),
                    dy: data.dy.clone(),
                    walk_rotation: data.walk_rotation.clone(),
                }));
                state.data = Some(data);
            }
            Some(Err(std::sync::mpsc::TryRecvError::Empty)) => state.coming = Some(coming),
            _ => {}
        }
    }
    let (Some(visor), Some(data)) = (state.visor.as_mut(), state.data.as_ref()) else { return };
    // The filter moved first, then the frame's shields heard: the game's
    // filter moves before a switch made in the frame (`courses8`'s visors).
    let equipment = &carried.equipment;
    visor.update(time.delta_secs());
    visor.observe(shield_in(equipment, "Headwear"), shield_in(equipment, "FaceCover"));
    // The cracks drawn anew at the camera's aspect (`DrawScratches`), as the
    // camera drew them.
    let cracks = visor.cracks();
    let cracked = state.cracks && images.crack.is_some();
    // (Not before the camera has its aspect: then on the next frame.)
    let aspect = projections.iter().find_map(|projection| match projection {
        Projection::Perspective(perspective) => Some(perspective.aspect_ratio),
        _ => None,
    });
    if let (true, Some(aspect)) = (cracked && cracks.drawings != crack_screen.drawings, aspect) {
        let quads = crack_quads(cracks.seed, cracks.count, aspect);
        info!("the face shield's cracks drawn anew: {} by its seed {}", quads.len(), cracks.seed);
        *crack_screen = CrackScreen { drawings: cracks.drawings, quads };
    }
    let step_frequency = hands.as_ref().map_or(0.0, |rig| rig.hands.step_frequency);
    let [vx, _, vz] = controller.movement.velocity;
    let frame = visor.frame(time.delta_secs(), step_frequency, (vx * vx + vz * vz).sqrt());
    if state.mask != Some(frame.mask) {
        state.mask = Some(frame.mask);
        images.mask = state.masks.iter().find(|(kind, _)| *kind == frame.mask).and_then(|(_, picture)| picture.clone());
    }
    let mut shown = VisorScreen::default();
    if Visor::drawn(&frame) && *view == crate::View::FirstPerson {
        let n = 1.0 / (1u32 << data.downsample) as f32;
        let mut steps = [0.0; 4];
        for (round, step) in steps.iter_mut().enumerate().take(data.blur_iterations as usize) {
            *step = data.blur_size * n + round as f32;
        }
        shown = VisorScreen {
            swing: Vec2::from(frame.swing),
            intensity: frame.intensity,
            edge_fade: frame.edge_fade,
            rotation: frame.rotation,
            hit_zoom: 0.0,
            scratches_intensity: data.scratches_intensity,
            light: if data.affected_by_light { 1.0 } else { data.min_light },
            distort_intensity: data.distort_intensity,
            blindness: blindness.0,
            textured: if frame.mask == MaskKind::Narrow { 0.0 } else { 1.0 },
            framed: if images.mask.is_some() { 1.0 } else { 0.0 },
            blur_steps: Vec4::from(steps),
            rounds: data.blur_iterations.min(4) as f32,
            on: 1.0,
            cracks: if cracked { 1.0 } else { 0.0 },
        };
    }
    for (camera, had) in &cameras {
        if had.is_none_or(|had| had.on != shown.on || shown.on > 0.0) {
            commands.entity(camera).insert(shown);
        }
    }
}
