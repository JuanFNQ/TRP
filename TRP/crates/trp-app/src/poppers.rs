//! The reactive targets (`docs/spec/range-targets.md`; the options'
//! `REACTIVE TARGETS`): the hideout shooting range's steel poppers, read
//! from the install as the run begins (`trp_data::range_targets`, away
//! from the frame), stood in rows out along the open start's way, each
//! moved by the simulation's popper (`trp_sim::popper`) as the bullets hit
//! it: its face folds it, its neck or edge rocks it. The plate's two
//! colliders are met by the bullets as the range's things are
//! (`bullets::RangeWorld`), turned with the plate; each popper plays its
//! own sounds where it stands.

use std::sync::atomic::AtomicU32;
use std::sync::Arc;

use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task};
use trp_sim::ballistics::{RayHit, Surface, V3};
use trp_sim::math::{quat_axis, quat_rotate};
use trp_sim::popper::{Popper, Sound, FACE_RADIUS};
use trp_sim::World as _;

use crate::render_data::PreparedLooks;
use crate::ui_sound::{Group, Levels, Mixer, Samples, Voice};

/// Where the rows stand down the lanes, in metres past the knee-high wall,
/// and across them (Bevy's x: the user's range of 2026-10-26), how many in
/// a row and how far apart (`PROVISIONAL`: the app's, for "far away"; the
/// game's rows are 6, 13.6 and 20 m from its firing line).
const ROWS: [f32; 5] = [25.0, 50.0, 100.0, 200.0, 300.0];
const ROW_X: f32 = -6.3;
const IN_A_ROW: usize = 3;
const APART: f32 = 1.6;
/// A row's sign (the user, 2026-10-26: "Add range markers for each step
/// up, so people can tell how far they are", and then "I dont want the
/// floating markers, can we make a white sign with black text?"): a white
/// board on a post to the row's left, its distance painted on it in black,
/// facing the firing line, beside the row's left end; the board
/// `SIGN_BOARD` at 25 m, and larger as the square root of how much farther
/// the row is (larger still, from the firing line the farther boards would
/// stand behind the nearer). The app's.
/// How far a row's sign stands from its leftmost popper's middle (the
/// user, 2026-10-26: "just put all the signs on the left of each pair of
/// targets").
const SIGN_GAP: f32 = 0.7;
const SIGN_POST: Vec3 = Vec3::new(0.08, 1.4, 0.08);
const SIGN_BOARD: Vec3 = Vec3::new(1.2, 0.6, 0.04);
const SIGN_SIZED_AT: f32 = 25.0;
/// The board's picture, in pixels, and the size a letter's dot is painted
/// at in it (the letters 5 dots wide and 7 high, a dot between them).
const SIGN_PICTURE: [u32; 2] = [128, 64];
const SIGN_DOT: u32 = 4;

/// The plate's two colliders in its own axes (its x across, its y toward
/// the shooter, its z up from the hinge): the face, a disc whose centre is
/// the target collider's place (read from the install); the plate and its
/// neck, a slab just behind it. Stand-ins for the colliders' meshes, by
/// their bounds (`PROVISIONAL`: range targets spec).
const FACE_HALF: f32 = 0.156;
const SLAB_MIN: V3 = [-0.155, -0.011, 0.0];
const SLAB_MAX: V3 = [0.155, 0.005, 1.11];
/// The plate's colliders' own figures (`MetalThin`, penetration level 0
/// and chance 0, ricochet 0.21, fragmentation 0.07, deviation chance 0.18,
/// deviation 0.031: range targets spec).
const STEEL: Surface = Surface { material: trp_sim::ballistics::material::METAL_THIN, figures: [0.0, 0.0, 0.21, 0.07, 0.18, 0.031] };
/// The bullets' numbers for the poppers' colliders: two a popper, the face
/// first.
pub const BULLET_POPPERS: u64 = 1 << 22;
/// The sources' fall-off: full within 1 m, straight down to nothing at 70
/// m (the rock's at 65).
const NEAR: f32 = 1.0;
const FAR: f32 = 70.0;
const FAR_ROCK: f32 = 65.0;

/// The model read and made ready away from the frame.
struct Prepared {
    nodes: Vec<(Option<usize>, Transform, bool, String)>,
    drawn: Vec<Vec<(usize, usize)>>,
    looks: PreparedLooks,
    plate: usize,
    roller: usize,
    /// The hinges (each part's place under the popper) and the face's
    /// centre under the plate, the simulation's axes.
    plate_hinge: V3,
    roller_hinge: V3,
    face: V3,
    clips: Vec<Samples>,
}

/// One popper on the range: where it stands and how it is turned (the
/// simulation's axes), its simulation, its two parts' objects, and the
/// level its sound now follows.
pub struct Placed {
    at: V3,
    turn: [f32; 4],
    pub popper: Popper,
    plate: Entity,
    roller: Entity,
    sound: Option<crate::ui_sound::Level>,
}

/// The poppers: being read, then on the range.
#[derive(Resource, Default)]
pub struct Poppers {
    task: Option<Task<anyhow::Result<Prepared>>>,
    asked: bool,
    pub placed: Vec<Placed>,
    plate_hinge: V3,
    roller_hinge: V3,
    face: V3,
    clips: Vec<Samples>,
    rng: u32,
}

fn prepare(install: trp_data::Install) -> anyhow::Result<Prepared> {
    let model = trp_data::range_targets::popper(&install)?;
    let props = &model.props;
    let mut looks = PreparedLooks::default();
    let mut made = std::collections::HashMap::new();
    let mut materials = std::collections::HashMap::new();
    let mut drawn = Vec::with_capacity(props.nodes.len());
    for node in &props.nodes {
        let mut own = Vec::new();
        for &(mesh, material) in &node.drawn {
            let material = *materials.entry(material).or_insert_with(|| looks.prop_material(&props.materials[material], &props.pictures, &mut made));
            for part in looks.custom_mesh(&props.meshes[mesh]) {
                own.push((part, material));
            }
        }
        drawn.push(own);
    }
    let nodes = props.nodes.iter().map(|node| (node.parent, crate::convert::transform(node.translation, node.rotation, node.scale), node.in_root, node.name.clone())).collect();
    let face = props.nodes.iter().find(|node| node.name == "Shooting_range_popper_target" && node.parent == Some(model.plate)).map(|node| node.translation).ok_or_else(|| anyhow::anyhow!("the popper's plate has no target collider"))?;
    let clips = trp_data::range_targets::popper_clips(&install)?.into_iter().map(|(_, clip)| Samples::of_owned(clip)).collect();
    Ok(Prepared {
        nodes,
        drawn,
        looks,
        plate: model.plate,
        roller: model.roller,
        plate_hinge: props.nodes[model.plate].translation,
        roller_hinge: props.nodes[model.roller].translation,
        face,
        clips,
    })
}

/// As the run begins: the popper asked of another thread.
pub fn read_poppers(mut poppers: ResMut<Poppers>, game: Res<crate::GameData>) {
    if poppers.asked {
        return;
    }
    poppers.asked = true;
    let install = game.unity.install.clone();
    poppers.task = Some(AsyncComputeTaskPool::get().spawn(async move { prepare(install) }));
}

/// The popper read: one put into the world at each place, its plate and
/// roller found to be turned.
pub fn place_poppers(
    mut commands: Commands,
    mut poppers: ResMut<Poppers>,
    range: Res<crate::range::Range>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some(task) = poppers.task.as_mut() else { return };
    let Some(done) = bevy::tasks::block_on(bevy::tasks::futures_lite::future::poll_once(task)) else { return };
    poppers.task = None;
    let prepared = match done {
        Ok(prepared) => prepared,
        Err(e) => {
            warn!("the reactive targets could not be read, and are not on the range: {e:#}");
            return;
        }
    };
    let Prepared { nodes, drawn, looks, plate, roller, plate_hinge, roller_hinge, face, clips } = prepared;
    let (mesh_handles, material_handles, _) = looks.add_with_pictures(&mut meshes, &mut images, &mut materials);
    // Each faces the firing line as the game's row turns its poppers: −90
    // degrees about x, their z up and their y toward the shooter (the
    // simulation's −z: back up the lanes).
    let turn = quat_axis(0, -90.0);
    let post = meshes.add(Cuboid::from_size(SIGN_POST));
    let white = materials.add(StandardMaterial { base_color: Color::srgb(0.92, 0.92, 0.9), perceptual_roughness: 0.9, ..default() });
    for (row, distance) in ROWS.iter().enumerate() {
        let z = -crate::range::KNEE_WALL_Z + distance;
        // The row's sign just to its left, its board larger the farther
        // the row.
        let grow = (distance / SIGN_SIZED_AT).sqrt().max(1.0);
        let size = SIGN_BOARD * Vec3::new(grow, grow, 1.0);
        let sign_x = ROW_X - (IN_A_ROW as f32 - 1.0) / 2.0 * APART - SIGN_GAP - size.x / 2.0;
        let ground = range.ground_height(sign_x, z);
        let foot = crate::convert::position([sign_x, ground, z]);
        let posts = if grow > 1.5 { [-0.4, 0.4] } else { [0.0, 0.0] };
        for (n, side) in posts.iter().enumerate().take(if grow > 1.5 { 2 } else { 1 }) {
            let place = Transform::from_translation(foot + Vec3::new(side * size.x, SIGN_POST.y / 2.0, -size.z)).with_scale(Vec3::new(grow, 1.0, grow));
            commands.spawn((Name::new(format!("range sign {distance} m, post {n}")), Mesh3d(post.clone()), MeshMaterial3d(white.clone()), place));
        }
        let board_at = foot + Vec3::Y * (SIGN_POST.y + size.y / 2.0);
        commands.spawn((Name::new(format!("range sign {distance} m, its board")), Mesh3d(meshes.add(Cuboid::from_size(size))), MeshMaterial3d(white.clone()), Transform::from_translation(board_at)));
        // Its face toward the firing line (Bevy's +z), with the distance.
        let face = materials.add(StandardMaterial { base_color_texture: Some(images.add(sign_picture(&format!("{distance:.0} M")))), perceptual_roughness: 0.9, ..default() });
        commands.spawn((
            Name::new(format!("range sign {distance} m, its face")),
            Mesh3d(meshes.add(Rectangle::new(size.x, size.y))),
            MeshMaterial3d(face),
            Transform::from_translation(board_at + Vec3::Z * (size.z / 2.0 + 0.002)),
        ));
        for k in 0..IN_A_ROW {
            let x = ROW_X + (k as f32 - (IN_A_ROW as f32 - 1.0) / 2.0) * APART;
            let at = [x, range.ground_height(x, z), z];
            let top = commands.spawn((Name::new(format!("popper {row}.{k}")), crate::convert::transform(at, turn, [1.0; 3]), Visibility::Inherited)).id();
            let mut entities: Vec<Entity> = Vec::with_capacity(nodes.len());
            for (index, (parent, place, _, name)) in nodes.iter().enumerate() {
                let under = parent.and_then(|parent| entities.get(parent).copied()).unwrap_or(top);
                let place = if index == 0 { Transform::IDENTITY } else { *place };
                entities.push(commands.spawn((Name::new(name.clone()), place, Visibility::Inherited, ChildOf(under))).id());
            }
            for (index, own) in drawn.iter().enumerate() {
                let on = if nodes[index].2 { top } else { entities[index] };
                for &(mesh, material) in own {
                    commands.spawn((Mesh3d(mesh_handles[mesh].clone()), MeshMaterial3d(material_handles[material].clone()), Transform::IDENTITY, ChildOf(on)));
                }
            }
            poppers.placed.push(Placed { at, turn, popper: Popper::default(), plate: entities[plate], roller: entities[roller], sound: None });
        }
    }
    (poppers.plate_hinge, poppers.roller_hinge, poppers.face, poppers.clips) = (plate_hinge, roller_hinge, face, clips);
    info!("the reactive targets: {} poppers, {} m to {} m down the lanes past the knee-high wall, each row with its sign", poppers.placed.len(), ROWS[0], ROWS[ROWS.len() - 1]);
}

/// A letter of the signs' own dot letters: seven rows of five dots, the
/// highest bit the leftmost (the app's).
fn letter(c: char) -> [u8; 7] {
    match c {
        '0' => [0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110],
        '1' => [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        '2' => [0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111],
        '3' => [0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110],
        '4' => [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010],
        '5' => [0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110],
        '6' => [0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110],
        '7' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000],
        '8' => [0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110],
        '9' => [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100],
        'M' => [0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001],
        _ => [0; 7],
    }
}

/// A sign's picture: white, its words painted black in the middle.
fn sign_picture(words: &str) -> Image {
    use bevy::image::{ImageSampler, ImageSamplerDescriptor};
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
    let [width, height] = SIGN_PICTURE;
    let mut pixels = vec![235u8; (width * height * 4) as usize];
    let letters: Vec<[u8; 7]> = words.chars().map(letter).collect();
    let wide = letters.len() as u32 * 6 * SIGN_DOT - SIGN_DOT;
    let (left, top) = (width.saturating_sub(wide) / 2, height.saturating_sub(7 * SIGN_DOT) / 2);
    for (n, rows) in letters.iter().enumerate() {
        for (row, bits) in rows.iter().enumerate() {
            for col in 0..5u32 {
                if bits & (0b10000 >> col) == 0 {
                    continue;
                }
                for dy in 0..SIGN_DOT {
                    for dx in 0..SIGN_DOT {
                        let (x, y) = (left + (n as u32 * 6 + col) * SIGN_DOT + dx, top + row as u32 * SIGN_DOT + dy);
                        if x < width && y < height {
                            let at = ((y * width + x) * 4) as usize;
                            pixels[at..at + 3].copy_from_slice(&[18, 18, 18]);
                        }
                    }
                }
            }
        }
    }
    for alpha in pixels.iter_mut().skip(3).step_by(4) {
        *alpha = 255;
    }
    let mut image = Image::new(Extent3d { width, height, depth_or_array_layers: 1 }, TextureDimension::D2, pixels, TextureFormat::Rgba8UnormSrgb, bevy::asset::RenderAssetUsages::default());
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor::linear());
    image
}

/// A point of a popper's plate, in its own axes, where it is now in the
/// world (the simulation's axes).
fn plate_to_world(placed: &Placed, hinge: V3, local: V3) -> V3 {
    let turned = quat_rotate(quat_axis(0, placed.popper.plate), local);
    let in_popper = [hinge[0] + turned[0], hinge[1] + turned[1], hinge[2] + turned[2]];
    let r = quat_rotate(placed.turn, in_popper);
    [placed.at[0] + r[0], placed.at[1] + r[1], placed.at[2] + r[2]]
}

/// A point of the world in a popper's plate's own axes.
fn world_to_plate(placed: &Placed, hinge: V3, world: V3) -> V3 {
    let conj = |q: [f32; 4]| [-q[0], -q[1], -q[2], q[3]];
    let off = [world[0] - placed.at[0], world[1] - placed.at[1], world[2] - placed.at[2]];
    let in_popper = quat_rotate(conj(placed.turn), off);
    quat_rotate(conj(quat_axis(0, placed.popper.plate)), [in_popper[0] - hinge[0], in_popper[1] - hinge[1], in_popper[2] - hinge[2]])
}

/// A direction of a plate's own axes in the world.
fn plate_way(placed: &Placed, local: V3) -> V3 {
    quat_rotate(placed.turn, quat_rotate(quat_axis(0, placed.popper.plate), local))
}

/// Where a ray from `o` along `d` (both in a box's axes, `t` 0 to 1) enters
/// the box through a face turned toward it, and that face's normal; none
/// from inside.
fn entered_box(o: V3, d: V3, min: V3, max: V3) -> Option<(f32, V3)> {
    let (mut near, mut far, mut axis, mut sign) = (0.0f32, 1.0f32, None, 0.0);
    for k in 0..3 {
        if d[k].abs() < 1e-12 {
            if o[k] < min[k] || o[k] > max[k] {
                return None;
            }
            continue;
        }
        let (a, b) = ((min[k] - o[k]) / d[k], (max[k] - o[k]) / d[k]);
        let (t0, t1, s) = if a < b { (a, b, -1.0) } else { (b, a, 1.0) };
        if t0 > near {
            (near, axis, sign) = (t0, Some(k), s);
        }
        far = far.min(t1);
        if near > far {
            return None;
        }
    }
    let axis = axis?;
    let mut normal = [0.0; 3];
    normal[axis] = sign;
    Some((near, normal))
}

impl Poppers {
    /// What a bullet's cast meets of the poppers (`BulletWorld::ray`): each
    /// plate's face from its front, and its slab, each once.
    pub fn ray(&self, from: V3, to: V3, hits: &mut Vec<RayHit>) {
        let way = [to[0] - from[0], to[1] - from[1], to[2] - from[2]];
        let length = (way[0] * way[0] + way[1] * way[1] + way[2] * way[2]).sqrt();
        if length < 1e-9 {
            return;
        }
        for (index, placed) in self.placed.iter().enumerate() {
            // Far from it, nothing (its whole reach is within 1.3 m of its foot).
            let mid = [placed.at[0], placed.at[1] + 0.6, placed.at[2]];
            let t = (((mid[0] - from[0]) * way[0] + (mid[1] - from[1]) * way[1] + (mid[2] - from[2]) * way[2]) / (length * length)).clamp(0.0, 1.0);
            let near = [from[0] + way[0] * t - mid[0], from[1] + way[1] * t - mid[1], from[2] + way[2] * t - mid[2]];
            if near[0] * near[0] + near[1] * near[1] + near[2] * near[2] > 1.7 {
                continue;
            }
            let o = world_to_plate(placed, self.plate_hinge, from);
            let e = world_to_plate(placed, self.plate_hinge, to);
            let d = [e[0] - o[0], e[1] - o[1], e[2] - o[2]];
            let id = BULLET_POPPERS + index as u64 * 2;
            // The face: a disc in the plane through its centre, met only
            // from its front (its y toward the shooter).
            let face = self.face;
            if d[1] < 0.0 && o[1] > face[1] {
                let t = (face[1] - o[1]) / d[1];
                if (0.0..=1.0).contains(&t) {
                    let (x, z) = (o[0] + d[0] * t - face[0], o[2] + d[2] * t - face[2]);
                    if x * x + z * z <= FACE_HALF * FACE_HALF {
                        let point = [from[0] + way[0] * t, from[1] + way[1] * t, from[2] + way[2] * t];
                        hits.push(RayHit { distance: t * length, point, normal: plate_way(placed, [0.0, 1.0, 0.0]), collider: id, thing: id, mesh: true, surface: STEEL, body: None });
                    }
                }
            }
            if let Some((t, normal)) = entered_box(o, d, SLAB_MIN, SLAB_MAX) {
                let point = [from[0] + way[0] * t, from[1] + way[1] * t, from[2] + way[2] * t];
                hits.push(RayHit { distance: t * length, point, normal: plate_way(placed, normal), collider: id + 1, thing: id + 1, mesh: true, surface: STEEL, body: None });
            }
        }
    }

    /// The fold sound's draw (`Random.Range(0, 2)`).
    fn draw(&mut self) -> u32 {
        self.rng = self.rng.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        (self.rng >> 16) & 1
    }
}

/// Each popper the frame's bullets hit told so, once a collider (the face
/// first: on the target within its radius, else a miss; the slab a miss),
/// every popper moved on, its plate and roller turned, its sounds played.
pub fn move_poppers(
    mut poppers: ResMut<Poppers>,
    bullets: Option<Res<crate::bullets::Bullets>>,
    time: Res<Time>,
    mut transforms: Query<&mut Transform>,
    mixer: Option<Res<Mixer>>,
    levels: Option<Res<Levels>>,
    camera: Query<&GlobalTransform, With<crate::MainCamera>>,
) {
    if poppers.placed.is_empty() {
        return;
    }
    let poppers = &mut *poppers;
    let mut reports: Vec<(usize, bool, bool)> = Vec::new();
    for hit in bullets.iter().flat_map(|bullets| &bullets.hits) {
        let Some(n) = hit.collider.checked_sub(BULLET_POPPERS).filter(|n| (*n as usize) < poppers.placed.len() * 2) else { continue };
        let (index, face) = ((n / 2) as usize, n % 2 == 0);
        let local = world_to_plate(&poppers.placed[index], poppers.plate_hinge, hit.point);
        info!("popper {index}: met on its {} at ({:.3}, {:.3}, {:.3}) of its plate (the face's centre at ({:.3}, {:.3}, {:.3}))", if face { "face" } else { "slab" }, local[0], local[1], local[2], poppers.face[0], poppers.face[1], poppers.face[2]);
        let on_target = face && {
            let centre = plate_to_world(&poppers.placed[index], poppers.plate_hinge, poppers.face);
            let gap = [hit.point[0] - centre[0], hit.point[1] - centre[1], hit.point[2] - centre[2]];
            (gap[0] * gap[0] + gap[1] * gap[1] + gap[2] * gap[2]).sqrt() < FACE_RADIUS
        };
        match reports.iter_mut().find(|(i, f, _)| *i == index && *f == face) {
            Some(report) => report.2 |= on_target,
            None => reports.push((index, face, on_target)),
        }
    }
    reports.sort_by_key(|&(index, face, _)| (index, !face));
    for (index, _, on_target) in reports {
        let draw = poppers.draw();
        poppers.placed[index].popper.hit(on_target, draw);
        info!("popper {index}: {}", if on_target { "hit on its face" } else { "a miss: rocked" });
    }
    let dt = time.delta_secs();
    let camera = camera.iter().next();
    let overall = levels.map_or(1.0, |levels| levels.overall_gain());
    for placed in &mut poppers.placed {
        placed.popper.step(dt);
        if let Ok(mut transform) = transforms.get_mut(placed.plate) {
            transform.rotation = crate::convert::rotation(quat_axis(0, placed.popper.plate));
        }
        if let Ok(mut transform) = transforms.get_mut(placed.roller) {
            transform.rotation = crate::convert::rotation(quat_axis(0, placed.popper.roller));
        }
        let sounds = std::mem::take(&mut placed.popper.sounds);
        let (Some(mixer), Some(camera)) = (mixer.as_ref(), camera) else { continue };
        for sound in sounds {
            let (clip, volume, far) = match sound {
                Sound::Fold { second, volume } => (usize::from(second), volume, FAR),
                Sound::Unfold { volume } => (2, volume, FAR),
                Sound::Rock => (3, 1.0, FAR_ROCK),
            };
            // The one before stops (`PlayAudio`).
            if let Some(level) = placed.sound.take() {
                crate::ui_sound::set_level(&level, -144.0);
            }
            let Some(samples) = poppers.clips.get(clip) else { continue };
            let source = crate::convert::position([placed.at[0], placed.at[1] + 0.6, placed.at[2]]);
            let to_source = source - camera.translation();
            let distance = to_source.length();
            if distance >= far || volume <= 0.0 {
                continue;
            }
            let falloff = if distance <= NEAR { 1.0 } else { 1.0 - (distance - NEAR) / (far - NEAR) };
            let side = if distance > 1e-4 { (to_source / distance).dot(camera.right().as_vec3()).clamp(-1.0, 1.0) } else { 0.0 };
            let angle = (side + 1.0) * std::f32::consts::FRAC_PI_4;
            let gain = volume.clamp(0.0, 1.0) * falloff * overall;
            let own: crate::ui_sound::Level = Arc::new(AtomicU32::new(0.0f32.to_bits()));
            mixer.0.add_to(Group::Main, Voice::placed(samples, [angle.cos() * gain, angle.sin() * gain], Arc::new(AtomicU32::new(1.0f32.to_bits()))).following_level(own.clone()));
            placed.sound = Some(own);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A ray into a box's face is met there with that face's normal; one
    /// from inside, or passing by, is not.
    #[test]
    fn a_box_is_entered_through_the_face_turned_toward_the_ray() {
        let (min, max) = ([-1.0, -1.0, -1.0], [1.0, 1.0, 1.0]);
        let (t, normal) = entered_box([0.0, 5.0, 0.0], [0.0, -10.0, 0.0], min, max).unwrap();
        assert!((t - 0.4).abs() < 1e-6);
        assert_eq!(normal, [0.0, 1.0, 0.0]);
        assert!(entered_box([0.0, 0.0, 0.0], [0.0, -10.0, 0.0], min, max).is_none());
        assert!(entered_box([3.0, 5.0, 0.0], [0.0, -10.0, 0.0], min, max).is_none());
    }
}
