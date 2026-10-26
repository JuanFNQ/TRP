//! The game's deferred decals (`DeferredDecalRenderer`; grenades spec, "The
//! scorch"): the box a static decal is laid in (`AddCubeToMesh`,
//! `DeferredDecalMeshHelper.GenerateVerts`), and what its shader
//! (`Decal/Deferred DecalShader Diffuse+Normals`) does at a surface's point
//! inside it: where on its picture, and whether the surface faces it. In the
//! game's axes. The app draws it (`decal_view`).

/// The most a static decal's half size across is (`MaxStaticDecalSize`).
pub const MAX_STATIC_DECAL_SIZE: f32 = 0.4;

/// The least a surface's normal's dot with the decal's must be for the
/// decal to be laid on it (the shader's `dot − 0.8 < 0` discard).
pub const LEAST_FACING: f32 = 0.8;

/// A decal's box: its middle, its three ways (across, along its normal,
/// across: `right`, `up`, `fwd` in the game's code), and its half sizes
/// along each.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DecalBox {
    pub center: [f32; 3],
    pub right: [f32; 3],
    pub normal: [f32; 3],
    pub forward: [f32; 3],
    pub half: [f32; 3],
}

/// A static decal's box (`AddCubeToMesh`): its half size across drawn
/// between the decal's two sizes at `size_share` (`Random.Range`), held
/// within nought and `MAX_STATIC_DECAL_SIZE`; its half height along the
/// normal the renderer's projector height; its ways across the normal
/// crossed with the world's up (its right where the normal is within 0.999
/// of up either way) and the normal crossed with that, both turned about
/// the normal by `turn_degrees` where the decal turns at random (a whole
/// number from 0 to 358 in the game: `Random.Range(0, 359)`).
pub fn static_box(position: [f32; 3], normal: [f32; 3], sizes: [f32; 2], size_share: f32, projector_height: f32, turn_degrees: Option<f32>) -> DecalBox {
    let size = (sizes[0] + (sizes[1] - sizes[0]) * size_share).clamp(0.0, MAX_STATIC_DECAL_SIZE);
    let n = unit(normal);
    let across = if dot(n, [0.0, 1.0, 0.0]).abs() > 0.999 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] };
    let mut right = unit(cross(n, across));
    let mut forward = unit(cross(n, right));
    if let Some(degrees) = turn_degrees {
        right = turn_about(n, degrees, right);
        forward = turn_about(n, degrees, forward);
    }
    DecalBox { center: position, right, normal: n, forward, half: [size, projector_height, size] }
}

impl DecalBox {
    /// Where on the decal's picture a point of the scene is (`((x/s + 1)/2,
    /// (z/s + 1)/2)` by its offsets along the two ways across), or none
    /// outside the box.
    pub fn picture_at(&self, point: [f32; 3]) -> Option<[f32; 2]> {
        let d = sub(point, self.center);
        let local = [dot(self.right, d), dot(self.normal, d), dot(self.forward, d)];
        if (0..3).any(|k| local[k].abs() > self.half[k]) {
            return None;
        }
        Some([(local[0] / self.half[0] + 1.0) * 0.5, (local[2] / self.half[2] + 1.0) * 0.5])
    }

    /// Whether a surface of that normal takes the decal.
    pub fn faces(&self, surface_normal: [f32; 3]) -> bool {
        dot(unit(surface_normal), self.normal) >= LEAST_FACING
    }
}

/// `Quaternion.AngleAxis(degrees, axis) * v` (the axis a unit).
fn turn_about(axis: [f32; 3], degrees: f32, v: [f32; 3]) -> [f32; 3] {
    let (s, c) = degrees.to_radians().sin_cos();
    let k = cross(axis, v);
    let along = dot(axis, v) * (1.0 - c);
    std::array::from_fn(|i| v[i] * c + k[i] * s + axis[i] * along)
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn unit(a: [f32; 3]) -> [f32; 3] {
    let l = dot(a, a).sqrt();
    if l > 1e-12 {
        a.map(|c| c / l)
    } else {
        a
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: [f32; 3], b: [f32; 3]) -> bool {
        (0..3).all(|k| (a[k] - b[k]).abs() < 1e-5)
    }

    /// A grenade's scorch on the ground: the decal's sizes (1.93 to 2) held
    /// at 0.4, 0.7 along the normal; with the normal up, its ways the
    /// world's back and left (the normal crossed with the world's right,
    /// then with that), turned about the normal as the game turns them.
    #[test]
    fn a_grenade_s_scorch_on_the_ground() {
        let b = static_box([1.0, 0.0, 2.0], [0.0, 1.0, 0.0], [1.933, 2.0], 0.5, 0.7, None);
        assert_eq!(b.half, [0.4, 0.7, 0.4]);
        assert!(near(b.right, [0.0, 0.0, -1.0]) && near(b.forward, [-1.0, 0.0, 0.0]), "{b:?}");
        // `AngleAxis(90, up) * forward` is the right in the game's axes.
        let turned = static_box([1.0, 0.0, 2.0], [0.0, 1.0, 0.0], [1.933, 2.0], 0.5, 0.7, Some(90.0));
        assert!(near(turn_about([0.0, 1.0, 0.0], 90.0, [0.0, 0.0, 1.0]), [1.0, 0.0, 0.0]));
        assert!(near(turned.right, [-1.0, 0.0, 0.0]) && near(turned.forward, [0.0, 0.0, 1.0]), "{turned:?}");
        // Its picture across the box: the middle at a half each way, an edge
        // at nought; past the box, nothing.
        assert_eq!(b.picture_at([1.0, 0.0, 2.0]), Some([0.5, 0.5]));
        let edge = b.picture_at([1.0, 0.0, 2.399]).expect("at its edge");
        assert!((edge[0] - 0.00125).abs() < 1e-4 && (edge[1] - 0.5).abs() < 1e-5, "{edge:?}");
        assert_eq!(b.picture_at([1.0, 0.0, 2.41]), None);
        assert_eq!(b.picture_at([1.0, 0.71, 2.0]), None);
        // The ground takes it; a wall does not, nor a slope beyond about 37°.
        assert!(b.faces([0.0, 1.0, 0.0]) && b.faces([0.0, 0.81, 0.586]));
        assert!(!b.faces([1.0, 0.0, 0.0]) && !b.faces([0.0, 0.79, 0.613]));
    }
}
