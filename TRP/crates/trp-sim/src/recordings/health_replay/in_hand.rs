//! The meds hands' rig, variants and loops taken in hand for the replay.

use super::*;

/// An item's rig's animator (its `UsePrefab`) and its number of variants.
fn rig_of(db: &Database, assets: &mut Assets, template: &str) -> Result<(WeaponAnimatorData, u32)> {
    let item = db.items.get(template).with_context(|| format!("no item {template}"))?;
    let path = item.props["UsePrefab"]["path"].as_str().with_context(|| format!("{template} has no UsePrefab"))?;
    let rig = trp_data::weapons::weapon::assemble_prefab(assets, path)?;
    let data = WeaponAnimatorData::load(assets, &rig)?;
    let variants = item.props["AnimationVariantsNumber"].as_u64().unwrap_or(0) as u32;
    Ok((data, variants))
}

/// The variants' count's start that gives the recording's first draw: each
/// start tried on an animator alone, made and told as the hands' is, run
/// over the recording's frames until its state leaves the first.
fn first_variant(data: &WeaponAnimatorData, variants: u32, frames: &[Frame]) -> i32 {
    if variants < 2 {
        return 0;
    }
    let score = |first: i32| {
        let mut animator = WeaponAnimator::new(data);
        animator.update(0.0001);
        animator.set("AnimationVariant", ((first + 1).rem_euclid(variants as i32)) as f32);
        animator.set_flag("Active", true);
        frames
            .iter()
            .take(240)
            .take_while(|frame| frame.hands == "MedsController")
            .filter(|frame| {
                animator.update(frame.engine_dt);
                state_hash(animator.state().0) == frame.anim
            })
            .count()
    };
    (0..variants as i32).max_by_key(|first| (score(*first), -first)).unwrap_or(0)
}

/// A starving effect's figures (its damage and how often).
pub(super) fn loop_of(health: &Health, kind: Kind) -> [f32; 2] {
    if kind == Kind::Dehydration {
        health.settings.dehydration_damage
    } else {
        health.settings.exhaustion_damage
    }
}

/// An item's states' names by their hashes, as a recording keeps them.
pub(super) type StateNames = BTreeMap<u32, String>;

/// The item the recording takes in hand on the frame at `index`, put to
/// work by its hands as they spawn: its template the recording's, its
/// resource what it held, the part the one the recording's effect came on
/// where the way of use names one, food and drink by the share used; and,
/// with the assets, its hands' use to set beside the game's.
#[allow(clippy::too_many_arguments)]
pub(super) fn take_in_hand(
    db: &Database,
    assets: Option<&mut Assets>,
    health: &mut Health,
    frames: &[Frame],
    index: usize,
    med_before: &[&(String, String)],
    step: &str,
    item: u64,
    notes: &mut Vec<String>,
) -> Result<(MedsHands, Option<(HandsUse, StateNames)>)> {
    let frame = &frames[index];
    let template = if frame.in_hands.is_empty() { frames[index..].iter().find(|f| !f.in_hands.is_empty()).map(|f| f.in_hands.clone()).unwrap_or_default() } else { frame.in_hands.clone() };
    let medicine = Arc::new(Medicine::of(db, &template).with_context(|| format!("no medicine {template}"))?);
    let resource = frame.resource.filter(|r| *r > 0.0).unwrap_or_else(|| medicine.full());
    // The part: the one the recording's effect came on (the HEALTH tab's or
    // the inventory's "Use" name one; the recorded use speed of the
    // surgical kit says a part was named).
    let key = frame.effects.keys().filter(|(kind, _)| kind == "MedEffect").find(|key| !med_before.contains(key));
    // Food and drink: the share used, as the game's split dialog let it be
    // chosen (`ItemUiContext.ShowSplitMenuDialog`): what the item held when
    // its effect ended, taken from what it held as it began, over its whole.
    let amount = match key.filter(|_| medicine.food() && medicine.max_resource > 0.0) {
        Some(key) => {
            let end = frames[index..].iter().position(|f| !f.effects.contains_key(key)).map(|i| index + i);
            let left = end.and_then(|e| frames[e].resource).unwrap_or(0.0);
            Some(((resource - left) / medicine.max_resource).clamp(0.0, 1.0))
        }
        None => None,
    };
    let rig = match assets {
        Some(assets) => Some(rig_of(db, assets, &template)?),
        None => None,
    };
    let first = rig.as_ref().map_or(0, |(data, variants)| first_variant(data, *variants, &frames[index..]));
    // The parts' list as the way of use makes it: a part dropped on in the
    // HEALTH tab's course, the part an item with a priority of its own goes
    // to by the inventory's "Use" (`BodyPartsPriority`), else the common
    // body, as a quick-use key and "Use" name it (`TargetBodyParts`, the
    // game's continuous healing off, as in the recordings).
    let named = step.starts_with("med-on-health-tab") || !medicine.part_priority.is_empty();
    let part = key.filter(|_| named).and_then(|key| Part::of_name(&key.1));
    let mut made = MedsHands::new(rig.as_ref().map(|r| &r.0), medicine, rig.as_ref().map_or(0, |r| r.1));
    let started = made.take(health, item, resource, &[part], amount, Some(first));
    notes.push(format!(
        "{:.3} s: {} ({template}) on {}, resource {resource}{}: {}",
        frame.t,
        db.items.get(&template).map_or("?", |i| i.name.as_str()),
        part.map_or("the common body", |part| part.name()),
        amount.map_or(String::new(), |a| format!(", a share of {a:.3}")),
        if started.is_some() { "started" } else { "REFUSED" }
    ));
    let use_of = rig.as_ref().map(|(data, _)| {
        let names = data.state_names().into_iter().map(|name| (state_hash(&name), name)).collect();
        (HandsUse { template: template.clone(), first, ..Default::default() }, names)
    });
    Ok((made, use_of))
}
