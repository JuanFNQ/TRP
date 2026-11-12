//! The world set up: the main camera, the things shown, the scene.

use super::*;

#[derive(Component)]
pub struct MainCamera;

/// What moves of one of the range's things, by its place in `Range::things`.
#[derive(Component)]
pub struct ThingPart(pub usize);

/// Puts each thing's moving part where the thing stands now. The things
/// move on the simulation's frames (`Range::step_things`), and are drawn
/// as the last of them left them.
pub(super) fn show_things(range: Res<world::range::Range>, mut parts: Query<(&ThingPart, &mut Transform)>) {
    for (part, mut transform) in &mut parts {
        if let Some((place, _, _)) = range.things.get(part.0).and_then(|placed| placed.part_shown()) {
            if *transform != place {
                *transform = place;
            }
        }
    }
}

/// The near plane of the game's camera, as a recorded trace reports it.
const NEAR_PLANE: f32 = 0.03;

pub(super) fn setup_world(
    mut commands: Commands,
    options: Res<Options>,
    features: Res<screens::options_view::Features>,
    range: Res<world::range::Range>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<world::grid::GridMaterial>>,
    level: Option<Res<rendering::lighting::LevelLighting>>,
    mut images: ResMut<Assets<Image>>,
) {
    // The warm-up's curtain, over everything, in the sky's colour.
    let curtain = Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() };
    commands.spawn((WarmupProp, curtain, BackgroundColor(Color::srgb(0.55, 0.63, 0.70)), GlobalZIndex(1000)));
    let camera = commands.spawn((
        MainCamera,
        // It sees the hands' rigs besides the world (`weapon::HANDS_LAYER`).
        weapons::weapon::seeing_hands(),
        // Its depth readable after the main pass (`muzzle_view`'s copy).
        Camera3d { depth_texture_usages: (bevy::render::render_resource::TextureUsages::RENDER_ATTACHMENT | bevy::render::render_resource::TextureUsages::TEXTURE_BINDING).into(), ..default() },
        // The field of view follows the weapon motion every frame.
        // The near plane is the game camera's, as a recorded trace reports it.
        // Bevy 0.18 clips by a plane of its own besides (`near_clip_plane`,
        // 0.1 m unless told): it is put at the near plane, or everything
        // nearer than 10 cm is cut away.
        Projection::Perspective(PerspectiveProjection {
            fov: options.field_of_view.to_radians(),
            near: NEAR_PLANE,
            near_clip_plane: Vec4::new(0.0, 0.0, -1.0, -NEAR_PLANE),
            ..default()
        }),
        Transform::from_xyz(0.0, 1.6, 0.0),
        // The world blurred behind the inventory, as the game's camera
        // blurs it (`screen_blur`).
        ui::screen_blur::ScreenBlur::default(),
        // A contusion's screen effects and a flash's burn (`contusion_view`).
        effects::contusion_view::ContusionScreen::default(),
        effects::contusion_view::EyeBurnScreen::default(),
        // Blood on the screen (`blood_view`).
        effects::blood_view::BloodScreen::default(),
        // A shot's flash and the gun smoke (`muzzle_view`).
        weapons::muzzle_view::JetScreen::default(),
        weapons::muzzle_view::SmokeScreen::default(),
        effects::particles_view::FumeScreen::default(),
        // The sparks and the tracers (`tracer_view`), both of the flash's
        // pass.
        (effects::particles_view::SparkScreen::default(), weapons::tracer_view::TracerScreen::default(), weapons::flare_view::FlareScreen::default()),
        effects::particles_view::HazeScreen::default(),
        effects::explosion_view::ExplosionScreen::default(),
    )).id();
    // The scene's multisampling, as the options have it (`GRAPHICS`, `MSAA`).
    commands.entity(camera).insert(screens::options_view::msaa_of(options.msaa.unwrap_or(4)));
    // The view whose levels of detail a light's own shadow views take
    // (`lod_view`: TRP's own rule picks by each view's own pixel).
    commands.entity(camera).insert(assets::lod_view::LodMainView);
    // The game's ambient occlusion at the choice the run took, with
    // `AMBIENT OCCLUSION`: its passes, and Bevy's normal prepass for the
    // normals they read (`hbao_view`).
    if let Some(settings) = options.ssao.and_then(trp_sim::lighting::hbao::SsaoMode::hbao).filter(|_| features.ambient_occlusion) {
        commands.entity(camera).insert((bevy::core_pipeline::prepass::NormalPrepass, rendering::hbao_view::camera_parts(settings, options.ssao_ao_only)));
    }
    // With `TARKOV UI` the interface is drawn as the game's engine draws
    // its own, its edges hard: Bevy's softening of every node's edge over a
    // pixel left a light seam where two of the game's see-through pieces
    // meet (a slot's dark ground beside the next one's in the quick-use
    // bar). The app's own look keeps it (its one rounded corner).
    if features.tarkov_ui {
        commands.entity(camera).insert(bevy::ui_render::UiAntiAlias::Off);
    }
    // The screens composed in gamma space with `TARKOV UI` (`gamma_ui`).
    if features.tarkov_ui {
        commands.entity(camera).insert(ui::gamma_ui::GammaUi);
    }
    let cascades = rendering::shadows::with_baked_reach(rendering::shadows::cascades(options.shadow_quality.unwrap_or(rendering::shadows::START_QUALITY), NEAR_PLANE), world::range::baked_reach());
    match level.as_deref() {
        // Customs' sun, its ambient as an environment map, and the camera
        // drawing the scene's values in HDR for PRISM (`lighting`,
        // `prism_view`); the sky is `tod_sky_view`'s, black behind it as the
        // game's camera clears.
        Some(light) => {
            // The clouds with `CUSTOMS CLOUDS` (`clouds_view`): the sun
            // carries their shadows' cookie, laid from its light's place.
            let cookie = light.clouds.as_ref().and_then(|clouds| rendering::clouds_view::begin(&mut commands, &mut images, camera, clouds));
            let turned = Transform::default().looking_to(light.sun_forward, Vec3::Y);
            match cookie {
                Some((image, place, scale)) => {
                    commands.spawn((light.sun_light(), cascades, turned.with_translation(place).with_scale(Vec3::splat(scale)), bevy::light::DirectionalLightTexture { image, tiled: true }, rendering::lighting::RangeSun, weapons::weapon::seeing_hands()));
                }
                None => {
                    commands.spawn((light.sun_light(), cascades, turned, rendering::lighting::RangeSun, weapons::weapon::seeing_hands()));
                }
            }
            let environment = light.environment(&mut images);
            // The range hall's stencil volume over its inside, and its
            // lamps with `HALL LAMPS` (`lighting`, `range::hall_lamps`).
            if let Some(mut probe) = rendering::lighting::hall_probe(&environment) {
                // With the lamps, the light they bounce off the hall in place
                // of the sky's it takes away (`lighting::hall_bounce`).
                if features.hall_lamps {
                    if let Some(bounce) = rendering::lighting::hall_bounce(&mut images) {
                        probe.1 = bounce;
                    }
                }
                commands.spawn(probe);
            }
            // The game's ambient highlight over the hands' stencil type, with
            // `AMBIENT HIGHLIGHT` (`highlight_view`): within the hall, by its
            // indoor volume's alpha.
            if let Some((sh, multiplier, blur)) = light.highlight.filter(|_| features.ambient_highlight) {
                let rows = [sh.a[0], sh.a[1], sh.a[2], sh.b[0], sh.b[1], sh.b[2], sh.c].map(Vec4::from_array);
                let indoors = world::range::indoors().map(|(centre, size, yaw)| (Transform::from_translation(centre).with_rotation(Quat::from_rotation_y(yaw)).with_scale(size).to_matrix().inverse(), rendering::lighting::HALL_STENCIL));
                info!("the ambient highlight over the hands: times {multiplier}");
                commands.insert_resource(rendering::highlight_view::HighlightFigures { sh: rows, multiplier, blur, indoors });
            }
            if features.hall_lamps {
                for at in world::range::hall_lamps() {
                    commands.spawn((rendering::lighting::hall_lamp(at.y), Transform::from_translation(at).looking_to(Vec3::NEG_Y, Vec3::Z)));
                }
            }
            commands.entity(camera).insert((
                bevy::render::view::Hdr,
                bevy::core_pipeline::tonemapping::Tonemapping::None,
                bevy::camera::Exposure { ev100: rendering::lighting::EXPOSURE_EV100 },
                environment,
                rendering::prism_view::screen_of(&light.prism),
                Camera { clear_color: ClearColorConfig::Custom(Color::BLACK), ..default() },
            ));
            // The game's shading asked for, with its ambient pass's
            // brightness term (`shading`).
            if rendering::shading::on() {
                commands.entity(camera).insert(rendering::shading::asking(light.ambient_brightness()));
            }
            if let Some(filter) = &light.color_filter {
                commands.entity(camera).insert(rendering::color_filter_view::ColorFilterScreen::of(filter.amount));
            }
            if let Some(s) = &light.scattering {
                commands.entity(camera).insert((
                    rendering::scattering_view::ScatteringScreen::of(&light.sky.globals, s.height_falloff, s.density, s.sunrise_glow, &s.dithering),
                    rendering::scattering_view::ScatteringZero(s.zero_level),
                ));
            }
        }
        None => {
            commands.spawn((
                DirectionalLight { illuminance: rendering::lighting::RANGE_OWN_SUN, shadows_enabled: true, ..default() },
                weapons::weapon::seeing_hands(),
                // Its cascades as the game's shadow quality has them, from
                // the camera's near plane (`shadows`).
                cascades,
                // At the options' hour once Customs' time of day is read
                // (`range_day`).
                rendering::lighting::range_own_sun_turn(),
                rendering::lighting::RangeSun,
            ));
        }
    }

    // A plain test range: ground, posts every 5 m along a lane to judge
    // speed against, a ring of blocks to judge turning against, and two
    // walls to push the weapon into. The simulation feels the same boxes.
    // All of it carries the same grid of lines, to read distance, speed and
    // height by.
    let lined = |lines: world::grid::Lines| world::grid::GridLines { lines: if options.grid { lines } else { lines.none() } };
    commands.spawn((
        // (800 m across, for the reactive targets 300 m down
        // the lanes.)
        Mesh3d(meshes.add(Plane3d::default().mesh().size(800.0, 800.0))),
        MeshMaterial3d(materials.add(world::grid::GridMaterial {
            // A neutral grey:
            // with `CUSTOMS LIGHT` the camera's exposure holds the frame's
            // brightness, so a lighter ground darkens all else. Measured
            // (the camera's kept brightness, `docs/spec/lighting.md`, "On
            // the range"): at 0.35 the views from the start, before the
            // storage and down at the ground keep the exposure at its top;
            // at 0.45 two of them do not, and white darkens every one.
            base: StandardMaterial { base_color: Color::srgb(0.35, 0.35, 0.35), perceptual_roughness: 0.95, ..default() },
            extension: lined(world::grid::Lines::ground()),
        })),
    ));
    // One material for each colour, with lines or without, shared by every
    // block of it: each material holds six of the graphics card's places
    // for samplers, of which DirectX 12 has 2,048 for all that is drawn,
    // and one for each of the range's 67 blocks and
    // parts had taken 402 of them. The lines go by where a point is, so a
    // shared material draws each block as its own did.
    // The lamps' glowing undersides (`range::hall_lamps`), lit while the
    // lamps are, white as their light.
    let lit = features.hall_lamps && level.is_some();
    let glow = materials.add(world::grid::GridMaterial {
        base: StandardMaterial {
            base_color: Color::srgb(0.9, 0.9, 0.9),
            emissive: if lit { LinearRgba::rgb(2.4, 2.4, 2.4) } else { LinearRgba::BLACK },
            ..default()
        },
        extension: lined(world::grid::Lines::solid().none()),
    });
    let mut made: std::collections::HashMap<([u8; 4], bool), Handle<world::grid::GridMaterial>> = std::collections::HashMap::new();
    let mut material = |color: Color, lines: bool| {
        let key = (color.to_srgba().to_u8_array(), lines);
        made.entry(key)
            .or_insert_with(|| {
                let extension = if lines { lined(world::grid::Lines::solid()) } else { lined(world::grid::Lines::solid().none()) };
                materials.add(world::grid::GridMaterial { base: StandardMaterial::from_color(color), extension })
            })
            .clone()
    };
    // (A solid of no colour is not drawn: the workbench's, under its
    // model.)
    for solid in range.solids.iter().filter(|solid| solid.color != world::range::UNSEEN) {
        let place = Transform::from_translation(solid.centre).with_rotation(Quat::from_rotation_y(solid.yaw));
        commands.spawn((Mesh3d(meshes.add(Cuboid::from_size(solid.size))), MeshMaterial3d(material(solid.color, true)), place));
    }
    // The sampler of surfaces: each square a slab a centimetre thick on the
    // ground, a little apart from the next, drawn only (`range::surface_squares`).
    for square in world::range::surface_squares() {
        let slab = Vec3::new(square.size.x - 0.04, 0.01, square.size.y - 0.04);
        let place = Transform::from_translation(square.centre + Vec3::Y * slab.y / 2.0).with_rotation(Quat::from_rotation_y(square.yaw));
        commands.spawn((Mesh3d(meshes.add(Cuboid::from_size(slab))), MeshMaterial3d(material(square.color, false)), place));
    }
    // The hall's lamps' housings under the roof, which are no solids: their
    // undersides glowing while the lamps are on.
    let housing = material(Color::srgb(0.18, 0.18, 0.19), false);
    for at in world::range::hall_lamps() {
        commands.spawn((Mesh3d(meshes.add(Cuboid::new(0.7, 0.14, 0.7))), MeshMaterial3d(housing.clone()), Transform::from_translation(at + Vec3::Y * 0.08)));
        commands.spawn((Mesh3d(meshes.add(Cuboid::new(0.6, 0.01, 0.6))), MeshMaterial3d(glow.clone()), Transform::from_translation(at)));
    }
    // The barbed wire, which is no solid.
    let wire = material(Color::srgb(0.22, 0.20, 0.17), false);
    for (place, size) in world::range::barbed_wire_look() {
        commands.spawn((Mesh3d(meshes.add(Cuboid::from_size(size))), MeshMaterial3d(wire.clone()), place));
    }
    // What moves of the things the hands work: a door's leaf, a lid, a
    // drawer. Plain, without the grid.
    for (index, placed) in range.things.iter().enumerate() {
        let Some((place, size, color)) = placed.part_shown() else { continue };
        commands.spawn((ThingPart(index), Mesh3d(meshes.add(Cuboid::from_size(size))), MeshMaterial3d(material(color, false)), place));
    }

}
