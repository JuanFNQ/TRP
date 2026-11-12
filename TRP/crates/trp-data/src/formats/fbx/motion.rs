//! A binary FBX file's motion: its bones, and how each is placed and turned in its parent over the file's take.
//!
//! A take is kept as curves (`AnimationCurve`: key times and values), three
//! to a curve node (`AnimationCurveNode`, its `d|X`, `d|Y` and `d|Z`), each
//! node tied to one property of a model (`Lcl Translation`, `Lcl Rotation`)
//! by the file's connections. Times are in the format's own ticks, of which
//! there are 46 186 158 000 to a second (the figure Blender's reader of the
//! format goes by, `FBX_KTIME`). A bone's turn in its parent is its
//! `PreRotation`, then its `Lcl Rotation`, then its `PostRotation` undone
//! (the SDK's "Computing transformation matrices"), each three angles in
//! degrees turned about x, then y, then z.
//!
//! What is read is a skeleton baked a key to a frame, as a motion library
//! exports one: between two keys a value goes by a straight line, and the
//! curves' own tangents are not followed.

use super::*;

const TICKS_A_SECOND: f64 = 46_186_158_000.0;

/// A turn as a quaternion (x, y, z, w).
pub type Turn = [f64; 4];

/// The turn of `b`, then of `a`.
pub fn turns(a: Turn, b: Turn) -> Turn {
    [
        a[3] * b[0] + a[0] * b[3] + a[1] * b[2] - a[2] * b[1],
        a[3] * b[1] - a[0] * b[2] + a[1] * b[3] + a[2] * b[0],
        a[3] * b[2] + a[0] * b[1] - a[1] * b[0] + a[2] * b[3],
        a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2],
    ]
}

pub fn undone(q: Turn) -> Turn {
    [-q[0], -q[1], -q[2], q[3]]
}

/// A vector turned.
pub fn turned(q: Turn, v: [f64; 3]) -> [f64; 3] {
    let p = turns(turns(q, [v[0], v[1], v[2], 0.0]), undone(q));
    [p[0], p[1], p[2]]
}

/// The turn by angles in degrees about x, then y, then z, each about the
/// fixed axes: the format's default order.
pub fn turn_of(degrees: [f64; 3]) -> Turn {
    let [(sx, cx), (sy, cy), (sz, cz)] = degrees.map(|d| (d.to_radians() / 2.0).sin_cos());
    turns([0.0, 0.0, sz, cz], turns([0.0, sy, 0.0, cy], [sx, 0.0, 0.0, cx]))
}

/// One bone of a motion.
#[derive(Debug, Clone)]
pub struct MovingBone {
    pub name: String,
    pub parent: Option<usize>,
    /// Its place and its turn in its parent as the file's models stand, in
    /// the file's axes and units: the pose the skeleton was made in.
    pub rest: ([f64; 3], Turn),
    /// Its place and its turn in its parent on each frame. One entry, the
    /// rest's, where the take does not move it.
    pub places: Vec<[f64; 3]>,
    pub turns: Vec<Turn>,
}

impl MovingBone {
    /// Its place and turn in its parent on a frame.
    pub fn on(&self, frame: usize) -> ([f64; 3], Turn) {
        let pick = |count: usize| frame.min(count.saturating_sub(1));
        (self.places[pick(self.places.len())], self.turns[pick(self.turns.len())])
    }
}

/// A file's take: its bones, parents before children, and how many frames
/// it has at what rate.
#[derive(Debug, Clone)]
pub struct Motion {
    pub take: String,
    pub bones: Vec<MovingBone>,
    pub frames: usize,
    /// Frames to a second.
    pub rate: f64,
    /// Centimetres to a unit of the file.
    pub unit: f64,
}

impl Motion {
    pub fn seconds(&self) -> f64 {
        self.frames.saturating_sub(1) as f64 / self.rate
    }

    pub fn bone(&self, name: &str) -> Option<usize> {
        self.bones.iter().position(|bone| bone.name == name)
    }

    /// Every bone's place and turn in the file's own axes: on a frame, or
    /// (none) as the skeleton was made.
    pub fn placed(&self, frame: Option<usize>) -> Vec<([f64; 3], Turn)> {
        let mut out: Vec<([f64; 3], Turn)> = Vec::with_capacity(self.bones.len());
        for bone in &self.bones {
            let (place, turn) = frame.map_or(bone.rest, |frame| bone.on(frame));
            out.push(match bone.parent {
                Some(parent) => {
                    let (at, facing) = out[parent];
                    let shifted = turned(facing, place);
                    (std::array::from_fn(|k| at[k] + shifted[k]), turns(facing, turn))
                }
                None => (place, turn),
            });
        }
        out
    }
}

/// A curve's value at a time: by a straight line between its keys, and its
/// first or last key's outside them.
pub(super) fn value_at(times: &[i64], values: &[f64], time: i64) -> Option<f64> {
    let after = times.partition_point(|key| *key <= time);
    if after == 0 {
        return values.first().copied();
    }
    if after >= times.len() || after >= values.len() {
        return values.get(times.len().min(values.len()).checked_sub(1)?).copied();
    }
    let (from, to) = (times[after - 1], times[after]);
    let share = (time - from) as f64 / (to - from).max(1) as f64;
    Some(values[after - 1] + (values[after] - values[after - 1]) * share)
}

/// Reads a file's motion: every model as a bone, and its one take.
pub fn read_motion(data: &[u8]) -> Result<Motion> {
    let root = parse(data)?;
    let objects = root.child("Objects").ok_or_else(|| anyhow!("the file has no objects"))?;
    let models: Vec<&Record> = objects.all("Model").collect();
    let model_at: HashMap<i64, usize> = models.iter().enumerate().map(|(index, record)| (id_of(record), index)).collect();
    // A curve node by its id: what its three curves stand at where it has none.
    let nodes: HashMap<i64, [Option<f64>; 3]> = objects
        .all("AnimationCurveNode")
        .map(|node| (id_of(node), ["d|X", "d|Y", "d|Z"].map(|axis| node.property(axis).and_then(|v| v.first()?.as_f64()))))
        .collect();
    let curves: HashMap<i64, (&[i64], &[f64])> = objects.all("AnimationCurve").filter_map(|curve| Some((id_of(curve), (curve.ints("KeyTime")?, curve.floats("KeyValueFloat")?)))).collect();

    // What hangs from what, which node drives which property of which
    // model, and which curve is which axis of which node.
    let mut parent_of: HashMap<usize, usize> = HashMap::new();
    let mut driven: HashMap<(usize, &str), i64> = HashMap::new();
    let mut axes: HashMap<i64, [Option<i64>; 3]> = HashMap::new();
    for link in root.child("Connections").into_iter().flat_map(|c| c.all("C")) {
        let (Some(kind), Some(child), Some(parent)) = (link.props.first().and_then(Prop::as_str), link.props.get(1).and_then(Prop::as_i64), link.props.get(2).and_then(Prop::as_i64)) else { continue };
        let property = link.props.get(3).and_then(Prop::as_str);
        match (kind, property) {
            ("OO", _) => {
                if let (Some(&child), Some(&parent)) = (model_at.get(&child), model_at.get(&parent)) {
                    parent_of.insert(child, parent);
                }
            }
            ("OP", Some(property)) if nodes.contains_key(&child) => {
                if let Some(&model) = model_at.get(&parent) {
                    driven.insert((model, property), child);
                }
            }
            ("OP", Some(property)) if curves.contains_key(&child) => {
                if let Some(axis) = ["d|X", "d|Y", "d|Z"].iter().position(|name| *name == property) {
                    axes.entry(parent).or_default()[axis] = Some(child);
                }
            }
            _ => {}
        }
    }

    // The take's frames: from the first key of any curve to the last, a
    // frame to the shortest gap between two keys of the curve with most.
    let keyed = curves.values().map(|(times, _)| *times).filter(|times| times.len() > 1);
    let (first, last) = keyed.clone().fold((i64::MAX, i64::MIN), |(first, last), times| (first.min(times[0]), last.max(times[times.len() - 1])));
    let gap = keyed.max_by_key(|times| times.len()).and_then(|times| times.windows(2).map(|pair| pair[1] - pair[0]).filter(|gap| *gap > 0).min());
    let (frames, gap) = match gap {
        Some(gap) => (((last - first) as f64 / gap as f64).round() as usize + 1, gap),
        None => (1, TICKS_A_SECOND as i64 / 30),
    };

    // Parents before children.
    let mut order: Vec<usize> = Vec::with_capacity(models.len());
    let mut placed = vec![usize::MAX; models.len()];
    while order.len() < models.len() {
        let before = order.len();
        for index in 0..models.len() {
            if placed[index] == usize::MAX && parent_of.get(&index).is_none_or(|parent| placed[*parent] != usize::MAX) {
                placed[index] = order.len();
                order.push(index);
            }
        }
        ensure!(order.len() > before, "the file's models hang from one another in a ring");
    }

    let mut bones = Vec::with_capacity(models.len());
    for &index in &order {
        let record = models[index];
        let name = name_of(record);
        let get = |property: &str| record.property_vec3(property).unwrap_or([0.0; 3]);
        ensure!(record.property("RotationOrder").and_then(|v| v.first()?.as_i64()).unwrap_or(0) == 0, "{name} turns in an order other than x, y, z");
        ensure!(get("RotationPivot") == [0.0; 3] && get("RotationOffset") == [0.0; 3], "{name} turns about a pivot of its own");
        let (before, after) = (turn_of(get("PreRotation")), undone(turn_of(get("PostRotation"))));
        let whole = |angles: [f64; 3]| turns(before, turns(turn_of(angles), after));
        // A property's three values on each frame, where a node drives it.
        let track = |property: &str, rest: [f64; 3]| -> Option<Vec<[f64; 3]>> {
            let node = driven.get(&(index, property))?;
            let (stands, feeds) = (nodes[node], axes.get(node).copied().unwrap_or_default());
            Some(
                (0..frames)
                    .map(|frame| {
                        let time = first + gap * frame as i64;
                        std::array::from_fn(|axis| feeds[axis].and_then(|curve| curves.get(&curve)).and_then(|(times, values)| value_at(times, values, time)).or(stands[axis]).unwrap_or(rest[axis]))
                    })
                    .collect(),
            )
        };
        let (place, angles) = (get("Lcl Translation"), get("Lcl Rotation"));
        bones.push(MovingBone {
            name,
            parent: parent_of.get(&index).map(|parent| placed[*parent]),
            rest: (place, whole(angles)),
            places: track("Lcl Translation", place).unwrap_or_else(|| vec![place]),
            turns: track("Lcl Rotation", angles).map_or_else(|| vec![whole(angles)], |frames| frames.into_iter().map(whole).collect()),
        });
    }

    let take = objects.child("AnimationStack").map(name_of).unwrap_or_default();
    let unit = root.child("GlobalSettings").and_then(|s| s.property("UnitScaleFactor")).and_then(|v| v.first()?.as_f64()).unwrap_or(1.0);
    Ok(Motion { take, bones, frames, rate: TICKS_A_SECOND / gap as f64, unit })
}
