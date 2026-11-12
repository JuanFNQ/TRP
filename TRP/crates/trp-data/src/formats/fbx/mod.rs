//! Reading a binary FBX file: the model of a weapon that is not the install's.
//! The weapon is `crate::custom`'s.
//!
//! The format is Autodesk's and is not published by them; what is here
//! follows the account of it that Blender's developers wrote from the
//! files ("FBX binary file format specification", code.blender.org) and
//! the transform rules of the FBX SDK's own documentation ("Computing
//! transformation matrices"). A file is a tree of records, each with a
//! name, a list of typed values and records of its own; the scene is in
//! the records `Objects` (models, geometries, materials) and
//! `Connections` (which hangs from which).

use std::collections::HashMap;

use anyhow::{anyhow, bail, ensure, Context, Result};

mod records;
mod model_scene;
mod motion;

pub use records::*;
pub use model_scene::*;
pub use motion::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_turn_about_each_axis_in_the_formats_order() {
        // A quarter turn about x takes y to z; then one about z takes x to y.
        let m = euler([90.0, 0.0, 90.0]);
        let y = direction(m, [0.0, 1.0, 0.0]);
        assert!((y[2] - 1.0).abs() < 1e-9, "{y:?}");
        let x = direction(m, [1.0, 0.0, 0.0]);
        assert!((x[1] - 1.0).abs() < 1e-9, "{x:?}");
    }

    #[test]
    fn an_affine_matrix_undone() {
        let m = mul(translation([3.0, -2.0, 0.5]), mul(euler([106.0, 20.9, -166.9]), scaling([0.01, 0.01, 0.01])));
        let back = inverse_affine(m).unwrap();
        let p = [0.3, -0.7, 1.9];
        let q = point(back, point(m, p));
        assert!((0..3).all(|k| (q[k] - p[k]).abs() < 1e-9), "{q:?}");
    }

    #[test]
    fn a_turn_as_a_quaternion_is_the_matrix_s() {
        let angles = [106.0, 20.9, -166.9];
        let (m, q) = (euler(angles), turn_of(angles));
        for v in [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.3, -0.7, 1.9]] {
            let (by_matrix, by_turn) = (direction(m, v), turned(q, v));
            assert!((0..3).all(|k| (by_matrix[k] - by_turn[k]).abs() < 1e-9), "{by_matrix:?} {by_turn:?}");
        }
        // And two turns one after the other are their matrices' product.
        let (second, r) = ([-12.0, 75.0, 31.0], [0.5, 0.25, -1.0]);
        let (by_matrix, by_turn) = (direction(mul(m, euler(second)), r), turned(turns(q, turn_of(second)), r));
        assert!((0..3).all(|k| (by_matrix[k] - by_turn[k]).abs() < 1e-9), "{by_matrix:?} {by_turn:?}");
    }

    #[test]
    fn a_curve_goes_straight_between_its_keys_and_holds_outside_them() {
        let (times, values) = ([10, 20, 40], [1.0, 3.0, -1.0]);
        assert_eq!(value_at(&times, &values, 0), Some(1.0));
        assert_eq!(value_at(&times, &values, 15), Some(2.0));
        assert_eq!(value_at(&times, &values, 20), Some(3.0));
        assert_eq!(value_at(&times, &values, 30), Some(1.0));
        assert_eq!(value_at(&times, &values, 99), Some(-1.0));
        assert_eq!(value_at(&[], &[], 5), None);
    }

    #[test]
    fn a_file_that_is_not_one_is_refused() {
        assert!(parse(b"not an fbx").is_err());
        assert!(read_motion(b"not an fbx").is_err());
    }
}
