//! Tagging bodies and collecting the touches PhysX reports.

use super::*;

/// The mark on a shape's filter (its second word) of a body that reports
/// its touches (`World::report_touches`).
pub(super) const REPORTS_TOUCHES: u32 = 1;

/// What an actor is to its world, kept in its user data: a body or a still
/// thing by its place, or nothing.
pub(super) const TAG_BODY: usize = 1;

pub(super) const TAG_STILL: usize = 2;

pub(super) fn tag(kind: usize, index: usize) -> *mut std::ffi::c_void {
    ((index << 2) | kind) as *mut std::ffi::c_void
}

pub(super) unsafe fn tagged(actor: *mut PxRigidActor) -> Option<Met> {
    if actor.is_null() {
        return None;
    }
    let tag = (*(actor as *mut PxActor)).userData as usize;
    match tag & 3 {
        TAG_BODY => Some(Met::Body(Body(tag >> 2))),
        TAG_STILL => Some(Met::Still(tag >> 2)),
        _ => None,
    }
}

/// What a body that reports its touches met, as the engine first found
/// them touching (Unity's `OnCollisionEnter`: once a pair of shapes, as
/// the pair begins to touch, by the discrete step or by a sweep;
/// `PROVISIONAL`: how Unity reports a touch a sweep found is not read, and
/// it is taken as a touch like any other).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Touch {
    pub body: Body,
    pub other: Met,
    /// The pair's first point of contact, and the normal there, pointing
    /// out of what it met towards the body; none where the engine gave no
    /// point.
    pub contact: Option<(V3, V3)>,
}

/// What was met: another body, or a still thing of the world by the order
/// it was put in (`add_static` and `add_ground` count together).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Met {
    Body(Body),
    Still(usize),
}

/// One of the shapes a swept box touched (`World::sweep_box_touches`): how
/// far along the way, the surface's normal and the point (where the box
/// begins inside the shape, the distance nought, the normal against the way
/// and the point not given), what it was, and that thing's layer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SweepTouch {
    pub distance: f32,
    pub normal: V3,
    pub point: V3,
    pub met: Option<Met>,
    pub layer: u16,
}

/// How many touches a query keeps before it hands them on (Unity's own
/// buffer behind every `…NonAlloc` cast: `SweepCallbackNonAlloc`, 128).
pub(super) const TOUCHES_AT_ONCE: usize = 128;

/// The engine handing a query's touches on (`PxHitCallback::processTouches`):
/// each kept in the order given, and the query goes on.
pub(super) unsafe extern "C" fn keep_touches(hits: *const PxSweepHit, count: u32, user: *mut std::ffi::c_void) -> bool {
    if let (Some(kept), false) = ((user as *mut Vec<PxSweepHit>).as_mut(), hits.is_null()) {
        kept.extend_from_slice(std::slice::from_raw_parts(hits, count as usize));
    }
    true
}

pub(super) unsafe extern "C" fn touches_done(_: *mut std::ffi::c_void) {}

/// The engine's report of the pairs that began to touch in a step, kept as
/// touches of the bodies that asked (`World::take_touches`). Called from
/// within `World::step`, on its thread.
pub(super) unsafe extern "C" fn on_contact(user: *mut std::ffi::c_void, header: *const PxContactPairHeader, pairs: *const PxContactPair, count: u32) {
    let (Some(touches), Some(header)) = ((user as *mut Vec<Touch>).as_mut(), header.as_ref()) else { return };
    let removed = (PxContactPairHeaderFlag::eREMOVED_ACTOR_0 | PxContactPairHeaderFlag::eREMOVED_ACTOR_1) as u16;
    if header.flags.mBits & removed != 0 || pairs.is_null() {
        return;
    }
    let sides = [tagged(header.actors[0]), tagged(header.actors[1])];
    let began = (PxPairFlag::eNOTIFY_TOUCH_FOUND | PxPairFlag::eNOTIFY_TOUCH_CCD) as u16;
    let gone = (PxContactPairFlag::eREMOVED_SHAPE_0 | PxContactPairFlag::eREMOVED_SHAPE_1) as u16;
    for pair in std::slice::from_raw_parts(pairs, count as usize) {
        if pair.events.mBits & began == 0 || pair.flags.mBits & gone != 0 {
            continue;
        }
        let mut point: PxContactPairPoint = std::mem::zeroed();
        let found = PxContactPair_extractContacts(pair, &mut point, 1) > 0;
        for side in 0..2 {
            let (Some(Met::Body(body)), Some(other)) = (sides[side], sides[1 - side]) else { continue };
            if PxShape_getSimulationFilterData(pair.shapes[side]).word1 & REPORTS_TOUCHES == 0 {
                continue;
            }
            // The engine's normal is the way the pair's first shape must
            // go to leave the second.
            let sign = if side == 0 { 1.0 } else { -1.0 };
            let contact = found.then(|| ([point.position.x, point.position.y, point.position.z], [point.normal.x * sign, point.normal.y * sign, point.normal.z * sign]));
            touches.push(Touch { body, other, contact });
        }
    }
}
