//! The Icons resource, the rounds drawn, and the pictures' keys and signatures.

use super::*;

/// A picture being drawn: the model in the studio, its target, how long
/// it has stood there (or, once its reading back is asked, how long that
/// has been waited for), and the reading back asked, if one is: only its
/// answer is taken.
pub(super) struct Drawing {
    pub(super) template: String,
    pub(super) size: [u32; 2],
    pub(super) model: Entity,
    pub(super) target: Handle<Image>,
    pub(super) frames: u32,
    pub(super) wait: u32,
    pub(super) asked: Option<Entity>,
    pub(super) tries: u32,
    /// All of the model is on the graphics card, and how many frames the
    /// old rule had waited for it.
    pub(super) on_card: bool,
    pub(super) old_rule: u32,
}

/// So many frames at the most a picture's reading back is waited for: the
/// engine drops one it cannot make (its target not on the graphics card
/// yet: "Unknown image for screenshot, skipping") and never answers, and
/// the studio had waited on it for ever, every picture after it with it.
/// It is asked again then, as a picture that came out empty is.
pub(super) const ANSWER_FRAMES_AT_MOST: u32 = 120;

/// Every picture asked for, made, being made or to be made.
#[derive(Resource)]
pub struct Icons {
    pub(super) install: trp_data::Install,
    pub(super) db: std::sync::Arc<trp_data::Database>,
    /// The options' switch `ICON GENERATION` as the run began: off, no
    /// picture is drawn or read.
    pub(super) on: bool,
    folder: Option<PathBuf>,
    /// For an item whose model is not the install's own, a word of what
    /// it is drawn from, put into its picture's name in the cache: another
    /// model, another picture (`signatures`).
    signatures: HashMap<String, String>,
    /// Templates asked for already, with their size.
    pub(super) known: HashSet<(String, [u32; 2])>,
    pub(super) queue: VecDeque<(String, [u32; 2])>,
    pub(super) worker: Option<Worker>,
    pub(super) task: Option<Task<(Option<Worker>, (String, [u32; 2]), anyhow::Result<Made>)>>,
    pub(super) ready: VecDeque<PreparedIcon>,
    /// Pictures on their way to the screen away from the frame: read from
    /// the cache (`LoadFromUserCacheAsync`), or drawn and being finished
    /// and kept (`receive_icons`).
    pub(super) arriving: Vec<Task<Arrived>>,
    pub(super) studio: Option<Studio>,
    pub(super) camera: Option<Entity>,
    /// The studio's lights and eye for the game's own surface, with the
    /// camera.
    pub(super) surface: Option<crate::rendering::game_surface::SurfaceView>,
    pub(super) drawing: Option<Drawing>,
    /// How many were drawn, read from the cache, and could not be made.
    pub drawn: usize,
    pub cached: usize,
    pub failed: usize,
    /// Those counts as last told in the log.
    pub(super) reported: (usize, usize, usize),
}

impl Icons {
    pub fn new(install: trp_data::Install, db: std::sync::Arc<trp_data::Database>, on: bool, signatures: HashMap<String, String>) -> Self {
        // `local` beside the workspace, as the options' file is looked for;
        // or the folder `TRP_ICON_CACHE` names (to try a run with a cache of
        // its own, an empty one say, and leave the kept one be).
        let here = std::env::current_dir().unwrap_or_default();
        let cache = cache_folder(crate::effects::glass_view::on());
        let folder = match std::env::var_os("TRP_ICON_CACHE") {
            Some(folder) => Some(PathBuf::from(folder).join(cache)),
            None => here.ancestors().map(|dir| dir.join("local")).find(|local| local.is_dir()).map(|local| local.join("icon_cache").join(cache)),
        };
        if !on {
            println!("the inventory's pictures are not drawn (ICON GENERATION is off)");
        }
        Self {
            install,
            db,
            on,
            folder,
            signatures,
            known: HashSet::new(),
            queue: VecDeque::new(),
            worker: None,
            task: None,
            ready: VecDeque::new(),
            arriving: Vec::new(),
            studio: None,
            camera: None,
            surface: None,
            drawing: None,
            drawn: 0,
            cached: 0,
            failed: 0,
            reported: (0, 0, 0),
        }
    }

    /// Whether any picture is still to be made.
    pub fn busy(&self) -> bool {
        !self.queue.is_empty() || self.task.is_some() || !self.ready.is_empty() || self.drawing.is_some() || !self.arriving.is_empty()
    }

    /// Where the cache keeps a picture, by its key: its template, `folded`
    /// where it is the folded one, a magazine's rounds (how many and a
    /// checksum of their templates), its size, and its signature.
    pub(super) fn path(&self, key: &str, size: [u32; 2]) -> Option<PathBuf> {
        let parts = split_key(key);
        let signed = self.signatures.get(parts.template).map_or(String::new(), |signature| format!("_{signature}"));
        let fold = if parts.folded { "_folded" } else { "" };
        let rounds = if parts.rounds.is_empty() { String::new() } else { format!("_r{}_{:08x}", parts.rounds.len(), fnv(parts.rounds.join(",").as_bytes())) };
        // A build's parts by a checksum of its key (`trp_data::weapon::
        // build_key`), as the rounds are.
        let build = parts.build.map_or(String::new(), |build| format!("_b{}_{:08x}", build.len(), fnv(build.as_bytes())));
        let on = if parts.on.is_empty() { String::new() } else { format!("_s{}_{:08x}", parts.on.len(), fnv(parts.on.join(",").as_bytes())) };
        self.folder.as_ref().map(|folder| folder.join(format!("{}{build}{fold}{rounds}{on}_{}x{}{signed}.png", parts.template, size[0], size[1])))
    }
}

/// FNV-1a over some bytes: a short name for a list of rounds in a file's.
fn fnv(bytes: &[u8]) -> u32 {
    bytes.iter().fold(0x811c_9dc5u32, |hash, &byte| (hash ^ byte as u32).wrapping_mul(0x0100_0193))
}

/// What marks a picture's key as a folded thing's.
const FOLDED: &str = "#folded";

/// What begins a magazine's rounds in a picture's key: their templates
/// from the top, joined by commas.
const ROUNDS: &str = "#rounds:";

/// Whether a magazine's pictures are drawn with its rounds (the options'
/// `MAGAZINE ROUNDS`, set once as the run begins).
static ROUNDS_DRAWN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

/// Sets whether a magazine's pictures have its rounds (`MAGAZINE ROUNDS`).
pub fn draw_rounds(on: bool) {
    ROUNDS_DRAWN.store(on, std::sync::atomic::Ordering::Relaxed);
}

/// What begins, in a picture's key, the places of a thing's parts whose
/// switch is on (`place_of`), joined by commas.
pub(super) const SWITCHED: &str = "#on:";

/// A part's place in a thing, for a picture's key: the slots from the
/// thing down to it, joined by `/`; the thing itself `.`.
pub(super) fn place_of(path: &[(String, String)]) -> String {
    if path.is_empty() {
        return ".".to_owned();
    }
    path.iter().map(|(slot, _)| slot.as_str()).collect::<Vec<_>>().join("/")
}

/// A thing other than a weapon by what its slots hold, as a weapon's
/// build is named (`trp_data::weapon::build_key`); its template where they
/// hold nothing, or where it hides what is in it (`HideEntrails`, which
/// the game's key stops at, `IconsHash.GetAllItemHashes`).
pub(crate) fn parts_key(item: &trp_sim::items::inventory::Item) -> String {
    use trp_data::setup::database::{Preset, PresetItem};
    fn parts(item: &trp_sim::items::inventory::Item, items: &mut Vec<PresetItem>) {
        for (part, slot) in item.slots.iter().zip(&item.kind.slots) {
            let Some(part) = part.as_ref() else { continue };
            items.push(PresetItem { id: part.id.to_string(), template: part.kind.template.clone(), parent: Some(item.id.to_string()), slot: Some(slot.name.clone()) });
            if !part.kind.hide_entrails {
                parts(part, items);
            }
        }
    }
    if item.kind.hide_entrails || item.slots.iter().all(Option::is_none) {
        return item.kind.template.clone();
    }
    let mut items = vec![PresetItem { id: item.id.to_string(), template: item.kind.template.clone(), parent: None, slot: None }];
    parts(item, &mut items);
    trp_data::weapons::weapon::build_key(&Preset { id: item.id.to_string(), name: String::new(), root: item.id.to_string(), items, encyclopedia: None })
}

/// The places of a thing and of its parts whose switch is on
/// (`TogglableComponent.On`, which the game's key has for each,
/// `IconsHash.HashForItem`).
fn switched_on(item: &trp_sim::items::inventory::Item, path: &mut Vec<(String, String)>, on: &mut Vec<String>) {
    if trp_sim::items::inventory::has_switch(item) && item.on {
        on.push(place_of(path));
    }
    if item.kind.hide_entrails {
        return;
    }
    for (part, slot) in item.slots.iter().zip(&item.kind.slots) {
        let Some(part) = part.as_ref() else { continue };
        path.push((slot.name.clone(), part.kind.template.clone()));
        switched_on(part, path, on);
        path.pop();
    }
}

/// What a thing's picture is asked for, kept and found by: its template
/// (a weapon built otherwise than its stock build, its build's key:
/// `trp_sim::inventory::rig_key`; anything else with parts in its slots,
/// those parts, `parts_key`: the game's key has every part of a thing,
/// `IconsHash`), marked where its fold is folded (a folded weapon's
/// picture is its own: drawn to its folded size, its fold's bones turned),
/// with a magazine's rounds and the places of its switches that are on.
/// A new key is a new picture: so the game's picture is made again when a
/// switch on it or below it is flipped (`ToggleResult.RaiseEvents`, its
/// icon refreshed) or a part goes into or out of a slot of it
/// (`GridItemView.CheckSlot`).
pub fn picture_key(item: &trp_sim::items::inventory::Item) -> String {
    let own = if trp_sim::items::inventory::is(item, "Weapon") { trp_sim::items::inventory::rig_key(item) } else { parts_key(item) };
    let mut key = if trp_sim::items::inventory::folding_part(item).is_some_and(|part| part.folded) {
        format!("{own}{FOLDED}")
    } else {
        own
    };
    // A magazine's rounds, as many as its picture shows (the game's key
    // has its visible figure, `IconsHash`), each by its template.
    if !item.kind.visible_rounds.is_empty() && !item.kind.hide_entrails && ROUNDS_DRAWN.load(std::sync::atomic::Ordering::Relaxed) {
        let shown = trp_sim::items::magazine_view::picture_rounds(&item.kind.visible_rounds, trp_sim::items::inventory::magazine_rounds(item));
        let rounds: Vec<String> = (0..shown).filter_map(|from_top| trp_sim::items::inventory::magazine_round_at(item, from_top)).map(|kind| kind.template.clone()).collect();
        if !rounds.is_empty() {
            key = format!("{key}{ROUNDS}{}", rounds.join(","));
        }
    }
    let mut on = Vec::new();
    switched_on(item, &mut Vec::new(), &mut on);
    if !on.is_empty() {
        key = format!("{key}{SWITCHED}{}", on.join(","));
    }
    key
}

/// A picture's key taken apart: its template, whether it is the folded
/// one, a magazine's rounds' templates from the top, a weapon's build's
/// key where it is not its stock build (or another thing's parts), and the
/// places of its switches that are on.
pub(super) struct KeyParts<'a> {
    pub(super) template: &'a str,
    pub(super) folded: bool,
    pub(super) rounds: Vec<&'a str>,
    pub(super) build: Option<&'a str>,
    pub(super) on: Vec<&'a str>,
}

pub(super) fn split_key(key: &str) -> KeyParts<'_> {
    let (key, on) = key.split_once(SWITCHED).map_or((key, Vec::new()), |(key, on)| (key, on.split(',').collect()));
    let (head, rounds) = key.split_once(ROUNDS).map_or((key, Vec::new()), |(head, rounds)| (head, rounds.split(',').collect()));
    let (head, folded) = head.strip_suffix(FOLDED).map_or((head, false), |head| (head, true));
    let template = trp_sim::items::inventory::key_template(head);
    let build = (template.len() != head.len()).then_some(head);
    KeyParts { template, folded, rounds, build, on }
}

/// The words for the cache of the items whose models are not the
/// install's: an outfit of the other game's by its model of the item (a
/// checksum of its path; `donor` where it has none and has its donor's),
/// and a weapon whose stock preset the app put a magazine into
/// (`magazine`; 0.9.1 drew the RPK-16 without one).
pub fn signatures(db: &trp_data::Database) -> HashMap<String, String> {
    let mut signed: HashMap<String, String> = db
        .outfits
        .iter()
        .map(|(id, outfit)| (id.clone(), outfit.world_visual.as_deref().map_or("donor".to_owned(), |world| format!("{:08x}", trp_data::formats::xray_archive::crc32(world.as_bytes())))))
        .collect();
    let mut add = |id: &str, word: &str| {
        signed.entry(id.to_owned()).and_modify(|signature| *signature = format!("{signature}_{word}")).or_insert_with(|| word.to_owned());
    };
    for id in &db.given_magazines {
        add(id, "magazine");
    }
    // A weapon that folds, by its own word or a stock of its preset's:
    // drawn with its fold's turns, an unfolded one with its
    // unfolded turns (the SR-3M's stock lies 3.4 degrees off them as made).
    let flag = |item: &trp_data::setup::database::ItemTemplate, name: &str| item.props.get(name).and_then(|on| on.as_bool()).unwrap_or(false);
    for item in db.items.values().filter(|item| db.is_a(item, "Weapon")) {
        let own = flag(item, "Foldable") || flag(item, "Retractable");
        let stock = || db.default_preset(&item.id).is_ok_and(|preset| preset.items.iter().filter_map(|part| db.items.get(&part.template)).any(|part| db.is_a(part, "Stock") && flag(part, "Foldable")));
        if own || stock() {
            add(&item.id, "fold");
        }
    }
    signed
}
