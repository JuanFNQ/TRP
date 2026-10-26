//! The edit build screen's rules that are not the engine's (workbench
//! spec, "The edit build screen"): a build's slots and their classes, what
//! part a slot takes, a build put on a weapon (`WeaponAssembler`), and
//! where the slots' views lie about the weapon (`ItemObserveScreen.
//! UpdatePositions`: cast onto an ellipse, measured along it by
//! `EllipseCalculator`'s tables, spaced by `Spacer.SpaceOut`). No engine
//! types; the app draws it.

use std::sync::Arc;

use crate::inventory::{is, Id, Item, Kind};

/// What sort of slot a slot is (`EModClass`), which its colour, its
/// toggle and its icon go by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModClass {
    /// A part its parent cannot do without (`Master`).
    Vital,
    Functional,
    Gear,
}

/// The game's mod types (`EWeaponModType`) that are gear
/// (`ItemViewFactory.GetSlotModClass`); every other slot is functional,
/// one whose name is not among the game's types too.
const GEAR: [&str; 10] = ["mod_mount", "mod_stock", "mod_magazine", "mod_barrel", "mod_handguard", "mod_reciever", "mod_charge", "mod_pistol_grip", "mod_launcher", "mod_mag_shaft"];

/// A slot's class: vital where its parent cannot do without it
/// (`_required`), else by its name.
pub fn slot_class(name: &str, required: bool) -> ModClass {
    if required {
        ModClass::Vital
    } else if GEAR.contains(&name) {
        ModClass::Gear
    } else {
        ModClass::Functional
    }
}

/// The words a gear or vital slot is named by (`GetModSlotName`, to be
/// looked up in the game's texts and put in capitals): its own name but
/// for those the game names otherwise. A name that is not one of the
/// game's mod types is shown as a tactical slot's (`mod_tactical`), as the
/// game's parse falls back to it.
pub fn slot_words(name: &str) -> &str {
    match name {
        "mod_magazine" => "Magazine",
        "mod_stock" => "Stock",
        "mod_foregrip" => "Foregrip",
        "mod_barrel" => "Barrel",
        "mod_charge" => "Charge",
        "mod_reciever" => "Receiver",
        "mod_mag_shaft" => "Shaft",
        "mod_pistol_grip" => "PGrip",
        _ if TYPES.contains(&name) => name,
        _ => "mod_tactical",
    }
}

/// The game's mod types by name (`EWeaponModType`).
const TYPES: [&str; 29] = [
    "mod_mount", "mod_scope", "mod_tactical", "mod_stock", "mod_magazine", "mod_barrel", "mod_handguard", "mod_muzzle", "mod_sight_front", "mod_sight_rear", "mod_foregrip", "mod_reciever", "mod_charge", "mod_pistol_grip", "mod_launcher",
    "mod_bipod", "mod_mag_shaft", "mod_silencer", "mod_tactical_2", "chamber0", "chamber1", "patron_in_weapon", "mod_gas_block", "mod_equipment", "mod_equipment_000", "mod_equipment_001", "mod_nvg", "mod_flashlight", "mod_muzzle_001",
];

/// Whether a slot is one of a weapon's chambers, which the screen shows no
/// view for (`CreateModSlots`).
fn is_chamber(name: &str) -> bool {
    name.starts_with("chamber") || name.starts_with("patron_in_weapon") || name.starts_with("camora")
}

/// A slot of a build: the item it is of (the weapon or a part, by its
/// number), its place among that item's slots, its name, its class and
/// what is in it.
#[derive(Debug, Clone, PartialEq)]
pub struct BuildSlot {
    pub owner: Id,
    pub index: usize,
    pub name: String,
    pub class: ModClass,
    pub part: Option<Id>,
    /// Its parent cannot do without it and it is empty: drawn red.
    pub missing: bool,
}

/// Every slot of a build but the chambers, the weapon's first, each
/// part's after its own slot, in its slots' order (`Weapon.AllSlots`).
pub fn build_slots(weapon: &Item) -> Vec<BuildSlot> {
    fn of(item: &Item, out: &mut Vec<BuildSlot>) {
        for (index, (holder, part)) in item.kind.slots.iter().zip(&item.slots).enumerate() {
            if is_chamber(&holder.name) {
                continue;
            }
            let part = part.as_ref().filter(|part| is(part, "Mod"));
            out.push(BuildSlot { owner: item.id, index, name: holder.name.clone(), class: slot_class(&holder.name, holder.required), part: part.map(|part| part.id), missing: holder.required && part.is_none() });
            if let Some(part) = part {
                of(part, out);
            }
        }
    }
    let mut out = Vec::new();
    of(weapon, &mut out);
    out
}

/// Every part of the game a slot could take (`ItemFactory.
/// CreateAllModsEver`): each kind of part that is an item.
pub fn all_parts(kinds: impl IntoIterator<Item = Arc<Kind>>) -> Vec<Arc<Kind>> {
    kinds.into_iter().filter(|kind| kind.is_item && kind.class_names.iter().any(|class| class == "Mod")).collect()
}

/// Whether the slot of that item of the build takes a bare part of that
/// kind in place of what it holds (`Slot.CheckConditions` as the screen
/// asks it): every filter of the slot names the part or a class of it and
/// none's excluded list does (`ItemFilterExtension.CheckItem`); and no
/// part the build keeps names the part among its conflicting items, nor
/// the part one of them (`CheckConflictingItems`, by templates; the
/// slot's own part and what hangs on it go).
pub fn fits(weapon: &Item, owner: Id, index: usize, part: &Kind) -> bool {
    let Some(item) = weapon.find(owner) else { return false };
    let Some(holder) = item.kind.slots.get(index) else { return false };
    if !part.among(&holder.takes) || part.among(&holder.refuses) {
        return false;
    }
    let leaving = item.slots.get(index).and_then(Option::as_ref).map(|old| old.id);
    let mut kept: Vec<&Kind> = Vec::new();
    fn gather<'a>(item: &'a Item, leaving: Option<Id>, kept: &mut Vec<&'a Kind>) {
        if Some(item.id) == leaving {
            return;
        }
        kept.push(&item.kind);
        for part in item.slots.iter().flatten().filter(|part| is(part, "Mod")) {
            gather(part, leaving, kept);
        }
    }
    gather(weapon, leaving, &mut kept);
    !kept.iter().any(|kind| kind.conflicting.iter().any(|id| *id == part.template) || part.conflicting.iter().any(|id| *id == kind.template))
}

/// The build with a bare part of that kind in the slot of that item (none:
/// emptied), what was there taken out with what hung on it
/// (`EditBuildManipulation.Select`). Whether it was done.
pub fn select(build: &mut Item, owner: Id, index: usize, part: Option<&Arc<Kind>>) -> bool {
    let Some(item) = build.find_mut(owner) else { return false };
    let Some(slot) = item.slots.get_mut(index) else { return false };
    *slot = part.map(|kind| Item::new(kind.clone()));
    true
}

/// Whether a weapon already has a build's parts (`WeaponAssembler.
/// CheckIfAlreadyBuilt`): as many, and no template among either's that is
/// not among the other's.
pub fn already_built(weapon: &Item, build: &Item) -> bool {
    let parts = |item: &Item| -> Vec<String> {
        let mut out = Vec::new();
        fn of(item: &Item, out: &mut Vec<String>) {
            for part in item.slots.iter().flatten().filter(|part| is(part, "Mod")) {
                out.push(part.kind.template.clone());
                of(part, out);
            }
        }
        of(item, &mut out);
        out
    };
    let (have, want) = (parts(weapon), parts(build));
    if have.len() != want.len() {
        return false;
    }
    let mut union: Vec<&String> = have.iter().chain(&want).collect();
    union.sort();
    union.dedup();
    union.len() == have.len()
}

/// A build put on the weapon (`WeaponAssembler.AssembleAsync`): every part
/// the weapon has taken off, its own parts with it; each part of the build,
/// slot by slot from the weapon down, one of those taken off of its
/// template where there is one (kept as it was: a magazine's rounds, a
/// fold, a switch), else a new bare one; what is left of those taken off
/// goes (the game's into the stash; the user's: nowhere). The weapon's own
/// chambers keep their rounds. The parts that went, by their numbers.
pub fn assemble(weapon: &mut Item, build: &Item) -> Vec<Id> {
    fn take_off(item: &mut Item, pool: &mut Vec<Item>) {
        let parts: Vec<usize> = (0..item.slots.len()).filter(|&at| item.slots[at].as_ref().is_some_and(|part| is(part, "Mod"))).collect();
        for at in parts {
            if let Some(mut part) = item.slots[at].take() {
                take_off(&mut part, pool);
                pool.push(part);
            }
        }
    }
    fn put_on(item: &mut Item, wanted: &Item, pool: &mut Vec<Item>) {
        for (at, part) in wanted.slots.iter().enumerate() {
            let Some(part) = part.as_ref().filter(|part| is(part, "Mod")) else { continue };
            let mut placed = match pool.iter().position(|old| old.kind.template == part.kind.template) {
                Some(found) => pool.swap_remove(found),
                None => Item::new(part.kind.clone()),
            };
            put_on(&mut placed, part, pool);
            if let Some(slot) = item.slots.get_mut(at) {
                *slot = Some(placed);
            }
        }
    }
    let mut pool = Vec::new();
    take_off(weapon, &mut pool);
    put_on(weapon, build, &mut pool);
    pool.iter().map(|part| part.id).collect()
}

/// An ellipse's arc lengths by whole degrees of its first quarter, as the
/// game's calculator keeps them (`EllipseCalculator.Ellipse`): made with
/// its `width` and `height` as the screen hands them over (the vertical
/// half first), each degree's arc summed over forty steps along it.
#[derive(Debug, Clone)]
pub struct Ellipse {
    width: f64,
    height: f64,
    eccentricity: f64,
    lengths: [f64; 90],
    pub length: f64,
}

const STEPS: usize = 40;
const DEGREE: f64 = 0.01745329;

impl Ellipse {
    pub fn new(width: f64, height: f64) -> Self {
        let mut ellipse = Self { width, height, eccentricity: 1.0 - height * height / (width * width), lengths: [0.0; 90], length: 0.0 };
        for i in 0..90 {
            let arc = ellipse.curve(DEGREE * i as f64, DEGREE * (i + 1) as f64);
            ellipse.lengths[i] = if i > 0 { ellipse.lengths[i - 1] } else { 0.0 } + arc;
        }
        ellipse.length = ellipse.lengths[89] * 4.0;
        ellipse
    }

    /// The arc from one angle to another (`GetCurveLength`).
    fn curve(&self, start: f64, angle: f64) -> f64 {
        if (start - angle).abs() < 1e-10 {
            return 0.0;
        }
        let (c, c0) = (angle.cos(), start.cos());
        let r = self.height / (1.0 - self.eccentricity * c * c).sqrt();
        let r0 = self.height / (1.0 - self.eccentricity * c0 * c0).sqrt();
        let (x, x0) = (r * c, r0 * c0);
        let step = (x - x0).abs() / STEPS as f64;
        let mut at_x = x0;
        let mut at_y = (self.height * self.height * (1.0 - at_x * at_x / (self.width * self.width))).sqrt();
        if at_y.is_nan() {
            at_y = 0.0;
        }
        let mut sum = 0.0;
        let way = if angle > std::f64::consts::PI { -1.0 } else { 1.0 };
        for _ in 0..STEPS {
            let next_x = at_x - step * way;
            let next_y = (self.height * self.height * (1.0 - next_x * next_x / (self.width * self.width))).sqrt();
            sum += ((at_x - next_x).powi(2) + (at_y - next_y).powi(2)).sqrt();
            (at_x, at_y) = (next_x, next_y);
        }
        sum
    }

    /// The length along the ellipse to a whole degree (`GetCashLength`).
    fn length_to(&self, angle: i32) -> f64 {
        if angle <= 0 {
            return 0.0;
        }
        let quarter = self.lengths[89];
        let share = angle as f32 / 90.0;
        if share < 1.0 {
            self.lengths[(angle - 1) as usize]
        } else if share > 1.0 && share < 2.0 {
            quarter + (quarter - self.lengths[(179 - angle) as usize])
        } else if share > 2.0 && share < 3.0 {
            quarter * 2.0 + self.lengths[(angle - 180) as usize]
        } else if share > 3.0 && share < 4.0 {
            quarter * 3.0 + (quarter - self.lengths[(359 - angle) as usize])
        } else {
            share as f64 * quarter
        }
    }

    /// The length along it to an angle in degrees (`LengthFromAngle`).
    pub fn length_at(&self, mut angle: f32) -> f32 {
        if angle < 0.0 {
            angle += 360.0;
        } else if angle > 360.0 {
            angle -= 360.0;
        }
        let whole = angle.floor() as i32;
        (self.length_to(whole) + self.curve(DEGREE * whole as f64, DEGREE * angle as f64)) as f32
    }

    /// The angle in degrees at a length along it (`AngleFromLength`,
    /// `GetAngle`, `GetNearestAngle`).
    pub fn angle_at(&self, mut length: f32) -> f32 {
        if length < 0.0 {
            length += self.length as f32;
        } else if length as f64 > self.length {
            length -= self.length as f32;
        }
        if length.abs() < 0.001 {
            return 0.0;
        }
        let quarter = self.lengths[89] as f32;
        let share = length / quarter;
        if share < 1.0 {
            self.nearest(length)
        } else if share > 1.0 && share < 2.0 {
            90.0 + (90.0 - self.nearest(2.0 * quarter - length))
        } else if share > 2.0 && share < 3.0 {
            180.0 + self.nearest(length - 2.0 * quarter)
        } else if share > 3.0 && share < 4.0 {
            270.0 + (90.0 - self.nearest(4.0 * quarter - length))
        } else {
            share * 90.0
        }
    }

    fn nearest(&self, length: f32) -> f32 {
        let at = self.lengths.iter().position(|&l| length - (l as f32) < 0.0).unwrap_or(0);
        let before = if at == 0 { 0.0 } else { self.lengths[at - 1] };
        let span = self.lengths[at] - before;
        (at as f64 + (length as f64 - before) / span) as f32
    }
}

/// A point put on the ellipse (half axes `a` across and `b` down, about
/// `centre`) along the line from its middle through it
/// (`CastPointsOnEllipse`).
pub fn cast_on_ellipse(point: [f32; 2], centre: [f32; 2], a: f32, b: f32) -> [f32; 2] {
    let angle = (point[1] - centre[1]).atan2(point[0] - centre[0]);
    let (s, c) = angle.sin_cos();
    let r = 1.0 / (b * b * c * c + a * a * s * s).sqrt() * (a * b);
    [r * c + centre[0], r * s + centre[1]]
}

/// Spreads values along a line that comes round on itself after `period`
/// so that no two are nearer than `distance` (`Spacer.SpaceOut`): each a
/// group of its own, groups that come within the distance of each other
/// merged about their mean (the later's values moved a period where they
/// lie across the line's ends), until none meet; then each group's values
/// laid out from its mean, in order, the distance apart. The values in
/// their own order.
pub fn space_out(values: &mut [f32], distance: f32, period: f32) {
    struct Group {
        average: f32,
        half: f32,
        items: Vec<usize>,
    }
    impl Group {
        fn left(&self) -> f32 {
            self.average - self.half
        }
        fn right(&self) -> f32 {
            self.average + self.half
        }
    }
    let settle = |group: &mut Group, values: &[f32]| {
        if !group.items.is_empty() {
            group.average = group.items.iter().map(|&i| values[i]).sum::<f32>() / group.items.len() as f32;
            group.half = (group.items.len() - 1) as f32 * distance * 0.5;
        }
    };
    let meet = |a: &Group, b: &Group| {
        let (mut l1, mut r1, mut l2, mut r2) = (a.left(), a.right(), b.left(), b.right());
        let gap = b.average - a.average;
        if gap.abs() >= period * 0.5 {
            if gap > 0.0 {
                l1 += period;
                r1 += period;
            } else {
                l2 += period;
                r2 += period;
            }
        }
        if l1 >= l2 { l1 < r2 + distance } else { r1 + distance > l2 }
    };
    let mut groups: Vec<Group> = (0..values.len()).map(|i| Group { average: 0.0, half: 0.0, items: vec![i] }).collect();
    loop {
        let mut merged = false;
        for group in &mut groups {
            settle(group, values);
        }
        if groups.len() > 1 {
            let mut first = 0;
            let mut n = 1;
            while n <= groups.len() && groups.len() > 1 {
                let index = if n < groups.len() { n } else { 0 };
                if meet(&groups[first], &groups[index]) {
                    merged = true;
                    // `Group.Merge`: the other's values moved a period
                    // where the two lie across the line's ends.
                    let other = groups.remove(index);
                    let into = if index < first { first - 1 } else { first };
                    let mut sum = groups[into].average * groups[into].items.len() as f32 + other.items.iter().map(|&i| values[i]).sum::<f32>();
                    let gap = other.average - groups[into].average;
                    if gap.abs() >= period * 0.5 {
                        let shift = if gap > 0.0 { -period } else { period };
                        sum += other.items.len() as f32 * shift;
                        for &i in &other.items {
                            values[i] += shift;
                        }
                    }
                    groups[into].items.extend(other.items);
                    groups[into].average = sum / groups[into].items.len() as f32;
                    groups[into].half = (groups[into].items.len() - 1) as f32 * distance * 0.5;
                    first = into;
                    if index == 0 {
                        break;
                    }
                } else {
                    first = index;
                    n += 1;
                }
            }
        }
        if !(merged && groups.len() > 1) {
            break;
        }
    }
    for group in &mut groups {
        let mut at = group.left();
        group.items.sort_by(|&l, &r| values[l].total_cmp(&values[r]));
        for &i in &group.items {
            values[i] = at;
            at += distance;
        }
    }
}

/// Where each slot's view lies (`UpdatePositions`), its bone's point on the
/// screen given (the panel's units, from its bottom left): on the ellipse
/// of half axes `a` across and `b` down about `centre`, by the length along
/// it (the calculator made with `b` and `a`, as the screen hands them) of
/// the bone's point cast onto it, spaced 110 apart along its whole length.
pub fn place_slots(bones: &[[f32; 2]], centre: [f32; 2], a: f32, b: f32) -> Vec<[f32; 2]> {
    const APART: f32 = 110.0;
    if bones.is_empty() || a <= 0.0 || b <= 0.0 {
        return Vec::new();
    }
    let ellipse = Ellipse::new(b as f64, a as f64);
    let mut lengths: Vec<f32> = bones
        .iter()
        .map(|&bone| {
            let on = cast_on_ellipse(bone, centre, a, b);
            let (x, y) = (on[0] - centre[0], on[1] - centre[1]);
            let angle = (y * a).atan2(x * b);
            ellipse.length_at(angle.to_degrees())
        })
        .collect();
    // Sorted along the ellipse before they are spaced (`list.Sort`), each
    // back to its own slot after.
    let mut order: Vec<usize> = (0..lengths.len()).collect();
    order.sort_by(|&l, &r| lengths[l].total_cmp(&lengths[r]));
    let mut sorted: Vec<f32> = order.iter().map(|&i| lengths[i]).collect();
    space_out(&mut sorted, APART, ellipse.length as f32);
    for (at, &i) in order.iter().enumerate() {
        lengths[i] = sorted[at];
    }
    lengths
        .iter()
        .map(|&length| {
            let angle = ellipse.angle_at(length).to_radians();
            [angle.cos() * a + centre[0], angle.sin() * b + centre[1]]
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A circle's quarter is a quarter of its round, and an angle comes
    /// back from its length; in the third quarter a degree on, as the
    /// game's table has it (`GetCashLength` reads `CachedLengths[angle −
    /// 180]`, a degree further than the others' reading).
    #[test]
    fn a_circle_s_lengths_and_angles() {
        let circle = Ellipse::new(100.0, 100.0);
        assert!((circle.length - 2.0 * std::f64::consts::PI * 100.0).abs() < 1.0, "{}", circle.length);
        for (angle, back) in [(10.0f32, 10.0), (45.0, 45.0), (100.0, 100.0), (200.0, 201.0), (300.0, 300.0)] {
            let length = circle.length_at(angle);
            assert!((circle.angle_at(length) - back).abs() < 0.2, "{angle}: {length} back to {}", circle.angle_at(length));
        }
    }

    /// Values nearer than the distance are pushed apart about their mean;
    /// those far apart stay.
    #[test]
    fn values_are_spaced_about_their_mean() {
        let mut values = [100.0, 105.0, 500.0];
        space_out(&mut values, 110.0, 2000.0);
        assert!((values[0] - 47.5).abs() < 1e-3 && (values[1] - 157.5).abs() < 1e-3 && values[2] == 500.0, "{values:?}");
        // Across the line's ends: the later moved round.
        let mut values = [10.0, 1990.0];
        space_out(&mut values, 110.0, 2000.0);
        assert!((values[1] - values[0]).abs() > 109.0, "{values:?}");
    }

    /// A slot's view lies on the ellipse.
    #[test]
    fn slots_lie_on_the_ellipse() {
        let placed = place_slots(&[[900.0, 600.0], [1000.0, 300.0]], [960.0, 540.0], 576.0, 325.0);
        for [x, y] in placed {
            let on = ((x - 960.0) / 576.0).powi(2) + ((y - 540.0) / 325.0).powi(2);
            assert!((on - 1.0).abs() < 0.02, "{x}, {y}: {on}");
        }
    }

    #[test]
    fn slot_classes_and_words() {
        assert_eq!(slot_class("mod_magazine", false), ModClass::Gear);
        assert_eq!(slot_class("mod_scope", false), ModClass::Functional);
        assert_eq!(slot_class("mod_mount_000", false), ModClass::Functional);
        assert_eq!(slot_class("mod_scope", true), ModClass::Vital);
        assert_eq!(slot_words("mod_pistol_grip"), "PGrip");
        assert_eq!(slot_words("mod_gas_block"), "mod_gas_block");
        assert_eq!(slot_words("mod_mount_000"), "mod_tactical");
    }
}
