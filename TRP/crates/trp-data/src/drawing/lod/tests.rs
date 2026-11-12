//! Tests of the levels of detail: a renderer's levels and TRP's own measure of a coarser level.

use super::*;

fn group(levels: &[(f32, &[i64])]) -> LodGroup {
    let at = |path_id: i64| ObjRef { file: 0, path_id };
    LodGroup {
        id: None,
        object: None,
        size: 2.0,
        reference: [0.0; 3],
        fade_mode: 0,
        animate_cross_fading: false,
        levels: levels.iter().map(|(height, renderers)| LodLevel { height: *height, fade_width: 0.0, renderers: renderers.iter().map(|&id| at(id)).collect() }).collect(),
        deviation: Vec::new(),
    }
}

#[test]
fn a_renderer_has_the_levels_that_list_it() {
    let lods = group(&[(0.6, &[1, 3]), (0.015, &[2, 3])]);
    assert_eq!(lods.levels_of(ObjRef { file: 0, path_id: 1 }), 0b01);
    assert_eq!(lods.levels_of(ObjRef { file: 0, path_id: 2 }), 0b10);
    assert_eq!(lods.levels_of(ObjRef { file: 0, path_id: 3 }), 0b11);
    assert_eq!(lods.levels_of(ObjRef { file: 0, path_id: 4 }), 0);
    assert_eq!(lods.heights(), vec![0.6, 0.015]);
    assert!(!lods.governs_nothing());
    assert!(group(&[(0.5, &[]), (0.1, &[])]).governs_nothing());
}

/// A square of `n` by `n` cells (two triangles each) of side `side` in
/// the plane z = `height(x, y)`.
fn sheet(n: usize, side: f32, height: &dyn Fn(f32, f32) -> f32) -> Shape {
    let mut shape = Shape::default();
    for j in 0..=n {
        for i in 0..=n {
            let (x, y) = (side * i as f32 / n as f32, side * j as f32 / n as f32);
            shape.points.push([x, y, height(x, y)]);
        }
    }
    let at = |i: usize, j: usize| (j * (n + 1) + i) as u32;
    for j in 0..n {
        for i in 0..n {
            shape.triangles.push([at(i, j), at(i + 1, j), at(i + 1, j + 1)]);
            shape.triangles.push([at(i, j), at(i + 1, j + 1), at(i, j + 1)]);
        }
    }
    shape
}

#[test]
fn a_coarser_flat_level_is_no_deviation() {
    // The same flat square made of 32 by 32 cells and of 2 by 2: every
    // point of either lies on the other.
    let (fine, coarse) = (sheet(32, 1.0, &|_, _| 0.0), sheet(2, 1.0, &|_, _| 0.0));
    assert!(deviation(&fine, &coarse) < 1e-5, "{}", deviation(&fine, &coarse));
    // Each level against the fullest, their outlines closed alike.
    let levels = deviations(&[fine.clone(), coarse, fine]);
    assert_eq!(levels.len(), 3);
    assert!(levels[0] == 0.0 && levels[1] < 1e-5 && levels[2] < 1e-5, "{levels:?}");
}

#[test]
fn a_bump_left_out_is_its_height() {
    // A square with a tent 3 cm high at its middle, and the flat square:
    // the deviation is the tent's height (its peak is a corner of the
    // fine mesh); lifted 1 cm, the coarse one is 2 cm from the peak and
    // 1 cm from the flat rim, so 2 cm.
    let tent = |x: f32, y: f32| (0.03 - 0.12 * ((x - 0.5).abs().max((y - 0.5).abs()))).max(0.0);
    let fine = sheet(16, 1.0, &tent);
    let flat = sheet(1, 1.0, &|_, _| 0.0);
    assert!((deviation(&fine, &flat) - 0.03).abs() < 1e-4, "{}", deviation(&fine, &flat));
    let lifted = sheet(1, 1.0, &|_, _| 0.01);
    assert!((deviation(&fine, &lifted) - 0.02).abs() < 1e-4, "{}", deviation(&fine, &lifted));
    // Each level against the fullest, never less than a finer one's.
    let levels = deviations(&[fine.clone(), lifted.clone(), sheet(4, 1.0, &tent)]);
    assert_eq!(levels.len(), 3);
    assert_eq!(levels[0], 0.0);
    assert!((levels[1] - 0.02).abs() < 1e-4);
    assert!(levels[2] >= levels[1]);
}

#[test]
fn a_part_left_out_counts_from_both_sides() {
    // The fine level has a second square 10 cm above the first that the
    // coarse leaves out: 10 cm, though every coarse point lies on the
    // fine surface.
    let mut fine = sheet(4, 1.0, &|_, _| 0.0);
    let above = sheet(2, 0.2, &|_, _| 0.1);
    let base = fine.points.len() as u32;
    fine.points.extend(above.points.iter().copied());
    fine.triangles.extend(above.triangles.iter().map(|t| t.map(|i| i + base)));
    let coarse = sheet(1, 1.0, &|_, _| 0.0);
    assert!((deviation(&fine, &coarse) - 0.1).abs() < 1e-5, "{}", deviation(&fine, &coarse));
    assert!(deviation(&coarse, &coarse) < 1e-6);
    // A level with no surface at all against one with: no measure.
    assert_eq!(deviation(&fine, &Shape::default()), f32::INFINITY);
}

/// A tube of `n` sides, radius 0.1, from z 0 to `length`, open at both
/// ends (or bent through `bend` radians about y over its length, its
/// ends then rings that are not in one plane with each other).
fn tube(n: usize, length: f32) -> Shape {
    let mut shape = Shape::default();
    for ring in 0..2 {
        for k in 0..n {
            let a = k as f32 / n as f32 * std::f32::consts::TAU;
            shape.points.push([0.1 * a.cos(), 0.1 * a.sin(), ring as f32 * length]);
        }
    }
    for k in 0..n as u32 {
        let (a, b) = (k, (k + 1) % n as u32);
        shape.triangles.push([a, b, b + n as u32]);
        shape.triangles.push([a, b + n as u32, a + n as u32]);
    }
    shape
}

#[test]
fn a_parts_openings_are_closed_and_a_straps_edge_is_not() {
    // A tube open at both ends (a sleeve, a waist): each end's ring is
    // fanned shut, eight triangles each.
    let sleeve = tube(8, 0.5);
    let closed = sleeve.closed();
    assert_eq!(closed.triangles.len(), sleeve.triangles.len() + 16);
    assert_eq!(closed.points.len(), sleeve.points.len() + 2);
    // A cap one level has and another lacks counts for nothing once
    // both are closed: the tube with its end shut against the tube open.
    assert!(deviations(&[closed.clone(), sleeve.clone()])[1] < 1e-5);
    // A strap: a long strip bent round half a circle, its one edge loop
    // far from any plane, left open.
    let mut strap = Shape::default();
    for k in 0..=16 {
        let a = k as f32 / 16.0 * std::f32::consts::PI;
        strap.points.push([a.cos(), a.sin(), 0.0]);
        strap.points.push([a.cos(), a.sin(), 0.05]);
    }
    for k in 0..16u32 {
        strap.triangles.push([2 * k, 2 * k + 2, 2 * k + 3]);
        strap.triangles.push([2 * k, 2 * k + 3, 2 * k + 1]);
    }
    assert_eq!(strap.closed().triangles.len(), strap.triangles.len());
}

#[test]
fn a_corner_where_fans_meet_is_walked_by_its_fans() {
    // A flat sheet of 4 by 4 cells with two cells cut out that share one
    // corner (1,1 and 2,2), and the sheet's outline. At that corner two
    // fans of triangles meet (one triangle each); the walk turns through
    // the fan its side is in, so the two cut cells' rims are one loop of
    // eight sides through the corner twice, fanned from the sheet's
    // middle, the corner itself (covering both cells), and the outline
    // another: the same caps whatever order the triangles come in (a
    // walk by a map's order would make two loops or one).
    let sheet = sheet(4, 1.0, &|_, _| 0.0);
    let cut: Vec<[u32; 3]> = sheet.triangles.iter().enumerate().filter(|(n, _)| ![10, 11, 20, 21].contains(n)).map(|(_, t)| *t).collect();
    let mut orders = vec![cut.clone(), cut.iter().rev().copied().collect::<Vec<_>>()];
    orders.push(cut.iter().cycle().skip(7).take(cut.len()).copied().collect());
    let mut caps: Vec<Vec<[u32; 3]>> = Vec::new();
    for triangles in orders {
        let closed = Shape { triangles, ..sheet.clone() }.closed();
        let middles: Vec<[i32; 2]> = closed.points[sheet.points.len()..].iter().map(|p| [(p[0] * 8.0).round() as i32, (p[1] * 8.0).round() as i32]).collect();
        assert_eq!(middles, vec![[4, 4], [4, 4]], "{middles:?}");
        assert_eq!(closed.caps, 16 + 8);
        caps.push(closed.triangles[closed.triangles.len() - closed.caps..].to_vec());
    }
    assert!(caps.iter().all(|got| got == &caps[0]));
    // The cut cells lie under the caps: a point in each is on the closed
    // surface, which the open sheet's samples land on.
    let closed = Shape { triangles: cut, ..sheet.clone() }.closed();
    let probe = Shape { points: vec![[0.375, 0.375, 0.0], [0.625, 0.625, 0.0], [0.4, 0.4, 0.0]], triangles: vec![[0, 1, 2]], caps: 0 };
    assert!(far_samples(&probe, &closed, 1e-6).is_empty(), "{:?}", far_samples(&probe, &closed, 1e-6));
}

#[test]
fn a_cap_is_landed_on_and_not_measured() {
    // The bear body's coarser wrist: one level's sleeve ends open in the
    // plane z = 0.5 (its cap, across the axis, is hidden by the hand);
    // the other's turns in there to a lining 1 cm inside, back to z =
    // 0.2, with nothing across the axis near 0.5. Every drawn point of
    // either lies within about a centimetre of the other's surface, the
    // cap's middle 9 cm from the lining: not sampled, it is no deviation;
    // the lining's own points land on the cap or the sleeve.
    let rings = |rings: &[(f32, f32)]| {
        let mut shape = Shape::default();
        let n = 16u32;
        for &(r, z) in rings {
            for k in 0..n {
                let a = k as f32 / n as f32 * std::f32::consts::TAU;
                shape.points.push([r * a.cos(), r * a.sin(), z]);
            }
        }
        for ring in 0..rings.len() as u32 - 1 {
            for k in 0..n {
                let (a, b) = (ring * n + k, ring * n + (k + 1) % n);
                shape.triangles.push([a, b, b + n]);
                shape.triangles.push([a, b + n, a + n]);
            }
        }
        shape
    };
    let lined = rings(&[(0.1, 0.0), (0.1, 0.5), (0.09, 0.5), (0.09, 0.2)]);
    let open = rings(&[(0.1, 0.0), (0.1, 0.5)]);
    let levels = deviations(&[lined.clone(), open.clone()]);
    assert!(levels[1] < 0.012, "{levels:?}");
    // Sampled as drawn, the open level's cap would have been 9 cm.
    let sampled = Shape { caps: 0, ..open.closed() };
    assert!((deviation(&lined.closed(), &sampled) - 0.09).abs() < 0.002, "{}", deviation(&lined.closed(), &sampled));
}

#[test]
fn a_point_is_placed_out_and_in_again() {
    // A child 1 m along x of a parent turned 90 degrees about z and
    // scaled 2: out to the root and in again to the child is where it
    // was.
    let (s, c) = (std::f32::consts::FRAC_1_SQRT_2, std::f32::consts::FRAC_1_SQRT_2);
    let child: Placing = ([1.0, 0.0, 0.0], [0.0, 0.0, 0.0, 1.0], [1.0; 3]);
    let parent: Placing = ([0.0, 0.0, 3.0], [0.0, 0.0, s, c], [2.0; 3]);
    let out = out_through([0.1, 0.0, 0.0], &[child, parent]);
    assert!((out[0] - 0.0).abs() < 1e-5 && (out[1] - 2.2).abs() < 1e-5 && (out[2] - 3.0).abs() < 1e-5, "{out:?}");
    let back = in_through(out, &[parent, child]);
    assert!((back[0] - 0.1).abs() < 1e-5 && back[1].abs() < 1e-5 && back[2].abs() < 1e-5, "{back:?}");
}
