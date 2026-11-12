//! The attribute lines of medicine and stimulators, and the buffs they give.

use super::*;

/// Medicine's, food's and drink's rows (`Meds`, `FoodDrink`; each compact):
/// the medkit's resource where it has one (`MedKitComponent`), then
/// (`HealthEffectsComponent`) the use time, each health effect (coloured
/// by its value's sign), each damage effect it takes away (`_replacements`'
/// kinds), and a stimulant's buffs (`SetupStimulatorBuffsAttributes`), in
/// the template's and the set's order (inspect-window spec, "Medicine").
pub(super) fn medicine(item: &Item, db: &Database, num: &dyn Fn(&Item, &str) -> f32, words: &dyn Fn(&str) -> String, row: &dyn Fn(&'static str, Option<String>, Show) -> Row, figures: &mut Figures) {
    let props = db.items.get(&item.kind.template).map(|template| &template.props);
    let sec = words("sec");
    let (del, dur) = (words("Del."), words("Dur."));
    // A delay's, a duration's and a figure's parts, " / " between.
    let timed = |delay: f32, duration: f32| -> Vec<String> {
        let mut parts = Vec::new();
        if delay > 1.0 {
            parts.push(format!("{del} {}{sec}", plain(delay)));
        }
        if duration > 0.0 {
            parts.push(format!("{dur} {}{sec}", plain(duration)));
        }
        parts
    };
    let most = num(item, "MaxHpResource");
    if is(item, "Meds") && most > 0.0 {
        let now = item.resource.unwrap_or(most);
        figures.rows.push(row("HpResource", Some(format!("{}/{} {}", now as i64, most as i64, words("HP"))), Show::Compact));
    }
    let use_time = if is(item, "Meds") { num(item, "medUseTime") } else { num(item, "foodUseTime") };
    figures.rows.push(row("UseTime", Some(format!("{}{sec}", trimmed(use_time, 1))), Show::Compact));
    let field = |name: &str| props.and_then(|props| props.get(name)).and_then(Json::as_object);
    let f = |spec: &Json, key: &str| spec.get(key).and_then(number).unwrap_or(0.0);
    // (The install's keys are in the game's order; serde's map keeps them
    // sorted, which is that order while they are.)
    for (factor, spec) in field("effects_health").into_iter().flatten() {
        let Some(id) = HEALTH_FACTORS.iter().find(|name| name.eq_ignore_ascii_case(factor)).copied() else { continue };
        let value = f(spec, "value");
        let mut parts = timed(f(spec, "delay"), f(spec, "duration"));
        if value != 0.0 {
            parts.push(plain(value.abs()));
        }
        figures.rows.push(Row { table: Table::Health, name: words(id).to_uppercase(), colored: Some(value), ..row(id, Some(parts.join(" / ")), Show::Compact) });
    }
    for (kind, spec) in field("effects_damage").into_iter().flatten() {
        let Some((id, key)) = DAMAGE_NAMES.iter().find(|(name, _)| name == kind).copied() else { continue };
        let mut parts = timed(f(spec, "delay"), f(spec, "duration"));
        let cost = f(spec, "cost");
        if cost > 0.0 {
            parts.push(format!("{} HP", plain(cost)));
        }
        figures.rows.push(Row { table: Table::Damage, name: words(key).trim_end().to_uppercase(), ..row(id, Some(parts.join(" / ")), Show::Compact) });
    }
    let set = props.and_then(|props| props.get("StimulatorBuffs")).and_then(Json::as_str).filter(|set| !set.is_empty());
    let settings = set.and_then(|_| crate::condition::health::StimulatorSettings::read(&db.config["Health"]["Effects"]).ok());
    for buff in set.zip(settings.as_ref()).and_then(|(set, settings)| settings.sets.get(set)).into_iter().flatten() {
        use crate::condition::health::BuffKind;
        let name = match (&buff.kind, &buff.skill) {
            (BuffKind::SkillRate, Some(skill)) => format!("{} \"{}\"", words("Skill"), words(skill)),
            _ => words(buff.kind.name()),
        };
        figures.rows.push(Row { table: Table::Buff, name: name.to_uppercase(), ..row(buff.kind.name(), Some(buff_value(buff, words)), Show::Compact) });
    }
}

/// A buff's value as its row writes it (`StimulatorBuffSettings.
/// GetStringValue`): its chance below a whole, its delay over a second, its
/// duration under two hours with its figure in brackets, else its figure
/// alone; the figure in the buff's colour or the side effect's
/// (`StimulatorHelper.BuffColoredStringValue`: by `IsBuff`, not by its sign).
fn buff_value(buff: &crate::condition::health::BuffDef, words: &dyn Fn(&str) -> String) -> String {
    let sec = words("sec");
    let mut parts = Vec::new();
    if buff.chance < 1.0 {
        parts.push(format!("{} {}%", words("UI/ItemAttribute/Chance"), (buff.chance * 100.0).round() as i64));
    }
    if buff.delay > 1.0 {
        parts.push(format!("{} {}{sec}", words("Del."), plain(buff.delay)));
    }
    let colored = (buff.value.abs() >= f32::EPSILON).then(|| buff_figure(buff, words));
    let mut said = parts.join(" / ");
    if buff.duration > 0.0 && buff.duration < 7200.0 {
        if !said.is_empty() {
            said.push_str(" / ");
        }
        said.push_str(&format!("{} {}{sec}", words("Dur."), plain(buff.duration)));
        if let Some(colored) = colored {
            said.push_str(&format!(" ({colored})"));
        }
    } else if let Some(colored) = colored {
        if !said.is_empty() {
            said.push_str(" / ");
        }
        said.push_str(&format!(" {colored}"));
    }
    said
}

/// A buff's figure as the game writes it in colour (`StimulatorHelper.
/// BuffColoredStringValue`): as shown (`GetVisualBuffValue`: absolute to a
/// place, else a percentage to a whole, halves to the even), "+" before
/// more than nought, its units after (`UnitsName`: "%" where relative; " C°"
/// for the body's temperature, a space and the words for " kg" for the
/// weight), in the buff's colour or the side effect's (`ColoredWithPrefix`:
/// by `IsBuff`, not by its sign; 84, 193, 255 or 196, 0, 0) where it is not
/// nought as shown.
pub fn buff_figure(buff: &crate::condition::health::BuffDef, words: &dyn Fn(&str) -> String) -> String {
    use crate::condition::health::BuffKind;
    let shown = if buff.absolute { round_even(buff.value, 1) } else { round_even(buff.value * 100.0, 0) };
    let units = match (buff.kind, buff.absolute) {
        (_, false) => "%".to_owned(),
        (BuffKind::BodyTemperature, true) => " C°".to_owned(),
        (BuffKind::WeightLimit, true) => format!(" {}", words(" kg")),
        _ => String::new(),
    };
    let sign = if shown >= f32::EPSILON { "+" } else { "" };
    let said = format!("{sign}{}{units}", plain(shown));
    if shown.abs() < f32::EPSILON {
        return said;
    }
    let color = if buff.kind.is_buff(buff.value) { "#54C1FFFF" } else { "#C40000FF" };
    format!("<color={color}>{said}</color>")
}

/// A buff's name as a stimulators' icon lists it (`StimulatorHelper.
/// BuffName`): the words for its kind, or for its skill where it is one,
/// and its figure in brackets where it has one.
pub fn buff_name(buff: &crate::condition::health::BuffDef, words: &dyn Fn(&str) -> String) -> String {
    let key = match (buff.kind, &buff.skill) {
        (crate::condition::health::BuffKind::SkillRate, Some(skill)) => skill.as_str(),
        (kind, _) => kind.name(),
    };
    let name = words(key);
    if buff.value.abs() < f32::EPSILON {
        name
    } else {
        format!("{name} ({})", buff_figure(buff, words))
    }
}

/// What a stimulators' icon's tooltip says, a line each (`EffectIcon.
/// VariationBuffs`), from what it lists (`Health::stimulator_listing`):
/// the first stimulator's buffs, then each stimulator's again in turn, each
/// taking the place of the first line of the same words before it and
/// going to the end (`EffectDescription.Add`; so words a set says twice
/// stay twice); each its name and figure (`buff_name`) and, with between
/// nought and 7,200 seconds left, the seconds in brackets
/// (`BuffDescription.TimeLeftDisplay`). The figures keep the game's colour
/// tags.
pub fn stimulator_lines(listing: &[Vec<(&crate::condition::health::BuffDef, Option<i32>)>], words: &dyn Fn(&str) -> String) -> Vec<String> {
    let named = |(buff, left): &(&crate::condition::health::BuffDef, Option<i32>)| (buff_name(buff, words), *left);
    let mut lines: Vec<(String, Option<i32>)> = listing.first().into_iter().flatten().map(named).collect();
    for (name, left) in listing.iter().flatten().map(named) {
        if let Some(at) = lines.iter().position(|(had, _)| *had == name) {
            lines.remove(at);
        }
        lines.push((name, left));
    }
    lines
        .into_iter()
        .filter(|(name, _)| !name.is_empty())
        .map(|(name, left)| match left {
            Some(seconds) if seconds > 0 && seconds < 7200 => format!("{name} ({seconds})"),
            _ => name,
        })
        .collect()
}
