//! Tests of the muzzle: its jets, light, smoke, fumes, sparks, haze and trail.
use super::*;
use trp_data::formats::curve::{Curve, Key};
use trp_data::weapons::muzzle::{Jet, JetMaterial, JetParticle};
use trp_data::unity::extract::{Node, TextureData};
use trp_data::unity::ObjRef;

fn node(name: &str, parent: Option<usize>, translation: [f32; 3], rotation: [f32; 4]) -> Node {
    Node {
        name: name.to_owned(),
        parent,
        translation,
        rotation,
        scale: [1.0; 3],
        active: true,
        renderer: None,
        scripts: Vec::new(),
        script_objects: Vec::new(),
        grip: None,
        placer: None,
        transform: ObjRef { file: 0, path_id: 0 },
        colliders: Vec::new(),
        bounded: None,
        lower: None,
        lod: None,
    }
}

fn data() -> MuzzleData {
    let key = |time, value, slope| Key { time, value, in_slope: slope, out_slope: slope };
    let texture = TextureData { name: String::new(), width: 1, height: 1, format: 4, mip_count: 1, srgb: false, data: vec![0; 4], streaming_mipmaps: false, point: false, clamp: [false; 2], faces: 1 };
    MuzzleData {
        shot_length: 0.12,
        move_curve: Curve { keys: vec![key(0.0, -0.53, 1.75), key(1.0, 1.2166, 0.45)] },
        light_curve: Curve { keys: vec![key(0.0, 0.0, 2.0), key(0.128, 1.0, 0.017), key(0.609, 1.0, 0.057), key(1.0, 0.0, 0.0)] },
        atlas: [2, 2],
        mesh_parent: 1,
        material: JetMaterial { texture, tint: [1.0, 0.46, 0.26, 1.0], inv_window: 1.37, randomize_phase: 2.0, hdr: 1.2, z_shift: 0.94, render_queue: -1 },
        jets: vec![Jet {
            node: 3,
            particles: (0..8).map(|i| JetParticle { position: i as f32 * 0.02, size: 0.05, shift: [0.0, 0.0], axis_shift: 0.0 }).collect(),
            bounds: [0.0, 0.1, 0.3],
            chance: 0.15,
        }],
        smokes: Vec::new(),
        fumes: Vec::new(),
        sparks: Vec::new(),
        hazes: Vec::new(),
        pivots: Vec::new(),
    }
}

/// The AKM's smoke (`the_akm_s_muzzle_flash_is_read`).
fn smoke_data() -> SmokeData {
    let key = |time, value, slope| Key { time, value, in_slope: slope, out_slope: slope };
    let texture = TextureData { name: String::new(), width: 1, height: 1, format: 4, mip_count: 1, srgb: false, data: vec![0; 4], streaming_mipmaps: false, point: false, clamp: [false; 2], faces: 1 };
    SmokeData {
        node: 0,
        smoke_end: 0.03,
        brake_distance: 0.01,
        drag: 0.95,
        gravity: -1.1,
        smoke_velocity: 0.4,
        turbulence_density: 0.4,
        turbulence_intensity: 0.1,
        diffusion_by_velocity: 32.0,
        speed_multiplier: 1.2,
        speed_turbulence_density: Curve { keys: vec![key(0.0, 1.0, 0.1), key(9.0, 5.0, 0.0)] },
        speed_turbulence_strength: Curve { keys: vec![key(0.0, 1.0, 30.0), key(8.0, 80.0, 0.0)] },
        speed_smoke_strength: Curve { keys: vec![key(0.0, 1.0, -0.26), key(2.27, 0.65, -0.05), key(10.0, 0.1, -0.23)] },
        speed_start_diffusion: Curve { keys: vec![key(33.0, 0.05, 0.01), key(35.0, 0.5, 0.005)] },
        smoke: Curve { keys: vec![key(0.0, 0.0, 0.1), key(0.2, 1.0, 0.0), key(0.45, 1.0, 0.0), key(1.0, 0.0, 0.0)] },
        smoke_strength: 0.3,
        smoke_length: 20.0,
        smoke_length_randomness: 10.0,
        increasing_by_shot: 0.4,
        drop_time: 0.5,
        material: trp_data::weapons::muzzle::SmokeMaterial { texture, width: 0.005, uv_y_scale: 0.03, diffusion_strength: 60.0, start_fade: 0.0003, end_fade: 0.05, end: 0.03, inv_fade: 1.0, alpha: 1.0 },
    }
}

#[test]
fn the_smoke_s_draws_are_the_game_s_hash() {
    // `FastRndom.Int(8324234, −1000, 2000)` worked by hand.
    let mut n: i32 = 8_324_234;
    n ^= n.wrapping_shl(7);
    n ^= n >> 3;
    assert_eq!(hashed(8_324_234, -1000, 2000), -1000 + n.wrapping_abs() % 2000);
    // Whole places give their own draw; between, a smooth step.
    let (a, b) = (smoke_draw(3.0), smoke_draw(4.0));
    assert!((-1.0..1.0).contains(&a) && (-1.0..1.0).contains(&b));
    assert!((smoke_draw(3.5) - (a + b) / 2.0).abs() < 1e-5);
}

/// The AKM's first fume (`muzzleflash_000`; firing spec, "The muzzle's
/// fumes").
fn akm_fume() -> FumeData {
    let key = |time, value, slope| Key { time, value, in_slope: slope, out_slope: slope };
    FumeData {
        node: 0,
        start_pos: 0.0,
        emitter_radius: 0.0,
        conus_size: 0.0,
        sizes: Curve { keys: vec![key(0.0, 0.4025, 0.0), key(1.0, 0.7521, 0.0)] },
        speeds: Curve { keys: vec![key(0.3999, 0.8285, 1.0922), key(0.5778, 1.0, 1.3817)] },
        lifetimes: Curve { keys: vec![key(0.4, 0.7, 0.0), key(0.6, 1.0, -0.0401)] },
        sizes_rnd: 0.068,
        size: 0.4,
        speed: 5.0,
        lifetime: 0.2,
        color: trp_data::drawing::particles::Gradient { colors: vec![(0.0, [1.0; 3]), (1.0, [1.0; 3])], alphas: vec![(0.0, 0.8627), (0.494, 0.8392), (1.0, 0.0824)], stepped: false },
        count_min: 1,
        count_range: 1,
    }
}

#[test]
fn a_fume_s_puffs_are_the_game_s() {
    // `FastRndom.Int(start, range)` and `Uint()` from their first
    // state, worked by hand.
    let mut n: i32 = 242_313_459;
    n ^= n.wrapping_shl(7);
    n ^= n >> 3;
    let mut u: u32 = 242_313_459;
    u ^= u << 7;
    u ^= u >> 3;
    let mut fast = FastRandom::default();
    assert_eq!(fast.int(3, 5), 3 + n.wrapping_abs() % 5);
    assert_eq!(fast.uint(), u);
    // The three tables' draws come off the one cursor, stepping back.
    let mut a = FastRandom::default();
    let mut b = FastRandom::default();
    assert_eq!(a.float(), b.float());
    assert_eq!(a.rotation(), b.rotation());
    assert!((square(a.vector()) - 1.0).abs() < 1e-5);
    // One puff a shot, straight ahead, with half the muzzle's movement.
    let f = akm_fume();
    let mut rng = Rng::new(3);
    for _ in 0..50 {
        let puffs = fume_puffs(&f, [1.0, 1.5, 0.0], [0.0, 0.0, 1.0], [0.02, 0.0, 0.0], &mut fast, &mut rng);
        assert_eq!(puffs.len(), 1);
        let p = puffs[0];
        assert_eq!(p.position, [1.0, 1.5, 0.0]);
        assert!((p.velocity[0] - 0.01).abs() < 1e-6 && p.velocity[1] == 0.0);
        assert!(p.velocity[2] >= 0.8285 * 5.0 - 1e-3 && p.velocity[2] <= 5.0 + 1e-3, "{:?}", p.velocity);
        assert!(p.lifetime >= 0.14 - 1e-4 && p.lifetime <= 0.2 + 1e-4, "{}", p.lifetime);
        assert!(p.size >= (0.4025 - 0.068) * 0.4 - 1e-4 && p.size <= (0.7521 + 0.068) * 0.4 + 1e-4, "{}", p.size);
        assert!((21..=220).contains(&p.color[3]), "{:?}", p.color);
        assert!((0.0..360.0).contains(&p.rotation));
    }
}

#[test]
fn a_muzzle_s_sparks_are_the_game_s() {
    // The AKM's port's: a count of −5 and a range of 7, so most shots
    // none and some one; each from its tables' one entry.
    let table = |v: f32| vec![v; 128];
    let s = SparksData {
        node: 0,
        start_pos: 0.0,
        emitter_radius: 0.0,
        conus_size: 0.1,
        count_min: -5,
        count_range: 7,
        lifetime: table(0.6),
        speed: (0..128).map(|i| i as f32).collect(),
        gravity: table(0.4),
        drag: table(0.0016),
        emission: vec![255; 128],
        size: vec![13; 128],
        turbulence: vec![27; 128],
        frequency: vec![134; 128],
        emission_time_shift: -0.01,
    };
    let (mut fast, mut rng) = (FastRandom::default(), Rng::new(9));
    let counts: Vec<usize> = (0..200).map(|_| spark_emits(&s, [0.0; 3], [0.0, 0.0, 1.0], 5.0, &mut fast, &mut rng).len()).collect();
    assert!(counts.iter().all(|&n| n <= 1) && counts.contains(&0) && counts.contains(&1), "{counts:?}");
    let one = (0..200).find_map(|_| spark_emits(&s, [0.0; 3], [0.0, 0.0, 1.0], 5.0, &mut fast, &mut rng).first().copied()).expect("a spark");
    assert_eq!((one.time, one.lifetime, one.size), (4.99, 0.6, 13));
    // Its speed the table's entry, along a way strayed by the cone.
    let speed = square(one.velocity).sqrt();
    assert!(speed.fract().abs() < 1e-4 || (1.0 - speed.fract()).abs() < 1e-4, "{speed}");
    assert!(one.velocity[2] > 0.0);
}

#[test]
fn a_shot_s_haze_is_the_game_s() {
    let haze = HazeData { node: 0, lifetime: [0.46, 1.45], size: [0.29, 0.86], count: 3, velocity_factor: 10.0, angle: 0.04, delay: 0.15, chance: 0.25 };
    let mut rng = Rng::new(5);
    // A quarter of the shots, about; none with no puffs.
    let had = (0..4000).filter(|_| haze_shot(&haze, &mut rng)).count();
    assert!((800..1200).contains(&had), "{had}");
    assert!(!haze_shot(&HazeData { count: 0, chance: 1.0, ..haze.clone() }, &mut rng));
    // Three puffs of one size, each along the place from its parent
    // (2 cm ahead) times ten, give or take the ball's 0.04 times ten.
    let puffs = haze_puffs(&haze, [0.0, 1.5, 0.0], [0.0, 0.0, 0.02], &mut rng);
    assert_eq!(puffs.len(), 3);
    assert!(puffs.iter().all(|p| p.size == puffs[0].size && (0.29..=0.86).contains(&p.size)));
    for p in &puffs {
        assert!((p.velocity[2] - 0.2).abs() <= 0.4 + 1e-5 && p.velocity[0].abs() <= 0.4 + 1e-5, "{:?}", p.velocity);
        let ahead = std::array::from_fn::<f32, 3, _>(|k| p.position[k] - [0.0, 1.5, 0.0][k]);
        assert!((square(ahead).sqrt() - p.size * 0.5).abs() < 1e-4);
        assert!((0.0..90.0).contains(&p.rotation) && p.rotation.fract() == 0.0);
        assert!((0.46..=1.45).contains(&p.lifetime) && p.color == [255; 4]);
    }
}

/// The game works its trail once a frame, its drag and spread by the
/// frame: the trail's life grows with the frame's time cubed. So the app
/// ages it at the game's own frame rate setting (`muzzle_view`), where it
/// lives the few seconds it does in the game at 120, where at a higher
/// rate it would live far longer.
#[test]
fn the_trail_s_life_goes_by_the_frame_rate() {
    let f = smoke_data();
    let mut lives = Vec::new();
    for fps in [60.0f32, 120.0, 300.0] {
        let dt = 1.0 / fps;
        let mut smoke = Smoke::default();
        let mut rng = Rng::new(4);
        for _ in 0..3 {
            smoke.shot(&f, &mut rng);
        }
        // The muzzle swung 0.3 m across in a fifth of a second, then
        // held still, after a second at rest where the trail's head
        // eased in.
        let (mut t, mut frames) = (0.0f32, 0u32);
        let forward = [0.0, 0.0, 1.0];
        while smoke.on() {
            let x = 0.3 + 0.3 * ((t - 1.0) / 0.2).clamp(0.0, 1.0);
            smoke.frame(&f, dt, [x, 1.5, 0.6], forward);
            t += dt;
            frames += 1;
            assert!(frames < 200_000, "it never ended at {fps}");
        }
        lives.push(t);
    }
    // 1.33, 4.18 and 58.2 seconds (measured).
    assert!((1.0..1.7).contains(&lives[0]), "{lives:?}");
    assert!((3.5..5.0).contains(&lives[1]), "{lives:?}");
    assert!(lives[2] > 40.0, "{lives:?}");
}

#[test]
fn the_third_shot_begins_a_trail_that_rises_spreads_and_ends() {
    let f = smoke_data();
    let mut smoke = Smoke::default();
    let mut rng = Rng::new(4);
    smoke.shot(&f, &mut rng);
    smoke.shot(&f, &mut rng);
    assert!(!smoke.on(), "two shots: not yet");
    smoke.shot(&f, &mut rng);
    assert!(smoke.on(), "the third");
    // The muzzle at rest away from the origin: the head eases in from
    // the origin (as the game's does the first time), leaving points.
    let place = [0.3, 1.5, 0.6];
    let forward = [0.0, 0.0, 1.0];
    for _ in 0..12 {
        smoke.frame(&f, 1.0 / 60.0, place, forward);
    }
    assert!(smoke.points.len() >= 3, "{}", smoke.points.len());
    // Each point worked rises (gravity −1.1) and spreads as it goes.
    let before: Vec<(i32, f32, f32)> = smoke.points.iter().filter(|p| !p.skip).map(|p| (p.number, p.position[1], p.diffusion)).collect();
    assert!(!before.is_empty());
    let x = 0.3;
    for _ in 0..10 {
        smoke.frame(&f, 1.0 / 60.0, [x, 1.5, 0.6], forward);
    }
    let rose = before.iter().any(|&(number, y, d)| smoke.points.iter().any(|p| p.number == number && p.position[1] > y && p.diffusion >= d));
    assert!(rose, "{before:?}");
    let corners = smoke.corners();
    assert_eq!(corners.len(), 4 * (smoke.points.len().max(1) - 1));
    // Left alone, it spreads past its end and goes off.
    let mut frames = 0;
    while smoke.on() {
        smoke.frame(&f, 1.0 / 60.0, [x, 1.5, 0.6], forward);
        frames += 1;
        assert!(frames < 100_000, "it never ended");
    }
}

#[test]
fn a_jet_s_quads_lie_along_its_axis_from_its_place() {
    // A weapon object, a muzzle 0.4 m ahead of it, and the jet turned to
    // point down its barrel: its own down along the weapon's ahead.
    let quarter = [-std::f32::consts::FRAC_1_SQRT_2, 0.0, 0.0, std::f32::consts::FRAC_1_SQRT_2];
    let hierarchy = Hierarchy { nodes: vec![node("root", None, [0.0; 3], [0.0, 0.0, 0.0, 1.0]), node("weapon", Some(0), [0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]), node("muzzle", Some(1), [0.0, 0.0, 0.4], [0.0, 0.0, 0.0, 1.0]), node("jet", Some(2), [0.0, 0.0, 0.05], quarter)], ..Hierarchy::default() };
    let (vertices, indices) = jet_mesh(&data(), &hierarchy, &mut Rng::new(1));
    assert_eq!((vertices.len(), indices.len()), (32, 48));
    for (k, v) in vertices.iter().enumerate() {
        let along = (k / 4) as f32 * 0.02;
        assert!((v.position[2] - (0.45 + along)).abs() < 1e-5 && v.position[1].abs() < 1e-5, "{v:?}");
        assert!((v.normal[2] - 1.0).abs() < 1e-5);
        assert!((v.color[3] - 38.0 / 255.0).abs() < 1e-6, "the chance as a byte");
    }
    // The share along the jet: 0 for the first, 1 for the last.
    assert_eq!((vertices[0].color[2], vertices[31].color[2]), (0.0, 1.0));
    assert!((vertices[31].tangent[0] - 0.2).abs() < 1e-6);
}

#[test]
fn a_shot_lights_its_time_and_goes_dark() {
    let data = data();
    let mut flash = MuzzleFlash::default();
    assert_eq!(flash.shot_vals(&data, 5.0)[0], 1.0, "nothing before a shot");
    let draws = flash.shot(&data, 5.0, &mut Rng::new(2));
    assert!(draws.random_vals.iter().all(|v| (RANDOM_SCALE..2.0 * RANDOM_SCALE).contains(v)));
    assert!((draws.rot_mx[0] - draws.rot_mx[3]).abs() < 1e-6 && (draws.rot_mx[1] + draws.rot_mx[2]).abs() < 1e-6);
    assert!(flash.shot_vals(&data, 5.0)[0] > 0.99, "dark at the shot");
    assert!(flash.shot_vals(&data, 5.0 + 0.04)[0] < 0.01, "lit a third in");
    assert!((flash.shot_vals(&data, 5.2)[0] - 1.0).abs() < 1e-6, "dark past its time");
}

/// The light the manager makes: twice the class's line, from 2 at the
/// shot to nothing half the flash's time on (the AKM's 0.06 s), off
/// before any shot and after; a reach of 6 to 12 m each shot; none for a
/// weapon whose jets have no chance.
#[test]
fn the_muzzle_s_light_is_the_class_s() {
    let data = data();
    let mut flash = MuzzleFlash::default();
    assert_eq!(MuzzleLight::intensity(&flash, &data, 5.0), 0.0, "off before a shot");
    flash.shot(&data, 5.0, &mut Rng::new(2));
    let at = |dt: f32| MuzzleLight::intensity(&flash, &data, 5.0 + dt);
    assert!((at(0.0) - 2.0).abs() < 1e-5, "twice the curve's 1 at the shot");
    assert!((at(0.03) - 1.0).abs() < 1e-4, "half way down a quarter of the flash in");
    assert_eq!(at(0.06), 0.0, "out half the flash in");
    assert_eq!(at(0.1), 0.0);
    assert!((at(0.06 - 0.06 * 0.0004) - 0.0).abs() < 1e-6, "off at 0.001 of the curve and below");
    let mut light = MuzzleLight::default();
    let mut rng = Rng::new(9);
    for _ in 0..100 {
        light.shot(&mut rng);
        assert!((LIGHT_RANGE[0]..LIGHT_RANGE[1]).contains(&light.range));
    }
    assert!(MuzzleLight::lit_by(&data));
    let mut dark = data.clone();
    dark.jets[0].chance = 0.0;
    assert!(!MuzzleLight::lit_by(&dark));
    dark.jets.clear();
    assert!(!MuzzleLight::lit_by(&dark));
}

/// The AKM's receiver's shimmer as the install has it (`probe hot`): a
/// box about (0, -0.88, 0) of extent (0.009, 0.04, 0.009), its life 0.3
/// to 0.5 more, its curves flat at (0, 0) to (1, 20), (0, 0.15) to
/// (1, 0.3) and (0, 0.01) to (1, 0.14).
fn akm_haze() -> trp_data::weapons::heat::HotHaze {
    use trp_data::formats::curve::Curve;
    trp_data::weapons::heat::HotHaze {
        center: [0.0, -0.88, 0.0],
        extent: [0.009, 0.04, 0.009],
        lifetime_delta: [0.3, 0.5],
        count: Curve::flat(&[(0.0, 0.0), (1.0, 20.0)]),
        life: Curve::flat(&[(0.0, 0.15), (1.0, 0.3)]),
        size: Curve::flat(&[(0.0, 0.01), (1.0, 0.14)]),
    }
}

/// Below about 35 heat the count's curve is under four and nothing
/// shimmers; from 123.2 on it is the curve's end: twenty every
/// twentieth of a second, 0.14 large, living 0.6 to 0.8 s.
#[test]
fn a_hot_barrel_shimmers_past_its_least() {
    let haze = akm_haze();
    assert!((barrel_haze_share(123.2) - 1.0).abs() < 1e-5);
    let mut cool = BarrelHaze::woken(0.0, &haze);
    cool.give(&haze, 35.0);
    assert!(cool.count < 4.0 && cool.puffs(&haze, 10.0, |p| p, [0.0; 3], 200, &mut Rng::new(1)).is_empty(), "{cool:?}");
    cool.give(&haze, 36.0);
    assert!(cool.count >= 4.0, "{cool:?}");

    let mut hot = BarrelHaze::woken(0.0, &haze);
    hot.give(&haze, 200.0);
    assert_eq!((hot.count, hot.size), (20.0, 0.14));
    // A second of frames at sixty: 20 at each twentieth of a second.
    let mut made = 0;
    let mut rng = Rng::new(3);
    for frame in 1..=60 {
        let puffs = hot.puffs(&haze, frame as f32 / 60.0, |p| [p[0] + 1.0, p[1], p[2]], [0.0, 0.0, 2.0], 200, &mut rng);
        for puff in &puffs {
            assert!((0.991..=1.009).contains(&puff.position[0]) && (-0.92..=-0.84).contains(&puff.position[1]), "{puff:?}");
            assert!((0.6..=0.8).contains(&puff.lifetime) && puff.velocity == [0.0, 0.0, 1.0]);
        }
        made += puffs.len();
    }
    // The twentieth step falls on the last frame's edge, which the
    // floats' sum may put a frame later.
    assert!(made == 400 || made == 380, "twenty steps of twenty in a second: {made}");
}

/// Its last emission does not move while it is too cool: the first frame
/// past its least makes every step since (the system's room holds it).
#[test]
fn a_barrel_s_first_shimmer_makes_up_the_time_it_was_cool() {
    let haze = akm_haze();
    let mut haze_state = BarrelHaze::woken(0.0, &haze);
    haze_state.give(&haze, 10.0);
    assert!(haze_state.puffs(&haze, 30.0, |p| p, [0.0; 3], 200, &mut Rng::new(1)).is_empty());
    haze_state.give(&haze, 200.0);
    assert_eq!(haze_state.puffs(&haze, 30.0, |p| p, [0.0; 3], 200, &mut Rng::new(1)).len(), 200);
    assert!((haze_state.last_emit - 30.0).abs() < 1e-3, "{haze_state:?}");
}
