//! Two interfaces paired element by element.

use super::*;

/// How a pair was made: by identical words, or by their boxes' overlap.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Paired {
    Words,
    Overlap,
}

/// Two interfaces' drawn elements, paired: `this`'s and `other`'s in draw order, `other`'s
/// boxes scaled to `this`'s window (`scale`, 1 where the sizes agree or one is not known).
#[derive(Clone, Debug, PartialEq)]
pub struct Beside {
    pub this: Vec<Element>,
    pub other: Vec<Element>,
    pub scale: [f64; 2],
    /// Each text's font size in `this`'s window pixels ([`State::font_px`]; the other's
    /// times the window's scale too, the vertical one, a font's size being a height).
    pub this_font_px: Vec<Option<f64>>,
    pub other_font_px: Vec<Option<f64>>,
    /// Indices into `this` and `other`, by `this`'s draw order.
    pub pairs: Vec<(usize, usize, Paired)>,
    pub alone_this: Vec<usize>,
    pub alone_other: Vec<usize>,
}

/// The intersection over the union of two boxes (two empty boxes alike count as 1).
pub fn overlap(a: &[f64; 4], b: &[f64; 4]) -> f64 {
    let area = |r: &[f64; 4]| (r[2] - r[0]).max(0.0) * (r[3] - r[1]).max(0.0);
    let w = (a[2].min(b[2]) - a[0].max(b[0])).max(0.0);
    let h = (a[3].min(b[3]) - a[1].max(b[1])).max(0.0);
    let common = w * h;
    let union = area(a) + area(b) - common;
    if union > 0.0 {
        common / union
    } else if a == b {
        1.0
    } else {
        0.0
    }
}

/// Pairs two lists of elements: first the texts of identical words (trimmed; words repeated
/// are paired by the nearest box centres first), then the rest by their boxes' overlap
/// (intersection over union of 0.5 or more, the largest first, whatever their kinds).
pub fn pair(this: &[Element], other: &[Element]) -> Vec<(usize, usize, Paired)> {
    let mut taken_this = vec![false; this.len()];
    let mut taken_other = vec![false; other.len()];
    let mut pairs = Vec::new();

    let mut groups: BTreeMap<&str, (Vec<usize>, Vec<usize>)> = BTreeMap::new();
    for (i, element) in this.iter().enumerate() {
        if let Some(words) = element.trimmed_words() {
            groups.entry(words).or_default().0.push(i);
        }
    }
    for (j, element) in other.iter().enumerate() {
        if let Some(words) = element.trimmed_words() {
            groups.entry(words).or_default().1.push(j);
        }
    }
    for (mine, theirs) in groups.values() {
        let mut near: Vec<(f64, usize, usize)> = Vec::new();
        for &i in mine {
            for &j in theirs {
                let (a, b) = (this[i].centre(), other[j].centre());
                near.push(((a[0] - b[0]).hypot(a[1] - b[1]), i, j));
            }
        }
        near.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
        for (_, i, j) in near {
            if !taken_this[i] && !taken_other[j] {
                taken_this[i] = true;
                taken_other[j] = true;
                pairs.push((i, j, Paired::Words));
            }
        }
    }

    let mut overlaps: Vec<(f64, usize, usize)> = Vec::new();
    for (i, a) in this.iter().enumerate().filter(|(i, _)| !taken_this[*i]) {
        for (j, b) in other.iter().enumerate().filter(|(j, _)| !taken_other[*j]) {
            let share = overlap(&a.bounds, &b.bounds);
            if share >= 0.5 {
                overlaps.push((share, i, j));
            }
        }
    }
    overlaps.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
    for (_, i, j) in overlaps {
        if !taken_this[i] && !taken_other[j] {
            taken_this[i] = true;
            taken_other[j] = true;
            pairs.push((i, j, Paired::Overlap));
        }
    }
    pairs.sort_by_key(|&(i, j, _)| (i, j));
    pairs
}

/// Two interfaces side by side: the elements each draws, the other's boxes scaled to this
/// one's window where the two sizes differ, paired by [`pair`].
pub fn beside(this: &State, other: &State) -> Beside {
    let scale = match (this.size, other.size) {
        (Some(a), Some(b)) if a != b && b[0] > 0.0 && b[1] > 0.0 => [a[0] / b[0], a[1] / b[1]],
        _ => [1.0, 1.0],
    };
    let (drawn_this, drawn_other) = (this.in_draw_order(false), other.in_draw_order(false));
    let this_font_px = drawn_this.iter().map(|element| this.font_px(element)).collect();
    let other_font_px = drawn_other.iter().map(|element| other.font_px(element).map(|size| size * scale[1])).collect();
    let mine: Vec<Element> = drawn_this.into_iter().cloned().collect();
    let theirs: Vec<Element> = drawn_other.into_iter().map(|element| element.scaled(scale)).collect();
    let pairs = pair(&mine, &theirs);
    let mut paired_this = vec![false; mine.len()];
    let mut paired_other = vec![false; theirs.len()];
    for &(i, j, _) in &pairs {
        paired_this[i] = true;
        paired_other[j] = true;
    }
    let alone_this = (0..mine.len()).filter(|&i| !paired_this[i]).collect();
    let alone_other = (0..theirs.len()).filter(|&j| !paired_other[j]).collect();
    Beside { this: mine, other: theirs, scale, this_font_px, other_font_px, pairs, alone_this, alone_other }
}
