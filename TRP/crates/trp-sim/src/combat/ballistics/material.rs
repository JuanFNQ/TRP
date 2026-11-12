//! The numbers and names of the materials a bullet meets.
//! After the game's `MaterialType`.

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
