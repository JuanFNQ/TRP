//! The hall's light probe and lamps, and the range's own sun.

use super::*;

/// The share of the sky's ambient and its reflection the range hall's
/// stencil volume takes away inside it (`StencilShadow.Ambient`'s alpha,
/// which the screen's ambient passes keep one less of): Customs' concrete
/// warehouse's (`sklad_concrete`), whole (`trp-sim/tests/time_of_day.rs`
/// reads it from the install).
pub const HALL_STENCIL: f32 = 1.0;

/// The range hall's stencil volume, as a Bevy light probe over its inside
/// (`range::indoors`): within it the environment map's light at one
/// less the stencil's share, in place of the view's (Bevy's reflection
/// probes take a fragment inside them, its diffuse and its reflection,
/// over the view's). With its index put in the view's clusters by
/// `HallProbePlugin`.
pub fn hall_probe(view: &EnvironmentMapLight) -> Option<(bevy::light::LightProbe, EnvironmentMapLight, Transform, HallProbe)> {
    // The indoors (`range::indoors`: the hall's, a little into its walls,
    // or a level file's), turned as it is.
    let (centre, size, yaw) = crate::world::range::indoors()?;
    let map = EnvironmentMapLight { intensity: view.intensity * (1.0 - HALL_STENCIL), ..view.clone() };
    let place = Transform::from_translation(centre).with_rotation(Quat::from_rotation_y(yaw)).with_scale(size);
    Some((bevy::light::LightProbe::new(), map, place, HallProbe))
}

/// The hall's surfaces' colours for its bounced light (`hall_bounce`): its
/// floor the range's ground (0.35), its walls and roof its concrete
/// (`range::HALL_GREY`, 0.55).
const HALL_FLOOR_ALBEDO: f32 = 0.35;

const HALL_WALL_ALBEDO: f32 = 0.55;

/// The light the hall's lamps bounce off its floor, walls and roof, for its
/// probe with `HALL LAMPS` in place of the sky's ambient it takes away (the
/// character inside was very dark: the game's
/// indoors are lit by their baked light, which the hall has not).
/// `ESTIMATE`, the app's own: the hall a box (its indoor volume), each lamp
/// lighting its faces as Bevy draws it (`hall_lamp`: its power, its cone
/// and its range's fade; a face's light the mean over a grid on it), each
/// face's light bounced by its colour made linear, the faces lighting each
/// other by the shares of each one's view the others fill (each weighted by
/// its slant), ten times over; then, for each way of the probe's map, what
/// a surface at the hall's middle facing it takes from the faces. The sun
/// through the windows and the things inside are left out. In the probe's
/// map at the exposure's intensity, as the sky's. With the game's shading
/// (`shading`), the lamps light the faces as it draws them, their sum the
/// game's number made linear.
pub fn hall_bounce(images: &mut Assets<Image>) -> Option<EnvironmentMapLight> {
    let game = crate::rendering::shading::on();
    let (centre, size, yaw) = crate::world::range::indoors()?;
    let half = size / 2.0;
    let turn = Quat::from_rotation_y(-yaw);
    // Each lamp in the box's own axes, with its height over the floor.
    let lamps: Vec<(Vec3, f32)> = crate::world::range::hall_lamps().into_iter().map(|at| (turn * (at - centre), at.y)).collect();
    let faces = BoxFaces { half };
    let albedo: [f32; 6] = std::array::from_fn(|f| linear(if f == 0 { HALL_FLOOR_ALBEDO } else { HALL_WALL_ALBEDO }));
    // The lamps' light on each face, its mean over a grid on it.
    let direct: [f32; 6] = std::array::from_fn(|f| {
        let n = 24;
        let mut sum = 0.0;
        for i in 0..n {
            for j in 0..n {
                let p = faces.point(f, (i as f32 + 0.5) / n as f32, (j as f32 + 0.5) / n as f32);
                let lit = lamps.iter().map(|&(at, height)| lamp_light(at, height, p, faces.normal(f), game)).sum::<f32>();
                sum += if game { linear(lit) } else { lit };
            }
        }
        sum / (n * n) as f32
    });
    // The faces' shares of each face's view, each way weighted by its
    // slant: from a grid on the face, ways spread evenly by their slant.
    let shares: [[f32; 6]; 6] = std::array::from_fn(|f| {
        let mut share = [0.0f32; 6];
        let (n, ways) = (8, 16);
        for i in 0..n {
            for j in 0..n {
                let p = faces.point(f, (i as f32 + 0.5) / n as f32, (j as f32 + 0.5) / n as f32);
                for way in slanted_ways(faces.normal(f), ways) {
                    if let Some(hit) = faces.hit(p + faces.normal(f) * 1e-3, way) {
                        share[hit] += 1.0;
                    }
                }
            }
        }
        let total: f32 = share.iter().sum::<f32>().max(1.0);
        share.map(|s| s / total)
    });
    let mut bright = [0.0f32; 6];
    for _ in 0..10 {
        bright = std::array::from_fn(|f| albedo[f] * (direct[f] + (0..6).map(|g| shares[f][g] * bright[g]).sum::<f32>()));
    }
    let at_middle = |way: [f32; 3]| -> [f32; 3] {
        // The game's way (z turned round) into the box's axes.
        let way = turn * Vec3::new(way[0], way[1], -way[2]);
        let rays = slanted_ways(way, 12);
        let taken = rays.iter().filter_map(|&r| faces.hit(Vec3::ZERO, r)).map(|f| bright[f]).sum::<f32>() / rays.len() as f32;
        [taken; 3]
    };
    let diffuse = images.add(cube(16, 1, at_middle));
    let specular = images.add(cube(16, 5, at_middle));
    let lit = (0..6).map(|f| format!("{:.3}", bright[f])).collect::<Vec<_>>().join(", ");
    info!("the hall's bounced light (floor, roof, walls): {lit}");
    Some(EnvironmentMapLight { diffuse_map: diffuse, specular_map: specular, intensity: EXPOSURE_SCALE, ..default() })
}

/// A lamp's light, as Bevy draws the hall's (`hall_lamp`), on a place of a
/// surface facing `normal`: in the frame's linear light, the lamp at `at`
/// hung `height` over the floor, the light at the floor under it the
/// game's there (made linear), elsewhere by Bevy's fall-off with the square
/// of the distance, its range's fade and its cone's edge. With the game's
/// shading (`game`), in the game's numbers: its intensity by the engine's
/// fall-off, its slant and Bevy's cone.
fn lamp_light(at: Vec3, height: f32, p: Vec3, normal: Vec3, game: bool) -> f32 {
    let to = p - at;
    let d2 = to.length_squared().max(1e-4);
    let way = to / d2.sqrt();
    let facing = (-way).dot(normal).max(0.0);
    let (inner, outer) = (0.5f32.to_radians().cos(), (LAMP_CONE / 2.0).to_radians().cos());
    let scale = 1.0 / (inner - outer).max(1e-4);
    let cone = ((way.dot(Vec3::NEG_Y) * scale - outer * scale).clamp(0.0, 1.0)).powi(2);
    if game {
        return LAMP_INTENSITY * trp_sim::lighting::shading::attenuation(d2 / (LAMP_RANGE * LAMP_RANGE)) * facing * cone;
    }
    let fade = (1.0 - (d2 / (LAMP_RANGE * LAMP_RANGE)).powi(2)).clamp(0.0, 1.0).powi(2);
    let at_floor = linear(LAMP_INTENSITY * unity_attenuation(FACTORY_LAMP_HEIGHT / LAMP_RANGE));
    at_floor * height * height / d2 * facing * cone * fade
}

/// So many ways about a normal (their count squared), spread evenly by
/// their slant to it (a disc's points raised onto the half sphere).
fn slanted_ways(normal: Vec3, count: usize) -> Vec<Vec3> {
    let n = normal.normalize_or(Vec3::Y);
    let t = if n.y.abs() < 0.9 { n.cross(Vec3::Y).normalize() } else { n.cross(Vec3::X).normalize() };
    let b = n.cross(t);
    let mut ways = Vec::with_capacity(count * count);
    for i in 0..count {
        for j in 0..count {
            let r = ((i as f32 + 0.5) / count as f32).sqrt();
            let a = (j as f32 + 0.5) / count as f32 * std::f32::consts::TAU;
            let (x, y) = (r * a.cos(), r * a.sin());
            let z = (1.0 - r * r).max(0.0).sqrt();
            ways.push((t * x + b * y + n * z).normalize());
        }
    }
    ways
}

/// A box about its middle: its faces by number (0 the floor, 1 the roof,
/// 2 and 3 the walls across, 4 and 5 those along), their places, inward
/// normals and which a ray from inside meets.
pub(super) struct BoxFaces {
    pub(super) half: Vec3,
}

impl BoxFaces {
    fn normal(&self, f: usize) -> Vec3 {
        [Vec3::Y, Vec3::NEG_Y, Vec3::X, Vec3::NEG_X, Vec3::Z, Vec3::NEG_Z][f]
    }

    fn point(&self, f: usize, u: f32, v: f32) -> Vec3 {
        let h = self.half;
        let (a, b) = (u * 2.0 - 1.0, v * 2.0 - 1.0);
        match f {
            0 => Vec3::new(a * h.x, -h.y, b * h.z),
            1 => Vec3::new(a * h.x, h.y, b * h.z),
            2 => Vec3::new(-h.x, a * h.y, b * h.z),
            3 => Vec3::new(h.x, a * h.y, b * h.z),
            4 => Vec3::new(a * h.x, b * h.y, -h.z),
            _ => Vec3::new(a * h.x, b * h.y, h.z),
        }
    }

    fn hit(&self, from: Vec3, way: Vec3) -> Option<usize> {
        let mut best: Option<(f32, usize)> = None;
        for axis in 0..3 {
            let w = way[axis];
            if w.abs() < 1e-9 {
                continue;
            }
            let (neg, pos) = [(2, 3), (0, 1), (4, 5)][axis];
            let (face, plane) = if w < 0.0 { (neg, -self.half[axis]) } else { (pos, self.half[axis]) };
            let t = (plane - from[axis]) / w;
            if t > 0.0 && best.is_none_or(|(b, _)| t < b) {
                best = Some((t, face));
            }
        }
        best.map(|(_, face)| face)
    }
}

/// The hall's probe, marked to be carried into the render world.
#[derive(Component, Clone, Copy)]
pub struct HallProbe;

impl bevy::render::extract_component::ExtractComponent for HallProbe {
    type QueryData = ();
    type QueryFilter = With<HallProbe>;
    type Out = HallProbe;

    fn extract_component(_: bevy::ecs::query::QueryItem<'_, '_, Self::QueryData>) -> Option<Self::Out> {
        Some(HallProbe)
    }
}

/// Puts the hall's probe in the view's clusters. Bevy 0.18 counts a
/// reflection probe in each cluster it reaches but never writes its index
/// there (the probe is not carried into the render world, nor given an
/// index among the clusters' objects, as its point and spot lights and
/// decals are), so a fragment read the index after the cluster's own: the
/// next cluster's first light's, or none, and took another probe or an
/// empty one, which holds every place and takes all the ambient away
/// Here the probe is carried over and given its place in
/// the view's list of probes, the first: the hall's is the range's only
/// one. And Bevy puts a probe in the clusters its bounding sphere reaches
/// (25 m about the hall's middle) but in a view's list only while its box
/// is in the view: looking away from the hall from within the sphere, the
/// clusters named a probe the list had not, and an empty one took the
/// sky's ambient away outdoors. So the probe is taken out of the clusters
/// of a view that leaves it out (`clear_hall_probe`).
pub struct HallProbePlugin;

impl Plugin for HallProbePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(bevy::render::extract_component::ExtractComponentPlugin::<HallProbe>::default());
        app.add_systems(PostUpdate, clear_hall_probe.after(bevy::light::SimulationLightSystems::AssignLightsToClusters));
        let Some(render_app) = app.get_sub_app_mut(bevy::render::RenderApp) else { return };
        render_app.add_systems(
            bevy::render::Render,
            index_hall_probe.in_set(bevy::render::RenderSystems::ManageViews).after(bevy::pbr::prepare_lights),
        );
    }
}

/// Takes the hall's probe out of every cluster of a view whose frustum
/// misses its box, by the test Bevy's own gathering of a view's probes
/// makes (`LightProbeInfo::frustum_cull`: the unit box by the probe's
/// transform, the near plane counted, the far not).
fn clear_hall_probe(probes: Query<(Entity, &GlobalTransform), With<HallProbe>>, mut views: Query<(&bevy::camera::primitives::Frustum, &mut bevy::light::cluster::Clusters)>) {
    let unit = bevy::camera::primitives::Aabb { center: bevy::math::Vec3A::ZERO, half_extents: bevy::math::Vec3A::splat(0.5) };
    for (probe, transform) in &probes {
        for (frustum, mut clusters) in &mut views {
            if frustum.intersects_obb(&unit, &transform.affine(), true, false) {
                continue;
            }
            for cluster in clusters.clusterable_objects.iter_mut() {
                if let Some(at) = cluster.entities.iter().position(|entity| *entity == probe) {
                    cluster.entities.remove(at);
                    cluster.counts.reflection_probes -= 1;
                }
            }
        }
    }
}

fn index_hall_probe(probes: Query<Entity, With<HallProbe>>, mut objects: ResMut<bevy::pbr::GlobalClusterableObjectMeta>) {
    for probe in &probes {
        objects.entity_to_index.insert(probe, 0);
    }
}

/// Factory's industrial ceiling lamp (`Lamp_Industrial_01_High_Big`'s main
/// spot, as recorded on Factory by day: its untuned instances' figures):
/// its intensity, range in metres and cone in degrees. Its colour (gamma)
/// is white in place of the lamp's warm (1, 0.863, 0.651), to make the
/// inside feel brighter, the hall being the
/// app's own.
pub(super) const LAMP_COLOR: [f32; 3] = [1.0, 1.0, 1.0];

pub(super) const LAMP_INTENSITY: f32 = 8.0;

pub(super) const LAMP_RANGE: f32 = 18.2;

pub(super) const LAMP_CONE: f32 = 90.0;

/// How high Factory's industrial lamps hang over its floor (recorded:
/// 10.577 m), which the floor's light under one is reckoned at.
const FACTORY_LAMP_HEIGHT: f32 = 10.577;

/// The engine's own fall-off of a point or spot light's light with the
/// share of its range (its attenuation texture: `1 / (1 + 25 x²)`, faded
/// to nought over the range's last fifth; `trp_sim::shading::attenuation`).
fn unity_attenuation(share: f32) -> f32 {
    trp_sim::lighting::shading::attenuation(share * share)
}

/// An engine light's colour and power as Bevy's, as the devices' lights are
/// given (`tactical_view::lamp_of`): with the game's shading (`shading`), as
/// the game gives them (its intensity times 4π, which Bevy divides by);
/// otherwise matched a fifth of its reach out, where the engine's fall-off
/// has halved it, for a camera at the game's light's exposure, times `gain`
/// for one at another (`tactical_view::exposure_gain`).
pub fn engine_light(color: [f32; 3], intensity: f32, range: f32, gain: f32) -> (Color, f32) {
    let pi = std::f32::consts::PI;
    if crate::rendering::shading::on() {
        (Color::linear_rgb(color[0], color[1], color[2]), 4.0 * pi * intensity)
    } else {
        let reach = 0.2 * range;
        let at = intensity / (1.0 + 25.0 * 0.2 * 0.2);
        (Color::linear_rgb(linear(color[0]), linear(color[1]), linear(color[2])), 4.0 * pi * pi * EXPOSURE_SCALE * reach * reach * linear(at) * gain)
    }
}

/// The hall's lamp as Bevy's spot light, hung this high over the floor:
/// its colour the linear of the lamp's, and its power such that the floor
/// straight under it takes what Factory's floor takes under the same lamp
/// hung where Factory hangs it (the lamp's intensity times the engine's
/// fall-off there, made linear, at the exposure above: Bevy's spot light
/// gives `I / 4π` over the square of the distance, and its diffuse light a
/// `1 / π`). Reckoned at the hall's own height, the floor took 1.7 times
/// Factory's and showed nearly white. `PROVISIONAL`: elsewhere Bevy's
/// fall-off (with the square of the distance) and its cone's edge are its
/// own, and no shadows are cast, as the game's lamp casts none.
///
/// With the game's shading (`shading`), the lamp as the game gives it: its
/// colour, its intensity (times 4π, which Bevy divides by) and its range,
/// the engine's fall-off at every distance; hung at the hall's height, the
/// floor under it takes 1.7 times what Factory's takes.
pub fn hall_lamp(height: f32) -> SpotLight {
    let at_floor = LAMP_INTENSITY * unity_attenuation(FACTORY_LAMP_HEIGHT / LAMP_RANGE);
    let pi = std::f32::consts::PI;
    let (color, intensity) = if crate::rendering::shading::on() {
        (Color::linear_rgb(LAMP_COLOR[0], LAMP_COLOR[1], LAMP_COLOR[2]), 4.0 * pi * LAMP_INTENSITY)
    } else {
        (Color::linear_rgb(linear(LAMP_COLOR[0]), linear(LAMP_COLOR[1]), linear(LAMP_COLOR[2])), 4.0 * pi * pi * EXPOSURE_SCALE * height * height * linear(at_floor))
    };
    SpotLight {
        color,
        intensity,
        range: LAMP_RANGE,
        radius: 0.0,
        shadows_enabled: false,
        inner_angle: 0.5f32.to_radians(),
        outer_angle: (LAMP_CONE / 2.0).to_radians(),
        ..default()
    }
}

/// The range's sun, Customs' or the range's own: the light the effects'
/// shaders take their way to the sun from (other directional lights light
/// the icon studio and the figures' views).
#[derive(Component)]
pub struct RangeSun;

/// The range's own light (`CUSTOMS LIGHT` off) as it stands at the range's
/// own hour: a sun of 12,000 lux from up and to the side, an even ambient
/// and a plain sky's colour (the app's, not the game's).
pub const RANGE_OWN_SUN: f32 = 12_000.0;

pub const RANGE_OWN_AMBIENT: f32 = 400.0;

pub const RANGE_OWN_SKY: [f32; 3] = [0.55, 0.63, 0.70];

/// The range's own sun's turn at its own hour.
pub fn range_own_sun_turn() -> Transform {
    Transform::from_xyz(3.0, 8.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y)
}
