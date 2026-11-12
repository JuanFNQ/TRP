//! Fill rules and drawing loot into a container.

use super::*;

/// What a fill leaves out and how far it goes (`LocationLootGenerator`'s
/// figures, SPT's configuration in the install).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FillRules {
    /// Things never put in (the lootable blacklist, and the seasonal
    /// events' things while no event is on).
    pub left_out: HashSet<String>,
    /// The most of a kind the location holds, and how many are out so far.
    pub limits: HashMap<String, u32>,
    pub counted: HashMap<String, u32>,
    /// The count drawn is multiplied by this (`staticLootMultiplier`).
    pub multiplier: f64,
    /// Failures to fit after which a container takes no more
    /// (`fitLootIntoContainerAttempts`: the fill stops once more than this
    /// many have failed).
    pub fit_attempts: u32,
}

/// One of a list by weight (`ProbabilityObjectArray.Draw`: the weights'
/// running sum over their whole, the first whose share reaches the roll).
pub fn draw<'a, T>(list: &'a [(T, f64)], roll: f64) -> Option<&'a T> {
    let whole: f64 = list.iter().map(|(_, weight)| weight.max(0.0)).sum();
    if whole <= 0.0 {
        return None;
    }
    let mut sum = 0.0;
    for (value, weight) in list {
        sum += weight.max(0.0);
        if sum / whole >= roll {
            return Some(value);
        }
    }
    list.last().map(|(value, _)| value)
}

/// A count rounded halves to the even, as .NET's `Math.Round`.
fn round_even(value: f64) -> u32 {
    let floor = value.floor();
    let rest = value - floor;
    let whole = if (rest - 0.5).abs() < 1e-9 {
        if floor % 2.0 == 0.0 {
            floor
        } else {
            floor + 1.0
        }
    } else {
        value.round()
    };
    whole.max(0.0) as u32
}

/// What a container is to hold: how many (`GetWeightedCountOfContainerItems`:
/// a count drawn, times the multiplier, rounded) and which, each drawn by
/// weight with every one back in the pool after (SPT's
/// `allowDuplicateItemsInStaticContainers`), those left out taken out of
/// the pool first.
pub fn draw_loot(table: &LootTable, rules: &FillRules, dice: &mut Dice) -> Vec<String> {
    let Some(&count) = draw(&table.counts, dice.unit()) else { return Vec::new() };
    let count = round_even(rules.multiplier * count as f64);
    let pool: Vec<(String, f64)> = table.items.iter().filter(|(template, _)| !rules.left_out.contains(template)).cloned().collect();
    (0..count).filter_map(|_| draw(&pool, dice.unit()).cloned()).collect()
}

/// A thing of loot as the app makes it (by its own rule, not the game's):
/// a thing with a stock build (a weapon, armour with its plates) as
/// that build, else the kind alone with its locked slots filled
/// (`Item::fill_locked`: a helmet's built-in inserts); money and rounds a stack of a size
/// drawn between the template's `StackMinRandom` and `StackMaxRandom`
/// (SPT's rule for them), anything else one.
pub fn loot_item(db: &Database, template: &str, kinds: &mut HashMap<String, Arc<Kind>>, dice: &mut Dice) -> Result<Item> {
    if db.default_preset(template).is_ok() {
        let list = trp_data::character::inventory::Loadout::of_preset(db, template)?;
        return Ok(Item::of_list(db, &list)?.0);
    }
    let kind = match kinds.get(template) {
        Some(kind) => kind.clone(),
        None => {
            let kind = Arc::new(Kind::of(db, template)?);
            kinds.insert(template.to_owned(), kind.clone());
            kind
        }
    };
    let mut item = Item::new(kind);
    item.fill_locked(db)?;
    if item.kind.stack_max > 1 && (is(&item, "Money") || is(&item, "Ammo")) {
        let props = db.items.get(template).map(|t| (t.prop_f32("StackMinRandom").unwrap_or(1.0), t.prop_f32("StackMaxRandom").unwrap_or(1.0)));
        let (least, most) = props.unwrap_or((1.0, 1.0));
        item.count = dice.whole(least.max(1.0) as u32, (most.max(least).max(1.0) as u32).min(item.kind.stack_max)).max(1);
    }
    Ok(item)
}

/// Fills a container's first grid from its table (`AddLootToContainer`):
/// what is drawn, each made (`loot_item`) and laid in the first room the
/// grid has (rows from the top, cells from the left, turned where it fits
/// nowhere as it lies: `Item::find_room`); a kind at its location's most is
/// passed over, a thing that does not fit is passed over, and once more
/// than the rules' attempts have failed nothing more goes in. What went in.
pub fn fill(db: &Database, table: &LootTable, container: &mut Item, rules: &mut FillRules, dice: &mut Dice) -> Vec<Id> {
    let mut kinds = HashMap::new();
    let mut failed = 0;
    let mut put = Vec::new();
    for template in draw_loot(table, rules, dice) {
        if let Some(&most) = rules.limits.get(&template) {
            let out = rules.counted.entry(template.clone()).or_insert(0);
            if *out >= most {
                continue;
            }
            *out += 1;
        }
        let item = match loot_item(db, &template, &mut kinds, dice) {
            Ok(item) => item,
            Err(_) => continue,
        };
        match container.find_room(0, &item) {
            Some(place) => {
                let id = item.id;
                if container.put(0, item, place).is_ok() {
                    put.push(id);
                }
            }
            None => {
                if failed > rules.fit_attempts {
                    break;
                }
                failed += 1;
            }
        }
    }
    put
}
