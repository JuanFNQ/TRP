//! Tests of the shots' sounds.
use super::*;

/// Two clips' deck deals them in turn, the first second (an integer's
/// `Random.Range(0, 1)` is always 0), whatever the draws.
#[test]
fn a_deck_of_two_alternates() {
    let mut deck = Deck::default();
    let mut rng = Rng::new(7);
    let dealt: Vec<usize> = (0..6).map(|_| deck.deal(2, &mut rng)).collect();
    assert_eq!(dealt, vec![0, 1, 0, 1, 0, 1]);
    // One clip is always the first.
    assert_eq!(deck.deal(1, &mut rng), 0);
}

/// The AKM's blend values, 5, 30, 55 and 100 m: the close clip alone
/// under 5 m, the close and the distant blended to 30, the distant
/// alone to 55.
#[test]
fn a_shot_is_picked_by_its_distance() {
    let clip = |name: &str| (name.to_owned(), Samples::of(&trp_data::audio::sound::Clip { name: name.into(), channels: 1, frequency: 44100, samples: vec![0; 10] }));
    let bank = Bank { name: "akm_main".into(), volume: 1.0, clip_length: 1.6, blend: Some(vec![5.0, 30.0, 55.0, 100.0]), lists: [vec![clip("close")], vec![clip("distant")], vec![clip("far")]] };
    let (mut deck, mut rng) = (Deck::default(), Rng::new(1));
    let (first, second, share) = pick(&bank, &mut deck, &mut rng, 0.5);
    assert!(first.is_some() && second.is_none());
    assert_eq!(share, Some(1.0));
    let (first, second, share) = pick(&bank, &mut deck, &mut rng, 17.5);
    assert!(first.is_some() && second.is_some());
    // Halfway from 5 to 30: the close clip's share a half.
    assert!((share.unwrap() - 0.5).abs() < 1e-6);
    let (_, second, share) = pick(&bank, &mut deck, &mut rng, 40.0);
    assert!(second.is_none());
    assert_eq!(share, Some(1.0));
}
