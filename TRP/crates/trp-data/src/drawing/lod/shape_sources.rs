//! The shapes of skins and trees, placed in and out through their nodes.

use super::*;

/// Each level's surface of skins drawn without a place of their own (a
/// body part's, a worn skin's: `arms::load_levels`, `dress::load_levels`),
/// their meshes in their own axes, as at their bind pose: the meshes each
/// level draws (`levels_of`: a mesh and its levels, a bit each). None where
/// a mesh cannot be read.
pub fn skin_shapes(assets: &Assets, levels: usize, meshes: &[(ObjRef, u8)]) -> Option<Vec<Shape>> {
    let mut shapes = vec![Shape::default(); levels];
    for &(mesh, bits) in meshes {
        let data = crate::unity::extract::read_mesh(assets, mesh).ok()?;
        for (level, shape) in shapes.iter_mut().enumerate() {
            if bits & (1 << level) != 0 {
                for indices in &data.submeshes {
                    shape.add(&data.positions, indices, &|p| p);
                }
            }
        }
    }
    Some(shapes)
}

/// A place under a parent: its place, turn (x, y, z, w) and scale, as a
/// transform stores them.
pub type Placing = ([f32; 3], [f32; 4], [f32; 3]);

/// A point under a chain of places, the nearest first, taken out to the
/// last's parent: each scaled, turned and moved in turn.
pub fn out_through(mut p: [f32; 3], chain: &[Placing]) -> [f32; 3] {
    for (t, q, s) in chain {
        let scaled = [p[0] * s[0], p[1] * s[1], p[2] * s[2]];
        let turned = rotate(*q, scaled);
        p = [turned[0] + t[0], turned[1] + t[1], turned[2] + t[2]];
    }
    p
}

/// A point taken in through a chain of places, the outermost first: the
/// inverse of `out_through` on the same chain reversed.
pub fn in_through(mut p: [f32; 3], chain: &[Placing]) -> [f32; 3] {
    for (t, q, s) in chain {
        let moved = [p[0] - t[0], p[1] - t[1], p[2] - t[2]];
        let turned = rotate([-q[0], -q[1], -q[2], q[3]], moved);
        p = std::array::from_fn(|k| if s[k].abs() > 1e-12 { turned[k] / s[k] } else { turned[k] });
    }
    p
}

fn rotate(q: [f32; 4], v: [f32; 3]) -> [f32; 3] {
    let (x, y, z, w) = (q[0], q[1], q[2], q[3]);
    let t = [2.0 * (y * v[2] - z * v[1]), 2.0 * (z * v[0] - x * v[2]), 2.0 * (x * v[1] - y * v[0])];
    [v[0] + w * t[0] + (y * t[2] - z * t[1]), v[1] + w * t[1] + (z * t[0] - x * t[2]), v[2] + w * t[2] + (x * t[1] - y * t[0])]
}

/// Each level's surface of a tree's group (a rigid model's: a weapon's
/// part, a thing on the ground, a prop), in the axes of the object the
/// group is on: the meshes of the objects whose renderers each level lists
/// (`drawn`: an object's renderer by its index, the levels it is drawn at
/// and its mesh's corners and triangles), each placed out through its
/// parents (`parent`, `placing`) to the tree's root and in again to the
/// group's object. Where an object's mesh is in the root's axes already
/// (`in_root`), it is not placed out. None where no level has a mesh.
pub fn tree_shapes(
    levels: usize,
    group_node: usize,
    parent: &dyn Fn(usize) -> Option<usize>,
    placing: &dyn Fn(usize) -> Placing,
    drawn: &[(usize, u8, bool, Arc<MeshPoints>)],
) -> Option<Vec<Shape>> {
    let chain_out = |mut node: usize| -> Vec<Placing> {
        let mut chain = vec![placing(node)];
        while let Some(up) = parent(node) {
            node = up;
            chain.push(placing(node));
        }
        chain
    };
    let mut chain_in = chain_out(group_node);
    chain_in.reverse();
    let mut shapes = vec![Shape::default(); levels];
    for (node, bits, in_root, mesh) in drawn {
        let out = if *in_root { Vec::new() } else { chain_out(*node) };
        let place = |p: [f32; 3]| in_through(out_through(p, &out), &chain_in);
        for (level, shape) in shapes.iter_mut().enumerate() {
            if bits & (1 << level) != 0 {
                for indices in &mesh.triangles {
                    shape.add(&mesh.points, indices, &place);
                }
            }
        }
    }
    shapes.iter().any(|shape| !shape.is_empty()).then_some(shapes)
}

/// A mesh's corners and its triangle lists, for measuring.
#[derive(Debug, Default)]
pub struct MeshPoints {
    pub points: Vec<[f32; 3]>,
    pub triangles: Vec<Vec<u32>>,
}
