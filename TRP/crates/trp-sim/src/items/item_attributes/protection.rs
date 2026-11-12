//! The attribute lines of armour, penetration and plates.

use super::*;

/// Armour's rows, in the order its class makes them (`ArmoredEquipment`:
/// its own armour's, its penalties', its eyes' protection, then what holds
/// armour moves its zones and class to the end with the ricochet between;
/// a `Vest` its penalties before its own armour's).
pub(super) fn armour(item: &Item, db: &Database, num: &dyn Fn(&Item, &str) -> f32, words: &dyn Fn(&str) -> String, row: &dyn Fn(&'static str, Option<String>, Show) -> Row, figures: &mut Figures) {
    let template = db.items.get(&item.kind.template).map(|template| &template.props);
    let text = |name: &str| template.and_then(|props| props.get(name)).and_then(Json::as_str).unwrap_or("").to_owned();
    let list = |item: &Item, name: &str| -> Vec<String> {
        db.items.get(&item.kind.template).and_then(|template| template.props.get(name)).and_then(Json::as_array).into_iter().flatten().filter_map(Json::as_str).map(str::to_owned).collect()
    };
    let vest = is(item, "Vest");
    let equipment = is(item, "ArmoredEquipment");
    // A backpack's penalties (`Backpack`: its `EquipmentPenaltyComponent`
    // with no plate's slot, made where any of the three is not nought),
    // each where its own is not nought, after its CONTAINER SIZE.
    if is(item, "Backpack") {
        for (id, name) in [("ChangeMovementSpeed", "speedPenaltyPercent"), ("ChangeTurningSpeed", "mousePenalty"), ("Ergonomics", "weaponErgonomicPenalty")] {
            let value = num(item, name);
            if value != 0.0 {
                let mut penalty = row(id, Some(format!("{} %", plain(value))), Show::Compact);
                penalty.colored = Some(value);
                figures.rows.push(penalty);
            }
        }
        return;
    }
    if !vest && !equipment {
        return;
    }
    // Slots for armour (an insert's or a plate's: any but an attachment's).
    let armour_slots = item.kind.slots.iter().any(|holder| !holder.name.starts_with("mod_"));
    let own = has_armour(item, num);
    let others: Vec<&Item> = parts(item).into_iter().skip(1).filter(|part| has_armour(part, num)).collect();
    // Its zones' names in the install's words, once each (`UniqueLocalizedZones`).
    let zone_names = |of: &[&Item]| -> Vec<String> {
        let mut names: Vec<String> = Vec::new();
        for part in of {
            for zone in list(part, "armorColliders").into_iter().chain(list(part, "armorPlateColliders")) {
                let said = words(&format!("Armor Zone {zone}"));
                if !names.contains(&said) {
                    names.push(said);
                }
            }
        }
        names
    };
    let armours: Vec<&Item> = parts(item).into_iter().filter(|part| has_armour(part, num)).collect();
    let own_rows = |figures: &mut Figures| {
        if !own {
            return;
        }
        let zones = zone_names(&[item]);
        if !zones.is_empty() {
            figures.rows.push(row("ArmorZone", Some(zones.join("\n")), if zones.len() > 1 { Show::Dropdown } else { Show::Compact }));
        }
        // With more than one armour in it, each one's short name and class.
        let class = if armours.len() > 1 {
            armours.iter().map(|part| format!("{}: {}", words(&format!("{} ShortName", part.kind.template)), plain(num(part, "armorClass")))).collect::<Vec<_>>().join("\n")
        } else {
            plain(num(item, "armorClass"))
        };
        figures.rows.push(row("ArmorClass", Some(class), if armours.len() > 1 { Show::Dropdown } else { Show::Compact }));
        figures.rows.push(row("ArmorMaterial", Some(words(&format!("Mat{}", text("ArmorMaterial")))), Show::Compact));
        let kind = text("ArmorType");
        if kind == "Light" || kind == "Heavy" {
            figures.rows.push(row("ArmorType", Some(words(&kind)), Show::Compact));
        }
    };
    let penalty_rows = |figures: &mut Figures| {
        for (id, name, shown) in [("ChangeMovementSpeed", "speedPenaltyPercent", true), ("ChangeTurningSpeed", "mousePenalty", true), ("Ergonomics", "weaponErgonomicPenalty", true)] {
            let value = num(item, name) + others.iter().map(|part| num(part, name)).sum::<f32>();
            if shown && (armour_slots || num(item, name) != 0.0) {
                let mut penalty = row(id, Some(format!("{} %", plain(value))), Show::Compact);
                penalty.colored = Some(value);
                figures.rows.push(penalty);
            }
        }
    };
    let any_penalty = ["speedPenaltyPercent", "mousePenalty", "weaponErgonomicPenalty"].iter().any(|name| num(item, name) != 0.0) || armour_slots;
    if vest {
        if any_penalty {
            penalty_rows(figures);
        }
        own_rows(figures);
    } else {
        own_rows(figures);
        if any_penalty {
            penalty_rows(figures);
        }
        let blindness = num(item, "BlindnessProtection");
        if blindness != 0.0 {
            figures.rows.push(row("BlindnessProtection", Some(percent(blindness)), Show::Compact));
        }
    }
    // What holds armour (`ArmorHolderComponent`): body armour; a rig,
    // headwear or a face cover with a class or slots for armour.
    let holder = is(item, "Armor") || ((vest || is(item, "Headwear") || is(item, "FaceCover")) && (num(item, "armorClass") > 0.0 || armour_slots));
    if !holder {
        return;
    }
    let zones = zone_names(&armours);
    figures.rows.retain(|row| row.id != "ArmorZone" && row.id != "ArmorClass");
    let mut zone_row = row("ArmorZone", None, if zones.len() > 1 { Show::Dropdown } else { Show::Compact });
    zone_row.tip = zones.iter().cloned().map(TipLine::words).collect();
    figures.rows.push(zone_row);
    if is(item, "Headwear") || is(item, "FaceCover") {
        let head = parts(item).into_iter().find(|part| has_armour(part, num));
        let top = head.and_then(|part| db.items.get(&part.kind.template)).and_then(|template| template.props.get("RicochetParams")).and_then(|params| params.get("y")).and_then(number).unwrap_or(0.0);
        let chance = if top > 0.3 { "RHigh" } else if top > 0.15 { "RMed" } else { "RLow" };
        figures.rows.push(row("Ricochet", Some(words(chance)), Show::Compact));
    }
    let class = armours.first().map_or(0.0, |part| num(part, "armorClass"));
    let mut class_row = row("ArmorClass", Some(plain(class)), if armours.len() > 1 { Show::Dropdown } else { Show::Compact });
    class_row.tip = plate_lines(item, db, num, words);
    figures.rows.push(class_row);
}

/// A round's chance through each armour class as its dropdown says it
/// (`MultiLineInfo(penetrationPower)`): for classes 1 to 6, its mark and
/// the install's words for the chance (`VisualizeArmorPenetrationChance`:
/// the chance through new armour of the class, `RealResistance` at 100 of
/// 100: under 20 very low, under 40 low, under 60 medium, under 80 high,
/// else very high).
pub(super) fn penetration_lines(db: &Database, penetration: f32, words: &dyn Fn(&str) -> String) -> Vec<TipLine> {
    let Ok(rules) = crate::condition::armor::ArmorRules::new(&db.config) else { return Vec::new() };
    (1..=6)
        .map(|class| {
            let chance = crate::condition::armor::Resistance::of(100.0, 100.0, rules.resistance_of(class), penetration).chance(penetration);
            let key = match chance {
                c if c < 20.0 => "UI/ArmorPenetration/VeryLow",
                c if c < 40.0 => "UI/ArmorPenetration/Low",
                c if c < 60.0 => "UI/ArmorPenetration/Medium",
                c if c < 80.0 => "UI/ArmorPenetration/High",
                _ => "UI/ArmorPenetration/VeryHigh",
            };
            TipLine { class: Some(class as u32), text: words(key), ..TipLine::default() }
        })
        .collect()
}

/// What holds armour's class dropdown (`MultiLineInfo(item, false)`): the
/// install's words for ARMOR POINTS, a break, then each plate in its own
/// slots (an `ArmorSlot` with an armour plate in it), the locked ones first
/// and each group by its class downwards, a break between the locked and
/// the rest; each its class's mark where the class changes (else as much
/// room), its slot's name in the install's words, " - ", its durability and
/// its greatest to a place.
fn plate_lines(item: &Item, db: &Database, num: &dyn Fn(&Item, &str) -> f32, words: &dyn Fn(&str) -> String) -> Vec<TipLine> {
    let template = db.items.get(&item.kind.template);
    let mut plates: Vec<(bool, u32, String, [f32; 2])> = item
        .kind
        .slots
        .iter()
        .zip(&item.slots)
        .filter(|(holder, part)| holder.armour && part.as_ref().is_some_and(|part| is(part, "ArmorPlate")))
        .filter_map(|(holder, part)| {
            let part = part.as_ref()?;
            let locked = template.and_then(|template| crate::condition::armor::ArmorSlot::of(template, &holder.name)).is_some_and(|slot| slot.locked);
            Some((locked, num(part, "armorClass") as u32, words(&holder.name), part.durability()))
        })
        .collect();
    plates.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)));
    let mut lines = vec![TipLine::words(words("ARMOR POINTS")), TipLine { divider: true, ..TipLine::default() }];
    let (mut seen_locked, mut broke, mut last) = (false, false, None);
    for (locked, class, name, [now, most]) in plates {
        seen_locked |= locked;
        if !locked && !broke && seen_locked {
            lines.push(TipLine { divider: true, ..TipLine::default() });
            broke = true;
            last = None;
        }
        let mark = (last != Some(class)).then_some(class);
        last = Some(class);
        let round = |value: f32| trimmed((value * 10.0).round() / 10.0, 1);
        lines.push(TipLine { class: mark, indent: mark.is_none(), text: format!("{name} - {} / {}", round(now), round(most)), divider: false });
    }
    lines
}
