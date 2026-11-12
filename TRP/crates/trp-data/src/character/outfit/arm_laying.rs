//! Laying an arm's bones along the body's: bases, laid bones and hangs.

use super::*;

/// Three directions at right angles: to the character's right, up and
/// ahead.
#[derive(Clone, Copy)]
pub(super) struct Basis {
    pub(super) right: V3,
    pub(super) up: V3,
    pub(super) ahead: V3,
}

impl Basis {
    /// From where two things lie to the left and to the right, and one
    /// low and one high. A model stands along the axes of its own space,
    /// so each direction is the axis nearest to it: a head a little
    /// behind the hips does not tip what is up.
    pub(super) fn of(left: V3, right: V3, low: V3, high: V3) -> Option<Self> {
        let axis = |v: V3| -> Option<V3> {
            let most = (0..3).max_by(|a, b| v[*a].abs().total_cmp(&v[*b].abs()))?;
            let mut along = [0.0; 3];
            along[most] = v[most].signum();
            (v[most].abs() > 1e-6).then_some(along)
        };
        let (right, up) = (axis(sub(right, left))?, axis(sub(high, low))?);
        (dot(right, up) == 0.0).then(|| Self { right, up, ahead: cross(right, up) })
    }

    /// A vector's three parts along them.
    pub(super) fn parts(&self, v: V3) -> V3 {
        [dot(v, self.right), dot(v, self.up), dot(v, self.ahead)]
    }

    /// The vector that has those parts.
    pub(super) fn whole(&self, parts: V3) -> V3 {
        add(add(scaled(self.right, parts[0]), scaled(self.up, parts[1])), scaled(self.ahead, parts[2]))
    }
}

/// Three axes at right angles, the first along one direction and the
/// second as near another as that leaves it.
pub(super) fn frame(along: V3, toward: V3) -> Option<[V3; 3]> {
    let x = unit(along)?;
    let y = unit(sub(toward, scaled(x, dot(toward, x))))?;
    Some([x, y, cross(x, y)])
}

/// A vector turned about an axis of length one.
pub(super) fn turned(v: V3, axis: V3, angle: f32) -> V3 {
    let (sin, cos) = angle.sin_cos();
    add(add(scaled(v, cos), scaled(cross(axis, v), sin)), scaled(axis, dot(axis, v) * (1.0 - cos)))
}

/// How a piece of the model about one of its bones is laid at rest on the
/// outfit's rig: the bone's joint goes to a place, and its axes are turned
/// to the pose the install's clothing is made in. **Nothing is stretched**
/// but a finger seen by its own eyes: the piece keeps its size and its
/// shape. With a twist, what lies further along the bone is turned about
/// it by so much more, up to that angle at its far end (a forearm, which
/// turns with the hand).
#[derive(Clone, Copy)]
pub(super) struct Laid {
    pub(super) from: V3,
    pub(super) to: V3,
    pub(super) ours: [V3; 3],
    pub(super) theirs: [V3; 3],
    /// The length of the model's bone.
    pub(super) long: f32,
    pub(super) twist: f32,
    /// How many times as long the piece is made along its bone, and no
    /// broader: one everywhere but a finger's joint of arms seen by their
    /// own eyes, which is made as long as the install's (`lay_arm`).
    pub(super) stretch: f32,
}

impl Laid {
    /// A bone from its joint towards the next, laid from a place along a
    /// direction, with a second direction on each side to settle the turn
    /// about it.
    pub(super) fn bone(from: V3, next: V3, toward: V3, to: V3, their_along: V3, their_toward: V3) -> Option<Self> {
        let (ours, theirs) = (frame(sub(next, from), toward)?, frame(their_along, their_toward)?);
        Some(Self { from, to, ours, theirs, long: length(sub(next, from)), twist: 0.0, stretch: 1.0 })
    }

    /// The same turn about another joint: a bone with none after it, laid
    /// as the one before it, at its own length.
    pub(super) fn carried_to(&self, from: V3, to: V3) -> Self {
        Self { from, to, twist: 0.0, stretch: 1.0, ..*self }
    }

    /// The same laying, the piece made as long along its bone as takes
    /// the bone's end to a place that many metres from its joint.
    pub(super) fn as_long_as(self, long: f32) -> Self {
        Self { stretch: long / self.long.max(1e-6), ..self }
    }

    /// How far along the bone a point is, from none at its joint to one
    /// at the next.
    pub(super) fn share_along(&self, p: V3) -> f32 {
        (dot(sub(p, self.from), self.ours[0]) / self.long.max(1e-6)).clamp(0.0, 1.0)
    }

    pub(super) fn turn(&self, v: V3, along: f32) -> V3 {
        let v = if self.twist != 0.0 { turned(v, self.ours[0], self.twist * along) } else { v };
        add(add(scaled(self.theirs[0], dot(v, self.ours[0])), scaled(self.theirs[1], dot(v, self.ours[1]))), scaled(self.theirs[2], dot(v, self.ours[2])))
    }

    pub(super) fn place(&self, p: V3) -> V3 {
        let from_joint = sub(p, self.from);
        let longer = scaled(self.ours[0], dot(from_joint, self.ours[0]) * (self.stretch - 1.0));
        add(self.to, self.turn(add(from_joint, longer), self.share_along(p)))
    }

    pub(super) fn normal(&self, p: V3, n: V3) -> V3 {
        self.turn(n, self.share_along(p))
    }

    /// The angle by which another laying turns things about this bone
    /// more than this one does: the hand's about the forearm.
    pub(super) fn twist_to(&self, other: &Laid) -> f32 {
        // One of this bone's cross axes as the other laying leaves it,
        // seen in the axes this one is laid to.
        let theirs = other.turn(self.ours[1], 0.0);
        dot(theirs, self.theirs[2]).atan2(dot(theirs, self.theirs[1]))
    }
}

/// The names a model has for an arm's bones.
pub(super) struct ArmNames {
    pub(super) upper: String,
    /// The forearm's bone, and any other whose share is the forearm's (a
    /// bone some models have for its turn).
    pub(super) fore: Vec<String>,
    pub(super) hand: String,
    /// The fingers, each with its three joints from the knuckle out and
    /// the number the install's skeleton has for it (the thumb is 1).
    pub(super) fingers: Vec<([String; 3], u8)>,
    /// The finger by whose knuckle the hand is laid, and the one whose
    /// knuckle beside that settles the hand's turn.
    pub(super) by: usize,
    pub(super) beside: usize,
}

/// What an arm hangs from.
#[derive(Clone, Copy)]
pub(super) enum Hang {
    /// A place (the model's own shoulder), on a bone of the rig: the arm
    /// reaches from there to the hand with its own two lengths, and the
    /// elbow is wherever that puts it.
    From { at: V3, collar: usize },
    /// The hand: the arm goes back from it, the forearm and then the
    /// upper arm each lying as the install's lies, and its shoulder is
    /// wherever that puts it. For arms seen by their own eyes, of which
    /// the hand and the forearm are in the picture and the shoulder is
    /// not.
    Hand,
}

/// A bone of the install's put on a rig, with the outfit's joint for it.
pub(super) fn add_bone(rig: &mut Rig, skeleton: &[RestBone], name: &str, joint: V3, follows: Follows) -> Result<usize> {
    let (index, their_joint) = theirs(skeleton, &format!("Base Human{name}"))?;
    rig.bones.push(RigBone { path: skeleton[index].path.clone(), bind: skeleton[index].bind, joint, their_joint, follows });
    Ok(rig.bones.len() - 1)
}

/// Lays one arm of a model at rest and puts its bones on the rig.
///
/// Its hand is the install's, for it is on the weapon. **The hand is laid
/// by its knuckles**: the knuckle of one finger on the install's, the
/// hand turned as the install's (along the wrist to that knuckle, the
/// next finger's knuckle beside it), and its wrist wherever that leaves
/// it, which two skeletons with the wrist's joint at other places in the
/// hand do not agree on. The rest of the arm goes by what it hangs from
/// (`Hang`).
///
/// **The fingers of arms seen by their own eyes are the install's
/// fingers**: every joint of every finger, the thumb's too, is put on the
/// install's own joint, and each piece of a finger made as long as the
/// install's (and no broader) so that the next joint falls on the next.
/// Every hold of the install's weapons is a pose of those joints; a
/// finger whose joints are elsewhere, turned by the same angles, ends
/// elsewhere, and the further the more it is bent: a thumb whose first
/// bone is a third longer and begins two centimetres away wraps a grip
/// two centimetres from where the install's thumb lies. A body seen from
/// outside keeps its fingers as they are.
pub(super) fn lay_arm(ours: &dyn Fn(&str) -> Result<V3>, skeleton: &[RestBone], rig: &mut Rig, goes: &mut HashMap<String, Goes>, their_side: &str, names: &ArmNames, hang: Hang) -> Result<()> {
    let missing = || anyhow!("two joints of an arm are at one place");
    let named = |name: &str| format!("{their_side}{name}");
    let their = |name: &str| theirs(skeleton, &format!("Base Human{their_side}{name}")).map(|(_, at)| at);
    let (shoulder, elbow, wrist) = (ours(&names.upper)?, ours(&names.fore[0])?, ours(&names.hand)?);
    let (their_shoulder, their_elbow, their_wrist) = (their("Upperarm")?, their("Forearm1")?, their("Palm")?);
    let digit = |finger: usize, joint: usize| their(&format!("Digit{}{}", names.fingers[finger].1, joint + 1));

    // The hand.
    let (knuckle, beside) = (ours(&names.fingers[names.by].0[0])?, ours(&names.fingers[names.beside].0[0])?);
    let (their_knuckle, their_beside) = (digit(names.by, 0)?, digit(names.beside, 0)?);
    let (across, their_across) = (sub(beside, knuckle), sub(their_beside, their_knuckle));
    // **A hand of five fingers seen by its own eyes is laid by the whole
    // row of its knuckles and by its wrist**: the middle of the line from
    // the first finger's knuckle to the little finger's on the middle of
    // the install's, that line along the install's, and the hand turned
    // about it towards the install's wrist. Laid by two neighbouring
    // knuckles alone, a hand whose row of knuckles is bent a little
    // otherwise than the install's is rolled about its own length: the
    // one model measured stood a centimetre and more out of the
    // install's hand at the little finger.
    //
    // **And it bends at the install's wrist.** The wrist is taken where
    // that laying has the install's own, wherever the model's maker put
    // the joint (the one model measured has it two centimetres inside
    // the palm, and its skin bends behind it, where this puts it). So the
    // forearm comes away from the install's own wrist along the install's
    // own forearm, and is turned about that wrist; turned about a joint
    // inside the palm, it left the install's forearm by as much as the
    // wrist was bent.
    let of_number = |number: u8| names.fingers.iter().position(|finger| finger.1 == number);
    let (hand, wrist) = match (hang, of_number(2), of_number(5)) {
        (Hang::Hand, Some(first), Some(little)) => {
            let (first_at, little_at) = (ours(&names.fingers[first].0[0])?, ours(&names.fingers[little].0[0])?);
            let (their_first, their_little) = (digit(first, 0)?, digit(little, 0)?);
            let (middle, their_middle) = (scaled(add(first_at, little_at), 0.5), scaled(add(their_first, their_little), 0.5));
            let axes = frame(sub(little_at, first_at), sub(wrist, middle)).ok_or_else(missing)?;
            let their_axes = frame(sub(their_little, their_first), sub(their_wrist, their_middle)).ok_or_else(missing)?;
            let hand = Laid { from: middle, to: their_middle, ours: axes, theirs: their_axes, long: 1.0, twist: 0.0, stretch: 1.0 };
            let back = sub(their_wrist, their_middle);
            (hand, add(middle, add(scaled(axes[0], dot(back, their_axes[0])), scaled(axes[1], dot(back, their_axes[1])))))
        }
        _ => {
            let mut hand = Laid::bone(wrist, knuckle, across, their_wrist, sub(their_knuckle, their_wrist), their_across).ok_or_else(missing)?;
            hand.to = add(hand.to, sub(their_knuckle, hand.place(knuckle)));
            (hand, wrist)
        }
    };
    let wrist_at = hand.place(wrist);

    // The upper arm and the forearm, the elbow's axis (at right angles to
    // both) settling the turn about each; and the forearm twisted towards
    // the hand along its length. From a shoulder, the elbow is where the
    // model's own two lengths put it, bent to the side the install's arm
    // is bent at rest; from the hand, each bone lies as the install's.
    let (upper_long, fore_long) = (length(sub(elbow, shoulder)), length(sub(wrist, elbow)));
    let (shoulder_at, elbow_at) = match hang {
        Hang::From { at, .. } => (at, bend(at, wrist_at, upper_long, fore_long, off_line(their_shoulder, their_elbow, their_wrist), [0.0; 3])),
        Hang::Hand => {
            let (their_upper, their_fore) = (unit(sub(their_elbow, their_shoulder)).ok_or_else(missing)?, unit(sub(their_wrist, their_elbow)).ok_or_else(missing)?);
            let elbow_at = sub(wrist_at, scaled(their_fore, fore_long));
            (sub(elbow_at, scaled(their_upper, upper_long)), elbow_at)
        }
    };
    let fore_along = unit(sub(wrist_at, elbow_at)).ok_or_else(missing)?;
    let hinge = cross(sub(elbow, shoulder), sub(wrist, elbow));
    let hinge_at = cross(sub(elbow_at, shoulder_at), fore_along);
    let upper = Laid::bone(shoulder, elbow, hinge, shoulder_at, sub(elbow_at, shoulder_at), hinge_at).ok_or_else(missing)?;
    let mut fore = Laid::bone(elbow, wrist, hinge, elbow_at, fore_along, hinge_at).ok_or_else(missing)?;
    fore.twist = fore.twist_to(&hand);
    let along_fore = |share: f32| add(elbow_at, scaled(fore_along, fore_long * share));
    let first = rig.bones.len();
    let (upper_bone, fore_bone, palm) = match hang {
        Hang::From { collar, .. } => {
            add_bone(rig, skeleton, &named("Upperarm"), shoulder_at, Follows::Limb { from: collar, lower: first + 1, end: first + 4 })?;
            add_bone(rig, skeleton, &named("Forearm1"), elbow_at, Follows::Lower)?;
            for (name, share) in [("Forearm2", 1.0 / 3.0), ("Forearm3", 2.0 / 3.0)] {
                add_bone(rig, skeleton, &named(name), along_fore(share), Follows::Along { of: first + 1 })?;
            }
            add_bone(rig, skeleton, &named("Palm"), wrist_at, Follows::Theirs)?;
            (first, first + 1, first + 4)
        }
        Hang::Hand => {
            add_bone(rig, skeleton, &named("Palm"), wrist_at, Follows::Theirs)?;
            for (name, share) in [("Forearm1", 0.0), ("Forearm2", 1.0 / 3.0), ("Forearm3", 2.0 / 3.0)] {
                add_bone(rig, skeleton, &named(name), along_fore(share), Follows::Held { from: first, at: first })?;
            }
            add_bone(rig, skeleton, &named("Upperarm"), shoulder_at, Follows::Held { from: first + 1, at: first + 1 })?;
            (first + 4, first + 1, first)
        }
    };
    goes.insert(names.upper.clone(), Goes::To(upper, upper_bone));
    for name in &names.fore {
        goes.insert(name.clone(), Goes::Along(fore, [Some(fore_bone), Some(fore_bone + 1), Some(fore_bone + 2)]));
    }
    goes.insert(names.hand.clone(), Goes::To(hand, palm));

    // The fingers: each joint hangs where the piece before it carries it
    // and lies along the install's joint at rest; the last as the one
    // before it. A finger's turn about itself is settled by the line of
    // the knuckles; the thumb lies along that line, so its turn is
    // settled by the hand's own length.
    let (along_hand, their_along_hand) = (sub(knuckle, wrist), sub(their_knuckle, their_wrist));
    for (finger, (joint_names, number)) in names.fingers.iter().enumerate() {
        let (toward, their_toward) = if *number == 1 { (along_hand, their_along_hand) } else { (across, their_across) };
        let joints = [ours(&joint_names[0])?, ours(&joint_names[1])?, ours(&joint_names[2])?];
        let digits = [digit(finger, 0)?, digit(finger, 1)?, digit(finger, 2)?];
        let (first, second) = match hang {
            Hang::Hand => {
                let first = Laid::bone(joints[0], joints[1], toward, digits[0], sub(digits[1], digits[0]), their_toward).ok_or_else(missing)?;
                let second = Laid::bone(joints[1], joints[2], toward, digits[1], sub(digits[2], digits[1]), their_toward).ok_or_else(missing)?;
                (first.as_long_as(length(sub(digits[1], digits[0]))), second.as_long_as(length(sub(digits[2], digits[1]))))
            }
            Hang::From { .. } => {
                let first = Laid::bone(joints[0], joints[1], toward, hand.place(joints[0]), sub(digits[1], digits[0]), their_toward).ok_or_else(missing)?;
                let second = Laid::bone(joints[1], joints[2], toward, first.place(joints[1]), sub(digits[2], digits[1]), their_toward).ok_or_else(missing)?;
                (first, second)
            }
        };
        let last = second.carried_to(joints[2], second.place(joints[2]));
        let mut from = palm;
        for (joint, laid) in [first, second, last].into_iter().enumerate() {
            from = add_bone(rig, skeleton, &named(&format!("Digit{number}{}", joint + 1)), laid.to, Follows::Hung { from })?;
            goes.insert(joint_names[joint].clone(), Goes::To(laid, from));
        }
    }
    Ok(())
}
