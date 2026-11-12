//! The warm-up before the first frame, the pacing log and the frames' log.

use super::*;

/// Whether the run is still warming up.
///
/// The engine builds what it draws a kind of thing with (a pipeline: its
/// shaders, for one layout of mesh and one sort of material) the first
/// time that kind of thing is in the picture. It builds in the background,
/// several at once, and leaves the thing out of the picture until its
/// pipeline is ready, which on DirectX is a second or more: the range, the
/// body and the weapon were missing from the first second and a half of
/// every run, and a weapon first drawn later (a character that begins with
/// empty hands) was missing again while its own were built.
///
/// So the run begins with what it will come to draw in the picture at
/// once, behind a curtain: the weapon though the character may begin
/// without it, and a thing as it lies on the ground. When the renderer
/// says it has nothing left to build, the curtain goes and the run begins.
/// Measured on the machine this was written on: 1.7 s with DirectX 12 and
/// 0.4 s with Vulkan (`--backend vulkan`), which is how long the range and
/// the weapon used to be missing for; nothing is built that would not have
/// been, and nothing of it is left once it is over. Not shown beforehand,
/// and so still a moment late the first time they are seen: the upper body
/// from outside (the free camera; a fifth of the kind and a second more on
/// DirectX), and whatever is first worn in a run that began bare.
#[derive(Resource)]
pub struct Warmup(pub bool);

/// Whether the run has begun: the warm-up is over. The simulation, the
/// keys, the scripted presses and drags of an unattended run and the
/// frames a capture counts all wait for it, so that a run's first frame
/// is the first one that shows it whole.
/// The mixed sound of the run written out as it ends (`--sound-out`).
pub(super) fn write_tape(mut exit: MessageReader<AppExit>, options: Res<Options>, mixer: Res<audio::ui_sound::Mixer>) {
    if exit.read().next().is_none() {
        return;
    }
    let (Some(path), Some(wav)) = (&options.sound_out, mixer.0.tape_wav()) else { return };
    match std::fs::write(path, wav) {
        Ok(()) => info!("the run's sound written to {}", path.display()),
        Err(e) => warn!("the run's sound could not be written to {}: {e}", path.display()),
    }
}

pub fn running(warmup: Res<Warmup>) -> bool {
    !warmup.0
}

/// How many pipelines the renderer still has to build, as it last said.
/// The renderer's own world writes it at the end of each of its frames.
#[derive(Resource, Clone)]
pub(crate) struct PipelinesWaiting(pub(crate) std::sync::Arc<std::sync::atomic::AtomicUsize>);

/// The frames a warm-up lasts at the least (what reaches the renderer does
/// so a frame or two after it is made), the frames in a row the renderer
/// must have had nothing to build, and the seconds after which the run
/// begins whatever the renderer says.
const WARMUP_FRAMES: u32 = 4;

/// With `--fixed-dt`, the warm-up's frames at the least: the run begins
/// on the same frame each time (the renderer ready within 15 to 41 frames
/// in the runs measured), so that what moves under the curtain (the
/// dummies) has moved as far.
const FIXED_DT_WARMUP_FRAMES: u32 = 90;

const WARMUP_CALM: u32 = 8;

const WARMUP_LIMIT: f32 = 30.0;

/// What is in the scene only while the run warms up: the curtain, and
/// things shown for the building's sake.
#[derive(Component)]
pub(super) struct WarmupProp;

/// In the renderer's world: how many pipelines are still to be built.
pub(super) fn count_waiting(cache: Res<bevy::render::render_resource::PipelineCache>, waiting: Res<PipelinesWaiting>, mut frame: Local<u32>) {
    waiting.0.store(cache.waiting_pipelines().count(), std::sync::atomic::Ordering::Relaxed);
    *frame += 1;
    // `TRP_ICON_TRACE`: each pipeline still waiting, every 60 of the
    // renderer's frames, by its label and how far it has got (a new kind of
    // material's took 1 to 2.5 s on DirectX 12).
    if std::env::var_os("TRP_ICON_TRACE").is_some() && *frame % 60 == 0 {
        use bevy::render::render_resource::{CachedPipelineState, PipelineDescriptor};
        let pipelines: Vec<&bevy::render::render_resource::CachedPipeline> = cache.pipelines().collect();
        for id in cache.waiting_pipelines() {
            let Some(pipeline) = pipelines.get(id) else { continue };
            let state = match &pipeline.state {
                CachedPipelineState::Queued => "queued".to_owned(),
                CachedPipelineState::Creating(_) => "creating".to_owned(),
                CachedPipelineState::Ok(_) => "ok".to_owned(),
                CachedPipelineState::Err(e) => format!("error {e}"),
            };
            let (label, defs) = match &pipeline.descriptor {
                PipelineDescriptor::RenderPipelineDescriptor(d) => (d.label.clone(), d.fragment.as_ref().map_or(0, |f| f.shader_defs.len())),
                PipelineDescriptor::ComputePipelineDescriptor(d) => (d.label.clone(), d.shader_defs.len()),
            };
            info!("pipeline trace {}: {id} {label:?} {state}, {defs} defs", *frame);
        }
    }
}

/// Ends the warm-up when the renderer has caught up, and clears its
/// things away.
pub(super) fn warm_up(
    mut commands: Commands,
    mut warmup: ResMut<Warmup>,
    waiting: Res<PipelinesWaiting>,
    time: Res<Time<Real>>,
    props: Query<Entity, With<WarmupProp>>,
    mut budget: ResMut<bevy::render::render_asset::RenderAssetBytesPerFrame>,
    preparing: Option<Res<weapons::weapon::Preparing>>,
    mut frames: Local<[u32; 2]>,
    options: Res<Options>,
    screens: Option<Res<screens::health_view::ScreensComing>>,
    effects: Option<Res<assets::effects_reads::EffectsComing>>,
    sounds: Option<Res<weapons::weapon::SoundsComing>>,
) {
    if !warmup.0 {
        return;
    }
    let [seen, calm] = &mut *frames;
    *seen += 1;
    // A rig asked for at the start is made ready on another thread; the
    // curtain waits for it, so that what it brings to the graphics card
    // goes up under it (the StG 44's uncompressed pictures had stood a
    // frame of play still for 100 ms). The inventory's pictures are not
    // waited for: as in the game they are asked for when the screen first
    // shows an item (`icons::ask_icons`; icons spec, "When a picture is
    // asked for").
    let preparing = preparing.is_some_and(|preparing| preparing.busy());
    let least = if options.fixed_dt.is_some() { FIXED_DT_WARMUP_FRAMES } else { WARMUP_FRAMES };
    // And the inventory's screens, read on their thread (`ScreensComing`):
    // whole from the run's first frame, their pictures up under the curtain.
    // And the effects (`EffectsComing`): the first shot or blast drawn
    // whole, their pictures up under the curtain.
    let building = waiting.0.load(std::sync::atomic::Ordering::Relaxed) > 0 || *seen < least || preparing || screens.is_some() || effects.is_some() || sounds.is_some();
    *calm = if building { 0 } else { *calm + 1 };
    if *calm >= WARMUP_CALM || time.elapsed_secs() > WARMUP_LIMIT {
        warmup.0 = false;
        info!("the renderer had built what it draws with {:.2} s into the run ({} frames)", time.elapsed_secs(), *seen);
        for prop in &props {
            commands.entity(prop).despawn();
        }
        // From here on what goes up to the graphics card is spread over
        // frames, a picture or so a frame: a rig made ready away from the
        // frame (`weapon::Preparing`) brings tens of megabytes of pictures
        // at once, which in one frame stood it still for 137 ms (the
        // StG 44's, uncompressed). Under the curtain, all at once.
        budget.max_bytes = Some(UPLOAD_BUDGET);
    }
}

/// What goes up to the graphics card a frame after the curtain, at most
/// (a whole picture or mesh at least).
const UPLOAD_BUDGET: usize = 8 << 20;

/// When this frame's systems began, for `--log-pacing`.
#[derive(Resource)]
pub(super) struct FrameMark(pub(super) std::time::Instant);

pub(super) fn mark_frame(mut mark: ResMut<FrameMark>) {
    mark.0 = std::time::Instant::now();
}

/// Writes one line for the frame about to be drawn, when asked to: how long
/// the frame took by the engine's clock and by a clock read here, what the
/// simulation ran in it and what that cost, and where the camera is in the
/// world. Whether the app draws evenly is read off these.
pub(super) fn log_pacing(
    options: Res<Options>,
    real: Res<Time<Real>>,
    mark: Res<FrameMark>,
    controller: Res<Controller>,
    player: Query<&Transform, With<weapons::weapon::Player>>,
    pivot: Query<&Transform, With<weapons::weapon::ViewPivot>>,
    camera: Query<&Transform, With<MainCamera>>,
    body: Option<Res<player::body_view::BodyRig>>,
    parts: Res<player::body_view::BodyParts>,
    helper: TransformHelper,
    mut window: Query<&mut Window, With<PrimaryWindow>>,
    mut out: Local<Option<std::io::BufWriter<std::fs::File>>>,
    mut last: Local<Option<std::time::Instant>>,
    mut frame: Local<u32>,
) {
    use std::io::Write as _;
    let Some(path) = &options.log_pacing else { return };
    let (Ok(player), Ok(pivot), Ok(camera)) = (player.single(), pivot.single(), camera.single()) else { return };
    *frame += 1;
    // An unattended run is there to show what a played window does, and a
    // window is presented differently without the focus: it asks for it.
    let mut focused = false;
    if let Ok(mut window) = window.single_mut() {
        focused = window.focused;
        if options.unattended() && !focused && *frame % 60 == 30 {
            window.focused = true;
        }
    }
    if out.is_none() {
        match std::fs::File::create(path) {
            Ok(file) => {
                let mut file = std::io::BufWriter::new(file);
                let _ = writeln!(
                    file,
                    "frame t dt_ms wall_ms pacer_ms sim_frames physics_steps sim_ms update_ms blend cam_x cam_y cam_z yaw pitch state focused feet_x feet_y feet_z body_yaw lfoot_x lfoot_y lfoot_z rfoot_x rfoot_y rfoot_z pelvis_x pelvis_y pelvis_z rib_x rib_y rib_z script_t ltoe_x ltoe_y ltoe_z rtoe_x rtoe_y rtoe_z anim anim_next"
                );
                *out = Some(file);
            }
            Err(e) => {
                warn!("{}: {e}", path.display());
                return;
            }
        }
    }
    let now = std::time::Instant::now();
    let wall = last.replace(now).map_or(0.0, |before| (now - before).as_secs_f64() * 1000.0);
    // The camera hangs from the view pivot, and that from the player.
    let eye = player.transform_point(pivot.transform_point(camera.translation));
    let (yaw, pitch) = controller.look();
    // The point the view hangs at, in the world.
    let ribcage = player.transform_point(pivot.translation);
    let paced = controller.paced;
    // The drawn body's feet and hips, in the world, where a body is drawn.
    let drawn = |end: &str| {
        body.as_ref()
            .and_then(|rig| rig.bone(&parts, end))
            .and_then(|bone| helper.compute_global_transform(bone).ok())
            .map_or(Vec3::ZERO, |at| at.translation())
    };
    let (left, right, pelvis) = (drawn("/Base HumanLFoot"), drawn("/Base HumanRFoot"), drawn("/Base HumanPelvis"));
    let (left_toe, right_toe) = (drawn("/Base HumanLToe"), drawn("/Base HumanRToe"));
    // The moment drawn, by the clock of the script being played if one is.
    let script_time = controller.drawn_time() - options.scenario.as_ref().map_or(0.0, |_| options.scenario_at);
    // The state the body's animator is in and the one it is entering, by
    // their own names. These are the simulation's newest, a frame or two
    // ahead of what is drawn.
    let own = |name: &str| name.rsplit('.').next().unwrap_or(name).replace(' ', "_");
    let (anim, anim_next) = controller.movement.body.state_names();
    let (anim, anim_next) = (own(anim), anim_next.map_or(String::from("-"), own));
    let Some(out) = out.as_mut() else { return };
    let _ = writeln!(
        out,
        "{} {:.5} {:.4} {:.4} {:.4} {} {} {:.4} {:.4} {:.4} {:.5} {:.5} {:.5} {:.4} {:.4} {:?} {} {:.5} {:.5} {:.5} {:.4} {:.5} {:.5} {:.5} {:.5} {:.5} {:.5} {:.5} {:.5} {:.5} {:.5} {:.5} {:.5} {:.5} {:.5} {:.5} {:.5} {:.5} {:.5} {:.5} {anim} {anim_next}",
        *frame,
        real.elapsed_secs_f64(),
        real.delta_secs_f64() * 1000.0,
        wall,
        paced.host_dt * 1000.0,
        paced.frames,
        paced.physics_steps,
        paced.cost * 1000.0,
        (now - mark.0).as_secs_f64() * 1000.0,
        paced.blend,
        eye.x,
        eye.y,
        eye.z,
        yaw,
        pitch,
        controller.movement.state,
        focused as u8,
        player.translation.x,
        player.translation.y,
        player.translation.z,
        controller.movement.drawn_body_yaw(),
        left.x,
        left.y,
        left.z,
        right.x,
        right.y,
        right.z,
        pelvis.x,
        pelvis.y,
        pelvis.z,
        ribcage.x,
        ribcage.y,
        ribcage.z,
        script_time,
        left_toe.x,
        left_toe.y,
        left_toe.z,
        right_toe.x,
        right_toe.y,
        right_toe.z,
    );
}

/// Writes one line for the frame about to be drawn, when asked to. Places
/// are in the camera's own axes (x right, y up, -z ahead), in metres.
pub(super) fn log_frame(
    options: Res<Options>,
    time: Res<Time>,
    player: Res<ClipPlayer>,
    rig: Option<Res<WeaponRig>>,
    held: Option<Res<weapons::weapon::HandsRig>>,
    controller: Res<Controller>,
    body: Option<Res<player::body_view::BodyRig>>,
    parents: Query<&ChildOf>,
    transforms: Query<&Transform>,
    camera: Query<(Entity, &Transform), With<MainCamera>>,
    mut out: Local<Option<std::io::BufWriter<std::fs::File>>>,
    mut frame: Local<u32>,
) {
    use std::io::Write as _;
    let (Some(path), Some(rig), Ok((camera_entity, camera))) = (&options.log_frames, rig, camera.single()) else { return };
    *frame += 1;
    if out.is_none() {
        match std::fs::File::create(path) {
            Ok(file) => *out = Some(std::io::BufWriter::new(file)),
            Err(e) => {
                warn!("{}: {e}", path.display());
                return;
            }
        }
    }
    // The rig and the camera both hang from the view pivot.
    let seen = |entity: Entity| {
        player::hands_view::in_root(entity, rig.pivot, |e| transforms.get(e).ok().copied(), &parents)
            .map(|t| camera.rotation.inverse() * (t.translation - camera.translation))
    };
    let show = |v: Option<Vec3>| v.map_or("-".to_owned(), |v| format!("{:.4} {:.4} {:.4}", v.x, v.y, v.z));
    // In the world: up the whole hierarchy from the thing.
    let world = |entity: Entity| -> Option<Transform> {
        let mut result = Transform::IDENTITY;
        let mut current = entity;
        loop {
            result = transforms.get(current).ok()?.mul_transform(result);
            match parents.get(current) {
                Ok(parent) => current = parent.parent(),
                Err(_) => return Some(result),
            }
        }
    };
    let world_camera = world(camera_entity);
    let world_weapon = rig.weapon_root_anim.and_then(world).map(|t| t.translation);
    // Each arm's upper arm to elbow, elbow to palm and upper arm to palm,
    // and the weapon from the view pivot (the ribcage), in the world.
    let at = |e: Entity| world(e).map(|t| t.translation);
    let arm_lengths: Vec<String> = rig
        .arms
        .iter()
        .map(|arm| match (at(arm.upper()), at(arm.elbow()), at(arm.palm())) {
            (Some(u), Some(e), Some(p)) => format!("{:.4} {:.4} {:.4}", u.distance(e), e.distance(p), u.distance(p)),
            _ => "- - -".to_owned(),
        })
        .collect();
    let apart = at(rig.pivot).zip(world_weapon).map_or(-1.0, |(p, w)| p.distance(w));
    // Each collarbone the meshes hang on to the nearest upper arm.
    let collars: Vec<String> = body
        .as_deref()
        .map(|body| {
            body.kept_collarbones()
                .filter_map(at)
                .map(|c| rig.arms.iter().filter_map(|arm| at(arm.upper())).map(|u| c.distance(u)).fold(f32::MAX, f32::min))
                .map(|d| format!("{d:.4}"))
                .collect()
        })
        .unwrap_or_default();
    let weapon = rig.weapon_root_anim.and_then(seen);
    let hands: Vec<String> = rig.arms.iter().map(|arm| show(seen(arm.palm()))).collect();
    // The clip on screen, where clips are played by name; in play, the
    // state the rig's animator is in or on its way into, how far through
    // the state it is in, and whether it is on a way between two.
    let state = held.as_deref().and_then(|h| h.animator()).map(|animator| animator.state());
    let (clip, at, blend) = match (player.by_name, &state) {
        (false, Some((state, at, next))) => (next.unwrap_or(state).trim_start_matches("Hands.").replace(' ', "_"), *at, next.is_some() as u8 as f32),
        _ => {
            let (name, at) = player.status().map_or(("-", 0.0), |(name, at, _)| (name, at));
            (name.to_owned(), at, player.blend())
        }
    };
    // The state the body's layer of gestures is in or entering, how far
    // through its state it is, and how much of the left hand is the
    // body's just now.
    let gesture = controller.movement.body.interaction_state().map_or(String::from("- 0 0"), |(state, at, next)| {
        let name = next.unwrap_or(state).trim_start_matches("InteractionsLayer.").replace(' ', "_");
        format!("{name} {at:.3} {:.3}", controller.movement.arm_pose(false).hand)
    });
    // The arms' stamina, whether the quick draw's hand shake runs, and the
    // hands' rotation spring (the recorder's `hands_stamina`, the mask's
    // `HandShake` bit, `hands_rot`).
    let motion = held.as_deref().map_or(String::from("- -"), |h| {
        let rot = h.hands.hands_rotation.current;
        format!("{} {:.4} {:.4} {:.4}", h.hands.hand_shake_on() as u8, rot[0], rot[1], rot[2])
    });
    let arms_stamina = controller.movement.stamina.arms.current;
    let Some(out) = out.as_mut() else { return };
    let _ = writeln!(
        out,
        "{} t {:.4} clip {clip} {at:.3} blend {blend:.2} weapon {} hands {} camera {:.4} {:.4} {:.4} {:.4} gesture {gesture} world_weapon {} world_camera {} {} arms {} apart {apart:.4} collar_to_upper {} stamina_arms {arms_stamina:.2} shake {motion}",
        *frame,
        time.elapsed_secs(),
        show(weapon),
        hands.join(" | "),
        camera.rotation.x,
        camera.rotation.y,
        camera.rotation.z,
        camera.rotation.w,
        show(world_weapon),
        show(world_camera.map(|t| t.translation)),
        world_camera.map_or("-".to_owned(), |t| {
            let f = t.rotation * Vec3::NEG_Z;
            format!("{:.4} {:.4} {:.4}", f.x, f.y, f.z)
        }),
        arm_lengths.join(" | "),
        collars.join(" "),
    );
    let _ = out.flush();
}
