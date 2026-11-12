//! Tests of the voice: grunts, cries, breathing, a face shield's muffle and the choice of clips.
use super::*;

fn speaker() -> Speaker {
    let mut s = Speaker::new(1);
    let clips = |n: usize, mask: u32| (0..n).map(|i| BankClip { mask, length: 0.5 + i as f32 * 0.1 }).collect::<Vec<_>>();
    let mut breath = clips(1, 0x2000);
    breath.extend([BankClip { mask: 0x1800, length: 10.8 }, BankClip { mask: 0x0401, length: 14.6 }, BankClip { mask: 0x0402, length: 7.9 }]);
    // The hurt's bank chains to the disappointment's at 70 in a hundred
    // (`Bear_2_Eng`'s), which the player's speaker never queues.
    let disappointed = Chain { trigger: trigger::BEING_HURT_DISAPPOINTMENT, probability: 70 };
    s.set_banks(vec![
        Bank { trigger: trigger::BEING_HURT, importance: 2, ignore_tags: false, clips: clips(13, 0), chain: Some(disappointed) },
        Bank { trigger: trigger::BREATH, importance: 0, ignore_tags: false, clips: breath, chain: None },
        Bank { trigger: trigger::DEATH, importance: 3, ignore_tags: true, clips: clips(7, 0), chain: None },
        Bank { trigger: trigger::AGONY, importance: 3, ignore_tags: true, clips: clips(4, 0), chain: None },
        Bank { trigger: trigger::FIGHT, importance: 0, ignore_tags: false, clips: clips(41, 0), chain: None },
    ]);
    s
}

/// A bot's speaker with the same banks, its hurt's chain certain, and
/// a disappointment's bank (importance 1: an injured one's clip and a
/// dying one's).
fn bot() -> Speaker {
    let mut banks = speaker().banks;
    banks[0].chain = Some(Chain { trigger: trigger::BEING_HURT_DISAPPOINTMENT, probability: 100 });
    banks.push(Bank {
        trigger: trigger::BEING_HURT_DISAPPOINTMENT,
        importance: 1,
        ignore_tags: false,
        clips: vec![BankClip { mask: tag::INJURED, length: 4.9 }, BankClip { mask: tag::BADLY_INJURED | tag::DYING, length: 2.3 }],
        chain: None,
    });
    let mut s = Speaker::of_ai(1);
    s.set_banks(banks);
    s
}

/// A bot's hit: no grunt of its own (`ApplyHitDebuff`'s is the
/// player's), the damage's grunt as anyone's, and a fall's damage on
/// painkillers as any other's (4 or less, nothing).
#[test]
fn a_bot_grunts_by_its_damage_alone() {
    let mut s = bot();
    s.hit(Part::Chest, 30.0, DamageKind::Bullet, 1.0, &calm());
    assert!(s.take_said().is_empty(), "no grunt of the hit's own");
    assert!(s.is_aware(1.5), "made aware all the same");
    s.damaged(DamageKind::Bullet, 30.0, 2.0, &calm());
    assert!(matches!(s.take_said()[..], [Said::Phrase(Phrase { bank: 0, .. })]));
    let numb = Situation { on_painkillers: true, painkiller: true, ..calm() };
    let mut s = bot();
    s.damaged(DamageKind::Fall, 3.0, 1.0, &numb);
    assert!(s.take_said().is_empty(), "a bot's fall is any damage");
    let mut s = speaker();
    s.damaged(DamageKind::Fall, 3.0, 1.0, &numb);
    assert_eq!(s.take_said().len(), 1, "the player's, even so");
}

/// A bot's grunt queues its bank's chain with the tags it was said
/// with: said as the grunt's clip and the tenth after it are over,
/// injured though the bot is dying by then (`3-bot-shots`, frame 3863);
/// the player's never. A phrase said over the grunt drops what was
/// queued, and so does a shut.
#[test]
fn a_bot_s_grunt_is_followed_by_its_chain() {
    let injured = Situation { status: Status::Injured, ..calm() };
    let dying = Situation { status: Status::Dying, ..calm() };
    let mut s = bot();
    s.damaged(DamageKind::Bullet, 20.0, 1.0, &injured);
    let [Said::Phrase(grunt)] = s.take_said()[..] else { panic!() };
    let over = 1.0 + 0.5 + grunt.clip as f32 * 0.1 + BUFFER;
    s.update(over - 0.01, &dying);
    assert!(s.take_said().is_empty(), "still busy");
    s.update(over + 0.01, &dying);
    assert_eq!(s.take_said(), vec![Said::Phrase(Phrase { bank: 5, clip: 0, muffled: false })], "the injured one's clip");
    assert_eq!(s.importance(), 1);
    // The player's speaker: the grunt let go of, nothing after it.
    let mut s = speaker();
    s.damaged(DamageKind::Bullet, 20.0, 1.0, &injured);
    s.take_said();
    s.update(10.0, &calm());
    assert_eq!(s.take_said(), vec![Said::Release]);
    // A death over the grunt drops the chain; a head's shut too.
    let mut s = bot();
    s.struck(Part::Chest);
    s.damaged(DamageKind::Bullet, 20.0, 1.0, &injured);
    s.died(DamageKind::Bullet, 1.1, &Situation { alive: false, ..dying });
    assert_eq!(s.take_said().len(), 2);
    s.update(20.0, &Situation { alive: false, ..dying });
    assert_eq!(s.take_said(), vec![Said::Release], "no chain after the death's cry");
    let mut s = bot();
    s.struck(Part::Head);
    s.damaged(DamageKind::Bullet, 40.0, 1.0, &injured);
    s.died(DamageKind::Bullet, 1.0, &Situation { alive: false, ..dying });
    s.take_said();
    s.update(1.01, &Situation { alive: false, ..dying });
    assert_eq!(s.take_said(), vec![Said::Release]);
    s.update(20.0, &Situation { alive: false, ..dying });
    assert!(s.take_said().is_empty(), "nothing queued after the shut");
}

/// A damage that leaves a bot dying, the speaker free: its health's
/// change asks the breathing first (`HealthChangedEvent` before
/// `ApplyDamageEvent`), and the grunt cuts the breath on the same frame
/// (`3-bot-shots`, frame 3412: `scav4_breath_death`, then
/// `scav4_hit_08`); once the grunt is over the dying's breath again
/// (frame 3444). A death by the head on such a frame shuts it all.
#[test]
fn the_dying_breath_comes_before_the_grunt_and_after_it() {
    let dying = Situation { status: Status::Dying, ..calm() };
    let mut s = bot();
    s.banks[0].chain = None;
    s.breath(1.0, &dying);
    s.damaged(DamageKind::Bullet, 77.8, 1.0, &dying);
    let said = s.take_said();
    assert!(matches!(said[..], [Said::Phrase(Phrase { bank: 1, clip: 0, .. }), Said::Phrase(Phrase { bank: 0, .. })]), "{said:?}");
    s.update(3.0, &dying);
    assert_eq!(s.take_said(), vec![Said::Phrase(Phrase { bank: 1, clip: 0, muffled: false })], "the dying's breath once the grunt is over");
}

/// Another's face shield seen from outside: muffled, and nothing said
/// for it; the breath it needs comes the next time the breathing is
/// asked, quietly (importance -1).
#[test]
fn another_s_face_shield_muffles_and_says_nothing() {
    let mut s = bot();
    s.face_shield_seen(true, true);
    assert!(s.muffled && s.take_said().is_empty());
    s.breath(1.0, &calm());
    assert!(matches!(s.take_said()[..], [Said::Phrase(Phrase { bank: 1, clip: 2, muffled: true })]));
    assert_eq!(s.importance(), -1);
}

/// The group's tag goes with every phrase: a clip for a group is drawn
/// only where the speaker is in one.
#[test]
fn the_group_s_tag_chooses_the_clip() {
    let mut s = bot();
    s.banks[1].clips = vec![BankClip { mask: tag::HEALTHY | tag::SOLO, length: 1.0 }, BankClip { mask: tag::HEALTHY | tag::COOP, length: 1.0 }];
    s.excluded[1] = vec![false; 2];
    s.face_shield_seen(true, true);
    s.breath(1.0, &calm());
    assert!(matches!(s.take_said()[..], [Said::Phrase(Phrase { bank: 1, clip: 0, .. })]));
    s.heavy_breath = false;
    s.group = tag::COOP;
    s.breath(5.0, &calm());
    assert!(matches!(s.take_said()[..], [Said::Phrase(Phrase { bank: 1, clip: 1, .. })]));
}

/// A berserk's start says the fight's phrase (`Say(OnFight)`), at its
/// bank's importance, 0: not over a grunt still being said.
#[test]
fn a_berserk_shouts() {
    let mut s = speaker();
    s.berserk(1.0, &calm());
    let said = s.take_said();
    assert!(matches!(said.as_slice(), [Said::Phrase(Phrase { bank: 4, .. })]), "{said:?}");
    s.update(10.0, &calm());
    s.take_said();
    s.hit(Part::Chest, 20.0, DamageKind::Bullet, 20.0, &calm());
    s.take_said();
    s.berserk(20.1, &calm());
    assert!(s.take_said().is_empty(), "the grunt goes on");
}

/// The menu's mumble: the mutter's bank while unaware, the fight's
/// where aggressive (a middle click, a key pressed twice) or aware (a
/// shot of one's own for 15 s, an enemy's hit for 20); the commands'
/// clips by the awareness's tag; a phrase the voice lacks, nothing.
#[test]
fn the_mumble_is_a_mutter_or_a_fight() {
    let roger = Bank { trigger: 67, importance: 2, ignore_tags: false, clips: vec![BankClip { mask: 0, length: 1.0 }, BankClip { mask: tag::COMBAT, length: 1.0 }], chain: None };
    let mutter = Bank { trigger: trigger::MUTTER, importance: 0, ignore_tags: true, clips: vec![BankClip { mask: 0, length: 1.0 }], chain: None };
    let mut s = speaker();
    let mut banks = s.banks.clone();
    banks.extend([roger, mutter]);
    s.set_banks(banks);
    let (fight, roger, mutter) = (4, 5, 6);
    let said = |s: &mut Speaker| match s.take_said().as_slice() {
        [Said::Phrase(phrase)] => (phrase.bank, phrase.clip),
        other => panic!("{other:?}"),
    };
    assert!(s.say(trigger::MUMBLE, false, 1.0, &calm()));
    assert_eq!(said(&mut s).0, mutter);
    assert!(s.say(trigger::MUMBLE, true, 5.0, &calm()));
    assert_eq!(said(&mut s).0, fight);
    // Unaware, Roger's clip for the unaware alone, every time.
    for at in [20.0, 30.0, 40.0] {
        assert!(s.say(67, false, at, &calm()));
        assert_eq!(said(&mut s), (roger, 0));
    }
    s.aware(15.0, 50.0);
    assert!(s.say(trigger::MUMBLE, false, 60.0, &calm()), "aware: the fight's");
    assert_eq!(said(&mut s).0, fight);
    assert!(!s.is_aware(65.0));
    s.hit(Part::Chest, 1.0, DamageKind::Fall, 70.0, &calm());
    assert!(!s.is_aware(70.5), "a fall is no enemy's");
    s.hit(Part::Chest, 1.0, DamageKind::GrenadeFragment, 80.0, &calm());
    assert!(s.is_aware(99.9) && !s.is_aware(100.0));
    assert!(!s.say(30, false, 110.0, &calm()) && !s.has(30), "no bank, nothing");
    assert!(s.has(trigger::MUMBLE));
}

fn calm() -> Situation {
    Situation { alive: true, status: Status::Healthy, on_painkillers: false, painkiller: false, audible: false }
}

#[test]
fn tags_compare_field_by_field() {
    assert!(compare(0, tag::DYING | tag::SOLO));
    assert!(compare(0x0401, tag::HEALTHY | tag::UNAWARE | tag::SOLO));
    assert!(!compare(0x0402, tag::HEALTHY | tag::UNAWARE | tag::SOLO), "aware against unaware");
    assert!(!compare(0x1800, tag::HEALTHY | tag::AWARE), "hurt against healthy");
    assert!(compare(0x1800, tag::BADLY_INJURED));
}

#[test]
fn every_clip_once_a_round_and_never_twice_running() {
    // The first round has all 13; each after it 12, the clip said
    // last held out of it (`Match` clears the others' marks as it
    // takes the round's last).
    let mut s = speaker();
    let drawn: Vec<usize> = (0..13 + 12 * 5).map(|_| s.draw(0, tag::HEALTHY).unwrap()).collect();
    let distinct = |w: &[usize]| w.iter().enumerate().all(|(i, c)| !w[..i].contains(c));
    assert!(distinct(&drawn[..13]));
    for round in drawn[13..].chunks(12) {
        assert!(distinct(round), "{round:?}");
    }
    assert!(drawn.windows(2).all(|w| w[0] != w[1]), "never twice running");
}

#[test]
fn a_hit_grunts_once_and_painkillers_hush_small_ones() {
    let mut s = speaker();
    let now = 1.0;
    s.damaged(DamageKind::Bullet, 15.0, now, &calm());
    s.hit(Part::LeftArm, 15.0, DamageKind::Bullet, now, &calm());
    assert_eq!(s.take_said().len(), 1, "one grunt a hit");
    let numb = Situation { on_painkillers: true, painkiller: true, ..calm() };
    s.damaged(DamageKind::Bullet, 3.0, 10.0, &numb);
    s.hit(Part::LeftArm, 3.0, DamageKind::Bullet, 10.0, &numb);
    assert!(s.take_said().is_empty(), "4 or less on painkillers");
    s.damaged(DamageKind::Fall, 2.0, 20.0, &numb);
    assert_eq!(s.take_said().len(), 1, "a fall even so");
}

#[test]
fn a_bleeding_grunts_every_sixteenth_tick() {
    let mut s = speaker();
    s.effect_started(Kind::LightBleeding, Part::LeftLeg);
    let mut grunts = 0;
    for t in 0..48 {
        s.damaged(DamageKind::LightBleeding, 0.5, t as f32 * 5.0, &calm());
        grunts += s.take_said().len();
    }
    assert_eq!(grunts, 3);
    s.effect_started(Kind::HeavyBleeding, Part::Chest);
    let mut grunts = 0;
    for t in 0..64 {
        s.damaged(DamageKind::HeavyBleeding, 0.5, 1000.0 + t as f32 * 5.0, &calm());
        grunts += s.take_said().len();
    }
    assert_eq!(grunts, 2, "two at work: every 32nd");
}

#[test]
fn a_death_cries_unless_the_head_was_hit() {
    let mut s = speaker();
    s.hit(Part::Chest, 80.0, DamageKind::Bullet, 1.0, &calm());
    s.take_said();
    let dead = Situation { alive: false, status: Status::Dying, ..calm() };
    s.died(DamageKind::Bullet, 1.0, &dead);
    assert!(matches!(s.take_said()[..], [Said::Phrase(Phrase { bank: 2, .. })]), "over the grunt: the death's");
    let mut s = speaker();
    s.hit(Part::Head, 80.0, DamageKind::Bullet, 1.0, &calm());
    s.take_said();
    s.died(DamageKind::Bullet, 1.0, &dead);
    assert!(s.take_said().is_empty());
    s.update(1.05, &dead);
    assert_eq!(s.take_said(), vec![Said::Release], "the grunt let go of");
}

#[test]
fn breathing_runs_on_while_needed_and_plays_out_after() {
    let mut s = speaker();
    let tired = Situation { audible: true, ..calm() };
    s.audible_changed(0.0, &tired);
    assert!(matches!(s.take_said()[..], [Said::Phrase(Phrase { bank: 1, clip: 3, .. })]), "the sprint's breath");
    s.update(8.0, &tired);
    assert!(matches!(s.take_said()[..], [Said::Phrase(Phrase { bank: 1, clip: 3, .. })]), "again at its end");
    s.audible_changed(9.0, &calm());
    assert!(s.take_said().is_empty() && !s.heavy_breath, "no longer needed: plays on");
    s.update(16.1, &calm());
    assert_eq!(s.take_said(), vec![Said::Release]);
}

#[test]
fn a_face_shield_down_breathes_softly_and_up_lets_it_play_out() {
    let mut s = speaker();
    s.face_shield(true, true, 0.0, &calm());
    assert!(matches!(s.take_said()[..], [Said::Phrase(Phrase { bank: 1, clip: 2, muffled: true })]));
    assert_eq!(s.importance(), -1);
    s.face_shield(false, true, 5.0, &calm());
    assert!(s.busy(5.0), "not shut: it plays out");
    s.face_shield(true, false, 30.0, &calm());
    assert!(!s.muffled, "no armour on the jaw");
}
