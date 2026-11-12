//! Tests of a weapon of one's own: its kept pictures and the hand's hold.

use super::*;

/// What is kept is read back as it was.
#[test]
fn compressed_pictures_are_kept_and_read_back() {
    let picture = |channels: usize, blocks| Some(Picture { width: 8, height: 4, mips: 4, channels, data: (0..40u8).collect(), blocks });
    let pictures = Pictures { colour: picture(4, Some(Blocks::Bc1)), normal: picture(2, Some(Blocks::Bc5)), surface: None };
    let file = std::env::temp_dir().join(format!("trp-kept-picture-{}.bin", std::process::id()));
    write_kept(&file, &pictures).unwrap();
    let back = read_kept(&file).unwrap();
    let _ = std::fs::remove_file(&file);
    for (a, b) in [(&pictures.colour, &back.colour), (&pictures.normal, &back.normal), (&pictures.surface, &back.surface)] {
        assert_eq!(a.as_ref().map(|p| (p.width, p.height, p.mips, p.channels, &p.data, p.blocks)), b.as_ref().map(|p| (p.width, p.height, p.mips, p.channels, &p.data, p.blocks)));
    }
}

#[test]
fn a_picture_is_halved_to_its_limit_and_turned_over() {
    // Four pixels by two, the top row red, the bottom row blue.
    let data: Vec<u8> = [[255, 0, 0, 255]; 4].into_iter().chain([[0, 0, 255, 255]; 4]).flatten().collect();
    let picture = ready(4, 2, 4, data, 4);
    assert_eq!((picture.width, picture.height, picture.mips), (4, 2, 3));
    // The bottom row comes first.
    assert_eq!(&picture.data[..4], &[0, 0, 255, 255]);
    // 4 x 2, 2 x 1 and 1 x 1.
    assert_eq!(picture.data.len(), (8 + 2 + 1) * 4);
    let small = ready(4, 2, 4, picture.data[..32].to_vec(), 2);
    assert_eq!((small.width, small.height), (2, 1));
}

#[test]
fn the_hand_follows_the_nearest_hold() {
    // A weapon's bone with a marker, a reference and a magazine below it.
    let parents = vec![None, Some(0), Some(0), Some(0)];
    let hold = |bone: Option<usize>, palm: [f32; 3], shift: [f32; 3]| Hold { bone, palms: vec![palm], shift };
    let edits = PoseEdits {
        parents,
        weapon: 0,
        moved: vec![(3, [0.0, -0.05, 0.0])],
        hand: Some((1, 2, vec![hold(None, [0.0, -0.4, 0.0], [0.0, -0.06, 0.01]), hold(Some(3), [0.0, 0.1, -0.2], [0.0, -0.05, 0.0])], [0.0, -0.06, 0.01], 3.0)),
        donor_rest: Vec::new(),
    };
    let still = [0.0, 0.0, 0.0, 1.0];
    // On the first hold the marker goes by it alone, the reference with it.
    let mut pose = vec![([0.0; 3], still), ([0.0, -0.4, 0.0], still), ([0.0, -0.4, 0.0], still), ([0.0, -0.3, 0.0], still)];
    edits.apply(&mut pose);
    assert!((pose[1].0[1] + 0.46).abs() < 1e-4 && (pose[1].0[2] - 0.01).abs() < 1e-4, "{:?}", pose[1].0);
    let near = |a: [f32; 3], b: [f32; 3]| sub(a, b).iter().all(|d| d.abs() < 1e-6);
    assert!(near(pose[2].0, [0.0, -0.46, 0.01]), "{:?}", pose[2].0);
    assert!(near(pose[3].0, [0.0, -0.35, 0.0]), "{:?}", pose[3].0);
    // Holding the magazine, wherever it has been carried, the hand
    // keeps its place on it.
    let mut pose = vec![([0.0; 3], still), ([0.3, 0.2, -0.6], still), ([0.0, -0.4, 0.0], still), ([0.3, 0.1, -0.4], still)];
    edits.apply(&mut pose);
    let between = sub(pose[1].0, pose[3].0);
    assert!((between[1] - 0.1).abs() < 2e-3 && (between[2] + 0.2).abs() < 2e-3, "{between:?}");
}
