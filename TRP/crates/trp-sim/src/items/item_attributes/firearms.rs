//! The attribute lines of weapons and ammunition.

use super::*;

/// A share as a whole percentage (`P0`, the invariant culture's: "15 %").
pub(super) fn percent(share: f32) -> String {
    format!("{} %", (share * 100.0).round() as i64)
}

/// A weapon's rows (`Weapon`): its durability row, its six bars over its
/// parts, and its compact rows.
pub(super) fn weapon(item: &Item, db: &Database, num: &dyn Fn(&Item, &str) -> f32, words: &dyn Fn(&str) -> String, row: &dyn Fn(&'static str, Option<String>, Show) -> Row, figures: &mut Figures) {
    let mods: Vec<&Item> = parts(item).into_iter().skip(1).filter(|part| is(part, "Mod")).collect();
    let sum = |name: &str| mods.iter().map(|part| num(part, name)).sum::<f32>();
    let [now, most] = item.durability();
    let peak = num(item, "MaxDurability").max(most);
    figures.durability = Some(Durability { now, most, peak });
    // ERGONOMICS.
    let ergonomics = num(item, "Ergonomics");
    let delta = sum("Ergonomics") / ergonomics.max(1.0);
    let total = (ergonomics * (1.0 + delta)).clamp(0.0, 100.0);
    figures.rows.push(row("Ergonomics", Some(trimmed(total, 2)), Show::Bar { range: [0.0, 100.0], base: ergonomics.round(), delta }));
    // ACCURACY: the barrel's (else the weapon's) centre of impact and
    // deviation, the parts' accuracy, the round's.
    let barrel = parts(item).into_iter().skip(1).find(|part| is(part, "Barrel"));
    let from = barrel.unwrap_or(item);
    let base = num(from, "CenterOfImpact");
    let change = -sum("Accuracy") / 100.0;
    let curve = num(from, "DeviationCurve");
    let greatest = Some(num(from, "DeviationMax")).filter(|max| *max != 0.0).unwrap_or(100.0);
    let deviation = |durability: f32| {
        let twice = 2.0 * curve;
        let s = if 100.0 - twice == 0.0 { durability / twice } else { (-curve + ((100.0 - twice) * durability + curve * curve).sqrt()) / (100.0 - twice) };
        let rest = 1.0 - s;
        rest * rest * greatest + 2.0 * s * rest * curve + s * s
    };
    // The weapon's own deviation now times a repair buff's spread, once
    // (`Weapon.GetBarrelDeviation()`), in the bar's change and the MOA.
    let now_deviation = deviation(now) * crate::items::repair::RepairBuff::factor(item.buff.as_ref(), crate::items::repair::BuffKind::WeaponSpread);
    let at_peak = deviation(peak);
    let delta = change + (now_deviation - at_peak) / (greatest - at_peak);
    let round = current_round(item, db);
    let factor = round.as_deref().map_or(1.0, |round| {
        let accuracy = of(db, round, "ammoAccr");
        if accuracy <= 0.0 { (100.0 + accuracy.abs()) / 100.0 } else { 100.0 / (100.0 + accuracy) }
    });
    let moa = base * (1.0 + change) * factor * now_deviation * 100.0 / 2.9089;
    let widest = db.items.values().filter(|template| db.is_a(template, "Barrel")).filter_map(|template| template.props.get("CenterOfImpact").and_then(number)).fold(0.0f32, f32::max);
    let mut accuracy = row("CenterOfImpact", Some(format!("{} {}", one_or_two(moa), words("moa"))), Show::Bar { range: [widest + 0.1, 0.001], base, delta });
    accuracy.less_is_good = true;
    figures.rows.push(accuracy);
    // SIGHTING RANGE.
    let sighting = mods.iter().map(|part| num(part, "SightingRange")).fold(num(item, "SightingRange"), f32::max);
    figures.rows.push(row("SightingRange", Some(if sighting > 0.0 { plain(sighting) } else { "N/A".to_owned() }), Show::Bar { range: [0.0, 5000.0], base: sighting, delta: 0.0 }));
    // Recoil up and back: a folded stock's recoil left out.
    let folded = folding_part(item).is_some_and(|part| part.folded);
    let recoil = mods.iter().filter(|part| !(folded && is(part, "Stock"))).map(|part| num(part, "Recoil")).sum::<f32>() / 100.0;
    for (id, force) in [("RecoilUp", "RecoilForceUp"), ("RecoilBack", "RecoilForceBack")] {
        let base = num(item, force);
        let mut recoil_row = row(id, Some(format!("{}", (base + base * recoil).round() as i64)), Show::Bar { range: [0.0, 1000.0], base, delta: recoil });
        recoil_row.less_is_good = true;
        figures.rows.push(recoil_row);
    }
    // Velocity: the round it would fire.
    let speed = round.as_deref().map_or(0.0, |round| of(db, round, "InitialSpeed"));
    let delta = (sum("Velocity") + num(item, "Velocity")) / 100.0;
    figures.rows.push(row("Velocity", Some(format!("{} {}", (speed * (1.0 + delta)).round() as i64, words("m/s"))), Show::Bar { range: [0.0, 1500.0], base: speed, delta }));
    // The compact rows, each left out where its base is nought.
    let template = db.items.get(&item.kind.template).map(|template| &template.props);
    let modes: Vec<String> = template.and_then(|props| props.get("weapFireType")).and_then(Json::as_array).into_iter().flatten().filter_map(Json::as_str).map(words).collect();
    if !modes.is_empty() {
        figures.rows.push(row("WeaponFireType", Some(modes.join(", ")), Show::Compact));
    }
    let caliber = template.and_then(|props| props.get("ammoCaliber")).and_then(Json::as_str).unwrap_or("").replace("Caliber", "");
    if !caliber.is_empty() {
        figures.rows.push(row("AmmoCaliber", Some(words(&caliber)), Show::Compact));
    }
    let rate = num(item, "bFirerate");
    if rate != 0.0 {
        figures.rows.push(row("FireRate", Some(format!("{} {}", plain(rate), words("rpm"))), Show::Compact));
    }
    let distance = num(item, "bEffDist");
    if distance != 0.0 {
        figures.rows.push(row("EffectiveDist", Some(format!("{} {}", plain(distance), words("meters"))), Show::Compact));
    }
}

/// A figure to one place and a second where it has one (.NET's `0.0#`).
fn one_or_two(value: f32) -> String {
    let two = format!("{value:.2}");
    match two.strip_suffix('0') {
        Some(one) => one.to_owned(),
        None => two,
    }
}

/// The template of the round a weapon would fire (`CurrentAmmoTemplate`):
/// the one in its chamber, else its magazine's first (`FirstRealAmmo`:
/// its cartridges' last, the top one, as a magazine here keeps them),
/// else its template's own (`defAmmo`).
fn current_round(item: &Item, db: &Database) -> Option<String> {
    let chamber = item.slots.iter().zip(&item.kind.slots).find(|(_, holder)| holder.name.starts_with("patron_in_weapon")).and_then(|(round, _)| round.as_ref());
    let fed = item.slots.iter().zip(&item.kind.slots).find(|(_, holder)| holder.name == crate::items::inventory::MAGAZINE_SLOT).and_then(|(magazine, _)| magazine.as_ref()).and_then(|magazine| magazine.loose.last());
    chamber.or(fed).map(|round| round.kind.template.clone()).or_else(|| db.items.get(&item.kind.template)?.props.get("defAmmo")?.as_str().map(str::to_owned))
}

/// A round's rows (`AmmoTemplate.GetCachedReadonlyQualities`), each where
/// its base is not nought but its calibre.
pub(super) fn ammo(item: &Item, db: &Database, num: &dyn Fn(&Item, &str) -> f32, words: &dyn Fn(&str) -> String, row: &dyn Fn(&'static str, Option<String>, Show) -> Row, figures: &mut Figures) {
    let damage = num(item, "Damage");
    if damage != 0.0 {
        figures.rows.push(row("MaxAmmoDamage", Some(plain(damage)), Show::Compact));
    }
    let penetration = num(item, "PenetrationPower");
    if penetration != 0.0 {
        let mut penetration_row = row("AmmoPenetrationPower", Some(plain(penetration)), Show::Dropdown);
        penetration_row.tip = penetration_lines(db, penetration, words);
        figures.rows.push(penetration_row);
    }
    // (The round's own `Caliber`, in the install's words.)
    let caliber = db.items.get(&item.kind.template).and_then(|template| template.props.get("Caliber")).and_then(Json::as_str).unwrap_or("");
    figures.rows.push(row("Caliber", Some(words(caliber)), Show::Compact));
    let projectiles = num(item, "ProjectileCount");
    if projectiles > 1.0 {
        figures.rows.push(row("AmmoProjectileCount", Some(plain(projectiles)), Show::Compact));
    }
    let speed = num(item, "InitialSpeed");
    if speed != 0.0 {
        figures.rows.push(row("BulletSpeed", Some(format!("{} {}", plain(speed), words("m/s"))), Show::Compact));
    }
    let accuracy = num(item, "ammoAccr");
    if accuracy != 0.0 {
        let mut accuracy_row = row("CenterOfImpact", Some(format!("{accuracy:.1}%")), Show::Compact);
        accuracy_row.colored = Some(accuracy);
        figures.rows.push(accuracy_row);
    }
    let recoil = num(item, "ammoRec");
    if recoil != 0.0 {
        let mut recoil_row = row("Recoil", Some(plain(recoil)), Show::Compact);
        (recoil_row.colored, recoil_row.less_is_good) = (Some(recoil), true);
        figures.rows.push(recoil_row);
    }
    for id in ["HeavyBleedingDelta", "LightBleedingDelta"] {
        let chance = num(item, id);
        if chance != 0.0 {
            figures.rows.push(row(id, Some(percent(chance)), Show::Compact));
        }
    }
}
