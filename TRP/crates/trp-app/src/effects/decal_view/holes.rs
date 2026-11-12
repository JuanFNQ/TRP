//! Bullet holes and blood: their pictures read, prepared and laid.

use super::*;

/// With no decals laid, the bleedings' starts and ends are let go.
pub(super) fn forget_bleedings(mut controller: ResMut<crate::player::controller::Controller>, dummies: Option<ResMut<crate::world::dummy::Dummies>>) {
    controller.bleeding_drops.clear();
    if let Some(mut dummies) = dummies {
        dummies.bleeding_drops.clear();
    }
}

/// The bullets' decals read, their atlas, each decal's tag of its place in
/// it, the static ring (each sheet's places by the shared index) and the
/// moving holes' ring, and Unity's generator for the draws.
#[derive(Resource)]
pub struct BulletHoles {
    pub(super) decals: Vec<(Vec<i32>, trp_data::drawing::particles::DeferredDecal, u32)>,
    generic: Option<(trp_data::drawing::particles::DeferredDecal, u32)>,
    atlas: Handle<Image>,
    pub(super) laid: Vec<Vec<Option<Entity>>>,
    index: usize,
    moving: Vec<Option<Entity>>,
    moving_index: usize,
    random: trp_sim::base::unity_random::UnityRandom,
    /// The blood's two decals, if read, and the characters bleeding
    /// heavily with each one's next drop.
    blood: Option<Blood>,
    bleedings: trp_sim::combat::impacts::Bleedings,
}

/// The renderer's two decals of blood, each with its picture and its tag
/// (its whole picture, `WHOLE_PICTURE`), their sheets in the ring after the
/// bullets' generic one; the time between a bleeding's drops, and how far
/// down from the feet a drop is looked for (`DRAW_BLEEDING_MAX_DISTANCE`).
pub(super) struct Blood {
    environment: (trp_data::drawing::particles::DeferredDecal, Handle<Image>),
    bleeding: (trp_data::drawing::particles::DeferredDecal, Handle<Image>),
    spawn_delta: [f32; 2],
    reach: f32,
}

/// A tag's place for a decal whose picture is its own: the whole of it.
const WHOLE_PICTURE: u32 = (63 << 12) | (63 << 18);

/// The player's character, among the bleedings (the dummies' are 1 on).
const PLAYER: u32 = 0;

/// A decal's picture with each of its own levels, its colour (`_Color`)
/// put on every texel as the decal's shader puts it on the sample (the
/// picture times the colour, its alpha too), its rows in their stored
/// order as the atlas's.
fn coloured_picture(decal: &trp_data::drawing::particles::DeferredDecal) -> Option<Image> {
    let levels = trp_data::unity::texture::mips(&decal.picture).ok()?;
    let mut data = Vec::new();
    for level in &levels {
        let px = trp_data::unity::texture::decode_rgba(&decal.picture, *level).ok()?;
        data.extend(px.chunks_exact(4).flat_map(|p| -> [u8; 4] { std::array::from_fn(|c| (f32::from(p[c]) * decal.color[c]).round().clamp(0.0, 255.0) as u8) }));
    }
    let mut image = Image::new(Extent3d { width: decal.picture.width, height: decal.picture.height, depth_or_array_layers: 1 }, TextureDimension::D2, data, TextureFormat::Rgba8Unorm, RenderAssetUsages::RENDER_WORLD);
    image.texture_descriptor.mip_level_count = levels.len() as u32;
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::ClampToEdge,
        address_mode_v: ImageAddressMode::ClampToEdge,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Linear,
        ..default()
    });
    Some(image)
}

/// A still decal's box and tag as `AddCubeToMesh` draws them: its size,
/// then its turn where it turns at random, then its tile (the column's
/// draw, then the row's: `GenerateTangents`), from Unity's generator; as a
/// clustered decal's matrix, in the app's axes.
fn drawn(random: &mut trp_sim::base::unity_random::UnityRandom, decal: &trp_data::drawing::particles::DeferredDecal, base: u32, position: [f32; 3], normal: [f32; 3]) -> (Mat4, u32, f32) {
    let size = random.range(decal.sizes[0], decal.sizes[1]);
    let share = if decal.sizes[1] != decal.sizes[0] { (size - decal.sizes[0]) / (decal.sizes[1] - decal.sizes[0]) } else { 0.0 };
    let turn = decal.randomize_rotation.then(|| random.range_int(0, 359) as f32);
    let column = random.range_int(0, decal.columns as i32) as u32;
    let row = random.range_int(0, decal.rows as i32) as u32;
    // The game puts the row's draw on the picture's first axis and the
    // column's on the second (`GenerateTangents`).
    let tag = base | ((row.min(3)) << 24) | ((column.min(3)) << 26) | ((decal.columns.clamp(1, 4) - 1) << 28) | ((decal.rows.clamp(1, 4) - 1) << 30);
    let b = trp_sim::effects::decals::static_box(position, normal, decal.sizes, share, decal.projector_height, turn);
    let axis = |v: [f32; 3], length: f32| crate::platform::convert::position(v) * length;
    let matrix = Mat4::from_cols(
        axis(b.right, 2.0 * b.half[0]).extend(0.0),
        axis(b.forward.map(|c| -c), 2.0 * b.half[2]).extend(0.0),
        axis(b.normal, 2.0 * b.half[1]).extend(0.0),
        crate::platform::convert::position(b.center).extend(1.0),
    );
    (matrix, tag, b.half[0])
}

/// A still decal put in the ring's next place on its sheet, the one there
/// before taken away.
fn into_ring(commands: &mut Commands, laid: &mut [Vec<Option<Entity>>], index: &mut usize, sheet: usize, entity: Entity) {
    let slot = *index;
    if let Some(old) = laid[sheet][slot].replace(entity) {
        commands.entity(old).try_despawn();
    }
    *index = (slot + 1) % laid[sheet].len();
}

/// The atlas's sides are cut into 64 parts each, a picture's place and
/// size given in them (and so a multiple of a 64th of the atlas).
const ATLAS_PARTS: u32 = 64;

const ATLAS_WIDTH: u32 = 1024;

/// The most moving holes at once (`_maxDynamicDecals`, as the install has it).
const MOVING_HOLES: usize = 200;

/// A picture's place in the atlas as a tag's bits: its corner and size in
/// 64ths of the atlas (the size less one), each six bits.
fn place_bits(x: u32, y: u32, w: u32, h: u32, height: u32) -> u32 {
    let (ux, uy) = (ATLAS_WIDTH / ATLAS_PARTS, height / ATLAS_PARTS);
    (x / ux) | ((y / uy) << 6) | ((w / ux - 1) << 12) | ((h / uy - 1) << 18)
}

/// The bullets' decals, their pictures packed into one atlas (on shelves,
/// tallest first; each picture's rows in their stored order, as every
/// picture of the game's is laid), and its levels made by halving.
///
/// Read away from the frame, on a thread with a reader of its own, as the
/// run begins (`receive_holes` puts them in the stores when they come): on
/// the frame's thread they took 0.2 s of the start, and nothing at the
/// start shows them. A frame that lays one before they come waits for them
/// (`receive_holes`).
pub(super) fn read_holes(mut commands: Commands, game: Res<crate::GameData>) {
    let install = game.unity.install.clone();
    let reach = game.hard.float("DRAW_BLEEDING_MAX_DISTANCE").map_err(|e| warn!("a bleeding's drops are not laid: {e:#}")).ok();
    let task = bevy::tasks::AsyncComputeTaskPool::get().spawn(async move {
        let mut unity = trp_data::unity::Assets::new(install)?;
        prepare_holes(&mut unity)
    });
    commands.insert_resource(HolesReading { task, reach });
}

/// The bullets' holes being read (`read_holes`), and how far down a
/// bleeding's drop is looked for.
#[derive(Resource)]
pub(super) struct HolesReading {
    task: bevy::tasks::Task<anyhow::Result<PreparedHoles>>,
    reach: Option<f32>,
}

/// The bullets' holes made ready: their decals with their tags, the atlas
/// of their pictures, the static ring's length, and the blood's decals.
pub(super) struct PreparedHoles {
    pub(super) decals: Vec<(Vec<i32>, trp_data::drawing::particles::DeferredDecal, u32)>,
    generic: Option<(trp_data::drawing::particles::DeferredDecal, u32)>,
    atlas: Image,
    most: usize,
    blood: Option<PreparedBlood>,
}

pub(super) struct PreparedBlood {
    environment: (trp_data::drawing::particles::DeferredDecal, Image),
    bleeding: (trp_data::drawing::particles::DeferredDecal, Image),
    spawn_delta: [f32; 2],
}

/// The bullets' holes, once read: into the stores, laid from then on. A
/// frame with one to lay before they have come (a bullet's hole, the blood
/// behind a hit, a bleeding's start or end) waits for them, so that none is
/// lost to the read (`background::take`) rather than read at the start.
pub(super) fn receive_holes(
    mut commands: Commands,
    reading: Option<ResMut<HolesReading>>,
    mut images: ResMut<Assets<Image>>,
    (bullets, controller, dummies): (Option<Res<crate::weapons::bullets::Bullets>>, Res<crate::player::controller::Controller>, Option<Res<crate::world::dummy::Dummies>>),
) {
    let Some(mut reading) = reading else { return };
    let wanted = bullets.is_some_and(|bullets| !bullets.wall_blood.is_empty() || bullets.effects.iter().any(|(_, entry)| bullets.entries.get(*entry).is_some_and(|e| e.deferred_decal)))
        || !controller.bleeding_drops.is_empty()
        || dummies.is_some_and(|dummies| !dummies.bleeding_drops.is_empty());
    let Some(made) = crate::assets::background::take(&mut reading.task, wanted, "the bullets' holes") else { return };
    let reach = reading.reach;
    commands.remove_resource::<HolesReading>();
    let PreparedHoles { decals, generic, atlas, most, blood } = match made {
        Ok(made) => made,
        Err(e) => {
            warn!("the bullets' holes are not laid: {e:#}");
            return;
        }
    };
    let blood = match (blood, reach) {
        (Some(blood), Some(reach)) => Some(Blood {
            environment: (blood.environment.0, images.add(blood.environment.1)),
            bleeding: (blood.bleeding.0, images.add(blood.bleeding.1)),
            spawn_delta: blood.spawn_delta,
            reach,
        }),
        _ => None,
    };
    info!("the bullets' holes: read away from the frame, laid from now on");
    let laid = vec![vec![None; most]; decals.len() + 3];
    let random = trp_sim::base::unity_random::UnityRandom::new(crate::fixed_seed(0xdeca1).map(|seed| seed as i32).unwrap_or_else(|| std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |d| d.subsec_nanos() as i32)));
    commands.insert_resource(BulletHoles { decals, generic, atlas: images.add(atlas), laid, index: 0, moving: vec![None; MOVING_HOLES], moving_index: 0, random, blood, bleedings: trp_sim::combat::impacts::Bleedings::default() });
}

/// Reads the bullets' decals and the blood's and makes their pictures, away
/// from the frame (`read_holes`).
fn prepare_holes(unity: &mut trp_data::unity::Assets) -> anyhow::Result<PreparedHoles> {
    let read = trp_data::drawing::particles::deferred_bullet_decals(unity)?;
    let mut pictures: Vec<(usize, u32, u32, Vec<u8>)> = Vec::new();
    let all: Vec<&trp_data::drawing::particles::DeferredDecal> = read.by_material.iter().map(|(_, d)| d).chain([&read.generic]).collect();
    for (at, decal) in all.iter().enumerate() {
        let decoded = trp_data::unity::texture::mips(&decal.picture).ok().and_then(|levels| levels.first().copied()).and_then(|level| trp_data::unity::texture::decode_rgba(&decal.picture, level).ok().map(|px| (level.width, level.height, px)));
        match decoded {
            Some((w, h, px)) => pictures.push((at, w, h, px)),
            None => warn!("a bullet hole's picture {} is not read: left out", decal.picture.name),
        }
    }
    // On shelves of the atlas's width, the tallest first; each place a
    // multiple of a 64th of the side (the generic decal's 30 by 29 is laid
    // in a 32 by 32 place).
    let unit = ATLAS_WIDTH / ATLAS_PARTS;
    let fit = |n: u32| n.div_ceil(unit) * unit;
    let mut order: Vec<usize> = (0..pictures.len()).collect();
    order.sort_by_key(|&i| std::cmp::Reverse(pictures[i].2));
    let (mut x, mut y, mut shelf) = (0u32, 0u32, 0u32);
    let mut places = vec![(0u32, 0u32, 0u32, 0u32); pictures.len()];
    for &i in &order {
        let (w, h) = (fit(pictures[i].1), fit(pictures[i].2));
        if x + w > ATLAS_WIDTH {
            (x, y, shelf) = (0, y + shelf, 0);
        }
        places[i] = (x, y, w, h);
        x += w;
        shelf = shelf.max(h);
    }
    let height = (y + shelf).next_power_of_two().max(ATLAS_PARTS);
    let mut atlas = vec![0u8; (ATLAS_WIDTH * height * 4) as usize];
    for (i, (_, w, h, px)) in pictures.iter().enumerate() {
        let (px0, py0, _, _) = places[i];
        for row in 0..*h {
            let from = (row * w * 4) as usize;
            let to = (((py0 + row) * ATLAS_WIDTH + px0) * 4) as usize;
            atlas[to..to + (w * 4) as usize].copy_from_slice(&px[from..from + (w * 4) as usize]);
        }
    }
    // Its levels, each the one above halved by the mean of four.
    let mut data = atlas.clone();
    let (mut lw, mut lh, mut level) = (ATLAS_WIDTH, height, atlas);
    let mut levels = 1;
    while lw > 1 || lh > 1 {
        let (nw, nh) = ((lw / 2).max(1), (lh / 2).max(1));
        let mut next = vec![0u8; (nw * nh * 4) as usize];
        for j in 0..nh {
            for i in 0..nw {
                for c in 0..4 {
                    let at = |x: u32, y: u32| level[((y.min(lh - 1) * lw + x.min(lw - 1)) * 4 + c) as usize] as u32;
                    let sum = at(2 * i, 2 * j) + at(2 * i + 1, 2 * j) + at(2 * i, 2 * j + 1) + at(2 * i + 1, 2 * j + 1);
                    next[((j * nw + i) * 4 + c) as usize] = ((sum + 2) / 4) as u8;
                }
            }
        }
        data.extend_from_slice(&next);
        (lw, lh, level) = (nw, nh, next);
        levels += 1;
    }
    let mut image = Image::new(Extent3d { width: ATLAS_WIDTH, height, depth_or_array_layers: 1 }, TextureDimension::D2, data, TextureFormat::Rgba8Unorm, RenderAssetUsages::RENDER_WORLD);
    image.texture_descriptor.mip_level_count = levels;
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::ClampToEdge,
        address_mode_v: ImageAddressMode::ClampToEdge,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Linear,
        ..default()
    });
    let atlas = image;
    let tag_of = |at: usize| -> Option<u32> {
        let i = pictures.iter().position(|p| p.0 == at)?;
        let (x, y, _, _) = places[i];
        Some(place_bits(x, y, fit(pictures[i].1), fit(pictures[i].2), height))
    };
    let count = read.by_material.len();
    let decals: Vec<(Vec<i32>, trp_data::drawing::particles::DeferredDecal, u32)> = read.by_material.into_iter().enumerate().filter_map(|(at, (m, d))| Some((m, d, tag_of(at)?))).collect();
    let generic = tag_of(count).map(|tag| (read.generic, tag));
    let most = decals.first().map_or(200, |d| d.1.max_decals as usize);
    for (materials, d, _) in &decals {
        info!("a bullet's hole on {materials:?}: {} ({}x{}, {}x{} cells, {:?} m, by {})", d.picture.name, d.picture.width, d.picture.height, d.columns, d.rows, d.sizes, d.shader);
    }
    info!("the bullets' holes: {} sheets in an atlas of {ATLAS_WIDTH} by {height}, at most {most} still and {MOVING_HOLES} moving", decals.len());
    // The blood's two, after the generic one in the ring.
    let blood = match trp_data::drawing::particles::deferred_blood_decals(unity) {
        Ok(read) => {
            let environment = coloured_picture(&read.environment);
            let bleeding = coloured_picture(&read.bleeding);
            for (name, d) in [("behind a hit", &read.environment), ("under a bleeding", &read.bleeding)] {
                info!("blood {name}: {} ({}x{}, {}x{} cells, {:?} m, colour {:?}, by {})", d.picture.name, d.picture.width, d.picture.height, d.columns, d.rows, d.sizes, d.color, d.shader);
            }
            info!("a heavy bleeding's drops every {:?} s", read.bleeding_spawn_delta);
            match (environment, bleeding) {
                (Some(e), Some(b)) => Some(PreparedBlood { environment: (read.environment, e), bleeding: (read.bleeding, b), spawn_delta: read.bleeding_spawn_delta }),
                _ => {
                    warn!("the blood's pictures are not read: no blood laid");
                    None
                }
            }
        }
        Err(e) => {
            warn!("the blood's decals are not read: {e:#}");
            None
        }
    };
    Ok(PreparedHoles { decals, generic, atlas, most, blood })
}

/// The frame's bullets' holes (`Effect.Emit` → `DeferredDecalRenderer.
/// DrawDecal`, for each effect played whose entry lays a deferred decal):
/// the struck material's decal, its size and turn drawn as the box's
/// (`AddCubeToMesh`), then its tile (`GenerateTangents`: the column's draw,
/// then the row's), as a clustered decal over the atlas; on a door's leaf,
/// a child of the leaf, so that it swings with it.
pub fn lay_holes(
    mut commands: Commands,
    bullets: Res<crate::weapons::bullets::Bullets>,
    mut holes: ResMut<BulletHoles>,
    parts: Query<(Entity, &crate::ThingPart, &GlobalTransform)>,
) {
    let holes = &mut *holes;
    // The blood behind a body's hit, laid as the hit was handed on: before
    // the frame's effects play.
    if let Some(blood) = holes.blood.as_ref() {
        let sheet = holes.decals.len() + 1;
        for &(point, normal) in &bullets.wall_blood {
            let (decal, picture) = &blood.environment;
            let (matrix, tag, half) = drawn(&mut holes.random, decal, WHOLE_PICTURE, point, normal);
            let made = ClusteredDecal { base_color_texture: Some(picture.clone()), normal_map_texture: None, metallic_roughness_texture: None, emissive_texture: None, tag };
            let entity = commands.spawn((made, Transform::from_matrix(matrix))).id();
            into_ring(&mut commands, &mut holes.laid, &mut holes.index, sheet, entity);
            info!("blood behind a hit at ({:.2}, {:.2}, {:.2}), {:.2} m across", point[0], point[1], point[2], 2.0 * half);
        }
    }
    for (impact, entry) in &bullets.effects {
        // (A blade's hit lays no decal.)
        if impact.knife || !bullets.entries.get(*entry).is_some_and(|e| e.deferred_decal) {
            continue;
        }
        let which = holes.decals.iter().position(|(m, _, _)| m.contains(&impact.material));
        let (decal, base) = match which {
            Some(i) => (holes.decals[i].1.clone(), holes.decals[i].2),
            None => match &holes.generic {
                Some((d, tag)) => (d.clone(), *tag),
                None => continue,
            },
        };
        let (matrix, tag, _) = drawn(&mut holes.random, &decal, base, impact.position, impact.normal);
        let made = ClusteredDecal { base_color_texture: Some(holes.atlas.clone()), normal_map_texture: None, metallic_roughness_texture: None, emissive_texture: None, tag };
        // On a door's leaf: a child of the leaf as it stands.
        let leaf = crate::world::range::bullet_thing_part(impact.thing).and_then(|thing| parts.iter().find(|(_, part, _)| part.0 == thing));
        if let Some((parent, _, at)) = leaf {
            let local = at.affine().inverse() * bevy::math::Affine3A::from_mat4(matrix);
            let entity = commands.spawn((made, Transform::from_matrix(Mat4::from(local)))).id();
            commands.entity(parent).add_child(entity);
            let slot = holes.moving_index;
            if let Some(old) = holes.moving[slot].replace(entity) {
                commands.entity(old).try_despawn();
            }
            holes.moving_index = (slot + 1) % MOVING_HOLES;
        } else {
            let entity = commands.spawn((made, Transform::from_matrix(matrix))).id();
            let sheet = which.unwrap_or(holes.decals.len());
            into_ring(&mut commands, &mut holes.laid, &mut holes.index, sheet, entity);
        }
    }
}

/// The drops under a character bleeding heavily (`EffectsCommutator.
/// Update`, `UpdatePlayersBleedings`, `EmitBleeding`): the bleedings'
/// starts and ends since the last frame taken (the player's and each
/// dummy's), then each one due cast down from its feet within the reach on
/// the effects' layers and, where that meets something, the renderer's
/// bleeding decal laid there as a still thing's hole is, its next due 1 to
/// 3 s on (`trp_sim::impacts::Bleedings`). At the game's time (the app's
/// clock). `PROVISIONAL`: after the frame's holes (the order of the game's
/// two updates not read).
pub fn lay_bleeding(
    mut commands: Commands,
    holes: Option<ResMut<BulletHoles>>,
    mut controller: ResMut<crate::player::controller::Controller>,
    dummies: Option<ResMut<crate::world::dummy::Dummies>>,
    range: Res<crate::world::range::Range>,
    time: Res<Time>,
) {
    let mut dummies = dummies;
    let changes: Vec<(u32, bool)> = controller.bleeding_drops.drain(..).map(|on| (PLAYER, on)).chain(dummies.as_mut().map(|d| d.bleeding_drops.drain(..).collect::<Vec<_>>()).unwrap_or_default()).collect();
    let Some(mut holes) = holes else { return };
    let holes = &mut *holes;
    for (character, on) in changes {
        if on {
            holes.bleedings.start(character);
        } else {
            holes.bleedings.stop(character);
        }
    }
    let Some(blood) = holes.blood.as_ref() else { return };
    let sheet = holes.decals.len() + 2;
    let player = controller.movement.position;
    let feet = |character: u32| -> Option<[f32; 3]> { if character == PLAYER { Some(player) } else { dummies.as_ref()?.feet(character) } };
    let (laid, index) = (&mut holes.laid, &mut holes.index);
    holes.bleedings.update(time.elapsed_secs(), blood.spawn_delta, &mut holes.random, |character, random| {
        let Some(from) = feet(character) else { return false };
        let Some(found) = trp_sim::World::ray(&*range, from, [0.0, -1.0, 0.0], blood.reach, trp_sim::weapons::mounting::Layer::Environment) else { return false };
        let (decal, picture) = &blood.bleeding;
        let (matrix, tag, half) = drawn(random, decal, WHOLE_PICTURE, found.point, found.normal);
        let made = ClusteredDecal { base_color_texture: Some(picture.clone()), normal_map_texture: None, metallic_roughness_texture: None, emissive_texture: None, tag };
        let entity = commands.spawn((made, Transform::from_matrix(matrix))).id();
        into_ring(&mut commands, laid, index, sheet, entity);
        info!("a bleeding's drop under character {character} at ({:.2}, {:.2}, {:.2}), {:.2} m across", found.point[0], found.point[1], found.point[2], 2.0 * half);
        true
    });
}
