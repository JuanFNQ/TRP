//! Spawning a weapon, and its sounds attached.

use super::*;

pub fn spawn_weapon(
    mut commands: Commands,
    mut game: ResMut<GameData>,
    options: Res<Options>,
    mut converted: ResMut<Converted>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut bindposes: ResMut<Assets<SkinnedMeshInverseBindposes>>,
    mut clip_player: ResMut<ClipPlayer>,
    mut controller: ResMut<crate::player::controller::Controller>,
    features: Res<crate::screens::options_view::Features>,
    mut shared: ResMut<crate::assets::render_data::SharedPictures>,
) {
    let GameData { unity, db, hard, engine, hands_data, .. } = &mut *game;
    // What each part of the weapon's making took (the run's log).
    let began = std::time::Instant::now();
    let mut took: Vec<(&str, f32)> = Vec::new();
    let mark = |part: &'static str, took: &mut Vec<(&str, f32)>| took.push((part, began.elapsed().as_secs_f32() * 1000.0));
    // The game's Vorbis setup headers, as the rigs' sounds ask for them.
    let mut setups = trp_data::audio::sound::VorbisSetups::new(&unity.install);
    // The first rig, which every rig of the hands built after it hangs
    // from and shares the arms' and the camera's bones with: the run's own
    // weapon's when the run begins with a weapon in hand (or plays clips
    // by their names); else the game's empty hands', and every weapon, the
    // run's own among them, has its rig made when it is first hung where
    // the hands can take it, as the rest are. The empty hands have every bone a
    // weapon's rig shares (`trp-sim/tests/every_weapon.rs`).
    let first = if options.starts_armed || !options.play.is_empty() || options.hold_at.is_some() {
        match trp_data::weapons::weapon::assemble(unity, db, &options.weapon) {
            Ok(mut weapon) => {
                // Its backup sights folded or not as its sights have them:
                // as any model of a weapon is made, then by the hands.
                trp_sim::weapons::sights::fold_sights_as_made(&mut weapon);
                trp_sim::weapons::sights::fold_sights(&mut weapon);
                Some(weapon)
            }
            Err(e) => {
                error!("could not assemble weapon {}: {e:#}", options.weapon);
                return;
            }
        }
    } else {
        None
    };
    for skipped in first.iter().flat_map(|weapon| &weapon.skipped) {
        warn!("part not placed: {skipped}");
    }
    mark("assembled", &mut took);
    let empty = match trp_data::weapons::weapon::assemble_prefab(unity, trp_data::weapons::weapon::EMPTY_HANDS) {
        Ok(empty) => Some(empty),
        Err(e) => {
            warn!("the empty hands' rig: {e:#}");
            None
        }
    };
    mark("the empty hands assembled", &mut took);
    let Some(base) = first.as_ref().or(empty.as_ref()) else {
        error!("neither the weapon {} nor the empty hands could be read: no hands", options.weapon);
        return;
    };

    let player = commands.spawn((Name::new("Player"), Player, Transform::default(), Visibility::default())).id();
    let pivot = commands.spawn((Name::new("View"), ViewPivot, Transform::default(), Visibility::default())).id();
    commands.entity(player).add_child(pivot);
    let eye = commands.spawn((Name::new("Eye"), Eye, Transform::default(), Visibility::default())).id();
    commands.entity(pivot).add_child(eye);

    // The body's animator picks how it carries the weapon by its width.
    controller.movement.body.set_weapon_width(base.cell_width as f32);

    let hierarchy = &base.hierarchy;
    let eye_index = hierarchy.nodes.iter().position(|n| n.name == "Camera_animated");
    // Made as every later rig is (`Worker::prepare`, `RigBuilder::finish`),
    // here on the frame's own thread: its pictures are shared by their
    // marks with what comes after (a magazine let go, another rig with
    // them), as a later rig's are. It is never let go.
    let ((parts, skins, extra), ..) = looks_into_world(looks_of(unity, base), &mut meshes, &mut images, &mut materials, &mut bindposes, &mut shared);
    let tree = spawn_nodes(&mut commands, hierarchy, pivot, None, &parts, &skins, &extra);
    if let Some(weapon) = first.as_ref() {
        glows_of(&mut commands, weapon, &tree, &parts, &skins);
    }
    let Tree { entities, by_path, rendered, .. } = tree;
    info!("{}: {} nodes, {} meshes", base.name, hierarchy.nodes.len(), rendered);
    mark("its meshes and pictures made", &mut took);

    // The game skins the arms to the body's skeleton and makes that follow
    // the weapon's arm bones. Until the body is in, the arms are skinned to
    // the weapon's arm bones directly.
    // The hands, and what their bundle draws beside them (a watch's case
    // and glass on some: `trp_data::arms::load`), each skinned the same way.
    match trp_data::character::arms::load(unity, &options.arms) {
        Ok(hands) => {
            for arms in std::iter::once(&hands).chain(&hands.beside) {
                let entry = match converted.mesh(unity, arms.mesh, false, &mut meshes) {
                    Ok(entry) => entry,
                    Err(e) => {
                        warn!("arms mesh: {e:#}");
                        continue;
                    }
                };
                let joints: Vec<Entity> = arms
                    .weapon_bone_paths()
                    .iter()
                    .map(|path| {
                        by_path.get(*path).copied().unwrap_or_else(|| {
                            warn!("arms bone {path} is not on the weapon");
                            entities[0]
                        })
                    })
                    .collect();
                let inverse_bindposes = bindposes.add(SkinnedMeshInverseBindposes::from(entry.bind_poses.clone()));
                // A watch's glass is not drawn (`trp_data::arms::is_glass`).
                for (slot, mesh) in entry.parts.iter().enumerate().filter(|(slot, _)| !arms.glass.get(*slot).copied().unwrap_or(false)) {
                    let material_ref = arms.materials.get(slot).copied().flatten();
                    let material = converted.material(unity, material_ref, &mut images, &mut materials);
                    let part = commands
                        .spawn((
                            Name::new("Arms"),
                            FirstPersonHands,
                            Mesh3d(mesh.clone()),
                            MeshMaterial3d(material),
                            SkinnedMesh { inverse_bindposes: inverse_bindposes.clone(), joints: joints.clone() },
                            // Skinned bounds are not tracked, and the arms are always in view.
                            NoFrustumCulling,
                            // The game's hands cast no shadow: their
                            // renderers are set so, and nothing changes
                            // it (recorded, tenth recording: drawn,
                            // shadow casting off). The arms' shadow on
                            // the ground is the upper body's sleeves.
                            bevy::light::NotShadowCaster,
                        ))
                        .id();
                    commands.entity(entities[0]).add_child(part);
                }
            }
        }
        Err(e) => warn!("arms {}: {e:#}", options.arms),
    }

    mark("the arms", &mut took);
    // A weapon's clips by their names, to be looked at one by one (`--play`,
    // `--at`): read for those alone (`animation::play_clips` plays nothing
    // else).
    if let Some(weapon) = first.as_ref().filter(|_| !options.play.is_empty() || options.hold_at.is_some()) {
        match clips_of(unity, weapon) {
            Some((library, idle, _)) => {
                clip_player.idle = idle;
                info!("{} animation clips, idle is {}", library.clips.len(), clip_player.idle);
                commands.insert_resource(library);
            }
            None => warn!("the weapon has no animation clips"),
        }
    }

    let view = trp_sim::ViewSettings {
        field_of_view: options.field_of_view,
        head_bobbing: options.head_bobbing,
        seed: options.seed,
        aim_speed_bonus: options.aim_speed_bonus.unwrap_or(0.0),
    };
    let in_hand = first.as_ref().and_then(|weapon| {
        // The run's mastering, if given; the family's is put on as the
        // first frame runs (`skills::mastery_apply`).
        let hands = hands_of(unity, db, hard, engine, hands_data.clone(), weapon, view, options.mastering.unwrap_or(0), features.underbarrel_launcher)?;
        mark("its hands and logic", &mut took);
        Some(hands)
    });
    // Its sounds, read on a worker (`SoundsComing`): nothing on the first
    // frame sounds, and they were a third of the weapon's making (65 ms).
    let wanted = crate::audio::hands_sound::Wanted::of(&features);
    if in_hand.is_some() && (wanted.hands || wanted.shots) {
        let (install, db, template) = (unity.install.clone(), db.clone(), options.weapon.clone());
        let task = bevy::tasks::AsyncComputeTaskPool::get().spawn(async move { read_sounds(install, &db, &template, wanted) });
        commands.insert_resource(SoundsComing(task));
    }
    if let Some(hands) = &in_hand {
        // The rangefinder's aim weighs 1 at its own rate
        // (`UsableItemController.IsAiming`: `Physical.Aim(1)`).
        controller.movement.stamina.aim_mass = if hands.rangefinder.is_some() { 1.0 } else { hands.aim_mass };
        controller.movement.stamina.set_rangefinder(hands.rangefinder.is_some());
    }

    if let Some(i) = eye_index {
        // The game takes a half turn about x as this bone's rest.
        info!("camera bone rests at {:?}", hierarchy.nodes[i].rotation);
    }
    let limbs = trp_data::character::player::LimbData::load(unity).and_then(|limbs| {
        limbs.iter().try_for_each(trp_sim::weapons::arms::check)?;
        Ok(limbs)
    });
    if let Err(e) = &limbs {
        warn!("the arms' solver: {e:#}");
    }
    let limbs = limbs.ok();

    // Every other weapon's rig is made ready on another thread when it is
    // first hung where the hands can take it (`Preparing::ask`): with every
    // weapon of the install in the range's crates, making them all from
    // the start held the curtain for minutes and kept every one's pictures
    // in memory.
    commands.insert_resource(Preparing {
        queue: options.measure_rigs.iter().cloned().collect(),
        seed: Seed {
            install: unity.install.clone(),
            db: db.clone(),
            hard: hard.clone(),
            engine: *engine,
            hands_data: hands_data.clone(),
            view,
            mastering: options.mastering.unwrap_or(0),
            sounds: crate::audio::hands_sound::Wanted::of(&features),
            launcher: features.underbarrel_launcher,
        },
        worker: None,
        task: None,
        measuring: (!options.measure_rigs.is_empty()).then(Measured::default),
        ahead: None,
    });
    if !options.measure_rigs.is_empty() {
        info!("measuring the rigs of {} weapons", options.measure_rigs.len());
    }

    let mut standing = Vec::new();
    let (rig, in_hand, held, built) = match &first {
        Some(weapon) => {
            let mut rig = rig_of(hierarchy, &entities, &by_path, pivot, limbs, weapon.cell_width as f32, kind_of(db, &options.weapon));
            rig.shell_port = shell_port_of(base, &entities);
            rig.shell_ports = shell_ports_of(base, &entities);
            rig.muzzle = muzzle_of(base, &entities);
            (rig.launcher_fireport, rig.launcher_muzzle) = launcher_of(base, &entities);
            (rig.launcher_chamber, rig.launcher_shell_port) = launcher_ports_of(base, &entities);
            rig.devices = std::sync::Arc::new(base.devices.clone());
            // The hands' other rig: the game's empty hands. Its arms and its
            // camera bone are the weapon rig's own entities; its weapon
            // bone, with the markers the arms are solved onto, is its own
            // and hangs beside the weapons'.
            if let Some(empty) = &empty {
                let tree = spawn_tree(&mut commands, unity, &mut converted, &mut meshes, &mut images, &mut materials, &mut bindposes, &empty.hierarchy, pivot, Some(&by_path), None);
                let stood = empty_hands_of(unity, db, hard, engine, hands_data.clone(), empty, &tree.entities, &tree.by_path, pivot, limbs, view);
                standing.extend(stood.map(|(rig, mut hands)| {
                    hands.sounds = crate::audio::hands_sound::read(unity, empty, &mut setups, crate::audio::hands_sound::Wanted::of(&features));
                    (None, rig, hands)
                }));
            }
            (rig, in_hand, Some(options.weapon.clone()), vec![options.weapon.clone()])
        }
        // The empty hands are the first rig, and in hand; no weapon has a
        // rig yet.
        None => {
            let empty = empty.as_ref().expect("the first rig is the empty hands'");
            let Some((rig, mut hands)) = empty_hands_of(unity, db, hard, engine, hands_data.clone(), empty, &entities, &by_path, pivot, limbs, view) else {
                error!("the empty hands could not be made: no hands");
                return;
            };
            hands.sounds = crate::audio::hands_sound::read(unity, empty, &mut setups, crate::audio::hands_sound::Wanted::of(&features));
            (rig, Some(hands), None, Vec::new())
        }
    };
    mark("the empty hands standing by", &mut took);
    info!("the weapon in hand made in {:.0} ms: {}", began.elapsed().as_secs_f32() * 1000.0, took.iter().map(|(part, at)| format!("{part} by {at:.0}")).collect::<Vec<_>>().join(", "));
    let empty = held.is_some() && !standing.is_empty();
    // Each rig's own tree is parked whenever it is hidden (`platform::park`).
    for root in std::iter::once(rig.weapon_root).chain(standing.iter().map(|(_, rig, _)| rig.weapon_root)).flatten() {
        commands.entity(root).insert(crate::platform::park::Park);
    }
    if let Some(hands) = in_hand {
        commands.insert_resource(hands);
        commands.insert_resource(Stowed {
            standing,
            held,
            empty,
            built,
            fresh: Vec::new(),
            failed: Vec::new(),
            pivot,
            shared: by_path.clone(),
            limbs,
            field_of_view: options.field_of_view,
            own: HashMap::new(),
            skills: trp_sim::condition::skills::Buffs::default(),
            aim_speed: 0.0,
            mastery: Default::default(),
            next_build: None,
        });
    }
    commands.insert_resource(rig);
}

/// The weapon in hand's sounds being read on a worker as the run starts
/// (`spawn_weapon`): with a reader of
/// its own, which assembles the weapon again (a weapon's objects are its
/// reader's), and put on its hands when they come (`attach_sounds`). The
/// warm-up waits for them, so that the first sound plays as before.
#[derive(Resource)]
pub struct SoundsComing(bevy::tasks::Task<Option<std::sync::Arc<crate::audio::hands_sound::RigSounds>>>);

fn read_sounds(install: trp_data::Install, db: &trp_data::Database, template: &str, wanted: crate::audio::hands_sound::Wanted) -> Option<std::sync::Arc<crate::audio::hands_sound::RigSounds>> {
    let mut unity = trp_data::unity::Assets::new(install.clone()).map_err(|e| warn!("the weapon's sounds are not played: their reader could not be made: {e:#}")).ok()?;
    let weapon = trp_data::weapons::weapon::assemble(&mut unity, db, template).map_err(|e| warn!("the weapon's sounds are not played: {template} could not be assembled: {e:#}")).ok()?;
    let mut setups = trp_data::audio::sound::VorbisSetups::new(&install);
    crate::audio::hands_sound::read(&unity, &weapon, &mut setups, wanted)
}

/// The weapon in hand's sounds, read: put on its hands.
pub fn attach_sounds(mut commands: Commands, coming: Option<ResMut<SoundsComing>>, hands: Option<ResMut<HandsRig>>) {
    let Some(mut coming) = coming else { return };
    let Some(sounds) = crate::assets::background::take(&mut coming.0, false, "the weapon's sounds") else { return };
    commands.remove_resource::<SoundsComing>();
    if let Some(mut hands) = hands {
        hands.sounds = sounds;
    }
}
