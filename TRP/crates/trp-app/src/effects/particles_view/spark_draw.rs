//! The sparks' uniform, prepared and drawn each frame.

use super::*;

/// What the sparks' shader is given (`Custom/SimpleParticleMoveAdd`'s
/// figures and the camera's).
#[derive(Clone, Copy, Default, ShaderType)]
pub struct SparkUniform {
    pub clip_from_world: Mat4,
    /// `_MainTime`, `_LastMainTime`, `_Size`, `_FadeOut`.
    pub times: Vec4,
    /// The turbulence's two frequencies and amplitudes.
    pub turbulence: Vec4,
    /// `_TintColor`.
    pub tint: Vec4,
}

/// The sparks to draw this frame: the figures, the picture, each live
/// spark's streak's corners (place, picture, offset, share and draw, born,
/// velocity, bytes: twenty numbers each) and their triangles.
#[derive(Clone)]
pub struct SparkDraw {
    pub uniform: SparkUniform,
    pub picture: Handle<Image>,
    pub corners: Arc<Vec<f32>>,
    pub indices: Arc<Vec<u32>>,
}

/// On the camera: the sparks while any may show.
#[derive(Component, Clone, Default)]
pub struct SparkScreen(pub Option<SparkDraw>);

impl ExtractComponent for SparkScreen {
    type QueryData = &'static SparkScreen;
    type QueryFilter = ();
    type Out = SparkScreen;

    fn extract_component(sparks: QueryItem<'_, '_, Self::QueryData>) -> Option<Self::Out> {
        sparks.0.as_ref().is_some_and(|draw| !draw.indices.is_empty()).then(|| sparks.clone())
    }
}

/// The shared sparks' ring and what the weapons' sparks draw from:
/// `FastRndom`, the engine's own draws, the shots seen, the last frame's
/// time, the renderer's picture and figures, and one streak's corners and
/// triangles.
#[derive(Resource)]
pub struct Sparks {
    ring: SparksRing,
    pub(super) fast: FastRandom,
    pub(super) rng: Rng,
    pub(super) shots_seen: [u32; 2],
    last_time: f32,
    pub(super) picture: Handle<Image>,
    pub(super) uniform: SparkUniform,
    pub(super) corners: Vec<trp_sim::effects::sparks::Corner>,
    indices: Vec<u32>,
}

impl Sparks {
    /// A spark into the shared ring (an older emitter's: `explosion_view`).
    pub fn emit(&mut self, spark: trp_sim::effects::sparks::SparkEmit) {
        self.ring.emit(spark);
    }
}

/// The shared sparks' renderer read, at the start.
pub(crate) fn prepare_sparks(unity: &mut trp_data::unity::Assets, images: &mut impl crate::assets::background::AddImage) -> Option<Sparks> {
    let data = match trp_data::drawing::particles::muzzle_sparks_renderer(unity) {
        Ok(data) => data,
        Err(e) => {
            warn!("the muzzles' sparks are not drawn: their renderer could not be read: {e:#}");
            return None;
        }
    };
    let Some(picture) = picture(&data.texture, images) else { return None };
    info!("the muzzles' sparks: {} of {} segments ({})", data.count, data.segments, data.shader);
    let mut rng = Rng::new(0x5_9a2c);
    Some(Sparks {
        ring: SparksRing::new(data.count, data.segments, &mut rng),
        fast: FastRandom::default(),
        rng,
        shots_seen: [0; 2],
        last_time: 0.0,
        picture,
        uniform: SparkUniform { times: Vec4::new(0.0, 0.0, data.size, data.fade_out), turbulence: Vec4::from(data.turbulence.unwrap_or([0.0; 4])), tint: Vec4::from(data.tint), ..default() },
        corners: trp_sim::effects::sparks::corners(data.segments),
        indices: trp_sim::effects::sparks::indices(data.segments),
    })
}

/// A frame, once everything is placed: each shot's sparks from the weapon
/// in hand's, and every spark that may show handed to each camera that
/// draws them (the main camera, and the optic camera while on: the sparks'
/// renderer is an object of the world's layer).
pub fn spark_frame(
    rig: Option<Res<crate::weapons::weapon::WeaponRig>>,
    controller: Res<crate::player::controller::Controller>,
    time: Res<Time>,
    mut sparks: ResMut<Sparks>,
    places: Query<&GlobalTransform>,
    mut cameras: Query<(&Camera, &GlobalTransform, &mut SparkScreen)>,
) {
    let sparks = &mut *sparks;
    let now = time.elapsed_secs();
    let game = |v: Vec3| [v.x, v.y, -v.z];
    for (parts, shots) in crate::weapons::muzzle_view::fired_muzzles(rig.as_deref(), &controller, &mut sparks.shots_seen) {
        for (index, entity) in parts.sparks.iter().enumerate() {
            let Some(place) = entity.and_then(|entity| places.get(entity).ok()) else { continue };
            for _ in 0..shots.min(8) {
                for spark in trp_sim::weapons::muzzle::spark_emits(&parts.data.sparks[index], game(place.translation()), game(-place.up().as_vec3()), now, &mut sparks.fast, &mut sparks.rng) {
                    sparks.ring.emit(spark);
                }
            }
        }
    }
    let last = std::mem::replace(&mut sparks.last_time, now);
    // The streaks' corners once, in the world (each camera's shader makes
    // the streak on its own screen).
    let per = sparks.corners.len() as u32;
    let mut corners = Vec::new();
    let mut indices = Vec::new();
    for (k, (spark, draw)) in sparks.ring.live(now, last).enumerate() {
        let born = [spark.time, spark.lifetime, spark.gravity, spark.drag];
        let bytes = [spark.emission, spark.size, spark.turbulence, spark.frequency].map(|b| b as f32 / 255.0);
        for c in &sparks.corners {
            corners.extend_from_slice(&spark.position);
            corners.extend_from_slice(&c.uv);
            corners.extend_from_slice(&c.offset);
            corners.extend_from_slice(&[c.share, draw]);
            corners.extend_from_slice(&born);
            corners.extend_from_slice(&spark.velocity);
            corners.extend_from_slice(&bytes);
        }
        indices.extend(sparks.indices.iter().map(|i| i + k as u32 * per));
    }
    let shown = (!indices.is_empty()).then(|| (Arc::new(corners), Arc::new(indices)));
    for (camera, eye, mut screen) in &mut cameras {
        let Some((corners, indices)) = shown.as_ref().filter(|_| camera.is_active) else {
            if screen.0.is_some() {
                screen.0 = None;
            }
            continue;
        };
        screen.0 = Some(SparkDraw {
            uniform: SparkUniform { clip_from_world: camera.clip_from_view() * eye.to_matrix().inverse(), times: Vec4::new(now, last, sparks.uniform.times.z, sparks.uniform.times.w), ..sparks.uniform },
            picture: sparks.picture.clone(),
            corners: corners.clone(),
            indices: indices.clone(),
        });
    }
}
