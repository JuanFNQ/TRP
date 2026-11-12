//! Tests of the sounds of what is in the hands.
use super::*;

/// The square of the ergonomics' shortfall, within 0.1 and 0.2: an
/// AKM's 38 is at the top, a pistol's 90 at the floor, 60 between;
/// the overweight adds to it as to the kit.
#[test]
fn the_aiming_s_volume_goes_by_the_ergonomics() {
    assert!((aiming_volume(38.0, 0.0) - 0.2).abs() < 1e-6);
    assert!((aiming_volume(90.0, 0.0) - 0.1).abs() < 1e-6);
    assert!((aiming_volume(60.0, 0.0) - 0.16).abs() < 1e-5);
    assert!((aiming_volume(38.0, 0.5) - 0.3).abs() < 1e-6);
}

/// Every clip once before any again, and never the last one left while
/// another is (an integer's `Random.Range(0, n - 1)`).
#[test]
fn an_aiming_clip_is_not_played_again_until_all_have_been() {
    let mut rng = Rng::new(3);
    let mut left = Vec::new();
    for _ in 0..4 {
        let mut round: Vec<usize> = (0..5).map(|_| next_clip(&mut left, 5, &mut rng).expect("a clip")).collect();
        // The first drawn of a fresh round is never the list's last.
        assert_ne!(round[0], 4);
        round.sort();
        assert_eq!(round, [0, 1, 2, 3, 4]);
    }
    assert_eq!(next_clip(&mut Vec::new(), 1, &mut rng), Some(0));
    assert_eq!(next_clip(&mut Vec::new(), 0, &mut rng), None);
}
