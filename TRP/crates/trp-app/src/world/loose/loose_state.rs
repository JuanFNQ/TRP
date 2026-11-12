//! How a thing goes loose, readied models, flights and the Loose resource.

use super::*;

/// How a thing left the character.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum How {
    /// Put down from the inventory, or a backpack off the shoulders
    /// (`GameWorld.ThrowItem` with no direction).
    Thrown,
    /// The magazine a quick reload lets go, where the weapon has it
    /// (`DropMod`).
    LetFall,
    /// A grenade thrown from the hand, from below or not
    /// (`BaseGrenadeHandsController.Throw`; grenades-and-knife spec, "The
    /// throw"): from the hand's grenade, with the throw's force, on its own
    /// body, until its fuse.
    Grenade { low: bool },
    /// A live round the weapon throws out (`Firearms.ThrowPatronAsLoot`):
    /// from its first shell port as drawn, by the round's own throw.
    FromPort,
}

/// A thrown grenade's own body (`GrenadeItself`'s rigid body and colliders,
/// read from the install), and its fuse's time.
#[derive(Clone)]
pub(super) struct GrenadeBody {
    pub(super) mass: f32,
    pub(super) drag: f32,
    pub(super) angular_drag: f32,
    pub(super) continuous: bool,
    pub(super) colliders: Vec<Collider>,
    pub(super) fuse: f32,
    /// Its item's template, which its blast goes by.
    pub(super) item: String,
}

/// A model come from the thread: what it draws and its colliders, in the
/// game's axes, in its first object's.
pub(super) struct Prepared {
    pub(super) template: String,
    pub(super) model: Model,
    pub(super) colliders: Vec<Collider>,
    pub(super) caliber: Option<u8>,
    /// A round's convex meshes' points, by mesh, kept for its colliders
    /// where it hangs in a thing on the ground (`RoundBody`).
    pub(super) convex: HashMap<trp_data::unity::assets::ObjRef, Arc<[V3]>>,
    /// Its parts that glow and shimmer with its heat (a weapon's).
    pub(super) hot: Vec<trp_data::weapons::heat::HotPart>,
}

/// A round's model as its colliders are had where it hangs in a magazine
/// or a weapon on the ground (`loose::rounds_colliders`): its objects and
/// its convex meshes' points.
pub(crate) struct RoundBody {
    pub(super) hierarchy: trp_data::unity::extract::Hierarchy,
    pub(super) convex: HashMap<trp_data::unity::assets::ObjRef, Arc<[V3]>>,
}

/// A model as the frame keeps it: each node's parent, own transform and
/// whether it is on, and its name; what each draws; its colliders; and
/// for a round its calibre's class (`AmmoPoolObject._caliber`).
pub(crate) struct Ready {
    pub(crate) nodes: Vec<(Option<usize>, Transform, bool)>,
    pub(crate) names: Vec<String>,
    pub(crate) drawn: Vec<(usize, Handle<Mesh>, Handle<StandardMaterial>)>,
    /// Beside `drawn`, each piece's material's figures for the game's own
    /// surface and its cube, where it is an install's (`game_surface`: the
    /// inspect window's preview draws with them).
    pub(crate) games: Vec<Option<(crate::assets::render_data::GameFigures, Option<Handle<Image>>)>>,
    /// What casts its shadow alone, as `drawn` (`icons::Model::shadows`):
    /// only the thing on the ground has it.
    pub(crate) shadows: Vec<(usize, Handle<Mesh>, Handle<StandardMaterial>)>,
    /// Beside `drawn`, the sights' own shader each piece is drawn with in
    /// the game, where it is one (`PreparedLooks::lens`).
    pub(crate) lens: Vec<Option<&'static str>>,
    /// Beside `drawn`, each piece's slot among its node's renderer's; none
    /// for a mesh of one's own weapon's not in the tree (`icons::Model::
    /// extra`).
    pub(super) slots: Vec<Option<usize>>,
    /// A weapon's parts that glow and shimmer with its heat, numbered by
    /// `nodes` (`trp_data::heat::read` on the thread, as the rig's are
    /// read): on the ground and slung on the body (`heat_view`).
    pub(super) hot: Vec<trp_data::weapons::heat::HotPart>,
    pub(crate) caliber: Option<u8>,
    /// A magazine's `patron_` points in it, by node and number, breadth
    /// first from the magazine's own object (its first object for a
    /// magazine, the first under its magazine's slot for a weapon:
    /// `magazine_rounds`); none for anything else.
    pub(crate) points: Vec<(usize, u32)>,
    /// A weapon's chambers' objects, by the slots' names
    /// (`patron_in_weapon`, `_000` on: the shallowest of that name, the
    /// weapon's own); none for anything else.
    pub(super) chambers: Vec<(String, usize)>,
    /// Each object placed in the first object's axes, in the game's
    /// (`loose::placed`): where a round hangs.
    pub(super) placed: Vec<loose::Placed>,
    pub(super) colliders: Vec<Collider>,
    /// A round's, for its colliders where it hangs in a thing on the
    /// ground; none for anything else.
    pub(super) round: Option<RoundBody>,
    /// The marks of the pictures it shares with the rigs and the rest
    /// (`render_data::SharedPictures`), to let go of when nothing on the
    /// ground draws with it any more.
    pub(super) marks: Vec<u64>,
    /// A weapon's tactical devices, numbered by `nodes` (`icons::Model::
    /// devices`): lit where it hangs on the body (`slung_view`).
    pub(crate) devices: Arc<Vec<trp_data::weapons::tactical::Device>>,
    /// With the levels of detail drawn by (`lod_view`, either switch): what
    /// draws at a coarser level only, as `drawn`, each with its sights'
    /// shader where it is one (`lens`); drawn by what picks the levels for
    /// its owner (`slung_view` for a body's, with the characters' on; a
    /// thing on the ground or in flight, the rounds it shows and a spent
    /// case with the world's things' on: `spawn_levels`), by nothing else,
    /// which draws the fullest at any distance.
    pub(crate) lower: Vec<(usize, Handle<Mesh>, Handle<StandardMaterial>, Option<&'static str>)>,
    /// Each node's levels of detail, where its group lists its renderer and
    /// that is not on bones: its group's place among `lod_groups`, and the
    /// levels; each group by the node it is on.
    pub(crate) node_lods: Vec<Option<(usize, u8)>>,
    pub(crate) lod_groups: Vec<(usize, Arc<crate::assets::lod_view::LodSpec>)>,
}

impl Ready {
    /// The levels of detail a piece drawn on the node is tagged with
    /// (`lod_view::LodPiece`), its group placed by the entity of the node
    /// the group is on (among `entities`, by node); none where it has none.
    pub(crate) fn lod_piece(&self, node: usize, entities: &[Entity]) -> Option<crate::assets::lod_view::LodPiece> {
        let (group, levels) = (*self.node_lods.get(node)?)?;
        let (on, spec) = self.lod_groups.get(group)?;
        Some(crate::assets::lod_view::LodPiece { group: spec.clone(), anchor: Some(*entities.get(*on)?), levels })
    }

    /// Its levels of detail drawn by the picker (`lod_view`): each piece of
    /// its fullest drawn (`pieces`: its place in `drawn` and its entity)
    /// tagged with its levels, and each piece of a coarser level spawned on
    /// its node's entity (among `entities`, by node), tagged, on `layer`
    /// where one is given; its sights' glass left out as `drawn`'s is. A
    /// piece no group lists stays untagged: drawn at any distance. Says how
    /// many it tagged and made.
    pub(crate) fn spawn_levels(&self, commands: &mut Commands, entities: &[Entity], pieces: &[(usize, Entity)], layer: Option<&bevy::camera::visibility::RenderLayers>) -> (usize, usize) {
        let mut tagged = 0;
        for &(k, entity) in pieces {
            let Some(&(node, _, _)) = self.drawn.get(k) else { continue };
            if let Some(lod) = self.lod_piece(node, entities) {
                commands.entity(entity).insert(lod);
                tagged += 1;
            }
        }
        let mut made = 0;
        for (node, mesh, material, lens) in &self.lower {
            let Some(lod) = self.lod_piece(*node, entities).filter(|_| !left_out(*lens, false)) else { continue };
            let part = commands.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(material.clone()), Transform::default(), lod)).id();
            if let Some(layer) = layer {
                commands.entity(part).insert(layer.clone());
            }
            commands.entity(entities[*node]).add_child(part);
            made += 1;
        }
        (tagged, made)
    }

    /// Its groups of levels of detail and their levels, for the log: how
    /// many groups, and how many of each number of levels.
    pub(crate) fn levels_said(&self) -> String {
        if self.lod_groups.is_empty() {
            return "no levels of detail".to_owned();
        }
        let counts: Vec<String> = self.lod_groups.iter().map(|(_, spec)| spec.heights.len().to_string()).collect();
        let deviation = self.lod_groups.first().map(|(_, spec)| spec.deviation.clone()).unwrap_or_default();
        format!("{} groups of levels of detail (levels {}; switching together, deviations {deviation:?} m), {} coarser pieces", self.lod_groups.len(), counts.join(", "), self.lower.len())
    }

    /// Its parts that glow and shimmer with its heat given their twins and
    /// states (`heat_view::spawn_glows`), on the pieces drawn (`pieces`:
    /// each piece's place in `drawn` and the entity drawing it), on the
    /// world's layer, their owner the model's root, given its item's heat
    /// (`heat_view::HeatOwner`; `seen_only` for one's own slung weapon).
    pub(crate) fn spawn_glows(&self, commands: &mut Commands, root: Entity, item: Id, pieces: &[(usize, Entity)], seen_only: bool) {
        if self.hot.is_empty() {
            return;
        }
        let mut parts: Vec<Vec<(Handle<Mesh>, Handle<StandardMaterial>)>> = vec![Vec::new(); self.nodes.len()];
        for (k, (node, mesh, material)) in self.drawn.iter().enumerate() {
            if self.slots.get(k).copied().flatten().is_some() {
                parts[*node].push((mesh.clone(), material.clone()));
            }
        }
        let drawn: Vec<(usize, usize, Entity)> = pieces.iter().filter_map(|&(k, entity)| Some((self.drawn.get(k)?.0, self.slots.get(k).copied().flatten()?, entity))).collect();
        // (Its meshes drawn by their objects, not on bones: `unskinned`.)
        crate::weapons::heat_view::spawn_glows(commands, root, &self.hot, &drawn, &parts, &|_| false, 0);
        commands.entity(root).insert(crate::weapons::heat_view::HeatOwner::item(item, seen_only));
    }
}

/// What the thread brings back: its reader, how long the reader took to
/// make where this read made it (s), and the model read with how long that
/// took (s); no model where the reader alone was asked for (`Foreseen`).
pub(super) struct Read {
    pub(super) worker: Option<Worker>,
    pub(super) reader_made: Option<f32>,
    pub(super) model: Option<(String, anyhow::Result<Prepared>, f32)>,
}

pub(super) enum Asked {
    Waiting,
    Ready(Arc<Ready>),
    /// Not to be had: the thing lies as the plain box it was before.
    Failed,
}

/// A thing on its way down.
pub(super) struct Flight {
    pub(super) id: Id,
    pub(super) template: String,
    pub(super) mass: f32,
    /// Where it set off, and how; or, for a thing set down, the place on
    /// the ground it is set on (in the game's axes).
    pub(super) start: Start,
    pub(super) placed: bool,
    /// How long after it set off it is first drawn
    /// (`ThrowLootMakeVisibleDelay` for a throw of the inventory's).
    pub(super) hidden_for: f32,
    pub(super) since: f32,
    /// The physics' steps and the frames' times since it set off, while
    /// its model was still to come.
    pub(super) missed: Vec<(f32, f32)>,
    pub(super) moving: Option<Moving>,
    /// A thrown grenade's own body: it is not watched to a stop, and is
    /// gone at its fuse.
    pub(super) grenade: Option<GrenadeBody>,
    /// A weapon thrown from the inventory sounds as it lands
    /// (`landing_sound`): its rule, from where it set off.
    pub(super) landing: Option<trp_sim::sound::landing::Landing>,
}

pub(super) struct Moving {
    pub(super) world: World,
    pub(super) body: Body,
    pub(super) swept: bool,
    pub(super) watch: Watch,
}

/// A thing's model in the world.
#[derive(Component)]
pub struct LooseModel(pub Id);

/// Where a loose thing is: its centre of mass in the engine's axes,
/// which is what the eye looks for.
#[derive(Debug, Clone, Copy)]
pub struct Place {
    pub centre: Vec3,
}

#[derive(Resource)]
pub struct Loose {
    pub(super) install: trp_data::Install,
    pub(super) db: std::sync::Arc<trp_data::Database>,
    pub(super) physics: Option<PhysicsSettings>,
    pub(super) swept_under: f32,
    pub(super) hidden_for: f32,
    pub(super) worker: Option<Worker>,
    pub(super) task: Option<Task<Read>>,
    pub(super) queue: VecDeque<String>,
    pub(super) models: HashMap<String, Asked>,
    pub(super) flights: Vec<Flight>,
    pub(super) dice: Dice,
    /// Every loose thing's place, by item, once it has one.
    pub places: HashMap<Id, Place>,
    /// Grenades whose fuse has run out, to be taken off the ground.
    pub(super) blown: Vec<Id>,
    /// Models kept for something else of the world (`model_for`).
    pub(super) kept: Vec<String>,
    /// The thread's reader kept with nothing to read, for a screen that
    /// asks for models one after another (the edit build screen, while it
    /// is open).
    pub keep_reader: bool,
    /// The hard settings' `GrenadeForce`, which a throw goes by.
    pub(super) grenade_force: f32,
    /// The model of the grenade in hand, asked for as it is drawn and kept
    /// while it is held, for it to be there as it leaves the hand.
    pub(super) held: Option<String>,
    /// Whether a magazine's and a chamber's rounds are there (the options'
    /// `MAGAZINE ROUNDS`): a thing on the ground then shows them and its
    /// body has their colliders; off, neither.
    pub rounds: bool,
    /// What draws with the models, kept between frames (`Uses`).
    pub(super) uses: Uses,
    /// What the inventory screen may put down next (`Foreseen`).
    pub(super) foreseen: Foreseen,
}

impl Loose {
    pub fn new(install: trp_data::Install, db: std::sync::Arc<trp_data::Database>, physics: Option<PhysicsSettings>, hard: &trp_data::setup::hard_settings::HardSettings, seed: u64) -> Self {
        let figure = |name: &str, otherwise: f32| {
            hard.float(name).unwrap_or_else(|e| {
                eprintln!("{e:#}; {otherwise} is taken for {name}");
                otherwise
            })
        };
        // The engine's floor (`trp_physics::World::add_floor`, the range's
        // ground) cooked now, not on the first throw or death (some 30 ms,
        // once).
        trp_physics::cook_floor();
        Self {
            install,
            db,
            physics,
            swept_under: figure("LootVolumeForHighQuallityPhysicsClient", loose::SWEPT_UNDER),
            hidden_for: figure("ThrowLootMakeVisibleDelay", 0.15),
            worker: None,
            task: None,
            queue: VecDeque::new(),
            models: HashMap::new(),
            flights: Vec::new(),
            dice: Dice::new(seed ^ 0x6c6f_6f73_6521),
            places: HashMap::new(),
            blown: Vec::new(),
            kept: Vec::new(),
            keep_reader: false,
            grenade_force: figure("GrenadeForce", 11.55),
            held: None,
            rounds: false,
            uses: Uses::default(),
            foreseen: Foreseen::default(),
        }
    }

    /// Whether a thing on the ground is drawn as the plain box: its model
    /// cannot be had, or the engine's settings could not be read.
    pub fn stand_in(&self, id: Id, template: &str) -> bool {
        self.physics.is_none() || matches!(self.models.get(template), Some(Asked::Failed)) || !(self.places.contains_key(&id) || self.flights.iter().any(|f| f.id == id))
    }

    pub(super) fn ask(&mut self, template: &str) {
        if !self.models.contains_key(template) {
            self.models.insert(template.to_owned(), Asked::Waiting);
            self.queue.push_back(template.to_owned());
        }
    }

    /// A model asked for by something else of the world (a round for its
    /// spent case, `cases`, or for a magazine, `magazine_rounds`), kept
    /// while it is asked for, and had once read.
    /// The game's physics settings, where they could be read.
    pub fn physics(&self) -> Option<&PhysicsSettings> {
        self.physics.as_ref()
    }

    /// A model asked for by `model_for` that is no longer wanted there (the
    /// window that showed it shut): let go of once nothing else draws
    /// with it.
    pub fn let_go(&mut self, template: &str) {
        let kept = self.kept.len();
        self.kept.retain(|kept| kept != template);
        if self.kept.len() != kept {
            self.uses.stale = true;
        }
    }

    pub fn model_for(&mut self, template: &str) -> Option<Arc<Ready>> {
        if !self.kept.iter().any(|kept| kept == template) {
            self.kept.push(template.to_owned());
            self.uses.stale = true;
        }
        self.ask(template);
        match self.models.get(template) {
            Some(Asked::Ready(ready)) => Some(ready.clone()),
            _ => None,
        }
    }
}

/// The range as the engine has it: the ground on the terrain's layer, and
/// every box of the range (`Range::boxes`) on the layer the world's
/// colliders are on.
pub fn range_world(physics: &PhysicsSettings, range: &Range) -> World {
    let mut world = World::new(trp_physics::Settings::of(physics));
    let (terrain, high_poly) = (physics.layer("Terrain").unwrap_or(11), physics.layer("HighPolyCollider").unwrap_or(12));
    world.add_ground(0.0, Material::default(), terrain);
    for (centre, size, turn) in range.boxes() {
        let shape = Shape::Box { half: [size.x / 2.0, size.y / 2.0, size.z / 2.0] };
        world.add_static([centre.x, centre.y, -centre.z], convert::rotation_back(turn), &[Collider { shape, at: [0.0; 3], turn: [0.0, 0.0, 0.0, 1.0], material: Material::default() }], high_poly);
    }
    world
}
