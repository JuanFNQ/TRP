//! Running the dummies each frame: what is seen and how they are posed.

use super::*;

/// A view a dummy may be seen in (the main camera's, an optic's): its
/// newest place, its frustum, its height in pixels and its projection's
/// [1][1].
pub(super) struct SeenBy {
    pub(super) eye: Vec3,
    pub(super) frustum: Frustum,
    pub(super) pixels: f32,
    pub(super) clip_y: f32,
    pub(super) orthographic: bool,
}

/// The views as they stand now: each active camera that draws the world
/// for the eye (the main camera, the optic's), from its place as the
/// frame's transforms have it so far (the character's camera already moved
/// for the frame).
#[allow(clippy::type_complexity)]
fn views_now(cameras: &Query<(Entity, &Camera, &Projection), Or<(With<crate::MainCamera>, With<crate::weapons::optic_view::OpticCamera>)>>, helper: &TransformHelper) -> Vec<SeenBy> {
    cameras
        .iter()
        .filter(|(_, camera, _)| camera.is_active)
        .filter_map(|(entity, camera, projection)| {
            let size = camera.physical_viewport_size()?;
            let global = helper.compute_global_transform(entity).ok()?;
            let clip = projection.get_clip_from_view();
            let frustum = Frustum::from_clip_from_world(&(clip * global.to_matrix().inverse()));
            Some(SeenBy { eye: global.translation(), frustum, pixels: size.y.max(1) as f32, clip_y: clip.y_axis.y, orthographic: clip.w_axis.w == 1.0 })
        })
        .collect()
}

/// A character's largest size on screen over the views (14d's rule,
/// `trp_sim::lod::pixels_tall`, its whole thing's `size` at the middle of
/// its bound), where any sees it: its bound's ball within a view with
/// `VIEW_MARGIN` round it; or within the sun's shadow reach of a view's
/// place, its shadow may be seen (`PROVISIONAL`); or, its light lit, within
/// its light's reach of a view (`PROVISIONAL`). None where none does.
pub(super) fn size_on_screen(views: &[SeenBy], bound: &trp_sim::characters::puppet::Bound, size: f32, shadow_reach: f32, light_reach: f32) -> Option<f32> {
    let bevy = |v: [f32; 3]| Vec3::new(v[0], v[1], -v[2]);
    let (a, b) = (bevy(bound.ends[0]), bevy(bound.ends[1]));
    let centre = (a + b) / 2.0;
    let radius = a.distance(b) / 2.0 + bound.radius;
    views
        .iter()
        .filter_map(|view| {
            let distance = centre.distance(view.eye);
            let margin = VIEW_MARGIN + VIEW_MARGIN_SHARE * distance;
            let within = |reach: f32| view.frustum.intersects_sphere(&Sphere { center: centre.into(), radius: radius + margin + reach }, false);
            let seen = within(0.0) || distance <= shadow_reach || (light_reach > 0.0 && within(light_reach));
            seen.then(|| trp_sim::characters::lod::pixels_tall(size, distance, view.pixels, view.clip_y, view.orthographic))
        })
        .reduce(f32::max)
}

/// What a dummy is drawn with this frame, by its size on screen.
enum Draw {
    /// A pose: at the moment drawn, or its newest frame's while held.
    Pose(PuppetPose),
    /// Its skeleton held, its root and its weapon's frame moved to its
    /// newest frame's places.
    Move { root: Transform, player: Transform, pivot: Transform },
    Nothing,
}

/// The places a pose's figures put its root (the body's place and turn)
/// and the frame the rig in its hands hangs in (the feet turned with the
/// view, the ribcage from them turned by the view's pitch), Bevy's axes.
fn frames_of(position: [f32; 3], yaw: f32, pitch: f32, body_at: [f32; 3], body_turn: [f32; 4], ribcage: [f32; 3]) -> (Transform, Transform, Transform) {
    let game = |v: [f32; 3]| Vec3::new(v[0], v[1], -v[2]);
    let root = Transform { translation: game(body_at), rotation: convert::rotation(body_turn), scale: Vec3::ONE };
    let player = Transform { translation: game(position), rotation: Quat::from_rotation_y(-yaw.to_radians()), scale: Vec3::ONE };
    let pivot = Transform { translation: game(ribcage), rotation: Quat::from_rotation_x(-pitch.to_radians()), scale: Vec3::ONE };
    (root, player, pivot)
}

/// Each living dummy's character on: the walker's keys to its right or its
/// left by where it is, its frames run with its health followed after each
/// (as the player's: body hits spec, "The dummy on the range"), and its
/// body, the weapon in its hands and its arms drawn by its size on screen
/// (the posing economy: third-person spec, "Posing by
/// distance and off screen: the game's, and TRP's own"): at least
/// `trp_sim::lod::POSED_EVERY_FRAME_PIXELS` tall in a view, posed in full
/// and drawn at the moment drawn, between two of its frames, as the
/// player's body is (`Puppet::drawn`); smaller, posed when
/// asked and its newest frame drawn every few of its frames, held between,
/// its place moved on every one; culled whole by the levels of detail, or
/// out of every view, nothing posed for drawing and its entities left as
/// they are. Its hit colliders go by its frames alone (`Dummy::placement`).
/// With the economy off, every one in full, drawn every frame. Its sounds
/// of moving are told how far the listener (the camera) is, which past a
/// surface check plays no step (`steps::StepSounds::listener`). The frame
/// its colliders were of is kept for this frame's blasts (`Puppet::
/// keep_from`). Its posing's time is said in the log now and then.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn run_dummies(
    dummies: Option<ResMut<Dummies>>,
    rigs: Option<Res<DummyRigs>>,
    range: Res<crate::world::range::Range>,
    time: Res<Time>,
    camera: Query<&GlobalTransform, With<crate::MainCamera>>,
    cameras: Query<(Entity, &Camera, &Projection), Or<(With<crate::MainCamera>, With<crate::weapons::optic_view::OpticCamera>)>>,
    (state, options, wholes, lights): (Option<Res<crate::screens::options_view::OptionsState>>, Res<crate::Options>, Option<Res<crate::assets::lod_view::Wholes>>, Option<Res<DummyLights>>),
    (suns, spots, points): (Query<(&DirectionalLight, &bevy::light::CascadeShadowConfig), With<crate::rendering::lighting::RangeSun>>, Query<&SpotLight>, Query<&PointLight>),
    mut set: ParamSet<(TransformHelper, Query<&mut Transform>)>,
) {
    let (Some(mut dummies), Some(rigs)) = (dummies, rigs) else { return };
    let dummies = &mut *dummies;
    // The ear in the game's axes.
    let ear = camera.single().ok().map(|camera| {
        let at = camera.translation();
        [at.x, at.y, -at.z]
    });
    let economy = dummies.economy;
    let views = if economy { views_now(&cameras, &set.p0()) } else { Vec::new() };
    let tolerance = crate::assets::lod_view::tolerance(state.as_deref(), &options);
    let shadow_reach = suns.iter().filter(|(sun, _)| sun.shadows_enabled).filter_map(|(_, config)| config.bounds.last().copied()).fold(0.0f32, f32::max);
    let light_reach = if lights.is_some_and(|lights| lights.on) { spots.iter().map(|light| light.range).chain(points.iter().map(|light| light.range)).fold(0.0f32, f32::max) } else { 0.0 };
    let mut transforms = set.p1();
    let (mut stepped, mut drawing) = (0.0f64, 0.0f64);
    let mut counts = [0u32; 3];
    for dummy in &mut dummies.list {
        if !dummy.dressed || dummy.fall.is_some() || !dummy.health.alive {
            continue;
        }
        let Some(puppet) = dummy.puppet.as_mut() else { continue };
        let rig = &rigs.0[dummy.which];
        // How it is drawn, by its size on screen as its newest frame
        // stands (`size_on_screen`); posed in full where it is drawn every
        // frame, else when asked.
        let (drawn_by, tall) = match (economy, puppet.bound()) {
            (true, Some(bound)) => {
                let size = wholes.as_deref().and_then(|wholes| wholes.size_of(rig.root)).unwrap_or(CHARACTER_SIZE);
                let light = if dummy.holds.is_some() { light_reach } else { 0.0 };
                let tall = size_on_screen(&views, &bound, size, shadow_reach, light);
                (trp_sim::characters::lod::posing_drawn(tall, tolerance), tall)
            }
            _ => (PosingDrawn::EveryFrame, None),
        };
        // (Said as it changes, with `RUST_LOG=info,trp::dummy=debug`.)
        if drawn_by != dummy.drawn_by {
            debug!("dummy {} drawn {drawn_by:?}: {} on screen", dummy.which, tall.map_or("not".to_owned(), |tall| format!("{tall:.1} px tall")));
        }
        dummy.drawn_by = drawn_by;
        puppet.posing(if dummy.drawn_by == PosingDrawn::EveryFrame { Posing::Full } else { Posing::Asked });
        // The frame its colliders were of, which this frame's blasts meet
        // (`blast_on_dummies`), kept however many frames this step makes.
        puppet.keep_from(dummy.colliders_frame);
        let place = &places()[dummy.which];
        let mut keys = [0.0f32; 2];
        if place.walks {
            let x = puppet.movement.position[0];
            if x > place.at.x + WALK_SPAN {
                dummy.heading = -1.0;
            } else if x < place.at.x - WALK_SPAN {
                dummy.heading = 1.0;
            }
            // Its right along the world's x: the yaw's cosine.
            keys[0] = dummy.heading * puppet.movement.yaw.to_radians().cos().signum();
        }
        puppet.movement.sounds.listener = ear.map(|ear| {
            let at = puppet.movement.position;
            ((at[0] - ear[0]).powi(2) + (at[1] - ear[1]).powi(2) + (at[2] - ear[2]).powi(2)).sqrt()
        });
        let began = std::time::Instant::now();
        puppet.step(time.delta_secs(), keys, &*range, Some(&mut dummy.health));
        stepped += began.elapsed().as_secs_f64();
        let firearm = puppet.firearm().is_some();
        dummy.followed = true;
        // (`TRP_DUMMY_DEBUG`: its pace once a second, to see what its hits
        // and health do to it.)
        let now = time.elapsed_secs();
        if std::env::var_os("TRP_DUMMY_DEBUG").is_some() && now.floor() != (now - time.delta_secs()).floor() {
            let m = &puppet.movement;
            info!(
                "dummy {} at {:.2}: {:?}, {:.2} m/s, its speed limit {:.3} of {:.3}, legs {:.1}, the legs damaged {}{}",
                dummy.which,
                m.position[0],
                m.state,
                (m.velocity[0].powi(2) + m.velocity[2].powi(2)).sqrt(),
                m.speed_limit(),
                m.settings.max_speed,
                m.stamina.legs.current,
                [(m.conditions.left_leg, "left"), (m.conditions.right_leg, "right")].iter().filter(|(on, _)| *on).map(|(_, side)| *side).collect::<Vec<_>>().join(" and "),
                if m.conditions.on_painkillers { ", on painkillers" } else { "" }
            );
        }
        let began = std::time::Instant::now();
        let newest = puppet.newest_frame();
        let draw = match dummy.drawn_by {
            PosingDrawn::EveryFrame => {
                dummy.held_frame = None;
                puppet.drawn().map_or(Draw::Nothing, Draw::Pose)
            }
            PosingDrawn::Held(every) => {
                let due = match (dummy.held_frame, newest) {
                    (Some(held), Some(newest)) => newest < held || newest >= held + u64::from(every),
                    (None, Some(_)) => true,
                    (_, None) => false,
                };
                match (due, puppet.recipe()) {
                    (true, _) => match puppet.pose().cloned() {
                        Some(pose) => {
                            dummy.held_frame = newest;
                            Draw::Pose(pose)
                        }
                        None => Draw::Nothing,
                    },
                    (false, Some(r)) => {
                        let (root, player, pivot) = frames_of(r.position, r.yaw, r.pitch, r.body_at, r.body_turn, r.ribcage);
                        Draw::Move { root, player, pivot }
                    }
                    (false, None) => Draw::Nothing,
                }
            }
            PosingDrawn::Not => {
                dummy.held_frame = None;
                Draw::Nothing
            }
        };
        counts[match dummy.drawn_by {
            PosingDrawn::EveryFrame => 0,
            PosingDrawn::Held(_) => 1,
            PosingDrawn::Not => 2,
        }] += 1;
        match draw {
            Draw::Pose(pose) => pose_dummy(dummy, rig, &pose, firearm, &mut transforms),
            Draw::Move { root, player, pivot } => {
                if let Ok(mut transform) = transforms.get_mut(rig.root) {
                    *transform = root;
                }
                if let Some(model) = dummy.model.as_ref().filter(|_| firearm) {
                    for (entity, transform) in [(model.player, player), (model.pivot, pivot)] {
                        if let Ok(mut t) = transforms.get_mut(entity) {
                            *t = transform;
                        }
                    }
                }
            }
            Draw::Nothing => {}
        }
        drawing += began.elapsed().as_secs_f64();
    }
    // The posing's time, said every five seconds: the frames' mean.
    let log = &mut dummies.posing_log;
    log.frames += 1;
    log.stepped += stepped;
    log.drawing += drawing;
    log.every_frame += counts[0];
    log.held += counts[1];
    log.not += counts[2];
    log.since += time.delta_secs();
    if log.since >= 5.0 {
        let frames = log.frames.max(1) as f64;
        let made = PLACEMENTS_MADE.load(Ordering::Relaxed);
        info!(
            "the dummies' posing (the economy {}; the sun's shadows reach {shadow_reach:.0} m, their lights {light_reach:.0} m): {:.1} standing a frame, {:.1} drawn every frame, {:.1} held, {:.1} not drawn; their steps {:.3} ms a frame, their drawing {:.3} ms, their placing {:.3} ms, {:.2} colliders' placings a frame, over {} frames",
            if economy { "on" } else { "off" },
            (log.every_frame + log.held + log.not) as f64 / frames,
            log.every_frame as f64 / frames,
            log.held as f64 / frames,
            log.not as f64 / frames,
            log.stepped * 1000.0 / frames,
            log.drawing * 1000.0 / frames,
            log.placing * 1000.0 / frames,
            made.wrapping_sub(log.placed_before) as f64 / frames,
            log.frames
        );
        *log = PosingLog { placed_before: made, ..PosingLog::default() };
    }
}

/// A dummy's skeleton as one of its character's poses puts it (`pose`):
/// its root's place in the world, each bone in its parent (the bones its
/// body's animator gives, each arm from the upper arm to the palm where
/// its solver put it on the rig, the fingers as the rig has them, the rest
/// as last posed), and the frame the rig in its hands hangs in: the feet
/// turned with the view, the ribcage from them turned by the view's pitch.
pub(super) struct Posed {
    pub(super) root: Transform,
    pub(super) locals: Vec<Transform>,
    pub(super) player: Transform,
    pub(super) pivot: Transform,
}

/// The skeleton as `pose` puts it (`Posed`), its hands holding a `firearm`
/// (the AKM's rig, else the empty hands'): for its hit colliders (`hits`)
/// the arms solved on the animator's straight pose, so that `SMOOTH
/// ANIMATIONS` moves no hit, else as drawn.
pub(super) fn posed(dummy: &Dummy, pose: &PuppetPose, firearm: bool, hits: bool) -> Posed {
    let (root, player, pivot) = frames_of(pose.position, pose.yaw, pose.pitch, pose.body_at, pose.body_turn, pose.ribcage);
    let mut locals = dummy.locals.clone();
    for (k, bone) in dummy.drawn.iter().enumerate() {
        let (Some(bone), Some((at, turn))) = (bone, pose.bones.get(k)) else { continue };
        locals[*bone] = convert::transform(*at, *turn, [1.0; 3]);
    }
    let grip = if firearm { dummy.model.as_ref().map(|model| &model.grip) } else { dummy.empty_grip.as_ref() };
    if let Some(grip) = grip {
        let rig_world = player.compute_affine() * pivot.compute_affine() * grip.root_local.compute_affine();
        // The arms onto the rig: each bone where the solver put it, in its
        // parent on the skeleton.
        let world = world_of(&dummy.parents, &locals, &[], None, Affine3A::IDENTITY);
        let root_world = root.compute_affine();
        let arms = if hits { &pose.straight_arms } else { &pose.arms };
        for (side, bones) in dummy.arm_bones.iter().enumerate() {
            let [Some(collar), Some(upper), Some(f1), Some(f2), Some(f3), Some(palm)] = *bones else { continue };
            let placed: [Affine3A; 5] = arms[side].map(|(at, turn)| root_world.inverse() * rig_world * Affine3A::from_rotation_translation(convert::rotation(turn), convert::position(at)));
            let chain = [collar, upper, f1, f2, f3, palm];
            for k in 0..5 {
                let parent = if k == 0 { world[chain[0]] } else { placed[k - 1] };
                locals[chain[k + 1]] = Transform::from_matrix(Mat4::from(parent.inverse() * placed[k]));
            }
        }
        for (from, bone) in &grip.fingers {
            if let Some((at, turn)) = pose.rig.get(*from) {
                locals[*bone] = convert::transform(*at, *turn, [1.0; 3]);
            }
        }
    }
    Posed { root, locals, player, pivot }
}

/// A dummy drawn in `pose` (`posed`): its body's root and every bone; the
/// weapon's frame and every bone of its rig (the AKM's, where its hands
/// hold a `firearm`; the empty hands' rig is not drawn); its outfits.
pub(super) fn pose_dummy(dummy: &mut Dummy, rig: &DummyRig, pose: &PuppetPose, firearm: bool, transforms: &mut Query<&mut Transform>) {
    let Posed { root, locals, player, pivot } = posed(dummy, pose, firearm, false);
    if let Ok(mut transform) = transforms.get_mut(rig.root) {
        *transform = root;
    }
    // The weapon's frame and its rig: the AKM's drawn, the empty hands'
    // only placed.
    if let Some(model) = dummy.model.as_ref().filter(|_| firearm) {
        for (entity, transform) in [(model.player, player), (model.pivot, pivot)] {
            if let Ok(mut t) = transforms.get_mut(entity) {
                *t = transform;
            }
        }
        for (entity, (at, turn)) in model.tree.iter().zip(&pose.rig).skip(1) {
            if let Ok(mut t) = transforms.get_mut(*entity) {
                t.translation = convert::position(*at);
                t.rotation = convert::rotation(*turn);
            }
        }
    }
    dummy.locals = locals;
    for (bone, local) in rig.bones.iter().zip(&dummy.locals) {
        if let Ok(mut t) = transforms.get_mut(*bone) {
            *t = *local;
        }
    }
    let world = world_of(&rig.parents, &dummy.locals, &[], None, Affine3A::IDENTITY);
    pose_outfits(rig, &world, transforms);
}

/// A bone's place in the world (Bevy's axes) with its root at `root` and
/// each bone at `locals` in its parent, put together as the engine puts
/// the bones' own entities together (`TransformHelper::
/// compute_global_transform`: from the bone up, each parent's on the left,
/// the root's last), so that a collider placed by it is where the same
/// pose drawn would place it, to the last bit.
pub(super) fn in_world(parents: &[Option<usize>], root: &Transform, locals: &[Transform], bone: usize) -> Affine3A {
    let mut at = locals[bone].compute_affine();
    let mut parent = parents[bone];
    while let Some(p) = parent {
        at = locals[p].compute_affine() * at;
        parent = parents[p];
    }
    root.compute_affine() * at
}

/// Each bone of a dummy in its root's axes: where its body is in the
/// engine, else its place under its parent.
pub(super) fn world_of(parents: &[Option<usize>], locals: &[Transform], moved: &[Option<Body>], engine: Option<&World>, root: Affine3A) -> Vec<Affine3A> {
    let into_root = root.inverse();
    let mut world: Vec<Affine3A> = Vec::with_capacity(locals.len());
    for (index, local) in locals.iter().enumerate() {
        let parent = parents[index].map_or(Affine3A::IDENTITY, |p| world[p]);
        let here = match (moved.get(index).copied().flatten(), engine) {
            (Some(body), Some(engine)) => {
                let (at, turn) = engine.pose(body);
                into_root * Affine3A::from_rotation_translation(convert::rotation(turn), convert::position(at))
            }
            _ => parent * local.compute_affine(),
        };
        world.push(here);
    }
    world
}

/// Its outfits' skeletons on its bones as they stand (`death::follow`'s).
pub(super) fn pose_outfits(rig: &DummyRig, world: &[Affine3A], transforms: &mut Query<&mut Transform>) {
    use trp_data::character::outfit::Move;
    for outfit in &rig.outfits {
        let theirs: Vec<Move> = outfit
            .theirs
            .iter()
            .zip(&outfit.rest)
            .map(|(bone, rest)| {
                let moved = bone.map_or(Affine3A::IDENTITY, |bone| world[bone]) * *rest;
                Move { axes: [moved.matrix3.x_axis.to_array(), moved.matrix3.y_axis.to_array(), moved.matrix3.z_axis.to_array()], shift: moved.translation.to_array() }
            })
            .collect();
        for (entity, moved) in outfit.ours.iter().zip(outfit.rig.pose(&theirs)) {
            let turn = Quat::from_mat3(&Mat3::from_cols(moved.axes[0].into(), moved.axes[1].into(), moved.axes[2].into())).normalize();
            if let Ok(mut transform) = transforms.get_mut(*entity) {
                *transform = Transform { translation: moved.shift.into(), rotation: turn, scale: Vec3::ONE };
            }
        }
    }
}
