//! Which of the install's weapons a weapon of one's own could borrow its
//! animator from (`docs/spec/custom-weapons.md`, "Choosing the donor").
//!
//! ```text
//! donor [--no-catch] [--detail] [--from-bone] [part of a name or a template id]...
//! donor --overlay <weapon> <out.png> [--moment idle|draw:<s>|reload:<s>]
//!       [--model <fbx> --place <ahead> <up> <right>] [--without <model>]...
//!       [--scale <pixels a centimetre>] [--window <ahead> <behind> <above> <below>]
//! ```
//!
//! For each weapon named (every rifle, carbine, submachine gun and machine
//! gun of the database if none is): where its trigger, its magazine, its
//! muzzle and its butt are on the weapon's own bone, where the left hand
//! rests on it, and what its hands do when its action is worked, in a
//! draw with nothing chambered and in a reload from empty: which bone
//! travels furthest, how far, and which hand has left its hold at that
//! moment. Everything is given from the middle of the trigger's mesh (or
//! with `--from-bone` from the weapon's own bone), in centimetres: ahead
//! along the barrel, up, and to the right. `--no-catch` runs each as a
//! weapon with no catch for its bolt would run the same animator;
//! `--detail` adds every bone that travelled and both palms every
//! twentieth of a second.
//!
//! `--catalogue [file]` measures every weapon of the database that has a
//! stock preset and whose kind of logic is built, and writes one line for
//! each distinct animator (the weapons that share its clips named on it)
//! to a table of tabs, `local\donors\catalogue.tsv` unless told: the
//! install's animators as donors, to choose among by figures.
//! `--rank <a weapon of one's own's file>` reads that table (making it if
//! it is not there) and the file's `wanted` block, and lists the donors
//! nearest to what is wanted, by the spec's three rules in their order.
//!
//! `--overlay` draws one weapon, as its animator has it at a moment, from
//! its side and from above, with both hands' bones and, if asked, a model
//! laid over it. Given a weapon of one's own it draws that weapon as
//! built, its pose edited.

use anyhow::{anyhow, Context, Result};
use trp_data::unity::extract::{read_mesh, Hierarchy};
use trp_data::unity::Assets;
use trp_data::weapon::Weapon;
use trp_data::{Database, Install};
use trp_sim::math::quat_rotate;
use trp_sim::rig::WeaponRig;
use trp_sim::weapon_animator::WeaponAnimatorData;
use trp_sim::Firearm;

const WEAPON: &str = "Weapon_root/Weapon_root_anim/weapon";
const CLASSES: [&str; 5] = ["AssaultRifle", "AssaultCarbine", "Smg", "MarksmanRifle", "MachineGun"];
/// Every class the catalogue goes over.
const ALL_CLASSES: [&str; 12] = [
    "AssaultRifle", "AssaultCarbine", "Smg", "MarksmanRifle", "MachineGun", "SniperRifle", "Shotgun", "Pistol", "Revolver", "GrenadeLauncher",
    "SpecialWeapon", "RocketLauncher",
];
const FRAME: f32 = 1.0 / 120.0;
/// Where the catalogue is kept unless told: scratch, never the repository.
const CATALOGUE: &str = "local/donors/catalogue.tsv";

/// The action worked once, as measured: the states gone through, the
/// part that travelled furthest back with how far and when and where it
/// rests, and the hand that had left its hold then, with its palm.
#[derive(Debug, Clone, Default)]
struct Stroke {
    states: String,
    part: String,
    travel: f32,
    at_time: f32,
    part_rest: [f32; 3],
    hand: String,
    palm: [f32; 3],
}

/// One weapon as a donor. Every place is in centimetres from the middle
/// of its trigger: ahead along the barrel, up, and to the right.
#[derive(Debug, Clone, Default)]
struct Record {
    name: String,
    id: String,
    class: String,
    /// The mark of its set of clips: weapons with the same are one donor.
    clips: u64,
    rounds: u32,
    catch: bool,
    rate: f32,
    modes: String,
    internal_magazine: bool,
    bolt_action: bool,
    pistol: bool,
    bore_up: f32,
    muzzle_ahead: f32,
    butt_behind: f32,
    length: f32,
    /// The magazine's middle ahead, its top and its bottom.
    magazine: Option<[f32; 3]>,
    rest_left: [f32; 3],
    rest_right: [f32; 3],
    draw: Stroke,
    reload: Stroke,
}

impl Record {
    const HEAD: &'static str = "clips\tweapon\tid\tclass\tshared_with\trounds\tcatch\trate\tmodes\tinternal_magazine\tbolt_action\tpistol\tbore_up\tmuzzle_ahead\tbutt_behind\tlength\tmagazine_ahead\tmagazine_top\tmagazine_bottom\trest_left\trest_right\tdraw_hand\tdraw_part\tdraw_travel\tdraw_part_rest\tdraw_palm\treload_hand\treload_part\treload_travel\treload_part_rest\treload_palm\treload_states";

    fn line(&self, shared: &[String]) -> String {
        let v = |p: [f32; 3]| format!("{:.1} {:.1} {:.1}", p[0], p[1], p[2]);
        let m = |at: usize| self.magazine.map_or("-".to_owned(), |m| format!("{:.1}", m[at]));
        [
            format!("{:08x}", self.clips),
            self.name.clone(),
            self.id.clone(),
            self.class.clone(),
            if shared.is_empty() { "-".to_owned() } else { shared.join(" ") },
            self.rounds.to_string(),
            self.catch.to_string(),
            format!("{:.0}", self.rate),
            self.modes.clone(),
            self.internal_magazine.to_string(),
            self.bolt_action.to_string(),
            self.pistol.to_string(),
            format!("{:.1}", self.bore_up),
            format!("{:.1}", self.muzzle_ahead),
            format!("{:.1}", self.butt_behind),
            format!("{:.1}", self.length),
            m(0),
            m(1),
            m(2),
            v(self.rest_left),
            v(self.rest_right),
            self.draw.hand.clone(),
            self.draw.part.clone(),
            format!("{:.1}", self.draw.travel),
            v(self.draw.part_rest),
            v(self.draw.palm),
            self.reload.hand.clone(),
            self.reload.part.clone(),
            format!("{:.1}", self.reload.travel),
            v(self.reload.part_rest),
            v(self.reload.palm),
            self.reload.states.clone(),
        ]
        .join("\t")
    }

    fn read(line: &str) -> Option<(Self, Vec<String>)> {
        let cells: Vec<&str> = line.split('\t').collect();
        if cells.len() < 32 {
            return None;
        }
        let n = |at: usize| cells[at].parse::<f32>().unwrap_or(f32::NAN);
        let v = |at: usize| -> [f32; 3] {
            let p: Vec<f32> = cells[at].split(' ').filter_map(|c| c.parse().ok()).collect();
            if p.len() == 3 { [p[0], p[1], p[2]] } else { [f32::NAN; 3] }
        };
        let stroke = |at: usize| Stroke { hand: cells[at].to_owned(), part: cells[at + 1].to_owned(), travel: n(at + 2), part_rest: v(at + 3), palm: v(at + 4), ..Stroke::default() };
        let shared = if cells[4] == "-" { Vec::new() } else { cells[4].split(' ').map(str::to_owned).collect() };
        Some((
            Self {
                clips: u64::from_str_radix(cells[0], 16).ok()?,
                name: cells[1].to_owned(),
                id: cells[2].to_owned(),
                class: cells[3].to_owned(),
                rounds: cells[5].parse().unwrap_or(0),
                catch: cells[6] == "true",
                rate: n(7),
                modes: cells[8].to_owned(),
                internal_magazine: cells[9] == "true",
                bolt_action: cells[10] == "true",
                pistol: cells[11] == "true",
                bore_up: n(12),
                muzzle_ahead: n(13),
                butt_behind: n(14),
                length: n(15),
                magazine: (cells[16] != "-").then(|| [n(16), n(17), n(18)]),
                rest_left: v(19),
                rest_right: v(20),
                draw: stroke(21),
                reload: Stroke { states: cells[31].to_owned(), ..stroke(26) },
            },
            shared,
        ))
    }
}

/// A point of the rig on the weapon's own bone, as the rig is posed.
fn on_weapon(rig: &WeaponRig, weapon: usize, bone: usize) -> [f32; 3] {
    let (at, turn) = rig.placed(weapon);
    let back = [-turn[0], -turn[1], -turn[2], turn[3]];
    let there = rig.placed(bone).0;
    quat_rotate(back, std::array::from_fn(|k| there[k] - at[k]))
}

/// The corners of what a mesh's node draws, on the weapon's own bone at
/// rest: least and most along each axis.
fn mesh_box(assets: &Assets, hierarchy: &Hierarchy, weapon: usize, node: usize) -> Option<([f32; 3], [f32; 3])> {
    let renderer = hierarchy.nodes[node].renderer.as_ref()?;
    let mesh = read_mesh(assets, renderer.mesh).ok()?;
    // The node's place on the weapon's bone, with every scale on the way.
    let mut chain = Vec::new();
    let mut at = node;
    while at != weapon {
        chain.push(at);
        at = hierarchy.nodes[at].parent?;
    }
    let (mut low, mut high) = ([f32::MAX; 3], [f32::MIN; 3]);
    for p in &mesh.positions {
        let mut p = *p;
        for &i in &chain {
            let n = &hierarchy.nodes[i];
            let turned = quat_rotate(n.rotation, std::array::from_fn(|k| p[k] * n.scale[k]));
            p = std::array::from_fn(|k| turned[k] + n.translation[k]);
        }
        (0..3).for_each(|k| (low[k], high[k]) = (low[k].min(p[k]), high[k].max(p[k])));
    }
    (low[0] <= high[0]).then_some((low, high))
}

fn grow(into: &mut Option<([f32; 3], [f32; 3])>, other: ([f32; 3], [f32; 3])) {
    *into = Some(match *into {
        None => other,
        Some((low, high)) => (std::array::from_fn(|k| low[k].min(other.0[k])), std::array::from_fn(|k| high[k].max(other.1[k]))),
    });
}

/// Ahead, up and to the right of a reference, in centimetres.
fn from(reference: [f32; 3], p: [f32; 3]) -> [f32; 3] {
    [-(p[1] - reference[1]) * 100.0, (p[2] - reference[2]) * 100.0, (p[0] - reference[0]) * 100.0]
}

fn measure(assets: &mut Assets, db: &Database, hard: &trp_data::hard_settings::HardSettings, template: &str, detail: bool, no_catch: bool) -> Result<String> {
    measure_record(assets, db, hard, template, detail, no_catch).map(|(text, _)| text)
}

/// The same, with the figures as a record for the catalogue.
fn measure_record(
    assets: &mut Assets,
    db: &Database,
    hard: &trp_data::hard_settings::HardSettings,
    template: &str,
    detail: bool,
    no_catch: bool,
) -> Result<(String, Record)> {
    let weapon: Weapon = trp_data::weapon::assemble(assets, db, template)?;
    let mut record = Record { name: weapon.name.clone(), id: template.to_owned(), ..Record::default() };
    // The mark its set of clips is told by.
    {
        use std::hash::{Hash, Hasher};
        let table = trp_data::states::StateTable::load(assets, &weapon)?;
        let mut clips: Vec<String> = table.states.iter().flat_map(|state| state.clips.iter().map(|clip| clip.0.clone())).collect();
        clips.sort();
        clips.dedup();
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        clips.hash(&mut hasher);
        record.clips = hasher.finish() & 0xFFFF_FFFF;
    }
    let hierarchy = &weapon.hierarchy;
    let mut rig = WeaponRig::new(hierarchy);
    let weapon_bone = rig.bone(WEAPON).ok_or_else(|| anyhow!("no weapon bone"))?;
    let below = |mut node: usize, ancestor: usize| loop {
        if node == ancestor {
            return true;
        }
        match hierarchy.nodes[node].parent {
            Some(parent) => node = parent,
            None => return false,
        }
    };
    let path = |node: usize| hierarchy.path(node);
    // What the weapon is made of: every mesh below its bone but the hands'.
    let (mut whole, mut trigger, mut magazine) = (None, None, None);
    for node in (0..hierarchy.nodes.len()).filter(|&n| below(n, weapon_bone)) {
        let Some(bounds) = mesh_box(assets, hierarchy, weapon_bone, node) else { continue };
        let path = path(node).to_lowercase();
        if path.contains("lod1") || path.contains("lod2") || path.contains("lod3") {
            continue;
        }
        grow(&mut whole, bounds);
        if path.contains("trigger") && !path.contains("trigger_group/") || path.contains("/weapon_trigger") || path.contains("mod_trigger/") {
            if hierarchy.nodes[node].name.to_lowercase().contains("trigger") || path.contains("weapon_trigger/") || path.contains("mod_trigger/") {
                grow(&mut trigger, bounds);
            }
        }
        if path.contains("/mod_magazine/") {
            grow(&mut magazine, bounds);
        }
    }
    let whole = whole.context("no meshes")?;
    let centre = |b: ([f32; 3], [f32; 3])| -> [f32; 3] { std::array::from_fn(|k| (b.0[k] + b.1[k]) * 0.5) };
    // The reference: the middle of the trigger's blade, or its bone.
    let named = |name: &str| (0..hierarchy.nodes.len()).find(|&n| hierarchy.nodes[n].name == name && below(n, weapon_bone));
    let reference = match trigger {
        Some(bounds) => centre(bounds),
        None => on_weapon(&rig, weapon_bone, named("weapon_trigger").or(named("mod_trigger")).context("no trigger")?),
    };
    // With `--from-bone` everything is given from the weapon's own bone
    // instead, which is the frame a model of one's own is laid in.
    let reference = if std::env::args().any(|a| a == "--from-bone") { [0.0; 3] } else { reference };
    let rel = |p: [f32; 3]| from(reference, p);
    let magazine_bone = named("mod_magazine");
    let mut out = format!("{:<46} {template}\n", weapon.name);
    let muzzle = named("fireport").map(|n| rel(on_weapon(&rig, weapon_bone, n)));
    out += &format!(
        "  trigger {} | bore {:.1} up | muzzle {:.1} ahead | butt {:.1} behind | length {:.1}\n",
        if trigger.is_some() { "blade" } else { "BONE" },
        muzzle.map_or(f32::NAN, |m| m[1]),
        muzzle.map_or(f32::NAN, |m| m[0]),
        (whole.1[1] - reference[1]) * 100.0,
        (whole.1[1] - whole.0[1]) * 100.0,
    );
    (record.bore_up, record.muzzle_ahead) = (muzzle.map_or(f32::NAN, |m| m[1]), muzzle.map_or(f32::NAN, |m| m[0]));
    (record.butt_behind, record.length) = ((whole.1[1] - reference[1]) * 100.0, (whole.1[1] - whole.0[1]) * 100.0);
    if let Some((low, high)) = magazine {
        let (a, b) = (rel(low), rel(high));
        out += &format!("  magazine: {:.1} to {:.1} ahead, top {:.1} up, bottom {:.1} up\n", b[0], a[0], b[1], a[1]);
        record.magazine = Some([(a[0] + b[0]) * 0.5, b[1], a[1]]);
    }
    let marker = |side: char| rig.bone(&format!("{WEAPON}/weapon_{side}_IK_marker"));
    let (left, right) = (marker('L').context("no left marker")?, marker('R').context("no right marker")?);

    // The animator, and the logic on it.
    let data = WeaponAnimatorData::load(assets, &weapon)?;
    let mut settings = trp_sim::firearm_settings(assets, db, hard, &weapon)?;
    // As a weapon with no catch for its bolt would run this animator.
    if no_catch {
        settings.bolt_catch = false;
    }
    out += &format!(
        "  {} rounds, catch {}, {} rounds a minute, modes {:?}\n",
        settings.magazine_capacity, settings.bolt_catch, settings.fire_rate, settings.modes
    );
    (record.rounds, record.catch, record.rate) = (settings.magazine_capacity, settings.bolt_catch, settings.fire_rate);
    record.modes = format!("{:?}", settings.modes).replace(' ', "");
    (record.internal_magazine, record.bolt_action) = (settings.internal_magazine, settings.bolt_action);
    record.pistol = db.item(template).is_ok_and(|item| db.is_a(item, "Pistol") || db.is_a(item, "Revolver"));
    let moved = rig.moved_bone();
    // The bones of the weapon itself whose travel is looked at: not the
    // markers, the elbows' goals, the magazine or what hangs from them.
    let parts: Vec<usize> = (0..hierarchy.nodes.len())
        .filter(|&n| n != weapon_bone && below(n, weapon_bone))
        .filter(|&n| {
            let p = path(n).to_lowercase();
            !(p.contains("marker") || p.contains("bend_goal") || p.contains("mod_magazine") || p.contains("/prop") || p.contains("patron") || p.contains("shell") || p.contains("base human"))
        })
        .collect();
    let rest: Vec<[f32; 3]> = parts.iter().map(|&n| on_weapon(&rig, weapon_bone, n)).collect();

    // At rest in hand.
    let mut w = Firearm::new(settings.clone(), &data);
    w.start_drawn();
    for _ in 0..60 {
        w.step(FRAME);
    }
    rig.pose(w.animator().pose(), moved);
    let idle_left = rel(on_weapon(&rig, weapon_bone, left));
    let idle_right = rel(on_weapon(&rig, weapon_bone, right));
    out += &format!("  at rest: left palm {idle_left:.1?}, right palm {idle_right:.1?}\n");
    (record.rest_left, record.rest_right) = (idle_left, idle_right);

    // The action worked: a draw with rounds in the magazine and none
    // chambered, and a reload from empty.
    let mut strokes = |label: &str, w: &mut Firearm, seconds: f32, out: &mut String| -> Stroke {
        let mut stroke = Stroke::default();
        let mut best: Option<(f32, usize, f32, [f32; 3], [f32; 3], [f32; 3])> = None;
        let mut furthest = vec![0.0f32; parts.len()];
        let mut states: Vec<String> = Vec::new();
        let mut path_log = Vec::new();
        for frame in 0..(seconds / FRAME) as usize {
            w.step(FRAME);
            let (state, _, next) = w.animator().state();
            let now = next.unwrap_or(state).trim_start_matches("Hands.").to_owned();
            if states.last() != Some(&now) {
                states.push(now);
            }
            rig.pose(w.animator().pose(), moved);
            let (l, r) = (rel(on_weapon(&rig, weapon_bone, left)), rel(on_weapon(&rig, weapon_bone, right)));
            let mut most: Option<(f32, usize, [f32; 3])> = None;
            for (index, &n) in parts.iter().enumerate() {
                let at = on_weapon(&rig, weapon_bone, n);
                // Back along the barrel.
                let travel = (at[1] - rest[index][1]) * 100.0;
                furthest[index] = furthest[index].max(travel);
                if most.is_none_or(|m| travel > m.0) {
                    most = Some((travel, n, rel(at)));
                }
            }
            if let Some((travel, n, at)) = most {
                if frame % 6 == 0 {
                    let magazine = magazine_bone.map(|bone| rel(on_weapon(&rig, weapon_bone, bone))).unwrap_or_default();
                    path_log.push((frame as f32 * FRAME, travel, l, r, magazine, states.last().cloned().unwrap_or_default(), at));
                }
                if best.as_ref().is_none_or(|b| travel > b.0) {
                    best = Some((travel, n, frame as f32 * FRAME, l, r, at));
                }
            }
            if w.is_idle() && frame > 30 {
                break;
            }
        }
        *out += &format!("  {label}: {}\n", states.join(" > "));
        stroke.states = states.join(">");
        if let Some((travel, n, at_time, l, r, at)) = best {
            let name = &hierarchy.nodes[n].name;
            let rest_at = rel(rest[parts.iter().position(|&p| p == n).unwrap()]);
            let _ = at;
            // The hand that works it is the one that has left its hold.
            let gone = |p: [f32; 3], hold: [f32; 3]| (0..3).map(|k| (p[k] - hold[k]).powi(2)).sum::<f32>().sqrt();
            let (left_gone, right_gone) = (gone(l, idle_left), gone(r, idle_right));
            let (hand, palm) = if left_gone > right_gone { ("LEFT", l) } else { ("RIGHT", r) };
            (stroke.part, stroke.travel, stroke.at_time, stroke.part_rest) = (name.clone(), travel, at_time, rest_at);
            (stroke.hand, stroke.palm) = (hand.to_owned(), palm);
            *out += &format!(
                "    {name} goes {travel:.1} back at {at_time:.2} s (rests at {rest_at:.1?}); worked by the {hand} hand, its palm at {palm:.1?} (left {left_gone:.1} from its hold, right {right_gone:.1})\n"
            );
        }
        if detail {
            // Every bone that went back with it, where it rests.
            for (index, &n) in parts.iter().enumerate().filter(|(index, _)| furthest[*index] > 2.0) {
                if hierarchy.nodes[n].renderer.is_some() || hierarchy.nodes[n].name.contains("LOD") {
                    continue;
                }
                *out += &format!("      {:<34} {:5.1} back, rests at {:.1?}\n", hierarchy.nodes[n].name, furthest[index], rel(rest[index]));
            }
            for (time, travel, l, r, magazine, state, furthest_at) in path_log {
                *out += &format!("      {time:5.2} s  {state:<22} travel {travel:5.1} at {furthest_at:6.1?}  left {l:6.1?}  right {r:6.1?}  magazine {magazine:6.1?}\n");
            }
        }
        stroke
    };
    let mut w = Firearm::new(settings.clone(), &data);
    w.chamber = false;
    w.draw();
    record.draw = strokes("drawn unchambered", &mut w, 6.0, &mut out);
    let mut w = Firearm::new(settings.clone(), &data);
    w.start_drawn();
    for _ in 0..30 {
        w.step(FRAME);
    }
    (w.chamber, w.magazine) = (false, Some(0));
    // A weapon filled a round at a time is filled from what is in reach.
    if settings.internal_magazine {
        w.reload_shells(settings.magazine_capacity);
    } else {
        w.reload();
    }
    record.reload = strokes("reloaded from empty", &mut w, 14.0, &mut out);
    Ok((out, record))
}

/// Measures every weapon with a stock preset and writes the catalogue:
/// one line for each distinct set of clips, the first weapon of the set
/// (by name) measured and the others named on its line.
fn catalogue(assets: &mut Assets, db: &Database, hard: &trp_data::hard_settings::HardSettings, file: &str) -> Result<()> {
    let mut items: Vec<_> = db.items.values().filter(|item| item.name.starts_with("weapon_") && db.is_a(item, "Weapon")).collect();
    items.sort_by(|a, b| a.name.cmp(&b.name));
    let mut records: Vec<(Record, Vec<String>)> = Vec::new();
    let mut skipped = Vec::new();
    for item in items {
        let class = ALL_CLASSES.iter().find(|class| db.is_a(item, class)).copied().unwrap_or("-");
        match measure_record(assets, db, hard, &item.id, false, false) {
            Ok((_, mut record)) => {
                record.class = class.to_owned();
                match records.iter_mut().find(|(known, _)| known.clips == record.clips) {
                    Some((_, shared)) => shared.push(record.name),
                    None => records.push((record, Vec::new())),
                }
            }
            Err(e) => skipped.push(format!("{} ({class}): {e:#}", item.name)),
        }
    }
    if let Some(folder) = std::path::Path::new(file).parent() {
        std::fs::create_dir_all(folder)?;
    }
    let mut text = String::from(Record::HEAD);
    text.push('\n');
    for (record, shared) in &records {
        text += &record.line(shared);
        text.push('\n');
    }
    std::fs::write(file, text)?;
    let weapons: usize = records.iter().map(|(_, shared)| 1 + shared.len()).sum();
    println!("{} donors (distinct sets of clips) of {weapons} weapons written to {file}", records.len());
    for line in &skipped {
        println!("  not measured: {line}");
    }
    Ok(())
}

/// What a weapon of one's own asks of a donor (the `wanted` block of its
/// file): the kind of weapon, the hand that works its action and the
/// side its handle is on, whether it has a catch and fires automatically,
/// and where its things are from the middle of its trigger, in
/// centimetres.
#[derive(Debug, Default)]
struct Wanted {
    kind: String,
    worked_by: String,
    side: String,
    automatic: bool,
    bore_up: Option<f32>,
    muzzle_ahead: Option<f32>,
    butt_behind: Option<f32>,
    magazine_ahead: Option<f32>,
    handle: Option<[f32; 3]>,
}

/// Lists the catalogue's donors nearest to what a weapon of one's own
/// wants, by the spec's rules in their order: the kind and the ways of
/// firing decide who is listed at all; then the hand that works the
/// action and the side it is worked on, which no edit of a pose can
/// change; then how far the donor's things are from this weapon's.
fn rank(file: &str, wanted_file: &str) -> Result<()> {
    let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(wanted_file).with_context(|| wanted_file.to_owned())?)?;
    let block = json.get("wanted").with_context(|| format!("{wanted_file} has no `wanted` block"))?;
    let word = |name: &str| block.get(name).and_then(|v| v.as_str()).unwrap_or("").to_lowercase();
    let number = |name: &str| block.get(name).and_then(|v| v.as_f64()).map(|v| v as f32);
    let wanted = Wanted {
        kind: word("kind"),
        worked_by: word("worked_by"),
        side: word("side"),
        automatic: block.get("automatic").and_then(|v| v.as_bool()).unwrap_or(false),
        bore_up: number("bore_up"),
        muzzle_ahead: number("muzzle_ahead"),
        butt_behind: number("butt_behind"),
        magazine_ahead: number("magazine_ahead"),
        handle: block.get("handle").and_then(|v| v.as_array()).and_then(|a| Some([a.first()?.as_f64()? as f32, a.get(1)?.as_f64()? as f32, a.get(2)?.as_f64()? as f32])),
    };
    let text = std::fs::read_to_string(file).with_context(|| format!("{file}: make it with --catalogue"))?;
    let own = json.get("id").and_then(|v| v.as_str()).unwrap_or("");
    let mut listed = Vec::new();
    for (record, shared) in text.lines().skip(1).filter_map(Record::read) {
        // The weapon itself is no donor of its own.
        if record.id == own {
            continue;
        }
        // A shotgun is one whatever it is fed from; a tube is any other
        // weapon filled a round at a time.
        let kind = match (record.pistol, record.bolt_action, record.class == "Shotgun", record.internal_magazine) {
            (true, ..) => "pistol",
            (_, true, ..) => "bolt",
            (_, _, true, _) => "shotgun",
            (.., true) => "tube",
            _ => "rifle",
        };
        if !wanted.kind.is_empty() && wanted.kind != kind {
            continue;
        }
        if wanted.automatic && !record.modes.contains("FullAuto") {
            continue;
        }
        // The hand, and the side the part worked rests on.
        let hand_off = !wanted.worked_by.is_empty() && record.reload.hand.to_lowercase() != wanted.worked_by;
        let side = if record.reload.palm[2] < -1.0 { "left" } else if record.reload.palm[2] > 1.0 { "right" } else { "middle" };
        let side_off = !wanted.side.is_empty() && side != wanted.side;
        let gap = |want: Option<f32>, has: f32| want.filter(|_| has.is_finite()).map(|want| has - want);
        let gaps = [
            gap(wanted.bore_up, record.bore_up),
            gap(wanted.muzzle_ahead, record.muzzle_ahead),
            gap(wanted.butt_behind, record.butt_behind),
            gap(wanted.magazine_ahead, record.magazine.map_or(f32::NAN, |m| m[0])),
            gap(wanted.handle.map(|h| h[0]), record.reload.part_rest[0]),
            gap(wanted.handle.map(|h| h[1]), record.reload.part_rest[1]),
        ];
        let distance = gaps.iter().flatten().map(|g| g * g).sum::<f32>().sqrt();
        listed.push((hand_off, side_off, distance, record, shared, side, gaps));
    }
    listed.sort_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)).then(a.2.total_cmp(&b.2)));
    println!("donors for {wanted_file}, nearest first (centimetres off: bore, muzzle, butt, magazine, handle ahead, handle up):");
    for (hand_off, side_off, distance, record, shared, side, gaps) in listed.iter().take(15) {
        let off: Vec<String> = gaps.iter().map(|g| g.map_or("   -".to_owned(), |g| format!("{g:+5.1}"))).collect();
        println!(
            "  {:5.1}  {:<44} worked by the {} hand, {side}{}{}  [{}]  catch {}, {} rounds{}",
            distance,
            record.name,
            record.reload.hand.to_lowercase(),
            if *hand_off { " (NOT the hand wanted)" } else { "" },
            if *side_off { " (NOT the side wanted)" } else { "" },
            off.join(" "),
            record.catch,
            record.rounds,
            if shared.is_empty() { String::new() } else { format!("; the same clips: {}", shared.join(", ")) }
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_of_the_catalogue_reads_back() {
        let record = Record {
            name: "weapon_x".to_owned(),
            id: "abc".to_owned(),
            class: "AssaultRifle".to_owned(),
            clips: 0x1234_abcd,
            rounds: 30,
            catch: true,
            rate: 600.0,
            modes: "[Single,FullAuto]".to_owned(),
            bore_up: 6.5,
            muzzle_ahead: 57.0,
            butt_behind: 35.5,
            length: 92.5,
            magazine: Some([11.5, 2.0, -18.0]),
            rest_left: [30.0, 1.5, -4.0],
            rest_right: [-2.0, -4.5, 3.0],
            reload: Stroke { hand: "LEFT".to_owned(), part: "weapon_bolt".to_owned(), travel: 9.5, part_rest: [9.0, 8.5, -2.0], palm: [3.0, 9.0, -6.5], states: "RELOAD IN>IDLE".to_owned(), ..Stroke::default() },
            ..Record::default()
        };
        let line = record.line(&["weapon_y".to_owned()]);
        assert_eq!(line.split('\t').count(), Record::HEAD.split('\t').count());
        let (back, shared) = Record::read(&line).expect("a line of its own");
        assert_eq!(shared, ["weapon_y"]);
        assert_eq!((back.clips, back.name.as_str(), back.rounds, back.catch), (0x1234_abcd, "weapon_x", 30, true));
        assert_eq!((back.magazine, back.reload.part_rest, back.reload.hand.as_str()), (Some([11.5, 2.0, -18.0]), [9.0, 8.5, -2.0], "LEFT"));
        assert_eq!(back.reload.states, "RELOAD IN>IDLE");
    }
}

/// Colours to tell a model's materials apart by.
const PALETTE: [[u8; 3]; 5] = [[150, 110, 70], [95, 100, 110], [120, 125, 135], [70, 75, 85], [140, 120, 60]];

/// A picture of one of the install's weapons as its animator has it at a
/// moment, from its side and from above, with both hands' bones as the
/// clip has them (the right green, the left blue; a cross at each palm's
/// marker) and, half seen through, a model of one's own laid over it.
///
/// `moment` is `idle`, or `draw:<seconds>` into a draw with nothing
/// chambered, or `reload:<seconds>` into a reload from empty. `place` is
/// where the model's own origin goes on the weapon's bone: centimetres
/// ahead, up and to the right. The model's barrel runs along -z, y up.
#[allow(clippy::too_many_arguments)]
fn overlay(
    assets: &mut Assets,
    db: &Database,
    hard: &trp_data::hard_settings::HardSettings,
    template: &str,
    model: Option<&str>,
    out: &str,
    moment: &str,
    place: [f32; 3],
    scale: f32,
    window: Option<[f32; 4]>,
    skip: &[&str],
    no_catch: bool,
) -> Result<()> {
    let weapon: Weapon = trp_data::weapon::assemble(assets, db, template)?;
    let hierarchy = &weapon.hierarchy;
    let mut rig = WeaponRig::new(hierarchy);
    let weapon_bone = rig.bone(WEAPON).ok_or_else(|| anyhow!("no weapon bone"))?;
    let data = WeaponAnimatorData::load(assets, &weapon)?;
    let mut settings = trp_sim::firearm_settings(assets, db, hard, &weapon)?;
    if no_catch {
        settings.bolt_catch = false;
    }
    let mut w = Firearm::new(settings, &data);
    let (what, seconds) = moment.split_once(':').map_or((moment, 0.0), |(what, s)| (what, s.parse::<f32>().unwrap_or(0.0)));
    match what {
        "draw" => {
            w.chamber = false;
            w.draw();
        }
        _ => {
            w.start_drawn();
            for _ in 0..60 {
                w.step(FRAME);
            }
            if what == "reload" {
                (w.chamber, w.magazine) = (false, Some(0));
                w.reload();
            }
        }
    }
    for _ in 0..(seconds / FRAME).round() as usize {
        w.step(FRAME);
    }
    let (state, through, next) = w.animator().state();
    println!("{}: {state} at {through:.3}{}", weapon.name, next.map_or(String::new(), |next| format!(", on its way to {next}")));
    rig.pose(w.animator().pose(), rig.moved_bone());

    // A point of a node on the weapon's bone, as the rig is posed, with
    // every scale on the way; and from there to the picture's own space.
    let on_bone = |node: usize, mut p: [f32; 3]| -> Option<[f32; 3]> {
        let mut at = node;
        while at != weapon_bone {
            let (place, turn) = rig.local(at);
            let turned = quat_rotate(turn, std::array::from_fn(|k| p[k] * hierarchy.nodes[at].scale[k]));
            p = std::array::from_fn(|k| turned[k] + place[k]);
            at = hierarchy.nodes[at].parent?;
        }
        Some(p)
    };
    let seen = |p: [f32; 3]| [-p[1] * 100.0, p[2] * 100.0, p[0] * 100.0];
    let mut triangles: Vec<trp_data::sketch::Drawn> = Vec::new();
    for (index, node) in hierarchy.nodes.iter().enumerate() {
        let Some(renderer) = &node.renderer else { continue };
        if node.name.contains("LOD1") || node.name.contains("LOD2") {
            continue;
        }
        let Ok(mesh) = read_mesh(assets, renderer.mesh) else { continue };
        let Some(placed) = mesh.positions.iter().map(|p| on_bone(index, *p)).collect::<Option<Vec<_>>>() else { continue };
        for triangle in mesh.submeshes.iter().flat_map(|indices| indices.chunks_exact(3)) {
            if triangle.iter().all(|&i| (i as usize) < placed.len()) {
                triangles.push(([0, 1, 2].map(|k| seen(placed[triangle[k] as usize])), [150, 155, 165], 1.0));
            }
        }
    }
    // A weapon of one's own: its parts as the edited pose has them.
    for part in weapon.custom.iter().flat_map(|custom| &custom.parts) {
        let Some(placed) = part.mesh.positions.iter().map(|p| on_bone(part.node, *p)).collect::<Option<Vec<_>>>() else { continue };
        for (slot, indices) in part.mesh.submeshes.iter().enumerate() {
            let colour = PALETTE[part.materials.get(slot).copied().unwrap_or(0) % PALETTE.len()];
            for triangle in indices.chunks_exact(3) {
                triangles.push(([0, 1, 2].map(|k| seen(placed[triangle[k] as usize])), colour, 1.0));
            }
        }
    }
    if let Some(file) = model {
        let scene = trp_data::fbx::read(&std::fs::read(file)?)?;
        let unit = scene.unit as f32;
        for part in scene.models.iter().filter(|part| !skip.contains(&part.name.as_str())) {
            let Some(mesh) = &part.mesh else { continue };
            for corners in mesh.positions.chunks_exact(3) {
                let placed = [0, 1, 2].map(|k| [-corners[k][2] * unit + place[0], corners[k][1] * unit + place[1], corners[k][0] * unit + place[2]]);
                triangles.push((placed, [235, 120, 30], 0.5));
            }
        }
    }
    // The hands as the clip has them: each finger from the palm out.
    let in_frame = |bone: usize| {
        let (at, turn) = rig.placed(weapon_bone);
        let back = [-turn[0], -turn[1], -turn[2], turn[3]];
        let there = rig.placed(bone).0;
        seen(quat_rotate(back, std::array::from_fn(|k| there[k] - at[k])))
    };
    let (mut lines, mut marks) = (Vec::new(), Vec::new());
    for (side, colour) in [('R', [20u8, 140, 40]), ('L', [30, 60, 220])] {
        let arm = format!("Base Human{side}Collarbone/Base Human{side}Upperarm/Base Human{side}Forearm1/Base Human{side}Forearm2/Base Human{side}Forearm3");
        let (Some(wrist), Some(palm)) = (rig.bone(&arm), rig.bone(&format!("{arm}/Base Human{side}Palm"))) else { continue };
        // The game does not show the rig's own arms: it solves the
        // body's onto the palm's marker. So the hand is drawn there, as
        // the clip has it turned: where the marker is not where the
        // clip's own palm is (a weapon of one's own moves it), the
        // picture shows the hand the player will see.
        let moved = rig.bone(&format!("{WEAPON}/weapon_{side}_IK_marker")).map(|marker| {
            let (to, from) = (in_frame(marker), in_frame(palm));
            [to[0] - from[0], to[1] - from[1], to[2] - from[2]]
        });
        let moved = moved.unwrap_or_default();
        let in_frame = |bone: usize| {
            let at = in_frame(bone);
            [at[0] + moved[0], at[1] + moved[1], at[2] + moved[2]]
        };
        lines.push((in_frame(wrist), in_frame(palm), colour));
        for finger in 1..=5 {
            let mut path = format!("{arm}/Base Human{side}Palm");
            let mut last = palm;
            for joint in 1..=3 {
                path += &format!("/Base Human{side}Digit{finger}{joint}");
                let Some(bone) = rig.bone(&path) else { break };
                lines.push((in_frame(last), in_frame(bone), colour));
                last = bone;
            }
            // The last joint's own length is not a bone: a stub along it.
            let (at, turn) = rig.placed(last);
            let tip = quat_rotate(turn, [-0.025, 0.0, 0.0]);
            let (weapon_at, weapon_turn) = rig.placed(weapon_bone);
            let back = [-weapon_turn[0], -weapon_turn[1], -weapon_turn[2], weapon_turn[3]];
            let end = seen(quat_rotate(back, std::array::from_fn(|k| at[k] + tip[k] - weapon_at[k])));
            let end = [end[0] + moved[0], end[1] + moved[1], end[2] + moved[2]];
            lines.push((in_frame(last), end, colour.map(|c| c / 2 + 110)));
        }
        marks.push((in_frame(palm), colour));
        // The figures a model is laid to the hand by: the end of the
        // forefinger and the middle finger's first joint.
        let joint = |finger: u32, joints: u32| {
            let mut path = format!("{arm}/Base Human{side}Palm");
            for joint in 1..=joints {
                path += &format!("/Base Human{side}Digit{finger}{joint}");
            }
            rig.bone(&path).map(in_frame)
        };
        println!(
            "  {side}: palm {:.1?}, forefinger's joints {:.1?} {:.1?} {:.1?}, middle finger's first joint {:.1?}, thumb's {:.1?}",
            in_frame(palm),
            joint(2, 1).unwrap_or_default(),
            joint(2, 2).unwrap_or_default(),
            joint(2, 3).unwrap_or_default(),
            joint(3, 1).unwrap_or_default(),
            joint(1, 1).unwrap_or_default()
        );
    }
    // The weapon's own bones a model's parts would be hung on.
    for (index, node) in hierarchy.nodes.iter().enumerate() {
        let name = node.name.to_lowercase();
        let wanted = ["charg", "bolt", "mod_magazine", "fireport", "trigger", "mod_stock", "mod_align", "aim_camera", "selector", "catch", "release", "mod_pistol_grip", "mod_handguard"];
        if node.renderer.is_none() && !name.contains("lod") && wanted.iter().any(|w| name.contains(w)) && hierarchy.path(index).starts_with(WEAPON) {
            println!("  {:<28} {:.1?}", node.name, in_frame(index));
        }
    }
    trp_data::sketch::views(&triangles, &marks, &lines, out, scale, window)
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // `donor --overlay <weapon> <out.png> [--model <fbx>] [--moment idle|
    // draw:<s>|reload:<s>] [--place <ahead> <up> <right>] [--scale <pixels
    // a centimetre>] [--window <ahead> <behind> <above> <below>]
    // [--without <model>]...`: a picture in place of the figures.
    if let Some(at) = args.iter().position(|a| a == "--overlay") {
        let word = |name: &str, offset: usize| args.iter().position(|a| a == name).and_then(|i| args.get(i + offset));
        let number = |name: &str, offset: usize| word(name, offset).and_then(|v| v.parse::<f32>().ok());
        let (Some(wanted), Some(out)) = (args.get(at + 1), args.get(at + 2)) else { anyhow::bail!("--overlay <weapon> <out.png>") };
        let install = Install::locate()?;
        let db = Database::load(&install)?;
        let hard = trp_data::hard_settings::HardSettings::load(&install)?;
        let mut assets = Assets::new(install)?;
        let template = db
            .items
            .values()
            .filter(|item| item.id == *wanted || item.name.to_lowercase().contains(&wanted.to_lowercase()))
            .map(|item| item.id.clone())
            .min()
            .ok_or_else(|| anyhow!("no weapon named {wanted}"))?;
        let place = [number("--place", 1).unwrap_or(0.0), number("--place", 2).unwrap_or(0.0), number("--place", 3).unwrap_or(0.0)];
        let window = (|| Some([number("--window", 1)?, number("--window", 2)?, number("--window", 3)?, number("--window", 4)?]))();
        let skip: Vec<&str> = args.windows(2).filter(|pair| pair[0] == "--without").map(|pair| pair[1].as_str()).collect();
        return overlay(
            &mut assets,
            &db,
            &hard,
            &template,
            word("--model", 1).map(String::as_str),
            out,
            word("--moment", 1).map_or("idle", String::as_str),
            place,
            number("--scale", 1).unwrap_or(12.0),
            window,
            &skip,
            args.iter().any(|a| a == "--no-catch"),
        );
    }
    let detail = args.iter().any(|a| a == "--detail");
    let no_catch = args.iter().any(|a| a == "--no-catch");
    let install = Install::locate()?;
    let db = Database::load(&install)?;
    let hard = trp_data::hard_settings::HardSettings::load(&install)?;
    let mut assets = Assets::new(install)?;
    // `--catalogue [file]` and `--rank <file of a weapon of one's own>
    // [catalogue]`.
    let after = |name: &str| args.iter().position(|a| a == name).map(|at| args.get(at + 1).filter(|next| !next.starts_with("--")).cloned());
    if let Some(file) = after("--catalogue") {
        return catalogue(&mut assets, &db, &hard, file.as_deref().unwrap_or(CATALOGUE));
    }
    if let Some(wanted_file) = after("--rank") {
        let wanted_file = wanted_file.context("--rank <the file of a weapon of one's own>")?;
        let file = args.iter().position(|a| a == "--rank").and_then(|at| args.get(at + 2)).filter(|next| !next.starts_with("--")).cloned().unwrap_or(CATALOGUE.to_owned());
        if !std::path::Path::new(&file).exists() {
            catalogue(&mut assets, &db, &hard, &file)?;
        }
        return rank(&file, &wanted_file);
    }
    let wanted: Vec<String> = args.iter().filter(|a| !a.starts_with("--")).map(|a| a.to_lowercase()).collect();
    let mut templates: Vec<(&str, &str)> = db
        .items
        .values()
        .filter(|item| item.name.starts_with("weapon_") && CLASSES.iter().any(|class| db.is_a(item, class)))
        .filter(|item| wanted.is_empty() || wanted.iter().any(|w| item.id == *w || item.name.to_lowercase().contains(w)))
        .map(|item| (item.name.as_str(), item.id.as_str()))
        .collect();
    templates.sort();
    for (name, template) in templates {
        match measure(&mut assets, &db, &hard, template, detail, no_catch) {
            Ok(text) => print!("{text}"),
            Err(e) => println!("{name:<46} {template}\n  not measured: {e:#}"),
        }
    }
    Ok(())
}
