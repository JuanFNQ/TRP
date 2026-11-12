//! What SPT's server makes of a bot's look as it generates one, as far as the range's dummies need it.
//! After `docs/spec/body-hits.md`, "The dummy on the range" (`BotGenerator`):
//! its look, each part a weighted pick from its type's list
//! (`SetBotAppearance`).

/// One of `items` picked by its weight as SPT's server picks
/// (`WeightedRandomHelper.GetWeightedValue`, `WeightedRandom`), `draw` the
/// random number from nought up to one: the only item where there is one;
/// where every weight is one, each alike (`GetInt(0, count - 1)`, both
/// ends in); else the first whose running sum of weights reaches `draw`
/// times their sum, a negative weight skipped (its running sum left at
/// nought). None where there is nothing to pick.
pub fn weighted<T>(items: &[(T, f64)], draw: f64) -> Option<&T> {
    match items {
        [] => return None,
        [(only, _)] => return Some(only),
        _ => {}
    }
    let mut sums = vec![0.0; items.len()];
    let mut sum = 0.0;
    for (at, (_, weight)) in items.iter().enumerate() {
        if *weight >= 0.0 {
            sum += weight;
            sums[at] = sum;
        }
    }
    if sum == items.len() as f64 {
        let at = ((draw * items.len() as f64) as usize).min(items.len() - 1);
        return Some(&items[at].0);
    }
    let reach = sum * draw;
    items.iter().zip(&sums).find(|(_, running)| **running >= reach).map(|((item, _), _)| item)
}

#[cfg(test)]
mod tests {
    use super::weighted;

    #[test]
    fn a_pick_goes_by_the_running_sum_of_the_weights() {
        let items = [("banana", 0.0), ("orange", 0.2), ("apple", 0.8)];
        // The running sums 0, 0.2 and 1: a draw at or under a fifth the
        // orange, above it the apple, never the banana but at nought.
        assert_eq!(weighted(&items, 0.1), Some(&"orange"));
        assert_eq!(weighted(&items, 0.2), Some(&"orange"));
        assert_eq!(weighted(&items, 0.5), Some(&"apple"));
        assert_eq!(weighted(&items, 0.999), Some(&"apple"));
    }

    #[test]
    fn weights_of_one_are_picked_alike_and_one_item_always() {
        let items = [("a", 1.0), ("b", 1.0), ("c", 1.0), ("d", 1.0)];
        assert_eq!(weighted(&items, 0.0), Some(&"a"));
        assert_eq!(weighted(&items, 0.26), Some(&"b"));
        assert_eq!(weighted(&items, 0.99), Some(&"d"));
        assert_eq!(weighted(&[("only", 7.0)], 0.5), Some(&"only"));
        assert_eq!(weighted::<&str>(&[], 0.5), None);
    }

    #[test]
    fn a_negative_weight_is_skipped() {
        let items = [("a", 2.0), ("b", -5.0), ("c", 2.0)];
        // The sums 2, (nought), 4: half the sum is reached by the first.
        assert_eq!(weighted(&items, 0.5), Some(&"a"));
        assert_eq!(weighted(&items, 0.75), Some(&"c"));
    }
}
