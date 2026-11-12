//! The features' switches as one struct.

use super::*;

/// What of the app's systems is on, field by field (settings spec,
/// "Features"). Each `Switch` of the FEATURES page sets the fields it stands
/// for (`Switches::features`); a field alone is set from the command line
/// (`--feature <field>=off`, by its `Part` key). A field of a group is on
/// only while its group is (`effective`). Every field is "restart required":
/// the resource holds the fields as the run began until it ends, and a
/// switch thrown on the tab is kept for the next run.
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct Features {
    /// An outfit's own arms drawn for the first person. Off, the hands are
    /// the install's default arms whatever is worn.
    pub custom_hands: bool,
    /// The animations of the thing in hand drawn on smooth curves through
    /// their clips' samples. Off, they are drawn as the engine has them; the
    /// simulation is the same either way.
    pub smooth_animations: bool,
    /// The body drawn down onto the ground where it floats over it within the
    /// game's ground check (`feet_down`). Off, the body is drawn where the
    /// capsule holds it; only the drawing changes.
    pub grounded_feet: bool,
    /// The group of the muzzle flash, particles, tracers, spent cases,
    /// magazine rounds and the face shield's mask. Off, all six are off.
    pub visual_effects: bool,
    /// A face shield worn down drawn over the first person's view
    /// (`visor_view`). Off, nothing of it is read or drawn.
    pub face_shield_mask: bool,
    /// A face shield's hits marked and drawn as cracks in its mask
    /// (`shield_marks`, `visor_view`). Off, nothing is marked, read or drawn.
    pub face_shield_cracks: bool,
    /// A fired round's case thrown from the shell port, flying and bouncing
    /// (`cases`). Off, nothing of it is read or drawn.
    pub spent_cases: bool,
    /// A magazine's rounds drawn in its model, in the hands, on the ground
    /// and in its picture (`magazine_rounds`). Off, none is drawn.
    pub magazine_rounds: bool,
    /// A shot's muzzle flash and the gun smoke's trail (`muzzle_view`). Off,
    /// nothing of them is read or drawn.
    pub muzzle_flash: bool,
    /// Unity's particle systems: the muzzles' fumes, a launcher's blasts, a
    /// grenade's explosion and scorch. Off, nothing of them is read or drawn.
    pub particles: bool,
    /// A tracer round's trace (`tracer_view`). Off, nothing of it is read or
    /// drawn; it also needs `ballistics`.
    pub tracers: bool,
    /// The range lit by Customs' sun, ambient and sky by the game's time of
    /// day, with its exposure and tonemap (`lighting`, `prism_view`). Off, the
    /// range's own light and Bevy's tonemapper.
    pub customs_light: bool,
    /// The level camera's own colour filter after the tonemap, with
    /// `customs_light` (`color_filter_view`). Off, nothing of it is read or
    /// drawn.
    pub camera_color_filter: bool,
    /// The camera's sharpening and desaturation, with `customs_light`
    /// (`sharpen_view`). Off, nothing of it is read or drawn.
    pub camera_sharpening: bool,
    /// The sky's light scattered over the scene by distance and height, with
    /// `customs_light` (`scattering_view`). Off, nothing of it is read or drawn.
    pub sky_scattering: bool,
    /// Customs' cloud layer over the sky and its shadows in the sun's cookie,
    /// with `customs_light` (`clouds_view`). Off, nothing of it is read or drawn.
    pub customs_clouds: bool,
    /// The ambient drawn again over the first-person arms and weapon outdoors,
    /// with `customs_light` (`highlight_view`). Off, nothing of it is drawn.
    pub ambient_highlight: bool,
    /// The scene's surfaces lit by the game's deferred light and ambient pass,
    /// with `customs_light` (`shading`). Off, Bevy's own shading.
    pub tarkov_shading: bool,
    /// The game's ambient occlusion at the GRAPHICS page's SSAO setting
    /// (`hbao_view`). Off, nothing of it is drawn and there is no normal prepass.
    pub ambient_occlusion: bool,
    /// The hall's ceiling lamps as spot lights, with `customs_light`
    /// (`lighting::hall_lamp`). Off, the hall is lit by its windows alone.
    pub hall_lamps: bool,
    /// The inventory's pictures drawn from the items' models and kept
    /// (`icons`). Off, none is drawn: each box has its sort's plain shape.
    pub icon_generation: bool,
    /// What goes with the thing under the mouse or carried lit orange
    /// (`inventory_view::light_related`). Off, only the box under the mouse is lit.
    pub compatible_highlight: bool,
    /// The character's own model behind the gear page's slots
    /// (`figure_view`). Off, a paper figure of plain shapes.
    pub character_model: bool,
    /// The inventory's screens drawn as the game's own where they are built so
    /// (`health_view`). Off, the app's own look throughout.
    pub tarkov_ui: bool,
    /// The simulation's systems (`trp_sim::Systems`) and the app's own: off,
    /// the weapon's keys are not heard, the first-person body is not drawn, the
    /// inventory opens on the options alone, and nothing in the world is worked.
    pub movement: bool,
    pub vaulting: bool,
    /// The group of the stamina drain and exhaustion. Off, both are off
    /// whatever their own switches say.
    pub stamina: bool,
    /// The stamina pools drain and refill. Off, nothing is paid for.
    pub stamina_drain: bool,
    pub weight: bool,
    pub exhaustion: bool,
    pub weapon_motion: bool,
    pub recoil: bool,
    pub weapon_handling: bool,
    pub first_person_body: bool,
    /// The weapons not in hand hung on the body, seen from outside
    /// (`slung_view`). Off, none is read or drawn.
    pub weapons_on_body: bool,
    /// The watch on the first-person wrist at the computer's local time
    /// (`watch_view`). Off, none is read or drawn.
    pub wrist_watch: bool,
    /// The head's things fitted to one another, and a suit's hood by the
    /// headwear (`body_view::fit_head_things`). Off, each keeps its prefab's mesh.
    pub head_fitting: bool,
    /// An armband worn drawn skinned to the body (`body_view::fit_armbands`).
    /// Off, no armband is read or drawn.
    pub armband: bool,
    /// The OVERALL tab's figure posed and lit as the game's menu figure
    /// (`figure_view::Overall`). Off, the gear page's still figure; needs
    /// `character_model`.
    pub overall_figure: bool,
    /// The characters drawn by their levels of detail, a whole character
    /// culled under a pixel (`lod_view`). Off, the fullest level at any
    /// distance and nothing culled.
    pub distance_detail: bool,
    pub inventory: bool,
    pub world_interactions: bool,
    /// The body's parts and what harms and heals them. Off, the character
    /// cannot be hurt and is taken as fed and watered.
    pub health: bool,
    /// Each skill's level changes what the game's code says it changes
    /// (`skills`). Off, every skill counts as untrained.
    pub skills: bool,
    /// Each weapon family's mastering level read by its weapons, and the
    /// MASTERING page (`skills::Mastery`). Off, every weapon is at level 0.
    pub weapon_mastering: bool,
    /// What the character does earns skill points, mastering and experience
    /// (`experience`). Off, nothing is earned.
    pub experience: bool,
    /// Every experience award at its whole worth, without fatigue or falling
    /// shares; the app's own departure from the game. Off, the game's rates.
    pub simple_xp_gain: bool,
    /// The hand signs and voice phrases, their menu, prompt and binds
    /// (`gestures`). Off, Y and F1 to F12 do nothing.
    pub gestures: bool,
    /// Things put down, dropped and let fall thrown and falling in the
    /// physics engine with their models (`loose`). Off, a thing lies as a box.
    pub item_physics: bool,
    /// The world's things (on the ground, in flight, spent cases, benches,
    /// poppers) drawn by their levels of detail (`lod_view::world_on`). Off,
    /// their fullest level at any distance.
    pub world_distance_detail: bool,
    /// The group of the movement, hands and shot sounds. Off, all three are off.
    pub sounds: bool,
    /// The footsteps and the other movement sounds (`step_sound`). Off, the
    /// banks are not read and nothing plays.
    pub movement_sounds: bool,
    /// What the thing in hand sounds as its animation marks it (`hands_sound`).
    /// Off, the players are not read and nothing plays.
    pub hands_sounds: bool,
    /// A weapon's shots from its own banks (`shot_sound`). Off, the banks are
    /// not read and the shots are silent.
    pub shot_sounds: bool,
    /// What the range's doors, containers and trunks sound (`thing_sound`).
    /// Off, nothing of them is read and nothing plays.
    pub world_sounds: bool,
    /// The headwear's headphones template applied to what is heard
    /// (`hearing`). Off, every sound group stays at its own level.
    pub headgear_hearing: bool,
    /// Grenades thrown and the knife's strokes by their keys. Off, their keys
    /// do nothing and their rigs are not made.
    pub grenades_and_knife: bool,
    /// Each shot's bullets flown and meeting the range's surfaces, with each
    /// hit's effects (`bullets`). Off, a shot makes no bullet.
    pub ballistics: bool,
    /// The PMC dummy on the range with hit colliders, health and armour
    /// (`dummy`). Off, it is not built.
    pub dummy: bool,
    /// The steel poppers down the open start's way (`poppers`). Off, none is
    /// made or read.
    pub reactive_targets: bool,
    /// The dummies posed by their size on screen and their colliders placed
    /// only where something may meet them (`dummy::run_dummies`). Off, every
    /// dummy is posed and placed in full every frame; the results are the same.
    pub posing_economy: bool,
    /// Every weapon made from its parts, and a scope's lens drawn by its optic
    /// camera (`optic_view`). Off, every weapon is its stock build and a lens
    /// is a blank disc.
    pub attachments: bool,
    /// The gun bench and its EDIT BUILD, MODIFY WEAPON and REPAIR GEAR
    /// (`workbench`). Off, it is not there.
    pub workbench: bool,
    /// The armor bench with MODIFY HELMET, MODIFY ARMOR and REPAIR GEAR
    /// (`range::BenchKind`). Off, it is not there.
    pub armor_bench: bool,
    /// The install's in-raid limits on how many of a kind are carried, and the
    /// inspect window's row for them. Off, no limit and no such row.
    pub raid_limits: bool,
    /// A thing of an unknown kind shown as unknown until examined
    /// (`trp_sim::examine`). Off, everything is known.
    pub examining: bool,
    /// The range's crates filled by SPT's tables and searched as a raid's
    /// containers (`trp_sim::search`). Off, the crates are empty.
    pub searching: bool,
    /// The things SPT never hands out left off the tables, cabinets and
    /// benches' lists (`GameData::unfinished`). Off, every item is laid out.
    pub hide_unfinished: bool,
    /// The OVERALL tab's suit dropdowns, each suit read and put on when chosen
    /// (`clothing`). Off, nothing of it is read and the default suits are worn.
    pub clothing: bool,
    /// Weapon lights and lasers and the head light switched by their keys
    /// (`tactical_view`). Off, the keys do nothing and every device shows off.
    pub tactical_devices: bool,
    /// The weapon held over cover or out to the side by `Alt`+`W` and
    /// `Alt`+`S` (`trp_sim::blind_fire`). Off, its keys are not heard.
    pub blind_fire: bool,
    /// The weapon rested on cover by `V` and the bipod put out by `Ctrl`+`V`
    /// (`trp_sim::mounting`). Off, both keys do nothing.
    pub mounting: bool,
    /// The weapon carried on the left shoulder by `Mouse4`
    /// (`trp_sim::left_stance`). Off, its key is not heard.
    pub left_shoulder: bool,
    /// A rifle's underbarrel launcher used by its slot key pressed again
    /// (`trp_sim::firearm::launcher`). Off, the launcher hangs on the rifle as a part.
    pub underbarrel_launcher: bool,
    /// Face shields' and window magazines' glass drawn as dithered glass seen
    /// through (`glass_view`). Off, the glass is drawn opaque.
    pub see_through_glass: bool,
    /// A shot's wear, the roll for a malfunction and the heat each shot
    /// leaves (`trp_sim::malfunction`). Off, each stops: no wear, no
    /// malfunction, no heat.
    pub durability: bool,
    pub malfunctions: bool,
    pub overheating: bool,
}
