//! A magazine's rounds as drawn (inventory spec, "A magazine's rounds
//! drawn"; `SpringMagazineVisual`, `Magazine.GetMaxVisibleAmmo`): which of
//! its model's `patron_` points show a round and which round each shows,
//! and the figure its picture is known by. The host finds the points in
//! its model and makes the rounds' models.

/// The game's visible ranges where a template has none or they do not read
/// (`MagazineTemplate.VisibleAmmoRanges`).
const DEFAULT_RANGES: [u32; 2] = [1, 2];

/// A magazine template's visible ranges (`VisibleAmmoRangesString`: each
/// `start-end`, joined by `;`), or the game's default, 1 to 2, where it has
/// none or any of them does not read.
pub fn visible_ranges(text: Option<&str>) -> Vec<[u32; 2]> {
    let Some(text) = text.map(str::trim).filter(|text| !text.is_empty()) else { return vec![DEFAULT_RANGES] };
    let read: Option<Vec<[u32; 2]>> = text
        .split(';')
        .map(|range| {
            let mut ends = range.split('-');
            let start = ends.next()?.trim().parse().ok()?;
            let end = ends.next()?.trim().parse().ok()?;
            Some([start, end])
        })
        .collect();
    read.unwrap_or_else(|| vec![DEFAULT_RANGES])
}

/// `Magazine.GetMaxVisibleAmmo`, the figure a magazine's picture is known
/// by (`IconsHash.HashForItem`): the count where a range holds it; below
/// the first range, its start; past a range and short of the next, that
/// range's end; past the last, the last's end.
pub fn max_visible(ranges: &[[u32; 2]], count: u32) -> u32 {
    let Some(last) = ranges.last() else { return count };
    for (index, &[start, end]) in ranges.iter().enumerate() {
        if (start..=end).contains(&count) {
            return count;
        }
        if count < start {
            return if index == 0 { start } else { ranges[index - 1][1] };
        }
    }
    last[1]
}

/// How many rounds a magazine's picture is drawn with: none for an empty
/// one, else the lesser of its count and its figure (`max_visible`). The
/// game draws a key's picture at the count it was first asked for; the
/// app draws it at what its key says (`PROVISIONAL`: the same where a
/// template's ranges cover the points that can be seen).
pub fn picture_rounds(ranges: &[[u32; 2]], count: u32) -> u32 {
    if count == 0 {
        0
    } else {
        max_visible(ranges, count).min(count)
    }
}

/// A point's number, where an object's name makes it one (a name with
/// `patron_` in it, its number the rest: `patron_001` is 1). The game's
/// parse of any other such name would fail; it is left out.
pub fn point_number(name: &str) -> Option<u32> {
    if !name.contains("patron_") {
        return None;
    }
    name.replace("patron_", "").trim().parse().ok()
}

/// A magazine's points among a model's objects (each its parent and its
/// name, parents before children, siblings in order), breadth first from
/// `root` as the game finds them (`FindAllTransformsWithNameContains`):
/// each object's place and its number.
pub fn points<'a>(nodes: impl IntoIterator<Item = (Option<usize>, &'a str)>, root: usize) -> Vec<(usize, u32)> {
    let nodes: Vec<(Option<usize>, &str)> = nodes.into_iter().collect();
    let mut children = vec![Vec::new(); nodes.len()];
    for (index, (parent, _)) in nodes.iter().enumerate() {
        if let Some(parent) = parent.filter(|&parent| parent < nodes.len()) {
            children[parent].push(index);
        }
    }
    let mut found = Vec::new();
    let mut queue = std::collections::VecDeque::from([root]);
    while let Some(at) = queue.pop_front() {
        let Some((_, name)) = nodes.get(at) else { continue };
        if let Some(number) = point_number(name) {
            found.push((at, number));
        }
        queue.extend(children[at].iter().copied());
    }
    found
}

/// What the point of that place in the list and that number shows with
/// `count` rounds (`FillAmmoObjectsOnStart`): the round so many down from
/// the top (its place in the list), or nothing, the point switched off.
pub fn round_at(place: usize, number: u32, count: u32) -> Option<u32> {
    (count > 0 && number <= count && (place as u32) < count).then_some(place as u32)
}

/// Whether a point stays on as the count falls (`UpdateCartridgesActiveState`:
/// one numbered past the count goes off; none comes back on).
pub fn stays(number: u32, count: u32) -> bool {
    number <= count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_template_s_ranges_are_read_or_the_default_taken() {
        assert_eq!(visible_ranges(Some("1-3")), vec![[1, 3]]);
        assert_eq!(visible_ranges(Some("1-5; 30-30")), vec![[1, 5], [30, 30]]);
        assert_eq!(visible_ranges(None), vec![[1, 2]]);
        assert_eq!(visible_ranges(Some("  ")), vec![[1, 2]]);
        assert_eq!(visible_ranges(Some("1-x")), vec![[1, 2]]);
        assert_eq!(visible_ranges(Some("4")), vec![[1, 2]]);
    }

    #[test]
    fn the_visible_figure_is_the_game_s() {
        // The 6L10's one range, 1 to 3.
        let one = [[1, 3]];
        assert_eq!([0, 1, 3, 4, 30].map(|count| max_visible(&one, count)), [1, 1, 3, 3, 3]);
        // Two ranges: between them the first's end, past both the last's.
        let two = [[2, 5], [10, 12]];
        assert_eq!([1, 2, 7, 10, 11, 20].map(|count| max_visible(&two, count)), [2, 2, 5, 10, 11, 12]);
        // A picture's rounds: none for an empty magazine, never more than
        // it holds.
        assert_eq!([0, 1, 3, 30].map(|count| picture_rounds(&one, count)), [0, 1, 3, 3]);
        assert_eq!(picture_rounds(&two, 1), 1);
    }

    #[test]
    fn the_points_are_found_breadth_first_and_filled_from_the_top() {
        // The 6L10's points under its root, with a point nested below a
        // mesh to show the breadth-first order.
        let nodes = [
            (None, "mag_ak_izhmash_6L10_762x39_30"),
            (Some(0), "mag_LOD0"),
            (Some(1), "patron_030"),
            (Some(0), "patron_001"),
            (Some(0), "patron_002"),
            (Some(0), "patron_in_weapon"),
        ];
        let found = points(nodes, 0);
        assert_eq!(found, vec![(3, 1), (4, 2), (2, 30)]);
        // Two rounds: the first point the top round, the second the next,
        // the thirtieth off; a full magazine shows the thirtieth too.
        let shown = |count: u32| found.iter().enumerate().map(|(place, &(_, number))| round_at(place, number, count)).collect::<Vec<_>>();
        assert_eq!(shown(2), [Some(0), Some(1), None]);
        assert_eq!(shown(30), [Some(0), Some(1), Some(2)]);
        assert_eq!(shown(0), [None, None, None]);
        // As it is fired down, the thirtieth goes off and stays off.
        assert!(!stays(30, 29) && stays(1, 29));
    }
}
