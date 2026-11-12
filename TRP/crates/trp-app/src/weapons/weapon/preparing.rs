//! Preparing rigs ahead and freeing them.

use super::*;

/// Rigs being made ready on another thread, one at a time, in the order
/// asked: a weapon when it is first hung where the hands can take it
/// (`ask`). **Nothing of a rig's making is done in the frame** but putting
/// it into the world (`RigBuilder::finish`), so taking a weapon from the
/// crate does not stop the game for it: built in the frame, a rig had
/// cost 0.1 to 0.8 s there. The thread's reader is let go when nothing is
/// left to make.
#[derive(Resource)]
pub struct Preparing {
    pub(super) queue: VecDeque<String>,
    pub(super) seed: Seed,
    pub(super) worker: Option<Worker>,
    pub(super) task: Option<(String, bevy::tasks::Task<(Option<Worker>, anyhow::Result<Prepared>)>)>,
    /// `--measure-rigs`: every weapon's rig is made ready as it would be
    /// and measured, not put into the world (`Measured`).
    pub(super) measuring: Option<Measured>,
    /// A weapon whose rig is to be made before it is hung, if nothing hung
    /// waits (`ask_ahead`).
    pub(super) ahead: Option<String>,
}

/// What making every weapon's rig ready comes to (`--measure-rigs`): for
/// each, its name, the seconds it took away from the frame, the bytes of
/// its pictures and of its meshes (what goes to the graphics card), and its
/// nodes (the entities it would put into the world).
#[derive(Default)]
pub(super) struct Measured {
    rows: Vec<(String, f32, usize, usize, usize)>,
}

impl Measured {
    fn print(&self) {
        let mb = |bytes: usize| bytes as f32 / (1 << 20) as f32;
        for (name, seconds, pictures, meshes, nodes) in &self.rows {
            println!("rig {name}: {seconds:.2} s, pictures {:.1} MB, meshes {:.1} MB, {nodes} nodes", mb(*pictures), mb(*meshes));
        }
        let sum = |pick: fn(&(String, f32, usize, usize, usize)) -> usize| self.rows.iter().map(pick).sum::<usize>();
        let (pictures, meshes, nodes) = (sum(|row| row.2), sum(|row| row.3), sum(|row| row.4));
        let seconds: f32 = self.rows.iter().map(|row| row.1).sum();
        let mut each: Vec<usize> = self.rows.iter().map(|row| row.2 + row.3).collect();
        each.sort_unstable();
        let median = each.get(each.len() / 2).copied().unwrap_or(0);
        println!(
            "rigs measured: {}; in all {:.2} s away from the frame, pictures {:.0} MB, meshes {:.0} MB, {nodes} nodes; a rig's median {:.1} MB, the largest {:.1} MB",
            self.rows.len(),
            seconds,
            mb(pictures),
            mb(meshes),
            mb(median),
            mb(each.last().copied().unwrap_or(0))
        );
    }
}

impl Preparing {
    /// Whether a rig is being made ready, or waits to be.
    pub fn busy(&self) -> bool {
        self.task.is_some() || !self.queue.is_empty()
    }

    /// Asks for a weapon's rig before every other waiting.
    pub fn ask(&mut self, template: &str) {
        if self.task.as_ref().is_some_and(|(making, _)| making == template) || self.queue.front().is_some_and(|first| first == template) {
            return;
        }
        self.queue.retain(|waiting| waiting != template);
        self.queue.push_front(template.to_owned());
    }

    /// Asks for a weapon's rig ahead of its being hung (the mouse rests on
    /// it, or it has been taken up): made once nothing hung waits for a
    /// rig, and one at a time: a later ask takes the place of one not yet
    /// begun.
    pub fn ask_ahead(&mut self, template: &str) {
        if self.task.as_ref().is_some_and(|(making, _)| making == template) || self.queue.iter().any(|waiting| waiting == template) {
            return;
        }
        if self.ahead.as_deref() != Some(template) {
            self.ahead = Some(template.to_owned());
        }
    }
}

/// Each frame: what was put into the world on an earlier frame may be
/// taken in hand from now; a rig made ready is put into the world; and
/// the next waiting is begun, if none is being made.
pub fn prepare_rigs(preparing: Option<ResMut<Preparing>>, stowed: Option<ResMut<Stowed>>, mut builder: RigBuilder) {
    let (Some(mut preparing), Some(mut stowed)) = (preparing, stowed) else { return };
    let stowed = &mut *stowed;
    // A rig all on the graphics card may be taken in hand.
    let ready = builder.uploads.take(|template| stowed.fresh.iter().any(|fresh| fresh == template));
    for template in ready {
        if let Some(at) = stowed.fresh.iter().position(|fresh| *fresh == template) {
            stowed.built.push(stowed.fresh.remove(at));
        }
    }
    let preparing = &mut *preparing;
    let done = preparing.task.as_mut().and_then(|(_, task)| bevy::tasks::block_on(bevy::tasks::futures_lite::future::poll_once(task)));
    if let Some((worker, made)) = done {
        let (template, _) = preparing.task.take().expect("a task was polled");
        preparing.worker = worker;
        match made {
            // Measured, and not put into the world.
            Ok(prepared) if preparing.measuring.is_some() => {
                let mesh_bytes = prepared.looks.meshes.iter().map(|mesh| mesh.count_vertices() * mesh.get_vertex_size() as usize + mesh.indices().map_or(0, |indices| indices.len() * 4)).sum();
                let row = (prepared.weapon.name.clone(), prepared.took.iter().sum(), prepared.looks.picture_bytes(), mesh_bytes, prepared.weapon.hierarchy.nodes.len());
                if let Some(measured) = preparing.measuring.as_mut() {
                    measured.rows.push(row);
                }
            }
            Ok(prepared) => builder.finish(stowed, prepared),
            Err(e) => {
                warn!("the weapon {template} is not built: {e:#}");
                stowed.failed.push(template);
            }
        }
    }
    if preparing.task.is_some() {
        return;
    }
    // Nothing hung waits: the rig asked for ahead, if any.
    if preparing.queue.is_empty() {
        if let Some(template) = preparing.ahead.take().filter(|template| !stowed.has_rig(template) && !stowed.failed.contains(template)) {
            info!("{template}: its rig made ahead of its being hung");
            preparing.queue.push_back(template);
        }
    }
    while let Some(template) = preparing.queue.pop_front() {
        if stowed.has_rig(&template) || stowed.failed.contains(&template) {
            continue;
        }
        let (worker, seed, making) = (preparing.worker.take(), preparing.seed.clone(), template.clone());
        let task = bevy::tasks::AsyncComputeTaskPool::get().spawn(async move {
            match worker.map_or_else(|| Worker::new(&seed), Ok) {
                Ok(mut worker) => {
                    let made = worker.prepare(&making);
                    (Some(worker), made)
                }
                Err(e) => (None, Err(e)),
            }
        });
        preparing.task = Some((template, task));
        return;
    }
    // Every rig measured: what they came to, and the run ends.
    if let Some(measured) = preparing.measuring.take() {
        measured.print();
        builder.exit.write(AppExit::Success);
    }
    // Nothing left to make: the thread's reader and its bundles go, let
    // go of away from the frame (the reader and the database are many
    // small things, and freeing them is not quick).
    if let Some(worker) = preparing.worker.take() {
        bevy::tasks::AsyncComputeTaskPool::get()
            .spawn(async move {
                let began = std::time::Instant::now();
                drop(worker);
                info!("the thread that makes rigs ready let go of its reader of the install in {:.1} ms", began.elapsed().as_secs_f32() * 1000.0);
            })
            .detach();
    }
}

/// Lets go of the rigs nobody carries (a weapon hung where the hands can
/// take it, or in them, is carried; a rig still going up to the graphics
/// card is let be): one carried for the last time `FREE_AFTER` ago; the
/// one carried longest ago while more than `MOST_UNCARRIED` are kept.
/// Looked at twice a second.
pub fn free_rigs(
    mut commands: Commands,
    carried: Res<crate::screens::inventory_view::Carried>,
    stowed: Option<ResMut<Stowed>>,
    mut shared: ResMut<crate::assets::render_data::SharedPictures>,
    mut looked: Local<Option<std::time::Instant>>,
) {
    let Some(mut stowed) = stowed else { return };
    let now = std::time::Instant::now();
    if looked.is_some_and(|at| now.duration_since(at).as_secs_f32() < 0.5) {
        return;
    }
    *looked = Some(now);
    let stowed = &mut *stowed;
    let hung: Vec<String> = trp_sim::items::inventory::HAND_SLOTS.into_iter().filter_map(|slot| carried.equipment.worn(slot)).map(trp_sim::items::inventory::rig_key).collect();
    let mut uncarried: Vec<(std::time::Instant, String)> = Vec::new();
    for (template, own) in &mut stowed.own {
        if hung.contains(template) || stowed.held.as_deref() == Some(template.as_str()) || stowed.fresh.contains(template) || stowed.next_build.as_deref() == Some(template.as_str()) {
            own.carried = now;
        } else {
            uncarried.push((own.carried, template.clone()));
        }
    }
    uncarried.sort();
    let mut going: Vec<(String, String)> = Vec::new();
    for (index, (at, template)) in uncarried.iter().enumerate() {
        let left = uncarried.len() - index;
        let why = if now.duration_since(*at).as_secs_f32() > FREE_AFTER {
            format!("nobody having carried it for {FREE_AFTER:.0} s")
        } else if left > MOST_UNCARRIED {
            format!("{} rigs nobody carries being kept, and it carried longest ago", left)
        } else {
            continue;
        };
        going.push((template.clone(), why));
    }
    for (template, why) in going {
        let_go(&mut commands, stowed, &mut shared, &template, &why);
    }
}

/// Takes a rig made on the thread out of the world: its own entities, and
/// its pictures where no other rig draws with them.
fn let_go(commands: &mut Commands, stowed: &mut Stowed, shared: &mut crate::assets::render_data::SharedPictures, template: &str, why: &str) {
    let Some(own) = stowed.own.remove(template) else { return };
    let Some(at) = stowed.standing.iter().position(|(is, ..)| is.as_deref() == Some(template)) else { return };
    stowed.standing.remove(at);
    stowed.built.retain(|built| built != template);
    // (Each is taken with what hangs from it, which is its own too.)
    for entity in &own.entities {
        commands.entity(*entity).try_despawn();
    }
    let before = shared.len();
    shared.release(&own.marks);
    info!(
        "{template}: its rig let go of, {why}: {} entities, {} of its {} pictures off the graphics card ({} kept for the rigs that share them)",
        own.entities.len(),
        before - shared.len(),
        own.marks.len(),
        own.marks.len() - (before - shared.len())
    );
}

/// A rig of the hands' as the engine has it: where its own weapon bone
/// and the arms' bones are.
pub(super) fn rig_of(
    tree: &Hierarchy,
    entities: &[Entity],
    by_path: &HashMap<String, Entity>,
    pivot: Entity,
    limbs: Option<[trp_data::character::player::LimbData; 2]>,
    width: f32,
    kind: f32,
) -> WeaponRig {
    WeaponRig {
        root: entities[0],
        pivot,
        weapon_root: by_path.get("Weapon_root").copied(),
        weapon_root_anim: by_path.get("Weapon_root/Weapon_root_anim").copied(),
        arms: ['L', 'R'].into_iter().filter_map(|side| crate::player::arms::ArmRig::find(tree, entities, side)).collect(),
        limbs,
        by_hash: by_path.iter().map(|(path, entity)| (path_hash(path), *entity)).collect(),
        width,
        kind,
        bones: entities.to_vec(),
        shell_port: None,
        shell_ports: Vec::new(),
        muzzle: None,
        // The first fireport, as the weapon's length and the turn-away
        // take it (`trp_sim::weapon_line_ends`).
        fireport: tree.nodes.iter().position(|node| node.name == "fireport").and_then(|index| entities.get(index).copied()),
        launcher_fireport: None,
        launcher_muzzle: None,
        launcher_chamber: None,
        launcher_shell_port: None,
        devices: std::sync::Arc::new(Vec::new()),
    }
}

/// What kind of thing the body's animator is told a weapon is. The game
/// takes anything that is not a pistol, a knife, a thing thrown or a
/// launcher for a rifle (`Player.GetWeaponAnimationType`), an item used
/// among them; a revolver is a pistol to it.
pub(super) fn kind_of(db: &trp_data::Database, template: &str) -> f32 {
    let is = |class: &str| db.items.get(template).is_some_and(|item| db.is_a(item, class));
    if is("Pistol") || is("Revolver") {
        trp_sim::locomotion::body::hands_kind::PISTOL
    } else if is("ThrowWeap") {
        trp_sim::locomotion::body::hands_kind::THROWN
    } else if is("Knife") {
        trp_sim::locomotion::body::hands_kind::KNIFE
    } else if is("PortableRangeFinder") {
        // Held as a pistol (`Player.GetWeaponAnimationType`).
        trp_sim::locomotion::body::hands_kind::PISTOL
    } else {
        trp_sim::locomotion::body::hands_kind::RIFLE
    }
}

/// What a rig is made from: a firearm's stock build, an item's hands for
/// using it (medicine, food, drink: its `UsePrefab`), or a grenade's or a
/// melee weapon's own prefab (the item's `Prefab`).
pub(super) enum RigKind {
    Weapon,
    Medicine(trp_sim::condition::health::Medicine),
    Grenade,
    Knife,
    /// The handheld rangefinder (`PortableRangeFinder`; rangefinder spec).
    Rangefinder,
}

impl RigKind {
    pub(super) fn of(db: &trp_data::Database, template: &str) -> Self {
        if let Some(medicine) = trp_sim::condition::health::Medicine::of(db, template) {
            return Self::Medicine(medicine);
        }
        match db.items.get(template) {
            Some(item) if db.is_a(item, "ThrowWeap") => Self::Grenade,
            Some(item) if db.is_a(item, "Knife") => Self::Knife,
            Some(item) if db.is_a(item, "PortableRangeFinder") => Self::Rangefinder,
            _ => Self::Weapon,
        }
    }
}
