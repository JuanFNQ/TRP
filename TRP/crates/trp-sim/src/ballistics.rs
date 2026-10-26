//! A shot after it leaves the weapon: its flight, what it meets, and what
//! each hit does to it (`docs/spec/ballistics.md`). In Unity's axes; the
//! world it flies through is answered by the host (`BulletWorld`): the app's
//! range, or a recording's hits for a replay.

use crate::unity_random::{fast, PrecalculatedRandoms, UnityRandom};

pub type V3 = [f32; 3];

fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn scale(a: V3, k: f32) -> V3 {
    [a[0] * k, a[1] * k, a[2] * k]
}
fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn magnitude(a: V3) -> f32 {
    ((a[0] * a[0] + a[1] * a[1] + a[2] * a[2]) as f64).sqrt() as f32
}
/// Unity's `normalized`: nought for a vector shorter than 1e-5.
fn normalized(a: V3) -> V3 {
    let m = magnitude(a);
    if m > 1e-5 {
        [a[0] / m, a[1] / m, a[2] / m]
    } else {
        [0.0; 3]
    }
}
/// Unity's `Vector3.Lerp`, its share held within 0 and 1.
fn lerp(a: V3, b: V3, t: f32) -> V3 {
    let t = t.clamp(0.0, 1.0);
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}
/// Unity's `Vector3.Angle`, in degrees.
fn angle(from: V3, to: V3) -> f32 {
    let denominator = ((dot(from, from) * dot(to, to)) as f64).sqrt() as f32;
    if denominator < 1e-15 {
        return 0.0;
    }
    let cos = (dot(from, to) / denominator).clamp(-1.0, 1.0);
    (cos as f64).acos() as f32 * 57.295_78
}
fn clamp01(x: f32) -> f32 {
    x.clamp(0.0, 1.0)
}

/// The smallest float above nought (C#'s `float.Epsilon`), the game's test
/// for a chance that is there at all.
const EPSILON: f32 = f32::from_bits(1);

/// The standard G1 drag table (Mach number, drag coefficient), as published
/// and as the game has it from Mach 0 to 5.
const G1: [(f32, f32); 79] = [
    (0.0, 0.2629), (0.05, 0.2558), (0.1, 0.2487), (0.15, 0.2413), (0.2, 0.2344), (0.25, 0.2278), (0.3, 0.2214), (0.35, 0.2155),
    (0.4, 0.2104), (0.45, 0.2061), (0.5, 0.2032), (0.55, 0.202), (0.6, 0.2034), (0.7, 0.2165), (0.725, 0.223), (0.75, 0.2313),
    (0.775, 0.2417), (0.8, 0.2546), (0.825, 0.2706), (0.85, 0.2901), (0.875, 0.3136), (0.9, 0.3415), (0.925, 0.3734), (0.95, 0.4084),
    (0.975, 0.4448), (1.0, 0.4805), (1.025, 0.5136), (1.05, 0.5427), (1.075, 0.5677), (1.1, 0.5883), (1.125, 0.6053), (1.15, 0.6191),
    (1.2, 0.6393), (1.25, 0.6518), (1.3, 0.6589), (1.35, 0.6621), (1.4, 0.6625), (1.45, 0.6607), (1.5, 0.6573), (1.55, 0.6528),
    (1.6, 0.6474), (1.65, 0.6413), (1.7, 0.6347), (1.75, 0.628), (1.8, 0.621), (1.85, 0.6141), (1.9, 0.6072), (1.95, 0.6003),
    (2.0, 0.5934), (2.05, 0.5867), (2.1, 0.5804), (2.15, 0.5743), (2.2, 0.5685), (2.25, 0.563), (2.3, 0.5577), (2.35, 0.5527),
    (2.4, 0.5481), (2.45, 0.5438), (2.5, 0.5397), (2.6, 0.5325), (2.7, 0.5264), (2.8, 0.5211), (2.9, 0.5168), (3.0, 0.5133),
    (3.1, 0.5105), (3.2, 0.5084), (3.3, 0.5067), (3.4, 0.5054), (3.5, 0.504), (3.6, 0.503), (3.7, 0.5022), (3.8, 0.5016),
    (3.9, 0.501), (4.0, 0.5006), (4.2, 0.4998), (4.4, 0.4995), (4.6, 0.4992), (4.8, 0.499), (5.0, 0.4988),
];

/// The speed of sound the game takes, m/s.
const SOUND: f32 = 343.0;

/// The G1 drag coefficient at a speed (`CalculateG1DragCoefficient`): the
/// table's bracket by `floor(speed / 343 / 0.05)` of its places, drawn
/// straight by speed from the place before; nought at the bottom, the last
/// past the end.
pub fn g1_drag(speed: f32) -> f32 {
    let place = ((speed / SOUND / 0.05) as f64).floor() as i64;
    if place <= 0 {
        return 0.0;
    }
    if place as usize > G1.len() - 1 {
        return G1[G1.len() - 1].1;
    }
    let place = place as usize;
    let (low, high) = (G1[place - 1].0 * SOUND, G1[place].0 * SOUND);
    let below = G1[place - 1].1;
    (G1[place].1 - below) / (high - low) * (speed - low) + below
}

/// The step of a flight's table, and how many it may have (13 s).
pub const STEP: f32 = 0.01;
pub const STEPS: usize = 1300;
/// The time past which a flight is thrown on under gravity alone.
const TABLE_TIME: f32 = 13.0;

#[derive(Debug, Clone, Copy)]
struct Step {
    time: f32,
    position: V3,
    velocity: V3,
}

/// A bullet's flight (`TrajectoryCalculator`): its table of steps of 0.01 s,
/// made as far as they are asked for.
#[derive(Debug, Clone)]
pub struct Trajectory {
    steps: Vec<Step>,
    factor: f32,
    slowdown: f32,
    mass_x2: f32,
    gravity: V3,
}

impl Trajectory {
    /// From a start and a velocity, a round's mass (g), diameter (mm) and
    /// ballistic coefficient; `gravity` nought for a bot's shot.
    pub fn new(start: V3, velocity: V3, mass_g: f32, diameter_mm: f32, coefficient: f32, gravity: V3) -> Self {
        let mass = mass_g / 1000.0;
        let diameter = diameter_mm / 1000.0;
        let factor = mass * 0.001_422_3 / (diameter * diameter * coefficient);
        let area = diameter * diameter * std::f32::consts::PI / 4.0;
        let mut steps = Vec::with_capacity(64);
        steps.push(Step { time: 0.0, position: start, velocity });
        Self { steps, factor, slowdown: 1.2 * area, mass_x2: mass * 2.0, gravity }
    }

    fn next(&mut self) -> Step {
        let current = *self.steps.last().expect("a flight has its start");
        let speed = magnitude(current.velocity);
        let drag = g1_drag(speed) * self.factor;
        let slowing = scale(normalized(current.velocity), self.slowdown * (0.0 - drag) * speed * speed / self.mass_x2);
        let accel = add(self.gravity, slowing);
        let step = Step {
            time: current.time + STEP,
            position: add(add(current.position, scale(current.velocity, STEP)), scale(accel, 5e-5)),
            velocity: add(current.velocity, scale(accel, STEP)),
        };
        self.steps.push(step);
        step
    }

    fn step(&mut self, index: usize) -> Step {
        while self.steps.len() <= index {
            self.next();
        }
        self.steps[index]
    }

    /// Where it is and its velocity at a time since the shot
    /// (`PredictedTrajectoryCalculation`).
    pub fn at(&mut self, time: f32) -> (V3, V3) {
        let index = (((time / STEP) as f64).floor() as i64).clamp(0, STEPS as i64 - 1) as usize;
        if !(time > TABLE_TIME) && index + 1 <= STEPS - 1 {
            let (a, b) = (self.step(index), self.step(index + 1));
            (lerp(a.position, b.position, (time - a.time) / (b.time - a.time)), a.velocity)
        } else {
            let last = *self.steps.last().expect("a flight has its start");
            let t = time - last.time;
            (add(add(last.position, scale(last.velocity, t)), scale(self.gravity, 0.5 * t * t)), add(last.velocity, scale(self.gravity, t)))
        }
    }
}

/// A round's figures as a shot takes them (`AmmoTemplate`).
#[derive(Debug, Clone, PartialEq)]
pub struct Round {
    pub initial_speed: f32,
    pub mass_g: f32,
    pub diameter_mm: f32,
    pub ballistic_coefficient: f32,
    pub damage: f32,
    pub penetration_power: f32,
    pub penetration_deviation: f32,
    /// `PenetrationChanceObstacle`.
    pub penetration_chance: f32,
    pub ricochet_chance: f32,
    pub fragmentation_chance: f32,
    pub min_fragments: i32,
    pub max_fragments: i32,
    /// `ArmorDamage` over 100.
    pub armor_damage: f32,
    /// `AmmoLifeTimeSec`.
    pub life: f32,
    /// `ammoAccr`.
    pub accuracy: i32,
    pub projectiles: i32,
    pub penetration_damage_mod: f32,
    /// It carries an explosive (`ExplosiveAmmoComponent`).
    pub explosive: bool,
    /// What a hit of it does to a body beyond its damage (`DamageInfo`'s):
    /// the stamina burnt for each point (`StaminaBurnPerDamage`), and what it
    /// adds to the odds of a heavy and a light bleeding (`HeavyBleedingDelta`,
    /// `LightBleedingDelta`).
    pub stamina_burn: f32,
    pub heavy_bleeding: f32,
    pub light_bleeding: f32,
}

impl Round {
    /// A round's template from the item database.
    pub fn of(template: &trp_data::database::ItemTemplate) -> Option<Self> {
        let f = |name: &str| template.prop_f32(name);
        let whole = |name: &str| template.props.get(name).and_then(|v| v.as_f64()).map(|v| v as i32);
        Some(Self {
            initial_speed: f("InitialSpeed")?,
            mass_g: f("BulletMassGram")?,
            diameter_mm: f("BulletDiameterMilimeters")?,
            ballistic_coefficient: f("BallisticCoeficient")?,
            damage: f("Damage")?,
            penetration_power: f("PenetrationPower")?,
            penetration_deviation: f("PenetrationPowerDiviation").unwrap_or(0.0),
            penetration_chance: f("PenetrationChanceObstacle").unwrap_or(0.0),
            ricochet_chance: f("RicochetChance").unwrap_or(0.0),
            fragmentation_chance: f("FragmentationChance").unwrap_or(0.0),
            min_fragments: whole("MinFragmentsCount").unwrap_or(0),
            max_fragments: whole("MaxFragmentsCount").unwrap_or(0),
            armor_damage: f("ArmorDamage").unwrap_or(50.0) / 100.0,
            life: f("AmmoLifeTimeSec").unwrap_or(13.0),
            accuracy: whole("ammoAccr").unwrap_or(0),
            projectiles: whole("ProjectileCount").unwrap_or(1).max(1),
            penetration_damage_mod: f("PenetrationDamageMod").unwrap_or(0.0),
            explosive: template.props.get("HasGrenaderComponent").and_then(|v| v.as_bool()).unwrap_or(false),
            stamina_burn: f("StaminaBurnPerDamage").unwrap_or(0.0),
            heavy_bleeding: f("HeavyBleedingDelta").unwrap_or(0.0),
            light_bleeding: f("LightBleedingDelta").unwrap_or(0.0),
        })
    }

    /// `AmmoFactor`: what the round does to the weapon's spread.
    pub fn ammo_factor(&self) -> f32 {
        if self.accuracy <= 0 {
            (100.0 + self.accuracy.unsigned_abs() as f32) / 100.0
        } else {
            100.0 / (100 + self.accuracy) as f32
        }
    }
}

/// What a hit thing is made of: its material's number (`MaterialType`) and
/// its six figures (penetration level, penetration chance, ricochet chance,
/// fragmentation chance, deviation chance, deviation).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Surface {
    pub material: i32,
    pub figures: [f32; 6],
}

impl Surface {
    fn level(&self) -> f32 {
        self.figures[0]
    }
    fn penetration_chance(&self) -> f32 {
        self.figures[1]
    }
    fn ricochet_chance(&self) -> f32 {
        self.figures[2]
    }
    fn fragmentation_chance(&self) -> f32 {
        self.figures[3]
    }
    fn deviation_chance(&self) -> f32 {
        self.figures[4]
    }
    fn deviation(&self) -> f32 {
        self.figures[5]
    }
}

/// The materials' numbers (`MaterialType`).
pub mod material {
    pub const NONE: i32 = 0;
    pub const ASPHALT: i32 = 1;
    pub const BODY: i32 = 2;
    pub const CARDBOARD: i32 = 3;
    pub const CHAINFENCE: i32 = 4;
    pub const CONCRETE: i32 = 5;
    pub const GARBAGE_METAL: i32 = 7;
    pub const GARBAGE_PAPER: i32 = 8;
    pub const GLASS: i32 = 10;
    pub const GRASS_LOW: i32 = 14;
    pub const GRAVEL: i32 = 15;
    pub const METAL_THIN: i32 = 16;
    pub const METAL_THICK: i32 = 17;
    pub const PEBBLES: i32 = 19;
    pub const PLASTIC: i32 = 20;
    pub const SOIL: i32 = 22;
    pub const TILE: i32 = 24;
    pub const WATER_PUDDLE: i32 = 26;
    pub const WOOD_THIN: i32 = 27;
    pub const WOOD_THICK: i32 = 28;
    pub const BODY_ARMOR: i32 = 32;
    pub const SWAMP: i32 = 33;
    pub const HELMET: i32 = 34;
    pub const GLASS_VISOR: i32 = 35;
    pub const HELMET_RICOCHET: i32 = 36;
    pub const SNOW: i32 = 38;

    /// The materials' names, by number, as the game spells them.
    pub const NAMES: [&str; 39] = [
        "None", "Asphalt", "Body", "Cardboard", "Chainfence", "Concrete", "Fabric", "GarbageMetal", "GarbagePaper", "GenericSoft",
        "Glass", "GlassShattered", "Grate", "GrassHigh", "GrassLow", "Gravel", "MetalThin", "MetalThick", "Mud", "Pebbles", "Plastic",
        "Stone", "Soil", "SoilForest", "Tile", "Water", "WaterPuddle", "WoodThin", "WoodThick", "Tyre", "Rubber", "GenericHard",
        "BodyArmor", "Swamp", "Helmet", "GlassVisor", "HelmetRicochet", "MetalNoDecal", "Snow",
    ];

    /// A material by its name (any case).
    pub fn named(name: &str) -> Option<i32> {
        NAMES.iter().position(|n| n.eq_ignore_ascii_case(name)).map(|at| at as i32)
    }

    /// The range's material for a surface one walks on (`ESurfaceSound`'s
    /// number): ballistics spec, "Surfaces", this work's table.
    pub fn of_surface(surface: u8) -> i32 {
        const BY_SURFACE: [i32; 20] = [
            CONCRETE,      // Concrete
            METAL_THICK,   // Metal
            WOOD_THICK,    // Wood
            SOIL,          // Soil
            GRASS_LOW,     // Grass
            ASPHALT,       // Asphalt
            GLASS,         // Glass
            GRAVEL,        // Gravel
            METAL_THIN,    // MetalThin
            WATER_PUDDLE,  // Puddle
            PEBBLES,       // Slate
            TILE,          // Tile
            WOOD_THICK,    // WoodThick
            SWAMP,         // Swamp
            GARBAGE_PAPER, // Garbage
            GARBAGE_METAL, // GarbageMetal
            PLASTIC,       // Plastic
            GLASS,         // WholeGlass
            WOOD_THIN,     // WoodThin
            SNOW,          // Snow
        ];
        BY_SURFACE.get(surface as usize).copied().unwrap_or(CONCRETE)
    }
}

/// The figures of a material by the presets (`ColliderPresets`): the first
/// preset of that material, as an editor finds it (`FindPresetIndex`).
pub fn preset_surface(presets: &[trp_data::hard_settings::BallisticPreset], material: i32) -> Surface {
    presets
        .iter()
        .find(|p| p.material == material)
        .map(|p| Surface { material, figures: std::array::from_fn(|k| p.figures.get(k).copied().unwrap_or(0.0)) })
        .unwrap_or(Surface { material, figures: [0.0; 6] })
}

/// What a ray meets (one hit for each collider, Unity's `RaycastNonAlloc`):
/// how far from the ray's start, where, the face's normal, the collider and
/// the ballistic thing it belongs to (several colliders may make one thing),
/// whether the collider is a mesh (the backward cast goes on through a
/// mesh), and what it is made of.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RayHit {
    pub distance: f32,
    pub point: V3,
    pub normal: V3,
    pub collider: u64,
    pub thing: u64,
    pub mesh: bool,
    pub surface: Surface,
    /// A character's hit collider (body hits spec), none for anything else.
    pub body: Option<BodyRef>,
}

/// A character's hit collider a ray met (body hits spec, "The hit
/// colliders"): whose (the host's number), its body part (`EBodyPart`),
/// collider type and plate (0 for a body part's own), and its damage share
/// (`penetrationDamageMod`). Its other figures are the hit's surface's.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BodyRef {
    pub character: u32,
    pub part: u8,
    pub collider: u8,
    pub plate: u16,
    pub damage_mod: f32,
}

impl BodyRef {
    /// `IsParentHasSameBodyPart`'s test of two hits: a plate's only the same
    /// plate of the same part; a plate and a body collider never the same.
    fn same_part(&self, other: &BodyRef) -> bool {
        match (self.plate, other.plate) {
            (0, 0) => self.part == other.part,
            (a, b) if a != 0 && b != 0 => self.part == other.part && a == b,
            _ => false,
        }
    }
}

/// Which bullet asks (a replay's world answers by it; the app's ignores it).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Asker {
    /// The shot it is of (the host's number for it).
    pub shot: u32,
    pub fragment: i32,
    pub seed: i32,
    /// The cast looks along the bullet's way (the forward one), not back.
    pub forward: bool,
}

/// The world a bullet flies through: every collider on the bullets' layers
/// that a ray from `from` toward `to`, within its length, enters through a
/// face turned toward it (nothing it starts inside), one hit each.
pub trait BulletWorld {
    fn ray(&self, asker: Asker, from: V3, to: V3, hits: &mut Vec<RayHit>);

    /// The armour a character wears and its skills (body hits spec), none
    /// for one that wears none or is not known.
    fn wearer(&self, _character: u32) -> Option<&crate::armor::Wearer> {
        None
    }

    /// A hit handed on as the game's shot delegate takes it, right after its
    /// bullet's step and before the bullets it made fly (`UpdateShots`): a
    /// body's damage is done here, so that its armour's wear is there for
    /// the next hit.
    fn delivered(&mut self, _hit: &Hit) {}
}

/// How a hit left the bullet (`EBulletState`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Flying,
    DeviationHit,
    RicochetHit,
    FragmentationHit,
    StopHit,
}

impl State {
    pub fn name(self) -> &'static str {
        match self {
            State::Flying => "Flying",
            State::DeviationHit => "DeviationHit",
            State::RicochetHit => "RicochetHit",
            State::FragmentationHit => "FragmentationHit",
            State::StopHit => "StopHit",
        }
    }
}

/// One bullet in flight (`Shot`).
#[derive(Debug, Clone)]
pub struct Bullet {
    pub shot: u32,
    pub fragment: i32,
    pub seed: i32,
    /// How many bullets came before it (`Parent` and theirs).
    pub depth: u32,
    /// How many of those ricocheted.
    ricochets: u32,
    /// The things and colliders those before it hit.
    before: Vec<(u64, u64)>,
    pub round: std::sync::Arc<Round>,
    /// The way it was made with (`Direction`: not always a unit).
    direction: V3,
    pub initial_speed: f32,
    /// The speed it was made with (`Speed`).
    pub speed: f32,
    pub damage: f32,
    pub penetration: f32,
    penetration_chance: f32,
    ricochet_chance: f32,
    fragmentation_chance: f32,
    deviation_chance: f32,
    min_fragments: i32,
    max_fragments: i32,
    pub armor_damage: f32,
    life: f32,
    /// Time since it was made.
    pub age: f32,
    pub position: V3,
    pub velocity: V3,
    trajectory: Trajectory,
    pub state: State,
    done: bool,
    /// The thing and collider it hit.
    hit_thing: u64,
    hit_collider: u64,
    /// The pieces of armour that deflected and blocked it (`DeflectedBy`,
    /// `BlockedBy`: none for a bullet newly made).
    pub deflected_by: Option<u32>,
    pub blocked_by: Option<u32>,
    /// The forward hits on characters' colliders of the bullets before it,
    /// and the characters any of them hit (`AvoidAdditionalDamage`'s tests).
    forward_parts: Vec<BodyRef>,
    characters: Vec<u32>,
    /// Its hit on a character's collider, and whether it was forward.
    hit_body: Option<BodyRef>,
    hit_forward: bool,
    /// It does no damage (`AvoidAdditionalDamage`).
    pub no_damage: bool,
}

impl Bullet {
    fn finished(&self) -> bool {
        self.done || self.age > self.life
    }
}

/// A hit, as the game's shot delegate sees it, and what came of it.
#[derive(Debug, Clone)]
pub struct Hit {
    pub shot: u32,
    pub fragment: i32,
    pub seed: i32,
    pub depth: u32,
    pub point: V3,
    pub normal: V3,
    pub forward: bool,
    pub thing: u64,
    pub collider: u64,
    pub surface: Surface,
    pub state: State,
    /// The speed it was made with.
    pub speed: f32,
    /// Its velocity at the hit.
    pub velocity: V3,
    pub damage: f32,
    pub penetration: f32,
    pub armor_damage: f32,
    /// The hit cosine: the face's normal on the segment's way.
    pub cos: f32,
    /// Its time since it was made, at the hit.
    pub age: f32,
    /// How many bullets it made.
    pub fragments: usize,
    /// Past its life: the delegate does nothing with it.
    pub out_of_life: bool,
    /// `IsFirstHit`: the shot's first bullet, or the one it turned into.
    pub first_hit: bool,
    /// A character's collider it hit, and by which armour pieces it was
    /// deflected and blocked there (body hits spec).
    pub body: Option<BodyRef>,
    pub deflected_by: Option<u32>,
    pub blocked_by: Option<u32>,
    /// It does no damage: an earlier bullet of its shot hit the same part
    /// (`AvoidAdditionalDamage`).
    pub no_damage: bool,
    /// The way the bullet was made with (`Direction`: not always a unit),
    /// which a killing hit pushes a corpse along.
    pub direction: V3,
    pub round: std::sync::Arc<Round>,
}

/// The ballistics calculator (`BallisticsCalculator`): its tables, the
/// bullets in flight, and its settings.
pub struct Ballistics {
    tables: PrecalculatedRandoms,
    /// `Ballistic.GlobalDamageDegradationCoefficient`.
    pub degradation: f32,
    pub gravity: V3,
    bullets: Vec<Bullet>,
    hits_scratch: Vec<RayHit>,
}

impl Ballistics {
    pub fn new(degradation: f32, gravity: V3) -> Self {
        Self { tables: PrecalculatedRandoms::ballistics(), degradation, gravity, bullets: Vec::new(), hits_scratch: Vec::new() }
    }

    pub fn tables(&self) -> &PrecalculatedRandoms {
        &self.tables
    }

    pub fn bullets(&self) -> &[Bullet] {
        &self.bullets
    }

    /// A shot's bullets (`InitiateShot`): one, or for a round of several
    /// projectiles as many, each spread by the weapon's shotgun dispersion.
    /// `random` is Unity's own generator (the seeds, the pellets' ways);
    /// `speed_factor` the weapon's.
    pub fn shoot(&mut self, shot: u32, round: &std::sync::Arc<Round>, origin: V3, direction: V3, speed_factor: f32, shotgun_dispersion: f32, random: &mut UnityRandom) {
        if round.initial_speed <= 0.0 {
            return;
        }
        if round.projectiles == 1 {
            let seed = random.range_int(0, 512);
            self.create(shot, round, 0, seed, origin, direction, speed_factor);
        } else {
            for i in 0..round.projectiles {
                let pick = random.range_int(0, 512);
                let way = normalized(add(scale(direction, 100.0), scale(self.tables.direction(pick), shotgun_dispersion)));
                let seed = random.range_int(0, 512);
                self.create(shot, round, i * 2, seed, origin, way, speed_factor);
            }
        }
    }

    /// A bullet as a shot makes it (`CreateShot`), its seed given: for a
    /// replay, the recorded one.
    pub fn create(&mut self, shot: u32, round: &std::sync::Arc<Round>, fragment: i32, seed: i32, origin: V3, direction: V3, speed_factor: f32) {
        let speed = round.initial_speed * speed_factor;
        let penetration = self.tables.normal(seed, round.penetration_power, round.penetration_deviation);
        let bullet = self.made(
            shot,
            round,
            Made {
                fragment,
                seed,
                origin,
                direction,
                initial_speed: speed,
                speed,
                damage: round.damage,
                penetration,
                penetration_chance: round.penetration_chance,
                ricochet_chance: round.ricochet_chance,
                fragmentation_chance: round.fragmentation_chance,
                deviation_chance: 1.0,
                min_fragments: round.min_fragments,
                max_fragments: round.max_fragments,
            },
            None,
        );
        self.bullets.push(bullet);
    }

    fn made(&self, shot: u32, round: &std::sync::Arc<Round>, m: Made, parent: Option<&Bullet>) -> Bullet {
        let velocity = scale(m.direction, m.speed);
        let (depth, ricochets, before) = match parent {
            Some(p) => {
                let mut before = p.before.clone();
                before.push((p.hit_thing, p.hit_collider));
                (p.depth + 1, p.ricochets + u32::from(p.state == State::RicochetHit), before)
            }
            None => (0, 0, Vec::new()),
        };
        let (mut forward_parts, mut characters) = parent.map_or((Vec::new(), Vec::new()), |p| (p.forward_parts.clone(), p.characters.clone()));
        if let Some(body) = parent.and_then(|p| p.hit_body) {
            if parent.is_some_and(|p| p.hit_forward) {
                forward_parts.push(body);
            }
            if !characters.contains(&body.character) {
                characters.push(body.character);
            }
        }
        Bullet {
            shot,
            fragment: m.fragment,
            seed: m.seed,
            depth,
            ricochets,
            before,
            round: round.clone(),
            direction: m.direction,
            initial_speed: m.initial_speed,
            speed: m.speed,
            damage: m.damage,
            penetration: m.penetration,
            penetration_chance: m.penetration_chance,
            ricochet_chance: m.ricochet_chance,
            fragmentation_chance: m.fragmentation_chance,
            deviation_chance: m.deviation_chance,
            min_fragments: m.min_fragments,
            max_fragments: m.max_fragments,
            armor_damage: round.armor_damage,
            life: round.life,
            age: 0.0,
            position: m.origin,
            velocity,
            trajectory: Trajectory::new(m.origin, velocity, round.mass_g, round.diameter_mm, round.ballistic_coefficient, self.gravity),
            state: State::Flying,
            done: false,
            hit_thing: 0,
            hit_collider: 0,
            deflected_by: None,
            blocked_by: None,
            forward_parts,
            characters,
            hit_body: None,
            hit_forward: false,
            no_damage: false,
        }
    }

    /// One frame (`UpdateShots`): every bullet in flight moved on by `dt`,
    /// each hit handled and the bullets it makes moved on by what is left of
    /// the frame. The hits in the order they came.
    pub fn tick(&mut self, dt: f32, world: &mut impl BulletWorld) -> Vec<Hit> {
        self.bullets.retain(|b| !b.finished());
        let mut queue: std::collections::VecDeque<(usize, f32)> = (0..self.bullets.len()).map(|i| (i, dt)).collect();
        let mut hits = Vec::new();
        while let Some((index, time)) = queue.pop_front() {
            let Some((hit, children, left)) = self.update(index, time, &*world) else { continue };
            world.delivered(&hit);
            hits.push(hit);
            for child in children {
                self.bullets.push(child);
                queue.push_back((self.bullets.len() - 1, left));
            }
        }
        hits
    }

    /// `Shot.Tick` and `Update`: the bullet moved on; a hit handled.
    fn update(&mut self, index: usize, dt: f32, world: &impl BulletWorld) -> Option<(Hit, Vec<Bullet>, f32)> {
        let bullet = &mut self.bullets[index];
        if bullet.finished() {
            return None;
        }
        let (previous, previous_velocity) = (bullet.position, bullet.velocity);
        bullet.age += dt;
        (bullet.position, bullet.velocity) = bullet.trajectory.at(bullet.age);
        let asker = Asker { shot: bullet.shot, fragment: bullet.fragment, seed: bullet.seed, forward: true };
        let mut scratch = std::mem::take(&mut self.hits_scratch);
        let found = linecast_both_ways(world, asker, previous, self.bullets[index].position, &self.bullets[index].before, &mut scratch);
        self.hits_scratch = scratch;
        let (hit, forward) = found?;
        Some(self.handle(index, dt, previous, previous_velocity, hit, forward, world))
    }

    /// `HandleCollision` and `CreateFragments`.
    #[allow(clippy::too_many_arguments)]
    fn handle(&mut self, index: usize, dt: f32, previous: V3, previous_velocity: V3, hit: RayHit, forward: bool, world: &impl BulletWorld) -> (Hit, Vec<Bullet>, f32) {
        let degradation = self.degradation;
        let bullet = &mut self.bullets[index];
        let past = magnitude(sub(bullet.position, hit.point));
        let segment = sub(bullet.position, previous);
        let share = past / magnitude(segment);
        let left = dt * share;
        bullet.age -= left;
        let mut speed = magnitude(bullet.velocity);
        let before = magnitude(previous_velocity);
        speed -= (speed - before) * share;
        bullet.velocity = scale(normalized(bullet.velocity), speed);
        bullet.hit_thing = hit.thing;
        bullet.hit_collider = hit.collider;
        bullet.hit_body = hit.body;
        bullet.hit_forward = forward;
        // `AvoidAdditionalDamageWhereCollidersIntersect`.
        if let Some(body) = hit.body {
            bullet.no_damage = bullet.forward_parts.iter().any(|p| p.same_part(&body)) && bullet.characters.contains(&body.character);
        }
        let cos = dot(hit.normal, normalized(segment));
        if forward {
            let r = clamp01(magnitude(bullet.velocity) / bullet.initial_speed);
            let k = r + (1.0 - r) * clamp01(degradation);
            bullet.damage *= k;
            bullet.penetration *= k;
        }
        bullet.done = true;
        let way = normalized(segment);
        let children = self.fragments(index, &hit, forward, cos, way, world);
        let b = &self.bullets[index];
        let record = Hit {
            shot: b.shot,
            fragment: b.fragment,
            seed: b.seed,
            depth: b.depth,
            point: hit.point,
            normal: hit.normal,
            forward,
            thing: hit.thing,
            collider: hit.collider,
            surface: hit.surface,
            state: b.state,
            speed: b.speed,
            velocity: b.velocity,
            damage: b.damage,
            penetration: b.penetration,
            armor_damage: b.armor_damage,
            cos,
            age: b.age,
            fragments: children.len(),
            out_of_life: b.age > b.life,
            first_hit: b.fragment == 0 && b.depth <= 1,
            body: hit.body,
            deflected_by: b.deflected_by,
            blocked_by: b.blocked_by,
            no_damage: b.no_damage,
            direction: b.direction,
            round: b.round.clone(),
        };
        (record, children, left)
    }

    /// What follows a hit (`CreateFragments`): a ricochet, fragments, the
    /// bullet turned on, or nothing.
    fn fragments(&mut self, index: usize, hit: &RayHit, forward: bool, cos: f32, way: V3, world: &impl BulletWorld) -> Vec<Bullet> {
        let draw = self.tables.float(self.bullets[index].seed);
        let back = angle(scale(way, -1.0), hit.normal);
        let body = hit.body;
        let wearer = body.and_then(|r| world.wearer(r.character));
        // Deflected by heavy armour (`IsShotDeflectedByHeavyArmor`).
        let heavy = body.is_some_and(|r| {
            self.bullets[index].ricochets < 2 && back > 42.5 && back < 80.0 && wearer.is_some_and(|w| w.heavy_deflects(r.collider, r.plate, draw))
        });
        // A ricochet (`IsRicocheted`): at a character's collider its armour
        // deflects the bullet, and may block it on the way (`SetShotStatus`).
        let ricochet = heavy
            || (forward
                && match body {
                    Some(r) => {
                        let b = &self.bullets[index];
                        let (mut blocked, mut deflected) = (b.blocked_by, None);
                        if let Some(w) = wearer {
                            deflected = w.shot_status(r.collider, r.plate, back, b.penetration, draw, &mut blocked);
                        }
                        let b = &mut self.bullets[index];
                        b.blocked_by = blocked;
                        if deflected.is_some() {
                            b.deflected_by = deflected;
                        }
                        deflected.is_some()
                    }
                    None => {
                        let (b, s) = (&self.bullets[index], hit.surface);
                        s.ricochet_chance() >= EPSILON && back > 42.5 && back < 80.0 && b.ricochet_chance > 0.0 && (s.ricochet_chance() + b.ricochet_chance) * (1.0 - cos.abs()) > draw
                    }
                }
                && self.bullets[index].ricochets < 2);
        let tables = &self.tables;
        let b = &self.bullets[index];
        let s = hit.surface;
        if ricochet {
            let sum = (b.ricochet_chance + s.ricochet_chance()).min(1.0);
            let mirrored = sub(normalized(b.velocity), scale(hit.normal, 2.0 * cos));
            let turned = if heavy { [0.0; 3] } else { scale(tables.direction(b.seed), s.deviation()) };
            let m = Made {
                fragment: b.fragment + 1,
                seed: fast::next_seed(b.seed),
                origin: hit.point,
                direction: normalized(add(mirrored, turned)),
                initial_speed: b.initial_speed,
                speed: magnitude(b.velocity) * 0.7,
                damage: b.damage * sum * 0.5,
                penetration: b.penetration * sum * 0.5,
                penetration_chance: b.penetration_chance * 0.2,
                ricochet_chance: b.ricochet_chance * 0.2,
                fragmentation_chance: b.fragmentation_chance * 0.2,
                deviation_chance: 1.0,
                min_fragments: b.min_fragments,
                max_fragments: b.max_fragments,
            };
            self.bullets[index].state = State::RicochetHit;
            let b = &self.bullets[index];
            return vec![self.made(b.shot, &b.round, m, Some(b))];
        }
        // Through (`IsTargetPenetrated`): a character's collider always from
        // behind; from before when not blocked and the penetration, worn by
        // the armour over it, is above its level.
        let through = match body {
            Some(_) if !forward => true,
            Some(r) => b.blocked_by.is_none() && b.penetration * wearer.and_then(|w| w.factor(r.collider, r.plate, b.penetration)).unwrap_or(1.0) > s.level(),
            None => s.penetration_chance() >= EPSILON && b.penetration > s.level() && b.penetration_chance + s.penetration_chance() > draw,
        };
        if !through {
            self.bullets[index].state = State::StopHit;
            return Vec::new();
        }
        // A character's collider's share of what goes on, and the armour's
        // wear on each bullet that follows (`ApplyPenetratedDegradation`).
        let share = body.map_or(1.0, |r| ((b.penetration - s.level()) / 100.0 + r.damage_mod + b.round.penetration_damage_mod).clamp(0.0, 1.0));
        let worn = |m: &mut Made| {
            if let (true, Some(r), Some(w)) = (forward, body, wearer) {
                if let Some(factor) = w.factor(r.collider, r.plate, m.penetration) {
                    m.damage *= factor;
                    m.penetration *= factor;
                }
            }
        };
        let in_pieces = forward && b.fragmentation_chance + s.fragmentation_chance() > draw && s.fragmentation_chance() >= EPSILON && b.fragmentation_chance >= EPSILON;
        if in_pieces {
            self.bullets[index].state = State::FragmentationHit;
            let b = &self.bullets[index];
            let count = fast::int(b.seed, b.min_fragments, b.max_fragments - b.min_fragments);
            if count <= 0 {
                return Vec::new();
            }
            let most = b.max_fragments as f32;
            let damage = b.damage * share * 0.5 / most;
            let penetration = b.penetration * share * 0.5 / most;
            let origin = add(hit.point, scale(b.direction, 0.002));
            let along = normalized(b.velocity);
            let speed = magnitude(b.velocity) * (1.0f32).min(1.0 / most * 0.7);
            let mut pick = fast::next_seed(b.seed);
            let mut out = Vec::with_capacity(count as usize);
            for _ in 0..count {
                let direction = normalized(add(along, scale(tables.direction(pick), s.deviation())));
                pick = fast::next_seed(pick);
                let mut m = Made {
                    fragment: b.fragment + 1,
                    seed: pick,
                    origin,
                    direction,
                    initial_speed: b.initial_speed,
                    speed,
                    damage,
                    penetration,
                    penetration_chance: b.penetration_chance * 0.2,
                    ricochet_chance: b.ricochet_chance * 0.2,
                    fragmentation_chance: 0.0,
                    deviation_chance: 1.0,
                    min_fragments: 0,
                    max_fragments: 0,
                };
                worn(&mut m);
                out.push(self.made(b.shot, &b.round, m, Some(b)));
            }
            return out;
        }
        if b.deviation_chance < EPSILON || (s.material == material::CHAINFENCE && b.round.explosive) {
            // Through, and nothing follows: it stays flying.
            return Vec::new();
        }
        let mut turned = normalized(b.velocity);
        if forward && s.deviation_chance() > draw {
            turned = add(turned, scale(tables.direction(b.seed), s.deviation()));
        }
        // From before at a character's collider: its share of the damage and
        // penetration, 0.8 to 1 of the speed and 0.2 to 1 of the chances by
        // its penetration chance.
        let (share, keep_speed, keep_chances) = if forward && body.is_some() {
            let p = s.penetration_chance();
            (share, 0.8 + 0.2 * p, 0.2 + 0.8 * p)
        } else {
            (1.0, 1.0, 1.0)
        };
        let mut m = Made {
            fragment: b.fragment,
            seed: fast::next_seed(b.seed),
            origin: add(hit.point, scale(turned, 0.002)),
            direction: turned,
            initial_speed: b.initial_speed,
            speed: magnitude(b.velocity) * keep_speed,
            damage: b.damage * share,
            penetration: b.penetration * share,
            penetration_chance: b.penetration_chance * keep_chances,
            ricochet_chance: b.ricochet_chance * keep_chances,
            fragmentation_chance: b.fragmentation_chance * keep_chances,
            deviation_chance: b.deviation_chance - 0.08,
            min_fragments: b.min_fragments,
            max_fragments: b.max_fragments,
        };
        worn(&mut m);
        self.bullets[index].state = State::DeviationHit;
        let b = &self.bullets[index];
        vec![self.made(b.shot, &b.round, m, Some(b))]
    }
}

/// What a bullet is made with.
struct Made {
    fragment: i32,
    seed: i32,
    origin: V3,
    direction: V3,
    initial_speed: f32,
    speed: f32,
    damage: f32,
    penetration: f32,
    penetration_chance: f32,
    ricochet_chance: f32,
    fragmentation_chance: f32,
    deviation_chance: f32,
    min_fragments: i32,
    max_fragments: i32,
}

/// Unity's `Linecast` of the game's (`PhysicsExtensions.Linecast`): of the
/// ray's hits not ignored, the nearest (or, `reverse`, the farthest).
fn linecast(world: &impl BulletWorld, asker: Asker, from: V3, to: V3, reverse: bool, before: &[(u64, u64)], hits: &mut Vec<RayHit>) -> Option<RayHit> {
    hits.clear();
    world.ray(asker, from, to, hits);
    let reach = magnitude(sub(to, from));
    let mut best: Option<RayHit> = None;
    for hit in hits.iter().filter(|h| h.distance <= reach) {
        // `IsIgnoreSameCollider`: another collider of a thing one before
        // it hit.
        if before.iter().any(|&(thing, collider)| thing == hit.thing && hit.collider != collider) {
            continue;
        }
        let better = match best {
            None => true,
            Some(b) => if reverse { hit.distance > b.distance } else { hit.distance < b.distance },
        };
        if better {
            best = Some(*hit);
        }
    }
    best
}

/// `LinecastInBothSides`: forward from the segment's start, backward from
/// its end (through a mesh up to eight times); the nearer to the start, and
/// whether it was the forward one.
fn linecast_both_ways(world: &impl BulletWorld, asker: Asker, start: V3, end: V3, before: &[(u64, u64)], hits: &mut Vec<RayHit>) -> Option<(RayHit, bool)> {
    let ahead = linecast(world, asker, start, end, false, before, hits);
    let asker = Asker { forward: false, ..asker };
    let mut behind: Option<RayHit> = None;
    let mut from = end;
    let mut nudge = [0.0; 3];
    for i in 0..8 {
        let Some(found) = linecast(world, asker, from, start, true, before, hits) else { break };
        behind = Some(found);
        if !found.mesh {
            break;
        }
        if i == 0 {
            nudge = scale(normalized(sub(start, end)), 0.001);
        }
        from = add(found.point, nudge);
    }
    match (ahead, behind) {
        (Some(a), Some(b)) => {
            if a.distance < magnitude(sub(b.point, start)) {
                Some((a, true))
            } else {
                Some((b, false))
            }
        }
        (Some(a), None) => Some((a, true)),
        (None, Some(b)) => Some((b, false)),
        (None, None) => None,
    }
}

/// A weapon's figures for its shots, from its build (ballistics spec, "A
/// shot leaves the weapon"): its centre of impact (its barrel's, else its
/// own, times one less its parts' accuracy over 100), its speed factor (one
/// plus its own and its parts' velocity over 100), its shotgun dispersion
/// (its barrel's or its own, times the same accuracy), its iron sight range
/// and its default round.
#[derive(Debug, Clone, PartialEq)]
pub struct WeaponFigures {
    pub centre_of_impact: f32,
    pub speed_factor: f32,
    pub shotgun_dispersion: f32,
    pub iron_sight_range: i32,
    pub default_round: Option<String>,
    /// The barrel's deviation by durability (`DeviationCurve`,
    /// `DeviationMax`): its barrel part's, else the weapon's own; a maximum
    /// of nought is 100.
    pub deviation_curve: f32,
    pub deviation_max: f32,
}

impl WeaponFigures {
    /// From the weapon's template and its parts' (every part in its build,
    /// in the order its slots hold them, depth first).
    pub fn of(db: &trp_data::Database, weapon: &str, parts: &[&str]) -> Option<Self> {
        let template = db.items.get(weapon)?;
        let part_templates: Vec<&trp_data::database::ItemTemplate> = parts.iter().filter_map(|p| db.items.get(*p)).collect();
        let barrel = part_templates.iter().find(|t| t.props.get("CenterOfImpact").is_some_and(|v| v.is_number()));
        let base = barrel.and_then(|b| b.prop_f32("CenterOfImpact")).or_else(|| template.prop_f32("CenterOfImpact")).unwrap_or(0.0);
        let accuracy: f32 = part_templates.iter().map(|t| t.prop_f32("Accuracy").unwrap_or(0.0)).sum();
        let delta = -accuracy / 100.0;
        let velocity: f32 = part_templates.iter().map(|t| t.prop_f32("Velocity").unwrap_or(0.0)).sum::<f32>() + template.prop_f32("Velocity").unwrap_or(0.0);
        let dispersion = barrel.and_then(|b| b.prop_f32("ShotgunDispersion")).or_else(|| template.prop_f32("ShotgunDispersion")).unwrap_or(0.0);
        let of_barrel = |name: &str| barrel.and_then(|b| b.prop_f32(name)).or_else(|| template.prop_f32(name));
        let deviation_max = of_barrel("DeviationMax").filter(|max| *max != 0.0).unwrap_or(100.0);
        Some(Self {
            centre_of_impact: base * (1.0 + delta),
            speed_factor: 1.0 + velocity / 100.0,
            shotgun_dispersion: dispersion * (1.0 + delta),
            iron_sight_range: template.props.get("IronSightRange").and_then(|v| v.as_f64()).unwrap_or(0.0) as i32,
            default_round: template.props.get("defAmmo").and_then(|v| v.as_str()).map(str::to_owned),
            deviation_curve: of_barrel("DeviationCurve").unwrap_or(1.0),
            deviation_max,
        })
    }

    /// The barrel's deviation at a durability (`Weapon.GetBarrelDeviation`):
    /// a quadratic Bézier from `DeviationMax` at nought through
    /// `DeviationCurve` to 1 at 100, found for its parameter by the
    /// durability (the repair buff's `WeaponSpread`, 1 without one, not
    /// built).
    pub fn barrel_deviation(&self, durability: f32) -> f32 {
        let curve = self.deviation_curve;
        let twice = 2.0 * curve;
        let t = if 100.0 - twice == 0.0 { durability / twice } else { (-curve + ((100.0 - twice) * durability + curve * curve).sqrt()) / (100.0 - twice) };
        let u = 1.0 - t;
        u * u * self.deviation_max + 2.0 * t * u * curve + t * t
    }

    /// The spread a shot is given (`Shot`): the centre of impact times the
    /// round's factor, the overheat's multiplier, the barrel's deviation
    /// and the repair buff's spread (each 1 for a new weapon without one).
    pub fn spread(&self, round: &Round) -> f32 {
        self.centre_of_impact * round.ammo_factor()
    }

    /// The same with the weapon's wear and heat: its durability's barrel
    /// deviation and the overheat's multiplier (`heat`: 1 below
    /// `OverheatProblemsStart`; `crate::malfunction::spread_factor`).
    pub fn spread_worn(&self, round: &Round, durability: f32, heat: f32) -> f32 {
        self.spread(round) * heat * self.barrel_deviation(durability)
    }
}

/// The local shot direction for iron sights (`CalculateShotDirectionForIron`):
/// `point` the place on the sights' line `range` out, less the muzzle, in
/// the weapon's own bone (`method_14`); the default round flown along x at
/// its speed times the weapon's, and its height where its distance, then
/// its x, first passes the range (the second search over a table that, the
/// flight restarted, holds each step one on: the game's).
pub fn iron_shot_direction(point: V3, round: &Round, speed_factor: f32, range: i32, gravity: V3) -> V3 {
    let speed = round.initial_speed * speed_factor;
    let mut flight = Trajectory::new([0.0; 3], [speed, 0.0, 0.0], round.mass_g, round.diameter_mm, round.ballistic_coefficient, gravity);
    let range = range as f32;
    let search = |flight: &mut Trajectory, shift: usize, by: &dyn Fn(V3) -> f32| -> Option<f32> {
        let (mut low, mut high) = (0i64, STEPS as i64 - 1);
        while low <= high {
            let mid = (low + high) / 2;
            if mid as usize + 1 + shift > STEPS {
                return None;
            }
            let a = flight.step(mid as usize + shift).position;
            let b = flight.step(mid as usize + 1 + shift).position;
            if by(a) < range && by(b) > range {
                return Some(a[1]);
            }
            if by(a) > range && by(b) > range {
                high = mid - 1;
            } else {
                low = mid + 1;
            }
        }
        None
    };
    let mut drop = search(&mut flight, 0, &|p| magnitude(p)).unwrap_or(0.0);
    if let Some(again) = search(&mut flight, 1, &|p| p[0]) {
        drop = again;
    }
    [0.0, -1.0, (point[1] - drop) / point[2]]
}

/// The shot's start and way moved for the wide field of view's squeeze
/// (`AdjustShotVectors`): each put into the hands' root's axes (`at`,
/// `turn`), its forward figure times the squeeze, and put back; the way made
/// a unit again.
pub fn squeezed(position: V3, direction: V3, at: V3, turn: [f32; 4], squeeze: f32) -> (V3, V3) {
    let back = [-turn[0], -turn[1], -turn[2], turn[3]];
    let mut p = crate::math::quat_rotate(back, sub(position, at));
    let mut d = crate::math::quat_rotate(back, direction);
    p[2] *= squeeze;
    d[2] *= squeeze;
    (add(at, crate::math::quat_rotate(turn, p)), normalized(crate::math::quat_rotate(turn, d)))
}

/// A shot's way with the weapon's spread (`Shot`): the way times 100 plus a
/// point within a ball (Unity's own generator) times the spread, made a unit.
pub fn spread(direction: V3, amount: f32, random: &mut UnityRandom) -> V3 {
    let ball = random.inside_unit_sphere();
    normalized(add(scale(direction, 100.0), scale(ball, amount)))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The barrel's deviation by durability (`GetBarrelDeviation`): 1 new,
    /// its most worn out, between the two along the curve (the AKM's
    /// `DeviationCurve` 1.35 and `DeviationMax` 23).
    #[test]
    fn the_barrel_s_deviation_grows_as_the_weapon_wears() {
        let akm = WeaponFigures { centre_of_impact: 0.1, speed_factor: 1.0, shotgun_dispersion: 0.0, iron_sight_range: 100, default_round: None, deviation_curve: 1.35, deviation_max: 23.0 };
        assert!((akm.barrel_deviation(100.0) - 1.0).abs() < 1e-5);
        assert!((akm.barrel_deviation(0.0) - 23.0).abs() < 1e-4);
        let (half, worn) = (akm.barrel_deviation(50.0), akm.barrel_deviation(20.0));
        assert!(half > 1.0 && worn > half && worn < 23.0, "{half} {worn}");
    }

    fn m882() -> Round {
        Round {
            initial_speed: 385.0,
            mass_g: 5.4,
            diameter_mm: 9.0,
            ballistic_coefficient: 0.068,
            damage: 56.0,
            penetration_power: 18.0,
            penetration_deviation: 0.8,
            penetration_chance: 0.2,
            ricochet_chance: 0.1,
            fragmentation_chance: 0.2,
            min_fragments: 1,
            max_fragments: 3,
            armor_damage: 0.35,
            life: 5.0,
            accuracy: 0,
            projectiles: 1,
            penetration_damage_mod: 0.0,
            stamina_burn: 0.0,
            heavy_bleeding: 0.0,
            light_bleeding: 0.0,
            explosive: false,
        }
    }

    #[test]
    fn the_g1_table_is_drawn_between_its_places() {
        assert_eq!(g1_drag(10.0), 0.0);
        // The list's places are taken as if 0.05 Mach apart, which from
        // Mach 0.6 they are not: at Mach 1 the game draws on through its
        // places 19 and 20 (Mach 0.85 and 0.875), not the table's 0.4805.
        let at_one = g1_drag(343.0);
        assert!((at_one - 0.4311).abs() < 1e-3, "{at_one}");
        // Below Mach 0.6 the places are what they say.
        assert!((g1_drag(0.3 * 343.0) - 0.2214).abs() < 1e-4);
        assert_eq!(g1_drag(10_000.0), 0.4988);
    }

    #[test]
    fn a_flight_slows_and_falls() {
        let mut t = Trajectory::new([0.0; 3], [0.0, 0.0, 378.301], 5.4, 9.0, 0.068, [0.0, -9.81, 0.0]);
        let (at, v) = t.at(1.0);
        assert!(at[2] > 200.0 && at[2] < 378.0, "{at:?}");
        assert!(at[1] < -3.0, "{at:?}");
        assert!(magnitude(v) < 378.0);
    }

    /// Recorded (`courses3`, `2-far`): an M882 from the MP-443 left at
    /// 378.301 m/s; 0.015 s on it was 5.5714 m out (frame 118).
    #[test]
    fn the_recorded_flight_is_flown() {
        let start = [-1.734_767_9, 1.358_049_4, -0.858_825_7];
        let velocity = [-361.611_18, -30.546_637, 106.845_17];
        let mut t = Trajectory::new(start, velocity, 5.4, 9.0, 0.068, [0.0, -9.81, 0.0]);
        let (at, v) = t.at(0.015);
        let recorded = [-7.06057, 0.906_938_1, 0.714_782_7];
        let off = magnitude(sub(at, recorded));
        assert!(off < 1e-3, "{at:?} is {off} m off");
        let recorded_v = [-353.515_38, -29.960_854, 104.4531];
        assert!(magnitude(sub(v, recorded_v)) < 0.05, "{v:?}");
    }

    struct Wall;
    impl BulletWorld for Wall {
        fn ray(&self, _: Asker, from: V3, to: V3, hits: &mut Vec<RayHit>) {
            // A plane at z = 10 facing back, thick concrete.
            let surface = Surface { material: material::CONCRETE, figures: [100.0, 0.0, 0.41, 0.16, 0.23, 0.037] };
            let way = sub(to, from);
            if from[2] < 10.0 && to[2] >= 10.0 {
                let t = (10.0 - from[2]) / way[2];
                let point = add(from, scale(way, t));
                hits.push(RayHit { distance: t * magnitude(way), point, normal: [0.0, 0.0, -1.0], collider: 1, thing: 1, mesh: false, surface, body: None });
            }
        }
    }

    #[test]
    fn a_bullet_stops_in_concrete_head_on() {
        let mut b = Ballistics::new(0.5, [0.0, -9.81, 0.0]);
        let round = std::sync::Arc::new(m882());
        b.create(1, &round, 0, 15, [0.0, 0.0, 0.0], [0.0, 0.0, 1.0], 1.0);
        let mut hits = Vec::new();
        for _ in 0..10 {
            hits.extend(b.tick(1.0 / 60.0, &mut Wall));
        }
        assert_eq!(hits.len(), 1);
        let hit = &hits[0];
        assert!((hit.point[2] - 10.0).abs() < 1e-4);
        // Head on there is no ricochet; concrete has no penetration chance.
        assert_eq!(hit.state, State::StopHit);
        assert!(hit.cos < -0.99);
        assert!(hit.damage < 56.0 && hit.damage > 50.0);
    }
}
