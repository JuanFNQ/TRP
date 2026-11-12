//! The figure's camera, lights, picks and drawing.

use super::*;

/// Where the OVERALL tab's view puts the model, in the view's own axes:
/// 1 m below it and 5 m on (`PlayerModelView.Show`, its `position` unless
/// told).
const MODEL_AT: [f32; 3] = [0.0, -1.0, 5.0];

/// The model turned about the vertical by so many degrees, the engine's
/// (`Quaternion.Euler(0, yaw, 0)`), in the engine's axes.
pub(super) fn turned(yaw: f32) -> Quat {
    let (sin, cos) = (yaw.to_radians() / 2.0).sin_cos();
    convert::rotation([0.0, sin, 0.0, cos])
}

/// The OVERALL tab's camera where the scene has it, the model standing as
/// it is put in (`MODEL_AT`, `TURN`): the scene's place and turn in the
/// view's axes taken into the model's, and those into the figure's place.
fn overall_camera(camera: &trp_data::interface::screen::ModelCamera) -> Transform {
    in_view(camera.place, camera.turn)
}

/// A place and turn in the OVERALL tab's view's own axes (its camera's,
/// its lights') where the figure stands as it is put in, in the world.
pub(super) fn in_view(place: [f32; 3], turn: [f32; 4]) -> Transform {
    let model = Quat::from_array(TURN);
    let from_model = Vec3::from_array(place) - Vec3::from_array(MODEL_AT);
    let place = model.inverse() * from_model;
    let turn = model.inverse() * Quat::from_array(turn);
    let root = convert::rotation(TURN);
    Transform { translation: PLACE + root * convert::position(place.to_array()), rotation: root * convert::rotation(turn.to_array()), scale: Vec3::ONE }
}

/// The OVERALL tab's eight lights (`OverallPanel::lights`), each where the
/// view has it, on the OVERALL tab's lights' layer, made by the one maker
/// of a screen's lights (`edit_build::preview_light`): in the game's units
/// for the game's light rule where it is installed (`shading`), which its
/// camera then asks for.
pub(super) fn overall_lights(commands: &mut Commands, lights: &[trp_data::interface::edit_build::PreviewLight]) -> Vec<Entity> {
    let layer = RenderLayers::layer(OVERALL_LIGHTS);
    let game = crate::rendering::shading::installed();
    lights.iter().filter(|light| light.on).map(|light| crate::screens::edit_build::preview_light(commands, light, in_view(light.position, light.rotation), &layer, game)).collect()
}

/// `--figure-hold <seconds>`: the OVERALL tab's own figure's clock stops
/// there at each showing, for a capture beside a picture of the game's
/// taken at that time of the figure's (`capture`). Not in a played run.
#[derive(Resource, Clone, Copy)]
pub struct FigureHold(pub f32);

/// `--figure-pick <pick>`: what the OVERALL tab's own figure's idle picks at
/// its first events, in order, at each showing, in place of its draws (the
/// later ones drawn): for a capture of a look or a weapon move at a time of
/// its. Not in a played run.
#[derive(Resource, Clone)]
pub struct FigurePicks(pub Vec<trp_sim::characters::figure::Pick>);

/// A pick as `--figure-pick` gives it: `idle`, `random:<1 to 4>` (1
/// `LookOut`, 2 `LookOut_Subtle`, 3 `LookOut_Alert`, 4 the melee check),
/// `move:<index>` (a weapon move by its place among the weapon's: `Look`,
/// then `CheckChamber`, then `CheckAmmo` and `Reload` where it has them).
pub fn parse_pick(given: &str) -> Option<trp_sim::characters::figure::Pick> {
    use trp_sim::characters::figure::Pick;
    let (what, number) = given.split_once(':').unwrap_or((given, ""));
    match what {
        "idle" => Some(Pick::Idle),
        "random" => number.parse().ok().filter(|n| (1..=4).contains(n)).map(Pick::Random),
        "move" => number.parse().ok().map(Pick::WeaponMove),
        _ => None,
    }
}

/// Shows each piece of the figure as its piece of the body is shown, and
/// asks for a drawing when that changes.
pub(super) fn mirror(figure: Option<ResMut<Figure>>, originals: Query<&Visibility, (Without<Twin>, crate::platform::park::EvenParked)>, mut twins: Query<(&Twin, &mut Visibility), crate::platform::park::EvenParked>) {
    let Some(mut figure) = figure else { return };
    for (twin, mut visibility) in &mut twins {
        let Ok(original) = originals.get(twin.0) else { continue };
        if *visibility != *original {
            *visibility = *original;
            figure.ask();
        }
    }
}

/// The figures' hung weapons, shown only while a picture of their figure is
/// drawn or about to be, and parked otherwise (`platform::park`): they are
/// on layers no other camera sees, and stayed in the engine's work every
/// frame of the run. The gear page's figure's while one of its drawings is
/// under way, or wanted and able to begin (with `TARKOV UI` its gear
/// picture has no size and is never drawn); the OVERALL tab's own figure's
/// while its camera is on (`live`). After `draw` and `live`, so that they
/// are back on the frame a drawing begins. The two figures themselves the
/// same way: each parked whole while no picture of it is drawn.
#[allow(clippy::type_complexity)]
pub(super) fn show_hung(
    figure: Option<Res<Figure>>,
    cameras: Query<&Camera>,
    (controller, selected): (Res<crate::player::controller::Controller>, Res<crate::screens::inventory_view::Selected>),
    mut hung: Query<(&crate::player::slung_view::FigureHung, &mut Visibility), crate::platform::park::EvenParked>,
    mut roots: Query<&mut Visibility, (Without<crate::player::slung_view::FigureHung>, crate::platform::park::EvenParked)>,
) {
    let Some(figure) = figure else { return };
    let soon = |shot: &Shot| shot.wanted && shot.camera.is_some() && shot.size != UVec2::ZERO;
    let own = figure.overall.is_some();
    let gear = figure.drawing.is_some() || soon(&figure.shots[GEAR]) || (!own && soon(&figure.shots[OVERALL]));
    let overall = figure.shots[OVERALL].camera.and_then(|camera| cameras.get(camera).ok()).is_some_and(|camera| camera.is_active);
    // The gear page's figure itself, with all on it: its skeleton, its
    // clothing and what hangs from it.
    if let Ok(mut visibility) = roots.get_mut(figure.skeleton.root) {
        let wish = if gear { Visibility::Inherited } else { Visibility::Hidden };
        if *visibility != wish {
            *visibility = wish;
        }
    }
    // The OVERALL tab's own figure likewise, while its camera is off and its
    // page is not shown: back from the frame its page shows, through the
    // frames it is made in, as it stood before it was parked; its pose
    // written by `live` even parked, so that it is right the frame it is back.
    let page = controller.inventory_open && selected.0 == crate::screens::inventory_view::OVERALL;
    if let Some(Ok(mut visibility)) = figure.overall_root().map(|root| roots.get_mut(root)) {
        let wish = if overall || page { Visibility::Inherited } else { Visibility::Hidden };
        if *visibility != wish {
            *visibility = wish;
        }
    }
    for (hung, mut visibility) in &mut hung {
        let seen = if hung.0 == crate::player::slung_view::Body::Overall { overall } else { gear };
        let wish = if seen { Visibility::Inherited } else { Visibility::Hidden };
        if *visibility != wish {
            *visibility = wish;
        }
    }
}

/// A picture with nothing in it to send, so that it is on the card at once
/// (`icons`, `ANSWER_FRAMES_AT_MOST`); the camera clears it.
fn target(images: &mut Assets<Image>, size: UVec2) -> Handle<Image> {
    let mut target = Image::new_uninit(Extent3d { width: size.x, height: size.y, depth_or_array_layers: 1 }, TextureDimension::D2, TextureFormat::Rgba8UnormSrgb, RenderAssetUsages::default());
    target.texture_descriptor.usage = TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::RENDER_ATTACHMENT;
    images.add(target)
}

/// The pictures: the gear page's the size of its panel in the window's
/// pixels (without `TARKOV UI`, whose gear page has no model), the OVERALL
/// tab's twice its view's size in the screen's units; and a drawing, when
/// one is wanted and the figure is all on the graphics card: one camera on
/// for a few frames, the model turned as its page has it, and then off;
/// the OVERALL tab's kept on while its turn changes (a drag).
#[allow(clippy::too_many_arguments)]
pub(super) fn draw(
    mut commands: Commands,
    figure: Option<ResMut<Figure>>,
    mut image: ResMut<FigureImage>,
    mut overall: ResMut<OverallFigure>,
    mut images: ResMut<Assets<Image>>,
    materials: Res<Assets<StandardMaterial>>,
    meshes: Res<Assets<Mesh>>,
    uploads: Res<UploadWatch>,
    waiting: Res<crate::PipelinesWaiting>,
    carried: Res<Carried>,
    features: Res<crate::screens::options_view::Features>,
    window: Query<&Window, With<PrimaryWindow>>,
    mut cameras: Query<(&mut Camera, &mut RenderTarget)>,
    mut transforms: Query<&mut Transform, crate::platform::park::EvenParked>,
    look: Res<crate::screens::health_view::HealthLook>,
) {
    let Some(mut figure) = figure else {
        // (No model to make.)
        (overall.remake, overall.making) = (false, false);
        return;
    };
    let figure = &mut *figure;
    if !figure.waiting {
        figure.waiting = true;
        // What was taken off since is gone: nothing waits for it.
        figure.meshes.retain(|mesh| meshes.contains(*mesh));
        figure.materials.retain(|material| materials.contains(*material));
        let pictures = figure.materials.iter().filter_map(|material| materials.get(*material)).flat_map(pictures_of).map(|picture| picture.id()).collect();
        uploads.wait(ON_CARD.to_owned(), pictures, figure.meshes.clone());
    }
    if !figure.on_card && !uploads.take(|name| name == ON_CARD).is_empty() {
        figure.on_card = true;
    }
    // The gear page's: of the panel's size in the window's own pixels,
    // made again when that changes.
    if let (Ok(window), Some(camera), false) = (window.single(), figure.shots[GEAR].camera, features.tarkov_ui) {
        let panel = crate::screens::inventory_view::gear_panel(&carried);
        let scale = window.height() / crate::screens::inventory_view::SCREEN_HEIGHT * window.scale_factor();
        let size = UVec2::new((panel[0] * scale).round().max(1.0) as u32, (panel[1] * scale).round().max(1.0) as u32);
        if size != figure.shots[GEAR].size {
            let made = target(&mut images, size);
            if let Some(old) = image.0.replace(made.clone()) {
                images.remove(&old);
            }
            if let Ok((_, mut render_target)) = cameras.get_mut(camera) {
                *render_target = RenderTarget::Image(made.into());
            }
            figure.shots[GEAR].size = size;
            figure.shots[GEAR].wanted = true;
        }
    }
    // The OVERALL tab's, once its page has said where it is (as the
    // inventory's screens are made, shown or not): its camera as the
    // scene's (`Camera_inventory`: no antialiasing), its picture twice the
    // view's whole units across and down (`CameraImage`). Until the page is
    // first shown the picture is a small one (`WARM_SIZE`), drawn into once
    // as the run begins: that builds the renderer's pipelines its drawings
    // take, which go by the picture's kind and not its size, without a
    // view's textures at its whole size; the page's first showing makes the
    // picture at its own size and draws it afresh, as every showing does.
    // Made at that showing instead, the camera cost that frame and the next
    // 24 to 30 ms.
    if overall.remake {
        figure.shots[OVERALL].shown = true;
    }
    // With `OVERALL FIGURE` the OVERALL tab draws its own figure, every
    // frame it is shown (`live`); here only its camera and picture.
    let live = figure.overall.is_some();
    if let (Some(rect), Some(scene_camera)) = (overall.rect, overall.camera) {
        let size = if figure.shots[OVERALL].shown { UVec2::new(2 * (rect[0].max(1.0) as u32), 2 * (rect[1].max(1.0) as u32)) } else { WARM_SIZE };
        // The game's eight lights, with its own figure: no ambient (the
        // camera forces it black), PRISM's filmic tonemap, on the lights'
        // layer only its camera sees (inventory spec, "The character's
        // light"); summed as the game sums them, in
        // its numbers, by its engine's light (`shading::asking`, with no
        // brightness term: the game's ambient pass is not on this camera),
        // whatever `TARKOV SHADING` says.
        let eight = look.overall.as_ref().filter(|_| live && figure.shots[OVERALL].camera.is_none()).map(|panel| (panel.lights.clone(), panel.prism.clone())).filter(|(lights, _)| !lights.is_empty());
        if let Some((lights, prism)) = eight {
            let camera = commands
                .spawn((
                    Name::new("Character model's camera, the OVERALL tab's own"),
                    Camera3d { depth_texture_usages: crate::effects::glass_view::depth_usages(), ..default() },
                    Camera { is_active: false, order: -5, clear_color: ClearColorConfig::Custom(Color::NONE), ..default() },
                    if crate::rendering::shading::installed() { crate::rendering::shading::asking(0.0) } else { AmbientLight { color: Color::WHITE, brightness: 0.0, ..default() } },
                    bevy::render::view::Msaa::Off,
                    Projection::Perspective(PerspectiveProjection {
                        fov: scene_camera.fov.to_radians(),
                        near: scene_camera.near,
                        near_clip_plane: Vec4::new(0.0, 0.0, -1.0, -scene_camera.near),
                        far: scene_camera.far,
                        ..default()
                    }),
                    overall_camera(&scene_camera),
                    RenderLayers::from_layers(&[OVERALL_LAYER, OVERALL_LIGHTS]),
                ))
                .id();
            let made = overall_lights(&mut commands, &lights);
            // Its pieces then drawn with the game's own surface, lit by
            // these (`figure_surface`).
            if crate::rendering::shading::installed() {
                crate::screens::figure_surface::set_lit();
            }
            if let Some(prism) = prism.as_ref().filter(|prism| prism.use_tonemap && prism.tonemap_type == 0) {
                commands.entity(camera).insert((bevy::camera::Exposure { ev100: 0.0 }, bevy::render::view::Hdr, bevy::core_pipeline::tonemapping::Tonemapping::None, crate::screens::preview_tonemap::PreviewTonemap::filmic(prism)));
            }
            info!(
                "the OVERALL tab's own figure's camera: the game's {} lights ({} made), {}, {}",
                lights.len(),
                made.len(),
                if crate::rendering::shading::installed() { "summed by the game's light rule" } else { "by Bevy's light" },
                if prism.is_some() { "PRISM's filmic tonemap" } else { "no PRISM read, Bevy's tonemap" }
            );
            figure.shots[OVERALL].camera = Some(camera);
        }
        if figure.shots[OVERALL].camera.is_none() {
            // Lit as the inspect window's model is: its two lights, no ambient, its photographic
            // tonemap; where that preview cannot be read, the gear page's
            // light and ambient. (Read with the rest of the screens, which
            // the page's place comes with: `HealthLook::inspect`.)
            // (With its own figure and no eight lights read, the same on
            // its own figure's layer.)
            let inspect = look.inspect.clone();
            let ambient = if inspect.is_some() { 0.0 } else { FIGURE_AMBIENT };
            let lights = if inspect.is_some() { OVERALL_LIGHTS } else { GEAR_LIGHTS };
            let drawn = if live { OVERALL_LAYER } else { LAYER };
            let camera = commands
                .spawn((
                    Name::new("Character model's camera, the OVERALL tab's"),
                    Camera3d { depth_texture_usages: crate::effects::glass_view::depth_usages(), ..default() },
                    Camera { is_active: false, order: -5, clear_color: ClearColorConfig::Custom(Color::NONE), ..default() },
                    AmbientLight { color: Color::WHITE, brightness: ambient, ..default() },
                    bevy::render::view::Msaa::Off,
                    Projection::Perspective(PerspectiveProjection {
                        fov: scene_camera.fov.to_radians(),
                        near: scene_camera.near,
                        near_clip_plane: Vec4::new(0.0, 0.0, -1.0, -scene_camera.near),
                        far: scene_camera.far,
                        ..default()
                    }),
                    overall_camera(&scene_camera),
                    RenderLayers::from_layers(&[drawn, lights]),
                ))
                .id();
            if live {
                warn!("the OVERALL tab's own figure: the game's eight lights were not read; lit as the inspect window's model");
            }
            if let Some(inspect) = &inspect {
                crate::screens::info_preview::inspect_lights(&mut commands, inspect, &RenderLayers::layer(OVERALL_LIGHTS));
                commands.entity(camera).insert(bevy::camera::Exposure { ev100: 0.0 });
                if let Some(exposure) = inspect.exposure {
                    commands.entity(camera).insert((bevy::render::view::Hdr, bevy::core_pipeline::tonemapping::Tonemapping::None, crate::screens::preview_tonemap::PreviewTonemap::of(exposure)));
                }
            }
            figure.shots[OVERALL].camera = Some(camera);
        }
        if size != figure.shots[OVERALL].size {
            let made = target(&mut images, size);
            if let Some(old) = overall.image.replace(made.clone()) {
                images.remove(&old);
            }
            // (Made this frame: set as it comes, on the next.)
            if let Some(camera) = figure.shots[OVERALL].camera {
                commands.entity(camera).insert(RenderTarget::Image(made.into()));
            }
            figure.shots[OVERALL].size = size;
            figure.shots[OVERALL].wanted = !live;
        }
        if figure.shots[OVERALL].yaw != overall.yaw && !live {
            figure.shots[OVERALL].wanted = true;
        }
        if overall.remake && !live {
            overall.remake = false;
            figure.shots[OVERALL].wanted = true;
            overall.making = true;
        }
    }
    // A drawing begun: the gear page's first; the model turned for it.
    // (The OVERALL tab's own figure is drawn by `live`.)
    if figure.drawing.is_none() && figure.on_card {
        let ready = |shot: &Shot| shot.wanted && shot.camera.is_some() && shot.size != UVec2::ZERO;
        let shots: &[usize] = if live { &[GEAR] } else { &[GEAR, OVERALL] };
        if let Some(index) = shots.iter().copied().find(|&index| ready(&figure.shots[index])) {
            figure.shots[index].wanted = false;
            figure.drawing = Some((index, 0));
        }
    }
    // The model made anew is in once its drawing is done.
    if overall.making && !live && !figure.shots[OVERALL].wanted && figure.drawing.is_none_or(|(index, _)| index != OVERALL) {
        overall.making = false;
    }
    let Some((index, frames)) = figure.drawing.as_mut() else { return };
    let index = *index;
    let yaw = if index == OVERALL { overall.yaw } else { YAW };
    // Turned since: drawn on.
    if figure.shots[index].yaw != yaw {
        figure.shots[index].yaw = yaw;
        *frames = 0;
    }
    if let Ok(mut root) = transforms.get_mut(figure.skeleton.root) {
        let rotation = turned(yaw);
        if root.rotation != rotation {
            root.rotation = rotation;
        }
    }
    *frames += 1;
    let done = (*frames > DRAW_FRAMES && waiting.0.load(std::sync::atomic::Ordering::Relaxed) == 0) || *frames > DRAW_FRAMES_AT_MOST;
    if done {
        figure.drawing = None;
    }
    for (at, shot) in figure.shots.iter().enumerate().filter(|(at, _)| !live || *at == GEAR) {
        let Some(Ok((mut camera, _))) = shot.camera.map(|camera| cameras.get_mut(camera)) else { continue };
        let on = at == index && !done;
        if camera.is_active != on {
            camera.is_active = on;
        }
    }
}
