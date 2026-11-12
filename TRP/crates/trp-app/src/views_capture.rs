//! Switching views, the free camera and the screenshot capture.

use super::*;

/// Applies the starting view, and switches views by the debug panel's
/// buttons (no longer `F4` and `F5`, which the gestures' binds took):
/// FREE CAMERA between playing and the free camera, WATCHED between playing
/// and playing watched from outside (from where the free camera stands, if
/// it was flying; else from three metres off).
pub(super) fn switch_view(
    mut commands: Commands,
    mut asked: MessageReader<crate::platform::debug_menu::DebugCommand>,
    options: Res<Options>,
    rig: Option<Res<WeaponRig>>,
    controller: Res<Controller>,
    globals: Query<&GlobalTransform>,
    mut view: ResMut<View>,
    mut watching: ResMut<Watching>,
    mut camera: Query<(Entity, &mut Transform), With<MainCamera>>,
    mut frame: Local<u32>,
) {
    *frame += 1;
    // Global transforms of the new entities exist from the second frame.
    let starting = *frame == 2;
    let asked: Vec<crate::platform::debug_menu::DebugCommand> = asked.read().copied().collect();
    let free = asked.contains(&crate::platform::debug_menu::DebugCommand::FreeCamera);
    let watched = asked.contains(&crate::platform::debug_menu::DebugCommand::Watched);
    let wanted = if starting {
        *view
    } else if free {
        match *view {
            View::FirstPerson | View::Watched => View::Free,
            View::Free => View::FirstPerson,
        }
    } else if watched {
        match *view {
            View::FirstPerson | View::Free => View::Watched,
            View::Watched => View::FirstPerson,
        }
    } else {
        return;
    };
    let (Some(rig), Ok((camera, mut transform))) = (rig, camera.single_mut()) else { return };
    // A camera already outside stays where it is when the keys change
    // hands; one that leaves the character's eyes is put beside it.
    let leaving_the_eyes = starting || *view == View::FirstPerson;
    match wanted {
        View::FirstPerson => {
            // The camera hangs from the rig's root; its place and rotation
            // there are set every frame with the weapon motion.
            commands.entity(rig.pivot).add_child(camera);
            *transform = Transform::default();
        }
        View::Free | View::Watched => {
            if leaving_the_eyes {
                let root = globals.get(rig.root).map_or(GlobalTransform::IDENTITY, |g| *g);
                commands.entity(camera).remove::<ChildOf>();
                *transform = outside_view(&root, &options, options.whole || (wanted == View::Watched && !starting));
            }
        }
    }
    watching.0 = (wanted == View::Watched).then(|| transform.translation - controller.shown().position);
    // Whether playing has the mouse goes by the view (`cursor::hold`).
    *view = wanted;
}

/// Where the camera stands for a view from outside, given the weapon
/// rig's root (at the ribcage, turned to the view): beside a held rifle,
/// measured from the root so that animation does not move the viewpoint;
/// for the whole figure, three metres off to the front and left of where
/// the player faces, level, looking at the middle of the body; or, for a
/// plan of the range, high over the character looking straight down, with
/// the lane's way (ahead from the start) at the top.
fn outside_view(root: &GlobalTransform, options: &Options, whole: bool) -> Transform {
    if let Some(height) = options.map {
        let under = Vec3::new(root.translation().x, 0.0, root.translation().z - 2.0);
        return Transform::from_translation(under + Vec3::Y * height).looking_at(under, Vec3::NEG_Z);
    }
    if !whole {
        let (target, from) = (Vec3::new(0.13, -0.12, -0.42), Vec3::new(-0.62, 0.03, -0.67));
        return Transform::from_translation(root.transform_point(from)).looking_at(root.transform_point(target), Vec3::Y);
    }
    let ahead = root.forward();
    let facing = Quat::from_rotation_y(f32::atan2(-ahead.x, -ahead.z));
    // The middle of the body is about half the ribcage's height up, and
    // the camera is kept over the ground for a body that lies.
    let ribcage = root.translation();
    let middle = Vec3::new(ribcage.x, ribcage.y * 0.52, ribcage.z);
    let round = Quat::from_rotation_y(options.watch_turn.to_radians());
    Transform::from_translation(middle + facing * (round * Vec3::new(-2.4, 0.35, -1.6))).looking_at(middle, Vec3::Y)
}

pub(super) fn fly_camera(
    time: Res<Time>,
    view: Res<View>,
    options: Res<Options>,
    rig: Option<Res<WeaponRig>>,
    globals: Query<&GlobalTransform>,
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    controller: Res<Controller>,
    watching: Res<Watching>,
    mut camera: Query<&mut Transform, With<MainCamera>>,
) {
    let Ok(mut transform) = camera.single_mut() else { return };
    if *view == View::FirstPerson {
        return;
    }
    // An unattended capture from outside keeps its place beside the rig,
    // so that a player who has walked or crawled away is still in it.
    if let (true, Some(rig)) = (options.unattended(), &rig) {
        if let Ok(root) = globals.get(rig.root) {
            *transform = outside_view(root, &options, options.whole);
        }
        return;
    }
    // A played run watched from outside: the camera keeps its place from
    // the character's feet, turned as it was left.
    if *view == View::Watched {
        if let Some(from_feet) = watching.0 {
            transform.translation = controller.shown().position + from_feet;
        }
        return;
    }
    if !buttons.pressed(MouseButton::Right) {
        return;
    }
    let (mut yaw, mut pitch, _) = transform.rotation.to_euler(EulerRot::YXZ);
    yaw -= motion.delta.x * 0.003;
    pitch = (pitch - motion.delta.y * 0.003).clamp(-1.54, 1.54);
    transform.rotation = Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.0);

    let axis = |pos: KeyCode, neg: KeyCode| keys.pressed(pos) as i32 as f32 - keys.pressed(neg) as i32 as f32;
    let step = transform.right() * axis(KeyCode::KeyD, KeyCode::KeyA)
        + transform.up() * axis(KeyCode::KeyE, KeyCode::KeyQ)
        + transform.forward() * axis(KeyCode::KeyW, KeyCode::KeyS);
    let speed = if keys.pressed(KeyCode::ShiftLeft) { 4.0 } else { 0.6 };
    transform.translation += step * speed * time.delta_secs();
}

/// The region `crop` (`[x, y, w, h]`, in the window's pixels) of a captured frame, written as a
/// PNG. The frame comes as tightly packed 8-bit pixels, RGBA or BGRA by the window's surface.
fn save_cropped(image: &Image, crop: [u32; 4], path: &std::path::Path) -> anyhow::Result<()> {
    use bevy::render::render_resource::TextureFormat;
    let (width, height) = (image.width(), image.height());
    let data = image.data.as_ref().ok_or_else(|| anyhow::anyhow!("the captured frame has no data"))?;
    let format = image.texture_descriptor.format;
    let bgra = match format {
        TextureFormat::Rgba8Unorm | TextureFormat::Rgba8UnormSrgb => false,
        TextureFormat::Bgra8Unorm | TextureFormat::Bgra8UnormSrgb => true,
        other => anyhow::bail!("a {other:?} frame: not four bytes a pixel"),
    };
    anyhow::ensure!(data.len() == (width * height * 4) as usize, "a {width}x{height} {format:?} frame of {} bytes", data.len());
    let [x, y, w, h] = crop;
    let (x, y) = (x.min(width), y.min(height));
    let (w, h) = (w.min(width - x), h.min(height - y));
    anyhow::ensure!(w > 0 && h > 0, "the crop {crop:?} lies outside the {width}x{height} frame");
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for row in y..y + h {
        let start = ((row * width + x) * 4) as usize;
        for pixel in data[start..start + (w * 4) as usize].chunks_exact(4) {
            if bgra {
                rgba.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
            } else {
                rgba.extend_from_slice(pixel);
            }
        }
    }
    trp_data::interface::icon::save_png(path, w, h, &rgba)
}

pub(super) fn capture(
    mut commands: Commands,
    options: Res<Options>,
    controller: Res<Controller>,
    mut frame: Local<u32>,
    mut written: Local<Option<u32>>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(path) = &options.screenshot else {
        // A run that plays a script and logs how it was drawn ends with
        // the script.
        if let (Some(scenario), true) = (&options.scenario, options.unattended()) {
            if controller.drawn_time() > options.scenario_at + scenario.duration + 0.25 {
                exit.write(AppExit::Success);
            }
        }
        return;
    };
    *frame += 1;
    // With `--screenshot-frames <n>`, a picture of each of n frames from
    // that one on, each named with its frame (`<name>_<frame>.png`); the
    // run ends once the last is written.
    let count = options.screenshot_count.max(1);
    let path_of = |at: u32| -> PathBuf {
        if count == 1 {
            return path.clone();
        }
        let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        path.with_file_name(format!("{stem}_{at}.png"))
    };
    let last = options.screenshot_frame + count - 1;
    // The default leaves time for textures to reach the GPU.
    if (options.screenshot_frame..=last).contains(&*frame) {
        let path = &path_of(*frame);
        // Whatever an earlier run left under this name goes first, so
        // that the file being there means this run's picture is written.
        let _ = std::fs::remove_file(path);
        match options.screenshot_crop {
            None => {
                commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path.clone()));
            }
            Some(crop) => {
                let path = path.clone();
                commands.spawn(Screenshot::primary_window()).observe(move |trigger: On<ScreenshotCaptured>| {
                    if let Err(e) = save_cropped(&trigger.event().image, crop, &path) {
                        warn!("the cropped picture is not written: {e:#}");
                    }
                });
            }
        }
    }
    // The picture is written some frames after it is asked for, and a
    // good many more while the renderer is still building what it draws
    // with: the run ends once the file is there.
    if *frame > last {
        let path = &path_of(last);
        match (path.exists(), *written) {
            (true, None) => *written = Some(*frame),
            (true, Some(at)) if *frame > at + 5 => {
                exit.write(AppExit::Success);
            }
            (false, _) if *frame > options.screenshot_frame + 1800 => {
                warn!("the picture was not written: {}", path.display());
                exit.write(AppExit::Success);
            }
            _ => {}
        }
    }
}
