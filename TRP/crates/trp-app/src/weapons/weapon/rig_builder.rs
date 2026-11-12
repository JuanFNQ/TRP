//! Uploads watched, the rig builder, the preparing worker and the looks.

use super::*;

/// What a rig made on the thread has of its own: its entities (not the
/// first rig's that it shares), the marks of the pictures it draws with
/// (`render_data::SharedPictures`), and when it was last carried.
pub(super) struct OwnOfRig {
    pub(super) entities: Vec<Entity>,
    pub(super) marks: Vec<u64>,
    pub(super) carried: std::time::Instant,
}

/// A rig nobody has carried (hung where the hands can take it, or in them)
/// for so many seconds is let go of: its entities, and the pictures no
/// other rig draws with, which leave the graphics card. Hung again, it is
/// made again, as it was the first time. `ESTIMATE`: the app's choice.
pub(super) const FREE_AFTER: f32 = 30.0;

/// So many rigs at the most are kept that nobody carries: one more, and
/// the one carried longest ago is let go of. `ESTIMATE`: the app's choice.
pub(super) const MOST_UNCARRIED: usize = 4;

// (Until 0.12.4 the rigs were also held to the graphics card's places for
// samplers: on DirectX 12, wgpu 24 copied every bind group's samplers into
// one heap of 2,048. wgpu 26, Bevy 0.17's, keeps each distinct sampler once
// for the device.)

/// The rigs put into the world whose meshes and pictures are still going
/// up to the graphics card, a few megabytes a frame (`UPLOAD_BUDGET`), and
/// those that have gone up: a rig is taken in hand only once all of it is
/// on the card, so that it is never drawn without them (the hands, and the
/// box that blinks for them, wait as they do for the rig). Shared by the
/// app's world, which asks, and the renderer's, which answers
/// (`watch_uploads`).
#[derive(Default)]
pub struct Uploads {
    /// A rig's template, its pictures and meshes, and the renderer's frames
    /// it has waited.
    pub(super) waiting: Vec<(String, Vec<AssetId<Image>>, Vec<AssetId<Mesh>>, u32)>,
    pub(super) ready: Vec<String>,
}

#[derive(Resource, Clone, Default)]
pub struct UploadWatch(pub std::sync::Arc<std::sync::Mutex<Uploads>>);

impl UploadWatch {
    /// Waits for these meshes and pictures to be on the card, under a name.
    pub fn wait(&self, name: String, pictures: Vec<AssetId<Image>>, meshes: Vec<AssetId<Mesh>>) {
        if let Ok(mut uploads) = self.0.lock() {
            uploads.waiting.push((name, pictures, meshes, 0));
        }
    }

    /// The names waited for that are all on the card now and that `mine`
    /// picks, taken; the rest are left for whoever waits for them.
    pub fn take(&self, mine: impl Fn(&str) -> bool) -> Vec<String> {
        let Ok(mut uploads) = self.0.lock() else { return Vec::new() };
        let (taken, left) = std::mem::take(&mut uploads.ready).into_iter().partition(|name| mine(name));
        uploads.ready = left;
        taken
    }
}

/// So many of the renderer's frames at most a rig waits for its uploads
/// (one that cannot go up is not waited for for ever).
const UPLOAD_FRAMES_AT_MOST: u32 = 300;

/// In the renderer's world: a rig whose meshes and pictures the renderer
/// has all prepared is ready.
pub fn watch_uploads(watch: Res<UploadWatch>, images: Res<bevy::render::render_asset::RenderAssets<bevy::render::texture::GpuImage>>, meshes: Res<bevy::render::render_asset::RenderAssets<bevy::render::mesh::RenderMesh>>) {
    let Ok(mut uploads) = watch.0.lock() else { return };
    let uploads = &mut *uploads;
    let mut index = 0;
    while index < uploads.waiting.len() {
        let (_, pictures, shapes, frames) = &mut uploads.waiting[index];
        *frames += 1;
        let up = pictures.iter().all(|id| images.get(*id).is_some()) && shapes.iter().all(|id| meshes.get(*id).is_some());
        if up || *frames > UPLOAD_FRAMES_AT_MOST {
            let (template, _, _, frames) = uploads.waiting.remove(index);
            if !up {
                warn!("{template}: not all of it was on the graphics card after {frames} frames; taken in hand as it is");
            } else {
                info!("{template}: on the graphics card after {frames} of the renderer's frames");
            }
            uploads.ready.push(template);
        } else {
            index += 1;
        }
    }
}

/// What putting a weapon's rig into the world needs of the app: the
/// stores of meshes, pictures, materials and bind poses.
#[derive(SystemParam)]
pub struct RigBuilder<'w, 's> {
    pub(super) commands: Commands<'w, 's>,
    pub(super) meshes: ResMut<'w, Assets<Mesh>>,
    pub(super) images: ResMut<'w, Assets<Image>>,
    pub(super) materials: ResMut<'w, Assets<StandardMaterial>>,
    pub(super) bindposes: ResMut<'w, Assets<SkinnedMeshInverseBindposes>>,
    pub(super) uploads: Res<'w, UploadWatch>,
    pub(super) shared: ResMut<'w, crate::assets::render_data::SharedPictures>,
    pub(super) exit: MessageWriter<'w, AppExit>,
}

impl RigBuilder<'_, '_> {
    /// Puts a rig made ready on another thread into the world, to stand
    /// by out of sight until it is taken in hand: its own weapon bone and
    /// what hangs from it, with the arms' bones shared with the first rig.
    /// Its entities are there from the end of the frame; the hands may
    /// take it from the next (`Stowed::fresh`).
    pub(super) fn finish(&mut self, stowed: &mut Stowed, prepared: Prepared) {
        let began = std::time::Instant::now();
        let Prepared { template, weapon, mut hands, looks, parts, skins, extra, kind, took } = prepared;
        let ((parts, skins, extra), pictures, meshes, shared, picture_count) =
            looks_into_world((looks, parts, skins, extra), &mut self.meshes, &mut self.images, &mut self.materials, &mut self.bindposes, &mut self.shared);
        self.uploads.wait(template.clone(), pictures.iter().map(Handle::id).collect(), meshes.iter().map(Handle::id).collect());
        let tree = spawn_nodes(&mut self.commands, &weapon.hierarchy, stowed.pivot, Some(&stowed.shared), &parts, &skins, &extra);
        glows_of(&mut self.commands, &weapon, &tree, &parts, &skins);
        info!(
            "{}: {} nodes, {} meshes, standing by, made ready in {:.2} s away from the frame (assembled {:.2}, its logic and animator {:.2}, its meshes and pictures {:.2}); put into the world in {:.1} ms; {} of its {} pictures already on the card ({:.1} MB not sent again)",
            weapon.name,
            weapon.hierarchy.nodes.len(),
            tree.rendered,
            took.iter().sum::<f32>(),
            took[0],
            took[1],
            took[2],
            began.elapsed().as_secs_f32() * 1000.0,
            shared.already,
            picture_count,
            shared.bytes_spared as f32 / (1 << 20) as f32
        );
        // What of it is its own: not the first rig's nodes that it shares.
        let first: std::collections::HashSet<Entity> = stowed.shared.values().copied().collect();
        let entities = tree.entities.iter().copied().filter(|entity| !first.contains(entity)).collect();
        stowed.own.insert(template.clone(), OwnOfRig { entities, marks: shared.marks, carried: std::time::Instant::now() });
        let mut rig = rig_of(&weapon.hierarchy, &tree.entities, &tree.by_path, stowed.pivot, stowed.limbs, weapon.cell_width as f32, kind);
        rig.shell_port = shell_port_of(&weapon, &tree.entities);
        rig.shell_ports = shell_ports_of(&weapon, &tree.entities);
        rig.muzzle = muzzle_of(&weapon, &tree.entities);
        (rig.launcher_fireport, rig.launcher_muzzle) = launcher_of(&weapon, &tree.entities);
        (rig.launcher_chamber, rig.launcher_shell_port) = launcher_ports_of(&weapon, &tree.entities);
        rig.devices = std::sync::Arc::new(weapon.devices.clone());
        if let Some(root) = rig.weapon_root {
            // Hidden while it stands by, and parked so (`platform::park`): out of the engine's
            // work until it is taken in hand.
            self.commands.entity(root).insert((Visibility::Hidden, crate::platform::park::Park));
        }
        // A weapon that lies in the crate has no round in its chamber,
        // and works its action when it is first taken
        // in hand. Its view is the field of view as it now stands.
        if let Some(firearm) = &mut hands.firearm {
            firearm.found();
        }
        hands.hands.set_field_of_view(stowed.field_of_view);
        stowed.standing.push((Some(template.clone()), rig, hands));
        stowed.fresh.push(template);
    }
}

/// A weapon's rig made ready away from the frame, all but its entities:
/// the weapon as assembled, its motion and logic, and its meshes,
/// pictures and materials as the engine's own values. For each node of
/// the weapon's tree, its meshes (each with its material) by their places
/// in `looks`, and the bones and bind poses of those drawn on bones; and a
/// weapon of one's own's parts, each with its node.
pub struct Prepared {
    pub(super) template: String,
    pub(super) weapon: trp_data::weapons::weapon::Weapon,
    pub(super) hands: HandsRig,
    pub(super) looks: PreparedLooks,
    pub(super) parts: Vec<Vec<(usize, usize)>>,
    pub(super) skins: Vec<Option<(Vec<usize>, Vec<Mat4>)>>,
    pub(super) extra: Vec<(usize, usize, usize)>,
    pub(super) kind: f32,
    /// Seconds: assembling it, its logic and animator, its meshes and
    /// pictures.
    pub(super) took: [f32; 3],
}

/// What the thread that makes rigs ready starts from, the same for every
/// rig of the run.
#[derive(Clone)]
pub(super) struct Seed {
    pub(super) install: trp_data::Install,
    /// The run's database, shared (a second one read for the thread took
    /// its time and as much memory again).
    pub(super) db: std::sync::Arc<trp_data::Database>,
    pub(super) hard: trp_data::setup::hard_settings::HardSettings,
    pub(super) engine: trp_data::setup::engine::EngineSettings,
    pub(super) hands_data: Option<trp_data::character::player::HandsData>,
    pub(super) view: trp_sim::ViewSettings,
    pub(super) mastering: u32,
    /// Which of the rigs' sounds are read (`HANDS SOUNDS`, `SHOT SOUNDS`).
    pub(super) sounds: crate::audio::hands_sound::Wanted,
    /// A build's underbarrel launcher is fitted to its hands
    /// (`UNDERBARREL LAUNCHER`).
    pub(super) launcher: bool,
}

/// That thread's own reader of the install and its own database, made
/// there the first time and kept from one rig to the next (the bundles a
/// rig loaded stay loaded for the next that shares them), and the game's
/// Vorbis setup headers as the rigs' sounds have asked for them.
pub(super) struct Worker {
    pub(super) unity: trp_data::unity::Assets,
    pub(super) db: std::sync::Arc<trp_data::Database>,
    pub(super) seed: Seed,
    pub(super) setups: trp_data::audio::sound::VorbisSetups,
}

impl Worker {
    pub(super) fn new(seed: &Seed) -> anyhow::Result<Self> {
        let began = std::time::Instant::now();
        let mut unity = trp_data::unity::Assets::new(seed.install.clone())?;
        // A window magazine's glass's cube (`glass_view`).
        crate::effects::glass_view::open_cubes(&mut unity);
        let worker = Self {
            unity,
            db: seed.db.clone(),
            seed: seed.clone(),
            setups: trp_data::audio::sound::VorbisSetups::new(&seed.install),
        };
        info!("the thread that makes rigs ready: its reader of the install and its database in {:.2} s", began.elapsed().as_secs_f32());
        Ok(worker)
    }

    /// A weapon's rig, or an item's (medicine, food or drink: its
    /// `UsePrefab`), all but its entities, by its key
    /// (`trp_sim::inventory::rig_key`): a weapon's build, or a template.
    pub(super) fn prepare(&mut self, key: &str) -> anyhow::Result<Prepared> {
        let began = std::time::Instant::now();
        let template = trp_sim::items::inventory::key_template(key);
        let Seed { hard, engine, hands_data, view, mastering, launcher, .. } = &self.seed;
        let kind = RigKind::of(&self.db, template);
        // An item's prefab by its field (`UsePrefab` for using it, `Prefab`
        // for a grenade's or a melee weapon's own).
        let prefab_of = |db: &trp_data::Database, field: &str| -> anyhow::Result<String> {
            let item = db.items.get(template).ok_or_else(|| anyhow::anyhow!("no item {template}"))?;
            item.props[field]["path"].as_str().filter(|path| !path.is_empty()).map(str::to_owned).ok_or_else(|| anyhow::anyhow!("{} has no hands to be held by", item.name))
        };
        let mut weapon = match &kind {
            RigKind::Medicine(_) => trp_data::weapons::weapon::assemble_prefab(&mut self.unity, &prefab_of(&self.db, "UsePrefab")?)?,
            RigKind::Grenade | RigKind::Knife | RigKind::Rangefinder => trp_data::weapons::weapon::assemble_prefab(&mut self.unity, &prefab_of(&self.db, "Prefab")?)?,
            RigKind::Weapon => {
                let mut weapon = trp_data::weapons::weapon::assemble_key(&mut self.unity, &self.db, key)?;
                // Its backup sights folded or not as its sights have them:
                // as any model of a weapon is made, then by the hands.
                trp_sim::weapons::sights::fold_sights_as_made(&mut weapon);
                trp_sim::weapons::sights::fold_sights(&mut weapon);
                weapon
            }
        };
        let assembled = began.elapsed().as_secs_f32();
        let props = self.db.items.get(template).map(|item| item.props.clone()).unwrap_or_default();
        let hands = match kind {
            RigKind::Medicine(medicine) => {
                let variants = props["AnimationVariantsNumber"].as_u64().unwrap_or(0) as u32;
                item_hands_of(&self.unity, &self.db, hard, engine, hands_data.clone(), &weapon, *view, medicine, variants).ok_or_else(|| anyhow::anyhow!("its hands could not be made"))?
            }
            RigKind::Grenade => {
                let thrown = trp_data::weapons::grenade::GrenadeData::read(&self.unity, &weapon)?;
                let hidden_in_throw = props["CanBeHiddenDuringThrow"].as_bool().unwrap_or(false);
                let (mut hands, animator) = plain_hands_of(&self.unity, &self.db, hard, engine, hands_data.clone(), &weapon, *view).ok_or_else(|| anyhow::anyhow!("its hands could not be made"))?;
                hands.grenade = Some(trp_sim::GrenadeHands::new(&animator, hidden_in_throw));
                hands.thrown_bone = thrown.throwing_parts.first().and_then(|name| weapon.hierarchy.nodes.iter().position(|node| &node.name == name));
                if hands.thrown_bone.is_none() {
                    warn!("{}: the hand's grenade ({:?}) is not on its rig: it cannot be thrown", weapon.name, thrown.throwing_parts.first());
                }
                hands.thrown = Some((template.to_owned(), std::sync::Arc::new(thrown)));
                hands
            }
            RigKind::Knife => {
                let (mut hands, animator) = plain_hands_of(&self.unity, &self.db, hard, engine, hands_data.clone(), &weapon, *view).ok_or_else(|| anyhow::anyhow!("its hands could not be made"))?;
                hands.knife = Some(trp_sim::KnifeHands::new(&animator, trp_sim::KnifeSettings::of(&props)));
                hands.blade = crate::weapons::knife::Blade::find(&weapon.hierarchy);
                if hands.blade.is_none() {
                    warn!("{}: no blade's box was found (`KnifeCollider`): its strokes meet nothing", weapon.name);
                }
                hands
            }
            RigKind::Rangefinder => {
                let (mut hands, animator) = plain_hands_of(&self.unity, &self.db, hard, engine, hands_data.clone(), &weapon, *view).ok_or_else(|| anyhow::anyhow!("its hands could not be made"))?;
                // Its eye is its own `aim_camera`, with no sight on it
                // (`FindAimTransformsWithoutSights`), at the controller's own
                // aiming speed and no aim sway (`ManualSetVariables`).
                // (Its eye squeezed at the field of view as `Hands::new`
                // squeezes a weapon's.)
                // Its scope, the prefab's own, a part of the sights whose
                // optic the controller switches (rangefinder spec).
                match trp_data::weapons::sight::prefab_part(&self.unity, &weapon.hierarchy, template) {
                    Ok(Some(part)) => weapon.sights.push(part),
                    Ok(None) => warn!("{}: no scope of its own was found", weapon.name),
                    Err(e) => warn!("{}: its scope could not be read: {e:#}", weapon.name),
                }
                let mut sights = trp_sim::weapons::sights::Sights::of_weapon(&weapon);
                if let Some(sights) = sights.as_mut() {
                    sights.set_scale(hands.hands.fov_scale());
                }
                hands.hands.settings.sights = sights;
                hands.hands.manual_set_variables(trp_sim::held::rangefinder::AIM_SPEED, 0.0);
                hands.hands.keep_fov_aimed();
                hands.rangefinder = Some(trp_sim::RangefinderHands::new(&animator));
                hands
            }
            RigKind::Weapon => {
                let hands = hands_of(&mut self.unity, &self.db, hard, engine, hands_data.clone(), &weapon, *view, *mastering, *launcher)
                    .ok_or_else(|| anyhow::anyhow!("its motion could not be made"))?;
                // A weapon with no logic of its own cannot be worked.
                anyhow::ensure!(hands.firearm.is_some(), "it has no logic of its kind");
                hands
            }
        };
        let mut hands = hands;
        hands.sounds = crate::audio::hands_sound::read(&self.unity, &weapon, &mut self.setups, self.seed.sounds);
        let animated = began.elapsed().as_secs_f32();
        let (looks, parts, skins, extra) = looks_of(&self.unity, &weapon);
        let at = began.elapsed().as_secs_f32();
        let kind = kind_of(&self.db, template);
        Ok(Prepared { template: key.to_owned(), weapon, hands, looks, parts, skins, extra, kind, took: [assembled, animated - assembled, at - animated] })
    }
}

/// What a rig draws, as the engine's own values: its meshes, its pictures
/// (marked, to be shared: `render_data::SharedPictures`) and materials, and
/// for each node of its tree what it draws (a mesh and a material by their
/// places), a skinned one's bones and poses, and a weapon of one's own's
/// parts (their node, mesh and material).
pub(crate) type Looks = (PreparedLooks, Vec<Vec<(usize, usize)>>, Vec<Option<(Vec<usize>, Vec<Mat4>)>>, Vec<(usize, usize, usize)>);

/// A weapon's looks, away from the frame (`Worker::prepare`) or, for the
/// first rig, on it (`spawn_weapon`); and the OVERALL figure's weapon in
/// hand's (`figure_view`).
pub(crate) fn looks_of(unity: &trp_data::unity::Assets, weapon: &trp_data::weapons::weapon::Weapon) -> Looks {
    looks_into(unity, weapon, PreparedLooks::default())
}

/// The same, into looks made so (the OVERALL figure's weapon's, its cubes
/// read: `PreparedLooks::with_figure_cubes`).
pub(crate) fn looks_into(unity: &trp_data::unity::Assets, weapon: &trp_data::weapons::weapon::Weapon, mut looks: PreparedLooks) -> Looks {
    let mut parts = vec![Vec::new(); weapon.hierarchy.nodes.len()];
    let mut skins = vec![None; weapon.hierarchy.nodes.len()];
    for (index, node) in weapon.hierarchy.nodes.iter().enumerate() {
        let Some(renderer) = drawn_renderer(node) else { continue };
        // A mesh on bones is drawn where they put it, not by its node.
        match looks.mesh_with_poses(unity, renderer.mesh, !renderer.skinned && weapon.hierarchy.mirrored(index)) {
            Ok((meshes, poses)) => {
                // On bones only where every mesh has its joints: one
                // with none drawn on bones fails the frame (the F-1's).
                let jointed = meshes.iter().all(|mesh| looks.meshes.get(*mesh).is_some_and(|mesh| mesh.attribute(Mesh::ATTRIBUTE_JOINT_INDEX).is_some()));
                match skin_bones(&weapon.hierarchy, index).filter(|bones| jointed && bones.len() == poses.len()) {
                    Some(bones) => skins[index] = Some((bones, poses)),
                    None => meshes.iter().for_each(|mesh| looks.unskin(*mesh)),
                }
                for (slot, mesh) in meshes.into_iter().enumerate() {
                    parts[index].push((mesh, looks.material(unity, renderer.materials.get(slot).copied().flatten())));
                }
            }
            Err(e) => warn!("mesh on {}: {e:#}", node.name),
        }
    }
    let mut extra = Vec::new();
    if let Some(custom) = weapon.custom.as_deref() {
        let materials = looks.custom_materials(&custom.materials);
        for part in &custom.parts {
            for (slot, mesh) in looks.custom_mesh(&part.mesh).into_iter().enumerate() {
                // Hung on its node and drawn as it lies.
                looks.unskin(mesh);
                if let Some(material) = part.materials.get(slot).and_then(|index| materials.get(*index)) {
                    extra.push((part.node, mesh, *material));
                }
            }
        }
    }
    looks.mark_pictures();
    (looks, parts, skins, extra)
}

/// What each node draws, its skins and a weapon of one's own's parts, as
/// the engine's handles.
pub(crate) type Drawn = (Vec<Vec<(Handle<Mesh>, Handle<StandardMaterial>)>>, Vec<Skin>, Vec<(usize, Handle<Mesh>, Handle<StandardMaterial>)>);

/// Looks into the engine's stores, a picture already on the card (by its
/// mark) drawn with that one: what is drawn; the pictures' and the meshes'
/// handles; what was taken of the shared pictures, and how many it has.
pub(crate) fn looks_into_world(
    (looks, parts, skins, extra): Looks,
    meshes: &mut Assets<Mesh>,
    images: &mut Assets<Image>,
    materials: &mut Assets<StandardMaterial>,
    bindposes: &mut Assets<SkinnedMeshInverseBindposes>,
    shared: &mut crate::assets::render_data::SharedPictures,
) -> (Drawn, Vec<Handle<Image>>, Vec<Handle<Mesh>>, crate::assets::render_data::Shared, usize) {
    let picture_count = looks.picture_count();
    let (meshes, materials, pictures, kept) = looks.add_sharing(meshes, images, materials, Some(shared));
    let drawn = |(mesh, material): &(usize, usize)| (meshes[*mesh].clone(), materials[*material].clone());
    let parts = parts.iter().map(|node| node.iter().map(drawn).collect()).collect();
    let skins = skins.into_iter().map(|skin| skin.map(|(bones, poses)| (bones, bindposes.add(SkinnedMeshInverseBindposes::from(poses))))).collect();
    let extra = extra.iter().map(|(node, mesh, material)| (*node, meshes[*mesh].clone(), materials[*material].clone())).collect();
    ((parts, skins, extra), pictures, meshes, kept, picture_count)
}
