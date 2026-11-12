//! Where dummies stand, the crowd, their build, lights and look.

use super::*;

/// Their colliders' numbers as ballistic things, apart from the range's;
/// each dummy's from its own base.
pub(super) const THINGS: u64 = 1 << 40;

pub(super) const THINGS_EACH: u64 = 1 << 16;

/// The size a dummy's whole thing is taken at where the levels of detail
/// do not say (`lod_view::whole_size`, off): its body's group's, as the
/// install has every body's (third-person spec, "Levels of detail").
pub(super) const CHARACTER_SIZE: f32 = 2.0;

/// The room round a view a character counts as in it by, that it is posed
/// before its first drawn frame as it comes in (`run_dummies`): a metre,
/// and a share of its distance (some 6 degrees). `ESTIMATE`, this work's.
pub(super) const VIEW_MARGIN: f32 = 1.0;

pub(super) const VIEW_MARGIN_SHARE: f32 = 0.1;

/// Where each stands (Bevy's axes), and whether it walks: down the lanes,
/// 9.4 m past the knee-high wall (at z −5.6), facing it; the walker 3.8 m to
/// the other's right as the shooter sees them (TRP's range). This work's
/// choice on TRP's range.
///
/// `PROVISIONAL`: this work's place and facing on the app's range (the game's
/// is not followed here); its helmet's hinge drawn at the model's rest (its
/// armour's hinged pieces count by their switches:
/// `worn_of`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DummyPlace {
    pub at: Vec3,
    pub walks: bool,
}

/// (Nearer the firing line and to its left than they first stood, 20 m out
/// at 3.25 and 7.25.)
pub const PLACES: [DummyPlace; 2] = [DummyPlace { at: Vec3::new(-2.5, 0.0, -15.0), walks: false }, DummyPlace { at: Vec3::new(1.3, 0.0, -15.0), walks: true }];

/// The crowd's places, where the run's start is `crowd-<n>` (`set_crowd`).
static CROWD: std::sync::OnceLock<Vec<DummyPlace>> = std::sync::OnceLock::new();

/// Every dummy's place, by its number: the two (`PLACES`), or the crowd's.
pub fn places() -> &'static [DummyPlace] {
    CROWD.get().map_or(&PLACES[..], Vec::as_slice)
}

/// The range's crowd (`--start crowd-<n>`: a scene of many characters for
/// the posing economy's timings): `n` dummies (two at least), set once as the
/// run begins, before any is made.
pub fn set_crowd(n: usize) {
    let _ = CROWD.set(crowd(n));
}

/// The crowd a start names (`crowd-<n>`, two to a thousand), none else.
pub fn crowd_of(start: &str) -> Option<usize> {
    start.strip_prefix("crowd-")?.parse::<usize>().ok().filter(|n| (2..=1000).contains(n))
}

/// The crowd's `n` places (this work's, on TRP's range): the two
/// dummies where they stand, and the rest standing in rows of 13, 1.4 m
/// apart (every other row by half that), in turn one in eight behind the
/// crowd start (between it and the knee-high wall: out of view, within
/// the sun's shadow reach), two in eight near (from 18 m down the lanes,
/// rows 3 m apart), three in eight far (from 100 m, rows 12 m apart: under
/// 32 px tall at 1080 lines and 50 degrees from 72 m), two in eight off to
/// the left on the open floor (70 to 87 m off, out of view and past the
/// shadows' reach), each band's rows filled in turn; the near band takes
/// what the one behind has no room for.
pub fn crowd(n: usize) -> Vec<DummyPlace> {
    const ACROSS: usize = 13;
    let mut out: Vec<DummyPlace> = PLACES.iter().copied().take(n.max(2)).collect();
    let mut filled = [0usize; 4];
    for i in 0..n.saturating_sub(2) {
        let mut band = match i % 8 {
            0 => 0,
            1 | 2 => 1,
            3..=5 => 2,
            _ => 3,
        };
        // Behind the start, three rows at most.
        if band == 0 && filled[0] >= 3 * ACROSS {
            band = 1;
        }
        let slot = filled[band];
        filled[band] += 1;
        let (row, column) = (slot / ACROSS, slot % ACROSS);
        let across = 1.4 * column as f32 + if row % 2 == 1 { 0.7 } else { 0.0 };
        let at = match band {
            0 => Vec3::new(-8.5 + across, 0.0, -10.0 + 1.5 * row as f32),
            1 => Vec3::new(-8.5 + across, 0.0, -18.0 - 3.0 * row as f32),
            2 => Vec3::new(-8.5 + across, 0.0, -100.0 - 12.0 * row as f32),
            _ => Vec3::new(-70.0 - across, 0.0, -20.0 - 3.0 * row as f32),
        };
        out.push(DummyPlace { at, walks: false });
    }
    out
}

/// How far either way of its place the walker goes before its keys turn
/// back (this work's): it coasts a little past as its walk slows.
pub(super) const WALK_SPAN: f32 = 2.0;

/// The dummies' skills: every one at level 1 (TRP's choice: level 1 where
/// the game's rules need a level; body hits spec, "The dummy on the range"),
/// with `SKILLS` on; untrained with it off, as the player's buffs are.
pub const SKILL_LEVEL: u32 = 1;

/// The way they face: the game's yaw, half round (toward the firing line).
pub(super) const FACING: f32 = 180.0;

/// The weapon in their hands: the AKM, the run's default.
pub const WEAPON: &str = "59d6088586f774275f37482f";

/// The light on it (TRP's choice: the dummies' guns get a light, so that a
/// light shines at the camera: tactical-devices spec,
/// "The light shaft and the flare"): the AKM's stock build with its wooden
/// handguard swapped for the Zenit B-10M and, in that one's `mod_tactical`,
/// the Zenit Klesch-2U (a white light with its flares; the B-10M's slot
/// takes it, `probe item 5c617a5f2e2216000f1e81b3`). This work's choice of
/// parts on TRP's range.
pub const HANDGUARD: &str = "5c617a5f2e2216000f1e81b3";

pub const LIGHT: &str = "5b3a337e5acfc4704b4a19a0";

/// The dummies' weapon's build: the AKM's stock build, its handguard the
/// B-10M with the Klesch-2U on it (`HANDGUARD`, `LIGHT`).
pub fn build(db: &trp_data::Database) -> anyhow::Result<trp_data::setup::database::Preset> {
    let mut build = db.default_preset(WEAPON)?.clone();
    let handguard = build.items.iter_mut().find(|item| item.slot.as_deref() == Some("mod_handguard")).ok_or_else(|| anyhow::anyhow!("the AKM's build has no handguard"))?;
    handguard.template = HANDGUARD.to_owned();
    let under = handguard.id.clone();
    build.items.push(trp_data::setup::database::PresetItem { id: format!("{under}_light"), template: LIGHT.to_owned(), parent: Some(under), slot: Some("mod_tactical".to_owned()) });
    build.id = format!("{}_dummy", build.id);
    build.name = "AKM, the dummies' (B-10M, Klesch-2U)".to_owned();
    Ok(build)
}

/// The dummies' weapon lights, on or off (the debug panel's `LIGHTS`; off
/// as the run begins): their devices' first mode as the game's item keeps
/// it (`LightComponent`), the light's model, its light and its flares.
#[derive(Resource, Default)]
pub struct DummyLights {
    pub on: bool,
}

/// The debug panel's button switches the dummies' lights.
pub fn switch_dummy_lights(mut lights: ResMut<DummyLights>, mut panel: MessageReader<crate::platform::debug_menu::DebugCommand>) {
    for _ in panel.read().filter(|command| **command == crate::platform::debug_menu::DebugCommand::DummyLights) {
        lights.on = !lights.on;
    }
}

/// Each dummy's side, by its number (TRP's choice, from the PMCs' starting
/// kits: the one that stands a USEC, the walker a BEAR). Its kit
/// is a new character's of that side and of `KIT_EDITION` (`profiles.json`),
/// its look a PMC bot's of that side as SPT's server dresses one
/// (`bots/types/<side>.json`, `BotGenerator.SetBotAppearance`).
pub const SIDES: [&str; 2] = ["usec", "bear"];

pub const KIT_EDITION: &str = "Standard";

/// A dummy's place on the range, facing the firing line.
pub fn place(which: usize) -> Transform {
    Transform::from_translation(places()[which].at).with_rotation(Quat::from_rotation_y(std::f32::consts::PI))
}

/// A piece of a dummy, drawn with the body's meshes (or its own suit's and
/// head's): its dummy's number.
#[derive(Component)]
pub struct DummyTwin {
    pub which: usize,
}

/// What a piece of a dummy is shown by, on the dummy's own gear as the
/// body's pieces are on the character's (`show_dummies`).
#[derive(Component, Clone, Debug)]
pub struct DummyShows(pub Shows);

#[derive(Clone, Debug, PartialEq)]
pub enum Shows {
    /// A worn thing's mesh for what it is over (any, if the list is
    /// empty), or a model on the head as a whole (`body_view::show_worn`).
    Worn { item: String, over: Vec<trp_data::character::dress::Under> },
    /// A part a model on the head may have fitted: the worn thing and the
    /// slots and templates down to it (`body_view::show_head_parts`).
    Head { item: String, path: Vec<(String, String)> },
    /// Its own body, its top or not, and of a torso of the newer kind the
    /// mesh for one case of what is worn (`show_outfit`, `show_torso`).
    Own { top: bool, torso: Option<trp_data::character::arms::Torso> },
    /// An outfit's, not the install's, or one of the install's tops worn
    /// as one.
    Outfit { item: String },
}

/// What a dummy has on, as its pieces are shown by: the things in the
/// slots the game draws, by template; what is worn over armour or a vest
/// takes its mesh by; the torso's mesh; the outfit with a body or a top
/// to draw (its item, a body, a top); and every way down from a thing
/// worn to a part fitted to it.
#[derive(Debug, Default)]
pub struct Look {
    pub on: Vec<String>,
    pub under: trp_data::character::dress::Under,
    pub torso: Option<trp_data::character::arms::Torso>,
    pub outfit: Option<(String, bool, bool)>,
    pub fitted: Vec<(String, Vec<(String, String)>)>,
}

impl Look {
    pub fn of(parts: &crate::player::body_view::BodyParts, gear: &trp_sim::items::inventory::Item) -> Self {
        let worn: Vec<&trp_sim::items::inventory::Item> = trp_sim::items::inventory::WORN.iter().filter_map(|slot| gear.in_slot(slot)).collect();
        let on: Vec<&str> = worn.iter().map(|item| item.kind.template.as_str()).collect();
        let mut fitted = Vec::new();
        for item in &worn {
            let mut found = Vec::new();
            ways_down(item, &mut Vec::new(), &mut found);
            fitted.extend(found.into_iter().map(|path| (item.kind.template.clone(), path)));
        }
        Self {
            under: parts.under(&on),
            torso: Some(parts.torso_of(gear)),
            outfit: parts.outfit_drawn(&on).map(|(item, body, top)| (item.to_owned(), body, top)),
            on: on.into_iter().map(str::to_owned).collect(),
            fitted,
        }
    }

    /// Whether a piece shown by that is drawn.
    pub fn shows(&self, shows: &Shows) -> bool {
        match shows {
            Shows::Worn { item, over } => self.on.contains(item) && (over.is_empty() || over.contains(&self.under)),
            Shows::Head { item, path } => self.fitted.iter().any(|(on, way)| on == item && way == path),
            Shows::Own { top, torso } => {
                let covered = self.outfit.as_ref().is_some_and(|(_, body, outfit_top)| *body || (*outfit_top && *top));
                !covered && torso.is_none_or(|case| Some(case) == self.torso)
            }
            Shows::Outfit { item } => self.outfit.as_ref().is_some_and(|(on, ..)| on == item),
        }
    }
}

/// Every way down from a thing to a part fitted to it, by slot and
/// template, each before the ways to the parts fitted to that part.
fn ways_down(item: &trp_sim::items::inventory::Item, above: &mut Vec<(String, String)>, found: &mut Vec<Vec<(String, String)>>) {
    for (holder, held) in item.kind.slots.iter().zip(&item.slots) {
        let Some(held) = held else { continue };
        above.push((holder.name.clone(), held.kind.template.clone()));
        found.push(above.clone());
        ways_down(held, above, found);
        above.pop();
    }
}
