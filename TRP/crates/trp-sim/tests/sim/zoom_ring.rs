//! A variable scope's ring turned by its zoom (`trp_sim::sights::Sights::
//! ring_turns`; optics spec, "The ring"): the SPEAR's Tango6T the long way
//! round (`InverseRotation`), the VUDU on the SPEAR's mount the short way
//! with its start turned over; the game's own slerp. Those that need the
//! install pass trivially, with a note, where none is configured.

use trp_sim::math::quat_mul;
use trp_sim::sights::{game_slerp, Sights};

const SPEAR: &str = "65290f395ae2ae97b80fdf2d";
/// The SPEAR with the VUDU on its Alpha4 mount (no preset carries it).
const SPEAR_VUDU: &str = "65290f395ae2ae97b80fdf2d[mod_reciever=6529119424cbe3c74a05e5bb[mod_scope=6567e751a715f85433025998[mod_scope=5b3b99475acfc432ff4dcbee[]]]]";

/// The turn from `a` to `b` (`a⁻¹ b`) as the lesser of its two ways: its
/// angle in degrees and its axis, in `a`'s own axes.
fn turn_between(a: [f32; 4], b: [f32; 4]) -> (f32, [f32; 3]) {
    let mut rel = quat_mul([-a[0], -a[1], -a[2], a[3]], b);
    if rel[3] < 0.0 {
        rel = rel.map(|v| -v);
    }
    let s = (rel[0] * rel[0] + rel[1] * rel[1] + rel[2] * rel[2]).sqrt();
    let axis = if s > 1e-6 { [rel[0] / s, rel[1] / s, rel[2] / s] } else { [0.0; 3] };
    (2.0 * s.atan2(rel[3]).to_degrees(), axis)
}

fn sights_of(key: &str) -> Option<Sights> {
    let install = crate::fixture::install()?;
    let db = crate::fixture::database()?;
    let mut assets = trp_data::unity::Assets::new(install).expect("manifest");
    let weapon = trp_data::weapon::assemble_key(&mut assets, db, key).expect("the weapon");
    Some(Sights::of_weapon(&weapon).expect("its sights"))
}

/// The Tango6T's ring: at its start pivot's turn at the widest view, its
/// end pivot's after the magnification key's snap to the narrowest; half
/// way it has turned 92.7 degrees about its own +y, the long way (the
/// short way would be 87.3 about −y).
#[test]
fn the_tangos_ring_turns_the_long_way_with_its_zoom() {
    let Some(mut sights) = sights_of(SPEAR) else {
        eprintln!("no install configured: skipped");
        return;
    };
    let rings: Vec<(usize, [f32; 4])> = sights.ring_turns().collect();
    assert_eq!(rings.len(), 1, "one variable scope: {rings:?}");
    let ring = sights.parts().iter().find_map(|part| part.zoom_ring).expect("the Tango's ring");
    assert!(ring.inverse, "the Tango turns the way its pivots stand");
    assert_eq!(rings[0].0, ring.node);
    let (angle, _) = turn_between(ring.start, rings[0].1);
    assert!(angle < 0.01, "at the widest the start pivot's turn: {angle} degrees off");
    assert!(sights.change_magnification(), "the key snaps the zoom");
    let now = sights.ring_turns().next().expect("the ring").1;
    let (angle, _) = turn_between(ring.end, now);
    assert!(angle < 0.01, "at the narrowest the end pivot's turn: {angle} degrees off");
    let half = game_slerp(ring.start, ring.end, 0.5, !ring.inverse);
    let (angle, axis) = turn_between(ring.start, half);
    assert!((angle - 92.7).abs() < 0.1, "half way: {angle} degrees");
    assert!(axis[1] > 0.999, "about its own +y: {axis:?}");
    let (whole, _) = turn_between(ring.start, ring.end);
    assert!((whole - 174.6).abs() < 0.1, "its pivots stand 174.6 degrees apart the short way: {whole}");
}

/// The VUDU's ring goes the short way: its start pivot turned over, 163
/// degrees in all about its own +y, 81.5 half way; at either end its
/// pivots' turns.
#[test]
fn the_vudus_ring_turns_the_short_way_with_its_zoom() {
    let Some(mut sights) = sights_of(SPEAR_VUDU) else {
        eprintln!("no install configured: skipped");
        return;
    };
    let ring = sights.parts().iter().find_map(|part| part.zoom_ring).expect("the VUDU's ring");
    assert!(!ring.inverse, "the VUDU's goes the short way");
    let dot: f32 = (0..4).map(|k| ring.start[k] * ring.end[k]).sum();
    assert!(dot < 0.0, "its pivots stand more than half a turn apart as stored: {dot}");
    let at_widest = sights.ring_turns().next().expect("the ring").1;
    assert!(turn_between(ring.start, at_widest).0 < 0.01, "at the widest its start");
    let half = game_slerp(ring.start, ring.end, 0.5, !ring.inverse);
    let (angle, axis) = turn_between(ring.start, half);
    assert!((angle - 81.5).abs() < 0.1, "half way: {angle} degrees");
    assert!(axis[1] > 0.999, "about its own +y: {axis:?}");
    assert!((turn_between(ring.start, ring.end).0 - 163.0).abs() < 0.1);
    assert!(sights.change_magnification(), "the key snaps the zoom");
    let now = sights.ring_turns().next().expect("the ring").1;
    assert!(turn_between(ring.end, now).0 < 0.01, "at the narrowest its end");
}

/// The slerp as the game writes it: its ends; the arc as the two stand
/// where the long way is asked; on the short way the same turn either sign.
#[test]
fn the_games_slerp_keeps_its_ends_and_its_way() {
    let half_turn_about_y = |degrees: f32| {
        let r = (degrees * 0.5).to_radians();
        [0.0, r.sin(), 0.0, r.cos()]
    };
    let p = [0.0, 0.0, 0.0, 1.0];
    let q = half_turn_about_y(200.0);
    assert!(q[3] < 0.0, "200 degrees stands more than half a turn away");
    for t in [0.0, 1.0] {
        let r = game_slerp(p, q, t, false);
        let end = if t == 0.0 { p } else { q };
        assert!((0..4).all(|k| (r[k] - end[k]).abs() < 1e-5), "t {t}: {r:?}");
    }
    // The long way: 100 degrees about +y half way.
    let (angle, axis) = turn_between(p, game_slerp(p, q, 0.5, false));
    assert!((angle - 100.0).abs() < 0.01 && axis[1] > 0.999, "{angle} {axis:?}");
    // The short way: 80 degrees about −y.
    let (angle, axis) = turn_between(p, game_slerp(p, q, 0.5, true));
    assert!((angle - 80.0).abs() < 0.01 && axis[1] < -0.999, "{angle} {axis:?}");
    // The same turn twice gives it back.
    assert_eq!(game_slerp(p, p, 0.3, true), p);
}
